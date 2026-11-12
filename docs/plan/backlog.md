# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M2.5-Fix: a properly working program

Fixes only. The gate is the owner's own hands-on test: the wizard, a new project, placing, wiring, routing, the
symbol editor and the QET import work without anything that stops the work. Its release is 0.2.0.

### The owner's hands-on test of 0.1.0-rc.3

- FT-1645 `ew-import-qet, ew-scene`: What an imported QET folio still shows differently, after the full-fidelity work of M2.5-Fix: a master's contact list has no contact symbols and its rows stand closer than QET's 10 px (the scene has no list drawing and the text style no line height); `displayhas="cross"` and `snapto="bottom"` cross-references print as a list, with a report entry; QET's folio list arrives as EW's own table of contents; an element's `mirror` is not read (QET 0.100 writes none; newer versions do); fonts are substituted, so texts sit slightly lower.
- FT-1656 `ew-ui`: **High.** The interface feels clunky and shows too little: a polish pass over the main window, the browsers, the panels and the dialogs for density and finish, judged at 1920 × 1080 (doc 16a).

### Wiring and routing

- FT-866 `ew-commands`: **Medium.** When the router falls back on a pin 1 nm off, the wire ends in a 1 nm segment, so P places the interruption point sideways. Seen with a row of adjacent interruption points one grid step apart; applies to symbol connection points too (FIXPT40, cycle 40).
- FT-517 `The auto-route re-solve with FT-18 (WP-S2.04's router)`: Provisional routes that run along other wires end to end or through symbol bodies: on Golden A's folio 101 four wires (`-101F1:1 -> F55:1`, `S56:14 -> E56:X1`, `junction 2 -> S56:13`, `junction 2 -> T55:5`) lie on other wires for their whole length and are reached only by FT-500's repeated-click walk, and E55:L's wire appears …
- FT-768 `ew-viewmodel`: **Medium.** Accidental series insertion: ID-888's through route takes any free representation in line, so a straight wire across an unwired fuse, contact or coil whose CPs face in and out inserts it in series, and going round it needs a bend. Watch for it in the next play-test, and decide whether a symbol other than a terminal should need …
- FT-773 `ew-commands (route)`: The router detours far round a black box when the gap under it is less than one grid step, instead of taking the near side (seen at cycle 39's wave-1 window check).
- FT-791 `ew-commands (normalise)`: **Medium.** A wire drawn by autoconnect has no stored route, so normaliser #7 (`connection_split`) does not split it when a terminal is dropped on it: confirm with a test, then let the normaliser read the connection's drawn line where no route is stored.
- FT-898 `ew-scene, ew-commands`: The router detours around the PSU's box for a short wire (t0029) (play-test rv-vis-4, cycle 40; repeated in rv-vis-5).
- FT-967 `ew-viewmodel, ew-commands`: **Medium.** FT-901's remaining half: making one page arrow the source is refused until the other page's arrow is changed; swap the source in one step (P42, cycle 42).
- FT-1433 `ew-viewmodel`: A device dropped on a wire connects into it without asking ("behind my back"); show the proposals and ask, or say so in the toast, after R1's record for ADR-0007 (R0-F4, Nielsen 2; R0 P-7).
- FT-1434 `ew-derive`: The router's provisional routes jog, take a rectangular detour on a straight vertical connection and run over existing wires (R0-F5, Nielsen 2; R0 P-8).
- FT-1545 `ew-viewmodel`: A target placed by the page's quarters no longer pairs with a source on its own since ID-1462 (the user types the name or uses Continue on page…); watch it in R1 and decide whether the quarters propose the source's name as a typed one.
- FT-1579 `ew-api`: Routing at an import's front door for `ew import csv`, whose `route` column is optional (a QET import needs none any more: every wire arrives routed).
- FT-1588 `ew-derive`: Wire markers are no obstacles for the router or measure (d), since they move with their wire; place them clear of other wires and texts after routing (ID-1502 (3)).
- FT-1589 `ew-derive`: A layout measure for a route over the frame bands; the router already blocks them (WP-S2.19 §5.1 rule 5).
- FT-1590 `ew-derive`: QET-grid connection points can sit 1 nm off a grid line, so the exact measures miss an overlap 1 nm off.
- FT-1594 `ew-viewmodel`: The interruption point tool's ghost connection point should move with the form when D changes the direction (the insertion point stays).
- FT-1597 `golden`: On Golden A's folio 106 the wires to the modules' lower pins (4, 5, FE) wrap round the module bodies, because doc 20 §15 puts the interruption points of the top and bottom pins' feeds in one cell (B1, E1); give the lower-pin feeds cells of their own (below the modules) and route them without the wrap.
- FT-1598 `ew-derive`: A straight route between two connection points whose coordinates are rounded through different paths leans by 1 nm, and a macro inserted at 90° keeps a 1 nm jog; square the route in the router or the insert, then take the 1 nm tolerance out of the testkit's diagonal check and the macro test.
- FT-1602 `ew-api`: `ew layout` still measures an unrouted connection's dashed line for crossings and bends, while D3 counts such a connection only as unrouted; leave it out of the measures as the check rules do.
- FT-1604 `ew-viewmodel`: A wire drawn to a connection point that already holds a wire, where the neighbouring points' stubs leave no clean second route (§5.1 rules 1 and 2), stays unrouted; propose ending it on the first wire as a T junction instead, as drawing practice does.
- FT-1607 `ew-ui`: **Medium.** Route page's and Route selection's preview card lists the plan but draws no planned route on the canvas, the dashed lines staying until Apply; draw the planned routes as the preview's ghost.
- FT-1608 `ew-viewmodel`: A route drawn before its interruption point has a partner can run through the cross-reference text that appears with the partner, since a text change re-solves no route; re-solve the routes a new text crosses, as a move does (D14).

### The window: feedback and dead ends

- FT-1673 `ew-render2d, ew-text`: The pin labels of a black box (the power supply) and the squares of open connection points are tiny on the page (the persona round, RP1 to RP4). Both come from tokens that every page picture and the sample depend on (the built-in text-style table, ADR-0032; the square of doc 16a): decide the sizes in the finish pass (FT-1656) and re-bless once.
- FT-1671 `ew-viewmodel`: Choose part shows a part's tier as "(system)", a debug name; give the tiers words.
- FT-1672 `ew-viewmodel`: The PLC panel lists no cards once a project has two or more CPUs.
- FT-1669 `ew-scene`: A connection point marked not used shows no mark on the page (doc 06 defines none); decide the mark and draw it.
- FT-1550 `ew-commands, ew-api`: `page.create` and the built-in template give a new page the QET 10 px grid, against FR-PAG-031, DEC-045 and doc 06 (new pages use the native grid); build FR-PAG-031's setting "grid for new pages", default native, and move the defaults with it (FT-458's ruling, ID-650). It is a command default: 95 test files rely on it and a changed default changes how an existing command log replays, so it needs a decision under ADR-0011 first.

