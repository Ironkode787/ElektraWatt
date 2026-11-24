# Drafting: routing wires, arranging parts and settling texts on a schematic page: what a good page is, and the model of a page

A chapter of [Drafting: routing wires, arranging parts and settling texts on a schematic page](06a-drafting.md), whose table says where each section lives.

---

## Summary

1. **One measure.** A page's drawing is judged by one function: what is a fault (never drawn by the program,
   reported by a check), and what is counted and weighed (length 1 per grid step, a bend 4, a crossing 6, a wire
   across a text 12, a text on a text or on another part 24). The router, the placer, the text placer, the checks,
   `ew layout` and the tests all call it. Nothing else rates a drawing, with one difference in the text placer: its cost
   of a place counts a wire within `LABEL_CLEAR` (1/8 step) of a text as across it, where the measure counts a wire
   strictly inside the text.
2. **Two commands, and nothing by itself.** *Route* draws wires; parts stay. *Arrange and route* first puts the
   parts where the page is best for them, across the page where need be and within the rules of a drawing of its
   kind (the potentials at the head and the foot, current running down a circuit, a card's channels in their
   order: §1.6), then routes. Each works on the selection, or on the page when nothing is selected. No part is ever moved
   outside these two commands, and a part placed by hand stands where it is put.
3. **Pins decide.** The placer has no template per kind of page. It puts each part *in line with the pin it is
   wired to, on the side that pin faces*, starts from the largest part and works outward, keeps the order the
   person gave, and makes room where things touch. Rails, current paths, card channels and fluid chains come out of
   this rule and the trunk rule below. Every arrangement it weighs is routed and measured for real, and none is
   offered that Route alone beats (§4.7). The spacings (two steps between wires and between rails, five between the
   devices of a chain) and the places of texts are those of the pilot's reference set; a card's pins stand eight
   apart (§7.5).
4. **A net is routed as one thing.** Wires joined by junctions, or leaving one pin, are one job. A net with
   several taps gets a **trunk**: one straight line with its junctions on it and a straight drop to each pin.
   The junctions' wiring order is the person's and is never changed; their positions are the router's. A wire that
   ends on a wire is joined in the bevel form by default (§7.6).
5. **Structure before search.** Straight wires first, then trunks, then the wires that step sideways between two
   rows of pins (assigned to tracks so that they nest and do not cross), and only what is left goes to the exact
   grid search that the program already has. There is no negotiation and no comparing of routing orders.
6. **Texts yield, by candidates.** Every text has a short list of places. One text placer chooses among them
   against parts, wires and other texts, the same function at every drawing of a page and inside the router, so
   a wire is never led round a text that could have stepped aside. A text the person moved stays. The pin texts
   of a drawn box are never touched.
7. **The drawing is stored; texts are derived.** Routes, junction positions and part positions are stored and
   change only by a command. Text positions are computed from them, unless the person moved the text.
8. **His routes are his.** A route is marked when the program computed it. Only marked routes, unrouted wires and
   broken ones are routed again by a page-wide Route; a route the person shaped is replaced only when he selects
   the wire, or when it is broken and he ticks it in the preview.
9. **Preview and one Undo.** Arrange and route always shows the page as it would become, with a list of what
   moves; unticking a part plans the page again around it. Route applies at once and names the wires it changed,
   unless it would replace a route the person shaped. Either is one undo step.
10. **Guides, not snapping.** While a part hangs on the pointer, a guide shows where one of its pins is in line
    with a pin it would face. The part takes that line when the pointer is within three quarters of a grid step
    of it (the ordinary snap is half a step), at most 8 screen points further, and lets go beyond. Nothing is
    proposed or moved by the placer here.

What goes: the page solve of `ew_derive::routing` with its order comparisons, rip-up rounds, mates and the 8/5
rule; the scene's nine text-moving steps; the actions Route page and Reroute; the test that holds the reference
project's routes. What stays, moved: the exact search of one route, with its tie rule.

The format gains two optional members and no format step (§2.5). A new crate `ew-drafting` holds everything in
this document that computes.

---

## 1. What a good page is

### 1.1 Terms

- **Step:** one step of the page's connection grid (4 mm on a native page). **Half step:** half of it.
- **Part:** a representation on the page: a symbol, a drawn box, a terminal, an interruption point or supply
  mark. **Pin:** a connection point of a part, with the **side** a wire leaves it on (up, down, left, right).
- **Wire:** a connection drawn on the page; tubes and bus lines are wires here. **Net:** the wires of a page that
  are joined by junctions or end at one pin.
- **Trunk:** the straight line on which a net's taps lie. **Tap:** a junction, or the point where two wires of one
  pin part. **Drop:** the wire from a tap to a pin.
- **Label:** a text that belongs to a part or a wire and may be placed by the program.
- **Fault, cost:** §1.2.

### 1.2 The measure

`measure(page) -> Quality` is the one place where a drawing is judged.

