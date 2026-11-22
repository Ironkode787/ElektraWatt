# Drafting: routing wires, arranging parts and settling texts on a schematic page

**Status:** Accepted · **Date:** 2026-11-19 · **Decided by** [ADR-0034](../adr/0034-schematic-drafting.md) ·
**Supersedes** the routing parts of [06 Schematic editor](06-schematic-editor.md) §5.4 and of
[ADR-0033](../adr/0033-schematic-routing-model.md)

**Purpose.** This document says how ElektraWatt lays out a schematic page: where wires and tubes run, where parts
stand when the person asks the program to arrange them, and where the texts of parts and wires stand. It replaces
the page router of ADR-0033 and the scene's text rules with one design, and it is written so that it can be built
in the steps of §10 from this text alone. It has two grounds. The research is the note `docs/research/schematic-layout.md`; a rule taken from there is cited by its number (D-7, R-3). The second is the pilot's reference set: drawings that the pilot user named as neat and tidy. It is not in the
repository and nothing of it is reproduced here, but it was measured, and **where it and the research differ,
the reference set gives this program's defaults** (§1.3 marks those rules with an E).

## Summary

1. **One measure.** A page's drawing is judged by one function: what is a fault (never drawn by the program,
   reported by a check), and what is counted and weighed (length 1 per grid step, a bend 4, a crossing 6, a wire
   across a text 12, a text on a text or on another part 24). The router, the placer, the text placer, the checks,
   `ew layout` and the tests all call it. Nothing else rates a drawing.
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
   devices of a chain, five between a card's pins) and the places of texts are those of the pilot's reference set.
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
| C8 | A mark's wire that jogs to its trunk: a wire from a pin of an interruption point or supply mark to a tap (a junction, or a pin another wire ends at too) that bends, where the net goes on from the tap along the mark's axis, so the trunk runs beside the mark's line, off it | 32 | a wire into a mark stays on the mark's line (owner, 2026-11-22, FT-1909): on the sample's PLC page 104 Route's plan laid the 0 V trunk off its mark's line for some 28 steps less, so the jog costs more than that |

**Compared** in this order: fewer faults, then fewer unrouted wires, then lower cost. `Quality` is that triple
with the counts behind it.

*C8's tests.* `ew-drafting`'s `c8_a_marks_wire_that_jogs_off_its_line_to_its_trunk` prices a trunk two steps off its
mark's line at two steps, a bend and the jog more than the same trunk on the line, and a mark wired round a corner into
a pin no other wire ends at at no jog; `ew-api`'s `arrange_keeps_a_marks_straight_wire_on_its_line` runs Arrange and
route on the sample's 104, whose marks' wires run straight, and finds each of them straight in the plan.

**Never worse.** A plan (§3.9, §4.9) is offered only when the page's `Quality` after it is better than before,
and when no wire or text that was free of a finding (F2 to F6, C4, C5) gets one. The second half is what a person
sees as "a check's warning more": a drawing with nine crossings fewer and one wire newly across a text is not
offered.

**Reported, not weighed.** `ew layout` and the preview print four more figures, because they say what a reader
sees and make a page comparable before and after: the wires drawn **straight** against all wires; the
**turn-backs** (three segments in a row of which the third runs against the first: the loop round a caption);
the **detour** (the summed length against the summed city-block distance of the ends, in percent); and the
**labels moved** off their first place. Their causes are already in the cost.

*As built (steps 1 and 2).* F3 excepts the common beginning of two wires of one pin: their routes walked from the
pin together while they run the same way (the shared run of §3.4). F6 is a corner on another wire's corner: a corner or
an end on another wire's end, a junction on a crossing and a wire's end on a wire are F2 by their geometry. F7
leaves out an outline drawn round other parts, a part without pins and two parts on one pin's point (an
interruption point on a pin). Two texts meet when they overlap by more than a tick (a text's box is rounded outward
to ticks); C4 does not count a wire across a text of a cable mark or of the wire itself. The checks name parts as
the program does, "symbols": F7 is EW-CORE-077 "Symbol on a symbol", and §8's "Text on a part" is EW-CORE-076 "Text
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
| `STEP`, `BEND`, `CROSSING` | 1, 4, 6 | the cost |
| `TEXT_CROSSED`, `TEXT_STEP` | 12, 1 | the cost |
| `LABEL_CLASH`, `LABEL_RANK` | 24, 1 | the cost |
| `HALF_STEP_EXTRA` | 1 | the cost |
| `MARK_JOG` | 32 | the cost |
| `BODY_GAP`, `STUB` | 1 step each | router: the least distances |
| `TRACK_PITCH` | 2 steps (1 where a channel is too narrow) | tracks, parallel wires |
| `CHAIN_GAP` | 5 steps (never less than 2) | placer: between facing pins of two parts in line |
| `TRUNK_GAP`, `TRUNK_PITCH` | 2 steps each | router: a trunk from the pins it taps; trunks side by side |
| `MARK_PITCH`, `MARK_LEAD` | 2 steps each | placer: marks in a stack; a mark before its drop |
| `ROW_GAP` | 1 step | placer: between two footprints in a row |
| `ROW_PULL` | 6 steps | placer: a part joins a common line when within this |
| `MOVE_PRICE` | 2 per part | placer: what a move has to earn on the routed page (§4.7) |
| `REACH` | 24 steps | placer: how far a tree's root may step, each way, to find its tree room (§4.7) |
| `ANCHORS` | 6 | placer: the most units of a tree tried as its anchor (§4.2) |
| `BRANCHES`, `ROUNDS` | 8; 4 | placer: the most trees, and branches, whose plans are routed one by one; the most runs of one press (§4.7) |
| `EARTH_REACH` | 8 steps | placer: an earth symbol from the pin it earths |
| `LABEL_CLEAR` | 2 ticks (1/8 step) | text placer |
| `WINDOWS` | 4 steps, 16 steps, the drawing area | search areas round the two ends |
| `PASSES` | 2 | improvement passes; text rounds |
| `MAX_NODES` | 262 144 | search area bound (kept) |
| `GUIDE_SHOW`, `GUIDE_CATCH`, `GUIDE_CATCH_PT` | 2 steps, 3/4 step, 8 screen points | guides (§6) |
| `CARD_PIN_PITCH` | 8 steps | cards drawn from the station view (§7.5) |
| `CARD_HEIGHT`, `CARD_PINS_MOST` | 6 steps; 8 pins | cards drawn from the station view (§7.5) |

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

*As built (step 4).* `ew_drafting::rules`: each rule a function over the page that names the parts breaking it;
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
only a pair whose parts stood against no rule (one that had to move): it is closed up when it stands nearer than it
stood and nearer than the chain gap, and spread when it stands further than it stood, or than the chain gap, by more
than `ROW_PULL`. E4 does not read a pair whose two pins face the same way.

*What was sharpened on 2026-11-20, and why.* A4 was read for every pair; it then forbade the nesting of a stack and
the order of wires that turn back, both of which §4.4 and §1.4 ask for. A5 forbade any closing and any spreading; the
designed pages need a part that stood nearer than the chain gap to take it, a row to take one line, and a chain whose
host moved to keep its distance. E4 bound hairpin parts to the far side of the card. A4 gained the strip: without it
the plan could leave two devices of two terminals of one strip in each other's order.

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
2. *At the source (step 8 of §10):* the functions that give a pin's point, a box pin's point and a junction's
   point return the grid's own number when the point is within 1 nm of a grid position. Routes stored by an older
   build keep their numbers: a reader joins an end that is 1 nm beside its pin with a segment of 1 nm, which
   draws as nothing and which the layout never sees. The comparisons that today read points "settled" in six
   places outside the router then compare equal numbers, and those calls go.

