# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.

### What M3's reviews left

- FT-1765 `ew-import-qet, ew-scene`: What still stands apart from QElectroTech's own PDF after FT-1746 put the static texts on QET's baseline: the texts of title-block cells (0.5 to 2.7 mm sideways) and free texts of 12 pt and more (0.7 to 1.2 mm high). On one pilot project 730 of about 4,800 texts.
- FT-1766 `ew-api, ew-commands, ew-viewmodel, ew-ui`: **Gate of the 0.3.0 candidate (owner, 2026-11-15).** A PLC or bus part cannot be made in the window (the owner asked, 2026-11-15): the part editor changes identity and technical values only. Wanted: New part; a PLC section (channels by kind and count, address ranges, the pin map) and a bus section (interfaces with bus, role and termination) as forms of the part editor; the symbol editor offering "make a part for this symbol". EW-PLC-012's quick fix then opens the PLC section.
- FT-1767 `ew-schema, ew-derive, ew-checks`: **Gate of the 0.3.0 candidate, with FT-1766.** A network switch is not modelled (an unmanaged one passes the net through; a managed one has many ports and one address), nor a generic PROFINET device that is no PLC: a port role or a part kind whose ports pass the net through and count as infrastructure in the Bus navigator, the bus overview and EW-BUS-006; N6 already walks through passive ports.
- FT-1761 `ew-viewmodel, ew-ui`: What the frame check of M3's window found and its fix lane left: a table that does not widen its column or fill its panel at 2560 × 1440 (the Bus, Devices and Medium tables); the Medium table's count against the navigator's badge ("1 lines"); the rack editor's refusal at the dialog's foot and the dialog's jump; a grid tooltip above a dialog's backdrop; forms without a hint or an example (symbol policy, process-tag scheme, fluid settings, mode and safety); a line's Properties without its medium; a "generated" chip on every row of a report page's Properties; the Properties header cut by its chips at 1920; the symbol browser opening on a disabled row; lower-case preview rows, zero counts and "Xrefs"; a medium's category "Electrical"; the Mapping dialog at the top of the window; context menus that cover their row and grey entries without a reason.
- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Work that still runs on the window's thread: an EDZ's and a parts XML's reading (twice), a PLC import's read and merge, and a report spreadsheet becoming a project; each needs its session-bound step split out first. The drawing import with its DXF conversion is a job, and the rack view is made once per revision, since M3's reviews.
- FT-1764 `ew-ui`: Two tests the window's code review asked for: the rack and slot editor's refusal of a slot (EW-PLC-011), and what `a_smaller_window_stays_usable` really proves (its label query matches any node; it checks the command bar and the page's size only).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