**Faults.** The program never draws one; a stored drawing that has one is reported by a check. A wire that cannot
be routed without a fault stays unrouted (a dashed line, as today).

| # | Fault | Rule | Check |
|---|---|---|---|
| F1 | A segment that is not horizontal or vertical; a vertex off the half grid | D-1, D-2 | refused by the command that stores a route |
| F2 | A wire through a pin or a junction it does not end at | R-9, R-14 | EW-CORE-067 |
| F3 | A wire on another wire, except along a common beginning at a pin both end at (§3.4) | R-14 | EW-CORE-068 |
| F4 | A wire through the body of a part, except straight out of its own pin | D-3 | EW-CORE-069 |
| F5 | A junction with four or more arms | D-7 | EW-CORE-072 |
| F6 | A false joint: a corner on another wire's corner or end, a junction on a crossing, a wire end on a wire without a junction | D-7, R-14 | new check "Wires meet without joining" |
| F7 | A part on another part, or outside the drawing area | — | new check "Part on a part" (same rule file) |

**Unrouted.** The number of wires without a route (EW-CORE-073).

**Cost.** One number, in grid steps of wire, summed over the page.

| # | What | Weight | Why this weight |
|---|---|---|---|
| C1 | Wire length | 1 per step | the unit |
| C2 | A bend | 4 | a wire takes four steps more to save a bend (today's ratio, kept) |
| C3 | A crossing of two wires | 6 | crossings cost a reader most (R-1); a wire detours up to six steps, but takes no two bends, to avoid one (today's ratio, kept) |
| C4 | A wire across a text | 12, and 1 for each step on it | a way round with a bend or two and a few steps is taken; a way round a group of parts is not (ADR-0033 amendments 2 and 5, now one number) |
| C5 | A label on another text, on another part's body, or outside the drawing area | 24 each | worse than a wire across a text: two texts on each other cannot be read |
| C6 | A label not at its first place | 1 per place down its list | keeps a label at home when nothing is in its way |
| C7 | A step on the half grid | 1 more per step | the half grid is for the stretch between close pins only (§3.6) |
| C8 | A mark's wire that jogs to its trunk: a wire from a pin of an interruption point or supply mark to a tap (a junction, or a pin another wire ends at too) that bends, where the net goes on from the tap along the mark's axis, so the trunk runs beside the mark's line, off it | 32 | a wire into a mark stays on the mark's line (owner, 2026-11-22): on the sample's PLC page 104 Route's plan laid the 0 V trunk off its mark's line for some 28 steps less, so the jog costs more than that |

**Compared** in this order: fewer faults, then fewer unrouted wires, then lower cost. `Quality` is that triple
with the counts behind it.

*C8's tests.* `ew-drafting`'s `c8_a_marks_wire_that_jogs_off_its_line_to_its_trunk` prices a trunk two steps off its
mark's line at two steps, a bend and the jog more than the same trunk on the line, and a mark wired round a corner into
a pin no other wire ends at at no jog; `ew-api`'s `arrange_keeps_a_marks_straight_wire_on_its_line` runs Arrange and
route on the sample's 104, whose marks' wires run straight, and finds each of them straight in the plan.

**Never worse.** A plan (§3.9, §4.9) is offered only when the page's `Quality` after it is better than before,
and when no wire routed before and no text that was free of a finding (F2 to F6, C4, C5) gets one. The second half
is what a person sees as "a check's warning more": a drawing with nine crossings fewer and one wire newly across a
text is not offered. **The exception:** a wire without a route is the worst state a wire has, so a plan that routes
more wires than stood routed before is offered although a text then lies across a wire routed before (C4) or a text
gets a finding (C5); only a new fault (F2 to F6) on a wire routed before refuses it. The plan's quality names the
wire.

**Reported, not weighed.** `ew layout` and the preview print four more figures, because they say what a reader
sees and make a page comparable before and after: the wires drawn **straight** against all wires; the
**turn-backs** (three segments in a row of which the third runs against the first: the loop round a caption);
the **detour** (the summed length against the summed city-block distance of the ends, in percent); and the
**labels moved** off their first place. Their causes are already in the cost.

F2 excepts a pin or junction that stands where one of the wire's own ends stands (the clamps of a terminal drawn at
one point). F3 excepts the common beginning of two wires of one pin: their routes walked from the
pin together while they run the same way (the shared run of §3.4). F6 is a corner on another wire's corner: a corner or
an end on another wire's end, a junction on a crossing and a wire's end on a wire are F2 by their geometry. C3 counts
each point where two wires meet that is an end of neither, lies on no stretch they share and is no F6. F7
leaves out an outline drawn round other parts, a part without pins and two parts on one pin's point (an
interruption point on a pin). Two texts meet when they overlap by more than a tick (a text's box is rounded outward
to ticks); C4 does not count a wire across a text of a cable mark or of the wire itself. The checks name parts as
the program does, "symbols": F7 is EW-CORE-077 "Symbol on a symbol", and C5's text on a part is EW-CORE-076 "Text
on a symbol".