### Texts, and Dutch

- FT-1674 `ew-import-qet, ew-checks`: The 21 EW-IMP rules of a migration report read in English in a Dutch window: their message is the importer's own text, stored in the project. Wording them per language needs their arguments stored instead, which changes the format.
- FT-1675 `ew-checks, ew-viewmodel`: Three texts name a thing by its ID or path: EW-REL-004 prints a revision's ID, EW-CORE-008 a form copy's ID, and the refusal of `image.file` the file's path; name them in words.

### Symbols, parts and content

- FT-1667 `content`: The starter contactor EW-GEN-CON-1 is filed as a relay (its category is its coil's), so the browser lists it under Relays and nothing under Contactors. Decide its category, and with it its letter (K or Q), in the starter parts.
- FT-657 `WP-S7.02 content`: The generic starter parts have no MCB 3P and no solenoid valve 3/2: WP-S2.18 D5 de-scoped them by its §5 rule because the starter set has no FnDef or symbol for either. Add the starter FnDefs and symbols, then the two generic parts in `ew-starter-parts` (regenerated by `starter_parts.rs`, a path in `starter_symbols.rs`).
- FT-1530 `content, ew-library`: The starter symbols have English names only, so Device from part's symbol step shows them in English in a Dutch window; give them Dutch names in a new library version.
- FT-1619 `ew-library`: The QET edition of the three-phase motor draws "M" over "3~", and its pin texts stand 0.27 mm from the next lead; space them.
- FT-682 `ew-library, Golden A`: **Medium.** Source-output CPs for EW-GEN-PSU-1 and Golden A's -102G1, so the flow reaches the 24 V circuits (affects EW-POT-013, 018, 021 and the load lists); a golden change.
- FT-03 `WP-S3.02`: Add the Noto Sans trademark line (name ID 7) to `NOTICE`.

### Checks, reports and the format

- FT-1670 `ew`: `ew render` sums up a page's findings as "(unresolved wire ends, unknown styles, overflowing table texts)" whatever they are; on an imported folio they were fifteen title-block fields the file leaves empty. Say what they are.
- FT-1662 `ew-viewmodel, ew-api`: **Medium.** Nothing refreshes the project lock while a project is open: `Session::heartbeat` has no caller in the window or the CLI, so after an hour another session may take the lock as stale (doc 05 §7.2). Call it on the heartbeat interval.
- FT-1663 `ew-api`: The session's change feed (`Session::drain_changes`) is filled on every commit and never drained by the window, so it grows for as long as a project is open. Drain it, or stop filling it where nothing reads it.
- FT-1664 `ew-api, ew-checks`: `ew verify-derived` does not sweep the memoised check findings (`check_findings` of `ew-checks`), and its guard test does not show the gap because the golden workload runs no check through the memo store. Sweep the family, and run the checks in the guard's workload.
- FT-903 `ew-reports, ew-derive`: **Medium.** The PLC overview shows all 16 channels "used" with empty Designation and Function columns while the check reports unconnected channels (play-test rv-vis-4, cycle 40).
- FT-681 `ew-checks`: **Medium.** EW-POT-021 reports a false positive on two loads in series; refine the sourceless test or the load pairing.
- FT-1559 `ew-library`: `Registry::resolve` (`crates/ew-library/src/registry.rs:904`) never searches the `plc_profile` kind, so a company library's PLC profile cannot be resolved by reference; search it as the other kinds are searched, with a test.
- FT-1153 `ew-format`: **Medium.** `Migration::record` runs every format record step before every kind step, so across two or more hops a record step for a later format sees records at an earlier `$v`; let kind steps name their format and interleave them.

### Tests

- FT-847 `ew-testkit (acceptance runner), Golden A`: `golden/J-E-2416/expected/acceptance_headless.md` pins totals of growing sets ("3469 keys of 84 memoised families", "181 files a save writes", "644 commands"), against CONTRIBUTING.md's Tests rule: every new family, file or builder command re-blesses it …
- FT-1591 `ew-testkit`: The fuzz `device.place` arm counts Connections +0 although a random drop onto a wire splits it (normaliser 7).
- FT-1596 `ew-ui`: FT-1091's test (a connection point on a link) is `#[ignore]`d because no folio of Golden A has a connection point on a link after its routing; give the test a staged page of its own and run it again; then take its name out of the deep run's command in CONTRIBUTING.md.
- FT-1599 `tests`: Tests such as `a_view_change…` pin page points to Golden A's layout and move whenever its layout does; give them a fixture of their own that the router's layout passes do not touch.
- FT-1626 `ew-ui`: The snapshot tests have no Linux images: the 15 Linux-only overrides were stale since rc.3 (never re-blessed on Linux) and are removed, so on Linux the affected dialogs compare with the Windows images and fail until they are blessed there. Bless on Linux when a Linux run is wanted again (1.0 at the latest).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
