# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.



### What the migration verifier found (WP-S3.13)

- FT-1746 `ew-import-qet, ew-scene`: **Medium.** An imported QElectroTech text stands too high against QET's own PDF, a fixed text by about 0.45 to 0.5 of its height: doc 14 §4.10's rule "x y is the baseline" does not hold for QET's texts (the anchor is the baseline less the descent, plus an offset of QET's), and the font's ascent alone does not explain it. A change of the rule moves the 222 generated QET-10 symbols and the golden files, so it is measured first: it waits for a PDF exported by QET of a pilot project in the private corpus folder.

### WP-S2.13: automation workspaces

- FT-1757 `ew-viewmodel, ew-ui`: A fluid interruption point has its definition and forms (`ew.fn.ip-fluid`, `ew.iso.ip`) and no tool in the window: let the interruption-point tool place the fluid form on a fluid page.
- FT-1753 `ew-viewmodel, ew-ui, ew-api`: **Medium.** A drawing import runs on the window's thread: the probe of a DXF when the dialog opens, and a DWG's conversion for up to 120 seconds, during which the window does not answer. Run it as a job with its progress and Cancel, as the exports do.
- FT-1756 `ew-viewmodel, ew-ui`: The PLC import's preview offers "take the file's" on a comment conflict that cannot be taken (one side has languages, the other none); the command then refuses in words. Do not offer it, and let the refusal `refusal-plc-exchange-not-acceptable` state the reason that applies.

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
