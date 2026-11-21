# ADR-0033: Schematic routing: stored routes written by one page router, unrouted connections and one route convention

**Status:** Accepted; built in cycle 53; decisions 4, 5 and 9 and amendments 1 to 5 superseded by
[ADR-0034](0034-schematic-drafting.md) · **Date:** 2026-11-07 ·
**Deciders:** lead designer (design lane, the project owner may reverse it)

## Context

- **The defect.** On Golden A's folio 101 in rc.1 every connection had no stored route and was drawn by the provisional
  path (`ew_derive::provisional_path`), an L chosen from the two ends alone, one connection at a time. Wires ran
  through connection points they do not join, along other wires and through symbols. The owner asked for "a proper,
  intelligent, autorouter" that "should intelligently set up wires based on standard electrical drawing theory"
 . FT-517, FT-1434 (R0-F5) and ID-1421 record the same defect; ID-1421 found that no rule for one wire at a
  time avoids the overlaps.
- **Two routers.** The Wire tool already routes with obstacles (`ew_commands::route`, FT-706: bodies, texts, other
  wires, other CPs, frame bands), but only for the wire being drawn; every other route-less connection is drawn by the
  provisional path. The router and the drawing disagree.
- **Where text boxes exist.** A text box needs the scene's text measurement (`ew-text`), which only `ew-scene` and the
  crates above it have. `ew-scene` cannot depend on `ew-commands`; normaliser 7 (`ew-commands`) and EW-TERM-006
  (`ew-checks`) read the drawn path below `ew-api`. So an obstacle-aware path cannot be computed by every reader.
- **Stable drawings.** ADR-0003's reality 3: a drawing must look identical years later. A route computed when the page
  is drawn would change whenever the router changes or a neighbour moves.
