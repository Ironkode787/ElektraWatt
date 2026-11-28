# Manufacturing outputs: holes and the drawings for drilling

A chapter of [Manufacturing outputs](13-manufacturing-outputs.md): what the panel builder drills and cuts in his
cabinet's surfaces, as a list and as drawings. The holes themselves (what gives them, their sizes, their numbers) are
[12-3d](12-3d-routing-thermal.md) §10; the surfaces are [12](12-placement-pages.md) §2.8.

---

## 9. Drilling data and hole tables

The holes and cut-outs of every surface of a cabinet ([12-3d](12-3d-routing-thermal.md) §10.4, `surface_features`)
reach the workshop three ways, all from that one query, so they cannot disagree: the hole table, a template he prints
at full size and lays on the surface, and a DXF for a machine. His shop makes holes both by hand and on a machine, so
both drawings are made.

**Where they are in the window.** The Output tab's Workshop group holds, after Wire list, Rework list…, Labels…,
Assembly lists, Cut list and Consumables:

- **Hole tables** (`lists.holes`) opens the project's hole table page; a project without one opens Report block… on
  that kind, whose Create makes it (one undo step), as Wire list and the workshop lists do.
- **Drilling…** (`drilling.export`) is the form dialog "Export the drawings for drilling": one row per surface that has
  holes (its words, its page, its hole count, the sheets its template takes and the names of its two files,
  "+CAB Door template.pdf, +CAB Door.dxf"), each ticked; Template to print at full
  size (PDF) and DXF for a machine, both ticked; Paper, A4 or A3; the folder
  (Drilling, beside the project, as Labels… proposes its own). A line under the rows names the findings of the holes
  ("1 hole is closer to another than 5 mm") with Show. Export writes the files as a job; its toast says which files
  are which surface's: "Wrote 6 files to Drilling: a template and a DXF for each of Mounting plate, Door, Bottom." with
  Open folder. A surface without holes is not offered; with none at all the button is greyed: "No
  surface of this project has holes yet".

The command line: `ew export drilling --dir <folder> [--surface <words or ID>]… [--template a4|a3] [--no-dxf]
[--no-template]`; `ew export report --kind hole_table --format csv|xlsx`.

### 9.1 The drawings for drilling (FR-MFG-028)