### 1.3 Rules that hold on every page

Source: a rule number of the research, or **E** for a rule measured on the pilot's reference set and taken as this program's default (on a sheet of
102 by 64 grid steps, the frame of 17 columns by 8 rows).

| Rule | Number | Source |
|---|---|---|
| A wire leaves a pin straight, along the pin's side | at least 1 step | D-18, R-9 |
| A wire keeps clear of a part it does not end at | at least 1 step from its body | R-9 |
| Two wires side by side | **2 steps apart** where there is room (a wire number fits between), never less than 1; half a step only between close pins (§3.6) | E; D-6 |
| A wire between two pins that face each other on one grid line with nothing between | one straight segment, always | R-3, E |
| A typical wire | none or one bend; two where pins of two devices have another pitch; never a turn back where a straight or an L exists | E, R-3 |
| A potential that serves several devices | one straight trunk; every device taps it by a straight drop | E, D-10 |
| Trunks side by side | 2 steps apart; the nearest one 2 steps from the pins it taps | E |
| The order of trunks over a row of poles | the trunk of the leftmost tapped pole lies nearest the poles, the next pole's one further out, so that each drop crosses only the trunks of the poles to its left. With the usual pole order L1 lies nearest and L3 outermost; the textbook order (L1 outermost, D-10) is not taken | E against D-10 |
| A tap of two wires on one pin | the arriving wire turns into the drop; the wire that goes on leaves the drop half a step from the corner by a bevel; no dot | E, D-8 |
| A junction of three wires | on the whole grid, three arms, a dot (or the bevel form the person chose), never on a crossing | D-7 |
| A trunk's end | at its last tap, by a corner, or at a mark at the sheet's edge; never a loose end | E |
| An incoming potential's mark | at the left of the sheet; an outgoing one at the right; marks of one stack 2 steps apart | E |
| Parts of one chain | in line, their facing pins 5 steps apart or more (a turned wire number fits), never less than 2 | E |
| Protective earth | not a trunk: each device's earth wire goes by a short hook to an earth symbol beside it, so it crosses no path | E, research §2.3 |
| A label | within one step of what it names, nearer to it than to anything else of its kind, at 0° or 90° | D-16, D-17, R-11 |
| A label and anything it does not belong to | at least 1/8 step apart (0.5 mm on a native page) | R-11 |
| Where each kind of label stands | §5.2: the tag at the upper left of its symbol, make and type under it, pin numbers right of the pins, wire numbers along the wire | E against the "left, centred" of D-17 |

### 1.4 What the rules give per kind of page

The placer and the router have no setting for the kind of page. The kinds below are what §3 and §4 produce from
the pins, with the numbers that apply; they are also the fixtures of §9. A page that is none of these is drawn by
the same rules.

| Kind | What stands where | Trunks | Numbers |
|---|---|---|---|
| **Supply and power** | Each feeder is one column: protection, switching, overload, terminals, cable mark, load, each in line with the pole it is wired to, poles 2 steps apart, the earth wire at the right of the phases. Where two devices have another pole pitch, one pole is in line and the others step once, nested. | The phases and N run across the head of the sheet from their marks at the left to their marks at the right, 2 steps apart; the first device of each feeder hangs 2 steps under the nearest. A drop crosses the trunks between it and its own without a dot. | Chain gap 5 steps; trunk 2 steps from the pins it taps; trunks 2 steps apart |
| **Control circuit in current paths** | Each path is a column between the two trunks; the contacts of all paths on one line, the coils (or lamps) on one line; a parallel branch in its own column beside the path. The contact list of a coil stands left of the coil under its tag; the path's function text under the lower trunk. | The upper potential along the head, the lower one along the foot, each from a mark at the left; a trunk ends at its last path by a corner. | Paths clear of each other by their footprints and `ROW_GAP`; the last parts' lower pins on one line when within 6 steps of it |
| **PLC inputs** | The card is a row of channel pins along the foot of the sheet, pins up. Signals from other pages arrive from marks, each mark 2 steps before its drop and 2 steps higher than the one before (a staircase), so the drops do not cross. Devices on the same page stand in line above their terminal and pin. | The devices' common potentials run as trunks between the devices and the card, or beyond the devices. | Card pins 8 steps apart (§7.5): a device with its tag and address stands over each pin |
| **PLC outputs** | The mirror: the card along the head of the sheet, pins down; each load in line under its pin; the loads on one line near the foot; tags and contact lists under the lower trunk, in two rows where the pitch is tighter than a tag is wide. | The loads' return potential is one trunk under the loads, from mark to mark. | Card pins 8 steps apart |
| **Bus layout** | Stations with their ports on the same side stand level; a link between neighbours runs on the nearest free track beyond the ports and the captions it has to pass; a station's make, type, tag and address stand above its box. | A line bus with three or more stations is a trunk. | Track at least 2 steps from the ports |
| **Pneumatic circuit** | The valve terminal or the valves are the anchor. Their working ports face up, so flow controls and actuators stand above, in the order of the ports; the supply port faces down, so the service unit and the source stand below. No part is turned. (From ISO 1219-2 as the research reads it; the reference set has no page of this kind.) | The supply line of single valves is a trunk below them. | Tubes are wires; the fluid grid's step applies |
| **Distribution, and pages that are none of these** | The largest part of each group is its anchor; marks stack beside the pins they are wired to, incoming at the left and outgoing at the right, 2 steps apart, in the order of the pins, so that their wires nest. | As the nets ask. | Marks 2 steps apart |

