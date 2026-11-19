# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.

### What M3's reviews left

- FT-1765 `ew-import-qet, ew-scene`: What still stands apart from QElectroTech's own PDF after FT-1746 put the static texts on QET's baseline: the texts of title-block cells (0.5 to 2.7 mm sideways) and free texts of 12 pt and more (0.7 to 1.2 mm high). On one pilot project 730 of about 4,800 texts.
- FT-1766 `ew-api, ew-commands, ew-viewmodel, ew-ui`: What is left of "your own PLC and bus parts" (owner, 2026-11-15): a device with several interfaces is one box per interface on a bus page (one box needs a box connection point that names its function, a format change, or a generated symbol per part), the boxes can be dragged apart and one placed near the page's edge runs past it; a port a corrected part adds appears only when the box is drawn again, and removed sockets can leave marks; a part cannot be removed from My parts in the window; EW-PLC-012's quick fix that opens the PLC section; a card with two channel kinds in one part; reading GSD and GSDML files into such a part (1.x in the plan). From the last persona pass: a device placed first on a bus page takes its tag from that page, and placing its black box on a schematic page later changes the tag to that page's without a word (-3A2 became -4A2): say it in the toast, or keep the tag a device already has.
- FT-1768 `ew-api`: `centred_datum` (`crates/ew-api/src/output/layout_edit.rs`, where an enclosure is placed) takes the frame's drawing area, which `drawing_area` already gives in page coordinates, and maps it through the frame's page-to-sheet map once more: on a frame with an offset the mounting plate stands off-centre. The valve-terminal overview had the same fault and is fixed (`output/rack.rs`); a test on a frame with an offset shows the plate's place.
- FT-1761 `ew-viewmodel, ew-ui`: From the persona re-test: the Bus table's Termination cell is free text with no hint of its words (a choice, or a hint under the editor); EW-POT-009's message says to mark a pin not used and neither Properties nor the message's row offers it (it is in the pin's "…" menu); the first click on a ribbon button right after a grid cell edit only ends the edit. What the frame check of M3's window found and its fix lane left: a table that does not widen its column or fill its panel at 2560 × 1440 (the Bus, Devices and Medium tables); the Medium table's count against the navigator's badge ("1 lines"); a grid tooltip above a dialog's backdrop; a line's Properties without its medium; a "generated" chip on every row of a report page's Properties; the Properties header cut by its chips at 1920; the symbol browser opening on a disabled row; lower-case preview rows, zero counts and "Xrefs"; a medium's category "Electrical"; the Mapping dialog at the top of the window; context menus that cover their row and grey entries without a reason. From the last persona pass: the check's toast has no way to the Messages list.
- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Work that still runs on the window's thread: an EDZ's and a parts XML's reading (twice), a PLC import's read and merge, and a report spreadsheet becoming a project; each needs its session-bound step split out first. The drawing import with its DXF conversion is a job, and the rack view is made once per revision, since M3's reviews.
- FT-1764 `ew-ui`: A test the window's code review asked for: what `a_smaller_window_stays_usable` really proves (its label query matches any node; it checks the command bar and the page's size only).

### What the second candidate left

