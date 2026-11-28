# Design language: the layout

A chapter of [Design language](16a-design-language.md), whose table says where each section lives.

---

## 5. Layout architecture

### 5.1 The window

The window is [the mockup](assets/design-language-mockup.html)'s (open it in a browser; it toggles light and dark and the two densities). In outline:

```text
+-------------------------------------------------------------------------------------------------------+
| [# ElektraWatt v] | [S][U][R] | Project  Page  [Draw]  Check  Output  View    [Q Search commands and devices      Ctrl K] | tab row 32, chrome
|-------------------------------------------------------------------------------------------------------|
| [Select] | [Symbol] [Device] Terminal  | [Wire] Potential    | [Text] Measure | Rotate  Align     [Check project] | command row 52, panel
|          |                   Black box |        Interruption |                | Mirror  Renumber                  |
+----+-----------------------------+----------------------------------------------------------------+----+
| Pg | PROJECT                 ... | Power supply 400 V | * Conveyor motor M1 |                     | Sy |
| Tm | [ Pages     |   Devices   ] |  +----------------------------------------------------------+   | Pt |
| Cb | [ Filter pages            ] |  |  white sheet on the grey surround       [toast: PDF       ]|   |    |
| Po | v =A1 Machine            6  |  |                                          [exported   Open ]|   |    |
| St |     Cover sheet          1  |  |                                                           |   |    |
|    |     Conveyor motor M1 (1) 3 |<-|  selected (accent)                                       + |   |    |
|    |-----------------------------|  |                                                         - |   |    |
|    | PROPERTIES                  |  |     ( [Select] wire symbol text | 1 selected ... )      # |   |    |
|    | -K1                         |  +----------------------------------------------------------+   |    |
|    | Contactor ... · page 3      | Messages (3)  Find  Jobs  History                          x   |    |
|    | v Main                      | Severity  Message                     Object  Page  Rule       |    |
|    |   Tag       [-K1         ]  | (x) Error Connection point ...        -M1     3     ...        |    |
| Hi |   Coil v.   24 V DC   (part)| (!) Warning ...                                                 |    |
+----+-----------------------------+----------------------------------------------------------------+----+
| Ready                       (x) 1 error · 2 warnings  1 selected  3-C2 · X 168.0 · Y 92.0 mm  Grid 4 mm · Snap  125 %  Saved  GPU | 26
+-------------------------------------------------------------------------------------------------------+
```

At 1920 × 1080 and 100 %, Comfortable: command bar 85 px (tab row 32, command row 52 with its top hairline, the
bottom line 1; Compact 69), canvas-tab bar 32 px, status bar 26 px, left rail 110 (its entries named, §5.4) + left
column 480 px, right rail 40 px, so the canvas is 1290 × 937 px: 67 % of the width (ADR-0031 decision 5 requires at
least 65 %; the owner chose the named rail from pictures, 2026-11-28). The bottom dock, when open, takes 350/1000 of that height, at least 300 px (§5.7). At
2560 × 1440 the left column is 640 px and the canvas 1840 × 1297 px (72 %). The sheet is centred on the surround at
up to 92 % of the canvas width.

### 5.2 The command surface

The **command bar** (ADR-0031 decision 3) has two rows: the tab row on `color.surface.chrome` and the command row on
`color.surface.panel`, with a `color.border.subtle` hairline between them and a `color.border.default` line under
the bar.

**The tab row (32 px, `space.3` gutters), left to right:**
1. **App button** (accessible name "Menu", Alt or F10): the product mark (a 16-px `color.accent` square with
   `radius.sm` and the bolt glyph in `color.text.on_accent`), the name "ElektraWatt" in `type.body.strong` and a
   16-px chevron; `size.control.inline` high, `radius.md`, `space.3` padding. It opens the full command tree as a
   menu (§9.2). (A macOS global menu bar is not built: macOS is not a supported platform.)
2. A 1 × 16 px `color.border.default` separator, then **quick access**: Save (Ctrl+S), Undo (Ctrl+Z), Redo (Ctrl+Y /
   Ctrl+Shift+Z) as `size.quick_access` icon buttons with `icon.sm` in `color.text.secondary`, `radius.sm`; then
   another separator. Microsoft's ribbon guidance pairs every ribbon with an application button and a quick-access
   toolbar so file commands and undo never need a tab change.
3. **Tabs**: Project, Page, Draw, PLC, Fluid, Check, Output, View, in `type.body` with `space.4` padding; the active one in
   `type.body.strong` and `color.text.primary` with a 2-px `color.accent` underline inset `space.4` from both ends
   (`radius.mark`), the others `color.text.secondary`; hover shows the hover overlay. One click switches; the tab row
   is a `TabList` with `Tab` children for AccessKit. With no project open the bar shows the
   Project tab (its Templates works there); the tabs that need a project (all but Project and View) stand in
   `color.text.disabled` with their reason as tooltip, a click on one says it as a greyed command does, and the tab
   chosen before comes back with a project.
