# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.

### What M3's reviews left

- FT-1765 `ew-import-qet, ew-scene`: What still stands apart from QElectroTech's own PDF after FT-1746 put the static texts on QET's baseline: the texts of title-block cells (0.5 to 2.7 mm sideways) and free texts of 12 pt and more (0.7 to 1.2 mm high). On one pilot project 730 of about 4,800 texts.
- FT-1766 `ew-api, ew-commands, ew-viewmodel, ew-ui`: What is left of "your own PLC and bus parts" (owner, 2026-11-15): a device with several interfaces is one box per interface on a bus page (one box needs a box connection point that names its function, a format change, or a generated symbol per part), the boxes can be dragged apart, and one placed near the page's edge runs past it, or where another device stands lands on it (a CPU's second box on a valve terminal's); a port a corrected part adds appears only when the box is drawn again, and removed sockets can leave marks; a part cannot be removed from My parts in the window; EW-PLC-012's quick fix that opens the PLC section; a card with two channel kinds in one part; reading GSD and GSDML files into such a part (1.x in the plan). From the last persona pass: a device placed first on a bus page takes its tag from that page, and placing its black box on a schematic page later changes the tag to that page's without a word (-3A2 became -4A2): say it in the toast, or keep the tag a device already has.
- FT-1768 `ew-api`: `centred_datum` (`crates/ew-api/src/output/layout_edit.rs`, where an enclosure is placed) takes the frame's drawing area, which `drawing_area` already gives in page coordinates, and maps it through the frame's page-to-sheet map once more: on a frame with an offset the mounting plate stands off-centre. The valve-terminal overview had the same fault and is fixed (`output/rack.rs`); a test on a frame with an offset shows the plate's place.
- FT-1761 `ew-viewmodel, ew-ui`: From the persona re-test: the Bus table's Termination cell is free text with no hint of its words (a choice, or a hint under the editor); EW-POT-009's message says to mark a pin not used and neither Properties nor the message's row offers it (it is in the pin's "…" menu); the first click on a ribbon button right after a grid cell edit only ends the edit. What the frame check of M3's window found and its fix lane left: a table that does not widen its column or fill its panel at 2560 × 1440 (the Bus, Devices and Medium tables); the Medium table's count against the navigator's badge ("1 lines"); a grid tooltip above a dialog's backdrop; a line's Properties without its medium; a "generated" chip on every row of a report page's Properties; the Properties header cut by its chips at 1920; the symbol browser opening on a disabled row; lower-case preview rows, zero counts and "Xrefs"; a medium's category "Electrical"; the Mapping dialog at the top of the window; context menus that cover their row, and grey entries without a reason outside the navigators' row menus (those show it under the entry now). From the last persona pass: the check's toast has no way to the Messages list.
- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Work that still runs on the window's thread: an EDZ's and a parts XML's reading (twice), a PLC import's read and merge, and a report spreadsheet becoming a project; each needs its session-bound step split out first. The drawing import with its DXF conversion is a job, and the rack view is made once per revision, since M3's reviews.
- FT-1764 `ew-ui`: A test the window's code review asked for: what `a_smaller_window_stays_usable` really proves (its label query matches any node; it checks the command bar and the page's size only).

### What the second candidate left

