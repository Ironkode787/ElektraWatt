# Schematic editor: the window, outputs, edge cases and defects

A chapter of [Schematic editor](06-schematic-editor.md), whose table says where each section lives.

---

## 6. UI and UX touchpoints

**Canvas navigation.**
- Zoom from 5 % to 5000 % of paper size, in √2 steps, around the cursor. Pan with the middle button, Space-drag or
  Ctrl+arrows; fit page and fit selection.
- Page Up and Page Down walk the page order, generated pages included (read-only).
- Pages open as tabs, and two pages can be viewed side by side with their own viewports (FR-UI-035).
- Clicking an xref or IP text jumps to its target; Back and Forward walk the jump history with the previous
  viewport (FR-UI-019, FR-XRF-019). Hovering an xref for 300 ms shows a CPU-rendered preview of the target region.
- **Go to** (Ctrl+G) accepts a page or a cell in the project's xref format: "103-A1" opens folio 103 with cell A1
  centred (FR-UI-039).
- The status bar shows the cursor in mm and as a cell ("103-A1"), the grid, snap mode, selection count, render
  backend and running jobs (FR-UI-034, FR-UI-028).
- Level of detail: text below 2 px renders as a bar, and hairlines clamp to 1 px (ARCH §8.2). Panning and zooming a
  dense A3 page (about 5,000 primitives) keeps ≥ 60 fps on the baseline machine, and an A4 page ≥ 30 fps on the
  baseline machine without GPU (CPU renderer; NFR-PERF-06).
- **Display language.** Each canvas view renders `MlText` (function texts, page titles, free texts) in a display
  language chosen per view; a missing translation falls back to the source text and is marked (FR-I18N-005).
  Identifiers never change with the language.

