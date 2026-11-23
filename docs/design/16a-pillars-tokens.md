# Design language: the user, the pillars and the tokens

A chapter of [Design language](16a-design-language.md), whose table says where each section lives.

---

## 1. Why a design language

The window checks that preceded this document (screenshots named by file in §10)
show a window that works and looks unfinished: egui's default dark theme, eight menus and no toolbar, a hint strip under
the menu that repeats the status bar, pane titles that repeat their own tabs, a property grid where every row says
"= authored" and carries a clipped "edit" button beside internal fields (`$v`, keys, UUIDs), navigator rows with
"[QET grid]" in their text, a tab strip that hides navigators behind an arrow, dialogs whose buttons all weigh the
same, chips that collide on the canvas and highlights too faint on the white sheet. Each was a reasonable local
choice; together they are noise. The owner's brief: "Sleek, Clear, Minimalist, the eye being naturally drawn to
important element, logical menu progression." A design language turns those words into rules a lane can follow and a
test can check, so the next forty panels do not each invent their own look.

**What it governs.** Every surface drawn by `ew-ui`: chrome, panels, dialogs, overlays on the canvas, icons and the
words on them. **What it does not touch.** The sheet itself: page content renders print-true in every theme
(FR-UI-043, doc 16 §11.2); drawing fonts stay ADR-0028's; the PDF is unaffected.

**The reference the owner liked** (a CAM nesting application) works for its users because: commands sit in labelled
groups in task order on tabs, with large buttons for the frequent ones; a structure tree at top left with a property
grid under it; a visual palette at the right whose state colours and counts ("12 of 12", green when placed) answer
"what is left to do" at a glance; the canvas dominates with labelled sheets; a floating tool window serves the running
tool; and the status bar is quiet. EW keeps those principles and none of its branding or icons.

---

## 2. The user and the job

| Who | Comes from | Wants from the window |
|---|---|---|
| Panel-builder engineer (the pilot) | QElectroTech today, EPLAN at customers | Draw folios 101 to 103 and export the PDF without help; see at a glance what is unfinished |
| EPLAN user | A ribbon since EPLAN 2022, navigators at left, properties dialogs | Commands where a ribbon would put them; EPLAN words (device, function, interruption point) |
| QET user | Menus, toolbars, an element panel at left | A familiar left panel; drag symbols from a palette |
| New user | Office, browsers | Labels on buttons, one obvious next step, no jargon in the chrome |

**The core loop** is: pick a page → place a symbol → wire it → fix its properties → check → export. It repeats
hundreds of times per project; the rest (project set-up, numbering, reports) happens a few times. The language is
built around the loop: it lives on one tab, its panels are open by default, and everything else is one click or one
key away. **Screens:** 1920 × 1080 is the main resolution: the window is designed for it and judged there, for look and
hierarchy and for nothing truncated or overlapping. 2560 × 1440 is its extension: the extra space goes to the work
(the page, a list's rows, a dialog's content), never to empty chrome beside a capped width. A smaller window only
has to stay usable, every control reachable by folding, wrapping or scrolling; no layout is designed for it and none
is shaped by what fits there (the owner, 2026-11-13). Remote desktop sessions are common (no reliance on animation or subtle gradients; NFR-PLAT-03). A first start
opens the window maximised; a remembered window is restored. The offscreen driver renders 1920 × 1080 at one pixel
per point by default (`--window 2560x1440 --ppp 1` for the extension); the gated snapshots are `main-*` at 1920 ×
1080 and `*-2560` at 2560 × 1440.

**Widths at the main resolution and its extension.** Every width is set at 1920 × 1080 and
grows on to 2560 × 1440, where the extra room goes to the page, to more rows and to wider lists; a token is a width's
least value in a small window.

| Width | 1920 × 1080 | 2560 × 1440 | Rule |
|---|---|---|---|
| Left column (navigator, Properties) | 480 | 640 | 480 + 0.35 × (window − 1920), up to `size.panel.left_max` (640, from about 2380), while the user has not dragged it; below 1920 it narrows the same way until the layout's own share (280 of the 1200 points a 1280 window leaves between the rails) stands. Display only: the user's share stays the layout, like the bottom dock's clamp (§5.7). The strip editor's and a table's lack widens it further, up to two thirds of the dock |
| Command search | 384 | 512 | 0.2 of the tab row, never under `size.search` (280); no cap beside the tab row's empty middle |
| Form dialog of fields (New page…, Terminal…) | 864 | 864 | 0.45 of the window from `size.dialog.form` (640), stopping at its 1920 width: short values gain nothing from wider fields |
| Form or dialog that holds a list (Choose part, the parts mapping card, Keyboard shortcuts, Notices) | 864 | 1152 | 0.45 of the window from `size.dialog.form`; a list dialog fits its content up to that width (the mapping card), a short list keeps the token width |
| Small list dialog (Show changes) | 576 | 768 | 0.3 of the window from `size.dialog.small`, fitting its lines |
| Print pre-dialog | 1248 | 1664 | 0.65 of the window from `size.dialog.preview` (1040); the preview takes the gain |
| Command palette | 768 | 1024 | 0.4 of the window from `size.palette` (560) |
| Toast | 422 | 563 | 0.22 of the window from `size.toast` (300) |

