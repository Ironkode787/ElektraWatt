# Symbol editor and automated authoring

Open **Edit → Symbol editor**. A symbol draft has its own undo history, drawing, function definition,
connection points, text slots and cross-reference anchor. Changes do not alter existing placements.
Choose A to draw; B–H show live rotated/mirrored previews. Select a graphic to move it by dragging or edit
its coordinates in the properties. Pin coordinates, directions and function mappings are editable in **Pins**.
Text fields can show tags, function text, technical data, pin designations or cross-references.
**Advanced JSON** exposes the complete draft, including other slot types, styles and arrangements.

Native connections must lie on the 4 mm grid. Drawing clicks snap to 0.5 mm; text clicks to 1 mm.
Numeric pin edits remain exactly as entered: a pin at 4.5 mm is visibly invalid and blocks publication.
See the [design standard](../../../design/07a-symbol-design-standard.md) for the full authoring profile.

**Save to library** writes a symbol and its owned function definition to the chosen `.ewlib` folder.
An assigned existing function definition stays referenced and its semantics cannot be edited in place.
Increase the version when changing previously published content. **Export draft** writes portable JSON to a
new file, including incomplete work; an existing file is never overwritten. Closing an unsaved draft requires
the explicit **Discard changes** choice. **Test on scratch page** creates a page in the current project and
places embedded copies there. Project undo removes the placement after closing the editor.

## AI and command-line tools

No remote service or generated code is required. Use `ew symbol`:

```text
ew symbol template example.power "Power block" > power.symbol.json
ew symbol schema > symbol-authoring.schema.json
ew symbol validate power.symbol.json
ew symbol publish power.symbol.json --library symbols.ewlib
ew symbol import element.elmt --key example.import > imported-drafts.json
ew symbol import manufacturer.edz --key example.import > imported-drafts.json
```

Template and schema print JSON. Validation prints stable issue codes, JSON pointers and details, exiting 0
when valid, 1 for validation findings, or 2 for unreadable/malformed input. Import prints an **array** of drafts;
choose one object and save it as a `.symbol.json` file. QET conversion findings are retained in
`import_details` with their original message, code and source line, so tools can identify each loss. Publication validates again and uses the ordinary library
writer's lock, version and transaction protections. It does not save the current project.

The portable envelope uses `format: "ew.symbol/1"`, `key`, `version`, `symbol`, `function`, optional
`function_ref`, `import_notes`, `import_details` and `import_reviewed`. File coordinates are integer **nanometres**;
4 mm is `4000000`. A pin has `idx`, `fn_cp`, `pos: [x,y]` and outward `dir`.
The function's pin with the corresponding `idx` holds its printed `designation`.
Keep the reference created by the template; the publisher replaces temporary references for owned functions.

The window and `.ewui` scripts share these actions:

| Action | Parameters |
|---|---|
| `symbol.new` | `key`, `name` |
| `symbol.open` | `file`, optional import `key` |
| `symbol.candidate` | zero-based `index` from an import package |
| `symbol.fork` | source symbol `source` key, new `key`; includes embedded QET imports |
| `symbol.draw` | `tool`, `x`, `y`, optional `x2`, `y2`, `text` |
| `symbol.path` | `points` as millimetre pairs, optional `closed` for a polygon |
| `symbol.cp` | `x`, `y`, optional `idx`, `fn_cp`, `designation`, `direction` |
| `symbol.slot` | `kind`, `x`, `y`; e.g. `"tag"` or `{"cp_designation":1}` |
| `symbol.link` | `name`, `x`, `y` |
| `symbol.function` | existing function definition `key` |
| `symbol.replace` | complete `document` object |
| `symbol.undo`, `symbol.redo`, `symbol.validate` | none |
| `symbol.variant` | `variant` 0–7 for A–H |
| `symbol.export`, `symbol.save` | `file` or library `directory` respectively |
| `symbol.place` | `x`, `y` on the current project page |
| `symbol.scratch` | none; creates a test page in the current project |
| `symbol.close` | optional `discard: true` |

Action coordinates are **millimetres**, unlike JSON document coordinates. Placement snaps the insertion point to
the destination page grid; it never moves a pin within the symbol. Replacing/opening/closing dirty drafts
also requires `discard: true` where applicable. `ui.describe` and the script state expose the open editor and
validation findings. [The tutorial](../tutorials/symbol-editor.md) includes a complete six-pin example.

## Imported formats and limits

| Input | Supported behavior |
|---|---|
| QElectroTech `.elmt` | Existing bounded element reader; preserves the QET 10 px grid and provenance. Review conversion warnings. |
| EPLAN `.edz` | Bounded 7-Zip or ZIP container; candidates from supported XML members. Never extracts files to disk. |
| EPLAN `.ema`, `.ems`, `.esl` XML | PXF revision **1.23**: straight lines, rectangles, polylines, full circles and text. Each macro variant becomes a separate candidate. |
| EPLAN parts XML | Connection designations become a generated rectangular outline. This is explicitly **not the original macro artwork**. |
| EPLAN binary `.slk` or binary macros | Refused with an explanation. Export a supported XML macro/parts file from EPLAN. |

PXF arcs/ellipses, images, groups, external symbol references and unrecognised records are not faithfully
reconstructed. The importer reports incomplete geometry and style differences; unresolved macro pins have an
unknown direction and cannot be published until corrected. Macro insertion points are not verified terminal
positions. Check each position and mapping against the source. No claim of complete EPLAN compatibility is made.
The reference manufacturer EDZ was exercised locally; its content is not distributed with ElektraWatt.

EPLAN drafts require a licence/source review and explicit acknowledgement. A download is not treated as a
redistribution licence. No links or embedded executables are followed. Input limits: 4 MiB per XML/draft,
32 MiB compressed archive, 64 MiB unpacked total, 4,096 archive entries, 512 recovered candidates, 4,096 editable
primitives/pins per symbol; archive names, CRCs, decompression ratio and XML depth are checked.
