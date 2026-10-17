# Reference drawing set: page-by-page requirements extraction

**Subject:** `<drawing id>.pdf`, cabinet **J-E-2416** ("Expansion Control Cabinets <end user>"). It has 18 pages, A4 landscape (842 × 595 pt), and was produced by QElectroTech 0.100.0-dev on Qt 5.15.11. The PDF metadata gives `CreationDate D:20240712172955+02'00'`.

**Purpose:** This document does three jobs:
- It verifies and corrects the analysis in the shared brief.
- It lists every element type per folio, together with the data that element implies.
- It derives the requirement candidates (`REF-nnn`) and the defect list.
- It defines a **golden project**. ElektraWatt must reproduce this project defect-free from its own model.

---

## Method and evidence base

| Evidence | How it was obtained |
|---|---|
| Visual reading of all 18 pages | Rendered three times with the `Read` tool (pages 1-6, 7-12, 13-18). Re-rendered at 300 dpi with `pdftoppm`, then 25 detail crops (folios 3, 101, 102, 103, 106, 201, 202, 203, 902) were inspected. |
| Exact geometry (mm) | Vector extraction with `pdfplumber`: lines, rectangles, curves, line width, dash array and fill colour per object, converted from pt to mm (× 25.4/72). |
| Text sizes and positions | Character-level extraction (font name, size, x/y). Each text cluster was mapped to its grid cell `<row><column>`. |
| Raw drawing transforms | `pypdf` content-stream inspection. Every drawing is placed with `cm 0.986282578 … ` inside a `0.75` px→pt transform. QET therefore printed "fit to page": 1 QET px = 0.75 × 0.98628 pt = **0.26096 mm** on paper. |
| PDF structure | 0 annotations (no hyperlinks), no outline (no bookmarks), page labels `1…18` (physical numbers, not folio numbers). Fonts: Tahoma and Tahoma-Bold (CID TrueType, Identity-H, embedded, **not subset**). Logos are raster images (98 × 140 px and 98 × 129 px). |
| QET format semantics | Taken from the sibling research file `research/interop-formats.md`, which read the QET source ([borderproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/borderproperties.cpp), [xrefproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/xrefproperties.cpp), [assignvariables.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/autoNum/assignvariables.cpp)). |
| Standards facts | Cross-checked against the sibling `research/standards.md` (IEC 60757, IEC 60445, EN 60204-1, IEC 81346-2 tables). |
| **Web verification** | **Not possible in this run.** The shared web-search budget was exhausted, and vendor domains (festo.com, phoenixcontact.com, se.com) did not resolve from the sandbox. Every vendor fact that is *not printed in the drawing* (part-number meanings, pin colour codes, dimensions) is marked **(unverified)**. |

Coordinates below are **paper millimetres from the top-left corner of the A4 sheet**, unless stated as "native". Grid references use the drawing's own format `<folio>-<row><column>`.

---

## Corrections to the shared brief

| # | Brief statement | Finding | Evidence |
|---|---|---|---|
| C1 | Defect 6: description inconsistent on "pages 101, 106" | **Only folio 101** says "Control Cabinet Expansion". Folio 106 says "Expansion Control Cabinets". Folio 106 differs in **revision** (V2.0), not in description. | Title-block text per page (§5.1) |
| C2 | Defect 7: "Cabinet layout title block Next: empty" | **Not a defect.** Folio 902 is the last page, so an empty `Next` is correct, as is the empty `Prev` on folio 1. All 17 Prev/Next links are correct. The real navigation gap is in the PDF: no bookmarks, no hyperlinked cross-references, and page labels are physical numbers. | Title blocks; `pypdf` structure check |
| C3 | Folio 203-206 "ports 2/4/3/5(15)" | The tiny text "3 15" reads **"3 1 5"**: exhaust 3, supply 1, exhaust 5 on top. Working ports 2 and 4 are at the bottom. | 300-dpi crop of folio 203 |
| C4 | Folio 902 shows "DIN rails" | **No DIN rails are drawn.** It shows the cabinet outline, 5 duct segments (4 labelled "40x60", 1 unlabelled), device rectangles and terminal-strip blocks with 4 end brackets. | Vector dump of folio 902 |
| C5 | Defect 2: "layout uses S51/1X1 without the dash/page prefix" | S51 and 1X1 are **identical** to the schematic tags, which also have no prefix. The layout drops the **location prefix** of `+JE2416-X1031/-X1032` (shown as `X1031`/`X1032`). It also shows **T51** instead of **T55**. | Folio 902 text |
| C6 | Defect 4: "Part list misses -106W2, -201U4..U8" | The part list misses **-106W2 and -201U4 … -201U9 (7 tags)**. It also misses the second part of -201U1 (550208), all 28 individual valves 571334, the enclosure, the backplate, ducts and accessories. The table is **not truncated**: it is sorted by label and its last row (T55) is present. | Folio 901 |
| C7 | Folio 201/202 "-201U9 (533354 VMPA1-FB-SP end plate)" | "End plate" is an interpretation. The same part 533354 appears in the middle of the manifold (-201U9, before the page break) **and** at its end (-202U3). "SP" more likely means *supply plate* **(unverified)**. | Folios 201, 202 |
| C8 | Folio 202 implies 4 valves on -202U2 | The overview shows **5 valves** (571334 × 5) on the 4-position sub-base -202U2. Detail folio 206 and the part list ("4x 5/2 Valve") show 4. **New defect.** | 300-dpi crop of folio 202; text count: 29 × "571334" on 201+202 |
| C9 | Folio 101 "fan/heater E55" | The part list calls it "Heater". The symbol is a box with an "∞"-like glyph and three connection points N, L, PE. Treat it as an enclosure heater **(part identity unverified)**. | Folios 101, 901 |
| C10 | Folio 3 "off-page DP interruption" | The "DP" arrow has **no cross-reference**: it is an *external* connection to another drawing ("To J-E-206 / JE-N1000-03"). The J-E-206-L/N/PE arrows on folio 101 are the same kind. | Folios 3, 101 |
| C11 | Each manifold module has "∞ 203-B4" references | Only -201U1, -201U3 … -201U8, -202U1 and -202U2 have them. -201U2, -201U9 and -202U3 have none. The xref of -201U1 to folio 3 reads **"3-B0"**, which points at an empty column (see D-14). | Folios 201, 202 |
| C12 | Electrical correctness not discussed | Folio 101 draws **thermostat T55 and door switch S56 in the neutral conductor**. The live conductor of E55 and E56 comes unswitched from F55:2. **New, safety-relevant finding** (D-13). | 300-dpi crop of folio 101 |
| C13 | Cross-references (implicitly suspect) | **All 22 interruption-point pairs and all 10 main↔auxiliary reference pairs (9 devices) are mutually consistent**, except the "3-B0" anchor oddity. The QET cross-reference engine worked. The defects are in tags, parts, layout and title-block data. | §4.13 |

---

## 1. Per-folio inventory

Each folio entry lists its element types and the data that type implies for the model.

### 1.1 Folio 1: Title page ("J-E-2416 / Electrical Definition")

| Element type | Instances and content | Data implied |
|---|---|---|
| Frame without grid | Outer rectangle 9.79-281.18 × 9.79-182.02 mm. Standard title block below. | Page type "Title page" uses a frame variant without the column/row grid. |
| Heavy and light horizontal rules | 1.305 mm rule at y = 13.70 (x 11.09-279.88). 0.522 mm rule at y = 45.02. | Form graphics. |
| Cover title | "J-E-2416" at 35.51 pt (centred, y 17.6). "Electrical Definition" at 19.97 pt. | Project property `object/cabinet id` and document type text. |
| Logo (raster) | 256.72/14.97 mm, 19.69 × 25.92 mm, PNG 98 × 129 px (≈126 dpi) | Company logo asset; vector form is preferred. |
| **Project data box** (left) | 13.70-118.09 × 47.63-175.50 mm, line 0.418 mm, drop shadow 2.61 mm (black filled strips). 4 sub-groups separated by 0.261 mm rules at y = 84.16, 120.70, 154.62. | Project properties (next row). |
| Project properties (label → value) | Company / Customer → *<customer>*; Project Description → *Expansion Control Cabinets <end user>*; Drawing Identification → *<drawing id>*; Project Reference → *<project ref>*; End User → *<end user>*; Address → *<address>*; Postcode / City → *<postcode, city>*; Country → *Netherlands*; Manufacturing Year → *2024*; Supply → *1 x 230 VAC +N +PE, 50hz*; Control Current → *24VDC*; Consumption → *6A*; Created On → *10-06-2024*; Edited On → *12-07-2024*; Engineer → *<engineer>* | Structured project master data. The supply line implies `Supply{phases:1, N:true, PE:true, Un:230 V, f:50 Hz}`. The control voltage implies a potential type +24 V DC. Consumption implies a rated current of 6 A. |
| **Cable colour definitions table** | 128.52-274.66 × 47.63-144.18 mm, header "CABLE COLOUR DEFINITIONS" (11.84 pt) plus "IEC60757 / EN60204-1" (8.88 pt). Columns VOLTAGE / FUNCTION / COLOUR / SIGN at x = 128.52, 170.28, 217.25, 251.17, 274.66. 17 data rows of 5.22 mm. | Colour profile: (voltage class, function) → colour name and IEC 60757-style code. Rows: 24 VDC {Positive DBU, Signal DBU, 0V WH}; 230 VAC {Live BK, Signal BK, Neutral BU, Earth GNYE}; 400 VAC {Phase 1/2/3 BK, Neutral BU, Earth GNYE}; Analogue {Positive GY, Negative WH, Earth GNYE}. |
| **Standard size tables** (2) | Control: 128.52-198.98 × 149.40-172.89 (DBU / WH / GNYE = 0.75 mm²). Mains: 204.20-274.66 × same y (BK / BU / GNYE = 1.5 mm²). | Default cross-section per colour or circuit class. |
| Rule note | "If size or sign is not indicated in diagram, standard size or sign is to be used." (6.66 pt) | The engineering rule is held in free text. It must become a **potential-type → wire-property rule** (D-08). |
| Typos | "Yellow / 4 Green" (×3), "50hz", "0.75 mm ²" (space before the superscript) | Need spell/format checks and unit formatting (D-18). |

### 1.2 Folio 2: Table of contents ("Table of Contents 1")

