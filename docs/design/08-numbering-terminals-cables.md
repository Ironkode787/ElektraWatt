# Connections, terminals and cables: wire numbering, terminals, cables and checks

A chapter of [Connections, terminals and cables](08-connections-terminals-cables.md), whose table says where each section lives.

---

## 5. Wire numbering

### 5.1 Where the number lives

`Connection.number: NumberSpec = Unassigned | Assigned{counter} | Frozen(Ident)` ([03](03-domain-model.md) §8.7).

- **Carrier.** The number of a physical wire is stored on its **carrier connection**. All other constituents stay
  `Unassigned` and print the wire's number. The number is bound to connection identity, so moves, route edits and
  splits change it only through the numbering rule (FR-NUM-023, FR-NUM-031). A split keeps the UUID and the number
  on the `a` part (FR-CON-008, [06](06-schematic-editor.md) §5.3).
- **Number transfer.** An edit can change which connection carries a wire, for example a junction sequence change or
  a redrawn IP leg. The number-transfer normaliser then moves the `NumberSpec` to the new carrier of the wire with
  the same two end CPs, in the same transaction, and with it the wire's `WireOverrides.ends` (the per-end overrides
  of [13](13-manufacturing-outputs.md) §2.3) and `WireOverrides.length` (a manual length). It rewrites every other
  record keyed by the old wire ID the same way, such as `Bond.via`. Reordering J1 to
  (E55:L, F55:2, E56:X2) is the example: W13 keeps its ends F55:2 and E55:L, but its carrier becomes the F55:2 → J1
  connection, and its number moves there. A number that finds no matching wire stays where it is and is reported as
  an orphaned wire number (info).

### 5.2 Schemes

A `WireNumberingScheme` holds format groups, an order, a counter scope and an online flag.

| Element | Values | Release |
|---|---|---|
| Orientation | connection-oriented (each physical wire its own number); potential-oriented (one number per potential) | MVP (FR-NUM-019) |
| Scope with net extension | signal: one number per signal, kept across terminal levels and plug pins | 1.0 (FR-NUM-020) |
| Tokens | `{page}` `{column}` `{row}` `{counter}` `{subcounter}` `{potential}` `{signal}` `{src_tag}` `{src_cp}` `{dst_tag}` `{dst_cp}` `{prefix}` | `{subcounter}`, `{signal}` and the src/dst tokens at 1.0 |
| Format groups | selector (circuit class, potential type, connection group, cores yes/no) → format, scope | 1.0 |
| Counter scope | page, potential, project | MVP |
| Formulas | sandboxed rhai (FR-NUM-026) | 1.0 |

- **Numbering order** (FR-NUM-028): the page of the wire's From end, then the From end's cell (column, then row),
  then the To end's page and cell (the page first, then column, then row), then, among wires equal in all of these, the From end's CP label, the To end's CP label (natural order, as `cp_label` prints them) and the first route vertex of the From end's constituent read from the From end (y, then x; a wire without one after those with one), and the wire ID only for wires equal in all of them. The cell of an end is
  the cell of the CP's drawn point on the page of the constituent that ends there, a terminal in a strip segment
  included (§6.7). Counters are assigned once and stored. `{page}` and `{column}` are tokens
  evaluated from the From end. A page renumber therefore changes page-based numbers only through its preview, as it
  does for tags.
- **Commands.** `NumberWires{scope, scheme, mode: All | Unassigned | NonConforming, include_frozen: false}` is
  `Cost::Bulk` and runs as a previewed fork (ADR-0008). Online numbering numbers new wires at commit when the scheme
  sets `online = true`. Otherwise they print "?" (FR-NUM-021).
- **Excluded.** Bridges are never numbered. Cores are numbered only if the format-group selector includes them.
- **Display** (FR-CON-026): on each segment, once per page or at both ends, with the text style `wire.number`.
- **Example** (▲U-33): connection-oriented `{page}.{counter}` (the `{page}` token prints the folio label; it is the
  one page token of every format), counter scope page, cores not numbered: W1–W34 receive 34 unique numbers (W29 102.10, W33 103.1); ties between wires of
  equal cells go by what is printed, then by the wire ID. The numbers are derived by the query `wire_numbers` and printed
  in the wire list; they are not stored (`NumberWires` is not built), and the pages print them with the drawing
  setting `wire_numbers` ([03](03-domain-model.md) §5.1). Where wires
  tie on their cells, the query keeps the counter range of the group (`WireNumber::tie`) beside the exact counter.
- **By potential.** A drawing specification may ask for wire codes by potential: one code per
  potential (0 V DC D01, 24 V DC D02, AC A01 …), a circuit number in front where a set has several circuits (6A03), every
  wire carrying its potential's code. The project setting `drawing.wire_numbering: per_potential` (Draw ▸ Connect ▸
  Number wires by potential; `project.settings.set`, member `drawing`) gives every wire the code of its potential: the potential's name
  by steps 1 to 2c of §3.5 (a definition's name, an interruption point's, a supply's), exactly as written. *What it is:*
  the person names the potentials as the specification codes them, circuit number included (`6A03`, `7A03`), and every
  wire of the potential prints that code; a frozen number still prints in place of the code; EW-POT-015 counts the
  potential as the unit, so the wires of one potential print one code without a finding, and two potentials of one name
  are one. *What it is not:* it does not make the codes (D01, D02 in the order of the diagram) nor the circuit number,
  which the model does not have; it gives no code to a potential named only by its fallback `<tag>:<cp>` (the wire
  prints none: name the potential), to a protective earth (type PE or PEN: its colour names it) nor to a bond wire,
  which joins two potentials; and it is not the signal scope of FR-NUM-020, nor `{potential}.{subcounter}`.
- **Potential-oriented example** (1.0): `{potential}.{subcounter}` with net extension gives W29 and the BN core of
  -106W1 one number, "24VDC-1.1", because terminal 1 is one signal. The WH core on terminal 2 is in another signal
  and gets "24VDC-1.2", because bridges bound signals.

### 5.3 Frozen on purpose

- **Freezing.** `FreezeNumbers{scope}` turns `Assigned` into `Frozen(text)`. `ReleaseRevision` runs it on request
  (FR-NUM-011, ADR-0003).
- **Effect.** Frozen numbers survive renumbering (FR-NUM-022). The numbering report lists each one with its
  rule-derived value (FR-REP-022), and EW-NUM-002 reports deviations as info (FR-CHK-037,
  [15](15-checks-revisions-collaboration.md) §4.3).
- **It prints.** A wire whose carrier holds a frozen number prints it in place of the scheme's: on the page
  (when the project prints wire numbers), in the wire list, in `api_v1_wire.number`, in Properties' title and Number
  row. It keeps its place in the count, so freezing one wire renumbers no other. A cable's core and a wire drawn on no
  page stay unnumbered, frozen or not. Properties' Number row takes the number typed as the page should print it
  (`6A03`); an empty entry unfreezes, and a text that is no identifier (a tab, a line break) is refused in words.
- **Import.** A QET `num` imports as a frozen number (FR-IMP-012).
- **Uniqueness.** The numbering unit is the scheme's scope: a physical wire (connection-oriented), a signal (signal
  scope) or a potential (potential-oriented). One printed number on two units, or two numbers within one unit,
  produce EW-POT-015 (error). Under potential orientation every wire of 24VDC-1 legitimately prints the same number.
  The machine pre-flight EW-MFG-005 ([13](13-manufacturing-outputs.md)) repeats the duplicate test on the export
  set, for duplicates that the export mapping creates; EW-POT-015 supersedes it per target.

