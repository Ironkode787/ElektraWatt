# WP-S1.18: revisions, release gate and as-built archive, the remaining scope (closed specification)

**Status:** specified in cycle 36 (documents only), an M2 package, for what remains after the typed revision of cycle
17, in five parts: part 1 the revision lifecycle data and commands (0.6 pw, D2 to D5), part 2 page digests, the page
change status and the revision overview (0.8 pw, D6, D7), part 3 the content digest, sign-off and the release gate
(0.8 pw, D8 to D10), part 4 the atomic release with the as-built archive (1.1 pw, D11 to D13), part 5 the archive's
use, the dialogs and AC-23 (0.5 pw, D14, D15); part 1 first; part 2 after part 1; part 3 after parts 1 and 2 and
WP-S1.16 part 5 (waivers); part 4 after part 3; part 5 last · **Date:** 2026-10-30 · **Work package:**
[WP-S1.18](../02-work-packages.md#wp-s118-revisions-release-gate-and-as-built-archive) · **Decision log:** ID-836 ·
**Part of:** ElektraWatt plan, specifications

**Purpose.** Cycle 17 typed the revision table (`revision`, rank 60) with EW-INV-016 to EW-INV-020 and the command
`revision.create` (FT-165; decision log ID-427 to ID-433). Everything that makes a revision managed is still open:
the label format, the open revision that appears with the first edit after a release, the page change status by
digest, the revision overview 911, the four-eyes sign-off bound to the saved content, the release gate, the atomic
release that writes and verifies the as-built archive, and reproduction from it (FT-207, FT-302, FT-303, FT-402,
AC-23). This document specifies that remainder against the code at `adb0df1a` (§2.1). Where it and a design document
differ, this document is the narrower and wins for the lanes; the design documents stay the target, and the
documents of §12 are updated at the design pass after each part. A choice a lane meets that this document does not
settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the revision lifecycle data and commands](#5-part-1-the-revision-lifecycle-data-and-commands)
6. [Part 2: page digests, change status and the revision overview](#6-part-2-page-digests-change-status-and-the-revision-overview)
7. [Part 3: content digest, sign-off and the release gate](#7-part-3-content-digest-sign-off-and-the-release-gate)
8. [Part 4: the atomic release and the as-built archive](#8-part-4-the-atomic-release-and-the-as-built-archive)
9. [Part 5: the archive's use, the dialogs and AC-23](#9-part-5-the-archives-use-the-dialogs-and-ac-23)
10. [Files owned, what a lane may not decide, risks](#10-files-owned-what-a-lane-may-not-decide-risks)
11. [Size, order and routing](#11-size-order-and-routing)
12. [Follow-ups and the documents to update](#12-follow-ups-and-the-documents-to-update)
13. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package a project's revisions carry one label format; the first content edit after a
release opens the next revision in the same undo step; each page shows the revision it last changed in, from
rendered-content digests; folio 911 lists the revisions with their changed pages; a checker other than the author
approves the saved content with `revision.approve`, and any later content edit voids it; `revision.release` refuses
with the release scheme's blockers, or freezes, saves, writes and verifies `<label>.ewarchive` outside the project,
tags it in git when the project is a repository, and stamps the revision released, atomically (NFR-REL-15); the
archive reproduces its PDF; Golden A's release is refused with its blockers, and a release fixture releases.

**Non-goals** (each with its package):
- No revision compare, deltas, clouds, overlays or transmittals (FR-REV-014 to FR-REV-017, FR-REV-023; WP-S1.31). No
  issue purposes beyond the as-built text (doc 15 §5.9, 1.0): `purpose` stays in `extra`.
- No completed-project state (FR-REV-013, 1.0) and no `ew archive verify` or `ew archive rebuild` (FR-REV-025, 1.0);
  part 5's reproduction is a library call and a test.
- No draft marking outside the frame (FR-OUT-021, 1.0), no `.ewpp` in the archive (1.0), no JSON Schemas or format
  specification in the archive (NFR-REL-10's 1.0 part; doc 15 §5.5).
- No gix: the git steps use the `git` executable (D12); local history is 1.x (ADR-0012 item 7).
- No review comments and no EW-REL-003 (FR-REV-024, 1.0).
- No change to Golden A's exported bytes: Golden A holds V1.0 released (historic) and V2.0 open and is never
  released by a test that writes into `golden/`.

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Checks, revisions, collaboration](../../design/15-checks-revisions-collaboration.md) §2.5, §4.8, §5.1 to §5.6, §5.10 | Lifecycle, labels, the open revision, sign-off, release, page change status, withdrawal, the EW-REL rules |
| [Project format](../../design/05-project-format-and-persistence.md) §12, §14 | The deterministic ZIP rules, the release sequence, the archive layout and its use |
| [Domain model](../../design/03-domain-model.md) §18 | `Revision`, `ReleaseRecord`, `PageChangeStatus` |
| [Golden project](../../design/20-golden-project.md) §2.4, §18.3 | Golden A-R and the release gate's expected result |
| [Reports](../../design/09-reports-forms-output.md) §2.6, §2.7 | The revision cell, `{page.last_changed_in}`, `revision_overview` |
| ADR-0003, ADR-0008, ADR-0009 amendment 6, ADR-0011 amendment 4, ADR-0013, ADR-0019 | Committed computations, `HumanOnly`, the revision invariants, typing within 0.1 (ID-334), the archive, release mode |
| Specifications | [WP-S1.16](WP-S1.16-mvp-rule-packs.md) (schemes, release mode, waivers in part 5) |
| Follow-ups | FT-139 (with WP-S3.10), FT-207, FT-286, FT-302, FT-303, FT-402 |

### 2.1 What exists at `adb0df1a` (the base contract)

| Item | Where | Used here for |
|---|---|---|
| `Revision` (label, date, author, checker, description, state, page_notes, withdraws, extra), `RevisionState` (open, released, withdrawn), `PageNote` | `crates/ew-schema/src/revision.rs` | Part 1 types `sign_off`, `release` and `reason` from `extra` |
| `revision.create` (an open or a historic released revision; author, checker, page notes), `fields.set` and `edit.delete` on the open revision; EW-INV-016 to EW-INV-020 | `crates/ew-commands/src/commands/revision.rs` | Part 1 builds on it |
| The exposure classes and `HumanOnly` refusal for scripted origins, undo and redo included (ID-649); `project.assets.purge` as a `HumanOnly` command | `ew-commands`, `ew-api/src/cmdlog.rs` | The approve, release, withdraw and author commands |
| The title block's revision cell from the typed records | `crates/ew-derive/src/title_block.rs` | Unchanged; part 2 adds `{page.last_changed_in}` |
| `revision_overview` registered as `NotDerived::AwaitsQuery("revision_rows")` | `crates/ew-reports/src/kinds/mod.rs` | Part 2 (FT-207) |
| `page_display_list(view, layer, fonts)` and `page_scene` | `crates/ew-scene/src/page.rs`; `crates/ew-api/src/output/layers.rs` | Page digests |
| The manifest with per-file SHA-256 and generations, the save protocol, the `Vfs` seam with fault injection | `crates/ew-format/src/manifest.rs`; `crates/ew-store/` | Content digest, the save of g and g+1, kill-point tests |
| `ew check --mode release`, the schemes, `CheckMode::Release` | `bin/ew/src/cli.rs`; `crates/ew-checks/src/scheme.rs` | The gate |
| The PDF export job (PDF/A-2b) and `ew export report --format csv` | `ew-api`, `ew-export`, `bin/ew` | Archive contents |
| `miniz_oxide` 0.8.9 in the workspace (the XLSX reader, ID-827) | `Cargo.toml` | The archive's deflate (D11) |

### 2.2 Gaps found

| Gap at `adb0df1a` | Consequence |
|---|---|
| No label format, no next-label proposal, no `revision.ensure_open`; author and checker still writable by `fields.set` (FT-402); `sign_off`, `release`, `reason` untyped; no withdraw; a deleted page keeps the open revision's note (FT-303) | Part 1 |
| No page digest, no change status, no `revision_rows` (FT-207) | Part 2 |
| No content digest, no approve, no EW-REL rule, no gate | Part 3 |
| No release, no archive writer, no deterministic ZIP, no `ew revision` subcommands | Part 4 |
| No archive reader, no dialogs, AC-23 not evaluated | Part 5 |

## 3. Requirements traced

| Requirement | What this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-REV-001 | Built (cycle 17); generation, packs and profile in `ReleaseRecord` | 1, 4 | RL-1, RA-6 |
| FR-REV-002 | `settings.revision_label` and the next-label proposal; EW-REL-004 | 1 | RL-2 |
| FR-REV-004 | `last_changed_in` from page digests | 2 | PD-2 |
| FR-REV-005 | EW-REL-001 (author = checker, checker missing); `revision.approve` refuses it | 3 | RG-2 |
| FR-REV-006 | `revision.release`, the six steps of doc 05 §14.1 | 4 | RA-1 |
| FR-REV-007 | The gate: release scheme in release mode, `Tbd`, EW-REL | 3 | RG-4 |
| FR-REV-008 | The as-built archive (canonical text, libraries, PDF/A-2b, reports, check report, sums) | 4 | RA-2 |
| FR-REV-009 | Released records immutable (built); `revision.ensure_open`; `revision.withdraw` | 1 | RL-3, RL-5 |
| FR-REV-010 | Reproduction of the archived PDF from `project/` | 5 | RU-1 |
| FR-REV-011 | Frozen report rows `reports/*.csv` in the archive | 4, 5 | RA-2, RU-2 |
| FR-REV-012 | Built (page notes); notes drive non-digest releases | 2 | PD-3 |
| FR-REV-020 | `ReleaseRecord.approval` copied from the sign-off | 3, 4 | RG-1, RA-6 |
| FR-REV-021 | `sign_off.content_digest` over the saved manifest | 3 | RG-1, RG-3 |
| FR-REV-022 | `page_digests` and `changed_pages` in the release record | 2, 4 | PD-1, RA-6 |
| NFR-REL-10 | The MVP part of the archive (no schemas or specification, 1.0) | 4 | RA-2 |
| NFR-REL-15 | Atomic release with kill-point tests | 4 | RA-3, RA-4 |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under ID-836.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Remaining scope only.** The cycle-17 work is taken as **0.8 pw** of the 4.6 pw PERT; the five parts fill the remaining **3.8 pw** with no reserve and the de-scope rule of §11. The CSV is unchanged | As WP-S1.21's D1 |
| D2 | **Typed members within format 0.1** (ADR-0011 amendment 4, ID-334): `Revision.sign_off: Option<SignOff {checker, date, scheme, packs: [{id, version, sha256}], waivers_ack: [waiver id], content_digest}>`, `Revision.release: Option<ReleaseRecord>` with doc 03 §18's members (`archive {name, sha256}`, `git_tag?`, `page_digests` as a map of page ID to 32 lower-case hex digits, `changed_pages` in page order), `Revision.reason: MlText` (withdrawal records only). `purpose` stays in `extra`. Ruling R4's check of `revision.create` moves from `extra["release"]` to the typed member (FT-302). `ProjectSettings.revision_label: Option<LabelFormat>` with `v_major_minor` (`V{major}.{minor}`), `two_digit` (`{nn}`), `letter` (`{A}`, skipping I and O); absent means `v_major_minor`, so Golden A and every existing file read as they do | One format step for the whole package, before 0.1's release; the absent setting is the golden form, so nothing migrates |
| D3 | **`revision.ensure_open`**, a normaliser row (doc 06 §3.2): when a transaction changes a record that the content digest covers (D8's file set) and the project has a released revision and no open one, it adds `Revision {label: next, state: open}` with ID `v5(latest released revision, "next")`, no author; undo of the edit removes it. Revision page notes, comments and view state are not content | doc 15 §5.3; a v5 ID makes replays exact. There is no user identity at MVP, so the author is set explicitly (D4) |
| D4 | **Roles have one confirmed path each** (FT-402): `fields.set` refuses `author` and `checker` from every origin ("set by Revision ▸ Set author or by approval"); `revision.set_author {revision, role}` (`HumanOnly`, open revision only) sets the author; the checker is written only by `revision.approve` (D9). `revision.create` keeps both parameters for a historic released revision and refuses `checker` on an open one | Four eyes as a hard rule needs one place where each role is written; imports of historic revisions keep their roles |
| D5 | **`revision.withdraw {revision, reason}`** (`HumanOnly`) adds a record in state `withdrawn` with `withdraws` and `reason`, ID `v5(revision, "withdrawal")`; the released record stays (EW-INV-020). **FT-303:** `page.delete` drops the open revision's note on the deleted page in the same transaction; notes of released and withdrawn revisions stay | doc 15 §5.10; the note fix is the one place where a page deletion touches a revision |
| D6 | **Page digests** (doc 15 §5.6): `page_digest(p)` = the first 128 bits of SHA-256 over a canonical byte encoding of `page_display_list(p, layer, lang)` for every documentation language and every layer except frame, highlights and the derived revision layer, followed by the title-block cell values except the revision label, the dates and the status mark. The canonical encoding is a new function in `ew-dl` (`canonical_bytes(&LayerList)`: fixed field order, integers little-endian, text as UTF-8 with lengths), so the digest does not depend on Rust's `Debug` or on serde. Computed in `ew-api` per page over the existing scene cache; Class I | Digests, not file ownership, as designed; an own encoding keeps digests stable across dependency upgrades |
| D7 | **Change status and folio 911.** `changed_now(p)`, `last_changed_in(p)` and `changed_pages(r)` as doc 15 §5.6; `revision_rows` lists the revisions in label order with a withdrawal record directly after the revision it withdraws (FT-207); `revision_overview` reads it; the placeholder `{page.last_changed_in}` joins the catalogue of doc 09 §2.6; the page navigator gains a "Changed in" column | FR-REV-004 and FR-REV-012 with one query each |
| D8 | **Content digest:** SHA-256 over the sorted `(path, sha256)` entries of the saved manifest, excluding `revisions.jsonl` and `comments.jsonl`, as a pure function in `ew-format`. **Saved state only:** approve and release need the write lock and no unsaved edits | doc 15 §5.4; the digest is a function of one generation |
| D9 | **`revision.approve {revision, checker}`** (`HumanOnly`): preconditions checker ≠ author (structural, D-34), a release-mode run of the release scheme with no unwaived errors except EW-REL-001, every safety-kind waiver acknowledged; writes `checker` and `sign_off`. **EW-REL-001** (author missing, checker missing, author = checker, sign-off missing or its digest ≠ the current content digest), **EW-REL-002** (stale committed computations; registered, reporting nothing until committed routes exist, WP-S4.x), **EW-REL-004** (a label out of the project's format or not greater in natural order) with catalogue rows (present), rule pages and messages; EW-REL rules run in the gate only (as built in `scheme.rs`) | The design's four-eyes rule with the smallest new surface; EW-REL-002 has no input before committed routes |
| D10 | **The gate** runs on a fork at the saved generation: the release scheme in release mode, the `Tbd` rules and EW-REL; its result is the release's refusal list. **CLI:** `ew revision approve --as <role>`, `ew revision release <label>`, `ew revision withdraw <label> --reason <text>`, each submitting its `HumanOnly` command only after the person types the label on an interactive terminal; without a terminal they exit 2. Tests confirm through the test-build confirmation hook (doc 18), which release builds lack | doc 15 §5.4 and doc 17 §2.3 as written |
| D11 | **The archive writer is an own deterministic ZIP writer in `ew-store`** on `miniz_oxide` (already in the workspace, ID-827), not the `zip` crate doc 05 §19 names: entries in byte order of their paths, `mimetype` first and stored, every other entry deflated at a fixed level, DOS time 1980-01-01 00:00, no extra fields, no comments; the layout of doc 05 §14.2 at MVP: `mimetype`, `ARCHIVE.json`, `project/` (the canonical directory at g with `lib/` and `assets/`), `pdf/<drawing id>_<label>.pdf`, `reports/<report-block id>.csv`, `check/report.json`, `SHA256SUMS`; written as `<label>.ewarchive.tmp-<g>`, synced, re-read and verified (sums, and the canonical text hashes equal manifest g), then renamed. The check report as PDF (FR-CHK-014) is not built, so the archive carries the JSON only (FT-667) | One deterministic writer, no new crate, and the same reasoning as ID-827: the archive's bytes must not depend on a dependency's defaults |
| D12 | **Git through the `git` executable.** When the project directory is inside a git worktree, step 3 asks, then commits only the project directory ("Release <label>") with `git -C <dir> commit` (argument vectors, no shell), and step 5 creates the annotated tag `ew/rev/<label>` whose message carries the archive's SHA-256; a missing executable or a refused commit refuses the release before the archive; outside a worktree both steps are skipped and `git_tag` stays absent. gix stays for 1.x local history | No new crate for two operations; a project in git has git installed. The trade-off is a process boundary whose output is parsed only for the exit status |
| D13 | **Atomicity and recovery** (doc 05 §14.1): a failure in steps 4 to 6 leaves the revision open and removes the temporary file and any archive or tag this run created; the freeze of step 2 stays as an ordinary undoable transaction; the release asserts every page digest equal before and after the freeze and aborts otherwise. A crash leaves at most a complete, unreferenced archive; the next open reports it (`Session::incomplete_release()`), and the user discards it or completes the stamp. **At MVP the freeze applies the numbering scheme's freeze-at-release setting only** (FR-NUM-011); without one it is an empty step | NFR-REL-15 as designed; only frozen numbers exist as committed computations at MVP |
| D14 | **Reproduction** (FR-REV-010): `Session::open_archive(path)` opens `project/` read-only from the archive in memory with the release context (date, label); exporting its PDF gives the archived PDF's bytes; the frozen report CSVs equal the reports re-derived from `project/` (FR-REV-011) | The reproduction guarantee as a test, without the 1.0 CLI |
| D15 | **AC-23 in two halves.** The refusal half on Golden A: `ew revision release V2.0` refuses with exactly the release scheme's errors that `ew check --scheme pre-delivery --mode release` reports on that build plus EW-REL-001 "checker not set"; it equals doc 20 §18.3's 42 errors when AC-21 passes. The success half runs on a release fixture (`golden/fixtures/release-min`: one page, two devices, author and checker parties, no finding) through `ew revision approve` and `ew revision release`; **Golden A-R's release is FT-666**, because the A-R overlay needs the fluid, layout and accessory work of other packages | A gate criterion that can pass when this package is done, with the rest named |

No decision changes an ADR. ADR-0013 (archive outside the project), ADR-0008 (`HumanOnly`), ADR-0009 amendment 6 and
ADR-0011 amendment 4 are applied as written. D11 and D12 deviate from doc 05 §19's dependency list (zip, gix for
release); the design pass after part 4 amends doc 05 §19.

## 5. Part 1: the revision lifecycle data and commands

**Scope.** D2 to D5, FT-302's data and withdraw half, FT-303, FT-402. **0.6 pw.**

**Acceptance.** **RL-1** the typed members round-trip; a file written before part 1 with `sign_off` in `extra` loads
into the typed member; Golden A byte-identical; `schemas/0.1` re-blessed additively. **RL-2** the next-label proposal
after V2.0 is V3.0 (`v_major_minor`), after 02 is 03, after H is J; `revision.create` of "2" in a `v_major_minor`
project gives EW-REL-004. **RL-3** in a project with V1.0 released and no open revision, `fields.set` on a device opens
V2.0 in the same transaction; undo removes both; a comment edit opens nothing. **RL-4** `fields.set author` and
`checker` are refused with D4's text; `revision.set_author` from a script origin is refused with the exposure error.
**RL-5** `revision.withdraw` adds the withdrawal record with its reason; the released record is byte-identical.
**RL-6** deleting a page drops the open revision's note on it; a released revision's note stays. **RL-7** targeted
checks for `ew-schema`, `ew-format`, `ew-commands`, `ew-api`.

## 6. Part 2: page digests, change status and the revision overview

**Scope.** D6, D7, FT-207. **0.8 pw.**

**Acceptance.** **PD-1** `page_digest` of every Golden A page is stable across two runs, across 1 and 4 threads and
across a save and reload; moving a device's representation on folio 101 changes 101's digest and no other; changing
only the revision label changes none. **PD-2** in a fixture with a digest-based release, an edit on page 2 sets
`last_changed_in(2)` to the open label and leaves page 1 at the released one. **PD-3** Golden A (V1.0 historic, no
digests): `changed_pages(V2.0)` is {101, 106} from the page notes; folio 911 lists V1.0 released and V2.0 open with
101 and 106; a withdrawal record is listed after the revision it withdraws. **PD-4** `{page.last_changed_in}` in a
fixture title block prints V2.0 on 101 and 106 and V1.0 elsewhere. **PD-5** Golden A's exported bytes: folio 911
changes from the placeholder to its rows (a golden change the lead decides under the owner's standing delegation,
listed); every other byte equal. **PD-6** targeted checks for `ew-dl`, `ew-scene`, `ew-api`, `ew-reports`.

## 7. Part 3: content digest, sign-off and the release gate

**Scope.** D8 to D10. **0.8 pw.** After WP-S1.16 part 5 (waivers and their acknowledgement).

**Acceptance.** **RG-1** after a save, `revision.approve --as checker` writes `sign_off` with the content digest of the
saved manifest and the pack versions. **RG-2** approve is refused when the checker is the author, when there are unsaved
edits, and when a release-mode error other than EW-REL-001 is unwaived (the list printed). **RG-3** a content edit
after approval makes EW-REL-001 report a stale sign-off; a comment edit or a page-note edit does not. **RG-4** the gate on
Golden A lists the release scheme's errors plus EW-REL-001 "checker not set" (D15's refusal half). **RG-5** the three
subcommands exit 2 without a terminal; with the test hook they submit; a `.ewcmd` line naming `revision.approve` is
refused with the exposure error. **RG-6** targeted checks for `ew-format`, `ew-commands`, `ew-checks`, `ew-api`, `ew`.

## 8. Part 4: the atomic release and the as-built archive

**Scope.** D11 to D13, FT-302's release half, NFR-REL-15. **1.1 pw.**

**Acceptance.** **RA-1** on the release fixture after approval, `ew revision release V1.0` performs the six steps and
the revision is released at g+1 with `ReleaseRecord` complete. **RA-2** the archive holds exactly D11's entries;
`SHA256SUMS` verifies with `sha256sum -c` semantics; the project text hashes equal manifest g. **RA-3** kill points
before and after each of steps 2 to 6 (through the `Vfs` seam and an injected git failure): the revision stays open
and no archive or tag is referenced; after a crash at most one complete unreferenced archive exists and
`incomplete_release()` names it; discard removes it, complete stamps it. **RA-4** a freeze that would change a page
digest aborts the release. **RA-5** releasing twice from the same saved state writes byte-identical archives (fixed
release context). **RA-6** the release record carries the sign-off as `approval`, the packs, the derivation profile,
the content digest, every page digest and the changed pages. **RA-7** in a temporary git repository the commit and the
tag exist with the archive's hash in the tag message; without `git` on the path a release inside a worktree is refused
before the archive. **RA-8** targeted checks for `ew-store`, `ew-api`, `ew-commands`, `ew`.

## 9. Part 5: the archive's use, the dialogs and AC-23

**Scope.** D14, D15, the GUI dialogs. **0.5 pw.**

- **Dialogs** (Check tab and Revision menu): Approve revision (choose the checker's role, see the gate's result, type
  the label to confirm), Release revision (the gate's blockers or the six steps with progress through the job
  model), Withdraw (reason required), Set author; each submits its `HumanOnly` command through the confirmed GUI
  path. The incomplete-release offer at open.

**Acceptance.** **RU-1** `open_archive` of RA-1's archive exports a PDF byte-equal to `pdf/`. **RU-2** the archived
report CSVs equal the reports re-derived from `project/`. **RU-3** the dialogs in the headless shell: approve, release
and withdraw complete; a wrong typed label refuses; `ui.describe` words counted. **RU-4 (AC-23 as D15)** the refusal
half on Golden A and the success half on the release fixture, in the acceptance runner. **RU-5** targeted checks for
the changed crates.

## 10. Files owned, what a lane may not decide, risks

**Files owned.** Part 1: `crates/ew-schema/src/{revision.rs,project.rs}`, `schemas/0.1/**`,
`crates/ew-commands/src/commands/{revision.rs,fields.rs,page.rs}`, `crates/ew-commands/src/normalise/**`, their tests.
Parts 2 to 5: named by crate in their scope and fixed by each dispatch brief. Shared append-only files as in the
[WP-S1.19 specification](WP-S1.19-parts-libraries-and-macros.md) §13.1. Read-only: other crates, `golden/**` except the
new `golden/fixtures/release-min` and PD-5, `ci/**`, `.github/**`, `docs/**`.

**A lane stops and reports when:** a change needs a new crate, a new external dependency or a new `ci/dep-rules.toml`
edge; a member beyond D2 would be typed; Golden A's exported bytes change outside PD-5; a release path can be reached
from a replay, a script, MCP or a plugin; the archive's bytes differ between two runs; a later part's work is reached.

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 Page digests change with unrelated rendering fixes and mark every page changed | Medium · medium | D6's own encoding; derivation-profile pinning (doc 15 §5.6); at MVP an upgrade records page notes |
| R2 The release loses data or leaves a half state on a crash | Low · high | D13; RA-3's kill points; part 4 at Opus `xhigh` |
| R3 `ensure_open` fires on non-content edits and opens revisions by surprise | Medium · low | D3's file set is D8's; RL-3 covers a comment edit |
| R4 The gate depends on WP-S1.16 part 5 and slips with it | Medium · medium | Part 3 is ordered after it; parts 1 and 2 do not wait |
| R5 The `git` executable behaves differently across platforms | Low · medium | Exit status only, argument vectors, RA-7 on every CI host |

## 11. Size, order and routing

**Size.** 3.8 pw of the 4.6 pw PERT (D1): part 1 **0.6** (types and setting 0.2, ensure_open 0.15, roles and
withdraw 0.15, tests 0.1), part 2 **0.8** (encoding and digest 0.3, status and rows 0.25, 911 and placeholder 0.15,
tests 0.1), part 3 **0.8** (content digest 0.05, approve 0.2, EW-REL rules 0.25, gate and CLI 0.2, tests 0.1), part 4
**1.1** (ZIP writer 0.25, release sequence 0.35, git 0.15, recovery 0.15, kill-point tests 0.2), part 5 **0.5**.
**De-scope rule** (more than 20 % over at a midpoint report or at review), in this order, without a further ruling:
the page navigator's "Changed in" column, the Set author dialog (the CLI stays), the incomplete-release "complete"
action (discard stays), the `two_digit` and `letter` label formats.

**Order and routing** (Opus or Haiku only, ID-700): part 1 **Opus at `xhigh`** (format typing); part 2 **Opus at
`high`**; part 3 **Opus at `high`**, after WP-S1.16 part 5; part 4 **Opus at `xhigh`** (data loss and atomicity);
part 5 **Opus at `medium`**. **Unblocks:** WP-S1.31 (compare, deltas) after part 4; AC-23's A-R half (FT-666) with
Golden A-R.

## 12. Follow-ups and the documents to update

**Follow-ups.** FT-207, FT-302, FT-303 and FT-402 close with their parts. New: FT-666 (Golden A-R's release, D15),
FT-667 (the check report PDF in the archive, D11), FT-668 (a per-user party so that `ensure_open` writes the author,
D3). FT-139 and FT-286 stay with WP-S3.10 and WP-S7.01.

**Documents.** **After part 1:** doc 03 §18 (as built), doc 15 §5.2 and §5.3 (as built), doc 06 §3.2 (the
normaliser row), doc 05 §2.2. **After part 2:** doc 15 §5.6, doc 09 §2.6 and `revision_overview` (as built).
**After part 3:** doc 15 §4.8 and §5.4 (as built), doc 17 §3.3 (the subcommands). **After part 4:** doc 05 §14 and
§19 (the own ZIP writer, the `git` executable), ADR-0013's consequences by a dated note. **After part 5:** doc 16 (the
dialogs), doc 20 §18.3 (AC-23's two halves).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.18, WP-S1.16, WP-S1.31; Appendix B (FT-139, FT-165, FT-207,
  FT-286, FT-302, FT-303, FT-402, FT-666 to FT-668).
- [Decision log](../06-decision-log.md): ID-427 to ID-433, ID-473, ID-593, ID-626, ID-649, ID-700, ID-827, ID-829,
  ID-836.
- [Checks, revisions, collaboration](../../design/15-checks-revisions-collaboration.md) §4.8, §5;
  [project format](../../design/05-project-format-and-persistence.md) §12, §14, §19;
  [domain model](../../design/03-domain-model.md) §18; [golden project](../../design/20-golden-project.md) §2.4, §18.3.
- [ADR-0003](../../adr/0003-freshness-contract.md), [ADR-0008](../../adr/0008-transactions-undo-preview.md),
  [ADR-0009](../../adr/0009-invariant-catalogue.md), [ADR-0011](../../adr/0011-format-versioning-and-migration.md),
  [ADR-0013](../../adr/0013-revisions-and-as-built-archive.md), [ADR-0019](../../adr/0019-rule-engine-and-rule-packs.md).
- Requirements: FR-REV-001, FR-REV-002, FR-REV-004 to FR-REV-012, FR-REV-020 to FR-REV-022, NFR-REL-10, NFR-REL-15.
