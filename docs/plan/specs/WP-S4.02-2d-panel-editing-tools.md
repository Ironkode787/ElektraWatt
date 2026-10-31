# WP-S4.02: 2D panel editing tools (closed specification)

**Status:** specified in cycle 41 (documents only), an M2 package, in six parts: part 1 the rail model, snapping and
the placing commands (1.1 pw, D3 to D5), part 2 assemblies, bulk placement and the set-up and edit commands (1.0 pw,
D6, D7), part 3 placing in the window (1.2 pw, D8 to D10), part 4 setting up and editing in the window (1.0 pw, D10
to D12), part 5 end brackets in the outputs (0.6 pw, D13), part 6 the starter library's layout parts (0.3 pw, D14);
part 1 first; part 2 after part 1; part 3 after part 1 and WP-S4.01 part 3; part 4 after parts 2 and 3; part 5 after
part 1 and WP-S4.01 parts 3 to 5; part 6 at any time. The parts sum to **5.2 pw plus 0.3 pw reserve** against a 3.1
pw PERT that covered the five requirement rows only (D15, O1) · **Date:** 2026-10-31 ·
**Work package:** [WP-S4.02](../02-work-packages.md#wp-s402-2d-panel-editing-tools) ·
**Decision log:** ID-? · **Part of:** ElektraWatt plan, specifications

**Purpose.** WP-S4.01 made the layout a model with outputs and checks and left every gesture to this package (its
D1). WP-S4.02 is where a person lays out a panel: drag a device from the unplaced list onto a rail, let it snap,
push or swap its neighbours, place a strip or a valve terminal as a whole, fill a rail in one step, draw the rails
and ducts, create the enclosure, and see the end brackets that the strips need derived, drawn and counted. This
document closes the package to the field for part 1 and to its items, decisions and acceptance for parts 2 to 6
(D16). It is written against the code at `93798f2f` (§2.1) and against WP-S4.01 part 3's specification (§7 of
[WP-S4.01](WP-S4.01-layout-model-and-2d-panel-pages.md)), which is being built beside it. Where this document and a
design document differ, this document is the narrower and wins for the lanes; the documents of §13 are updated at
the design pass after each part. A choice a lane meets that this document does not settle is escalated, never
guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: the rail model, snapping and the placing commands](#5-part-1-the-rail-model-snapping-and-the-placing-commands)
6. [Part 2: assemblies, bulk placement and the set-up and edit commands](#6-part-2-assemblies-bulk-placement-and-the-set-up-and-edit-commands)
7. [Part 3: placing in the window](#7-part-3-placing-in-the-window)
8. [Part 4: setting up and editing in the window](#8-part-4-setting-up-and-editing-in-the-window)
9. [Part 5: end brackets in the outputs](#9-part-5-end-brackets-in-the-outputs)
10. [Part 6: the starter library's layout parts](#10-part-6-the-starter-librarys-layout-parts)
11. [Files owned and what a lane may not decide](#11-files-owned-and-what-a-lane-may-not-decide)
12. [Risks](#12-risks)
13. [Size, order and routing](#13-size-order-and-routing)
14. [Follow-ups and the documents to update](#14-follow-ups-and-the-documents-to-update)
15. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package a person can lay out Golden A's panel from the window alone, at 1920 × 1080 and
at the 1280 × 800 floor: create the enclosure -UC1 from its part, open folio 902, draw the rails and ducts, drag F55
from the Unplaced devices navigator between -101F1 and -102F1 in push mode (the five devices to its right move 18 mm,
one undo step), insert -201U9 between -201U8 and -202U1 (-202U1 … -202U3 move 20 mm), auto-fill an empty rail in DT
order, edit a placement's position, rotation and lock in Properties, and delete a placement or an empty carrier with
Delete. Rail R2's four end brackets are derived from the strips' positions, drawn on 902, listed in the legend and
counted in BOM row 26 with folio 902; moving -X1032 50 mm away from -X1031 gives five. AC-33 then passes in full. A
new project can do the same with the starter library's generic enclosure, plate, rail, duct and end bracket.

**Non-goals** (each with its package or release):
- No balloons (FT-806, 1.0, doc 12 §5.2). No end covers, markers or bridges derived from part-data accessory lists
  (`AccessoryRef{per: Group}`, FR-PRT-005; 1.0): at the MVP the only derived accessory is the end bracket (D3).
- No strip segments (FR-LAY-063, 1.0), no piggy-back mounting, no door, gland, side or roof surfaces and no door
  items, no busbar systems, no frame punching lattice (doc 12 §17's 1.0 and 1.x columns).
- No 3D, gizmo or 3D rail-snap dragging (WP-S4.04, FR-UI-029). No collision, clearance keep-outs (EW-LAY-007) or
  routing (WP-S4.06, WP-S4.07). Clearance pads the snapping only (D3).
- No format change: no kind, member or variant is typed or added; every new value (brackets, carrier labels, free
  length, snap results) is derived. No carrier designation member (FT-808, 1.0).
- No new navigator beyond Unplaced devices; the layout tree navigator (space → enclosure → surfaces → carriers →
  placements) is 1.0 (doc 16 §5.2's last row).
- No contextual command-bar tab (ADR-0031 decision 3: "No contextual tabs"); D10 places the commands.
- No performance measurement (owner's rule, ID-512): every timing claim is "not measured (owner's rule)".

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [Panel layout](../../design/12-panel-layout.md) §2.3, §2.6 (as built), §3.2 to §3.6, §5.2, §12.1, §12.3, §12.4, §16.1, §17 | Pose constraints, the derived queries, rails and ducts, placing and snapping, strips and group accessories, part and schematic changes, carrier deletion, the panel layers, the 2D tools, the panels, keyboard placement, the golden expectations, the MVP column |
| [UX and UI](../../design/16-ux-ui.md) §4.5, §4.8, §5.1, §5.2 (the navigator catalogue), §6, §10.1 | Tool host, drag and drop with its keyboard twin, shared navigator behaviour, the Unplaced devices row, the property grid, AccessKit |
| [Design language](../../design/16a-design-language.md) §5.2 to §5.5, §6.6, §6.8, §6.12, §6.17, §6.20, §7.3, §7.4, §8 | Command bar, tabs, rail, HUD, tree rows, property rows, dialogs, empty states, segmented control, ghosts, canvas chips, wording; every UI part cites it (CLAUDE.md rule 5) |
| [Golden project](../../design/20-golden-project.md) §9 (-X1032's accessories), §13 (test geometry), §14.1 (rows 22 to 27), §21 (AC-33) | R2's brackets, the placements, row 26, the acceptance |
| [WP-S4.01](WP-S4.01-layout-model-and-2d-panel-pages.md) D3 to D13, §7 (part 3), §13 (FP-1, FP-6) | The records, commands, queries and outputs this package builds on |
| [WP-S2.03](WP-S2.03-viewer-and-tool-framework.md), [WP-S2.04](WP-S2.04-placement-wiring-drawing-tools.md), [WP-S2.05](WP-S2.05-property-grid-find-replace-clipboard.md), [WP-S2.06](WP-S2.06-navigators-and-table-mode.md) D4 | The tool contract, Select's drag and keyboard move, the property sheet and its edit routes, the navigator framework and FT-486 |
| ADR-0008, ADR-0031 | One gesture, one transaction; the task-ordered command bar without contextual tabs |
| Follow-ups | FT-204 (row 26), FT-486, FT-804, FT-805, FT-806, FT-837, FT-839, FT-840; FT-838 as context only (its owner is WP-S4.01 part 2) |

### 2.1 What exists at `93798f2f` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| The eleven `layout.*` commands of WP-S4.01 D6; `layout.place {device, part?, function?, carrier, pose}` and `layout.move {placement, carrier?, pose}` with explicit poses and no `mode`; `layout.carrier.add` without `copies`; `follow_part_change` (D7) | `crates/ew-commands/src/commands/layout.rs` | Part 1 adds `mode`; part 2 adds commands and members |
| `check_path`, `path_length`, `RailAxis`, `Profile`, `fits_rail`, `fits_surface`, `Quarter`, `check_pose`, `clip_offset`, `rehome_pose`, `plate_frame` | `crates/ew-derive/src/layout/{carrier,fit,pose}.rs` | Shared by the resolver and the tools |
| The queries `Mountable`, `Unplaced`, `PlacementGeometry` (`PlacementGeom {surface, footprint, turn, unknown, outline, part}`), `RailOccupancy` (`Vec<Occupied {s0, s1, placement, device, locked, unknown}>`, sorted, no padding, no group accessories), `LayoutLabel`, `CarrierGeometry`; none stored | `crates/ew-derive/src/layout/queries.rs` | Part 1 extends `Occupied` and adds two queries |
| `Session::layout_spaces`, `Session::unplaced(space)` | `crates/ew-api/src/lib.rs` | The navigator's rows |
| `PartKind::EndBracket`; `MechData.clearance`; `PartSections.{enclosure, rail, duct, mounting_plate}`; `DuctSection` has inner dimensions only (the nominal W × H are carrier `params`) | `crates/ew-schema/src/library.rs` | Bracket declaration, padding, set-up |
| Golden A: strips 1X1 (UK 5 N, UK 5 N, USLKG 5: 18.6 mm), -X1031 (20 × PT 2.5-TWIN: 104 mm), -X1032 (5 × PT 2.5-TWIN-PE: 26 mm) on R2 at s = 30, 58.6, 172.6 mm; each strip carries a "TBD end bracket" part use (1X1 × 2, -X1031 × 1, -X1032 × 1); the TBD end bracket part (kind `end_bracket`) has `mech.width` 10 mm; BOM row 26 prints quantity 4 with folio "101, 103" (ID-317) | `golden/J-E-2416/tables/{devices,terminals,parts,placements}.csv`, `golden/J-E-2416.ew/` | Parts 1 and 5 |
| The tool contract (`Tool`, `ToolCx`, `CanvasEvent` in sheet nm, `Overlay`, `Ghost`, `Mods {shift, ctrl, alt}`), `CanvasPage {…, tables: Arc<PageTables>}` fetched once per (session, revision, folio); Select's drag rows and keyboard move (`move_keys`) | `crates/ew-viewmodel/src/tools/`, `src/canvas.rs` | Parts 3 and 4 |
| `NavKind` (14 kinds, no Unplaced), `DragSource {Page, Function, Channel, Macro}`; the rail order of doc 16a §5.4 | `crates/ew-api/src/output/navigators.rs`, `crates/ew-viewmodel/src/shell/navigators.rs` | Part 3 |
| The canvas action `tool.next_proposal` bound to Tab; a chord unique per shell context (ID-628); the shell's mapping of a chord to another action in one focus (the `strip.move` precedent of doc 16 §4.4) | `crates/ew-viewmodel/src/actions.rs` | Part 3's Tab |
| `page.create` accepts `panel_layout`; `device.create` (a device without a representation, with part uses) and the `copies` member of `device.place` that embeds library documents in the same transaction (`embed_closure`, FT-656) | `crates/ew-commands/src/commands/{page,device,assign}.rs` | Part 2's enclosure command and `copies` |
| The property sheet over `field_catalogue`, derived rows with their edit routes (`EditRoute`, `route_of`) | `crates/ew-api/src/output/{properties,fields}.rs` | Part 4 |
| `ew-starter-parts`: 18 generic parts, none with `mech` or a layout section, generated by `crates/ew-library/tests/starter_parts.rs` (`defs()`, `EW_BLESS=1`) into `content/libraries/ew-starter-parts.ewlib/` and compiled in by `crates/ew-library/src/starter_parts.rs` | as named | Part 6 |
| WP-S4.01 part 3 (being built): the `surface_view` item, `page.surface_view.set {page, surface, datum_at, label_mode?}`, the panel layers, hit-testing that returns placements and carriers (each item's `EntityRef` is the record's UUID), the projection `x = datum.x + u·num/den`, `y = datum.y − v·num/den` | WP-S4.01 §7 | Parts 3 to 5 read it as specified; a lane that finds part 3 built otherwise stops and reports |

### 2.2 Gaps found (why the parts are shaped as they are)

| Gap at `93798f2f` | Consequence |
|---|---|
| `rail_occupancy` has no padding and no brackets; no function decides where a dropped device lands | Part 1: the rail model (D3) and one resolver shared by the tools and the commands (D4, D5) |
| No assembly placement, no bulk placement, no ordering of an assembly's members outside a rule not yet built (EW-LAY-016, WP-S4.01 part 5) | Part 2 (D6), with one shared member order |
| No window command creates an enclosure from a part; `layout.carrier.add` cannot embed a library part; no edit command for a space's name, an enclosure's pose or a plate's PE bond (FT-840); `edit.delete` refuses layout kinds (FT-837) | Part 2 (D7) |
| No panel data reaches a tool (a tool never calls the `Session`); no Unplaced devices navigator (FT-486); no layout tool | Part 3 (D8, D9) |
| No draw-rail or draw-duct tool, no enclosure gesture, no surface view from the window, layout members read-only in Properties (FT-804, FT-839) | Part 4 (D10 to D12) |
| The end brackets are authored part uses; row 26 prints the schematic folios (FT-204's rest); nothing draws them on 902 | Part 5 (D13), a golden change |
| A new project has no layout part and no dimensions (FT-805) | Part 6 (D14) |
| Doc 02's "Scope in" lacks FT-804's items (create a space and an enclosure, draw a rail, draw a duct, the surface view, the Properties rows); no other work package or closed specification owns any of them (the `page.surface_view.set` command is WP-S4.01 part 3's, its window row is this package's) | The ruling: they join this package; doc 02 is amended (D1) |

## 3. Requirements traced

The five rows of the package, and the rows it completes for WP-S4.01.

| Requirement | What this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-LAY-014 | Rail snapping on the 1-D model: free, push, swap; clearance padding; brackets at strip boundaries; snap targets (device edges, rail ends, plate grid 1 and 5 mm, column alignment with the rail above); mount-kind refusal shown on the ghost | 1, 3 | SN-1 to SN-6, PW-3, PW-4 |
| FR-LAY-015 | A strip placed as a whole in strip order (built, WP-S4.01 D5) with its end brackets (D3); the computed length is re-derived on every strip change and its overflow, brackets included, reported (part 5's EW-LAY-010 case). End plates, markers and bridges from accessory lists are 1.0 | 1, 5 | SN-1, EB-4 |
| FR-LAY-016 | `layout.auto_fill` in DT or schematic order, greedy with the clearance gaps, listing what did not fit; the Auto-fill rail dialog | 2, 4 | AS-3, AS-4, PS-6 |
| FR-LAY-060 | End brackets derived from the rail occupancy, one per strip boundary in a run and one per run end, counted in the BOM with the strips' DTs, drawn on 902 | 1, 5 | SN-1, EB-1 to EB-3 |
| FR-LAY-061 | `layout.place_assembly`; inserting a missing member uses push; the member order shared with EW-LAY-016 | 2, 3 | AS-1, AS-2, PW-5 |
| FR-LAY-013 (the drag half; WP-S4.01 built the list) | The Unplaced devices navigator with drag-to-place and its keyboard twin (FT-486) | 3 | PW-1, PW-2 |
| FR-LAY-004, FR-LAY-005 (the drawing gestures) | Draw rail, draw duct (FT-804) | 4 | PS-2, PS-3 |
| FR-LAY-002 (the window gesture) | Create the enclosure and its plate from a part (FT-804) | 2, 4 | AS-5, PS-1 |
| FR-LAY-010 (the window gesture) | The surface view from Properties (FT-804) | 4 | PS-4 |
| FR-REP-007 (the rest) | End brackets in the BOM by derived count, folio 902 (FT-204's row 26) | 5 | EB-1 |
| NFR-A11Y-01, NFR-A11Y-02 (the layout part) | Keyboard placement in carrier order and the live-region announcement (doc 12 §12.4) | 3 | PW-6 |
| AC-33 (shared with WP-S4.01) | R2's four brackets; AC-33 complete | 5 | EB-5 |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under the decision row of this
specification (and the as-built entries after each part).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Scope.** The five rows; FT-804 (FP-1: create the space and the enclosure from a part, draw a rail, draw a duct, set the surface view, the placement's Properties rows), FT-486 (the Unplaced devices navigator), FT-805 (the starter library's layout parts and dimensions), FT-837, FT-839, FT-840 and FT-204's row 26 (FP-6). Doc 02's "Scope in" is amended to say so. Out: FT-806 (balloons, 1.0) and §1's non-goals | They are the 2D editing tools; no other package or closed specification owns any of them (§2.2). FT-805 is small data, and without it a new project cannot lay out a panel at all |
| D2 | **Where the code lives.** The rail model and the resolver are pure functions in `ew-derive` (`src/layout/snap.rs`, `src/layout/accessories.rs`) with two memo queries; the commands in `ew-commands` (`commands/layout.rs`, `commands/edit.rs` for FT-837); the panel data for tools in `ew-api` (`Session::panel_tables`); the tools, the navigator and the dialogs in `ew-viewmodel` and `ew-ui`. `ew-panel` stays empty; no edge of `ci/dep-rules.toml` changes; no new external crate | WP-S4.01 D2's reasoning holds: `ew-checks`, `ew-reports` and `ew-scene` read the bracket intervals and may not depend on `ew-panel`; the window reads the same functions the commands run |
| D3 | **The rail model.** A rail holds *items* along `s ∈ [0, L]`: each placement's footprint interval `[s0, s1]` from `rail_occupancy`. **Clearance** pads a non-strip item for snapping only: `pad_before` = the part's `clearance.left`, `pad_after` = `clearance.right` (swapped when the placement is flipped); strips have no pads; EW-LAY-006 keeps comparing bare footprints. **End brackets:** a strip *declares* its bracket part by its first part use whose part has kind `end_bracket`; the bracket's width `w` is that part's `mech.width` (absent: 0). **Minimum gap** between consecutive items a (left) and b (right): strip–strip `w(a)`; strip–other `w(a) + pad_before(b)`; other–strip `pad_after(a) + w(b)`; other–other `pad_after(a) + pad_before(b)`; at the rail start before a strip `w(strip)`, at the rail end after a strip `w(strip)`, else 0. **Runs:** consecutive strips are in one run when their gap is 0 or less than `w(a) + w(b)`; otherwise each starts its own. **Brackets** (k + 1 per run of k strips): one at the run start `[s0 − w, s0]` attributed to the first strip, and one after each strip `[s1, s1 + w]` attributed to it; a strip that declares no bracket part contributes none | FR-LAY-060's "abutting" must allow the bracket between two strips, and its acceptance ("moving -X1032 50 mm away yields 5") needs a threshold: a gap with room for two brackets separates the runs. Attribution to the left strip reproduces Golden A's authored quantities exactly (1X1 × 2, -X1031 × 1, -X1032 × 1), so row 26 keeps its quantity and only its folio changes. Declaring the part on the strip uses data that exists; the 1.0 accessory lists replace the declaration without changing the count |
| D4 | **Two halves of snapping.** *Target snapping* is the tool's (it depends on the zoom): the dragged item's left edge snaps, within 15 px on screen, to the positions where it would touch a neighbour at the minimum gap, to the rail ends (at their minimum gap), to the edges of the occupants of the nearest parallel rail above whose range overlaps (column alignment; for a vertical rail the nearest one to its left), and to the plate grid (5 mm, or 1 mm by the tool option) along the axis in surface coordinates; at equal distance edges win over rail ends, rail ends over columns, columns over the grid; Shift suspends target snapping (the cursor's s rounded to 0.1 mm). *Mode resolution* is a pure function `resolve(items, L, moved, s_left, mode)` in `ew_derive::layout`, zoom-independent, run by the tool for its ghost and by the command for the commit, so the ghost and the commit agree by construction. `layout.place` and `layout.move` gain `mode?: free \| push \| swap`; without `mode` they take the explicit pose exactly as built (scripts, imports and the test DSL unchanged) | A command cannot know the zoom, and a ghost computed from different code than the commit would lie; one function over one occupancy at one revision gives the same answer twice |
| D5 | **The modes** (integer nm; the moved item first removed from the rail; `s_left` its footprint's left edge; poses written as clip points, `s_clip = s_left + clip offset along the axis`). **Free:** if the item fits at `s_left` with every minimum gap held, it stays; else it goes to the position in the gap closest to `s_left` where it fits, ties to the smaller s; no gap fits: refused ("No gap on rail 1 is wide enough for -F55 (18 mm)."). **Push:** the occupants whose centre is left of the item's centre stay (an occupant centred exactly on it counts as right); the item starts at `max(s_left, the left neighbour's end + minimum gap)`; each occupant to the right moves right only as far as its minimum gap to its new left neighbour requires, cascading; refused when a locked placement would move ("Pushing would move -102F2, which is locked.") or the last moved item, with its rail-end gap, passes L ("Pushing would move S51 past the rail end by 20 mm."). **Swap:** only between two placements on the same rail ("Swap works within one rail; use push or free to place it there."); the target is the occupant under the item's centre ("Drop onto a placement to swap with it."); the segment from the first to the last of the two is re-laid with the two exchanged, every other item in its order, each gap kept where it still meets the new minimum gap, else the minimum gap; when the segment grows, the items after it cascade as in push; refused when a locked placement is in the segment or would move. The neighbours' new poses are written in the same transaction (one undo step) | Push to the right only and the centre split make a drop between two devices insert between them, never on top of the left one; keeping gaps in swap moves nothing outside the two when widths are equal, the common case. Locked placements never move, as doc 12 §2.3 says |
| D6 | **Assemblies and bulk.** The **member order** of an assembly or PLC station (the parent first if mountable, then `Device.position`, then the module slot, then natural tag order) is one function `ew_derive::layout::member_order`, shared with EW-LAY-016 (if WP-S4.01 part 5 has built its own in `ew-checks`, part 2 moves it to `ew-derive` and the rule calls it). `layout.place_assembly {device, carrier, s, mode?}` places the unplaced mountable members of `device`'s assembly: when none is placed on that rail, as one block of the members at their minimum gaps whose left edge is `s` (resolved as one item); when some are, each unplaced member is inserted after its predecessor in member order in push mode (`s` ignored). `layout.auto_fill {carrier, devices?, order: dt \| schematic}` places `devices` (default: the unplaced devices of the rail's space that fit its profile), in natural DT order or in schematic order (page order, row, column of the main representation; none: last, by DT), an assembly's members as one block at its first member's place: next-fit from the rail start, each item at the first position at or after the previous item's end where it fits with its minimum gaps; an item that fits nowhere is left unplaced and listed in the outcome's notice with its reason ("E55: screw-mounted, not on a rail"; "-201U9: no room left on rail 3"), and the fill continues. Bulk above 200 placements, previewed | One order for the tool and the rule means the layout and the check cannot disagree (D-10). Next-fit keeps the chosen order on the rail, which is what the user asked for; listing what did not fit is doc 12 §3.3's behaviour |
| D7 | **The set-up and edit commands** (each one undo step, Interactive unless stated): `layout.enclosure.create_from_part {location, part, copies?, pose?, pe_bond?}` creates, in one transaction, the location's layout space when it has none (name empty), the enclosure device (letter U, the next counter as `device.create` gives it, the location, the focused page's function when the window passes one), its part use, a child device carrying the enclosure section's plate part when the section names one, and the enclosure and plate surface exactly as `layout.enclosure.create` does; `copies` embeds library documents first (FT-656's pattern); refusals: a part without an enclosure section, a location that already has an enclosure (D3 of WP-S4.01). `layout.carrier.add` gains `copies`. FT-840: `layout.space.edit {space, name}`, `layout.enclosure.edit {enclosure, pose}`, `layout.surface.edit {surface, pe_bond}`. FT-837: `edit.delete` accepts the five layout kinds with WP-S4.01 D4's policies (a placement is removed; an empty carrier is deleted; an occupied carrier is refused: "Rail 2 holds 3 placements; use Delete carrier… to remove or rehome them."; a space and an enclosure cascade to their surfaces and carriers and are refused while any carrier is occupied or the enclosure's device would go). `layout.move` gains `part?` (the realised part use, checked as `layout.place` checks it; EW-LAY-003's quick fix "realise the primary part") | One gesture, one transaction (ADR-0008): creating an enclosure from the window is one gesture, so it is one command; the three small edit commands close FT-840 without a generic layout setter, which would bypass the per-member checks |
| D8 | **Carrier labels.** A carrier is named in every text by a derived label "Rail n" or "Duct n": rails then ducts, each numbered from 1 top to bottom, then left to right by `path[0]` (WP-S4.01 D11's legend order; one function `carrier_labels(surface)` in `ew-derive`, which part 4 of WP-S4.01 reuses or which part 1 builds first). Never stored; a new carrier may renumber the others, like legend positions | Carriers have no name member (FT-808, 1.0); Golden A's carriers come out as Rail 1 … 3 and Duct 1 … 5, which match R1 … R3 and D1 … D5 of doc 20 §13 |
| D9 | **Panel data for tools.** `Session::panel_tables(page)` returns, for a panel page with a surface view, `PanelTables {page, surface, scale (num, den), datum, usable, carriers: [{id, kind, label, path, axis, length, profile, part, width, height}], placements: [{id, device, label, carrier, pose, footprint, locked, unknown, strip, bracket_width, pads, mount_kind}], brackets, occupancy per rail, free length per rail, unplaced: [{device, label, part number, W × H, mount kind, strip, assembly}]}`, all in surface nm; `None` for any other page. `CanvasPage` gains `panel: Option<Arc<PanelTables>>`, fetched once per (session, revision, folio) like `tables`. Sheet ↔ surface is the inverse projection, `u = (x − datum.x)·den/num`, `v = (datum.y − y)·den/num`, in i128 with one rounding to the nearest nm, halves away from zero | The tool contract forbids a tool to call the `Session`; this is `page_tables`' pattern for layout data |
| D10 | **The command surface** (doc 16a §5.3's design-language review, done here). The **Draw** tab gains a **Layout** group after Connect with three small buttons, Rail, Duct and Auto-fill rail…, enabled on a panel layout page with a surface view and disabled elsewhere with the reason "Open a panel layout page" (doc 16a §5.2: nothing hidden or reflowed). **Enclosure…** joins the Project tab's Project group (set-up, done once). **Unplaced devices** joins the navigator rail after Report blocks (Ctrl+1 … Ctrl+9 keep their places). Draw's existing Rotate (R) rotates a surface placement 90° and Mirror (X) flips a rail placement on a panel page; Delete deletes through `edit.delete`; Lock and Unlock are in the context menu and Properties, with no key | ADR-0031 forbids contextual tabs, and Draw must hold the core loop; three small buttons in one group cost about one button column, and reusing R and X keeps one key per meaning (doc 12 §12.1). The trade-off is three disabled buttons on schematic pages |
| D11 | **The drawing tools.** *Draw rail:* two clicks on the surface; the points snap to the plate grid (5 mm, or 1 mm by option), to the duct outlines and to the ends and axes of other rails; the second point is constrained to the dominant axis of the cursor's displacement; the part is the tool option's rail part (the project's parts of kind `rail`, else the installed libraries' ones, embedded through `copies`), the profile from its rail section; refused without a rail section ("The rail part has no profile; choose a rail part with a rail section."). *Draw duct:* clicks add vertices of an orthogonal centreline (each segment on its dominant axis); Backspace removes the last vertex; a double click or Enter finishes (`Tool::finishes_on_double_click`); the points snap as for rails and also to the usable edges inset by half the duct width and to other ducts' outlines (T joints); width × height from the part's `mech` W × H, else the tool option fields (default 40 × 60 mm, then the last used). *Moving a carrier:* Select's drag of a carrier translates its path on the grid (`layout.carrier.edit`), its placements with it | Doc 12 §3.2 and §12.1 as stated; snapping a duct flush with the plate edge and under another duct gives Golden A's D1 at v = 880 mm and D2 from (20, 860) mm in two gestures each |
| D12 | **Properties for layout records** (FT-839, doc 12 §12.1). A placement: Device (link), Realised part (choice of the device's mountable part uses; `layout.move {part}`), Carrier (label; read-only), Position s (mm) on a rail or u, v (mm) on a surface (`layout.move`, mode free), Rotation (0°, 90°, 180°, 270° on a surface; Normal or Flipped on a rail; `layout.move`), Locked (`layout.lock`); derived rows with provenance: Label, Footprint (W × H mm), Clearance, Findings. A rail: Part, Profile (read-only), Start (u, v), Length (moves the end along the axis), Free length (derived: L − Σ items − Σ brackets; R1 341 mm, R2 381.4 mm), PE bond; a duct: Part, Width, Height, Length (derived), PE bond (`layout.carrier.edit`). A surface: PE bond (`layout.surface.edit`); an enclosure: Position (`layout.enclosure.edit`); a space: Name (`layout.space.edit`). A **panel layout page** gains Surface view rows: Surface (choice of the project's surfaces by "<enclosure tag> plate"), Label mode (Abbreviated, Full) and Datum (x, y mm), committed by `page.surface_view.set`; choosing a surface on a page without a view computes the datum that centres the enclosure's paper rectangle in the frame's drawing area | Each row writes through the command that checks it; a generic `fields.set` on layout records would skip the pose and fit checks |
| D13 | **End brackets in the outputs** (part 5). For a placed strip, the BOM's line of its declaring end-bracket part use takes its quantity from the brackets attributed to it (`LineSource::GroupAccessory`), not from the authored quantity, and prints the folios of the panel pages whose surface view shows its rail's surface; an unplaced strip keeps its authored quantity and folios (doc 12 §3.4). The `placements` layer draws each bracket as a rectangle of its width by the run's largest terminal height, centred on the rail axis; the legend gains one row per bracket part on the surface after the placements (doc 12 §5.4's "layout-only accessories"), DT cell empty, quantity its count; EW-LAY-010 gains two cases: a bracket that overlaps a placement or another bracket, and a bracket outside `[0, L]` | Golden A keeps row 26's quantity (D3's attribution) and gains folio 902 as doc 20 §14.1 prints it; drawing the brackets makes 902 show what the shop mounts |
| D14 | **The starter library's layout parts and dimensions** (part 6) are the values of §10, original test values of the project (licence of the library), never vendor data | A new project needs one part of each layout kind to lay out a panel at all; dimensions on the rail-mounted generic parts make their footprints known |
| D15 | **Size and reserve.** The parts sum to **5.2 pw**; **0.3 pw** is held for part 5's golden change. The 3.1 pw PERT covered the five rows; D1's added scope (FT-804, FT-486, FT-805, FT-837, FT-839, FT-840) is about 2.1 pw. O1 asks for the re-estimate | The added items are the ruling's; the alternative, leaving the set-up gestures out, would ship a layout that only scripts can start |
| D16 | **Part 1 closed to the field; parts 2 to 6 to their items, decisions and acceptance**; each part is re-read against the delivered earlier parts (and WP-S4.01 parts 3 to 5) at dispatch without reopening D1 to D15 | Part 1 can dispatch now on built code; the window parts depend on WP-S4.01 part 3, which is being built |

No decision changes an ADR. D10 is the design-language review that doc 16a §5.3 asks for before a new group on Draw.

## 5. Part 1: the rail model, snapping and the placing commands

**Scope.** D3, D4's resolver, D5, D8, FR-LAY-014's model half, FR-LAY-060's derivation. **1.1 pw.**

**Build list.**
1. `crates/ew-derive/src/layout/queries.rs`: `Occupied` gains `pad: [i128; 2]` (before, after along s; D3; flipped
   placements swap them; strips `[0, 0]`), filled from the realised part's `clearance`; ordering and every existing
   field unchanged.
2. `crates/ew-derive/src/layout/accessories.rs`: the memo query `group_accessories(carrier)` →
   `Vec<Bracket {s0, s1, part: PartRef, strip: Id<Device>, placement: Id<Placement>}>`, sorted by `s0`, from
   `rail_occupancy` and each strip's declaring part use (D3); empty for a duct, a rail without a valid axis or a rail
   without strips. Added to `verify-derived`'s sweep.
3. `crates/ew-derive/src/layout/snap.rs`: `RailItem {s0, s1, strip, w, pad, locked, placement}`, `min_gap(a, b)`,
   `runs(items)`, `brackets(items)` (the same rule as item 2, over items, so the tool can evaluate a candidate
   arrangement), `resolve(items, length, moved: RailItem, s_left, mode) -> Result<Resolution {s_left, moved:
   Vec<(Id<Placement>, i128)>}, SnapError>` with D5's three modes, and `free_length(items, brackets)`.
   `SnapError` carries what the texts need; no panics on any input (overlapping or out-of-range loaded items are
   accepted as given; only the items that D5 moves are re-checked).
4. `carrier_labels(surface)` (cold) in `crates/ew-derive/src/layout/` (D8), unless WP-S4.01 part 4 has built the
   legend's carrier order, which it then reuses.
5. `crates/ew-commands/src/commands/layout.rs`: `layout.place` and `layout.move` gain `mode?` (D4); with `mode` on a
   rail, the pose's `t.x` is read as the intended clip point, converted to `s_left`, resolved, and the placement and
   every moved neighbour written in the one transaction; with `mode` on a surface only `free` is accepted and means
   the explicit pose ("Push and swap work on a rail."); the refusal texts of D5; existing refusals unchanged.
6. The command fuzzer's layout arms gain `mode` (append-only).

**Acceptance.**
- **SN-1 (FR-LAY-060)** `group_accessories(R2)` on Golden A is exactly [20, 30], [48.6, 58.6], [162.6, 172.6],
  [198.6, 208.6] mm, attributed 1X1, 1X1, -X1031, -X1032; with -X1032 moved to s = 222.6 mm (gap 60 mm ≥ 20 mm) it
  gives five, the new ones [212.6, 222.6] and [248.6, 258.6] attributed -X1032; a strip without an end-bracket part
  use gives none; a bracket part without a width gives zero-width brackets and joins only touching strips.
- **SN-2 (free)** On Golden A's R1 (items [61, 131], [131, 149], [149, 167], [167, 212], [212, 245], [245, 290]),
  `layout.place` of F55 (18 mm) with `mode: free` and clip s = 158 mm lands at clip 52 mm (the left gap's end is
  closer than the right gap's start); with clip s = 320 mm it stays at 320 mm; a 300 mm test part when both gaps are
  narrower is refused with D5's text naming "rail 1" and the width.
- **SN-3 (push)** F55 with `mode: push` and clip s = 158 mm (left edge 149 mm) lands at 158 mm and moves -102F1,
  -102F2, T55 and S51 to 176, 207.5, 246.5 and 285.5 mm in the same step; one undo restores all five; with -102F2
  locked it is refused naming -102F2; a 300 mm test part pushed at the same place is refused "past the rail end by
  20 mm".
- **SN-4 (swap)** `layout.move` of -101F1 onto -102F2 with `mode: swap` gives -102F2 153.5, -102F1 185, -101F1
  203 mm and moves nothing else; -201U3 and -201U4 on R3 swap to 148 and 108 mm; a swap onto another rail and onto
  an empty spot are refused with D5's texts.
- **SN-5 (clearance)** A fixture part with `clearance.left = clearance.right = 5 mm` placed with `mode: free` next to
  another keeps a 10 mm gap; flipped, its pads swap; EW-LAY-006 (when built) still compares bare footprints.
- **SN-6 (strips)** On R2, a non-strip 18 mm test part dropped in free mode at left edge 200 mm lands at 208.6 mm
  (after -X1032's run-end bracket); a 26 mm test strip with a 10 mm bracket part dropped at left edge 205 mm lands
  at 208.6 mm, joins the run and `group_accessories(R2)` gives five brackets.
- **SN-7** `layout.place` and `layout.move` without `mode` behave exactly as LM-5 states; the resolver's result for
  a ghost equals the committed poses (a property test over random rails, items and modes); `carrier_labels` names
  Golden A's carriers Rail 1 … 3 and Duct 1 … 5 as doc 20 §13's R1 … R3 and D1 … D5; Golden A and AC-15 path (b):
  every exported byte unchanged; twice-run determinism.
- **SN-8** `cargo nextest run -p ew-derive -p ew-commands -p ew-api`, the matching clippy, `cargo fmt --all --check`,
  `cargo xtask check-deps`.

## 6. Part 2: assemblies, bulk placement and the set-up and edit commands

**Scope.** D6, D7. **1.0 pw.**

- `member_order` (D6) in `ew_derive::layout`; `layout.place_assembly` and `layout.auto_fill` (D6) in
  `commands/layout.rs`; `layout.enclosure.create_from_part`, `layout.carrier.add`'s `copies`, `layout.space.edit`,
  `layout.enclosure.edit`, `layout.surface.edit`, `layout.move`'s `part` (D7); `edit.delete` on the layout kinds
  (`KindName` extended; FT-837); the fuzzer's arms (append-only).

**Acceptance.** **AS-1 (FR-LAY-061)** On Golden A with -201U9 unplaced, `layout.place {device: -201U9, carrier: R3,
mode: push}` at clip 338 mm and `layout.place_assembly {device: -201U1, carrier: R3}` each place -201U9 at 338 mm
and move -202U1, -202U2, -202U3 to 373, 418 and 448 mm in one step (Golden A-R's insertion). **AS-2** With R3
emptied (fixture), `layout.place_assembly {device: -201U1, carrier: R3, s: 20 mm}` places the twelve members abutting
from 20 mm in member order, -201U9 between -201U8 and -202U1; `member_order` gives the order EW-LAY-016 uses. **AS-3
(FR-LAY-016)** With R1 emptied (fixture), `layout.auto_fill {carrier: R1, devices: [-102G1, -102F2, -101F1, -102F1],
order: dt}` places -101F1, -102F1, -102F2, -102G1 at clip 9, 27, 58.5 and 116 mm; with the clearance fixture of SN-5 on
-102F1 the gaps grow by its pads; a screw-mounted device in the list is not placed and named in the notice. **AS-4**
`order: schematic` sorts by (page order, row, column) of the main representation, devices without one last by DT;
an assembly's members stay one block; more than 200 placements make it Bulk with a preview. **AS-5** On a project
with a location and no layout, `layout.enclosure.create_from_part` with a library enclosure part (embedded through
`copies`) whose section is 750 × 1000 × 420 mm with a 650 × 900 mm plate part gives one space, one enclosure device
with its part use, one child device with the plate part, one enclosure and one plate surface with frame (50, 50, 0)
mm, in one undo step; a part without an enclosure section is refused. **AS-6 (FT-840)** The three edit commands
change their member and nothing else; a pose for an enclosure outside its degrees of freedom is refused. **AS-7
(FT-837)** `edit.delete` of a placement removes it; of an empty carrier deletes it; of an occupied carrier is refused
with D7's text; of an enclosure with an occupied rail is refused; the fuzzer finds no sequence that leaves a
placement without a device or carrier. **AS-8** Golden A unchanged; targeted checks for `ew-derive`,
`ew-commands`, `ew-api`; `cargo xtask check-deps`.

## 7. Part 3: placing in the window

**Scope.** D4's target snapping, D9, D10's navigator and keys, FT-486. **1.2 pw.** UI briefs cite
[doc 16a](../../design/16a-design-language.md).

- `Session::panel_tables` and `CanvasPage.panel` (D9).
- **The Unplaced devices navigator** (`NavKind::Unplaced`, doc 16 §5.2's row): tree location → device; table columns
  Part, W × H (mm), Mount; badges `strip`, `assembly`, `unknown size`; drag source `DragSource::Device`; Enter and
  the context command "Place on carrier…" arm the layout place tool with the device floating at the grid cursor of
  the focused panel page (doc 16 §4.8's keyboard twin); empty states (doc 16a §6.17): no layout space, "No panel
  layout yet" with the action Enclosure…; all placed, "Every device of +J-E-2416 is placed."; on the rail after
  Report blocks (D10).
- **The layout place behaviour** on a panel page: while a device floats or a placement is dragged, the target is the
  nearest rail within 15 px whose axis the pointer projects onto, else the surface; a rail-mounted device (a TS mount
  kind) off every rail is refused in the tool ("-F55 mounts on a TS 35 rail; drop it on a rail."); a screw-mounted
  device always targets the surface; an unknown-size device either. On a surface: free placement snapped to the
  plate grid, to the edges and centres of other footprints and to duct outlines, rotation by R. Modes: free by
  default; Ctrl held push, Alt held swap; a segmented control Free | Push | Swap and the grid 5 mm | 1 mm on the
  tool options card (doc 16a §5.2, §6.20) set the sticky choice; Shift suspends target snapping (D4). A member of an
  assembly whose siblings are placed on the target rail defaults to push at its member-order slot (FR-LAY-061).
- **The ghost** (doc 16a §7.3): the footprint (or the strip's terminals' outline and its brackets) in `canvas.ghost`,
  one label chip with its layout label (§7.4's placement rule), each neighbour that the mode moves as a 1s-px dashed
  accent outline at its new place; a refused drop draws the ghost in `canvas.conflict` with a diagonal cross and the
  refusal text in its chip (shape and text, never colour alone). Release commits `layout.place` or `layout.move` with
  the mode.
- **Select on a panel page:** a click selects a placement or carrier (hit-testing of WP-S4.01 part 3); a drag moves it
  as above; arrow keys nudge by the grid step in the current mode (free: stops where it would touch a neighbour, the
  live region saying "Blocked by -102F2"; push: pushes); R rotates a surface placement 90°, X flips a rail placement;
  Delete runs `edit.delete`; the context menu adds Lock or Unlock.
- **Keyboard placement** (doc 12 §12.4): Tab and Shift+Tab cycle the placements of the focused rail in carrier order
  (the shell maps the canvas's Tab, `tool.next_proposal`, to `layout.next` on a panel page, as it maps the page-move
  chords to `strip.move` (doc 16 §4.4), so a chord stays unique per context, ID-628); Enter focuses
  Properties; the live region announces "-102F1, rail 1 at 158 mm, no findings".
- **HUD prompts** (doc 16a §5.5): "Place -F55 · Ctrl pushes · Alt swaps · Shift no snap · Esc done"; with a placement
  selected "1 selected · arrows nudge · R rotates · X flips · Del deletes".

**Acceptance.** **PW-1 (FT-486)** On Golden A the navigator lists F55, E55, E56, S56, -201U9 under +J-E-2416 with
their part, size and mount kind (S56 and E56 "unknown size"); placing F55 removes its row in the same revision.
**PW-2** A kittest flow drags F55 from the navigator onto R1 between -101F1 and -102F1 with Ctrl held: the ghost shows
the four moved neighbours; release gives SN-3's poses in one undo step; Enter on the F55 row then arrows and Enter do
the same from the keyboard. **PW-3** Dropping a TS 35 device on a TS 32 rail fixture shows the refusal chip and
commits nothing; dropping -F55 away from every rail is refused with its text; dropping E55 (screw) on the plate places
it on the surface at the snapped point. **PW-4** Snap targets: a drag near -102F2's end snaps to it at 15 px; Shift
suspends it; the 1 mm grid option changes the grid step. **PW-5** Dragging -201U9 onto R3 defaults to push at its slot
and gives AS-1's poses. **PW-6** Tab cycles R1's placements in carrier order and the live region speaks the text
above; arrows nudge; R, X and Delete work on a selected placement. **PW-7** Window check (offscreen, `--visual`) at
1920 × 1080 and 1280 × 800: 902 with the navigator open, the push ghost and the selected placement; nothing truncated
at the floor (the navigator's Part column gives way first). **PW-8** Targeted checks for `ew-api`, `ew-viewmodel`,
`ew-ui`.

## 8. Part 4: setting up and editing in the window

**Scope.** D10's command bar, D11, D12, FT-804's window gestures, FT-839. **1.0 pw.** UI briefs cite doc 16a.

- Draw ▸ Layout (Rail, Duct, Auto-fill rail…) and Project ▸ Enclosure… in the registry with icons, labels and docs
  anchors (doc 16a §5.2's registry test); the Create enclosure dialog (doc 16a §6.12; title "Create enclosure",
  primary "Create"): location (default the focused page's), enclosure part (the project's and the installed
  libraries' parts with an enclosure section) → `layout.enclosure.create_from_part`.
- The Draw rail and Draw duct tools and carrier dragging (D11); the Auto-fill rail dialog (title "Auto-fill rail",
  the selected rail, the order Schematic | DT, the devices: the navigator's selection, else every unplaced device
  that fits; primary "Fill") → `layout.auto_fill`, its notice as a toast.
- Properties for layout records and the panel page's Surface view rows (D12).

**Acceptance.** **PS-1** On a new project from the starter library (part 6, else a fixture library), Enclosure…
creates the enclosure with its plate in one undo step; a new panel layout page with Surface view set to its plate
shows the plate centred in the frame. **PS-2** Draw rail with two clicks from (40, 740) to (610, 740) mm (grid 5 mm)
gives Golden A's R1 geometry; a diagonal second click is constrained to horizontal. **PS-3** Draw duct snapped to
the plate's top edge inset by 20 mm gives D1's centreline at v = 880 mm; Backspace and double click work; a duct
starting under D1's lower edge snaps to (20, 860) mm. **PS-4** The Surface view rows commit `page.surface_view.set`;
Label mode Full prints "=P01+J-E-2416-X1031". **PS-5** A placement's Position, Rotation, Realised part and Locked
rows commit their commands; a rail's Free length reads 341 mm on R1 and 381.4 mm on R2; Length lengthens the rail;
the space, enclosure and surface rows commit FT-840's commands. **PS-6** Auto-fill rail on an emptied R1 (fixture)
gives AS-3's poses and toasts what did not fit. **PS-7** The Layout group is enabled on 902 and disabled on 101 with
its reason; kittest flows for PS-1 to PS-6. **PS-8** Window check at both sizes: the command bar with the Layout group,
the Create enclosure dialog, the draw-duct tool mid-gesture, a placement's Properties; no truncation at the floor.
**PS-9** Targeted checks for `ew-api`, `ew-viewmodel`, `ew-ui`.

## 9. Part 5: end brackets in the outputs

**Scope.** D13, FT-204's row 26, the rest of AC-33. **0.6 pw.**

**Acceptance.** **EB-1** BOM row 26 on 901.1 reads quantity 4, tags as before, folio 902; placing a fourth strip
fixture in R2's run gives 5; removing -X1032's placement returns its line to its authored quantity and folios; row
count stays 27 (AC-07). **EB-2** Folio 902 draws R2's four brackets at their positions (±0.2 mm at 1:10). **EB-3**
The legend lists one end-bracket row after the placements with quantity 4 (31 rows); if 31 rows do not fit the
legend block's rect at the form's row height, the lane stops and reports (WP-S4.01's LB-2 rule). **EB-4**
EW-LAY-010 reports a bracket overlapping a placement (a fixture with a 5 mm gap between two strips) and a bracket past
the rail end (-X1031 grown by added terminals); Golden A gives none. **EB-5** AC-33 passes in full; the golden change
is exactly 901.1's row 26, 902's expected outputs and the legend block, decided by the orchestrator and recorded;
every other exported byte equal. **EB-6** Targeted checks for `ew-derive`, `ew-reports`, `ew-checks`, `ew-api`.

## 10. Part 6: the starter library's layout parts

**Scope.** D14, FT-805. **0.3 pw.** Haiku: every value is given; the shapes are those of Golden A's copies
(`golden/J-E-2416.ew/lib/parts/`: an `enclosure` section with `width`, `height`, `depth` and `plate`; a
`mounting_plate` section; a `rail` section with `profile`; `mech` in nm).

**New parts** (manufacturer as the library's others; part numbers and kinds exact):

| Part number | Kind | Description (en) | Data (mm) | Unit |
|---|---|---|---|---|
| EW-GEN-ENC-1 | enclosure | Wall enclosure 600 × 800 × 250 mm | enclosure section 600 × 800 × 250, plate EW-GEN-PLT-1 | piece |
| EW-GEN-PLT-1 | mounting_plate | Mounting plate 520 × 720 mm | mounting-plate section 520 × 720 | piece |
| EW-GEN-RAIL-1 | rail | DIN rail TS 35 × 7.5 | rail section profile `ts35x7_5`, stock length 2000 | m |
| EW-GEN-DUCT-1 | duct | Wiring duct 40 × 60 mm | `mech` width 40, height 60 | m |
| EW-GEN-EB-1 | end_bracket | End bracket for TS 35 | `mech` width 10, height 45, depth 35, mount `ts35x7_5` | piece |

**Dimensions on existing parts** (`mech`: W × H × D, mount kind `ts35x7_5`): EW-GEN-MCB-1 18 × 85 × 70;
EW-GEN-MCB-2 36 × 85 × 70; EW-GEN-FUS-1 18 × 85 × 60; EW-GEN-SW-1 18 × 85 × 70; EW-GEN-RLY-1 27 × 80 × 70;
EW-GEN-CON-1 45 × 85 × 80; EW-GEN-PSU-1 60 × 90 × 110; EW-GEN-CPU-1 100 × 100 × 75; EW-GEN-DI-1 25 × 100 × 75;
EW-GEN-DO-1 25 × 100 × 75; EW-GEN-TRM-1 5.2 × 45 × 40; EW-GEN-TRM-2 5.2 × 45 × 40. The door and field parts
(EW-GEN-SEL-1, -PB-1, -LMP-1, -POS-1, -VLV-1, -MOT-1) get none (door surfaces are 1.0).

**Acceptance.** **SP-1** The generator in `crates/ew-library/tests/starter_parts.rs` emits the five parts and the
twelve `mech` members; `EW_BLESS=1` regenerates `content/libraries/ew-starter-parts.ewlib/` and
`crates/ew-library/src/starter_parts.rs`; `the_shipped_library_is_the_generated_one` passes. **SP-2**
`every_generic_part_is_usable` skips the five layout kinds (no function template; the layout tools use them) and
still checks the others. **SP-3** No test pins the number of starter parts (CONTRIBUTING.md); a test that does is
changed to a relation, not a new total. **SP-4** `cargo nextest run -p ew-library -p ew-api`, the matching clippy,
`cargo fmt --all --check`, `cargo xtask check-licence-map`, `reuse lint`.

## 11. Files owned and what a lane may not decide

### 11.1 Files owned

**Part 1:** `crates/ew-derive/src/layout/**`, `crates/ew-derive/tests/layout.rs`, `crates/ew-commands/src/commands/{layout.rs,mod.rs}`,
`crates/ew-commands/tests/**`, the fuzzer's layout arms, `crates/ew-api/src/verify.rs` (the sweep only). **Part 2:**
`crates/ew-derive/src/layout/**`, `crates/ew-commands/src/commands/{layout.rs,edit.rs,mod.rs}`,
`crates/ew-commands/tests/**`. **Part 3:** `crates/ew-api/src/{lib.rs,output/panel.rs,output/navigators.rs}`,
`crates/ew-viewmodel/src/{canvas.rs,tools/layout.rs,tools/mod.rs,shell/**,navigators.rs,strings.rs,actions.rs}`,
`crates/ew-ui/src/**`, `crates/ew-ui/tests/**`. **Part 4:** as part 3 plus `crates/ew-viewmodel/src/tools/{rail_draw.rs,duct_draw.rs}`,
`crates/ew-viewmodel/src/{dialogs.rs,properties.rs}`, `crates/ew-api/src/output/{properties.rs,fields.rs}`. **Part 5:**
`crates/ew-derive/src/{bom.rs,layout/**}`, `crates/ew-reports/**`, the placements layer where WP-S4.01 part 3 built
it (`ew-scene` or `ew-api`'s output families),
`crates/ew-checks/src/rules/**` (EW-LAY-010), golden files only as EB-5 lists. **Part 6:**
`crates/ew-library/tests/starter_parts.rs`, `crates/ew-library/src/starter_parts.rs`,
`content/libraries/ew-starter-parts.ewlib/**`.

Shared append-only files (registry lists, the fuzzer arms, the strings table, generated command schemas and pages)
may be touched by other lanes; the second to integrate rebases. Read-only for every lane: other crates, `golden/**`
except as named, `ci/**`, `.github/**`, `docs/**` except generated reference pages. Each lane commits on its own
branch and does not merge or push.

### 11.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change needs a new workspace package, a new dependency edge in `ci/dep-rules.toml` or a new external crate;
2. a kind, member or variant of the project format would be typed or added (§1: everything here is derived);
3. any exported byte of Golden A, AC-15's path (b) or the acceptance result changes outside EB-5's list (part 5) or
   at all (parts 1 to 4, 6);
4. a golden fact here (R2's four brackets and their attribution, SN-2 to SN-4's poses, AS-1's poses, row 26's
   quantity 4, the free lengths) is contradicted by the data;
5. a command beyond D5 to D7 would be added, or a refusal that D5 to D7 do not name, or `layout.place` without
   `mode` would change;
6. a placement would move without a command naming it or a mode moving it (D5), or a locked placement would move;
7. the command bar would gain a tab, a group or a key beyond D10, or a user text would name an ID instead of words;
8. WP-S4.01 part 3, 4 or 5, as built, differs from what this document reads from its specification;
9. the work of a later part is reached (D16).

The lane decides freely: private helper names, module layout inside the owned files, the exact texts beyond those
quoted here (doc 16a §8's rules hold), test fixtures, icon choice among the bundled icons.

## 12. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 The ghost and the commit disagree | Medium · high | One resolver (D4); SN-7's property test; PW-2 compares the ghost with the result |
| R2 Push or swap moves a placement the user did not expect | Medium · medium | Push only to the right, the centre split, locked placements never move (D5); every moved neighbour is outlined in the ghost |
| R3 `Occupied` changes collide with WP-S4.01 part 5 built beside it | Medium · low | The change is additive (one field); the second lane to integrate rebases (§11.1) |
| R4 The bracket rule surprises a user whose strips sit further apart | Low · medium | The threshold is stated in Properties' provenance ("one bracket between, gap below two bracket widths"); EB-4 reports a bracket that does not fit |
| R5 The window parts outgrow 1.2 pw | Medium · medium | The de-scope rule of §13 |
| R6 The legend outgrows its block with the bracket row | Medium · low | EB-3 stops the lane; the rect is a fixture choice |

## 13. Size, order and routing

**Size.** 5.2 pw plus 0.3 pw reserve (D15): part 1 **1.1** (padding and brackets 0.25, resolver 0.45, commands 0.2,
tests 0.2), part 2 **1.0** (member order and place_assembly 0.25, auto_fill 0.25, enclosure command and `copies`
0.2, FT-840 0.1, FT-837 0.1, tests 0.1), part 3 **1.2** (panel tables 0.2, navigator 0.25, place and move behaviour
with the ghost 0.4, keyboard and live region 0.15, kittest flows and the window check 0.2), part 4 **1.0** (command
bar and the enclosure dialog 0.2, draw rail and duct 0.35, Properties rows 0.3, auto-fill dialog 0.05, tests 0.1),
part 5 **0.6** (BOM line 0.2, drawing 0.1, legend 0.1, EW-LAY-010 cases 0.1, golden 0.1), part 6 **0.3**.
**De-scope rule:** when a part is projected (at its lane's midpoint report) or found (at its review) more than 20 %
over its size, these items leave as follow-ups without a further ruling, in this order: swap mode (D5), column
alignment with the rail above (D4), the auto-fill dialog (the command stays), carrier dragging (D11), keyboard
placement's Tab cycling (the live region stays); the lane reports which ones left.

**Order and routing** (Opus or Haiku only, ID-700): part 1 first, **Opus at `high`** (core derived data with an
exact geometric invariant; it can start now, on WP-S4.01 parts 1 and 2); part 2 after part 1, **Opus at `high`**
(`edit.delete`'s cascade policies and the one-transaction enclosure command); part 3 after part 1 and WP-S4.01 part
3, **Opus at `high`** (a new session API and the tool behaviour); part 4 after parts 2 and 3, **Opus at `medium`**
(window work within the patterns part 3 sets); part 5 after part 1 and WP-S4.01 parts 3, 4 and 5, **Opus at `high`**
(the BOM and a golden change); part 6 at any time, **Haiku at `medium`** (closed data, regeneration). The
feasibility read before dispatch flags parts 1, 2 and 5 (a core query, delete invariants, a golden change).
**Unblocks:** AC-33 in full after part 5; WP-S4.04's 3D rail-snap dragging reuses part 1's resolver (FR-UI-029).

## 14. Follow-ups and the documents to update

**Follow-ups.** FT-204 closes with part 5 (row 26); FT-486 closes with part 3; FT-804 with parts 2 and 4; FT-805 with
part 6; FT-837 and FT-840 with part 2; FT-839 with part 4. FT-806 stays open for 1.0. Proposed (not opened; the
design pass numbers them):
- **FP-1** Terminal divisions inside a placed strip on 902 (`strip_layout(p)`, doc 12 §2.6), a golden change of 902.
- **FP-2** End covers, markers and bridges from the 1.0 accessory lists (`AccessoryRef{per: Group}`), replacing D3's
  declaration by part use with the same bracket count.
- **FP-3** Editing a duct's path vertices in Properties and by grips (the path is read-only in the grid here).
- **FP-4** The layout tree navigator (space → enclosure → surfaces → carriers → placements; 1.0, doc 16 §5.2).
- **FP-5** The mockup of doc 16a §11 gains the Layout group on Draw and the Unplaced devices rail entry (D10).
- A de-scoped item of §13 becomes a new FT at the design pass.

**Documents.** **After part 1:** doc 12 §2.6 (`group_accessories`, `carrier_labels`, padding as built), §3.3 (the two
halves of snapping, the modes as built), §3.4 (the run threshold and attribution); 02b FR-LAY-060 (the meaning of
"abutting": one bracket between, a gap below two bracket widths). **After part 2:** doc 12 §3.7 (the commands as
built). **After part 3:** doc 16 §4.8 and §5.2 (the Unplaced devices navigator as built), doc 16a §5.4 (the rail
entry), doc 12 §12.1, §12.4. **After part 4:** doc 16a §5.3 (Draw's Layout group, Project's Enclosure…; D10 as the
design-language review), doc 16 §6 (the layout rows). **After part 5:** doc 09 §2.2 (the bracket line), doc 12
§5.2, §5.4, doc 20 §14.1 (row 26's as-built note superseded) and §13, doc 15 §4.5.4 (EW-LAY-010's bracket cases).
**After part 6:** doc 07's starter-library note.

**Open questions for the orchestrator.**
- **O1 (effort).** Doc 02's PERT of 3.1 pw covers the five requirement rows; D1's added scope makes the parts 5.2 pw
  plus 0.3 pw reserve. *Recommendation:* re-estimate WP-S4.02 at O 4 · L 5.5 · P 7 · **PERT 5.5** (σ 0.5) in
  `work-packages.csv` at the design pass and regenerate the schedule; the alternative, moving FT-805 (0.3 pw) to
  WP-S7.03, saves little and leaves a new project unable to lay out a panel.
- **O2 (scheduling).** Part 1 can run beside WP-S4.01 part 5; both touch `crates/ew-derive/src/layout/queries.rs`
  (R3). *Recommendation:* dispatch part 1 in the wave after WP-S4.01 part 5 is integrated, or accept one rebase.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S4.01, WP-S4.02; Appendix B (FT-204, FT-486, FT-804 to FT-806,
  FT-837 to FT-840).
- [Decision log](../06-decision-log.md): ID-317, ID-628, ID-682, ID-700, ID-893, ID-911, ID-924, ID-932.
- [Panel layout](../../design/12-panel-layout.md) §2.3, §2.6, §3, §5, §12, §16.1, §17; [UX and UI](../../design/16-ux-ui.md)
  §4.5, §4.8, §5, §6, §10; [design language](../../design/16a-design-language.md) §5, §6, §7, §8;
  [golden project](../../design/20-golden-project.md) §9, §13, §14.1, §21.
- [WP-S4.01](WP-S4.01-layout-model-and-2d-panel-pages.md), [WP-S2.03](WP-S2.03-viewer-and-tool-framework.md),
  [WP-S2.04](WP-S2.04-placement-wiring-drawing-tools.md), [WP-S2.05](WP-S2.05-property-grid-find-replace-clipboard.md),
  [WP-S2.06](WP-S2.06-navigators-and-table-mode.md).
- [ADR-0008](../../adr/0008-transactions-undo-preview.md),
  [ADR-0031](../../adr/0031-design-language-and-task-ordered-command-bar.md).