The navigator table re-fills its capped columns on a resize. **Exceptions, fixed on purpose:** the junction editor
and the parts card keep `size.dialog.small`; the first-run screen keeps the reading measure `size.text_measure`
(520), a line length that does not grow with the window. A right-click menu (the canvas's, a navigator row's) is as
wide as its widest entry: the icon column, the label, `space.6` and the shortcut, no label wrapping and a greyed
entry's reason wrapping within it. What
remains of the look at 1920 (the ribbon's and the menu bar's empty middle, the right rail) is not solved.

**The macro card's value fields** follow a form dialog, not
the preview's width: the label column is as wide as the widest label, at most half the row; the field is as wide as
a form dialog's field (`dialog::form_width` less its margins, growing with the window, never wider than the panel);
a refusal sits right under its field (about 735 px of field at 1920). The Macros navigator's
thumbnail gives up height before its detail card clips.

**The keyboard-shortcuts list** (Help ▸ Keyboard shortcuts) is a
list dialog that fits its content up to half the window's height, in whole rows: a row is `max(size.row,
size.control)` high (28 px) with no gap, so 19 rows show at 1920 and 25 at 2560; mid-scroll positions are not
snapped to whole rows. Of the columns' room the keys column takes at most 25 % and the "where" column at most
35 %, each fitting its widest content below that; the action column takes the rest, and a cut action title shows its
whole text on hover.

---

## 3. Pillars

Ten pillars: the owner's five, expanded, and five that complete them. Each has rules a reviewer can check; "Do" and
"Don't" are written against things the current window does.

### P1 Calm by default (sleek)

The chrome recedes and the drawing leads. Neutral surfaces, hairline structure, no decoration.
- **Do** use the neutral ramp for every surface and border; let the white sheet on its grey surround be the brightest
  thing on screen. Separate regions with one 1-px border or a change of surface, never both plus a shadow.
- **Do** keep chrome flat (elevation 0); only floating things (menus, popovers, dialogs, toasts, canvas chips) cast a
  shadow.
- **Don't** use colour for structure (coloured headers, coloured panels, gradients), and don't draw boxes around
  groups that spacing already separates ("structure should be felt, not seen").

### P2 Clear

Every element says what it is with one name, in the user's words, and shows its state.
- **Do** give each pane one title, in its tab. Show values in the user's vocabulary: "Generated", not `[generated]`;
  "-X1031 : 21", not `#21` in the chrome.
- **Do** show state with icon + word + colour together (FR-UI-025): a finding is an icon and "Error", never a red tint
  alone.
- **Don't** expose implementation (`$v`, `pages|103`, UUIDs, `ew.iec.terminal@qet10`) outside the Advanced section
  and the scripting views.

### P3 Minimal

Every element earns its place by being used or by preventing a mistake. Show exceptions, not normal states.
- **Do** show a status only when it departs from the normal: provenance chips only on values that are not authored;
  the renderer field calm on the GPU and in warning colour on the CPU or WARP; findings counts only when non-zero.
- **Do** hide a command that cannot run in this build (a palette entry with "Parameter forms come with a later
  version" is noise); disable a command that cannot run *now*, with its reason.
- **Don't** repeat a fact in two places on the same screen (hint strip and status bar; pane heading and tab; a page
  folio in the tab, the status bar and the window title all at once is the limit).

### P4 The eye goes to what matters

One accent, one primary action per surface, a contrast budget.
- **Do** spend the accent on four things only: the primary action, keyboard focus, the selected or active state (in
  lists, on the rail, the active tool and on the canvas) and links; plus three marks, the product mark in the app
  button, the 6-px unsaved dot on a canvas tab and the 6-px dot on the Preview tab while a card waits (§6.23). Selection uses the same blue in the navigator, the Properties
  header and the canvas, so the eye joins them.
- **Do** keep each view to at most three type sizes and three contrast levels (text primary, secondary, tertiary);
  NN/g: "If everything is contrasted, then nothing stands out."
- **Do** give each dialog, card and banner exactly one filled (primary) button, and the command row at most one: the
  tab's next step at its right end (§5.2); the others are secondary or subtle.
- **Don't** fill hovered menu items with saturated accent; hover is a neutral overlay.

### P5 Task-ordered commands (logical progression)

Commands appear in the order of the work, left to right and top to bottom.
- **Do** order the command tabs Project → Page → Draw → Check → Output, then View (§9); inside Draw, the groups run
  Select → Place → Connect → Annotate → Arrange.
