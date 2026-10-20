# ADR-0002: Derived-data engine: cold-first queries plus a cut-down memo layer (`ew-memo`); no salsa

**Status:** Accepted · amended 2026-10-18, 2026-10-19, 2026-10-20 (G-SP outcome: Recycle; the memo engine as built) · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

## Context

ADR-0001 makes every printed value a query over the model. The XL size class reaches 5,000 pages, 50k devices and
250k connections; its estimate case XL-A has 2,000 pages, 50k devices, about 150k functions, 200k connections and
500k instantiated connection points ([non-functional requirements](../design/02c-nonfunctional-requirements.md)
§2.1). Recomputing everything per frame is impossible there, while the budgets are tight:

| Budget | Requirement |
|---|---|
| Frame ≤ 16 ms, of which derived updates of visible views ≤ 5 ms | NFR-PERF-04 |
| Commit of a move or wire gesture p99: S/M ≤ 5 ms, L ≤ 10 ms, XL ≤ 20 ms | NFR-PERF-05 |
| A change touching many derived values (profile colour of "24 V DC") visible within ≤ 100 ms at XL | NFR-PERF-07 |
| Memo memory ≤ 300 MB at XL | NFR-PERF-13 |

Three engine options were on the table:
- **Proposal A as written:** a full hand-rolled incremental engine (about 4 kLOC) with parallel fork evaluation,
  sharded locks and memo merge-back. The judges called it the largest novel infrastructure on the critical path,
  with a bus factor of one.
- **salsa 0.28.5:** a write cancels or blocks in-flight readers. EW must export 500 pages from a snapshot while the
  user edits (NFR-PERF-11). Salsa inputs would also duplicate the imbl tables, and the crate is still 0.x.
- **Hand-declared invalidation per service** (proposal D): freshness by discipline.

Stale memos would recreate the D-01 class invisibly. This is risk 1 of
[architecture](../design/04-architecture.md) §16.

## Decision

1. **Query shape.** A derived value is a pure function `fn(q: &Q, key) -> Arc<V>`. `Q` exposes only tracked
   accessors (`q.devices().get(id)`, `q.idx().reps_by_page(p)`, `q.get::<PageOrder>(())`). Model tables are
   `pub(crate)` in `ew-model`, so **an untracked read inside a query does not compile**. rhai formulas inside
   queries read through the same accessors.
2. **Phase 1 (spine): cold.** Queries are computed on demand, with a request-scoped cache that prevents repeated
   work within one top-level evaluation. No engine is needed, and contributors write plain functions.
3. **Phase 2 (from the performance spike, gate G-SP of the plan, right after the core model WP WP-S1.03): memoise where measured.** A query is promoted to `Memo` when profiling
   on the synthetic L/XL projects shows it on an interactive path. Promotion and demotion are one-line policy
   changes.
4. **`ew-memo` v1 contains exactly:**
   - revision stamps (`changed_at: Rev`) on every row, index key and table, and memos
     `{value: Arc<V>, deps: SmallVec<(DepKey, observed)>, verified_at, changed_at}`;
   - red-green verification and **early cut-off**: an unchanged result (`Eq`, or a 128-bit hash for bulky values)
     is backdated, so dependents stay green;
   - three durability levels (library, settings, drawing);
   - a byte-budget LRU for bulky memos (display lists, pagination);
   - cycle detection (panic in debug builds; the TOC page-count fixpoint iterates inside one query, at most three
     rounds);
   - statistics per query and per commit (hits, verifications, re-executions, cut-offs), emitted through
     `tracing`.
5. **Out of v1:** parallel evaluation of the main store, sharded locks, memo merge-back from forks and salsa-style
   reader cancellation. The single exception is **adopt-if-unchanged**: when a job finishes and the main memo store
   is still pointer-equal to the job's base at the same revision, the session adopts the job's store in O(1) (warm-up
   after open, idle full checks).
6. **Granularity rules.**
   - No per-CP memos. The finest key is a page, device, potential, strip, module, station, report block or layout
     space.
   - Per-connection values are grouped per potential: `wire_props_of(potential) -> Arc<Map>`, with `wire_props(conn)`
     a cold lookup into it.
   - Memoise only when the cold cost exceeds about 20 µs or the query is read every frame.
   - CI gates memo count and bytes per query family on the XL fixture.
7. **Fallback.** A query family that cannot meet its budget within the memory limits falls back to page-scoped dirty
   flags with coarse recomputation, still under the oracle.
