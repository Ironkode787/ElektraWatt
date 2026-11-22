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

- FT-1941 `ew-import-qet, ew-scene`: After FT-1765, 493 of about 4,800 imported texts of the pilot reference pair still stand apart from QET's own PDF (doc 14 §4.10): title-block cells 1 to 2 mm across, because QET's `hadjust` scales a too-wide text without a floor and never for its height while EW shrinks a cell's lines to fit its height and stops at 0.5 mm (`frames.rs`, `forms/text.rs`); rich-text free texts that QET's computer printed in Tahoma 0.7 to 1.6 mm higher (its ascent is 1.0 em; the face QET printed is its computer's substitution, which the file does not say); 94 texts of 8 pt in a lower band of some pages 0.5 mm higher, not yet examined. Fix round (2026-11-22): not measured again, so the counts stand: the private corpus folder holds the QET projects but not QET's own PDF of the pair, and the comparison of FT-1765 was a lane's script that was not kept; the pair and a kept script (doc 14 §4.10's counts) come first. The `hadjust` class needs QET's own fit, a text condensed across only and without a floor: a new `Overflow` variant of a form cell drawn by `forms/text.rs` and every output, a format addition decided first.

#### Lists and frames


#### External points, earth and 0 V


#### Files and the text interface

- FT-1985 `ew-import-eplan`: A multi-level terminal that the report importer folds (EW-IMP-207) lands every level's connections on level 1; since FT-1878 its listed levels have connection points of their own (doc 08 §6.3), so each level's connections can land on its own. Found in the fix round (2026-11-22): the importer folds levels only in a terminal diagram, which carries no connections, and a connection list names its ends `strip:designation` without a level (all taken as level 1), so no import knows a connection's level. The fix needs a level per end in the connection map (members `from_level`, `to_level`, in the mapper too) and the end resolved to that level's CPs (`Terminal::level_copies`): a change of the report map, decided first.

#### Two drawing faults


### What a person at the window meets

From the owner's tests of 0.3.0's candidates and the persona runs.

#### Your own PLC and bus parts


#### The station view and the PLC grid


#### The window's finish