---

## 6. Terminal strips and terminals

### 6.1 Model

- **Strip.** A strip is a device with the `Strip` aspect (-X1031).
- **Terminals.** Each terminal is a device with the `Terminal` aspect `{strip, seq, designation, levels,
  rail_bonded}` (FR-TRM-001, ADR-0006).
  - `seq` is the mounting and drawing order. It is independent of the designation (FR-TRM-003).
  - Parts may differ from terminal to terminal (FR-TRM-018). 1X1 uses UK 5 N × 2 and USLKG 5.
  - Physical terminals are counted from parts, never from symbol instances (FR-TRM-005).
- **Terminal CPs** come from the part's CP pattern: level, side, clamp, shaft and rail foot. PT 2.5-TWIN is level 1
  with clamp a on side 1, clamps b and c on side 2, and bridge shafts as its part data declares them (unverified).
  It has no rail foot. PT 2.5-TWIN-PE adds a rail foot.
- **Which end is which** (0.3.5). `ew.iec.terminal` draws clamp a (side 1) at its lower end and clamp b at its upper
  end, and looks the same turned half round, so a person could not tell the ends apart nor see what R did. While
  terminals are selected, the window names the clamp at each of their drawn ends in a chip centred on the end's line
  2 mm beyond it (`a`, `b`, the letter of the end's label `-X1:1/a`; the overlay label's anchor `Centre`, which the
  chip rule leaves where it is); nothing of it prints. The letters show while at most four terminals are selected
  (more would hide the symbols), and the hint line says which clamp is the internal side, the cabinet's wiring, and
  which the field side, read from the first selected terminal's sides and its strip's field side (§2.1).
- **CP index space of a terminal**. A terminal function is template-origin when its
  device carries a part whose function template declares a function with the same FnDef (and role, if given); the
  template's CP pattern then defines the function's CP indexes and count, not the FnDef: the PT 2.5-TWIN
  terminal has the CPs a, b, c and the shaft. The starter FnDef `ew.fn.terminal` keeps its two CPs, which apply to a
  terminal without such a part; no twin FnDef is needed.
- **Printed label.** `Strip.label_mode = Designation | FunctionText` decides what the terminal symbol prints
  (FR-TRM-021). The example's -X1031 is designated 1–20 and prints "1+", "2+", … "4-".
- **Groups.** `Strip.groups` prints "Pneumatics" and "Spare" once per range (FR-TRM-010).
- **Structure.** A terminal always takes its strip's structure assignment, never its page's. A terminal drawn on a
  page of another location still prints `=P01+J-E-2416-X1031:n` (FR-TRM-027).

### 6.2 Terminal types and internal connectivity

| Type (FR-TRM-019) | FnDef internal links (FR-TRM-020) | Effect |
|---|---|---|
| Feed-through | `Short` over all CPs of a level, shaft included | W29 (clamp a) and the BN core (clamp b) of -X1031:1 form one signal |
| PE | `Short` over clamps, shaft and rail foot; `rail_bonded` default true | joins the location PE node (§3.7) |
| Multi-level | `Short` per level; levels separate | an `InterLevel` bridge joins two levels |
| Disconnect, N isolating | `PassThrough{opens: true}`; N isolating sets `n_isolation` | splits potentials; EW-POT-013 reports N isolating terminals as info, not error |
| Fuse | `PassThrough{opens: true}`, category Protection | the downstream potential is `protected_by` the terminal |
| Diode, LED, sensor/actuator | `PassThrough` as declared by the part | type propagation only |

### 6.3 Multi-level and twin terminals

- **Multi-level.** A 3-level installation terminal has CPs 1a, 1b, 2a, 2b, 3a and 3b. It is one terminal with
  three rows in the terminal diagram (FR-TRM-004). The EPLAN report-spreadsheet importer folds consecutive
  same-designation terminals with rising levels into one multi-level terminal (EW-IMP-207,
  [14](14-interop.md) §5.4), and a connection list's ends land on the levels its level columns name.
- **A number per level.** A draughtsman draws a two-level block as two
  terminal symbols in two circuits with a number on each (1 above, 2 below). It is one terminal: one place on the
  strip (`seq`), one part, its levels listed in `Terminal.levels`.
  - *Members.* A `LevelSpec` may carry its own `designation`; a level without one is designated by the terminal's
    `designation` (one number on every level, as the EPLAN importer folds them), which is also that of the first
    level. `designation` is an optional member without cascade semantics (ADR-0011 amendment 6).
  - *Connection points.* A part whose CP pattern has the levels gives them. A terminal whose CPs (of its FnDef or a
    one-level part) carry one level gains a copy of them for every other level it lists, the k-th at index
    `idx + k·n`, n being the highest CP index plus one (`Terminal::level_copies`, read by `function_cps` and the
    commands alike): `ew.fn.terminal` listed with levels 1 and 2 has CPs 0 and 1 on level 1 and 2 and 3 on level 2.
    CPs without a level (a rail foot) stay once. The level rule of §6.2 joins each level's CPs; levels stay apart.
  - *How a person makes one.* Both numbers are drawn as terminals of the strip, or come so from an import; then the
    second joins the first: select its symbol, Join device… (J, or the page's right-click menu), and choose the first
    terminal, whose row says "next level" (`rep.join_device {rep, device}`). The second becomes the next level of the
    first (one above the levels it lists, else above the one its CPs carry): its symbols draw that level (each symbol
    CP by its pin map), its wires, bridges, potential definitions and channels move to the level's CPs, its number
    becomes the level's designation (none when it is the block's own number: two terminals of one number, as an import
    of a two-level terminal leaves them, become one block of that number with two levels), and its device goes, with
    what it carries besides, which the outcome names; the block keeps its part, or takes the joined terminal's when it
    has none. One undo step. Refused in words: a terminal of another strip, a terminal of more than one function, a
    terminal that has levels itself, a block whose part has no further level or whose new level lacks a connection
    point that a drawn or connected one would move to, a device link. A level's number is changed in Properties
    (Terminal ▸ Levels) or by `fields.set` on `terminal.levels`. The chooser lists the strip's free terminals only
    (none that lists two levels or more), the nearest on the strip first, on a tie the one before it.
  - *A row of blocks* (0.3.5). Draw ▸ Place ▸ Terminal…'s Levels 2 (`tool.place_terminal`'s `levels`) lays each
    terminal of the row as a block: two symbols side by side, numbered on (1 and 2, 3 and 4 …), the second of each pair joined into
    the first as above after it gave up its part, all in one undo step; How many counts the blocks. The form then
    proposes a part of two levels, one whose category's level word is `2Level`: the starter part EW-GEN-TRM-3, a
    feed-through terminal whose CP pattern gives clamps a and b on level 1 and on level 2.
  - *What prints.* The symbol that draws a level prints that level's designation (or the function text, by
    `label_mode`); a wire end at a level's CP reads `-X1:2/a`; the terminal diagram has a row per level (§6.9) with the
    level's designation, its level in the Level column (empty for a terminal of one level), potential, sides, bridge
    and state, the block's part on its first row; the parts list counts the block's part once. The Terminal strips
    navigator shows no Level column: its rows name a level by its number.
  - *The check.* EW-TERM-002 counts each level's designation as a designation of the strip.
  - *What an older build shows.* 0.3.0 opens such a project, keeps a level's `designation` in the level's `extra` and
    writes it back. It derives the CPs of the FnDef or the part only, so the copies of a block drawn from a one-level
    FnDef or part are not the terminal's CPs there: the second level's symbol prints the block's one number, its wires
    are drawn but its two sides are not joined, and the terminal diagram lists them on no row.
  - *Not built.* Splitting a block into terminals again (Undo takes a join back); a strip segment that shows a block's
    levels apart; the EPLAN importer still lands every level's connections on the first level.
