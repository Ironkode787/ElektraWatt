<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: MIT OR Apache-2.0
-->

# ew-schema

The persisted record type of every entity kind of the ElektraWatt project format, the kind registry,
the per-kind schema versions and the generated JSON Schemas (`schemas/<format>/`). It is a permissive
crate (MIT OR Apache-2.0): third parties read and write EW files with it without linking GPL code
(FR-COL-012). `ew-model` wraps these structs in tables and never defines a second data type (ADR-0001).

v0 (WP-S1.02) holds the golden subset: `project`, `import_lineage`, `structure_node`, `section`,
`external_ref`, `page`, `device` (aspects `strip`, `terminal`, `cable`), `function`, `representation`,
`connection`, `junction`, `conductor`, `potential_def`, `bond`, `report_block`, and the library documents
`part`, `symbol`, `fndef` and `form`. Every other kind of doc 05 §2.2 is in the registry as reserved.

Field-level truth: `docs/design/03-domain-model.md`. Encoding: `docs/design/05-project-format-and-persistence.md`
§3. Versioning: ADR-0011 and doc 05 §10.

## Canonical form

The compact `serde_json` output of a `Record` is its canonical line (doc 05 §3.1):

1. `$k`, `id`, `$v` (omitted when 1);
2. known members in **declaration order**: the order of the fields in the struct is the member order on
   disk;
3. flattened device aspects (`strip`, `terminal`, `cable`, then the later aspects);
4. `extra`, in byte-wise key order.

Rules every struct follows:

- **Defaults are omitted.** Every optional member has `#[serde(default, skip_serializing_if = …)]`: `None`,
  empty arrays and maps, `false`, empty text and default enum variants are not written. A member whose
  schema default is not the type's default (`CpDef.physical`, `Strip.field_side`) names its own default.
- **Every JSON object has `extra`**, flattened last: `pub extra: Extra` with
  `#[serde(flatten, deserialize_with = "de_extra")]`. Records and nested value objects alike, so an
  unknown member survives a load and a save at any depth. A writer never puts a known member name into
  `extra` (it would be written twice); `Record::validate` checks it.
- **No inline struct variants.** A data variant whose payload is an object is a newtype variant over a
  payload struct with its own `extra` (`NumberSpec::Assigned(AssignedNumber)`,
  `JunctionStyle::Bevel(BevelSpec)`, `LineageAnchor::Copy(LineageCopy)`, `ItemStatus::Deprecated(Deprecation)`,
  `InternalKind::PassThrough(PassThroughSpec)`, `Overflow::ShrinkToFit(ShrinkSpec)`); the JSON is the same
  one-member object. `tests/unknown_members.rs` injects an unknown member into every object of every
  example line and checks that it survives.
- **Every persisted enum is open and read tag first.** Its last variant is `#[serde(skip)] Unknown(Value)`;
  the container carries `#[serde(remote = "Self")]` and `#[schemars(rename = "<Name>")]`, and the module
  lists the enum in `open_enum!(…)`, which writes `Serialize` and the tag-first `Deserialize`
  (`src/track.rs`). Hand-written enums (`Csa`, `ColourCode`, `Slot`, `TechVal`) follow the same rule, and
  foreign enums get it through `Open<T>`. The rule (doc 05 §10.3 rule 3, ADR-0011 amendment 2): a tag (a
  string, or the key of a one-member object) that names no known variant is newer data and is kept as
  `Unknown`; a known tag whose payload does not parse is a **malformed variant**; a value that is neither a
  string nor a one-member object is a schema violation.
- **Enums** are snake_case unit variants or one-member objects for data variants.
- **No floats anywhere**; integers within ±(2⁵³ − 1). Typed members enforce it, `check_value` covers
  unknown content.