- FT-2006 `ew-ui`: Two faults of FT-1761 not seen again on the sample at 1920 × 1080: the first click on a command bar button right after a grid cell edit only ending the edit (Bus table, Wire: one click ends the edit and runs Wire), and the Properties header cut by its chips; each needs its repro (which grid, which selection) before a fix. Tried again in M3.5 step 13 (a window test on the sample's page 102, 1920 × 1080 and 1000 × 900, a cell of the Strips and of the Bus table opened by a double click and typed in, then one click on Wire): Wire arms at the first click; on the Bus table the cell's editor stays open beside the armed tool, which may be the fault seen. Not fixed without the person's steps.

#### Properties and the title block's words

- FT-2065 `ew-viewmodel`: The help of a device's tag, parts and technical data, a strip's Label mode, a page's Scale, Grid and Number, a section's, a report block's and a party's address rows says what the program does with the value, an example and its unit (FT-1809's hand, 2026-11-22); the rows of the other kinds still often repeat their label (a symbol's Arrangement, Anchor and text slots, a junction's Position and Style, a page item's Layer and its kinds, a bus port's Bus, Interface, Role and Termination, a connection point's Description, an assembly's configured products): do the same for them, checking each against the code that reads it.

#### The layout page

- FT-2031 `ew-reports, ew-api`: Two layout pages that show one plate share one legend list (the first in page order gets the links), and the second is offered no legend of its own; doc 12 §5.4 leaves a legend page per layout page open.

#### What a new item takes

- FT-2055 `ew-derive`: A new wire's cross-section reads its circuit class only (FT-1834, doc 08 §4.2), not the rating of the device that protects it: a 16 A circuit of the control class takes the control size. Sizing by the protecting device's rating needs a table of cross-sections by current (installation method, temperature, grouping), which the program does not hold; design first.

#### What the sample project's script met

Faults met while `fixtures/sample/sample.uiscript` was written; each has its steps in the script's own words.


#### What the tests met on the sample

Faults met while the tests moved from the reference project to the sample (M3.5, step 9); each test took another way round its fault, so a fix adds the test that goes straight.

- FT-1918 `ew-commands, ew-derive`: A device inserted onto a wire splits it, and the wire's printed number goes to the new half: on the sample's page 101 the PE wire of -101G1 prints 101.4, and after a contact is put on its upright part the kept side prints 101.17 and the new half 101.4. FR-CON-008 keeps the number on the side that keeps the record. `tests/it/wiring.rs` pins it as it comes and turns with the fix. Found in the fix round (2026-11-22): the split does keep the record and its `NumberSpec` on the `a` part; the sample's numbers are derived, never stored (`numbering.rs`, doc 08 §5.1), and follow the numbering order, so the half that keeps the wire's From end takes the old place in the count. Keeping the printed number needs stored numbers (FR-NUM-020) or the record kept on the From end's half, which changes FR-CON-008's acceptance (the `a` side): a decision first.
- FT-1929 `ew-scene, ew-ui`: In the symbol editor the turned variants of a symbol print their texts horizontal and on top of each other: Symbol editor on a copy of the starter coil (`symbol.copy_edit` of `ew.iec.coil@qet10`), the strip "Turned and mirrored", variants B, D, F and H. In 0.2.0 those texts stood turned a quarter beside the symbol and could be read. Whether a turned coil on a page draws the same is not looked at. The README's symbol-editor picture is still 0.2.0's and is shot again with the fix, with a library folder that is no local path. Found in M3.5 step 13: the change is fabf4c4da (2026-11-15, the owner's finding that a turned part's tag and designations must read level): a slot text keeps its angle in every variant while its anchor turns, so the texts that stand under each other beside an upright coil (tag, Text, Technical data, Cross-reference) stand side by side on one line in B, D, F and H and run into each other ("Technical dataText -U1"); the pin designations A1 and A2 read well. Turning them a quarter again, as 0.2.0 did, undoes that rule on the pages too; keeping them level needs a rule that stacks them again beside a turned symbol. Which one is the owner's to choose, with the two pictures.

#### The page's drawing

- FT-2020 `ew-scene`: A strip segment draws no bridges: doc 08 §6.5 asks for the vertical bus with dots across the terminals a bridge joins, which only terminals drawn alone in a row have (FT-1908).

#### A command's words

- FT-1819 `ew-commands, ew-viewmodel`: A command's extra notices read in English in a Dutch window, because `Outcome.notices` (`ew-commands/src/command.rs`) is a list of strings: carry a text key through `Outcome` (a box reshape's moved pin, a paste's notices). Counted in M3.5 step 13: some 35 places write a notice (`box_reshape`, `company`, `device`, `exchange`, `join`, `join_level`, `overview`, `paste`, `plc`, `plc_exchange`, and `TxBuilder::notice` from `fluid` and `page`), and many build their English from helpers (`words::device`, `listed`/`was_or_were`, `binding_words`, `address_words`, `ew_plc::field_words`/`object_words`), which must yield `Text` as well; `layout.auto_fill_rail` already builds `Text` and flattens it. The change: `notices: Vec<Text>`, a key per notice in both languages, the helpers on `TextArg`, the window worded by `command_error_words`' path and the CLI by `Display`: a lane of its own in `ew-commands`.

#### Work on the window's thread

- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Reading an EDZ or a parts XML (twice), a PLC import's read and merge, and a report spreadsheet becoming a project still run on the window's thread; each needs its session-bound step split out before it can be a job. Looked at in M3.5 step 13 (not built): for the EDZ and parts XML the split is small, `Session::eplan_parts` takes from the session only the manufacturer master and the project ID before the pure `parts_import`, so those two can be handed to a thread and `update_parts_from_import` run at the end; but the shell's jobs (`JobsState::spawn`) end in an `ExportDone` and a written file, so a job that hands back an import's result and opens a dialog at its end needs a job kind of its own first; the report spreadsheet and the PLC import then follow the same way.

### The placer and the router

From the drafting redesign (ADR-0034, `docs/design/06a-drafting.md`) and the pages since. Step 13 takes a row only
where a real page shows it; each row says whether one is known.

- FT-1891 `ew-drafting`: Route weighs a kept route per net, not per wire: on a page with an older two-row card its plan is two crossings worse than the page was, because one wire at a supply mark loses the person's route. **Page:** known, a page with an older two-row card. Not taken in the fix round (2026-11-22): no page with an older two-row card and the person's routes is in the repository to reproduce it on; a wire-by-wire pass after the nets' in `route::Router::weigh` (keep a wire's stored route where its ends have not moved and the page costs no more) is the likely fix.
- FT-1893 `ew-drafting`: On a page with cards a feed terminal without a channel stays in the middle of its strip and its wires cross the channels; it wants a place at the strip's end. **Page:** none named; step 13 takes a row only where a real page shows it, so it waits for one (2026-11-22).
- FT-1894 `ew-drafting`: A card does not step aside where another card's used columns interleave with its own (the columns should be chosen by the channels in use). **Page:** not found yet: it needs a page where two cards' used columns interleave (the sample's 104 has one channel used on its card of inputs). The row's other half, "Draw on a page" setting a card of inputs under the controller, is done (2026-11-22).
- FT-1895 `ew-drafting`: Wires that go round a body between two rows of pins (an older two-row card, a box with pins on two sides) are searched one by one and cross each other; lay them side by side in nested tracks, with the placer opening the gap they need. **Page:** known, a page with an older two-row card. Not taken in the fix round (2026-11-22): it is the router phase doc 06a §4.8 names as started and not built (the wires round one card laid together, the placer making the room), a design larger than a row, and no such page is in the repository.
- FT-1896 `ew-drafting`: After Arrange and route a supply page gains one text on a text and a small zigzag between two pins a step apart; two wires that get their first route add three crossings; a wire goes round a part where the part could stand in line; wires cross texts on a page with an older card. **Page:** seen on single pages, not named; step 13 takes a row only where a real page shows it, so it waits for one (2026-11-22).
- FT-1897 `ew-commands, ew-viewmodel`: An older two-row card is drawn in the new form only by deleting its box and drawing it again from the station view, and terminals that then face the other way are rewired by hand: a command that redraws a card in place and keeps its wiring. **Page:** known, every page with an older two-row card. Looked at in M3.5 step 13: `box.redraw` (Redraw them, `Session::redraw_box_command`) reshapes one box in place and keeps its wires, but only to a part's own `box` layout; a card's new form is one or two boxes per channel kind (`card_box`, `card_share` in `station_boxes`), so a card of inputs and outputs needs a second box made and its pins moved to it, and its terminals turned and their wires redrawn: a design of its own (which box keeps the record, where the second stands, how the terminals follow), not a row.
- FT-1898 `ew-scene, ew-api`: A card drawn in cells with level texts (address and function text per channel), as the pilot's reference set draws it; the pin texts of a drawn box stay turned a quarter until then. The bevel junction's symbol is a quarter step where two wires parting at a pin take half a step: one size. **Page:** known, the pilot's reference set. Fix round (2026-11-22): the bevel's size is the symbol `ew.gen.junction-bevel` (and its QET edition, compiled into `ew-api`'s view), half a step in two coordinates and `cargo xtask symbols-qet10`; but a compiled-in symbol changes every project's drawing, so it ships with a new derivation profile (`view.rs` test `the_compiled_in_generic_symbols_keep_their_seals_and_profile`), and the snapshots, page texts and wire picks of the sample's folio 101 turn with it: a profile step decided first. The card in cells is a design of doc 10 §6 first (which cells, which texts per channel, how they follow the channels).
- FT-1837 `ew-drafting`: Tubes from a wide valve terminal with upward ports to a cylinder below are long, because every clear way is. **Page:** known, the persona runs' fluid pages. Not reproduced (2026-11-22): on a page built after it (a terminal of four valves 32 steps wide, its ports up, a cylinder under its third valve) Route goes round the terminal, as it must, and Arrange and route sets the cylinder over its valve with two straight tubes (rule F1); the persona runs' pages, which are not in the repository, are needed to see what held their cylinders below.
- FT-2010 `ew-commands, ew-drafting`: After a grid change a wire to a pin that its symbol's own spacing keeps off the new grid finds no way, though it routed before. The off-grid pin is not the cause (a wire to one nearly in line now steps square, 2026-11-22): the fit keeps gaps in millimetres, and on the coarser grid there is no grid line a step from both a body and its neighbour (-104A1's cards stand 2.3 steps from the drawing area's edge, with their supply pins on that side; -201X1's corner stands 1 mm beside -201X2's pin's line). The fit would have to keep the room a pin's wire needs on the new grid (doc 06 §8 item 3), or the search to pass a body nearer than a step at a pin. **Page:** known, the sample's 104 (three wires of -104A1's cards) and 201 (-201K1:3 to -201X2:1) changed to 4 mm.
- FT-1831 `ew-scene, ew-derive`: A black box's caption stands above it, left of its first top pin, where the wires and tubes into those pins pass, so they loop round it: place the caption clear of the pins' ways. **Page:** known, the persona runs' pages. Found in the fix round (2026-11-22): the caption is the box's tag slot, placed by the label placer before the router (`LabelKind::BoxTag`'s places in `ew-drafting/src/labels.rs` `part_places`: above left of the box, above from its left edge, above right, inside top left), which knows the pins' sides but not the ways the wires will take; the fix is a rule of the placer redesign (which places lie in a pin's way, and the order), in the router lane's crate, so it was not made beside that lane.

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