- **Do** keep the core loop on one tab (Draw), so placing, wiring and rotating never change tabs (Microsoft: "Users
  should spend most of their time on the Home tab").
- **Don't** move commands with the selection (no contextual tabs); disable what does not apply.

### P6 Progressive disclosure, two levels

Show the frequent; keep the rest one step away; never more than two levels.
- **Do** show the main properties of a selection first (tag, function text, part, location) and collapse the other
  categories; put internal fields under **Advanced**, off by default.
- **Do** open the bottom dock (Messages, Jobs, History) on demand from its status-bar field, the rail's History
  button or its key; keep it closed by default so the canvas has the height.
- **Don't** nest a third level (a dialog from a dialog from a menu); NN/g: designs beyond two disclosure levels
  "typically have low usability".

### P7 Consistent

One component per job, one token source, one name per thing.
- **Do** build every list row from the tree-row, table-row or property-row component (§6); never style a list locally.
- **Do** take every colour, size and radius from the token module; a literal in widget code fails review.
- **Do** use the same word for a command in the bar, the menu, the palette, the tooltip and the help.
- **Don't** introduce a second button style, a second chip style or a second dialog layout.

### P8 Keyboard-first, pointer-complete

Every action has a keyboard path; the frequent ones a key (NFR-A11Y-01, doc 16 §4.1).
- **Do** show the key in every tooltip and menu item; let the command search (Ctrl+K) reach every enabled action.
- **Do** draw a visible focus ring (2 px accent) on every focusable element; F6 cycles regions (§5.9).
- **Don't** make a hover the only way to reveal an action (a close button that appears on hover is also reachable by
  Ctrl+W and the context menu).

### P9 Accessible by construction

Contrast, names and patterns are part of the token and component definitions, not a later pass.
- **Do** meet 4.5:1 for text and 3:1 for icons, component boundaries and focus rings in light and dark, and 7:1 for
  text in high contrast (NFR-A11Y-04, WCAG 2.1 SC 1.4.3, 1.4.11); overlays reach 3:1 on the white sheet
  (NFR-A11Y-03). The token test enforces it.
- **Do** give every icon-only control an accessible name and a tooltip, from the registry label.
- **Don't** carry meaning by colour alone, and don't put text under 11 px at 100 % scale.

### P10 Stable and honest

The window does not move under the user, and it never pretends.
- **Do** keep layouts fixed while working: no panel opens, closes or switches tab by itself, nothing steals focus
  (doc 16 §4.1 item 7), toasts appear where they always appear.
- **Do** say what a job, a check or a preview is based on ("as of rev 3"), and say when something is stale.
- **Don't** animate for decoration; motion only confirms a change (§4.12), and Reduce motion turns it off.

---

## 4. Tokens

### 4.1 Structure

Three layers, as in Fluent 2 and Material 3:
1. **Global tokens** are raw values with no meaning: the ramps of §4.2 to §4.4 (`slate-600`, `volt-600`). Widget code
   never names them.
2. **Alias tokens** carry meaning and have one value per theme: `color.text.secondary`, `color.surface.panel`,
   `space.3`, `radius.md`. Widgets use these.
3. **Component tokens** are the few values a component needs beyond aliases: `size.command.large`,
   `size.quick_access`. They are defined in terms of aliases where possible.

In `ew-ui` the tokens are one module (`theme/tokens.rs`): a `Tokens` struct per theme built from the ramps, applied to
egui's `Style` by `theme::apply` (§4.14) and passed to the canvas host for overlay colours. `tests/expected/tokens.txt`
is the blessed dump of every alias in every theme with its contrast against its declared background; a change to a
token is a reviewed diff of that file. Names in this document are the dump's names.

### 4.2 Neutral ramp: Slate

A cool grey (hue about 215°, very low chroma): crisp on screens and quiet next to the black-on-white drawing.
Linear's 2026 refresh moved to a warmer grey; ours stays cool so that the white sheet, not the chrome, reads as paper.

| Token | Hex | Token | Hex |
|---|---|---|---|
| `slate-0` | #FFFFFF | `slate-400` | #868E99 |
| `slate-25` | #FAFBFC | `slate-450` | #959DA8 |
| `slate-50` | #F4F5F7 | `slate-500` | #646C78 |
| `slate-100` | #ECEEF1 | `slate-600` | #555C66 |
| `slate-150` | #E3E6EA | `slate-700` | #3F454D |
| `slate-200` | #D5D9DF | `slate-750` | #343941 |
| `slate-300` | #BBC1C9 | `slate-800` | #2A2E34 |
| `slate-350` | #A7AEB8 | `slate-850` | #212429 |
|  |  | `slate-900` | #1A1D21 |
|  |  | `slate-950` | #121417 |

### 4.3 Accent ramp: Volt blue

One brand accent. Blue because it is the colour of selection and links on every platform, stays distinguishable under
the three common colour-vision deficiencies, and sits apart from the four highlight hues and the three severity hues.
`volt-600` is a little brighter and more saturated than Okabe–Ito blue #0072B2 (5.19:1), at 5.69:1 on white.

| Token | Hex | Use |
|---|---|---|
| `volt-50` | #E7F0FC | Selected row fill (light) |
| `volt-500` | #4A90EA | Pressed accent (dark) |
| `volt-600` | #0B63CE | **The accent** (light); canvas selection in both themes |
| `volt-700` | #0A58B8 | Hover on accent (light) |
| `volt-800` | #094C9F | Pressed on accent (light) |
| `volt-300` | #5AA2FF | **The accent** (dark) |
| `volt-250` | #77B3FF | Hover on accent (dark) |
| `volt-950` | #0B1A33 | Text on the dark accent |
| `volt-900` | #203754 | Selected row fill (dark) |

### 4.4 Semantic ramps

