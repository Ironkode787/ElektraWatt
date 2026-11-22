# Tools

A tool decides what a click on the page does. One tool is active at a time; Select is active when no other is. The
Draw tab of the command bar holds the tools, and each is in the Draw menu at the path given below. `F1`
with a tool active, or over its button, opens its section here.

The keys of every tool are on the [keymap](../keymaps.md) page, for each keymap preset. A tool's own keys work only
while it is active. `Esc` or a right click on the page cancels what the active tool is doing, one step at a time,
and ends at Select; with Select active, a right click opens the page's menu.

While a symbol hangs on the pointer or is dragged, a dotted line shows where one of its pins would stand in line with
a pin it faces, and a faint one where it would stand level with a symbol drawn alike beside it. Within three quarters
of a grid step of the first line the symbol takes it and the line turns solid; move on and it lets go. The symbol
always stands on the grid, and nothing moves it after you set it down. Hold `Alt` for no guides; Alt also places or
moves it without connecting it.

The page's menu is the quick way to what fits the thing under the pointer: its first entries are that thing's, then
come the general ones (Rotate, Mirror and Renumber devices… on a symbol; Cut, Copy, Paste, Delete, Properties of
selection). A right click selects the thing first; Shift+F10 or the menu key opens the same menu for the selection.

- On a wire: Route, Wire marker, Define potential…, Cable… and Print wire numbers; a wire does not turn, so
  there is no Rotate or Mirror. On several selected wires: Make cable from the selection….
- On a terminal: Add terminal (the strip's next number, of the terminal's kind and part, floating at the pointer),
  Choose part… and Edit strip, which opens the strip editor. On several selected terminals: Choose part…, for them
  all, and Spacing…, which spaces a row of them the grid steps you choose apart from the leftmost (their wires
  follow, one undo step). A strip's row in the Terminal strips navigator offers Add terminal (with the part of the strip's last terminal
  of that kind), Choose part for its terminals without one, and Edit strip.
- On a relay's coil: Add contact, one entry per free contact of the relay by name; it floats the contact's symbol for
  a click. On a contact: Go to coil.
- On a symbol of a device: Join device… (only where another device can take it, such as a contact another relay
  has free), Choose part…, Edit symbol and Copy and edit.
- On an interruption point: Supply comes in here…, Go to partner, Direction: Source or Target, and Continue on page….
  Supply comes in here… says that the supply enters here from another drawing set (L1, L2, L3, N, PE from another
  drawing): its form proposes where the supply comes from and the device there, and one Apply marks every selected
  point; the points then print where they come from and are no longer reported as unpaired, and a panel at them asks
  what each carries (its kind of conductor and voltage; Enter takes them, Escape leaves them without). On a marked
  point the entry reads Remove the supply mark, and Potential… opens that panel again. The same link stands under the
  point's name in Properties.
- On the title block: Title block fields…. On an empty spot: Paste, Page properties, New page… and Print wire numbers.