- **Twin.** On PT 2.5-TWIN two wires on side 2 use clamps b and c. A third wire on side 2 needs a twin ferrule on b
  or c, which the clamp data must permit (FR-CON-028, FR-MFG-006, 1.0).
- **Where to land a wire (clamp pick).** A wire that ends on a terminal names a clamp. If the drawing leaves it
  open, the clamp-pick normaliser picks the first free clamp of the drawn side in clamp order and records it as a
  `cp` endpoint. The pick is never re-run behind the user's back. On -X1031:1 it gives W29 clamp a (side 1) and the
  BN core of -106W1 clamp b (side 2); clamp c stays free.

### 6.4 PE terminals

PE terminals are rail-bonded by default. Clearing the flag cuts their rail join. At MVP an island then appears as a
second PE potential in the location, which EW-POT-008 reports. At 1.0 the full continuity rule EW-POT-007 traces
wires, bridges, rail feet, carriers and surfaces to the main PE point (§8.2). FR-TRM-017 asks for exactly this:
"the PE continuity check needs no drawn wire to the rail".

### 6.5 Bridges versus wires

| | Bridge | Wire |
|---|---|---|
| Record | `Connection{kind: Bridge(BridgeSpec)}`; `page = None` when made in the strip editor, `Some` when drawn | `Connection{kind: Wire \| Core}` |
| Kinds | PlugIn (shaft), Saddle (1.0), Wire bridge, InterLevel | — |
| Joins | signals into one potential | CPs into nets |
| Appears in | terminal diagrams, assembly list, BOM (one part per group), wire list as a *bridge row* | wire list as a wire row, wire records, machine files |
| Numbered | never | by the scheme |

The printed wire list (FR-REP-014) shows bridges as a separate block of rows of kind "bridge". The reports' `bridges` kind is built on
`strip_bridges` (§2.3), and `bridge_groups` gives every bridge group of the project with its
label `BR1`, `BR2` … (numbered over the strips in natural order of the strip tag, then group, then strip ID), which the
Terminal strips navigator's Bridge column shows. Bridges are never wire rows, never wire records and never in machine files (FR-TRM-007,
FR-TRM-026, FR-MFG-016). At 1.0
the strip editor proposes bridges between adjacent terminals of one potential (FR-TRM-008). On a copy of -X1032
without BR6 it proposes one 5-pole plug-in bridge over 1–5. Strip segments draw the bridges as the familiar vertical
bus with dots. That drawing is derived from the bridge connections.

**A bridge on a schematic page.** Terminals drawn alone in a row with a bridge between them would stand as separate
terminals, so the page draws the bridge as a jumper bar is drawn in IEC 60617 practice: a line across the terminals
it joins with a connection dot on each.

- *What it is.* The program's drawing of the model's bridge, made from the bridge's connections and the terminals'
  places on every drawing of the page, like a junction's dot: no record, nothing to select, move or route, no wire
  number, no row of the wire list (the bridge keeps its bridge row there). The terminal diagram and the Terminal strips
  navigator name it (BR1); the page shows which terminals it joins and prints no name.
- *Where it stands.* Each terminal of the bridge that the page draws by a representation of its own gives a point on
  the lead of the clamp the bridge joins there (the connection point its connections name; on -X1021 clamp 0, the
  upper lead of 1 to 4, which are mirrored, and the lower lead of 5 to 8; a bridge on the bridge shaft, which no symbol
  draws, takes the lead of the terminal's lowest side-1 clamp, where `strip.bridge` bridges a terminal without a
  shaft), half way between the terminal's centre and that clamp. Points whose leads leave the same way and lie on one line across them (one height for upright leads, one
  x for turned ones) are one row. A row of two or more is one straight line from its first point to its last in the
  wire pen, with a dot of the junction dot's size at each point. So the bar stands on the side of the clamps it joins,
  clear of the terminal's circle and of its designation and function text, which stand at the circle's height.
- *In a strip segment.* A terminal a segment draws has its side-1 point above and its side-2 point below
  (§6.7); its centre is half way between them, so its point stands a quarter of the segment's height from the side the
  bridge joins, and the segment's bridged terminals make the bus across its leads with a dot on each, by the same
  rule.
- *What is not drawn wrong.* A terminal of the bridge on another page, in another row or turned another way joins no
  line on this page (a terminal drawn alone and one in a segment stand in two rows); a row with one terminal of the
  bridge draws nothing. Every line joins
  terminals the bridge joins, so a reader never sees a bridge that is not there; a bridge split over pages or rows
  shows as its parts, and the terminal diagram lists it whole. A terminal that stands between two of the row and is
  not part of the bridge is crossed without a dot, as a wire crosses a wire; across up or down leads the line hops
  over that lead with a half circle a quarter of a grid step high, so a block's level-2 lead never reads as bridged.
- *A block of two levels* (0.3.5). The level-2 symbol's point stands three quarters of the way to its clamp instead
  of half way, so a level-1 bridge and a level-2 bridge along the same leads run apart, each with its own dots; the
  level-1 line hops towards the bodies, the level-2 line towards the clamps. The two symbols of a block are tied by a
  short dashed line in pen 0.13 between their bodies, half a grid step above their middle (clear of the numbers beside
  the bodies), so the block reads as one terminal.
- *Texts keep clear of a bridge.* Each bridge line, a tick to either side, is one of the keep-outs of the layout's
  page, so the text placer sets no wire number or other text on one; the drawing and the layout share one
  computation of the lines. The router does not read these keep-outs: a wire may cross a bridge, as a wire crosses a
  wire.

### 6.6 Spares and reserves

- **Spare is authored.** It is the terminal's `Device.state = Spare` (FR-TRM-009). A terminal with no connection at
  all is *unused* (derived). It is reported only through strip statistics, never as an unconnected CP.
- **Example.** -X1031 has 12 spares (5–10, 15–20). Spares 5, 7 and 9 receive the feeds W30–W32 on side
  1, so a spare may be fed. A spare wired on its field side produces EW-TERM-003.
- **Unplaced terminals.** A terminal may exist unplaced, appear in the terminal diagram and be placed later from the
  strip editor (FR-TRM-015).

### 6.7 Segments on several pages

- **One device.** A strip is drawn as one or more `StripSegment` representations of its main function
  (FR-TRM-011). A segment stores no list of terminals: which terminals it shows follows from the strip's terminal
  order and the number of its segments, by the split below. -X1031 at 103-A3 (24 V group) and 103-A10 (0 V group) is one device with
  one BOM row (D-42).
- **Tag printing.** `tag_print = PerSegment` prints "-X1031" at both segments. `PerPage` prints it once per page.
- **Terminal CPs of a segment**. A segment gives each terminal it draws one
  CP on each side, so wires end there and a terminal shown by a segment needs no representation of its own. Segment j
  of a strip's m segments, ordered by (page position, y, x, ID), draws the terminals ⌊jN/m⌋ … ⌊(j+1)N/m⌋ − 1 of its
  N terminals in (`seq`, ID) order. Terminal k of the segment has its side-1 point at (g·(k+1), 0), leaving up, and
  its side-2 point at (g·(k+1), h), leaving down, where g is the segment symbol's grid and h the bottom of its R0
  bounds snapped to g; every function CP of a side draws at that side's point, and CPs without a side (shaft, rail
  foot) are not drawn.
