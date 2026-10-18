# ADR-0004: Identity: UUIDv7 for authored records, UUIDv5 for instantiated, imported and generated ones

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

- **Tags cannot be identity.** Tags, folios, terminal designations and wire numbers change on renumbering. QET
  stores the evaluated label as element information, and the part list kept the stale copy -4W1 after folio 3 was
  renumbered to -3W1 (D-01). Anything keyed or matched by tag breaks the same way.
- **Several consumers need stable identity across regenerations and edits:**
  - PDF named destinations and link annotations, so that a link to "102-A1" survives a re-export (FR-OUT-007);
  - review comments anchored to entities, surviving renumbering and moves (FR-COL-018);
  - wire IDs for numbering, ferrules and machine export (ADR-0007);
  - typed external references to other projects, e.g. the J-E-206 / JE-N1000-03 connection (FR-PRJ-021).
- **Re-imports must update in place.** Importing the same QET file again must produce the same records, so diffs
  and merges stay meaningful (FR-IMP-002).
- **No coordination.** Two engineers on two git branches must be able to create records without ID collisions.
- **Size.** The XL-A estimate case instantiates about 500k connection points, and the XL class bound is 625k
  ([non-functional requirements](../design/02c-nonfunctional-requirements.md) §2.1). Persisting a row per CP would
  dominate the files.
- **Storage depends on ID bits.** JSONL line order and device sharding use parts of the ID (ADR-0010).

## Decision

Every reference in the model is a typed `Id<T>` (crate `ew-ids`, permissive). Tags and folios are properties.

| Object | ID | Notes |
|---|---|---|
| New authored records | **UUIDv7** (uuid 1.28.0) | Time-ordered leading bytes, random tail; minted locally |
| Instantiated connection points | **`v5(function_id, idx)`** | Computed, never stored. Only `CpOverride` rows are persisted. `idx` is the CP's index in the function definition, which also keeps duplicate printed designations distinct (FR-DEV-006) |
| Generated report pages | **`v5(report_block_id, page_index)`** | PDF named destinations on generated report folios (2, 901 and 903–911 in the golden project) stay stable |
| Imported records | **`v5(namespace, source_file_hash, source_id)`** | With a `source_ref` that keeps provenance (QET element UUID, EDZ part number, DXF handle). The namespace is anchored on an import lineage (amendment 1) |
| Functions created from a part's function template | **`v5(device_id, template_index)`** | Amendment 2. Example: the 13-14 signal contact of -102F2, created from the MICO 4.10 template |
| PLC channels | **`v5(module_id, "channel", number)`** | Amendment 2. Stored, because a channel carries authored data; two branches that add the same module mint the same channel IDs |
| Embedded library copies | **`v5(NS_LIBCOPY, library_id ‖ item_id)`** | Amendment 2. One copy per catalogue item and project. Only an explicit "keep both" after a cross-project paste of another version mints `v5(NS_LIBCOPY, library_id ‖ item_id ‖ version)` for the second copy |

- **Interning.** Within a session, IDs are interned to dense `u32` values for union-find arrays and other hot
  structures. Interned values are never persisted or exported.
- **Encoding.** Files carry the canonical lower-case hyphenated UUID string.

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Import lineage** (2026-10-18; [interop](../design/14-interop.md) §2.3). The namespace of imported records is
   anchored on an `ImportLineage`: the QET project UUID (QET 0.200+), or the SHA-256 of the lineage's *first*
   imported file. Records are `v5(namespace, source_key)` and carry `source_ref{lineage, key}`. With the first-file
   anchor the formula is literally this ADR's, with `source_file_hash` read as "hash of the lineage's first file".
   Re-importing an edited file therefore keeps the IDs of unchanged source keys (FR-IMP-002, FR-IMP-040).