- **Two meanings of a stored route.** The writers (`require_route`, the tables importer, the Wire tool, the route
  editor) store the full vertex list with both ends; the readers (`drawn_path`, `ew-scene`'s `connection_polyline`)
  add the ends again; the QET importer stores the interior only. Folio 203 draws duplicated ends.
- **Interruption points.** The owner: "the arrows all pointing to the right. But incoming signals connected to the tip
  of the arrow, and outgoing to the line." EW's IP symbol has its one connection point on the tail, so no orientation
  puts a target's wire on the tip.
- **Constraints.** The format is free until the 0.1.0 release; the commands never route (the tool computes, the command stores, replay never runs the router); ADR-0007: explicit connections, moves never change
  connectivity, autoconnect an editing aid with its open question 2 on moves; importers emit format-level records
  (ADR-0021); determinism (NFR-REL-07).

## Decision

1. **A connection's stored route is its drawing.** No reader routes. The router runs only when something writes a
   route: the Wire tool, Route page and Route selection, a move's re-solve, and the tool that writes Golden A's routes.
   A re-route is an explicit, previewed command (ADR-0003's committed computations).
2. **One convention.** A non-empty `Connection.route` is the full vertex list from end `a`'s point to end `b`'s, both
   included; `require_route` checks both ends, a junction's included. Readers draw the list as stored, merging equal
   neighbours, and join an end whose point differs (a stale route) with one straight segment. The QET importer
   converts its conductors to this form.
3. **Unrouted connections.** An empty route on a page connection whose ends differ means *unrouted*: it is drawn on
   every output as a dashed straight line between its ends, has no wire geometry for normalisers and checks, and is
   reported (EW-CORE-073). The provisional path is retired. A router that finds no route that keeps the hard rules
   leaves the connection unrouted and says so; nothing is ever drawn through.
4. **One router core in `ew-derive`.** The search moves from `ew_commands::route` to `ew_derive::routing`, beside the
   page geometry and the layout measures; it is pure, integer-only and deterministic. `ew-api` builds the page
   geometry (the scene's text boxes included) and offers the one API (`page_geometry`, `route_plan`, `route_one`)
   that the tools, the actions and the CLI use; tools receive the geometry from the host. `ew-commands` keeps no
   router and `ew-scene` never routes.
5. **Hard rules and costs.** A route never passes a connection point it does not join, never runs along another
   connection, never crosses a symbol body, a text or the frame's bands, never passes a junction it does not end at
   and never makes a four-way junction. Within them it minimises integer costs for steps, bends, crossings and steps
   beside a parallel mate, then the tie rule of WP-S2.04 (fewest bends, then the move sequence right < down < left <
   up). A page is routed in a fixed order (supply potentials first, then by span), mates together in the order L1,
   L2, L3, L, N, PEN, PE, FE, L+, M, L−, with at most two bounded rip-up rounds.
6. **Commands store, never route** (kept). `connection.route_many` stores a plan in one transaction;
   `rep.move` takes optional re-solved `routes` and autoconnect `connect` hints; autoconnect hints store their
   straight run, which the proposal rule already guarantees to be clear.
7. **Interruption points follow their direction.** EW's IP symbol has two forms, both drawn with the arrow pointing
   right: the source form with the wire on the tail and the target form with the wire on the tip. A point drawn with
   either form is never turned, and its form is its direction's; commands refuse a contradiction in words and a
   direction change swaps the form. Other IP symbols (imported, company) keep a free orientation.
8. **Connect on move** (ADR-0007 open question 2, answered by R1's B1: two "connect", one "ask"): a move proposes
   autoconnect wires by default and commits them with the move, shown before release; a modifier moves without them,
   and a session toggle changes the default.
9. **Golden A** keeps its routes in its source tables, written once by the router, so a later router change appears
   as an explicit golden diff.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **Routes derived when the page is drawn** (every route-less connection routed by the page router at render time, memoised) | Drawings change when the router or a neighbour changes (ADR-0003 reality 3); the readers below `ew-api` (normaliser 7, EW-TERM-006) cannot compute it without the scene's text boxes, so the drawing and the model's rules would disagree; every page draw pays a page solve |
| **The provisional path held to the hard rules in `ew-derive`** | Text boxes are not available there; moving text layout and measurement into `ew-derive` is a refactor of the scene, out of reach for the third candidate |
| **Routing inside the commands** | Breaks ID-564: a replayed log would route again, with whatever router the replaying build has; the command has no text boxes either |
| **A per-connection "automatic" flag, re-routed on every change (QET's model)** | A format member and the instability of derived routes for every automatic wire; the move's re-solve (decision 6) gives the useful half |
| **Interior-only routes** | Every writer, `require_route` and `follow_end` already use the ends; changing them is the larger change |
| **A negotiated-congestion router** | Its result depends on iteration counts that are hard to explain; tens of wires per page do not need it; proposed for 1.0 if the rip-up rounds fall short |
| **One IP form turned by orientation** | No orientation of a symbol with its CP on the tail puts a target's wire on the tip with the arrow pointing right |

## Consequences

### Positive

- What the canvas shows, the PDF prints, the checks judge and the normalisers split is one stored list.
- A drawing never changes behind the user's back; a re-route is a previewed, undoable step.
- Nothing is drawn through a symbol, a text, a connection point or another wire: what cannot be routed is shown as
  unrouted and reported.
- The router has one home below every caller, and the Wire tool's ghost and Route page give the same routes.

### Negative

- Route-less connections from scripts, older files and table imports show as dashed lines until Route page runs, and
  every expected output that drew one changes once.
- Golden A's wire layout comes from the router, not from the reference drawing.
- A move's re-solve runs the router in the Select tool at release; the drag itself still shows followed routes.
- The IP symbol gains a second form and a normaliser; a project must hold both forms to change a point's direction.

## Amendments

The decision text above is unchanged; these entries refine it. All five are superseded by
[ADR-0034](0034-schematic-drafting.md): what they decided is now the drafting design's
([06a](../design/06a-drafting.md)), and amendment 4's settled readings give way to one number at the source (06a §2.1).

1. **A route across a text, as the last resort** (2026-11-15, the M3 persona re-test). A tube from port 4 of a valve
   stacked under another one had no route: the facing port's stub reserved the tube's own stub, and the only way out
   ran through the valve's tag, so the tube stayed a dashed line through the valve body, "the worst outcome a person
   can get". Decisions 3 and 5 are refined: an end's own stub is never reserved by another connection point's stub
   on the same point; where no route keeps every hard rule, the Wire tool and the page solve look for one that keeps
   every hard rule but rule (d), the texts, check it against the layout measures, take it and say so (the Wire tool's
   hint, Route page's card row, `RoutePlan::over_texts`; EW-CORE-070 reports the text it crosses). A route through a
   body, a connection point or another wire is still never drawn.
2. **Crossing a text is a cost, not a prohibition** (2026-11-16, the rc.2 persona runs; decided by the coordinator
   under the owner's delegation). A wire from a switch to the breaker beside it went round the switch's tag and over
   the sheet (90 mm for a 42 mm way), and a bus line went round a whole station's box to keep clear of its three
   caption lines: worse drawings than a line across a text. Decision 5 and amendment 1 are refined: a route may cross
   a text also where every text-clear route is a far longer way. The search finds the best route across a text with
   a cost for it (entering a text `TEXT_COST` = 20, ten steps, and each step on a text a step more), and takes it only
   where the best text-clear route is more than 8/5 of its length (or there is none): a way round a text of a few
   steps and a bend or two is still taken, a detour round a symbol group or the sheet is not. (A cost alone cannot
   tell the two apart: a bend weighs four steps, so a short way round with two bends costs as much as a long one
   without; the rc.2 switch-to-breaker wire is 1.7 times as long round its texts, the earlier play-test's L wire to a
   power supply 1.27 times and its N wire 1.57 times, which stay round.) A route across a text keeps every other hard rule by the layout
   measures, else the texts block for it; the page solve routes the jobs that cross a text after the text-clear ones,
   so that a text-clear route keeps its way. Bodies, connection points and other wires stay hard. A device tag the
   route crosses moves off the wire where its symbol has a free place (the scene's tag rule, on every drawing of the
   page); a text that does not move is drawn under the line, which no renderer knocks out on a schematic page (since
   0.3.5 the line is cut round it: [ADR-0034](0034-schematic-drafting.md) amendment 3).
   Golden A's routes are unchanged at this cost. In the same round, a symbol drawn with a connection point on an end's
   point (an interruption point drawn on a pin) is that end's own for rule (c): the wire leaves the pin through it, as
   through the pin's own symbol (the persona's 0 V wire stayed unrouted under such a point).
3. **Half steps between close pins; corners never meet** (2026-11-17, the owner's rc.2 test; decided by the
   coordinator under the owner's delegation). A two-pole switch over a two-pole breaker, two grid steps between the
   rows of pins, wired cross-over: Reroute gave one wire a detour round the switch, and its corner met the other
   wire's corner in one point, which reads as a joint and is none. Two wires that change sides between two columns
   cross once, and each needs a row of its own to step sideways on; on the whole grid this place has one row, which
   is both wires' stub. Decision 5 is refined in three ways:
   - **Corners never meet.** A route never puts a corner on another wire's corner or end, when it is drawn and when
     it is rerouted, a hard rule like "never through a pin" (the search blocks every vertex of another wire, an end's
     stub included, but where that wire leaves the same pin along the stub). Amendment 1's shared stub (a free
     connection point facing an end two steps away) is still the end's own; a stub point that another connection
     point's wire leaves through is that wire's. Reroute mends an older drawing with such a place. No check reports
     it (the reference drawing would grow findings).
   - **The half grid, last.** Where the whole grid gives no route, or one more than 8/5 of the direct way between the
     ends (amendment 2's measure), the router searches half the page's grid: bodies and the frame grown by half a
     step, the stubs of the other pins and the texts as before, a free pin's half step kept too (a wire passing right
     in front of a pin reads as joined to it). A step between the page's grid lines is taken only between the two
     ends (its track strictly between their coordinates, within their box) and costs a step more, so the route
     leaves the grid lines only for the stretch that needs it. Its route is taken where the whole grid's is more than
     8/5 of it or there is none. The owner's two wires then leave their pins by half a step, cross once between the
     devices and run straight otherwise.
   - **Commands accept a corner on the half grid** (`require_route`): a stored route's interior corner may lie on
     the page's grid or half way between two of its lines. The format is unchanged (a route is a list of points);
     an older build draws such a route as stored.

   Golden A's routes are stored and not rerouted; its pictures and its routes are unchanged by this.
4. **One number for a grid position; a reroute starts from the drawing** (2026-11-18, the owner's rc.4 test; decided
   by the lane under the owner's delegation). A wire from a junction on a vertical wire to the pin beside the one
   below left the junction to the left, went down and came back under the wire it started from, and EW-CORE-068
   reported it lying on that wire. He asked for "a deep look into the autorouter, optimised also for larger
   drawings". Decision 5 is refined in four ways:
   - **A grid position is one number to the router and the measures.** On a grid whose step is not a whole number
     of nm (the QET grid, 7 937 500/3) a pin's point is its symbol's or box's place plus the pin's offset, each
     rounded, and can lie 1 nm beside the number the grid gives the same position (`Grid::at`). A straight wire
     between two such pins on one grid line holds a step of 1 nm at its middle. The router read that step as a
     stretch of wire in the way of the first step beside it (the owner's loop), and the measures as two bends, an arm
     of a junction and an overlap with the wire leaving over it. The page solve, the Wire tool's route and the layout
     measures now read the page with every coordinate within 1 nm of a grid position set to the grid's own number
     (`Grid::settle`, `PageGeometry::settled`); a route found there takes its ends' stored numbers again, and a
     vertex that a wire runs straight through is no corner to another route. The stored numbers are unchanged: a
     pin's point is still its place plus its offset, because Golden A's stored routes hold 68 coordinates that are
     such sums, and a change of the rule would move them.
   - **A reroute starts from the drawing and never leaves the page worse.** Reroute routed its wires from nothing,
     the longest first: on a full page the long wires took the straight tracks between the pins, the short ones
     stepped aside, and a wire that found no route kept its old one under the new routes of the others. A solve
     whose jobs have stored routes now rates two drawings by the wires without a good route (none, or one that
     touches a point, lies on a wire or runs through a body) and then by what the routes cost as drawn (steps, bends,
     crossings and texts by the search's weights), and takes the better, the first on a tie: the drawing as it is,
     each wire routed again against all the others and the new route taken where the old one breaks a hard rule or
     costs more (at most two passes); and the jobs routed from nothing, the shortest first. A wire that is drawn as
     well as the router would draw it keeps its route, also where the tie rule would take another of the same cost.
   - **The first routing of a page solves both orders** (Route page on unrouted wires, an import, the wires a
     paste or a grid change redraws; decided 2026-11-18 by the main session under the owner's delegation of golden
     decisions). The jobs are routed from nothing the longest first (decision 5's order) and the shortest first, and
     the shortest-first drawing is taken where it rates better at no price a reader sees: no more wires without a
     good route, no more bends, no more crossings and no more texts crossed. The rating alone is not enough: on
     Golden A's folio 103 it prefers the shortest-first drawing (23 crossings for 27, at 54 bends for 52), which
     leads two wires round the back of their terminal strip to leave it the other way, runs a feeder across the
     sheet through four other wires and gives one wire a fifth bend; a draughtsman keeps the nested bundles of the
     longest-first drawing and their crossings, so folio 103 keeps its routes. Golden A changes on two folios, as
     many bends on each: folio 106 has 6 crossings for 14 (the loops under the two valve terminals nest instead of
     crossing), folio 203 none for 8 (each valve's second tube runs round its cylinder instead of across the
     first). Folio 101's routes are unchanged; its layout line counts 36 bends for 38 because the measures no
     longer count a step of 1 nm as two.
   - **No junction on a crossing.** The Wire tool does not end a wire on another at the point where a third wire
     crosses it: the dot would sit on both wires and read as a joint of four arms.
   - **Other comparisons of page points read them settled too** (2026-11-19, the review of the candidate). Outside
     the router and the measures, the places that two numbers of one grid position can reach are mended with the
     same reading, none with a tolerance of its own: a symbol dropped on a wire is wired in where its pins lie 1 nm
     beside the number of the wire's line (normaliser 7 cuts the wire on its own line and ends each part on its
     pin's number); an autoconnect hint between two pins on one grid line stores the straight run with its step,
     not an unrouted wire; EW-TERM-006 reports a terminal 1 nm beside the number of a wire it lies in, and sees a
     junction on a terminal's pin as joining; the interruption-point tool finds the pin or the wire under its
     point, and connects with the step as the wire's route; the Wire tool's test for a crossing reads settled. A
     box's pin that a reshape leaves where it is keeps its place also 1 nm beside the grid, as an untouched corner
     does; a pin that is new or moved stands on the grid. Left exact, because the tools hand in the wire's and the
     pins' own numbers: the split point and the route's ends that the commands check (`require_split_point`,
     `require_route`), and `drawn_path`'s joining of a stale end.
   - **The texts beside a multipin plug's pins are texts of the router's page.** The symbols layer draws them, so
     the page's geometry did not hold them and wires ran across them. Each is a text box now, cut back to a quarter
     of a grid step from the line its own pin's wire leaves along (the note stands right beside that line), so the
     pin's wire runs past its note and every other wire goes round it.

   Less work on a full page, with the same routes: the search marks the crossings and the mates along each wire
   instead of testing every node against every wire, and a job with no way between its ends on the grid or the half
   grid is searched for once in a solve. Golden A's routes change on folios 106 and 203 only, as said above.
5. **A reroute goes round a text where the Wire tool would; Reroute says what it changed** (2026-11-19, the rc.5
   persona runs; decided by the coordinator under the owner's delegation). A wire through any text is worse than a
   wire with a bend more that keeps clear of it, also where the text is one of a device the wire ends at, unless no
   clear way exists or every clear way is far longer (amendment 2). The search already ranked so; a reroute did not:
   weighed as drawn (amendment 4) a text costs as much as a bend and six steps, so a stored wire across a text kept
   its route where the search offered one round the text a few steps longer. A reroute now takes the new route where
   the stored one crosses a text and the new one keeps clear of every text. Two wires side by side keep one grid step
   between them (decision 5; the half grid of amendment 3 only between close pins). On the persona's supply page no
   wire crossed a text: the two wires from the breaker to the power supply loop over the supply's caption, which
   stands above the box left of its first top pin, where every wire from the left into its top pins passes; no way
   clear of it is without that loop, and the way across it is the worse drawing, so Reroute keeps them. Reroute's
   toast names the wires it changed, by their numbers or their ends, and the page draws them as selected wires while
   the toast is up; where nothing changes it says that the wires already run as well as the router can route them.
   Later in the same round a whole-page Reroute made a page worse: routed from nothing it had nine crossings fewer and
   rated better, but one wire ran across a sensor's text that its stored route went round, a check's warning more. A
   reroute now never breaks a hard rule a wire kept, wire by wire and for the page: a new route, and the drawing routed
   from nothing, is taken only where no wire touches a point, lies on a wire or runs through a body that it kept clear
   of, nor runs across a text it kept clear of unless that rids it of one of those faults.

## Related

- **Work package:** WP-S2.19 specification; WP-S2.04 (the router's
  origin, §8.1); FT-18, FT-517, FT-1434.
- **ADRs:** [ADR-0003](0003-freshness-contract.md), [ADR-0007](0007-explicit-connections-and-autoconnect.md) (amendment
  3 records decision 8), [ADR-0008](0008-transactions-undo-preview.md), [ADR-0021](0021-importers-emit-format-level-records.md),
  [ADR-0027](0027-native-symbol-grid-and-qet-grid-policy.md).
- **Design:** [schematic editor](../design/06-schematic-editor.md) §3.2, §3.5, §4.4, §5.4; [checks](../design/15-checks-revisions-collaboration.md) §4.
- **Requirements:** FR-CON-001, FR-CON-003, FR-CON-004, FR-CON-007, FR-CON-031, FR-XRF-009, FR-XRF-014, NFR-REL-07.
- **Decision log:** ID-564, ID-1325, ID-1416, ID-1421, ID-1498.