- **Cross-references.** A strip segment is never a cross-reference target and never the cross-reference main
  representation of its device: it neither refers nor is referred to. The tag, numbering and
  BOM main representation is unchanged.
- **Terminals drawn alone**. A terminal with its own representation (1X1) prints its
  designation or function text (`label_mode`), never the full tag. On a page without a segment of its strip, the
  strip tag is printed once, at the strip's first terminal representation on the page in the strip's own order
  (designation in natural order, then terminal ID, then representation ID; as the strip's main representation is
  taken) whose tag slot is shown (its `visible` override, else its arrangement, else the slot's own `visible`); a
  strip without such a terminal on the page prints no tag there.
  Page 101 prints "1X1" at terminal "1" (a (row, column, ID)
  order would pick the PE terminal, as all three share 101-D2). The tag is a page-level text
  (`PageView::strip_tags`), at that terminal's tag-slot anchor moved one connection-grid step up in the text's own
  frame (exactly on the page axes for a quarter turn; otherwise by the deterministic arc point of doc 09 §4.3 rule 4),
  in the slot's style and horizontal alignment, bottom-aligned; the tag-placement rules neither move it nor yield to
  it, and the designation stays where it was. It is the strip's displayed tag on the page whatever `label_mode` is,
  so an unnumbered strip's terminals name it, with "?" for what is not derived: "-X101?" on page 101 under a strip
  rule with `{page}` (FR-NUM-021, FR-NUM-032), "-X??" without a page token. Under `label_mode` FunctionText a
  terminal prints its function text in its tag slot (an empty one falls back to the designation) and nothing in its
  function-text slot. `tag_print` PerPage prints the tag at the strip's first segment on each page in (row, column,
  ID) order.
- **Page token.** The strip's `{page}` comes from its first segment in (page order, row, column, ID). A strip
  without a segment takes the page of its first placed terminal (FR-NUM-032): the lowest page by folio order, folio
  label and page ID, then the lowest designation in natural order, then the smallest ID (the
  rule shared with [03](03-domain-model.md) §8.2 step 4). 1X1, drawn only as three terminal
  symbols at 101-D2, derives the rule value -X1011, which the numbering report shows next to its frozen tag "1X1"
  (EF-15); without the strip rule it prints -101X1. Moving the first segment of -X1031 to folio 104 makes the strip -X1041 everywhere, unless its tag is
  frozen ([03](03-domain-model.md) §8.4).

### 6.8 Designation uniqueness (D-11, D-12)

- **Prevent rule.** The designation must be non-empty and unique within the strip (FR-TRM-002, ADR-0009). It is an
  NFC plain string that is never translated (FR-I18N-002). Comparison uses the project's case setting
  (FR-NUM-016, FR-NUM-017). The rule is evaluated on the candidate snapshot at transaction end, so swaps,
  re-designations and inserts need no temporary names (FR-TRM-022).
- **Strict mode** rejects the transaction with the duplicate set listed in the error.
- **Lenient mode** (imports) commits and reports EW-TERM-001 (empty) and EW-TERM-002 (duplicate). For the reference
  set these are MF-08 and MF-09. The quick fix "renumber 1–n, keep labels as
  function text" is the conversion that turns the drawing's "1+ ×4" into terminals 1–4 that print "1+".
- **Generating designations** (FR-TRM-014): a pattern of prefix, start, step, numeric or alphanumeric and per level,
  as a preview. Schemes per profile follow at 1.0 (FR-NUM-024). Sorting uses natural order: 1, 2, … 10, never 1, 10,
  2 (FR-NUM-018).
- EW-TERM-002 is the prevent rule of this section, not a structural invariant. It is evaluated through the shipped prevent hook on the base and the candidate snapshot, and only a duplicate the batch creates is refused. The command `terminal.designate` sets the designations of several terminals in one transaction, so a swap needs no temporary names (FR-TRM-022). EW-TERM-002 skips fallback strips, those whose stored frozen tag contains `?`. The quick fix "renumber 1–n" is not built.

### 6.9 Terminal reports linkage

`terminal_rows(strip)` (memo per strip) is the single row source for the terminal diagram (FR-REP-009, folios
903–905), the strip overview (FR-REP-010), the line-up diagram (FR-REP-011, 1.0) and the terminal assembly list
(FR-MFG-016, 1.0): specified; not built. The terminal diagram's rows are its report kind's, `terminal_diagram` of
`crates/ew-reports/src/kinds/terminal_diagram.rs`, whose head names each column: per terminal (and per level of a
block of several levels) its strip, designation, level, printed label, group, part, potential, the far ends wired to
side 1 and side 2 (a CP without a side lists on side 1; a core with its cable), its bridge and its state.

Row 1 of folio 904 in the example:

> 1 · "1+" · Pneumatics · PT 2.5-TWIN · 24VDC-1 · used · side 1: -102F2:Out1 (W29), BR1 1–4 · side 2: -201U1:5 via
> -106W1 BN

Strip statistics are one value per strip. -X1031 has 20 terminals and 12 spares, its levels and parts. At 1.0 the
strip length is the sum of terminal and accessory widths (FR-TRM-024).

### 6.10 Plugs (1.0)

Plugs and sockets reuse the terminal model: the plug plays the strip's role, its pins are terminal-like devices with
male or female ends, and plug diagrams use the same row source (FR-TRM-023, FR-REP-024). The 7/8" socket of -106W1
is the example at 1.0. The PROFIBUS connector -3X1 is not a plug in this sense: it is a bus connector with
a port function, In entry A1/B1 and Out entry A2/B2 ([10](10-plc-and-bus.md) §2.7).

---

## 7. Cables and conductors

### 7.1 Model

- **Cable.** A cable is a device with the `Cable` aspect `{length, core_id_mode: Number | Colour | Both,
  pre_assembled}` (FR-CBL-001, FR-CBL-006).
- **Conductors.** Each core and the shield is a `Conductor{core, colour, csa, shield, pair?}`. Assigning the cable
  part generates the conductors from the part's `cable_cores` (FR-CBL-004). Colours are checked against IEC 60757 and
  the part (FR-CBL-005).
- **Pairs.** Twisted pairs group cores at 1.0 (FR-CBL-019).
- **Catalogue.** A cable-type catalogue (H05VV-F, control, bus) supplies core definitions at 1.0 (FR-CBL-017).

### 7.2 Cable definition line and conductor assignment

The cable's main function is drawn as the cable definition line with its marker (tag, part, core codes). Drawing the
line runs `cable.assign{cable, page, line, mode}` (FR-CBL-002, FR-CBL-003). Assignment is stored. Moving the line
later never re-assigns. Only the explicit "re-assign" command does.

```text
crossings = Wire/Core connections whose route crosses the line on this page, sorted by the crossing parameter t
mode ByOrder:  cores in part order → crossings in t order
mode ByColour: core colour = the crossing's effective colour (level 7 marker or imported text)
mode Manual:   edit-cable table
crossings > free cores → error, nothing assigned;   fewer → the remaining cores stay spare (FR-CBL-011)
each assigned connection becomes Core(conductor); a core may continue through IPs on other pages (FR-CBL-014)
```

On folio 106 the -106W1 line crosses five wires left to right, so ByOrder gives BK → pin 1 (W35), BU → 2 (W36), GNYE
→ 3 (W37), WH → 4 (W38) and BN → 5 (W39). An edit-cable table lists the cores with assignment, colour, targets and
potential. Swapping two cores is one transaction (FR-CBL-016).

