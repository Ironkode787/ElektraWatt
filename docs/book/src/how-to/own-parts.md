# Make your own parts

Every part, the starter parts and your own, lives in one place: **Parts** (**Project ▸ Parts**, `F9`). On the left is
the list with its search and its filters, in the middle the open part's sheet, on the right its pictures and **Ready
for**. A part you make is typed from its data sheet: nothing here depends on what ElektraWatt knows about a
manufacturer's product. **Save** puts it into the project and into your own parts library, **My parts**, so that your
next project has it too.

You need an open project.

## Start a new part

1. Choose **New part** in the bar of Parts, or **New part…** in the device browser (**Draw ▸ Place ▸ Device from
   part…**, `Shift+I`) or in the part dialog (**Choose part…**).
2. The sheet asks first: **What is it?** Its cards stand in four groups, Devices; Terminals, cables and wiring;
   Cabinet; Pneumatics, each with a line of what it will ask for. Choose the card of your part: **Device with
   symbols** for a contactor or a breaker, **Device drawn as a box** for a power supply or a device on a bus, **PLC
   card or station**, **Pushbutton or lamp in a door**, **Terminal**, **Cable**, **Wire**, **Ferrule or lug**,
   **Marker**, **Cable gland**, **Enclosure**, **Mounting plate**, **Rail**, **Wiring duct**, **End bracket or
   accessory**, **Valve**, **Tube** or **Other pneumatic device**.
3. The sheet shows what that kind needs and nothing else. Type the **Maker**, the **Part number** and a
   **Description**: these are all a part needs to be saved. A maker your libraries do not have yet is made with the
   part.
4. Type what the data sheet gives: a terminal's clamps and width, a cable's cores and outer diameter, a cabinet's size,
   plate and doors. **Ready for** on the right says, per feature, what the part still lacks and what follows from it
   (for the cabinet layout, a part without its size stands on the layout as a 20 × 20 mm square marked unknown size); a
   group of the sheet that lacks something has a dot in the list of groups.
   Leave what you do not know: the part saves without it, and Ready for keeps saying it.
5. **Save**. From the part dialog the button reads **Save and use for -102Q1**: the part goes to the devices you were
   choosing for, and you are back on the page.

A field that does not read is refused under it, in words. Save is refused only when the part has no maker or no number,
or when My parts or the project has a part of that maker and number already: open that one from the list instead.

**A part like it…**, the card under the others, or **Like this** in the bar with a part open, starts a new part with
everything the other part has but its number. A cable type sold in several sizes has **Save and add another size**
beside Save: the next size opens with everything but the number and the cross-section.

## A card such as a digital input card

1. Choose **PLC card or station**. Under **PLC**, the role is **Module**; choose **Channel kind ▸ DI** and type
   **Channels** `8`.
2. The **Pin pattern** gives the pin of channel n: `{n+1}` gives the pins 1 to 8, `X2:{n+1}` gives X2:1 to X2:8. The
   line under it shows the first and the last pin ("Channel 0 → pin 1 … channel 7 → pin 8"), so that a wrong pattern
   shows before you save, and the next line the input or output bits the card occupies.
3. Type the supply pins (**Supply L+ pin**, **Supply M pin**) when the card has them, and the **Allowed slots**
   (`1-8, 10`) when the data sheet restricts them. **Save**.
4. Place the card beside the CPU from the device browser: it stands as a box with its pins, joins the project's CPU,
   and its channels are addressed and bound to their pins. The catalogue of a station's rack (**Rack…**) offers it as
   a module.

A part has channels of one kind: a card with 16 inputs and 16 outputs is two parts, one for each.

## A head station such as an IM 152

1. Choose **PLC card or station**, then **PLC role ▸ Station**; type the supply pins and **Most modules** (`32`).
2. Under **Network ports**, choose **Add interface**: type the interface `DP`, choose **Bus ▸ PROFIBUS DP** and
   **Role ▸ Slave**. Its PROFINET sister has the interface `X1`, **Bus ▸ PROFINET**, **Role ▸ Device** and **Ports**
   `2`. **Save**.
3. Place it from the device browser on a bus-topology page: it stands there as a box with its DP socket, and its
   supply pins wait for a schematic page (drag its unplaced function from the **Devices** list there, and draw the
   box with its pins). Placed first on a schematic page, it gets its port box on a bus-topology page from **Place
   port DP** on its row in the PLC navigator. Add its cards in its rack (**Rack…** on that row). A station that only
   answers on the bus is a head station.