- **`BTreeMap`, never `HashMap`.** Sets and keyed lists keep their canonical order in the type, so a value
  built in memory is written sorted, and the reader rejects unsorted or repeated input
  (`SchemaError::UnsortedList`, `SchemaError::DuplicateKey`, each with the path of the element):
  - a **set** is a `Set<T>`, sorted by the compact JSON text of its elements, byte-wise (`aliases`,
    `translations`, `keywords`, `manual.include`, `manual.exclude`);
  - a **keyed list** is a `KeyedList<T>` (`T: Keyed`), sorted by key in the key type's natural order
    (`cp_overrides`, `cp_pattern`, `cps` of symbols and function definitions by `idx`; `tech_data` and
    `free_props` by `key`; `pin_map` by `symbol_cp`; `text_slots` by `slot`).
- **Strings are NFC** (doc 05 §3.1). `ew-schema` stays strict: `MlText` and `Ident` reject non-NFC text.
  The format loader (`ew-format`) calls `normalize_nfc` on the parsed value of every line **before** typed
  deserialisation and marks the file as non-canonical when it returns `true`; plain `String` members are
  covered by the same pass, so the typed reader never sees a non-NFC string from a file.
- **`Orient2` has no catch-all**: its eight values are the dihedral group of the square, which is closed, so
  a newer format cannot add a value. It is the one persisted enumeration without `Unknown`.
- Field types from the foundation crates name a schema stand-in with `#[schemars(with = "…")]`
  (`src/proxy.rs`). A stand-in is never laxer than the serde implementation where JSON Schema can say it;
  what it cannot say (NFC, a reduced grid step) the reader enforces.

## Reading records

`ew-format` builds on four functions (doc 05 §3.6, §6.3, §10.3):

1. `parse_value(text)` parses one line or document and rejects a member name repeated within one object
   at any depth (`SchemaError::DuplicateMember { path, key }`). `ew-format` relies on this: a `Value`
   built by `serde_json` itself keeps the last of two equal members silently, so the loader must not parse
   with `serde_json::from_str::<Value>`.
2. `normalize_nfc(&mut value)` (see above).
3. `Record::load(value)` / `LibDocument::load(value)` is the loader's typed reading: a malformed variant is
   kept verbatim as that enum's `Unknown`, so nothing is lost and the record stays editable, and it is
   returned as a `MalformedVariant { path, variant, message }`, which the loader reports as
   **EW-CORE-017** (error, blocks release; ADR-0011 amendment 2).
4. `Record::from_value(value)` / `from_json(text)` and serde's `Deserialize` are the strict readers for
   third parties and tests: a malformed variant is an error (`SchemaError::MalformedVariant`).

Which caller uses what: **`ew-format`** reads every line with `parse_value` → `normalize_nfc` → `load`;
**third parties and tests** use the strict readers. `load` keeps only the malformed enum value as
`Unknown`, at the innermost enum whose payload fails (a malformed `BridgeKind` inside a well-formed
`ConnectionKind::Bridge` leaves the bridge typed); the rest of the record stays typed, and the finding's
path names the value. Any failure inside a known variant's payload makes that enum value malformed,
whatever the cause (the message names the inner path). A malformed value outside every enum payload (a
wrong type, a missing member, an unsorted set) fails the whole record in both modes; `ew-format` then
quarantines the line verbatim (EW-CORE-007).

Every error of typed reading carries the JSON Pointer of the value it concerns, relative to the record
(`SchemaError::path`): `"/a"`, `"/kind/bridge/kind"`, `"/tech_data/1"`. Deserialising an inner type
directly with `serde_json` gives the same rules without paths. A record with an unsupported `$v` is
`SchemaError::UnsupportedVersion`; the loader keeps such a line verbatim (doc 05 §10.2).

## How to add a member

1. **Append** it after the existing members of its struct (before `extra`; in a device, before the
   aspects). Never insert, reorder, rename or change the type, meaning or unit of an existing member
   within a format major (doc 05 §10.3).