The viewer implements this list except the snap mode, the selection count and
the running jobs in the status bar (the fields are in doc 16 §3.6). **Tabs:** at most 32 over one or two groups, each tab with its
own history and a viewport per page. **Following:** a click or F on a cross-reference jumps to its target page in the
same tab and centres the target point at no less than twice the zoom that fits the target page (the previous zoom if it
is larger, the jump view); Alt+← and Alt+→ restore the exact previous view of the tab's history, a fitted view
refitting. Page Up and Page Down walk the page order and add no history entry. **Go to** (Ctrl+G) takes a folio, which
opens the page, or a cell in the project's xref format ("103-A1"), which opens the page with the cell's centre shown and
the grid cursor on it; a target that lacks a non-empty row part or a non-empty column part is read as a page name, so
"103-A" is "not found" rather than "no such cell", because folios can carry hyphenated suffixes. **The grid
cursor** (`CanvasModel.cursor`, a sheet point on the page's connection grid): the arrow keys move it one grid step
(without a cursor it appears at the grid point nearest to the view centre), Shift+Arrow moves it by one cell (the page's
column width or row height, then the nearest grid point; ten grid steps on a page without cells), and after each move the
view pans by the least amount that brings it inside the pane inset by 10 % of each side, the zoom never changing.
Shift+F and Ctrl+Shift+F put the cursor on the next or previous cross-reference in reading order (by cell, then
position), so a keyboard user reaches every link. The cursor is a crosshair over every other overlay, and the canvas
announces the cursor's cell, its position and a link at it (doc 16 §10.1). Selecting by pointer, and the connection-point
jumps with `]`, `[` and N, are built (§4.3, §5.7; keys in doc 16 §4.4). The 300 ms
hover preview of a link is a 320 × 200 point popup beside the pointer while the active tool wants hover (Select yes,
Measure no), as doc 16 §4.5 says.

**Panels the editor drives** ([16](16-ux-ui.md) owns their layout): the page navigator, where drag to reorder opens
the renumber preview (FR-PAG-024); the device navigator with unplaced functions as drag sources; the IP and potential
navigators, which highlight a potential on all pages with pattern plus colour (FR-UI-018); the property grid, message
and history panels; the bulk preview dialog (FR-UI-022); and the command palette with EPLAN and QET aliases such as
"folio report" and "master/slave" (FR-UI-012, NFR-UX-03).

**Keyboard shortcuts.** The bindings of the canvas and of every other shell context are data of the action registry
with conflict detection; the generated keymap reference (`docs/book/src/reference/keymaps.md`, written by
`cargo xtask docs-gen`) lists them per preset, and [the action map](../map/actions.md) per action. The rules they
follow:

- The presets are "EW" and "KiCad-like" (FR-UI-013, which fixes M, R, W and E for the KiCad-like one); "EPLAN-like"
  ships only once EPLAN users confirm its bindings (FR-UI-014). Where KiCad's own hotkeys differ, the KiCad-like preset
  follows KiCad.
- A chord is unique within its shell context and may mean something else in another (Shift+Enter finishes the
  active tool on the canvas and opens the multi-language editor in the property grid).
- Punctuation keys bind by physical key ([16](16-ux-ui.md) §4.4), so they stay reachable without AltGr on QWERTZ and
  AZERTY layouts. On macOS Ctrl reads Cmd where [16](16-ux-ui.md) §4.4 sets no macOS binding; macOS is not a
  supported platform and this is not tested.
- A pane takes its own navigation keys: the picker Up, Down, Home, End and Enter; the property grid the arrows, Home,
  End, Enter, Space (a checkbox) and the keys that start an edit; the find pane Enter (search, or jump on a result),
  Up, Down, Home, End and Tab. Escape stays the global dismiss.

**Accessibility.** Every tool is operable from the keyboard (NFR-A11Y-01). The selected object is announced as a live
region ("-102G1, power supply 24 V DC, 7 connection points, 9 wires"; FR-UI-026, NFR-A11Y-02), and the navigators
are the accessible representation of the page. Highlights and findings use pattern plus colour (NFR-A11Y-03). Wire
colours always show their IEC 60757 code.

---

## 7. Outputs

| Output | Content | Consumer |
|---|---|---|
| Transactions | command, ops, origin, page, viewport, selection | undo stack, journal, macro recorder, `ew diff` (ADR-0008) |
| Display-list layers | frame, symbols, wires, labels (derived texts), highlights (session only) | `ew-render2d`, `ew-render-cpu`, PDF, SVG (ADR-0015) |
| Link sources and targets | every xref text a `LinkSource`; every anchor a named destination (v5 IDs) | PDF links, screen jumps (FR-OUT-005, FR-OUT-007) |
| View state | tabs, viewports, jump history, tool options | `.ew/u-<hash>/` (never committed; FR-UI-033) |

The editor writes no derived value. A PDF exported while a tool is active is rendered from the snapshot taken at
export start (FR-OUT-001).

---

## 8. Edge cases and pitfalls

1. **Frame change re-targets xrefs.** Assigning folio 106 a 10-column frame changes every xref into 106 and no
   other (FR-XRF-021). `fields.set` on `Page.form` shows a toast with the count of changed xref texts and an undo
   button.
2. **Anchors outside the drawing area.** A drop is clamped so that the anchor stays inside; imported violations are
   reported by FR-CHK-035.
3. **Mixed grids.** Pasting from a QET-grid page onto a native page keeps nm geometry. The fragment's insertion point
   snaps to the native grid, but its CPs stay on the 10 px lattice, so autoconnect cannot align them with native
   symbols. The ghost warns ("12 CPs off the page grid"). The remedies are to keep QET symbols on QET pages or to
   convert the page (FR-PAG-022, 1.x). A paste onto a page of another grid is fitted to it: each
   symbol moves so that its first pin lies on the nearest grid point of the page (a symbol whose own pin spacing does
   not fit keeps its other pins where its spacing puts them), junctions and structure-box corners go to their nearest
   grid points, page items keep their place with the rest, and the pasted wires are routed anew between the same pins
   in the paste's undo step; a toast says the paste was fitted, how many items moved and that the wires were redrawn.
   A page's Connection grid changed in Properties while it holds items asks first (what moves, that the wires are
   redrawn; Change and fit or Cancel) and fits the whole page the same way (`page.set_grid`), one undo step.

   *The fit parts what it would push together.* Each item going to its nearest grid point alone would put two
   symbols on one another and two pins so close that a wire finds no way. The fit moves the symbols and junctions of
   the page (of the paste) together, one function for `page.set_grid`, `edit.paste` and `macro.insert`
   (`geometry::fit`), axis by axis, x first. Each item takes the box of its body and its pins and goes onto the grid by
   its point (a symbol's first pin, a junction's own point): to the nearest grid point, then, taken in the order of
   their left (top) edges, as many whole steps further right (down) as keep the gap to every item before it that
   faces it: a gap of a step of the new grid or more (room for a wire) at least what it was, or two steps where it was
   more; a narrower gap at least open. Two items face each other along x where their boxes share some height, along y
   where they share some width after the x pass. *It keeps* the order of the things that face each other along each
   axis; two things that stood apart do not overlap; two pins more than a step apart keep a grid line between them for
   a wire. *It gives up* the exact nearest point of an item pushed on (a row of wide symbols whose pitch the new grid
   does not hold spreads to whole steps; a strip of terminals closer than a step may close up to touching), and the
   line-up of things that do not face each other (a column above a row may part from it by a step; its wires, routed
   anew, take the turn). Items that overlapped along an axis before keep no rule between them there. *A row keeps
   one pitch.* Three or more items of one box height (width, for a column) whose points stood at one pitch
   along the axis are a row: the first goes to its nearest grid point and the others follow at one pitch of whole
   steps, the old pitch's nearest that keeps the gaps between every two of them by the rule above (twelve narrow
   symbols 5.29 mm apart on a 4 mm grid stand 8 mm apart throughout, not by turns 4 and 8 mm); another item before the
   row still pushes a member on where it would be overlapped.
   *A pin's wire keeps its way.* The gaps alone would leave wires without a way (a card whose pins face the drawing
   area's edge too close to it, the router keeping a step from both the edge and a body; a terminal wired straight up
   to a contactor's pin a few ticks beside it, its column closed by a neighbour). So `page.set_grid` takes the page's drawing area as the router reads
   it (`area`, which the window fills): a symbol with pins on a side of its box that faces an edge of the area (the
   pins on that side, their wires leaving towards the edge) whose wires turn along that edge, which stood a step of the
   new grid or more from that edge, keeps a grid line for each of those pins, a step from the edge, from one another
   and from its box; a pin whose wire runs straight on to its other end (a mark in line with it) needs none. From the
   left (top) edge it is pushed on, from the right (bottom) edge taken back by whole steps as far as the gaps before it
   allow; each pin has a line of its own. Two pins wired straight that stood in line (a
   wire without a route, or one along their line) stay as near in line as the grid allows: the later of their symbols
   along the axis goes to the grid point nearest the line of the other's pin as fitted (field devices under a card's
   channels follow the card when it is pushed on). Where the pin of only one of them lands on the grid (the other's
   symbol has its own spacing), the wire cannot stay straight; what stands beside that pin's line between the two pins
   keeps a step from it, so the wire's step has a column. A `page.set_grid` without `area` keeps no room at the edges;
   a paste and a macro fit by the gaps alone.
   Structure
   boxes take their corners' nearest grid points and page items keep their places. A copy of pages is no
   fit: it keeps every position as its source holds it (§3.6).