4. **The workspace switch**, left of the
   command search with a `space.3` gap: a segmented control (§6.20) of Schematic, PLC and Fluid power, and a saved
   workspace by its name while it is the active one; the active workspace is the chosen item, a click loads another.
   For AccessKit it is a radio group (the command tabs hold a tab named PLC). It shows while a project is open, where it fits right of
   the tabs. Saved workspaces, Save layout as and Reset stay in View ▸ Workspaces.
5. **Command search**, right-aligned, `size.search` (280 × 24): `color.surface.panel` fill, a 1-px
   `color.border.default`, `radius.md`, a 16-px search icon, the placeholder "Search commands and devices" in
   `color.text.tertiary` and the key chip "Ctrl K" at its right end. The icon and the placeholder identify the
   field (WCAG 1.4.11). Focusing it opens the command palette (§6.14) anchored under the field.

**The command row (52 px Comfortable, 36 px Compact, `space.3` gutters)** shows the active tab's **groups**, left to
right in task order, separated by a 1-px `color.border.subtle` divider with `space.2` either side; buttons in a
group sit 2 px apart. Group names are not printed (the
Simplified Ribbon's choice); they are the tooltip of the divider region, the submenu names in the app menu and the
breadcrumb in the palette. Inside a group:
- **Large buttons**: `icon.md` over a label in `type.small` with a 2-px gap, 46 px high, at least 52 px wide, `space.icon`
  side padding, `radius.md`, at most two per group, for the commands the user reaches for most (`size = Large` in the
  registry). In Compact they are 28 px high with the icon beside the label.
- **Small buttons**: `icon.sm` and a label in `type.small` side by side with `space.icon` between, 24 px high,
  stacked two to a column in Comfortable, in one row in Compact.
- **Labels in the bar** are the short form of the registry label, without the ellipsis ("Device" for "Device from
  part…", "Renumber" for "Renumber devices…"); the tooltip, the menus and the palette carry the full label.
- **Active and toggled commands** (the running tool, Grid, Snap) show `color.accent.subtle` fill with the icon and
  label in `color.accent`.
- **The tab's next step**: at the right end of the row, after a flexible space, at most one Primary button (§6.2) with
  a 16-px icon: on Draw "Check project" (F7), on Check "Export PDF"; the other tabs have none. It is the one
  accent fill in the chrome and reads as the way forward in the order of work (§9.1).
- **Split buttons** for a command with variants (Mirror ▸ left to right, top to bottom; Check scheme ▾): the main
  part runs the default, the chevron opens a menu.
- A command that cannot run now is drawn disabled with its reason as tooltip; nothing is hidden or reflowed by state,
  so the bar never jumps. A click on a disabled command shows
  its reason as an information toast as well ("No symbols or wires go on a section divider: draw on a schematic
  page"), where the user looks (P3); a second click replaces that toast. A command that is not
  greyed but whose action is refused says why the same way, from the bar, a menu, a key, a row's or a dialog's
  button: the toast, and the status line's state field while the toast shows; in an open dialog the sentence stands
  in `color.error` over its buttons. A refusal the step already shows (the HUD, a form's or a row's line, a toast)
  is not said twice.

**Collapse.** Double-clicking a tab or pressing Ctrl+F1 collapses the bar to the tab row (32 px); a tab click then
shows its command row as a popover over the canvas until a command runs or Escape. **Overflow.** When the window is too
narrow, groups collapse from the right into a "More" split button, each into one button that opens the group as a
menu; the Draw tab's Select and Place groups never collapse, and its Connect group only after every other group
has (at the smallest window in a language of long words). At 1920 × 1080 no group folds, in English, Dutch or the pseudo-locale.
Before any group folds, the tab's next step shows its icon alone (named by its label, its
label and key in the tooltip) where that keeps every group whole; at 1920 that happens only in the pseudo-locale on
Draw.

**The bar is generated from the registry.** `ActionMeta` gains `placement: Option<Placement { tab, group, order,
size, bar_label }>` and `icon: IconId`, and each tab names its next-step action; the bar draws whatever has a
placement, in that order. A registry test asserts that every
placed action has an icon, a label and a docs anchor, and that each tab's groups follow §5.3's order.

**A tool on several tabs keeps its place.** A tool's place is its group (`Placement.group`), and a group a tab shares
with others names them (`Group::shared_with`): it is the first group of its own tab and leads every tab it is shared
with, before that tab's own groups (`Tab::bar_groups`), so each of its buttons stands at the same place on each.
Draw's first group, Select and Wire (the tools every drawn page is worked with: Wire draws bus lines on a bus page and
lines on a fluid page), leads PLC and Fluid as well. A tool never gets a second place of its own on another tab; a
*command* that people look for on two tabs (Report on Page and Output, Enclosure on Project and Draw) keeps its second
place (`ActionMeta::also`), which is no tool and arms nothing. Tests:
`tools_keep_their_place::select_and_wire_stand_at_the_same_place_on_draw_plc_and_fluid` and
`a_shared_group_leads_every_tab_it_stands_on`. PLC workspace and Fluid workspace are small buttons over their Guide
(the workspace switch in the tab row above does the same) and Addresses ▾ a small one, so PLC keeps within 1920 in the
pseudo-locale.

The command bar is as above: a tab row with the app button, quick access (Save, Undo, Redo), the tabs, the workspace switch and the command search; a command row with groups of large, small and split buttons, More and the next step. Ctrl+F1 collapses the bar to 33 px; Alt alone or F10 opens the app menu. The fields of the Interruption point, Text, Wire annotation and Structure box tools (the name, the binding) are a floating **tool options card** at the page view's top left, shown only while one of those tools runs. Each of its text fields commits when it is left changed (Enter, Tab, a click elsewhere or the pointer going back to the sheet), never per keystroke; Escape leaves the field without committing and the tool stays. The command search lists, among the commands, the left rail's panels by their names ("PLC" shows the PLC panel), the pages by folio, title and kind in words ("panel" finds the panel layouts, "door" the door's page; the hit says "Page · Panel layout"; `#` finds among the pages by the same one matcher) and the rows of Project properties by their words and the rows they stand under ("rail fixing"; the hit says "Project properties · Workshop", and Enter shows the project's sheet filtered to the row). Commands with no registry action yet are listed by name and drawn nowhere. F6 cycles command bar, navigator, canvas, Properties (and History while it is shown).

### 5.3 The tabs

| Tab | Groups, left to right (large buttons in **bold**; bar labels) | Next step · notes |
|---|---|---|
| Project | Project: **Properties**, Structure · Parts: **Parts** (the place for parts and symbols, [07](07-place.md) §5.1; F9), Project parts · Standards: Schemes, Company standard · Import: Re-import QElectroTech project… alone (File ▸ Import… takes every file and goes on by what it is; the separate entries Parts table…, Import QElectroTech project…, Import drawing… and Import EPLAN data… are in no menu and no button, the command search finds each by its name) | None · set up once; New, Open and Save are in the app menu and quick access, never duplicated here |
| Page | Pages: **New page**, New section, Page properties, Delete page, **Report** (the Report block… dialog: a report block is put on a page; its second place, beside its first on Output, `ActionMeta::also`) · Frame: **Edit frame** (the frame and title block changed on the page, [09](09-reports-forms-output.md) §3.9), Title block fields… · Order: Renumber pages, Move up, Move down · Go: **Go to** (Ctrl+G), Back (Alt+Left), Forward (Alt+Right), Follow reference (F) | None · folio navigation and page set-up |
| **Draw** (home) | Select and wire: **Select**, **Wire** (W) (shared: it leads PLC and Fluid too, §5.2) · Place: **Symbol** (I), **Device** (Shift+I), Terminal… (its form asks the strip, the kind, how many and from which number), Black box (B), New symbol (Ctrl+I) over Parts in one column (New symbol opens Parts on a new drawing that is saved and placed on this page, [07](07-place.md) §5.6, with a menu for Like the last one and Like the selected one; Parts is the place for parts and symbols, its second place beside Project's, `ActionMeta::also`; the group's two large buttons are Symbol and Device, and the column takes the place the Symbol editor's button had, so no column is added) · Connect: Route, Potential (Shift+P), Interruption (P), Cable (the selected wires as a cable's cores, alone in its column; below 1920 the Layout group folds into its group button first) · Annotate: **Text** (T), Measure, Marker (the Wire annotation, Shift+W) · Arrange: Rotate (R), Mirror (X, Y), Align, Renumber | **Check project** (F7) · the core loop and the tab that opens with a project. Route stands first in Connect. Parts opens the place where a part or a symbol is looked up, changed, made and drawn. New symbol's bar label is whole ("New symbol", "Nieuw symbool"); where the pseudo-locale would fold a group at 1920 × 1080, Parts gives up its second place on Draw (it keeps Project's and F9) before New symbol does. Move, Delete, Wire annotation, Graphic, Image, Structure box, Properties and Change symbol are reached by their keys, the context menu, the app menu's Draw and the palette; Align arrives with its package |
| PLC | Select and wire (Draw's, §5.2) · Station: **Rack** ▾ (a station's rack, in its tab, which carries the commands on a module in a bar of its own), Attach cards ▾ (a station that takes cards) · Addresses: Addresses ▾ (Re-address…, Make manual addresses automatic…, Freeze addresses… of each configuration project), Addresses at terminals, Process-tag scheme, Symbol policy ▾ · Bus: **Bus page** (New page… of type Bus topology), Place port ▾ (the ports the Bus panel lists as not drawn yet; on a bus-topology page), Bus panel · Lists: **PLC grid**, Instrument list ▾ · PLC exchange: Export to PLC software, Import from PLC software · Workspace: PLC workspace (on while it is active; a second press goes back to Schematic), Guide | None · the PLC tools are not hidden in menus. A menu button (▾) lists one entry per station or configuration project; with nothing to list it is greyed with the reason. The settings of one module or one channel (Safety data…, Mode and safety…) stay on their rows |
| Fluid | Select and wire (Draw's, §5.2) · Tubes: **Fluid page** (New page… of type Fluid), **Tube**, Plug ports (the open working ports of the selected valves) · Place: **Fluid symbol** (the device browser at the pneumatic parts, so that what is placed has its part; a bare symbol is Draw ▸ Symbol's), Valve terminal (a terminal of the project drawn on no fluid page first, else the device browser at the valve terminals), Air supply (the device browser at the air supply and its preparation) · Media: **New medium**, Fluid settings, Medium table · Lists: **Tube table**, Generate overview (of the selected valve terminal), Draw plug (of the selected multipin terminal) · Workspace: Fluid workspace, Guide | None · Tube is greyed off a fluid page with the reason; the Place buttons work from any page, on the nearest fluid page or on one they make. A fluid line's medium stays on its context menu |
| Check | Run: **Check project** (F7), Check scheme ▾ · Findings: **Messages**, Next finding (F8), Previous finding (Shift+F8) · Numbering: Freeze tag, Unfreeze tag · Trace: Highlight potential (Ctrl+Shift+H), Clear highlights · Revision: Set author, Approve, **Release**, Revisions (the revision overview), Rework list (its second place, beside Output's; `ActionMeta::also`); the app menu's Check ▸ Revision adds Withdraw revision… and Settle the incomplete release… | **Export PDF** · make it right before output; Renumber lives on Draw (the mockup) |
| Output | PDF: **Export PDF**, Print, Export DXF (a DXF set in a folder, doc 09 §4.9) · Reports: **Report** (the Report block… dialog, where every list page is made), Open generated page · Lists: Parts list, Terminal diagram (open their generated pages) · Workshop: **Wire list** (opens the wire list's first page, else Report block… on that kind; doc 13 §12), Rework list… (what the fitter changes since a release, doc 13 §3.5), …, **Package…** (one file for the workshop, doc 13 §8.5) · Jobs: Jobs | None · everything that leaves the project; the PLC exchange is on the PLC tab |
| View | Zoom: **Fit page** (Home), Zoom in, Zoom out, Actual size · Show: Grid, Snap, Frame statistics, Show tips · Panels: Navigators ▾, Properties, Messages, Jobs, History, Notifications · Layout: Side by side, Workspaces ▾, Reset layout · Appearance: Theme ▾, Density ▾, UI scale ▾ | None · never needed for the work itself |

Commands of later packages join their tab's group when they exist (Macro in Place, Cable in Connect, Waivers in
Findings); a new group, or a new command on Draw, needs a design-language review against the mockup.

**What a new command must satisfy** (read this before adding a button or an action; each point is held by a test,
and learning them from the failures costs an afternoon):

1. *One declaration.* The action is one entry of the action table (`crates/ew-viewmodel/src/actions/table/`) with
   its label in both locale files. The bar, the menu, the palette, the keymap sheet and the reference files follow
   from it: run `cargo xtask gen`.
2. *At most two large buttons per group.* A third command is a small one; small ones stack two to a column in the
   order of their places.
3. *It must fit the main resolution.* At 1920 × 1080, in English, Dutch and the long-text test locale, no group folds
   (`command_surface::nothing_folds_at_1920_in_english_dutch_and_the_pseudo_locale`); in the pseudo-locale the Draw
   tab is full, its next step already in its icon form: a new button there must make room, so say where and look at
   the picture. Below 1920 the groups fold in this order, from the right: Connect last, and Annotate (Text, the Wire
   marker) just before it, so that the panel pages' Layout group gives way first; every command stays
   reachable through its group's menu or More
   (`command_surface::at_1280_by_800_every_command_is_reached_and_the_page_keeps_half_the_window`).
4. *A bar label is settled.* "Renumber devices" stays whole (it was confused with Renumber pages); do not shorten
   a bar label to gain width.
5. *One icon per concept.* An icon key serves one command or pane; two entries share one only when they are one
   concept, with the reason in `crates/ew-ui/tests/it/icons.rs`. A new Tabler icon is the SVG of the pinned release
   in `assets/icons/tabler/`.
6. *The ellipsis tells the truth* (§8 item 3): it asks first, or it has none.
7. *A tool has its section in the guide*: `docs/book/src/reference/window/tools.md`, headed by the tool's English
   label, which is where F1 on the tool opens.
8. *The pictures change.* A new button changes the window's pictures: look at the old and the new one, then bless
   (`EW_BLESS=1 cargo nextest run -p ew-ui -E 'test(snapshots::)'`). The Edit commands Cut, Copy, Paste, Duplicate and Select all
live in the app menu's Edit, the canvas context menu and their keys, as everywhere else.

**View ▸ Show.** The View tab's Show group is as listed, Grid, Frame statistics and Show tips; Snap has no action. The app menu's View ▸ Show is a submenu of the group's switches plus CPU renderer, a setting
that has no button on the tab: Grid, Frame statistics, Show tips, CPU renderer. Wire colours and Mouse wheel scrolls
stay items of View itself, outside the group. The palette shows each switch's state after its breadcrumb ("View ·
Show · On"). Frame statistics is drawn with its icon where its siblings show their check.

### 5.4 Left: the rail, the navigator and Properties

- **The navigator rail** (`size.rail`, `color.surface.chrome`, a 1-px `color.border.default` on its right, `space.3`
  top padding) shows one 32-px icon button (`radius.md`, `icon.md` in `color.text.secondary`, `space.2` apart) per
  entry, in this order: **Project** (Pages and Devices), Terminal strips, Cables, Potentials, Structure, then
  Connections, PLC, Interruption points, Parts in use and Report blocks; **History** sits at the rail's foot and
  opens the bottom dock's History tab. The active entry has `color.accent.subtle` fill, an accent icon and a 2-px
  accent indicator bar (`radius.mark`) 4 px left of the button, inset 8 px top and bottom. Each has an accessible
  name and a tooltip with its key. In a window 1600 points wide and wider the rail is `size.rail_labelled` (110) and
  names each entry under its icon, the button the rail's width less `space.2` on each side: the pane's own name in
  `type.small`, centred, on at most two lines by one rule: words fill a line; a long word breaks where its text holds a
  soft hyphen (the Dutch "Onderbrekings-" / "punten"), the line ending in "-"; a line still too wide is cut with "…",
  whole in the tooltip. A narrower window (1280 × 800 among them, which only has to stay usable) keeps the 40-px rail
  of icons, so its canvas keeps those 70 px. This replaces the overflowing tab strip: every navigator is one click and
  one name away. Clicking the active one collapses the column (the canvas gets the width); Ctrl+1 to Ctrl+9 select by
  position (the keys of `rail.entry`, which a keymap rebinds like any other). Messages is not on the rail; it lives in the bottom dock.
- **The navigator panel** (`size.panel.left`, `color.surface.panel`, a 1-px `color.border.default` on its right): a
  `size.panel_header` header with the entry's name in `type.label` ("PROJECT") and its actions as 24-px icon buttons
  at the right ("More"); for the Project entry a **segmented control** "Pages | Devices" (§6.20) chooses the
  navigator, because the user flips between the two most; then the filter field ("Filter pages", §6.5); then the
  tree (§6.6). No second heading.
- **Properties** is docked under the navigator in the same column, below a 1-px `color.border.default` splitter, and
  takes half the column's height by default (resizable, the split remembered per workspace): its header "PROPERTIES"
  in `type.label`, the object header and the rows (§6.8). It shows the selection, whatever selected it.
- Both remain egui_tiles panes: the rail is a view of the left tab group whose own tab bar is not drawn, so docking,
  the workspace file `ew.workspace/1` and its determinism stay as they are (doc 16 §2.5).

The rails are egui side panels outside the dock, views of the view model's rail state, not a view of the left tab group: the dock's tab bars are hidden through egui_tiles' own switch, so the workspace file does not change with them. The right rail's Symbol browser and Device browser open the symbol and the device browser. The dock tabs' restyle of §6.4 is not built.

### 5.5 Centre: canvas tabs, canvas, HUD

- **Canvas tabs**: a `size.dock_tab_bar` bar on `color.surface.chrome` with a `color.border.default` line under it
  and `space.3` gutters; tabs 28 px high with `space.4` padding and `radius.md` top corners, the page title in
  `type.small` (the folio and full path in the tooltip). Inactive tabs are text in `color.text.secondary` with no box;
  the active tab takes `canvas.surround` fill, a 1-px `color.border.default` on its top and sides and none below, so
  it opens into the canvas, with its title in `type.small` SemiBold and `color.text.primary`. An unsaved page shows
  a 6-px `color.accent` dot before its title. Badges are icons with names (generated: `sparkles`; QET grid: a
  "QET" caption badge); the close button shows on the active and hovered tab (and is Ctrl+W for all); "+" (New
  tab, Ctrl+T) at the end. No "Page view" heading above them.
- **The canvas**: the white sheet with `elevation.1`, centred on `canvas.surround` at up to 92 % of the canvas
  width; overlays per §7.
- **The tool HUD** is always shown: a pill (`radius.pill`, opaque `color.surface.inverse`, `elevation.2`, 4-px
  padding) centred in the band at the canvas's foot (the pill with `space.2` above and below it), floating over
  the canvas with nothing behind it but its own pill, so the drawing shows up to its edge. It holds the active tool as a 28-px pill button with
  `icon.sm` and its name (`color.accent` fill, `color.text.on_accent`, SemiBold), three quick tools as 28-px icon
  pills (Wire, Symbol, Text), a 1 × 16 px divider and the **prompt** in `type.small` at 80 % opacity: the selection
  and its keys with Select ("1 selected · Del deletes · R rotates"), the tool's target and keys otherwise
  ("Place -X1031:21 · R rotates · V variant · Esc done"; a terminal of a strip without a tag yet in words, "terminal 1
  on the new strip (no tag yet)", until the preview predicts its tag). It never takes focus and it stays
  where it is, so its buttons can be reached; a fit leaves the sheet's margin under it. Its buttons
  show the hover and the press as a light overlay of `color.text.on_inverse` (14 %, 26 %) on the dark pill, where
  `color.state.hover` does not show, and a quick tool that is the active tool is filled at 26 %. After a Wire click that placed a bend instead of ending on a
  connection point its prompt is "Bend placed: click a connection point to end the wire." It is the floating tool window of the
  owner's reference, reduced to what the running tool needs; the hint strip under the menu is removed.
- **The zoom control**: at the canvas's bottom right (`space.4` from the right, `space.5` from the bottom), a
  vertical stack of three 28-px icon buttons (Zoom in, Zoom out, Fit page) on `color.surface.raised`, `radius.md`,
  `elevation.1`.
- **Toasts** appear at the canvas's top right (§6.10).
- **Scroll bars**: along the canvas's right and bottom edges, `space.3` thick, a thumb in
  `color.text.tertiary` (`color.text.secondary` under the pointer and while dragged) with `radius.pill`, standing for
  the part of the page in view against the page and the room beyond it the view shows; shown only while part of the
  page is out of view (P3: a fitted page has none), and a thumb dragged pans the view as far. While the horizontal bar
  shows, the tool HUD's band stands on it (raised by the bar's `space.3`), so the bar runs whole under the pill and
  its thumb can be seen and dragged anywhere.
- **The wheel's choice where the zoom is first met**: the plain mouse wheel zooms; its first zoom of a page
  shows the one-time tip "The mouse wheel zooms the page. It can scroll the page instead, Ctrl with the wheel then
  zooms (View ▸ Mouse wheel scrolls)." with the action Wheel scrolls the page, which turns the setting on. It is a tip
  of FR-UI-048 (a toast with one action, once, off with View ▸ Show tips), shown to everyone, not only to a person
  who comes from EPLAN or QET; a zoom by a button or a key shows none.

The HUD is one status node holding the zoom control, **centred on the page-view pane** (not on the canvas sheet), on the inverse surface (light in dark mode, as specified). The Place prompt reads "`<target>` · R rotates · V variant · Esc done"; a repeated click on a pick stack reads "`<n>` objects overlap here · click again to select the next" (`hud-stack`). The tool strip's fields are on the tool options card (§5.2).

**The double-click rule:** the window turns a double click's second press into `tool.finish` only for a tool that finishes on a double click (`Tool::finishes_on_double_click`): Wire, Select (a free text opens for editing; the title block or the column numbers or row letters open Edit frame), the polyline and polygon graphics and Structure box. Every other tool sees two plain presses, so two quick clicks with Place place twice (for the stacked twin). A right-click on the canvas opens a context menu: over an object the selection's Arrange commands in the command bar's order, then Delete; over nothing Paste when it is enabled; no menu when there is nothing to offer. The Black box tool marks its first corner, and after an Escape its hint says that a second Escape leaves the tool.

**The tool options card takes the pointer only with its controls (the band they stand in):** a click on its frame reaches the sheet beneath, and while the tool's ghost stands under the card (by the card's rectangle of the last frame) the card is drawn at 40 % opacity and is no obstacle to the canvas chips. **It is one row high and stands right under the page view's tab bar** (`space.1` below it, `space.2` padding, no title: each field carries its own label), so a fitted page's margin and its frame's band of column numbers hold it and it covers none of the drawing, at 1920 × 1080 and at 2560 × 1440; on a page zoomed in it fades as above.

