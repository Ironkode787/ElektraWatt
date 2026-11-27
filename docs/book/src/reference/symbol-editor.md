# Drawing a symbol, and automated authoring

A symbol is drawn in **Parts**, on its **Drawing** tab, and kept with your parts in My parts. The ways in:

- **Draw ▸ Place ▸ New symbol** (`Ctrl+I`), the symbol picker's foot (**Nobody has it? New symbol…**) and a placed
  symbol's **Draw one like it…**: a new drawing for the page in front, at once, with no question first.
- **New symbol** on the Part tab of Parts: a symbol of your own, for no page.
- The **Symbols** shelf of Parts, and **Open in Parts** under a symbol in the symbol picker: a symbol you have.
- A piece's **Symbol** cell on a part's sheet: **Draw a new symbol…** and **Change it for this part…**
  ([below](#the-symbol-of-a-piece)).

A drawing has its own undo history. Changes do not alter existing placements. Press **F1** on the drawing to open
this reference.

## The screen

A new drawing opens ready to draw on: on the connection grid of the page it is drawn for (4 mm for a symbol of no
page), as what you drew last (a normally open contact the first time), with the texts its kind needs where the
standard puts them (the device tag at the upper left, and under it a contact's or a coil's cross-reference), and with
your drawing grid, **Snap to grid** and the tool you left the last time.

- **The strip** over the drawing says what it is drawn for: "A new symbol for page 102 Control.", "A symbol like
  -102S1's, for page 102.", or the piece of the part it draws.
- **The bar** holds the way back (**Back without it** for a page, **To the part** for a piece, else **Back**), the
  tools (**Select**, **Line**, **Rectangle**, **Circle**, **More shapes** with ellipse, arc, polyline and polygon,
  **Text**, **Pin**, **Text field**, and **Other marks** with the cross-reference point and the mechanical link point),
  the **Drawing grid** and **Snap to grid**, **Test on a page** and **Export draft**, and at its right end the next step:
  **Save and place on 102** for a page, **Save and use for -102S1** for a drawing like a placed symbol, **Done** for a
  piece, else **Save** (`Ctrl+Enter`). Each tool's tooltip says what it does and how.
- <a id="the-outline"></a>**The short sheet**, on the left: **Name** and **What it is**, typed and chosen where they stand, and what follows
  from them: where the symbol is **Listed under** and the grid its pins stand on. A symbol that is not yours (a starter
  symbol, one of a library you cannot write, one only in this project) reads as it is, with a line that says why and
  **Make my own from it**. A saved symbol of yours offers **New part drawn with it…**. Under it **the outline** lists
  what the symbol consists of: its pins (designation, and the potential each carries), its texts and its shapes; a
  click on a row selects the thing in the drawing, and the other way round. **To do** lists what keeps the symbol
  from being saved, each in a sentence and each a click to the thing it is about; with nothing left it says **Ready
  to save**. An import with more drawings lists them first, and an import's report stands under the lists.
- **The drawing**, in the middle, with the symbol **Turned and mirrored** under it: A is the drawing you edit, B to H
  show it turned and mirrored.
- **Properties**, on the right, shows the rows of what is selected: a shape's place and size, a text's words, style,
  alignment and rotation, a pin's designation, potential, direction and place. Several selected things show the rows
  they share, and a change goes to all of them. With nothing selected it says so. A row commits when you press Enter
  or leave it, so partially typed numbers do not reshape the drawing; Escape cancels the entry.

## Selecting and moving

What you know from a page works on the drawing. With **Select**: a click selects a thing, `Shift` adds and `Ctrl`
toggles; a rectangle dragged from empty paper selects the things inside it when dragged to the right and the things it
touches when dragged to the left; `Ctrl+A` selects everything; a click on empty paper or `Esc` clears the selection.
A shape that is not filled is taken by its line, a filled one by its face: a click or a drag inside an empty box is on
empty paper.
A drag on a thing moves the selection, with an outline of where it lands; the arrow keys move it one grid step. A
pin moves on the connection grid, and so does a selection that holds one; anything else moves by the drawing grid's
step. `Delete` removes the selection (a pin with the text that writes its number), `Ctrl+C`, `Ctrl+X` and `Ctrl+V`
copy, cut and paste, `Ctrl+D` duplicates, and `Ctrl+Z` and `Ctrl+Y` step the drawing's own history. The one selected
shape shows handles on its ends or corners: drag one to reshape it. With several things selected, **Line up** in
Properties puts them on one edge. Each change is one undo step.

## Drawing

<a id="a-shape"></a>A line, a rectangle, a circle, an ellipse and an arc are drawn by one drag or by two clicks; the circle takes its
centre and a point on its circumference, arcs and ellipses opposite corners of their box. A polyline and a polygon take
a click per corner and a double-click on the last. **Text**: click where the text goes, type it and press Enter
(`Shift+Enter` starts a new line); a new text takes the style you chose last. What a tool drew is selected, so its
rows in Properties take exact numbers, an arc's angles, a shape's **Fill shape** or a text's **Text style**.
<a id="a-connection-point"></a>**Pin** places a connection point on the connection grid, with the text that writes its number; the card at the
drawing's top left holds its designation, which counts on, and its direction: **Along its lead**, the first, faces the
way the line it ends points, so a wire carries straight on from it. **Text field** places a text the page fills in:
the device's tag, its function text, its technical data, its cross-reference, or the number of a pin. `Esc` ends a
half-drawn shape, then returns to **Select**.

**Cross-reference point** (under **Other marks**) sets the point used to calculate the symbol's cross-reference cell
on the project page. It does not move the symbol's origin, the small cross it is held by when it is placed. Place it
on the left edge for a supply block. **Mechanical link point** adds a named point where a dashed mechanical link
attaches; it makes no electrical connection.

**Drawing grid** chooses the step that lines, shapes, texts and link points snap to (0.5, 1, 2 or 4 mm, or QET 10 px;
0.5 mm at first), and **Snap to grid** turns that snapping off; both stay as you leave them for the next drawing. The
drawing shows the drawing grid's dots and, larger, the connection grid's. A pin keeps to the connection grid whatever
is chosen; one typed off the grid stays where it was typed, shows under **To do** and keeps the symbol from being
saved.

## What the symbol is

**What it is** asks what the symbol stands for, in a panel builder's words: a device box, a normally open contact, a
coil, a protective device, a pressure, position or temperature switch, an indicator light, a heater, a terminal. It
decides how the program lists and checks the symbol's device (a contact is listed with its coil, a terminal in the
terminal diagram) and where the browsers list it, which **Listed under** follows. A symbol drawn for a fluid page is
drawn as pneumatic, and its kinds are those of fluid power; one pin can differ, in its rows.

## Saving, testing and leaving

The next step saves the symbol with your parts, in My parts (a symbol of a library you can write is saved back
there), and finishes what it was drawn for: **Save and place on 102** shows page 102 with the Place tool armed and the
symbol under the pointer, one click places it, and Undo takes the placing back, not the symbol; **Save and use for
-102S1** puts it in place of the symbol it was drawn like, its wires kept; **Done** gives the piece its symbol and
returns to the part. You never choose a folder, a function type, a key or a library.

A symbol takes its key from its name (`user.power-block` for **Power block**); a second symbol of the same name takes
the next free one (`user.power-block-2`). Later saves of a symbol update it: changed content is saved as the next patch
version (1.0.1 after 1.0.0). A save says where the symbol went; one refused says why above the drawing and under
**To do**. Embedded copies stay at their version until you update them.

**Back** leaves the drawing in hand with its history: New symbol's **Go on with the drawing in hand** returns to it.
A new drawing over one with changes not saved first asks whether to save the symbol; so does quitting. While the
drawing shows, `Ctrl+S` saves the symbol and the File menu's **Save project** saves the project. **Export draft** writes
portable JSON to a new file, including incomplete work; an existing file is never overwritten.

**Test on a page** places a test copy on the page in front, or on a new page of the symbol's connection grid when the
page in front has another one; the page then shows with the copy selected, and the message's **Back to the editor**
returns to the drawing. Project undo removes the placement; a second undo removes the new page. Test placements are
independent copies; to place the symbol itself, save it and use **Draw ▸ Place ▸ Symbol…**, where it is listed with
its library's name.

## The symbol of a piece

A part drawn as pieces (a coil, a contact) shows in each piece's **Symbol** cell the symbol that draws it, the
starter's marked as such, and offers the symbols of what the piece is. **Draw a new symbol…** opens a drawing of the
piece's function with its pins standing on it already; **Change it for this part…** opens your copy of the symbol
that draws the piece now. The symbol it came from stays as it is; yours is kept with your parts and draws this piece
only. **Done** gives the piece the symbol; the part's Save keeps it, and a page draws the piece with it where the page
has it (a page of another grid takes the first symbol of the piece's function).

## The box of a part

A device that is a box with pins (a circuit protector, a power supply, a drive, a gateway) needs no drawing: its part
says which pins it has and where they stand on its box. **Arrange…**, under the part's pins in Parts, under such a
part in the device browser, and **Add pin** on a box of the part on a page, opens the box as the drawing of Parts.
There is one list of pins, the part's: a pin dragged on the box is its row changed. A part that is not yours is
arranged once you made your own from it.

- **The drawing** shows the box as a device of the part is drawn. Drag a pin along its edge or onto another edge;
  drag any edge or corner of the box to size it: the pins on that edge go with it, the others stay where they are,
  and the box gets no smaller than its pins need. A click selects a pin, `Shift` and `Ctrl` select more; the arrow
  keys move the selected pins one grid step along their edge and `Delete` removes them; `Ctrl+C` and `Ctrl+V` copy
  them and paste pins of the same names, each last on its edge; **Add pin** adds one.
- <a id="a-pin-of-the-box"></a>**Properties** shows the selected pin's **Name**, **Potential**, **Edge** and its place **Along the edge**; with no
  pin selected the box's width and height.

**Done** and **To the part** give the part's sheet the box; nothing is saved until the part is. A box part is laid out
when it is saved, so **Draw ▸ Place ▸ Device from part…** places its box with one click, laid out as you drew it; on the page its
edges and pins can still be dragged. Where boxes of the part are drawn already, Save's card names each box it draws
anew, and its Apply redraws them in one undo step. A PLC card takes its pins from its channels, a bus device its box on
a bus page from its interfaces and ports, and a part drawn with a symbol its connections from that symbol: the box
shows where its pins come from.

## AI and command-line tools

No remote service or generated code is required. Use `ew symbol`:

```text
ew symbol template example.power "Power block" > power.symbol.json
ew symbol schema > symbol-authoring.schema.json
ew symbol validate power.symbol.json
ew symbol publish power.symbol.json --library symbols.ewlib
ew symbol import element.elmt --key example.import > imported-drafts.json
ew symbol import manufacturer.edz --key example.import > imported-drafts.json
ew symbol import custom.slk --key example.import > imported-drafts.json
```

Template and schema print JSON. Validation prints stable issue codes, JSON pointers and details, exiting 0
when valid, 1 for validation findings, or 2 for unreadable/malformed input. Import prints an **array** of drafts;
choose one object and save it as a `.symbol.json` file. QET conversion findings are retained in
`import_details` with their original message, code and source line, so tools can identify each loss. Publication
validates again and uses the ordinary library writer's lock, version and transaction protections. It does not save the
current project. Without `--key`, import uses the file's name as the key. Publication prints `result` (`added`,
`updated` or `unchanged`) and the draft's `identity`; copy that `identity` into the draft file to publish later
versions of the same symbol. A draft without it is a new symbol, refused (exit 2) when the library holds its key for
another one. Validation reports a member it does not know as `symbol-unknown-member`, which blocks publication.

The portable envelope uses `format: "ew.symbol/1"`, `key`, `version`, `symbol`, `function`, optional
`identity`, `function_ref`, `import_notes`, `import_details` and `import_reviewed`. File coordinates are integer
**nanometres**; 4 mm is `4000000`. A pin has `idx`, `fn_cp`, `pos: [x,y]` and outward `dir`. The function's pin with
the corresponding `idx` holds its printed `designation`. Keep the reference created by the template; the publisher
replaces temporary references for owned functions.

The window and `.ewui` scripts share these actions:

| Action | Parameters |
|---|---|
| `parts.new_symbol` | optional `like`: `last` (a copy of the symbol saved last), `selected` (like the symbol selected on the page), `hand` (the drawing in hand again); optional `discard: true` |
| `parts.open` | `symbol`: `library:<library UUID>:<item UUID>`, `id:<copy UUID>` of the project's, or a key |
| `parts.draw` | `what: "box"` (optional `part`), or `piece` (its index on the sheet) with `from` (`new`, `change`); without them the drawing in hand; `done: true` back to the Part tab |
| `parts.field` | `field`: `symbol.name` or `symbol.what` (a word of What it is), `value` |
| `parts.save`, `parts.back`, `parts.own`, `parts.discard` | none: the next step, the way back, Make my own from it, Discard |
| `parts.find` | `shelf`: `parts` or `symbols`, with `text` and `scope` (`all`, `project`, `mine`) |
| `symbol.open` | `file`, optional import `key` (default: the file's name), optional `discard: true` |
| `symbol.candidate` | zero-based `index` from an import package |
| `symbol.draw` | `tool`, `x`, `y`, optional `x2`, `y2`, `text`, arc `start`/`sweep` in whole degrees |
| `symbol.path` | `points` as millimetre pairs, optional `closed` for a polygon |
| `symbol.cp` | `x`, `y`, optional `idx`, `fn_cp`, `designation`, `direction` (`up`, `right`, `down`, `left`, or `lead`: along the lead that ends there) |
| `symbol.slot` | `kind`, `x`, `y`; e.g. `"tag"` or `{"cp_designation":1}` |
| `symbol.link` | `name`, `x`, `y` |
| `symbol.undo`, `symbol.redo`, `symbol.validate` | none |
| `symbol.variant` | `variant` 0–7 for A–H |
| `symbol.export` | `file` |
| `symbol.tool` | `tool`: the drawing tool to choose |
| `symbol.select` | `items`: a list of things (`cp:0`, `slot:1`, `graphic:2`, `link:0`, `anchor`, each by its place in its list) or `"all"`; optional `mode`: `replace`, `add` or `toggle` |
| `symbol.move`, `symbol.nudge` | `dx`, `dy`: the selection moved by millimetres, or by grid steps |
| `symbol.delete`, `symbol.copy`, `symbol.cut`, `symbol.paste`, `symbol.duplicate` | none; they act on the selection |
| `symbol.align` | `to`: `left`, `centre`, `right`, `top`, `middle` or `bottom` |
| `symbol.grip` | `grip` (a handle of the one selected shape, counted from 0), `x`, `y` |
| `symbol.prop` | `key`, `value`: a row of Properties of the selection (`x`, `y`, `w`, `h`, `dir`, `role`, `discipline`, `style`, …) |
| `symbol.grid` | optional `step` in millimetres, optional `snap` |
| `part.pin` | `add: true` with optional `name`; `remove`: a pin; or `pin` with `name`, `role`, `side` (`top`, `bottom`, `left`, `right`) and `at` in millimetres along the edge |
| `part.box_size` | `width`, `height` in millimetres; `from_left: true` changes the width at the left edge and `from_top: true` the height at the top |
| `part.box_select` | `pins`: a list of pin numbers or `"all"`; optional `mode` |
| `symbol.place` | `x`, `y` on the current project page |
| `symbol.scratch` | none; Test on a page |

Action coordinates are **millimetres**, unlike JSON document coordinates. Placement snaps the insertion point to
the destination page grid; it never moves a pin within the symbol. A different symbol/page grid is refused
before changing the project, and the refusal offers a new page with the symbol's grid: **Place on a new page with
its grid** (`symbol.new_page`) places the saved symbol there and selects it while the drawing is saved and unchanged
since, and **Test on a new page** places a test copy otherwise. A new drawing or an import over a drawing with changes
not saved requires `discard: true` in a script. While a box is arranged, `parts.save`, `symbol.undo`, `symbol.redo`,
`symbol.delete`, `symbol.nudge` and `symbol.prop` act on the box. `app.quit` with `discard: true` discards a drawing
not saved. `ui.describe` and the script state expose the open drawing and its findings.
[The tutorial](../tutorials/symbol-editor.md) includes a complete six-pin example.

## Imported formats and limits

| Input | Supported behaviour |
|---|---|
| QElectroTech `.elmt` | Existing bounded element reader; preserves the QET 10 px grid and provenance. Review conversion warnings. |
| EPLAN `.edz` | Bounded 7-Zip or ZIP container; candidates from supported XML members. Never extracts files to disk. |
| EPLAN `.ema`, `.ems`, `.esl` XML | PXF revision **1.23**: straight lines, rectangles, polylines, full circles and text. Each macro variant becomes a separate candidate. |
| EPLAN parts XML | Connection designations become a generated rectangular outline. This is explicitly **not the original macro artwork**. |
| EPLAN `.slk` with its companion `.sdb` folder | Direct read without EPLAN: observed SLK 1.2 / FE53 tables, UTF-16LE or UTF-8 descriptor, A-variant lines, axis-aligned rectangles, full circles and graphical connection points. Review required. |
| Other binary macros | Refused with an explanation; use a supported XML macro/parts export. |

PXF arcs/ellipses, images, groups, external symbol references and unrecognised records are not faithfully
reconstructed. The importer reports incomplete geometry and style differences; unresolved macro pins have an
unknown direction and cannot be published until corrected. Macro insertion points are not verified terminal
positions. Check each position and mapping against the source. No claim of complete EPLAN compatibility is made.
The reference manufacturer EDZ was exercised locally; its content is not distributed with ElektraWatt.

EPLAN drafts require a licence/source review and explicit acknowledgement. A download is not treated as a
redistribution licence. No links or embedded executables are followed. Input limits: 4 MiB per XML/draft,
32 MiB compressed archive, 64 MiB unpacked total, 4,096 archive entries, 512 recovered candidates, 4,096 editable
primitives/pins per symbol; archive names, CRCs, decompression ratio and XML depth are checked.

## How the symbol data fits together

| Record | What it owns |
|---|---|
| Symbol | Artwork, insertion/reference anchors, text slots, pin positions and directions, and each pin's function mapping. |
| Function definition | Electrical type, pin designations, potential roles, optional/source flags and internal connections. |
| Manufacturer part | Purchased-product information such as article number, manufacturer and technical ratings; the symbol each of its pieces is drawn with; assigned to a project device separately. |
| Placed device/function | Device tag, function text, chosen part and embedded copies of the symbol/function definitions. |

## The function types

What **What it is** chooses sets one of these function types. The type decides which pins the symbol has and how its
device is listed and checked; which types are offered follows the symbol's discipline: a valve for a fluid symbol, a
contact or a coil for an electrical one, a port for a bus one, a black box for each.

| Function type | For |
|---|---|
| Generic black box | A device the program has no type for, such as a drive or a module, with connection points of its own. |
| Normally open, normally closed and change-over contact | A contact of a relay, contactor or switch; it is listed with the coil or device that switches it. |
| Coil | The coil of a relay or contactor; its contacts refer to it. |
| Pole | A path through a device, or out of a source: a pole of a breaker, fuse or switch, a supply or an air supply. |
| Terminal | A terminal of a terminal strip, listed in the terminal diagram. |
| PLC input/output | A channel of a PLC card, which gets an address and is listed in the PLC lists. |
| Bus port | A port such as PROFINET or PROFIBUS; its device joins a bus net. |
| Valve | A fluid power valve; its ports carry air or oil, and it can sit on a valve terminal. |

Typing `FE` as a designation changes the printed label; select the FE potential role to give it electrical meaning.
Text slots obtain tags, technical data and function text from the placed device; the drawing shows sample values.
Library publication does not update already placed copies. The EPLAN importer does not create complete manufacturer
part records, and SLK connection designations are provisional numbers until the function is reviewed.

Keep `custom.slk` beside the complete `custom.sdb` folder (or the sibling folder named in the descriptor).
Only the Symbol, Variant, Instance and Blob drawing tables are read. No EPLAN installation, script, workstation
record or lock file is used. The supported subset is based on an inspected public library sample; it is not a
vendor-certified database reader. Unsupported versions and legacy descriptor encodings are refused. Other
primitives, static text, fills and custom line styles are reported as incomplete. Only A is reconstructed; EW
then derives B–H, which can differ from separately authored EPLAN variants. Review geometry, every connection,
function semantics and licence before publishing. Missing connection records are reported, never invented.
Companion tables are limited to 4 MiB each and reconstructed streams to 64 MiB total; oversized libraries are refused.
