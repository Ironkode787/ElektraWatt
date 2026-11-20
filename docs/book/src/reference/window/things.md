# Things on a page

What a page holds, one section for each kind of thing: what it is, how you make one, how you change it, and where
its rows of Properties are explained. Point at a thing on the page, or select it, and press `F1` to open its section
here. The rows of Properties have their own page: [Properties](../properties.md).

## Devices and symbols

A device is one thing of the cabinet or the machine, such as a contactor -K1, a motor -M1 or a lamp -E2. A symbol
draws the device, or a part of it, on a page; a device can have several. A contactor has its coil on one page and its
contacts where they switch, and each contact names the place of its coil under it, the coil the places of its
contacts.

To make one: **Draw ▸ Place ▸ Symbol…** places a symbol and makes its device, **Draw ▸ Place ▸ Device from part…**
places a device with its part from the start ([Tools](tools.md#symbol)). A right click on a relay's or a contactor's
coil offers **Add contact**.

To change one: select it and change its rows in Properties: its tag, its part (**Choose part…**), what it does. A
right click offers Rotate, Mirror, **Edit symbol** and **Join device…**. Its rows are explained under
[Device](../properties.md#device), [Function](../properties.md#function) and
[Representation](../properties.md#representation).

To keep a symbol where it stands: select it, right-click and choose **Keep in place**, or tick **Keep in place** in
Properties. Arrange and route then leaves it where it is and arranges the other symbols around it. It does not hold the
symbol for you: you can still move it yourself, and Route still routes its wires. A small pin shows on it while it is
selected. Choose **Keep in place** again, with its check showing, to let it go.

## Wires

A wire joins two connection points. A junction, the dot where a wire branches, joins three. A wire's number,
cross-section and colour come from its potential and the project's profile until you give it its own.

To make one: **Draw ▸ Connect ▸ Wire** (`W`), then a click on a start point and one on the end point
([Wire](tools.md#wire)).

To change one: select it. The selected wire shows round handles; drag one to change its route
([Change a wire's route by hand](tools.md#change-a-wires-route-by-hand)), or choose **Reroute** to route it again
([Reroute](tools.md#reroute)). Its number, colour and cross-section are its rows in Properties, explained under
[Connection](../properties.md#connection).

## Cables and their marks

A cable bundles wires: each wire of it is one of its cores, with the core's colour and cross-section. The cable's mark
on the page is the line across its wires, a slanted stroke on each core with the core's name beside its wire, and the
cable's tag with its type at the line's start.

To make one: select the wires, then **Draw ▸ Connect ▸ Cable…**. Its form proposes the tag and lists the cores; its
**Cable type** lists the cable parts that fit, and **New cable type…** makes a new one. The mark is placed at once
([A terminal strip and a cable](../../tutorials/terminal-strip-and-cable.md)).

To change one: drag the mark along the wires, or select it and drag its tag. The cable's length, type and cores are
its rows in Properties, explained under [Device](../properties.md#device); the Cables navigator lists every cable
with its cores ([Cables](panels.md#cables)).

## Terminals and terminal strips

A terminal is a point where a wire is clamped, on a terminal strip such as -X1. Each terminal of a strip has its
number (-X1:1, -X1:2) and its kind: feed-through, protective earth and others.

To make one: **Draw ▸ Place ▸ Terminal…** lays a row of terminals of one strip ([Terminal](tools.md#terminal)). A
right click on a terminal offers **Add terminal**, the strip's next number.

To change one: select it and change its rows in Properties, or choose **Edit ▸ Strip editor** for the whole strip as
a table ([Strip editor](panels.md#strip-editor)). Its rows are explained under [Device](../properties.md#device).

## Interruption points

An interruption point ends a potential on this page; it goes on at the point of the same name on another page. The
arrow points the way the signal flows: a source at the page's right edge, its target at the left edge of another page.

To make one: **Draw ▸ Connect ▸ Interruption point**, type its name and click where it goes
([Interruption point](tools.md#interruption-point)).

To change one: select it and change its name or its direction in Properties. A right click offers **Go to partner**
and **Continue on page…**. Its rows are explained under [Function](../properties.md#function).

## Supply points

A supply point is an interruption point where the supply comes in from another drawing set: L1, L2, L3, N and PE
from a main distribution, say. It prints where the supply comes from instead of a partner on another page. Supply
points that stand together, one under the other, print that place once.

To make one: **Supply** on the Draw tab, beside Interruption, asks what comes in (single phase L, N, PE; three phase
with or without N; 24 V DC; your own names) and where it comes from, and the marks follow the pointer as one group
until you click. Or draw the points with Interruption, select them, right-click and choose **Supply comes in here…**;
its form proposes where the supply comes from and the device there. When the marks are placed, a small panel opens at them and asks what comes
in: a row per mark, with the kind of conductor (L1, L2, L3, L, N, PE, L+, M, …) and the voltage with AC or DC
(`230 V AC`), filled in from the mark's name where the name tells it (`L1-EXT` is a phase at the project supply's
voltage, `N` the neutral, `PE` protective earth, `24V` a 24 V DC plus, `0V` or `M` its minus). Enter defines every
row's potential in one step; Escape places the marks without one. A mark on a potential that has its type already is
not asked. **Potential…** on the marks' right-click menu opens the panel again to change it.

To change one: the place and the device are its rows in Properties; a value typed there moves the point and its
group. A right click offers **Group these supply points**, **Take out of its group** and **Remove the supply mark**.
Its rows are explained under [Function](../properties.md#function).

## PLC cards and stations

A PLC station is a CPU, a head station or another device with a rack; its cards (modules) sit in the rack's slots,
each with its channels. On a page a card is drawn as a box with its channels' pins, and a station as the box of its
head.

To make one: **New station…** on the PLC tab, then put cards into its rack in the station's view
([Make a station](../../how-to/plc-workspace.md#make-a-station)). **Draw on page** in the view draws a card on a page.

To change one: open the station's view from the PLC navigator, or select the card and change its rows in Properties
([Read and change a rack](../../how-to/plc-workspace.md#read-and-change-a-rack)).

## Valves and cylinders

A valve, a cylinder and the other parts of a pneumatic or hydraulic circuit stand on a fluid page with their ports. A
valve worked by a solenoid has the solenoid's coil on an electrical page too.

To make one: open the Fluid tab on a fluid page and choose the valve or the cylinder
([Draw a circuit](../../how-to/fluid-workspace.md#draw-a-circuit)).

To change one: select it; its medium and its solenoid are rows in Properties
([The solenoid of a valve](../../how-to/fluid-workspace.md#the-solenoid-of-a-valve)).

## Tubes

A tube joins the ports of fluid parts, the way a wire joins connection points. A pilot or drain tube is dashed, a hose
is drawn as an arc.

To make one: **Tube** on the Fluid tab ([Tube](tools.md#tube)).

To change one: right-click it for its line kind; its tube and length are its rows in Properties
([Tubes and the tube list](../../how-to/fluid-workspace.md#tubes-and-the-tube-list)).

## Texts

A free text is a text of its own on the page, such as a note; it can show a value of the project, such as its title.
The texts of a device, its tag and its function text, belong to its symbol.

To make one: **Draw ▸ Annotate ▸ Text** ([Text](tools.md#text)).

To change one: select it and change its rows in Properties (a double click opens them). Drag a device's tag to move it beside
its symbol. How big texts print is set for the whole project in **Text styles…**
([Text styles](../../how-to/start-a-project.md#text-styles)).

## Graphics and images

A graphic is a line, a rectangle, a circle or another shape without electrical meaning; an image is a picture file on
the page.

To make one: **Draw ▸ Annotate ▸ Graphic** or **Draw ▸ Annotate ▸ Image…** ([Graphic](tools.md#graphic)).

To change one: select it and drag it, or change its rows in Properties.

## Boxes

A structure box is the dashed frame round devices that belong to one function or one location of the project, such
as one machine part. A black box is a device drawn as a rectangle with its connection points, for a device the library
has no symbol for.

To make one: **Draw ▸ Annotate ▸ Structure box** ([Structure box](tools.md#structure-box)) or **Draw ▸ Place ▸ Black
box…** ([Black box](tools.md#black-box)).

To change one: select it; a selected black box shows grips on its corners and connection points.

## The frame and title block

The frame round the page, with its column numbers and row letters (the cells such as 101-B6 that cross-references
name) and the title block at the bottom. The frame comes from the project's template.

To change it: right-click the title block and choose **Title block fields…** for the texts it prints, such as the
drawing's title and who drew it; a page's own texts are its **Page properties**.

## Lists on a page

A list is a page the program writes: the parts list, the wire list, the terminal diagram, the cable list, the list of
PLC cards or of tubes. It is filled from the drawing every time, so you do not type into it.

To make one: **Report** on the Output tab (**Output ▸ Reports ▸ Report block…**) ([Reports and PDF](../../tutorials/reports-and-pdf.md)).

To change one: change the drawing; the Report blocks navigator lists the lists ([Report blocks](panels.md#report-blocks)).
