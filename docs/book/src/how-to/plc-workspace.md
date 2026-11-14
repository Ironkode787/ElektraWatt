# Work in the PLC workspace

The PLC workspace shows a project's PLC configuration beside its pages: the PLC navigator in the left column, the
page in the middle and the PLC grid, every channel as a row of a table, in the bottom dock. It is the place to address
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

## Rack and slots

5. Right-click a station and choose **Rack and slots…**. Pick the card in **Module part to add** and choose **Add
   here** on a free slot; type another slot into a card's Slot field to move it. A slot the card's part does not
   allow is refused, and the reason stays in the dialog.

## The PLC grid

6. Each row of the grid is a channel. Type in a cell to change a channel's designation, address, symbol, state,
   target, comment, data type or signal range; the slot cell takes a number or `<rack>.<slot>`. Safety data is edited
   in Properties.
7. Type in the grid's filter to narrow the rows: a word of a cell, or one of `spare`, `unplaced`, `unassigned`,
   `tbd`, `duplicate`, `safety`, `config:<name>` and `bus:<net>` (`bus:profibus-dp`). A click on a column's header
   sorts by it, a second click reverses the order.

The panels are described in the [panel reference](../reference/window/panels.md): PLC and PLC grid; the dialog in the
[dialog reference](../reference/window/dialogs.md): Rack and slots.
