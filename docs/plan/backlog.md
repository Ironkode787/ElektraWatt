# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.



### What the migration verifier found (WP-S3.13)

- FT-1746 `ew-import-qet, ew-scene`: **Medium.** An imported QElectroTech text stands too high against QET's own PDF, a fixed text by about 0.45 to 0.5 of its height: doc 14 §4.10's rule "x y is the baseline" does not hold for QET's texts (the anchor is the baseline less the descent, plus an offset of QET's), and the font's ascent alone does not explain it. A change of the rule moves the 222 generated QET-10 symbols and the golden files, so it is measured first: it waits for a PDF exported by QET of a pilot project in the private corpus folder.

### The window: what its fix lane left

- FT-1761 `ew-viewmodel, ew-ui`: What the frame check of M3's window found and its fix lane left: a table that does not widen its column or fill its panel at 2560 × 1440 (the Bus, Devices and Medium tables); the Medium table's count against the navigator's badge ("1 lines"); the rack editor's refusal at the dialog's foot and the dialog's jump; a grid tooltip above a dialog's backdrop; forms without a hint or an example (symbol policy, process-tag scheme, fluid settings, mode and safety); a line's Properties without its medium; a "generated" chip on every row of a report page's Properties; the Properties header cut by its chips at 1920; the symbol browser opening on a disabled row; lower-case preview rows, zero counts and "Xrefs"; a medium's category "Electrical"; the Mapping dialog at the top of the window; context menus that cover their row and grey entries without a reason.
- FT-1763 `ew-viewmodel, ew-ui, ew-api`: Work that still runs on the window's thread: a DXF's parse at Apply, an EDZ's and a parts XML's reading (twice), a PLC import's read and merge, a report spreadsheet becoming a project, and the rack view with its slot check twice a frame while the editor is open. The drawing import's read, probe and DWG conversion are a job since M3's review.
- FT-1764 `ew-ui`: Two tests the window's code review asked for: the rack and slot editor's refusal of a slot (EW-PLC-011), and what `a_smaller_window_stays_usable` really proves (its label query matches any node; it checks the command bar and the page's size only).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
