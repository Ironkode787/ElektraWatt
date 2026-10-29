1. Keep or drop
6/10 - I would keep trying it: wires, junctions, IPs and cross-references come out right and a PDF falls out with no fuss, but parts, terminal groups and bridges are not yet reachable from the interface, so I cannot hand this set to a customer as is.

2. Where it lost me
- Device from part picker: filter '533356' gave 0 rows; only 4 parts exist (MICO 4.10, 2904602, PT 2.5-TWIN, PT 2.5-TWIN-PE). The sheet said all device parts were in the project; Eaton, Schneider, Siemens and Rittal parts were not offered.
- Parts property: typing a part number gave raw serde errors ('invalid type: integer, expected a sequence', then 'expected struct PartUse'); the guide says it needs a copy UUID. A property editor with no choice list.
- place.pick designation=S51 refused 'reason-no-strip': 'designation' means a terminal number, not a tag.
- select.set refs '-101Q1' failed until I learned the 'tag:' prefix (the error did tell me).
- Autoconnect on placement added two wrong wires (T55:5 to F55:2, E55:L to T55:3) that I never asked for; Alt+Enter avoids it, but the default should not guess wires.
- Strip '-X??' was silently renamed to -X1011 after the first terminal, so my next pick with '-X??' was refused 'no-strip'.
- Freezing the strip tag 1X1 opened a preview card that needed Apply, unlike every other device; I noticed only because the next step failed.
- Wire marker: a pointer drag across the wires went 'crossing' then 'armed' and made nothing, no message. Enter-move-Enter worked.
- Marker items in the page text have a cell but no position, so deleting a stray one is a guess.
- Black box from part: first time the ghost had 'cps 0' and the box came with no connection points (needed Add connection point x7); the second time 'cps 7' and they were laid out automatically. Unexplained.
- Tag letter F on the MICO refused 'counter 1 with letter F is already used ... use 2' but there is no counter field; had to freeze -102F2.
- IP tool flips the direction to 'target' after each placement by itself; N-1 on 102 went in as target and the direction is read-only afterwards (raw '{ip:{direction:target}}'), so delete and redo.
- Place by part ignores strip and designation silently ('target part'), so strip terminals cannot carry the PT 2.5-TWIN part.
- Bridge terminals: refused 'reason-bridge-selection' with 4 devices by tag, 4 symbols by click, and 2 symbols; only the strip table finally said 'no command yet'.
- Strip table: 'No row -X1031:1 is on show' although it is; the row key format is never printed.

3. What the tool is
EW is an electrical CAD program for control-panel schematics, like QET but model-based: every symbol is a device with a tag, connection points and properties; wires know their potential, number and cross-section from a size profile; interruption points across folios carry the potential and print their cross-references by themselves; strips are objects with terminals; junctions are real objects with style (dot/bevel), wiring (star/chain) and a sequence; parts lists, terminal diagrams, wire lists and potential overviews are generated as folios of their own. You can drive it fully from the keyboard: go to a cell, pick, Enter.

4. Broken
- strip.bridge (Draw > Bridge terminals): refused 'reason-bridge-selection' three ways; harness marked it broken. The strips table says bridge 'no command yet'. BR1, BR5, BR6 not drawn.
- Strip table group column: 'no command yet' (Pneumatics/Spare groups not set).
- Parts on devices: no usable editor; parts missing for S51, -101F1, F55, T55, E55, -102F1 and all strip terminals (-X1031, -X1032, 1X1). Only -102G1 (2904602) and -102F2 (MICO 4.10) have parts.
- IP direction cannot be changed after placement (read-only).
- Wire marker by pointer drag does nothing (keyboard route works).

5. The drawing set against the design language
As far as the page text tells me: folio 101 has S51 at B2, -101F1 B8, F55 B12, T55 C12, S56 D14, E55 E12, E56 E14, strip 1X1 (1, 2, PE) at D2-D4, earth -XPE at D5, external points J-E-206-L/N/PE at G0 linked to JE-N1000-03, IPs L-1/N-1 at H16 and PE-1.1 at G15, 21 connections, junctions J1/J2 dot-star with the sheet's sequences, two 2.5 mm2 marker groups at S51. Folio 102: -102G1 and -102F2 black boxes from parts, -102F1 at C7, nine IPs at the given cells (directions as sheet), 19 connections, four bevel junctions (J3/J4 chain, J5/J6 star) with exact sequences, 1.5 mm2 overrides and two marker groups ('1.5 BU BU BU' below -102G1, '1.5 BK BK BU BU' above -102F2). Folio 103: -X1031 1,2,3,4,5,7,9,11-14 and -X1032 1-4 (rail bonded) with printed texts 1+ ... PE, 20 IPs, 20 connections; no bridges, no groups. Tags, wire numbers (101.4 etc.) and cross-references are derived, not typed - that is the right habit. Cells and folios work as in QET. A QET veteran would draw these three folios in about 1.5 to 2 hours by drag and drop (plus manual cross-references and wire numbers); I needed 92 turns and about 700 steps, i.e. far more actions, though each was quick; with a mouse EW would likely be on par or faster for wiring and junctions and slower for black boxes and strips.

6. Flow chart
flowchart TD
  A[Create folios 101-103 + location + external refs] -->|smooth| B[Place devices 101 by Go to cell + Enter]
  B --> C{Parts?}
  C -->|only 4 parts, PartUse UUID| C1[Skip parts]
  B --> D[Autoconnect adds wrong wires] --> D1[Delete, use Alt+Enter]
  D1 --> E[Retag via Frozen tag - smooth]
  E --> F[Strip 1X1: -X?? renamed silently, preview card]
  F --> G[Wires by pressing cps - smooth]
  G --> H[Junctions by starting on a wire - very smooth]
  H --> I[IPs and external points]
  I --> J[Cross sections override - smooth]
  J --> K{Marker drag does nothing}
  K -->|Enter-move-Enter| L[Markers ok, delete stray]
  L --> M[102 black boxes: cps 0 then cps 7, redraw box]
  M --> N[Letter F refused, freeze tag]
  N --> O[IP direction flips, redo N-1]
  O --> P[Wires + junction styles bevel/chain/star - smooth]
  P --> Q[103 strips: part route ignores strip]
  Q --> R[Terminals by symbol, auto numbering - ok]
  R --> S[20 IPs + 20 wires - long but steady]
  S --> T{Bridges}
  T -->|refused x3, broken, no command yet| U[Function texts via multi-select]
  U --> V[Save + Export set.pdf 16 pages]

7. Good
- The page description: every connection point with its mm position, wires, potentials, junction sequences. It made blind drafting possible.
- Go to cell (101-B8) and Enter placement; cells and folios as in QET.
- Wires snap to the named cp and the status confirms 'from'; starting on a wire makes a junction with the right sequence; the junction editor shows style, wiring and the resulting wires.
- Potentials, wire numbers, cross-references and IP partner links derived automatically across folios.
- Cross-section override with reason text; multi-select + one property set (10 wires, 4 terminals at once).
- Terminal tool auto-numbers the next terminal; the strips table (potential, clamp a/b ends) is a great overview.
- Generated partlist, terminal diagram, wire list folios; Save and Export PDF in one step each, no dialogs.
- Error messages that list the valid options (tag:, column keys, next free counter).
