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
   this rule and the trunk rule below. The spacings (two steps between wires and between rails, five between the
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

**Compared** in this order: fewer faults, then fewer unrouted wires, then lower cost. `Quality` is that triple
with the counts behind it.

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
| A tap of two wires on one pin | the arriving wire turns into the drop; the wire that goes on leaves the drop one step from the corner by a bevel; no dot | E, D-8 |
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
| **Control circuit in current paths** | Each path is a column between the two trunks; the contacts of all paths on one line, the coils (or lamps) on one line; a parallel branch in its own column beside the path. The contact list of a coil stands left of the coil under its tag; the path's function text under the lower trunk. | The upper potential along the head, the lower one along the foot, each from a mark at the left; a trunk ends at its last path by a corner. | Paths `COLUMN_GAP` clear of each other; the last parts' lower pins on one line when within 6 steps of it |
| **PLC inputs** | The card is a row of channel pins along the foot of the sheet, pins up. Signals from other pages arrive from marks, each mark 2 steps before its drop and 2 steps higher than the one before (a staircase), so the drops do not cross. Devices on the same page stand in line above their terminal and pin. | The devices' common potentials run as trunks between the devices and the card, or beyond the devices. | Card pins 5 steps apart (§7.5); a card stretched to 10 steps takes a device over each pin |
| **PLC outputs** | The mirror: the card along the head of the sheet, pins down; each load in line under its pin; the loads on one line near the foot; tags and contact lists under the lower trunk, in two rows where the pitch is tighter than a tag is wide. | The loads' return potential is one trunk under the loads, from mark to mark. | Card pins 5 steps apart |
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
| `BODY_GAP`, `WIRE_GAP`, `STUB` | 1 step each | router: the least distances |
| `TRACK_PITCH` | 2 steps (1 where a channel is too narrow) | tracks, parallel wires |
| `CHAIN_GAP` | 5 steps (never less than 2) | placer: between facing pins of two parts in line |
| `TRUNK_GAP`, `TRUNK_PITCH` | 2 steps each | router: a trunk from the pins it taps; trunks side by side |
| `MARK_PITCH`, `MARK_LEAD` | 2 steps each | placer: marks in a stack; a mark before its drop |
| `ROW_GAP` | 1 step | placer: between two footprints in a row |
| `ROW_PULL` | 6 steps | placer: a part joins a common line when within this |
| `COLUMN_GAP` | 6 steps | placer: between two groups set side by side |
| `MOVE_PRICE` | 2 per part | placer: what a move has to earn in the estimate (§4.7) |
| `BAND` | 3 steps, and `TRUNK_PITCH` per trunk | placer: the head and the foot of the sheet, kept for trunks and their marks |
| `EARTH_REACH` | 8 steps | placer: an earth symbol from the pin it earths |
| `LABEL_CLEAR` | 2 ticks (1/8 step) | text placer |
| `WINDOWS` | 4 steps, 16 steps, the drawing area | search areas round the two ends |
| `PASSES` | 2 | improvement passes; text rounds |
| `MAX_NODES` | 262 144 | search area bound (kept) |
| `GUIDE_SHOW`, `GUIDE_CATCH`, `GUIDE_CATCH_PT` | 2 steps, 3/4 step, 8 screen points | guides (§6) |
| `CARD_PIN_PITCH` | 5 steps | cards drawn from the station view (§7.5) |
| `CARD_HEIGHT`, `CARD_PINS_MOST` | 6 steps; 16 pins | cards drawn from the station view (§7.5) |

### 1.6 The rules that bind the placer

Arrange and route may move a part anywhere on its page. Two things hold it: the **rules** of this section, which
no plan breaks, and the **measure**, which decides everything the rules leave open. A plan sets every part it
moves by the rules; a part that stands against a rule is set right where the sheet has room, and left and named
where it has none.

| | Rule (never broken by a plan) |
|---|---|
| **Every page** | A1. Every part inside the drawing area; no part on a part. A2. A part that is kept in place, outside the scope or inside a structure box does not move. A3. No part is turned or mirrored, and none leaves its page. A4. The parts of a row stand in the order of the pins they are wired to; the parts of a circuit in the order of the wiring. A5. The spacing along a circuit is the person's: it is neither closed up nor spread; only a part that has to move takes the chain gap. |
| **Electrical pages** (energy and signals flow down, potentials from left to right) | E1. Current runs down a circuit: of two parts wired through a pin that faces down and a pin that faces up, the first stands above the second. E2. Potentials at the head and the foot: a trunk that feeds lies above every part of the circuits it feeds, a trunk that returns below every part that returns to it, and no part but a mark stands above the highest feeding trunk or below the lowest returning one (the bands). E3. A potential or signal enters at the left and leaves at the right: an incoming mark stands at the left end of its wire or trunk, an outgoing mark at the right end. E4. A card's channels stand in channel order, each channel's parts in the channel's column on the side its pin faces, and no part of another circuit between a pin and its channel's parts. E5. An earth symbol stands within `EARTH_REACH` of the pin it earths. |
| **Fluid pages** (energy flows up) | F1. The pressure source and the service unit stand lower than every valve, and every actuator higher than the valve that drives it. F2. The actuators keep their order from left to right (the order of operation is the person's). |
| **Bus pages** | B1. The stations of a line stand in the order of the line, the controller first at the left. B2. Stations with their ports on the same side stand level. |

Weighed by the measure, and so free to change where the page gains by it: the order of independent circuits
across the sheet and the place each takes; on which side of its host a part stands when both sides keep the
rules; whether a mark stands at the sheet's edge or near its drop; the track of each trunk within its band; the
place of a part that has no line to take.

The rules are checked on a plan by one function, `rules_kept(before, after)`, beside the measure; the placer's
tests use it on every plan.

---

## 2. The model of a page for layout

### 2.1 Units: grid ticks, never nanometres

The layout computes in **ticks**: 1/16 of a grid step, as `i32`. A grid position is a multiple of 16, a half-grid
position a multiple of 8, and a text box is rounded outward to whole ticks (0.25 mm on a native page). The adapter
in `ew-api` converts the page into ticks once and a plan back into nanometres once, through `ew_units::Grid`.
Inside the crate no nanometre exists, so two numbers for one grid position cannot meet.

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

### 2.2 What the layout sees

```rust
pub struct DraftPage {
    pub kind: PageKind,             // Electrical | Fluid | Bus: which rules of §1.6 apply
    pub area: Rect,                 // the drawing area
    pub keep_out: Vec<Rect>,        // the frame's bands, the title block
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

### 2.7 Where the code lives

A new crate **`ew-drafting`** (GPL-3.0-or-later; it depends on `ew-units` and `ew-ids` only): the model, the
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

### 3.2 Jobs

`route(page, jobs) -> Plan`. The caller names the wires that may be routed:

| Command | Jobs |
|---|---|
| Route, nothing selected | every wire of the page that is unrouted, has a fault as stored (F1 to F6), or is `computed` |
| Route with a selection | the selected wires and every wire with an end on a selected part, whoever shaped them |
| Arrange and route | every wire with an end on a part that moves, and the jobs of Route for the same scope |
| A move by hand | the wires the move leaves unrouted or with a fault, or with more bends than before (as today) |

A route the person shaped (not `computed`) that has a fault is a job of a page-wide Route too, but it is flagged
**his**: the plan lists it apart, the command shows a preview instead of applying at once, and its row is ticked
because the wire is broken; he can untick it (§7.2).

Every other routed wire is **fixed**: an obstacle with its own right of way. A junction may move when every wire
at it is a job.

### 3.3 The occupancy

One structure, built once per run over the half grid of the drawing area and updated as wires are laid and
lifted: per node whether it is inside a body grown by `BODY_GAP`, a pin, a pin's stub, a junction, a vertex of a
wire; per move between two nodes which wires run along it and which cross it; per node the labels and fixed texts
over it. The search and the three structured phases read it. It replaces the per-request preparation of today's
search, which rebuilt the page for every wire.

### 3.4 Nets, trunks and shared runs

**A net job** is the set of job wires joined through movable junctions, or through a pin that two or more of them
end at. Its **taps** are those junctions and those pins. A wire with no tap at either end is a net of one wire.

**Shared runs.** Wires that end at one pin may run on one line from that pin, as long as they go the same way; where
one turns off, the others run on. This is the common beginning that F3 allows. The scene draws the parting as the
trade does for two wires on one terminal: the wire that arrives along
the trunk turns into the drop by a square corner, and the wire that goes on leaves the drop one step from that
corner by a 45° bevel; there is no dot. (Today the scene draws this bevel only where two wires of one pin part
after their first step.) It is how a potential that is wired from pin to pin, the usual way in this program, is
drawn as a rail with drops instead of a staircase of rectangles. A part dropped on a shared run is refused in words ("two wires run here; place the part where they have parted").
A junction of the bevel form is drawn the same way, and the router sets the way it points from the routes that
meet at it.

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
3. *Each tap's place* on a candidate: the coordinate of the pin it drops to, so that the drop is straight. Two
   taps that would fall on one point (two pins in line on either side of the trunk) are set one step apart: two T
   junctions, never a four-way one (D-7).
4. *Valid* when the taps then lie along the trunk in the spine's order, no trunk segment or straight drop has a
   fault in the occupancy, and no tap falls on a crossing.
5. *Chosen:* the valid candidate of least cost (the trunk's length, each drop's length, 4 for each bend a drop
   or an end still needs, 6 for each wire it crosses), ties to the line the taps lie on now, then horizontal, then
   the smaller coordinate.
6. *Laid:* the trunk segments and the straight drops go into the occupancy; a drop that cannot be straight is left
   for the later phases, with its tap fixed.

A net with one tap has no trunk: the tap is tried at each crossing of a grid line through one end's exit with a
grid line through another's (at most nine points) and at its present place, and the cheapest point with three
arms wins, ties to the present place.

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
distance and two bends. The order of trunks is the supply rank, then the first tapped pin's place (left first),
then more taps first, then the ID. The shared run is part of the search (§3.6): a wire of a pin starts, or ends, on
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
be. No mate or bundle price was left in the search to delete.

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
shared run); onto another wire's vertex; a turn back; a move that would end a wire on a crossing.

Areas: the box of the two ends grown by 4 steps, then by 16, then the drawing area; the first area with a route
wins. **The half grid** is searched last and only between close pins, as today: where the whole grid has no route
or one more than 8/5 of the city-block distance of the ends, the search runs on half steps (bodies grown by half a
step, a half-grid track only between the two ends), and its route is taken where the whole grid's is more than
8/5 of it or there is none.

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
among the new routes of the others, keeps the stored ones. So a wire that already runs as well as the rules allow
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

---

## 4. Placing

### 4.1 The rule

**Put each part where the page is best for it, within the rules of a drawing of its kind.** That is almost
always: in line with the pin it is wired to, on the side that pin faces; the circuits side by side in the order
that crosses least; the potentials at the head and the foot. A part may cross the page for it. The spacing along
a circuit stays the person's.

*Best* is the measure of §1.2 on the arranged and routed page. *The rules* are those of §1.6; they are not
weighed.

It is what a draughtsman does by hand, and it needs no template: a breaker's lower pin faces down, so the
contactor stands below it; a card's input pin faces up, so the terminal and the sensor stand above it; a valve's
working port faces up, so the cylinder stands above the valve.

*Alternative: a template per kind of page (rails here, eight channels there), as page macros in other tools.* It
needs more than the type of a page says, fails on mixed pages, and would recast a hand-made page into the
template's shape. *Alternative: a layered layout (Sugiyama, ELK Layered) of the whole page.* It draws a new page
from the netlist and does not keep what the person arranged; its crossing reduction is not needed where a part's
place follows from a pin. *Alternative: a general constraint solver for alignment and separation.* The
constraints here form rows on a tree, which a one-pass method solves exactly (§4.4). *Alternative: only line
parts up and make room, every large part left where it stands* (this design's first form). It leaves a device on
the wrong part of the sheet, and the pilot user chose the full move. *Alternative: an upper bound on the parts
one run moves, or a second mode that only lines up.* A bound would leave a page half arranged at an arbitrary
cut, and every run is previewed, which is the asking; a second mode would be a third command, and the selection
and the preview's ticks give its result where it is wanted.

### 4.2 Units, relations and anchors

- **Units.** Parts with one `unit` number move as one: a valve terminal with its valves; parts the adapter finds
  inside a `frame_only` part. Every other part is its own unit. A unit is **fixed** when it is locked, outside
  the scope, off the grid, or inside a structure box.
- **Relations.** Two units are related when a wire joins a pin of one to a pin of the other, directly or through
  one tap. Through a net with a trunk there is no relation between its pins; each tapped unit is related to the
  trunk (§4.6). A relation has a **kind** from the two pins' sides:

  | Kind | Sides | Best drawing | Where the second unit belongs |
  |---|---|---|---|
  | **Facing** | opposite | straight, no bend | beyond the first pin along its side, the two pins on one line |
  | **Corner** | at a right angle | one bend | beyond the first pin along its side, and the first pin beyond the second along the second's side |
  | **Level** | the same | two bends (out, across, back) | beside the first, the two pins level; where no room is beside, in a row beyond the pins, each offset by its own width so that the wire passes it |
  | — | a pin without a side | — | no place follows |

- **Weight.** The number of a unit's pins that have a wire, times 4, plus its body's area in square steps over
  16. **Anchors:** every fixed unit, and in each group of related units without a fixed one, the heaviest (ties to
  the upper, then the left one, then the ID).
- **Hosts.** From the anchors outward, breadth first over the relations: a unit's **host** is the related unit
  one step nearer to an anchor; among several, a Facing relation before a Corner before a Level one, then the
  heavier host, then the relation that needs the smaller move, then the pin order. That relation is the unit's
  **lead**. The hosts form a tree under each anchor. A movable anchor is the root of its tree; a tree with all that hangs on it is a **group**, and §4.7 says where
  a group stands.

### 4.3 A follower's place

For a follower with its lead (host pin *h* with side *s*, follower pin *f*):

- **Across** (the coordinate at a right angle to *s*): Facing wants *f* on *h*'s line. Corner and Level keep the
  present coordinate when the relation's drawing is possible there, else take the nearest coordinate where it is.
- **Along** (the coordinate along *s*): the follower stays at its present distance when it is beyond the host by
  at least 2 steps and its footprint is clear of the host's; else it stands `CHAIN_GAP` (5 steps) beyond, further
  when a label of either or a channel's tracks need the room (§4.5). **A part is never pulled nearer, and a chain
  is never spread out**: how a path is spaced over the sheet is the person's.
- **On the wrong side** (the follower stands behind the pin that faces away from it): it moves to the side the
  pin faces. This is the one large move the placer makes, and the preview shows it.

The **footprint** of a unit is its bodies, one step along each wired pin, and its labels at their first places;
for a host, also the footprints of its followers once they are placed.

### 4.4 Rows and stacks

The followers of one side of one host are placed together.

- **A row** (Facing and Level followers): across, each wants its pin's line; they keep the order of the host's
  pins; their footprints stay `ROW_GAP` apart. The places are the solution of least total move from the wanted
  lines under that order and those gaps: neighbours that would overlap are merged into a block, the block stands
  at the median of its members' wishes, and merging repeats until no two blocks overlap (pool adjacent
  violators; exact for this problem, one pass, a stack of blocks). Followers narrower than the pin pitch stand
  exactly in line; wider ones form a row centred on their pins, and their wires become the stepping wires of
  §3.5. Along, the row has one line: the largest distance its members need or keep, for members within
  `ROW_PULL` of it; a member further out keeps its own.
- **A stack** (Corner followers that turn the same way; mostly marks): they are ordered along so that their wires
  nest: for wires that enter pins from one hand, the pin furthest from that hand takes the outermost place. Their
  pitch is `MARK_PITCH` (2 steps), more where a footprint needs it. Across, each keeps its coordinate when it is
  at least `MARK_LEAD` (2 steps) before its drop (a column of marks at the sheet's edge stays a column, with
  nested wires; marks that stand near their pins stay near them and form a staircase); a mark nearer than that,
  or beyond its drop, is set `MARK_LEAD` before it.
- **A mark at the end of a trunk** stands on the trunk's line: it moves along the trunk's cross axis only.

The order within a row or a stack follows the host's pins, not the places the followers had. Groups keep the
order the person gave them across the sheet unless a change of places pays for itself (§4.7).

### 4.5 Room between a host and a row

The distance between a host's pins and a row is at least `CHAIN_GAP`, at least what the labels between them need,
and at least one step more than the tracks the row's stepping wires need (counted with the assignment of §3.5 on
the wanted places). So a row is set far enough for its wires before the router runs.

### 4.6 Trunks

A unit tapped by a trunk and without a lead of another kind takes the trunk as its host: the units tapped on one
side of one trunk form a row with one line (their tapped pins level), across they keep their places and order. A
unit that has a lead keeps its place across from the lead; when it is also tapped by a trunk on its far side, it
joins that side's line if it is within `ROW_PULL` of it (the coils of a control page on one line). The trunk's
own coordinate is the router's (§3.4): it follows the rows.

### 4.7 Where a group stands

After the places inside each group are set (sizes from the leaves up, places from the root down), each movable
group is placed as one block.

1. **The estimate.** Choices are compared by an estimate of the measure that needs no routing: for every wire of
   the scope the city-block distance of its pins; 4 for each bend that the sides and places of its pins make
   unavoidable (none for a facing pair in line, one for a corner, two for a step or a hairpin, four for a pair on
   the wrong side of each other); 6 for every crossing among the wires' plain shapes (the straight, the L, the
   step at its middle track); and `MOVE_PRICE` for every part that does not stand where it stood. The price is
   not part of the measure: it is what a move has to earn, so that a part moves only where the page gains more
   than the move costs, and never for a tie.
2. **The bands** (rule E2; on a fluid page rule F1 in the same way). The head band is the stretch at the top of
   the drawing area that the page's feeding trunks need (`BAND`), the foot band the same for returning trunks. A
   group is moved along the flow, whole and by the least distance, so that none of its parts but a mark stands in
   a band or beyond a trunk it taps. Otherwise a group keeps its place along the flow: a circuit is not pulled
   up under its trunk, its drop is as long as the person left it.
3. **The place across.** Fixed groups stand. The movable groups, heaviest first, each take the place of least
   estimate among: where it stands; directly beside each group it is wired to, left and right, `COLUMN_GAP`
   clear; and the first free place from the left. Then two neighbouring groups change places where that lowers
   the estimate. Both are done at most `PASSES` times. A group without a wire to another group keeps its place.
4. **Fitting.** A group that sticks out of the drawing area is moved back in by the least distance. Of two
   groups on each other, the later (fixed first, then heavier, then upper left) takes the next free place across.
   A branch that still has no room on the side its pin faces goes back to where its parts stood.

A part that goes back is named in the plan: "no room below -F2; left where it stands". The placer does not close
up or spread a circuit, does not turn a part to make it fit, and does not move a part to another page.

### 4.8 What keeps a page recognisable

Parts may cross the page, so these hold the person's picture of it:

- **The preview.** Every move is shown before anything is stored: the page as it would become, each moved part
  tinted, its old place outlined and joined to the new one. The card says how many parts move and lists first the
  ones that move further than a third of the sheet. Unticking a part plans again around it (§4.9).
- **Keep in place and the selection.** A part marked to stay stays; with a selection only the selected parts
  move.
- **The price of a move.** No part moves for a small gain or a tie (§4.7), so a page that is already in order
  comes back with an empty plan, and a second run after a first changes nothing.
- **Order.** The parts of a circuit stand in the order of the wiring, the parts of a row in the order of the
  pins; circuits keep their order across the sheet unless a change of places removes a crossing or a long wire.
- **Spacing.** Along a circuit the distances are the person's.
- **The rules of the kind.** After a run a device stands where a draughtsman would look for it: the potentials
  at the head and the foot, a load under its protection, a field device at its channel, an actuator over its
  valve.

There is no upper bound on the parts one run moves, and no lighter mode, for the reasons of §4.1.

### 4.9 The plan

`arrange(page, movable, jobs) -> Plan`: the units' new places; then `route` on the moved page with the jobs of
§3.2. A plan is offered only when `rules_kept` holds (§1.6) and the never-worse rule of §1.2 holds for the whole
plan with F7 included: when the arranged and routed page is not better than the page as it is, the plan is empty
and the message says that the page is already arranged as well as the rules allow.

**A selection** makes only the selected parts movable; everything else is fixed and can be a host. A selection of
one feeder arranges that feeder and leaves the page.

**In part.** Unticking a part in the preview adds it to the fixed units and plans again; the result is a
consistent page whichever rows are ticked. The preview never applies half of a plan.

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
The other places follow from the part's shape. "Free side" means a side of the body without a pin. For a part
whose pins leave up and down, "left" and "right" are meant as written; for a part turned a quarter they turn with
it, and a text always reads from the bottom or from the right (D-16).

| Label | Places, in order | Priority |
|---|---|---|
| Pin number of a symbol (terminal designation) | right of the pin's line at the pin's outer end; left of it | 1 |
| Tag of a symbol | at the upper left: right-aligned against the symbol's left edge, its baseline level with the upper pin's end; the upper right (left-aligned against the right edge); the lower left; the lower right; centred above, when no wire leaves upward; centred below | 2 |
| Cross-reference at a contact | under the tag, in brackets; under the tag's next place | 3 |
| Make, type and rating of a symbol | under the tag (and its cross-reference), line under line, on the tag's side; the same on the other side | 4 |
| Tag of a drawn box | above the upper left corner, right-aligned to the box's left edge; above the upper edge, left-aligned at the corner, where no wire leaves there; above the upper right corner; inside the box at the upper left, where no pin text stands | 2 |
| Make and type of a drawn box | left of the box under its top, line under line; right of it; inside it under the tag | 4 |
| Contact list of a coil | left of the coil, under its tag, one line per contact; under the lower trunk below the coil; the same one line lower (two rows, where the neighbours' lists are in the way) | 3 |
| Function text of a path | centred under the path beyond the lower trunk; right of the load | 4 |
| Name of a mark (interruption point, supply mark) | in line with the wire beyond the arrow: left of an incoming mark, right of an outgoing one; above the arrow | 3 |
| Cross-reference of a mark | under the wire on the wire's side of the arrow; above the wire there | 3 |
| Name of a terminal | right below the terminal's point; left below it | 1 |
| Strip designation | left of the strip's first terminal, level with it; above it | 4 |
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

### 5.4 What is never moved

- **A text the person moved** keeps its place and angle with its part, and is a fixed text to everything else.
- **The pin texts of a drawn box** are as the scene draws them: inside the box, turned a quarter and reading from
  bottom to top on the upper and lower sides, horizontal on the left and right sides. The layout treats them as
  fixed texts and changes neither place nor angle.
- Free texts, captions of structure boxes and plug pin notes.

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

- the lead: "Arrange and route: page 102. 14 parts move, 3 of them across the sheet; 9 wires are routed. Nothing
  has changed yet.";
- now and then, from the measure: crossings, bends, wires straight, wires through texts, faults;
- **Parts:** a ticked row per part that moves, with its tag and its move in words ("6 steps right, 31 up"), the
  parts that move further than a third of the sheet first;
- **Wires you drew yourself:** a row per route of the person's that would change, unticked unless it is broken;
- **Left as it is:** parts kept in place, parts without room (with the reason), wires without a way;
- **Apply** and **Cancel**.

Unticking a row plans again with that part fixed, or that wire kept, and the canvas follows. Apply stores what
is shown, in one `layout.apply`; one Undo takes all of it back. After Apply a toast says "Arranged page 102: 4
parts moved, 9 wires routed" with Undo.

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

### 7.5 Cards drawn from the station view

The router and the placer work with a card as it is drawn, whatever its form. "Draw on a page" draws a card
as **a low, wide box with its channel pins in one
row on one edge, `CARD_PIN_PITCH` (5 steps) apart**: the upper edge for inputs, with the card set at the foot of
the drawing area; the lower edge for outputs, with the card set at its head; a card with both gets two boxes. Its
supply pins are on its left side. Today it draws two rows of pins two steps apart, so half the channels face
away from their circuits and nothing fits over a pin. A card is `pins × CARD_PIN_PITCH` wide plus two steps and
six steps high; a card wider than the drawing area is split into boxes of at most 16 pins, and the command asks
for the page of each further box. Stretching a box already spreads its pins at one common pitch: a card stretched to ten steps per
pin takes a field device over each pin. Cards already drawn are not changed. The pin texts stay as the program
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
of inputs as near the foot as there is room and one of outputs as near the head, each line from the left; a card that
finds no room waits for another page, which a toast and the To do list say. A row wider than the drawing area takes as
many pins as fit, `CARD_PINS_MOST` at most; "asks for the page" is the station view's To do list, which offers the
rest on another page, and a toast after the draw says so. `plc.draw_station` draws a box of a card whose black-box
function a page draws already as another drawing of it when none of its pins is drawn yet, which also gives a card
with inputs and outputs its two boxes in one step. The starter symbols that have a pin leaving upward (not the
terminals, the earth symbols and the valve solenoid) carry the tag right-aligned one millimetre left of their
graphics with its foot one step under the upper pin, then the PLC address, make, type, rating, function text and
cross-reference line under line 6 mm apart (a coil's contact list last): 4 mm on the QET grid, which scales places
and not texts, as in the reference set; there the lines are one block, on the native grid each line is placed alone. The interruption point's two forms keep their cross-reference under the
arrow: under the wire on the wire's side, a wire that turns at once (a supply mark's drop) ran through it. The text placer gained two places for this: a part's right corners stand beyond its pin
numbers, and a mark's cross-reference may stand centred under or over its arrow.

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

*As built (step 2).* The rows of this table marked 2 are gone. A shared run is that common beginning (a run from
the pin of both wires while both go the same way), so -068 needs nothing more. `ew route` keeps its name and now
stores `layout.apply`.

### 8.1 What is left out

- **A PLC card drawn in cells with level texts** (address and function text per channel). It is a matter of how
  a card's symbol is drawn, not of the layout: the router and the placer work with a card as it is drawn.
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
| 1 | Two parts whose tags' first places overlap: both tags are clear. A wire across a tag's first place: the tag stands at its second. A moved text and a box's pin texts are where they were. The measure counts a turn-back on a wire drawn round a caption |
| 2 | A potential wired from pin to pin over three parts is one line with two drops. A T junction moves onto the trunk and keeps its three wires and its sequence. A wire the person shaped is in no page-wide plan; a broken one is, flagged. The same page with its lists shuffled gives the same plan. A plan never changes which pin a wire ends at |
| 3 | Two-pole device over a two-pole device of another pitch: one pole straight, one step, no crossing. Five sensors over pins two steps apart: five wires step in and none crosses |
| 4 | A sensor below a card whose pin faces up stands above it after the run. A circuit that stands above the trunk that feeds it stands below it after the run; an actuator below its valve stands above it. Two circuits whose drops cross change places, two whose drops do not cross keep theirs. A part with no wire does not move. `rules_kept` holds on every plan. A second run gives an empty plan. Unticking one part gives a plan in which it has not moved and nothing overlaps. One Undo restores every place and route |
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
- The placer: relations, and per row a sort of its members.
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
| **4 Arrange and route** | The placer with the rules of §1.6 and the estimate of §4.7, the preview with its rows, planning again on a tick, the second button | — | 2 (3 for the room of §4.5; without it the chain gap alone) | Very large: about 4 500 lines |
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
3. **The estimate is not the measure** (§4.7). Groups are placed by an estimate that routes nothing; the routed
   page is then judged by the measure, and a plan that is not better is not offered. Where the two disagree a
   good arrangement can be missed, never a bad one taken. The price of a move (2 per part) is a first value, to
   be set on the proof pages.
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
