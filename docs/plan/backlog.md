# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M2.5-Fix: a properly working program

Fixes only. The gate is the owner's own hands-on test: the wizard, a new project, placing, wiring, routing, the
symbol editor and the QET import work without anything that stops the work. Its release is 0.2.0.

### The owner's hands-on test of 0.1.0-rc.3

- FT-1629 `ew-viewmodel, ew-ui`: **Medium.** The company-standard wizard keeps referring to the sample project, which production software should not; each step explains its setting by itself and shows a worked example of what it produces (a designation, a wire number, a page, a report).
- FT-1630 `ew-ui`: **Medium.** The company-standard wizard's buttons and layout look clunky; lay it out again to the design language (doc 16a).
- FT-1631 `ew-viewmodel, ew-ui`: **Medium.** The wizard's colour-profile step: the chosen profile does not show in the preview, and nothing says what a profile does, which one is the default, or that it can be changed later and where.
- FT-1632 `ew-viewmodel, ew-ui`: **Medium.** The wizard's identifiers and numbering step does not say which `{…}` tokens exist; list them, each with what it stands for, and show the pattern's result live.
- FT-1633 `ew-viewmodel, content`: **Medium.** The wizard's reports and checks step describes its options in jargon a person cannot follow; say in plain words what each report makes and what each check finds, with an example.
- FT-1634 `ew-viewmodel, ew-ui`: **Medium.** A project has a button to add a page but no way to add a section; add New section beside it and on the project tree's right-click menu.
- FT-1635 `ew-ui`: **Medium.** The device browser stays in front: a click on the backdrop does not close it and it has no close button, so only Esc closes it; every overlay gets a close button and closes on a backdrop click.
- FT-1636 `ew-viewmodel, ew-ui`: **Medium.** The device browser is clunky: no categories, no tree, and too little on screen; browse by a category tree with denser rows.
- FT-1637 `ew-scene, ew-viewmodel`: While a device is being placed its texts jitter around it as it moves, and the ghost reads "Preview of Place device"; keep the texts still relative to the symbol and word the label properly.
- FT-1638 `ew-viewmodel`: **Medium.** A right click cancels the active tool (placing a device, drawing a wire, any armed tool) as Esc does; the context menu opens only when no tool is active.
- FT-1639 `ew-ui`: **Medium.** The bar on the page shows buttons beside the active tool that cannot be reached, since the bar moves away as the pointer comes near; settle its purpose and design (an information strip with no buttons, or a tool bar that stays put).
- FT-1640 `ew-ui`: **Medium.** The symbol editor is entered from a small button named "Symbols", and its Close editor button is squeezed between other options; give the editor an entry of its own named for it, and Close editor a place the eye goes to first.
- FT-1641 `ew-viewmodel, ew-ui`: **Medium.** The symbol editor has no precise positioning: a moved item shows no ghost and the arrow keys do not nudge the selection; add both.
- FT-1642 `ew-viewmodel`: **High.** In the symbol editor the select tool can neither move nor delete a connection point.
- FT-1643 `ew-viewmodel, ew-library`: **High.** A symbol drawn in the editor (a rectangle and a few connection points) never reaches a page: Save to library seems to do nothing, Test on new page does nothing, and the symbol is not in the part browser; make the flow work end to end and confirm each step, or say in words what is missing.
- FT-1644 `ew-viewmodel, ew-ui`: **High.** The QET import window lists information a person cannot act on: about 654 errors and 1,200 warnings with no conclusion and no stated goal, and an "Apply" that does not say what it applies; lead with what the import will do and a verdict, group the findings by kind with what each means and whether it needs action, and name the button for its action.
- FT-1645 `ew-import-qet`: **High.** An imported QET project's folios look nothing like the project in QET: wires drawn diagonal and messy, cross-references without names, texts overlapping, free graphics (a drawn box) lost, and some folios unrecognisable; import the geometry faithfully, compared folio by folio with QET's own rendering of a private test project. Where the gap is WP-S3.10 or WP-S3.11 scope not yet built, M2.5-Fix's planning rules whether that scope moves in or the import is withheld until M3.
- FT-1646 `ew-ui`: **Medium.** While an interruption point is placed and its name is typed, the small name window turns grey as if disabled.
- FT-1647 `ew-viewmodel, ew-ui`: **High.** Edit route is clunky; edit a route with the mouse: a handle on each segment and bend that drags, keeping the route orthogonal.
- FT-1648 `ew-viewmodel, ew-ui`: **Medium.** Properties rows do not say what they do; give every row a tooltip that explains it, in English and Dutch.
- FT-1649 `ew-viewmodel`: **Medium.** A connection point cannot be marked as not used: Properties shows it but offers nothing; add the action there and on the point's right-click menu.
- FT-1650 `ew-ui`: **Medium.** Properties sections do not fold: a click on the Placement section's header does nothing.
- FT-1651 `ew-viewmodel, ew-derive`: **Medium.** A wire from an off-page reference to a device gets BK 0.75 mm², shown as derived but not from what, and neither the reference nor the wire shows the voltage or the kind of signal or where to set them; only an override is offered. Show the source of a derived value and let the potential's voltage and signal kind be set from the wire and the reference.
- FT-1652 `ew-ui`: **Medium.** Important parts such as the terminal strip have no button of their own; give the main kinds one each.
- FT-1653 `ew-viewmodel, content`: **High.** Many starter symbols do nothing when clicked in the symbol list; each one arms its placement or says in words why it cannot.
- FT-1654 `ew-viewmodel, ew-derive`: **High.** Wire markers do not work in the window: clicking Wire marker in the symbol list does nothing, so there is no visible way to add one, and a wire's "Wire marker" check box has no visible effect.
- FT-1655 `ew-derive`: **High.** The router draws a diagonal route when it finds no clear direction; a route is never diagonal: fall back to an orthogonal route with a finding.
- FT-1656 `ew-ui`: **High.** The interface feels clunky and shows too little: a polish pass over the main window, the browsers, the panels and the dialogs for density and finish, judged at 1920 × 1080 (doc 16a).

