# Open-source and commercial landscape: lessons for ElektraWatt

*Research note for the ElektraWatt design (Rust, open source, 2–4 developers). Status: 2026-10-17.*

## 0. Method and evidence quality

- **The web-search budget for this run was used up before this agent started.** Every `WebSearch` call returned
  "budget used up". Most vendor domains were also unreachable through `WebFetch` (DNS failures for kicad.org,
  librepcb.org, qelectrotech.org, nlnet.nl, eplan.com, autodesk.com, wikipedia.org).
- **Primary sources that *were* reachable were read directly:**
  - shallow `git` clones of the QElectroTech source and element repositories, LibrePCB, the KiCad source (tags
    7.0.0–10.0.7), the KiCad **website source** (blog posts, donation FAQ, sponsor page) and Zed;
  - `git ls-remote` tag lists for FreeCAD, Fritzing, Horizon EDA, Slint, Contributor Covenant, REUSE and cargo-deny;
  - the **crates.io API** for Rust crate versions and licenses;
  - GitHub and GitLab project pages.
- Facts from those reads link to the file in the repository. Commit and tag dates come from `git log` on the fetched
  tag.
- EPLAN facts come from the sibling note
  [`eplan-features.md`](./eplan-features.md),
  which verified them against EPLAN's own pages earlier in this workflow. Where this note reuses them, it links that
  note's sources.
- **Anything marked "(unverified)" comes from my own knowledge.** That covers most commercial pricing and the
  funding-programme details. Check those claims before anyone makes a decision that depends on them.

---

## 1. Executive summary

1. **QElectroTech (QET) is graphical-first by design.**
   - The data model *is* the Qt scene graph: `Diagram : QGraphicsScene`, `Element : QetGraphicsItem`,
     `Conductor : QGraphicsObject`.
   - Device data is a free-form key/value bag on each placed symbol.
   - Reports come from copying scene state into a throw-away SQLite database.
   - Every defect in the reference set traces back to this: stale tags in the BOM, a layout not linked to the
     schematic, per-folio title-block drift, and no quantity aggregation. ElektraWatt must be **device-first**, with
     the drawing as a *view*.
2. **QET is moving fast.** Between 0.100 (2026-01-25) and master on 2026-10-09 it gained 2,253 commits and 669 merged
   PRs ([release notes](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/QET_release_notes_0.100_to_master.md)):
   - EDZ import, a PLC manager, a from-to wiring list, a cable manager, renumbering of placed elements;
   - a headless CLI, a JavaScript API, an MCP server for AI assistants;
   - deterministic saves and lasting UUIDs.

   ElektraWatt cannot win on "QET plus a few features". It has to win on *engineering semantics*: devices, potentials,
   terminals, cables, PLC, revisions, 3D and manufacturing data, all in one consistent model.
