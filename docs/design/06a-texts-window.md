# Drafting: routing wires, arranging parts and settling texts on a schematic page: texts, placing by hand, the window and proof

A chapter of [Drafting: routing wires, arranging parts and settling texts on a schematic page](06a-drafting.md), whose table says where each section lives.

---

## 5. Texts

### 5.1 One text placer

`settle_labels(&DraftPage) -> Vec<Settled>` (each a label, its box, its rank down its list and whether it reads
turned a quarter) is the one place where a label gets its place. It runs when the scene draws a page, inside `measure`, and inside the router and the
placer. Its input is what is stored: the parts' bodies, the wires' routes, the fixed texts, the drawing area and
the labels with their sizes and places. So a page looks the same on the canvas, in the PDF and in the plan.

### 5.2 The places of each kind of label

The first place of each kind is this program's default, taken from the pilot's reference set; the starter
symbols' slots are set to these first places, and a library symbol whose author chose another slot keeps its slot as first place.
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
| Tag of a drawn box | above the upper left corner, right-aligned to the box's left edge; above the upper edge, left-aligned at the corner; above the upper right corner; inside the box at the upper left, where no pin text stands. A place in the way out of a pin a wire leaves (over a top pin, beside a pin on a side, within `LABEL_CLEAR` of its line) comes after every place clear of such ways, so the wires into the pins do not go round the caption | 2 |
| Make and type of a drawn box | left of the box under its top, line under line; right of it; inside it under the tag | 4 |
| Contact list of a coil | left of the coil, under its tag, one line per contact; under the lower trunk below the coil; the same one line lower (two rows, where the neighbours' lists are in the way) | 3 |
| Function text of a path | centred under the path beyond the lower trunk; right of the load | 4 |
| Name of a mark (interruption point, supply mark) | in line with the wire beyond the arrow: left of an incoming mark, right of an outgoing one; beyond the arrow over the wire's line, then under it (marks one step apart, where a name is taller than the step: one name stands in line, the next steps aside); above the arrow | 3 |
| Cross-reference of a mark | under the wire on the wire's side of the arrow; above the wire there; the same past the number of the pin the mark is set on (an interruption point on a contact's pin); under and over the mark's name where the name stands | 3 |
| Name of a terminal | right below the terminal's point; left below it | 1 |
| Strip designation | left of the strip's first terminal, level with it; above it; above the terminal's name, where it stands over the terminal; above it, ending at its left edge; centred below it; right of it, level with it (where a wire comes to the terminal round its left and over its top) | 4 |
| Wire number, tube tag | along the wire: on a vertical stretch right of the line, turned to read from the bottom; on a horizontal stretch above the line. Tried on each straight stretch at least the text's length plus two steps long: first level with the number of the parallel wire beside it that is already placed, then at the middle of the longest stretch, then at its quarters, then on the other side of the line | 5 |
| Type text of a cable mark; the cable's tag | the type left of the row of ticks, the tag right of it; both right; both left | 5 |

A device's function text that no slot of its symbol prints (the symbol has no function-text slot, or its slot is
hidden) is a label of each representation that prints the device's tag, with the places of a path's function text,
in a function-text slot's style; a terminal prints its function text as its label instead.

The texts of one part on one side are a block: they keep their order and stand line under line. Where tags of a
row are wider than the row's pitch, every second one takes its "one line lower" place: two rows, as a
draughtsman staggers them.

### 5.3 The choice

Labels are placed in the order of priority, then of their owner's place (top, then left), then of their ID. Each
takes its first place that is clear by `LABEL_CLEAR` of every part's body but its own, every wire, every fixed
text and every label placed before it, and inside the drawing area. A first place that leaves the drawing area is
tried next moved in to stand `LABEL_CLEAR` inside the area's line, but not where that sets it on its own part's body,
which its author kept it off: the tag and data of a symbol placed near the area's edge stand at the places of their
kind beside it, clear of the symbol and its pins' numbers. A label with no clear place
takes the place of least cost (C4, C5) of those inside the drawing area, and is then counted: a text never stands
outside the area. Where every place of a label leaves the area, its places moved in, onto
its own part's body too, are its places. There the first place of another text of the same symbol (its tag,
cross-reference, data, contact list) not yet placed costs nothing, as that text moves on, and a neighbour's first place
costs as a text: a symbol's texts stand by their own symbol before a neighbour's. One repair round follows: for each such label, each label
it lies on is tried at each of its other clear places in turn, and the label without a clear place then at its first
clear place; the first change that lowers the labels' cost on the page is kept, and while the label still lies on
another, the next one it lay on is tried. The result does not depend on the order of the page's lists.

