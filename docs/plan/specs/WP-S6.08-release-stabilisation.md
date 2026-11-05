# WP-S6.08: MVP release stabilisation and the first release, 0.1.0 (closed specification)

**Status:** steps 1 to 4 built in cycle 52 by the lane S608 (decision log ID-1396), accepted with fixes by review part 2 (ID-1417); steps 5 and 6 (the lock and the pre-run) wait for cycle 53, right before the first candidate, with review part 2's two findings (C52-p2#2, the format surface's line check; C52-p2#3, unknown members in their original line), FT-1489, FT-1490 and the release branch's procedure (`release/0.1`, ID-1412); G3 (LG-1, ID-1401) and G4 (the release manager, ID-1400) on file (ID-1436) · specified in cycle 50 (documents only), an M2 package in two halves: the lane's six build steps of §6,
built whole by one lane (ID-1281) with one hand-back point (§11), about 1.8 pw, and the release cycle of §7 (the
candidates, the checklist, the acceptance run, the fix rounds, the go and the promote), about 4.2 pw; the PERT of 6.0
pw unchanged · **Date:** 2026-11-04 · **Work package:**
[WP-S6.08](../02-work-packages.md#wp-s608-mvp-release-stabilisation-and-the-first-release) · **Decision log:**
ID-1187, ID-1310, ID-1313, ID-1324, ID-1325, ID-1327 · **Part of:** ElektraWatt plan, specifications

**Purpose.** WP-S6.07 built the release workflow, the procedure and the checklist template; WP-S1.17 built the
released-formats list, the steward's check and the fixture corpus job, each waiting for "WP-S6.08 adds 0.1"; nothing
yet names a version other than `0.0.0`, writes the release notes of 0.1.0, guards the released schemas, freezes the
release's project, collects the evidence of the G-M2 items, or fills the checklist for a candidate. The owner moved the
format lock to the 0.1.0 release (ID-1325). This document closes the package to decisions, a build order and
acceptance, against the code at `e89345a2` (§2.1), and lists the owner's items before the first candidate as gate
items, without deciding them (§5). What ID-1187 moved to 1.0 stays there. Its size is an estimate, not a commitment
(ID-1210); every time is not measured (owner's rule). Where this document and a design document differ, this document
is the narrower and wins for the lane; the design documents stay the target and are updated at the design pass (§14).
A choice the lane meets that this document does not settle is escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The owner's items before the first candidate](#5-the-owners-items-before-the-first-candidate)
6. [Build order](#6-build-order)
7. [The release cycle](#7-the-release-cycle)
8. [The texts](#8-the-texts)
9. [Acceptance](#9-acceptance)
10. [Neighbours, files owned, what a lane may not decide, and risks](#10-neighbours-files-owned-what-a-lane-may-not-decide-and-risks)
11. [Size, routing and the hand-back point](#11-size-routing-and-the-hand-back-point)
12. [Follow-ups, proposed](#12-follow-ups-proposed)
13. [Open questions](#13-open-questions)
14. [Documents to update](#14-documents-to-update)
15. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package `v0.1.0` is a published GitHub release whose files are the verified files of its
last candidate, byte for byte, with the release and migration notes `docs/release-notes/0.1.0.md`; the release dossier
`docs/release/dossier/0.1.0/` holds each candidate's filled checklist (RC-01 to RC-10), the acceptance report over every
G-M2 item with its evidence or its declared gap, the translation report, IN-15's and PV-13's manual records and the
links to the drafts, SBOMs and attestations; the workspace is version `0.1.0`, `RELEASED_FORMATS` names `0.1`, the
release's Golden A is frozen in `fixtures/format/0.1/` as the migration test's input, and `cargo xtask api-guard` fails
any non-additive change to the released schemas or to the frozen copy from the release tag on; the release-due follow-ups
are built, re-homed or closed by §4's rulings; and the owner's go is on file.

**Non-goals** (each with its package):
- No signing, no signed tag or sums, no release key, no second approval of release jobs, no reproducible builds, no
  crates.io publishing, no MSI, deb or Flatpak, no update check, no crash upload (WP-S6.10 and 1.0; ID-1187).
- No steward processes, LG-3, LG-4 or the steward checklist (1.0; ID-1187, ID-1188).
- No change to the release workflow, the procedure or the checklist template beyond the dossier pointer (WP-S6.07, as
  built), and none to the installers (WP-S6.06) beyond FT-1267's step.
- No new feature and no routine integration work (the 20 % reserve); a blocker's fix is a fix round (D16).
- No usability round (WP-S2.12 owns R0 and R1; this package fixes R1's severe findings through D16).
- No monthly 0.x after 0.1.0 (WP-S6.09).
- No new Rust dependency; no change to `deny.toml`, `supply-chain/` or `Cargo.lock` beyond the version bump's own lines.

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [Quality](../../design/18-quality-security-release.md) §7.1, §7.2, §7.4, §8, §9.2, §18 | Versions and cadence, the format window, CI-R and RC rows, 0.1.0's artefacts, sums and provenance, the scope by release |
| [ADR-0011](../../adr/0011-format-versioning-and-migration.md) decisions 4 to 6, amendments 1 and 6; [ADR-0023](../../adr/0023-supply-chain-and-release-integrity.md) amendment 7 | The lock at the release, the fixture, the guard; 0.1.0 unsigned from one workflow |
| [Project format](../../design/05-project-format-and-persistence.md) §10.4, §11.2, §11.3 | The fixture corpus, development builds, the merge driver and the manifest merge |
| [Governance](../../design/19-open-source-governance.md) §3.3 | LG-1 at RC-10; "first binary release" |
| [Gates](../03-gates.md) §5.5 (G-M2-01 to G-M2-25), GP-09 to GP-11 | The acceptance run and the dossier |
| [Golden project](../../design/20-golden-project.md) AC-21, AC-23, AC-40, AC-41 | The package's acceptance rows |
| Decisions ID-1187, ID-1188, ID-1310, ID-1313, ID-1324 (its WP-S6.04 half), ID-1325, ID-1327 | Scope, the owner's items, the lock at the release |
| Follow-ups FT-1154, FT-1235, FT-1236, FT-1267, FT-1269, FT-1271, FT-1276, FT-1293, FT-1296; table rows FT-96, FT-403, FT-969 | D3 to D8, D10 |
| Specifications [WP-S6.07](WP-S6.07-release-pipeline.md) §12, §13; [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md) IN-15, §13 O1; [WP-S6.05](WP-S6.05-privacy-and-crash-log.md) PV-13; [WP-S6.04](WP-S6.04-security-engineering.md) (RC-01, RC-04); [WP-S7.05](WP-S7.05-user-documentation.md) D13, D15 | What each hands to the release |

### 2.1 What exists at `e89345a2` (the base contract)

The lane relies on exactly these items, and on every M2 package merged before its dispatch (§6). If one differs, the
lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| The procedure: the owner's settings before the first candidate, the steps before the tag, cutting the candidate, the checklist, the promote (dry, then real), a blocker | `docs/release/README.md:31-106` | §5, §7 |
| The checklist template RC-01 to RC-10 in 0.1.0's form; "who is the release manager at 0.1.0 is the owner's choice" | `docs/release/checklist.md:13-30` | D11, §7 |
| `release.yml` (verify, package, sums, attest, release; the promote by dispatch) and `ci/release-verify.py` (the tag's version is the workspace version; `docs/release-notes/<X.Y.Z>.md` exists and is not empty) | `.github/workflows/release.yml`; `ci/release-verify.py` | §7, D10 |
| The workspace version `0.0.0`; Golden A's manifest stamped `"generator": "ElektraWatt 0.0.0"`, a member its fixture test ignores; the expected XLSX files carry the version in a cell | `Cargo.toml:15`; `golden/J-E-2416.ew/manifest.json:4`; `golden/J-E-2416/README.md:806-807`; `golden/J-E-2416/expected/xlsx/` | D13 |
| `FORMAT = "0.1"`; `RELEASED_FORMATS` empty, "WP-S6.08 adds `"0.1"`"; a store is a release build when its format is released, and a save marks `unstable` when the list is not empty and lacks the format it writes | `crates/ew-schema/src/kind.rs:11`, `:604-606`; `crates/ew-store/src/store.rs:87-95`, `:123-125` | D13 |
| An unstable file opens read-only in a release build or another format's reader | `crates/ew-format/src/version.rs:87-89` | D3, D13 |
| The migration chain is empty; the steward's check fails a released format without `fixtures/format/<f>/`; the corpus job opens, saves and compares each `fixtures/format/<f>/<project>/` with `<project>.expected/`, passing while the corpus is absent | `crates/ew-format/src/migrate.rs:71-74`, `:272-293`; `crates/ew-store/tests/it/format_steward.rs:1-7`, `:63-80` | D13 |
| No `fixtures/` folder at the repository's root | — | D13 |
| The published schemas `schemas/0.1/` (`index.json` with each kind's file, rank, schema, status and `v`) | `schemas/0.1/index.json` | D9 |
| `api-guard`: five surfaces read as data at the merge base and in the tree, breaking or additive, a breaking change passing before 1.0 only with a `- api-break:` line in `unreleased.md`; the CI job checks out the whole history | `xtask/src/api_guard.rs:4-35`; `.github/workflows/ci.yml:137-174` | D9 |
| The manifest merge takes the later `format` and `min_reader` of both sides; the merge driver reads the format from ours' manifest, else the reader's | `crates/ew-format/src/merge.rs:1174`; `bin/ew/src/cli/merge_driver.rs:123-145` | D4 |
| The About dialog's support line: no planned end before 1.0, the text `Support: pre-release build; only the latest 0.x release is supported.` | `crates/ew-viewmodel/src/about.rs:22-23`, `:49-53`; `crates/ew-viewmodel/locales/en-GB/shell.ftl:1614` | D7 |
| `unreleased.md`'s line kinds `api-add`, `api-break`, `dependency`, `text-change`; `m1-draft.md`; no `0.1.0.md` | `docs/release-notes/` | D10 |
| AC-40's installed acceptance on the installed tree; the guide's folder proven present only by listings | `ci/installed-acceptance.sh:5-17`; FT-1267 | D6 |
| The acceptance runner `cargo xtask acceptance [--milestone] <report>` (the `gate` test of `ew-api`'s acceptance binary; `--milestone` adds the performance assertions) | `xtask/src/main.rs:363-368`, `:668-680` | D12 |
| Golden A's expected findings | `crates/ew-api/tests/expected/findings_mvp.csv` | AC-21 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Acceptance |
|---|---|---|
| NFR-REL-11 | **Built:** CI-PR-05's headless export on Golden A without libraries or system fonts; AC-40's installed acceptance (WP-S6.06). Here: run on the last candidate and recorded | RL-13 |
| NFR-DOC-07 | `docs/release-notes/0.1.0.md` with the format, profiles, CLI and API changes and the upgrade steps (D10); `release-verify` refuses a candidate without it (built) | RL-8 |
| NFR-REL-05, NFR-REL-06 (G-M2-11) | The released format's fixture opens (D13); the warning on an unreleased format's save (D3, FT-1154) | RL-2, RL-7 |
| ADR-0011 amendment 6 (ID-1325) | The lock at the release: the guard and the frozen copy (D9, D13) | RL-6, RL-7, RL-15 |
| AC-21, AC-23, AC-40, AC-41 | Green on the last candidate (D12) | RL-13 |
| G-M2-01 to G-M2-25 | The acceptance report, one row each (D12) | RL-9 |
| FT-96, FT-969, FT-1154, FT-1235, FT-1267 | Built (D3 to D6, D10) | RL-2 to RL-5, RL-8 |
| FT-1236, FT-1269, FT-1271, FT-1276, FT-1293, FT-1296, FT-403 | Re-dated, observed, re-homed or already closed (D8) | §12 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Two halves.** The **lane** builds §6's six steps whole and hands back a candidate-ready tip; it is dispatched when every other M2 package is merged (the package's Depends), WP-S7.05 and WP-S2.11 included. The **release cycle** (§7) is the orchestrator's procedure with the owner's acts: CI on the tip, the candidates, the checklist and the acceptance run, D16's fix rounds, the gate review, the go and the promote. Nothing of the cycle is cut into lane parts | A lane cannot push a tag or sign a checklist, and the candidates wait on people; the build is still one lane and one review (ID-1281) |
| D2 | **The procedure as built.** `docs/release/README.md` §2 to §8 is used unchanged; the lane adds one line to its §6 naming the dossier (D11). The candidates are `v0.1.0-rc.<n>`; the owner cuts them and dispatches the promote unless §5 G4 names another | WP-S6.07 built and tested it; the release is its first run |
| D3 | **FT-1154: the warning on an unreleased format's save.** A save whose manifest is written with `unstable: true` reports it: the window shows S1 as the save's toast, every time; `ew` prints S2 on standard error after the save, exit code unchanged. The store returns the flag with the save's result; nothing else changes. Tested with a store whose `released` list lacks its format | ADR-0011 decision 5 and doc 05 §10.4; after the lock a development build that moves the format must say so on every save |
| D4 | **FT-1235: a newer side is not merged.** When the ancestor's, ours' or theirs' manifest names a format newer than the reader's `FORMAT`, `ew merge-driver` merges nothing in that project: `%A` stays ours, it prints S3 and exits 1 (its meaning "left as ours, theirs stays in git's index", ID-1273); the manifest merge never takes a format the reader cannot read | The maximum of D16 with a side the reader cannot type would stamp records merged as the older format with the newer one |
| D5 | **FT-96: proven before it is built.** A test in `ew-store` writes a project of format 0.1 whose record lines carry an appended member the reader does not know (as a later additive release would write it), opens it, edits another record, saves, and asserts the untouched lines byte for byte. If it passes, FT-96 closes on that test; if it fails, the lane makes untouched records keep their original line (ADR-0011 amendment 1) in this step | The 0.1.0 reader is the one that will meet later members and cannot be changed after the release; the test shows whether the canonical writer already keeps such lines |
| D6 | **FT-1267: the installed guide is found.** A headless step on the installed tree (beside `ci/installed-acceptance.sh`, in `package.yml`'s install tests) runs the installed `elektrawatt --headless --script` with Help ▸ Documentation and checks that the request resolves to `docs/index.html` of the installed tree and that the file exists, with `docs/nl/index.html` when WP-S7.05's Dutch build is present | The listings prove the folder is there, not that the program finds it |
| D7 | **The About dialog's support line at 0.1.0.** `about-support-prerelease` reads S4 (no "pre-release build": 0.1.0 is a release); nl-NL with it. Before 1.0 no build has a planned end, so the line names the rule, as `SECURITY.md` does (ID-1324: 0.1.0 supported until the next minor) | A released build must not call itself a pre-release |
| D8 | **The rows not built here.** **FT-1236** is re-dated to the first new format version: at 0.1.0 no older released format exists, so no side needs migrating. **FT-1293** and **FT-1296** move to WP-S6.09 and stand in the notes' known limitations (§13 O5). **FT-1269** is watched: it closes at the go when IN-8 is green on the last candidate, and a subtree is added only with the run that shows it (ID-1283). **FT-1271**: `NOTICE` keeps mimalloc (built); telling upstream is a public act from the owner's account (§5's list). **FT-1276** closes with the design pass after the lane, which writes WP-S6.06's documents with this package's (§14). **FT-403** closed in cycle 48 (`FT-Closes: FT-95, FT-403`); nothing is left | Each row's due date asked for a ruling at the release; only the rows that the 0.1.0 binary needs are built |
| D9 | **The format guard: a sixth `api-guard` surface, `format`.** For each folder `schemas/<f>/` its base is the oldest final release tag (`v<X.Y.Z>`, no `-rc`) whose tree holds `schemas/<f>/index.json`; with none, the format is not released and nothing is compared (the guard prints "format <f>: not released; nothing to compare"). Against the base, **breaking** (always fails, no `api-break` line lets it pass): a schema file or a kind removed; a property removed or its `type` changed; a property newly required; an enum value or a `oneOf`/`anyOf` branch removed; a kind's `v`, `rank` or `file` changed, or the index's `format`; any byte of `fixtures/format/<f>/<project>/` changed (the `.expected` folders may change). **Additive with a line**: a new enum value or branch passes only when `unreleased.md` gains `- format-variant: <kind> <path> <value>` in the same diff (the reviewer checks the type's `Unknown(Value)` arm, ADR-0011 amendment 1). **Additive:** a new optional property, a new kind's schema. A new format version writes `schemas/<new>/` and leaves the released folder unchanged | ADR-0011 amendment 6's guard, read from the release itself, so no hand-kept list of tags; the schemas cannot show an `Unknown` arm, so a new variant asks for a line a reviewer reads; api-guard's job already has the whole history |
| D10 | **The release and migration notes.** `docs/release-notes/0.1.0.md` (CC-BY-4.0), in this order: what 0.1.0 is (the first release, unsigned, Windows and Linux, for the pilot); the downloads and how to verify them (the guide's "Verify a download"); what it contains (by area, from the M0 to M2 packages); **the format**: it writes and reads format 0.1, which is locked from this release (ADR-0011 amendment 6), with what that means for later releases; the derivation and check profiles it ships (WP-S7.03's colour, size and address profiles; the check schemes); the CLI and API: every `api-add`, `api-break`, `dependency`, `text-change` and `behaviour-change` line of `unreleased.md` folded under its surface, all surfaces Preview (doc 17 §11); **upgrade steps**: none from an earlier release; projects of pilot-preview builds as §13 O4 rules; known limitations (FT-1293, FT-1296, unsigned binaries, the 1.0 items of ID-1187); the notices of the AppImage's runtime (ID-1275). **FT-969:** `unreleased.md` gains the line kind `behaviour-change:` and a one-line legend of the kinds at its head; after the promote it is emptied to that head (§7 step 9). The book shows the notes through WP-S7.05's D13 | NFR-DOC-07's list and RC-08; the line kinds already sort the changes by surface |
| D11 | **The release dossier** `docs/release/dossier/0.1.0/` (CC-BY-4.0): `README.md` (the candidates with commit, draft and result; the go); `checklist-rc.<n>.md` per candidate, copied from `docs/release/checklist.md` and filled; `acceptance.md` (D12); `translation.md` (WP-S7.05's report on the candidate's commit); `manual.md` (IN-15, PV-13 and RC-05's other tests: build, system, steps, result); links to the drafts, the SBOMs and the attestations. Names are roles only ("the owner", "the release manager", "the tester"), never a person's name; addresses, machine names and serial numbers stay out | GP-11 in the repository, where the gate reads it; the repository is public |
| D12 | **The acceptance run.** `acceptance.md` has one row per G-M2-01 to G-M2-25 and per AC-21, AC-23, AC-40, AC-41 and NFR-REL-11: the criterion (by reference), its evidence (a CI run's link and job, a test, a report, a manual record), the result (met, met with a declared gap, not met, not applicable by its ruling). §7's table gives each row's evidence. A row is "met" only with its evidence on the last candidate's commit; a timing criterion reads as §13 O3 rules; a row whose evidence is a person's (R1, the pilot) is filled from that report | G-M2's "acceptance of every reached milestone row"; one row per item keeps nothing implicit |
| D13 | **The lock.** One commit, the last before `v0.1.0-rc.1`: `[workspace.package].version = "0.1.0"`; `RELEASED_FORMATS = &["0.1"]`; `fixtures/format/0.1/J-E-2416.ew/`, a byte copy of Golden A's committed files (read, never opened in a writing or locking mode), and `J-E-2416.ew.expected/`, the corpus job's full save of a temporary copy, whose record files equal the fixture's byte for byte (only `manifest.json` may differ); the expected files that carry the version (the XLSX cells, any recording) re-blessed, and Golden A's own files left as committed, since its fixture test ignores the `generator` stamp (`Golden:` trailer with the ID the brief gives). A blocker fixed before the go that changes the format re-does the copy and its expected folder in the fix's commit (the format is not released until the promote, ID-1325); after the promote the copy never changes (D9) | ADR-0011 amendment 6: Golden A as the release ships it is the fixture; the steward's check and the corpus job were built to take exactly this; the candidate's bytes must already be a release build's |
| D14 | **The tag arms the guard.** No flag is flipped after the promote: D9's base is the tag `v0.1.0` once it exists, so the next pull request is compared with it | The release itself is the switch |
| D15 | **The manual checks.** IN-15 (the four desktop installs, each with the window, the sample and the guide) and PV-13 (on Windows, a window ended in Task Manager with a dirty project is offered at the next start; `icacls` shows the state directory's per-user ACL), with RC-05's RDP, NVDA, Orca, two printers and Windows network observation, run once on the last candidate's artefacts by the tester the owner names, recorded in `manual.md` | They need desktops and people; once per release, on the bytes that ship |
| D16 | **Fix rounds.** A blocker found on a candidate, a severe finding of R1 (Nielsen 3 or 4, fixed with a kittest flow, G-M2-20), and a pilot blocker are each a fix round (ID-1281's rule), routed by the orchestrator to the package that owns the code; a fix in this package's files resumes this package's author while its context is under about 750k (ID-1322). Each lands on `main` before the next candidate | One owner per fix, the lane's own rule for fixes |
| D17 | **No 1.0 item enters** (ID-1187): §1's non-goals | — |
| D18 | **Every item closed;** no decision changes an ADR (amendment 6 is applied) | — |

## 5. The owner's items before the first candidate

These are the owner's. This document decides none of them; each is a gate item of the first candidate, and the
orchestrator asks for each before the lane's tip is tagged.

| # | Item | Source | Checked by | Blocks |
|---|---|---|---|---|
| G1 | The repository settings: (a) the environment `release`, deployment limited to `v*` tags and `main`, no reviewer at 0.1.0; (b) a tag ruleset on `v*` (maintainers create, nobody updates or deletes, the workflow's token creates the final tag); (c) immutable releases on, if offered; (d) the default workflow token read-only; (e) the required checks with `c-only` and `api-guard` | WP-S6.07 §13 O1; ID-1310 | `docs/release/README.md` §2, by the release manager before the tag | The first candidate |
| G2 | Whether ID-723's AI-performed audits and recorded exemptions cover 0.1.0, a public binary release | WP-S6.07 §13 O2 | The owner's statement in the dossier | The go |
| G3 | LG-1: the employer copyright statement, or the statement that no employer's staff contributed | WP-S6.07 §13 O3; RC-10; ID-1203 | RC-10 | The go |
| G4 | Who cuts the candidates, signs the checklist as release manager and dispatches the promote | WP-S6.07 §13 O4; `docs/release/checklist.md:29-30` | The dossier's `README.md` | The first candidate |
| G5 | Private vulnerability reporting switched on in the repository's settings, so `SECURITY.md`'s channel works (the owner's acceptance of WP-S6.04's O1, ID-1324) | ID-1324; WP-S6.04 D11 | RC-04's check reads `SECURITY.md` and the setting | The first candidate |

The owner's acts later in the cycle, also never decided here: the tester of RC-05 (D15); the gate review and the go
(G-M2's decision maker); the pilot's concurrence and R1's participants (WP-S2.12); FT-1271's upstream report; §13 O3
and O4.

## 6. Build order

One lane builds the six steps in this order and commits at the end of each, after every other M2 package is merged.

1. **The fixes due at the release** (D3 to D7): FT-1154 with S1 and S2; FT-1235 with S3; FT-96's test (and the
   original-line rule if it fails); FT-1267's installed step; S4. The nl-NL texts listed for the orchestrator.
2. **The format guard** (D9): the `format` surface in `api-guard` with S5 to S7, its unit tests on a temporary
   repository with and without a release tag, `CONTRIBUTING.md`'s line on `- format-variant:`.
3. **The release notes** (D10): `0.1.0.md` from `unreleased.md` and the delivered packages; FT-969.
4. **The dossier** (D11, D12): the folder, its `README.md`, `acceptance.md` with every row and its evidence source,
   `manual.md`'s form, the pointer in `docs/release/README.md` §6.
5. **The lock** (D13): the version, `RELEASED_FORMATS`, `fixtures/format/0.1/`, the re-blessed files, the steward's
   check and the corpus job green.
6. **The lane's acceptance pre-run**: the targeted runs a lane may make on its tip (`cargo xtask acceptance`'s report
   without `--milestone`, the steward and corpus tests, `api-guard`, `release-verify.py --test`), their results entered
   in `acceptance.md` as "lane tip", the rows that need CI or people left for §7.

## 7. The release cycle

After the lane is merged. Each step names who acts; nothing here is a lane's.

1. **The owner's items** G1 to G5 (§5) on file. *The owner.*
2. **CI on the tip:** `ci.yml`, `deep-nightly.yml` and `package.yml` green on the commit (the procedure's §3). *The
   orchestrator.*
3. **The candidate:** the tag `v0.1.0-rc.<n>` on that commit; `release.yml` builds the draft. *G4's person.*
4. **The checklist** `checklist-rc.<n>.md`, each item with its evidence (agents prepare the evidence; the release
   manager signs). *G4's person.*
5. **The acceptance run** (D12), the rows from that candidate's runs:

   | Row | Evidence |
   |---|---|
   | G-M2-01 | CI-N-04's digests from both OSes on the commit |
   | G-M2-02 | CI-PR-03 and CI-PR-07 (WP-S2.11's flows); the 16 ms frame bound as §13 O3 rules |
   | G-M2-03, AC-21, AC-23 | The acceptance runner's report; the release kill-point test |
   | G-M2-04, AC-40, AC-41 | The runner's report; CI-R-10's installed acceptance |
   | G-M2-05 | The runner's report per D-nn |
   | G-M2-06 | CI-N-02 (the command fuzzer) in the deep nightly; Golden B |
   | G-M2-07 | §13 O3 |
   | G-M2-08 | CI-N-05 (WARP, CPU renderer); RC-05's RDP run; the 30 fps bound as §13 O3 rules |
   | G-M2-09 | CI-N-07, CI-PR-11 on ext4 and NTFS; APFS not applicable (no macOS, ID-880) |
   | G-M2-10 | CI-PR-07 (the name walker, keyboard flows, lints) |
   | G-M2-11 | CI-PR-14 with the `format` surface; the corpus job with `fixtures/format/0.1/` |
   | G-M2-12 | `translation.md` (WP-S7.05 D15), or Weblate's statistics if a project exists |
   | G-M2-13 | The candidate's `release.yml` run (CI-R-01 to CI-R-11 in 0.1.0's form) and IN-15 |
   | G-M2-14 | CI-R-02's log; the deep nightly's fuzz; WP-S6.04's hostile corpus test |
   | G-M2-15 | CI-N-06 (the network sandbox); CI-R-09's listening-socket check |
   | G-M2-16, G-M2-18, G-M2-19, G-M2-25 | Not applicable by their rulings (ID-454, ID-1187) |
   | G-M2-17 | The filled checklist |
   | G-M2-20, G-M2-21 | The R1 report (GP-10, WP-S2.12) |
   | G-M2-22 | The R0 report and its closed fixes |
   | G-M2-23 | WP-S7.05's acceptance; `docs-book` on the commit |
   | G-M2-24 | The pilot report (GP-09) |
   | NFR-REL-11 | CI-PR-05 on the commit |

6. **A blocker:** a fix round (D16) on `main`, then the next candidate from step 2. *The orchestrator, the fix lane.*
7. **The gate review** G-M2 (3 hours on the candidate, 30 minutes' go or no-go). *The chair; the go is the owner's.*
8. **The promote:** the dry run, then the real one, from the last candidate. *G4's person.*
9. **After the promote:** the dossier's `README.md` records the go and the published release; `unreleased.md` is
   emptied to its head (D10); FT-1269 closes; the next pull request's `api-guard` names `v0.1.0` as the format's base
   (RL-15); the records. *The orchestrator.*

## 8. The texts

Every text is asserted whole (en-GB); the nl-NL texts are the orchestrator's (ID-1320), written by the lane and listed
for the orchestrator's read.

| # | Where | Text |
|---|---|---|
| S1 | The window, the toast after a save that wrote an unreleased format | `Saved in a format that is not released yet. Released versions of ElektraWatt open this project read-only.` |
| S2 | `ew`, standard error after such a save | `warning: saved in a format that is not released yet; released versions of ElektraWatt open this project read-only` |
| S3 | `ew merge-driver`, a side newer than the reader (exit 1) | `merge-driver: <path>: written by a newer ElektraWatt (format <f>; this one reads <r>); left as ours: merge it with that version` |
| S4 | About, the support line before 1.0 (`about-support-prerelease`) | `Support: only the latest 0.x release is supported, until the next minor release.` |
| S5 | `api-guard`, a breaking change to a released format | `format <f>: <item>: <change> since <tag>; a released format changes only additively (ADR-0011 amendment 6)` |
| S6 | `api-guard`, a new value without its line | `format <f>: <item>: the new value <value> since <tag> needs the line "- format-variant: <item> <value>" in docs/release-notes/unreleased.md` |
| S7 | `api-guard`, a changed fixture | `format <f>: fixtures/format/<f>/<file> changed since <tag>; a released format's fixture never changes` |

## 9. Acceptance

- **RL-1** (§5) G1 to G5 are on file in the dossier before the first candidate's tag (named manual check, the owner).
- **RL-2** (FT-1154, NFR-REL-05) With a `released` list that lacks the store's format, a save in the window shows S1
  and `ew` prints S2, both whole; with the format released, neither; the project's manifest carries `unstable: true` in
  the first case only (`ew-store`, `ew-viewmodel`, `ew` tests).
- **RL-3** (FT-1235) A merge whose theirs names a format newer than the reader's prints S3, exits 1 and leaves `%A`
  byte-equal to ours (`ew`'s `cli` test); the manifest merge of two readable sides is unchanged.
- **RL-4** (FT-96) D5's round-trip test passes.
- **RL-5** (FT-1267) The installed step passes in `package.yml` on both OSes (the orchestrator's dispatch on the lane's
  tip).
- **RL-6** (D9) On a temporary repository: each breaking change of D9's list fails with S5, a new enum value without
  its line with S6 and with it passes, a changed fixture with S7, a new optional property passes, and without a
  release tag the guard prints the "not released" note and passes (`xtask` tests); `api-guard` is green on the tip.
- **RL-7** (D13, ADR-0011 amendment 6) `RELEASED_FORMATS` is `["0.1"]`; the steward's check and the corpus job pass with
  `fixtures/format/0.1/`; a release-build save writes no `unstable`; `ew --version` and `elektrawatt --version` print
  `0.1.0` and the setup's `DisplayVersion` is `0.1.0` (WP-S6.06 IN-6, IN-8 on the `package.yml` run).
- **RL-8** (NFR-DOC-07, RC-08, FT-969) `0.1.0.md` has D10's sections and every line of `unreleased.md` under its
  surface; `release-verify.py` accepts it; `unreleased.md` has the `behaviour-change:` kind; `doccheck.py` passes on
  both.
- **RL-9** (G-M2) `acceptance.md` has a row for each item of D12, each with its evidence or its declared gap;
  `doccheck.py` resolves every gate ID and link.
- **RL-10** (G-M2-13; WP-S6.07 RP-3's tag half, RP-4 to RP-6) The candidate's `release.yml` run is green in every job
  and its draft holds the files of `docs/release/README.md` §1 (a CI run on the tag).
- **RL-11** (G-M2-17) RC-01 to RC-10 are filled for the last candidate, each with evidence and the signing role (named
  manual check, the release manager).
- **RL-12** (IN-15, PV-13, RC-05) `manual.md` records each test with the build it ran on (named manual check, the
  tester).
- **RL-13** (AC-21, AC-23, AC-40, AC-41, NFR-REL-11) Green on the last candidate's commit (the runner's report, CI-R-10,
  CI-PR-05).
- **RL-14** The promote: the dry run's output, then the published `v0.1.0` with the candidate's files, verified by the
  `release` job (the CI run of the dispatch; the owner's go on file).
- **RL-15** (D14) The first `api-guard` run after the promote reports `v0.1.0` as format 0.1's base (a CI run).
- **RL-16** The hand-back checks of the lane's package set pass (fmt, settle, clippy, doc, nextest, policy, trailers
  with `Golden:`).

## 10. Neighbours, files owned, what a lane may not decide, and risks

### 10.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S6.07 | `release.yml`, the procedure, the checklist template | Runs them; adds the dossier pointer |
| WP-S6.06 | `package.yml`, the artefacts, the install tests, ID-476's hold (§13 O2) | Adds FT-1267's step |
| WP-S1.17 | The released-formats list, the steward's check, the corpus job, the merge driver | Fills the list and the corpus; FT-1235, FT-96 |
| WP-S6.04 | `SECURITY.md`, the threat model | RC-01, RC-04 read them |
| WP-S7.05 | The guide, the translation report | RC-06; the notes in the book |
| WP-S2.11, WP-S2.12 | The kittest flows; R0, R1 | G-M2-02, G-M2-10; G-M2-20 to G-M2-22 and D16's R1 fixes |
| WP-S6.09 | The 0.x releases after 0.1.0 | Takes FT-1293 and FT-1296 (§13 O5) |

### 10.2 Files owned

`Cargo.toml` (the version); `crates/ew-schema/src/kind.rs` (`RELEASED_FORMATS`); `fixtures/format/**` (new);
`golden/**` and the other expected files the version stamp reaches (re-blessed, listed in the commit);
`crates/ew-store/` (the save's flag, D5's test), `crates/ew-format/src/merge.rs` (D4), `bin/ew/src/cli/` (S2, S3),
`crates/ew-viewmodel/` (S1, S4: the toast, the strings, both locales) and their tests; `xtask/src/api_guard.rs` and its
tests; `ci/installed-acceptance.sh` or a sibling script and `.github/workflows/package.yml` (D6's step);
`docs/release-notes/0.1.0.md` (new), `docs/release-notes/unreleased.md` (the head and FT-969);
`docs/release/dossier/**` (new), `docs/release/README.md` (one line); `CONTRIBUTING.md` (one line). Read-only:
`release.yml`, `checklist.md`, `deny.toml`, `supply-chain/`, every other document. A fix round of D16 owns what its
brief names.

### 10.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. the corpus job's save of the frozen copy differs from it beyond `manifest.json`, or Golden A does not open as a
   release build's project;
2. FT-96's fix would change the canonical form of an edited record, or of any line but untouched ones;
3. a G-M2 row has no evidence it can name: the row is "not met" or a declared gap, never marked met;
4. a step needs a tag, a release, a GitHub setting or an owner's statement (§5);
5. a text would differ from §8, a new dependency seems needed, or a change reaches a neighbour's files beyond §10.2;
6. the version bump changes an expected file whose change is not the stamp.

The lane decides freely: the notes' wording within D10, the dossier's layout within D11, the guard's item names, the
test fixtures, the installed step's script name, and the nl-NL wording (listed for the orchestrator).

### 10.4 Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The candidates wait on the owner's items | Certain · medium | §5 is asked before the lane's tip is tagged; nothing the lane builds waits on it |
| R2 R1's method is not ruled (ID-1327), so G-M2-20 and G-M2-21 have no evidence | High · high | The row reads what WP-S2.12 delivers; the go is the owner's with the row as it stands (§13 O8) |
| R3 A blocker after the lock changes the format | Medium · medium | D13's re-do in the fix's commit, with a `Golden:` trailer; the format is not released until the promote |
| R4 ID-476's hold keeps the installers out of the release files | Medium · medium | §13 O2; the release files are then the CLI archives, `SHA256SUMS` and the SBOMs (ID-1313) |
| R5 The version bump re-blesses more files than the stamp | Medium · low | §10.3 item 6; every re-blessed file listed |
| R6 A checkout without tags runs the guard as "not released" | Low · medium | The CI job checks out the whole history with tags; the note names the missing base |

## 11. Size, routing and the hand-back point

**Size.** The lane about 1.8 pw: step 1 0.45, step 2 0.35, step 3 0.3, step 4 0.25, step 5 0.3, step 6 0.15. The
release cycle about 4.2 pw: about three candidates with their checklists and acceptance runs, the manual checks, and
the fix rounds of blockers, R1's severe findings and the pilot's blockers, whose number is unknown until R1 and the
candidates. The PERT of 6.0 pw (O 3.8 · L 5.8 · P 8.8) stays. **De-scope rule** for the lane: when it is projected
more than 20 % over, FT-1267's step leaves as a follow-up without a further ruling (IN-15's manual install shows the
guide).

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `high`** (the format lock, the guard and the merge
driver's refusal: format work routes at `high`), dispatched after every other M2 package is merged; the brief gives the
`Golden:` ID of step 5. The fix rounds are routed by the orchestrator as D16 says.

**Hand-back point** (ID-1322). The lane hands back at the end of **step 4**, before the lock, when it then stands above
about 550k tokens; one that nears 800k anywhere commits and hands back. Its hand-off note then holds: the steps done
with their commits; FT-96's test result and whether the original-line rule was built; the guard's item forms and the
tags its tests use; the notes' sections and the `unreleased.md` lines folded; the dossier's rows and their evidence
sources; the files the version bump will reach (from a dry search of the stamp); the test filters (`ew-store`'s
`format_steward` and the save tests, `ew`'s `cli` merge-driver tests, `xtask`'s `api_guard` tests, `ew-viewmodel`'s
about and save tests); and every choice of §10.3's free list already made.

## 12. Follow-ups, proposed

The design pass opens the rows. **FT-96**, **FT-969**, **FT-1154**, **FT-1235** and **FT-1267** close with steps 1 and
3; **FT-1269** at the go; **FT-1276** with the design pass; **FT-1236** is re-dated to "the first new format version";
**FT-1293** and **FT-1296** move to WP-S6.09 (§13 O5).
- **FP-1** The release notes drafted by git-cliff from the commits (doc 18 CI-R-01), when the history's subjects are
  regular enough to need only editing.
- **FP-2** RC-03's waiver expiry and RC-04's table checked by a script (WP-S6.07's FP-2).
- **FP-3** A per-release dossier generator that fills `acceptance.md`'s CI rows from the runs' API.

## 13. Open questions

- **O1** The package's shape (D1, D16). Options: (1) as D1: one lane for the build, the release cycle as the
  orchestrator's procedure, blockers as fix rounds routed to their owners; (2) the WP-S6.08 lane stays open through the
  candidates, resumed per blocker while its context allows. Recommendation: (1); a fix belongs with its code's owner,
  and the lane's own files are resumed as ID-1322 allows.
- **O2** ID-476's distribution hold (WP-S6.06 §13 O1) is not ruled: until it lifts the setup and the AppImage are not
  release files (ID-1313). Options as WP-S6.06 gives them; recommendation: its (1), the hold binding the STEP worker's
  spawn only, ruled before the first candidate so that 0.1.0 ships its installers.
- **O3** (the owner's) Timing criteria at the release: G-M2-07's performance gate, G-M2-02's 16 ms frame bound and
  G-M2-08's 30 fps. Options: (1) recorded "not measured (owner's rule)", the go decided without them; (2) one measured
  run on the last candidate (`cargo xtask acceptance --milestone`, the nightly performance report), reported, not
  gating; (3) gating as written. Recommendation: (2), the owner's rule placing performance at the release.
- **O4** (the owner's) Projects from pilot-preview builds. The G-PP record has no preview build handed to the pilot, and
  before the release the format changed without migrations (ID-1325). Options: (1) none exist: the notes say projects of
  development builds are not supported; (2) some exist: converting them before the lock is a pilot blocker (D16).
  Recommendation: (1), unless the owner knows of such projects.
- **O5** FT-1293 (online-only placeholders) and FT-1296 (the rest of `ew check`'s flags) to WP-S6.09, listed as known
  limitations. Options: (1) re-home both; (2) build either in step 1. Recommendation: (1); neither is a G-M2 criterion,
  and both are additive in a 0.x.
- **O6** D9's line for a new enum value. Options: (1) as D9, a `- format-variant:` line a reviewer checks; (2) a new
  value is additive with no line. Recommendation: (1); an old reader without an `Unknown` arm fails on it, and the
  schemas cannot show the arm.
- **O7** The size: the lane 1.8 pw and the cycle 4.2 pw, the PERT of 6.0 pw unchanged. Recommendation: keep it.
- **O8** R1's method waits for a ruling (ID-1327). Options: (1) G-M2-20 and G-M2-21 read whatever WP-S2.12 delivers and
  the go weighs them; (2) the release waits for R1 with people as G-M2-20 is written. Recommendation: rule WP-S2.12's
  method first; this package takes either.

## 14. Documents to update

At the design pass after the lane: doc 18 §7.1 (0.1.0 released; the format locked from it), §7.2 (before 1.0 after the
lock: additive within 0.1 or a new format with its migration; the `format` guard), §7.4 (the dossier; RC rows as filled);
doc 05 §10.4 (the corpus as built, `RELEASED_FORMATS` with 0.1, FT-1154's warning), §11.3 (a newer side not merged,
FT-1235); doc 17 §11's surfaces list (the `format` surface of the guard); ADR-0011 amendment 6 (applied at `v0.1.0`);
02c NFR-REL-05 (as built); `docs/release/README.md` (the dossier, by the lane); doc 03 §5.5 (the gate record of G-M2);
and, closing FT-1276, WP-S6.06's documents: doc 18 §7.4 (CI-R-05 and CI-R-10 in 0.1.0's form), §16 items 11 and 12,
§5.2 (CI-N-04, CI-N-05, CI-W-03), §6.3 (the issue form) and open question 10, and doc 20 §21 (AC-40's evidence).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S6.08, WP-S6.07, WP-S6.06, WP-S6.05, WP-S6.04, WP-S6.09, WP-S1.17,
  WP-S7.05, WP-S2.11, WP-S2.12; FT-96, FT-403, FT-969.
- [Follow-ups](../follow-ups.md): FT-1154, FT-1235, FT-1236, FT-1267, FT-1269, FT-1271, FT-1276, FT-1293, FT-1296.
- [Decisions](../decisions.md): ID-1187, ID-1188, ID-1203, ID-1210, ID-1275, ID-1281, ID-1310, ID-1313, ID-1320,
  ID-1322, ID-1324, ID-1325, ID-1327; [decision log](../06-decision-log.md): ID-454, ID-476, ID-723, ID-880.
- [Quality](../../design/18-quality-security-release.md) §7.1, §7.2, §7.4, §8, §9.2, §18;
  [project format](../../design/05-project-format-and-persistence.md) §10.4, §11.2, §11.3;
  [governance](../../design/19-open-source-governance.md) §3.3; [golden project](../../design/20-golden-project.md)
  AC-21, AC-23, AC-40, AC-41; [non-functional requirements](../../design/02c-nonfunctional-requirements.md)
  NFR-REL-05, NFR-REL-11, NFR-DOC-07.
- [ADR-0011](../../adr/0011-format-versioning-and-migration.md), [ADR-0023](../../adr/0023-supply-chain-and-release-integrity.md).
- [Gates](../03-gates.md) §5.5, GP-09 to GP-11; `docs/release/README.md`, `docs/release/checklist.md`; the
  specifications of [WP-S6.07](WP-S6.07-release-pipeline.md), [WP-S6.06](WP-S6.06-pilot-installers-and-platform-matrix.md),
  [WP-S6.05](WP-S6.05-privacy-and-crash-log.md), [WP-S6.04](WP-S6.04-security-engineering.md) and
  [WP-S7.05](WP-S7.05-user-documentation.md).
