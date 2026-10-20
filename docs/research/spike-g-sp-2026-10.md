# Performance spike for gate G-SP (WP-S1.05, PD-8)

Status: evidence for gate G-SP, S, M and L complete; XL pending. Evidence for gate G-SP (`docs/plan/03-gates.md` §5.1). The decision (PD-8) is the project owner's
and the gate chair's; this note gives the measurements, the analysis and a recommendation.

## 1. Summary and recommendation

**Recommendation: Recycle** (one re-spike of at most four weeks, `docs/plan/03-gates.md` §5.1), on S, M and L
evidence. The cold implementations miss the commit budget of NFR-PERF-05 by 2.6-3× at S, 25-27× at M and 66-97×
at L, and memoising `potential_partition` as one unit does not help (it re-executes on every edit). The causes are
of the kind the Recycle outcome names: whole-table reads in `stitch_graph` and `potential_partition`, potential
names (device tags, anchor cells, page order) computed inside the topology query, a super-linear wire-property
chain, and a memo engine that verifies by walking every dependency instead of starting from the touched keys.
The folio renumber (NFR-PERF-07) and the open-time proxy (NFR-PERF-03) are within budget cold at every measured
class. The affected-potentials part of the profile change is within budget at S, M and L; it is a **lower bound**
of the memo design's cost (it leaves out verifying the unaffected `wire_props_of` memos, which pull verification
would add, section 5), so NFR-PERF-07 is reachable only with push invalidation and the reshaped profile lookup.

- **KT-3 check.** Not triggered: KT-3 applies after the fallback prototype and one recycle. For the record, the
  M-class commit p99 is 134.7 ms cold (13× the KT-3 line of 10 ms); the memo prototype's page-local part alone is
  4.8 ms (move) and 8.3 ms (wire) at M. The open-time proxy is 1.0 s p99 at L (budget 5 s); a linear projection to
  XL is about 4 s against the KT-3 line of 40 s.
- **What XL could change.** Nothing in the direction of the recommendation: every failing family scales at least
  linearly from M to L, so XL can only widen the gaps. XL can still decide two open points: whether the memory
  projection of section 7 holds (G-SP-04) and how the affected-potentials profile change scales (it is at 0.9×
  budget at L). XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit.
- **G-SP-01 (H) stays open** until the XL results are appended (raw numbers per class S-XL). The XL run omits
  `connectivity.wire_props_all` and `profile.change.all` (appendix A.5), so its peak working set is not comparable
  with the L figure, whose peak comes from exactly those two families.
