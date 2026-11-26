# User experience and user interface: the interaction model

A chapter of [User experience and user interface](16-ux-ui.md), whose table says where each section lives.

---

## 4. Interaction model

### 4.1 Principles

1. **Keyboard-first, pointer-complete.** Every action is in the palette; frequent ones have keys (NFR-A11Y-01).
2. **Escape is always safe.** Following the routing order of §4.2, it first closes the topmost transient (popup,
   palette, dialog), then cancels the active tool, then clears the selection. It never changes the model (FR-UI-016,
   NFR-UX-07).
3. **One gesture, one undo step** ([06](06-schematic-editor.md) §3.9).
4. **Preview before anything bulk or identity-changing** (FR-UI-022, §7.5).
5. **Nothing modal while a job runs.** Progress lives in the jobs panel and status bar (NFR-UX-07).
6. **Derived values look derived.** Changing one means an explicit override or freeze (§6).
7. **No time limits and no focus stealing.** A finished job never moves focus or switches the page (NFR-A11Y-04).
8. **Standard vocabulary.** Device, function, connection point (CP), interruption point (IP), potential, structure
   identifier; EPLAN and QET terms work as aliases (NFR-UX-03).

### 4.2 Input routing and focus

Key events go to the first consumer in this order:
1. IME composition or a focused text field;
2. a modal dialog or popup;
3. the active tool of the focused canvas;
4. the focused panel (arrows, Enter, F2, Delete, type-ahead);
5. the keymap context of the focused pane (`canvas.page2d`, `panel.table` …);
6. the global keymap.

Single-key bindings without modifiers (W, R, M) are inactive while a text field has focus. Each window has one
focused pane, shown by a 2 px focus ring in the `focus` token (§11) and announced through AccessKit.

The page navigator takes its keys through its own context; step 3 is described below. Every registry action
has a **context**: `global` (every action of no other context), `canvas` (the grid-cursor actions, F, Shift+F and
Ctrl+Shift+F) and `navigator` (its row keys). The window binds a context's chords only while the model's focus is in
that context, and single keys never fire while a text field has focus. The model's `Focus` is the navigator or a canvas
group; the window gives egui's keyboard focus to that node each frame and tells the model when egui's own traversal
lands on a node. Headless scripts start with the canvas focused, so a canvas action needs no focus step. **Pointer
rules:** a primary press in a canvas focuses its group first. A primary click (press and release within 3 points) on a
cross-reference follows it, as does F on the link under the grid cursor, else under the hovered link; the 3-point
slope separates a click from the start of a drag. **The mouse's back and forward buttons act on the focused group**
(`nav.back`, `nav.forward`), which is not always the group under the pointer. The navigator's row cursor follows the focused tab's page until the user
moves it with the arrow keys, and then stays on the row the user chose while that row is shown, also after a jump made
from the canvas or a link. The transcript's `navigator.row` is `null` while the
cursor follows the current page.