**What a draughtsman does and the program does not.** He turns a part so that a page flows one way and every
wire is one straight line; he spreads a circuit over the height of the sheet; he writes headings over groups. The
placer turns no part, keeps the spacing along a circuit as the person left it, and writes no text. What it does
take over from that hand: lines, rows, nesting, the staircase of marks, two rows of tags where one does not fit,
wire numbers level across a bundle, and the order of the circuits across the sheet.

The numbers of practice (trunk pitch, chain gap, card pin pitch) are fixed constants here (§1.5), not settings:
an alpha with one pilot does not need a settings page for them, and a setting can be added when a second user
asks.

### 1.5 The numbers, in one place

One `const` table in `ew-drafting` holds these; nothing else in the program repeats them.

| Name | Value | Used by |
|---|---|---|
| `TICKS_PER_STEP` | 16 | the unit of the model (§2.1) |
| `HALF` | 8 ticks (half a step) | the half grid (§2.1, §3.3) |
| `STEP`, `BEND`, `CROSSING` | 1, 4, 6 | the cost |
| `TEXT_CROSSED`, `TEXT_STEP` | 12, 1 | the cost |
| `LABEL_CLASH`, `LABEL_RANK` | 24, 1 | the cost |
| `HALF_STEP_EXTRA` | 1 | the cost |
| `MARK_JOG` | 32 | the cost |
| `BODY_GAP`, `STUB` | 1 step each | router: the least distances |
| `TRACK_PITCH` | 2 steps (1 where a channel is too narrow) | tracks, parallel wires |
| `CHAIN_GAP` | 5 steps (never less than 2) | placer: between facing pins of two parts in line |
| `TRUNK_GAP`, `TRUNK_PITCH` | 2 steps each | router: a trunk from the pins it taps; trunks side by side |
| `TRUNK_LINES` | 8 | router: the candidate trunk lines beyond the pins, each way (§3.4) |
| `LEFT_TO_SEARCH` | 8 steps | router: what a wire left to the search adds to its ends' distance where trunks are compared: two bends' worth, written apart from `BEND` (§3.4) |
| `MARK_PITCH`, `MARK_LEAD` | 2 steps each | placer: marks in a stack; a mark before its drop |
| `ROW_GAP` | 1 step | placer: between two footprints in a row |
| `ROW_PULL` | 6 steps | placer: a part joins a common line when within this |
| `MOVE_PRICE` | 2 per part | placer: what a move has to earn on the routed page (§4.7) |
| `REACH` | 24 steps | placer: how far a tree's root may step, each way, to find its tree room (§4.7) |
| `ANCHORS` | 6 | placer: the most units of a tree tried as its anchor (§4.2) |
| `PIN_WEIGHT`, `AREA_PER_WEIGHT` | 4; 16 square steps | placer: a unit's weight per wired pin; its bodies' area per weight (§4.2) |
| `ANCHOR_SHARE` | 3/4 | placer: an anchor's least share of its tree's heaviest weight (§4.2) |
| `MATE_REACH` | 12 steps (twice `ROW_PULL`) | placer: the farthest mate (§4.2) |
| `MARK_PITCHES` | 2, 3, 4 steps | placer: the pitches a stack of marks tries (§4.4) |
| `RESETS` | 2 | placer: the most times the trees are set again after their first setting (§4.4) |
| `TRIAL_STUB` | 2 steps | placer: a wire out of a pin on the page of a few parts the text placer is asked about (§4.5) |
| `BRANCHES`, `ROUNDS` | 8; 4 | placer: the most trees, and branches, whose plans are routed one by one; the most runs of one press (§4.7) |
| `EARTH_REACH` | 8 steps | placer: an earth symbol from the pin it earths |
| `LABEL_CLEAR` | 2 ticks (1/8 step) | text placer |
| `NUMBER_ROOM`, `NUMBER_LEVEL` | 2 steps; 4 steps (twice `TRACK_PITCH`) | text placer: a stretch long enough for a wire number, beyond its length; a parallel wire's number it stands level with (§5.2) |
| `NUMBER_AT` | 1/2, 1/4, 3/4 | text placer: the places along a stretch a wire number tries after the level one (§5.2) |
| `LINE_GAP` | 1/4 step | text placer: the most between the lines of a block of texts (§5.2) |
| `WINDOWS` | 4 steps, 16 steps, the drawing area | search areas round the two ends |
| `PASSES` | 2 | router: improvement passes (§3.6) |
| `TEXT_ROUNDS`, `TAKE_BACK_ROUNDS` | 2 each | router: text rounds (§3.7); the rounds in which a job takes its stored route back (§3.9) |
| `DETOUR_MOST` | 8/5 | search: the longest grid route before the half grid is taken (§3.6) |
| `MAX_NODES` | 262 144 | search area bound (kept) |
| `GUIDE_SHOW`, `GUIDE_CATCH`, `GUIDE_CATCH_PT` | 2 steps, 3/4 step, 8 screen points | guides (§6) |
| `CARD_PIN_PITCH` | 8 steps | cards drawn from the station view (§7.5) |
| `CARD_HEIGHT`, `CARD_PINS_MOST` | 6 steps; 8 pins | cards drawn from the station view (§7.5) |
| `MARKER_REACH`, `MARKER_GAP` | 1.5 mm; 1 mm | the scene: a wire marker's line past its outer wires; its text beyond the line |
| `CABLE_TICK`, `CABLE_TEXT_GAP` | 1 mm; 0.5 mm | the scene: half a cable mark's stroke on a core; its texts beside stroke and line |
| `CABLE_MARK_ROOM`, `SUPPLY_GROUP_REACH` | 6 mm; 20 mm | the commands: the stretch a cable's mark placed with its cores prefers; how far apart two supply points of one group stand in a column |

