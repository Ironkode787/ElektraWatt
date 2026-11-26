# Design language: components: states, buttons, fields, rows, cards, toasts, dialogs and menus

A chapter of [Design language](16a-design-language.md), whose table says where each section lives.

---

## 6. Components

---

### 6.1 States

Every interactive component has these states, drawn the same way everywhere:

| State | Drawing |
|---|---|
| Rest | As specified per component |
| Hover | `color.state.hover` overlay; no size change (`expansion` 0) |
| Pressed | `color.state.pressed` overlay |
| Selected | `color.accent.subtle` fill; text primary, icon `color.accent`; in lists a 2-px accent bar (`radius.mark`) at the left edge, inset 4 px top and bottom |
| Focused (keyboard) | 2-px `color.focus` ring, 1 px outside the element, `radius` of the element + 1 |
| Disabled | `color.text.disabled` text and icon, no hover; the reason as tooltip (doc 16 §2.4). A disabled Primary draws as a Secondary (its fill and border) with `color.text.disabled`, without egui's extra fade; about 2.2 to 2.3:1, exempt as a disabled control. The exemption stands and there is no 3:1 disabled token; `color.text.disabled` is 2.23:1 (light) and 2.27:1 (dark) on the panel, legible yet clearly under every enabled label, and a test keeps it within [2, 3) and at most half the primary text's ratio, so it can neither fade into illegibility nor make a disabled Primary read as an enabled button |
| Error | `color.error` boundary on fields, the message under the field with an icon |
| Busy | the label stays; a 12-px spinner replaces the icon; never disables the window |

---

### 6.2 Buttons

| Kind | Rest | Use |
|---|---|---|
| Primary | `color.accent` fill, `color.text.on_accent`, `type.body.strong` | The one recommended action of a dialog, card, banner or empty state |
| Secondary | `color.surface.panel` fill, 1-px `color.border.default`, `color.text.primary` | Every other dialog action |
| Subtle | no fill, no border, `color.text.primary`; hover overlay | Toolbar-like actions inside panels, toasts, rows |
| Danger | Secondary with `color.error` text; Primary with `color.error` fill only in a confirmation of a destructive step | Discard…, Delete…, Purge… |

`size.control` high, `radius.md`, padding `space.4` horizontally, minimum width 72 px for text buttons in dialogs. An
icon in a text button sits left at `icon.sm`, `space.icon` before the label. Labels are verbs (§8). The default (Enter) button is the primary one, never a
destructive one.

---

### 6.3 Icon buttons and command buttons

Icon buttons are Subtle buttons with only an icon in `color.text.secondary`: 24 px square with `icon.sm` and
`radius.sm` in panel headers, rows, toasts and the dock ("More", close), `size.quick_access` in the tab row, 28 px in
the zoom control; they always have an accessible name and a tooltip "<Label> (<key>)". Command buttons are §5.2's large, small and split buttons; their
tooltip is "<Label> (<key>). <One-sentence description>. F1: help" (doc 16 §3.3).

---

### 6.4 Tabs

Three kinds, one family: inactive tabs are text in `color.text.secondary` with the hover overlay and no box; the
active tab is SemiBold in `color.text.primary`. **Command tabs** (§5.2) are `type.body` with a 2-px accent underline
inset `space.4`. **Bottom-dock tabs** (§5.7) are `type.small` with a 2-px accent underline inset 10 px and a count
chip. **Canvas tabs** (§5.5) are `type.small`; the active one opens into the canvas (surround fill, a border on three
sides) with no accent line, because the canvas below it is the indication. Close buttons only on dock and canvas
tabs. Tab bars never carry a second heading.

---

### 6.5 Fields