A label leaves its first place only when something stands there, so a change elsewhere on the page does not move
it.

**The row rule.** Like parts that stand level (one shape: the same body and pins, their origins on one
line) take one place for each of their texts: when the first of them comes in the order, the place the same number
down every one's list at which all of them are clear is taken for all, so the tags and cross-references of a row of
relays stand alike and a wire across one tag's first place moves the row's tags, not that one. Where their lists are
not alike (a wire leaves one of them upward) or no place is clear for all, each is placed on its own; texts wider than
the row's pitch keep the two-rows rule of §5.2.

**The stacked-marks rule.** A symbol's author may have set a mark's name above its arrow (an imported library, a
symbol of the person's own). Such a mark is three to four steps tall with its texts, and marks stacked two
steps apart (§4.4) would stand on each other's names. So the name of a mark whose texts, where their author put
them, reach another mark or its texts, or leave the drawing area, has the first place of its kind in §5.2 before
the author's: in line with the wire beyond the arrow. A mark that stands alone keeps its name where its author put
it, and so does a name its author set in line already (the starter symbols). The placer reads these first places (`first_places`), so it sets a stack by the texts as they will stand.

### 5.4 What is never moved

- **A text the person moved** keeps its place and angle with its part (through a turn of the part, its place on the
  page beside it, [06](06-commands-connecting.md) §3.4), and is a fixed text to everything else.
- **The pin texts of a drawn box** are as the scene draws them: inside the box, turned a quarter and reading from
  bottom to top on the upper and lower sides, horizontal on the left and right sides; a card drawn in one row prints
  its channels level in cells ([10](10-plc-and-bus.md) §3.6). The layout treats them as fixed texts and changes
  neither place nor angle.
- Free texts, captions of structure boxes and plug pin notes. A text of a symbol ([06](06-schematic-editor.md) §3.8)
  is the symbol's to the layout: a plan that moves the symbol moves it along.

**Where the placer may move a text.** The placer places only a text without a stored place: a slot text at
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

*A data text on its own pin's line (decided 2026-11-22).* A symbol whose author set its make, type or rating
on the line of one of its pins prints it through that pin's wire only where the text's place is stored. Without one the
text gives way to the wire as every label does (§5.3: a wire stands at its first place), and nothing is added for it.
A stored place stays the person's, whatever runs through it (the paragraph above): a script or an import that pins
every text of a symbol where its author drew it keeps them there, and Reset text positions gives them to the placer.

### 5.5 Derived, not stored

Label places are computed, not stored, as the tag rule is. *Alternative: the layout stores a place for
every label it settles.* That would make texts as stable as routes, but it needs a mark in the file to tell the
program's place from the person's, a text command that takes many, and text moves in every tool that writes a
route; and a part placed by hand would keep a colliding tag until a button is pressed. The present rule (a text
is placed by the program unless the person moved it) stays, with one placer instead of nine steps. What is given
up: a later version of the placer may place a text of an old drawing differently. The fixtures of §9 hold its
results.

The adapter is the scene's (`ew-scene`, `draft.rs`): it builds the page's `DraftPage` from
what the Labels layer draws, and `Session::page_draft` adds the supply ranks and the page's kind; `role` is
`Other` in the scene's draft. A symbol's slot, where its author put the text, is a label's first
place; there a label only must not lie on a fixed text or a text of its own part, and two texts of one part both at
their author's places do not count against each other. After §5.2's places each kind has a few more, tried in this order: one
line lower (a tag in a row); above the upper corners; a strip designation above it, ending at its left edge; a
mark's name one line higher; a drawn box's data under the earlier data, and above the box over its tag; a
terminal's name turned a quarter, and beside its body; a mark's cross-reference beyond the arrow, under then over
the wire. A mark's cross-reference reads its places by the name from the name's box where the name is placed, not
from its own height. A label stands nowhere it reads as another part's (nearer another part of the same body size than its
own), and a label of a part without pins stays inside its outline or clear of it. A place that is the free first
place of a label not yet placed is taken only when no other is clear. A text whose author's place lies on a text
of its own part placed before it has the place under that text second (line under line). A clashing label is then
moved by the repair round of §5.3.

---

## 6. Placing by hand, helped

