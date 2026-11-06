# Symbol editor and automated authoring

Open **Edit ▸ Symbol editor…**. A symbol draft has its own undo history, drawing, function definition,
connection points, text slots and cross-reference anchor. Changes do not alter existing placements.
Choose A to draw; B–H show live rotated/mirrored previews. Select a graphic to move it by dragging or edit
its coordinates in **Drawing**. **Draw by coordinates** also creates geometry from the keyboard.
Pin coordinates, directions and function mappings are editable in **Electrical data**.
In **Pin mapping**, drag a function pin onto a connection point or use its list. Assigning a pin already used
by another compatible connection swaps their mappings in one undo step. Repeated designations have an ordinal
in parentheses, so two terminals both named N remain distinguishable. Function choices include their category
and source library. In **Library**, **Fork existing symbol** searches project and installed-library symbols by name
or key; each choice shows its source and grid. **Create editable copy** leaves the original unchanged.
The command bar and status bar follow the active symbol; library paths and publication settings live in **Library**.
The ribbon uses the same small icons, labels and selected state as the main window. **More shapes** contains
ellipse, arc, polyline and polygon. Press **F1** in the editor to open this reference.
Text fields can show tags, function text, technical data, pin designations or cross-references.
**Advanced document** exposes the complete draft, including other slot types, styles and arrangements.
The circle tool takes a centre and a point on its circumference; arcs and ellipses take opposite bounding-box
corners. Arc creation angles are whole degrees; negative sweep reverses direction. Select a shape and press
**Delete** to remove it, or drag it to move it; either change takes one undo step. Numeric property edits commit
when you press Enter or leave the field, so partially typed coordinates do not reshape the drawing.
Selected rectangles, circles, ellipses and polygons have **Fill shape**. Text and text slots have a **Text style**
list showing the project's style sizes; choose a larger style when connection numbers need to match other labels.

**Anchor** sets the point used to calculate the symbol's cross-reference cell on the project page. It does not
move the symbol's insertion origin. Place it on the left edge for a supply block. **Link point** instead adds a
named attachment position for a mechanical link; it does not create an electrical connection.

Native connections must lie on the 4 mm grid. Drawing clicks snap to 0.5 mm; text clicks to 1 mm.
Numeric pin edits remain exactly as entered: a pin at 4.5 mm is visibly invalid and blocks publication.
See the
[design standard](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/design/07a-symbol-design-standard.md)
for the full authoring profile.

**Save to library** writes a symbol and its owned function definition to the chosen `.ewlib` folder.
The window remembers that folder. Its symbols appear under **Draw ▸ Place ▸ Symbol…** (`I`), with the library name;
the page must use the symbol's connection grid. Already embedded copies stay at their existing version until
you explicitly update them. Repeated placement keeps the authored outline and pins.
An assigned existing function definition stays referenced and its semantics cannot be edited in place; any
later compatible version of it is accepted. A new symbol has no key until you give it one. The first save
gives the draft its own identity, and later saves of that draft update the same symbol: increase the version
when changing previously published content. The symbol's own function definition keeps its version while it is
unchanged. A library that already holds the key for a different symbol refuses the save: choose another key, or
fork that symbol under its own key to change it. **Export draft** writes portable JSON to a
new file, including incomplete work; an existing file is never overwritten. Leave **Symbol file** empty to
choose the file in a file dialog, and choose **Choose folder…** to pick the library folder; an empty **Library
folder** is refused when you save. A relative path is read from your default user library folder.
**Close editor** keeps the draft and its undo history: **Edit ▸ Symbol editor…** returns to it. Only
**Discard draft** drops it, and **New symbol** starts a fresh draft. While the draft has unsaved changes,
**New symbol**, **Open or import**, **Create editable copy**, another drawing of an import and **Discard draft**
first ask whether to save the symbol. While the editor shows, Ctrl+S saves the symbol and the File menu's **Save project**
saves the project. Quitting with unsaved symbol changes asks whether to save the symbol. **Test on scratch page** creates a
page in the current project with the symbol's connection grid and places embedded copies there. The page then
shows with the copy selected; the message's **Back to the editor** returns to the draft. Project undo removes
the placement; a second undo removes the scratch page.
Scratch placements are independent draft copies, so **Source missing** in the Libraries navigator refers to
their temporary draft source. To place a copy linked to the published library, use **Draw ▸ Place ▸ Symbol…**
after saving.

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
| `symbol.slot` | `kind`, `x`, `y`; e.g. `"tag"` or `{"cp_designation":1}` |
| `symbol.link` | `name`, `x`, `y` |
| `symbol.function` | existing function `key`, or `library:<library UUID>:<item UUID>` in `key` |
| `symbol.replace` | complete `document` object |
| `symbol.undo`, `symbol.redo`, `symbol.validate` | none |
| `symbol.variant` | `variant` 0–7 for A–H |
| `symbol.export`, `symbol.save` | `file` or library `directory` respectively |
| `symbol.directory` | library `directory` that Save uses |
| `symbol.tool` | `tool`: the drawing tool to choose |
| `symbol.place` | `x`, `y` on the current project page |
| `symbol.scratch` | none; creates a test page in the current project |
| `symbol.close` | optional `discard: true` |

Action coordinates are **millimetres**, unlike JSON document coordinates. Placement snaps the insertion point to
the destination page grid; it never moves a pin within the symbol. A different symbol/page grid is refused
before changing the project; use a scratch page with the matching grid. Replacing or opening over a dirty draft
requires `discard: true`; `symbol.close` keeps the draft unless it has `discard: true`, and `symbol.new` without a
key or name returns to it, or with `discard: true` starts a fresh draft. `app.quit` with `discard: true` discards an unsaved draft. `ui.describe` and the script state expose the open editor and
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

A new drawing starts with a generic black-box function. In **Electrical data**, under **Function definition**, choose its
function type and set each pin's potential role and flags, or assign an existing library function. Existing assigned
functions remain read-only. Internal connections and less common fields are available through **Advanced document**.
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
