<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# WP-S2.19: the schematic autorouter (closed specification)

**Status:** specified in cycle 53 by the documents lane RTR-SPEC (decision log ID-1498), against the working branch at
`f2c0bd821` · an M2 package that ships in 0.1.0's third candidate · one implementation lane, RTR-A, which builds the
obstacle model and the layout measures first and is resumed with this specification (§10) · **Date:** 2026-11-07 ·
**Work package:** [WP-S2.19](../02-work-packages.md#wp-s219-the-schematic-autorouter) · **ADR:**
[ADR-0033](../../adr/0033-schematic-routing-model.md) · **Part of:** ElektraWatt plan, specifications

**Purpose.** On Golden A's folio 101 in rc.1 every connection had no stored route and was drawn by the provisional path,
one at a time, so wires ran through connection points they do not join, along other wires and through symbols. The
owner: "it looks more like abstract art to me than an electrical diagram"; "for RC3 we need to implement a proper,
intelligent, autorouter. It should intelligently set up wires based on standard electrical drawing theory." This
package replaces the one-wire-at-a-time stand-in with a page router that keeps hard drawing rules, weighs the costs of
standard practice, routes a page or a selection in one previewed step, and stores what it drew. It also settles the
interruption point's arrow (the owner: "the arrows all pointing to the right. But incoming signals connected to the
tip of the arrow, and outgoing to the line") and one meaning of a stored route. **WP-S2.19 takes over FT-1434 (R0-F5),
FT-517 and FT-18**: their rows close into this package when it is accepted. A choice a lane meets that this document
does not settle is escalated, never guessed.

## 1. Goal and non-goals

**Goal.** Every connection drawn on a schematic page has a stored orthogonal route that breaks none of the hard rules
of §5.1, found deterministically by one router that every writer uses: the Wire tool's ghost, Route page and Route
selection, the move of a representation, and the tool that writes Golden A's routes. A connection the router cannot
route is never drawn through anything: it stays unrouted, is drawn as an unrouted connection and is reported. An
interruption point's arrow points right, with the target's wire on its tip and the source's on its tail.

**Non-goals.** Panel-layout routing in ducts (WP-S4.03 and later, `RouteWires`); placing or moving symbols to make room
(the router never moves a representation); creating, splitting or merging junctions (connectivity is the model's,
ADR-0007); hops beyond FR-CON-031 as built; single-line, fluid and bus pages' own conventions (the router routes them
with the same rules, no special case); vertical-flow interruption points (§12, OQ3); routing at render time (rejected,
ADR-0033); a negotiated-congestion router (rejected for 0.1.0, §5.5).

## 2. Inputs and the code as it is (base `f2c0bd821`)