A mark's sizes and the commands' two distances are in nm, not steps: they stand the same on every grid. Where the
mark's line and its wires cross, and where a wire's own marker stands, is one function each over the point's type
(`ew_drafting::marks::{crossing, middle}`), which the layout calls in ticks and the scene in nm.

### 1.6 The rules that bind the placer

Arrange and route may move a part anywhere on its page. Two things hold it: the **rules** of this section, which
no plan breaks, and the **measure**, which decides everything the rules leave open. A plan sets every part it
moves by the rules; a part that stands against a rule is set right where the sheet has room, and left and named
where it has none.

| | Rule (never broken by a plan) |
|---|---|
| **Every page** | A1. Every part inside the drawing area; no part on a part. A2. A part that is kept in place, outside the scope or inside a structure box does not move. A3. No part is turned or mirrored, and none leaves its page. A4. The parts of a row stand in the order of the pins they are wired to; the parts of a circuit in the order of the wiring. The terminals of a strip are one row. A5. The spacing along a circuit is the person's: it is neither closed up nor spread; a part that has to move takes the chain gap, and one that joins its row's line may stand up to `ROW_PULL` further. |
| **Electrical pages** (energy and signals flow down, potentials from left to right) | E1. Current runs down a circuit: of two parts wired through a pin that faces down and a pin that faces up, the first stands above the second. E2. Potentials at the head and the foot: a trunk that feeds lies above every part of the circuits it feeds, a trunk that returns below every part that returns to it, and no part but a mark stands above the highest feeding trunk or below the lowest returning one (the bands). E3. A potential or signal enters at the left and leaves at the right: an incoming mark stands at the left end of its wire or trunk, an outgoing mark at the right end. E4. A card's channels stand in channel order, each channel's parts in the channel's column on the side its pin faces, and no part of another circuit between a pin and its channel's parts. A part wired to a card's pin by a pin of its own that faces the same way (a hairpin) has no place in the column: it stands at the card (§4.3). E5. An earth symbol stands within `EARTH_REACH` of the pin it earths. |
| **Fluid pages** (energy flows up) | F1. The pressure source and the service unit stand lower than every valve, and every actuator higher than the valve that drives it. F2. The actuators keep their order from left to right (the order of operation is the person's). |
| **Bus pages** | B1. The stations of a line stand in the order of the line, the controller first at the left. B2. Stations with their ports on the same side stand level. |

Weighed by the measure, and so free to change where the page gains by it: the order of independent circuits
across the sheet and the place each takes; on which side of its host a part stands when both sides keep the
rules; whether a mark stands at the sheet's edge or near its drop; the track of each trunk within its band; the
place of a part that has no line to take.

The rules are checked on a plan by one function, `rules_kept(before, after)`, beside the measure; the placer's
tests use it on every plan.

`ew_drafting::rules`: each rule a function over the page that names the parts breaking it;
`rules_kept` holds when every rule broken after the plan was broken before by the same parts and none of them moved.
The rules read the parts and their nets: a **pair** is a net of two pins on two parts, a **rail** a net with two
junctions or more on one horizontal line whose pins (but marks') all face it from one side. E1 reads pairs from a
pin facing down to one facing up; E2 the circuits of a rail (the parts reached from its pins through pairs) and the
band above the highest feeding rail and below the lowest returning one; E3 a mark against every pin and junction of
its net; E4 a card's pairs (beyond its pin, in its channels' order; "no part of another circuit between" is not
checked); E5 the nearest other pin of the earth symbol's net, city-block; F1 a source and service unit below every
valve, an actuator above every valve it is reached from through parts that are neither; B1 a line read from its
controller, else from its leftmost station. A4 reads facing pairs only (wires that turn a corner nest, and wires
that turn back lie inside one another: for both the order of the parts is the other way round), per line of pins
that face one side: the pins of one part, or of parts of one size whose pins stand on one line (a strip). A5 judges
a pair of facing pins whose second stands at least two steps beyond the first, and only a pair whose parts stood
against no rule (one that had to move): it is closed up when it stands nearer than it
stood and nearer than the chain gap, and spread when it stands further than it stood, or than the chain gap, by more
than `ROW_PULL`. E4 does not read a pair whose two pins face the same way. So A4 allows the nesting of a stack and
the order of wires that turn back (§4.4, §1.4), and A5 lets a part that stood nearer than the chain gap take it, a
row take one line and a chain whose host moved keep its distance.

---

## 2. The model of a page for layout

### 2.1 Units: grid ticks, never nanometres

The layout computes in **ticks**: 1/16 of a grid step, as `i32`. A grid position is a multiple of 16, a half-grid
position a multiple of 8, and a text box is rounded outward to whole ticks (0.25 mm on a native page). The adapter
in `ew-api` converts the page into ticks once and a plan back into nanometres once, through `ew_units::Grid`.
Inside the crate no nanometre exists, so two numbers for one grid position cannot meet.

**A box in ticks.** On the QET grid, whose step is no whole number of nanometres, a grid point is up to half a
nanometre off its own line and a symbol's offset is rounded to a whole nanometre, so an edge set on a tick of its
symbol lies up to about a nanometre beside that tick, on one side or the other according to where the symbol
stands. Rounded outward as it is, a text's box was one tick larger or smaller by its symbol's place, the page as
stored after a plan moved symbols was not quite the page the plan was made for, and a second press could route a
few wires again. So an edge within two nanometres of a tick takes that tick, and only an edge further from every
tick is rounded outward (`Grid::tick_span`): a box keeps its ticks wherever its symbol stands. On a grid whose tick
is a whole number of nanometres (the native 4 mm grid) nothing is rounded and every edge is rounded outward. The
scene's one conversion (`ew_scene::ticks::Ticks`) applies the rule to every box the layout sees: the texts, the
symbols' bodies, the frame's bands, the drawing area (rounded inward by the same rule), a structure box's fence and
the body of a part floating on the pointer (§6).

**What the design needs from the position model.** On a page whose step is not a whole number of nanometres, a
pin's stored point is a sum of two rounded numbers and can lie 1 nm beside the grid's number for that position.
The design needs two things:

1. *At the adapter:* a point within 1 nm of a half-grid position converts to that position's tick; any other point
   converts to its nearest tick and is flagged **off grid**. Back from ticks, a position on the grid is the grid's
   own number (`Grid::at`); an end at an off-grid pin takes the pin's stored point, and the corner next to it
   takes that point's coordinate on the axis of the segment between them (today's `fit`).