**New part…** at the foot of a station's catalogue starts the part the rack asks for: a CPU, a remote station, a
valve terminal, a compact device or a module.

## A switch such as a SCALANCE, or a device on PROFINET

1. Choose **Device drawn as a box**. Under **Network ports**, **Add interface**: `X1`, **PROFINET**, and the number
   of **Ports** (sockets) of the data sheet.
   - An **unmanaged** switch has the role **Passive**: no address, it passes the net through.
   - A **managed** switch and a drive, an HMI or a gateway have the role **Device**: one PROFINET name and IP
     address for the whole interface, however many sockets it has.
2. **Save**, then place it from the device browser on a bus-topology page with one click: it stands as its bus
   symbol, a socket per port, P1, P2 …, labelled inside its body. A device with several interfaces, such as the
   starter CPU with DP and X1, stands as one symbol with a compartment per interface. Draw the bus lines with
   **Draw ▸ Wire**: the CPU, the devices and the managed
   switch are one network, and an unmanaged switch stands in it with "passes the net through" in the **Bus**
   navigator.
   Type each node's name in the navigator's table (its **Address** column): its compartment prints it, the name above
   the IP address, and "TBD" while it is open.

The starter parts hold such parts to begin with: EW-GEN-ESW-5 (unmanaged switch, 5 ports), EW-GEN-ESW-8 (managed
switch, 8 ports) and EW-GEN-PND-1 (a PROFINET device, 2 ports).

## Change a part

1. Open the part in Parts: from the list (type a part of its number, maker or description), or select a device and
   choose **Project ▸ Parts**, which opens on the device's part.
2. A part of your own, in My parts or only in this project, opens ready to change. A starter part or a part of another
   library is not yours to change: its fields show what it is, and **Make my own** starts a part of your own with its
   data, which you save under a part number of your own.
3. Change what you need and **Save**. A part of My parts is saved there as its next version, and the card beside the
   sheet lists what changes in words ("Interface X1: 8 ports → 6 ports", "Channels: 8 → 16", the parts list row's
   changed column) and the devices that use it; **Apply** brings the project's copy to it. A part only in this project
   changes in place, with the same card. **Discard** drops what you typed.
4. A device already drawn follows the correction: its box shows the pins and sockets the part still has, and its bus
   port keeps its bus, role and address. A bus line at a socket the part no longer has keeps its end there; the card
   names it before **Apply**, so that you draw it to another socket. A pin or socket the part adds stands after the
   last one of its edge, the box growing to take it; a box that would then run past the drawing area goes back along
   that edge until it fits, its wires with it. The card names each box drawn anew and its page before **Apply**.

A part only in this project has **Keep in my parts** in the bar: it is written into My parts, and the project's devices
take that copy. **Remove** takes a part of your own out of My parts (it asks first); a project that uses it keeps its
copy. **Back** returns to where you came from.

## Bring parts in from a file

**Import** in the bar of Parts reads a file of parts; **File ▸ Import…** goes on to Parts for one too.

- **A table** (CSV or Excel), such as a maker's list or a family of your own parts: Parts shows **Which column is
  what**, each column of the file with its first value and the field it **Goes to**, chosen from the fields by their
  words (**Identity ▸ Part number**, **Width**, **Technical data ▸ Rated current**). A column whose header names a field
  is mapped already; a column no field keeps stays **Not imported**. **Width**, **Height** and **Depth** go to the
  field of that name the row's kind has: a cabinet's outer size, a plate's size, a device's size in the cabinet. A
  column mapped to **Like** names a part by its number: a new row starts as that part (its pins, what it is drawn as,
  its clamps, what it comes with) and takes its own columns over it, so that a family is one part made on its sheet
  and a table of the others. A column mapped to **Cable ▸ Number of cores** (a column named Cores is mapped already)
  gives a cable that many numbered cores, of the cross-section a column mapped to **Cable ▸ Cross-section** gives; an
  empty cell leaves its cores as they are. The map is saved under the name in **Saved as**, and the next table of that
  source is read the same way.
