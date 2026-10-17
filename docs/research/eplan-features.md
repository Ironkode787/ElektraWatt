# EPLAN Platform feature inventory (2025 → 2027) — reference for ElektraWatt

> Research note for the ElektraWatt design team. Topic: exhaustive feature inventory of the EPLAN Platform
> (Electric P8, Pro Panel, Fluid, Preplanning, Data Portal, Smart Wiring / Smart Production, eVIEW / eBUILD / Cogineer,
> Harness proD) with a suggested ElektraWatt priority per feature, mapped to the user's reference drawing set
> *<drawing id>* (folios 1, 2, 3, 100–106, 200–206, 900–902).
> Date of research: 2026-10-17.

## 0. Scope, method and caveats

- **Sources.** Primary sources are the EPLAN online help ("Infoportal", `eplan.help`, versions 2.8 → 2026, plus the
  2027 API reference), EPLAN press releases, EPLAN 2026 "performance descriptions" (PDF), EPLAN TechTips, and a few
  reseller and partner pages. In this research session, direct page fetching (WebFetch/curl) was blocked by the egress
  proxy, so I verified facts through web-search result extracts of those pages. Citations link to the page that
  supports the claim. **Anything marked "(unverified)" comes from my own knowledge of EPLAN** and should be checked
  against a running EPLAN installation or the help before anyone relies on it.