`size.control` high, `color.surface.panel` fill, 1-px `color.border.control`, `radius.md`, `space.3` padding;
placeholder in `color.text.tertiary`; focus replaces the border with the 2-px focus ring. Filter fields (the
navigator's "Filter pages", inset `space.4` from the panel's sides) carry a leading 16-px filter icon and a trailing
clear button when not empty; the command search field is §5.2's. **Inline value cells** in Properties are §6.8's. Numbers align right and carry their unit as a suffix in
`color.text.secondary` ("2.5 mm²").

---

### 6.6 Tree rows

`size.row`, `space.4` horizontal padding, `space.3` between parts, indent 20 px per level, then: a 12-px chevron
column in `color.text.tertiary` (absent on leaves), a 16-px kind icon (`color.text.secondary`), the **title** in
`type.body` and `color.text.primary` with ellipsis and a tooltip when cut, then right-aligned **badges** (a finding
count badge, "generated" as an icon, "QET" as a caption badge) and the **meta column** in `type.small`,
`color.text.tertiary`, tabular digits: a page's folio, a group's child count, a device's page. Group rows (sections,
structure levels) look like other rows with a folder icon and their child count. The row of the page shown in the focused canvas tab is
`type.body.strong`. Selected rows take the Selected state; selection synchronisation with the canvas (doc 16 §5.4)
shows the same accent in both. Pending or muted rows use `color.text.tertiary` and an outline icon.

**A single click on a page row opens that page** in the focused
canvas tab, as a double click does (which still opens); Ctrl-click and Shift-click mark rows without opening, so a
selection of pages is made without leaving the page on show.

---

### 6.7 Table and grid rows

Header row 24 px on `color.surface.sunken`, `type.small` SemiBold in `color.text.secondary`, sticky, with a
`color.border.subtle` line under it and a sort indicator icon; body rows `size.row` in `type.small` with `space.4`
cell padding and one 1-px `color.border.subtle` line under each row; cells that name an object are links in
`color.accent`; no vertical lines,
no zebra stripes (the line and the hover are enough at this density); numbers right-aligned with tabular digits;
editable cells show a field on focus or double-click; the selected row takes the Selected state; the focused cell a
focus ring.

---

### 6.8 Property rows and the Properties header

**Header** (under the panel header, `space.4` side padding, 4 px above and `space.3` below): the object's printed
name in `type.title` ("-K1", "103 Power supply 3"), one line in `type.small` and `color.text.secondary` (kind and
rating, then the page: "Contactor, 3 NO + 1 NO aux · page 3"), and at the right the findings badge if any. For a
multi-selection: "3 devices" and the mixed-selection rule of doc 16 §17. A filter field ("Filter properties")
appears under it only when the open categories hold more than twelve rows.

**Categories**: 28-px collapsible headers in `type.small` SemiBold, `color.text.secondary`, with a chevron; **Main**
(the fields of the kind marked primary in the field catalogue: tag, function text, part, location, page title,
number) and the other categories open by default, a collapse the user makes kept in the settings per kind. **Advanced** (the record's internal members: `$v`, ID, key,
source reference, lineage, form UUIDs, counters) is hidden; a link "Show advanced (n)" in `type.small` and
`color.accent` at the panel's foot shows it, and the choice is remembered (Settings ▸ Show advanced properties). Under
a filter, n counts the advanced rows the filter leaves and the link goes at 0.