8. **Ownership.** `ew-memo` is a generic, permissive crate that knows nothing about ECAD. It has two named owners by
   M1 (NFR-MAINT-07) and is not published to crates.io before that.

Initial policy for representative queries ([architecture](../design/04-architecture.md) §6.4):

| Query | Initial policy |
|---|---|
| `page_order()`, `device_tag(dev)`, `xref_text(rep)`, `potential_partition()`, `bom_rows(mode)` | Memo |
| `function_cps(f)`, `rep_anchor_cell(rep)` | Cold |
| `page_display_list(page, layer)` | Memo + LRU |
| `sql_table(api_table)` | Class II job (ADR-0003, ADR-0017) |

## Amendments

Accepted on 2026-10-18. The decision text above is unchanged; these entries refine it.

1. **Reverse dependencies per query family** (2026-10-18;
   [checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md) §3.3, open question 18;
   [architecture](../design/04-architecture.md) §6.2). `ew-memo` v1 also offers a generic API that returns, per query
   family, the memos whose recorded reads include a given key. It stays ECAD-agnostic. The online check engine
   builds its `rdeps` index from it, so a commit re-evaluates exactly the check scopes whose last evaluation read a
   touched key. The performance spike (G-SP) measures its memory (≤ 40 MB estimated at XL); above budget the engine falls back to
   coarse page and shard keys with re-verification of the matching scopes.
2. **"Does not compile" in practice** (2026-10-18, core team; ADR-0001 amendment 1;
   [architecture](../design/04-architecture.md) §3 rule 1). Rust cannot limit visibility to chosen crates, so
   decision 1's guarantee is carried by two facts: the raw read API of `ew-model` exists only with its cargo feature
   `raw-read`, which only `ew-derive`, `ew-commands` and `ew-store` may enable (`cargo xtask check-features`); and a
   query receives only `&Q`, which exposes tracked accessors and never the `Model` itself. A crate outside the
   dependency cone of those three cannot name the raw API at all. Because cargo features are additive, crates that
   depend on `ew-derive` compile with the raw API present; for them the guarantee rests on `Q` being the only handle
   a query gets, on review, and on oracle mode.
   Since cycle 6 the source guard `cargo xtask check-raw-read` also covers them (ADR-0001 amendment 2).
3. **`ew-memo` v1 as built** (2026-10-19, core team; decision log ID-81 to ID-83, ID-95, ID-96;
   [architecture](../design/04-architecture.md) §6.2). Decision 4 is implemented with these precisions:
   - **Durability contract.** `advance(rev, d)` records `rev` as the last change of level `d` and of every less
     durable level. `d` must cover the durability of every changed input **both before and after** the change: an
     input moved from library to drawing durability changes a library-level dependency, and the naive rule (the
     level after the change) let a library-durable memo skip its verification wrongly, which the property test
     found. Conversely, every verification that finds a memo green recomputes its durability as the minimum of its
     dependencies' **current** durabilities, so a memo whose input became less durable cannot keep skipping on the
     old level (a stale-memo path found in the review; task FT-33).
   - **Cut-off and eviction.** A cut-off of a held `Eq` value keeps the old `Arc`. An LRU-evicted memo keeps its dependencies and its
     fingerprint, so its dependents can still be verified without recomputing it; when an evicted `Eq` value is
     recomputed because it is read again and its dependencies are unchanged, the new value is treated as unchanged
     (the query is pure), so `changed_at` stays.
   - **Cycles.** A cycle panics in debug builds and is an error (`Cycle`) otherwise; the error leaves the previous
     memo in place.
   - **Oracle.** Oracle mode re-runs memoised reads cold at every depth, not only at the top-level read, and is
     sampled deterministically (`Oracle::OneIn(n)`); mismatches are collected for the caller (`take_mismatches`).
   - **Statistics** are `tracing` events (target `ew_memo::stats`) emitted on `advance`, which also resets the
     commit window.
   - **Forks.** `fork()` is O(1) (`imbl`), and `adopt_if_unchanged` is the decision-5 exception and nothing more.
     "Unchanged" is exact: a fork is adopted only when the store's mutation token still equals the fork's base
     token, and the store's revision and the fork's revision both equal the base revision, so a fork that advanced
     on its own is never adopted (task FT-33).
   - **Determinism.** No hash-map iteration, floating point or clock.
   `ew-derive`'s `Q` does not use `ew-memo` yet: phase 1 runs every family cold behind a no-op `Tracker`, so that
   promoting a family after the spike (decision 3) changes its `POLICY` and the tracker, never the query body.