4. **QET anchors.** QET-imported symbols keep QET's element-origin anchor so that imported texts match the source,
   including "3-B0"; FR-CHK-035 then reports the empty cell (FR-IMP-038).
5. **Graphic lines used as wires** (a QET habit). A graphic line ending on a CP has no meaning; the lint EW-CORE-071
   (kind hygiene, online, MVP) reports it at info with the quick fix "convert to connection".
6. **Two CPs at one point.** Hit-testing offers a chooser after a short hold.
7. **Junction dissolution.** Deleting one branch of a three-way junction merges the remaining two connections into
   one, which keeps the UUID of the connection first in the sequence. If both carried frozen numbers, the dropped
   number is reported (FR-NUM-022).
8. **Star source deleted.** Deleting the star source of an IP name leaves its targets unpaired. The finding offers
   "make this point the star source".
9. **Chain with an odd count.** A third "L-1" in chain mode pairs with nothing until a fourth exists (FR-XRF-007);
   the pending finding names the expected partner.
10. **External change while a tool is active.** A git pull lands as an undoable transaction (ARCH §7.4); the ghost
    is recomputed, and the tool cancels if the entity under it vanished.
11. **Assembly overviews are generated.** Dragging a valve onto folio 202 is refused with "edit -202U2's children
    in the device navigator" (FR-DEV-023, D-10).
12. **Main representation moves change tags.** Dragging -101F1's symbol to folio 102 changes its tag to -102F3. The
    ghost label shows the new tag before release, and the toast offers "freeze previous tag", which creates a frozen
    tag reported at info.
