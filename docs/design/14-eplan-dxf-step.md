# Interoperability: import, export and migration: EPLAN data, DXF and DWG, STEP and PLC exchange

A chapter of [Interoperability: import, export and migration](14-interop.md), whose table says where each section lives.

---

## 5. EPLAN data

### 5.1 What EW reads

| Tier | Data | Release |
|---|---|---|
| A | EDZ part data, parts XML, report spreadsheets (device, terminal, wire lists), PDF/DXF pages as underlays | 1.0 |
| B | Project XML `.epj` (structure, devices, part numbers, connections), built from observed files | Later, experimental (FR-IMP-031) |
| C | `.elk`, `.edb`, `.zw1`, `.zw0`, `.elp`/`.els`/`.elx`/`.elr`/`.ell` | never |

EPLAN macros (`.ema`, `.ems`, `.emp`) reference EPLAN's own symbol libraries; their graphics are never reproduced.

### 5.2 EDZ part import

- **Container.** 7-Zip normally, ZIP for a minority (C-1, EW-IMP-203); content sniffing decides. Encrypted archives
  are refused. Members are read under the archive limits of §11, and only `*.part.xml`, pictures, drilling and
  connection-point data are opened.
- **Initiation.** User action only, local file only; the Data Portal terms notice is shown before the first EDZ
  import of a session and logged (EW-IMP-201, FR-IMP-026; wording in [07](07-symbols-parts-macros.md) §3.6). The
  network sandbox job asserts that the import opens no connection (NFR-PRIV-01).
- **Target.** A user or company library, never a project directly; projects pick parts up through assignment or the
  previewed "update from library" (FR-PRT-014, AC-38).

| EDZ / parts XML | EW part field |
|---|---|
| `P_ARTICLE_PARTNR`, `P_ARTICLE_MANUFACTURER` | part number (`pn`, `pn_norm`); manufacturer through the master's aliases |
| `P_ARTICLE_ORDERNR`, `P_ARTICLE_TYPENR` | order number, type number |
| `P_ARTICLE_DESCR1`, `P_ARTICLE_DESCR2` | `MlText`: `de_DE@text;en_US@text;` → {de-DE, en-US} (FR-I18N-008); `;;` inside a string is a literal separator |
| `P_ARTICLE_PICTUREFILE` | picture asset, user library only |
| `<variant>` (`P_ARTICLE_VARIANT`) | part variant |
| `<functiontemplate connectionDesignation connectiondescription terminalNr functiondefinition>` | template function with CPs; FnDef through `fndef-map.toml` (EPLAN category/group/id → EW FnDef), else a generic pin (EW-IMP-202) |
| `<accessorylist>`, `<freeproperty>` | accessories; user properties |
| `<drillingPosition>`, connection-point patterns | `FeaturePattern`, 3D CP data in the EW part frame with the source transform kept ([12](12-panel-layout.md) OQ 13) (FR-IMP-027) |
| `.e3d` | ignored in favour of STEP (EW-IMP-204) |
| provenance | source file SHA-256, import date (`as_of`), source "EDZ", licence "vendor terms" (FR-PRT-017) |

- **Pins.** The template order is kept; natural sort applies within a group of equal function definitions. Duplicate
  designations stay distinct by CP index (REF-029): the 2904602 template yields L, N, PE, +, +, −, −.

### 5.3 Parts XML

`<partsmanagement>` (FR-IMP-028) is streamed with quick-xml: `length-unit` (default mm) scales every length to nm
(EW-IMP-205 when not mm); `<part>` with its `P_ARTICLE_*` attributes, `<variant>`s, function templates, free
properties and accessory lists map as in §5.2. Duplicates on `(manufacturer, pn_norm)` update in place under the
library's dedupe rule ([07](07-symbols-parts-macros.md) §3.6).

### 5.4 Report spreadsheets

EPLAN device, terminal-diagram and wire lists in XLSX or CSV (FR-IMP-029) are imported through the **column
mapper**:

