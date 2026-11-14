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

## Valve terminals

4. Right-click a valve's row: **Set spare** or **Set used**, **Blank position** to put a blanking plate in its place,
   and **Plug port** for each working port that has no line. A module's row adds a valve at a blank or empty position
   (**Add valve at position**), of the part its other valves have.
5. Right-click the valve terminal's row and choose **Generate overview…**. The Preview tab lists which modules stand
   on which overview page, and the pages it adds where the frame is full; the tags that change with a page are listed
   below. Choose **Apply**. Run it again after adding a module: it refreshes the pages.

## Tubes

6. The tube table has one row per tube. Type a length in millimetres into **Length** or a tube part into **Tube
   part**; **Source** then says the value is manual. Leave a cell empty to take the derived value again.

## Media

7. Choose **Project ▸ New medium…**, give the medium a name and its pressure in bar, and choose **Create**.
8. Right-click a fluid line and choose **Medium:** with the medium's name: the line's net and every net the medium
   reaches through the valves carry it. The fluid navigator counts the lines of each medium.
9. Select the medium's row in the medium table and press `Ctrl+Shift+H` to highlight its zones on the page; the
   highlight is a colour with a hatch pattern, so it reads without colour too.

The panels are described in the [panel reference](../reference/window/panels.md): Fluid, Tube table and Medium table.
