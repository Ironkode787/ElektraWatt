# Symbol editor and automated authoring

Open **Draw ▸ Place ▸ Symbol editor…**. A symbol draft has its own undo history, drawing, function,
connection points, text fields and cross-reference point. Changes do not alter existing placements. Press **F1** in the
editor to open this reference.

## The screen

With nothing in hand the editor shows a **start card**: **New symbol**, **Copy a library symbol…** (a list of the
project's and the libraries' symbols with a filter; the original stays as it is), **Device box for a part…**
([below](#the-device-box)), **Import a file…**, the draft in hand and the symbols edited lately. **Start** in the
command bar returns to it.

With a symbol in hand the screen is built as the main window is:

- **The command bar** holds the tools (**Select**, **Line**, **Rectangle**, **Circle**, **More shapes** with
  ellipse, arc, polyline and polygon, **Text**, **Connection point**, **Text field**, and **Other marks** with the
  cross-reference point and the mechanical link point), the **Drawing grid** and **Snap to grid**, and at the right
  end **Test on a page**, **Save**, the **Draft** menu (**Make a part for this symbol…**, **Export draft**,
  **Discard draft**) and **Close editor**. Each tool's tooltip says what it does and how.
- **The outline**, on the left, lists what the symbol consists of: its connection points (designation, and the
  potential each carries), its texts and its shapes. A click on a row selects the thing in the drawing, and the other
  way round. **To do** lists what keeps the symbol from being saved, each in a sentence and each a click to the thing
  it is about; with nothing left it says **Ready to save**. An import with more drawings lists them first, and an
  import's report stands under the lists.
- **The drawing**, in the middle, with the symbol **Turned and mirrored** under it: A is the drawing you edit, B to H
  show it turned and mirrored.
- **Properties**, on the right, shows the rows of what is selected: a shape's place and size, a text's words, style,
  alignment and rotation, a connection point's designation, potential, direction and place. Several selected things
  show the rows they share, and a change goes to all of them. With nothing selected it shows the symbol's own rows:
  **Name**, **What it is**, **Discipline**, **Listed under** and the **Library folder**; the rows a rare case needs
  stand under **More**. A row commits when you press Enter or leave it, so partially typed numbers do not reshape
  the drawing; Escape cancels the entry.

## Selecting and moving

What you know from a page works on the drawing. With **Select**: a click selects a thing, `Shift` adds and `Ctrl`
toggles; a rectangle dragged from empty paper selects the things inside it when dragged to the right and the things it
touches when dragged to the left; `Ctrl+A` selects everything; a click on empty paper or `Esc` clears the selection.
A shape that is not filled is taken by its line, a filled one by its face: a click or a drag inside an empty box is on
empty paper.
A drag on a thing moves the selection, with an outline of where it lands; the arrow keys move it one grid step. A
connection point moves on the connection grid, and so does a selection that holds one; anything else moves by the
drawing grid's step. `Delete` removes the selection (a connection point with the text that writes its number),
`Ctrl+C`, `Ctrl+X` and `Ctrl+V` copy, cut and paste, `Ctrl+D` duplicates, and `Ctrl+Z` and `Ctrl+Y` step the editor's
own history. The one selected shape shows handles on its ends or corners: drag one to reshape it. With several things
selected, **Line up** in Properties puts them on one edge. Each change is one undo step.

## Drawing

A line, a rectangle, a circle, an ellipse and an arc are drawn by one drag or by two clicks; the circle takes its
centre and a point on its circumference, arcs and ellipses opposite corners of their box. A polyline and a polygon take
a click per corner and a double-click on the last. **Text**: click where the text goes, type it and press Enter
(`Shift+Enter` starts a new line). What a tool drew is selected, so its rows in Properties take exact numbers, an
arc's angles, a shape's **Fill shape** or a text's **Text style**. **Connection point** places a point on the
connection grid; the card at the drawing's top left holds its designation, which counts on, and its direction.
**Text field** places a text the page fills in: the device's tag, its function text, its technical data, its
cross-reference, or the number of a connection point. `Esc` ends a half-drawn shape, then returns to **Select**.

**Cross-reference point** (under **Other marks**) sets the point used to calculate the symbol's cross-reference cell
on the project page. It does not move the symbol's origin, the small cross it is held by when it is placed. Place it
on the left edge for a supply block. **Mechanical link point** adds a named point where a dashed mechanical link
attaches; it makes no electrical connection.

Native connections must lie on the 4 mm grid. **Drawing grid** chooses the step that lines, shapes, texts and link
points snap to (0.5, 1, 2 or 4 mm, or QET 10 px; 0.5 mm at first), and **Snap to grid** turns that snapping off. The
drawing shows the drawing grid's dots and, larger, the connection grid's. A connection point keeps to the connection
grid whatever is chosen. The choice stays for the session. A connection point typed off the grid stays where it was
typed: it shows under **To do** and blocks the save.

## What the symbol is

**What it is** asks what the symbol stands for, in a panel builder's words: a device box, a normally open contact, a
coil, a protective device, an indicator light, a terminal. It decides how the program lists and checks the symbol's
device (a contact is listed with its coil, a terminal in the terminal diagram) and where the browsers list the symbol,
which **Listed under** changes on its own. **Discipline** says what the symbol is drawn for (electrical, pneumatic,
vacuum, hydraulic or bus); one connection point can differ, under **More** in its rows.

### Function type and ready-made functions

Under **More**, **Function type** shows the program's own word for what **What it is** chose, and **Ready-made
function** takes a function of a library when one fits: it brings its type and its connection points, so you do not
enter them yourself, and each connection point's **Function pin** row (under **More**) says which of its pins the point
draws. While a ready-made function is used the type, the category and the pins' names are locked; choosing **None**
unlocks them and keeps the function's type and connection points. Repeated designations have an ordinal in
parentheses, so two terminals both named N remain distinguishable.

**Whole document (for experts)**, under the symbol's rows, shows the complete draft as text, including what the rows
do not show (other text arrangements, an import's extra members). See the
[design standard](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/design/07a-symbol-design-standard.md)
for the full authoring profile.

## Saving, testing and closing

**Save** writes the symbol and its own function to the library named by **Library folder** (a `.ewlib` folder; a name
without a folder is a library in your user library folder, and **Choose folder…** picks one). **Make a part for this
symbol…**, in the **Draft** menu, then opens the part editor on a new part drawn with it: its identity, bus
interfaces and PLC data are typed there ([Make your own PLC or bus part](../how-to/own-parts.md)), and a device placed
from the part is drawn with the symbol.
The window remembers that folder. Its symbols appear under **Draw ▸ Place ▸ Symbol…** (`I`), with the library name;
the page must use the symbol's connection grid. Already embedded copies stay at their existing version until
you explicitly update them. Repeated placement keeps the authored outline and pins.
A new symbol takes its key from its name (`user.power-block` for **Power block**), and the key follows the name until
you type one. The first save gives the draft its own identity, and later saves of that draft update the same symbol:
changed content is saved as the next patch version (1.0.1 after 1.0.0), and a version you type below the library's
is refused. The symbol's own function keeps its version while it is unchanged. A library that already holds a key you
typed for a different symbol refuses the save: choose another key, or copy that symbol to change it; a key made from
the name takes the next free number instead (`user.power-block-2`). A save says where the symbol went, and a refused
one says why above the drawing and under **To do**. Saving a draft under another key than its symbol's makes a new
symbol, and a message says so; the symbol with the earlier key stays. **Export draft** writes portable JSON to a
new file, including incomplete work; an existing file is never overwritten.
**Close editor** keeps the draft and its undo history: **Draw ▸ Place ▸ Symbol editor…** returns to it. Only
**Discard draft** drops it. While the draft has unsaved changes, a way in of the start card, another drawing of an
import and **Discard draft** first ask whether to save the symbol. While the editor shows, Ctrl+S saves the symbol
and the File menu's **Save project** saves the project. Quitting with unsaved symbol changes asks whether to save the
symbol. **Test on a page** creates a page in the current project with the symbol's connection grid and places
embedded copies there. The page then shows with the copy selected; the message's **Back to the editor** returns to
the draft. Project undo removes the placement; a second undo removes the new page.
Test placements are independent draft copies, so **Only in this project** in the Libraries navigator refers to
their temporary draft source. To place a copy linked to the published library, use **Draw ▸ Place ▸ Symbol…**
after saving.

## The device box

A device that is a box with pins (a circuit protector, a power supply, a drive, a gateway) needs no drawing: its part
says which pins it has and where they stand on its box. The editor edits that box on the same screen. Its ways in are
**Device box for a part…** on the start card (every part drawn as a box, of the project and of the libraries,
placed or not, or a new part), **Pins and box…** in the part editor and under such a part in the device browser, and
**Edit this device's box…** in the menu of a box placed on a page. A part you cannot change (a starter or sample part)
opens as a copy of your own, and a line over the box says what Save makes: the project's own copy, which its devices
take, or, for a part not yet in the project, a new part in your own library. The original stays as it is.

- **The pin table**, on the left, lists the pins in the order they stand on the box, each with its name, the potential
  it carries and its edge. **Add pin** adds one; a row dragged onto another takes its place there.
- **The drawing** shows the box as a device of the part is drawn. Drag a pin along its edge or onto another edge;
  drag any edge or corner of the box to size it: the pins on that edge go with it, the others stay where they are,
  and the box gets no smaller than its pins need. A click selects a pin, `Shift` and
  `Ctrl` select more; the arrow keys move the selected pins one grid step along their edge and `Delete` removes them;
  `Ctrl+C` and `Ctrl+V` copy them and paste pins of the same names, each last on its edge.
- **Properties** shows the selected pin's **Name**, **Potential**, **Edge** and its place **Along the edge**; with no
  pin selected the part's number, manufacturer and description (typed here for a new part; **Part data…** opens the
  part editor for the rest) and the box's width and height.

**Save** saves the part: a new part into your parts library and the project, a part of your own in place, and a
library's part as the project's own copy, which the devices that used it follow. From then on
**Draw ▸ Place ▸ Device from part…** places the part's box with one click, laid out as you drew it; on the page its edges and pins can still be dragged.
Where boxes of the part are drawn already, the message of the save says how many and offers **Redraw them**, here or
back on the page; the message then says how many were redrawn. **Test on a page** saves the box and arms the Device
tool with the part. **Close box** returns where you opened the box: the page, or the editor's ways in, where a symbol
you were drawing waits to be continued.

A PLC card takes its pins from its channels, a bus device its box on a bus page from its interfaces and ports, and a
part drawn with a symbol its connections from that symbol: the editor shows such a part's box and says where its pins
come from.

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
`import_details` with their original message, code and source line, so tools can identify each loss. Publication validates again and uses the ordinary library
writer's lock, version and transaction protections. It does not save the current project.
Without `--key`, import uses the file's name as the key. Publication prints `result` (`added`, `updated` or
`unchanged`) and the draft's `identity`; copy that `identity` into the draft file to publish later versions of the
same symbol. A draft without it is a new symbol, refused (exit 2) when the library holds its key for another one.
Validation reports a member it does not know as `symbol-unknown-member`, which blocks publication.

The portable envelope uses `format: "ew.symbol/1"`, `key`, `version`, `symbol`, `function`, optional
`identity`, `function_ref`, `import_notes`, `import_details` and `import_reviewed`. File coordinates are integer **nanometres**;
4 mm is `4000000`. A pin has `idx`, `fn_cp`, `pos: [x,y]` and outward `dir`.
The function's pin with the corresponding `idx` holds its printed `designation`.
Keep the reference created by the template; the publisher replaces temporary references for owned functions.

The window and `.ewui` scripts share these actions:

| Action | Parameters |
|---|---|
| `symbol.new` | optional `key` and `name`, optional `discard: true` |
| `symbol.open` | `file`, optional import `key` (default: the file's name) |
| `symbol.candidate` | zero-based `index` from an import package |
| `symbol.fork` | `source`: embedded copy UUID, `library:<library UUID>:<item UUID>`, or legacy key; new `key` |
| `symbol.draw` | `tool`, `x`, `y`, optional `x2`, `y2`, `text`, arc `start`/`sweep` in whole degrees |
| `symbol.path` | `points` as millimetre pairs, optional `closed` for a polygon |
| `symbol.cp` | `x`, `y`, optional `idx`, `fn_cp`, `designation`, `direction` |
| `symbol.map_cp` | drawing `cp`, target function `fn_cp`; an occupied compatible pin swaps |
| `symbol.role` | drawing `cp` and the potential `role` of its function pin (`l1` … `undefined`; `none` clears it) |
| `symbol.slot` | `kind`, `x`, `y`; e.g. `"tag"` or `{"cp_designation":1}` |
| `symbol.link` | `name`, `x`, `y` |
| `symbol.function` | existing function `key`, or `library:<library UUID>:<item UUID>` in `key`; an empty `key` takes an assigned function off |
| `symbol.replace` | complete `document` object |
| `symbol.undo`, `symbol.redo`, `symbol.validate` | none |
| `symbol.variant` | `variant` 0–7 for A–H |
| `symbol.export`, `symbol.save` | `file` or library `directory` respectively |
| `symbol.directory` | library `directory` that Save uses |
| `symbol.tool` | `tool`: the drawing tool to choose |
| `symbol.select` | `items`: a list of things (`cp:0`, `slot:1`, `graphic:2`, `link:0`, `anchor`, each by its place in its list) or `"all"`; optional `mode`: `replace`, `add` or `toggle` |
| `symbol.move`, `symbol.nudge` | `dx`, `dy`: the selection moved by millimetres, or by grid steps |
| `symbol.delete`, `symbol.copy`, `symbol.cut`, `symbol.paste`, `symbol.duplicate` | none; they act on the selection |
| `symbol.align` | `to`: `left`, `centre`, `right`, `top`, `middle` or `bottom` |
| `symbol.grip` | `grip` (a handle of the one selected shape, counted from 0), `x`, `y` |
| `symbol.prop` | `key`, `value`: a row of Properties, of the selection or of the symbol itself (`name`, `what`, `category`, `key`, `x`, `y`, `w`, `h`, `dir`, `role`, …) |
| `symbol.grid` | optional `step` in millimetres, optional `snap` |
| `symbol.start` | optional `leave: true`: the start card, or back to the draft in hand |
| `part.box` | optional `part` (a part number, or `id:<copy UUID>`), optional `new: true`: the device box editor |
| `part.pin` | `add: true` with optional `name`; `remove`: a pin; or `pin` with `name`, `role`, `side` (`top`, `bottom`, `left`, `right`) and `at` in millimetres along the edge |
| `part.box_size` | `width`, `height` in millimetres; `from_left: true` changes the width at the left edge and `from_top: true` the height at the top |
| `part.box_select` | `pins`: a list of pin numbers or `"all"`; optional `mode` |
| `part.redraw_boxes` | `part`: the boxes drawn of the part redrawn as it lays them out, in one undo step |
| `symbol.place` | `x`, `y` on the current project page |
| `symbol.scratch` | none; creates a test page in the current project |
| `symbol.close` | optional `discard: true` |

Action coordinates are **millimetres**, unlike JSON document coordinates. Placement snaps the insertion point to
the destination page grid; it never moves a pin within the symbol. A different symbol/page grid is refused
before changing the project, and the refusal offers a new page with the symbol's grid: **Place on a new page with
its grid** (`symbol.new_page`) places the published symbol there and selects it while the draft is saved to a library
and unchanged since, and **Test on a new page** places a test copy otherwise. Replacing or opening over a dirty draft
requires `discard: true`; `symbol.close` keeps the draft unless it has `discard: true`, and `symbol.new` without a
key or name returns to it (with no draft in hand it shows the start card), or with `discard: true` starts a fresh
draft. While a device box is edited, `symbol.save`, `symbol.scratch`, `symbol.undo`, `symbol.redo`, `symbol.delete`,
`symbol.nudge`, `symbol.prop` and `symbol.close` act on the box. `app.quit` with `discard: true` discards an unsaved draft. `ui.describe` and the script state expose the open editor and
validation findings. [The tutorial](../tutorials/symbol-editor.md) includes a complete six-pin example.

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
| Manufacturer part | Purchased-product information such as article number, manufacturer and technical ratings; assigned to a project device separately. |
| Placed device/function | Device tag, function text, chosen part and embedded copies of the symbol/function definitions. |

## The function types

What **What it is** chooses is one of these function types (the row **Function type** under **More** shows it).
The type decides which connection points the symbol has and how its device is listed and checked; which types are
offered follows the symbol's discipline: a valve for a fluid symbol, a contact or a coil for an electrical one, a
port for a bus one, a black box for each.

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
Text slots obtain tags, technical data and function text from the placed device; the editor shows sample values.
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
