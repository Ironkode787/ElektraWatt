# ElektraWatt research: file formats and interoperability

Status: research note for the ElektraWatt design (2026-10-17). Scope: every format ElektraWatt (EW) must read or
write, with enough technical detail to scope the importers and exporters. For each format the note covers spec
availability, openness and legal limits, structure, existing parsers (Rust first, then permissively licensed C/C++),
and difficulty.

> **Anonymization note:** dates and the manufacturing year of the reference project in this document are synthetic placeholders, not the original values.

**Method and evidence levels.**
- **QElectroTech (QET)** facts come from a direct read of the QET source mirror (`master`, commit `894f9e3`,
  2026-10-17) and of its 24 example projects (file versions 0.80 to 0.200.1). They are verified.
- **Crate facts** (version, licence, date) come from the crates.io API on 2026-10-17. They are verified.
- **Vendor facts** come from web search summaries of vendor pages. The fetch tool could not resolve most vendor
  hosts in this session, and the shared web-search budget ran out part-way through, so some sections rely on my own
  knowledge. Those claims are marked **(unverified)**.
- Folio numbers (e.g. "folio 102") refer to the user's reference set `<drawing id>.pdf`. Its PDF
  metadata says `Creator: QElectroTech 0.100.0-dev`, `Producer: Qt 5.15.11`, created 2025-04-14. The PDF has
  **no outline (bookmarks) and no link annotations**.

---

## 0. Summary matrix

Priority uses the brief's v1 decisions: v1 = must ship in v1; v1.x = soon after; later = roadmap.