**Where a cable without a drawn
line is numbered.** A cable's tag takes the page of its definition line when one is drawn; without one, the page of
its first connected core or of the first bus line it carries (the connection with the smallest ID); with
neither, the project's counter and no page
in its tag (`-W1`). A cable placed later takes its page's next free counter. `fields.set` refuses an unplaced cable a
counter another unplaced cable holds, in words naming no page, and renumbering groups the cables by the same scope
([03](03-domain-model.md) §8.2's note).

**The Cable form.** Draw ▸ Connect ▸ Cable… (`cable.from_wires`, on the Draw tab's Connect group)
makes the selected wires a cable's cores in one form (with nothing selected it is greyed with "Select the wires that
become the cable's cores"): the cable's tag (the tag a new cable would take, else the cable
one of the wires already belongs to), its type as a person writes it (`3G1.5`: three cores, the last green-yellow;
`4x0.75`: no green-yellow; or a cable part's number) and the wires with the core each gets, which can be changed. A
written type gives HD 308 S2's harmonised colours up to five cores (`2x` BN BU, `3x` BN BK GY, `4x` BU BN BK GY, `3G`
BN BU GNYE, `4G` BN BK GY GNYE, `5G` BN BK GY BU GNYE) and numbered black cores beyond, each core with the written
cross-section, so the wires print the core's values (level 6 of §4.1), not their potential's. The tag proposed is the
one the cable takes: the scheme's format for a device with the `cable` aspect on the page shown, not the strips'
format. A wire without a core takes one by what its potential says it is: its type, or without one a name (from a
definition, a source point or the supply) that is a type's own word (`N`, `PE`, `L1`, `L2`, `L3`, `L`, `L+`, `L-`, `M`,
`0V`). An N takes the blue core and a PE the green-yellow one (a PE with no green-yellow core left takes none); the
other wires take the cores left in the selection's order, never the green-yellow one. The type proposed for wires
in no cable is `<n>x<cross-section>`, or `<n>G<cross-section>` when one of them is a PE, so the protective conductor
has its core. Retyping the type proposes the cores of the wires not yet in a cable afresh (a core typed for one is
proposed again), and a wire of the cable keeps its core where the new type has it. Apply on a new cable is
one `cable.create` whose conductors list their wires (one undo step). On a selection that holds a cable's wire the
same form opens that cable: a wire added takes a free core, a core emptied takes the wire out, a changed type sets the
cores' cross-section; that is one `fields.set_many`, and one `conductor.add` more when the cable gains cores. A typed
tag of another existing cable puts the wires in that cable. New cable… and Connect cores… stay in the menu. A wire's
Properties name its cable and core in its Kind row (`-1W1 BN`).

### 7.3 Pre-assembled cables

The NEBU-G78W5-K-2-N-LE5 part declares a 7/8" socket on end B with a pin per core (BK 1, BU 2, GNYE 3, WH 4, BN 5,
▲U-18), an open end A and a fixed length (2 m, ▲) (FR-CBL-013).

- EW-CORE-035 reports a core whose end-B CP designation differs from the part's pin.
- The research note says common 7/8" practice is 1 BK, 2 BU, 3 GNYE, 4 BN, 5 WH (D-17, unverified). If vendor data
  confirms that, the corrected part data makes EW-CORE-035 report the drawn WH/BN assignment. That is the intended
  effect.
- At 1.0 a pre-assembled cable routes as one object and is checked as route length ≤ part length minus connector
  allowances (FR-LAY-050).

### 7.4 Shields

- **Model.** A shield is a conductor with `shield = true` (FR-CBL-007). Its terminations are ordinary `Core`
  connections to a shield clamp, FE or PE, or to a connector shell CP.
- **Single-ended shielding** is legal. A shield connected at neither end produces EW-CORE-037 (warning). The rule
  skips cables carried only by single-line `Bus` lines (§7.5), where the bus connectors terminate the shield. The
  -3W1 shield is terminated by the connector -3X1 to FE (FR-CBL-007, ▲U-21).
- **Bonds.** A shield bonded to PE at a cabinet entry may use `Bond{kind: Shield}`, so that FE and PE stay distinct
  where required.

### 7.5 Bus cables, single-line

On topology pages a bus line is one `Connection{kind: Bus(BusLine{cable})}` between bus-port CPs (FR-CBL-008). -3W1
on folio 3 is one line with its marker, from the In entry of -3X1 to the external point "DP" (JE-N1000-03 in
+J-E-206). Its cable diagram lists the cores A, B and shield from the part (Siemens 6XV1830-0EH10). A single-line
cable's ends are the line's two endpoints, ordered by the rules of §3.8: -3X1 is end A and the external reference
is end B (FR-CBL-015). Per-core pins are resolved only where a multi-line representation of the port exists. Port
signal maps belong to [10](10-plc-and-bus.md). Bus lines form no physical wires: they appear in
cable rows and in the bus cable list (FR-BUS-008).

### 7.6 Lengths

`LengthSource = Manual(Nm) | FromPart | Estimated(C) | Routed(C) | Missing` (FR-CBL-009).

| Source | Release | Rule |
|---|---|---|
| Manual, FromPart, Missing | MVP | Missing is a warning while the drawing grows and an error that blocks release in the check for delivery (EW-CORE-033, FR-CHK-033) |
| Routed | 1.0 | a committed computation from `RouteWires`; stale once its input hash changes (EW-LAY-011, FR-CBL-010) |
| Estimated | 1.0 | from footprints and duct paths, flagged (FR-LAY-053) |

Example: -106W1 and -106W2 are FromPart (2 m, ▲). -3W1 is Missing (EF-08). The cable length is the cable's value.
Single-wire lengths belong to [12](12-panel-layout.md) and [13](13-manufacturing-outputs.md).

**Voltage drop (1.x).** With a length, a core cross-section and the load current of the potential (FR-REP-026 data),
a cable's voltage drop is computed per core pair and checked against a per-profile threshold (EW-POT-020,
FR-CHK-059). The relevant case is a long 24 V field cable between cabinets such as the J-E-206 links. Limit values are
unverified (IEC 60364-5-52 informative), so the table ships with `verified = false`.

### 7.7 Cable ends and reports

- **Ends** are derived (FR-CBL-012). End A is the set of devices at the From ends of its cores, and end B the set at
  the To ends (§3.8), each with its location. -106W1 runs from `+J-E-2416` -X1031/-X1032 to -201U1.
- **External end.** A cable end may be an external reference (FR-CBL-015): the supply cable -101W1 runs
  from `+J-E-206` to 1X1. Wires to an external connection point that are not cores of a cable produce EW-CORE-034,
  one finding per external reference (EF-07).

The cable diagram (FR-REP-012) and the cable overview (FR-REP-013) on folio 906 are one report kind,
`cable_overview_diagrams` of `crates/ew-reports/src/kinds/cable_overview.rs`, whose head names its rows and columns:
one row per cable (type, ends A and B, length), then per cable a heading and one row per core (colour,
cross-section, the ends of its wire): 3 cables, 13 cores and 4 headings, 20 rows. A memo `cable_rows(cable)` shared
with other consumers is specified; not built.

---

## 8. Check rules

### 8.1 Catalogue

This document owns the content of these rules. [15](15-checks-revisions-collaboration.md) §4.4 adopts the IDs
unchanged, assigns each rule its kind and scope, and §2.2 gives their six-digit codes: EW-POT in class 005
(connections and potentials, EW-POT-013 → `005013`), EW-TERM in class 001 (terminals), and the cable rules in
EW-CORE's 03x block of class 003, mirroring the EPLAN class topics ([research](../research/eplan-features.md) §9).
15 adds EW-POT-018 (load versus rating, FR-CHK-041, 1.0). EW-POT-019 (mixed units, 1.0) and EW-POT-020 (voltage
drop, 1.x) were proposed by this document and are in 15's catalogue. The modes are o = online, f = offline,
p = prevent and r = release.