**The rule.** While a part hangs on the pointer (the placing tool) or is dragged (the Select tool), it stands on
the grid point nearest the pointer. Nothing computes a place for it, and nothing moves it after it is
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
get on placement is shown dashed, as connecting on placement shows it. No router runs while a part hangs or is
dragged: a moved part's wires follow it and are solved at release (§7.4).

*Alternative: show where the placer would put the part and snap to it.* It is what the pilot user ruled out: a
part placed by hand stands where the person puts it.

`ew_drafting::guides(page, ghost, pointer, points_per_step)` takes the hanging part at the
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
schematic, fluid or bus page they are disabled. Route both routes the unrouted and improves what the program routed,
so no second button says which.

### 7.2 Route

1. The plan is computed.
2. *Nothing to do:* a toast, "Nothing to route: every wire runs as well as the rules allow."
3. *No wire of the person's would change:* the plan is applied at once (one undo step). A toast names the wires,
   "Routed 5 wires: 102.3, 102.7, 102.9 and 2 more", with Undo; the wires are drawn as selected while the toast is
   up; wires left unrouted are named with what is in their way.
4. *A route the person shaped would change* (it is broken, §3.2): a preview card as in §7.3 with those wires as
   ticked rows; nothing changes until Apply.

On a selection the toasts of 2 and 3 say so and how to route the page: "Routed 1 selected wire: 1.8. Select nothing to
route the whole page.", "Nothing to route among the 14 selected items: no wire would run better. Select nothing to
route the whole page."

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

`layout.arrange` beside Route in the Connect group and in the menu of a symbol and of a selection
of symbols; `ew route --arrange`. The card (the Preview tab) has the lead, the figures now and then, a ticked row
per symbol that moves (far moves first, its move in steps) and per wire you drew that gets a new route, and "Left as it
is": kept symbols with their pin, symbols without room with the side, wires without a way and nets wired against
their order. A tick (`layout.arrange_tick`) plans again with every unticked symbol where it stands and every unticked
wire as it runs; unticked symbols stay listed. The canvas paints the page the preview's candidate would leave
(`Session::preview_page`, read once per plan) with each moved symbol's new place tinted, its old one a faint dashed
outline and a dotted line between; while B is held it paints the page as it is (B, the black box tool's key, does
nothing else while the card is open). The toast after Apply: "Arranged folio 102: 4 symbols moved, 9 wires routed."

The jobs are every wire of the scope (§3.2), and Route's plan of the same jobs is the plan to
beat. The lead counts the wires you drew that get a new route ("9 wires are routed, 2 of them drawn by you"); the
heading "Wires you drew yourself" is a tick of its own (`layout.arrange_tick {all}`) that unticks every row under it,
or ticks them all. Such a wire's row belongs to the card even where Route alone keeps the wire as it runs (Route
keeps a drawn wire; Arrange's plan gives it a new route where that beats Route's plan): with every symbol unticked the
card can still list them, as the sample's folio 103 lists -X1031:SH/b → -103M1:PE. The history names a
`layout.apply` with symbol moves "Arrange and route" (`ARRANGE_STEP`), one
without "Route wires". A canvas fitted when the card opened fits again at the first new size of its pane after the card
is gone. The card's proposal is the whole page: a zoomed page is fitted when the card opens, and so fits into what the
Preview pane leaves; the card's figures and rows scroll with its lists inside the pane.

A tick plans again with the unticked rows and, of the wires the person drew,
every one the card does not list kept as they run (`ShellModel::act_layout_arrange_tick` reads them from the page's
draft); when nothing is left to apply the card closes with `toast-arrange-none-left`.

### 7.4 The Wire tool, connecting on placement, moving

- **The Wire tool** stays as it is for the person: click a pin, click a pin, the tool finds the way; a click on
  the grid fixes a bend. A wire starts at a pin, a junction or a wire: a first click in free space is refused in
  those words (beside a pin the hint says the click missed it). Its way is `ew_drafting::route_wire`, the search of §3.6 against the occupancy of the
  page, so the tool and the Route command cannot disagree. A wire does not go round a tag that has another free
  place (the tag steps aside when the wire is stored, because labels are derived), and a wire drawn to a pin that
  already has a wire may share that wire's run (§3.4). A route found between two clicks is marked `computed`; one with a fixed bend is the person's. A wire that ends
  on a wire is joined in the bevel form (§7.6).
