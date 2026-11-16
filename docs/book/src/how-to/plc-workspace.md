# Work in the PLC workspace

This guide is where you look up one PLC job. To learn the jobs in order, follow the tutorials
[A PLC with its modules](../tutorials/plc-station.md) and [A bus network](../tutorials/bus-network.md) first.

EW keeps the PLC hardware of a project beside its drawings: each CPU and remote I/O head with the modules in its
rack, every channel with its address, symbol and what it is wired to, and the bus networks between them. You draw the
modules on schematic pages as boxes with their pins and wire the field devices to them, as in any ECAD tool; the PLC
side knows which channel each pin is and exports it all to TIA Portal or CODESYS.

The PLC workspace shows the PLC side beside the page:

![The PLC workspace: the PLC panel at the left, the page, the PLC grid under it, the PLC tab](images/plc-workspace.png)

- the **PLC** tab of the command bar at the top, with a button for each PLC job;
- the **PLC panel** at the left: each PLC with its stations, their modules and each module's channels;
- **Properties** under it, for what is selected;
- the page in the middle;
- the **PLC grid** under the page: one row per channel, edited like a spreadsheet.

## The words

| Word | What it is |
|---|---|
| PLC | One controller's configuration, shown as PLC1 in the PLC panel: what a CPU and its remote stations share. |
| Station | A CPU or a remote I/O head with the modules it carries; a valve terminal with a bus node is one too. TIA Portal calls it a device in its network view. |
| Rack | The row of slots of a station. |
| Slot | A place in the rack, numbered from 1. The slot order gives the addresses. |
| Module | An I/O card in a slot (the window says module). |
| Channel | One input or output of a module, such as I0.0: one row of the PLC grid and one pin of the module's box. |
| Symbol | The PLC tag name of a channel, such as `Door_closed`, as in the PLC tag table. The drawing symbol of a device is another thing. |

## In TIA Portal you … here you …

| In TIA Portal you … | Here you … |
|---|---|
| add a CPU in the device view | choose **New station** on the PLC tab and pick its head part |
| plug modules into the rack from the catalogue | choose **Add module…** on a free slot of the station's rack |
| move a module to another slot | use the arrows of its row in **Rack…**, or type the slot |
| read the I/O addresses in the device overview | read the **Addresses** column of **Rack…**, or the PLC grid |
| change a start address | type the address in the PLC grid, or choose **Re-address…** |
| type the PLC tags | type each channel's **Symbol** in the PLC grid |
| add an IO device in the network view | place its part on a bus-topology page |
| connect it to the subnet | draw a bus line between the sockets with **Draw ▸ Connect ▸ Wire** |
| assign an IO device to a controller | right-click it and choose **Add to** and the PLC |
| set the PROFINET device name and IP address | type them into the **Address** cell of the Bus panel's table |

The exchange of the hardware and the tags with the PLC software is at the end of this guide.

## Open the workspace

1. Choose **PLC** in the workspace switch at the right of the tab row, or **PLC workspace** on the PLC tab.
2. **Schematic** in the switch goes back. A change you make to the layout is kept for the workspace.

## Make a station

1. Choose **New station** on the PLC tab (**PLC ▸ New station…**), or **+** in the PLC panel.
2. Choose what it is: **PLC (CPU)**, **Remote I/O**, **Valve terminal** or **Compact device** (a hover says each in
   full).
3. Choose its **Head part**: the CPU, the interface module of a remote station, the valve terminal. **New part…**
   beside it opens the part editor set for that kind, and the part you save is chosen when you come back.
4. Choose the PLC it belongs to (a remote station: the PLC it serves), or **New PLC**.
5. Choose **Add module…** on the free slot and pick a module; **How many** fills that many slots from there.
6. Choose **Create**: the station and its modules are made in one step, drawn on no page yet.
7. Choose **Draw on this page** to draw the station and its modules side by side in slot order on the schematic page
   shown, or **Put it on a bus page** to draw its bus interface on the first bus-topology page.

A station part placed straight from the device browser opens the same rack at once.

## Change a rack

1. Right-click the station (on the page, in the PLC panel) and choose **Rack…**, or choose **Rack** on the PLC tab.
2. **Add module…** on a free slot adds modules; the arrows move a module up or down; the bin empties its slot;
   **Replace…** gives it another part; typing a number into its slot field moves it there.
3. A slot the module's part does not allow is refused, and the reason stays under the slots. When modules move, the
   line under the slots says their addresses moved with them.

## Put a module into a station

1. A module drawn on a page by itself (from the device browser or the black box) joins the project's only station,
   and its toast names the slot it got, with **Rack…**.
2. With two stations it joins the one drawn beside it on its page; on a page without one, a window asks which station
   takes it. Without any station, its toast offers **New station…**.
3. A module that waits is listed under **Modules without a station** in the PLC panel: drag its row onto the station's
   row, or right-click the station and choose **Attach modules without a station**.

Only a module goes into a rack: a device whose part has no PLC data is refused in words.

## Wire a field device to a channel

1. Draw the device (a sensor, a lamp, a coil) on the page beside the module's box.
2. Choose **Draw ▸ Connect ▸ Wire** (`W`) and wire it to the module's pin: pin 1 is channel 0.
3. Or drag a channel's row from the PLC panel onto the device's connection point: the device is attached to the module
   and the channel bound to it in one step, which one Undo takes back.

