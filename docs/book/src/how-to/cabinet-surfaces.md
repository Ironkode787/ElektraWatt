# Place devices in the door, and make holes and cable glands

A cabinet has more surfaces than its mounting plate: the door, the two sides, the top and the bottom. Each one is a
panel layout page of its own, drawn as you see it from the side you work it from: the door from outside, the bottom
from below. What you place on a surface brings its holes and cut-outs, and the hole table lists them for the workshop.

You need a panel layout page with its enclosure and its devices, as [Draw a panel layout](panel-layout.md) makes it.
The sample project has its mounting plate on page 902, its door on page 904 and its bottom on page 905.

## Open the door's page

1. On the plate's page, the HUD says when devices wait for the door, such as "2 devices go in a cut-out", with the
   button **Door**. A pushbutton or a lamp dropped on the plate is refused with "-102S1 goes in a cut-out of a door or
   a wall: place it on the door's page.", and the toast offers **Open the door**.
2. Choose **Door** or **Open the door**. The door's page opens; a door on no page yet gets one, after the plate's
   page and with its legend page, in one step of Undo. For any surface, choose **Surface…** in the menu of
   Enclosure's arrow on the Draw tab, or the surface in the page's right-click menu (**Surface: Door**). Surface…
   asks "Which surface of -UC1?" and lists each surface with its size and its page, such as "Door · 600 × 800 mm ·
   hinged left · page 904"; its button reads **Go to page 904**, or **Draw on a new page** for a surface on no page
   yet.

## Place devices in the door

3. Show **View ▸ Panels ▸ Navigators ▸ Unplaced devices** and drag a pushbutton or a lamp onto the door. The door's
   page shows the plate's devices faintly behind the door, on the screen only, so that you see what stands behind.
   The cut-out shows under the device, and it lines up with the centres and edges of the others. `R` turns it by a
   quarter turn.
4. A device with one part in the door and one on the plate, such as a main switch with its door coupling handle, is
   listed once per part to place. Put the handle on the door in front of its switch.

## Holes and cut-outs

5. A part brings its holes: a pushbutton its cut-out, a rail and a duct their fixing holes. Select the device, the
   rail or the duct: Properties shows the section **Holes**, one row per hole with its place from the surface's
   corner ("270, 480 mm") and, in its chip, its number and how it is made ("H3 · Ø 22.3").
6. To move one of these holes, type another place into its row: the chip then says "moved", and **Reset** puts the
   hole back. Tick **Leave out** below a hole you do not make; it is then drawn and listed nowhere.
7. Your shop's way of making holes is set once, in **Project properties ▸ Workshop**: **Fixing holes** (Tapped,
   Self-tapping screws, or Through, with a nut), **Rail fixing** and **Duct fixing** (**Screw**, **Holes at most …
   apart** and **From each end**), **Web between holes** and **Measured from**, the corner every surface's holes are
   measured from. The command search finds each of these rows by its words, such as "rail fixing", and opens it. When a new Rail fixing or Duct fixing puts a rail's or a duct's holes elsewhere, a hole you moved
   or left out there is reset, and the card of the change says how many and on which rails and ducts before you apply
   it.
8. To add a hole of your own, right-click the surface where it goes and choose **Add hole here…**. The form "Add a
   hole to -UC1 door" asks for the **Kind** (Hole, Fixing, Cut-out, Gland or Rectangle), the **Size**, **X (mm)** and
   **Y (mm)** from the corner the surface is measured from (the point you clicked) and what it is **For**, such as
   "Earth stud". Choose **Add**. A size that does not read is refused in words: "Give the hole's size: a diameter for a
   hole, M3 to M8 for a fixing, 16, 22 or 30 for a cut-out, M12 to M63 for a gland, width × height for a rectangle."
9. A hole of your own is dragged with Select, or moved by typing its X and Y in Properties, whose rows are Kind,
   Size, X, Y and For, and **Made**, which says "By hand"; `Delete` removes it.
10. Choose **Check ▸ Check project**. **Holes too close** (EW-LAY-012) names both holes and how far apart they are,
    such as "… is 3 mm from … on …: keep at least 5 mm between holes", a hole inside a door's edge margin ("… lies
    12 mm from the edge of …, inside its 30 mm margin and the 5 mm web; move it inward"), and a hole off the sheet.
    Move one of the holes, or set the web your shop keeps (step 7). **Holes not known** (EW-LAY-017) names a part in
    a cut-out that has no holes: **Edit part…** opens it in Parts, where you give it its holes under **In the
    cabinet** ([Make your own parts](own-parts.md)).

## Cable glands

11. A cable that leaves the cabinet, such as the supply cable or a motor cable, brings its cable gland, chosen from
    the workshop stock by the cable's outer diameter. While glands wait, the plate's HUD says "2 glands to place" with
    the button **Bottom**, which goes to the bottom's page.
12. On the bottom's page the HUD offers **Place cable glands**, which places every gland that waits in a row. Or drag
    them from Unplaced devices, where they are listed under **Glands** by their cable's tag ("gland M25"): several
    dropped together stand in a row, each as far from the next as their nuts need, and the outline of each nut
    follows the pointer until you let go. A gland goes in a side, the top or the bottom; the door and the plate refuse
    it.
13. Select the cable: in Properties, under its part, **Gland** names the gland part with the chip "proposed for
    12.5 mm", and **Gland hole** names its hole, "H2" with "on page 905", or says "not placed". Type another gland part
    of the project into Gland to choose it; an empty Gland takes the proposal back. Select a placed gland itself: its
    Properties are a hole's, and **Made** says "For the gland of -101W1" with **Select the cable**.
14. Check project names a gland that waits (**Cable gland**, EW-LAY-018, an error when you release) with **Place on
    the bottom**. A cable whose gland changes keeps the hole you made, and the finding offers **Take** with the new
    size, such as **Take M32**. A gland hole whose cable is gone offers **Remove the hole**; to keep it as a spare
    entry, right-click the hole and choose **Keep it as a hole**.

## The hole table

15. Choose **Output ▸ Workshop ▸ Hole tables**. The hole table page lists every hole and cut-out of each surface,
    surface by surface, numbered in the order they are made, with X and Y from the corner its heading names (such as
    "Door, page 904, seen from outside, measured from the bottom-left corner"), how each is made ("Ø 4.2, tap M5",
    "Ø 22.3") and what it is for. A project without a hole table opens **Report block…** on it.

The drawings for drilling, the template to print at full size and the DXF, are in
[Export the labels, the drilling drawings and the package](workshop-package.md). The rules are in the
[rule reference](../reference/rules/ew-layout.md).