| Meaning | Light text and icon | Light subtle fill | Dark text and icon | Dark subtle fill |
|---|---|---|---|---|
| Error | #C42B3C | #FDECEE | #FF8A8A | #3A2226 |
| Warning | #9A5B00 (icon #B26A00) | #FFF4E0 | #F5B544 | #3A2E17 |
| Success | #17733F | #E8F5EE | #47C97E | #1D3326 |
| Info | the accent | `volt-50` | the accent | `volt-900` |

Severity is never colour alone: error is a circled cross and "Error", warning a triangle and "Warning", info a
circled "i" and "Info" (doc 16 §10.3). Success appears only for outcomes ("Done", "placed").

### 4.5 Alias colour tokens

Contrast is WCAG 2.1 relative luminance against the background in the "On" column; the token test recomputes every
row. **Text** needs 4.5:1, **non-text** (icons, boundaries, focus) 3:1.

| Alias | Light | Dark | On | Light ratio | Dark ratio | Use |
|---|---|---|---|---|---|---|
| `color.surface.chrome` | `slate-50` #F4F5F7 | `slate-900` #1A1D21 | — | — | — | The command bar's tab row, rails, canvas-tab bar, status bar |
| `color.surface.panel` | `slate-0` #FFFFFF | `slate-850` #212429 | — | — | — | The command row, docked panels, the command search field |
| `color.surface.raised` | `slate-0` #FFFFFF | `slate-800` #2A2E34 | — | — | — | Menus, popovers, dialogs, toasts, palette |
| `color.surface.sunken` | `slate-50` #F4F5F7 | `slate-900` #1A1D21 | — | — | — | Table headers, the segmented control's track, the panel's count badges (the dock-tab counts use `color.badge.on_chrome`, §4.15) |
| `color.surface.inverse` | `slate-900` #1A1D21 | `slate-100` #ECEEF1 | — | — | — | Tooltips, canvas chips, the tool HUD (opaque) |
| `color.text.primary` | `slate-900` #1A1D21 | `slate-100` #ECEEF1 | panel | 16.9 | 13.4 | Values, labels of rows, body |
| `color.text.secondary` | `slate-600` #555C66 | `slate-350` #A7AEB8 | panel | 6.75 | 7.0 | Property labels, meta, shortcuts |
| `color.text.tertiary` | `slate-500` #646C78 | `slate-450` #959DA8 | chrome | 4.86 | 6.2 (5.0 on raised) | Placeholders, folio column, captions |
| `color.text.disabled` | `slate-350` #A7AEB8 | `slate-600` #555C66 | — | exempt | exempt | Disabled controls (WCAG exempts them) |
| `color.text.on_accent` | #FFFFFF | `volt-950` #0B1A33 | accent | 5.69 | 6.65 | Primary button label |
| `color.text.on_inverse` | `slate-100` #ECEEF1 | `slate-900` #1A1D21 | inverse | 14.6 | 14.6 | Tooltip and chip text |
| `color.accent` | `volt-600` #0B63CE | `volt-300` #5AA2FF | panel | 5.69 | 6.0 | Primary fill, focus ring, links, selected marks |
| `color.accent.hover` | `volt-700` #0A58B8 | `volt-250` #77B3FF | — | — | — | Primary hover |
| `color.accent.subtle` | `volt-50` #E7F0FC | `volt-900` #203754 | — | — | — | Selected row and tab fill |
| `color.border.subtle` | `slate-150` #E3E6EA | `slate-750` #343941 | — | — | — | Dividers inside a panel |
| `color.border.default` | `slate-200` #D5D9DF | `slate-700` #3F454D | — | — | — | Panel edges, raised-surface outline |
| `color.border.control` | `slate-400` #868E99 | `slate-400` #868E99 | panel | 3.31 | 4.7 | Text-field and checkbox boundary |
| `color.focus` | = `color.accent` | = `color.accent` | panel | 5.69 | 6.0 | 2-px focus ring |
| `color.state.hover` | `slate-900` at 6 % | #FFFFFF at 7 % | — | — | — | Hover overlay on any row or subtle button |
| `color.state.pressed` | `slate-900` at 10 % | #FFFFFF at 12 % | — | — | — | Pressed overlay |
| `color.error` … `color.success` | §4.4 | §4.4 | panel | 5.59, 5.43, 5.90 | 6.9, 8.6, 7.4 | Findings, outcomes |

Every value in both columns is recomputed by the token test; a token that fails is changed in the
dump, not exempted. Selected rows keep their text readable: primary text on `color.accent.subtle` is 14.7:1 (light)
and 10.4:1 (dark); the accent label of an active command on `color.accent.subtle` is 4.95:1 and 4.64:1. Other pairs
the mockup uses: secondary text on chrome 6.19:1, warning text on its subtle fill 4.98:1 (dark 7.31:1), the HUD's
text on the inverse surface 14.6:1 and its hint at 80 % opacity 10.3:1.

### 4.6 Canvas tokens

The sheet is white in every theme (FR-UI-043), so overlay colours are chosen once, against white and against the black
lines of the drawing.