**Rows** (`size.row`, `space.4` side padding, `space.3` gap): the label in `type.small` and `color.text.secondary` in
the `size.property_label` column with ellipsis and tooltip; the value in a `size.control.inline` cell (`radius.sm`,
`space.icon` padding, a transparent 1-px border that turns `color.border.control` on hover and `color.accent` with a
1-px accent ring while editing) in `color.text.primary`; unit as suffix; at the cell's right end a **provenance chip
only when the value is not authored**: a pill in `type.caption` and `color.text.tertiary` with a 1-px
`color.border.default` naming the source in one word, "inherited", "varies", "page", "derived", "override", "frozen",
"library" or "generated", with doc 16 §6.2's full chip text as tooltip and accessible description. Where every value row of the sheet shares one such source (a report page's rows, all generated), the header shows that chip once after the title and the rows show none.
An authored value shows nothing, so the eye finds the exceptions. An empty authored value shows an em dash in
`color.text.tertiary`. A value chosen from a list (an enumeration, a colour, a structure
node, the connection grid) shows a 16-px `chevron-down` in `color.text.secondary` at the cell's right end, always,
and its node is a combo box; a value chosen in the part picker shows a 16-px `dots` icon there while the row's "…"
actions button is not on show. A navigator table's cell is typed, never a list, so it shows neither. **Editing is in place:** a click or Enter on an editable value turns it into its editor;
Enter commits, Escape cancels; a read-only value shows no editor and its reason as tooltip (a cut one its whole text
first, the reason under it), and a click on it moves
the row cursor there, so what is typed next is refused with that reason. There are no "edit"
buttons. The row's actions (Reset to derived, Freeze, Why this value?) are in its context menu and in a "…" icon
button that appears on hover and focus. Two rows never share a label: the field catalogue gives each a distinct name.

No chip says "part": the model has no part source of its own for a property value. The header's finding chips and actions popup are not built; a filter for more than twelve rows is not built.

**The header says what the device is and the rows follow it.** Under the title of one device
a line in `type.small` names it in trade words from its natures (the disciplines of its points, a bus port, a PLC
station or module, a terminal, a strip, a cable; `Session::device_natures`): its symbol's name, then its natures
("Compressed-air supply point · pneumatic", "5/2 valve, solenoid and spring · pneumatic, 1 solenoid"), or the nature
itself ("PLC station · CPU", "Valve terminal · PLC station on PROFIBUS DP", "Terminal"). A row of another trade is
not listed (an empty strip, terminal, cable or assembly aspect, an empty bus port or interruption point, an empty
cross-reference of a symbol without an electrical point). Main holds the kind's primary fields in their list's order;
a device's are tag, function text, part, location, a source's medium, any other fluid device's ports with the medium
each carries, a valve's solenoids and position. The rows only a rare case needs stand in a last group **More** per
section, closed until opened (its open state is remembered); the filter searches closed groups too.

The header shows one count chip per severity, also for one finding ("1 Warning", "2 Warnings"); glyphs replace the words when the title does not fit. An edit field opened on its stored value starts with the whole text selected, so typing replaces it.

**The label column fits its labels,
then gives way to a value.** It is as wide as the widest label of the rows on show, from `size.property_label` up to
40 % of the row; then it gives way, down to half of `size.property_label`, to the widest value it can make
whole: a value counts whole, or, when too wide for that, by its first word if that word contains a digit (a tag, a
part number: "EW-GEN-CPU-1 …"), so the tag or the number is not cut while the labels have room.