| Format | Dir. | Priority | Spec | Legal / licence | Best existing parser (licence) | Difficulty |
|---|---|---|---|---|---|---|
| QET `.qet` (XML) | R (W later) | **v1** | No spec; source code is the spec (GPL-2.0+) | Format facts are free to use; clean-room the code | none in Rust; write with `quick-xml`/`roxmltree` | Medium (many legacy variants) |
| QET `.qetz` (zip + SQLite, format 4) | R | v1.x | `QETZ-FORMAT.md` in the source | as above | `zip` + `rusqlite` | Low–medium |
| QET `.elmt`, `.titleblock` | R | **v1** | as `.qet` | Element library CC-BY-3.0 + "no ML" clause | none | Medium |
| EPLAN EDZ (7z: `*.part.xml` + `.ema` + images) | R | **v1** (part data only) | Partial: parts-XML tags documented in EPLAN help | Data Portal terms reportedly limit use to EPLAN products | QET's C++ importer (GPL); `sevenz-rust2` (Apache-2.0) | Low (part.xml), very high (macro graphics) |
| EPLAN parts XML (`<partsmanagement>`) | R | v1 | Tags/attributes documented in EPLAN help | Customer's own data | none | Low–medium |
| EPLAN `.ema/.ems/.emp` macros, `.epj` project XML | R | later (experimental) | Undocumented XML ("PXF") | Reverse engineering of outputs only | none | Very high |
| EPLAN `.elk/.edb`, `.zw1` backup | — | never | Closed | Closed | none | Not feasible |
| EPLAN labeling / report exports (XLSX/CSV/TXT) | R | v1 | User-defined schemes | Customer's data | `calamine`, `csv` | Low |
| DXF (R12–2018) | R/W | **v1** | Autodesk DXF reference (public PDFs) | Free to implement | `dxf` (MIT), `acadrust` (MPL-2.0) | Medium (import fidelity) |
| DWG | R (W later) | v1.x | No official spec; ODA spec is public | Autodesk trademark history; LibreDWG GPL-3.0+; ODA SDK paid | `acadrust` (MPL-2.0, R/W), `cadkit-dwg` (MIT/Apache, read) | High |
| SVG | W (R for logos) | **v1** | W3C | Free | `svg` (MIT/Apache), `usvg`/`resvg` (MIT/Apache) | Low |
| PDF (outline, links, PDF/A) | W | **v1** | ISO 32000, ISO 19005 | Free | `krilla` (MIT/Apache) | Medium |
| XLSX / CSV | R/W | **v1** | ECMA-376 / RFC 4180 | Free | `rust_xlsxwriter`, `calamine`, `csv` | Low |
| AutomationML (CAEX, AR APC) | R/W | **v1** | IEC 62714 (paid), AR APC (free on AML site) | Free to implement | none in Rust; AML.Engine (C#) as reference | High (profile semantics) |
| PLCopen XML (TC6 v2.01) | W (R) | v1.x | Free XSD from PLCopen **(unverified)** | Free | `plcopen` crate (MIT, parser) | Medium |
| Rockwell L5X | W (R) | later | 1756-RM084 reference manual | Free to read | `l5x` crate (MIT) | Medium |
| GSD / GSDML / EDS / ESI / IODD | R | v1.x | Owned by PI / ODVA / CiA / ETG / IO-Link; some members-only | Device files ship freely from vendors | `taktora-ethercat-esi`, `ethercat-esi`, `canopen-host`, `ioddengine` | Medium each |
| Komax (WPCS via TopConvert CSV) | W | **v1** (CSV) | Customer docs only | Komax customer channel | none | Low (CSV), medium (WPCS) |
| Rittal WT, Schleuniger, Steinhauer, Weidmüller WPC | W | v1 (generic) / v1.x | Customer docs; Excel templates | Vendor channel | none | Low–medium |
| Marking (Phoenix, Weidmüller, Brady, Murrplastik) | W | v1 (CSV/XLSX) | Import wizards with user mapping | Free | none | Low |
| Perforex / Steinhauer NC, DXF drilling | W | v1.x | Proprietary NC; DXF route | Vendor channel | DXF crates | Medium |
| STEP AP203/214/242 | R (W) | **v1** (R) | ISO 10303 (paid) | Free to implement | OCCT via `opencascade-rs` (LGPL-2.1); pure Rust: `truck-stepio`, `step-io` | High |
| glTF 2.0, STL | W (R) | v1 | Khronos (free) | Free | `gltf`, `stl_io` | Low |
| BMEcat / ETIM / ECLASS XML | R | v1.x | BMEcat free; ETIM ODC-By; ECLASS licensed | ECLASS content cannot be shipped | none | Medium |
| IFC | — | later | ISO 16739 | Free | — | Skip for v1 |

---

## 1. QElectroTech — the migration path (near-lossless import)

### 1.1 Versions and file types

- **The user's tool.** The reference PDF was produced by "QElectroTech 0.100.0-dev". The project files say
  `version="0.100.0-dev"` on each `<diagram>` and probably `version="0.100.0"` on `<project>` (seen in the shipped
  example `photovoltaique.qet`).
- **Release line.** The 0.100 tag is dated **2026-01-25** ([ChangeLog.MD](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/ChangeLog.MD)).
  Master is now **0.200.1-dev** ([qetversion.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetversion.cpp)).
  Its release notes cover 2,253 commits since 0.100: Qt 6 only, English source strings, `.edz` import, a
  `.qetz` zipped format, a headless CLI and an MCP server
  ([release notes 0.100→master](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/QET_release_notes_0.100_to_master.md)).
- **Licences.** Code is **GPL-2.0-or-later**. The element collection is a separate git submodule
  (`qelectrotech/qelectrotech-elements`) under
  [ELEMENTS.LICENSE](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/ELEMENTS.LICENSE):
  - use inside diagrams is unrestricted;
  - redistribution outside a diagram falls under CC-BY 3.0;
  - an added clause forbids use "as sample data for … building machine learning models".

| File | What | Container |
|---|---|---|
| `.qet` | Project: folios, embedded element collection, title-block templates, properties | Single UTF-8 XML document |
| `.qetz` | Same project split into files, engineering data in SQLite (0.200.1-dev, **format 4**) | ZIP (Deflate), [QETZ-FORMAT.md](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/container/QETZ-FORMAT.md) |
| `.elmt` | One element (symbol) definition | XML, root `<definition>` |
| `.titleblock` | Title-block template | XML, root `<titleblocktemplate>` |
| `qet_directory` | Category metadata in collections | XML `<qet-directory><names>` **(unverified tag name)** |

Element references in a project use location URIs: `common://`, `company://`, `custom://`, `macros://`, and
`embed://` for the copy embedded in the project
([elementslocation.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/ElementsCollection/elementslocation.cpp)).
Example: `type="embed://import/10_electric/10_allpole/…/parafoudre.elmt"`. Every placed element's definition is
embedded under `<collection>`, so **a `.qet` file is self-contained**. The importer never needs the user's QET
library.

### 1.2 `.qet` document tree (as written by `QETProject::toXml`)

Derived from [qetproject.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetproject.cpp)
and [diagram.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/diagram.cpp), and
cross-checked against the example corpus:

```text
<project version="0.100.0" title="…" [uuid="{…}" (0.200+)]>
  <titleblocktemplates> <titleblocktemplate name="…">…</titleblocktemplate>* </titleblocktemplates>
  <properties> <property name="saveddate" show="1">…</property>* </properties>   project-wide variables
  <usage/> <wire_crossings hop="…"/> <symbol_texts upright="true"/> <wiring rules…>  (0.200 only)
  <newdiagrams>                                   defaults for new folios
     <border cols rows colsize rowsize displaycols displayrows/>
     <inset author date title folio filename plant locmach indexrev version auto_page_num displayAt>
        <properties><property name show/>*</properties>   custom title-block variables
     <conductors …ConductorProperties…/>   <report label="%f-%l%c"/>
     <xrefs><xref type="coil|protection|commutator|…" displayhas snapto xrefpos master_label slave_label offset …/>*</xrefs>
     <conductors_autonums current_autonum freeze_new_conductors> <conductor_autonum title formula>…
     <folio_autonums>… <element_autonums>… <cable_autonums> (0.200) <guides>
  <diagram order="1" title author date folio="%id/%total" indexrev plant locmach filename version
           cols rows colsize rowsize height displayAt displaycols displayrows
           freezeNewElement freezeNewConductor [conductorAutonum] [uuid (0.200)]>
     <properties><property name show>value</property>*</properties>       folio custom variables
     <defaultconductor …/>
     <elementautonumfoliosequentials>/<conductorautonumfoliosequentials>  counters per folio
     <elements> <element …/>* </elements>
     <conductors> <conductor …/>* </conductors>
     <inputs> <input x y text font rotation uuid/>* </inputs>        free texts
     <images> <image …>base64</image>* </images>
     <shapes> <shape type x1 y1 x2 y2 rx ry closed z is_movable><pen color style widthF/><brush …/></shape>* </shapes>
     <tables> <graphics_table x y width height uuid name display_n_row>…<model>…SQL…</model></graphics_table>* </tables>
     <terminal_strip …/>*                                              drawn strips (0.100+)
  </diagram>*
  <terminal_strips> <terminal_strip><terminal_strip_data uuid><informations>…</informations></terminal_strip_data>
        <physical_terminal><real_terminal element_uuid/>*</physical_terminal>* <terminal_strip_bridge uuid color>…
  </terminal_strips>                                                   (0.9-dev / 0.100+)
  <cables> <cable uuid designation bmk installation location length type cores …>
        <colors><color/>*</colors> <part uuid diagram x1 y1 x2 y2/>* <core index part conductor x y/>* </cable>* </cables>  (0.200)
  <collection> <category name="import"><names><name lang>…</name></names>
        <category …>…<element name="x.elmt"><definition …>…</definition></element>… </collection>
</project>
```

**Units and geometry.**
- Scene units are **pixels at a nominal 96 dpi**. QET's paper presets compute `mm × 96 / 25.4`
  ([borderpropertieswidget.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/ui/borderpropertieswidget.cpp)).
  The y axis points down. The wiring grid is 10 px.
- The default border is **17 columns × 60 px and 8 rows × 80 px**
  ([borderproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/borderproperties.cpp)).
  This is exactly the reference set's 0–16 / A–H grid, so cross-references like "101-H16" survive the import if EW
  keeps the source grid per page.

### 1.3 Element instances (`<element>` inside `<diagram>`)

[element.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetgraphicsitem/element.cpp) `toXml` writes:

- **Attributes:** `type` (location URI), `uuid`, `prefix` (e.g. `F`), `freezeLabel`, `x`, `y`, `z`,
  `orientation` (0–3 = N/E/S/W, ×90°), `mirror` (`horizontal|vertical|both`, 0.200), `is_movable`.
- `<terminals><terminal id x y orientation [uuid]/>` — the **integer `id` is per-folio**, kept "for backward
  compatibility".
- `<links_uuids><link_uuid uuid group_index?/>` — master↔slave and report↔report links.
- `<elementInformations><elementInformation name="label" show="1">-101F1</elementInformation>…`.
  - The keys are listed in [qetinformation.h](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetinformation.h):
    `label formula comment function description designation manufacturer manufacturer_reference
    machine_manufacturer_reference supplier quantity unity model category voltage_rating current_rating notes
    width height depth plant location auxiliary1..4 (+ description_/designation_/manufacturer_/…_auxiliaryN)
    plc_type plc_address plc_function plc_comment plc_crossref xref exclude_from_bom`.
  - `plant` holds the `=` aspect and `location` the `+` aspect. `label` is **free text**.
- `<properties><element_type terminal_type terminal_function terminal_led/>` — terminals only.
- `<plcMasterData>` — PLC masters, layout only (0.200).
- `<dynamic_texts><dynamic_elmt_text uuid x y rotation font frame text_from="UserText|ElementInfo|CompositeText"
  Halignment Valignment text_width keep_visual_rotation><text/><info_name>label</info_name>|<composite_text>…`
  ([dynamicelementtextitem.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetgraphicsitem/dynamicelementtextitem.cpp)).
- `<texts_groups>`.

### 1.4 Conductors (`<conductor>`)

From [conductor.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetgraphicsitem/conductor.cpp)
and [conductorproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/conductorproperties.cpp):

- **Endpoints.** Either the legacy form `terminal1="118"` (integer id inside the same folio) or the modern form
  `element1="{uuid}" terminal1="{terminal uuid}"`. The modern form also writes the computed `element1_label`,
  `element1_name`, `element1_linked` and `terminalname1`; the same applies to end 2.
- **The two forms are mixed per end in real files.** Measured on the corpus:
  - `photovoltaique.qet` (0.100.0): 31 of 64 wires have `element1`, and 42 use at least one integer reference.
  - `schema_indus.qet`, re-saved by 0.200.1: all 69 wires still use integer references, because old symbols have
    no terminal uuids.
  - **The importer must resolve both forms.**
- **Path.** `<segment orientation="horizontal|vertical" length>` elements, only when the user edited the route;
  otherwise QET auto-routes. An incoherent path is discarded and recomputed.
- **Properties.**
  - Electrical: `num` (wire number text), `formula`, `function`, `tension_protocol`, `conductor_color`,
    `conductor_section`, `cable`, `cable_uuid`/`cable_slot` (0.200), `bus`.
  - Single-line: `type="multi|single"`, plus `phase`/`neutral`/`ground` counts.
  - Display: `color`, `color2`, `bicolor`, `dash-size`, `style`, `condsize`, `numsize`, `text_color`,
    `displaytext`, `onetextperfolio`, `horizrotatetext`, `vertirotatetext`, `horizontal-alignment`,
    `vertical-alignment`, and the text position `userx`/`usery`/`rotation`.
  - `freezeLabel`, `uuid` (0.200).
- **Potentials are implicit.** QET has no potential or net object. A net is "everything connected", and
  `function`/`tension_protocol`/colour/section are copied between wires of a potential by the UI. EW must
  **rebuild nets** from connectivity, folio reports and terminal bridges.

### 1.5 Element definitions (`.elmt` / embedded `<definition>`)

- **Root attributes:** `type="element" version width height hotspot_x hotspot_y link_type`.
  - `link_type` is one of `simple | master | slave | next_report | previous_report | terminal | thumbnail`.
    Enum values Simple=1, NextReport=2, PreviousReport=4, Master=8, Slave=16, Terminale=32, Thumbnail=64
    ([element.h](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetgraphicsitem/element.h)).
  - Legacy files carry an `orientation="dnny"` string.
- **Children:**
  - `<uuid uuid>`, `<names><name lang>`, `<informations>` (author/licence text).
  - `<elementInformations>` — default element information.
  - `<kindInformations><kindInformation name="type|state|number|function|…">`.
    - Master types: `coil | protection | commutator | plc`.
    - Slave types: `simple | power | delayOn | delayOff | delayOnOff`, with `state` `NO|NC|SW`.
    - Terminal types: `generic | fuse | sectional | diode | ground | …`, with functions `generic | phase | neutral | PE`.
    - Also `max_slaves`, `slaveContactGroups` (0.200).
    - All of these are listed in [elementdata.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/elementdata.cpp).
  - `<description>` — the graphics.
- **Graphic primitives:**
  - `line` (`x1 y1 x2 y2 end1 end2 length1 length2`), `rect` (`x y width height rx ry`), `ellipse`, `circle`
    (`x y diameter`), `arc` (`x y width height start angle`, degrees, Qt convention), `polygon`
    (`x1..xn y1..yn closed`), `text` (static: `x y text size|font color rotation`).
  - `dynamic_text` (as in instances), legacy `input` (`tagg="label"`).
  - `terminal` (`x y orientation="n|e|s|w" name type="Generic|Inner|Outer" uuid`, plus label-display attributes,
    `potential`, `use_master_label`; [terminaldata.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/terminaldata.cpp)).
- **Style mini-language:** `style="line-style:normal|dashed|dotted|dashdotted;line-weight:none|thin|normal|hight|eleve;filling:…;color:…"`,
  plus `antialias`. Note the historical French tokens `hight` and `eleve`
  ([customelementgraphicpart.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/editor/graphicspart/customelementgraphicpart.cpp)).
- **Fonts** are Qt `QFont::toString()` strings. The Qt 5 form has 10 fields
  (`Sans Serif,9,-1,5,50,0,0,0,0,0`, weight on a 0–99 scale); the Qt 6 form has 11 or more
  (`…,5,400,…,normal`, weight 1–1000). Both forms occur in one corpus (1101 × 11 fields and 24 × 10 fields).
- **Definition versions vary inside one project.** `photovoltaique.qet` + `industrial.qet` contain definitions
  tagged 0.3, 0.4, 0.5, 0.51, 0.60, 0.70, 0.80 and 0.90.

### 1.6 Cross-references, folio reports, master/slave

- **Folio reports** (QET's interruption points) are pairs of `next_report`/`previous_report` elements linked by
  `links_uuids`. Their label format is `<report label="%f-%l%c"/>` (folio, row letter, column).
- **Master/slave** (coil + contacts, protection + aux contacts, PLC master + I/O slaves) also use `links_uuids`.
- **Xref display** is configured per master type by `<xref>`: `displayhas="cross|contacts"`,
  `snapto="label|bottom"`, `xrefpos`, `master_label` (default `%f-%l%c`), `slave_label` (default `(%f-%l%c)`),
  `offset`, `showpowerctc`, `showterminalname`, `showallconfiguredslaves`, `stackoverlapping`, and per-type
  `…prefix`
  ([xrefproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/xrefproperties.cpp)).
- **These map directly to EW concepts.** Reports become interruption points; master/slave becomes main and
  auxiliary functions (folio 106 "(201-E1)", folio 201 "∞ 203-B4"). EW recomputes the xref text instead of
  importing it.

### 1.7 Numbering formulas (labels, wires, folios)

[assignvariables.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/autoNum/assignvariables.cpp):

| Variable | Meaning |
|---|---|
| `%F` | folio label |
| `%f` | folio index |
| `%id` / `%total` | folio number / folio count |
| `%M` | plant (`=`) |
| `%LM` | location (`+`) |
| `%l` / `%c` | row letter / column of the element |
| `%prefix` | element prefix |
| `%wf` `%wv` `%wc` `%ws` | wire function / voltage / colour / section |
| `%{key}` | any element-information key |

Numbering rules live in `<element_autonums>`/`<conductor_autonums>` with typed parts (unit, ten, hundred, string,
folio, …; rewritten in 0.200). The reference set's `-101F1` is QET formula `-%F%prefix%autonum`-style page-based
numbering **(exact formula used by the user unverified)**. The importer should keep `formula` alongside the
resolved `label` so EW can offer "convert to EW numbering rule".

### 1.8 Title-block templates (`.titleblock`)

From [titleblocktemplate.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/titleblocktemplate.cpp)
and [titleblockcell.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/titleblockcell.cpp):

- **Structure:**
  - `<titleblocktemplate name><information/><logos><logo name type="png|svg" storage="base64|xml">…</logo></logos>`
  - `<grid rows="25;25;" cols="t22%;r100%;t22%;">` — rows are absolute px. Columns are absolute px, `tN%` (percent
    of total width) or `rN%` (percent of the remaining width).
  - Cells: `<field row col rowspan colspan name align valign hadjust displaylabel fontsize><value><translation lang>%author</translation></value><label>…</label></field>`,
    `<logo resource>` and `<empty>`.
- **Variables:** `%author %title %filename %plant %locmach %indexrev %version %folio %folio-id %folio-total
  %auto_page_num %previous-folio-num %next-folio-num %date`, plus custom `%{var}` from project or folio
  `<properties>`
  ([bordertitleblock.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/bordertitleblock.cpp)).
- **Mapping:** The reference title block (Page/Prev/Next, `=P01`, `+J-E-2416`, Revision, Checked) maps onto these.
  Defect 7 (empty "Next:") is `%next-folio-num` on the last page. Defects 5 and 6 come from **per-folio overrides
  of `indexrev` and title-block text**. The importer should surface value conflicts between folios and offer to
  lift them into one project property.
- **SVG logos** need an SVG renderer: `usvg`/`resvg`.

### 1.9 Tables, BOM ("nomenclature"), summary, wiring list

- **Tables (0.8+).** `<graphics_table>` stores a **SQL query** against QET's in-memory SQLite project database,
  together with layout and `previous_table` chaining. The 0.8 changelog notes the nomenclature/summary tables and
  the custom-SQL feature
  ([ChangeLog](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/ChangeLog)).
  - The views are `element_nomenclature_view` (all element-info columns) and `project_summary_view`.
  - `wiring_list_view` (wire number, from label/terminal, to label/terminal, folio position) is 0.200
    ([projectdatabase.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/dataBase/projectdatabase.cpp)).
  - Folios 2 (TOC) and 901 (part list) of the reference set are such tables.
  - **Import as EW report pages, not as SQL.** QET's SQL schema is internal.

### 1.10 Version differences that matter to an importer

| Version (date) | Format changes relevant to import |
|---|---|
| 0.3–0.6 (≤2018) | Element texts as `<input>`. No terminal uuids. Wires reference integer terminal ids only |
| 0.6→0.7 (2019) | Element texts converted to `dynamic_elmt_text`; text groups |
| 0.7→0.8 (2020/21) | Terminal uuids added. Nomenclature/summary `graphics_table`. Rewritten BOM CSV export |
| 0.9 (2023-01) | `properties/element_type` for terminals (seen in a 0.90 example). DXF→element import via the external `dxf2elmt`. Slave contact type "Other". Terminal-strip data model in the dev line **(unverified exact version)** |
| 0.100 (2026-01-25) | Terminal-strip editor and drawn `terminal_strip` items on folios. Folio `version` attribute such as `0.100.0-dev` |
| 0.200.1-dev (2026) | Persistent uuids on project, folio, wire and shape. `<cables>` with parts and cores. `mirror`. `plcMasterData`. `slaveContactGroups`. Wire hops. `.qetz` containers (formats 2→4). Deterministic save order |

**Robustness rules from QET's own fixes** (release notes): reject NaN/inf coordinates (QET hit CVE-2026-15037 in
QDom serialization). Accept shared element uuids in old copy-pasted files: QET falls back to integer terminal ids
for those (#1408).

### 1.11 `.qetz` (master, format 4)

- **Archive:** ZIP containing
  - `mimetype` (stored, first): `application/x-qet-project+zip`, or the older `application/x-qelectrotech-project+zip`;
  - `manifest.xml`: `<qetz format="4" min-reader="4" writer="…"/>`;
  - `project.xml`, with each part replaced by `<qetz-part file>`;
  - `folios/<uuid>.xml`, `elements/…`, `titleblocks/…`, `images/<sha256>.png`;
  - `project.sqlite`.
- **Database tables:** `meta`, `project_property`, `folio`, `folio_property`, `element`, `element_info`, `link`,
  `conductor` (ends, num, formula, function, tension_protocol, colour, section, cable, bus), `terminal_strip`,
  `strip_terminal`, `strip_bridge`, `strip_bridge_terminal`. They have no keys and are keyed by item uuids.
  Moved blocks leave a `<qetz-db block="…"/>` marker.
- **Reading safely:** open with `PRAGMA trusted_schema=OFF`, read-only. Refuse views. Enforce the limit of
  100 000 entries / 2 GiB. Reject `..`, `\`, `:` and absolute names.
- **Implementation:** easy in Rust with `zip` (MIT) + `rusqlite` (MIT). The cleanest route is to re-join into the
  `.qet` DOM, exactly as QET does, and reuse the `.qet` importer.

### 1.12 QET → EW mapping (what is lossless, what is not)

| QET | EW target | Loss / risk |
|---|---|---|
| `<diagram>` + title-block attributes + `<properties>` | Page with page properties. Conflicting per-page values are promoted to project properties after user confirmation | None |
| Element definition | EW symbol in a per-project "QET import" library. Primitives map 1:1. Terminals become connection points (`name`, direction) | Qt arc and text metrics; Qt font fallbacks |
| `elementInformations` | Device properties: `label` is parsed into an IEC 81346 RDS (`=`/`+`/`-`) using `plant`/`location`; `manufacturer` + `manufacturer_reference` become a part reference | Free-text labels such as "S51" vs "-101F1" (defect 3) need a migration report |
| master/slave links | Main/aux function link | None when uuids resolve |
| report pairs | Interruption points (same name), xref recomputed | None |
| conductors | Connections plus a rebuilt potential/net. `conductor_color`/`section` become wire properties. `num` becomes the wire number | Implicit potentials: inferred |
| terminal elements + `terminal_strips` | Terminal strip with levels and bridges | Pre-0.100 projects have no strip objects; infer strips from label prefix (e.g. `X1031:1+`) |
| `<cables>` (0.200) or cable elements (older) | Cable with cores | Older projects draw cables as plain symbols (folio 106 `-106W1`); infer them |
| `graphics_table` | Generated report page (TOC, BOM) | Custom SQL is dropped and reported |
| shapes, inputs, images | Graphic objects | None |
| PLC info `plc_*` | PLC channel properties | Partial (0.200 PLC manager only) |

**Write-back to `.qet`** (v1.x) is useful for users who must hand files to customers still on QET. Use the same
schema, writing `version="0.100.0"`, modern uuid endpoints, and folio `<properties>` for EW-only fields.

**Licence handling.** Do not copy QET C++ code into a non-GPL EW. The file-format facts above are not copyrightable
expression. Use QET's GPL example projects only as **test fixtures kept outside the shipped binary**. Do not
redistribute QET element definitions in an EW library unless EW meets CC-BY-3.0 (attribution). The "no ML"
clause conflicts with CC-BY's no-additional-restrictions term; flag this for legal review.

---

## 2. EPLAN

### 2.1 What EPLAN stores and what is exchangeable

| Artefact | Format | Openness |
|---|---|---|
| Project `*.elk` + `*.edb` folder | Proprietary database files **(unverified internals)** | Closed. Not a target |
| Backup `*.zw1`; compressed project `*.zw0`; split backups `.000`–`.999` | Compressed archive of the project ([EPLAN help: backup principle](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/bakbackupdlggui_k_prinzip.htm)) | Undocumented. A newer `.zw1` will not restore in an older EPLAN. Not a target |
| Other project types `.elp/.els/.elx/.elr/.ell` (packed/archived/completed/revision) | **(unverified list)** | Closed |
| **Project XML export `*.epj`** (action `export /TYPE:PXFPROJECT`) | "stored in the target directory in XML format" ([Exporting projects](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/projects_h_exportieren.htm); [export action](https://eplan.help/en-US/Infoportal/content/api/2025/export.html)). Options: with or without master data and connections ([API PXFProject](https://www.eplan.help/en-US/Infoportal/content/api/2027/Eplan.EplApi.HEServicesu~Eplan.EplApi.HEServices.Export~PXFProject(String,String,Boolean,Boolean).html)) | XML, but **no public schema** |
| Macros `*.ema` (window), `*.ems` (symbol), `*.emp` (page) ([EPLAN help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/macrosgui_h_makroserzeugen.htm); [HE_PlaceMacros](https://eplan.help/en-US/Infoportal/content/api/2025/HE_PlaceMacros.html)) | Same "PXF" XML family. QET describes `.ema` as "tokenised EPLAN PXF that references EPLAN's own symbol libraries" ([QET EDZ README](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/import/edz/README.md)) | Undocumented. Symbol graphics are references into EPLAN symbol libraries (`.slk`), not inline geometry |
| Parts XML (`<partsmanagement>`) and EDZ | §2.2 | Tags documented |
| DXF/DWG/PDF/SVG page export, with layer mapping to `EPLANxxx` layers ([DXF layers](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/xdxfgui_r_ebenen.htm)) | Open formats | Graphics only. "EPLAN logic is not exported" |
| Reports/labeling exports (XLSX, CSV, TXT, XML) driven by user schemes **(unverified exact list)** | Open formats | Customer-defined columns |
| PLC exchange: AML AR APC 1.0–1.4, CSV address overview | §4 | Open (AML) |
| Manufacturing: wire-fabrication formats, NC formats, RiPanel AML, Smart Production EPDZ, Smart Wiring | §6 | Partly open |

**The `.epj`/`.ema` XML in practice (unverified).**
- From inspection reports in the community, these files have a root like `<EplanPxfRoot …>`.
- Objects appear as tokenised elements (`<O4 …>`, `<O17 …>` …) whose attributes carry numeric property IDs
  (`P20xxx` style) and references to symbol libraries.
- A search for an open-source parser found none.
- This is a **reverse-engineering project, not an importer**. It is feasible for structure (pages, devices, part
  numbers, connections) and expensive for graphics, because symbols resolve into EPLAN libraries that the user holds
  under EPLAN licence.

### 2.2 EDZ and the parts XML

**EPLAN parts XML (documented)** ([tags overview](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/xmlexport_k_start.htm);
[tags and attributes](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/xmlexport_o_tags.htm);
[function definitions](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/xmlexport_o_funcdef.htm)):

- **Root** `<partsmanagement>` (exactly once). Attributes `length-unit` (default mm), `type="EPLAN.PartsManagement"`, `build`.
- **Children:** `<part>` with `P_ARTICLE_*` attributes, then one or more `<variant>` (`P_ARTICLE_VARIANT` up to
  3 characters, …), then `<functiontemplate>`, `<freeproperty>`, `<accessorylist>`, position tags,
  `<drillingPosition>` and terminal tags.
- **Function templates** carry `functiondefcategory`/`functiondefgroup`/`functiondefid`. This is the EPLAN function
  definition, i.e. the "what is this pin/contact" semantics.
- **Export.** The parts-management export offers "EPLAN Data Portal exchange format (EDZ)" and "XML"
  ([export dialog](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/partsmanagementgui_d_artikelexport.htm)).
  A CSV import/export with a configurable scheme also exists
  ([CSV](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/partsmanagementgui_k_konfigurationcsvartikel.htm)).
- EPLAN also imports **VDMA 66413** and **ECLASS BMEcat** (see EW's EPLAN feature research).

**EDZ container** (verified from QET's working importer,
[edzarchive.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/import/edz/edzarchive.cpp),
[edzpart.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/import/edz/edzpart.cpp)):

- **Archive.** Normally a **7-Zip archive** (magic `7z\xBC\xAF\x27\x1C`). Some packages are **ZIP** (`PK\x03\x04`),
  and QET 0.200-dev reports those as unsupported.
- **Contents:** `*.part.xml` (the parts XML above), an EPLAN macro `*.ema`, product image(s), metadata. EDZ can
  also hold 3D graphics (`.e3d`), DXF, drilling patterns and connection-point patterns, per the EPLAN Data Portal FAQ
  ([FAQ](https://www.eplan.com/en/support/data-portal.html)).
- **Fields QET maps:**
  - `P_ARTICLE_PARTNR`, `P_ARTICLE_MANUFACTURER`, `P_ARTICLE_ORDERNR`, `P_ARTICLE_TYPENR`;
  - `P_ARTICLE_DESCR1`/`DESCR2` — multilingual strings in the form `de_DE@text;en_US@text;…`, with `;;` used as an
    in-text separator;
  - `P_ARTICLE_PICTUREFILE`;
  - pins from `<functiontemplate connectionDesignation=… connectiondescription=… terminalNr=… functiondefinition=…>`.
  - QET sorts pins naturally within groups and generates a generic box symbol.
- **What EW gets "for free" from an EDZ:** manufacturer, order number, type number, multilingual description,
  pin list with designations and functions (e.g. PSU L/N/PE/+/−, folio 102), drilling patterns, and 3D (if present
  and parsable).
- **What is hard:** the `.ema` schematic graphics and the `.e3d` 3D format (proprietary **(unverified)**). For 3D,
  prefer the manufacturer's STEP (§7).
- **Rust:** `sevenz-rust2` 0.24.0 (Apache-2.0, 2026-10-11) for 7z, `zip` 9.0.4 (MIT) for ZIP, `quick-xml` for the
  XML. Difficulty: **low for part data**, very high for macros.

**Terms of use (the blocking question).**
- QET's README states that the **Data Portal Terms of Use (§ 5.4 at the time of writing) restrict use of downloaded
  data to EPLAN products**. QET therefore reads only "portable, factual part data" and displays a responsibility
  disclaimer ([README](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/import/edz/README.md)).
- I could not open the primary EPLAN terms in this session (unverified).
- Individual manufacturers add their own terms. Mitsubishi Electric, for example, forbids resale or
  sub-distribution of the CAD data it publishes there ([Mitsubishi terms](https://www.mitsubishielectric.com/fa/download/index.html)).
- Downloading in EPLAN format requires an EPLAN software service agreement (Data Portal FAQ).
- **Consequence for EW:** never bundle EDZ content. The user-initiated import must show the same disclaimer. Prefer
  sources whose licence explicitly allows third-party tools (manufacturers' own download sites, §9).

### 2.3 What other ECAD vendors manage with EPLAN data

| Vendor | EPLAN interop |
|---|---|
| WSCAD | Converts WSCAD ↔ **EDZ** for article data ([WSCAD digitization](https://www.wscad.com/us/digitization/)). Project import: "Not directly", DXF/PDF only ([ctb comparison](https://www.ctb.co.at/en/knowledge/wscad-vs-eplan-comparison/)) |
| SEE Electrical (IGE+XAO) | DXF/DWG legacy import. EPLAN DWG signal numbers come in as text ([V4R2 release notes](https://www.ige-xao.com/wp-content/uploads/Release_Notes_SEE_Electrical_Expert_V4R2_Service_Pack_7_4_47_A_EN.pdf)) |
| AutoCAD Electrical | "No direct conversion"; DWG only ([Autodesk](https://www.autodesk.com/support/technical/article/caas/sfdcarticles/sfdcarticles/Can-EPLAN-P8-structures-be-imported-into-AutoCAD-Electrical.html)) |
| Zuken E3.series | No public EPLAN project converter found (unverified). Supplies its own Komax/manufacturing exports |
| EPLAN itself | Sells an "import service" that rebuilds structure, devices, terminals and connections from DXF/DWG ([EPLAN UK blog](https://blog.eplan.co.uk/legacy-data-and-data-migration)) |
| QElectroTech 0.200-dev | EDZ part import (pins plus metadata, generic symbol) |

**Bottom line:** nobody in the market imports EPLAN projects with intelligence except EPLAN. The realistic EW
offer is the following.
- **Tier A (v1):** EDZ and parts-XML part data; DXF/PDF pages as graphics underlays; EPLAN labeling/report
  exports (XLSX/CSV: device list, terminal list, wire list) imported into EW's structured model with a column
  mapper. This re-creates intelligence from EPLAN's own reports.
- **Tier B (later, experimental):** an `.epj` XML structural importer built by black-box observation.
- **Tier C:** never `.elk/.edb/.zw1`.

---

## 3. DXF, DWG, SVG

### 3.1 DXF versions

| `$ACADVER` | Release | Notes |
|---|---|---|
| AC1006 | R10 | QET's DXF export header still says AC1006 ([createdxf.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/createdxf.cpp)) |
| AC1009 | R11/R12 | Last "classic" DXF. No LWPOLYLINE, MTEXT, HATCH, ELLIPSE, SPLINE (all R13+) **(unverified per entity)** |
| AC1012 / AC1014 | R13 / R14 | Handles mandatory; LWPOLYLINE |
| AC1015 | 2000 | Safest modern export target **(recommendation)** |
| AC1018 / AC1021 | 2004 / 2007 | 2007+ files are UTF-8. Earlier ones use `$DWGCODEPAGE` (e.g. ANSI_1252) with `\U+XXXX` escapes **(unverified detail)** |
| AC1024 / AC1027 / AC1032 | 2010 / 2013 / 2018+ | Current |

The spec is Autodesk's public "DXF Reference" PDFs, one per release; dxf-rs lists links for R10–2013+
([dxf-rs README](https://github.com/ixmilia/dxf-rs)).

### 3.2 Entity coverage EW needs

**Import (symbols, title blocks, legacy sheets, enclosure drawings):**
- Geometry: LINE, LWPOLYLINE/POLYLINE (bulges = arcs; 2D/3D), ARC, CIRCLE, ELLIPSE, SPLINE (flatten), SOLID/TRACE,
  POINT, HATCH (solid fill plus boundary; patterns optional).
- Text: TEXT, MTEXT (strip or convert `\P`, `\f`, `\H` codes).
- Blocks: INSERT (nested, scaled, mirrored; MINSERT arrays), ATTDEF/ATTRIB.
- DIMENSION and LEADER/MLEADER: render via their anonymous blocks (`*D…`).
- IMAGE: raster underlay.
- Tables: LAYER (colour, linetype, on/off), LTYPE, STYLE (font), BLOCK_RECORD.
- Header: `$INSUNITS`, `$MEASUREMENT`, `$EXTMIN/MAX`.
- Paper space layouts and viewports.

**Symbol creation heuristics** (the EPLAN "window macro from DXF" equivalent):
- block = symbol;
- ATTDEFs named like `TAG`/`DT`, `PIN*`/`CONN*` = label and connection points;
- or connection points on a dedicated layer;
- otherwise an interactive "pick connection points" step.

The precedent is **dxf2elmt** (Rust, MIT, `dxf` crate). It converts LINE, CIRCLE, ARC, TEXT, ELLIPSE, POLYLINE,
LWPOLYLINE, SOLID and SPLINE to QET `.elmt` and is what QET shells out to
([dxf2elmt](https://github.com/antonioaja/dxf2elmt); [dxftoelmt.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/dxf/dxftoelmt.cpp)).

**Fonts.** SHX fonts are Autodesk-proprietary and appear in almost every EPLAN or AutoCAD DXF. EW needs a
substitution table (SHX name → bundled open font) and a warning. Rendering SHX shapes without the font files is
impossible; LibreCAD's LFF fonts are GPL **(unverified)**.

**Export (graphic exchange with customers, NC machining route, EPLAN-like):**
- One file per page, mm units (`$INSUNITS=4`).
- Layers per object class, mappable by scheme.
- Symbols as BLOCK/INSERT with ATTRIB for device tag, part number and connection designations. QET 0.200 does the
  same ("symbols as blocks with attributes and hidden part data").
- Real TEXT entities.
- Version AC1015 by default, AC1009 optional for old NC software.

### 3.3 DXF/DWG libraries

| Library | Licence | DXF | DWG | Notes |
|---|---|---|---|---|
| [`dxf`](https://crates.io/crates/dxf) 0.6.2 (2026-10-03) | MIT | R/W ASCII + binary (DXB) | — | Mature (2016–), 311 k downloads |
| [`acadrust`](https://github.com/hakanaktt/acadrust) 0.6.4 (2026-10-11) | **MPL-2.0** | R/W R12–2018+ | **R/W R12–2018+** (AC1009 DWG incomplete) | 48 entity types, ACIS SAT/SAB, failsafe reads with diagnostics, ~40 code pages. New (first release 2026-02) |
| [`cadkit`](https://github.com/TNYCL/cadkit) 0.3.3 (2026-10-16) | MIT OR Apache-2.0 | R R12–2018, W ASCII | **R** AC1014–AC1032, no writer | Clean-room with documented provenance; also DGN. Very young (2026-10) |
| `undxf` 0.8.0 | MIT | R ASCII | — | Part of the `uncad` family |
| `uncad` / `libredwg-sys` 0.12.0 | **GPL-3.0-or-later** | — | R (LibreDWG) | Copyleft. Only if EW is GPL-3 |
| LibreDWG (C) | GPL-3.0-or-later | — | R (W partial) | FSF project **(unverified current state)** |
| ODA Drawings SDK (C++) | Commercial membership | R/W | R/W all | Paid annual membership; terms bar OSS redistribution **(unverified)**. The free **ODA File Converter** can be run as an external tool (the dxf-rs tests do this) |
| ezdxf (Python) | MIT | R/W | — | Best reference implementation for semantics |

**DWG legal situation.**
- DWG has no official public specification. ODA publishes a reverse-engineered "Open Design Specification for .dwg
  files" **(unverified current version)**.
- Autodesk's former trademark and "TrustedDWG" dispute with ODA was settled long ago **(unverified dates)**. Naming
  the format "DWG" in docs is common practice (cadkit: "used only to name file formats").
- Linking LibreDWG makes the combined work GPL-3.0. Running it as a separate process is the usual way around that,
  but it is legally debatable.
- **Recommendation:** DWG via `acadrust` (MPL-2.0 is file-level copyleft, compatible with an Apache/MIT or GPL EW)
  behind a feature flag. Fall back to "convert with ODA File Converter" as an external, user-installed tool.
  Validate against a DWG corpus before calling it supported.

### 3.4 SVG

- **Export per page** (web viewing, documentation, ePOCKET-like sharing):
  - SVG 1.1 with `<text>` kept as text, so it stays searchable;
  - `<a href="#p102">` or `page-102.svg#A1` hyperlinks for cross-references;
  - layers as `<g id>` groups.
- **Crates:** [`svg`](https://crates.io/crates/svg) 0.18 (MIT/Apache) or hand-written; `usvg`/`resvg` 0.48.1
  (MIT/Apache) for importing SVG (QET logos, manufacturer pictograms). Difficulty: low.

---

## 4. AutomationML and PLC engineering tools

### 4.1 Standards

- **CAEX and AML parts.** CAEX 3.0 is IEC 62424:2016. AML is IEC 62714 (-1:2018 architecture, -2 role classes,
  -3 geometry via COLLADA, -4:2020 logic via PLCopen XML, -5:2022 communication). See the standards research and
  [AutomationML](https://www.automationml.org/about-automationml/specifications/).
- **CAEX building blocks:** `CAEXFile` → `InstanceHierarchy` → nested `InternalElement` (with `Attribute`,
  `ExternalInterface`, `RoleRequirements`/`SupportedRoleClass`, `RefBaseSystemUnitPath`), `InternalLink` between
  interfaces, plus `RoleClassLib`, `InterfaceClassLib`, `SystemUnitClassLib`, `AttributeTypeLib` (CAEX 3.0).
- **AR APC** (Application Recommendation "Automation Project Configuration") is the ECAD↔PLC hardware-configuration
  profile.
  - Versions 1.0.0–**1.4.0**. 1.3.0 added user-defined structured variables. 1.4 adds Automation Component
    Specification compatibility and pluggable router ports
    ([AML news](https://www.automationml.org/news/new-version-of-the-ar-apc-is-now-available/);
    [download archive](https://www.automationml.org/download-archive/)).
  - A drive extension (ARE) exists
    ([ARE Drive](https://www.automationml.org/news/application-recommendation-extension-drive-for-automation-project-configuration-is-available/)).
  - Modelled objects: project, device (controller, head station, I/O device), device items (rack, module,
    submodule), channels with I/O type, length and address, networks and nodes (PROFINET/PROFIBUS/Ethernet
    addresses), and the tag table (symbol, data type, address, comment). Exact role class names are in the AR APC
    library **(unverified naming)**.
- The AR APC document and libraries are a free download from automationml.org **(unverified; downloads require
  no membership as far as known)**.

### 4.2 Who exchanges what

| Tool | Mechanism | Data | Direction |
|---|---|---|---|
| **Siemens TIA Portal** | **CAx export/import** (AML). Introduced in V14 SP1 (secondary source) and still present in V20/V21 ([TIA V20 doc](https://docs.tia.siemens.cloud/r/de-de/v20/projekte-bearbeiten/projekte-anlegen-und-verwalten/cax-daten-exportieren-und-importieren); [IMD blog](https://industrialmonitordirect.com/blogs/knowledgebase/exporting-tia-portal-hardware-configuration-to-aml-v14-sp1-guide)) | Hardware configuration **without parameterization**: stations, racks, modules, folder hierarchy, subnet membership, device addresses, **I/O address ranges**, PROFINET IO / PROFIBUS DP master systems. A newer AML will not import into older TIA. Import conflicts are parked in a "ParkingLot" folder. Program and PLC tags: not part of CAx per secondary sources | ECAD ⇄ TIA |
| TIA Portal **Openness** | .NET API (`Siemens.Engineering`), Windows-only, needs TIA installed and the user in the "Siemens TIA Openness" group. CAx import/export callable from code (`CaxProvider`) **(unverified API names)**. Tag-table XML export/import also via Openness **(unverified)** | All of the above, plus tag tables | Scriptable, both directions |
| TIA Selection Tool | AML export (EPLAN converter "TST", AR APC 1.1.0) | Hardware configuration | → ECAD |
| **EPLAN** | AML AR APC 1.0–1.4. Converters `PlcDcExchangerSiemensTIA16/17AML`, `…TSTAML`, `PlcDcAMLExchangerGeneral`; XML converters for ABB, B&R, Bosch Rexroth, Schneider; CSV address overview ([API](https://www.eplan.help/en-US/Infoportal/content/api/2024/PlcDcExchangerSiemensTIA16AML.html); [AML help](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2.9/Content/htm/plcgui_k_amlbusdatenaustausch.htm)) | Cards, channels, addresses, symbolic names (TagTable), bus topology | Both |
| **Beckhoff TwinCAT 3** | **TE1120 XCAD Interface**: AML-based, "incremental, bidirectional"; still accepts the older **Beckhoff XML** ([TE1120](https://www.beckhoff.com/en-us/products/automation/twincat/texxxx-twincat-3-engineering/te1120.html)). TwinCAT 2: TS1120 ECAD Import (XML) ([TS1120](https://beckhoff.com/ts1120)) | I/O topology (EtherCAT terminals), variable links, PLC program skeleton | Both |
| **Rockwell Studio 5000** | AML with EPLAN ([Rockwell KB](https://support.rockwellautomation.com/cc/okcsFattachCustom/get/1086730_4)). **L5X/L5K** import/export ([docs](https://www.rockwellautomation.com/en-ie/docs/studio-5000-logix-designer/38-00/contents-ditamap/studio-5000-logix-designer/import-and-export.html)); L5X is documented in *Logix 5000 Controllers Import/Export*, 1756-RM084 **(unverified number)** | Modules (`CatalogNumber`, `ParentModule`, ports and slots), tags, alias tags (e.g. `Local:1:I.Data.0`) | Both |
| **CODESYS** | **PLCopen XML** (TC6 XML v2.01) import/export of POUs, variables and data types. CODESYS adds device-tree data in `<addData>` **(unverified)**. Device descriptions are CODESYS `devdesc` XML. No AML ECAD interface verified | Variables / I/O mapping, programs | Both (partial) |
| **Phoenix Contact clipx ENGINEER** | AML file exchange with EPLAN Pro Panel ([EPLAN help](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2023/Content/htm/amlgui_h_phoenixclipxengineerimportierenexportieren.htm)) | Terminal strips and mounting panels, part references only | Both |

**Rust status.**
- No AutomationML or CAEX crate exists (crates.io search on 2026-10-17 returned nothing).
- Build it on `quick-xml` + `serde` with CAEX 3.0 (and 2.15 for older tools) as typed structs, validated against
  the CAEX XSD in CI.
- The AML.Engine (C#, by AutomationML e.V.) is the reference implementation to test against **(unverified licence)**.
- `plcopen` 0.3.1 (MIT) parses PLCopen TC6 XML. `l5x` 0.6.0 (MIT) parses L5X. Both come from
  [radevgit/plc](https://github.com/radevgit/plc).
- Difficulty: **high**. The XML is easy; matching each tool's AR APC dialect is hard (EPLAN pins drive data to
  1.2.0). Budget for round-trip test projects from TIA V18–V21, TwinCAT and EPLAN.

**EW v1 cut:**
- AML AR APC 1.2 + 1.4 export: devices, racks, modules, channels, addresses, symbols.
- AML import from TIA (CAx) to create PLC boxes, cards and channel slots.
- CSV/XLSX I/O lists (universal fallback).
- PLCopen XML global-variable list export for CODESYS.
- On folio 203–206 the outputs `Q0.1…Q6.7` + function text (`TA05-V-15`) become AR APC channels + tags.

---

## 5. Fieldbus device descriptions (auto-create PLC and bus devices)

| Format | Bus | Encoding | Owner / spec access | Key content for ECAD | Rust |
|---|---|---|---|---|---|
| **GSD** | PROFIBUS DP | ASCII keyword file (`#Profibus_DP`, `Vendor_Name`, `Ident_Number`, `Module="…" … EndModule`, slots) | PI, "GSD Specification for PROFIBUS" **(unverified edition)** | Ident number, modules with I/O byte lengths, baud rates. Folio 3: Festo CPX-FB13 is a DP slave whose GSD defines its modules | none (simple parser) |
| **GSDML** | PROFINET | XML (ISO 15745 profile): `DeviceIdentity` (VendorID/DeviceID), `DeviceAccessPointList`, `ModuleList`, `SubmoduleList`, `IOData`, `ExternalTextList`, graphics | PI. Spec pages list features up to V2.44. Checker supports V2.45, editor V2.46/V2.50. Tools are members-only ([PI GSDML](https://www.profibus.com/download/gsdml-gsdx-specification-for-profinet)). V2.43 added the **GSDX** container | DAP, modules and submodules per slot, I/O sizes, ports (port count = topology), order numbers | none |
| **EDS** (EtherNet/IP) | CIP | INI-like text (`[File] [Device] VendCode ProdCode MajRev [Params] [Assembly] [Connection Manager] [Port]`) | ODVA, CIP Networks Library Vol. 1 ch. 7 **(unverified chapter)** | Identity, assemblies (I/O sizes), ports | none |
| **EDS / XDD / XDC** (CANopen) | CANopen | EDS INI per CiA 306; XDD XML per CiA 311 **(unverified numbers)** | CiA | Object dictionary, PDO mapping | `canopen-host` 0.6.1 (MIT/Apache) includes an EDS parser |
| **ESI** | EtherCAT | XML `EtherCATInfo` → `Vendor`, `Descriptions/Devices/Device` (`Type ProductCode RevisionNo`), PDOs, sync managers, DC | ETG. ETG.2000 v1.0.18 and schema v1.21 (2025-05-26), **members-only, no mirroring** ([ETG](https://www.ethercat.org/en/downloads/downloads_48EF1F220AF54F77AF58921401342864.htm)) | Product code, PDO entries (channels), groups | `taktora-ethercat-esi` 0.5.3 (MIT/Apache, 2026); `ethercat-esi` 0.2 (MIT/Apache, 2020) |
| **IODD** | IO-Link | XML (IODD 1.1) | IO-Link Community; IODDfinder catalogue **(unverified API terms)** | Process-data layout, device identity | `ioddengine` 0.1.3 (MIT, 2024) |

**Use in EW.**
- Import a device description into the part library. This creates a PLC or bus "device type" with slots, modules
  and channels, I/O widths and bus ports. A rack configuration then expands into PLC cards and channel slots for the
  PLC overview and I/O list.
- **These files do not contain physical connector or pin designations.** Those still come from the part/macro data
  (EDZ pin list, manufacturer datasheet).
- Users obtain device files from manufacturers. EW should not redistribute them unless the vendor licence allows it.
- Difficulty: medium per format; GSDML and ESI have the most value.

---

## 6. Manufacturing outputs

### 6.1 The canonical "production wire list"

EPLAN's generic "General wires parts list" holds connection endpoints, routing track, length, termination
processing and bundle numbers ([EPLAN wire fabrication](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/productionwiringgui_k_start.htm)).
**Each machine format is a separately licensed EPLAN interface**; the API lists 1 = Komax, 2 = CadCabel,
3 = Schleuniger, 4 = Steinhauer PWA ([API ExportProductionWiring](https://www.eplan.help/en-US/Infoportal/content/api/2027/ExportProductionWiring.html)).

EW should define one internal record and render it through **templates**: CSV/XLSX column maps per machine,
shareable as files.

| Field | Source in EW |
|---|---|
| order / bundle / sequence | Wiring order (routing, duct) |
| wire id / number | Connection numbering |
| source DT + pin, target DT + pin | Connection |
| cross-section, colour, wire type / part no. (e.g. H07V-K 1.5 BK) | Potential-based wire properties (defect 8) |
| length (routed + reserve) | 3D routing |
| end A/B processing: strip length, ferrule part no., crimp, twin ferrule | Part data of terminal + wire |
| marking text A/B (sleeve / print) | Labelling scheme |
| routing path (duct segments) | 3D |
| target position (mounting-plate coordinates) | 3D layout (Smart-Wiring-style guidance) |

### 6.2 Machines

| Machine / software | Import route | Openness |
|---|---|---|
| **Komax Zeta 6x0** (wire sets), **Kappa** (cut and strip), TopWin HMI | **WPCS** ("Wire Processing Communication Standard") for production data and feedback, simple ASCII ([TopWin WPCS](https://komtech.co.za/wp-content/uploads/2020/07/0068407_0_PRO_TopWin_WPCS_EN.pdf)). **TopConvert** turns customer-defined CSV cut lists into WPCS, configured via an "Excel specs sheet" ([TopConvert](https://www.komaxgroup.com/en/products/wire-processing/software-and-networking/topconvert); [Cablespeed](https://cablespeed.co.uk/product/topconvert/)). Zuken E3 ships a CSV for TopConvert ([E3.ExportToKomax](https://www.zuken.com/en/product/e3series/e3-export-to-komax/)). **WIRE Mind** imports Excel/CSV/ECAD for Zeta ([article](https://wiringharnessnews.com/komax-transforms-digital-wire-set-production-in-north-america-with-wire-mind-and-wire2go/)). EPLAN has a native "Komax – Zeta" export with `WireKey` (cross-section + colour, e.g. "2.5 blk") and `FontKey` ([EPLAN Komax tab](https://eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/productionwiringgui_r_draehtekomax.htm)) | WPCS spec through Komax customer channels only. **CSV → TopConvert is open** |
| **Schleuniger** | EPLAN target "Schleuniger (Easy ProductionServer)"; wires need Schleuniger part numbers ([EPLAN](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/productionwiringgui_r_draehteschleuniger.htm)). **CAYMAN Toolbox** imports Excel cable lists ([CAYMAN](https://www.schleuniger.com/en-us/cayman-toolbox)) | Excel route open |
| **Rittal Wire Terminal WT 24 / WT 36** (0.5–2.5 mm², 24/36 wire types) | EPLAN Pro Panel export. The import template is made with Rittal's "Wire Cockpit" software. The machine returns a "Wire Terminal" CSV (source, target, colour, section, labelling, length, magazine, position) to Smart Wiring ([EPLAN ESW help](https://www.eplan.help/en-us/Infoportal/Content/ESW/2022/Content/htm/smartwiring_wire_terminal_WT.htm); [RAS brochure](https://www.rittal.com/us_en/apps/download/img/uploads/US445%20RAS%20Wire%20Processing%20Machines%206.19.pdf)) | Template-driven CSV. Exact columns unverified |
| **Steinhauer** (PWA – Personal Wiring Assistant) | EPLAN "Steinhauer PWA" export with a fixed file name | Closed (unverified) |
| **Weidmüller WPC** | Excel template ("specific arrangement of the individual parameters"), CSV, or direct from EPLAN / Zuken E3 / WSCAD. A consistency check covers length, wire-end processing, quantity and marking ([Weidmüller](https://www.weidmueller.com/int/service/engineering_data.jsp)) | Template public via WPC Tool |
| CadCabel, Metzner | EPLAN targets | Unverified |
| **EPLAN Smart Wiring** | Consumes Pro Panel data or **Excel wire lists from other ECAD systems** | An EW XLSX could feed it (unverified mapping) |

### 6.3 Marking and label printers

| System | Import |
|---|---|
| **Phoenix Contact MARKING system** / PROJECT complete | CSV, XML, Excel, plus an EPLAN P8 interface ([MARKING system](https://www.phoenixcontact.com/en-pc/products/software-marking-system-software-1669297); [PROJECT complete](https://www.phoenixcontact.com/en-us/products/planning-and-marking-software-project-complete-1050453)). **clipx ENGINEER** opens legacy CLIP `.klp` files and exports project XML ([cxE help](https://www.phoenixcontact.com/cxe/help/en/DataExchange.htm)). It exchanges AML with EPLAN. `.KLM` files are mentioned anecdotally ([forum](https://forums.autodesk.com/t5/autocad-electrical-forum/exporting-with-phoenix-contact/td-p/7365092)) |
| **Weidmüller M-Print PRO** | Import wizard for `*.txt`, `*.csv`, `*.xml` and Excel with user-made import scripts; WMC (configurator) interface; "transfer of data from CAE systems" ([M-Print PRO](https://www.weidmueller.com/int/products/workplace_accessories/software/m_print_pro.jsp)) |
| **Brady Workstation** | Excel/CSV data import into label templates **(unverified)** |
| **Murrplastik** (MPM marking software) | Excel/CSV import **(unverified)** |

**EW approach:** a "label export" generator with per-product-family presets: terminal markers (strip, terminal
designation), wire markers (both ends), device markers (DT), cable markers. Output CSV/XLSX in the printer
software's import layout, one row per label and one column per text line. Handle multi-level terminals ("1+",
"2−", PE) and twin terminals.

### 6.4 Enclosure machining (drilling, cut-outs) and ePOCKET

| Target | Format |
|---|---|
| **Rittal Perforex** (BC/MT/Secarex; ex-Kiesling) | EPLAN "Export Rittal – Perforex BC / Secarex" NC with machine schemes, tools and drill exceptions ([EPLAN](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/ncgui_d_einstellungenncexportkiesling.htm)). EPLAN imports Perforex NC files `*.PBT`, `*.PVB`, `*.PKU` as drilling patterns ([EPLAN](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/propanelmasterdata_h_bohrbildausncdaten.htm)). Perforex workshop programming **imports DXF** ([RAS brochure](https://www.rittal.com/us_en/apps/download/img/uploads/RittalAutomationSystemsBrochure_US337-BR-TR_SingleWeb.pdf)). The Rittal configurator ships CNC + DXF + Pro Panel files ([Rittal app note](https://www.rittal.com/imf/none/4_11198/)) |
| **Rittal RiPanel Processing Center** | AML export of layout spaces (drilling, rails and ducts cut to length). Needs EPLAN Pro Panel 2.9 SP1 Update 7+ or 2022+ ([FAQ](https://www.rittal.eplan.com/int/en/support/rittal-processing-center.html); [EPLAN](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2022/Content/htm/amlgui_h_rittalripanelprocessingcenterexportieren.htm)) |
| **Steinhauer** (ModCenter etc.) | EPLAN NC export "CNC with decimal point / decimal comma" ([EPLAN](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/ncgui_r_allgemeinexportsteinhauer.htm)). Steinhauer `.CNC` holds all components, so the import route is DXF/DWG with an import scheme mapping layers to EPLAN layers 802–805 at scale 1 ([EPLAN DXF drilling patterns](https://www.eplan.help/en-us/infoportal/content/Plattform/2024/Content/htm/propanelmasterdata_h_bohrbildausdxfdaten.htm)) |
| Generic | **DXF per mounting surface**: holes as CIRCLE (or POINT + diameter attribute), cut-outs as closed LWPOLYLINE, threads on a separate layer, tool table as layer naming |
| **Rittal ePOCKET** | Cloud space per enclosure (QR code), based on EPLAN Cloud / eVIEW. 1 GB per pocket, "all standard data formats can be stored" ([FAQ](https://rittal.eplan.com/int/en/support/rittal-epocket.html)). For EW: upload PDF/A + exports manually. No third-party API known |
| **EPLAN Smart Production (EPDZ)** | Layout space + schematic pages + routed connections + an embedded **AML** with part placements + **E3D** 3D per layout space ([EPLAN help](https://eplan.help/en-us/infoportal/content/Plattform/2024/Content/htm/edaexportgui_k_start.htm)). Closed container. Not a target |

**EW v1:** DXF drilling/cut-out export per surface with a configurable layer convention, plus a drilling-pattern
library per part (from EDZ `drillingPosition` and manual entry).
**Later:** AML part placement in the RiPanel dialect, and Perforex NC once a sample and spec are available from
Rittal.

---

## 7. 3D formats

| Format | Use | Notes |
|---|---|---|
| **STEP** AP203 (ed2 2011), **AP214** (ed3 2010), **AP242** (ed1 2014, ed2 2020) **(editions unverified)** | **Import** manufacturer device models (Phoenix, Siemens, Schneider, Rittal and Festo all publish STEP, mostly AP214). **Export** the cabinet assembly to MCAD | B-rep. Needs a kernel or tessellator. Keep assembly structure + names (DT) on export |
| glTF 2.0 (ISO/IEC 12113:2022 **(unverified)**) | Internal render cache, web viewer, sharing | [`gltf`](https://crates.io/crates/gltf) 1.4.1 (MIT/Apache) |
| STL | Simple mesh exchange, 3D-printed parts | [`stl_io`](https://crates.io/crates/stl_io) 0.11 (MIT) |
| VRML97 / X3D | Legacy manufacturer downloads | Low priority. Convert offline |
| 3D PDF (PRC ISO 14739-1 / U3D ECMA-363) | Customer deliverable (PDF/A-4e allows 3D) | No Rust libraries. Later |
| EPLAN E3D, Rittal formats | — | Proprietary. Not targets |

**STEP in Rust (2026-10).**

| Crate | Licence | Status |
|---|---|---|
| `truck-stepio` 0.3.0 | Apache-2.0 | Part of the truck kernel, 2024 |
| `ruststep` 0.4.0 | Apache-2.0 | Low-level |
| `step-io` 0.2.5 | MIT/Apache, 2026 | Experimental. "Reads every mainstream AP, writes only AP242 ed2" |
| `cadmpeg-codec-step` 0.6.1 | Apache-2.0 | |
| `oxideav-step` 0.0.1 | MIT | Tessellates |
| `opencascade` 0.3.0 / `occt-sys` 7.8.1 | **LGPL-2.1** | OCCT 7.8 bindings. The robust option, via FFI |

The brief allows FFI for a B-rep/STEP kernel, so OCCT is the safe v1 choice for import and tessellation. Pure-Rust
crates are worth tracking.

**ECAD–MCAD.** IDX/EDMD (ProSTEP) is PCB-centric and not needed. For cabinets, deliver STEP AP242 (or AP214 for
older MCAD) of enclosure + rails + ducts + devices with DT names. Import enclosure STEP from Rittal or Schneider
(folio 902: NSYPLA1074G).

---

## 8. Reports out

### 8.1 PDF (the primary deliverable)

Required features, each motivated by the reference set:
- **Outline (bookmarks):** one per folio, grouped by section (100/200/900 separators). The reference has none.
- **Internal links:** every cross-reference, interruption point and TOC row links to the target page and position
  (GoTo destination with an XYZ or FitR rectangle). The reference has none. QET master only added clickable xref
  links after 0.100.
- **Page labels** matching sparse folio numbers (1, 2, 3, 100…199, 200…, 900…).
- **Searchable text:** device tags as real text in embedded, subset fonts.
- **Metadata:** XMP with title, project and revision.
- **PDF/A:** 2b by default, 3b/4f to embed the native project and CSV/AML, 4e for 3D (see the standards research).
- **Optional content (layers)** for grid, comments and revision clouds.
- **Reproducible output:** a fixed `CreationDate` from `SOURCE_DATE_EPOCH`, as QET does.

**Crates.**
- [`krilla`](https://github.com/LaurenzV/krilla) 0.8.2 (MIT/Apache) covers outlines, page labels, links, named
  destinations, metadata, tagged PDF, PDF 1.4–2.0, PDF/A-1…4 and PDF/UA-1. **Optional content groups are not in
  its feature list** (unverified gap).
- `pdf-writer` 0.15 (MIT/Apache) is the low-level layer under krilla.
- `printpdf` 0.12.8 (MIT) has layers **(unverified for 0.12)**.
- `lopdf` 0.45 (MIT) can post-process.
- **Recommendation:** krilla. Add OCG via a small `pdf-writer` patch or upstream contribution.

### 8.2 Excel / CSV

- **Crates:** `rust_xlsxwriter` 0.100.0 (MIT/Apache, released 2026-10-17) to write; `calamine` 0.36.1 (MIT) to
  read xls/xlsx/ods; `umya-spreadsheet` 3.1.1 (MIT) to read-modify-write templates; `csv` 1.4 (Unlicense/MIT).
- **Practical detail for a Dutch user:** Excel with a nl-NL locale expects `;` separators and decimal commas, and
  UTF-8 needs a BOM to open correctly **(unverified, common experience)**. Make dialect presets
  (`excel-nl`, `excel-en`, `rfc4180`).

### 8.3 ERP / BOM and catalogue exchange

| Format | Use | Notes |
|---|---|---|
| CSV/XLSX BOM (aggregated by part, with quantity) | Purchasing / ERP import | Defect 4: no quantity aggregation in the reference set. Make aggregation and a quantity column default |
| **BMEcat 2005** (BME e.V., XML) | Manufacturer catalogue import (parts, prices, ETIM/ECLASS features via `PRODUCT_FEATURES`) | Free spec **(unverified)**. EPLAN imports ECLASS-flavoured BMEcat (9.0 since 2.6, 10.0.1 since 2.9) ([EPLAN](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/news_p_artikelverwaltung_eclassimport_235591.htm)) |
| **ETIM xChange** (JSON) | ETIM's successor to BMEcat-ETIM for product data | JSON schema published by ETIM International **(unverified)**. ETIM model ODC-By ([ETIM](https://www.etim-international.com/?p=1811)) |
| **ECLASS Advanced** XML | Rich part data (connection points, drilling, thermal) | ECLASS 15.0 licensed ([ECLASS](https://eclass.eu/en/eclass-standard/licenses/language-versions)). EW may read user-supplied files but must not ship the dictionary |
| openTRANS (BME) | Orders | Out of scope |
| IFC 4.3 | Building BIM (`IfcElectricDistributionBoard`) | Unlikely for a panel builder. Later |

---

## 9. Parts data sources and licensing for an OSS project

| Source | Access | Licence situation for EW |
|---|---|---|
| EPLAN Data Portal (>2 M datasets, ~500 manufacturers) | Through EPLAN; EPLAN-format download needs a service agreement | Terms reportedly bind use to EPLAN products (QET README). **Do not integrate. User-side EDZ import only, with a disclaimer** |
| Manufacturer portals (Siemens CAx Download Manager produces EDZ macros and other CAx data ([secondary](https://industrialmonitordirect.com/blogs/knowledgebase/download-siemens-eplan-edz-macros-via-cax-data-manager)); Phoenix Contact, Weidmüller, WAGO, Festo download pages) | Per-product download of EDZ/STEP/DXF/datasheets | Each vendor's terms. Usually free use, no redistribution. EW: user imports; **ask vendors for an explicit OSS redistribution licence** for an "ElektraWatt parts pack" |
| BMEcat / ETIM catalogues from manufacturers | Download or partner portals | Usually customer-licensed. Import is fine; shipping is not |
| ETIM model | Open, ODC-By | Can be bundled with attribution |
| ECLASS | Licence or membership | Store IRDIs only |
| Distributor APIs: Mouser (API key), DigiKey (OAuth2), Nexar/Octopart (tiered), Farnell/element14 (key) | Online, per-user keys | ToS generally forbid bulk caching or redistribution **(unverified)**. Offer as optional connectors where users bring their own key; never ship data |
| Siemens Industry Mall / SiePortal, Phoenix online catalogue | Web shops | No public catalogue API known **(unverified)** |
| IODDfinder | Online IODD database | API terms unverified |

**Legal frame (not legal advice).**
- In the EU, the database *sui generis* right (Directive 96/9/EC) protects substantial extraction from a
  manufacturer catalogue even where single facts (part number, rating) are not copyrightable.
- Images, 3D models and symbol graphics are copyrighted works.
- So EW should ship only:
  1. its own symbol library (permissive, e.g. CC-BY-4.0 or CC0);
  2. part packs contributed under explicit licences, with per-record provenance (source, licence, date);
  3. importers that run on the user's machine with the user's files.

---

## 10. Cross-cutting implementation notes (Rust)

- **One import pipeline.**
  `trait Importer { fn sniff(&[u8]) -> Confidence; fn import(src, &mut Project, &mut ImportReport) }`.
  - Each importer writes into the EW domain model through the same commands as the UI, so validation, numbering and
    potentials all run.
  - The `ImportReport` lists every dropped or inferred item with the source location (file, folio, uuid).
- **Provenance.** Keep `source_ref` (QET uuid, EDZ part number, DXF handle) on imported objects. Re-import then
  updates instead of duplicating, and the migration report can link back.
- **XML:** `quick-xml` 0.42 (MIT) streaming for large files (QET examples reach 3.5 MB); `roxmltree` 0.21
  (MIT/Apache) for DOM-style lookups (uuid maps).
- **Archives:** `zip` 9.0.4 (MIT), `sevenz-rust2` 0.24 (Apache-2.0), `rusqlite` 0.40 (MIT, bundled SQLite).
- **Golden tests:** QET example corpus (24 files, 0.80–0.200.1), the user's own `.qet` projects, a DXF/DWG corpus
  (ODA File Converter round-trips), AML exports from TIA V18–V21, EPLAN and TwinCAT. Validate PDF/A with veraPDF in
  CI.
- **Untrusted input:** all imports are parsers of hostile files. Apply size and entry limits like QETZ's
  (≤100 000 entries, ≤2 GiB), reject NaN/inf, and fuzz with `cargo-fuzz`.

---

## Implications for ElektraWatt

1. **QET import is the v1 anchor.** Build a full `.qet` importer for file versions 0.3–0.200.
   - Resolve both legacy integer and uuid terminal references, per wire end.
   - Read every element definition from the embedded `<collection>`; no library dependency.
   - Map the `link_type` semantics (master/slave/report/terminal) to EW main/aux functions and interruption points.
   - Rebuild potentials from connectivity.
   - Import title-block templates (grid, `t%`/`r%` columns, PNG/SVG logos) and `%variables`.
   - Convert `graphics_table` pages into EW generated reports.
   - Add `.qetz` (format 4) by re-joining it into the same DOM.
   - **Acceptance:** every QET example project and the user's <customer> project import with zero lost connections,
     and re-render visually equivalent to QET's PDF.
2. **Make the migration report a feature.** On import, run EW's consistency rules and list each finding with a
   one-click fix. This addresses the reference defects directly:
   - tag-scheme mismatches (S51 vs -101F1, defect 3);
   - BOM vs. schematic tag drift (-4W1 vs -3W1, defect 1);
   - layout vs. schematic (T51 vs T55, defect 2);
   - per-folio revision/description conflicts (defects 5–6);
   - missing part data (defect 4);
   - wires without colour or section (defect 8).
3. **Keep EW non-GPL-clean if a permissive licence is chosen.** Re-implement from the format facts in this note.
   Use QET examples and elements only as external test fixtures. Do not bundle QET's element library; offer
   "import my QET collection" locally instead.
4. **EPLAN interop v1 = data, not drawings.**
   - EDZ / parts-XML import of part data and pin lists (7z + ZIP).
   - Import of EPLAN XLSX/CSV report exports (devices, terminals, wires) with a column mapper.
   - EPLAN PDF/DXF pages as underlays.
   - Never touch `.elk/.edb/.zw1`.
   - Put `.epj`/`.ema` reverse engineering behind an "experimental" flag, after v1, built only from observed output
     files, with a provenance log like cadkit's.
   - Show the Data Portal terms disclaimer on every EDZ import and never ship EDZ content.
5. **DXF first-class, DWG behind a flag.**
   - Use `dxf` (MIT) or `acadrust` (MPL-2.0) for DXF R12–2018 read/write.
   - Implement the entity list in §3.2, including INSERT/ATTRIB, HATCH, MTEXT and SHX substitution.
   - DXF→symbol wizard: blocks and attributes become symbols and connection points.
   - Export DXF AC1015 per page, with blocks and attributes and layer-mapping schemes.
   - DWG read via `acadrust` once a corpus passes; otherwise point the user to ODA File Converter. Avoid LibreDWG
     unless EW is GPL-3.0.
6. **PLC exchange through AML AR APC, with CSV as the universal fallback.**
   - Implement a typed CAEX 3.0 model (no crate exists) and AR APC 1.2 + 1.4 dialects.
   - Export devices, racks, modules, channels, addresses and tags to TIA (CAx import), TwinCAT (TE1120) and EPLAN.
   - Import TIA CAx exports to create PLC cards.
   - Add PLCopen XML variable lists for CODESYS and an L5X tag/module export later.
   - Device-description import (GSDML, ESI first, then GSD, EDS, IODD) feeds the device library.
7. **Manufacturing outputs via templates, not hard-coded formats.**
   - One canonical production wire list (§6.1), rendered by user-editable CSV/XLSX templates.
   - Ship presets for Komax TopConvert CSV, Weidmüller WPC Excel, Schleuniger CAYMAN Excel, Rittal WT CSV, and
     label exports (Phoenix MARKING, Weidmüller M-Print PRO, Brady).
   - Native WPCS, Perforex NC and RiPanel AML come later, through vendor partnerships (the pilot user as a pilot customer).
8. **PDF is the flagship output.** Use krilla with outlines per folio and section, links on every cross-reference
   and TOC row, page labels for sparse folio numbers, searchable text and PDF/A-2b, plus 3b/4f with an embedded
   project. Contribute OCG layers upstream or patch via pdf-writer. Add reproducible builds and veraPDF in CI.
9. **3D:**
   - STEP import via OCCT (`opencascade-rs`/`occt-sys`, LGPL-2.1, dynamically linked).
   - Tessellate into an internal glTF cache (`gltf` crate).
   - Export STEP AP242/AP214 assemblies with DT names, and glTF for viewers.
   - Re-evaluate the pure-Rust `step-io`/`truck` in 2027.
10. **Parts data strategy for OSS:**
    - ETIM (ODC-By) as the bundled classification;
    - ECLASS IRDIs as keys only;
    - BMEcat / ETIM xChange / ECLASS Advanced importers for user-supplied catalogues;
    - distributor APIs as bring-your-own-key connectors;
    - a separately licensed, provenance-tracked community "parts pack" repository;
    - outreach to manufacturers (Phoenix, Weidmüller, WAGO, Festo, Schneider) for explicit redistribution licences.
11. **Engineering hygiene:** a common `Importer` trait and `ImportReport`, `source_ref` provenance on every imported
    object, fuzzing, size limits, and golden-file regression suites per format.

## Open questions

1. **EPLAN Data Portal terms.** What do the current terms (§ 5.4 or its successor) actually say? Does a user-side
   import of EDZ part data into a non-EPLAN tool breach them? Do manufacturers' own direct downloads (e.g. Siemens
   CAx Download Manager output) carry different terms? This needs a reading of the primary text and possibly legal
   advice.
2. **User's QET files.** Can the pilot user provide their real `.qet` projects (<customer> and others), and the QET build used
   (0.100.0-dev)? These would serve as golden fixtures. Do they use custom title-block templates,
   company collections (`company://`) or summary/BOM tables?
3. **QET format freeze.** Will the user move to QET 0.200 / `.qetz` before migrating? If so, `.qetz` format 4 (and
   future formats) moves into v1 scope.
4. **EW licence choice.** Permissive (MIT/Apache) or GPL-3.0? This decides whether GPL code (QET's importer,
   LibreDWG) can be reused and whether `acadrust` (MPL-2.0) is acceptable. MPL-2.0 is acceptable under either.
5. **AR APC dialects.** Which TIA Portal version(s) does the pilot user use (V18/V19/V20/V21)? Does the TIA CAx export in
   that version include tag tables? A sample CAx AML export from the user's TIA installation is needed.
6. **CODESYS.** Do any customers use CODESYS-based PLCs? If so, does CODESYS accept device-tree data in PLCopen XML
   `<addData>`, or only variables and POUs?
7. **Machine park.** Which wire-processing and marking machines does the pilot user own? Komax Zeta/Kappa, Rittal WT, a
   Weidmüller WPC, Phoenix or Weidmüller printers? Can we obtain their import specs and sample files (WPCS, WT
   import template, M-Print PRO script) under NDA, with permission to implement openly?
8. **Perforex / Steinhauer NC.** Would Rittal or Steinhauer share NC format documentation or accept DXF from
   third-party tools? Which layer and tool conventions do their DXF importers expect?
9. **DWG demand.** How often does the pilot user receive DWG (vs. DXF/PDF) from customers? This decides whether DWG is v1.x
   or later, and whether `acadrust`'s young DWG writer is needed at all.
10. **EPLAN `.epj` access.** Can the pilot user or partners provide `.epj`/`.ema` samples exported from EPLAN under terms
    that allow black-box format analysis? Is an EPLAN-structured import valuable enough (customer projects arrive
    in EPLAN?) to fund the reverse engineering?
11. **Unverified specification details** to confirm before implementation: DXF R12 entity limits; GSD/EDS/CiA spec
    editions and access; ETIM xChange schema location; BMEcat 2005 licence; krilla OCG support; TIA Openness API
    names; L5X manual number.
