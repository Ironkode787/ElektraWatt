# WP-S2.03: the viewer, its view models and the canvas tool framework (closed specification)

**Status:** specified in cycle 22 (documents only) for two lead-programmer lanes: part 1 (the read-only viewer, D4) as a
cycle-23 candidate, part 2 (the tool framework, D4) after part 1's delivery; Opus review of each · **Date:**
2026-10-22 · **Work package:**
[WP-S2.03](../02-work-packages.md#wp-s203-viewer-view-models-and-canvas-tool-framework) · **Decision log:** the
entries the cycle-22 design pass allocates for this specification (§3) · **Part of:** ElektraWatt plan,
specifications

**Purpose.** This is the closed specification of the viewer and of the framework every editing tool builds on. Part 1
turns the canvas of [WP-S2.02](WP-S2.02-canvas-renderer.md) into a viewer one can find one's way in: a page navigator,
tabs and two pages side by side, cross-reference jumps with Back and Forward, Go to page or cell with the cell in the
status bar, and a keyboard grid cursor. Part 2 adds the tool state machines with their transient overlay, the preview
command protocol, snapping with its indicators and keyboard-driven tool input, without a tool that changes the model:
the viewer stays read-only until WP-S2.04. Where this document and a design document differ, this document is the
narrower and wins for the lanes; the design documents stay the target of the whole work package, and the documents
named in §14 are updated at the design pass after each part. A choice a lane meets that this document does not settle
is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [The decisions of this specification](#3-the-decisions-of-this-specification)
4. [Crates and boundaries](#4-crates-and-boundaries)
5. [Dependencies](#5-dependencies)
6. [Part 1: the read-only viewer](#6-part-1-the-read-only-viewer)
7. [Part 2: the tool framework](#7-part-2-the-tool-framework)
8. [Determinism, the headless mode and exact counts](#8-determinism-the-headless-mode-and-exact-counts)
9. [Order of work and the parallel WP-S2.02 part 2](#9-order-of-work-and-the-parallel-wp-s202-part-2)
10. [Acceptance checks](#10-acceptance-checks)
11. [Files owned](#11-files-owned)
12. [Out of scope](#12-out-of-scope)
13. [Review checklist for the Opus reviewer](#13-review-checklist-for-the-opus-reviewer)
14. [Size, routing and the documents to update](#14-size-routing-and-the-documents-to-update)
15. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the work package the project view is a viewer and a tool host. **Part 1:** the page list
becomes the page navigator (flat, section and structure views, sort by folio or type, a filter, keyboard operation);
the canvas area holds several pages as tabs and shows two of them side by side, each tab with its own viewport per
page; a click or `F` on a cross-reference or interruption-point text jumps to its target, and Back and Forward walk the
tab's jump history with the previous viewport; Go to (Ctrl+G) accepts a folio or a cell in the project's cross-reference
format; the status bar shows the cell beside the millimetres (FT-345); a grid cursor moves over the page's connection
grid from the keyboard and is announced through AccessKit. **Part 2:** tools are state machines whose outputs are a
transient overlay, a preview command, a committed command or a cancel; the host previews on a fork through the
`Session`, keeps the overlay, snaps to the page's connection grid and connection points with indicators, routes the
keyboard (grid cursor, next and previous connection point, Enter as a click, Escape) to the active tool, and ships two
tools that change nothing: Select (selection, rubber band, link hover preview) and Measure. Every flow of both parts is
a registry action on `ShellModel::dispatch`, so `elektrawatt --headless --script` and the blind drafter drive it and
read it back without a screenshot (decision log ID-443).

**Non-goals** (each is named with its package in §12):
- **No editing.** No shipped tool commits a command; moving, placing, wiring and drawing are WP-S2.04. The commit path
  of the framework is exercised by a tool that exists only in tests (§7.6).
- No persistence of open tabs and viewports in `view.json`; no drag-to-reorder or multi-selection in the page
  navigator; no potential highlighting (proposed follow-up tasks, §12).
- No typed selection, `page_spatial_index` or exact kurbo picking: part 2's picking stays on the display list's entity
  rectangles of WP-S2.02 (§7.6).
- No change to the GPU path of WP-S2.02 part 2, to any exported byte, to the workspace file format `ew.workspace/1` or
  to the project format.

## 2. Inputs

| Input | What the lanes take from it |
|---|---|
| [WP-S2.01's specification](WP-S2.01-application-shell.md) and [WP-S2.02's](WP-S2.02-canvas-renderer.md), with their as-built notes | The shell model, the registry, `dispatch`, the headless mode and `ui.describe` (ID-474, ID-493), the canvas model, the tile path, the overlay module, the frame statistics, the exact-count test A9 and its counters (ID-519), the rules of their §3 and §4.1 |
| Code as built | `crates/ew-viewmodel/src/{canvas.rs,shell.rs,shell/canvas_actions.rs,page_list.rs,layout.rs,actions.rs,status.rs,describe.rs,script.rs,strings.rs}`; `crates/ew-ui/src/{canvas.rs,dock.rs,app.rs,keys.rs,headless.rs}`; `crates/ew-render2d/src/{tiles.rs,overlay.rs,view.rs}`; `crates/ew-dl/src/{item.rs,link.rs,text.rs}`; `crates/ew-api/src/{lib.rs,output/mod.rs,output/view.rs}`; `crates/ew-derive/src/{cell.rs,cp_point.rs,pages.rs,profile.rs}`; `crates/ew-export/src/pdf.rs` (the link-rectangle rule) |
| [Architecture](../../design/04-architecture.md) §3 (rules 3, 6, 7), §5.1, §5.2, §5.4, §8.2, §9 | `Session` as the only door; the transaction-read rule (ADR-0008 amendment 4, ID-525); preview is a fork; the canvas tools row; the hit-testing line |
| [ADR-0008](../../adr/0008-transactions-undo-preview.md), [ADR-0014](../../adr/0014-gui-toolkit-egui.md), [ADR-0005](../../adr/0005-value-types-nm-dec3-mltext.md), [ADR-0027](../../adr/0027-native-symbol-grid-and-qet-grid-policy.md) | Previews are the real command on a fork, preview + commit = execute; egui and egui_tiles 0.18.0 (ID-490); integer nm and grid indices; the QET grid |
| [UX/UI](../../design/16-ux-ui.md) §1, §2.3 to §2.5, §3.4 to §3.6, §4.1, §4.2, §4.5, §4.6, §5.1, §5.2, §10.1 to §10.3, §17 | View actions never touch the model; selection; view state; canvas tabs and their keys; the status-bar cursor; Escape's order; the tool host; the navigators' shared behaviour; AccessKit; the overlay colours and patterns |
| [Schematic editor](../../design/06-schematic-editor.md) §2.4, §2.8, §4.1, §4.3, §4.4, §5.1, §5.7, §6 | The three geometric systems; xref texts are links; the tool contract; the Select tool's table; keyboard wiring; the exact cell rule; snapping on grid indices; the canvas navigation and keys |
| [Reports and output](../../design/09-reports-forms-output.md) §4.2 | The `LinkSource` rectangle of an xref text and of each listed item |
| [Quality](../../design/18-quality-security-release.md) §3.5, §3.11, §3.16, §3.17 | Determinism; UI tests; the source rules; what the blind drafter drives |
| Requirements | FR-UI-015, FR-UI-016, FR-UI-018, FR-UI-019, FR-UI-035, FR-UI-039, FR-PAG-024, FR-XRF-019, NFR-A11Y-01 (the work package's nine rows), with FR-UI-034, FR-UI-026, NFR-A11Y-02, NFR-A11Y-03, NFR-UX-07, NFR-REL-08, NFR-PERF-19 and NFR-PERF-20 where they touch it; AC-28 (the follow-and-back part) |

The WBS lists WP-S2.01, WP-S2.02 and WP-S1.08 as dependencies. WP-S2.01 and WP-S2.02 part 1 are delivered; WP-S1.08
(cross-references and the page order) is in place as built. WP-S2.02 part 2 (the GPU path) runs in parallel in cycle 22;
nothing here depends on it (§9).

## 3. The decisions of this specification

Each is a lead ruling that the owner may reverse, except the five marked "ruling (delegated)", which the owner handed
to the lead for this specification and which are recorded as taken. The design pass allocates the decision-log entries
(the IDs continue after ID-530).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Two parts.** Part 1 (2.5 pw): the read-only viewer (§6). Part 2 (2.4 pw): the tool framework (§7). Part 2 starts after part 1's delivery; both go to a lead-programmer lane at D4 | Part 1 gives R0 and the blind drafter a navigable viewer one part earlier; part 2 builds on its multi-canvas host, link list and grid cursor. Both add public items below the shell, so neither is a programmer lane |
| D2 | **Ruling (delegated): no editing semantics in part 1, and the viewer stays read-only until WP-S2.04.** Part 1 adds no action of kind `Command`; part 2's shipped tools (Select, Measure) commit nothing; the commit path is exercised by a test-only tool | The owner's reading of the shell as a viewer (ID-513) holds until the editing gestures exist; the framework is still proven end to end |
| D3 | **Ruling (delegated): side by side is two canvases sharing one `Session`, never a second `World`.** Every tab and both groups read the same session, revision and memo store; two tabs on one page share its page content and its tiles | One model, one undo stack, one selection; a second `World` would need a second change feed and could show two revisions at once |
| D4 | **Ruling (delegated): Back and Forward walk a history per tab, which is not persisted.** At most 50 entries back; a jump clears the forward list; closing a tab drops its history | Doc 16 §3.5 says "per window" and §2.5 lists the history in `view.json`; a per-tab history matches the browser model users know, and an unpersisted one needs no format. Doc 16 §2.5 and §3.5 are amended at the design pass |
| D5 | **Ruling (delegated): the grid cursor is a canvas-model concept, with no focus of its own, announced through AccessKit.** It is state of the tab's `CanvasModel`, moved by registry actions; the canvas stays one focusable AccessKit node whose description and a polite live region carry the cursor | A focusable node per grid point would flood the accessibility tree; a model concept is equally drivable headless |
| D6 | **The canvas area is per-project state in its own nested egui_tiles tree, not part of the workspace file.** The workspace's `page-view` pane is the canvas area; inside it `ew-ui` draws a nested `egui_tiles::Tree` built from the toolkit-neutral `CanvasArea` of `ew-viewmodel`: one or two tab groups side by side, at most 32 tabs | Tabs are project view state (doc 16 §2.5), the workspace file is user state (ID-489); keeping them apart keeps `ew.workspace/1` unchanged and deterministic. egui_tiles is already in the graph and gives the tab strip, close buttons and reordering |
| D7 | **Navigation semantics.** Walks (Page Up, Page Down, first, last) move the tab without a history entry; jumps (follow a link, Go to, the navigator's Open, Back and Forward) record one. Each tab remembers a viewport per page; a page seen for the first time in a tab is fitted. A jump to a point centres it at `max(current zoom, 2 × the target page's fit zoom)` | Back returns "with the previous viewport" (FR-UI-019); a jump into a page shown whole would not show where it landed, and twice the fit is a page-relative rule that needs no cell size. The factor is a UX choice to validate at R0 (WP-S2.12) |
| D8 | **Five public items below the shell**, each in its own commit with tests: part 1 `DisplayList::link_sources` (ew-dl), `Session::page_rows`, `Session::page_cells` with `PageCells`, `Session::resolve_goto` and `Session::link_targets` (ew-api); part 2 `Session::page_cps` (ew-api). No new workspace edge and no new external crate | Each is a read query over data the export path already derives; the view model evaluates the cell rule itself from `PageCells`, so a pointer move never calls the `Session` (NFR-PERF-20) |
| D9 | **The preview command protocol is `Session::preview`, a `TxBuilder` dry run on a fork, not a preview model.** A tool's `Preview(cmd)` runs `Session::preview` on the UI thread after the pointer rests 80 ms; the semantic ghost is `Session::derived_diff` of that preview; `Commit(cmd)` commits that preview when it is fresh and its command is equal, else executes the command | ADR-0008: previews are exact because they are the real command; a separate preview model would be a second semantics that can disagree (NFR-REL-08). The transaction-read rule holds: the command reads no memoised query inside its transaction, and the derived diff reads `Q` only over the finished candidate. The `Session` is not `Send`, so the preview runs on the UI thread; a preview slower than NFR-PERF-19 moves to a fork job later (proposed task, §12) |
| D10 | **The tool host and the tools are toolkit-neutral and live in `ew-viewmodel`; `ew-ui` paints.** Doc 16 §4.5 puts the host in `ew-ui`; doc 06 §4.1's `ToolCx` carries a `World`. Here the tool context carries the prepared page tables (display list, entity rectangles, links, cells, grid, connection points) and no `World`, and tools never call the `Session` | What the headless mode cannot do, the shell does not do (ID-443); no `ew-model` name reaches the tools (architecture §3 rule 3). Doc 16 §4.5 and doc 06 §4.1 are amended at the design pass |
| D11 | **Timing is an input, not model state.** Pointer rest is the action `canvas.dwell` with its duration; the window host sends it after 80 ms and after 300 ms of rest, a script sends it explicitly | Transcripts stay free of time (ID-474); the 80 ms semantic ghost and the 300 ms link preview of doc 06 §4.1 and §6 are scriptable |
| D12 | **The headless mode and `ui.describe` grow by the members and words of §6.10 and §7.9** (ID-474 extended; a reviewed transcript change, as ID-511 was) | The blind drafter navigates, follows, goes to, moves the cursor, uses Select and Measure, and reads all of it back |
| D13 | **FT-345 and FT-356 are closed by part 1.** The cell in the status bar uses `PageCells`; hidden tabs keep their tiles within a quarter of the cache | Both waited for tabs and the cell lookup of this package |

## 4. Crates and boundaries

### 4.1 Roles

| Crate | Layer | Part 1 adds | Part 2 adds | Workspace edges |
|---|---|---|---|---|
| `ew-viewmodel` | GPL | `area` (the canvas area: groups, tabs, focus), `history`, `navigator` (the page navigator model replacing `page_list`), the grid cursor and the link list in `canvas`, Go to, the new actions, status fields, transcript members and describe lines | `tools` (the `Tool` trait, `CanvasEvent`, `ToolOutput`, the host, `Selection`, `Snapper`, the Select and Measure tools), the overlay model in sheet coordinates | `ew-api` only (unchanged) |
| `ew-render2d` | GPL | `tiles`: the hidden-scene budget of FT-356; `overlay`: the grid cursor, the link hover outline | `overlay`: the tool overlay primitives (sheet to screen, snapshot compositing) | unchanged |
| `ew-dl` | permissive | `DisplayList::link_sources` (§4.2) | — | unchanged |
| `ew-derive` | GPL | `cell_in_grid` made public for `ew-api` (visibility only, no behaviour change) | — | unchanged |
| `ew-api` | GPL | `PageRow`, `PageCells`, `CellLabel`, `GotoTarget`, `GotoError`, `LinkTarget` in `output`; `Session::page_rows`, `page_cells`, `resolve_goto`, `link_targets` (§4.2) | `PageCp`; `Session::page_cps` | unchanged |
| `ew-ui` | GPL | The canvas-area host (the nested egui_tiles tree, N canvases over one tile pool and cache), the navigator pane replacing the page list, the keyboard contexts, the cursor's AccessKit announcement, `canvas.snapshot` per group | The tool overlay painting, the pointer and key mapping onto tool events, the dwell timer, the link preview popup, `canvas.preview_snapshot` | unchanged |
| `elektrawatt` | GPL | Tests only | Tests only | unchanged |

**Rules the lanes keep** (WP-S2.01's and WP-S2.02's rules stand: through `Session` only and `check-raw-read` green; no
`unsafe`; no unordered output; one clock, `ew_render2d::stats::now`; `ew-render2d` names no egui type):
1. **Only the public items of §4.2** are added below the shell, first and each in its own commit with tests. Any other
   missing item stops the lane.
2. **No new workspace edge and no new package** in `Cargo.lock` (`cargo tree -p elektrawatt` package count equal before
   and after; the lane says so in its commit message).
3. **One mutation path.** Every change of the shell's state is a registry action on `ShellModel::dispatch`; the only
   other mutation stays `set_canvas_backend` (WP-S2.02). Tabs, focus, history, the cursor, tools, selection and previews
   are all dispatched.
4. **Tools never call the `Session`.** Only the host does: `preview`, `preview_with`, `derived_diff`, `commit`,
   `execute_with` and `run_script`.
5. **Nothing timed in the model.** The window host measures pointer rest with `ew_render2d::stats::now` and sends
   `canvas.dwell`; no time value reaches the model, a transcript or a describe line.

### 4.2 The public items added below the shell

| Part | Crate | Item | Contract |
|---|---|---|---|
| 1 | `ew-dl` | `pub fn DisplayList::link_sources(&self) -> Vec<(Destination, Rect)>` | For every item at any depth whose `link` is `Link::Source(d)`: `d` and the axis-aligned bounds, under the enclosing transforms, of the rectangle the PDF writer annotates (a glyph run's `layout_quad`, else the item's control-point bounds; `ew-export`'s `pdf.rs` rule); in paint order (depth first, pre-order); items without a point are left out. MIT OR Apache-2.0 headers |
| 1 | `ew-api` | `pub struct PageRow { pub id: Uuid, pub folio: String, pub title: String, pub kind: String, pub kind_rank: u16, pub generated: bool, pub section: Option<(u32, String)>, pub function: Option<String>, pub location: Option<String>, pub qet_grid: bool }` in `output`; `pub fn Session::page_rows(&self) -> Vec<PageRow>` | One row per entry of the derived page order, in that order (the entries of `Session::page_labels`, same count and folios). `id` is `output::page_uuid` of the entry (a drawn page's ID, a divider's section ID, a generated page's v5 ID); `title` the neutral page title without the folio; `kind` the page type's snake_case spelling and `kind_rank` its position in the schema's `PageType` declaration; `generated` for report pages; `section` the section's number and neutral title (`None` for front matter); `function` and `location` the page's `=` and `+` designations as printed (`None` when unassigned); `qet_grid` when the page's connection grid is the QET grid `7_937_500/3` nm |
| 1 | `ew-api` | `pub struct CellLabel { pub row: String, pub column: String }` (`Display`: row then column, `H15`); `pub struct PageCells` (opaque) with `pub fn cell_at(&self, sheet: (i64, i64)) -> Option<CellLabel>`, `pub fn cell_rect(&self, cell: &CellLabel) -> Option<[i64; 4]>`, `pub fn text(&self, cell: &CellLabel) -> String`, `pub fn pitch_nm(&self) -> (i64, i64)`; `pub fn Session::page_cells(&self, folio: &str) -> Option<PageCells>` | `None` when no page has that folio or the page has no frame grid. `cell_at` maps the sheet point to page coordinates by the inverse of the page frame's sheet map (`PageGrid`'s map: `page = (sheet − offset) × 10⁹ / scale_ppb`, each axis in `i128`, rounded half away from zero) and evaluates `ew_derive`'s exact rule (`cell_in_grid` with the page's connection grid and its 1 nm on-grid window, doc 06 §5.1); `None` outside the drawing area. `cell_rect` is the cell's page rectangle mapped to the sheet, each edge rounded to the nearest nm; row and column labels match ASCII case-insensitively. `text` prints the cell in the project's `ip` cross-reference format for this page (`102-H15`; `{function}` and `{location}` empty, `{page_index}` the page's position). `pitch_nm` is the column width and row height on the sheet, rounded to nm. Evaluating a `PageCells` never touches the `Session` |
| 1 | `ew-api` | `pub struct GotoTarget { pub page: Uuid, pub folio: String, pub cell: Option<CellLabel>, pub rect_nm: Option<[i64; 4]> }`; `pub enum GotoError { Empty, NotFound, NoSuchCell { folio: String, cell: String } }`; `pub fn Session::resolve_goto(&self, text: &str) -> Result<GotoTarget, GotoError>` | `text` trimmed; empty is `Empty`. A text equal to a printed folio label is that page with no cell. Otherwise it is parsed against the project's `ip` format (`{page}` a folio label, `{row}` and `{column}` labels of the target page's frame, `{page_index}` digits naming a position, `{function}` and `{location}` any non-empty text, ignored); a parse naming an existing folio and cell gives the cell and its `cell_rect`; an existing folio with an unknown cell is `NoSuchCell`; anything else `NotFound`. FR-UI-039's examples hold: `103-A1` in the QET-style format, `/103.A1` after the EPLAN cell preset is set |
| 1 | `ew-api` | `pub struct LinkTarget { pub page: Uuid, pub folio: String, pub at_nm: Option<(i64, i64)>, pub cell: Option<String> }`; `pub fn Session::link_targets(&self, dests: &[ew_dl::Destination]) -> Vec<Option<LinkTarget>>` | One result per input, in order. `Representation(r)` and `Cp(c)`: the page whose sheet display list holds the item `Link::Target` of that destination, `at_nm` that item's anchor in sheet nm (the point the PDF's named destination uses), `cell` the anchor's cell printed by `PageCells::text`; `Page(p)`: that page, `at_nm` and `cell` `None`. `None` when the destination is not in this project's page order (an external reference) or does not exist. The implementation may derive the page and point through `Q` (the representation's page, its anchor point, the sheet map) but must not build the display list of a page other than the targets' own; A-check V1 pins the equality with the target items |
| 2 | `ew-api` | `pub struct PageCp { pub id: Uuid, pub label: String, pub at_nm: (i64, i64), pub grid_index: Option<(i64, i64)>, pub connected: bool }`; `pub fn Session::page_cps(&self, folio: &str) -> Vec<PageCp>` | Every connection point drawn on the page: `id` its v5 ID (`Destination::Cp`'s), `label` as `cp_label` prints it (`-102F1:2`), `at_nm` its point on the sheet (`cp_point` under the sheet map, rounded to nm), `grid_index` the index `k` of the page's connection grid when the page point lies within 1 nm of `k × g` (always, by FR-SYM-009, for symbol CPs), `connected` whether any connection ends at it. In reading order (§6.4). Empty for an unknown folio or a page without CPs |

## 5. Dependencies

Neither part adds an external crate or a workspace edge. `ew-ui` already depends on `egui_tiles =0.18.0` (ID-490); the
nested canvas-area tree uses the same version. If WP-S2.02 part 2 has landed when a lane starts, its lyon packages are
in the lock already; the lane's own commits add none. Checks: `cargo xtask check-deps`, `check-features`,
`check-raw-read`, `check-licence-map`, `check-disallowed-methods`, `check-ordered-collections`, `python -m reuse lint`.

## 6. Part 1: the read-only viewer

### 6.1 The page navigator (FR-PAG-024, the viewing part)

The `pages` pane of the workspace (the key stays `pages`; `ew.workspace/1` is unchanged) shows the navigator instead
of the page list. Its model, `ew_viewmodel::navigator::PageNavigator`, is computed from `Session::page_rows` once per
revision and kept in `ShellModel`:

| Field | Values | Default |
|---|---|---|
| `mode` | `flat`, `section`, `structure` | `section` |
| `sort` | `folio` (page order), `type` (`kind_rank`, then page order) | `folio` |
| `filter` | text; a row matches when the filter, case-folded (Unicode simple case folding of `str::to_lowercase`), is a substring of its folio or of its title; parents of a match stay shown and expand | empty |
| `expanded` | the keys of the expanded nodes (a `BTreeSet`) | every node expanded |
| `row` | the key of the row cursor (arrows move it) | the focused tab's page |

**Trees.** `flat`: one level, the pages. `section`: a node per section in section-number order, front matter first under
the node `Front matter`, then the pages. `structure`: function (`=`) nodes, location (`+`) nodes inside them, section
nodes inside those, then the pages; a page without a `function` or `location` sits under the node `(none)` of that
level. Node keys are stable strings (`s:<n>`, `f:<text>`, `l:<text>`, `p:<page uuid>`); nodes are ordered by their first
page in the chosen sort. **Row text:** a page row is `<folio> <title>` with the badges `generated` and `QET grid`
(FR-UI-018, FR-PAG-021: the marker on QET-grid pages) and `current` on the focused tab's page.

**Keyboard** (doc 16 §5.1): Up and Down move the row cursor, Left collapses (or moves to the parent), Right expands,
Enter opens the row's page in the focused tab (a jump, §6.3), Ctrl+Enter in a new tab; Home and End go to the first and
last row. A click opens; a middle click opens in a new tab. Rows are drawn with egui's row virtualisation
(`ScrollArea::show_rows`). **Accessible names:** `"<folio> <title>, page, <badges>, level n, m of k"` and for a node
`"<label>, <section|function|location>, level n, m of k"` (doc 16 §5.1).

Not in part 1: multi-selection, drag to reorder with the renumber preview, type-ahead, facets (§12).

### 6.2 The canvas area: tabs and two pages side by side (FR-UI-035)

`ShellModel` replaces its single canvas, page index and pane with a `CanvasArea`:

| Item | Content |
|---|---|
| `CanvasArea` | `groups: Vec<TabGroup>` (one or two, side by side, left to right), `focused: usize` (the group), `next_tab: u32` (tab IDs from 1 per opened project, never reused in it) |
| `TabGroup` | `tabs: Vec<Tab>` (at least one), `active: usize`, `pane: (f64, f64)` points, `ppp: f64` (default 1200 × 800 at 1.0 headless, per group) |
| `Tab` | `id: u32`, `page: Uuid` (a `PageRow::id`), `canvas: Option<CanvasModel>`, `history: History` (§6.3), `viewports: BTreeMap<Uuid, (Viewport, u64)>` (per-page memory with a visit counter, at most 64 entries, the lowest counter evicted) |
| `Viewport` | `centre: (i64, i64)`, `zoom: f64`, `fitted: bool` (a fitted viewport refits to the pane when restored) |

**Pages are kept by identity.** A tab names its page by `PageRow::id`, so a renumber that changes folios keeps the tab on
its page; after a revision change a tab whose page no longer exists moves to the page at the same position in page order
(clamped), as the shell does today. **Page content is shared:** `ShellModel` keeps one `Arc<CanvasPage>` per (revision,
folio) for the tabs that show it, built once by the existing `sync_canvas` path and dropped at the end of a dispatch
when no tab refers to it; two tabs on one page share it.

**Invariants:** at least one group and one tab while a project is open; at most two groups; at most 32 tabs over both
groups; the focused group's active tab is the shell's current page (`current_folio`, `page_index`, `canvas()` and the
transcript's `page` keep their meaning for it, so the existing tests keep passing).

| Action | Effect | Refused when |
|---|---|---|
| `tab.new` (Ctrl+T) | A new tab after the active one in the focused group, on the same page with the same viewport, and active | 32 tabs (`reason-tabs-full`) |
| `tab.close` (Ctrl+W) | Closes the active tab of the focused group; the tab to its right (else its left) becomes active; closing a group's last tab removes the group and leaves side by side | The project's last tab (`reason-last-tab`) |
| `tab.next`, `tab.previous` (Ctrl+Tab, Ctrl+Shift+Tab) | The next or previous tab of the focused group, wrapping | One tab in the group (`reason-one-tab`) |
| `view.side_by_side` (View, checked while two groups exist) | On: a second group to the right with one new tab on the page after the focused page in page order (the same page when it is the last), focused. Off: the right group's tabs move to the end of the left group, which is focused with its active tab unchanged | 32 tabs when turning on |
| `tab.focus_other` (Ctrl+F6) | Focuses the other group | One group (`reason-one-group`) |
| `tab.activate` (hidden) | `group`, `index`: activate and focus | Out of range (`error-bad-param`) |
| `tab.move` (hidden) | `group`, `index`, `to_group`, `to_index`: move a tab within or between the groups (drag in the tab strip); a group left empty is removed | Out of range; it would leave the project without a tab |
| `tab.open` (hidden) | `folio`, optional `group`: a new tab on that page in that group (default the focused one), active and focused | Unknown folio (`error-no-folio`); 32 tabs |

**The host.** `ew-ui` builds a nested `egui_tiles::Tree` inside the `page-view` pane from `CanvasArea` whenever its
version changes (a counter bumped by every structural change, like `layout_version`): one `Tabs` container per group in a
horizontal `Linear` container with equal shares. Tab titles are `<folio> <title>` with the badges `QET grid` and
`generated`. A tab click dispatches `tab.activate`, a close button `tab.close` (after activating), a tab drag within or
between the groups `tab.move`; any other structural change the tree reports (a drop that would make a third group or a
vertical split) is discarded and the tree rebuilt from the model, so the two-group invariant always holds. Each group
paints its active tab's canvas with that group's pane size (`view.canvas_resize` with `group`). The splitter between the
groups has no accessible name in egui_tiles 0.18.0: the accessible-name walker's skip count adds the nested tree's
`splitter_count()` (ID-490's rule; FT-338 names them later).

### 6.3 Navigation: walks, jumps, history and viewports (FR-UI-019, FR-XRF-019)

**Arrival on a page in a tab:** the tab's remembered viewport of that page if it has one, else the fitted view; the grid
cursor is cleared unless the arrival places it (§6.4, §6.5). **Leaving a page** stores its viewport in the tab's memory.

| Kind | Actions | History |
|---|---|---|
| Walk | `page.next`, `page.previous`, `page.first`, `page.last` (unchanged keys) | none |
| Jump | `nav.follow`, `nav.goto`, `page.goto`, `pages.open` | the current location (page and viewport) is pushed on the tab's back list (at most 50, the oldest dropped) and the forward list is cleared; a jump to the page and viewport already shown records nothing |
| History | `nav.back` (Alt+Left), `nav.forward` (Alt+Right) | `back`: the current location goes on the forward list and the last back entry is restored exactly (a fitted entry refits); `forward` the mirror. An entry whose page no longer exists is dropped and the next one is tried; none left is `reason-history-empty` |

**The jump view.** A jump to a point `p` (a link target's `at_nm`, a cell's centre) sets `zoom' = clamp(max(zoom, 2 ×
fit))`, where `fit` is the fit zoom of the target page in this group's pane, and `centre = p` clamped to the sheet; a jump
to a page without a point (a page destination, Go to a folio, `pages.open`) is an arrival. The previous zoom is the
tab's zoom before the jump. Nothing animates (doc 16 §10.4).

### 6.4 Cross-reference links

**The link list.** When `sync_canvas` builds a `CanvasPage` it also builds `links: Vec<PageLink { dest, rect, target:
Option<LinkTarget> }>` from `DisplayList::link_sources` and one `Session::link_targets` call over the distinct
destinations, in **reading order**: by the cell of the rectangle's centre (column, then row, from the page's `PageCells`;
no cell sorts last), then by `(y, x)` of the centre, then paint order. The same order is the order of `page_cps` (§4.2).

**Hover.** A pointer inside a link rectangle inflated by 4 points at the current zoom hovers that link (smallest area
wins, ties to the later in paint order); a hovered link takes precedence over the entity hover of WP-S2.02 and is drawn
with the same dashed hover outline. The status hint becomes `Click or F: go to <label>` (`<label>` the target's `cell`,
else its folio), or `Link to another project` for a `None` target.

**Follow.** `nav.follow` (F; Page ▸ Follow cross-reference) with optional `x_mm`, `y_mm` follows the link at that
point; without them, the link at the grid cursor when the cursor is shown, else the hovered link. A primary click (press
and release within 3 points) on a link dispatches `nav.follow` with the click point. The jump goes to the target page in
the same tab (a jump, §6.3), centres `at_nm` by the jump view and puts the grid cursor there (§6.6). Refused with
`reason-no-link` without a link, `reason-link-external` for a `None` target.

**Next and previous link.** `nav.next_link` (Shift+F) and `nav.previous_link` (Ctrl+Shift+F) put the grid cursor on the
centre of the next or previous link in reading order after the cursor's position (wrapping; from the view centre when no
cursor is shown), panning by §6.6's rule, so a keyboard user reaches every link without a pointer (NFR-A11Y-01).

### 6.5 Go to page or cell, and the cell in the status bar (FR-UI-039, FT-345)

**Go to.** `nav.goto` (Ctrl+G; Page ▸ Go to page or cell) with `text` resolves through `Session::resolve_goto`: a folio
is a jump to the page (an arrival), a cell a jump that centres `rect_nm`'s centre by the jump view and puts the grid
cursor on the grid point nearest to it. Without `text` the GUI opens the existing go-to dialog (now titled "Go to page or
cell", its field taking either), headless it fails `error-missing-param`. `GotoError` maps to `error-goto-empty`,
`error-goto-not-found` (with the text) and `error-goto-no-cell` (with the folio and the cell). `page.goto` keeps its
`folio` parameter, becomes hidden (no menu entry, no shortcut) and is a jump; the existing scripts keep working.

**The cell.** Each `CanvasPage` carries the page's `PageCells` (fetched with the page content, once per (revision,
folio)). The status bar's cursor field becomes `<cell> · x 255.0 y 172.5 mm` (doc 16 §3.6) with `<cell>` from
`PageCells::text` of the shown point, and only the millimetres where no cell exists; the shown point is the pointer
while it is over the focused group's canvas, else the grid cursor when shown, else the field is left out.

### 6.6 The keyboard grid cursor (FR-UI-015, the cursor part; ruling D5)

`CanvasModel` gains `cursor: Option<(i64, i64)>`, a sheet point in nm. Grid points are those of WP-S2.02's grid overlay:
`offset + scale × k × pitch` of the page's `PageGrid`, rounded to nm.

| Action | Key (canvas context) | Effect |
|---|---|---|
| `cursor.left`, `cursor.right`, `cursor.up`, `cursor.down` | Arrow keys | Without a cursor: the grid point nearest to the view centre. Else snap to the nearest grid point, then one grid step in that direction (clamped to the sheet) |
| `cursor.cell_left`, `cursor.cell_right`, `cursor.cell_up`, `cursor.cell_down` | Shift+Arrow keys | One `PageCells::pitch_nm` in that direction, then the nearest grid point; 10 grid steps on a page without cells |
| `cursor.set` (hidden) | — | `x_mm`, `y_mm`: the nearest grid point; without parameters the cursor is hidden |

**Auto-pan.** After a cursor move, when the cursor's screen point lies outside the pane inset by 10 % on a side, the view
pans by the least amount that brings it to that inset; the zoom never changes. **What resets it:** an arrival on a page
clears it; a jump to a point places it (§6.4, §6.5). **What it reads:** the cell (`PageCells::text`), the millimetres, the
link at the cursor (its rectangle inflated by 4 points contains it) and the entity at the cursor (WP-S2.02's hover pick
at the cursor point).

**Drawing** (`ew-render2d::overlay`): a crosshair of two 1-physical-pixel lines 17 pixels long with a 5-pixel gap at
the centre, in the provisional token `cursor` #D55E00 (3.87:1 on white; the crosshair shape, not the hue, carries the
meaning, doc 16 §10.3), above every other overlay.

**Keyboard contexts.** The registry gains a context per action: `global` (every existing action), `canvas` (the cursor
actions, `nav.follow`, `nav.next_link`, `nav.previous_link`) and `navigator` (its keys, §6.1). The window binds a
context's chords only while the model's focus is in that context (single keys never fire while a text field has
focus, WP-S2.01's routing). `ShellModel` gains `focus: Focus { Navigator, Canvas(group) }`, set by `ui.focus` (hidden,
`area`: `navigator` or `canvas`, optional `group`), by F6 and Shift+F6 (`ui.focus_next`, `ui.focus_previous`: navigator,
canvas group 1, canvas group 2, cyclic) and by a click into a pane; it is the canvas of the focused group when a project
opens. `ew-ui` gives egui's keyboard focus to the canvas node or the navigator list from the model's focus each frame,
and dispatches `ui.focus` when egui's own traversal lands on one of them. Headless scripts start with the canvas
focused, so canvas actions run without a focus step.

### 6.7 Actions and input

The new and changed registry actions of part 1 (label keys and English texts in `ew_viewmodel::strings`; kind `View`
for all):

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `tab.new`, `tab.close`, `tab.next`, `tab.previous` | Window | Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab | global | — |
| `view.side_by_side` | View (checked) | — | global | — |
| `tab.focus_other` | Window | Ctrl+F6 | global | — |
| `tab.activate`, `tab.move`, `tab.open` | hidden | — | global | §6.2 |
| `nav.back`, `nav.forward` | Page | Alt+Left, Alt+Right | global | — |
| `nav.follow` | Page | F | canvas | optional `x_mm`, `y_mm` |
| `nav.next_link`, `nav.previous_link` | Page | Shift+F, Ctrl+Shift+F | canvas | — |
| `nav.goto` | Page ("Go to page or cell") | Ctrl+G (moved from `page.goto`) | global | `text` |
| `page.goto` | hidden (was Page, Ctrl+G) | — | global | `folio` (unchanged) |
| `cursor.left` … `cursor.down` | hidden | Arrow keys | canvas | — |
| `cursor.cell_left` … `cursor.cell_down` | hidden | Shift+Arrow keys | canvas | — |
| `cursor.set` | hidden | — | canvas | optional `x_mm`, `y_mm` |
| `pages.mode` | View ▸ Page navigator (three checked items) | — | global | `mode` |
| `pages.sort` | View ▸ Page navigator (two checked items) | — | global | `by` |
| `pages.filter` | hidden (the navigator's filter field) | — | global | `text` |
| `pages.toggle`, `pages.row` | hidden (Left, Right; Up, Down, Home, End) | navigator keys | navigator | `key`; `pages.row` takes `key` or `step` (−1, 1) or `to` (`first`, `last`) |
| `pages.open` | hidden (Enter, Ctrl+Enter, click) | navigator keys | navigator | `key` (a page row's key) or `folio`, optional `new_tab` |
| `ui.focus`, `ui.focus_next`, `ui.focus_previous` | hidden; F6, Shift+F6 | — | global | `area`, optional `group` |
| `view.canvas_resize`, `view.zoom_to`, `view.pan`, `canvas.pointer`, `canvas.snapshot` and every other canvas view action | unchanged | unchanged | unchanged | each gains an optional `group` (0 or 1; default the focused group) |

The Window menu is new (doc 16 §3.2): the four tab actions, `tab.focus_other` and the list of open tabs (one checked
item per tab of the focused group, dispatching `tab.activate`). Every new action is in the palette (hidden ones are not,
as today). A canvas view action and `canvas.pointer` with `group` act on that group's active tab; `canvas.pointer` on one
group clears the pointer of the other. `clears_banner` gains every new action except `nav.goto`, `page.goto` and
`tab.open` (navigation must not dismiss a banner; the three that can fail with a message clear it as before).

**Pointer input** added to WP-S2.02's mapping: a primary click on a link is `nav.follow`; a primary press anywhere in a
group's canvas dispatches `ui.focus` for that group first; the mouse's back and forward buttons are `nav.back` and
`nav.forward`.

### 6.8 The host: several canvases over one tile pool (FT-356)

`ew-ui`'s `CanvasHost` serves every visible canvas (one per group) from one `TilePool` and one `TileCache`: scenes are
keyed by (revision, folio) as today, so two tabs on one page share tiles. Each frame the host plans the visible canvases
in the order focused group, then the other, and submits one merged plan (the focused group's tiles first in each
priority class of WP-S2.02 §6.3), with the upload limit of 8 per frame shared.

**Retention (FT-356).** A scene not drawn in the current frame is *hidden*. The cache never evicts a tile drawn in the
current frame; above the 256 MiB budget it evicts hidden scenes' tiles first, least recently shown scene first, then
visible tiles by the existing least-recently-used order; hidden scenes together keep at most 64 MiB (a quarter of the
budget), evicted the same way. A scene no open tab shows any more (its tab closed, or a new revision for the same folio
arrived and drew) is dropped at once. With no canvas on screen (the wizard, the start page) the host keeps the
cycle-21 rule: an empty plan and no scenes. The `CanvasCounters` of ID-519 are kept per frame for all canvases.

`canvas.snapshot` takes `group` (default focused) and writes that group's pane as WP-S2.02 §6.6 defines, now including
the grid cursor and the link hover outline.

### 6.9 Status bar and accessibility

**Status bar** (left to right, changes only): the hint (`Read-only view`, or the link hint of §6.4), the page `<folio>
(<n> of <total>)` of the focused tab, the cursor field of §6.5, then WP-S2.02's fields unchanged.

**AccessKit** (doc 16 §10.1; NFR-A11Y-02): each visible canvas is one focusable node, role `Canvas`, name `Page <folio>,
<title>`, description `<hint>, zoom <zoom>` and, while the grid cursor is shown, `, cursor <cell> x <x> y <y> mm` (one
decimal). A child node with role `Status`, name `Canvas cursor` and polite live updates carries, after each cursor move
or jump, `<cell>, x <x> y <y> mm` followed by `, link to <label>` when a link is at the cursor. The tab strip exposes
`TabList` and `Tab` nodes with the tab titles; the navigator `Tree` and `TreeItem` nodes with the names of §6.1. The
walker passes after every kittest step with the extended skip count of §6.2.

### 6.10 The headless mode

**Transcript** (`state`, members appended in this order; a reviewed transcript change, every expected state in the
existing tests gains them):

| Member | Content |
|---|---|
| `canvas` (extended) | After `statistics`: `cursor_nm` (`[x, y]` or `null`), `cell` (the status cursor field's cell text or `null`), `link` (the hovered link: `{"folio": "103", "cell": "103-A1"}` with `cell` possibly `null`, `{"external": true}`, or `null`) |
| `tabs` (new, after `canvas`) | `null` without a project; else `{"focused": [g, i], "groups": [[{"id": 1, "folio": "102", "back": 1, "forward": 0}, …], …]}`, `g` and `i` 0-based |
| `focus` (new) | `"navigator"`, `"canvas"` or `null` without a project |
| `navigator` (new) | `null` without a project; else `{"mode": "section", "sort": "folio", "filter": "", "row": "<key>" or null}` |

**`ui.describe`**: the `page list` lines are replaced by the navigator lines, and tab lines precede the `page view:`
line; the canvas lines describe the focused group's canvas and gain two lines; every new line is listed here (indent two
spaces under its header line; texts in quotes are user text):

```text
navigator "Pages" mode section sort folio filter ""
  row level 1 "Front matter" expanded
  row level 2 page "1" "Title Page" current
  row level 2 page "2" "Table of Contents" generated
  row level 2 page "101" "Power Supply 1" badge "QET grid" cursor
tabs group 1 of 2 focused
  tab "102" "Power Supply 2" selected history back 1 forward 0
  tab "103" "…"
tabs group 2 of 2
  tab "103" "…" selected history back 0 forward 0
focus canvas group 1
canvas pointer 255.000 172.500 mm cell "102-H15"        (the existing line, with the cell when one exists)
canvas cursor 255.000 172.500 mm cell "102-H15"         (or: canvas cursor none)
canvas link "103-A1" folio "103"                        (or: canvas link external; canvas link none)
```

A collapsed node ends in `collapsed` and its rows are not listed; the navigator lists only the rows the filter keeps.
`VOCABULARY` gains `navigator`, `mode`, `flat`, `section`, `structure`, `sort`, `folio`, `type`, `filter`, `row`,
`level`, `expanded`, `collapsed`, `current`, `generated`, `badge`, `cursor`, `tab`, `group`, `focused`, `history`,
`back`, `forward`, `focus`, `cell`, `link` and `external` (27 words; ID-474, ID-493); FT-314's harness help lists them.

## 7. Part 2: the tool framework

### 7.1 The tool contract (`ew_viewmodel::tools`)

```rust
pub enum CanvasEvent {
    PointerMove { at: (i64, i64), mods: Mods },              // sheet nm
    Press { at: (i64, i64), button: Button, mods: Mods },    // Button::Primary only reaches tools
    Release { at: (i64, i64), button: Button, mods: Mods },
    Dwell { ms: u32 },                                       // pointer or cursor at rest (D11)
    CursorMove { at: (i64, i64) },                           // the grid cursor moved (§7.7)
    Activate,                                                // Enter: a click at the grid cursor
    Back,                                                    // Backspace: the tool's own step back
    Escape,
}
pub struct Mods { pub shift: bool, pub ctrl: bool, pub alt: bool }
pub enum ToolOutput {
    None,
    Transient(Overlay),                       // replaces the overlay
    Preview(Box<dyn DynCommand>),             // evaluated by the host on a fork (§7.3)
    Commit(Box<dyn DynCommand>),              // handed to the host (§7.3)
    Cancel,                                   // drop overlay and preview; revision unchanged
    Select(SelectionChange),                  // the Select tool's result (no command)
    Follow(Destination),                      // a click on a link (§6.4 behaviour kept)
}
pub trait Tool {
    fn name(&self) -> &'static str;                         // "select", "measure"
    fn state(&self) -> &'static str;                        // "idle", "pressed", "rubber-band", "armed", "measuring"
    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput;
    fn status_hint(&self) -> &'static str;                  // a key of `strings`
    fn wants_hover(&self) -> bool;                          // hover, link hover and link preview are shown
}
pub struct ToolCx<'a> {
    pub page: &'a CanvasPage,                 // display list, entity rectangles, links, cells, grid, CPs (§7.5)
    pub points_per_nm: f64, pub pane: (f64, f64),
    pub snapper: &'a Snapper,
    pub selection: &'a Selection,
}
```

`DynCommand` is `ew_api::DynCommand`. Doc 06 §4.1's `ToolCx` "World snapshot" is replaced by the prepared page (D10);
`Select` and `Follow` are added to doc 06 §4.1's outputs. One output per event (doc 06 §4.1).

### 7.2 The tool host

`ShellModel` keeps one active tool per window, acting on the focused group's canvas (activating a tool cancels the
current one; focusing the other group sends `Escape` to a tool that is not in its first state). Default tool: Select.

| Output | Host action |
|---|---|
| `None` | Nothing |
| `Transient(o)` | The overlay becomes `o` (sheet coordinates, §7.4) |
| `Preview(cmd)` | §7.3; the overlay is kept |
| `Commit(cmd)` | §7.3; then the overlay and preview are cleared |
| `Cancel` | The overlay and preview are cleared; the revision is unchanged |
| `Select(change)` | The selection changes (§7.6); the overlay is cleared |
| `Follow(d)` | `nav.follow` for the destination's link (§6.4); the overlay is cleared |

**Escape** (doc 16 §4.1): `ui.dismiss` closes the topmost popup or dialog; else it sends `Escape` to the active tool;
when the tool answers `None` (it was in its first state), a tool other than Select is replaced by Select, and Select
with a selection clears it. The status hint is the active tool's `status_hint`; a state change is announced through the
canvas node's polite live region: `<tool label>: <hint text>`.

### 7.3 The preview command protocol (D9)

- **Preview.** On `Preview(cmd)` the host runs `Session::preview_with(cmd, submit)` once on the UI thread, with
  `submit` the GUI origin (`Origin::Gui`) in the window and `Origin::CmdLog` with the script's file name headless, and
  keeps `PreviewState { base_rev, command: cmd.to_json(), name, result: Ok(Preview) or Err(message), derived:
  DerivedDiff }`. A tool emits `Preview` only on a `Dwell` of at least 80 ms (doc 06 §4.1's semantic ghost); the host
  ignores a `Preview` whose command JSON and base revision equal the kept one.
- **The semantic ghost** is the kept preview's `DerivedDiff` (changed tags and cross-reference texts), drawn as text
  chips by the overlay (§7.4) and read back by the transcript; a preview whose `base_rev` is not the current revision is
  **stale**: drawn with the `stale` badge (doc 16 §10.3) and recomputed on the next dwell.
- **Commit.** In the window: when the kept preview is fresh and its command JSON equals `cmd.to_json()`,
  `Session::commit(preview)` (the rebase check of ADR-0008 amendment 2), else `Session::execute_with(cmd,
  Submit::from(Origin::Gui))`, and on `CmdError::NeedsPreview` a preview followed by its commit. Headless: always as a
  one-line command log through `Session::run_script`, exactly the `command.run` path, so the exposure rules (a
  `HumanOnly` command is refused for a scripted origin, ID-443) and `--record` apply unchanged. Either way the result
  equals executing the command on the current revision (preview + commit = execute, NFR-REL-08).
- **The transaction-read rule holds.** The command runs in its transaction as everywhere else (no memoised query read,
  ADR-0008 amendment 4); `derived_diff` reads `Q` only over the finished candidate, outside any transaction.
- **Cost.** A preview runs on the UI thread because the `Session` is not `Send`; its time is not measured in this
  package (ID-512). Moving previews to a fork job is a proposed follow-up (§12).

### 7.4 The overlay layer

`Overlay` (toolkit-neutral, `ew-viewmodel`) is a `Vec<OverlayItem>` in sheet nm, drawn in order above the hover and
selection outlines and below the snap indicator and the grid cursor:

| Item | Drawn as (screen space, `ew-render2d::overlay`) |
|---|---|
| `Line { a, b, style }`, `Polyline { points, style }`, `Rect { r, style }` | 1-pixel lines (2 at `ppp ≥ 2`); `style` `solid` or `dashed` (4 on, 4 off) in one of the tokens below |
| `Ghost { rects, offset }` | The rectangles of `rects` (entity rectangles) translated by `offset`, dashed in `ghost` (the geometric ghost of a drag, backend-neutral) |
| `Marker { at, kind }` | `grid`: a 7-pixel cross; `cp`: a 9-pixel ring; `junction`: a 7-pixel filled square; in `snap` |
| `Label { at, text, anchor }` | A text chip in the chrome font (the readout of Measure, a ghost's predicted tag), drawn by `ew-ui`; listed in `ui.describe`, never composited into `canvas.snapshot` |

Provisional tokens (constants of `ew-render2d`, replaced by WP-S2.08's theme): `ghost` #0072B2, `snap` #009E73 (3.42:1),
`measure` #A86F00 (4.25:1), `rubber-band` #0072B2 (solid while it encloses, dashed while it touches); the shapes carry
the meaning (NFR-A11Y-03). The headless `canvas.snapshot` composites every item but labels by integer pixel fills.

### 7.5 Snapping and its indicators (FR-UI-018, doc 06 §5.7)

`Snapper` is built per `CanvasPage` from its `PageGrid` and the page's `Session::page_cps` (fetched with the page content,
once per (revision, folio)). `snap(at, points_per_nm, alt) -> Snap { at_nm, page_nm, grid_index, kind, label }`:

1. Unless `alt` is held, the CP nearest to `at` within 8 screen points (squared distance in `i128`, ties by reading
   order) wins: `kind = cp`, `label` its `PageCp::label`, `at_nm` its point, `grid_index` its index.
2. Else the grid point nearest to `at`: `k = round((page(at) × den) / num)` per axis on the page's rational grid in
   `i128` (half away from zero), `page_nm = round(k × num / den)`, `at_nm` its sheet point; `kind = grid`.

All snapping is on grid indices (ADR-0005; doc 06 §5.7); `page_nm` is what a command would receive, on the page's
connection grid, so a tool never meets the grid rule's rejection (ID-54). The tool decides when to snap and shows the
result as a `Marker` in its overlay; the transcript's `snap` member reports the marker present (§7.9).

### 7.6 The shipped tools and the probe

**Select** (`select`; doc 06 §4.3 without the drag-move rows, which are WP-S2.04's):

| State | Event | Next | Output |
|---|---|---|---|
| idle | press on a link | idle | `Follow` on release within 3 points |
| idle | press on an entity | pressed | `None` |
| idle | press on empty canvas | rubber-band | `Transient` (the band) |
| pressed | release within 3 points | idle | `Select`: replace with the entity; Shift adds; Ctrl toggles |
| pressed | move beyond 3 points | rubber-band | `Transient` |
| rubber-band | move | rubber-band | `Transient`: solid while the drag goes right (encloses), dashed while it goes left (touches) |
| rubber-band | release | idle | `Select`: left to right the entities whose rectangle lies inside the band, right to left those that intersect it; Shift adds |
| any | `Activate` | as a press and release at the grid cursor | — |
| any | `Escape` | idle | `Cancel`, or `None` from idle |

Picking uses WP-S2.02's entity rectangles (smallest area containing the point inflated by 4 points, ties to the later in
paint order). `Selection { items: Vec<EntityRef> (no duplicates, in selection order), primary: Option<EntityRef>,
generation: u64 }` is per window, survives page and tab changes, and is drawn on the shown page as WP-S2.02's
`selected` outline (2-pixel solid #0072B2) around each selected entity's rectangles. The canvas node's live region
announces `<n> selected` on a change. Typed entity references, the per-tool pick priorities and the rich announcement
of doc 16 §4.6 are proposed follow-ups (§12).

**Measure** (`measure`): armed → `Press` or `Activate` at A (snapped) → measuring: every move `Transient` with a
`measure` line A to B (snapped), the markers and a label `dx <dx> dy <dy> mm, <d> mm` (sheet millimetres, three
decimals, `d` the Euclidean distance rounded to nm with `i128::isqrt` of the squared sum) → `Press` or `Activate` fixes
B and returns to armed, keeping the overlay until the next move → `Escape` from measuring returns to armed, from armed
`None` (and the host returns to Select). Measure never emits `Preview` or `Commit`.

**The probe** (`tests/` only, never in the registry): a tool that on a drag of a selected representation emits
`Transient(Ghost)`, on `Dwell(80)` `Preview` of `rep.move` by the snapped offset, on release `Commit` of the same
command, and on `Escape` `Cancel`. The tests register it through `ShellModel::set_tool_for_tests`, which exists only
with the cargo feature `test-tools` of `ew-viewmodel` (`#[cfg(feature = "test-tools")]`); the probe's tests are a test
target with `required-features = ["test-tools"]`, and no binary enables the feature. WP-S2.04's move gesture follows
the probe's pattern.

### 7.7 Keyboard tool input (FR-UI-015, NFR-A11Y-01)

The grid cursor of part 1 is the tool's keyboard pointer: every cursor move also sends `CursorMove` to the active tool,
and:

| Action | Key (canvas context) | Effect |
|---|---|---|
| `cp.next`, `cp.previous` | `]`, `[` (the physical keys, so they need no AltGr on QWERTZ and AZERTY, doc 16 §4.4); also Alt+N and Alt+Shift+N | The cursor goes to the next or previous CP of `page_cps` in reading order after the cursor's position (wrapping) |
| `cp.next_unconnected` | N | The next CP with `connected = false` |
| `canvas.activate` | Enter | `Activate` to the tool |
| `tool.back` | Backspace | `Back` to the tool |
| `tool.select`, `tool.measure` | Tools menu (no default keys) | Activate the tool |

The describe text names the CP at the cursor (`canvas cursor … cp "-102F1:2"`), so a blind drafter can reach and name
every connection point with the keyboard alone.

### 7.8 The link hover preview (FR-UI-019, the hover part)

While the active tool `wants_hover`, a `canvas.dwell` of at least 300 ms with a hovered link sets `link_preview` to that
link's target; any pointer move, cursor move or view action clears it. The window shows a popup next to the pointer: the
target page's region of 320 × 200 points around `at_nm` at the jump view's zoom for that size, rendered on the tile
pool's workers by `render_region` from the target page's content (fetched once per (revision, folio) through the
shell's page-content cache), with the page bounds and a `cursor` crosshair at the target. Headless, the hidden action
`canvas.preview_snapshot` with `path` writes the same image as PNG (refused without a preview, `reason-no-preview`).
`canvas.dwell` itself is hidden (`ms`, a positive integer).

### 7.9 Actions, transcript and describe of part 2

New hidden actions with their parameters: `canvas.press`, `canvas.release` (`x_mm`, `y_mm`, optional `button`
`primary` or `middle`, optional `shift`, `ctrl`, `alt`, optional `group`), `canvas.dwell` (`ms`), and
`canvas.preview_snapshot` (`path`); `canvas.pointer` gains the optional `shift`, `ctrl`, `alt`. Visible: `tool.select`,
`tool.measure` (Tools), `cp.next`, `cp.previous`, `cp.next_unconnected`, `canvas.activate`, `tool.back` (§7.7). The
window maps a primary press and release onto `canvas.press` and `canvas.release` (the middle button keeps panning,
WP-S2.02), and sends `canvas.dwell` after 80 ms and after 300 ms of rest.

**Transcript:** `canvas` gains, after `link`: `tool` (`{"name": "select", "state": "idle"}`), `snap` (`{"kind": "cp",
"at_nm": [x, y], "label": "-102F1:2"}` or `null`), `selection` (the UUIDs as strings, in selection order), `preview`
(`{"command": "rep.move", "state": "ready" | "stale" | "failed", "tags": n, "xrefs": n}` or `null`), `overlay` (the
number of overlay items) and `link_preview` (the target's folio or `null`).

**`ui.describe`** gains, after the canvas lines:

```text
canvas tool "Select" state idle hint "Click to select; drag for a rubber band"
canvas snap cp "-102F1:2" at 255.000 172.500 mm          (or: canvas snap grid at … mm; canvas snap none)
canvas selection 2 "<uuid>" "<uuid>"                       (or: canvas selection none)
canvas preview ready "rep.move" tags 1 xrefs 2             (or stale, failed with error: <message>; or canvas preview none)
canvas overlay 3 label "dx 12.700 dy 0.000 mm, 12.700 mm"  (one line per label; or canvas overlay 0)
canvas link preview "103"                                  (only while shown)
```

The tool's label and hint are quoted user text. `VOCABULARY` gains `tool`, `state`, `idle`, `pressed`, `rubber-band`,
`armed`, `measuring`, `hint`, `snap`, `cp`, `at`, `selection`, `stale`, `failed`, `tags`, `xrefs`, `overlay` and `label`
(18 words; `preview`, `ready`, `none`, `grid`, `mm` and `link` exist).

## 8. Determinism, the headless mode and exact counts

1. **Every flow is a script.** Each acceptance flow of §10 that runs in kittest also runs as an `ew.uiscript/1` script
   through the built binary, with equal transcripts on two runs (V12, T10). No timer, no clock and no hash order reaches
   the model; the dwell is an action (D11).
2. **Snapshots** stay CPU images (WP-S2.02 D4): `canvas.snapshot` with the cursor, the hover, the selection and the
   overlay (without labels), and `canvas.preview_snapshot`, are byte-identical on equal inputs per build target.
3. **Exact counts** (NFR-PERF-20; WP-S2.02 A9's counters): after each page's first frame, pointer moves (over links
   included), cursor moves, link and CP jumps within a page, tab switches between pages already shown at the same
   revision, side by side on and off, Back and Forward between pages already shown, and Select and Measure events cause
   0 memo re-executions, 0 further `page_content` calls, 0 further `link_targets`, `page_cells` or `page_cps` calls, and
   no tile requested twice while cached; a preview calls `Session::preview_with` exactly once per dwell that changes the
   command or the revision.
4. **No time values, paths or process IDs** in transcripts or describe text; IDs are tab counters and UUIDs.

## 9. Order of work and the parallel WP-S2.02 part 2

Part 1 starts from the integration tip of cycle 22. If WP-S2.02 part 2 (the GPU path, the switch rule, FT-349) has been
integrated, the lane keeps its behaviour per canvas: one offscreen target per visible canvas group, the switch rule per
process, FT-349's UI-scale divisor in every group's `CanvasModel`; the tile path of §6.8 stays the fallback and the
loading state. If it has not, the lane builds on WP-S2.02 part 1's CPU canvas and the GPU lane rebases onto part 1 at
its own integration. Nothing in this specification uses a GPU item, and every acceptance check runs on the CPU canvas
(no render state in kittest or headless). Part 2 starts after part 1's delivery and review.

## 10. Acceptance checks

All tests are ordinary nextest tests in the existing test jobs, on temporary copies of Golden A with `EW_CONFIG_DIR` set
to a temporary directory. The golden examples named below come from the requirement rows; if one is not in the as-built
display lists (a link that is not there, a cell that differs), the lane stops and reports it.

**Part 1**

| # | Check | Where | Proves |
|---|---|---|---|
| V1 | Public items: `link_sources` exact under nested transforms and for a glyph run's `layout_quad`, equal to the PDF writer's annotation rectangles on folios 101 to 103; `page_rows` equals `page_labels` in count and folios on Golden A, with the generated flag, sections, `qet_grid` and `kind_rank`; `page_cells` on the QET frame: `cell_at` of the L-1 anchor gives H16, column 15 spans x 249.9 to 265.6 mm (doc 16 §3.6) at its edges, a boundary point belongs to the right and lower cell, outside the drawing area `None`, `cell_rect` round-trips through `cell_at`, `text` prints `102-H15`; `resolve_goto` for a folio, `103-A1`, `103-a1`, the three errors, and `/103.A1` after `project.settings.set` switches the `ip` format to the EPLAN cell preset (and `103-A1` then `NotFound`); `link_targets` resolves every link source of folios 101 to 103, each `at_nm` equal to the anchor of the matching `Link::Target` item, external references `None`; FR-XRF-019's "103-G6" at PE-1.1 targets folio 103 cell G6 | `ew-dl`, `ew-api` tests | §4.2 |
| V2 | Canvas area: the invariants of §6.2 under every tab action (new, close, next, previous, activate, move, open, side by side on and off, focus other), the refusals, IDs never reused, the page kept by identity across a `page.renumber` on a temporary copy | `ew-viewmodel` unit tests | §6.2 |
| V3 | History and viewports: walks record nothing; jumps record one entry; Back restores the exact viewport (a fitted one refits after a resize); Forward; the cap of 50; a vanished page skipped; the per-page memory and its cap of 64; the jump view's zoom rule | `ew-viewmodel` unit tests | §6.3 |
| V4 | Navigator: the three modes and two sorts on Golden A against an oracle computed from `page_rows` by §6.1's rules; the filter keeps ancestors; keys stable across revisions; the row cursor keys; accessible names | `ew-viewmodel` tests | §6.1 |
| V5 | Grid cursor: first placement, grid steps under the QET sheet map (every position a grid point of the overlay), cell steps, auto-pan's least move, reset on arrival, placement on a jump, link and entity at the cursor | `ew-viewmodel` unit tests | §6.6 |
| V6 | Headless follow and back (FR-UI-019, AC-28's first part): open, `page.goto` 102, zoom to 200 % and pan, `nav.follow` at the "103-A1" text of 24VDC-1: the tab shows 103, `cell` reads `103-A1`, the cursor is at the target; `nav.back` restores 102 with the exact `view_nm`; `nav.forward` returns. Likewise "102-A1" from 101-H16 | `bin/elektrawatt/tests` | §6.3, §6.4 |
| V7 | Headless go to (FR-UI-039): `nav.goto` `103-A1` centres A1's rectangle; `103` arrives fitted; the errors; `/103.A1` after the preset switch | `bin/elektrawatt/tests` | §6.5 |
| V8 | Headless side by side (FR-UI-035): 102 left and 103 right, different zooms and pans per group, a pointer on each: both `view_nm` kept independently; tab switches and Back keep each tab's viewport; `canvas.snapshot` per group | `bin/elektrawatt/tests` | §6.2, §6.8 |
| V9 | Transcript and describe: the members of §6.10 in order, the navigator, tab, focus, cursor and link lines; the vocabulary test passes with the 27 words; existing expected states updated | `ew-viewmodel` tests; `bin/elektrawatt/tests` | §6.10 |
| V10 | Exact counts of §8 item 3 for part 1, with WP-S2.02 A9's counters and a counter per new `Session` call: 200 mixed steps over two tabs and both groups | `ew-ui` test | §8 |
| V11 | FT-356: hidden scenes' tiles evicted first and least recently shown first; their total never above 64 MiB; a tile drawn this frame never evicted; returning to a hidden tab within the budget rasters no tile; closing the last tab of a page drops its scene | `ew-render2d` unit tests; `ew-ui` test | §6.8 |
| V12 | kittest without pointer events (NFR-A11Y-01's viewing part): Ctrl+G, type `103-A1`, Enter: 103 shown and the status field reads `103-A1 · x …`; Shift+F to a link, F, Alt+Left back; Ctrl+T, Ctrl+Tab, Ctrl+W; View ▸ Side by side, Ctrl+F6; F6 to the navigator, Down, Enter; arrows move the cursor and the canvas node's description and live region change; the walker passes after each step; the two scripts of V6 and V8 twice give byte-identical transcripts | `ew-ui` tests; `bin/elektrawatt/tests` | §6.6 to §6.9, §8 |
| V13 | Policy: `check-deps` (no new edge), `check-features`, `check-raw-read`, `check-licence-map`, `check-disallowed-methods`, `check-ordered-collections`, `reuse lint`; no new package in `Cargo.lock`; `unsafe` only in `forbid` attributes | lane | §4, §5 |
| V14 | Targeted tests and lints of the changed crates (`ew-dl`, `ew-derive`, `ew-api`, `ew-render2d`, `ew-viewmodel`, `ew-ui`, `elektrawatt`): `cargo nextest run -p …` with and without `--all-features`, `cargo clippy -p … --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`, `cargo doc -p ew-api -p ew-viewmodel -p ew-ui --no-deps` with `-D warnings` | lane | testing policy |
| V15 | Manual on the lane's OS, reported and not gated: the navigator, tabs, side by side, a follow and Back, Go to and the cursor in the window | lane or orchestrator | — |

**Part 2**

| # | Check | Where | Proves |
|---|---|---|---|
| T1 | Host: every output of §7.2 with a scripted tool; activating a tool cancels the current one; focusing the other group escapes a busy tool; the Escape chain of §7.2 (popup, tool, Select, selection) | `ew-viewmodel` tests | §7.2 |
| T2 | Preview protocol with the probe on folio 102: drag, `Dwell(80)` gives one `preview_with`, a repeat dwell none; the derived diff in the state; a commit by another script step makes it stale; release commits with revision + 1 through `commit(preview)` in GUI mode and through `run_script` headless (the `--record` log holds the line, origin `CmdLog`); undo restores; a `HumanOnly` command is refused headless | `ew-viewmodel` tests (feature `test-tools`) | §7.3 |
| T3 | FR-UI-016's acceptance: the probe drags a symbol on folio 102 and Escape leaves the revision, the history cursor and the dirty flag unchanged, with no preview and an empty overlay | `ew-viewmodel` tests | §7.2, §7.3 |
| T4 | Select: click, Shift and Ctrl clicks; left-to-right and right-to-left bands on folio 103 against an oracle over the entity rectangles; a click on a link follows; selection kept across tab changes; the selected outline drawn | `ew-viewmodel`, `ew-render2d` tests | §7.6 |
| T5 | Snapping: grid snaps on the QET grid under the sheet map are exact grid indices (`page_nm` within 1 nm of `k × g`); a CP within 8 points wins, beyond it the grid; Alt suppresses CP snaps; `page_cps` on folio 102 contains -102F1:2 at the point the display list draws it, in reading order | `ew-api`, `ew-viewmodel` tests | §4.2, §7.5 |
| T6 | Keyboard tool input: `]`, `[`, N cycle in reading order and wrap; Enter selects the entity at the cursor; Measure from the keyboard alone gives the exact distance between two CPs | `ew-viewmodel` tests; kittest | §7.7 |
| T7 | Overlay: the sheet-to-screen mapping of every item; `canvas.snapshot` composites ghost, markers, band and cursor deterministically, labels excluded | `ew-render2d` tests; `bin/elektrawatt/tests` | §7.4 |
| T8 | Link hover preview: `canvas.dwell` 300 over a link shows it, a pointer move clears it, 299 does not; `canvas.preview_snapshot` byte-identical twice; one `page_content` call for the target folio, none on a second preview of it | `ew-viewmodel`, `bin/elektrawatt/tests`, `ew-ui` | §7.8 |
| T9 | Escape safety (doc 16 §10.5): every registered tool cancels with Escape in every state and the revision is unchanged | `ew-viewmodel` tests | NFR-UX-07 |
| T10 | Transcript and describe of §7.9; the 18 words; a Select-and-Measure script twice byte-identical; kittest of the same flow without pointer events, walker passing | `ew-viewmodel`, `ew-ui`, `bin/elektrawatt/tests` | §7.9, §8 |
| T11 | Exact counts of §8 item 3 for part 2 | `ew-ui` test | §8 |
| T12 | Part 1's V1 to V14 still pass; policy and targeted checks as V13 and V14 (adding `--features test-tools` for `ew-viewmodel`) | lane | — |

The workspace suite, the acceptance runner and the performance runs are not run by the lanes (testing policy, ID-512);
the integrating session runs `cargo xtask ci` at the cycle delivery.

## 11. Files owned

**Part 1:** `crates/ew-dl/src/item.rs` and its tests (`link_sources`); `crates/ew-derive/src/cell.rs` (the visibility of
`cell_in_grid` only); `crates/ew-api/src/output/mod.rs` and a new `crates/ew-api/src/output/navigation.rs` with tests
(`PageRow`, `PageCells`, `CellLabel`, `GotoTarget`, `GotoError`, `LinkTarget` and the four `Session` methods);
`crates/ew-render2d/src/{tiles.rs,overlay.rs}` and tests; `crates/ew-viewmodel/src/**` and tests (new `area.rs`,
`history.rs`, `navigator.rs`; `page_list.rs` removed; `canvas`, `shell`, `shell/canvas_actions`, `actions`, `status`,
`strings`, `describe`, `script`, `layout` (titles only)); `crates/ew-ui/**` (the canvas-area host, the navigator pane,
the keyboard contexts, `headless.rs`, `benches/canvas.rs` only as far as the multi-canvas host changes its calls);
`bin/elektrawatt/tests/**`. **Part 2:** additionally `crates/ew-api/src/output/navigation.rs` (`PageCp`,
`Session::page_cps`); `crates/ew-viewmodel/src/tools/**` and its `Cargo.toml` (the `test-tools` feature, with
`ci/dep-rules.toml`'s feature rules if `check-features` requires an entry); `crates/ew-render2d/src/overlay.rs`.

Read-only for the lanes: every other crate, `ci/licence-map.toml`, `deny.toml`, `supply-chain/**`, `.github/**`,
`docs/**` (the design pass updates the documents of §14). Each lane commits on its own branch and does not merge or
push.

## 12. Out of scope

| Item | Where it goes |
|---|---|
| Placing, moving, rotating, wiring, drawing; drag commits; rubber-band connections while dragging; the inline dimension readout of a drag | WP-S2.04 (the overlay, preview and snap APIs of part 2 are its base) |
| Typed selection references (doc 16 §2.3), `page_spatial_index` with exact kurbo picking and the per-tool pick priorities (doc 06 §5.7), the rich selection announcement and undo restoring the selection | Proposed follow-up with WP-S2.04 and WP-S2.05 |
| Open tabs and per-page viewports persisted in `view.json` (doc 16 §2.5, FR-UI-033) | Proposed follow-up (a `Store` accessor for the per-user state directory and an `ew.view/1` file) |
| The page navigator's multi-selection and drag to reorder with the renumber preview (FR-PAG-024's rest) | Proposed follow-up with WP-S2.06 and WP-S2.09 |
| Potential and net highlighting with pattern and colour (FR-UI-018's rest) | Proposed follow-up with WP-S2.06's potential navigator |
| Previews on a fork job off the UI thread | Proposed follow-up with FT-251 (after a release-phase measurement, ID-512) |
| The palette's `#` page and `:` cell prefixes over `page_rows` and `resolve_goto` | Proposed follow-up with WP-S2.08 |
| Keymap files, presets and the keymap editor; theme tokens | WP-S2.08 |
| Naming the dock's and the canvas area's splitter handles | FT-338 (WP-S2.08) |
| The GPU path, the switch rule, FT-349 | WP-S2.02 part 2 |
| A second window, Compare and Preview tabs | WP-S2.09; 1.0 |

FT-313's rest (the export job and the recovery dialog) stays with WP-S2.09; FT-339 to FT-342 stay with their packages;
none is this package's. FT-314's harness help gains the new words of §6.10 and §7.9.

## 13. Review checklist for the Opus reviewer

1. **Boundaries.** No new edge or package; the public items of §4.2 and no other below the shell; no `ew-model` name in
   `ew-viewmodel` or `ew-ui`; `ew-render2d` free of egui; tools never call the `Session`.
2. **One path.** Every state change in the GUI ends in `ShellModel::dispatch`; the nested tile tree is rebuilt from the
   model, never the reverse; the only other mutation is `set_canvas_backend` (and `set_tool_for_tests` behind its
   feature).
3. **Read-only.** No shipped action or tool commits a command in either part; the probe is unreachable from the
   binaries (the feature is off in their builds).
4. **Exactness.** The cell rule is `ew-derive`'s, evaluated on exact rationals; snapping on grid indices; link targets
   equal the PDF's destination anchors; no float equality anywhere a decision depends on it.
5. **Preview protocol.** Previews only through `Session::preview_with`; commits through `commit` or `execute_with` in the
   window and `run_script` headless; stale previews never committed directly; the transaction-read rule untouched.
6. **Determinism and privacy.** No time, path or process ID in outputs; V12 and T10 pass; dwell is an action.
7. **Accessibility.** The canvas node, its cursor description and live region, the tab and tree nodes, the walker with
   the extended skip count; no meaning in colour alone (cursor, snap, measure and band shapes).
8. **Memory.** Tabs capped at 32, history at 50, viewports at 64 per tab, hidden tiles at 64 MiB; scenes dropped with
   their last tab.
9. **Exact counts.** V10 and T11 as specified, with counters that cannot pass vacuously (each asserted above zero where
   the flow must use it).
10. **Scope.** Nothing of §12 half-done; deferred items named in the lane report.

## 14. Size, routing and the documents to update

**Size.** WP-S2.03 keeps 6.8 pw PERT (O 4.5 · L 6.5 · P 10); the CSV is unchanged. **Part 1, 2.5 pw:** the four
public items and their tests 0.3; the canvas area, tabs and the per-tab canvases in `ew-viewmodel` 0.35; the
multi-canvas host with the nested tree and FT-356 0.45; the navigator 0.4; links, follow and history 0.3; Go to and the
cell (FT-345) 0.2; the grid cursor, focus and AccessKit 0.2; transcript, describe and the headless and kittest tests 0.3.
**Part 2, 2.4 pw:** the contract and the host 0.4; the overlay layer 0.3; the preview protocol 0.3; Select with the
selection 0.4; snapping, `page_cps` and the indicators 0.35; keyboard tool input 0.2; Measure 0.1; the link preview
0.15; transcript, describe and tests 0.2. The 1.9 pw between the parts' 4.9 pw and the PERT is the deferred scope of §12
that the WBS still counts here; the design pass decides whether it stays as the package's reserve or moves with the
proposed follow-ups.

**Routing.** Both parts add public items below the shell, and part 1 changes the performance-critical tile host while
part 2 sets the tool API that WP-S2.04 and later plugins build on (`CLAUDE.md`: the lead-programmer's), so each goes to
one lead-programmer lane at D4 (`opus`, `high`) with an Opus review. Part 2 starts after part 1's delivery.

**Documents the design passes update** (named here because D4, D6, D9 and D10 change accepted text): doc 16 §2.5 (the
jump history per tab and not persisted; tabs and viewports in `view.json` remain the target), §3.2 (the Window menu as
built), §3.4 (the canvas area as a nested tree, Ctrl+T and Ctrl+F6), §3.5 (Back and Forward per tab), §3.6 (the cursor
field: the pointer, else the grid cursor, the cell in the project's `ip` format), §3.9 (as built), §4.2 (the focus
areas and keyboard contexts), §4.5 (the tool host in `ew-viewmodel`, `ew-ui` paints), §5.1 and §5.2 (the page navigator
as built), §10.1 (the canvas node's cursor description and live region); doc 06 §4.1 (`ToolCx` without a `World`, the
`Select` and `Follow` outputs, previews on the UI thread), §4.3 (a click on a link follows; the drag rows are WP-S2.04's),
§5.7 (picking on entity rectangles until the spatial index), §6 (the jump view, Shift+Arrow, Shift+F and Ctrl+Shift+F);
architecture §5.4 (interactive tool previews on the UI thread) and §9 (the canvas-tools row and the as-built note); doc
18 §3.17 (what the drafter can do and read back); ADR-0014's as-built note (the nested egui_tiles tree). FR-PAG-024's
acceptance text counts "18 folios plus 903–911", while the as-built page order of Golden A has 36 entries (doc 16 §5.2);
the design pass reconciles the requirement's count (§10's V4 tests the as-built order).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S2.01 to WP-S2.06, WP-S2.08, WP-S2.09, WP-S2.12, WP-S1.08, WP-S6.13;
  Appendix B (FT-251, FT-313, FT-314, FT-338 to FT-342, FT-345, FT-349, FT-356).
- [Decision log](../06-decision-log.md): ID-54, ID-341, ID-443, ID-444, ID-474, ID-489, ID-490, ID-493, ID-505 to
  ID-511, ID-512, ID-513, ID-517 to ID-521, ID-525, ID-529.
- [ADR-0005](../../adr/0005-value-types-nm-dec3-mltext.md), [ADR-0008](../../adr/0008-transactions-undo-preview.md),
  [ADR-0014](../../adr/0014-gui-toolkit-egui.md), [ADR-0027](../../adr/0027-native-symbol-grid-and-qet-grid-policy.md);
  [architecture](../../design/04-architecture.md) §3, §5, §8.2, §9; [UX/UI](../../design/16-ux-ui.md) §1 to §5, §10,
  §17; [schematic editor](../../design/06-schematic-editor.md) §2.4, §2.8, §4, §5.1, §5.7, §6;
  [reports and output](../../design/09-reports-forms-output.md) §4.2;
  [quality](../../design/18-quality-security-release.md) §3.5, §3.11, §3.16, §3.17.
- Requirements: FR-UI-015, FR-UI-016, FR-UI-018, FR-UI-019, FR-UI-026, FR-UI-033, FR-UI-034, FR-UI-035, FR-UI-039,
  FR-PAG-021, FR-PAG-024, FR-SYM-009, FR-XRF-019, NFR-A11Y-01, NFR-A11Y-02, NFR-A11Y-03, NFR-UX-07, NFR-REL-08,
  NFR-PERF-19, NFR-PERF-20; AC-28.
