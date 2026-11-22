# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## 0.3.5: the fix round

Step 13 of [m3.5.md](m3.5.md) takes these rows in this order. Each sub-heading is the work of one lane: its rows
share a cause or the files they touch. A row that turns out to be a feature larger than a lane is designed first or
moves to its milestone with a reason.

### What the first production job found

A real drawing set was brought in from QElectroTech, made to follow an end user's drawing specification and doubled
for a second installation; almost everything that made it look like the source was done by scripts around the
program. The groups follow the share of the job's time that each cost, largest first; the job's findings stand
beside the lesson they belong to. What worked and is to be kept as it is: the project as plain line records with
stable ids; `ew fmt` and the integrity check, which named every malformed record; the SQL views, whose wire view
resolves through interruption points to the device at the far end; the cross-references, derived and right after
every rebuild; the headless script; a save that rewrites only what changed.

#### The source's typeface

- FT-1941 `ew-import-qet, ew-scene`: After FT-1765, 493 of about 4,800 imported texts of the pilot reference pair still stand apart from QET's own PDF (doc 14 §4.10): title-block cells 1 to 2 mm across, because QET's `hadjust` scales a too-wide text without a floor and never for its height while EW shrinks a cell's lines to fit its height and stops at 0.5 mm (`frames.rs`, `forms/text.rs`); rich-text free texts that QET's computer printed in Tahoma 0.7 to 1.6 mm higher (its ascent is 1.0 em; the face QET printed is its computer's substitution, which the file does not say); 94 texts of 8 pt in a lower band of some pages 0.5 mm higher, not yet examined. Fix round (2026-11-22): not measured again, so the counts stand: the private corpus folder holds the QET projects but not QET's own PDF of the pair, and the comparison of FT-1765 was a lane's script that was not kept; the pair and a kept script (doc 14 §4.10's counts) come first. The `hadjust` class needs QET's own fit, a text condensed across only and without a floor: a new `Overflow` variant of a form cell drawn by `forms/text.rs` and every output, a format addition decided first. Measured again in bundle H (2026-11-22, the pilot set's own PDF now in the private corpus; `ew import qet` then `ew verify-migration --json` is the comparison, no script needed), default typeface: 4,170 texts matched, 552 moved, 11 missing, 43 extra, 4 raster cells. The moved ones: 236 title-block texts (seven cells on each of the 34 pages: the four lines of the 5 pt notice cell, which EW shrinks to fit its height and QET does not, 2.2 to 2.7 mm off; right-aligned and centred cells 0.5 to 1.6 mm off by the default face's widths); 105 later lines of 6 pt multi-paragraph rich texts 0.5 mm higher (Qt's paragraph line pitch, not modelled); about 108 rich-text free texts in "Sans Serif", which QET's computer printed in Tahoma, 0.7 to 1.8 mm higher; about 100 others (cross-references and tables several mm off). Not fixed: the `hadjust` fit needs a horizontal glyph scale in the display list (`ShapedRun`) and its six outputs (PDF, SVG, DXF, CPU and GPU raster, the window), beside the variant; the two rich-text classes need Qt's metrics per paragraph and per substituted face, and a measurement with QET's face as the typeface (Liberation Sans is not on this machine) to tell face widths from placement.

#### Lists and frames


#### External points, earth and 0 V


#### Files and the text interface


#### Two drawing faults


### What a person at the window meets

From the owner's tests of 0.3.0's candidates and the persona runs.

#### Your own PLC and bus parts


#### The station view and the PLC grid


#### The window's finish

