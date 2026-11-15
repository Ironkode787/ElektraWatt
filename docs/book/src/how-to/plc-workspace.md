# Work in the PLC workspace

The PLC workspace shows a project's PLC configuration beside its pages: the PLC navigator and the bus navigator in the
left column, the page in the middle and the PLC grid, every channel as a row of a table, in the bottom dock. It is the place to address
modules, bind channels and check what each channel drives.

## Open it

1. Choose **PLC** in the workspace switch at the right of the tab row, or **PLC workspace** at the end of the
   **PLC** tab. The left column shows the PLC navigator and the bottom dock the PLC grid; Properties stays under the
   navigator. **Schematic** in the switch goes back; a change you make to the layout is kept for the workspace. The
   **PLC** tab holds a button for each PLC and bus command: Rack, Addresses, Bus page, Place port, PLC grid, the
   exchange with PLC software and this guide.

## The PLC navigator

2. The tree runs from the configuration project to its stations (a CPU, a head station, a valve terminal), their
   racks once a module has a rack position, their modules and each module's channels, with the station's bus ports
   after its modules. Selecting a row selects the device or channel on the page shown; a drawable on another page is a
   chip at the top of the canvas that opens it.
3. Drag a channel's row onto a connection point of a module pin or a coil to bind it. A device wired off the module,
   or a valve beside a valve terminal, is attached to the module and bound in one step; one Undo takes both back.
4. A module drawn on a page joins the project's only station, and its toast says which slot it got, with **Rack…**.
   In a project with two stations it joins the one drawn beside it on its page; drawn on a page without a station, a
   window asks which station takes it, the one last given a module first. Without any station its toast offers **New
   station…**. A module that waits is listed under **Modules without a station**: drag its row onto the station's row
   (a module of another station moves there), or right-click it and choose **Attach to** and the station's tag; or
   right-click the station that takes all of them and choose **Attach modules without a station**. Only a module goes
   into a rack: a device whose part has no PLC data is refused in words, and a bus device is given to a PLC as a
   station of its own (step 5).

## Stations

