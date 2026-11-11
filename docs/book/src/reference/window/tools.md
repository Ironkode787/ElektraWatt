# Tools

A tool decides what a click on the page does. One tool is active at a time; Select is active when no other is. The
Draw tab of the command bar holds the tools, and each is in the Draw menu at the path given below. Help ▸
Documentation with a tool active opens its section here.

The keys of every tool are on the [keymap](../keymaps.md) page, for each keymap preset. A tool's own keys work only
while it is active. `Esc` or a right click on the page cancels what the active tool is doing, one step at a time,
and ends at Select; with Select active, a right click opens the page's menu.

## Select

Selects, moves and opens what you click: a click selects one item, a drag draws a selection band, a double click
opens the item's properties. Open it with Draw ▸ Select.
See [Use the application window](../../how-to/application-window.md).

## Measure

Measures the distance between two points on the page, in millimetres of the sheet. Open it with Draw ▸ Annotate ▸
Measure.

## Symbol

Places a symbol from the library: double-click the symbol in the browser (its categories at the left, a picture of
the chosen row at the right), then click where it goes; each click places one more until you finish. Open it with
Draw ▸ Place ▸ Symbol…. See [A first schematic page](../../tutorials/first-page.md).

## Device from part

Places a device from a part of the library, so that the device has its part from the start. The browser lists one
row per part; a part with several functions opens to them with its arrow (or Right). Open it with Draw ▸ Place ▸
Device from part…. See [Choose parts](../../how-to/choose-parts.md).

## Black box

Draws a black box: a rectangle with connection points for a device that the library has no symbol for. Open it with
Draw ▸ Place ▸ Black box….

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
click in between. Open it with Draw ▸ Connect ▸ Wire. See [A first schematic page](../../tutorials/first-page.md).

While you draw, the wire shows the route it will take. The route never runs through another connection point, along
another wire, through a symbol or a text, over the frame's bands, or through a junction, and never makes a fourth
arm of a junction. When no such route exists the wire shows as a dashed right-angled line and the hint
says "No clear route: the wire stays unrouted. Click to place bends." Ending it there stores an unrouted wire: it
keeps its connection, but it is drawn dashed and the check reports it until it has a route. Click on empty points
to place bends and the wire follows them ("Bend placed: click a connection point to end the wire."). A junction
that already joins three wires is not an end: the hint says "This junction already joins three wires: end on a wire
to make a new junction."

An unrouted wire, from an import or a placement, is drawn as a dashed line between its ends, straight or with
one corner and never diagonal, on the page and in the PDF. The line takes no notice of what lies under it: it is
no route yet. Route page or Route selection draws it.

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

## Edit route

Changes the drawn route of a wire with the mouse. Select the wire: a handle shows on each of its segments and a
dashed one on each bend. Drag a segment and it moves sideways; drag a bend and the corner moves. The wire stays
right-angled and its ends stay on their connection points: a first or last segment that is dragged keeps a short
stub at its end and jogs after it, so a straight wire dragged sideways goes round what was in its way. A dashed line
shows the new route while you drag, and one undo takes the change back. A wire without a route, drawn dashed, takes
its first route this way too. Draw ▸ Connect ▸ Edit route does the same from the keyboard: `Tab` chooses the
segment, the arrow keys move it, `Enter` stores it.

## Route page

Routes every unrouted wire of the page on show in one step. Open it with Draw ▸ Connect ▸ Route page. The Preview
tab shows the plan first: "Route 3 wires on folio 101: 2 routed, 1 not routed.", and for each wire that no route
keeps clear, why, such as "No route keeps clear of other wires: -101F1:2 → -101E55:L." Apply draws the routes, and
one Undo takes them back; the toast says "Routed 2 wires on folio 101." When every wire of the page has a route it
says "Every wire on folio 101 has a route." instead. The router routes supply wires and long wires first, keeps the
wires of a feed side by side in the order L1, L2, L3, N, PE where their ends allow it, and keeps every route clear as
the Wire tool does.
A wire it cannot route stays unrouted; move a symbol to make room, or place its bends with the Wire tool.

## Route selection

Routes the selected wires, and every wire with an end on a selected symbol, again, routed or not; the other wires of
the page stay as they are. Open it with Draw ▸ Connect ▸ Route selection. It shows its plan and applies it as Route
page does.

## Interruption point

Places an interruption point: the end of a potential on this page that continues at a point of the same name
elsewhere. Open it with Draw ▸ Connect ▸ Interruption point.

The arrow always points right, the way the signal flows. A source, where the potential leaves the page, has its wire
on the arrow's tail and belongs at the page's right edge; a target, where the potential arrives, has its wire on the
arrow's tip and belongs at the left edge. The name is above the arrow and the cross-reference below it. The arrow is
never rotated or mirrored: change the point's direction instead. While you draw a wire, the interruption point's key
ends the wire in a point of the right form for that end.

## Interruption point direction

Turns the interruption point that the tool floats to its next direction. Draw ▸ Connect ▸ Interruption point
direction.

The point takes the form of its new direction: the wire moves from the tail to the tip, or back. Changing the
direction of a placed point in its properties does the same, and is refused when the project does not hold the other
form yet: place one point of that direction first.

## Star source

Marks the interruption point as the source of its name in star mode: one source, many targets. Draw ▸ Connect ▸
Star source.

## Wire marker

Places a wire marker, the label that prints a wire's number or text on the wire. Open it with Draw ▸ Connect ▸ Wire
marker.

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

On a panel layout page, draws a mounting rail. Open it with Draw ▸ Rail.

## Duct

On a panel layout page, draws a wiring duct. Open it with Draw ▸ Duct.
