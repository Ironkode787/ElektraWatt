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
- FT-1768 `ew-api`: Placing an enclosure maps the frame's drawing area, which is in page coordinates, back as if it were sheet millimetres (`crates/ew-api/src/output/layout_edit.rs`, where the enclosure is placed): on a page whose frame is scaled onto the sheet the enclosure stands off. The valve-terminal overview had the same fault and is fixed (`output/rack.rs`); a test on a new project's layout page shows whether this one bites.
- FT-1761 `ew-viewmodel, ew-ui`: From the persona re-test: the Bus table's Termination cell is free text with no hint of its words (a choice, or a hint under the editor); EW-POT-009's message says to mark a pin not used and neither Properties nor the message's row offers it (it is in the pin's "…" menu); the first click on a ribbon button right after a grid cell edit only ends the edit. What the frame check of M3's window found and its fix lane left: a table that does not widen its column or fill its panel at 2560 × 1440 (the Bus, Devices and Medium tables); the Medium table's count against the navigator's badge ("1 lines"); the rack editor's refusal at the dialog's foot; a grid tooltip above a dialog's backdrop; a line's Properties without its medium; a "generated" chip on every row of a report page's Properties; the Properties header cut by its chips at 1920; the symbol browser opening on a disabled row; lower-case preview rows, zero counts and "Xrefs"; a medium's category "Electrical"; the Mapping dialog at the top of the window; context menus that cover their row and grey entries without a reason. From the last persona pass: the Wire tool gives no sign when the pointer is on a connection point, and a click a few pixels beside a small pin is a bend; the check's toast has no way to the Messages list.
- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Work that still runs on the window's thread: an EDZ's and a parts XML's reading (twice), a PLC import's read and merge, and a report spreadsheet becoming a project; each needs its session-bound step split out first. The drawing import with its DXF conversion is a job, and the rack view is made once per revision, since M3's reviews.
- FT-1764 `ew-ui`: Two tests the window's code review asked for: the rack and slot editor's refusal of a slot (EW-PLC-011), and what `a_smaller_window_stays_usable` really proves (its label query matches any node; it checks the command bar and the page's size only).

### What the second candidate left

- FT-1769 `ew-api, ew-viewmodel`: A valve terminal part that comes in several sizes is not asked for its size when it is placed: the number of positions is the part's, and a part that names its sizes changes the part record, which the format grows only additively (decide how a part names them first).
- FT-1770 `ew-viewmodel, ew-ui`: The part editor has no controls for a valve terminal's kind (bus node or multipin) or for its pin map (which plug pin drives which solenoid); the starter parts carry them.
- FT-1771 `ew-viewmodel, ew-ui`: An editor for a part's drawn front view (`mech.front_2d`): the part editor's Size and mounting section edits only the rectangle the layout draws.
- FT-1772 `ew-api, ew-viewmodel`: The printed columns of a legend that exists already: a block made before `ReportBlock.columns` keeps the report's four columns (position, tag, quantity, part number), and nothing offers it the description.
- FT-1773 `ew-ui`: A wiring test of the multipin valve terminal's plug in the window: a PLC output wired to a plug pin by clicks drives the solenoid at session level only; no window test does it by clicks.

### The development loop

- FT-1774 `xtask`: The compile cache shared between worktrees. Tried on 2026-11-16: no plain setting does it with sccache 0.19 (`SCCACHE_BASEDIRS` is read for C and C++ only, a remap in the flags enters the cache key, `trim-paths` is unstable). A small rustc wrapper behind sccache that remaps the build folder for crates with an `OUT_DIR` took the misses of a second worktree from 77 to 24 of 266 and about 35 s (16 %) off a lane's first build; it was not installed, because it sits in the machine's cargo settings for every project. Look again when sccache releases Rust base directories and its new proc-macro hashing. Also seen: a worktree inside the main checkout reads both `.cargo/config.toml` files and passes the linker flag twice, so the main checkout and the lanes never share cache entries.
- FT-1775 `xtask, ew-ui`: One build flavour for window checks, so that the window tests, the pictures and the lint share one build of the heavy crates.
- FT-1776 `xtask`: The linter as the test build's wrapper, so that one compile serves the lint and the tests.
- FT-1777 `ew-viewmodel`: Split the view-model crate: it sets the edit cycle (10 to 15 seconds of its compile on every change to it).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