2. *At the source:* the functions that give a pin's point, a box pin's point and a junction's
   point return the grid's own number when the point is within 1 nm of a grid position. Routes stored by an older
   build keep their numbers: a reader joins an end that is 1 nm beside its pin with a segment of 1 nm, which
   draws as nothing and which the layout never sees. Comparisons of points then compare equal numbers.

`Grid::settle_point` gives a point within 1 nm of a grid position the grid's own number; the
functions that give a pin's and a box pin's point return it: ew-derive's `cp_end` (and `rep_cp_end`, which takes the
page's grid), the commands' `rep_cps`, the drawn connection points of the scene and of the page tables, the two
importers' (tables and QElectroTech), and the tools' ghost pins (placing, the interruption point, a moved symbol's
proposals). A move (by steps, by a script's `delta`) and the Select tool's re-solve move a wire's end by its pin's own
displacement, which can differ from its symbol's by 1 nm. A position 1 nm beside a grid point is still off the grid to
the commands (its pins' points are on it, but it is no fitted symbol). `Grid::settle` is the rule `settle_point`
applies.

A stored route and a junction are read by the same rule, so that a drawing of an older build
(an earlier build's projects, every import) compares with its pins by equal numbers: `ew_derive::read_connection`
(each vertex by `settle_point`, equal neighbours merged) and `ew_derive::read_junction`. They are applied where a
record enters the working model: the commands' `TxBuilder::row` and `base_row` (every read of a connection or a
junction in a command or a normaliser), the page's connection and junction views of the scene (and so the layout's
page), the page tables of the tools, `drawn_path` (the checks) and the clipboard's copy. A record keeps its numbers
until a command writes it, and is then written as read: opening and saving changes nothing, and a wire a command
touches is stored on the grid's numbers, a change of that wire only. Before, a junction's point was compared exactly
with a route's end (`require_route`), a split point and the tables' wire ends, and read settled against a terminal's
pin by EW-TERM-006; now all of them read the one number.

### 2.2 What the layout sees

```rust
pub struct DraftPage {
    pub kind: PageKind,             // Electrical | Fluid | Bus: which rules of §1.6 apply
    pub area: Rect,                 // the drawing area
    pub keep_out: Vec<Rect>,        // the frame's bands, the title block, a strip's drawn bridges
    pub parts: Vec<Part>,
    pub junctions: Vec<Junction>,
    pub wires: Vec<Wire>,
    pub labels: Vec<Label>,         // texts the program may place
    pub fixed_texts: Vec<FixedText>,// texts it may not: pin texts of drawn boxes, free texts, captions of
                                    // structure boxes, a text the person moved
    pub marks: Vec<Mark>,           // cable and wire markers that sit across wires
}
pub struct Part {
    pub id: PartId,
    pub at: Pt,                     // its origin on the page
    pub body: Rect,                 // relative to `at`: the bounds of its graphics, a box's outline
    pub frame_only: bool,           // an outline drawn round other parts (a valve terminal round its valves)
    pub pins: Vec<Pin>,
    pub unit: u32,                  // parts with one number move as one (§4.2)
    pub role: Role,                 // MarkIn | MarkOut | Earth | Card | Source | Valve | Actuator | Station |
                                    // Controller | Other: what the rules of §1.6 ask about a part
    pub locked: bool,               // "keep in place" (§2.4), or outside the command's scope
    pub off_grid: bool,
}
pub struct Pin { pub id: PinId, pub at: Pt /* relative */, pub side: Option<Side>, pub off_grid: bool }
pub struct Junction { pub id: JunctionId, pub at: Pt }
pub struct Wire {
    pub id: WireId,
    pub ends: [End; 2],             // End::Pin(PinId) | End::Junction(JunctionId)
    pub route: Vec<Pt>,             // every vertex, both ends included; empty: unrouted
    pub computed: bool,             // the stored route was computed by the program (§2.4)
    pub class: WireClass,           // Wire | Tube | Bus
    pub supply: Option<u8>,         // rank in L1, L2, L3, L, N, PEN, PE, FE, L+, M, L-; None for a signal
}
pub struct Label {
    pub id: LabelId,
    pub owner: Owner,               // Part(PartId) | Wire(WireId) | Mark(MarkId)
    pub kind: LabelKind,            // §5.2
    pub size: (i32, i32),           // of the whole block of lines, ticks
    pub places: Places,             // At(Vec<Pt>) relative to the owner's origin, in order of preference,
                                    // or AlongWire: derived from the owner's route (§5.2)
}
```

What each thing of the page becomes:

| On the page | In the model |
|---|---|
| A symbol | a part; its pins with the sides the symbol declares, turned with it |
| A drawn box (black box, PLC card, station) | a part whose body is its outline; its pin texts are fixed texts inside it |
| A valve terminal with valves in its cells | parts of one unit; the terminal's part is `frame_only` |
| A terminal of a strip | a part of its own (a strip is not one block: its terminals are spaced by the person) |
| An interruption point, a supply mark, an external point | a part with one pin; its form gives the side |
| A potential, a rail | nothing of its own: a net with taps has a trunk (§3.4) |
| A junction | a junction; its style, wiring and sequence are not the layout's and are not in the model |
| A wire, a cable core, a tube, a bus line | a wire |
| A cable mark, a wire marker | a mark across the wires it names; its tag is a label |
| Tag, captions, pin texts of a symbol, cross-references, contact image, wire number, a mark's tag, a point's name | labels (§5.2) |
| Pin texts of a drawn box, free texts, captions of structure boxes, plug pin notes | fixed texts |
| A structure box | its outline is a fence: a part inside stays inside, a part outside stays outside; in the first build the parts inside a structure box are locked for the placer |
| The frame | `area` and `keep_out` |
| A strip's drawn bridge (doc 08 §6.5) | a `keep_out` of its line and a tick to either side: no text stands on it, and the router does not read it |
| The page's type | `kind`: a fluid page, a bus page, every other schematic page electrical |
| What a part is, for the rules | `role`: from the function's kind (an interruption point's direction, a card, a station), from the potential of an earth symbol's one pin, and on fluid pages from the roles of its ports; a part the adapter cannot tell is `Other` and is bound by the rules of every page only |