A navigator's row offers what the same thing offers on the page; a right click on a row selects it first. A device's
row offers Go to (its first placement), its symbol's entries and Properties of selection; a cable's row Cable… and Go
to; a page's row Open page in a new tab, Title block fields…, Page properties, New page… and Delete page (the right
click opens the page). See [Navigators](panels.md#navigators).

An entry that cannot be used now is greyed out, and its tooltip says why.

## Select

Selects, moves and opens what you click: a click selects one item, a drag draws a selection band, a double click
opens the item's properties. Open it with Draw ▸ Select.
See [Use the application window](../../how-to/application-window.md).

## Measure

Measures the distance between two points on the page, in millimetres of the sheet. Open it with Draw ▸ Annotate ▸
Measure.

## Symbol

Places a symbol from the library. A symbol is a drawing without a part; the symbol browser lists the symbols alone
(the parts are the device browser's, the terminals Terminal's). Double-click the symbol in the browser (its
categories at the left, a picture of the chosen row at the right), then click where it goes; each click places one
more until you finish. Under the picture, **Edit symbol** opens the symbol in the symbol editor, where its next
version is saved to its library; a symbol of a read-only library (the starter's) or one only the project holds offers
**Copy and edit** alone, which opens a copy that is saved to your own library. Open it with Draw ▸ Place ▸ Symbol….
See [A first schematic page](../../tutorials/first-page.md).

## Device from part

Places a device from a part of the library, so that the device has its part from the start. A device is a part of
the catalogue with its functions. The browser lists one row per part; a part with several functions opens to them
with its arrow (or Right). Edit symbol and Copy and edit act on the symbol of the chosen row, as in the symbol
browser; a part drawn as a box offers **Pins and box…** instead, which opens its device box in the symbol editor (a
copy of your own for a part you cannot change). Open it with Draw ▸ Place ▸ Device from part…. See [Choose parts](../../how-to/choose-parts.md).

## Terminal

Asks which strip (one of the project's, or a new strip with the tag it will take), which kind of terminal
(feed-through, protective earth and the others of the page's library), which part, how many and from which number
(empty: the strip's next free number), and the Spacing between the terminals in grid steps, each shown with its
length ("2 grid steps · 8.0 mm"; the project's next row takes the spacing chosen last). The Part list holds No part and the terminal parts of the project and the
installed libraries. It proposes the part of the strip's last terminal of that kind, and for a new strip the starter
part of the kind (EW-GEN-TRM-1 for a feed-through terminal, EW-GEN-TRM-2 for a protective-earth terminal); another strip
or kind proposes again, and a part you choose sets the kind it is drawn as. Place then floats the first terminal; one
click lays the whole row, each terminal the Spacing right of the one before and each with the part, and the tool
ends; one Undo takes the row back. Draw ▸ Place ▸ Terminal….

## Black box

Draws a black box: a rectangle with connection points for a device that the library has no symbol for. Open it with
Draw ▸ Place ▸ Black box….

A box drawn without a part takes pins where it stands. Select the box and choose **Add pin** (Draw ▸ Arrange, beside
Rotate and Mirror): the new pin follows the pointer along the box's edge nearest it, and a click puts it there.
Right-click the box and choose Add pin to put the pin on the edge nearest the click at once. A field then asks the
pin's name; Enter adds it, Tab adds it and asks for the next pin further along the same side. Two pins may have one
name (two 24 V inputs), as a part's pins may; where a name cannot be taken, the line under the field says why. A
wire drawn to the box's edge where no pin is asks for a pin there in the same way and ends at it. Double-click a pin
to rename it; right-click it for **Remove pin**, offered while no wire holds it. Each is one Undo step. The box goes
no narrower than its pins need and no lower than its longest top pin name and its longest bottom pin name need (those
two sides' names are turned a quarter, reading from bottom to top; the left and right sides' are horizontal). A box
of a part draws the part's pins: its Add pin opens the part's device box, where they are edited.

## Tool step back

Takes back the last step of the active tool, such as the last corner of a wire, without leaving the tool. It has no
menu item; use its key.

## Rotate clockwise

Turns the symbol that the tool floats, or the selection, by 90° clockwise. Draw ▸ Arrange ▸ Rotate clockwise.

## Rotate counter-clockwise

Turns the floating symbol, or the selection, by 90° counter-clockwise. Draw ▸ Arrange ▸ Rotate counter-clockwise.

## Mirror left to right

Mirrors the floating symbol, or the selection, about its vertical axis. Draw ▸ Arrange ▸ Mirror left to right.

## Mirror top to bottom

Mirrors the floating symbol, or the selection, about its horizontal axis. Draw ▸ Arrange ▸ Mirror top to bottom.

## Next variant

Switches the floating symbol to its next variant, such as another contact arrangement of the same function. Draw ▸
Arrange ▸ Next variant.

## Next representation type

Switches the floating symbol to its next representation type, for example from the multi-line to the single-line
form. Draw ▸ Arrange ▸ Next representation type.

## Move

Moves the selection with the pointer or the keyboard; wires attached to it follow. Draw ▸ Arrange ▸ Move.

When you let go, a wire whose followed route would now run through a symbol, a text, a connection point or along
another wire, or would bend more than before, is routed again; a wire that stays clean keeps its shape. Both happen
in the same undo step as the move.

## Finish

Ends what the active tool is drawing, such as a wire or a polyline, at the current point. It has no menu item; use
its key or double-click where a tool allows it.

## Wire

Draws connections between connection points: click a start point, then the end point; corners are placed where you
click in between. Open it with Draw ▸ Wire. See [A first schematic page](../../tutorials/first-page.md).
A click takes the connection point within about ten screen points of it, at any zoom, and the ring under the pointer
shows which. A click that misses and lands right beside one is said in the status line at once ("The click placed a
bend beside a connection point, not on it: …"), as is a start beside one that took nothing and a wire just drawn that
bends beside one without joining it; Ctrl+Z takes such a wire back.

While you draw, the wire shows the route it will take. The route never runs through another connection point, along
another wire, through a symbol, over the frame's bands, or through a junction, and never makes a fourth arm of a
junction. It keeps clear of texts too, unless the only way round them is far longer (round a group of symbols or
the sheet) or there is none: then the route crosses the text and the hint says "The route crosses a text: keeping
clear of it is a far longer way." A device tag on the wire then moves to a free place beside its symbol. When no route exists at all the wire shows as a dashed right-angled
line and the hint says "No clear route: the wire stays a dashed line without a route. Click free points to lead it,
or route it later with Route." Ending it there stores an unrouted wire: it keeps its connection, but it is drawn
dashed and the check reports it ("Wire … has no route: it is drawn as a dashed line.") until it has a route. Click
on empty points to place bends and the wire follows them ("Bend placed: click a connection point to end the wire.");
a click inside a symbol places no bend and says so ("A bend cannot sit inside a symbol; click one of its connection
points, or a free point outside it"). Where no clean way leads on from your last bend, the wire is routed from its
start as if drawn anew, without your bends, or stays unrouted: a wire never runs through a pin or along another wire.
The hint says what a click at the pointer does: on another wire it joins that wire with a tee, drawn with a bevel
(Alt+Click places a bend there instead), on the start point it does nothing. A junction
that already joins three wires is not an end: the hint says "This junction already joins three wires: end on a wire
to make a new junction." The point where two wires cross is not an end either (a junction there would join all
four arms): end on the wire a step beside the crossing.

An unrouted wire, from an import or a placement, is drawn as a dashed line between its ends, straight or with
one corner and never diagonal, on the page and in the PDF. The line takes no notice of what lies under it: it is
no route yet. Route draws it.

A route the tool finds between your two clicks counts as the program's: Route may draw it again later. A route with a
bend you placed is yours, and so is one you change by hand; Route leaves it alone unless you select the wire.

## Dot at the joint

A wire that ends on another wire joins it with a bevel: the wires then run from terminal to terminal, and the bevel
shows which terminal carries two of them. Press `.` while you draw the wire, before you click on the other wire, to
join it with a dot instead: then every wire of the joint starts from the terminal the potential comes from. Press `.`
again for the bevel. A joint you made keeps its form; change it later in the junction's editor (select the junction and
press Enter).

## Tube

The Wire tool on a fluid page, drawing tubes at once: open it with Tube on the Fluid tab (Fluid ▸ Tubes ▸ Tube). It draws
the line kind used last, Working at first; the bar under the page says the kind and the project's default tube, and its
Line kind button steps through Working, Pilot, Exhaust, Drain and Flexible. Draw ▸ Wire draws working tubes. A pilot or drain
tube is dashed; a flexible one (a hose) is drawn as an arc between its ends. Right-click a tube to change its line
kind.

## Next autoconnect proposal

While a symbol floats near other connection points, shows the next of the connections EW proposes to draw when you
place it. It has no menu item; use its key.

## Connect while moving

While moving a symbol, connects its connection points to the ones it lands on. It has no menu item; use its key.

Connect while moving is on when you start: the wires the moved symbol would make are shown while you drag and are
drawn with the move when you let go, in the same undo step. Hold the key that places without wires as you let go to
move without them. The key of Connect while moving turns it off, and on again, until you close the program; the
hint of the move and the status bar show the key followed by "Connect on move: on" or "off". A move with the keyboard
follows that setting.

## Change a wire's route by hand

Select the wire with Select: it shows as a band along its path, with a round handle in the middle of each segment and
on each bend; the handle under the pointer grows and the pointer shows which way it moves. Drag a segment's handle
(or the segment itself) and it moves sideways; drag a bend's handle and the corner moves. The wire stays
right-angled and its ends stay on their connection points: a first or last segment that is dragged keeps a short
stub at its end and jogs after it, so a straight wire dragged sideways goes round what was in its way. A dashed line
shows the new route while you drag, `Esc` puts it back, and one undo takes the change back. A wire without a route,
drawn dashed, takes its first route this way too. Only the selected wire has handles: where wires lie over one
another, click again on the same spot to select the next of them.

With the keyboard: select the wire, then `Tab` gives its first handle the focus and the handle shows as the one under
the pointer does; `Tab` and `Shift+Tab` go from handle to handle along the wire. The arrow keys move the handle one
grid step, each step one undo step, and `Esc` gives the focus back to the wire.

When it draws or routes a wire, the router never puts a corner of one wire on a corner or an end of another: it
reads as a joint. Between pins that stand close, such as two two-pole devices wired cross-over a few grid steps
apart, a wire may step half a grid step to cross the other once instead of going round.

## Route

Routes the wires of the selection (the selected wires and every wire with an end on a selected symbol), or with
nothing selected the wires of the page that need it: those without a route, those whose route is broken (it lies on
another wire, runs through a pin or a symbol) and those the program routed before. Symbols stay where they are. A
wire you drew yourself keeps its route unless you select it or it is broken. Open it with Draw ▸ Connect ▸ Route, or
Route in the menu of a wire or a symbol.

The wires of one potential are routed together. A potential that runs over several symbols, from pin to pin or
through junctions, becomes one straight line with a straight drop to each pin; the junctions move onto that line and
keep their wires and their order. Two wires of one pin may run on one line out of it and part where they go different
ways: the page draws the parting with a 45° bevel, the way two wires on one terminal are drawn. A wire between two
pins that face each other on one line is one straight line. What is left is routed as the Wire tool routes it, and a
tag that stands in a wire's way steps aside.

Route applies at once, in one undo step: the toast names the wires it changed, by their numbers or their ends, three
of them and then how many more ("Routed 5 wires: 101.3, 101.5, 101.7 and 2 more."), and the page shows them as
selected wires while the toast is up. A wire without a way keeps its route, or stays a dashed line, and the toast says
what is in its way, such as "No way for -101F1:2 → -101E55:L: other wires are in its way." Where Route would replace a
broken route you drew, the Preview tab shows the plan first and names those wires; Apply stores it, Cancel leaves the
page as it is.

With nothing selected, Route draws again by itself only the wires the program routed, the unrouted ones and the broken
ones. Every other routed wire counts as drawn by you, and so does every wire of a project made before this version:
the toast says how many keep their routes ("Nothing changed: 14 wires keep the routes they have."), and where routing
them anew would make the page better, its Route these too… shows that plan in the Preview tab first. To have some of
them drawn again, select them and choose Route. When no wire would run better, it says "Nothing to route: no wire
would run better."

## Arrange and route

Moves the symbols of the selection, or with nothing selected the symbols of the page, to where the page is best for
them, and routes their wires: every wire of the page, or of the selection and the symbols in it, also the wires you
drew yourself. Open it with Draw ▸ Connect ▸ Arrange and route, or Arrange and route in the menu of a symbol. Nothing
changes until you choose Apply.

Each symbol is put in line with the pin it is wired to, on the side that pin faces: a contact under the breaker that
feeds it, a sensor over its input's pin, a cylinder over its valve, a mark at the left end of the potential that comes
in. A symbol may cross the page for it, within the rules of a drawing of its kind: on an electrical page the
potentials stand at the head and the foot and current runs down a circuit, a card's channels keep their order; on a
fluid page the supply stands below the valves and each actuator above its valve, the actuators in their order; on a
bus page the stations stand in the order of the line, level. The space you left along a circuit stays: a symbol is
neither pulled nearer nor spread out, unless it has to move. A row you made (the terminals of a strip, cards side by
side, like symbols level) moves as one or stays as one; a terminal wired to a card stands in the column of its
channel, so a strip between a card and its field devices takes the order and the spacing of the channels. Every arrangement is routed and measured before you see it. A
move has a price, so a symbol moves only where the page gets visibly better, and a page already in order gives no
plan. It never offers less than Route would do with the same wires: when moving symbols does not make the page better
than routing alone, it offers Route's plan and says that no symbol moves. That is also what it does on a page wired
against its symbols, such as a card whose pins face up wired to the upper ends of terminals that stand below it: no
arrangement mends that, wire the terminals by the ends that face the card. A card drawn before this version has two
rows of pins, so half its channels face away from their circuits. You can draw it again without wiring it anew:
delete its box on the page (the card, its channels and its wires stay in the project), choose Draw on a page for it
in the station view, and run Arrange and route, which draws the wires to the new pins. A terminal whose card now
stands on its other side still has to be wired by the end that faces the card.

It never moves a symbol you keep in place (right-click ▸ Keep in place), a symbol outside the selection, a symbol in a
structure box, or a symbol to another page, and it never turns or mirrors one. The symbols you keep stay where they
are and the others are arranged round them.

The Preview tab shows what it would do: how many symbols move and how many of them across the sheet, how many wires
are routed and how many of those you drew, the page's crossings and bends now and then, a ticked row per symbol that
moves (the far moves first) with its move in grid steps, a ticked row per wire you drew that gets a new route, and what
is left as it is: the symbols kept in place,
with their pin, the symbols that stand against a rule of the drawing and found no better place on the side their pin
faces, and the wires without a way. The page shows
it as it would become: the symbols that move tinted, their old places as faint outlines joined to the new ones by a
dotted line. Hold B to see the page as it is.

Untick a row to plan again with that symbol where it stands, or that wire as it runs; the page and the rows follow,
and what you see is what Apply stores. The tick of the heading "Wires you drew yourself" unticks all your wires at
once, so that each keeps its route, and ticks them all again. A row you unticked stays out, and a plan made again
brings in no other wire of yours. When you have unticked everything, the card closes and says "Nothing is left to
apply: every change of this preview is unticked." Apply stores it in one step, and the toast says what
moved and what was routed ("Arranged folio 102: 4 symbols moved, 9 wires routed."); one Undo takes all of it back. In
the history the step is called Arrange and route, or Route wires when no symbol moved. When the card closes, a page
that was shown whole is shown whole again. When nothing would make the page better, it says "Nothing to arrange: the
page is already arranged as well as the rules allow." Where something stands in the way, it says "Nothing to arrange:
no plan was better than the page as it stands." and names it: a symbol left against a rule, a wire without a route,
the faults the page keeps.

## Interruption point

Places an interruption point: the end of a potential on this page that continues at a point of the same name
elsewhere. Open it with Draw ▸ Connect ▸ Interruption point.

The arrow always points right, the way the signal flows. A source, where the potential leaves the page, has its wire
on the arrow's tail and belongs at the page's right edge; a target, where the potential arrives, has its wire on the
arrow's tip and belongs at the left edge. The name is above the arrow and the cross-reference below it. The arrow is
never rotated or mirrored: change the point's direction instead. While you draw a wire, the interruption point's key
ends the wire in a point of the right form for that end.

Type the name in the field Interruption point name at the page's top left, once for a pair: the tool then floats the
same name as the first point's partner, on this page or the next: its target after a source, its source after a
target that no source feeds yet. The name stays in the field until you change it: once the pair is whole, each
further click places one more target of the same source, and the source's cross-reference lists every place it goes
to. Without a typed name the tool offers the partner of a point that still waits for one (a target without a source,
a source without a target), and a target of a name that has its source keeps that name; only otherwise does a point
get a new name. A potential comes from one source and goes on at its targets. Source and Target beside the name field
choose the direction, as D does, and the choice holds until you change it: with Target chosen, every click places one
more target, its source drawn or not, and the bar says when the name still has no source ("next: one more target of
24V, which has no source yet"). After a source the tool turns to Target by itself, as a name has one source, and the
bar says so before the click ("next: a target, where 24V goes on"). When you
choose the tool from the Draw tab or the menu, the field has the keys with its name selected: type the
name and press Enter or Tab, then click where the point goes. While the field has the keys, D is a letter of the name
and the bar says "Enter ends the name"; after Enter, Tab or a click on the page, D turns the point. The toast of a
placed point says what it is and where its partner is ("L1: source here at 101-B2, its target at /102.3"), or that
the partner is still to come.
A point dropped on a pin, or with the pointer as near to it as the Wire tool catches a pin, joins that pin, whichever
its form, and no other point's text keeps it off the pin. Alt places it without joining; a point left beside a pin
that has no wire says so when it is placed, and the check's message for that pin offers to join the two. A free point
too close to another says how far apart the two must stand.
A point names the potential it carries whichever its direction, so a wire on a target that no source feeds yet
carries the target's name.

A point keeps room for its texts: where its name or its cross-reference would print over another interruption
point's, it is not placed, and the message says so. Three grid steps below or above the other point is clear;
points side by side are clear when their texts are. Hold Alt to place it anyway.

## Interruption point direction

Turns the interruption point that the tool floats from source to target, or back. Draw ▸ Connect ▸ Interruption
point direction. An undirected point, which pairs either way, is a choice of a placed point's Direction in its
properties.

The point takes the form of its new direction: the wire moves from the tail to the tip, or back. Changing the
direction of a placed point in its properties does the same, and is refused when the project does not hold the other
form yet: place one point of that direction first.

## Star source

Marks the interruption point as the source of its name in star mode: one source, many targets. Draw ▸ Connect ▸
Star source.

## Wire marker

Places a wire marker, the label that prints the cross-section and colours of the wires it marks. Open it with
**Marker** on the Draw tab or Shift+W. Drag a line across the wires, type the cross-section and the colours in the
tool's fields if they are to change, and press Enter. The fields may be filled before or after the line: a field
takes its value when you leave it (Enter, Tab, a click on the other field or back on the page), and the marker
prints what the fields show. A colour is a code in any case (`BK`, `bk`), two codes of one
wire (`GN+YE` or `GNYE`), a company code of the project's colour profile, or a colour's name from that profile; one
colour is every marked wire's, and a list (`BK, BU`) gives each wire its own in the order the line crosses them. A
colour the project does not know is refused with the codes it knows. The marker prints a short line across the wires
and its text beyond the line's end, clear of the wires.

The wires' numbers print on the page with **Draw ▸ Connect ▸ Print wire numbers** or the checkbox of that name in the
project's Properties, one setting for the whole project that the PDF and print follow too: each wire's number stands beside it, once, at the middle of its longest segment, and on a
wire that carries its own marker the number comes first in the marker's text (`101.1 1.5 BK`). A cable's core prints
no number; its cable and core name it.

## Text

Places a free text on the page. Open it with Draw ▸ Annotate ▸ Text.

## Graphic

Draws a graphic: a line, polyline, rectangle, circle, arc or polygon, which has no electrical meaning. Open it with
Draw ▸ Annotate ▸ Graphic and the shape.

## Structure box

Draws a structure box: a dashed frame that puts what it encloses into a function or location of the project's
structure. Open it with Draw ▸ Annotate ▸ Structure box.

## Image

Places an image file (SVG, PNG or JPEG) on the page. Open it with Draw ▸ Annotate ▸ Image….

## Place device

On a panel layout page, places a device of the location on a rail or the mounting plate. Open it from the Unplaced
devices navigator, which lists the devices of the location that have no place yet.

## Rail

On a panel layout page, draws a mounting rail. Open it with Draw ▸ Rail. The page shows an enclosure first: Project ▸
Setup ▸ Enclosure… puts the new enclosure's plate on it ([Draw a panel layout](../../how-to/panel-layout.md)).

## Duct

On a panel layout page, draws a wiring duct. Open it with Draw ▸ Duct.
