# Drafting: routing wires, arranging parts and settling texts on a schematic page: placing

A chapter of [Drafting: routing wires, arranging parts and settling texts on a schematic page](06a-drafting.md), whose table says where each section lives.

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
second, so the real answer is affordable and the estimate went (ADR-0034).

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

On pages with a card at the head and one at the foot:

- **A strip's terminals stand by their channels.** A member of a row does not take a line that carries it past a
  unit of its row that has no line of its own (a fixed one, or one that hangs beside its host), nor further from
  it than it stood by more than `ROW_PULL`; a unit of the row with a line of its own binds nobody. So the terminals
  of one strip that are wired to two cards each stand at their own card's pin (rule E4).
- **A strip keeps its order** (owner, 2026-11-23). The terminals of a strip that stand in a row keep the order they
  stand in: no terminal passes another, in any step of the placer (the rows' setting, the opening below, the judge's
  steps). A person reads a row of terminals by its numbers; a row drawn in order that came back out of order (a
  terminal wired to a card's second row of pins set in that pin's column, a feed terminal sent to the strip's end) was
  a fault, whatever it saved in crossings. What may change is the pitch and the place of the row as a whole, and the
  wires then cross where the person's wiring crosses. The judge takes no plan that sets one terminal of a row past
  another (two that stood at one place along the row have no order to keep).
- **A strip opens as one.** Where the strip's other terminals hold a channel's terminal off its column
  (a strip at a narrow pitch over or under a card of a wide one, with a feed, the sensors' common terminals, a
  terminal of another card or a spare among them), or where the rows set a row of terminals out of its order, the
  arrangement is made once more with the strip opened, and the judge takes the better page. The strip is the
  terminals joined as mates side by side, three or more and none fixed, two or more of them in line on pins of one
  card that face up or down. Of its channels' terminals
  the longest run that stands in the order of the channels' columns, with room between each two for the terminals that
  stand between them in the order (their bodies a row's gap apart: two steps for a terminal), takes its columns; of runs
  as long, the first. A channel's terminal whose column would put it out of order (a card with two rows of pins, a
  strip wired crosswise) does not go to its column. Every other terminal (a feed, a common terminal, a terminal of
  another card, a spare, and those channels' terminals) keeps its place in the order: between two terminals of the run
  evenly in the room there is, before the first or after the last at the strip's pitch, and there beyond what hangs on
  the run's outer two in line (a button over the first with its tag at its left), so that their wires to the parts over
  the run pass clear of those parts and texts. All stand on one line, the strip's designation at the first; what hangs
  on a terminal in line (a sensor) stands over it, what hangs round a corner on a terminal that is no channel's (a mark
  at the sheet's edge) stays. While the rows are set, the strip's other terminals are in no member's way, and none of
  them is set beside or round a corner from its host. The strip stays as it stands where no two channels' terminals can
  take their columns (their channels in another order, or too little room between their columns), or where a terminal
  or what moves with it would stand on another part or outside the drawing area, and a strip that opened goes or stays
  as one in the judge's steps; where the judge finds the page no better with it open (a card with two rows of pins), it stays too. In each case the
  message of an empty plan names the strip and what held it (§4.9).
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
  `RESETS` times at most after their first setting (so `RESETS` + 1 settings in all).
- **A card steps along its edge.** A card that is the root of its tree, with its channel pins on its upper or
  lower edge and its supply pins on its left side, steps right along its edge (a) until every pin its supply is
  wired to on the side its channels face (the controller's supply pin, a mark, the supply pin of a card at the
  other edge) stands left of it, so that the supply drops beside its first channel and crosses none, and (b)
  further where the columns of its channels in use interleave with those of a card that faces it (one card's used
  columns among or on the other's): of the two, the card with fewer channels in use (of two as many, the one further
  right) steps on until its used columns stand right of the other's, two steps clear, so that the parts of the two
  cards' channels do not stand among each other and a strip between them keeps its order; a spare terminal of that
  strip between the terminals of the two cards' columns asks for the strip's pitch more, so that it keeps its place
  in the strip. Columns without a wire count for nothing. Whether
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
   least first, down, up, right and left, `REACH` (24) at most each way, until the whole tree has room; the
   other roots of its shape that stood level with it take the same step along that axis, so that a row of cards
   stays a row. Without such a step the root stays and what has no room goes back to where it stood. The trees
   are set heaviest first, each clear of those set before it. On a fluid page (rule F1) what hangs on a valve's
   unit with a part of the supply steps below every valve, and what hangs there with an actuator steps above
   that unit's valves and takes the blocks above it along.
2. **The rules.** The parts a broken rule names go back, with what hangs on them, until `rules_kept` holds
   (§1.6; the rules read the parts and the nets, so this needs no routing).
3. **Routed and judged.** The arrangement is routed and measured. It is taken when
   - the rules hold on the routed page;
   - no wire and no text gains a finding (the never-worse rule of §1.2, with F7); where it leaves fewer wires
     without a way than Route's plan, a wire newly across a text does not count, as a new crossing does not: the
     dashed wire weighs more (§3.8). A supply page drawn side by side, whose first mark's wire had no way into the
     main switch, got no plan, because the plan that set the feeder in one column and routed that wire laid two
     other wires across a mark's and a breaker's text;
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

A part that stays against a rule is named in the plan with what was in the way of its place on the side its host's
pin faces, so that the person knows what to move: "-K3 stands in its place below its pin", "its place below its pin
lies outside the drawing area", or, where the arrangement moved it there and the judge refused the plan, the judge's
reason ("in its place below its pin a wire would run through a text of -X11", "... the wires would cross more than
they do now", "... the page would be no better"); "no place below its pin has room for it" where its place is clear
but the arrangement found no room for it with its row and what hangs on it. Its place is the one in line with its
host's pin, with what hangs on it, `CHAIN_GAP_MIN` beyond the pin at least and clear of the host. The placer
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
  one, at the pitch it had or the one its new place asks of all its members. The terminals of a strip keep the order
  they stand in, always (§4.4, "A strip keeps its order"). Within that order rule E4 comes first: a terminal wired to
  a card's pin stands in that channel's column where its column lies in the strip's order, so a strip between a card
  and its field devices takes the pitch of the channels, and a terminal of it that is wired to another card stands at
  that card's pin. A terminal of the strip wired to no channel (a feed terminal, the sensors' common terminal, a
  spare) keeps its place in the order, between its neighbours, and its wires cross where the person's wiring crosses;
  it does not go to the strip's end. Where
  the strip stands at a narrower pitch than the card's, it opens as one to the channels' columns in its own order
  (§4.4, "A strip opens as one").
- **Order.** The parts of a circuit stand in the order of the wiring, the parts of a row in the order of the
  pins (rule A4, which reads the terminals of a strip as one row); a strip's terminals keep the order they stand in.
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

**A card in the older form.** A project of an earlier version may hold a card with two rows of pins, so half its
channels face away from their circuits: such a page is the case above. No placement makes that form (doc 10 §3.6).
Draw the card in one row, in the box's menu ([10](10-plc-and-bus.md) §3.6), draws the card again in its place without wiring it anew, in one undo step:
the device, its channels and every connection stay, because a connection joins connection points of the device's
function, not the box; the connections end at the new pins, and Arrange and route draws them. What stays for the person is
each terminal of a channel that now faces the other way: the wire to the card belongs on the terminal's pin that
faces the card. *Terminal wires to card side* (`terminal.face_card`, in the card box's menu) does that for the terminals of a card or of a strip: the wire to the card goes to the terminal's pin that
leaves the opposite way to the channel pin, the field's to the other. Route lays the wires round one card together as
nested rings (§3.5, Rings): they
remove no crossing that the order of the strip forces. Arrange sets such terminals beside the card as hairpins where
it may (§4.4), and where they stay behind it opens the rings' room (§3.5).

### 4.9 The plan

`arrange(page, scope, fixed) -> Arranged`: the parts' moves, the parts left against a rule with the side their
host's pin faces and what was in the way (§4.7), and the plan of the page as it would become (the routes, junctions and marks, with the
quality before and after). The plan is empty when nothing better than the page as it is was found. The message
says that the page is already arranged as well as the rules allow only where nothing stands in the way; else it says
that no plan was better than the page as it stands and names what stands in the way: each part left against a rule
(as the preview's rows do), each strip over a card that did not open as one (§4.4) with what held it (its terminals
wired to the channels in another order, too little room between the channels' columns, the part in the way, the
drawing area's edge, or the page no better with it open), each wire without a way (with what is in it, where the router found that), and the faults the
page keeps.

**A selection** makes only the selected parts movable; everything else is fixed and can be a host. A selection of
one feeder arranges that feeder and leaves the page.

**In part.** Unticking a part in the preview adds it to the fixed units and plans again; unticking a wire keeps
its route and the parts at its two ends. The result is a consistent page whichever rows are ticked. The preview
never applies half of a plan.

**The same plan whatever the order of the page's lists**: units, relations and blocks are taken in an order made
of their weights, places and IDs.

`ew_drafting::place` (the run and the judge) and `ew_drafting::place::sketch` (the arrangement),
each with its module documentation step by step. Units by `unit` number, by an outline drawn round other parts
(`frame_only`: the parts inside it, but an actuator or a pressure source, which is no part of a valve terminal, so a
cylinder dragged inside the outline is moved out as any other) and by a pin set on another part's pin; a unit
is fixed also when its origin is off the whole grid
(the API keeps a symbol whose position is not exactly a grid point fixed, since a move by steps counts from it).
A junction that stands in line with the drop of a part's pin follows that part across, and a junction joined to
it by a wire with it, so that a rail's junctions stay over the parts they feed when those move. The text placer
is asked through `measure` on a page of the parts in question (§4.5). Not built: an order of independent
circuits across the sheet other than the one their trees' steps give (§1.6 leaves it to the measure; no candidate
swaps two trees), and the labels between a host and a row as room of their own (the text placer moves them).