| Element type | Content | Data implied |
|---|---|---|
| Report table | 16.31-277.27 × 15.01-138.96 mm. Header 6.52 mm high, grey fill RGB 0.753 (#C0C0C0). 18 data rows of 6.52 mm. Line 0.183 mm. Columns: *Folio number* (16.31-79.73), *Title* (79.73-214.90), *Location (+)* (214.90-277.27). Text 8.88 pt left-aligned. | The TOC is generated from page properties: folio number, page description, location. It needs no grid. |
| Rows | All 18 folios. Location is always "+J-E-2416". Section pages carry the full string "=P01+J-E-2416 - Section n - Page … - …". | Page description, page type and structure (=, +) per page. The TOC is complete and correct. |

### 1.3 Folio 3: Bus topology, Profibus

| Element type | Instances (cell) | Data implied |
|---|---|---|
| Device aux representation | **-201U1** label at A1 (9.62 pt). Texts: "Festo / 51E-F13GCQP-D+H2GG / CPX FB13 / Node TBD" (6.66 pt). Xref "(201-E1)" (5.18 pt). | Aux function of the main device on 201. Shows the **configurator order code** (not the article number), the type "CPX FB13" and the **bus station address placeholder "TBD"**. |
| Bus port (black box) | Rectangle 47.63-71.11 × 45.02-55.46 mm, inner label "DP" (3.70 pt) | Device port of kind PROFIBUS DP (Sub-D 9). |
| Plug | **-3X1** (C1, 9.62 pt), Festo 532216, "DP Connector", note *Switch to "ON"* (5.18 pt). Inner rectangle 50.24-60.68 × 47.63-52.85 with a circle. | Bus connector device with property `termination = ON`. |
| Bus cable (single-line) | **-3W1** at E2: a cable marker tick across one bus line | Bus cable (2-core, shielded). Its part appears only in the part list, as -4W1, Siemens 6XV1830-0EH10. |
| Bus line | Polyline from -3X1 down to G2, then right to G15 | Bus connection drawn single-line. |
| External connection | Dotted box 243.34-279.88 × 131.13-167.67 mm (line 0.261, dash 1:2). Arrow "DP" (8.88 pt). "To J-E-206 / JE-N1000-03" (6.66 pt). Caption "External Connection" (9.62 pt). | Interface to another project or cabinet (J-E-206) and device (JE-N1000-03). No xref, no master or PLC shown. |

### 1.4 Folios 100, 200, 900: section separators

| Element type | Content | Data implied |
|---|---|---|
| Section header | 1.305 mm rule at y = 18.92; "=P01+J-E-2416" Tahoma-Bold 27.37 pt; subtitle 21.45 pt "Section 1 - Page 100 - 199 - Wiring Definition" (or "Section 2 - Page 200 - 299 - Pneumatics", "Section 9 - Page 900 - 999 - Nomenclature"); 0.522 mm rule at y = 50.24 (x 13.70-277.27); logo 259.33/20.18, 19.69 × 25.92 mm | Page type "Section divider" generated from a **section definition** (number, folio range, title). The structure-identifier prefix is shown. |
| Title block | Page description = the same string as the header (8.88 pt) | Page description is derived from the section. |

### 1.5 Folio 101: Power Supply 1, Main Current (revision V2.0, description "Control Cabinet Expansion")

| Element type | Instances (cell) | Data implied |
|---|---|---|
| External connection box | Dotted box (dash 1:2 × pen width), cells F0-H5 (≈17-83 × 130-172 mm). Caption "External Connection (J-E-206 / JE-N1000-03)" (9.62 pt). | External supply origin (another cabinet). |
| External arrows (incoming, no xref) | "J-E-206-L", "J-E-206-N", "J-E-206-PE" at G0 (8.88 pt), hollow triangles | External source points with names. **No supply cable is drawn** (no tag, type or size). |
| Terminal strip | **1X1** at D2 (9.62 pt). Terminals **1, 2, PE** (6.66 pt): circle symbol with a through-line. | 3 terminals. Field side (bottom) to J-E-206, internal side (top) to S51 and to earth. Parts (from 901) "UK5N / ULSKG5". |
| Main switch | **S51** at B1 (8.88 pt; other tags 9.62). "Eaton / Holec", "533356", "40A" (6.66). 2-pole NO contacts with pins **1/2, 3/4** (3.70 pt). Manual rotary-type actuator joined by a dashed linkage (0.261 mm, dash 4:2). | 2-pole load-break switch. **Fed from the bottom** (pins 2 and 4), outputs 1 and 3. |
| Wire markers | Two markers (above pins 1/3 and below pins 2/4): horizontal tick line with an oblique tick per conductor and texts "2.5", "BK", "BU" (3.70 pt) | Explicit cross-section and colour per conductor, drawn as **graphics, not wire properties**. |
| MCB 1P+N | **-101F1** at B7: Schneider A9P52610, "10A C". Pins N, 1 (top); N, 2 (bottom) at 3.70 pt. Thermal and magnetic trip boxes ("I>" 8.88 pt). Manual toggle actuator box. Internal linkage 0.130 mm dashed (2:1). | 1P+N MCB with an unprotected N pole. Connection point "N" occurs **twice** (in and out). |
| MCB 1P+N | **F55** at B11: Siemens 5SY4 204-7, "4A C", same symbol. | Same as -101F1. |
| Thermostat | **T55** at C11: Rittal SK 3110.000, setpoint "15c". Contact pins 5, 3. Thermal actuator box (θ). | Temperature switch with setpoint 15 °C. The contact sits **in the N path** (F55:N → T55:5 → T55:3 → E55:N). |
| Heater | **E55** at E11: Rittal SK 3102.000. Pins N, L (top), PE (bottom). | Load with 3 connection points. |
| Door switch | **S56** at D14 (8.88 pt). Position-switch symbol with a dashed bracket actuator. **No part data, no pin designations.** | Contact in the **N path** to E56. |
| Lamp | **E56** at E14: IEC lamp (circle with X). **No part data, no pin designations.** | Load. Its L comes from the F55:2 junction. |
| Protective earth symbol | Circle with an earth symbol at D5, fed from 1X1:PE | Connection to the PE bar or mounting plate. It is **not connected in the drawing** to the PE terminal strip on 103 (D-21). |
| Junctions | Dots at F55:2 (L branch to E56) and F55:N' (N branch to S56). **Bevelled corners** without dots at the top bus (S51:1 → -101F1:1 → F55:1 and S51:3 → -101F1:N → F55:N). | The vector dump shows these are **separate conductors daisy-chained on -101F1 terminals**. The bevel encodes the wiring sequence. |
| Interruption points (outgoing) | **PE-1.1** at G15 → "103-G6"; **L-1** at H16 → "102-A1"; **N-1** at H16 → "102-A1". Label 8.88 pt, xref 6.66 pt below the arrow. | Named connection continuation with an automatic cross-reference. |

### 1.6 Folio 102: Power Supply 2, Control Current

| Element type | Instances (cell) | Data implied |
|---|---|---|
| Interruption points in | **N-1** (A0/A1, xref "101-H16") above **L-1** (A1, "101-H16"). The order is the reverse of folio 101. | |
| PSU (black box) | **-102G1** at A5/B5: Phoenix Contact 2904602, "20A". Pins **L, N, PE** (top), **+, +, −, −** (bottom) at 5.92 pt. Inner graphic: diagonal line, "AC ~" / "24 VDC" (5.18 pt). | 1AC → 24 V DC converter with duplicated output terminals. |
| Wire markers | "1.5 DBU DBU WH WH" under the PSU outputs; "1.5 DBU DBU" above -102F2 | Explicit 1.5 mm² wires. |
| MCB 1P | **-102F1** at C5: Schneider A9F78116, "16 B". Pins "1 / L1" (top), "2 / T1" (bottom). Two designations per point. | 1-pole MCB in **+24 V DC**. A connection point has an alias designation. |
| Electronic load distribution (black box) | **-102F2** at E5: "Murr Elektronik", "MICO 4.10". Fields 24VDC (2 points), GND (2 points), signal contact **13-14** (NO symbol), **Out 1-Out 4**, each "6A" (6.66 pt). "24VDC"/"GND" at 8.14 pt. | 4-channel electronic circuit breaker with a per-channel current setting (6 A). The diagnostic contact 13-14 is **unconnected**. |
| PE connection | -102G1:PE → **PE-1.2** at A15 → "103-G6" | |
| 0 V branches | −(2) → **0VDC** at C15 → "103-A1"; −(1) → **PE-0VDC** at D15 → "103-G1" (bevel branches). Both − terminals also feed -102F2 GND (1) and (2). | 0 V is bonded to PE on folio 103 (PELV earthing). |
| Outgoing 24 V | Out1 → **24VDC-1** (H15 → "103-A1"); Out2 → **24VDC-2** (H15 → "103-C1"); Out3 → **24VDC-3** (G15 → "103-D1"); Out4 → **24VDC-4** (G15 → "103-F1") | Four protected 24 V circuits. |
| Parallel feeds | +(1) and +(2) → -102F1:1 (two wires in one clamp). -102F1:2 → -102F2:24VDC (1) and (2). | Two wires per terminal. A clamp capacity check is needed. |

### 1.7 Folio 103: Power Supply 3, Control Current Distribution

| Element type | Instances (cell) | Data implied |
|---|---|---|
| Interruption points in | 0VDC (A0, "102-C15"); 24VDC-1 (A0, "102-H15"); 24VDC-2 (C0, "102-H15"); 24VDC-3 (D0, "102-G15"); 24VDC-4 (F0, "102-G15"); PE-AIR2 (G0, "106-B1"); PE-0VDC (G0, "102-D15"); PE-AIR1 (G0, "106-E1") | |
| Terminal strip "+JE2416-X1031" (24 V side) | Tag at A3 (9.62 pt), group label "Pneumatics" (8.88 pt). 4 terminals **"1+"** bridged (dots at x ≈ 290 px-equivalent), outgoing to 24VDC-AIR1…4. "Spare": 2 × **"2+"**, **"3+"** plus one **unlabelled** terminal, 2 × **"4+"**. | 10 terminal symbols. Duplicate designations. One missing designation (D-11). |
| Terminal strip "+JE2416-X1031" (0 V side) | Second tag at A10, label "Pneumatics". 4 × **"1-"** → 0VDC-AIR1…4. Spare: 2 × "2-", 2 × "3-", 2 × "4-". Fed from a 0VDC bus line with bevel branches (col 9). | 10 terminal symbols. The **same strip is drawn as two "sub-strips"**, and the tag is repeated. |
| Terminal strip "+JE2416-X1032" (PE) | Tag at F3. 5 terminals all designated **"PE"**. Left side: PE-AIR2, PE-0VDC, PE-AIR1 into the bridge. Right side: PE-1.1 … PE-1.4. 5th terminal spare. | 5 PE terminals, bridged. Duplicate designations. |
| Interruption points out | 24VDC-AIR1 (A6 → "106-E1"); -AIR2 (B6 → "106-E1"); -AIR3 (B6 → "106-B1"); -AIR4 (B6 → "106-B1"); 0VDC-AIR1 (A14 → "106-E1"); -AIR2 (B14 → "106-F1"); -AIR3 (B14 → "106-B1"); -AIR4 (B14 → "106-C1") | The same net carries several **interruption-point names** (24VDC-1 = 24VDC-AIR1…4). |
| Interruption points in (from the right, ◁) | PE-1.1 (G6, "101-G15"); PE-1.2 (G6, "102-A15"); PE-1.3 (G6, "106-D1"); PE-1.4 (G6, "106-A1") | The arrow direction encodes signal direction, not page side. |
| Free texts | "Pneumatics", "Spare" (8.88 pt) | Terminal group or function text. |

### 1.8 Folio 106: Power Supply 4, Pneumatics (revision V2.0)

| Element type | Instances (cell) | Data implied |
|---|---|---|
| Interruption points | PE-1.4 (A1, ◁, "103-G6"); 24VDC-AIR3 (B1, "103-B6"); 24VDC-AIR4 (B1, "103-B6"); PE-AIR2 (B1, ◁, "103-G1"); 0VDC-AIR3 (B1, "103-B14"); 0VDC-AIR4 (C1, "103-B14"); PE-1.3 (D1, ◁, "103-G6"); 24VDC-AIR1 (E1, "103-A6"); 24VDC-AIR2 (E1, "103-B6"); PE-AIR1 (E1, ◁, "103-G1"); 0VDC-AIR1 (E1, "103-A14"); 0VDC-AIR2 (F1, "103-B14") | |
| Cables (multi-line) | **-106W1** (F5) and **-106W2** (F11), 9.62 pt. "Festo / NEBU-G78W5-K-2-N-LE5" (5.92 pt). Cable marker across 5 conductors with core colours **BK, BU, GNYE, WH, BR** (3.70 pt). | Cable with 5 cores and core-colour identification. "BR" is not an IEC 60757 code (BN). |
| Device aux representation (black box with plug) | **-201U1** (G2, xref "(201-E1)") and **-202U1** (G8, "(202-E1)"). Box text "FESTO CPX" (5.92 pt). Pins **1-5** (5.92 pt) drawn as plug-pin bars (three parallel 0.26 mm strokes, ≈0.78 × 1.83 mm), with names "0V Val", "0V El", "PE", "24V El", "24V Val". Separate earth terminal (⏚ plus circle) on the right side of the box. | Power connector (M18/7/8", 5-pin) and functional-earth terminal of the CPX power interlinking block. |
| Wiring | -201U1: 1 ← 0VDC-AIR2 (BK), 2 ← 0VDC-AIR1 (BU), 3 ← PE-AIR1 (GNYE), 4 ← 24VDC-AIR2 (WH), 5 ← 24VDC-AIR1 (BR); FE ← PE-1.3. -202U1: 1 ← 0VDC-AIR4, 2 ← 0VDC-AIR3, 3 ← PE-AIR2, 4 ← 24VDC-AIR4, 5 ← 24VDC-AIR3; FE ← PE-1.4. | The supply pairs are **crossed** (El is fed by 24VDC-AIR2 but returns on 0VDC-AIR1). All four 24 V feeds come from one MICO channel (D-25). |

### 1.9 Folios 201, 202: Valve Manifold Overview 1 and 2

| Element type | Instances | Data implied |
|---|---|---|
| "PART DATA" box | 16.31-89.38 × 16.31-52.85 mm, drop shadow to x 91.99 (on 902 the box is 16.31-71.11). "PART DATA" 15.53 pt. "Order Code 1:" **51E-F13GCQP-D+H2GG** and "Order Code 2:" **32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS** (8.88 pt). Repeated identically on 202. | **Configurable product order codes** (Festo configurator type codes for the CPX and MPA parts). The data belongs to an assembly, not to a page. |
| Assembly front view | Row of module rectangles, row E-F, line 0.261 mm with thicker module separators. Each module shows manufacturer, part number and type (5.18 pt). Valves drawn as stylised top views (front-view symbol, not ISO 1219). | Physical order of the manifold modules. **Break marks** (slanted lines) at the right end of 201 and the left start of 202 show that one manifold continues across pages. |
| Modules on 201 | -201U1: Festo 195740 CPX-FB13 (top), Festo 550208 CPX-M-GE-EV-S-7/8-5POL (bottom). -201U2: 552286 VMPA-FB-EPLM-G. -201U3 … -201U8: 533361 VMPA1-FB-EMG-8, each with 4 × 571334. -201U9: 533354 VMPA1-FB-SP. | Composite device (-201U1 has **2 parts**). 24 valves. |
| Modules on 202 | -202U1: 550208 CPX-M-GE-EV-S-7/8-5POL. -202U2: 533361 VMPA1-FB-EMG-8 with **5 × 571334**. -202U3: 533354 VMPA1-FB-SP. | 5 valves drawn on a 4-position module (D-10). |
| Main→aux xrefs ("∞" link glyph, vector-drawn, plus target) | -201U1: "3-B0", "106-G3". -201U3: "203-B4". -201U4: "203-B12". -201U5: "204-B4". -201U6: "204-B12". -201U7: "205-B4". -201U8: "205-B12". -202U1: "106-G9". -202U2: "206-B4". | The main function is on the overview. Aux functions are on 3, 106 and 203-206. |
| Tag text sizes | -201U1 … U9 9.62 pt. -202U1 and -202U3 **8.88 pt**, -202U2 9.62 pt. | Inconsistent formatting (D-29). |

### 1.10 Folios 203-206: Valve Manifold 1-4 (ISO 1219 pneumatic schematic)

| Element type | Instances | Data implied |
|---|---|---|
| Module aux representation | Tag (8.88 pt) and xref to the overview: 203: -201U3 "(201-E4)", -201U4 "(201-E6)"; 204: -201U5 "(201-E8)", -201U6 "(201-E10)"; 205: -201U7 "(201-E12)", -201U8 "(201-E14)"; 206: -202U2 "(202-E3)" | |
| Valve function boxes | Per valve a cell of 26.1 mm × ≈ 24 mm. Top row: **PLC output address** (6.66 pt), second row: **function text** (6.66 pt). | Valve position with a PLC output and a function text. |
| 5/2 valve symbol (ISO 1219-1) | Two squares (separating double bar 0.522 mm), spring return on the left ("W"), solenoid with manual override on the right. Ports **3 1 5** on top (5.92 pt), **2** and **4** at the bottom (connection circles). | Valve type 5/2 monostable, solenoid / spring. Pneumatic ports 1-5. |
| Addresses and functions | 203: Q0.1 TA05-V-15, Q0.3 TA05-V-33, Q0.5 TA05-V-18, Q0.7 TA05-V-17 (-201U3); Q1.1 TA05-V-36, Q1.3 TA05-V-38, Q1.5 TA05-V-16, Q1.7 TA05-V-37 (-201U4). 204: Q2.1/3/5/7, Q3.1/3/5/7 all "Spare". 205: Q4.x, Q5.x "Spare". 206: Q6.1/3/5/7 "Spare". | 28 valve positions; 8 used, 20 spare. **Only odd bits** are used (D-26). |
| Pneumatic working lines | 203 only: from ports 2 and 4 down to row G | Fluid connections. **No tube size, material or tags.** |
| Process actuator and valve | 203 only, row G: double-acting actuator (box with two arrowed inputs) linked by a dashed mechanical linkage (0.261, dash 4:2) to a 2-way process valve (bow-tie symbol) | Field devices for TA05-V-xx. **Not tagged**, no location. |
| Missing fluid elements | No supply line to port 1, no exhaust or silencers on ports 3/5, no air preparation, no pressure data | Incomplete fluid model (D-28). |

### 1.11 Folio 901: Partlist 1 (nomenclature)

| Element type | Content | Data implied |
|---|---|---|
| Report table | 16.31-277.27 × 15.01-171.58 mm. Header 6.52 mm, grey #C0C0C0. 23 rows of 6.52 mm (the page is exactly full). Columns: *Label* (16.31-63.81), *Manufacturer* (-111.30), *Article number* (-170.02), *Textual description* (-233.95), *Folio number* (-277.27). 8.88 pt. Sorted by label as an ASCII string ("+" < "-" < digits < letters). | QET nomenclature: **one row per element instance**. No quantity, no position number, no order number separate from the type. |
| Rows (verbatim) | +JE2416-X1031 / Phoenix Contact / PT 2.5-TWIN / – / 103 (**twice**); +JE2416-X1032 / Phoenix Contact / PT 2.5-TWIN-PE / – / 103; -101F1 / Schneider / A9P52610 / 10A C / 101; -102F1 / Schneider / A9F78116 / 16 B / 102; -102F2 / Murr Elektronik / MICO 4.10 / Protective Gear / 102; -102G1 / Phoenix Contact / 2904602 / 20A / 102; -106W1 / Festo / NEBU-G78W5-K-2-N-LE5 / Connector / 106; -201U1 / Festo / 195740 / CPX DP Connection 195740 / 201; -201U2 / Festo / 552286 / – / 201; -201U3 / Festo / 533361 / 4x 5/2 Valve 571334 / 201; -202U1 / Festo / 550208 / Halfway PSU / 202; -202U2 / Festo / 533361 / 4x 5/2 Valve 571334 / 202; -202U3 / Festo / 533354 / – / 202; **-4W1** / Siemens / 6XV1830-0EH10 / DP Cable / 3; **-4X1** / Festo / 532216 / DP Connector / 3; 1X1 / Phoenix Contact / UK5N / ULSKG5 / – / 101; E55 / Rittal / SK 3102.000 / Heater / 101; E56 / – / – / – / 101; F55 / Siemens / 5SY4 204-7 / 4A C / 101; S51 / Eaton / Holec / 533356 / Main Switch / 101; S56 / – / – / – / 101; T55 / Rittal / SK 3110.000 / Thermostat / 101 | 22 distinct labels in 23 rows. "Description" holds ratings ("10A C", "20A"). One field holds two alternative parts ("UK5N / ULSKG5"). Manufacturer names are not normalised (D-35, D-18). |

### 1.12 Folio 902: Cabinet Layout

| Element type | Content | Data implied |
|---|---|---|
| "PART DATA" box | 16.31-71.11 × 16.31-52.85 mm. "Cabinet: Schneider Electric NSYPLA1074G"; "Backplate: Schneider Electric NSYPMM107" (8.88 pt) | Enclosure and mounting-plate parts. **Neither is in the part list.** |
| Cabinet outline | 118.09-206.81 × 31.97-159.84 mm (88.72 × 127.87), line 0.522 mm | Enclosure front view. |
| Cable ducts | Top 120.70-204.20 × 34.58-39.80 ("40x60"); left vertical 120.70-125.91 × 39.80-138.96 ("40x60", rotated); right vertical 198.98-204.20 × 39.80-91.99 ("40x60", rotated); middle 125.91-198.98 × 65.89-71.11 ("40x60"); lower 125.91-198.98 × 86.77-91.99 (**unlabelled**). Labels 5.18 pt. | Duct type 40 × 60 mm, length per segment. |
| Devices (plain rectangles plus text) | -102G1, -101F1 (rotated), -102F1 (rotated), -102F2, **T51**, S51 (row 1); 1X1, X1031, X1032 (rotated) with 4 end brackets (row 2); -201U1, -201U2, -201U3 … -201U8, -202U1, -202U2, -202U3 (row 3) | Placement by tag. **Not linked** to schematic devices. Rotated labels are scaled down to 1.88-3.40 pt. |
| Missing | F55, E55, E56, S56, **-201U9**, DIN rails, earth bar, enclosure accessories | Layout is incomplete (D-32). |
| Scale | No scale is stated. Duct 40 mm drawn as 5.22 mm (≈1:7.7). -102G1 drawn 7.83 × 10.44 mm (aspect 0.75), while the real QUINT4 20 A is ≈ 70 × 130 mm (aspect ≈ 0.54, **unverified**). | Not to scale. Not usable for drilling or production. |

---

## 2. Requirement candidates

**Categories:** project, page/frame, symbol, device, connection/wire, potential, cross-ref, terminal, cable, PLC/bus, fluid, parts/BOM, reports, panel layout, title block/forms, numbering, revision, checks.

| ID | Requirement statement ("The system shall …") | Source folio(s) | Category |
|---|---|---|---|
| REF-001 | … store project master data as typed properties: customer, end user (name, address, postcode, city, country), project description, drawing identification, project reference, object/cabinet id, manufacturing year, engineer, created and edited dates. | 1 | project |
| REF-002 | … store the **supply definition** as structured data (phases, N, PE, nominal voltage, frequency, earthing system) and render it as text (e.g. "1 x 230 VAC +N +PE, 50 Hz"). | 1 | project |
| REF-003 | … store control voltage and rated consumption as project properties usable in forms and checks. | 1 | project |
| REF-004 | … distinguish *customer* and *end user* as separate roles. Each title-block field binds to exactly one of them. | 1, all title blocks | project |
| REF-005 | … generate cover pages from a form template bound to project properties, including tables of colour definitions and standard sizes taken from the active colour/size profile. | 1 | title block/forms |
| REF-006 | … provide a **colour profile** mapping (voltage class, potential function) → colour name + code, with IEC 60757 codes and company custom codes (e.g. DBU = dark blue) kept distinct. | 1 | potential |
| REF-007 | … provide a **standard wire size profile** per circuit class (control 0.75 mm², mains 1.5 mm²). It shall apply the profile automatically to wires without explicit size. | 1 | connection/wire |
| REF-008 | … show the effective (explicit or rule-derived) wire properties in reports. It shall optionally display only the deviations from the standard on schematic pages, reproducing the "only if different" convention of folio 1. | 1, 101, 102 | connection/wire |
| REF-009 | … support sparse, section-based folio numbering (1-3, 100-199, 200-299, 900-999). It shall not renumber unless explicitly requested and shall allow gaps (104, 105 absent). | 2, all | page/frame |
| REF-010 | … define **sections** (number, folio range, title). It shall generate section-divider pages and their page descriptions automatically. | 100, 200, 900 | page/frame |
| REF-011 | … assign every page a page type: title page, TOC, topology/overview, multi-line schematic, fluid schematic, assembly overview, section divider, parts list, panel layout, and generated reports. | all | page/frame |
| REF-012 | … generate the table of contents (folio, title, location) automatically, keep it up to date on every change, and paginate it ("Table of Contents 1, 2, …"). | 2 | reports |
| REF-013 | … provide a frame form with a configurable column grid (here 17 columns numbered **from 0**) and row grid (8 rows A-H), with column and row labels on top and left only. | 3, 101-206, 902 | page/frame |
| REF-014 | … compute every cross-reference text from the model in a configurable format, here `<folio>-<row><column>`, with "(…)" for aux→main references. | 3, 101-106, 201-206 | cross-ref |
| REF-015 | … provide **interruption points** (named, direction arrow) that pair across pages automatically by name and show the counterpart's location below the arrow. | 101, 102, 103, 106 | cross-ref |
| REF-016 | … treat interruption-point arrow direction (▷ / ◁) as a signal-direction property independent of page side. | 103, 106 | cross-ref |
| REF-017 | … support **one-to-many** interruption points and report unpaired or ambiguous interruption points. | 101-106 | checks |
| REF-018 | … support **external connection points** to objects in other projects or cabinets (J-E-206, JE-N1000-03), without in-project cross-reference but with a structured target (project, location, device, connection). | 3, 101 | cross-ref |
| REF-019 | … draw structure boxes (dotted or dash-dot) with a caption marking objects outside the current location, and assign the enclosed objects that location. | 3, 101 | symbol |
| REF-020 | … implement **main function / auxiliary function** representations of one device on several pages (overview, bus topology, power supply, fluid detail), with automatic mutual cross-references. | 3, 106, 201-206 | device |
| REF-021 | … anchor cross-references to the visible symbol (configurable: symbol origin, label, centre) so that a reference never points to an empty grid cell. | 201 ("3-B0") | cross-ref |
| REF-022 | … compute device tags from structure (=function, +location, -product). It shall keep **no stored copy** that can diverge: all pages, reports and the layout read the same computed value. | 3, 901, 902 | numbering |
| REF-023 | … support page-based device numbering `-<folio><letter><counter>` (e.g. -101F1, -102G1, -106W1, -201U3) as a configurable numbering rule. | 101-202 | numbering |
| REF-024 | … renumber devices on page renumbering or move, with preview, and update all references and reports atomically. | 3, 901 (-3W1 vs -4W1) | numbering |
| REF-025 | … allow **fixed ("frozen") tags** for template/standard devices (S51, F55, T55, E55, S56, E56, 1X1). It shall flag them in a numbering report, and never mix schemes without warning. | 101 | numbering |
| REF-026 | … hold one canonical spelling of each structure identifier (e.g. `+J-E-2416`). It shall render it consistently in title blocks, TOC, tags and terminal strips. | 103, 901 | numbering |
| REF-027 | … support IEC 81346-2 letter codes (2009 and 2019 sets) with a project-level choice and a check for misapplied letters (e.g. T for a thermostat). | 101 | numbering |
| REF-028 | … represent devices as black boxes with named connection points (PSU L/N/PE/+/+/−/−, MICO 24VDC/GND/13/14/Out1-4, CPX 1-5/FE). | 102, 106 | symbol |
| REF-029 | … allow **duplicate printed connection-point designations** (two "N" on a 1P+N MCB, two "+" on a PSU) by keeping a unique internal connection-point index, so wire lists stay unambiguous. | 101, 102 | device |
| REF-030 | … support connection-point alias designations (e.g. "1 / L1", "2 / T1"). | 102 | device |
| REF-031 | … provide IEC 60617 symbols used here: 2-pole switch with manual actuator, 1P+N and 1P MCB with thermal and magnetic trip, thermostat contact, position switch, lamp, heater, terminal, protective earth, functional earth, plug pin, cable marker, wire marker, junction dot. | 101-106 | symbol |
| REF-032 | … draw mechanical linkages as dashed lines tied to the device's contacts, and never as free graphics. | 101, 102, 203 | symbol |
| REF-033 | … provide ISO 1219-1 fluid-power symbols: 5/2 solenoid / spring valve with ports 1-5, double-acting actuator, process (2-way) valve, mechanical linkage. | 203-206 | fluid |
| REF-034 | … represent **T-junctions** as explicit model objects. Each junction has a display style (dot or oriented bevel) and a defined **connection sequence** (which device terminal each wire goes to). | 101, 102, 103 | connection/wire |
| REF-035 | … store wire properties (cross-section, colour, wire number, type, length) **on the connection** and render wire markers from those properties, not as free symbols. | 101, 102 | connection/wire |
| REF-036 | … derive **potentials** (nets) from connectivity, interruption points, terminal bridges and plug-in jumpers. It shall assign each potential a name and type (L, N, PE, +24 V, 0 V, signal). | 101-106 | potential |
| REF-037 | … allow several interruption-point names on one potential (24VDC-1 = 24VDC-AIR1…4). It shall report the potential's canonical name in lists. | 103, 106 | potential |
| REF-038 | … model **intentional bonding** between potentials (0 V to PE at one point, PELV) as an explicit object so that potential checks accept it and can verify there is exactly one bond. | 102, 103 | potential |
| REF-039 | … model the PE path through rail-mounted PE terminals and the mounting plate, so PE continuity is checkable even when not drawn as wires. | 101, 103 | potential |
| REF-040 | … provide terminal strips as first-class objects with unique terminal designations, terminal order, levels and connection points per terminal part, bridges (plug-in, wire) and spare terminals. | 101, 103 | terminal |
| REF-041 | … reject or flag duplicate terminal designations within a strip and terminals without designation. | 103 | checks |
| REF-042 | … allow one terminal strip to be drawn in several places (24 V side and 0 V side of -X1031). It shall use one strip object without duplicating the strip tag. | 103 | terminal |
| REF-043 | … support terminal groups and function texts ("Pneumatics", "Spare") per terminal or terminal range. | 103 | terminal |
| REF-044 | … support multi-connection terminals (PT 2.5-TWIN: 3 connection points) and compute the number of physical terminals from parts, not from symbol instances. | 103, 901 | terminal |
| REF-045 | … generate terminal-strip diagrams/plans for 1X1, X1031, X1032, with internal/external side, bridges, cables and targets. | (missing) | reports |
| REF-046 | … provide cables as devices with conductors (core colour or number, cross-section, shield), a cable type from the part, and a cable marker symbol that assigns cores to connections. | 106 | cable |
| REF-047 | … use IEC 60757 colour codes for cores (BN, not "BR") and validate core colours against the cable part's core definition. | 106 | cable |
| REF-048 | … support single-line bus cables (2-core shielded Profibus) drawn as one line. | 3 | cable |
| REF-049 | … generate cable overview and cable diagrams (cores, sources, targets, length). | (missing) | reports |
| REF-050 | … support composite devices with **several parts** per device (e.g. -201U1 = 195740 + 550208), each with a quantity. | 201, 901 | parts/BOM |
| REF-051 | … support **configured assemblies** with a configurator order code (Festo type codes) that own sub-modules. The BOM can list either the assembly or its components. | 201, 202, 3 | parts/BOM |
| REF-052 | … provide a parts database: manufacturer (normalised master record), part number, type designation, description, technical data, ECLASS/ETIM class, dimensions, connection-point pattern and macro. | 901 | parts/BOM |
| REF-053 | … generate the parts list with position, quantity aggregation, unit, manufacturer, part number, type, description, and the list of tags and folios. Duplicate rows (X1031 twice) shall not appear. | 901 | parts/BOM |
| REF-054 | … include enclosure, mounting plate, ducts, rails, end brackets, bridges and markers in the BOM via panel-layout objects and accessory lists. | 902 | parts/BOM |
| REF-055 | … flag devices without part assignment (E56, S56) and parts with incomplete data (empty description). | 901 | checks |
| REF-056 | … keep separate fields for description, technical characteristics (rating "10 A C", "20 A") and function text. | 901 | parts/BOM |
| REF-057 | … model PLC-type bus nodes (CPX-FB13 as PROFIBUS DP slave) with station address, I/O image and per-channel addresses. It shall support "TBD" placeholders that are listed as open items. | 3, 203-206 | PLC/bus |
| REF-058 | … assign PLC outputs to valve solenoids (Q-addresses) with function texts, including spare channels. | 203-206 | PLC/bus |
| REF-059 | … validate address assignment against the module's address scheme (2 addresses per valve position; which coil maps to which bit). | 203-206 | checks |
| REF-060 | … generate a PLC / I/O overview (per node and module: address, symbol, function, used/spare). It shall export it for TIA Portal or Codesys (AML/CSV). | 203-206 (missing) | PLC/bus |
| REF-061 | … generate bus topology from device bus ports, with termination state ("Switch to ON") as a connector property and a check that both segment ends are terminated. | 3 | PLC/bus |
| REF-062 | … give valve-terminal sub-components (valve positions, individual valves) their own tags as children of the assembly. | 201-206 | device |
| REF-063 | … model fluid connections (tubes) with tag, outer diameter, material and colour, and pneumatic ports with designations 1-5. | 203 | fluid |
| REF-064 | … link field devices (process valves TA05-V-xx and their actuators) to the controlling valve via function text and mechanical linkage, with their own location aspect. | 203 | fluid |
| REF-065 | … support spare reservations (spare valve positions, spare terminals, spare MICO channels) as states that reports count and list. | 103, 204-206 | device |
| REF-066 | … produce a 2D panel layout where every placed object **is** a schematic device (one object, several representations), using real part dimensions from the part macro. | 902 | panel layout |
| REF-067 | … report devices not placed in the panel layout and layout objects without schematic device. | 902 | checks |
| REF-068 | … place cable ducts and DIN rails as parametric layout objects (type, length) that contribute to the BOM. | 902 | panel layout |
| REF-069 | … show a part-data box on layout pages (enclosure, backplate) bound to the enclosure and plate objects. | 902 | panel layout |
| REF-070 | … render layout pages to a stated scale (e.g. 1:5 / 1:10) or "not to scale" explicitly in the title block. | 902 | panel layout |
| REF-071 | … define title blocks as form templates with cells bound to project properties, page properties or computed values (page, prev, next, creation date, saved date). | all | title block/forms |
| REF-072 | … drive title-block **Description** from one project property. A per-page override must be explicit and shall be reported. | 101 | title block/forms |
| REF-073 | … compute Prev/Next folio numbers, leaving them empty on the first and last pages. | 1, 902 | title block/forms |
| REF-074 | … show the function (=) and location (+) aspects of the page in dedicated title-block cells. | all | title block/forms |
| REF-075 | … manage **project revisions** (index format, date, author, description, list of changed pages). The title block shows the managed index in a single format. | all | revision |
| REF-076 | … track page-level change status (last changed in revision X) and optionally add revision clouds or markers. | 101, 106 | revision |
| REF-077 | … record author and checker separately, and optionally enforce checker ≠ author before release. | all | revision |
| REF-078 | … format dates using a project setting (dd-mm-yyyy here; ISO 8601 optional). | all | title block/forms |
| REF-079 | … embed fonts and support vector (SVG/PDF) logos in forms. | all | title block/forms |
| REF-080 | … export PDF with bookmarks per page and section, hyperlinked cross-references, `/PageLabels` equal to folio numbers, document metadata, and PDF/A as an option. | all | reports |
| REF-081 | … generate a wire/connection list (source, target, potential, cross-section, colour, wire number, length) for manufacturing. | (missing) | reports |
| REF-082 | … generate wire numbers by a configurable rule and print them on connections. | (missing) | connection/wire |
| REF-083 | … flag unconnected connection points (e.g. MICO 13-14), with severity "info" when the point is marked "intentionally unused". | 102 | checks |
| REF-084 | … run an electrical rule check that flags **single-pole switching contacts in the neutral conductor** without a corresponding switch in the line conductor. | 101 | checks |
| REF-085 | … check PE cross-section against the associated phase conductor (copper: S ≤ 16 mm² → PE ≥ S) when sizes are rule-derived. | 1, 101 | checks |
| REF-086 | … check clamp capacity (number of wires × cross-section per connection point) against the part's connection-point data. | 102 | checks |
| REF-087 | … check that overview and detail representations of an assembly agree on module and valve counts and on positions. | 202 vs 206 | checks |
| REF-088 | … check text height against a minimum print size (e.g. ≥ 1.8 mm cap height on A4) and report undersized texts. | 101, 102, 106 | checks |
| REF-089 | … apply text formats (font, size, alignment) to property texts via styles, so all device tags of one kind render identically. | 101, 202 | symbol |
| REF-090 | … normalise manufacturer names via a manufacturer master list (short code, full name). | 901, 902 | parts/BOM |
| REF-091 | … support a part-alternatives field (UK5N / USLKG5) only as structured alternatives, never as free text in the part number. | 901 | parts/BOM |
| REF-092 | … show part data at the symbol (manufacturer, part number, rating) through property placement driven by the assigned part. | 101-106 | device |
| REF-093 | … support device-level "process tag" / function text (TA05-V-15) separate from the reference designation. | 203 | device |
| REF-094 | … provide a project-wide "open items" list (TBD placeholders, missing parts, unconnected points, unplaced devices). | 3, 901, 902 | checks |
| REF-095 | … import this project from QElectroTech (.qet). It shall map folio reports to interruption points, master/slave to main/aux, element info to device or part data, and conductor properties to wire properties. | all | project |
| REF-096 | … reproduce the printed sheet geometry of this set (frame, grid, title block) from a form definition, within ±0.2 mm. | all | page/frame |
| REF-097 | … export the panel layout and BOM data needed for later 3D (enclosure, plate, ducts, rails, device macros). | 902 | panel layout |
| REF-098 | … support devices with both electrical and fluid connection points (CPX node with 7/8" power, DP port, pneumatic ports) in one device object. | 3, 106, 201 | device |
| REF-099 | … support multi-wire assignment to one terminal point and daisy-chaining on device terminals, preserving order. | 101, 102 | connection/wire |
| REF-100 | … support tables with header shading, column widths and pagination for all report pages, with consistent row height (6.52 mm here). | 2, 901 | reports |
| REF-101 | … mark the electrical environment of each potential (AC/DC, voltage) so colour and size rules and voltage-class checks work (e.g. 230 V vs 24 V in the same strip). | 1, 101-106 | potential |
| REF-102 | … provide a potential overview report (potential name, type, pages, connected devices). | 101-106 | reports |
| REF-103 | … provide a device-tag overview (structure-identifier overview) report listing =, + and device tags. | all | reports |
| REF-104 | … keep the TOC "Location (+)" column bound to each page's location. | 2 | reports |
| REF-105 | … support drop-shadow box styles and line widths 0.13 / 0.18 / 0.26 / 0.42 / 0.52 / 1.3 mm in forms, to reproduce the house style. | 1, 201, 902 | title block/forms |
| REF-106 | … record per-device "technical data" such as switch rating (40 A), MCB characteristic (C10, C4, B16), thermostat setpoint (15 °C) and channel current setting (6 A), and render them at the symbol. | 101, 102 | device |
| REF-107 | … model the supply origin (external source, upstream protection, prospective short-circuit current) at the external connection, for later protection checks. | 101 | project |
| REF-108 | … support a project-level "drawing identification" separate from project reference and file name (<drawing id> vs <project ref>). | 1, all | project |

---

## 3. Inconsistencies and defects (verified and extended)

The severity scale has three levels:
- **S** (safety/function): would produce a wrong or unsafe cabinet.
- **M** (manufacturing/data): would produce wrong orders, wiring or documentation.
- **C** (cosmetic/format).

| ID | Severity | Defect | Evidence (folio) | Preventing feature |
|---|---|---|---|---|
| D-01 | M | Part list shows **-4W1 / -4X1**, but folio 3 shows **-3W1 / -3X1** (folio column already says 3). Root cause: QET stores the evaluated label as element info, and the stored copy went stale after a page renumber (e.g. folio 4 → 3). | 3, 901 | Tags computed, never stored (REF-022); atomic renumber (REF-024) |
| D-02 | M | Layout shows **T51**; the schematic has **T55**. Layout tags `X1031`/`X1032` lack the location prefix. Layout objects are free rectangles, not devices. | 101, 902 | One device, several representations (REF-066) |
| D-03 | M | Four tag schemes mixed: fixed (S51, F55, T55, E55, S56, E56, 1X1); page-letter-counter (-101F1, -102G1); letter-page-counter with location (+JE2416-X1031 = X + 103 + 1); location spelled "+J-E-2416" vs "+JE2416". | 101-103, 901 | Numbering rules per project, frozen-tag flag, canonical identifiers (REF-023/025/026) |
| D-04 | M | Part list incomplete: missing **-106W2, -201U4 … -201U9**, the second part 550208 of -201U1, the 28 valves 571334 (only text "4x" on two rows), enclosure NSYPLA1074G, backplate NSYPMM107, ducts, rails, end brackets, bridges. **+JE2416-X1031 appears twice** and there is no quantity column. | 201, 202, 902, 901 | BOM from model with quantities (REF-050/053/054) |
| D-05 | M | Revision field has four formats: "1" (f. 1), "V1.0" (13 pages), "V1" (f. 100), "V2.0" (f. 101, 106). No revision table. | all | Managed revisions (REF-075/076) |
| D-06 | C | Description: folio 101 "Control Cabinet Expansion", all others "Expansion Control Cabinets". The cover says "Expansion Control Cabinets <end user>". | 1, 101 | Title block bound to one project property (REF-072) |
| D-07 | C | *(Brief's defect 7 withdrawn: an empty Next on the last page is correct.)* Real gap: the PDF has **no bookmarks, no hyperlinks, physical page labels**, and a raster logo. | all | PDF export features (REF-080) |
| D-08 | M | Wire sizes and colours rely on the free-text rule "if not indicated, standard size". Only 4 marker groups exist for about 60 wires. No machine-readable wire data. | 1, 101, 102 | Potential-type → wire-property rules (REF-006/007/035) |
| D-09 | M | No wire/connection list, terminal diagram, cable diagram, PLC overview, potential overview or wire numbers. | — | Report generators (REF-045/049/060/081/082/102) |
| D-10 | M | **-202U2 overview shows 5 valves**. The detail page (206) and the BOM show 4. The 4-position sub-base cannot hold 5. Overview count 29 vs detail 28. | 202, 206, 901 | Overview generated from assembly model; consistency check (REF-087) |
| D-11 | M | Second terminal of group "3+" on X1031 has **no designation**. | 103 | Terminal designation mandatory (REF-041) |
| D-12 | M | **Duplicate terminal designations**: "1+" ×4, "2+" ×2, "4+" ×2, "1-" ×4, "2-"/"3-"/"4-" ×2, "PE" ×5. Terminal plans and wire lists become ambiguous. | 103 | Unique designations and connection points per terminal part (REF-040/044) |
| D-13 | **S** | **Thermostat T55 and door switch S56 are drawn in the neutral conductor.** F55:N → T55 → E55:N and F55:N → S56 → E56. E55:L and E56 are fed directly from F55:2. When the contacts are open, the loads stay live. Single-pole switching devices in the neutral are not permitted in IEC 60364-based wiring rules (NEN 1010 in NL) **(clause number unverified)**. It could also be a pole-labelling error on F55. | 101 | N-switching rule check on potential types (REF-084) |
| D-14 | C | Main-function xref of -201U1 on 201 reads **"3-B0"**. Column 0 on folio 3 is empty: the label is in A1 and the bus port in B2. The xref anchors to an invisible symbol origin. | 3, 201 | Xref anchor policy (REF-021) |
| D-15 | M | Profibus node address **"TBD"**. No master, baud rate or segment drawn. Termination appears only as a free text "Switch to ON". | 3 | Bus model and open-items list (REF-057/061/094) |
| D-16 | M | CPX pin 3 is labelled "PE" and fed from the PE strip. Festo designates this pin as FE (functional earth) **(unverified)**. A separate FE screw is also wired to PE. Functional and protective earth are mixed. | 106 | Potential types FE vs PE; part-defined pin functions (REF-036/052) |
| D-17 | C | Core colour **"BR"** is not an IEC 60757 code (BN). The cable's actual core colours per pin are **unverified**. Common 7/8" 5-pin practice is 1 BK, 2 BU, 3 GNYE, 4 BN, 5 WH, which would swap 4 and 5 relative to the drawing **(unverified)**. | 106 | Cable part with core definitions; colour code validation (REF-046/047) |
| D-18 | C | Typos and format errors: "Yellow / 4 Green" (×3), "50hz", "15c", "ULSKG5" (Phoenix's PE terminal is USLKG 5 **(unverified)**), "Murr Elektronik" (Murrelektronik), "Schneider" vs "Schneider Electric", "Eaton / Holec". | 1, 101, 901, 902 | Manufacturer master data, unit formatting, spell check (REF-090) |
| D-19 | M | The incoming supply from J-E-206 to 1X1 has **no cable** (tag, type, cores, size). -3W1 has no length. Cable lengths of -106W1/W2 are only implied by the type code ("K-2", likely 2 m **(unverified)**). | 3, 101, 106 | Cables as devices; open items (REF-046) |
| D-20 | C | Junction notation is mixed: dots (101 F55 outputs, 103 strips) and bevelled corners (101 top bus, 102, 103 0 V bus). The bevels are side effects of overlapping conductors. The wiring sequence is visible but not modelled. | 101-103 | Explicit T-node objects with sequence (REF-034) |
| D-21 | M | **PE continuity not drawn**: 1X1:PE goes to an earth symbol on 101. The PE distribution X1032 (103) is not connected to it, except implicitly via DIN rail and mounting plate. | 101, 103 | PE rail model; PE continuity check (REF-039) |
| D-22 | M | 0 V is bonded to PE by an ordinary wire named "PE-0VDC". In a net model, 0 V and PE then merge into one potential without a declared bond. | 102, 103 | Explicit bonding object (REF-038) |
| D-23 | C | MICO 4.10 diagnostic contact 13-14 is unconnected. No fault signal goes to the PLC. | 102 | Unconnected-point check (REF-083) |
| D-24 | C | One net carries 5 interruption-point names (24VDC-1, 24VDC-AIR1…4), and similarly 0VDC and PE. The potential name is ambiguous in lists. | 103, 106 | Canonical potential name (REF-037) |
| D-25 | M | CPX supply pairs crossed: 24V El ← 24VDC-AIR2 with 0V El ← 0VDC-AIR1, and 24V Val ← 24VDC-AIR1 with 0V Val ← 0VDC-AIR2. All four feeds come from one MICO channel (6 A). Valve supply cannot be switched off separately from electronics. This is design-review material, not necessarily wrong. | 103, 106 | Potential-pair labelling; load and protection summary per channel |
| D-26 | M | Valve addresses use only **odd bits** (Qn.1/3/5/7). Each valve position of an 8-output module occupies 2 addresses. If Festo assigns the lower (even) address to pilot solenoid 14, the drawn addresses point to the unused coil 12 **(convention unverified)**. | 203-206 | Address scheme validation from the module's device description (REF-059) |
| D-27 | M | The 28 valves have **no device tags**. The 8 process valves and actuators (TA05-V-xx) have neither tags nor location; the function text is the only link. | 201-206 | Child tags; process-tag property; field location (REF-062/064/093) |
| D-28 | M | Fluid model incomplete: no supply to port 1, no exhaust or silencer on 3/5, no tube sizes, no air preparation, no pressure. | 203 | Fluid connection properties (REF-063) |
| D-29 | C | Device-tag text sizes vary (8.88 pt for S51, S56, E55, E56, -202U1, -202U3 and the 203-206 tags; 9.62 pt elsewhere). | 101, 202, 203 | Text styles (REF-089) |
| D-30 | M | Title block "Customer: <end user>", but the cover "Company / Customer: <customer>" (<end user> is the **end user**). | 1, all | Separate customer and end-user roles (REF-004) |
| D-31 | C | Texts at 3.70 pt printed (≈ 1.3 mm em, ≈ 0.95 mm cap height): wire markers, MCB pin numbers, core colours. Several 5.18 pt texts are on the edge of legibility. Rotated layout labels are down to 1.88 pt. | 101, 102, 106, 902 | Minimum text height check (REF-088) |
| D-32 | M | Layout incomplete and not to scale. F55, E55, E56, S56 and -201U9 are absent. No DIN rails. The unlabelled lower duct has unknown type. | 902 | Layout from devices, unplaced-device report (REF-066/067/070) |
| D-33 | C | PDF page labels are 1…18, so a viewer's "page 5" means folio 101. Text extraction loses spaces in some labels ("SK3110.000"), which hurts search. | all | PDF export with page labels and proper text spacing (REF-080) |
| D-34 | M | Author = Checker = "<initials>" on every page. No four-eyes check is recorded. | all | Release workflow (REF-077) |
| D-35 | C | BOM "Textual description" misused: ratings ("10A C", "20A"), wrong noun ("Connector" for cable -106W1), redundant article number ("CPX DP Connection 195740"), generic "Protective Gear". | 901 | Separate description and technical-data fields from the part DB (REF-056) |
| D-36 | M | -201U1 shows the configurator order code on folio 3, part 195740 in the BOM, and two parts on 201. The valve terminal ordering basis (order codes vs components) is undefined. | 3, 201, 901 | Configured assembly model (REF-051) |
| D-37 | C | Cover cites "IEC60757 EN60204-1", but DBU (dark blue) and WH for 0 V are company conventions. IEC 60757 has no DBU code, and EN 60204-1 13.2.4 recommends BU for DC control. IEC 60445 requires RD/WH for DC line conductors (see `standards.md` §6). | 1 | Colour profiles naming their basis (REF-006) |
| D-38 | M | Under the "standard size" rule, PE wires get 1.5 mm² (mains) or 0.75 mm² (control). That is less than the 2.5 mm² phase conductors at S51/1X1 if applied to 1X1:PE. This conflicts with the PE sizing rule (S ≤ 16 mm² → PE ≥ S) **(rule per IEC 60364-5-54 / EN 60204-1 Table 1; verify)**. | 1, 101 | PE sizing check (REF-085) |
| D-39 | C | Folio 102 lists incoming N-1 above L-1, while folio 101 lists L-1 above N-1. The order is not systematic. | 101, 102 | Interruption-point sort/arrange tools |
| D-40 | M | -202U1 is labelled 550208 "CPX-M-GE-EV-S-7/8-5POL" (a CPX interlinking block). It sits after the MPA supply plate -201U9 in the middle of the valve manifold (break marks 201→202, layout row order). A CPX block cannot sit inside the MPA section, so either the part number or the topology is wrong **(Festo architecture unverified)**. BOM description: "Halfway PSU". | 201, 202, 902 | Configured-assembly validation against vendor configurator data (REF-051) |
| D-41 | C | Same physical assembly tagged from two pages (-201U1…U9 and -202U1…U3) because of page-based numbering. The assembly identity is lost. | 201, 202 | Assembly-level tag with child numbering (REF-062) |
| D-42 | C | Terminal strip X1031 is drawn twice with its full tag (A3 and A10), and its BOM row appears twice. | 103, 901 | One strip, several drawn segments (REF-042) |

---

## 4. Golden project specification (acceptance test)

**Acceptance principle.** ElektraWatt shall hold this project as a model, defined by the tables below. From that model alone it shall regenerate:
- the 18 folios of the reference (same page numbers, page types, frame geometry, symbol arrangement, texts and cross-references);
- the additional report pages listed in §4.14.

The output must avoid D-01 … D-42. Where the reference is ambiguous, the golden value is a **proposal** (marked ▲) that the user must confirm (see Open questions).

### 4.1 Project properties

| Property | Golden value |
|---|---|
| Project reference | <project ref> |
| Drawing identification | <drawing id> |
| Object / cabinet | J-E-2416 |
| Project description | Expansion Control Cabinets |
| Long description (cover) | Expansion Control Cabinets <end user> |
| Customer | <customer> ▲ (cover); the title-block "Customer" cell changes to *End user* or shows <customer>, per user decision |
| End user | <end user>, <address>, <postcode, city>, Netherlands |
| Manufacturing year | 2024 |
| Supply | 1 ph, N, PE, 230 V AC, 50 Hz (render "1 x 230 VAC + N + PE, 50 Hz"); earthing system TN-S ▲ |
| Control voltage | 24 V DC (PELV, 0 V earthed at -103X2) |
| Rated consumption | 6 A |
| Engineer / author | <initials> (<engineer>) |
| Checker | ▲ (must differ from author for release) |
| Created / edited | 2024-06-10 / 2024-07-12 (display dd-mm-yyyy) |
| Revisions ▲ | V1.0 2024-06-10 "Initial"; V2.0 2024-07-12 "Changes on 101, 106". Title block shows the project revision **V2.0** on all pages. Page status: 101 and 106 last changed in V2.0. |
| Colour profile | "the pilot user 24 V DBU/WH" (§4.6) |
| Size profile | control 0.75 mm²; mains 1.5 mm² |
| Company | The pilot panel builder (logo, confidentiality notice text) |

### 4.2 Structure identifiers

| Aspect | Identifier | Meaning | Used by |
|---|---|---|---|
| `=` function | **=P01** | Higher-level function / plant part (meaning ▲) | all pages, all devices |
| `+` location | **+J-E-2416** | This cabinet (single canonical spelling; "+JE2416" retired) | all in-cabinet devices |
| `+` location (external) | **+J-E-206** | Upstream cabinet, source of supply and Profibus | external connection points on 3 and 101 |
| External device | **JE-N1000-03** | Device or terminal in +J-E-206 (meaning ▲) | 3, 101 |
| `+` location (field) ▲ | **+FIELD** (or plant area "TA05") | Process valves TA05-V-xx and actuators | 203 |

Full designation example: `=P01+J-E-2416-102G1`. Tags on pages are printed **without** `=P01+J-E-2416` when they equal the page's structure (EPLAN-style identifier suppression). The title block carries `=P01` and `+J-E-2416`.

### 4.3 Pages

| Folio | Page type | Page description (title block) | Section | Page revision status |
|---|---|---|---|---|
| 1 | Title page (form "the pilot user cover") | Title Page | — | V1.0 |
| 2 | Table of contents (report) | Table of Contents 1 | — | generated |
| 3 | Bus topology (single-line schematic) | Bus Topology - Profibus | — | V1.0 |
| 100 | Section divider | =P01+J-E-2416 - Section 1 - Page 100 - 199 - Wiring Definition | 1 (100-199) | generated |
| 101 | Multi-line schematic | Power Supply 1 - Main Current | 1 | **V2.0** |
| 102 | Multi-line schematic | Power Supply 2 - Control Current | 1 | V1.0 |
| 103 | Multi-line schematic | Power Supply 3 - Control Current Distribution | 1 | V1.0 |
| 106 | Multi-line schematic | Power Supply 4 - Pneumatics | 1 | **V2.0** |
| 200 | Section divider | =P01+J-E-2416 - Section 2 - Page 200 - 299 - Pneumatics | 2 (200-299) | generated |
| 201 | Assembly overview (fluid) | Valve Manifold Overview 1 | 2 | V1.0 |
| 202 | Assembly overview (fluid) | Valve Manifold Overview 2 | 2 | V1.0 |
| 203 | Fluid-power schematic (ISO 1219) | Valve Manifold 1 | 2 | V1.0 |
| 204 | Fluid-power schematic | Valve Manifold 2 | 2 | V1.0 |
| 205 | Fluid-power schematic | Valve Manifold 3 | 2 | V1.0 |
| 206 | Fluid-power schematic | Valve Manifold 4 | 2 | V1.0 |
| 900 | Section divider | =P01+J-E-2416 - Section 9 - Page 900 - 999 - Nomenclature | 9 (900-999) | generated |
| 901 | Parts list (report) | Partlist 1 | 9 | generated |
| 902 | Panel layout (2D) | Cabinet Layout | 9 | V1.0 |

All pages carry `=P01` and `+J-E-2416`. Prev/Next are computed: folio 1 has Prev empty, Next 2; folio 902 has Prev 901, Next empty.

### 4.4 Devices

**Tag columns:**
- **Golden A (faithful):** keeps the user's visible tags and fixes only the defects. **This is the acceptance target.**
- **Golden B (normalised):** page-based tags for everything. It tests the renumbering function.

| # | As drawn | Golden A | Golden B | IEC 81346-2:2019 class (see `standards.md` §1.5) | Function | Main representation | Other representations |
|---|---|---|---|---|---|---|---|
| 1 | S51 | S51 (frozen) | -101Q1 ▲ | QB | Main switch, 2-pole, 40 A | 101-B2 | 902 |
| 2 | -101F1 | -101F1 | -101F1 | FC | MCB 1P+N C10, feeds PSU | 101-B8 | 902 |
| 3 | F55 | F55 (frozen) | -101F2 | FC | MCB 1P+N C4, feeds climate and light | 101-B12 | 902 (missing in reference) |
| 4 | T55 | T55 (frozen) | -101B1 ▲ | BT | Thermostat, setpoint 15 °C | 101-C12 | 902 (as T51 in reference) |
| 5 | E55 | E55 (frozen) | -101E1 | EB | Enclosure heater | 101-E12 | 902 (missing in reference) |
| 6 | S56 | S56 (frozen) | -101B2 ▲ | BG | Door switch | 101-D14 | 902 (door) |
| 7 | E56 | E56 (frozen) | -101E2 | EA | Cabinet lamp | 101-E14 | 902 |
| 8 | 1X1 | 1X1 (frozen) | -101X1 | XD | Incoming supply terminals | 101-D2 | 902 |
| 9 | -102G1 | -102G1 | -102G1 | TC (2019) / G as drawn | PSU 1AC/24 V DC 20 A | 102-B6 | 902 |
| 10 | -102F1 | -102F1 | -102F1 | FC | MCB 1P B16 in +24 V | 102-C7 | 902 |
| 11 | -102F2 | -102F2 | -102F2 | FC | 4-channel electronic load distribution | 102-E7 | 902 |
| 12 | +JE2416-X1031 | **-X1031** at +J-E-2416 | -103X1 | XD | 24 V / 0 V distribution terminals | 103-A3 (24 V), 103-A10 (0 V) | 902 |
| 13 | +JE2416-X1032 | **-X1032** at +J-E-2416 | -103X2 | XE | PE distribution terminals | 103-F3 | 902 |
| 14 | -106W1 | -106W1 | -106W1 | WD | Power cable to -201U1 (7/8") | 106-F5 | — |
| 15 | -106W2 | -106W2 | -106W2 | WD | Power cable to -202U1 (7/8") | 106-F11 | — |
| 16 | -3X1 | -3X1 | -3X1 | XG | Profibus plug, termination ON | 3-B2 | — |
| 17 | -3W1 | -3W1 | -3W1 | WG | Profibus cable to +J-E-206 | 3-E2 | — |
| 18 | -201U1 | -201U1 | -201KE1 ▲ | KE | CPX bus node (DP) + power interlinking block | 201-E1 | 3-B2 (bus port), 106-G3 (power) |
| 19 | -201U2 | -201U2 | -201U2 | — ▲ | MPA pneumatic interface | 201-E2 | 902 |
| 20-25 | -201U3 … -201U8 | same | same | KH/QM sub-base ▲ | 4-position electronic module and sub-base | 201-E4 … E14 | 203-B4, 203-B12, 204-B4, 204-B12, 205-B4, 205-B12; 902 |
| 26 | -201U9 | -201U9 | -201U9 | ▲ | MPA supply plate (533354) | 201-E15 | 902 (**add**) |
| 27 | -202U1 | -202U1 | -202U1 | ▲ | Additional 7/8" power supply ("halfway PSU") | 202-E1 | 106-G9 |
| 28 | -202U2 | -202U2 | -202U2 | ▲ | 4-position module and sub-base | 202-E3 | 206-B4 |
| 29 | -202U3 | -202U3 | -202U3 | ▲ | Supply/end plate (533354) | 202-E4 | 902 |
| 30-57 | (untagged) | **-201U3-QM1 … QM4**, …, **-202U2-QM1 … QM4** ▲ | same | QM | 28 × 5/2 solenoid valve 571334 | 203-206 cells | 201/202 front view |
| 58-65 | (untagged) | **TA05-V-15, -33, -18, -17, -36, -38, -16, -37** as external devices in +FIELD ▲ | same | QM (process valve) + MM (actuator) | Process valves with double-acting actuators | 203-G | — |
| 66 | (earth symbol) | PE connection to mounting plate / PE bar **-XPE** ▲ | same | WE | Protective earth reference | 101-D5 | 902 (add PE bar) |
| 67 | (cabinet) | **-UC1** ▲ enclosure NSYPLA1074G | same | UC | Enclosure | 902 | — |
| 68 | (backplate) | mounting plate NSYPMM107 (child of -UC1) | same | UC | Mounting plate | 902 | — |
| 69-73 | (ducts) | 5 duct segments 40×60 ▲ | same | UA/UC | Wiring ducts | 902 | — |
| 74+ | (rails) | DIN rails TS35 ▲ (3 rows) | same | UA | Mounting rails | 902 (add) | — |

Representation cells are the approximate symbol cells. The cross-reference cells that must match exactly are in §4.13.

**Connection points** (printed designation, with internal index where duplicated):

| Device | Connection points |
|---|---|
| S51 | 1, 2 (pole 1); 3, 4 (pole 2) |
| -101F1, F55 | 1, 2 (L pole); N#1 (in), N#2 (out), both printed "N" |
| T55 | 5, 3 (as drawn; real Rittal terminals ▲) |
| E55 | L, N, PE |
| S56 | ▲ (e.g. 13, 14; drawn without designations) |
| E56 | ▲ (X1, X2 per IEC 60445; drawn without designations) |
| -102G1 | L, N, PE, +#1, +#2, −#1, −#2 |
| -102F1 | 1 (alias L1), 2 (alias T1) |
| -102F2 | 24VDC#1, 24VDC#2, GND#1, GND#2, 13, 14, Out1, Out2, Out3, Out4 |
| -201U1 | power X1 pins 1 (0V Val), 2 (0V El), 3 (FE/"PE"), 4 (24V El), 5 (24V Val); FE screw; DP port (Sub-D 9) |
| -202U1 | power pins 1-5 (as -201U1); FE screw |
| Valves QMn | coil 14 (electrical, PLC output); ports 1, 2, 3, 4, 5 |

### 4.5 Parts (golden BOM; one line per part with quantity)

| Pos | Qty | Unit | Manufacturer | Type / part number (as drawn) | Description (golden) | Tags | Note |
|---|---|---|---|---|---|---|---|
| 1 | 1 | pc | Eaton (Holec) | 533356 | Main switch 2-pole 40 A | S51 | Number similar to Festo 5333xx; **verify** |
| 2 | 1 | pc | Schneider Electric | A9P52610 | MCB 1P+N, C, 10 A | -101F1 | (range unverified) |
| 3 | 1 | pc | Siemens | 5SY4204-7 | MCB 1P+N, C, 4 A | F55 | |
| 4 | 1 | pc | Schneider Electric | A9F78116 | MCB 1P, B, 16 A | -102F1 | DC suitability ▲ |
| 5 | 1 | pc | Phoenix Contact | 2904602 | Power supply 1AC / 24 V DC / 20 A | -102G1 | (QUINT4 range, unverified) |
| 6 | 1 | pc | Murrelektronik | MICO 4.10 | Electronic load distribution, 4 channels, setting 6 A | -102F2 | article number ▲ |
| 7 | 1 | pc | Rittal | SK 3110.000 | Thermostat | T55 | |
| 8 | 1 | pc | Rittal | SK 3102.000 | Enclosure heater | E55 | identity unverified |
| 9 | 1 | pc | ▲ | ▲ | Door switch | S56 | missing in reference |
| 10 | 1 | pc | ▲ | ▲ | Cabinet lamp | E56 | missing in reference |
| 11 | 2 | pc | Phoenix Contact | UK 5 N | Feed-through terminal | 1X1:1, 1X1:2 | split from "UK5N / ULSKG5" |
| 12 | 1 | pc | Phoenix Contact | USLKG 5 (drawn "ULSKG5") | PE terminal | 1X1:PE | spelling unverified |
| 13 | 20 ▲ (or 10) | pc | Phoenix Contact | PT 2.5-TWIN | Feed-through terminal, 3 connections | -X1031:1…20 | quantity depends on Q4 |
| 14 | 5 | pc | Phoenix Contact | PT 2.5-TWIN-PE | PE terminal, 3 connections | -X1032:1…5 | |
| 15 | ▲ | pc | Phoenix Contact | ▲ | Plug-in bridges, end brackets (4 drawn), end covers, markers | 1X1, X1031, X1032 | from terminal model |
| 16 | 2 | pc | Festo | NEBU-G78W5-K-2-N-LE5 | Connecting cable 7/8", 5-pin, open end, length 2 m (unverified) | -106W1, -106W2 | |
| 17 | 1 | pc | Festo | 532216 | PROFIBUS DP plug (Sub-D), with terminating resistor | -3X1 | |
| 18 | ▲ m | m | Siemens | 6XV1830-0EH10 | PROFIBUS FC standard cable | -3W1 | length ▲ |
| 19 | 1 | set | Festo | Order code 1: 51E-F13GCQP-D+H2GG | CPX terminal (configured) | -201U1 … | assembly header |
| 20 | 1 | set | Festo | Order code 2: 32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS | MPA valve terminal (configured) | -201U2 … -202U3 | assembly header |
| 20.1 | 1 | pc | Festo | 195740 | CPX-FB13 bus node PROFIBUS DP | -201U1 | component of 19 |
| 20.2 | 2 | pc | Festo | 550208 | CPX-M-GE-EV-S-7/8-5POL interlinking block with power supply | -201U1, -202U1 | see D-40 |
| 20.3 | 1 | pc | Festo | 552286 | VMPA-FB-EPLM-G | -201U2 | |
| 20.4 | 7 | pc | Festo | 533361 | VMPA1-FB-EMG-8 electronic module (4 positions / 8 outputs) | -201U3 … -201U8, -202U2 | |
| 20.5 | 28 | pc | Festo | 571334 | 5/2 monostable solenoid valve | QM children | |
| 20.6 | 2 | pc | Festo | 533354 | VMPA1-FB-SP | -201U9, -202U3 | |
| 21 | 1 | pc | Schneider Electric | NSYPLA1074G | Enclosure | -UC1 | dims ▲ |
| 22 | 1 | pc | Schneider Electric | NSYPMM107 | Mounting plate | -UC1 | |
| 23 | ▲ m | m | ▲ | 40 × 60 duct | Wiring duct (5 segments) | 902 | |
| 24 | ▲ m | m | ▲ | TS 35 rail | DIN rail (3 rows) | 902 | |

The BOM report shall offer two modes: *assembly mode* (items 19-20 only) and *component mode* (items 20.1-20.6).

### 4.6 Potentials and colour/size rules

| Potential (golden name) | Type | Voltage | Drawn names | Members (connection points) | Colour | Default size |
|---|---|---|---|---|---|---|
| L | L | 230 V AC | J-E-206-L | ext. → 1X1:1, S51:2 | BK | 2.5 (explicit) |
| N | N | 230 V AC | J-E-206-N | ext. → 1X1:2, S51:4 | BU | 2.5 (explicit) |
| L-S | L | 230 V AC | — | S51:1, -101F1:1, F55:1 | BK | 2.5 / 1.5 ▲ |
| N-S | N | 230 V AC | — | S51:3, -101F1:N#1, F55:N#1 | BU | 2.5 / 1.5 ▲ |
| L-1 | L | 230 V AC | L-1 | -101F1:2, -102G1:L | BK | 1.5 |
| N-1 | N | 230 V AC | N-1 | -101F1:N#2, -102G1:N | BU | 1.5 |
| L-55 | L | 230 V AC | — | F55:2, E55:L, E56:X2 | BK | 1.5 |
| N-55 | N | 230 V AC | — | F55:N#2, T55:5, S56:13 | BU | 1.5 |
| N-55T | N (switched) | 230 V AC | — | T55:3, E55:N | BU | 1.5 |
| N-55S | N (switched) | 230 V AC | — | S56:14, E56:X1 | BU | 1.5 |
| PE | PE | — | J-E-206-PE, PE-1.1…1.4, PE-AIR1/2, PE-0VDC | 1X1:PE, -XPE, -X1032:1…5, E55:PE, -102G1:PE, -201U1:3, -201U1:FE, -202U1:3, -202U1:FE (+ rail) | GNYE | 1.5 mains / 0.75 control ▲ (D-38) |
| +24V | +24 V DC | 24 V DC | — | -102G1:+#1, +#2, -102F1:1 | DBU | 1.5 (explicit) |
| +24V-F | +24 V DC | 24 V DC | — | -102F1:2, -102F2:24VDC#1, #2 | DBU | 1.5 (explicit) |
| 24VDC-1 | +24 V DC | 24 V DC | 24VDC-1, 24VDC-AIR1…4 | -102F2:Out1, -X1031:1…4, -201U1:4, -201U1:5, -202U1:4, -202U1:5 | DBU | 0.75 |
| 24VDC-2 | +24 V DC (spare) | 24 V DC | 24VDC-2 | -102F2:Out2, -X1031:5, 6 | DBU | 0.75 |
| 24VDC-3 | +24 V DC (spare) | 24 V DC | 24VDC-3 | -102F2:Out3, -X1031:7, 8 | DBU | 0.75 |
| 24VDC-4 | +24 V DC (spare) | 24 V DC | 24VDC-4 | -102F2:Out4, -X1031:9, 10 | DBU | 0.75 |
| 0V | 0 V (M) | 24 V DC | 0VDC, 0VDC-AIR1…4 | -102G1:−#1, −#2, -102F2:GND#1, #2, -X1031:11…20, -201U1:1, 2, -202U1:1, 2 | WH | 0.75 (1.5 explicit at PSU) |
| Bond 0V–PE | bonding object | — | PE-0VDC | -102G1:−#1 ↔ -X1032:2 (one declared bond) | GNYE ▲ or WH ▲ | 0.75 ▲ |
| DP | bus | — | DP | -201U1:DP, -3X1, -3W1, ext. JE-N1000-03 | (cable) | — |

### 4.7 Connections (wires)

"Rule" in the size/colour column means the value comes from the §4.6 profile and is not printed. "Explicit" means a wire marker is printed.

| W# | Folio | From | To | Potential | Size / colour | Via |
|---|---|---|---|---|---|---|
| 1 | 101 | +J-E-206 / JE-N1000-03 (L) | 1X1:1 (field side) | L | ▲ (incoming cable, D-19) | cable ▲ -W? |
| 2 | 101 | +J-E-206 (N) | 1X1:2 (field side) | N | ▲ | cable ▲ |
| 3 | 101 | +J-E-206 (PE) | 1X1:PE (field side) | PE | ▲ | cable ▲ |
| 4 | 101 | 1X1:1 | S51:2 | L | 2.5 BK explicit | — |
| 5 | 101 | 1X1:2 | S51:4 | N | 2.5 BU explicit | — |
| 6 | 101 | 1X1:PE | -XPE (earth point) | PE | GNYE ≥ 2.5 ▲ | — |
| 7 | 101 | S51:1 | -101F1:1 | L-S | 2.5 BK explicit | daisy-chain |
| 8 | 101 | -101F1:1 | F55:1 | L-S | 1.5 BK rule | daisy-chain |
| 9 | 101 | S51:3 | -101F1:N#1 | N-S | 2.5 BU explicit | daisy-chain |
| 10 | 101 | -101F1:N#1 | F55:N#1 | N-S | 1.5 BU rule | daisy-chain |
| 11 | 101→102 | -101F1:2 | -102G1:L | L-1 | 1.5 BK rule | IP L-1 |
| 12 | 101→102 | -101F1:N#2 | -102G1:N | N-1 | 1.5 BU rule | IP N-1 |
| 13 | 101 | F55:2 | E55:L | L-55 | 1.5 BK rule | junction dot |
| 14 | 101 | F55:2 | E56:X2 | L-55 | 1.5 BK rule | junction dot |
| 15 | 101 | F55:N#2 | T55:5 | N-55 | 1.5 BU rule | junction dot |
| 16 | 101 | F55:N#2 | S56:13 ▲ | N-55 | 1.5 BU rule | junction dot |
| 17 | 101 | T55:3 | E55:N | N-55T | 1.5 BU rule | — |
| 18 | 101 | S56:14 ▲ | E56:X1 ▲ | N-55S | 1.5 BU rule | — |
| 19 | 101→103 | E55:PE | -X1032:1 (side 2) | PE | 1.5 GNYE rule | IP PE-1.1 |
| 20 | 102 | -102G1:+#1 | -102F1:1 | +24V | 1.5 DBU explicit | 2 wires on -102F1:1 |
| 21 | 102 | -102G1:+#2 | -102F1:1 | +24V | 1.5 DBU explicit | |
| 22 | 102 | -102G1:−#1 | -102F2:GND#1 | 0V | 1.5 WH explicit | |
| 23 | 102 | -102G1:−#2 | -102F2:GND#2 | 0V | 1.5 WH explicit | |
| 24 | 102 | -102F1:2 | -102F2:24VDC#1 | +24V-F | 1.5 DBU explicit | |
| 25 | 102 | -102F1:2 | -102F2:24VDC#2 | +24V-F | 1.5 DBU explicit | |
| 26 | 102→103 | -102G1:−#2 | -X1031:11 | 0V | 0.75 WH rule | IP 0VDC |
| 27 | 102→103 | -102G1:−#1 | -X1032:2 (side 1) | Bond 0V–PE | ▲ | IP PE-0VDC |
| 28 | 102→103 | -102G1:PE | -X1032:2 (side 2) | PE | 1.5 GNYE rule | IP PE-1.2 |
| 29 | 102→103 | -102F2:Out1 | -X1031:1 | 24VDC-1 | 0.75 DBU rule | IP 24VDC-1 |
| 30 | 102→103 | -102F2:Out2 | -X1031:5 | 24VDC-2 | 0.75 DBU rule | IP 24VDC-2 |
| 31 | 102→103 | -102F2:Out3 | -X1031:7 | 24VDC-3 | 0.75 DBU rule | IP 24VDC-3 |
| 32 | 102→103 | -102F2:Out4 | -X1031:9 | 24VDC-4 | 0.75 DBU rule | IP 24VDC-4 |
| 33 | 103→106 | -X1032:3 (side 2) | -201U1:FE | PE | 0.75 GNYE rule ▲ | IP PE-1.3 |
| 34 | 103→106 | -X1032:4 (side 2) | -202U1:FE | PE | 0.75 GNYE rule ▲ | IP PE-1.4 |
| 35-44 | 103→106 | -X1031 / -X1032 terminals | -201U1 / -202U1 pins | see §4.9 | cable cores | -106W1 / -106W2 |

Terminal bridges (plug-in, from the terminal model):
- -X1031: 1-2-3-4 (24VDC-1), 5-6, 7-8, 9-10, and 11 through 20 (0V; drawn as the 0VDC bus on 103).
- -X1032: 1-2-3-4-5 (PE), plus rail contact.

### 4.8 Terminal strips

Golden designation scheme ▲: unique sequential numbers per strip. The drawn group label ("1+", "2-", "PE") becomes the terminal **function text**, printed as in the reference so the visual stays the same. The drawn symbol order is kept.

**1X1** (+J-E-2416), drawn on 101-D2:

| Terminal | Part | Side 1 (field) | Side 2 (internal) | Potential |
|---|---|---|---|---|
| 1 | UK 5 N | +J-E-206 L (W1) | S51:2 (W4) | L |
| 2 | UK 5 N | +J-E-206 N (W2) | S51:4 (W5) | N |
| PE | USLKG 5 | +J-E-206 PE (W3) | -XPE (W6) | PE |

**-X1031** (+J-E-2416), drawn on 103, two segments:

| # | Drawn label | Function text | Side 1 (from/bridge) | Side 2 (to) | Potential | Spare |
|---|---|---|---|---|---|---|
| 1 | 1+ | Pneumatics | -102F2:Out1 (W29) + bridge | -106W1 BN/"BR" → -201U1:5 (24VDC-AIR1) | 24VDC-1 | |
| 2 | 1+ | Pneumatics | bridge | -106W1 WH → -201U1:4 (24VDC-AIR2) | 24VDC-1 | |
| 3 | 1+ | Pneumatics | bridge | -106W2 BN/"BR" → -202U1:5 (24VDC-AIR3) | 24VDC-1 | |
| 4 | 1+ | Pneumatics | bridge | -106W2 WH → -202U1:4 (24VDC-AIR4) | 24VDC-1 | |
| 5 | 2+ | Spare | -102F2:Out2 (W30) + bridge | — | 24VDC-2 | yes |
| 6 | 2+ | Spare | bridge | — | 24VDC-2 | yes |
| 7 | 3+ | Spare | -102F2:Out3 (W31) + bridge | — | 24VDC-3 | yes |
| 8 | 3+ (unlabelled in ref., D-11) | Spare | bridge | — | 24VDC-3 | yes |
| 9 | 4+ | Spare | -102F2:Out4 (W32) + bridge | — | 24VDC-4 | yes |
| 10 | 4+ | Spare | bridge | — | 24VDC-4 | yes |
| 11 | 1- | Pneumatics | -102G1:−#2 (W26) + bridge | -106W1 BU → -201U1:2 (0VDC-AIR1) | 0V | |
| 12 | 1- | Pneumatics | bridge | -106W1 BK → -201U1:1 (0VDC-AIR2) | 0V | |
| 13 | 1- | Pneumatics | bridge | -106W2 BU → -202U1:2 (0VDC-AIR3) | 0V | |
| 14 | 1- | Pneumatics | bridge | -106W2 BK → -202U1:1 (0VDC-AIR4) | 0V | |
| 15-16 | 2- | Spare | bridge | — | 0V | yes |
| 17-18 | 3- | Spare | bridge | — | 0V | yes |
| 19-20 | 4- | Spare | bridge | — | 0V | yes |

**-X1032** (+J-E-2416, PE terminals, rail-bonded), drawn on 103-F3:

| # | Drawn label | Side 1 (left) | Side 2 (right) |
|---|---|---|---|
| 1 | PE | -106W2 GNYE → -202U1:3 (PE-AIR2) | E55:PE (PE-1.1, W19) |
| 2 | PE | -102G1:−#1 (PE-0VDC bond, W27) | -102G1:PE (PE-1.2, W28) |
| 3 | PE | -106W1 GNYE → -201U1:3 (PE-AIR1) | -201U1:FE (PE-1.3, W33) |
| 4 | PE | — | -202U1:FE (PE-1.4, W34) |
| 5 | PE | — (spare) | — (spare) |

### 4.9 Cables

| Cable | Part | Cores | Core → source → target |
|---|---|---|---|
| -106W1 | Festo NEBU-G78W5-K-2-N-LE5 (7/8" 5-pin socket, open end; colours per vendor ▲) | 5 | BK: -X1031:12 → -201U1:1 (0V Val); BU: -X1031:11 → -201U1:2 (0V El); GNYE: -X1032:3 → -201U1:3; WH: -X1031:2 → -201U1:4 (24V El); BN (drawn "BR"): -X1031:1 → -201U1:5 (24V Val) |
| -106W2 | same | 5 | BK: -X1031:14 → -202U1:1; BU: -X1031:13 → -202U1:2; GNYE: -X1032:1 → -202U1:3; WH: -X1031:4 → -202U1:4; BN: -X1031:3 → -202U1:5 |
| -3W1 | Siemens 6XV1830-0EH10 (2-core shielded; A/B core colours per vendor ▲) | 2 + shield | -3X1 (on -201U1:DP) → +J-E-206 / JE-N1000-03; length ▲ |
| ▲ -W (new) | incoming supply cable, 3 cores (L, N, PE; size ▲ ≥ 2.5 mm²) | 3 | +J-E-206 → 1X1:1, 2, PE |

### 4.10 PLC and bus data

| Item | Golden value |
|---|---|
| Bus | PROFIBUS DP; master in +J-E-206 (JE-N1000-03 ▲); baud rate ▲ |
| Slave | -201U1 (CPX-FB13), station address **▲ (TBD in reference; must become a value before release)** |
| Termination | -3X1 terminating resistor ON (-201U1 is the segment end) |
| Output image | 7 bytes Q0 … Q6, one per VMPA1-FB-EMG-8 module, in manifold order (relative to node start address; absolute address ▲) |

| Address | Module / position | Valve | Function text | State |
|---|---|---|---|---|
| Q0.1 | -201U3 pos 1 | -201U3-QM1 | TA05-V-15 | used |
| Q0.3 | -201U3 pos 2 | -201U3-QM2 | TA05-V-33 | used |
| Q0.5 | -201U3 pos 3 | -201U3-QM3 | TA05-V-18 | used |
| Q0.7 | -201U3 pos 4 | -201U3-QM4 | TA05-V-17 | used |
| Q1.1 | -201U4 pos 1 | -201U4-QM1 | TA05-V-36 | used |
| Q1.3 | -201U4 pos 2 | -201U4-QM2 | TA05-V-38 | used |
| Q1.5 | -201U4 pos 3 | -201U4-QM3 | TA05-V-16 | used |
| Q1.7 | -201U4 pos 4 | -201U4-QM4 | TA05-V-37 | used |
| Q2.1/.3/.5/.7 | -201U5 pos 1-4 | -201U5-QM1…4 | Spare | spare |
| Q3.1/.3/.5/.7 | -201U6 pos 1-4 | -201U6-QM1…4 | Spare | spare |
| Q4.1/.3/.5/.7 | -201U7 pos 1-4 | -201U7-QM1…4 | Spare | spare |
| Q5.1/.3/.5/.7 | -201U8 pos 1-4 | -201U8-QM1…4 | Spare | spare |
| Q6.1/.3/.5/.7 | -202U2 pos 1-4 | -202U2-QM1…4 | Spare | spare |
| Qn.0/.2/.4/.6 | coil-12 (or 14) addresses | — | not assigned | check D-26 |

### 4.11 Pneumatic (fluid) functions

| Valve | Type | Ports | Actuation | Drives |
|---|---|---|---|---|
| 28 × 571334 (QM children of -201U3…U8, -202U2) | 5/2 monostable | 1 supply, 2/4 working, 3/5 exhaust (designations printed "3 1 5" / "2" / "4") | solenoid 14 ▲ + manual override; spring return | 8 used: ports 2 and 4 → double-acting actuator of process valve TA05-V-xx (8 tube pairs, Ø ▲) |
| 8 process valves TA05-V-15/-33/-18/-17/-36/-38/-16/-37 | 2-way process valve + double-acting actuator, mechanical linkage | actuator ports from 2 and 4 | pneumatic | process (location ▲) |

To add for completeness ▲: supply connection to port 1 (manifold supply plate), exhaust or silencers on 3/5, and an air service unit if it is part of this cabinet.

### 4.12 Panel layout placements

The table records the reference positions in paper mm on folio 902 (not to scale). Relative x/y are measured from the cabinet outline's top-left corner (118.09, 31.97). The golden layout uses real part dimensions at a declared scale. **Placement order and row assignment** are the acceptance criteria, not these paper dimensions.

| Object | Row | x0 rel | y0 rel | w | h | Note |
|---|---|---|---|---|---|---|
| Cabinet outline NSYPLA1074G | — | 0 | 0 | 88.72 | 127.87 | line 0.52 mm |
| Duct top 40×60 | — | 2.61 | 2.61 | 83.51 | 5.22 | |
| Duct left 40×60 (vertical) | — | 2.61 | 7.83 | 5.22 | 99.16 | |
| Duct right 40×60 (vertical) | — | 80.89 | 7.83 | 5.22 | 52.19 | |
| Duct middle 40×60 | — | 7.82 | 33.92 | 73.07 | 5.22 | |
| Duct lower (unlabelled) | — | 7.82 | 54.80 | 73.07 | 5.22 | type ▲ |
| -102G1 | 1 | 9.65 | 15.66 | 7.83 | 10.44 | |
| -101F1 | 1 | 17.48 | 13.05 | 3.91 | 15.66 | label rotated |
| -102F1 | 1 | 21.39 | 13.05 | 3.92 | 15.66 | label rotated |
| -102F2 | 1 | 25.31 | 13.05 | 13.05 | 15.66 | |
| T55 (drawn "T51") | 1 | 38.36 | 15.66 | 10.44 | 10.44 | fix tag |
| S51 | 1 | 66.54 | 15.66 | 10.44 | 10.44 | |
| F55 | 1 | ▲ | ▲ | ▲ | ▲ | **add** |
| End bracket | 2 | 9.13 | 44.36 | 1.30 | 5.22 | |
| 1X1 | 2 | 10.43 | 42.28 | 5.22 | 9.39 | 3 terminals |
| End bracket | 2 | 15.65 | 44.36 | 1.31 | 5.22 | |
| -X1031 | 2 | 16.96 | 43.06 | 8.35 | 7.83 | 20 terminals ▲ |
| End bracket | 2 | 25.31 | 44.36 | 1.30 | 5.22 | |
| -X1032 | 2 | 26.61 | 42.80 | 3.92 | 8.35 | 5 terminals |
| End bracket | 2 | 30.53 | 44.36 | 1.30 | 5.22 | |
| -201U1 | 3 | 10.96 | 63.93 | 3.91 | 18.27 | valve-terminal row |
| -201U2 | 3 | 14.87 | 63.93 | 3.65 | 18.27 | |
| -201U3 … -201U8 | 3 | 18.52, 26.61, 34.44, 42.27, 50.10, 57.93 | 63.93 | 8.09, then 7.83 ×5 | 18.27 | |
| **-201U9** | 3 | ▲ (between U8 and -202U1) | 63.93 | ▲ | 18.27 | **add** |
| -202U1 | 3 | 65.76 | 63.93 | 3.91 | 18.27 | |
| -202U2 | 3 | 69.67 | 63.93 | 7.83 | 18.27 | |
| -202U3 | 3 | 77.50 | 63.93 | 3.91 | 18.27 | ends at 81.41 |
| E55 heater, E56 lamp, S56 door switch | — | ▲ | ▲ | | | **add** (heater low in cabinet; lamp and switch at the door) |
| DIN rails (rows 1-3), PE bar -XPE | — | ▲ | | | | **add** |

### 4.13 Cross-reference acceptance table

Every pair must be regenerated exactly as printed.

**Interruption points (22 pairs, all verified consistent in the reference):**

| Name | Source (cell, ▷/◁) | Target (cell) | Text printed at source | Text printed at target |
|---|---|---|---|---|
| L-1 | 101-H16 ▷ | 102-A1 ▷ | 102-A1 | 101-H16 |
| N-1 | 101-H16 ▷ | 102-A1 ▷ | 102-A1 | 101-H16 |
| PE-1.1 | 101-G15 ▷ | 103-G6 ◁ | 103-G6 | 101-G15 |
| PE-1.2 | 102-A15 ▷ | 103-G6 ◁ | 103-G6 | 102-A15 |
| 0VDC | 102-C15 ▷ | 103-A1 ▷ | 103-A1 | 102-C15 |
| PE-0VDC | 102-D15 ▷ | 103-G1 ▷ | 103-G1 | 102-D15 |
| 24VDC-1 | 102-H15 | 103-A1 | 103-A1 | 102-H15 |
| 24VDC-2 | 102-H15 | 103-C1 | 103-C1 | 102-H15 |
| 24VDC-3 | 102-G15 | 103-D1 | 103-D1 | 102-G15 |
| 24VDC-4 | 102-G15 | 103-F1 | 103-F1 | 102-G15 |
| 24VDC-AIR1 | 103-A6 | 106-E1 | 106-E1 | 103-A6 |
| 24VDC-AIR2 | 103-B6 | 106-E1 | 106-E1 | 103-B6 |
| 24VDC-AIR3 | 103-B6 | 106-B1 | 106-B1 | 103-B6 |
| 24VDC-AIR4 | 103-B6 | 106-B1 | 106-B1 | 103-B6 |
| 0VDC-AIR1 | 103-A14 | 106-E1 | 106-E1 | 103-A14 |
| 0VDC-AIR2 | 103-B14 | 106-F1 | 106-F1 | 103-B14 |
| 0VDC-AIR3 | 103-B14 | 106-B1 | 106-B1 | 103-B14 |
| 0VDC-AIR4 | 103-B14 | 106-C1 | 106-C1 | 103-B14 |
| PE-AIR1 | 106-E1 ◁ | 103-G1 ▷ | 103-G1 | 106-E1 |
| PE-AIR2 | 106-B1 ◁ | 103-G1 ▷ | 103-G1 | 106-B1 |
| PE-1.3 | 106-D1 ◁ | 103-G6 ◁ | 103-G6 | 106-D1 |
| PE-1.4 | 106-A1 ◁ | 103-G6 ◁ | 103-G6 | 106-A1 |

**External (no xref):** J-E-206-L, J-E-206-N, J-E-206-PE (101-G0) and DP (3-G15). Golden: these print the external target "+J-E-206 / JE-N1000-03".

**Main ↔ auxiliary device references:**

| Device | Main (printed on aux as "(…)") | Aux positions (printed on main after "∞") |
|---|---|---|
| -201U1 | 201-E1 | **3-B2** (golden; reference prints 3-B0, D-14), 106-G3 |
| -202U1 | 202-E1 | 106-G9 |
| -201U3 | 201-E4 | 203-B4 |
| -201U4 | 201-E6 | 203-B12 |
| -201U5 | 201-E8 | 204-B4 |
| -201U6 | 201-E10 | 204-B12 |
| -201U7 | 201-E12 | 205-B4 |
| -201U8 | 201-E14 | 205-B12 |
| -202U2 | 202-E3 | 206-B4 |

### 4.14 Generated pages to add (defect-free and EPLAN-class)

Proposed folios ▲ in section 9, after 902:

| Folio ▲ | Report | Content source |
|---|---|---|
| 903 | Terminal diagram 1X1 | §4.8 |
| 904 | Terminal diagram -X1031 | §4.8 |
| 905 | Terminal diagram -X1032 | §4.8 |
| 906 | Cable overview + cable diagrams (-W supply, -3W1, -106W1, -106W2) | §4.9 |
| 907 | Wire/connection list (W1 … W44 plus bridges) | §4.7 |
| 908 | PLC / I/O overview -201U1 (Q0.0 … Q6.7) | §4.10 |
| 909 | Potential overview | §4.6 |
| 910 | Device-tag / structure-identifier overview | §4.4 |
| 911 | Revision overview | §4.1 |
| 2 (cont.) | TOC updated automatically | — |

### 4.15 Acceptance criteria (summary)

1. **Pages.** The page set, folio numbers, page types and page descriptions equal §4.3. The TOC equals the page set. Prev/Next are computed.
2. **Frame geometry.** The frame and title block match §5 within ±0.2 mm in the PDF output at the same fit-to-page scale (or exactly in native mm).
3. **Cross-references.** All 22 interruption-point pairs and all 9 main/aux reference sets print exactly as in §4.13.
4. **Device tags.** Every tag printed anywhere (schematic, overview, BOM, layout) is identical for the same device. No -4W1, no T51.
5. **BOM.** The BOM equals §4.5, with quantities and both assembly/component modes.
6. **Terminals.** Terminal designations are unique. The terminal diagrams list exactly §4.8.
7. **Wire data.** Every wire has an effective cross-section and colour (explicit or rule-derived). The wire list equals §4.7.
8. **Checks.** The check run reports D-13 (N switching) unless corrected, D-26 (address bits), and the open items (TBD address, missing parts S56/E56, unplaced devices) until resolved, with zero errors otherwise.
9. **PDF.** The PDF has bookmarks, link annotations for every cross-reference, page labels equal to folio numbers, and embedded fonts.

---

## 5. Frame, title block, grid, fonts, line types and symbol styles

### 5.1 Title-block field list and layout (every page)

Overall: x 9.79-281.18 mm (271.39 wide), y 182.02-199.76 mm (17.74 high). The block has 4 rows of 4.44 mm (17 QET px). Native size at QET 96 dpi: 1040 × 68 px = 275.17 × 17.99 mm. Printed scale: 0.98628.

| Cell | x (mm) | y (mm) | W × H (mm) | Native px | Content | Text (pt, printed) | Alignment |
|---|---|---|---|---|---|---|---|
| Logo | 9.79-21.79 | 182.02-199.76 | 12.00 × 17.74 | 46 × 68 | The pilot user logo, raster PNG 98 × 140 | — | fit |
| Page description | 21.79-172.62 | 182.02-190.89 | 150.83 × 8.87 | 578 × 34 | e.g. "Bus Topology - Profibus" | 8.88 | left, middle |
| Project | 21.79-131.92 | 190.89-195.33 | 110.12 × 4.44 | 422 × 17 | "Project: <project ref>" | 6.66 | left |
| Description | 21.79-131.92 | 195.33-199.76 | 110.12 × 4.44 | 422 × 17 | "Description: Expansion Control Cabinets" | 6.66 | left |
| Drawing | 131.92-172.62 | 190.89-195.33 | 40.71 × 4.44 | 156 × 17 | "Drawing: <drawing id>" | 6.66 | left |
| Revision | 131.92-172.62 | 195.33-199.76 | 40.71 × 4.44 | 156 × 17 | "Revision: V1.0" | 6.66 | left |
| Function (=) | 172.62-199.76 | 182.02-190.89 | 27.14 × 8.87 | 104 × 34 | "=P01" | 8.88 | centre |
| Location (+) | 172.62-199.76 | 190.89-199.76 | 27.14 × 8.87 | 104 × 34 | "+J-E-2416" | 8.88 | centre |
| Customer | 199.76-226.90 | 182.02-186.46 | 27.14 × 4.44 | 104 × 17 | "Customer: <end user>" | 6.66 | left |
| Object | 199.76-226.90 | 186.46-190.89 | 27.14 × 4.44 | 104 × 17 | "Object: J-E-2416" | 6.66 | left |
| Checked | 199.76-226.90 | 190.89-195.33 | 27.14 × 4.44 | 104 × 17 | "Checked: <initials>" | 6.66 | left (value tabbed) |
| Author | 199.76-226.90 | 195.33-199.76 | 27.14 × 4.44 | 104 × 17 | "Author: <initials>" | 6.66 | left (value tabbed) |
| Creation | 226.90-254.04 | 182.02-186.46 | 27.14 × 4.44 | 104 × 17 | "Creation: 10-06-2024" | 5.92 | centre |
| Saved | 226.90-254.04 | 186.46-190.89 | 27.14 × 4.44 | 104 × 17 | "Saved: 12-07-2024" | 5.92 | centre |
| Page | 254.04-281.18 | 182.02-186.46 | 27.14 × 4.44 | 104 × 17 | "Page: 101" | 8.88 | centre |
| Prev | 254.04-267.61 | 186.46-190.89 | 13.57 × 4.44 | 52 × 17 | "Prev: 100" | 5.92 | centre |
| Next | 267.61-281.18 | 186.46-190.89 | 13.57 × 4.44 | 52 × 17 | "Next: 102" | 5.92 | centre |
| Notice | 226.90-281.18 | 190.89-199.76 | 54.28 × 8.87 | 208 × 34 | 4 lines: "<confidentiality notice>" | 5.18 | centre |

Notes:
- Cell lines are 0.261 mm. Each cell rectangle is emitted **twice** in the PDF (QET artefact).
- The label ("Project:") and value are in one text run. ElektraWatt should model them as label + bound value.
- **Field values by page:**
  - Revision: page 1 "1"; page 100 "V1"; pages 101 and 106 "V2.0"; all others "V1.0".
  - Description: page 101 "Control Cabinet Expansion"; all others "Expansion Control Cabinets".
  - Unchanged on every page: Creation 10-06-2024, Saved 12-07-2024, Checked <initials>, Author <initials>, Customer <end user>, Object J-E-2416, Project <project ref>, Drawing <drawing id>, =P01, +J-E-2416.

### 5.2 Page frame and grid geometry

| Item | Paper mm (measured) | Native (QET px → mm at 96 dpi) |
|---|---|---|
| Sheet | A4 landscape 297 × 210 (842 × 595 pt) | — |
| Frame outer | x 9.79-281.18, y 9.79-199.76 (271.39 × 189.97); right margin 15.82, bottom margin 10.24 | 1040 × 728 px = 275.17 × 192.61 mm |
| Print scale | 0.98628 (fit to page), offset 9.79 mm | 1 px = 0.26458 mm native, 0.26096 mm printed |
| Column header band | y 9.79-15.01 (5.22) | 20 px |
| Row header band | x 9.79-15.01 (5.22) | 20 px |
| Columns | 17 columns numbered **0 … 16**, each 15.66 mm (x0 = 15.01 + n × 15.66) | 60 px = 15.875 mm |
| Rows | 8 rows **A … H**, each 20.88 mm (y0 = 15.01 + n × 20.88) | 80 px = 21.17 mm |
| Drawing area | x 15.01-281.18 (266.17), y 15.01-182.02 (167.01) | 1020 × 640 px |
| Grid labels | 8.88 pt, centred in the header cells; top and left only | QET default border ([borderproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/borderproperties.cpp), via `interop-formats.md`) |
| Cell lookup | `col = floor((x − 15.01)/15.66)`, `row = "ABCDEFGH"[floor((y − 15.01)/20.88)]` | xref format `%F-%l%c` |
| Frame variants | (a) grid frame: 3, 101-106, 201-206, 902; (b) no grid: 1, 2, 100, 200, 900, 901 (plain outer rectangle, same title block) | |

### 5.3 Fonts and text sizes

Fonts: **Tahoma** and **Tahoma-Bold** only (embedded, not subset). Tahoma is a Microsoft font, so ElektraWatt needs a font-substitution strategy and an OFL-licensed default; see the Implications section.

Printed pt = QET px × 0.7397. Native pt at 96 dpi = px × 0.75.

| Printed pt | mm (em) | ≈ cap height mm | QET px | Use |
|---|---|---|---|---|
| 35.51 | 12.53 | 9.1 | 48 | Cover title "J-E-2416" |
| 27.37 bold | 9.66 | 7.0 | 37 | Section-page identifier "=P01+J-E-2416" |
| 21.45 | 7.57 | 5.5 | 29 | Section subtitle |
| 19.97 | 7.04 | 5.1 | 27 | Cover subtitle "Electrical Definition" |
| 15.53 | 5.48 | 4.0 | 21 | "PART DATA" box titles |
| 11.84 | 4.18 | 3.0 | 16 | Cover tables (upper project block, table headers) |
| 9.62 | 3.39 | 2.5 | 13 | Device tags (main), terminal-strip tags, external captions, cover project values (lower block) |
| 8.88 | 3.13 | 2.3 | 12 | Grid labels, page title, =/+ in title block, Page:, interruption-point names, some device tags, "I>", table body text (TOC, BOM, colour table), group texts |
| 8.14 | 2.87 | 2.1 | 11 | MICO field texts "24VDC", "GND" |
| 6.66 | 2.35 | 1.7 | 9 | Part data at symbols, cross-references under arrows, terminal designations, title-block fields, PLC addresses and function texts |
| 5.92 | 2.09 | 1.5 | 8 | Creation/Saved/Prev/Next, PSU pins, CPX pins and names, valve port numbers, cable part text |
| 5.18 | 1.83 | 1.3 | 7 | Confidentiality notice, device xrefs "(201-E1)", overview module texts, "Switch to ON", "AC ~", layout labels |
| 3.70 | 1.31 | 0.95 | 5 | Wire-marker texts (2.5, BK, BU, DBU, WH), core colours, MCB/switch pin numbers, "DP" port label |
| 1.88-3.40 | 0.66-1.20 | ≤ 0.9 | scaled | Rotated labels in the panel layout (element scaled) |

Cap heights assume Tahoma cap height ≈ 0.727 em (unverified font metric).

### 5.4 Line widths and line types

| Width (mm) | QET px | Dash (× pen width) | Use |
|---|---|---|---|
| 0.261 | 1.0 | solid | Conductors, symbol outlines, frame, grid, title-block cells, layout objects |
| 0.130 | 0.5 | solid | Thin internal symbol lines (MCB handle box) |
| 0.130 | 0.5 | **dashed**: PDF dash array [2 1] px = 0.52 mm dash / 0.26 mm gap (Qt DashLine 4:2 × 0.5 px pen) | MCB internal mechanical linkage (-101F1, F55, -102F1) |
| 0.261 | 1.0 | **dashed 4:2** (dash 1.04 mm, gap 0.52 mm) | Main-switch actuator linkage (S51); actuator-to-process-valve linkage (203) |
| 0.261 | 1.0 | **dotted 1:2** (dot 0.26 mm, gap 0.52 mm) | "External Connection" boxes (3, 101) |
| 0.183 | 0.7 | solid | Report table grid (TOC, BOM) |
| 0.418 | 1.6 | solid | Cover page boxes and tables |
| 0.522 | 2.0 | solid | Cabinet outline (902), valve symbol double bar (203-206), thin cover/section rule |
| 1.305 | 5.0 | solid | Heavy rule on cover and section pages |
| filled black | — | — | Drop shadows (2.61 mm offset) on cover boxes and PART DATA boxes; junction dots; arrowheads in valve symbols |
| filled #C0C0C0 | — | — | Report table header rows |

The patterns are Qt's `DashLine` (4,2) and `DotLine` (1,2), scaled by pen width, as written by QET via Qt's PDF engine.

### 5.5 Symbol styles observed

| Symbol | Style details | Standard |
|---|---|---|
| Interruption point | Hollow triangle ≈ 2.9 × 2.1 mm. Name 8.88 pt beside the tip. Xref 6.66 pt below, right-aligned to the arrow. | IEC 61082-1 interruption / continuation |
| Terminal | Small circle (≈ 1.0 mm diameter) on the conductor, designation 6.66 pt below-right | IEC 60617 S00017-style |
| Junction | Filled dot ≈ 0.8 mm; or 45° bevel (≈ 0.7 mm) for daisy-chain branches | IEC 60617 S00019 (dot) |
| Wire marker | Horizontal line across the conductors, oblique tick on each conductor, size at left, colour codes above (3.70 pt) | Company style |
| Cable marker | As wire marker, with cable tag 9.62 pt and part text 5.92 pt at the right end | Company style |
| MCB | Contact blades at 30°, "×" breaker mark at the fixed contact, thermal box plus magnetic "I>" box, manual-actuator box, thin dashed linkage | IEC 60617 S00287-family (unverified number) |
| Switch (S51) | 2 NO blades, actuator "⌐" handle with dashed linkage | IEC 60617 |
| Black boxes | Rectangle with named pins (PSU, MICO, CPX). Pins as circles; plug pins as ≈0.78 × 1.83 mm bars (three parallel strokes) plus circle. Earth terminal with ⏚ glyph. | IEC 61082-1 black-box representation |
| Protective earth | Earth symbol inside a circle | IEC 60417-5019 |
| Functional earth (CPX) | Earth glyph (three bars) next to the terminal circle | IEC 60417-5018 (unverified mapping) |
| Lamp | Circle with X | IEC 60617 S00965 |
| Heater (E55) | Box with "∞"-like glyph and a 3-cell top strip | Non-standard ▲ |
| 5/2 valve | Two squares, internal arrows, T blocks, spring "W", solenoid with manual override, ports 3 1 5 / 2 4 | ISO 1219-1 |
| Process valve and actuator | Bow-tie 2-way valve; actuator box with two arrowed inputs; dashed linkage | ISO 1219-1 / ISO 14617-style |
| Manifold front view | Module rectangles with part texts at 5.18 pt, valve top-view glyph, break marks (slanted double line) | Company style (assembly view) |
| Drop-shadow box | Box 0.418 mm with filled 2.61 mm offset shadow (cover, PART DATA) | Company style |

---

## Sources

- Reference PDF (primary evidence): (working notes) (18 pages) and its text layer (working notes). Measured with pdfplumber, pypdf and pdftoppm in this run.
- QElectroTech border, xref and numbering semantics: [borderproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/borderproperties.cpp), [xrefproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/properties/xrefproperties.cpp), [assignvariables.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/autoNum/assignvariables.cpp), [conductorproperties.cpp](https://github.com/qelectrotech/qelectrotech-source-mirror/blob/master/sources/conductorproperties.cpp). Summarised in `research/interop-formats.md` §1.2-§1.8.
- Colour codes and IEC 81346-2 letter-code mapping of this drawing: `research/standards.md` §1.5 and §6 (IEC 60757:2021 via [IEC webstore](https://webstore.iec.ch/publication/66266) and the [Electropedia CDD](https://www.electropedia.org/cdd/iec61360/iec61360.nsf/EnumerationsAllVersions/0112-2---61360_4%23ASA266)).
- Vendor facts (Festo, Phoenix Contact, Schneider Electric, Siemens, Rittal, Murrelektronik, Eaton) could **not** be verified in this run and are marked (unverified).

---

## Implications for ElektraWatt

1. **Make this project the first end-to-end acceptance test (`golden/J-E-2416`).**
   - Encode §4 as a fixture: a project file plus expected outputs (PDF page geometry, cross-reference texts, BOM, terminal and wire lists).
   - Run it in CI.
   - Add a second fixture, **Golden B**, that renumbers to fully page-based tags. It tests renumbering without breaking references.
   - Add a third fixture: QET import of the user's original `.qet`. Once the user provides it, it must import, show defects D-01 … D-42 as check results, and be fixable to Golden A.
2. **Device-oriented single source of truth.**
   - One device object owns its tag, parts, function texts, technical data, connection points and representations (main, aux, overview, layout).
   - Tags are **computed**, never stored as display copies. This one rule removes D-01, D-02, D-41 and D-42, the most expensive class of defects in the set.
3. **Potential-typed nets with rule-derived wire properties.**
   - Nets are derived from connectivity, interruption points, bridges and rail contact.
   - Each potential has a type (L/N/PE/FE/+24V/0V/bus) and an environment (AC/DC, U).
   - Colour and size come from a selectable **profile**, shipped as data. "the pilot user DBU/WH" reproduces folio 1, and the cover tables are generated from it.
   - Wire markers on pages are a *view* of these properties ("show only deviations"), which preserves the user's drawing style (D-08).
4. **First-class junctions, bonds and terminals.**
   - T-nodes carry the connection sequence. 0 V/PE bonds are explicit objects.
   - PE terminals declare rail bonding.
   - Terminal strips are objects with unique designations, function texts, levels and connection points from the part, bridges and spares.
   - One strip can be drawn in several segments.
   - These features prevent D-11, D-12, D-20, D-21, D-22 and D-42.
5. **Electrical rule checks from day one** (cheap once nets are typed). Priority list:
   - neutral-only switching (D-13);
   - PE continuity and PE sizing (D-21, D-38);
   - unpaired interruption points;
   - unconnected points;
   - duplicate or missing designations;
   - unplaced and unparted devices;
   - text minimum size;
   - overview-detail count mismatch (D-10);
   - address-scheme validation (D-26).
   Present them in an "open items" view usable before release.
6. **Assemblies and configured products.**
   - Model the valve terminal as one assembly: a configurator order code with child modules and valve positions, each with its own PLC address and child tag.
   - The overview page (201/202) and the panel-layout row are **generated** from the assembly.
   - The BOM offers assembly and component modes (D-04, D-36, D-40, D-41).
   - The same mechanism covers CPX/ET200SP/Murr IO-Link-style stations for the v1 PLC/I-O scope.
7. **Forms engine with exact geometry.**
   - Frames and title blocks are declarative templates in mm, with cells bound to typed properties.
   - The engine must support:
     - two frame variants (grid / no grid);
     - 0-based column numbering;
     - drop-shadow boxes;
     - the 6 line widths;
     - the Qt-like dash patterns.
   - Its output must match §5 to ±0.2 mm.
   - Ship an OFL fallback font. Respect user-installed Tahoma, but never redistribute it. Record font substitution in the PDF metadata.
   - Support SVG logos (§5.1).
8. **Managed revisions and roles.**
   - Use a project revision index with a single format and a revision overview page.
   - Track page change status, and require author ≠ checker before a release export (D-05, D-34).
   - Keep customer and end user as separate roles (D-30).
9. **PDF export features** (low effort, high value):
   - bookmarks by section and page;
   - link annotations for every cross-reference;
   - `/PageLabels` equal to folio numbers;
   - embedded or subset fonts;
   - PDF/A-2b option;
   - correct text spacing for search (D-07, D-33).
10. **Panel layout bound to devices.**
    - The 2D layout places device macros (real dimensions) on rails and ducts, and adds enclosure and accessories to the BOM.
    - It reports unplaced devices (D-32).
    - It is the 2D entry point of the 3D Pro-Panel-class scope.
11. **Fluid power as a first-class discipline:**
    - ISO 1219 symbol library;
    - ports 1-5 and 12/14;
    - tubes with diameter and material;
    - process field devices with their own location;
    - generated pneumatic tube lists (D-28).
12. **Reports this set lacks:**
    - terminal diagrams;
    - cable diagrams;
    - wire list with lengths;
    - PLC overview with TIA/Codesys export;
    - potential overview;
    - structure-identifier overview;
    - revision overview.
    These are needed to make the user's existing documents match EPLAN output (D-09).

---

## Open questions

1. **Tag scheme for the golden project.** Should Golden A keep the fixed tags S51/F55/T55/E55/S56/E56/1X1 (company template numbering 5x)? Or should all devices move to page-based tags (Golden B)? Which IEC 81346-2 edition (2009 single letters as drawn, or 2019 two-letter classes)?
2. **Canonical location spelling.** Should it be "+J-E-2416" or "+JE2416"? What do "=P01" and "JE-N1000-03" mean, and is J-E-206 a separate project file the tool should link to?
3. **Revision history.** What changed in V2.0 on folios 101 and 106? Is V2.0 the project revision? Which format ("V2.0", "2", "B") should the managed index use?
4. **Terminal count and designations.** Is each drawn terminal symbol on X1031/X1032 one physical PT 2.5-TWIN(-PE)? That gives 20 + 5. Or is it one of the two outputs of a TWIN (10 + ≤5)? Should designations be sequential numbers with "1+/1-/PE" as function text, or should "1+"-style designations stay with a suffix?
5. **T55 and S56 in the neutral (D-13).** Is this a drawing error (F55 pole labelling) or the actual wiring? What are the real terminal designations of Rittal SK 3110.000 (drawn 5/3)?
6. **E55 identity.** Is Rittal SK 3102.000 a heater, a fan heater or a fan? Does it need its own thermostat contact type (NC for heating)?
7. **Valve terminal configuration.** Please decode the Festo order codes 51E-F13GCQP-D+H2GG and 32P-VGL-R-MAHAHAHAHAHAHUL3LAHU-28MS. Specifically:
   - Is -202U1 really 550208 (a CPX interlinking block) inside the MPA section, or an MPA electrical supply plate?
   - Is 533354 VMPA1-FB-SP a supply plate or an end plate?
   - Is -202U2 populated with 4 or 5 valves?
8. **PLC addressing.** Are odd bits (Qn.1/3/5/7) correct for the monostable 571334 valves, given Festo's solenoid-14/12 address order? What are the PROFIBUS station address, master (in J-E-206?) and the node's absolute start address in the PLC?
9. **Cable core colours.** Which pin carries which core colour on Festo NEBU-G78W5-K-2-N-LE5 (drawn 1 BK, 2 BU, 3 GNYE, 4 WH, 5 "BR")? Is pin 3 PE or FE?
10. **Wire sizes not drawn.** What size should the PE wires (PE-1.1, PE-1.2, 1X1:PE → earth) and the 0V–PE bond (PE-0VDC) use: 0.75 or 1.5 mm², GNYE or WH? What is the incoming supply cable (type, cores, size)?
11. **Process field devices.** What location or plant code do TA05-V-xx belong to, and should ElektraWatt tag them (e.g. =TA05-QM15)? What tube sizes are used?
12. **Customer field.** Should the title-block "Customer" show <customer> (the contracting party on the cover) or <end user> (end user)?
13. **Panel layout.** Should the layout be at a stated scale? What are the real enclosure dimensions (NSYPLA1074G)? Where are F55, E55, E56, S56 and -201U9 placed? Which DIN rails and the unlabelled lower duct type are used?
14. **Part data gaps.** What are the part numbers for S56 (door switch) and E56 (lamp)? What is the Murrelektronik article number for MICO 4.10? Confirm Eaton 533356 for S51, since the number is suspiciously close to Festo 5333xx numbers.
15. **Original source.** Can the user provide the `.qet` project file? It would let the QET importer be tested against this exact set, and resolve the symbol-anchor question behind "3-B0".
16. **Additional report pages.** Should the generated reports (terminal, cable, wire, PLC, potential, revision overviews) be added to section 9 with the proposed folios 903-911, or kept as separate documents?
