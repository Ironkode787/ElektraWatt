# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M2.5-Fix: a properly working program

Fixes only. The gate is the owner's own hands-on test: the wizard, a new project, placing, wiring, routing, the
symbol editor and the QET import work without anything that stops the work. Its release is 0.2.0.

### The owner's hands-on test of 0.1.0-rc.3

- FT-1645 `ew-import-qet, ew-scene`: What an imported QET folio still shows differently, after the full-fidelity work of M2.5-Fix: a master's contact list has no contact symbols and its rows stand closer than QET's 10 px (the scene has no list drawing and the text style no line height); `displayhas="cross"` and `snapto="bottom"` cross-references print as a list, with a report entry; QET's folio list arrives as EW's own table of contents; an element's `mirror` is not read (QET 0.100 writes none; newer versions do); fonts are substituted, so texts sit slightly lower.

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