- **Correctness.** The memo prototype agreed with cold evaluation on every compared sample (6,000 comparisons at
  S, 4,200 at M, 420 at L, plus `ew-memo`'s own oracle at every depth on all warm-up rounds): no mismatch.

## 2. Method

**Instruments.** `ew_testkit::synth` (generator version 2, seed 1, default `Options { routes: true }`) builds the
four fixtures; `ew_testkit::bench` holds the statistics, the budgets and the table printer; the benchmark
`crates/ew-testkit/benches/spike.rs` runs the G-SP families; `crates/ew-testkit/benches/spike/memo.rs` is the
memo prototype (section 6) and `crates/ew-testkit/tests/spike_memo.rs` its non-timing correctness tests. Run with

```text
cargo bench -p ew-testkit --bench spike                       # class S
EW_BENCH_CLASS=M|L|XL cargo bench -p ew-testkit --bench spike  # opt-in classes
cargo bench -p ew-testkit --bench spike -- --quick            # smoke run, not evidence
EW_SPIKE_ONLY=move,memo.partition ...                          # a subset of families
```

**Procedure** (`docs/design/18-quality-security-release.md` §4.3, gate §5.1). Interactive operations are latency
benchmarks: 20 untimed warm-up samples, then 1,000 timed samples, reported as min, median, p99 (nearest rank) and
max. Bulk operations and builds are single operations: one warm-up run, then the median of 10 runs. Every latency
sample runs on a fresh `Session` over an O(1) clone of the generated baseline, built outside the timer, and the
previous sample's state is dropped before the next one is built. Every derived value is computed in a fresh `Q` per
sample (cold: nothing survives between samples).

**Clamped rows.** When the median of the warm-up samples of a latency family exceeds ten times its budget, the
family takes 100 timed samples instead of 1,000 and the row is flagged `clamped`. Such a family is decided by its
median (it is over budget by an order of magnitude whatever its p99), and 1,000 samples of a one-second XL
operation would take hours. This is a deviation from 18 §4.3, recorded here; no clamped row is within budget.

**Families** (the operation and the derived queries it invalidates, all cold unless named `memo.*`):

| Family | Operation (timed) | Budget |
|---|---|---|
| `move.commit_potentials` | `rep.move` of a load representation (6 × 3 grid steps) + `page_local_nets(page)` + `potential_partition()` | NFR-PERF-05 |
| `wire.commit_potentials` | `connection.draw` between two free CPs of a page + the same two queries | NFR-PERF-05 |
| `folio.renumber.derive` | `page.renumber` of one folio to a free number (the maximum folio plus one; preview + commit) + `page_order()` + `device_tag` of every device drawn on the page | NFR-PERF-07 (5 ms plus the preview diff) |
| `profile.change.all` | the 24 V DC row of the colour profile changes colour, applied at the model layer + `wire_props_of` of every potential | NFR-PERF-07 |
| `profile.change.affected` | the same change + `wire_props_of` of the affected potentials only, with the topology already derived in that `Q` | NFR-PERF-07 (memo-ideal work) |
| `connectivity.*` | `stitch_graph()` + `potential_partition()` from scratch, and its breakdown | single operation, no budget |
| `open.session_from_records` | `Session::from_records` (tables, indexes, integrity check, project notes) | NFR-PERF-03 proxy |
| `memo.*` | the same edits, then the memoised families of the prototype (section 6) | NFR-PERF-05 |

`*.commit` and `*.potentials` / `*.derive` / `*.tags` rows split the total into the command pipeline and the
derivation. The sessions have no store attached, so `.commit` excludes the journal write that the session performs
after a commit when a project store is attached (WP-S1.09); G-M1 measures it.

**Profile change at the model layer.** No command edits `ProjectSettings` yet, so the profile change is one
`Op::Put` of the project row applied with `Model::apply`; it excludes the command pipeline, which costs what the
`*.commit` rows show (well under a millisecond).

**Oracle.** Every memoised value is compared with a cold evaluation of the same snapshot after the timer stops:
every sample at S and M, every tenth at L and XL, and `ew-memo`'s own oracle (`Oracle::Every`, every depth) runs on
all warm-up rounds. No mismatch was found (the counts are in appendix A).

**Memory.** `model_bytes` mirrors the undo-history row estimator of `ew-commands` over the generated records;
`History::bytes` after one step; `Store::memory` per memo family (memos, dependency entries) with the entry size
`size_of::<(ew_memo::DepKey<ew_derive::DepKey>, ew_memo::Rev)>()`. Peak resident memory is not measurable inside
the benchmark (no permissively licensed crate in the lockfile, `unsafe` is forbidden); the run measured the peak
working set of the benchmark process from outside (PowerShell, `Process.PeakWorkingSet64` polled every 200 ms). It
is an upper bound for the application: the process holds the generator's records and the model at the same time.

## 3. Fixtures

Seed 1, generator version 2. Counts from `synth::project` (the S and M rows are asserted by `tests/synth.rs`):

| Class | Pages | Devices | Functions | Connections | CPs | Terminals | Channels | Representations | Junctions | Records |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| S | 50 | 300 | 900 | 1,500 | 3,637 | 120 | 60 | 900 | 348 | 4,024 |
| M | 300 | 2,500 | 7,500 | 15,000 | 37,178 | 1,000 | 500 | 7,500 | 2,960 | 35,884 |
| L | 1,500 | 12,000 | 36,000 | 70,000 | 171,330 | 4,800 | 2,400 | 36,000 | 14,226 | 170,319 |
| XL | 5,000 | 50,000 | 150,000 | 250,000 | 603,482 | 20,000 | 10,000 | 150,000 | 59,438 | 666,389 |

The class bounds are the assumed bounds of NFR §2.1 (AS-2): the recalibration against the pilot's three largest
projects (CP-03) is **pending**, so G-SP-08 stays open. The derived counts (local nets, stitch nodes and edges,
potentials) are printed per class in appendix A.

## 4. Machine class and toolchain

Desktop, x64, 12 cores / 24 threads (Zen 2, 3.8 GHz), 32 GB RAM, local NVMe SSD, Windows 11 Pro; no CPU governor
pinning or core isolation (18 §4.3 prescribes a Linux bare-metal runner of the BM class for the gate; this machine
is faster than BM per core). Rust 1.99.0, release profile of the workspace, single-threaded benchmark. The numbers
are therefore indicative for BM within roughly a factor 1.5 to 2 (BM: a 4-core 2018+ x64 CPU). The desktop was
not dedicated to the run: other development sessions compiled on it at times (appendix A.5).

## 5. Results per query family and class

All values are p99 in milliseconds over 1,000 samples (latencies) or the median of 10 runs (single operations);
`(c)` marks a clamped row (100 samples, section 2). The complete tables, with min, median and max, are in appendix
A. XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit.

**Derived counts.**

| Class | Local nets | Stitch nodes / edges | Potentials | Devices drawn per page | 24 V potentials (wires) affected by the profile change |
|---|---:|---:|---:|---:|---:|
| S | 804 | 804 / 226 | 587 | 8.8 | 3 (153) |
| M | 9,080 | 9,080 / 1,676 | 7,455 | 12.6 | 17 (1,325) |
| L | 41,548 | 41,548 / 8,211 | 33,589 | 12.0 | 84 (6,352) |

**Commit plus global potential update (NFR-PERF-05).**

| Row | S | M | L |
|---|---:|---:|---:|
| `move.commit` (command pipeline only) | 0.096 | 0.152 (c) | 0.198 (c) |
| `move.potentials` (fresh `Q`: `page_local_nets` + `potential_partition`) | 14.835 | 134.586 (c) | 974.096 (c) |
| `move.commit_potentials` | **14.918** | **134.739** (c) | **974.255** (c) |
| `wire.commit` | 0.055 | 0.067 (c) | 0.065 (c) |
| `wire.potentials` | 12.993 | 123.853 (c) | 656.230 (c) |
| `wire.commit_potentials` | **13.032** | **123.903** (c) | **656.282** (c) |

The command pipeline is a fraction of a millisecond in every class; the whole cost is the cold global
connectivity.

**Global connectivity, cold breakdown (median of 10, ms).**

| Row | S | M | L |
|---|---:|---:|---:|
| `connectivity.cold` (`stitch_graph` + `potential_partition` from scratch) | 9.569 | 107.568 | 1,107.899 |
| `connectivity.local_nets_all` (every page) | 1.904 | 23.043 | 111.928 |
| `connectivity.stitch_given_nets` | 1.824 | 19.670 | 98.020 |
| `connectivity.partition_given_stitch` | 5.084 | 63.510 | 318.269 |
| `connectivity.partition_given_stitch_tags` (tags, anchor cells, CPs, page order precomputed) | 4.006 | 59.719 | 267.816 |
| `connectivity.device_tags_all` | 0.886 | 9.770 | 50.787 |
| `connectivity.wire_props_all` (every potential, partition precomputed) | 11.942 | 330.979 | 8,095.835 |

Readings:

1. `page_local_nets` costs about 38 µs per page at S and 75-77 µs per page at M and L: above the 20 µs
   promotion threshold, linear, and local to the edited page.
2. `stitch_graph` given the local nets (20 ms at M, 98 ms at L) and `potential_partition` given the stitch graph
   (64 ms at M, 318 ms at L) are global and linear in the project size. Each alone exceeds the M and L budgets, so
   **no memo policy over the current query shapes can meet NFR-PERF-05 for an edit that re-executes them**.
3. Most of the partition's own cost is not the union-find: with tags, anchor cells, CPs and the page order
   already evaluated it still takes 60 ms at M. The fallback potential names (`<tag>:<designation>` of the first
   CP in page and cell order) and the per-CP role checks walk every member CP through tracked index reads,
   allocating a bucket vector per read. This is work for a separate `potential_names` family, not for the
   topology.
4. The wire chain is super-linear. `connectivity.wire_props_all` (partition precomputed; `physical_wires_of`, the
   flow and `wire_props_of` computed) has a mean per potential of 20 µs at S, 44 µs at M and 241 µs at L: 5.5× in
   per-potential cost from M to L for 4.5× as many potentials. The figure of `profile.change.affected` is not
   comparable with that mean (it times `wire_props_of` alone, with `physical_wires_of` and the flow precomputed,
   for the 24 V potentials only); it is about 0.21 ms per affected potential at both M and L, so the growth is not
   in `wire_props_of` of potentials of a fixed shape. The likely place is the project-wide potentials (L, N and PE
   run through interruption-point pairs across all replicas) in `physical_wires_of` or the flow, but the cause is
   not isolated. It is a defect of the cold implementation to fix before any memo decision on `physical_wires_of`
   and `wire_props_of`.
5. The `connectivity.cold` median at L (1.11 s) is twice the sum of its parts (0.53 s), and its p99 is 2.4 s. The
   machine was not dedicated (section 4): treat L medians of single operations as ±30 %. The decision does not
   depend on it.

**Folio renumber (NFR-PERF-07: "103 → 104" commits in 5 ms plus the preview diff).** One folio is renumbered to a
free number, the maximum folio plus one, which is the NFR's case of a folio moving to an unused number.

| Row | S | M | L |
|---|---:|---:|---:|
| `page.renumber.folio` (preview + commit) | 0.043 | 0.078 | 0.228 |
| `folio.renumber.tags` (`page_order` + the tags of the page's devices, fresh `Q`) | 0.119 | 0.381 | 1.430 |
| `folio.renumber.derive` (both) | **0.160** | **0.449** | **1.707** |

Within budget cold in every measured class, with a factor 3 of margin at L.

**Profile change: the 24 V DC row changes colour (NFR-PERF-07).**

| Row | S | M | L |
|---|---:|---:|---:|
| `profile.change.all` (every potential re-derived, cold worst case) | 34.335 | 545.186 (c) | 10,882.561 (c) |
| `profile.change.affected` (only the affected potentials, topology already derived) | **0.569** | **8.824** | **56.370** |

`profile.change.all` is what the current shape costs: the project row is one dependency, so a profile edit
invalidates the stitch graph (it reads `settings.ip_pairing`), the partition and every `wire_props_of`.
`profile.change.affected` is the work an ideal memo does and a **lower bound**: it re-derives the affected
potentials only and leaves out the verification of the other `wire_props_of` memos (33,505 at L). Its L median is
18.2 ms and its p99 56.4 ms (0.94× the 60 ms budget). With pull verification each unaffected memo is walked: with
about ten dependencies per memo once nested memos are dependencies (section 11) and the 0.1-0.2 µs per dependency
entry of section 6, that adds roughly 35-70 ms at L, which puts the family over budget. With push invalidation from
the touched settings member through `profile_row_for` the unaffected memos are not visited, and the lower bound is
the expected cost.

**Open-time proxy (NFR-PERF-03).** `open.session_from_records` (tables, indexes, integrity check, project notes),
p99: S 8.6 ms, M 95.8 ms, L 1,017 ms (median 706 ms). Budgets 0.5 / 2 / 5 s: within, by a factor 5 or more.

## 6. Memo prototype

**Db mapping** (`benches/spike/memo.rs`). The inputs are `ew-derive`'s `DepKey`s exactly as `Q`'s tracked
accessors report them (project row, row, table, index key, whole index, reserved records). `ModelDb::changed_at`
performs the same tracked read through a probe `Q` whose tracker echoes the observed stamp, so the stamps are by
construction those of `ew-derive`'s verifier contract, without raw-read access (a test asserts equality with the
recorded stamps). Durability: project row = settings, embedded library copies = library, the rest = drawing; a
commit advances the store with the most durable level among the change feed's touched keys; the store revision is
the model revision. Each sample works on an O(1) fork of a store warmed on the baseline.

**Families.** `Memo<F>` memoises the `ew-derive` family `F` as one opaque unit: it evaluates `F` in a fresh `Q`
with a recording tracker and reports the minimised reads (duplicates removed, row reads of tables read whole
dropped) as the memo's dependencies. Prototyped for `potential_partition()` and `page_local_nets(page)` (all
pages). Nested families cannot be separate memos without a hook in `Q` (section 11).

**Results** (p99 ms; `.derive` = the memo step alone: new `Db`, `advance`, the reads; the row without suffix adds
the commit; the counters are those of the last sample):

| Row | S | M | L | Counters |
|---|---:|---:|---:|---|
| `memo.partition.hit` (same revision) | 0.011 | 0.211 | 0.716 | hit |
| `memo.partition.unrelated` (conductor colour edit) | 0.572 | 15.238 (median 4.0) | 26.494 | 1 verification, no re-execution |
| `memo.partition.move` | 17.313 | 338.600 (c) | 719.296 (c) | re-executed, cut off |
| `memo.partition.wire` | 13.978 | 233.479 (c) | 687.614 (c) | re-executed, changed |
| `memo.local_nets.move` (every page read) | 0.529 | 4.758 | 21.237 | 1 re-execution (cut off), N-1 verifications |
| `memo.local_nets.wire` | 0.517 | 8.329 (median 3.9) | 20.824 | 1 re-execution, N-1 verifications |