### The persona round on rc.3

- FT-1657 `ew-viewmodel`: **Medium.** FT-1621 re-dated (ID-1521): the interruption-point tool's direction (RP1, RP4): the status line says source and the point goes in as target; after one placement the tool renames itself "L1-1" and switches to target; D types into the Tool options name field when it has the focus.
- FT-1658 `ew-viewmodel`: **Medium.** FT-1622 re-dated (ID-1521): adding a contact to an existing device (RP2, RP4): picking a contact of -102K1 from "In this project" makes a new device; Join on a placed contact is refused; deleting the extra device leaves an empty "-?K2"; Join's list shows the device twice per slot; the duplicate-designation finding's Fix does nothing visible.
- FT-1659 `ew-viewmodel`: FT-1623 re-dated (ID-1521): a drag from a symbol's centre does nothing (only its edge grabs); a paste lands about 20 px from the click; the "4 Warnings" badge opens Messages filtered on "2 devices"; Esc leaves the coil tool armed (RP2, RP3).
- FT-1660 `ew-viewmodel`: FT-1624 re-dated (ID-1521): the full-junction HUD reads as a refusal though the wire is made; an outgoing point's connection point one step behind its tip goes unseen; the auto-connect notice on placement is missed; the text interface gives Route page's result as bend counts only, not where a wire runs; the power supply's pin labels and the points' connection squares are tiny (RP1 to RP4).

### Wiring and routing

- FT-1665 `ew-derive`: **Medium.** The router can return a route that runs on another wire: the property test `routing::search::tests::a_found_route_keeps_off_other_wires` fails for the case `fx = 0, fy = 0, tx = 1, ty = 2, fd = None, td = Some(1), ws = [(7, 4, -6, true), (0, 3, 2, true)]` and for `fx = 2, fy = 5, tx = 0, ty = 0, fd = Some(3), td = None, ws = [(3, 4, -2, true), (2, 0, 3, false)]` (found by random draws of the suite on 2026-11-10; it passes on most draws, so the suite fails now and then until this is fixed). Add the cases as fixed tests and fix the search.
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
- FT-1579 `ew-api`: Routing at an import's front door (a project imported without routes routed once on import, previewed), WP-S2.19 OQ5.
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