**A jump's
arrival.** After a cross-reference jump the target is centred and marked by a ring of `size.control.inline` radius in
`color.accent`, which fades over 1.5 s of frame time; under reduced motion it stays fully drawn for 1.5 s, then goes. A jump keeps the view on
the sheet: when centring the target would show off-sheet space, the view stops at the sheet's edge and the target sits
off centre, still marked; a sheet smaller than the pane is centred in it. The ring is drawn under the page's texts, so
it never covers one. This applies to a jump and the link preview, not to panning or fitting. **Back beside the page tabs.** Each canvas tab bar starts with a
Back button, enabled only when there is a page to go back to; its tooltip takes the key from the
keymap, "Back (Alt+Left)", and when disabled reads "Back (Alt+Left): No page to return to". **A page's first view** is
the whole sheet (fit-page), so that the title block is in view, unless a fit to the width is clearly larger (by more
than 15 %: a wide, low pane), which is then taken with the top of the page 16 pt below the pane's top, so a drawing frame's
pin texts are legible in a small window; it stays the first view of a new pane until Fit page is
pressed, and fit-page stays one key away (Home). A new project's pages open with the whole sheet in
view, at 1920 × 1080 and at 2560 × 1440.

### 5.6 Right: the palettes

A right rail (`size.rail`) with **Symbols** and **Parts**; the right column is closed by default and opens to
`size.panel.right` on a rail click (or View ▸ Panels), staying open until closed. **Symbols** shows the library's
symbols as a filterable grid of 72-px thumbnails with names; a click arms Place with that symbol, a drag drops it.
**Parts** shows the project's parts with a thumbnail and a state count, the reference's idea in our domain: "2 of 4
placed" in `color.text.secondary`, turning to `color.success` with a check icon when every function of the part is
placed, `color.warning` with "unplaced" when the part has functions not on any page. The Insert ▸ Symbol picker (I)
stays as the keyboard path; both open the same Place tool.

