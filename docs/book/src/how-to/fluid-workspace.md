# Work in the Fluid workspace

The Fluid workspace shows a project's pneumatics beside its pages: the fluid navigator in the left column, the page in
the middle, and the tube table and the medium table in the bottom dock.

## Open it

1. Choose **View ▸ Workspaces ▸ Fluid power**. **View ▸ Workspaces ▸ Schematic** goes back; a change you make to the
   layout is kept for the workspace.

## Valves and lines

2. The fluid navigator lists each valve terminal with its modules and valves, and every other device a fluid line
   ends at, each with its lines. Select a line's row to find it on the page.
3. Draw a fluid line with **Draw ▸ Wire** on a fluid page. Right-click a line to choose its kind (**Line: Working**,
   **Line: Pilot** and the others) or its medium.

## Tubes

4. The tube table has one row per tube. Type a length in millimetres into **Length** or a tube part into **Tube
   part**; **Source** then says the value is manual. Leave a cell empty to take the derived value again.

## Media

5. Choose **Project ▸ New medium…**, give the medium a name and its pressure in bar, and choose **Create**.
6. Right-click a fluid line and choose **Medium:** with the medium's name: the line's net and every net the medium
   reaches through the valves carry it. The fluid navigator counts the lines of each medium.
7. Select the medium's row in the medium table and press `Ctrl+Shift+H` to highlight its zones on the page; the
   highlight is a colour with a hatch pattern, so it reads without colour too.

The panels are described in the [panel reference](../reference/window/panels.md): Fluid, Tube table and Medium table.
