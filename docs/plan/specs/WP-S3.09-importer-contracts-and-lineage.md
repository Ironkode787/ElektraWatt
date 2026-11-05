# WP-S3.09: Importer contracts, lineage and the migration report (closed specification)

**Status:** built in cycle 51 by one lane in two sittings (S309, S309b) with its fix round FIX309 (decision log
ID-1360, ID-1374, ID-1379), complete when cycle 52's first review part accepts FIX309; specified in cycle 50
(documents only), an M2 package built whole by one lane (ID-1281) in the seven steps
of §6, with one hand-back point (§11); about 3.1 pw by its steps, the PERT of 3.1 pw unchanged · **Date:** 2026-11-04 ·
**Work package:**
[WP-S3.09](../02-work-packages.md#wp-s309-importer-contracts-lineage-and-migration-report) · **Decision log:**
ID-1016, ID-1281, ID-1282, ID-1321 · **Part of:** ElektraWatt plan, specifications

**Purpose.** The importer contract of doc 14 §2.1 exists in `ew-interop-api` in a minimal form, and the QET project
import has run since cycle 11 as a spike: the batch becomes the whole model through `Session::from_records`, with no
`ApplyImport`, no preview, no import run (the kind `import_run` is a reserved marker type), no re-import into a
lineage, and a migration report that lives only as long as the command that printed it. The input limits are spread
over six crates as constants, an importer panic ends the session, and no importer has a cargo-fuzz target. This
document closes doc 14's MVP row "Contracts, lineage, import run, migration report, input limits and fuzz targets"
([interop](../../design/14-interop.md) §16) to decisions, a build order and acceptance, against the code at
`9d111131` (§2.1). The format-specific importers stay with WP-S3.10 onward. Its size is an estimate, not a commitment
(ID-1210); every time is not measured (owner's rule). Where this document and a design document differ, this document
is the narrower and wins for the lane; the design documents stay the target and are updated at the design pass (§13).
A choice the lane meets that this document does not settle is escalated, never guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The texts](#5-the-texts)
6. [Build order](#6-build-order)
7. [Acceptance](#7-acceptance)
8. [Golden B and C's expected changes](#8-golden-b-and-cs-expected-changes)
9. [Neighbours, files owned and what a lane may not decide](#9-neighbours-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, routing and the hand-back point](#11-size-routing-and-the-hand-back-point)
12. [Follow-ups, proposed](#12-follow-ups-proposed)
13. [Open questions and the documents to update](#13-open-questions-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package every importer is called through one front door that enforces one `Limits` value,
runs the importer under `catch_unwind` and checks the batch's contract before anything reaches a model; a QET project
becomes a project through `ApplyImport`, a transaction run on a fork with the normalisers and the structural
invariants (prevent rules relaxed to findings), shown first as a preview of its pages and counts and committed, into an
empty project, as a confirmed checkpoint; the transaction writes the lineage and an `ImportRun` record with the
import's findings, statistics and record hashes; the migration report is a run of the `migration` scheme that lists
the persisted import findings beside the live lenient findings, one row per condition, also after the project is saved
and reopened; a re-import of an edited source into its lineage keeps the IDs of unchanged source keys and previews a
diff classed unchanged, source-changed, EW-changed, conflict, added and removed, never deleting silently; the
`Exporter` trait of ID-1016 exists with the table writers as its implementation; and the four importers that exist
(QET project, QET element, the golden tables, the parts table) each have a registered cargo-fuzz target with seeds.

**Non-goals** (each with its package or follow-up):
- No new importer and no new mapping: QET formulas, structure nodes, parts, text to data, cables, report blocks,
  title-block forms, the import passes P1 to P7 and the detections of FT-128 to FT-143 and FT-173 to FT-178 are
  WP-S3.10 and WP-S3.11's (doc 14 §4.16). The migration report shows what the importers detect as built.
- No proposal review queue inside the import preview (doc 14 §3.1 step 7): a proposal stays a quick fix on its finding
  after the commit (FP-1). No `acknowledged` entries and no acknowledge command (FP-2).
- No import as a copy (FR-IMP-045, Could, 1.0) and no import of a QET project into an existing project as a new
  lineage (FP-3); no `ew import qet --batch` (1.0).
- No `ImportBatch.assets` (no importer emits an asset; ADR-0021 amendment 1, FP-4), no `TableImage` (QETZ, 1.x), no
  `TargetHints`, options, cancellation or progress members of `ImportContext` (they arrive with the importers that need
  them).
- No QET-mirror corpus job (doc 14 §12, SC-4, WP-S3.10; FP-5) and no counting-allocator hostile corpus (WP-S6.04).
- No fuzz target for a parser that is not an importer (the table round trip, the SQL sandbox, SVG and raster assets,
  textconv, command logs: WP-S6.04).
- No Fluent messages for import findings: their text stays the importer's English (FP-6).
- No new Rust dependency; no change to `deny.toml`, `supply-chain/` or `Cargo.lock` beyond the fuzz crate's own lock.

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [Interop](../../design/14-interop.md) §1.3, §2.1 to §2.5, §3.1 to §3.4, §4.15, §4.16, §11, §12, §15, §16 | The contract, lineage, report and run, the migration report, the pipeline, lenient boundaries, re-import, limits, corpora, Golden C |
| [Architecture](../../design/04-architecture.md) §10.6 | The contract and the threat row "Malicious project or import file" |
| [Domain model](../../design/03-domain-model.md) `:520-521` | `ImportLineage` and `ImportRun` |
| [Project format](../../design/05-project-format-and-persistence.md) §2.1 (`imports/`), §3.2a, the kind table (rank 72) | The run's file, the v5 name layout |
| ADR-0004 amendments 1 and 4; ADR-0021 decisions 1 to 3, amendments 1, 2, 5, 6; ADR-0011 amendment 4 | Identity, the contract, typing a reserved kind within 0.1 |
| [Golden project](../../design/20-golden-project.md) §20, AC-25; `golden/J-E-2416/README.md` "Golden C as built"; `golden/J-E-2416/expected/c-migration-report.csv` | Golden C's counts and statuses |
| Decisions ID-1016 (WP-S3.08's O2: the `Exporter` trait is this package's), ID-274, ID-275, ID-281, ID-299, ID-312, ID-375 | §4 |
| Specification [WP-S3.08](WP-S3.08-table-exports-and-excel-round-trip.md) D13 | The round trip is not an `ImportBatch` import |

### 2.1 What exists at `9d111131` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| `Importer` (`id`, `version`, `detect`, `import(&ImportInput, &mut ImportContext)`), `ImportBatch {records, lib, report}`, `LineageInfo`, `Limits` (eight XML and input members, presets `element_definition`, `bootstrap_tables`, `stricter`), `ImportContext {project, lineage, as_of, limits, mode}` | `crates/ew-interop-api/src/importer.rs:16-276` | D1 to D4 |
| The crate's own list of what it leaves to WP-S3.09: `Proposal`, `TableImage`, the `Exporter` trait; no `targets` on `ReportEntry` | `crates/ew-interop-api/src/lib.rs:25-40` | D1, D14 |
| `ImportInput` (a file name, never a path; bytes; SHA-256), `check_file_name` | `crates/ew-interop-api/src/input.rs:15-89` | D2 |
| `ImportReport`, `ReportEntry {code, severity, defect, message, location, fix}`, `ImportStats` (with `suppressed`), `Code` (`EW-IMP-nnn`) | `crates/ew-interop-api/src/report.rs:22-294` | D1, D9 |
| `SourceRef {lineage, key, extra}`; every record kind an importer emits has a `source_ref` member | `crates/ew-schema/src/value.rs:966-977`; e.g. `crates/ew-schema/src/device.rs:85` | D3 |
| `ImportLineage` typed (rank 15, `project.jsonl`); `ImportRun` a marker type "reserved in v0"; the kind `import_run` at rank 72 with the `imports/` file rule; `last_run` documented as a reference to a kind without a table | `crates/ew-schema/src/project.rs:30-31`, `:476-505`; `crates/ew-schema/src/kind.rs:301`; `crates/ew-model/src/links.rs:846-848` | D6 |
| `lineage_namespace(importer, anchor)` and the `NS_IMPORT` names of ADR-0004 amendment 4 | `crates/ew-ids/src/v5.rs:166-187` | D3, D11 |
| Importers: `QetProjectImporter` and `QetElementImporter` implement `Importer`; `TablesImporter::import(&[ImportInput], &ImportContext)` (an input set, no `impl Importer`); `ew_import_catalog::parse_parts(input, map, master, target)` | `crates/ew-import-qet/src/project/mod.rs:110`, `crates/ew-import-qet/src/element.rs:552`; `crates/ew-import-tables/src/import.rs:71`; `crates/ew-import-catalog/src/parts.rs:811` | D2, D4, D15 |
| Limits held as crate constants: `MAX_ZIP_ENTRIES` 100,000 (XLSX); `MAX_ROWS` 1,000,000, `MAX_COLUMNS` 1,024, `MAX_CELL_BYTES` 32 KiB, `table_limits()`; `MAX_TABLE_FILES` 64; report caps `MAX_PROJECT_ENTRIES`, `MAX_REPORT_ENTRIES`, `MAX_ENTRIES_PER_DEFINITION` | `crates/ew-import-catalog/src/xlsx.rs:34`, `table.rs:12-22`; `crates/ew-import-tables/src/lib.rs:104-107`; `crates/ew-import-qet/src/project/cx.rs:19`, `crates/ew-import-qet/src/lib.rs:94-109` | D2 |
| XML: the QET reader pre-scans before `roxmltree` (depth, attributes, nodes, text) and `roxmltree`'s default refuses a DTD; the XLSX walker refuses unknown entities but ignores a DOCTYPE event | `crates/ew-import-qet/src/xml.rs:1-30`, `:209`; `crates/ew-import-catalog/src/xlsx.rs:221-272` | D2, D5 |
| The spike front door: `import_qet(name, bytes, library)` imports with `new_project_context()` (nil project, lenient) and opens `Session::from_records`, then runs `lenient_findings` (the `migration` scheme) | `crates/ew-api/src/import.rs:1-201`; `crates/ew-import-qet/src/project/mod.rs:55-59`; `crates/ew-api/src/lib.rs:644` | D4, D7, D9 |
| `MigrationReport` (import entries grouped by code plus `LiveGroup`s), its md, csv and json renderings | `crates/ew-import-qet/src/migration.rs:18-37`, `:214`; `crates/ew-api/src/import.rs:137-139` | D9 |
| The `migration` scheme: `pre-delivery`'s rules, lenient, "the run behind a migration report" | `crates/ew-checks/src/scheme.rs:113-121` | D9 |
| The rule catalogue has no `EW-IMP` row | `crates/ew-checks/catalogue/rules.json` | D9 |
| `ew import qet <file> [--content] [--report md\|csv\|json] [--report-out] [--strict] [--json]` reports only and writes no project; `ew import csv` writes a new project through `create_project` (sibling directory, renamed when complete) | `bin/ew/src/cli.rs:585-614`, `:3939-4069`, `:4074-4170`; `crates/ew-api/src/import.rs:566-641` | D7, D8, D12 |
| The window's File ▸ Import QET project reads the file, imports and writes a new project folder at once (`create_and_install`), with a banner of counts | `crates/ew-viewmodel/src/shell/projects.rs:225-302` | D8 |
| The engine: `Preview` (candidate, ops, diff, affected pages) and `Engine::preview`; an internal command is applied without the prevent hook; `diff_model` | `crates/ew-commands/src/engine.rs:124-175`, `:430-440`, `:543-549`; `crates/ew-commands/src/diff.rs:148` | D7, D11 |
| The Preview tab (one card for every origin: Summary, Entities, Tags, Xrefs, Pages) and a job kind for long work | `crates/ew-viewmodel/src/preview.rs:1-40`; `crates/ew-viewmodel/src/shell/table_file.rs:1-20` | D8, D12 |
| Golden A's `project.jsonl` holds the golden lineage (`csv␟fixed␟golden`) with `last_source_sha256` and no `last_run` | `golden/J-E-2416.ew/project.jsonl` | D13 |
| Golden C: the corpus-gated tests `the_migration_report_has_the_expected_counts_per_code` and `the_fixed_import_compared_with_golden_a` (skipped without `EW_PRIVATE_CORPUS`); the expected counts per code | `crates/ew-api/tests/it/golden_c.rs:331-380`, `:679`; `golden/J-E-2416/expected/c-migration-report.csv` | D16 |
| A synthetic QET project of Golden A's folios 101 to 103 (public) | `crates/ew-api/tests/qet_synth/mod.rs` | §7 |
| The fuzz crate: six targets (`jsonl_loader`, `migration_chain`, `command_sequence`, `png_pdf`, `ewz_read`, `merge_file`); the CI-PR-08 table with each target's closure, checked by an `xtask` test; the deep nightly's matrix; the seed generator and the replay test | `fuzz/Cargo.toml:56-100`; `ci/fuzz-smoke-targets.toml`; `.github/workflows/deep-nightly.yml:282-306`; `fuzz/src/bin/seed_corpus.rs`; `fuzz/tests/replay.rs` | D15 |
| The `Exporter` trait does not exist; WP-S3.08's table writers and readers are in `ew-reports` and `ew-api` | `crates/ew-interop-api/src/lib.rs:39`; `crates/ew-api/src/output/table_io.rs:1-25` | D14 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Acceptance |
|---|---|---|
| FR-IMP-001 | Pure importers behind one front door; `ApplyImport` turns a batch into a previewed transaction in lenient mode; a fuzz target per importer (D4, D7, D15). Content sniffing as built (`detect`); the EDZ clause is 1.0's (no EDZ importer) | IC-4, IC-6, IC-17 |
| FR-IMP-002 | `v5(namespace, source_key)` as built; the contract check re-derives every ID (D3); the same `.qet` twice gives identical IDs and an empty diff (D11) | IC-3, IC-12 |
| FR-IMP-003 | The migration report is a run of the `migration` scheme: persisted import findings plus live lenient findings, one row per condition, with location and quick fix (D9). The detections the acceptance names (D-03's spelling variants, S56 and E56 without parts) are WP-S3.10 and WP-S3.11's | IC-9, IC-10 |
| FR-IMP-004 | The preview (pages, counts, entity diff) before the commit; one transaction; into an empty project a confirmed, non-undoable checkpoint; Cancel leaves the project unchanged (D7, D8, D12) | IC-6, IC-7, IC-8, IC-13 |
| FR-IMP-005 | One `Limits` from the front door; archive and table limits as its members; DTDs refused; NaN refused with a report entry (D2, D5) | IC-1, IC-2, IC-5 |
| FR-IMP-040 (Should) | Re-import into a lineage with the six classes, conflicts and removals kept and listed (D10 to D12) | IC-12, IC-13 |
| FR-IMP-041 | `ImportRun` typed and written in the import transaction; its findings in the migration report after save and reopen (D6, D9) | IC-8, IC-10 |
| NFR-SEC-02 (importers' part) | D2, D5; the PR hostile corpus with its allocation check is WP-S6.04's | IC-1, IC-2 |
| NFR-SEC-18 | Importers read only their inputs: a source scan forbids file, network, process and environment access in the importer crates; DTD, XInclude and external entities refused (D5); the file-access trace is WP-S6.04's | IC-5 |
| AC-25 (this package's part; G-M3, with WP-S3.10 and WP-S3.11) | Golden C imports through `ApplyImport`: 18 folios without structural error, the 63 texts as before, the migration report from the run (D16) | IC-16 |
| FR-EXP-001 (the trait clause, ID-1016) | The `Exporter` trait over an `ExportView` with the table writers as its implementation (D14; §13 O3) | IC-15 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The batch as built is the contract.** `ImportBatch` keeps `{records, lib, report}`: no `provenance` vector (each record's own `source_ref` member is its provenance, ADR-0021 decision 1 reads so at the design pass) and no `assets` (FP-4). `ReportEntry` gains `targets: Vec<RecordKey>` (`RecordKey {kind: ew_schema::Kind, id: Uuid}`, default empty), which the importers fill where they know the record (every entry about an emitted record) and which D9's folding reads. Every addition to `ew-interop-api` builds on Rust 1.85 (no let chains, no newer library API) | A parallel vector would duplicate a member every record already has and could disagree with it; targets are what "one row per condition" needs |
| D2 | **One `Limits` for every input limit.** `Limits` gains `max_archive_entries` (100,000), `max_expanded_bytes` (2 GiB), `max_entry_bytes` (512 MiB), `max_entry_ratio` (200), `max_input_files` (64), `max_table_rows` (1,000,000), `max_table_columns` (1,024) with these defaults (doc 14 §11's "Archives" and "CSV, XLSX" rows), and `stricter` covers them. The constants `MAX_ZIP_ENTRIES`, `MAX_ROWS`, `MAX_COLUMNS`, `MAX_TABLE_FILES` become reads of `ctx.limits` (or of the target's `limits` for `parse_parts`); `table_limits()` stays a preset built from the members (`max_text_bytes` 32 KiB). Report-volume and work caps (`MAX_PROJECT_ENTRIES`, `MAX_REPORT_ENTRIES`, `MAX_ENTRIES_PER_DEFINITION`, the style-pair caps, `MAX_PAIR_PROBES`, `MAX_PATH_BYTES`, `MAX_MANTISSA`) stay crate constants: they bound work and report size, not input (doc 14 §11). `check_archive_name(name)` beside `check_file_name` refuses `..` segments, absolute and drive paths, `\`, `:`, NUL and control characters, and the XLSX reader uses it. The project loader (`ew-format`'s `Limits`) and `.ewz` (`ew-zip`, 65,535 entries) keep their own limits: they are not importers | One place to read, test and document; a limit is never a second constant that can drift. The `.ewz` bound is stricter than NFR-SEC-02's because the writer has no ZIP64 |
| D3 | **The contract check.** The front door (D4) refuses a batch, with T2, when two records share `(kind, id)`; when a record with a `source_ref` names another lineage than the batch's; when `v5(namespace, source_ref.key)` differs from the record's ID for a kind whose ID the source key gives (every kind but the project record of a bootstrap, the library copies, and template functions and channels, whose IDs ADR-0004 amendment 2 derives); or when records are not sorted by kind rank, then ID. It never repairs a batch | ADR-0021 amendment 5's re-minting, done now for every importer: an importer bug is refused before a model sees it, and FR-IMP-002's identity becomes a checked property |
| D4 | **One front door: `ew_api::import`.** Every host call of an importer (`ew import qet`, `ew import csv`, `ew import parts`, `parts.import`'s reader in the window, the window's QET import, the fuzz targets) goes through one function that (a) refuses inputs above `limits.max_input_bytes`, a total above `max_expanded_bytes` and a set above `max_input_files` before calling the importer; (b) runs the importer inside `catch_unwind`, a panic becoming T1 with the model untouched (NFR-REL-12's importer half); (c) checks D3 for the batch kinds (QET project, tables). The window runs it, and D7's preview, as a job (`ew-jobs`: its own `catch_unwind`, the window stays responsive), the CLI in-line | The limits, the panic isolation and the contract are enforced once and cannot be skipped by a new host path |
| D5 | **NFR-SEC-18 in code.** (a) A lexical check in `xtask` (a step of `check-deps`, which lane-check's policy step and CI already run) fails when a non-test source file of `ew-interop-api` or an `ew-import-*` crate names `std::fs`, `std::net`, `std::process`, `std::env`, `File::`, `TcpStream` or `Command::new`; (b) the XLSX walker refuses a DOCTYPE (an `ImportError::Malformed` naming it), as `roxmltree` already does for QET; (c) fixtures: a `.qet` with an external entity, one with an `xi:include` (kept as unknown content and reported, never followed), an XLSX part with a DOCTYPE. The parts import's sibling tables are read by the host by fixed names in the chosen file's folder (`read_parts_tables`), never named by file content: they are the selected source, a set of inputs (ADR-0021 amendment 6) | The purity rule becomes a check instead of a review item; one DTD rule for every XML reader |
| D6 | **`ImportRun` typed within the unreleased 0.1** (ADR-0011 amendment 4: a reserved kind with a rank and a file rule; no format or `$v` bump; `schemas/0.1` re-blessed with additions only). Members in this order: `id` (UUIDv7 from the engine's ID source), `lineage` (`Id<ImportLineage>`, a link: EW-CORE-008 when dangling, no cascade), `importer`, `importer_version`, `source` `{name, sha256, size, format, format_version?}`, `as_of?`, `mode` (`strict` or `lenient`, catch-all), `entries` (each `{code, severity, defect?, message, location?, targets[], fix?}` in the importer's order; `fix` as `{label, command, params}`), `stats` `{per_kind, dropped, inferred, lost_connections, suppressed}`, `record_hashes` (D11), `extra`. File `imports/<run-uuid>.jsonl`, one line. `ImportLineage.last_run` becomes a link to the run (no cascade). A run is never changed after its transaction and no command deletes one at the MVP | Every member has a reader in this package; a build from before the change keeps the reserved kind verbatim (ID-206), so nothing is lost. No migration (owner, 2026-10-31) |
| D7 | **`ApplyImport` is one internal transaction, `import.apply`,** built in `ew-commands` (not in the public command catalogue: `ew apply` and the command schemas do not gain it; origin `import`). On a fork of the target it (1) puts the batch's library documents through the embedding path of `part.assign`'s copies and its records; (2) creates or updates the `ImportLineage` (`last_source_sha256`, `last_run`); (3) runs the normalisers; (4) checks the structural invariants, never relaxed (doc 14 §3.3), while the prevent hook does not run (the engine's internal path, `engine.rs:437`), so prevent violations stay in the model for the lenient findings; (5) writes the `ImportRun`. A structural violation refuses the import with the report and leaves the target unchanged (doc 14 §2.1). Into an empty project the committed candidate is the new project's first state: no undo step (the checkpoint of ADR-0021 decision 3). `Session` gains `import_preview` (the job's result: the candidate, its pages, counts and diff) and `commit_import` | One path for every batch import; the rule that structural invariants hold and prevent rules become findings is the engine's, not each host's |
| D8 | **Import into a new project.** `ew import qet <file> --out <dir>` writes the project (the `create_project` rule: a sibling directory renamed when complete; `<dir>` absent or empty) after printing the report; without `--out` it writes nothing, as today. In the window, File ▸ Import QET project, after the file and the folder, opens the Preview tab's card (origin `import`) with the Summary (T5's counts and the finding counts by severity) and the Pages list; Apply asks the checkpoint confirmation T4 (headless: `confirm: true` on the action, else the step fails naming it, as the other confirmations), then writes and opens the project and shows the migration report (D9) in Messages; Cancel writes nothing | FR-IMP-004 in both front ends; the folder is still chosen first, as today, so no new dialog step is added |
| D9 | **The migration report is a check run.** (a) The `EW-IMP` codes that the importers emit at the base, plus EW-IMP-010 and EW-IMP-011 (D12), are rows of `ew-checks`' catalogue, message class 160 (`EW-IMP-101` prints `160101`), with a rule page each (`cargo xtask docs-gen`), severity as doc 14 §2.5's table; (b) a run of the `migration` scheme (and of `full`) reads the `ImportRun` of each lineage's `last_run` and adds each entry as a finding: rule its code, the entry's severity, message and location in words, targets, quick fix; (c) **folding**: an entry whose code and a live rule form a pair of the fixed list {EW-IMP-102 with EW-RDS-010, EW-IMP-105 with EW-CORE-023, EW-IMP-114 with EW-CORE-063} and whose targets equal a live finding's is not listed; the live finding carries the import code as its origin and the entry's fix when it has none; (d) `ew import qet --report` renders this run of the candidate, `ew check --scheme migration` the same run of a saved project, and the window's Messages panel shows it when the project has a lineage (a scheme choice "Migration report" beside the project's scheme). `MigrationReport` becomes a rendering of the run; its md, csv and json formats keep their columns | FR-IMP-003 and FR-IMP-041 in one run: import-only facts survive the import, live ones disappear when fixed, and each MF condition is one row (doc 14 §2.5) |
| D10 | **Lineage detection.** The importer emits its lineage with its natural anchor (as built). The host compares the batch lineage's namespace with the target's lineages: equal (a QET project UUID) is the lineage; otherwise, when a target lineage has the same importer and `source_name` and at least 50 % of the batch's source keys (by `source_ref.key`) exist in it, the host proposes it (T6) and, when accepted, imports again with `ctx.lineage` set to it, so the IDs come from its namespace; otherwise the import into this project is refused (T7; FP-3) | Doc 14 §3.4's rule, deterministic, without the importer seeing the model |
| D11 | **Record hashes and the base.** `record_hashes` maps each imported record's ID to `{source, ew?}`: `source` the first 128 bits (32 hex digits) of SHA-256 over the record's canonical line as the importer emitted it, `ew` the same over the record as committed after the normalisers, omitted when equal; both lines without the `source_ref` member. The base of a re-import is the lineage's `last_run` | Doc 14 names one hash, but the normalisers may change an imported record in the commit, so one hash would class every such record EW-changed on the next re-import |
| D12 | **Re-import** (FR-IMP-040), per record of the batch and of the base: *unchanged* (source hash = base `source`, current = base `ew`); *source-changed* (source hash differs, current = base `ew`: the batch's record is applied); *EW-changed* (source hash = base, current differs: kept); *conflict* (both differ: kept, EW-IMP-011 per record); *added* (ID not in the base: applied); *removed* (in the base, not in the batch: kept, EW-IMP-010 per record). "Current" is the hash of the record in the target now (`ew` rule of D11); a base record already deleted in EW stays deleted. The preview card (origin `import`) shows T8 and the rows of the four non-trivial classes with ours and theirs per changed member for a conflict (`diff_model`'s field paths); Apply commits one undoable transaction; Cancel leaves the target unchanged. The CLI: `ew import qet <file> --into <project>` prints T8 and the rows and writes nothing; `--apply` commits and saves (the project lock taken as every writing command) | Never silent deletion, EW's edits win by default, and a conflict is a listed choice (doc 14 §3.4); one transaction per re-import |
| D13 | **Which imports persist a run.** Batch imports of project records into a lineage (the QET project import and, later, WP-S3.10's formats) write an `ImportRun`. The golden CSV bootstrap (`ew import csv`), the parts table import (library documents, doc 07 §3.6) and the table round trip (WP-S3.08 D13) write none, and Golden A stays byte-equal (§13 O1) | A run records import-only findings of a lenient import; the strict bootstrap refuses on any error and is the format's reference fixture, which a UUIDv7 run file would make non-reproducible |
| D14 | **The `Exporter` trait** (ID-1016) in `ew-interop-api`: `trait Exporter: Send + Sync { fn id(&self) -> &'static str; fn export(&self, view: &ExportView<'_>, sink: &mut dyn std::io::Write) -> Result<ExportReport, ExportError>; }`; `ExportView {rev: u64, records: &[Record], rows: &BTreeMap<String, RowTable>, context: ExportContext}` with `ExportContext {date, language, omit_personal_data}`; `ExportReport {rev, outputs: Vec<OutputMeta {name, sha256, media_type}>, findings: Vec<ReportEntry>}`. `RowTable` is a new plain-data row set written for the trait in the permissive crate (`name`, `key`, columns `{key, label, kind}`, rows `{id?, heading, cells}` with each cell as its neutral text and kind); `ew-reports`' GPL `Table` (`crates/ew-reports/src/table.rs:25`) stays where it is and converts to and from it inside `ew-reports`, so no GPL code moves into the permissive layer. The CSV presets and the XLSX writer implement the trait in `ew-reports`, and `ew export … --format xlsx\|csv` and the window's table export call them through it with byte-equal outputs | FR-EXP-001's trait clause waits for this package (WP-S3.08 D13); an implementation keeps the trait honest without moving PDF or SVG exports (their trait use is FP-7); the licence boundary is kept by a new permissive type instead of a relicensed one |
| D15 | **Fuzz targets** (this package's four; WP-S6.04 owns the others, its D5 and D9): `qet_project` (bytes as one `.qet` through the front door, then `ApplyImport` on an empty project), `qet_element` (bytes as one `.elmt` under `Limits::element_definition()`), `golden_tables` (a selector byte splits the input into up to eight named tables of the golden dialect, then the bootstrap's own path, `session_from_batch`, which D13 keeps), `parts_table` (bytes as a CSV or XLSX supplier table with a fixed column map). Properties: no panic; every emitted record re-reads through `ew-schema`'s strict reader (`Record::load` of its own line, no malformed variant) and `Record::validate` passes (the typed reader is the strict check, ADR-0011 amendment 3); D3 holds; where `ApplyImport` (`qet_project`) or the bootstrap's session (`golden_tables`) runs, it commits or refuses with a report and a committed model passes the structural check. Each is a `[[bin]]` of `fuzz/Cargo.toml` with its harness in `fuzz/src/`, seeds under `fuzz/corpus/<target>/` from `seed_corpus.rs` (the `.elmt` fixtures, `qet_synth`'s project, Golden A's tables, a CSV and an XLSX parts table), a row of `ci/fuzz-smoke-targets.toml` (`cargo xtask fuzz-smoke-table`) and an entry of the deep nightly's matrix (`max_len` 65,536; 16,384 for `qet_element`) | Doc 14 §11's three properties, the schema half through the typed reader instead of a JSON Schema validator (no new dependency); the names say which importer each target drives (doc 14 §11's `table_csv` and `table_xlsx` are `golden_tables` and `parts_table`) |
| D16 | **Golden C through the pipeline.** `import_qet` becomes the front door plus `ApplyImport` into an empty project, so the corpus-gated tests run the real path. The expected file `c-migration-report.csv` keeps its columns; its `as_built` column is the run's count per code, and a count that changes is re-blessed with a `Golden:` trailer and named in the commit (§8). The 18 folios and 63 texts are unchanged | AC-25's part that is this package's: the pipeline, not new detections |
| D17 | **No decision changes an ADR's decision;** D1, D6 and D11 are recorded at the design pass as ADR-0021 and ADR-0011 notes (§13) | — |

*As built (2026-11-05, cycle 51; decision log ID-1360, ID-1374, ID-1379).* D4's ruled deviation: the window runs the
importer on a job thread, while the transaction and the preview stay on the window thread, since a `Session` cannot
move between threads (ID-1360); the re-import's importer runs once, on the job thread, as one job (FIX309, ID-1379).
At the card's Apply the unsaved-changes question is asked again for whatever project is open then, Cancel or Escape
keeping the edit and bringing the card back (ID-1374's major, fixed by FIX309). D4's fuzz host for one `.elmt` is
`ew_api::import::import_qet_element`, so the fuzz crate holds no edge to `ew-import-qet` (ADR-0024 decision 4,
ID-1379). D11 as built: the hash is the first 128 bits of SHA-256 over the record's canonical line (`$k` first, the
members in declaration order, as `ew-format` writes it) less the `source_ref` member, for both `source` and `ew`
(ID-1374; the lane's first build hashed sorted-key JSON, and the format is free before 0.1.0, ID-1325). D12 as built:
a record the base lists as removed keeps its base hashes, so it is classed removed again on each re-import of the
same file and a new run is written (a row); `ew import qet --strict --out` writes nothing when it exits 1, as
`ew import csv --strict` does.

## 5. The texts

Every text is asserted whole (en-GB). `<file>` is the source's file name, `<folder>` a folder's name; records are named
as `words` names them. The nl-NL texts are the orchestrator's (ID-1320): the lane writes them and lists them for the
orchestrator's read.

| # | Where | Text |
|---|---|---|
| T1 | Front door, importer panic (CLI exit 2; window banner) | `The importer stopped on an internal error while reading <file>; nothing was imported.` |
| T2 | Front door, contract refused | `The import of <file> was refused because <reason>; nothing was imported.` with `<reason>` one of `two records share one identity`, `a record's identity does not follow from its source key`, `a record names another import`, `the records are out of order` |
| T3 | Run too large (its line above the loader's line limit) | `<file> makes more records than one import can record (<n>); nothing was imported.` |
| T4 | Checkpoint confirmation (window) | title `Create the project from <file>?`; body `The import creates a new project in <folder> with <p> pages and <d> devices. It cannot be undone: delete the folder to discard it.`; buttons `Create project`, `Cancel` |
| T5 | Preview card Summary, new project | `<p> pages, <d> devices, <c> connections; <e> errors, <w> warnings and <i> infos in the migration report` |
| T6 | Lineage proposal (window question; CLI notice with `--into`) | `<file> matches the earlier import of <source name> (<k> % of its items). Re-import it into that import?`; buttons `Re-import`, `Cancel` |
| T7 | Refusal, no lineage | `<file> is not an earlier import of this project; import it into a new project instead.` |
| T8 | Re-import summary (card and CLI) | `Re-import of <file>: <a> added, <s> changed in the source, <e> kept with your changes, <c> changed on both sides and kept, <r> removed in the source and kept, <u> unchanged.` |
| T9 | EW-IMP-010 (warning) | `<record> was removed in <file>; it is kept here. Delete it if it is no longer needed.` |
| T10 | EW-IMP-011 (warning) | `<record> was changed both in <file> and here; this project's version is kept.` |
| T11 | Messages scheme choice | `Migration report` |
| T12 | CLI, `--into` without `--apply` | `nothing was written: add --apply to commit the re-import` |

## 6. Build order

One lane builds the seven steps in this order and commits at the end of each.

1. **The contract** (D1, D2, D14): `Limits`' new members and `stricter`, `check_archive_name`, `RecordKey` and
   `ReportEntry.targets`, the importers reading `ctx.limits` (the constants of D2 go), the `Exporter` trait with the
   table writers implementing it (outputs byte-equal); MSRV 1.85 for the permissive crate.
2. **The format** (D6): `ImportRun` typed in `ew-schema`, the link and its catalogue line in `ew-model`, round-trip
   and unknown-value tests, `schemas/0.1` re-blessed (additions only).
3. **The front door** (D3, D4, D5): `ew_api::import`'s one function with the pre-checks, `catch_unwind` and the contract
   check; every host path moved onto it; the `xtask` source check; the DTD rule and the NFR-SEC-18 fixtures.
4. **`ApplyImport` into a new project** (D7, D8, D13): `import.apply`, `Session::import_preview` and `commit_import`,
   the run written; `ew import qet --out`; the window's job, card and checkpoint; frames of §7 IC-7.
5. **The migration report** (D9, D16): the catalogue rows, rule pages and Messages scheme choice, the run's findings and
   the folding, `ew check --scheme migration` after reopen; Golden C's corpus-gated tests run (§13 O2) and
   `c-migration-report.csv` re-blessed where the run's counts change, with its `Golden:` trailer.
6. **Re-import** (D10 to D12): detection, the hashes, the classes, EW-IMP-010 and -011, `--into` and `--apply`, the
   window's card; frames of IC-13.
7. **Fuzz targets** (D15): four harnesses, seeds, the `[[bin]]` entries, the smoke table regenerated, the nightly's
   matrix; the replay test passes for the new seeds.

## 7. Acceptance

- **IC-1** (FR-IMP-005, NFR-SEC-02) Per new `Limits` member a test feeds the smallest input above it (generated, not
  committed) through the front door: the refusal names the limit (`ImportError::LimitExceeded`'s text whole); a
  `Limits::stricter` test covers every member.
- **IC-2** An XLSX entry named `../x`, `/x`, `C:x`, `a\b` and one with NUL is refused by `check_archive_name` with its
  whole text; a ZIP bomb (an entry over the 200 : 1 ratio) is refused; a `.qet` with a NaN coordinate imports with an
  EW-IMP-002 entry and no panic.
- **IC-3** (FR-IMP-002) The contract check refuses four hand-made batches with T2's four reasons; every batch of the
  committed fixtures passes it.
- **IC-4** (NFR-REL-12) A test importer that panics, through the front door: T1, and a session's model digest
  unchanged; headless, the window's import of it shows T1 in the banner and the session goes on.
- **IC-5** (NFR-SEC-18) The `xtask` check fails on a fixture crate that names `std::fs` and passes the workspace; the
  three fixtures of D5 (c) are refused or reported as D5 says.
- **IC-6** (FR-IMP-001, FR-IMP-004) `ew import qet` of the synthetic folios 101 to 103 with `--out`: the project opens
  with the batch's records after the normalisers, an `import_run` file under `imports/`, `last_run` set; without
  `--out` nothing is written; two runs with the same engine seed give equal bytes.
- **IC-7** (window, FR-IMP-004) The import opens the card with T5; Cancel writes no folder; Apply shows T4, then the
  project; frames at 1280 × 800 and 1920 × 1080 of the card and the confirmation.
- **IC-8** (FR-IMP-041) A run round-trips through save and load byte-equal; an unknown member and an unknown `mode`
  survive; `schemas/0.1`'s diff holds additions only.
- **IC-9** (FR-IMP-003) The migration report of the synthetic import lists each import entry once and the live
  findings of the `migration` scheme; a fixture with an EW-IMP-114 entry and an EW-CORE-063 finding on the same target
  lists one row under EW-CORE-063 with the import code as its origin.
- **IC-10** (FR-IMP-041) After save and reopen, `ew check --scheme migration` lists the same import findings
  (EW-IMP-006 as an info on the fixture with a substituted font).
- **IC-11** Every code an importer emits at the end of the package has a catalogue row of class 160 and a rule page;
  `cargo xtask docs-gen --check` is clean.
- **IC-12** (FR-IMP-002, FR-IMP-040) Re-importing the identical file gives T8 with every count 0 but unchanged, and
  writes nothing on Apply; re-importing a variant with one more terminal after a function-text edit in EW adds that
  terminal and its connections, keeps the edit and lists no other change; a record changed on both sides is kept with
  EW-IMP-011, a record removed in the source is kept with EW-IMP-010; the IDs of unchanged source keys are equal.
- **IC-13** (window, FR-IMP-004) The re-import card with T8 and its rows; Cancel leaves the model digest unchanged;
  Apply is one undo step; T6 on a fixture without a project UUID; T7 on a foreign file; frames at both sizes.
- **IC-14** Golden A's bytes are unchanged (the bootstrap writes no run, D13); AC-15's paths (a) and (b) stay equal.
- **IC-15** (FR-EXP-001) The CSV and XLSX exports of every table key give the same bytes through the trait as before
  (the existing golden digests); an `ExportReport` names the revision.
- **IC-16** (AC-25, corpus-gated, §13 O2) Golden C imports through `ApplyImport` with 18 folios, no structural error,
  the 63 texts as before; the run's counts per code equal `c-migration-report.csv`.
- **IC-17** (NFR-SEC-03 for importers) The four targets build (`cargo check` of the fuzz crate, lane-check's `fuzz`
  step), their seeds replay clean (`fuzz/tests/replay.rs`, run from `fuzz/` at the merge point), the `xtask` table test
  passes, and the delivery's CI run has a 60-second `fuzz-smoke` per target and a complete deep-nightly run without a
  finding.
- **IC-18** The hand-back checks of the lane's package set pass (clippy, fmt, doc, nextest, settle, policy, api-guard:
  `--out`, `--into` and `--apply` are additions).

## 8. Golden B and C's expected changes

| Golden | Change |
|---|---|
| Golden A (`golden/J-E-2416.ew/`, `golden/J-E-2416/expected/`) | None (D13); a changed byte stops the lane |
| Golden B (`variants/b1-…`, `b2-…`) | None |
| Golden C (`golden/J-E-2416/expected/c-migration-report.csv`) | The `as_built` column may change where the normalisers and the run's folding change a count (D16); each changed row is named in the step-5 commit with its reason and the `Golden:` trailer the orchestrator gives. The statuses `holds`, `differs` and `ft` follow the counts; the `doc20` column is unchanged |

## 9. Neighbours, files owned and what a lane may not decide

### 9.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S3.10, WP-S3.11 | The QET mapping and detections (FT-127 to FT-143, FT-173 to FT-178), the import passes, AC-25's MF rows | Gives them the front door, `ApplyImport`, the run and the report |
| WP-S6.04 | The threat model, the other fuzz targets, the hostile corpus with its allocation check, NFR-REL-12 beyond importers | Its importer rows and the importer half of NFR-REL-12 (D4) |
| WP-S3.08 | The table round trip and the table writers | D14 calls its writers through the trait, bytes unchanged |
| WP-S1.15 | The `migration` scheme and the rule engine | D9 adds findings from runs to the scheme's run |
| WP-S1.19 | `parts.import` and the parts reader | Moves its reader onto the front door only |

### 9.2 Files owned

`crates/ew-interop-api/`; `crates/ew-schema/src/project.rs` (`ImportRun`) and `schemas/0.1/`; `crates/ew-model/src/links.rs`
(the run's link); `crates/ew-import-qet/src/{lib,xml}.rs`, `project/cx.rs` and the emitters of `targets`;
`crates/ew-import-tables/src/{lib,import}.rs`; `crates/ew-import-catalog/src/{table,xlsx,parts}.rs`;
`crates/ew-commands/src/commands/` (a new import module) and its registry; `crates/ew-api/src/import.rs`, the session's
import functions, `crates/ew-api/tests/it/golden_c.rs`; `crates/ew-checks/` (catalogue rows, the run's findings, the
folding, rule pages); `crates/ew-reports/` and `crates/ew-api/src/output/` where the writers meet the trait;
`crates/ew-viewmodel/src/shell/projects.rs`, the import card and job, `strings/`, `locales/`; `crates/ew-ui/` (the card's
summary, the confirmation, the scheme choice); `bin/ew/src/cli.rs` (`import qet`) and `bin/ew/api/cli.json`;
`xtask/src/deps.rs` (the source check); `fuzz/` (four targets, seeds, `Cargo.toml`, `Cargo.lock`),
`ci/fuzz-smoke-targets.toml`, `.github/workflows/deep-nightly.yml` (the matrix);
`golden/J-E-2416/expected/c-migration-report.csv` and `golden/J-E-2416/README.md`'s "Golden C as built" counts.
Read-only: `deny.toml`, `supply-chain/`, `Cargo.lock`, `golden/J-E-2416.ew/`, every other document.

### 9.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a format member, kind or link beyond D6 seems needed, or a member must change type;
2. a Golden A or Golden B byte changes, or a Golden C count changes for a reason other than D16's;
3. a normaliser run by `ApplyImport` would delete or replace an imported record (it may add or fill);
4. a text would differ from §5, or a new dependency, a change to another package's mapping or a GitHub setting seems
   needed;
5. the private corpus is needed and not given (§13 O2);
6. a host path calls an importer and cannot go through the front door.

The lane decides freely: module, function and job names, the card's layout under the design language, the CLI's
summary lines other than §5, the harnesses' input encodings, the seeds, and the nl-NL wording (listed for the
orchestrator, ID-1320).

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The normalisers change imported records (CP instantiation, junction clean-up), so Golden C's counts or texts move | Medium · medium | §9.3 items 2 and 3; D11's second hash keeps re-import honest when they only fill |
| R2 A run's line passes the loader's 16 MiB line limit on a large import | Low · medium | T3 refuses before the commit; about 200,000 records fit |
| R3 Re-import's "current" hash differs from the base for a reason no user caused (a format default written differently) | Medium · low | Both hashes come from one canonical writer; a test re-imports right after an import and finds every record unchanged (IC-12) |
| R4 The fuzz crate's lock needs new entries for the importers' dependencies | Low · low | They are already in the fuzz graph through `ew-api` (`command_sequence`'s closure); the lock is regenerated in the fuzz workspace only |
| R5 The `Exporter` move reaches beyond the table writers | Low · medium | D14 limits it to the CSV and XLSX writers; PDF and SVG are FP-7 |
| R6 Size | Medium · medium | §11's de-scope rule |

## 11. Size, routing and the hand-back point

**Size.** About 3.1 pw: step 1 0.45, step 2 0.25, step 3 0.35, step 4 0.7, step 5 0.45, step 6 0.6, step 7 0.3. The
PERT of 3.1 pw (O 2 · L 3 · P 4.5) stays. **De-scope rule:** when the lane is projected more than 20 % over, these leave
as follow-ups without a further ruling, in this order: the window's re-import card (the CLI's `--into` stays), the
Messages scheme choice (the CLI's `ew check --scheme migration` stays), the XLSX writer's trait implementation (the CSV
one stays).

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `high`** (a typed format kind, a new transaction
path, a golden expected file, a permissive crate's contract). It depends on WP-S3.05 and WP-S1.15's built parts, both on
the working branch.

**Hand-back point** (ID-1282). The lane hands back at the end of **step 5**, the migration report committed with its
`Golden:` trailer if the CSV moved, when it then stands above about 450k tokens; a lane that stands there before it
begins step 5 hands back at the end of step 4 instead, and one that nears 650k anywhere commits and hands back. Its
hand-off note then holds: the steps done with their commits; the `Limits` members and the constants removed; the host
paths moved onto the front door and any left (with why); the `ImportRun` members as written and the schema diff; the
internal command's name and how the engine runs it without the prevent hook; the catalogue rows added; Golden C's
counts as run and who ran them; the open items of §7 still red; the test filters (`-E 'test(/import/)'`, the
`golden_c` module with `EW_PRIVATE_CORPUS`, `ew`'s `cli` binary, `ew-viewmodel`'s import tests, `ew-ui`'s snapshots);
and every choice of §9.3's free list already made.

## 12. Follow-ups, proposed

The design pass opens the rows. FT-143 (the path through `ApplyImport`) closes with step 5.
- **FP-1** The proposal review queue inside the import preview (doc 14 §3.1 step 7), with group acceptance.
- **FP-2** Acknowledged import findings: the run's `acknowledged` member and an acknowledge command (doc 14 §2.5).
- **FP-3** Importing a source into an existing project as a new lineage; import as a copy (FR-IMP-045, 1.0).
- **FP-4** `ImportBatch.assets` with the first importer that emits an image (FT-134).
- **FP-5** The QET mirror corpus fetched at a pinned commit in CI (doc 14 §12; SC-4 with WP-S3.10).
- **FP-6** Fluent keys and arguments for import findings (doc 14 §2.4's `FluentKey`).
- **FP-7** The PDF, SVG and report exports through the `Exporter` trait.
- **FP-8** A merge rule for `ImportLineage.last_run` when two branches import into one lineage.

## 13. Open questions and the documents to update

**Open questions for the orchestrator.**
- **O1** Which imports persist a run. Options: (1) batch imports into a lineage only; the golden bootstrap, the parts
  table and the round trip write none, and Golden A stays byte-equal (D13); (2) every import writes one, Golden A gains
  `imports/` and `last_run` (a golden change, and a UUIDv7 file in the format's reference fixture). Recommendation: (1).
- **O2** Golden C's corpus-gated tests need `EW_PRIVATE_CORPUS`. Options: (1) the brief gives the lane the variable; the
  tests print counts and sanitised strings only (ID-299 allows derived counts); (2) the orchestrator runs them at the
  merge point and a fix round re-blesses the CSV. Recommendation: (1), so the step-5 commit carries the counts it
  re-blesses.
- **O3** The `Exporter` trait (ID-1016). Options: (1) as D14, with a new permissive `RowTable` and the CSV and XLSX
  writers implementing the trait in `ew-reports`; (2) the trait alone, with no implementation; (3) a follow-up.
  Recommendation: (1); a trait no exporter uses is not proven. Moving `ew-reports`' `Table` into the permissive crate
  is not proposed: it would relicense GPL code, a licence-boundary question.
- **O4** The size: the steps add up to the PERT of 3.1 pw. Recommendation: keep it, with §11's de-scope rule.
- **O5** ID-1322 raised the auto-compact window after ID-1282. Options: (1) keep §11's hand-back numbers (450k and
  650k) for this lane; (2) scale them as an amendment of ID-1282 says. Recommendation: (2) if the orchestrator amends
  ID-1282, else (1).

**Documents to update** (the design pass after the lane): doc 14 §2.1 (`ImportBatch` as built, no `provenance` or
`assets`; `Limits`' new members; the front door), §2.3 and §3.4 (the two hashes of D11), §2.4 (`ImportRun` as built;
`acknowledged` is FP-2), §2.5 (EW-IMP-010 and -011; the folding list; class 160 registered; "As built (cycle 12)"
replaced), §3.1 (no review queue at the MVP; the job), §4.16 (the spike route gone), §11 (the target names of D15),
§16's as-built note; doc 03 `:521` (`ImportRun` members); doc 05 §2.1 (`imports/` as built); doc 04 §10.6 (the batch);
ADR-0021 (decision 1's `provenance` read as the records' `source_ref`; amendment 1's assets still pending) and ADR-0011
amendment 4 ("applied again" for `import_run`); doc 15's catalogue (class 160 rows); 02b FR-IMP-001 to -005, -040,
-041 and FR-EXP-001 ("as built" notes); `golden/J-E-2416/README.md`'s Golden C section (the pipeline, FT-143 closed);
doc 18 §3.12 (the importer targets).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S3.09, WP-S3.05, WP-S3.08, WP-S3.10, WP-S3.11, WP-S1.15, WP-S1.19,
  WP-S6.04; FT-127 to FT-143, FT-173 to FT-178.
- [Decisions](../decisions.md): ID-1210, ID-1281, ID-1282, ID-1320, ID-1321; [decision log](../06-decision-log.md): ID-206,
  ID-273 to ID-284, ID-299, ID-312, ID-375, ID-1016.
- [Interop](../../design/14-interop.md); [architecture](../../design/04-architecture.md) §10.6;
  [domain model](../../design/03-domain-model.md) §5; [project format](../../design/05-project-format-and-persistence.md)
  §2.1, §3.2a; [golden project](../../design/20-golden-project.md) §20, AC-25;
  [extended requirements](../../design/02b-requirements-extended.md) §7 (IMP), FR-EXP-001;
  [non-functional requirements](../../design/02c-nonfunctional-requirements.md) NFR-SEC-02, NFR-SEC-18, NFR-REL-12.
- [ADR-0004](../../adr/0004-identity-uuidv7-uuidv5.md), [ADR-0011](../../adr/0011-format-versioning-and-migration.md),
  [ADR-0021](../../adr/0021-importers-emit-format-level-records.md).
- The specification of [WP-S3.08](WP-S3.08-table-exports-and-excel-round-trip.md).
