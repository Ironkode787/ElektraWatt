# Drafting: routing wires, arranging parts and settling texts on a schematic page: routing

A chapter of [Drafting: routing wires, arranging parts and settling texts on a schematic page](06a-drafting.md), whose table says where each section lives.

---

## 3. Routing

### 3.1 The method, and what was weighed

The method is **structure first, search last**, on nets:

1. wires that can be one straight segment are that segment;
2. a net with taps gets a trunk: a straight line with the taps on it and straight drops;
3. wires that must step sideways between two rows of pins get tracks, assigned together so that they nest;
4. what is left is routed one wire at a time by the exact grid search, shortest first, and improved in at most
   two passes;
5. the texts are placed against the result, and a wire that still crosses a text that could not move is routed
   once more.

*Alternative: one search per two-ended connection in a fixed order, with rip-up rounds and a comparison of
orders (the present design).* It has no way to draw a rail, its result depends on the order, and five amendments
in one release patched the order. *Alternative: negotiated congestion (PathFinder).* It cures the order problem
for a general graph, but its result is hard to explain to a draughtsman, and once straights, trunks and tracks are
taken out, few wires are left to compete. *Alternative: free tree shapes per net (rectilinear Steiner trees).* The
shape of a net is the wiring order the person declared (D-8) and is not the router's to choose. *Alternative: a
library (libavoid, ELK).* Neither routes on a grid with pin sides, rails and texts; the first is gridless C++
under the LGPL, the second Java. Everything here is written anew except the search, which is moved.

*Why the straights come first.* The search would draw most of them the same, but a straight laid first
keeps its line from the structure: a wire between two marks that face each other, passing between two devices whose
wire steps from one to the other on that line, stays one segment and the stepping wire takes its next track; searched
after the tracks, it went round the stepping wire.

### 3.2 Jobs

`route(page, jobs) -> Plan`. The caller names the wires that may be routed:

| Command | Jobs |
|---|---|
| Route, nothing selected | every wire of the page that is unrouted, has a fault as stored (F1 to F6), or is `computed` |
| Route with a selection | the selected wires and every wire with an end on a selected part, whoever shaped them |
| Arrange and route | every wire of its scope, whoever shaped it: with nothing selected every wire of the page, with a selection the jobs of Route with that selection; but the wires kept (§7.3) |
| A move by hand | the wires the move leaves unrouted or with a fault, or with more bends than before (as today) |

A route the person shaped (not `computed`) that has a fault is a job of a page-wide Route too, but it is flagged
**his**: the plan lists it apart, the command shows a preview instead of applying at once, and its row is ticked
because the wire is broken; he can untick it (§7.2).

Arrange and route flags his routes the same way, broken or not: each route of the person's that its plan would change
is a ticked row of its card, and unticking it keeps the wire as it runs (with the parts at its ends) and plans again
(§7.3). Its plan is judged against Route's plan of the same jobs on the page as it stands: a plan that Route's beats
(by the measure with the price of the moves added, or by a figure the card shows) is not offered; Route's plan is, and
the card says that no part moves.

Every other routed wire is **fixed**: an obstacle with its own right of way. A junction may move when every wire
at it is a job.

