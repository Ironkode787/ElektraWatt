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

- FT-1872 `ew-export, ew-text, ew-scene`: The export prints one typeface, and a free text is anchored by its middle, so a set that must match an existing print in another typeface has every text set again outside the program (a fifth of the job's time). Smallest version: a project setting that names one typeface file, used by the PDF and the DXF export, and texts anchored by their baseline.
- FT-1765 `ew-import-qet, ew-scene`: Imported texts stand apart from QElectroTech's own PDF: the texts of title-block cells by 0.5 to 2.7 mm sideways, free texts of 12 pt and more by 0.7 to 1.2 mm in height (730 of about 4,800 texts on one pilot project). Measure again once FT-1872 anchors texts by their baseline.

#### The import binds what it draws

- FT-1869 `ew-import-qet, ew-commands, ew-viewmodel`: An imported element cannot be the symbol of a device with a part, so a set brought in loses its look as it gains devices, strips and cables (the job's pages were drawn anew with starter symbols, nothing like the source: owner, 2026-11-20; a fifth of the job's time). Wanted: a mapping from an element to a function definition, from its terminals to the function's connection points and from its texts to tag, number and cross-reference, keeping the element's symbol, place and texts. Smallest version: a mapping file for terminals, coils, contacts and interruption points.
- FT-1861 `ew-import-qet`: The import's device model of a wired cabinet is wrong: junction elements and earth symbols become terminals of inferred strips, unlabelled terminals on a strip's second folio do not join it, cable marks become plain devices instead of cables with cores, and routes on one folio are dropped (on the job: eight strips for three, about twice the terminals, no cable for three).
- FT-1860 `ew-commands, ew-viewmodel`: A device cannot take a second symbol of its own from the window or the text interface: `place.pick` with a library symbol and a device asks for a symbol, `rep.join_device` takes contacts only, and `ew import parts` refuses a function definition of the project's library. Only the model command `device.place` with `join_device` does it.
- FT-1859 `ew-import-qet, ew-commands, ew-viewmodel`: The import writes a title block's values (function, location, object, drawing number, description, revision, dates) into the form as fixed texts, where the project's and the page's fields should print; `library.embed` refuses a project-local form, and a report block's form cannot be set. Editing a form in the window is not this row: it is FT-1814, M4.

#### Lists and frames

- FT-1874 `ew-reports, ew-scene, ew-viewmodel`: A report block cannot take the page's own frame and title block, chosen columns with their headings and widths, a caption, a legend line, or a row that is not in the model, and a list page prints English column heads in a Dutch project. The job's terminal, cable and material lists and its contents were lines and texts written by a script from the SQL views (an eighth of its time).
- FT-1826 `content, ew-export, ew-reports`: There is no A3 frame or title block among the starter forms, the list pages keep an A4 frame of their own, and `ew print --fit` never enlarges, so a set that must be A3 needs forms made for it.
- FT-1805 `ew-reports`: The terminal diagram (`kinds/terminal_diagram.rs`) lists terminal numbers without their strip, so two strips' terminals 1 read alike.

#### Texts the schematic carries

- FT-1875 `ew-commands, ew-scene, ew-drafting`: Texts that follow nothing in the model have no place: a cable's name and type and a core's number at a point on a wire, a strip's name once per strip and page (the program names it at every terminal), a device's function text over its current path, a note on a page. The text placer moves the tags and cross-references of an imported page away from where the drawing has them (the job pinned some three hundred by script): a switch that leaves a text where its symbol has it, or the placer only for texts without a place.
- FT-1870 `ew-drafting, ew-scene`: The tags and cross-references of like devices in one row stand at different places from device to device (each text is placed alone; a row should take one place for all); a code written as a free text beside a wire lies on the wire; a mark set at a wire's head leaves a small hook where the mark's pin is not on the wire's line.
- FT-1858 `ew-derive, ew-scene, ew-reports, ew-viewmodel`: A wire's frozen number is stored and never printed: the page, the wire list and the SQL view keep the automatic number, Properties shows "automatic · frozen", and typing the number into its row is refused as "not strict JSON". There is no numbering scheme by potential (one code per potential, a circuit number in front), which a drawing specification may ask for.
- FT-1799 `ew-api`: An unnumbered wire's Properties title reads "?" (`wire_number_text`, `output/properties/selection.rs`); say "wire without a number" in words.

#### External points, earth and 0 V

- FT-1876 `ew-derive, ew-scene, ew-checks`: An external point must have a location and a device and prints both, not the connection it stands for. A set whose signals come from another cabinet needs the reference's parts chosen (an address alone), a mark named for its signal, and a point whose far end is not known.
- FT-1867 `ew-scene, ew-derive`: Technical data has no unit `mA`; a coil's data prints through the coil's wire; a 0 V potential bonded to an earth symbol is typed PE as a whole.
- FT-1820 `ew-commands, ew-derive`: An earth symbol takes its number from the cables' W counter (its tag is hidden on the page and used in the wire list), so a project's first cable can be W2 and two devices can be tagged W1; changing the symbol's letter is EW-RDS-003. Decide the symbol's own letter, or give it no tag.
- FT-1821 `ew-derive, ew-checks`: The rule that a wire from 0 V to earth is the bond (`bond_carrier.rs`) looks at one page only: a 0 V that comes in through an interruption point from another page still needs Draw ▸ Connect ▸ Bond.

#### The DXF as the PDF

- FT-1877 `ew-export`: The DXF export does not equal the PDF: other type metrics, no picture in the title block, no AutoCAD 2010 format, and none of the page's structure (blocks with attributes, cross-reference links). The job's DWG files were made from the PDF's geometry: faithful as a plot, with no blocks and no attributes. A DWG is made from the DXF by a converter, as the import reads one; the program gets no DWG writer (owner, 2026-11-20).

#### What the model has no place for

- FT-1878 `ew-model, ew-checks, ew-scene`: A draughtsman's drawing shows what the model cannot hold: two terminal numbers on one two-level terminal block (the job put half a part on each); a part whose type is not known (no placeholder: the device has no part and the check warns). The other three such gaps are FT-1905, M4.

#### Files and the text interface

- FT-1879 `ew-store, ew-library, ew`: A library file that the manifest does not name is not read and nothing says so; `ew fmt` refuses a slot with `"visible": false`, and a slot is hidden unless it says `true`.
- FT-1865 `ew-commands`: `cable.from_wires` ignores `cores` given as a list (a string works) and assigns the first colours of another cable; `page.delete` with `delete_devices` leaves the emptied strips; `strip.new` with a tag that exists makes a second strip silently; `layout.enclosure_new` refuses a part of the project's library as "not on offer"; a library symbol is addressed as `library:<id>:<id>` before its first placement and by its key after, and each form is refused in the other state; a part on the starter power-supply function with a symbol of its own is refused by the Place tool ("place it with box.black") and by the Black box tool ("not a black box").
- FT-1868 `ew-viewmodel, ew`: No action prints the action list; `report_block.new` offers words in its error and takes keys; page rows cannot be reached after `props.project`; `ew import qet --out a/b.ew` with a missing folder `a` imports and then writes nothing.
- FT-1866 `ew-store, ew-viewmodel`: A leftover crash journal (`journal.rs`, kept outside the project folder and found by the project's id and path) is offered to another project that is later made at the same path with the same id; FT-832 (backlog-later) is the same cause for a copy.

#### A copy of pages

- FT-1862 `ew-commands, ew-viewmodel`: No copy of pages keeps what belongs together (coil and contacts, a strip's terminals, a cable's cores) and renumbers by a rule, so a second, identical installation is built by running the same script twice. Paste hangs by its middle and cannot land in place, and is refused when one text lies in the header band where the Text tool put it; `macros.save` with pages says nothing is selected.

#### Two drawing faults

- FT-1864 `ew-drafting, ew-commands`: A straight wire drawn one step above a row of symbols is rerouted round them without a word, and the taps on it then hang free.
- FT-1871 `ew-ui, ew-scene`: A symbol drawn in the symbol editor with pins on its lower edge prints the pin texts across the pins, and a text of the inner drawing above its box; the editor shows neither before the symbol stands on a page.

### What a person at the window meets

From the owner's tests of 0.3.0's candidates and the persona runs.

#### Your own PLC and bus parts

- FT-1766 `ew-api, ew-commands, ew-viewmodel, ew-ui`: What is left of "your own PLC and bus parts" (owner, 2026-11-15): the boxes of a device with several interfaces can be dragged apart on a bus page, and one placed near the page's edge runs past it or lands on another device (one box for such a device is FT-1904, M4); a port a corrected part adds appears only when the box is drawn again, and removed sockets can leave marks; a part cannot be removed from My parts; EW-PLC-012 has no quick fix that opens the PLC section; a part cannot hold a card with two channel kinds. A device placed first on a bus page takes its tag from that page, and placing its black box on a schematic page later changes the tag to that page's without a word: say so in the toast, or keep the tag.
- FT-1788 `ew-viewmodel, ew-ui`: A PLC module's or a bus device's device box opens read-only (`shell/symbol_editor/device_box.rs` locks pins that come from channels or bus interfaces); edit them in the box itself.

#### The station view and the PLC grid

- FT-1846 `ew-viewmodel, ew-api`: Paste and fill-down in the PLC grid's Function text column say "no command yet"; let both act on the wired device's function text, as typing does.
- FT-1838 `ew-ui, ew-viewmodel`: F2 does not open a channel's cell in the station view's channel table; a click does.
- FT-1823 `ew-viewmodel`: The overview page that a station's To do proposes takes a folio in the wiring section.
- FT-1824 `ew-ui`: A toast can cover a To do button of the station view.

#### The window's finish

- FT-1761 `ew-viewmodel, ew-ui`: The Bus table's Termination cell is free text with no hint of its words (Properties has one); EW-POT-009 says to mark a pin not used and its message row does not offer it; the first click on a ribbon button right after a grid cell edit only ends the edit; the Bus, Devices and Medium tables do not widen their columns or fill their panel at 2560 × 1440; the Medium table's count disagrees with the navigator's badge ("1 lines"); a grid tooltip stands above a dialog's backdrop; a line's Properties lack its medium; every row of a report page's Properties has a "generated" chip; the Properties header is cut by its chips at 1920; the symbol browser opens on a disabled row; preview rows in lower case, zero counts and "Xrefs"; a medium's category "Electrical"; the Mapping dialog at the top of the window; context menus that cover their row, and grey entries without a reason outside the navigators' row menus; the check's toast has no way to the Messages list.
- FT-1806 `ew-viewmodel`: A tool stands at another place on another tab of the command bar (Wire on Draw, another button at that place on PLC); a tool used on several tabs should keep its place.
- FT-1810 `ew-ui`: The symbol editor's start card gets a "Carry on with…" row on top after a save, which moves every entry down; Copy a library symbol is one long list without order or preview; the pin list of a device box sorts itself again after an edge is changed.
- FT-1830 `ew-viewmodel, ew-ui`: A person who expects the mouse wheel to pan meets a wheel that zooms; View ▸ Mouse wheel scrolls makes it pan, but only the zoom's tooltip says so. Offer the choice where the zoom is first met, and give the page scroll bars.
- FT-1845 `ew-viewmodel, ew-ui`: A part search for "sensor" finds nothing, "normally open contact" ranks the contactor first, and the first row is selected before the person chose it, so Enter takes it. Search by what a part is for as well as by its words, and preselect nothing.

#### Properties and the title block's words

- FT-1803 `ew-viewmodel, ew-api, ew-scene`: Project properties name their rows otherwise than the title block's cells (Project reference and Project, Designer and Author), Document title prints in no cell of the program's frame, and the Checker row does not feed the Checked cell (which prints who approves the current revision); only Title block fields… says which cell prints which. Name the rows as the cells, and decide whether Checker feeds Checked.
- FT-1779 `ew-viewmodel, ew-ui`: A wire's Potential row in Properties should open Define potential, as the wire's menu does; today only the row's "…" menu offers it.
- FT-1808 `ew-viewmodel`: Placing a part that has no symbol arms the Black box tool without saying that the part is drawn as a box.
- FT-1811 `ew-ui, ew-api`: A pin's place typed between two steps of the pin grid is taken to the next step without a word; a copy of a starter part keeps the starter's part number without saying so.
- FT-1812 `ew-viewmodel`: F1 on a selected connection point in the symbol editor opens the outline's section although the guide has a section for a connection point.
- FT-1813 `ew-viewmodel`: The Place tool shows "-?X?:1" for a new strip until the pointer moves; the project's Kind row shows a dash.
- FT-1809 `docs/book`: A property's help page often repeats the row's label (the project's Control voltage); say what the program does with the value, its unit and an example.

#### The grid fit and moves

- FT-1817 `ew-commands, ew-api`: After a page's grid change or a paste fitted to another grid each item goes to its nearest grid point one by one (`commands/page.rs`, `commands/paste.rs`, `geometry.rs`), so two symbols can end on one another (EW-CORE-077 reports it) and two pins so close that a wire finds no way. Let the fit part what it pushed together and leave room for every wire.
- FT-1818 `ew-viewmodel`: With a fitted symbol (its first pin on the grid, its position between grid points) the first arrow key moves the ghost a part of a step in the readout, while the move itself is whole steps.
- FT-1836 `ew-viewmodel, ew-scene`: While a wire's pin name is asked (a wire drawn to a box's edge where no pin is), the wire's ghost is hidden; show it until the name is given.
- FT-1796 `ew-viewmodel`: A drag that starts on a pin's label of a symbol moves the symbol; the tag, a box's caption and the texts of one selected symbol are dragged by themselves already.
- FT-1839 `ew-viewmodel`: The left and right sides of a drawn box have no smallest height by their pin texts, only one step per pin; the top and bottom sides have.
- FT-1780 `ew-scene, ew-viewmodel`: A selected wire does not mark its two ends.

#### The layout page

- FT-1768 `ew-api`: `centred_datum` (`output/layout_edit.rs`, where an enclosure is placed) maps the drawing area, which `drawing_area` (`output/scale_fit.rs`) gives in page coordinates already, through the frame's page-to-sheet map once more: on a frame with an offset the mounting plate stands off-centre. A test on a frame with an offset shows the plate's place.
- FT-1802 `ew-viewmodel, ew-api`: Auto-fill rail gives every device without a location the cabinet's and places them all, field devices too (sensors, cylinders, a valve terminal); let the person choose which devices go on the rail.

#### Lists and legends

These touch the files of "Lists and frames" above: not beside that lane.

- FT-1772 `ew-api, ew-viewmodel`: A legend block made before `ReportBlock.columns` keeps the report's four columns (position, tag, quantity, part number), and nothing offers it the description.
- FT-1782 `ew-scene`: The legend's part-number column is shortened and, last, left out where the legend has no room (decide whether the legend wraps its texts or asks for a wider box); an enclosure's legend shows "…" in its quantity and part-number columns (the embedded table's own cutting rule), and its header and drop shadow are heavy.
- FT-1827 `ew-reports`: The potential overview numbers its bond rows by id order, so a drawn bond can shift the numbers of the declared ones.
- FT-1842 `ew-reports, ew-derive`: A list's last page can hold only one or two rows.

#### What a new item takes

- FT-1834 `ew-derive, ew-viewmodel`: A new wire takes the control size, 0.75 mm², unless its potential names another circuit class, so a 16 A mains wire gets it. Propose the cross-section from the circuit.
- FT-1791 `ew-viewmodel, ew-api`: The spacing between the terminals of a row (`shell/terminals.rs`) is kept by the open project only: keep it between sessions (a user setting, or a project setting, which is a format change).
- FT-1840 `content`: There is no proximity or reed sensor among the starter parts: a three-wire PNP sensor with its symbol.

#### What the sample project's script met

Faults met while `fixtures/sample/sample.uiscript` was written; each has its steps in the script's own words.

- FT-1899 `ew-commands, ew-derive`: A new enclosure and its mounting plate get frozen tags that deviate from the scheme at once: `layout.enclosure_new` on a new panel-layout page, then the check, gives EW-NUM-001 for both ("frozen, deviates from scheme"). Give them the scheme's tags, or do not freeze them.
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
- FT-1920 `ew-scene, ew-commands`: The legend that Enclosure… places on the sample's panel page 902 draws 21 of its 30 rows ("too many rows") and cuts Qty and Part number on every row. The sample is what a new person opens first: the legend a command places should hold its rows and its columns their values.
- FT-1921 `ew-export, ew-import-dxf`: A page exported to DXF and read back puts a terminal's texts (designations and strip tags, the terminal style of 2.52 mm) 0.049 mm higher than they stood, where every other text returns within 0.01 mm: the sample's pages 101 to 103, the first on 101 at (51.00, 49.31) mm.
- FT-1922 `ew-viewmodel, ew-commands`: A bus line drawn with the window's bus tools (the DP line from -104A1 to -201K1 on the sample) makes the logical net only. The bus segments, EW-BUS-004 and the bus overview see nothing until `bus.physical_net.create` is run by hand; drawing the line should make the physical net on both ports.
- FT-1923 `ew-reports, ew-scene`: An I/O list block on the PLC overview's form cuts the CPU's tag (-104A1) and the part number on every row of the sample, with or without the deactivated channels.
- FT-1924 `ew-checks, ew-viewmodel`: The Messages row of a cable without its length (EW-CORE-033; the sample's -101W1 with its length cleared) names nothing on a page: Enter opens nothing, where it should open the page at the cable's cores.
- FT-1925 `ew-api`: Verifying the sample against its own PDF reports a deviation: each part description of the parts lists 901 and 902 that wraps ("Miniature circuit breaker, …") counts as two missing texts and one extra text. A project verified against the PDF it wrote should deviate nowhere.
- FT-1926 `ew-commands`: Generate overview on a station whose overview runs over several pages names them 104.1, 104.1.1, 104.1.1.1 (the sample's -104A1 with a band one module wide), where they should be 104.1, 104.2, 104.3.
- FT-1927 `ew-derive`: The valve terminal configuration of a terminal that is its own valve module lists its valves as modules, with Position, Valve, Valve part, Solenoids and Address empty (`valve_terminal_rows()` never reads the terminal's own valve slots; doc 11 sections 2.6, 5.5 and 8.2 ask for a row per position). The sample's -201K1 should give positions 1 to 4 with -201K1-Q1 to -Q4, their part, solenoid 14 and Q0.0 to Q0.6. The view `api_v1_valve_position` reads the same rows: `ew sql fixtures/sample/sample.ew "SELECT * FROM api_v1_valve_position"` gives no row, where the four positions of -201K1 belong (two valves, two spare). `crates/ew-api/tests/expected/kinds/valve_terminal_config.csv` holds the rows as they come and turns with the fix.
- FT-1928 `ew-commands, ew-api`: A project made by File ▸ New has no supply, and no command gives it one: `project.supply.set` refuses ("refusal-supply-none"). Doc 03 leaves only basic and macro projects without a supply, so the sample, which its script makes from a new project, has none either. A new schematic project should hold a supply, or the first `project.supply.set` should make it. Not settled which.

#### The page's drawing

- FT-1798 `ew-scene`: An interruption point on the bottom pin of a box draws its arrow along the box's edge.
- FT-1786 `ew-scene`: A text under a wire on a schematic page is not knocked out: a text that a route crosses and that does not move is drawn under the line, and no renderer clears the line round it (ADR-0033, amendment 2). Only the panel layout's labels have knock-outs.
- FT-1908 `ew-scene, ew-derive`: A strip's bridges are not drawn on a schematic page: four terminals joined by a bridge stand as four separate terminals, and a reader cannot see that a supply reaches the others (the sample's control page). Draw the bridge between the terminals it joins where they stand in one row on one page.

#### A command's words

- FT-1819 `ew-commands, ew-viewmodel`: A command's extra notices read in English in a Dutch window, because `Outcome.notices` (`ew-commands/src/command.rs`) is a list of strings: carry a text key through `Outcome` (a box reshape's moved pin, a paste's notices).

#### Work on the window's thread

- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Reading an EDZ or a parts XML (twice), a PLC import's read and merge, and a report spreadsheet becoming a project still run on the window's thread; each needs its session-bound step split out before it can be a job.

### The placer and the router

From the drafting redesign (ADR-0034, `docs/design/06a-drafting.md`) and the pages since. Step 13 takes a row only
where a real page shows it; each row says whether one is known.

- FT-1880 `ew-drafting`: The placer does not always settle in one press: after one symbol is moved two steps, a second press of Arrange and route moves ten symbols and routes thirteen wires and only the third offers nothing; on two pages with wires that find no way a second Route moves one wire by a step. The design's invariant is that a second press offers nothing: iterate to the fixed point before the plan is offered, or find why the first plan is not it. **Page:** known, a page of the reference project (rebuild the case on the sample before step 9 removes it).
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

## M3.5: what its own steps leave

Rows that the hygiene steps of [m3.5.md](m3.5.md) take, or that they left.

- FT-1816 `golden, ew-api, ew-viewmodel, ew-ui, ew-derive, ew-commands, ew-testkit, xtask, docs`: Retire the reference project (owner, at the fourth candidate of 0.3.0): step 9, as [m3.5-reference-project.md](m3.5-reference-project.md) designs it.
- FT-1764 `ew-ui`: `a_smaller_window_stays_usable` (`tests/it/command_surface.rs`) proves less than its name: its label query matches any node, and it checks the command bar and the page's size only (step 10).
- FT-1787 `elektrawatt`: The deep run's replay of every script twice (`nightly_every_script_transcript_is_byte_identical_twice`, `tests/it/headless.rs`) runs close to its time limit; split it or give it room once step 9 has changed its scripts (step 10). Not measured.
- FT-1775 `xtask, ew-commands, ew-viewmodel`: The test builds and a plain `cargo build` (the headless pictures, `ew`) compile every crate above `ew-commands` twice: the tests with the test-only `testkit` features of `ew-commands` and `ew-viewmodel` (`[[uniform_test_features]]`), the plain build without; `ew-commands`' `testkit` turns on `ew-model`'s, so a lower crate tested alone builds twice too. One flavour would save the second compile of the heavy crates (not measured).
- FT-1890 `ew-api, ew-commands, ew-derive, ew-viewmodel, ew-ui, ew-scene, xtask`: Comments still carry about 650 section references that name no document (a bare `§`) and 154 candidate tags (`rc.3`, `0.3.0-rc.1`); step 7 removed the rest. Name the document, or say the point without the reference.
## M4

Rows for M4, manufacturing and the 2D/3D panel ([ROADMAP.md](../../ROADMAP.md)). FT-1814 and FT-1815 the owner asked
for at his test of 0.3.0's fourth candidate; each starts with a design from the user's side (who changes it, when,
what he sees while he does it) before anything is built.

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
