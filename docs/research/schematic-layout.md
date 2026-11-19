# How a circuit diagram is laid out, and how routing, placing and labelling are automated

Research note for the redesign of ElektraWatt's schematic autorouter, which is to place devices and parts as well.
Read 2026-11-19. It names no person, customer or project, and it states every rule from a standard in its own words.

**How sure each statement is.** Every rule carries one of these marks:

| Mark | Meaning |
|---|---|
| **P** | Read in a primary source: the standard's own public preview or contents list, or a tool's own manual for what the tool does |
| **S** | Read in a reliable secondary source: a textbook sample, a national committee's or a university's summary, a vendor's description of a standard |
| **T** | Trade practice read in a tool manual or training text (it shows what tools do, not what a standard requires) |
| **K** | Practice the author knows but did not find in a source this round: **not verified** |

**What could be read of the standards.** The text of IEC 61082-1, ISO 1219-2, ISO 128 and ISO 3098 is sold. Public
previews stop before the rules. Two previews were read as primary sources:

- The VDE preview of IEC 61082-1:2014. It gives the complete contents list, figure list, scope and terms, but none of
  clauses 5 to 7.
- DIN's contents lists of ISO 1219-2:2012 and DIN ISO 1219-2:2019.

These show which rules exist and where they sit. The content of the rules comes from secondary sources and tool
manuals. Section 9 lists what could not be verified.

Sketches beside this file:

- [`sketch-power.svg`](../images/schematic-layout/sketch-power.svg)
- [`sketch-control.svg`](../images/schematic-layout/sketch-control.svg)
- [`sketch-plc-inputs.svg`](../images/schematic-layout/sketch-plc-inputs.svg)
- [`sketch-plc-outputs.svg`](../images/schematic-layout/sketch-plc-outputs.svg)
- [`sketch-bus.svg`](../images/schematic-layout/sketch-bus.svg)
- [`sketch-pneumatic.svg`](../images/schematic-layout/sketch-pneumatic.svg)

In all of them, **one step is the page's connection grid**: 4 mm on a native page, which is also EPLAN's default for
schematic pages.

---

## 1. Summary: ten findings a designer must not miss

1. **An IEC circuit page is a structured grid, not a free graph.**
   - Potentials run as horizontal rails along the top and bottom of the page.
   - Each circuit is a vertical current path in its own numbered column, between the rails.
   - The column numbers are the address that every cross-reference uses (page.column).
   - Sources: the German circuit-diagram literature (S) and EPLAN's path numbering (T).
   - The placer's job is mostly assigning symbols to slots (page, then column, then height in the column). Optimising
     free positions is secondary. A route on such a page is mostly a straight vertical drop.
2. **The standard's own drawing rules are few and qualitative.** IEC 61082-1:2014 has clauses on the following
   (P for their existence, S for their content):
   - flow direction (7.1.1);
   - connecting lines, including avoiding bends and crossings and the spacing of lines (7.1.3, Figs 36–37);
   - where a reference designation stands at a symbol (7.1.6, Fig. 45);
   - terminal designations (7.1.7);
   - lining up symbols and grouping functionally related ones (7.4.2, Figs 64–65);
   - attached and detached presentation (7.4.3).

   The rules say "minimise", "align", "group" and "keep apart". They give no weights and no numbers beyond the symbol
   module. Numbers come from the module and from tool practice:
   - IEC 60617 symbols were built on a module M = 2.5 mm, with connecting lines spaced at 2M so that a terminal
     designation fits between them (S).
   - EPLAN uses a 4 mm grid for schematic pages and 2 mm for fluid pages (P).
3. **Crossings matter most for readability, then path continuity, then bends** (Purchase 1997; Ware et al. 2002).
   - Symmetry and the other aesthetics matter less.
   - Orthogonal drawings cross at right angles, which costs readers least (Huang et al. 2014).
   - A wire that turns back or detours breaks the reader's path more than its extra length suggests.
   - No controlled study of *circuit* diagrams was found. The ranking comes from studies of general graphs.
4. **Most ECAD tools do not route on schematics.**
   - EPLAN, WSCAD, QElectroTech and AutoCAD Electrical's Interconnect connect points that line up horizontally or
     vertically ("autoconnect"). Junction symbols (T and double junction) declare the wiring order.
   - Routing is left to the layout: the person places symbols so that their pins line up.
   - Only E3.series and Altium have a schematic auto-route. Both use a small cost model: bends, crossing symbols,
     crossing wires, short against long.
   - Lesson: an automatic *placer* that lines pins up does more for these pages than a cleverer router.
5. **Potentials need a trunk model.**
   - A rail is a multi-pin net whose trunk is fixed and horizontal. Each pin reaches it by one vertical drop that ends
     in a T on the rail.
   - ElektraWatt routes every connection as an independent two-ended job (FR-CON-001). Rails exist only as wires that
     happen to line up.
   - Trunk-and-branch (rectilinear Steiner) routing is the textbook answer, and yFiles' bus router is the
     product-level answer.
6. **Labels and routes must be solved together, but texts are not hard walls.**
   - Today the router treats every text as an obstacle (with a detour limit of 8/5), and the scene moves tags off wires
     afterwards. Each yields to the other, which gives loops round captions (ADR-0033 amendments 2 and 5).
   - The literature handles labels as candidate positions per text, resolved with a conflict graph:
     - Christensen, Marks & Shieber 1995;
     - Kakoulis & Tollis 2003;
     - KiCad's field autoplacer.
   - A tag has 3–5 conventional places, so the problem is small.
7. **Negotiated congestion (PathFinder) is the standard cure for net-order dependence.**
   - ElektraWatt uses a fixed order with two rip-up rounds, and ADR-0033 rejected negotiation as hard to explain.
   - On a structured page the order problem shrinks. Rails and straight drops leave few nets for the general search.
   - Channel track assignment (left-edge) orders and spaces parallel runs without any search.
8. **Placement methods that fit, and those that do not.**
   - Fit: layered layout with fixed port sides (Sugiyama in the ELK Layered style: layers are heights between the
     rails, the order within a layer is the column order).
   - Fit: constraint-based compaction or alignment (VPSC/IPSep-CoLa style) with pinned objects as hard constraints.
   - Do not fit: topology-shape-metrics, HOLA, force-directed layout and ML generators. They ignore rails, fixed pin
     sides and flow, or they are not stable under edits.
   - Most practical tools place by **templates**, not by optimisation: macros, PLC page macros with 8 channels per
     page, AutoCAD's ladder and PLC generators.
9. **Placing by hand must stay free; incremental changes must keep the picture.**
   - Users fight unwanted joins and snaps. KiCad issue 16834 and AutoCAD's trap distance show this.
   - Re-running a layout destroys the reader's mental map (Misue et al. 1995; North 1995).
   - Good tools reroute only what an edit touched: libavoid transactions, yFiles scope, EPLAN Smart connect.
   - For placing by hand, light alignment guides (snap-dragging, Bier & Stone 1986; persistent guides, StickyLines
     2016) are the documented alternative to computed positions.
10. **Licences decide what can be reused.**
    - Free to use from MIT/Apache crates: petgraph, pathfinding and rstar (MIT/Apache) for graph search and spatial
      indexes, and rust-sugiyama (MIT).
    - libavoid/Adaptagrams (LGPL-2.1+, C++) can be linked only from GPL code, by FFI.
    - ELK (EPL-2.0 OR GPL-3.0-or-later, Java) can be *ported* only into GPL-3.0-or-later code.
    - OGDF (GPL) is the same, and fits poorly anyway.
    - Nothing ready-made routes on a grid with rails, pin sides and labels. The core has to be written; the literature
      tells how.

---

## 2. The rules of the drawing

This section gives the background to the rules table in section 4. It corrects and extends
`docs/research/standards.md`, which it does not repeat.

### 2.1 IEC 61082-1:2014: what exists, and what the clauses say

The contents list of the 2014 edition (P, VDE preview) places the rules that matter here:

| Clause | Title (paraphrased) | What it covers, as far as it could be read |
|---|---|---|
| 5.1, 5.2 | Legibility; text orientation | Texts can be read from the bottom or from the right of the sheet, so they stand at 0° or 90° only (S: ibKastl wiki; company design standard BN-DS-E2) |
| 5.7.3 | Content area of a page | Reference grid (Fig. 8) |
| 5.8 | Cross-references | Reference to page and grid zone (Fig. 9) |
| 5.10, 5.11 | Line widths; text fonts | Values not read (see 2.2) |
| 5.12.2, 5.12.3 | Symbol size; orientation of symbols | Symbols may be enlarged, turned and mirrored (Figs 12–13) |
| 5.20.2, 5.20.3 | Reference designations; simplified presentation | The *common initial portion* of the designations on a page or in a frame is written once and left out at each symbol (Fig. 19) |
| 7.1.1 | Flow of energy, signal | Flow runs left to right and/or top to bottom; inputs on the left or top, outputs on the right or bottom (S: IEC 60617 symbol notes; a protection relay manual) |
| 7.1.3 | Connecting lines | Joining (Figs 29–31), crossings (Fig. 30), bundles (Figs 39–40), **avoiding bends and cross-overs** (Fig. 36), **spacing of lines** (Fig. 37) |
| 7.1.5 | Boundary frames | Frames round functional groups, designations at the frame |
| 7.1.6 | Presentation of reference designations | **Location of the designation at a symbol** (Fig. 45) and at lines (Fig. 46); the 2014 edition adds exceptions to the placement rule (S: iTeh abstract) |
| 7.1.7 | Presentation of terminal designations | Fig. 52 |
| 7.1.9 | Multi-phase circuits | Single-line or multi-line (Fig. 56) |
| 7.4.2 | Circuit diagrams: layout | **Lining-up of symbols** (Fig. 64) and **grouping of functionally related components** (Fig. 65) |
| 7.4.3 | Methods for representation of components | **Attached** (Fig. 66) and **detached** (Fig. 67) presentation, inset tables (Fig. 68) |
| 7.4.4 | Components with movable parts | Orientation of contact symbols (Fig. 75) |
| 7.4.5 | Supply circuits | AC and DC supply circuits (Figs 76–77) |
| 7.5 | Connection diagrams | Terminals, cables, cores |
| 12 | CAx conformance requirements | Requirements on tools |

Its content, from secondary sources:

