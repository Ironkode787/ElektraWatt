# Schematic editor: copy, selection, texts, undo and online checks

A chapter of [Schematic editor](06-schematic-editor.md), whose table says where each section lives.

---

### 3.6 Copy, paste and duplicate

The clipboard carries a **project fragment** in the macro format ([07](07-symbols-parts-macros.md), ARCH §5.5): JSONL
records of the selection, the embedded library copies they use (symbols, FnDefs, parts, with their copy metadata
`{library_id, item_id, version, sha256}`), the structure nodes they reference (by aspect and code) and a format
stamp. arboard 3.7.0 offers text and images only ([rust-ecosystem](../research/rust-ecosystem.md)), so at MVP the
fragment travels as **text** behind a `#ew-fragment v1` header line; another EW instance recognises the header, any
other application sees JSON Lines (FR-UI-040). "Copy tag list" (TSV) and "Copy as image" (PNG) are separate
commands. A multi-format clipboard (fragment, TSV, PNG and a vector flavour at once) needs platform code and is 1.0
(FR-UI-051). A pasted fragment is untrusted
input: it is schema-validated with the importer's size limits and never carries rule packs, reports or scripts.

Paste (`edit.paste`, FR-DEV-029, FR-UI-036):
1. **Identity.** Every pasted record gets a new UUIDv7, and references inside the fragment are remapped. After a Cut
   the devices the cut left unplaced are the exception: a cut and its paste are a move, and they come back with
   their IDs (below).
2. **Numbering.** Tag counters are re-assigned by the target's scheme through the online-numbering normaliser, in
   the order of the fragment's anchor cells, so the result does not depend on record or creation order
   (FR-NUM-028). Frozen tags are not copied: a pasted "S51" is derived like any new device. The ghost shows the
   resulting tags.
3. **Structure.** Nodes rebind by (aspect, code) or alias; unknown nodes map to the target page's structure, and the
   ghost lists the mapping.
