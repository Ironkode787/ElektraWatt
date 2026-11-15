# Work in the Fluid workspace

The Fluid workspace shows a project's pneumatics beside its pages: the fluid navigator in the left column, the page in
the middle, and the tube table and the medium table in the bottom dock.

## Open it

1. Choose **View ▸ Workspaces ▸ Fluid power**. **View ▸ Workspaces ▸ Schematic** goes back; a change you make to the
   layout is kept for the workspace.

## Valves and tubes

2. The fluid navigator lists each valve terminal with its modules and valves, and every other device a tube ends at,
   each with its tubes. Select a tube's row to find it on the page. Its table mode (the Table toggle) lists the tubes.
3. Draw a tube with **Draw ▸ Wire** on a fluid page, or choose **Draw ▸ Connect ▸ Tube…** to pick its line kind first
   (Pilot, Exhaust, Drain, Flexible). A symbol placed with a fluid port facing a free port is joined to it with a tube.
   Right-click a tube to change its line kind (**Line kind: Working**, **Line kind: Pilot** and the others) or its
   medium; a flexible tube is drawn as an arc.

## Valve terminals

4. To start a valve terminal, choose **Draw ▸ Device**, find **Valve terminal, 4 valve positions, 8 outputs** and
   click two corners of its box on the page. It is a valve terminal because its part has valve positions: the fluid
   navigator lists it under **Valve terminals**.
5. Right-click the terminal's box or its row and choose **Add valve at position 1**: the valve becomes the terminal's
   child at that position, and its symbol follows the pointer until you click where it goes. The valve is of the part
   the terminal's other valves have, else the library's valve for its positions. A valve placed into the box from
   **Symbol** or **Device**, or dragged into it, becomes the terminal's valve at the next free position too; the toast
   names the position. A valve's **Valve position** in Properties moves it to another free position. The supply plate
   **Valve terminal supply plate, ports 1, 3 and 5** placed into the box joins the terminal too: a tube to its port 1
   feeds every valve of the terminal.
6. Right-click a valve's row: **Set spare** or **Set used**, **Blank position** to put a blanking plate in its place,
   and **Plug port** for each working port that has no line. A module's row adds a valve at a blank or empty position
   the same way (**Add valve at position**).
7. Right-click the valve terminal's box or row and choose **Generate overview…**; from the command search it acts on
   the selected terminal and says so while none is selected. The Preview tab lists which modules stand on which
   overview page, and the pages it adds where the frame is full; the tags that change with a page are listed below.
   Choose **Apply**. A new overview page is titled by the terminal, such as **Valve terminal -201A1**: its modules
   stand in a strip inside the frame, each valve position on top with the valve's part number, a blank position
   crossed. Run it again after adding a module: it refreshes the pages.

## Tubes

8. The tube table has one row per tube. Type a length in millimetres into **Length** or a tube part into **Tube
   part**; **Source** then says the value is manual. Leave a cell empty to take the derived value again. A tube's
   **Tube part** in Properties offers the tube parts, such as the starter tubes PU 6 × 4, 8 × 6 and 10 × 8 mm.
9. Choose **Project ▸ Fluid settings…** for the default tube of every line (its outer and inner diameter in mm and
   its material) and to tag tubes by page and counter. A toast says what the default tube is now; it reaches the
   tubes drawn already. A tube is specified with its size and its material: without a material, the check names it.

## Media

10. Choose **Project ▸ New medium…**, or **New medium…** at the top of the Fluid panel beside **Fluid settings…**,
    give the medium a name and its pressure in bar, and choose **Create**.
11. Right-click a tube and choose **Medium:** with the medium's name: the tube's net and every net the medium
    reaches through the valves carry it. The fluid navigator counts the tubes of each medium. A tube's **Medium** in
    Properties shows the medium and where it is defined, and sets it too, on every tube selected; its list ends in
    **New medium…**, which makes one and gives it to the tubes. Its **Tube** row shows the tube it is made of (such as
    6×4 mm PU) and where that comes from. The air supply's Properties show the medium it supplies, and set it there.
12. Select the medium's row in the medium table and press `Ctrl+Shift+H` to highlight its zones on the page; the
    highlight is a colour with a hatch pattern, so it reads without colour too.

The panels are described in the [panel reference](../reference/window/panels.md): Fluid, Tube table and Medium table.