4. **G-SP outcome: Recycle; incremental connectivity replaces the coarse fallback** (2026-10-20, project owner
   (delegated); decision log ID-218 to ID-226; spike report
   [`spike-g-sp-2026-10.md`](../research/spike-g-sp-2026-10.md); [gates](../plan/03-gates.md) §5.1;
   [architecture](../design/04-architecture.md) §6.1 to §6.4). This closes PD-8 and is the amendment G-SP-02 asks
   for.
   - **Outcome.** Recycle: one re-spike of at most four weeks after the reshaped queries below exist. On S, M and L
     evidence the cold global connectivity misses the commit budget of NFR-PERF-05 by 2.6–3× at S, 25–27× at M and
     66–97× at L, and memoising `potential_partition` as one opaque unit is worse than cold (339 ms against 135 ms
     at M), because every drawing edit re-executes it. The folio renumber (NFR-PERF-07) and the open-time proxy
     (NFR-PERF-03) are within budget cold at S, M and L, and so is the affected-only part of the profile change. The causes are of the kind a recycle addresses, and all four are confirmed in the code by the review:
     whole-table reads in `stitch_graph` and `potential_partition`; potential names computed inside the topology
     query; a super-linear wire-property chain whose cause is not yet isolated; and pull verification that is
     O(dependencies), hits included. The XL run (report appendix A.4; medians decide, because concurrent builds
     inflated its tails) confirms the direction: the move commit p99 is 9.24 s (462×, median 2.85 s), the wire
     commit 6.17 s (308×); the folio renumber with page order and tags takes 6.09 ms (1.2×), so at XL it needs the
     planned memos of `page_order` and `device_tag`; the affected-only profile change takes 209 ms (2.1×), a lower
     bound already over the budget, so the wire chain must get cheaper as well; the memo prototype stays over budget
     (partition 7.6× after an unrelated edit, 139–142× after a move or wire; per-page local nets 4.4× with 4,999
     verifications); the open-time proxy passes (2.49 s of 20 s). The oracle found no mismatch (420 comparisons at
     XL). **KT-3 is not triggered**: it applies after the fallback prototype and one recycle, and the XL open time
     is far below its 40 s line. Exit criteria at this outcome: G-SP-01, -02, -03 (S to XL, with predicted values),
     -05 and -06 are met; G-SP-04 (memory) and G-SP-08 (the pilot's size data) are open. The NFR-PERF budgets are
     unchanged; a recycle never relaxes them.
   - **The memo list stays.** Decisions 2 and 3 and the initial policy stand: cold first, memoise where measured,
     under the oracle. The query shapes change as listed below.
   - **Decision 7 is replaced: incremental connectivity.** The fallback "page-scoped dirty flags with coarse
     recomputation" does not meet NFR-PERF-05 as worded, because the coarse recompute of the global part alone
     costs 87 ms at M. Global connectivity becomes component-scoped: the local nets stay one memo per page (one
     page re-executes per edit, within budget at M); the stitch graph and the potential partition are kept per
     connected component; an edit that only adds stitch edges unions components in near-constant time, and an edit
     that removes an edge or changes a page's nets recomputes only the components that held the touched page nets,
     from the per-page nets. The incremental result must equal the cold family, which stays the oracle's reference
     in CI. `ew-memo` makes this possible without knowing ECAD: a family's re-execution may receive its previous
     value and the touched inputs (an incremental re-execution hook), and its result is still checked against
     `run` cold. The detailed algorithm is cycle 9's WP-S1.07 work for the lead programmer.
   - **Decision 4 gains four items, still v1.** (a) **Push invalidation**: `advance` receives the commit's touched
     inputs from the change feed; the memos reachable from them through the reverse-dependency index (amendment 1)
     are marked possibly changed and verified at their next read, and every other memo is re-stamped in O(1) at its
     next read without walking its dependencies. Pull verification alone costs 12 µs per page memo (18.8 ms for the
     1,499 untouched page memos at L, over the budget on its own) and 0.1–0.2 µs per dependency entry. The index is
     keyed by **page-level keys** (page, device shard, table, settings member), as amendment 1 foresees: with
     row-level keys it would hold about 56 MB at XL (0.87 million dependency entries of the two prototyped families
     alone), over its 40 MB budget. (b) **O(1) hits**: a read checks `verified_at` before it clones
     anything; dependency lists sit behind an `Arc`. (c) **Nested memos**: `Q` gets a memo backend, and a read of a
     memoised family inside another records one memo dependency instead of the nested family's flattened reads.
     (d) **Value bytes**: the connectivity families implement `Query::bytes`, so the memory gate counts values and
     not only dependencies.
   - **Query shapes** (architecture §6.4). `potential_names` is split out of `potential_partition`, which keeps the
     topology only; `profile_row_for(attributes)` is a new memo, and `wire_props_of(potential)` reads the profile
     only through it, so a profile edit re-derives the affected potentials only; the project row is read per
     settings member (one projection memo per member with early cut-off, so a colour-profile edit no longer
     invalidates the pairing); `potential_flow(potential)` becomes Cold (a lookup into `potential_flow_bfs`);
     `stitch_graph` reads page-less connections, bridges, interruption-point functions and rail-bonded terminals
     through indexes; the super-linear chain of `physical_wires_of` is fixed before that family is memoised, and
     `physical_wires_of` and `wire_props_of` go under the LRU if their value bytes stay bulky. Granularity rule
     (decision 6) added: **no whole-table read on an interactive path**.
   - **Expectations for the re-test.** The commit of a move or a wire (NFR-PERF-05) is predicted at a few
     milliseconds at L (commit, one page's local nets, the touched components and their names). The profile change
     (NFR-PERF-07) is feasible **only with push invalidation**: the affected-only measurement (56 ms p99 at L
     against 60 ms) is a lower bound, because it leaves out the verification of the about 33,000 unaffected
     `wire_props_of` memos, and pull verification of those alone would exceed the budget at L; at XL even the lower
     bound (209 ms) is over, so the wire-chain fix is needed too. Memory (G-SP-04) is open: the benchmark's peak
     working set, an upper bound that includes the generator's records, is 1.83 GiB at L, over the 1.5 GB of
     NFR-PERF-13, and 5.78 GB at XL, over the 4 GB budget, although the XL run left out the two heaviest
     (super-linear) families, so the true worst case is higher and the XL peak is not comparable with L. The model
     rows are 313 MiB at XL and the prototyped dependency lists 28 MB; the re-spike measures the value bytes per
     family after the wire-chain fix.
   - **Evidence rules, accepted once.** The spike ran on a non-dedicated Windows desktop and clamped a latency
     family to 100 timed samples when its warm-up median exceeded ten times the budget (the row decided by its
     median). Both deviations from [quality](../design/18-quality-security-release.md) §4.3 are accepted as evidence
     for this outcome only, because every gap is an order of magnitude beyond the machine factor (about 1.5–2 to
     BM). The re-test after the recycle runs on a BM-class runner by §4.3.
   - **ADR-0001 is unchanged.** Push invalidation works over the existing row, index-key and table stamps; the
     page-level keys are **required in the reverse-dependency index** (56 MB at XL with row-level keys) and live only
     there, and per-member settings keys are projection memos over the one project-row stamp. Page-scoped stamps in
     `ew-model` remain the fallback, to be proposed as an ADR-0001 amendment only if page-level keys in the index do
     not bring it within 40 MB at XL.
   - **Plan impact** (G-SP-06; [work breakdown](../plan/02-work-packages.md)): WP-S1.05 +3 pw, WP-S1.07 +4 pw and
     WP-S1.08 +0.5 pw (likely); M1 of the plan of record moves from month 15 to month 16 (+3 to +5 weeks on the
     critical chain); the likelihood of SR-7 (R-018) rises. Cycle 9 takes WP-S1.07's incremental connectivity with
     the `ew-memo` integration as its lead item.
5. **The memo engine and the memo backend of `Q` as built** (2026-10-20, project owner (delegated); decision log
   ID-241 to ID-250; WP-S1.07 part 1, reviewed in cycle 9 and all twelve findings fixed in its fix round;
   [architecture](../design/04-architecture.md) §2, §6.2, §6.4). Amendment 4's items (a) to (d) and its incremental
   hook are implemented in `ew-memo`, and `Q` is memoised through it in every `Session`, with these readings:
   - **Re-entrant store.** Every read takes `&self`, and no internal borrow is held while host code runs (a query
     body, `changed_at`, `durability`, `coarse`); a body that calls back into the store through a shared handle
     nests safely, and its reads are attributed to the innermost execution. `ew-derive`'s `MemoStore` keeps this
     state behind shared interior mutability, so **`MemoStore` and `Session` are not `Send`** (a `compile_fail`
     doctest pins it). That is the thread model of
     architecture §2: the `Session` lives on the UI thread, and a job receives an O(1) fork, never the session;
     the job-model work makes the fork it hands out sendable.
   - **Coarse registration keys.** A read is registered in the reverse-dependency index under `Db::coarse` of the
     input, computed in the snapshot the read observed (the page a row is drawn on, a 4,096-way ID or index-key
     shard, or the read itself for whole tables, whole indexes, the project row and the reserved records; decision
     log ID-226). The touch contract: a commit reports, for every put row, its coarse keys before and after the
     change. As first built the registrations were not kept with the memo, so a row that moved to another page left
     a stale registration (over-marking only); the fix round **stores each memo's registrations with it**
     (`Memo::reg`) and replaces them when the memo re-executes, so nothing leaks. Their memory at XL is measured in
     the re-spike.
   - **Marking cost.** `Changes::via` is collected only for incremental families. A **mark epoch** bounds the walk:
     a commit does not re-walk a memo whose mark already walked its readers in the current epoch, and the epoch is
     bumped whenever a mark is cleared without verifying what marked it (a durability skip, or an incremental run
     that left a `via` memo unread). The reserved records count as touched only when they differ.
   - **Part dependencies are the "per connected component" mechanism** of amendment 4. A reader takes one part of a
     producer's value (`get_part`: a potential's topology, an endpoint's potential, a potential's flow entry) and
     is invalidated only when the producer's re-execution declares that part changed (`changed_parts`), or changes
     without declaring parts. Every verification of a reader **recomputes the producers** it depends on, so the
     latent stale path the review found (a green memo keeping old producers after a cut-off) is closed. No family
     reads parts yet; the partition becomes a producer in WP-S1.07 part 2.
   - **Volatile executions; amendment 3 "Cycles" is amended.** Amendment 3 said that a cycle error "leaves the
     previous memo in place". As built, an execution that caught a cycle below it, or read a volatile value, is
     **volatile**: its value depends on the evaluation context, so it is returned but **never kept**, and its
     readers become volatile too. A memoised `Q` therefore re-runs such a value on every read, while a cold `Q`
     caches it for its own lifetime; both return the same value for the same snapshot and order. The store counts
     volatile executions in its statistics (`Counters::volatile`). Debug builds still panic on a cycle unless the
     host turns that off.
   - **Notes are part of the memoised value.** The value of a memoised `ew-derive` family is its value with the
     derivation notes its execution pushed; a hit replays the notes, and cut-off and the oracle compare both (a
     changed note is a changed value). The notes form a shared DAG (`NoteSet`): a reader refers to the note sets of
     the values it read instead of copying them, replays them by reference, and memory counts a note set once, with
     the value that owns it. This closes FT-44.
   - **Memoisation is on by default, under a committed oracle test.** `SessionOptions::memo` is `Off`, `On` (the
     default) or `Oracle` (every memoised read also runs cold); `Session::query` hands out a memoised `Q`, a commit
     advances the store with the touched keys of its change feed, and `query_over(model)` stays cold. Every
     `Memo` and `Memo + LRU` family is memoised, those of `ew-reports` included. The default rests on
     `crates/ew-api/tests/memo_oracle.rs`: over 16 edit states of the golden project (moves on and across pages,
     wires, a delete, an override, a rename, renumbers, undo and redo, a colour-profile edit) it reads every key of
     every memoised family twice through memoising `Q`s and once cold, comparing values and notes, and runs the
     same states in `Oracle` mode sampled at `OneIn(8)`; a nightly or release run at `Oracle::Every` is FT-113.
     The cold reference of the oracle is a fresh cold `Q` over the same snapshot.
   - **Stamp content rule.** The `Db` stamp contract is one-sided: every change of what a read observes must change
     its stamp, but a changed stamp with equal content need not be avoided or reported specially. A row put twice
     in one batch bumps an index bucket's stamp without changing its content; that costs a verification, never a
     wrong value. The rule is part of the `Db` contract.
   - **No silent defaults; no history jumps.** `Query::fingerprint` has no default: a family compared by
     fingerprint must implement it (a default of 0 would have cut off every change). `MemoStore::replace` is
     removed: a new history (another lineage of snapshots) needs a new store, and a store never moves to an
     arbitrary snapshot.
   - **Value bytes and the query shapes.** The five connectivity families implement `Query::bytes` (amendment 4
     item (d)), tested on a small wired fixture because `ew-derive`'s golden fixture has no connections (FT-116),
     and `potential_flow` is Cold, a lookup into `potential_flow_bfs` (decision log ID-221).
   - **Page order inside pagination.** The connectivity queries (`physical_wires_of`, the potential queries and
     the stitch graph) read `drawn_page_order`, never `page_order`, so they can run inside the report engine's
     pagination without a cycle; the cross-reference token `{page_index}` still reads `page_order` (decision log
     ID-189).
   - **Still whole-project** (WP-S1.07 part 2, cycle 10): `stitch_graph` and `potential_partition` are memoised as
     one value each and read whole tables; the hub specification scans every interruption-point group per
     potential, `wire_props_of` parses the profile per potential, the PE rule scans the whole strip per wire end,
     and the bond vias, the interruption-point pairing and the flow BFS are global. Part 2 makes the partition a
     producer (a topology-only value keyed by `PotentialId`, cold = the global algorithm, the incremental
     re-execution a union-find over the affected components), adds `settings_member`, `profile_row_for` and
     `potential_names`, fixes the wire chain and benchmarks on `MemoStore`. The G-SP re-test waits for it.
   - **Licence.** `ew-memo` stays permissive (MIT OR Apache-2.0); `ew-derive` and `ew-api` are GPL-3.0-or-later;
     no dependency was added.

## Consequences

### Positive

- The engine shrinks to about 1.5–2.5 kLOC, owned by two people by M1. Most contributors never touch it.
- Early cut-off keeps a wire edit on folio 102 from touching folio 203, and lets an unchanged display-list layer keep
  its GPU buffers (ADR-0015).
- Jobs never block edits and are never invalidated by them: a job works on a read-only memo snapshot plus a
  job-local overlay that is dropped at the end.
- Memo bugs are diagnosable:
  - **oracle mode** in CI, debug builds and nightly runs recomputes sampled queries cold after each transaction
    (all of them nightly) and compares;
  - `ew verify-derived` recomputes everything cold;
  - `ew query <name> <key> --explain` prints the dependency trail, memo state and provenance (FR-EXT-005,
    FR-UI-011).

### Negative

- EW owns a memo engine. Its correctness rests on the oracle, the command-sequence fuzzer and the access rules.
- There is no parallel speed-up on the interactive path. Global connectivity remains the hotspot at 5–10 ms per
  committed wire edit in the XL-A estimate case; live highlighting during a drag uses page-local nets only.
- Promotion decisions need profiling infrastructure before the performance spike (G-SP): the synthetic S/M/L/XL
  fixtures and the perf gate (ADR-0022). The date of the spike is the plan's ([global plan](../plan/01-global-plan.md)
  §14, PD-8; [milestones.csv](../plan/milestones.csv)), not a calendar week of the spine.