| Alias | Value | Contrast on white | Shape (never colour alone) |
|---|---|---|---|
| `canvas.surround` | light `slate-150` #E3E6EA, dark #2E3238 | — | The grey around the sheet |
| `canvas.sheet_shadow` | elevation 1 (§4.10) | — | Lifts the sheet off the surround |
| `canvas.grid_dot` | #B8BEC7 | 1.9 (intentionally faint) | Dots; the pitch doubles below 8 points |
| `canvas.selection` | `volt-600` #0B63CE | 5.69 (3.69 on black lines) | Solid 2s-px outline with a 1s-px white casing, 8s-px square corner handles |
| `canvas.hover` | `volt-600` | 5.69 | 1s-px dashed outline |
| `canvas.ghost` | `volt-600` at 60 % | — | The placed symbol's strokes, its label chip (§7.3); a Level guide, dotted (doc 06a §6) |
| `canvas.snap` | `volt-600` | 5.69 | Crosshair with a 6-px gap; CP ring 8s + 1 px; the In-line guide of a symbol placed by hand, dotted (s px on, 2s off), solid with a tick across when in line (doc 06a §6) |
| `canvas.open_cp` | `slate-400` #868E99 | 3.31 | A (4s + 1)-px hollow square with an s-px rim on each connection point without a connection on the page (screen only; drawn while a grid step is at least 4 points on screen). A subdued neutral ink, not the accent, so the squares on a valve page do not read as selection handles; fluid ports keep the marker; `slate-400`, 3.31:1 on the sheet, never an accent shade |
| `canvas.highlight.1` | violet #9878FC (dark orange #A86F00 before) | 3.27 | 45° hatch halo, name chip filled with the hue |
| `canvas.highlight.2` | reddish purple #CC79A7 | 3.06 | 135° hatch halo, name chip filled with the hue |
| `canvas.highlight.3` | bluish green #009E73 | 3.42 | 0° hatch halo, name chip filled with the hue |
| `canvas.highlight.4` | vermillion #D55E00 | 3.87 | 90° hatch halo, name chip filled with the hue |
| `canvas.measure` | `color.text.primary` (light value) | 16.9 | 1-px line with end ticks; readout chip |
| `canvas.conflict` | `color.error` (light value) #C42B3C | 5.59 | A 2s-px line over each wire of a potential whose EW-POT-001 finding stands (screen only, under the highlights) |
| `canvas.stale` | `slate-400` at 30 % | — | Diagonal hatch |
| `canvas.chip` | `color.surface.inverse` at 94 % | — | Rounded `radius.sm`, `type.small` |
| `canvas.highlight.label` | #000000 | 6.43, 6.86, 6.14, 5.43 on the four hues | `type.caption` SemiBold on a highlight name chip |

`s` is the integer scale factor `max(1, ⌊ppp⌋)`. **The highlight slots** take doc 16 §10.3's
hues and geometry (an 8-pt band, fill alpha 77, 2-pt hatch at a 6-pt pitch). **The selection departs from §10.3's #0072B2** for the
accent #0B63CE, so the tree, the Properties header and the canvas show the selected object in one colour; both are
blue, and the CVD renders of NFR-A11Y-03 are re-run with the new value. **Highlight name chips** are filled with
their slot's hue (`radius.sm`, 20 px high), as in the mockup; their label is black, because white on the four hues
is under NFR-A11Y-04's 4.5:1 for text (one of the two corrections of §11).