- **Versions.** EPLAN switched from 2.x numbering to year numbering (2022, 2023, 2024, 2025, 2026). **Eplan Platform 2027
  was released in September 2026** ([press](https://www.eplan.com/us-en/about-us/press/the-eplan-platform-2027-has-arrived/)),
  so the inventory covers 2025, 2026 and the new 2027 features. Most functional behaviour described in the help has been
  stable since 2.8/2.9. Where a page from an older help version is cited, the 2026 help has the same topic.
- **Property IDs.** EPLAN identifies every property by a numeric ID written in angle brackets, e.g. `<20100>`
  ([EPLAN Properties: Basics](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/properties_k_prinzip.htm)).
  I list the IDs I could verify; the rest are named without an ID.
- **Priority legend** (for ElektraWatt, given the brief's fixed v1 decisions):
  - **Must**: required for v1. These are the core schematic and data model, the outputs that prevent the reference
    defects, and the four mandated v1 extras: PLC/I-O, manufacturing outputs, 3D panel layout, and import of
    EPLAN/QET/DXF/EDZ.
  - **Should**: v1 if capacity allows, otherwise v1.x. Valuable, but the core works without it.
  - **Could**: nice to have, with clear value for some users.
  - **Later**: out of scope for v1. Keep the data model open for it.

---

## 1. Release and portfolio context

### 1.1 Portfolio and tiers (2026 restructuring)

With the 2026 release EPLAN rebuilt its portfolio around three products: **Eplan Preplanning** (for operators and
planners), **Eplan Electric P8** (for machine builders) and **Eplan Pro Panel** (for control-cabinet construction).
The cloud collaboration apps, the **Data Portal**, cloud parts management **eStock** and data management **eManage**
are now included in all three ([EPLAN press, 2026](https://www.eplan.com/in-en/about-us/press/more-of-everything-and-perfectly-customised/);
[GlobalSpec reprint](https://insights.globalspec.com/article/24350/new-software-new-portfolio-the-eplan-platform-2026)).
Electric P8 now comes in four expansion levels: **Basic, Advanced, Pro, Premium**
([EPLAN performance description P8 Basic 2026](https://www.eplan.com/content/dam/eplan/corporate/performance-descriptions/2026/en/performance-description-eplan-electric-p8-basic.pdf);
[German P8 Advanced/Pro/Premium 2026](https://www.eplan.com/content/dam/eplan/corporate/performance-descriptions/2026/de/leistungsbeschreibung-eplan-electric-p8.pdf)).

Feature gating per tier, as summarised by a German EPLAN partner ([ibKastl, "Lizenzierung Eplan ab 2026"](https://ibkastl.de/blog/lizenzierung-eplan-ab-2026))
and a user forum ([CAD.de](https://ww3.cad.de/foren/ubb/Forum467/HTML/015655.shtml)). This is a secondary source: check
it against the official PDFs.

| Capability | Basic | Advanced | Pro | Premium |
|---|---|---|---|---|
| Data Portal, eStock, eManage, eView | yes | yes | yes | yes |
| Single-line and multi-line ("all-pole") schematics, 2D mounting plates | yes | yes | yes | yes |
| Fluid power schematics (now part of P8) | yes | yes | yes | yes |
| Page limit removed (Basic is limited to 40 pages) | — | yes | yes | yes |
| PLC and bus-system planning, "networked connections", ERP integration | — | yes | yes | yes |
| Revision management, multilingual translation, project management, standards exchange, multi-user, eBUILD | — | — | yes | yes |
| Rights management, FieldSys cable-length calculation | — | — | yes | yes |
| ERP/PDM integration (extended), **EPLAN API extensions** | — | — | — | yes |

The sources disagree on how the old 2025 products (Select, Professional, …) map to the new tiers. The forum post also
reports that **Fluid is not included in Pro Panel** ([CAD.de](https://ww3.cad.de/foren/ubb/Forum467/HTML/015655.shtml)).

*ElektraWatt takeaway:* EPLAN locks PLC planning, revision management, translation and the API behind the higher tiers.
ElektraWatt can offer all of these in one open-source edition. This differentiates it most for small panel builders
such as the pilot user.

### 1.2 What changed per release

| Release | Notable new functions (verified unless marked) |
|---|---|
| **2025** (summer 2024) | Macro preview. Device-specific property displays. Comments navigator in the ribbon (own comments, history, status). **Linked images and QR codes placed on symbols**, updated when the schematic changes. Cable dependencies to components such as current transformers, carried into connection and cable lists. Pre-assembled cables defined in P8 and cabled with the new **Eplan Cable proD**. Terminal-strip planning through Phoenix Contact **clipx ENGINEER** ([EPLAN press 2025](https://www.eplan.com/de-en/about-us/press/new-software-with-smart-features-for-easier-and-faster-engineering/)). |
| **2026** (Oct 2025) | Portfolio and tier rebuild (above). Device management opens **device selection in the Data Portal directly** ([GlobalSpec](https://insights.globalspec.com/article/24350/new-software-new-portfolio-the-eplan-platform-2026)). Pro Panel: new **Wiring Manager**, one dialog to list, route and export the routed connections of a layout space ([EPLAN help 2026 news](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2026/Content/htm/news_p_propanel_verdrahtungsmanager_488766.htm)). Pro Panel **multi-view**: save and switch between up to ten 3D views ([Ecodesign summary](https://ecodesign.net.nz/eplan-2026/)). eBUILD included in the P8 Pro level. Download Center open to all users. |
| **2027** (Sep 2026) | **Eplan Copilot** AI assistant (text and voice prompts: product knowledge, cross-project part-number search, translations, BOMs, quality checks). **Bidirectional sync** of device and master data (formulas, plot frames, symbol libraries) between cloud and local. Preplanning: automated symbol rotation and arrangement, line interruptions, process flow direction, project generation in 3D. **Wire-end sleeves assigned by wire class** and documented for automatic or manual wire assembly. **Compare up to 10 components** with filtering on differences. Graphical editor: **built-in preview** of a device's functions or macros without inserting it. **Tag management** for user-defined tags. "Intelligent search and selection". Automated fitting selection and improved cable data exchange for machine wiring. EPLAN claims more than 500 enhancements ([press "2027 has arrived"](https://www.eplan.com/us-en/about-us/press/the-eplan-platform-2027-has-arrived/); [press "Now is best to test"](https://www.eplan.com/us-en/about-us/press/now-is-best-to-test-the-eplan-platform-2027/); [product page](https://www.eplan.com/us-en/products/eplan-platform/)). |

---

## 2. Platform fundamentals

### 2.1 Projects, files and templates

| Feature | What it does / how EPLAN implements it | Why panel builders need it | Ref. folios | Prio |
|---|---|---|---|---|
| **Project file and data directory** | A project is a `*.elk` file plus a `*.edb` data directory with the project database and project-local master data ([help: project icons](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/projects_k_icons.htm)). | One self-contained project per cabinet or job. | all | Must (as a file-based project container) |
| **Project states and archive formats** | `*.elp` packed project (zip, read-only open), `*.els` backup, `*.zw1` zipped filed-off backup, `*.elx` archived, `*.elr` completed / write-protected project, `*.ell` revision project ([help: project icons](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/projects_k_icons.htm); [help: complete project](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/revisionmgtgui_h_projektabschliessen.htm)). | Archiving at delivery, frozen "as-built" revisions, sending projects to customers. | 1 (revision field) | Must: single-file archive plus a "completed / read-only" state |
| **Basic projects and project templates** | `*.zw9` **basic project** (settings, pages, master data; the newer mechanism) and `*.ept` **project template** (older). Samples: `IEC_bas001.zw9`, `IEC_tpl001.ept`, `GOST_bas001.zw9`. An action `XPrjConvertBaseProjectsAction` converts templates to basic projects ([help: basic projects](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/projects_k_basisprojekteerweitert.htm); [2.9](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/projects_k_basisprojekt.htm)). | A company standard (the pilot user title block, colour table, numbering, cross-reference format) that every new project inherits. Prevents defects 5 and 6. | 1, every title block | Must |
| **Project types** | "Schematic project" vs **"Macro project"**, set under "Type of project" ([help: macro projects](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/macrosgui_h_makroprojekteerstellen.htm)). | Macro libraries are maintained as ordinary projects. | — | Should |
| **Project properties** | Project-level property set: name, description, customer, job number and so on. Includes "supplementary fields" `<10901>` for projects and `<11901>` for pages ([help: supplementary fields](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/userdefproperties_k_zusatzfelder.htm)), plus a cloud project ID `<10120>`. Shown in title blocks through placeholder ("special") texts. | One source of truth for title-block data. | 1, every title block (Customer, Object, Project, Drawing, Description) | Must |
| **Settings layers and schemes** | Settings at Project / User / Workstation / Company level. Most settings are stored as named **schemes** (e.g. report, numbering, export, check schemes), exportable as XML (unverified on the exact layer names). | Company standards are portable and versionable. | — | Must (as TOML/JSON profiles) |
| **Project export as XML (`*.epj`)** | A project can be exported from the Backstage view or project management. The exported `*.epj` is XML, can be edited externally and re-imported ([help: export project](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/projects_h_exportieren.htm)). | Key route for **EPLAN → ElektraWatt migration** (v1 decision 4). | — | Must (import side) |

### 2.2 Master data

| Feature | Description | Prio |
|---|---|---|
| **Master data types** | Symbol libraries `*.slk` ([help: symbol variants](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/symboleditorgui_h_mitsymbolvariantenarbeiten.htm)). Forms `*.f01`…`*.f33` (one extension per report type, §7). Plot frames (title blocks; `*.fn1`, unverified). Macros `*.ema` / `*.ems` / `*.emp` (§5). Parts database, translation dictionary, images. | Must |
| **Project-local copies and synchronisation** | Used master data (symbols, forms, plot frames) is copied into the project, so the project can travel. "Synchronise project master data" updates the copies from the system master data (unverified wording). | Must: projects must embed the master data they use, or every shared project breaks. |
| **Standards libraries** | IEC (`IEC_symbol`, `IEC_single_symbol`), NFPA, GOST and GB symbol libraries and matching basic projects (unverified library names; GOST templates verified above). | Must for IEC 60617; NFPA Later |
| **"Standards exchange"** | Swaps a project's symbol standard, e.g. IEC to NFPA. Listed as a Pro-tier feature ([ibKastl](https://ibkastl.de/blog/lizenzierung-eplan-ab-2026)). | Later |

### 2.3 Property system

| Feature | How EPLAN implements it | Why it matters | Prio |
|---|---|---|---|
| **Numeric, typed, indexed properties** | Each property has an ID; indexed properties take `[n]` (e.g. part number `<20100>[3]`), and index `[0]` outputs all values ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/properties_k_prinzip.htm)). The API exposes them as `Properties.Function`, `Properties.Page`, `Properties.Project` and similar enums ([API: EPLAN properties](https://www.eplan.help/en-us/Infoportal/Content/api/2025/EPLAN_Properties.html)). | Reports, forms, labels and scripts all address properties by stable IDs. | Must (a stable property registry with IDs and namespaces) |
| **Key function property IDs (verified)** | `<20010>` DT displayed, `<20011>` Function text (manual), `<20031>` Function text (automatic), `<20012>` DT prefix, `<20024>` Mounting site (describing), `<20027>` Technical characteristics, `<20030>` Terminal / pin designation, `<20038>` Connection point designations (all), `<20039>` Connection point descriptions (all), `<20100>` Part number. Structure: `<1220>` Mounting location, `<1420>` Installation site. Connection colour (multilingual) `<31061>` ([help: properties for direct editing](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/inplaceeditinggui_k_eigenschaften.htm); [API FUNC_TEXT](https://www.eplan.help/en-us/Infoportal/Content/api/2.9/Eplan.EplApi.DataModelu~Eplan.EplApi.DataModel.FunctionPropertyList~FUNC_TEXT.html); [API net-definition properties](https://www.eplan.help/en-us/Infoportal/Content/api/2026/Eplan.EplApi.DataModelu~Eplan.EplApi.DataModel.NetDefinitionPointPropertyList_members.html)). | The reference shows exactly these properties on symbols: DT "-101F1", manufacturer, part number "A9P52610", technical characteristics "10A C", and function text "TA05-V-15" on folio 203. | Must (map them for EPLAN import) |
| **Manual vs automatic function text** | `<20011>` is manual. `<20031>` is computed: the manual text if present, otherwise the **path function text** ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/gededitgui_k_pfadtextprinzip.htm)). | "Pneumatics" and "Spare" labels on folio 103 act as path function texts for the terminals below them. | Should |
| **Direct (in-place) editing** | Property texts on symbols can be edited in the canvas. Applies to functions, not to interruption points or structure boxes ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/inplaceeditinggui_k_eigenschaften.htm)). | Speed. | Must |
| **Property arrangements** | Saved layouts of which property texts appear where on a symbol. Configured under *Project data > Configure property arrangements*. Overwriting a user-defined arrangement updates every component that uses it ([2.9 news](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/news_p_eigenschaftsanordnungen_konfigurieren_205915.htm); [2026 dialog](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/devicetaggui_d_eigschanordnungspeichern.htm)). | Consistent look of device labels. The reference stacks DT, manufacturer, part number and rating on each device (folios 101, 102). | Must |
| **User-defined properties** | Freely named properties with a type, default/selection list and category, defined in the *Configure properties* dialog ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/userdefproperties_k_start.htm)). | Company-specific data such as Festo "order code" or a customer tag. | Must |
| **Block properties / calculations** | User-defined "block property" with a **calculation formula** (units ignored, numeric result) ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/blockproperties_k_berechnung.htm)). | Derived values: totals, power sums, display strings. | Should |
| **Multilingual properties** | Text properties can hold several languages (see translation, §11). | Export to non-Dutch customers. | Must in the data model; UI Should |

### 2.4 Project structure (IEC 81346 reference designation aspects)

EPLAN calls the structure segments **identifier blocks**. Each block has a prefix, and blocks can be freely combined;
the combination is chosen when the project is created ([help: page structure basics](https://eplan.help/en-us/Infoportal/Content/Plattform/2.8/Content/htm/pleditorgui_k_grundlagen.htm)):

| Identifier block | Prefix | IEC 81346 meaning | Used in reference? | Prio |
|---|---|---|---|---|
| Functional assignment | `==` (also `#` or `≠`) | function (higher level) | no | Should |
| **Higher-level function** | `=` | function aspect | **=P01** on every page | Must |
| **Installation site** | `++` | location (plant/building) | no (J-E-206 is another cabinet, which could be `++`) | Should |
| **Mounting location** | `+` | location aspect | **+J-E-2416**, **+JE2416** (inconsistent: defect 3) | Must |
| **Document type** | `&` (or space). Used for KKS power-plant structures and **excluded from device numbering** ([glossary](https://eplan.help/en-us/Infoportal/Content/Plattform/2.8/Content/htm/Glossary_o_dokumentenart.htm)) | document kind (IEC 61355) | sections 100/200/900 behave like document types | Should |
| **User-defined** | arbitrary, default `#` ([glossary](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/Glossary_o_benutzerdefiniert.htm)) | — | no | Could |
| Product aspect | `-` | product aspect | -101F1 | Must |

Supporting functions:

- **Extended project structures** dialog: sets the preceding signs and the separators for sub-identifiers. The default
  EN 81346 mapping is `=` higher-level function, `+` mounting location, `-` product
  ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/prjmanagementgui_r_darstellungerwprjstrukturen.htm)).
  **Must.**
- **Structure identifier management** (*Project data > Structure identifier management*): a central table of all
  identifiers with descriptions. Its order defines the sort order; identifiers are not sorted alphanumerically
  ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/pleditorgui_k_start.htm)).
  This is what prevents "+J-E-2416" vs "+JE2416" (defect 3): identifiers are picked from a list, not typed.
  **Must.**
- **Structure identifier overview report**: a list of the identifiers with their descriptions, one of the overview
  reports ([help: report types](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/formgeneratorgui_k_auswertungstypen.htm)). **Should.**

### 2.5 Pages: types, properties, numbering

**Page types.** Interactive pages are marked (I) and automatic report pages (A). Autoconnecting is active on logic
pages except "Panel layout", "Overview" and "Pre-planning"
([help 2026: page types](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/pagebrowsergui_k_seitentypen.htm);
[2022](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/pagebrowsergui_k_seitentypen.htm)).

| Page type | Purpose | Ref. folios | Prio |
|---|---|---|---|
| Schematic multi-line (I) | All-pole circuit diagram with logic and autoconnect | 101, 102, 103, 106 | Must |
| Schematic single-line (I) | Single-line overview (plant overview, power distribution) | (3 could be single-line) | Should |
| Overview (I) | Overview representation of devices (e.g. a PLC card or manifold with all connection points). No autoconnect. Cross-references to the main function. | 201, 202 (manifold overview); 3 (bus overview) | Must |
| Graphic (I) | Graphics without logic | 100, 200, 900 (section separators); 1 (colour table) | Must |
| Panel layout (I) | 2D mounting layout of parts | 902 | Must |
| Fluid power schematic (I) | ISO 1219 hydraulics and pneumatics. Rotating or flipping to change standard is not possible ([help Fluid page types](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/ftechnic_k_seitentyp_fluid.htm)) | 203–206 | Must (pneumatics is in the reference) |
| P&I diagram (I) | Process and instrumentation diagrams (Preplanning) | — | Later |
| Pre-planning (I) | Graphical preplanning without logic | — | Later |
| Topology (I) | Routing-path networks for cable routing ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/cablinggui_k_vorgehensweise.htm)) | — | Could |
| Model view (I) | 2D views generated from a 3D layout space, editable (Pro Panel) ([handbook](https://www.slideshare.net/slideshow/eplan-p8-handbook-emerson-eduardo-rodrigues/271355948)) | 902 (would become a model view) | Must (with 3D v1) |
| External document (I) | Embeds or links an external file (unverified) | — | Could |
| Title page / cover sheet (A) and every report type (A) | Generated from forms (§7) | 1, 2, 901 | Must |

**Page properties.** The most important are **Page name** (full name including separators and subpage identifiers,
without structure identifiers), **page type**, and **page description**, which is multilingual and may contain hard
returns. The page also carries structure identifiers, plot frame, scale and grid
([help 2026: page properties](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/pagebrowsergui_k_eigenschaften.htm)).
Page properties live in the 11000 ID range (API `Properties.Page`, e.g. `PAGE_FORMULAR` = 11015)
([API 2.8](http://eplanhelpwe.azurewebsites.net/en-US/infoportal/content/api/2.8/Eplan.EplApi.DataModelu~Eplan.EplApi.DataModel.Properties+Page.html)).
Page supplementary fields are `<11901>`.

| Feature | How EPLAN does it | Ref. folios / defect | Prio |
|---|---|---|---|
| **Full page name** `=P01+J-E-2416/101` | Structure identifiers plus page name; subpages (e.g. 101.1) (unverified syntax details) | every page | Must |
| **Sparse, section-based page numbers** | Free page names. A *Number pages* function renumbers, can keep gaps, and assigns numbers per structure (unverified details) | 1, 2, 3, 100–199, 200–299, 900–999 | Must |
| **Automatic page description** | "Format for automatic page description", e.g. report pages named after their content ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/formeditorgui_d_formatfuerautomatseitenbeschreibung.htm)) | 901 "Partlist 1", 2 | Should |
| **Prev / Next page in title block** | Plot-frame special texts resolve the previous and next page automatically (unverified exact property names) | defect 7 (902 "Next" empty) | Must |
| **Page navigator** | Tree by structure or list view, sort by page type, drag to reorder, filter ([help: project page view](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/pagebrowsergui_k_prinzip.htm)) | — | Must |
| **Plot frame per page** | Default per project, overridable per page | every page | Must |
| **Grid, paths, columns and rows** | The plot frame defines path (column) and row division used for cross-references | 0..16 × A..H grid | Must |

---

## 3. Schematic object model

### 3.1 Symbols, symbol libraries, function definitions

| Feature | How EPLAN implements it | Why | Prio |
|---|---|---|---|
| **Symbol libraries (`*.slk`)** | A symbol is identified by library, name and number plus a variant (e.g. `new.test:1;A`). Symbol numbers run from 0 to 32 000 ([help: symbol editor](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/symboleditorgui_d_symboleigenschaften.htm)). | Reusable IEC 60617 symbols. | Must |
| **Symbol variants A–H** | Up to 8 graphical variants (rotations and mirrors) per symbol plus a contact-image variant and a connection-point variant. Variants have the same connection points in a different arrangement. Created from a source variant by rotate or mirror ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/symboleditorgui_h_mitsymbolvariantenarbeiten.htm)). | Placing symbols in any orientation while keeping logic. | Must |
| **Function definitions** | A symbol's logic lives in its **function definition** (category / group / definition). It defines the connection-point logic, i.e. which points are connected internally (target tracking). Changing the function definition replaces the symbol's connection-point data ([help](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/symboleditorgui_d_symboleigenschaften.htm)). | This separates "what it is" (e.g. NO contact, coil, MCB 1P+N, PSU) from "how it looks". Reports and checks depend on it. | Must |
| **Representation types** | Multi-line, Single-line, P&I diagram, Pair cross-reference, Overview, External, Graphic, Panel layout, Detailed panel layout. Enumerated in the API as `SYMB_SYBMOLFUNCTIONTYPE` ([API 2022](https://eplan.help/en-us/Infoportal/Content/api/2022/Eplan.EplApi.DataModelu~Eplan.EplApi.DataModel.MasterData.SymbolPropertyList~SYMB_SYBMOLFUNCTIONTYPE.html)). | The same device in several representations (folio 106 detail vs 201 overview). | Must |
| **Symbol editor** | Choose variant → symbol properties → assign function definition → draw graphics → place connection points ([help 2025](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/symboleditorgui_h_symboleanlegen.htm)). | Users need custom symbols (Festo CPX, MICO). | Must |
| **Black-box alternative** | Instead of a symbol, a device is drawn as a black box with device connection points (§3.8). | Fast drawing of devices that have no library symbol. | Must |

### 3.2 Functions vs devices vs parts; main and auxiliary functions

- **Function**: one placed or unplaced logical element, such as a coil, one contact, an MCB pole set, a PSU, a PLC
  connection point or a terminal. It has a DT, a function definition, connection points and properties.
- **Device**: all functions that share the same DT, e.g. `-102F2` with its 4 outputs and the 13-14 signal contact.
- **Part**: a catalogue article (manufacturer, part number, technical data) assigned to a function, normally the
  **main function**. Parts can carry a **function template**, the list of functions the part provides. Placing the
  part creates the device's functions as unplaced functions; the API can generate function templates from part macros
  ([API: MDPartsDatabase](https://www.eplan.help/en-US/Infoportal/content/api/2027/Eplan.EplApi.MasterDatau~Eplan.EplApi.MasterData.MDPartsDatabase.html)).
- **Main / auxiliary functions**: the main function carries the part data and refers to all auxiliary functions; each
  auxiliary function refers back to the main function
  ([help: cross-reference types](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/xessettingsgui_k_querverweisarten.htm)).
- **Pair cross-reference**: the same contact drawn twice, e.g. a motor protection switch drawn once in the overview and
  once wired. Conditions: both on multi-line pages, same function definition, DT and connection-point designations; the
  main-function contact has representation type "Pair cross-reference" and the wired one is an auxiliary function
  ([help 2025](https://eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/xessettingsgui_h_paarquerverweiseerzeugen.htm)).
- **Device navigator and unplaced functions**: functions can exist unplaced, created from parts or a device list, and
  be dragged onto pages later (unverified UI details). Checks report unplaced or duplicate functions.

| Feature | Ref. folios | Prio |
|---|---|---|
| Function / device / part triad, with parts on the main function | every device; BOM 901 is a parts report | **Must** |
| Main/aux functions and overview representation, e.g. "-201U1 (201-E1)" on 106 and "-201U3 (201-E4)" on 203 | 106, 201–206 | **Must** |
| Function templates from parts (placing a Festo VMPA1 sub-base yields 4 valve slots) | 201–206 | **Should** |
| Pair cross-reference | — | Could |
| Unplaced functions and a device-list workflow (plan the device list first, then draw) | — | Should |

### 3.3 Device tags (DT) and numbering

| Feature | How EPLAN implements it | Ref. / defect | Prio |
|---|---|---|---|
| **Full DT vs displayed DT** | The full DT includes all structure identifiers. The displayed DT `<20010>` omits identifiers that equal the page's identifiers ([help: Full DT](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/devicetaggui_d_vollstaendigesbmk.htm); [Displayed DT](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/devicetaggui_r_feldsichtbaresbmk.htm)). | "+JE2416-X1031" shown in full on 103 vs "-101F1" with location omitted: in EPLAN this is computed, not typed. Fixes defect 3. | **Must** |
| **Identifier rules** | Letter codes (identifier), prefix `<20012>`, counter, sub-counter | IEC 81346-2 classes F, G, S, T, E, X, W, U, Q | **Must** |
| **Online numbering** | Assigns a DT automatically to every new function or device from a format; increments the counter ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/numerationgui_h_online.htm)) | — | **Must** |
| **Offline numbering (renumber)** | Separate formats. Elements: identifier, page, column, row, counter, separator; an identifier is always required. Counters can run project-wide or restart per page, by row or by column ([help: format](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/numerationgui_k_format.htm); [offline formats](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/offlinenumerationgui_d_nummerierungsformate.htm)) | The reference uses page + identifier + counter (-101F1, -102G1). **Renumbering must update every reference: defect 1** (-3W1 / -4W1). | **Must** |
| **Format violation check** | Message P017008 when a DT does not match the online format (e.g. "-FA" under "identifier + counter") ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/messages_p_017008.htm)) | Defect 3 (S51, T55 without page prefix) | **Must** |
| **DT adoption** | Terminals and pins adopt the DT from black boxes; functions inside boxes adopt the box DT ([help: device connection points](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/ftechnic_k_geraetekaesten_anschluesse.htm)) | -102F2 MICO outputs | **Must** |
| **Tag management (2027)** | Overview of user-defined tags and their devices ([press](https://www.eplan.com/us-en/about-us/press/now-is-best-to-test-the-eplan-platform-2027/)) | — | Could |

### 3.4 Connections, autoconnect, connection definition points, wire numbering

| Feature | How EPLAN implements it | Why / ref. | Prio |
|---|---|---|---|
| **Autoconnecting** | A connection line is drawn automatically between connection points that face each other horizontally or vertically. Active on schematic pages only ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/connectionbrowsergui_k_prinzipverbindungen.htm)). | Core drawing productivity. Every schematic folio relies on it. | **Must** |
| **Connection symbols** | Angles (corners), **T-nodes** (4 directions, 3 points; points "1"/"2" give line order), double junctions, jumpers, interruption points. T-nodes, jumpers and interruption points need a target-tracking direction. 2026: junctions shown as plain dots or with target specification ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/egedgui_k_grundlagen.htm)). | Defines **which point connects to which**, i.e. the wire list. QET's plain conductors lack this. | **Must** |
| **Connection definition points (CDP)** | Objects placed on a connection that carry its properties: designation (wire number), cross-section, colour, length and so on. Cables use CDPs at each intersection with the cable definition line ([help: cables](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/cablegui_k_arbeitsweise.htm)). | Wire markings "2.5 BK", "1.5 DBU" on 101/102 are effectively CDPs. | **Must** |
| **Connection-property inheritance** | Properties flow from **connection points → potential → signal → net → connection → connection definition point**, and higher-priority levels win. The potential definition carries cross-section and colour from a *Connection colours* list, with multilingual colour names mapped to codes ([help: connection properties](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/connectionbrowsergui_k_verbindungseigenschaften.htm)). | **Directly replaces folio 1's "if not indicated, standard size is to be used" rule (defect 8).** Define 24 VDC = DBU 0.75 mm² once and every wire on that potential inherits it. | **Must** |
| **Connection numbering (wire numbers)** | Offline numbering from the generated connection data, after placing CDPs. Two allocation styles: **potential-oriented** and **connection-oriented**. **Format groups** with a scope of individual connections, nets, signals or potentials. Placeholders for connected device properties and signal/potential properties, with counter and sub-counter. **Net extension** across terminals and pins ([help: principle](https://www.eplan.help/en-us/infoportal/content/Plattform/2023/Content/htm/wirenumberinggui_k_prinzip.htm); [net extension](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/wirenumberinggui_h_netzerweiterung.htm)). Unnumbered CDPs show placeholder "?" text (unverified). | Defect 9: there are no wire numbers in the reference. Wire numbers drive ferrule labels and Komax/Rittal output (v1 decision 2). | **Must** |
| **Connection navigator** | Lists all generated connections with source, target and properties; editable in a table (unverified details) | Wire list editing | **Must** |
| **Connection groups** | Predefined groups such as general, PLC and terminal connections, used for numbering and filtering ([glossary](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/Glossary_o_verbindungsgruppen.htm)) | — | Should |
| **Wire colour codes** | Colour table with IEC 60757 codes (BK, BN, RD, OG, YE, GN, BU, VT, GY, WH, PK, GNYE …) | Folio 1 uses **DBU**, which is not in IEC 60757, so the table must be user-extensible | **Must** |

### 3.5 Potentials, signals, nets

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Potential definition point / potential connection point** | Placed via *Insert*. They define for the whole potential or signal: **potential name, signal name, potential type, potential value, frequency, possible counter-potentials**. The definition point has priority on conflict ([help: definition of potentials](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/potentialbrowsergui_k_prinzippotenziale.htm); [drawing PDPs](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/potentialbrowsergui_h_pdpzeichnen.htm)). | 24VDC-1…4, 0VDC, PE-1.x, L-1, N-1 on 101–106 | **Must** |
| **Potential types** | Types include L (phases), N, PE, + / −, M, FE and undefined (exact enum unverified; [course material](https://www.udemy.com/course/mastering-eplan-course-on-electrical-engineering-design/) mentions L1–L3, 24 V DC, neutral, PE, machine potential, functional ground, undefined). | Folio 1 colour table is "voltage → function → colour" | **Must** |
| **Potentials navigator and highlighting** | Lists placed definition points per potential. Potentials, signals and nets can be highlighted in colour ([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/potentialbrowsergui_d_potenziale.htm)). | Checking 24 VDC distribution across 102 → 103 → 106 | **Must** |
| **Signal / potential conflict check** | P005019 "Conflict between signal and potential" ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2023/Content/htm/messages_p_005019.htm)). | Catches 0 V and 24 V shorted by mistake | **Must** |
| **Potential overview report** | Overview report of all potentials ([help: report types 2.5](https://secops-eplanhelp-prod0-westeurope-wa.azurewebsites.net/help/platform/2.5/en-US/help/Content/htm/formgeneratorgui_k_auswertungstypen.htm)) | Could **generate folio 1's colour table** | **Should** |

### 3.6 Interruption points

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Interruption point** | Off-page connector. Points with the **same full DT** belong together and cross-reference each other ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xessettingsgui_h_abbruchstellenquerverweiseerzeugen.htm)). | L-1 → "102-A1", N-1, PE-1.1 → "103-G6", 24VDC-1 → "103-A1" … on 101–106 | **Must** |
| **Star vs chain cross-references** | **Star**: one point has the property *Star source* and all others refer to it. **Chain**: pairwise, 1→2, 3→4 …, ordered via the *Sort interruption points* dialog or sort codes. | 103 fans 24VDC-1 out to AIR1..AIR4 | **Must** |
| **Matrix display of cross-references** | Project setting for how many targets per row and in total ([help 2026: settings](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/xessettingsgui_d_einstellungenprojektabbruchstellen.htm); [representation](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/interruptionpointgui_k_darstellungabbruchstellen.htm)). | — | Should |
| **Missing counterpart check** | A point without a counterpart is an error in message management ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xessettingsgui_h_abbruchstellenquerverweiseerzeugen.htm)). | The reference has hand-maintained targets that drift, e.g. PE-1.1 "101-G15" on 103 vs the symbol at G16 on 101. | **Must** |
| **Interruption point navigator** | Lists interruption points and their partners (unverified name). | — | Should |

### 3.7 Cross-references and contact images

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Cross-reference types** | Main ↔ auxiliary function, interruption points, pair cross-references, **multi-line ↔ single-line / overview** (devices with the same DT on different representation pages cross-reference each other) ([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/xessettingsgui_k_querverweisarten.htm); [symbol cross-refs](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/xessettingsgui_h_symbolquerverweiseerzeugen.htm)). Also a PLC connection-point pair between schematic and overview page. | "(201-E1)", "∞ 203-B4" on 106/201–206 | **Must** |
| **Contact image (Kontaktspiegel)** | On a coil or main function: a list or graphic of all contacts with their locations. Placement **"In path"** (under the coil in its column) or **"On component"**. Without an alignment only a cross-reference list is shown ([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/xessettingsgui_k_querverweisarten.htm)). The contact-image graphic comes from the symbol's contact-image variant. | No relays in this set, but standard in every relay panel | **Must** |
| **Configurable cross-reference format** | Project settings *Cross-references / contact image*: order and separators of structure identifiers, page, column, row; whether to show identifiers only when they differ (unverified option names). Typical EPLAN default is `/page.column`, e.g. `/12.4` (unverified). | The reference uses **`<page>-<row><column>`**, e.g. "102-A1", "101-H16", so the format must be configurable | **Must** |
| **Cross-reference property** | `FUNC_CROSSREFERENCE` property in the API ([API](https://eplan.help/en-us/Infoportal/Content/api/2.9/Eplan.EplApi.DataModelu~Eplan.EplApi.DataModel.FunctionPropertyList~FUNC_CROSSREFERENCE.html)) | — | Must |
| **Cross-references in PDF are hyperlinks** | §13 | — | Must |

### 3.8 Boxes: black boxes, device connection points, structure and location boxes

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Black box** | Rectangle or polygon that turns its content into a device. Used for devices missing from the mandated symbol library, PLC assemblies, complex devices such as VFDs, and devices whose internals are unknown or confidential ([help: uses](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/blackbox_k_einsatzmoeglichkeiten.htm); [help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/blackbox_k_start.htm)). | -102G1 PSU, -102F2 MICO, -201U1/-202U1 Festo CPX (106) | **Must** |
| **Device connection point** | Connection point inside a black box. Two-sided variants have one designation, an internal and an external side. Device connection points without their own DT inside a box are **auxiliary functions** of the box ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/ftechnic_k_geraetekaesten_anschluesse.htm); [inserting](https://eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/blackbox_h_anschluesseeinfuegen.htm)). | PSU L/N/PE/+/+/−/−; CPX pins 1..5 + FE | **Must** |
| **Structure box** | Assigns structure identifiers (=, +) to everything inside it, e.g. a part of a page belonging to another location (unverified detail) | dashed "External Connection (J-E-206 / JE-N1000-03)" box on 101 and 3 | **Must** |
| **Location box** | A box representing a mounting location; can carry parts and appears in reports (unverified detail) | as above; "PART DATA" boxes on 201/202/902 could be location boxes plus embedded reports | Should |

### 3.9 Terminal strips

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Terminals as functions** | Each terminal is a function with designation `<20030>`, connection points, part and level. A strip is defined by its DT. | 1X1 (1, 2, PE), +JE2416-X1031 (1+, 2+ … 1-, 2-), X1032 (PE) on 101/103 | **Must** |
| **Terminal strip definition** | Function that carries strip-level data (strip part, accessories, settings); created automatically when terminals are placed (unverified detail) | — | **Must** |
| **Edit terminal strip dialog (strip editor)** | Table of all terminals with sort, number, add or remove **alignable accessories** (end plates, brackets), **generate multi-level terminals**, and a preview with levels, connection counts, saddle-jumper options and potential-type colours. Settings are stored in a scheme ([help 2025](https://www.eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/stripmanagementgui_d_klemmenleistebearbeiten.htm)). | — | **Must** |
| **Multi-level terminals** | Level > 0 marks a level of a multi-level terminal. Terminals form one when they share a DT, are consecutive in sort order, and have rising or falling levels. The highest level becomes the **main terminal**, others auxiliary; requires "Do not use main terminals" off ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/terminalgui_k_mehrstockklemmen.htm)). | Phoenix **PT 2.5-TWIN** (multi-connection) and **UK5N** in BOM 901 | **Must** |
| **Jumpers / bridges** | **Saddle jumpers** (internal or external, manual or automatic); jumpers between levels are stored as `1/-1` on the first jumpered terminal ([TechTip: manual saddle jumpers](https://www.eplan.help/techtipps/en-us/Klemmen/TechTip-Behaviour-of-manual-saddle-jumpers.pdf); [help: jumpers](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/terminalgui_k_verwendungbruecken.htm)). Wire jumpers and plug-in bridges also exist (unverified naming). | the four "1+" terminals on 103 are commonly bridged | **Must** |
| **Terminals with variable structure** | Parts whose connection-point layout varies ([TechTip](https://www.eplan.help/techtipps/en-US/Klemmen/TechTip-Terminals-with-variable-structure.pdf)) | — | Should |
| **Spare terminals, sorting, designation generation** | Spare marking, numeric or alphanumeric sorting, designate in sequence (unverified details) | "Spare" on 103 | **Must** |
| **Terminal checks (class 001)** | e.g. P001001 terminal without designation, **P001005 designation used more than once** ([help](https://eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/messages_o_001Terminals.htm)) | four "1+" on X1031 would trigger P001005 unless they are levels or connection points of one terminal | **Must** |
| **Terminal reports** | Terminal diagram `.f13`, terminal-strip overview `.f14`, terminal line-up diagram `.f12`, terminal-connection diagram (§7) | defect 9 | **Must** |
| **clipx ENGINEER integration (2025)** | Phoenix Contact terminal-strip configurator linked to EPLAN ([press](https://www.eplan.com/de-en/about-us/press/new-software-with-smart-features-for-easier-and-faster-engineering/)) | — | Later |

### 3.10 Cables and shields

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Cable definition line** | Drawn across the connections that belong to a cable. Those connections get function definition "Conductor / wire" and adopt the cable DT. CDPs are placed at each intersection ([help](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/cablegui_k_arbeitsweise.htm)). | -106W1, -106W2 (106); -3W1 (3) | **Must** |
| **Conductor assignment** | Manually in the *Edit cable* dialog or automatically: conductors (colour or number) are mapped to connections | -106W1 BK, BU, GNYE, WH, BR | **Must** |
| **Shields** | Shield symbol on a cable. Unnamed shields cannot be told apart; the organisation is detected after the first assignment in *Edit cable*. Shields defined on the cable part are generated on insertion ([help 2023](https://eplan.help/en-us/infoportal/content/Plattform/2023/Content/htm/cablegui_k_verhaltenabschirmungen.htm)). | Profibus cable 6XV1830 (3) is shielded | **Must** |
| **Automatic cable generation and selection** | Connections with "Cable connection = Cable" get cables automatically; cable numbering and automatic part selection run separately ([help](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/cablegui_k_automatikkabel.htm)) | — | Should |
| **Cable navigator** | Edit cables in the navigator or graphically | — | **Must** |
| **Cable checks (class 003)** | e.g. P003011 too many cable connections, **P003022 invalid cable length**, P003027 function text difference ([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/messages_o_003Cables.htm)) | -106W2 missing from BOM (defect 4) | **Must** |
| **Cable reports** | Cable diagram `.f09`, cable overview `.f10`, cable connection diagram `.f07`, cable assignment diagram `.f08` ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/cablegui_h_kabeldarstellungsplan.htm)) | defect 9 | **Must** |
| **Cable dependencies (2025)** and pre-assembled cables / Cable proD | §1.2 | — | Later |

### 3.11 Plugs

Plugs and sockets with pins are handled like terminals: functions with a plug definition, a plug navigator, and plug
diagram and plug overview reports (unverified details). The Festo M12 cable connectors on folio 106 and the DP
connector -3X1 on folio 3 are plug-like devices. **Should** (the Profibus connector is modelled as a device today).

### 3.12 PLC and bus systems

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **PLC box, PLC card, PLC connection points** | PLC box (device), cards, and connection-point types such as digital/analog input/output, multi-function points, **card power supply**, **bus port**. Connection-point designations must be unique within a PLC box; descriptions may repeat at card power supplies ([TechTip: PLC properties](https://www.eplan.help/techtipps/en-US/SPS/TechTip-Overview-of-the-PLC-properties.pdf); [help: PLC connection points](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/plcgui_k_spsanschluesse.htm)). | Festo CPX (201U1) = bus node; valve outputs Q0.1 … Q6.7 on 203–206 | **Must** (v1 decision 1) |
| **PLC properties** | PLC address, symbolic address, data type, channel number, function text, CPU name, start address and so on ([TechTip](https://www.eplan.help/techtipps/en-US/SPS/TechTip-Overview-of-the-PLC-properties.pdf)) | "Q0.1 / TA05-V-15" on 203 | **Must** |
| **PLC navigator** | *Project data > PLC > Navigator*. Creates PLC boxes and points, including unplaced ones ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/plcgui_h_spsanlegen.htm)) | — | **Must** |
| **Addressing / readdressing** | *Project data > PLC > Address* on selected points, or from the navigator ([2.8 news](https://eplan.help/en-us/infoportal/content/Plattform/2.8/Content/htm/news_p_sps_weiteres_195263.htm)) | sequential Q0.1, Q0.3 … | **Must** |
| **PLC overview pages** | PLC card shown in overview representation with all channels, cross-referenced to the schematic channels | — | **Must** |
| **Bus ports and bus configuration transfer** | Bus ports on PLC boxes; configuration data can be copied to the matching bus ports ([2.8 news](https://eplan.help/en-us/infoportal/content/Plattform/2.8/Content/htm/news_p_sps_weiteres_195263.htm)) | Profibus topology (3) | **Must** |
| **PLC data exchange: AutomationML AR APC** | Export and import in **AutomationML AR APC V1.0.0–V1.4.0**; assignment list = "TagTable". GUIDs are assigned per object; export requires an editable project. Import from TIA assigns points by channel number ([TechTip PLC data exchange, EPLAN 2027, Apr 2026](https://www.eplan.help/techtipps/en-us/SPS/TechTip-PLC-data-exchange.pdf); [help AR APC](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2023/Content/htm/plcgui_k_amlbusdatenaustausch.htm)). | Exchange with TIA Portal (v1 decision 1) | **Must** |
| **Vendor converters** | `PlcDcExchangerSiemensTIA17AML` (TIA Portal V17, AR APC 1.2.0), `PlcDcExchangerSiemensTSTAML` (TIA Selection Tool, AR APC 1.1.0), `PlcDcAMLExchangerGeneral` (PLC standard exchange format), plus XML converters for ABB, B&R, Bosch Rexroth and Schneider (`PlcDcXMLExchangerSchneider`). Address overview / assignment list exported as **CSV** (`EXPORTADDRESSOVERVIEW`) ([help: action plcservice](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2024/Content/htm/availableactions_o_plcservice.htm); [Import PLC data dialog 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/plcgui_d_importbuskonfig.htm)). Rockwell Studio 5000 ↔ EPLAN via AML ([Rockwell](https://support.rockwellautomation.com/cc/okcsFattachCustom/get/1086730_4)). | Codesys has no verified EPLAN converter: an open question for ElektraWatt | **Must** (AML + CSV); others Should |
| **TIA Selection Tool exchange** | Requires a valid PLC address, data type and CPU name ([TechTip](https://www.eplan.help/techtipps/en-us/SPS/TechTip-PLC-data-exchange-with-SIEMENS-TIA-Selection-Tool.pdf)) | — | Could |
| **PLC reports** | PLC diagram, PLC card overview, assignment list / address overview (§7) | defect 9 | **Must** |
| **Generate schematic from PLC data** | API `PlcService.GenerateSchematic` ([API](https://eplan.help/en-us/infoportal/content/api/2.9/Eplan.EplApi.HEServicesu~Eplan.EplApi.HEServices.PlcService~GenerateSchematic.html)) | 203–206 are highly repetitive and could be generated | Should |

### 3.13 Texts, graphics, images, comments, layers

| Feature | Notes | Ref. | Prio |
|---|---|---|---|
| Free texts, path function texts, property texts | Path function texts are reported as the function text of devices in the same path ([help 2022](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/gededitgui_k_pfadtextauswertung.htm)) | "Pneumatics", "Spare" (103) | Must |
| Graphics: lines, rectangles, arcs, dimensions, images | Graphic editor elements | 1, 902 (duct "40x60" labels) | Must |
| **Linked images and QR codes on symbols (2025)** | Images follow symbol changes ([press](https://www.eplan.com/de-en/about-us/press/new-software-with-smart-features-for-easier-and-faster-engineering/)) | — | Could |
| **Comments navigator and PDF comment import** | Comments with status and history (2025); importing PDF comments back into the project (unverified) | review loop with customer | Should |
| **Layer management** | Standard layers `EPLANxxx` (descriptions fixed) plus user layers; also used for DXF layer mapping ([help DXF layers](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/xdxfgui_r_ebenen.htm)) | — | Must |

---

## 4. Navigators

EPLAN's navigators are dockable tree or list views over the project database, with filters, drag-to-place, multi-edit
and jump-to-graphic. Names verified where cited; the rest from knowledge (unverified).

| Navigator | Shows / does | Prio |
|---|---|---|
| Page navigator | Pages by structure, sort by type ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/pagebrowsergui_k_prinzip.htm)) | Must |
| Device navigator | Devices → functions (placed and unplaced), parts, drag to place | Must |
| Connection navigator | Generated connections, properties table | Must |
| Terminal-strip navigator | Strips → terminals; opens *Edit terminal strip* | Must |
| Cable navigator | Cables → conductors and shields | Must |
| Plug navigator | Plugs → pins (unverified) | Should |
| PLC navigator | PLC boxes → cards → points; addressing ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/plcgui_h_spsanlegen.htm)) | Must |
| Potentials navigator | Potentials → definition points ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/potentialbrowsergui_d_potenziale.htm)) | Must |
| Interruption point navigator | Interruption points and partners (unverified) | Should |
| Structure identifier management | Central identifier table ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/pleditorgui_k_start.htm)) | Must |
| Message management | Check results ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/msgmanagementgui_k_start.htm)) | Must |
| Macro navigator, placeholder-object navigator | Macro projects and value sets (unverified names) | Should |
| Topology navigator | Routing paths, points, targets ([help](https://eplan.help/en-us/infoportal/content/Plattform/2.8/Content/htm/cablinggui_d_navigator.htm)) | Could |
| Pre-planning navigator | Structure segments, planning objects, PCT loops ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/planninggui_d_navigator.htm)) | Later |
| 3D mounting-layout navigator, Wiring Manager (Pro Panel) | Placements and routed connections per layout space | Must (3D v1) |
| Comments navigator | Comments with status (2025) | Should |
| Bill-of-materials / parts navigator | Parts per device, bulk update ([course listing](https://www.udemy.com/course/mastering-eplan-course-on-electrical-engineering-design/)) | Should |

---

## 5. Macros and reuse

| Feature | How EPLAN implements it | Why / ref. | Prio |
|---|---|---|---|
| **Window macro `*.ema`** | Saved selection of objects from a page | e.g. a "PSU + MCB + MICO" block (102) | **Must** |
| **Symbol macro `*.ems`** | Like a window macro, but selectable like a symbol | — | Should |
| **Page macro `*.emp`** | One or more whole pages; inserting creates pages ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/macrosgui_h_makroserzeugen.htm); [API HE_PlaceMacros 2027](https://www.eplan.help/en-US/Infoportal/content/api/2027/HE_PlaceMacros.html)) | Standard pages, e.g. "Valve manifold" 203 | **Must** |
| **Variants per representation type** | A macro can hold several variants per representation type. Angle variants are not created automatically; store them under the same name with another variant designation. Variant range reported as A–P (unverified; one third-party listing claims 26 variants). | multi-line / overview / panel-layout / 3D representation of the same device | **Must** (variants per representation) |
| **Macro boxes and macro projects** | Macro boxes in a macro project define name, type and usage ("Defining" → generated macros "Referencing", so they can be updated). *Project data > Macros > Generate automatically*, with optional "Prepare macros for automatic generation" and overwrite ([help](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/macrosgui_h_makrosausmakroprojekt.htm); [API GenerateMacrosFromMacroProject](https://www.eplan.help/en-US/Infoportal/content/api/2027/Eplan.EplApi.HEServicesu~Eplan.EplApi.HEServices.Masterdata~GenerateMacrosFromMacroProject(Project).html)). Inserting macro boxes depends on the project setting "Also insert macro boxes". | library maintenance at scale | Should |
| **Placeholder objects and value sets** | An "anchor" object in a macro. Variables are bound to properties of selected objects; **value sets** are rows of variable values (e.g. 4 kW / 7.5 kW motor variants). Selected on insertion through a value-set dialog; block editing needs equal variable and value-set names ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/macrosgui_h_platzhalterobjekteerzeugen.htm); [dialog](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2023/Content/htm/macrosgui_d_platzhalterobjekt.htm)). | 203–206: same valve block with different addresses and tags | **Should** (high value; v1.x) |
| **Macro preview (2025) / built-in preview (2027)** | Preview before insertion | — | Should |
| **Part macros** | Parts reference macros per representation (schematic, panel layout, 3D). Data per **part variant** allows different macros per variant ([2022 news](https://eplan.help/en-us/infoportal/content/Plattform/2022/Content/htm/news_p_artikelverwaltung_refactoring_270288.htm)) | 902 layout from part macros | **Must** |

---

## 6. Parts management, Data Portal, classification

| Feature | How EPLAN implements it | Why / ref. | Prio |
|---|---|---|---|
| **Parts database** | Since 2022 an internal format `*.alk` + `*.adb` directory replaced Access `*.mdb`, with SQL Server as an option. Data is stored per **part variant**. Databases from 2.9 or older must be migrated ([2022 news](https://eplan.help/en-us/infoportal/content/Plattform/2022/Content/htm/news_p_artikelverwaltung_refactoring_270288.htm); [2025](https://www.eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/news_p_artikelverwaltung_datenbankaktualisieren_449029.htm)). | Company article master (Schneider, Phoenix, Festo, Rittal, Murr). | **Must** |
| **Part fields** | Manufacturer, part number, ERP number, description, technical data, dimensions, macros, documents, images, product group, accessories ([help 2026: parts tab properties](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/articlesgui_r_eigenschaften.htm)) | BOM 901 columns | **Must** |
| **Accessory lists and accessory placements** | Parts can carry accessory lists and placements, exportable with the part ([API](https://www.eplan.help/en-US/Infoportal/content/api/2027/Eplan.EplApi.MasterDatau~Eplan.EplApi.MasterData.MDPartsDatabase.html)) | 201U1 = CPX-FB13 + CPX-M-GE-EV-S (accessory) | **Must** |
| **Part assemblies / sets** | A main part with sub-parts, reported in the summarized parts list ([help: report types](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/formgeneratorgui_k_auswertungstypen.htm)) | Festo order codes "51E-F13GCQP-D+H2GG" (201) = configured assembly | **Must** |
| **Function templates** | Functions a part provides; generated from macros ([API](https://www.eplan.help/en-US/Infoportal/content/api/2027/Eplan.EplApi.MasterDatau~Eplan.EplApi.MasterData.MDPartsDatabase.html)) | — | Should |
| **Device selection** | Choose a part filtered by function definition and connection data such as cross-section ([help: potential definition / cross-section filter](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/connectionbrowsergui_k_verbindungseigenschaften.htm)) | — | Should |
| **Import / export formats** | **EDZ** and XML are the main exchange formats. Import also supports **VDMA 66413** and **ECLASS** (BMEcat XML; 9.0 since 2.6, 10.0.1 since 2.9; deprecated ECLASS properties not offered) ([2022 news](https://eplan.help/en-us/infoportal/content/Plattform/2022/Content/htm/news_p_artikelverwaltung_refactoring_270288.htm); [ECLASS import](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/news_p_artikelverwaltung_eclassimport_235591.htm)). ETIM support was **not confirmed**. | Manufacturer data import (v1 decision 4: EDZ) | **Must** (EDZ + ECLASS/BMEcat); ETIM Should |
| **EDZ** | "EPLAN Data Portal exchange format". Imported via parts management *Extras > Import* ([API IXPartsImportExportEdz 2027](https://www.eplan.help/en-US/Infoportal/content/api/2027/IXPartsImportExportEdz.html); [FileInfo](https://fileinfo.com/extension/edz)). Can contain logic macros, 2D graphics, connection-point patterns, **drilling patterns**, 3D graphics, DXF ([Data Portal FAQ](https://www.eplan.com/en/support/data-portal.html)). Internal container: an archive of XML plus macro files (unverified, see open questions). | Migration and Data Portal content | **Must** |
| **Data Portal** | Web portal, integrated in the platform. **More than 2 million static device data sets from about 500 manufacturers**, and more than 4 million with configurators (2025) ([TimesTech](https://timestech.in/next-milestone-achieved-for-the-eplan-data-portal/)). June 2024: 492 manufacturers, more than 1 758 000 sets ([EPLAN](https://www.eplan-software.com/company/news/eplan-data-portal-update-01-june-2024)). EPLAN-format download needs a software service agreement and 2.9 SP1+ ([FAQ](https://www.eplan.com/en/support/data-portal.html)). From 2026 it is integrated in device management. | Fastest source of macros and 3D | Later (no open API; licensing, see open questions) |
| **EPLAN Data Standard (EDS)** | Defines which data manufacturers supply and how. Data Portal publishing requires EDS compliance. ECLASS Advanced 9.0+ archives can be imported by manufacturers ([FAQ](https://www.eplan.com/en/support/data-portal.html); [EDS doc 2021](https://ww3.cad.de/foren/ubb/upl/E/EPLAN+kurz+erklart/EPLAN_Data_Standard_Documentation_20211029_EN.pdf)) | Data-quality model to emulate | Should (adopt EDS-like quality levels) |
| **eStock** | Cloud device management with browser access and rights management. Switch between a local DB and an eStock collection (2024) ([eStock](https://eplan.com/us-en/products/eplan-estock.html); [2023 news](https://www.eplan.help/en-us/infoportal/content/Plattform/2023/Content/htm/news_p_artikelverwaltung_estock_370593.htm)) | — | Later |
| **Compare up to 10 components (2027)** | Side-by-side comparison filtered to differences ([product page](https://www.eplan.com/us-en/products/eplan-platform/)) | — | Could |

---

## 7. Reports

**Mechanics.** EPLAN reports are generated from forms. **Report types are predefined and cannot be user-created**. They
fall into three families ([help: report types](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/formgeneratorgui_k_auswertungstypen.htm)):

- **Function-related reports**: a device in the header, e.g. one page set per terminal strip; each occurs once per
  project ([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/formgeneratorgui_k_funktionsbezogen.htm)).
- **Overview reports**: cover many devices and may occur multiple times
  ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/formgeneratorgui_k_auswertungsuebersicht.htm)).
- **Graphical reports**, built from engineering data
  ([help 2025](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/formgeneratorgui_k_auswertunggrafisch.htm);
  [webcast 2026](https://www.eplan.com/my-en/about-us/events/the-power-of-graphical-reporting-in-eplan/)).

Reports are produced as **report pages** (automatic pages) or as **embedded reports** placed inside an interactive page
([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/formgeneratorgui_k_ausgabeformen.htm)).
**Report blocks / templates** store the full set of report settings so that *update reports* regenerates all of them
([help 2022](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2022/Content/htm/formgeneratorgui_k_auswertungsbloecke.htm)).
Filters, sorting and manual selection are configured per report ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/formgeneratorgui_d_filtererweitert.htm)).

| Report type | Form ext. | Content | Ref. folios / defect | Prio |
|---|---|---|---|---|
| Title page / cover sheet | `.f26` ([de 2.8](https://eplan.help/help/platform/2.8/de-de/help/content/htm/formgeneratorgui_k_auswertungsuebersicht.htm)) | Project properties | **1** | Must |
| Table of contents | `.f06` | Pages with description and structure identifiers | **2** (Folio / Title / Location +) | Must |
| Structure identifier overview | (unverified) | Identifiers and descriptions | — | Should |
| **Parts list** (BOM) | `.f01` | All parts per DT | **901** | Must |
| **Summarized parts list** | `.f02` | Identical parts consolidated, with quantities ([help 2022](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/formgeneratorgui_k_auswertungsuebersicht.htm)) | **defect 4**: X1031 listed twice, no quantity column | Must |
| Device tag list (device list) | `.f03` | All devices | — | Must |
| Assembly / module overview | (unverified) | Part assemblies | 201/202 order codes | Should |
| Manufacturer / supplier list | `.f31` | Manufacturers used | — | Could |
| Order list | (unverified) | Purchasing view | — | Should |
| **Terminal diagram** | `.f13` | Per strip: terminals, internal and external targets, jumpers ([2022 search extract](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/formgeneratorgui_k_funktionsbezogen.htm)) | defect 9 | Must |
| Terminal line-up diagram | `.f12` | Terminals in list form per strip ([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/formgeneratorgui_k_funktionsbezogen.htm)) | defect 9 | Must |
| Terminal-connection diagram | (unverified) | Graphical terminal and connection view | defect 9 | Should |
| **Terminal-strip overview** | `.f14` | All strips with counts | defect 9 | Must |
| **Cable diagram** | `.f09` | Conductors of each cable with targets | defect 9 | Must |
| Cable connection diagram | `.f07` | Cable connections across devices, sorted by cable | — | Should |
| Cable assignment diagram | `.f08` | Cable structure incl. plugs ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/cablegui_h_kabeldarstellungsplan.htm)) | — | Could |
| **Cable overview** | `.f10` | All cables, type, length | defect 9 | Must |
| Device connection diagram | `.f05` | Connection points per device | — | Should |
| **Connection list / wire list** | (unverified) | Every connection: source, target, cross-section, colour, length, wire number | defect 9; v1 manufacturing | Must |
| Plug diagram / plug overview | (unverified) | Plugs and pins | — | Should |
| **PLC diagram** | (unverified) | Per PLC card: channels and assignment | v1 PLC | Must |
| **PLC card overview** | (unverified) | All cards and their addressing ([search extract](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/formgeneratorgui_k_auswertungstypen.htm)) | v1 PLC | Must |
| PLC address overview / assignment list | CSV export also exists | Symbol table / tag table | v1 PLC | Must |
| Bus / network overview | (unverified) | Bus devices and addresses | 3 | Should |
| **Potential overview** | (unverified) | Potentials, types, values, colours | could generate the **1** colour table | Should |
| **Revision overview** | `.f17` | Revisions with dates and descriptions | **defect 5** | Must |
| Enclosure / cut-out legend | `.f18` | Legend of panel-layout placements | **902** | Must |
| Mounting list | `.f32` | Assembly instructions for placed parts ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/cabinetgui_k_auswertungmontageliste.htm)) | 902 | Should |
| PCT loop legend | `.f33` | Process loops (Preplanning) | — | Later |
| Symbol overview, forms documentation, plot-frame documentation | (unverified) | Master-data documentation | — | Could |
| Graphic reports (e.g. multi-level terminal graphics, nameplates, loop diagrams) | — | 2026 webcast examples | — | Could |

---

## 8. Forms and plot frames (title blocks)

| Feature | How EPLAN implements it | Ref. / defect | Prio |
|---|---|---|---|
| **Form editor / plot-frame editor** | Graphic editor for forms and plot frames (Master data tab in 2022+) ([help 2026: special texts](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/formeditorgui_h_sondertexteeinfuegen.htm); [basics](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/formeditorgui_k_grundlagen.htm)) | The pilot user title block | **Must** |
| **Special texts** (project, page, plot-frame properties) | *Insert > Text > Project properties / Page properties*; property chosen in the Placement tab. Only properties allowed for that form or plot-frame type are offered | "Customer: <end user>", "Project: <project ref>", "=P01", "+J-E-2416", "Page / Prev / Next", Creation and Saved dates. **Defects 5, 6, 7.** | **Must** |
| **Placeholder texts** in report forms | Replaced by the values of the reported objects, grouped into header, data area, footer and so on ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/formeditorgui_k_platzhalter.htm); [placeholder elements](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2024/Content/htm/placeholder_o_main.htm)) | 901 BOM columns | **Must** |
| **Form types and IDs** | Each form type has an ID that fixes the available placeholders ([glossary](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/Glossary_o_formulartypen.htm)) | — | Must |
| **Dynamic forms** | Rows grow with data; header areas are mainly for function-related reports ([help 2026](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2026/Content/htm/formeditorgui_k_dynamformulare.htm)) | — | Must |
| **Next forms** | A different form for continuation pages ([help 2024](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2024/Content/htm/formeditorgui_k_folgeformulare.htm)) | — | Should |
| **Hide placeholders while editing** | View toggle ([help 2022](https://www.eplan.help/en-us/infoportal/content/Plattform/2022/Content/htm/formeditorgui_h_platzhaltertexteanzeigen.htm)) | — | Could |
| **Community form libraries** | e.g. [STLMINC/EPLAN_FORMS](https://github.com/STLMINC/EPLAN_FORMS) with `.f13` multi-level terminal diagram using conditional sub-forms | shows what users customise | — |

---

## 9. Checks and message management

| Feature | How EPLAN implements it | Prio |
|---|---|---|
| **Message management** | A list of check results with jump-to-object and filter. Resolved messages stay until the next offline check ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/msgmanagementgui_k_start.htm); [dialog](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/msgmanagementgui_d_meldungsverwaltung.htm)) | **Must** |
| **Message numbers and classes** | 6 characters: the first 3 are the class, the last 3 the message, shown as `P001005` etc. Classes are topical, e.g. **001 Terminals**, **003 Cables**, **005 Connections**, **017 DT/numbering** ([glossary: message classes](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/Glossary_o_meldungsklassen.htm); [005 Connections](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/messages_o_005Connections.htm)). Category per message: error, warning or note. | **Must** |
| **Check types per message** | **Offline** (only during a project check; connections refreshed first), **Online/offline** (written as they occur), **Prevent errors** (the offending change is undone and page, DT and position reported), **Module-specific** (preset, e.g. during cable generation or PLC addressing), **No** ([help 2026: settings](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/xesinspectiongui_d_einstellungenmeldungen.htm); [check-run principle](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/msgmanagementgui_k_prueflaufprinzip.htm)) | **Must** |
| **Check schemes** | Named sets of check settings, e.g. "IEC full", "Pre-delivery" (unverified scheme names) | **Must** |
| **Copilot quality checks (2027)** | AI-assisted checks | Later |

Representative checks that would have caught the reference defects:

| Check | Message (EPLAN) | Defect prevented |
|---|---|---|
| DT does not match numbering format | P017008 | 3 (S51/T55/E55/1X1 vs page-based) |
| Terminal designation used more than once | P001005 | 103 duplicated "1+" |
| Too many connections at a point / unconnected connection point | P005025 / P005026 | wiring errors |
| Signal vs potential conflict | P005019 | 0 V / 24 V short |
| Invalid cable length | P003022 | manufacturing output |
| Interruption point without counterpart | (class unverified) | stale off-page refs |
| Part without part number, panel-layout placement without schematic function (ElektraWatt-specific) | — | 2 (T51 vs T55), 4 (E56/S56 no part data) |

---

## 10. Revision management

| Feature | How EPLAN implements it | Ref. / defect | Prio |
|---|---|---|---|
| **Complete project / complete revision** | Write-protects the project, optionally creating a completed copy (`*.elr`) and a revision project (`*.ell`). The API can complete a revision for the whole project, single pages or installation spaces ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/revisionmgtgui_h_projektabschliessen.htm); [API Revision 2026](https://www.eplan.help/en-us/Infoportal/Content/api/2026/Eplan.EplApi.HEServicesu~Eplan.EplApi.HEServices.Revision.html)) | **defect 5**: "1", "V1.0", "V1", "V2.0" | **Must** |
| **Property comparison** | Compares every property of a project with a comparison (reference) project; creates a comparison project | — | **Must** |
| **Change tracking and revision marks** | Adds revision markers to changed objects, plus markers for deleted objects in the writable comparison project. Restore markers via *File > Revision control > Property comparison* ([help 2023](https://eplan.help/en-us/infoportal/content/Plattform/2023/Content/htm/revisionmgtgui_h_revmarkierungenerzeugen.htm)). Revision ID per object `INSTANCE_REVISIONID` ([API](https://www.eplan.help/en-US/Infoportal/content/api/2025/Eplan.EplApi.DataModelu~Eplan.EplApi.DataModel.ProjectPropertyList~INSTANCE_REVISIONID.html)) | customers need "what changed in V2.0" | **Must** |
| **Page revision vs project revision** | Revision index per page and a project revision, shown in the title block (unverified naming) | 101/106 show V2.0, others V1.0 | **Must** |
| **Revision overview report** `.f17` | §7 | — | **Must** |
| Tier note | Revision management is a Pro-tier feature in 2026 ([ibKastl](https://ibkastl.de/blog/lizenzierung-eplan-ab-2026)) | — | — |

---

## 11. Translation and multilingual projects

| Feature | How EPLAN implements it | Prio |
|---|---|---|
| **Project languages** | A source language (the entry language) plus translation languages, set in the project settings ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/translategui_r_einstellungprojekt.htm)) | Must in the data model; Should in UI |
| **Dictionary** | One dictionary with keywords in several languages, for project and parts texts. Export untranslated words, translate externally and re-import. List of words not to translate. Untranslated text stays language-neutral ([help 2025: dictionary](https://www.eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/translatedbgui_k_start.htm)) | Should |
| **Translation method** | Word-, sentence- or entire-entry based, with a number-handling option ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/translategui_r_einstellungprojekt.htm); method names unverified) | Should |
| **Display languages** | Which languages are shown in the graphics, e.g. NL + EN stacked (unverified naming) | Should |
| **Multilingual colour names** | Colour code → colour name per language ([help](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/connectionbrowsergui_k_verbindungseigenschaften.htm)) | Must |
| **Copilot translations (2027)** | AI translation | Later |
| Tier note | Multilingual translation is Pro-tier in 2026 | — |

The reference set is English-only, but a Dutch integrator typically delivers NL/EN/DE.

---

## 12. Scripting and API

| Feature | How EPLAN implements it | Prio for ElektraWatt |
|---|---|---|
| **Scripting** | C# (and VB) script files loaded at runtime. `[DeclareAction]` registers a method as an action whose string parameters become action parameters; `[Start]` runs on load ([API 2026: DeclareAction](https://www.eplan.help/en-us/Infoportal/Content/api/2026/Eplan.EplApi.AFu~Eplan.EplApi.Scripting.DeclareAction.html)). Scripting is a restricted subset with no extra licence; full API add-ins need the API licence (unverified detail; see the third-party summary at [mcpmarket](https://mcpmarket.com/tools/skills/mp-eplan-development-d21968b1)). | Should: embedded scripting such as Rhai, Lua (mlua) or WASM components |
| **Actions / command line** | Everything is an "action" callable from menus, scripts and the command line, e.g. `plcservice /TYPE:EXPORTADDRESSOVERVIEW … /CONVERTERID:…` ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2024/Content/htm/availableactions_o_plcservice.htm); [available actions](https://www.eplan.help/en-us/infoportal/content/Plattform/2024/Content/htm/availableactions_o_eplanaktionen.htm)), and `XAMlExportProductionData2RASCenterAction` (RiPanel AML) ([API](https://eplan.help/en-us/infoportal/content/api/2023/XAMlExportProductionData2RASCenterAction.html)) | **Must**: a headless CLI for batch report/PDF/export (CI-friendly) |
| **.NET API** | Namespaces such as `Eplan.EplApi.DataModel`, `HEServices` (Revision, CableService, PlcService), `MasterData` (MDPartsDatabase, MDSymbol), with property lists per object type ([API properties](https://www.eplan.help/en-us/Infoportal/Content/api/2025/EPLAN_Properties.html); [CableService](https://eplan.help/en-us/infoportal/content/api/2.9/Eplan.EplApi.HEServicesu~Eplan.EplApi.HEServices.CableService_members.html)). Exclusive locks are needed for structural changes, copy, rename and backup ([API: Locking 2027](https://www.eplan.help/en-US/Infoportal/content/api/2027/Locking.html)). API product description: [Eplan API 2026 PDF](https://www.eplan.com/content/dam/eplan/corporate/performance-descriptions/2026/en/performance-description-eplan-api.pdf). | Should: a stable Rust crate API (`elektrawatt-core`) is the "API"; plugins Later |
| **Event handlers** | Scripts react to events such as project open or page change (`[DeclareEventHandler]`, unverified) | Could |
| Tier note | API extensions are Premium-only in 2026 ([ibKastl](https://ibkastl.de/blog/lizenzierung-eplan-ab-2026)) | — |

---

## 13. Project management, multi-user, cloud

| Feature | How EPLAN implements it | Prio |
|---|---|---|
| **Project management** | Database of projects with properties, search and per-project user info ([help: properties tab](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/prjmanagementgui_r_eigenschaften.htm)). Moved to an internal `*.db` format in 2023 (unverified engine). | Could: a file browser plus metadata index |
| **Multi-user** | Projects edited simultaneously from several workstations. Open modes **Default / Exclusive / Read-only**. Users are notified when two users change the same data ([help: multi-user conflicts](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xesmultiuserconflictgui_k_start.htm); [open modes](https://www.eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/projects_k_bearbeitungsmodusbeimoeffnen.htm)). Pro tier and above. | Later: offline-first file format plus git-friendly diff/merge instead |
| **eManage** | Cloud data management (share and store projects) | Later |
| **ERP/PDM integration** | Advanced/Premium tiers; an SAP integration example exists ([Kaeser forum 2024](https://www.eplan-software.com/fileadmin/cloud/public/events/forum2024/2.2%20Kaeser%20Kompressoren%20-%20SAP%20Integration.pdf)) | Later: CSV/JSON BOM export covers v1 |

---

## 14. Import and export

| Format / function | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **PDF with hyperlinks** | Export includes logical links: **bookmarks to pages** and **jumps between linked elements** (cross-references, interruption points) ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/pdfexportgui_k_start.htm)). Viewers report "three-way jumps" (previous / parent / next) ([Bluebeam community](https://community.bluebeam.com/discussion/779/match-sharepoint-support-for-link-preview)). Colour or black-and-white, layers, 3D PDF from Pro Panel (unverified). | The reference PDF itself is the deliverable | **Must** (PDF/A-capable) |
| **DXF / DWG** | Import as graphic pages, insert into pages, or **create window macros from DXF/DWG**. Export per scheme with **layer mapping** to `EPLANxxx` layers. AutoCAD format versions configurable. EPLAN logic is not exported ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xdxfgui_k_start.htm); [layers 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/xdxfgui_r_ebenen.htm)) | v1 decision 4 | **Must** (DXF; DWG Should because of library licensing) |
| **EDZ** | Parts plus macros (§6) | v1 decision 4 | **Must** |
| **Project XML `*.epj`** | Full project export in XML ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/projects_h_exportieren.htm)). The schema is not publicly documented. | **EPLAN migration path** | **Must** (import) |
| **Parts-list XML** | `XPalXmlExporter`: project article list import/export in XML ([API](https://eplan.help/en-us/infoportal/content/api/2023/XPalXmlExporter.html)) | — | Should |
| **AutomationML (PLC)** | AR APC 1.0–1.4 (§3.12). PLC XML export is single-language, AML is multi-language ([help: export PLC data](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/plcgui_d_exportbuskonfig.htm)) | v1 decision 1 | **Must** |
| **AutomationML (production)** | RiPanel: layout spaces exported as AML for the **Rittal RiPanel Processing Center** (drilling, cutting rails and ducts) ([API action](https://eplan.help/en-us/infoportal/content/api/2023/XAMlExportProductionData2RASCenterAction.html)). Smart Production **EPDZ** = layout spaces + schematic pages + routed connections + embedded **AML** (part placements) + **E3D** 3D data ([help 2024](https://eplan.help/en-us/infoportal/content/Plattform/2024/Content/htm/edaexportgui_k_start.htm)) | v1 decision 2 | **Must** (AML placements); EPDZ Later |
| **Wire fabrication export** | Manufacturing data for wire-processing machines: **Rittal Wire Terminal WT**, **Komax Zeta** and others, **each format a separately licensed interface**, plus a configurable "General wires parts list" with connections, routing track and length ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/productionwiringgui_k_start.htm)) | v1 decision 2 | **Must** |
| **NC data for enclosure machining** | Housing, door and mounting-plate modifications to NC machines, e.g. **Rittal Perforex**, **Steinhauer** ([Pro Panel page](https://www.eplan.com/de-en/products/eplan-pro-panel)) | v1 decision 2 (Rittal) | Should |
| **Labeling** | *Export / labeling* to text or **Excel** with a scheme per label type (terminal markers, wire markers, device tags), Excel templates, sheet names per language ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/labelgui_k_start.htm); [output dialog](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/labelgui_d_beschriftungausgabe.htm)). Vendor marking tools (WAGO Smart Script, Phoenix, Weidmüller) import these ([WAGO](https://techdocs.wago.com/Software/WAGO_EPLAN_Plug-in/en-US/2873315467.html)) | v1 decision 2 | **Must** (CSV/XLSX) |
| **Report export (Excel/CSV)** | Reports can be output as files besides pages (unverified detail) | — | Must |
| **eVIEW upload** | Publish to the cloud viewer (§17) | — | Later |
| **3D: STEP** | Pro Panel imports STEP to build 3D macros (unverified detail) | v1 3D | **Must** |

---

## 15. Pro Panel (3D cabinet layout): v1 decision 3

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Layout spaces and 3D mounting layout** | 3D model of enclosure, mounting panel, **DIN rails, wire ducts**, components, drill holes, cut-outs ([Pro Panel](https://www.eplan.com/de-en/products/eplan-pro-panel); [help 2025](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/cabinetgui_k_start.htm)) | **902**: Schneider NSYPLA1074G + NSYPMM107, ducts 40×60, rails | **Must** |
| **Placement linked to schematic devices** | Parts placed in 3D *are* the devices' functions or part placements, so a DT is never typed twice ([help: part placements](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/panellayoutgui_h_eigenschaftenartikelplatzierungenbearbeiten.htm)) | **defect 2** (T51 vs T55, S51 without prefix) | **Must** |
| **Collision control and installation checks** | Dynamic collision control and automatic checks of installation conditions ([Pro Panel 2.1](https://www.automation.com/en-us/products/product12/eplan-releases-pro-panel-21)) | — | **Must** |
| **Routing paths and wire routing** | Routing paths and rules. **Automatic wire lengths** and **duct fill levels**. Cables and conductors routed in 3D. **Wiring holes** for shortest paths through plates ([Pro Panel page](https://www.eplan.com/de-en/products/eplan-pro-panel)) | v1 decision 3 | **Must** |
| **Wiring Manager (2026)** | Central dialog to route and export the connections of a layout space; *Edit > Routing connections > Manager* ([2026 news](https://www.eplan.help/en-US/Infoportal/Content/Plattform/2026/Content/htm/news_p_propanel_verdrahtungsmanager_488766.htm)) | — | **Must** (as a wire-routing table) |
| **Connection diagrams in 3D** | Generate connection diagrams in the layout ([help](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/panellayoutgui_h_verbindungsdiagramme.htm)) | — | Should |
| **Model views (2D from 3D)** | Derived, dimensioned 2D drawings with legend `.f18` | 902 as a generated view | **Must** |
| **Multi-view (2026)** | Up to 10 saved 3D views | — | Could |
| **Copper / busbars** | Busbars ("Busbar (bending)") defined by a rail macro, path and bend radius. Flat or edgewise bends. **Unfolds** for bending machines; NC transfer ([help: unfolds](https://eplan.help/en-us/Infoportal/Content/Plattform/2.9/Content/htm/copper_k_abwicklung.htm); [API BusBar 2026](https://www.eplan.help/en-us/Infoportal/Content/api/2026/BusBar.html)) | — | Later |
| **Drilling patterns and cut-outs** | From part data (EDZ / Data Portal); cut-out legend; NC export | 902 | Should |
| **Wire-end sleeves by wire class (2027)** | §1.2 | — | **Must** (ferrules are in v1 decision 2) |
| **Heat / climate** | Rittal climate calculation via partner tools (unverified) | IEC 61439 temperature rise | Later |

---

## 16. Manufacturing: Smart Production, Smart Wiring, labels

| Product / feature | What it does | Prio |
|---|---|---|
| **Eplan Smart Wiring** | Virtual wiring assistant for manual panel wiring. Step-by-step guidance; optional 3D cabinet view; red/yellow/green status; PC or tablet; schematics reachable from each connection; production report per order. Data comes from Pro Panel or from Excel wire lists of other ECAD systems; a central server lets several technicians work on one project ([product](https://eplan.com/us-en/products/eplan-smart-wiring.html); [2.9 news](https://eplan-software.com/company/news/eplan-smart-wiring-2-9-wiring-made-easy)) | Could: a "wiring checklist" mode in a viewer is cheap once the wire list exists |
| **Eplan Smart Production** | Enclosure production web app fed by EPDZ. Interface to **Rittal Wire Terminal WT C** keeps bundle information in sync with the wiring list ([Smart Production page](https://eplan.com/us-en/products/eplan-smart-wiring); [EPDZ help](https://eplan.help/en-us/infoportal/content/Plattform/2024/Content/htm/edaexportgui_k_start.htm); [2026 description PDF](https://www.eplan.com/content/dam/eplan/corporate/performance-descriptions/2026/en/performance-description-eplan-smart-production.pdf)) | Later (the export data is Must, the web app Later) |
| **Labeling** | §14 | Must |

---

## 17. Fluid (pneumatics, hydraulics)

| Feature | How EPLAN implements it | Ref. | Prio |
|---|---|---|---|
| **Fluid power schematics** | Hydraulics, pneumatics, cooling, lubrication, cooling lubricant, gas; ISO 1219 or IEC 81346 mechatronic structure ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/ftechnic_k_start.htm); [product](https://www.eplan.com/de-en/products/eplan-electric-p8/fluid-power-engineering/)) | **203–206**: 5/2 solenoid valves, ports 2/4/3/5 | **Must** |
| **Fluid symbol libraries** | Per discipline, usable separately or together | ISO 1219-1 symbols | **Must** (pneumatic subset) |
| **Fluid device identification** | ISO 1219-2 or other predefined identification structures | — | Should |
| **Connection splicers, line connectors** | Real items, reported in BOMs and order lists | tube fittings | Should |
| **Mechatronic cross-references** | The solenoid coil (electrical) and the valve (fluid) are one device across disciplines; valve-terminal modules modelled as black boxes or PLC cards (unverified modelling details) | Q0.1 on 203 ↔ CPX output | **Must** |
| **Fluid page rules** | Rotate/flip-to-standard not possible on the fluid page type ([help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/ftechnic_k_seitentyp_fluid.htm)) | — | — |
| Packaging | Since 2026 fluid is part of every Electric P8 tier, but reportedly not of Pro Panel ([CAD.de](https://ww3.cad.de/foren/ubb/Forum467/HTML/015655.shtml)) | — | — |

---

## 18. Preplanning, P&ID, topology

| Feature | How EPLAN implements it | Prio |
|---|---|---|
| **Preplanning segments** | **Structure segments** (project structure) and **planning objects** in the Pre-planning navigator. Types: PCT loops (only PCT-loop-function children), PCT loop functions, containers, connection planning objects (piping, cable). Structure boxes on Pre-planning and P&I pages. Function templates and macros on planning objects ([help 2026](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/planninggui_d_navigator.htm); [principle](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/planninggui_k_prinzip.htm); [macro tab](https://eplan.help/en-us/infoportal/content/Plattform/2024/Content/htm/planninggui_r_makro.htm)) | Later |
| **P&ID page type** | Process diagrams; segments can colour linked P&ID components | Later |
| **2027 Preplanning automation** | Automatic symbol rotation, arrangement, line interruption, flow direction; 3D project generation | Later |
| **Topology / routing-path networks** | Topology pages and navigator. Routing paths between topology functions or routing points (*Insert > Topology > Routing path*). Routing generates "Topology"-representation connections and determines **track and length** automatically ([help 2025](https://www.eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/cablinggui_k_vorgehensweise.htm); [insert routing path](https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/cablinggui_h_streckeeinfuegen.htm)) | Could: field-cable lengths between cabinets (J-E-206 ↔ J-E-2416) |
| **FieldSys cable-length calculation** | Pro tier ([ibKastl](https://ibkastl.de/blog/lizenzierung-eplan-ab-2026)) | Later |

---

## 19. Automated engineering: Cogineer and eBUILD

| Feature | How EPLAN implements it | Prio |
|---|---|---|
| **EPLAN Cogineer** | Rule-based generation of schematics from macros ("typicals") with value sets. The Designer builds libraries; Project Builder configures. Placeholder syntax `#<Name>` for value sets ([Cogineer help: macro value set](https://www.eplan.help/en-us/Infoportal/Content/Cogineer/2.9/Content/htm/cogineer_d_mtypical_macro_valueset.htm); [release notes](https://www.eplan.help/en-us/Infoportal/Content/Cogineer/2.9/Content/htm/cogineer_release_notes.htm)) | Later |
| **EPLAN eBUILD** | Cloud-based automated engineering. **Project Builder** (also in the free *eBUILD Free*) plus **Designer** for own libraries. Needs P8, Fluid or Pro Panel (2D) 2.8 HF3+. Output goes to a local project; Cogineer libraries can be uploaded ([FAQ](https://www.eplan.com/int/en/support/ebuild.html); [2026 description](https://www.eplan.com/content/dam/eplan/corporate/performance-descriptions/2026/en/performance-description-eplan-automated-project-generation.pdf)). Included in P8 Pro from 2026. | Later (but placeholder objects and value sets, Should, are the foundation) |

---

## 20. Cloud apps and AI: eVIEW, eManage, eStock, Copilot

| Feature | How EPLAN implements it | Prio |
|---|---|---|
| **eVIEW** | Cloud project viewer in the browser or inside the platform. **Redlining and greenlining in the browser only**; the platform side can change redline status and show/hide them. Roles and rights; free tier after cloud registration; needs Platform 2.9 SP1+; Chrome/Safari/Firefox ([help: what is eVIEW](https://www.eplan.help/en-us/Infoportal/Content/eView/Content/htm/eVIEW_whatIsEview.htm); [create redlining](https://eplan.help/en-US/Infoportal/Content/eView/Content/htm/eVIEW_red_create.htm); [FAQ](https://www.eplan.com/int/en/support/eview-free.html)) | Later. Could: a static HTML/WASM viewer export |
| **eManage / eStock** | §13 / §6 | Later |
| **Eplan Copilot (2027)** | AI assistant (§1.2) | Later |

---

## 21. Harness proD (brief)

A stand-alone 3D wire-harness tool. It imports 3D MCAD geometry (CATIA, Inventor, NX, SolidWorks, Solid Edge) and ECAD
wire lists, routes wires on paths, and checks bend radii, cross-sections (also AWG), collisions and bundle limits. It
produces **2D nailboard (formboard)** drawings and reports such as wire cutting lists and BOMs
([help overview](https://www.eplan.help/es-es/Infoportal/Content/Harness/2.8/Content/htm/Overview.htm);
[release notes incl. 2026 Update 1](https://www.eplan.help/en-US/Infoportal/Content/htm/portal_rel_notes_harness.htm);
[2026 performance description](https://www.eplan.com/content/dam/eplan/corporate/performance-descriptions/2026/en/performance-description-eplan-harness-prod.pdf)).
A P8 ↔ Harness link exists ([help 2025](https://eplan.help/en-us/infoportal/content/Plattform/2025/Content/htm/harnessgui_k_start.htm)).
**ElektraWatt priority: Later.** Not relevant to cabinet building at the pilot user.

---

## 22. Reference drawing set → EPLAN feature map

| Folio | Content | EPLAN features that produce it | ElektraWatt must-haves derived |
|---|---|---|---|
| 1 | Title page, project data, cable colour definitions, standard sizes | Title page `.f26` from project properties; potential definitions with connection properties; potential overview or graphic page | Project properties as the single source; potential-based colour and cross-section table that also **drives** wire properties (defect 8) |
| 2 | TOC (Folio / Title / Location +) | TOC `.f06` | Auto-regenerated report pages |
| 3 | Profibus topology: -201U1, -3X1, -3W1, "DP" interruption, external connection box | Overview page; PLC box with bus port; bus cable; interruption point; structure box with `+J-E-206` | Bus ports and bus cables; structure boxes; renumbering consistency (defect 1: -3W1 vs BOM -4W1) |
| 100, 200, 900 | Section separators | Graphic pages or page macros; document-type/section structure (`&`) | Page sections; separator page macro |
| 101 | Main current: S51, 1X1, MCBs, thermostat, fan, door switch, lamp; outgoing L-1 / N-1 / PE-1.1 | Multi-line page; autoconnect; terminals; part data per main function; interruption points with cross-refs; connection definition points "2.5 BK" | Online numbering conformity check (defect 3); part completeness check (E56/S56, defect 4) |
| 102 | PSU -102G1 (black box), -102F1, -102F2 MICO; 24VDC-1..4, 0VDC, PE-0VDC | Black boxes and device connection points; potential definition points; interruption points (star) | Potentials with types; DT adoption into boxes |
| 103 | Distribution strips +JE2416-X1031 / X1032, "Pneumatics" / "Spare" | Terminal strips, multi-level/twin terminals, saddle jumpers, path function texts, displayed DT vs full DT | Terminal model with levels and jumpers; P001005-type check; computed displayed DT |
| 106 | Festo cables -106W1/W2 with conductor colours; CPX terminals with "(201-E1)" | Cable definition lines, conductor assignment; overview cross-references | Cable model, conductor colours, cable diagram; cable-in-BOM check (-106W2 missing) |
| 201, 202 | Manifold overview, "PART DATA" box, module references "∞ 203-B4" | Overview representation; part assemblies and accessories; **embedded report** for PART DATA; main ↔ aux cross-references | Embedded reports; assemblies with order codes; overview pages |
| 203–206 | ISO 1219 5/2 valves with PLC addresses Q0.x and process tags | Fluid page type; PLC connection points; function text; placeholder-object macros or PLC-driven generation | Fluid symbols; PLC address model; macro value sets |
| 901 | Part list | Parts list `.f01` / summarized `.f02` | Quantity aggregation; always regenerated (defects 1, 4) |
| 902 | Cabinet layout with PART DATA, ducts, rails, placed devices | Panel layout / Pro Panel model view; enclosure legend `.f18` | Placements bound to device identity (defect 2); 3D (v1) |
| every page | 0..16 × A..H frame, title block with =P01, +J-E-2416, Revision, Prev/Next | Plot frame with special texts; configurable cross-reference format | Managed revisions (defect 5); single project description (defect 6); computed Prev/Next (defect 7) |
| (absent) | No wire list, terminal diagram, cable diagram, PLC overview, wire numbers | Connection list, terminal diagram `.f13`, cable diagram `.f09`, PLC card overview, connection numbering | All are **Must** (defect 9) |

---

## Implications for ElektraWatt

1. **Copy EPLAN's object model, not its UI.** The core entities should be: *Project → Pages → placed objects*, and
   *Functions* (with function definition and connection points) grouped into *Devices* by DT, with *Parts* on the main
   function. Add *Connections* generated from graphics plus connection definition points, *Potentials/Signals/Nets*,
   *Terminal strips*, *Cables*, *PLC boxes/cards/points*, *Structure identifiers* and *Placements* (2D/3D). Every
   cross-reference, contact image, displayed DT, Prev/Next, TOC and BOM must be a **computed view** over this model,
   never stored text. This alone eliminates defects 1, 2, 6 and 7.
2. **Store the DT as a structured key, not a string.** Use typed IEC 81346 aspects `==`, `=`, `++`, `+`, `&`, `#`, `-`
   with entries drawn from a **structure identifier table** (unique, ordered, described). Derive the full DT, the
   displayed DT and the page full name from it. Run online numbering on insert and offline renumbering with formats
   (identifier / page / column / row / counter / separator), with ID-stable references so renumbering cannot desync
   the BOM (defect 1). Ship a **page-based format** preset matching the pilot user's `-101F1` convention and a P017008-style
   check (defect 3).
3. **Make potentials the source of wire properties (defect 8).** Implement EPLAN's inheritance chain: project default
   < potential definition < signal < net < connection < connection definition point. Use a user-extensible colour-code
   table (IEC 60757 codes plus custom codes such as **DBU**) with multilingual names. Generate folio 1's colour table
   from the potential definitions, so documentation and data cannot diverge.
4. **The cross-reference engine must have a configurable format.** Support `<page>-<row><column>` ("102-A1") as well as
   EPLAN-style `/page.column`. Implement star and chain interruption points, main/aux and overview cross-references,
   contact images "in path" and "on component", and the missing-counterpart check. Export all of them as PDF
   hyperlinks.
5. **Use fixed report types with user forms.** Follow EPLAN's split: a fixed set of report types (a Rust enum: TOC,
   parts list, summarized parts list, device list, terminal diagram, terminal-strip overview, cable diagram, cable
   overview, connection/wire list, PLC diagram, PLC card overview, assignment list, potential overview, revision
   overview, title page, enclosure legend, mounting list), each rendered through a **user-editable form** (header /
   data / footer, dynamic rows, next forms) with typed placeholders. Add **report blocks** that regenerate on save and
   on PDF export, and **embedded reports** for "PART DATA" boxes (201, 202, 902). Defects 4, 7 and 9 disappear.
6. **Title blocks only through special texts.** Every title-block field is a placeholder bound to a project, page or
   plot-frame property. Managed **revisions** (project revision, page revision, revision overview, completed/frozen
   revision, property comparison with revision marks) are Must for v1 (defect 5).
7. **Ship a check engine on day one.** Mirror EPLAN's six-digit class/message numbering, e.g. `EW001005` for terminals,
   `EW003xxx` cables, `EW005xxx` connections, `EW017xxx` numbering, plus ElektraWatt classes for parts and placements.
   Support modes offline / online / **prevent** and named check schemes. Provide a default "pre-delivery" scheme that
   flags every defect class in the reference set: unplaced or orphan placements, missing part data, duplicate terminal
   designations, inconsistent identifiers, revision mismatch, cable without BOM entry.
8. **Terminal and cable models must support the real hardware.** Multi-level terminals with main/aux levels, twin
   connection points (PT 2.5-TWIN), saddle and wire jumpers (including the `1/-1` inter-level jumper semantics),
   accessories (end plates, covers, markers), spare terminals, and the terminal-strip editor table. Cables need
   definition lines, conductor colour/number, shields and automatic conductor assignment.
9. **PLC v1 scope (decision 1).** PLC box / card / point model with address, symbolic address, data type, channel and
   function text; addressing/readdressing; PLC overview pages; PLC diagram, card overview and assignment list. Export
   **AutomationML AR APC 1.x** (IEC 62714 CAEX; the TIA Portal V17+ and Studio 5000 path), **CSV** (TIA tag table) and,
   for Codesys, **PLCopen XML** (EPLAN has no verified Codesys converter, which is an opportunity). Model Festo CPX
   valve terminals as PLC boxes with DO cards so that 203–206 addresses are first-class data.
10. **Manufacturing v1 (decision 2).** Wire list with source/target, cross-section, colour, wire number, **routed
    length** (from 3D) and **ferrule / wire-end sleeve** (rule-based by wire class, as in EPLAN 2027). Labeling export
    to CSV/XLSX by scheme. Machine exports: Komax and Rittal WT formats (licensing and format docs are an open
    question; start with a generic configurable CSV like EPLAN's "General wires parts list"). AML part-placement
    export for Rittal RiPanel / Perforex.
11. **3D v1 (decision 3).** Layout spaces, enclosure/plate/rail/duct as parametric parts, placements bound to device IDs
    (never DT strings), collision checks, a routing-path graph, shortest-path routing with duct fill levels, automatic
    lengths, a Wiring-Manager-style table, and generated 2D model views with an enclosure legend. STEP import for part
    3D (needs an FFI B-rep kernel such as OCCT; justified by the brief). Copper busbars and multi-view: Later.
12. **Import v1 (decision 4).** Priority order: **QElectroTech `.qet`** (the user's current tool), **DXF** (graphics or
    macro creation, with layer mapping), **EDZ** (parts plus macros plus drilling patterns plus 3D), then **EPLAN
    `*.epj` XML project export** (undocumented schema: reverse-engineer from samples) and EPLAN parts XML. Do not
    attempt the binary `*.elk`/`*.edb` database.
13. **Macros.** v1: window and page macros with variants per representation type and part macros. v1.x: placeholder
    objects with value sets. These are the basis for later rule-based generation (Cogineer/eBUILD equivalent): folios
    203–206 are four copies of the same typical with different addresses and tags.
14. **Parts DB.** SQLite, file-based and git-friendly exports. Use part **variants**, accessories, assemblies (Festo
    order codes), function templates, technical data, and classification attributes (ECLASS IRDI; ETIM class).
    Import BMEcat/ECLASS. A Data-Portal-style online library is Later; consider a community-curated open parts
    repository instead.
15. **Translation in the data model now.** Every user-visible text property is a `MultiLangString` from v1, even if
    the dictionary UI arrives in v1.x. Retrofitting it later is very expensive (EPLAN's colour names, page
    descriptions and function texts are all multilingual).
16. **Scripting and automation.** Expose a headless CLI ("actions") for PDF, report, export and check runs from v1; this
    is CI-friendly and replaces much of EPLAN scripting. Embedded scripting (Rhai, Lua or WASM components) can follow
    in v1.x on top of the same command layer.
17. **Do not do now:** multi-user concurrent editing, cloud apps (eVIEW/eManage/eStock), Copilot, Preplanning/P&ID,
    Harness, NFPA standards exchange, busbar copper, Smart Wiring. Keep the model open for them: stable object UUIDs,
    an event log, and a representation-type enum that can grow.

## Open questions

1. **EDZ container format**: exact archive type (ZIP vs 7z, community reports vary) and the XML schema of the part and
   macro files. Also, may an open-source tool parse EDZ files downloaded under the EPLAN Data Portal terms? This needs
   a legal review.
2. **EPLAN `*.epj` project XML**: is the schema stable across 2024–2027, and does the export contain full logic
   (connections, CDPs, PLC data) or only page graphics plus properties? Can the user (the pilot user) or a partner provide
   sample `*.epj` files for reverse engineering?
3. **Machine formats**: are the Rittal Wire Terminal WT and Komax (Zeta/Topwin) import formats publicly documented, or
   do they require vendor partnership? Does Rittal accept AML (EPLAN's RiPanel route) from third-party tools?
4. **ETIM**: EPLAN's support for ETIM is unconfirmed. Which classification does the pilot user's purchasing/ERP actually use
   (ETIM is common with Dutch wholesalers such as 2BA)?
5. **Cross-reference format detail**: exact EPLAN option set (separators, order of page/row/column, identifier
   suppression). Needed only for faithful EPLAN import; ElektraWatt's own format engine can be a superset.
6. **Potential types enum**: the exact EPLAN list (L, N, PE, M, +, −, FE, undefined …) for lossless EPLAN import.
7. **Twin terminals on folio 103**: are the four "1+" designations two PT 2.5-TWIN terminals with multiple connection
   points, four separate bridged terminals, or levels? The user's intent decides the default terminal modelling.
8. **Section pages 100/200/900**: does the user want them as document-type structure (`&`), as pure graphic pages, or
   generated automatically from page sections?
9. **Revision workflow**: what does a the pilot user revision cycle look like (customer approval, "as-built")? This decides
   whether page-level revision indices or only project-level revisions are needed in v1.
10. **Codesys exchange**: is PLCopen XML sufficient for the user's Codesys targets, or is a CSV/IO-mapping import into
    Codesys' device tree required?
11. **Festo configurator order codes** ("51E-F13GCQP-D+H2GG"): model them as an assembly part with child parts, or as
    a user property on a black box? Does the user want BOM explosion of configured products?
12. **EPLAN 2027 details**: the wire-class ferrule rules and the "automated fitting selection" are only described in
    marketing text. Check them hands-on (a beta or trial licence) if parity matters.