- **Connecting on placement and on a move** stays as it is: straight wires between pins in line, shown before the
  part is set down, never changing a connection. It needs no router. The placement's toast names the wires it made
  after what it placed, "Placed -101K2 at 101-E4 and connected 13 to -101K1:14", two or more by their count and the
  first ("… and connected 3 wires: 13 to -101K1:14 and 2 more"), a terminal's by the terminal, and ends with the way
  to place without them ("Alt+Click or Ctrl+Enter places without connecting"); a Terminal… row names the wires of
  all its terminals. That toast has a key of its own, so the next placement's toast leaves it up, and the next
  placement that made wires adds its sentence to it.
- **A move by hand** keeps its re-solve at release: wires follow their ends, and those left broken or with more
  bends are routed (§3.2) and stored with the move. It uses `route` with those jobs.
- **`ew route`** on the command line runs Route for a page or a selection (`--arrange` runs Arrange and route);
  `--dry-run` prints the plan.

Route is `layout.route` in the Connect group, and in the menu of a wire and
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

*The straight line.* Between two ends on one line the tool takes the straight line where it crosses
nothing (`ew_drafting::route`, `crosses_nothing`): it leaves a pin end along its side and enters the other against
it, takes no turn back after a fixed bend, and on it lies no pin or junction but at its ends, no wire (along it, across
it, or meeting it but at its ends), no body but its ends' own parts' (nor a body's edge) and no text that cannot step
aside, nor along the wire's own route so far. A pin's stub and the step round a body close nothing here, as they do
to the search (§3.6): a person who draws
a rail one step over a row of pins gets it as drawn, and each tap he then draws drops down a stub onto it. Where something stands on the straight line the tool goes round, and its hint says so ("Something stands on
the straight line between these two points, so the wire goes round it"). Route keeps such a rail and its taps: no way
round costs less (§3.9).

*Never back over itself.* After a fixed bend the tool's way on runs along none of the wire so far: the
router takes the wire's route up to the bend (`route_wire`'s `before`) as a wire laid, which the way on may cross but
not run along, nor pass through its bends, so no way on runs down, back and up onto the bend's own line and along the
first leg; where no other way leads on, the wire is routed anew from
its start, as for any bend from which no clear way leads. No route the search finds, for Route or for the tool, holds
a point twice in a row or turns back along the line it came by: a debug build asserts it on what the search returns
(`search::route`, `geom::tidy`), so that the phase that would make one is found, not cleaned after it. A route put
back from the page or kept (a job without a way keeps its stored route, §3.8) is the person's or an older version's
and is laid as it stands.

### 7.5 Cards drawn from the station view

The router and the placer work with a card as it is drawn, whatever its form. "Draw on a page" draws a card as **a
low, wide box with its channel pins in one row on one edge, `CARD_PIN_PITCH` (8 steps) apart**: the upper edge for
inputs, with the card set at the foot of the drawing area; the lower edge for outputs, with the card set at its head;
a card with both gets two boxes. Stretching a box spreads its pins at one common pitch. The pin texts stay as the
program draws them on a box.

- **What a card is.** A device of the station whose part's pin map gives its channels pins: inputs are digital and
  analogue inputs, counters, IO-Link and multi-function ports; outputs are digital and analogue outputs. Its box is
  `pins × CARD_PIN_PITCH` and two steps wide, the row centred (half a step to the left), `CARD_HEIGHT` (6 steps)
  high; its other pins (the supply) stand down its left side `MARK_PITCH` apart, the lowest that far above the lower
  edge of a card of inputs and the highest that far under the upper edge of a card of outputs. The CPU and any
  device without channel pins keep their box; so does a box a person draws (Place here).
- **Where it stands.** The other boxes stand from the next free place, in slot order; then each card's box takes the
  first place that keeps a gap from what the page draws, a card of inputs as near the foot as there is room and one
  of outputs as near the head, each line from the left. On its line a card of inputs takes the first place beside the
  boxes drawn with it that are no card (the controller's), a gap off, before one under them, so the supply wires
  from the head cross none of its channels. A card that finds no room waits for another page, which a toast and the
  To do list say.
- **Wide cards.** A row wider than the drawing area takes as many pins as fit, `CARD_PINS_MOST` (8) at most, a byte
  a box; the station view's To do list offers the rest on another page, and a toast after the draw says so.
- **A part that lays out its box** ([07](07-symbols-parts-macros.md) §2.5: a person placed its pins in the box) is
  one box as the part lays it out, placed as a card of outputs is; so is it placed by Place here, with one click.
- **Further boxes.** `plc.draw_station` draws a box of a card whose black-box function a page draws already as
  another drawing of it when none of its pins is drawn yet, which also gives a card with inputs and outputs its two
  boxes in one step.
- **Why 8 steps.** A sensor with its tag and address is six to seven steps wide, so at a smaller pitch no row of
  devices can stand in line with a card's channels and every wire steps; at 8 steps a device stands over each pin,
  and a card of eight channels (66 steps) still fits beside the controller's box at the head of an A3 sheet. The
  price: a card of sixteen channels does not fit one A3 page, and a card of inputs, which is set at the foot from the
  left edge, finds no room on a page where a symbol stands within two gaps of the foot (the To do list then offers
  another page). The pins of a card at the head and one at the foot stand on columns two steps apart where the
  controller's box is not a whole number of pitches wide; the command draws them so, and Arrange and route steps the
  card aside (§4.4), for a card the person drew as well.

**The starter symbols' slots.** The starter symbols that have a pin leaving upward (not the terminals, the earth
symbols and the valve solenoid) carry the tag right-aligned one millimetre left of their graphics with its foot one
step under the upper pin, then the PLC address, make, type, rating, function text and cross-reference line under line
6 mm apart (a coil's contact list last): 4 mm on the QET grid, which scales places and not texts, as in the reference
set; there the lines are one block, on the native grid each line is placed alone. The interruption point and the
supply mark have their name in line with the wire, so that marks stack two steps apart; the interruption point's two
forms keep their cross-reference under the arrow: under the wire on the wire's side, a wire that turns at once (a
supply mark's drop) ran through it. The text placer has two places for this: a part's right corners stand beyond its
pin numbers, and a mark's cross-reference may stand centred under or over its arrow.

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
feeding terminal. The junction's Properties show the wires it produces, and the sequence can be
reordered there.

**Drawings made before keep their junctions as stored.** The default is written into a junction when it is
made; it is never read into a junction that exists, and a junction stored without a form reads as a dot, as
before. Importers keep what their source says. The layout draws both forms and changes neither (§2.3).

---

## 8. One router, one text placer, one measure

There is one router, one text placer and one measure: `ew_drafting::route`, `ew_drafting::settle_labels` and
`ew_drafting::measure`, read through `Session::page_draft` and stored by `layout.apply`; Route is `layout.route` (§7.2).

**As ADR-0033 decided:** a stored route is the drawing and no reader routes; a route is the full vertex list; an
unrouted wire is a dashed line and is reported; commands store and never route; the two forms of the interruption
point; connecting on a move.

**The checks.** EW-CORE-067, -068, -069, -070, -072 and -073 read the measure; -068 allows a shared run (a run from
the pin of both wires while both go the same way, §3.4). EW-CORE-074 "Wires meet without joining" (F6), -075 "Text on
a text", -076 "Text on a symbol" and -077 "Symbol on a symbol" (F7, a conflict warning) are rows of
[15](15-checks-revisions-collaboration.md) §4.2. EW-TERM-006 is unchanged. `ew route` stores `layout.apply`.

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
   and the pages of the person-like test runs of each kind.

### 9.2 The measures, before and after

For each page `ew layout` prints faults, unrouted wires, cost, crossings, bends, wires straight, turn-backs,
detour and labels moved. The tests hold:

- every fixture, untidy, after Arrange and route: no fault, no unrouted wire, no turn-back, no wire across a
  text, no text on a text; every wire between two pins that face each other is straight; every net with three or
  more taps is a trunk with straight drops;
- every fixture, as wired by hand, after Route: an empty plan (nothing better is found), and after Arrange and
  route: an empty plan;
- the full sheet: Route and Arrange and route end, with no fault.

The private pages are measured outside the repository: no page worse in any figure.

### 9.3 The tests of each part

No test pins the total of a growing set, a generated name or a path; each states a property of a small page.

| Row | Part | Tests |
|---|---|---|
| 1 | the text placer (§5) | Two parts whose tags' first places overlap: both tags are clear. A wire across a tag's first place: the tag stands at its second. A moved text and a box's pin texts are where they were. The measure counts a turn-back on a wire drawn round a caption. The name of a mark stacked under another stands in line beyond its arrow, and a mark alone keeps its author's place |
| 2 | the net router and its trunks (§3.4) | A potential wired from pin to pin over three parts is one line with two drops. A T junction moves onto the trunk and keeps its three wires and its sequence. A wire the person shaped is in no page-wide plan; a broken one is, flagged. The same page with its lists shuffled gives the same plan. A plan never changes which pin a wire ends at. A junction with two drops stands over one of them and its net has no fault. A tap may stand on a line across between a pin and the pins that face it |
| 3 | the tracks (§3.5) | Two-pole device over a two-pole device of another pitch: one pole straight, one step, no crossing. Five sensors over pins two steps apart: five wires step in and none crosses |
| 4 | the placer (§4, §1.6) | A sensor below a card whose pin faces up stands above it after the run. A circuit that stands above the trunk that feeds it stands below it after the run; an actuator below its valve stands above it. Two circuits whose drops cross change places, two whose drops do not cross keep theirs. A part with no wire does not move. `rules_kept` holds on every plan. A second run gives an empty plan. Unticking one part gives a plan in which it has not moved and nothing overlaps. One Undo restores every place and route. The marks of a strip, scattered, stand in stacks and their wires nest: one corner each, none crossing. The parts a potential feeds from pin to pin stand level under one rail. Field devices stand in a row over their terminals, the terminals in line over their pins, the supply's riser beside the row: no crossing. On a valve terminal the cylinders stand above in the valves' order, the service unit below, no tube crossing. A part wired by a hairpin stands at its host, off the pin's line. A strip is one row for A4, a hairpin is not bound by E4, and a chain may take the chain gap and open by a row's pull and no more. When every row of the preview is unticked the toast says that nothing is left to apply |
| 5 | keep in place (§2.4) | A locked part is in no plan; it is a host for the others |
| 6 | the guides (§6) | A pointer 0.7 step from an In-line guide's line puts the part on it; 0.8 step does not; with Alt neither guide nor nudge |
| 7 | cards drawn from the station view, and marks (§7.5, §5.3) | An input card drawn from the station view has every channel pin on its upper edge, `CARD_PIN_PITCH` apart. Two marks of the starter library stack two steps apart without their texts meeting |
| 8 | one number for a grid position (§2.1) | On a page of the QElectroTech grid a pin's point equals `Grid::at` of its position |

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
  way, `RESETS` times at most after the first (`RESETS` + 1 settings). On a page where a card would step along its edge the whole run is made three times
  (§4.4). So the work of a press grows with the wires of the page times a bounded number of
  plans, and with the number of trees times the steps tried for each.
- Trunks: nets × candidates (a handful) × taps. Tracks: per channel a sort of its wires.
- The search: per remaining wire, the nodes of its area (at most `MAX_NODES`, five states each) times a
  logarithm; areas grow from the ends' box only when no route is found; `PASSES` passes.
- The occupancy is built once per run and changed per wire, not rebuilt per wire.

On a full sheet the search has the most work; the straights, trunks and tracks take most wires away from it, so no
wire is searched against every other.

---

## 10. Libraries

None is added. `petgraph`, `pathfinding` and `rstar` would fit by licence, but the page is small, the search exists,
and a bucket grid serves the text placer; `rust-sugiyama` is not used because the placer is not a layered layout.
Nothing is ported from libavoid, ELK or OGDF.

---

## 11. Where this design is unsure

1. **Texts derived, not stored** (§5.5). It is the lean choice and matches the tag rule; it means a text can stand
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
   (§4.8) gets Route's plan and no move; wires that have to go round their own card are nested by the router as
   rings (§3.5).
4. **The weights** (§1.2) are first values. The text weight is one number for a wire across a text, where a rule of
   thumb would need two; the private pages named in ADR-0033's amendments are its test.
5. **The rules of §1.6 are few on purpose.** They say what binds on every page of a kind. What a draughtsman
   also does (the loads at the foot of the sheet, like circuits drawn alike) is left to the measure and the
   person; a rule is easy to add when pages show the need. A part whose role the adapter cannot tell is bound
   by the general rules only, except that a one-pin end symbol that is no mark stands beside the pin it is wired to
   on a short hook, as an earth symbol does.
6. **No readability study of circuit diagrams exists** (research §9): the ranking of crossings, bends and length
   comes from general graphs and from the trade's practice.
7. **Bevels by default** (§7.6) make every joint of a new drawing a chain in the wire list. It is the pilot's
   choice and the trade's notation; a user who wires in stars asks for the dot each time, until a setting is
   wanted.
8. **The reference set is one hand.** The defaults taken from it (two steps between wires, five between devices)
   and the card pitch of §7.5 are constants, and a second user may want settings.

