# Design language: components: palettes, chips, grids, choosers, preview cards and finding rows

A chapter of [Design language](16a-design-language.md), whose table says where each section lives.

---

### 6.14 The command palette

Opened by Ctrl+K, Ctrl+Shift+P or the command search field: 560 px wide, anchored under the search field (or centred
at the top fifth of the window when opened by key), `elevation.2`, `radius.lg`. The search field is 36 px with a
search icon and no title above it. Result rows (32 px): icon, label with the matched characters in `type.body.strong`,
the breadcrumb in `color.text.tertiary` ("Draw · Place"), the key as key chips at the right. Disabled results follow
the enabled ones with their reason in `color.text.secondary`. Actions that cannot run from the window in this build
are not listed (P3); scripts still reach them.

The palette has no title, shows breadcrumbs and key chips, and **lists neither actions that need parameters nor model commands**; both stay reachable headless through `command.run`.

---

### 6.15 The symbol and part palettes

Grid of tiles, 88 × 104 px: a 72-px thumbnail on `color.surface.sunken` with `radius.md`, the name in `type.small`
under it (two lines, ellipsis), for parts the state line ("2 of 4 placed" in `color.text.secondary`; "All placed" in
`color.success` with a check icon; "1 unplaced" in `color.warning` with its icon). Hover lifts nothing; it shows the
hover overlay and a tooltip with the full name, library and version. Selected or armed: Selected state with an accent
2-px border.

---

### 6.16 Chips and badges

