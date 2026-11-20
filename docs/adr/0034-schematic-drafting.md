# ADR-0034: Schematic drafting: one measure, routing by nets with trunks, arranging on request, texts by candidates

**Status:** Accepted · **Date:** 2026-11-19 ·
**Deciders:** lead designer; accepted by the project owner
**Supersedes:** [ADR-0033](0033-schematic-routing-model.md) decisions 4 and 5 and amendments 1 to 5; its decisions
1 to 3 and 6 to 8 are kept and restated here; its decision 9 falls with the reference project's routes.

## Context

- **What people met.** The page router of ADR-0033 was amended five times in one release. Hand tests and three
  rounds of person-like test runs kept meeting the same drawings: a wire that loops round a caption, a wire
  through a text, wires that nearly lie on one another, tags on each other, a reroute that changes little or
  leaves a page worse, tubes the long way round, and parts that had to be pushed about by hand until their wires
  ran straight.
- **Why patches did not cure it.** Every connection is routed as a job of its own between two fixed ends, so a
  potential over several devices cannot be drawn as a rail, and the result depends on the order of the jobs; the
  amendments compare orders. Texts are obstacles that another module moves afterwards, so each yields to the
  other. No part is ever moved, so pins that are not in line cannot get a straight wire. The rating that accepts
  a reroute does not count what a reader sees (loops, texts on texts).
- **What a good page is** was researched (`docs/research/schematic-layout.md`) and measured on the pilot's
  reference set, drawings that the pilot user named as neat and tidy (not in the repository): potentials as
  straight trunks at the head and the foot of a sheet, devices of a circuit in line, wires and trunks two grid
  steps apart, taps drawn as two wires on one terminal, texts at fixed places beside their symbols. Where the
  research and the reference set differ, the reference set gives this program's defaults.
- **Fixed by the pilot user.** A part placed by hand stands where the person puts it; guides are welcome and a
  snap is a light nudge; the router and the placer act only when their button is pressed; the pin texts of a
  drawn box stay as they are. Arrange and route may move a part across the page where that is best, keeping the
  general rules of a drawing in mind (the supply at the top, and the like). Route acts at once. A wire that ends
  on a wire is joined by a bevel. How a PLC card is drawn is a matter of its symbol, not of the router.
- **Free.** The file format may change where the design needs it; the reference project no longer holds a design
  back.

## Decision

1. **A stored route is the drawing; commands store and never compute** (kept). A connection's route is its full
   vertex list. A route-less connection is unrouted: a dashed line, no wire geometry, reported. Routes, junction
   positions and part positions change only by a command; replaying a log runs no router.
2. **One measure.** `ew_drafting::measure` judges a page: faults (a wire through a pin, on a wire, through a
   part, a four-way junction, a false joint, a part on a part), unrouted wires, and one cost in grid steps (a
   step 1, a bend 4, a crossing 6, a wire across a text 12, a text on a text or on another part 24). The router,
   the placer, the text placer, the checks, `ew layout` and the tests use it and nothing else. A plan is offered
   only when the page is better by it and no wire or text gains a finding it did not have.
3. **Two commands, and nothing by itself.** *Route* (wires; parts stay; applied at once, the changed wires named,
   one Undo; previewed only when a route the person shaped would change) and *Arrange and route* (parts, then
   wires; always previewed), each on the selection or the page. They replace Route page and Reroute.
4. **Nets, not connections, are routed.** Wires joined by junctions or ending at one pin are one job. A net with
   several taps is drawn as a trunk with straight drops. The junction tree, each junction's sequence and the
   order of wires on a pin are the wiring order: the layout moves junctions and never changes what a wire ends at.
5. **Shared runs.** Wires that end at one pin may share their run out of it and part with a bevel. A potential
   wired from pin to pin is thereby a rail with drops.
6. **Structure before search.** Straight wires, then trunks, then stepping wires assigned to tracks (the
   left-edge method), then the exact grid search of ADR-0033 for what is left, shortest first, improved in at
   most two passes. No negotiation, no comparison of routing orders.
7. **Arranging: best by the measure, within the rules of the page's kind.** Arrange and route may move a part
   anywhere on its page. It puts each part in line with the pin it is wired to, on the side that pin faces, from
   the largest part outward, in rows solved together, and places each group of parts across the sheet where an
   estimate of the measure is lowest. **Rules that no plan breaks:** on every page, parts inside the drawing area
   and off each other, kept parts unmoved, no part turned or moved to another page, a row in the order of its
   pins and a circuit in the order of its wiring, the spacing along a circuit as the person left it; on an
   electrical page, current running down a circuit, the potentials' trunks at the head and the foot with no part
   beyond them, a potential entering at the left and leaving at the right, a card's channels in their order and
   columns, an earth symbol beside its pin; on a fluid page, the supply below the valves and each actuator above
   its valve, the actuators in their order; on a bus page, the stations in the order of the line and level. The
   measure decides the rest.