| Rule | Checks | Req. | Modes | Severity |
|---|---|---|---|---|
| EW-POT-001 | Type or voltage conflict in one potential without a bond | FR-CHK-028 | o r | error |
| EW-POT-002 | PELV potential without exactly one 0 V–PE bond; SELV potential with any | FR-CHK-028 | o r | error |
| EW-POT-003 | FE-role CP on a PE potential without an FePe bond | FR-CHK-030 | o r | warning |
| EW-POT-004 | GNYE on a non-PE potential (bond wires exempt) | FR-CHK-031 | o r | error |
| EW-POT-005 | Exact code BU on a non-N conductor in a project with an N potential | FR-CHK-031 | o r | warning |
| EW-POT-006 | PE cross-section below the sizing table for its associated line conductors | FR-CHK-020 | o r | error |
| EW-POT-007 | PE continuity to the main PE point through wires, bridges, rails, carriers (1.0) | FR-CHK-029 | f r | error |
| EW-POT-008 | More than one PE potential in one location (PE island), or a rail-bonded terminal without a location in a project with two or more own locations (with at most one, the terminal belongs to the implicit main location, §3.7) | FR-CHK-055 | o r | warning |
| EW-POT-009 | Unconnected electrical CP of a placed function, not `unused` (fluid ports: EW-FL-001) | FR-CHK-018 | o r | warning; info if `unused` |
| EW-POT-010 | Conductors or csa on a CP beyond its clamp data (1.0) | FR-CHK-021 | o r | error |
| EW-POT-011 | Junction feed ambiguous (§3.2) | FR-CON-041 | o | warning |
| EW-POT-012 | Wires in a ring (a ring in the drawing graph of a net) | FR-CON-005, FR-CON-035 | o | warning |
| EW-POT-013 | Opening pass-through pair carrying N without the associated L (§8.2) | FR-CHK-019 | o r | error |
| EW-POT-014 | Physical wire with constituents on different conductors or with conflicting overrides | FR-CHK-057 | o r | error |
| EW-POT-015 | One wire number on two numbering units, or two numbers within one unit (§5.3) | FR-NUM-019, FR-NUM-020 | o r | error |
| EW-POT-016 | Override equal to the value below it | FR-CON-025 | o | info |
| EW-POT-017 | Two definitions with different names in one potential | FR-CON-012 | o r | warning |
| EW-POT-019 | Cross-section units mixed within one potential (mm², AWG, kcmil) (1.0) | FR-CON-032 | o | warning |
| EW-POT-020 | Voltage drop of a cable above the profile's threshold; threshold table unverified (1.x, §7.6) | FR-CHK-059 | f r | warning |
| EW-POT-022 | Cross-section set by hand below the size the rating of the wire's protecting device needs by the current table (§4.2); fix "Take the proposed size" (0.3.5) | — | o r | warning |
| EW-TERM-001 | Terminal without designation | FR-TRM-002, FR-CHK-016 | p o r | error |
| EW-TERM-002 | Duplicate designation within a strip | FR-TRM-002, FR-CHK-016 | p o r | error |
| EW-TERM-003 | Spare terminal wired on its field side | FR-TRM-009 | o | warning |
| EW-TERM-004 | Bridge group not consecutive, overlapping, or across strips | FR-TRM-007 | p o | error |
| EW-TERM-005 | `rail_bonded` set on a terminal without any PE CP | FR-TRM-017 | o | error |
| EW-CORE-031 | Core colour not an IEC 60757 or profile code; a prevent rule, so only a lenient import stores a foreign code such as "BR", with the quick fix "use BN" | FR-CHK-032 | p o r | error |
| EW-CORE-032 | Core differs from the cable part's core definition | FR-CHK-032 | o r | warning |
| EW-CORE-033 | Cable without length (blocks release) | FR-CHK-033 | o r | warning (release: error) |
| EW-CORE-034 | External connection without a cable | FR-CHK-033 | o r | warning |
| EW-CORE-035 | Pre-assembled core on a pin other than the part's pin map | FR-CHK-056 | o r | warning |
| EW-CORE-036 | Core assigned to two wires, or more assignments than cores (invariant CI-05) | FR-CBL-003, FR-CBL-011 | p o | error |
| EW-CORE-037 | Shield connected at neither end | FR-CBL-007 | o | warning |

Frozen wire numbers deviating from the scheme are reported by EW-NUM-002 (FR-CHK-037). The
PLC-specific card supply rule EW-PLC-013 supersedes EW-POT-003 at 1.0 (EF-22). The opt-in EW-EN60204 pack
([15](15-checks-revisions-collaboration.md) §4.7) includes EW-POT-004, -005, -006, -007 and -010 with clause
references and adds overcurrent coordination (I_b ≤ I_n ≤ I_z, EW-EN60204-007) over the effective cross-sections of
§4.

### 8.2 Algorithms of the safety rules

**EW-POT-013, switching only the neutral (D-13)** (pack EW-POT, scope potential, online):

```text
for each function f, for each PassThrough pair p = (u, d, opens = true) of f.def.internal:
  P_up = upstream side of p (potential_flow)
  if ptype(P_up) ∈ {N, PEN}:
     ok = f has an opening pair whose upstream type ∈ {L, L1, L2, L3}   // its poles are one mechanism
       or (another function of f's device has one and loads(P_d, S) = ∅, S = the downstream potentials of all
           the device's opening pairs)   // a 1P+N breaker drawn as a pole per function
     if f.def.flags.n_isolation: report info "N isolating terminal" and continue
     if not ok: report error {function f, potential P_up → P_d, loads}
  loads = loads(P_d, the downstream potentials of f's opening pairs)
  loads(P, S) = devices with CPs in P whose line CPs lie in a potential outside S
```

- **Poles of one device.** Another function's line pole excuses an N pole only where the device's poles open the
  line of every load that N pole feeds: a 1P+N breaker drawn as a phase pole and a neutral pole is accepted, a relay
  whose contact 13-14 switches the line of one lamp and whose contact 23-24 opens the neutral of another lamp left on
  its line is reported.

- **Example.** T55 is reported: pair 5–3 opens N-55 into N-55T, and E55:L stays on L-55, which T55 does not switch.
  S56 is reported: pair 13–14 opens N-55 into N-55S, and E56:X2 stays on L-55.
- **Not reported:** S51 (2-pole, opens L as well) and the MCBs -101F1 and F55 (1P+N). A function that opens a line
  pair as well switches its neutral with it (its poles are one mechanism), so a two- or four-pole breaker or switch of
  any definition is accepted, flag or not; the starter `ew.fn.mcb-1pn` sets `flags.switches_n_with_l`. The message says why and what to do: "-1F1 opens the neutral N → N/1F1:4 without
  the phase: what is behind it stays live while it looks switched off. Put the contact in the phase, or use a device
  that opens phase and neutral together, such as a two-pole breaker."
- **Message:** "T55 switches the neutral N-55 → N-55T; load E55 stays connected to L-55." There is no automatic
  quick fix, because the correct wiring is a human decision (▲U-11). A waiver needs a reason.
- **Scope.** A single-pole fuse in N is reported the same way.

**EW-POT-007, PE continuity (1.0, offline)**:

```text
main = PE CP of the location's main earthing terminal (device role MainPe, -XPE), else the incoming-supply PE terminal
graph: PE-typed CPs; non-bond physical wires and bridges of the PE potential; terminal Shorts; rail feet of
       rail-bonded terminals → their carrier if carrier.pe_bond; carrier → surface if surface.pe_bond; surface → main
BFS from main; every PE-typed CP not reached → error with the nearest reached node and the missing link kind
```

- **Example.** No finding (EF-23). -X1032, 1X1:PE, -XPE and the CPX FE screws reach the plate through the bonded
  rails R1–R3.
- **Test.** Clearing `rail_bonded` on -X1032 cuts off an island: the five -X1032 terminals and the PE CPs wired to
  them (E55:PE via W19, -102G1:PE via W28, -201U1:3, -201U1:FE, -202U1:3 and -202U1:FE). EW-POT-007 reports each
  unreached CP once, grouped in one finding per island with the missing link kind "rail foot not bonded". At MVP
  the same edit yields one EW-POT-008 warning naming the PE of -X1032's side, which reaches no earth, by its first
  pin and page.
- **Checklist.** The same traversal feeds the EN 60204-1 18.2.2 continuity checklist (FR-MFG-036).

**EW-POT-006, PE sizing (D-38).**

- **Associated line conductors.** For each PE wire they are, at each end:
  - at a terminal end, the line-type wires on the same side of the same strip;
  - at a device end, the line-type wires on the device's CPs.
- **Rule.** The required PE csa comes from the sizing data pack (copper: equal up to 16 mm², then 16, then S/2;
  IEC 60364-5-54 and EN 60204-1 8.2, values marked (verify), unverified data). The default
  parameter `line_types = [L, L1, L2, L3]` excludes L+.
- **Example.** W6 at 1X1:PE (internal side) has W4 at 2.5 mm² next to it and is reported. W3 at 1X1:PE (field side)
  has only the 1.5 mm² W1 next to it and is not.
- The table ships as rule data, `crates/ew-checks/data/pe_sizing.toml`, with
  the clauses cited and not reproduced and `verified = false` (NFR-COMP-05); while it is unverified every finding says
  so. Cross-sections in AWG or kcmil are not compared. One finding per PE wire, at the end with the largest
  requirement, targeting the wire's carrier connection.

**EW-POT-009, unconnected CPs (D-23).** The rule leaves out:
- terminal CPs, which strip statistics cover;
- CPs of unplaced functions;
- non-physical CPs;
- CPs whose CpDef is optional (`CpDef.optional`, [07](07-symbols-parts-macros.md) §2.4), such as the outgoing
  entry of a terminating bus connector or a parallel duplicate pin;
- the pin of a PLC channel whose effective state is spare, reserved or deactivated ([10](10-plc-and-bus.md) §3.2):
  the channel's state already says that the pin stays free, so a card with one used channel gives no warning for the
  other seven.

Marking a CP `unused` turns the warning into an info.

---

## 9. UI and UX touchpoints

The interaction design is in [16](16-ux-ui.md), canvas tools in [06](06-schematic-editor.md). This area needs:

| Touchpoint | Content | Req. |
|---|---|---|
| Potential navigator | potential → signals → nets → CPs, each with canonical name, aliases, type, voltage, class, switched, protected by, colour, size, pages and member count; "define potential", "trace" (all CPs, devices, pages, IPs), "highlight" with pattern plus colour | FR-CON-027, FR-UI-005, FR-UI-018 |
| Property grid for a wire | effective csa, colour and part with provenance, override marker, "reset to derived", wire ID, number, From/To, constituents, carrier | FR-CON-025, FR-UI-010 |
| "Why this value?" | the full 7-level trail of §4.1 | FR-UI-011 (1.0) |
| Junction page | sequence as a reorderable list with the resulting physical wires | FR-CON-005 |
| Strip editor | table of seq, designation, label, group, level/clamp layout, part, potential, side 1, side 2, bridge comb column, state, rail-bonded. Insert, delete, move, re-designate, bridge a range, add accessory, generate levels, place unplaced: each one transaction, with prevent-rule violations shown in the ghost before commit | FR-TRM-013 |
| Edit-cable table | cores with colour, csa, shield, pair, assignment (From → To), potential, spare; swap; "assign by order" and "assign by colour" | FR-CBL-016 |
| Wire list table | the connection navigator in table mode (one of the MVP navigators of FR-UI-005), keyed by wire ID, with Excel round trip | FR-UI-005, FR-UI-008, FR-EXP-004 |
| Quick fixes | "declare PELV bond via this wire", "set core colour BN", "renumber 1–n, keep labels", "mark CP unused", "create supply cable", "remove override", "move override to profile" | FR-CHK-010 |

The strip editor is the Terminal strips navigator in
table mode, opened on one strip by Edit ▸ Strip editor (`strip.edit`), which sets its filter to `strip:<tag>`. Its
rows are in mounting order (the `Order` sort: strips in natural tag order, `-X1031`, `-X1032`, `1X1`; each strip's
terminals in `seq` order, with its accessories as rows of their own at their `after_seq`, numbered `<after_seq>.<k>`
in the Pos column). The columns add Pos (read-only: "move rows with Alt+Up and Alt+Down") and Bridge part (the
carrier's part, §2.3); the Group cell edits the strip's group labels on the terminals; an accessory row's Part cell
edits the accessory's part. Insert (`terminal.insert`: the following terminals, group bounds and accessory positions
shift by the count; the new terminals copy the reference terminal's definition and parts), delete (terminal rows
through the table card; accessory rows directly), move (`terminal.move`: only the sequence values of the changed
range are permuted, so groups and accessories stay where they are and a moved terminal takes the group of its new
place), add accessory (one `fields.set` of `strip.accessories`), Bridge terminals and Remove bridge are each one
command and one undo step; designations never change. A terminal's Properties show its function text as "Printed
label" and an editable "Group" row. Placing an unplaced terminal from its row (Enter, drag), bridge
proposals (FR-TRM-008), generate levels and the strip length are not built.

Wire colours are never shown by colour alone (FR-UI-025, NFR-A11Y-03). The keyboard path from CP to CP is in
[06](06-schematic-editor.md) §4.4.

---

## 10. Outputs

Report kind names are those of [09](09-reports-forms-output.md) §2.4.

| Output (report kind) | Row source | Req. |
|---|---|---|
| Wire list `wire_list` (folio 907, `ew export wires`) | physical wires plus bridge rows: number, From/To with level/clamp, potential, csa, colour, wire part, length and source, cable and core, page references, provenance | FR-REP-014, FR-CON-034 |
| Terminal diagrams `terminal_diagram` (903–905, `ew export terminals`) | `terminal_rows` | FR-REP-009 |
| Strip overview `terminal_overview` | strip statistics | FR-REP-010 |
| Cable overview and diagrams `cable_overview_diagrams` (906, `ew export cables`) | `cable_rows` | FR-REP-012, FR-REP-013 |
| Potential overview `potential_overview` (909) | partition and flow: name, aliases, type, voltage, class, colour, default csa, switched, protected by, bonds, member counts, pages | FR-REP-016 |
| Numbering report `numbering_report` | frozen numbers with derived values | FR-REP-022 |
| Cover tables `profile_tables` (folio 1) | profile rows | FR-REP-028, FR-REP-038 |
| Device connection diagram `device_connection` (1.0) | `function_cps` per device joined with `physical_wires_of` | FR-REP-015 |
| Load summary `load_summary` (1.0) | `potential_flow` (`protected_by`), the CP role classification of `api_v1_cp.role` (`source`, `through`, `load`, `other`; [17](17-extensibility.md) §6.1, ADR-0017 amendment 3) and the optional part load current per supply CP (`i_load`, [07](07-symbols-parts-macros.md) §2.5) | FR-REP-026 |
| SQL projection | `api_v1_wire`, `_terminal`, `_cable`, `_conductor`, `_potential` | FR-EXT-007 |
| Manufacturing (1.0) | `WireRecord` per physical wire ID, without bridges | FR-MFG-001, FR-MFG-002 |