3. **The element collection is reusable, with conditions.**
   - [ELEMENTS.LICENSE](https://github.com/qelectrotech/qelectrotech-elements/blob/master/ELEMENTS.LICENSE) allows
     use in diagrams with no conditions.
   - Redistribution outside a diagram follows **CC-BY 3.0**.
   - It adds a **non-standard clause that forbids using the files as machine-learning training data**.
   - The collection has 8,838 `.elmt` files, 4,102 of them manufacturer-specific across 134 manufacturer folders
     (counted 2026-10-08).
   - QET source code is **GPL-2.0-or-later**, so it can be ported into a GPL-3.0-or-later project.
4. **KiCad is the best model for file format and governance.**
   - Diffable S-expression files carry a date-stamped `version` token (schematic `20260306` in 10.0.7).
   - Libraries are referenced through library tables.
   - Major releases come every year (7.0 Feb 2023, 8.0 Feb 2024, 9.0 Feb 2025, 10.0 Mar 2026), with frequent point
     releases.
   - Funding mixes donations ($100k+ from 2,500+ donors in one drive), corporate sponsor tiers ($1k–$15k), a
     services company (KiCad Services Corp) and grants (NLnet €50k in 2022).
   - Lesson from the out-of-process **IPC API** (protobuf over NNG): announced in 9.0 to cover the schematic editor
     "during version 10 development", it still ships an empty schematic proto in 10.0.7. Design the API into the
     core from day one.
5. **LibrePCB shows what a mixed Slint/Rust CAD app looks like.**
   - 2.0.0 shipped on 2026-01-28 with a Slint UI (122 `.slint` files).
   - The C++/Qt core remains (~1,575 C++ files), plus a small Rust crate (19 `.rs` files) linked through cbindgen.
   - The CAD canvas is still painted by Qt's `QGraphicsScene` into a `slint::Image`.
   - Its library model (Symbol, Component, Package, Device) and its policy of "file-format changes only in major
     releases" both transfer well to ElektraWatt.
6. **Rust CAD at scale works for applications, not for kernels built from scratch.**
   - Zed (~250 crates, GPL-3.0-or-later app with an Apache-2.0 UI framework) and Graphite (MIT/Apache) prove the
     language.
   - Fornjot (b-rep kernel) was **archived 2026-06-19** with the note "its goals were never reached". CADmium was
     archived 2025-09-05.
   - For a 2–4 person team: **do not write a B-rep kernel or a UI framework.**
7. **Commercial tools are sticky because of data and ecosystem, not drawing features.** The moats are:
   - parts and macro data (EPLAN Data Portal: more than 2 million device data sets from about 500 manufacturers);
   - OEM-mandated EPLAN deliverables;
   - TIA Portal / AML integration and Rittal/Komax machine links;
   - years of company macros and trained staff.

   An OSS tool can win on:
   - an open, git-diffable format;
   - no tiering (EPLAN locks PLC, revisions and the API behind its Advanced, Pro and Premium tiers);
   - Linux and macOS;
   - CI-driven outputs and data-quality gates;
   - scripting;
   - migration import.
8. **Licensing recommendation.** Every OSS ECAD project in this note is GPL (KiCad GPL-3+, LibrePCB GPL-3,
   QET GPL-2+, Horizon GPL-3, Fritzing GPL-3). Slint's free OSS option is GPL-3.0-only and LibreDWG is GPL-3+.
   Recommended layering:
   - **Application and engine: GPL-3.0-or-later**, with a plugin exception;
   - **format spec, schemas, reference reader/writer crates and plugin interface: MIT OR Apache-2.0**;
   - **symbols, templates and macros: CC-BY-SA-4.0 with a KiCad-style design exception**;
   - **ElektraWatt-authored part data: CC0-1.0**;
   - contributions under the **DCO, no CLA**.
9. **Funding fit.** NLnet (<city>), the NGI Zero funds and the Dutch WBSO R&D tax credit fit a Netherlands-based
   project best (details unverified). Corporate sponsorship by panel builders and component manufacturers is the
   long-term base. Services in the KiCad Services Corp style beat open-core.

---

## 2. QElectroTech in depth

### 2.1 Fact sheet

| Item | Value | Source |
|---|---|---|
| License (code) | GPL-2.0 text. File headers say "either version 2 of the License, or (at your option) any later version", i.e. **GPL-2.0-or-later** | [LICENSE](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/LICENSE), [qetscripting.h header](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/scripting/qetscripting.h) |
| License (elements) | Use in diagrams: unconditional. Redistribution outside a diagram: CC-BY 3.0. **No use as ML training data.** No warranty. Text in 16 languages | [ELEMENTS.LICENSE](https://github.com/qelectrotech/qelectrotech-elements/blob/master/ELEMENTS.LICENSE) |
| Stable releases | 0.1 (2008-03-08), 0.3 (2013-09-28), 0.4 (2015-02-20), 0.5 (2015-11-27), 0.6 (2018-03-06), 0.7.0 (2019-07-17), 0.8.0 (2021-02-21), 0.9 (2023-01-03), **0.100 (2026-01-25)** | [ChangeLog.MD](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/ChangeLog.MD) |
| Cadence | One stable release every **2–3 years**, with nightly/dev builds in between. The user runs **0.100.0-dev**, i.e. a dev build. Master is now "0.200.1-dev" | ChangeLog, release notes |
| Activity 2026 | 2,253 commits and 669 merged PRs from 26 Jan to 9 Oct 2026. Last commit seen on 2026-10-17 | [release notes](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/QET_release_notes_0.100_to_master.md) |
| Repo size | 891 `.cpp`/`.h` files, ~9.4 MB of source under `sources/`; 10,582 commits | git clone; [GitHub](https://github.com/qelectrotech/qelectrotech-source-mirror) |
| Community | GitHub mirror: 579 stars, 180 forks. About 30 named contributors plus 4 translators in the 2026 notes. Founded in 2007 by two French students, community-maintained since 2013. Forum and Mantis bug tracker on qelectrotech.org | GitHub, README, [publiccode.yml](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/publiccode.yml) |
| Stack | C++20, Qt 6 / KF6 (Qt 5 removed in 2026), CMake only (qmake removed), pugixml, SQLite, SingleApplication, optional Qt Qml for scripting | README, release notes |
| Element collection | Separate repo (git submodule). 8,838 `.elmt` files on 2026-10-08: 10_electric 7,001, 60_energy 1,325, 50_pneumatic 343, 30_hydraulic 94, 20_logic 75. Manufacturer articles: 4,102 elements in 134 manufacturer folders | [qelectrotech-elements](https://github.com/qelectrotech/qelectrotech-elements) (git ls-tree count) |
| Funding | PayPal donations; no foundation or company behind it (README). Windows MSI signed via SignPath | README, release notes |

### 2.2 Architecture

- **Scene graph = model.**
  - [`Diagram : public QGraphicsScene`](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/diagram.h),
    one per folio.
  - `Element : public QetGraphicsItem` and `Conductor : public QGraphicsObject`.
  - Undo is a `QUndoStack` of per-operation commands (`sources/undocommand`).
  - No separate domain model exists. Engineering queries walk graphics items.
- **Reporting by copy.** `projectDataBase::createDataBase()` opens a `QSQLITE` connection with no file name, using
  `PRAGMA temp_store = MEMORY` and `journal_mode = MEMORY`
  ([projectdatabase.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/dataBase/projectdatabase.cpp)).
  It copies element information into `element_info` and exposes `element_nomenclature_view`. BOM, summary and
  custom SQL reports query that view
  ([smart-device-bom.md](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/docs/smart-device-bom.md)).
- **Source modules** (directories under `sources/`):
  - `TerminalStrip`: physical and real terminals, bridges, levels, layout patterns;
  - `cable` and `cablelist`;
  - `materiallist` (CSV lookup);
  - `genericdevice` (box-symbol wizard);
  - `import/edz`;
  - `dxf` (DXF-to-element);
  - `scripting`: JS API, macro recorder, `LiveServer` for AI assistants;
  - `spacemouse`;
  - `logging` (crash handler, watchdog);
  - `autoNum`, `titleblock`, `print`, `svg`, `editor` (element editor).
- **File formats.** These are all XML:
  - `.qet` project (root `<project>` with `<properties>`, `<titleblocktemplates>`, `<newdiagrams>`, one `<diagram>`
    per folio, and an embedded `<collection>`);
  - `.qetz` (zipped project);
  - `.elmt` symbols, with a historic DTD in
    [misc/definition_element.dtd](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/misc/definition_element.dtd);
  - `.titleblock` templates.

  Projects **embed a copy of every element they use** (`embed://` URIs in
  [qetproject.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/qetproject.cpp)),
  which makes `.qet` files self-contained. That helps import, because ElektraWatt never needs the user's QET
  library.

### 2.3 Data model: why it is "weak device semantics"

| Concept | QET representation | Consequence |
|---|---|---|
| Device | No device object. A placed **element** carries an "information bag" (`ElementData::m_informations`, a `DiagramContext`): `label`, `manufacturer`, `manufacturer_reference`, `description`, `quantity`, `voltage_rating`, … All values are free text ([smart-device-bom.md](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/docs/smart-device-bom.md)) | The same physical device drawn twice (coil plus contacts, overview plus detail, schematic plus layout) gives two independent bags. Quantity is text. No unique device identity across views |
| Element kinds | `enum Type { Simple=1, NextReport=2, PreviousReport=4, Master=8, Slave=16, Terminal=32, Thumbnail=64 }`; `MasterType {Coil, Protection, Commutator}`; `SlaveState {NO, NC, SW}`; PLC I/O types ([elementdata.h](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/elementdata.h)) | Only a master/slave link joins representations of one device. It suits coil/contact cross-references but not "device shown on overview + detail + layout" (folios 201–206, 902) |
| Connection | `Conductor` between two terminals. Properties are free text: `num`, `function`, `tension_protocol`, `conductor_color`, `conductor_section`, `cable`, `cable_uuid`, `bus`, `formula` ([conductorproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/conductorproperties.cpp)) | No potential object, so "24VDC → DBU 0.75 mm²" cannot be a rule (defect 8). Wire colour and size are per-segment text |
| Interruption points | Folio-report elements (`NextReport`/`PreviousReport`) linked pairwise. The 2026 design-rule check flags "folio reports with several wires" ([qet-mcp README](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/misc/qet-mcp/README.md)) | One-to-one only. Fan-out of a potential to several pages (folio 103) needs several pairs |
| Terminal strips | 0.9+ `TerminalStrip` (physical/real terminal, bridge, level) beside the schematic | Better than nothing, but a terminal remains a symbol first |
| Layout | Another folio with symbols. Since 2026 elements have width/height/depth fields and cabinet thumbnails (#723, #1170) | Layout symbols are not the schematic devices (defect 2: T51 vs T55) |
| Title block | Project-level plus per-folio variables. 2026 fixes for "empty page-level overrides" (#572, #717, …) | Per-folio overrides allow "V1"/"V1.0"/"V2.0" drift (defects 5, 6) |
| Identity | Lasting UUIDs for projects, sheets, symbols, wires and terminals arrived only in 2026 (#883, #884, #1105, #1107, #1118, #1122) | Before 2026 there were no stable IDs, so diffing, merging and import cross-references were hard |
| Determinism | "Saving is now reproducible … a second save changes nothing" only from 2026 (#779, #814, #844, #846, #1030, #1109) | Older `.qet` files are not git-friendly. The importer must not assume stable ordering |

### 2.4 Features (0.100 and master)

| Area | Present in 0.100 / master (2026) | Gap versus EPLAN-class |
|---|---|---|
| Drawing | Rich 2D editing: groups, align, arc and fillet tools, mirror, command search, shortcut bar, SpaceMouse | Comparable for drawing |
| Auto-numbering | Formula-based. **Renumbering of already placed elements only after 0.100 (#704)** | The user's 0.100.0-dev could not renumber. That explains defect 1 (-4W1 vs -3W1) |
| Cross-references | Master/slave contact mirrors, folio reports, PDF hyperlinks (#490) | No potential cross-reference lists. No device-wide "all representations" view |
| BOM | SQL view, **one row per element**. "Format as bill of materials" (grouping) **off by default**. Quantity is free text ([smart-device-bom.md](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/docs/smart-device-bom.md)) | Explains defect 4: X1031 listed twice, no quantity column |
| Wiring list | From-to list built on new terminal/conductor tables (2026) | No ferrules, lengths, routing or machine export |
| Cables | Cable manager, cable list export (2026) | No cable catalogue with conductor colours bound to parts |
| PLC | PLC manager, PLC slave terminals (2026) | No AML/TIA exchange, no card catalogue |
| Parts | Material list CSV lookup (2026), EDZ import as generic box symbol (2026) | No parts database, no ECLASS/ETIM, no accessories, no macros |
| 3D / panel | None (2D layout folios only) | Missing entirely |
| Revisions | None | Missing (defect 5) |
| Automation | Headless CLI (export PDF/PNG/SVG/DXF/BOM/wires/nets; check-elements; `--set-titleblock`), JS API (off by default), macro recorder, **`qet-mcp` MCP server** with `qet_diff`, `qet_check`, `qet_layout_check` | QET has now set the bar for scripting and AI integration. ElektraWatt must match it |
| Robustness | CVE-2026-15037 (QDom serialization) fixed; ASan/Valgrind fixes; crash snapshots | Lesson: importers of untrusted project files need fuzzing |
| Performance | 2026 fixes for Qt 6 load-time regression (#570), ElementFactory pugixml pass (#567), DB rebuilt once per load (#840) | Evidence of load-time scaling problems on big projects. Scale beyond that is unverified |

### 2.5 Limitations that push users toward EPLAN (mapped to the reference set)

| Reference defect (brief) | QET root cause | ElektraWatt design response |
|---|---|---|
| 1. BOM shows -4W1/-4X1, page shows -3W1/-3X1 | The BOM is derived from text labels. No renumbering in the dev build used | The device is the primary object; tags are computed from the structure (=/+/-) and a numbering scheme, and every view renders the device's tag |
| 2. Layout T51 vs schematic T55 | Layout symbols are separate elements | A layout placement *references* the device ID. A placement without a device is an error |
| 3. Mixed tag schemes, "+J-E-2416" vs "+JE2416" | Labels are free text; no structure-identifier objects | Structure identifiers (=, +) are managed objects. Each project uses one tag-format policy |
| 4. Missing parts, duplicates, no quantity | One row per element, free-text quantity | Part assignments live on devices with a numeric quantity; the BOM groups by part number, and a "device without part" check runs |
| 5/6. Revision and description drift | Per-folio title-block overrides | Title-block fields bind to project properties. Revisions are managed objects |
| 7. "Next:" empty, TOC by hand | Page navigation fields not computed for every folio type | Page navigation and the TOC are generated reports |
| 8. "Standard size if not indicated" | No potential object | Potentials and wire-type rules (IEC 60757 colour / cross-section) apply automatically |
| 9. No wire, terminal, cable or PLC reports | Reports are only partly present (2026) | A report engine over the device model (≥ EPLAN core report set) |

### 2.6 What ElektraWatt can learn and reuse from QET

1. **The `.qet` importer is the migration path, and porting is legal.** QET is GPL-2.0-or-later, so a GPL-3.0-or-later
   ElektraWatt may port parsing logic (element XML, conductor properties, auto-number formulas, title-block
   variables) with attribution. A permissively licensed ElektraWatt could not, and would need a clean-room
   reimplementation.
2. **Handle elements carefully.**
   - Elements *embedded in a user's project* are covered by "use in diagrams without conditions". Converting the
     user's own project is fine.
   - **Bundling** the collection inside ElektraWatt is redistribution under **CC-BY 3.0 Unported**. That requires
     attribution and the license text.
   - CC-BY 3.0 Unported does not clearly license the EU *sui generis* database right; CC 4.0 does.
   - The extra **anti-ML clause** makes the terms non-standard. Do not merge QET elements into a CC-BY-SA-4.0
     ElektraWatt library. Keep them in a separate `qet-legacy` library carrying QET's own license file, or convert
     on demand from the user's installation.
   - Never use the collection to train or fine-tune models. This matters for a team that relies on AI assistance.
3. **Copy the EDZ legal posture.** QET's
   [EDZ README](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/import/edz/README.md)
   says:
   - the EPLAN Data Portal Terms of Use (§5.4 "at time of writing") restrict downloaded data to EPLAN products;
   - QET imports only "portable, factual part data" (pin designations, order numbers, manufacturers);
   - it does **not** reproduce `.ema` macro geometry, "tokenised EPLAN PXF that references EPLAN's own symbol
     libraries".

   ElektraWatt's v1 EDZ import should copy this posture: user-initiated, local, part data only, with a clear notice.
   It should prefer EDZ files that manufacturers publish under their own terms.
4. **Match QET's automation.** Headless CLI, deterministic saves, `qet_diff`-style semantic diff, design-rule checks,
   and an MCP server with an ask-first "live mode" are now the baseline. ElektraWatt should ship a CLI and MCP server
   from its first public release.
5. **Expect a fast-moving, AI-accelerated QET.** QET's 2026 pace (669 merged PRs in about 8.5 months) is far above
   its history. QET is a moving target and may close drawing-level gaps. ElektraWatt's lasting advantage has to be
   the model: devices, potentials, structure, 3D and manufacturing.
6. **Learn from QET's community structure.**
   - The element collection is the main contribution channel for non-programmers.
   - Translations matter: QET has 16 languages in the license text alone, and Dutch for Belgium.
   - Stable releases every 2–3 years push professionals onto dev builds. ElektraWatt should release often and keep
     master usable.

---

## 3. KiCad

### 3.1 Fact sheet

| Item | Value | Source |
|---|---|---|
| License | GPL-3.0-or-later for the combined work. Third-party parts under Apache-2.0, BSL-1.0 (clipper, pegtl …), ISC, MIT (fmt, nlohmann/json, sentry-native …), BSD (pybind11) | [LICENSE.README](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/LICENSE.README) |
| Releases | 7.0.0 (tag 2023-02-11), 8.0.0 (2024-02-22), 9.0.0 (2025-02-19; blog 2025-02-20), **10.0.0 (2026-03-19; blog 2026-03-20)**, 10.0.7 (blog 2026-10-07). 7 point releases in 6.5 months after 10.0.0, each preceded by an rc | git tags; [release-10.0.0.adoc](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2026/release-10.0.0.adoc) |
| Cadence | "Our annual development cycle continues to provide regular major releases". 7,609 unique commits in the v10 cycle | release-10.0.0 post |
| Repo | GitLab, 59,625 commits, 163 tags; C++ / wxWidgets | [gitlab.com/kicad/code/kicad](https://gitlab.com/kicad/code/kicad) |
| Libraries 2025 | +952 symbols, +1,216 footprints, +386 3D models. **STEP-only 3D models from v10.** **>78 % of footprints generated from data.** 2,105 library MRs in 2025; median MR time down from 3 days to 18 hours | release-10.0.0 post |
| Library license | CC-BY-SA 4.0 with an exception: designs using the libraries and generated files are released from Article 3 obligations | [kicad-symbols LICENSE.md](https://gitlab.com/kicad/libraries/kicad-symbols/-/blob/master/LICENSE.md) |
| Governance | Lead development team. Major features discussed on the dev mailing list first. AI-tool contributions governed by a dev policy linked from CONTRIBUTING | [CONTRIBUTING.md](https://gitlab.com/kicad/code/kicad/-/blob/master/CONTRIBUTING.md); [code-of-conduct.md](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/code-of-conduct.md) |
| Legal home | Joined the Linux Foundation on 2019-11-22. Donations handled with KiCad Services Corporation and the LF; "neither is a charitable organization under US tax law" | [LF post](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2019/kicad-joins-lf-announcement.adoc); [donate FAQ](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/donate/faq.adoc) |

### 3.2 File format (the VCS-friendly benchmark)

- **S-expressions**, one top-level object per file type:
  - `.kicad_sch` (schematic), `.kicad_pcb`, `.kicad_sym` (symbol library, many symbols per file), `.kicad_mod`
    (one footprint per file inside a `.pretty` directory), `.kicad_wks` (worksheet);
  - library tables `sym-lib-table` and `fp-lib-table`;
  - `.kicad_pro` project settings (JSON) and `.kicad_prl` (per-user local state, kept out of VCS);
  - jobset files (9.0+). (File list from my knowledge, unverified in detail.)
- **Versioning by date token.** Every schematic format change bumps `SEXPR_SCHEMATIC_FILE_VERSION`, with a one-line
  reason
  ([sch_file_versions.h](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/eeschema/sch_file_versions.h)). Recent
  examples:
  - `20240620` Embedded Files;
  - `20250513` "Groups can have design block lib_id";
  - `20250922` Schematic variants;
  - `20251012` Flat schematic hierarchy support;
  - `20260101` PCB variants;
  - **`20260306`** "Variant in_bom semantics corrected" (current in 10.0.7).

  There were about 25 bumps between Nov 2023 and Mar 2026, which works because readers upgrade old files
  automatically. `generator` and `generator_version` tokens (since `20231120`) record which program wrote the file.
- **Objects carry UUIDs.** Symbol instances store their library symbol inline (`lib_symbols`), so a schematic opens
  without its libraries, much like QET's embedded collection (unverified detail).
- **Lesson for ElektraWatt:**
  - a text format with stable ordering, one token per line, and UUIDs;
  - a monotonic, date- or integer-based format version with an upgrade chain;
  - a "generator" stamp;
  - per-user state kept separate from project data.

### 3.3 Library model

- Separate **symbol**, **footprint** and **3D model** libraries are linked by `lib_id` strings (`Lib:Name`). Library
  tables exist globally (user configuration) and per project, mapping nicknames to paths or plugins. Database
  libraries (ODBC) and HTTP libraries add parts metadata on top (unverified versions: database libraries in 7.0,
  HTTP in 8.0).
- **KiCad Library Convention (KLC)** plus generator scripts keep 2,000 MRs a year reviewable: 78 % of footprints are
  generated.
- **Weakness for ElektraWatt's purposes:** KiCad has no concept of a *purchasable device* that binds symbol, footprint
  and MPN in one library object. That arrives through fields or database libraries. LibrePCB and Horizon model it
  explicitly (sections 4 and 5.3). EPLAN's device-oriented model is closer to LibrePCB's.

### 3.4 Netlist, ERC, CLI, API

- **ERC:** a pin-type conflict matrix (input, output, power, passive, …). **Netlists** export in several formats
  (KiCad, SPICE, OrCAD, Allegro, PADS, CadStar), from my knowledge, unverified.
- **`kicad-cli`** runs headless exports (PDF, SVG, netlist, BOM, Gerber, STEP) and ERC/DRC. **Jobsets** in 9.0 are
  "reusable … output pipelines" runnable from the command line
  ([release-9.0.0](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2025/release-9.0.0.adoc)).
  This is the CI model ElektraWatt should copy: an `ew jobs run release.ewjobs` produces PDFs, BOM, wire list,
  terminal plan and Komax CSV.
- **IPC API.**
  - Introduced in 9.0 for the board editor. It "will eventually replace the current Python wrapper interface" and
    "will be extended to support the schematic editor during version 10 development" (release-9.0.0).
  - Transport: protobuf over NNG (`find_package( nng REQUIRED )` and `option( KICAD_IPC_API … ON )` in
    [CMakeLists.txt](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/CMakeLists.txt)).
  - Common commands include `BeginCommit`/`EndCommit`, `CreateItems`, `UpdateItems`, `DeleteItems`, `GetSelection`,
    `HitTest`, `GetTitleBlockInfo`, `RunAction`
    ([editor_commands.proto](https://gitlab.com/kicad/code/kicad/-/blob/10.0.7/api/proto/common/commands/editor_commands.proto)).
  - In 10.0.7, **`api/proto/schematic/schematic_commands.proto` contains only the syntax and package lines**. The
    schematic API did not ship.
  - SWIG bindings are still in the 10.0.7 tree (`pcbnew/python/swig`).
  - The Python client is `kicad-python` / `kipy` (unverified).
  - **Lesson:** retrofitting an API onto a GUI-centric C++ codebase takes years. ElektraWatt should put every
    mutation behind a command and transaction layer in the core (the API *is* the internal command bus). The GUI,
    CLI, scripts and MCP then all use it.
- **Importers as an anti-lock-in message.** KiCad 10 added Allegro (`.brd` v16–23, reverse-engineered, "supported by
  Quilter"), PADS (`.asc`, schematic and board) and gEDA/Lepton importers. The post opens with "Proprietary file
  formats are vendor lock-in by another name"
  ([new-importers-kicad-10](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2026/new-importers-kicad-10.adoc)).
  ElektraWatt's EPLAN/QET/DXF importers support the same positioning, and a sponsored-importer model is proven.

### 3.5 Funding and governance model

| Channel | Facts | Source |
|---|---|---|
| Donations | One fund drive raised "$100,000+ from more than 2,500 donors". Payments by card, SEPA, iDEAL; IBAN to KiCad Services Corporation | [2026 fund drive](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2026/2026-annual-fund-drive.adoc), donate FAQ |
| Corporate sponsors | Platinum ≥ US$15,000/yr, Gold ≥ 7,500, Silver ≥ 3,000, Bronze ≥ 1,000 | [become-a-sponsor](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/sponsors/become-a-sponsor.adoc) |
| Services company | **KiCad Services Corporation** (KiPro, founder Seth Hillbrand). Project leader Wayne Stambaugh joined it in 2019 | [post](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2019/project-leader-joins-kipro.adoc) |
| CERN | Ended its donation programme in 2023 and now pays KiCad Services Corp support contracts directly | [CERN post 2023-09-29](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2023/cern-support-announcement.adoc) |
| Grants | **NLnet €50k (2022)** for SPICE improvements, an import wizard, the CADSTAR importer, database libraries, flat schematics … | [nlnet-grant](https://gitlab.com/kicad/services/kicad-website/-/blob/master/content/blog/2022/nlnet-grant.adoc) |
| Sponsored features | Allegro importer funded by Quilter and the community | importers post |

---

## 4. LibrePCB

| Item | Value | Source |
|---|---|---|
| Releases | 1.0.0 (2023-09-24), 1.1.0 (2024-04-03), 1.2.0 (2024-12-01), 1.3.0 (2025-03-24), **2.0.0 (2026-01-28)**, 2.0.1 (2026-02-22), 2.1.0 (2026-05-19), 2.1.1 (2026-06-12). Master is `2.1.2-unstable` | [metainfo.xml](https://github.com/LibrePCB/LibrePCB/blob/master/share/metainfo/org.librepcb.LibrePCB.metainfo.xml), [CMakeLists.txt](https://github.com/LibrePCB/LibrePCB/blob/master/CMakeLists.txt) |
| License | GPL-3.0. The repo follows the REUSE layout (`.reuse/`, `LICENSES/` with CC0, CC-BY-SA-4.0, GPL-3.0-or-later, MIT …) | [LICENSE.txt](https://github.com/LibrePCB/LibrePCB/blob/master/LICENSE.txt) |
| Stack | C++20 + Qt ≥ 6.2 core (~1,575 `.cpp`/`.h`), **Slint UI** (122 `.slint` files; Slint vendored as a submodule), **Rust ≥ 1.92** for `librepcb-rust-core` (19 `.rs` files: geometry types, zip, interactive HTML BOM) built through **corrosion** and exported to C++ through **cbindgen**. Optional OCCT for STEP | [README](https://github.com/LibrePCB/LibrePCB), [rust-core/Cargo.toml](https://github.com/LibrePCB/LibrePCB/blob/master/libs/librepcb/rust-core/Cargo.toml) |
| Canvas | `SlintGraphicsView::render()` paints the Qt `GraphicsScene` with `QPainter` (optionally into an OpenGL FBO) into a `QPixmap`, then returns a `slint::Image` | [slintgraphicsview.cpp](https://github.com/LibrePCB/LibrePCB/blob/master/libs/librepcb/editor/graphics/slintgraphicsview.cpp) |
| Popularity | ~3.0k stars, 343 forks; OpenSSF Best Practices badge; Patreon; Discourse; Matrix | GitHub README |
| Governance | Effectively one lead maintainer (@ubruhin). "Ask for feedback first". No major or file-format changes outside planned windows. An explicit **LLM policy**: declare any LLM use, verify output in detail, no LLM-written PR descriptions | [CONTRIBUTING.md](https://github.com/LibrePCB/LibrePCB/blob/master/CONTRIBUTING.md) |

**File format and release policy** ([release_workflow.md](https://github.com/LibrePCB/LibrePCB/blob/master/dev/doxygen/pages/release_workflow.md)):

- The file-format version equals the application major version (`LIBREPCB_FILE_FORMAT_VERSION "2"`). "We consider
  *any* file format change as breaking because we don't provide forward compatibility at all". Any format change
  forces a major release.
- File-format changes are merged only while the next major release is in development: "every 1–2 years there is a
  window of a few months". The reason given: "accepting file format changes at any time causes a lot of overhead and
  complexity."
- Only the latest release is maintained. Tags are GPG-signed. A `FILE_FORMAT_STABLE` flag puts a warning in the GUI of
  unstable builds.

**Library model** ([library.md](https://github.com/LibrePCB/LibrePCB/blob/master/dev/doxygen/pages/library.md)):

- **Symbol** is graphics for schematics.
- **Package** is graphics for boards (with 3D).
- **Component** is "a generic (abstract) electrical device" with signals.
- **Device** is "a concrete (i.e. purchaseable) electrical device": component + package + pin-to-signal map, plus parts
  (MPNs).
- Categories are separate.
- Each element is a directory named by UUID under `sym/`, `pkg/`, `cmp/`, `dev/`, `cmpcat/`, `pkgcat/`, with an `.lp`
  S-expression file and a version marker file. Libraries are `.lplib` directories and are downloaded through a JSON
  REST API (`/api/v1/libraries/…`, paginated)
  ([server_api.md](https://github.com/LibrePCB/LibrePCB/blob/master/dev/doxygen/pages/server_api.md)).

**Mapping to ElektraWatt:**

| LibrePCB | ElektraWatt |
|---|---|
| Symbol | Symbol (IEC 60617 or ISO 1219 graphics, connection points) |
| Component | Function template: a "logical device" with function definitions such as coil, NO contact, PSU L/N/PE/+/− |
| Package | Mechanical part: 2D footprint plus 3D (STEP), mounting data, wiring-side connection-point positions |
| Device | Article: manufacturer, article number, ECLASS/ETIM class, accessories, function templates and mechanics |

The UUID-directory-per-object layout produces very clean git diffs. Many files per project is the price: watch
filesystem performance on Windows.

**Takeaways:**

- Slint can host a large CAD application's chrome. The canvas still needs its own renderer: LibrePCB keeps Qt for it.
- Incremental Rust works (small FFI-exported core crate).
- A strict file-format window policy protects a small team.

---

## 5. Other relevant projects

### 5.1 FreeCAD, Fritzing, Horizon EDA

| Project | Facts | Lesson |
|---|---|---|
| **FreeCAD** | LGPL-2.1 per its GitHub page; 34.4k stars; OCCT kernel plus a Python-scriptable workbench architecture. 1.0.0 tag 2024-11-18, 1.1.0 2026-03-24, weekly builds (`weekly-2026.10.14`), and a **`26.3rc1` tag (2026-10-08)** that suggests a move to calendar versioning. The FreeCAD Project Association (FPA) gives developer grants ([GitHub](https://github.com/FreeCAD/FreeCAD)) | (a) A Python workbench ecosystem drives adoption; (b) OCCT is the realistic STEP/B-rep path; (c) the "1.0 took ~20 years" story shows the cost of an over-broad scope. A commercial FreeCAD derivative (Ondsel) reportedly shut down in 2024 (unverified), a caution for open-core around a community CAD |
| **Fritzing** | GPL-3 code; documentation and parts **CC-BY-SA 3.0**; 4.8k stars; maintained by Kjell Morgenstern with support from AISLER ([GitHub](https://github.com/fritzing/fritzing-app)). Official binaries require a payment or donation, while the source stays free (unverified) | A "paid binaries, free source" model funds a small team but alienates some users. Parts under CC-BY-SA 3.0, with no design exception, create uncertainty about outputs. ElektraWatt should use 4.0 plus an exception |
| **Horizon EDA** | GPL-3.0; C++ / gtkmm3; JSON files; "pool" library (units, entities, symbols, packages, padstacks, parts); v2.7.2 (2025-12-03); 1.3k stars; essentially one maintainer ([GitHub](https://github.com/horizon-eda/horizon), [pool](https://github.com/horizon-eda/horizon-pool)) | JSON plus a SQLite index over a git-hosted pool is a good library-distribution pattern. Pool contributions go through GitHub PRs with a bot-checked review. Single-maintainer bus factor |

### 5.2 Rust CAD/EDA projects: what exists, and what failed

| Project | Status (verified) | License | Relevance |
|---|---|---|---|
| **Zed** (editor) / **gpui** | 91.9k stars; ~250 crates in one workspace; app "primarily GPL-3.0-or-later, with Apache-2.0 components"; `crates/gpui` has `LICENSE-APACHE`. gpui on crates.io is 0.2.2 (2025-10), "still pre-1.0. Breaking changes between versions are common" ([repo](https://github.com/zed-industries/zed), [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui), [crates.io](https://crates.io/crates/gpui)) | GPL-3+ / Apache-2.0 | Shows the **license split** (copyleft app, permissive framework) and a large-workspace Rust architecture. A custom GPU UI framework took a VC-funded team, which is not an option for 2–4 developers |
| **Graphite** | 27.6k stars; Rust node-graph engine plus web frontend; "100% community built and funded" ([repo](https://github.com/GraphiteEditor/Graphite)) | MIT OR Apache-2.0 | Proves a community-funded Rust 2D editor at scale |
| **LibrePCB rust-core** | See §4 | GPL-3 | Incremental Rust in C++ |
| **Fornjot** | **Archived 2026-06-19.** "This project has been shut down. Its goals were never reached." ~2.6k stars, ~19,972 commits ([repo](https://github.com/hannobraun/fornjot)) | 0BSD | **Do not build a B-rep kernel.** |
| **CADmium** | **Archived 2025-09-05**; Rust on truck, wasm and Svelte; browser CAD ([repo](https://github.com/CADmium-Co/CADmium)) | Elastic License 2.0 (not OSI) | Second failed Rust MCAD attempt. A source-available license limited the community |
| **truck** | `truck-modeling` 0.6.0, last release 2024-09-20 ([crates.io](https://crates.io/crates/truck-modeling)) | Apache-2.0 | B-rep in Rust exists but is slow-moving |
| **ruststep** | "experimental stage. DO NOT USE FOR PRODUCT" ([repo](https://github.com/ricosjp/ruststep)) | Apache-2.0 | Pure-Rust STEP is immature. Use OCCT |
| **opencascade-rs** | `opencascade` 0.3.0 (2026-08-24) ([crates.io](https://crates.io/crates/opencascade)) | LGPL-2.1 | Realistic OCCT path (see the rust-ecosystem note) |
| **kicad-rs** | `kicad_files`, `serde_kicad_sexpr` for the KiCad v6 format ([org](https://github.com/kicad-rs)) | Apache-2.0 | Shows serde-based S-expression (de)serialisation |
| **diodeinc/pcb** ("Zener") | 463 stars; Starlark-based schematic-as-code, Rust CLI, generates KiCad 10 layouts ([repo](https://github.com/diodeinc/pcb)) | MIT | There is demand for **design-as-code** and for reusing an existing tool as backend. ElektraWatt could expose a text/DSL API for generating standard circuits |

**What a Rust CAD application at scale looks like:**

- One Cargo workspace with many small crates, so incremental compiles stay fast (Zed ~250 crates).
- A permissively licensed engine or framework layer beside a copyleft application.
- An existing UI toolkit plus a **custom canvas renderer** (wgpu/vello or similar), not a custom toolkit.
- FFI only for the B-rep kernel (OCCT).
- Headless core crates tested without a GUI.

No mature open-source Rust schematic editor for electrical (IEC) work was found. ElektraWatt would be first in that
niche.

---

## 6. Commercial competitors and what makes them sticky

### 6.1 Overview

Pricing is indicative and **unverified** unless linked. Vendors rarely publish list prices.

| Product | Vendor / owner | Platform | Data model | Licensing (indicative) | Sticky because |
|---|---|---|---|---|---|
| **EPLAN Electric P8 / Pro Panel / Fluid / Preplanning / Harness proD / Smart Wiring** | EPLAN GmbH & Co. KG (Friedhelm Loh Group, sister of Rittal) | Windows | Device- and function-oriented project database. Master data: symbols, forms, parts, macros | 2026 tiers **Basic / Advanced / Pro / Premium**: Basic limited to 40 pages; PLC and bus in Advanced+; revisions, translation, multi-user and eBUILD in Pro+; API extensions Premium only ([eplan-features.md §1.1](./eplan-features.md), via [ibKastl](https://ibkastl.de/blog/lizenzierung-eplan-ab-2026)). Subscription; no public price (my estimate: low to high four figures € per seat per year, unverified) | Data Portal; OEM-mandated EPLAN deliverables (automotive and machine-building standards); report forms; macros; AML/TIA; Rittal machining and wire processing; very large trained user base. **Eplan Platform 2027 (Sep 2026) adds Eplan Copilot (AI)** ([press](https://www.eplan.com/us-en/about-us/press/the-eplan-platform-2027-has-arrived/)) |
| **SEE Electrical / SEE Electrical Expert / 3D Panel** | IGE+XAO (Dassault Systèmes group, unverified) | Windows | Expert: database-centred | Edition-based; cheaper entry than EPLAN (unverified) | Strong in France and building/installation; vendor catalogues |
| **WSCAD SUITE** | WSCAD GmbH (DE) | Windows | Project database. Part data via wscaduniverse.com | Subscription (unverified); has an "ELECTRIX AI" offering (unverified) | Large free parts universe; integrated disciplines (electrical, cabinet, building automation, P&ID, fluid) |
| **AutoCAD Electrical toolset** | Autodesk | Windows (the Mac AutoCAD lacks toolsets, unverified) | DWG plus a catalogue database (Access/SQL, unverified) | Part of the AutoCAD subscription (about US$2k/yr, unverified) | DWG ubiquity; AutoCAD skills; "free" with AutoCAD |
| **SOLIDWORKS Electrical** (Schematic, 3D) | Dassault Systèmes | Windows | Collaborative SQL Server database; live 2D/3D link with SOLIDWORKS | Perpetual + maintenance, or subscription (unverified) | Bundled with SOLIDWORKS 3D seats; harness and cabinet routing |
| **Zuken E3.series** | Zuken | Windows | Object-oriented single model across schematic, cable, panel and formboard. COM API | High-end per seat (unverified) | Harness/formboard; rail, aerospace, automotive |
| **AUCOTEC Engineering Base (ELCAD legacy)** | AUCOTEC AG (Hannover) | Windows + web | Central database (SQL Server/Oracle), multi-discipline, multi-user | Enterprise | Plant/energy/mobility; data-centric, "single source of truth" |
| **Siemens Capital Electra X** | Siemens DISW | Browser (SaaS) | Cloud | Subscription (unverified) | Siemens Xcelerator / TIA ecosystem (unverified) |
| **PC\|SCHEMATIC Automation**, **ProfiCAD**, **CADe_SIMU** | Small vendors | Windows | Mixed | Low cost or free | Price-sensitive niches. These are ElektraWatt's direct substitutes |

On the brief's examples:

- **EPLAN eVIEW** is EPLAN's cloud viewer and redlining app. With 2026 it is included in all tiers, together with
  Data Portal, eStock and eManage (eplan-features.md §1.1).
- **"Proficy"** is GE Vernova's HMI/SCADA/MES and PLC-programming family (unverified), not an ECAD tool. It is not a
  competitor.
- Free **vendor configurators** feed the EPLAN ecosystem and are integration targets rather than competitors:
  Phoenix Contact clipx ENGINEER / PROJECT complete, Weidmüller WMC, Rittal RiPanel and Rittal Therm, the Siemens TIA
  Selection Tool. EPLAN 2025 integrated clipx ENGINEER for terminal-strip planning (eplan-features.md §1.2).

### 6.2 EPLAN's moats in detail (see eplan-features.md for the full inventory)

1. **Data Portal.** More than 2 million static device data sets from about 500 manufacturers. EPLAN-format download
   requires a software service agreement. From 2026 it is integrated into device management (eplan-features.md,
   citing [EPLAN 2024](https://www.eplan-software.com/company/news/eplan-data-portal-update-01-june-2024) and
   [TimesTech](https://timestech.in/next-milestone-achieved-for-the-eplan-data-portal/)). The **EPLAN Data
   Standard (EDS)** defines data-quality levels. The ToU restricts use to EPLAN products (per the QET README).
2. **Macros and value sets.** `.ema`/`.ems`/`.emp` macros and macro projects are where companies keep their engineering
   standards. Migration of *company* macros, not drawings, is the real switching cost.
3. **Report engine.** 30+ report types driven by forms (`.f**` files): terminal diagrams, cable diagrams, connection
   lists, PLC overviews, structure-identifier overviews, revision overviews. These outputs are what customers and
   OEMs inspect.
4. **Integration chain.**
   - AML (IEC 62714) PLC exchange with TIA Portal;
   - EPLAN Smart Production with the **Rittal Wire Terminal WT C** interface (eplan-features.md);
   - Pro Panel NC data for Rittal Perforex machining;
   - ERP/PDM connectors.
5. **Ecosystem lock-in.** Panel builders receive EPLAN projects from OEM customers and must return them in EPLAN.
   ElektraWatt cannot remove this for EPLAN-mandated customers. It can serve customers who receive PDFs (like the pilot user's
   reference set) and import EPLAN deliveries.
6. **AI.** Copilot in 2027 (text and voice; part search; translations; BOMs; quality checks). QET has MCP. The bar for
   "AI-assisted engineering" is moving up across the field.

### 6.3 Differentiators an OSS tool can win on

| Differentiator | Why incumbents won't or can't match it | Evidence | ElektraWatt feature |
|---|---|---|---|
| **Open, documented, git-diffable format** | Proprietary databases (.elk/.edb, SQL Server) are part of the lock-in | KiCad and LibrePCB text formats; QET only became deterministic in 2026 | Text format, stable order, UUIDs, semantic `ew diff`, three-way merge driver |
| **No tiers, no dongles, no page limits** | Tiering is EPLAN's pricing model | P8 Basic is limited to 40 pages; PLC, revisions and API sit in higher tiers | Every feature in one edition |
| **Linux and macOS native** | All incumbents' desktop products are Windows-only | Table 6.1 | Native builds via CI; Flathub, Homebrew, winget |
| **CI-driven documentation and quality gates** | Outputs are GUI-driven; automation needs API licences | KiCad jobsets, QET CLI | `ew check` (tag/BOM/layout/revision consistency, the brief's defects 1–9), `ew jobs` |
| **Scripting and AI without an extra licence** | The EPLAN API is a paid extension | Premium tier | Python/Rhai/WASM plugins; MCP server; stable command API |
| **Data-quality checks built in** | Incumbents check what their model knows. Small shops skip the expensive tiers | Reference defects | Rule engine with severity, waivers and CI exit codes |
| **Migration import** | Incumbents have no incentive | KiCad 10 importers | QET (`.qet`), EPLAN XML/EDZ, DXF/DWG |
| **Open parts library with provenance** | Data Portal ToU limits reuse | QET EDZ README | Per-record license and source; ETIM/ECLASS classes; manufacturer-contributed packs |
| **Modern UX and speed** | Legacy codebases | QET productivity work in 2026 | Command palette, GPU canvas, instant search |
| **Price-sensitive and education markets** | EPLAN Education exists (unverified), but small panel builders, installers and emerging markets pay full price | — | Free; Dutch, German and English first |

---

## 7. Licensing

### 7.1 Constraints from dependencies

Verified on crates.io and GitHub on 2026-10-17. The rust-ecosystem note §12 has the full table.

| Dependency | License | Constraint |
|---|---|---|
| Slint 1.18.1 | `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` ([crates.io](https://crates.io/crates/slint), [LICENSE.md](https://github.com/slint-ui/slint/blob/master/LICENSE.md)) | The only OSI-clean route is GPL-3.0-only, which makes the distributed binary effectively GPL-3.0. Royalty-free 2.0 forbids embedded use, requires an AboutSlint widget or badge, and says "applications that expose the Software's APIs … cannot be distributed" ([text](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)). That last condition is risky for a plugin platform |
| gpui 0.2.2 | Apache-2.0 | GPL-3-compatible, not GPL-2-compatible |
| egui 0.37.0 / iced 0.14.0 | MIT OR Apache-2.0 / MIT | No constraint |
| cxx-qt 0.10.0 (+ Qt 6) | MIT OR Apache-2.0 (Qt: LGPL-3.0 / GPL / commercial) | Dynamic Qt linking. LGPL-3 anti-tivoization is irrelevant on the desktop |
| OCCT | LGPL-2.1 + `OCCT_LGPL_EXCEPTION.txt`, or commercial ([GitHub](https://github.com/Open-Cascade-SAS/OCCT)) | Fine with GPL-3 (LGPL-2.1 §3 allows conversion to GPL). With a permissive app, static linking needs relinkable objects |
| opencascade-rs 0.3.0 | LGPL-2.1 | As OCCT |
| LibreDWG | GPL-3.0-or-later; DWG writing "good enough for everything but R2007", R2010–R2018 writing still causes CRC errors ([GitHub](https://github.com/LibreDWG/libredwg)) | In-process use requires a GPL-3-compatible app. Otherwise run it out of process |
| ODA Drawings SDK / ODA File Converter | Proprietary (unverified) | Cannot be linked into GPL. A user-installed external converter is possible |
| wasmtime 49.0.3 | Apache-2.0 WITH LLVM-exception | Fine |
| QET code | GPL-2.0-or-later | Portable into GPL-3.0-or-later only |
| KiCad code | GPL-3.0-or-later | Portable into GPL-3.0-or-later only |
| LibrePCB / Horizon code | GPL-3.0 | Same |

### 7.2 Options compared

| Option | Pros | Cons | Fit |
|---|---|---|---|
| **GPL-3.0-or-later** | ECAD norm (KiCad, LibrePCB, Horizon, Fritzing; QET GPL-2+). Can port QET, KiCad, LibrePCB and Horizon code. Slint (GPL) and LibreDWG usable. Blocks proprietary forks by vendors. Accepted by distributions | Companies cannot embed the engine in proprietary products. Some corporate contributors hesitate. Needs a plugin exception for proprietary plugins | **Recommended for the application and engine** |
| AGPL-3.0-or-later | Also covers network use (SaaS) | Adds nothing for an offline desktop app; many companies ban AGPL internally | Use only for an optional server (parts sync, collaboration) |
| MPL-2.0 | File-level copyleft. Proprietary embedding allowed. GPL-compatible through the secondary-license clause | Cannot absorb GPL code (no QET or KiCad porting). With Slint the binary is GPL anyway, so the main benefit disappears | Second choice if engine embedding by partners becomes strategic |
| MIT OR Apache-2.0 (Rust norm) | Maximum adoption; patent grant (Apache); zero friction with crates | No porting of GPL code. Slint only through non-OSI terms. LibreDWG out of process. A vendor could ship a closed fork | **Recommended for the format, schema, SDK and reusable parser crates** |
| LGPL-3.0 | Library copyleft | Awkward with Rust static linking and generics/monomorphisation | Not recommended |
| EUPL-1.2 | EU-drafted, 23 languages, downstream compatible with GPL-2/3, AGPL, MPL, LGPL | GPL code cannot be pulled *into* EUPL code (no QET or KiCad porting). Rare in Rust | Possible, but GPL-3+ gives more |

### 7.3 Recommended licence layering

| Layer | Licence | Rationale |
|---|---|---|
| `elektrawatt` app, GUI, engine, report engine, importers that port GPL code | **GPL-3.0-or-later** (SPDX), plus **an additional permission (§7) for plugins** that talk only through the published plugin interface | Copyleft protects the commons. Enables Slint/GPL, LibreDWG and QET porting. The exception allows a proprietary vendor plugin ecosystem (e.g. a Rittal or Komax exporter) |
| `ew-format` (spec, JSON Schema/grammar, reference reader/writer), `ew-plugin-api` (WIT), standalone parsers (DXF writer, AML, EDZ part.xml) that do not depend on GPL code | **MIT OR Apache-2.0** | Anyone can read and write ElektraWatt files, including ERP, MES, machine vendors and even EPLAN converters. The open format is the strategic asset |
| Symbols, symbol libraries, page and report templates, macros, title blocks | **CC-BY-SA-4.0 plus a design exception** modelled on KiCad's: designs and generated outputs are released from Art. 3 | Projects embed library copies (like QET and KiCad), so the exception keeps users' drawings free of share-alike. 4.0 covers EU database rights |
| Part data authored by the project (manufacturer, article, dimensions, ETIM class) | **CC0-1.0**. Third-party packs carry a per-record `license` and `source` field (SPDX ID) | Facts are not copyrightable, but EU *sui generis* database rights (Directive 96/9/EC) can apply to collections. CC0 removes friction |
| Documentation | **CC-BY-4.0** | Easy reuse by integrators and trainers |
| Example and tutorial projects | **CC0-1.0** | Users copy freely into commercial work |
| Optional server components (parts sync, collaboration, web viewer) | **AGPL-3.0-or-later** | Prevents closed SaaS forks |

Mechanics:

- **[REUSE](https://github.com/fsfe/reuse-tool) compliance**: SPDX headers on every file and a `LICENSES/` directory,
  as LibrePCB does. `reuse lint` runs in CI (reuse-tool v6.2.0).
- **[cargo-deny](https://github.com/EmbarkStudios/cargo-deny)** (0.20.2) enforces a license allowlist: MIT, Apache-2.0,
  BSD-2/3, ISC, Zlib, MPL-2.0, Unicode-3.0, LGPL-2.1 (OCCT only), GPL-3.0 (Slint and LibreDWG only). It rejects
  GPL-2.0-only and AGPL in the app. The `ew-format` crates get a stricter permissive-only list so they never pull GPL
  in.
- Generate third-party notices into the About dialog (also satisfies Slint's attribution if that route is ever used).
- **Plugin exception wording.** Draft it with counsel. Precedents: GCC Runtime Library Exception, Classpath exception,
  SPDX `GPL-3.0-linking-exception`. The exception must exist from the first commit, because adding it later needs
  every copyright holder's consent. WASM component plugins in wasmtime that only import the MIT/Apache WIT world make
  the boundary technically clear.

### 7.4 Contributor terms: DCO vs CLA

| | DCO 1.1 (`Signed-off-by`) | CLA |
|---|---|---|
| Friction | Low: a git flag and a bot check | High: legal review, signatures |
| Relicensing later | Impossible without all contributors, except via "or later" and permissive layers | Possible: enables dual licensing / open-core |
| Community trust | High (Linux, GitLab moved to DCO) | Lower: contributors fear proprietary capture |
| Fit | **Recommended** | Only if the pilot user or a company plans commercial dual licensing. This note does not recommend that |

Inbound equals outbound licence. No copyright assignment. Watch the Dutch employer-ownership rule: under Auteurswet
art. 7, an employer is deemed the author of works employees create in their job (verify with counsel). Code written by
the pilot user staff on company time is therefore the pilot user's to license. Get a written the pilot user statement releasing it under the
project licence.

### 7.5 Trademark policy for "ElektraWatt"

- **Clearance first.** Run TMview/EUIPO/BOIP searches in Nice classes 9 (software) and 42 (SaaS and design services).
  "Electra"-named products already exist in electrical CAD: Electra Cloud, and Siemens *Capital Electra X* (unverified
  lineage). "Elektra" is common. This is a real confusion risk in the same market.
- **Register** a word mark, and later a logo, at BOIP (Benelux) or EUIPO. Fee size (unverified): EUIPO about €850 for
  one class.
- **Holder:** the pilot panel builder or the founder at first, transferred to a foundation (Stichting ElektraWatt) once it exists.
- **Policy**, modelled on the Rust Foundation, Mozilla and Blender policies:
  - unmodified builds may use the name;
  - modified distributions must rebrand, or say "based on ElektraWatt";
  - nominative use ("compatible with ElektraWatt", "ElektraWatt plugin") is allowed;
  - no use in company names or domains without permission;
  - certification marks ("ElektraWatt-verified part data") are reserved for manufacturer data packs that pass the
    data-quality checks.

### 7.6 Commercial model options

| Model | Examples | Fit for ElektraWatt |
|---|---|---|
| Services company plus donations and sponsors | KiCad Services Corp (support contracts, CERN as customer); Collabora for LibreOffice (unverified) | **Best.** Support SLAs, signed LTS builds, migration services (EPLAN/QET), custom report and label templates, training |
| Sponsored features and bounties | Allegro importer funded by Quilter (KiCad) | **Good.** Manufacturers sponsor machine exports (Komax, Rittal) or data packs |
| Paid binaries, free source | Fritzing (unverified details) | Possible, but friction with "open" positioning |
| Open-core | GitLab; FreeCAD/Ondsel (unverified) | **Avoid** for engineering features. A small team cannot afford community distrust or a fork |
| Hosted optional services (AGPL) | Parts-library sync, team server, web viewer | **Later.** Revenue without closing the core |
| Foundation with corporate tiers | KiCad sponsor tiers $1k–$15k; Blender Development Fund (unverified) | Medium term: a Dutch *stichting* or an umbrella (see §8.4) |

---

## 8. Community and governance for a 2–4 person core team

### 8.1 Contribution workflow

- **Repository:** GitHub `ironkode787/ElektraWatt` as a monorepo (Cargo workspace). Use issue templates for bug,
  feature and part-data requests, and the `good first issue` / `help wanted` labels (as LibrePCB does).
- **PR rules:**
  - one approving review from a maintainer;
  - CI green: `cargo fmt`, `clippy -D warnings`, tests, `cargo deny`, `reuse lint`, golden-file round-trip tests of
    the file format, PDF snapshot tests;
  - DCO check;
  - conventional-commit titles for changelog generation (QET uses git-cliff: `cliff.toml`).
- **Contribution tracks for non-coders:** symbols, part data, report templates, translations and test projects. QET
  and KiCad show the library is the largest contribution channel. Use KLC-style rules plus a library linter in CI, and
  **generate** from data where possible (KiCad: 78 % of footprints generated).
- **AI contribution policy** (this team uses AI assistance itself):
  - disclose AI use in the PR;
  - a human author verifies and owns every line, and the DCO sign-off covers it;
  - no AI-written issue or PR discussion;
  - never feed third-party licensed content (QET elements, EDZ, IEC standard texts) into model training.

  Precedents: [LibrePCB CONTRIBUTING](https://github.com/LibrePCB/LibrePCB/blob/master/CONTRIBUTING.md) and KiCad's
  tool-generated-contributions policy.
- **Security:** `SECURITY.md` with private reporting. Fuzz every importer (`cargo-fuzz` on `.qet`, DXF, EDZ, AML,
  STEP). QET's CVE-2026-15037 shows project files are an attack surface. Sign releases and tags. Aim for an OpenSSF
  Scorecard and the Best Practices badge (LibrePCB has one).

### 8.2 Decision-making, RFCs, roadmap

- **`GOVERNANCE.md`:**
  - roles: project lead, maintainers, committers, contributors;
  - lazy consensus with a 72-hour objection window for normal changes;
  - the project lead breaks ties until a foundation board exists.
- **RFCs** for anything with a long tail: the file format, the plugin API (WIT), the data-model ontology (device,
  function, potential, structure), CLI/API stability and licensing. Keep them in `rfcs/NNNN-title.md` as PRs, with a
  "final comment period" of 7 days. This is a lightweight version of Rust's process.
- **ADRs** in `docs/adr/` for internal architecture choices (GUI toolkit, renderer, OCCT binding).
- **Public roadmap:** `ROADMAP.md` plus a GitHub Projects board with milestones that map to the design document's work
  packages. Mark items "funded" (NLnet milestone) or "help wanted".
- **Code of conduct:** [Contributor Covenant](https://github.com/EthicalSource/contributor_covenant) 2.1 (latest tagged
  version). Name enforcement contacts outside the core team where possible.

### 8.3 Release cadence and LTS

| Phase | Cadence | Policy |
|---|---|---|
| Pre-1.0 (0.x) | Monthly 0.x releases plus nightlies | Format may change. Every release reads all previous 0.x files through an upgrade chain (the KiCad approach). Unstable-format warning in the GUI (the LibrePCB `FILE_FORMAT_STABLE` idea) |
| 1.x onward | **Major yearly** (KiCad's rhythm), **minor about quarterly**, patches as needed (KiCad shipped 7 patch releases in 6.5 months after 10.0) | **File-format changes only in majors** (LibrePCB's rule). Always read older formats. Never write older formats unless an explicit "save as v(N-1)" exists |
| LTS | None until funded. Then one LTS per two years, 24 months of security and data-loss fixes, sold as a support product | Panel builders value stability. LTS is a natural paid service and avoids burdening 2–4 developers |

### 8.4 Funding sources (Netherlands-based project)

| Source | Fit | What it pays | Notes |
|---|---|---|---|
| **NLnet / NGI Zero** (e.g. NGI0 Commons Fund) | **High**. NLnet is in <city> and funded KiCad's €50k grant in 2022. LibrePCB has been NLnet-funded (unverified) | Milestone-based grants. Typically €5k–€50k for a first proposal, with follow-ups possible (unverified) | Requires an OSI/FSF licence. Work is paid per milestone. Calls roughly every two months (unverified). NLnet also arranges free support services: security audit, accessibility, packaging, REUSE/licensing (unverified). Pitch: an open, interoperable ECAD data format and migration away from proprietary lock-in. EU NGI funding after 2025 has been politically uncertain (unverified); check current calls |
| **WBSO** (Dutch R&D payroll-tax credit) | **High for the pilot user** | Reduces payroll tax for R&D hours of the pilot user employees (unverified details) | Company-side; does not conflict with OSS |
| MIT-regeling / innovation subsidies (NL) | Medium | SME innovation projects (unverified) | Company-side |
| Sovereign Tech Agency (DE) | Low–medium | Targets open digital infrastructure (unverified) | Possible for reusable crates (STEP/DXF/AML parsers), not for the app |
| Prototype Fund (DE) | Low | Individuals resident in Germany (unverified) | Only if a team member lives in Germany |
| GitHub Sponsors | Low–medium | Individual and company donations | Easy start |
| Open Collective (EU fiscal host) | Medium | Transparent budget; invoices to companies | Host fees apply (unverified ~10 % for Open Source Collective) |
| Corporate sponsor tiers | Medium–high (later) | KiCad model: $1k / 3k / 7.5k / 15k a year | Panel builders, wholesalers, component manufacturers |
| Manufacturer data partnerships | **Strategic** | Mostly in kind (data packs, 3D models), sometimes cash | Manufacturers already produce EPLAN EDS data. Ask them to publish it in the open format too. ETIM/ECLASS data pools are common in the Netherlands and Belgium (e.g. 2BA for the installation sector, unverified) |
| Sponsored features / bounties | Medium | Komax/Rittal exports, EPLAN importers | KiCad Allegro precedent |

**Legal home over time:**

1. Start under the pilot user or the founders.
2. Then a fiscal host (Open Collective Europe).
3. Then a Dutch **stichting**, or an umbrella foundation.

Umbrella options:

- Linux Foundation: KiCad's choice, but US-based and "not charitable".
- Software Freedom Conservancy: US-based (unverified).
- Eclipse Foundation AISBL: Brussels; has industrial-automation projects such as 4diac (IEC 61499), but requires the
  Eclipse Contributor Agreement instead of the DCO (unverified).

A stichting is simple and local. Revisit it once there are more than 3 regular external contributors or more than
€50k a year in income.

### 8.5 Community channels and localisation

- GitHub Discussions (Q&A), Matrix for chat, and later Discourse (as LibrePCB has). The docs site is an mdBook in the
  repo.
- Translations: Project Fluent (`fluent-rs`) strings plus hosted Weblate (unverified hosting terms). Prioritise
  English, Dutch and German: the pilot user and EPLAN's home market. Translations also cover symbol and report text, which QET
  handles per element.
- Publish a `publiccode.yml` (QET has one) for EU public-sector discoverability.
- Produce migration guides ("from QET", "from EPLAN PDF/XML") and a reference project, the pilot user's <drawing id> set rebuilt
  in ElektraWatt with all reports. These are the strongest adoption tools.

---

## Implications for ElektraWatt

1. **Be device-first, not drawing-first.**
   - Make the device (article plus functions plus structure identifiers) the persistent primary object.
   - Make schematic symbols, overview and detail representations, terminal-strip entries, 2D and 3D layout placements,
     and BOM rows *views* that reference the device UUID.
   - This removes the root cause of reference defects 1–4 and 7, and is the main differentiator against QET.
2. **Model potentials and wire rules explicitly.**
   - Use potential objects with IEC 60757 colour and cross-section rules (the folio 1 table becomes project data).
   - Inherit wire properties automatically and flag overrides (defect 8).
3. **Bind every title-block field to a project property.**
   - Revisions are managed objects (revision index, date, author, change note).
   - Page navigation, TOC and section separators are computed (defects 5–7).
4. **Use a KiCad-grade text format.**
   - Line-stable ordering, UUIDs on every object, a `generator` stamp, and a monotonic format version with an
     automatic upgrade chain.
   - Per-user state goes in a separate file that is ignored by git.
   - Ship `ew diff` (semantic) and a git merge driver.
   - Publish the spec and the reference reader/writer as **MIT OR Apache-2.0** crates.
5. **Freeze the format by policy.**
   - Before 1.0, monthly releases with upgrade-on-read.
   - After 1.0, format changes only in yearly majors (LibrePCB rule).
   - A GUI warning on unstable-format builds.
6. **Make the API the internal command bus.**
   - All edits go through `Transaction { commands }` in the core crate, with undo derived from it.
   - The GUI, CLI (`ew`), scripting (Python or Rhai), WASM plugins and an **MCP server** all use the same commands.
   - Do not repeat KiCad's multi-year API retrofit. The schematic API is still empty in 10.0.7.
7. **Ship a headless CLI from the first public release** with these commands:
   - `ew export pdf|bom|wires|terminals|cables|plc|komax|dxf`;
   - `ew check` (rule engine with CI exit codes: tag/BOM sync, layout↔schematic linkage, device without part,
     inconsistent tag schemes, revision drift);
   - `ew jobs run` (KiCad-style jobsets).

   This matches QET's 2026 CLI and MCP baseline and turns the brief's defects into automated gates.
8. **Make the QET importer a first-class product.**
   - Port QET parsing logic under GPL-3.0-or-later with attribution.
   - Map element information bags to devices, master/slave links to device functions, folio reports to interruption
     points, and conductor text properties to wires and potentials.
   - Do not rely on deterministic ordering or UUIDs in pre-2026 files.
   - Build a regression corpus from QET's `examples/*.qet` (23 files, 22 distinct projects) plus the pilot user's projects.
9. **Do not bundle the QET element collection in the main library.**
   - Convert elements on demand from the user's project or installation, or ship a separate `qet-legacy` pack under
     QET's CC-BY 3.0 terms with the anti-ML clause preserved.
   - Never use it for model training.
10. **EDZ import:** user-initiated, local, part data only, no `.ema` geometry, never auto-download, and a ToU notice.
    Copy QET's README posture. Pursue direct manufacturer permission for open data packs.
11. **Licence the application GPL-3.0-or-later** with a §7 plugin exception drafted before the first external
    contribution. Licence the format, SDK and parser crates MIT OR Apache-2.0.
    - Content: CC-BY-SA-4.0 with a design exception (symbols, templates), CC0-1.0 (project part data, examples),
      CC-BY-4.0 (docs).
    - AGPL-3.0-or-later for any future server.
    - Take contributions under the **DCO**, with no CLA.
    - Enforce with `reuse lint` and `cargo deny` in CI.
12. **Secure IP ownership now.** Get a written statement from the pilot panel builder that code produced by its staff is released
    under the project licences (Dutch Auteurswet art. 7). Decide who holds the trademark.
13. **Trademark:** run a clearance search for "ElektraWatt" against "Electra"-named ECAD products in classes 9 and 42
    before public launch. Register at BOIP or EUIPO. Publish a Blender/Rust-style trademark policy.
14. **Do not build a UI framework or a B-rep kernel.**
    - Use an existing toolkit (Slint, egui or iced; see the rust-ecosystem note) with a custom wgpu canvas.
    - Use OCCT via FFI for STEP and B-rep.
    - Model cabinets, rails and ducts as parametric prisms in Rust.
    - Fornjot's and CADmium's shutdowns are the warning.
    - If Slint is chosen, its GPL-3.0-only option fits the recommended licence. Do not rely on the royalty-free licence
      for a plugin platform, because it forbids "exposing the Software's APIs".
15. **DWG:** write and read DXF natively. Offer DWG through LibreDWG (GPL-3, compatible with the recommended licence;
    R2010+ writing is unreliable), or through a user-installed external converter.
16. **Governance files on day one:**
    - `GOVERNANCE.md`, `CONTRIBUTING.md` with the AI policy, `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1),
      `SECURITY.md`, `ROADMAP.md`;
    - `rfcs/` for the format, API and ontology; `docs/adr/` for architecture.
17. **Funding sequence:**
    1. WBSO for the pilot user-employed developers;
    2. an NLnet NGI0 proposal scoped to the open format, the QET/DXF importers and the CLI checks, as €30–50k of
       milestones;
    3. GitHub Sponsors and Open Collective at public launch;
    4. sponsor tiers and manufacturer data partnerships after v1;
    5. support, LTS and migration services as the long-term revenue, in the KiCad Services Corp style.
18. **Community levers:**
    - library contributions with a linter and generators (KiCad's 78 % generated);
    - Dutch, German and English UI and docs;
    - migration guides;
    - the the pilot user <drawing id> reference project rebuilt as a showcase with every report EPLAN would produce.
19. **Positioning:** "EPLAN-class engineering data, open format, no tiers, every OS, CI-checked drawings". Target small
    and medium panel builders and integrators who deliver PDFs today (like the pilot user) before EPLAN-mandated OEM chains.
    Import EPLAN deliveries; do not promise round-trip to EPLAN in v1.

## Open questions

1. **Copyright owner:** who owns the code, the pilot panel builder (employment) or the repository owner (`ironkode787`)? Will the pilot user
   sign a release statement and accept the GPL-3.0-or-later plus permissive-format layering?
2. **Plugin exception:** is a §7 plugin exception wanted, i.e. should proprietary plugins (manufacturer exporters,
   company-internal automation) be allowed, and with what boundary (WASM/WIT only)? Counsel must draft the text.
3. **Commercial intent:** is a commercial entity (support, LTS, migration services) planned? If dual licensing is ever
   wanted, a CLA must be decided *before* the first external contribution, which this note advises against.
4. **Trademark:** is "ElektraWatt" clear of conflicts with "Electra"-branded ECAD products in classes 9 and 42? Who
   registers it, and where (BOIP or EUIPO)?
5. **EDZ legality:** what do the current EPLAN Data Portal ToU (§5.4) and individual manufacturers' download terms say?
   Are some manufacturers (Phoenix Contact, Weidmüller, Festo, Schneider, Rittal, Murrelektronik, all present in the
   reference set) willing to license their data for open use?
6. **QET elements:** is the anti-ML clause compatible with redistributing elements in a separate pack, or is
   on-demand conversion the only safe route? Would the QET team grant a clarification or relicensing (e.g.
   CC-BY-4.0) for import use?
7. **Relationship with QET:** compete, or coordinate? A shared open exchange format, or an ElektraWatt→QET exporter,
   could turn QET's community into an ally. Does the user want outreach to the QET maintainers?
8. **NLnet scope and timing:** which milestone set fits an NGI0 call (open format plus importers plus checks), and
   who writes the proposal? The current EU NGI funding status and call deadlines need checking (unverified here).
9. **Slint versus alternatives:** the licence analysis favours GPL-compatible choices. Does the GUI decision (open in
   the brief) accept that Slint's free route makes the binary GPL-3.0-only in practice?
10. **LTS demand:** do the pilot user and peer panel builders need LTS (multi-year format and behaviour stability)? Would they
    pay for it?
11. **Library governance:** who reviews part-data and symbol PRs? What is the minimum quality level (EDS-like levels:
    commercial data, logistics data, technical data, macros, 3D)?
12. **Foundation timing:** when to create a stichting versus joining an umbrella (Linux Foundation, Conservancy,
    Eclipse)? What trigger: number of external maintainers, budget, or the first corporate sponsor?
13. **Unverified items:** commercial pricing, Ondsel's shutdown, Fritzing's paid-binary terms, NLnet amounts and
    deadlines, WBSO rules, EUIPO and BOIP fees. Re-check all of these before quoting them externally. Web search was
    unavailable in this run.