## Addresses

1. A channel's address comes from its module's slot and the modules before it: the first input module of a station
   starts at I0.0, the first output module at Q0.0.
2. To give one channel another address, type it into its **Address** cell in the PLC grid; it keeps it, with the
   badge **manual** in the panel and a lock in the grid.
3. Right-click a module and choose **Re-address…** (or the PLC row for all its modules): the Preview tab lists each
   channel whose address moves, old and new; **Apply** makes the change in one step.
4. Right-click the PLC row and choose **Freeze addresses…** to keep every used or reserved channel's address when
   modules move later, or **Make manual addresses automatic…** to drop manual addresses that equal the automatic ones.
   The three are also under **Addresses** on the PLC tab.

## The PLC grid and the symbols

1. Choose **PLC grid** on the PLC tab (**PLC ▸ PLC grid**). Each row is a channel.
2. Click a cell and type: the designation, address, symbol, state, target, comment, data type or signal range. The
   slot cell takes a number or `<rack>.<slot>`.
3. Type into the grid's filter to narrow the rows: a word of a cell, or one of `spare`, `unplaced`, `unassigned`,
   `tbd`, `duplicate`, `safety`, `config:<name>` and `bus:<net>` (`bus:profibus-dp`). A click on a column's header
   sorts by it.
4. To have EW write the symbols, right-click the PLC row and choose **Symbol policy…**: the template of the automatic
   symbol, the PLC tag table and whether symbols must be strict IEC 61131-3 names.

## Give a bus device to a PLC

1. On a bus-topology page, a schematic page or in the Bus panel, right-click a bus device that belongs to no PLC.
2. Choose **Add to** and the PLC, or **Add to a new PLC**: it becomes a station of that PLC and its rack opens.

A switch or a plug takes no part in a PLC; its menu says why. A station's menu says **PLC:** and its PLC's name.

## Bus ports and nets

A bus-topology page is the network view: each device stands as one box per bus interface (PROFINET X1, PROFIBUS DP),
with a connection point per socket, and the bus lines run between the sockets. The device's power and I/O pins stay
on the schematic pages.

1. Choose **Bus page** on the PLC tab: the new-page form opens with the type **Bus topology**.
2. Place a station or another bus device from the device browser (**Draw ▸ Place ▸ Device from part…**) with two clicks
   for its box: it stands there as a box per interface, and a station's rack opens.
3. A station drawn first on a schematic page gets its port box from **Place port DP** (or **Place port X1**) on its row
   in the PLC panel; **Ports not drawn yet** in the Bus panel lists every interface no page draws yet.
4. Draw the bus lines with **Draw ▸ Connect ▸ Wire**: the ports become one network, named for its bus, with the CPU's
   port as its controller or master.
5. Choose **Bus panel** on the PLC tab and switch it to its table. Type each node's address into its **Address** cell
   as the cell says: a PROFIBUS DP station address such as `3`, a PROFINET device name with its IP address such as
   `plc1 192.168.0.1`. The box prints it. An unmanaged switch passes the net through and takes none.
6. Select a bus line and choose **Cable** on the Draw tab: choose the cable (the ones made for that bus first), its
   length and the plug at each end with its termination, and **Apply**. The line prints the cable's tag; the cable and
   the plugs go into the parts list.

The starter parts hold a CPU with PROFINET and PROFIBUS DP, a remote I/O head, an unmanaged and a managed switch, a
PROFINET device, PROFIBUS and Ethernet cables and their plugs. Your own devices with their ports are made in the part
editor: see [Make your own PLC or bus part](own-parts.md).

## Exchange with the PLC software

1. Choose **Export to PLC** on the PLC tab (**PLC ▸ Export to PLC software…**) and pick the format: **AutomationML AR
   APC 1.4.0** (or 1.2.0) carries the stations, their modules, addresses and symbols to TIA Portal; **TIA Portal PLC
   tags (XLSX)** carries the tag table; **PLCopen XML (CODESYS)** is for CODESYS. Choose **Export**, then the file.
2. Choose **Import from PLC** on the PLC tab (**PLC ▸ Import from PLC software…**) for the file the PLC software wrote
   back. Look through its tabs: each conflict shows the side **Apply** takes, and offers the file's value where it can
   be taken. Choose **Apply**.
3. After an import, what the PLC software owns (the module starts, the symbols or the PLC data) is protected:
   right-click the PLC row and choose **Unlock module starts** (or the other group) to edit it here again.
4. Right-click the PLC row and choose **Import instrument list…** for a list of process tags and signal types (CSV or
   workbook): the Preview tab lists the spare channels it reserves.

## Settings

- **Process-tag scheme** on the PLC tab: the pattern process tags follow.
- **Addresses at terminals** on the PLC tab: each channel's address printed at the terminals on its path.
- Right-click a module and choose **Safety data…** for its F-addresses and F-watchdog time; right-click a channel and
  choose **Mode and safety…** for a multi-function port's mode and the channel's safety data.

The panels are described in the [panel reference](../reference/window/panels.md) (PLC, PLC grid, Bus), and the dialogs
in the [dialog reference](../reference/window/dialogs.md) (Rack and slots, PLC import).
