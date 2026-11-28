# Manufacturing outputs: the workshop lists

A chapter of [Manufacturing outputs](13-manufacturing-outputs.md): what the panel builder hands his fitter beside
the wire list. Where every device goes and per strip what to mount in which order (§6), for the cabinet what to cut
(§9.3), and what to order of the parts a wire consumes (§3.6). The parts these lists print, their accessories and
variants and the project's workshop stock are [07](07-symbols-parts-macros.md) §2.5; the wire records, the end treatments and the markers they
count are §2 to §5 of [13](13-manufacturing-outputs.md).

**Where they are in the window.** The Output tab's Workshop group holds Assembly lists, Cut list and Consumables. Each
opens the project's list page of that kind; a project without one opens Report block… on that kind, whose Create
makes it (one undo step, [09](09-reports-forms-output.md) §3.8). A list page prints on the set's frame like every
list, follows the project when it changes, and exports as a file by **Export list…** (`lists.export`), in its page's
menu and in Output ▸ Workshop (greyed with its reason where no list page is shown): the columns its page prints, a
`.csv` name the CSV in English, any other the Excel file in the project's language, for people (no `id` column, no
hidden sheet of the round trip), by the writer of the package (`list_output`); the file dialog asks before a file is
replaced. `ew export report --kind assembly_list | cut_list | consumables --format csv | xlsx` writes every column. None of the three needs a layout: without one the
assembly list names no rail and the cut list holds no rails or ducts. Their findings stand in the Messages pane under
Check ▸ Check project (§6.3).

---

## 3.6 Consumables summary (FR-MFG-049)

`consumables` (list kind `consumables`, rows of `ew-reports` over `ew-derive`) is what to order of what the wires
consume, separate from the parts list, which counts devices and their accessories. It reads, of the wire design and
nothing else:

- each wire record of the project's cabinet wires (`wire_record_set`, §2.5): its `csa`, `colour`, `wire_part`,
  `length` (the value and whether there is one) and whether it is a field wire;
- of each end the shop makes, its `treatment`: the ferrule's or lug's part, a twin ferrule's part once for its two
  wires, or none (bare, a connector, undetermined);
- of each label row of the project's label sets (`label_rows`, §5.3), its marker part.

Three groups, in this order:

| Group | One row per | Quantity |
|---|---|---|
| Wire | wire part, else size and colour | the summed lengths in metres, exact to the millimetre; "0.45 and 6 wires without a length" while some have none, "length missing for 7 wires" while none has one |
| Ferrules and lugs | ferrule or lug part | the ends it is crimped on, a twin ferrule counted once (by the wire with the lower ID); ends left without a part count under their size and form as "no ferrule found", "no ring lug found" |
| Markers | marker part | the labels it prints; For names its label set, or each set with its labels where one part marks several ("Wire markers 82, Tube markers 14"); a set without a part is a row of its own |

