# ADR-0001: Core model: typed Rust records in persistent maps; everything printed is derived

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

The worst defects of the reference drawing set share one root cause: **a value that should have been computed was
stored as a copy, and the copy went stale** ([reference analysis](../research/reference-drawing.md) §3):

| Defect | What went stale |
|---|---|
| D-01 | The part list (folio 901) still shows -4W1 / -4X1 after folio 3 was renumbered to -3W1 / -3X1 |
| D-02 | The cabinet layout (folio 902) shows T51 while the schematic has T55; layout rectangles are not bound to devices |
| D-05 | The revision field has four formats: "1", "V1", "V1.0", "V2.0" |
| D-06 | Folio 101's title-block description differs from the project description |
| D-08 | Wire sizes and colours rely on the free-text rule "if not indicated, standard size" |
| D-10 | The overview on folio 202 shows 5 valves; the detail page 206 and the BOM show 4 |

EW must also serve many concurrent readers of **one consistent state**: the canvas inside a 16 ms frame
(NFR-PERF-04), a 500-page PDF export while the user keeps editing (NFR-PERF-11), previews of bulk commands, offline
checks, the SQL projection (ADR-0017) and the save writer thread. The writers are just as varied: GUI tools, the
CLI, rhai, importers, WASM plugins and MCP agents.

Four substrates were proposed and scored by a judge panel on feasibility, domain fit and longevity
([architecture](../design/04-architecture.md) §17):

| Proposal | Substrate | Score |
|---|---|---|
| A | Typed records in persistent maps; derived queries | 22 |
| B | bevy_ecs | 16.5 |
| C | SQLite as the live model | 22 |
| D | Retained object graph with observers | 17.5 |

A and C tied. A won on the grounds that C's strengths can be grafted onto A as derived artefacts, while A's
(freshness by construction, O(1) snapshots, plain-struct editing) cannot be grafted onto C.

## Decision

1. **Single source of truth.** A project in memory is one `Model` value made of typed record tables:
   `Table<T> { rows: imbl::OrdMap<Id<T>, Row<T>>, changed_at: Rev }` with `Row<T> { value: Arc<T>, changed_at: Rev }`.
   Cloning a `Model` is O(1). The record structs are the persisted `ew-schema` types (ADR-0024); `ew-model` adds
   behaviour (ops, indexes, invariants) but never a second data definition.
2. **Store intent, derive presentation.** Only facts a human decided are stored. Every printed value is a query
   result over the model (ADR-0002): device tags, cross-reference texts, nets and potentials, effective wire
   properties, title-block values, TOC and Prev/Next, BOM rows, report pages, layout labels, check findings and
   display lists. **Derived data is never persisted**, neither in the model nor in files (ADR-0010).
3. **One mutation path.** The model changes only through transactions of typed ops produced by commands
   (ADR-0008). `ew-model` tables are `pub(crate)`; other crates read through the tracked query context `Q` and write
   through `TxBuilder`.
4. **Typed, not property bags.** There is one Rust struct per entity kind. Multi-role devices carry typed aspects
   (ADR-0006). User-defined properties and unknown fields live in `extra` maps. There is no entity-attribute-value
   model for built-in data.
5. **Base indexes** (reverse-FK maps such as `functions_by_device`, `reps_by_page`, `connections_by_endpoint`,
   `terminals_by_strip`, `placements_by_device`) are maintained by `Op::apply` by delta, rebuilt on load and never
   saved.
6. **Deliberate exceptions are authored data, not caches.** Frozen tags, frozen wire numbers, manual PLC addresses
   and committed routes are stored on purpose and checked against their rule (ADR-0003).

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Raw reads behind a policy-checked feature** (2026-10-18, core team;
   [architecture](../design/04-architecture.md) §3 rule 1; ADR-0002 amendment 2). Rust cannot restrict an item's
   visibility to chosen dependent crates, so decision 3 is implemented as follows. The table fields stay
   `pub(crate)`. The raw read API that the layers directly above the model need (`Model::table`, `Model::lib`,
   `Model::indexes`, `Model::row`, `Model::delete_plan` and the getters of `Table` and `BaseIndexes`) is compiled
   only with the cargo feature **`raw-read`** of `ew-model`. `delete_plan` is gated because the `Op`s it returns
   carry row contents; `Model::apply` and `Model::apply_with` stay public. Only `ew-derive` (which builds the tracked context `Q` on it),
   `ew-commands` (`TxBuilder`, normalisers, validators) and `ew-store` (save, external-change diff) may enable it,
   plus `ew-model`'s own tests. `cargo xtask check-features` enforces the list through a `[[banned_features]]`
   entry of `ci/dep-rules.toml` with `allowed_packages`. Every other crate reads through `Q` (queries) or
   `TxBuilder` (commands), as decision 3 says. Cargo features are additive, so a crate that depends on `ew-derive`
   is compiled with the raw API present: the feature check stops a crate from requesting it, and ADR-0002
   amendment 2 says what covers the rest. A source-level guard for that remainder (for example a per-crate clippy
   `disallowed-methods` list) is open ([decision log](../plan/06-decision-log.md) §5.1, v0.5).