Findings:

1. **An opaque partition memo is worse than cold** for a move or a wire (M: 339 ms against 135 ms cold): it
   depends on whole tables (connections, functions, devices, pages, potential definitions, bonds) and on every
   representation row, so every drawing edit re-executes it, and the re-execution pays the recording on top.
   A move is cut off (the partition value is unchanged), which confirms that early cut-off would protect the
   downstream families if the partition re-execution itself were cheap.
2. **Verification is O(dependencies), not O(change).** An unrelated edit verifies the partition's 15k (M) and 71k
   (L) dependencies: 4 ms at M, 21 ms at L median. Reading every page's local nets after a move verifies
   N-1 page memos of about 100 dependencies each: 12 µs per page, 3.6 ms at M and 18.8 ms at L (2.1× the L
   budget; at 5,000 XL pages about 63 ms by projection). Pull verification alone cannot meet NFR-PERF-05 at XL
   (about 3× the budget) and is marginal at L. The per-dependency cost is pessimistic: every `changed_at` of the
   prototype goes through a tracked read on a probe `Q` (accessor dispatch, a bucket vector per index read), which
   a `DeriveDb` with raw read access inside `ew-derive` avoids; a factor 2-3 faster verification would bring L
   within budget but not XL, whose cost grows with the page count. The engine needs to start from the touched keys
   (push invalidation over the reverse-dependency index) or much coarser keys (one stamp per page).
3. **A hit is O(dependencies) too** (0.2 ms at M, 0.7 ms at L for one memo with 15k and 71k dependencies):
   `Store::up_to_date` clones the memo, dependency list included, before it checks `verified_at`
   (`crates/ew-memo/src/store.rs`, the `slots[..].memo.clone()` at the top of `up_to_date`). A hit should be O(1).
4. The page-local part is cheap and local, as designed: one page re-executes (77 µs) and is cut off after a move.

## 7. Memory and open time

| Measure | S | M | L |
|---|---:|---:|---:|
| Model rows (`model_bytes`, the `ew-commands` row estimator) | 1.9 MiB | 17.3 MiB | 81.7 MiB |
| Undo history after one move / wire / folio renumber | 464 B / 56 B / 0.7 KB | 7.8 KB / 56 B / 0.7 KB | 7.1 KB / 56 B / 0.7 KB |
| `page_local_nets` memos: dependency entries (bytes at 32 B each) | 3,550 (0.11 MB) | 31,500 (1.0 MB) | 149,500 (4.8 MB) |
| `potential_partition` memo: dependency entries (bytes) | 1,881 (0.06 MB) | 14,983 (0.48 MB) | 71,246 (2.3 MB) |
| Peak working set of the benchmark process (polled) | 46 MiB | 380 MiB | 1,826 MiB |
| Open-time proxy, p99 (budget) | 8.6 ms (0.5 s) | 95.8 ms (2 s) | 1,017 ms (5 s) |

XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit.

- **Memo bytes (G-SP-04, ≤ 300 MB at XL).** Only dependency bytes are measured: the prototype's `Query::bytes` is
  the default `size_of`, so value bytes are not counted. By linear projection (XL = 3.6× L in connections) the two
  prototyped families hold about 0.8 M dependency entries (26 MB) at XL. The value bytes of the connectivity
  families are not measured; the re-spike must implement `Query::bytes` for them.
- **Reverse-dependency index (≤ 40 MB at XL, ADR-0002 amendment 1).** An index over the same dependencies has one
  entry per dependency entry; with the persistent-map overhead (key, slot and node, about 64 B per entry) the two
  families alone project to about 50 MB at XL, over budget before any other family is added. With fine-grained
  keys the rdeps index does not fit; with one stamp per page (and per shard for the global families) the entry
  count drops by roughly two orders of magnitude. **Plan: coarse page and shard keys**, as amendment 1 foresees.
- **Peak RSS (NFR-PERF-13, ≤ 4 GB at XL).** The benchmark's peak working set grows faster than the model: 1,826 MiB at
  L against 82 MiB of model rows. The peak comes from the families that hold every potential's wire properties in
  one `Q` (`connectivity.wire_props_all`, `profile.change.all`), which also carry the super-linear wire chain. A
  memo store holding `wire_props_of` for every potential would hold the same values. The projection to XL is not
  reliable until the wire chain is linear; at the measured L ratio it would exceed 4 GB. **At L the measured peak
  (1,826 MiB = 1.91 GB) already exceeds the NFR-PERF-13 L budget of 1.5 GB**; it is an upper bound for the
  application (the benchmark process also holds the generator's records, a second baseline for the profile change
  and every potential's wire properties in one cold `Q`), but nothing measured shows it below budget. **Plan:** measure the
  value bytes per family in the re-spike, after the wire-chain fix, and keep `physical_wires_of` and
  `wire_props_of` under the LRU (`MemoLru`) if they stay bulky.
- **Open time (NFR-PERF-03).** Within budget at S, M and L with a factor 5 or more (model build only; the real open
  time, parsing included, is confirmed at G-M1).

## 8. Budget table (G-SP-03)

Measured p99 against the budget; the proposed decision is per family; the predicted values assume the
re-spike's design (section 11) and are not measured. XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit.

| Family | Class | Budget | Measured p99 (cold) | Proposed decision | Predicted after the re-spike |
|---|---|---:|---:|---|---|
| Move commit incl. potentials | S | 5 ms | 14.9 ms (3.0×) | Memo (two-level, push invalidation) | < 1 ms: commit 0.1 ms + one page's local nets 0.08 ms + names of the touched potentials |
| | M | 5 ms | 134.7 ms (26.9×) | Memo | < 1.5 ms |
| | L | 10 ms | 974.3 ms (97.4×) | Memo | < 2 ms |
| | XL | 20 ms | XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit | Memo | < 3 ms |
| Wire commit incl. potentials | S | 5 ms | 13.0 ms (2.6×) | Fallback: incremental connectivity under the oracle | ≤ 2 ms |
| | M | 5 ms | 123.9 ms (24.8×) | Fallback (incremental) | ≤ 3 ms; a wire joining two project-wide rails recomputes one component |
| | L | 10 ms | 656.3 ms (65.6×) | Fallback (incremental) | ≤ 6 ms typical; rail components to be measured |
| | XL | 20 ms | XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit | Fallback (incremental) | ≤ 15 ms typical, to be confirmed |
| Folio renumber to a free number (max + 1) incl. page order and tags | S | 5 ms + preview | 0.16 ms | Cold (within budget) | — |
| | M | 5 ms + preview | 0.45 ms | Cold | — |
| | L | 5 ms + preview | 1.71 ms | Cold | — |
| | XL | 5 ms + preview | XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit | Cold | by linear projection about 6 ms; Memo of `page_order` and `device_tag` covers it |
| Profile change reaching all views | S | 30 ms | 34.3 ms all (1.1×); 0.57 ms affected (lower bound) | Memo with per-member settings keys, `profile_row_for` and push invalidation | ≥ 0.6 ms |
| | M | 30 ms | 545.2 ms all (18.2×); 8.8 ms affected (lower bound) | Memo | ≥ 9 ms; + about 7-15 ms with pull verification |
| | L | 60 ms | 10,883 ms all (181×); 56.4 ms affected (0.94×, lower bound) | Memo | ≥ 56 ms with push invalidation (tight); about 90-125 ms with pull verification of the 33.5k unaffected memos (over) |
| | XL | 100 ms | XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit | Memo | to be measured |
| Open-time proxy (model build) | S | 0.5 s | 8.6 ms | — (within) | — |
| | M | 2 s | 95.8 ms | — | — |
| | L | 5 s | 1.02 s | — | — |
| | XL | 20 s | XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit | — | about 4 s by linear projection |

The failing rows at M and L: move commit (M 26.9×, L 97.4×), wire commit (M 24.8×, L 65.6×) and the cold profile
change (M 18.2×, L 181×). Of the memo prototype's rows, `memo.partition.move` and `.wire` (M 46.7-67.7×,
L 68.8-71.9×), `memo.partition.unrelated` (M 3.0×, L 2.6×), `memo.local_nets.wire` at M (1.7×) and both
page-local rows at L (2.1×) fail as well, the last three for the verification reason of section 6. The L peak
working set (1,826 MiB = 1.91 GB) is over the NFR-PERF-13 L budget of 1.5 GB (an upper bound, section 7).

## 9. Query-policy table (G-SP-05)