With slot 1 violet, white on the four hues in slot order is 3.27,
3.06, 3.42 and 3.87:1, so the label stays black (the table's 6.43, 6.86, 6.14 and 5.43).

The open-CP squares are a session overlay, never in the display list, print or PDF. On paper, a black box's pins print their CP designations as stored, with the PLC address after one space when the CP is bound ("1 I0.0"): on the top and bottom sides turned a quarter, reading from bottom to top (a text of signs only, "+" or "−", horizontal), on the left and right sides horizontal; pin texts never move, and a cross-reference that would overlap one drops below it.

### 4.7 High-contrast themes

High contrast dark (MVP) and High contrast light (Should) follow doc 16 §11.1 and these rules: text ≥ 7:1 on its
surface (NFR-A11Y-04); every control, row and panel edge has a visible 1-px boundary of at least 3:1; selection and
focus use 2-px outlines, never fills alone; no translucency; shadows replaced by borders. Starting values: dark
surfaces #000000, text #FFFFFF, secondary text #D7DBE0, accent #79B8FF, borders #FFFFFF at 1 px; light surfaces
#FFFFFF, text #000000, secondary text #2A2E34, accent #0047A3, borders #000000. The token test asserts the rules,
so these values may move in part 1 only to pass it.

### 4.8 Typography

**Face.** Noto Sans, the bundled UI and drawing face (ADR-0028), in two weights: Regular (bundled) and
**SemiBold** (to bundle: `NotoSans-SemiBold.ttf`, OFL-1.1, the same family and release as the bundled Regular).
Noto Sans Math stays the symbol fallback. Bold remains for the drawing styles, not the chrome. Noto Sans's lining
digits are tabular by default (each of 0 to 9 advances 572 units in the bundled Regular), so coordinates and counts
do not jitter; no monospace face is needed.

| Token | Size / line (px at 100 %) | Weight | Use |
|---|---|---|---|
| `type.caption` | 11 / 14 | Regular | Badges, key chips, the folio column's secondary text, tiny meta |
| `type.small` | 12 / 16 | Regular | Status bar, command-button labels, canvas and dock tabs, table cells, property labels, secondary lines, canvas chips |
| `type.label` | 12 / 16 | SemiBold, upper case, +0.02 em tracking | Panel headers ("PROJECT", "PROPERTIES") and property category headers, in `color.text.secondary` |
| `type.body` | 13 / 18 | Regular | Default: tree rows, property values, menus, buttons, fields |
| `type.body.strong` | 13 / 18 | SemiBold | Active command tab, the app button's name, primary button, toast titles, the current page row |
| `type.title` | 15 / 20 | SemiBold | Dialog titles, the Properties header, empty-state titles |
| `type.display` | 20 / 26 | SemiBold | Start page and wizard headings only |

At most three sizes in one panel (NN/g). Sizes scale with the UI scale (100 % to 300 %); `type.caption` at 11 px is
the floor and the configurable minimum font size (FR-UI-027) raises every token proportionally.

**The minimum text size raises each token below it
to the minimum**, not every token proportionally, as platforms' minimum font sizes do; the UI scale is the proportional
control. View ▸ Minimum text size offers Off, 12, 14 and 16; its "pt" is this table's px at 100 %. At 16 `type.title` and
`type.body` share a size and differ by weight, which is accepted for a minimum.

### 4.9 Spacing and sizes

A 4-px grid for layout, with one 2-px step inside components (the mockup's 6-px icon gap and 10-px dock-tab
padding). Spacing tokens (a subset of Carbon's scale plus `space.icon`):

| Token | px | Typical use |
|---|---|---|
| `space.1` | 2 | Icon to badge, hairline gaps |
| `space.2` | 4 | Between rail buttons; either side of a command-group divider; inside key chips |
| `space.icon` | 6 | Between an icon and its label (small and Compact command buttons, the app button, primary buttons, status fields, dock tabs); chip and value-cell padding |
| `space.3` | 8 | Between an icon and the text in tree rows and toasts; the gutters of the tab row, the command row and the rail |
| `space.4` | 12 | Row and panel horizontal padding; command-tab padding; primary-button padding; toast padding; canvas inset of the toast and zoom control |
| `space.5` | 16 | Dialog padding; the HUD's distance from the canvas bottom |
| `space.6` | 24 | Between dialog sections; empty-state spacing |
| `space.7` | 32 | Start-page blocks |

| Size token | Comfortable | Compact | Notes |
|---|---|---|---|
| `size.row` | 28 | 24 | Tree, table, property and menu rows (Carbon's 32 and 24, between them) |
| `size.control` | 28 | 28 | Primary and secondary buttons, the filter field |
| `size.control.inline` | 24 | 24 | Property value cells, the command search field, segmented items, the app button, quick access |
| `size.tab_row` | 32 | 32 | Command tabs and quick access |
| `size.command_row` | 52 | 36 | The row of command groups |
| `size.command.large` | 46 × ≥ 52 | 28 high | Comfortable: icon 20 over label, 2-px gap; Compact: icon 20 beside label |
| `size.command.small` | 24 | 24 | Icon 16 and label beside it |
| `size.quick_access` | 28 × 24 | 28 × 24 | Save, Undo, Redo |
| `size.search` | 280 × 24 | 280 × 24 | The command search field's least width; it is 0.2 of the tab row, 384 at 1920 and 512 at 2560 (§2) |
| `size.panel_header` | 32 | 32 | Navigator, Properties and palette headers |
| `size.dock_tab_bar` | 32 | 32 | Canvas-tab bar (tabs 28 high) and the bottom dock's tab row |
| `size.rail` | 40 | 40 | Rail width; rail buttons 32 square |
| `size.status` | 26 | 26 | Status bar; its fields 22 high |
| `size.panel.left` | 280 (220 to 640) | 280 | Navigator and Properties column; Properties takes half its height; its least default width (shown 480 wide at 1920 and 640 at 2560 while not dragged, §2) |
| `size.panel.left_max` | 640 | 640 | The widest the left column is shown while it keeps its default share (from about 2380) |
| `size.panel.right` | 280 (220 to 480) | 280 | Palettes. Not built: no right column reads it, so the token is not in the theme |
| `size.text_measure` | 520 | 520 | The line length of running text on a full-window page (the first-run screen), fixed on purpose |
| `size.dock.bottom` | 184 | 184 | The mockup's reference height of the bottom dock only (the mockup's `.bottom` rule, checked by DL8); the dock does not open at it (§5.7) |
| `size.dock.bottom_min` | 300 | 300 | The bottom dock's least shown height: in the vertical split that holds the page view, the child holding a bottom pane is shown at least this high, at most 2/3 of the split; display only, the user's shares stay the layout (§5.7) |
| `size.property_label` | 96 | 96 | The property label column |

Density changes the row height, the command row and the command buttons' layout only (§4.13).

### 4.10 Radii, borders, elevation

| Token | Value | Use |
|---|---|---|
| `radius.sm` | 4 | Quick-access and other icon buttons, property value cells, segmented items, key chips, status fields, canvas chips, the focus ring |
| `radius.md` | 6 | Command, rail, primary and secondary buttons, the app button, fields, the segmented track, canvas tabs' top corners, the zoom control, menus, popovers, tooltips, cards, palette thumbnails |
| `radius.lg` | 8 | Dialogs, toasts, the command palette |
| `radius.pill` | 999 | Count badges, provenance chips, the tool HUD and its buttons |
| `radius.mark` | 2 | The 2-px accent bars: tab underlines, the rail indicator, the selected-row bar |
| `border.width` | 1 px | Every border; focus rings are 2 px |
| `elevation.0` | none | Chrome and docked panels (flat) |
| `elevation.1` | light: offset 0,1 blur 2 rgba(16,24,40,0.10) plus a 1-px ring rgba(16,24,40,0.04); dark: 0,1 blur 2 rgba(0,0,0,0.40) plus a 1-px ring rgba(255,255,255,0.04) | The sheet, the zoom control, the selected segmented item, menus, popovers, tooltips, canvas chips |
| `elevation.2` | light: 0,8 blur 24 rgba(16,24,40,0.16) plus 0,2 blur 6 rgba(16,24,40,0.08); dark: 0,8 blur 24 rgba(0,0,0,0.50) plus 0,2 blur 6 rgba(0,0,0,0.30) | The tool HUD, toasts, dialogs, the command palette |

In the dark theme a raised surface is lighter than the one below it (`slate-800` over `slate-850`), because shadows
barely show on dark grey (Material's dark-theme guidance, with #121212 as its base surface); the 1-px ring of
`elevation.1` gives small floating things their edge.

### 4.11 Icons

- **Set.** Tabler Icons (MIT): 24-px grid, stroke-only paths with round caps and joins, 5,203 outline icons including
  nineteen `circuit-*` electrical glyphs. Vendored as SVG files at one pinned release (3.50.0, the current npm
  release at writing) under `assets/icons/tabler/` with its licence text, and converted by `cargo xtask gen-icons`
  into a generated Rust table of paths (`crates/ew-ui/src/icons/generated.rs`), drawn through epaint. No icon font and
  no icon crate (the egui wrapper crates track older egui releases).
- **EW glyphs.** Glyphs Tabler lacks (cable, terminal strip, interruption point, black box, potential, structure box,
  QET grid, DIN rail, duct) are drawn by the project on the same grid with the same stroke rules, stored beside the vendored set under
  the application's licence, and go through the same generator.
- **Sizes and stroke.** `icon.sm` 16 px (rows, menus, small command buttons, quick access, the HUD, status fields,
  dock tabs), `icon.md` 20 px (large command buttons, rails, toasts), `icon.lg` 24 px (dialogs; empty states at 32).
  Stroke 1.5 px at 16 and 20 px, 1.75 px at 24 px, multiplied by the pixels per point; Tabler's own stroke 2 at 24 px
  is too heavy beside 13-px text.
- **Colour.** Icons take the text colour of their context (`color.text.secondary` at rest, `color.text.primary` on
  hover, `color.accent` on the active rail button, an active or toggled command and a selected tree row); semantic
icons take their
  semantic colour. No multicolour icons.
- **Meaning.** One icon per concept everywhere (the Pages icon is the same on the rail, in the menu and in the
  palette); an icon never replaces a label in the command bar's large buttons, on dialogs or in menus.

### 4.12 Motion

| Token | Value | Use |
|---|---|---|
| `motion.fast` | 80 ms | Hover and press fades (egui's `animation_time`) |
| `motion.base` | 140 ms | Popover, menu, toast and HUD fade in; a toast fades out too |
| `motion.none` | 0 | Everything when Reduce motion is on, and always on the CPU renderer (doc 16 §10.4) |

No slide-ins, no bounce, no animated zoom by default. A toast or a popover fades in, and a toast fades out as it fades
in; nothing moves the layout.

### 4.13 Density

Two modes in View ▸ Density, stored in the user settings (`density = "comfortable" | "compact"`):
- **Comfortable** (default): 28-px rows, the command row with large labelled buttons for each group's frequent
  commands. For learning and for the pilot preview.
- **Compact**: 24-px rows and a 36-px command row in which every command shows its label beside the icon (large
  commands 28 high with icon 20, small ones 24 high in one row). Rails, tabs, panels and the status bar keep their
  sizes. For experts and small screens (the JetBrains "compact mode" precedent).

Density changes sizes only; colours, type sizes and the order of everything stay the same.

### 4.14 Mapping onto egui

`theme::apply(tokens, ctx)` sets, once per theme or density change:

| egui | Token |
|---|---|
| `Visuals::dark_mode` | theme is dark |
| `Visuals::panel_fill` | `color.surface.panel` (the chrome panels paint `color.surface.chrome` themselves) |
| `Visuals::window_fill`, `window_stroke`, `window_shadow`, `window_corner_radius` | `color.surface.raised`, none, `elevation.2`, `radius.lg` |
| `Visuals::popup_shadow`, `menu_corner_radius` | `elevation.1`, `radius.md` |
| `Visuals::extreme_bg_color`, `text_edit_bg_color` | the text-field fill (`color.surface.panel`) |
| `Visuals::selection` (`bg_fill`, `stroke`) | `color.accent.subtle`, `color.accent` |
| `Visuals::hyperlink_color`, `warn_fg_color`, `error_fg_color` | `color.accent`, `color.warning`, `color.error` |
| `Visuals::widgets.noninteractive` | `bg_stroke` `color.border.subtle`, `fg_stroke` `color.text.secondary` |
| `Visuals::widgets.inactive` | `weak_bg_fill` transparent (subtle buttons), `bg_fill` field fill, `bg_stroke` `color.border.control` for fields, `fg_stroke` `color.text.primary`, `corner_radius` `radius.md`, `expansion` 0 |
| `Visuals::widgets.hovered`, `active`, `open` | the same plus `color.state.hover` / `color.state.pressed` overlays, `expansion` 0 (nothing grows under the pointer) |
| `Visuals::striped`, `indent_has_left_vline`, `button_frame`, `collapsing_header_frame` | false, false, true, false |
| `Visuals::disabled_alpha` | replaced by `color.text.disabled` in the component code |
| `Spacing::item_spacing`, `button_padding`, `interact_size`, `indent`, `menu_margin`, `window_margin`, `icon_width`, `icon_spacing` | (`space.3`, `space.2`), (`space.4`, `space.2`), y = `size.control`, 20, `space.2`, `space.5`, 16, `space.icon` |
| `Style::text_styles` | `Small` = `type.caption`, `Body` and `Button` = `type.body`, `Heading` = `type.title`, `Monospace` = Noto Sans at 12; `Name("small")`, `Name("strong")`, `Name("display")` for the rest; SemiBold through `FontFamily::Name("semibold")` |
| `Style::animation_time` | `motion.fast` |
| egui_tiles `Behavior` (`tab_bar_height`, `tab_bar_color`, `tab_bg_color`, `tab_text_color`, `tab_outline_stroke`, `tab_bar_hline_stroke`, `gap_width`, `resize_stroke`) | `size.dock_tab_bar`, `color.surface.chrome`, `canvas.surround` for the active canvas tab or transparent, primary or secondary text, `color.border.default` on the active canvas tab, `color.border.default`, 1, `color.border.default` (hover: `color.accent`) |

Component code (the command bar, rows, chips) paints with the token values directly through `ew-ui`'s widget helpers;
none of it reads raw egui defaults.

### 4.15 Component tokens of the built widgets

These are for the widgets built alongside this
document (§6.21 to §6.25). Each is a component token (§4.1, layer 3) defined by an alias or an existing size; none
adds a raw value, so the token test checks them through their aliases. `size.dialog.label` and `size.dialog.preview` are the exceptions, raw 160 px and 1040 px in both densities; `color.badge.on_chrome` is checked as a background of `color.text.secondary` in the token dump.

| Token | Comfortable | Compact | Defined as | Use |
|---|---|---|---|---|
| `size.table_header` | 24 | 24 | `size.control.inline` | The header row of every table and data grid (§6.7, §6.21) |
| `size.table_column` | 110 (40 to 480) | 110 (40 to 480) | raw | A grid column's starting width and its resize range |
| `size.severity_icon` | 16 | 16 | `icon.sm` | Severity icons in finding rows, grid cells and the Messages counts |
| `size.chooser` | 240 to 360 wide, 8 rows | the same | `size.row` × 8 high at most | The chooser list (§6.22) |
| `size.preview_list` | 10 rows | 10 rows | `size.row` × 10 | Rows a Preview list shows before it scrolls |
| `size.card_strip` | 28 | 28 | `size.control` | The "Preview open" strip (§6.23) |
| `color.table.frozen_edge` | = `color.border.default` | = `color.border.default` | alias | The frozen key column's right edge, the grid's one vertical line (§6.21) |
| `color.table.cursor` | = `color.focus` | = `color.focus` | alias | The cursor cell's 2-px inset ring while the grid has the keyboard (5.69:1 and 6.0:1 on the panel, 4.95:1 and 4.64:1 on a marked row) |
| `color.table.cursor_idle` | = `color.accent` | = `color.accent` | alias | The cursor cell's 1-px inset ring while the focus is elsewhere; the width, not the colour, tells the two apart |
| `color.history.undone` | = `color.text.tertiary` | = `color.text.tertiary` | alias | Undone History entries, always with their state icon (§6.24) |
| `color.card.strip` | = `color.surface.sunken` | = `color.surface.sunken` | alias | The fill of the "Preview open" strip |
| `size.dialog.label` | 160 | 160 | raw size | The fixed label column of every form dialog, followed by `space.3` (§6.12) |
| `size.dialog.preview` | 1040 | 1040 | raw size | The print pre-dialog's least width; it is 0.65 of the window, 1248 at 1920 and 1664 at 2560 (§6.12) |
| `color.badge.on_chrome` | = `color.border.subtle` | = `color.border.subtle` | alias (high-contrast themes: `color.surface.sunken`, outlined with `color.border.default`) | A count badge on the chrome, such as the dock tab row's count chip (§6.16); secondary text on it is 5.40:1 (light) and 5.19:1 (dark) |

`color.border.control` is not used for the idle cursor: on `color.accent.subtle` it falls to 2.88:1, under 3:1.
Tertiary text does not sit on `color.accent.subtle` either (4.62:1 light, 4.42:1 dark): a selected or marked row
draws its tertiary parts in `color.text.secondary`.