The right column is not built: the right rail is a nearly empty column at 1920, and
`size.panel.right` is not in the theme, because nothing reads it.

The part picker's rows lead with the part's description in words, not its code (`CataloguePart.description`); a disabled row says why on hover (`disabled_tip`), as the command bar's disabled tools already did; a change-over contact reads "changeover contact 11-12-14" by a display mapping.

**The device and symbol browser** replaces the flat picker list for Insert symbol, Insert device,
Change symbol, Insert black box and the strip chooser: one overlay over the `color.backdrop` scrim, the window less
`space.6` on each side and most of its height, `elevation.2`, `radius.lg`, `space.5` padding; the title in
`type.title` with the close button at the right, the filter field, then three columns. **Left, the tree:** All, In
this project, the trades Electrical, PLC and bus, Pneumatics, Enclosure and mounting (F28: each with the
chevron of a tree node, opening to its sub-groups one `size.tree.indent` in; one table, `trade_of`, files a category
under its trade), the sub-groups one node per function category (the first level of a symbol's FnDef category, of a
part's own category or its main function's, else the group its kind gives; the words are `function-category-<key>`
texts, a category without words or a trade falls under Other), Other, each with its count of hits under the filter. A
trade opens when it or one of its groups is chosen, a second click closes it; under a filter every trade stands open,
and a chosen node without a hit gives way to All. **Middle, the table** of §6.7 (`type.small`, `size.row`, the
header on `color.surface.sunken`): name, part number, ratings and library, a greyed row's reason in the last column;
one row per part, which opens to its functions with a chevron (Left and Right in the table), and whose own row places
its main function. **Right, what will be placed:** the symbol's outline on the white sheet with its connection
points, its name, part number and ratings, its connection points' designations, for a part its card (F22: type
and order number, its technical data as label and value rows, its size with its outline to scale over a 35 mm rail,
what it gives, where it comes from; the same card on hover over a Devices row and behind a device sheet's Part
details link), a greyed row's reason, and the Primary Place. A click makes a row or node current (the Selected state with the 2-px accent bar), a double click or Enter
places; Tab goes between the filter, the tree and the table. The picture is read through the shape cache, once per
symbol and revision.

