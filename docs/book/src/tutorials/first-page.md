# A first schematic page with potentials

In this tutorial you draw your first schematic page in the ElektraWatt window: a small control transformer that feeds
two cabinet lamps. You place the symbols, draw the wires, give the two potentials their names and read what EW knows
about the circuit. At the end you have a saved project whose checks show no error.

## What you need

- ElektraWatt installed and started; see [Install ElektraWatt](../how-to/install.md).
- An empty folder for the project, or a place where EW may create one.

Nothing else: the project starts from EW's built-in basic project.

## Steps

1. Choose **File ▸ New project…** (`Ctrl+N`) and pick an empty folder named `first-page.ew`.
   EW creates the project from the basic project and shows its one empty page, folio 1, with its frame.
2. Choose **Draw ▸ Place ▸ Symbol…** (`I`). In the symbol picker, choose **transformer-1ph** and click near the top
   left of the page. Then choose **Draw ▸ Select** to put the tool away.
   The transformer -1T1 is on the page, with its primary pins 1U1 and 2U1 at the top and its secondary pins 1U2 and
   2U2 at the bottom.
3. Choose **Draw ▸ Place ▸ Symbol…** again, choose **lamp**, and click twice, to the right of and below the
   transformer, to place two lamps side by side. Choose **Draw ▸ Select**.
   The lamps -1E1 and -1E2 are on the page, each with the pins X1 at the top and X2 at the bottom.
4. Choose **Draw ▸ Connect ▸ Wire** (`W`). Click the transformer's pin 1U2, then the pin X1 of -1E1: the wire is
   drawn. Draw three more wires the same way: from X1 of -1E1 to X1 of -1E2, from 2U2 to X2 of -1E1, and from X2 of
   -1E1 to X2 of -1E2. Choose **Draw ▸ Select**.
   The two lamps are wired in parallel to the transformer's secondary. Each wire takes a number, such as 1.1.
5. Click the wire between the two lamps' X1 pins, then choose **Draw ▸ Connect ▸ Define potential…**. Type the name
   `24VAC`, the type `L` and the voltage `24 V AC`, and confirm the form.
   Every wire of that potential now carries the name 24VAC.
6. Click the wire between the two lamps' X2 pins, choose **Draw ▸ Connect ▸ Define potential…**, and give it the name
   `0VAC`, the type `N` and the voltage `24 V AC`.
   The circuit has its two named potentials.
7. Choose **View ▸ Panels ▸ Navigators ▸ Potentials**.
   The Potentials navigator lists 0VAC and 24VAC, each with its two wires.
8. Choose **Check ▸ Messages**.
   The Messages pane lists no error and five warnings, still pending while you draw: three devices without a part
   (EW-CORE-041: -1E1, -1E2 and -1T1) and the transformer's two open primary pins (EW-POT-009: -1T1:1U1 and
   -1T1:2U1).
9. Choose **File ▸ Save** (`Ctrl+S`).
   The title bar no longer marks the project as changed, and the status bar says Saved.

## What you learnt

- A project starts from a basic project; **File ▸ New project…** makes one in a folder of its own.
- A symbol you place is a function of a device, and the device gets its tag (-1T1, -1E1) from the numbering scheme.
- Wires join connection points; the wires that touch form a potential, which you name with Define potential….
- The checks run while you draw. A warning tells you what is still open; the [rule reference](../reference/rules/index.md)
  explains each rule, and **Help ▸ Documentation** on a finding opens its rule's page.
