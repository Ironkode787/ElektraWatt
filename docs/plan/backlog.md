# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M2.5-Fix: a properly working program

Fixes only. The gate is the owner's own hands-on test: the wizard, a new project, placing, wiring, routing, the
symbol editor and the QET import work without anything that stops the work. Its release is 0.2.0.

### The owner's hands-on test of 0.1.0-rc.3

- FT-1645 `ew-import-qet, ew-scene`: What an imported QET folio still shows differently, after the full-fidelity work of M2.5-Fix: a master's contact list has no contact symbols and its rows stand closer than QET's 10 px (the scene has no list drawing and the text style no line height); `displayhas="cross"` and `snapto="bottom"` cross-references print as a list, with a report entry; QET's folio list arrives as EW's own table of contents; an element's `mirror` is not read (QET 0.100 writes none; newer versions do); fonts are substituted, so texts sit slightly lower.

### Wiring and routing

- FT-1597 `golden`: On Golden A's folio 106 the wires to the modules' lower pins (4, 5, FE) wrap round the module bodies, because doc 20 §15 puts the interruption points of the top and bottom pins' feeds in one cell (B1, E1); give the lower-pin feeds cells of their own (below the modules) and route them without the wrap.

### The window: feedback and dead ends

- FT-1671 `ew-viewmodel`: Choose part shows a part's tier as "(system)", a debug name; give the tiers words.
- FT-1672 `ew-viewmodel`: The PLC panel lists no cards once a project has two or more CPUs.
- FT-1669 `ew-scene`: A connection point marked not used shows no mark on the page (doc 06 defines none); decide the mark and draw it.
- FT-1550 `ew-commands, ew-api`: `page.create` and the built-in template give a new page the QET 10 px grid, against FR-PAG-031, DEC-045 and doc 06 (new pages use the native grid); build FR-PAG-031's setting "grid for new pages", default native, and move the defaults with it (FT-458's ruling, ID-650). It is a command default: 95 test files rely on it and a changed default changes how an existing command log replays, so it needs a decision under ADR-0011 first.

### Texts, and Dutch

- FT-1674 `ew-import-qet, ew-checks`: The 21 EW-IMP rules of a migration report read in English in a Dutch window: their message is the importer's own text, stored in the project. Wording them per language needs their arguments stored instead, which changes the format.
- FT-1675 `ew-checks, ew-viewmodel`: Three texts name a thing by its ID or path: EW-REL-004 prints a revision's ID, EW-CORE-008 a form copy's ID, and the refusal of `image.file` the file's path; name them in words.



### Tests

- FT-847 `ew-testkit (acceptance runner), Golden A`: `golden/J-E-2416/expected/acceptance_headless.md` pins totals of growing sets ("3469 keys of 84 memoised families", "181 files a save writes", "644 commands"), against CONTRIBUTING.md's Tests rule: every new family, file or builder command re-blesses it …
- FT-1626 `ew-ui`: The snapshot tests have no Linux images: the 15 Linux-only overrides were stale since rc.3 (never re-blessed on Linux) and are removed, so on Linux the affected dialogs compare with the Windows images and fail until they are blessed there. Bless on Linux when a Linux run is wanted again (1.0 at the latest).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