- FT-1771 `ew-viewmodel, ew-ui`: An editor for a part's drawn front view (`mech.front_2d`): the part editor's Size and mounting section edits only the rectangle the layout draws.
- FT-1772 `ew-api, ew-viewmodel`: The printed columns of a legend that exists already: a block made before `ReportBlock.columns` keeps the report's four columns (position, tag, quantity, part number), and nothing offers it the description.
- FT-1779 `ew-viewmodel, ew-ui`: A wire's Potential row in Properties opens Define potential, as the wire's menu does.
- FT-1780 `ew-scene, ew-viewmodel`: A selected wire marks its two ends.
- FT-1782 `ew-scene`: The legend's part-number column is shortened and, last, left out where the legend has no room: decide whether the legend wraps its texts or asks for a wider box. An enclosure's legend still shows "…" in its quantity and part-number columns (the embedded table's own cutting rule), and its header and drop shadow are heavy.
- FT-1785 `ew-viewmodel, ew-ui`: Dragging devices onto a location's group in the Devices list gives them that location; Location… does it by menu.
- FT-1786 `ew-scene`: A knock-out for a text under a wire on schematic pages: a text that a route crosses and that does not move is drawn under the line, and no renderer clears the line round it (ADR-0033, amendment 2).
- FT-1787 `ew-ui, xtask`: The deep run's replay of every script (`nightly_every_script_transcript_is_byte_identical_twice`) runs close to its time limit; split it or give it room. Not measured.

### What the third candidate left

- FT-1788 `ew-viewmodel, ew-ui`: A PLC module's or a bus device's device box opens read-only, as its pins come from its channels and its bus interfaces; edit them in the box itself.
- FT-1789 `ew-viewmodel`: The symbol editor draws a shape by the pointer only; draw a line, a rectangle or a circle from the keyboard alone (accessibility), as a placed shape already moves with the arrow keys.
- FT-1790 `ew-scene, ew-viewmodel`: Round handles show for one selected wire; with several selected, none does. Decide whether handles of several wires help or only crowd.
- FT-1791 `ew-viewmodel, ew-api`: The spacing between the terminals of a row is remembered by the open project only: keep it between sessions (a user setting, or a project setting, which is a format change).
- FT-1796 `ew-viewmodel`: A drag that starts on a pin's label of a symbol moves the symbol; the tag, a box's caption and the texts of one selected symbol are dragged by themselves already.
- FT-1798 `ew-scene`: An interruption point on the bottom pin of a box draws its arrow along the box's edge.
- FT-1799 `ew-api`: An unnumbered wire's Properties title reads "?" (`wire_number` falls back to it); say "wire without a number" in words.
- FT-1802 `ew-viewmodel, ew-api`: Auto-fill rail gives every device without a location the cabinet's and places them all, field devices too (sensors, cylinders, a valve terminal); let the person choose which devices go on the rail.
- FT-1803 `ew-viewmodel`: Project properties name their rows otherwise than the title block's cells (Project reference and Project, Designer and Author), and Document title prints in no cell of the program's frame (a persona met this again); Title block fields… says which cell prints which, the rows themselves do not.
- FT-1805 `ew-api`: The terminal diagram lists terminal numbers without a column for the strip, so two strips' terminals 1 read alike.
- FT-1806 `ew-viewmodel`: A tool stands at another place on another tab of the command bar (Wire on Draw, another button at that place on PLC); a tool a person uses on several tabs keeps its place.
- FT-1807 `ew-api, ew-viewmodel`: A template cannot be applied to a project that exists (its sections, text styles, check level); only a new project starts from one.
- FT-1808 `ew-viewmodel`: Placing a part that has no symbol arms the Black box tool without saying that the part is drawn as a box.
- FT-1809 `docs/book`: A property's help page often repeats the row's label (the project's Control voltage); say what the program does with the value, its unit and an example.
- FT-1810 `ew-ui`: The symbol editor's start card gets a "Carry on with…" row on top after a save, which moves every entry down; Copy a library symbol is one long list without order or preview; the pin list of a device box sorts itself again after an edge is changed.
- FT-1811 `ew-ui, ew-api`: A pin's place typed between two steps of the pin grid is taken to the next step without a word; a copy of a starter part keeps the starter's part number without saying so.
- FT-1812 `ew-viewmodel`: F1 on a selected connection point in the symbol editor opens the outline's section although the guide has a section for a connection point.
- FT-1813 `ew-viewmodel`: The Place tool shows "-?X?:1" for a new strip until the pointer moves; the project's Kind row shows a dash.

### What the fourth candidate left

- FT-1817 `ew-commands, ew-api`: After a page's grid change or a paste fitted to another grid each item goes to its nearest grid point, one by one, and two symbols can end on one another (EW-CORE-077 reports it; the texts are placed by the text placer). Part what the fit pushed together.
- FT-1818 `ew-viewmodel`: With a fitted symbol (its first pin on the grid, its position between grid points) the first arrow key moves the ghost a part of a step in the readout, while the move itself is whole steps.
- FT-1819 `ew-commands, ew-viewmodel`: A command's extra notices read in English in a Dutch window: `Outcome.notices` is a list of strings. Carry a text through `Outcome` (a box reshape's moved pin, a paste's notices).
- FT-1820 `ew-commands, ew-derive`: An earth symbol takes a number from the cables' W counter (its tag is hidden on the page and used in the wire list), so a project's first cable can be W2 and a project can hold two devices tagged W1. Decide the symbol's own letter, or give it no tag.
- FT-1821 `ew-derive, ew-checks`: The rule that a wire from 0 V to earth is the bond looks at one page only (pin roles, defined types and names count): a 0 V that comes in through an interruption point from another page still needs Draw ▸ Connect ▸ Bond.
- FT-1822 `ew-viewmodel, ew-ui`: Properties builds its rows again on every frame with a device selected. Not measured.
- FT-1823 `ew-viewmodel`: The overview page that a station's To do proposes takes a folio in the wiring section.
- FT-1824 `ew-ui`: A toast can cover a To do button of the station view.
- FT-1826 `content, ew-api`: There is no A3 form among the starter frames and title blocks.
- FT-1827 `ew-reports`: The potential overview numbers its bond rows by id order, so a drawn bond can shift the numbers of the declared ones.
- FT-1829 `ew-ui`: The Text styles page builds a small list on every frame. Not measured.
- FT-1830 `ew-viewmodel, ew-ui`: Two persona runs expected the mouse wheel to pan and met a wheel that zooms; View ▸ Mouse wheel scrolls makes it pan, but only the zoom's tooltip says so. Offer the choice where the zoom is first met, and give the page scroll bars.
- FT-1831 `ew-scene, ew-derive`: A black box's caption stands above it, left of its first top pin, exactly where the wires and tubes into those pins pass, so they loop round it (the loops on the persona pages). Place the caption clear of the pins' ways, so that the wires can run straight.
- FT-1833 `ew-commands, ew-api`: A paste fitted to another grid or a grid change can leave two pins so close that a wire finds no way (the message counts them, and Route after moving the items apart mends it). Let the fit leave room, so that every wire finds a way.
- FT-1834 `ew-derive, ew-viewmodel`: A new wire takes the control size, 0.75 mm², unless its potential names another circuit class, so a 16 A mains wire gets it. Propose the cross-section from the circuit.
- FT-1835 `ew-print`: The test `a_cancelled_print_aborts_before_its_first_band` fails with "The handle is invalid" while other builds run and passes alone. Find what it shares with a build that runs beside it.
- FT-1836 `ew-viewmodel, ew-scene`: While a wire's pin name is asked (a wire drawn to a box's edge where no pin is), the wire's ghost is hidden; show it until the name is given.
- FT-1837 `ew-drafting`: Tubes from a wide valve terminal with upward ports to a cylinder below are long, because every clear way is.
- FT-1838 `ew-ui, ew-viewmodel`: F2 does not open a channel's cell in the station view's channel table; a click does.
- FT-1839 `ew-viewmodel`: The left and right sides of a drawn box have no smallest height by their pin texts, only one step per pin; the top and bottom sides have.
- FT-1840 `content`: There is no proximity or reed sensor among the starter parts: a three-wire PNP sensor with its symbol.
- FT-1841 `ew-viewmodel`: The strip rule of letter, page and counter (-X1021) as the wizard's default is the reference project's convention; look at it again when the golden project is retired (FT-1816).
- FT-1842 `ew-reports, ew-derive`: A list whose last page would hold one or two rows.
- FT-1843 `ew-api, ew-viewmodel`: A panel layout has no door and no front panel: a device mounted in the door (a pushbutton, a pilot lamp, a display) can only be dragged onto a rail, where it lands as "unknown size". Design the door as a surface of the enclosure that holds such devices with their cut-outs.
- FT-1844 `ew-api, ew-scene`: The Checker of the project's data is not the title block's Checked cell: Checked prints the checker who approves the current revision, so a person who fills the Checker row finds Checked empty. Decide whether the row feeds the cell, and say which does in Title block fields….
- FT-1845 `ew-viewmodel, ew-ui`: A part search for "sensor" finds nothing, "normally open contact" ranks the contactor first, and the first row of a part search is selected before the person chose it, so Enter takes it. Search by what a part is for as well as by its words, and preselect nothing.
- FT-1846 `ew-viewmodel, ew-api`: Paste and fill-down in the PLC grid's Function text column say "no command yet"; type each cell. Let both act on the wired device's function text, as typing does.

### What the drafting redesign left

Accepted by the owner as built on 2026-11-20 (the sixth candidate); these are what the builder and the reviews of
its pages left open (ADR-0034, `docs/design/06a-drafting.md`).

- FT-1848 `ew-units, ew-drafting`: A second press of Arrange and route on a page of the QET grid may still route a few wires: a text's box is one tick larger or smaller according to where its symbol stands, so the stored page is not the plan's page. A tolerance in `Grid::tick_span` mends it and moves texts on every page of that grid (reference output, snapshots and tutorials regenerate).
- FT-1849 `ew-drafting`: Route weighs a kept route per net, not per wire: on a page with an older two-row card its plan is two crossings worse than the page was, because one wire at a supply mark loses the person's route.
- FT-1850 `ew-viewmodel`: The Arrange and route card, after rows are unticked, still lists a few wire rows that Route would redraw; find whether they belong there (the test's loop bound went from 8 to 64 without the cause).
- FT-1851 `ew-drafting`: On a page with cards a feed terminal without a channel stays in the middle of its strip and its wires cross the channels; it wants a place at the strip's end.
- FT-1852 `ew-drafting`: A card does not step aside where another card's used columns interleave with its own; the columns should be chosen by the channels in use. "Draw on a page" still sets a card of inputs under the controller, so supply wires cross its first channels until Arrange and route moves it.
- FT-1853 `ew-drafting`: Wires that go round a body between two rows of pins (an older two-row card, a box with pins on two sides) are searched one by one and cross each other; lay them side by side in nested tracks, with the placer opening the gap they need. It removes no crossing the strip's order forces.
- FT-1854 `ew-drafting`: Seen on single pages and not looked into: after Arrange and route a supply page gains one text on a text and a small zigzag between two pins a step apart; two wires that get their first route add three crossings; a wire goes round a part where the part could stand in line; wires across texts on a page with an older card.
- FT-1855 `ew-commands, ew-viewmodel`: An older two-row card is drawn in the new form only by deleting its box and drawing it again from the station view; terminals that then face the other way are rewired by hand. A command that redraws a card in place and keeps its wiring.
- FT-1856 `ew-scene, ew-api`: A card drawn in cells with level texts (address and function text per channel), as the pilot's reference set draws it; the pin texts of a drawn box stay turned a quarter until then. The bevel junction's symbol is a quarter step where two wires parting at a pin take half a step: one size.
- FT-1857 `ew-api`: The acceptance report commits the SVG export's digest, a copy of what the code produces; every change to a drawn line regenerates it by hand (and its shards race when blessed together).

### What the first production job found

A real drawing set of the pilot was brought in from QElectroTech through the text interface on the sixth candidate
of 0.3.0, made to follow an end user's drawing specification and doubled for a second, identical installation.
What the program could not do, or did wrongly, most serious first.

- FT-1858 `ew-derive, ew-scene, ew-reports, ew-viewmodel`: A wire's frozen number is stored and never printed: the page, the wire list and the SQL view keep the automatic number, Properties shows "automatic · frozen", and typing the number into its row is refused as "not strict JSON". There is no setting for a numbering scheme by potential (one code per potential, a circuit number in front), which is what such a specification asks: the job carried the codes on potentials and set them as free texts beside the wires.
- FT-1859 `ew-import-qet, ew-commands, ew-viewmodel`: The QElectroTech import writes a title block's values (function, location, object, drawing number, description, revision, dates) into the form as fixed texts, where the project's and the page's fields should print. No action edits a form, a project-local form is refused by `library.embed`, and a report block's form cannot be set; the job replaced the forms by hand outside the program.
- FT-1860 `ew-commands, ew-viewmodel`: A device cannot take a second symbol of its own from the window or the text interface (`place.pick` with a library symbol and a device asks for a symbol; `rep.join_device` takes contacts only; `ew import parts` refuses a function definition of the project's library). Only the model command `device.place` with `join_device` does it.
- FT-1861 `ew-import-qet`: The import's device model of a wired cabinet: junction elements and earth symbols become terminals of inferred strips, unlabelled terminals on a strip's second folio do not join it, cable marks become plain devices instead of cables with cores, and routes on one folio were dropped. On the job's project: eight strips for three, about twice the terminals, no cable for three.
- FT-1862 `ew-commands, ew-viewmodel`: No copy of a page that keeps what belongs together (coil and contacts, a strip's terminals, a cable's cores) and renumbers by a rule; paste hangs by its middle and cannot land in place, and is refused when one text lies in the header band where the Text tool put it; `macros.save` with pages says nothing is selected. A second, identical installation was made by running the building script twice.
- FT-1863 `ew-export, ew-reports, ew-scene`: The sheet is A4: a set that must be A3 needs frame forms made for it, `ew print --fit` never enlarges, and the list pages keep an A4 frame of their own, with English column heads in a project whose language is Dutch; a terminal diagram's rows do not name their strip.
- FT-1864 `ew-drafting, ew-commands`: A straight wire drawn one step above a row of symbols is rerouted round them without a word, and the taps on it then hang free.
- FT-1865 `ew-commands`: Small faults of commands: `cable.from_wires` ignores `cores` given as a list (a string works) and assigns the first colours of another cable; `page.delete` with `delete_devices` leaves the emptied strips, and `strip.new` with a tag that exists makes a second strip silently; `layout.enclosure_new` refuses a part of the project's library as "not on offer"; a library symbol is addressed as `library:<id>:<id>` before its first placement and by its key after, and each form is refused in the other state; a part on the starter power-supply function with a symbol of its own is refused by the Place tool ("place it with box.black") and by the Black box tool ("not a black box").
- FT-1866 `ew-store, ew-viewmodel`: The journal lives outside the project folder and is offered to another project that is later made at the same path.
- FT-1867 `ew-scene, ew-derive`: An external point prints its location and device, not the connection it stands for; technical data has no unit `mA`, and a coil's data prints through the coil's wire; the earth symbol's letter collides with cable tags and changing it is EW-RDS-003; a 0 V potential bonded to an earth symbol is typed PE as a whole.
- FT-1868 `ew-viewmodel, ew`: The text interface: no action prints the action list; `report_block.new` offers words in its error and takes keys; page rows cannot be reached after `props.project`; `ew import qet --out a/b.ew` with a missing folder `a` imports and then writes nothing.
- FT-1869 `ew-import-qet, ew-commands, ew-viewmodel`: A device that is given a real part after an import does not keep the look of the drawing it came from: the job's schematic pages were drawn anew with starter symbols and symbols made in the editor, and the owner found them nothing like the source's (owner, 2026-11-20). An imported element must be usable as the symbol of a device with a part (its pins bound to the part's), so that a set brought in keeps its symbols while it gains devices, strips and cables.
- FT-1870 `ew-drafting, ew-scene`: Seen on the job's pages: a mark set at the head of a wire leaves a small hook in the wire where the mark's pin is not on the wire's line; the tags and cross-references of like devices in one row stand at different places from device to device (each text is placed alone; a row should take one place for all); a code written as a free text beside a wire lies on the wire.
- FT-1871 `ew-ui, ew-scene`: A symbol drawn in the symbol editor with pins on its lower edge prints the pin texts across the pins, and a text of the inner drawing above its box; the editor shows neither before the symbol stands on a page.

### The development loop

- FT-1774 `xtask`: The compile cache shared between worktrees. Tried on 2026-11-16: no plain setting does it with sccache 0.19 (`SCCACHE_BASEDIRS` is read for C and C++ only, a remap in the flags enters the cache key, `trim-paths` is unstable). A small rustc wrapper behind sccache that remaps the build folder for crates with an `OUT_DIR` took the misses of a second worktree from 77 to 24 of 266 and about 35 s (16 %) off a lane's first build; it was not installed, because it sits in the machine's cargo settings for every project. Look again when sccache releases Rust base directories and its new proc-macro hashing. Also seen: a worktree inside the main checkout reads both `.cargo/config.toml` files and passes the linker flag twice, so the main checkout and the lanes never share cache entries.
- FT-1775 `xtask, ew-ui`: One build flavour for window checks, so that the window tests, the pictures and the lint share one build of the heavy crates.
- FT-1776 `xtask`: The linter as the test build's wrapper, so that one compile serves the lint and the tests.

## M4

Asked for by the owner at his test of 0.3.0's fourth candidate, for the next milestone. Each starts with a design
from the user's side (who changes it, when, what he sees while he does it) before anything is built.

- FT-1814 `ew-api, ew-viewmodel, ew-ui, ew-scene`: The title block cannot be changed in the window: its layout, extra fields of the person's own, and pages that have their own title block (a cover page, a divider). Design first: an editor for a title block that a person who has never seen a form editor can use, what a template carries of it, and how a running project takes a changed one.
- FT-1815 `ew-api, ew-viewmodel, ew-ui, ew-scene`: The frame's rulers (the numbers along the top, the letters along the left) cannot be changed: how many columns and rows a page has, numbers or letters on either side, where counting starts. Design first, together with FT-1814: cross-references and the page's grid depend on the columns and rows.
- FT-1847 `ew-reports` (WP-S5.07): `tubes.json` of the production package, with the tube rows of `cutlists.csv`, comes with the package (doc 13 §8.1); the tube cut list's allowance and order increment are M5's (FT-1732).

### Before M4's new work: the golden project is retired

- FT-1816 `golden, ew-api, ew-viewmodel, ew-ui, ew-derive, ew-commands, ew-testkit, xtask, docs`: Retire the golden project (owner, at the fourth candidate of 0.3.0: it was the result to build towards, the program can now make such a cabinet itself, and the persona runs do its job better). About four hundred test files and a hundred and sixty documents name it. Design first: which tests end with it (the acceptance specification of doc 20, the regression tests of the reference drawing's defects, the gates), and which only use it as "a whole cabinet" and move to a cabinet made in the program (generic parts, no origin outside the repository: electrical, PLC and bus, fluid, layout, reports), small enough to read. Then `golden/`, its blessing route, the rule in `CLAUDE.md` and doc 20 go. Since the fifth candidate of 0.3.0 the golden project no longer holds a design back: a decided change regenerates it.

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