| Kind | Drawing | Example |
|---|---|---|
| Count badge | `radius.pill`, `type.caption`, 18 px high, `space.icon` padding; the semantic subtle fill with the semantic text, or `color.surface.sunken` with `color.text.secondary`; on the chrome (a dock tab's count chip) the fill is `color.badge.on_chrome`, because `color.surface.sunken` is the chrome's own colour in light and dark | a finding count on a tree row, "3" on the Messages tab |
| Provenance chip | `radius.pill`, `type.caption`, `color.text.tertiary`, 1-px `color.border.default`, 16 px high | "inherited", "page" (§6.8) |
| Status badge | `radius.sm`, subtle semantic fill, icon + word in `type.caption` | "Pending", "Stale", "Read-only" |
| Key chip | `radius.sm`, 1-px `color.border.default`, `type.caption`, `color.text.tertiary`, `space.2` padding; the keys separated by a space | "Ctrl K", "R" |
| Canvas chip | §7.4 | "-X1031:21", "24VDC-1" |

---

### 6.17 Empty states

Every list that can be empty says why and what to do, centred in its pane: a 32-px icon in `color.text.tertiary`, one
line in `type.body.strong` ("No jobs yet"), one sentence in `color.text.secondary` ("Exports you start appear here
with their progress."), and at most one action. A filter with no hits says "No pages match "abc"" with a Clear button.

---

### 6.18 Tooltips

`color.surface.inverse`, `color.text.on_inverse`, `type.small`, `radius.md`, max 320 px, after 500 ms of hover or at
once on keyboard focus; the label, the key in a lighter tone, one sentence, and "F1: help" when a topic exists.

A tooltip stops at eight lines, ending in "…".

---

### 6.19 Progress

Determinate: a 2-px accent line under the job field and in the Jobs row, with the percentage as text. Indeterminate: a
12-px spinner beside the text. A finished job names its outcome as a status badge (Done, Cancelled, Failed) next to
its result.

---

### 6.20 Segmented control

Two to four mutually exclusive views of one panel ("Pages | Devices"): a track on `color.surface.sunken` with
`radius.md` and 2-px padding, inset `space.4` from the panel's sides; equal-width items `size.control.inline` high
with `radius.sm` in `type.small` and `color.text.secondary`; the chosen item on `color.surface.panel` with
`elevation.1`, SemiBold and `color.text.primary`. It is a `TabList` for AccessKit; Left and Right move between items.

---

### 6.21 Data grid (a navigator's table mode)

The table of §6.7 grown into an editable grid, for every navigator's table mode (egui_table). The behaviour is
doc 16 §7's; this section fixes its look, and says where the grid draws otherwise.

**Above the grid**, in the pane's tool line under the panel header: the **Table toggle**, a 24-px icon button
(`table`) that takes the Selected state while the pane is in table mode (drawn as a toggle button named "Table");
the filter field (§6.5), shared with the tree view and the only filter of the grid; the **Columns** menu button (a
Subtle button with a chevron) whose menu (§6.13) lists every column with a check icon, the key column's item
disabled with its reason, and for each movable column on show two 24-px icon buttons (`arrow-left`, `arrow-right`)
named "Move <column> left" and "… right"; and, when an edit failed, the error line of §6.1 (icon and message
in `color.error`, `type.small`).

**Header** (`size.table_header`, `color.surface.sunken`, sticky, a 1-px `color.border.subtle` line under it): each
header cell is a Subtle button with the column label in `type.small` SemiBold `color.text.secondary`; the sorted
column's label is `color.text.primary` and is followed by a 16-px sort icon (`arrow-narrow-up` ascending,
`arrow-narrow-down` descending; the grid draws the characters ↑ and ↓); a derived (read-only) column's label is
followed by a 16-px `math-function` icon in `color.text.tertiary` with the tooltip "Derived; read-only" (the grid
draws the label in italics). A click sorts and a second click reverses. The **resize handle** at a header cell's right
edge is invisible at rest; on hover and while dragging it is a 1-px `color.accent` line over the header and body, as
the dock splitters (§4.14), and its name is "Resize <column> column".

**Frozen key column**: the first column stays at the left while the grid scrolls (egui_table's sticky column) and is
closed on its right by a 1-px `color.table.frozen_edge` line from the header to the last row, the grid's only vertical
line. No shadow: the panel is flat (`elevation.0`).

**Body rows** (`size.row`, `type.small`, `space.4` cell padding, one 1-px `color.border.subtle` line under each row,
no zebra, columns `size.table_column` wide and resizable): text in `color.text.primary`; derived cells in
`color.text.secondary` (the header icon and the cell's accessible name ", read-only" carry the meaning; italics go,
since no italic face is bundled, §4.8); a read-only cell shows no edit affordance and its reason as tooltip.

**The columns fill the grid.** Each column is sized to its widest cell on show, at least its start
width; a table with a filling column (the Pages Title, the Unplaced devices' Part, the Libraries Name) gives that
column the room the others leave, and every other table shares that room evenly among its columns, less a scroll bar's
room. A resize of the grid sizes the columns again (a width dragged by hand gives way to it).

| State | Drawing |
|---|---|
| Rest | As above |
| Hover | `color.state.hover` across the row |
| Marked (selected) | `color.accent.subtle` across the row; tertiary parts drawn in `color.text.secondary` (§4.15) |
| Cursor row | a 2-px accent bar (`radius.mark`) at the key cell's left edge, inset 4 px top and bottom (the grid draws the key cell selected) |
| Cursor cell | 2-px `color.table.cursor` ring inside the cell while the grid has the keyboard; 1-px `color.table.cursor_idle` ring otherwise (the grid draws underlined text on a selected fill) |
| Editing | the cell holds an inline value cell in its editing look (§6.8: `color.surface.panel`, 1-px accent border and 1-px accent ring, `radius.sm`, inset 2 px) |
| Disabled | a read-only project changes nothing visible; the double click does nothing |
| Error | the error line above the grid; a pattern error inside the pattern card |

The grid itself is the focusable node: the focus ring of §6.1 is drawn around the whole grid, inset 1 px into the
pane, and the cursor cell shows where the keys act. The cursor row's bar sits inside the ring (left edge + 1 + the ring's
width), at one position with or without focus; the grid's scroll handle is `color.border.control` at rest and
`color.text.secondary` on hover or drag; the cell editor sits on the cell's text baseline. **Density** changes the row height only; header, column widths
and padding stay. **Keyboard and accessibility:** arrows, Tab and Shift+Tab, Home, End,
Ctrl+Home, Ctrl+End move the cursor; Shift+Up and Shift+Down extend the marks; Enter opens the row; Ctrl+Enter applies
the table card; a printable character starts editing; in the editor Enter commits, Ctrl+Enter commits every marked
row, Escape reverts. The editor takes every character typed after the first, also in
the frame it opens in; while more than one row is marked a tooltip under the editor (§6.18) says "Ctrl+Enter sets all 12
marked rows."; Shift+click marks the range from the cursor row, Ctrl+click toggles a row and Ctrl+A marks every row of the
filtered table; `table.set` without `rows` sets the marked rows (else the cursor row) and names a column by its key or
its header label. A column the grid scrolls into view (the cursor's cell, the cell editor) lands whole to the right of
the frozen key column, and one wider than the room with its left edge there. The grid is a `Table` named "<navigator> table", a header cell a button "Sort by <column>", a
cell a `Cell` named "<column> <text>, row <i> of <n>[, read-only]", a resize handle a `Splitter`.

**Cards of the grid** follow §6.9 and §6.23: the **pattern fill card** (title "Pattern fill" in `type.body.strong`,
the field with its hint, the first values as a polite live line in `type.small` `color.text.secondary`, the error
inline; Enter applies and Escape closes, so it has no buttons) and the **table card** of a paste, fill or
edit gesture (the gesture title, a Stale or Failed status badge in place of the bracketed word, the ignored cells as a
secondary line, each blocking cell as a row with the error icon, "row · column · text · reason", and an ambiguous
cell's readings as Secondary buttons).

---

### 6.22 Chooser list

A popup list of candidates the user picks one from, for a choice too small for a dialog and too contextual for the
palette. `color.surface.raised`, `elevation.1`, `radius.md`, `space.2` padding, `size.chooser`; no title. Rows are
`size.row`: a 16-px kind icon in `color.text.secondary`, the printed name in `type.body`, and right-aligned the kind
word in `type.small` `color.text.tertiary`; the active row has the hover overlay (as menus, §6.13, never the accent)
and, when the candidates are canvas objects, its object shows the hover outline of §7.1 while it is active. It opens
8 px from the pointer by the placement rule of §7.4 (the HUD and the statistics panel are obstacles) and never covers
the pointer's 16-px box. A last line in `type.caption` `color.text.tertiary` shows the keys as key chips ("↑ ↓",
"Enter", "Esc"). Keys: Up and Down (wrapping), Home, End, type-ahead on the name, Enter picks, Escape closes and leaves
everything as it was. AccessKit: a `ListBox` named by the question ("Objects here"), its rows `ListBoxOption`s with
position and set size; the active row is announced. **Density** changes the row height only.

The pick stack is walked by repeated clicks (§6.25); a chooser for it is not built. The component is fixed here so
that any "which one?" choice gets this look and not a new one.

**Two uses.** The first is the strip editor's accessory
part choice when several parts fit. The second is the Place tool's **Join chooser** (D2, J;
doc 16 §4.4): "New device" first, then the device's unplaced slots with the kind word "slot 13-14", then the devices
whose family takes auxiliary functions, in natural tag order, type-ahead on the tag; Enter arms the target (the ghost
label reads `join -K1`), Escape closes it and keeps the target. Its active row's device shows the hover outline of
§7.1 (`ShellModel::chooser_object`), and it is in the pixel gate as `join-chooser-light-comfortable`. The
chooser's rows and keys are registry actions (`chooser.row`, `chooser.type`, `chooser.choose`), so the headless mode
drives it.

---

### 6.23 Preview card, the Preview tab and the card strip

The card of §6.9 in its three places (FR-UI-022). The behaviour is doc 16 §7's.

**Inline cards** (the property grid's, the quick fix's, the Pages reorder, the table card, Find's replace, the parts
import): §6.9 with these details: the title in `type.body.strong`; "as of rev 3" in `type.small`
`color.text.tertiary` after it; a stale or failed card carries a **status badge** (§6.16: "Stale" in the warning
style, "Failed" in the error style, with the failure's sentence under the title in `color.error`) instead of the
as-built "[stale]" text; the summary line in `type.small` `color.text.secondary`; Apply (Primary) and Cancel
(Secondary) right-aligned.

**The Preview tab** (bottom dock): a header row with the title, revision and badge at the left and Apply and Cancel at
the right, so the actions stay visible above long lists (the tab draws them under the summary); the summary line under
it ("12 items · 4 tags · 6 cross-reference texts · 3 pages", a count of none left out, as
in the cards' lines; an import's created and skipped counts); then the
lists **Entities, Tags, Cross-references, Pages** as collapsible sections with headers in the style of the property categories
(§6.8: 28 px, `type.small` SemiBold `color.text.secondary`, a chevron) and a count badge (§6.16) instead of "(n)".
List rows are `size.row` in `type.small`: an entity row shows the change word ("Changed", in sentence case as the
row's first word) in `color.text.tertiary` in a 64-px
column, the kind in `color.text.secondary`, the label in `color.text.primary` and the changed fields in
`color.text.tertiary`; a tag or cross-reference row shows "old → new" with the arrow in `color.text.tertiary` and, for a
cross-reference, the folio as the meta column (tabular digits). Each list shows `size.preview_list` rows, then
scrolls. Without a card a tab the user opened by hand shows the empty state of §6.17 ("No preview is open"); a tab a card opened closes with its last card. AccessKit: a
`Group` named "Preview", the header a polite live region, buttons named by their text.

**The card strip**, in an origin pane while the Preview tab shows its card: one line `size.card_strip` high on
`color.card.strip`, `radius.md`, `space.3` padding: a 16-px `eye` icon in `color.text.secondary`, "Preview open:
<summary>" in `type.small` with ellipsis and a tooltip when cut, then Apply (a Subtle button with its label in
`color.accent` SemiBold), Cancel and Show (Subtle). A polite live region.

A staged card is said in four places: the dock opens to show the card's header with the Preview tab in front
(§5.7); the Preview tab carries a 6-px `color.accent` dot after its title for as long as a card waits, also behind
another tab, its description the status bar's words; the header row is tinted with the accent once when a card first
shows, fading out over 1.5 s of frame time (still with reduced motion); the status bar's waiting field (§5.8). The
strip also stands for a bulk command's card (Update…, Delete page, the renumber dialogs) in the Pages pane under its
tool line and in the Libraries pane under Update…, so the card can be applied or cancelled where it began.

A failed card leads in words with what blocks the apply: a macro card reads "The macro clashes with this project and cannot be inserted here (3 conflicts)", and the rule code follows under it as secondary text in `type.small` `color.text.secondary` ("Rule …"); the conflict list below is not repeated in the lead. A macro failure without conflicts reads "The macro cannot be inserted here: …" with the refusal's words; every other failed card keeps its "card-failed" sentence.

---

### 6.24 Finding rows, History rows and the chip line

**Finding rows** (the Messages tree) are tree rows (§6.6) with a severity icon of
`size.severity_icon` before the title, in the severity's colour: error `circle-x` in `color.error`, warning
`alert-triangle` in the warning icon colour (§4.4), info `info-circle` in `color.accent`; the three shapes stay distinct. Severity group rows ("Errors", "Warnings", "Infos")
are group rows with the severity icon in place of the folder and their count in the meta column; rule rows show the
rule ID in the meta column. A group or rule row's count stands right after the
row's name, `space.2` from it, not in the meta column at the row's far end, so the name and its count read together
at any width. Severity never rests on colour: the shape differs, the group row names it, and the
accessible name of every finding row carries the severity word. A **pending** finding draws its icon and
title in `color.text.tertiary` and carries a "Pending" status badge in the neutral style (`color.surface.sunken`,
`color.text.secondary`), as in the badge list. Enter and the double click jump to the object.

**The Messages header** (under the panel header): the scheme in `type.small` `color.text.secondary`, then one count
badge per severity (§6.16, the severity icon at 12 px, the number, the semantic subtle fill and text), then "waived n"
and "pending n" in `color.text.tertiary`; the polite live value is the sentence ("<scheme> · E 1 · W 2 · I
0 · waived 0 · pending 1"). **The detail block** of the cursor's finding, under a 1-px `color.border.subtle` line: the
rule ID and title in `type.body.strong`; the kind and severity in `type.small` `color.text.secondary`; the message in
`type.body`; "Quick fix: <label>" in `type.small` with **Fix…** as a Secondary button (disabled on a read-only project,
with the reason as tooltip, §6.1); **Rule page** as a link in `color.accent`. The quick-fix card is §6.23's inline
card. A finding row's Fix status badge is a button too: a click opens the same card for that row. A Properties
finding chip opens Messages filtered to the record at its first row; with several records selected it opens Messages
without a filter. The filter is the words the
findings name the record by: a page's folio facet, a tube's name (its tag, else its two ends) and an unnumbered wire's
ends, else the title. The pane has no footer (the owner, 2026-11-12); About keeps the disclaimer's one line
(NFR-COMP-04).

**History rows** (the History panel; newest first): `size.row` list rows: a 16-px state
icon column (the current entry `arrow-right` in `color.accent`, an undone entry `arrow-back-up` in
`color.history.undone`, a done entry none), the entry number right-aligned in a 28-px column in `type.small`
`color.text.tertiary` with tabular digits, the label in `type.body` (`type.body.strong` for the current entry; a step of the project settings by what it sets, a drawing setting changed alone by its setting, "Print wire
numbers", else "Set drawing data", "Set frames", "Set languages" or "Set supply", in the row and in the Undo and Redo
toasts), then in `type.small` `color.text.tertiary` the origin and the pages, then a "Not
undoable" status badge (neutral, `lock` icon) where they apply. Undone entries draw their label in
`color.history.undone` with the icon, so the state is not colour alone; their accessible name ends ", undone".
The row cursor takes the Selected state. The row actions **Undo to here** (`history`) and **Show changes**
(`list-details`) are 24-px icon buttons in `color.text.secondary`, named and with tooltips. **Marker
lines** are separator rows: the marker text in `type.small` `color.text.tertiary` followed by a 1-px
`color.border.subtle` rule to the row's end (the panel draws the text in italics). The header ("12 steps · cursor at 9") is
`type.small` `color.text.secondary`; the empty panel is §6.17's empty state ("Nothing to undo yet"). **Show changes**
is a small dialog of §6.12 in its frame over its backdrop, titled "Changes of step n", its records as list rows, Close
as a Secondary button; Escape or a click beside it closes it.

**The chip line** ("Not on this page:") above the canvas: the label in `type.small`
`color.text.secondary`, then one **jump chip** per target: `radius.pill`, 20 px high, 1-px `color.border.default`,
`space.icon` padding, a 12-px `file` icon and "Go to page <folio>, <cell>" in `type.small` `color.accent`, the hover
overlay and the focus ring, named as it reads. On a bus-topology page
that does not draw the device yet, a Secondary button Put it here follows the chips.

---

### 6.25 The built widgets mapped

Every widget built before or alongside this document that §6.2 to §6.24 did not name, with its component, tokens,
states, density and the keyboard and accessibility it keeps.

| Widget | Component | Tokens | States | Density | Keyboard and accessibility |
|---|---|---|---|---|---|
| Table mode grid (`nav_table.rs`) | Data grid §6.21 (header, frozen key column, cursor, editor) on §6.7 | `size.table_header`, `size.table_column`, `size.row`, `color.surface.sunken`, `color.table.frozen_edge`, `color.table.cursor`, `color.table.cursor_idle`, `color.accent.subtle`, `color.state.hover` | Rest, hover, marked, cursor row, cursor cell (focused and idle), editing, read-only, error | Row height | §6.21: a `Table` with `Cell` and `Splitter` nodes, sort buttons, the cell cursor's keys |
| Table toggle, Columns menu, pattern fill card, table card | Icon button §6.3; menu §6.13; card §6.9 and §6.23 | `size.control.inline`, `color.surface.raised`, `elevation.1` | Toggle Selected; menu items checked or disabled with reason; card stale or failed | Menu rows `size.row` | Named toggle, check boxes, "Move <column> left/right"; the pattern field's Enter and Escape |
| Messages tree rows and severity icons (`navigator.rs`, `messages.rs`) | Tree row §6.6 with the finding-row anatomy §6.24 | `size.severity_icon`, `color.error`, the warning icon colour, `color.accent`, `color.text.tertiary` | Rest, hover, selected, focused, pending | Row height | `Tree` and `TreeItem` with level, position, expanded; severity word in the name; F8, Enter, `navigator.jump` |
| Messages header counts, detail block, footer | Count badges §6.16; text styles §4.8; Secondary button and link §6.2 | `color.*.subtle` fills, `type.small`, `type.body.strong`, `type.caption` | Fix… disabled with reason on a read-only project | None | Header a polite live region; buttons named by their text |
| Messages in table mode (not built) | Data grid §6.21; the severity cell draws icon and word in SemiBold in the severity colour (the mockup's Severity column) | as the grid, plus `size.severity_icon` | as the grid | Row height | as the grid |
| Messages quick-fix card | Inline card §6.23 | as §6.9 | Stale, failed | None | Live summary; Apply, Cancel |
| Preview tab (`preview.rs`) | Preview tab §6.23 | `size.preview_list`, `size.row`, `type.body.strong`, status badges | Card open, stale, failed, empty | List row height | `Group` "Preview"; polite header; Apply, Cancel |
| "Preview open" strip | Card strip §6.23 | `size.card_strip`, `color.card.strip`, `color.accent` | Rest, hover on its buttons | None | Polite live; Apply, Cancel, Show |
| History panel (`history.rs`) | History rows §6.24; empty state §6.17 | `color.history.undone`, `color.accent`, `size.row`, count and status badges | Done, current, undone, cursor (Selected), focused | Row height | `Group` "History"; each entry a `Group` named by label and state; the actions `history.row` and `history.undo_to` exist, but the window routes no Up, Down or Enter; Tab to the row buttons |
| Show changes popup | Dialog frame §6.12 (small, non-modal) | `color.surface.raised`, `elevation.2`, `radius.lg` | — | List row height | Escape and Close |
| Pick stack (`tools::pick_stack`) and the hover outline | Canvas hover outline §7.1 on the stack's first entity; the walk moves the selection (§7.1) | `canvas.hover`, `canvas.selection` | Hover, selected | None | The repeated click walks the stack, Shift and Ctrl never walk; no node of its own |
| Chip line "Not on this page" (`navigator.rs`) | Jump chip §6.24 | `radius.pill`, `color.border.default`, `color.accent` | Rest, hover, focused | None | Buttons "Go to page <folio>, <cell>" |
| Highlights and their name chips | §4.6, §7.4, §7.5 (complete) | `canvas.highlight.1` to `.4`, `canvas.highlight.label` | — | None | View state only; no node |
| Jobs pane (`jobs.rs`) | Table rows §6.7; progress §6.19; outcome badges; empty state §6.17 | `color.accent` (progress line), status badges | Running, done, cancelled, failed | Row height | `Group` "Jobs"; a running entry a `ProgressIndicator`; Cancel, Open |
| Toasts and the notifications log (`notifications.rs`) | §6.10 (complete; it moves the stack to the canvas's top right) | `elevation.2`, `radius.lg` | Action valid or not | None | Polite live; 8 s on the window's clock; the log never takes the focus |
| Recovery dialog and the read-only banner | §6.12, §6.11 (complete) | `elevation.2`, the warning subtle fill | — | None | Escape cancels; Enter the Primary |

**Proposed follow-ups** (behaviour the design language suggests; none is built):
1. **Pick-stack chooser.** Alt+click (or a held click) on overlapping objects opens the chooser list of §6.22 with the
   pick stack, the active row outlined on the canvas; the repeated-click walk stays.
2. **Pick-stack hint.** While the pointer is over more than one pickable object, the HUD's prompt says so ("3 objects overlap
   here · click again to select the next", `hud-stack`, which a repeated click shows), so the walk can be found.
3. **Messages opens as the mockup's table.** The Messages pane opens in table mode with Severity, Message, Object,
   Page and Rule on show in that order (fix and state hidden), the tree one toggle away.
4. **Sort state in the header's name.** A sorted header's accessible name adds ", sorted ascending" or ", sorted
   descending" (only the arrow shows it).
5. **History row actions on demand.** Undo to here and Show changes appear on the cursor row, on hover and on focus
   only, as the property row's "…" button (§6.8), instead of on every row.

---

### 6.26 The print pre-dialog

A modal dialog (§6.12) opened by Output ▸
Print… (`file.print`, Ctrl+P, enabled with pages) and by the command bar's Print button. It is titled "Print"
(`print-title`), 0.65 of the window wide (1248 px at 1920, 1664 px at 2560; never under `size.dialog.preview`, 1040 px) and
holds two columns: the options on the left, 36 % of the body, on §6.12's form rows, and the preview on the right, at
most 56 % of the window's height.
- **Options.** *Range*, a radio group: All, Current page, Sections (a check list, "Section {n}: {title}", six rows
  before it scrolls) and Pages (a text field under the radio, enabled only for it; a text the parser refuses shows one sentence in words under the field only, with the field's error border and §6.1's error icon, and the preview shows §6.17's empty state); *Paper*, a choice field: "Sheet size" or a paper size; *Scale to fit* and
  *Monochrome*, check boxes named by their labels. An option changed while the set builds cancels that build and starts
  one for the new options: the build is a job on the snapshot, so the window never waits for it, and the set prints the
  revision it was built at.
- **Preview.** The set's page at the page's aspect ratio, drawn by the placed render of `ew-render-cpu` (the print's
  own rule) at the resolution that fills the preview's pixels (24 to 200 dpi) on a thread of its own, with
  the stamp ("scaled 70 %") visible when the fit scaled the page; an image that no longer matches the build, the page,
  the resolution or the colour mode is dropped. Under it "{n} of {m}" (`print-page-of`) between Previous and Next
  (Secondary buttons). While the set builds, §6.19's progress line, "Building the print set… {n} %", stands in the
  preview's place, and "Writing the print set…" in the build's writing phase; a failed build shows "The print set
  could not be built: {reason}" as a live line; with no page chosen (no section ticked, an empty Pages field) §6.17's
  empty state stands there: "No pages to print", "Tick a section or type the pages to print".
- **Footer**, right-aligned in the platform's order (§6.12): **Print…** the Primary (Enter), *Open in PDF viewer* and
  *Cancel* the Secondaries; both print buttons are disabled until the set is built. Escape and Cancel close the dialog
  and cancel a running build, and nothing prints.
- **After the button.** One toast ends the job: "Sent {0} to the printer", "Opened {0} in your PDF viewer", the
  fallback notice or the failure's text; it names no page count. On Linux the portal is called
  without a parent window and falls back to `lp`, then to the viewer, with one notice; a cancel in the portal ends the
  chain silently. The dialog's own steps are the hidden actions `print.set`, `print.page` and `print.send`, which the
  palette does not list.
- **Known gaps:** the Pages error shows twice and lacks the field's error border and
  icon; the first focus lands on the chosen Range radio and not on the Primary; an A3 preview
  without a fit draws no paper edge.