The machine-readable table is `docs/research/spike-g-sp-query-policy.csv`. Changes against ARCH §6.4 (the
families not listed keep their policy; they were not measured separately):

| Query | §6.4 | Proposed | Change | Evidence and reason |
|---|---|---|---|---|
| `page_local_nets(page)` | Memo | Memo | none | 77 µs per page cold (> 20 µs); one page re-executes per edit; the rest must not be walked |
| `stitch_graph()` | Memo | Memo, reshaped; incremental for topology edits | reshape | 20 ms (M) / 98 ms (L) given the nets; reads whole tables (connections, functions, devices) and the whole project row, so every edit re-executes it; read through indexes and the `ip_pairing` member only |
| `potential_partition()` | Memo | Memo (topology only); incremental for topology edits | split | 64 ms (M) / 318 ms (L) given the stitch graph, most of it names and role checks |
| `potential_names()` | — | Memo | new | the fallback names read device tags, anchor cells and page order; a folio renumber or a move re-derives names, not topology |
| `physical_wires_of(potential)` | Memo | Memo after a fix | fix first | super-linear wire chain (44 µs per potential at M, 241 µs at L) |
| `potential_flow_bfs()` | Memo | Memo | none | not measured separately |
| `potential_flow(potential)` | Memo | Cold | demote | a map lookup into `potential_flow_bfs` (< 20 µs), like `wire_props(connection)` |
| `wire_props_of(potential)` | Memo | Memo, reshaped | reshape | 0.21 ms per 24 V potential; the profile row lookup moves to `profile_row_for` so that a profile edit cuts off |
| `profile_row_for(attributes)` | — | Memo | new | a few keys (one per distinct type, voltage, AC/DC and class); re-executes on a profile edit and cuts off |
| `device_tag(dev)` | Memo | Memo | none | 3.9 µs per device at M (below 20 µs) but read every frame and by `potential_names` |
| `page_order()` | Memo | Memo | none | read every frame; folio renumber with tags 1.7 ms at L |

Every Memo row above has a cold cost above 20 µs or is read every frame (G-SP-05), except `profile_row_for`,
whose value is the cut-off point of the profile change rather than a cost saving.

## 10. Plan impact (G-SP-06)

Estimates in person-weeks (likely, `docs/plan/work-packages.csv`); the deltas are the lead programmer's estimate
for the design lead to confirm.

| WP | Likely now | Delta | New likely | Why |
|---|---:|---:|---:|---|
| WP-S1.05 | 6.0 | +3.0 | 9.0 | `Q` → `ew-memo` hook with nested memo dependencies, push invalidation from touched keys (or page stamps), O(1) hits, `Query::bytes` for the connectivity values, and the re-spike itself |
| WP-S1.07 | 6.1 | +4.0 | 10.1 | reshape `stitch_graph` and `potential_partition` (index reads, settings member keys), split `potential_names`, fix the super-linear wire chain, incremental connectivity for topology edits under the oracle, `profile_row_for` |
| WP-S1.08 | 3.5 | +0.5 | 4.0 | cross-references read IP names and anchor cells through the new `potential_names` shape |

- **M1 re-forecast.** WP-S1.07 is on the M1 critical chain; +4 to +5 weeks on the chain with one developer on it,
  about +3 weeks if WP-S1.05's engine work runs in parallel on a second developer. The Recycle itself takes at most
  four weeks and overlaps that work (the re-spike measures the reshaped queries).
- **SR-7 / R-018.** Partly materialised: the fallback named in ARCH §6.1 (page-scoped dirty flags) is needed in
  the form of incremental connectivity for topology edits. Raise the likelihood; the impact stays "1-2 months at
  M1"; the mitigation becomes the Recycle plan above.
- **KT-3.** Not triggered; it is re-evaluated after the re-spike against the M-class commit p99 (10 ms) and the XL
  open time (40 s).

## 11. What the `Q` / `ew-memo` integration needs

What the prototype had to work around, and the API it asks for (no change to `ew-derive` or `ew-memo` was made in
this lane):

1. **A memo backend in `Q`.** `Q::get::<F>` consults an installed backend for families whose `POLICY` is not
   `Cold`: on a miss the backend executes `F::run` in a child `Q` whose tracker forwards reads to the memo frame
   (`Cx::read`), and a nested `q.get::<G>` of a memo family becomes `Cx::get::<MemoOf<G>>`, so the caller records
   one `DepKey::Memo` instead of `G`'s flattened reads. Shape: `Q::with_memo(model, &RefCell<Store<DeriveDb>>)`,
   values type-erased per family (`TypeId`), the `Db` (`DeriveDb`) inside `ew-derive` (it holds raw read access, so
   it reads the stamps directly instead of through the probe `Q` of the prototype). The store lives in the
   `Session` next to the `Engine`, advanced per commit from the change feed (the durability rule of the
   prototype). The report-pages provider becomes part of the store's configuration (ARCH §6.1 already says so).
2. **Push invalidation in `ew-memo`.** `Store::advance_with(rev, durability, touched: &[Input])`: memos reachable
   from the touched inputs through the reverse-dependency index are marked possibly-changed; every other memo is
   re-stamped lazily in O(1) at its next read instead of walking its dependencies. Without it, verification costs
   12 µs per page memo and 0.1-0.2 µs per dependency entry (section 6). Alternatively or additionally,
   **page-scoped stamps** in `ew-model` (one `changed_at` per page over the rows drawn on it) as `DepKey::Page`,
   which cut the dependency lists by two orders of magnitude and make the rdeps index fit (section 7).
3. **O(1) hits.** `Store::up_to_date` must check `verified_at` before cloning the memo (or keep the dependency list
   behind an `Arc`).
4. **Settings member keys.** `DepKey::Project` splits into one key per settings member read (`ip_pairing`,
   `colour_profile`, `size_profile`, `wire_default`, `numbering`, `frames`, the pinned profile), so a colour-profile
   edit does not invalidate pairing.
5. **No whole-table reads on interactive paths.** `stitch_graph` reads page-less connections, bridges, IP
   functions, link-bearing functions and rail-bonded terminals through indexes (new base indexes or derived
   index families) instead of `iter()` over connections, functions and devices.

## 12. Decisions for the design lead

1. **G-SP outcome**: Recycle (recommended) against Go with conditions; the owner decides, ADR-0002 is amended
   with the outcome (G-SP-02).
2. **Topology edits**: incremental connectivity (union-find delta for added edges, component recompute for
   removed ones, under the oracle) or the ARCH §6.1 fallback of page-scoped dirty flags with coarse recomputation.
   The measurements say a coarse recompute of the global part (`stitch_given_nets` + `partition_given_stitch`,
   medians) costs 83.2 ms at M, so the fallback as worded does not
   meet NFR-PERF-05; an amendment of ARCH §6.1 and ADR-0002 decision 7 is needed either way.
3. **Engine scope of `ew-memo` v1** (ADR-0002 decision 4): add push invalidation from touched keys, or page-scoped
   stamps in `ew-model` (ADR-0001), or both; and the `Q` memo hook of section 11.
4. **Query shapes** (ARCH §6.4): the split of `potential_names` out of `potential_partition`, `profile_row_for`,
   per-member settings keys, `potential_flow` demoted to Cold, index reads in `stitch_graph`.
5. **Measurement deviations**: accept the clamp rule and the non-dedicated Windows desktop as G-SP evidence, or
   require the re-spike on a BM-class runner (18 §4.3).
6. **Dependency rule**: the dev-only edges `ew-testkit` → `ew-derive`, `ew-memo` were added to `ci/dep-rules.toml`
   for the spike bench; ARCH §3 (the `ew-testkit` row and the dotted dev edges of the graph) needs the matching
   text.
7. **G-SP-08**: the pilot recalibration of the class bounds is still pending; set a date.
8. **G-SP-04 memory**: the L peak working set of the benchmark (1,826 MiB = 1.91 GB, upper bound) exceeds the NFR-PERF-13 L
   budget of 1.5 GB; decide whether the re-spike must show the per-family value bytes and an application-like peak
   (open, warm-up, memo store) below budget before Go, or whether a plan (LRU for the bulky wire families) suffices
   for Go with conditions.

## Appendix A. Raw benchmark output

### A.1 Class S