5. Choose **New station…** (Project menu, or **+** in the PLC navigator). Choose what the station is (a hover over
   each choice says it in full), its head part and the PLC it belongs to, then fill the rack: **Add module…** on a
   free slot picks a module part and how many fill the slots from there. **New part…** beside the head part, or at
   the foot of the module picker, opens the part editor already set for what you asked (a CPU's bus interface leads
   the bus, a remote I/O station's answers on it, a module is a module), and the part you save is chosen when the
   dialog comes back. **Create** makes the station and its modules, drawn on no page yet. **Draw on this page** puts
   the station's box and each module's beside it in slot order on the schematic page shown, in rows under what is
   drawn there, in one undo step (a module drawn already stays where it is); **Put it on a bus page** draws its bus
   interface. A station part placed directly from the device browser opens
   the same rack at once. On a bus-topology page, the Bus panel or a schematic page, right-click a bus device that
   belongs to no PLC and choose **Add to** and the PLC (or **Add to a new PLC**): it becomes a station of that PLC and
   its rack opens; a part without PLC data gets an open rack whose modules you add. A switch or a plug takes no part.
   A station's menu says **PLC:** and its name, and offers **Rack…**.

## Bus ports and nets

6. A station placed from the device browser on a bus-topology page stands there as its bus interfaces, a box each
   with its sockets (the starter CPU as DP and X1), and without its supply pins, which are its box on a schematic
   page. A station's port that is not drawn on the bus-topology page shown is offered on the station's row as **Place
   port DP** (or **Place port X1** for PROFINET): it arms the Black box tool with that interface's box; two clicks
   draw it. On another page the same entry places the port symbol joined to the station, for a port no function
   carries yet. Draw the bus between the ports with **Draw ▸ Wire**, which draws bus lines on a
   bus-topology page: the two ports become one network (a new one named for the bus, PROFIBUS DP, when neither has
   one), with the CPU's port as its master. The starter parts hold a CPU with PROFINET and PROFIBUS DP, a remote I/O
   head (a station of its own: attach its modules with **Attach modules without a station** on its row), an unmanaged and a
   managed switch, a PROFINET device and a PROFIBUS and an Ethernet cable. Your own cards, stations, switches and
   devices are made in the part editor: see [Make your own PLC or bus part](own-parts.md).
7. Choose **Bus** on the rail for the nets: each logical net with its ports, each physical net with its segments and
   their ends. Show it as a table to type a port's address or switch its termination: the cell's editor says the form
   under it (a PROFIBUS DP station address such as `3`, a PROFINET device name and IP address such as `plc1
   192.168.0.1`). Selected on the page, a port's symbol or box shows its function first in Properties, with the same
   **Address** row. **Ports not drawn yet** lists
   every interface a part gives a device that no page draws: Enter, or **Place port** in its menu, draws its box on
   the bus-topology page shown.

## Addresses

8. Right-click a module and choose **Re-address…**, or a configuration project for all its modules: the Preview tab
   lists each channel whose address moves, old and new, and **Apply** makes the change in one step.
   **Make manual addresses automatic…** drops the manual addresses that equal the derived ones, and **Freeze
   addresses…** on a configuration project keeps the address of every used or reserved channel as a manual one and
   pins each card's start, so that a later change of a card moves none; a spare channel keeps its automatic address.
   The configuration project's three commands are also in the **Addresses** menu at the top of the PLC panel and of
   the PLC grid. After **Apply** a toast says how many addresses were frozen or moved, and a manual address (typed,
   or frozen) carries the badge **manual** in the panel and a lock in the grid's Address cell.

## Rack and slots

9. Right-click a station and choose **Rack…**. **Add module…** on a free slot adds modules; the arrows on a module's
   row move it, the bin empties its slot, **Replace…** gives it another part; type another slot into a module's Slot
   field to move it. A slot the module's part does not allow is refused, and the reason stays in the dialog.

## The PLC grid

10. Each row of the grid is a channel. Type in a cell to change a channel's designation, address, symbol, state,
   target, comment, data type or signal range; the slot cell takes a number or `<rack>.<slot>`. Safety data is set
   with **Safety data…** and **Mode and safety…** (step 17).
11. Type in the grid's filter to narrow the rows: a word of a cell, or one of `spare`, `unplaced`, `unassigned`,
    `tbd`, `duplicate`, `safety`, `config:<name>` and `bus:<net>` (`bus:profibus-dp`). A click on a column's header
    sorts by it, a second click reverses the order.

## Exchange with the PLC software

12. Choose **Export to PLC software** on the PLC tab (or **PLC ▸ Export to PLC software…**), pick the format
    and choose **Export**, then the file: the dialog names it for the format (`.aml`, `.xlsx` or `.xml`), and the
    document date starts as today's. Import the file in TIA Portal or CODESYS.
13. Choose **Import from PLC software** on the PLC tab (or **PLC ▸ Import from PLC software…**) for the file the
    PLC software wrote back. Look through its tabs; each conflict shows the side Apply takes, and offers the file's
    value where it can be taken. Choose **Apply**.
14. After an import, the module starts, the symbols or the PLC data the PLC software owns are protected. Right-click
    the configuration project and choose **Unlock module starts** (or the other group) to edit them in the project.
15. Right-click the configuration project and choose **Import instrument list…** for a CSV or workbook of process tags
    and signal types: the Preview tab lists the spare channels it reserves and the tags left open. Choose **Apply**.

## Settings

16. Right-click the configuration project and choose **Symbol policy…** for the template of the automatic symbol, the
    PLC tag table and whether symbols are strict IEC 61131-3 identifiers.
17. Right-click a module and choose **Safety data…** for its F-source and F-destination address and its F-watchdog
    time; right-click a channel and choose **Mode and safety…** for a multi-function port's mode and the channel's
    safety evaluation and requirement.
18. Choose **Process-tag scheme** on the PLC tab for the pattern process tags follow, and **Addresses at
    terminals** to print a channel's address at the terminals on its path.

The panels are described in the [panel reference](../reference/window/panels.md): PLC, PLC grid and Bus; the dialog in the
[dialog reference](../reference/window/dialogs.md): Rack and slots and PLC import.
