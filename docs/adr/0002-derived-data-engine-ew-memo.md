# ADR-0002: Derived-data engine: cold-first queries plus a cut-down memo layer (`ew-memo`); no salsa

**Status:** Accepted · amended 2026-10-18 · **Date:** 2026-10-17 · **Deciders:** core team (chief architect, after the architecture judge panel)

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
3. **Phase 2 (from the M0 spike, weeks 3–5): memoise where measured.** A query is promoted to `Memo` when profiling
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

Accepted on 2026-10-18. The decision text above is unchanged; this entry refines it.

1. **Reverse dependencies per query family** (2026-10-18;
   [checks, revisions and collaboration](../design/15-checks-revisions-collaboration.md) §3.3, open question 18;
   [architecture](../design/04-architecture.md) §6.2). `ew-memo` v1 also offers a generic API that returns, per query
   family, the memos whose recorded reads include a given key. It stays ECAD-agnostic. The online check engine
   builds its `rdeps` index from it, so a commit re-evaluates exactly the check scopes whose last evaluation read a
   touched key. The M0 spike measures its memory (≤ 40 MB estimated at XL); above budget the engine falls back to
   coarse page and shard keys with re-verification of the matching scopes.

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
- Promotion decisions need profiling infrastructure from week 3: the synthetic S/M/L/XL fixtures and the
  perf gate (ADR-0022).
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
