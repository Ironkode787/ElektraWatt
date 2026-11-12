# A terminal strip and a cable

In this tutorial you take the lamp circuit of the first tutorial out of the cabinet: a third lamp sits on the machine,
and it is fed through a terminal strip in the cabinet and a cable. You place the strip's terminals, wire their
cabinet side and their field side, create the cable with its cores, and read the strip in the strip editor and the
cable in the Cables navigator. At the end you have a saved project whose checks show no error.

## What you need

- The project of the tutorial [A first schematic page with potentials](first-page.md), `first-page.ew`, as you saved
  it at its end.

## Steps

1. Choose **File ▸ Open…** (`Ctrl+O`) and open `first-page.ew`.
   Its page shows the transformer -1T1 and the lamps -1E1 and -1E2.
2. Choose **Draw ▸ Place ▸ Terminal strip**.
   EW creates an empty terminal strip. It takes its tag when its first terminal is placed.
3. Choose **Draw ▸ Place ▸ Symbol…** (`I`) and choose **terminal**. In the list of strips, choose the new strip. Click
   twice, to the right of -1E2 and level with it, to place two terminals side by side. Choose **Draw ▸ Select**.
   The terminals -1X1:1 and -1X1:2 are on the page. Each has its cabinet side, clamp a, at the bottom and its field
   side, clamp b, at the top.
4. Choose **Draw ▸ Place ▸ Symbol…** again, choose **lamp**, and click above and to the right of the terminals. Choose
   **Draw ▸ Select**.
   The lamp -1E3, the lamp on the machine, is on the page.
5. Choose **Draw ▸ Connect ▸ Wire** (`W`). Draw the cabinet side: from X1 of -1E2 to the bottom of -1X1:1, and from X2
   of -1E2 to the bottom of -1X1:2. Then the field side: from the top of -1X1:1 to X1 of -1E3, and from the top of
   -1X1:2 to X2 of -1E3. Choose **Draw ▸ Select**.
   The terminal -1X1:1 carries 24VAC and -1X1:2 carries 0VAC to the lamp on the machine.
6. Click the field wire of -1X1:1, then Shift+click the field wire of -1X1:2. Choose **Draw ▸ Connect ▸ Cable…**
   (**Cable** on the Draw tab). The form proposes the tag -1W1 and lists the two wires with the core each gets. Type
   `3G1.5` as the **Type**, a three-core cable of 1.5 mm² whose last core is green-yellow: the first wire gets the
   core BN, the second BU. Choose **Apply**.
   The toast says "Cable -1W1 made from 2 wires": the cable is made with its three cores and the
   two field wires are its cores, in one step that one Undo takes back. The cable takes its page number from its
   first core, and the wires take the cores' 1.5 mm² and colours.
7. Choose **View ▸ Panels ▸ Navigators ▸ Cables** and click the cable -1W1. In the Properties pane, set **Length** to
   `5 m`.
   The Cables navigator lists the cable -1W1 with its three cores: BN, BU and GNYE.
8. Click the terminal -1X1:1 and choose **Edit ▸ Strip editor**.
   The Terminal strips navigator shows the strip -1X1 as a table: terminal 1 on 24VAC, with -1E2:X1 on side 1 and
   the field wire to -1E3:X1 via -1W1 BN on side 2; terminal 2 on 0VAC, with -1E2:X2 and -1E3:X2 via -1W1 BU.
9. Choose **Check ▸ Run checks** (`F7`).
   The toast says "Check: 0 errors, 8 warnings, 2 infos". The warnings are the devices that have no part yet and the
   transformer's open primary pins; the infos say that no report covers the strip or the cable yet.
10. Choose **File ▸ Save** (`Ctrl+S`).
    The title bar no longer marks the project as changed.

## What you learnt

- A terminal strip is a device of its own, and each terminal you place from the strip's list is one of its terminals,
  with a cabinet side and a field side.
- **Draw ▸ Connect ▸ Cable…** makes the selected wires a cable's cores in one form: the cable's type as you write it
  (`3G1.5`, `4x0.75`, or a cable part's number) gives the cores, and each wire's core can be changed before Apply.
  The same command on a cable's wire opens that cable, to add a wire or take one out. A cable without a drawn cable
  line takes its page number from its first core.
- The strip editor and the Cables navigator show what passes through each terminal and each core, from the drawing.
- The reports that list them, the terminal diagram and the cable list, are the subject of
  [Reports and PDF](reports-and-pdf.md).