- **Lines** (S: ibKastl wiki's summary of DIN EN 61082-1):
  - A connecting line should not cross other symbols.
  - Bends and crossings are kept to a minimum.
  - A line may be interrupted (with a designation and a cross-reference) to avoid bends and crossings.
- **Detached presentation** (S: de.wikipedia, textbook sample):
  - The parts of one device (coil, contacts) are drawn where their circuits need them.
  - The parts are tied together by the same reference designation and by cross-references.
- **Contact image.** The trade's contact image under the coil comes from the cross-reference rules. EPLAN places it
  "in path", aligned vertically below the coil (T).
- **De-energised state** (S: de.wikipedia; BN-DS-E2):
  - Contacts are drawn de-energised and not actuated.
  - Any deviation is noted.
- **Junctions.** A T junction joins; a crossing does not (P for the figures' existence; T for the tools).

### 2.2 Other standards

- **IEC 60617 and ISO/IEC 81714-1** (symbol design):
  - The symbols of the 1996 edition of IEC 60617-2 were drawn on a module **M = 2.5 mm**.
  - Connecting lines leave a symbol at multiples of the module. **2M was chosen as the spacing between connecting
    lines** so that a terminal designation fits between them (S: search summary of DIN EN 60617-2:1996).
  - IEC TS 63064:2018 Annex A specifies an orthogonal electronic grid with M = 2.5 mm (S: catalogue entry).
  - ISO 81714-1:2010 has clauses on the grid and module (6.5) and on connect nodes (6.10) (S: catalogue).
  - In practice:
    - EPLAN draws schematic symbols with connection points on a 4 mm grid (P: page-type grid; S: an ABB library rule
      of 4 mm);
    - ElektraWatt's native symbols use a 4 mm pin grid (doc 07a);
    - QElectroTech uses 10 px.
  - **Rule a program can use:** pins on the page grid; parallel lines at least one grid step apart, and two steps where
    a text stands between them.
- **IEC 81346-1/-2** (reference designations):
  - Where the designation is *written* on a diagram is IEC 61082-1's matter (7.1.6), not IEC 81346's.
  - IEC 81346 allows leaving out the common initial portion when the context (page, frame, title block) gives it
    (S: myElectrical; P for IEC 61082-1 5.20.3 and Fig. 19).
  - Effect on layout: a page-relative tag ("-K1") is short. A fully qualified one ("=A1+S2-K1") is three to four times
    as wide and needs a wider text slot.
- **ISO 128 and ISO 3098** (lines, lettering):
  - Lettering heights follow the series 2.5, 3.5, 5, 7, 10, 14, 20 mm, stepping by √2 like the paper sizes (S:
    cadsetterout, draftsperson).
  - The line width for lettering is h/10 (type B) or h/14 (type A) (S).
  - ISO 128 line widths form the series 0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0 mm, with thick lines twice
    the thin ones (K, not verified this round).
  - For the router these fix the size of text boxes (2.5 mm text is common on A3 schematics, K) and the minimum gap
    that keeps two lines apart in print.
- **ISO 5457 and ISO 7200** (frame, zones, title block):
  - Reference-grid fields are 50 mm, counted from the centring marks: A3 has 8 × 6 fields and A4 has 6 × 4.
  - Letters (without I and O) run top to bottom, numbers run left to right.
  - The grid border sits 5 mm inside the frame.
  - Labels are 3.5 mm high (S: cadsetterout).
  - The title block's position and width were not read this round; `standards.md` §12.2–12.3 holds what is known.
  - **Electrical practice uses columns that are current paths instead of 50 mm fields.** EPLAN's help uses 10 paths
    per frame as its example (T). An A3 page at 4 mm then has columns of about 9–10 grid steps.
- **IEC 60204-1** (documentation of machines):
  - Each conductor must be identifiable at each termination, in agreement with the documentation (S).
  - Training material reads this as "a plain dot at a potential tap does not tell where each wire lands". The drawing
    should show the wiring order at junctions (S: ibKastl wiki). EPLAN's T-node variants and WSCAD's destination
    wiring elements encode exactly that (P).

### 2.3 The trade's practice in the IEC world

- **Control circuit in detached presentation with current paths** (S: de.wikipedia "Stromlaufplan"; the textbook
  sample on Voltimum; T: EPLAN):
  - The supply potentials run horizontally along the top and bottom edges of the circuit.
  - Each current path runs vertically between them and is numbered.
  - The plan is drawn as free of crossings as possible. In building services PE is often left out so that it does not
    cross the paths (S).
  - Symbols of one path stand on one vertical axis.
  - Commands come first (stop and NC contacts above the start and NO contacts), then interlocks, then the coil, which
    sits at the bottom of the path next to the lower rail (K; the sketches of every textbook show it).
  - The contact image stands directly below the coil, under the lower rail, and lists each contact with the path where
    it is drawn (T: EPLAN "in path, align automatically").
- **Power circuit:**
  - Drawn from the supply rails (L1, L2, L3, N, PE, in that order from the top) downward.
  - Each feeder runs: protection (fuse, circuit breaker or motor protective breaker), switching device (contactor main
    contacts), overload relay where separate, terminal strip, load (motor, with PE) (S: textbook example; K for the
    general rule).
  - Each pole drops from its rail with a junction on that rail and crosses the rails below it without joining.
- **PLC pages** (T: EPLAN help):
  - A channel has one active I/O connection point.
  - Overview pages show the whole card and are kept in cross-reference agreement with the channel pages.
  - The PLC macro example puts 8 channels on a page, with the placement direction set by the frame.
  - Common practice draws the card as a box along one edge of the circuit with one field device per channel, each in
    its own column (K; an industry article describes both a whole-module box and a channel-per-path form, S).
- **Terminals and cables:**
  - Terminals are drawn in the path, between the device and the field device or load. Their strip designation is
    written once, beside the first terminal (K; T: AutoCAD and EPLAN examples).
  - Cables are shown by a cable designation at the bundle of cores they carry (P: IEC 61082-1 7.5.4 and Fig. 83 exist).
- **PE.** On machine schematics PE is the last rail and drops to each load's PE terminal. Its colour profile is in
  `standards.md` §6 (K).

### 2.4 North American practice, for contrast (NFPA 79, ladder diagrams)

- Two vertical rails stand at the left and right. Rungs run horizontally between them, numbered down the left side
  (S: EC&M).
- Inputs (stop first, normally closed, in series; then start, normally open, in parallel) stand on the left; outputs
  (coils, lamps, solenoids) stand on the right, in parallel.
- Numbers on the right of a coil's rung name the rungs that hold its contacts. A number with an underline (or brackets)
  marks a normally closed contact (S: EC&M; not confirmed as a standard).
- Wire numbers change at every device. They are often tied to the rung number (T: AutoCAD Electrical's line-reference
  tags and wire numbers).
- AutoCAD Electrical's ladder has a rung spacing, a width, a first reference and an index step. Inserting a component
  on a rung breaks the wire (T).
- The layout is the IEC current path turned by 90°. The same router and placer can serve both if the flow axis is a
  page setting.

### 2.5 Fluid power: ISO 1219-1 and ISO 1219-2

- **ISO 1219-2:2012** (P, contents list) has these clauses:
  - general rules on presentation (4.1), format (4.2), **layout (4.3, pages 5–8)** and equipment (4.4);
  - the identification code of components (5.1);
  - port identification (5.2);
  - piping codes (5.3, 5.4);
  - technical information per component (6.4);
  - an electrical reference designation on the fluid diagram (6.3);
  - example diagrams (Annexes B–D).

  The text of 4.3 was not read.
- **Identification code.** The 2019 DIN contents list writes the component code as *installation code – medium code
  and circuit number . component number* (P for the structure). The older letter codes for component kinds, as in the
  teaching notation 1A1 and 1V1, are no longer part of the 2012 code (S: ibKastl wiki).
- **Arrangement:**
  - Energy flows from bottom to top: the compressed-air supply is at the bottom, and the actuators are at the top
    (S: RS guide; Misumi).
  - Actuators sit at the top in order of operation, left to right. The other components stand below them in sequence,
    from the bottom up and left to right (S, weak: a training deck).
  - Cylinders and directional valves are drawn horizontally, in the initial position, without regard to their
    physical place; each actuator has its own vertical chain (K: Festo teaching material; not verified this round).
- **Module.**
  - EPLAN's default grid for fluid pages is 2 mm (P).
  - ElektraWatt's fluid module is a 12 mm valve position with ports 4 mm apart (doc 11).
  - Line types are not verified (`standards.md` §5).

### 2.6 Corrections and additions to `docs/research/standards.md`

- **§2.1 (IEC 61082-1).**
  - The clause numbers there are marked unverified. The contents list now confirms them: 5.8 cross-references;
    5.20.2–5.20.3 designations and their simplified presentation; 7.1.3 connecting lines; 7.1.6 reference
    designations; 7.1.7 terminal designations; 7.4.2 circuit layout; 7.4.3 attached and detached presentation.
  - The placement "left of or above the symbol" is still not verified in the text. Fig. 45 shows it, but its content
    was not readable.
- **§4 (IEC 60617).** Add the 2.5 mm module and the 2M spacing of connecting lines (S), and IEC TS 63064:2018
  Annex A (grid) as the current guidance document.
- **§5 (ISO 1219-2).**
  - The code structure (installation – medium + circuit . component) is now confirmed from the contents list.
  - Add that clause 6.3 covers electrical reference designations on fluid diagrams, which supports the cross-reference
    from the solenoid to the electrical page.
  - The teaching notation 1A1/1V1 is the older practice.
- **§12.3 (ISO 5457).** The grid border is 5 mm inside the frame, and the zone labels are 3.5 mm high (S).

---

## 3. What makes a drawing good, measurably

Evidence, strongest first:

1. **Crossings.**
   - Purchase (1997) found crossings to be by far the most important of the aesthetics she tested. Bends and symmetry
     had smaller effects.
   - Ware, Purchase, Colpoys and McGill (2002) measured path finding. After the length of the path, its **continuity**
     (how straight it runs through each node) and the crossings *on that path* governed the time readers needed.
2. **Crossing angle.** Huang, Eades and Hong (2014) showed that crossings near 90° cost readers little, and small
   angles cost a lot. Orthogonal drawings get this for free. What remains is the *number* of crossings.
3. **Bends and orthogonality.** Tamassia, Di Battista and Batini (1988) ranked readability goals for orthogonal
   drawings as crossings, then bends, then area. Purchase's metrics (2002) define orthogonality and bend counts as
   measurable scores.
4. **Domain matters.** Purchase, Carrington and Allder (2002) found that only some aesthetics matter significantly,
   and which ones depends on the domain. For UML, orthogonality and direction were among the preferences.
5. **Human-like layouts.** In the HOLA study (Kieffer et al. 2016), readers preferred layouts that follow what people
   do (alignment, compactness, trees placed tidily) and did tasks faster on them.
6. **Stability.** Re-running a layout destroys the reader's mental map (Misue, Eades, Lai and Sugiyama 1995). Dynamic
   layout ranks consistency and stability before readability (North 1995).
7. **Labels.** Cartography's rules, from Imhof (1975):
   - a label is near its object and unambiguous;
   - it does not overlap;
   - it has preferred positions (upper right first for points).

   These are the basis of point-feature label placement (Christensen, Marks and Shieber 1995). Edge labels follow the
   same rules (Kakoulis and Tollis 2003). A study in a layered layout found labels on the edge more compact, and they
   were preferred with direction arrows (Schulze et al. 2018).
8. **The draughtsman's rules of thumb** (S where cited, else K):
   - straight runs over detours;
   - no wire along a symbol's edge or just in front of a pin it does not join (it reads as joined);
   - corners never touching another wire;
   - no T on a crossing;
   - parallel wires keep their order through a bend (nested, not crossed);
   - repeated circuits drawn alike;
   - a free column left for later additions.

   ElektraWatt's ADR-0033 amendments 3 and 4 already record several of these as observed problems.

What a program can measure on a page is in the rules table (rules R-1 to R-14).

---

## 4. The rules table

Columns:

- **Rule:** in one sentence.
- **Source:** with the sureness mark.
- **Measure / enforce:** how a program checks or keeps it.
- **ElektraWatt today:** from ADR-0033 with amendments 1–5 and the code in `crates/ew-derive/src/routing/` at HEAD.
  "Partly" says what is missing.

Facts about today's router used below:

- **Search.**
  - An exact Dijkstra over states of (grid node, arriving heading), integer costs, a 4 mm grid.
  - Costs: step 2, bend 8, crossing 12, a step beside a parallel mate 1, entering a text 20 plus one step per text step.
- **Hard rules.**
  - It never passes a pin it does not join.
  - It never runs along a wire.
  - It never crosses a body or the frame bands.
  - It never passes a junction it does not end at, and makes no four-way junction.
  - No corner touches another wire.
  - Texts block, unless every clear way is more than 8/5 as long.
- **Routing jobs.** Every connection is a two-ended job. The page is solved in a fixed order (supply potentials, then
  span) with two rip-up rounds and two improvement passes.
- **Grids.** A half-grid fallback exists between close pins.
- **No placement.** There is no symbol placement. Tags are moved off wires by the scene afterwards.

### 4.1 Drawing rules (section 1 of the brief)

| ID | Rule | Source | Measure / enforce | ElektraWatt today |
|---|---|---|---|---|
| D-1 | Connecting lines run only horizontally and vertically. | IEC 61082-1 7.1.3 (P exists, S); every IEC tool (P) | Every segment axis-parallel | **Yes** |
| D-2 | Connection points and line vertices lie on the page grid. The grid is a multiple of the symbol module (2.5 mm in IEC 60617; 4 mm in EPLAN and ElektraWatt). | DIN EN 60617-2:1996 (S); EPLAN page-type grid (P); doc 07a | Each vertex mod grid = 0 (after settling within 1 nm) | **Yes**. Half-grid corners allowed between close pins (amendment 3). |
| D-3 | A connecting line never crosses a symbol. | IEC 61082-1 7.1.3 (S: ibKastl) | Count of segment–body intersections = 0 | **Yes** (hard rule) |
| D-4 | Bends and crossings are kept to a minimum. | IEC 61082-1 7.1.3, Fig. 36 (P exists, S) | Total and per-wire counts of bends and crossings | **Yes** (costs 8 and 12) |
| D-5 | A long way round is replaced by an interruption point with a cross-reference. | IEC 61082-1 7.1.3 / 5.8 (S: ibKastl) | A route above a length or bend threshold, or crossing more than *k* wires, becomes a pair of interruption points | **No.** The router never proposes an interruption; IPs are placed by hand. |
| D-6 | Parallel lines keep a uniform spacing, wide enough for the texts written between them (2M in IEC 60617; one grid step at least). | IEC 61082-1 Fig. 37 (P exists); DIN EN 60617-2 (S) | Minimum gap between parallel segments ≥ 1 step; ≥ text height + clearance where a text stands between; variance of gaps in a bundle | **Partly.** One step is hard (no run along a wire), and mates are drawn one step apart. There is no text-aware spacing and no track assignment. |
| D-7 | A joint is a T junction with a dot (or a T symbol). A crossing is never a joint. No four-way joint; where one is needed, two offset T junctions. | IEC 61082-1 Figs 29–31 (P exists); KiCad, ProfiCAD, AutoCAD (P) | Every degree-3 point has a junction; no degree-4 point; no junction on a crossing | **Yes** (four-arm rule; no T on a crossing, amendment 4) |
| D-8 | Where wiring order matters, a junction shows which way the conductor runs (target order). | IEC 60204-1 13.2.1 as read by training material (S); EPLAN T-node variants (P); WSCAD destination wiring (P) | Each junction has a declared order; drawn form matches it | **Partly.** Junction `sequence` and Star/Chain exist (doc 06 §3.5); the router does not use them. |
| D-9 | Energy and signals flow left to right and/or top to bottom; inputs left or top, outputs right or bottom. | IEC 61082-1 7.1.1 (P exists, S); IEC 60617 symbol notes (S) | Share of source-to-load pairs placed above or left of their load; share of wire segments that run against the flow | **No.** No flow notion in router or placement. |
| D-10 | Supply potentials are horizontal rails along the top (and bottom) of a page, in the order L1, L2, L3, N, PE (control: L+ on top, M below). | de.wikipedia "Stromlaufplan" (S); textbook (S) | Rails are single horizontal lines spanning the circuit; order as listed | **No** rail concept ("no rail routing concept", doc 06 §3.5); supply potentials are only routed first. |
| D-11 | Each circuit is one vertical current path in its own numbered column; cross-references give page and column. | de.wikipedia (S); EPLAN path numbering (P) | Symbols of a path share one x axis (± 0); one path per column; column index derivable for every symbol | **No** (no placement). Column numbers exist only in the frame grid and xref tokens. |
| D-12 | Symbols of a path are lined up on one axis; functionally related components are grouped. | IEC 61082-1 7.4.2, Figs 64–65 (P exists) | Number of distinct pin x-coordinates in a path, against its minimum; bounding-box compactness of a group | **No** |
| D-13 | Detached presentation: coil and contacts of one device stand in different paths, tied by the same tag and by cross-references. A contact image under the coil lists each contact and its path. | IEC 61082-1 7.4.3 Fig. 67 (P exists); EPLAN contact image (P) | Every contact has a cross-reference; the contact-image box stands directly below its coil, in the coil's column | **Partly.** Cross-references and contact image exist (doc 06 §2.8). The space under the coil is not reserved by any placer. |
| D-14 | A power feeder runs down its column: protection, switching, overload, terminal, load (motor with PE). | Textbook example (S); practice (K) | Vertical order of the symbols of a feeder = topological order from rail to load | **No** |
| D-15 | Contacts are drawn de-energised and not actuated; contact symbols are oriented so that they operate left to right or bottom to top. | de.wikipedia (S); BN-DS-E2 (S); IEC 61082-1 Fig. 75 (P exists) | Symbol variant in the allowed set for the path direction | Symbol matter (not the router) |
| D-16 | Texts are readable from the bottom or the right of the page (0° or 90° only). | IEC 61082-1 5.2 (S: ibKastl); BN-DS-E2 (S) | Text angle ∈ {0°, 90°} | **Yes** in the scene. Texts no longer turn with the symbol beyond that. |
| D-17 | A reference designation stands next to its symbol, outside it, not crossed by a line. In a vertical path it stands on the left of the symbol; with horizontal lines, above. | IEC 61082-1 7.1.6, Fig. 45 (P exists; side not verified); tool defaults (K) | Distance from the text box to its own symbol ≤ *d* and smaller than to any other symbol; no line through the text; one side per page | **Partly.** The scene tries five places (mirrored, raised, dropped, above, below), and the router avoids texts. There is no common side rule and no joint decision with the router. |
| D-18 | Terminal designations stand outside the symbol, beside the connecting line at the terminal, not on it. | IEC 61082-1 7.1.7, Fig. 52 (P exists); DIN EN 60617-2 (S: 2M spacing for them) | Text box within one step of its pin and beside, not on, the wire leaving the pin | **Partly.** Pin texts are router obstacles. Overlaps between neighbours are not resolved. |
| D-19 | The common initial portion of designations is written once (title block, frame) and left out at the symbols. | IEC 61082-1 5.20.3, Fig. 19 (P exists); myElectrical (S) | Width of a tag text depends on the display mode; the placer reserves for the mode in use | **Partly.** The display setting exists; long qualified tags then collide (the placer must size slots from the actual text). |
| D-20 | The page carries a reference grid. For circuit pages this is the column (path) grid, numbered left to right; rows by letters without I and O where used. | ISO 5457 (S); EPLAN plot frame (P) | Every symbol's column and row computable; cross-reference text derived from it | **Yes** (parametric frames, `{page}-{row}{column}`) |
| D-21 | Text heights from the series 2.5, 3.5, 5 … mm; line widths from the ISO 128 series, one or two per diagram. | ISO 3098 (S); ISO 128 (K) | Text box size from height × length; minimum printed text 2.5 mm at the output scale | Scene matter; the router reads the boxes |
| D-22 | A multi-phase circuit may be drawn single-line, with a mark for the number of conductors. | IEC 61082-1 7.1.9 (P exists) | — | Out of scope for routing today |
| D-23 | On a ladder (NFPA 79 practice), rungs are horizontal and numbered on the left, inputs on the left and outputs on the right, contact cross-references at the right of the coil. | EC&M (S); AutoCAD Electrical (P) | As D-9 to D-13 with the flow axis turned | **No**; a page flow setting would cover it |
| D-24 | PLC: one active connection point per channel; channel pages show one field device per channel; an overview page shows the card and links each channel to its page and column. | EPLAN help (P) | Each channel pin has at most one field-device chain; overview cross-references complete | **Partly** (PLC data model exists; no page arrangement) |
| D-25 | Fluid: energy flows from bottom to top; supply and service unit at the bottom, valves above it, actuators on top in order of operation. | RS, Misumi (S); ISO 1219-2 4.3 exists (P) | Vertical order supply < valve < actuator per chain; actuators sorted by sequence left to right | **No** (tubes use the electrical router; no fluid placement) |
| D-26 | Fluid: each actuator has its own vertical chain; cylinders and directional valves are drawn horizontally, in the initial position. | Festo teaching (K) | One column band per actuator; symbol rotation 0° | **No** |
| D-27 | Fluid: components carry the ISO 1219-2 code (installation – medium + circuit . component) or the project's IEC 81346 tag, next to the symbol; ports carry their numbers at the port. | ISO 1219-2 5.1, 5.2 (P structure); doc 11 | As D-17 and D-18 | **Partly** (port designations as symbol texts; tags as D-17) |

### 4.2 Readability rules (section 2 of the brief)

| ID | Rule | Source | Measure on a page | ElektraWatt today |
|---|---|---|---|---|
| R-1 | Fewest crossings: the most important single aesthetic. | Purchase 1997; Ware et al. 2002 | Count of wire–wire crossings; crossings per wire; crossings on the main path of a circuit | **Yes** (cost 12, about one and a half bends) |
| R-2 | Crossings at right angles only. | Huang et al. 2014 | All crossings 90° (automatic when orthogonal) | **Yes** |
| R-3 | Path continuity: a wire runs as straight as it can; no turning back; no detour. | Ware et al. 2002 | Detour ratio = route length / Manhattan distance of the ends; count of U-turns (two bends in opposite senses); bends | **Partly**: bends cost 8; detours over 8/5 are refused only against texts (amendments 2–3) |
| R-4 | Fewest bends, and few per wire. | Purchase 1997; Tamassia et al. 1988 | Total bends; maximum bends per wire | **Yes** (cost) |
| R-5 | Short total length, and uniform lengths among like wires. | Purchase 2002 (metrics) | Total length; variance of length among wires of one bundle | **Yes** (step cost); uniformity no |
| R-6 | Consistent flow direction. | Purchase et al. 2002; IEC 61082-1 7.1.1 | As D-9 | **No** |
| R-7 | Alignment: objects and pins on shared lines; repeated circuits drawn alike. | HOLA study (Kieffer et al. 2016); IEC 61082-1 7.4.2 | Distinct x and y coordinates used by pins against minimum; similarity of repeated sub-circuits | **No** (no placement) |
| R-8 | Symmetry: a weaker effect. | Purchase 1997 | Mirror or translation similarity of sibling circuits | **No** |
| R-9 | Uniform spacing; no near-misses. A wire does not run along a symbol's edge or pass just in front of a pin it does not join; corners do not touch. | Practice (K); ADR-0033 amendments 3–4 | Minimum wire–body gap ≥ 1 step; no wire within half a step of a foreign pin; no corner on another wire | **Yes** (bodies grown by one step; pins reserved; corners hard) |
| R-10 | Parallel wires keep their order through a bend (nest, do not cross); a bundle stays together. | Practice (K); ADR-0033 amendment 4 | Crossings among wires of one bundle = 0 | **Partly** (mate cost pulls mates together; no explicit ordering) |
| R-11 | A label is close to its own object, closer than to any other, and overlaps nothing. | Imhof 1975; Christensen et al. 1995 | Label–own-object distance; ambiguity (a nearer foreign object of the same kind); overlap area with texts, symbols and wires = 0 | **Partly** (tag moves of the scene; texts as router obstacles; overlaps remain) |
| R-12 | Compact, with reserve: little wasted area, but free columns for later additions. | Tamassia et al. 1988 (area); practice (K) | Used area / drawing area; free columns at the right of a page | **No** |
| R-13 | Stability: an edit changes as little as possible of what the reader already knows. | Misue et al. 1995; North 1995 | Number of unchanged objects that moved; wires rerouted per edit; displacement | **Partly**: routes are stored; a move re-solves only affected wires; a reroute never leaves the page worse (amendments 4–5). Placement does not exist yet. |
| R-14 | No false joint: no junction on a crossing, no corner on a wire, no wire end touching a foreign pin. | Practice (K); ADR-0033 | Counts = 0 | **Yes** |

---

## 5. Typical pages

Each page below is described on an A3 landscape drawing area of about 100 × 64 steps at 4 mm, with 10 numbered
columns of about 10 steps. The numbers are the arrangement the rules give. Spacings marked K are practice that the
designer should make page or project settings.

### 5.1 Supply and power circuit ([`sketch-power.svg`](../images/schematic-layout/sketch-power.svg))

- **Rails.** L1, L2, L3, N and PE are horizontal across the whole drawing area, one step apart (K: one or two steps), in
  that order from the top, starting 2–4 steps below the column-number band. They enter from the left as interruption
  points carrying the previous page's reference and leave on the right to the next page.
- **Feeders.** Each feeder takes one column, or two when it is wide.
  - Its poles drop from the rails at the symbols' pole pitch (2 steps in the sketch).
  - Each drop has a junction on its own rail and crosses the rails below without a dot.
- **Order down the column:**
  1. protection (circuit breaker or motor protective breaker), about 5 steps tall, 5–9 steps below the rails;
  2. contactor main contacts, about 5 steps below that;
  3. overload relay where separate;
  4. terminal strip (one terminal per conductor plus PE, in a row);
  5. the load: a motor circle about 6 steps across, near the bottom of the area.

  The PE drop runs at the right of the feeder from the PE rail to the terminal and on to the motor.
- **Texts.**
  - The tag stands left of each symbol, vertically centred on it.
  - Terminal numbers (1/2, 3/4, 5/6) stand beside the pins, left of each line.
  - Under the contactor's tag stands its coil's page and column.
- **Power supply.** A power supply for the control circuit takes its own column. It is fed from L1 and N through an
  MCB, and its outputs leave downward to interruption points for the control pages.
- **Free columns.** Unused columns at the right stay empty.

### 5.2 Control circuit in current paths ([`sketch-control.svg`](../images/schematic-layout/sketch-control.svg))

- **Rails.** L+ (or L/N for AC control) is the top rail, 2–3 steps below the column band, entering by an interruption
  point. M is the bottom rail at about 70 % of the area's height, so that a band of 10–16 steps below it stays free for
  contact images.
- **Paths.** One path per column, every symbol of a path on the column's axis.
- **Order from the top:**
  1. emergency stop and stop contacts (normally closed);
  2. start contacts (normally open);
  3. interlocks and overload contact;
  4. the coil.

  The coils of all paths stand on one line, 2 steps above the M rail. Symbols are about 4–5 steps tall, with 2–3 steps
  of line between them.
- **Branches.**
  - A parallel branch (the holding contact) takes the next column. It leaves and rejoins the main path with T
    junctions, at heights that do not meet another symbol.
  - Lamps and other loads in the same path family stand at the coils' height.
- **Texts.**
  - Tags stand left of each symbol.
  - Terminal numbers (13/14, A1/A2, 95/96) stand right of the pins.
  - The contact image stands below the coil, under the M rail, centred on the coil's axis. It is a cross with one row
    per contact: terminal pair on the left, path reference on the right.
  - A function text for the path ("Motor 1 running") may stand right of the load or in the column band.

### 5.3 PLC inputs ([`sketch-plc-inputs.svg`](../images/schematic-layout/sketch-plc-inputs.svg))

- **Rails.** L+ on top, M at the bottom.
- **The card.** The card is a box along the lower part of the area (about 8 steps tall), spanning the channel columns.
  - Its channel pins are on its top edge, each at a column's axis. The tag stands left of the box, the card type and
    slot inside.
  - The card's own supply pins (L+, M) are in the first column, at its left end.
- **Channels.** One channel per column, 8 channels per page (EPLAN's example, P). Above each channel pin, on the same
  axis:
  1. a terminal of the input terminal strip (6–10 steps above the card);
  2. above it, the field device (switch or sensor), fed from the L+ rail.
- **Three-wire sensors.** A third wire runs to the M rail, at the right of the channel's axis.
- **Texts.**
  - The channel address (I0.0) inside the card under its pin.
  - The terminal number beside the terminal.
  - The device tag left of the device.
  - The function text ("Start") under the card, on the channel's axis.
- **Wiring.** Every connection on this page is one straight vertical segment.

### 5.4 PLC outputs ([`sketch-plc-outputs.svg`](../images/schematic-layout/sketch-plc-outputs.svg))

- **Arrangement.** The mirror of the input page:
  - The card is along the upper part, fed from the L+ rail, its output pins on the bottom edge at the column axes.
  - Below each pin, on the same axis: the output terminal, then the load (relay coil, valve solenoid, lamp).
  - The loads stand on one line and return to the M rail.
- **Texts.**
  - Function texts stand between load and rail, or under the M rail.
  - Relay coils get contact images under the M rail, as on a control page.
  - Valve solenoids carry a cross-reference to the fluid page.

### 5.5 Bus layout ([`sketch-bus.svg`](../images/schematic-layout/sketch-bus.svg))

- **Page.** An overview page in single-line form, without current paths.
- **Stations.** Boxes about 12 × 9 steps in one row near the top, in the order of the network's topology from the
  controller at the left, 20 steps apart.
- **Ports.** On the edge that faces the network: the bottom edge here.
- **Star network.** Each port drops straight to the switch, a wide box below the row, whose ports stand on its top edge
  at each station's axis.
- **Line network (daisy chain).** Short horizontal links between neighbouring stations at one height.
- **A second network** (fieldbus, IO-Link) gets its own horizontal trunk at least 4 steps away, with straight drops.
- **Texts.** The cable tag stands beside each drop. The station tag stands above its box, at the right. The address
  (device name, IP or station number) is inside the box.

### 5.6 Pneumatic circuit ([`sketch-pneumatic.svg`](../images/schematic-layout/sketch-pneumatic.svg))

- **Levels**, from the bottom (D-25):
  1. the supply and service unit, at the bottom left;
  2. the supply line, horizontal along the bottom;
  3. directional valves, one under each actuator, with port 1 dropping to the supply line and the exhausts 3 and 5
     down to silencers;
  4. flow controls in the working lines;
  5. actuators on top, in the order in which they move, left to right.
- **Chains.** One chain per actuator, as a band of columns. The sketch uses 30 steps on a 4 mm grid; on EPLAN's 2 mm
  fluid grid the same picture is 60 steps.
- **Lines.**
  - Cylinders and valves are horizontal, in the initial position (D-26).
  - Working lines run vertically from ports 2 and 4 up to the actuator's ports, with one horizontal jog at a common
    height just above the valve.
  - Position sensors are marked at the cylinder's ends.
- **Texts.** Tags stand left of each component; port numbers stand at the ports, outside the symbol.
- **Valve terminals.** A valve terminal places the valves of all chains side by side on one sub-base, whose gallery
  replaces the separate supply drops. The chains above it then keep the valves' pitch.

---

## 6. What the tools do

From the tools survey (all read 2026-11-19; source URLs in section 10).

- **Plain URL:** the page was opened and read.
- **"snippet":** only a search summary was seen.

| Tool | How connections come about | Automatic routing | What helps placing | Texts kept apart | Note |
|---|---|---|---|---|---|
| **EPLAN Electric P8 / Fluid** | **Autoconnect:** two connection points exactly facing each other, horizontally or vertically, are joined, with a preview while placing. Each point has a direction arrow. T-nodes (4 directions × 4 variants) and double junctions declare the target order. A "break point" stops an unwanted join. | None on schematics. The help says the course of a connection is drawn with T-nodes and double junctions. | Window, page and symbol macros with placeholders and value sets. PLC page macros (8 channels per page in the example), filled in the frame's direction. Pre-planning and Cogineer generate pages from typicals. **Smart connect** redraws autoconnect lines after a move, and puts interruption points on cut lines. | Property arrangements per symbol variant. The contact image goes "in path", aligned below the coil, or "on component". Cross-references are shortened by context. No collision check found. | Grid per page type: 4 mm schematic, 2 mm fluid, 2.5 mm P&ID. Path numbering per frame (10 paths in the example). |
| **AutoCAD Electrical** | Wires drawn by hand, snapped to the nearest connection point within a "trap" distance. The digit in a pin attribute gives its preferred side (1 right, 2 up, 4 left, 8 down). At most three wires per point. | **Interconnect** joins only aligned points. **Multiple Bus** draws parallel phase wires at the rung spacing. Wire gaps or loops at crossings. | Ladders (width, rung spacing, reference numbers, index). A component inserted on a wire breaks the wire. Scoot and Align keep wires attached. Circuit Builder places from templates and a spreadsheet. PLC I/O drawings from a spreadsheet (ladders per drawing, rungs, spacing, device offsets). | XREF attributes in the symbol. In-line wire labels centred between connection points. No collision check found. | Users report wires taking the wrong pin when pins are closer than the trap (snippet). |
| **SOLIDWORKS Electrical** | Single and multiple wire commands, with crossing and uncrossing keys. A symbol dropped on a wire cuts it if it has connection points. | None found | Macros, Excel automation, PLC "generate drawings" from channel mapping (snippets) | None found | *Move* detaches wires, *Stretch* keeps them (P: reseller blog). |
| **Zuken E3.series** | Auto Route in five cases: aligned pins; moved pins already connected (rubber band); two selected pins; pins with the same signal name; pins already joined. | **Yes.** Costs 0–100 for turns, crossing symbols and crossing connections; turn distance in grid multiples; a priority between short and long connections. | Wiring diagram generator from part and connection lists | None found | The clearest published cost model of a schematic router. |
| **QElectroTech** | Autoconnect toggle: an added element connects to terminals on the same horizontal or vertical line. Conductors otherwise drawn terminal to terminal. | Default path: a stub from each terminal in its direction, then an L or a Z with the bridge at the midpoint, on a 10-unit grid. | — | Conductor label centred on the conductor or its longest segment | Keeps the user's shape as a **profile** per travel direction. On a move it shares the change evenly over the stretchable segments, clamps reversals, and falls back to the default path. |
| **KiCad Eeschema** | Wires drawn by hand. Ends must coincide; crossings never join; a junction dot is added where a wire ends on a wire. | **None** (the moderator's "not in the foreseeable future" concerns the PCB side). Third-party scripts route orthogonally. | Move (detaches) and Drag (keeps wires) are separate commands. | **Autoplace fields:** ranks the four sides by symbol shape; avoids pins and, in manual mode, collisions; fits between neighbouring wires; snaps to the grid. | Users debate rubber-banding: some want rigid blocks; cleanup after a drag can cost as much as redrawing. An issue asks to switch off accidental joins. |
| **WSCAD** | F10 automatic connection: horizontal and vertical lines drawn when symbols are placed, cut and healed when they are removed. Destination-wiring elements (angles, T-pieces, arrows) declare targets. | None found | Macros with variables, project wizard, automation interface from configurators. A 2026 "AI" claims cabinet layout, not schematic layout (vendor claim). | None found | Connection data is kept in a database, not on the page. |
| **SEE Electrical** | Automatic connection of symbols placed from rails; orthogonal wiring of several wires at once (brochure, snippets) | Unknown | Unknown | Unknown | Thin coverage |
| **Festo FluidDraw** | Autoconnect; insertion of symbols without breaking lines; "intelligent line routing" with support points (product page, snippets) | Unknown details | — | — | Manuals did not parse |
| **Altium (schematic)** | Wire modes including **Auto Wire**: a point-to-point router that avoids objects, with a time-out (3 s) and an "avoid cutting wires" weight. Components cut wires. Drag Orthogonal. | Yes, point to point | — | — | — |
| **ProfiCAD, Elecdes** | Orthogonal wires. A symbol dropped on a wire connects into it. Elecdes breaks and heals the line under a moved symbol. | No | — | — | — |
| **Research tools** | netlistsvg (ELK layered, fixed ports); Weave (2026, layered layout plus a net-by-net check of the result) | Yes (layered) | Generated from a netlist | — | Weave 100 % on 117 circuits, 88 % on 3,460; failures in dense multi-pin modules |

**What the tools teach:**

1. **Connectivity is data, and geometry only shows it.** EPLAN generates connections from autoconnect lines. Weave
   re-reads its drawing and compares nets. A router must never change a net, and a net comparison after each route is
   a cheap guard.
2. **The IEC idiom is alignment on a coarse grid plus explicit junctions.** The output vocabulary is straight runs,
   corners and T-junctions with an order, on the grid.
3. **A pin's direction is part of the data.** EPLAN's arrows, AutoCAD's digits and QElectroTech's stubs all use it.
   ElektraWatt already leaves pins along their direction.
4. **People need a way to say "do not join here", and a preview before anything joins.** EPLAN's break point exists
   for this, and KiCad's open issue asks for it.
5. **Where tools route, the cost model is small and visible:** E3's turn, symbol and crossing weights; Altium's time-out
   and crossing weight.
6. **Placement in practice is by templates and slots** (macros, PLC page macros, ladder and PLC generators), not by
   optimisation. A placer that fills page, column and slot keeps cross-references meaningful.
7. **Rubber-banding must keep the person's shape and degrade gracefully.** QElectroTech's profile model is the one
   worth studying.
8. **Text placement is per symbol, plus a few rules.** KiCad's autoplacer is the clearest algorithm. No tool has a
   page-wide text collision check, which is an opening.
9. **Where tools deliberately do not route, it works because the person places pins in line.** The drawing *is* the
   wiring order (EPLAN), and a routed detour would be worse than a straight line the person can achieve by moving the
   symbol. A placer that achieves that alignment is the missing half.

---

## 7. How layout is automated: methods, fit and recommendation

The problem has these features:

- a coarse grid;
- orthogonal lines;
- symbols with pins on fixed sides (mostly top and bottom);
- texts that belong to symbols;
- potentials as rails;
- 30–300 connections per page;
- objects the person placed that must stay.

### 7.1 Routing between placed objects

| Method | What it is | Good for this problem | Bad for it | Fixed objects; incremental |
|---|---|---|---|---|
| **Lee maze (BFS)** (Lee 1961) | Wave expansion over grid cells | Simple, complete, obstacles are blocked cells | No bend cost | Natural; local |
| **A\* or Dijkstra with bend cost** over (cell, heading) states (Hart, Nilsson & Raphael 1968); on a visibility graph (Wybrow, Marriott & Stuckey 2009; "Seeing around corners" 2014) | Shortest path where the state carries the heading, so bends are priced | **The right core for one route.** Every rule becomes a cost or a block; pin direction is a forced first step. ElektraWatt has this. | Routes one net at a time, so the result depends on net order | Obstacles; reroute only nets touched |
| **Line probe** (Hightower 1969; Mikami & Tabuchi 1968) | Probe lines from both ends | Few bends, little memory | Incomplete or not optimal; no gain on a coarse grid | — |
| **Channel routing** (left-edge, Hashimoto & Stevens 1971; dogleg, Deutsch 1976) and switchbox routing (Luk 1985) | Assign nets crossing a strip to tracks | **Ordering and spacing parallel runs**: between a rail block and a row of symbols, along a terminal strip, in a bundle. Deterministic and tidy. Used in netlist-to-schematic tools (N2S 1993). | Needs the page cut into channels; cyclic vertical constraints need doglegs | A fixed run is a pre-assigned track; re-solve one channel |
| **Rip-up and reroute** (Nair 1987) | Remove and reroute nets in conflict | Simple repair | Greedy; can oscillate | Never rip fixed wires |
| **Negotiated congestion** (PathFinder, McMurchie & Ebeling 1995) | Route all nets; raise the price of shared resources (present and history) until none is shared | **The standard answer to net-order dependence**, a few lines over A\* | Iterative; needs an iteration cap; harder to explain (ADR-0033's reason to reject it) | Warm start from history (K) |
| **Rectilinear Steiner trees** (Hanan 1966; Batched 1-Steiner, Kahng & Robins 1992; FLUTE, Chu & Wong 2008) and **trunk-and-branch** (yFiles bus router) | A multi-pin net as one tree | **Rails are trunks.** Each pin needs one drop to its rail. Local multi-pin nets are small: Hanan-grid heuristics or "route to the nearest point of the partial tree" suffice. | FLUTE's licence (attribution clauses); the Rust crate is GPL only | A fixed trunk is given; reconnect only the moved pin's branch |
| **Hyperedge routing** (Sander 2003; Wybrow, Marriott & Stuckey 2012; Eschbach, Günther & Becker 2006 for circuits) | Route a net with several ends as one orthogonal tree, and optimise the junction points | Multi-pin signal nets; crossing heuristics for circuits | Optimal crossing count is NP-hard (Eschbach et al.) | libavoid refines junctions locally |
| **Nudging and bundling** (libavoid; LP nudging, Hegemann & Wolff 2023) | After routing, order segments in a shared channel and spread them | Ideas for even spacing | Free-space nudging gives off-grid coordinates; on a grid it *is* track assignment | Fixed ports as constraints |
| **Object-avoiding orthogonal connector routing: libavoid** (Wybrow et al. 2005–2014) | Incremental routing on an orthogonal visibility graph, with ports, nudging, hyperedges and transactions that reroute only affected connectors | **The reference design for interactive routing.** Its incremental transaction model is worth copying. | Gridless (needs snapping); C++ and LGPL; crossing penalties experimental; reported slow at 300 nodes in its 2009 form (EditLens 2016) | Transactions; progress callback |
| **Crossing reduction after routing** | Crossing cost plus rerouting the nets with most crossings (yFiles); removal of repeated crossings (Hegemann & Wolff) | Cheap improvement pass | Local | — |

### 7.2 Placement

| Method | Input it needs | Good for this problem | Bad | Fixed objects |
|---|---|---|---|---|
| **Layered drawing** (Sugiyama, Tagawa & Toda 1981; Gansner et al. 1993; median heuristic, Eades & Wormald 1994; coordinates by Brandes & Köpf 2001) | Directed graph; layer assignment; order within layers | **Fits the current path:** layers are heights between the rails (command, interlock, coil); crossing reduction orders the columns; coordinates align each path on one axis | Feedback (self-holding contacts) makes cycles; rails are not ordinary nodes; pure layering spreads wide | Only through "interactive" strategies |
| **ELK Layered with port constraints** (Schulze, Spönemann & von Hanxleden 2014) | As above, plus each port's side, order or position (FREE … FIXED_POS) | Pins on fixed sides (FIXED_POS); orthogonal routing between layers; mature phases; model-order and interactive strategies keep the person's order | Routes only between adjacent layers (not an obstacle router); "interactive" keeps order approximately, not pinned objects; Java, EPL-2.0 or GPL-3.0+ | Approximate |
| **Topology–shape–metrics** (Tamassia 1987; OGDF) | Planar embedding; min-cost-flow bend minimisation; compaction | Minimum bends for a fixed embedding | Re-embeds the whole graph; knows no rails, no flow, no fixed pin positions; unstable under edits | Poor |
| **Constraint-based layout** (VPSC/IPSep-CoLa, Dwyer, Koren & Marriott 2006; stress majorization, Gansner, Koren & North 2004; ACA, Kieffer et al. 2013; CoDaFlow with ports, Rüegg et al. 2014) | Positions, separation and alignment constraints | **"Tidy" and compaction around fixed objects:** a pinned object is an equality constraint; alignment and minimum gaps come out exactly; incremental | Does not create the current-path structure by itself | Hard constraints |
| **Force-directed** | Graph | — | Not orthogonal; unstable | — |
| **HOLA** (Kieffer, Dwyer, Marriott & Wybrow 2016) | Graph | Human-like general networks | Not port- or rail-aware; 2–4 times the area of newer pipelines; slow | — |
| **Netlist-to-schematic generation**: classic (Arya et al. 1985; Swinkels & Hafer 1990; Parng et al. 1991, ASG; Frezza & Levitan 1993, SPAR; Naveen & Raghunathan 1993, N2S) and modern (netlistsvg on ELK; Weave 2026) | Netlist, symbol shapes | Partition, place by signal flow, then route; Weave adds a net check of the result | Digital logic oriented; nothing published for ladder, IEC current paths or fluid diagrams (practice is template-driven) | Mostly none (one paper on incremental generation with undo) |
| **ML/LLM generation** (Schemato 2024; EEschematic 2025; SchGen 2026; an RL patent) | Training data | — | Correctness below deterministic layout (Weave against Schemato); not explainable | — |
| **Templates and slots** (EPLAN page macros, AutoCAD ladder and PLC generators, Circuit Builder) | A rule per page kind | **What practice does.** Predictable, and keeps cross-references stable | Needs a template per kind of page; poor for mixed pages | Fixed objects fill their slot |

### 7.3 Label placement

- **Point-feature label placement.**
  - It is NP-hard in the four-position model (Formann & Wagner 1991).
  - Simulated annealing gave the best quality for its time among greedy, gradient descent and annealing (Christensen,
    Marks & Shieber 1995).
  - Three simplification rules on the candidate conflict graph solve most instances (Wagner, Wolff, Kapoor & Strijk
    2001).
- **Edge labels.** Kakoulis & Tollis (2003) unify node and edge labels with Imhof's rules. ELK reserves label space as
  dummy nodes in the layered layout (Schulze et al. 2018).
- **Practice.**
  - KiCad's field autoplacer: rank sides, avoid pins and collisions, fit between wires, snap to the grid.
  - A silkscreen placer: candidate positions, the smallest displacement wins, shrink text as a last resort.
- **For this problem:**
  - A tag has 3–5 conventional places (left, right, above, below; D-17).
  - Terminal numbers have two (either side of the wire).
  - Contact images have one (under the coil).
  - A greedy pass by priority, with conflict tests against an R-tree and a local repair (swap, or a short annealing)
    for the few that fail, is enough.
  - Labels the person placed are fixed obstacles.

### 7.4 Libraries a Rust program could use or learn from

| Library | Language | Licence | Use |
|---|---|---|---|
| petgraph | Rust | MIT OR Apache-2.0 | Graph structures; usable in any crate |
| pathfinding | Rust | MIT/Apache-2.0 | A\*, Dijkstra, BFS; base for a (cell, heading) search (ElektraWatt has its own) |
| rstar | Rust | MIT OR Apache-2.0 | R\*-tree for obstacle, label-conflict and hit queries |
| rust-sugiyama | Rust | MIT | Layering, median/barycenter, Brandes–Köpf; no ports, no routing. A reference or base for column ordering |
| layout-rs | Rust | MIT | DOT layout; no ports documented |
| msagl-rust | Rust | MIT | Port of MSAGL's rectilinear router; immature; not on crates.io |
| libavoid-rust | Rust | Conflicting (Apache file, LGPL readme); a port of LGPL code | Treat as LGPL; experimental |
| steiner-tree (LibrEDA) | Rust | GPL-3.0-or-later | FLUTE-style Steiner trees; GPL crates only |
| Adaptagrams (libavoid, libcola, libvpsc, libdialect/HOLA) | C++ | LGPL-2.1-or-later (+ commercial) | Reference design; FFI from GPL code only |
| ELK / elkjs | Java / JS | EPL-2.0 OR GPL-3.0-or-later | Read and port into GPL-3.0-or-later code only; ports, interactive strategies, label handling |
| OGDF | C++ | GPL-2/3 with exceptions | Read; poor fit |
| Graphviz | C | EPL-2.0 | Ortho mode ignores ports; not suitable |
| dagre, d3-dag, WebCola | JS/TS | MIT | Read or port (layered; constraint layout) |
| yFiles, GoJS | — | Commercial | Feature benchmarks only (bus router, scope, crossing cost) |
| KiCad autoplace_fields | C++ | GPL-3.0-or-later (K) | Read; label heuristic |
| FLUTE | C / Java | Attribution-assurance terms on the Java port; C not verified | Avoid |

The licence boundary matters. Format crates are MIT OR Apache-2.0, and the router lives in `ew-derive`. The designer
must check that crate's licence first. If it is permissive, no LGPL, EPL or GPL code (and no port of it) can go in.

### 7.5 Recommendation: what fits and why

Fits:

1. **Keep the exact grid search as the core of a single route.** Dijkstra or A\* over (cell, heading) with integer
   costs and the tie rule. It already holds the hard rules and determinism, and nothing in the literature beats it on a
   coarse grid.
2. **Add structure before search.**
   - A page model of rails, columns (paths) and slots, derived from the frame and from the potentials.
   - Rails as trunks: every pin of a rail potential gets a straight drop with a T on its rail (Steiner trunk-and-branch,
     reduced to drops by the column structure).
   - Wires within a column: straight segments.
   - Only what is left goes to the general search.

   This removes most of the order problem that ADR-0033 meets with rip-up rounds.
3. **Use channel track assignment (left-edge with doglegs) for bundles.** Applies to horizontal runs between a block of
   rails and a row of symbols, along terminal strips, and to parallel tubes. It gives the spacing and nesting rules
   (D-6, R-10) directly and deterministically, and on the grid it replaces nudging.
4. **Solve labels jointly with routes, in a short alternation.**
   - Place labels by candidates (D-17, D-18, R-11).
   - Route with the chosen label boxes as soft costs.
   - Move a label a wire would cross to its next free candidate, then reroute only the wires the move frees or blocks.
   - Cap the alternation at two or three rounds.

   This replaces "texts block, the scene moves tags afterwards", which produces the loops round captions that
   amendments 2 and 5 describe.
5. **Placement by slots, ordered by a layered method.**
   - For a page of known kind (power, control, PLC, fluid), the placer assigns each device function to a page, a column
     and a height band by the rules of section 5 (template).
   - It orders columns and contacts by a layered crossing-reduction pass (median/barycenter) over the netlist, with pins
     on fixed sides.
   - Objects the person placed are fixed slots.
   - A constraint pass (separation and alignment, VPSC-style) compacts and aligns what is left, never moving a fixed
     object.
6. **One quality function.**
   - It computes the measures of the rules table.
   - It serves as the acceptance test of every automatic change (never worse on hard rules, better on the weighted
     sum: amendment 5's rule, extended to placement) and as the test oracle.
   - ElektraWatt's `measures.rs` is its seed. It lacks flow (D-9, R-6), alignment (D-12, R-7), label ambiguity and
     overlap (R-11), detour ratio (R-3) and bundle order (R-10).
7. **Negotiated congestion only if the structured approach still leaves order effects.** With a fixed iteration count
   and fixed order it stays deterministic. Its explanation can be "the router let wires bid for crowded tracks", shown
   per wire in the preview.

Does not fit:

- topology–shape–metrics;
- HOLA;
- force-directed layout;
- gridless libavoid as the core (its incremental design is worth copying, its code is not);
- ML or LLM generation.

In each case the reasons are those given in the tables: no rails, no fixed pin sides, no stability, a licence, or no
correctness.

The constraints already set for this design (in its brief, not in the literature) also fit this approach:

- the placer acts only on its command;
- a part placed by hand stays where it is put (only the ordinary grid snap, and light alignment guides);
- the Wire tool and connecting on placement remain.

A slot-based placer that runs on command and fills free slots around fixed objects meets them. Snap-dragging (Bier &
Stone 1986) and persistent guides (StickyLines, Ciolfi Felice et al. 2016, 40 % faster than align commands in their
study) are the documented forms of light guides.

---

## 8. Open questions for the designer

1. **Page kinds.** Is the kind of a page (power, control, PLC in, PLC out, bus, fluid, mixed) declared by the person,
   derived from its content, or both? Templates need it. A mixed page needs a fallback (layered plus constraints).
2. **Rails as objects.**
   - Does the model gain a rail (a potential drawn as a trunk with drops), or are rails a router convention over
     ordinary connections?
   - FR-CON-001 (two endpoints per connection) and the junction model (doc 06 §3.5) decide how a drop's T is stored.
3. **Columns.** Are current paths a property of the frame (EPLAN), of the page, or computed? How many per page by
   default (10 on A3 is common, but not verified as a norm)?
4. **Flow direction per page.** IEC top-to-bottom, ladder left-to-right, fluid bottom-to-top: one setting, or one per
   page kind?
5. **Spacings as settings.** Rail pitch (1 or 2 steps), symbol gap in a path (2–3 steps), the band under the M rail for
   contact images, the channel pitch, the pitch of fluid chains: which are project settings, which page settings,
   which fixed?
6. **Labels.** Which texts the placer may move:
   - tags;
   - terminal numbers;
   - function texts;
   - cross-references;
   - captions of black boxes, part texts.

   Which are fixed (moved by the person)? What is the candidate order per text kind, and is the side of a tag a page
   rule (D-17)?
7. **Interruptions** (D-5). May the router replace a long route with a pair of interruption points? If so, at which
   threshold (length, bends, crossings), and is that a preview the person accepts?
8. **Junction order** (D-8). Does the router choose a junction's star or chain order, and its T's direction, or keep
   the stored sequence?
9. **Quality weights.** The weights of the measures in one function. Today: step 2, bend 8, crossing 12, text 20. What
   is the order of priority between crossings, bends, detours, label distance and flow? The literature ranks crossings
   first; the trade adds alignment.
10. **Stability.** What may an automatic run move?
    - Everything not fixed?
    - Only what is unrouted or breaks a hard rule?
    - Only within the selection?

    How is "fixed" marked: by having been placed by hand, by a pin, or by the selection?
11. **Fluid.** Are tubes routed by the same router with fluid costs (the sketch's vertical working lines and horizontal
    supply line), and does the placer know chains and valve terminals?
12. **Determinism and preview.** Every run gives the same result for the same page and is shown before it is applied
    (ADR-0033 decisions 1 and 6). Does a placement run preview as one step, or as placement and routing separately?
13. **Licence of the crate** that will hold the placer (section 7.4).
14. **Measure of success.** Which pages are the test set (the page kinds of section 5, plus imported pages), and which
    measures must improve against today's router before the new one replaces it?

---

## 9. What could not be found out, or only second-hand

- **The rule texts of IEC 61082-1:2014, clauses 5 to 7.**
  - The VDE preview ends after clause 3, and the iTeh sample is encrypted.
  - The rules in 2.1 and in the table rest on the contents list (P: they exist), on German summaries (ibKastl wiki,
    textbook samples) and on tool manuals.
  - **Not verified:** the side on which a reference designation stands (Fig. 45), the content of Fig. 37 (line
    spacing), the line widths and text heights of 5.10 and 5.11.
- **ISO 1219-2:2012 clause 4.3 (layout).** Only its existence and page range were read. The fluid arrangement rests on
  vendor and teaching summaries (RS, Misumi) and on Festo teaching practice that was not re-read (K).
- **ISO 128-20 line-width values.** These are from memory (K). The ISO 3098 lettering heights are second-hand (S).
- **ISO 5457 and ISO 7200:** the title-block position and width (see `standards.md` §12).
- **Numbers of practice:** rail pitch, gap between symbols in a path, paths per page as a norm, PLC channel pitch.
  - EPLAN's help gives 10 paths and 8 PLC channels per page only as *examples*.
  - EPLAN's user grids A–E have no documented defaults ("grid C = 4 mm" is not verified).
- **Tools whose manuals did not parse or were blocked:** SEE Electrical, Festo FluidDraw, SOLIDWORKS Electrical (help
  renders by script), the AutoCAD Electrical and QElectroTech forums.
- **No readability study of circuit diagrams themselves.** The evidence in section 3 is from general graphs, UML and
  data-flow diagrams.
- **No published algorithm for IEC current-path, ladder or fluid-diagram layout.** Practice is template-driven, and
  the papers found are for digital logic schematics.
- **Licences of a few libraries** were read from repository files, not from legal review (marked in section
  7.4). The FLUTE C licence is not verified.

---

## 10. Sources

All read 2026-11-19 unless marked "snippet" (search summary only). Papers give authors, title and year.

### 10.1 Standards and their previews, summaries

- https://www.vde-verlag.de/iec-normen/preview-pdf/info_iec61082-1{ed3.0}b.pdf (served from assets.vde-verlag.de):
  IEC 61082-1:2014 preview. Contents, figure list, scope, terms (**P**). The basis of 2.1.
- https://cdn.standards.iteh.ai/samples/19897/2d9fca205e194863a04013d515c0e149/IEC-61082-1-2014.pdf: iTeh sample of
  the same. Encrypted, not readable.
- https://standards.iteh.ai/catalog/standards/iec/129a81b3-4c7b-4bd9-92c1-2e90fd482ca2/iec-61082-1-2014: catalogue
  abstract (snippet). The third edition adds exceptions to the placement of reference designations.
- https://wiki.ibkastl.de/view/Normen_f%C3%BCr_die_Schaltplanerstellung: German summary of the standards for circuit
  diagrams (**S**). Text orientation, lines not crossing symbols, minimise bends and crossings, interruptions; IEC
  60204-1 conductor identification; ISO 1219-2 code parts and media letters; colours.
- https://ww3.cad.de/foren/ubb/uploads/Stelvio/Schaltplanrelevante_Normen.pdf: print of an earlier version of a
  similar wiki page (mkdoc). IEC 81346 classes.
- https://de.wikipedia.org/wiki/Stromlaufplan: forms of presentation; rails along the top and bottom; numbered vertical
  paths; crossing-free drawing and PE; de-energised state (**S**).
- https://content.voltimum.com/de/127002/221532_seiten_254-257_schaltplaene.pdf: textbook pages on kinds of circuit
  diagram, with examples of rails L1 L2 L3 N PE and numbered paths (**S**).
- https://www.europa-lehrmittel.de/leseprobe/35717-8.pdf: textbook sample (Technische Kommunikation Elektrotechnik).
  Front matter only was relevant.
- https://www.din.de/de/mitwirken/normenausschuesse/nam/veroeffentlichungen/wdc-beuth:din21:165434690/toc-1919947/download:
  ISO 1219-2:2012 contents (**P**).
- https://www.din.de/de/mitwirken/normenausschuesse/nam/veroeffentlichungen/wdc-beuth:din21:297840821/toc-3015820/download:
  DIN ISO 1219-2:2019 contents with the code structure (**P**).
- https://webstore.ansi.org/preview-pages/ISO/preview_ISO+1219-2-2012.pdf: refused (403).
- https://ch.rs-online.com/web/content/discovery-portal/produktratgeber/pneumatik-schaltplan: pneumatic diagram, energy
  flow bottom to top, ISO 1219-2 designations (**S**).
- https://es.misumi-ec.com/en/techblog/general-info/pneumatic-circuits-diagrams-fluid-engineering-fundamentals/:
  bottom-to-top arrangement (snippet; the page refused the fetch).
- https://www.slideshare.net/slideshow/basic-pneumatic-circuit/41282465: actuators on top in order of operation
  (snippet, weak).
- https://www.festo.com/pdf_flip/didactic/Pneumatics_and_Hydraulics/en/214/: Festo Didactic catalogue (snippet; TP101
  contents names "circuit layout").
- https://www.wevolver.com/article/how-to-design-a-pneumatic-circuit: generic design advice. Little on layout.
- Search summaries of DIN EN 60617-2:1996 (module M = 2.5 mm, 2M line spacing) and IEC TS 63064:2018 Annex A:
  https://standards.globalspec.com/std/597240/din-en-60617-2 ,
  https://standards.iteh.ai/catalog/standards/iec/0c2778a8-4900-4fc6-982f-015bb667080d/iec-ts-63064-2018 (snippets).
- https://cdn.standards.iteh.ai/samples/23412/4e30edb3f1bc4568a23b28d969824421/IEC-TS-63064-2018.pdf and
  https://cdn.standards.iteh.ai/samples/12806/d238628bfbf04cdab4d2325ed86f149c/IEC-61082-1-2006.pdf: encrypted
  samples, not readable.
- https://cadsetterout.com/drawing-standards/grid-reference-frame/: ISO 5457 grid fields, letters, offsets (**S**).
- https://cadsetterout.com/drawing-standards/lettering-heights/ and https://blog.draftsperson.net/text-heights-in-drawings/:
  ISO 3098 heights and line widths per height (snippets).
- https://en.wikipedia.org/wiki/ISO_128: list of parts. No values.
- https://www.red-bag.com/imageslib/BN-DS-E2_02.pdf: a company electrical design standard derived from IEC 617.
  Upright inscriptions, contact operation direction, de-energised state (**S**).
- https://myelectrical.com/opinion/entryid/24/IEC-Reference-Designations: shortened designations from context (**S**).
- https://www.ecmweb.com/basics/article/20897064/electrical-ladder-drawing-basics: ladder conventions (**S**).
- https://opentextbc.ca/basicmotorcontrol/chapter/numbering-system/: refused (403).
- https://industrialmonitordirect.com/blogs/knowledgebase/iec-plc-io-module-drawing-vertical-vs-horizontal-representation:
  two ways of drawing PLC modules (**S**, low authority).
- https://bsl.schneider-electric.com/eDocuments/657/530/825657530/html/-/-A/-3/D-ADMIN-0000000740.1.html: a protection
  relay manual's flow convention (snippet).
- https://library.iec.ch/grasymb/grasymb.nsf/(SysANEN)/A00003: IEC 60617 application note on junctions (snippet).

### 10.2 Readability, interaction

- H. C. Purchase, "Which aesthetic has the greatest effect on human understanding?", GD 1997:
  https://link.springer.com/doi/10.1007/3-540-63938-1_67
- H. C. Purchase, "Metrics for graph drawing aesthetics", JVLC 2002:
  https://research.monash.edu/en/publications/metrics-for-graph-drawing-aesthetics/
- H. C. Purchase, D. Carrington, J.-A. Allder, "Empirical evaluation of aesthetics-based graph layout", 2002:
  https://eprints.gla.ac.uk/35821 . Also "Graph layout aesthetics in UML diagrams: user preferences", JGAA 2002:
  https://www.jgaa.info/index.php/jgaa/article/view/paper54
- C. Ware, H. Purchase, L. Colpoys, M. McGill, "Cognitive measurements of graph aesthetics", 2002:
  https://research.monash.edu/en/publications/cognitive-measurements-of-graph-aesthetics/
- W. Huang, P. Eades, S.-H. Hong, "Larger crossing angles make graphs easier to read", JVLC 2014:
  https://unpaywall.org/10.1016%2Fj.jvlc.2014.03.001
- R. Tamassia, G. Di Battista, C. Batini, "Automatic graph drawing and readability of diagrams", IEEE TSMC 1988:
  https://scilit.net/publications/24fe64a57fa5d3a7decb84112d2e94b4
- C. Bennett, J. Ryall, L. Spalteholz, A. Gooch, "The aesthetics of graph visualization", 2007:
  https://diglib.eg.org/handle/10.2312/COMPAESTH.COMPAESTH07.057-064 (bibliographic record only)
- K. Misue, P. Eades, W. Lai, K. Sugiyama, "Layout adjustment and the mental map", 1995:
  https://typeset.io/papers/layout-adjustment-and-the-mental-map-36y8o59qtx
- S. North, "Incremental layout in DynaDAG", GD 1995: https://researchr.org/publication/North95
- E. A. Bier, M. C. Stone, "Snap-dragging", SIGGRAPH 1986: https://history.siggraph.org/?p=116043
- M. Ciolfi Felice, N. Maudet, W. E. Mackay, M. Beaudouin-Lafon, "Beyond snapping: persistent, tweakable alignment and
  distribution with StickyLines", UIST 2016: https://hal.archives-ouvertes.fr/hal-01410171

### 10.3 Tools (from the tools survey; plain = read, snippet = summary only)

**EPLAN**

- Autoconnect, preview, source and target:
  https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/connectionbrowsergui_k_prinzipverbindungen.htm
- Glossary entry for autoconnect:
  https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/Glossary_o_autoconnecting.htm
- Graphical and logical connection symbols:
  https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/egedgui_k_start.htm
- Target tracking: https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/egedgui_k_grundlagen.htm
- T-node and double junction:
  - https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/egedgui_k_darstellungverbindungspunkte.htm
  - https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/egedgui_d_tstueck.htm
  - https://www.eplan.help/en-us/infoportal/content/Plattform/2022/Content/htm/egedgui_d_kreuzung.htm
- Break point: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/egedgui_h_verbindungenunterbrechen.htm
- Connection direction: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/Glossary_o_anschlussrichtungen.htm
- Connection definition points: https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/connectionbrowsergui_k_vdp.htm
- Smart connect: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/connectionbrowsergui_h_smartconnecting.htm
- PLC:
  - Page generation: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/plcgui_k_makrosschaltplanerzeugen.htm
  - Channel-oriented work: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/plcgui_k_kanalorientiertarbeitsweise.htm
  - Card overviews: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/Glossary_o_sps_kartenuebersichten.htm
  - Channels: https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/plcgui_k_kanaele.htm
- Pre-planning: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/planninggui_k_prinzip.htm
- Cogineer: https://eplan.help/help/cogineercloud/en-us/Content/htm/cogineer_d_basics.htm
- Property arrangements: https://eplan.help/en-us/infoportal/content/Plattform/2023/Content/htm/devicetaggui_h_eigschanordnungen.htm
- Contact image: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xessettingsgui_h_einstellungenkontaktspiegel.htm
- Cross-reference shortening: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/xessettingsgui_h_qvwanzeigeoptimieren.htm
- Grid per page type, path numbering: https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/pagebrowsergui_d_projekteinstellungen.htm
- Fluid connection symbols: https://eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/ftechnic_k_verbindungssymbole.htm
- Reviews: https://www.capterra.com/reviews/152595/Electric-P8 (snippet)

**AutoCAD Electrical**

- Wire basics: https://help.autodesk.com/cloudhelp/2025/ENU/AutoCAD-Electrical/files/GUID-A5EAD418-0275-476C-AEE0-7B1918D3ADC8.htm
- Insert Wire: https://help.autodesk.com/cloudhelp/2016/ENU/AutoCAD-Electrical/files/GUID-936BB8A9-9C01-4AC6-B833-CB97700A7203.htm
- Pin direction attributes: https://help.autodesk.com/cloudhelp/2027/ENU/AutoCAD-Electrical/files/GUID-A42DA038-B1AB-4401-BD71-BC7A683D8DF7.htm
- Drawing format, trap distance: https://help.autodesk.com/cloudhelp/2016/ENU/AutoCAD-Electrical/files/GUID-F7311B74-094E-41F0-800E-69FE83D157C9.htm
- Ladders:
  - https://help.autodesk.com/cloudhelp/2016/ENU/AutoCAD-Electrical/files/GUID-9073D451-43CE-4745-B07B-1783CF22426F.htm
  - https://help.autodesk.com/cloudhelp/2025/ENU/AutoCAD-Electrical/files/GUID-4797FDE4-99EF-4E28-AC64-DA58390BA7B6.htm
- Circuit Builder: https://help.autodesk.com/cloudhelp/2015/ENU/AutoCAD-Electrical/files/GUID-41B776E6-CAAE-4F46-8010-B6A13D309D0C.htm
- PLC I/O from a spreadsheet: https://help.autodesk.com/cloudhelp/2025/ENU/AutoCAD-Electrical/files/GUID-23A0C8BA-7EA6-41A0-AB34-DD7CA3D972CC.htm
- Forums (snippets):
  - https://forums.autodesk.com/t5/autocad-electrical-forum/erratic-wire-routing-behavior/m-p/9268605
  - https://forums.autodesk.com/t5/autocad-electrical-forum/maintain-wire-connections-when-moving-components/m-p/7833892

**SOLIDWORKS Electrical**

- Move against Stretch: https://hawkridgesys.com/blog/solidworks-electrical-stretch-vs-move-commands
- Multiple wire, symbol on a wire (snippets):
  - https://help.solidworks.com/2023/English/swelec/t_swelec_WireDrawMultiple.htm
  - https://help.solidworks.com/2023/english/swelec/r_swelec_EWINSBPTUNKNOWN.htm

**E3.series**

- Auto Route cases and costs: https://www.zuken.com/en/set-up-autorouting-e3series
- Wiring diagram generator: https://www.zuken.com/en/product/e3series/schematic-wiring-diagram-generator/

**QElectroTech**

- Autoconnect: https://download.tuxfamily.org/qet/manual_0.7/build/_sources/schema/conductor/conductor_creation.rst.txt
- Conductor handles: https://download.qelectrotech.org/qet/manual_0.7/build/schema/conductor/conductor_modify.html
- Default path and profiles, from the source:
  https://raw.githubusercontent.com/qelectrotech/qelectrotech-source-mirror/master/sources/qetgraphicsitem/conductor.cpp
- Conductor API: https://download.qelectrotech.org/qet/doxygen/html/da/d7f/class_conductor.html

**KiCad**

- Wiring, Move and Drag: https://docs.kicad.org/9.0/en/eeschema/eeschema.html
- Field autoplacer: https://docs.kicad.org/doxygen/autoplace__fields_8cpp_source.html
- Rubber-band debate: https://forum.kicad.info/t/why-does-eeschema-ever-do-anything-other-than-rubberband-wires/21056
- Accidental joins: https://gitlab.com/kicad/code/kicad/-/issues/16834
- Autoplace overlap: https://gitlab.com/kicad/code/kicad/-/issues/11529

**WSCAD**

- Automatic connection: https://docu.wscad.com/HELP/English/WSCAD54/AT/07_Schematic/07-01Connection_line/Line.htm
- Destination wiring: https://docu.wscad.com/HELP/English/WSCAD54/AT/07_Schematic/07-10Zielverdrahtung/Working_with_Destination_wiring.htm
- Macros, project wizard: https://www.wscad.com/us/?p=147419

**Other tools**

- SEE Electrical: https://d2uars7xkdmztq.cloudfront.net/app_resources/67854/documentation/398667_1695366242941_en-US.pdf (snippet)
- FluidDraw: https://www.festo.com/ph/en/app/fluiddraw.html (snippet)
- Altium:
  - https://altium.com/documentation/altium-designer/sch-dlg-autowirer-optionsformpoint-to-point-router-options-ad?version=21
  - https://altium.com/documentation/altium-nexus/schematic-wire (snippet)
- ProfiCAD: https://www.proficad.com/help/el/how-to-draw-electrical-wires.htm
- Elecdes: https://help.elecdes.com/ELECDES_CUST/auto_breakheal.htm

### 10.4 Methods and libraries (from the methods survey)

**Routing**

- C. Y. Lee, "An algorithm for path connections and its applications", 1961: https://en.wikipedia.org/wiki/Lee_algorithm
- P. Hart, N. Nilsson, B. Raphael, "A formal basis for the heuristic determination of minimum cost paths", 1968:
  https://www.wikipedia.org/wiki/A*_algorithm
- D. W. Hightower, "A solution to line-routing problems on the continuous plane", 1969:
  https://bibtex.github.io/DAC-1969-Hightower.html
- K. Mikami, K. Tabuchi, 1968: https://www.scilit.net/publications/1ee51f638f14d0c4e813d852c71594ba
- Channel routing (left-edge, dogleg): https://www.eecs.northwestern.edu/~haizhou/357/lec7.pdf ;
  https://typeset.io/papers/a-dogleg-channel-router-43kw8gyppt
- Switchbox routing: https://api.crossref.org/works/10.1016%2F0167-9260%2885%2990029-X ;
  https://dblp.org/rec/journals/tcad/CohoonH88.html
- R. Nair, "A simple yet effective technique for global wiring", 1987:
  https://researcher.watson.ibm.com/publications/a-simple-yet-effective-technique-for-global-wiring
- L. McMurchie, C. Ebeling, "PathFinder: a negotiation-based performance-driven router for FPGAs", 1995:
  https://dblp.org/rec/conf/fpga/McMurchieE95.html ;
  https://www.esa.informatik.tu-darmstadt.de/archive/twiki/pub/Lectures/AlgorithmenImChipEntwurfDe/router-pathfinder.pdf
- A. Kahng, G. Robins, iterated 1-Steiner, 1992:
  https://www.cs.virginia.edu/~robins/papers/A_New_Class_of_Steiner_Tree_Heuristics_with_Good_Performance_the_Iterated_1_Steiner_Approach.pdf
- C. Chu, Y.-C. Wong, "FLUTE", 2008: https://home.engineering.iastate.edu/cnchu/pubs/j29.pdf ;
  licence of the Java port: https://home.engineering.iastate.edu/cnchu/Flute.java
- yFiles bus router: https://yed.yworks.com/support/manual/layout/router_orthogonal_bus.html
- yFiles orthogonal router: https://yed.yworks.com/support/manual/layout/router_orthogonal.html
- G. Sander, "Layout of directed hypergraphs with orthogonal hyperedges", 2003: https://researchr.org/publication/Sander03
- T. Eschbach, W. Günther, B. Becker, "Orthogonal hypergraph drawing for improved visibility", 2006:
  https://jgaa.info/index.php/jgaa/article/view/paper122
- M. Wybrow, K. Marriott, P. Stuckey, "Orthogonal connector routing", GD 2009:
  https://researchr.org/publication/WybrowMS09
- The same authors, "Orthogonal hyperedge routing", 2012: https://unpaywall.org/10.1007%2F978-3-642-31223-6_10
- K. Marriott, P. Stuckey, M. Wybrow, "Seeing around corners", 2014: https://link.springer.com/10.1007/978-3-662-44043-8_4
- libavoid documentation:
  - https://www.adaptagrams.org/documentation/libavoid.html
  - https://www.adaptagrams.org/documentation/namespaceAvoid.html
  - https://www.adaptagrams.org/documentation/classAvoid_1_1Router.html
  - https://raw.githubusercontent.com/mjwybrow/adaptagrams/master/README.md
- S. Gladisch et al., "Orthogonal edge routing for the EditLens", 2016: https://ar5iv.labs.arxiv.org/html/1612.05064
- T. Hegemann, A. Wolff, "A simple pipeline for orthogonal graph drawing", GD 2023: https://arxiv.org/abs/2309.01671
- JointJS router-avoid: https://docs.jointjs.com/learn/features/automatic-layouts/standalone-link-routing

**Placement**

- K. Sugiyama, S. Tagawa, M. Toda, 1981: https://researchr.org/publication/SugiyamaTT81/bibtex
- E. Gansner et al., "A technique for drawing directed graphs", 1993: https://patents.google.com/patent/US4953106A
- P. Eades, N. Wormald, "Edge crossings in drawings of bipartite graphs", 1994: https://link.springer.com/doi/10.1007/BF01187020
- U. Brandes, B. Köpf, 2001: https://rd.springer.com/content/pdf/10.1007/3-540-45848-4_3.pdf ; erratum 2020:
  https://arxiv.org/abs/2008.01252
- C. D. Schulze, M. Spönemann, R. von Hanxleden, "Drawing layered graphs with port constraints", 2014:
  https://unpaywall.org/10.1016%2Fj.jvlc.2013.11.005 ; https://rtsys.informatik.uni-kiel.de/~biblio/downloads/papers/jvlc13.pdf
- ELK:
  - https://eclipse.dev/elk/reference/options/org-eclipse-elk-portConstraints.html
  - https://eclipse.dev/elk/reference/algorithms/org-eclipse-elk-layered.html
  - https://eclipse.dev/elk/blog/posts/2025/25-08-21-layered.html
  - https://eclipse.dev/elk/blog/posts/2023/23-01-09-constraining-the-model.html
  - https://projects.eclipse.org/projects/modeling.elk/releases/0.12.0/review
  - https://cdn.jsdelivr.net/npm/elkjs@0.12.0/package.json
- R. Tamassia, "On embedding a graph in the grid with the minimum number of bends", 1987:
  https://scilit.com/publications/6d66dc5fb99d80575f0131b9348edaf8
- T. Dwyer, Y. Koren, K. Marriott, "IPSep-CoLa", 2006:
  https://www.bohrium.com/paper-details/ipsep-cola-an-incremental-procedure-for-separation-constraint-layout-of-graphs/812038880860569601-2445
- E. Gansner, Y. Koren, S. North, "Graph drawing by stress majorization", 2004: https://researchr.org/publication/GansnerKN04
- S. Kieffer et al., "Incremental grid-like layout using soft and hard constraints", 2013: https://ar5iv.labs.arxiv.org/html/1308.6368
- U. Rüegg et al., "Stress-minimizing orthogonal layout of data flow diagrams with ports", 2014: https://ar5iv.arxiv.org/html/1408.4626
- S. Kieffer, T. Dwyer, K. Marriott, M. Wybrow, "HOLA: human-like orthogonal network layout", 2016:
  https://cs.reviewer.ly/app/publication/83a58a3f-fac3-41b2-83f1-357c25c870cf
- Netlist-to-schematic generation:
  - Arya et al., 1985: https://researchr.org/publication/AryaKSM85
  - SPAR (Frezza & Levitan), 1993: https://researchr.org/publication/FrezzaL93/bibtex
  - N2S (Naveen & Raghunathan), 1993: https://manuscript.isc.ac/Inventory/49/866398.htm
  - ASG (Parng et al.), 1991: https://katalog.lib.cas.cz/KNAV/EdsRecord/edsair,edsair.doi...........f46cd4372625d65f0221a2cca43aa33c
  - Swinkels & Hafer, 1990, cited via the N2S record
  - Korean signal-flow placement paper, 1989: https://koreascience.kr/article/JAKO198911920697200.page?lang=en
  - Incremental generation with undo: https://www.researchgate.net/publication/251946837_Efficient_undo-redo_with_incremental_netlist_to_schematic_generation
- Modern generation:
  - netlistsvg: https://github.com/nturley/netlistsvg
  - Weave, 2026: https://arxiv.org/abs/2607.03835
  - Schemato: https://arxiv.org/abs/2411.13899v2
  - EEschematic: https://arxiv.org/pdf/2510.17002
  - SchGen: https://arxiv.org/html/2605.30345v1
- Simulink layout with port constraints (Klauske et al. 2012): https://link.springer.com/doi/10.1007/978-3-642-31223-6_11

**Labels**

- J. Christensen, J. Marks, S. Shieber, "An empirical study of algorithms for point-feature label placement", 1995:
  https://dash.harvard.edu/bitstream/1/2032678/2/AnEmpiricalStudy.pdf
- M. Formann, F. Wagner, "A packing problem with applications to lettering of maps", 1991:
  https://i11www.iti.kit.edu/map-labeling/points/one-square/info/problem.html
- F. Wagner, A. Wolff, V. Kapoor, T. Strijk, "Three rules suffice for good label placement", 2001:
  https://research.tue.nl/en/publications/three-rules-suffice-for-good-label-placement/
- E. Imhof, "Positioning names on maps", 1975, cited via https://wikipedia.com/wiki/Automatic_label_placement
- K. Kakoulis, I. Tollis, "A unified approach to automatic label placement", 2003:
  https://www.scilit.net/publications/c4b134d20753a369b7bf7bf9a8ff709e
- C. D. Schulze, N. Wechselberg, R. von Hanxleden, "Edge label placement in layered graph drawing", 2018:
  https://link.springer.com/chapter/10.1007/978-3-319-91376-6_10
- ELK node label placement: https://eclipse.dev/elk/reference/options/org-eclipse-elk-nodeLabels-placement.html
- Silkscreen auto placer: https://github.com/coffeenmusic/Silkscreen_Auto_Placer

**Libraries and licences**

- Adaptagrams: https://www.adaptagrams.org/
- libavoid-rust: https://github.com/boxabirds/libavoid-rust
- msagl-rust: https://github.com/SignalCanvas/msagl-rust
- OGDF licence: https://raw.githubusercontent.com/ogdf/ogdf/master/LICENSE.txt
- Graphviz licence: https://graphviz.org/license/
- Graphviz ortho and ports: https://forum.graphviz.org/t/ortho-edges-and-ports-a-partial-work-around/1937
- dagre: https://github.com/dagrejs/dagre
- d3-dag: https://github.com/erikbrinkman/d3-dag
- WebCola: https://github.com/tgdwyer/WebCola
- crates.io records (versions and licences):
  - https://crates.io/api/v1/crates/petgraph
  - https://crates.io/api/v1/crates/pathfinding
  - https://crates.io/api/v1/crates/rstar
  - https://crates.io/api/v1/crates/rust-sugiyama
  - https://crates.io/api/v1/crates/layout-rs
  - https://crates.io/api/v1/crates/steiner-tree
- rust-sugiyama: https://github.com/paddison/rust-sugiyama
- Tulip: https://www.labri.fr/en/valorisation/logiciels/tulip
- JointJS: https://github.com/clientIO/joint
- mxGraph licence: https://raw.githubusercontent.com/jgraph/mxgraph/master/LICENSE
- GoJS: https://gojs.net/latest/intro/links.html
- FreeRouting: https://github.com/freerouting/freerouting

### 10.5 The program's own documents (repository, read at HEAD)

- `docs/adr/0033-schematic-routing-model.md` (decisions and amendments 1–5)
- `docs/design/06-schematic-editor.md` (§2.4, §3.3, §3.5, §4.4, §5.2, §5.4)
- `docs/design/07a-symbol-design-standard.md`
- `docs/design/11-fluid-power.md`
- `docs/research/standards.md`
- `crates/ew-derive/src/routing/` (`search.rs` costs and search; `plan.rs` page solve, order, rip-up; `measures.rs`)