8. **What keeps an arranged page recognisable.** Every run is previewed, with each move shown and the far moves
   listed first; unticking a part plans again around it; a kept part and everything outside a selection stay; a
   move has a price in the placer's estimate, so no part moves for a small gain and a second run changes nothing.
   There is no upper bound on the parts a run moves and no second, lighter mode.
9. **Texts are placed by one function from candidates,** the same at every drawing of a page and inside the
   router and the placer, and stay derived: a text is the program's to place unless the person moved it. The pin
   texts of a drawn box are never touched.
10. **Whose route.** `Connection.computed` marks a route the program computed. A page-wide Route routes unrouted,
    broken and computed wires; a route the person shaped is replaced only on his selection, or when broken and
    ticked in a preview. `Representation.locked` keeps a part where it stands. Both members are optional and
    additive: no format step.
11. **The bevel is the form of a new junction.** A wire that ends on a wire is joined in the bevel form, in the
    Wire tool and wherever the program joins wires; the dot is there on request. A bevel is a chain in the wire
    list (the wires run from terminal to terminal in the junction's sequence) where a dot is a star (every branch
    a wire from the feeding terminal): a tap of a new drawing gives as many wires with other ends. Junctions of
    drawings made before stay as stored, and the layout changes no junction's form.
12. **Positions are grid positions.** The layout computes in sixteenths of a grid step and never in nanometres;
    a pin's, a junction's and a route vertex's point on the grid is the grid's own number, at the source.
13. **Guides while placing by hand.** A guide shows where a pin of the hanging part is in line with a pin it
    faces; within three quarters of a grid step (the ordinary snap is half a step), at most eight screen points
    further, the part takes that line. No place is computed for a part that is placed by hand.
14. **One home.** A crate `ew-drafting` (GPL-3.0-or-later; `ew-ids` only, `ew-units` allowed) holds the model, the
    measure, the text placer, the router, the placer and the guides. `ew_derive::routing` is deleted.
15. **Kept from ADR-0033:** the two forms of the interruption point; connecting on a move.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| **A sixth amendment of the page solve** | Its faults come from its shape (two-ended jobs in an order, texts as walls, no placing), not from its constants |
| **Negotiated congestion (PathFinder)** | Cures the order problem of a general graph at the price of a result that is hard to explain; with straights, trunks and tracks taken out, few wires compete |
| **Free tree shapes per net (rectilinear Steiner trees)** | The shape of a net is the wiring order the person declared |
| **A template per kind of page** | Needs more than the type of a page says, fails on mixed pages, and would recast a hand-made page; the rules of decision 7 bind without one |
| **A layered layout of the whole page (Sugiyama, ELK Layered)** | Draws a new page from the netlist; does not keep what the person arranged; no rails |
| **A general constraint solver for the placer** | The constraints are rows on a tree, which a one-pass method solves exactly |
| **Arranging that only lines up and makes room** | Leaves a device on the wrong part of the sheet; the pilot user chose the full move |
| **An upper bound on the parts one run moves; a second, lighter mode** | A bound leaves a page half arranged at an arbitrary cut and every run is previewed; a mode would be a third command, and the selection and the preview's ticks give its result |
| **Storing the place of every text** | A format mark, a many-text command and text moves in every tool, for stability that a deterministic placer mostly gives |
| **The dot as the form of a new junction** | The trade's notation and the pilot's drawings show where each wire lands; the dot stays on request |
| **A library (libavoid, ELK, OGDF)** | None routes on a grid with pin sides, trunks and texts; licences and languages do not fit |
| **Proposing a place while a part is placed by hand** | Ruled out by the pilot user: the part stands where the person puts it |

## Consequences

### Positive

- A potential over several devices is a line with drops; devices of a circuit stand in line; a wire is not led
  round a text that can step aside.
- An arranged page follows the rules of its kind: the potentials at the head and the foot, current running down,
  a field device at its channel, an actuator over its valve.
- One function decides what is better; a reroute can say truthfully that nothing better exists.
- The same page gives the same plan whatever the order of its records; every phase is bounded.
- A hand-drawn route and a kept part are safe from the program unless the person asks.
- One router, one text placer, one measure; the search of ADR-0033 is the only part carried over.

### Negative

- Every stored drawing may show its texts at other places once (the new text placer), and expected pictures of
  tests change once.
- A text can stand elsewhere after a later change of the text placer: texts are derived.
- Arrange and route can move many parts far; the preview is the only thing between a careless Apply and a page
  the person has to learn again (one Undo takes it back).
- The placer chooses by an estimate and the routed page is judged by the measure: a good arrangement can be
  missed, though a worse page is never offered.
- New drawings get chains where older ones got stars at a joint: the wire list of a tap differs between a
  drawing made before and one made after, until the person sets the form.
- A shared run asks the reader to take a bevel as "two wires part here"; a part cannot be dropped on one.
- Nothing is turned and no circuit is spread: some pages stay for the person to arrange.
- Two optional members in the file; an older build keeps them and does not obey them.
- The reference project's routes are no longer pinned; the fixtures of the design take their place.

## Amendments

The decision text above is unchanged; these entries refine it.

