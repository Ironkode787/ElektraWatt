# WP-S3.06: the MVP report pack, the remaining scope (closed specification)

**Status:** specified in cycle 41 (documents only), an M2 package, in three parts: part 1 the list kinds and the
export by kind (0.9 pw, D4 to D6, D9, D11), part 2 the open-items list (0.7 pw, D7, D8), part 3 the report back-links
drawn (0.6 pw, D10); 0.3 pw reserve; in M2's order after WP-S1.21's remaining work (doc 01, ID-779); part 1 and
part 2 in any order or side by side, part 3 after both and after WP-S4.01 part 4b · **Ruled in cycle 41** (decision log ID-950, project owner (delegated)): §12's O1 to O4 as proposed, except FP-2 (O4), which is not taken at MVP; the PERT in `work-packages.csv`; the FT annotations made · **Date:** 2026-10-31 ·
**Work package:** [WP-S3.06](../02-work-packages.md#wp-s306-mvp-report-pack) · **Decision log:** ID-950 ·
**Part of:** ElektraWatt plan, specifications

**Purpose.** Most of WP-S3.06's scope was built ahead of the package by other packages: nine of its fourteen
requirement rows are met in the code today, eight in full and FR-REP-042 in part, and two more belong to WP-S1.21
part 5 (§3). What is left is three report kinds that still give
`NotDerived::Unsupported` (`device_list`, `terminal_overview`, `open_items`), a way to export a kind that Golden A has
no block for, the back-links that report rows carry but that are drawn for connection points only, and the
revision overview's changed pages in its CSV. This document traces the built rows with their evidence, closes the
rest to its items, decisions and acceptance, and settles the boundaries with WP-S1.14, WP-S1.18, WP-S1.21, WP-S1.22,
WP-S3.08 and WP-S4.01. It is written against the code at `344e1098` (§2.1). Where this document and a design
document differ, this document is the narrower and wins for the lanes; the design documents stay the target, and the
documents of §13 are updated at the design pass after each part. A choice a lane meets that this document does not
settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the list kinds and the export by kind](#5-part-1-the-list-kinds-and-the-export-by-kind)
6. [Part 2: the open-items list](#6-part-2-the-open-items-list)
7. [Part 3: the report back-links drawn](#7-part-3-the-report-back-links-drawn)
8. [Neighbours and who owns what](#8-neighbours-and-who-owns-what)
9. [Files owned and what a lane may not decide](#9-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, order and routing](#11-size-order-and-routing)
12. [Open questions for the orchestrator](#12-open-questions-for-the-orchestrator)
13. [Follow-ups and the documents to update](#13-follow-ups-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package every MVP report kind of WP-S3.06's scope derives rows: `device_list` lists the
68 devices of doc 20 §6.1 with their full tag, function text, parts, technical data and main-representation cell;
`terminal_overview` prints "-X1031: 20 terminals, 12 spare" (FR-REP-010); `open_items` lists the TBD station address,
S56 and E56 without parts and the missing -3W1 length (FR-REP-021), exactly as the release check reports them.
Each of the three has a golden CSV of Golden A's data, written by `ew export report --kind <kind> --format csv`, and
no existing golden byte changes for them. Clicking "101" in the BOM's Folios cell of -101F1 opens folio 101 at
-101F1, on screen and in the PDF (FR-REP-042). The revision overview's CSV lists the same changed pages as folio 911.

**Non-goals** (each with its package):
- No XLSX writer, CSV dialect presets or Excel round trip: WP-S3.08 (D3). The new kinds export as CSV in the golden
  dialect now and as XLSX when WP-S3.08's writer exists, because every kind exports from its `RowSet`.
- No `io_list`, no FR-PLC-023 columns, no PLC overview box (FR-PLC-052): WP-S1.21 part 5 (D2).
- No layout legend or layout BOM rows: built by WP-S4.01 part 4a and drawn by part 4b; the end brackets are WP-S4.02
  part 5 (D2).
- No `check_report` (FR-CHK-014): it is not in WP-S3.06's scope (doc 09 §2.4 gives it its own requirement), and its
  PDF in the archive is FT-667. Part 2's dependency edge makes it buildable without a further edge.
- No new Golden A folio and no new report block in Golden A (D4). No change to a drawn page's look.
- No typed `params` or `filter` member in the schema; the `params` encoding is ruled, not typed (D9).
- No report-block gallery or block editing in the window (FT-385), no change to the company-standard wizard's block
  set (FP-2).
- No back-links in embedded blocks (PART DATA, the legend, the cover) and none in the wire list, terminal diagrams,
  cable overview, potential overview, numbering report or revision overview (FP-3).

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Reports](../../design/09-reports-forms-output.md) §2.2, §2.3, §2.4, §3.2, §3.4, §3.6, §4.6, §7 items 3 and 4, §8.1 | `ReportBlock` and its deferred `params`, `RowSet` and `LinkRef`, the kinds table, pagination and the "No entries" page, embedded blocks, tag runs, XLSX and CSV, report coverage, the golden folios |
| [Checks](../../design/15-checks-revisions-collaboration.md) §2.3 (`open_item`), §2.5, §3.2, §3.6, §4.2, §4.4, §4.5, §7 | The open-item field, waivers, release severity, supersession, the catalogue rows of the open-item rules, the Open items touchpoint |
| [PLC and bus](../../design/10-plc-and-bus.md) §5 (the open-items paragraph) | Which PLC values are open items |
| [Golden project](../../design/20-golden-project.md) §6.1, §9, §10, §14.1, §16, §18.1 | The 68 devices, the strips and their states, -3W1's length, the BOM folios, the report folios, EF-03, EF-04, EF-05, EF-08, EF-09, EF-17, EF-19 |
| [Extensibility](../../design/17-extensibility.md) §3.3 | `ew export report` as built (WP-S1.22 part 2) |
| [Architecture](../../design/04-architecture.md) §3 (`ew-reports`, `ew-checks`, rules 6 and 7) | The crate edges; part 2 adds one (D8) |
| ADR-0015 amendment 9, ADR-0017, ADR-0018, ADR-0019 | One link per listed item, the SQL projection's class, the report engine, the rule engine |
| Specifications | [WP-S1.21](WP-S1.21-plc-and-bus-model.md) D13 and PX-1 to PX-3; [WP-S4.01](WP-S4.01-layout-model-and-2d-panel-pages.md) D11, D16 to D18; [WP-S1.22](WP-S1.22-cli-surface-and-api-guard.md) D2 and part 2; [WP-S1.18](WP-S1.18-revisions-release-and-archive.md) D6, D7; [WP-S2.06](WP-S2.06-navigators-and-table-mode.md) D3 (`project_rows`) |
| Follow-ups | FT-46, FT-61, FT-69, FT-218, FT-231, FT-241, FT-242, FT-704 |

### 2.1 What exists at `344e1098` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| 14 registered kinds: `toc`, `parts_summary`, `parts_list`, `part_data`, `structure_overview`, `terminal_diagram`, `cable_overview_diagrams`, `wire_list`, `potential_overview`, `plc_overview`, `numbering_report`, `profile_tables`, `revision_overview`, `enclosure_legend` | `crates/ew-reports/src/kinds/mod.rs:55-72` | Parts 1 and 2 register three more |
| A schema kind without a registered engine gives `NotDerived::Unsupported(<name>)` and one placeholder page: `device_list`, `terminal_overview`, `io_list`, `open_items`, `check_report` | `crates/ew-reports/src/queries.rs:55-60`; `kind.rs` `kind_name` | Parts 1 and 2 |
| `ReportKindDef { name, columns, rows }`; `Registry::builtin()`; `project_rows(q, kind, include)`, the whole-project rows of a kind in its default order with no block arrangement (the navigators' source) | `crates/ew-reports/src/kind.rs`, `queries.rs` | Part 1's export by kind reads `project_rows` |
| `structure_overview`: structure rows, then one row per device that is not a terminal, by the natural order of the full tag; 68 device rows on Golden A (folio 910) | `crates/ew-reports/src/kinds/structure_overview.rs` | `device_list` takes the same device set and order |
| `terminal_diagram`: strips from `manual.include`, else every strip by the natural order of the tag; terminals in `(seq, ID)` order; the state `spare`, `reserved` or `not fitted` as authored, else `used` with a wire or bridge, else `unused` | `crates/ew-reports/src/kinds/terminal_diagram.rs` | `terminal_overview` counts the same states |
| `parts_summary` reads `extra.params.mode` provisionally (ID-131) | `crates/ew-reports/src/kinds/parts.rs:45-51` | D9 |
| `DevicePartLines` with `LineSource::Device`, `Carrier` (WP-S4.01 part 4a), strip accessory, bridge, assembly config | `crates/ew-derive/src/bom.rs` | `device_list`'s parts cell |
| `MainRep(device)`, `RepAnchorCell(rep)`, `tech_data_text(q, device)`, `DeviceTag`, `StructureOf` | `crates/ew-derive/src/tag.rs`, `cell.rs`, `part_text.rs`, `structure.rs` | `device_list` |
| `Cell::CellRef(PageRef, GridCell)` renders "101-B8"; `Cell::Folios`; `Cell::Range` with `compress_range` (FR-REP-046) | `crates/ew-reports/src/rowset.rs`, `render.rs:293-302` | Cells |
| `LinkRef { column, target }`, `LinkTarget::{MainRep, Cp, Page}`; `toc` sets `Page` on column 0, `parts_summary` `MainRep` on its Tags column, `plc_overview` `Cp` | `crates/ew-reports/src/rowset.rs:505-528`; `kinds/toc.rs`, `parts.rs:118-123`, `plc_overview.rs:175` | Part 3 |
| Only `Cp` row links are drawn: `row_links` in `PageTableOf`; `TableRow.links: Vec<(usize, Destination)>`, one link over a whole cell's text | `crates/ew-api/src/output/view.rs:856-868`; `crates/ew-scene/src/table.rs:33-90` | Part 3 |
| `Session::link_targets(&[Destination])` resolves page, representation and CP destinations; the canvas follows a display list's link sources (`PageLink`, `link_at`) | `crates/ew-api/src/output/navigation.rs:887-930`; `crates/ew-viewmodel/src/canvas.rs:70-90, 757` | Part 3's screen half needs no new view code |
| `ew export report [<project>] --block <block> [--format csv\|pdf] [--out] [--json]`; `RowExport::Report`; `row_export(q, what, block)`; the golden CSV dialect (`ew_reports::csv`) | `bin/ew/src/cli.rs:37, 418-430, 772-840`; `crates/ew-api/src/output/rows.rs:33-60, 217` | Part 1 adds `--kind` |
| `revision_overview`'s pages cell filled at draw time from `changed_pages(q, rev)`; the CSV of `--block 911` does not get it (FT-704) | `crates/ew-api/src/output/view.rs:931-937`; `output/revisions.rs:258` | Part 1 (D11) |
| `ew_checks::run(q, scheme, mode)` (supersession, waivers, scheme severity), `CheckOf` (`check_findings(rule, scope)`, Memo), `Finding { rule, severity, state, targets, discriminator, subject, page, message, fingerprint, … }`, `RuleMeta` (no open-item member) | `crates/ew-checks/src/engine.rs:30-60, 277`; `finding.rs:128-161`; `rule.rs:180-204` | Part 2 |
| The project's release scheme is `settings.extra.checks.release_scheme`, read by `Session::project_check_scheme(Release)`, else `pre-delivery` | `crates/ew-api/src/lib.rs:910-935` | Part 2 |
| EW-CORE-022 and EW-CORE-025 read `PageOrder`; no other rule does | `crates/ew-checks/src/rules/page.rs:149, 274` | D8's cycle rule |
| EW-CORE-026 (report coverage, FR-REP-047) | `crates/ew-checks/src/rules/page.rs:299-360` | Traced as built |
| EW-LAY-001 is not registered (WP-S4.01 part 5) | `crates/ew-checks/src/rules/` | OI-1's two row counts |
| `ci/dep-rules.toml`: `"ew-reports" = ["ew-derive", "ew-sqlview", "ew-text"]`, `"ew-sqlview" = ["ew-derive", "ew-checks"]`: `ew-checks` is already in `ew-reports`' graph | `ci/dep-rules.toml:73-78` | D8 |
| Golden A: 11 page blocks and 4 embedded blocks (doc 09 §2.2); `expected/csv/` holds 8 files; `expected/pdf/structure.json` 92 links (64 cross-reference items, 28 PLC overview rows); `expected/acceptance_headless.md` pins 92 links (AC-13, AC-14, D-07) | `golden/J-E-2416/` | D4, part 3's golden change |

### 2.2 Gaps found (why the parts are shaped as they are)

| Gap at `344e1098` | Consequence |
|---|---|
| `device_list`, `terminal_overview`, `open_items` give a placeholder page | Parts 1 and 2 |
| Golden A has no block of those kinds, and `ew export report` needs a block | The export by kind (D4) |
| No rule says whether its findings are open items; `ew-reports` has no edge to `ew-checks` | D7, D8 |
| A row source that runs the whole check set would read `PageOrder` from inside the pagination (EW-CORE-022, -025): a query cycle | D8 limits the list to open-item rules, none of which reads the page order |
| `MainRep` and `Page` row links are carried and not drawn; a Folios cell has one link at most | Part 3 (D10, FT-231) |
| `revision_rows` outside the drawn page lacks the changed pages | D11 (FT-704) |
| The `params` encoding is a format decision left to this package (FT-46, ID-131) | D9 |

## 3. Requirements traced

The 14 rows of the package. "Built" rows are met at `344e1098`; each names its evidence and adds no part.

| Requirement | State and what this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-REP-007 | **Built** by WP-S4.01 part 4a: enclosure, plate, rail and duct lines (`LineSource::Carrier`, 840667cf; BOM rows 22 to 25 at 1.71 m and 3.17 m, folio 902); the end brackets stay authored part uses until WP-S4.02 part 5 (FT-204's row 26) | — | WP-S4.01 LB-1 |
| FR-REP-008 | `device_list` (D5) | 1 | RK-1, RK-4 |
| FR-REP-010 | `terminal_overview` (D6) | 1 | RK-2, RK-4 |
| FR-REP-017 | **Built** (cycle 7): `structure_overview`, folio 910, 4 structure rows and 68 device rows; part 3 adds the device rows' links | 3 | BL-2 |
| FR-REP-018 | **Built** by WP-S1.18 part 2 (ID-861, FT-207): `revision_overview`, folio 911 V1.0 and V2.0 with 101, 106; part 1 gives its CSV the changed pages (D11, FT-704) | 1 | RK-6 |
| FR-REP-019 | **Built** (cycle 13, ID-333): `plc_overview`, folios 908 to 908.2, 56 rows | — | AC-29 (WP-S1.21 part 3) |
| FR-REP-021 | `open_items` from the findings of the open-item rules (D7); "unwaived errors" are the check report's and the release gate's (D7) | 2 | OI-1 to OI-5 |
| FR-REP-022 | **Built** (cycle 14, ID-349): `numbering_report`, EF-15's ten frozen tags | — | — |
| FR-REP-042 | **Built** for connection points (`plc_overview` rows, `view.rs:856`); part 3 draws page and representation links, one per listed folio (D10) | 3 | BL-1 to BL-4 |
| FR-REP-046 | **Built** (`compress_range`, `render.rs:293`): 901 prints "-201U3-Q1 … -202U2-Q4" | — | — |
| FR-REP-047 | **Built**: EW-CORE-026 (`rules/page.rs:299`) | — | — |
| FR-PLC-022 | **Built**: folio 908 (cycle 13); the `PlcAddress` slot and pin addresses (WP-S1.21 part 2); Golden A's slots (FT-698) | — | AC-29 |
| FR-PLC-023 | WP-S1.21 part 5 (PX-1, `io_list`); its XLSX clause waits for WP-S3.08 (D3); its switch for deactivated channels is a block parameter under D9 (§12, O1) | — | WP-S1.21 PX-1 |
| FR-PLC-052 | WP-S1.21 part 5 (PX-3, a de-scope candidate there) | — | WP-S1.21 PX-3 |

**AC-29** passes since WP-S1.21 part 3 (folio 908 is this package's FR-PLC-022 share, built). **AC-33**'s share here is
the legend rows and FR-REP-007, both WP-S4.01's (parts 4a and 4b); this package adds nothing to AC-33.

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under the decision log row of this
specification (ID-950) and the as-built entries after each part.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Built rows are traced as built and sized at nil.** The package's remaining size is parts 1 to 3 (2.2 pw) and a 0.3 pw reserve; the proposed PERT is O 1.8 · L 2.4 · P 3.6 · **PERT 2.5** (σ 0.3), against 3.7 in `work-packages.csv`. WP-S3.03's status row that still calls `plc_overview`, `profile_tables` and `revision_overview` placeholders (cycle 9) is stale: the first two are derived since cycles 13 and 12 (its own later rows), the third since cycle 37 (WP-S1.18 part 2) | Eight rows (and FR-REP-042's connection-point links) were built by WP-S3.03, WP-S1.18, WP-S1.21 and WP-S4.01 and are counted there; re-sizing them here would count them twice |
| D2 | **Boundaries** (§8). `io_list`, FR-PLC-023 and FR-PLC-052 are WP-S1.21 part 5's and are not specified again. `enclosure_legend`, the layout BOM rows and the legend drawing are WP-S4.01's; the end brackets WP-S4.02 part 5's. Revisions and the release are WP-S1.18's (complete); only FT-704 comes here. `device_list`, `terminal_overview` and `open_items` are this package's (FT-69: "the WP that owns each kind"). `check_report` stays outside. New CLI follows WP-S1.22 part 1's conventions | One owner per kind; the kinds in this package's scope-in with no other owner are built here |
| D3 | **The XLSX writer is WP-S3.08's**, not this package's (WP-S1.22's routing, FT-218, FT-241) and not WP-S3.03's (FT-61). WP-S3.08 owns FR-REP-030 (every report as XLSX and CSV), FR-EXP-001 to -004 (the hidden `id` column and the `_ew` sheets of the round trip), NFR-SEC-17 (the formula-injection guard) and the CSV presets (FT-242); the writer, its determinism (ZIP dates, workbook properties) and the round-trip sheets are one design (doc 09 §4.6), built once. FT-218, FT-241's XLSX half and FT-242 move with it; FT-61's XLSX half too, its wrapped rows stay with WP-S3.03 until the schema has a wrap policy | A first writer here would be reworked by the round trip; WP-S3.03 is an M1 package closed in practice. The cost: FR-PLC-023's, FR-EXT-003's and FR-EXT-027's XLSX clauses wait for WP-S3.08, an M2 package too, so G-M2 is not moved |
| D4 | **Golden examples without new folios: the export by kind.** Golden A reproduces the reference's 36 folios (doc 20 §16), so a kind without a folio gets its golden as an exported row file of Golden A's data, `golden/J-E-2416/expected/csv/<kind>.csv`, written by **`ew export report --kind <kind> --format csv`**: the kind's `project_rows` (whole project, default order, no block arrangement) in the golden CSV dialect. `--kind` and `--block` exclude each other; `--kind` takes CSV only (pages need a block). Refusals, exact: "give `--block` or `--kind`, not both"; "`--kind` exports rows: a kind has pages only through a block, use `--block <block> --format pdf`"; "unknown report kind `<name>`; the kinds of this build are: <registered names, comma-separated>"; "`terminal_diagram` lists one strip: use `ew export terminals`". `--json`'s summary gives the kind, the row count and no block. A kind that has a folio in doc 20 keeps it and gets no CSV here | Adding blocks to Golden A would change the fixture, the tables, the TOC and every page after; the export adds files and changes no existing byte. `project_rows` is what the navigators show, so the golden proves the navigator rows too |
| D5 | **`device_list`** (FR-REP-008). One row per device that is not a terminal, **the device rows of `structure_overview`** (one predicate, shared), in the natural order of the full tag, ties by ID; row ID `entity(device)`; sources the device. Columns (key, type, label): `tag` Ident "Tag" (the full designation); `function_text` Text "Function text" (empty when none); `parts` Ident "Parts" (the part numbers of the device's `LineSource::Device` lines, distinct, in line order, joined by ", "; no carrier, accessory, bridge or configuration line); `tech_data` Text "Technical data" (`tech_data_text`); `location` Ident "Location (+)" (as 910 prints it); `main_rep` CellRef "Main representation" (`MainRep`'s page and `RepAnchorCell`; empty when the device has no representation). Doc 09's group "location" is a block's `group` over the `location` column; the kind does not group | The same device set as 910 by construction, so the two lists never disagree; FR-REP-008's five facts and nothing else. Carrier lines stay off -UC1's row because the legend and the BOM list them |
| D6 | **`terminal_overview`** (FR-REP-010). One row per strip in `terminal_diagram`'s default strip order; row ID `entity(strip)`. Columns: `strip` Ident "Strip" (the short tag, as the terminal-diagram titles print it); `location` Ident "Location (+)"; `terminals` Number "Terminals"; `used` Number "Used"; `spare` Number "Spare" (state `spare`); `other` Number "Other" (states `reserved`, `not fitted`, `unused`), so that terminals = used + spare + other; `parts` Text "Parts" (each distinct terminal part as "<count> × <part number>" in the order of first appearance by `seq`, joined by ", "); `segments` Text "Segments" (the cells of the strip's segment representations in their drawing order, joined by ", "). The state rule is `terminal_diagram`'s, moved to one shared function | FR-REP-010 asks for counts, spares and parts; one state rule keeps 904's State column and this row in agreement |
| D7 | **`open_items` is a view of the findings** (doc 15 §2.3 `open_item`, §7). Its rows are the **open** findings (not waived, not superseded) of the **open-item rules** in **release mode** of the project's release scheme, else `pre-delivery`, through the engine's own path (scheme, supersession, waivers, severity), so a finding is listed exactly when `ew check --mode release` prints it open. The open-item rules, with their category (the words print in the `category` column): **"TBD value"** EW-CORE-043, EW-CORE-044, EW-PLC-015, EW-BUS-001 (discriminator `address` only; `duplicate` is not an open item); **"Device without part"** EW-CORE-041; **"Unplaced device"** EW-LAY-001 (when registered); **"Unconnected connection point"** EW-POT-009; **"Missing length"** EW-CORE-033. Order: category in that order, then the natural order of the subject, then the fingerprint; row ID `name(<rule>/<fingerprint hex>)`. Columns: `category` Text "Category"; `item` Text "Item" (the finding's subject); `description` Text "Description" (the finding's message, en-GB as `ew check` prints it); `rule` Ident "Rule"; `severity` Text "Severity" (`error`, `warning`, `info`); `page` Folio "Page" (the finding's page, else empty). **FR-REP-021's "unwaived errors"** are not repeated here: every unwaived error is the check report's (FR-CHK-014) and blocks the release gate (FR-CHK-042); listing them would make this list a second check report, and the rules that read the page order (EW-CORE-022, -025) cannot run inside a pagination (D8) | One fact in one place: the message panel, `ew check` and the printed list agree by construction, and a TBD value is found by the rule that owns it, not by a second predicate in `ew-reports`. The cost is FR-REP-021's last clause, recorded as an as-built note on 02a (§13); its acceptance (the TBD address, S56 and E56, -3W1's length) is met in full |
| D8 | **Where the open-item code lives, and one new edge.** The table of D7 is data in `ew-checks` (`open_items.rs`: rule ID, optional discriminator, category), not a `RuleMeta` member (a member would touch every rule definition for one reader); the catalogue snapshot `crates/ew-checks/catalogue/rules.json` and the generated rule pages gain an `open_item` column from it; a registry test asserts that every listed ID is registered. `ew_checks::open_items(q) -> Vec<Finding>` runs those rules, and only those, through the engine path of D7 with `try_get`, reporting a query cycle as a cycle, never a panic; the release-scheme lookup moves into `ew-checks` (`release_scheme_of(q)`), and `Session::project_check_scheme(Release)` calls it. The kind lives in `ew-reports` and calls `ew_checks::open_items`: **`ci/dep-rules.toml` gains `ew-checks` in `"ew-reports"`**, and arch §3's `ew-reports` row gains the edge at the design pass. **Contract:** an open-item rule never reads `PageOrder` (the list paginates inside it); D7's rules do not today, and adding a rule that does to the table is a stop (§9.2) | `ew-checks` is already in `ew-reports`' graph through `ew-sqlview`, so no package enters the binary and no cycle forms; the alternative, a provider hook on `Q` like `ReportPagesProvider`, is more machinery for the same reads. The SQL projection would make the list Class II (stale on screen). `check_report` will need the same edge |
| D9 | **The `params` encoding (FT-46), ruled and not typed.** `params` is a **top-level member** of a report block, a JSON object of the parameters its kind declares: snake_case keys, JSON scalar values. Declared now: `parts_summary` `mode` ("component", the default, or "assembly"); `io_list` `include_deactivated` (boolean, default false) when WP-S1.21 part 5 adopts it (O1). A kind declares its parameters through a new `ReportKindDef::params()` (key, type, default; empty by default); `ew_reports::kind_params(block)` reads them; an undeclared key is ignored and a value of the wrong type gives the default, each with one `DeriveNote::MalformedShim` naming the block and the key. The member stays in `extra`: no `schemas/0.1` change. **`filter` stays untyped** too | No command, import or wizard writes `params` or `filter` at the MVP, so typing them now would type members with no writer (WP-S4.01 D3's rule); the shape is fixed now, so a 0.1 file that carries it reads the same once the schema steward types both with their first writer (FT-385's `report_block.set`) |
| D10 | **Back-links drawn** (FR-REP-042, FT-231). Every `LinkTarget` is drawn on a generated page: `MainRep(d)` → the representation `MainRep(d)` (no link when the device has none: never a dead link), `Page(p)` → the page, `Cp` as built. **One link per listed item:** `LinkRef` gains `item: Option<u16>` (the n-th item of a multi-item cell) and `LinkTarget` gains `Rep(Id<Representation>)`; `ew-api` renders the cell's items one by one to find each item's byte span in the cell text and hands `ew-scene` `(column, span, destination)` links, which it draws as a cross-reference text's items are drawn (ADR-0015 amendment 9). **The links, closed:** `toc` folio cell → the page (carried, now drawn); `parts_summary` Tags cell → `MainRep` of its first device (carried, now drawn) and **each folio of its Folios cell** → the representation, on that folio, of the row's first source device drawn there (smallest representation ID), else the page (902 for a layout object); `parts_list` tag → `MainRep`; `structure_overview` designation of a device row → `MainRep`; `device_list` tag and main representation → `MainRep`; `terminal_overview` strip → `MainRep`; `open_items` item → the finding's page, else `MainRep` of its first device target, else none; `plc_overview` as built. Embedded blocks draw no links. **A golden change** (§7) | The acceptance asks for the folio, not the tag ("101" opens folio 101 at the device); one link per item is the cross-reference rule users already know. The window follows display-list links already, so the screen half needs no view code |
| D11 | **The revision overview's changed pages outside the page (FT-704).** `ew-api` fills the pages cell of `revision_overview` rows with `changed_pages(q, rev)` in every row export (`--block` and `--kind`), through the function the drawn cell uses (`view.rs:931`), so the CSV and folio 911 print the same pages. No navigator lists revision rows at the base, so FT-704's navigator half is moot | The changed pages need the page order, which a row query may not read; the export runs outside the pagination and can |
| D12 | **Every part closed to its items, decisions and acceptance;** each part is re-read at dispatch against the delivered earlier parts and the neighbours' state (WP-S1.21 part 5, WP-S4.01 parts 4b and 5) without reopening D1 to D11 | The three parts are small and independent; part 3 waits for the PDF changes before it |
| D13 | **Size and reserve.** The parts sum to **2.2 pw**; **0.3 pw** is held for golden re-blesses that neighbours' lanes force on the new CSV files and the PDF, with the de-scope rule of §11 | Each new golden file is one more file other golden changes may re-bless |

No decision changes an ADR. D8 changes one edge of `ci/dep-rules.toml` and arch §3's `ew-reports` row (§13).

## 5. Part 1: the list kinds and the export by kind

**Scope.** D4 to D6, D9, D11; FT-69's `device_list` and `terminal_overview`; FT-46 as ruled; FT-704. **0.9 pw.**

**Build list.**
1. `crates/ew-reports/src/kinds/device_list.rs` and `terminal_overview.rs`, registered in `kinds/mod.rs`; the
   non-terminal device predicate shared with `structure_overview.rs`; the terminal state rule moved from
   `terminal_diagram.rs` into one shared function (in `kinds/conn.rs` or a new private module).
2. `crates/ew-reports/src/kind.rs`: `ReportKindDef::params()` with an empty default; `kind_params(block)`;
   `parts_summary` reads `mode` through it; `io_list` too if WP-S1.21 part 5 is integrated at the lane's base.
3. `crates/ew-api/src/output/rows.rs`: `RowExport::Kind(String)` over `ew_reports::project_rows`, the refusals of
   D4 as `RowsError` variants; D11's pages cell for `revision_overview` rows in both exports.
4. `bin/ew/src/cli.rs`: `--kind <kind>` on `ew export report`, its help line and the refusals of D4.
5. Golden: `expected/csv/device_list.csv` and `expected/csv/terminal_overview.csv`, blessed by a new test that
   compares each with the export by kind (`EW_BLESS=1` writes it), and a `bin/ew/tests/outputs.rs` case that runs the
   command line.

**Acceptance.**
- **RK-1** `ew export report golden/J-E-2416.ew --kind device_list --format csv` writes a header and **68** rows: the
  device rows of folio 910, in 910's order; -101F1 reads tag "=P01+J-E-2416-101F1", parts "A9P52610", main
  representation "101-B8"; -201U1 parts "195740, 550208", main representation "201-E1"; -UC1 parts "NSYPLA1074G" (no
  rail or duct line) and an empty main representation; S56 and E56 an empty parts cell. The output equals
  `expected/csv/device_list.csv`. A value that contradicts doc 20 §6.1 is a stop.
- **RK-2** `--kind terminal_overview` writes three rows: 1X1 3 terminals, 3 used, 0 spare, 0 other, "2 × UK 5 N, 1 ×
  USLKG 5", segments "101-D2"; -X1031 20, 8, 12, 0, "20 × PT 2.5-TWIN", "103-A3, 103-A10"; -X1032 5, 4, 1, 0, "5 × PT
  2.5-TWIN-PE", "103-F3"; it equals `expected/csv/terminal_overview.csv`. A fixture with a `reserved` terminal counts
  it in Other; renumbering -X1031 to -X1041 keeps the row ID; each strip's counts agree with its `terminal_diagram`
  rows' State column.
- **RK-3** The four refusals of D4, each a whole-text assertion; `--json` gives the kind, the row count and an empty
  block list; `--kind parts_summary` writes the component-mode rows of 901 and 901.1 in the kind's default order.
- **RK-4** A fixture with a `device_list` and a `terminal_overview` block created by `report_block.create` (folios
  912 and 913) paginates them with the house layout (68 rows: 912, 912.1, 912.2) and no placeholder page; the TOC
  lists them; Golden A's page order is unchanged.
- **RK-5** `parts_summary` with `params.mode` "assembly" gives the assembly rows, with "Assembly" (wrong case), 3 or an
  undeclared key the component rows and one `MalformedShim` note naming the block and the key; no file byte changes
  on save.
- **RK-6** `ew export report --block 911 --format csv` prints the pages cell "101, 106" for V2.0 and folio 911's cell
  for V1.0, the same texts as folio 911 prints; `--kind revision_overview` the same rows.
- **RK-7** No existing byte of Golden A changes (PDF, PNG, text, rows, CSV, the fixture); the two CSV files are new;
  twice-run determinism of both exports; targeted checks for `ew-reports`, `ew-api` and the `ew` binary
  (`cargo nextest run -p ew-reports -p ew-api -p ew --all-features`, the matching clippy,
  `cargo fmt --all --check`).

## 6. Part 2: the open-items list

**Scope.** D7, D8; FT-69's `open_items`. **0.7 pw.**

**Build list.**
1. `crates/ew-checks/src/open_items.rs`: D7's table, `open_items(q)`, `release_scheme_of(q)`; the catalogue
   snapshot's `open_item` column and its test; `cargo xtask docs-gen` for the generated rule pages.
2. `ci/dep-rules.toml`: the one entry `ew-checks` in `"ew-reports"`; `crates/ew-reports/Cargo.toml`: the dependency.
3. `crates/ew-reports/src/kinds/open_items.rs`, registered; rows read through `try_get`, a cycle reported as
   `NotDerived::Cycle`.
4. `crates/ew-api/src/lib.rs`: `project_check_scheme(Release)` through `release_scheme_of`.
5. Golden: `expected/csv/open_items.csv`, blessed and compared as part 1's files.

**Acceptance.**
- **OI-1** On Golden A the rows are exactly: "TBD value" 5 (-201U1:DP, EF-03; the four TBD parts of EF-17),
  "Device without part" 2 (S56, E56; -XPE is waived by W-1, EF-19, and not listed), "Unconnected connection point" 2
  (-102F2:13, -102F2:14, EF-09), "Missing length" 1 (-3W1, EF-08): **10 rows**; with EW-LAY-001 registered at the
  lane's base, also "Unplaced device" 5 (F55, E55, E56, S56, -201U9, EF-05): **15 rows**. EW-PLC-012's 28 and
  EW-POT-013's 2 are not listed. The output of `--kind open_items` equals `expected/csv/open_items.csv`. Another count
  is a stop.
- **OI-2** Agreement: on Golden A and on fixtures (a TBD channel address, where EW-PLC-015 lists it once and
  EW-CORE-044 not; S56's finding waived, where its row goes; a project whose release scheme is `drafting`), the rows
  are exactly the open findings of the open-item rules (EW-BUS-001 with discriminator `address` only) that
  `Session::check(release scheme, Release)` reports.
- **OI-3** An `open_items` block (fixture, folio 914) paginates inside `page_order` with no cycle note; giving -102G1
  a TBD part adds one "TBD value" row and keeps the block's pages in the same commit; a project without open items
  gives no rows, and a block with `empty` "No entries" prints that page (doc 09 §7 item 4).
- **OI-4** The catalogue snapshot's `open_item` column marks exactly D7's rules; every ID of the table is registered
  (EW-LAY-001 is in the table only when registered); `cargo xtask docs-gen` is clean; `cargo xtask check-deps`
  passes with the one new edge and no other.
- **OI-5** No existing byte of Golden A changes; `open_items.csv` is new; twice-run determinism; targeted checks for
  `ew-checks`, `ew-reports`, `ew-api` and the `ew` binary; `check-deps`.

## 7. Part 3: the report back-links drawn

**Scope.** D10; FR-REP-042's remainder; FT-231. **0.6 pw.** After parts 1 and 2 (their kinds' links) and after
WP-S4.01 part 4b (both re-bless the golden PDF).

**Build list.**
1. `crates/ew-reports`: `LinkRef.item`, `LinkTarget::Rep`; the links of D10's table in `parts.rs`,
   `structure_overview.rs`, `device_list.rs`, `terminal_overview.rs`, `open_items.rs`.
2. `crates/ew-api/src/output/view.rs`: `row_links` resolves every target and each item's span; `PageTableOf` hands
   them to the scene.
3. `crates/ew-scene/src/table.rs`: a link over a span of a cell's text, drawn as a cross-reference text's item links
   are; a whole-cell link as before.
4. Golden: the re-bless of D10's change, listed below.

**Golden change** (decided by the orchestrator under the owner's standing delegation of 2026-10-27, re-blessed and
recorded): `golden/J-E-2416/expected/pdf/sha256.txt`; `expected/pdf/structure.json` (`links`, `destinations`);
`expected/acceptance_headless.md` (AC-13's and D-07's link count, AC-14's SVG link count and hash); the blind selftest
recordings' golden hash where it follows; every test that pins the 92 links. PNG, text, rows and CSV files do not
change; if one does, the lane stops.

**Acceptance.**
- **BL-1** On 901, -101F1's row's Folios cell carries a link over "101" to -101F1's representation on 101; a row with
  two folios carries two links, each over its own folio's text; rows 22 to 25 (layout objects) link page 902.
  `Session::link_targets` of the "101" link gives folio 101 and the cell 101-B8, so the window follows it like a
  cross-reference.
- **BL-2** The TOC's 36 rows link their pages; 910's 68 device rows link their devices' main representations, and
  -UC1 and -UC1-plate (no representation) get none; 908's 28 CP links are unchanged.
- **BL-3** RK-4's and OI-3's fixture blocks draw D10's links; an open item whose finding has no page and no device
  target draws none.
- **BL-4** The golden PDF holds one link annotation per drawn row link to a named destination; `structure.json`'s
  `links` equals 92 plus the drawn row links, counted by a test from the page views (no hand-written total); the SVG
  export's `<a href>` all resolve (AC-14); the golden change is exactly the list above; twice-run determinism.
- **BL-5** Window check (offscreen, `--visual`): folio 901 at 1920 × 1080 and at the 1280 × 800 floor, the pointer
  over "101" in -101F1's row shows the link's label 101-B8, and following it opens folio 101 with -101F1's
  representation in view; nothing in the page's look changes.
- **BL-6** Targeted checks for `ew-reports`, `ew-scene`, `ew-api` and `ew-viewmodel`; the snapshot gate and both blind
  selftests.

## 8. Neighbours and who owns what

| Neighbour | It owns | This package |
|---|---|---|
| WP-S1.14 (complete, M-PP) | Strips, terminals with `seq`, part and authored state, bridges, cables with lengths and their source | Reads them: `terminal_overview` (strips, terminal parts and states), `open_items` through EW-CORE-033 (cable lengths). No open WP-S1.14 item blocks a kind here (FT-104, FT-106 and FT-141 touch other columns) |
| WP-S1.18 (complete) | Revisions, the release, `revision_overview` and folio 911 | FT-704 only (D11) |
| WP-S1.21 part 5 (specified, not built) | `io_list` with FR-PLC-023's columns (PX-1), the PLC overview box (PX-3, FR-PLC-052) | References them; D9 gives `io_list` its block parameter; the XLSX of PX-1's list is WP-S3.08's (D3) |
| WP-S1.22 part 2 (built) | `ew export report --block`, `RowExport`, the golden CSV dialect | Adds `--kind` under part 1's conventions (D4) |
| WP-S3.08 (not specified) | The XLSX writer, CSV presets, the round trip, the formula-injection guard | Every kind of this package exports through it when it lands (D3) |
| WP-S4.01 parts 4a (built), 4b and 5 (specified) | `enclosure_legend`, the layout BOM rows, the embedded blocks drawn, EW-LAY-001 | Nothing of the legend; OI-1's fifth category follows part 5; part 3 re-blesses the PDF after 4b |
| WP-S4.02 part 5 (specified) | The end brackets in the BOM (FR-REP-007's rest) | Nothing |
| WP-S1.16 and FT-667 | The check report (FR-CHK-014) | Leaves it; D8's edge serves it |

## 9. Files owned and what a lane may not decide

### 9.1 Files owned

**Part 1:** `crates/ew-reports/src/kinds/{device_list.rs,terminal_overview.rs,mod.rs,structure_overview.rs,terminal_diagram.rs,conn.rs,parts.rs}`,
`crates/ew-reports/src/kind.rs`, `crates/ew-reports/tests/**`, `crates/ew-reports/README.md` (the kind rows);
`crates/ew-api/src/output/rows.rs`, `crates/ew-api/tests/**` (new tests only); `bin/ew/src/cli.rs`,
`bin/ew/tests/**`; `golden/J-E-2416/expected/csv/{device_list,terminal_overview}.csv`. **Part 2:**
`crates/ew-checks/src/{open_items.rs,lib.rs}`, `crates/ew-checks/catalogue/**`, `crates/ew-checks/tests/**`, the
generated rule pages; `ci/dep-rules.toml` (the one entry of D8 only); `crates/ew-reports/Cargo.toml`,
`crates/ew-reports/src/kinds/{open_items.rs,mod.rs}`; `crates/ew-api/src/lib.rs` (the scheme lookup only);
`golden/J-E-2416/expected/csv/open_items.csv`. **Part 3:** `crates/ew-reports/src/{rowset.rs,kinds/**}` for the links;
`crates/ew-api/src/output/view.rs`, `crates/ew-scene/src/table.rs`, their tests; the golden files of §7's list.

Shared append-only files (the kind registry, the CLI help, the catalogue's generated pages, `Cargo.lock`) may be
touched by other lanes; the second to integrate rebases. Read-only for every lane: other crates, `golden/**` except as
named, `ci/**` except part 2's entry, `.github/**`, `docs/**` except generated reference pages. Each lane commits on
its own branch and does not merge or push.

### 9.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change needs a new workspace package, an external crate or a dependency edge other than D8's;
2. a schema member would be typed (`params`, `filter` or any other) or `schemas/0.1` would change;
3. an existing exported byte of Golden A, AC-15's path (b) or the acceptance result changes outside its part's list;
4. a golden fact here (68 device rows, the three strips' counts, OI-1's rows, 101-B8) is contradicted by the data;
5. a rule would join D7's table, or one in it would read `PageOrder`;
6. a link target beyond D10's table, or a link in an embedded block, would be added;
7. the work of a neighbour (§8) is reached.

The lane decides freely: private helper names, module layout inside the owned files, column weights, the
`MalformedShim` texts, test names and fixtures.

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The open-items list closes a query cycle (a rule reaching the page order) | Low · high | D8's contract and §9.2 item 5; rows through `try_get`; OI-3 paginates the list inside `page_order` |
| R2 The new CSV goldens pin data that other lanes change (a device, a part, a TBD value) | High · low | Blessed with `EW_BLESS=1` and named in each part's list; a later lane's golden list names the files it re-blesses (CLAUDE.md rule 20); D13's reserve |
| R3 OI-1's count depends on whether WP-S4.01 part 5 lands first | Certain · low | Both counts are stated; the later of the two lanes re-blesses `open_items.csv` and lists it |
| R4 Part 3's PDF re-bless collides with WP-S4.01 part 4b's | Medium · low | Part 3 after part 4b (§11) |
| R5 Span links misplace on shaped text | Low · medium | The cross-reference item rule already does it; BL-1 asserts the "101" box |

## 11. Size, order and routing

**Size.** 2.2 pw plus 0.3 pw reserve (D13): part 1 **0.9** (two kinds 0.35, the export by kind 0.15, the params reader
0.1, FT-704 0.1, golden and tests 0.2), part 2 **0.7** (the open-item table, `open_items` and the catalogue 0.3, the
edge 0.05, the kind 0.15, golden and tests 0.2), part 3 **0.6** (span links in the scene 0.2, link resolution 0.15,
the kinds' links 0.1, golden and window check 0.15). **De-scope rule:** when a part is projected (at its lane's
midpoint report) or found (at its review) more than 20 % over its size, these items leave as follow-ups without a
further ruling, in this order: for part 1 FT-704, then the params reader; for part 3 the per-item folio links (the
Folios cell then links whole to its first folio's representation, and BL-1's two-folio clause becomes an FT), then
`structure_overview`'s links; the lane reports which ones left.

**Order and routing** (Opus or Haiku only, ID-700). In M2's order (doc 01, ID-779) the package follows WP-S1.21's
remaining work, so it is dispatched after WP-S1.21 part 5; its parts need nothing of part 5 except part 1's
`io_list` parameter clause, which applies only if part 5 is in. Part 1 **Opus at `medium`** (two kinds on the
existing pattern and a CLI flag; new golden files only); part 2 **Opus at `high`** (a crate edge, the check engine and
a cycle hazard), side by side with part 1 or after it; part 3 after parts 1 and 2 and after WP-S4.01 part 4b, **Opus
at `high`** (a page output path and a golden PDF change; its feasibility read is flagged for Opus `low`). The
feasibility read before dispatch flags part 2 (the edge, a check rule's meaning). No part needs a Haiku lane; the
golden re-blesses run inside the parts.

**Unblocks:** `check_report` (FR-CHK-014) through D8's edge; WP-S3.08's XLSX of these kinds; FT-385's block gallery
can offer the three kinds.

## 12. Open questions for the orchestrator

- **O1** WP-S1.21's PX-1 says `io_list` lists 56 rows on Golden A; FR-PLC-023's acceptance says 28 by default and 56
  with the deactivated filter off. Proposed: the default hides deactivated channels and the block parameter
  `include_deactivated` (D9) shows them, PX-1 asserting both counts; for WP-S1.21 part 5's brief, not edited here.
- **O2** The PERT re-estimate of D1 (O 1.8 · L 2.4 · P 3.6 · PERT 2.5) for `work-packages.csv`.
- **O3** The FT annotations of §13 for the design pass.
- **O4** FP-2: whether new projects get the three blocks.

*Ruled at the cycle-41 design pass (ID-950, project owner (delegated)):* O1 as proposed, into WP-S1.21 part 5's brief (`include_deactivated`, default false: 28 rows, 56 with it); O2 entered in `work-packages.csv` (PERT 2.5, the remaining work only); O3 the annotations made in doc 02; O4 no open-items block in the company standard by default at MVP (the export by kind and the report kind cover it).

## 13. Follow-ups and the documents to update

**Follow-ups, proposed annotations** (the design pass edits the rows): **FT-46** encoding ruled by D9, typing waits
for the first writer (FT-385); **FT-61** XLSX half to WP-S3.08 (D3), wrapped rows stay with WP-S3.03; **FT-69** closes
for `device_list`, `terminal_overview` and `open_items` with parts 1 and 2 (`numbering_report` since cycle 14,
`enclosure_legend` since WP-S4.01 part 4a, `io_list` with WP-S1.21 part 5), `check_report` stays open with FT-667;
**FT-218** to WP-S3.08; **FT-231** to WP-S3.06 part 3, closes with it; **FT-241** closed in part by WP-S1.22 part 2
(`wires`, `terminals`, `--block`, PDF through `export report --block`), its XLSX to WP-S3.08; **FT-242** to WP-S3.08;
**FT-704** to WP-S3.06 part 1, its navigator half moot (D11). Proposed (not opened):
- **FP-1** The schema steward types `params` and `filter` with `report_block.set` (FT-385).
- **FP-2** The company-standard wizard's block set (doc 16 §9.2 step 9, doc 09 §2.2) gains an open-items block, so a
  new project prints the list without a script; and the device list and terminal overview if the pilot asks.
- **FP-3** Back-links in the wire list (wire ends), terminal diagrams (targets), cable overview, potential overview,
  numbering report, revision overview (changed pages) and embedded blocks.
- **FP-4** `open_items` messages in the display language (they print en-GB like `ew check`).
- **FP-5** The 1.0 open items of doc 10 §5 join D7's table with their rules: EW-PRC-002 (an instrument-list tag
  without device or channel) and a rule for a used channel without a main representation, which no catalogue row
  names yet.
- A de-scoped item of §11 becomes a new FT at the design pass.

**Documents.** **After part 1:** doc 09 §2.4 (`device_list`, `terminal_overview` as built; the export by kind), §2.2
(the `params` encoding of D9), §4.6; doc 17 §3.3 (`--kind`); doc 20 §3.1 (the new CSV files); the WP-S1.22
specification's non-goal and FR-EXT-003, FR-EXT-027 rows (XLSX with WP-S3.08, D3). **After part 2:** doc 09 §2.4
(`open_items` as built); doc 15 §2.3 and §7 (the open-item table as data, the categories); 02a FR-REP-021 (as-built
note: unwaived errors are the check report's, D7); arch §3 (`ew-reports` → `ew-checks`, D8). **After part 3:** doc 09
§2.3 (`LinkRef.item`, `LinkTarget::Rep`), §8.1 and doc 20 §21 (AC-13's link count), 02a FR-REP-042 (as built). **At
the next design pass:** WP-S3.03's stale status row (D1); WP-S1.18's specification status row (part 5 built);
`work-packages.csv` (O2).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S3.03, WP-S3.06, WP-S3.08, WP-S1.14, WP-S1.18, WP-S1.21, WP-S1.22,
  WP-S4.01, WP-S4.02; Appendix B (FT-46, FT-61, FT-69, FT-218, FT-231, FT-241, FT-242, FT-385, FT-667, FT-704).
- [Decision log](../06-decision-log.md): ID-131, ID-160, ID-333, ID-349, ID-356, ID-700, ID-779, ID-861.
- [Reports](../../design/09-reports-forms-output.md); [checks](../../design/15-checks-revisions-collaboration.md) §2.3,
  §4, §7; [PLC and bus](../../design/10-plc-and-bus.md) §5; [golden project](../../design/20-golden-project.md) §6.1,
  §9, §16, §18.1, §21; [extensibility](../../design/17-extensibility.md) §3.3;
  [architecture](../../design/04-architecture.md) §3.
- [ADR-0015](../../adr/0015-display-list-renderers-pdf.md), [ADR-0018](../../adr/0018-reports-forms-engine-and-machine-profiles.md),
  [ADR-0019](../../adr/0019-rule-engine-and-rule-packs.md).