**A value that names a record is a link.** A placement's Device row is
drawn in `color.accent` (P4), underlined on hover; a click or F (`props.follow`, the Properties context of every
keymap) shows the device's main page with its drawings selected, and the tooltip is the canvas link's hint ("Click or
F: go to -102F1"). While an editor field has the focus F is typed. A page's scale reads and is typed "1:10";
its connection grid is an Advanced row. The editor's text is centred as the value's; a choice list
opens under the value cell and not at all when nothing matches; Home, End and the arrows scroll the
cursor row into view. A pre-filled form or dialog field selects its text when it takes the focus.

**A cut text and its tooltip:** a list whose rows may be
cut (the find results, the symbol picker) shows each row's whole line as a tooltip on every row, cut or not, so the
pointer reads any row the same way; a single cut label (a property label, the part dialog's fields) shows its tooltip
only when it is cut.

**The
label column takes what its labels need, up to half the row.** It is as wide as the widest label of the rows on show,
up to half the row, the values keeping at least half; a label longer than its column wraps at its word boundaries
onto further lines instead of being cut, so no nested Dutch label is cut, in a 1280 × 800 window either. The identifier
rule above never narrows the column below the widest single word of a label with its indent, and prose values and
list counts take no width from the column (only identifiers do). The rule holds at every width. An
identifier (the Tag value beside its chip in a narrow column, for one) keeps its whole width and its chip, the label column
yielding first, its labels wrapping and a single overlong label word cut with its tooltip. A
prose value wraps at its words onto up to three lines, its row growing with it, and only a value longer than three
lines is cut, with its whole text as its tooltip.

**Labels give way to
identifiers only, down to half their width.** The word floor above does not hold for
an identifier, so an identifier value (a tag, a part number, any word with a digit) can narrow the label column down to
half of `size.property_label`, and a label whose longest word no longer fits is then cut on one line with its whole text as
its tooltip; a nested Dutch label can therefore be cut in a narrow column beside an identifier. A prose value or choice word
without a digit, one word or several, and a list count take no width from the labels; a mixed value (‹varies›) counts
whole, as an identifier does. The row's "…" button counts
in the value's width, so the selected row's identifier stays whole too.

---

### 6.9 Cards

A card is a bounded panel inside a pane for one object that waits for a decision (the preview card): `color.surface.panel`, 1-px `color.border.default`, `radius.md`, `space.4` padding, a title in
`type.body.strong`, a summary line, then the lists, then the actions right-aligned with one Primary (Apply) and one
Secondary (Cancel). A stale card shows the warning banner style inside its top (§6.11).

---

### 6.10 Toasts and the notifications log

Toasts stack at the canvas's top right, `space.4` from its edges, newest at the top, at most three:
300 px wide, `color.surface.raised`, `elevation.2`, `radius.lg`, `space.4` padding. Anatomy, in three columns with
`space.3` gaps: the 20-px semantic icon; the title in `type.body.strong`, an optional line in `type.small` and
`color.text.secondary` ("6 pages, 1 warning"), and an optional action as a text link in `type.small` SemiBold
`color.accent` ("Open folder", "Undo"); a 24-px close icon button. They
fade in (`motion.base`) and leave after 8 s; hovering or focusing one pauses it; every toast is also in the log, so no
action depends on its time (NFR-A11Y-04). The **notifications log** is a popover from the status bar's bell: a list
in tree-row style, newest first, each with its time, message and still-valid actions; "Clear" at its foot.

The notification model has no severity and no secondary line, so **the toast's icon follows from whether it offers an action**, and the line row is not drawn. The log's foot has Close, not Clear.

Deleting a device's last representation names the device left without a symbol in the delete toast, with "(now <new tag>)" after a re-tag. The check toast counts findings by the status bar's rule (§5.8). A check that found something offers Show messages, which opens the Messages list in the bottom dock.

A parts import says each outcome in one toast, the applied import's with Undo, and no banner (a banner is for a state of the whole project, §6.11); and the Preview tab shows the summary and the lists without repeating the counts.

**A toast grows with the window:** 0.22 of the window's width (422 px at 1920, 563 px at 2560), never
narrower than `size.toast` (300 px) and never wider than the window. Its line wraps at
word boundaries up to four rows, so a refusal's reason is never cut at one line; what does not fit in four rows ends
in an ellipsis, with the whole text in the tooltip, the HUD and the log.

**A toast may show a brief of its text**: where several reasons stack, the toast shows the first and how many more ("And 2 more: hover
here, or see Notifications"), and its tooltip and the log show the whole text. Arrange and route's "Nothing to
arrange" is a result toast: its title, then what stands in the way as its line. **A toast that says again** what the
visible toast of its key says counts the times in a row at its end ("… (2×)"), so that a second Arrange and route with
nothing to arrange is seen to have run; the log keeps each entry's text.

**A toast fades out as it fades in:** its opacity rises from 0 to
1 over `motion.base` from the frame that first paints it, stands at 1, and falls to 0 over the last `motion.base` of
its 8 s, all on the window's frame clock; then it leaves. Hovering a toast pauses it: its clock stands still
while the pointer is on it, once the pointer has moved onto it: a toast that appears under a pointer at rest
fades on time, and a toast already paused stays paused while the pointer is still.

**A toast's tone says what happened, and nothing else; no toast stays over the sheet:** a toast with an offer or of
the warning tone shows for its 8 s as any other and then goes to the notifications list of the status bar's bell,
where its offer stays usable as long as it can run (Delete's Remove them from the project, Remove it from the project
for one terminal, beside its Undo; Cut's Undo). Delete's toast of terminals that stay in the project and the Cut's toast
of the wires it removed report a step done, with the success icon; a check's toast with warnings carries the warning
icon.

**Tips** are toasts with the info icon, whatever
their action, and the key `tip`. A toast with two actions (Undo and Show changes after an
external change) shows them on one row while they fit.

**The toasts keep clear of a tool at work:** while a
tool other than Select is active, its press in the canvas's top-right quarter, where the stack would stand over the
next placing, sends the toasts to the canvas's bottom right, `space.4` above the zoom control and the HUD (§5.5) and
`space.4` from the right edge, newest still at the top. They stay there until Select is back or a press elsewhere on
the canvas; moving the pointer, or pressing on the stack itself, moves nothing, so a toast the user reaches for never
moves away.

**In Parts** the toasts stand at the top right of the sheet or the drawing, `space.4` under the strip that says
where the person came from, so a save's toast never covers the way back.

**In a station's view** (doc 10 §6) the top right holds the catalogue and its filter, so the toasts stand
at the bottom right of the rack's lists, `space.4` from their edges. What the person does next is the To do's buttons,
so the To do is not the lists' last part there but their first, right under the rack: a toast covers at most the
right end of the places table or the channels, which scroll from under it.

---

### 6.11 Banners

A banner is for a state of the whole project (read-only, recovery pending, integrity findings, GPU fallback): full
width under the command bar, `size.control` + `space.3` padding, the semantic subtle fill, its icon, one sentence, one
Primary action and at most one Subtle action, a close button when it may be dismissed. One banner at a time; a second
state queues behind it with a count ("1 more").

The banner lies over the content as an overlay rather than taking a band of its own: along the top edge of the canvas below the page tabs when a page view is shown (full width of the page view), else under the command bar; the toasts start below it. Showing or dismissing it never moves the canvas or a panel. A refused canvas click (an interruption point that EW-CORE-052's prevent mode refuses) is no project state: it says why in the HUD and a toast, not in the banner. The refusal is said once, in words, with the source's page and the way out ("Not placed: 0V already has a source on page 101. Press D for a target, or mark one source as the star."): no canvas-top preview strip, the HUD and a toast carry it, and Messages keeps the rule's code.

**A state banner stays while its state holds:** the
held-saves banner (an external change with collisions waiting) and the cloud-folder banner are kept through every
action until the state ends. The held-saves banner's Primary, "Choose…", reopens the collision dialog, and it has no
Dismiss, since the saves stay held.

---

### 6.12 Dialogs

Modal only when the work cannot continue without the answer (doc 16 §4.1). `color.surface.raised`, `elevation.2`,
`radius.lg`, width 440 px (small), 640 px (forms) or `size.dialog.preview`, 1040 px (the print pre-dialog), each the least of a width that grows with the window (§2: a form 864 px at 1920, the print pre-dialog 1248 px at 1920 and 1664 px at 2560), `space.5` padding, a backdrop of `slate-900` at 24 %. Anatomy:
**title** in `type.title`, left-aligned, phrased as the question or the task ("Recover unsaved changes?"); **body** in
`type.body`, at most three short paragraphs, facts as a two-column label and value list; **footer** with buttons
right-aligned in the platform's order (Windows: Primary then Secondary; Linux: Secondary then Primary), a
destructive action left-aligned and apart from the others. Escape cancels; Enter runs the Primary; the first focus is
the Primary unless it is destructive. No close cross: Cancel is the way out.

**A floating overlay is no dialog:** the browser (§5.6), the command palette, the Join chooser,
the junction editor, the text popup, Show changes and the notifications log close by Escape, by a click beside them
and, where they have a title row, by a close button (the `x` icon button at the right, its tooltip "Close (Esc)" with
the key of `ui.dismiss`); the click beside lands on a backdrop under the overlay (clear, the browser's
`color.backdrop`), so it reaches nothing below, and is `ui.dismiss` as Escape is; while a list of the overlay is open
(a palette form's Type, Strip or Kind) the click beside closes the list only, and what was typed stays. One
component does it (`components::overlay_backdrop`, `close_button`). A dialog that asks keeps asking.

Every dialog is drawn modal over the `color.backdrop` scrim. **In a confirmation of a destructive step (journal discard, purge) Cancel is the Primary**, so Enter keeps the data; the step is the danger button, and the Primary is drawn as a Secondary beside it. The button order on Linux and macOS has no test.

**`color.backdrop` is per theme:** light and high-contrast light `slate-900` at 24 % (#1A1D213D), dark and high-contrast dark black at 40 % (#00000066, the alpha of the dark `elevation.1` shadow). The scrim covers the whole window, so the command bar, rail, navigator, Properties and status bar are visibly darker under a dialog in every theme (a pixel test asserts it in light and dark). **Form dialogs** lay their rows out on one fixed label column, `size.dialog.label` (160 px) then `space.3`, so the field edges line up from dialog to dialog; the fields fill the body at `size.control` height and carry no placeholder that repeats their label; a choice field shows the display words of §8 ("Multi-line", "Location", "Page-based", "User-defined") while the action receives the raw value, and a yes-or-no field is a check box named by its label. The text caret is 1 px (`border.width`).

**First focus:** a form dialog's first focus is its first
text field, else its Primary, never a check box, with the focus ring shown; every other dialog takes its first focus on
its Primary on every opening, a reopening by a poll included (a dialog of another kind than the last frame's is a new
one), and in a destructive confirmation the Primary is Cancel, so the journal-discard dialog opens on Cancel.
A form whose Primary is disabled at open takes no first focus. **Enter on a focused check box does nothing;
Space toggles it** ("Enter runs the Primary" gives way there, so Enter never submits a form by accident). **Ctrl+Enter**
submits a form from any focus; with a disabled Primary its action refuses in its tooltip's words, and a refused submit
shows its reason inside the form, which stays open. A form whose fields scroll (New part) says what is wrong in its
foot row, left of its buttons, naming the field ("Width (mm): …"): one line in a row it has anyway, so a message
that comes and goes costs no height and moves no field. **A save refused for held collisions
opens the collision dialog again**: the save is the user's own request and the dialog is the way out, while no poll
reopens it after Decide later.

**A form whose Primary is disabled
at open** (Create enclosure with no part) takes its first focus on its first text field, else on its first enabled
control, a check box included, with the focus ring shown: "never a check box" and "takes no first focus"
above give way for that case, and Enter on that check box still does nothing while Space toggles it. The start-up
dialog takes its first focus on its Primary, as every other dialog does.

---

### 6.13 Menus and context menus

`color.surface.raised`, `elevation.1`, `radius.md`, `space.2` padding, rows of `size.row`: a 16-px icon column (empty
when the item has no icon, so labels align), the label, the key in `color.text.secondary` right-aligned, a chevron for
a submenu. Hover and keyboard highlight use the hover overlay, not the accent; a checked item shows a check icon in the
icon column. Separators are 1-px `color.border.subtle` lines with `space.2` above and below. Disabled items keep their
place and show their reason as tooltip; in a right-click menu and in a button's menu (a command bar button's, a panel's
More) the reason also stands on a caption line under the entry, while the
app menu keeps it in the tooltip, where a line under every greyed command would bury the tree. A row's right-click
menu opens under its row, where the pointer was along it, or over the row where there is no room under it; it never
covers the row it acts on.

**The right-click menu is the quick way to what fits the thing under the pointer.** One rule: its first
entries are what fits the thing, by what it is; then the general ones. A right-click on an object with the Select tool
selects it first; a right-click on an empty spot clears the selection. Shift+F10 or the menu key opens the same menu
at the selection while the canvas has the keys. Every entry calls an action the window has elsewhere, aimed at the
thing; one that cannot run now is disabled with its reason, as the command row's buttons are. The canvas's entries:

| Under the pointer | First entries |
|---|---|
| A connection point | Mark not used, or Mark used |
| One wire | Route, Wire annotation, Define potential…, Cable… (a core's own cable), Show wire numbers on the pages (checked when on) |
| Several selected wires | Make cable from the selection… |
| A junction | Define potential… |
| An interruption point | Supply comes in here… (Remove the supply mark on a marked point), Go to partner (its cross-reference's cell; disabled while it has none or the point is external), Direction: Source, Direction: Target (the current one checked), Continue on page… |
| Several interruption points | Supply comes in here… for them all |
| A terminal | Add terminal (its strip, its own kind, the next free number, floating at the pointer), Edit strip, Define potential… |
| Several selected terminals | Choose part… for them all, Bridge terminals (two of one strip), Spacing… |
| A symbol of a device | on a coil Add contact per free contact of its device by name (each floats its symbol; disabled with the way round when there is none), on a contact Go to coil; Join device… where another device can take its function (a contact a relay's or a contactor's, a function a device that lists it unplaced; never otherwise); Choose part…; Edit symbol (disabled with the reason a symbol can only be copied), Copy and edit; Define potential… on a symbol of one connection point |
| A placement of a panel page | Turn, Flip, Lock or Unlock, Delete (and nothing general) |
| The frame or the title block | Title block fields…, then an empty spot's entries |
| An empty spot | Paste, Paste in place, Page properties, New page…, Show wire numbers on the pages (not on a panel page) |

After an object's own entries come the general ones, where they can apply:

| Selection | General entries |
|---|---|
| Holds a symbol (a device's, a terminal, an interruption point) | the command row's Arrange group (Rotate, Mirror, Renumber devices…), Move to page…, Keep in place (checked while every selected symbol is kept, doc 06a §2.4), then Cut, Copy, Paste, Paste in place, Delete, Properties of selection |
| Wires, a junction, a text or a frame without a symbol | Cut, Copy, Paste, Paste in place, Delete, Properties of selection (nothing turns or mirrors) |

A navigator's row offers what the same thing offers on the page, from the one function that decides a thing's
entries; the right-click selects the row first, as a click does (a page row: opens its page), unless it is marked
already. Each entry that cannot run is disabled with its reason.

| Row | Entries |
|---|---|
| A device (Devices, the Project pane's Devices list; a PLC station or module) | Go to its first placement (`101-E4`; disabled when it is not placed), the device's entries of the table above for its first placement (Add contact, Choose part…, Edit symbol, Copy and edit), Properties of selection |
| A PLC channel | Go to its place, Properties of selection |
| A cable | Cable… on the cable's wires (shows the page of its first wire, its wires there selected; disabled while no wire is a core), Go to its first wire |
| A page (Pages tree and table) | Open page in a new tab, Title block fields…, Page properties, New page…, Delete page |
| A section of the Pages tree | New page…, New section… |
| A Potentials row with a definition point | Define potential… |
| A Terminal strips row of a strip | Add terminal, Edit strip |

Not offered: a strip's accessories (the strip editor's), a cable's core rows and a device's function rows.

