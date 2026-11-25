# Manufacturing outputs: the workshop lists

A chapter of [Manufacturing outputs](13-manufacturing-outputs.md): what the panel builder hands his fitter beside
the wire list. Per strip what to mount in which order (§6), for the cabinet what to cut (§9.3), and what to order of
the parts a wire consumes (§3.6). The parts these lists print, their accessories and variants and the project's
workshop stock are [07](07-symbols-parts-macros.md) §2.5; the wire records, the end treatments and the markers they
count are §2 to §5 of [13](13-manufacturing-outputs.md).

**Where they are in the window.** The Output tab's Workshop group holds Assembly lists, Cut list and Consumables. Each
opens the project's list page of that kind; a project without one opens Report block… on that kind, whose Create
makes it (one undo step, [09](09-reports-forms-output.md) §3.8). A list page prints on the set's frame like every
list, follows the project when it changes, and exports from its page's menu as CSV and XLSX (`ew export report
--kind assembly_list | cut_list | consumables --format csv | xlsx`). None of the three needs a layout: without one the
assembly list names no rail and the cut list holds no rails or ducts. Their findings stand in the Messages pane under
Check ▸ Check project (§6.3).

---

## 3.6 Consumables summary (FR-MFG-049)

`consumables` (list kind `consumables`, rows of `ew-reports` over `ew-derive`) is what to order of what the wires
consume, separate from the parts list, which counts devices and their accessories. It reads, of the wire design and
nothing else:

- each wire record of the project's cabinet wires (`wire_record_set`, §2.4): its `csa`, `colour`, `wire_part`,
  `length` (the value and whether there is one) and whether it is a field wire;
- of each end the shop makes, its `treatment`: the ferrule's or lug's part, a twin ferrule's part once for its two
  wires, or none (bare, a connector, undetermined);
- of each label row of the project's label sets (`label_rows`, §5.3), its marker part.

Three groups, in this order:

| Group | One row per | Quantity |
|---|---|---|
| Wire | wire part, else size and colour | the summed lengths in metres, exact to the millimetre; "length missing for n wires" while any has none |
| Ferrules and lugs | ferrule or lug part | the ends it is crimped on, a twin ferrule counted once; ends left without a part count under their size as "no ferrule found" |
| Markers | marker part and label set | the labels |

A field wire (an end outside the cabinet) counts the ferrule and the markers of the end the shop makes, but not its
metres: it is cut on site. The row prints the part's number and description, or the wire's size and colour as the
lists print them; the For cell says what it serves ("12 wires", "2.5 mm²", "Wire markers"). Nothing is rounded here:
a wire's length is the wire list's, and an order quantity belongs to the order list.

## 6. Terminal-strip assembly lists

### 6.1 Content (FR-MFG-016)

`assembly_list` (list kind `assembly_list`) prints, per strip, the order the fitter mounts it in. Its rows come from
`assembly_rows(strip)` of `ew-derive`, which reads `strip_layout` and `part_accessories`
([07](07-symbols-parts-macros.md) §2.5, [12](12-panel-layout.md) §3.4) and the strip's bridges (`strip_bridges`);
the strips stand in the order of their first segment's page, then by tag.

**What he reads first** is the strip's heading row: its tag, where it goes and how long it is, as one text with
arguments ("-X1011 · on rail 1 from 0 to 48.2 mm · rail cut to 470 mm · 5 terminals · 48.2 mm with its end
brackets"). A strip on no rail says "on no rail"; a strip turned on its rail says that its first terminal stands at
the right; a strip placed in pieces prints a heading per piece.

**Then the mounting order**, one row per item from the rail's left end: the end bracket before it (a strip that abuts
the one before it starts against that strip's bracket and says so in its heading), the terminals by `seq`, each
accessory at its place (an end plate after the last terminal whose part names it, the strip's own after the
terminal it follows), the end bracket after it.

| Column | Holds |
|---|---|
| Pos | 1, 2, … in mounting order |
| Item | End bracket, Terminal, End plate, Partition plate, Marker carrier, Cover (the accessory's kind), Accessory |
| Terminal | the terminal's designation; a block's levels print once |
| Part number, Description | the part's, the variant's where it gives its own (07 §2.5) |
| Colour | the technical value `colour`: the device's own, else its variant's, else its part's |
| Width | the item's width in millimetres; a part without one prints "width missing" and the heading's length says "at least" |
| Marker | the terminal marker's text (§5.3) |
| Note | spare, or spare (fed); for an accessory the part it comes with ("with EW-GEN-TRM-2") or "added" for the strip's own |

**Then the bridges**, under a heading row Bridges: one row per bridge group, its name, the terminals it joins
("1 to 4"), its pole count and its part. A bridge is never a wire, a wire record or a machine row (FR-TRM-007). A
placeholder part prints TBD and blocks release (FR-CHK-034).

The XLSX and the CSV hold the same rows, the heading rows as `heading` rows as the legend's do; a strip's rows carry
its tag in the hidden ID, so a filter by strip keeps them together.

### 6.2 Rail cut length

A rail is drawn by two points ([12](12-panel-layout.md) §3.2), so its cut length is its path length rounded up to the
project's cut increment, never what stands on it. The heading of a strip on a rail prints that length; the rail's
free length, `L` minus everything on it with its end brackets, stands in the rail's Properties. What does not fit is
EW-LAY-010 ([12](12-panel-layout.md) §4), never a longer rail: "-X1031 occupies 449.0 to 487.2 mm on rail 1, beyond
its 0.0 to 470.0 mm; shorten it or move it".

### 6.3 What the check says

| Rule | Says | Its fix |
|---|---|---|
| EW-MFG-020 | "-X1011 has no end plate after its last terminal PE" (warning) | Add an end plate… (the strip editor's Add accessory, kind End plate, after PE); its hint: "Or name the end plate in the terminal's part, under Accessories, so every strip gets it." |
| EW-MFG-021 | "The bridge BR1 of -X1021 (terminals 1 to 4) has no part" (warning, error in release) | Choose a part… (the part dialog of the bridge's Bridge part cell) |
| EW-LAY-010 | a rail too short for what stands on it, per item past its end (above) | — |
| EW-CORE-085 | "-X1011:N names the variant RD, which EW-GEN-TRM-1 does not hold" | Choose a variant… (Properties ▸ Parts ▸ Variant) |

A strip whose terminals have no part asks nothing of its end plate: the part decides whether one is needed.

## 9.3 Cut lists (FR-MFG-030)

`cut_list` (list kind `cut_list`) is everything the fitter cuts to length, in five groups, each piece once:

| Group | One row per | Piece length |
|---|---|---|
| Rails | rail part and piece length | the rail's path, rounded up to the cut increment |
| Ducts | duct part and piece length, then each accessory a duct names per unit (its cover) beside it | the duct's path, rounded up |
| Cables | cable | its length as entered, rounded up; a cable of its part's own length (pre-assembled) is not cut and is left out |
| Wires | wire part, else size and colour, and length | the wire list's length, never rounded again |
| Tubes | tube part, else material, size and colour, and length | the tube's length, rounded up |

A row prints the part number and description (or the wire's size and colour, the tube's material, size and colour),
For (the carriers by label, "Rail 1, Rail 2"; the cables by tag; the wires by number, a run compressed), the pieces,
the piece length in millimetres and the total in metres. Pieces without a length count on one row of their group
with "length missing"; a field wire is cut on site and left out. The cut increment is the project's
(`workshop.cut_increment`, [07](07-symbols-parts-macros.md) §2.5; Project properties ▸ Workshop ▸ Cut to), 5 mm
unless set; a piece is rounded once, here, and its total is the sum of the rounded pieces. Rails and ducts come from
the layout: a project without one lists none.

**The tube rows are the cut list's.** The tube cut list of [11](11-fluid-power.md) §8.2 is the Tubes group: one row
source, one list. A stored report block of kind `tube_cut_list` (a project made before) prints the cut list's Tubes
group under its own title, and Report block… no longer offers that kind.