**The template**, `<location> <surface> template.pdf` ("+CAB Door template.pdf", in the project's language), is the
surface at full size (1:1 on paper), seen from the side its page shows it, over as many sheets as it needs; every word
on its sheets is in the project's language:

| Item | How |
|---|---|
| What it covers | the surface from **the corner the hole table measures from** (the surface's datum corner, §9.2) to 20 mm past the farthest hole, within the sheet; the two edges at that corner are drawn thick with their words ("Left edge of the door", "Bottom edge of the door"), so the template is laid against the edges every number is measured from without measuring, its first sheet at the datum corner. The top and the bottom, seen from another side than the fitter stands at, name their edges as he stands at the door: "Left edge of the bottom, at the cabinet's left as you face its door", "Back edge of the bottom, away from the door", "Door's edge of the top" |
| The sheets | each paper sheet less 10 mm all round and a 12 mm band at its foot; the orientation that needs the fewer sheets (portrait on a tie); neighbouring sheets overlap by 10 mm, or by the largest hole's size to the millimetre where that is more, so that a hole on a join stands whole on one of the two sheets: each sheet prints its neighbours' overlap on its right and its top, tinted, and is cut along its dashed left and bottom edges; a sheet that holds no part of a hole and neither of the two edges is not printed (the map shows it empty); the printed sheets numbered row by row from the corner the template lies in |
| Join marks | a cross at every corner where four sheets meet, on each sheet that shows it, so two sheets laid together put their crosses on each other |
| A hole | its circle or rectangle at true size in a 0.25 mm line, a centre cross 4 mm past its edge (to centre-punch through), its number (H3), how it is made (Ø 22.3) and its owner (-102S1) in 2.5 mm text beside it |
| Also | the edge margin thin dashed, a door's hinge ticked, the datum's "0" where it falls on the sheet |
| The band | the project, the surface and how it is seen ("Door, seen from outside"), the revision, the sheet ("Sheet 4 of 5, row 2, column 1") and where it lies ("0–180 mm from the left edge, 255–510 mm from the top edge"), which corner the hole table measures from ("Hole table: X and Y from the bottom-left corner, marked 0"), a small map of the sheets with this one filled, and a 100 mm scale bar with 10 mm ticks: "100 mm: print at actual size (100 %) and measure this bar" |
| Numbers | a hole's size and the sheet's place print with the decimal sign the project's lists print ("Ø 22,3" in a Dutch project); the DXF keeps the point, which a machine reads |
| Left out | device outlines, labels, carriers, dimensions, the frame and the title block: it is a template for holes, not an assembly drawing |

**The DXF**, `<location> <surface>.dxf`, is written by the program's own writer (`ew-export`'s `dxf` module, the
version, the header, the handles and the number format of its page DXFs), from a typed list of entities per layer that
`ew-api` builds from `surface_features`; the writer knows no record:

| Item | How |
|---|---|
| Units and origin | millimetres (`$INSUNITS` 4); the origin at the surface's datum corner; x right and y up as the surface's page shows it (a door seen from outside), so nothing is mirrored |
| `OUTLINE` | the surface's sheet as a closed `LWPOLYLINE` |
| `MARGIN` | the edge margin's inner line as a closed `LWPOLYLINE` |
| `HOLE_<d>` | a hole drilled through, at its drill: `CIRCLE` of that diameter (`HOLE_4.5`) |
| `TAP_<M>` | a tapped fixing: `CIRCLE` at its tap drill (`TAP_M5`, Ø 4.2) |
| `CUT_<d>` | a round cut-out or a gland hole: `CIRCLE` (`CUT_22.3`) |
| `CUT_RECT` | a rectangle: a closed `LWPOLYLINE`, its corner radius as bulges |
| `NUMBERS` | each hole's number as `TEXT`, 3 mm high, beside it |
| `DATUM` | a cross at the origin and a `TEXT` that names the corner and the side: "0,0: bottom-left corner of the door, seen from outside" (the project's language) |
| Order | by layer in this table's order, then by hole number; coordinates from nanometres to 0.001 mm |
| Left out | devices, labels, carriers, the frame, the title block and dimensions |

A layer per way of making a hole lets a machine's own software take a tool per layer. A named machine's format (a
punching machine's own file) is not written: no specification of one is read ([the later backlog](../plan/backlog-later.md)).

### 9.2 The hole table (FR-MFG-029)

`hole_table` (list kind `hole_table`, rows of `ew-reports` over `surface_features`) lists, surface by surface in the
page order of their layout pages, every hole in its number's order:

| Column | Cell |
|---|---|
| No. | H1 … Hn, the drilling order ([12-3d](12-3d-routing-thermal.md) §10.4) |
| X mm, Y mm | the hole's centre from the surface's datum, to the 0.1 mm, half up, a whole millimetre without its ".0" (a rectangle: its centre) |
| Hole | how it is made: "Ø 22.3", "Ø 4.2, tap M5", "Ø 4.5", "92 × 92, R 2" |
| What | the kind and its size: "Cut-out 22 mm", "Fixing M5", "Gland M20", "Hole Ø 6", "Rectangle 92 × 92" |
| For | its owners: device tags, "Rail 1", "Duct 1", a cable's tag for its gland ("-103W1"), a purpose typed by hand; two owners joined ("-101F1, -102F1") |

Each surface opens with a heading row that names its size (width × height) and says where its numbers are measured
from, the surface's corner or the project's ([12](12-placement-pages.md) §2.8): "Door 600 × 800 mm, page 904, seen
from outside, measured from the bottom-left corner"; "Bottom 600 × 250 mm, page 905, seen from below; the door's edge
at the top, measured from the bottom-left corner". The list prints in the project's language on the set's frame, follows
the project, and exports from its page's menu as CSV and XLSX; the CSV and XLSX lead with a Surface column. A surface
without holes is not listed.

The cut lists of §9.3 are [the workshop lists](13-workshop-lists.md) §9.3.
