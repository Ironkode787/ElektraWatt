# Draw a panel layout

A panel layout page shows an enclosure's mounting plate to scale, with its rails, its wiring ducts and the devices
placed on them. The devices are the project's own: a device of the schematic is placed once, and the panel layout,
the parts list and the enclosure legend read it from there.

## Make the page

1. Choose **Page ▸ New page…**, set **Type** to Panel layout, give the page a title such as "Panel layout", and
   choose **Create**. The page opens empty; the Draw tab's Rail and Duct say what is missing.
2. An enclosure belongs to a location (`+K1`). If the project has none yet, add one with **Project ▸ Setup ▸
   Structure node…**: kind Location, designation `K1`.

## Add the enclosure

3. Choose **Project ▸ Setup ▸ Enclosure…**. The dialog proposes the page's location and the first enclosure part of the
   project or of an installed library, such as "Wall enclosure 600 × 800 × 250 mm"; choose another from the lists if
   you like, then **Create**.
   The page now shows the enclosure and its mounting plate, centred and at the scale that fits the page. Another
   plate goes on the page through the page's **Surface** row in Properties.

## Rails and ducts

4. Choose **Draw ▸ Rail** and click the rail's start and end on the plate (a rail runs across or up and down); the
   tool's card offers the rail part and the grid step. Draw as many rails as the cabinet needs.
5. Choose **Draw ▸ Duct** and draw the ducts the same way; the card asks for a duct's size when its part gives none.

## Place the devices

6. Show **View ▸ Panels ▸ Navigators ▸ Unplaced devices**. It lists the devices of the page's location that have no
   place yet. A device without a location is not listed: give it the location in Properties, under Structure.
7. Drag a device from the list onto a rail, or select its row, press `Enter` and click the place. On a rail it
   snaps to its neighbour, the rail's ends or the plate grid; a device mounted by screws goes on the plate.
8. To fill a rail at once, select the rail and choose **Draw ▸ Auto-fill rail…**: it places the devices you marked in
   the list, or every unplaced device that fits, in the order of the schematic or of their tags, and a toast says what
   did not fit.

The tools are described one by one in the [tool reference](../reference/window/tools.md): Place device, Rail and
Duct.