```text
G-SP spike benchmarks (WP-S1.05)
class=S generator_version=2 seed=1 mode=full
counts: Bounds { pages: 50, devices: 300, functions: 900, connections: 1500, cps: 3637, terminals: 120, channels: 60 }
records: 4024 (project row included), library documents: 25
samples: latency 1000 (warm-up 20; clamped to 100 when the warm-up median is over ten times the budget, full mode only), single operation 10 (warm-up 1)
filter: none
baseline model built in 9.917 ms
bench open.session_from_records class=S n=10 min=8.081ms median=8.301ms p99=8.631ms max=8.631ms
connectivity: 50 pages, 804 local nets, stitch graph 804 nodes and 226 edges, 587 potentials, 587 signals
bench connectivity.cold class=S n=10 min=9.228ms median=9.569ms p99=9.813ms max=9.813ms
bench connectivity.local_nets_all class=S n=10 min=1.879ms median=1.904ms p99=2.070ms max=2.070ms
bench connectivity.stitch_given_nets class=S n=10 min=1.720ms median=1.824ms p99=1.974ms max=1.974ms
bench connectivity.partition_given_stitch class=S n=10 min=5.012ms median=5.084ms p99=5.484ms max=5.484ms
bench connectivity.partition_given_stitch_tags class=S n=10 min=3.945ms median=4.006ms p99=4.191ms max=4.191ms
bench connectivity.device_tags_all class=S n=10 min=0.868ms median=0.886ms p99=1.002ms max=1.002ms
bench connectivity.wire_props_all class=S n=10 min=11.441ms median=11.942ms p99=12.613ms max=12.613ms
bench move.commit class=S n=1000 min=0.024ms median=0.044ms p99=0.096ms max=0.117ms
bench move.potentials class=S n=1000 min=8.626ms median=9.053ms p99=14.835ms max=15.666ms
bench move.commit_potentials class=S n=1000 min=8.659ms median=9.089ms p99=14.918ms max=15.754ms
bench wire.commit class=S n=1000 min=0.017ms median=0.026ms p99=0.055ms max=0.113ms
bench wire.potentials class=S n=1000 min=8.489ms median=9.011ms p99=12.993ms max=14.794ms
bench wire.commit_potentials class=S n=1000 min=8.512ms median=9.038ms p99=13.032ms max=14.840ms
folio: 50 pages, 8.8 devices per page on average with a representation on it
bench page.renumber.folio class=S n=1000 min=0.017ms median=0.018ms p99=0.043ms max=0.118ms
bench folio.renumber.derive class=S n=1000 min=0.069ms median=0.083ms p99=0.160ms max=0.230ms
bench folio.renumber.tags class=S n=1000 min=0.051ms median=0.064ms p99=0.119ms max=0.147ms
profile: 587 potentials, 3 affected, 153 wires in affected potentials
bench profile.change.all class=S n=1000 min=20.438ms median=21.795ms p99=34.335ms max=36.191ms
bench profile.change.affected class=S n=1000 min=0.307ms median=0.334ms p99=0.569ms max=0.717ms
memo first execution: potential_partition 12.094 ms, page_local_nets of 50 pages 2.742 ms
memo store after the first execution: page_local_nets memos=50 values=50 value_bytes=400 dep_entries=3550
memo store after the first execution: potential_partition memos=1 values=1 value_bytes=8 dep_entries=1881
oracle: memo.partition.hit 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.hit class=S n=1000 min=0.005ms median=0.006ms p99=0.011ms max=0.052ms [hit=1 ver=0 re=0 cut=0]
oracle: memo.partition.unrelated 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.unrelated.derive class=S n=1000 min=0.245ms median=0.271ms p99=0.420ms max=0.502ms
bench memo.partition.unrelated class=S n=1000 min=0.289ms median=0.336ms p99=0.572ms max=0.731ms [hit=0 ver=1 re=0 cut=0]
oracle: memo.partition.move 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.move.derive class=S n=1000 min=10.408ms median=11.087ms p99=17.225ms max=18.849ms
bench memo.partition.move class=S n=1000 min=10.483ms median=11.145ms p99=17.313ms max=18.920ms [hit=0 ver=0 re=1 cut=1]
oracle: memo.partition.wire 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.wire.derive class=S n=1000 min=10.326ms median=10.850ms p99=13.927ms max=15.484ms
bench memo.partition.wire class=S n=1000 min=10.361ms median=10.886ms p99=13.978ms max=15.536ms [hit=0 ver=0 re=1 cut=0]
oracle: memo.local_nets.move 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.local_nets.move.derive class=S n=1000 min=0.306ms median=0.336ms p99=0.480ms max=0.601ms
bench memo.local_nets.move class=S n=1000 min=0.324ms median=0.364ms p99=0.529ms max=0.658ms [hit=0 ver=49 re=1 cut=1]
oracle: memo.local_nets.wire 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.local_nets.wire.derive class=S n=1000 min=0.310ms median=0.339ms p99=0.481ms max=0.605ms
bench memo.local_nets.wire class=S n=1000 min=0.324ms median=0.354ms p99=0.517ms max=0.645ms [hit=0 ver=49 re=1 cut=0]

memory: model rows 1989723 bytes (1.9 MiB; estimator of ew-commands mirrored in ew_testkit::bench::model_bytes)
memory: undo history after one move step 464 bytes in 1 entries
memory: undo history after one wire step 56 bytes in 1 entries
memory: undo history after one folio.renumber step 675 bytes in 1 entries
memory: undo history after one memo.partition.unrelated step 273 bytes in 1 entries
memory: undo history after one memo.partition.move step 464 bytes in 1 entries
memory: undo history after one memo.partition.wire step 56 bytes in 1 entries
memory: undo history after one memo.local_nets.move step 464 bytes in 1 entries
memory: undo history after one memo.local_nets.wire step 56 bytes in 1 entries
memory: size_of DepKey<ew_derive::DepKey> 24 bytes, ew_memo::Rev 8 bytes, one dependency entry 32 bytes
memory: memo page_local_nets memos=50 values=50 value_bytes=400 dep_entries=3550 rdep_entries=0 dep_bytes_estimate=113600
memory: memo potential_partition memos=1 values=1 value_bytes=8 dep_entries=1881 rdep_entries=0 dep_bytes_estimate=60192
peak RSS: measured externally by the spike run

## G-SP spike, class S

| Family | Class | n | Median | p99 | Max | Budget | p99 / budget | Flags |
|---|---|---:|---:|---:|---:|---:|---:|---|
| open.session_from_records | S | 10 | 8.301 ms | 8.631 ms | 8.631 ms | 500.000 ms | 0.0× |  |
| connectivity.cold | S | 10 | 9.569 ms | 9.813 ms | 9.813 ms | — | — |  |
| connectivity.local_nets_all | S | 10 | 1.904 ms | 2.070 ms | 2.070 ms | — | — |  |
| connectivity.stitch_given_nets | S | 10 | 1.824 ms | 1.974 ms | 1.974 ms | — | — |  |
| connectivity.partition_given_stitch | S | 10 | 5.084 ms | 5.484 ms | 5.484 ms | — | — |  |
| connectivity.partition_given_stitch_tags | S | 10 | 4.006 ms | 4.191 ms | 4.191 ms | — | — |  |
| connectivity.device_tags_all | S | 10 | 0.886 ms | 1.002 ms | 1.002 ms | — | — |  |
| connectivity.wire_props_all | S | 10 | 11.942 ms | 12.613 ms | 12.613 ms | — | — |  |
| move.commit | S | 1000 | 0.044 ms | 0.096 ms | 0.117 ms | — | — |  |
| move.potentials | S | 1000 | 9.053 ms | 14.835 ms | 15.666 ms | — | — |  |
| move.commit_potentials | S | 1000 | 9.089 ms | 14.918 ms | 15.754 ms | 5.000 ms | 3.0× | over |
| wire.commit | S | 1000 | 0.026 ms | 0.055 ms | 0.113 ms | — | — |  |
| wire.potentials | S | 1000 | 9.011 ms | 12.993 ms | 14.794 ms | — | — |  |
| wire.commit_potentials | S | 1000 | 9.038 ms | 13.032 ms | 14.840 ms | 5.000 ms | 2.6× | over |
| page.renumber.folio | S | 1000 | 0.018 ms | 0.043 ms | 0.118 ms | 5.000 ms | 0.0× |  |
| folio.renumber.derive | S | 1000 | 0.083 ms | 0.160 ms | 0.230 ms | 5.000 ms | 0.0× |  |
| folio.renumber.tags | S | 1000 | 0.064 ms | 0.119 ms | 0.147 ms | — | — |  |
| profile.change.all | S | 1000 | 21.795 ms | 34.335 ms | 36.191 ms | 30.000 ms | 1.1× | over |
| profile.change.affected | S | 1000 | 0.334 ms | 0.569 ms | 0.717 ms | 30.000 ms | 0.0× |  |
| memo.partition.hit | S | 1000 | 0.006 ms | 0.011 ms | 0.052 ms | — | — | hit=1 ver=0 re=0 cut=0 |
| memo.partition.unrelated.derive | S | 1000 | 0.271 ms | 0.420 ms | 0.502 ms | — | — |  |
| memo.partition.unrelated | S | 1000 | 0.336 ms | 0.572 ms | 0.731 ms | 5.000 ms | 0.1× | hit=0 ver=1 re=0 cut=0 |
| memo.partition.move.derive | S | 1000 | 11.087 ms | 17.225 ms | 18.849 ms | — | — |  |
| memo.partition.move | S | 1000 | 11.145 ms | 17.313 ms | 18.920 ms | 5.000 ms | 3.5× | over, hit=0 ver=0 re=1 cut=1 |
| memo.partition.wire.derive | S | 1000 | 10.850 ms | 13.927 ms | 15.484 ms | — | — |  |
| memo.partition.wire | S | 1000 | 10.886 ms | 13.978 ms | 15.536 ms | 5.000 ms | 2.8× | over, hit=0 ver=0 re=1 cut=0 |
| memo.local_nets.move.derive | S | 1000 | 0.336 ms | 0.480 ms | 0.601 ms | — | — |  |
| memo.local_nets.move | S | 1000 | 0.364 ms | 0.529 ms | 0.658 ms | 5.000 ms | 0.1× | hit=0 ver=49 re=1 cut=1 |
| memo.local_nets.wire.derive | S | 1000 | 0.339 ms | 0.481 ms | 0.605 ms | — | — |  |
| memo.local_nets.wire | S | 1000 | 0.354 ms | 0.517 ms | 0.645 ms | 5.000 ms | 0.1× | hit=0 ver=49 re=1 cut=0 |
```

