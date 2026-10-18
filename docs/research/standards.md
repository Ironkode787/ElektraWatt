# Engineering standards for ElektraWatt and the software rules they imply

Research note for the ElektraWatt design (open-source, Rust, EPLAN-class ECAD for control cabinets).
Date of research: 2026-10-17. Scope: which standards ElektraWatt must implement or respect, and how
each one becomes **data structures, validation rules, check-runs and report contents**.

> **Anonymization note:** dates and the manufacturing year of the reference project in this document are synthetic placeholders, not the original values.

**How to read this note**

- Claims backed by a source carry an inline link. Claims marked **(unverified)** come from the
  author's own knowledge, or from a secondary source that could not be checked against the standard
  text. During this session WebFetch could only reach GitHub hosts. Most other facts come from search
  result summaries of the linked pages.
- "Folio NNN" refers to the user's reference set <drawing id> (QElectroTech 0.100.0-dev
  output). "Defect n" refers to the defect list in the shared brief.
- **Copyright rule used throughout.** IEC, ISO and CENELEC texts, tables, figures and database
  contents are copyrighted. A database such as IEC 60617 or the IEC CDD is also protected by
  database rights. ElektraWatt may *implement the rules*: algorithms, grammars, check logic, letter
  codes cited as identifiers, and numbers that are facts. It must not ship the standards' prose
  definitions, figures or bulk database extracts. Section 14 gives a ship/do-not-ship table.

---

## 0. Edition register (status as of 2026-10)

