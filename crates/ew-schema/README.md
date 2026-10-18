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
  unknown member survives a load and a save at any depth.
- **Every persisted enum has `Unknown(Value)`** as its last variant (`#[serde(untagged)]`,
  `#[schemars(skip)]`, `deserialize_with = "de_unknown"`). Foreign enums get one through `Open<T>`.
- **Enums** are snake_case unit variants or one-member objects for data variants.
- **No floats anywhere**; integers within ±(2⁵³ − 1). Typed members enforce it, `check_value` covers
  unknown content.
- **`BTreeMap`, never `HashMap`.** Sets and keyed lists are stored sorted (doc 05 §3.2).
- **Strings are NFC.** `MlText` and `Ident` enforce it; plain `String` members are documented as NFC and
  the writer normalises them.
- Field types from the foundation crates name a schema stand-in with `#[schemars(with = "…")]`
  (`src/proxy.rs`).

## How to add a member

1. **Append** it after the existing members of its struct (before `extra`; in a device, before the
   aspects). Never insert, reorder, rename or change the type, meaning or unit of an existing member
   within a format major (doc 05 §10.3).
2. Make it **optional** with a default and `skip_serializing_if`, so files without it stay canonical.
3. Record its `since = "<format minor>"` in the doc comment and in the format change log; it feeds
   `min_reader` (ADR-0011 amendment 1).
4. Regenerate the schemas: `EW_SCHEMA_BLESS=1 cargo test -p ew-schema --test schemas`.
5. Add or extend a canonical line in `tests/records.rs` and the fixture of the format.
6. Update the field table of doc 03 (or the owning feature document) in the same change.

A new enum variant is additive in the same way: append it before `Unknown`.

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
- [ ] a fixture: a canonical line in `tests/records.rs`, and the format fixture corpus
      (`fixtures/format/<version>/`, once it exists) opened and saved unchanged;
- [ ] documentation: the field table of doc 03 or the owning feature document, and doc 05 if the
      registry, an encoding or an ownership rule changes;
- [ ] `cargo xtask ci` green, including `check-licence-map` over `schemas/`;
- [ ] approval by the quarter's schema steward, who never approves their own format change.