```toml
# Illustrative only: real column names depend on the customer's EPLAN report scheme.
[mapping.eplan-terminal-diagram-de]
target = "terminals"              # devices | terminals | connections
sheet = "Klemmenplan"             # XLSX sheet; a CSV source sets delimiter and decimal instead
header_row = 3
row_key = ["strip", "designation", "level"]

[mapping.eplan-terminal-diagram-de.csv]
delimiter = ";"
decimal = ","
encoding = "windows-1252"

[mapping.eplan-terminal-diagram-de.columns]
strip = "Klemmleiste"
designation = "Klemme"
level = "Ebene"
part = "Artikelnummer"
target_a = "Ziel 1"
target_b = "Ziel 2"
```

- Header row and CSV dialect (delimiter, decimal mark, encoding) are auto-detected on the first 20 rows; the user
  confirms or edits the mapping, which is saved in the company basic project for the next file. Column names are
  localised in EPLAN, so presets per UI language ship as data.
- Full DTs (`=P01+J-E-2416-102F2:13`, `+J-E-2416-X1031:1`) are parsed by `ew-rds`; devices are created unplaced, with functions whose
  CPs are created on demand by designation; connections join CPs without graphics. Consecutive same-designation
  terminals with rising levels fold into one multi-level terminal (EW-IMP-207, [08](08-connections-terminals-cables.md)
  §6.3).
- The result is a project without drawings that the user places from the navigators; the import is a batch with
  row-key lineage, so a newer export of the same list updates in place.
- **Levels in a connection list.** A connection list names its ends `strip:designation`, which says
  no level. Its map has two optional members, `from_level` and `to_level`, the columns that hold each end's level (a
  number; mapped in the column mapper as From level and To level, no preset names them):
  - an end whose level column is absent or empty is level 1, as before: the terminal's own connection points;
  - an end at another level names that level of its terminal: the terminal lists every level its ends name, level 1
    included, in rising order (one multi-level terminal, EW-IMP-207, said once), and the end joins that level's
    connection point of its side, the copy `Terminal::level_copies` gives it ([08](08-connections-terminals-cables.md)
    §6.3); a level that is no number is taken as 1 and said, as in a terminal diagram;
  - a connection's lineage key names a level other than 1 (`-X1:2/2`), so the connections of level 1 keep their IDs;
    two ends of one terminal at two levels are a connection between its levels, not one to itself.
  - Not built: the levels of a device's connection points (a report gives no device connection points, below), and a
    level designation (`LevelSpec.designation`) from a report.

