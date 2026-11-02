# WP-S3.08: table exports and the Excel round trip (closed specification)

**Status:** specified in cycle 42 (documents only), an M2 package, in three parts: part 1 the table writers (XLSX,
the CSV presets, the formula guard) behind every row export (1.0 pw, D2 to D7), part 2 the round trip (navigator
tables, `ew import table`, the I/O list, 1.0 pw, D8 to D13), part 3 the window (export and import from a navigator's
table, the export as a job, 0.5 pw, D14); 0.3 pw reserve; part 1 first, part 2 after it, part 3 after part 2 ·
**Date:** 2026-10-31 · **Work package:**
[WP-S3.08](../02-work-packages.md#wp-s308-table-exports-and-excel-round-trip) · **Decision log:** ID-1016, ID-1145 (part 1's rulings); part 1 as built ID-1157 · **Part of:** ElektraWatt plan, specifications

**Purpose.** WP-S3.08's CSV half was built ahead of it: every row export writes the rows of a report block, as its
folios print them, in the golden CSV dialect (WP-S3.03, cycle 14; WP-S1.22 part 2, cycle 40), and an own bounded XLSX
reader exists for the parts import (ID-827). What is left is the XLSX writer that WP-S3.06's D3 hands to this package,
the CSV presets and the formula-injection guard, the round trip with its hidden `id` column and `_ew` sheets, the
I/O-list round trip of FR-PLC-038, and the window's export and import of a table. This document traces the built
rows with their evidence, closes the rest to items, decisions and acceptance, and settles the boundaries with WP-S3.06,
WP-S1.22, WP-S3.09, WP-S2.06, WP-S2.09, WP-S1.21 and WP-S3.03. It is written against the code at `453d252d` (§2.1).
Where this document and a design document differ, this document is the narrower and wins for the lanes; the design
documents stay the target and are updated at the design pass after each part (§13). A choice a lane meets that this
document does not settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the table writers](#5-part-1-the-table-writers)
6. [Part 2: the round trip](#6-part-2-the-round-trip)
7. [Part 3: the window](#7-part-3-the-window)
8. [Neighbours and who owns what](#8-neighbours-and-who-owns-what)
9. [Files owned and what a lane may not decide](#9-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, order and routing](#11-size-order-and-routing)
12. [Open questions for the orchestrator](#12-open-questions-for-the-orchestrator)
13. [Follow-ups and the documents to update](#13-follow-ups-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package every row export of `ew export` and `ew sql` also writes XLSX
(`--format xlsx --out <file.xlsx>`) and the CSV presets `excel-nl`, `excel-en` and `rfc4180` (`--preset`), the Excel
presets with the apostrophe guard of NFR-SEC-17; the golden dialect stays the default, so no existing golden byte
changes. Every XLSX carries a hidden `id` column and a hidden `_ew` sheet; an editable table also carries `_ew_base`.
`ew export table strips --strip -X1031 --format xlsx` writes the -X1031 table; editing five terminal function texts in
it and running `ew import table <file> --apply` changes exactly five fields in one undo step, and a row whose terminal
was deleted meanwhile is reported (FR-EXP-004). The golden I/O list exported as `excel-nl` CSV, with three comments
edited, re-imports as exactly three changed fields in one transaction (FR-PLC-038). In the window, a navigator in table
mode exports its table and imports a round-trip file into the Preview tab.

**Non-goals** (each with its package or follow-up):
- No `Exporter` trait or `ExportView` in `ew-interop-api`: WP-S3.09's (the crate's own module note hands them over;
  D13). No plugin exporters (1.x).
- No import batch, lineage or import run for a round trip; no creation of rows from unknown IDs (D12, FP-1).
- No per-cell conflict choice in the preview: one switch takes the file's value for every conflict (D11, FP-2).
- No ECLASS, ETIM or type designation columns and no column-map profile for the purchasing BOM (D15; FR-EXP-005 is
  WP-S3.03's row and its acceptance passes since cycle 12).
- No encoding other than UTF-8 (FP-3); no TIA or PLCopen tables (WP-S3.16); no machine profiles (doc 13, 1.0).
- No wrapped rows (FT-61's other half stays with WP-S3.03); no change to a drawn page; no new report kind or block in
  Golden A.

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Reports](../../design/09-reports-forms-output.md) §1.3, §2.3, §4.3, §4.6, OQ 8 | The writers' crate, `RowSet`, determinism, XLSX and CSV, the guard ruling |
| [Interop](../../design/14-interop.md) §8.1, §8.2 | The round-trip sheets and the merge, narrowed by D9 to D12 |
| [UX](../../design/16-ux-ui.md) §7.5 to §7.7; [design language](../../design/16a-design-language.md) | The Preview tab and the round trip's UI; every window text and layout (part 3) |
| [Checks](../../design/15-checks-revisions-collaboration.md) §10 (EW-IMP-301 to -305) | The round trip's findings |
| [Extensibility](../../design/17-extensibility.md) §3.1, §3.3 | CLI conventions, the export rows as built |
| [Golden project](../../design/20-golden-project.md) §6, §8, §10, §12, §14.1 | The rows the goldens hold |
| ADR-0018 item 5, ADR-0015 | One row source, many sinks (amended by D3) |
| Specifications | [WP-S3.06](WP-S3.06-mvp-report-pack.md) D3, D4; [WP-S1.22](WP-S1.22-cli-surface-and-api-guard.md) D2, D4, D5, D13, D14; [WP-S2.06](WP-S2.06-navigators-and-table-mode.md) D3, D10 to D13; [WP-S2.09](WP-S2.09-messages-bulk-previews-history-jobs-and-recovery-ui.md) D11; [WP-S1.21](WP-S1.21-plc-and-bus-model.md) PX-1 |
| Follow-ups | FT-61, FT-218, FT-241, FT-242 |

### 2.1 What exists at `453d252d` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| The golden CSV dialect: `csv_field`, `row_texts`, `rows_csv(q, set)` (LF, no BOM, header `row,<labels>`, `data` or `heading`, neutral language) | `crates/ew-reports/src/csv.rs` | Kept as the default dialect `ew` (D6) |
| `RowSet { columns: Vec<ColumnDef>, rows: Vec<Row> }`, `Row { id: RowId, kind, cells, links, sources }`, `CellType` (Text, Ident, Number, Tag, Folio, Folios, CellRef, Qty, Colour, Address, Date, Bool, Range), `RowId` with a text form and `parse` | `crates/ew-reports/src/rowset.rs:170-441` | The table model (D4) and the `id` column |
| `RowExport::{Cables, Bom(mode), Wires, Terminals{strip}, Report}`, `row_export(q, what, block) -> RowsCsv`, `find_block`, `RowsError` | `crates/ew-api/src/output/rows.rs` | Part 1 adds the XLSX and preset outputs over the same row choice |
| `ew export cables\|bom\|wires\|terminals\|report` with `--block`, `--format csv` (report also `pdf`); `--format xlsx` exits 2 "`--format xlsx` is not built yet (…; XLSX waits for the writer of FT-61)"; `ew sql --format csv\|json`, `xlsx` exits 2 | `bin/ew/src/cli.rs:35-41`; the refusals in `bin/ew/src/cli/tree.rs:1361-1405` (`sql_format`, `row_format`, `report_format`, as of `b7f06639`; ID-1145) | Part 1 replaces both refusals |
| `ew export report --kind` | Not at the base; **arrives with WP-S3.06 part 1** (lane G42, its D4) | Part 1 gives it `--format xlsx` if it is in at the lane's base, else the WP-S3.06 lane adds the one match arm (§8) |
| The CLI table `bin/ew/src/catalogue.rs` (D4 of WP-S1.22) and the result schemas `bin/ew/schemas/` | WP-S1.22 part 1 | Every new flag and subcommand enters both |
| `NavKind` (14 navigators), `Session::navigator_table(kind) -> NavTable { kind, rev, columns: Vec<NavColumn>, nodes: Vec<NavNode>, index }`; `NavColumn { key, label, derived, key_column, route: Option<(kind, path)>, read_only, strip_cell }`; `NavNode { key, role, records, kinds, define_at, cells, table_row, … }` | `crates/ew-api/src/output/navigators.rs:50-330, 696` | Navigator table exports and the round trip's routes (D8, D9) |
| `EDITABLE_COLUMNS` (29 routes, e.g. `(Strips, "label", "device", "function_text")`, `(Plc, "comment", "channel", "comment")`), `DERIVED_COLUMNS` (13), `STRIP_CELLS` (3) | `crates/ew-api/src/output/navigators.rs:426-500` | D9 |
| Table mode's pure rule: `row_targets`, `route_type`, `route_member`, `compose_leaf`, `mark_conflicts`, `compile(gesture, base) -> Option<Compiled>` (to `fields.set` or one `fields.set_many`), `cell_reason` | `crates/ew-viewmodel/src/navigators/table.rs:293-883` | Moved down by D10 |
| `fields.set_many` (`SetManyFields { edits, copies }`, one transaction, at most `SET_MANY_MAX_EDITS` = 10,000 entries) | `crates/ew-commands/src/commands/fields.rs:620-700` | The round trip's one command (D11) |
| `Session::parse_field_input` | `crates/ew-api/src/output/properties.rs:2047` | Each imported cell parsed as the grid parses it |
| `io_list` (WP-S1.21 part 5): 20 FR-PLC-023 columns; row ID `entity(channel)` + `name("channel")`; `params.include_deactivated` | `crates/ew-reports/src/kinds/io_list.rs:1-60, 301` | FR-PLC-038 (D9) |
| The XLSX reader (`read_xlsx(bytes, sheet, limits) -> Sheet`, the first or the named sheet, cells as displayed text, a formula's cached value, every entry inflated under a cap) and the CSV reader (`read_csv`), both `pub(crate)` | `crates/ew-import-catalog/src/xlsx.rs:452`, `table.rs:170, 257` | Made public by D12 |
| `miniz_oxide =0.8.9` (`with-alloc`) and `quick-xml =0.41.0` as workspace dependencies, pinned so that no ZIP crate turns on flate2's `zlib-rs` (ID-827) | `Cargo.toml:27-32` | The writer's deflate (D3) |
| `ci/dep-rules.toml`: `"ew-reports" = ["ew-derive", "ew-sqlview", "ew-text"]`; `"ew-api"` reaches `ew-reports`, `ew-import-catalog`; importers depend on `ew-interop-api` only | `ci/dep-rules.toml:59-100` | One new workspace crate `ew-zip` and two edges, `ew-store` → `ew-zip` and `ew-reports` → `ew-zip` (D2, ID-1145); no other new edge (D12) |
| The export job on a snapshot: `Session::export_job`, `ExportJob::run(ctl)`, `ew-jobs` | `crates/ew-api/src/output/job.rs` | Part 3's table export job (D13) |
| The Preview tab with `CardOrigin`s (WP-S2.09 part 3) | `crates/ew-viewmodel/src/` | Part 3's import preview |
| Golden A: `expected/csv/` holds `bom_assembly`, `bom_parts`, `cables`, `potentials`, `terminals_1X1`, `terminals_X1031`, `terminals_X1032`, `wires` | `golden/J-E-2416/expected/csv/` | Unchanged; new files beside them (D16) |

### 2.2 Gaps found (why the parts are shaped as they are)

| Gap at `453d252d` | Consequence |
|---|---|
| No XLSX writer; doc 09 and ADR-0018 name rust_xlsxwriter, whose ZIP dependency is the hazard of ID-827 | D3: an own writer |
| Navigator tables are not `RowSet`s (Strips adds `strip`, `pos`, `bridge_part`; PLC has its own columns) | D4: one neutral table model for both |
| The edit routes and the compile rule live in the view model, which the CLI does not link | D10: the pure rule moves to `ew-api` |
| The XLSX reader is private to the parts import | D12: a public sheet API, no second reader |
| A CSV carries no metadata, so it cannot name its table | D11: `--table` on a CSV import |
| No round-trip findings in code (EW-IMP-301 to -305 exist in doc 15 only) | D11 |

## 3. Requirements traced

| Requirement | State and what this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-EXP-001 | **Partly here.** The window's table export runs as a job on a snapshot and never blocks editing; every XLSX names the revision it was made at (`_ew`), every `--json` summary too. The `Exporter` trait over an `ExportView` is WP-S3.09's (D13). "Release-mode exports refuse stale Class II results" is moot at the MVP: the only Class II table export is `ew sql`, whose projection is built at open, and the window has no SQL console before 1.0 (as-built note, §13) | 1, 3 | TX-7, TW-3 |
| FR-EXP-002 | CSV of every report row source **built** (WP-S3.03 cycle 14, WP-S1.22 part 2; `rows.rs`); here XLSX and the presets of every row source and the navigator tables (D7, D8) | 1, 2 | TX-1, TX-2, RT-1 |
| FR-EXP-003 | The presets `excel-nl`, `excel-en`, `rfc4180` with `--bom` and `--line-end` (D6); encodings other than UTF-8 are FP-3 (as-built note) | 1 | TX-3 |
| FR-EXP-004 | The round trip (D5, D9 to D12) | 2 | RT-2 to RT-6 |
| FR-EXP-006 | CSV **built** (the golden files are compared byte for byte on every platform CI runs); here the XLSX bytes by D3 and the preset files by their goldens; the platform comparison of digests is FT-234's (WP-S6.06) | 1 | TX-5 |
| FR-REP-030 | XLSX and CSV from the same `RowSet` as the pages (D4): the golden wire list's XLSX has the rows of folio 907 | 1 | TX-1 |
| FR-PLC-038 | The I/O list round trip (D9, D11): `io_list`'s rows, an `id` column, `excel-nl` (UTF-8 BOM, `;`), matching by ID or by (station, slot, channel) | 2 | RT-5 |
| NFR-SEC-17 | XLSX text as string cells, never a formula; the Excel presets' apostrophe guard; `rfc4180` and `ew` verbatim (D4, D6) | 1 | TX-4 |

**AC-07** passes since cycle 12 (WP-S3.03); this package adds no AC-07 entry. FR-EXT-003's and FR-EXT-027's XLSX clauses
(WP-S1.22's rows) are met by part 1 (TX-1, TX-2).

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under this specification's
decision-log row.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Built rows are traced as built and sized at nil.** The remaining size is parts 1 to 3 (2.5 pw) and a 0.3 pw reserve; proposed PERT O 2.0 · L 2.7 · P 4.2 · **PERT 2.8** (σ 0.4), against 2.7 in `work-packages.csv`: the CSV half is built, the writer is own code (D3) and the window part is new scope the row did not price | Counting the CSV exports here again would double-count WP-S3.03's and WP-S1.22's work |
| D2 | **The writers live in `ew-reports`** (ruling 3 of the brief; doc 09 §1.3; FT-61): `table.rs` (the model of D4), `xlsx.rs` (D3), `csv.rs` (D6). `ew-reports` gains the workspace crate `ew-zip` (ID-1145: `ew-store`'s deterministic ZIP moved, unchanged, into the new GPL library crate `ew-zip`, re-exported as `ew_store::zip`, `ew-store` dropping its own `miniz_oxide`); `ew-api` builds navigator tables into the same model. The licence boundary is untouched: `ew-reports` and `ew-zip` are GPL, `miniz_oxide` is MIT OR Zlib OR Apache-2.0, already in `deny.toml`'s allowed graph | One writer for report rows and navigator tables; the alternative crate `ew-export` holds page outputs and has no row types |
| D3 | **An own XLSX writer, no rust_xlsxwriter and no external ZIP crate.** ADR-0018 item 5 and doc 09 name rust_xlsxwriter; it depends on `zip`, and `zip` 9 turned on flate2's `zlib-rs` through feature unification and changed Golden A's PDF bytes (ID-827). The writer writes a ZIP of **stored-order fixed entries**: `[Content_Types].xml`, `_rels/.rels`, `docProps/core.xml`, `docProps/app.xml`, `xl/workbook.xml`, `xl/_rels/workbook.xml.rels`, `xl/styles.xml`, then `xl/worksheets/sheet<n>.xml` in sheet order; each deflated at level 6 by `ew-zip` (the workspace's own ZIP, over `miniz_oxide`; ID-1145); DOS date and time 1980-01-01 00:00 on every entry; no extra fields, no data descriptor, no ZIP64 (more than 65,535 entries or 4 GiB is refused); an own CRC-32 (IEEE, table-driven, tested on "123456789" = `cbf43926`). `docProps/core.xml` dates are the export date of doc 09 §4.3 rule 1 (the `SOURCE_DATE_EPOCH`, else the model's edited date, as the PDF); `app.xml` names "ElektraWatt" and no version. Text is written as inline strings (`t="inlineStr"`), no shared-strings part. **ADR-0018 is amended** at the design pass after part 1 (text in §13) | Byte identity is ours by construction and no external package enters the graph (`ew-zip` is the workspace's own, ID-1145); the reader of ID-827 already knows the format. The cost: no rich formatting (one bold header style and one date style), and a `miniz_oxide` bump re-blesses the XLSX goldens |
| D4 | **One table model and the sheet content.** `ew_reports::table::Table { name, key, columns: Vec<TableColumn { key, label, cell: CellType, editable: bool }>, rows: Vec<TableRow { id: Option<String>, heading: bool, cells: Vec<TableCell> }> }` with `TableCell::{Text(String), Number(String), Date(String)}` (numbers and dates in their neutral printed form, `.` decimal). `Table::from_rowset(q, set, name, key)` renders each cell as the CSV does (`row_texts`) and types Number and Qty cells as `Number`, Date cells as `Date`, every other cell as `Text`; a heading row has `heading` set and no ID. **The sheet:** column A is the `id` column (header `id`, the row's `RowId` text, hidden, width 0); the table's columns follow with their neutral labels; row 1 bold and frozen; an autofilter over the header; widths of `max(header, longest cell)` characters clamped to 6 to 60 (a rule, not a fit: no fixture tunes it; the lane may change the bounds and reports it); `Number` cells as numeric cells, `Date` cells as date serials with the style `yyyy-mm-dd`, every other cell an inline string, so "-101F1", "+J-E-2416" and "=HYPERLINK(…)" stay text (NFR-SEC-17); no `<f>` element is ever written. The sheet name is the table's name with `[]:*?/\` replaced by `_` and cut to 31 characters; a second sheet of the same name gets " (2)" | Report rows and navigator tables share one writer; the `id` column first stays attached to its row when Excel sorts |
| D5 | **The round-trip sheets** (doc 14 §8.1). Every XLSX carries a hidden sheet `_ew` of key/value rows: `format` `ew-table/1`; `project` (the project ID); `table` (the key of D8); `rev` (the session revision of the snapshot); `generation` (the store generation, empty for a model without a store); `locale` `neutral`; `ew` (the program version); then one row per column: `column`, key, label, type (`CellType` name), editable (`true`/`false`). A table with an editable column also carries a hidden sheet `_ew_base`: the `id` column and the editable columns as exported, and a last row `sha256` with the SHA-256 (hex) over the sheet's cell texts in row order, tab-separated, LF line ends, so an edited base is detected. Report tables other than `io_list` have no editable column and no `_ew_base` | The revision and generation are informational; the three-way merge reads `_ew_base`, never a revision number, so an edit in a later session is still detected |
| D6 | **The CSV dialects.** `--preset ew\|excel-nl\|excel-en\|rfc4180`, **default `ew`**, the golden dialect of `rows_csv` byte for byte (no existing file changes). `excel-nl`: `;`, decimal comma in `Number` cells, UTF-8 BOM, CRLF. `excel-en`: `,`, decimal point, UTF-8 BOM, CRLF. `rfc4180`: `,`, CRLF, no BOM, values verbatim. The three presets write the `id` column first (visible: a CSV has no hidden column) and no `row` column; heading rows have an empty `id`. Quoting as RFC 4180 with the preset's delimiter. **The guard** (NFR-SEC-17, doc 09 OQ 8): in `excel-nl` and `excel-en` a `Text` cell whose first character is `=`, `+`, `-`, `@`, tab or CR gets a leading apostrophe; `Number` cells are never guarded ("-1,5" stays a number); `ew` and `rfc4180` never alter a value. `--bom on\|off` and `--line-end crlf\|lf` override the preset (FR-EXP-003's options); the encoding is UTF-8 only (FP-3). Refusals, exact: "`--preset` applies to CSV: `--format xlsx` takes none"; "unknown preset `<name>` (ew, excel-nl, excel-en, rfc4180)" | The acceptance files stay in the golden dialect (FT-242); the guard changes tags such as "-3X1" to "'-3X1" in the Excel presets only, which the round trip strips again (D11) |
| D7 | **The CLI surface** (WP-S1.22's conventions; each new flag and subcommand enters `catalogue.rs`, its result schema and, once WP-S1.22 part 5 is in, the CLI snapshot; each gets one line in `docs/release-notes/unreleased.md`). `--format xlsx` on `export cables`, `bom`, `wires`, `terminals`, `report --block` and `report --kind` (when WP-S3.06 part 1 is in), and on `ew sql` (FT-218): it needs `--out` ("`--format xlsx` writes a file: give `--out <file.xlsx>`"); `export terminals` without `--strip` writes one sheet per strip (named by its printed tag) in the order of the CSV. `--preset`, `--bom`, `--line-end` on every CSV row export and `ew sql --format csv`. **New:** `ew export table <navigator> [<project>] [--strip <tag>] [--format xlsx\|csv] [--preset …] [--out <file>] [--json]` (part 2) and `ew import table <file> [<project>] [--table <key>] [--preset excel-nl\|excel-en\|rfc4180] [--take-file] [--apply] [--json]` (part 2); results `ew.export-table-result/1` and `ew.import-table-result/1`; the existing export results gain the optional `format` and `preset` members (additive). `ew import io-list` of doc 17 is not added: `ew import table` covers the I/O list. Exit 0 on success, 2 on every refusal, as the other exports | One import subcommand for every table; the XLSX of every export through one writer |
| D8 | **What a table is and its key.** Report tables: a block's rows (`block:<uuid>`), a kind's whole-project rows (`kind:<name>`, the export by kind of WP-S3.06 D4), the BOM export (`kind:parts_summary/<mode>`), `ew sql` (`sql`, never re-importable). Navigator tables: `nav:<word>` for the table navigators (pages, devices, connections, strips, cables, plc, potentials, ips, structure, parts, blocks; not macros, libraries or messages), `nav:strips/<strip uuid>` with `--strip`. The CLI exports a navigator table unfiltered, its table rows in node order and every column; the window exports the rows and columns on show (D14). Refusals, exact: "unknown navigator `<word>` (the table navigators are: <words>)"; "`--strip` applies to `export table strips`" | The navigator rows are already the kinds' rows where a kind lists them (WP-S2.06 D3), so the CLI and the window export one derivation |
| D9 | **What is editable** (one closed rule). A cell is editable exactly when its column has a `fields.set` route of `EDITABLE_COLUMNS` and its row has a record of the route's kind (`row_targets`), as table mode decides. Strip-editor cells (`group`, `bridge_part`, an accessory's `part`) and a Potentials row's defining cells (`define_at`) are read-only in a round trip ("edit in the window"). **`io_list`** is the one editable report table: its columns `designation`, `address`, `symbol`, `state`, `target` and `comment` take the PLC navigator's routes (`channel` kind), the target being the row's channel (its row ID's entity). Every other report table exports with every column read-only | One rule for table mode and the round trip; FR-PLC-038 names FR-PLC-023's columns, which `io_list` has and the PLC navigator does not |
| D10 | **Table mode's pure rule moves down.** `row_targets`, `route_type`, `route_member`, `route_identifier`, `is_scalar`, `compose_leaf`, `mark_conflicts`, `compile` and the types they take move from `crates/ew-viewmodel/src/navigators/table.rs` to `crates/ew-api/src/output/table_edit.rs`; the view model re-exports them under their old paths, so no caller changes and its tests run unchanged. No behaviour change | The CLI cannot link the view model; one compile rule means the grid and the round trip produce the same command for the same cells |
| D11 | **The merge** (doc 14 §8.2, narrowed). **Read:** an XLSX through D12's reader; `_ew` names the table, else the import needs `--table`; a CSV always needs `--table <key>` ("a CSV names no table: give `--table <key>`"), reads the `id` column, and strips exactly one leading apostrophe before `=`, `+`, `-`, `@`, tab or CR unless `--preset rfc4180`; `Number` columns parse `excel-nl`'s decimal comma. **Match** by `id`; for `kind:io_list` a row without an `id` matches by (station, slot, channel) (FR-PLC-038); heading rows and rows without an ID and a match are skipped; an unknown ID is EW-IMP-303 (no row is created, FP-1); a table row missing from the file is listed, never deleted. **Per cell:** a read-only column edited is EW-IMP-301 with the column's read-only reason; a number in a text column is converted canonically and EW-IMP-305 when lossy ("1.10" → "1.1"); with `_ew_base` intact, three-way: changed only in the file → applied; only in EW since the export → kept, EW-IMP-302; in both to different values → EW-IMP-304, EW's value kept unless `--take-file` (window: "Take the file's values"); without `_ew_base` (a CSV, an edited base) two-way against the current value. Each applied cell is parsed by `Session::parse_field_input` for its route; an unparsable cell is refused with the parser's text and not applied. **Apply:** D10's `compile` turns the applied cells into one `fields.set_many` (one transaction, one undo step), committed only with `--apply` or Accept; more than 10,000 entries is refused ("<n> changed cells are more than the 10,000 one import applies; split the file"). The preview (and `--json`) lists per changed cell the row label, column, base, EW and file values and the outcome, then the findings and the missing rows | The doc's three-way rule, with the switches the MVP needs; `fields.set_many` already is one undo step, so no `table.apply_changes` command is added (doc 14 and doc 16 name one; amended at the design pass) |
| D12 | **One reader: the parts import's.** `ew-import-catalog` makes its readers public as `ew_import_catalog::sheet`: `read_sheet(bytes, format: SourceFormat, sheet: Option<&str>, limits: &Limits) -> Result<SheetTable, ImportError>` with `SheetTable { headers: Vec<String>, rows: Vec<Vec<String>> }` and `sheet_names(bytes, limits) -> Result<Vec<String>, ImportError>` (every sheet, hidden ones included); an error cell reads empty; nothing else of the reader changes. Both are built in part 1, because TX-1 and D16's dumps read the workbook through them (ID-1145). The round-trip diff lives in `ew-api` (`output/table_io.rs`), a pure function of the file's bytes and the current table. Arch §3's `ew-import-tables` row ("the round-trip table reader … later calamine") is amended at the design pass | No second ZIP or XLSX reader and no calamine (ID-827); `ew-import-tables` cannot depend on another importer (ARCH §3 rule 4) |
| D13 | **Boundaries with WP-S3.09 and the `Exporter` trait.** A round-trip import is not an `ImportBatch` import: it creates no record, mints no ID, records no lineage or import run; it follows the importer rules (pure over the bytes it is handed, `ew-interop-api`'s `Limits`, no file system or clock) and reports EW-IMP-301 to -305 of doc 15 in its own preview. The `Exporter` trait and `ExportView` are WP-S3.09's (`crates/ew-interop-api/src/lib.rs:39`); FR-EXP-001's trait clause waits for it (§12, O2) | Round-trip rows are edits of existing records; an import batch would add a lineage for nothing |
| D14 | **The window** (part 3; every text and layout by [doc 16a](../../design/16a-design-language.md)). Two actions in a navigator's table mode, `navigator.export_table` (`path`, `format` xlsx\|csv, `preset`) and `navigator.import_table` (`path`, `take_file`), in the table's menu and the Output menu: the export writes the rows and columns on show, as a job on the snapshot (`ew-jobs`, progress, cancel, `<file>.tmp` then rename), XLSX by default; the import reads the file, opens the round trip's card in the Preview tab (`CardOrigin::TableImport`) with the counts (changed, kept, conflicts, refused, unknown, missing), the per-cell list and the switch "Take the file's values", and Accept commits D11's command. Strings in `strings.rs` | The Preview tab and the job runner exist (WP-S2.09); a table-only dialog would be a second preview model |
| D15 | **The purchasing BOM at the MVP** is `ew export bom [--mode]` in XLSX or an Excel preset: quantity, unit, manufacturer, part number, description, tags and folios. FR-EXP-005's type, ECLASS, ETIM and column-map profile are not this package's (its row is WP-S3.03's, its acceptance passes); the classification data does not exist in the schema and the column-map engine is doc 13's (FP-4) | No column can be filled without the data; the acceptance of FR-EXP-005 is already met |
| D16 | **Golden examples add files and change no existing byte.** New: `golden/J-E-2416/expected/xlsx/sha256.txt` (one line `<sha256>  <file>` per XLSX golden) and one text dump per XLSX golden, `expected/xlsx/<name>.cells.txt` (each sheet's name, then its rows as tab-separated cell texts read back through D12's reader); `expected/csv/presets/<name>.<preset>.csv`. Part 1: `bom_parts.xlsx`, `wires.xlsx`, `block_908.xlsx` (dumps and digests), `presets/bom_parts.excel-nl.csv`, `bom_parts.excel-en.csv`, `bom_parts.rfc4180.csv`. Part 2: `strips_X1031.xlsx`, `io_list.xlsx`, `presets/io_list.excel-nl.csv`. Each is blessed by a test with `EW_BLESS=1` and compared otherwise. **Not a golden change** | Digests plus readable dumps keep binaries out of review and still pin every byte |
| D17 | **Every part closed;** each part is re-read at dispatch against the delivered earlier parts and the neighbours' state (WP-S3.06 part 1, WP-S1.22 part 5) without reopening D1 to D16 | — |

One decision changes an architecture edge: D2 adds the crate `ew-zip` with the edges `ew-store` → `ew-zip` and
`ew-reports` → `ew-zip` (ID-1145; doc 04 §3's graph). D3 amends ADR-0018 item 5; D12 amends arch §3's `ew-import-tables`
and `ew-reports` rows (§13).

## 5. Part 1: the table writers

**Scope.** D2 to D7 (export side), D15, D16's part-1 files; FT-61's XLSX half, FT-218, FT-241's XLSX half, FT-242's
preset half. **1.0 pw.**

**Build list.**
1. `crates/ew-reports/src/table.rs`: D4's model and `Table::from_rowset`; `crates/ew-reports/src/xlsx.rs`: D3's ZIP
   and D4's workbook, `xlsx(tables: &[Table], meta: &WorkbookMeta) -> Vec<u8>` with `WorkbookMeta { date, project, rev,
   generation, version }` and D5's `_ew` (and `_ew_base` for editable columns, used by part 2);
   `crates/ew-reports/src/csv.rs`: `Preset`, `CsvOptions { preset, bom, line_end }`, `table_csv(&Table, &CsvOptions)`;
   `rows_csv` unchanged. `crates/ew-reports/Cargo.toml`: the `ew-zip` path dependency (ID-1145). `crates/ew-zip`:
   `ew-store`'s ZIP, moved. `crates/ew-import-catalog`: D12's `sheet::{read_sheet, sheet_names}`.
2. `crates/ew-api/src/output/rows.rs`: `RowsOutput::{Csv(String), Xlsx(Vec<u8>)}` over the same row choice;
   `RowExport::Terminals` as one sheet per strip; `ew sql`'s result set into a `Table` (`key` `sql`).
3. `bin/ew/src/cli.rs`, `catalogue.rs`, `schemas/`: D7's flags and refusals; the XLSX file written as `<file>.tmp`,
   synced and renamed (as the PDF).
4. Golden and tests: D16's part-1 files; `bin/ew/tests/` cases running each command line.

**Acceptance.**
- **TX-1** `ew export wires golden/J-E-2416.ew --format xlsx --out w.xlsx` writes a workbook whose first sheet, read
  back through D12's reader, has the header `id` + the labels of `wires.csv` and its 51 rows (50 data rows, one heading
  row) in folio 907's order with the same texts as `wires.csv` (FR-REP-030); column A is hidden; `_ew` names `block:<uuid of 907's block>`
  and every column `editable` `false`; no `_ew_base`.
- **TX-2** `ew export bom --format xlsx` gives the rows of `bom_parts.csv` with Qty as numeric cells (row 2's "1" a
  number); `ew export report --block 908 --format xlsx` 56 rows (FR-EXT-027); `ew export terminals --format xlsx` three
  sheets `1X1`, `-X1031`, `-X1032`; `ew sql "<select>" --format xlsx --out q.xlsx` the query's rows (FT-218); each
  `--format xlsx` without `--out` is refused with D7's text.
- **TX-3** `--preset excel-nl` on `ew export bom` equals `presets/bom_parts.excel-nl.csv`: a BOM, `;`, CRLF, `id`
  first, the quantities 1.71 and 3.17 (rows 22 to 25) printed "1,71" and "3,17";
  `excel-en` and `rfc4180` equal their files; `--bom off --line-end lf` on `excel-nl` drops the BOM and writes LF; the
  two refusals of D6, whole-text.
- **TX-4** (NFR-SEC-17) A fixture whose -101F1 function text is "=HYPERLINK(\"x\")": the XLSX holds it as an inline
  string and no sheet has an `<f>` element; `excel-nl` writes "'=HYPERLINK(""x"")" quoted as RFC 4180 requires;
  `rfc4180` writes it verbatim; a tag "-3X1" is "'-3X1" in `excel-en` and "-3X1" in `rfc4180`; a negative number cell
  is never prefixed.
- **TX-5** Determinism: each export twice gives the same bytes; the XLSX of a model loaded with its records shuffled
  (AC-14's seed) equals the unshuffled one; the CRC-32 of "123456789" is `cbf43926`; every ZIP entry has the date
  1980-01-01 00:00; with `SOURCE_DATE_EPOCH` set, `core.xml` carries that date.
- **TX-6** No existing byte of Golden A changes (PDF, PNG, SVG, text, rows, the eight CSV files, the fixture, the
  acceptance report); the files of D16's part-1 list are new; `cargo xtask check-deps` passes with one new workspace
  crate and two edges (`ew-zip`, ID-1145); `cargo deny check` sees no new external package.
- **TX-7** Every `--json` export result names the format, the preset (CSV) and the revision; the new flags are in
  `catalogue.rs` and the schemas; `docs/release-notes/unreleased.md` has one line per new surface; targeted checks
  (`cargo nextest run -p ew-reports -p ew-api -p ew --all-features`, the matching clippy, `cargo fmt --all --check`).

*As built (cycle 46, C46; decision log ID-1157):* `ew-zip` holds `ew-store`'s ZIP unchanged, `ew-store` re-exporting it
as `ew_store::zip` and dropping `miniz_oxide`; `ew-reports` has `Table`, `xlsx()` returning `Result<_, XlsxError>`,
`Preset`, `CsvOptions`, `restyle` and `table_csv`; D12's reader is `ew_import_catalog::sheet::{read_sheet,
sheet_names}`. A quantity with a unit stays text; the presets write dates as ISO; a sheet is named after its kind; the
BOM's key is `kind:parts_summary/…` even with `--block`, and several strips share `kind:terminal_diagram`; the id
column is always written; the formula guard covers the header labels; the preset goldens are stored with LF and the
test asserts CRLF. Seven golden files are new and no golden byte moved; `api-guard` saw 32 additive changes. Left to
part 2: `_ew_base` (it needs SHA-256), the reader cutting `_ew`'s five-cell rows to the header's two cells (C46-p2#2),
numbers read back in their shortest form and dates as serials, and the review's C46-p2#1, #3, #4 and #10 (ID-1165).

## 6. Part 2: the round trip

**Scope.** D5 (`_ew_base`), D8 to D13, D16's part-2 files; FR-EXP-004, FR-PLC-038. **1.0 pw.**

**Build list.**
1. `crates/ew-api/src/output/table_edit.rs`: D10's move; `crates/ew-viewmodel/src/navigators/table.rs`: the
   re-exports only.
2. `crates/ew-import-catalog/src/lib.rs`, `table.rs`, `xlsx.rs`: D12's public `sheet` module (visibility and the two
   functions only).
3. `crates/ew-api/src/output/table_io.rs` (new): `Session::table_export(key) -> Table` for navigator tables (D8);
   `Session::table_import(bytes, &ImportTableOptions) -> Result<TableImport, TableImportError>` (D11's preview: cells,
   findings, missing rows, the compiled command) and `Session::apply_table_import(&TableImport)`; the `io_list` routes
   and the (station, slot, channel) match.
4. `bin/ew`: `ew export table`, `ew import table`, their catalogue entries, schemas and release-note lines.
5. Golden and tests: D16's part-2 files.

**Acceptance.**
- **RT-1** `ew export table strips golden/J-E-2416.ew --strip -X1031 --format xlsx --out x.xlsx` writes the 20
  terminal rows of -X1031 in the navigator's order with every column of the Terminal strips table; `_ew` names
  `nav:strips/<uuid>`, marks `terminal`, `label` and `state` editable and `group`, `bridge_part`, `potential` not;
  `_ew_base` holds the 20 IDs and the three editable columns with a valid SHA-256; the file's digest and dump equal
  `strips_X1031`'s.
- **RT-2** (FR-EXP-004) Five Label cells of x.xlsx changed (the test rewrites the sheet XML the way Excel saves it:
  a shared-strings table and `t="s"` cells, the hidden sheets kept) and `ew import table x.xlsx --apply` changes exactly
  five `function_text` fields in one transaction and one undo step; without `--apply` nothing changes and the preview
  lists the five cells; `ew apply` of the session's command log replays it.
- **RT-3** After the export, one -X1031 terminal deleted in EW and its row edited in the file: the row is reported
  (EW-IMP-303, naming its ID) and nothing else changes; a terminal whose Label was changed in EW and not in the file
  is kept with EW-IMP-302; changed in both, EW-IMP-304 keeps EW's value, and with `--take-file` the file's.
- **RT-4** A Potential (derived) cell edited gives EW-IMP-301 with "derived"; a Group cell "edit in the window"; a
  Terminal cell "1.10" written by Excel as the number 1.1 gives EW-IMP-305 and the value "1.1"; an edited `_ew_base`
  falls back to two-way with one notice; a workbook from another project is refused ("the file was exported from
  another project (<id>)"); a CSV without `--table` is refused with D11's text.
- **RT-5** (FR-PLC-038) `ew export report --kind io_list --preset excel-nl` (or the export of an `io_list` block in a
  fixture, if WP-S3.06 part 1 is not in) equals `presets/io_list.excel-nl.csv`: UTF-8 BOM, `;`, an `id` column, 28 rows
  (deactivated channels hidden, ID-950); three Comment cells changed in Dutch and `ew import table <csv> --table
  kind:io_list --apply` changes exactly three `comment` fields in one transaction; the same file with the `id` column
  removed matches by (station, slot, channel) with the same result; `io_list.xlsx` round-trips the same way.
- **RT-6** Twice-run determinism of both new exports; table mode's view-model tests pass unchanged after D10; the
  hostile-input bounds of the reader apply (a workbook over the loader limit is refused before inflation); targeted
  checks for `ew-import-catalog`, `ew-api`, `ew-viewmodel`, `ew`; `check-deps`.

## 7. Part 3: the window

**Scope.** D14, FR-EXP-001's window clause. **0.5 pw.** Every brief of this part cites
[doc 16a](../../design/16a-design-language.md).

**Build list.** `crates/ew-viewmodel/src/actions.rs` (the two actions), the shell's handlers, the job for the export,
`CardOrigin::TableImport` with its describe line, `crates/ew-ui` (the card's per-cell list and the switch),
`strings.rs` keys `action-navigator-export-table`, `action-navigator-import-table`, `table-import-*`.

**Acceptance.**
- **TW-1** Headless (the actions dispatched as the transcript tests do): the Strips navigator in table mode, filtered to -X1031, `navigator.export_table`
  writes the shown rows and columns, byte-equal to `ew export table strips --strip -X1031` when nothing is filtered
  out beyond the strip; `navigator.import_table` of RT-2's file opens the card with "5 changed", Accept commits one
  undo step, Escape commits nothing.
- **TW-2** The conflict switch: RT-3's file shows "1 conflict"; with "Take the file's values" on, Accept applies it.
- **TW-3** The export runs as a job: an edit during the export is applied at once and the file holds the revision the
  job started from (`_ew`'s `rev`); cancel leaves no file.
- **TW-4** Window check (offscreen, `--visual`): the table menu with both actions and the import card at 1920 × 1080
  and at the 1280 × 800 floor, no truncated text; each text finding quotes its `strings.rs` key.
- **TW-5** Targeted checks for `ew-viewmodel`, `ew-ui`, `elektrawatt`; the snapshot gate.

## 8. Neighbours and who owns what

| Neighbour | It owns | This package |
|---|---|---|
| WP-S3.06 (specified; part 1 in this cycle, lane G42) | The kinds, `ew export report --kind` (its D4), the `params` encoding | **Follows its D3**: the XLSX writer is here, in `ew-reports` (D2). Whichever of WP-S3.06 part 1 and this part 1 integrates second adds `--format xlsx` to `--kind` (one match arm) |
| WP-S1.22 (parts 1 to 4 built; part 5 in this cycle, lane D42) | CLI conventions, `catalogue.rs`, result schemas, the API guard and the CLI snapshot | Adds its surfaces under those conventions (D7); after part 5 every addition re-blesses the snapshot and adds a release-note line. Its D2 still names WP-S3.06 for XLSX, stale against ID-950 (design pass) |
| WP-S3.09 (not specified) | `ImportBatch`, lineage, import runs, import preview, the `Exporter` trait | Nothing of them (D13); FP-1's row creation joins its lineage |
| WP-S2.06 (complete) | Navigators, table mode, `EDITABLE_COLUMNS` | D10 moves its pure rule, no behaviour change; reads its routes |
| WP-S2.09 (complete) | The Preview tab, `ew-jobs` | Part 3 adds one `CardOrigin` and one job |
| WP-S1.21 part 5 (built at the base) | `io_list` | Reads it; its PLC routes for FR-PLC-038 |
| WP-S3.03 (M1) | FT-61's wrapped rows, FR-EXP-005 | Neither (D15) |
| WP-S6.06 | FT-234's platform digest comparison | Its matrix covers the new goldens without change |

## 9. Files owned and what a lane may not decide

### 9.1 Files owned

**Part 1:** `crates/ew-reports/src/{table.rs,xlsx.rs,csv.rs,lib.rs}`, `crates/ew-reports/Cargo.toml` (the one dependency
line), `crates/ew-reports/tests/**`; `crates/ew-api/src/output/rows.rs`, `crates/ew-api/tests/**` (new tests);
`bin/ew/src/{cli.rs,catalogue.rs}`, `bin/ew/schemas/**`, `bin/ew/tests/**`; `docs/release-notes/unreleased.md` (lines);
`golden/J-E-2416/expected/xlsx/**`, `golden/J-E-2416/expected/csv/presets/**` (part-1 files); `crates/ew-zip/**` and
`crates/ew-store/{src/lib.rs,src/zip.rs,Cargo.toml}` (the move),
`crates/ew-import-catalog/src/{lib.rs,table.rs,xlsx.rs}` (D12's two functions), `ci/dep-rules.toml`,
`ci/licence-map.toml` and doc 04 §3 (the crate's rows; ID-1145). **Part 2:**
`crates/ew-api/src/output/{table_edit.rs,table_io.rs,mod.rs}`, `crates/ew-viewmodel/src/navigators/table.rs` (the moved
items and re-exports only), `crates/ew-import-catalog/src/{lib.rs,table.rs,xlsx.rs}` (visibility only; D12's two
functions moved to part 1, ID-1145); `bin/ew/**` as part 1; the part-2 golden files. **Part 3:**
`crates/ew-viewmodel/src/{actions.rs,shell.rs,shell/**,strings.rs}` for the two actions and the card,
`crates/ew-ui/src/**` for the card, their tests.

Shared append-only files (the CLI table, the release notes, `Cargo.lock`, `REUSE.toml` for the new golden paths) may be
touched by other lanes; the second to integrate rebases. Read-only for every lane: other crates, `golden/**` except as
named, `ci/**`, `.github/**`, `docs/**` except the release notes and generated reference pages.

### 9.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change needs a new workspace package (other than `ew-zip`, ID-1145), an external crate in `ew-reports`, or a
   workspace dependency edge (other than `ew-store` → `ew-zip` and `ew-reports` → `ew-zip`);
2. an existing exported byte of Golden A, AC-15's path (b) or the acceptance result changes;
3. a cell would become editable beyond D9's rule, or a round trip would create or delete a record;
4. a new command would be registered (the round trip is `fields.set_many`);
5. a golden fact here (the 50 wire data rows, 56 rows of 908, 20 terminals of -X1031, 28 I/O rows) is contradicted by the
   data;
6. D10's move would change a table-mode behaviour or test;
7. the work of a neighbour (§8) is reached.

The lane decides freely: private helper names, module layout inside the owned files, the column-width bounds (D4,
reported), the card's layout within doc 16a, test names and fixtures.

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 Excel opens our workbook but repairs it (an XML detail) | Medium · medium | The writer follows the minimal part set Excel accepts; the pilot opens `bom_parts.xlsx` in Excel and LibreOffice before G-M2 (manual entry, §12 O4); R-036 |
| R2 Excel changes data on a round trip (dates, leading zeros, a deleted `id` column) — R-036 | High · medium | Strings for every identifier (D4); EW-IMP-305; a missing `id` column refuses the XLSX ("the hidden id column is missing") and only `io_list` matches without it |
| R3 A dependency bump changes the XLSX bytes (`miniz_oxide`) | Low · low | Pinned `=0.8.9`; a bump re-blesses `expected/xlsx/sha256.txt` and lists it |
| R4 D10's move breaks table mode | Low · high | Re-exports under the old paths; view-model tests unchanged (RT-6) |
| R5 `--kind` lands after this part 1 | Medium · low | The second lane adds the match arm (§8) |

## 11. Size, order and routing

**Size.** 2.5 pw plus 0.3 pw reserve: part 1 **1.0** (the table model 0.1, the XLSX writer and ZIP 0.35, presets and
guard 0.15, CLI and `ew sql` 0.2, goldens and tests 0.2), part 2 **1.0** (D10's move 0.15, the public reader 0.05, the
navigator export 0.15, the merge and preview 0.35, the CLI 0.1, `io_list` and FR-PLC-038 0.1, goldens and tests 0.1),
part 3 **0.5** (actions and job 0.15, the card 0.2, window check and tests 0.15). **De-scope rule:** when a part is
projected or found more than 20 % over its size, these leave as follow-ups without a further ruling, in this order:
for part 1 `--bom` and `--line-end`, then `ew sql --format xlsx` (FT-218 stays open); for part 2 the (station, slot,
channel) match (FR-PLC-038 then needs the `id` column), then EW-IMP-305's lossy detection (a converted number is then
refused); for part 3 the conflict switch (conflicts then always keep EW).

**Order and routing** (Opus or Haiku only, ID-700). Part 1 **Opus at `high`** (a byte-exact file format and the
licence-sensitive dependency choice; its feasibility read flagged for Opus `low`); part 2 after part 1, **Opus at
`high`** (a move in a core view-model path, a data-changing import); part 3 after part 2, **Opus at `medium`** (UI on
existing patterns, with the window check). No Haiku lane: the golden blesses run inside the parts. Part 1 may run beside
WP-S3.06 part 1 (§8).

**Unblocks:** FR-PLC-023's, FR-EXT-003's and FR-EXT-027's XLSX clauses; the archive's `reports/<block>.xlsx` if 1.0
wants it; FT-385's gallery export.

## 12. Open questions for the orchestrator

- **O1** The PERT of D1 (O 2.0 · L 2.7 · P 4.2 · PERT 2.8) for `work-packages.csv`. Options: (1) enter it; (2) keep
  2.7 and cut part 3 to the export action only. Recommendation: (1).
- **O2** FR-EXP-001's trait clause. Options: (1) WP-S3.09 owns the `Exporter` trait and `ExportView` (the code's own
  note), FR-EXP-001 closes with it; (2) add a minimal trait here. Recommendation: (1), both are M2.
- **O3** D3's own writer against ADR-0018's rust_xlsxwriter. Options: (1) own writer, ADR-0018 amended (§13);
  (2) rust_xlsxwriter with FT-652 (pin krilla's compression backend) first. Recommendation: (1): (2) needs FT-652 and
  re-opens ID-827's hazard for one sink.
- **O4** A manual check that Excel and LibreOffice open `bom_parts.xlsx` and a round-trip file unrepaired, as a pilot
  item before G-M2. Options: (1) add it to the G-M2 owner checklist; (2) rely on the pilot's normal use.
  Recommendation: (1).

**Rulings** (project owner (delegated), 2026-10-31, decision log ID-1016): O1 option (1), the PERT 2.8 pw entered;
O2 option (1), WP-S3.09 owns the `Exporter` trait; O3 option (1), the own writer with ADR-0018 item 5 amended after
part 1; O4 option (1), the G-M2 owner checklist item, beside WP-S3.07's O4. The FT annotations of §13 are applied.

## 13. Follow-ups and the documents to update

**FT annotations, proposed** (the design pass edits the rows): **FT-61** XLSX half closes with part 1 (the writer in
`ew-reports`, own code, D3), wrapped rows stay with WP-S3.03; **FT-218** closes with part 1 (`ew sql --format xlsx`),
or stays open if de-scoped (§11); **FT-241** its XLSX half closes with part 1, so the row closes in full; **FT-242** its
preset half closes with part 1 (the golden dialect stays the default); its FR-EXP-005 half moves to FP-4. Proposed (not
opened):
- **FP-1** Round-trip row creation from unknown IDs (terminals with a strip column, channels with a module column) with
  WP-S3.09's lineage.
- **FP-2** Per-cell and per-column conflict choice in the import card.
- **FP-3** CSV encodings other than UTF-8 (Windows-1252).
- **FP-4** FR-EXP-005's type, ECLASS and ETIM columns with the classification data (WP-S1.28) and the column-map
  profile with doc 13's engine.
- A de-scoped item of §11 becomes a new FT at the design pass.

**Documents.** **After part 1:** ADR-0018 item 5, amended to read: "One row source, many sinks. The same rows feed PDF
pages, XLSX (an own writer in `ew-reports` over `miniz_oxide`, no ZIP crate: decision log ID-827 and this package's
D3), CSV and the `.ewpp`…" (rest unchanged); doc 09 §1.3 (`ew-reports`' writers), §4.6 (XLSX as built: inline strings,
the `id` column, `_ew`; the presets and `ew` as the default dialect); doc 14 §2's table row (written by `ew-reports`'
own writer); arch §3's `ew-reports` row ("later: rust_xlsxwriter, csv" replaced by `miniz_oxide`); doc 17 §3.3
(`--format xlsx`, `--preset`); doc 20 §3.1 (the new golden files); 02b FR-EXP-003 (as-built note: UTF-8 only).
**After part 2:** doc 14 §8.1 and §8.2 (as built: D11's merge, `fields.set_many` instead of `table.apply_changes`, the
reader of D12 instead of calamine); doc 16 §7.2's and §7.7's `table.apply_changes`; doc 17 §3.3 (`ew export table`,
`ew import table`; `ew import io-list` and the 1.0 `ew import table` row replaced); arch §3's `ew-import-tables` row.
**After part 3:** doc 16 §7.7 (the card as built); 02b FR-EXP-001 (as-built note: the trait with WP-S3.09, the stale
clause moot at the MVP). **At the next design pass:** WP-S1.22 specification D2's XLSX owner (WP-S3.08, ID-950).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S3.08, WP-S3.03, WP-S3.06, WP-S3.09, WP-S1.21, WP-S1.22, WP-S2.06,
  WP-S2.09, WP-S6.06; Appendix B (FT-61, FT-218, FT-234, FT-241, FT-242, FT-652).
- [Decision log](../06-decision-log.md): ID-356, ID-700, ID-827, ID-950.
- [Reports](../../design/09-reports-forms-output.md) §1.3, §4.3, §4.6; [interop](../../design/14-interop.md) §8;
  [UX](../../design/16-ux-ui.md) §7; [design language](../../design/16a-design-language.md);
  [checks](../../design/15-checks-revisions-collaboration.md); [extensibility](../../design/17-extensibility.md) §3;
  [golden project](../../design/20-golden-project.md); [architecture](../../design/04-architecture.md) §3.
- [ADR-0018](../../adr/0018-reports-forms-engine-and-machine-profiles.md); [risk register](../04-risk-register.md)
  R-036.
