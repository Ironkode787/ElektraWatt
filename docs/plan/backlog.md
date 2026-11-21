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

- FT-1941 `ew-import-qet, ew-scene`: After FT-1765, 493 of about 4,800 imported texts of the pilot reference pair still stand apart from QET's own PDF (doc 14 §4.10): title-block cells 1 to 2 mm across, because QET's `hadjust` scales a too-wide text without a floor and never for its height while EW shrinks a cell's lines to fit its height and stops at 0.5 mm (`frames.rs`, `forms/text.rs`); rich-text free texts that QET's computer printed in Tahoma 0.7 to 1.6 mm higher (its ascent is 1.0 em; the face QET printed is its computer's substitution, which the file does not say); 94 texts of 8 pt in a lower band of some pages 0.5 mm higher, not yet examined.

#### Lists and frames

- FT-1995 `ew-reports`: A list or a box whose rows do not derive prints its reason in English, with a family or kind name in backquotes, also in a Dutch project ("Not derived: query cycle in `page_order`"; `NotDerived`'s `Display`, `paginate.rs`, `view.rs` `boxed_table`).
- FT-1934 `content`: The starter library's parts and symbols carry their descriptions in English only (134 texts), so a Dutch project's parts list prints "Feed-through terminal" beside Dutch headings and units. Give the generic parts and symbols a Dutch description; the lists already print the project's language first.

#### External points, earth and 0 V

- FT-1981 `ew-api, ew-import-catalog`: The units of technical data are listed by hand three times beside `Unit` (the typed units, a parts catalogue's units, the derive's symbols); the catalogue's list lacks `mm`, `piece` and `set`. One list from `Unit`.
- FT-1933 `ew-derive`: A protective-earth potential that only an earth symbol names takes that symbol's pin name: an imported earth point's pin is `1`, so the potential is called `1` in the potential list and the SQL view. Call such a potential PE.

#### Files and the text interface

- FT-1985 `ew-import-eplan`: A multi-level terminal that the report importer folds (EW-IMP-207) lands every level's connections on level 1; since FT-1878 its listed levels have connection points of their own (doc 08 §6.3), so each level's connections can land on its own.

#### Two drawing faults

- FT-1990 `ew-viewmodel`: A connection point moved alone in the symbol editor, or given another edge in Properties, leaves the text that writes its number where it stood; since FT-1871 every new point has one.

### What a person at the window meets

From the owner's tests of 0.3.0's candidates and the persona runs.

#### Your own PLC and bus parts

- FT-1970 `ew-api, ew-derive`: A card of two channel kinds that go the same way (inputs with analogue inputs, outputs with analogue outputs) is still two parts; a card of inputs and outputs is one (FT-1766, doc 10 §2.6). A module has one start per direction, so the second kind's place in its direction's range (after the first's bits, a word aligned) and the profile's cursor per class must be settled first.

#### The station view and the PLC grid

- FT-2000 `docs`: The guide's picture of a rack (`how-to/images/plc-rack.png`) is older than the station view: it lacks the Function text column and shows the To do under the channels; paint it again from the view.

#### The window's finish

- FT-2005 `ew-ui`: The pin list of a device box sorts itself again when a pin's edge changes (FT-1810's third fault): the rows should keep their places while the same pins are edited; a test through the visual driver did not show the jump, so it needs its repro first.
- FT-2006 `ew-ui`: Two faults of FT-1761 not seen again on the sample at 1920 × 1080: the first click on a command bar button right after a grid cell edit only ending the edit (Bus table, Wire: one click ends the edit and runs Wire), and the Properties header cut by its chips; each needs its repro (which grid, which selection) before a fix.
- FT-1965 `ew-viewmodel, ew-ui`: A form table that adds rows names its cells and remove buttons for a screen reader as value sets ("`<column>`, value set 2", "Remove value set 6"), Copy pages…'s rename table too (`value_table.rs`); name them by the table's own row word, as its add button is named since FT-1862.

#### Properties and the title block's words

- FT-1803 `ew-viewmodel, ew-api, ew-scene`: Project properties name their rows otherwise than the title block's cells (Project reference and Project, Designer and Author), Document title prints in no cell of the program's frame, and the Checker row does not feed the Checked cell (which prints who approves the current revision); only Title block fields… says which cell prints which. Name the rows as the cells, and decide whether Checker feeds Checked.
- FT-1779 `ew-viewmodel, ew-ui`: A wire's Potential row in Properties should open Define potential, as the wire's menu does; today only the row's "…" menu offers it.
- FT-1808 `ew-viewmodel`: Placing a part that has no symbol arms the Black box tool without saying that the part is drawn as a box.
- FT-1811 `ew-ui, ew-api`: A pin's place typed between two steps of the pin grid is taken to the next step without a word; a copy of a starter part keeps the starter's part number without saying so.
- FT-1812 `ew-viewmodel`: F1 on a selected connection point in the symbol editor opens the outline's section although the guide has a section for a connection point.
- FT-1813 `ew-viewmodel`: The Place tool shows "-?X?:1" for a new strip until the pointer moves; the project's Kind row shows a dash.
- FT-1809 `docs/book`: A property's help page often repeats the row's label (the project's Control voltage); say what the program does with the value, its unit and an example.
- FT-1936 `ew-viewmodel`: A legend's Properties prints its Rectangle row as the stored value in nanometres (raw JSON); show its place and size in millimetres, as a drawn box's rows do.

#### The layout page

- FT-2025 `ew-api`: A new layout page at its fitting scale on an A4 frame gets no legend with its view: the layout, centred (FT-1768), leaves 59 mm beside the starter enclosure at 1:5, under Add legend's 60 mm, so Enclosure… and the Surface row add none and Add legend refuses for room (the sample's script sets 902's datum by hand to keep its legend). Goes with the legend's redesign (FT-1920).

#### What a new item takes

- FT-1834 `ew-derive, ew-viewmodel`: A new wire takes the control size, 0.75 mm², unless its potential names another circuit class, so a 16 A mains wire gets it. Propose the cross-section from the circuit.
- FT-1791 `ew-viewmodel, ew-api`: The spacing between the terminals of a row (`shell/terminals.rs`) is kept by the open project only: keep it between sessions (a user setting, or a project setting, which is a format change).
- FT-1840 `content`: There is no proximity or reed sensor among the starter parts: a three-wire PNP sensor with its symbol.

#### What the sample project's script met

Faults met while `fixtures/sample/sample.uiscript` was written; each has its steps in the script's own words.

- FT-1900 `ew-api`: A project made by `file.new` without the template's pages takes its first page's id from the clock (`new_project.rs`: the scratch session runs with default options), so two runs of one script differ. Take the id from the session's id source.
- FT-1901 `ew-commands, ew-viewmodel`: `supply.place` with a location written with its sign (`+UP`) prints `++UP` and makes a structure node of that name. Take the name with or without its sign, as the other commands do.
- FT-1902 `ew-commands`: `bus.cable` finds a plug only by its record id; a part number, which every other part parameter takes, is refused with "choose one from the list".
- FT-1903 `ew-checks`: A breaker with a phase pole and a neutral pole (1P+N) wired with L and N is reported as opening the neutral without the phase (EW-POT-013, an error): the rule does not see that both poles belong to one device and switch together.
- FT-1907 `ew-viewmodel, ew-commands`: Draw on a page from the station view, for a station whose CPU is not placed yet, puts the CPU's box below the frame, off the sheet. Place it inside the drawing area as the cards are.

#### What the tests met on the sample

Faults met while the tests moved from the reference project to the sample (M3.5, step 9); each test took another way round its fault, so a fix adds the test that goes straight.

- FT-1910 `ew-ui`: Supply comes in here… on the sample's page 102, with the incoming 24 V mark selected and Enter pressed: the potentials panel's Conductor list and its Voltage field have no accessible name (`canvas.rs`). The same on page 103's mark names them.
- FT-1911 `ew-viewmodel, ew-ui`: A tap on a mark's own cross-reference text does not follow it while that mark is selected (the sample's page 103, the incoming PE mark, its text 101-C10); with nothing selected it does.
- FT-1912 `ew-viewmodel`: A wire selected on the page does not always mark its row in Connections: on the sample's page 104 the wire numbered 101.16 does not, on page 102 the wire 101.15 does. The cause is not known.
- FT-1913 `ew-viewmodel`: In Dutch the Devices navigator prints "3P- manualswitch" and "3P- mcb" for the sample's -101Q1 and -103F1: a kind whose generic part has no Dutch words gets a stray "- " after its pole count, where it should stay as written, as "2P mcb" does. The exception list `WRONG_KIND_WORDS` of `tests/it/dutch_words.rs` goes with the fix.
- FT-1914 `ew-api, ew-derive`: `page.describe` of the sample's page 102 or 103, asked twice from page 101, computes again: the memo's totals move (six more on 102), where the second answer should come from the cache. Both pages draw -102Q1's coil or contacts; the pages 104, 105, 201 and 902 hold. Its cost is not measured.
- FT-1915 `ew-viewmodel`: Properties of the sample's panel page 902, with Advanced on: the page's Form row is empty, where the frame's name belongs. The test of 902's rows in `tests/it/props_follow.rs` pins it empty and turns with the fix.
- FT-1916 `ew-viewmodel`: Five rows of Properties have no explanation in either language: the project's Source, Translations and Frames, and Kind and Part of a function's connection point closure. The project's Frames row also shows a form's raw ID instead of its name. The exception list `UNEXPLAINED` of `tests/it/props_explain.rs` goes with the fix.
- FT-1917 `ew-store, ew-api`: Two sessions that each set another override of one wire (the cross-section in one, the colour in the other) collide when the wire's record held no overrides yet: the merge shows the cross-section as changed on both sides ("base (none) mine 2.5 theirs (none)") and refuses the later edit with "changed since the choice was shown". Two fields that do not touch should merge. The same with an unsaved edit here and a colleague's save of the colour alone: a second collision is shown on the cross-section.
- FT-1918 `ew-commands, ew-derive`: A device inserted onto a wire splits it, and the wire's printed number goes to the new half: on the sample's page 101 the PE wire of -101G1 prints 101.4, and after a contact is put on its upright part the kept side prints 101.17 and the new half 101.4. FR-CON-008 keeps the number on the side that keeps the record. `tests/it/wiring.rs` pins it as it comes and turns with the fix.
- FT-1920 `ew-scene, ew-commands`: The legend that Enclosure… places on the sample's panel page 902 draws 21 of its 30 rows ("too many rows") and cuts Qty and Part number on every row. The sample is what a new person opens first: the legend a command places should hold its rows and its columns their values. The README's panel layout picture shows it and is shot again with the fix.
- FT-1921 `ew-export, ew-import-dxf`: A page exported to DXF and read back puts a terminal's texts (designations and strip tags, the terminal style of 2.52 mm) 0.049 mm higher than they stood, where every other text returns within 0.01 mm: the sample's pages 101 to 103, the first on 101 at (51.00, 49.31) mm.
- FT-1922 `ew-viewmodel, ew-commands`: A bus line drawn with the window's bus tools (the DP line from -104A1 to -201K1 on the sample) makes the logical net only. The bus segments, EW-BUS-004 and the bus overview see nothing until `bus.physical_net.create` is run by hand; drawing the line should make the physical net on both ports.
- FT-1923 `ew-reports, ew-scene`: An I/O list block on the PLC overview's form cuts the CPU's tag (-104A1) and the part number on every row of the sample, with or without the deactivated channels.
- FT-1924 `ew-checks, ew-viewmodel`: The Messages row of a cable without its length (EW-CORE-033; the sample's -101W1 with its length cleared) names nothing on a page: Enter opens nothing, where it should open the page at the cable's cores.
- FT-1925 `ew-api`: Verifying the sample against its own PDF reports a deviation: each part description of the parts lists 901 and 902 that wraps ("Miniature circuit breaker, …") counts as two missing texts and one extra text. A project verified against the PDF it wrote should deviate nowhere.
- FT-1926 `ew-commands`: Generate overview on a station whose overview runs over several pages names them 104.1, 104.1.1, 104.1.1.1 (the sample's -104A1 with a band one module wide), where they should be 104.1, 104.2, 104.3.
- FT-1927 `ew-derive`: The valve terminal configuration of a terminal that is its own valve module lists its valves as modules, with Position, Valve, Valve part, Solenoids and Address empty (`valve_terminal_rows()` never reads the terminal's own valve slots; doc 11 sections 2.6, 5.5 and 8.2 ask for a row per position). The sample's -201K1 should give positions 1 to 4 with -201K1-Q1 to -Q4, their part, solenoid 14 and Q0.0 to Q0.6. The view `api_v1_valve_position` reads the same rows: `ew sql fixtures/sample/sample.ew "SELECT * FROM api_v1_valve_position"` gives no row, where the four positions of -201K1 belong (two valves, two spare). `crates/ew-api/tests/expected/kinds/valve_terminal_config.csv` holds the rows as they come and turns with the fix.
- FT-1928 `ew-commands, ew-api`: A project made by File ▸ New has no supply, and no command gives it one: `project.supply.set` refuses ("refusal-supply-none"). Doc 03 leaves only basic and macro projects without a supply, so the sample, which its script makes from a new project, has none either. A new schematic project should hold a supply, or the first `project.supply.set` should make it. Not settled which.
- FT-1929 `ew-scene, ew-ui`: In the symbol editor the turned variants of a symbol print their texts horizontal and on top of each other: Symbol editor on a copy of the starter coil (`symbol.copy_edit` of `ew.iec.coil@qet10`), the strip "Turned and mirrored", variants B, D, F and H. In 0.2.0 those texts stood turned a quarter beside the symbol and could be read. Whether a turned coil on a page draws the same is not looked at. The README's symbol-editor picture is still 0.2.0's and is shot again with the fix, with a library folder that is no local path.

#### The page's drawing

- FT-2020 `ew-scene`: A strip segment draws no bridges: doc 08 §6.5 asks for the vertical bus with dots across the terminals a bridge joins, which only terminals drawn alone in a row have (FT-1908).
- FT-2021 `ew-scene, ew-drafting`: The layout's page does not hold a strip's drawn bridges (doc 08 §6.5): the text placer may set a text on a bridge's line and the router may run a wire across it; neither is reported.

#### A command's words

- FT-1819 `ew-commands, ew-viewmodel`: A command's extra notices read in English in a Dutch window, because `Outcome.notices` (`ew-commands/src/command.rs`) is a list of strings: carry a text key through `Outcome` (a box reshape's moved pin, a paste's notices).

#### Work on the window's thread

- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Reading an EDZ or a parts XML (twice), a PLC import's read and merge, and a report spreadsheet becoming a project still run on the window's thread; each needs its session-bound step split out before it can be a job.

### The placer and the router

From the drafting redesign (ADR-0034, `docs/design/06a-drafting.md`) and the pages since. Step 13 takes a row only
where a real page shows it; each row says whether one is known.

- FT-1891 `ew-drafting`: Route weighs a kept route per net, not per wire: on a page with an older two-row card its plan is two crossings worse than the page was, because one wire at a supply mark loses the person's route. **Page:** known, a page with an older two-row card.
- FT-1892 `ew-viewmodel`: The Arrange and route card, after rows are unticked, still lists a few wire rows that Route would redraw; find whether they belong there (the test's loop bound went from 8 to 64 without the cause). **Page:** none known; a window test shows it.
- FT-1893 `ew-drafting`: On a page with cards a feed terminal without a channel stays in the middle of its strip and its wires cross the channels; it wants a place at the strip's end. **Page:** none named.
- FT-1894 `ew-drafting`: A card does not step aside where another card's used columns interleave with its own (the columns should be chosen by the channels in use), and "Draw on a page" still sets a card of inputs under the controller, so supply wires cross its first channels until Arrange and route moves it. **Page:** any card drawn from the station view shows the second.
- FT-1909 `ew-drafting, ew-scene`: A card's supply pins stand at its left end, where a page's incoming marks stand, so their wires bend round the marks, and Arrange and route makes the page worse, not better. **Page:** known, the sample's PLC page.
- FT-1895 `ew-drafting`: Wires that go round a body between two rows of pins (an older two-row card, a box with pins on two sides) are searched one by one and cross each other; lay them side by side in nested tracks, with the placer opening the gap they need. **Page:** known, a page with an older two-row card.
- FT-1896 `ew-drafting`: After Arrange and route a supply page gains one text on a text and a small zigzag between two pins a step apart; two wires that get their first route add three crossings; a wire goes round a part where the part could stand in line; wires cross texts on a page with an older card. **Page:** seen on single pages, not named.
- FT-1897 `ew-commands, ew-viewmodel`: An older two-row card is drawn in the new form only by deleting its box and drawing it again from the station view, and terminals that then face the other way are rewired by hand: a command that redraws a card in place and keeps its wiring. **Page:** known, every page with an older two-row card.
- FT-1898 `ew-scene, ew-api`: A card drawn in cells with level texts (address and function text per channel), as the pilot's reference set draws it; the pin texts of a drawn box stay turned a quarter until then. The bevel junction's symbol is a quarter step where two wires parting at a pin take half a step: one size. **Page:** known, the pilot's reference set.
- FT-1831 `ew-scene, ew-derive`: A black box's caption stands above it, left of its first top pin, where the wires and tubes into those pins pass, so they loop round it: place the caption clear of the pins' ways. **Page:** known, the persona runs' pages.
- FT-1837 `ew-drafting`: Tubes from a wide valve terminal with upward ports to a cylinder below are long, because every clear way is. **Page:** known, the persona runs' fluid pages.
- FT-2010 `ew-commands, ew-drafting`: After a grid change a wire to a pin that its symbol's own spacing keeps off the new grid finds no way, though it routed before. The off-grid pin is not the cause (a wire to one nearly in line now steps square, 2026-11-22): the fit keeps gaps in millimetres, and on the coarser grid there is no grid line a step from both a body and its neighbour (-104A1's cards stand 2.3 steps from the drawing area's edge, with their supply pins on that side; -201X1's corner stands 1 mm beside -201X2's pin's line). The fit would have to keep the room a pin's wire needs on the new grid (doc 06 §8 item 3), or the search to pass a body nearer than a step at a pin. **Page:** known, the sample's 104 (three wires of -104A1's cards) and 201 (-201K1:3 to -201X2:1) changed to 4 mm.
- FT-1935 `ew-commands`: After a grid change a row of like symbols that stood at one pitch stands at an uneven one, because each goes to its own nearest grid point (twelve coils of an imported page changed to 4 mm: some a step closer than their neighbours). Let a row that had one pitch keep one.

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