- FT-2006 `ew-ui`: Two faults of FT-1761 not seen again on the sample at 1920 × 1080: the first click on a command bar button right after a grid cell edit only ending the edit (Bus table, Wire: one click ends the edit and runs Wire), and the Properties header cut by its chips; each needs its repro (which grid, which selection) before a fix. Tried again in M3.5 step 13 (a window test on the sample's page 102, 1920 × 1080 and 1000 × 900, a cell of the Strips and of the Bus table opened by a double click and typed in, then one click on Wire): Wire arms at the first click; on the Bus table the cell's editor stays open beside the armed tool, which may be the fault seen. Tried once more in bundle F (2026-11-22, a window test not kept): the sample's page 105, the Bus table, the address cell of -104A1 DP opened by a double click and typed in, then one click on the ribbon's Wire: the typed value is committed (both rows read it), the cell's editor closes (three text fields become two), focus goes to the canvas and Wire is armed. Not fixed without the person's steps.

#### Properties and the title block's words


#### The layout page


#### What a new item takes


#### What the sample project's script met

Faults met while `fixtures/sample/sample.uiscript` was written; each has its steps in the script's own words.

- FT-2090 `fixtures, ew-api, ew, elektrawatt`: The sample's script sets the shield core of -103W1 to 2.5 mm² by hand (`props.set` of `connection/overrides.csa` after `cable.from_wires` on 103), and since FT-2055 the program proposes that size itself (a PE wire takes the highest rating at its ends: the 16 A breaker before -103M1), so the sample carries one EW-POT-016 info, "Remove that setting", which the Messages snapshots of `ew-ui` show. Dropping the line breaks the tests that find "the sample's one wire with a cross-section override" (`checks.rs`, `property_grid.rs`, `release_gate.rs`, `external.rs` of `ew-api`, `messages_scripts.rs`, `cli/revision.rs` and `cli.rs` of `ew`): they find the shield by its ends instead, in the same change.

#### What the tests met on the sample

Faults met while the tests moved from the reference project to the sample (M3.5, step 9); each test took another way round its fault, so a fix adds the test that goes straight.


#### The page's drawing


#### A command's words

- FT-2080 `ew-plc, ew-commands, ew-viewmodel`: The PLC exchange's notices (Apply's preview list: "-201U3: input start 0 → 8 (apply, taken from the file)", parked and removed objects) are keyed since FT-1819, but the object, field and state words inside them are still `ew_plc::object_words`, `field_words` and `State::word`, English in a Dutch window. The window has its own Dutch words for the same objects and fields (`plc_object_text`, `plc_field_text` over the `plc-object-*` and `plc-field-*` keys): two mappings of one thing. Make `ew-plc` name each object, field and state by its key (one place), the window and the notices render the key, and the command line its English.

#### Work on the window's thread

- FT-2081 `ew-api, ew-viewmodel`: File ▸ Import…'s "Add to my parts library" of an EDZ or a parts XML still reads the file on the window's thread: `eplan::import_into_library` parses it against the user's library inside the registry's write (`catalog::import_parts_into_library` reads the manufacturers and the held parts for the version bump while it parses). The other reads of FT-1763 run as jobs (`shell/read_jobs.rs`); this one needs the registry's part (manufacturers, held parts) taken first, the parse on the job's thread, and the write at its end.

### The placer and the router

From the drafting redesign (ADR-0034, `docs/design/06a-drafting.md`) and the pages since. Step 13 takes a row only
where a real page shows it; each row says whether one is known.

