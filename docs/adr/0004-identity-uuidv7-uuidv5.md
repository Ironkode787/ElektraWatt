# ADR-0004: Identity: UUIDv7 for authored records, UUIDv5 for instantiated, imported and generated ones

**Status:** Accepted · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

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
| Imported records | **`v5(namespace, source_file_hash, source_id)`** | With a `source_ref` that keeps provenance (QET element UUID, EDZ part number, DXF handle) |

- **Interning.** Within a session, IDs are interned to dense `u32` values for union-find arrays and other hot
  structures. Interned values are never persisted or exported.
- **Encoding.** Files carry the canonical lower-case hyphenated UUID string.

## Consequences

### Positive

- Renumbering cannot orphan a reference: the BOM, the layout and the xrefs hold IDs and ask `device_tag` for text.
- Re-importing the same QET file yields the same IDs, so the import preview shows an empty diff and git diffs stay
  small.
- PDF links and named destinations are stable across regenerations, including on generated report pages.
- Files hold no CP rows unless a CP is overridden; at XL this avoids about 500k rows.
- Records created on two branches never collide, which the merge driver relies on (ADR-0012).

### Negative

- UUID strings make files larger than integer keys would. Accepted for merge safety.
- **UUIDv7's leading bytes are time.** All v7 IDs share their first byte until 2039, so sharding and line order must
  use the random bits (bytes 8–15: the 62-bit `rand_b` after the two variant bits, ADR-0010). `ew-ids` must therefore guarantee random tail bits even for IDs minted
  within one millisecond.
- **CP identity is tied to `idx`.** A function definition must not reorder or remove CP indexes within a version.
  "Update from library" must remap overrides and connection endpoints by designation in a previewed command
  (FR-PRT-014).
- **`source_file_hash` is per file version.** Re-importing an *edited* source file would mint new IDs for unchanged
  items; see the pending amendment below.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Tags as keys** | Renumbering breaks every reference (D-01) |
| **Database rowids or per-project integer counters** | Collide across branches; meaningless outside one file |
| **Seeded RNG for imported IDs** | Less robust than content-derived v5: any change in import order or importer version reshuffles IDs |
| **Dual Entity/Uid identity** (proposal B) | Two identities to keep in sync for every reference |
| **Storing CP rows** | About 500k extra rows at XL, most of them carrying no authored data |
| **ULID** (ulid 3.0.0) | Equivalent ordering properties, but outside RFC 9562 and the uuid crate already used for v5 |

## Pending amendments

These refinements were raised by later design documents. They do not change the decision above; each needs a
revision of this ADR once accepted.

1. **Import lineage** ([interop](../design/14-interop.md) §2.3, open question 1). The namespace is anchored on an
   `ImportLineage`: the QET project UUID (QET 0.200+) or the SHA-256 of the lineage's *first* imported file, and
   records are `v5(namespace, source_key)`. With the first-file anchor the formula is literally this ADR's, with
   `source_file_hash` read as "hash of the lineage's first file". Re-importing an edited file then keeps the IDs of
   unchanged source keys. *Recommendation:* accept and reword FR-IMP-002 accordingly.
2. **Functions created from a part's function template** get `v5(device_id, template_index)`
   ([domain model](../design/03-domain-model.md) §4.1). This is consistent with the "instantiated → v5" rule.
   *Recommendation:* add it to the table above.
3. **UUIDv7 method.** Line order and sharding need all 62 bits of `rand_b` to be fresh random bits for every ID.
   RFC 9562 §6.2 method 3 (sub-millisecond clock bits in `rand_a`, `rand_b` random) satisfies this. A counter that
   reaches into `rand_b` (method 1 with a long counter, or method 2's monotonic increment) does not, because IDs
   minted in one burst would then sort next to each other. The uuid crate's default v7 context appears to use such
   a counter (**verify**); if so, `ew-ids` builds IDs from the timestamp and its own random bytes instead.
   *Recommendation:* specify this in `ew-ids`, with a CI test that 10,000 IDs generated within one millisecond spread evenly over shards and sort
   positions ([project format](../design/05-project-format-and-persistence.md), open question 12).

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0006](0006-function-device-representation-model.md),
  [ADR-0007](0007-explicit-connections-and-autoconnect.md), [ADR-0010](0010-project-storage-jsonl-directory.md),
  [ADR-0012](0012-git-integration.md), [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0021](0021-importers-emit-format-level-records.md).
- **Design:** [architecture](../design/04-architecture.md) §4.4; [domain model](../design/03-domain-model.md) §4.1;
  [project format](../design/05-project-format-and-persistence.md) §3; [interop](../design/14-interop.md) §2.3.
- **Requirements:** FR-DEV-002, FR-DEV-006, FR-IMP-002, FR-OUT-007, FR-COL-018, FR-PRJ-021, FR-PRT-014,
  FR-NUM-028, NFR-REL-07.