A field wire (an end outside the cabinet) counts the ferrule and the markers of the end the shop makes, but not its
metres: it is cut on site. A cable's core is cut with its cable: its ends count, its metres are the cable's. The row prints the part's number and description, or the wire's size and colour as the
lists print them; the For cell says what it serves ("12 wires", "2.5 mm²", "Wire markers"). Nothing is rounded here:
a wire's length is the wire list's, and an order quantity belongs to the order list. A length along a way that no
longer holds ([12-3d](12-3d-routing-thermal.md) §8.5) is summed as the wire list prints it, and the row's metres print
after "≈" when one of them is such a length, beside the wires without a length too ("≈ 0.45 and 6 wires without a
length"). Whether a length is exact is asked in one place, the wire record's length, which the wire list's "≈" reads
as well.

## 6. Assembly lists

### 6.1 Content (FR-MFG-016)

`assembly_list` (list kind `assembly_list`) prints where every device goes and, per strip, the order the fitter
mounts it in. Its rows come from `ew-derive`: `rail_assembly(rail)` and `surface_items(surface)` (where a device
stands, from the layout's own positions: `rail_occupancy` with the end brackets a strip takes, `placement_geometry`
and `surface_features`; [12](12-panel-layout.md) §2.6), `unplaced(space)`, and `assembly_rows(strip)`, which reads
`strip_layout` and `part_accessories` ([07](07-symbols-parts-macros.md) §2.5, [12](12-panel-layout.md) §3.4) and the
strip's bridges (`strip_bridges`). No position is computed a second time: the list, the page and the hole table read
the same queries.

**Every position is measured as the hole table measures** ([13-drilling](13-drilling.md) §9.2): from the surface's
datum corner, and along a rail from **its start, the end nearest that corner** (the left end of a rail that runs across
under a left corner, the lower end of one that runs up under a bottom corner). The rail's heading says which end that
is, and a strip's heading and a device's place on the rework list (§3.5 of [13](13-manufacturing-outputs.md)) measure
from the same end.

**The order of the blocks.** For each surface a layout page shows, in the order of its first such page:

1. **each rail** in its surface's legend order, as a block of its own: a heading with the rail, its surface and page,
   its cut length, where it lies and which end its positions start from ("Rail 1 · Mounting plate, page 902 · cut to
   470 mm · its centre line at Y 580 mm, from X 25 to 495 mm, measured from the bottom-left corner · positions from
   its left end"), a row with the rail's part, then **everything on it in mounting order from its start**: each device
   with its tag, its part, its width and where it stands (from–to in mm, "48.2–102.2"), a strip as one row that names
   its own block, and a Gap row for every space between two items, before the first ("from the rail's start") and
   after the last ("to the rail's end");
2. **the blocks of the strips on that rail**, in the order they stand on it;
3. **the devices placed on the surface without a rail** (in the door, on the plate), under a heading with the surface,
   its page, how it is seen and its datum corner: each with its tag, its part, its width, its centre ("X 270, Y 480")
   and the holes it sits in by the hole table's numbers ("in hole H3"), in the order of their tags.

Then the strips on no rail, in the order of their first segment's page, then by tag; then, per cabinet, **the devices
not placed** (`unplaced`) under "Not placed in the layout of +CAB", each with "place missing" in its Position cell. A
device without a width says "width missing", and its Position the centre of the placeholder the page draws ("centre
at 437.2"). A block that chooses its strips (Report block…) lists those strips and the rails they stand on, and no
surface or cabinet block.

**A strip's block: what he reads first** is the strip's heading row: its tag, where it goes and how long it is, as one
text with arguments ("-X1011 · on rail 1 from 0 to 48.2 mm · rail cut to 470 mm · 5 terminals · 48.2 mm with its end
brackets"). A strip on no rail says "on no rail"; a strip turned on its rail says that its first terminal stands at
the right. A strip stands on its rail as a whole: the heading names its placement on the rail that comes first in
the legend's order, and a heading per piece comes with placing a strip in pieces
([12](12-placement-pages.md) §2.3, `segment`).

**Then the mounting order**, one row per item from the rail's left end: the end bracket before it (a strip that abuts
the one before it starts against that strip's bracket and says so in its heading), the terminals by `seq`, each
accessory at its place (an end plate after the last terminal whose part names it, the strip's own after the
terminal it follows), the end bracket after it.

| Column | Holds |
|---|---|
| Pos | 1, 2, … in mounting order |
| Item | Rail, Device, Strip, Gap; in a strip's block End bracket, Terminal, End plate, Partition plate, Marker carrier, Cover (the accessory's kind), Accessory |
| Device | a device's or a strip's tag in a rail's or a surface's block |
| Terminal | the terminal's designation; a block's levels print once |
| Part number, Description | the part's, the variant's where it gives its own (07 §2.5) |
| Colour | the technical value `colour`: the device's own, else its variant's, else its part's |
| Width | the item's width in millimetres; a part without one prints "width missing" and the heading's length says "at least" |
| Position mm | on a rail from–to from its start ("0–48.2"), on a surface the centre ("X 137.5, Y 622.5"), "place missing" for a device not placed |
| Marker | the terminal marker's text (§5.3) |
| Note | spare, or spare (fed); for an accessory the part it comes with ("with EW-GEN-TRM-2") or "added" for the strip's own; a strip's row in a rail's block names its block; a device on a surface its holes; a gap at a rail's start or end says so |
**Then the bridges**, under a heading row Bridges: one row per bridge group, its name, the terminals it joins
("1 to 4"), its pole count and its part. A bridge is never a wire, a wire record or a machine row (FR-TRM-007). A
placeholder part prints TBD and blocks release (FR-CHK-034).

The XLSX and the CSV hold the same rows, the heading rows as `heading` rows as the legend's do; a strip's rows carry
the strip in their row ID, so a filter by strip keeps them together.

### 6.2 Rail cut length

A rail is drawn by two points ([12](12-panel-layout.md) §3.2), so its cut length is its path length rounded up to the
project's cut increment, never what stands on it. The heading of a strip on a rail prints that length; the rail's
free length, `L` minus everything on it with its end brackets, stands in the rail's Properties. What does not fit is
EW-LAY-010 ([12](12-panel-layout.md) §4), never a longer rail: "-X1031 occupies 449.0 to 487.2 mm on rail 1, beyond
its 0.0 to 470.0 mm; shorten it or move it".

### 6.3 What the check says

| Rule | Says | Its fix |
|---|---|---|
| EW-MFG-020 | "-X1011 has no end plate after its last terminal PE" (warning) | Where a terminal's part names an end plate: Add the end plate EW-GEN-EP-1 after PE (the strip's own accessory after its last terminal). Its hint: add one in the strip editor (Add accessory, kind End plate), "or name the end plate in the terminal's part, under Accessories, so every strip gets it." |
| EW-MFG-021 | "The bridge BR1 of -X1021 (terminals 1 to 4) has no part" (warning, in release too: a project made before a bridge could have a part releases as it did) | No quick fix: its hint points to the Bridge part cell of the strip editor, whose part dialog chooses it |
| EW-LAY-010 | a rail too short for what stands on it, per item past its end (above; an error) | its quick fix, move to a carrier it fits |
| EW-CORE-085 | "-X1011:N names the variant RD, which EW-GEN-TRM-1 does not hold" | Use the part's own number (then a variant in Properties ▸ Parts ▸ Variant) |

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
with "length missing"; a field wire is cut on site and left out. A wire's length along a way that no longer holds
([12-3d](12-3d-routing-thermal.md) §8.5) prints after "≈", piece and total alike; wires of one key and one length share
a row, which prints "≈" when one of them is such a length. The cut increment is the project's
(`workshop.cut_increment`, [07](07-symbols-parts-macros.md) §2.5; Project properties ▸ Workshop ▸ Cut to), 5 mm
unless set; a piece is rounded once, here, and its total is the sum of the rounded pieces. A cable's core is cut with
its cable and is no wire row; a duct's cover is one piece of the duct's length per duct. Rails and ducts come from
the layout: a project without one lists none.

**The tube rows are the cut list's.** The tube cut list of [11](11-fluid-power.md) §8.2 is the Tubes group: one row
source, one list. A stored report block of kind `tube_cut_list` (a project made before) prints the cut list's Tubes
group under its own title, and Report block… no longer offers that kind.