### 2.3 What is fixed and what may move

| | Route | Arrange and route |
|---|---|---|
| A part | never | within the scope, unless locked: anywhere on its page, by whole steps, within the rules of §1.6; never turned, mirrored, reshaped or moved to another page |
| A pin of a box, a box's size | never | never |
| A junction's position | when every wire at it is a job | the same |
| A junction's wires, form (dot or bevel), wiring and sequence; which pin a wire ends at | never | never |
| The way a bevel junction points | set to the routes that meet at it | the same |
| A wire's route | when the wire is a job (§3.2) | the same, and every wire with an end on a moved part |
| A label | by the text placer, always (§5) | the same |
| A text the person moved; a fixed text | never | never (it moves with its part) |
| A mark across wires | follows the wires it names (§3.10) | the same |

The wiring order is data: a net's junction tree, each junction's sequence and the order of the wires on a pin say
which wires the shop cuts. The layout changes none of it. As a guard, the command that stores a plan (§2.6) takes
no member that could.

### 2.4 How a person says "leave this alone"

- **A part:** *Keep in place* (context menu and Properties; Dutch: *Op zijn plaats houden*). It sets
  `Representation.locked`. The placer never moves a locked part and arranges the others around it; moving it by
  hand is unaffected. A small pin glyph shows on a locked part while it is selected and in the preview. The same
  word and meaning as `Placement.locked` on panel pages.
