# Draw a panel layout

A panel layout page shows an enclosure's mounting plate to scale, with its rails, its wiring ducts and the devices
placed on them. The devices are the project's own: a device of the schematic is placed once, and the panel layout,
the parts list and the enclosure legend read it from there.

## Make the page

1. Choose **Page ▸ New page…**, set **Type** to Panel layout, give the page a title such as "Panel layout", and
   choose **Create**. The page opens empty and names its first step: start with an enclosure.

## Add the enclosure

2. Choose **Enclosure** in the Draw tab's Layout group (also **Project ▸ Setup ▸ Enclosure…**). An enclosure
   belongs to a location (`+CAB1`), the cabinet. The dialog proposes the page's location, else the project's first;
   in a project without one it proposes **New location**, the first choice of the Location list, with a code such as
   `CAB1` that you can change. It proposes the first enclosure part of the project or of an installed library, such as
   "Wall enclosure 600 × 800 × 250 mm"; choose others from the lists if you like, then **Create**.
   The page now shows the enclosure and its mounting plate, centred on the whole sheet at the scale that fits it, and
   the layout gets its legend: a list page right after the layout page (902.1 after 902) with the number, tag,
   quantity, part number and description of each item, which runs on over as many pages as it needs. Each device
   shows its tag where the tag fits inside it, and a numbered balloon where it does not (a terminal, a small part).
   A new location, the enclosure, and the plate on the page with its legend page are three steps of Undo. Another
   plate goes on the page through the page's **Surface** row in Properties, or **Mounting plate…** in the menu of
   Enclosure's arrow.

   A location that has its enclosure already (made from another page) is not made twice: with that location
   chosen, the dialog's button reads **Draw -UC1 of +CAB1 on this page** and puts its mounting plate on the page.
   On a page that is not a panel layout page, the button reads **Create and draw on a new panel layout page** (or
   **Draw … on a new panel layout page**): it makes the panel layout page after the page shown and draws the
   enclosure there. **Create only** beside it makes the enclosure without drawing it.

## Rails and ducts

3. Choose **Draw ▸ Layout ▸ Rail** and click the rail's start and end on the plate (a rail runs across or up and
   down); the tool's card offers the rail part and the grid step. Draw as many rails as the cabinet needs.
4. Choose **Draw ▸ Layout ▸ Duct** and click a duct's first point and each next point; a double click or `Enter`
   finishes it (`Esc` cancels it). The card asks for a duct's size when its part gives none.

## Place the devices

5. Show **View ▸ Panels ▸ Navigators ▸ Unplaced devices**. It lists the devices of the page's location that have no
   place yet. A device without a location is not listed: give it the location in Properties, under Structure, or
   give many at once: in the Devices list, mark their rows (or their group "(none)"), right-click and choose
   **Location…**.
6. Drag a device from the list onto a rail, or select its row, press `Enter` and click the place. On a rail it
   snaps to its neighbour, the rail's ends or the plate grid; a device mounted by screws goes on the plate.
7. To fill a rail at once, select the rail and choose **Draw ▸ Layout ▸ Auto-fill rail…**: it lists the cabinet's
   unplaced devices and the devices without a location, each with a tick. The devices you marked in the list start
   ticked; else the cabinet's own devices and those whose part goes on a rail, while sensors, cylinders, motors and
   valve terminals without a location start unticked. Untick what stands in the field and choose **Fill**: the ticked
   devices are placed in the order of the schematic or of their tags, a ticked device without a location takes the
   cabinet's (one step of Undo), and a toast says what did not fit. When there is nothing to place, the dialog says
   why.

## Labels and the legend

8. The page's right-click menu (**Labels: Tags**, **Labels: Item numbers**, **Labels: Both**, **Labels: Tags or
   numbers**) and, with nothing selected, the page's **Labels** row in Properties choose what each device shows: its
   tag, its item number in a balloon, both, or its tag where it fits and its balloon where it does not (what a new
   layout shows). The legend lists every item whatever the labels show. A legend added while the devices show their
   tags offers **Show item numbers**. Labels that would run into each other move apart, above and below the row, with a
   line to their device. On the cabinet's own layout the tags leave out the cabinet's location.
9. A layout without a legend gets one through **Add legend** in the page's right-click menu (also **Page ▸ Order ▸
   Add legend**): in the free space beside the layout where every row and every text fits there whole, one table as
   wide as its block and as long as its rows; otherwise, as a toast says, on its own page after the layout page. Click
   a legend beside the layout to select it, drag it to move it, and drag one of its corners to make it wider or
   narrower. **Move the legend to its own page** in its right-click menu (also **Page ▸ Order**) makes such a legend,
   or one of a project made before, the layout's legend page, in one step of Undo. The legend page's columns and
   caption are chosen as any list's: **Output ▸ List columns…** on its page.
10. **Follow reference** (`F`) on a device's tag or balloon goes to its row on the legend page, and on a row's tag back
    to the device on the layout; in an exported PDF a click does the same.
11. A label or balloon can be dragged on its own: one beside its device at once, one on its device once the device is
    selected (the label then shows a dashed box with a handle). Its line follows it to the device.
    **Reset label position** in the device's right-click menu puts it back; **Reset label positions** in the page's
    right-click menu puts every label of the page back.

The tools are described one by one in the [tool reference](../reference/window/tools.md): Place device, Rail and
Duct.