- FT-2085 `ew-drafting`: The placer keeps no room for the rings of the wires round a body (doc 06a §3.5, Rings): where a part wired round a card or a box stays behind it (kept in place, or held in its row by its other wires), the rings need their depth and a step between the body and it, and Arrange does not open that gap. **Page:** none yet: on the pages tried (the sample's former PLC page with cards in the older form, a strip under a card, with and without field devices under the strip) Arrange set such parts beside the body as hairpins, so no wire went round it.
- FT-1896 `ew-drafting`: After Arrange and route a supply page gains one text on a text and a small zigzag between two pins a step apart; two wires that get their first route add three crossings; a wire goes round a part where the part could stand in line; wires cross texts on a page with an older card. **Page:** seen on single pages, not named. Tried in the fix round's bundle G (2026-11-22): Arrange and route on the retired reference project's 101, 102, 103, 106 and 203 and on the sample's pages before and after their redraw (its former 104 has cards in the older form) left no text on a text and no wire across a text; the one small zigzag it left (the sample's former 201: two silencers wider than their ports' pitch, the second a step aside) is the stepping wire doc 06a §4.4 draws for followers wider than their pins' pitch, as designed. It waits for a page that shows the rest.
- FT-2083 `ew-commands, ew-viewmodel`: A card of the older two-row form is drawn in one row by Draw the card in one row (FT-1897), but a terminal of a channel that then faces the other way keeps its wires on the pin it had: the wire to the card belongs on the terminal's pin that faces the card (doc 06a §4.8). A command that swaps the two wires of such terminals for a strip (or for the terminals of one card) would take that hand work away; the redraw also does not split a row wider than the drawing area into boxes as Draw on a page does.
- FT-1837 `ew-drafting`: Tubes from a wide valve terminal with upward ports to a cylinder below are long, because every clear way is. **Page:** known, the persona runs' fluid pages. Not reproduced (2026-11-22): on a page built after it (a terminal of four valves 32 steps wide, its ports up, a cylinder under its third valve) Route goes round the terminal, as it must, and Arrange and route sets the cylinder over its valve with two straight tubes (rule F1); the persona runs' pages, which are not in the repository, are needed to see what held their cylinders below. Tried again in the fix round's bundle G (2026-11-22) on the sample's 201: with -201M2 moved 30 steps down, under the valve terminal, Route lays its tubes round the terminal (as it must) and Arrange and route sets it over its valve again with short tubes; moved 22 steps it lands inside the terminal's outline, joins the terminal's unit and stays, its tubes round the terminal (a part set inside an outline moves with it, doc 06a §4.2), which is not this row. It waits for the persona runs' pages.
- FT-2010 `ew-commands, ew-drafting`: After a grid change a wire to a pin that its symbol's own spacing keeps off the new grid finds no way, though it routed before. The off-grid pin is not the cause (a wire to one nearly in line now steps square, 2026-11-22): the fit keeps gaps in millimetres, and on the coarser grid there is no grid line a step from both a body and its neighbour (-104A1's cards stand 2.3 steps from the drawing area's edge, with their supply pins on that side; -201X1's corner stands 1 mm beside -201X2's pin's line). The fit would have to keep the room a pin's wire needs on the new grid (doc 06 §8 item 3), or the search to pass a body nearer than a step at a pin. **Page:** known, the sample's 104 (three wires of -104A1's cards) and 201 (-201K1:3 to -201X2:1) changed to 4 mm. Reproduced in the fix round's bundle G (2026-11-22), two causes the fit cannot see today: on 104 the cards' left edges stand 2.3 steps from the drawing area's edge with their supply pins facing it, and the search keeps a step clear of both the edge and the body, so no grid line is left between; the fit has no item for the area's edge, and `page.set_grid` neither carries the drawing area nor can `ew-commands` reach the frame's geometry (`ew-scene`'s `FrameGeometry`). On 201 -201K1:3 and -201X2:1 stood in line, one straight wire; the fit sets each first pin on the grid, so they end five ticks apart, the wire becomes a search, and -201X1's corner, a quarter step beside -201X2's pin line, closes the only column. The fix needs the drawing area handed to the fit (a member of `page.set_grid` its callers fill, or the frame read below `ew-scene`) with a grid line kept a step from both a body and the edge, and a rule for two wired pins that stood in line (keep them so, or keep a step of room beside the second pin's line).

## M3.5: what its own steps leave

Rows that the hygiene steps of [m3.5.md](m3.5.md) take, or that they left.

- FT-1775 `xtask, ew-commands, ew-viewmodel`: The test builds and a plain `cargo build` (the headless pictures, `ew`) compile every crate above `ew-commands` twice: the tests with the test-only `testkit` features of `ew-commands` and `ew-viewmodel` (`[[uniform_test_features]]`), the plain build without; `ew-commands`' `testkit` turns on `ew-model`'s, so a lower crate tested alone builds twice too. One flavour would save the second compile of the heavy crates (not measured).

## M4

Rows for M4, manufacturing and the 2D/3D panel ([ROADMAP.md](../../ROADMAP.md)). FT-1814 and FT-1815 the owner asked
for at his test of 0.3.0's fourth candidate; each starts with a design from the user's side (who changes it, when,
what he sees while he does it) before anything is built.

- FT-1937 `ew-scene, ew-viewmodel, ew-api, ew-commands, ew-reports`: **First step of M4 (owner, 2026-11-22): make a change cheaper to make.** Measured on seven lanes of the 0.3.5 fix round: a lane had about 290,000 tokens in context at its first edit, 60% of the source it read came from files of 30 to 100 KB, and one idea often took many files (the program's own words in a Dutch list: 21 files, every list kind builds its cells itself; a fix commit touches 7 files at the median, 12 or more in a quarter of them). Three parts: split the large files that are read most along the lines they are read (`ew-scene/src/page.rs`, `ew-viewmodel/src/shell/editing.rs`, `ew-api/src/output/view.rs`, `ew-commands/src/commands/paste.rs`, `ew-viewmodel/src/shell/part_dialog.rs`, `ew-viewmodel/src/tools/black_box.rs`, and the list the 0.3.5 review leaves); give what is repeated per list kind and per command notice one place; and take one standard small fix as the yardstick, measured before and after. No behaviour changes; the yardstick is not a test.
- FT-1814 `ew-api, ew-viewmodel, ew-ui, ew-scene`: The title block cannot be changed in the window: its layout, extra fields of the person's own, and pages that have their own title block (a cover page, a divider). Design first: an editor for a title block that a person who has never seen a form editor can use, what a template carries of it, and how a running project takes a changed one.
- FT-1815 `ew-api, ew-viewmodel, ew-ui, ew-scene`: The frame's rulers (the numbers along the top, the letters along the left) cannot be changed: how many columns and rows a page has, numbers or letters on either side, where counting starts. Design first, together with FT-1814: cross-references and the page's grid depend on the columns and rows.
- FT-1847 `ew-reports` (WP-S5.07): `tubes.json` of the production package, with the tube rows of `cutlists.csv`, comes with the package (doc 13 §8.1); the tube cut list's allowance and order increment are M5's (FT-1732).
- FT-1843 `ew-api, ew-viewmodel`: A panel layout has no door and no front panel: a device mounted in the door (a pushbutton, a pilot lamp, a display) can only be dragged onto a rail, where it lands as "unknown size". Design the door as a surface of the enclosure that holds such devices with their cut-outs; new development of the panel.
- FT-1904 `ew-schema, ew-api, ew-commands, ew-scene`: A device with several bus interfaces is one box per interface on a bus page. One box needs a box connection point that names its function (a format change) or a generated symbol per part: design first. Moved here from the fix round of 0.3.5 because it changes the format (owner, 2026-11-20).
- FT-1905 `ew-model, ew-checks, ew-scene`: Three things a draughtsman's drawing shows and the model cannot hold: a wire that arrives at a pin and bends into the next one by a slanted corner (EW-CORE-068 allows a shared beginning only at a right angle); a text that reads downward (the program turns it to read upward); a length that is "about". Each is a format change; the first production job did without them (owner, 2026-11-20).
- FT-1771 `ew-viewmodel, ew-ui`: An editor for a part's drawn front view (`mech.front_2d`): the part editor's Size and mounting section edits only the rectangle the layout draws; new development of the panel.

## M5

Rows that a work package of [m5.md](m5.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.

- FT-1760 `ew-commands` (WP-S1.35): N-FL-4 makes a `drives` link from a valve's working port's own net only, so a cylinder behind a throttle gets none by itself (the checks follow the path through it since M3's review).
- FT-1732 `content, ew-reports` (WP-S1.36): The tube cut list has no cut allowance and no order increment, and the tube labels no marker part: none has part data yet (WP-S1.25).
- FT-1783 `ew-viewmodel, ew-ui` (WP-S1.36): A tube's size is chosen in the Tube tool's bar, beside its line kind; it takes the project's default now.
- FT-1801 `ew-viewmodel` (WP-S1.36): A tube drawn in a project without a default tube stops for the project's fluid settings; offer the size where the tube is drawn.
- FT-1769 `ew-api, ew-viewmodel` (WP-S2.21): A valve terminal part that comes in several sizes is not asked for its size when it is placed: the number of positions is the part's, and a part that names its sizes changes the part record, which the format grows only additively (decide how a part names them first).
- FT-1770 `ew-viewmodel, ew-ui` (WP-S2.21): The part editor has no controls for a valve terminal's kind (bus node or multipin) or for its pin map (which plug pin drives which solenoid); the starter parts carry them.
- FT-1773 `ew-ui` (WP-S2.21): A wiring test of the multipin valve terminal's plug in the window: a PLC output wired to a plug pin by clicks drives the solenoid at session level only; no window test does it by clicks.
- FT-1784 `ew-viewmodel` (WP-S2.21): A Blanking plate row in the device browser of a valve terminal's cell, beside the valves made for a terminal; the cell's menu has Blanking plate at position.
- FT-1793 `ew-api, ew-commands` (WP-S2.21): A valve stays undrawn when a valve terminal's positions are filled through `ew apply` without the symbols (`copies`); the command could find the symbols itself, or say so. Draw its valves in the valve terminal draws them.

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
