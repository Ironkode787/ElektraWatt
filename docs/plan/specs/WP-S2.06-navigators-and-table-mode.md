# WP-S2.06: navigators and table mode (closed specification)

**Status:** specified in cycle 30 (documents only) for three parts after the delivered first slice (FT-408 and FT-409,
cycle 27, ID-650): part 1 (the navigator framework, the Pages navigator on it with multi-selection and drag to
reorder, the Devices navigator, selection synchronisation, 2.2 pw, D4), part 2 (the nine further navigators and
potential and net highlighting, 1.05 pw, D3), part 3 (table mode: columns, cell editing, fill-down, pattern fill, paste
from Excel, 1.5 pw, D4); order 1 → 2 → 3; one independent Opus review per cycle (ID-667); **parts 1 and 2 delivered in
cycle 30** (§6.11, §7.5; decision log ID-709, ID-712, ID-713), part 3 next ·
**Date:** 2026-10-28 · **Work package:**
[WP-S2.06](../02-work-packages.md#wp-s206-navigators-and-table-mode) · **Decision log:** ID-681 to ID-689, the nine
entries of §15.3 in order (follow-ups FT-486 to FT-493, the eight of §15.4 in order) · **Part of:** ElektraWatt plan,
specifications

**Purpose.** This is the closed specification of the package that gives the MVP its navigators: virtualised trees and
tables of pages, devices, connections, terminal strips, cables, PLC channels, potentials, interruption points, structure
identifiers, parts in use and report blocks, each with a filter, multi-selection, a jump to the drawing, selection kept
in step with the canvas both ways, and a table mode in which a column is edited, filled down, pattern-filled or pasted
from Excel as one command. It is written against **WP-S2.03's, WP-S2.04's and WP-S2.05's delivered contracts** (§2.1),
not against their code. Where this document and a design document differ, this document is the narrower and wins for
the lanes; the design documents stay the target of the whole work package, and the documents named in §15.6 are updated
at the design pass after each part. A choice a lane meets that this document does not settle is escalated, never
guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [The decisions of this specification](#3-the-decisions-of-this-specification)
4. [Crates, boundaries and the public items below the shell](#4-crates-boundaries-and-the-public-items-below-the-shell)
5. [Commands](#5-commands)
6. [Part 1: the framework, the Pages and Devices navigators, synchronisation](#6-part-1-the-framework-the-pages-and-devices-navigators-synchronisation)
7. [Part 2: the further navigators and highlighting](#7-part-2-the-further-navigators-and-highlighting)
8. [Part 3: table mode](#8-part-3-table-mode)
9. [Determinism, the headless mode and exact counts](#9-determinism-the-headless-mode-and-exact-counts)
10. [Acceptance checks](#10-acceptance-checks)
11. [NFR-PERF-15 and the gates](#11-nfr-perf-15-and-the-gates)
12. [Files owned](#12-files-owned)
13. [Out of scope](#13-out-of-scope)
14. [Review checklist for the Opus reviewer](#14-review-checklist-for-the-opus-reviewer)
15. [Size, order, routing and the documents to update](#15-size-order-routing-and-the-documents-to-update)
16. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package every navigator of §7.1 can be opened from View ▸ Navigators, docks like the Pages
pane, shows its rows as a tree, a flat list or a table, filters them by text and facets, sorts them naturally, lets the
user select several rows, and opens the drawing at a row (Enter) or keeps the canvas selection in step without
switching the page. A selection made on the canvas reveals and marks its rows in every open navigator. The Pages
navigator reorders pages by drag or keys through one previewed command. Potentials and nets are highlighted on the
canvas with a colour and a hatch pattern. In table mode a cell, a column over several rows, a fill-down, a pattern fill
or a paste from Excel is one command and one undo step, through the edit policy of FR-UI-044. Everything runs headless
through `ew.uiscript/1` and reads back through the transcript and `ui.describe`, and the keyboard alone can do
everything (NFR-A11Y-01). The proofs are FR-UI-005, FR-UI-007, FR-UI-008, FR-UI-018 and FR-PAG-024's rest on Golden A
(§10); NFR-PERF-15 is shown structurally, its timings are not measured (owner's rule, ID-512; §11).

**Non-goals** (each is named with its package in §13):
- **No message panel.** The Messages entry of FR-UI-005 is WP-S2.09's message panel (FR-UI-021), built on this
  package's framework once part 1 is delivered (D4; FT-493).
- **No Unplaced devices navigator** (the panel-layout view of doc 16 §5.2): Golden A has no layout placement record
  before WP-S4.01, so the navigator would list every device (D4; FT-486).
- No domain table editors (strip editor, edit-cable table, PLC grid with drag-bind, part selection): WP-S2.07. No row
  insert, delete or move in a table, no Delete key in a tree, no F2 rename in a tree (FT-490).
- No bulk preview tab: WP-S2.09. The inline card of WP-S2.05 §6.6 stands in, inside the navigator pane.
- No library status of parts in use (FT-487), no finding-count badges and no `has:finding` facet (FT-488), no page
  thumbnail mode (FR-UI-005's "may"; FT-492), no persisted navigator state (FT-489), no signal and net levels in the
  potentials tree (FT-491).
- No change to any exported byte of Golden A, to AC-15's command log, to the record logs, to the project format or to
  the acceptance result; no new memoised family; the only new packages are those of D9.

## 2. Inputs

| Input | What the lanes take from it |
|---|---|
| [WP-S2.03's specification](WP-S2.03-viewer-and-tool-framework.md) §6 and §7 as built | The Pages navigator (`PageNavigator`, `NavRow`, node keys), the `pages.*` actions, the host's preview and commit path, `Selection`, the focused tab, jumps with back history, the transcript and describe rules, the exact-count counters (`SessionCalls`) |
| [WP-S2.04's specification](WP-S2.04-placement-wiring-drawing-tools.md) §7.2 as built | `place.pick` with `function` arms `existing_function` (the drag of an unplaced function); `PageTables` with the drawn connections |
| [WP-S2.05's specification](WP-S2.05-property-grid-find-replace-clipboard.md) §4.2, §6.1, §6.5, §6.6, §7.2 as built | The field catalogue (`field_catalogue`, `FieldType`, `EditRoute`, `identifier`), `parse_field_input`, `format_field_value`, `field_choices`, `fields.set` and `fields.set_many`, the policy, the card (`Card`, `CardOrigin`) and the toast |
| Code at 1892c87e | `crates/ew-viewmodel/src/{navigator.rs,describe.rs,actions.rs,layout.rs,properties.rs,shell/navigation.rs,tools/overlay.rs,tools/mod.rs}`; `crates/ew-ui/src/{navigator.rs,dock.rs,app.rs,canvas.rs}`; `crates/ew-api/src/output/{navigation.rs,fields.rs,properties.rs}`; `crates/ew-reports/src/{kind.rs,rowset.rs,queries.rs,kinds/*.rs}`; `crates/ew-commands/src/commands/page.rs`; `crates/ew-checks/src/engine.rs` |
| [Architecture](../../design/04-architecture.md) §3, §9; [ADR-0014](../../adr/0014-gui-toolkit-egui.md); [ADR-0008](../../adr/0008-transactions-undo-preview.md) | `ew-viewmodel` holds navigator trees and table rows with their edit → command mapping; egui only in `ew-ui`; virtualised to 100k rows; one command per table edit |
| [UX/UI](../../design/16-ux-ui.md) §5, §7.1 to §7.5, §10.1, §10.3, §15.2 to §15.4 | Shared behaviour, the catalogue, filter syntax, selection synchronisation, table mode, the edit → command table, pattern fill, paste from Excel, the policy, AccessKit, patterns plus colour, the algorithms |
| [Domain model](../../design/03-domain-model.md) §6 to §13 | Pages, devices, functions, representations, connections, potentials, interruption points, terminals, cables, PLC |
| [Golden project](../../design/20-golden-project.md) and `golden/J-E-2416/` | The oracles of §10: the tables, the report blocks 901 and 903 to 910, `expected/csv/*.csv` |
| Requirements | The 5 rows of the work package: FR-UI-005, FR-UI-007, FR-UI-008, FR-UI-018, NFR-PERF-15; with FR-PAG-024 (FT-360), FR-UI-036, FR-UI-044, FR-I18N-010, FR-I18N-011, FR-UI-025, NFR-A11Y-01 to NFR-A11Y-03, NFR-REL-07, NFR-PERF-19, NFR-PERF-20 where they touch it; FT-360, FT-362, FT-371's open navigator item and FT-461 |

### 2.1 What the earlier packages provide (the base contract)

The lanes rely on exactly these delivered items and on nothing of their code beyond them. If a delivered item differs,
the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| `PageNavigator` with `NavMode`, `NavSort`, the node keys and the six actions `pages.mode`, `pages.sort`, `pages.filter`, `pages.toggle`, `pages.row`, `pages.open`; its describe lines | S2.03 §6.1, doc 16 §5.2 as built | Migrated onto the framework with names, keys and lines unchanged (D5) |
| `Session::page_rows` (`PageRow`: folio, title, section, type, generated, QET grid) | S2.03 | The Pages table's rows |
| `Selection { items, primary, generation }` per window; `select.set` | S2.03 §7.6; S2.05 | Synchronisation (§6.5) |
| The host's preview and commit path, `Session::preview_with`, `commit`, `run_script` | S2.03 §7.3 | Every commit (D11) |
| `place.pick { function }` arming `existing_function` | S2.04 §7.2 | The drag of an unplaced function (§6.7) |
| `PageTables` (the drawn connections with their polylines) | S2.04 §4.3 | Highlight geometry (§7.3) |
| `field_catalogue`, `FieldType`, `EditRoute::FieldsSet`, `FieldSpec.identifier`, `Session::parse_field_input`, `format_field_value`, `field_choices` | S2.05 §4.2 | Editable cells (§8.2) |
| `fields.set`, `fields.set_many` (≤ 10,000 entries, one (target, path) once) | S2.05 §5 | The compiled commands (§8.3) |
| `Card`, `CardOrigin`, the stale-card rule, the toast | S2.05 §6.6 | The table and reorder cards (§6.6, §8.7) |
| The headless clipboard | S2.05 §8.7 | `table.paste` and `table.copy` headless (§8.6) |
| `ReportKindDef::rows(q, &ReportBlock)`, `RowSet`, `RowId`, the built-in kinds | `ew-reports` | The report-kind tables (D3) |

### 2.2 Gaps found below the shell (why the parts are shaped as they are)

| Gap at 1892c87e | Consequence |
|---|---|
| Only the Pages navigator exists; its model is page-specific | One generic framework (D5); Pages migrated onto it |
| No query lists devices, connections, strips, cables, channels, potentials, interruption points, structure nodes or parts as rows; the report kinds do, but only for a stored report block | `ew_reports::project_rows` over an unfiltered block, and `Session::navigator_table` (D2, D3) |
| `page.renumber` keeps the order of the pages; nothing reorders them | The new command `page.reorder` (D8) |
| The canvas overlay has no highlight item | `OverlayItem::Highlight` and `Session::highlight_members` (D14) |
| egui_table is named by ADR-0014 but not in the graph | Added at its egui 0.37 release (D9) |
| FR-UI-007's acceptance names folio 902 for -201U3; Golden A has no layout representation | D16 |

## 3. The decisions of this specification

Each is a lead ruling that the owner may reverse. The design pass records them as ID-681 to ID-689 (§15.3).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Three parts after the delivered slice.** Part 1 the framework, Pages and Devices, synchronisation and `page.reorder` (2.2 pw, D4); part 2 the nine further navigators and highlighting (1.05 pw, D3); part 3 table mode (1.5 pw, D4). Order 1 → 2 → 3. The parts and the delivered slice (0.45 pw) fill the PERT of 5.2 pw with **no reserve**; the de-scope rule of §15.1 replaces one | Each part is one lane-cycle; part 1 sets the contract that parts 2 and 3 and WP-S2.09 build on, part 2 is the same pattern nine times, part 3 is where commands are compiled from user input |
| D2 | **One row source per navigator: `Session::navigator_table(kind) -> NavTable`** in `ew-api`, a plain function over `Q` built on demand; the view model fetches it once per (kind, revision) **only for open navigators** and keeps it as an `Arc`. No memoised family | The view model cannot name `ew-model`; per-revision rebuild of open navigators only is the simplest correct cache; incremental tables wait for a measurement (FT-491) |
| D3 | **Where a report kind lists the rows, the navigator's table is that kind's rows**: wire list (Connections), terminal diagram per strip (Terminal strips), cable overview (Cables), potential overview (Potentials), PLC overview (PLC), parts summary in component mode (Parts in use), structure overview (Devices and Structure). They come from one new function `ew_reports::project_rows` over an unfiltered block; the navigator's columns are the kind's columns in its order. Pages, Interruption points and Report blocks have their own columns (§6.6, §7.1) | FR-UI-005 asks for the wire list as the Connections table; one derivation for the printed list and the navigator means the two never disagree, and Golden A's blocks are the oracle (§10) |
| D4 | **The navigator set at M-PP is eleven kinds** (§7.1). The **Messages** navigator is WP-S2.09's message panel (FR-UI-021) built on this framework, not a second findings view here; **Unplaced devices** (panel layout) moves to WP-S4.02, because no layout placement exists before WP-S4.01; the potentials tree is potential → connections (signals and nets later); parts in use carry no library status; no finding-count badges | Avoids two specifications of one panel; no navigator that cannot show true data; the deferred items are follow-ups (§15.4) |
| D5 | **One generic navigator framework; the Pages navigator migrated onto it** with its six `pages.*` actions, their parameters, its node keys, its describe lines (byte-equal while it shows at most 50 rows) and its AccessKit names unchanged; the other navigators use generic `navigator.*` actions with a `kind` parameter, which also accept `pages` | Committed recordings (WP-S6.13) replay unchanged; one widget, one state machine, one test set |
| D6 | **Filter:** a row matches when every query token is a prefix of one of the row's tokens, or the whole query is a substring of its label (both case-folded and NFC); facets from a closed list (§6.3); ancestors of a match stay shown and expand. **It always runs on the UI thread** over the precomputed tokens; doc 16 §15.2's job path above 20,000 nodes is not built | The union keeps the as-built Pages filter a subset (recordings); a linear scan of precomputed tokens is well inside 100 ms at 100k rows by estimate; the job path waits for the release-phase measurement (FT-491) |
| D7 | **Selection.** Each navigator keeps its own set of marked rows. Navigator → canvas: the window `Selection` becomes the marked rows' drawables on the shown page if there are any, else their records; the page is never switched, and a chip line names the drawables elsewhere. Canvas (or any other origin) → navigators: the rows of the selected records are marked and revealed, the row cursor moves to the first of them, and the focus stays. A generation guard stops loops. Enter opens a row's target in the focused tab as a jump (doc 16 §5.4, §15.3) | FR-UI-007 both ways with no page jump on selection; the property grid shows what the user picked in a navigator even off the page |
| D8 | **`page.reorder`**, a new command: the dragged page records are put before a page of the same section, and the folio keys (number, label) of the range of positions that changed are given to the pages of that range in their new order; pages outside the range keep theirs. Always previewed (cost `Bulk`), as the card in the Pages pane until WP-S2.09's tab | A permutation of the section's own folio keys cannot collide with any page outside the range and touches the fewest pages; the trade-off is that 106 dragged before 103 yields 106 → 103 and 103 → 106, not a shift to 104 (§5.1) |
| D9 | **Tables use egui_table `=0.11.0`** (the release for egui 0.37, MSRV 1.95, MIT OR Apache-2.0; it adds `egui_table` and `vec1 1.12.1`, both `MIT OR Apache-2.0`); **trees stay `ew-ui`'s own rows on `ScrollArea::show_rows`**, and egui_ltreeview is not adopted. ADR-0014 amendment 4 records it | egui_table gives sticky key columns, column resize and row virtualisation from the egui author's own crate; the as-built Pages tree already virtualises with AccessKit tree roles, so a tree crate would only add an upgrade |
| D10 | **Editable cells are a closed table** (§8.2): a column is editable only when it is listed there, its route is an `EditRoute::FieldsSet` of WP-S2.05's catalogue, and the route's `FieldType` is a scalar (Text, Ident, MlText, Bool, Int, Dec3, Quantity, Csa, Nm, Colour, Enum, or Optional of one); the targets of a cell are all records of the row of the route's kind. Every other cell is read-only with its reason | The table cannot do what the grid and a script cannot; group, list, link and JSON values stay in the property grid |
| D11 | **One gesture, one command.** A cell edit, a multi-row commit, a fill-down, a pattern fill, a paste or a clear compiles to `fields.set` when every cell has the same (path, value), else to one `fields.set_many`; the FR-UI-044 policy rules 1 to 3 are evaluated on its preview; a direct commit shows the toast, any other the card in the navigator pane | Doc 16 §7.2; one undo step; the same path as the property grid |
| D12 | **Pattern fill and paste are closed grammars** (§8.5, §8.6): the template language of doc 16 §7.3 with an exact grammar, and the TSV parser of doc 16 §15.4 with English headers only until WP-S2.08's locales; no row inserts at M-PP; every cell text goes through `parse_field_input` | Deterministic, testable, no second parser of field values |
| D13 | **Data safety in multi-cell edits:** a cell whose (record, path) an earlier cell of the same gesture sets to a different value is invalid ("same record as row <n>"), and an invalid or ambiguous cell keeps the card from applying until it is resolved or dropped; read-only and out-of-range cells are invalid too | A paste never writes one record twice or half-silently; the card shows every refusal |
| D14 | **Highlighting is view state, not model state**: up to four highlights per window, each a potential or a net, drawn on the canvas as a hatched band in an Okabe–Ito colour with a slot-specific hatch angle, composited into `canvas.snapshot`, listed by `ui.describe` | FR-UI-018 with pattern plus colour (FR-UI-025); no command, no undo entry, nothing saved |
| D15 | **The headless surface grows by 31 actions and 30 describe words** (§6.9, §6.10, §7.4, §8.8; ID-474 extended, a reviewed transcript change); all new lines sit under the `navigator` and `canvas` sections, so the blind harness's `look` needs no change; the describe text shows at most 50 rows per navigator (a window around the cursor) | The blind drafter reads every navigator as text at a bounded size; recordings and the harness stay valid |
| D16 | **Golden data that the requirement texts misstate.** FR-UI-007: -201U3 has representations on 201 (E4) and 203 (B4) only; folio 902 is a panel-layout page without layout representations until WP-S4.01. FR-UI-005: "W1–W44" are the golden table's wire keys, not a model field; the Connections navigator lists the wire list's rows, whose records cover every connection of kind wire or core. Doc 16 §5.2's message counts are not asserted here (D4). The design pass amends FR-UI-005's and FR-UI-007's acceptance texts | Lanes test against the data; a requirement example that the data does not support is a document fix, not a lane guess |

## 4. Crates, boundaries and the public items below the shell

### 4.1 Roles

| Crate | Layer | Part 1 | Part 2 | Part 3 |
|---|---|---|---|---|
| `ew-reports` | GPL | `project_rows` (with the structure overview) | the other kinds through `project_rows` (no new item) | — |
| `ew-api` | GPL | `output/navigators.rs`: `NavKind`, `NavTable` and its types, `navigator_table` for Pages and Devices | the nine further providers; `highlight_members` | — |
| `ew-commands` | GPL | `page.reorder` | — | — |
| `ew-viewmodel` | GPL | `navigators/` (state, flattening, filter, sort, selection, sync, the Pages migration, reorder card), panes, actions, transcript, describe | the providers' views and badges, `highlight.rs`, `OverlayItem::Highlight` | `navigators/table.rs`, `pattern.rs`, `paste.rs`, compile, the table card |
| `ew-ui` | GPL | the generic tree widget, drag and drop, the chip line, keys | the highlight painting | the table widget on egui_table |
| `ew-testkit` | GPL | DSL verb and fuzzer arm of `page.reorder` | — | a fuzzer arm for the TSV parser |
| `elektrawatt` | GPL | tests only | tests only | tests only |

**Rules the lanes keep** (the earlier packages' rules stand: through `Session` only, `check-raw-read` green, no
`unsafe`, no unordered output, one clock, `ew-render2d` names no egui type):
1. **Only the public items of §4.2 and the command of §5** are added below the shell, each first and in its own commit
   with tests. Any other missing item stops the lane.
2. **New packages, exactly:** part 3 adds `egui_table =0.11.0` (default features off) to `ew-ui`, which brings `vec1
   1.12.1`; `cargo tree -p elektrawatt` grows by exactly these two (stated in the commit message), and the lane adds
   their cargo-vet exemptions at `safe-to-deploy` (ID-447's rule: the lane writes no audit) and runs `cargo deny` and
   `cargo vet`. A third new package, a new licence or a build script stops the lane. Parts 1 and 2 add no package. **No
   new workspace edge**: `ew-api → ew-reports` exists. `cargo xtask check-deps` green.
3. **One mutation path.** Every model change is a registered command committed by the host path (preview, then commit
   or the card). Highlights, filters, sorts, columns, marks and expansion are view state and never a command.
4. **No transaction reads `Q`.** `page.reorder` reads rows through `TxBuilder` only.
5. **Golden A's outputs do not change**; no file under `golden/` changes; the existing golden, acceptance and AC-15
   tests pass unmodified. `project_rows` must not change `rows_of` for any stored block.
6. **No `ew-model` name in `ew-viewmodel` or `ew-ui`**; no egui name outside `ew-ui` and `elektrawatt`.

### 4.2 The public items added below the shell

| Part | Crate | Item | Contract |
|---|---|---|---|
| 1 | `ew-reports` | `pub fn project_rows(q: &Q, kind: &str, include: Option<Uuid>) -> Option<RowSet>` | The rows of the built-in kind `kind` for a block of that kind with no selection filter, no manual order, no grouping and no page limits (the kind's default order), and for `terminal_diagram` the strip `include`; `None` for an unknown kind or a `terminal_diagram` without `include`. **Agreement:** on Golden A it equals `rows_of(q, block)` for the blocks 901 (in component mode), 903 to 910 (903 to 905 with their strip); a difference stops the lane |
| 1 | `ew-api` | `pub enum NavKind { Pages, Devices, Connections, Strips, Cables, Plc, Potentials, Ips, Structure, Parts, Blocks }` with `ALL`, `word()` (`pages`, `devices`, `connections`, `strips`, `cables`, `plc`, `potentials`, `ips`, `structure`, `parts`, `blocks`) and `from_word` | The eleven navigators in this menu order |
| 1 | `ew-api` | `pub enum NavRole { Section, Function, Location, Page, Device, Representation, Connection, Potential, Strip, Terminal, Cable, Conductor, Station, Module, Channel, Name, Ip, StructureNode, Manufacturer, Part, ReportBlock }` with `word()` (`section`, `function`, `location`, `page`, `device`, `representation`, `connection`, `potential`, `strip`, `terminal`, `cable`, `conductor`, `station`, `module`, `channel`, `name`, `ip`, `structure_node`, `manufacturer`, `part`, `report_block`) | What a row is; the describe word and the accessible kind |
| 1 | `ew-api` | `pub struct NavTable { pub kind: NavKind, pub rev: u64, pub columns: Vec<NavColumn>, pub nodes: Vec<NavNode>, pub index: BTreeMap<Uuid, Vec<u32>> }`; `pub struct NavColumn { pub key: String, pub label: String, pub derived: bool, pub key_column: bool, pub route: Option<(String, String)>, pub read_only: Option<String> }`; `pub struct NavNode { pub key: String, pub parent: Option<u32>, pub role: NavRole, pub label: String, pub badges: Vec<String>, pub records: Vec<Uuid>, pub drawables: Vec<Drawable>, pub open: Option<NavOpen>, pub cells: Vec<String>, pub table_row: bool, pub tokens: Vec<String>, pub facets: Vec<(String, String)>, pub natural: Vec<NaturalKey> }` (the label's key first, then one per cell; `ew-api` re-exports `ew_mltext::NaturalKey`, so the view model sorts without naming `ew-mltext`); `pub struct Drawable { pub page: Uuid, pub folio: String, pub cell: String, pub record: Uuid, pub main: bool }`; `pub struct NavOpen { pub page: Uuid, pub select: Vec<Uuid> }` | §6.1. `nodes` in tree order (a parent before its children, siblings in the provider's default order); `cells` aligned with `columns` for `table_row` nodes, empty otherwise; `index` maps every record and drawable ID to the nodes that carry it, in node order. `route` is `(kind, path)` of §8.2's table, `read_only` the reason otherwise. Deterministic; reads memoised queries and `project_rows` only |
| 1 | `ew-api` | `pub fn Session::navigator_table(&self, kind: NavKind) -> NavTable` | Part 1 implements `Pages` and `Devices`; the other kinds return an empty table with `columns` and no nodes until part 2 |
| 2 | `ew-api` | `pub enum HighlightTarget { Potential(Uuid), Net(Uuid) }`; `pub fn Session::highlight_members(&self, target: &HighlightTarget) -> Vec<Uuid>` | §7.3: the connection IDs of the potential (named by a potential overview row's record) or of the net of the given connection, sorted; empty for an unknown ID |

No other item below the shell is added. The view model's types (`Navigator`, `TableState`, `Highlight`) are
`ew-viewmodel` items and free to shape within §6 to §8.

## 5. Commands

### 5.1 `page.reorder` (part 1)

Exposure `Edit`, stability `Preview`, undoable as one step, `params_version` 1, `deny_unknown_fields`, cost `Bulk`
(always previewed). The lane delivers, in its command commit, the `Command` with its doc comment, registration, the
committed schema, the name in `tests/params.rs`, the generated reference page and its `SUMMARY.md` line (`cargo xtask
docs-gen`, then `--check` green), a **DSL verb**, a **fuzzer arm** and unit tests for every rejection. The registered
count goes from 54 to **55**.

| Parameter | Type | Meaning |
|---|---|---|
| `pages` | `[IdS]`, 1 to 1,000 | The page records to move, in any order (taken in page order) |
| `before` | `IdS` or `null` | The page record they are put before; `null`: after the last page of the section |
| `section` | `IdS` or `null`; given exactly when `before` is `null` | The section at whose end the pages go (`null`: front matter); with `before` given, the section is `before`'s and `section` is omitted |

**Semantics.** Let S be the page records of the section (the section of `before`, or `section`) in page order (section
number, folio number, label in natural order, ID: `page.renumber`'s order). O is S without the moved pages, with the
moved pages (in their order in S) inserted before `before`, or at the end. Let i be the first and j the last position
where O and S differ. For every k in i..=j the page O[k] gets the folio key (number, label) that S[k] had. Pages
outside i..=j are unchanged. References are by ID, so tags, xref texts and the table of contents follow (as for
`page.renumber`). Outcome: no primary; one notice per changed page `"<old> → <new>"` in position order.

**Example (Golden A, section 1 = 101, 102, 103, 106):** `pages = [106]`, `before = 103`: O = 101, 102, 106, 103; i = 2,
j = 3; the old 106 becomes 103 and the old 103 becomes 106; 101 and 102 are untouched.

**Rejected** (each at its JSON pointer): empty `pages`; a page listed twice; an unknown page; `before` among `pages`;
a page of `pages` in another section than `before` (or than `section`), with "move pages between sections by changing
their section first"; O equal to S ("the pages are already in this order"); more than 1,000 pages.

### 5.2 Commands used as built

`fields.set` and `fields.set_many` (part 3), `place.pick` (WP-S2.04, as the drag target of part 1), `page.new`,
`structure.new` (FT-408, FT-409; pre-filled by §6.6 and §7.1). No command other than §5.1 is added or amended.

## 6. Part 1: the framework, the Pages and Devices navigators, synchronisation

### 6.1 The table contract

Every provider builds a `NavTable` (§4.2) by these rules:
1. **Keys** are path strings, unique in the table, stable across revisions for the same records and the same tree:
   `<parent key>/<role word>:<id or text>` (no leading `/` at the top level), where a record node uses its UUID text
   and a group node its label (`function:=P01/location:+J-E-2416/device:<uuid>`). The Pages keys stay as built
   (`s:<n>`, `f:<text>`, `p:<uuid>` …).
2. **Labels** are what the drawing prints: tags by `Tag::short` (the derived tag), folios as printed, representation
   rows `<folio>-<cell>` of the representation's anchor cell (the cell cross-reference texts use), function rows
   `<function text or the FnDef name>` with the word `main` or `aux` as a badge, MlText in the project's source
   language (WP-S2.05's rule).
3. **Badges** are fixed English texts: `unplaced` (a function without representation; a device whose functions are
   all unplaced), `frozen` (a frozen tag), `spare` (state spare), `generated`, `QET grid`, `main`, `aux`, `external`
   (an external interruption point).
4. **Tokens:** the label and every cell, case-folded and NFC, split at whitespace, at `- . _ : / + = ( ) ,` and at
   every letter/digit boundary, empty pieces dropped, deduplicated, sorted. **Natural keys:** `ew_mltext::natural_key`
   of the label and of each cell.
5. **Facets** (closed list): `page:<folio>` (the folios of the node's drawables, or its page), `potential:<name>`,
   `state:<used|spare|reserved|not_fitted>`, `kind:<role word>`, `part:<part number>`, and the flags `unplaced` and
   `frozen` as `(word, "")`.
6. **Records and drawables:** `records` are the IDs the row stands for (§6.7, §7.1); `drawables` are the
   representations and connections that draw them, main first, then page order, cell row, cell column, ID. The
   expansion: device → the representations of its functions; function → its representations; representation and
   connection → themselves; terminal and cable devices → as a device; conductor → its core connections; interruption
   point → its representation; every other record → none.
7. **Open target:** the page of the first drawable with the node's drawables on that page to select; for a page row
   the page with nothing selected; for a potential the first folio of its `pages` cell with nothing selected; for a
   channel the page of its `page` cell; for a report block its first generated page; none otherwise.

### 6.2 The navigator state and the visible rows

`ew-viewmodel` keeps per open navigator: `view` (`tree` or `flat`; the Pages navigator keeps `flat`, `section`,
`structure`), `table: bool` (part 3), `sort` (the Pages orders as built; others: by label or, in table mode, by a column,
ascending or descending), `filter`, the collapsed keys (every node starts expanded), the marked keys, the row cursor,
and the cached `Arc<NavTable>` with its revision. The **visible rows** are a depth-first walk over the expanded nodes
(`flat`: the `table_row` nodes, or for Devices the device nodes, one level), rebuilt only when the table, the view, the
sort, the filter or an expansion changes, and cached per (table revision, those inputs) (FT-371's cache, generalised).
Siblings sort by the natural key of the label (or the column cell), ties by node order. The widget paints only the rows
in view.

### 6.3 Filter

As D6. A query token of the form `<facet>:<value>` with a facet of §6.1 item 5 matches a node with that facet and a
case-folded equal value; `unplaced` and `frozen` alone match the flag; any other token with a colon is a plain token.
Plain tokens are split like node tokens (§6.1 item 4); every resulting token must prefix one node token, or the whole
case-folded query must be a substring of the case-folded label. Matches keep their ancestors, shown and expanded. An
empty filter shows everything. The Pages navigator's matches are a superset of its as-built matches (a test compares
both on every folio and title substring of Golden A).

### 6.4 Keys, mouse and type-ahead

Navigator context, as built for Pages and now for all: Up and Down move the cursor; Shift+Up and Shift+Down extend the
marked range; Left collapses or goes to the parent, Right expands; Home and End; Space toggles the cursor row's mark;
Enter opens (a jump in the focused tab); Ctrl+Enter is mapped by the widget, not by a registry shortcut, to
`navigator.apply` when the pane shows a card, to `table.commit { all: true }` while a cell is being edited (part 3), and
otherwise to opening in a new tab (as built for Pages). In the tree and flat views, typing a printable character other
than Space, without Ctrl or Alt, appends it to a type-ahead buffer (reset after 1 s of no typing, measured on the
window's clock and never in the transcript) and moves the cursor to the next visible row whose label starts with the
buffer, case-folded; headless `navigator.jump` does the same without a clock. In table mode the same keys start editing
the cursor cell instead (§8.3). No navigator shortcut is a bare printable key other than Space. A click marks one row and moves the
cursor, Ctrl+click toggles, Shift+click marks the range in visible order, a double click opens, a middle click opens
in a new tab.

### 6.5 Selection synchronisation (D7)

```text
on navigator marks changed (navigator N):
  rows = marked rows of N in visible order; recs = their records, deduplicated in that order
  D = their drawables on the shown page (the focused tab's page), deduplicated in order
  if D ≠ ∅: Selection := D (primary: first);  else: Selection := recs (primary: first)
  elsewhere := drawables of rows not on the shown page, main first, as "<folio>-<cell>", at most 12, if D = ∅ and recs ≠ ∅
  N.applied := Selection.generation
on Selection changed with generation g, origin not N (canvas, another navigator, select.set, undo):
  for every open navigator M with M.applied ≠ g:
    M.marked := nodes of M.index for the selection's items (the deepest node per item, in node order)
    expand their ancestors; cursor := the first of them in visible order; scroll it into view; focus unchanged
    M.applied := g
on a page switch by any origin: elsewhere := none; the Pages navigator's cursor := the current page's row
```

The **chip line** is drawn above the canvas ("Not on this page: 201-E4, 203-B4"), each chip a button that opens that
page and selects the drawable there. The Pages navigator's cursor following a page switch closes FT-371's open navigator
item.

### 6.6 The Pages navigator on the framework, multi-selection and reorder

The Pages provider builds its tree as built (S2.03 §6.1) from `page_rows`, and its table rows are the page rows with the
columns `folio` (key column), `title`, `type`, `frame`, `generated`. Multi-selection follows §6.4. A page row's records
are the page record (none for a generated page); marking pages selects nothing on the canvas and sets `Selection` to the
pages (the property grid shows them).

**Reorder.** In the flat and section views a page record row is a drag source; dropping it, or the marked page records,
between two rows of the same section dispatches `pages.reorder { pages, before }` (the row below the drop line, or the
section's end). Alt+Up and Alt+Down move the marked page records one position up or down within their section (the
`before` of that position). Generated pages are not draggable and refuse `pages.reorder` with "a generated page has no
folio of its own". `pages.reorder` builds `page.reorder` and always opens the card (`CardOrigin::Reorder`) in the Pages
pane: "Preview: reorder pages · 106 → 103, 103 → 106 · <n> tags, <m> cross-reference texts", Apply (Ctrl+Enter or
`navigator.apply`) commits, Escape cancels; the stale-card rule of WP-S2.05 applies.

**FT-461 (pre-fill).** Page ▸ New page and the Pages navigator's New page fill the Folio field with the proposal: if the
current page is a page record with number k, the smallest integer n > k whose text is not a folio of `page_labels`;
otherwise empty with the placeholder. The section field takes the section of the marked or cursor section node, else the
current page's section.

### 6.7 The Devices navigator

**Tree** (`tree`, the default): function nodes (`=`), location nodes inside them, then the devices that are not children
of another device, in natural tag order; a device's child devices (by `parent`, in `position` order, then natural tag)
under it; under each device its functions (main first, then by FnDef name, then ID) and under each function its
representations. Terminal devices are not listed (they are the Terminal strips navigator's rows, as in the structure
overview). **Flat:** the device nodes, one level. **Table rows:** the device rows of `project_rows(q,
"structure_overview")` (its structure-node rows belong to the Structure navigator), columns as that kind. **Records:** a
device row its device; a function row its function; a representation row its representation. **Badges:** §6.1 item 3.
An **unplaced function** row is a drag source onto a canvas: the drop (or `Enter` on a canvas after arming) dispatches
`place.pick { function: <id> }` (WP-S2.04 §7.2), and WP-S2.04's Place tool takes it from there.

**Golden A:** -201U3's row has two representation rows, `201-E4` and `203-B4` (D16). Marking it while folio 102 is shown
leaves the canvas selection empty on 102 and shows `elsewhere "201-E4" "203-B4"`; Enter opens 201 with its
representation selected.

### 6.8 Panes, menu and accessibility

`PaneKind` gains `Navigator(NavKind)` for the ten kinds other than Pages (the workspace key is the kind word; `pages`
stays `PaneKind::Pages`). The default workspaces do not change. View ▸ Navigators lists the eleven navigators as checked
items (`navigator.show`, one item per kind, checked when open); showing one docks it as a tab beside the Pages pane (a
new tab in the left dock) and focuses it; Pages stays where it is. `Focus::Navigator` carries the kind; the transcript's
`focus` member is `"navigator"` for Pages (unchanged) and `"navigator.<kind>"` for the others.

AccessKit: the tree is a `Tree`, each row a `TreeItem` named `<label>, <role word>[, <badges>]` with level, position, set
size, expanded state and selected state as AccessKit properties (as built for Pages); the chip line's chips are buttons
named `Open <folio>-<cell>`. The accessible-name walker passes on every new pane.

### 6.9 Actions of part 1 (14 new; 201 → 215)

| ID | Kind | Label | Menu | Shortcut | Context | Params |
|---|---|---|---|---|---|---|
| `navigator.show` | View | Navigator | View ▸ Navigators (one item per kind) | — | global | `kind:enum(pages ¦ devices ¦ …), on?:bool` (needs-parameters) |
| `navigator.view` | View | Navigator view | — | — | global, hidden | `kind, view:text` |
| `navigator.sort` | View | Navigator order | — | — | global, hidden | `kind, by:text, descending?:bool` |
| `navigator.filter` | View | Filter navigator | — | — | global, hidden | `kind, text:text` |
| `navigator.toggle` | View | Expand or collapse | — | — | navigator, hidden | `kind, key:text` |
| `navigator.row` | View | Move in the navigator | — | — | navigator, hidden | `kind, key?:text, step?:int, to?:enum(first ¦ last), extend?:bool` |
| `navigator.open` | View | Open row | — | — | navigator, hidden | `kind, key?:text, new_tab?:bool` |
| `navigator.select` | View | Mark rows | — | — | global, hidden | `kind, keys:list(text), add?:bool` |
| `navigator.mark` | View | Toggle mark | — | Space | navigator, hidden | `kind?, key?:text` |
| `navigator.jump` | View | Jump to row | — | — | navigator, hidden | `kind, prefix:text` |
| `navigator.apply` | Command | Apply the preview | — | — (Ctrl+Enter through the widget, §6.4) | navigator, hidden | `` |
| `pages.reorder` | Command | Reorder pages | — | — | navigator, hidden | `pages?:list(text), before?:text, end?:bool` (folios; default the marked page records) |
| `pages.move_up` | Command | Move page up | — | Alt+Arrow Up | navigator | `` |
| `pages.move_down` | Command | Move page down | — | Alt+Arrow Down | navigator | `` |

The six `pages.*` actions keep their IDs and parameters; `navigator.*` with `kind=pages` does the same as them. In the
table, `¦` stands for the separator of the enum list. `kind` omitted means the focused navigator; a hidden action with
an unknown kind, view, column or key refuses with the valid values listed. If the keymap test reports a conflict for
Space, Alt+Arrow Up or Alt+Arrow Down in the navigator context, the lane stops and reports.

### 6.10 Transcript and describe of part 1

The Pages lines are unchanged while it shows at most 50 rows. Every other open navigator prints:

```text
navigator "Devices" kind devices view tree sort "Tag" ascending filter "" rows 245 showing 1 to 50
  row level 1 function "=P01" expanded
  row level 3 device "-201U3" badge "unplaced" selected cursor
  row level 5 representation "201-E4" badge "main"
```

A row line is `row level <n> <role word> "<label>"`, then each badge as `badge "<text>"`, then `expanded` or
`collapsed` for a node with children, then `selected` when marked, then `cursor`. **The window:** at most 50 rows,
`start = clamp(cursor index − 10, 0, max(0, n − 50))`; the header's `showing <a> to <b>` (1-based) is printed for
non-Pages navigators always and for Pages only when it has more than 50 rows. The canvas section gains `  elsewhere
"<folio>-<cell>" …` while §6.5's chip line is shown. The card lines are WP-S2.05's with the origin word `reorder`.

**New describe words (18):** `kind`, `pages`, `devices`, `strips`, `cables`, `plc`, `potentials`, `ips`, `parts`,
`blocks`, `tree`, `ascending`, `descending`, `showing`, `to`, `elsewhere`, `aux`, `reorder` (267 → 285).

### 6.11 As built in cycle 30 (part 1)

**WP-S2.06 part 1 delivered in cycle 30 (lane H30; decision log ID-709).** NV1 to NV12 pass; 55 commands, 215 actions
and 285 describe words. Built as §6 says, with these points where the code settles what the text left open:

- **Tables.** `Session::navigator_table` builds Pages (the section tree as built, keys `s:<n>` and `s:front` › `…/p:<uuid>`;
  columns folio, title, type, frame, generated) and Devices (function `=` › location `+` › devices not under a listed
  parent in natural tag order › child devices by position, then tag › functions labelled by their FnDef key with `main`
  or `aux` › representations `<folio>-<cell>`). The other nine kinds return their columns and no nodes until part 2.
  `ew_reports::project_rows` equals the stored blocks 901 and 903 to 910 on Golden A (NV1, in `ew-api`'s
  `tests/reports_golden.rs`, because `ew-reports` cannot load Golden A without a new dev edge).
- **Framework.** `navigators::Navigator` in the view model holds the view (tree or flat), the sort (label or a column,
  ascending or descending), the filter, the collapsed and marked keys, the cursor, the Shift anchor, the `Arc<NavTable>`
  and the applied selection generation; the visible rows are cached per (table, view, sort, filter, expansion). The
  Pages navigator reads `page_rows` once per revision as built, so `navigator_table(Pages)` is used by part 3's table
  only, and §9 item 4's counts cover the other navigators.
- **Filter.** Parts `<facet>:<value>` of the closed list and the flags `unplaced` and `frozen` are conditions; the rest is
  plain text whose tokens must each prefix a node token, in any order (`-12k4` also matches -49K12), or whose case-folded
  text is a substring of the label. Pages uses the same filter over `<folio> <title>`. `ew-api` re-exports
  `nav_tokens` (so a query is split as node tokens are), `nav_natural_key` and `Uuid`.
- **Selection.** Marking selects the drawables on the shown page, else the records. The chips (at most 12, main
  first) are cleared by a selection or page change. Every open navigator follows a foreign selection generation once
  (the deepest node per record, ancestors expanded, the cursor on the first). A page switch puts the Pages cursor on the
  current page (FT-371's navigator item).
- **Reorder.** `pages.reorder {pages?, before?, end?}` (default: the marked page records, else the cursor's page)
  previews `page.reorder` into a `CardOrigin::Reorder { notices }` card in the Pages pane; Apply is `navigator.apply`,
  Ctrl+Enter or `preview.apply`, Escape cancels, a stale card previews again; `pages.move_up` and `pages.move_down`
  (Alt+Up, Alt+Down) move by one within the section. `page.reorder`'s `section` is an `Option`: without `before` it
  names the section (null or absent is the front matter), together with `before` it is refused at `/section`, and an
  order that does not change is refused at `/pages`. The card's describe line reads `card ready "page.reorder" targets
  1 changes 2 tags N xrefs M reorder "106 → 103" "103 → 106"`.
- **New page (FT-461).** It proposes the next free integer after the current page record and the section of the marked
  or cursor section node, else of the current page.
- **Window.** View ▸ Navigators lists the 11 kinds as checked items; a shown navigator docks as a tab beside Pages
  (`PaneKind::Navigator(kind)`, workspace key = the kind word). Dock tab groups are not draggable as a whole (only
  panes), so the tab bar is not an unnamed drag area.
- **Widget.** One generic tree widget: `Tree` and `TreeItem` with level, position, size, expanded and selected (for
  Pages, `selected` is now the mark, not the cursor); type-ahead with a 1 s reset; a click marks and a double click or
  Enter opens (Pages included); a click does not scroll the list, which scrolls only when the cursor leaves the rows on
  show; drag of page rows (on a page row: before it; on a section node: to its end) and of unplaced function rows onto
  the canvas (`place.pick {function}`, which now works without `symbol`: the first allowed catalogue symbol whose FnDef
  key equals the row's label). The chip line sits above the canvas area, with buttons named `Open <folio>-<cell>`.
- **Focus and describe.** `Focus::Navigator(NavKind)` for every navigator; `ui.focus` gains `kind?`; the describe focus
  line for the other kinds reads `focus navigator kind <word>`, the transcript `navigator.<kind>`. Of the 18 new words,
  `aux` (badges print quoted) and `pages` (the Pages header omits `kind`) cannot appear structurally; they stay in
  `VOCABULARY` as specified and the reach test excludes them with that reason. Devices' function rows order by FnDef
  name under the default label sort; the sort label reads Tag for Devices and Label for the others; every column has
  `derived = false` until part 3 (§8.2). -201U3 sits at level 4, under -201U1.
- **Headless surface.** 14 actions, 18 words.

## 7. Part 2: the further navigators and highlighting

### 7.1 The providers

| Kind (title) | Tree (`tree` view) | Table rows and columns | Records of a table row | Golden A oracle |
|---|---|---|---|---|
| `connections` (Connections) | potential (the row's `potential` cell, natural; empty → `(none)`) → the physical-wire rows; a `Bridges` group → the bridge rows | `project_rows(q, "wire_list")`: the physical-wire rows and, after the "Bridges" heading (a group node), the bridge rows; the kind's columns | the connections of the physical wire (through `PhysicalWires`), the bridge connections of a bridge row | block 907's rows; the records of the data rows cover every connection of kind wire or core exactly once (the 66 that carry W1 to W44) |
| `strips` (Terminal strips) | strip (natural tag) → terminal rows | for each strip in natural tag order, `project_rows(q, "terminal_diagram", Some(strip))`; a leading column `strip` (key column), then the kind's columns | the terminal device | blocks 903 to 905; 1X1 3, -X1031 20 (12 spare), -X1032 5 terminals |
| `cables` (Cables) | cable (the cable heading rows) → conductor rows | `project_rows(q, "cable_overview_diagrams")`: the per-cable data rows as cable rows and the core rows as conductor rows; the "Cable overview" heading is dropped | the cable device; for a core row also the conductor | block 906's rows: 3 cables, 13 cores |
| `plc` (PLC) | station (the module device's parent with the station aspect, else `(none)`) → module (natural tag) → channel rows | `project_rows(q, "plc_overview")` | the channel and its module device | block 908's rows: station -201U1, 7 modules, 56 channels |
| `potentials` (Potentials) | potential rows → the Connections navigator's physical-wire rows whose `potential` cell names it (the bond row has none) | `project_rows(q, "potential_overview")` | the potential definition or the CP that names it; the bond row its bond | block 909's rows: 19 potentials and bond B1 |
| `ips` (Interruption points) | name (`ip_name`, natural) → point rows | one row per interruption point: columns `name` (key), `page_cell`, `direction`, `partner` (the paired point's page-cell, or `external`), `external` (yes or no), from `IpPairingQ` | the interruption-point function | 26 names: 22 pairs and 4 external points (48 functions) |
| `structure` (Structure identifiers) | aspect (`function`, `location`, … in the aspect's declaration order) → node rows (natural designation, child nodes nested by `parent`) | the structure-node rows of `project_rows(q, "structure_overview")` plus a column `usage` (the number of devices and pages that name the node) | the structure node | =P01, +FIELD, +J-E-206, +J-E-2416 (alias JE2416) |
| `parts` (Parts in use) | manufacturer → part (BOM row) → device rows | `project_rows(q, "parts_summary")` in component mode | the devices that the BOM row counts | block 901's rows in component mode: the 27 rows of `expected/csv/bom_parts.csv`; 2904602 → -102G1 |
| `blocks` (Report blocks) | section (number, front matter first) → block rows | one row per report block: `title` (key), `kind`, `first_folio`, `section`, `pages` (the number of pages it generates) | the report block | 15 blocks: 11 page blocks (2, 901, 903 to 911) and 4 embedded (cover tables, part data on 201, 202, 902) |

Every provider's view `flat` lists its table rows, one level. Column labels are the report kind's labels; the columns
this document adds are labelled by their key with the first letter in capitals and `_` read as a space (`page_cell` →
"Page cell", `first_folio` → "First folio"), and the Pages columns "Folio", "Title", "Type", "Frame", "Generated". The **structure navigator's New node** (FT-409's form)
is pre-filled with the marked structure node as parent when its aspect equals the chosen kind's aspect (FT-461's second
half). A data row of a report kind without records (a heading) is a group node, never a table row.

### 7.2 Badges of part 2

Terminals: `spare` (state spare); the strip row: `<n> spare` when n > 0. Potentials: none. Interruption points:
`external`. Connections: none. Channels: `spare` when the channel's state cell is spare. No other badge.

### 7.3 Highlighting (D14; FR-UI-018, FT-362)

`navigator.highlight` (Ctrl+Shift+H in the navigator context) on a potential row highlights `Potential(<its record>)`; on a
connection row (in Connections or Potentials) `Net(<its first connection>)`; on any other row it refuses with "only
potentials and connections can be highlighted". Pressing it on a highlighted target removes that highlight; a fifth
highlight replaces the oldest. View ▸ Clear highlights removes all. Highlights belong to the window and survive page
switches and revisions (their members are re-read once per revision through `highlight_members`).

**Members** (`Session::highlight_members`): for `Potential(id)` the connections whose potential (by
`potential_partition`) is the one named by `id`; for `Net(c)` the connections of the net that contains connection `c`
(by the derivation's nets). Sorted by ID.

**Drawing.** `OverlayItem::Highlight { polylines, slot }` per highlight on the shown page, from the members' drawn
polylines in `PageTables`. Slot n (0 to 3) has the colour of the Okabe–Ito set (orange `#E69F00`, sky blue `#56B4E9`,
bluish green `#009E73`, vermillion `#D55E00`) and the hatch angle 45°, 135°, 0°, 90°: each segment is drawn as a band 8
screen pixels wide at 30 % opacity of the colour, crossed by 2-pixel hatch lines in the full colour every 6 pixels at
the slot's angle (computed in `ew-viewmodel` as screen-space line segments clipped to the band, painted by `ew-ui`
through the existing line primitive). The highlight is composited into `canvas.snapshot` like `Ghost`. The theme's
contrast test (doc 16 §10.3) covers the four colours on the white sheet.

### 7.4 Actions, transcript and describe of part 2 (2 new; 215 → 217)

| ID | Kind | Label | Menu | Shortcut | Context | Params |
|---|---|---|---|---|---|---|
| `navigator.highlight` | View | Highlight | — | Ctrl+Shift+H | navigator | `kind?, key?:text, off?:bool` |
| `view.clear_highlights` | View | Clear highlights | View | — | global | `` |

The provider rows print as §6.10 with their role words. The canvas section gains, per highlight on the shown page,
`  highlight slot <n> potential "<name>" connections <k>` or `  highlight slot <n> connection "<wire number or
label>" connections <k>`; a highlighted row's line ends with `highlight <n>`.

**New describe words (6):** `cable`, `module`, `station`, `manufacturer`, `highlight`, `slot` (285 → 291).

### 7.5 As built in cycle 30 (part 2)

**WP-S2.06 part 2 delivered in cycle 30 (lane N30; decision log ID-712, ID-713).** NP1 to NP6 pass; 55 commands, 217
actions and 291 describe words. Built as §7 says, with these points:

- **Providers.** Connections, Terminal strips, Cables, PLC, Potentials, Interruption points, Structure identifiers, Parts
  in use and Report blocks read their rows through `ew_reports::project_rows`, and every one equals its stored block on
  Golden A. A connection's drawable is the cell of its first end that is a CP drawn on the page; an unnumbered wire
  row reads `<from> <cp> → <to> <cp>`; bridges sit under a Section-role group "Bridges"; a potential's children include
  both sides of a `⇄` cell. Structure aspect groups use the Function and Location roles (StructureNode for other
  aspects), and aliases are filter tokens. A PLC channel's label is its address, else its number, and its open target
  is the page of its representation, with nothing selected. An embedded report block shows its host page as first
  folio, a page count of 0 and no open target; a block's section is the section of its first page.
- **NP2 (N30-1).** Of the 72 wire and core connections every one is listed at least once, and only junction
  (star-feed) arms are shared by two physical wires, among them the numbered W20 and W24; FR-UI-005's acceptance reads
  "at least once; shared only by junction arms".
- **Highlighting.** `HighlightTarget { Potential(Uuid), Net(Uuid) }` and `Session::highlight_members` (sorted,
  deduplicated, empty for an unknown ID). Four slots with #E69F00, #56B4E9, #009E73 and #D55E00 and the hatch angles
  45°, 135°, 0° and 90°; an 8 pt band, a translucent fill at alpha 77, hatch lines 2 pt wide at a 6 pt pitch, sizes in
  points × ppp with square caps, one global hatch-line family, the angle in screen coordinates; a fifth highlight
  replaces the oldest; members are refreshed once per revision; `paint_at` is the one geometry source of the window and
  `canvas.snapshot`. A net is the page-local net that holds the connection; a potential's members are the connections
  with either end on it; the bond row and group rows cannot be highlighted; `Net(first record)` takes the smallest
  UUID. Highlights are painted above the overlays and below the statistics panel and the preview chips. Slots 0 and 1
  are below 3:1 on white (FT-506).
- **Actions and words.** `navigator.highlight` (`kind?`, `key?`, `off?`; Ctrl+Shift+H; in the navigator) and
  `view.clear_highlights` (View menu, disabled without highlights); strings `reason-no-highlights` and
  `error-highlight-row`; the six words of §7.4.
- **FT-461.** New structure node pre-fills the parent from the marked Structure node, else the cursor's, in the GUI
  only; headless, a missing parent parameter still fails.
- **NP4.** A click on W29's route picks W30, or nothing on a repeat, independent of the navigators (N30-3, FT-500);
  the test clicks, asserts the picked wire's rows, then selects W29 by `select.set`.

## 8. Part 3: table mode

### 8.1 Columns and the table widget

`navigator.table` switches a navigator between its tree or flat view and its table (the header carries a Table toggle).
The table shows the `table_row` nodes that pass the filter, sorted by the sort column (natural, ascending or
descending; ties by node order); the key column is frozen at the left; derived columns are drawn italic with their
provenance word (`derived`) in the tooltip. The column chooser (header menu, `table.columns`) shows and hides columns
and orders them; the key column cannot be hidden. Rows have a fixed height; egui_table virtualises them. A **cell
cursor** (row, column) moves with the arrow keys, Tab and Shift+Tab; Home and End go to the first and last column;
Ctrl+Home and Ctrl+End to the first and last row. Rows are marked as in §6.4 and synchronised as in §6.5.

AccessKit: each focusable cell is named `<column label> <cell text>, row <i> of <n>` and, when read-only, `, read-only`;
the header cells are buttons named `Sort by <column label>`.

### 8.2 The editable columns (D10)

| Navigator | Column | Route (kind, path) | Identifier |
|---|---|---|---|
| Pages | `title` | page, `title` | no |
| Pages | `type` | page, `kind` | no |
| Devices | `function_text` | device, `function_text` | no |
| Connections | `colour` | connection, `overrides.colour` | no |
| Connections | `cross_section` | connection, `overrides.csa` | no |
| Terminal strips | `terminal` | device, `terminal.designation` | yes |
| Terminal strips | `label` | device, `function_text` | no |
| Terminal strips | `state` | device, `state` | no |
| Cables | `colour` | conductor, `colour` | no |
| Cables | `cross_section` | conductor, `csa` | no |
| Potentials | `potential` | potential_def, `name` | yes |
| Potentials | `class` | potential_def, `circuit_class` | no |
| Potentials | `colour` | potential_def, `props.colour` | no |
| Potentials | `cross_section` | potential_def, `props.csa` | no |
| Structure identifiers | `designation` | structure_node, `code` | yes |
| Structure identifiers | `function_text` | structure_node, `description` | no |

A test asserts that each route is an `EditRoute::FieldsSet` of `field_catalogue` with a scalar `FieldType` (D10) and
that `identifier` agrees with the catalogue; a route that fails stops the lane. A leaf below a settable member
(`props.colour`) is composed into that member per target exactly as the property grid composes it (WP-S2.05 §6.3
"leaf" rule, per target). Every other column is read-only with the reason, in this order of precedence: "derived"
(a derived column), "generated" (a generated page), "no record of this kind" (the row has none), "edit in the property
grid" (a route whose type is not scalar), "no command yet" (no route).

### 8.3 Editing and the compiled command (D11, D13)

- **One cell:** F2 or typing a character starts editing the cursor cell (the buffer starts with the cell's text, or with
  the typed character); Enter commits it for the cursor row, **Ctrl+Enter** for every marked row (doc 16 §7.2), Escape
  reverts. The text is parsed by `parse_field_input(kind, path, text)`; a parse error stays in the field with the
  message and commits nothing; an ambiguous number offers its readings in a list (WP-S2.05 §6.5).
- **Targets:** for each row, all its records of the route's kind, in record order, deduplicated across the gesture.
- **Compile:** collect the (target, path, value) triples in (row display order, column visible order). If every triple
  has the same path and the same canonical-JSON value, the command is `fields.set {kind, targets, path, value}`;
  otherwise `fields.set_many {edits}` with one entry per (kind, path, value) in order of first appearance, its targets
  in order of appearance. A triple whose (target, path) appeared earlier with a different value is **invalid** ("same
  record as row <n>"); with the same value it is merged.
- **Policy:** the command is previewed once; FR-UI-044's rules apply as built for the grid: rule 1 (more than 500
  targets or a `Bulk` command), rule 2 (an identifier route on more than one target), **rule 3** (any invalid or
  ambiguous cell in the gesture). With none, the preview is committed and the toast reads "Set <column label> on <n>
  <row kind plural> · Undo"; otherwise the card opens in the pane (`CardOrigin::Table`).
- **Clear** (Delete): sets the cursor column of the marked rows to `null` when the type is Optional, to the empty text
  for Text, Ident and MlText (MlText: removes the source language as WP-S2.05 does), and is invalid for any other type
  ("cannot be cleared").

### 8.4 Fill down (Ctrl+D)

The cursor column over the marked rows (at least two; else refused "mark the rows to fill"), in display order: every
row gets the parsed value of the first marked row's cell. Golden A: after terminal 13 of -X1031 has the function text
"Spare" (one cell edit, its own undo step), marking terminals 13 to 17 and filling down is one `fields.set` with the 5
terminals as targets, a direct commit with the toast, and one undo step that restores terminals 14 to 17.

### 8.5 Pattern fill (Ctrl+Shift+D; D12)

A dialog with the pattern field and the first five values of the marked rows as the preview; Enter applies. Headless,
`table.pattern_fill { pattern }` applies at once. The **grammar**:

```text
template   := ( literal | "{{" | "}}" | "{" expr "}" )*
literal    := any character except "{" and "}"
expr       := counter | "col:" column-key | "prev"
counter    := "n" [ "*" uint ] [ ("+" | "-") uint ] [ ":" "0" digit+ ]
```

`n` is the row's 1-based position among the marked rows in display order; `n*k+j` and `n*k-j` compute in `i64` (an
overflow or a negative result is an error for that row); `:0w` pads the decimal with zeros to w digits (`{n:02}` gives
`01`); `{col:<key>}` is the row's cell text of that column (an unknown key is an error for the whole pattern); `{prev}`
is the text this fill produced for the previous marked row, and for the first row the cell text of the cursor column
in the row directly above it in display order (empty when there is none). Every produced text is parsed by
`parse_field_input` (a failure makes that cell invalid). Examples: `Spare_{n:02}` → `Spare_01` … `Spare_20`;
`{col:label}`; `Reserve`.

### 8.6 Paste from Excel and copy (Ctrl+V, Ctrl+C; D12)

**Parse** (the clipboard's text, or `table.paste`'s `text`, or the headless clipboard): rows split on CRLF or LF, cells on
TAB; Excel quoting (a cell in double quotes may hold TAB, line breaks and doubled quotes); one trailing empty row
dropped; more than 100,000 cells or a single cell over 10,000 characters refuse the whole paste ("too large"); a quote
that does not close refuses it ("unclosed quote in row <n>"). **Header:** when the first row's cells match (case-folded
and trimmed) at least half of the visible column labels, it is a header and columns are mapped by label (unmatched
pasted columns are ignored and listed in the card); otherwise columns are mapped by position from the cursor column
rightwards over the visible columns. **Rows** start at the cursor row and go down in display order; a pasted row beyond
the last row is invalid ("no row"); a cell on a read-only column is invalid with that column's reason. Each cell text
goes through `parse_field_input`; then §8.3's compile and policy apply. A paste is one command.

**Copy** (Ctrl+C, `table.copy`) writes the marked rows' visible cells as TSV with a header row of the visible column
labels, quoting as Excel does; headless it writes the headless clipboard.

### 8.7 The table card

The card of WP-S2.05 §6.6 with origin `table`: its title states the gesture and counts ("Preview: paste 12 cells on 12
terminals · 2 invalid"), it lists up to 20 changed records and every **invalid** or **ambiguous** cell (row label,
column label, text, reason). **Apply** is refused while an invalid or ambiguous cell remains ("2 cells are invalid or
ambiguous"); `table.resolve { cell, choice }` picks a reading of an ambiguous cell and `table.drop { cell }` removes a
cell from the gesture; each rebuilds the command and its preview. Apply commits; Cancel or Escape drops it; the stale
rule as built (a stale card rebuilds and needs a second Apply).

### 8.8 Actions, transcript and describe of part 3 (14 new; 217 → 231)

| ID | Kind | Label | Menu | Shortcut | Context | Params |
|---|---|---|---|---|---|---|
| `navigator.table` | View | Table | — | — | global, hidden | `kind, on?:bool` |
| `table.columns` | View | Columns | — | — | global, hidden | `kind, show?:list(text), hide?:list(text), order?:list(text)` |
| `table.cell` | View | Move the cell cursor | — | — | navigator, hidden | `kind?, row?:text, column?:text, step?:int, across?:int` |
| `table.edit` | View | Edit cell | — | F2 | navigator, hidden | `kind?, row?:text, column?:text` |
| `table.input` | View | Type into the cell | — | — | navigator, hidden | `text:text` |
| `table.commit` | Command | Commit the cell | — | — | navigator, hidden | `all?:bool` |
| `table.set` | Command | Set cells | — | — | global, hidden | `kind, column:text, value:text, rows?:list(text), all?:bool` |
| `table.fill_down` | Command | Fill down | — | Ctrl+D | navigator | `` |
| `table.pattern_fill` | Command | Pattern fill... | — | Ctrl+Shift+D | navigator | `pattern?:text` |
| `table.paste` | Command | Paste cells | — | Ctrl+V | navigator | `text?:text` |
| `table.copy` | View | Copy cells | — | Ctrl+C | navigator | `` |
| `table.clear` | Command | Clear cells | — | Delete | navigator | `` |
| `table.resolve` | Command | Choose a reading | — | — | navigator, hidden | `cell:uint, choice:uint` |
| `table.drop` | Command | Drop a cell | — | — | navigator, hidden | `cell:uint` |

Enter while editing is `table.commit`; Ctrl+Enter while editing is `table.commit {all: true}`, and with a card shown
`navigator.apply`. A table action while the navigator is not in table mode refuses "switch to the table first".

```text
navigator "Terminal strips" kind strips view tree table sort "Terminal" ascending filter "x1031" rows 20 showing 1 to 20
  columns "Strip" "Terminal" "Function text" "Group" "Part" "Potential" "Side 1 (clamp a)" "Side 2 (clamp b)" "Bridge" "State"
  row "-X1031" "1" "1+" "Pneumatics" "PT 2.5-TWIN" "24VDC-1" "-102F2:Out1" "-201U1:5 via -106W1 BN" "BR1" "used" selected
  row "-X1031" "13" "" … cursor "Function text"
  editing "Function text" buffer "Spare"
  pattern "Spare_{n:02}" values "Spare_01" "Spare_02" "Spare_03" "Spare_04" "Spare_05"
```

The card's invalid cells print as `  cell <i> "<row label>" "<column label>" invalid "<reason>"` or `… ambiguous
"<reading 1>" "<reading 2>"`.

**New describe words (6):** `table`, `columns`, `invalid`, `ambiguous`, `pattern`, `values` (291 → 297).

## 9. Determinism, the headless mode and exact counts

1. **Every flow is a script.** Each acceptance flow of §10 that runs in kittest also runs as an `ew.uiscript/1` script
   through the built binary, byte-equal on two runs. The type-ahead reset and the toast's timeout are window-only.
2. **Every commit is reproducible.** The `--record` log of each script, applied with `ew apply` to the starting project,
   gives equal records (one test per part); `page.reorder` and the compiled `fields.set`/`fields.set_many` carry
   explicit values, so replay neither reads the clipboard nor parses a pattern.
3. **Tie-breaks are named:** node order (§6.1, §6.7, §7.1), sibling and column sort (natural, then node order), token
   order, the drawables' order, the marked-row order (visible order), the compile order (§8.3), the highlight slots
   (first free, else the oldest). None depends on a hash map's order or on time (NFR-REL-07).
4. **Exact counts** (NFR-PERF-20; `SessionCalls` gains `navigator_table` and `highlight_members`): opening a navigator
   or a new revision with it open causes exactly one `navigator_table(kind)` call per open navigator on the next frame;
   filtering, sorting, expanding, moving the cursor, marking, type-ahead, switching to the table, moving the cell cursor
   and typing into a cell cause 0 `navigator_table` calls and 0 memo re-executions; a closed navigator causes 0 calls;
   a gesture of §8.3 to §8.6 makes exactly one `preview_with` and then one `commit` (window) or one `run_script`
   (headless), or none more until Apply; a highlight causes one `highlight_members` call per highlight per revision.
5. **No time values, paths, process IDs or UUIDs** in transcripts or describe text; IDs print as their alias `@n`
   (ID-630).

## 10. Acceptance checks

All tests are ordinary nextest tests in the existing jobs, on temporary copies of Golden A (or a wizard project), with
`EW_CONFIG_DIR` set to a temporary directory. A golden fact of this document that the data contradicts stops the lane
for a lead ruling.

### 10.1 Part 1

| # | Check | Where | Proves |
|---|---|---|---|
| NV1 | `project_rows` agreement: on Golden A equal to `rows_of` for blocks 901 (component mode) and 903 to 910 (903 to 905 with their strips); `None` for an unknown kind; stored blocks' rows unchanged | `ew-reports` tests | D3, §4.2 |
| NV2 | `navigator_table(Pages)`: the 36 entries in the derived page order in every view (as built); `navigator_table(Devices)`: one device row per non-terminal device of Golden A, natural tag order inside each structure node; -201U3 with representation rows `201-E4` and `203-B4`; keys stable across a no-op revision; `index` maps a representation to its row | `ew-api` tests | §6.1, §6.6, §6.7, D16 |
| NV3 | Filter: tokens and facets of §6.3 on a fixture table (prefix, the substring union, `page:`, `state:spare`, `unplaced`, an unknown facet as a plain token); Pages: the as-built matches are a subset for every folio and title substring of Golden A; ancestors shown and expanded | `ew-viewmodel` tests | D6 |
| NV4 | Scale: a synthetic `NavTable` of 100,000 nodes (built in the test, no `Session`): flattening, a filter keystroke and a sort produce correct rows; the kittest paints at most (visible rows + 2) rows per frame; no `navigator_table` call while scrolling | `ew-viewmodel`, `ew-ui` tests | NFR-PERF-15 (structural), §11 |
| NV5 | FR-UI-007's acceptance (amended, D16), kittest and script: folio 102 shown; marking -201U3 in Devices leaves the canvas selection without drawables on 102, the property grid shows -201U3, the chip line `elsewhere "201-E4" "203-B4"`; Enter opens 201 with its representation selected (one back-history entry); clicking a representation on the canvas marks and reveals its row with the focus unchanged; no feedback loop (each change applies once, by the generation counter) | `ew-viewmodel`, `ew-ui` tests | D7, §6.5 |
| NV6 | `page.reorder`: every rejection with its pointer; the example of §5.1 (106 → 103, 103 → 106; 101, 102 untouched); on a fixture section with a labelled page (folio 5 and 5.1), the (number, label) pairs move as whole keys; undo in one step restores every folio; DSL verb and fuzzer arm | `ew-commands`, `ew-testkit` tests | §5.1 |
| NV7 | FR-PAG-024's rest (FT-360): marking 103 and 106 with Shift+Down selects both; dragging 106 before 103 (kittest drag, and `pages.reorder` in a script) opens the card with "106 → 103, 103 → 106" and its tag and xref counts; Apply commits one `page.reorder`, one undo step; a generated page refuses; Alt+Up on 106 gives the same card | `ew-viewmodel`, `ew-ui` tests | D8, §6.6 |
| NV8 | FT-461: with folio 103 current, Page ▸ New page proposes `104`; with folio 1 current it proposes `4` (2 is the generated table of contents and 3 a page record); with a generated page current the field is empty; the section field takes the marked section node's section | `ew-viewmodel` tests | §6.6 |
| NV9 | Drag of an unplaced function: on a copy with -102F1's device given a second, unplaced function (added by `command.run`), its row shows `unplaced`, and dropping it on folio 102 dispatches `place.pick {function}` (the Place tool armed with `existing_function`) | `ew-ui` test | §6.7 |
| NV10 | Transcript and describe: the Pages lines byte-equal to the as-built expected text; a Devices window of 50 rows around the cursor; the 18 words; the `focus` member values; the scripts twice byte-identical; replay by `ew apply` gives equal records; the blind references (`actions.txt`, `vocabulary.txt`) re-blessed with 215 actions and 285 words | `ew-viewmodel`; `bin/elektrawatt/tests` | §6.10, §9 |
| NV11 | Exact counts of §9 item 4 for part 1 | `ew-ui` test | §9 |
| NV12 | Golden unchanged (golden output, acceptance and AC-15 tests unmodified); targeted checks for the changed crates (`ew-reports`, `ew-api`, `ew-commands`, `ew-testkit`, `ew-viewmodel` with and without `testkit`, `ew-ui`, `elektrawatt`): nextest, clippy `-D warnings`, `cargo fmt --all --check`, `check-deps`, `check-features`, `check-raw-read`, `check-licence-map`, `reuse lint`, `docs-gen --check`; no new package; the accessible-name walker on every new pane | lane | §4.1 |

### 10.2 Part 2

| # | Check | Where | Proves |
|---|---|---|---|
| NP1 | Each provider of §7.1 on Golden A: the table rows equal its oracle (the block's data rows; for `ips` the 48 functions under 26 names with 4 `external`; for `blocks` 15 rows; for `structure` the 4 nodes with JE2416 as an alias); the records and open targets of one row per provider as §7.1 and §6.1 item 7 | `ew-api` tests | §7.1 |
| NP2 | FR-UI-005's acceptance (amended, D16): the Connections data rows' records cover each of the 66 wire and core connections exactly once; PLC lists 56 channels under -201U1; Interruption points 22 pairs and 4 external points; Terminal strips 3 strips; Cables 3 cables | `ew-api` tests | FR-UI-005 |
| NP3 | FR-UI-018's acceptance (FT-362), kittest and script: highlighting 24VDC-1 from the Potentials navigator gives slot 0; on folios 102 and 103 the overlay holds exactly its members' polylines on that page (the members equal the connections of the wire-list rows whose potential is 24VDC-1), the snapshot has the slot colour at each segment's midpoint and the hatch lines at 45°; the describe line `highlight slot 0 potential "24VDC-1" connections <k>`; Ctrl+Shift+H again removes it; a fifth highlight replaces the first | `ew-viewmodel`, `ew-ui` tests | D14, §7.3 |
| NP4 | Synchronisation from the canvas: with Connections and Potentials open, clicking W29 on folio 103 marks its wire row in both (in Potentials under the row of W29's potential, 24VDC-1), reveals them and leaves the focus on the canvas | `ew-ui` test | §6.5 |
| NP5 | FT-461's second half: Structure ▸ New node with +J-E-2416 marked and kind `location` pre-fills the parent | `ew-viewmodel` test | §7.1 |
| NP6 | Transcript, describe, the 6 words (291), 217 actions re-blessed; scripts twice byte-identical; exact counts; golden unchanged; targeted checks as NV12 for `ew-api`, `ew-viewmodel`, `ew-ui`, `elektrawatt` | lane; tests | §7.4, §9 |

### 10.3 Part 3

| # | Check | Where | Proves |
|---|---|---|---|
| TB1 | The editable-column table: each route of §8.2 is a scalar `FieldsSet` route of the catalogue with the stated identifier flag; every other column read-only with the reason of §8.2's precedence | `ew-api` or `ew-viewmodel` test | D10 |
| TB2 | FR-UI-008's acceptance, kittest and script: Terminal strips in table mode filtered `x1031` and sorted by Terminal list 1, 2, … 10, … 20; filling "Spare" down over 5 terminal rows (§8.4's example) is one `fields.set` with 5 targets, a direct commit with the toast, **one undo step** (the history grows by one; undo restores terminals 14 to 17) | `ew-viewmodel`, `ew-ui` tests | FR-UI-008, §8.4 |
| TB3 | Pattern fill: the grammar's every branch with an example and every error (unclosed brace, `{x}`, unknown column, overflow, negative); `Spare_{n:02}` over -X1031's 12 spare terminals gives `Spare_01` … `Spare_12` in one `fields.set_many`; `{{` and `}}` give literal braces | `ew-viewmodel` tests | §8.5 |
| TB4 | Paste: a TSV with a header row in another column order maps by label; one without maps by position from the cursor; quoted cells with TAB and line breaks; `0,75` and `1,500` (ambiguous → the card, `table.resolve` then Apply); a read-only column and a row past the end are invalid; two rows on one record with different values are invalid ("same record as row <n>"); Apply refused until resolved or dropped; the paste is one command and one undo step; the fuzzer arm feeds random text to the parser without a panic | `ew-viewmodel`, `ew-testkit` tests | §8.6, §8.7, D13 |
| TB5 | The policy: designations of 2 terminals edited at once open the card (rule 2); a 501-row fill on a synthetic project opens it (rule 1); a single-cell edit of a potential's name opens no card (one target); a clear of an enum is invalid | `ew-viewmodel` tests | §8.3 |
| TB6 | Copy: the marked rows' TSV with header, byte-equal to an expected text for -X1032's 5 terminals; paste of that text back into the same rows changes nothing (the command is empty and commits nothing) | `ew-viewmodel` test | §8.6 |
| TB7 | The table widget: the key column stays visible while scrolling right; the column chooser hides and orders columns; header click sorts; the AccessKit cell names of §8.1; keyboard-only flows of TB2 to TB4 without pointer events | `ew-ui` tests | §8.1 |
| TB8 | Transcript, describe, the 6 words (297), 231 actions re-blessed; scripts twice byte-identical; replay; exact counts; golden unchanged; exactly two new packages with their exemptions, `cargo deny` and `cargo vet` green; targeted checks as NV12 | lane; tests | §8.8, §9, §4.1 rule 2 |

The workspace suite, the acceptance runner and the performance runs are not run by the lanes (testing policy, ID-512);
the integrating session runs `cargo xtask ci` at the cycle delivery.

## 11. NFR-PERF-15 and the gates

NFR-PERF-15 is the package's acceptance requirement. Its timings (scroll within NFR-PERF-04's frame, a filter keystroke
within 100 ms, find within 1 s at XL) **are not measured** during development (owner's rule, ID-512). The package
proves its structure: virtualised painting (NV4, TB7), a filter over precomputed tokens without `Session` calls (NV3,
NV4, §9 item 4), one table build per open navigator and revision (§9 item 4). Part 1 also adds an `#[ignore]`d
release-profile test that builds the Devices table of the XL synthetic fixture and runs a 5-character filter, which the
release phase runs with FT-491; until then perf claims read "not measured (owner's rule)". No gate criterion changes:
G-PP depends on no navigator beyond FT-408 and FT-409 (delivered), and no gate of [gates](../03-gates.md) names
NFR-PERF-15 today; its measured value is the release phase's (FT-491).

## 12. Files owned

**Part 1:** `crates/ew-reports/src/{lib.rs,queries.rs}` (`project_rows`) and its tests; a new
`crates/ew-api/src/output/navigators.rs`, `crates/ew-api/src/output/mod.rs` and `src/lib.rs` (methods and re-exports
only), ew-api tests; `crates/ew-commands/src/commands/{page.rs,mod.rs}` (`page.reorder`), `schemas/**`, `tests/**`;
`crates/ew-testkit/src/golden/dsl.rs`, `tests/command_fuzz/**`; `crates/ew-viewmodel/src/**` (a new `navigators/`
module that absorbs `navigator.rs`, `layout.rs`, `actions.rs`, `strings.rs`, `describe.rs`, `script.rs`,
`shell/navigation.rs`, `properties.rs` for `CardOrigin::Reorder`); `crates/ew-ui/**` (`navigator.rs`, `dock.rs`,
`app.rs`, `canvas.rs` for the chip line); `tools/blind/reference/*.txt` (re-blessed); `bin/elektrawatt/tests/**`; the
generated reference pages.
**Part 2:** `crates/ew-api/src/output/navigators.rs` (the nine providers, `highlight_members`) and tests;
`crates/ew-viewmodel/src/{navigators/**,highlight.rs,tools/overlay.rs}` and the shared lists; `crates/ew-ui/src/canvas.rs`
(highlight painting); `tools/blind/reference/*.txt`; `bin/elektrawatt/tests/**`.
**Part 3:** `crates/ew-viewmodel/src/navigators/{table.rs,pattern.rs,paste.rs}` and the shared lists,
`properties.rs` for `CardOrigin::Table`; a new `crates/ew-ui/src/table.rs`, `crates/ew-ui/Cargo.toml` (egui_table);
`Cargo.lock`; `supply-chain/config.toml` (the two exemptions only); `crates/ew-testkit/tests/**` (the TSV fuzzer arm);
`tools/blind/reference/*.txt`; `bin/elektrawatt/tests/**`.

Shared append-only files (registry lists, `VOCABULARY`, transcript members, `params.rs`, fuzzer arms, DSL verbs) may be
touched by every part and by other lanes; the second to integrate rebases onto the first. Read-only for the lanes: every
other crate, `golden/**`, `ci/**`, `deny.toml`, `.github/**`, `docs/**` except the generated reference pages. Each lane
commits on its own branch and does not merge or push.

## 13. Out of scope

| Item | Where it goes |
|---|---|
| The Messages navigator (FR-UI-005's entry) and the message panel (FR-UI-021) | WP-S2.09, on this framework (FT-493) |
| The Unplaced devices navigator (panel layout) | WP-S4.02, after WP-S4.01's layout placements (FT-486) |
| Library status of parts in use (current, update available, local fork) | WP-S1.19 (FT-487) |
| Finding-count badges and the `has:finding` facet | WP-S2.09 (FT-488) |
| Persisting a navigator's view, sort, filter, columns and widths (doc 16 §5.3, §2.5) | WP-S2.08 (FT-489) |
| Delete in navigators with the cascade preview, F2 rename in trees, row insert, delete and move in tables | WP-S2.07 (FT-490) |
| Signal and net levels in the potentials tree; the measured NFR-PERF-15 with incremental tables or the job filter | FT-491 (release phase, WP-S6.03) |
| Page thumbnails mode | WP-S2.14 (FT-492) |
| The strip editor, the edit-cable table, the PLC grid with drag-bind, part selection | WP-S2.07 |
| The bulk preview tab replacing the card; the Excel round trip (`table.apply_changes`) | WP-S2.09; FR-EXP-004's package |
| Headers in other UI languages when pasting | WP-S2.08 (with its Fluent labels) |
| Bus, fluid, layout and comment navigators | 1.0 (WP-S2.13 and the layout packages) |

## 14. Review checklist for the Opus reviewer

1. **Boundaries.** Only the items of §4.2 and the command of §5 below the shell; no new edge; exactly two new packages
   in part 3; nothing under `golden/` changed; no new memoised family; no `ew-model` name in `ew-viewmodel` or `ew-ui`.
2. **One row source.** `navigator_table` is the only door; report-kind tables come from `project_rows` and agree with
   the stored blocks; no provider re-derives what a report kind already lists.
3. **Pages unchanged where it matters.** The `pages.*` actions, keys, describe lines (≤ 50 rows) and AccessKit names
   as built; committed recordings replay.
4. **Synchronisation.** §6.5 exactly: no page switch on marking, the chip line, the generation guard, focus unchanged
   on reveal, the cursor following page switches.
5. **`page.reorder`.** The permutation rule, the rejections, `Bulk`, the card, undo in one step.
6. **Tables.** Editable cells only by §8.2 and D10; targets all records of the kind; compile order and the
   same-record rule; FR-UI-044 rules 1 to 3; invalid and ambiguous cells block Apply; one command per gesture.
7. **Grammars.** Pattern fill and TSV parsing as §8.5 and §8.6, every error reachable; no second value parser.
8. **Highlighting.** View state only; colours and hatch angles as §7.3; composited into the snapshot; nothing saved.
9. **Determinism and privacy.** Named tie-breaks; transcripts byte-identical; replay equality; no time, path or UUID.
10. **Accessibility.** Every gesture keyboard-only; tree and table names; patterns plus colour.
11. **Scope.** Nothing of §13 half-done.

## 15. Size, order, routing and the documents to update

### 15.1 Size

WP-S2.06 keeps **5.2 pw PERT** (O 3.5 · L 5 · P 7.5); the CSV is unchanged. **Delivered slice:** FT-408 and FT-409,
0.45 pw (cycle 27). **Part 1, 2.2 pw:** `project_rows` with its agreement test 0.15; the table contract, Pages and
Devices providers 0.35; the framework state, flattening, filter, sort and cache 0.35; selection synchronisation and the
chip line 0.3; `page.reorder` with its schema, verb, arm and tests 0.25; multi-selection, drag, keys and the reorder card
0.25; panes, menu, the generic tree widget and AccessKit 0.3; FT-461's pages half, actions, transcript, describe and
scripts 0.25. **Part 2, 1.05 pw:** the nine providers 0.65; highlighting with `highlight_members`, the overlay and the
snapshot 0.3; FT-461's structure half, transcript and tests 0.1. **Part 3, 1.5 pw:** the table widget on egui_table with
the column chooser, cell cursor and AccessKit 0.4; editing, the compile rule and the policy 0.3; fill down and pattern
fill 0.2; paste and copy 0.3; the table card with resolve and drop 0.15; actions, describe, scripts and the dependency
entries 0.15. The parts and the slice sum to **5.2 pw: no reserve.** **De-scope rule:** when a part is projected (at
its lane's midpoint report) or found (at its review) more than 20 % over its size, the next items in this order leave
the package as follow-ups without a further ruling, until the projection fits: the Report blocks navigator (part 2),
then the Parts in use navigator (part 2, an MVP Should), then type-ahead (part 1), then `table.copy` (part 3); the lane
reports which ones left.

### 15.2 Order and routing

Part 1 starts now (WP-S2.03, WP-S2.04 and WP-S2.05 are delivered). Part 2 starts after part 1's delivery; part 3 after
part 2's (its acceptance uses the Terminal strips navigator). Part 3 may run in the same cycle as part 2 only when the
orchestrator bases it on part 2's branch once part 2's strips provider commit exists. WP-S2.09's message panel may start
on the framework after part 1's delivery. **Routing** (ID-667: Claude subagents, model and effort per task): parts 1
and 3 to a lead-programmer lane at **D4** (`opus`, `high`): part 1 sets a public contract and adds a core command,
part 3 compiles user input into bulk commands; part 2 to a programmer lane at **D3** (`sonnet`, `high`): nine providers
in a decided pattern with oracles. Each part is planned as one lane-cycle plus the cycle's independent review and fix
round.

### 15.3 Decision-log entries (recorded as ID-681 to ID-689)

1. **ID-681** WP-S2.06 specified in three parts after the delivered slice, with sizes, order and routing, no reserve and
   the de-scope rule (D1, §15.1, §15.2).
2. **ID-682** The navigator set at M-PP and its boundaries (D4): eleven kinds; Messages as WP-S2.09's message panel on
   this framework; Unplaced devices to WP-S4.02; the potentials tree simplified; no library status, no finding badges,
   no thumbnails, no persisted state.
3. **ID-683** One row source and the report-kind tables (D2, D3): `navigator_table` per (kind, revision) for open
   navigators only, no memo family; `ew_reports::project_rows` agreeing with the stored blocks; the table contract of
   §6.1.
4. **ID-684** The framework's behaviour (D5, D6, D7): the Pages navigator migrated with its actions and lines kept;
   token-and-substring filter with the closed facets on the UI thread; selection synchronisation both ways without page
   switches, the chip line, the generation guard; FT-371's navigator item closed by the cursor rule.
5. **ID-685** `page.reorder` (D8): the permutation of folio keys in the changed range, same section only, `Bulk`, the
   card; FT-360 closed by part 1.
6. **ID-686** Table mode (D9 to D13): egui_table `=0.11.0` and no egui_ltreeview (ADR-0014 amendment 4); the closed
   editable-column table; the compile rule to `fields.set` or one `fields.set_many`; FR-UI-044 rules 1 to 3; the
   pattern and paste grammars; the same-record rule; Apply blocked by invalid or ambiguous cells.
7. **ID-687** Highlighting (D14): potential and net targets, four slots with Okabe–Ito colours and hatch angles, view
   state only; FT-362 closed by part 2.
8. **ID-688** The headless surface (D15): 31 actions (201 → 231), 30 describe words (267 → 297), the 50-row window, the
   `focus` values, no new top-level section; one new command (54 → 55).
9. **ID-689** The departures from the requirement texts (D16): FR-UI-007's acceptance on Golden A lists 201-E4 and
   203-B4 (902 with WP-S4.01); FR-UI-005's W1 to W44 read as the wire list's rows covering the wire and core
   connections; doc 16 §5.2's message counts not asserted here.

### 15.4 Follow-up tasks (recorded as FT-486 to FT-493)

1. **FT-486** The Unplaced devices navigator (location → device; part, dimensions; drag to rail or surface) on this
   framework, once WP-S4.01 records layout placements (WP-S4.02, M2).
2. **FT-487** The library status of parts in use (current, update available, local fork) as a column and badge, from
   the library version comparison of FR-SYM-016 (WP-S1.19, M2).
3. **FT-488** Finding-count badges ("E 2") on navigator rows and the `has:finding` facet, fed by the message panel's
   findings of the current revision (WP-S2.09, M2).
4. **FT-489** Persist each navigator's view, sort, filter, visible columns, column order and widths in the per-user view
   state `.ew/u-<hash>/view.json` (doc 16 §2.5, §5.3) (WP-S2.08, M2).
5. **FT-490** Delete in navigators with the cascade preview, F2 rename of authored labels in trees, and row insert,
   delete and move in tables through the domain commands (doc 16 §5.1, §7.2) (WP-S2.07, M2).
6. **FT-491** NFR-PERF-15 measured at XL in the release phase (the `#[ignore]`d test of §11); if a budget fails,
   incremental navigator tables or doc 16 §15.2's job filter; the signal and net levels of the potentials tree
   (WP-S6.03, release phase).
7. **FT-492** The Pages navigator's thumbnail mode from the cached page thumbnails (FR-UI-005's "may") (WP-S2.14, 1.0).
8. **FT-493** The Messages navigator as WP-S2.09's message panel on this framework (`NavKind::Messages`: severity →
   rule → finding, the §8.4 columns, Enter jumps, synchronisation), FR-UI-005's last entry (WP-S2.09, with WP-S2.09).

### 15.5 Open questions for the owner

None. Two rulings the owner may wish to reverse, each with the lead's recommendation to keep it: D8's permutation (the
alternative, renumbering the changed range consecutively from its first number, would turn the old 103 into 104 instead
of 106; keep the permutation, it cannot collide and touches fewest pages) and D4's move of the Messages navigator into
WP-S2.09 (keep it: one panel, one specification).

### 15.6 Documents the design passes update

**Now (cycle 30):** [ADR-0014](../../adr/0014-gui-toolkit-egui.md) amendment 4 (D9); plan 02 (WP-S2.06's status, FT-486
to FT-493); decision log ID-681 to ID-689. **After each part:** doc 16 §5.1 to §5.4 (the framework as built, the facets,
the synchronisation and the chip line, the eleven navigators, Messages with WP-S2.09, Unplaced devices with WP-S4.02,
the simplified potentials tree), §7.1 to §7.5 (table mode as built, the editable columns, the compile rule, the pattern
and paste grammars, the table card), §10.3 (the highlight colours and hatches), §15.2 (no job path at M-PP);
doc 02b FR-UI-005 and FR-UI-007 (D16's acceptance texts) and FR-UI-018's note (built); doc 02a FR-PAG-024 (the drag
built, `page.reorder`'s rule); doc 06 §3.1 (`page.reorder`, the command count); doc 17 §2.1 (the count); doc 18 §3.17
(what the drafter reads in navigators); architecture §9 (the navigators and tables row as built, egui_table); WP-S2.09's
row (the message panel on this framework, FT-493) and WP-S4.02's (FT-486); the traceability and the glossary
(navigator table, drawable, highlight slot).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S2.03, WP-S2.04, WP-S2.05, WP-S2.06, WP-S2.07, WP-S2.08, WP-S2.09,
  WP-S2.14, WP-S4.01, WP-S4.02, WP-S6.03, WP-S6.13; Appendix B (FT-360, FT-362, FT-371, FT-408, FT-409, FT-443, FT-458,
  FT-461).
- [Decision log](../06-decision-log.md): ID-447, ID-474, ID-490, ID-512, ID-531 to ID-538, ID-556 to ID-560, ID-591 to
  ID-600, ID-623, ID-626, ID-628, ID-630, ID-634, ID-648, ID-650, ID-667.
- [ADR-0008](../../adr/0008-transactions-undo-preview.md), [ADR-0014](../../adr/0014-gui-toolkit-egui.md);
  [architecture](../../design/04-architecture.md) §3, §9; [domain model](../../design/03-domain-model.md) §6 to §13;
  [UX/UI](../../design/16-ux-ui.md) §5, §7, §10, §15; [reports](../../design/09-reports-forms-output.md);
  [golden project](../../design/20-golden-project.md).
- Requirements: FR-UI-005, FR-UI-007, FR-UI-008, FR-UI-018, FR-UI-025, FR-UI-036, FR-UI-044, FR-PAG-024, FR-I18N-010,
  FR-I18N-011, NFR-PERF-15, NFR-PERF-19, NFR-PERF-20, NFR-A11Y-01 to NFR-A11Y-03, NFR-REL-07.
- egui_table 0.11.0 on crates.io (published 2026-10-19; depends on egui ^0.37.0, serde, vec1 ^1.12.1; MSRV 1.95;
  MIT OR Apache-2.0).