1. **Every plan the placer weighs is routed** (2026-11-20, the placer anew). Decision 7 placed each group of
   parts "where an estimate of the measure is lowest", and decision 8 priced a move "in the placer's estimate". On
   pages with rails or many nets the estimate did not foresee what the router draws: plans it ranked first were
   refused by the measure afterwards, the page fell back to Route's plan with no part moved, and stages ranked by
   the estimate undid each other's choices. A page has tens of parts and nets and a run of the router takes a
   fraction of a second, so the estimate goes. Decisions 7 and 8 are refined:
   - The placer proposes **one arrangement from the pins** and routes it. The parts hang in trees made of the
     relations between their pins, the strongest first over the whole page (a facing pair, a tap of a net, a
     corner pair, a pair that turns back), with a row the person made (parts of one shape that stand level: a
     strip, cards side by side) kept as mates; rows, stacks and rails are set from each tree's anchor, and a tree
     steps as a whole to find room (06a §4.2 to §4.7).
   - **The judge.** The arrangement, then its trees one by one, then it without or with one branch, are each
     routed and measured. A plan is taken when the rules hold on the routed page, no wire or text gains a finding,
     it has no more faults and no more crossings than Route's plan over the same wires, and it is better than
     that plan by the measure with the price of its moves added (2 steps of wire per moved part). The measure,
     not a rule per figure, decides between crossings and bends. Then each branch is put back where the page is
     no worse without it. Without a better plan Route's plan is offered and no part moves.
   - **A row goes or stays as one.** The members of a row the person made are never left half moved.
   - **Rules sharpened.** "A row in the order of its pins" (A4) reads facing pairs only, and reads the terminals
     of a strip as one row: wires that turn a corner nest and wires that turn back lie inside one another, for
     both the parts' order is the reverse. "The spacing along a circuit" (A5) lets a part that stood nearer than
     the chain gap take it and a part that joins its row's line stand up to six steps further; nearer or further
     is closed up or spread. "A card's channels in their columns" (E4) does not bind a part wired to a card's pin
     by a pin of its own that faces the same way: it stands at the card on a short hairpin.
   - **Whether texts fit is asked of the text placer,** on a page of the parts in question, not read from the
     labels' first places. One text rule is added to decision 9's candidates: the name of a mark whose texts, where
     their author put them, would stand on another mark takes the place in line with its wire beyond its arrow,
     so that marks stack two steps apart.
   - **What it costs.** A press runs the router a bounded number of times (06a §9.5), where the estimate ran it
     once. The consequence "the placer chooses by an estimate" is replaced: the placer weighs one arrangement and
     its parts, so an arrangement that needs two changes which each lose alone is missed; a worse page is still
     never offered. A page wired against its symbols (a card's pins that face up wired to the upper pins of
     terminals under the card) gets Route's plan and no move (06a §4.8).
   - *Alternative: keep the estimate and correct it with the router's result.* It would be a second measure to
     keep true to the first. *Alternative: route every candidate place of every part.* The candidates multiply;
     one arrangement built from the pins' structure, and its parts, are few.
2. **A card's pins stand 8 steps apart, and a strip stands by its channels** (2026-11-20). The design (06a §7.5) drew a
   card's channel pins 5 steps apart and advised stretching the box where devices stand over the pins. On a page
   with field devices no row of devices fits that pitch, so every wire stepped. The pitch is 8 steps (a device
   with its tag stands over each pin; a card of eight channels still fits beside the controller at the head of an
   A3 sheet), and one box of a row that does not fit takes 8 pins at most. With it the placer sets each terminal
   that is wired to a card in the column of its channel, also where the strip was set down in another order or
   between two cards, and a row that has no room at the chain gap stands nearer (06a §4.4, §7.5). The sentence
   of amendment 1 on a page wired against its symbols is corrected in 06a §4.8: the wires round a card cross pair
   by pair with the strip in the order of the pins, so no room the placer makes takes those crossings away.
   - *Alternative: keep 5 steps and stagger the devices in two rows.* A tag of six characters then clears the
     neighbouring wire by a fifth of a step; one more character and it does not.
   - *Not built:* the cards of one page on common columns clear of the controller's column (06a §7.5), and the
     router phase for the wires that go round a card of the older form (06a §4.8).

## Related

- **Design:** [06a Drafting](../design/06a-drafting.md) (the design this record decides),
  [06](../design/06-schematic-editor.md) §3.5, §5.4, [15](../design/15-checks-revisions-collaboration.md) §4.
- **ADRs:** [ADR-0003](0003-freshness-contract.md), [ADR-0007](0007-explicit-connections-and-autoconnect.md),
  [ADR-0008](0008-transactions-undo-preview.md), [ADR-0011](0011-format-versioning-and-migration.md) amendment 6,
  [ADR-0027](0027-native-symbol-grid-and-qet-grid-policy.md), [ADR-0033](0033-schematic-routing-model.md).
- **Requirements:** FR-CON-001, FR-CON-004, FR-CON-006, FR-CON-031, NFR-REL-07.
