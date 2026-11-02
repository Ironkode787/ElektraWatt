# WP-S1.17: collaboration: external change, migrations, integrity, merge driver (closed specification)

**Status:** specified in cycle 42 (documents only, lane S42b), an M2 package and a dependency of WP-S6.08, in eight
parts: part 1 the store's stamps and safeguards (0.9 pw, D3 to D7), part 2 `.ewz` transport (0.8 pw, D8, D9), part 3
format migrations and the upgrade backup (0.8 pw, D10 to D12), part 4 the three-way merge core and `ew merge-driver`
(1.2 pw, D13 to D16), part 5 conflict records, conflict markers and their resolution (0.9 pw, D17 to D19), part 6
external change in the store and the session (1.1 pw, D20 to D23), part 7 the window, cloud folders and the driver
status (0.8 pw, D24, D25), part 8 `ew diff`, `ew git setup`, the post-merge integrity and AC-41 (0.9 pw, D26 to D29).
The parts sum to **7.4 pw plus 0.3 pw reserve** against a 6.8 pw PERT (O1) · **Amended in cycle 42** (decision log ID-1029, the owner's format decision): part 3, D4 and D3's `unstable` marker move to the wave before the first release, beside WP-S6.08; the PERT is 6.9 pw; format parts route at `high`, a save, a journal, recovery or a merge at `xhigh` (§16) · **Date:** 2026-10-31 · **Work package:**
[WP-S1.17](../02-work-packages.md#wp-s117-collaboration-external-change-migrations-integrity-merge-driver) ·
**Decision log:** ID-1015; amended by ID-1029; part 1 ruled by ID-1115, its fixes by ID-1144; part 2 ruled by ID-1175; `fmt --repair` ruled by ID-1185, its backup by ID-1186; as built ID-1152, ID-1156 · **Part of:** ElektraWatt plan, specifications

**Purpose.** WP-S1.09 built the canonical format, the save protocol, the lock, the journal and recovery, and left to
this package everything that happens when a project changes outside the running session or between builds: a pull
or a colleague's save while the project is open, a hand edit, a sync client's conflict copy, a project from an older
or newer format, a git merge of two branches, and the transport of a project as one file. This document closes the
package against the code at `453d252d` (§2.1). Where it and a design document differ, this document is the narrower
and wins for the lanes; the design documents stay the target, and the documents of §14 are updated at the design pass
after each part. A choice a lane meets that this document does not settle is escalated, never guessed (`CLAUDE.md`,
rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Format and data](#5-format-and-data)
6. [Part 1: the store's stamps and safeguards](#6-part-1-the-stores-stamps-and-safeguards)
7. [Part 2: `.ewz` transport](#7-part-2-ewz-transport)
8. [Part 3: format migrations and the upgrade backup](#8-part-3-format-migrations-and-the-upgrade-backup)
9. [Part 4: the three-way merge core and `ew merge-driver`](#9-part-4-the-three-way-merge-core-and-ew-merge-driver)
10. [Part 5: conflict records, conflict markers and their resolution](#10-part-5-conflict-records-conflict-markers-and-their-resolution)
11. [Part 6: external change in the store and the session](#11-part-6-external-change-in-the-store-and-the-session)
12. [Part 7: the window, cloud folders and the driver status](#12-part-7-the-window-cloud-folders-and-the-driver-status)
13. [Part 8: `ew diff`, `ew git setup`, post-merge integrity and AC-41](#13-part-8-ew-diff-ew-git-setup-post-merge-integrity-and-ac-41)
14. [Files owned, boundaries, risks, size and routing](#14-files-owned-boundaries-risks-size-and-routing)
15. [Follow-ups, documents to update, open questions](#15-follow-ups-documents-to-update-open-questions)
16. [Amendment after the owner's format decision](#16-amendment-after-the-owners-format-decision-cycle-42-decision-log-id-1029)
17. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package a project that changes on disk while it is open is detected, and the change is
applied as one undoable "External change" step when no unsaved edit collides, or the colliding fields are shown for a
choice; a save never overwrites a file that changed outside the session; a read-only follower reloads on each of the
holder's saves; a project travels as a deterministic `.ewz` and opens from one, extracted or read-only; a project of
an older format migrates in memory and its first save asks and writes a backup `.ewz`; the manifest carries a computed
`min_reader` and, after the first release, the `unstable` mark; an untouched record with unknown members keeps its
original line; `ew merge-driver` merges three-way per record and per field and writes conflict records; conflict
markers left by a text merge load as conflict records; `ew merge resolve` resolves them in one step; `ew diff` and
its textconv print changes with derived tags; `ew git setup` configures the driver; the integrity check runs on open
and after merges with the "renumber the newer" fix for device tags; and AC-41 passes in a real git repository.

**Non-goals** (each with its package):
- No conflict-resolution panel for driver conflict records (FR-COL-015, 1.0; WP-S2.15) and no cross-file
  reconciliation (FR-COL-016, 1.0; WP-S1.31). At MVP conflict records load, block release (EW-CORE-011) and are
  resolved by `ew merge resolve` (doc 15 §6.5's release split).
- No visual comparison (FR-COL-017), work copies (FR-COL-022), request-edit files (FR-COL-023), local history
  through gix (FR-COL-020) or real-time co-editing (FR-COL-021).
- No `--omit-personal-data` on `ew export ewz` (FR-OUT-025, WP-S3.04; FT-255).
- No importer migration reports or `ew verify-migration` (WP-S3.09): "migration" here is ElektraWatt's own format
  chain only (§14.2).
- No signing of `.ewz`, trust store or threat model (WP-S6.04; §14.2).
- No `notify` watcher: detection polls the manifest (D20).
- No change to any exported byte of Golden A and no write into `golden/` (§5).

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Project format](../../design/05-project-format-and-persistence.md) §4, §5.2, §6.2, §6.3, §7.2, §7.3, §9, §10, §11, §12, §15, §16 | Manifest stamps, the save pre-check, mismatch classes, integrity findings, lock loss, read-only reload, external change, versioning, git integration, `.ewz`, cloud pitfalls, the UI touchpoints |
| [Checks, revisions, collaboration](../../design/15-checks-revisions-collaboration.md) §6.1 to §6.5 | Modes of working, the git workflow, branch-safe counters, the merge panel's release split, AC-41 |
| [Golden project](../../design/20-golden-project.md) AC-41 | The acceptance on W20 and -102F2 |
| [Extensibility](../../design/17-extensibility.md) §3.2, §3.3 | The CLI catalogue, the project argument (`--project <dir or .ewz>`), the merge driver's exit codes |
| ADR-0008, ADR-0009, ADR-0010 amendments 1 and 3, ADR-0011 amendments 1 to 5, ADR-0012 amendments 1 and 2 | Transactions and exposure, the invariants, the save protocol, versioning, git integration |
| Specifications | [WP-S1.18](WP-S1.18-revisions-release-and-archive.md) (D11 ZIP writer, D12 git executable, D14 archive open), [WP-S1.22](WP-S1.22-cli-surface-and-api-guard.md) (D2 ownership by library, D9 `ew fmt`), [WP-S2.05](WP-S2.05-property-grid-find-replace-clipboard.md) §8 (fragments, FT-403), [WP-S2.09](WP-S2.09-messages-bulk-previews-history-jobs-and-recovery-ui.md) (history origins, the recovery dialog) |
| Follow-ups | FT-95, FT-96, FT-97, FT-98, FT-403; `--integrity` of FT-191 |

### 2.1 What exists at `453d252d` (the base contract)

| Item | Where | Used here for |
|---|---|---|
| `reader_access` with `Access::{ReadWrite, ReadWriteNewer, ReadOnly, Migrate, Refuse}`; `0.x` steps count as majors | `crates/ew-format/src/version.rs` | Parts 1 and 3. `Migrate` is refused today as `OpenError::Migration` (`crates/ew-store/src/store.rs`, the open path) |
| `Manifest {format, min_reader, generator, project, generation, files, merged, unstable, extra}`, `FileHash::Pending` | `crates/ew-format/src/manifest.rs` | Stamps (part 1), the manifest merge (part 4) |
| `FORMAT = "0.1"`; the kind registry with `conflict` reserved at rank 255, file rule `TargetFile` | `crates/ew-schema/src/kind.rs` | Part 5 types `conflict` |
| The reader: kept lines (unknown kinds, quarantined lines, malformed variants), `NonCanonical` rewrites, findings EW-CORE-001 to -019 with `ConflictMarkers` (EW-CORE-006) as a read-only finding; a manifest with markers is `ManifestUnreadable` | `crates/ew-format/src/read.rs`, `finding.rs` | Parts 1 and 5 |
| The save: lock and manifest pre-check (`SaveError::ExternalChange` when `manifest.json`'s hash moved), unlisted targets moved aside to `.unlisted-<g>[-n]` names, `keep_format` for newer minors, generation = loaded + 1 (0 + 1 after an unreadable manifest, FT-97) | `crates/ew-store/src/store.rs` | Part 1 |
| `Store::read_generation` (one verified generation), `Store::canonical_rewrite` (`ew fmt`) | `crates/ew-store/src/store.rs` | `.ewz` packing (part 2), `ew fmt` (parts 1, 2) |
| The journal with `replay(model, txs, check)` and field collisions (`journal::Collision`), recovery onto a moved base | `crates/ew-store/src/journal.rs` | Part 6's rebase |
| The deterministic ZIP writer and strict reader (`zip::write`, `zip::read`, `ZipLimits`; no ZIP64, at most 65,535 entries) | `crates/ew-store/src/zip.rs` | Part 2 |
| The git seam (`Git`, `GitExe`, `worktree_root`, argument vectors, exit status only) | `crates/ew-store/src/git.rs` | Parts 6, 7, 8 |
| `Session::open_archive`: a ZIP read whole, checked, its `project/` opened read-only from a `MemVfs` | `crates/ew-api/src/release_run.rs` | The pattern of `Session::open_ewz` (part 2) |
| `diff_model`, `ModelDiff`, `derived_diff`, `Session::diff_from` (`ScriptDiff`) | `crates/ew-commands/src/diff.rs`, `crates/ew-api/src/cmdlog.rs` | `ew diff` (part 8) |
| `Origin::External`, `Origin::Merge`; the history's "external change" origin text | `crates/ew-commands/src/meta.rs`, `crates/ew-api/src/output/history.rs` | Parts 5 and 6 |
| `Vfs` with `rename_noreplace` (by hard link: files only), no exclusive directory create | `crates/ew-store/src/vfs.rs` | Part 2 adds `create_dir` |
| The uniqueness rules EW-RDS-004, EW-TERM-002, EW-POT-015, EW-CORE-052, EW-PLC-001, EW-PLC-007; EW-CORE-011 and -013 as catalogue rows only; `QuickFix` | `crates/ew-checks/src/rules/`, `finding.rs` | Part 8 |
| `ew fmt` and `ew recover` | `bin/ew/src/cli/project_files.rs:23, :159` | Part 1 (`fmt` keeps original lines); `fmt --repair` in its own lane after part 2 (ID-1175), built to ID-1185 |
| The CLI's read-only refusals of writers | `bin/ew/src/cli.rs:151, :161` | Parts 2, 3, 5 (the same refusal for `.ewz` and migrated projects) |
| The window's read-only reasons and save errors (`readonly-lock-lost`, `saveerr-external`) | `crates/ew-viewmodel/src/shell.rs` | Part 7 |

### 2.2 Gaps found (why the parts are shaped as they are)

| Gap at `453d252d` | Consequence |
|---|---|
| `min_reader` is written equal to `format`; `unstable` is never written and is read against `!cfg!(debug_assertions)`, so an optimised test build and a debug build disagree | Part 1 (FT-95, D3) |
| An untouched record with unknown members is re-emitted canonically (FT-96) | Part 1 (D4) |
| The save checks only `manifest.json`: a hand edit or a checkout of a file the manifest still names is overwritten by the next save that touches that file | Part 1 (D5), a data-loss fix |
| A conflicted manifest restarts the generation at 1 (FT-97); stray copies can multiply under a sync client (FT-98) | Part 1 (D6, D7) |
| No `.ewz`; no `ew fmt --repair` | Part 2 (`.ewz`); `fmt --repair` in its own lane after part 2 (ID-1175), built to ID-1185 |
| No migration chain; an older format does not open | Part 3 |
| No merge core, no driver, no strategy table in `ew-schema` | Part 4 |
| `conflict` untyped; markers force read-only and nothing resolves them | Part 5 |
| No detection of external change, no apply, no read-only reload | Part 6 |
| No cloud warning, no driver status, no `.ewz` menu items, no collision dialog | Part 7 |
| No `ew diff`, no textconv, no `ew git setup`, no managed git files, no `ew check --integrity`, EW-CORE-013 not detected | Part 8 |
| The code and doc 05 §11.7 disagree on Golden A's W20: the data holds `overrides {csa "1.5", show_marker true}` and takes its colour DBU from the profile | AC-41 is defined on the data (D29); doc 05 §11.7 is corrected at the design pass |

## 3. Requirements traced

| Requirement | What this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-COL-004 | A hand-edited page file with a broken line is repaired with no data loss: `ew fmt --repair` first packs the files as found, the edited file included, its manifest hashed to them (ID-1186); the open's hash check and integrity notice are built (WP-S1.09) | `fmt --repair`'s lane, after 2 | EZ-5 |
| FR-COL-007 | Detection by manifest polling, quiescence, the incoming generation, the three-way apply as one undoable "External change" step, collisions held for a choice, the save pre-check of target files, the read-only reload | 1, 6, 7 | SG-3, XC-1 to XC-7, XW-1 to XW-3 |
| FR-COL-008 | The cloud-folder warning; conflict copies listed (EW-CORE-004, built) with a test; online-only placeholders (de-scopable) | 7 | XW-4, XW-5 |
| FR-COL-009 | `.ewz` pack, extract and read-only open; `ew export ewz`; readers accept `.ewz` | 2, 7 | EZ-1 to EZ-6, XW-6 |
| FR-COL-010 | Two-part versions (built), computed `min_reader` and `unstable` (FT-95), original lines (FT-96), the migration chain, `ew upgrade`, the fixture corpus job | 1, 3 | SG-1, SG-2, MG-1 to MG-6 |
| FR-COL-013 | `ew diff <from> <to>` and `ew diff --textconv` with derived tags | 8 | DG-1, DG-2 |
| FR-COL-014 | `ew merge-driver %O %A %B %L %P` and the 4-argument form; exit 0, 1, 129 | 4 | MD-1 to MD-7 |
| FR-COL-024 | `ew git setup [--global] [--hooks]`, the managed `.gitattributes` and `.gitignore` blocks, the status-bar driver status | 7, 8 | DG-3, DG-4, XW-7 |
| FR-COL-025 | Conflict markers loaded as conflict records; resolved into a canonical file | 5 | CR-2, CR-3 |
| FR-COL-029, NFR-REL-06 | The first save in a newer format asks once and writes `<name>.ew.pre-<F>.ewz` first; declining writes nothing | 3, 7 | MG-3, MG-4 |
| FR-CHK-012 | Integrity on open (built) and after merges: merged manifests, conflict records (EW-CORE-011), EW-CORE-013 with "renumber the newer" for device tags, `ew check --integrity`, the post-merge hook | 5, 8 | CR-4, DG-5, DG-6 |
| NFR-REL-05 | (a) the fixture-corpus job and the steward check; (b) to (d) as built and refined by D3 | 1, 3 | SG-1, MG-5, MG-6 |
| AC-41 | Clean and conflicting merges of Golden A copies through the real `git` with `ew git setup` | 4, 8 | MD-6, DG-7 |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under the decision-log row of this
specification.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Scope and size.** The reader decisions, the save's manifest pre-check, the ZIP writer, the git seam and the in-memory archive open exist (built by WP-S1.09 and WP-S1.18) and are reused, not re-sized. The eight parts fill **7.4 pw plus 0.3 pw reserve**; the CSV's 6.8 pw PERT predates the driver's per-field strategies and the external-change apply on the merge core (O1) | Size what is left; the built seams save about 0.4 pw that the PERT counted |
| D2 | **One three-way merge core** in `ew-format` (permissive), on `serde_json::Value` per record: the driver (part 4), the marker loader (part 5) and the external-change apply (part 6) call the same function. Work copies (1.0) and a recovery onto a moved base reuse it later | doc 15 §6.1 asks for "the same core as the driver"; one place to test and fuzz |
| D3 | **Stamps (FT-95).** `ew-schema` gains `since` metadata as constants, never as doc comments (no generated schema byte moves): per kind (every kind `"0.1"`, reserved kinds included: `Kind::since`, `KIND_SINCE`), and the tables `MEMBER_SINCE` and `VARIANT_SINCE` (a kind, a member path in the form of `PERSONAL_DATA_FIELDS`, for a variant its name, and the format), empty while every member is 0.1. `min_reader` = the greatest `since` of the kinds, members and variants present, never below the format's major floor and never above `format`; an opaque file or asset counts as the floor. A pure `ew-format` function gives one file's value from its lines with the tables injected (`file_since`; a kind by its `$k`, a member by its path, a variant by its value), `ew-format` re-exports the shipped tables (`SINCE`: `ew-store` has no `ew-schema` edge), and the store keeps each file's value beside its hash, computed at open for the files read and at save for the files written (ID-1115). `ew-schema` gains `RELEASED_FORMATS: &[&str]`, **empty until WP-S6.08 adds `"0.1"` at the first release**. A save writes `unstable: true` exactly when `RELEASED_FORMATS` is not empty and does not hold `FORMAT`. A manifest with `unstable: true` opens read-write only in a reader whose `FORMAT` equals the file's format and is not released; every other reader opens it read-only ("written by a development build with an unreleased format"). `StoreOptions::release_build` is derived from `RELEASED_FORMATS`, never from `cfg!(debug_assertions)` | Before the first release nothing is marked, so pre-release and pilot projects open in 0.1.0 without a read-only surprise and no golden byte moves; after it, a build that changed the format has bumped `FORMAT` and marks its files. The behaviour no longer depends on the build profile |
| D4 | **Original lines (FT-96).** `ew-format`'s `Loaded` keeps, for each record with unknown content (a non-empty `extra` at any depth), its original line and its canonical form at load. The writer emits such a record by its original line while its canonical form is unchanged. Such a line is no reason for a `NonCanonical` rewrite, and `ew fmt` keeps it (WP-S1.22 D9's "unknown members stay verbatim"). Records without unknown content cost nothing extra | ADR-0011 amendment 1 as written; byte identity for files from a newer minor (FR-COL-010's acceptance) |
| D5 | **The save pre-check covers its targets.** Before its temps, a save reads every file it will replace or remove and compares its bytes with the baseline's hash; a file whose CRLF to LF form matches is equal (doc 05 §6.2). Any other difference refuses the save with `SaveError::ExternalChange { files }` naming the files in path order, and nothing is written. **Ruled (ID-1115):** the check runs after the plan and before any temp or move-aside; the hash compared is the one the store holds, as loaded or written, for record files and library documents and for opaque files and assets alike; the CRLF-to-LF form is the reader's `crlf_to_lf`, made public in `ew-format`; a new target already on disk is not compared, the save moves it aside as an unlisted file, as built; the manifest's own check stays, its refusal naming `manifest.json`. **Amended (ID-1144):** a record file or library document is compared by its text: the store also holds the hash of its bytes' CRLF-to-LF form when that differs from the raw hash (at open from the bytes read, and after each write), and the file is unchanged when the disk's bytes or their LF form hash to either, so a file held as CRLF that a checkout turned into LF does not refuse; opaque files and assets keep the one hash; the manifest's file hashes and the plan's comparison keep the raw hash | A hand edit or a checkout that leaves `manifest.json` alone was overwritten silently; reading the one to three targets of a typical save costs little (not measured, owner's rule) |
| D6 | **The generation after a conflicted manifest (FT-97).** When `manifest.json` holds git conflict markers, the reader takes the `"generation": <n>` lines of every hunk side (and of the text the sides share) and the store's generation after open is their greatest, so the next save writes it plus 1 (`Manifest::conflicted_generation`); without markers or without such a line, the rule stays 0 + 1. The roll-forward is unchanged (ID-1115). A driver-merged manifest already carries max + 1 (D16). **The ceiling (ID-1144):** a save whose next generation would pass `MAX_GENERATION` (2^53 − 1, the reader's limit, public in `ew-format`) is refused with `SaveError::GenerationLimit` before any temp or move-aside, its text ending with saving the project to a new folder (Save as…, through `Store::create`, which starts at 0); a save with nothing to write stays allowed | doc 05 §11.3's rule for the only case the driver does not cover |
| D7 | **Stray duplicates (FT-98).** After a move-aside, when the new stray's bytes equal an older stray of the same file and generation (every `n`, past gaps), re-read just before, the new stray is removed and the finding names the older one; nothing else is removed. It lives inside the move-aside, so that the save and the roll-forward both get it (ID-1115) | The bytes stay in the older stray; clutter only, as FT-98 says |
| D8 | **`.ewz` = one verified generation.** `ew_store::ewz::pack` writes, through `zip::write`, `mimetype` first and stored (`application/vnd.elektrawatt.project+zip`), then `manifest.json`, every file the manifest lists, and `.gitattributes` and `.gitignore` when present, in byte order of their paths, deflated, DOS time 1980-01-01 (D11 of WP-S1.18; the `SOURCE_DATE_EPOCH` date rule of doc 05 §12 is not used). It packs `Store::read_generation`, never a dirty model: a dirty session's export packs the saved state and says so. Strays, foreign files, `.ew/`, locks, temps and `.git/` never enter. At most 65,535 entries (the writer has no ZIP64; doc 05 §12's 100,000 is lowered). **One exception (ID-1186):** the repair's backup, `<project folder>.before-repair-g<generation>.ewz`, holds that generation's files as found (`ewz::pack_as_found`): every listed file as the store loaded it, a file edited outside EW since the last save included, its manifest the loaded one with each listed file's hash set to those bytes (a pending hash too); the repair's report and `--json` result count the files that differ from the generation as saved. `pack_store` and `ew export ewz` are unchanged and still refuse a changed or pending file | One generation, the same bytes for the same project and build; no new crate |
| D9 | **Opening an `.ewz`.** The reader accepts only that shape: `mimetype` first with that value, then `manifest.json`, the files it lists with matching hashes, and the two git files; anything else refuses the archive (doc 05 §12's extractor rules are `zip::read`'s). **Read-only:** `Session::open_ewz(path)` opens it from a `MemVfs` (the `open_archive` pattern), no lock, no journal. **Extract:** into a sibling `<target>.tmp-ewz` directory created exclusively, every file written and synced, read back and verified against the manifest, then the directory renamed to `<target>`; a target that exists refuses before anything is written. `Vfs` gains an exclusive `create_dir`. Every CLI reader accepts an `.ewz` as its project argument; every writer refuses it ("an .ewz opens read-only; extract it first") | Never into an existing directory, never a half project under the target's name; an in-memory open needs no cache directory |
| D10 | **The migration chain** in `ew-format`: `Migrations` holds per-kind steps `fn(Value) -> Result<Value, MigrationError>` from `$v` n to n + 1, and format steps from one format to the next (manifest-level and file-level). Steps are never deleted. **The shipped chain is empty at 0.1**; every test injects a test chain through `StoreOptions::migrations` (a `&'static Migrations`, default the shipped one). A step's error carries its JSON Pointer and refuses the open with `OpenError::Migration`, nothing written | 0.1 is the first format, so no user file can need a step yet; the machinery and its tests exist before the first format change has to ship under pressure (O3) |
| D11 | **Opening an older format** migrates in memory: `Access::Migrate` runs the chain, every migrated file is dirty, the session opens editable with the banner "This project is in an older format; saving converts it" and `OpenReport::migrated_from`. **The first save** returns `SaveError::UpgradeConsent { from }` until `Session::confirm_upgrade()`; the confirmed save first writes `<dir name>.pre-<F>.ewz` beside the project (D8's pack of the original generation, read back and verified; a name that exists gets `-<n>`), then saves. Declining leaves every project file untouched. `ew upgrade <project> [--check]` (a writer) asks on a terminal or takes `--yes`; `--check` writes nothing and exits 1 when a migration is needed | FR-COL-029 and NFR-REL-06; the backup is written and verified before the first project byte changes |
| D12 | **The corpus and the steward check.** `fixtures/format/<format>/` holds each released format's fixtures; the job opens, saves and compares each with its expected output. It starts empty: WP-S6.08 adds `fixtures/format/0.1/` (Golden A and `golden/fixtures/release-min` as 0.1.0 writes them) with the release. `cargo xtask check-format` fails when `RELEASED_FORMATS` is not empty and `FORMAT` is not in it while the chain has no step from the last released format, or its corpus directory is missing, or `schemas/<FORMAT>/` is stale. **FT-403:** `parse_fragment` migrates an older fragment's records through the shipped chain and refuses only what the chain cannot reach (the refusal text unchanged) | NFR-REL-05 (a) as a check that bites at the first format change, without freezing a pre-release copy that every golden change would have to re-bless |
| D13 | **Merge strategies are an `ew-schema` table, atomic by default.** A record merges per top-level member; a member merges as one value unless the table names its path. Entries at MVP, where the member exists at the base: `object` for `connection.overrides`, `device.tag` and `project.settings`; `keyed(idx)` for `cp_overrides`; `keyed(key)` for `tech_data`; `keyed(role)` for `project.parties`; `set` for `aliases`; `max` for `project.master.edited`. Below an `object` path the members are atomic unless listed. Unknown kinds merge as whole records | The design's full recursion would merge into enum objects (`{"bridge":…}` against `{"jumper":…}`) and the `Unknown` catch-alls would accept the invalid union silently; the price is an occasional spurious conflict, never a wrong merge. Tried against AC-41's W20 (`overrides` with `csa`, `show_marker` and an added `colour`) |
| D14 | **Record level and conflict records** as doc 05 §11.2: the table of O, A, B; a true conflict keeps ours in place (for delete/modify, the modified side) and adds `{"$k":"conflict","id":<target id>,"kind":<target $k>,"base":…,"ours":…,"theirs":…}` with only the conflicting members (`"deleted"` for a deleted side). A side that does not parse or is from a newer major keeps the other side and adds the **file-level** record `{"$k":"conflict","id":v5(CONFLICT_NS, path),"kind":"file","path":<path>,"side":"ours" or "theirs","lines":[…]}`; the `path` member is added to doc 05's form so that the record's file is known. A library document conflict is a top-level `$conflict` member holding the other side's document | The file stays loadable; nothing of either side is lost |
| D15 | **`ew merge-driver %O %A %B %L %P`** (and `%O %A %B %P`) reads the three files under the reader limits of doc 05 §3.6, merges by `%P`'s class (record file, library document, `manifest.json`; any other path is a file-level conflict), and writes `%A` all-or-nothing (temp beside it, then rename). Exit 0 clean, 1 conflict records written, 129 on a caught internal error with `%A` untouched. A fuzz target over the three inputs joins `fuzz/` | Exit codes as ADR-0012 amendment 2; a crash never leaves a torn `%A` |
| D16 | **Manifest merge** as doc 05 §11.3: `generation = max(a, b) + 1`, `format` and `min_reader` the maximum, `generator` from ours, `files` three-way per path with `sha256:pending` where both changed, `merged: true`, no `base` | As designed |
| D17 | **`conflict` typed within 0.1** (ADR-0011 amendment 4, the `page_item` precedent): the members of D14, the partial objects kept as `Value`. A record-level conflict lives in its target's file and shares its target's ID; `edit.delete` of a record removes the conflict with its ID in the same transaction (a rule of the delete path, no `LINKS` row, because the target's kind varies). A record-level conflict whose target is missing at load is kept verbatim and reported. EW-CORE-011 reports each conflict and blocks release; conflict records no longer force read-only | Resolution must be one undoable transaction (doc 15 §6.5), which needs the records in the model |
| D18 | **Markers load as conflict records (FR-COL-025).** In a JSONL file every hunk side must consist of complete record lines; then, with a diff3 base, D2's core merges the hunk's records (clean parts merge, true conflicts become records); without a base, records equal on both sides are kept, records that differ keep ours and get a conflict of every differing member, and a record on one side only is kept with a conflict whose other side is `"deleted"`. If any side line does not parse, the file stays EW-CORE-006 and the project read-only, as built. The file is rewritten canonically at the next save | Never guess at a torn hunk; a one-sided record may be a deletion, so the person confirms it |
| D19 | **`merge.resolve {choices: [{conflict, take: ours or theirs}]}`** (exposure `Edit`, history "Resolve merge conflicts"): ours removes the conflict; theirs writes the conflicting members into the target (or deletes it for `"deleted"`) and removes the conflict; a file-level theirs replaces the file's records with its lines when every line parses as a known record, else it is refused. **`ew merge resolve <project> --take ours or theirs [--kind <k>] [--field <path>]`** (a writer) resolves the matching conflicts in one transaction and saves | doc 15 §6.5's CLI resolver at MVP; the 1.0 panel submits the same command |
| D20 | **Detection by polling the manifest**, not `notify`: every 3 s and on window focus the session's store compares `manifest.json`'s raw bytes hash with the one it loaded or wrote. A change waits for quiescence: two reads 750 ms apart with equal bytes and no git operation in progress (`index.lock`, `MERGE_HEAD`, `rebase-merge/`, `rebase-apply/` in the git directory, a `.git` file resolved to its `gitdir`). A change of a file that leaves the manifest alone is caught by D5 at the next save | The manifest is the commit point of every save and every git change of a project; no new dependency and no platform watcher quirks on SMB; the trade-off is that a hand edit shows at the next save, not at once. Intervals from doc 05 §9, not measured (owner's rule) |
| D21 | **The incoming generation** is read read-only through the store's open path (roll-forward never, the base check, the findings). When it carries a read-only finding (EW-CORE-002, -003, -005, -019) or its model does not build, nothing is applied: on a cloud path it is re-read after 5 s up to three times, then the session keeps its model and edits, saves are held with "The project on disk changed and cannot be read; your edits are kept. Save a copy, or wait until the disk is complete." | A partial sync or a broken merge never replaces a good model |
| D22 | **Applying.** For each key the session merges baseline (the last loaded or saved records), ours (the current model) and theirs (incoming) with D2's core. With no conflict the result commits as one transaction `external.apply` (exposure `Internal`, submitted only by `Session::apply_external`, origin `Origin::External`, no normalisers, no prevent rules; checks run after), labelled "External change" with the git commit's short hash when `git rev-parse --short HEAD` answers. The baseline becomes the incoming generation; remaining local edits keep the project dirty. With conflicts nothing is applied: the session holds `ExternalChange {generation, changes, collisions}`, saves are refused, and `Session::apply_external(choices)` takes ours or theirs per colliding field. A gesture in progress defers the apply until it ends. Undo restores the pre-change model in memory; saving then writes it over the change | doc 05 §9 on the one core; no normaliser rewrites a state another build saved |
| D23 | **The journal follows the baseline.** After an apply the store starts a new segment at the incoming generation and journals one transaction `external.rebase` with the net local diff (incoming to current, assets included), then deletes the old segment, as after a save. A read-only session reloads instead: `Session::reload()` reopens the new generation and keeps nothing to merge | A crash after an apply recovers the live model; replaying the external change itself onto the new base would collide on every field |
| D24 | **The window** (doc 16a applies): the toast "External change applied: <n> records on folios <list> (git <hash>)" with Undo and Show changes (the history entry); the collision dialog, a form dialog listing each colliding field with derived tags ("W20 (-102G1:+ → -102F1:1) · overrides.colour"), base, mine and theirs, a choice per row, "All mine" and "All theirs", and "Decide later" (saves stay held, a banner says why); the read-only reload keeps page, zoom and selection where the records exist; the upgrade consent dialog of D11; File ▸ Export .ewz… and Open ▸ .ewz with "Extract and open" and "Open read-only" | doc 05 §16's touchpoints at their MVP size; the rich panel is WP-S2.15 |
| D25 | **Cloud folders** (doc 05 §15 rows 7 to 11): `ew_store::cloud::detect(path)` from the OneDrive environment variables, `~/Library/CloudStorage/`, `~/Library/Mobile Documents/` and Dropbox's `info.json`; a banner "This project is in a cloud-synced folder (OneDrive). Two people can open it within the sync delay; use git or a network share for teams." at open; on a cloud path EW-CORE-003 is re-checked after 5 s before it is reported. Online-only placeholders (Windows recall attributes, iCloud stubs and dataless files) open read-only with "<n> files are not downloaded" (de-scopable, §14.4) | FR-COL-008 with std APIs only |
| D26 | **`ew diff <from> <to> [--json]`**: each side a directory, an `.ewz`, an `.ewarchive` (`Session::open_archive`), `git:<rev>` or `rev:<label>` (the tag `ew/rev/<label>`). Both sides open read-only; `diff_model` and `derived_diff` print one line per changed entity labelled with the tags of its own side; exit 0 equal, 1 different with `--exit-code`, 2 errors. `git:<rev>` reads the blobs through one `git cat-file --batch` process: `<rev>:<project path>/manifest.json`, then each listed file, under the reader limits; the batch header (`<oid> <type> <size>` or `missing`) and the blob bytes are the only output parsed, a widening of WP-S1.18 D12. `--queries` is not built (FP-6) | gix stays 1.x; one process, a small fixed header to parse |
| D27 | **`ew diff --textconv <file>`** prints one line per record in file order: `<kind> <label> <path>=<value> …`, the members flattened to dotted paths with compact JSON leaves (`overrides.csa="1.5" overrides.colour=["BU"]`), labels derived from the project found in the working directory: the nearest ancestor holding a manifest, else the `*.ew` directories at most three levels below (tried against this repository's `golden/J-E-2416.ew` at depth 2), the first whose records contain the blob's IDs; unknown IDs print as kind and short ID | Deterministic text; doc 05 §11.6's example becomes the compact form |
| D28 | **`ew git setup [--global] [--hooks] [--yes]`** writes `merge.elektrawatt.name`, `merge.elektrawatt.driver = "<absolute ew path>" merge-driver %O %A %B %L %P`, `diff.elektrawatt.textconv` and `diff.elektrawatt.cachetextconv = true` through `git config` (argument vectors, exit status only) after consent: a y/N question on a terminal or `--yes`; without either it exits 2. It writes the managed `.gitattributes` and `.gitignore` blocks of doc 05 §11.1 into each project directory of the worktree, keeping lines outside the block. `--hooks` installs a `post-merge` hook running `ew check --integrity` and refuses to replace a hook it did not write. `Store::create` writes the two blocks only inside a git worktree; the golden fixture gets none | Git never takes drivers from a repository; consent stays the user's; no golden churn |
| D29 | **Integrity after merges.** `ew check --integrity` (from FT-191) runs EW-CORE-001 to -019 only and exits 1 on any error. **EW-CORE-013** is reported on open, after an external change and by `--integrity` when a group of the six uniqueness rules exists and the generation carries merge evidence (`merged: true`, conflict records or markers, a git merge or rebase in progress, or `--integrity --after-merge` as the hook passes it). The keeper order is doc 15 §6.4's (1) frozen or released, then (3) the older UUIDv7; step (2), the merge base, is FP-2. The "renumber the newer" fix is built for device tags (EW-RDS-004) as one previewed command; the other five rules report without a fix (FP-1). **AC-41** is defined on the data: base W20 `overrides {csa "1.5", show_marker true}`; branch A sets -102F2's function text and adds W20's `colour` BU; branch B sets `csa` "2.5"; the conflict variant has B also add `colour` WH | Deterministic on every machine; the acceptance names the data as it is |

No decision changes an ADR's decision text. D3 refines ADR-0011 decision 5's "release build" and D13 narrows
ADR-0012 decision 2's `object` strategy; both are recorded by dated notes on ADR-0011 and ADR-0012 at the design pass
after parts 1 and 4. D20 drops `notify` from doc 05 §9 and §19; D26 widens WP-S1.18 D12's git boundary.

## 5. Format and data

**Saved bytes.**
- Golden A, `golden/fixtures/release-min` and their exported bytes do not change in any part; no lane writes into
  `golden/`, and `golden/J-E-2416.ew` is opened only by reading or copied to a temporary directory.
- `min_reader` stays `"0.1"` for every 0.1 file (D3); `unstable` is not written before WP-S6.08 adds `"0.1"` to
  `RELEASED_FORMATS`.
- Part 1 changes the bytes of a save only for records with unknown members, which now keep their original line (D4).
- Part 4 defines new bytes: the merged manifest (D16), conflict records (D14) and the `$conflict` member.
- Part 5 types the reserved kind `conflict` within 0.1 (D17): `schemas/0.1` re-blessed additively, no format or `$v`
  bump; an older 0.1 build keeps conflict records verbatim as a reserved kind.
- Parts 2 and 3 write `.ewz` files outside the project's manifest; part 8 writes `.gitattributes`, `.gitignore`, the
  user's git configuration and a hook, none of them project data.
- **Migrations:** none ship (D10). A migration is all-or-nothing in memory, the original stays on disk until the
  consented save, and that save writes and verifies the backup first (D11).

**Data-loss cases and their tests** (each named in its part's acceptance):

| Case | Rule | Test |
|---|---|---|
| A file changed outside the session that the save would replace or remove | Save refused, nothing written (D5) | SG-3 |
| An untouched record with unknown members in a file the save rewrites | Original line kept (D4) | SG-2 |
| A save after a text-conflicted manifest | Generation max + 1, every file kept (D6) | SG-4 |
| A save whose next generation would pass 2^53 − 1 | Refused, nothing written; Save as… starts again at 0 (D6, ID-1144) | SG-4 |
| A move-aside under a sync client | Only a byte-equal duplicate stray removed (D7) | SG-5 |
| An `.ewz` extracted where the target exists, interrupted, or hostile | Refused before writing; the target never half-written; refused unread (D9) | EZ-3, EZ-4 |
| An older project whose upgrade is declined, or whose step fails | Nothing written; open refused with the path (D10, D11) | MG-2, MG-4 |
| A consented upgrade | The verified backup exists before the first project byte changes (D11) | MG-3 |
| A merge | No side lost: the result plus conflict records hold every input value; a side that does not parse is kept verbatim; `%A` untouched on an internal error (D14, D15) | MD-2, MD-4, MD-5 |
| A text merge with markers | Converted only when every hunk line parses; otherwise read-only (D18) | CR-2 |
| An external change colliding with unsaved edits | Nothing applied until chosen; saves held (D22) | XC-3 |
| An unreadable incoming generation | Model and edits kept; saves held (D21) | XC-5 |
| A crash after an applied external change with unsaved edits | Recovery equals the live model (D23) | XC-6 |
| A read-only follower's reload | Writes nothing (D23) | XC-7 |

## 6. Part 1: the store's stamps and safeguards

**Scope.** D3 to D7; FT-95, FT-96, FT-97, FT-98. **0.9 pw.** `ew-schema` (`since`, `RELEASED_FORMATS`), `ew-format`
(read, write, version), `ew-store` (save, open), `bin/ew` (`ew fmt` keeps original lines).

**Acceptance.**
- **SG-1** `min_reader` of Golden A is `"0.1"`; with an injected `since` table naming a member present in one record,
  it is that member's format: a unit test of the pure function (ID-1115), shown on a 1.x format because before 1.0 a
  format's major floor is the format itself, and never by opening a file whose `min_reader` is above the reader (the
  reader refuses it). A save writes no `unstable` while `RELEASED_FORMATS` is empty; with an injected released list
  without `FORMAT` it writes `unstable: true`. A manifest with `unstable: true` opens read-write in a reader of the
  same unreleased format and read-only in a released or different-format reader, the same in debug and optimised
  builds.
- **SG-2** A page file with one record carrying an unknown member placed before a known member, and an unknown kind's
  line, opens, takes an edit to another record of the same file, saves, and keeps both lines byte-identical; editing
  the first record re-emits it canonically with its unknown member kept; `ew fmt --check` reports nothing for the
  untouched file.
- **SG-3** A hand edit of a page file the next save replaces refuses the save with `SaveError::ExternalChange` naming
  the file; the file and the manifest are byte-identical afterwards; a CRLF-only change does not refuse. The same holds
  for a file the save removes and for an asset (ID-1115). A page file opened as CRLF and turned into LF on disk saves;
  one character changed instead refuses, nothing written (ID-1144).
- **SG-4** A manifest with conflict markers on the `generation` line (7 and 9) is followed by a save at generation 10;
  every listed file of both sides survives. At generation 2^53 − 1 a save is refused and every file is byte-identical
  afterwards; from 2^53 − 2 it writes 2^53 − 1 and reopens (ID-1144).
- **SG-5** A stray that a retried move-aside duplicates with equal bytes is removed; one with different bytes stays;
  the same in a roll-forward (ID-1115).
- **SG-6** Targeted checks for `ew-schema`, `ew-format`, `ew-store`, `ew`.

*As built (cycle 46, D46 and FIXD46; decision log ID-1152, ID-1156):* D3's stamps are `ew-schema` constants
(`Kind::since`, `KIND_SINCE`, `MEMBER_SINCE`, `VARIANT_SINCE`, `RELEASED_FORMATS` empty) read by `ew-format`'s
`since.rs` (`file_since`, `min_reader`); with `keep_format` a save's `min_reader` is the greater of the old and the
computed one. D5: a target deleted on disk is written again, not refused; a directory or a link at a target counts as a
change; the refusal names at most ten files, then "and N more"; a record file or library document held as CRLF also
keeps the hash of its LF form, the manifest, the plan, opaque files and assets the raw hash only. D6 also reads the
`generation` lines of the text the hunk sides share, and a save past `MAX_GENERATION` is refused with
`SaveError::GenerationLimit` before any temp. D7 lives inside the move-aside, so the roll-forward shares it. SG-4's
conflicted manifest is not tested through a roll-forward; FT-95's `unstable` marker waits for part 3.

## 7. Part 2: `.ewz` transport

**Scope.** D8, D9. **0.7 pw.** `ew-store` (`ewz.rs`, `vfs.rs`), `ew-api` (`Session::open_ewz`, the export), `bin/ew`
(`ew export ewz`, the reader argument, `ew extract <file.ewz> [--into <dir>]`). The window's menu items are part 7's.
`ew fmt --repair` (EW-CORE-005's fix: discard the unparseable lines after packing an `.ewz` of the current generation
beside the project; WP-S1.22 D2, 0.1 pw) moved to a lane of its own after part 2 (ruling 6 below), built to ID-1185's
rulings below: it discards the lines that are not JSON or not UTF-8, after packing the generation's files as found
(ID-1186) as `<project folder>.before-repair-g<generation>.ewz` in the project folder's parent.

**Acceptance.**
- **EZ-1** Golden A packed twice gives byte-identical `.ewz` files; `mimetype` is the first entry, stored; extracting
  gives a directory byte-equal to the source (FR-COL-009's acceptance).
- **EZ-2** Every reader of ruling 5 (`ew check`, `ew export pdf`, `ew export table` and `ew query` among them) on
  Golden A's `.ewz` gives the same exit code, stdout, stderr and written bytes as on the directory; `ew apply` and
  `ew import table` (with and without `--apply`) on it exit 2 with D9's text.
- **EZ-3** Extracting onto an existing directory exits 2 and writes nothing; a failure injected through the `Vfs` at
  each write leaves no `<target>` and removes the temporary directory.
- **EZ-4** Archives with `..`, a backslash, a duplicate name ignoring case, an entry outside D9's set, a hash mismatch,
  a missing `mimetype` or a 201 : 1 entry are refused unread (the bounds of `ZipLimits`); a fuzz target over the
  reader joins `fuzz/`.
- **EZ-5** (moved with `ew fmt --repair` to its own lane, ruling 6; built to ID-1185, whose ruling 8 adds cases)
  `ew fmt --repair` on a page file with one broken line writes the backup `.ewz`
  (`<project folder>.before-repair-g<generation>.ewz` beside the project folder), then a project without the line, and
  opens editable; without a broken line it changes nothing. As built after ID-1186: the backup holds the files as
  found, so a page file whose broken line was written by hand (the manifest untouched), or whose hash is pending in a
  merged manifest, is backed up as it is, the manifest hashed to it, and the report says "as found: 1 file edited
  outside EW" (`backup.edited` 1); the backup verifies, opens through `Session::open_ewz` and extracts through `ew
  extract` to every listed file as it was, while `ew export ewz` on that project still refuses. A file changed between
  the repair's open and its backup refuses with nothing written ("run the repair again"), as do no manifest, an
  incomplete save and a listed file that cannot be read.
- **EZ-6** Targeted checks for `ew-store`, `ew-api`, `ew`.

**Rulings at dispatch (ID-1175, cycle 47; facts at `b57b62db`).**
1. **The ZIP is `ew-zip`'s** (moved in cycle 46, ID-1145), reached as `ew_store::zip`; §2.1's and §14.2's `zip.rs` read
   `crates/ew-zip/src/lib.rs`. No new edge.
2. **Entry order.** `mimetype` (stored) first, `manifest.json` second, then the manifest's files and the git files
   present in strictly increasing byte order of their paths. The reader checks exactly this order (`ew_zip::read`
   returns entries in archive order) from the third entry on, and refuses any other; `archive.rs`'s order loop
   (`:333-340`, from the second entry) is not reused.
3. **Bounds.** The reader takes `ZipLimits::default()` (ratio 200 above the 64 KiB floor), not `open_archive`'s lifted
   ratio; EZ-4's 201 : 1 entry is larger than the floor (at least 327 compressed bytes). `pack` reads its archive back
   through the reader before it writes the file and refuses one the reader would refuse: "this project cannot be packed
   into an .ewz (<reason>); share the project folder instead".
4. **Extract** writes through `Vfs` only (`create_new`, `sync`, `sync_dir`; not `write_atomically`), so EZ-3's faults
   reach every write. `Vfs` gains a required `create_dir(path)` that fails when the path exists (`OsVfs` by
   `std::fs::create_dir`, `MemVfs`, and `archive.rs`'s test doubles `Racing` and `Faulty`); it creates the staging
   `<target>.tmp-ewz`. The finished staging directory is moved by `Vfs::rename` (not `rename_noreplace`, which takes
   files only) after the target's absence is checked again; `MemVfs`'s `rename` learns directory renames (a namespace
   operation moving every path under the prefix; today `mem.rs:550-553` refuses them). The residual race, an empty
   directory made at `<target>` between the check and the rename on Linux, is accepted and named in the module's note.
   Without `--into`, the target is `<stem>.ew` beside the archive (doc 05 §12).
5. **The CLI.** `ew export ewz [<project>] --out <file.ewz>`: `--out` is required ("`ew export ewz` writes a file: give
   `--out <file.ewz>`"). Readers that open an `.ewz` given as their project through `Session::open_ewz`: `check`, `sql`
   and `sql dump`, the row and page exports, `export table`, `print`, `render`, `verify-derived`, `query`,
   `fmt --check`. Every other subcommand that takes a project, `apply --preview`, `assets purge --preview` and
   `import table` without `--apply` included, refuses it with D9's text.
   Each new subcommand and flag enters `bin/ew/src/catalogue.rs`, gets a `--json` result under WP-S1.22's conventions
   (`crates/ew-api/src/cli_results.rs`, `bin/ew/schemas/` with `index.json`), re-blesses `bin/ew/api/cli.json`
   (additive), and one `docs/release-notes/unreleased.md` line.
6. **`ew fmt --repair` and EZ-5 move out of part 2** into a lane of their own after it, at `xhigh` (a save, ID-1029),
   ruled then: the store keeps a quarantined line's key and text but not its finding kind (`store.rs:1146-1149`),
   quarantines more than EW-CORE-005's lines (`read.rs:558-657`), has one read-only reason (`store.rs:1110`,
   `:129-144`), and every save writes the kept lines back (`store.rs:2187-2189`). Amended with it: §7's scope and
   `bin/ew` list (`:255-258`), EZ-5 (`:270`, marked moved), §4's rows `:101` and `:113`, §14.5's size (`:447`, part 2's
   0.8 pw loses `fmt --repair`'s 0.1) and §16's part-2 size (`:526`), and §15's list after part 2 (`:481`). §14's
   routing (`:416`) and WP-S1.22 D2 and D9 still give `--repair` to WP-S1.17 and do not change. A follow-up row is
   recorded at the design pass.
7. **Fuzz:** a target `ewz_read` over the reader, added beside `png_pdf` in `fuzz/Cargo.toml`, `fuzz/fuzz_targets/`,
   `fuzz/src/`, `fuzz/tests/replay.rs` (its array of four becomes a slice), `fuzz/README.md` (a row and a max_len line),
   `ci/fuzz-smoke-targets.py`'s list, `.github/workflows/deep-nightly.yml`'s matrix (its "(the four run in parallel)"
   loses the count), and `ci/fuzz-smoke-targets.toml`, which gains a `[targets.ewz_read]` entry with its roots by hand
   (`fuzz-smoke-table` refuses a target without one, `xtask/src/workflows.rs:533-538`) and is then regenerated by `cargo
   xtask fuzz-smoke-table`; these `ci/` and `.github/` files are granted to part 2 for that.
8. Golden A has no git files: EZ-1 adds them to a copy of Golden A that the test makes in a scratch folder; nothing new
   under `golden/`. Golden A is only read (`Session::open_read_only`), and packed into a scratch file.

**Rulings for `ew fmt --repair` and EZ-5 (ID-1185, cycle 47; facts at `a2fc2d5d`, part 2 as built).** They amend §7's
scope and EZ-5 where they say so; ruling 6 of ID-1175 moved them here. The lane is at `xhigh` (a save, ID-1029).
1. **What a repair discards.** Only the lines that gave EW-CORE-005 at line level: a JSON syntax error or a line that
   is not UTF-8 (`crates/ew-format/src/read.rs:575`, `:378-382`), as doc 05's quick fix says (§6.3). The other
   quarantined lines (EW-CORE-006, -007, -009, -019) stay byte for byte and are reported. A file-level EW-CORE-005 (a
   library document that is not UTF-8, `read.rs:734`) is not a line: it refuses the repair (ruling 3).
2. **The store tells them apart.** `quarantine()` (`read.rs:524`) takes the finding's kind at its eight call sites,
   and `KeptLine` (`read.rs:182-197`) carries it. The store keeps its kept entries as they are (`store.rs:649`) and
   adds the set of (file, line key) of the line-level EW-CORE-005 lines. The kind lives in memory only; kept lines are
   written back byte for byte, as today (`store.rs:2190-2191`).
3. **Refusals come first,** before anything is written, each named, exit 2, in this order: an `.ewz` as the project
   (ID-1175 ruling 5); a crashed session's journal, as `ew fmt` (`bin/ew/src/cli/project_files.rs:50-53`); any
   finding that forces read-only (`crates/ew-format/src/finding.rs:123-131`) other than a line-level EW-CORE-005.
   The reader counts those in a flag of its own, because `report.findings` is capped (`read.rs:270`,
   `finding.rs:212-216`) and a dropped finding must still refuse. A store that `pack_store` refuses (no manifest, a
   pending save) refuses too. With no refusal and no EW-CORE-005 line the repair writes nothing, prints "Nothing to
   repair" and exits 0.
4. **Opening for a repair.** `StoreOptions` (`store.rs:67-89`) gains a repair flag, which only the repair sets. With
   it the store takes the writer's lock, and the integrity demotion (`store.rs:1113-1114`) does not demote when the
   only forcing findings are line-level EW-CORE-005 lines. `open_editing` (`bin/ew/src/cli.rs:1833`) lets the repair
   through.
5. **The backup first.** `ew_store::ewz::pack_store` (`crates/ew-store/src/ewz.rs:410`) packs the generation's files
   as they are on disk, broken lines included; its read-back checks the archive's structure and hashes, and a broken
   JSON line inside a member must not refuse it (if it does, the lane stops). The archive is written beside the
   project, in its parent folder, as `<project folder>.before-repair-g<generation>.ewz` (this ruling's name), while
   the lock is held: to a temporary name, synced, then `Vfs::rename_noreplace` (`vfs.rs:77`, as the archive's writer
   does) and the folder synced. A name that exists is read. If it holds this generation's pack (equal bytes), it is
   reused and named in the report; otherwise the repair refuses (exit 2), naming the file and saying to move it. A
   parent folder that cannot be written refuses with nothing changed. *As built after ID-1186:* the backup is
   `ewz::pack_as_found`, the sibling of `pack_store` (sharing `pack` and the git files), not `pack_store`: the files as
   the store loaded them, each read again and refused when it changed since the open, `manifest.json` the loaded
   manifest hashed to them; so a file edited outside EW since the last save is backed up as found, not refused, and
   ruling 3's "a store that `pack_store` refuses" reads "files that cannot be packed as found" (ID-1186 ruling 4). The
   temporary name, `rename_noreplace`, the folder sync and the reuse on equal bytes are as above.
6. **Then one save.** A store method marks the files that hold the EW-CORE-005 lines for rewriting (`self.rewrite`,
   `store.rs:656`) and saves with `Changes::Keys` of no keys, through the save protocol (generation g+1). A
   rewritten file's records change form only (canonical), as `ew fmt`'s do; no record's content changes (WP-S1.22
   D9). The EW-CORE-005 entries leave the kept lines only after the save succeeds (in `adopt`, `store.rs:2046`), so a
   failed save loses nothing. Then the project is opened again. If it opens editable with no EW-CORE-005 finding the
   repair exits 0. Otherwise it exits 2, naming the finding and the backup; the new generation stays (EW never
   deletes and never rolls back by itself).
7. **The CLI.** `ew fmt [<project>] --repair`: the flag on `FmtCli` (`bin/ew/src/cli/tree.rs:944-958`, help
   `:116-126`, converted `:1780-1789`) and `FmtArgs` (`bin/ew/src/cli.rs:175-184`; built also at
   `project_files.rs:514`). `--repair` with `--check` is refused by the parser before any open (exit 2). The repair
   prints each discarded line from the store's own data (not `kept_lines`, whose list is capped): `file:line` and its
   text cut at 80 characters with "…" (this ruling's choice), then the backup's path. `--json` gives a result type in
   `crates/ew-api/src/cli_results.rs` (beside `FmtResult`), with its schema in `bin/ew/schemas/` and its `index.json`
   entry, and one line goes to `docs/release-notes/unreleased.md`. The catalogue's `fmt` row and its exit texts
   (`bin/ew/src/catalogue.rs:266-281`) and `bin/ew/api/cli.json` gain the flag.
8. **EZ-5** as specified, and: an EW-CORE-005 line beside an EW-CORE-007 line keeps the 007 line byte for byte; an
   EW-CORE-006 line refuses with nothing written; a forcing finding beyond the findings cap still refuses; a backup
   name holding another file refuses, one holding this generation's pack is reused. Faults injected through the `Vfs`
   at each write: during the backup (no backup left; the project unchanged); between the backup and the save (the
   project unchanged; a rerun reuses the backup); inside the save before its commit point (`store.rs:1874`; the
   project unchanged); after it (the next open completes generation g+1). A rerun after each fault succeeds. Golden A
   is never the repaired project; tests repair a copy.
9. **Files:** `crates/ew-format/src/{read.rs,finding.rs}`, `crates/ew-store/src/store.rs` (the options, the set, the
   repair's save), `crates/ew-store/src/ewz.rs` only if the backup's writing needs it, `crates/ew-api/src/{fmt.rs,
   cli_results.rs}` (a `repair` beside `format`, `fmt.rs:267`), `bin/ew/src/{cli.rs,catalogue.rs}`,
   `bin/ew/src/cli/{tree.rs,project_files.rs}`, `bin/ew/api/cli.json`, `bin/ew/schemas/`, the release notes, tests;
   doc 05 §6.3's EW-CORE-005 row as built (the backup's name). The window's banner stays part 7's.

**Ruling after FR47: the repair's backup holds the project as found (ID-1186, cycle 47; facts at `836cd1df`).** It
amends D8 and ID-1185's rulings 3 and 5 where it says so, and traces the backup to FR-COL-004 (a hand edit opens
"with an integrity notice and no data loss").
1. **Why.** A broken line almost always comes from an edit outside EW. The open adopts such a file with a
   `ForeignEdit` notice (`crates/ew-format/src/read.rs:437-477`). But its bytes no longer match the manifest, so
   `pack_store` refuses (`GenerationError::Changed`; a merged project's `Pending`; `store.rs:2610-2650`). As built,
   the repair refuses the case it exists for. A save before the backup is no remedy: it rewrites the files that were
   not canonical at open (`store.rs:2175`) and leaves a new generation even when the repair is then refused.
2. **The backup is packed as found.** A sibling of `pack_store` in `crates/ew-store/src/ewz.rs`, used by the repair
   alone, shares `pack` and the git-file loop (a private helper) without a second writer. It works as follows:
   - It reads every file the loaded manifest lists, through the store's `Vfs`.
   - It refuses when a file's bytes differ from what the store loaded (`Held.hash`): the file changed since the
     repair opened it, so nothing is written, and the text says to run the repair again.
   - It writes `manifest.json` as the loaded manifest (`store.manifest()`, `Manifest::to_text`) with each listed
     file's hash set to those bytes. A pending hash is set the same way. The generation and every other member stay
     as they are.
   The backup then verifies (doc 05 §12), opens through `Session::open_ewz`, and extracts to every listed file byte for
   byte as it was. Unlisted files (orphans, not read) are neither packed nor touched.
3. **D8 gains one exception:** an `.ewz` is one verified generation, except the repair's backup, which holds that
   generation's files as found. Its name stays `<project folder>.before-repair-g<generation>.ewz`.
   - The report says how many files differ from generation g as EW saved it: "backup <path> (generation g, as found:
     N files edited outside EW)", or without the clause when N is 0.
   - The `--json` result gains that count.
   - `pack_store` and `ew export ewz` are unchanged: an export hands over a verified generation, and still refuses a
     changed or pending file.
4. **Still refused,** with nothing written:
   - no manifest;
   - an incomplete save;
   - a listed file that cannot be read;
   - a file changed since the open (ruling 2);
   - the entry cap.
   The rest of ID-1185 stands: the order of the refusals, the name, the temporary file, `rename_noreplace`, the folder
   sync, and the reuse of an existing name only on equal bytes.
5. **Tests.**
   - EZ-5 on a hand-edited copy of Golden A, the broken line written without touching the manifest. The backup is
     written, then the project without the line, which opens editable. The backup extracts through `ew extract` with
     its manifest verified, every listed file equal to its pre-repair bytes. `ew export ewz` on the same project still
     refuses, with its text, asserted whole.
   - The same with a merged project's pending hash.
   - `a_repair_that_cannot_pack_the_backup_writes_nothing` takes a case that still cannot pack (a `Vfs` read fault on
     a listed file, or no manifest).
   - The reuse test computes its bytes with the sibling.
   - A file changed between the open and the backup refuses, with nothing written.
6. **Docs:**
   - doc 05 §6.3's EW-CORE-005 row ("after packing the files as found");
   - §12's "Uses", a third sub-bullet after `ew export ewz`: the repair's backup (§6.3), the generation's files as
     found, its manifest hashed to them;
   - D8;
   - ID-1185's ruling 5 and EZ-5 as built;
   - §3's trace, FR-COL-004;
   - the release note.

## 8. Part 3: format migrations and the upgrade backup

**Scope.** D10 to D12; FT-403. **0.8 pw.** `ew-format` (`migrate.rs`), `ew-store` (open, save consent), `ew-api`
(`confirm_upgrade`, `OpenReport::migrated_from`), `ew-commands` (`parse_fragment`), `bin/ew` (`ew upgrade`), `xtask`
(`check-format`). After parts 1 and 2.

**Acceptance.**
- **MG-1** With a test chain from a test format `"0.0"` (a kind step `$v` 1 to 2 renaming a member, a format step),
  a `"0.0"` fixture opens editable with every migrated file dirty and `migrated_from = "0.0"`; the shipped chain is
  empty.
- **MG-2** A step that fails refuses the open with `OpenError::Migration` carrying its JSON Pointer; nothing on disk
  changes.
- **MG-3** The first save returns `UpgradeConsent`; after `confirm_upgrade` the backup `<dir>.pre-0.0.ewz` exists,
  extracts byte-equal to the original directory, and only then the project is written in `FORMAT`; a second save
  does not ask.
- **MG-4** Declining (no confirmation, the session closed) leaves every project file byte-identical.
- **MG-5** `ew upgrade --check` exits 1 on the test fixture and 0 on Golden A; `ew upgrade --yes` writes the backup
  and saves; without a terminal or `--yes` it exits 2.
- **MG-6** `cargo xtask check-format` passes at the base and fails on an injected released list without a step; a
  fragment of the test format pastes through the test chain (FT-403).
- **MG-7** Targeted checks for `ew-format`, `ew-store`, `ew-api`, `ew-commands`, `ew`, `xtask`.

## 9. Part 4: the three-way merge core and `ew merge-driver`

**Scope.** D2, D13 to D16. **1.2 pw.** `ew-schema` (the strategy table, `CONFLICT_NS`), `ew-format` (`merge.rs`:
records, library documents, manifest), `bin/ew` (`ew merge-driver`), `fuzz/` (the driver target). Conflict records
are written here as lines; part 5 types them.

**Acceptance.**
- **MD-1** Unit cases for each row of the record table and each strategy of D13, including an enum member changed to
  different variants on both sides (one conflict, no union).
- **MD-2** Property tests: `merge(o,a,a) = a`, `merge(o,o,b) = b`, `merge(o,a,b)` equals `merge(o,b,a)` up to the side
  names; every value of a and b is in the result or in a conflict record.
- **MD-3** Every merged record of typed inputs deserialises typed.
- **MD-4** A side that does not parse, and one with a newer `$v`, keep the other side and add one file-level conflict
  with the lines verbatim and its `path`.
- **MD-5** The driver exits 0 and 1 as D15 says; an injected internal error exits 129 and `%A` is byte-identical; the
  4-argument form gives the same result.
- **MD-6** AC-41's driver half on Golden A copies (D29): the page-102 file merges cleanly to `csa "2.5"` and `colour`
  BU; the conflict variant writes one conflict record on `overrides.colour` and keeps BU in place; the manifest merge
  gives `max + 1`, `merged: true`, no `base`.
- **MD-7** Targeted checks for `ew-schema`, `ew-format`, `ew`; the fuzz target builds.

## 10. Part 5: conflict records, conflict markers and their resolution

**Scope.** D17 to D19; FR-COL-025. **0.9 pw.** `ew-schema` (the `conflict` kind), `ew-model` (its table), `ew-format`
(marker loading), `ew-store` (findings, read-only rule), `ew-commands` (`merge.resolve`, the delete path), `ew-checks`
(EW-CORE-011 from the model), `bin/ew` (`ew merge resolve`), `schemas/0.1`. After parts 1 and 4.

**Acceptance.**
- **CR-1** A project with MD-6's conflict record opens editable, reports EW-CORE-011 once, and saves the record back
  byte-identical; an older 0.1 file without conflicts reads as before; `schemas/0.1` re-blessed additively.
- **CR-2** A page file text-merged with diff3 markers on W20 opens with one conflict and the clean parts merged; the
  same without a base gives the D18 result; a hunk with a torn line keeps EW-CORE-006 and read-only.
- **CR-3** `ew merge resolve --take theirs --kind connection --field overrides.colour` leaves W20 with WH, no conflict,
  a canonical page file, and one undo step in a session; `--take ours` keeps BU.
- **CR-4** Deleting a record removes its conflict in the same transaction; undo restores both. EW-CORE-011 appears in
  `ew check --mode release` and blocks the gate.
- **CR-5** Targeted checks for `ew-schema`, `ew-model`, `ew-format`, `ew-store`, `ew-commands`, `ew-checks`, `ew`.

## 11. Part 6: external change in the store and the session

**Scope.** D20 to D23. **1.1 pw.** `ew-store` (poll, git busy, the incoming read, adopt, journal rebase), `ew-commands`
(`external.apply`), `ew-api` (`Session::poll_disk`, `apply_external`, `reload`). After parts 1 and 4.

**Acceptance.**
- **XC-1** In a temporary copy of Golden A opened for editing, a second store's save that changes folio 106 is
  detected after quiescence and applied as one step "External change"; undo restores the pre-change model; saving
  after the undo writes the earlier state (FR-COL-007's acceptance).
- **XC-2** With a local unsaved edit on folio 101 and the external change on 106, the apply keeps the edit and the
  project dirty; the next save writes only the edit's files and passes D5.
- **XC-3** A local and an external change of W20's colour collide: nothing applies, saves are refused, the collision
  names the field; choosing theirs, then mine, gives the expected models in one step each.
- **XC-4** A git operation in progress (a test `index.lock`) defers detection until it is gone; the session's own save
  is never taken for an external change.
- **XC-5** An incoming generation with a missing listed file applies nothing, keeps the edits and holds saves with
  D21's text; it applies once the file appears.
- **XC-6** A crash after an applied external change with unsaved edits recovers the live model (D23).
- **XC-7** A read-only session reloads on each of the holder's saves and writes nothing.
- **XC-8** Targeted checks for `ew-store`, `ew-commands`, `ew-api`.

## 12. Part 7: the window, cloud folders and the driver status

**Scope.** D24, D25, and the status-bar badge "Merge driver not configured" for a project in a git worktree whose
`git config --get merge.elektrawatt.driver` fails (exit status only), with "Set up merge driver" running part 8's
library call. **0.8 pw.** `ew-store` (`cloud.rs`), `ew-viewmodel` (dialogs, toast, banners, menus, strings),
`ew-ui`. After parts 2, 3 and 6; the badge's action after part 8's library. Every text follows
[design language](../../design/16a-design-language.md) §8.

**Acceptance.**
- **XW-1** Headless: an external change shows the toast with its folios; Undo takes it back.
- **XW-2** The collision dialog lists XC-3's field with base, mine and theirs; "All theirs" applies in one step;
  "Decide later" leaves the banner and a held save.
- **XW-3** A read-only window reloads on a save and keeps its page and zoom.
- **XW-4** A project under a test OneDrive variable shows the cloud banner; elsewhere none.
- **XW-5** An unlisted `pages/<id> (conflicted copy).jsonl` is listed at open as EW-CORE-004 (built; the test is new).
- **XW-6** Export .ewz… writes EZ-1's file; Open with "Open read-only" opens from memory, "Extract and open" refuses an
  existing folder; the upgrade dialog asks once (MG-3 through the window).
- **XW-7** The driver badge appears in a temporary repository without the configuration and disappears after setup;
  `ui.describe` words for every new control; the window check at 1920 × 1080 and the 1280 × 800 floor.
- **XW-8** Targeted checks for `ew-store`, `ew-viewmodel`, `ew-ui`.

## 13. Part 8: `ew diff`, `ew git setup`, post-merge integrity and AC-41

**Scope.** D26 to D29; `ew check --integrity` (FT-191's option). **0.9 pw.** `ew-store` (`git.rs`: config, cat-file,
hook, managed blocks), `ew-api` (diff of two sessions), `ew-checks` (EW-CORE-013, the renumber fix), `bin/ew`
(`diff`, `git setup`, `check --integrity`). After parts 4 and 5.

**Acceptance.**
- **DG-1** `ew diff` between Golden A and a copy with -101F1's part changed prints `-101F1: part … → …` and exits 1
  with `--exit-code`; the same between a directory and its `.ewz`, and between `git:HEAD` and the working copy in a
  temporary repository; `--json` validates against its result schema.
- **DG-2** `git log -p` in a temporary repository after `ew git setup --yes` shows textconv lines with derived tags
  for a page file change (FR-COL-013's acceptance).
- **DG-3** `ew git setup` without a terminal or `--yes` exits 2 and changes nothing; with `--yes` it writes the four
  keys and the blocks, keeps a user's line outside the block, and is idempotent.
- **DG-4** `--hooks` installs the hook and refuses to replace a foreign one.
- **DG-5** A merged copy of Golden A with two devices -102F3 (`merged: true`) reports EW-CORE-013 with the older
  UUIDv7 as keeper and the fix renumbering the other to the next free counter in one undo step (FR-CHK-012's
  acceptance); without merge evidence only EW-RDS-004 reports.
- **DG-6** `ew check --integrity` exits 1 on CR-1's project and 0 on Golden A.
- **DG-7 (AC-41)** On a fresh temporary clone of a repository holding a Golden A copy: `ew git setup --yes`, two
  branches as D29, `git merge` exits 0 with the clean result and `ew check --integrity` exits 0; the conflict variant
  leaves one conflict record, the project opens, and `ew merge resolve --take ours` makes it clean. Without setup the
  status is "merge driver not configured" (XW-7).
- **DG-8** Targeted checks for `ew-store`, `ew-api`, `ew-checks`, `ew`.

## 14. Files owned, boundaries, risks, size and routing

### 14.1 Files owned

Each part owns the crates named in its scope and fixes the file list in its dispatch brief; new modules:
`crates/ew-format/src/{migrate.rs,merge.rs,since.rs}`, `crates/ew-store/src/{ewz.rs,cloud.rs,external.rs}`, the new
`ew-schema` tables, `bin/ew/src/cli/{collab.rs,diff.rs}`. Shared append-only files (registry lists, fuzz targets,
the strings table, generated schemas and command pages) may be touched by other lanes; the second to integrate
rebases. Read-only: other crates, `golden/**`, `ci/**`, `.github/**`, `docs/**` except generated reference pages.

### 14.2 Boundaries with the neighbours

| Neighbour | Owns | This package |
|---|---|---|
| WP-S1.18 (complete) | Revisions, the release, the archive, `zip.rs`, the git release steps | Reuses `zip::write`/`read` for `.ewz` and the `open_archive` pattern; reads `.ewarchive` and `ew/rev/<label>` tags in `ew diff`; changes no archive byte |
| WP-S1.22 | The CLI conventions, `ew fmt`, `ew recover`, the API guard and the CLI snapshot (part 5) | Builds `ew diff`, `ew upgrade`, `ew merge-driver`, `ew merge resolve`, `ew git setup`, `ew export ewz`, `ew extract`, `ew fmt --repair` and `ew check --integrity` (D2 of WP-S1.22), each joining its conventions and snapshot; changes `ew fmt` only to keep original lines (D4) |
| WP-S3.09 (not specified) | Importer contracts and the migration report of foreign formats | "Migration" here is only ElektraWatt's own format chain (D10); no shared code; WP-S3.09 must not add steps to `Migrations` |
| WP-S6.04 | The threat model, signing, the trust store, the supply chain and the fuzz harness conventions | "Integrity" here is the project-integrity check (FR-CHK-012) and the bounds of the new parsers; the merge-driver and `.ewz` fuzz targets follow its harness; no signing |
| WP-S6.08 | The first signed release | Adds `"0.1"` to `RELEASED_FORMATS` and `fixtures/format/0.1/` (D3, D12) |
| WP-S2.15, WP-S1.31 | The 1.0 merge panel, cross-file reconciliation | Submit `merge.resolve` and reuse D2's core |
| WP-S3.04 | `--omit-personal-data` | Adds the flag to `ew export ewz` |

### 14.3 What a lane may not decide

A lane stops and reports when: a change needs a new crate, a new external dependency or a new `ci/dep-rules.toml`
edge; a kind, member or variant beyond D14 and D17 would be typed or added; any exported byte of Golden A changes or
a file under `golden/` would be written; a strategy beyond D13's table seems needed; a shipped migration step would be
added; an external change or a resolution would be applied without the core of D2; a save could proceed with a
target changed outside the session; `external.apply` could be reached from a script, a replay, MCP or a plugin; a
later part's work is reached. The lane decides private names, module layout inside its files, test fixtures and
texts beyond those quoted (doc 16a §8).

### 14.4 Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 A merge produces a structurally invalid record | Medium · high | Atomic by default (D13); MD-3; the fuzz target |
| R2 An external change overwrites unsaved work or a pull | Low · high | D5, D21, D22; XC-2, XC-3, XC-5; parts 1 and 6 at `xhigh` |
| R3 The journal and the baseline disagree after an apply | Medium · high | D23; XC-6 |
| R4 `unstable` read-only surprises at the release | Low · medium | D3 marks nothing before the release |
| R5 Polling misses a change or reads a half pull | Medium · medium | The manifest is the commit point; quiescence and git state (D20); D21 |
| R6 The package outgrows 7.4 pw | Medium · medium | The reserve and the de-scope rule below |

### 14.5 Size, order and routing

**Size.** Part 1 **0.9** (stamps 0.2, original lines 0.3, target pre-check 0.15, FT-97 and FT-98 0.15, tests 0.1);
part 2 **0.7** (pack 0.15, extract 0.2, read-only open and the CLI argument 0.2, tests 0.15; `fmt --repair`'s 0.1
moved to its own lane after part 2, ID-1175);
part 3 **0.8** (chain 0.2, open and consent 0.25, `ew upgrade` 0.1, corpus check 0.15, FT-403 0.05, tests 0.05);
part 4 **1.2** (table 0.15, record and field merge 0.4, conflict records 0.15, manifest and library documents 0.15,
driver 0.15, property and fuzz tests 0.2); part 5 **0.9** (typing 0.25, markers 0.25, `merge.resolve` and CLI 0.25,
tests 0.15); part 6 **1.1** (poll and quiescence 0.2, incoming and adopt 0.25, apply and collisions 0.35, journal
rebase 0.15, reload 0.05, tests 0.1); part 7 **0.8** (dialogs and toast 0.35, menus 0.1, cloud 0.2, badge 0.05,
tests 0.1); part 8 **0.9** (diff 0.25, textconv 0.15, git setup and blocks 0.2, integrity and EW-CORE-013 0.2, AC-41
0.1). Total **7.4 pw plus 0.3 reserve**.

**De-scope rule** (a part projected or found more than 20 % over its size), in this order, without a further ruling,
each becoming a follow-up: `ew git setup --hooks`; online-only placeholders (D25); `rev:<label>` sides; FT-403; the
collision dialog's per-field choice (All mine and All theirs stay); textconv discovery below the working directory.

**Order and routing** (Opus or Haiku only, ID-700). Wave A: part 1 (**Opus `xhigh`**, the save and the format),
part 4 (**Opus `xhigh`**, new format bytes), part 2 (**Opus `xhigh`**, file writes; beside part 1, different files).
Wave B: part 3 after parts 1 and 2 (**Opus `xhigh`**), part 5 after parts 1 and 4 (**Opus `xhigh`**, a core crate),
part 6 after parts 1 and 4 (**Opus `xhigh`**, concurrency and data loss). Wave C: part 8 after parts 4 and 5 (**Opus
`high`**), part 7 after parts 2, 3 and 6 (**Opus `medium`**, window work). The feasibility read flags parts 1 and 3
to 6 (format, store, a core crate). **Unblocks:** WP-S6.08 after part 8; WP-S2.15 and WP-S1.31 on D2 and D19.

## 15. Follow-ups, documents to update, open questions

**Follow-ups.** FT-95, FT-96, FT-97 and FT-98 close with part 1; FT-403 with part 3; FT-191's `--integrity` with part
8. Proposed (not opened; the design pass numbers them):
- **FP-1** "Renumber the newer" for EW-TERM-002, EW-POT-015, EW-CORE-052, EW-PLC-001 and EW-PLC-007.
- **FP-2** EW-CORE-013's keeper step (2), the merge base, with WP-S1.31.
- **FP-3** "Merge this copy" for an EW-CORE-004 conflict copy through D22.
- **FP-4** "Merge into current" after a lost lock (doc 05 §7.2) through D22.
- **FP-5** ZIP64 for `.ewz` beyond 65,535 entries; the export-date rule of doc 05 §12.
- **FP-6** `ew diff --queries` (bom, wires, terminals, plc, xref) and per-language `MlText` merging.
- A de-scoped item of §14.5 becomes a new FT at the design pass.

**Documents.** **After part 1:** doc 05 §4 and §10.3, §10.4 (stamps, original lines as built), §5.2 step 2 (D5),
§11.3 (FT-97); ADR-0011 a dated note (D3). **After part 2:** doc 05 §12 (D8, D9), doc 17 §3.3 (`export ewz`,
`extract`); `fmt --repair`'s row in doc 17 §3.3 after its own lane (ID-1175). **After part 3:** doc 05 §10.1, §10.2, §10.4 (the chain, the consent, the corpus).
**After part 4:** doc 05 §11.2, §11.3 (D13 to D16, the file-level `path`); ADR-0012 a dated note (D13). **After part
5:** doc 05 §2.2, §6.3 (EW-CORE-006 and -011), §11.4; doc 15 §6.5. **After part 6:** doc 05 §9 (D20 to D23) and §19
(no `notify`). **After part 7:** doc 05 §15, §16; doc 16 (the dialogs). **After part 8:** doc 05 §11.1, §11.6, §11.7
(W20 as the data holds it), doc 15 §6.4 (the MVP keeper order), doc 17 §3.3, doc 20 AC-41.

**Open questions for the orchestrator.**
- **O1 (effort).** The parts are 7.4 pw plus 0.3 reserve against a 6.8 pw PERT. *Options:* (1) re-estimate
  WP-S1.17 at O 5.5 · L 7.5 · P 10.5 · PERT 7.7 (σ 0.8) in `work-packages.csv` and regenerate the schedule; (2) keep
  6.8 by moving parts 3's consent and backup and part 8's textconv to the first format change and 1.0.
  *Recommendation:* (1); the Shoulds left are what AC-41 and the 0.1.0 pilot rely on.
- **O2 (detection).** D20 polls the manifest instead of `notify`. *Options:* (1) polling; (2) `notify` 8.2.0 as doc
  05 §9 names. *Recommendation:* (1): no dependency, the manifest is the commit point, and D5 covers hand edits.
- **O3 (migrations at MVP).** No user file can need a step before 0.2. *Options:* (1) build part 3 now with an empty
  shipped chain; (2) defer part 3 to the package that first changes the format. *Recommendation:* (1): FR-COL-010
  is a Must and the first format change should not also build the machinery.
- **O4 (EW-CORE-013).** *Options:* (1) report on merge evidence only (D29); (2) report on every duplicate group,
  doubling the six rules' findings. *Recommendation:* (1), with FP-1 putting the fix on the rules' own findings later.

---

## 16. Amendment after the owner's format decision (cycle 42, decision log ID-1029)

The owner, 2026-10-31: "The app is not in use yet. Change the format however much you like." Before the first signed
release (WP-S6.08 adds the first entry to `RELEASED_FORMATS`) a format change needs no migration, no reader for old
files and no byte rule kept only for compatibility with other builds; Golden A and the fixtures are re-blessed instead.
This section overrides §4 to §15 where they differ; the rulings on O1 to O4 are in decision log ID-1015.

**Deferred to the wave before the first release, beside WP-S6.08** (one part, still called part 3, re-read and sized
at its dispatch):
- **Part 3 whole:** D10 to D12 (the migration chain, the upgrade consent and its verified backup, `ew upgrade`,
  `cargo xtask check-format`, the corpus) and FT-403. O3's ruling (build part 3 now with an empty chain) is superseded.
- **D4 and FT-96** (an unedited record's unknown members kept by their original line): it serves only files written by
  another build, which cannot exist before the release. Part 1 drops it, and SG-2 moves to part 3; until then such a
  record is re-emitted canonically with its unknown members kept, as built.
- **D3's `unstable` marker** (writing `unstable: true` and the read-only open of an unstable file in another reader):
  `RELEASED_FORMATS` is empty until then, so the marker could not be written anyway. Part 1 keeps D3's `since` metadata,
  the computed `min_reader` and the empty `RELEASED_FORMATS`; SG-1's `unstable` clauses move to part 3.
- The data-loss rows of §5 for D4 (SG-2) and for D10 and D11 (MG-2 to MG-4) move with them.

**Kept where they are:** the data-loss protection inside a session: D5 (a save refuses to overwrite a file changed
outside the session, part 1), D6 and D7, the journals (D23), and the merges (D2, D13 to D19, D22). FT-95's `min_reader`
half, FT-97 and FT-98 still close with part 1, FT-191's `--integrity` with part 8.

**Size.** The parts left for M2 are part 1 **0.55** (stamps without the marker 0.15, the target pre-check 0.15, FT-97
and FT-98 0.15, tests 0.1), part 2 **0.7** and `fmt --repair`'s lane 0.1 (ID-1175), part 4 **1.2**, part 5 **0.9**, part 6 **1.1**, part 7 **0.8** (the upgrade
consent dialog of D24 leaves with part 3) and part 8 **0.9**: 6.25 pw. The PERT is **6.9 pw** (O 4.8 · L 6.7 · P 9.7):
the 7.7 pw of O1 less 0.8 pw (decision log ID-1029). The 0.65 pw between the parts and the PERT is the reserve, wider
than §14.5's 0.3 pw because part 1's D4 work (0.35 pw) leaves the M2 sizing too; the deferred part 3 (about 1.15 pw
with D4 and the marker) is sized against WP-S6.08 when that package is specified.

**Order and routing** (replacing §14.5's routing; Opus or Haiku only, ID-700). Format work routes at Opus `high`; a
part whose subject is a save, a journal, recovery or a merge stays at `xhigh`:

| Part | Subject | Routing | Wave |
|---|---|---|---|
| 1 | Stamps and the save's target pre-check | Opus `xhigh` (a save) | A |
| 2 | `.ewz` pack, extract, read-only open | Opus `high` (format; its extract is all-or-nothing, tested by EZ-3) | A |
| 4 | The three-way merge core and `ew merge-driver` | Opus `xhigh` (a merge) | A |
| 5 | Conflict records, markers, `merge.resolve` | Opus `xhigh` (a merge) | B, after parts 1 and 4 |
| 6 | External change, the apply and the journal rebase | Opus `xhigh` (a journal, a merge) | B, after parts 1 and 4 |
| 8 | `ew diff`, `ew git setup`, integrity, AC-41 | Opus `high` | C, after parts 4 and 5 |
| 7 | The window, cloud folders, the driver status | Opus `medium` | C, after parts 2 and 6 |
| 3 | Migrations, the upgrade backup, D4, the `unstable` marker, FT-403 | Opus `xhigh` (a save with a backup) | Before the first release, beside WP-S6.08 |

The feasibility read flags parts 1, 2 and 4 to 6. **Unblocks:** WP-S6.08 after parts 8 and 3.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.09, WP-S1.17, WP-S1.18, WP-S1.22, WP-S6.08; Appendix B (FT-95 to
  FT-98, FT-191, FT-403).
- [Decision log](../06-decision-log.md): ID-200 to ID-217, ID-334, ID-566, ID-596, ID-700, ID-827, ID-836, ID-895.
- [Project format](../../design/05-project-format-and-persistence.md) §4 to §12, §15, §16, §18, §19;
  [checks, revisions, collaboration](../../design/15-checks-revisions-collaboration.md) §6;
  [extensibility](../../design/17-extensibility.md) §3; [golden project](../../design/20-golden-project.md) AC-41.
- [ADR-0008](../../adr/0008-transactions-undo-preview.md), [ADR-0009](../../adr/0009-invariant-catalogue.md),
  [ADR-0010](../../adr/0010-project-storage-jsonl-directory.md), [ADR-0011](../../adr/0011-format-versioning-and-migration.md),
  [ADR-0012](../../adr/0012-git-integration.md).
- Requirements: FR-COL-007 to FR-COL-010, FR-COL-013, FR-COL-014, FR-COL-024, FR-COL-025, FR-COL-029, FR-CHK-012,
  NFR-REL-05, NFR-REL-06.