- **For one run:** the selection (only selected parts may move), and unticking a part in the preview (§7.3).
- **A wire:** nothing to do. A route is the person's unless it is marked `computed`. The Wire tool marks a route
  it found between two clicked ends; it does not mark one with a bend the person fixed by a click. Dragging a
  segment or a bend clears the mark. Route and Arrange and route mark what they write.
- **A text:** moving it. The stored slot override (as today) holds it; *Reset text positions* gives it back to
  the program.

No command of its own: Properties' row and the context menu's entry (`rep.keep_in_place`, a
check that keeps the selected symbols, or lets them go when all are kept, with a toast that names them) both run
`fields.set` of `locked`, one undo step for the selection. The pin is `IconId::pin` at the top-right corner of the
symbol's body while it is selected, painted with the panel pages' lock (`paint_locks`); the page tables carry
`TableRep.locked`, the adapter `Part.locked`. The glyph in the preview's "Left as it is" (§7.3) is step 4's, from
`Part.locked`.

### 2.5 The file format

Two optional members, appended after the released members of their records, written only when true:

| Member | Meaning | An older file | A new file in a 0.2.0 build |
|---|---|---|---|
| `Connection.computed: bool` | the stored route was computed by the program and may be computed again | absent: every stored route counts as the person's, so a page-wide Route touches only unrouted and broken wires until wires are routed anew | kept in `extra` and written back; the drawing is the same; a route edited there keeps a stale mark, so a later page-wide Route may replace it (it is shown and named, and undone in one step) |
| `Representation.locked: bool` | the placer leaves the part where it stands | absent: nothing is locked | kept and written back; that build has no placer |

Both are additive by ADR-0011 amendment 6 (an optional member without cascade semantics): no format step, the
published schemas are re-blessed, and the test that a released project opens and saves unchanged holds. Text
positions need no member (§5.5), nor do trunks or the kind of a page.

### 2.6 One command stores a plan

`layout.apply { page, parts: [{rep, steps: [kx, ky]}], junctions: [{junction, at, orient?}], routes: [{connection,
route}], markers: [{item, at}] }`: one transaction, one undo step. It checks each route as `connection.route`
does (both ends on their pins or junctions, after the moves of the same command), marks each stored route
`computed`, refuses a move of a locked part and a part onto another, sets `orient` only on a junction of the bevel form, and
takes nothing else: it cannot change an end, a junction's form, its wires or its sequence. Commands store and never compute (kept): replaying a log runs no router.
`connection.route_many` goes; `rep.move` keeps its `routes` for the move's own re-solve (§7.4).

`parts: [{rep, steps: [kx, ky]}]` moves each symbol as `rep.move`'s `steps` does (one function
for both). The command refuses a symbol kept in place, one named twice, one whose connection points or anchor would
leave the drawing area of a page with a frame grid (the cell lookup the command layer has), one whose body (its
symbol's graphics, a drawn box's outline) would overlap another's by a grid step or more each way where it did not
before, and a moved symbol whose routed wires (at its connection points) get no route in the same plan. A plan gives
every wire whose route it changes, an empty route for a wire of a moved symbol that has no way.

### 2.7 Where the code lives

A new crate **`ew-drafting`** (GPL-3.0-or-later; of the workspace it depends on `ew-ids` and the workspace-hack only, and `ci/dep-rules.toml` allows `ew-units` besides): the model, the
measure, the text placer, the search, the net router, the tracks, the placer, the guides. It reads no project
record, so it builds and tests in seconds, which the eight steps of the build use on every change. `ew-scene`
calls its text placer; `ew-api` builds a `DraftPage` (it has the scene's text sizes) and offers `page_draft`,
`route_plan`, `arrange_plan`, `route_wire` and the layout findings; the tools get the model from the host, as
today. `ew_derive::routing` is deleted.

*Alternative: keep it as a module of `ew-derive`.* That crate is 27 000 lines and every router test would build
it; the code reads nothing of it.

