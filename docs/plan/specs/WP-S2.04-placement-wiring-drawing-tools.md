# WP-S2.04: placement, wiring and drawing tools (closed specification)

**Status:** specified in cycle 23 (documents only) for five lead-programmer parts: part 1 (place and arrange, D4),
part 2 (wiring, D4), part 3 (interruption points and black boxes, D4), part 4 (page items, wire markers and structure
boxes, D5) and part 5 (images, D4, after FT-337); every part after WP-S2.03 part 2's delivery; Opus review of each ·
**Date:** 2026-10-22 · **Work package:**
[WP-S2.04](../02-work-packages.md#wp-s204-placement-wiring-and-drawing-tools) · **Decision log:** to be allocated by the
cycle-23 design pass (the IDs continue after ID-555; §17.3 lists the entries) · **Part of:** ElektraWatt plan,
specifications

**Purpose.** This is the closed specification of the first editing package. It turns the viewer and tool framework of
[WP-S2.03](WP-S2.03-viewer-and-tool-framework.md) into an editor in which the pilot draws golden folios 101 to 103 in a
new project (G-PP-01): placing symbols and parts with their variants, moving, rotating, mirroring and exchanging them,
wiring with the two-stage autoconnect ghosts, editing routes and junction sequences, splitting on insert, placing and
continuing interruption points, drawing black boxes and structure boxes, free texts, graphics, images and wire markers,
and showing crossings as hops. It is written against **WP-S2.03 part 2's specified contract** (§2.1), not against its
code, which no lane of this package may assume beyond that contract. Where this document and a design document
differ, this document is the narrower and wins for the lanes; the design documents stay the target of the whole work
package, and the documents named in §17 are updated at the design pass after each part. A choice a lane meets that this
document does not settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs](#2-inputs)
3. [The decisions of this specification](#3-the-decisions-of-this-specification)
4. [Crates, boundaries and the public items below the shell](#4-crates-boundaries-and-the-public-items-below-the-shell)
5. [Commands](#5-commands)
6. [Framework additions shared by every part](#6-framework-additions-shared-by-every-part)
7. [Part 1: place and arrange](#7-part-1-place-and-arrange)
8. [Part 2: wiring](#8-part-2-wiring)
9. [Part 3: interruption points and black boxes](#9-part-3-interruption-points-and-black-boxes)
10. [Part 4: page items, wire markers and structure boxes](#10-part-4-page-items-wire-markers-and-structure-boxes)
11. [Part 5: images](#11-part-5-images)
12. [Determinism, the headless mode and exact counts](#12-determinism-the-headless-mode-and-exact-counts)
13. [Acceptance checks](#13-acceptance-checks)
14. [Files owned](#14-files-owned)
15. [Out of scope](#15-out-of-scope)
16. [Review checklist for the Opus reviewer](#16-review-checklist-for-the-opus-reviewer)
17. [Size, order, routing and the documents to update](#17-size-order-routing-and-the-documents-to-update)
18. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package the project view is an editor for the multi-line pages of the reference set.
Every gesture is a tool state machine over WP-S2.03's contract, every model change is a registered command committed
through the tool host (preview + commit = execute), every gesture runs headless through `ew.uiscript/1` and is read back
through the transcript and `ui.describe`, and the keyboard alone can do everything the pointer can (NFR-A11Y-01). The
proof is the G-PP rehearsal of §13.6: a headless script, generated from Golden A's records, draws folios 101 to 103 in a
new project made by the company-standard wizard, with exact counts after each folio, and exports the PDF.

**Non-goals** (each is named with its package in §15):
- **No layout tools** (panel layout, rails, placements): WP-S4.02 (ruling, delegated).
- **No property grid**, no tag field and no provenance chips: WP-S2.05 (ruling, delegated). Typed tags, parts and
  potential definitions reach the model in this package through the existing `command.run` action (WP-S2.01) in scripts
  and the palette, or through the picker where §7.1 says so.
- **No navigators** (device navigator drag source, potential and IP navigators, highlighting): WP-S2.06. A headless
  `place.pick` with a `function` parameter covers the drag of an unplaced function (§7.2).
- No strip editor, no cable editor (WP-S2.07); no clipboard or paste (WP-S2.05); no macros (WP-S1.19).
- No change to any exported byte of Golden A, to the committed `golden/` tree, to AC-15's 609 commands or to the
  acceptance result (88 pass); no new workspace edge; no new external crate in parts 1 to 4.

## 2. Inputs

| Input | What the lanes take from it |
|---|---|
| [WP-S2.03's specification](WP-S2.03-viewer-and-tool-framework.md) §4.2, §6.2, §6.6, §7 and §8 | The contract of §2.1 below: the tool trait, the host, the preview protocol D9, the overlay, the snapper, the selection, keyboard tool input, the page-content cache, the transcript and describe rules, the `testkit` probe pattern |
| [WP-S2.01](WP-S2.01-application-shell.md) and [WP-S2.02](WP-S2.02-canvas-renderer.md) with their as-built notes | `ShellModel::dispatch`, the registry and contexts, `command.run`, the headless mode (ID-443, ID-474, ID-546), the canvas model and its entity rectangles, `canvas.snapshot` |
| Code as built at the cycle-22 delivery | `crates/ew-commands/src/commands/{device.rs,connection.rs,edit.rs,fields.rs,common.rs,mod.rs}`, `src/geometry.rs`, `src/normalise/connection_split.rs`; `crates/ew-schema/src/{kind.rs,page.rs,connect.rs,library.rs}`; `crates/ew-derive/src/{structure.rs,wires.rs,settings.rs}`; `crates/ew-scene/src/{page.rs,route.rs,symbol.rs}`; `crates/ew-api/src/{lib.rs,output/view.rs,output/layers.rs}`; `crates/ew-viewmodel/src/{wizard_data.rs,describe.rs,actions.rs}`; `crates/ew-testkit/src/golden/dsl.rs`, `tests/command_fuzz/arms.rs`; `content/symbols/**`, `content/fndefs/**` |
| [Architecture](../../design/04-architecture.md) §3 (rules 1, 3, 6), §5.1, §5.2, §9 | `Session` as the only door; the transaction-read rule (ADR-0008 amendment 4, ID-525); previews on a fork; the canvas tools row |
| [ADR-0007](../../adr/0007-explicit-connections-and-autoconnect.md), [ADR-0008](../../adr/0008-transactions-undo-preview.md), [ADR-0005](../../adr/0005-value-types-nm-dec3-mltext.md), [ADR-0009](../../adr/0009-invariant-catalogue.md), [ADR-0027](../../adr/0027-native-symbol-grid-and-qet-grid-policy.md) | Autoconnect is an aid whose result is explicit connections; previews are the real command; integer nm and grid indices; invariants; the QET grid |
| [Schematic editor](../../design/06-schematic-editor.md) §2.2, §2.4, §2.5, §2.7, §3.1 to §3.5, §3.8, §3.9, §4, §5.2 to §5.4, §5.7, §6, §8, §9 | Page-type rules, grids, variants A–H, boxes, the command table, normalisers 7 and 8, placement targets, moves, connections, junctions, IPs, markers, texts and graphics, undo granularity, the tool state machines, autoconnect, split, routing, snapping, keys, pitfalls, the walkthrough of folios 101 and 102 |
| [UX/UI](../../design/16-ux-ui.md) §4.1, §4.2, §4.4 to §4.8, §10.1 to §10.3, §17 | Escape and routing order, keymaps, the tool host and tool strip, selection, drag and drop with keyboard equivalents, AccessKit, colour and pattern rules, pitfalls |
| [Domain model](../../design/03-domain-model.md) §5 (`StructureBox`), §6 (`Representation`, `BoxSpec`, `PageItem`), §9, §10 | The records these tools write |
| [Connections](../../design/08-connections-terminals-cables.md) §4.5 | Marker modes and the marker text |
| [Extensibility](../../design/17-extensibility.md) §2.1, §2.3, §11 | The command registry, exposure classes, the stability of parameter schemas |
| [Quality](../../design/18-quality-security-release.md) §3.5, §3.11, §3.17 | Determinism, UI tests, what the blind drafter drives |
| [Golden project](../../design/20-golden-project.md) §5, §6, §8, §15, §17.5 and the tables of `golden/J-E-2416/tables/` | The content of folios 101 to 103 and 901, the text styles, the oracle of §13.6 |
| Requirements | The 21 rows of the work package: FR-UI-015, FR-UI-017, FR-UI-018, FR-SYM-015, FR-SYM-022, FR-SYM-023, FR-SYM-024, FR-SYM-025, FR-CON-003, FR-CON-005, FR-CON-006, FR-CON-007, FR-CON-008, FR-CON-024, FR-CON-031, FR-CON-039, FR-XRF-013, FR-XRF-014, FR-DEV-013, FR-DEV-039, FR-PRJ-020; with FR-UI-016, FR-UI-019, FR-CON-001, FR-CON-004, FR-CON-023, FR-SYM-004, FR-SYM-009, FR-DEV-003, FR-DEV-017, FR-DEV-038, FR-PRJ-019, NFR-A11Y-01 to NFR-A11Y-03, NFR-UX-07, NFR-REL-07, NFR-REL-08 where they touch it; AC-28 (the ghost-preview terminal step), AC-07 (on the new project, §13.6) |

The WBS lists WP-S2.03, WP-S1.13 and WP-S1.07 as dependencies. Of WP-S1.13 this package uses what is built (`box.black`,
`device.place`, the part `function_template`); of WP-S1.07 the connectivity and wire-property queries. WP-S2.03 part 2
must be delivered before part 1 starts (§17.2).

### 2.1 What WP-S2.03 part 2 provides (the base contract)

The lanes rely on exactly these items of the WP-S2.03 specification and on nothing of its code beyond them. If a
delivered item differs from this list, the lane stops and reports it.

| Item | Where specified | Used here for |
|---|---|---|
| `CanvasEvent` (`PointerMove`, `Press`, `Release`, `Dwell`, `CursorMove`, `Activate`, `Back`, `Escape`) with `Mods` | S2.03 §7.1 | Every tool's input |
| `ToolOutput` (`None`, `Transient`, `Preview`, `Commit`, `Cancel`, `Select`, `Follow`), one per event | S2.03 §7.1 | Every tool's output |
| `trait Tool` (`name`, `state`, `on_event`, `status_hint`, `wants_hover`) and `ToolCx` (`page: &CanvasPage`, `points_per_nm`, `pane`, `snapper`, `selection`) | S2.03 §7.1 | The tools of this package implement it |
| The tool host: one active tool per window on the focused group, the output table, the Escape chain, the live-region announcement | S2.03 §7.2 | Unchanged; §6.4 adds host duties |
| The preview protocol: `Session::preview_with` on a dwell of at least 80 ms, `PreviewState`, the semantic ghost from `Session::derived_diff`, staleness, commit by `Session::commit` or `execute_with` in the window and by `Session::run_script` headless | S2.03 §7.3 (D9) | Every preview and commit of this package |
| `Overlay` and its items (`Line`, `Polyline`, `Rect`, `Ghost`, `Marker`, `Label`) in sheet nm, the provisional tokens | S2.03 §7.4 | Ghosts, stubs, readouts; §6.3 adds one item |
| `Snapper::snap(at, points_per_nm, alt) -> Snap { at_nm, page_nm, grid_index, kind, label }` over the page grid and `Session::page_cps` | S2.03 §7.5 | Connection-grid and CP snapping; §6.5 adds the fine grid |
| `Selection { items, primary, generation }` and the Select tool's table | S2.03 §7.6 | §7.3 adds the drag rows |
| Keyboard tool input: the grid cursor, `cp.next`, `cp.previous`, `cp.next_unconnected`, `canvas.activate`, `tool.back` | S2.03 §7.7 | Keyboard placement and wiring |
| The page-content cache and `CanvasPage` (display list, entity rectangles, links, cells, grid, CPs), fetched once per (revision, folio) | S2.03 §6.2, §7.5 | §4.3's page tables join it |
| The actions `canvas.press`, `canvas.release`, `canvas.dwell`, `canvas.pointer` with modifiers; the transcript member `canvas` with `tool`, `snap`, `selection`, `preview`, `overlay`, `link_preview`; the describe lines of §7.9 | S2.03 §7.9 | Extended in each part |
| The `testkit` feature of `ew-viewmodel` and its `[[banned_features]]` rule | S2.03 §7.6 | Unchanged; the probe stays test-only |

**What this package adds to the framework** (all in part 1, §6): two `CanvasEvent` variants (`Key`, `Text`), the page
tables and the symbol shape in the tool context, one overlay item (`Paths`), the fine-grid snap, the picker and text
popups in the host, the typed resolution of selected entities, and the readouts of FR-UI-018's rest.

### 2.2 Gaps found below the shell (why the parts are shaped as they are)

| Gap at the cycle-22 tip | Consequence |
|---|---|
| `page_item` and `structure_box` are **reserved** kinds (`KindStatus::Reserved` in `ew-schema`); no free text, graphic, image, group wire marker or structure box can be stored | Part 4 implements both kinds (D10), a file-format and derivation change, hence D5 |
| No command for page items, structure boxes, symbol exchange, IP arrangement, box reshaping, a wire ending in a new IP, or the drawing settings | Eight new commands (§5.1) |
| `device.place` has no part-first or terminal target; `rep.move` takes one nm delta, which on the QET grid (`7_937_500/3` nm) cannot keep several representations exactly on `round(k·g)` | Two `PlaceTarget` variants and a grid-step move (§5.2) |
| `ew-library` is empty; a new project holds no symbol copies | The starter library of D8 |
| The A* router of doc 06 §5.4 is not built (FT-18, FT-43); `connection.draw` stores the route it is given; route-less connections are drawn with `ew-scene`'s provisional L route; normaliser 7 splits only connections that have a stored route | D7 |

## 3. The decisions of this specification

Each is a lead ruling that the owner may reverse, except the four marked "ruling (delegated)", which the owner handed
to the lead for this specification and which are recorded as taken. The design pass allocates the decision-log entries.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Five parts.** Part 1 place and arrange (2.4 pw, D4); part 2 wiring (2.4 pw, D4); part 3 interruption points and black boxes (1.9 pw, D4); part 4 page items, wire markers and structure boxes (2.5 pw, D5); part 5 images (0.5 pw, D4, after FT-337 and part 4). Order: 1 → 2 → {3 ∥ 4} → 5 (§17.2) | Each part is one lane-cycle; the cut follows the code: part 4 is the only one that changes the file format and the derivation, so it alone is D5; part 5 waits for FT-337's asset store, which is due before G-PP anyway. Against the cycle's first sketch, free texts, graphics and images leave part 1, and wire markers and structure boxes join part 4, because all of them need the record kinds that part 4 implements |
| D2 | **Ruling (delegated): every tool commits through registered commands only, existing or new** (ID-443). No tool, popup or host step writes a record any other way; eight new commands and five amended ones carry this package (§5); each new one has its schema, catalogue row, DSL verb and fuzzer arm | One mutation path, scriptable from `.ewcmd`, MCP and the blind drafter alike; a gesture the headless mode cannot drive does not exist |
| D3 | **Ruling (delegated): previews use `Session::preview` (WP-S2.03 D9); there is no second preview model.** Every tool's semantic ghost is the derived diff of the preview of the very command it would commit; geometry ghosts are overlay items computed from the page tables | Previews stay exact because they are the real command (NFR-REL-08); a separate model would be a second semantics |
| D4 | **Ruling (delegated): the autoconnect ghosts are overlay-only until commit.** Proposals are computed in `ew-viewmodel` from the page tables (doc 06 §5.2) and drawn as dashed overlay stubs; they reach the model only as the `connect` hints of the committed `device.place` (ADR-0007), and the 80 ms preview runs that full command with its hints | Autoconnect stays an editing aid, never model semantics; Escape or Alt leaves no trace (FR-CON-003) |
| D5 | **Ruling (delegated): no layout (panel) tools, and the property grid stays WP-S2.05.** The text tool's inline edit commits `fields.set` (doc 06 §4.3), which is not the property grid | Keeps the package inside its PERT and its milestone |
| D6 | **The tool context grows by the page tables and the symbol shape**, prepared by the host through two new read queries (`Session::page_tables`, `Session::symbol_shape`, §4.3) and cached per (revision, folio) and per symbol copy; tools still never call the `Session` (WP-S2.03 D10) | Tools need CP directions, functions, connection polylines, boxes and symbol geometry that `PageCp` and the display list do not carry; one query per page keeps NFR-PERF-20's exact counts |
| D7 | **Routing: one pure orthogonal router, used by the tools; the command stores the route it is given.** The A* of doc 06 §5.4 is the pure function `ew_commands::route::auto_route`, re-exported as `ew_api::routing::auto_route`; the Wire tool calls it for the ghost and passes the result as `connection.draw`'s `route`. `connection.draw` keeps its as-built semantics (an empty route stays route-less and is drawn provisionally), so Golden A's output does not change. Split on insert (normaliser 7) works on connections with a stored route, which every connection drawn with the Wire tool has; the route editor's first edit of a route-less connection stores its drawn polyline (§8.4) | A router inside `connection.draw` would change the golden output (every golden connection is route-less) and AC-15; a pure function in `ew-commands` is the one implementation that the commands' re-solve of FT-18 will reuse. Doc 06 §3.2's last paragraph and §5.4 are amended |
| D8 | **The starter library is the shipped `content/symbols` and `content/fndefs`, compiled into `ew_library::starter`.** The picker lists the project's embedded symbol and part copies and the starter symbols; choosing a starter symbol that is not embedded commits `library.embed` with the symbol and its FnDef (as far as not embedded) as its own undo step, then floats it. The picker shows the `@qet10` symbols on a page whose connection grid is the QET grid and the others elsewhere | A new project has no symbol copies; embedding is a model change (a gesture of its own, like the wizard's form embedding, ID-545), so a first placement costs two undo steps, against a composite command that no other front-end needs |
| D9 | **Two placement targets join `PlaceTarget`:** `from_part` (part-first placement: the device, its part use and every template function, the chosen one placed, the rest unplaced; FR-UI-017, FR-DEV-017) and `new_terminal` (one terminal of an existing strip, created and placed in one transaction; AC-28's ghost step). A new strip is its own step (`strip.new` → `device.create` with the strip aspect) | One gesture, one transaction (doc 06 §3.9) for the placements the pilot needs on folio 101; the strip editor stays WP-S2.07 |
| D10 | **Part 4 implements the reserved kinds `page_item` (typed variants `graphic`, `free_text`, `wire_marker`; `image` in part 5; every other variant kept verbatim as an unknown variant) and `structure_box`**, with their invariants, drawing, PDF output and the structure-box containment in `structure_of` (the hook of `ew-derive`'s `structure.rs`). No format-version bump: rank, file and ownership are fixed since WP-S1.02 and an older reader keeps the records verbatim (doc 05 §10.3). Their fields are edited with `fields.set`, which gains the two kinds | FR-SYM-023 to FR-SYM-025 map to this package alone; `fields.set` is the established edit path and gives WP-S2.05's grid the two kinds for free. This closes WP-S1.12's "structure boxes" scope item |
| D11 | **No undo coalescing in this package.** Each nudge, rotate press and junction reorder is one undo step, in the window and headless alike | Headless commits go through `run_script`, which carries no coalescing key; equal undo stacks in both front ends weigh more than doc 06 §3.9's merging, which a follow-up restores for both (§17.4) |
| D12 | **Commands that need derived data take it from the tool.** `ip.arrange` takes the new order; the tool sorts by name or by derived potential name. No new command reads a memoised query inside its transaction | The transaction-read rule (ID-525) |
| D13 | **Moves are in grid steps.** `rep.move` gains `steps: [kx, ky]`: each moved position becomes the grid point at its index plus the steps, exactly, and routes follow per moved CP (§5.2). The tools always send `steps` | On the QET grid a uniform nm delta cannot keep several representations on `round(k·g)` (D3 of WP-S1.04 refuses them); grid indices are the rule of doc 06 §5.7 and ADR-0005 |
| D14 | **Typed selection through the page tables.** A selected `EntityRef` is resolved to its kind (representation, connection, junction, page item, structure box) by the page tables; picking stays on WP-S2.02's entity rectangles plus exact integer segment distance for connections. FT-359 is narrowed to the spatial index, kurbo picking and the rich announcement | Delete, transform and move need kinds; the index is a performance measure, and performance is not measured during development (ID-512) |
| D15 | **The headless mode and `ui.describe` grow by the members and words of §7.7, §8.7, §9.6, §10.7 and §11** (ID-474 extended; a reviewed transcript change, as ID-511 and ID-537 were) | The blind drafter draws and reads back without a screenshot |
| D16 | **Marker modes are a project setting, stored in `settings.extra.drawing.marker_mode`; absent means the as-built rule (`WireOverrides.show_marker`).** The per-page mode of FR-CON-024 is a follow-up | Golden A's output stays byte-equal; the four modes are new values a project opts into |

## 4. Crates, boundaries and the public items below the shell

### 4.1 Roles

| Crate | Layer | Part 1 | Part 2 | Part 3 | Part 4 | Part 5 |
|---|---|---|---|---|---|---|
| `ew-viewmodel` | GPL | `tools/{place,picker,move_keys,transform}.rs`, Select's drag rows, framework additions (§6), actions, transcript, describe | `tools/{wire,autoconnect,route_edit,junction_editor}.rs` | `tools/{ip,black_box}.rs` | `tools/{text,graphic,marker,structure_box}.rs` | `tools/image.rs` |
| `ew-commands` | GPL | `PlaceTarget` variants, `rep.move` `steps`, `rep.exchange_symbol` | `route.rs` (the router), `project.drawing.set` | `connection.draw_to_ip`, `ip.arrange`, `box.reshape` | `box.structure`, `item.place`, `marker.place`; `rep.move` `items`; `edit.delete` and `fields.set` kinds; `project.drawing.set` `marker_mode` | `item.place` `image` |
| `ew-schema` | GPL | — | — | — | `PageItem`, `StructureBox` records; kinds `Implemented` | `PageItemKind::Image` |
| `ew-model` | GPL | — | — | — | the invariants of §10.1 and the delete policies of the two kinds | — |
| `ew-derive` | GPL | — | `drawing_settings` read | — | structure-box containment, `ip_structure`, bound texts, marker modes, group marker text | — |
| `ew-scene` | GPL | — | `hops.rs` | — | `items.rs` (page items, structure boxes, group markers) | image drawing |
| `ew-library` | GPL | `starter` module | — | — | — | — |
| `ew-api` | GPL | `output/editing.rs` (§4.3 items), `geometry` helpers | `routing` re-export, `junction_wires` | — | page tables gain items and structure boxes | — |
| `ew-render2d` | GPL | overlay item `Paths` | — | — | — | — |
| `ew-ui` | GPL | tool strip, picker popup, key mapping, inline readouts | junction editor popup | — | text field popup | — |
| `ew-testkit` | GPL | DSL verbs and fuzzer arms of the part's commands | same | same | same | same |
| `elektrawatt` | GPL | tests only | tests only | tests only | tests only | tests only |

**Rules the lanes keep** (WP-S2.01's to WP-S2.03's rules stand: through `Session` only, `check-raw-read` green; no
`unsafe`; no unordered output; one clock; `ew-render2d` names no egui type):
1. **Only the public items of §4.3 and the commands of §5** are added below the shell, each first and in its own commit
   with tests. Any other missing item stops the lane.
2. **No new workspace edge and no new package** in `Cargo.lock` in parts 1 to 4 (`cargo tree -p elektrawatt` package
   count equal before and after, stated in the commit message). Part 5 may add only what FT-337 already added.
3. **One mutation path.** Every state change of the shell is a registry action on `ShellModel::dispatch`; every model
   change is a command committed by the host (WP-S2.03 §7.3). The picker's embed step and the strip step are commits of
   the host, not of a tool.
4. **Tools never call the `Session`**; they may call the pure functions `ew_api::routing::auto_route` and
   `ew_api::geometry::*`.
5. **`golden/` is untouched**, the acceptance result stays at 88 pass, and every new setting is absent in Golden A, so
   every exported byte of Golden A is unchanged; each part's lane proves it by the existing golden output tests.

### 4.2 Coordinates

Tools receive sheet nm (WP-S2.03). Commands take page nm. The page tables and the symbol shape are in **page nm**; the
tools convert with the `PageGrid` map of the canvas page (the map `PageCells::cell_at` uses, S2.03 §4.2) and draw the
overlay in sheet nm. A route is the full vertex list from end `a` to end `b` inclusive, as `geometry::require_route`
checks it (a CP end's vertex equals the CP's position exactly).

### 4.3 The public items added below the shell

| Part | Crate | Item | Contract |
|---|---|---|---|
| 1 | `ew-library` | `pub mod starter` with `pub struct StarterItem { pub key: &'static str, pub kind: StarterKind, pub json: &'static str }`, `pub enum StarterKind { Symbol, Fndef }`, `pub fn items() -> &'static [StarterItem]` | Every file of `content/symbols/*.json`, `content/symbols/qet10/*.json` and `content/fndefs/*.json` compiled in with `include_str!`, in key order; `key` is the file stem. A test pins the count against the directory listing and that each `json` parses as the `lib/` document `library.embed` takes |
| 1 | `ew-api` | `pub struct PlaceCatalogue { pub symbols: Vec<CatalogueSymbol>, pub parts: Vec<CataloguePart> }`; `pub struct CatalogueSymbol { pub key: String, pub copy: Option<Uuid>, pub fndef_key: String, pub rep_type: String, pub letter: Option<String>, pub black_box: bool, pub terminal: bool, pub allowed: bool }`; `pub struct CataloguePart { pub copy: Uuid, pub part_number: String, pub manufacturer: String, pub functions: Vec<CatalogueTemplateFn> }`; `pub struct CatalogueTemplateFn { pub index: u16, pub fndef_key: String, pub main: bool, pub symbol: Option<String>, pub black_box: bool }`; `pub fn Session::place_catalogue(&self, folio: &str) -> Option<PlaceCatalogue>` | For the page with that folio (`None` for an unknown folio or a generated page). `symbols`: every embedded symbol copy and every starter symbol whose key is not embedded, keeping only the grid family of the page (keys ending `@qet10` on a page whose grid is `7_937_500/3` nm, the others elsewhere), in key order; `copy` the copy ID when embedded; `letter` the project identifier set's letter for the FnDef's first category (`ew-rds` mapping), `None` without a mapping; `black_box` when the FnDef's `contact_kind` is `black_box`; `terminal` when its first category is `Terminal`; `allowed` when the symbol's rep type is one the page type allows (`allowed_rep_types`). `parts`: every embedded part copy with a non-empty `function_template`, by part number; `functions` in template order, `symbol` the key of the first `symbols` row (allowed) that draws that FnDef, else `None` |
| 1 | `ew-api` | `pub fn Session::starter_embed(&self, key: &str) -> Option<serde_json::Value>` | The `library.embed` parameters (`{"copies": [...]}`) for the starter symbol `key` and its FnDef, leaving out what is embedded already; `None` when `key` is not a starter symbol or both are embedded. The host commits it through the normal path (§6.4) |
| 1 | `ew-api` | `pub struct SymbolShape { pub copy: Uuid, pub key: String, pub cps: Vec<ShapeCp>, pub body: [i64; 4], pub outline: Vec<Vec<(i64, i64)>>, pub rep_type: String, pub fndef: Uuid }`; `pub struct ShapeCp { pub symbol_cp: u16, pub fn_cp: u16, pub at: (i64, i64), pub dir: Dir }`; `pub enum Dir { Up, Down, Left, Right }`; `pub fn Session::symbol_shape(&self, copy: Uuid) -> Option<SymbolShape>` | The embedded symbol copy at orientation A (R0), relative to its insertion point, in page nm: every symbol CP with its FnDef CP and direction; `body` the bounds of its graphics without texts; `outline` every stroked or filled path of its graphics (not texts) flattened to polylines with a tolerance of 50 000 nm, in paint order. `None` for an unknown copy |
| 1 | `ew-api` | `pub mod geometry` with `pub fn orient_point(o: u8, p: (i64, i64)) -> (i64, i64)`, `pub fn orient_dir(o: u8, d: Dir) -> Dir`, `pub fn orient_then(o: u8, rotate: u8, mirror: bool) -> u8`, `pub fn variant_letter(o: u8) -> char` | Pure functions equal to `ew_units::Orient2`'s mapping (`o` 0..=7 is A..H = R0, R90, R180, R270, M0, M90, M180, M270, doc 06 §2.5; `orient_then` composes as `rep.transform` does). A test pins equality with `Orient2` for all 8 × 4 × 2 combinations |
| 1 | `ew-api` | `pub struct PageTables { pub folio: String, pub page: Uuid, pub grid: (i64, i64), pub reps: Vec<TableRep>, pub cps: Vec<TableCp>, pub connections: Vec<TableConnection>, pub junctions: Vec<TableJunction> }` and the row types below; `pub fn Session::page_tables(&self, folio: &str) -> Option<PageTables>` | Every record drawn on the page, in page nm, each list in ID order. `grid` the connection grid `(num, den)`. **`TableRep`** `{ id, functions: Vec<Uuid>, device: Option<Uuid>, symbol: Uuid, symbol_key: String, orient: u8, rep_type: String, pos, body: [i64; 4] (the shape's body under the orientation, translated), ip: Option<TableIp { name, direction: String }>, black_box: Option<TableBox { outline: Vec<(i64, i64)>, cps: Vec<(u16, (i64, i64), Dir)> }> }` (box geometry absolute). **`TableCp`** `{ id (the v5 of `Destination::Cp`, equal to `PageCp::id`), function, idx: u16, rep, label (as `cp_label` prints it), at, dir: Option<Dir>, connections: Vec<Uuid> (ending here, ascending ID), potential: Option<String> (the derived potential name) }`; `at` equals what `ew_commands::geometry::cp_position` returns for that CP (a test pins it over Golden A). **`TableConnection`** `{ id, a: TableEnd, b: TableEnd, kind: String, polyline: Vec<(i64, i64)> (the drawn polyline: the stored route, else `ew-scene`'s provisional route), stored_route: bool }`, `TableEnd` = `Cp { function, idx }` or `Junction { id }`. **`TableJunction`** `{ id, at, style: String ("dot" or "bevel"), bevel_orient: u8, wiring: String ("star" or "chain", the effective one), explicit_wiring: bool, sequence: Vec<Uuid> }`. **Cost:** memoised families only; computed once per (revision, folio) by the page-content cache (S2.03 §6.2) |
| 2 | `ew-api` | `pub mod routing` with `pub struct RouteRequest { pub from: RouteEnd, pub to: RouteEnd, pub obstacles: Vec<[i64; 4]>, pub grid: (i64, i64), pub area: [i64; 4] }`, `pub struct RouteEnd { pub at: (i64, i64), pub dir: Option<Dir> }`, `pub fn auto_route(req: &RouteRequest) -> Vec<(i64, i64)>` | Re-exports `ew_commands::route::auto_route` (§8.1): the full vertex list from `from.at` to `to.at`, deterministic, every interior vertex on the grid, ends exact |
| 2 | `ew-api` | `pub struct JunctionWire { pub label: String, pub from: String, pub to: String }`; `pub fn Session::junction_wires(&self, junction: Uuid) -> Vec<JunctionWire>` | The physical wires through the junction, from `physical_wires_of` (doc 06 §5.5), in wire order: `label` the printed wire number or `?`, `from` and `to` the CP labels of the wire's ends. Empty for an unknown junction |
| 4 | `ew-api` | `PageTables` gains `items: Vec<TableItem { id, kind: String ("graphic", "free_text", "wire_marker", "image" from part 5, "unknown"), bounds: [i64; 4], connections: Vec<Uuid> }>` and `structure_boxes: Vec<TableStructureBox { id, outline: Vec<(i64, i64)>, style: String, caption: String }>` | As above; `caption` the printed caption (§10.2) |

No other item below the shell is added. In particular `PageCp` (S2.03) is not changed; the page tables carry the richer
rows beside it.

## 5. Commands

### 5.1 New commands

All are exposure `Edit`, stability `Preview`, undoable, `params_version` 1, with `deny_unknown_fields`. The catalogue
grows from 42 to **50** registered commands. Positions are page nm; an `IdS` is a UUID string.

| Command (part) | Parameters | Semantics | Rejected |
|---|---|---|---|
| `rep.exchange_symbol` (1; FR-SYM-022) | `rep`, `symbol` (an embedded symbol copy), `pin_map?: [{symbol_cp, function_cp}]` | The representation's `symbol` becomes `symbol`, `variant` 0, orientation kept. Without `pin_map` the map is derived: when the new symbol's FnDef is the function's `def`, symbol CP → its `fn_cp`; otherwise by designation (the new FnDef CP's designation equal, ASCII case-insensitively, to a designation of the function's FnDef). Text-slot overrides whose slot index the new symbol has are kept, the others dropped with a notice. Routes of connections at the representation's CPs follow each CP's displacement (`geometry::follow_end`). Function CPs drawn before and not drawn now are listed as notices. Interactive; outcome the representation | A connected function CP (a connection on this page ends there) that the new map does not draw; a rep type the page does not allow; a symbol copy or representation that does not exist; an IP representation |
| `project.drawing.set` (2; FR-CON-031) | `crossings?: "plain" or "hops"` (part 4 adds `marker_mode`, §5.2) | Writes `settings.extra.drawing` (merging; unknown members kept; `null` removes a member). Absent `crossings` means plain. Interactive (no derived diff: presentation only) | A non-object `drawing` member found in the project; an unknown value; no member given |
| `connection.draw_to_ip` (3; doc 06 §4.4 "P") | `a` (`EndpointParam`), `page`, `route` (ending at the IP's CP), `kind?`, `ip: { symbol, fndef?, pos, orient, name, direction, star_source?, sort? }` | One transaction: the IP function and its representation exactly as `ip.place` creates them, then the connection from `a` to the IP's single CP exactly as `connection.draw` creates it (an `on_connection` end splits as there). Outcome the connection | As `ip.place` and `connection.draw`; an IP symbol whose FnDef has not exactly one CP |
| `ip.arrange` (3; FR-XRF-013) | `ips: [IdS]` (IP functions in the new order, at least two, each with exactly one representation, all on one page), `set_sort: bool` | The slots are the current positions of the given IPs' representations sorted by `(y, x)`; slot `i` goes to `ips[i]`. Routes of connections at moved IP CPs follow (`follow_end`). With `set_sort`, `IpSpec.sort` becomes `i + 1`. Interactive; outcome none | Fewer than two IPs; a function that is not an IP; representations on different pages; an IP listed twice |
| `box.reshape` (3; FR-DEV-039) | `rep` (a black-box representation with a `box`), `outline: [Pt2]` (relative to `pos`, at least 3 vertices, each on the connection grid in absolute terms), `cps?: [BoxCp]` | Replaces the outline. With `cps`, the CPs are replaced (indices distinct, each on the outline and on the grid). Without, every CP is re-projected: it keeps the index of the edge it lay on and its offset from that edge's first vertex, clamped to the new edge's length and snapped to the grid point along the edge nearest to it; a CP whose edge index no longer exists moves to the nearest grid point on the nearest remaining edge, listed as a notice (doc 06 §8 item 13). Inner graphics are kept. Routes follow each moved CP. Interactive | An outline with fewer than three vertices or an off-grid vertex; a CP off the outline or off the grid; two CPs with one index; a representation without a box |
| `box.structure` (4; FR-PRJ-020) | `page`, `outline: [Pt2]` (absolute, at least 3 vertices), `style: "dotted" or "dash_dot"`, `caption?: MlText`, `assigns: StructureAssignment`, `external?: IdS` | Creates a `StructureBox` record (§10.1). Outline vertices need not be on the connection grid. Outcome the box | Fewer than three vertices; a node in `assigns` whose aspect is not the aspect of its slot, or that does not exist; a missing page or external reference |
| `item.place` (4; FR-SYM-023, FR-SYM-024; part 5 adds `image`) | `page`, `kind`: `{"graphic": {shape, stroke?, dash?, fill?}}` or `{"free_text": {text, bind?, at, angle?, style?, align?}}` | Creates a `PageItem` record (§10.1). Outcome the item | A degenerate shape (zero-length line, a polyline or polygon with fewer than two or three distinct points, a radius of 0); `bind` not one of the bindable names of §10.2; an empty `text` without `bind`; a missing page |
| `marker.place` (4; FR-CON-023, FR-CON-039) | `page`, `connections: [IdS]` (at least one, in printed order, all on `page`, distinct), `at`, `angle?: 0 or 90`, `csa?: Csa`, `colours?: [ColourSpec]` (empty, or one per connection) | One transaction: on each connection `overrides.show_marker = true`, `overrides.csa = csa` when given, `overrides.colour` from `colours[i]` when given; then a `PageItem::WireMarker { connections, at, angle, fields: ["csa", "colour"] }`. Outcome the item | A connection on another page or missing; a colour that is neither IEC 60757 nor a profile company code (as `fields.set` refuses it, FR-CON-019) |

**For each new command** the lane delivers, in the part's command commit: the `Command` with its doc comment (the
semantics and rejections above), registration in `commands/mod.rs`, the committed schema in
`crates/ew-commands/schemas/<name>.schema.json` (regenerated by that crate's `schemas` test), the name in
`tests/params.rs`, the generated reference page under `docs/book/src/reference/commands/` and the `SUMMARY.md` line
(`cargo xtask docs-gen`, then `--check` green), a **DSL verb** in `crates/ew-testkit/src/golden/dsl.rs` (the builder's
typed method, as `project.company.set`'s `company`), a **fuzzer arm** in `crates/ew-testkit/tests/command_fuzz/arms.rs`
(valid and boundary-violating parameters), and unit tests in the command crate for every rejection.

### 5.2 Amended commands

Every amendment is additive (new optional members or enum variants), so `params_version` stays 1 (doc 17 §11); the
schemas, reference pages, DSL verbs and fuzzer arms are updated likewise.

| Command (part) | Amendment |
|---|---|
| `device.place`, `box.black` (1) | `PlaceTarget` gains **`from_part { part: IdS, template: u16, letter: Ident }`**: a device `{tag: {letter, counter: none}, parts: [the part, quantity 1]}` and one function per entry of the part copy's `function_template` (def = the entry's FnDef copy, which must be embedded; role main for `main: true`, else auxiliary; kind `black_box` when the FnDef's `contact_kind` is `black_box`, else normal; CPs per the template's `cps`, doc 03 §7); the representation draws entry `template`; the others stay unplaced. `device.place` requires the symbol's FnDef to equal entry `template`'s and that entry not to be a black box; `box.black` requires entry `template` to be a black box. **`new_terminal { strip: IdS, designation: Ident, seq?: u32 }`** (`device.place` only): one terminal device created as `terminal.add` creates it (letter, counter and structure from the strip, the next sequence number when `seq` is absent) with its main function `{def: the symbol's FnDef}`, then placed. Rejected: a missing part copy or template index; a template FnDef not embedded (hint: embed it); a strip that is not a strip; a used designation or sequence; `new_terminal` on `box.black` |
| `rep.move` (1; part 4 adds `items`) | **`steps?: [kx, ky]`** (mutually exclusive with a non-zero `delta`): each representation's position becomes `Grid::at(index(pos) + steps)` per axis, exactly; a route vertex coordinate on the grid moves by the same number of steps, a CP-exact coordinate (doc 06 §5.7) moves with its CP; `follow_end` uses the moving CP's own displacement. Rejected: both `steps` and a non-zero `delta`; a representation whose position is not on the grid (it cannot be moved by steps; the message names it). **Part 4: `items?: [IdS]`**: page items and structure boxes translated by `delta`, or by `round(steps × g)` per axis when `steps` is given; `reps` and `items` together name at least one record; every item on the page of the moved representations |
| `edit.delete` (4) | `KindName` gains `page_item` and `structure_box`. Deleting a connection removes it from every wire marker's list; a wire marker left without connections is deleted in the same plan (§10.1) |
| `fields.set` (4) | `SETTABLE` gains `page_item`: `kind` (a dotted path below it, such as `kind.free_text.text`, is allowed) and `layer`; and `structure_box`: `outline`, `style`, `caption`, `assigns`, `external` |
| `project.drawing.set` (4; FR-CON-024) | **`marker_mode?: "all", "deviations", "explicit", "none" or null`**, written to `settings.extra.drawing.marker_mode`; absent means the as-built `show_marker` rule (D16); read by the derivation of §10.2 |

`connection.draw`, `connection.route`, `junction.edit`, `ip.place`, `ip.continue`, `rep.transform` and `library.embed`
are used as built.

## 6. Framework additions shared by every part

Built in part 1, in `ew-viewmodel` (and painted by `ew-ui` and `ew-render2d`), before any tool of §7.

### 6.1 Events

`CanvasEvent` gains `Key(ToolKey)` and `Text(String)`:

```rust
pub enum ToolKey {
    RotateCw, RotateCcw, MirrorX, MirrorY,      // R, Shift+R, X, Y
    NextVariant, NextRepType,                   // V, Shift+V
    NextProposal, ConnectOnMove,                // Tab, C        (part 2)
    ToggleDirection, EndInIp, ToggleStar,       // D, P, —       (part 3)
    Finish,                                     // Shift+Enter, or a double click in the window
}
```

Each key is a registry action (§7.6, §8.7, §9.6, §10.7) whose handler sends `Key(k)` to the active tool; `text.input`
(hidden; `text`) sends `Text(s)`. Select and Measure answer `None` to both, except Select's handling of §7.4. The window
maps a double click (two primary presses within the platform's double-click interval, measured by the window with
`ew_render2d::stats::now`, never by the model) onto `tool.finish`.

### 6.2 The tool context

`CanvasPage` (S2.03 §6.2) gains `tables: Arc<PageTables>` (fetched with the page content, once per (revision, folio)).
A tool that needs a symbol holds an `Arc<SymbolShape>` given by the host when it builds the tool (the host fetches it
once per symbol copy per revision through `Session::symbol_shape`). `ToolCx` gains nothing else.

### 6.3 The overlay item `Paths`

`OverlayItem::Paths { paths: Arc<[Vec<(i64, i64)>]>, transform: (u8, (i64, i64)) }`: polylines in page nm relative to
an origin, under an orientation (0..=7) and translated to an absolute page point, drawn by `ew-render2d` as 1-pixel
dashed lines in the `ghost` token (2 pixels at `ppp ≥ 2`), composited by the headless snapshot like `Ghost`. The view
model converts to sheet nm (§4.2).

### 6.4 Host duties

1. **Popups.** The picker (§7.1), the text field (§10.5) and the junction editor (§8.5) are popups of the host; `ui.dismiss`
   closes the topmost (S2.03 §7.2's Escape chain is unchanged).
2. **Host commits.** The picker's embed step (`Session::starter_embed`) and `strip.new` are committed by the host
   through the same path as a tool's `Commit` (S2.03 §7.3: `execute_with` in the window, `run_script` headless), then
   the host arms the tool. A failed host commit leaves the tool unarmed and shows the message in the banner.
3. **After a commit.** The host keeps the active tool (sticky tools stay in their state, §7.2); after a `rep.move` or
   `rep.transform` committed by Select it sets the selection to the moved representations (one `Select` change, no
   command); a committed record that the next page tables no longer hold is dropped from the selection (doc 16 §17
   item 1).
4. **Tool strip.** `ew-ui` draws one row above the canvas with the active tool's options as read-only text plus the
   toggles the actions of each part list (doc 16 §4.5); headless, the same is in the `ghost` member.

### 6.5 Snapping additions

`Snapper` gains `snap_fine(at, kind, alt) -> (sheet_nm, page_nm)` for graphics and texts (doc 06 §2.4): on a QET-grid
page the fine grid is `793_750/3` nm (1 px) for both; elsewhere 500 000 nm for graphics and 1 000 000 nm for texts;
with `alt` the point is the exact page nm of the pointer (no snap). Object snaps (CPs, junctions, segments) and their
priorities are the tool's (§8.2). All snapping is on grid indices in `i128` (ADR-0005).

### 6.6 Typed selection and picking

`Selection` items stay `EntityRef`s; `selection_kinds(&PageTables)` resolves each to `Rep`, `Connection`, `Junction`,
`Item` or `StructureBox` (part 4) and leaves out entities the tables do not hold (frame, texts of generated pages).
Picking keeps WP-S2.02's entity rectangles (smallest area containing the point inflated by 4 points, ties to the later
in paint order); a connection is picked when the exact integer distance from the point to a segment of its `polyline`
is at most 4 points at the current zoom (ties by smaller distance, then by ID).

### 6.7 Readouts (FR-UI-018's rest)

While a drag or a route is in progress the overlay carries a `Label` next to the ghost: for a move `dx <dx> dy <dy> mm`
(sheet millimetres of the grid-step displacement, three decimals); for a routing wire `<length> mm` of the routed
polyline (sum of segment lengths, three decimals). **Rubber bands:** during a move, each connection with exactly one end
on a moved representation is drawn as a dashed `Polyline` from the moved CP's new position along the route as
`follow_end` would move it; connections with both ends moved are drawn translated.

## 7. Part 1: place and arrange

### 7.1 The picker

A popup listing `Session::place_catalogue` of the focused page: symbol rows (`<key>` with the badge `embedded` or
`starter`, disabled with a reason when `allowed` is false or when `black_box` is set before part 3: "use the black box
tool (B)"), then part rows (`<part number> <manufacturer>`, each expanding to its template functions `<n> of <count>
<fndef> [main]`). A filter field matches case-folded substrings of the key, part number and FnDef key; Up, Down, Home,
End move the row; Enter chooses; Escape closes (§6.4). Choosing:

| Row | Effect |
|---|---|
| Embedded symbol | Arms Place (§7.2) with the copy's shape and target `new_device` with the row's `letter` (refused `reason-letter-needed` when `letter` is `None` and no `letter` parameter is given); a terminal symbol asks for a strip and a designation (the strip list of the page tables' devices with the strip aspect, the next free designation proposed) and uses `new_terminal` |
| Starter symbol | Host commit of `Session::starter_embed(key)` (one undo step, "Embed symbol <key>"), then as an embedded symbol |
| Part function | Arms Place with the function's `symbol` row and target `from_part { part, template: index, letter }`; a function without `symbol` is disabled ("no symbol for this function on this page") |
| Exchange mode (§7.5) | Commits nothing; returns the copy to the exchange flow |

### 7.2 The Place tool (`place`; FR-SYM-015, FR-UI-017, FR-DEV-038)

State `floating` only (sticky: it stays floating after each commit with the same symbol and target; a `new_terminal`
target advances the designation when it is decimal digits, else the next commit asks again).

| State | Event | Next | Output |
|---|---|---|---|
| floating | `PointerMove` or `CursorMove` to p | floating | `Transient`: `Paths` (the shape's outline at the snapped insertion point `s = snapper.snap(p, alt = true)`'s `page_nm`, grid only), a `Marker cp` per shape CP, a `Label` with the variant letter, the target (`new F`, `join -102F2`, `terminal 1X1:3`, `part A9P52610`) and, when a fresh preview exists, the predicted tag; from part 2 the autoconnect stubs (§8.3) |
| floating | `Key(RotateCw, RotateCcw, MirrorX, MirrorY)` | floating | orientation `orient_then(o, 1, false)`, `(3, false)`, `(0, true)`, `(2, true)`; `Transient` |
| floating | `Key(NextVariant)` | floating | orientation A→B→…→H→A (`(o + 1) mod 8`); `Transient` |
| floating | `Key(NextRepType)` | floating | the next catalogue symbol (key order, wrapping) that draws the same FnDef with `allowed` set, if any (its shape fetched by the host on the next dispatch); `Transient` |
| floating | `Dwell { ms ≥ 80 }` | floating | `Preview(cmd)` |
| floating | `Press` primary or `Activate` | floating | `Commit(cmd)` |
| floating | `Back` | floating | `None` |
| floating | `Escape` | — | `None` (the host returns to Select, S2.03 §7.2) |

**The command.** `device.place {page, symbol, pos: s.page_nm, orient, target, connect: [] (part 2: the accepted
proposals)}`, where the target is the armed one, except that a `new_device` target whose insertion point lies inside
the outline of a black box on the page (exact even-odd point-in-polygon on `TableRep.black_box.outline`, the boundary
inside; innermost by area, ties by ID) becomes `join_device { device: that box's device, role: aux }` (doc 06 §2.7).
A `place.pick` with `function` arms `existing_function`. Black-box functions are placed by part 3's tool, never here.

**Keyboard:** the ghost follows the grid cursor (`CursorMove`), Enter commits, the keys above transform; the canvas
node's live region announces `Place <key>: <target>, variant <letter>, at <cell>` after each change.

### 7.3 Moving (Select's drag rows and the keyboard move)

The rows of doc 06 §4.3 join S2.03's Select table (the probe's pattern):

| State | Event | Next | Output |
|---|---|---|---|
| pressed | move beyond 3 points, the pressed entity a representation | dragging | `Transient`: `Ghost` of the moved entities' rectangles offset by the step displacement, rubber bands and the readout (§6.7). The moved set is the selection when the pressed entity is selected, else the pressed entity alone |
| dragging | move | dragging | `Transient`; steps `k = round(page displacement × den / num)` per axis (`i128`, half away from zero) |
| dragging | `Dwell { ms ≥ 80 }` | dragging | `Preview(rep.move { reps, steps: k })` when `k ≠ (0, 0)` |
| dragging | `Release` | idle | `Commit(rep.move { reps, steps: k })` when `k ≠ (0, 0)`, else `Select` (as a click) |
| dragging | `Escape` | idle | `Cancel` |
| any with a selection | `tool.move` (M) | moving | the grid cursor is placed at the primary representation's insertion point when not shown; `Transient` |
| moving | `CursorMove` | moving | steps = the cursor's grid displacement since entry; `Transient` |
| moving | `Dwell`, `Activate`, `Escape` | moving, idle, idle | as dragging: `Preview`, `Commit`, `Cancel` |

Connections, junctions and (until part 4) page items in a selection are not moved; a selection without representations
refuses `tool.move` with `reason-nothing-to-move`.

### 7.4 Transform and delete on a selection

With Select active and representations selected: `Key(RotateCw)` commits `rep.transform { reps, rotate: 1 }`,
`RotateCcw` `rotate: 3`, `MirrorX` `rotate: 0, mirror: true`, `MirrorY` `rotate: 2, mirror: true`; `NextVariant` on
exactly one representation commits the `rep.transform` whose composition gives the next letter (refused
`reason-one-rep` for several); `NextRepType` is refused (`reason-place-only`). `edit.delete_selection` (Del) commits
`edit.delete { targets }` with every resolved entity of the selection (§6.6); refused `reason-nothing-selected`.

### 7.5 Change symbol (FR-SYM-022)

`rep.exchange` (Edit ▸ Change symbol…; headless `symbol`) on exactly one selected non-IP representation opens the
picker in exchange mode listing symbols with `allowed` set; choosing one arms the `exchange` tool: state `armed`,
`Transient` = the new shape's outline at the representation's position and orientation with a `Marker cp` per mapped
CP and a dashed `Rect` around each function CP that would become unmapped; `Dwell` previews, `Activate` or a press
commits `rep.exchange_symbol { rep, symbol }`, `Escape` cancels. A starter symbol is embedded first (§7.1).

### 7.6 Actions of part 1

Label keys and English texts in `ew_viewmodel::strings`; kinds: `Command` for those that commit, else `View`.

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `tool.place` | Insert ▸ Symbol… | I | canvas | — (opens the picker) |
| `tool.place_part` | Insert ▸ Device from part… | Shift+I | canvas | — (picker filtered to parts) |
| `place.pick` | hidden | — | canvas | one of `symbol` (key or copy UUID), `part` (part number or copy UUID) with `template` (0-based), `function` (UUID); optional `letter`; for a terminal `strip` (tag or UUID) and `designation` |
| `picker.filter`, `picker.row`, `picker.choose` | hidden (the popup's field and keys) | Up, Down, Home, End, Enter | picker | `text`; `step` or `to`; — |
| `strip.new` | Insert ▸ Terminal strip… | — | canvas | `letter` (default `X`) |
| `tool.rotate_cw`, `tool.rotate_ccw`, `tool.mirror_x`, `tool.mirror_y` | Edit | R, Shift+R, X, Y | canvas | — |
| `tool.next_variant`, `tool.next_rep_type` | Edit | V, Shift+V | canvas | — |
| `tool.move` | Edit ▸ Move | M | canvas | — |
| `tool.finish` | hidden | Shift+Enter | canvas | — |
| `text.input` | hidden | — | canvas | `text` |
| `edit.delete_selection` | Edit ▸ Delete | Delete | canvas | — |
| `rep.exchange` | Edit ▸ Change symbol… | — | canvas | optional `symbol` |

A `Command` action is disabled with its reason on a read-only project or a generated page (doc 16 §17 item 9) and on a
page type that has no wire or symbol tools (doc 06 §2.2). `clears_banner` gains every action above.

### 7.7 Transcript and describe of part 1

**Transcript:** `canvas` gains, after `link_preview`: `ghost` (`null`, or `{"kind": "place", "symbol": "<key>",
"variant": "A", "rep_type": "multi_line", "at_nm": [x, y], "target": "new" | "join" | "function" | "part" |
"terminal", "tag": "-101F2" or null, "hints": 0}`, or `{"kind": "move", "count": n, "steps": [kx, ky]}`, or
`{"kind": "exchange", "symbol": "<key>", "unmapped": ["3"]}`) and `picker` (`null` or `{"mode": "place" | "exchange",
"filter": "", "rows": n, "row": i}`).

**`ui.describe`** gains, after the S2.03 canvas lines (texts in quotes are user text; enum values not in the vocabulary
are quoted):

```text
canvas picker filter "mcb" rows 3                      (while open; then one row line each)
  row symbol "ew.iec.mcb-1pn@qet10" embedded
  row symbol "ew.iec.mcb-1p@qet10" starter
  row part "A9P52610" function 1 of 1 "ew.fn.mcb-1pn" main
canvas ghost place symbol "ew.iec.mcb-1pn@qet10" variant "A" rep "multi_line" at 254.000 101.600 mm target new letter "F" tag "-101F2"
canvas ghost move 2 steps 3 0 readout "dx 7.938 dy 0.000 mm"
canvas ghost exchange symbol "test.signal-lamp@qet10" unmapped "3"
```

`VOCABULARY` gains `picker`, `rows`, `symbol`, `part`, `function`, `main`, `embedded`, `starter`, `ghost`, `place`,
`variant`, `rep`, `target`, `new`, `join`, `existing`, `terminal`, `strip`, `letter`, `tag`, `move`, `steps`,
`exchange`, `unmapped`, `floating`, `dragging`, `moving` and `readout` (28 words); FT-314's harness help lists them.

## 8. Part 2: wiring

### 8.1 The router (`ew_commands::route::auto_route`; doc 06 §5.4)

Input: the two ends (point and leaving direction, `None` for a junction or a split point), obstacle rectangles (the
`body` of every representation on the page except those of the two ends, inflated by one grid step), the grid and the
search area (the bounding box of both ends grown by 4 grid steps, clipped to the page's drawing area). Algorithm: A*
over grid nodes inside the area, 4-neighbourhood, cost 1 per step and 4 per bend, the heuristic the Manhattan distance
in steps; a CP end first leaves one step along its direction (the stub; a stub into an obstacle is allowed). Ties are
broken lexicographically on (bends, length, first-turn direction in the order right, down, left, up), so the result is
deterministic (NFR-REL-07). Without a path inside the area the fallback is the provisional L route of `ew-scene`'s
rule (the corner from the ends' own coordinates). Output: the full vertex list, interior collinear vertices removed,
interior vertices on the grid, the ends exact. Pure, integer-only, `i128` where products occur; a unit-test table
covers straight, L, Z, obstacle detours, the fallback and the tie rule.

### 8.2 The Wire tool (`wire`; doc 06 §4.4, FR-UI-015, FR-CON-001, FR-CON-006)

**Targets** in priority order within 8 screen points (doc 06 §5.7, Wire row): a CP of the page tables (of a compatible
discipline: the page type's, doc 06 §2.2), a junction, a point on a connection segment (the nearest grid point on the
segment, or the CP-exact coordinate of `require_split_point`), else the grid point. Alt suppresses object snaps.

| State | Event | Next | Output |
|---|---|---|---|
| armed | `PointerMove`, `CursorMove` | armed | `Transient`: the snap `Marker` of the target under the point |
| armed | `Press` or `Activate` on a CP, junction or segment | routing | start fixed; `Transient` |
| armed | `Press` or `Activate` on empty grid | armed | `None` (a wire needs two endpoints) |
| routing | move | routing | `Transient`: the routed ghost `Polyline` from the last fixed vertex to the snapped point through `auto_route` (one call per move whose snapped point changed), the fixed bends, the end target's marker, the length readout |
| routing | `Press` or `Activate` on empty grid | routing | a bend is fixed at the grid point (the route so far is kept as fixed vertices) |
| routing | `Back` | routing | the last fixed bend is removed; from no bend, `None` |
| routing | `Dwell { ms ≥ 80 }` on a valid end | routing | `Preview(connection.draw)` |
| routing | `Press` or `Activate` on a CP, junction or segment other than the start | armed | `Commit(connection.draw { a, b, page, route, kind })` |
| routing | `Key(EndInIp)` (part 3) | armed | `Commit(connection.draw_to_ip)` (§9.2) |
| routing | `Escape` | armed | `Cancel` |
| armed | `Escape` | — | `None` (back to Select) |

`route` is the fixed vertices joined with `auto_route` from the last fixed vertex to the end, so the committed route
is exactly the ghost. Ends: a CP → `cp`, a junction → `junction`, a segment point → `on_connection { connection, at }`.
`kind` is the page type's wire kind (doc 06 §2.2). **Keyboard wiring** is the S2.03 grid cursor with `]`, `[`, `N` and
Enter; the live region announces `Wire: routing from <label>. Click a connection point; Backspace removes the last bend;
Escape cancels` (doc 16 §4.5).

### 8.3 Autoconnect proposals (FR-CON-003, FR-CON-008; doc 06 §5.2)

`autoconnect::proposals(shape at pos and orient, tables) -> Vec<Proposal>` in `ew-viewmodel`, used by Place (on every
move of the ghost) and by Select's drag (shown only; committed only after `Key(ConnectOnMove)`):

1. On a page type with autoconnect off (doc 06 §2.2) nothing.
2. For each ghost CP without a connection, ascending symbol CP index: walk from the CP along its direction one grid step
   at a time, inside the drawing area: a representation body (other than the ghost's) stops the walk; a free CP `q` of
   the same discipline with `q.dir` opposite to the CP's direction, at the walk point, is a proposal `Cp(q)`; a connection
   segment collinear with the ray at the walk point stops the walk (the line is occupied); a segment crossing the ray
   is passed.
3. When two ghost CPs lie on one segment of a connection W with a stored route and face its two ends, both proposals are
   replaced by the split of W: no hint is sent for them, because normaliser 7 splits W when the symbol lands on its
   route (FR-CON-008, doc 06 §5.3); the split is drawn as two dashed stubs at the CPs and counted as one proposal. On a
   route-less W no split is proposed and the status hint reads "Give this wire a route first (Edit route)".
4. Ties by CP ID (v5); `Key(NextProposal)` (Tab) cycles each ray to its next hit further along, wrapping.

Proposals are `Line` overlay stubs (dashed, `ghost` token) and become `connect: [ConnectHint { cp, to }]` of the
committed `device.place` (Alt held at the commit sends none, FR-CON-003's rejection). During a move they are shown and
never committed: **moves never add connections in this package**, and `tool.connect_on_move` (C) is refused with
`reason-connect-on-place` until the follow-up of §17.4 item 2.

### 8.4 Route edit (FR-CON-007)

Select gains a row: a press on a segment of a **selected** connection that is not its first or last segment starts
`dragging-segment` (state name `dragging`): a horizontal segment moves vertically and a vertical one horizontally, in
grid steps; its two neighbours stretch; the readout shows the offset; `Release` or `Activate` commits
`connection.route { connection, route }`, `Escape` cancels. The keyboard form is `tool.route_edit` (Edit ▸ Edit route)
on one selected connection: Tab cycles the movable segments, the grid cursor's row or column is the new position,
Enter commits. On a route-less connection the first edit (pointer or keyboard) starts from its drawn `polyline`, so
the commit stores it with the moved segment (the remedy of §8.3 item 3).

### 8.5 The junction sequence editor (FR-CON-005)

`junction.open` (Enter on exactly one selected junction, or Edit ▸ Junction sequence…) opens a popup: the style
(`dot` or `bevel` with its orientation), the wiring (`star`, `chain`, `style default`), the sequence as a list of members
named by their far endpoint's label (doc 06 §3.5), and below it the physical wires of `Session::junction_wires`. Each
change commits `junction.edit` immediately (one undo step each, D11): `junction.member` with `from` and `to` indices
(Alt+Up and Alt+Down on the focused row) reorders, `junction.style` with `style` (and R on a bevel cycles its
orientation), `junction.wiring` with `wiring`. The popup refreshes from the next page tables and closes with
`ui.dismiss`.

### 8.6 Crossings as hops (FR-CON-031)

`project.crossings` (Project ▸ Drawing ▸ Show crossings as hops, checked) commits `project.drawing.set { crossings }`.
`ew-derive` reads `settings.extra.drawing` through a tracked read (`drawing_settings`), and `ew-api`'s page view passes
the value to `ew-scene`. **`ew-scene::hops`**: for every proper crossing of a horizontal segment of one connection and a
vertical segment of another (an interior point of both, no shared endpoint or junction there), in connection-ID order,
the horizontal segment is drawn interrupted on `[x − r, x + r]` with a half circle of radius `r = 1 000 000` nm above it
(towards −y); a crossing closer than `2r` to another hop on the same segment or to a vertex is drawn plain. Default and
absent: plain, so Golden A's output is byte-equal. The PDF uses the same display list.

### 8.7 Actions, transcript and describe of part 2

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `tool.wire` | Insert ▸ Connection | W | canvas | — |
| `tool.next_proposal` | hidden | Tab | canvas | — |
| `tool.connect_on_move` | hidden | C | canvas | — |
| `tool.route_edit` | Edit ▸ Edit route | — | canvas | — |
| `junction.open` | Edit ▸ Junction sequence… | Enter on a selected junction | canvas | — |
| `junction.member`, `junction.style`, `junction.wiring` | hidden (the popup) | Alt+Up, Alt+Down | junction | `from`, `to`; `style`, optional `orient`; `wiring` |
| `project.crossings` | Project ▸ Drawing (checked) | — | global | optional `on` |

**Transcript:** `canvas` gains `route` (`null` or `{"from": "<label>", "bends": n, "points": n, "end": "<label>" or
null}`) after `picker`, and `junction_editor` (`null` or `{"junction": "<uuid>", "style": "bevel", "wiring": "chain",
"sequence": ["+#1", "-102F1:1", "+#2"]}`); `ghost.hints` counts the proposals.

```text
canvas route from "-102F1:2" bends 1 points 4 end "-102F2:24VDC#1"
canvas proposals 2 "2 -> 1X1:1" "4 -> 1X1:2"              (while a Place ghost has proposals)
canvas junction "<uuid>" style bevel wiring chain sequence "+#1" "-102F1:1" "+#2"
  wire "W20" from "-102G1:+#1" end "-102F1:1"
canvas crossings hops                                   (only when set)
```

`VOCABULARY` gains `route`, `from`, `end`, `bends`, `points`, `proposals`, `junction`, `style`, `wiring`, `sequence`,
`wire`, `segment`, `routing`, `crossings`, `plain`, `hops`, `dot`, `bevel`, `star`, `chain` and `connect` (21 words).

## 9. Part 3: interruption points and black boxes

### 9.1 The IP tool (`ip`; doc 06 §3.5, §4.5)

State `floating` (sticky). The ghost is the IP symbol of the catalogue (`ew.gen.ip` of the page's grid family, embedded
first when needed, §7.1) at the snapped grid point, with a `Label` `<name> <direction>`. **Name proposal:** the
potential name of the CP the IP would connect to when that CP is under the ghost's CP, else the last committed name of
this tool; then, when the proposal is already used on the page by an IP of the same direction, the next name of its
numbered series (`24VDC-1` → `24VDC-2`: the trailing decimal digits after the last `-` incremented, else `-1`
appended). **Direction default:** `source` when the insertion point lies in the right or bottom quarter of the drawing
area, `target` in the left or top quarter, else the last used; `Key(ToggleDirection)` (D) cycles source → target →
undirected. `Key(ToggleStar)` toggles the star source. `Text(s)` (`ip.name`) sets the name. `Dwell` previews and a
press or `Activate` commits `ip.place { page, symbol, pos, orient, name, direction, star_source }`.

### 9.2 Ending a wire in an IP

In the Wire tool's `routing` state, `Key(EndInIp)` (P) commits `connection.draw_to_ip` with the IP at the routed end
point (the last snapped grid point; the IP oriented so its CP faces the arriving segment: the orientation among A–D
whose CP direction is opposite to the last segment's direction), named and directed by §9.1's rules. When no Wire tool
is routing, `tool.ip` (P) activates the IP tool (one action, routed by the host by the active tool's state).

### 9.3 Continue and arrange (FR-XRF-014, FR-XRF-013)

`ip.continue_on` (Edit ▸ Continue on page…; headless `folio`, optional `x_mm`, `y_mm`) on exactly one selected IP
commits `ip.continue { ip, page, pos }`, `pos` default: the same y; x the leftmost grid point inside the target page's
drawing area when the original lies in the right half of its page, else the rightmost (the drawing area from the first
and last `PageCells::cell_rect`). `ip.arrange_selected` (Edit ▸ Arrange interruption points; `by`: `name` or
`potential`, `chain_order` bool) on at least two selected IPs on one page sorts them in the view model (natural order of
the names, or of the table potential names then the names, ties by function ID) and commits `ip.arrange { ips,
set_sort: chain_order }` (D12); refused `reason-select-ips`.

### 9.4 The black box tool (`black_box`; FR-DEV-013, FR-DEV-039)

Armed from `tool.black_box` (B) with a target chosen like a placement (the picker's black-box rows: a black-box symbol
with `new_device` and the row's letter, or a part function with `black_box` set and `from_part`). States: `armed` →
press or `Activate` fixes the first corner → `outline` (the rectangle to the snapped grid point, `Transient`) → press or
`Activate` fixes the opposite corner and commits `box.black { page, symbol: the black-box symbol, pos: the top-left
corner, box: { outline: the rectangle relative to pos, cps, inner: [] }, target }`. **Default CPs** for a part target: the
part pattern's CPs of the template entry in pattern order, the first `⌈n/2⌉` on the top edge and the rest on the bottom
edge, left to right one grid step apart starting one grid step from the left corner, directions up and down; when an
edge is too short the rectangle grows to the right until they fit (the ghost shows it). Without a part, no CPs. Escape
cancels from `outline`; `None` from `armed`.

**Grips.** A selected black box shows its corner and edge-midpoint grips and its CPs (overlay markers). Dragging a grip
(or the keyboard form `box.grip` with `grip` index and the grid cursor) changes the rectangle and commits `box.reshape`
without `cps` (re-projection); dragging a CP along the outline commits `box.reshape` with `cps`; `box.add_cp` (Edit) adds
a CP at the grid cursor on the outline with the next free index. Polygon outlines are a follow-up (§17.4).

### 9.5 Picker integration

From part 3 the picker's black-box rows are enabled and arm this tool instead of Place.

### 9.6 Actions, transcript and describe of part 3

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `tool.ip` | Insert ▸ Interruption point | P | canvas | — |
| `tool.ip_direction`, `tool.star_source` | Edit | D, — | canvas | — |
| `ip.name` | hidden (the tool strip field) | — | canvas | `text` |
| `ip.continue_on` | Edit ▸ Continue on page… | — | canvas | `folio`, optional `x_mm`, `y_mm` |
| `ip.arrange_selected` | Edit ▸ Arrange interruption points | — | canvas | `by`, `chain_order` |
| `tool.black_box` | Insert ▸ Black box | B | canvas | as `place.pick` |
| `box.grip`, `box.add_cp` | hidden; Edit ▸ Add connection point | — | canvas | `grip`; — |

`ghost` gains `{"kind": "ip", "name": "24VDC-2", "direction": "source", "star": false, "at_nm": [x, y]}` and `{"kind":
"box", "outline": 4, "cps": 7}`.

```text
canvas ghost ip name "24VDC-2" direction source star off at 396.875 47.625 mm
canvas ghost box outline 4 cps 7
```

`VOCABULARY` gains `ip`, `name`, `direction`, `source`, `undirected`, `outline`, `cps`, `arrange`, `by` and `potential`
(10 words).

## 10. Part 4: page items, wire markers and structure boxes

### 10.1 The record kinds (D10)

**`page_item`** (rank 43, page file; doc 03 §6): `{"$k": "page_item", "id", "v", "page", "layer"?: Ident, "kind":
<variant>}`, the variant an externally tagged object:
- `graphic`: `{"shape": {"line": {"a", "b"}} | {"polyline": {"points"}} | {"rect": {"min", "max"}} | {"circle":
  {"centre", "r"}} | {"arc": {"centre", "r", "start": Angle2, "sweep": Angle2}} | {"polygon": {"points"}}, "stroke"?:
  {"width": Nm}, "dash"?: "solid" | "dashed" | "dotted" | "dash_dot", "fill"?: bool}`;
- `free_text`: `{"text": MlText, "bind"?: Ident, "at": Pt2, "angle"?: 0 | 90 | 180 | 270, "style"?: Ident (default
  `text.normal`), "align"?: "left" | "centre" | "right"}` (`role` `path_function` is a follow-up and kept verbatim);
- `wire_marker`: `{"connections": [IdS], "at": Pt2, "angle"?: 0 | 90, "fields": ["csa", "colour"]}` (`at` and `angle`
  are added to doc 03's shape: they position the group);
- any other variant (`macro_box`, `surface_view`, `model_view`, `dimension`, `underlay`, and `image` until part 5) is kept
  verbatim as `PageItemKind::Unknown(Value)` and round-trips byte-equal.

**`structure_box`** (rank 44, page file; doc 03 §5): `{"$k": "structure_box", "id", "v", "page", "outline": [Pt2]
(≥ 3), "style": "dotted" | "dash_dot", "caption"?: MlText, "assigns": StructureAssignment, "external"?: IdS}`.

**Invariants** (two new structural entries in ADR-0009's catalogue, numbered by the design pass): a page item's or
structure box's page exists (cascade: deleting a page deletes them, FR-PAG-025); every connection a wire marker lists
exists and lies on the marker's page (delete policy: a deleted connection leaves the list; an empty marker is deleted in
the same plan). `KindStatus` of both kinds becomes `Implemented`; `ew-format` reads and writes them through the
existing record machinery; a migration test reads a page file holding both kinds plus an unknown variant and writes it
back byte-equal.

### 10.2 Derivation

1. **Structure-box containment** (FR-PRJ-020, FR-PRJ-019). A derived `rep_structure_box(rep)`: the innermost structure
   box on the representation's page whose outline contains the representation's anchor point (the point
   `rep_anchor_cell` uses; exact even-odd test in `i128`, the boundary inside; innermost by absolute shoelace area,
   ties by box ID). `structure_of(device)` gains the step between its steps 1 and 3 (the hook its doc comment names):
   an inherited aspect of a device that is not a terminal or child-tag device takes the slot of the box around its main
   representation when the box's `assigns` has a node or `none` for that aspect (`SlotSource::StructureBox`), else the
   page's. **`ip_structure(function)`**: an IP function's structure is its representation's box's slots over its page's;
   every reader of an IP's structure (the pairing scope `location` and the xref `{function}` and `{location}` tokens)
   reads it. Golden A has no structure box, so no golden output changes.
2. **Bound texts** (FR-SYM-024): `bind` names one of `project.description`, `project.document_title`,
   `project.drawing_id`, `page.title`, `page.folio`, `page.full_name`; the printed text is the value the title block
   prints for that placeholder on this page (`title_block`'s resolver), freshness class I.
3. **Marker modes** (FR-CON-024, D16): with `marker_mode` absent a connection shows a marker when `show_marker` is set
   (as built); `all`: every wire; `deviations`: a wire with an effective cross-section or colour set at levels 3 to 7,
   cores excepted (doc 08 §4.5); `explicit`: a level-7 cross-section or colour; `none`: no marker. A connection listed by
   a wire marker item draws no marker of its own; the item draws the group whenever at least one of its connections
   would show a marker in the mode.
4. **Group marker text** (FR-CON-039): the distinct effective cross-sections of the listed connections in list order
   joined by `/`, then one colour code per connection in list order (the effective colour printed as the per-connection
   marker prints it), separated by spaces: `2.5 BK BU`, `1.5 DBU DBU WH WH`.
5. **Structure-box caption:** the caption text, then, when `external` is set, ` (<external>)` with the external
   reference printed as the external xref texts print it: `External Connection (J-E-206 / JE-N1000-03)`.

### 10.3 Drawing (`ew-scene::items`)

Graphics and structure-box outlines on the existing `Layer::Symbols`, free texts, wire markers and captions on
`Layer::Labels` (no new `ew-dl` layer, so the permissive display-list crate is unchanged); graphics in the page's wire
stroke unless `stroke` is set; free texts in their style and alignment, readable rotation as slot texts (doc 06 §2.5); wire markers with the `marker` style
over the `ew.gen.wire-marker` symbol at `at` and `angle` (the per-connection rule's drawing); structure boxes as the
outline in the page's wire stroke width and the dash pattern (dotted: 0.5 mm on, 1 mm off; dash-dot: 4 mm on, 1 mm off,
0.5 mm on, 1 mm off) with the caption in `tag.main` above the outline's top-left vertex. Each is tagged with its
entity reference, so WP-S2.02's entity rectangles pick it. The PDF writes the same display list.

### 10.4 Commands and amendments of part 4

`box.structure`, `item.place`, `marker.place` (§5.1); `rep.move` `items`, `edit.delete` and `fields.set` kinds,
`project.drawing.set` `marker_mode` (§5.2). From this part on, Select's drag and `tool.move` move page items and
structure boxes with the representations (`rep.move { reps, items, steps }`), and Delete deletes them.

### 10.5 The tools of part 4

| Tool | States | Commit |
|---|---|---|
| Text (`text`, T) | `armed` → press or `Activate` at the fine-snapped point opens the text field popup (`typing`); `Text(s)` sets the text; Enter (or `tool.finish`) → commit; `Escape` closes the popup and returns to `armed` | `item.place { free_text: { text: s, at, angle, style } }`; with the tool strip's binding chosen (`text.bind` with `bind`), `bind` instead of `text` |
| Text edit | In Select, a double click (or `text.edit`, F2) on a selected free text opens the popup with its text | `fields.set { kind: page_item, targets: [id], path: "kind.free_text.text", value }` |
| Graphic (`graphic`, L for a line; Insert ▸ Graphic ▸ line, polyline, rectangle, circle, arc, polygon) | `armed` → `drawing`: line and rectangle by two points; circle by centre and a point on it; arc by centre, start point and end point (sweep clockwise, Shift for anticlockwise); polyline and polygon by points, `Back` removing the last, `tool.finish` ending | `item.place { graphic: { shape } }` |
| Marker (`marker`, Shift+W; doc 06 §4.5) | `armed` → `crossing`: a rubber line from the press point to the pointer; every connection segment it crosses, in the order along the line, is collected; release or `Activate` → the tool strip's fields (cross-section, colours; `marker.fields` with `csa`, `colours`) → Enter commits | `marker.place { connections, at: the crossing line's midpoint snapped to the fine grid, angle: 90 when the crossed segments are horizontal, csa, colours }` |
| Structure box (`structure_box`, Shift+B) | `armed` → `outline` (a rectangle by two fine-snapped points; polygon points with `tool.finish`); then the tool strip's style, caption, location node and external reference (`structure_box.fields`) → Enter | `box.structure` |

All previews on a dwell of 80 ms as elsewhere; Escape returns to `armed`, from `armed` to Select.

### 10.6 The marker mode action

`project.marker_mode` (Project ▸ Drawing ▸ Wire markers ▸ four checked items and "As set per wire") commits
`project.drawing.set { marker_mode }`.

### 10.7 Actions, transcript and describe of part 4

| ID | Menu | Shortcut | Context | Params (headless) |
|---|---|---|---|---|
| `tool.text`, `tool.graphic` | Insert ▸ Text; Insert ▸ Graphic ▸ … | T; L (line) | canvas | `tool.graphic`: `shape` |
| `text.bind`, `text.edit` | hidden (tool strip); Edit ▸ Edit text | —; F2 | canvas | `bind`; — |
| `tool.marker`, `marker.fields` | Insert ▸ Wire marker; hidden | Shift+W | canvas | —; `csa`, `colours` |
| `tool.structure_box`, `structure_box.fields` | Insert ▸ Structure box; hidden | Shift+B | canvas | —; `style`, `caption`, `location`, `external` |
| `project.marker_mode` | Project ▸ Drawing (checked) | — | global | `mode` |

`ghost` gains `{"kind": "text", "at_nm": [x, y], "text": "…"}`, `{"kind": "graphic", "shape": "polyline", "points":
n}`, `{"kind": "marker", "connections": n, "text": "2.5 BK BU"}` and `{"kind": "structure_box", "outline": n}`.

```text
canvas ghost item text at 50.800 30.480 mm text "40x60"
canvas ghost item graphic polyline points 3
canvas ghost marker connections 2 text "2.5 BK BU"
canvas ghost box outline 4
```

`VOCABULARY` gains `item`, `text`, `graphic`, `line`, `polyline`, `rect`, `circle`, `arc`, `polygon`, `marker`,
`connections`, `typing`, `drawing` and `crossing` (14 words). The word `outline`, used by parts 3 and 4, is added by
whichever of the two integrates first, and the other's list drops it.

## 11. Part 5: images

After FT-337 has delivered the content-addressed asset store and part 4 has been integrated: the `image` variant
`{"asset": "sha256:…", "rect": [Pt2, Pt2]}` joins `PageItemKind` (records written by part 4's build round-trip as
unknown variants until then), `item.place` takes `{"image": {asset, rect}}` (rejected when the project holds no asset
with that hash), the Image tool (`image`, Insert ▸ Image…; headless `image.pick` with an asset hash already in the
project, or a file path in the window only) places it by two fine-snapped corners keeping the asset's aspect ratio
(FT-337's `AssetRef.size`), and `ew-scene` draws PNG as an image item and SVG as vector paths through the SVG reader
FT-337 delivers (FR-SYM-025: vector on screen and in the PDF). The lane uses FT-337's API as delivered; if it offers no
SVG-to-path reader or no `AssetRef.size`, the lane stops and reports. `ghost` gains `{"kind": "image", "asset": "…"}`
and `VOCABULARY` gains `image` (1 word). The window's file drop of doc 16 §4.8 (SVG or PNG from the OS) maps onto the
same tool.

## 12. Determinism, the headless mode and exact counts

1. **Every flow is a script.** Each acceptance flow of §13 that runs in kittest also runs as an `ew.uiscript/1` script
   through the built binary with byte-equal transcripts on two runs. No timer, clock or hash order reaches the model;
   dwell and double click are actions (S2.03 D11).
2. **Every commit is reproducible.** The `--record` log of a tool script, applied with `ew apply` to the starting
   project, gives a project whose records equal the scripted session's (a test per part); `connection.draw` carries its
   route, so the router never runs on replay.
3. **Tie-breaks are named** for every choice: proposals, routing, picking, innermost boxes, arrange sorting, the IP name
   series, the hop rule. None depends on insertion order of a hash map or on time (NFR-REL-07).
4. **Exact counts** (NFR-PERF-20; WP-S2.02 A9's counters, S2.03 §8 item 3): after a page's first frame, pointer and
   cursor moves with any tool armed, key transforms of a ghost, picker filtering and Tab cycling cause 0 memo
   re-executions and 0 `page_content`, `page_tables`, `page_cps` or `symbol_shape` calls; a dwell that changes the
   command or the revision calls `Session::preview_with` exactly once; a commit makes exactly one `commit`,
   `execute_with` or `run_script` call; after a commit the next frame makes exactly one `page_content` and one
   `page_tables` call for the page, and one `symbol_shape` call only when the revision changed and a tool holds a shape.
5. **No time values, paths or process IDs** in transcripts or describe text.

## 13. Acceptance checks

All tests are ordinary nextest tests in the existing jobs, on temporary copies of Golden A (or of a project made by
the wizard, §13.6) with `EW_CONFIG_DIR` set to a temporary directory. A golden example that the as-built data does not
support (a record, a geometry, a box that is not there) stops the lane for a lead ruling, as WP-S2.03's ruling 5 did.

### 13.1 Part 1

| # | Check | Where | Proves |
|---|---|---|---|
| PL1 | Public items: `starter::items` count and parse; `place_catalogue` on folio 102 of Golden A (grid family, `letter` of `ew.fn.mcb-1p` equal to `ew-rds`'s mapping of its category, which -102F1's letter F shows, `allowed`, black-box and terminal flags) and on a new wizard project (starter rows only); `starter_embed` closure minus embedded; `symbol_shape` CPs equal the scene's CP positions for every symbol of folio 101 under all 8 orientations; `geometry` equal to `Orient2`; `page_tables` on folios 101 to 103 against the golden tables (17, 12 and 23 representations; every `TableCp.at` equal to `geometry::cp_position`; connection polylines equal to the scene's) | `ew-library`, `ew-api` tests | §4.3 |
| PL2 | Commands: `from_part` with a two-function fixture part (a copy of the MICO 4.10 part whose template is the black box without CPs 13, 14 plus a `contact-no` 13-14 entry): placing the main function leaves the contact unplaced (FR-UI-017); `new_terminal` on `-X1031` (AC-28's step); `rep.move` `steps` on three representations of folio 101 with different `k mod 3` moved one step (each on the grid; before the amendment the same move with a delta is refused), routes following; `rep.exchange_symbol` keeping E56's two connections with a fixture symbol `test.signal-lamp@qet10` (FR-SYM-022) and refusing an unmapped connected CP; every rejection; DSL verbs and fuzzer arms | `ew-commands`, `ew-testkit` tests | §5 |
| PL3 | Place: the state table; FR-SYM-015's acceptance (placing a contact, V cycles A to H, the committed representation's orientation is the chosen one); R, X, Y compositions; the black-box containment join on folio 102 (a contact dropped inside -102F2's outline joins -102F2 as an auxiliary function); sticky placement with the next counter in the ghost tag; Escape leaves the revision unchanged | `ew-viewmodel` tests | §7.2 |
| PL4 | Move: drag and keyboard move of a selection of two representations on folio 101 by 1, 2 and 3 steps (exact positions, routes following, rubber bands and readout in the overlay); the selection after the commit; Escape; transform keys and Delete on a selection; `reason-one-rep`, `reason-nothing-to-move` | `ew-viewmodel` tests | §7.3, §7.4 |
| PL5 | Picker and embedding: a starter symbol embedded once (one `library.embed`, one undo step), a second placement embeds nothing; the grid-family filter; disabled rows with reasons | `ew-viewmodel` tests | §7.1 |
| PL6 | AC-28's terminal step: kittest on Golden A, folio 103, add a terminal to -X1031 with the ghost showing the predicted tag after the dwell, commit, undo | `ew-ui` tests | §7.2, AC-28 |
| PL7 | Transcript and describe of §7.7; the 28 words; the folio-101 placement script of §13.6 (placements only) twice byte-identical; its `--record` log applied with `ew apply` gives equal records | `ew-viewmodel`; `bin/elektrawatt/tests` | §7.7, §12 |
| PL8 | Exact counts of §12 item 4 for Place, Move and the picker | `ew-ui` test | §12 |
| PL9 | Golden unchanged: the existing golden output, acceptance and AC-15 tests pass unmodified | lane | §4.1 rule 5 |
| PL10 | Policy and targeted checks as WP-S2.03's V13 and V14 for the changed crates (`ew-library`, `ew-api`, `ew-commands`, `ew-testkit`, `ew-render2d`, `ew-viewmodel` with and without `testkit`, `ew-ui`, `elektrawatt`), `cargo xtask docs-gen --check`, `check-deps`, `check-features`, `check-raw-read`, `check-licence-map`, `reuse lint`; no new package | lane | testing policy |

### 13.2 Part 2

| # | Check | Where | Proves |
|---|---|---|---|
| WR1 | Router table tests (§8.1) and determinism under permuted obstacle order | `ew-commands` tests | §8.1 |
| WR2 | Wire tool: the state table; ends on CP, junction, segment (a junction created and the segment split) and empty grid refused; bends and Backspace; the committed route equal to the ghost; FR-CON-006's acceptance (two wires drawn to -102F1:1 on a copy listed in drawing order in the page tables) | `ew-viewmodel` tests | §8.2 |
| WR3 | FR-UI-015's acceptance: kittest without pointer events on Golden A folio 102 draws a connection from -102F1:2 to -102F2:24VDC#1 (grid cursor, `]`, `[`, `N`, Enter): exactly one new connection with those two endpoints, revision + 1, the walker passing after every step | `ew-ui` tests | §8.2 |
| WR4 | Autoconnect: proposals on Golden A where the geometry gives facing free CPs (folio 101's S51 over 1X1 in a copy where W4 and W5 were deleted) equal the deleted connections' endpoints; FR-CON-003's acceptance (a contact dropped between two aligned wire ends previews two connections; Alt commits none); Tab cycling; a move never adds a connection | `ew-viewmodel` tests | §8.3 |
| WR5 | FR-CON-008's acceptance: W13 given a route by the route editor, then a contact inserted: two connections, W13's ID and number on the F55:2 side, the same overrides on both, the new part printing `?` with online wire numbering off | `ew-viewmodel`, `ew-api` tests | §8.3, §8.4 |
| WR6 | FR-CON-007's acceptance: a segment of a connection on folio 102 dragged, saved, reopened: the same route to the nm | `bin/elektrawatt/tests` | §8.4 |
| WR7 | FR-CON-005's acceptance: the junction editor of J1 on folio 101 lists the F55:2 member before E55:L and E56:X2 and the physical wires W13 and W14; a reorder on J4 commits one `junction.edit` and the wires follow; style and wiring toggles | `ew-viewmodel`, `ew-api` tests | §8.5 |
| WR8 | FR-CON-031's acceptance: `crossings` hops changes folio 103's display list only by the hop arcs (every crossing of the rule hopped), the derived diff is empty and no connection changes; absent and `plain` give Golden A's bytes | `ew-scene`, `ew-api` tests | §8.6 |
| WR9 | Transcript, describe and the 20 words; the folio-101 and folio-102 wiring scripts of §13.6 twice byte-identical; replay by `ew apply` | `ew-viewmodel`; `bin/elektrawatt/tests` | §8.7, §12 |
| WR10 | Exact counts for the Wire tool and proposals (one `auto_route` per changed snapped point, no `Session` call); golden unchanged; policy and targeted checks as PL9, PL10 | lane; `ew-ui` test | §12 |

### 13.3 Part 3

| # | Check | Where | Proves |
|---|---|---|---|
| IB1 | Commands: `connection.draw_to_ip`, `ip.arrange`, `box.reshape` with every rejection; DSL verbs and fuzzer arms | `ew-commands`, `ew-testkit` tests | §5.1 |
| IB2 | IP tool: name proposal from the potential and the series (`24VDC-1` → `24VDC-2`), the direction default by quarter and D, star toggle; ending a wire in an IP is one transaction and one undo step | `ew-viewmodel` tests | §9.1, §9.2 |
| IB3 | FR-XRF-014's acceptance: on a copy of Golden A with 24VDC-2's partner on 103 deleted, "Continue on page 103" from 24VDC-2 on 102 places a paired IP on 103 at the default position; the pairing derives | `ew-viewmodel`, `ew-api` tests | §9.3 |
| IB4 | FR-XRF-013's acceptance: on a copy where N-1 on 102 was moved above L-1, Arrange by name puts L-1 in the upper slot as on 101 | `ew-viewmodel` tests | §9.3 |
| IB5 | FR-DEV-013's acceptance: the black box tool with the MICO 4.10 part on a new page gives a device whose 10 CPs (24VDC two, GND two, 13, 14, Out1 to Out4) all report it in the page tables | `ew-viewmodel` tests | §9.4 |
| IB6 | FR-DEV-039's acceptance: widening -102G1's box by two grid steps (right-edge grip) keeps its 7 CPs on the outline and all 7 connected | `ew-viewmodel`, `ew-commands` tests | §9.4 |
| IB7 | Transcript, describe, the 10 words; the IP and black-box steps of the folio scripts (§13.6) twice byte-identical; replay; exact counts; golden unchanged; policy and targeted checks | lane; tests | §9.6, §12 |

### 13.4 Part 4

| # | Check | Where | Proves |
|---|---|---|---|
| PI1 | Record kinds: schema round trip of every typed variant and of unknown variants byte-equal; the two invariants and the delete policies; a reader of the previous build keeps the records verbatim (format compatibility test) | `ew-schema`, `ew-model`, `ew-format` tests | §10.1 |
| PI2 | FR-PRJ-020's acceptance: a structure box assigning `+J-E-206` around the external points of folio 101 on a copy of Golden A: `ip_structure` of the three external points is `+J-E-206`; a device's main representation inside the box takes the box's location with `SlotSource::StructureBox`; moving the point out reverts to `+J-E-2416` | `ew-derive`, `ew-api` tests | §10.2 |
| PI3 | FR-SYM-024's acceptance: a free text bound to `project.description` prints the new description after a `fields.set` on the project | `ew-api` tests | §10.2 |
| PI4 | FR-SYM-023's acceptance: a line drawn onto a CP and the texts "40x60" leave every net, potential and wire row unchanged | `ew-api` tests | §10.1 |
| PI5 | FR-CON-039's acceptance: `marker.place` over W4 and W5 below S51 on a copy of Golden A prints `2.5 BK BU` once there and no per-connection marker for W4 and W5; a group over four connections prints `1.5 DBU DBU WH WH` | `ew-derive`, `ew-api` tests | §10.2 |
| PI6 | FR-CON-024's acceptance: on Golden A, `explicit` prints exactly the per-connection markers that the absent setting prints (the four reference groups' ten wires); `none` none; `all` one per wire; `deviations` the wires with a level 3 to 7 value | `ew-api` tests | §10.2 |
| PI7 | Tools: Text (with binding and edit), Graphic (every shape, Back, finish), Marker (crossing order, fields), Structure box; move and delete of items and boxes with representations in one command | `ew-viewmodel` tests | §10.5 |
| PI8 | Commands and amendments with every rejection; DSL verbs and fuzzer arms; the catalogue at 50 commands | `ew-commands`, `ew-testkit` tests | §5 |
| PI9 | Transcript, describe, the 14 words; the folio-101 structure-box and marker steps of §13.6 twice byte-identical; replay; exact counts; golden unchanged; policy and targeted checks, including `check-licence-map` for the touched permissive-boundary crates | lane; tests | §10.7, §12 |

### 13.5 Part 5

| # | Check | Where | Proves |
|---|---|---|---|
| IM1 | FR-SYM-025's acceptance: an SVG logo placed as an image item is vector paths in the page's display list and in the exported PDF (no raster image object for it); a PNG is an image object; an unknown hash is refused | `ew-api`, `ew-export` tests | §11 |
| IM2 | The Image tool, transcript, describe (`image`), replay; golden unchanged; policy and targeted checks | lane; tests | §11 |

### 13.6 The folio scripts and the G-PP rehearsal

**Start.** A new project made headless by the company-standard wizard with its defaults (WP-S2.01's flow: the sample
convention's profiles, numbering, frames, structure and sections), then `page.create` for 101, 102 and 103 as Golden A
holds them (multi-line, the QET grid frame, grid `qet10`), the structure node `+J-E-206` and the external reference
`JE-N1000-03` by `command.run`, and the golden part copies embedded by `command.run` `library.embed` from
`golden/J-E-2416.ew/lib/`.

**Generator.** A test-support module generates the UI script of a folio from Golden A's records (opened read-only from
`golden/J-E-2416.ew`): each representation by the gesture of the part that owns it (Place with `place.pick` and the
golden position as `canvas.press` in mm; terminals with `new_terminal`; black boxes with the black box tool; IPs with the
IP tool or the Wire tool's P), each drawn connection with the Wire tool from its golden ends (junction feeds by ending on
the segment), each junction's style, wiring and sequence through the junction editor, typed tags, parts, potential
definitions and records no WP-S2.04 tool draws (strip terminals of -X1031 and -X1032, cable cores, bridges) through
`command.run` with the golden parameters, and, on 101 and 102, the structure box and the four marker groups of doc 08
§4.5 (part 4). Steps a part does not yet own use `command.run` with the golden parameters, so every part runs the
scripts; the report lists the `command.run` steps left.

**Exact counts after each folio** (every part): representations 17, 12 and 23 on 101, 102 and 103; drawn connections
21, 19 and 20; junctions 2, 4 and 0. **Oracle:** every representation's derived anchor cell equals the golden table's
`cell`; every drawn connection's endpoint labels equal its golden row's `a` and `b` (up to orientation); every junction's
sequence, named by far-end labels, equals the golden one; every cross-reference text whose target lies on 101, 102 or
103 equals golden §15's (texts whose partner is on another folio are left out, because that folio is not drawn); after
part 4 the marker texts of 101 and 102 are `2.5 BK BU` twice, `1.5 DBU DBU WH WH` and `1.5 DBU DBU`, and the
external points of 101 have `ip_structure` `+J-E-206`. **G-PP rehearsal** (after part 4, `bin/elektrawatt/tests/gpp_demo.rs`):
the three folios, a save, `ew export pdf` of the project, and the generated parts list 901 equal to golden §14.1
restricted to the devices on 101 to 103, with each row's quantity counted over those devices only (AC-07 on the new
project). A difference from the oracle stops the lane for a
lead ruling.

The workspace suite, the acceptance runner and the performance runs are not run by the lanes (testing policy, ID-512);
the integrating session runs `cargo xtask ci` at the cycle delivery.

## 14. Files owned

**Part 1:** `crates/ew-library/src/{lib.rs,starter.rs}` and tests; `crates/ew-api/src/output/{mod.rs,editing.rs}`, a
new `crates/ew-api/src/geometry.rs`, `crates/ew-api/src/lib.rs` (the methods only) and tests; `crates/ew-commands/src/commands/{device.rs,mod.rs}`,
a new `commands/exchange.rs`, `crates/ew-commands/src/geometry.rs` (the step move), `schemas/**`, `tests/**`;
`crates/ew-testkit/src/golden/dsl.rs`, `tests/command_fuzz/**`; `crates/ew-render2d/src/overlay.rs`;
`crates/ew-viewmodel/src/**` (new `tools/{place,picker,move_keys,transform,exchange}.rs`, `tools/select.rs` drag rows,
`actions`, `strings`, `describe`, `script`, `canvas`, `shell`); `crates/ew-ui/**` (tool strip, picker popup, keys);
`bin/elektrawatt/tests/**` (with the script generator under `tests/support/`); `docs/book/src/reference/commands/**` and
the generated lines of `docs/book/src/SUMMARY.md`.
**Part 2:** additionally a new `crates/ew-commands/src/route.rs` and a new `commands/drawing.rs`
(`project.drawing.set`); `crates/ew-derive/src/settings.rs` (`drawing_settings`); a new `crates/ew-scene/src/hops.rs`
and one call in `page.rs`; `crates/ew-api/src/routing.rs`, `output/view.rs` (the crossings field); new
`crates/ew-viewmodel/src/tools/{wire,autoconnect,route_edit,junction_editor}.rs`; the junction popup in `ew-ui`.
**Part 3:** `crates/ew-commands/src/commands/connection.rs` (`connection.draw_to_ip`, `ip.arrange`), a new
`commands/box_reshape.rs`; new `crates/ew-viewmodel/src/tools/{ip,black_box}.rs`.
**Part 4:** `crates/ew-schema/src/{kind.rs,page.rs}` and a new `page_item.rs`, `crates/ew-schema/schemas` as its tests
generate them; `crates/ew-model` (the two invariants and delete policies); `crates/ew-derive/src/{structure.rs,wires.rs,title_block.rs}`
and a new `items.rs`; a new `crates/ew-scene/src/items.rs` and one call in `page.rs`; `crates/ew-commands/src/commands/{edit.rs,fields.rs,common.rs,device.rs}`
(`rep.move` `items`) and new `commands/{items.rs,structure_box.rs}`; `crates/ew-api` page tables and output;
`crates/ew-format` tests only; new `crates/ew-viewmodel/src/tools/{text,graphic,marker,structure_box}.rs`; the text
popup in `ew-ui`.
**Part 5:** the `image` variant in `ew-schema`, `item.place`, `ew-scene` image drawing, `tools/image.rs`, tests.

Shared append-only files (registry lists, `VOCABULARY`, transcript members, `params.rs`, fuzzer arms, DSL verbs) may be
touched by every part; when parts 3 and 4 run in parallel the second to integrate rebases onto the first. Read-only for
the lanes: every other crate, `golden/**`, `ci/**` (except as WP-S2.03's `testkit` entry exists), `deny.toml`,
`supply-chain/**`, `.github/**`, `docs/**` except the generated reference pages. Each lane commits on its own branch and
does not merge or push.

## 15. Out of scope

| Item | Where it goes |
|---|---|
| Layout tools (rails, placements, panel views) | WP-S4.02 (ruling, delegated) |
| The property grid, the tag field, provenance chips | WP-S2.05 (ruling, delegated) |
| Clipboard, paste and duplicate | WP-S2.05 |
| Device, potential and IP navigators, drag from the device navigator, potential highlighting (FR-UI-018's last part, FT-362) | WP-S2.06 |
| The strip and cable editors (creating -X1031's terminals interactively) | WP-S2.07 |
| Cross-page moves with the IP-pair replacement (FR-CON-043), moves onto a tab | WP-S1.07 and a follow-up |
| Undo coalescing (D11), move-time autoconnect commits, sticky wire continuation, junction moves and bend insertion, polygon black boxes, path function texts (FR-DEV-011), the per-page marker mode, detaching a function moved out of a black box, stored CP and contact-image variants while placing | Proposed follow-ups (§17.4) |
| The auto-route re-solve inside `rep.move` and `rep.transform` (FT-18, FT-43) | WP-S1.07 (the router of D7 is its implementation) |
| The spatial index and exact kurbo picking (FT-359, narrowed) | Follow-up with WP-S2.05 |
| Dimensions (FR-SYM-026) and page layers (FR-PAG-026) | 1.0 |
| Previews off the UI thread (FT-358) | Release phase (ID-512) |

## 16. Review checklist for the Opus reviewer

1. **Boundaries.** Only the items of §4.3 and the commands of §5 below the shell; no new edge or package in parts 1 to
   4; no `ew-model` name in `ew-viewmodel` or `ew-ui`; tools call no `Session` method, only the pure functions.
2. **One path.** Every model change is a registered command committed by the host (D2); the embed and strip steps are
   host commits; nothing writes a record otherwise; the headless commit goes through `run_script`.
3. **Previews.** Only `Session::preview_with`; autoconnect proposals reach the model only as hints of the committed
   command (D4); Escape and Alt leave the revision unchanged (NFR-UX-07).
4. **Exactness.** Grid indices everywhere (snaps, steps, router, box CPs); `TableCp.at` equals `cp_position`; no float
   equality in a decision; `i128` where products occur; the committed route equals the ghost.
5. **The transaction-read rule.** No new command reads `Q`; `ip.arrange` takes its order (D12); `marker.place` and
   `connection.draw_to_ip` derive nothing inside the transaction.
6. **Format (part 4).** The two kinds round-trip; unknown variants byte-equal; invariants and delete policies in the
   catalogue; no format-version bump and the compatibility test passes; Golden A byte-equal.
7. **Commands.** Each new command has its doc comment, schema, catalogue row and reference page, DSL verb, fuzzer arm and
   rejection tests; amendments are additive and `params_version` stays 1.
8. **Determinism and privacy.** The named tie-breaks; transcripts byte-identical on two runs; replay equality; no time,
   path or process ID in outputs.
9. **Accessibility.** Every tool operable from the keyboard alone (the kittest flows without pointer events); live-region
   announcements; shapes, not colour alone, carry ghost, snap and proposal meaning (NFR-A11Y-03).
10. **Scope.** Nothing of §15 half-done; the report names every `command.run` step the folio scripts still use.

## 17. Size, order, routing and the documents to update

### 17.1 Size

WP-S2.04 keeps **10.3 pw PERT** (O 7 · L 10 · P 15); the CSV is unchanged. **Part 1, 2.4 pw:** framework additions and
the four public items with tests 0.5; starter library, catalogue and picker 0.35; the Place tool with targets and
containment 0.4; `from_part`, `new_terminal` and the step move in the commands 0.35; Select drag, keyboard move,
transforms and delete 0.3; change symbol (command and tool) 0.25; transcript, describe, scripts and kittest 0.25.
**Part 2, 2.4 pw:** router 0.4; Wire tool and keyboard wiring 0.5; autoconnect and split 0.45; route edit 0.25; junction
editor with `junction_wires` 0.3; crossings with the command and the scene 0.3; tests and scripts 0.2. **Part 3,
1.9 pw:** IP tool 0.35; `connection.draw_to_ip` 0.25; continue 0.15; arrange (command and tool) 0.3; black box tool 0.3;
`box.reshape` and grips 0.35; tests 0.2. **Part 4, 2.5 pw:** the two record kinds with invariants and compatibility
0.45; derivation (containment, `ip_structure`, bound texts, marker modes, group text) 0.4; drawing and PDF 0.35;
commands and amendments 0.4; the four tools and the text popup 0.6; tests and the G-PP rehearsal 0.3. **Part 5,
0.5 pw.** The parts sum to 9.7 pw; the **0.6 pw** between them and the PERT is the reserve inside WP-S2.04, drawn on
for part 4 first. The reserve is thin because the PERT predates the gaps of §2.2 (two record kinds, eight commands,
the router); a part that closes more than 20 % over its size triggers a re-estimate at the next design pass.

### 17.2 Order and routing

Part 1 starts after WP-S2.03 part 2's delivery and review; part 2 after part 1; parts 3 and 4 after part 2, in parallel
or in either order; part 5 after part 4 and FT-337. Parts 1 to 3 and 5 go to a lead-programmer lane at D4 (`opus`,
`high`): each adds public items or commands below the shell and sets tool behaviour later work builds on. Part 4 goes to
a lead-programmer lane at **D5** (`opus`, `xhigh`): it changes the project format (two record kinds) and the derivation
(structure resolution), where a mistake is expensive and hard to detect (`CLAUDE.md` ladder). Each part has an Opus
review and is planned as one lane-cycle plus a fix round.

### 17.3 Proposed decision-log entries (for the design pass; IDs after ID-555)

1. WP-S2.04 specified in five parts with their sizes, order, routing and the thin reserve (D1, §17.1, §17.2).
2. The four delegated rulings: registered commands only (D2), `Session::preview` only (D3), autoconnect ghosts
   overlay-only until commit (D4), no layout tools and no property grid (D5).
3. The framework additions and the public items below the shell (D6, §4.3, §6), with the typed selection (D14).
4. Routing: the pure router, routes passed explicitly, `connection.draw` unchanged, split on insert on routed
   connections (D7).
5. The starter library and the two placement targets; strips as their own step (D8, D9).
6. The record kinds `page_item` and `structure_box`, their invariants, the structure-box containment and
   `ip_structure`, editing through `fields.set`; WP-S1.12's structure-box item closed by part 4 (D10).
7. The command catalogue 42 → 50 with five amendments; the grid-step move (§5, D13); D12's rule for derived inputs.
8. No undo coalescing in this package (D11); marker modes as a project setting with the as-built default (D16).
9. The headless mode and the 74 new describe words (28, 21, 10, 14, 1; D15; ID-474 extended).

### 17.4 Proposed follow-up tasks (for the design pass; FTs after FT-370)

1. Undo coalescing for nudges, rotate presses and junction reorders through a coalescing key that `run_script` carries
   too (doc 06 §3.9; D11).
2. Autoconnect proposals committed during a move (`C`, doc 06 §5.2) and the sticky wire continuing from its end point
   (doc 06 §4.4).
3. Junction moves and bend insertion or removal on a stored route (beyond segment drags).
4. The per-page wire-marker mode of FR-CON-024 (D16).
5. Path function texts (`FreeText` role `path_function`, FR-DEV-011).
6. Polygon outlines for black boxes, and the detach question when a joined function is moved out of a black box
   (doc 06 §2.7).
7. Stored CP and contact-image variants (`Representation.variant`) chosen while placing (FR-SYM-004).
8. **Golden data:** the MICO 4.10 part copy of Golden A has a one-function template (the black box with all ten CPs),
   while doc 06 §9.2 steps 7 and 8 and FR-UI-017's acceptance expect its 13-14 contact as a separate unplaced function;
   reconcile (a golden change needs the owner's acknowledgement; PL2 uses a fixture part meanwhile).

### 17.5 Open questions the lead could not close

1. **Folio 103 at G-PP.** Its strips -X1031 and -X1032 are strip segments over terminals that only the strip editor
   (WP-S2.07, M2) creates interactively; this package places terminals one by one (`new_terminal`) and strip segments of
   existing strips. The orchestrator decides with the pilot whether G-PP-01 accepts terminal symbols in place of the
   segments, or whether a minimal "add terminals" dialog joins M-PP.
2. **The grid of new pages.** The picker follows the page grid (D8); which grid `page.create` gives a page of a wizard
   project depends on PD-2 and ADR-0027, still with the owner.
3. **IP structure semantics.** D10's `ip_structure` changes the pairing scope `location` for IPs inside structure boxes;
   FR-PRJ-020's acceptance text names IPs, which have no device structure. The design pass confirms or narrows it.
4. **The default CP layout of a black box from a part pattern** (half on top, half on bottom) is a UX choice for R0
   (WP-S2.12) to validate.

### 17.6 Documents the design passes update

Named here because D7, D8, D10, D11, D13 and D16 change accepted text: doc 06 §2.4 (the fine grid as built), §2.7 (the
containment join as built), §3.1 (the command table and counts: 50, the new rows and amendments), §3.2 (normaliser 7 on
routed connections; the router is a pure function shared with the commands, D7), §3.5 (markers as built; IP name series
and direction defaults), §3.8 (page-item kinds as built), §3.9 (no coalescing for now, D11), §4.2 to §4.5 (the tool
tables as built), §5.2 (proposals on a move shown only), §5.4 (D7), §5.7 (grid-step moves, D13); doc 03 §5 and §6 (the
`StructureBox` and `PageItem` shapes as built, `WireMarker.at` and `angle`, the bindable names), §5.1 (the
`settings.extra.drawing` member and its command); doc 05 §2.2 (the two kinds implemented); doc 08 §4.5 (marker modes as
a project setting, the group text); doc 16 §4.4 and §4.5 (the keys and the tool strip as built), §4.8 (the keyboard
equivalents); doc 17 §2.1 and §2.3 (50 commands, the new rows); doc 18 §3.17 (what the drafter can draw and read back);
ADR-0009 (two invariants); architecture §9 (the canvas-tools row) and §5.2 (D12 as an example of the transaction-read
rule); plan 02 (WP-S2.04's status, the follow-ups, WP-S1.12's structure-box item), the traceability and the
glossary (page item, structure box, starter library).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.07, WP-S1.12, WP-S1.13, WP-S1.19a, WP-S2.03 to WP-S2.07, WP-S2.12,
  WP-S4.02, WP-S6.13; Appendix B (FT-18, FT-43, FT-314, FT-337, FT-358, FT-359, FT-362).
- [Decision log](../06-decision-log.md): ID-54, ID-385, ID-443, ID-474, ID-511, ID-512, ID-513, ID-525, ID-531 to
  ID-538, ID-545, ID-546.
- [ADR-0005](../../adr/0005-value-types-nm-dec3-mltext.md), [ADR-0007](../../adr/0007-explicit-connections-and-autoconnect.md),
  [ADR-0008](../../adr/0008-transactions-undo-preview.md), [ADR-0009](../../adr/0009-invariant-catalogue.md),
  [ADR-0027](../../adr/0027-native-symbol-grid-and-qet-grid-policy.md); [architecture](../../design/04-architecture.md)
  §3, §5, §9; [schematic editor](../../design/06-schematic-editor.md) §2 to §9; [UX/UI](../../design/16-ux-ui.md) §4,
  §10, §17; [domain model](../../design/03-domain-model.md) §5, §6; [connections](../../design/08-connections-terminals-cables.md)
  §4.5; [extensibility](../../design/17-extensibility.md) §2; [quality](../../design/18-quality-security-release.md)
  §3.17; [golden project](../../design/20-golden-project.md) §6, §8, §14.1, §15, §17; [gates](../03-gates.md) §5.4.
- Requirements: FR-UI-015 to FR-UI-019, FR-SYM-004, FR-SYM-009, FR-SYM-015, FR-SYM-022 to FR-SYM-026, FR-CON-001,
  FR-CON-003 to FR-CON-008, FR-CON-019, FR-CON-023, FR-CON-024, FR-CON-031, FR-CON-039, FR-CON-043, FR-XRF-013,
  FR-XRF-014, FR-DEV-003, FR-DEV-011, FR-DEV-013, FR-DEV-017, FR-DEV-038, FR-DEV-039, FR-PRJ-019, FR-PRJ-020,
  FR-PAG-025, FR-PAG-026, NFR-A11Y-01 to NFR-A11Y-03, NFR-UX-01, NFR-UX-07, NFR-REL-07, NFR-REL-08, NFR-PERF-20;
  AC-07, AC-28; G-PP-01.