### 5.7 Bottom: Messages, Find, Jobs, History, Preview

One tab group below the canvas, **closed by default**. It opens when the user asks: the status-bar
findings field or F8 opens Messages, the job field opens Jobs, Ctrl+F opens Find, a bulk preview opens Preview (the
user asked for it), Ctrl+H or the rail's History button opens History. It opens at 350/1000 of the page view's vertical split (the layout's 650:350), and the user's
splitter shares are then kept in the workspace; on show it is at least `size.dock.bottom_min` (300) high, but never more
than two thirds of that split, a display-only clamp that never rewrites the shares. It lies on
`color.surface.panel` with a 1-px `color.border.default` above it. Its tab row is 32 px with a `color.border.subtle`
line under it: tabs in `type.small` with 10-px padding, the active one SemiBold and `color.text.primary` with a 2-px
accent underline inset 10 px, the others `color.text.secondary`; a tab with items carries a count chip (§6.16), the
Preview tab a 6-px accent dot while a card waits (§6.23); one
close button (Ctrl+J) sits at the row's right end. The **Messages** table follows §6.7 with the columns Severity
(110 px; the semantic icon and word in SemiBold), Message, Object (120 px, a link in `color.accent` that selects and
shows the object), Page (80 px) and Rule (90 px), the last two with tabular digits in `color.text.secondary`. It
closes with its close button or Escape in an empty pane, and remembers its height.

Messages is `NavKind::Messages` on the navigator framework: a tree of
severity groups, rules and findings (§6.24). Its rows carry the columns severity, rule, message, object, page and
cell, fix and state, but **the pane has no table mode** (`navigator.table` refuses it and no Table
toggle shows). The mockup's table, a data grid (§6.21) with the mockup's five columns on show and opening first, is
not built. The other bottom panes: Find (table
rows), Jobs (§6.19 and §6.25), History (§6.24) and Preview (§6.23).

**Close.** A bottom pane alone in the dock, without the tab row, carries the dock's close button at its header's right end; the command that opens Messages closes it again (`messages.show` without `on` toggles, as History, Preview and Jobs do), and Escape with the focus in Messages closes it once a tool at work has had its Escape. **The canvas keeps its view.** Opening, closing or dragging a dock keeps each canvas tab's zoom and centre; a fit happens only on the host's first size of a canvas group, a page change or an explicit fit. The trade-off: with the Preview dock open the kept zoom crops the sheet's top row and title block, where the old refit showed the whole page; whether a fitted, untouched view should refit when a dock opens is not settled. **The clamp.** The display clamp above also leaves the page view at least half the window's client height, so in a short window `size.dock.bottom_min` gives way to it.

**The dock grows with its content.** Messages, Find, Preview,
Glossary, History and Jobs report their list's height; the dock is shown no higher than the shown pane needs and,
while the split keeps its default share (350/1000), grows past the page view's half to at most 40 % of the window's
height when the content needs it: in a 1280 × 800 window Messages shows six rows without its detail block, four with
it, and more at 1920 × 1080. A share the user dragged stands.

### 5.8 The status bar

`size.status` (26 px) on `color.surface.chrome`, `type.small` with tabular digits, separated from the content by a
1-px `color.border.default` line, `space.3` gutters. Each field is 22 px high with `space.3` padding and `radius.sm`,
text in `color.text.secondary` with an optional 16-px icon `space.icon` before it, and the hover overlay when it
is clickable. Left: the **state** ("Ready", or the running command's name), then the **revision**, while the project
has one: the `versions` icon and where the revision stands (`Session::revision_standing`, the one reading the package
and its Contents take too), "V1.0 released 20-11-2026" ("V1.0 released" where the release records no day), "V2.0 open ·
3 pages changed since V1.0" (the pages of doc 15 §5.6's `changed_now`), "V1.0 open" for a project never released,
"V1.0 approved" while its sign-off holds for the saved content (an edit makes it open again), or "V1.0 · a state of a
newer version" for a state this version does not know; a click opens the
revision overview (Check ▸ Revision ▸ Revisions). At 1920 × 1080 it shows whole; in a window too narrow for every
field's words it shows its short words, "V2.0 open", the whole in its tooltip. The tool prompt is in the HUD (§5.5).
Right, in this order:

| Field | Shown | Example | Click |
|---|---|---|---|
| Waiting card | while a card waits in the Preview tab | the `eye` icon and "1 change waits for Apply" in `color.text.primary` | shows the Preview tab |
| Findings | when any count is non-zero | the worst severity's icon and "1 error · 2 warnings", SemiBold in that severity's colour | opens Messages |
| Job | 200 ms after a job starts | "PDF export 40 %" with a 2-px progress line | opens Jobs |
| Selection | when not empty (FR-UI-034) | "1 selected" | — |
| Cursor | pointer over the canvas or grid cursor shown | "3-C2 · X 168.0 · Y 92.0 mm" (the cell first, FR-UI-034) | Go to (Ctrl+G) |
| Grid and snap | always | "Grid 4 mm · Snap" / "QET grid 10 px" / "Grid off" | View ▸ Grid |
| Zoom | always | "125 %" | Fit page |
| Save state | always | "Saved" in `color.success` with a check icon / "Unsaved changes" in `color.text.primary` with a dot / "Read-only" in `color.warning` | — |
| Renderer | always (FR-UI-034, NFR-PLAT-03) | "GPU" in `color.text.tertiary` / "CPU" or "WARP" in `color.warning`, the tooltip says why | About |
| Notifications | when the log is not empty | bell icon with "2" | opens the log |

The page field leaves the bar: the active canvas tab and the selected navigator row already show the page (P3).

The language field leaves the status bar (it stays on the start page and in Settings): a setting that changes once a
year does not belong in the busiest strip of the window.

The save-state field shows exactly the three texts above: "Saved", "Unsaved changes" and "Read-only".

The findings field, the check toast and the Messages tab's chip count by one rule, `MessageCounts::open`: the project's open errors, warnings and infos, without the waived and the pending ones (the completeness findings of the page being edited); the field adds "· N pending on this page" when there are any. The Properties chip stays scoped to its record or page.

After Check project the findings field shows the last check's counts while the project is at the revision it checked, also with Messages closed, and its click opens Messages; an edit hides it until the pane or a new check counts again.

### 5.9 Focus order

F6 and Shift+F6 cycle: command bar → left rail → navigator → Properties → canvas group(s) → right column (when open)
→ bottom dock (when open) → status bar (doc 16 §10.2, with the command bar first). Within the command bar, Left and
Right move between tabs, Tab moves into the command row, arrow keys move between commands.

### 5.10 Narrow windows

The minimum window is 1024 × 640. A window smaller than 1920 × 1080 only has to stay usable (§2): below 1280 px of
width the right column overlays the canvas instead of pushing it; where the command row is too narrow its groups
collapse from the right (§5.2; on the Draw tab in English below about 1450 points); the left column never shrinks below
220 px and collapses to its rail by a click on the active rail button.

