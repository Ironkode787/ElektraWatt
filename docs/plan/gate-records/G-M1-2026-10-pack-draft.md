# G-M1 gate pack (draft)

**Status:** Draft, not convened · **Date:** 2026-10-22 · **Gate:** G-M1 Spine exit · **Scenario:** the cycle-based
plan of record (decision log ID-271) · **Commit:** de23b55 (the cycle-16 delivery) · **Previous gate record:** G-SP,
held 2026-10-20 ([gates §5.1](../03-gates.md#51-g-sp-performance-spike-decision)) · **Part of:** ElektraWatt plan,
gate records

**Purpose.** This is what the project owner receives to decide on G-M1, in the form of the gate pack of
[gates §4.1](../03-gates.md#41-the-gate-pack) (GP-01 to GP-14) and the record template of
[§7](../03-gates.md#7-gate-review-template). It is drafted at the readiness run
([G-M1-2026-10-readiness.md](G-M1-2026-10-readiness.md)), so the review is not convened and the outcome is blank.
Fields in `<angle brackets>` are placeholders that the integrating session fills (CI run IDs) or that the review
fills (outcome, attendance). Roles only, never names; statements are cited, never copied (§1.2 principle 8 of the
gates).

---

## 1. The decision asked of the owner

**Now (at the readiness run): not the gate decision.** The readiness run's verdict is *not ready*: 11 of 15 hard
criteria and 6 of 10 must criteria are open, and G-M0 has not been held. Holding G-M1 today would end in *recycle*.
The owner is asked to:

1. **Decide R5, the second code owner** (G-M1-16, ID-424): name a second accountable maintainer, or take the option
   of moving that requirement to G-M2. The other rulings of the readiness report §8 were taken by delegation on
   2026-10-22: R1 D-06's property-grid entry at G-M2, R3 G-M0 first, R4 G-M1-24 restated and G-M1-25 not applicable
   (G-M1-26 waits for the owner's word on funding), R6 `gate-records/`, R7 FT-87 to FT-92 before G-PP, R8 FT-294 to
   FT-301.
2. **Start the owner-side items** of the readiness report §5, in that order: LG-5 and the G-M0 decisions first, then
   LG-1, LG-2 (with the CC-BY-SA question ID-383 in the same counsel brief), ADR-0030 with the enrolments, the
   BM-class runner (DEC-034), the second code owner, then the pilot's approvals, acknowledgement, answers and review.
3. **Confirm or amend the date rule at G-M0** (ID-421, adopted by delegated ruling R2): G-M1 is held within two weeks
   after the later of the cycle delivery whose repeated readiness run finds no development-side H open (and at most
   three M), and the day the last owner-side H item is on file, at the earliest seven nights after the BM-class
   runner is commissioned.

**At T (when the date rule fires): the gate decision.** The chair (project lead) decides go, go with conditions or
recycle (§3.2 of the gates); the project owner decides PD-7, PD-11 and the opening of community channels; the pilot
user concurs on the golden acknowledgement and the side-by-side review; counsel confirms LG-1, LG-2, ADR-0025,
ADR-0030 and the CRA scope in writing. G-M1 authorises the interactive phase as the main effort, the acceptance of
external contributions (LG-2), opening community channels and the re-based plan (PD-11).

## 2. Pack index

| # | Artefact | State at the readiness run | Where | To do before T − 7 days |
|---|---|---|---|---|
| GP-01 | Acceptance-runner report | 93 entries: 87 pass, 0 fail, 1 `not_built`, 5 `not_applicable`; 0 oracle mismatches | [`acceptance_headless.md`](../../../golden/J-E-2416/expected/acceptance_headless.md) at de23b55; the milestone report `<acceptance --milestone report, commit>` | Re-run on the gated commit, with `--milestone` on the BM-class runner (AC-17) and the raster comparison after FT-10 |
| GP-02 | Exit-checklist sheet | 27 rows with status, evidence and owner | Readiness report §4 | Re-issue at the scope freeze |
| GP-03 | CI report | §4 below, with placeholders | This pack | Fill the run IDs; the last 7 nightlies |
| GP-04 | Performance and memory report | Not measured at the gate's classes (milestone rule; no BM-class runner) | `perf.yml` artefact `perf-report` `<run id>` (class S, informational) | The G-SP re-test and the BM-class report (G-M1-05, -11, -12) |
| GP-05 | Forecast | The cycle-based progress view; the human-team reference unchanged | [Global plan §3.8](../01-global-plan.md#38-cycle-based-progress-view-plan-of-record-since-2026-10-20) | The re-plan defined by ruling R4 |
| GP-06 | Risk register | §6 below | [Risk register](../04-risk-register.md) | Re-score R-014 if R5's fallback is taken |
| GP-07 | Decision log | Decisions due at G-M1: 26 DEC rows (decision log summary); this run's ID-419 to ID-426 | [Decision log](../06-decision-log.md) | R1 to R4 and R6 to R8 recorded (delegated, 2026-10-22); R5 open for the owner |
| GP-08 | Legal-gate status | §7 below | This pack; statements with the steward (`legal/`, ST-09) | Statements on file, cited |
| GP-09 | Pilot report | §8 below | This pack | Pilot log, acknowledgements, review minutes |
| GP-10 | Usability report | Not required at G-M1 | — | — |
| GP-11 | Release dossier | Not required at G-M1 (no binary release; source tag only) | — | — |
| GP-12 | Demo script | §9 below | This pack | Rehearse on the gated commit |
| GP-13 | Funding evidence | Nothing on file; applicability is ruling R4 | — | Per R4 |
| GP-14 | Gate record draft | §10 below | This pack | Fill at T |

## 3. Acceptance (GP-01)

| Item | Result at de23b55 |
|---|---|
| Entries | 93: **87 pass, 0 fail, 1 `not_built`, 5 `not_applicable`**; 0 oracle mismatches |
| AC gated at G-M1 (§8.1 of the gates) | AC-01 … AC-13: pass except AC-02's raster row and AC-13's veraPDF row (`not_applicable`); AC-14 (Linux): pass; AC-15, AC-16: pass (AC-15 (a) = (c) evaluated by `tests/golden_fixture.rs`); AC-17: `not_applicable` (BM runner); AC-18 (CLI): pass; AC-20: pass; AC-39: `not_applicable` in the runner, green as `cargo xtask check-anonymisation` |
| D-rows gated at G-M1 (26, §8.2 of the gates) | 25 pass; D-06 passes three entries, its property-grid entry is `not_built` (ruling R1) |
| `not_applicable` entries (5) | AC-02 raster (headless approval: FT-10), AC-13 veraPDF (external tool: FT-294), AC-15 path (a) = (c) (fixture test), AC-17 (BM runner, `--milestone`), AC-39 (xtask check) |
| Success criteria touched | SC-1 headless: criteria 1 … 7 and 9 of the reference analysis §4.15 map to passing rows (readiness report, G-M1-10); SC-6 on Linux: AC-14; SC-11: the sample reader is not built (FT-298) |
| Current since | Cycle 16, decision log ID-406; cycle 17's FT-165 changes path (b)'s `unwritten` list, `<counts at the cycle-17 delivery>` |

## 4. CI evidence (GP-03)

Run IDs filled by the integrating session on 2026-10-22 (GitHub Actions, repository `ElektraWatt`; job wall times in
parentheses). Gates at de23b55 as reported: Windows 2,612 and Linux 2,623 tests, clippy,
doctests, rustdoc and the 16 steps of `cargo xtask ci`.

| Job (doc 18 §5.2) | Workflow and job | Commit | Run | Result |
|---|---|---|---|---|
| CI-PR-01 lint | `ci.yml` `lint` | de23b55 | 39706671490 | green (0:58) |
| CI-PR-02 test-linux | `ci.yml` `test-linux` | de23b55 | 39706671490 | green (21:30; the all-features run 2,623 tests) |
| CI-PR-03 test-windows | `ci.yml` `test-windows` | de23b55 | 39706671490 | green (30:54, above the 30-minute budget of NFR-MAINT-05 by 54 s, within the 50-minute guard; the filtered run 2,612 tests) |
| CI-PR-04 test-macos | `ci.yml` `test-macos` | de23b55 | 39706671490 | green (19:36) |
| CI-PR-09 supply-chain | `ci.yml` `supply-chain` | de23b55 | 39706671490 | green (1:17) |
| CI-PR-10 legal-docs | `docs.yml` `legal-docs` (reuse, licence map, anonymisation guard, governance files, rustdoc, DCO and LG-2 block on pull requests) | de23b55 | 39706671506 | green (1:12) |
| CI-PR-10 docs-book | `docs.yml` `docs-book` (first run on `main`, FT-283) | de23b55 | 39706671506 | green (0:49): the network-less build under `sudo unshare --net`, the pinned mdBook 0.5.4 digest and the link check all passed on the hosted runner; FT-283 closed |
| CI-PR-15 docs-diagrams | `docs.yml` `docs-diagrams` | de23b55 | 39706671506 | green (2:55) |
| CI-N-01 perf (hosted, report-only, class S) | `perf.yml` `perf-gate` | `<commit>` | `<run ids of the last 7 nights>` | `<report verdicts; informational, ID-411>` |
| `cargo xtask ci` (16 steps) | Integrating session, Windows and Linux | de23b55 | The integrating session's gate logs (cycle 16; not committed) | green: Windows 2,612 + 2,623 tests, Linux 2,623 (all features), clippy, doctests, rustdoc on both |
| CI-PR-05 headless | Not built (FT-295) | — | — | Required at G-M1 |
| CI-PR-06 output | Not a separate job; the golden PNG and PDF-structure comparisons run in the test jobs; veraPDF not built (FT-294) | — | — | Required at G-M1 |
| CI-PR-08 fuzz-smoke | Not built (FT-272 in cycle 17, FT-297) | — | — | Required at G-M1 |
| CI-PR-13 coverage | Not built (FT-296) | — | — | Required at G-M1 |
| CI-N-02 deep, CI-N-03 fuzz | Not built (FT-272 in cycle 17, FT-297) | — | — | Required at G-M1, 7 nights |
| CI-N-04 digests (Linux) | Not a job; AC-14's Linux entries pass in the runner | — | — | Required at G-M1 |
| CI-N-09 pdf | Not built (FT-294) | — | — | Required at G-M1 |
| CI-R-01 on the `m1` tag | No tag (FT-299; the owner signs) | — | — | Required at G-M1 |

## 5. Performance (GP-04)

Not measured at the gate's classes: perf claims stay "not measured (milestone rule)" until the milestone run, and the
hard criteria AC-17, G-M1-11 and G-M1-12 name the BM-class runner, which does not exist (DEC-034). The nightly hosted
report (class S, against the informational developer-machine baseline of ID-411) is attached as information only:
`<perf-report artefact, run id>`. The last measured state of the gate's classes is the G-SP spike report (ID-218).

## 6. Risks (GP-06)

| Risk | Indicator at the readiness run | Owner (role) |
|---|---|---|
| R-018 interactive latency at L and XL | The G-SP re-test not yet run; no BM-class runner | Core developer |
| R-045 LG-1 missing | No statement on file; hard at G-M1; refusal is KT-2 | Steward liaison |
| R-046 LG-5 missing | Overdue; the content is public | Steward liaison |
| R-050 plugin exception wording | ADR-0025 Proposed; counsel not engaged | Project owner |
| R-053 steward and signing identity late | ADR-0030 Proposed; no enrolment submitted; 12-month lead time not started | Project owner |
| R-014 bus factor in the core crates | One named code owner (G-M1-16) | Project owner |

## 7. Legal gates (GP-08)

| Legal gate | Required at G-M1 (§8.4 of the gates) | State | Evidence on file |
|---|---|---|---|
| LG-1 employer statement | **Hard** | Open | `<none>` |
| LG-2 exceptions adopted | **Closed** before the first non-maintainer merge | Open; the `legal-docs` merge block enforces it | `<none>` |
| LG-3 trademark clearance | Report on file | Search not commissioned | `<none>` |
| LG-4 steward and identity | **Decision** (ADR-0030 Accepted); enrolment plan | Open | `<none>` |
| LG-5 fixture consent | (G-M0) | Open, overdue | `<none>` |
| LG-6 steward review | — | Not triggered | — |
| ID-383 CC-BY-SA symbols in the GPL binary | Counsel's confirmation with the LG-2 brief | Open; doc 19 §7 row 3 marked **verify** | `<none>` |

## 8. Pilot (GP-09)

| Item | State |
|---|---|
| Kick-off (WP-S0.08) | Not held; 2 of 12 P1 questions answered (PQ-01, and PQ-02 in part; ID-272), 19 P2 questions open |
| Golden acknowledgement (G-M1-09) | Not on file; FT-29, FT-40, FT-63 wait for it; the change list is FT-300 |
| First PNG approval (FT-10) | Not given; the candidates' known gaps are listed in the golden README |
| Side-by-side review (G-M1-23) | Not held; it judges content against the reference set, the deviations of golden §3.4 being intended (ID-272, ID-340) |
| Pilot log | Not started |
| The pilot's own projects | Private test input (ID-299): three projects import with no lost connection (ID-273), all in class S (ID-272); counts only, never named |

## 9. Demo (GP-12)

On the gated commit, from the command line with no GUI: `ew export pdf golden/J-E-2416.ew` writes the 36-page PDF
whose SHA-256 equals `expected/pdf/sha256.txt`; the scripted edits of `variants/ac16-freshness.ewcmd` through
`ew apply` (rename T55, move -102F2, renumber folio 103 to 104, change the 24 V colour) each change every affected
output in one commit, and undo restores the bytes; the D-01 replay renumbers folio 3 to 4 and writes only that page's
file; `ew verify-derived` reports no difference. Golden B's two command logs show the previews of AC-18.

## 10. Gate record draft (GP-14)

### 10.1 Outcome

| Field | Value |
|---|---|
| **Outcome** | `<Go / Go with conditions / Recycle / Descope / Stop>`; readiness-run recommendation: do not convene until the date rule fires |
| **Authorises** | The interactive phase as the main effort; external contributions (LG-2); community channels; the re-based plan (PD-11) |
| **Decided by** | Chair: project lead · Owner decision: project owner (PD-7, PD-11, channels) |
| **Dissent** | `<none or by role>` |

### 10.2 Attendance and concurrence

| Role | Present | Concurs on | Concurrence on file |
|---|---|---|---|
| Project owner | `<>` | PD-7, PD-11, channels | `<>` |
| Project lead (chair) | `<>` | — | — |
| Core team | `<>` | — | — |
| Pilot user | `<>` | G-M1-09, G-M1-23 | `<>` |
| Counsel | `<>` | G-M1-18, -19, -20 | `<>` |
| Steward liaison | `<>` | GP-08 | `<>` |

### 10.3 Exit checklist

The 27 rows of the readiness report §4, re-issued at the scope freeze. Counts at the readiness run: H met 4 of 15 ·
M met 1 of 10 (2 with conditions, 1 not applicable) · S met 0 of 2.

### 10.4 Decisions taken or due

| Decision | Status before | Status after | Record |
|---|---|---|---|
| PD-7 / ADR-0030 | Proposed | `<>` | `<>` |
| PD-6 / ADR-0025 (LG-2) | Proposed | `<>` | `<>` |
| PD-11 re-plan (restated by R4) | Open | `<>` | `<>` |
| LG-1 | Open | `<>` | `<statement on file, cited>` |
| LG-3 report | Open | `<>` | `<>` |
| Channels (Discussions) | Not opened | `<>` | `<>` |
| Rulings R1 to R8 | R5 open; the others taken by delegation 2026-10-22, R2 to be confirmed at G-M0 | `<>` | ID-419 to ID-425 |

### 10.5 Conditions (go with conditions only)

| # | Condition | Owner (role) | Due (at most 4 weeks) | Test that closes it | Closed on |
|---|---|---|---|---|---|
| `<>` | `<>` | `<>` | `<>` | `<>` | `<>` |

### 10.6 Forecast and risks

| Item | Value |
|---|---|
| Re-plan (ruling R4) | `<the cycle-based forecast of M-PP and M2 by content, with the owner-side lead times>` |
| Top risks | §6 above |
| Funding | `<per ruling R4>` |

### 10.7 Sign-off (roles)

Chair · Project owner · Pilot user (acceptance items) · Counsel (legal items). Signed statements stay with the
steward; this record cites that they exist.
