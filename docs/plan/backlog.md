# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.

### What M3's reviews left

- FT-1765 `ew-import-qet, ew-scene`: What still stands apart from QElectroTech's own PDF after FT-1746 put the static texts on QET's baseline: the texts of title-block cells (0.5 to 2.7 mm sideways) and free texts of 12 pt and more (0.7 to 1.2 mm high). On one pilot project 730 of about 4,800 texts.
- FT-1766 `ew-api, ew-commands, ew-viewmodel, ew-ui`: What is left of "your own PLC and bus parts" (owner, 2026-11-15) after New part, its forms and the correction of a saved part were built: a device already drawn keeps the ports it was drawn with after its part is corrected (offer to redraw it), and a bus line on a port the part no longer has is not reported at the update; a part cannot be removed from My parts in the window; EW-PLC-012's quick fix that opens the PLC section; a card with two channel kinds in one part; reading GSD and GSDML files into such a part (1.x in the plan).
- FT-1761 `ew-viewmodel, ew-ui`: From the persona re-test: the Bus table's Termination cell is free text with no hint of its words (a choice, or a hint under the editor); EW-POT-009's message says to mark a pin not used and neither Properties nor the message's row offers it (it is in the pin's "…" menu); the first click on a ribbon button right after a grid cell edit only ends the edit. What the frame check of M3's window found and its fix lane left: a table that does not widen its column or fill its panel at 2560 × 1440 (the Bus, Devices and Medium tables); the Medium table's count against the navigator's badge ("1 lines"); the rack editor's refusal at the dialog's foot and the dialog's jump; a grid tooltip above a dialog's backdrop; forms without a hint or an example (symbol policy, process-tag scheme, fluid settings, mode and safety); a line's Properties without its medium; a "generated" chip on every row of a report page's Properties; the Properties header cut by its chips at 1920; the symbol browser opening on a disabled row; lower-case preview rows, zero counts and "Xrefs"; a medium's category "Electrical"; the Mapping dialog at the top of the window; context menus that cover their row and grey entries without a reason.
- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Work that still runs on the window's thread: an EDZ's and a parts XML's reading (twice), a PLC import's read and merge, and a report spreadsheet becoming a project; each needs its session-bound step split out first. The drawing import with its DXF conversion is a job, and the rack view is made once per revision, since M3's reviews.
- FT-1764 `ew-ui`: Two tests the window's code review asked for: the rack and slot editor's refusal of a slot (EW-PLC-011), and what `a_smaller_window_stays_usable` really proves (its label query matches any node; it checks the command bar and the page's size only).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