*A route that misses its end (decided 2026-11-22).* No command stores one: every command that moves a symbol,
a pin or a mark takes the routes at it along, and `connection.route` and `layout.apply` refuse a route that does not meet
its ends. A file written outside the program can hold one (a script set a mark beside its wire's line), and the page
draws the end joined to the route by one straight stretch: a hook. Such a route is broken as stored, so a page-wide
Route takes it, flagged as the person's when it is, and draws it to its end.

### 3.3 The occupancy

One structure, built once per run over the half grid of the drawing area and updated as wires are laid and
lifted: per node whether it is inside a body grown by `BODY_GAP`, a pin, a pin's stub, a junction, a vertex of a
wire; per move between two nodes which wires run along it and which cross it; per node the labels and fixed texts
over it. The search and the three structured phases read it. It replaces the per-request preparation of today's
search, which rebuilt the page for every wire.

It holds every node of the drawing area, the last area a search may take. It held what stood on
the page and two steps beyond, so a second trunk four steps beyond the pins it taps was laid outside it, and the
third trunk took the same line: on a page of three potentials over one row of poles two of them were left unrouted.

### 3.4 Nets, trunks and shared runs

**A net job** is the set of job wires joined through movable junctions, or through a pin that two or more of them
end at. Its **taps** are those junctions and those pins. A wire with no tap at either end is a net of one wire.

**Shared runs.** Wires that end at one pin may run on one line from that pin, as long as they go the same way; where
one turns off, the others run on. This is the common beginning that F3 allows. The scene draws the parting as the
trade does for two wires on one terminal: the wire that arrives along
the trunk turns into the drop by a square corner, and the wire that goes on leaves the drop half a step from that
corner by a 45° bevel (the size of the pilot's reference set; a whole step read as too large); there is no dot. (Today the scene draws this bevel only where two wires of one pin part
after their first step.) It is how a potential that is wired from pin to pin, the usual way in this program, is
drawn as a rail with drops instead of a staircase of rectangles. A part dropped on a shared run is refused in words ("two wires run here; place the part where they have parted").
A junction of the bevel form is drawn the same way, and the router sets the way it points from the routes that
meet at it; its symbol (`ew.gen.junction-bevel`) is that bevel too, half a step on each leg, so a junction and a
pin's parting draw one size.

**The trunk of a net with two or more taps**, in this order:

1. *The spine.* When the taps form a chain (each tap joined to at most two other taps), the chain is the spine;
   otherwise the longest chain is, and each side branch is routed afterwards as a net of its own that ends on the
   spine. A pin with several wires to pins that have no further wire (a fan) is a spine whose order is free: its
   taps are sorted along the trunk.
2. *Candidates.* For each axis (horizontal first), these track coordinates: the line the taps lie on now, when
   they lie on one; the coordinate of each end pin whose side runs along the axis (the trunk then runs straight
   into it: a mark at the sheet's edge); `TRUNK_GAP` beyond the outermost tapped pin on each side that pins face,
   and from there outward in steps of `TRUNK_PITCH` past the trunks already laid there; the middle between two
   tapped rows that face each other.
3. *Each tap's place* on a candidate: the coordinate of the pin it drops to, so that the drop is straight; a
   junction with several drops stands over the nearest of them, so that no two of its wires come to it along one
   stretch of the trunk. Two taps that would fall on one point (two pins in line on either side of the trunk) are
   set one step apart: two T junctions, never a four-way one (D-7).
4. *Valid* when the taps then lie along the trunk in the spine's order, no trunk segment or straight drop has a
   fault in the occupancy, and no tap falls on a crossing.
5. *Chosen:* the valid candidate of least cost (the trunk's length, each drop's length, 4 for each bend a drop
   or an end still needs, 6 for each wire it crosses), ties to the line the taps lie on now, then horizontal, then
   the smaller coordinate.
6. *Laid:* the trunk segments and the straight drops go into the occupancy; a drop that cannot be straight is left
   for the later phases, with its tap fixed.

A net with one tap has no trunk: the tap is tried at each crossing of a grid line through one end's exit with a
grid line through another's (at most nine points), between two ends that face each other along one axis (a drop
and what it feeds) on a line across, `TRUNK_GAP` from either or in the middle, in line with each end of the net,
and at its present place; the cheapest point with three arms wins, ties to the present place.

With no valid candidate the taps stay where they are and the net's wires are routed one by one. This is the case
when the person's wiring order runs against the parts' places (a rail tapped in the order 1, 3, 2): the plan says
so by name ("the taps of L+ are wired in another order than their parts stand"), because the person can mend it
and the router may not.

Trunks that tap one row of pins are laid in the order of their first tapped pin along the row (the leftmost
pole's trunk first, so it takes the nearest track and the others lie outward, each drop crossing only the trunks
of the poles before it: §1.3). Among other trunks the order is the supply ranks (L1 first, signals last), then
more taps first, then the first tap's place, then the ID.

`ew_drafting::nets` and `route` (module documentation there). A trunk is offered to a net whose
taps form a chain and to a pin's fan; a net whose taps branch keeps its taps and its wires go to the search one by
one (a side branch is not yet a net of its own on the spine). Each end of a trunk net has its foot on the line; a
wire runs end, foot, foot, end, so the wires of a pin tap share the drop and part on the trunk. A trunk is valid only
when every wire between two taps is free in the occupancy and at least two wires are; a wire whose drop is not
straight or not free goes to the search with its tap where the trunk put it, priced in the choice as its ends'
distance and two bends. A wire of a trunk that would lie along a wire of the same trunk laid before it, other than
on the run out of a pin both end at (two wires of one junction that leave it the same way: F3), goes to the search
too. The order of trunks is the supply rank, then the first tapped pin's place (left first), then more taps first,
then the ID. The shared run is part of the search (§3.6): a wire of a pin starts, or ends, on
each node of the run of a wire already laid from that pin, with that run's price, so it can only share a common
beginning. The note of an order that runs against the places names the net's first wire, not its potential.

### 3.5 Tracks

A **stepping wire** is a job between two ends that cannot be one segment and whose shape is known: a **Z**
between two pins that face each other and are not in line (it leaves along the first pin, steps sideways once,
arrives along the second), or a **U** between two pins on the same side (out, across, back). Its one free number
is the **track** of its middle segment, anywhere in the interval its pins leave.

Stepping wires of one direction whose intervals and spans overlap form a **channel**. Within a channel:

1. *Order.* Wire *p* must take a track before wire *q* (nearer to the row *p* leaves) when a leg of *p* would
   otherwise cross *q*'s middle: *p*'s first leg lies inside *q*'s span, or *q*'s last leg lies inside *p*'s span.
   Where two wires ask for both orders, one crossing is unavoidable and the pair is ordered by its ends' places.
2. *Assignment.* In that order (ties by the left end of the span), each wire takes the first track of its
   interval that no span on it overlaps and that is beyond the tracks of the wires before it in the order. Tracks
   are `TRACK_PITCH` (2 steps) apart, starting 2 steps from the row the wires leave; when the channel has too few
   such tracks for its wires, the assignment is made again 1 step apart. Wires of one pin may share a track where
   their spans meet at that pin's shared run.
3. *Laid* when the whole route is free of faults in the occupancy; a wire without a free track, or blocked, goes
   to the search.

This is the left-edge method of channel routing with its order constraints. It gives the nested steps of two
devices with different pole pitches, the fan of a row of sensors into a narrower row of pins, and stacks of marks
whose wires do not cross, without a search and without depending on any order of the wires.

`ew_drafting::tracks`, run by `route` after the trunks. A stepping wire is the one wire of its
net: the wires of one pin stay with the trunks and the search, which share their runs, so wires of one pin never
share a track here. A Z leaves its upper (or left) pin; a U leaves its row outward and its tracks reach to a step
inside the drawing area. A leg orders another wire only where it reaches the depths that wire's middle may take
(a leg runs from its pin to its track), so a U that rises past a channel from far below does not push its wires
out. The first track stays two steps from the row at the narrow pitch too, beyond the stubs of the pins. A track is
passed over, as blocked, where its route pays more for crossings and texts than the way the search would find for
the wire; the length and bends of the pitch are the track's to spend. A channel is too narrow when its assignment
at `TRACK_PITCH` leaves a wire without a track, counting room only. The placer's count of §4.5 is
`tracks_needed(&[(Tick, Tick)]) -> usize`: per stepping wire the coordinates along the rows of the pin it leaves and
the pin it reaches, counted by the same order and assignment at `TRACK_PITCH` with the rows as far apart as need
be. No mate or bundle price was left in the search to delete. The search lets a wire leave its pin over the stub of
a free connection point standing on the same point (a terminal's second point on one edge) on both nodes of that stub;
it allowed the full step only, and such wires had no way.

**Rings.** A wire between two pins, one on a body, whose other end lies behind that pin's edge (beside the
body and facing the way the wire comes, or beyond the body's far edge and facing it) has to go round one end of the
body: out of its pin, along the edge, beside the end, and on. The search lays such wires one by one, so those round
one end of one body run at whatever distance each found free and cross each other where they need not. After the
search (§3.6), the wires it laid that go round one end of one body, two or more, are laid again together as **nested
rings**: ring *r* runs *r* steps off the body (its lines off the edges rounded outward to the grid) along the pin's
edge, beside the end and, where the other end lies beyond the far edge, under it; the wire whose pin stands nearest
the corner takes ring 1. Each wire goes round the end whose way passes the fewest other wired pins of its edge, then
the shorter way, so the wires of a card in the older form part at its middle and nest on both sides; a pair whose
order on the far side runs against its order on the pin's edge still crosses once, as the strip's order forces
(§4.8). A wire whose ring is not clear keeps the search's route, and the rings are kept only where the page then
costs no more: a ring that would stand in another wire's way is not drawn. Every corner of a ring is on the grid; a
pin off it leaves its wire to the search. `ew_drafting::rounds` and `route`'s phase after the search.

*The placer's part.* A part wired round a body is the body's hairpin follower, and Arrange sets it beside the body
where it may (§4.4), so its wire goes round nothing. Where it stays behind the body (the person kept it in place, or
the body has no room beside it), the rings need their depth and a step between the body and it : after the
trees are set, a unit whose wires leave its pins on one side and go round its body to parts that do not move, beyond
its far edge, under it and facing it, steps away from them along that side, with what hangs on it, until the rings
round each end of the body (one step each; a wire goes round the end whose way passes the fewest other wired pins of
its edge, then the nearer) and a step clear of those parts' stubs fit between. Where it would then leave the drawing
area or stand on a unit, it stays. The judge takes the step as any other (§4.7): a page whose wires found no way round
gains them; one where the search found a longer way without a crossing keeps it, since the rings of a strip in the
order of the pins cross (§4.8).

### 3.6 The search

The exact search of today's router is kept and moved: a backward Dijkstra over (node, arriving heading) and a
forward walk that takes the lowest-ranked move on a least path (right, down, left, up), so that the result is the
same for the same page whatever the order of its lists. It reads the occupancy of §3.3 and prices a move by the
cost of §1.2:

| Move | Price |
|---|---|
| a step | 1 (2 on a half-grid track) |
| a turn | 4 |
| across another wire, at a right angle, through the inside of a segment | 6 more |
| onto a text that cannot move | 12 more; each further step on it 1 more |
| onto a label that has another free place | `LABEL_RANK` more (the label will step aside) |

Not taken at all: a move into a body grown by one step (but straight out of the wire's own pin, through its own
part's body and texts); onto a pin, a pin's stub or a junction that is not the wire's; along another wire (but a
shared run); onto another wire's vertex; a turn back; a move that would end a wire on a crossing. (The Wire tool's
straight line between two ends in line passes a stub and the step round a body, §7.4.)

Areas: the box of the two ends grown by 4 steps, then by 16, then the drawing area; the first area with a route
wins. **The half grid** is searched last and only between close pins, as today: where the whole grid has no route
or one more than 8/5 of the city-block distance of the ends, the search runs on half steps (bodies grown by half a
step, a half-grid track only between the two ends), and its route is taken where the whole grid's is more than
8/5 of it or there is none.

A part of one pin that is set on the end's own point (a potential's mark on a pin of a box) is left as an end
leaves its own part: the mark does not wall the pin in. Before, such a wire had no way and kept the route the
person drew.