### A.2 Class M

```text
G-SP spike benchmarks (WP-S1.05)
class=M generator_version=2 seed=1 mode=full
counts: Bounds { pages: 300, devices: 2500, functions: 7500, connections: 15000, cps: 37178, terminals: 1000, channels: 500 }
records: 35884 (project row included), library documents: 25
samples: latency 1000 (warm-up 20; clamped to 100 when the warm-up median is over ten times the budget, full mode only), single operation 10 (warm-up 1)
filter: none
baseline model built in 102.425 ms
bench open.session_from_records class=M n=10 min=91.099ms median=92.721ms p99=95.824ms max=95.824ms
connectivity: 300 pages, 9080 local nets, stitch graph 9080 nodes and 1676 edges, 7455 potentials, 7455 signals
bench connectivity.cold class=M n=10 min=106.345ms median=107.568ms p99=118.290ms max=118.290ms
bench connectivity.local_nets_all class=M n=10 min=22.918ms median=23.043ms p99=23.919ms max=23.919ms
bench connectivity.stitch_given_nets class=M n=10 min=19.245ms median=19.670ms p99=28.429ms max=28.429ms
bench connectivity.partition_given_stitch class=M n=10 min=62.740ms median=63.510ms p99=75.566ms max=75.566ms
bench connectivity.partition_given_stitch_tags class=M n=10 min=52.298ms median=59.719ms p99=77.390ms max=77.390ms
bench connectivity.device_tags_all class=M n=10 min=9.543ms median=9.770ms p99=13.513ms max=13.513ms
bench connectivity.wire_props_all class=M n=10 min=320.233ms median=330.979ms p99=358.493ms max=358.493ms
bench move.commit class=M n=100 min=0.109ms median=0.117ms p99=0.152ms max=0.189ms [clamped]
bench move.potentials class=M n=100 min=111.982ms median=115.019ms p99=134.586ms max=140.079ms [clamped]
bench move.commit_potentials class=M n=100 min=112.099ms median=115.130ms p99=134.739ms max=140.203ms [clamped]
bench wire.commit class=M n=100 min=0.047ms median=0.050ms p99=0.067ms max=0.068ms [clamped]
bench wire.potentials class=M n=100 min=112.338ms median=114.787ms p99=123.853ms max=125.494ms [clamped]
bench wire.commit_potentials class=M n=100 min=112.389ms median=114.839ms p99=123.903ms max=125.554ms [clamped]
folio: 300 pages, 12.6 devices per page on average with a representation on it
bench page.renumber.folio class=M n=1000 min=0.035ms median=0.038ms p99=0.078ms max=0.106ms
bench folio.renumber.derive class=M n=1000 min=0.246ms median=0.269ms p99=0.449ms max=0.498ms
bench folio.renumber.tags class=M n=1000 min=0.209ms median=0.229ms p99=0.381ms max=0.407ms
profile: 7455 potentials, 17 affected, 1325 wires in affected potentials
bench profile.change.all class=M n=100 min=456.697ms median=466.426ms p99=545.186ms max=571.111ms [clamped]
bench profile.change.affected class=M n=1000 min=3.330ms median=3.647ms p99=8.824ms max=27.940ms
memo first execution: potential_partition 140.954 ms, page_local_nets of 300 pages 30.134 ms
memo store after the first execution: page_local_nets memos=300 values=300 value_bytes=2400 dep_entries=31500
memo store after the first execution: potential_partition memos=1 values=1 value_bytes=8 dep_entries=14983
oracle: memo.partition.hit 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.hit class=M n=1000 min=0.048ms median=0.056ms p99=0.211ms max=0.252ms [hit=1 ver=0 re=0 cut=0]
oracle: memo.partition.unrelated 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.unrelated.derive class=M n=1000 min=3.020ms median=3.205ms p99=10.561ms max=36.254ms
bench memo.partition.unrelated class=M n=1000 min=3.811ms median=4.043ms p99=15.238ms max=39.105ms [hit=0 ver=1 re=0 cut=0]
oracle: memo.partition.move 100 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.move.derive class=M n=100 min=135.184ms median=181.882ms p99=338.384ms max=381.926ms [clamped]
bench memo.partition.move class=M n=100 min=135.310ms median=182.057ms p99=338.600ms max=382.151ms [clamped] [hit=0 ver=0 re=1 cut=1]
oracle: memo.partition.wire 100 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.wire.derive class=M n=100 min=130.504ms median=168.515ms p99=233.415ms max=359.489ms [clamped]
bench memo.partition.wire class=M n=100 min=130.555ms median=168.581ms p99=233.479ms max=359.603ms [clamped] [hit=0 ver=0 re=1 cut=0]
oracle: memo.local_nets.move 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.local_nets.move.derive class=M n=1000 min=3.348ms median=3.562ms p99=4.616ms max=4.947ms
bench memo.local_nets.move class=M n=1000 min=3.408ms median=3.653ms p99=4.758ms max=5.103ms [hit=0 ver=299 re=1 cut=1]
oracle: memo.local_nets.wire 1000 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.local_nets.wire.derive class=M n=1000 min=3.327ms median=3.800ms p99=8.234ms max=16.605ms
bench memo.local_nets.wire class=M n=1000 min=3.373ms median=3.851ms p99=8.329ms max=16.699ms [hit=0 ver=299 re=1 cut=0]

memory: model rows 18107647 bytes (17.3 MiB; estimator of ew-commands mirrored in ew_testkit::bench::model_bytes)
memory: undo history after one move step 7792 bytes in 1 entries
memory: undo history after one wire step 56 bytes in 1 entries
memory: undo history after one folio.renumber step 675 bytes in 1 entries
memory: undo history after one memo.partition.unrelated step 273 bytes in 1 entries
memory: undo history after one memo.partition.move step 7792 bytes in 1 entries
memory: undo history after one memo.partition.wire step 56 bytes in 1 entries
memory: undo history after one memo.local_nets.move step 464 bytes in 1 entries
memory: undo history after one memo.local_nets.wire step 56 bytes in 1 entries
memory: size_of DepKey<ew_derive::DepKey> 24 bytes, ew_memo::Rev 8 bytes, one dependency entry 32 bytes
memory: memo page_local_nets memos=300 values=300 value_bytes=2400 dep_entries=31500 rdep_entries=0 dep_bytes_estimate=1008000
memory: memo potential_partition memos=1 values=1 value_bytes=8 dep_entries=14983 rdep_entries=0 dep_bytes_estimate=479456
peak RSS: measured externally by the spike run

## G-SP spike, class M

| Family | Class | n | Median | p99 | Max | Budget | p99 / budget | Flags |
|---|---|---:|---:|---:|---:|---:|---:|---|
| open.session_from_records | M | 10 | 92.721 ms | 95.824 ms | 95.824 ms | 2000.000 ms | 0.0× |  |
| connectivity.cold | M | 10 | 107.568 ms | 118.290 ms | 118.290 ms | — | — |  |
| connectivity.local_nets_all | M | 10 | 23.043 ms | 23.919 ms | 23.919 ms | — | — |  |
| connectivity.stitch_given_nets | M | 10 | 19.670 ms | 28.429 ms | 28.429 ms | — | — |  |
| connectivity.partition_given_stitch | M | 10 | 63.510 ms | 75.566 ms | 75.566 ms | — | — |  |
| connectivity.partition_given_stitch_tags | M | 10 | 59.719 ms | 77.390 ms | 77.390 ms | — | — |  |
| connectivity.device_tags_all | M | 10 | 9.770 ms | 13.513 ms | 13.513 ms | — | — |  |
| connectivity.wire_props_all | M | 10 | 330.979 ms | 358.493 ms | 358.493 ms | — | — |  |
| move.commit | M | 100 | 0.117 ms | 0.152 ms | 0.189 ms | — | — | clamped |
| move.potentials | M | 100 | 115.019 ms | 134.586 ms | 140.079 ms | — | — | clamped |
| move.commit_potentials | M | 100 | 115.130 ms | 134.739 ms | 140.203 ms | 5.000 ms | 26.9× | clamped, over |
| wire.commit | M | 100 | 0.050 ms | 0.067 ms | 0.068 ms | — | — | clamped |
| wire.potentials | M | 100 | 114.787 ms | 123.853 ms | 125.494 ms | — | — | clamped |
| wire.commit_potentials | M | 100 | 114.839 ms | 123.903 ms | 125.554 ms | 5.000 ms | 24.8× | clamped, over |
| page.renumber.folio | M | 1000 | 0.038 ms | 0.078 ms | 0.106 ms | 5.000 ms | 0.0× |  |
| folio.renumber.derive | M | 1000 | 0.269 ms | 0.449 ms | 0.498 ms | 5.000 ms | 0.1× |  |
| folio.renumber.tags | M | 1000 | 0.229 ms | 0.381 ms | 0.407 ms | — | — |  |
| profile.change.all | M | 100 | 466.426 ms | 545.186 ms | 571.111 ms | 30.000 ms | 18.2× | clamped, over |
| profile.change.affected | M | 1000 | 3.647 ms | 8.824 ms | 27.940 ms | 30.000 ms | 0.3× |  |
| memo.partition.hit | M | 1000 | 0.056 ms | 0.211 ms | 0.252 ms | — | — | hit=1 ver=0 re=0 cut=0 |
| memo.partition.unrelated.derive | M | 1000 | 3.205 ms | 10.561 ms | 36.254 ms | — | — |  |
| memo.partition.unrelated | M | 1000 | 4.043 ms | 15.238 ms | 39.105 ms | 5.000 ms | 3.0× | over, hit=0 ver=1 re=0 cut=0 |
| memo.partition.move.derive | M | 100 | 181.882 ms | 338.384 ms | 381.926 ms | — | — | clamped |
| memo.partition.move | M | 100 | 182.057 ms | 338.600 ms | 382.151 ms | 5.000 ms | 67.7× | clamped, over, hit=0 ver=0 re=1 cut=1 |
| memo.partition.wire.derive | M | 100 | 168.515 ms | 233.415 ms | 359.489 ms | — | — | clamped |
| memo.partition.wire | M | 100 | 168.581 ms | 233.479 ms | 359.603 ms | 5.000 ms | 46.7× | clamped, over, hit=0 ver=0 re=1 cut=0 |
| memo.local_nets.move.derive | M | 1000 | 3.562 ms | 4.616 ms | 4.947 ms | — | — |  |
| memo.local_nets.move | M | 1000 | 3.653 ms | 4.758 ms | 5.103 ms | 5.000 ms | 1.0× | hit=0 ver=299 re=1 cut=1 |
| memo.local_nets.wire.derive | M | 1000 | 3.800 ms | 8.234 ms | 16.605 ms | — | — |  |
| memo.local_nets.wire | M | 1000 | 3.851 ms | 8.329 ms | 16.699 ms | 5.000 ms | 1.7× | over, hit=0 ver=299 re=1 cut=0 |
```