13. **Black box resize.** CPs slide along the outline and cannot leave it; a CP on a removed polygon edge moves to
    the nearest remaining edge and is listed in the preview. The CPs of a side keep their order and
    spacing and stay centred on it as a group, on grid points, a grid step clear of each corner; on a shorter side the
    spacing closes up to one step, and below that the grip stops and the status line says why (`box.reshape` refuses
    it in words); across the box it goes no lower than its longest top pin text and its longest bottom pin text
    together need, and no narrower than its longest left and right pin texts together need, each with a gap, one rule
    for the four sides, and the grip stops there (the texts stay as they were drawn: turned a quarter on the
    top and bottom sides, horizontal on the left and right). The representation's position follows the outline's top-left corner, so the box's texts keep their
    place relative to it. A CP never moves onto a corner; Centre the pins (the box's right-click, `box.reshape` with
    `centre`) re-centres a box stored with one there.
14. **Large pastes.** Above 500 records `edit.paste` becomes `Cost::Bulk` and previews as a job (NFR-PERF-08,
    NFR-PERF-19). Autoconnect, routing, folio proposals and paste numbering break ties by ID, never by hash-map order
    or wall clock (NFR-REL-07).
15. **Fragments from another EW version.** The `#ew-fragment` header carries the format version. An older fragment
    runs through the per-kind upgrade chain before paste (ADR-0011); a newer one is refused with "copied from EW
    x.y, which writes a newer format", never pasted partially.
16. **Generated pages in the clipboard.** Copying from folio 901 or 2 copies nothing editable; the editor offers
    "copy as image" and "copy table as TSV" instead.

---

## 9. Walkthrough: drawing folios 101 and 102

The sample project (`fixtures/sample/`) is drawn by its script
through the window's own actions, and the tools are specified in §3 and §4.

---

## 10. Reference defects this editor prevents

| Defect | How the editor prevents it |
|---|---|
| D-01 stale -4W1 | Tags are never typed into texts; page renumber is a previewed command from the navigator, and its diff lists -3W1 → -4W1 everywhere (AC-20) |
| D-02 layout tags "T51" | "Show all representations" and selection sync bind every drawing to one device; no free-text tag exists |
| D-03 mixed schemes, "+JE2416" | A typed tag outside the scheme becomes a reported frozen tag; structure nodes are picked, not typed; paste maps aliases |
| D-04 parts missing (S56, E56) | "Device without part" is a pending finding while drawing and a full warning on leaving the page (EW-CORE-041); part-first placement makes the part the default |
| D-06 description drift | Page properties offer the title, not the title-block description; an override is explicit and reported (EW-CORE-023) |
| D-07 no links | Every xref text is a link source with a named destination (§7) |
| D-08 free-text wire rule | Wire markers display connection data and profile-derived values; no free text can look like a marker |
| D-10 5 valves in the overview | Assembly overviews are generated; the canvas refuses drawing on them |
| D-11, D-12 terminal designations | Prevent rules refuse empty and duplicate designations at commit in strict mode |
| D-13 N switching | EW-POT-013 fires while the circuit is being drawn, not at release |
| D-14 "3-B0" | Cells come from anchor policies; an empty target cell is reported at once |
| D-17 core colour "BR" | Colour fields accept only IEC 60757 codes and profile codes; "BR" is offered as "BN" |
| D-19 supply without cable | Wiring an external connection point without a cable core raises the pending finding EW-CORE-034 |
| D-20 junction notation | Junctions are objects with a wiring sequence and a preview of the physical wires |
| D-22 0 V–PE wire | The type conflict appears on the connecting gesture, with a bond quick fix |
| D-23 unconnected 13-14 | Pending, then full finding; "mark unused" quick fix sets the CP override `unused` (FR-DEV-008) and drops the finding to info |
| D-24 five names on one net | Highlight and property grid show the canonical potential with its IP aliases |
| D-29, D-31 text sizes | Slot text heights come from styles and cannot be resized per text; the minimum-height rule (EW-CORE-061) runs online |
| D-38 PE smaller than line | EW-POT-006 fires when the PE wire is drawn, with the quick fix "set W6 to 2.5 mm²" |
| D-39 IP order | "Arrange" sorts IP stacks |
| D-41 assembly tagged from two pages | Folio 202 holds a continuation segment of -201U1's generated assembly view, so the drawing shows one assembly; child order is edited in the device navigator (`Device.position`), never by drawing on the generated segments (FR-DEV-022, FR-DEV-023) |
| D-42 strip drawn twice | Strip segments draw one device in several places without a second tag or BOM row |