2. Make it **optional** with a default and `skip_serializing_if`, so files without it stay canonical.
3. Record its `since = "<format minor>"` in the doc comment and in the format change log; it feeds
   `min_reader` (ADR-0011 amendment 1).
4. Regenerate the schemas: `EW_SCHEMA_BLESS=1 cargo test -p ew-schema --test schemas`.
5. Add or extend a canonical line in `tests/common/lines.rs` and the fixture of the format.
6. Update the field table of doc 03 (or the owning feature document) in the same change.

A new enum variant is additive in the same way: append it before `Unknown`. A variant with an object
payload gets a payload struct with `extra`, never an inline struct variant. A list whose order carries no
meaning is a `Set` or a `KeyedList`, never a `Vec`.

## Format change log

Format 0.1 is unreleased; these entries record what changed since the first golden subset (WP-S1.02).

| Change | Since | Notes |
|---|---|---|
| `MasterData.document_title: MlText` appended after `edited` | 0.1 | The cover subtitle (doc 03 §5, doc 20 §4.1) |
| `ReportBlock.first_folio` optional | 0.1 | Required for page-oriented blocks, ignored when `embedded` is set (doc 09 §2.2, ID-16) |
| `OrderUnit::Metre` written `"m"` (was `"metre"`) | 0.1 | One spelling with `Unit::M` (doc 05 §3.2) |
| `LineageAnchor::Copy.of` holds the original lineage's **namespace** | 0.1 | As `ew_ids::v5::LineageAnchor::Copy` and ADR-0004 amendment 4 |
| Object payloads of enum variants carry `extra` | 0.1 | No change on the wire |
| Sets and keyed lists are enforced on read | 0.1 | Files that were unsorted were never canonical |

Known limitation: device aspects that v0 does not implement (`assembly`, `plc_station`, `plc_module`,
`bus_node`, `valve_terminal`) are kept in `extra` and therefore written in byte-wise key order after
`cable`, not in doc 05 §3.1's aspect order (`plc_module` would precede `plc_station`). The order becomes
doc 05's as each aspect is implemented; until then an untouched record keeps its original line.

## How to add a kind

1. Take the kind's row from the registry in `src/kind.rs`; its rank is fixed and never reused. A kind
   that is not in the registry gets a **new** rank and an ownership rule, recorded in doc 05 §2.2 first.
2. Write the struct (`id` first, then `$v`, then the members, `extra` last), add the `Record` or
   `LibDocument` variant, switch the registry row to `Implemented` and add the kind to
   `SCHEMA_VERSIONS` with version 1.
3. Add its schema to `kind_schema`, regenerate the schemas, and add golden lines and anchor cases.
4. List personal-data members in `PERSONAL_DATA_FIELDS` (NFR-PRIV-06).

## When `$v` may change

A `$v` bump breaks its kind, so it happens **only in a format major** (after 1.0: only in the yearly
window, FR-COL-011). It changes one entry of `SCHEMA_VERSIONS` and ships a migration step
`fn(serde_json::Value) -> Result<Value>` that is never deleted (ADR-0011 decision 3, NFR-REL-05).
Within a major, kinds change only by additions.

## Pre-merge checklist (schema-steward rule, ARCH §7.6, ADR-0011 decision 6)

A pull request that changes the format ships, in the same change:

- [ ] the struct change following the rules above (append-only, optional, `since`);
- [ ] a migration step if a `$v` changes (format major only), never deleting an older step;
- [ ] the regenerated schemas under `schemas/<format>/` (the `schemas` test passes without bless);
- [ ] a fixture: a canonical line in `tests/common/lines.rs`, and the format fixture corpus
      (`fixtures/format/<version>/`, once it exists) opened and saved unchanged;
- [ ] documentation: the field table of doc 03 or the owning feature document, and doc 05 if the
      registry, an encoding or an ownership rule changes;
- [ ] `cargo xtask ci` green, including `check-licence-map` over `schemas/`;
- [ ] approval by the quarter's schema steward, who never approves their own format change.
