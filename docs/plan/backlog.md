# Backlog

Open work, one row each. A finished row is deleted; a new one is added under its milestone. The numbers are the
old follow-up numbers, kept so that a row's history can be found in git (`5516edba4`, `docs/plan/follow-ups.md`
and the work breakdown's Appendix B).

## M2.5-Fix: a properly working program

Fixes only. The gate is the owner's own hands-on test: the wizard, a new project, placing, wiring, routing, the
symbol editor and the QET import work without anything that stops the work. Its release is 0.2.0.

### The owner's hands-on test of 0.1.0-rc.3

- FT-1645 `ew-import-qet, ew-scene`: What an imported QET folio still shows differently, after the full-fidelity work of M2.5-Fix: a master's contact list has no contact symbols and its rows stand closer than QET's 10 px (the scene has no list drawing and the text style no line height); `displayhas="cross"` and `snapto="bottom"` cross-references print as a list, with a report entry; QET's folio list arrives as EW's own table of contents; an element's `mirror` is not read (QET 0.100 writes none; newer versions do); fonts are substituted, so texts sit slightly lower.

### The persona round before 0.2.0

- FT-1678 `ew-viewmodel, ew-ui`: **High.** Interruption points are the hardest part of wiring for a newcomer (two runs, 25 turns of one). A name typed right after the tool is armed is lost, and Enter then places a point at the grid cursor; D goes through source, target and undirected, and the bar names the next state, not the one the arrow has; an undirected arrow takes its wire at the tail and nothing shows which end that is; after the first point the default turns to target without a word. Make the name the first thing the tool takes, show the direction the arrow has, and mark the end that takes the wire.
- FT-1679 `ew-scene`: **High.** Interruption points placed two grid steps apart (8 mm) print their names and cross-references over their neighbours and through the arrows. A click on such an arrow then follows the reference lying on it to the other page instead of selecting the arrow, a drag of it moves nothing, and the wires under the texts are "wire through a text" findings the person did not cause. Give the texts room (a reference beside the arrow, or a refusal with a reason where a point would stand in a neighbour's texts).
- FT-1604 `ew-viewmodel`: **Medium.** Seen in two of the three wiring runs before 0.2.0: a second wire drawn to an occupied pin is routed over the first wire's last stretch and reads as a join without a dot, so the drawing does not say the two are connected. A wire drawn to a connection point that already holds a wire, where the neighbouring points' stubs leave no clean second route (§5.1 rules 1 and 2), stays unrouted; propose ending it on the first wire as a T junction instead, as drawing practice does.
- FT-1680 `ew-ui`: The symbol editor's connection points: a selected point shows no properties (its name is under Electrical data, Function definition); the Designation box of the Connection point tool names the next point, which reads as if it renamed the one just placed; "Facing up" does not say which way the wire leaves. The role list scrolling its row to the panel's top (an earlier decision, so that every role shows) made the next click land on another control in a picture-led run.
- FT-1681 `ew-viewmodel`: A terminal renamed through the Tag row of Properties gets a frozen tag ("-X1011:L (frozen)") that the page does not print: the sheet still shows 1, and no row for the terminal's own designation is in sight.
- FT-1682 `ew-viewmodel, ew-ui`: Smaller findings of the round: a part drawn as a black box (the power supply) wants two corners, and one click and Escape place nothing without a word; placing a second contact of a contactor from the browser makes a new contactor; Insert symbol lists a symbol twice, "embedded" and "library"; a wire is drawn by clicks and a drag from a pin only starts it; a new lamp is tagged E, not H; the mouse wheel zooms where an EPLAN user expects it to scroll.

## Later

New development and polish for M3 and after, not scheduled: [backlog-later.md](backlog-later.md).
