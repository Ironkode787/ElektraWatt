# Make your own PLC or bus part

Your own hardware, a card, a head station, a switch or a drive on PROFINET, is a part you type from its data sheet in
the part editor. Save puts it into the project and into your own parts library, **My parts**, so that your next project
has it too. Nothing here depends on what ElektraWatt knows about a manufacturer's product: you type what the data sheet
says.

You need an open project.

## Start a new part

1. Choose **Edit ▸ New part…**, or **New part…** in the device browser (**Draw ▸ Device**, `Shift+I`) or in the part
   dialog (**Choose part…**). The part editor opens on a new part.
2. Type the **Manufacturer** and the **Part number**, and a **Description**. A manufacturer the project and your
   libraries do not have yet is made with the part.
3. Leave **Also add to my parts library** ticked to keep the part for later projects; untick it to keep the part in
   this project alone.

A field that does not read is refused under it, in words; **Save** says the first such field again.

## A card such as a digital input card

4. Under **PLC**, choose **PLC role ▸ Module**, **Channel kind ▸ DI** and type **Channels** `8`.
5. The **Pin pattern** gives the pin of channel n: `{n+1}` gives the pins 1 to 8, `X2:{n+1}` gives X2:1 to X2:8. The
   line under it shows the first and the last pin ("Channel 0 → pin 1 … channel 7 → pin 8"), so that a wrong pattern
   shows before you save, and the next line the input or output bits the card occupies.
6. Type the supply pins (**Supply L+ pin**, **Supply M pin**) when the card has them, and the **Allowed slots**
   (`1-8, 10`) when the data sheet restricts them. **Save**.
7. Place the card beside the CPU from the device browser: it stands as a box with its pins, joins the project's CPU,
   and its channels are addressed and bound to their pins. In **Rack and slots…** on a station's row it is offered as
   a module.

A part has channels of one kind: a card with 16 inputs and 16 outputs is two parts, one for each.

## A head station such as an IM 152

8. Choose **PLC role ▸ Station**, type the supply pins and **Most modules** (`32`).
9. Under **Bus interfaces**, choose **Add interface**: type the interface `DP`, choose **Bus ▸ PROFIBUS DP** and
   **Role ▸ Slave**. Its PROFINET sister has the interface `X1`, **Bus ▸ PROFINET**, **Role ▸ Device** and **Ports**
   `2`. **Save**.
10. Place it, add its cards in **Rack and slots…** on its row in the PLC navigator, and place its port with **Place
    port DP** on that row, on a bus-topology page. A station that only answers on the bus is a head station.

## A switch such as a SCALANCE, or a device on PROFINET

11. Leave the PLC role at **None**. Under **Bus interfaces**, **Add interface**: `X1`, **PROFINET**, and the number
    of **Ports** (sockets) of the data sheet.
    - An **unmanaged** switch has the role **Passive**: no address, it passes the net through.
    - A **managed** switch and a drive, an HMI or a gateway have the role **Device**: one PROFINET name and IP
      address for the whole interface, however many sockets it has.
12. **Save**, then place it from the device browser on a bus-topology page: it stands as a box with a connection
    point per socket, P1, P2 …. Draw the bus lines with **Draw ▸ Wire**: the CPU, the devices and the managed switch
    are one network, and an unmanaged switch stands in it with "passes the net through" in the **Bus** navigator.
    Type each node's name in the navigator's table (its **Address** column).

The starter parts hold such parts to begin with: EW-GEN-ESW-5 (unmanaged switch, 5 ports), EW-GEN-ESW-8 (managed
switch, 8 ports) and EW-GEN-PND-1 (a PROFINET device, 2 ports).

## Correct a part you made

14. Select a device of the part and choose **Edit ▸ Edit part…** (or **Edit part…** in the menu of the **Parts** row
    of its Properties). A part of your own, in My parts or kept to the project, opens with the same forms and what you
    saved: change the number of ports, the pin pattern or any field and **Save**.
15. A part of My parts is saved there as its next version, and the Preview tab lists what changes and the devices that
    use it; **Apply** brings the project's copy to it. A part kept to the project changes in place, with the same card.
16. The next device you place takes the correction. A device already drawn keeps the sockets it was drawn with and the
    bus ports it carries: delete it and place it again to draw the change. The editor says so above the form.

A starter part or a part of another library is not yours to change: its editor says so and offers **Copy as a part of
your own…**, a new part with its data, which you save under a part number of your own.

A part cannot be removed from My parts in the window yet: it stays, unused, until you delete it from the library folder.

## A part drawn with your own symbol

13. In the [symbol editor](../reference/symbol-editor.md), save the symbol with **Save to library**, then choose
    **Make a part for this symbol…**. The part editor opens on a new part drawn with that symbol; a device placed from
    the part is drawn with it.

The PLC exchange with PLC software exports the stations of a configuration project; a switch or a device without a PLC
section is not in the exported file. See [Work in the PLC workspace](plc-workspace.md) for the navigators, the rack
editor and the exchange.