**Parts (§5.2, §5.3).** `ew_import_eplan::read_parts` reads an EDZ (the 7-Zip signature or `PK`;
only `*.part.xml` members are opened, a 3D model or picture beyond the member limit that nobody reads does not refuse
the archive; the archive limits are the older reader's, 32 MiB packed, 4 MiB a part file, 64 MiB unpacked, 200 : 1), an
EPLAN parts XML or a bare part XML, streamed with quick-xml under `Limits` (a DOCTYPE refused), into `PartDraft`s of
`ew-interop-api`; the crate does not depend on `ew-import-catalog` (an importer depends on permissive crates only), so
the placing of a draft (manufacturer through the master list, `pn_norm` dedupe, item IDs, new / updated / unchanged /
duplicate / rejected, `--bump`) is `ew_import_catalog::Placer`, which `parse_parts` uses for its rows too. The shapes
the reader takes: a part's attributes `P_ARTICLE_PARTNR`, `_MANUFACTURER`, `_ORDERNR`, `_TYPENR`, `_DESCR1`, `_DESCR2`
(joined per language), `_WIDTH`, `_HEIGHT`, `_DEPTH` (in `length-unit`, to `mech`); `<variant P_ARTICLE_VARIANT>`;
`<functiontemplate connectionDesignation connectiondescription terminalNr>` (one pin per line break or `¶`), its
function definition `functiondefcategory`, `functiondefgroup` and `functiondefid`, or `functiondefinition="c.g.i"`;
`<freeproperty name value>`; `<accessorylist><accessory P_ARTICLE_PARTNR P_ARTICLE_MANUFACTURER quantity>`;
`<drillingPosition>`. Decisions: the pins of the first variant are the part's CPs, one template function per function
definition, natural order within a group whose designations are all numbered and the template's order for lettered
pins (L, N, PE, +, +, −, −); the further variants and the accessories stay in `extra` as `eplan.variants` and
`eplan.accessories` (the part model has no member for either, doc 03 §7); the picture (a part holds none; libraries
store no assets) and the drilling positions (there is no `FeaturePattern` record) are reported, not read;
at most 4,096 pins a part and 262,144 a file. A part carries its provenance (`eplan.edz:<file>` or `eplan.parts-xml:<file>`,
the file's SHA-256, the date), the licence `LicenseRef-vendor-terms` and `foreign_terms`. `fndef-map.toml`
(`data/fndef-map.toml`, rows of category, group, id, `fndef` and an optional part `kind`) ships with a handful of rows
whose EPLAN numbering is **not verified against a real export**; the user's
`<config dir>/imports/fndef-map.toml`, or `--fndef-map`, wins over it. The terms notice is `ew_api::eplan::TERMS_NOTICE`
on stderr for every file that is an EDZ (`ew_import_eplan::is_edz`: the 7-Zip signature or `PK`, whatever the
subcommand and the file's name), and a dialog once per session in the window that stands before any step of an EDZ
(`how` named or not); both log EW-IMP-201 in the report. A row of the user's `fndef-map.toml` must name a function
definition of the starter library, else the file is refused in words (a pin would name nothing). **Targets.**
`ew import edz|parts-xml <file> --library <key|id>` writes a user or company library (the one writer of
`catalog::import_parts_into_library`; a part the library holds takes what the file knows laid over it, as the
project's copy does, because the library keeps one version of an item and a replacement would drop the technical data
a user added); `--project <dir>` writes no library: it previews the update of the project's
part copies of the same manufacturer and normalised part number (`Session::update_parts_from_import`: the held copy with the file's
description, numbers, pins, template, free properties, size and provenance laid over it, as its next patch version, what
the file lacks kept; the plan is `library.update`'s, the card's field diff and devices) and `--apply` commits it
(AC-38). **Reports (§5.4).** `ew_import_eplan::read_report` takes the table (`ew-import-catalog`'s readers: CSV with
its delimiter found, an XLSX sheet; the header row detected in the first 20 or given) and a `ReportMap` (`target`,
`sheet`, `header_row`, `[columns]`; shipped German and English presets, a user's saved `eplan-report-<key>.toml`) and
emits a project without drawings: unplaced devices with a black-box function, strips and terminals (rising levels of one
designation folded, EW-IMP-207), connections between terminals only (at the levels the map's `from_level` and
`to_level` columns name, above), since a device's connection points need its part
(an end that no terminal answers is EW-IMP-003); an unmapped column is EW-IMP-206, and so is an optional mapped column
the table lacks (a required one refuses the file); a terminal listed twice in a diagram, and a level that is no number
(taken as 1), are said; the part column of a terminal diagram is read (`eplan.part`, as a device's); the lineage is `eplan` anchored on the
mapping's key, so a newer export keeps the IDs. EW-IMP-206 and -207 are rules of the EW-IMP pack. `ew import
eplan-report` writes the new project; the window's File ▸ Import… offers the mappings that fit the file's headers and
has the column mapper: the detected columns (the header row found below a title), a kind of report and a member per column, saved under the file's source key as `eplan-report-<key>.toml` in the configuration directory, which the next file of the scheme is offered first; `File ▸ Import…` is the one entry for every file: a csv or xlsx that no mapping fits and that reads as no parts table asks for the mapper or the parts import. **Not built:** the picture and the drilling positions
(above), device connection points from a report, a re-import of a report into an existing project, import as a copy
(FR-IMP-045, Could). The fuzz targets `edz_7z`, `edz_zip`, `parts_xml` and `eplan_table` are the hostile corpus's
`eplan/` class read by `eplan_part_inputs` (the repository has no fuzz crate).

### 5.5 Underlays

A PDF page or a DXF layout can be placed on a page as a locked underlay (FR-IMP-030) that no query of the
engineering model reads. It is excluded from checks, reports and exports unless the user enables "print underlay".
- **DXF:** the layout is imported once (§6) into graphic page items on a locked `underlay` layer.
- **PDF:** a page item `Underlay{asset sha256, page, transform, opacity}`; the page is rasterised with hayro at the
  view's resolution and cached by hash (Class III).

The source file is embedded as an asset only when the user ticks "embed"; the dialog warns that a customer drawing
may be confidential.

Both kinds are one typed page item, `underlay{asset, format pdf|dxf, name, page, layout, unit,
rect, opacity}` (an additive kind, ADR-0011 amendment 6); a DXF underlay is not imported into page items but
converted again when drawn, so the project holds one item per underlay. `rect` replaces the transform: a PDF page
covers the whole sheet, a DXF drawing is stretched uniformly over its rectangle, which is the drawing's extents in
sheet millimetres through the inverse of the page's page-to-sheet map, so it prints where it was drawn. Both draw grey at the opacity and carry no
entity; the commands neither move nor copy an underlay. A PDF page is rasterised at 150 dpi of its rectangle, each
side at most 4,096 px. A source not embedded is kept in the user's cache, `<OS cache directory>/underlays/<sha256>`;
a project opened elsewhere draws a dashed frame with the file's name inside its top-left corner (also for a source that
does not convert or render; such a failure is kept per source, so it is not tried again at every draw). The window's canvas always draws underlays; an
export draws them only with `--underlays` (`ew export pdf`, `svg`, `dxf`, `png`). Printing draws none. The window takes
a PDF through File ▸ Import… as an underlay of the open page; `ew import pdf` and `ew import dxf --as underlay` do
the same from the command line.

---

## 6. DXF and DWG

### 6.1 Reading

`ew-import-dxf` reads ASCII and binary DXF R12–2018 with the dxf crate (FR-IMP-032):

| Group | Coverage |
|---|---|
| Geometry | LINE; LWPOLYLINE and POLYLINE with bulges (arcs) and widths; ARC; CIRCLE; ELLIPSE; SPLINE flattened at 0.01 mm; SOLID; TRACE; POINT; HATCH solid fill with boundary, patterns as boundary only (EW-IMP-255) |
| Text | TEXT with alignment; MTEXT with `\P`, `\f`, `\H`, `\C`, `\S` stacks, `%%c`/`%%d`/`%%p`, `\U+XXXX`; `$DWGCODEPAGE` for pre-2007 files |
| Blocks | INSERT nested, scaled (also non-uniformly), mirrored (negative scale, `MIRRTEXT`), MINSERT arrays; ATTDEF and ATTRIB |
| Annotations | DIMENSION and LEADER/MLEADER through their anonymous `*D…` blocks |
| Raster | IMAGE: the referenced file is never opened (EW-IMP-254); a placeholder with its path text is drawn |
| Tables | LAYER (colour by ACI, on/off, linetype), LTYPE (dash patterns), STYLE (font → §6.3), BLOCK_RECORD |
| Header | `$INSUNITS`, `$MEASUREMENT`, `$EXTMIN`/`$EXTMAX`; without units the dialog asks with an extents preview (EW-IMP-251) |
| Spaces | model space and each paper-space layout; viewports clip model content |
| Not imported | 3DFACE, 3DSOLID, REGION, BODY (ACIS), proxy objects (EW-IMP-253) |

Coordinates are `f64` in the file. Conversion is `round_half_even(v × unit_nm)` after a finiteness and range check.
DXF y points up and sheet y down, so the importer flips y (`y_sheet = y_top − y`, with `y_top` the top of the
imported extents) and leaves x as it is.
INSERT rotations, bulge arcs and spline flattening use the deterministic helpers of `ew-dl`, its pure-Rust
`libm`-style trigonometry ([09](09-reports-forms-output.md) §4.3), never platform `sin`/`cos` or fused multiply-add, so the result is the same
on every platform. The rotation angles of `TEXT`, `MTEXT` and `INSERT` entities that become free texts and graphics
are stored as `Angle2` (10⁻³ degree, `round_half_even`, [03](03-domain-model.md) §3.1) with their DXF value
unchanged: DXF counts counter-clockwise with y up, and after the importer's y flip the same value is
counter-clockwise as seen on the sheet, which is `Angle2`'s sense (ADR-0005 amendment 2). Geometry inside a block
that becomes a symbol is transformed into the symbol frame, whose placements keep `Orient2`.

The reader is the crate's own, not the dxf crate, whose dependency graph brings uuid's random
v4 IDs and clock-stamped headers into the shipped graph (both refused by the dependency rules); `ew-import-dxf`
stays MIT OR Apache-2.0. It reads ASCII and binary files (strings Windows-1252 before AC1021 whatever `$DWGCODEPAGE` names, UTF-8 from
AC1021 on) and the coverage of the
table above, with these differences: MLEADER, 3D and proxy entities are EW-IMP-253; ACAD_TABLE is drawn through its
anonymous block like DIMENSION; an IMAGE draws a frame with the file's name, never its folders; each paper-space
viewport draws model space at its scale, clipped to its rectangle. `y_top` is the header's `$EXTMAX` (`$PEXTMAX` for
a layout) when the header's extents hold the converted geometry (its `$EXTMIN` below it, its `$EXTMAX` above it, within
0.01 mm), else the top of the converted geometry: extents that do not hold the drawing are stale and would put it off
the sheet. `\U+XXXX` escapes are resolved in every code page, a pair of surrogate units as the one character. A unitless drawing is refused with EW-IMP-251's words until the
caller names the unit (`--units`, or the dialog, which offers millimetres). Each import is a new graphic page; a
second import of the same file is a copy (§2.3).

### 6.2 Import targets

| Target | Result |
|---|---|
| Graphic page | a page of `PageItem`s grouped per block insert, layers mapped to page layers (FR-PAG-026) |
| Underlay | §5.5 |
| Symbol | the DXF→symbol wizard below |
| Part outline / drilling pattern | the front outline as part geometry; holes and cut-outs by layer preset into `FeaturePattern` (FR-IMP-035; the EPLAN 802–805 preset, layer meanings unverified, [12](12-panel-layout.md) §10) |

**DXF→symbol wizard** (FR-IMP-034):
1. Pick a block; its insertion base becomes the symbol origin; scale to nm by `$INSUNITS`.
2. ATTDEF tags matching `^(TAG|DT|BMK|LABEL)$` → tag slot; `^(DESC|FUNC)` → function-text slot.
3. ATTDEF tags matching `^(PIN|CONN|TERM|X)_?(\w+)$` → CPs at their insertion points, designation = suffix or
   default value.
4. Else entities on a layer named `CONN*`, `PIN*` or `EW-CP` → CPs at POINT or small CIRCLE centres.
5. Else an interactive pick of line ends touching the block's bounding box.
6. Snap CPs to the native connection grid (ADR-0027). If any CP moves by more than 0.1 mm, offer a uniform scale that
   puts the CP spacing on the grid, or "move CPs" (EW-IMP-256).
7. CP direction is the outward normal of the nearest bounding-box side; the FnDef is a black box with n CPs.
8. Save into a user library with provenance and licence "vendor terms, not redistributable" (EW-LIB-011 in
   [07](07-symbols-parts-macros.md) §5).

A vendor block with ATTDEFs PIN1–PIN4 yields a symbol with four CPs on the native grid.

Steps 1 to 4, 6 and 7 run in `ew-import-dxf` for every named block; each block opens as a
drawing in Parts (its Import… takes a `.dxf`; `ew import dxf --as symbol --library <dir>` saves the drafts),
whose connection-point tool is step 5 and whose save is step 8. A block that moves a CP more than 0.1 mm reports
EW-IMP-256 with the uniform scale that would put the CP spacing on the grid, in the draft's details only (no shipped
rule: a draft is no project). **Part
outline:** `ew import dxf --as outline --library <dir> --part <number>` writes the drawing's lines as the part's
`mech.front_2d`, as its next patch version. The drilling half needs the `FeaturePattern` record
([12](12-panel-layout.md) §10), which is not built.

### 6.3 SHX and font substitution

SHX fonts are proprietary and never shipped. A data table maps SHX names to bundled fonts with a width factor
(`romans.shx`, `simplex.shx`, `isocp.shx` → osifont; `txt.shx`, `arial.shx` → Noto Sans); unknown SHX names use
the default family. Each substitution is EW-IMP-252 (FR-IMP-033).

osifont is not bundled, so every SHX name maps to Noto Sans, the default family; the table
(`SHX_SUBSTITUTES` in `ew-import-dxf`) names the known ones, and EW-IMP-252 names each font substituted once per
import. No width factor is applied: the factors are unverified.

### 6.4 Writing

DXF page export is specified in [09](09-reports-forms-output.md) §4.4 and §4.9: AC1024, `$INSUNITS` = mm, layers per
export scheme (`EW-FRAME`, `EW-SYMBOLS`, `EW-WIRES`, `EW-TEXTS`, `EW-GRAPHICS`, `EW-TABLES`), symbols as BLOCK/INSERT
per symbol and variant, Béziers flattened at 0.01 mm. The interop requirement is the **round trip**: EW's own DXF of
folios 101–106 re-imports as graphic pages within 0.01 mm (AC-37). Drilling DXF per surface is FR-MFG-028.
`ew export dxf` is the writer as [09](09-reports-forms-output.md) §4.4 and §4.9 record
it. The file is AutoCAD 2010's, UTF-8: a character is written as itself, and the importer reads it back as written
(AC-37's test compares symbol inserts, turned and mirrored, and a text of `Ää°²` with its position). A device's texts
are the attributes of its symbol's insert, which the importer draws where they stand, and a block's attribute
definitions `TAG` and `FUNCTION_TEXT` are what the symbol wizard (§6.2) reads as the tag and function-text slots. The drawing is in sheet millimetres, so a page made of it takes its items through the inverse of the new
page's page-to-sheet map (a frame may fit its page to the sheet, as a QElectroTech import's does) and prints where it
was drawn; AC-37's test re-imports folios 101 to 106 and compares the printed drawn points and text
baselines, and those of folio 101 as an underlay of 102, within 0.01 mm. A text takes the nearest built-in style and
its own height as written (through the page-to-sheet map), so it prints at the size and on the baseline it was
written at. A layer name an export scheme gives is refused in words when it is empty, longer than 255 characters or holds
a control character or one of `< > / \ " : ; ? * | , = '`; two classes that share a name, or one named `0`, are one
LAYER record. The AC1024 file has been read back by this importer and by ezdxf 1.4.5 (its audit finds no error and
repairs nothing on the sample's pages); no CAD program has opened it.

### 6.5 DWG

- **1.0 (Should), FR-IMP-036.** The user configures the path of an installed ODA File Converter. EW runs it as an
  external process on a copy in a temporary directory (DWG → DXF 2018), with a timeout, and imports the DXF. The
  converter is never bundled or downloaded; without it, the dialog explains how to configure it (EW-IMP-257).
- **1.x, FR-IMP-037.** Native reading through acadrust (MPL-2.0) behind a cargo feature, enabled by default only
  once the DWG corpus imports with the same entity counts as its ODA-converted DXF. LibreDWG is not used (C-4).
  The 1.0 placement deviates from [vision](00-vision-and-scope.md) §9 and is raised in [02b](02b-requirements-extended.md)
  OQ 3.

`ew import dwg <file> --converter <program>` and the window's File ▸ Import… (which asks for
the converter's program once and keeps it in the settings) copy the DWG into a fresh temporary folder, run
`<converter> <in> <out> ACAD2018 DXF 0 1 *.DWG` for at most 120 s (stopped after that, with the processes it started: its own process group on Unix,
`taskkill /T` on Windows), read the DXF it writes under the DXF input limit, remove the folder and import the DXF as a
page, underlay or symbol drafts; EW-IMP-257 records the conversion. The folder has a random name and is readable by its
user only, and a guard removes it on every way out and stops a converter still running. Without a converter the words
of EW-IMP-257 say what it is and where to get it. The window runs this, the import's conversion and the dialog's probe
in the action itself, on the window's thread: a long conversion holds the window (it is no job).

---

## 7. STEP, PLC and catalogue exchange

### 7.1 STEP and 3D

STEP is not imported as model records. A vendor STEP file is attached to a part by hash, and `ew-step-worker`
produces Class III artefacts (meshes, edges, hulls, frames) in the cache ([12](12-panel-layout.md) §6.2; ADR-0016).
The worker isolation is the importer security model applied out of process: per-job timeout, memory cap, input size
limit, quarantine after two crashes, fuzzed IPC decoder (NFR-REL-12, NFR-SEC-03). Budgets: 20 MB in ≤ 30 s, cached
reopen ≤ 1 s (NFR-PERF-18).

Exports: glTF 2.0 `.glb` with node names = placement UUIDs and DT and part in the extras, and STL per layout space
(FR-EXP-007); STEP AP242 (AP214 optional) assembly named by DTs through the worker's `ExportStep` (FR-EXP-008,
Could).

### 7.2 PLC and bus

Specified in [10](10-plc-and-bus.md) §4 and summarised here for completeness:

| Format | Direction | Style | Release | Requirements |
|---|---|---|---|---|
| AML AR APC 1.4.0 (CAEX 2.15 write; 2.15 and 3.0 read) | export, import | exchange: matched three-way merge with ownership, parking list | 1.0 Must | FR-PLC-029, FR-PLC-031 … FR-PLC-033 |
| AML AR APC 1.2.0 (older TIA versions, EPLAN) | export | same writer, version table | 1.0 Should | FR-PLC-030 |
| TIA "PLC Tags" XLSX (10 fixed columns) | export, import | exchange | 1.0 Must | FR-PLC-036 |
| PLCopen XML TC6 v2.01 GVL (CODESYS) | export | — | 1.0 Must | FR-PLC-037 |
| Generic I/O list CSV/XLSX | round trip | exchange (§8 mechanism) | MVP Should | FR-PLC-038 |
| Instrument / I/O list | import | exchange | 1.0 Should | FR-PLC-050 |
| GSD, GSDML, IODD 1.1; ESI, EDS | import | device types into a library, never into a project | 1.0; ESI, EDS 1.x | FR-PLC-025 … FR-PLC-027, FR-BUS-013 |
| TIA Selection Tool AML; Rockwell L5X, Beckhoff TE1120 | import; export | — | 1.x | FR-PLC-034, FR-PLC-039 |
| TIA Openness bridge | — | separate Windows tool outside the Rust core | Later | FR-PLC-040 |

AML, GSD, GSDML and IODD parsers follow the importer rules of §2.1 and §11 (pure, limits, fuzz targets). They live
in the permissive `ew-caex` and `ew-devdesc` crates of ARCH §3 ([10](10-plc-and-bus.md) §4.9); `ew-plc` keeps the
exchange orchestration, merge and writers.

The rows of AML (both versions), the TIA tag table, PLCopen XML and the instrument list are
built ([10](10-plc-and-bus.md) §4). The AML reader of `ew-caex` is pure, reads under the
`Limits` of §2.1 and refuses a DOCTYPE; it returns the neutral exchange image, not an `ImportBatch`: an exchange is
no import of records with a lineage, it is applied by `plc.apply_exchange` after the three-way merge. The hosts
read exchange files through `Session::plc_import` and `Session::instrument_list`, inside `catch_unwind`, not
through the import front door. The TIA workbook and the instrument list are read by the table readers of the
parts import (§8), under the table limits.

### 7.3 Catalogues (1.x)

- **BMEcat 2005** (FR-PRT-020): streamed parse of `ARTICLE`/`PRODUCT`: supplier and manufacturer IDs, short and long
  descriptions per `lang` → `MlText`, GTIN, `ARTICLE_FEATURES` with `REFERENCE_FEATURE_SYSTEM_NAME` (ETIM-x.y or
  ECLASS-x.y), group ID and `FEATURE` name, value, unit → classification references plus features mapped to native
  keys through the ETIM mapping table ([07](07-symbols-parts-macros.md) §2.6); prices → library section L4 with source
  and `as_of`; `MIME_INFO` images are references and are not fetched. Element names are verified against the
  specification before implementation.
- **ETIM xChange** (JSON): products and trade items with ETIM classification, same target fields.
- **ECLASS Advanced XML** supplied by the user: IRDIs and values are kept; the dictionary is never shipped.
- Targets are user or company libraries; a 100,000-row catalogue streams under the limits of §11 with the index
  built off the UI thread.
- **VDMA 66413**, which EPLAN's parts management also imports ([eplan-features](../research/eplan-features.md)
  §6), has no import requirement and is not planned; BMEcat covers most of the same supplier data. The *export* of
  the functional-safety values of parts and safety functions as a SISTEMA / VDMA 66413 data file is FR-EXP-012
  (Could, Later); EW stores and exports these values and calculates nothing ([vision](00-vision-and-scope.md) §9.2).