`api_v1_wire` columns ([17](17-extensibility.md) §6 owns the list): `id`, `number`, `kind` (wire, core, bridge),
`from_device_id`, `from_tag`, `from_cp`, `from_level`, `from_clamp`, `to_device_id`, `to_tag`, `to_cp`, `to_level`,
`to_clamp`, `potential_id`, `potential_name`, `csa`, `csa_unit`, `colour`, `colour_iec`, `wire_part`, `length_nm`,
`length_source`, `cable_id`, `cable_tag`, `core`, `pages`, `prov_csa`, `prov_colour`.

PDF, XLSX, CSV and `.ewpp` read the same row source, so the printed list and the machine file cannot disagree
(FR-REP-030, ADR-0018).

---

## 11. Performance and determinism

| Query | Policy | Budget link |
|---|---|---|
| `page_local_nets(page)` | memo per page | live highlight inside the 16 ms frame (NFR-PERF-04) |
| `stitch_graph`, `potential_partition` | memo, global | commit of a wire gesture: p99 S/M ≤ 5 ms, XL ≤ 20 ms (NFR-PERF-05) |
| `potential_flow(potential)` | memo per potential over one BFS per partition change | the same commit budget (NFR-PERF-05) |
| `physical_wires_of`, `wire_props_of` | memo per potential | profile change of "24 V DC" at XL, about 60k wires, ≤ 100 ms (NFR-PERF-07) |
| `terminal_rows`, `cable_rows` | memo per strip or cable | report pagination (NFR-PERF-10) |
| checks of §8 | online per potential, strip or cable; EW-POT-007 offline | ≤ 5 ms per rule per commit, else demoted; full check (NFR-PERF-09) |

Determinism (NFR-REL-07, FR-NUM-028) has four sources:
- union-find edges are applied in a fixed key order;
- every tie is broken by what is printed, then by ID;
- profiles reject ambiguous rows at load time;
- numbering order never uses creation order.

The oracle compares memoised results with cold recomputation after every sampled transaction (NFR-REL-08). Pinned
derivation profiles keep wire numbers and properties byte-identical across EW releases (NFR-REL-09).

---

## 12. Edge cases and pitfalls

1. **Two wires on one CP pair** are two physical wires with two IDs. The clamp count is 2. They are never merged.
2. **A wire across one contact** (a jumper over a pass-through) is legal. Both sides join one potential, and flow
   treats the contact as bypassed.
3. **A junction left with two connections** dissolves into one connection, which keeps the lower ID (CI-04). If the
   removed connection was the carrier, the number-transfer normaliser moves the wire's number onto the survivor
   (§5.1).
4. **Junction on junction** with disagreeing feed arms is resolved deterministically and reported (EW-POT-011).
5. **Odd chain-mode IPs.** The unpaired point keeps its wire open-ended. The wire record is marked "open end" and is
   excluded from machine export (FR-CHK-017).
6. **Wires to an external connection point** print the external target as their From end ("+J-E-206 /
   JE-N1000-03 (L)"). They are field wiring and are excluded from machine export by default.
   A placed point becomes one by Supply comes in here… (`ip.supply_in`): the supply that
   enters a drawing set by interruption points (L1, L2, L3, N, PE from another drawing) is an external reference, no
   flag of its own. One transaction makes the location node where the project lacks one of that code, the reference
   (reused for the same location and device), sets every point's `ip.external`, and the supply's
   `origin.source` where the project has a supply without one; `remove` clears the points' `ip.external`. The
   points then pair with nothing (no EW-CORE-051); their wires are an external connection, which EW-CORE-034 asks
   to be a cable's cores.
7. **A core whose wire continues** through an IP as a `Wire` connection on another page is still a core. The `Wire`
   constituent inherits the conductor and must not override its colour (EW-POT-014).
8. **A bridge between two defined potentials** merges them and raises EW-POT-001 if their types differ. It is never
   silently renamed.
9. **Net extension stops** at disconnect and fuse terminals (PassThrough) and continues across feed-through levels
   (Short).
10. **Case and NFC.** "pe" and "PE" are duplicates only in case-insensitive projects. A Cyrillic "Р" in "PE" is
    flagged as a confusable (FR-NUM-016).
11. **A strip used by mains and control** gives all its PE wires the Mains class. Individual wires can be
    overridden.
12. **Fallback names.** A potential-oriented number built from a fallback name (`-102F2:Out1`) changes on renumber.
    The numbering preview lists these, and an info suggests a definition point.
13. **A machine without company codes** receives the base code through `export_as`. The label keeps the company code.
14. **QET imports.** A QET junction element (one terminal, three or more conductors) becomes `Junction{Dot, Star}`
    with the info EW-IMP-118 "junction wiring assumed" ([14](14-interop.md) §4.7). "2,5" with a decimal comma parses
    as 2.5 mm². "BR" is kept as a foreign code, BN proposed (EW-CORE-031, MF-11). Strips come from QET 0.100+ strip
    data or are inferred from label prefixes
    (FR-IMP-013); cables come from QET 0.200 cable data or are inferred (FR-IMP-014); both are listed for review.
15. **A spare core with one end terminated** is listed as spare with that end.
16. **Undo of a profile change** restores every derived colour in one step, because no wire stored the old value.

---

## 13. Defects prevented

The tests open the sample project (`fixtures/sample/`). The defects of the
reference drawing set that this area prevents:

| Defect | Class | Mechanism here |
|---|---|---|
| D-04 (partly) | S | one strip is one BOM row; bridges counted per group; cables are devices with parts |
| D-08 free-text wire rule | S | profile rules with provenance (§4); cover tables generated |
| D-09 missing lists and numbers | S | wire list, terminal and cable diagrams, potential overview, wire numbering (§5, §10) |
| D-11, D-12 designations | I | prevent rule at transaction end; lenient import with quick fix (§6.8) |
| D-13 N switching | C | EW-POT-013 on derived flow (§8.2) |
| D-16 FE vs PE | C | EW-POT-003 from part roles; FePe bond |
| D-17 "BR" | S + C | `ColourCode` enum; EW-CORE-031, EW-CORE-032 |
| D-19 missing cable and lengths | C | EW-CORE-033, EW-CORE-034; length sources |
| D-20 junction notation | S | junction wiring and sequence, physical wires (§3.2, §3.8) |
| D-21 PE continuity | S + C | rail bonding (§3.7); EW-POT-008 at MVP, EW-POT-007 at 1.0 |
| D-22 0V–PE by a wire | S | `Bond` with `via`; EW-POT-001 and EW-POT-002 |
| D-23 unconnected 13-14 | C | EW-POT-009 |
| D-24 five names on one net | S | canonical name and aliases (§3.5) |
| D-25 crossed supply pairs | C | `protected_by` per potential feeds the load summary (FR-REP-026) and EW-POT-018 (FR-CHK-041, 1.0) |
| D-37 profile basis | S | `basis` on every profile |
| D-38 PE smaller than line | C | EW-POT-006 |
| D-42 strip drawn twice | S | segments of one strip device (§6.7) |

