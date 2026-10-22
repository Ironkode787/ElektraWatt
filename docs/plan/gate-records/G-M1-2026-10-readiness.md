# G-M1 readiness run

**Status:** Final (readiness run; the gate is not convened; the delegated rulings of 2026-10-22 are recorded in §8 and the addendum; the owner rulings of 2026-10-22 in the second addendum; the cycle-18 delivery in the addenda of §6 and §10; the cycle-19 delivery in the addendum of §6) · **Date:** 2026-10-22 · **Gate:** G-M1 Spine exit ·
**Commit:** de23b55 (the cycle-16 delivery of `claude/m0-foundations`, equal to `main`) · **Scenario:** the
cycle-based plan of record ([global plan §3.8](../01-global-plan.md#38-cycle-based-progress-view-plan-of-record-since-2026-10-20),
decision log ID-271) · **Part of:** ElektraWatt plan, gate records

**Purpose.** This is the readiness run of the [gates](../03-gates.md) §3.3 for G-M1: every entry criterion and every
exit criterion of [§5.3](../03-gates.md#53-g-m1-spine-exit-m1) against the state at de23b55, with the evidence, a
status, an owner and the rule that dates it; the owner-side items and what exactly the owner must do; the development
items left and their gate relevance; and, plainly, what is not ready. The gate pack draft built on it is
[G-M1-2026-10-pack-draft.md](G-M1-2026-10-pack-draft.md). Decision log ID-419 to ID-426 record the run.

---

## Summary for decision makers

**Verdict: not ready.** G-M1 cannot be held with a chance of passing. Of its 27 exit criteria, 15 are hard (H);
4 are met and 11 are open. Of the 11, five are owner-side only (LG-1, LG-2, ADR-0030 with counsel's opinion and the
enrolments, a second code owner, the PD-11 re-plan), four need both the owner and development (the BM-class
performance runner of DEC-034 for AC-17, the XL budgets and the seven green nights; the first PNG approval FT-10 with
a veraPDF job for AC-02 and AC-13), one is development only (the headless container job behind G-M1-01) and one needs
a ruling (D-06's property-grid half, a UI entry gated at a headless gate). Six of the 10 must (M) criteria are open as
well, more than the three that a *go with conditions* allows. G-M0 has not been held either.

**The headless engineering of M1 is close to done.** The acceptance runner reports 93 entries: 87 pass, 0 fail,
1 not built (D-06's property grid), 5 not applicable, with 0 oracle mismatches, and the gate's acceptance test proves
the committed report current at de23b55. What is missing on the development side is gate evidence rather than
features: CI jobs that §8.5 of the gates requires and that do not exist yet (veraPDF, the headless container, coverage,
the deep nightly, the fuzz targets), the G-SP re-test, and a short list of follow-up tasks. At the observed throughput
that is about two cycles after cycle 17 (an estimate, not a commitment).

**What bounds the date is the owner side.** The legal gates, the counsel opinions, the enrolments, the BM-class
runner and its seven nights, the pilot's approvals and answers do not move at the cycle pace.

**Proposed date rule** (ID-421, for the owner to adopt): G-M1 is held within two weeks after the later of (a) the
cycle delivery at which a repeated readiness run finds no development-side H criterion open and at most three M
criteria open, and (b) the day the last owner-side H item is on file, which for G-M1-12 is at the earliest seven
nights after the BM-class runner is commissioned. G-M0 is held first, or in the same meeting (§2.4 of the gates).

**What the owner is asked now:** the eight rulings of §8 and the owner-side items of §5, in the order given there.

**Addendum (rulings of 2026-10-22, delegated; decision log ID-419 to ID-425).** R1, R3, R4, R6, R7 and R8 are
accepted and R2 adopted, the project owner confirming or amending the date rule at G-M0; R5 is not ruled by
delegation and stays with the project owner. Effect on §4: G-M1-08 is met once the runner's gate for D-06's
property-grid entry is applied at the cycle-17 delivery; G-M1-24 is restated and stays open for the owner's re-plan
decision at T; G-M1-25 is not applicable. The rows below are the snapshot at the run; the next repeated readiness run
starts from these rulings.

**Addendum (owner rulings of 2026-10-22; decision log ID-450 to ID-455).** The project owner ruled in chat, not by
delegation: (1) PD-1 and PD-3 are adopted, the cycle-based plan of record is the plan of record, the human-team
months stay a reference only and there is no fourth developer, so G-M0-01 and G-M0-02 are met (ID-450); (2) the date
rule of §9 is adopted by the owner (ID-451); (3) a second named code owner is not required, so G-M1-16 is not
applicable, O7 is removed and R5 is closed (ID-452); (4) no funding is pursued, so G-M1-26 is not applicable (ID-453);
(5) "Regarding all the licensing, and legal items, this is no longer relevant": LG-1, LG-2 (with the counsel brief and
ID-383, now owner-accepted as a distribution fact), LG-3, LG-5, ADR-0030 with counsel's CRA opinion and the
enrolments are no longer gate items, so G-M0-08 and G-M1-18 to G-M1-21 are not applicable and O1, O3, O4, O5 and O13
are dropped (ID-454). **Recount after these rulings** (ID-455): G-M1 has 11 applicable hard criteria, of which 5 are
met (G-M1-03, -04, -06, -07, -08) and 6 open: G-M1-01 (development, FT-295), G-M1-02 (the pilot's PNG approval FT-10
and FT-294), G-M1-05, -11 and -12 (the BM-class runner and the re-test) and G-M1-24 (the PD-11 re-plan at T); of 7
applicable must criteria 1 is met, 2 are met with conditions and 4 are open (G-M1-13, -17, -22, -23), one more than a
*go with conditions* allows. G-M0 has 7 applicable hard criteria, of which 4 are met (G-M0-01, -02, -04, -06) and 3
open (G-M0-03 PD-2, G-M0-05 the boundary demonstration, G-M0-07 the two ADR acceptances). The verdict stays **not
ready**; what bounds G-M1 on the owner side is now the BM-class runner, the pilot's approvals and answers, PD-2 and
PD-14, and the re-plan at T. The rows below carry the per-row status edits of these rulings; the narrative is the
record of the run.

---

**Contents**
1. [Scope, evidence and method](#1-scope-evidence-and-method)
2. [Entry criteria](#2-entry-criteria)
3. [G-M0 comes first](#3-g-m0-comes-first)
4. [Exit criteria](#4-exit-criteria)
5. [Owner-side items](#5-owner-side-items)
6. [Development items left](#6-development-items-left)
7. [What is not ready](#7-what-is-not-ready)
8. [Rulings asked of the owner](#8-rulings-asked-of-the-owner)
9. [The proposed date rule](#9-the-proposed-date-rule)
10. [Cycle-17 rows to confirm at delivery](#10-cycle-17-rows-to-confirm-at-delivery)
11. [Sources](#sources)

---

## 1. Scope, evidence and method

**State assessed.** Commit de23b55, the cycle-16 delivery. Its gates are green on Windows and Linux (2,612 and 2,623
tests, clippy, doctests, rustdoc and the 16 steps of `cargo xtask ci`), as the integrating session reported; the run
IDs go into the pack. Cycle 17's lanes A (the typed `Revision`, FT-165) and B (the fuzzer's coverage FT-279 and its
nightly form FT-272) deliver concurrently; their rows are marked *in cycle 17, to confirm at delivery*, never as done.

**Evidence used.** The committed headless acceptance report
[`golden/J-E-2416/expected/acceptance_headless.md`](../../../golden/J-E-2416/expected/acceptance_headless.md)
(93 entries; an ordinary test runs the runner once and compares its output with the committed file, so the report is
current at de23b55); [golden project §22](../../design/20-golden-project.md#22-milestone-gates) and the golden README;
the [global plan §3.8](../01-global-plan.md#38-cycle-based-progress-view-plan-of-record-since-2026-10-20); the work
packages and the follow-up tasks of
[WBS Appendix B](../02-work-packages.md#appendix-b-follow-up-tasks-from-the-implementation-cycles); the
[decision log](../06-decision-log.md); [quality §5](../../design/18-quality-security-release.md#5-ci-pipeline) and
the workflows in `.github/workflows/`; [governance §3.3](../../design/19-open-source-governance.md#33-legal-and-organisational-gates);
the ADR headers; `.github/CODEOWNERS`. This lane ran no build: the two repository checks it ran are
`cargo xtask check-anonymisation` and `cargo xtask check-governance-files`, both green at de23b55 with this run's
changes.

**Procedure.** §3.3 of the gates puts the readiness run at T − 6 weeks. Under the cycle-based plan of record a
gate is due when its content is done (ID-271), so T is not known yet; this run is made at the delivery that closed
the headless acceptance list, and §9 proposes how the §3.3 calendar maps onto cycle deliveries.

**Where this lives, and why.** In `docs/plan/gate-records/`, the location that open question 3 of the gates
recommends for gate records (named `<gate>-<yyyy-mm>`), not as a numbered plan document or an appendix of the gates.
The numbered documents are standing documents that each cycle keeps current; a readiness run is a dated snapshot
that will be repeated and then superseded by the gate record, and an appendix would put a stale snapshot inside the
standing gate definitions. The owner still confirms the location (ID-419).

**Words used.** *Status:* met; met with conditions (met in substance, a named item still to close); open; not
applicable. *Owner:* development (agents, closed in cycles) or owner side (the project owner, the pilot user, counsel,
the steward or the repository owner; not at the cycle pace). *Gate relevance* of a left item: **blocking** (an H
criterion, or an M criterion beyond the three a *go with conditions* allows), **condition** (an M criterion that may
close within four weeks of a *go with conditions*), **follow-up** (no G-M1 criterion depends on it). *Date rule:*
**C17** confirm at the cycle-17 delivery; **Cn** closes at a later cycle delivery; **Pack** in the pack at T − 7 days;
**Owner** on file before the pack (T − 7 days); **BM+7** seven consecutive green nights after the BM-class runner is
commissioned; **Milestone** in the milestone run of the G-M1 delivery (full suite and performance, testing policy of
`CLAUDE.md`); **Ruling** closed by an owner ruling before the scope freeze; **T** at the review.

## 2. Entry criteria

The entry criteria decide whether the review can be *held* (§5 of the gates).

| # | Entry criterion | Evidence (at de23b55) | Status | Owner | Date rule |
|---|---|---|---|---|---|
| E1 | All 22 M1 WPs done or listed with a dated plan; the readiness run made | 17 of 22 delivered in whole or in large part ([global plan §3.8](../01-global-plan.md#38-cycle-based-progress-view-plan-of-record-since-2026-10-20)). In part: WP-S1.05 (the G-SP re-test), WP-S6.02 (FT-271, FT-272, FT-279), WP-S6.03 (the BM-class runner, FT-287 to FT-291), WP-S0.06 (FT-281 to FT-285). Not started: WP-S0.04 (owner side). §6 lists each development item with a cycle plan; this document is the readiness run | Met with conditions: the owner-side items carry no date until the owner gives one | Development; owner side for WP-S0.04 | Owner |
| E2 | The G-SP decision (PD-8) is closed and ADR-0002 amended | G-SP held 2026-10-20 with the outcome Recycle, PD-8 closed (ID-218; ADR-0002 amendment 4); the recycle's engine is built (amendments 5 and 6) | Met | — | — |
| E3 | Golden data A, B and C and expected outputs exist (WP-S7.01); the pilot has seen the golden tables | Golden A: tables, expected rows and 36 of 36 pages; Golden B: two command logs, AC-18 headless; Golden C: migration report and fix script, gated at G-M3 (ID-302); [golden §22](../../design/20-golden-project.md#22-milestone-gates), golden README. That the pilot has seen the tables is not on file | Met with conditions | Owner side (pilot user) | Owner |

## 3. G-M0 comes first

G-M0 has not been held ([global plan §3.8](../01-global-plan.md#38-cycle-based-progress-view-plan-of-record-since-2026-10-20), M0
row). G-M1's legal criteria build on G-M0's (LG-5 is a G-M0 H criterion and its refusal is KT-2; LG-1 is dated at
G-M0 and hard at G-M1), so G-M1 is not held before G-M0. G-M0 needs no development: its open H items are owner
decisions and documents.

| G-M0 H criterion | State at de23b55 | Owner action |
|---|---|---|
| G-M0-01 PD-1 recorded | Met: the owner adopted PD-1 on 2026-10-22 (ID-450) | — |
| G-M0-02 PD-3 recorded | Met: the owner recorded PD-3, no developer 4, on 2026-10-22 (ID-450) | — |
| G-M0-03 PD-2 recorded | Open (PQ-40 unasked) | Confirm lever (b) with the pilot |
| G-M0-04 CI on three OSes | Met: `ci.yml` (lint, test-linux, test-windows, test-macos, supply-chain), jobs of about 21 to 23 minutes (doc 18 §3.14); branch protection is a repository setting | Confirm branch protection (with FT-284) |
| G-M0-05 Licence boundary demonstrated | The check runs in the supply-chain job and `reuse lint` in `legal-docs`; the failing test pull request of the criterion is not on file | Development opens the demonstration pull request at the owner's go |
| G-M0-06 Anonymisation guard | Met (`cargo xtask check-anonymisation` in `legal-docs`) | — |
| G-M0-07 ADR-0027 and ADR-0028 Accepted | Both Proposed; their recommendations are in force as defaults since 2026-10-18 (ID-1, ID-2) | The pilot user accepts both (or changes them) |
| G-M0-08 LG-5 signed | Not applicable: dropped by owner ruling 2026-10-22 (ID-454) | — |

**Recommendation:** hold G-M0 as soon as these decisions are taken; it is a two-hour meeting with no development
dependency, and it unblocks the legal track that bounds G-M1. A combined meeting (§2.4 of the gates) is the fallback
when both are ready together; each gate keeps its own record.

## 4. Exit criteria

IDs, types and wording as in [§5.3](../03-gates.md#53-g-m1-spine-exit-m1); the criterion column is shortened.

| ID | Type | Criterion | Evidence (at de23b55) | Status | Owner | Date rule |
|---|---|---|---|---|---|---|
| G-M1-01 | H | `ew export pdf golden/J-E-2416.ew` writes the folio set in a container with no display, no GPU, no system fonts, empty library directories | Runner entry AC-15 "the shipped PDF export over path (c)": 36 pages, the digest of `expected/pdf/sha256.txt`, two bundled faces and nothing else; `bin/ew/tests/cli.rs` runs the command on one and four threads with no display, no starter content and empty user directories (golden README, "Expected page output"). The CI-PR-05 container job does not exist, and the hosted runner has system fonts. Folio 911 is printed as a placeholder (FT-207) | Open (evidence) | Development (FT-295) | Cn |
| G-M1-02 | H | AC-01 … AC-13 on Golden A | Every AC-01 … AC-13 entry passes except two `not_applicable` rows: AC-02's raster comparison (off headless; the PNGs are candidates awaiting FT-10) and AC-13's veraPDF cross-check (the report says "the CI job that has it runs the cross-check", but no workflow installs veraPDF; krilla's in-process PDF/A-2b validation passes) | Open | Owner side (FT-10) and development (FT-294, the raster run after approval) | Owner; Cn |
| G-M1-03 | H | AC-14 (Linux): byte-identical across serial and parallel export and shuffled record order | Both AC-14 entries pass (shuffle seed 0x5eedac14; 36 SVG files; 2 and 8 threads); the committed report is compared in every OS test job | Met | Development | Milestone (re-run) |
| G-M1-04 | H | AC-15 build paths agree; AC-16 scripted edits with the oracle on | AC-15: (a) = (c) by `tests/golden_fixture.rs` (by design not a runner row), SQL over (a) and (c), the PDF over (c) and path (b) pass (606 commands; only the 2 revisions unwritten, FT-165); AC-16: five entries pass, 2,037 keys of 65 memoised families equal their cold values after each edit | Met; FT-165 (lane A) empties path (b)'s `unwritten` list | Development | C17 |
| G-M1-05 | H | AC-17 on BM: open ≤ 0.5 s, PDF export ≤ 3 s, full check ≤ 2 s | `not_applicable` (BM runner, `cargo xtask acceptance --milestone`); no BM-class runner exists (DEC-034) | Open | Owner side (DEC-034) and development (the milestone run) | BM+7; Milestone |
| G-M1-06 | H | AC-18 (CLI) steps B1 and B2; AC-20 the D-01 replay | Five AC-18 and two AC-20 entries pass | Met | Development | — |
| G-M1-07 | H | AC-39 the anonymisation guard | `cargo xtask check-anonymisation` green at de23b55 (this run) and in the `legal-docs` job; by design not a runner row | Met | Development | — |
| G-M1-08 | H | Every D-row gated M1 passes (26 rows, AC-24) | All 26 rows are in the report; 25 pass in every entry; D-06 passes three entries (one description on every page, the per-page override, the info finding EW-CORE-023) and its fourth, "shown in the property grid" (FR-UI-009), is `not_built` | Open; met if ruling R1 moves the property-grid half to G-M2 | Owner side (ruling) | Ruling |
| G-M1-09 | M | The acceptance SQL/CSV pairs pass with the oracle on; every golden-table change is acknowledged by the pilot | The six SQL entries of AC-08 … AC-11 and the AC-15 SQL entries over paths (a) and (c) pass. No acknowledgement is on file: FT-29, FT-40 and FT-63 wait for it (PD-15), and no change list has been compiled | Met with conditions | Owner side (pilot user); development (FT-300, the list) | Owner; Cn |
| G-M1-10 | M | SC-1 headless: criteria 1 … 7 and 9 of the reference analysis §4.15; SC-6 determinism on Linux | Mapping of the passing rows: 1 pages → AC-03, AC-04; 2 frame → AC-01; 3 cross-references → AC-05; 4 tags → AC-06, D-03; 5 BOM → AC-07 (both modes); 6 terminals → AC-08, D-11, D-12; 7 wire data → AC-09, D-08; 9 PDF → AC-13, D-07, D-33. SC-6 on Linux → AC-14 | Met | Development | — |
| G-M1-11 | H | XL opens in ≤ 20 s; commit p99 ≤ 5 ms at M and ≤ 20 ms at XL (NFR-PERF-03, -05) | Not measured since G-SP's recycle (ID-218: cold connectivity missed NFR-PERF-05 by 2.6–3× at S, 25–27× at M; the XL open-time proxy 2.49 s). The recycle's engine is built (WP-S1.07 parts 1 and 2); the re-test is open (FT-26, FT-114 to FT-119); the nightly measures class S only, against an informational baseline (ID-411, ID-414) | Open | Development (the re-test, the XL benches, FT-288) and owner side (DEC-034) | BM+7 |
| G-M1-12 | H | The gate runs nightly on the dedicated runner with ≥ 7 consecutive green nights; a seeded 20 % slowdown turns it red; engine statistics as exact counts per PR (NFR-PERF-20) | `perf.yml` runs nightly, report-only on the hosted runner (ID-409); the seeded slowdown is proven by a unit test on parsed bench output, not inside the gate (FT-287); the per-PR engine-statistics snapshots of doc 18 §4.5 are not evidenced by this run | Open | Owner side (DEC-034) and development (FT-287, FT-291) | BM+7 |
| G-M1-13 | M | Oracle differential per PR; 7 nights of CI-N-02; fuzz targets for the JSONL loader, the migration chain and the command sequence (CI-PR-08, CI-N-03) with no open crash | The oracle runs on Golden A in the test jobs (AC-16, 0 mismatches). The command fuzzer is a seeded test covering 11 of 36 commands (doc 18 §3.6); CI-N-02, CI-PR-08, CI-N-03 and the loader and migration-chain targets do not exist. Lane B adds FT-279 and FT-272 in cycle 17 | Open | Development | C17; Cn (FT-297) |
| G-M1-14 | M | `load(save(m)) = m` byte-stable; determinism lints; incremental saves equal full saves | WP-S1.09 delivered (ID-200 to ID-217): incremental saves byte-identical to a full save, an unchanged project writes nothing; AC-20 writes one page file; AC-14 includes the JSONL files. The ordered-collection lint (FT-271) is not built | Met with conditions | Development (FT-271) | Cn |
| G-M1-15 | S | Core crates ≥ 80 % line coverage; ≤ 10 quarantined tests | CI-PR-13 does not exist (FT-296); no test is quarantined | Open (dated plan) | Development | Cn |
| G-M1-16 | H | Two named owners in `CODEOWNERS` for `ew-memo`, the `Op` set, `ew-format`, `ew-derive`; cross-lane review | `.github/CODEOWNERS` names one owner for the critical paths and states that a second is due by M1. Every cycle lane is reviewed independently (the review rulings in the decision log), which covers the review half; a second *named owner* is an accountable person, which agents cannot be | Not applicable by owner ruling 2026-10-22: a second named owner is not required (ID-452) | Owner side (ruling R5) | Owner |
| G-M1-17 | M | JSON Schemas generated and published; permissive crates deny missing docs; the permissive-only sample reader reads Golden A; one schema-steward handover | The schemas are generated from code with schema tests (`ew-schema`; the command parameter schemas of `ew-commands`); publication is not evidenced; rustdoc builds without warnings in `legal-docs`; the sample reader does not exist (FT-298); no schema steward is named | Open | Development (FT-298); owner side (name the steward) | Cn; Owner |
| G-M1-18 | H | LG-1 signed by the pilot panel builder and every other employer whose staff merged code | No statement on file; WP-S0.04 not started | Not applicable: dropped by owner ruling 2026-10-22 (ID-454) | Owner side (pilot user; counsel) | Owner |
| G-M1-19 | H | LG-2: ADR-0025 Accepted; the plugin and design exceptions in `LICENSES/` and the SPDX headers; the merge block lifted | ADR-0025 Proposed; the exception texts are not in `LICENSES/`. The `legal-docs` job's LG-2 merge block enforces NFR-LIC-12 until then | Not applicable: dropped by owner ruling 2026-10-22 (ID-454) | Owner side (project owner; counsel) | Owner |
| G-M1-20 | H | ADR-0030 Accepted; counsel's opinion on the CRA scope and on the preview's CRA status; the enrolments on track with receipts | ADR-0030 Proposed; no opinion on file; the enrolments of WP-S0.05a (G-M0-17) are not submitted, and their lead time of at least 12 months starts only at submission | Not applicable: dropped by owner ruling 2026-10-22 (ID-454) | Owner side (project owner; counsel) | Owner |
| G-M1-21 | M | LG-3 clearance report on file, or the rename decision | The search is not commissioned (WP-S0.05a) | Not applicable: dropped by owner ruling 2026-10-22 (ID-454) | Owner side (steward) | Owner (a typical condition, §5.3) |
| G-M1-22 | M | Signed source tag `m1` with release notes; nightly CLI builds labelled "unsigned development build"; no binary release | No tag, no nightly CLI artefact; no binary has been published (the negative part holds) | Open | Development (FT-299); owner side (signs the tag) | Cn; T |
| G-M1-23 | M | The pilot's side-by-side review of the generated PDF; the pilot log started; no pilot question open more than two months | Not held; no pilot log; the kick-off has not been held (2 of 12 P1 questions answered: PQ-01, and PQ-02 in part; ID-272). The review judges content, not style (ID-272, ID-340) | Open | Owner side (pilot user, project lead) | Owner |
| G-M1-24 | H | PD-11 re-plan: the velocity ratio over the spine WPs, `schedule.py` re-run, the AI scenarios retire, levers and PD-14 re-confirmed, PD-3 reconsidered | The cycle-based plan of record has no actual person-weeks to divide by (ID-271); the criterion needs restating (ruling R4) | Open | Owner side (ruling R4) | Ruling; T |
| G-M1-25 | M | With a two-developer team: the re-scope decision | There is no two-developer team (ID-271) | Not applicable (owner confirms, R4) | Owner side | Ruling |
| G-M1-26 | M | NLnet proposal 2 submitted; NL-1 … NL-4 evidence archived; WBSO renewal prepared | Nothing on file; the funding plan describes the human-team reference (ID-271) | Not applicable: no funding is pursued (owner ruling 2026-10-22, ID-453) | Owner side | Ruling |
| G-M1-27 | S | Landscape re-check against the OSS landscape | Not done | Open (dated plan) | Development (lead) | Pack |

**Counts.** H met 4 of 15 (G-M1-03, -04, -06, -07); 11 open, of which five are owner-side only (G-M1-16, -18, -19,
-20, -24), four need owner and development (G-M1-02, -05, -11, -12), one is development only (G-M1-01) and one is a
ruling (G-M1-08). M met 1 of 10 (G-M1-10), met with conditions 2 (G-M1-09, -14), open 6 (G-M1-13, -17, -21, -22,
-23, -26), not applicable 1 (G-M1-25). S met 0 of 2. Under §3.4 of the gates the outcome today would be *recycle*.

**The performance criteria stay on the BM-class runner.** AC-17, G-M1-11 and G-M1-12 name the baseline machine, and
the owner has ruled hosted and developer-machine numbers informational until that runner exists (ID-411). This run
proposes no interim machine for a hard criterion (ID-420).

## 5. Owner-side items

Ordered by what they unblock. "Pack" means on file before T − 7 days; statements are cited in the pack, never copied
(§1.2 principle 8 of the gates).

| # | Item | Owner (role) | What exactly the owner must do | Gate relevance | Date rule |
|---|---|---|---|---|---|
| O1 | *Dropped by owner ruling 2026-10-22 (ID-454).* **LG-5** fixture publication consent (DEC-013, PQ-37) | Pilot user, for the pilot panel builder (and its customer if its contract requires) | Sign the written consent that the anonymised technical content of the reference project is published as CC0 ([governance §3.3](../../design/19-open-source-governance.md#33-legal-and-organisational-gates)); file it with the steward (`legal/`, ST-09) | None (dropped) | Owner, now |
| O2 | **G-M0 decisions**: PD-1, PD-2, PD-3, PD-14; ADR-0027 and ADR-0028 | Project owner; pilot user for the ADRs and PD-14 | *PD-1 and PD-3 done on 2026-10-22 (ID-450); PD-2, PD-14 and the ADR acceptances remain.* Record PD-1 for the cycle-based plan (DEC-001, DEC-081), PD-2 with the pilot (PQ-40), PD-3; hold the PD-14 scope workshop (PQ-53); accept or change ADR-0027 and ADR-0028 (their defaults are in force) | Blocking for G-M0, which precedes G-M1 | Owner |
| O3 | *Dropped by owner ruling 2026-10-22 (ID-454).* **LG-1** employer copyright statement (DEC-014, PQ-38) | Pilot user, for the pilot panel builder; counsel reviews the text | Sign the statement with its parts (a) to (e) of governance §3.3, including the coverage of the repository owner's own work; counsel confirms in writing; file with the steward | None (dropped) | Owner |
| O4 | *Dropped by owner ruling 2026-10-22 (ID-454).* ID-383 is owner-accepted as a distribution fact; no counsel brief. **LG-2** exceptions adopted; with it the **ID-383** question | Project owner; counsel | Counsel finalises the plugin-exception and design-exception texts of ADR-0025 and, in the same brief, confirms that the CC-BY-SA-4.0 symbols compiled into the GPL binary are permitted (ID-383; doc 19 §7 row 3 keeps its **verify** mark until then); the owner accepts ADR-0025; development commits the texts to `LICENSES/` and the SPDX headers, and the `legal-docs` merge block lifts by itself | None (dropped) | Owner |
| O5 | *Dropped by owner ruling 2026-10-22 (ID-454).* **ADR-0030** and the enrolments (PD-7, PD-7a; WP-S0.05a) | Project owner; counsel | Decide the steward form, the CRA role, the support period and the signing identity; obtain counsel's opinion on the CRA scope (SR-14) and on the CRA status of the pilot preview (CP-15); name the phase-1 identity holder and submit D-U-N-S, the Apple Developer Program as an organisation and the Artifact Signing identity validation, keeping the receipts | None (dropped) | Owner |
| O6 | **DEC-034** the BM-class performance runner | Project owner, as steward | Provide a steward-owned BM-class machine, isolated from secrets, registered as a self-hosted runner that runs only `main` and maintainer pull requests labelled `perf` (doc 18 §5.4); development then re-blesses the baseline on it (FT-291) and the nights count | Blocking (G-M1-05, -11, -12, all H) | BM+7 |
| O7 | *Removed by owner ruling 2026-10-22: a second named owner is not required (ID-452).* **Second code owner** (NFR-MAINT-07) | Project owner | Name a second accountable maintainer for `ew-memo`, the `Op` set, `ew-format` and `ew-derive`, added to `CODEOWNERS` by handle; or take ruling R5's fallback | None (removed) | Owner |
| O8 | **FT-10** the first PNG approval | Pilot user | Review the PNG candidates of `golden/J-E-2416/expected/png/`, checking VR-01 (coil against resistor) and VR-02 (the changeover contact's hook); accept the known gaps of the golden README ("Known gaps for the FT-10 approval": the provisional router's overlapping routes on 101, 102, 103 and 106) as listed deviations or name the pages to wait for the router; development blesses the approved set and runs the raster comparison | Blocking with FT-294 (G-M1-02, H: AC-02's raster row) | Owner |
| O9 | **Golden acknowledgement** and the pilot having seen the tables | Pilot user | Acknowledge the golden-table change list (FT-300), including FT-29, FT-40 and FT-63 (PD-15), and confirm having seen the tables | Condition (G-M1-09); entry criterion E3 | Owner |
| O10 | **The pilot's P1 and P2 answers** ([decision log Part C](../06-decision-log.md#42-the-kick-off-agenda-all-p1-questions)) | Pilot user, at the kick-off (WP-S0.08) | Answer the ten open P1 questions (PQ-31, PQ-37 to PQ-42, PQ-53, PQ-06, PQ-07; PQ-01 is answered and PQ-02 in part, ID-272) and the 19 P2 questions, which freeze goldens and defaults; answers are recorded without names (§4.4 of the log) | P1: blocking through O1 to O3 and G-M0; P2: condition (G-M1-23's "no question open more than two months" starts counting when they are asked) | Owner |
| O11 | **Side-by-side review** and the pilot log | Pilot user with the project lead | Compare the generated PDF with the reference set; the intended deviations are those of golden §3.4, and the pilot's drawings are the content target, never a hard comparison of style (ID-272, ID-340); start the pilot log | Condition (G-M1-23) | Owner |
| O12 | **FT-284** `docs-book` as a required check; branch protection | Repository owner | The first `docs-book` run on `main` is green (2026-10-22, docs run 39706671506, FT-283 closed): add `docs-book` to the required status checks of `main`; confirm the protection rules of doc 18 §5.4 | Condition (CI-PR-10's docs part; G-M0-04's branch protection) | Owner |
| O13 | *Dropped by owner ruling 2026-10-22 (ID-454).* **LG-3** trademark clearance search | Steward | Commission the TMview, EUIPO eSearch and BOIP search (classes 9 and 42) and the domain check; file the report | None (dropped) | Owner |
| O14 | **PD-11** re-plan, G-M1-25 and G-M1-26 | Project owner | *G-M1-26 not applicable: no funding is pursued (ID-453); the PD-11 re-plan of G-M1-24 remains, at T.* Take ruling R4 and, at T, the re-plan decision it defines | Blocking (G-M1-24, H) | Ruling; T |
| O15 | **FT-281** Weblate project; **FT-285** website | A maintainer (FT-281); project owner (FT-285) | Create the Weblate project for the guide and the UI strings; commission the website with the download and privacy pages | Follow-up: FT-281 is due before the Dutch tutorials (MVP), FT-285 before the first public download (M-PP) | Their own dues |
| O16 | **The generated pages' licence** (ID-403) | — | Nothing: the owner ruled the generated reference pages GPL-3.0-or-later; recorded here so that the counsel brief of O4 does not reopen it | None | — |

## 6. Development items left

Each with its work package, its gate relevance and the cycle that closes it. Items marked *in cycle 17* are
delivered concurrently by lanes A and B and are confirmed at that delivery.

| Item | WP | Gate relevance | Plan | Note |
|---|---|---|---|---|
| **FT-165** the typed `Revision` (in cycle 17, lane A) | WP-S1.18 | Condition: G-M1-04 already passes; FT-165 empties path (b)'s `unwritten` list and removes the per-page revision values | C17 | Folio 911 stays a placeholder until FT-207 derives `revision_overview` from the revision records; take FT-207 before the side-by-side review (O11) |
| **FT-279, FT-272** the fuzzer's coverage of the 36 commands and its nightly form (in cycle 17, lane B) | WP-S6.02 | Condition (G-M1-13) | C17 | Seven nights of the nightly form then count towards G-M1-13 |
| **The G-SP re-test** (FT-26, FT-114 to FT-119, FT-121) with the XL benches and the budget check (FT-288) | WP-S1.05; WP-S6.03 | Blocking (G-M1-11) | Cn: class S first on the hosted runner, informational (ID-272); the authoritative M and XL numbers on the BM-class runner | KT-3 applies after one recycle if the M-class budgets stay out of reach |
| **WP-S6.03's open parts**: the in-gate seeded slowdown and the rolling baseline (FT-287), the class M baseline on the BM-class runner (FT-291), the bisect (FT-289), the reset label (FT-290), the counting allocator (FT-292), peak RSS on Windows and macOS (FT-89) | WP-S6.03 | Blocking: FT-287 and FT-291 (G-M1-12); follow-up: FT-289, FT-290, FT-292, FT-89 (the gate runs on Linux) | Cn; FT-291 after O6 | — |
| **FT-294** the veraPDF cross-check job (CI-N-09; AC-13's `not_applicable` row) | WP-S3.04; WP-S6.02 | Blocking (G-M1-02) | Cn | New task (ID-425) |
| **FT-295** the headless container job (CI-PR-05) | WP-S6.02; WP-S0.01 | Blocking (G-M1-01) | Cn | New task |
| **FT-297** the deep nightly (CI-N-02) and the parser fuzz targets for the JSONL loader and the migration chain (CI-PR-08, CI-N-03) | WP-S6.02 | Condition (G-M1-13) | Cn, after FT-272 | New task |
| **FT-271** the ordered-collection lint | WP-S6.02 | Condition (G-M1-14) | Cn | — |
| **FT-298** the permissive-only sample reader | WP-S1.02 | Condition (G-M1-17) | Cn | New task; also NL-4 and SC-11 |
| **FT-299** the nightly CLI artefact labelled "unsigned development build" and the `m1` release notes | WP-S6.02; WP-S0.01 | Condition (G-M1-22) | Cn; publishing waits for O4's ID-383 answer | New task; the owner signs the tag |
| **FT-300** the golden-table change list for the pilot's acknowledgement | WP-S7.01 | Condition (G-M1-09) | Cn | New task |
| **FT-296** coverage (CI-PR-13) | WP-S6.02 | S criterion: dated plan (G-M1-15) | Cn | New task |
| **The follow-up tasks due before the page approvals**: FT-102, FT-103, FT-107 (907, 903 … 905), FT-108 (909), FT-240 and the logo of FT-202 (folio 1; the logo needs an asset from the owner), FT-262 (folio 3), FT-246 | WP-S3.02; WP-S3.03; WP-S1.07; WP-S7.01; WP-S6.01 | Condition (G-M1-23, O8, O11): pages with known errors would be approved with deviations | Cn, before O8 and O11 | — |
| **FT-87, FT-88, FT-89, FT-91** (due "before M1") and **FT-92** (the crash harness, "G-M1 evidence") | WP-S1.09; WP-S6.02 | Follow-up: no G-M1 criterion names them (the crash harness is CI-N-07, required at G-M2) | Ruling R7 proposes "before G-PP" | The plan view of §3.8 had not listed them (ID-426) |
| **The rest of WP-S1.10**: the FR-EXT-032 views and their companions (FT-216 to FT-218, FT-220) and the file-backed XL projection (FT-219) | WP-S1.10 | Follow-up: FT-216 is due before G-M2; NFR-PERF-14 is not a G-M1 budget (§8.6 of the gates) | Their dues | FT-219's "before the M1 performance run" is moot while XL is measured only for open time and commit latency |
| **WP-S6.02's FT-234** the cross-platform digests | WP-S6.06; WP-S3.04 | Follow-up (G-M2, AC-14 all platforms) | Before G-M2 | — |
| **WP-S0.06's remaining items**: the first `docs-book` run (FT-283) and mdbook-i18n-helpers (FT-282) | WP-S0.06 | FT-283: closed (green on 2026-10-22, run 39706671506); FT-282: follow-up (with FT-281) | FT-283 at the next push to `main` | — |
| **The pack** (GP-01 … GP-14): the landscape re-check (G-M1-27), the forecast under ruling R4, the CI evidence | — | Pack | Pack | Lead work |
| **FT-301** the bookkeeping sweep of Appendix B | — | Follow-up (GP-07 hygiene) | Cn | 230 of 293 tasks are open, and many carry a past cycle as their due (FT-02 to FT-13 among them); their real state is not verified |

**Addendum (cycle-18 delivery, 2026-10-22; decision log ID-470).** Rows whose state changed, without rewriting the
run: **FT-294** and **FT-295** are built (the jobs `verapdf` and `headless` of `ci.yml`, ID-456, ID-457); G-M1-02 and
G-M1-01 stay open until their first green runs on `main`, and they become required checks after three green runs
(owner, FT-325). **FT-296** is built report-only (the weekly `coverage.yml`, ID-459); G-M1-15 waits for the owner's
dated plan. **FT-271** is closed (ID-463); its second half, the `disallowed-methods` lint, is FT-317 (condition,
G-M1-14).

**Addendum (cycle-19 delivery, 2026-10-22; decision log ID-483).** Rows whose state changed, without rewriting the
run: CI for the cycle-18 delivery f316d4d was green (CI run 39731089818), with the first runs of `headless` and
`verapdf` on `main`. **G-M1-01 is met** (the container export, FT-295). **G-M1-02**'s development part is done (the
veraPDF evidence, FT-294); it stays open on the owner side for the pilot's PNG approval (FT-10) and the raster run
after it. **G-M1-14 is met**: both determinism lints are built (FT-271 in cycle 18, FT-317 in cycle 19, decision log
ID-481). **G-M1-22**: the nightly unsigned CLI artefact is built (FT-299, CI-N-12) with the skeleton of the `m1` notes;
the first scheduled run, the filled notes and the owner's signed tag remain. FT-297, FT-298, FT-300, FT-304, the G-SP
re-test (stopped in cycle 19 to free the machine, not re-run) and the page tasks are unchanged. **Recount:** G-M1 has 6
of 11 applicable hard criteria met (G-M1-01, -03, -04, -06, -07, -08) and 5 open (G-M1-02 on the owner side, -05,
-11, -12, -24); of 7 applicable must criteria 2 are met (G-M1-10, -14), 1 is met with conditions (G-M1-09) and 4 are
open (G-M1-13, -17, -22, -23). The verdict stays **not ready**. The acceptance counts are unchanged (94 entries, 88
pass, 0 fail, 0 `not_built`, 6 `not_applicable`).

## 7. What is not ready

- **The legal basis.** None of LG-1, LG-2, LG-5 or ADR-0030 is on file; ADR-0025 and ADR-0030 are Proposed; no
  enrolment is submitted. Nothing on this list moves at the cycle pace, and LG-5 is overdue. *(Owner ruling
  2026-10-22, ID-454: no longer gate items.)*
- **The performance proof.** No BM-class runner exists; G-SP's re-test has not been run since the recycle; the
  nightly gate is report-only on class S. AC-17, G-M1-11 and G-M1-12 are hard criteria without a measurement.
- **The CI set that the gate names.** Of the jobs §8.5 of the gates requires at G-M1 beyond G-M0's, CI-PR-05, CI-PR-06,
  CI-PR-08, CI-PR-13, CI-N-02, CI-N-03, CI-N-04 and CI-N-09 do not exist as jobs (part of their test content, the
  golden PNG and PDF-structure comparisons and the seeded fuzzer, runs inside the three test jobs); CI-N-01 exists only
  as the hosted, report-only nightly; `docs-book` has not had its first run on `main`. AC-13's veraPDF row points at a job that does
  not exist.
- **The pilot's part.** No PNG approval, no golden acknowledgement, no side-by-side review, no kick-off: the pilot's
  answers and approvals, not the engineering, are the acceptance truth (§3.1 of the gates).
- **Ownership.** One named code owner for the critical crates. *(Owner ruling 2026-10-22, ID-452: a second named
  owner is not required.)*
- **The re-plan.** G-M1-24 asks for a velocity ratio in person-weeks that the cycle-based plan does not produce.
- **G-M0** has not been held.
- **D-06's property grid** does not exist and cannot before the UI of M-PP; as gated today it is a hard failure at a
  headless gate.

## 8. Rulings asked of the owner

| # | Ruling asked | Recommendation | Trade-off | Ruling (2026-10-22) | Record |
|---|---|---|---|---|---|
| R1 | D-06's property-grid half | Split D-06 like AC-18 and AC-14: its headless entries stay at G-M1, the property-grid entry (FR-UI-009) moves to G-M2 with AC-18 (GUI); edit §8.2 of the gates and re-run `trace.py` | A UI check leaves a headless gate, against keeping a criterion that no headless build can meet; nothing is dropped, the entry still gates before the first release | Accepted (delegated): §8.2 and G-M1-08 edited, `trace.py` regenerated, doc 20 §22 aligned; the runner's gate for the entry is code side, applied at the cycle-17 delivery | ID-422 |
| R2 | The date rule of §9 | Adopt | A content-triggered gate, against the §3.3 calendar that assumes a known milestone date | Adopted (delegated); adopted by the project owner on 2026-10-22 (ID-451); written into §3.3 of the gates | ID-421, ID-451 |
| R3 | G-M0 before G-M1 | Hold G-M0 as soon as O1 and O2 are done; combine only if both are ready together | One more meeting, against a G-M1 that inherits G-M0's open items | Accepted (delegated) | ID-421 |
| R4 | G-M1-24, -25 and -26 under the cycle-based plan | Restate G-M1-24 as: the cycle-based forecast of M-PP and M2 by content, with the owner-side lead times, replaces the AI scenarios, which retire; the human-team reference stays as reference; levers (a) to (c) and PD-14 are re-confirmed. G-M1-25 not applicable. G-M1-26: the owner states whether any funding is pursued; if none, not applicable | The criterion keeps its intent (a re-based, single plan of record) without inventing person-week actuals | Accepted (delegated): G-M1-24 restated in §5.3; G-M1-25 not applicable; G-M1-26 not applicable since the owner ruled on 2026-10-22 that no funding is pursued (ID-453) | ID-423, ID-453 |
| R5 | G-M1-16, the second code owner | Name a second accountable maintainer. Fallback: amend G-M1-16 so that the independent lane reviews count for the review half and the second named owner becomes a G-M2 hard criterion, with R-014 re-scored | The bus factor of the core is real whoever writes the code; the fallback defers it to before the first signed release | Not ruled by delegation; closed by the owner on 2026-10-22: not required (ID-452) | ID-424, ID-452 |
| R6 | Where gate records live (open question 3 of the gates) | `docs/plan/gate-records/`, as this run does | A dated folder beside the standing documents | Accepted (delegated); open question 3 of the gates resolved | ID-419 |
| R7 | FT-87, FT-88, FT-89, FT-91, FT-92 | Re-date them from "before M1" to "before G-PP": the pilot's first interactive use is where recovery and lenient opening matter, and no G-M1 criterion names them | The reliability work moves behind the headless gate, against holding G-M1 for work it does not check | Accepted (delegated): the five tasks re-dated to before G-PP in Appendix B (FT-89 keeps the lane-B extension) | ID-425 |
| R8 | The new tasks FT-294 to FT-301 | Accept them into Appendix B with the hosts and dues given | — | Accepted (delegated) | ID-425 |

## 9. The proposed date rule

*Decision log ID-421 (adopted by the project owner on 2026-10-22, ID-451).*

1. **Trigger.** G-M1 is held (T) within two weeks after the later of: (a) the cycle delivery at which a repeated
   readiness run finds no development-side H criterion open and at most three M criteria open, each closable within
   four weeks; and (b) the day the last owner-side H item is on file. For G-M1-12, (b) is at the earliest the seventh
   consecutive green night after the BM-class runner is commissioned (BM+7).
2. **The §3.3 calendar on cycles.** The readiness run is repeated at every cycle delivery as a delta of this
   document until (a) holds; it replaces "T − 6 weeks". The scope freeze ("T − 2 weeks") is the first re-run at which
   (a) holds and the rulings R1 to R8 are taken: from then on no new criterion and no new M1 work package. The pack is
   published at T − 7 days, the record within two weeks after T, and conditions close within four weeks (unchanged).
3. **G-M0 first** (R3).
4. **External dates** stay open and are derived from the content still open and the owner-side lead times when the
   owner names one (DEC-001, DEC-081, ID-271), never from the month tables.

**Earliest T under this rule.** Development: about two cycles after cycle 17 for (a). Owner side: (b) is bounded by
the slowest of LG-1, LG-2 with counsel, ADR-0030 with counsel and the enrolment submissions, the second code owner,
and BM+7. The development side will be ready first.

## 10. Cycle-17 rows to confirm at delivery

| Lane | Item | What the delivery must show for this report to change |
|---|---|---|
| A | FT-165 the typed `Revision` | The runner's AC-15 path (b) entry lists no unwritten records; the per-page revision values are removed; the acceptance counts stay 0 fail (expected: 93 entries, 87 pass, 1 `not_built`, 5 `not_applicable`, unless the lane adds entries) |
| B | FT-279, FT-272 the fuzzer's coverage and nightly form | The fuzzer covers the 36 registered commands; a nightly job runs it; its nights then count for G-M1-13 |
| Orchestrator | FT-283 the first `docs-book` run on `main` | Done: green on 2026-10-22 (docs run 39706671506, 49 s) with the network-less build; O12 can be done now |

**Addendum (cycle-17 delivery, 2026-10-22; decision log ID-438).** Lane A confirmed: path (b)'s `unwritten` list is empty and the counts are 94 entries, 88 pass, 0 fail, 0 `not_built`, 6 `not_applicable` (D-06's grid entry under ID-422, so G-M1-08 is met: 5 of 15 H).
Lane B confirmed: the fuzzer covers the 37 registered commands and the nightly `fuzz.yml` exists; its nights count from its first scheduled run, for G-M1-13's fuzzer part only until ASan and the rest of CI-N-02 (FT-304, FT-297) join it.

**Addendum (cycle-18 delivery, 2026-10-22; decision log ID-470).** No cycle-18 row was listed here; the rows of §6 that
cycle 18 changed (FT-294, FT-295, FT-296, FT-271) are in §6's addendum. The acceptance counts are unchanged (94
entries, 88 pass, 0 fail, 0 `not_built`, 6 `not_applicable`).

---

## Sources

- [Gates](../03-gates.md) §2.4, §3.2 to §3.5, §4.1, §5.2, §5.3, §7, §8.1, §8.2, §8.5, §8.6, open question 3.
- [Global plan](../01-global-plan.md) §3.8 (the cycle-based progress view and the M1 remaining list).
- [Work breakdown](../02-work-packages.md): the M1 work packages and their statuses; Appendix B.
- [Decision log](../06-decision-log.md): ID-1, ID-2, ID-200 to ID-218, ID-271, ID-272, ID-299, ID-302, ID-340,
  ID-383, ID-400 to ID-418; DEC-001, DEC-013, DEC-014, DEC-034, DEC-081; Part C.
- [Golden project](../../design/20-golden-project.md) §19, §21, §22; the golden README; the committed headless report.
- [Quality, security and release](../../design/18-quality-security-release.md) §3.6, §3.14, §4.4 to §4.7, §5.
- [Open-source governance](../../design/19-open-source-governance.md) §3.3, §7.
- [Reference analysis](../../research/reference-drawing.md) §4.15 (SC-1's criteria).
- [Risk register](../04-risk-register.md): R-014, R-018, R-045, R-046, R-050, R-053.
