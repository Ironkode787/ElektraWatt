<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Acceptance run: ElektraWatt 0.1.0

The acceptance of gate G-M2 for the release 0.1.0 (WP-S6.08 D12): one row per item G-M2-01 to G-M2-26 of
[the gates](../../../plan/03-gates.md#55-g-m2-mvp-release-010-m2) §5.5, and per AC-21, AC-23, AC-40 and AC-41 of
[the golden project](../../../design/20-golden-project.md) and NFR-REL-11. Each row names its criterion by reference,
the evidence that proves it and the result: **met**, **met with a declared gap** (the gap named), **not met**, **not
applicable** (by the ruling named), or **open** while its evidence does not exist yet.

A row is met only with its evidence on the last candidate's commit. A row whose evidence is a CI run on a candidate,
or a person's report (R1, the pilot, the manual tests), stays open until that evidence exists, its source named. The
timing criteria (G-M2-07's budgets, the 16 ms frame bound of G-M2-02 and the 30 fps bound of G-M2-08) get one measured
run on the last candidate, reported and not gating (decision log ID-1331 (3)).

**Candidate:** none yet · **Commit:** — · **Filled by:** the release manager's agents, signed with the checklist

| Item | Criterion | Evidence | Result |
|---|---|---|---|
| G-M2-01 | AC-14 on Windows and Linux: PDF, SVG, CSV and JSONL byte-identical across OSes, thread counts and record order | CI-N-04's digests from both OSes on the commit (`package.yml`'s `digests` job) | Open: the CI run on the candidate's commit |
| G-M2-02 | AC-18, AC-19 and AC-28 through the window; the 16 ms frame bound | CI-PR-03 and CI-PR-07 (WP-S2.11's flows) on the commit; the frame bound from the measured run of ID-1331 (3) | Open: the CI run on the candidate's commit; the measured run on the last candidate |
| G-M2-03 | AC-21 and AC-23 | The acceptance runner's report (`cargo xtask acceptance`); the release kill-point test | Open: the runner's report on the candidate's commit; lane tip: AC-23 passed in the runner's report (pre-run 1) |
| G-M2-04 | AC-29, AC-31, AC-32 (MVP part), AC-33 and the rest of the row; AC-40, AC-41 | The acceptance runner's report; CI-R-10's installed acceptance | Open: the runner's report and the candidate's `release.yml` run; lane tip: AC-31 and AC-32's MVP part passed in the runner's report (pre-run 1); AC-41's driver tests passed (pre-run 2) |
| G-M2-05 | Every defect regression gated M2 or MVP, and the M1 rows (AC-24) | The acceptance runner's report, per D-nn | Open: the runner's report on the candidate's commit; lane tip: 46 D-nn entries passed and D-06 not applicable in the runner, D-06 passed through the window (pre-run 1) |
| G-M2-06 | SC-1 and SC-2: no command produces a D-nn of class S or I; every class-C defect of a source project reported | CI-N-02 (the command fuzzer) in the deep nightly on the commit; Golden B | Open: the deep nightly on the candidate's commit |
| G-M2-07 | The performance budgets (NFR-PERF-03, NFR-PERF-04 and the rest of the row) | One measured run on the last candidate (`cargo xtask acceptance --milestone`, the nightly performance report), reported and not gating (ID-1331 (3)) | Open: the measured run on the last candidate |
| G-M2-08 | Without a hardware adapter: the CPU renderer at 30 fps or more, WARP, every 2D function; RDP without GPU | CI-N-05 (WARP, the CPU renderer) on the commit; RC-05's RDP run in [`manual.md`](manual.md); the 30 fps bound from the measured run | Open: the CI run, the manual test and the measured run |
| G-M2-09 | Reliability: interrupted, failed and crashed saves leave a recoverable project; fault injection on ext4 and NTFS | CI-N-07 and CI-PR-11 on ext4 and NTFS; APFS not applicable (no macOS, decision log ID-880) | Open: the CI runs on the candidate's commit |
| G-M2-10 | Accessibility: names, roles and states; the five core tasks from the keyboard; the colour lints | CI-PR-07 (the name walker, the keyboard flows, the lints) | Open: the CI run on the candidate's commit |
| G-M2-11 | Stability guards: the snapshots on every PR; every released format fixture opens; upgrade-on-read and the unstable-format warning | CI-PR-14 (`api-guard`) with its `format` surface; the corpus job and the steward's check with `fixtures/format/0.1/` | Open: the CI run on the candidate's commit (the lock is in, `bd4fedf2`); lane tip: `api-guard` green, the steward's check and the corpus job passed with `fixtures/format/0.1/` (pre-runs 3, 4) |
| G-M2-12 | English and Dutch: at least 95 % of the strings and every core-flow string translated (as amended: the translation report) | The translation report of `ci/translation-status.py` on the commit, in [`translation.md`](translation.md) | Open: the report on the candidate's commit |
| G-M2-13 | The release pipeline CI-R-01 to CI-R-11 in 0.1.0's form ran end to end on the candidate | The candidate's `release.yml` run; IN-15 in [`manual.md`](manual.md) | Open: the candidate's tag; lane tip: `release-verify.py --test` passed and `tag v0.1.0-rc.1` accepted the tree (pre-run 5) |
| G-M2-14 | No unpatched high or critical advisory; waivers within 90 days with owners; no open fuzz crash; the hostile corpus | CI-R-02's log; the deep nightly's fuzz; WP-S6.04's hostile-corpus test | Open: the candidate's runs |
| G-M2-15 | SC-10: no network connection without a user action, no update check or crash upload, no listening socket | CI-N-06 (the network sandbox); CI-R-09's listening-socket check | Open: the deep nightly and the candidate's `package` run |
| G-M2-16 | The CRA steward processes | — | Not applicable: decision log ID-1187 (NFR-COMP-01 at 1.0) |
| G-M2-17 | The release checklist RC-01 to RC-10 complete (RC-10 without LG-4 and the steward checklist) | The filled `checklist-rc.<n>.md` of the last candidate | Open: the first candidate |
| G-M2-18 | LG-1 statements as a gate item | — | Not applicable: decision log ID-454 (LG-1 stays RC-10's item, the owner's G3 in [`README.md`](README.md)) |
| G-M2-19 | The signing identity as a legal gate | — | Not applicable: decision log ID-454 and ID-1187 (signing is 1.0's) |
| G-M2-20 | R1 passed as a blind-persona round; its severe findings fixed with a kittest flow | The R1 report (GP-10, WP-S2.12) | Open: WP-S2.12's R1 report |
| G-M2-21 | ADR-0007 validated by the blind-persona R1 | The R1 report's validation record (WP-S2.12) | Open: WP-S2.12's R1 report |
| G-M2-22 | The R0 report exists and its severe fixes are closed before R1 | The R0 report and its closed fixes (WP-S2.12) | Open: WP-S2.12's R0 report |
| G-M2-23 | Documentation: the bundled guide with F1 anchors, a page per shipped rule, the tutorials, the glossaries, the release and migration notes, the governance files | WP-S7.05's acceptance; `docs-book` (CI-PR-10) on the commit | Open: the docs build on the candidate's commit |
| G-M2-24 | The pilot switch, read with decision log ID-1384: the candidate in the pilot's hands, the issue list triaged if it holds an item, NFR-UX-08 by `golden_from_the_basic_project`, the onboarding, the check-ins, the SC-3 baseline, the pilot's go | The pilot report (GP-09) | Open: the pilot report on the candidate |
| G-M2-25 | The re-forecast and funding | — | Not applicable: decision log ID-454 and ID-1187 |
| G-M2-26 | The corpus-gated tests pass on this PC in every delivery's last verify | The last verify's log | Met at cycle 51's merge point (decision log ID-1388); the last verify before the gate runs them again |
| AC-21 | `ew check --scheme pre-delivery` on Golden A reports exactly `findings_mvp.csv` and exits 1 | The acceptance runner's report | Open: the runner's report on the candidate's commit; lane tip: the AC-21 tests of `ew-api` and `ew` passed (pre-run 2) |
| AC-23 | The release of V2.0 is refused on Golden A with exactly the blockers of doc 20 §18.3 and succeeds on Golden A-R after the approval, writing a verified as-built archive | The acceptance runner's report; the release kill-point test | Open: the runner's report on the candidate's commit; lane tip: passed in the runner's report (pre-run 1) |
| AC-40 | The release ships Golden A as its sample project; it opens offline without libraries or system fonts and passes its acceptance CSVs | CI-R-10 (`ci/installed-acceptance.sh` in `package.yml`) | Open: the candidate's `release.yml` run |
| AC-41 | Two branches editing disjoint fields merge cleanly with `ew merge-driver`; editing the same field yields one conflict record | The acceptance runner's report; `ew`'s merge-driver tests | Open: the runner's report on the candidate's commit; lane tip: the merge-driver tests passed (pre-run 2) |
| NFR-REL-11 | The headless export on Golden A without libraries or system fonts | CI-PR-05 on the commit; AC-40 | Open: the CI run on the candidate's commit |

## The lane's pre-run (WP-S6.08 step 6)

The targeted runs a lane may make, on the lane's tip after the lock (`bd4fedf2`, version 0.1.0, format 0.1 released),
entered above as "lane tip". They prove nothing about a candidate: every row stays open until its evidence exists on
the last candidate's commit (§7 of the specification); the rows that need CI or people are left for the release cycle.

| # | Run | Result |
|---|---|---|
| 1 | `cargo xtask acceptance --gate M2 <report>` (no `--milestone`: no timing) | Passed: 107 entries, 102 pass, 0 fail, 0 not built, 5 not applicable (AC-13's veraPDF, AC-15, AC-17's timing, AC-39, D-06 in the runner), no oracle mismatch; the window's M2 criteria (AC-18, AC-28, D-06, NFR-A11Y-01, NFR-A11Y-02) pass; 103 requirements whose acceptance names GP are `missing`, which does not block at 0.1.0 |
| 2 | The package set's tests (`ew-store`, `ew-format`, `ew-schema`, `ew-api`, `ew-commands`, `ew`, `ew-viewmodel`, `ew-ui`, `xtask`, `elektrawatt`), AC-21's (`ac21_four_pack_subset_equals_findings_mvp_csv`, `ac21_through_the_cli_equals_findings_mvp_csv_for_the_built_packs`) and AC-41's merge-driver tests among them | Passed |
| 3 | `cargo xtask api-guard --base e8b5643e` | Passed: 1 breaking change with its `- api-break:` line (`cli merge-driver exit 1`, FT-1516); `format 0.1: not released; nothing to compare` until the tag `v0.1.0` exists (D14) |
| 4 | `ew-store`'s `mg6_the_shipped_chain_and_corpus_pass_the_steward_check` and `the_corpus_job_saves_each_fixture_to_its_expected_files` | Passed with `RELEASED_FORMATS = ["0.1"]` and `fixtures/format/0.1/J-E-2416.ew/` |
| 5 | `python -I ci/release-verify.py --test`; `python -I ci/release-verify.py tag v0.1.0-rc.1` | Passed: 14 tests; `version=0.1.0` |