| Area | As it is | What changes |
|---|---|---|
| The router | `crates/ew-commands/src/route.rs`: one connection at a time; `RouteRequest` :127 (ends with direction, obstacle rectangles, grid, area, `arrive`, `own`, `wires`, `reserved`); exact backward Dijkstra then forward walk (`search` :594); cost 1 per step, `BEND_COST` 4 (:62); no U-turn; tie rule (cost, bends, move sequence right < down < left < up); `MAX_NODES` 262,144 (:59); without a path the L of `fallback` :373 (`direct: true`) | Moves to `ew_derive::routing` (D4), keeps its search and tie rule, gains the costs of §5.2 and a page solve (§5.3 to §5.6); the fallback L is retired (D3) |
| The wrapper | `crates/ew-api/src/routing.rs`: its own `RouteRequest` with `Dir`; `auto_route` :85, `route` :104, `clear` :115, `direct_route` :121 | Re-exports the router from `ew-derive` and adds the page API of D4 |
| The Wire tool | `crates/ew-viewmodel/src/tools/wire.rs`: obstacles built in the tool (`rep_rects` :634, bodies grown one step and texts a quarter step; `frame_rects` :701; other wires and reserved CPs in `request`); `Pass::ALL` = Clear, OverTexts, Bodies (:729) × `AREA_MARGINS` [4, 16] (:627); FT-705's through route first; FT-1210's turned IP end | Takes the page geometry from the host and routes through D4's API; only the Clear pass remains (D13) |
| The provisional path | `ew_derive::provisional_path` (`crates/ew-derive/src/drawn_path.rs:67`), an L by the ends' directions; `drawn_path` :102 draws `[a] ++ route ++ [b]`; `ew-scene` `route::connection_polyline` :59 and `provisional_route` :80 (marked PROVISIONAL), read by `page.rs` `polyline_of` :776; read also by normaliser 7 (`normalise/connection_split.rs` via `geometry::provisional_path_of` :186), EW-TERM-006 (`ew-checks/src/rules/term.rs:831`), the page tables (`ew-api/src/output/editing.rs:1986`, :1991) and `describe` ("route stored/provisional bends N", `ew-viewmodel/src/describe.rs:3249`) | Retired: a route-less connection is unrouted (D3) |
| Stored routes | `Connection.route` (`crates/ew-schema/src/connect.rs:45`), "Orthogonal route in nm, in model order", omitted when empty; written by `connection.draw`, `connection.draw_many`, `connection.route`, `connection.draw_to_ip`, all checked by `geometry::require_route` (:488: on the grid, the first vertex at end `a`'s CP, the last at end `b`'s); `rep.move` follows the ends (`follow_end` :263) | One convention, the ends included (D2); `connection.route_many` added (D12) |
| The convention conflict | Writers store the ends (`require_route`, the tables importer's `stored_route` `emit/connect.rs:322`, the Wire tool, the route editor); readers add them again (`drawn_path`, `connection_polyline`), so folio 203's routes draw duplicated ends ("bends 5" for three corners); the QET importer stores the interior only (`project/wires.rs` `route_of` :304) | D2 |
| Interruption points | `ew.gen.ip` and `ew.gen.ip@qet10` (CP at the tail, pointing up; the arrow points down, tip at the origin); orientation free (`Representation.orient`), turned by the IP tool's keys; `ip.place` (`commands/connection.rs:680`) takes `orient`; `ip.continue` copies it; `ip.arrange` keeps it; Golden A's 44 interruption points are unturned (`representations.csv` has no orientation column) | D16 |
| Autoconnect | Proposals walk straight from a free CP (WP-S2.04 §8.3), so a proposal is always a straight run; `device.place`'s hints make route-less wires (`commands/device.rs:410`); a move shows proposals but never commits them; `tool.connect_on_move` (C) is refused (`shell.rs:1664`) | D12, D15 |
| Obstacle model and measures | RTR-A builds them now: `ew_derive::routing` (a page's geometry in plain nm types, the measures (a) to (g)) and the adapter in `ew-api` that fills it from the page tables and the scene's text boxes; the `ew layout` report | This package builds on them by their as-built names and does not redefine them |

The full facts, with every reader, test and caller, are the scout's at `7eaf5e8f1`; the lines above were re-read at the
base.

## 3. Requirements traced

FR-CON-001 (an explicit connection with a graphical route), FR-CON-003 (autoconnect as an editing aid, here on move as
well), FR-CON-004 (moves never change connectivity; routes follow), FR-CON-007 (orthogonal routes created
automatically, edited by the user and stored in nm), FR-CON-031 (crossings plain or as hops), FR-XRF-009 (the arrow's
direction is a signal-direction property), FR-XRF-014 (the counterpart point with the direction reversed). Seven rows.
NFR-REL-07 (determinism) and ADR-0003's reality 3 (drawings look identical years later) shape D1 and D10.

## 4. Decisions

| # | Decision | Why |
|---|---|---|
| D1 | **Stored routes are the drawing.** A connection on a page is drawn along its stored route and nowhere else. The router runs only when something writes a route (the Wire tool, Route page, Route selection, a move's re-solve, Golden A's route tool); no reader routes, so a drawing never changes because the router changed, and a re-route is an explicit, previewed step (ADR-0003's committed computations) | Stable drawings across releases and edits; every reader (scene, PDF, checks, normalisers, describe) agrees because they read one list |
| D2 | **One route convention: both ends included.** A non-empty `route` is the full vertex list from the point of end `a` to the point of end `b`: its first vertex is end `a`'s point (a CP's or a junction's), its last end `b`'s, every corner between on the page grid (or on an end's coordinate, as today). `require_route` checks a junction end's point too. A reader draws the list as stored, merging equal neighbours; where an end's point differs from the list's end vertex (a stale route), it joins that end with one straight segment. The schema's text (both `schemas/0.1/connection.schema.json` and the doc comment): "The orthogonal route in nm, in model order: every vertex from end a's point to end b's, both included. Empty: the connection is not routed." The QET importer stores its conductors in this form (D17). A format meaning, free until 0.1.0 (ID-1325): no migration | The writers already store the ends and the command validates them; changing two readers and one importer is the smallest change |
| D3 | **Unrouted connections; the provisional path is retired.** A page connection with an empty route whose ends lie at two different points is *unrouted*. It is drawn on the canvas and in every export as a dashed straight line between its ends, in the wire's colour at its thinnest pen, never as an L. It has no wire geometry: normaliser 7 does not split it, EW-TERM-006 and autoconnect do not see it, and the measures count it only as unrouted. `describe` says "unrouted" where it said "provisional". A route-less connection whose ends meet (FIXPT41's zero-length wire) is drawn as nothing and is not unrouted. `ew_derive::provisional_path`, `geometry::provisional_path_of`, `route::l_route`, `route::fallback` and `ew_api::routing::direct_route` are removed with their callers; FT-1434's stub rule (ID-1416) is not built | ID-1498: the provisional path held to the hard rules or retired; it cannot be held to them below `ew-api` (the text boxes need the scene's measurement), so it is retired, and what cannot be routed says so instead of being drawn through |
| D4 | **Where it lives.** The router core is a set of pure, integer-only functions in `ew_derive::routing`, beside RTR-A's page geometry and measures: the single-connection search moved from `ew_commands::route` with its tests (`ew-commands` keeps no router; `ew-commands` callers use `ew-derive`'s), and the page solve `route_page(geometry, jobs) -> plan` (§5). The page geometry is built in `ew-api` by RTR-A's adapter (the text boxes come from the scene) and cached per (revision, folio) like the page tables. The one API: `ew_api::routing` re-exports the core and adds `Session::page_geometry(folio)`, `Session::route_plan(folio, scope)` (Route page, Route selection, the move's re-solve, Golden A's route tool) and `route_one(&geometry, from, to, options)` (the Wire tool's ghost, one job against the page as drawn). The host puts the page geometry into the tool context as it puts the page tables (WP-S2.04 D6: tools never call the `Session`). `ew-scene` never routes | `ew-scene` cannot depend on `ew-commands`; `ew-derive` is below every caller; the text boxes exist only where the scene runs, so the geometry is built in `ew-api` and handed down as plain data |
| D5 | **Hard rules** (§5.1): never through a connection point the wire does not join, never along another connection, never through a symbol body or a text, never over the frame's bands, never through a junction it does not end at, never into a fourth arm of a junction. A route that breaks one is never returned | ID-1498's hard rules; the Wire tool's Clear pass keeps all but the last today |
| D6 | **Costs of standard practice** (§5.2), integers only: a step 2, a bend 8, a right-angle crossing 12, a step beside a mate (§5.4) at one grid step 1 instead of 2. The tie rule stays (least cost, then fewest bends, then the move sequence right < down < left < up) | IEC 61082-1's few bends and crossings and parallel wires at one spacing (unverified); the bend-to-step ratio of today |
| D7 | **Order: rails and long connections first.** The jobs of a solve are routed in the order of §5.3: connections of a supply potential first, then the rest, each group by falling span, mates together in their fixed order, ties by the page tables' order | The connections with the fewest good alternatives claim the straight lines; signal wires adapt around them |
| D8 | **Track assignment by mates.** Connections whose two pairs of ends lie side by side are mates; mates are routed one after another in the fixed order L1, L2, L3, L, N, PEN, PE, FE, L+, M, L−, then by end position, and a step beside an already routed mate costs less, so a group runs parallel at one grid step (§5.4) | Parallel wires in a fixed order and spacing, without a second search |
| D9 | **Rip-up and reroute, bounded.** After the first pass, each failed job rips up the routed jobs that block its free path, is routed first, and the ripped jobs are routed again; at most two such rounds, each kept only if it routes more jobs (§5.5). Each search grows its area 4, then 16 grid steps around the ends, then the whole drawing area, never over `MAX_NODES` | Sequential routing fails late jobs on crowded pages; two bounded rounds fix the usual case without a negotiated router's cost |
| D10 | **Determinism.** The plan is a function of the page geometry and the job set alone: inputs sorted on entry, integer costs, the tie rule, the order key ending in the connection ID, no hash map; the same plan on every platform and for any order of the input | NFR-REL-07; Golden A's routes are generated |
| D11 | **No route: report, never draw through.** A job without a route that keeps every hard rule after the rounds is left unrouted, listed in the plan with the reason, and drawn as D3 says. The plan never holds a fallback | ID-1498 |
| D12 | **Commands store, they never route** (ID-564 kept). New: `connection.route_many {routes: [{connection, route}]}`, several `connection.route`s in one transaction (one undo step), 1 to 10,000 entries, entry errors prefixed as `connection.draw_many` prefixes them. `device.place`'s hints and the new hints of `rep.move` (D15) store the straight route between their two ends (a proposal is a straight, clear run by WP-S2.04 §8.3), so a window placement leaves no unrouted wire. `connection.draw` keeps storing the route it is given; an empty one is unrouted | Replay never runs the router; the log holds the routes the preview showed |
| D13 | **The window.** Two actions, **Route page** (`route.page`: the page's unrouted connections) and **Route selection** (`route.selection`: the selected connections and every connection with an end on a selected representation, routed ones re-routed), in the Draw ribbon's Connect group after Edit route, show the plan as a preview (doc 16a §6.23's card) and commit `connection.route_many` on Apply. The Wire tool's ghost is `route_one`'s; the passes OverTexts and Bodies and FT-1210's turned end are removed; without a clear route the ghost is the unrouted line and the HUD says so, and a commit stores the wire unrouted unless the user fixed a bend (then the fixed bends and an L tail are the user's route, which the checks judge). A junction that already joins three wires is not an end target. The CLI gets `ew route <project> [--page <folio>] [--selection <ids>] [--dry-run] [--json]`, which saves through the same command | One previewed step; one undo step; the same path for the window, scripts and the CLI |
| D14 | **Re-solve on move (FT-18).** When a move of representations is released, the Select tool previews `rep.move`, and on the moved page geometry re-solves each connection with an end on a moved representation whose followed route now breaks a hard rule, has more bends than before, or which is unrouted; the others keep the followed route. It commits `rep.move` with a new optional member `routes: [{connection, route}]`, applied after the follow and checked by `require_route`, in the same transaction. The drag shows the followed routes; the re-solved ones appear on release. A script's `rep.move` without `routes` keeps today's follow | FT-18 without routing inside a command; a user's shape is kept while it stays clean |
| D15 | **Connect on move (ADR-0007 open question 2).** R1's B1: two "connect", one "ask". A move proposes autoconnect wires by default, as a placement does: the proposals the drag shows commit with the move as `rep.move`'s new optional `connect: [{rep, cp, to}]` hints (each a `device.place` hint naming its representation), in the same transaction. Holding the keymap's place-without-wires modifier at release moves without them; `tool.connect_on_move` (C) toggles the default for the session and the HUD shows its state. ADR-0007's decision 4 holds: no existing connection changes, and the new wires are shown before release | The majority answer, with the "ask" answer one key away; ADR-0007 amendment 3 records it |
| D16 | **Interruption points** (§7). Each of EW's IP symbols comes in two forms, drawn with the arrow pointing right and its tip at the origin: the source form (`ew.gen.ip`, version 0.2.0, redrawn), the wire on the tail, the CP one grid step behind the tip, leaving left; the target form (new `ew.gen.ip-target`, 0.1.0), the wire on the tip, the CP at the tip, leaving right; each with its `@qet10` twin. **Orientation is normalised:** an IP drawn with one of these forms is never turned (its `orient` is the default), and its form is its direction's. `ip.place`, `connection.draw_to_ip` and `ip.continue` refuse a non-default orientation or the other direction's form for these symbols in words; `rep.transform` refuses them; a direction change (`fields.set kind.ip.direction`) swaps the form by a new normaliser (doc 06 §3.2 row 18), refused in words when the project does not hold the twin; the IP tool and every embedding of one form carry both. Other IP symbols (imported, company) keep a free orientation | ID-1498: the arrow points right, the target's wire on its tip and the source's on its tail, the orientation following the direction, settled before format 0.1 locks |
| D17 | **Importers keep the source's routes.** The QET importer stores each conductor's route with both ends (D2): end `a`'s point, the points after each segment, the last equal to end `b`'s point, a coordinate within one QET px of an end's coordinate taking the end's exact value (the tables importer's rule). The tables importer's `route` column keeps its meaning (the corners between the ends) and its output already has the ends. EPLAN imports carry no connections. The import front door does not route (§12, OQ5) | Sources with geometry keep it; an imported route-less connection is unrouted and Route page draws it |
| D18 | **Golden A** (§8). Its interruption points take the forms of D16 in `representations.csv`; its route-less page connections get routes in `connections.csv`'s `route` column, written once by a route tool that imports the tables, routes each page with `route_plan`, and writes the corners; the fixture is regenerated from the tables by the existing bless test, so `golden/J-E-2416.ew` is never opened for writing. The golden decision is the orchestrator's | The source tables stay the one truth of Golden A; a later router change shows up as an explicit golden diff, not as a silent redraw |
| D19 | **The measures as check rules** (§9): EW-CORE-067 to -070, -072 and -073 in the EW-CORE pack's texts-and-graphics block, catalogued and worded in `ew-checks`, evaluated in `ew-api` from the page geometry and RTR-A's measures, and joined to the report on the engine's path for evidence the model does not hold (`CheckReport::with_merge_evidence` is the precedent), with the schemes, severities and waivers applied as for any rule | The text boxes exist only in `ew-api`; the engine already joins findings from evidence the model does not hold |
| D20 | **Format and API.** No record shape changes. The meaning of `Connection.route` changes (D2, D3) and the IP forms are library content (D16): both free until 0.1.0 (ID-1325), no migration, `schemas/0.1` re-blessed for the description only. Command schemas change: `connection.route_many` new; `rep.move` gains `routes` and `connect`; the refusals of D16. Each new command and member gets its schema, catalogue row, DSL verb or argument and fuzzer arm, and the release notes' unreleased page says what changed | ADR-0033 covers the meaning change, so no lane stops on it |

## 5. The rules

### 5.1 Hard rules (never broken by a returned route)

On the page grid, a route is a sequence of unit moves between grid nodes from end `a` to end `b`. It never:

1. **passes a connection point it does not join**: the node of every other CP and of its stub (one step along its
   leaving direction) is blocked (FT-706's reservation, kept); a second wire on the same CP may share that CP's stub;
2. **runs along another connection**: a move parallel to another route's segment closer than one grid step with a
   positive overlap is not taken, and the node of every vertex of another route (its ends and corners) is blocked, so
   a route crosses another only at a right angle through the inside of a segment (kept);
3. **passes through a symbol body**: every representation's body and black-box outline, grown by one grid step, blocks
   its nodes and the moves through it, except along an end's own escape corridor (kept);
4. **passes through a text**: every text box the scene draws (tags, slot texts, pin texts, cross-reference texts),
   grown by a quarter grid step, blocks the moves across it (FT-706's Clear pass, now the only pass);
5. **runs over the frame**: the bands outside the frame grid's drawing area block, with FT-758's exemption for an end
   within one step of the area's edge (kept);
6. **passes a junction** it does not end at (a junction's point is a vertex of its arms, rule 2);
7. **makes a four-way junction**: a route ending at a junction arrives from a side no other arm of the junction
   occupies; a route that would be its fourth arm is not returned (measure (g); the junction is reported, §9).

These are measures (a) to (d) and (g) of RTR-A as rules of the search; the property tests of §10 use RTR-A's measures
as the independent oracle.

### 5.2 Costs (what standard practice prefers)

| Cost | Value | Source |
|---|---|---|
| A unit step | 2 | — |
| A bend | 8 (four steps, today's ratio) | IEC 61082-1:2014, connecting lines with as few bends as possible (unverified) |
| A right-angle crossing of another route | 12 (six steps) | IEC 61082-1:2014, as few crossings as possible; a crossing is not a junction and carries no dot (unverified); FR-CON-031 |
| A step beside a mate's route, one grid step away, in the same heading | 1 instead of 2 | IEC 61082-1:2014, parallel connecting lines grouped at equal spacing (unverified) |
| Turning back on the last step | not taken (FT-437) | — |

Signal flow left to right and top to bottom (IEC 61082-1:2014, layout, unverified) is the layout's, not the router's:
the router keeps it by the tie rule (right and down first), by leaving every CP along its direction, and by D16's
arrows. Reference designations stay next to their symbols and readable (IEC 81346-1 as ADR-0026 cites the series;
IEC 61082-1 for their placement, unverified): rule 4 keeps every wire off them. Symbols are drawn to IEC 60617 on the
grid of ADR-0027, so the CPs and the router share one grid.

### 5.3 Order of the jobs

A solve routes its jobs one by one in this order; the jobs not in the solve (other connections of the page) are fixed
obstacles with their stored routes; unrouted connections outside the solve are not obstacles.

1. **Class.** First the connections of a supply potential (potential type L1, L2, L3, L, N, PEN, PE, FE, L+, M or L−),
   then the others (signal, undefined, none).
2. **Span**, falling: the Manhattan distance between the two ends' points.
3. **Mates** (§5.4) are routed together, at the place of the first of them, in the fixed order.
4. **Ties** by the page tables' connection order (first drawn end's point, then the last's, then the connection ID).

Rails first: a supply rail drawn as a chain of connections through junctions along a row (the reference's current
path layout, IEC 61082-1 (unverified)) claims its straight line before the connections that cross it, and the
vertical current paths from the rails' junctions are routed before the signal wires between them. The router never
creates a rail; it keeps the one the drawing has.

### 5.4 Mates and track assignment

Two connections are **mates** when their ends pair up side by side: each end of one lies on one horizontal or vertical
line with an end of the other, at most two grid steps apart, and both pairs face the same way (their CPs' leaving
directions are equal, or both are junctions). A group of mates is the transitive closure. Mates are routed one after
another in the fixed order L1, L2, L3, L, N, PEN, PE, FE, L+, M, L− (then the rest by the position of their `a` ends,
top to bottom, then left to right), and each mate's search counts a step beside an already routed mate of its group at
one grid step in the same heading as 1, not 2. So a three-phase feed from an interruption-point stack to a breaker runs
as three parallel wires one grid step apart, in L1, L2, L3 order where the ends allow it. Where the ends' order
contradicts the fixed order, the wires cross as few times as the costs allow, and the crossings are counted by
measure (e).

### 5.5 The search, its areas and the rip-up rounds

1. **One job** is RTR-A's geometry seen by the search of `route.rs` as it is (the stub, the escape corridors, the
   reserved CPs, the tie rule), with §5.1 and §5.2. Its area grows: the ends' bounding box with a margin of 4 grid
   steps, then 16, then the whole drawing area, each clipped to the drawing area and never over `MAX_NODES`; the first
   area with a route wins.
2. **First pass.** Every job in the order of §5.3; a routed job is an obstacle for the jobs after it.
3. **Rounds** (at most two). For each failed job `f` in order: route `f` against the fixed obstacles only (its free
   route); rip up every routed job of the solve that `f`'s free route meets against a hard rule; route `f`; route the
   ripped jobs again in their order. A round is kept only if it leaves fewer failed jobs (then the lower total cost);
   otherwise the solve keeps the result before it.
4. **Bounds.** At most (1 + 2 rounds) × 3 areas searches per job and `MAX_NODES` nodes per search; no timing is
   asserted (the owner's rule).

A negotiated-congestion router (each node's cost rising with its use over many iterations) is rejected for 0.1.0: its
result depends on iteration counts that are hard to explain to a user, and a schematic page has tens of wires, not
thousands; a follow-up row proposes it for 1.0 should the rounds fall short.

### 5.6 Determinism and the plan

`route_page(&geometry, &jobs) -> RoutePlan { routes: [(connection, route)], unrouted: [(connection, reason)] }`. The
geometry's lists and the jobs are sorted on entry, so the plan does not depend on their order; every cost is an
integer; the order key ends with the connection ID; no hash map feeds the result (NFR-REL-07). Each route is in D2's
form. A reason is one of: no route keeps clear of the other wires, of a symbol, of a text, of a connection point, or
the junction would get a fourth arm; it names the first rule the free route broke.

## 6. The commands and actions

| Name | Kind | Members | Notes |
|---|---|---|---|
| `connection.route_many` | command (new) | `routes: [{connection, route}]`, 1 to 10,000 | Each entry as `connection.route`, in order, one transaction; errors prefixed `/routes/<i>`; `Bulk` above 500 entries |
| `rep.move` | command (amended) | `routes: [{connection, route}]` (optional), `connect: [{rep, cp, to}]` (optional) | `routes` replaces the followed route of each named connection after the follow (`require_route`); `connect` makes hint wires as `device.place` does, with their straight routes |
| `device.place` | command (amended) | none | A hint stores the straight route between its two ends |
| `ip.place`, `connection.draw_to_ip`, `ip.continue` | commands (amended) | none | D16's two refusals for EW's IP forms; `ip.continue` gives the partner the other form |
| `rep.transform` | command (amended) | none | Refuses a representation drawn with one of EW's IP forms |
| `fields.set` | command (amended) | none | `kind.ip.direction` on such a point swaps its form (normaliser 18) |
| `route.page`, `route.selection` | view actions (new) | — | Menu Draw, group Connect, after `tool.route_edit`; no default key; previewed |
| `tool.connect_on_move` | view action (amended) | — | No longer refused: toggles connect on move for the session |
| `ew route` | CLI (new) | `<project> [--page <folio>] [--selection <ids>] [--dry-run] [--json]` | Text: the summary line and the unrouted list; JSON: the plan |

## 7. Interruption points

| Item | Source form (`ew.gen.ip`, 0.2.0) | Target form (`ew.gen.ip-target`, 0.1.0) |
|---|---|---|
| Arrow | Points right, tip at the origin, the size of 0.1.0 (3 mm long, 3 mm wide natively; the `@qet10` twin scaled as 0.1.0's) | The same |
| Wire | On the tail: a stub from the CP to the arrow's base | On the tip |
| CP | One grid step left of the tip, leaving left | At the tip, leaving right |
| Name text (function text) | Above the arrow, centred | The same |
| Cross-reference text | Below the arrow, right-aligned at the tip (FR-XRF-011's default) | The same |
| Anchor | The arrow tip (doc 20 §4.4's anchor policy) | The same |

A source sits at the right edge with its wire coming from the circuit on its left; a target at the left edge with its
wire leaving into the circuit on its right; both arrows point the way the signal flows. The direction quarter rule of
the IP tool stays (`quarter_direction`); D toggles the direction and with it the form; the rotate and mirror keys are
not offered for these forms. `ip.continue` places the partner with the other form at the opposite edge. `ip.arrange`
keeps forms and orientation. The QET importer keeps the source's own symbol and orientation (an imported point is not
one of these forms). The external-arrow symbol keeps its free orientation (§12, OQ4).

## 8. Golden A

1. **The interruption points.** `representations.csv` names `ew.gen.ip@qet10` for every source point and
   `ew.gen.ip-target@qet10` for every target; the cells stay (the insertion point is the arrow tip as before). The
   tables importer refuses an EW IP form that does not match its function's `ip_direction`, in words.
2. **The routes.** A route tool (an `ew-api` test in bless mode, `EW_BLESS_GOLDEN_ROUTES=1`) imports the tables with
   the router-written rows' routes left out, routes folios 3, 101, 102, 103, 106 and 203 with `route_plan`, and writes
   each route's corners into `connections.csv`'s `route` column in the column's grammar (`<cell>` or `<cell> <dx>
   <dy>`); folio 203's hand routes (U-51, FT-1315) were removed and the page laid out for the router (ID-1503). In
   compare mode the test asserts that the committed column equals the router's result, so a router change is a golden
   diff. A corner the grammar cannot express (a grid point that is no whole QET px from its cell's centre) is a stop,
   with the probe.
3. **The fixture.** `golden/J-E-2416.ew` is regenerated from the tables by the existing bless test
   (`EW_BLESS_GOLDEN_FIXTURE=1`, `crates/ew-api/tests/it/golden_fixture.rs`); no session opens it for writing.
4. **What changes** (re-blessed, each compared side by side with the reference rendering of the previous candidate):
   `golden/J-E-2416/tables/representations.csv` and `connections.csv`; `golden/J-E-2416.ew`;
   `fixtures/format/0.1/J-E-2416.ew.expected/pages/*.jsonl`; `golden/J-E-2416/expected/pdf` (`sha256.txt`,
   `structure.json`), `expected/print`, `expected/png/<folio>.png`, possibly `expected/text/<folio>.csv`;
   `crates/ew-viewmodel/tests/expected/page_describe_101.txt`, `_102`, `_103`; `crates/ew-api/tests/it/drawn_path_agreement.rs`'s
   expectations; `crates/ew-api/tests/expected/findings_mvp.csv` (EW-TERM-006's and the new rules' findings); the
   AC-16 memo-sweep line if a memoised family is added; the README's "Router" gap closed. Any other golden byte that
   changes stops the lane for a ruling.
5. **The decision.** Routes from the router, not transcribed from the reference drawing: the reference's wire layout
   is not reproduced, and Golden A becomes the router's regression page. This is the orchestrator's golden decision
   (delegated, ID-1498's "Golden A routed and re-blessed"), proposed here.

## 9. The check rules

Evaluated in `ew-api` from the page geometry with RTR-A's measures, joined to the report (D19); catalogued in
`crates/ew-checks/catalogue/rules.json`, registered in `registry.rs`, worded in `messages/en-GB|nl-NL/rules.ftl`, each
with its rule note (`docs/book/rule-notes/<id>.md`, standard lines marked unverified) and a row in doc 15 §4. The
wire is named as `words` names a wire, a device by its tag, a pin as `-101E56:X1`. Pack EW-CORE, scope page, every
shipped scheme, waivable.

| ID (code) | Title | What | Measure | Kind | Modes | Severity (release) |
|---|---|---|---|---|---|---|
| EW-CORE-067 (104067) | Wire through a connection point | A drawn wire passes a CP it does not join; a terminal's CP is left to EW-TERM-006 | (a) | conflict | o r | warning (error) |
| EW-CORE-068 (104068) | Wire along another wire | A segment runs on another connection's segment with a positive overlap (the shared stub of one CP excepted) | (b) | conflict | o r | warning (error) |
| EW-CORE-069 (104069) | Wire through a symbol | A segment passes through a representation body other than at its own end's escape | (c) | hygiene | o r | warning |
| EW-CORE-070 (104070) | Wire through a text | A segment passes through a text box | (d) | hygiene | o r | warning |
| EW-CORE-072 (104072) | Four-way junction | A junction joins four wires | (g) | hygiene | o r | warning |
| EW-CORE-073 (104073) | Unrouted wire | A page connection has no route (D3) | unrouted | completeness | o r | info (error) |

Crossings (e) and bends (f) are costs, not rules: `ew layout` reports them, and §10's acceptance bounds them; a
threshold rule would be arbitrary. EW-CORE-071 (a graphic line on a CP) is unrelated and keeps its number.

## 10. Build order (one lane, from RTR-A's foundation)

| Step | Scope | Size | After |
|---|---|---|---|
| 0 | RTR-A as briefed: `ew_derive::routing`'s geometry and measures, the `ew-api` adapter, `ew layout` | (RTR-A's own brief) | — |
| 1 | Move the search into `ew_derive::routing` with its unit and property tests, unchanged in behaviour; `ew_api::routing` re-exports it; `ew_commands::route` removed and its callers switched; a property test that every found route breaks no hard rule by RTR-A's measures | 0.4 pw | 0 |
| 2 | D2: the readers (`drawn_path`, `connection_polyline`), `require_route`'s junction ends, the QET importer, `describe`'s bend count, the schema text; folio 203's and Golden C's outputs re-blessed if their bytes change | 0.5 pw | 1 |
| 3 | §5's costs, order, mates, areas and rounds; `route_page`, `route_plan`, `route_one`, the page geometry in the tool context; `connection.route_many`; Route page and Route selection with the preview card; `ew route` | 1.4 pw | 2 |
| 4 | The Wire tool on `route_one` (D13), the hints' straight routes (D12) | 0.5 pw | 3 |
| 5 | D16: the four symbol files, normaliser 18, the refusals, the IP tool, `ip.continue`, the tables importer's check | 0.6 pw | 3 |
| 6 | §8: Golden A's forms and routes, the route tool, one re-bless of every Golden A output, side by side | 0.5 pw | 4, 5 |
| 7 | D3: the provisional path retired, the unrouted line, normaliser 7 and EW-TERM-006 on routed wires only; the remaining expected outputs with route-less wires re-blessed | 0.5 pw | 6 |
| 8 | §9's rules | 0.5 pw | 7 |
| 9 | D14 and D15: the move's re-solve and connect on move | 0.5 pw | 3 |
| 10 | The documents of §13 as built; the persona briefs of §11.3 ready for the orchestrator's round | 0.2 pw | 1 to 9 |

The order keeps Golden A's outputs to one re-bless (step 6) and lands the unrouted drawing (step 7) only when Golden A
has no route-less page wire left. A step more than 20 % over its size triggers a re-estimate; the de-scope order is:
`ew route` (step 3), then connect on move (step 9), then the mates' cost (step 3, keeping the order), each becoming a
follow-up row. The hard rules, D2, D3, D16 and Golden A are never de-scoped. The lane keeps RTR-A's worktree and
context budget; at about 800k tokens it hands back with a hand-off note and a fresh lane continues (ID-1322).

## 11. Acceptance

### 11.1 The router (unit and property tests in `ew-derive`)

| ID | Item |
|---|---|
| RT1 | Every route `route_page` returns breaks none of §5.1's rules, judged by RTR-A's measures (an oracle independent of the search), over generated pages (proptest, the nightly's case cap) |
| RT2 | The plan is the same twice, and for the geometry's lists and the jobs given in reversed or shuffled order |
| RT3 | Three mates (L1, L2, L3) from an IP stack to a breaker run parallel one grid step apart, in that order, with no crossing among them |
| RT4 | A supply connection routed after a signal connection in the input still claims the straight line (order by class and span) |
| RT5 | A crowded case that the first pass fails and a rip-up round routes; a case no round routes is unrouted with its reason, and nothing is drawn through |
| RT6 | A crossing is preferred to a detour of more than six steps, and a detour to a crossing below that (the cost table) |
| RT7 | The moved tests of `route.rs` pass unchanged in `ew-derive` (tie rule, no U-turn, the corridor, a text thinner than a step, reserved points) |
| RT8 | A sparse generated profile (symbols at least four grid steps apart) is routed in full; a dense profile has no hard-rule violation and reports every unrouted job |

### 11.2 The model, the window and Golden A

| ID | Item |
|---|---|
| RC1 | A stored route is drawn as stored: folio 203 shows three bends for three corners, not five; a QET import draws as before and stores both ends |
| RC2 | An unrouted connection is drawn as the dashed straight line on the canvas and in the PDF, and normaliser 7 does not split it |
| RW1 | Route page on a page with unrouted wires previews the plan, Apply commits one `connection.route_many` (one undo step), and the toast and preview texts are those of §12a, asserted whole in both languages |
| RW2 | Route selection re-routes a selected routed wire; nothing else on the page changes |
| RW3 | The Wire tool with no clear route shows the unrouted ghost and its HUD text, and commits an unrouted wire; with a fixed bend it commits the user's route |
| RW4 | A junction with three wires is not offered as an end; the HUD says so |
| RM1 | Moving a symbol so that a followed route would pass through another symbol re-solves that route in the same undo step; a followed route that stays clean is kept vertex for vertex |
| RM2 | Moving a symbol onto aligned free CPs commits the proposed wires with the move; with the modifier held, none |
| RI1 | A placed source IP points right with its wire on the tail, a target with its wire on the tip; D swaps the form; `rep.transform` on one is refused in words |
| RI2 | `fields.set kind.ip.direction` swaps the form; without the twin held it is refused in words |
| RG1 | Golden A: every page connection of folios 3, 101, 102, 103, 106 and 203 is routed; measures (a) to (d) are zero on every page and (g) is no higher than RTR-A's report of the base |
| RG2 | Golden A's folio 101 names: the heater's PE wire (`E55:PE` → `PE-1.1`) does not touch `E56:X1`, the lamp's L feed does not run along the switch's wires, and the heater's L feed does not pass through `T55` |
| RG3 | Golden A's bends and crossings per page are written by the route tool's report into `golden/J-E-2416/expected/layout.txt` and reviewed side by side at the re-bless; a later change that raises a page's total fails the test (a ratchet), and no connection has more than four bends unless the file lists it with its reason |
| RK1 | Each rule of §9 fires on a hand-built page and is silent on its fixed twin; its message is asserted whole in both languages |
| RK2 | Golden A's check report holds none of EW-CORE-067 to -070 and -073 |

### 11.3 The persona round (the third candidate, the orchestrator's, ID-1497)

Three or four greenfield runs at 1920 × 1080 on rc.3, Opus at `low`, a checker only where a result is in doubt. Each
is judged by **correctness** (the saved project's connections equal the brief's netlist, by `ew sql`) and **the
measures** (`ew layout`: no hard-rule violation; bends and crossings reported), and by the keep score.

| ID | Persona, mode | Brief outline |
|---|---|---|
| RP1 | `eplan-designer`, visual | A new project; a three-phase motor starter on one page: L1, L2, L3 and PE from interruption points at the left edge, the starter contactor's main contacts, the motor; Route page; export the PDF |
| RP2 | `qet-user`, visual | A 24 V control page: the power supply fed from L, N and PE points, two pushbuttons, a relay with its self-holding contact, a lamp; the 24 V and 0 V points continued on a second page |
| RP3 | `eplan-maintainer`, visual | A copy of Golden A: move the lamp on folio 101 two columns right, add a second lamp in parallel, Route selection, check, export |
| RP4 | `qet-veteran`, text | RP1 through the text interface, `route.page` included |

## 12. Open questions, ruled or left with a stop

| # | Question | Ruling |
|---|---|---|
| OQ1 | Golden A's routes from the router or transcribed from the reference drawing | Router (§8.5); the orchestrator's golden decision, proposed |
| OQ2 | An unrouted wire drawn as the old L or as a dashed straight line | Dashed straight line (D3): the L drew through symbols and read as a wire |
| OQ3 | Interruption points on a top-to-bottom flow (arrow down at the top and bottom edges) | Not in 0.1.0: ID-1498 rules the arrow right; a follow-up row proposes a vertical pair of forms |
| OQ4 | The external-arrow symbol under D16 | Not in this package; a follow-up row |
| OQ5 | The import front door routing an import's route-less connections | Not in this package (an import stays a pure record batch, ADR-0021); Route page after the import; a follow-up row |
| OQ6 | Splitting a four-way junction into two T junctions | Reported only (EW-CORE-072); a junction is connectivity, not the router's; a follow-up row |
| OQ7 | A stale route (an end moved without the route following, as after a symbol exchange) | Drawn joined by a straight segment (D2); a follow-up row for a check and an automatic re-solve |
| OQ8 | FR-XRF-009's "▷ outgoing, ◁ incoming" | Read as the printed direction marker of the cross-reference table, which stays; 02a gets a note at the design pass that the drawn arrow follows ID-1498 |
| OQ9 | FR-XRF-011's acceptance (the IP cross-references within ±0.2 mm of the reference) | The cross-reference keeps its slot below the arrow, right-aligned at the tip; if a test pins the old position the lane re-blesses it under §8.4 and reports it |

No question needs the owner; no contradiction between ID-1498 and an ADR was found (ADR-0007's decision 4 and its
open question 2 are reconciled by D15; ADR-0003 supports D1; ID-564's "the commands do not route" is kept by D12).

## 12a. The texts

English is the source; the Dutch is the lane's proposal, the orchestrator's to decide (ID-1320). `{wire}` is the
wire as `words` names it ("-101F1:2 → -101E55:L"), `{n}` a count, `{page}` a folio, `{pin}` a pin, `{device}` a tag.

| Where | en-GB | nl-NL |
|---|---|---|
| Action | Route page | Pagina routeren |
| Action | Route selection | Selectie routeren |
| Preview card | Route {n} wires on {page}: {routed} routed, {unrouted} not routed. | {n} draden op {page} routeren: {routed} gerouteerd, {unrouted} niet gerouteerd. |
| Preview row, a reason | No route keeps clear of other wires: {wire}. | Geen route blijft vrij van andere draden: {wire}. |
| Preview row, a reason | No route keeps clear of symbols: {wire}. | Geen route blijft vrij van symbolen: {wire}. |
| Preview row, a reason | No route keeps clear of texts: {wire}. | Geen route blijft vrij van teksten: {wire}. |
| Preview row, a reason | No route keeps clear of other connection points: {wire}. | Geen route blijft vrij van andere aansluitpunten: {wire}. |
| Preview row, a reason | The junction would join four wires: {wire}. | Het knooppunt zou vier draden verbinden: {wire}. |
| Toast | Routed {n} wires on {page}. | {n} draden op {page} gerouteerd. |
| Toast, nothing to do | Every wire on {page} has a route. | Elke draad op {page} heeft een route. |
| Wire tool HUD | No clear route: the wire stays unrouted. Click to place bends. | Geen vrije route: de draad blijft ongerouteerd. Klik om knikken te plaatsen. |
| Wire tool HUD | This junction already joins three wires: end on a wire to make a new junction. | Dit knooppunt verbindt al drie draden: eindig op een draad voor een nieuw knooppunt. |
| Move HUD | C Connect on move: on / off | C Verbinden bij verplaatsen: aan / uit |
| Refusal (`rep.transform`, `ip.place`) | An interruption point's arrow points the way its signal flows: change its direction instead. | De pijl van een onderbrekingspunt wijst in de richting van het signaal: wijzig in plaats daarvan de richting. |
| Refusal (`ip.place`) | A source interruption point is drawn with the outgoing arrow; a target with the incoming arrow. | Een bronpunt wordt getekend met de uitgaande pijl, een doelpunt met de inkomende pijl. |
| Refusal (`fields.set`) | The project does not hold the {form} arrow of {symbol}: place one interruption point of that direction first. | Het project bevat de {form} pijl van {symbol} niet: plaats eerst één onderbrekingspunt met die richting. |
| EW-CORE-067 | Wire {wire} runs through {pin}, which it does not join. | Draad {wire} loopt door {pin}, waarmee hij niet verbonden is. |
| EW-CORE-068 | Wire {wire} runs along {other} on {page}. | Draad {wire} loopt over {other} op {page}. |
| EW-CORE-069 | Wire {wire} runs through {device}. | Draad {wire} loopt door {device}. |
| EW-CORE-070 | Wire {wire} runs through the text "{text}" of {device}. | Draad {wire} loopt door de tekst "{text}" van {device}. |
| EW-CORE-072 | The junction at {cell} joins four wires: draw two T junctions instead. | Het knooppunt in {cell} verbindt vier draden: teken in plaats daarvan twee T-knooppunten. |
| EW-CORE-073 | Wire {wire} has no route: Route page draws it. | Draad {wire} heeft geen route: Pagina routeren tekent hem. |
| `describe` | unrouted | (a describe word, not translated) |

`{form}` is "outgoing" or "incoming" ("uitgaande", "inkomende"). Every text is asserted whole (`assert_eq!`); the key
names (C, the modifier) come from the keymap.

## 13. Neighbours, owned files and the documents to update

**Owned.** `crates/ew-derive/src/routing/**` (with RTR-A), `crates/ew-derive/src/drawn_path.rs`, `crates/ew-commands/src/route.rs`
(removed), `geometry.rs`'s route checks, `commands/connection.rs`, `commands/device.rs`'s hints and `rep.move`, the
normalisers 7 and 18, `crates/ew-api/src/routing.rs` and the page geometry cache, `ew-scene`'s `route.rs` and the
connection drawing in `page.rs`, the Wire, Select, Place and IP tools, the two actions, the `ew route` subcommand, the
importers' route lines, the four IP symbol files and the starter library's version, `ew-checks`' new rules, the golden
files of §8.4, the fuzz arms and oracles (`crates/ew-testkit/tests/command_fuzz/`, `fuzz/src/`) and the DSL verbs.

**A lane may not decide.** Any golden byte outside §8.4; a record shape change; a new crate edge (`ci/dep-rules.toml`);
a dependency; the hard rules; the Dutch texts' final wording.

**Documents updated as built** (step 10): doc 06 §3.2 (row 18; the commands that store routes), §3.5 (the arrow),
§4.4 (the ghost, the unrouted end, the three-arm junction), §5.4 (this router replaces the section's), §11; doc 03 §9
(the route's meaning); doc 05 (the schema text); doc 08 §3 (unrouted connections); doc 15 §4 (the six rules) and its
family table's note; doc 16 (the actions, the HUD); doc 20 (the IP forms, the routes in the tables, README's "Router"
gap); 02a's notes on FR-CON-007 and FR-XRF-009; ADR-0033's status line; the release notes' unreleased page; the
user guide's wiring chapter.

## 14. Size and PERT

| Estimate | pw |
|---|---|
| Optimistic | 4.0 |
| Likely | 5.5 |
| Pessimistic | 9.0 |
| **PERT** | **5.8** (σ 0.8) |

The steps of §10 sum to 5.6 pw (RTR-A's step 0 is outside, briefed on its own), 0.2 pw under the PERT as a reserve
drawn on first by step 3. Not measured: no timing is asserted anywhere (the owner's rule).

## Sources

- Decision log ID-1498, ID-1497, ID-1421, ID-1416, ID-1411, ID-1325; [06](../06-decision-log.md) ID-564, ID-635;
  [ADR-0003](../../adr/0003-freshness-contract.md), [ADR-0007](../../adr/0007-explicit-connections-and-autoconnect.md),
  [ADR-0021](../../adr/0021-importers-emit-format-level-records.md), [ADR-0026](../../adr/0026-default-device-tag-identifier-set.md),
  [ADR-0027](../../adr/0027-native-symbol-grid-and-qet-grid-policy.md), [ADR-0033](../../adr/0033-schematic-routing-model.md).
- [06 schematic editor](../../design/06-schematic-editor.md) §3.2, §3.5, §4.4, §5.2, §5.4;
  [WP-S2.04 specification](WP-S2.04-placement-wiring-drawing-tools.md) §8.1, §8.3;
  [WP-S2.12 specification](WP-S2.12-usability-rounds.md) §5.5 (B1);
  [15 checks](../../design/15-checks-revisions-collaboration.md) §2.2, §4; [16a design language](../../design/16a-design-language.md) §6.23.
- IEC 61082-1:2014 (connecting lines, junctions, crossings, layout and signal flow: every clause cited here is
  unverified, as in the rule notes); IEC 60617 (the symbols, through ADR-0027); IEC 81346 (reference designations,
  through ADR-0026).
- FT-18, FT-517 ([02](../02-work-packages.md) Appendix B); FT-1434 and R0-F5 ([follow-ups](../follow-ups.md)).