### A.3 Class L

```text
G-SP spike benchmarks (WP-S1.05)
class=L generator_version=2 seed=1 mode=full
counts: Bounds { pages: 1500, devices: 12000, functions: 36000, connections: 70000, cps: 171330, terminals: 4800, channels: 2400 }
records: 170319 (project row included), library documents: 23
samples: latency 1000 (warm-up 20; clamped to 100 when the warm-up median is over ten times the budget, full mode only), single operation 10 (warm-up 1)
filter: none
baseline model built in 726.668 ms
bench open.session_from_records class=L n=10 min=553.118ms median=705.565ms p99=1017.075ms max=1017.075ms
connectivity: 1500 pages, 41548 local nets, stitch graph 41548 nodes and 8211 edges, 33589 potentials, 33589 signals
bench connectivity.cold class=L n=10 min=734.074ms median=1107.899ms p99=2394.452ms max=2394.452ms
bench connectivity.local_nets_all class=L n=10 min=110.292ms median=111.928ms p99=118.523ms max=118.523ms
bench connectivity.stitch_given_nets class=L n=10 min=95.981ms median=98.020ms p99=107.483ms max=107.483ms
bench connectivity.partition_given_stitch class=L n=10 min=310.554ms median=318.269ms p99=353.260ms max=353.260ms
bench connectivity.partition_given_stitch_tags class=L n=10 min=262.201ms median=267.816ms p99=286.483ms max=286.483ms
bench connectivity.device_tags_all class=L n=10 min=50.072ms median=50.787ms p99=58.634ms max=58.634ms
bench connectivity.wire_props_all class=L n=10 min=7054.842ms median=8095.835ms p99=8449.263ms max=8449.263ms
bench move.commit class=L n=100 min=0.108ms median=0.116ms p99=0.198ms max=0.227ms [clamped]
bench move.potentials class=L n=100 min=565.754ms median=586.866ms p99=974.096ms max=1038.428ms [clamped]
bench move.commit_potentials class=L n=100 min=565.871ms median=586.978ms p99=974.255ms max=1038.547ms [clamped]
bench wire.commit class=L n=100 min=0.049ms median=0.052ms p99=0.065ms max=0.079ms [clamped]
bench wire.potentials class=L n=100 min=566.777ms median=581.040ms p99=656.230ms max=688.913ms [clamped]
bench wire.commit_potentials class=L n=100 min=566.827ms median=581.096ms p99=656.282ms max=688.971ms [clamped]
folio: 1500 pages, 12.0 devices per page on average with a representation on it
bench page.renumber.folio class=L n=1000 min=0.115ms median=0.120ms p99=0.228ms max=0.314ms
bench folio.renumber.derive class=L n=1000 min=1.029ms median=1.078ms p99=1.707ms max=2.073ms
bench folio.renumber.tags class=L n=1000 min=0.913ms median=0.953ms p99=1.430ms max=1.759ms
profile: 33589 potentials, 84 affected, 6352 wires in affected potentials
bench profile.change.all class=L n=100 min=7533.755ms median=7726.874ms p99=10882.561ms max=13522.843ms [clamped]
bench profile.change.affected class=L n=1000 min=15.828ms median=18.198ms p99=56.370ms max=80.439ms
memo first execution: potential_partition 701.497 ms, page_local_nets of 1500 pages 147.923 ms
memo store after the first execution: page_local_nets memos=1500 values=1500 value_bytes=12000 dep_entries=149500
memo store after the first execution: potential_partition memos=1 values=1 value_bytes=8 dep_entries=71246
oracle: memo.partition.hit 100 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.hit class=L n=1000 min=0.449ms median=0.481ms p99=0.716ms max=0.844ms [hit=1 ver=0 re=0 cut=0]
oracle: memo.partition.unrelated 100 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.unrelated.derive class=L n=1000 min=16.069ms median=16.433ms p99=18.365ms max=19.817ms
bench memo.partition.unrelated class=L n=1000 min=20.301ms median=20.985ms p99=26.494ms max=28.060ms [hit=0 ver=1 re=0 cut=0]
oracle: memo.partition.move 10 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.move.derive class=L n=100 min=687.834ms median=697.732ms p99=719.167ms max=719.276ms [clamped]
bench memo.partition.move class=L n=100 min=687.948ms median=697.853ms p99=719.296ms max=719.394ms [clamped] [hit=0 ver=0 re=1 cut=1]
oracle: memo.partition.wire 10 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.partition.wire.derive class=L n=100 min=661.758ms median=671.555ms p99=687.554ms max=688.238ms [clamped]
bench memo.partition.wire class=L n=100 min=661.820ms median=671.616ms p99=687.614ms max=688.296ms [clamped] [hit=0 ver=0 re=1 cut=0]
oracle: memo.local_nets.move 100 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.local_nets.move.derive class=L n=1000 min=18.164ms median=18.717ms p99=21.091ms max=22.939ms
bench memo.local_nets.move class=L n=1000 min=18.273ms median=18.830ms p99=21.237ms max=23.079ms [hit=0 ver=1499 re=1 cut=1]
oracle: memo.local_nets.wire 100 comparisons, 0 mismatches (20 warm-up rounds under Oracle::Every)
bench memo.local_nets.wire.derive class=L n=1000 min=18.128ms median=18.748ms p99=20.772ms max=21.488ms
bench memo.local_nets.wire class=L n=1000 min=18.174ms median=18.796ms p99=20.824ms max=21.538ms [hit=0 ver=1499 re=1 cut=0]

memory: model rows 85619460 bytes (81.7 MiB; estimator of ew-commands mirrored in ew_testkit::bench::model_bytes)
memory: undo history after one move step 7120 bytes in 1 entries
memory: undo history after one wire step 56 bytes in 1 entries
memory: undo history after one folio.renumber step 676 bytes in 1 entries
memory: undo history after one memo.partition.unrelated step 273 bytes in 1 entries
memory: undo history after one memo.partition.move step 7120 bytes in 1 entries
memory: undo history after one memo.partition.wire step 56 bytes in 1 entries
memory: undo history after one memo.local_nets.move step 7120 bytes in 1 entries
memory: undo history after one memo.local_nets.wire step 56 bytes in 1 entries
memory: size_of DepKey<ew_derive::DepKey> 24 bytes, ew_memo::Rev 8 bytes, one dependency entry 32 bytes
memory: memo page_local_nets memos=1500 values=1500 value_bytes=12000 dep_entries=149500 rdep_entries=0 dep_bytes_estimate=4784000
memory: memo potential_partition memos=1 values=1 value_bytes=8 dep_entries=71246 rdep_entries=0 dep_bytes_estimate=2279872
peak RSS: measured externally by the spike run

## G-SP spike, class L

| Family | Class | n | Median | p99 | Max | Budget | p99 / budget | Flags |
|---|---|---:|---:|---:|---:|---:|---:|---|
| open.session_from_records | L | 10 | 705.565 ms | 1017.075 ms | 1017.075 ms | 5000.000 ms | 0.2× |  |
| connectivity.cold | L | 10 | 1107.899 ms | 2394.452 ms | 2394.452 ms | — | — |  |
| connectivity.local_nets_all | L | 10 | 111.928 ms | 118.523 ms | 118.523 ms | — | — |  |
| connectivity.stitch_given_nets | L | 10 | 98.020 ms | 107.483 ms | 107.483 ms | — | — |  |
| connectivity.partition_given_stitch | L | 10 | 318.269 ms | 353.260 ms | 353.260 ms | — | — |  |
| connectivity.partition_given_stitch_tags | L | 10 | 267.816 ms | 286.483 ms | 286.483 ms | — | — |  |
| connectivity.device_tags_all | L | 10 | 50.787 ms | 58.634 ms | 58.634 ms | — | — |  |
| connectivity.wire_props_all | L | 10 | 8095.835 ms | 8449.263 ms | 8449.263 ms | — | — |  |
| move.commit | L | 100 | 0.116 ms | 0.198 ms | 0.227 ms | — | — | clamped |
| move.potentials | L | 100 | 586.866 ms | 974.096 ms | 1038.428 ms | — | — | clamped |
| move.commit_potentials | L | 100 | 586.978 ms | 974.255 ms | 1038.547 ms | 10.000 ms | 97.4× | clamped, over |
| wire.commit | L | 100 | 0.052 ms | 0.065 ms | 0.079 ms | — | — | clamped |
| wire.potentials | L | 100 | 581.040 ms | 656.230 ms | 688.913 ms | — | — | clamped |
| wire.commit_potentials | L | 100 | 581.096 ms | 656.282 ms | 688.971 ms | 10.000 ms | 65.6× | clamped, over |
| page.renumber.folio | L | 1000 | 0.120 ms | 0.228 ms | 0.314 ms | 5.000 ms | 0.0× |  |
| folio.renumber.derive | L | 1000 | 1.078 ms | 1.707 ms | 2.073 ms | 5.000 ms | 0.3× |  |
| folio.renumber.tags | L | 1000 | 0.953 ms | 1.430 ms | 1.759 ms | — | — |  |
| profile.change.all | L | 100 | 7726.874 ms | 10882.561 ms | 13522.843 ms | 60.000 ms | 181.4× | clamped, over |
| profile.change.affected | L | 1000 | 18.198 ms | 56.370 ms | 80.439 ms | 60.000 ms | 0.9× |  |
| memo.partition.hit | L | 1000 | 0.481 ms | 0.716 ms | 0.844 ms | — | — | hit=1 ver=0 re=0 cut=0 |
| memo.partition.unrelated.derive | L | 1000 | 16.433 ms | 18.365 ms | 19.817 ms | — | — |  |
| memo.partition.unrelated | L | 1000 | 20.985 ms | 26.494 ms | 28.060 ms | 10.000 ms | 2.6× | over, hit=0 ver=1 re=0 cut=0 |
| memo.partition.move.derive | L | 100 | 697.732 ms | 719.167 ms | 719.276 ms | — | — | clamped |
| memo.partition.move | L | 100 | 697.853 ms | 719.296 ms | 719.394 ms | 10.000 ms | 71.9× | clamped, over, hit=0 ver=0 re=1 cut=1 |
| memo.partition.wire.derive | L | 100 | 671.555 ms | 687.554 ms | 688.238 ms | — | — | clamped |
| memo.partition.wire | L | 100 | 671.616 ms | 687.614 ms | 688.296 ms | 10.000 ms | 68.8× | clamped, over, hit=0 ver=0 re=1 cut=0 |
| memo.local_nets.move.derive | L | 1000 | 18.717 ms | 21.091 ms | 22.939 ms | — | — |  |
| memo.local_nets.move | L | 1000 | 18.830 ms | 21.237 ms | 23.079 ms | 10.000 ms | 2.1× | over, hit=0 ver=1499 re=1 cut=1 |
| memo.local_nets.wire.derive | L | 1000 | 18.748 ms | 20.772 ms | 21.488 ms | — | — |  |
| memo.local_nets.wire | L | 1000 | 18.796 ms | 20.824 ms | 21.538 ms | 10.000 ms | 2.1× | over, hit=0 ver=1499 re=1 cut=0 |
```

