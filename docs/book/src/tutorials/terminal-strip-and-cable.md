# A terminal strip and a cable

In this tutorial you take the lamp circuit of the first tutorial out of the cabinet: a third lamp sits on the machine,
and it is fed through a terminal strip in the cabinet and a cable. You place the strip's terminals, wire their
cabinet side and their field side, create the cable with its cores, and read the strip in the strip editor and the
cable in the Cables navigator. At the end you have a saved project whose checks show no error.

## What you need

- The project of the tutorial [A first schematic page with potentials](first-page.md), `first-page.ew`, as you saved
  it at its end.
- The installed libraries of ElektraWatt, which hold the cable parts of the sample project.

## Steps

1. Choose **File ▸ Open…** (`Ctrl+O`) and open `first-page.ew`.
   Its page shows the transformer -1T1 and the lamps -1E1 and -1E2.
2. Choose **Draw ▸ Place ▸ Terminal strip…**.
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
6. Choose **Draw ▸ Connect ▸ New cable…**. In the form, choose the cable part **NEBU-G78W5-K-2-N-LE5**, a five-core
   connecting cable, and choose **Create**.
   The toast says "Created cable -?W? with 5 cores". The cable has the part's five cores; its tag shows question
   marks until you give it one in step 8.
7. Click the field wire of -1X1:1, then Shift+click the field wire of -1X1:2. Choose **Draw ▸ Connect ▸ Connect
   cores…**, choose the cable **-?W?**, and choose **Connect**.
   The toast says "Wires connected to the cores of -?W?: 2": the two field wires are now cores of the cable.
8. Choose **View ▸ Panels ▸ Navigators ▸ Cables** and click the cable -?W?. In the Properties pane, set **Frozen tag**
   to `-1W1` and **Length** to `5 m`.
   The Cables navigator lists the cable -1W1 with its five cores: BK, BN, BU, GNYE and WH.
9. Click the terminal -1X1:1 and choose **Edit ▸ Strip editor**.
   The Terminal strips navigator shows the strip -1X1 as a table: terminal 1 on 24VAC, with -1E2:X1 on side 1 and
   -1E3:X1 via -1W1 BK on side 2; terminal 2 on 0VAC, with -1E2:X2 and -1E3:X2 via -1W1 BU.
10. Choose **Check ▸ Run checks** (`F7`).
    The toast says "Check: 0 errors, 8 warnings, 3 infos". The warnings are the devices that have no part yet and the
    transformer's open primary pins; the infos say that no report covers the strip or the cable yet, and that the
    cable's tag is frozen.
11. Choose **File ▸ Save** (`Ctrl+S`).
    The title bar no longer marks the project as changed.

## What you learnt

- A terminal strip is a device of its own, and each terminal you place from the strip's list is one of its terminals,
  with a cabinet side and a field side.
- A cable is created from its part, with the part's cores; **Draw ▸ Connect ▸ Connect cores…** makes the selected
  wires its cores, in order.
- The strip editor and the Cables navigator show what passes through each terminal and each core, from the drawing.
- The reports that list them, the terminal diagram and the cable list, are the subject of
  [Reports and PDF](reports-and-pdf.md).