- Memory must be actively policed (granularity rules, LRU, memory gate).

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **salsa 0.28.5** | Writes cancel or block readers, which conflicts with snapshot exports during editing; inputs would duplicate the imbl tables; 0.x API churn |
| **Proposal A's full engine** | Parallel fork evaluation and memo merge-back are subtle; about 4 kLOC of novel critical-path code with a bus factor of one |
| **Hand-declared service invalidation everywhere** (D) | A missed invalidation is a stale value: freshness by discipline |
| **Cold only, forever** | Likely to meet S/M budgets but not L/XL ones (estimate). It stays the default and the per-query fallback |
| **ECS change detection** (B) | Rejected with the substrate in ADR-0001 |

## Related

- **ADRs:** [ADR-0001](0001-core-model-persistent-maps.md), [ADR-0003](0003-freshness-contract.md),
  [ADR-0008](0008-transactions-undo-preview.md), [ADR-0015](0015-display-list-renderers-pdf.md), [ADR-0022](0022-platform-matrix-and-performance-budgets.md),
  [ADR-0024](0024-licence-set-and-permissive-gpl-boundary.md).
- **Design:** [architecture](../design/04-architecture.md) §2, §6, §13, §15.1, §16 (risks 1–4), Appendix A;
  [quality, security and release](../design/18-quality-security-release.md) §3–§4.
- **Requirements:** FR-EXT-005, FR-UI-011, NFR-PERF-04, NFR-PERF-05, NFR-PERF-07, NFR-PERF-13, NFR-PERF-20,
  NFR-REL-08, NFR-MAINT-07.