### A.4 Class XL

XL: run in progress at the time of writing (started 03:04 local, class XL); results appended in a follow-up commit.

### A.5 Run log (commands, wall times, peak working set)

The evidence binary was built from commit `1a5c38f` of the lane branch (on top of `ed7d948`); the same change is
`3a94f56` on `claude/m0-foundations`. Between the two bases, `claude/m0-foundations` adds an O(1) model clone in
`Session::execute` and the cross-reference additions of WP-S1.08; the measured code paths (the commands, the
connectivity queries, the folio renumber and the tags) do not depend on either.

Build: `cargo bench -p ew-testkit --bench spike --no-run --locked` at commit `1a5c38f` (executable
`target/release/deps/spike-4ecf98b7aaa71494`). One class per process, sequential, in full mode; peak working set
polled every 200 ms from outside the process.

| Class | Command | Wall time (s) | Peak working set (MiB) | Exit |
|---|---|---:|---:|---:|
| S | `EW_BENCH_CLASS=S target/release/deps/spike-4ecf98b7aaa71494 --bench` | 126.6 | 46 | 0 |
| M | `EW_BENCH_CLASS=M target/release/deps/spike-4ecf98b7aaa71494 --bench` | 730.3 | 380 | 0 |
| L | `EW_BENCH_CLASS=L target/release/deps/spike-4ecf98b7aaa71494 --bench` | 2,862.0 | 1,826 | 0 |
| XL | `EW_BENCH_CLASS=XL EW_SPIKE_ONLY=open,connectivity.cold,connectivity.local_nets_all,connectivity.stitch_given_nets,connectivity.partition_given_stitch,connectivity.device_tags_all,move,wire,page.renumber,folio,profile.change.affected,memo target/release/deps/spike-4ecf98b7aaa71494 --bench` | in progress | in progress | — |

XL leaves out `connectivity.wire_props_all` and `profile.change.all`: both are super-linear (8.1 s and 7.7 s per
run at L), so 120 samples at XL would take several hours, and both are decided at M and L already. The XL run uses
the bench of the report commit, whose only change is that `EW_SPIKE_ONLY` filters these rows individually.

The machine was not dedicated: other development sessions on the same desktop compiled and tested during parts of
the runs. Medians are robust to that; p99 tails and the L single-operation medians are inflated (section 5,
reading 5). No decision in this report depends on a margin that the noise could close.