2. **Further v5 identities** (2026-10-18; [domain model](../design/03-domain-model.md) §4.1,
   [PLC and bus](../design/10-plc-and-bus.md) §2.4,
   [symbols, parts and macros](../design/07-symbols-parts-macros.md) §2.1). The identity table above gains three
   rows, all following the "instantiated → v5" rule: functions created from a part's function template, PLC
   channels, and embedded library copies. The copy ID is item-based, so "update from library" rewrites one `lib/`
   file and leaves every referencing record untouched, and two branches that embed the same version write
   identical files.
3. **UUIDv7 method** (2026-10-18; [project format](../design/05-project-format-and-persistence.md) §3.3). `ew-ids`
   fills `rand_a` with sub-millisecond clock bits and `rand_b` with 62 fresh random bits for every ID (RFC 9562
   §6.2 method 3). It never puts a counter in `rand_b`, because line order and sharding use bytes 8–15. If the uuid
   crate's default v7 context keeps a counter there (**verify**), `ew-ids` assembles the bits itself through uuid's
   builder API. A CI test checks that 10,000 IDs minted within one millisecond spread evenly over shards and sort
   positions.

## Consequences

### Positive

- Renumbering cannot orphan a reference: the BOM, the layout and the xrefs hold IDs and ask `device_tag` for text.
- Re-importing the same QET file yields the same IDs, so the import preview shows an empty diff and git diffs stay
  small.
- PDF links and named destinations are stable across regenerations, including on generated report pages.
- Files hold no CP rows unless a CP is overridden; in the XL-A estimate case this avoids about 500k rows.
- Records created on two branches never collide, which the merge driver relies on (ADR-0012).

### Negative

- UUID strings make files larger than integer keys would. Accepted for merge safety.
- **UUIDv7's leading bytes are time.** All v7 IDs share their first byte until 2039, so sharding and line order must
  use the random bits (bytes 8–15: the 62-bit `rand_b` after the two variant bits, ADR-0010). `ew-ids` must therefore guarantee random tail bits even for IDs minted
  within one millisecond.
- **CP identity is tied to `idx`.** A function definition must not reorder or remove CP indexes within a version.
  "Update from library" must remap overrides and connection endpoints by designation in a previewed command
  (FR-PRT-014).
- **`source_file_hash` is per file version.** Read literally, re-importing an *edited* source file would mint new
  IDs for unchanged items; amendment 1 anchors the namespace on the import lineage instead.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Tags as keys** | Renumbering breaks every reference (D-01) |
| **Database rowids or per-project integer counters** | Collide across branches; meaningless outside one file |
| **Seeded RNG for imported IDs** | Less robust than content-derived v5: any change in import order or importer version reshuffles IDs |
| **Dual Entity/Uid identity** (proposal B) | Two identities to keep in sync for every reference |
| **Storing CP rows** | About 500k extra rows in the XL-A estimate case, most of them carrying no authored data |
| **ULID** (ulid 3.0.0) | Equivalent ordering properties, but outside RFC 9562 and the uuid crate already used for v5 |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0006](0006-function-device-representation-model.md),
  [ADR-0007](0007-explicit-connections-and-autoconnect.md), [ADR-0010](0010-project-storage-jsonl-directory.md),
  [ADR-0012](0012-git-integration.md), [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0021](0021-importers-emit-format-level-records.md).
- **Design:** [architecture](../design/04-architecture.md) §4.4; [domain model](../design/03-domain-model.md) §4.1;
  [project format](../design/05-project-format-and-persistence.md) §3; [interop](../design/14-interop.md) §2.3;
  [PLC and bus](../design/10-plc-and-bus.md) §2.4; [symbols, parts and macros](../design/07-symbols-parts-macros.md)
  §2.1.
- **Requirements:** FR-DEV-002, FR-DEV-006, FR-IMP-002, FR-IMP-040, FR-OUT-007, FR-COL-018, FR-PRJ-021, FR-PRT-014,
  FR-NUM-028, FR-PLC-053, NFR-REL-07.