| Standard | Current edition found | Status / notes | ElektraWatt priority |
|---|---|---|---|
| IEC 81346-1 | Ed. 2.0 **2022** (replaces 2009). Adds the **type aspect**, an information model and metadata recommendations ([AFNOR](https://www.boutique.afnor.org/en-gb/standard/iec-8134612022/industrial-systems-installations-and-equipment-and-industrial-products-stru/xs141115/322260)) | EN IEC 81346-1:2022 | v1 core |
| IEC 81346-2 | Ed. 2.0 **2019** (replaces 2009). Classes now reflect the *inherent function*; three-level scheme; separate scheme for spaces ([IEC webstore](https://webstore.iec.ch/publication/29181)) | EN IEC 81346-2:2019. The 2009 edition is still widely used | v1 core (two identifier sets) |
| IEC 61082-1 | Ed. 3.0 **2014**, horizontal standard; adds wireless interconnections and exceptions for placing reference designations ([IEC](https://webstore.iec.ch/en/publication/4469)) | EN 61082-1:2015 | v1 core |
| IEC 61355-1 → **IEC 81355-1** | IEC 61355-1:2008 is **replaced by IEC 81355-1:2024** (published 2024-12). The DCC becomes the **ICC** (information kind classification code), and a DCC→ICC conversion table is provided ([ISO](https://www.iso.org/standard/83169.html), [iTeh](https://standards.iteh.ai/catalog/standards/iec/9634ad0a-4ba4-499b-a6e9-08e2e0d84a50/iec-81355-1-2024)) | EN IEC 81355-1:2025 supersedes EN 61355-1:2008 ([ANSI/DS](https://webstore.ansi.org/standards/ds/dseniec813552025)) | v1: support both codes |
| IEC 60617 | Online database only, by subscription ("IEC 60617:2026 DB") ([IEC](https://library.iec.ch/iec60617)) | Copyright plus database rights | Symbol *style* only |
| IEC 81714-2 / -3 | 81714-2:2006 (symbol libraries in computer-sensible form); 81714-3 (connect nodes) ([iss.rs](https://iss.rs/en/project/show/iec:proj:8258)) | Under periodic review | Data model guidance |
| ISO 1219-1 / -2 | ISO 1219-1:2012 + Amd 1:2016 (confirmed 2017); ISO 1219-2:2012 ([ISO](https://www.iso.org/standard/60184.html), [Amd1](https://www.iso.org/standard/66287.html)) | No newer edition found | v1 (pneumatics, folios 201–206) |
| IEC 60757 | Ed. 2.0 **2021** (replaces 1983). Letter codes only, a new subclause on alternating colours, and Annex A with RGB examples ([IEC](https://webstore.iec.ch/publication/66266)) | EN IEC 60757:2021 | v1 core |
| IEC 60445 | Ed. 7.0 2021 + **AMD1:2026** (consolidated Ed. 7.1, 2026-01) ([IEC](https://webstore.iec.ch/en/publication/111816)) | EN IEC 60445:2021/A1 approved by CENELEC 2026-03-04 | v1 core |
| IEC 60204-1 | Ed. 6.0 2016 + **AMD1:2021** (consolidated 6.1) ([IEC](https://webstore.iec.ch/en/publication/71256)) | **EN 60204-1:2018 + A1:2025**. The 2018 edition is withdrawn on 2028-04-30 ([NBS/BSI](https://www.thenbs.com/publicationindex/documents/details?Pub=BSI&DocId=347000)). DIN EN 60204-1:2026-06 ([DIN Media](https://www.dinmedia.de/en/standard/din-en-60204-1/400530593)). No 7th edition found | v1 core (rule pack) |
| IEC 61439-1 / -2 | Ed. 3.0 **2020** (both parts) ([IEC](https://webstore.iec.ch/en/publication/32338)) | Adds group rated current, DC requirements, class I/II ([KEMA](https://www.cesi.it/app/uploads/2020/11/KEMA-Labs-LV-White-Paper-IEC-61439-1-2020-WHATS-NEW.pdf)) | v1: power-loss/thermal; later: full DV |
| IEC TR 60890 | Ed. 3.0 **2022** ([sample](https://cdn.standards.iteh.ai/samples/103729/ac9f2264127744829ed89c229d8fa150/IEC-TR-60890-2022.pdf)) | Calculation method for temperature rise | v1/v1.x |
| UL 508A | 3rd ed. 2018 (+revisions); SB4 is the SCCR method ([UL summary](https://www.ul.com/sites/default/files/2020-06/UL508A-Summary-of-Requirements-3rd-Edition_August%2017_2018_Rev1.pdf)) | US | Later |
| NFPA 79 | 2024 edition (unverified: whether the 2027 cycle is published) | US | Later |
| CSA C22.2 No. 286 | **286:23** (3rd ed.), effective 2025-01-15 ([CSA](https://www.csagroup.org/store/product/2430558/), [SCC](https://scc-ccn.ca/standards/notices-of-intent/csa-group/industrial-control-panels-and-assemblies-1)) | Canada | Later |
| IEC 62424 / IEC 62714 | IEC 62424:2016 (CAEX 3.0); IEC 62714-1:2018 (AML Ed. 2); -2:2015; -4:2020; -5:2022 ([AutomationML](https://www.automationml.org/news/the-2nd-edition-of-iec-62424-is-released/)) | AR APC 1.4.0 on the AML site; EPLAN 2022 supports 1.2.0 | v1 (PLC exchange) |
| ECLASS / ETIM | ECLASS 15.0 (licensed); ETIM 10.0 (2024-12-05, ODC-By) ([ECLASS](https://eclass.eu/en/eclass-standard/licenses/language-versions), [ETIM](https://www.etim-international.com/?p=1811)) | — | v1 part-data keys |
| IEC 61360-1 / IEC CDD | IEC 61360-1:2017; CDD online with EULA ([CDD license](https://cdd.iec.ch/cdd/iec61360/iec61360.nsf/License?openPage)) | — | v1 property IDs |
| IEC 61131-3 | Ed. 4.0 **2025** (May 2025). IL removed, UTF-8 strings added ([Wikipedia](https://en.wikipedia.org/wiki/IEC_61131-3), [Standard Norge](https://online.standard.no/en/iec-61131-3-2025-ed4)) | — | v1 addresses |
| ISO 19005 (PDF/A) | Parts 1–4 (PDF/A-4:2020 is based on PDF 2.0) ([PDF Association](https://pdfa.org/resource/iso-19005-pdfa)) | — | v1 output |
| ISO 7200 | 2004 | — | v1 title block |
| ISO 5457 | 1999 + Amd 1:2010 ([ISO](https://www.iso.org/standard/46218.html)) | A4 is allowed in landscape and portrait | v1 frames |
| Regulation (EU) 2023/1230 | Applies from **2027-01-20** ([EU-OSHA](https://osha.europa.eu/en/legislation/directive/regulation-20231230eu-machinery)) | Replaces the Machinery Directive | Context |

---

## 1. IEC 81346-1 / -2: the reference designation system (RDS)

### 1.1 What the standards say

**IEC 81346-1 (structuring principles)**
- Systems are structured by **aspects**. Each aspect is identified by a **prefix sign**.
- Each structure is a tree. A **multi-level reference designation** is the path from the top node.
  Each level is a **single-level reference designation** (letter code + number).
- Edition history:
  - The 2009 edition already had function `=`, product `-` and location `+`, plus "other aspects" with
    `#` ([81346.com](https://81346.com/81346-1), summarised in search results).
  - The 2022 edition adds the **type aspect**. Secondary training material gives it the prefix `%`
    ([INCOSE IS2016 slides](https://www.incose.org/wp-content/uploads/legacy/presentation/IS2016-pdf/7-10-4.pdf)).
    It also adds an *information model* of the RDS, rules for designating *relations* between
    objects, and synchronisation with IEC 81346-2:2019 and ISO 81346-12:2018
    ([AFNOR abstract](https://www.boutique.afnor.org/en-gb/standard/iec-8134612022/industrial-systems-installations-and-equipment-and-industrial-products-stru/xs141115/322260)).
  - The normative prefix for the type aspect could not be read from the standard. Treat `%` as
    **(unverified)**.

**IEC 81346-2 (classes and letter codes)**
- The 2019 edition changed the basis of the entry classes from "purpose/task" (2009) to the
  **inherent function** of the object. It introduced a three-level scheme (class / subclass /
  sub-subclass), gave each class a definition and a preferred term, and added a separate
  classification for spaces ([IEC](https://webstore.iec.ch/publication/29181),
  [iTeh](https://standards.iteh.ai/catalog/standards/iso/6ae88f05-e5ea-4b25-a027-3ca8c616a9cd/iec-81346-2-2019)).
- In 2019, the letters **A, I and O shall not be used** as class codes, and **D, J, L, V, Y, Z are
  reserved** (from the published sample of the standard, as reported in search results;
  [sample](https://cdn.standards.iteh.ai/samples/21908/1b996d119d0945b2b28359332ffa9b3f/IEC-81346-2-2019.pdf)).

### 1.2 Prefix signs and separators ElektraWatt must parse

| Sign | Meaning | Origin | Example from the reference |
|---|---|---|---|
| `=` | Function aspect | IEC 81346-1 | `=P01` (title block) |
| `-` | Product aspect | IEC 81346-1 | `-101F1`, `-3W1` |
| `+` | Location aspect | IEC 81346-1 | `+JE2416`, `+J-E-2416` (see 1.6) |
| `%` | Type aspect (2022) | IEC 81346-1:2022, **(unverified sign)** | — |
| `#` | "Other aspects" (2009). In EPLAN, alternative sign for functional assignment | IEC 81346-1:2009; EPLAN | — |
| `==` | Functional assignment (EPLAN block above `=`) | EPLAN convention ([EPLAN help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/projectstructure_k_hauptkundunterk.htm)) | — |
| `++` | Installation site (EPLAN block above `+`). Also "site of installation" in RDS-PP practice | EPLAN; RDS-PP **(unverified for RDS-PP)** | — |
| `&` | Document kind (DCC/ICC) | IEC 61355-1 / IEC 81355-1 | `&EFS` |
| `:` | Terminal / connection point (e.g. `-X1:3`, `-K1:A1`) | IEC 61666 / 81346 practice **(unverified clause)** | `1X1:PE` |
| `;` | Signal designation | IEC 61175-1 / RDS-PP practice **(unverified)** | — |
| `/` | Page within a document (EPLAN page name, cross-reference) | EPLAN / IEC 61082-1 practice | `/2.5` |
| `.` | Column after page (`/7.8`), or separator between sub-levels within one aspect | EPLAN ([help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xessettingsgui_d_einstellungenprojektabbruchstellen.htm)) | — |

EPLAN fixes these identifier blocks for pages and devices: `==` functional assignment (also `#`),
`=` higher-level function, `++` installation site, `+` mounting location, `&` document type (a space
may replace `&`), plus a user-defined block with an arbitrary prefix. Blanks and `-` are not
permitted inside user-defined fields ([EPLAN 2.9 help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/projectstructure_k_hauptkundunterk.htm)).
A full EPLAN page name therefore looks like `==FA=P01++SITE+JE2416&EFS/101`, and a device looks like
`=P01+JE2416-F1`.

### 1.3 Grammar to implement (proposal)

```ebnf
rds          = { aspect_seg } ;
aspect_seg   = prefix , level , { (prefix | ".") , level } ;   (* same aspect repeated = deeper level *)
prefix       = "==" | "=" | "++" | "+" | "-" | "%" | "#" ;
level        = letter_code , number | number | letter_code ;   (* configurable strictness *)
letter_code  = upper , [ upper , [ upper ] ] ;                  (* 1..3 letters per 81346-2:2019 *)
upper        = "A".."Z" - ("I" | "O") ;
number       = digit , { digit } ;
terminal     = rds , ":" , term_id ;    term_id = (upper | digit | "+" | "-" | "." )+ ;
doc_kind     = "&" , upper , upper , [ upper ] , [ number ] ;    (* &EFS, &EFS1 *)
page_ref     = [ rds ] , [ doc_kind ] , "/" , page_name , [ "." , grid_ref ] ;
```

Notes:
- Whether `.` may replace a repeated prefix is **(unverified)** for IEC 81346-1. EPLAN lets the
  sub-level separator be configured per block.
- Real projects contain tags that are not conformant (Section 1.6). The parser must therefore run in
  two modes:
  - **strict**: ISO/IEC grammar only;
  - **lenient**: import mode, which keeps unknown tags as opaque strings and raises warnings.

### 1.4 Letter codes: 2009 versus 2019 (single letters)

The 2009 column is taken from the QElectroTech documentation, which quotes EN/IEC 81346-2:2009
([QET doc](https://github.com/qelectrotech/qelectrotech-doc/blob/175435e635838822e17a4bc1c9f776aa4af62e55/source/users/element/properties/element_numbering.rst)).
The 2019 column is taken from the CCI component table. CCI adopts the IEC 81346-2:2019 scheme
([CCI table in IfcOpenShell](https://github.com/IfcOpenShell/IfcOpenShell/blob/v0.8.0/src/bonsai/bonsai/bim/data/classifications/CCI%28EN%29.ifc)).
The 2019 terms below are CCI's wording; verify the IEC wording against a licensed copy.

| Letter | 2009 (purpose/task based) | 2019 (inherent function) |
|---|---|---|
| A | Two or more purposes or tasks | **not used** |
| B | Converting an input variable into a signal | Sensing object |
| C | Storing of energy, information or material | Storing object |
| D | (reserved) | reserved |
| E | Providing radiant or thermal energy | Emitting object |
| F | Direct protection from dangerous or unwanted conditions | Protecting object |
| G | Initiating a flow of energy or material | Generating object |
| H | Producing a new kind of material or product | Matter processing object |
| K | Processing signals or information | Information processing object |
| M | Providing mechanical energy for driving | Driving object |
| N | (not in the 2009 list) | **Covering object** (new) |
| P | Presenting information | Presenting object |
| Q | Controlled switching or varying a flow | Controlling object |
| R | Restricting or stabilising | Restricting object |
| S | Converting a manual operation into a signal | Human interaction object |
| T | Conversion of energy maintaining the kind of energy | Transforming object |
| U | Keeping objects in a defined position | Holding object |
| V | Processing (treating) of material or products | **reserved** (merged into H) |
| W | Guiding or transporting | Guiding object |
| X | Connecting objects | Interfacing object |

Electrical subclasses in 2019 that matter for cabinets (two letters; three-letter examples from CCI
are shown in brackets, **(verify against IEC Table 3)**):

| Code | Meaning (CCI wording) | Typical cabinet devices |
|---|---|---|
| BG | Physical dimension / position sensing (BGB position switch) | Door switch, limit switch, proximity sensor |
| BT | Temperature sensing (BTB temperature switch) | Enclosure thermostat |
| BP | Pressure sensing | Pressure switch/transmitter |
| EA | Light (EAA electric lamp) | Cabinet light |
| EB | Electric heating (EBD hot air blower) | Enclosure heater, fan heater |
| FA | Overvoltage protection (FAB varistor) | SPD |
| FB | Earth-fault current protection (FBA RCD) | RCD/RCBO |
| FC | Overcurrent protection (FCA fuse, FCB MCB) | Fuse, MCB, electronic circuit breaker |
| GQ | Gaseous flow generating (GQB mechanical fan) | Filter fan |
| KE | Electric signal processing (KEB control unit, KEC communication device) | PLC CPU, bus coupler |
| KF | Electric signal relaying (KFA relay, KFB time relay, KFE network switch) | Relays, Ethernet switch |
| KH | Fluid signalling (KHB pilot valve) | Pilot valves |
| MA / MB / MM | Rotational / linear electromagnetic / fluid-powered drive (MMB pneumatic cylinder) | Motors, solenoids, cylinders |
| PF / PG | Visible state indicator (PFA signal lamp) / scalar display | Pilot lights, meters |
| QA | Electric controlling (QAA contactor, QAB circuit breaker, QAC electronic switch) | Contactors, MCCB |
| QB | Electric separating (QBA disconnector, QBB fused disconnector) | Main switch / load-break switch |
| QM / QN | Sealed fluid switching (QMB gas shut-off valve) / varying | Solenoid valves, process valves |
| SF / SJ / SK | Face / finger (SJB push-button) / movement interaction | Push-buttons, selector switches |
| TA / TB / TC | Transforming AC↔AC (TAA transformer, TAC frequency converter) / converting AC↔DC (TBA rectifier) / universal power supply (TCB DC PSU) | Control transformer, VFD, 24 V PSU |
| UA / UC | Positioning (UAB cable gland) / enclosing (UCA cubicle) | DIN rail, gland, enclosure |
| WD / WE / WG | LV energy guiding (WDB cable, WDC wire) / reference potential (WEA earthing rail) / signal guiding (WGA control cable, WGB data cable) | Power cables, PE bar, fieldbus cable |
| XD / XE / XG | LV connecting (XDA terminal) / potential connecting (XEA PE terminal, XEC FE terminal) / signal connecting (XGA socket, XGB terminal) | Terminal strips, PE terminals, connectors |

### 1.5 The reference drawing mapped to the standards

| Tag in reference | Device (folio) | Letter as drawn | IEC 81346-2:2009 | IEC 81346-2:2019 | Comment |
|---|---|---|---|---|---|
| S51 | Eaton/Holec 533356 2-pole 40 A main switch (101) | S | Q (QB) | **QB** | S means *manual signal* in 2009/2019, not power switching |
| -101F1, F55, -102F1 | MCBs (101, 102) | F | F (FC) | **FC** (FCB) | Correct letter |
| T55 | Rittal SK 3110.000 thermostat (101) | T | B (BT) | **BT** (BTB) | T means transformer in every edition: **wrong** |
| E55 | Rittal SK 3102.000 fan/heater (101) | E | E | **EB** (heater) or **GQ** (fan) | Depends on the main function |
| S56 | Door switch (101) | S | B (BG) | **BG** (BGB) | A door switch senses position |
| E56 | Cabinet lamp (101) | E | E | **EA** | Correct |
| 1X1, X1031, X1032 | Terminal strips (101, 103) | X | X | **XD**; PE strip **XE** | Correct class; prefix missing |
| -102G1 | Phoenix 2904602 24 V PSU (102) | G | G (generating) | **TB** / **TC** | 2019 classifies PSUs as transforming |
| -102F2 | Murr MICO 4.10 electronic breaker (102) | F | F | **FC** (or QAC) | — |
| -3X1 | Profibus connector (3) | X | X | **XG** | — |
| -3W1, -106W1/W2 | Profibus cable; Festo NEBU supply cables (3, 106) | W | W | **WG** / **WD** | — |
| -201U1 | Festo CPX-FB13 bus node (201) | U | K | **KE** (KEC) | U = holding in 2019 |
| -201U3..U8 | VMPA1-FB-EMG-8 electrical sub-bases (201) | U | U | **UA** or **KF** | Judgement call: carrier vs. electronic module |
| Valves 571334 | 5/2 solenoid valves (203–206) | (none) | Q (QM) | **QM** (QMB); pilot **KH** | Valves carry no tag in the drawing |
| Q0.1 … Q6.7 | PLC outputs (203–206) | — | — | — | IEC 61131-3 addresses, not RDs (Section 11) |

**Finding.** The reference uses the legacy letter list of **DIN 40719-2 / IEC 60750**: A assemblies,
B transducers, E miscellaneous, F protection, G generators/power supplies, H signalling, K relays and
contactors, Q power switching, S switches, T transformers, U converters, W cables, X terminals,
Y electrically operated mechanical devices **(unverified; author knowledge)**. Some letters do not fit
even that list (T for a thermostat). An identifier set alone cannot make projects conformant.
ElektraWatt needs selectable **identifier sets** plus a **conformance check**.

### 1.6 Rules and check-runs derived from IEC 81346

**Data model**
- Every object has an immutable internal UUID. Reference designations are *derived, displayed
  properties*. They are never the identity. This removes defect 1: renumbering cannot desynchronise
  the BOM, because the BOM is computed from the same objects.
- **Structure trees are first-class objects.** Model one tree per aspect (`==`, `=`, `++`, `+`, `-`,
  plus `%` for types). A device holds *references* to tree nodes. It does not hold copied strings.
- Title-block values such as "=P01" and "+J-E-2416" are bound to those nodes. This removes defects 3
  and 6.
- **Identifier sets** (EPLAN term; [EPLAN 2026 help](https://eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/fctdeflibdataexchangegui_k_kennbuchstaben.htm)):
  - Shipped sets: `IEC81346-2:2019`, `EN81346-2:2009`, `IEC60750-legacy`, `NFPA79`, and `Custom`.
  - Each symbol and part function definition carries a *function category*, such as
    `ProtectionOvercurrent` or `PowerSupplyAcDc`. The active set maps the category to a letter code.
    This mirrors EPLAN, where the function definition supplies the identifier.
- **Numbering schemes** are a project setting:
  - `sequential`: -FC1, -FC2 …;
  - `page-prefixed` (the reference): `-{page}{letter}{counter}` → -101F1;
  - `page-column`: e.g. -FC101.3;
  - `free`.
  - Page-prefixed tags break the 81346-1 pattern "letter code + number". Show a non-blocking
    conformance warning when such a scheme is used.

**Validation (check-runs)**

| ID | Rule | Severity | Prevents |
|---|---|---|---|
| RD-01 | Each RD parses under the active grammar | Error (strict) / Warning (lenient) | Defect 3 |
| RD-02 | Structure node names contain no prefix or separator characters (`= - + % # & : ; / .` or space). **`+J-E-2416` fails**: it parses as location `J`, then products `-E` and `-2416` | Error | Defect 3 ("+J-E-2416" vs "+JE2416") |
| RD-03 | The letter code exists in the active identifier set and matches the device's function category (e.g. a thermostat must be BT) | Warning | T55, S51, U-tags |
| RD-04 | Uniqueness of the full RD within its parent node. Uniqueness scope (project / function / location) is configurable | Error | Duplicate tags |
| RD-05 | Every device on the layout (2D/3D) refers to a schematic device object, or is flagged "layout-only" | Error | Defect 2 (T51 vs T55; S51 without prefix) |
| RD-06 | Tags are shown consistently: either fully qualified or abbreviated relative to the page's structure context, as one project setting | Info | Defect 3 (`+JE2416-X1031` vs `-101F1`) |
| RD-07 | Letters I and O must not appear in letter codes; letters reserved in 2019 must not be used as class codes | Warning | — |
| RD-08 | The renumber operation is atomic and transactional. It updates every derived output (BOM, terminal plans, labels, layouts) | — | Defect 1 |

- **Display rule.** Inside a page whose title block already shows `=P01+JE2416`, a device may be
  displayed as `-F1`. Reports, labels and exports always use the full RD. This is common practice in
  EPLAN-style output.

---

## 2. IEC 61082-1: document preparation (diagrams, cross-references)

### 2.1 What it covers

IEC 61082-1:2014 gives general presentation rules, plus specific rules for diagrams, drawings and
tables. The table of contents has a clause on cross-references, "Figure 8 – Example of a reference
grid" and "Figure 9 – Examples of the application of cross-references"
([iTeh sample](https://cdn.standards.iteh.ai/samples/19897/2d9fca205e194863a04013d515c0e149/IEC-61082-1-2014.pdf)).
An earlier draft states:
- grid rows use capital letters and columns use numbers;
- a reference on the same page is written as a slash plus the zone;
- a reference to another page is written as a slash, the page and the zone;
- cross-references must be clearly distinguishable from other designations

(2002 draft, verify against the 2014 Figure 9).

Concepts the tool must implement (**(unverified)** clause numbers, author knowledge of the 2014
text):
- **Representation of objects.** *Attached* (all parts together), *semi-attached*, and *detached*
  (scattered) representation. In the detached form, a coil is drawn on one page and its contacts on
  others. This is EPLAN's "main function / auxiliary function", and it appears on folios 201 and
  203–206: overview device -201U3 with detail "(201-E4)".
- **State of representation.** Contacts are shown in the de-energised / non-actuated state. Exceptions
  are marked.
- **Cross-references**:
  - from each detached part to the other parts of the same object;
  - **contact images** (contact tables) under the coil, listing each contact with its terminal
    designations and location;
  - **interruption points**: a connection line is interrupted, carries a designation such as a
    potential or signal name, and refers to its continuation.
- **Placement of reference designations** (left of or above the symbol) and terminal designations
  next to connection points.
- **Reference grid** according to ISO 5457, which is used as the cross-reference coordinate system.

### 2.2 Cross-reference notations ElektraWatt must produce and parse

| Style | Example | Meaning | Source |
|---|---|---|---|
| QElectroTech | `102-A1`; slave `(201-E1)` | folio-row+column. QET defaults `master_label="%f-%l%c"`, `slave_label="(%f-%l%c)"`, report label `%f-%l%c` (an example file uses `/%f.%l%c`) | [QET source](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/xrefproperties.cpp) |
| EPLAN column | `/7.8` (page 7, column 8); page only `/7` | Separator before page, separator before column | [EPLAN help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xessettingsgui_d_einstellungenprojektabbruchstellen.htm) |
| EPLAN full | `=P01+JE2416/101.H16` | Structure of the *target* page + page + grid. Shown when the target is in a different structure | EPLAN option "display full identifier structure" |
| Same page | `/H16` or `.H16` | Zone only | IEC 61082-1 draft |

**Implementation**
- A cross-reference is a **computed relation** between two graph objects (UUIDs). It is never text.
- The display string comes from a per-project format template with tokens:
  `{struct_full}`, `{struct_rel}`, `{doc_kind}`, `{page}`, `{row}`, `{col}`, `{sep_page}` (`/`),
  `{sep_col}` (`.` or `-`).
- **Import.** The QET importer reads `%f`, `%l`, `%c` templates. A strict parser rebuilds the links
  from QET's own xref data, not from the rendered text.
- **Interruption-point topology** follows EPLAN:
  - *star*: one source, many targets;
  - *chain*: 1→2, 3→4 …
  - EPLAN also allows matrix display of multiple targets, "by row/by column, n per row"
    ([EPLAN help](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/interruptionpointgui_k_darstellungabbruchstellen.htm)).
  - On folio 103, one potential fans out to many pages, so star mode with a target list is needed.

**Check-runs**

| ID | Rule |
|---|---|
| XR-01 | Every interruption point has at least one partner with the same name in the same potential/net scope. Report orphans |
| XR-02 | Contact-image consistency: the number of auxiliary contacts used ≤ the contacts available from part data; NO/NC type matches |
| XR-03 | Every main function has its auxiliary functions placed, or they are explicitly listed as "unused contacts" in the contact image |
| XR-04 | Overview ↔ detail link exists ("∞ 203-B4" on folios 201/202) for each overview representation |
| XR-05 | Grid zone of every cross-reference target is inside the page frame (prevents stale refs after frame change) |

---

## 3. IEC 61355-1 / IEC 81355-1: document kind classification (the `&` block)

**Structure.**
- The DCC consists of the prefix `&` and three letters:
  - A1: technical area, optional when all documents share one area; E = electrical engineering and
    I&C;
  - A2: main class;
  - A3: subclass

  ([Wikipedia summary](https://en.wikipedia.org/wiki/IEC_61355);
  [IEC 61355 DB](https://std.iec.ch/iec61355/iec61355.nsf/$Description?OpenForm)).
- EPLAN uses the DCC as the "document type" identifier block (`&EFS`). A counter is often appended
  (`&EFS1`).

**Known codes (2008 table, from secondary sources)**

| Code | Kind | Use in ElektraWatt | Reference folio |
|---|---|---|---|
| &EAA | Title page / cover sheet (in utility guidelines, [Stadtwerke Bayreuth](https://www.stadtwerke-bayreuth.de/fileadmin/user_upload/netz/Installateure/swbt_zulieferrichtline_v03_2018_04.pdf)) | Cover page | 1 |
| &EAB | List of documents / table of contents (AB per [de.wikipedia](https://de.wikipedia.org/wiki/DIN_EN_61355)) | TOC | 2 |
| &EFB | Flow/block diagrams (block diagram, PFD, P&ID) | Bus topology, overview | 3 |
| &EFE | Function descriptions | Function texts | — |
| &EFS | Circuitry documents (circuit diagram) | Multi-line schematics | 101–106 |
| &EMA | Connection documents (connection diagram/table): terminal diagrams, wire lists | Terminal / wire lists | missing (defect 9) |
| &EMB | Cabling or piping documents (cable diagram, cable pulling card) | Cable diagrams | missing (defect 9) |
| &EPA | Material lists | — | — |
| &EPB | Parts lists, spare parts lists, label lists | BOM, labels | 901 |
| &EL? | Location documents (L class), e.g. arrangement drawings. Exact subclass **(unverified)** | Cabinet layout | 902 |

Sources: [Wikipedia (2008 classes)](https://en.wikipedia.org/wiki/IEC_61355),
[ibKastl](https://wiki.ibkastl.de/view/Normen_f%C3%BCr_die_Schaltplanerstellung). Company
conventions vary, and the Bayreuth guideline requires client approval for extensions. For
pneumatics, the technical area letter may differ from E (for example M or another area code;
**(unverified)**).

**IEC 81355-1:2024 changes**
- The focus moves from documents to *information containers*.
- The DCC is replaced by the **ICC**, and a DCC→ICC conversion table is provided. The meaning of the
  former DCC codes is carried over "to the extent possible"
  ([iTeh/IEC](https://standards.iteh.ai/catalog/standards/iec/9634ad0a-4ba4-499b-a6e9-08e2e0d84a50/iec-81355-1-2024)).
- Whether the `&` prefix is retained could not be verified.

**Software rules**
- `DocumentKind { scheme: Dcc2008 | Icc2024 | Custom, code: String, title: LocalizedString }`, with a
  mapping table between the schemes. The mapping is data, maintained by the project, filled from
  licensed copies.
- Each page **type**, such as multi-line schematic, fluid schematic, terminal diagram or report
  page, has a default document kind. Generated report pages (TOC, BOM, terminal diagram, cable
  diagram, PLC overview) take their kind automatically. This makes the TOC (folio 2) and
  "Prev/Next" (defect 7) pure computations.
- Section separator pages, such as folio 100 "=P01+J-E-2416 – Section 1 – Page 100–199", become
  generated pages driven by structure nodes. They are never hand-drawn.

---

## 4. IEC 60617, IEC 81714 and symbol licensing

**Facts**
- IEC 60617 exists only as an online database, accessed by subscription under the IEC Webstore
  licence ([IEC](https://library.iec.ch/iec60617)).
- The IEC's copyright terms:
  - Database structures and content are protected by copyright and database rights.
  - Users may use elements within their organisation.
  - IEC must be cited as the source.
  - "Duplication of the databases or the extraction of substantial portions of them for commercial
    exploitation **or for free sharing** is prohibited without explicit, written approval"

  ([IEC copyright page](https://webstore.iec.ch/en/copyright), summarised in search results).
- IEC 81714-2:2006 specifies requirements for symbols in a reference library in computer-sensible
  form, covering metadata and connect nodes. IEC 81714-3 classifies connect nodes and networks
  ([iss.rs](https://iss.rs/en/project/show/iec:proj:8258)). ISO 81714-1 sets basic design rules for
  graphical symbols **(unverified edition, 2010)**.

**How other open-source projects handle symbols**
- **QElectroTech.**
  - The code is GPL-2.0 ([repo](https://github.com/qelectrotech/qelectrotech-source-mirror)).
  - Its element collection (more than 8,200 symbols, community drawn) is under a custom licence.
    Use, modification and integration into diagrams are free "whatever the final license of the
    diagrams". Redistribution outside a diagram requires **CC-BY 3.0**. A further clause reads:
    "Permission is not granted to use this software or any of the associated files as sample data
    for the purposes of building machine learning models"
    ([ELEMENTS.LICENSE](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/ELEMENTS.LICENSE)).
- **KiCad.** The libraries are under **CC-BY-SA 4.0 with an exception**: designs and generated files
  are free of the share-alike obligation, while redistributed library collections stay under
  CC-BY-SA ([Debian copyright file](https://tracker.debian.org/media/packages/k/kicad-symbols/copyright-10.0.4-1)).
- Both projects **draw their own symbols** in the conventional IEC style. Neither redistributes
  IEC's database files.

**Implications**
- Simple geometric symbols, such as a rectangle with a diagonal for a fuse or a circle with a cross
  for a lamp, follow conventions that are functional. Independently drawn look-alikes are what the
  whole ECAD industry ships.
- ElektraWatt should:
  1. draw its own symbol library from scratch, with no tracing of IEC database graphics;
  2. *never* copy IEC symbol names, descriptions or notes verbatim;
  3. store the IEC 60617 symbol number (e.g. "S00227") only as an optional metadata field, cited
     with "IEC 60617" as the source, in line with the IEC attribution request;
  4. pick a symbol licence that fits the main licence. CC-BY 4.0 or CC0 for the library, with an
     explicit clause that diagrams made with it are unencumbered (as in the KiCad exception).
- **Do not bundle the QET collection.**
  - CC-BY 3.0 alone would be workable.
  - The extra "no ML" restriction makes it non-standard and arguably not an open licence. It also
    conflicts with the brief's AI-assisted development.
  - The QET importer may read users' own `.qet` files, which embed their element definitions, and
    convert them. It must not ship the collection.
- **Symbol data model** (in line with IEC 81714-2 concepts):
  `Symbol { id, graphics (vector), connection_points [{id, name, position, direction, potential_role}],
  function_category, representation_type (multi-line | single-line | overview | contact-image | fluid),
  variants[A..H rotations], text_anchors, iec60617_ref: Option<String> }`.

---

## 5. ISO 1219-1/-2: fluid power symbols and circuit diagrams (folios 201–206)

**Facts**
- ISO 1219-1:2012 sets rules for drawing fluid power symbols. Amd 1:2016 corrects example 7.3.11 and
  adds basic symbol 8.4.64 ([ISO](https://www.iso.org/standard/60184.html),
  [Amd 1 sample](https://cdn.standards.iteh.ai/samples/66287/7a31a431faa1438a84a668ba6a88beb8/ISO-1219-1-2012-Amd-1-2016.pdf)).
- ISO 1219-2:2012 gives the rules for circuit diagrams. Clause 5.1 is the component identification
  code: **installation code, medium code, circuit number, component number**, each a separate
  element ([ANSI preview](https://webstore.ansi.org/preview-pages/ISO/preview_ISO+1219-2-2012.pdf)).
  - The medium code is a letter code, needed only when several media are used.
  - The 2012 edition no longer uses a letter for component function (Festo teaching material,
    summarised in search results).
  - EPLAN's basic project numbers fluid devices per ISO 1219-2 with media code, circuit number and
    item number ([EPLAN help](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/ftechnic_h_fluidbetriebsmittel_online_nummerieren.htm)).
- **Port numbering** (ISO 5599-3 convention), shown with a 5/2 solenoid valve with spring return:
  - 1 supply; 2 and 4 working ports; 3 and 5 exhausts;
  - 12 and 14 pilot/control ports: "14" means the signal connects 1→4, "12" connects 1→2;
  - 82/84 pilot exhausts; X external pilot supply

  ([Tubes International](https://www.tubes-international.com/?p=138822); ISO 5599-3:1990 is listed
  as withdrawn but its scheme is universal).
- Valve function notation "n/m": n ports / m switching positions, e.g. 5/2 or 5/3 with
  closed/exhausted/pressurised centre.
- On folio 203 the reference shows 5/2 valves with ports 2/4 and "3 15" exhausts. Each valve carries a
  PLC address (Q0.1, Q0.3 …) and a process tag (TA05-V-15) as function text.

**Software rules**
- Fluid symbols are *the same graph engine* as electrical symbols, with typed connection points:
  `PortRole { Supply1, Work2, Work4, Exhaust3, Exhaust5, Pilot12, Pilot14, PilotExhaust82/84, ExtPilotX }`
  and a medium (`Pneumatic | Hydraulic | Vacuum | Lubrication`).
- Lines are typed: working line (solid), pilot/control line (dashed), drain/leakage line (dashed).
  Line style follows the type, not free styling **(unverified line-type assignments for the 2012
  edition)**.
- A valve is modelled as `positions[]`, each with a port connectivity matrix, plus actuators per side
  (solenoid, pilot, spring, manual override). The solenoid actuator is an *electrical* function of the
  same device. It carries terminals and a PLC channel and cross-references to the electrical page.
  This links folio 106 (CPX power) to folios 203–206.
- Identification: support **both** ISO 1219-2 codes (`{installation}-{medium}{circuit}.{component}`;
  the separator format is configurable, **(unverified)**) and IEC 81346 RDs, with QM/MM/KH letter
  codes. Mixed projects (folio 3 + folio 201) usually use 81346 throughout.
- Check-runs:
  - FL-01: unconnected working ports, unless plugged / marked "blanked";
  - FL-02: the number of solenoids per valve matches the PLC channels assigned (a single-solenoid
    valve gets one channel);
  - FL-03: manifold slot occupancy matches the configurator order code ("PART DATA" box on folio
    201);
  - FL-04: spare slots are reported (the reference marks "Spare").

---

## 6. Colours, conductor and terminal identification: IEC 60757, IEC 60445, IEC 60947

### 6.1 IEC 60757:2021

- Letter codes for colours in technical documentation and markings.
- Edition 2 restricts codes to letter codes, adds a subclause on alternating colours of the same item
  (e.g. GNYE), and adds Annex A with RGB examples ([IEC](https://webstore.iec.ch/publication/66266)).
  It does not specify how colours are represented visually.
- The IEC CDD value list holds BK, BN, RD, OG, YE, GN, BU, VT, GY, WH, PK, GD, TQ, SR, plus GNYE and
  two-colour combinations such as BKBN
  ([Electropedia CDD](https://www.electropedia.org/cdd/iec61360/iec61360.nsf/EnumerationsAllVersions/0112-2---61360_4%23ASA266)).
- GNYE is reserved for the purpose defined in IEC 60445 (protective conductors). No other GN/YE
  combinations are allowed. Multi-colour items concatenate codes in table order.

**Reference finding.** Folio 1 uses **DBU** (dark blue). DBU is *not* an IEC 60757 code. Folio 1
also writes "Yellow / 4 Green". ElektraWatt therefore needs:
- `ColourCode { iec: Option<Iec60757>, custom: Option<String>, rgb_display: Rgb, shade: Option<Light|Dark> }`.
- DBU becomes `{iec: BU, shade: Dark, custom: "DBU"}`. Exports to machines (Komax) and labels emit
  either the custom code or the IEC base code, per a profile setting.
- A check warns about non-IEC codes, and about GNYE used on anything other than PE/PEL/PEM/PEN.

### 6.2 IEC 60445:2021 + AMD1:2026

Edition 7 aligns its definitions with IEC 60050 and turns some colour recommendations into
**requirements**. It also adds marking of protective terminals for multiple supply inputs
([iTeh](https://standards.iteh.ai/catalog/standards/iec/7275345f-6850-44bf-a57f-02870c0e23d2/iec-60445-2021)).
Key rules:
- Neutral or mid-point conductor: **BLUE**.
- Protective conductor: **GREEN-AND-YELLOW**. PEL/PEM/PEN use GNYE with blue marking (or the line
  colour) at terminations.
- Functional earthing conductor: preferred colour **PINK** (search summary of the 2021 text).
- DC line conductors: **RED (L+)** and **WHITE (L−)**, introduced in Ed. 6 (2017) and kept in
  Ed. 7.1.
- Permitted colours: BK, BN, RD, OG, GN, YE, BU, VT, GY, WH, PK, TQ.
- AC line conductors L1/L2/L3: brown/black/grey **(unverified; common CENELEC HD 308 practice)**.
- Alphanumeric designations: L1, L2, L3, N, PE, PEN, PEL, PEM, FE, FB, L+, L−, M; equipment terminals
  U, V, W **(unverified for the full list; author knowledge)**.

**Conflict to model explicitly.**
- EN 60204-1 13.2.4 *recommends* BLACK for AC and DC power circuits, RED for AC control, BLUE for DC
  control, and ORANGE for excepted circuits (Section 7).
- IEC 60445 *requires* RED/WHITE for DC line conductors "where colour is used".
- The reference uses DBU (+24 V) and WH (0 V): a company profile that combines both.
- ElektraWatt should ship **colour profiles** as data: `IEC60445-installation`,
  `EN60204-1-machine`, `NFPA79`, `Company`. The project selects one, and the potential types map to
  colours through it.

### 6.3 Device terminal marking (contact images)

The marking rules come from IEC 60947-1 Annex L / IEC 60947-5-1, formerly EN 50005/50011/50012.
Contactor terminal marking follows IEC 60445, IEC 60947-1 and EN 50005
([ABB catalogue](https://www.rgbautomatyka.pl/it/p/file/dd0f3848e1f746a66561b2e101db0924/ABB_CA5.01_14_3_4.pdf)).

Rules **(author knowledge, verify against IEC 60947-1 Annex L)**:
- Main contacts: 1/L1–2/T1, 3/L2–4/T2, 5/L3–6/T3.
- Coils: A1–A2 (A3 for tapped coils).
- Auxiliary contacts carry two digits:
  - **sequence digit** (1, 2, 3 …), then **function digit**;
  - function digit 1–2 = NC, 3–4 = NO, 5–6 = NC special (time-delayed), 7–8 = NO special;
  - changeover = 1-2-4.
  - Examples: 13-14 NO, 21-22 NC.
- Overload relay: 95-96 NC, 97-98 NO.
- On folio 102 the MICO signal contact 13-14 follows this rule.

**Software rule**
- The part database stores each contact as
  `{terminals: [String;2..3], kind: NO|NC|CO, special: bool, sequence: u8}`.
- The contact image and contact-capacity check (XR-02) are generated from it.
- The "function digit vs symbol kind" consistency is checked when a contact symbol is assigned
  terminals.

---

## 7. EN 60204-1 / IEC 60204-1:2016+AMD1:2021: electrical equipment of machines

**Status.**
- EN 60204-1:2018 + **A1:2025** incorporates IEC AMD1:2021. It was implemented in the UK on
  2025-04-30, and the 2018 base edition is withdrawn on 2028-04-30
  ([NBS/BSI](https://www.thenbs.com/publicationindex/documents/details?Pub=BSI&DocId=347000)).
- The amendment is mostly editorial and updates references. The second paragraph of 4.4.2 (EMC)
  moves to informative Annex H.2, and bonding text in 8.2.1/8.2.2 changes: the phrase about
  "all conductive structural parts" is removed
  ([IBF](https://www.ibf-solutions.com/en/news-and-knowledge/technical-papers-and-news-on-ce-marking/the-new-en-60204-12018/a12025),
  [GT Engineering](https://www.gt-engineering.it/en/technical-standards/en-iec-standards/en-iec-60204-1-safety-of-machinery-electrical-equipment-of-machines/amendment-iec-60204-1-2021-gt-engineering-english/)).
- EN 60204-1 is a harmonised standard under the Machinery Directive. The Machinery Regulation (EU)
  2023/1230 applies from 2027-01-20
  ([EU-OSHA](https://osha.europa.eu/en/legislation/directive/regulation-20231230eu-machinery)).

**Rules that become rule-pack checks** (clause numbers follow the 2016/2018 text and are **(verify)**
where marked):

| ID | Clause | Rule as implemented | Data needed |
|---|---|---|---|
| M-01 | 5.1/5.2 | Incoming supply terminals are designated L1/L2/L3/N. The PE terminal is marked "PE" | Terminal designations; potential types |
| M-02 | 5.3 | Exactly one supply-disconnecting device per incoming supply. It has the "supply disconnect" role and is lockable in OFF. Its RD class is Q/QB, not S (folio 101 S51) | Device role flags from part data |
| M-03 | 5.3.5 / 13.2.4 | Excepted circuits upstream of the main switch, or supplied from outside, are identified ORANGE (or by marking) | Net topology relative to the disconnector |
| M-04 | 8.2 | Every exposed conductive part is in the protective bonding circuit. PE conductor cross-section vs. phase conductor: S ≤ 16 mm² → S; 16 < S ≤ 35 → 16; S > 35 → S/2 (copper; table in 8.2 / IEC 60364-5-54, **(verify)**) | Net graph; wire cross-sections |
| M-05 | 9.1.1 | AC control circuits are fed via a transformer with separate windings, with a stated exception | Supply topology |
| M-06 | 9.1.2 | Nominal control voltage *should* not exceed 230 V AC (50 Hz), 277 V AC (60 Hz) or 220 V DC ([IET forum quoting 9.1.2](https://engx.theiet.org/f/wiring-and-regulations/31616/230v-control-circuit/150910)) | Potential voltage attribute |
| M-07 | 7.2 / 12.4 | Overcurrent coordination: I_b ≤ I_n ≤ I_z for each conductor, using the current-carrying capacity table (Table 6: 40 °C, PVC, methods B1/B2/C/E) and minimum cross-sections (Table 5) **(verify values)** | Wire cross-section, installation method, protective device In |
| M-08 | 13.1 | Number of conductors per terminal ≤ the terminal's rating (twin terminals PT 2.5-TWIN on folio 103) | Part data: max conductors per connection point |
| M-09 | 13.2.2 | GNYE only for protective conductors, continuous over the full length | Potential type |
| M-10 | 13.2.3 | Neutral is (light) blue. Blue is not used for other conductors where confusion is possible. Warn when BU is used for non-N conductors in a project that has an N potential | Colour profile |
| M-11 | 13.2.4 | Recommended colours: BK AC/DC power; RD AC control; BU DC control; OG excepted circuits ([quoted on PLCtalk](https://www.plctalk.net/threads/wire-colour-code.91143/page-2)) | Colour profile |
| M-12 | 13.2.1 | Each conductor is identifiable at each termination in line with the documentation. This needs wire numbers or connection-based labelling (defect 9) | Wire numbering scheme |
| M-13 | 16.4 | Nameplate data on the enclosure: supplier, type, serial, main document number, rated voltage, phases, frequency, full-load current, short-circuit rating **(verify list)** | Project properties (folio 1 already has supply, control current, consumption) |
| M-14 | 16.5 | Enclosures, assemblies, control devices and components carry the *same* RD as the documentation | RD single source of truth (Section 1) |
| M-15 | 17 + Annex I | Documentation set (see below) | Report generator |

**Clause 17 / Annex I: documentation the tool must generate or support**
- Clause 17 requires information for identification, transport, installation, use, maintenance,
  decommissioning and disposal of the electrical equipment.
- Annex I (new in 2016) maps documentation types to standards: IEC 81346-1 for structuring,
  IEC 62023 for structuring technical documents, IEC 62027 for parts lists and document lists,
  plus IEC 61082-1 and IEC 61355/81355 ([Certifico summary](https://certifico.com/id/6667)).
- The guide CEI 44-14 structures the deliverables as:
  - general information;
  - installation diagram;
  - block/function diagrams;
  - circuit diagrams;
  - operation manual;
  - maintenance manual.
- **Minimum generated set:** cover page with supply data, TOC, overview/block diagrams, circuit
  diagrams, terminal diagrams, cable diagrams, connection (wire) list, parts list, layout drawing,
  PLC I/O overview, verification report template (clause 18 tests: protective bonding continuity,
  insulation resistance, voltage tests, functional tests).

**Supply notation.** "1 x 230 VAC +N +PE, 50hz" on folio 1 should be generated from structured data as
`1/N/PE AC 230 V 50 Hz` (IEC notation style, **(unverified source clause)**). The structured data is
`Supply { phases: 1|3, neutral: bool, pe: bool, system: TN-S|TN-C-S|TT|IT, un: V, f: Hz, ik_max: kA }`.

---

## 8. IEC 61439-1/-2:2020: low-voltage switchgear assemblies

### 8.1 Concepts and ratings ElektraWatt must model

The rated values of the assembly are:
- U_n: rated voltage;
- U_i: insulation voltage;
- U_imp: impulse withstand voltage;
- InA: rated current of the assembly;
- Inc: rated current of each circuit;
- **Ing: group rated current** (new in 2020);
- **RDF: rated diversity factor**;
- Icw: rated short-time withstand current;
- Ipk: rated peak withstand current;
- Icc: rated conditional short-circuit current;
- frequency, IP (IEC 60529), IK (IEC 62262), form of internal separation, protection class I/II
  (new), and DC ratings (new)

([KEMA white paper](https://www.cesi.it/app/uploads/2020/11/KEMA-Labs-LV-White-Paper-IEC-61439-1-2020-WHATS-NEW.pdf)).

RDF is the per-unit value of rated current to which outgoing circuits can be loaded continuously and
simultaneously, considering mutual heating. Annex E gives examples.

**Design verification** (clause 10) can be done by test, calculation/assessment or comparison with a
reference design. Items:
- 10.2 strength of materials;
- 10.3 IP;
- 10.4 clearances and creepage;
- 10.5 protection against electric shock / protective circuit;
- 10.6 incorporation of devices;
- 10.7 internal circuits;
- 10.8 terminals for external conductors;
- 10.9 dielectric properties;
- **10.10 temperature rise**;
- **10.11 short-circuit withstand**;
- 10.12 EMC;
- 10.13 mechanical operation.

Clause 11 is **routine verification** of every manufactured assembly, and Annex D lists the
design-verification methods. The clause mapping is taken from Ed. 2 material; verify for Ed. 3.

### 8.2 Temperature-rise verification by calculation

Secondary sources describe two routes in 61439-1:2020
([BEAMA guide](https://www.beama.org.uk/asset/15EC326C-E6D9-477B-B53D2F95EBFD965E/),
[AmpereSoft](https://amperesoft.net/en/toolsystem/temperaturecalculator)):

1. **10.10.4.2, single compartment, InA ≤ 630 A.** The total effective power loss of all built-in
   components and conductors must not exceed the enclosure's proven heat-dissipation capability
   (enclosure-manufacturer data). Conditions:
   - losses roughly evenly distributed;
   - components loaded ≤ 80 % of their free-air rating;
   - air circulation not impeded.
2. **10.10.4.3, multiple compartments, InA ≤ 1600 A.** Calculation per **IEC TR 60890:2022**, which
   applies to enclosures without forced ventilation. Components are de-rated to ≤ 80 %, and
   horizontal partitions must have adequate open area.

**IEC TR 60890 method**
- The effective cooling surface is **Ae = Σ (A0 × b)**, where *b* is a surface factor
  ([TR 60890 sample](https://cdn.standards.iteh.ai/samples/103729/ac9f2264127744829ed89c229d8fa150/IEC-TR-60890-2022.pdf)).
  Secondary reproductions give:
  - b = 1.4 exposed top;
  - b = 0.7 covered top;
  - b = 0.9 exposed sides;
  - b = 0.5 covered sides **(verify)**.
- The enclosure constant *k* and the temperature distribution factor *c* are read from figures. The
  figures are split by Ae > 1.25 m² or ≤ 1.25 m² and by ventilation openings.
- The model is:
  - Δt0.5 = d·k·P^x at mid-height;
  - Δt1.0 = c·Δt0.5 at the top;
  - x = 0.715 for unventilated enclosures with Ae > 1.25 m², 0.804 for Ae ≤ 1.25 m²;
  - d is a factor for horizontal partitions

  **(x and d values unverified; author recollection of ABB TAP 11,
  [ABB](https://library.e.abb.com/public/353080ba2e0e617e48257a700024a13b/1SDC007110G0201.pdf))**.
- The k/c curves are figures in a copyrighted TR. ElektraWatt must either digitise them under a
  licence or require the user/manufacturer to provide them as data. The **method** itself can be
  implemented.

**Power-loss model per device.** P_eff = P_n · (I_b / I_n)². Here P_n is the rated power loss from
part data, and I_b is the computed load current times the RDF/simultaneity. Conductor losses follow
the IEC 61439-1 Annex H tables, which give operating current and power loss of conductors **(verify
table contents)**. Busbars are added on top.

**Climate sizing (Rittal Therm-like)**
- Natural dissipation: Q_S = k · A · ΔT, with k = 5.5 W/(m²·K) for sheet steel (Rittal default) and A
  the *effective* surface area, which depends on the installation type
  ([Rittal Therm manual](https://electrical-engineering-portal.com/academy/wp-content/uploads/2021/08/Rittal-Therm-Manual.pdf),
  [Rittal SK](https://theengineer.markallengroup.com/production/content/uploads/2013/12/Systemklima_gb_lr.pdf)).
- Required cooling: Q_E = Q_V − k·A·(T_i − T_u). Rittal's example: 1500 W − 5.072 m² × 5.5 × (35 − 40)
  = 1639 W.
- Filter fan: **V = f · Q_V / ΔT** [m³/h]. The altitude factor f is 3.1 (0–100 m), 3.2 (100–250 m),
  3.3 (250–500 m), 3.4 (500–750 m), 3.5 (750–1000 m) m³·K/Wh
  ([Rittal catalogue](https://www.rittal.de/downloads/eBook/TSH/EN/Climate_control/pubData/SEO/Page_7.html)).
- Heater: Q_H = k·A·(T_i,min − T_u,min).
- Effective surface by installation type (IEC 60890 / Rittal handbook, seven cases), e.g.:
  - free-standing single: A = 1.8·H·(W+D) + 1.4·W·D;
  - wall-mounted single: A = 1.4·W·(H+D) + 1.8·D·H

  **(unverified coefficients; author recollection)**.

**Worked example (reference project, illustrative)**
- Assumptions: NSYPLA1074G enclosure taken as H 1.0 × W 0.75 × D 0.42 m **(verify dimensions)**;
  wall-mounted; Q_V = 60 W.
- A = 1.4·0.75·1.42 + 1.8·0.42·1.0 = 2.25 m².
- With sheet steel (k = 5.5): ΔT ≈ 60 / 12.4 ≈ 4.9 K.
- NSYPLA is polyester, so k is lower; take k from the manufacturer. ΔT then rises accordingly.
- This shows the data ElektraWatt needs: P_n per part, enclosure k and material, installation type,
  and ambient conditions.

### 8.3 Short-circuit withstand

**Exemptions (10.11.2).** No verification is needed for:
- assemblies with Icw or Icc ≤ **10 kA rms**;
- assemblies protected by current-limiting devices with cut-off current ≤ **17 kA** at the maximum
  prospective current at the incoming terminals;
- auxiliary circuits fed by control transformers ≤ **10 kVA** (secondary ≥ 110 V) or ≤ **1.6 kVA**
  (< 110 V) with u_k ≥ 4 %

([Schneider community, quoting the standard](https://community.se.com/t5/Power-Distribution-and-Digital/Circuits-of-ASSEMBLIES-which-are-exempted-from-the-verification/td-p/169115)).

Otherwise use comparison with a reference design (checklist table) or test.

The peak factor n (Ipk/Icw by current range) is Table 7 of the standard. Typical values are 1.5,
1.7, 2.0, 2.1 and 2.2 **(unverified)**.

**ElektraWatt rule.** Store `ik_max` at the supply point. Check each protective device and each
component SCCR/Icw against the prospective current at its location. For the IEC route this is a
simple upstream-current propagation (v1.x). Full IEC 60909 impedance calculation comes later.

### 8.4 Reports IEC 61439 implies

1. **Design verification record**: the Annex D list with method and evidence per item. Thermal
   (8.2) and short-circuit (8.3) are computed. Others are attachments.
2. **Routine verification checklist** per serial number (clause 11).
3. **Assembly documentation and marking**: manufacturer, type designation/ID, date of manufacture,
   "IEC 61439-2", ratings **(verify list)**.
4. **Power-loss/thermal report**: per device P_n, I_b, P_eff, sum Q_V, enclosure data, ΔT,
   recommended climate device.

---

## 9. North America: UL 508A, NFPA 79, CSA C22.2 No. 286 (expected "Later")

**UL 508A (3rd ed. 2018).** Supplement **SB4** is the SCCR method:
- SB4.2: rate each power-circuit component. Transformers, reactors, CTs, dry capacitors, resistors,
  one-port SPDs and voltmeters are exempt.
- SB4.3: modify for current-limiting devices. Branch components may be raised to the OCPD's
  interrupting rating if their SCCR ≥ the OCPD let-through.
- SB4.4: the overall panel rating is the lowest rating.
- NEC 409 references SB4. NEC 409.22(B) requires documenting the available fault current and the
  calculation date

([UL summary](https://www.ul.com/sites/default/files/2020-06/UL508A-Summary-of-Requirements-3rd-Edition_August%2017_2018_Rev1.pdf),
[UL SCCR article](https://ul.com/resources/determining-sccr-industrial-control-panels-and-industrial-machinery),
[Rockwell](https://literature.rockwellautomation.com/idc/groups/literature/documents/at/sccr-at002_-en-p.pdf)).

*Software:* an SCCR calculator over the power-circuit graph, with part attributes `sccr_ka`,
`current_limiting: {let_through_curve}` and `interrupting_rating_ka`.

**NFPA 79 (2024).** Colour rules (secondary source, **verify**):
- BK ungrounded AC/DC power;
- RD ungrounded AC control;
- BU ungrounded DC control;
- OG interlock from an external source;
- WH/GY grounded conductor;
- GN or GNYE equipment ground.

Device designations follow NFPA 79's annex list, e.g. CR, M, PB, LS, CB, FU, TB **(unverified)**.
This becomes the `NFPA79` identifier set plus colour profile.

**CSA C22.2 No. 286:23.** The third edition expands the SCCR determination criteria and revises the
SCCR markings. It covers panels ≤ 1500 V and is effective 2025-01-15
([SCC notice](https://scc-ccn.ca/standards/notices-of-intent/csa-group/industrial-control-panels-and-assemblies-1)).

**AWG vs mm²**
- Store the cross-section as `Section { value: f64, unit: Mm2 | Awg(i8) | Kcmil(u16) }`. Keep the
  user's original unit, and never round-trip through floats for display.
- Conversion: d_n = 0.127 mm × 92^((36 − n)/39). A = π·d²/4.
- Common values (mm²) **(computed)**:

  | AWG | mm² |
  |---|---|
  | 20 | 0.519 |
  | 18 | 0.823 |
  | 16 | 1.31 |
  | 14 | 2.08 |
  | 12 | 3.31 |
  | 10 | 5.26 |
  | 8 | 8.37 |
  | 6 | 13.3 |
  | 4 | 21.2 |
  | 2 | 33.6 |
  | 1/0 | 53.5 |
  | 2/0 | 67.4 |
  | 4/0 | 107 |

  The nearest IEC 60228 metric sizes are 0.5, 0.75, 1, 1.5, 2.5, 4, 6, 10, 16, 25, 35, 50, 70, 95,
  120 … *AWG and mm² are not interchangeable for compliance.* Show both, and validate against the
  active standard's own table.

---

## 10. Data exchange and classification: IEC 62424/62714, ECLASS, ETIM, IEC 61360/CDD

**AutomationML**
- **IEC 62424:2016** (Ed. 2) contains **CAEX 3.0**. IEC 62714-1:2018 (AML Ed. 2) is based on it, and
  is upward compatible with AML 2.0 / IEC 62714-1:2014
  ([AutomationML](https://www.automationml.org/news/automationml-standard-part-1-edition-2-has-been-published/)).
- Further parts: -2:2015 role class libraries; -3 geometry/kinematics via COLLADA **(unverified
  edition)**; -4:2020 logic; -5:2022 communication.
- **AR APC** (Application Recommendation Automation Project Configuration) is the profile used for
  ECAD↔PLC hardware configuration exchange:
  - The AML site lists the AR APC library at 1.4.0, plus ARE extensions for Profibus, Ethernet and
    IO-Link ([AML specs](https://www.automationml.org/about-automationml/specifications/)).
  - EPLAN 2022 supports AR APC **1.2.0**, and drive exchange works only with the 1.2.0 export
    ([EPLAN 2022 notes](https://eplan.help/en-us/infoportal/content/Plattform/2022/Content/htm/news_p_sps_datenaustauschaml120_281595.htm)).
  - Siemens implements AR APC in TIA Portal (AML conference 2026 abstract).
  - IEC 62424 also standardises PCE request representation in P&IDs. The process tags on folio 203
    (TA05-V-15) are such plant tags, carried as function text.
- *Software:* AML I/O is CAEX XML.
  - Map ElektraWatt PLC cards, racks, channels and bus topology (folio 3) to AR APC role classes.
  - Round-trip test against EPLAN/TIA exports, version-pinned to AR APC 1.2.0 *and* 1.4.0.

**ECLASS**
- Release 15.0 (BASIC: CSV 8.1 / XML 5.0; ADVANCED: XML 5.0). Use requires a licence or membership.
  Demo versions and Asset.xml sub-models are free
  ([ECLASS](https://eclass.eu/en/eclass-standard/licenses/language-versions),
  [FAQ](https://eclass.eu/en/faq)).
- *ElektraWatt cannot ship ECLASS data.* It can store ECLASS class IRDIs and property IRDIs that
  users, or EDZ/Data Portal imports, provide.

**ETIM**
- ETIM 10.0 was published 2024-12-05 (IXF XML, plus Excel/CSV).
- The model and the MC (Modelling Class) extension are under the **Open Data Commons Attribution
  License**, free for everyone. ETIM asks that only one maintained copy exist
  ([ETIM license](https://www.etim-international.com/?p=1811),
  [incony](https://www.incony.de/en/news/2024/etim-10/)).
- *ElektraWatt may bundle ETIM English (attribution required).* ETIM is the most practical open
  classification for part categories and feature keys (e.g. EC000042 "Miniature circuit breaker",
  **(example code unverified)**).

**IEC 61360-1:2017 / IEC CDD**
- Identifiers are IRDIs of the form `0112/2///61360_4#AAE867#001`: RAI # item code # version
  ([Wikipedia](https://en.wikipedia.org/wiki/IEC_Common_Data_Dictionary)).
- The CDD EULA declares "free attributes" free of IEC copyright claims. It requires citing IEC, and
  forbids redistributing the whole or a significant portion
  ([CDD EULA](https://cdd.iec.ch/cdd/iec61360/iec61360.nsf/License?openPage)).
- IEC 62683-1 builds LV switchgear product properties on the CDD **(unverified edition)**. This is
  where P_n, Icw and similar thermal/short-circuit properties come from.

**Software rule.** Part properties are keyed by `PropertyId { scheme: Etim | Eclass | IecCdd | Native, id }`,
with units from the dictionary. The thermal (8.2), SCCR (9) and contact (6.3) checks read *named
native properties*, mapped from whichever dictionary the part came from.

---

## 11. IEC 61131-3 and PLC addressing

**IEC 61131-3 Ed. 4 (2025).** IL removed; UTF-8 strings added; Annex B lists added, removed and
deprecated features ([Wikipedia](https://en.wikipedia.org/wiki/IEC_61131-3)).

**Directly represented variables**
- Form: `%` + location (`I` input, `Q` output, `M` memory) + size (`X` bit, may be omitted; `B` byte;
  `W` word; `D` double word; `L` long word) + a hierarchical address with `.`.
- Examples: `%IX0.0`, `%QX0.1`, `%IW64`, `%MD10`; `%I*`/`%Q*` for partly specified addresses in
  VAR_CONFIG.
- Whether Ed. 4 changed this syntax is **(unverified)**.

**Vendor notations**

| Platform | Bit out | Word in | Notes |
|---|---|---|---|
| IEC 61131-3 / CODESYS | `%QX0.1` | `%IW64` | — |
| Siemens S7 (TIA Portal, international mnemonics) | `Q0.1` (shown `%Q0.1`) | `IW64` / `%IW64` | `:P` suffix = direct peripheral access |
| Siemens S7 (German mnemonics) | `A0.1` | `EW64` | E = Eingang, A = Ausgang. Must be a project setting |
| Rockwell Logix | `Local:2:O.Data.1` | `Local:1:I.Ch0Data` | Tag-based; no absolute bit addresses |
| Beckhoff TwinCAT | `%QX0.1` via `AT` | — | Linked by the I/O configuration |

The reference (folios 203–206) uses Siemens international notation, **Q0.1 … Q6.7**, on a Festo CPX
Profibus node (folio 3).

**Rules / data model**
- `PlcAddress { area: I|Q|M|PI|PQ, size: Bit|Byte|Word|DWord|LWord, byte: u32, bit: Option<u8 0..=7>, notation: Iec|S7Intl|S7De|Rockwell(tag) }`.
- Parse every notation and render in the project's notation.
- Addresses are **assigned per channel of a PLC card/module**. They are never typed onto symbols.
  `PlcCard { start_address, channels[{index, kind: DI|DO|AI|AO|..., address, symbol_name, function_text}] }`.
- Folio 203 shows only odd output bits (Q0.1, Q0.3 …), so module address maps must be modelled, e.g.
  two bits per valve slot on the CPX/VMPA. Add `ModuleAddressMap` in part data.

**Check-runs**

| ID | Rule |
|---|---|
| PLC-01 | Duplicate address |
| PLC-02 | Address outside the module's range |
| PLC-03 | Bit index > 7 |
| PLC-04 | Channel kind ≠ connected function (e.g. a DO wired to a sensor) |
| PLC-05 | Symbol name not a valid IEC 61131-3 identifier |
| PLC-06 | Unassigned spare channels listed in the PLC overview (defect 9: no PLC overview) |

**Exports**
- AR APC AML (Section 10).
- CSV / Excel tag tables in TIA Portal "PLC tags" layout (name, data type, address, comment).
- CODESYS / PLCopen XML symbol lists **(format details belong to the interop research)**.

---

## 12. Document deliverables: PDF/A, ISO 7200 title block, ISO 5457 sheets

### 12.1 PDF/A (ISO 19005)

| Part | Base | Notes |
|---|---|---|
| PDF/A-1 (2005) | PDF 1.4 | No transparency, no layers |
| PDF/A-2 (2011) | PDF 1.7 | Adds transparency, layers (optional content), JPEG2000, embedded PDF/A files |
| PDF/A-3 (2012) | PDF 1.7 | Any attachment type (basis of ZUGFeRD/Factur-X) |
| PDF/A-4 (2020) | PDF 2.0 | No a/b/u levels; **4f** = with attachments; **4e** = engineering (3D, rich media; successor of PDF/E). Encryption prohibited |

Sources: [PDF Association](https://pdfa.org/resource/iso-19005-pdfa),
[PDFlib](https://pdflib.com/pdf-knowledge-base/pdfa/the-pdfa-standards). Validator:
**veraPDF**, open source, maintained by the PDF Association and the Open Preservation Foundation.

**Recommendation**
- Default to **PDF/A-2b** for maximum consumer compatibility, with PDF/A-4 as an option.
- Use **PDF/A-3b or PDF/A-4f** for "drawing + embedded native project / AML / BOM CSV" deliverables.
- Use PDF/A-4e for an optional 3D cabinet view.
- Embed fonts; use real text (searchable tags); add PDF bookmarks per page and per structure node;
  make cross-references *internal links*.
- Rust: **krilla** (MIT/Apache-2.0) supports validated export modes PDF/A-1…A-4 and PDF/UA-1, PDF
  1.4–2.0; MSRV 1.92 ([krilla](https://github.com/LaurenzV/krilla)).
- CI step: run veraPDF on golden outputs.

### 12.2 ISO 7200:2004 title block fields

ISO 7200 has 19 fields in three groups, 8 of them mandatory
([draftsperson](https://blog.draftsperson.net/drawing-sheet-layout-metric/)).

Identifying fields per the EN ISO 7200 table, with *recommended* lengths:

| Field | Length | M/O |
|---|---|---|
| Legal owner | — | M |
| Identification number | 16 | M |
| Revision index | 2 | O |
| Date of issue | 10 | M |
| Segment/sheet number | 4 | M |
| Number of segments/sheets | 4 | O |
| Language code | 4 | O |

Further fields **(lengths unverified)**:

| Field | M/O |
|---|---|
| Title | M |
| Supplementary title | O |
| Responsible department | O |
| Technical reference | O |
| Approval person | M |
| Creator | M |
| Document type | M |
| Classification / key words | O |
| Document status | O |
| Page number | O |
| Number of pages | O |
| Paper size | O |

The title block must not be wider than 180 mm **(unverified)**.

**Mapping to the reference title block**

| Reference field | Mapping |
|---|---|
| The pilot user | Legal owner |
| Drawing <drawing id> | Identification number |
| Revision 1 / V1.0 / V1 / V2.0 | Revision index. Defect 5: the 2-character recommendation suggests a managed `rev: "02"` style |
| Creation / Saved | Date of issue / edit date, ISO 8601 `YYYY-MM-DD` instead of `03-03-2025` |
| Page / Prev / Next | Sheet number + navigation, computed |
| Author / Checked | Creator / Approval person |
| Description | Title |
| Page title | Supplementary title |
| Confidentiality notice | Legal-owner notice |

**Rule.** Every title-block field is a *binding* to a project, page or revision property. Overriding
it per page is a deliberate, flagged action, so drift becomes visible (defect 6).

**Revision model.** `Revision { index, date, author, approver, description, affected_pages[] }`.
Pages derive "current revision" from it. A revision cloud/marker per change is optional.

### 12.3 ISO 5457 sheets and frames

- Sizes per ISO 216: A0 841×1189; A1 594×841; A2 420×594; A3 297×420; A4 210×297 mm. A4 may be
  landscape or portrait since Amd 1:2010.
- The standard covers borders and frame, centring marks and a grid reference system.
- Grid fields are 50 mm long, counted from the centring marks. A4 has 6×4 fields and A3 has 8×6.
  Capital letters run down the sides, omitting I and O; numbers run along the top and bottom
  ([CADSetterOut](https://cadsetterout.com/drawing-standards/grid-reference-frame/), secondary).
- Borders: 20 mm filing margin on the left, 10 mm elsewhere **(unverified)**.

**Reference finding.**
- The reference uses QElectroTech's own frame on A4 landscape: 17 columns numbered 0–16 and 8 rows
  A–H. This is far finer than ISO 5457's A4 grid. QET example files define `cols="17" colsize="60"
  rows="8" rowsize="80"`
  ([QET examples](https://github.com/qelectrotech/qelectrotech-source-mirror/tree/master/examples)).
- EPLAN-style schematics typically use 10 numbered columns on A3, often without rows.
- IEC 61082-1 permits this kind of grid as long as it is consistent.

ElektraWatt frames are therefore parametric:

`Frame { sheet: IsoSize|Custom, orientation, margins, columns: {count, start_index, labels}, rows: {count, labels: skip I/O}, title_block: TemplateRef }`.

Shipped templates: an ISO 5457 frame and a "QET-compatible 17×8, start 0" frame, so imported
projects keep their cross-reference coordinates (102-A1 stays 102-A1).

---

## 13. Other standards worth wiring in (brief)

| Standard | Use in ElektraWatt |
|---|---|
| IEC 61666:2010 (identification of terminals within a system) | Terminal designation syntax `RD:terminal`; terminal diagrams |
| IEC 62491:2008 (labelling of cables and cores) | Cable/core label content for labels and Komax export |
| IEC 61175-1:2015 (designations for signals) | Signal names stay stable across media changes ([AFNOR](https://www.boutique.afnor.org/en-gb/standard/iec-6117512015/industrial-systems-installations-and-equipment-and-industrial-products-desi/xs126921/249548)). Model potentials/signals with stable IDs |
| IEC 62027:2011 (object lists incl. parts lists) | BOM content and structure: aggregated quantity column, fixing defect 4 (+JE2416-X1031 listed twice, no quantity) |
| IEC 62023:2011 (structuring of technical information) | Main document / document list structure |
| IEC 60848 (GRAFCET) | Optional function charts (QET supports GRAFCET) |
| ISO 14617 | Graphical symbols for process/mechanical diagrams, harmonised with IEC 60617 |
| IEC 60228 | Conductor classes (class 5 flexible) and nominal sections |
| IEC 60947-7-1/-2 | Terminal block ratings (cross-section range, conductors per clamp) |
| DIN 46228-1/-4 | Wire-end ferrules and their colour code (DIN vs. French scheme) for manufacturing output **(details unverified)** |
| IEC 60529 / IEC 62262 | IP / IK attributes of enclosures, checked against the assembly's rated IP |

---

## 14. Copyright: what can be shipped

| Item | Ship in the OSS repo? | How |
|---|---|---|
| RDS grammar, prefix signs, check logic | Yes | Rules are not copyrightable. Cite standards by number |
| IEC 81346-2 letter codes (e.g. "FC") | Yes, as codes | Short identifiers are facts. Write our own one-line descriptions; do not copy IEC definitions. CCI tables carry their own licence, so check before reusing CCI text |
| IEC 60617 symbol graphics / names / database dump | **No** | Draw our own. Optionally store the S-number as a reference |
| QET elements collection | **No** (licence conflict: CC-BY 3.0 plus "no ML" clause) | Import user files only |
| ISO 1219 symbols | Own drawings only | Same approach as IEC 60617 |
| IEC TR 60890 k/c curves, IEC 60204-1 Table 6, IEC 61439 Annex H tables | **No** (verbatim tables) | Ship the algorithm. Values are user/organisation-provided "rule data packs" entered from licensed copies |
| IEC 61355/81355 DCC/ICC code list | Codes yes; official descriptions no | Own descriptions |
| IEC 60757 colour codes | Yes (codes) | — |
| ETIM 10 model | Yes, with attribution (ODC-By) | Bundle the English version |
| ECLASS | **No** | Store IRDIs supplied by users |
| IEC CDD | Free attributes / IRDIs yes; bulk no | Cite IEC |
| Rittal / Schneider enclosure data | Only if the vendor licence allows | Prefer vendor-supplied part files (EDZ import) |

---

## Implications for ElektraWatt

1. **Structure-first data model.**
   - Implement aspect trees (`==`, `=`, `++`, `+`, `-`, `%`) as first-class nodes.
   - Devices, pages and title blocks *reference* nodes. Reference designations are derived strings;
     identity is a UUID.
   - This removes defects 1, 2, 3 and 6 by construction.
2. **One RDS parser crate.** Make it standalone and well tested (e.g. `ew-rds`), with strict and
   lenient modes and the EBNF of Section 1.3.
   - Check RD-02 rejects structure names that contain prefix characters (`+J-E-2416`) and offers an
     auto-fix to `+JE2416`.
3. **Identifier sets as data.**
   - Ship `IEC81346-2:2019`, `EN81346-2:2009`, `IEC60750-legacy` (to keep the reference's tags) and
     `NFPA79` (later).
   - Function categories map to letters. Run RD-03 conformance warnings, using Section 1.5 as the
     test fixture.
   - Default for new projects: 2019 two-letter codes. Default for QET imports: keep the tags and
     report the suggested 2019 codes.
4. **Numbering schemes are pluggable.** Support sequential, page-prefixed (`-101F1`), page-column
   and free.
   - Renumbering is one transaction that updates all derived outputs (RD-08).
5. **Cross-references are computed relations**, rendered through format templates (QET `%f-%l%c`,
   EPLAN `/p.c`, full structure).
   - Interruption points support star and chain topologies.
   - Contact images are generated from part contact data (IEC 60947 numbering).
6. **Parametric frames.** Templates: ISO 5457 frames and a QET-compatible 17×8 (start 0) frame, so
   imported cross-references keep their meaning.
   - Title block fields per ISO 7200 are bound properties.
   - Revisions come from a managed revision table (defect 5).
7. **Generated pages.** Cover, TOC, section separators, BOM (IEC 62027, aggregated quantities),
   terminal diagram, cable diagram, wire list, PLC overview and layout BOM are generated from the
   model. Each has a default document kind (`&EAA`, `&EAB`, `&EFS`, `&EMA`, `&EMB`, `&EPB`, `&EL?`).
   - Support both DCC (IEC 61355-1:2008) and ICC (IEC 81355-1:2024), with a mapping table.
8. **Potential-driven wire properties.**
   - Potential types: L, N, PE, L+, L−, FE, signal. Colours come from a selected **colour profile**
     (`EN60204-1-machine`, `IEC60445`, `NFPA79`, `Company`).
   - The profile also sets the default cross-section per potential or voltage class, replacing
     folio 1's "if not indicated" note (defect 8).
   - Model DBU as IEC BU plus a dark shade, with a custom code.
9. **EN 60204-1 rule pack** with checks M-01…M-15: main switch class and role, excepted circuits
   orange, PE sizing, I_b ≤ I_n ≤ I_z, conductors per terminal, GNYE exclusivity, control voltage
   limits, nameplate data.
   - Thresholds and tables are loaded from a data pack, not hard-coded, so licensed values can be
     plugged in per organisation.
10. **IEC 61439 thermal module (v1).**
    - Per-part P_n and derating data. P_eff = P_n·(I_b/I_n)².
    - Enclosure k-value and installation type; Q_S, Q_E, fan V = f·Q/ΔT, heater sizing. Report as in
      Section 8.4.
    - IEC TR 60890 multi-compartment calculation in v1.x, with k/c curve data supplied as data.
    - Design-verification and routine-verification checklists are generated per assembly.
11. **Short-circuit/SCCR module (v1.x).** Propagate `ik_max` from the supply. Check device breaking
    capacity / Icw. A UL 508A SB4 calculator follows later on the same graph.
12. **Fluid power on the same graph engine.** Typed ports (1, 2, 3, 4, 5, 12, 14, 82, 84, X), media,
    line types, valve position matrices. Solenoids are electrical sub-functions that carry PLC
    channels. Checks FL-01…FL-04.
13. **PLC addressing.** Addresses are assigned per card channel, through module address maps.
    - Multi-notation parse/render (IEC, S7 international/German, Rockwell tags).
    - Checks PLC-01…06.
    - Exports: AR APC AML (1.2.0 and 1.4.0), TIA PLC-tag table CSV/XLSX, CODESYS/PLCopen.
14. **Part data dictionary.** Properties keyed by ETIM/ECLASS/IEC CDD IRDIs plus native names.
    - Bundle ETIM English (ODC-By).
    - Never bundle ECLASS or IEC 60617 content.
15. **Symbol library.** Draw our own IEC-style and ISO 1219-style symbols under CC-BY 4.0 (or CC0),
    with an explicit "diagrams are unencumbered" exception.
    - Metadata follows IEC 81714-2 concepts, with an optional IEC 60617 reference number.
    - Do not ship the QET collection.
16. **PDF/A output via krilla.** PDF/A-2b by default; PDF/A-3b/4f with the embedded project/AML/CSV;
    internal links for cross-references; bookmarks per structure.
    - veraPDF in CI on golden files built from the reference project.
17. **Rule packs are versioned data** (`ruleset: "EN60204-1:2018+A1:2025"`, `"IEC61439-1:2020"`).
    The project records which rule-pack versions it was checked against, for the design-verification
    record and for audits.
18. **Test fixtures from the reference set.** Encode defects 1–9 as regression tests:
    - re-tag -3W1 → BOM updates;
    - T51/T55 mismatch → RD-05 error;
    - `+J-E-2416` → RD-02;
    - mixed revisions → revision model;
    - duplicate X1031 lines → aggregated BOM;
    - missing E56/S56 part data → "device without part" check.

## Open questions

1. **Normative prefix for the type aspect** (IEC 81346-1:2022), the exact rule for repeated-prefix
   vs. `.` notation, and the 2022 information-model clauses. These need a licensed copy, so purchase
   IEC 81346-1:2022 and IEC 81346-2:2019 for the core team.
2. **Default identifier set for the user (the pilot user).** Should ElektraWatt migrate their projects to
   IEC 81346-2:2019 two-letter codes (QB, FC, BT, TB …), or keep their legacy single letters with
   warnings? Is the page-prefixed tag scheme (`-101F1`) a house rule that must be preserved?
3. **IEC 81355-1:2024 ICC codes.** Does the `&` prefix remain, and what are the ICC equivalents of
   EFS/EMA/EMB/EPB? Will customers (e.g. <end user>, utilities) require DCC or ICC on deliverables
   in 2027+?
4. **Colour profile conflict.** For DC control wiring in machine panels, does the user want
   EN 60204-1 practice (DBU/WH or BU), IEC 60445 RED/WHITE, or both selectable? Confirm DBU's exact
   meaning and RGB for printing.
5. **Licensed table data.** Can the project legally distribute digitised values for EN 60204-1
   Table 6, IEC 61439 Annex H, or IEC TR 60890 k/c curves, possibly via an IEC/DKE licence? Otherwise,
   who maintains the "rule data packs" per organisation?
6. **Enclosure thermal data.** Do Schneider (NSYPLA polyester) and Rittal publish k-values,
   dissipation curves and P_n data in machine-readable form (EDZ, ECLASS Advanced, IEC 62683-1) that
   ElektraWatt may import?
7. **Fluid identification.** Does the user want ISO 1219-2 component codes on pneumatic pages, or
   IEC 81346 RDs everywhere (current practice)? The ISO 1219-2:2012 separator format must be checked
   in a licensed copy.
8. **AR APC version.** Which version do the user's TIA Portal version and CODESYS target accept
   (1.2.0 vs 1.4.0)? Obtain sample AML files from TIA Portal and EPLAN for round-trip tests.
9. **Grid convention.** Keep QET's 17×8 grid starting at 0 for new projects, or switch to an
   ISO 5457 / EPLAN-style column grid? This affects every cross-reference string in migrated
   projects.
10. **North American scope timing.** Is UL 508A/NFPA 79/CSA 286 "Later" (post-v1)? SCCR and AWG
    support influence the part data model now, even if the checks ship later.
11. **EN 60204-1 next edition.** IEC 60204-1 Ed. 7 has been expected around 2026, but no publication
    was found. Monitor IEC TC 44 so the rule pack can be versioned when it appears.
12. **PDF/A level preferred by the user's customers** (archive systems at end users), and whether the
    native project file may be embedded in delivered PDFs, given the confidentiality notice in the
    title block.