- **An EPLAN parts file** (an EDZ, or a parts XML): before the first EDZ of a session a notice says that the EPLAN Data
  Portal's terms restrict its data to EPLAN products. Where the open project holds copies of the file's parts, a line
  says how many and how many would change, and **Update them…** shows the changed fields before anything changes.
- **An outline drawing** (DXF) gives the open part its front outline, as **Import outline…** does.
- **A symbol file**, on the **Symbols** shelf or on the drawing: a QElectroTech element, an EPLAN symbol file or a DXF
  drawing opens as a drawing of your own.

What a table or a parts file brings stands in the list as **This import**: each row **new**, **changed**, **the same**
as the part in My parts, or **refused** with its reason under **What was read** ("Row 9 is refused: it has no part
number"), and a dot where the part lacks something. A row shows on the right as it will be, with **Ready for**. Change
the map and the list follows. Nothing is saved until **Save 12 to My parts**, which writes the new and the changed
parts; a project takes a part when a device uses it. **Discard** drops the set.

## Change many parts at once

1. In the list of Parts, hold **Ctrl** and click each part to take it in or out of the selection, or hold **Shift** and
   click to take the rows from the open part to that one.
2. The sheet shows the fields the parts share: a value they all have, or **mixed** under a field they differ in. Where
   they have the same pins, the **Connections** table's cells are there too ("Pin 1 · Clamp").
3. Type a value: it goes to all of them. A field you leave **mixed** stays as each part has it.
4. **Save** saves each part as its next version in My parts. Where the open project uses them, the card beside the
   sheet names how many parts and which fields change, with the devices; **Apply** brings the project's copies to them
   in one step, which one **Undo** takes back in the project (My parts keeps what you saved).

A starter part among them is not yours to change: a line names it, and **Make my own from** it starts a part of your
own from it. A part only in this project is kept in My parts first (**Keep in my parts**).

**Export** in the bar of Parts writes the selected parts, or the open one, as a table (CSV, or Excel for a name ending
in `.xlsx`), one column per field in the window's words. A family of fifty terminals is made in a spreadsheet from it:
copy a row, change its number and its values, and **Import** reads the table back, its new rows as new parts and its
changed rows as changes. A table imported as it was exported brings every row as **the same**.

## A device that is a box with pins

A circuit protector, a power supply, a drive: a device the program has no symbol for is drawn as a box with its pins on
the edges. The part says which pins it has and where they stand.

1. Under **On the schematic**, **Pins** lists the part's pins, each with its name and the potential it carries. **Add
   pin** adds one, the cross at a row's end removes it; each pin has its row under **Connection points** for its
   clamp. A PLC card's pins come from its channels and a part drawn with a symbol takes that symbol's connections: the
   group lists them and says so.
2. Choose **Arrange…**. The box opens as the drawing of Parts: drag each pin along its edge or onto another edge, drag
   the box's edges to size it, choose **Done**, then **Save** the part
   ([The box of a part](../reference/symbol-editor.md#the-box-of-a-part)).
3. **Draw ▸ Place ▸ Device from part…** then places the part's box with one click, laid out as you drew it. On the page
   the box's edges and pins can still be dragged. When you drag an edge, the pins of each side stay centred on it with
   their spacing; the box goes no smaller than its pins need (one grid step each and one to spare at each corner), and
   no lower than its pin names need, and the status line says so. **Centre the pins** in the box's right-click menu
   centres them without a resize. The pin names of the top and bottom sides are turned a quarter, reading from bottom
   to top; those of the left and right sides are horizontal.
4. To change the box of a device already drawn, choose **Arrange its box…** in its menu. Save's card names each box of
   the part it draws anew, and its Apply redraws them.

## A part drawn with your own symbol

A piece's **Symbol** cell, under **On the schematic ▸ Drawn as**, offers the symbols of what the piece is; **Draw a new
symbol…** draws one for it, its pins standing on the drawing already, and **Change it for this part…** draws your copy
of the one it has. **Done** gives the piece the symbol; a device placed from the part is drawn with it. A symbol you
saved on its own offers **New part drawn with it…** under its name ([Drawing a symbol](../reference/symbol-editor.md)).

The PLC exchange with PLC software exports the stations of a configuration project; a switch or a device without a PLC
section is not in the exported file. See [Work in the PLC workspace](plc-workspace.md) for the navigators, the rack
editor and the exchange.