*Never worse, and a wire without a route.* A plan that routes a wire that had no route is offered under the
exception of §1.2: a text across a wire routed before does not refuse it, a fault on such a wire does.

*Two ends nearly in line.* Where the search's route between two ends is straight on the lattice but the ends
are not on one line (a pin off the grid a part of a step beside it, as a grid change leaves a symbol whose own spacing
the new grid does not hold), the route steps across at the grid line nearest its middle; it was one slanted segment.

Order: the remaining jobs by the city-block distance of their ends, shortest first, then by the first end's
place, then by ID. Then at most `PASSES` passes in the same order: lift one wire, search it against all the
others, keep the new route when its cost is lower. A pass that changes nothing ends the passes.

### 3.7 Texts and routes together

1. Before any job is laid, the labels are placed against what stays (parts, fixed wires, fixed texts): the
   starting places.
2. The phases of §3.4 to §3.6 run. A straight wire, a trunk and a track do not ask the labels; the search prices
   them as in the table.
3. The labels are placed again against all wires. A label a wire now crosses moves to its next free place.
4. A wire that still crosses a text (the text had no free place, or is not the program's to move) is searched once
   more with that text at its full price, and its new route is kept when the page's cost is lower. Then the labels
   it touched are placed a last time.

Two rounds at most. Because the scene places labels with the same function from the same stored routes (§5), the
places the router reckoned with are the places the page then shows.

### 3.8 How it ends, and what it does when no way exists

Every phase is bounded: one pass over the straights, the nets and the channels; the search runs once per
remaining job per pass, in a bounded area; two passes; two text rounds. A loop that runs until nothing changes is bounded by what it can
change: the never-worse round of §1.2 by the nets plus one, the wire-by-wire weighing of §3.9 by `TAKE_BACK_ROUNDS` rounds.

A job with no route free of faults keeps its stored route when it has one, else stays unrouted and is drawn as the
dashed line. The plan names it with the first thing in its way, found by searching again without the other wires,
then without the pins of other wires: "other wires", "connection points", "parts" when neither search finds a way,
or "a fourth arm at a junction". Nothing is ever drawn through.

### 3.9 The plan, and the drawing it starts from

The plan holds the new junction places and routes, the labels' places for the preview, the notes of §3.4 and
§3.8, and `Quality` before and after. Before it is offered, each net is weighed against its stored drawing in the
finished page: a net whose new routes cost no less than its stored ones, and whose stored ones are free of faults
among the new routes of the others, keeps the stored ones. Then each wire is weighed alone: a wire whose stored
route still meets its ends takes it back where the page costs no more and the wire keeps free of faults, so a net
with one new wire (one that had no route) is not redrawn whole, and its new drawing's fault does not send the whole
net, the new wire with it, back to no route. A new wire that the routes kept leave with a fault is searched again
among them, and its new route kept where the page costs less. So a wire that already runs as well as the rules allow
is not redrawn, and the message can say honestly that nothing better was found. Then the never-worse rule of
§1.2 decides whether the plan is offered at all.

### 3.10 Marks follow their wires

A cable mark or wire marker sits across the wires it names. When one of those wires is routed anew, the mark is
set on the stretch nearest to its old place where its wires run side by side within two steps of each other,
across them; with no such stretch, on its first wire alone. Its tag is a label.

For §3.6 to §3.10: the search prices a label at `LABEL_RANK` (it is taken to step aside) and a fixed
text in full; the second text round prices in full the texts a wire still crosses. A route found on the grid ends on
its end's own point, and the corner next to it takes that point's coordinate (a pin off the grid). A bend the person
led into a part over one of its pins may leave that part (the router of ADR-0033 allowed it); a pin inside another
part's body has no way out. A job without a way keeps its stored route in the occupancy at once, so the jobs after
it keep clear of it, and each pass tries it again first. Never worse: a net that gives a wire a fault it did not
have, or a wire routed before a finding it did not have, goes back to its stored drawing (a wire without one to no
route), until none does; the text part of the rule (a label newly on a text or a part) holds where the plan routes no
wire that had no route, since routing a dashed wire is the larger gain. A page whose Route gains only a tie is
offered nothing. The weighing of §3.9 runs both ways: from the new drawing a net goes back to its stored routes
where they cost no less; from the stored drawing a net takes its new routes where they cost less; the cheaper page
stands, so one net's gain is not lost to a loss that no single net's going back undoes. A mark moves along its line
only (its line reaches across to its wires wherever it stands), by
`layout.apply`'s `markers`, which this step adds with `junctions` and `routes`; `parts` come with step 4. The plan's
`his` rows are listed in Route's preview card (§7.2); ticking them comes with the preview of §7.3.

*A mark is drawn from its wires.* The page draws a cable mark's line past the outer crossings of its cores,
with a tick and a core name at each crossing; a marker's text beyond its line's end; a wire's own marker at the middle
of the wire's longest segment. The layout draws it so as well (`ew_drafting::marks::draw_marks`): the scene's adapter
gives each mark its point and what it draws relative to the crossing or the end of its line it stands at, and the
measure and the placer's page after a plan draw it from their routes. So a plan is judged on the page that is stored,
and a second press of Route or of Arrange and route offers nothing. Before, a plan kept a mark where it stood while
the page drew it where the new routes crossed it: on the sample, after a strip's earth terminal was moved two steps
up, the first press moved a cable's core name with its wire and the second routed a wire again.