- FT-1769 `ew-api, ew-viewmodel`: A valve terminal part that comes in several sizes is not asked for its size when it is placed: the number of positions is the part's, and a part that names its sizes changes the part record, which the format grows only additively (decide how a part names them first).
- FT-1770 `ew-viewmodel, ew-ui`: The part editor has no controls for a valve terminal's kind (bus node or multipin) or for its pin map (which plug pin drives which solenoid); the starter parts carry them.
- FT-1771 `ew-viewmodel, ew-ui`: An editor for a part's drawn front view (`mech.front_2d`): the part editor's Size and mounting section edits only the rectangle the layout draws.
- FT-1772 `ew-api, ew-viewmodel`: The printed columns of a legend that exists already: a block made before `ReportBlock.columns` keeps the report's four columns (position, tag, quantity, part number), and nothing offers it the description.
- FT-1773 `ew-ui`: A wiring test of the multipin valve terminal's plug in the window: a PLC output wired to a plug pin by clicks drives the solenoid at session level only; no window test does it by clicks.
- FT-1779 `ew-viewmodel, ew-ui`: A wire's Potential row in Properties opens Define potential, as the wire's menu does.
- FT-1780 `ew-scene, ew-viewmodel`: A selected wire marks its two ends.
- FT-1782 `ew-scene`: The legend's part-number column is shortened and, last, left out where the legend has no room: decide whether the legend wraps its texts or asks for a wider box. An enclosure's legend still shows "…" in its quantity and part-number columns (the embedded table's own cutting rule), and its header and drop shadow are heavy.
- FT-1783 `ew-viewmodel, ew-ui`: A tube's size is chosen in the Tube tool's bar, beside its line kind; it takes the project's default now.
- FT-1784 `ew-viewmodel`: A Blanking plate row in the device browser of a valve terminal's cell, beside the valves made for a terminal; the cell's menu has Blanking plate at position.
- FT-1785 `ew-viewmodel, ew-ui`: Dragging devices onto a location's group in the Devices list gives them that location; Location… does it by menu.
- FT-1786 `ew-scene`: A knock-out for a text under a wire on schematic pages: a text that a route crosses and that does not move is drawn under the line, and no renderer clears the line round it (ADR-0033, amendment 2).
- FT-1787 `ew-ui, xtask`: The deep run's replay of every script (`nightly_every_script_transcript_is_byte_identical_twice`) runs close to its time limit; split it or give it room. Not measured.

### What the third candidate left

- FT-1788 `ew-viewmodel, ew-ui`: A PLC module's or a bus device's device box opens read-only, as its pins come from its channels and its bus interfaces; edit them in the box itself.
- FT-1789 `ew-viewmodel`: The symbol editor draws a shape by the pointer only; draw a line, a rectangle or a circle from the keyboard alone (accessibility), as a placed shape already moves with the arrow keys.
- FT-1790 `ew-scene, ew-viewmodel`: Round handles show for one selected wire; with several selected, none does. Decide whether handles of several wires help or only crowd.
- FT-1791 `ew-viewmodel, ew-api`: The spacing between the terminals of a row is remembered by the open project only: keep it between sessions (a user setting, or a project setting, which is a format change).
- FT-1793 `ew-api, ew-commands`: A valve stays undrawn when a valve terminal's positions are filled through `ew apply` without the symbols (`copies`); the command could find the symbols itself, or say so. Draw its valves in the valve terminal draws them.
- FT-1795 `ew-scene`: A tag keeps clear of a neighbouring symbol only around a half-turned symbol (the tag rule's own-symbol and half-turn cases; wider rules changed Golden A's pictures). Around a quarter turn or a nudge a tag can still lie on a neighbour's body or text.
- FT-1796 `ew-viewmodel`: A drag that starts on a pin's label of a symbol moves the symbol; the tag, a box's caption and the texts of one selected symbol are dragged by themselves already.
- FT-1797 `ew-derive`: Two routes that ADR-0033's measures leave: a PE wire crosses L and N where the way round a box is far longer, and a pin right under a row of tags has no route clear of the texts.
- FT-1798 `ew-scene`: An interruption point on the bottom pin of a box draws its arrow along the box's edge.
- FT-1799 `ew-api`: An unnumbered wire's Properties title reads "?" (`wire_number` falls back to it); say "wire without a number" in words.
- FT-1800 `ew-viewmodel, ew-ui`: With the Wire tool a connection point is about five screen points at the usual zoom, and a click a few points off starts a bend instead of ending the wire; the ring under the pointer is shown with Select only. Give the Wire tool the same ring and a reach in screen points.
- FT-1801 `ew-viewmodel`: A tube drawn in a project without a default tube stops for the project's fluid settings; offer the size where the tube is drawn.
- FT-1802 `ew-viewmodel, ew-api`: Auto-fill rail gives every device without a location the cabinet's and places them all, field devices too (sensors, cylinders, a valve terminal); let the person choose which devices go on the rail.
- FT-1803 `ew-viewmodel`: Project properties name their rows otherwise than the title block's cells (Project reference and Project, Designer and Author), and Document title prints in no cell of the program's frame; Title block fields… says which cell prints which, the rows themselves do not.
- FT-1804 `ew-scene`: Several small symbols placed at a PLC card's pin pitch (four sensors side by side) have tags, channel texts and pin numbers on one another; the placement rule keeps a text clear of its own symbol and of a few named cases, not of a close neighbour's texts.
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

- FT-1817 `ew-commands, ew-api`: After a page's grid change or a paste fitted to another grid each item goes to its nearest grid point, one by one, and two items can end on one another (two texts; a supply mark's name on its neighbour's arrow), because the commands do not know a text's drawn size. Part what the fit pushed together.
- FT-1818 `ew-viewmodel`: With a fitted symbol (its first pin on the grid, its position between grid points) the first arrow key moves the ghost a part of a step in the readout, while the move itself is whole steps.
- FT-1819 `ew-commands, ew-viewmodel`: A command's extra notices read in English in a Dutch window: `Outcome.notices` is a list of strings. Carry a text through `Outcome` (a box reshape's moved pin, a paste's notices).
- FT-1820 `ew-commands, ew-derive`: An earth symbol takes a number from the cables' W counter (its tag is hidden on the page and used in the wire list), so a project's first cable can be W2 and a project can hold two devices tagged W1. Decide the symbol's own letter, or give it no tag.
- FT-1821 `ew-derive, ew-checks`: The rule that a wire from 0 V to earth is the bond looks at one page and at pin roles only: a 0 V that comes in through an interruption point from another page still needs Draw ▸ Connect ▸ Bond.
- FT-1822 `ew-viewmodel, ew-ui`: Properties builds its rows again on every frame with a device selected. Not measured.
- FT-1823 `ew-viewmodel`: The overview page that a station's To do proposes takes a folio in the wiring section.
- FT-1824 `ew-ui`: A toast can cover a To do button of the station view.
- FT-1825 `ew-derive, ew-viewmodel`: A wire drawn one at a time with the Wire tool dodges its own device's movable tag.
- FT-1826 `content, ew-api`: There is no A3 form among the starter frames and title blocks.
- FT-1827 `ew-reports`: The potential overview numbers its bond rows by id order, so a drawn bond can shift the numbers of the declared ones.
- FT-1828 `ew-api, ew-viewmodel`: The part editor's default box and a part's stored box are not widened for upright pin texts, as the black box tool widens the boxes it lays out.
- FT-1829 `ew-ui`: The Text styles page builds a small list on every frame. Not measured.

### The development loop

- FT-1774 `xtask`: The compile cache shared between worktrees. Tried on 2026-11-16: no plain setting does it with sccache 0.19 (`SCCACHE_BASEDIRS` is read for C and C++ only, a remap in the flags enters the cache key, `trim-paths` is unstable). A small rustc wrapper behind sccache that remaps the build folder for crates with an `OUT_DIR` took the misses of a second worktree from 77 to 24 of 266 and about 35 s (16 %) off a lane's first build; it was not installed, because it sits in the machine's cargo settings for every project. Look again when sccache releases Rust base directories and its new proc-macro hashing. Also seen: a worktree inside the main checkout reads both `.cargo/config.toml` files and passes the linker flag twice, so the main checkout and the lanes never share cache entries.
- FT-1775 `xtask, ew-ui`: One build flavour for window checks, so that the window tests, the pictures and the lint share one build of the heavy crates.
- FT-1776 `xtask`: The linter as the test build's wrapper, so that one compile serves the lint and the tests.
- FT-1777 `ew-viewmodel`: Split the view-model crate: it sets the edit cycle (10 to 15 seconds of its compile on every change to it).

## M4

Asked for by the owner at his test of 0.3.0's fourth candidate, for the next milestone. Each starts with a design
from the user's side (who changes it, when, what he sees while he does it) before anything is built.

- FT-1814 `ew-api, ew-viewmodel, ew-ui, ew-scene`: The title block cannot be changed in the window: its layout, extra fields of the person's own, and pages that have their own title block (a cover page, a divider). Design first: an editor for a title block that a person who has never seen a form editor can use, what a template carries of it, and how a running project takes a changed one.
- FT-1815 `ew-api, ew-viewmodel, ew-ui, ew-scene`: The frame's rulers (the numbers along the top, the letters along the left) cannot be changed: how many columns and rows a page has, numbers or letters on either side, where counting starts. Design first, together with FT-1814: cross-references and the page's grid depend on the columns and rows.

### Before M4's new work: the golden project is retired

- FT-1816 `golden, ew-api, ew-viewmodel, ew-ui, ew-derive, ew-commands, ew-testkit, xtask, docs`: Retire the golden project (owner, at the fourth candidate of 0.3.0: it was the result to build towards, the program can now make such a cabinet itself, and the persona runs do its job better). About four hundred test files and a hundred and sixty documents name it. Design first: which tests end with it (the acceptance specification of doc 20, the regression tests of the reference drawing's defects, the gates), and which only use it as "a whole cabinet" and move to a cabinet made in the program (generic parts, no origin outside the repository: electrical, PLC and bus, fluid, layout, reports), small enough to read. Then `golden/`, its blessing route, the rule in `CLAUDE.md` and doc 20 go. Since the fifth candidate of 0.3.0 the golden project no longer holds a design back: a decided change regenerates it.

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
