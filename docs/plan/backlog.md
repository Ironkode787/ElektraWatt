# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M3

Rows that a work package of [m3.md](m3.md) takes. The lane checks each row first: a row that the code already
answers is deleted with a line in its report.



### What the migration verifier found (WP-S3.13)

- FT-1746 `ew-import-qet, ew-scene`: **Medium.** An imported QElectroTech text stands about 0.6 mm above where QET's own PDF has it, rotated texts by the same amount: on one pilot project the verifier calls a third of the texts moved. Most likely the font's ascent in the baseline rule of doc 14 §4.10; find the cause and put the texts where QET has them.
- FT-1747 `ew-export`: **Medium.** A text read back from a PDF the program wrote has lost its `fi` and `fl` ligatures ("Denition"): FR-OUT-008 expects every text to be found and copied. Confirm it in the writer (the glyph-to-text map of a ligature) and fix it.
- FT-1748 `ew-api`: The verifier pairs pages by order when the PDF's labels do not name every folio, so one missing page shifts every pair after it; let a run state its pairs or align them. AC-27 on the private Golden C waits for it and for its reference PDF in the corpus folder.
- FT-1749 `golden`: `expected/c-migration-report.csv` (AC-25, the private Golden C import) is stale since M2.5-Fix: the deep run skips its test without the private corpus. Look at each changed count (EW-IMP-004 above all) before blessing it.

### WP-S2.13: automation workspaces

- FT-1757 `ew-viewmodel, ew-ui`: A fluid interruption point has its definition and forms (`ew.fn.ip-fluid`, `ew.iso.ip`) and no tool in the window: let the interruption-point tool place the fluid form on a fluid page.
- FT-1753 `ew-viewmodel, ew-ui, ew-api`: **Medium.** A drawing import runs on the window's thread: the probe of a DXF when the dialog opens, and a DWG's conversion for up to 120 seconds, during which the window does not answer. Run it as a job with its progress and Cancel, as the exports do.
- FT-1756 `ew-viewmodel, ew-ui`: The PLC import's preview offers "take the file's" on a comment conflict that cannot be taken (one side has languages, the other none); the command then refuses in words. Do not offer it, and let the refusal `refusal-plc-exchange-not-acceptable` state the reason that applies.

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