4. **Library copies.** A copy of an item the target already embeds in the same version (same `sha256`) is reused,
   and a copy of an item it does not embed is added. For another version of an embedded item the paste asks for
   one of three choices ([07](07-symbols-parts-macros.md) §2.1): update the existing copy (a previewed "update from
   library", FR-SYM-016), adapt the pasted records to the existing copy, or keep both, which mints the
   version-qualified copy ID `v5(NS_LIBCOPY, library_id ‖ item_id ‖ version)` for the second copy.
5. **Connections** are copied only when both ends are copied. The ghost counts the dropped ones ("3 connections not
   copied: one end outside the selection").
6. **IP names** are kept and may pair with points already in the target. The ghost lists new pairings so that an
   accidental star with a foreign "L-1" is visible before commit.

**Paste as representation** (paste special), specified and not built, creates new representations of the copied
functions instead of new devices, for example to draw an existing device on an overview page.

Copy, Cut, Paste, Duplicate and Copy tag list exist on the canvas (Ctrl+C, Ctrl+X, Ctrl+V, Ctrl+D; Copy tag list on the Edit menu), with the fragment of [05](05-project-format-and-persistence.md) §12. Items 1, 3, 5 and 6 hold as written (item 1 with the move after a Cut); item 2's copies are numbered by online numbering in the order of their anchor cells (FR-NUM-028); item 4's three-way choice is not built, and a copy the project holds in another version is refused ("update from library first"). **Cut leaves the device unplaced** (decided): Cut is Copy and then the deletion of the selected representations (with the wires drawn to them, §3.4), so the device and its functions stay in the project, unplaced (FR-DEV-003). **A cut and its paste are a move**: `edit.paste` brings back a device of the project that the fragment's symbols drew and no symbol draws any more (effect 8): the device, its functions and the strip of such a terminal keep their IDs, parts, frozen tags, levels and data, nothing is made anew, and what the cut left of them goes in the paste's step (the fragment's wires and junctions the project still holds, a wire on another page left open at them); the paste's notice, in its preview's label and its toast beside Undo, says how many such wires go and on which page ("6 wires on page 102 that ended at the moved devices are removed"). On the page the paste lands on, a wire from a device the cut did not take to one that comes back follows its new pin in the same step as a dragged symbol's wires do (`rep.move`: the route's end moves with its pin, `geometry::follow_end`, a dog-leg where the end's segment would leave its axis), so no wire is drawn to the spot where the device stood; the router is not run. A paste after a Copy, whose devices are still drawn, makes new devices; a second paste after a cut too. Where the page changes a tag (a strip's `{page}`), the paste's label says so before the click ("-X1011 becomes -X1021: a tag follows the page its device is drawn on"), and the toast, which says first what the paste moved ("Moved 12 terminals of -X1021 to page 102.") in place of the Cut's toast, names the old and the new tag with Keep -X1011 beside Undo, which freezes the old tag (`fields.set_many` of `tag.frozen`, one step; Keep the old tags when several changed). A strip whose terminals a cut or a delete left behind is deleted with them, their wires and bridges from the Devices tree when none of them is drawn by a symbol of its own (such a wire is drawn on no page); otherwise `edit.delete` refuses in words, naming the terminal and the page that draws it. The identity-keeping move of a symbol across pages is `rep.move` (§3.4). On Windows, Shift+Delete also cuts (egui-winit turns it into the Cut event, §6). A fragment representation off its own grid is refused, a fragment of another grid than the page's is fitted to it (§8 item 3), and a paste on a page that takes no symbols is refused with its reason. The pasted fragment floats with its middle at the pointer, its anchor snapped to the grid, moved in to stand two grid steps inside the drawing area where it fits, its numbers clear of the frame; Duplicate starts two grid steps from the copied anchor. The clipboard is text through egui's clipboard (arboard), whose Wayland and X11 behaviour is a manual check.

The fragment also carries page items and structure boxes (ranks 43/44); marker associations retain and remap copied connections, empty markers/unknown variants are skipped and counted, and cross-project structure-box paste clears `external`. Copy remains partial-selection tolerant; Cut refuses an incomplete selected page-item transfer before side effects. `CopiedFragment::incomplete_page_items` counts each selected page item omitted or copied with fewer marker associations once. Cut performs one copy and, when that count is nonzero, refuses with `reason-cut-incomplete-items` before any clipboard/outbox, selection, model or revision change: “Cut would lose page items or marker connections; include every marker wire or use Copy”. A complete Cut keeps one delete/undo step and the device-left-unplaced rule. The existing translated `reason-cut-incomplete-items` text is displayed through banner-if-none, preserving Failed precedence, other Refused behavior and the side-effect-free guard. The shipped clipboard feature is enabled and guarded: on the Windows build, keyboard Ctrl+C and Edit ▸ Cut each publish the fragment to the OS clipboard, and Ctrl+V plus placement renders changed external fragment text. An incomplete Ctrl+X or Edit ▸ Cut shows the exact reason and preserves the marker, selection and OS clipboard; ordinary-text Ctrl+V stays quiet.

#### Paste in place

Edit ▸ Paste in place (`edit.paste_in_place`) puts the clipboard's block down with its anchor where the copy's anchor
was: at the same page coordinates, so a block copied from one page lands on another page of the same frame exactly
where it stood. It is one `edit.paste` with `at` the fragment's anchor, one undo step, and Select takes over with the
pasted symbols selected, as after a paste. On a page of another connection grid the block is fitted to that grid as
every paste is (§8 item 3). On a page of another size the block keeps its coordinates; where a part of it would then
lie outside the page's drawing area it is refused in words, as Paste refuses it, and the block hangs on the pointer by
its middle to be placed by hand. It has no shortcut: Ctrl+Shift+V is kept for paste as representation.

**What may lie outside the drawing area.** A block's free texts and graphics may lie anywhere on the sheet, the frame's
bands included, as the Text tool puts them; only its symbols, wire markers and structure boxes must lie inside the
drawing area, and Paste is refused only for them (it was refused for a text in the header band). The fragment's header
names the ghost rectangles of free texts and graphics (`free`, doc 05 §12); a fragment without it keeps every rectangle
inside the drawing area.

#### A copy of pages

A second, identical installation is the same pages again with new folios, new tags and, where the set uses one, a new
structure node. Page ▸ Copy pages… makes it in one step that keeps together what belongs together. It is the page
macro's fragment of the pages ([07](07-symbols-parts-macros.md) §3.5), widened by what an installation needs and
pasted by `edit.paste`'s engine with the pages' positions kept: no second machine.

**What a person does.** Mark the pages in the Pages navigator (else the page under its cursor) and choose Copy pages…
(Page tab; the page row's right-click menu). The form asks for the rule:
- **Folio offset**: a whole number added to each copied page's folio number (1000 makes 6001 into 7001; a label that
  begins with the folio number follows it, 6001.1 becomes 7001.1). The copy goes into the section whose folio range
  holds its new folio; a page of no section stays in none. Without an offset, **After** names the page after which the
  copies go, default the last copied page, and the folios are proposed after it as a page macro's are (§5.6). The two
  are one choice: typing into one clears the other.
- **Structure**: a structure node the copied pages or their devices name, and the code of the copy's node (CAB and
  CAB2): the copy names the node of that code beside it (same aspect, same parent), which is created in the same step
  when there is none.
- **Rename**: pairs of an old and a new text for the copies' page titles and descriptions, frozen tags and
  interruption-point names (6 and 7; 24V and 24V.2; Inst 1 and Inst 2), as the line over the table says. The table
  opens with one empty row, its old text's field showing a hint, and a row added is empty too; the card lists the titles that change, old → new. Every occurrence is replaced, at each place the longest old text first, in one pass: 26 → 27 and 6 → 7 together
  turn 26K6 into 27K7. Its rows are named for a screen reader by the table's own word, as its add button is: "New
  text, text to change 2", "Remove text to change 2" (Save as macro…'s rows read "value set 2").

The Preview card names the new pages and tags and what the copy leaves out; Apply makes the copy, one undo step, and a
toast names the new folios. A copied bus device (a valve terminal's bus node, a PLC station) joins no bus and takes no
address of its own: the card lists the bus devices, and the toast says to draw their bus line and give each its address. A script runs `pages.copy {pages, offset | after, node, code, rename: {old: new}}`; the
command it commits is `page.copy` (`CopyPages`), whose parameters are the pages as a fragment (built by
`Session::copy_pages`) and the rule, so a command log replays it exactly.

**What the copy keeps together.**
- A **device** drawn on a copied page comes whole: all its functions, its part and its data, and every representation
  of it on a copied page. A coil and its contacts on copied pages are one device in the copy, and their
  cross-references point between the copies. A function drawn only on pages that are not copied is in the copy without
  a place: the card names those devices (decided: not refused, since an installation may share a page with another; the
  person sees which copies to complete).
- A **strip** comes with its terminals on the copied pages, which keep their designations, order and bridges; its
  terminals on other pages stay with the original strip only.
- A **cable** with a core on a copied page comes whole with all its conductors; its cores on the copied pages are the
  copy's cores, its cores elsewhere stay the original's. A bus line keeps its cable the same way.
- A **bond** whose two ends are copied comes along, and so do PLC channels and device links between copied devices, as
  a page macro takes them.
- **Interruption points** keep their names unless Rename changes them; external points keep their name and their
  reference, since they never pair. The copy is refused, naming the points, where a copied point would join the points
  it was copied from: a name whose points all lie on the copied pages, or whose copied points include a source, that
  neither Rename nor the structure rule (a new node in the aspect the project's pairing is scoped by) gives a pairing of
  its own. A copied target of a name whose source stays outside the copy (a supply both installations share) continues
  it, and the card says so. So does a name with no source anywhere in the project whose copied points are all targets,
  whether or not the copy takes all of its points: a supply that comes in from outside the drawing (L, N, PE drawn as
  targets only) feeds the copy too, and Rename still gives the copy its own name where the person wants one.
- **Parts** stay the project's copies. **Texts**, graphics and wire markers come at their places; a text bound to the
  folio or the title shows the copy's. A free text whose whole text is the printed tag a copied device had before the
  copy, with or without its sign (the strip's name beside its first terminal, a cable's name at its cores, as an
  imported set writes them), prints the copy's tag in the same form, and one whose whole text is the name of a copied
  interruption point that Rename changes prints the new name; a text that only holds a tag inside a longer text, or
  names a record outside the copy, stays as it is, and so does one whose tag several copied devices shared and their
  copies no longer share, which the card names. The card counts the texts that change. A **report block** embedded on
  a copied page comes with it; report blocks that make pages of their own are not copied: they list the whole project,
  the copy included.
- What the project holds is copied **as it holds it**: positions as they stand (a symbol off the grid too), symbol
  types on a page of their source's kind, as many records as the pages hold, and a state a check reports in the
  source (two terminals of one designation on a strip, a bridge group that breaks CI-06, a core on two wires, a core
  colour the profile does not know, two valves at one position) is reported in the copy too instead of refusing it. A
  copy can only reproduce those states, so their prevent rules (EW-TERM-001, EW-TERM-002, EW-TERM-004, EW-CORE-031,
  EW-CORE-036, EW-FL-013) do not judge it.

**How tags are made new.** Every numbered device keeps its counter, so its tag follows the project's numbering scheme
from its new page (-6001K2 becomes -7001K2); online numbering gives the next free counter only where two copies would
meet on one page. A frozen tag is renamed by Rename and stays frozen. One that Rename leaves as it was stays frozen too
where the copy stands in a structure node of its own (the structure rule gives the device, or the strip, parent, box or
page it takes its structure from, another node), since its full designation differs from its original's (+CAB2-X5
beside +CAB-X5); it is cleared only where the copy's full designation would equal its original's, so the copy takes
the scheme's tag from its new page (a copy never prints its original's designation), and the card counts those. A
terminal prints its strip's tag and its designation, a child its parent's tag, a cable the page of its first core: a tag
that carries no page is therefore a frozen one.

**Refused in words**, nothing copied, each with "Copy pages:" before it: no page; a page named twice; a page that
takes no symbols (title page, panel layout, model view, external document) or a generated one; pages of different
connection grids (a choice of pages, not a state: copy each grid's pages with the same rule); a new folio that a page,
a section's divider or a report block holds, or one that no section's range holds where the page had a section; an
offset and After together; a structure node the copied pages do not name, or a code that is no identifier; the
interruption points above; and the prevent rules a copy can break anew: a second source of a name (EW-CORE-052), a
manual PLC address, symbol or slot met again in the project's one configuration project (EW-PLC-001, EW-PLC-007,
EW-PLC-011), a name Rename makes confusable (EW-RDS-009).

**What it does not do.** Copy into another project (copy and paste a page's content, or save and insert a page macro);
give the copies other counters (Tools ▸ Renumber devices… afterwards); a folio rule other than an offset or After;
change texts other than frozen tags, interruption-point names and the free texts that are a copied device's tag or a
renamed point's name (titles and other texts: Find and replace afterwards); keep a wire's frozen number (the copy's
wires are numbered anew, as pasted ones are, so no number prints twice);
copy a panel layout (the second cabinet is laid out on its own); give the copy its own PLC configuration project or bus
net (a copied PLC station joins the project's as a pasted one does, doc 10, and a copied bus port is on no net until it
is joined to one).

---

### 3.7 Selection, multi-select and bulk edit

- The selection is a set of entity references (representations, connections, junctions, page items, boxes) that
  stays in sync with the navigators in both directions (FR-UI-007).
- Selection filters: only connections, only representations, same device, same symbol, same potential, all
  representations of the device (FR-DEV-019: for -201U1 the list shows 201-E1, 3-B2, 106-G3 and the folio-902
  placement).
- The property grid shows the fields common to the selection, `‹varies›` where values differ, and derived values
  greyed with their provenance ("0.75 mm² · from potential 24VDC-1 via profile 'DBU/WH 24 V'") plus "reset to
  derived" (FR-UI-009, FR-UI-010, FR-CON-025).
- One edit over a multi-selection is **one** `SetFields` command (FR-DEV-031): setting the part of 20 selected
  -X1031 terminals is one undo step.
- Find and replace across tags, function texts, technical data, part numbers and user properties is a previewed
  transaction (FR-DEV-032, FR-UI-020).
- Identifier fields are NFC-normalised and flagged for confusables on input (FR-NUM-016). `MlText` fields edit the
  neutral text or the project languages, with mutual exclusion (FR-I18N-001).

---

### 3.8 Texts and graphics

`PageItem` kinds ([03](03-domain-model.md) §6): `Graphic` (line, polyline, rectangle, circle, ellipse, arc, polygon;
stroke widths and dash patterns of FR-SYM-003), `FreeText` (multilingual, styled, or bound to a project or page
property, FR-SYM-024), `Image` (content-addressed SVG or PNG, FR-SYM-025) and `WireMarker`. Dimensions are 1.0
(FR-SYM-026). Graphics have no electrical meaning (FR-SYM-023) and sit on the graphic layer; layers with visibility
and print flags are 1.0 (FR-PAG-026).

A **path function text** is a free text tied to a column range. It becomes the automatic function text of the
representations in those columns unless they carry a manual text (FR-DEV-011). It is a `FreeText` with role
`PathFunction{columns}` ([03](03-domain-model.md) §6).

Mechanical linkages between related functions (an actuator and its contacts; a valve and a process valve) are
derived dashed lines, not graphics (FR-SYM-014, FR-DEV-028). Dragging the actuator moves its linkage, and the
linkage cannot be selected as a free line.

**Texts the schematic carries.** A drawing carries texts beside its symbols that a person reads as part of
the model: a cable's name and type, the core numbers at its wires, a strip's name, a device's function text over its
current path, a note. Each is one of these, and only the last follows nothing:

| Text | What it is | It follows | In the window | By command |
|---|---|---|---|---|
| A cable's name and type, a core's number at a wire | the cable mark: a wire marker across the cable's cores ([08](08-connections-terminals-cables.md) §4.5; doc 06a §5.2) | a rename of the cable, its part's type, a core's designation; its line moves with the mark | the Wire marker tool across the cores | `marker.place` |
| A strip's name, once on a page | the strip's tag that the program prints at its first terminal on the page ([08](08-connections-terminals-cables.md) §6.7), or, where the person wants it elsewhere, a **text of a symbol** bound to `strip.tag` | a rename or renumbering of the strip; a move of its terminal | Name the terminal strip here, in the canvas menu over one terminal: the text right-aligned one step left of the terminal, level with it, to be dragged where the drawing wants it | `item.place {free_text: {bind: "strip.tag", of: <the terminal's symbol>, at}}` |
| A device's function text over its current path | a **text of a symbol** bound to `device.function_text` (the symbol's own function-text slot prints beside the symbol, where its author put it) | a change of the function text; a move of the symbol | Function text over its path, in the canvas menu over one symbol of a device with a function text: the text centred on the symbol's column, its baseline a step above the highest wire or symbol of that column (over its current path), to be dragged where the drawing wants it | `item.place {free_text: {bind: "device.function_text", of: <the symbol>, at}}` |
| A note on a page | a free text | nothing: it is the person's words about the page, which no record holds | the Text tool | `item.place {free_text: {text, at}}` |

A **text of a symbol** is a free text whose optional member `of` names a representation on its page. It moves with
that symbol whichever command moves it (normaliser 18a, §3.2), goes with it when it is deleted (a cascade link,
load-tolerant: an older build keeps `of` and leaves the text, which then stands alone and prints a symbol's binding
empty), and with one of the symbol's two names prints that symbol's value: `device.function_text` its device's
function text, `strip.tag` the tag of the strip whose terminal it draws, as the page prints that tag. A strip named on
a page by such a text is named there by it alone: the program prints that strip's tag on that page no more, so the
name stands once. The QElectroTech import writes an element's own texts that show no slot of its symbol as texts of
that symbol ([14](14-interop.md) §4.4.2), so they follow the element as its slots do.

*Refused in words:* a symbol's name without its symbol ("`device.function_text` belongs to a symbol: name the symbol it
prints for"), a page's name with one, a symbol that does not exist or stands on another page than the text, a strip's
name of a symbol that is no terminal of a strip; Function text over its path on a device without a function text
("type it in Properties first").

*Where a person finds it.* The device's Function text row says in its help where the text prints
(the lists; beside the tag where the symbol has a place for it; else over the path with Function text over its path),
and its … menu offers Function text over its path while one of the device's symbols is selected.

*What it does not do:* bind a text to a cable or a core (the cable mark is that text), to a page column range (the path
function text of FR-DEV-011, below, is not built), or to any other value of a device; turn with its symbol (a turn
moves it by the symbol's displacement only); and it adds no setting: where the text placer may place a text is doc
06a §5.4's rule.

Text, Text edit, Graphic, Wire marker and Structure box are implemented through the tool host (§4.5); Image remains part 5.

---

### 3.9 Undo granularity

One user gesture is one transaction (ARCH §5.3, G3). The history panel names each entry by its command and origin
(FR-UI-023).

| Gesture | Transactions | Coalescing |
|---|---|---|
| Place a symbol, including its autoconnect connections and online numbering | 1 | none |
| Drag a selection (press to release) | 1 | one per drag |
| Keyboard nudges of the same selection | 1 | merge until the selection changes or another command runs |
| Rotate, mirror or cycle variant of a placed selection | 1 | consecutive presses on the same selection merge |
| Draw a wire with intermediate bend clicks | 1 at the final click | bends are transient |
| Edit one property field | 1 | keystrokes merge until focus leaves (FR-UI-009) |
| Edit one field over a multi-selection | 1 | as above |
| Paste, duplicate, insert macro, quick fix | 1 each | none |
| Page renumber, device renumber, find and replace | 1 each, committed from the preview | none |
| Junction sequence reorder | 1 | consecutive reorders of one junction merge |
| Zoom, pan, page change, selection, highlight | 0 | not model changes |

Escape cancels any tool without a model change (FR-UI-016, NFR-UX-07).

The "Coalescing" column above is not
built: **there is no undo coalescing**, so each keyboard nudge, each rotate press and each junction reorder
is one undo step, in the window and headless alike. Headless commits go through `run_script`, which carries no
coalescing key, and equal undo stacks in the two front ends weigh more than the merging; merging would come to the
window and to headless together, with a key that `run_script` also carries. The other rows hold: a drag, a placement with its autoconnect connections and a wire with bend clicks are one transaction
each.

A placement is one transaction; placing a starter symbol first embeds it with its function definition as its own undo step (`library.embed`), so the first placement of a starter symbol is two steps and the next is one. A drag, each keyboard move (M, then the arrow keys and Enter), each rotate, mirror or variant press and each exchange is one step, with no coalescing. **A property edit is one commit:** the grid writes nothing while keys are typed and commits one `fields.set` (or one `project.settings.set` member, from its preview card) when the field is left or Enter is pressed, so the row "Edit one property field" holds without coalescing; a multi-selection edit is one command with every target; Enter without a change writes nothing. A commit on several records shows a toast whose Undo takes back that one step.

A wire drawn with bend clicks is one `connection.draw` at the final click (an end on a wire adds the junction and the split in the same transaction); a segment drag, a bend drag or a keyboard route edit is one `connection.route`; each junction reorder, style or wiring change is one `junction.edit`, with no coalescing; the crossings toggle is one `project.settings.set`. A replace of find and replace is one `fields.set_many` committed from its card, one undo step however many records it changes; Freeze on several devices likewise.

---

### 3.10 Online checks while drawing

Online rules are memoised queries `checks(scope)` over page, potential, strip and station scopes, so a wire edit
re-evaluates only its page and potential (ADR-0019). Findings show in the message panel (FR-CHK-008, FR-UI-021) and
on the canvas as a severity icon at the target's anchor plus a patterned halo (FR-UI-025). Hovering shows the
message, the rule ID and the quick fixes.

Rule IDs, kinds and scopes are those of the [15](15-checks-revisions-collaboration.md) §4 catalogue, whose Kind
column (kinds per 15 §2.3) decides the pending display below; this document defines only how each kind is shown
while drawing. A scope is the memo key of `checks(scope)` (15 §2.3); prevent rules are evaluated at commit, on the
candidate snapshot, not as a memo scope.

| Rule (requirement) | Kind | Scope | Shown | Reference example |
|---|---|---|---|---|
| Prevent rules: EW-RDS-004 tag unique (FR-NUM-012), EW-TERM-001/-002 terminal designation non-empty and unique (FR-TRM-002), EW-CORE-052 at most one star source per IP name and scope, an even count in chain mode (FR-CHK-017) | conflict | device; strip; potential; evaluated at commit | the commit is refused with the reason (strict mode) | a second -102F1 under `=P01+J-E-2416` |
| EW-POT-013 N switching (FR-CHK-019) | safety | potential | immediately, error | T55 and S56 on 101 (EF-01, D-13) |
| EW-POT-001 type conflict without bond (FR-CHK-028) | conflict | potential | immediately, with quick fix "declare bond" | 0V wired to -X1032 on 103 (D-22) |
| EW-POT-006 PE smaller than line (FR-CHK-020) | safety | potential | immediately, error | W6 on 101 (EF-06, D-38) |
| EW-CORE-051 unpaired IP (FR-CHK-017) | completeness | potential | pending while the page is being edited | PE-1.1 before 103 exists |
| EW-POT-009 unconnected CP (FR-CHK-018) | completeness | page | pending while the page is being edited | -102F2:13, :14 (EF-09, D-23) |
| EW-CORE-041 device without part (FR-CHK-024) | completeness | device | pending while the page is being edited | S56, E56 (EF-04) |
| EW-CORE-034 external connection without cable (FR-CHK-033) | completeness | page | pending while the page is being edited | J-E-206 → 1X1 (EF-07, D-19) |
| EW-CORE-053 xref target cell empty or outside the frame (FR-CHK-035) | hygiene | page (one finding per cross-reference) | immediately, warning | "3-B0" (D-14) |
| EW-NUM-001 frozen tag deviates (FR-CHK-025); EW-RDS-003 letter code (FR-CHK-026) | hygiene | device | info; warning | S51 … 1X1 (EF-15); T55 (EF-18) |
| EW-CORE-061 printed cap height below minimum (FR-CHK-023) | hygiene | project (one finding per text style) | immediately, warning | styles `cp.small`, `marker` (EF-12, D-31) |
| EW-CORE-071 graphic line ending on a CP (FR-CON-001) | hygiene | page | info, quick fix "convert to connection" | a QET-imported line drawn onto a CP (§8 pitfall 5) |

**Pending findings.** Completeness rules (unconnected CP, unpaired IP, missing part, missing cable) fire on every
half-drawn circuit. On the page being edited they show in a muted "pending" style and do not raise the error
counter until the user leaves the page or runs a check; they always appear at full severity in check runs, exports
and the release gate (FR-UI-041). This is the `drafting` scheme of
[15](15-checks-revisions-collaboration.md) §4.9. Safety and conflict rules are never muted. An online rule has a
budget of 5 ms per commit (NFR-PERF-09); one that exceeds it three times in 100 commits is demoted to deferred for
the session and logged (FR-CHK-013, 15 §3.1).