- FT-1666 `ew-viewmodel, ew-api`: **Medium.** The preview of a table fill is very slow: filling one column over 502 marked device rows takes about 59 seconds before the preview card opens (debug build, measured in the test `tb5_a_fill_over_more_than_500_rows_opens_the_card`, about 0.12 s per row; not measured in a release build). Nothing is committed in that time; the cost is in building the bulk preview, probably one evaluation of the project per row. The test takes 70 to 100 s for this reason and times out when the machine is busy.
- FT-926 `ew-viewmodel, ew-ui`: **Medium.** A PLC card gets its addresses only once a CPU is placed, and nothing says so: the PLC panel reads "No PLC cards yet" with the card drawn and selected, and the card's properties offer no address field; say in the panel and on the card that a card needs a CPU, and how to add one (FT-823's area) (play-test rv-vis-5, cycle 41).
- FT-927 `ew-viewmodel, ew-ui`: The command search finds nothing for "PLC" (only fuzzy hits such as "Properties of selection"), and the PLC panel sits behind an unlabelled chip icon on the left rail (play-test rv-vis-5, cycle 41).
- FT-928 `ew-viewmodel, ew-ui`: **Medium.** A placement that places nothing gives no message: Device, typing "CPU", Enter and two corner clicks left nothing on the sheet and no reason (with FT-896) (play-test rv-vis-5, cycle 41).
- FT-929 `ew-ui`: The Tool options panel lies over the sheet and hides the terminals where the user wants to click (play-test rv-vis-5, cycle 41).
- FT-930 `ew-ui`: The hint bar moves from the bottom to the top of the sheet and covers the drawing there (play-test rv-vis-5, cycle 41).
- FT-981 `ew-ui`: The tool options card covers the sheet's column header and top-left corner at both sizes (doc 16a §5.2) (W42b, cycle 42).
- FT-614 `WP-S2.03`: Symbols are tiny at the default fit zoom (69 % on a new page in the 1536 × 994 frame): review the opening zoom of a page with few symbols.
- FT-824 `ew-ui`: Opening a page from the Pages list: a click or a double-click on a row does nothing; only Enter or Page ▸ Go to opens it (rv-vis-3b t0003, t0060).
- FT-826 `ew-ui, ew-viewmodel`: Rv-vis-3's first-attempt frictions: picking a PLC card switches to the Black box tool, which asks for corners (t0002); the "19 Pending" badge is never explained (t0004); the reasons on the grey rows of the Insert symbol list are cut (t0009). Known, not new: no parts-list command under Output (FT-717); toasts stack in the …
- FT-430 `WP-S2.05`: The Properties pane's Form row (and every reference-valued row the window shows) prints the referenced record's label, the symbol form's name or key, instead of its UUID; the window keeps real UUIDs in its widgets (ID-630), but a UUID is no value for a user; a kittest on a representation's sheet.
- FT-819 `ew-api (part_offers.rs), ew-viewmodel`: The Choose part dialog's detail line prints the template's FnDef keys ("functions ew.fn.mcb-1pn", or an item UUID when the FnDef does not resolve): `Session::part_offer_functions` returns `meta.key` by design, so this is not FT-800's cause (a word fallback for a FnDef not in the project), though one word source could serve …
- FT-1585 `ew-ui`: A device selected only by its tag (the palette's tag hits, `select.set tag:`) draws no outline on the canvas although the status bar counts it.
- FT-1586 `ew-viewmodel`: Ctrl+A on the canvas (select every item of the page) is not built; doc 16 §4.6 says so.
- FT-1587 `ew-viewmodel`: The page's Properties badge read "1 Pending" while the Messages panel said "pending 8" and the status bar "8 pending on this page" (FIXR1's frame 06, cause unverified: perhaps only findings whose subject is the page).
- FT-1546 `ew-viewmodel`: The terminal designation counter keeps a session memory (`crates/ew-viewmodel/src/tools/place.rs:786`) and can propose a designation already in use after it fills a gap (the command refuses it in words); read the strip again after each commit, as ID-1462 does for interruption points.
- FT-1550 `ew-commands, ew-api`: `page.create` and the built-in template give a new page the QET 10 px grid, against FR-PAG-031, DEC-045 and doc 06 (new pages use the native grid); build FR-PAG-031's setting "grid for new pages", default native, and move the defaults with it (FT-458's ruling, ID-650).
- FT-1563 `ew-viewmodel`: **Medium.** The symbol editor's open or import of a malformed symbol file and an export's file-system failure still show the reader's or the system's own message; key them as FIX25c keyed the editor's edits (`symbol-change-invalid`, ID-1483 F2).
- FT-1564 `ew-ui`: The symbol editor's properties panel is a little wider on the Drawing tab than on Library, so the canvas shifts between tabs; one width on every tab (doc 16a).
- FT-1565 `ew-viewmodel`: "Place on a new page with its grid" (the picker's `picker.new_page` and the editor's `symbol.new_page`) makes the page first; when the placement is then refused (a published terminal's `reason-no-strip`), the empty page is left behind; check the placement before the page, or undo the page with the refusal.
- FT-1571 `ew-viewmodel`: The QElectroTech import card does not say that the starter library was not found; the user learns it only from the banner after Apply; the card names it before Apply (WP-S3.09 D8).

### Texts, and Dutch

- FT-1551 `ew-viewmodel, ew-api`: The script reference resolver's reasons reach the PLC target cell in English in a Dutch window; key them.
- FT-1552 `ew-viewmodel`: A `SchemaError`'s window text carries its JSON Pointer; name the field in words.
- FT-1553 `ew-viewmodel`: The command-log errors of `run_logged` and the wizard are English in a Dutch window; key the ones a person can reach.
- FT-1554 `ew-viewmodel, ew-api`: The grid's "Part function" row, the live description and the place catalogue show a generic kind in English; word them as FT-1483 does, and the Libraries kind column too.
- FT-1555 `ew-viewmodel`: `StripEditError` passes its English to the window, and a Dutch window's yes/no editor refuses "ja"; key the one and accept the other.
- FT-1556 `ew-commands`: The attached-records refusal shows the raw member name ("pin fn_cp 0") and "a function without a device" for an interruption point's pins; name them in words.
- FT-1557 `ew-viewmodel`: `image.pick`'s refusal of an over-limit image names it by its `sha256:` hash, as its missing-asset error does; name the image by its file name where the project knows it.
- FT-912 `ew-checks`: English fallback words ("an unknown profile", "a location", "a device without designation", "a missing part") and "40.0"-style decimals print inside the nl-NL rule messages, across all rules (`layout.rs`, `cable.rs`, `pot.rs`) (the review's part-3 finding 3, cycle 41).
- FT-968 `ew-checks`: EW-CORE-052's new hint has no nl-NL text (P42, cycle 42).
- FT-680 `ew-checks`: Nl-NL Fluent messages beyond the twelve rules Golden A reports (WP-S1.16 part 5 de-scoped them by its §13.1 rule; the other rules fall back to en-GB): titles, hints and value patterns for every registered rule, with P5-1's completeness test extended to nl-NL.
- FT-1570 `ew-api`: The update's failed-save refusal carries an English detail into a Dutch window, as the purge refusal does; key the detail.
- FT-1600 `ew-api`: A connection's `{wire}` words with a junction end read "the junction in C11 → …" in English in every language (`connection_words`); give the junction end its words per language, as the rule messages do.
- FT-924 `ew-commands, ew-derive`: A device cloned without online numbering prints "-X?33" (B42, cycle 42).
- FT-743 `ew-commands`: `edit.paste` leaves child counters unassigned (a pasted valve prints "Q?") where `macro.insert` keeps them when the parent comes along.
- FT-933 `ew-checks`: EW-POT-011's message "Junction feed ambiguous" names no wire, page or place; say which wires feed which junction or group of interruption points, and where (play-test rv-vis-5, cycle 41).
- FT-1661 `docs/book`: The guide's Dutch translation leaves 4 of the 183 messages of the introduction and the tutorials untranslated, so `ci/translation-status.py` exits 1 on 0.1.0's commit (RC-06); translate them.

### Symbols, parts and content

- FT-657 `WP-S7.02 content`: The generic starter parts have no MCB 3P and no solenoid valve 3/2: WP-S2.18 D5 de-scoped them by its §5 rule because the starter set has no FnDef or symbol for either. Add the starter FnDefs and symbols, then the two generic parts in `ew-starter-parts` (regenerated by `starter_parts.rs`, a path in `starter_symbols.rs`).
- FT-1530 `content, ew-library`: The starter symbols have English names only, so Device from part's symbol step shows them in English in a Dutch window; give them Dutch names in a new library version.
- FT-1619 `ew-library`: The QET edition of the three-phase motor draws "M" over "3~", and its pin texts stand 0.27 mm from the next lead; space them.
- FT-682 `ew-library, Golden A`: **Medium.** Source-output CPs for EW-GEN-PSU-1 and Golden A's -102G1, so the flow reaches the 24 V circuits (affects EW-POT-013, 018, 021 and the load lists); a golden change.
- FT-03 `WP-S3.02`: Add the Noto Sans trademark line (name ID 7) to `NOTICE`.

### Checks, reports and the format

- FT-1662 `ew-viewmodel, ew-api`: **Medium.** Nothing refreshes the project lock while a project is open: `Session::heartbeat` has no caller in the window or the CLI, so after an hour another session may take the lock as stale (doc 05 §7.2). Call it on the heartbeat interval.
- FT-1663 `ew-api`: The session's change feed (`Session::drain_changes`) is filled on every commit and never drained by the window, so it grows for as long as a project is open. Drain it, or stop filling it where nothing reads it.
- FT-1664 `ew-api, ew-checks`: `ew verify-derived` does not sweep the memoised check findings (`check_findings` of `ew-checks`), and its guard test does not show the gap because the golden workload runs no check through the memo store. Sweep the family, and run the checks in the guard's workload.
- FT-903 `ew-reports, ew-derive`: **Medium.** The PLC overview shows all 16 channels "used" with empty Designation and Function columns while the check reports unconnected channels (play-test rv-vis-4, cycle 40).
- FT-681 `ew-checks`: **Medium.** EW-POT-021 reports a false positive on two loads in series; refine the sourceless test or the load pairing.
- FT-1559 `ew-library`: `Registry::resolve` (`crates/ew-library/src/registry.rs:904`) never searches the `plc_profile` kind, so a company library's PLC profile cannot be resolved by reference; search it as the other kinds are searched, with a test.
- FT-1153 `ew-format`: **Medium.** `Migration::record` runs every format record step before every kind step, so across two or more hops a record step for a later format sees records at an earlier `$v`; let kind steps name their format and interleave them.

### Tests

- FT-847 `ew-testkit (acceptance runner), Golden A`: `golden/J-E-2416/expected/acceptance_headless.md` pins totals of growing sets ("3469 keys of 84 memoised families", "181 files a save writes", "644 commands"), against CONTRIBUTING.md's Tests rule: every new family, file or builder command re-blesses it …
- FT-888 `ew-testdir, ew-viewmodel and other tests`: Tests leak their temporary directories into the plain temp directory (thousands of `ew-shell-test-*`, other `ew-*` and `.tmp*` entries seen on 2026-10-31), which a virus scanner then scans; every test temp directory is removed on drop, or lives under the test …
- FT-1584 `ew-ui`: The visual harness's drag still lets go of its modifiers in the release frame, as its click did before ID-1501; a modified drag is seen unmodified.
- FT-1591 `ew-testkit`: The fuzz `device.place` arm counts Connections +0 although a random drop onto a wire splits it (normaliser 7).
- FT-1596 `ew-ui`: FT-1091's test (a connection point on a link) is `#[ignore]`d because no folio of Golden A has a connection point on a link after its routing; give the test a staged page of its own and run it again.
- FT-1599 `tests`: Tests such as `a_view_change…` pin page points to Golden A's layout and move whenever its layout does; give them a fixture of their own that the router's layout passes do not touch.
- FT-1626 `ew-ui`: The snapshot tests have no Linux images: the 15 Linux-only overrides were stale since rc.3 (never re-blessed on Linux) and are removed, so on Linux the affected dialogs compare with the Windows images and fail until they are blessed there. Bless on Linux when a Linux run is wanted again (1.0 at the latest).

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