2. **A source-level guard: `cargo xtask check-raw-read`** (2026-10-18, core team;
   [architecture](../design/04-architecture.md) §3 rule 1; decision log ID-50). The remainder that amendment 1 left
   open is closed by a source check rather than per-crate clippy configuration, which every new crate would have to
   remember. `cargo xtask check-raw-read` fails when the Rust source of any workspace package other than `ew-model`,
   `ew-derive`, `ew-commands` and `ew-store` invokes `raw_read!`, names the `raw-read` feature, or calls an item
   that `ew-model` declares only under that feature. Test code is exempt, as the feature rule's dev exemption is:
   the `tests/` and `benches/` directories and `#[cfg(test)]` items. The check takes the gated item names from
   `ew-model`'s own source, so a newly gated reader is covered without a list to maintain; a false positive gets an
   allowlist entry with a reason in `ci/dep-rules.toml`, never an inline suppression. It runs in `cargo xtask ci`
   beside `check-features` (CI-PR-09).
3. **No row-carrying types above the transaction layer** (2026-10-18, core team, after the review of WP-S1.04;
   [architecture](../design/04-architecture.md) §3 rule 1; decision log ID-59). A full `Op` carries the before and
   after rows, so handing ops, diffs or history entries to a front-end would be an untracked read by another route.
   The normal public surface of `ew-commands` therefore carries no row: diffs, preview ops and history entries are
   summaries (op kind, record kind, key and the changed field paths). Full ops are reachable only under its
   dev-only `testkit` feature, which `ci/dep-rules.toml` bans outside dev edges.

## Consequences

### Positive

- A stale derived copy cannot exist. D-01, D-02, D-05, D-06, D-08 and D-10 become impossible by construction
  (class S in [architecture](../design/04-architecture.md) §14).
- Background export, previews, offline checks, the SQL projection and saving run on immutable snapshots without
  locks.
- Editing is synchronous and in memory on plain Rust structs. imbl mutates uniquely owned nodes in place, so a
  10,000-op transaction copies each touched B-tree node once.
- Ordered maps give deterministic iteration and ID range scans, the basis of byte-identical output (NFR-REL-07).
- Third parties read and write the same record types through the permissive crates (FR-COL-012).

### Negative

- A new failure mode replaces the old one: a **stale memo**. ADR-0002 guards it (untracked reads do not compile;
  oracle mode in CI).
- Every printed string costs a query. Interactive latency at L/XL depends on memoisation and on the performance spike (gate G-SP). Global
  connectivity is estimated at 5–10 ms per committed wire edit in the XL-A estimate case
  ([architecture](../design/04-architecture.md) §6.7).
- Overrides must be modelled explicitly (an override field, its provenance and "reset to derived"), never by writing
  into a derived value. Each feature needs more schema thought up front.
- Model and indexes take about 250 MB in the XL-A estimate case (2,000 pages, 50k devices, 200k connections;
  [non-functional requirements](../design/02c-nonfunctional-requirements.md) §2.1), inside the 4 GB budget of
  NFR-PERF-13.
- EW depends on imbl 7.0.2 (MPL-2.0+). `Table<T>` encapsulates it, so the map implementation can be replaced
  without touching callers.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **C: SQLite as the live model** | Three representations of the truth (text at rest, the working database, Rust mirrors for the canvas). An asynchronous commit actor makes every tool reason about uncommitted state. SQLite `UNIQUE` and `CHECK` constraints cannot be deferred, so QET projects containing D-11 and D-12 could not even be staged. Its strengths survive as the read-only projection (ADR-0017), JSONL storage (ADR-0010) and the invariant catalogue (ADR-0009) |
| **B: bevy_ecs** | The core would depend on bevy_ecs APIs that are churning (observers, lifecycle events, relationships). A `World` cannot be snapshotted, so previews would be modal and checks chunked on the main thread. Dual Entity/Uid identity |
| **D: retained object graph with observers** | Freshness rests on discipline: one missed subscription is the D-01 class again |
| **CRDT document** (Loro, Automerge) | Merges into well-formed but semantically invalid states (duplicate tags, dangling terminals); binary logs are opaque to git |
| **Stored derived values with an "update" step** (QET and EPLAN report generation) | The mechanism behind D-01, D-04 and D-10 |

## Verification

- Debug builds re-derive the base indexes after every transaction and compare them with the maintained ones. A
  generated field-mask test fails when a link field is added without an index declaration.
- CI enforces the access rules of [architecture](../design/04-architecture.md) §3 (NFR-MAINT-01).
- `load(save(m)) = m`, byte-stable (NFR-REL-04).
- Defect regression tests D-01 … D-42 run on the golden project ([golden project](../design/20-golden-project.md)
  §19).

## Related

- **ADRs:** [ADR-0002](0002-derived-data-engine-ew-memo.md), [ADR-0003](0003-freshness-contract.md),
  [ADR-0004](0004-identity-uuidv7-uuidv5.md), [ADR-0006](0006-function-device-representation-model.md),
  [ADR-0008](0008-transactions-undo-preview.md), [ADR-0009](0009-invariant-catalogue.md),
  [ADR-0010](0010-project-storage-jsonl-directory.md), [ADR-0017](0017-read-only-sql-projection-api-v1.md), [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md).
- **Design:** [architecture](../design/04-architecture.md) §0, §1, §4, §14, §17;
  [domain model](../design/03-domain-model.md) §2, §4.
- **Requirements:** FR-NUM-001, FR-XRF-001, FR-XRF-020, FR-CON-009, FR-CON-017, FR-PAG-013, FR-REP-001,
  FR-REP-035, FR-REP-036, FR-LAY-009, FR-COL-012, NFR-REL-04, NFR-REL-07, NFR-REL-08, NFR-MAINT-01.