*As built (step 8).* `Grid::settle_point` gives a point within 1 nm of a grid position the grid's own number; the
functions that give a pin's and a box pin's point return it: ew-derive's `cp_end` (and `rep_cp_end`, which takes the
page's grid), the commands' `rep_cps`, the drawn connection points of the scene and of the page tables, the two
importers' (tables and QElectroTech), and the tools' ghost pins (placing, the interruption point, a moved symbol's
proposals). A move (by steps, by a script's `delta`) and the Select tool's re-solve move a wire's end by its pin's own
displacement, which can differ from its symbol's by 1 nm. A position 1 nm beside a grid point is still off the grid to
the commands (its pins' points are on it, but it is no fitted symbol). The six settled comparisons are gone;
`Grid::settle` stays as the rule `settle_point` applies. Golden A's tables give its routes the grid's numbers: 38
coordinates moved by 1 nm, one jog of 1 nm on folio 101 went.

*As built (step 8b).* A stored route and a junction are read by the same rule, so that a drawing of an older build
(the last candidates' projects, every import) compares with its pins by equal numbers: `ew_derive::read_connection`
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
| A strip's drawn bridge (doc 08 §6.5) | a `keep_out` of its line and a tick to either side: no text stands on it, and the router does not read it (FT-2021) |
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

*As built (step 5).* No command of its own: Properties' row and the context menu's entry (`rep.keep_in_place`, a
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

*As built (step 4).* `parts: [{rep, steps: [kx, ky]}]` moves each symbol as `rep.move`'s `steps` does (one function
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

*Why the straights come first (FT-1932).* The search would draw most of them the same, but a straight laid first
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

*A route that misses its end (FT-1975, decided 2026-11-22).* No command stores one: every command that moves a symbol,
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

*As built (FT-1931).* It holds every node of the drawing area, the last area a search may take. It held what stood on
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
pin's parting draw one size (it was a quarter step, FT-1898).

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

*As built (step 2).* `ew_drafting::nets` and `route` (module documentation there). A trunk is offered to a net whose
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

*As built (step 3).* `ew_drafting::tracks`, run by `route` after the trunks. A stepping wire is the one wire of its
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

**Rings (FT-1895).** A wire between two pins, one on a body, whose other end lies behind that pin's edge (beside the
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
where it may (§4.4), so its wire goes round nothing. Where it stays behind the body, the rings need their depth and a
step between the body and it; the placer does not yet keep that room (backlog).

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

*Never worse, and a wire without a route (2026-11-20).* A wire without a route is the worst state a wire has. A
plan that routes one is offered although a text then lies across a wire that was routed before (the text had to
step aside; the result names the wire); only a fault on such a wire refuses it. Before, one such text refused the
whole plan and every wire stayed without a route.

*Two ends nearly in line (FT-2010).* Where the search's route between two ends is straight on the lattice but the ends
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
remaining job per pass, in a bounded area; two passes; two text rounds. There is no loop that runs until nothing
changes.

A job with no route free of faults keeps its stored route when it has one, else stays unrouted and is drawn as the
dashed line. The plan names it with the first thing in its way, found by searching again without the other wires,
then without the pins of other wires, then without the bodies: "other wires", "connection points", "parts", or
"a fourth arm at a junction". Nothing is ever drawn through.

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

*As built (step 2), §3.6 to §3.10.* The search prices a label at `LABEL_RANK` (it is taken to step aside) and a fixed
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

*A mark is drawn from its wires (FT-1880).* The page draws a cable mark's line past the outer crossings of its cores,
with a tick and a core name at each crossing; a marker's text beyond its line's end; a wire's own marker at the middle
of the wire's longest segment. The layout draws it so as well (`ew_drafting::marks::draw_marks`): the scene's adapter
gives each mark its point and what it draws relative to the crossing or the end of its line it stands at, and the
measure and the placer's page after a plan draw it from their routes. So a plan is judged on the page that is stored,
and a second press of Route or of Arrange and route offers nothing. Before, a plan kept a mark where it stood while
the page drew it where the new routes crossed it: on the sample, after a strip's earth terminal was moved two steps
up, the first press moved a cable's core name with its wire and the second routed a wire again.

---

## 4. Placing

### 4.1 The rule

**Put each part where the page is best for it, within the rules of a drawing of its kind.** That is almost
always: in line with the pin it is wired to, on the side that pin faces; the circuits side by side in the order
that crosses least; the potentials at the head and the foot. A part may cross the page for it. The spacing along
a circuit stays the person's.

*Best* is the measure of §1.2 on the arranged and routed page. *The rules* are those of §1.6; they are not
weighed.

**Every plan that is weighed has been routed.** The placer proposes one arrangement from the pins (§4.2 to §4.6)
and a few parts of it (§4.7). Each is routed by the router of §3 on the page as it would become and measured, and
Route's plan over the same wires is the plan to beat. Nothing estimates what the router will draw.

It is what a draughtsman does by hand, and it needs no template: a breaker's lower pin faces down, so the
contactor stands below it; a card's input pin faces up, so the terminal and the sensor stand above it; a valve's
working port faces up, so the cylinder stands above the valve.

*Alternative: a template per kind of page (rails here, eight channels there), as page macros in other tools.* It
needs more than the type of a page says, fails on mixed pages, and would recast a hand-made page into the
template's shape. *Alternative: a layered layout (Sugiyama, ELK Layered) of the whole page.* It draws a new page
from the netlist and does not keep what the person arranged; its crossing reduction is not needed where a part's
place follows from a pin. *Alternative: a general constraint solver for alignment and separation.* The
constraints here form rows on a tree, which a one-pass method solves (§4.4). *Alternative: only line
parts up and make room, every large part left where it stands* (this design's first form). It leaves a device on
the wrong part of the sheet, and the pilot user chose the full move. *Alternative: an upper bound on the parts
one run moves, or a second mode that only lines up.* A bound would leave a page half arranged at an arbitrary
cut, and every run is previewed, which is the asking; a second mode would be a third command, and the selection
and the preview's ticks give its result where it is wanted. *Alternative: rank the candidate places by an
estimate of the measure that needs no routing (the wires' city-block lengths, the bends their pins' sides force,
the crossings of their plain shapes), and route only the winner* (this design's second form, built first). On a
page with rails or many nets the estimate did not foresee what the router draws: plans it liked were refused
afterwards and the page fell back to Route's plan, and a chain of stages, each ranked by the estimate, undid its
own earlier choices. A page has tens of parts and tens of nets and a run of the router takes a fraction of a
second, so the real answer is affordable and the estimate went (ADR-0034, amended 2026-11-20).

### 4.2 Units, relations and trees

- **Units.** Parts with one `unit` number move as one: a valve terminal with its valves; parts the adapter finds
  inside a `frame_only` part; a part that stands with its pin on another part's pin (a mark set on a connection
  point). Every other part is its own unit. A unit is **fixed** when it is locked, outside the scope, unticked in
  the preview, off the grid, inside a structure box, or at the end of a wire that is kept.
- **Relations.** A relation joins a pin of one unit to a pin of another. By strength:

  | Kind | Pins | Best drawing | Where the second unit belongs |
  |---|---|---|---|
  | **Facing** | the two pins of a net of two, on opposite sides | straight, no bend | beyond the first pin along its side, the two pins on one line |
  | **Tap** | a pin of a net of three or more and the one pin of that net that faces the other way (a path's drop to its branches) | straight for the nearest branch | beyond that pin; the branch nearer across than every other in line, the others in their own columns |
  | **Corner** | the two pins of a net of two, at a right angle | one bend | beyond the first pin along its side, and the first pin beyond the second along the second's side |
  | **Level** | the two pins of a net of two, on the same side | two bends (out, across, back) | beside the first, the two pins level; else in the row beyond the pins, off the pin's line by its own width so that the wire passes it (§4.3) |
  | **Mate** | no wire: two units of one shape that stand level side by side or in line under one another, a step off at most, the nearest within twice `ROW_PULL` | — | where it stood against its mate: a strip's terminals, cards side by side, like devices in a row stay a row |
  | — | a pin without a side | — | no place follows |

  A tap relation holds only where the second unit already stands beyond the first (on a fluid page wherever it
  stands: rule F1 tells the supply from the work), and not for a unit in a row of three mates or more, which stays
  in its row. An end symbol of one pin that is no mark (an earth symbol among them) keeps its hook: it has no line
  to take (§1.3). A level pair is overruled, and counts after the mates, where more of the unit's mates face that
  host than are wired to it out and back.
- **Weight.** The number of a unit's pins that have a wire, times 4, plus its body's area in square steps over
  16.
- **Trees.** The wired relations are taken strongest first (of one strength the one nearest in line, then the
  heavier pair), each joining two units that no stronger relation joined yet: a spanning forest, in which every
  fixed unit is ground. Then the mates join those trees, the relation nearest the heaviest unit of the heavier
  tree first, and only where one of the two is the heaviest of its tree or fixed: a terminal with its device joins
  its strip, not the row of devices, and two marks of two strips are not joined. A unit's relation towards the
  root is its **lead**, the unit at its other end its **host**. A relation left out of the tree is kept for the
  stacks, which nest by it.
- **Anchors.** A tree hangs from its fixed units. A tree without one hangs from its anchor: of its heaviest units
  (those of at least three quarters of the heaviest's weight, `ANCHORS` at most) the one from which the tree is
  set with room for every unit, with every step a rail demands (§4.6) and with the least weight moved: a part
  out of place moves, not the circuit round it. Ties go to a unit a rail taps, then the heavier, the upper, the
  left one, the ID.

*Alternative: hosts chosen breadth first from the heaviest unit, a Facing relation before a Corner before a Level
one* (the form built first). The tree then depends on which unit is reached first, and a strong relation far
from the anchor loses to a weak one near it; taking the relations by strength over the whole page (Kruskal's
spanning forest) makes each part hang on the pin it is best drawn from. *Alternative: no mates.* A strip's
terminal without a wire of its own, or a card beside the card that moves, would stay behind and the row the
person made would be torn.

### 4.3 A follower's place

For a follower with its lead (host pin *h* with side *s*, follower pin *f*):

- **Across** (the coordinate at a right angle to *s*): a Facing follower, and the nearest branch of a tap, wants
  *f* on *h*'s line. It takes the line where the straight way between the two pins is free of other parts, and
  where that does not carry it past a member of its row that stays; else it stays where it stands across. Corner
  and Level followers have no line to take.
- **Along** (the coordinate along *s*): the follower stays at its present distance when it is beyond the host by
  at least 2 steps and its footprint is clear of the host's (one its host left more than `ROW_PULL` behind keeps
  the distance it had); else it stands `CHAIN_GAP` (5 steps) beyond, further when the tracks of a row's stepping
  wires need the room (§4.5). **A part in line is never pulled nearer, and a chain is never spread out**: how a
  path is spaced over the sheet is the person's (rule A5).
- **On the wrong side** (the follower stands behind the pin that faces away from it): it moves to the side the
  pin faces. This is the one large move the placer makes for a chain, and the preview shows it.
- **A level follower** whose wire has no way out and back (it stands under or over its host, so that the way
  out of one pin runs into the other part) is set beside the host, the two pins level, the pin nearest that side
  innermost, on the side the pins lie nearer to where it has the room, else on the other. One that stands neither
  beside its host nor near beyond its pins (2 steps to `CHAIN_GAP` and `ROW_PULL`, clear of the pin's line) takes
  the row beyond the pins: two steps clear of the host, further where the texts of the two need it, and off the
  pin's line by its own width so that the wire passes it, on the side where the host has the fewer pins of that
  edge. A lamp wired to a card's pin that faces the same way as its own stands at the card, on a short hairpin,
  and not across the sheet. Where no such place is free the follower stays. On a bus page two stations whose
  ports face one side stand level (rule B2).
- **A mate** takes its host's step, stands level with it, and takes the pitch its row now has where the row had
  one pitch before (a strip's terminal without a lead of its own stays in its strip, at the strip's new pitch).

The **footprint** of a unit is its bodies, one step along each wired pin, and its labels at their first places
(§5.2, with the stacked-marks rule of §5.3).

### 4.4 Rows and stacks

The followers of one side of one rank are placed together; a rank is the root, then the units that one step of
the method set beside one another. A level follower that keeps its place belongs to the next rank with the
members of the rows, so that what hangs on the terminals of a strip is one row, whatever joins each terminal to
the card.

- **A row** (Facing and Tap followers). Each wants its line (§4.3). The members whose extents meet along the side
  then stand in the order of their host pins with `ROW_GAP` between: by their bodies where the text placer then
  sets every text of the row clear and within a step of its part (a strip's terminals, two steps apart), else by their
  footprints. Followers narrower than the pin pitch stand exactly in line; wider ones form a row, and their wires
  become the stepping wires of §3.5. There are three ways to pack such a row, and the one is taken whose stepping
  wires cross the fewest of the wires that pass the row, and that sets no member outside the drawing area:
  1. by the least total move from the wanted lines under that order and those gaps (neighbours that would
     overlap are merged into a block, the block stands at the median of its members' wishes, and merging repeats
     until no two blocks overlap: pool adjacent violators, one pass);
  2. the first member at its line and each next one as near to its own as the gap allows;
  3. the last member at its line and each one before it likewise.

  A wire passes the row when it leaves the row's rank on the same side and does not end at a pin of the row that
  faces the rank: the supply's riser from a strip's first terminal to the far pins of a row of sensors. The second
  and third way set the row beside such a riser where the first would set it across. Members that keep their
  places and their distances are not packed: no part steps aside for a wire.

  Along, the row has one line: the largest distance its members need or keep, for members within `ROW_PULL` of
  it; a member further out keeps its own, and a keeper one step off the distance that most keepers of its shape
  keep takes that (a row set down a step crooked is levelled). Members that stand more than `ROW_PULL` beyond
  the nearest are set after what hangs on the near ones: a device wired straight to a card does not take the
  place of the row over the terminals. A member that then stands on a unit already set steps further out on its
  line where the line is free that far (two flow controls wider than their ports' pitch stand staggered, both in
  line); else it stands where it stood across and steps out from there.
- **A stack** (Corner followers on one side of a rank that turn the same way; mostly marks). Each member is set
  first with what hangs on it, as one block. The blocks are ordered along so that their wires nest: the drop
  nearest the side the blocks stand on turns first. A block keeps its place when every one of its wires has its
  corner (the drop at least 2 steps long, its own pin at least `MARK_LEAD` before the drop), it lies beyond the
  blocks before it and stands clear; else it takes the nearest such place, in the column where most single
  marks of the stack stand (a column of marks at the sheet's edge stays a column, with nested wires), or in the
  column `MARK_LEAD` before the nearest drop. The pitch is `MARK_PITCH` (2 steps), or the next larger of 3 and 4
  steps at which the text placer finds every text of the stack a place as clear as it had (§4.5). A stack that
  leaves the drawing area slides back towards its hosts as far as its first block has room.
- **A mark at the end of a rail** stands on the rail's line (§4.6).

The order within a row or a stack follows the hosts' pins, not the places the followers had.

*As built (2026-11-20, pages with cards).* What the pages with a card at the head and one at the foot needed:

- **A strip's terminals stand by their channels.** A member of a row does not take a line that carries it past a
  unit of its row that has no line of its own (a fixed one, or one that hangs beside its host), nor further from
  it than it stood by more than `ROW_PULL`; a unit of the row with a line of its own binds nobody. So the terminals
  of one strip that are wired to two cards each stand at their own card's pin (rule E4).
- **A terminal without a wire joins one strip.** A mate relation holds where one of the two is the top of its
  tree; a unit that hangs on its mate is the top of no tree any more, so a spare terminal between the terminals
  of two cards does not join the cards into one tree, in which one card would hang on a terminal.
- **A wire across a row asks nothing** where one of two units of that row would have to leave it (two cards side
  by side, a pin on the upper edge of one wired to a pin on the lower edge of the other); a wire along a row of
  parts in line keeps its relation.
- **A mark set on a pin.** A mark whose arrow lies along the body of the part it is set on (a potential's mark on
  a controller's supply pin) has no way out on its own side: it asks for no row. And a pin with such a mark is
  where the potential arrives: neither part steps for a corner of the wire from there.
- **A block heavier than its host keeps its place** in a stack: a card with its channels does not step for the
  corner of one supply wire.
- **A row nearer than the chain gap.** Where a member that takes the row's line would stand on a unit that is
  set, the members that take the line step back together, a step at a time and no nearer than `CHAIN_GAP_MIN`
  and their tracks, to the first line that is clear: sensors under the controller's box.
- **Who is in the way.** A tree is set as if the units of the trees after it will step aside. A unit that then
  kept its place where a moved unit came to stand is in the way from the start, and the trees are set again,
  `PASSES` times at most.
- **A card steps along its edge.** A card that is the root of its tree, with its channel pins on its upper or
  lower edge and its supply pins on its left side, steps right along its edge (a) until every pin its supply is
  wired to on the side its channels face (the controller's supply pin, a mark, the supply pin of a card at the
  other edge) stands left of it, so that the supply drops beside its first channel and crosses none, and (b)
  further where the columns of its channels in use interleave with those of a card that faces it (one card's used
  columns among or on the other's): of the two, the card with fewer channels in use (of two as many, the one further
  right) steps on until its used columns stand right of the other's, two steps clear, so that the parts of the two
  cards' channels do not stand among each other and a strip between them keeps its order. Columns without a wire
  count for nothing (FT-1894; it was half a pitch off all the pins of a card of the same pitch). Whether
  that is the better page the judge finds: where a card would step, the arrangement is made with (a) and (b),
  with (a) alone and with neither, each is routed and judged, and the best by the measure with the price of its
  moves is offered. The command that draws a card still sets it from the left edge; the placer moves it.

### 4.5 Room, and the texts

The distance between a host's pins and a row is at least `CHAIN_GAP` and at least one step more than the tracks
the row's stepping wires need (`tracks_needed` of §3.5 on the wanted places; with the narrow pitch where a
keeper stands nearer). So a row is set far enough for its wires before the router runs.

Whether texts fit is not estimated: **the text placer is asked.** For a row packed by its bodies, and for each
pitch of a stack, the placer builds a page of those parts alone where they would stand, each lead drawn in its
plain shape (straight, with its one corner, or stepping at the middle) and every other wired pin with its first
two steps of wire, and calls the measure. A row is packed by its bodies only where no text is then more on a
text, on a part or under a wire, and none more than a step away from its part (§1.3), than with the parts where
they stand; a stack takes the first pitch that leaves its texts as clear as they stood. Each question is asked once for a
given set of parts and places.

*Alternative: footprints with the labels at their first places, and no question.* A page link whose name stands
above its arrow is three to four steps tall by its footprint, so a stack could never take the pitch of two steps
that §1.4 asks for, and a strip's terminals could never stand two steps apart; the text placer finds those texts
a place (the stacked-marks rule of §5.3, the two rows of §5.2).

### 4.6 Rails

A net of three pins or more whose pins, but marks', all face one side up or down is a **rail**, whether it is
wired through junctions or from pin to pin. The units it taps belong on one row: the row most of them stand on
(of two as full, the one nearer the rail), for each unit within `ROW_PULL` of it. Where the rail has its
junctions on one line (two at least), a tapped unit that stands nearer that line than `TRUNK_GAP`, or beyond it,
has to take the step: a tree is set only where it does (rule E2). A mark of such a rail belongs on its line. The
rail's own coordinate is the router's (§3.4): it follows the rows.

A unit that has a lead of another kind keeps its place across from the lead and takes the rail's row along
where its lead leaves it the room (the coils of a control page on one line).

### 4.7 Which plan is taken

1. **Where a tree stands.** A tree is set from its root where the root stands. Where a unit of it then has no
   room (it would leave the drawing area or stand on a unit that is set), the root steps by whole steps, the
   least first, up, down, right and left, `REACH` (24) at most each way, until the whole tree has room; the
   other roots of its shape that stood level with it take the same step along that axis, so that a row of cards
   stays a row. Without such a step the root stays and what has no room goes back to where it stood. The trees
   are set heaviest first, each clear of those set before it. On a fluid page (rule F1) what hangs on a valve's
   unit with a part of the supply steps below every valve, and what hangs there with an actuator steps above
   that unit's valves and takes the blocks above it along.
2. **The rules.** The parts a broken rule names go back, with what hangs on them, until `rules_kept` holds
   (§1.6; the rules read the parts and the nets, so this needs no routing).
3. **Routed and judged.** The arrangement is routed and measured. It is taken when
   - the rules hold on the routed page;
   - no wire and no text gains a finding (the never-worse rule of §1.2, with F7);
   - it has no more faults than Route's plan over the same wires, and, where it leaves no more wires without a
     way, no more crossings;
   - and it is better than Route's plan by the measure with the **price** of its moves added: `MOVE_PRICE` (2
     steps of wire) for every part that does not stand where it stood. The price is not part of the measure: it
     is what a move has to earn, so that a part moves only where the page gains more than the move costs, and
     never for a tie.
4. **In parts.** Where the whole arrangement is not taken, its trees are tried one after another, each kept
   when the page gets better with it; then the arrangement without one branch, and one branch alone (`BRANCHES`,
   8, of each at most). A **branch** is a unit's own move with what hangs on it; the members of a row the person
   made (of one shape, level, on one host or on hosts that are a row themselves) are one branch, so that a row
   goes or stays as one and is never left half moved.
5. **The price, for real.** From the plan taken, each branch is put back in turn, where that breaks no rule the
   plan set right; the plan without it is routed, and kept when it is the better one with the price of its
   moves.
6. **Without a better plan** Route's plan of the scope is offered: no part moves, and the parts that stand
   against a rule and have no room are named.
7. **Again.** The run is made again on the page its plan leaves, until a run changes nothing (`ROUNDS`, 4, at
   most). The plan offered is the sum, so a second press finds nothing to do.

A part that stays against a rule is named in the plan: "no better place below -F2; left where it stands" (it
had no room there, or the page was no better with it there). The placer
does not close up or spread a circuit, does not turn a part to make it fit, and does not move a part to another
page.

*What was weighed in the judge.* Fewer crossings bought with a few more bends is a better page, and so is the
reverse where many bends go; both are what the measure's weights say (a crossing 6, a bend 4), so the measure
decides, and not a rule that neither figure may rise. One figure is held all the same: a plan that moves parts
may not have more crossings than Route's plan with every part in place, because a person who sees a part moved
and a new crossing beside it reads the move as the cause.

### 4.8 What keeps a page recognisable

Parts may cross the page, so these hold the person's picture of it:

- **The preview.** Every move is shown before anything is stored: the page as it would become, each moved part
  tinted, its old place outlined and joined to the new one. The card says how many parts move and lists first the
  ones that move further than a third of the sheet. Unticking a part plans again around it (§4.9).
- **Keep in place and the selection.** A part marked to stay stays; with a selection only the selected parts
  move.
- **The price of a move.** No part moves for a small gain or a tie (§4.7), so a page that is already in order
  comes back with an empty plan, and a second run after a first changes nothing.
- **Rows.** A row the person made (a strip, cards side by side, like devices level) moves as one or stays as
  one, at the pitch it had or the one its new place asks of all its members. Rule E4 comes first: a terminal
  wired to a card's pin stands in that channel's column, so a strip between a card and its field devices takes
  the pitch and the order of the channels, and a terminal of it that is wired to another card stands at that
  card's pin. A terminal of the strip wired to no channel but elsewhere (a feed terminal) stands at the strip's
  end, on the side its wires lead to (a wire to a part that stands in a channel's column counts at that column; one
  to a part that hangs on the terminal, its supply mark, follows it), so its wires cross no channel's; a spare
  terminal, without a wire, keeps its place in the strip.
- **Order.** The parts of a circuit stand in the order of the wiring, the parts of a row in the order of the
  pins (rule A4, which reads the terminals of a strip as one row).
- **Spacing.** Along a circuit the distances are the person's.
- **The rules of the kind.** After a run a device stands where a draughtsman would look for it: the potentials
  at the head and the foot, a load under its protection, a field device at its channel, an actuator over its
  valve.

There is no upper bound on the parts one run moves, and no lighter mode, for the reasons of §4.1.

**Where the placer moves nothing.** A page whose wiring runs against its symbols cannot be made good by
placing. The plain case: a card's pins face up and each is wired to the *upper* pin of a terminal that stands
below the card. Every such wire has to go round the card (four bends), and with the strip in the order of the
pins the wires that pass one side of the card cross each other pair by pair: `m` wires round one side cross
`m (m - 1) / 2` times, whatever room the placer makes. (An earlier text of this section said they nest clear;
they do so only with the strip in the reverse order.) The placer does not reverse a strip, does not turn or
mirror a terminal or a card (rule A3), and so on such a page it offers Route's plan and moves little or nothing.
The remedy is the person's: wire the terminals by the pins that face the card, or use the card with its pins on
the side the field stands.

**A card in the older form.** Until step 7 "Draw on a page" drew a card with two rows of pins, so half its
channels face away from their circuits: such a page is the case above. Draw the card in one row, in the box's menu
(FT-1897, [10](10-plc-and-bus.md) §3.6), draws the card again in its place without wiring it anew, in one undo step:
the device, its channels and every connection stay, because a connection joins connection points of the device's
function, not the box; the connections end at the new pins, and Arrange and route draws them. What stays for the person is
each terminal of a channel that now faces the other way: the wire to the card belongs on the terminal's pin that
faces the card. A command that swaps the two wires of such a terminal for a whole strip would take that hand work
away; it is not built. Route lays the wires round one card together as nested rings (§3.5, Rings): they
remove no crossing that the order of the strip forces. The placer's room for the rings is not built: Arrange sets
such terminals beside the card as hairpins (§4.4).

### 4.9 The plan

`arrange(page, scope, fixed) -> Arranged`: the parts' moves, the parts left against a rule with the side their
host's pin faces, and the plan of the page as it would become (the routes, junctions and marks, with the
quality before and after). The plan is empty when nothing better than the page as it is was found, and the
message says that the page is already arranged as well as the rules allow.

**A selection** makes only the selected parts movable; everything else is fixed and can be a host. A selection of
one feeder arranges that feeder and leaves the page.

**In part.** Unticking a part in the preview adds it to the fixed units and plans again; unticking a wire keeps
its route and the parts at its two ends. The result is a consistent page whichever rows are ticked. The preview
never applies half of a plan.

**The same plan whatever the order of the page's lists**: units, relations and blocks are taken in an order made
of their weights, places and IDs.

*As built.* `ew_drafting::place` (the run and the judge) and `ew_drafting::place::sketch` (the arrangement),
each with its module documentation step by step. Units by `unit` number, by an outline drawn round other parts
(`frame_only`) and by a pin set on another part's pin; a unit is fixed also when its origin is off the whole grid
(the API keeps a symbol whose position is not exactly a grid point fixed, since a move by steps counts from it).
A junction that stands in line with the drop of a part's pin follows that part across, and a junction joined to
it by a wire with it, so that a rail's junctions stay over the parts they feed when those move. The text placer
is asked through `measure` on a page of the parts in question (§4.5). Not built: an order of independent
circuits across the sheet other than the one their trees' steps give (§1.6 leaves it to the measure; no candidate
swaps two trees), and the labels between a host and a row as room of their own (the text placer moves them).

---

## 5. Texts

### 5.1 One text placer

`settle_labels(scene) -> Vec<(LabelId, Rect, rank)>` is the one place where a label gets its place. It runs when
the scene draws a page (in place of the scene's present rules), inside `measure`, and inside the router and the
placer. Its input is what is stored: the parts' bodies, the wires' routes, the fixed texts, the drawing area and
the labels with their sizes and places. So a page looks the same on the canvas, in the PDF and in the plan.

### 5.2 The places of each kind of label

The first place of each kind is this program's default, taken from the pilot's reference set; the starter
symbols' slots are set to these first places (step 7), and a library symbol whose author chose another slot keeps its slot as first place.
A symbol turned a quarter has the first places [07](07-symbols-parts-macros.md) §4.1 places anew (owner, 2026-11-22):
lying flat, its tag centred above the body and its other texts centred below it, line under line; the symbol editor's
strip of variants and a page show the same.
The other places follow from the part's shape. "Free side" means a side of the body without a pin. For a part
whose pins leave up and down, "left" and "right" are meant as written; for a part turned a quarter they turn with
it, and a text always reads from the bottom or from the right (D-16). A part lying flat has, last, the place centred
above over its tag for each of its texts: where wires run under it (a switch across the phases of a supply), its data
stand over its tag.

| Label | Places, in order | Priority |
|---|---|---|
| Pin number of a symbol (terminal designation) | right of the pin's line at the pin's outer end; left of it | 1 |
| Tag of a symbol | at the upper left: right-aligned against the symbol's left edge, its baseline level with the upper pin's end; the upper right (left-aligned against the right edge); the lower left; the lower right; centred above, when no wire leaves upward; centred below | 2 |
| Cross-reference at a contact | under the tag, in brackets; under the tag's next place | 3 |
| Make, type and rating of a symbol | under the tag (and its cross-reference), line under line, on the tag's side; the same on the other side | 4 |
| Tag of a drawn box | above the upper left corner, right-aligned to the box's left edge; above the upper edge, left-aligned at the corner; above the upper right corner; inside the box at the upper left, where no pin text stands. A place in the way out of a pin a wire leaves (over a top pin, beside a pin on a side, within `LABEL_CLEAR` of its line) comes after every place clear of such ways, so the wires into the pins do not go round the caption (FT-1831) | 2 |
| Make and type of a drawn box | left of the box under its top, line under line; right of it; inside it under the tag | 4 |
| Contact list of a coil | left of the coil, under its tag, one line per contact; under the lower trunk below the coil; the same one line lower (two rows, where the neighbours' lists are in the way) | 3 |
| Function text of a path | centred under the path beyond the lower trunk; right of the load | 4 |
| Name of a mark (interruption point, supply mark) | in line with the wire beyond the arrow: left of an incoming mark, right of an outgoing one; above the arrow | 3 |
| Cross-reference of a mark | under the wire on the wire's side of the arrow; above the wire there | 3 |
| Name of a terminal | right below the terminal's point; left below it | 1 |
| Strip designation | left of the strip's first terminal, level with it; above it; above the terminal's name, where it stands over the terminal; above it, ending at its left edge | 4 |
| Wire number, tube tag | along the wire: on a vertical stretch right of the line, turned to read from the bottom; on a horizontal stretch above the line. Tried on each straight stretch at least the text's length plus two steps long: first level with the number of the parallel wire beside it that is already placed, then at the middle of the longest stretch, then at its quarters, then on the other side of the line | 5 |
| Type text of a cable mark; the cable's tag | the type left of the row of ticks, the tag right of it; both right; both left | 5 |

The texts of one part on one side are a block: they keep their order and stand line under line. Where tags of a
row are wider than the row's pitch, every second one takes its "one line lower" place: two rows, as a
draughtsman staggers them.

### 5.3 The choice

Labels are placed in the order of priority, then of their owner's place (top, then left), then of their ID. Each
takes its first place that is clear by `LABEL_CLEAR` of every part's body but its own, every wire, every fixed
text and every label placed before it, and inside the drawing area. A label with no clear place takes the place of
least cost (C4, C5) and is then counted. One repair round follows: for each such label, each label it stands on
that came later in the order is tried at its next clear place, and the change is kept when the page's cost is
lower. The result does not depend on the order of the page's lists.

A label leaves its first place only when something stands there, so a change elsewhere on the page does not move
it.

**The row rule** (FT-1870). Like parts that stand level (one shape: the same body and pins, their origins on one
line) take one place for each of their texts: when the first of them comes in the order, the place the same number
down every one's list at which all of them are clear is taken for all, so the tags and cross-references of a row of
relays stand alike and a wire across one tag's first place moves the row's tags, not that one. Where their lists are
not alike (a wire leaves one of them upward) or no place is clear for all, each is placed on its own; texts wider than
the row's pitch keep the two-rows rule of §5.2.

**The stacked-marks rule.** A symbol's author may have set a mark's name above its arrow (the starter symbols of
before step 7, an imported library). Such a mark is three to four steps tall with its texts, and marks stacked two
steps apart (§4.4) would stand on each other's names. So the name of a mark whose texts, where their author put
them, reach another mark or its texts, or leave the drawing area, has the first place of its kind in §5.2 before
the author's: in line with the wire beyond the arrow. A mark that stands alone keeps its name where its author put
it, and so does a name its author set in line already (the starter symbols since step 7). The placer reads these first places (`first_places`), so it sets a stack by the texts as they will stand.

### 5.4 What is never moved

- **A text the person moved** keeps its place and angle with its part, and is a fixed text to everything else.
- **The pin texts of a drawn box** are as the scene draws them: inside the box, turned a quarter and reading from
  bottom to top on the upper and lower sides, horizontal on the left and right sides; a card drawn in one row prints
  its channels level in cells ([10](10-plc-and-bus.md) §3.6). The layout treats them as fixed texts and changes
  neither place nor angle.
- Free texts, captions of structure boxes and plug pin notes. A text of a symbol ([06](06-schematic-editor.md) §3.8)
  is the symbol's to the layout: a plan that moves the symbol moves it along.

**Where the placer may move a text** (FT-1875). The placer places only a text without a stored place: a slot text at
its symbol's slot (the author's place is its first place, which it leaves only when something stands there, §5.3),
and the texts the program derives (a strip's tag at its first terminal, a wire's number, a cable mark's tag and type
line). A text with a stored place is never moved: a slot whose place the person set by dragging it, or the import set
from its drawing (a slot override with a position), and every free text. The row offered two forms, a switch that
leaves a text where its symbol has it and the placer only for texts without a place; this is the second, for two
reasons. The stored place already says, text by text, that a text stands where someone put it, so a switch would say
it a second time, page-wide, and would also freeze the starter symbols' texts that the program places on a new symbol
drawn onto such a page; and a switch needs a member, a row in Properties and a default to choose. **An imported page
gets it with nothing set**: the QElectroTech import stores the place of every text it brings, a tag, a terminal's
number or a cross-reference as a shown slot at the place its drawing has it ([14](14-interop.md) §4.4.2), any other
text of an element as a text of its symbol; so the page prints them there, a move of an element and Arrange and route
keep each on its element, and only the texts the program derives are placed. *Not done:* a person who wants a starter
symbol's texts kept at its slots whatever stands there drags each once; Reset text positions gives them back.

*A data text on its own pin's line (FT-1980, decided 2026-11-22).* A symbol whose author set its make, type or rating
on the line of one of its pins prints it through that pin's wire only where the text's place is stored. Without one the
text gives way to the wire as every label does (§5.3: a wire stands at its first place), and nothing is added for it.
A stored place stays the person's, whatever runs through it (the paragraph above): a script or an import that pins
every text of a symbol where its author drew it keeps them there, and Reset text positions gives them to the placer.

### 5.5 Derived, not stored

Label places are computed, not stored, as the tag rule of today is. *Alternative: the layout stores a place for
every label it settles.* That would make texts as stable as routes, but it needs a mark in the file to tell the
program's place from the person's, a text command that takes many, and text moves in every tool that writes a
route; and a part placed by hand would keep a colliding tag until a button is pressed. The present rule (a text
is placed by the program unless the person moved it) stays, with one placer instead of nine steps. What is given
up: a later version of the placer may place a text of an old drawing differently. The fixtures of §9 hold its
results.

*As built (step 1).* The adapter is the scene's (`ew-scene`, `draft.rs`): it builds the page's `DraftPage` from
what the Labels layer draws, and `Session::page_draft` adds the supply ranks and the page's kind; `role` is
`Other` until the placer reads it (step 4). A symbol's slot, where its author put the text, is a label's first
place; there a label only must not lie on a fixed text or a text of its own part, and two texts of one part both at
their author's places do not count against each other. After §5.2's places each kind has a few more, tried in this order: one
line lower (a tag in a row); above the upper corners; a strip designation above it, ending at its left edge; a
mark's name one line higher; a drawn box's data under the earlier data, and above the box over its tag; a
terminal's name turned a quarter, and beside its body; a mark's cross-reference beyond the arrow, under then over
the wire. A label stands nowhere it reads as another part's (nearer another part of the same body size than its
own), and a label of a part without pins stays inside its outline or clear of it. A place that is the free first
place of a label not yet placed is taken only when no other is clear. A text whose author's place lies on a text
of its own part placed before it has the place under that text second (line under line). The repair round tries
each label a clashing label lies on at its first other clear place, and the clashing label then at its first clear
place.

---

## 6. Placing by hand, helped

**The rule.** While a part hangs on the pointer (the placing tool) or is dragged (the Select tool), it stands on
the grid point nearest the pointer, as today. Nothing computes a place for it, and nothing moves it after it is
set down.

**Guides.** `guides(page, ghost, at) -> Vec<Guide>` gives lines to show:

| Guide | Shown when | Drawn as |
|---|---|---|
| **In line** | a pin of the hanging part is within `GUIDE_SHOW` (2 steps) across of the line of a pin it would face (opposite sides, nothing between them, within the drawing area), or of the pin it is already wired to | a dotted line from that pin along its side, through the hanging part's height; solid, with a tick across, when the pins are in line |
| **Level** | a pin of the hanging part is within 2 steps of the level of the same pin of a part of the same symbol in the same row | a faint dotted line through both pins |

**The nudge.** Across an In-line guide, that guide's grid line is taken when the pointer is within `GUIDE_CATCH`
(three quarters of a step) of it, where the ordinary snap takes a line within half a step; never more than
`GUIDE_CATCH_PT` (8 screen points) beyond the ordinary half step, so the band thins when zoomed in. The part is
still on the grid: the nudge only decides between two neighbouring grid lines, in a band a quarter of a step
wide, in one direction. Moving the pointer past the band lets go; no key is needed. Level guides never nudge.
With Alt held there are no guides and no nudge (Alt already places without connecting).

**The wire's ghost.** When the hanging part's pin is in line with a free pin it faces, the straight wire it would
get on placement is shown dashed, as connecting on placement shows it today. No router runs while a part hangs or
is dragged: a moved part's wires follow as today and are solved at release (§7.4).

*Alternative: show where the placer would put the part and snap to it.* It is what the pilot user ruled out: a
part placed by hand stands where the person puts it.

*As built (step 6).* `ew_drafting::guides(page, ghost, pointer, points_per_step)` takes the hanging part at the
ordinary snap's place and the pointer in ticks; the tools call it on the canvas page's `DraftPage`. Each pin of the
hanging part has at most one In-line guide: the facing pin nearest across, then along. A pin it is already wired to
must face it too, but may have a part between. The nudge works per axis, only onto the neighbouring grid line, and not
on an axis where a guide is already in line at the snap's place. A faced pin off the part's grid nudges nothing. A
Level guide is drawn beside the nearest part with the same pins at the same points and sides (the same symbol in the
same orientation), whose body shares the hanging part's height. It runs at that part's level through the two top pins.
Select shows guides while the drag moves one symbol; the keyboard move (M) shows none. In-line guides are dotted in
`canvas.snap`, solid with a tick across half a step from the hanging pin when in line; Level guides are dotted in
`canvas.ghost`.

---

## 7. The window

### 7.1 The two buttons

In the Connect group of the command bar, after the Wire tool, and in the context menu of a selection:

| Action | English | Dutch | Tooltip (English) |
|---|---|---|---|
| `layout.route` | Route | Routeren | Route the wires of the selection, or of the page. Parts stay where they are. |
| `layout.arrange` | Arrange and route | Schikken en routeren | Line up the parts of the selection, or of the page, and route their wires. You see the result before anything changes. |

With a selection, each works on the selection (§3.2, §4.9); with none, on the page. On a page that is not a
schematic, fluid or bus page they are disabled. They replace **Route page** and **Reroute**, which go: Route does
what both did (it routes the unrouted, and improves what the program routed), and it no longer needs a second
button to say which.

### 7.2 Route

1. The plan is computed.
2. *Nothing to do:* a toast, "Nothing to route: every wire runs as well as the rules allow."
3. *No wire of the person's would change:* the plan is applied at once (one undo step). A toast names the wires,
   "Routed 5 wires: 102.3, 102.7, 102.9 and 2 more", with Undo; the wires are drawn as selected while the toast is
   up; wires left unrouted are named with what is in their way.
4. *A route the person shaped would change* (it is broken, §3.2): a preview card as in §7.3 with those wires as
   ticked rows; nothing changes until Apply.

### 7.3 Arrange and route: the preview

Always a preview (ADR-0008's fork). The canvas shows the page as it would become: the parts that move tinted,
their old places as faint outlines joined to the new ones by a dotted line, the new routes drawn; holding **B**
shows the page as it is. The Preview tab holds the card:

- the lead: "Arrange and route: page 102. 14 parts move, 3 of them across the sheet; 9 wires are routed, 2 of them
  drawn by you. Nothing has changed yet.";
- now and then, from the measure: crossings, bends, wires straight, wires through texts, faults;
- **Parts:** a ticked row per part that moves, with its tag and its move in words ("6 steps right, 31 up"), the
  parts that move further than a third of the sheet first;
- **Wires you drew yourself:** a ticked row per route of the person's that would change (every wire of the scope is
  a job, §3.2); the heading's own tick unticks them all, or ticks them all again;
- **Left as it is:** parts kept in place, parts without room (with the reason), wires without a way;
- **Apply** and **Cancel**.

Unticking a row plans again with that part fixed, or that wire kept, and the canvas follows. An unticked row
stays out of every later plan of that preview, and a plan made again brings in no wire of the person's that the
card did not list: those keep their routes too, so unticking the heading means that none of his wires changes.
When the unticked rows leave nothing to apply, the card closes and the toast says so ("Nothing is left to apply:
every change of this preview is unticked"), not that the page is already arranged. Apply stores what
is shown, in one `layout.apply`; one Undo takes all of it back. After Apply a toast says "Arranged page 102: 4
parts moved, 9 wires routed" with Undo. The undo step is named "Arrange and route" when parts moved and "Route wires"
when none did (Route's plan offered instead). The canvas that showed the whole page when the card opened shows it
whole again once the card is gone, whatever zoom was taken while it was open.

*As built (step 4).* `layout.arrange` beside Route in the Connect group and in the menu of a symbol and of a selection
of symbols; `ew route --arrange`. The card (the Preview tab) has the lead, the figures now and then, a ticked row
per symbol that moves (far moves first, its move in steps) and per wire you drew that gets a new route, and "Left as it
is": kept symbols with their pin, symbols without room with the side, wires without a way and nets wired against
their order. A tick (`layout.arrange_tick`) plans again with every unticked symbol where it stands and every unticked
wire as it runs; unticked symbols stay listed. The canvas paints the page the preview's candidate would leave
(`Session::preview_page`, read once per plan) with each moved symbol's new place tinted, its old one a faint dashed
outline and a dotted line between; while B is held it paints the page as it is (B, the black box tool's key, does
nothing else while the card is open). The toast after Apply: "Arranged folio 102: 4 symbols moved, 9 wires routed."

*As built (step 4b).* The jobs are every wire of the scope (§3.2), and Route's plan of the same jobs is the plan to
beat. The lead counts the wires you drew that get a new route ("9 wires are routed, 2 of them drawn by you"); the
heading "Wires you drew yourself" is a tick of its own (`layout.arrange_tick {all}`) that unticks every row under it,
or ticks them all. Such a wire's row belongs to the card even where Route alone keeps the wire as it runs (Route
keeps a drawn wire; Arrange's plan gives it a new route where that beats Route's plan): with every symbol unticked the
card can still list them, as the sample's folio 103 lists -X1031:SH/b → -103M1:PE (FT-1892). The history names a
`layout.apply` with symbol moves "Arrange and route" (`ARRANGE_STEP`), one
without "Route wires". A canvas fitted when the card opened fits again at the first new size of its pane after the card
is gone.

*As built (the placer anew, 2026-11-20).* A tick plans again with the unticked rows and, of the wires the person drew,
every one the card does not list kept as they run (`ShellModel::act_layout_arrange_tick` reads them from the page's
draft); when nothing is left to apply the card closes with `toast-arrange-none-left`.

### 7.4 The Wire tool, connecting on placement, moving

- **The Wire tool** stays as it is for the person: click a pin, click a pin, the tool finds the way; a click on
  the grid fixes a bend. Its inner workings become `ew_drafting::route_wire`, the search of §3.6 against the
  occupancy of the page, so the tool and the Route command cannot disagree. Two things improve: a wire no longer
  goes round a tag that has another free place (the tag steps aside when the wire is stored, because labels are
  derived), and a wire drawn to a pin that already has a wire may share that wire's run (§3.4). A route found between two clicks is marked `computed`; one with a fixed bend is the person's. A wire that ends
  on a wire is joined in the bevel form (§7.6).
- **Connecting on placement and on a move** stays as it is: straight wires between pins in line, shown before the
  part is set down, never changing a connection. It needs no router.
- **A move by hand** keeps its re-solve at release: wires follow their ends, and those left broken or with more
  bends are routed (§3.2) and stored with the move. It uses `route` with those jobs.
- **`ew route`** on the command line runs Route for a page or a selection (`--arrange` runs Arrange and route);
  `--dry-run` prints the plan.

*As built (step 2).* Route is `layout.route` in the Connect group where Reroute stood, and in the menu of a wire and
of a symbol; its toast lists the wires without a way and the nets whose order runs against their places. A
page-wide Route says how many routed wires it left as the person's (every wire of a file without the mark counts as
his); where routing them anew would make the page better, the toast offers Route these too…, which opens the preview
card of §7.2. The Wire
tool's dot key is `.` (`tool.wire_dot`, with a hint while the end is on a wire); the junction editor (Enter on a
selected junction) gives the dot to a junction that exists. A move's re-solve keeps every junction where it stands
(its command moves none). Paste and the grid fit route their wires on the same core and store one `layout.apply`.
`ew route` stores Route's plan; its JSON keeps the reason key `symbols` for parts. The scene draws the parting of a
shared run (§3.4): of two wires that turn away from each other there, the one turning right or down takes the bevel;
of one going on and one turning, the turning one; a run or a way on shorter than a step keeps its corner. A
symbol dropped on a shared run is refused ("two wires run here; place the symbol where they have parted").

*The straight line (FT-1864).* Between two ends on one line the tool takes the straight line where it crosses
nothing (`ew_drafting::route`, `crosses_nothing`): it leaves a pin end along its side and enters the other against
it, takes no turn back after a fixed bend, and on it lies no pin or junction but at its ends, no wire (along it, across
it, or meeting it but at its ends), no body but its ends' own parts' (nor a body's edge) and no text that cannot step
aside. A pin's stub and the step round a body close nothing here, as they do to the search (§3.6): a person who draws
a rail one step over a row of pins gets it as drawn, and each tap he then draws drops down a stub onto it. Before, the
search took the rail round the stubs without a word, and a tap clicked onto the line it was drawn on found no wire
there. Where something stands on the straight line the tool goes round, and its hint says so ("Something stands on
the straight line between these two points, so the wire goes round it"). Route keeps such a rail and its taps: no way
round costs less (§3.9).

### 7.5 Cards drawn from the station view

The router and the placer work with a card as it is drawn, whatever its form. "Draw on a page" draws a card
as **a low, wide box with its channel pins in one
row on one edge, `CARD_PIN_PITCH` (8 steps) apart**: the upper edge for inputs, with the card set at the foot of
the drawing area; the lower edge for outputs, with the card set at its head; a card with both gets two boxes. Its
supply pins are on its left side. Today it draws two rows of pins two steps apart, so half the channels face
away from their circuits and nothing fits over a pin. A card is `pins × CARD_PIN_PITCH` wide plus two steps and
six steps high; a card wider than the drawing area is split into boxes of at most 8 pins, and the command asks
for the page of each further box. Stretching a box already spreads its pins at one common pitch. Cards already
drawn are not changed. The pin texts stay as the program
draws them on a box.

In the same step the starter symbols get the first places of §5.2 as their slots (tag at the upper left, make
and type under it, pin numbers right of the pins), and the interruption point and supply mark get their name in
line with the wire, so that marks stack two steps apart.

*As built (step 7).* A card is a device of the station whose part's pin map gives its channels pins: inputs are
digital and analogue inputs, counters, IO-Link and multi-function ports; outputs are digital and analogue outputs.
Its box is `pins × CARD_PIN_PITCH` and two steps wide, the row centred (half a step to the left), `CARD_HEIGHT`
high; its other pins stand down its left side `MARK_PITCH` apart, the lowest that far above the lower edge of a card
of inputs and the highest that far under the upper edge of a card of outputs. The CPU and any device without
channel pins keep their box; so does a box a person draws (Place here). The other boxes stand from the next free
place, in slot order; then each card's box takes the first place that keeps a gap from what the page draws, a card
of inputs as near the foot as there is room and one of outputs as near the head, each line from the left; on its line a
card of inputs takes the first place beside the boxes drawn with it that are no card (the controller's), a gap off,
before one under them (FT-1894, 0.3.5), so the supply wires from the head cross none of its channels; a card that
finds no room waits for another page, which a toast and the To do list say. A row wider than the drawing area takes as
many pins as fit, `CARD_PINS_MOST` at most; "asks for the page" is the station view's To do list, which offers the
rest on another page, and a toast after the draw says so. A card whose part lays out its device box (doc 07 §2.5,
FT-1788: a person placed its pins in the box) is one box as the part lays it out, placed as a card of outputs is; so
is it placed by Place here, with one click. `plc.draw_station` draws a box of a card whose black-box
function a page draws already as another drawing of it when none of its pins is drawn yet, which also gives a card
with inputs and outputs its two boxes in one step. The starter symbols that have a pin leaving upward (not the
terminals, the earth symbols and the valve solenoid) carry the tag right-aligned one millimetre left of their
graphics with its foot one step under the upper pin, then the PLC address, make, type, rating, function text and
cross-reference line under line 6 mm apart (a coil's contact list last): 4 mm on the QET grid, which scales places
and not texts, as in the reference set; there the lines are one block, on the native grid each line is placed alone. The interruption point's two forms keep their cross-reference under the
arrow: under the wire on the wire's side, a wire that turns at once (a supply mark's drop) ran through it. The text placer gained two places for this: a part's right corners stand beyond its pin
numbers, and a mark's cross-reference may stand centred under or over its arrow.

*The pitch, changed on 2026-11-20.* The pitch was 5 steps, with the advice to stretch a card to 10 where devices
stand over its pins. On a page with field devices that was the commonest case and the box stood in the way: a
sensor with its tag and address is six to seven steps wide, so at 5 steps no row of devices can stand in line
with its channels, and every wire steps. At 8 steps a device stands over each pin, and a card of eight channels
(66 steps) still fits beside the controller's box at the head of an A3 sheet. `CARD_PINS_MOST` went from 16 to 8
with it: a row that does not fit is drawn a byte a box, 8 and 8 for a card of sixteen channels, not 12 and 4.
The price: a card of sixteen channels no longer fits one A3 page, and a card of inputs, which is set at the foot
from the left edge, finds no room on a page where a symbol stands within two gaps of the foot (the To do list
then offers another page). One thing the box still does that a draughtsman would not: the pins of a card at the
head and one at the foot stand on columns two steps apart where the controller's box is not a whole number of
pitches wide. The command still draws them so; Arrange and route steps the card aside (§4.4), for a card the person
drew as well. (A card of inputs under the controller's box, its first channels crossed by the supply wires from the
head, stands beside that box since FT-1894, above.)

### 7.6 Joining wires: the bevel by default

A wire that ends on a wire is joined in the **bevel form** by default: in the Wire tool, in the interruption-point
tool, and wherever a command joins a wire to a wire. The dot is there on request: a key while the tool's end is
on a wire, and the Form row of the junction's Properties (`junction.edit`).

**What this changes in the wire list.** The form of a junction says how the shop wires
([06](06-schematic-editor.md) §3.5). A dot is a star: every branch is a wire of its own from the terminal where
the potential arrives, so that one terminal carries them all. A bevel is a chain: the wires run from terminal
to terminal in the junction's sequence, two on each terminal between the first and the last, and the bevel shows
which terminal carries two (the one it leads into). So a tap drawn in a new drawing gives as many wires as
before with other ends: a potential looped from device to device, where the star gave several wires on the
feeding terminal. The junction's Properties show the wires it produces, as today, and the sequence can be
reordered there.

**Drawings made before keep their junctions as stored.** The default is written into a junction when it is
made; it is never read into a junction that exists, and a junction stored without a form reads as a dot, as
before. Importers keep what their source says. The layout draws both forms and changes neither (§2.3).

---

## 8. What goes

Delete before you add: there is one router, one text placer and one measure when the build is done.

| Goes | Replaced by | In step |
|---|---|---|
| `ew_derive::routing::measures` and its tests | `ew_drafting::measure` | 1 |
| The scene's text steps in `ew-scene/src/page.rs`: black-box captions, cross-references moved outside, tags off their own mark, `place_tags`, tags by their own symbol, `place_labels_clear`, `place_slot_texts`, `place_point_xrefs_off_own_wire`, `place_xrefs_off_wires`, and the derived places of wire numbers and cable tags | `ew_drafting::settle_labels` | 1 |
| `PageGeometry`, its text boxes without kind or owner, `Session::page_geometry` | `DraftPage`, `Session::page_draft` | 1 and 2 |
| `ew_derive::routing::plan` (`route_page`, the two routing orders and their comparison, the rip-up rounds, mate groups, the 8/5 rule for texts, `over_texts`, `Faults`) and `plan_tests` | `ew_drafting::route` | 2 |
| `ew_derive::routing::search` in its place (kept as code, moved, reading the occupancy) | `ew_drafting::search` | 2 |
| The actions `route.page` and `route.reroute`, their texts, tooltips, cards and tests | `layout.route` | 2 |
| `connection.route_many` | `layout.apply` | 2 |
| The mate discount (a step beside a mate at half price) | tracks | 3 |
| The pinned routes of the reference project and its layout file, with the test that compares them | the fixtures of §9 | 2 |
| The six places that read points "settled" outside the router | the grid's own number at the source (§2.1) | 8 |
| ADR-0033's decisions 4 and 5 and its amendments 1 to 5 | [ADR-0034](../adr/0034-schematic-drafting.md) | with this document |
| The dot as the form of a new junction | the bevel (§7.6); the dot on request | 2 |

**Kept from ADR-0033:** a stored route is the drawing and no reader routes; a route is the full vertex list; an
unrouted wire is a dashed line and is reported; commands store and never route; the two forms of the interruption
point; connecting on a move.

**The checks.** EW-CORE-067, -068, -069, -070, -072 and -073 keep their numbers and meaning and read the new
measure; -068 allows a shared run. Three are added with the next free numbers of that family: "Wires meet without
joining" (F6, a warning; it was a hard rule of the router that no check reported), "Text on a text" and "Text on
a part" (C5, hygiene warnings). EW-TERM-006 is unchanged.

*As built (step 1).* Four are added, F7 among them as §1.2 asks: EW-CORE-074 "Wires meet without joining",
-075 "Text on a text", -076 "Text on a symbol" and -077 "Symbol on a symbol" (a conflict warning), with the rows of
[15](15-checks-revisions-collaboration.md) §4.2. Until step 2, -068 allows the common beginning of §1.2's note only.

*As built (steps 2 and 8).* The rows of this table marked 2 and 8 are gone (step 8: §2.1). A shared run is that common beginning (a run from
the pin of both wires while both go the same way), so -068 needs nothing more. `ew route` keeps its name and now
stores `layout.apply`.

### 8.1 What is left out

- **Turning or mirroring a part,** spreading a circuit over the sheet, writing headings.
- **Proposing interruption points** for a wire with no good way (D-5): the plan names the wire; the person
  splits it.
- **Moving a part to another page,** or making pages.
- **Settings** for the numbers of §1.5, and stored places for the program's texts (§5.5).

---

## 9. Proof

### 9.1 The pages

1. **Seven fixtures in the repository,** built in code in `ew-drafting`'s tests after the kinds of §1.4, with
   invented devices: supply and power (three feeders under four rails that run from marks at the left to marks
   at the right), control circuit (four paths between two trunks, with a holding branch and contact lists), PLC
   inputs (a card row at the foot, eight signals from a staircase of marks, and the same with four devices over
   terminals), PLC outputs (a card row at the head, eight loads over one trunk), bus layout (a line of four
   stations and a switch), pneumatic circuit (a valve terminal with two actuators and a service unit),
   distribution (two terminal strips with stacks of marks). Each exists in two states: **untidy** (the parts shifted off their lines by a fixed pattern, two of them on the wrong side
   of their pin, every wire unrouted) and **as wired by hand** (parts in place, routes drawn as a person would).
   Beside them, small pages that each state one property of a kind (§9.3 row 4): a strip with scattered marks, a
   potential wired from pin to pin, a card with field devices through terminals and a supply terminal, a valve
   terminal with two cylinders through flow controls, a part wired by a hairpin.
2. **A full sheet:** generated in the test: twelve feeders and forty control paths on one A3 page, about 300
   wires.
3. **The private pages,** measured outside the repository by whoever holds them: the pilot user's supply page,
   and the pages of the person-like test runs of each kind. Their figures today and the figures to reach are kept with the build plan, without their names.

### 9.2 The measures, before and after

For each page `ew layout` prints faults, unrouted wires, cost, crossings, bends, wires straight, turn-backs,
detour and labels moved. The build is judged on:

- every fixture, untidy, after Arrange and route: no fault, no unrouted wire, no turn-back, no wire across a
  text, no text on a text; every wire between two pins that face each other is straight; every net with three or
  more taps is a trunk with straight drops;
- every fixture, as wired by hand, after Route: an empty plan (nothing better is found), and after Arrange and
  route: an empty plan;
- the full sheet: Route and Arrange and route end, with no fault;
- the private pages: no page worse in any figure, and the targets kept with the build plan.

### 9.3 Tests that fail without each step

No test pins the total of a growing set, a generated name or a path; each states a property of a small page.

| Step | A test that fails without it |
|---|---|
| 1 | Two parts whose tags' first places overlap: both tags are clear. A wire across a tag's first place: the tag stands at its second. A moved text and a box's pin texts are where they were. The measure counts a turn-back on a wire drawn round a caption. The name of a mark stacked under another stands in line beyond its arrow, and a mark alone keeps its author's place |
| 2 | A potential wired from pin to pin over three parts is one line with two drops. A T junction moves onto the trunk and keeps its three wires and its sequence. A wire the person shaped is in no page-wide plan; a broken one is, flagged. The same page with its lists shuffled gives the same plan. A plan never changes which pin a wire ends at. A junction with two drops stands over one of them and its net has no fault. A tap may stand on a line across between a pin and the pins that face it |
| 3 | Two-pole device over a two-pole device of another pitch: one pole straight, one step, no crossing. Five sensors over pins two steps apart: five wires step in and none crosses |
| 4 | A sensor below a card whose pin faces up stands above it after the run. A circuit that stands above the trunk that feeds it stands below it after the run; an actuator below its valve stands above it. Two circuits whose drops cross change places, two whose drops do not cross keep theirs. A part with no wire does not move. `rules_kept` holds on every plan. A second run gives an empty plan. Unticking one part gives a plan in which it has not moved and nothing overlaps. One Undo restores every place and route. The marks of a strip, scattered, stand in stacks and their wires nest: one corner each, none crossing. The parts a potential feeds from pin to pin stand level under one rail. Field devices stand in a row over their terminals, the terminals in line over their pins, the supply's riser beside the row: no crossing. On a valve terminal the cylinders stand above in the valves' order, the service unit below, no tube crossing. A part wired by a hairpin stands at its host, off the pin's line. A strip is one row for A4, a hairpin is not bound by E4, and a chain may take the chain gap and open by a row's pull and no more. When every row of the preview is unticked the toast says that nothing is left to apply |
| 5 | A locked part is in no plan; it is a host for the others |
| 6 | A pointer 0.7 step from an In-line guide's line puts the part on it; 0.8 step does not; with Alt neither guide nor nudge |
| 7 | An input card drawn from the station view has every channel pin on its upper edge, `CARD_PIN_PITCH` apart. Two marks of the starter library stack two steps apart without their texts meeting |
| 8 | On a page of the QElectroTech grid a pin's point equals `Grid::at` of its position |

### 9.4 A person-like test run

A run is told, in a person's words and with no word about the router: "Draw this supply page and this input page
from the list of devices and connections. Put the parts roughly where you think they belong; do not line them up.
When a page looks untidy, use what the program offers to tidy it. Say what you expected each button to do before
you press it, and afterwards whether you can find every device again." The run is judged on whether it finds the
two buttons without being told, whether it accepts the preview, how many parts it then moves by hand, and the
page's figures at the end.

### 9.5 What the cost grows with, and where the work is bounded

Not measured: no timing test goes into the repository.

- The text placer: labels × places × the things near a place (a grid of buckets over the page).
- The placer: one run of the router over the page for every plan it weighs. A run weighs the whole arrangement,
  at most `BRANCHES` trees, at most twice `BRANCHES` plans without or with one branch, and at most `BRANCHES` more
  for the price; a press makes at most `ROUNDS` runs. Setting the arrangement: relations and a sort per row; for
  each tree at most `ANCHORS` anchors, each with at most 4 × `REACH` steps of its root; the text placer once per
  row and per pitch of a stack, on a page of those parts alone; the trees are set again for each unit found in the
  way, `PASSES` times at most. On a page where a card would step along its edge the whole run is made three times
  (§4.4). So the work of a press grows with the wires of the page times a bounded number of
  plans, and with the number of trees times the steps tried for each.
- Trunks: nets × candidates (a handful) × taps. Tracks: per channel a sort of its wires.
- The search: per remaining wire, the nodes of its area (at most `MAX_NODES`, five states each) times a
  logarithm; areas grow from the ends' box only when no route is found; `PASSES` passes.
- The occupancy is built once per run and changed per wire, not rebuilt per wire.

On a full sheet the search has the most work; the straights, trunks and tracks take most wires away from it,
which is the difference to today's solve, where every wire is searched against every other in two orders.

---

## 10. The build, in steps

Each step leaves the program working and better than before; its brief is in the build plan. This document and
ADR-0034 are in the repository before step 1, so that every step cites them where they stand.

| Step | Delivers | Deletes | Needs | Size |
|---|---|---|---|---|
| **1 Texts settle** | The crate, the model in ticks, the measure, the one text placer in the scene; `ew layout` and the checks on the new measure, with the three new checks | The scene's text steps; `measures.rs` | — | Large: about 3 500 lines with tests |
| **2 Route** | Nets, shared runs, trunks, the search on the occupancy, texts and routes together, `computed`, `layout.apply`, the Route button, the Wire tool and the move on the new core, the bevel as the form of a new junction | `plan.rs`, `plan_tests`, `PageGeometry`, Route page, Reroute, `route_many`, the pinned reference routes | 1 | Very large: about 4 500 lines |
| **3 Tracks** | Channels and track assignment for stepping wires | The mate discount | 2 | Medium: about 900 lines |
| **4 Arrange and route** | The placer with the rules of §1.6, the preview with its rows, planning again on a tick, the second button. The placer was built twice: first with an estimate that ranked candidate places without routing, then anew (§4, as it stands) with every plan routed | The estimate, its stages and its constants (`COLUMN_GAP`, `BAND`) | 2 (3 for the room of §4.5; without it the chain gap alone) | Very large: about 4 500 lines, and about 2 800 for the placer anew |
| **5 Keep in place** | `Representation.locked`, the command, the glyph, the rows of the preview | — | 4 | Small: about 500 lines |
| **6 Guides** | Guides and the nudge in the placing tool and the Select tool | — | 1 | Medium: about 900 lines |
| **7 Cards and first places** | The station view's cards as one row of pins; the starter symbols' slots and the marks' name at the first places of §5.2 | The two-row pin layout of drawn cards | 1 (4 to see it arranged) | Medium: about 900 lines and the symbols' data |
| **8 Records** | The grid's own number at the source; the sections of doc 06 and doc 15 brought in line; the guide pages | The settled comparisons | 2 | Small: about 500 lines, mostly deletions and documents |

Side by side: step 6 beside step 2 (it touches the tools' pointer handling and one pure function); step 7 beside
step 5; step 8 after step 2 at any time. Steps 2 and 4 are the long ones and are not split: each is one design
that only works whole.

**Libraries.** None is added. `petgraph`, `pathfinding` and `rstar` would fit by licence, but the page is small,
the search exists, and a bucket grid serves the text placer; `rust-sugiyama` is not used because the placer is
not a layered layout. Nothing is ported from libavoid, ELK or OGDF.

---

## 11. Where this design is unsure

1. **Texts derived, not stored** (§5.5). It is the lean choice and matches today; it means a text can stand
   elsewhere after an upgrade of the placer. If stable texts turn out to matter more, the stored form is a later
   step that adds a member and changes no rule here.
2. **Shared runs** (§3.4). They follow the reference drawing's notation and make pin-to-pin potentials readable,
   but a reader must take the bevel as "two wires part here". If that is misread, the fallback is hops on their
   own tracks, which the tracks of §3.5 already give.
3. **One arrangement and its parts are weighed, not every arrangement** (§4.7). The placer proposes one
   arrangement from the pins and routes it, its trees, and it without or with one branch. Where a page would gain
   only by two changes that each lose alone, or by an order of its circuits that the trees' steps do not give, the
   plan is missed and Route's is offered: a good arrangement can be missed, never a bad one taken. The price of a
   move (2 per part) is a first value; it counts a part, not its distance. A page wired against its symbols
   (§4.8) gets Route's plan and no move; wires that have to go round their own card are the router's to nest,
   which it does not do yet: they cross where a draughtsman would nest them.
4. **The weights** (§1.2) continue today's ratios. The text weight of 12 replaces a rule of thumb (8/5) with one
   number; the private pages named in ADR-0033's amendments are its test.
5. **The rules of §1.6 are few on purpose.** They say what binds on every page of a kind. What a draughtsman
   also does (the loads at the foot of the sheet, like circuits drawn alike) is left to the measure and the
   person; a rule is easy to add when pages show the need. A part whose role the adapter cannot tell is bound
   by the general rules only.
6. **No readability study of circuit diagrams exists** (research §9): the ranking of crossings, bends and length
   comes from general graphs and from the trade's practice.
7. **Bevels by default** (§7.6) make every joint of a new drawing a chain in the wire list. It is the pilot's
   choice and the trade's notation; a user who wires in stars asks for the dot each time, until a setting is
   wanted.
8. **The reference set is one hand.** The defaults taken from it (two steps between wires, five between devices,
   five between card pins) are constants, and a second user may want settings.
