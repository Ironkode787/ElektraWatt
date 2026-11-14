# Work in the PLC workspace

The PLC workspace shows a project's PLC configuration beside its pages: the PLC navigator and the bus navigator in the
left column, the page in the middle and the PLC grid, every channel as a row of a table, in the bottom dock. It is the place to address
cards, bind channels and check what each channel drives.

## Open it

1. Choose **View ▸ Workspaces ▸ PLC**. The left column shows the PLC navigator and the bottom dock the PLC grid;
   Properties stays under the navigator. **View ▸ Workspaces ▸ Schematic** goes back; a change you make to the layout
   is kept for the workspace.

## The PLC navigator

2. The tree runs from the configuration project to its stations (a CPU, a head station, a valve terminal), their
   racks once a module has a rack position, their modules and each module's channels, with the station's bus ports
   after its modules. Selecting a row selects the device or channel on the page shown; a drawable on another page is a
   chip at the top of the canvas that opens it.
3. Drag a channel's row onto a connection point of a card pin or a coil to bind it. A device wired off the card, or a
   valve beside a valve terminal, is attached to the card and bound in one step; one Undo takes both back.
4. In a project with two CPUs a new card joins neither and is listed under **Cards without a CPU**. Right-click the
   CPU that takes it and choose **Attach cards without a CPU**.

## Bus ports and nets

5. A CPU's port that is in its part but not drawn yet is offered on the CPU's row as **Place port DP**: on a
   bus-topology page it arms the Place tool with the port symbol joined to the CPU, and a click places it. Draw the bus
   between the ports with **Draw ▸ Wire**, which draws bus lines on a bus-topology page.
6. Choose **Bus** on the rail for the nets: each logical net with its ports, each physical net with its segments and
   their ends. Show it as a table to type a port's address or switch its termination.

## Addresses

7. Right-click a module and choose **Re-address…**, or a configuration project for all its modules: the Preview tab
   lists each channel whose address moves, old and new, and **Apply** makes the change in one step.
   **Make manual addresses automatic…** drops the manual addresses that equal the derived ones, and **Freeze
   addresses…** on a configuration project keeps every derived address as a manual one, so that a later change of a
   card moves none.

## Rack and slots

8. Right-click a station and choose **Rack and slots…**. Pick the card in **Module part to add** and choose **Add
   here** on a free slot; type another slot into a card's Slot field to move it. A slot the card's part does not
   allow is refused, and the reason stays in the dialog.

## The PLC grid

9. Each row of the grid is a channel. Type in a cell to change a channel's designation, address, symbol, state,
   target, comment, data type or signal range; the slot cell takes a number or `<rack>.<slot>`. Safety data is edited
   in Properties.
10. Type in the grid's filter to narrow the rows: a word of a cell, or one of `spare`, `unplaced`, `unassigned`,
    `tbd`, `duplicate`, `safety`, `config:<name>` and `bus:<net>` (`bus:profibus-dp`). A click on a column's header
    sorts by it, a second click reverses the order.

## Exchange with the PLC software

11. Choose **Output ▸ Export to PLC software…**, pick the format and choose **Create**, then the file. Import the file in
    TIA Portal or CODESYS.
12. Choose **Output ▸ Import from PLC software…** for the file the PLC software wrote back. Look through its tabs, decide
    each conflict, and choose **Apply**.
13. After an import, the module starts, the symbols or the PLC data the PLC software owns are protected. Right-click
    the configuration project and choose **Unlock module starts** (or the other group) to edit them in the project.
14. Right-click the configuration project and choose **Import instrument list…** for a CSV or workbook of process tags
    and signal types: the Preview tab lists the spare channels it reserves and the tags left open. Choose **Apply**.

## Settings

15. Right-click the configuration project and choose **Symbol policy…** for the template of the automatic symbol, the
    PLC tag table and whether symbols are strict IEC 61131-3 identifiers.
16. Right-click a module and choose **Safety data…** for its F-source and F-destination address and its F-watchdog
    time; right-click a channel and choose **Mode and safety…** for a multi-function port's mode and the channel's
    safety evaluation and requirement.
17. Choose **Project ▸ Process-tag scheme…** for the pattern process tags follow, and **Project ▸ PLC addresses at
    terminals** to print a channel's address at the terminals on its path.

The panels are described in the [panel reference](../reference/window/panels.md): PLC, PLC grid and Bus; the dialog in the
[dialog reference](../reference/window/dialogs.md): Rack and slots and PLC import.