In step 3 the active tool of the
focused canvas takes the keys of the `canvas` context (§4.4) and the pointer events (§4.5). A click on a link is
the Select tool's `Follow` output; F stays the shell action `nav.follow` (above). **The Escape chain** runs in this
order: a popup or dialog, then the link preview, then the tool's own step, then back to Select, then clearing the
selection; with nothing open Escape reaches the tool through `ui.dismiss`. A click beside a floating overlay and its
close button are `ui.dismiss` too ([16a](16a-design-language.md) §6.12). A busy tool is escaped (its overlay,
preview and link preview cleared) on any change of the focused group, tab or page. **The pointer rule:** the canvas
takes the primary button's presses and releases from egui's pointer-button events in order, each with its own position
and modifiers, so a press and a release in one frame (a touchpad tap, a slow frame) still click. A press counts as the
tool's only when the canvas's widget response got the primary button (egui's widget hit test), so the splitter's grab
area between two groups (about 5 points into each pane) never starts a band. The middle button and Space+drag still
pan, and a primary press focuses its group first. A group hidden while a press is held (side by side turned off, or
its last tab closed by keyboard) is not handled.

### 4.3 Command palette

Ctrl+K (⌘K) opens the palette over every registered action (FR-UI-012). Ctrl+Shift+P is an alias for users of
code editors.

| Prefix | Searches | Example |
|---|---|---|
| none or `>` | actions, by label, aliases and ID | "renum" → Renumber devices… Ctrl+Shift+N |
| `@` | objects: tags, part numbers, potentials, wire IDs (find index) | "@-102F" → -102F1, -102F2 |
| `#` | pages by folio or title | "#pneu" → 106, 200 |
| `:` | a cell in the project's xref format | ":103-A1" opens folio 103 with A1 centred (FR-UI-039) |
| `?` | help topics and rule IDs | "?EW-POT-013" |

Without a prefix the search ("Search commands and devices") lists, after
the actions, the devices, terminal strips and cables whose tag holds the query ("X1031" → -X1031); `>` keeps to the
actions.

- **Ranking** is the deterministic score of §15.1, with a recency boost from the last 50 invocations. An empty query
  lists the eight most recent actions.
- **Aliases** come from the glossary (§9.3): "folio report" finds *Insert interruption point*, "master/slave"
  finds *Show all representations* and *Make auxiliary function*, "plot frame" finds the page frame setting,
  "message management" opens the messages panel, "CDP" finds *Insert wire annotation*. "Update reports" finds an
  explanation entry, "Reports are always current", that links to its documentation page.
- **Disabled actions** are listed last, greyed, with their reason.
- **Parameters.** An action with a parameter schema opens an inline form generated from its JSON Schema and
  pre-filled from the context. Example `numbering.renumber`: scope (selection, pages, structure, project; the
  selection by default), scheme (pre-filled `{page}{letter}{counter}`, the token set of
  [03](03-domain-model.md) §8.2, with a live sample "-102F1"),
  include frozen (no), order (column then row). Enter runs it; a `Cost::Bulk` command opens the preview (§7.6).
  The form's foot line says what Enter does: "Enter applies", and for an action whose declaration says that it
  shows a preview first (`previews`: Move to page…) "Enter shows the preview: nothing changes before Apply".
  An action asks only for a parameter it cannot run without, a required one of its signature; one that asks for a
  parameter itself (a dialog) has it optional. A `page` parameter offers the pages the action takes (Move to page…:
  the other schematic pages, then a new page). The same form asks when the action is started without the parameter
  anywhere in the window (its button, a right-click entry, a key): no start answers "Missing parameter". A parameter
  no form can ask for (a file, a selection) is refused in words that point to its menu; a script that leaves out a
  required parameter is answered with the parameter's name.

### 4.4 Keymaps

```toml
[meta]
name = "EW"
version = 1

[[bind]]
keys = "W"                     # letters and digits bind by logical key
action = "connection.draw"
context = "canvas.page2d"

[[bind]]
keys = "phys:BracketRight"     # punctuation binds by physical key; the label follows the active layout
action = "cp.next"
context = "tool"
alt = "Alt+N"                  # second binding for keyboards without that key; Alt+Right stays "Forward" (06 §6)
```

The bindings are generated from the action registry (`ActionMeta.shortcut`, their one source), so the keymap file of
the EW preset is its written form; the generated keymap reference (`docs/book/src/reference/keymaps.md`, from the
keymap sheet) lists every binding per preset and context, and [the action map](../map/actions.md) every action.

- **Presets.** "EW" and "KiCad-like" (FR-UI-013; the KiCad-like preset is the EW preset with A for Place instead of
  I and G beside M for Move); "EPLAN-like" ships only once EPLAN users confirm its bindings (FR-UI-014). The user's
  `keymap.toml` replaces the preset's bindings of the actions it names; a binding for an action the build does not
  know is kept and listed as unused.
- **Contexts and chords.** The contexts are the registry's seven (`global`, the parent, and six children): the four of
  the focus (`canvas`, `navigator`, `properties`, `find`) and two of a panel, `supply_panel` (the supply marks'
  Potential panel, whose Enter, `ip.supply_apply`, takes every row before the focus's keys while the panel is open)
  and `symbol_editor` (while the editor shows, its drawing takes of the canvas's keys those of its tools and of what
  acts on its selection, which run the editor's own actions, `keymap::context_takes`). The window asks the keymap which
  contexts hold the keys (`keymap::key_contexts`). An action only the window runs, a menu only it draws, is in the
  registry too (`window_only`): `ui.app_menu` (F10; Alt pressed and released alone runs it as well) and
  `ui.context_menu` (Shift+F10 and the menu key, on the canvas); dispatched to the shell it is refused in words. So
  every key of the window is a binding, and a rebinding holds wherever it applies. A chord
  is unique within its context: equal chords in one context are a conflict, a child context's chord over a global one
  shadows it (Ctrl+D duplicates on the canvas and fills down in a table; Shift+Enter finishes the tool on the canvas
  and opens the multi-language editor in the property grid; Ctrl+Enter is `canvas.activate_alt` on the canvas,
  `preview.apply` in the property grid and `find.replace_all` in the find pane). Where one focus needs another
  meaning for a chord, the shell maps it (Alt+Up and Alt+Down are `strip.move` while the Terminal strips navigator
  has the focus and `pages.move_up` and `pages.move_down` elsewhere), so the conflict rules still see one binding per
  chord.
- **Reachability.** No preset uses Ctrl+Alt+letter, because AltGr equals Ctrl+Alt on Windows and produces
  characters on QWERTZ and AZERTY layouts, and the reserved chords (Alt+F4, Alt+Tab, Win+anything) are refused.
  Letters, digits, named keys and `Plus`, `Minus` and `Equals` bind by logical key, punctuation by physical key, so
  every action of a preset is reachable on QWERTY, QWERTZ and AZERTY without AltGr (NFR-I18N-06; §15.5); the keymap
  rules check it per layout. Checking `]` and `[` on German and French keyboards by hand is an owner item. A chord
  is pressed with its modifiers exactly (Shift+F10 is not F10, Alt+W not W); a digit or a symbol key also takes an
  extra Shift, which a layout may need to type it (`1` on AZERTY, `+` on US).
- **Key caps.** One display function (`keymap::display`) shows every physical key as its US key cap (`phys:Minus`
  reads "-", `Shift+phys:BracketLeft` "Shift+[") in every text a person reads: the editor's texts, the chips, the
  menus and the HUD; the `phys:` grammar stays in the keymap file. A text that names a key for a way out takes the
  user's binding ("Press D for a target" with the EW preset) and names the action by its title when nothing is bound.
- **Focus.** A text field that has the keyboard focus keeps its own keys (copy and paste, and the letters a tool
  would take); a find field that takes the focus makes its pane the shell's focus, so Tab and Shift+Tab move between
  its fields and never reach a canvas chord. A pane takes its own navigation keys (the property grid the arrows,
  Home, End, Enter, Space and the keys that start an edit; a chooser Up, Down, Home, End, type-ahead, Enter and
  Escape; a creation form Enter to create, Escape to cancel and Tab to move on). F6 moves the focus between the page
  navigator, the canvas groups and the property grid. Copy, cut and paste on the canvas come through the platform's
  clipboard events, so on Windows egui-winit's Shift+Delete cuts too. Find and replace is Ctrl+H on every OS.
- **The keymap editor** is Help ▸ Keyboard shortcuts (`help.shortcuts`): search, Record (the recorded chord replaces
  the action's bindings in its registry context), Reset per action, refused edits with their reasons ("M is already
  Move in the canvas."), what a changed binding shadows, the unused bindings, the Menu-key note while a chord is
  recorded, Import keymap… and Export keymap… (the full effective keymap). An action is Changed when its effective
  bindings differ from the preset's. An edit applies at once and is written only when the keymap has no error; while
  the file could not be read, or a refused one could not be copied aside, edits apply for the session and the file is
  left as it is. The editor lists conflicts and shadowing only.
- **Native file dialogs** run on a thread of their own, and the answer returns to the shell by a channel and a
  repaint request, so the card or dialog raised after it is painted at once without pointer input; on Windows the
  dialog is owned by the main window, which takes no input while it is open.
- **Rows of the Terminal strips navigator.** Delete (`table.clear`) with the cell cursor in the key column (Strip)
  takes the rows whole: it deletes the marked terminal rows' terminals with their symbols and the wires drawn to
  them, one undo step, and its toast counts the wires as the page's Delete does; marked accessory rows it removes in
  one step with its toast; terminal and accessory rows together it refuses; in any other column it clears the cells.

### 4.5 Tool host

Tools are the state machines of [06](06-schematic-editor.md) §4. The host in `ew-ui` keeps one active tool per
canvas (activating another cancels the current one), routes `Transient` to the GPU overlay, `Preview` to a fork
job (dropping results older than the current revision) and `Commit` to `Session::execute`, and returns to Select
after Escape. It shows the tool's `status_hint` in the status bar and announces state changes as a polite live
region: "Wire: routing from -102F1:2. Click a connection point; Backspace removes the last bend; Escape cancels".
Tool options (Place: variant A–H, representation type, autoconnect; Wire: orthogonal, sticky) sit in a strip
above the canvas, never in a dialog. Layout and 3D tools ([12](12-panel-layout.md) §12) use the same contract.

The tool host and the tools live in
**`ew-viewmodel`**, toolkit-neutral and drivable headless; `ew-ui` only paints (the overlay, the status hint and the
live region). The host keeps **one active tool per window**, acting on the focused group's canvas, not one per canvas:
activating another tool cancels the current one, and focusing the other group sends Escape to a tool that is not in
its first state. So a tool's `Preview` is evaluated through `Session::preview`
on the UI thread after the pointer has rested 80 ms (the action `canvas.dwell`, so no timing lives in the model), and a
fork job off the UI thread is not built; and tools receive the prepared page tables and no `World` (doc 06 §4.1).

Select is the default tool. The host handles the tool's
output as follows: overlay items go to the canvas overlay, `Select` replaces the selection, `Follow` navigates the
focused group, and a preview or commit goes to the session (one output per event). **Previews** run on the UI thread
through `Session::preview_with`, once per changed command or revision, with the derived diff as the semantic ghost and
a stale state when the revision moves on; the fork job off the UI thread is not built. **Commits** in the window go
through `Session::commit` of a fresh equal preview, else `Session::execute_with`; headless they go through
`Session::run_script`, the `command.run` path, where a `HumanOnly` command is refused and `--record` holds the line. A
commit is refused on a read-only project (`reason-read-only`). A tool's preview does run on a read-only project and
shows as ready, since a preview is a read; only the commit is refused, and no extra badge shows.

The window measures the pointer's rest and sends the hidden action `canvas.dwell` after 80 ms and after 300 ms, so no
timing lives in the model. At 300 ms over a link, while the tool wants hover (Select yes, Measure no), a preview popup of
the target page opens next to the pointer (320 × 200 points, drawn from the same tile pool and page-content cache, with
the page bounds and a crosshair at the target); any pointer move, cursor move or view action closes it, and the status
hint reads `Click or F: go to <target>` (§3.6). Headless, `canvas.preview_snapshot` (`path`) writes the popup's image as
a PNG, as `canvas.snapshot` does for the page, and is refused without a preview. Under Measure the entity hover
outline and the link hover and preview are not drawn, while the transcript's `hover` member and describe's `canvas
hover` line still report the entity under the pointer: the transcript reports the model's truth, the tool only decides
what is drawn.

**The status hint and its announcement.** The canvas node's polite live region (§10.1) announces the active tool's hint
only when its text changes. "<n> selected" wins in the same step and stays until the cursor moves; an identical text
(for example a second `1 selected`) is not announced again. Measure's readout is announced on Press and on Activate
(NFR-A11Y-01).

**Preview chips.** The semantic ghost (the `before -> after` texts and the stale badge) shows as chips in the canvas's
top-left corner, because the derived diff carries no positions; placing them by the tag's position is not built. The
chips must not cover the statistics panel. The link preview popup has no AccessKit node (the status hint names the
target).

The committing tools: **Place** (armed from the picker, a dashed outline with connection-point marks and a label with the variant letter, the target and, after the 80 ms preview, the predicted tag; it stays armed after a placement; Escape disarms), the **keyboard move** (M) and Select's **drag rows** (a ghost, rubber bands and the readout `dx … dy … mm`, in grid steps), the **transforms** and Delete on a selection, and **Change symbol** (the exchange ghost names the unmapped connection points from the command's notices). The **tool strip** above the canvas shows, as read-only text, the active tool's label and what it floats or drags as the transcript's `ghost` says it (Place: the symbol, the variant letter, the target, the letter or strip and the predicted tag; "(new strip)" until the first terminal prints a new strip's tag); the options themselves are set by keys and hidden actions, and nothing is shown for Select at rest. A **double click** finishes the active tool: its second press is sent as `tool.finish` and its release dropped, measured by the window on egui's input clock: two presses within 0.5 s and 4 points (not the platform's interval), the one rule by which every row, button and field of the window double-clicks too; no timing reaches the model. A pending move is cancelled when the selection, the revision or one of its representations changes, and Delete cancels it before deleting. Previews, commits and the read-only refusal are as above; a preview's chips stay in the top-left corner.

The **Wire tool** (W) snaps a connection point, a junction, a point on a wire segment or the grid (Alt suppresses object snaps), draws a dashed routed ghost with the length readout, fixes a bend on each grid click, removes the last bend with Backspace and commits exactly the ghost's route; it draws the page type's kind and is disabled where that kind is not built ([06](06-schematic-editor.md) §2.2). The live region announces "Wire: routing from <label>. Click a connection point; Backspace removes the last bend; Escape cancels". **Proposals** are dashed stubs on the Place ghost, Tab cycles them, and Alt (Ctrl+Enter from the keyboard) places without them; a move shows its proposals and never commits them. **Route edit** drags a segment or a bend of a selected connection by its round handles (Tab and the arrow keys move a handle, and Route routes the selected wires anew); a moved first or last segment keeps a stub at its end. The **junction sequence editor** is a popup on exactly one selected junction. **Three rules from the window check:** a page or tab change ends Wire, a handle's drag, Place and Change symbol and Select takes over, the selection kept; the **tool strip's row is always reserved** (Select shows a hint there at rest), so the canvas is one row shorter at rest and never shifts under the pointer when a gesture starts; and a ready preview that changes no tag and no cross-reference text paints **no chip** (a route edit, a move, a wire that changes only potentials or wire numbers): the readout and the strip carry it. An undo or redo drops the tool's overlay and preview. A refused action writes its reason in the status bar, and Edit ▸ Junction sequence… says "Select exactly one junction" while disabled.

Routing a page or a selection is Route ([06a](06a-drafting.md) §7.1). The Wire tool's HUD says "No clear route: the wire stays a dashed line without a route. Click free points to lead it,
or route it later with Route." while its ghost is the
unrouted dashed line, "Bend placed: click a connection point to end the wire." after a bend, and "This junction
already joins three wires: end on a wire to make a new junction." over such a junction. **Connect on move** (C): a
drag of representations commits the proposals it shows with the move, C turns this off or on
for the session, and the move's HUD and the status bar say "C Connect on move: on" or "off", the key taken from the
keymap; Alt held at a drag's release moves without them, and the keyboard move (M) follows the session's setting,
since only a drag's release reads the modifier. On the release of a move the wires whose followed route would break
a hard rule, bend more or stay unrouted are routed again in the same undo step.

Text/Text edit input updates the ghost while typing and the 80 ms dwell preview before an explicit Enter commit; Escape cancels. Bound Text commits at its press. Graphic and Structure box use Shift+Enter or a double click for tool.finish; a two-corner box outline becomes a rectangle before its fields/Enter step. Marker fields determine the ghost and commit text, and angle is 90 degrees only when all crossed segments are horizontal. A new drawing-tool activation, including the same action and page/project reset, resets the displayed marker/structure drafts to that tool's defaults; editable drafts survive ordinary frames and outline transitions. Changing the page cancels every drawing tool. Text edit uses F2/double click on a free text and refuses a read-only page; Tools ▸ Wire annotations selects the checked project marker mode.

### 4.6 Selection

- Click selects; Shift+click adds; Ctrl+click (⌘) toggles. A rubber band dragged left to right selects enclosed
  objects, right to left touched objects (06 §4.3). Ctrl+A selects everything on the page or every row of the
  filtered table.
- Selection filters and "show all representations" are those of 06 §3.7 and FR-DEV-019.
- The selection is announced as a live region: "-102G1, power supply 24 V DC, 7 connection points, 9 wires"
  (NFR-A11Y-02; the counts of [08](08-connections-terminals-cables.md) §10); "3 devices selected: -101F1, -102F1,
  -102F2".
- The selection survives page switches. Undo restores the selection of its entry. Entities removed by undo or an
  external change are dropped with a status message.

Click selects, Shift adds and Ctrl toggles. A
left-to-right drag draws a solid band that selects what it encloses; a right-to-left drag draws a dashed band that
selects what it touches. An entity with several rectangles is enclosed only if all of them are inside the band and
touched if any is. A click on empty canvas clears the selection (Shift keeps it). Selected items carry a 2-pixel solid
blue outline (#0072B2). The live region says `<n> selected`. Not built: the rich announcement above and undo
restoring the selection.
Ctrl+A in a navigator's table marks every row of the filtered table; on the canvas (Edit ▸ Select all)
it selects every object the page shown draws, as a band around the whole page would. A device or a function selected
by its tag (the command search, `select.set tag:`) is outlined where the page draws it. The canvas's status field and
HUD count a selection only when part of it is drawn on the page shown: a page chosen in the page list (its page
record) or a device selected by its tag with nothing drawn there is not counted, and the HUD offers no Delete for it;
the live region and the page list's Delete keep the whole selection.

### 4.7 Context menus

Context menus are composed from the registry by predicate, in a fixed group order: primary actions, navigation,
edit, check, clipboard, help. Shift+F10 or the Menu key opens them from the keyboard. On the page a right click
opens the menu only while Select is the active tool: with any other tool it is Escape (`ui.dismiss`), which cancels
what the tool is doing one step at a time (the owner's hands-on test of 0.1.0). Example, right-click on
-102F2 on folio 102:

| Group | Entries |
|---|---|
| Primary | Properties (E) · Assign part… |
| Navigation | Show all representations (Alt+D) · Highlight potential (H) · Follow cross-reference (F), only on xref texts |
| Edit | Freeze tag · Rotate · Mirror · Delete |
| Check | Quick fixes for 2 findings ▸ Mark CP 13 unused, Mark CP 14 unused (EF-09, D-23) |
| Clipboard | Copy (the fragment as text with its header) · Copy tag list (TSV) · Copy as image (PNG) (FR-UI-040; one multi-format copy at 1.0, FR-UI-051) |
| Help | Help on load-distribution devices (F1) |

### 4.8 Drag and drop

Every drag has a keyboard equivalent: the source's context command "Place on canvas…" enters the Place tool with
the item floating at the grid cursor (arrows move it, Enter commits).

| Source | Target | Result | Keyboard equivalent |
|---|---|---|---|
| Symbol browser item ([07](07-symbols-parts-macros.md) §5) | page canvas | Place tool in Floating state; drop commits `device.place` | Enter on the item |
| Part browser item | page canvas | `device.place` with `target = FromPart` (`device.place_from_part` and `symbol.place` are registered aliases; the journal records `device.place`, [17](17-extensibility.md) §2.1); remaining functions appear as unplaced (FR-UI-017) | Enter on the item |
| Macro | page canvas | `InsertMacro` ghost with conflict list (FR-MAC-005) | Enter on the macro |
| Unplaced function (device navigator) | page canvas | new representation of the existing function (FR-UI-007) | "Place on canvas…" |
| PLC channel (PLC navigator) | coil or channel symbol | binds the channel (FR-PLC-020) | "Bind to…" with target picker |
| Unplaced device (layout list) | rail or surface | placement (FR-LAY-013) | "Place device" |
| Page row (page navigator) | another position | renumber preview (FR-PAG-024) | Alt+Up, Alt+Down |
| A file from the OS (the first dropped that is no folder) | window | the Image tool armed with it (`image.file`), the picture placed by a click; a file the asset store does not take (a `.qet`, a project, a table) is refused in the store's words and changes nothing | Insert ▸ Image |

A project is opened with File ▸ Open, a QElectroTech project imported with File ▸ Import ([14](14-interop.md) §13)
and a table imported from its table's menu (§7.7); none of them is a drop.

Drop targets show valid or invalid feedback with a reason in text, for example "Not allowed on a fluid page:
electrical symbol". Dropping a valve onto the generated overview 202 is refused with "edit -202U2's children in
the device navigator" (06 §8, D-10).

