# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M2.5-Fix: a properly working program

Fixes only. The gate is the owner's own hands-on test: the wizard, a new project, placing, wiring, routing, the
symbol editor and the QET import work without anything that stops the work. Its release is 0.2.0.

### The owner's hands-on test of 0.1.0-rc.3

- FT-1645 `ew-import-qet, ew-scene`: What an imported QET folio still shows differently, after the full-fidelity work of M2.5-Fix: a master's contact list has no contact symbols and its rows stand closer than QET's 10 px (the scene has no list drawing and the text style no line height); `displayhas="cross"` and `snapto="bottom"` cross-references print as a list, with a report entry; QET's folio list arrives as EW's own table of contents; an element's `mirror` is not read (QET 0.100 writes none; newer versions do); fonts are substituted, so texts sit slightly lower.

### The owner's hands-on test of 0.2.0-rc.1

After an hour on an imported project of their own. All of it goes into the second candidate (the owner's word).

- FT-1684 `ew-viewmodel, ew-ui`: **Major.** The Wire marker tool does not mark: with two sources and two wires no marker appears, and Enter with cross-section 1.5 and colour BK gives "invalid parameters at /colours"; the error stays after the colour is taken out again.
- FT-1685 `ew-viewmodel, ew-ui, ew-commands`: **Major.** Making a cable from wires cannot be found: it is in the main menu only, not on the Draw tab, and a right click on several selected wires offers nothing. Two wires put into a 1.5 mm² cable keep markers that say 0.75. The tutorial is out of date. The owner's word: the whole cable flow is clunky and needs a redesign; the command row should hold every such command, nobody should have to go through the menus.
- FT-1686 `ew-viewmodel, ew-ui, ew-import-qet`: **Major.** In an imported QElectroTech project the title block's fields (description, drawing, revision and the others at the foot of the page) cannot be edited; a right click on the title block with Edit is wanted.
- FT-1687 `ew-viewmodel, ew-ui, docs/book`: **Major.** No way is in sight to make a panel layout page, and the help and the tutorials do not mention one, so the panel layout could not be tried.
- FT-1688 `ew-commands, ew-viewmodel`: **Minor.** A wire whose potential is given the type L1 stays 0.75 mm² BK, though the standard in use gives L1 its colour and 1.5 mm²: a wire's cross-section and colour do not follow its potential's type.
- FT-1689 `ew-ui`: **Minor.** Open on the toast of a finished PDF export does nothing.
- FT-1690 `ew-ui, ew-viewmodel`: **Missing.** The symbol editor has no grid choices: other snap steps, snapping on and off, an easy way to change the grid; connection points should snap to the pages' connection grid.
- FT-1691 `ew-ui, ew-library`: **Missing.** A symbol drawn in the editor cannot be given a category: it always lands under "Assemblies and black boxes".
- FT-1692 `ew-scene, ew-viewmodel`: **Missing.** No way is in sight to print the wire numbers on the wires.
- FT-1693 `ew-ui, ew-viewmodel`: **General.** The right-click menu should be the quick way to what fits the thing under the pointer: a terminal strip offers a new terminal and the strip editor, a relay coil its contacts, several selected wires a cable, the title block its fields, a symbol its editor. The owner asks for this to be expanded throughout.
- FT-1694 `ew-ui, ew-viewmodel`: The device and symbol browsers have no easy way to edit an element or to copy and edit one (into the symbol editor).
- FT-1695 `ew-ui, ew-viewmodel`: Terminal on the Draw tab only opens the symbol browser; it should have a small interface of its own, and the terminals should leave the symbol browser.
- FT-1696 `ew-ui, ew-viewmodel`: The difference between the symbol browser and the device browser is not clear (they seem to open the same library, the symbol browser with more in it), and the right rail calls its panes Parts and Symbols where the Draw tab says Device and Symbol.
- FT-1697 `content`: Relay coils are drawn as resistors, not as coils.
- FT-1698 `ew-ui, ew-viewmodel`: Define potential asks for the type as typed text; a list as in Properties would show the choices.
- FT-1699 `ew-checks, ew-viewmodel`: An interruption point cannot be marked as a mains source, so the supply that enters by one stays an error until it is waived.
- FT-1700 `ew-derive, ew-viewmodel`: A wire's potential could be reasoned: a wire on A2 of a 24 V relay is 0VDC. The window should offer such a suggestion to accept or decline.
- FT-1701 `ew-checks`: A power supply's L pin on a wire of L1 is reported as an error ("expected L, got L1"): right to the letter, wrong in sense.
- FT-1702 `content`: The power supply symbol has one "+" at the top of its box instead of at the bottom with the others.
- FT-1703 `ew-ui`: The tools of the symbol editor have no tooltips; a short line on what each does is wanted.
- FT-1704 `ew-ui, ew-viewmodel`: Selecting a connection point in a part's Properties does not show it on the drawing, so which "+" of the power supply is meant stays unclear.
- FT-1705 `ew-commands, ew-viewmodel`: A waiver demands a reason of at least 10 characters; the owner wants no minimum, and reason and author optional.
- FT-1706 `ew-scene`: Wire markers look scruffy: the wire runs through the marker's text.
- FT-1707 `ew-ui, ew-viewmodel`: The strip editor is hidden in the menus; it should be a right click on a terminal strip away.
- FT-1708 `ew-viewmodel, ew-ui`: Importing a QElectroTech project belongs under File, Open; and opening an .ewz is a separate entry there for no reason a user sees.
- FT-1709 `docs/book`: The tutorials hold one that runs the command line on the sample project, which no user of the window needs there.
- FT-1710 `bin/elektrawatt, packaging`: On Windows the program has no icon, a console window opens before the application does, and the window's icon looks like a swastika from a distance and is not the mark of the ElektraWatt menu button.
- FT-1711 `ew-ui, ew-viewmodel, docs`: The line and the bar that say the checks support and do not replace engineering verification are to go: the owner wants no such disclaimers in the window.

### The persona round before 0.2.0

- FT-1604 `ew-viewmodel`: **Medium.** Seen in two of the three wiring runs before 0.2.0: a second wire drawn to an occupied pin is routed over the first wire's last stretch and reads as a join without a dot, so the drawing does not say the two are connected. A wire drawn to a connection point that already holds a wire, where the neighbouring points' stubs leave no clean second route (§5.1 rules 1 and 2), stays unrouted; propose ending it on the first wire as a T junction instead, as drawing practice does.
- FT-1680 `ew-ui`: The symbol editor's connection points: a selected point shows no properties (its name is under Electrical data, Function definition); the Designation box of the Connection point tool names the next point, which reads as if it renamed the one just placed; "Facing up" does not say which way the wire leaves. The role list scrolling its row to the panel's top (an earlier decision, so that every role shows) made the next click land on another control in a picture-led run.
- FT-1681 `ew-viewmodel`: A terminal renamed through the Tag row of Properties gets a frozen tag ("-X1011:L (frozen)") that the page does not print: the sheet still shows 1, and no row for the terminal's own designation is in sight.
- FT-1682 `ew-viewmodel, ew-ui`: Smaller findings of the round: a part drawn as a black box (the power supply) wants two corners, and one click and Escape place nothing without a word; placing a second contact of a contactor from the browser makes a new contactor; Insert symbol lists a symbol twice, "embedded" and "library"; a wire is drawn by clicks and a drag from a pin only starts it; a new lamp is tagged E, not H; the mouse wheel zooms where an EPLAN user expects it to scroll.

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
