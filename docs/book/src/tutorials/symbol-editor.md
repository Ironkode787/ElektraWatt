# Draw a six-pin power block

In this tutorial you draw a reusable symbol: a power block with six connections, designated 1 to 5 and FE. You check
it on the connection grid, preview its eight orientations, save it with your parts, keep a portable copy of the
drawing and test the symbol on a new page of a project. At the end you have the symbol in My parts and a saved project
with a test copy, ready to wire.

## What you need

- ElektraWatt installed and started; see [Install ElektraWatt](../how-to/install.md).
- An empty folder.

## Steps

1. Choose **File ▸ New project…** (`Ctrl+N`) and pick an empty folder named `symbol-scratch.ew`.
2. Choose **Project ▸ Parts** (`F9`). Parts takes the place of the page.
3. Choose **New symbol** on the Part tab (`Ctrl+I`). The drawing opens at once on the Drawing tab, on the 4 mm
   connection grid, with the device tag's text at the upper left. On the short sheet at the left, type **Power
   block** as its **Name** and choose **Device box** as **What it is**.
4. Choose **Rectangle** and draw the body from `(4, 0)` to `(24, 28)` mm: drag from corner to corner, or click both
   corners. The rectangle is selected, and its rows in **Properties** (x, y, width, height) take exact numbers.
5. Choose **Pin**. The card at the drawing's top left holds its **Designation** and **Direction**: set the direction to
   **Facing left** and click six pins at x = 0 mm and y = 4, 8, 12, 16, 20 and 24 mm; each brings the text that writes
   its number, the designation counts on from 1, and the last one is retyped as FE in its **Designation** row. Choose
   **Line** and draw a short lead from each pin to the body's left edge, at x = 4 mm.
   The outline lists the pins, the texts and the shapes, and **To do** says **Ready to save**. FE is a printed
   designation: before you use the symbol, select the FE pin and give it its **Potential**, and never confuse it with
   PE.
6. Click B to H under **Turned and mirrored**: the drawing shows the symbol turned and mirrored, every pin keeping its
   number and turning its direction with the drawing. Choose A to edit again.
7. Select the first pin (click it in the drawing or in the outline) and change its **x (mm)** to 4.5. **To do** says
   "Connection point 1 is off the connection grid. Move it onto the grid before saving."; a click on the sentence
   selects the pin. Press `Ctrl+Z` to undo the change. Fine drawing coordinates, such as 0.5 mm, remain valid.
8. Choose **Save** at the bar's right end. A message says the symbol is saved to the library My parts: you never
   choose a folder or a key.
9. Choose **Export draft** in the bar and name the file `power.symbol.json`; it is written beside your libraries. The
   file is the portable drawing, with the identity the save gave it.
10. Choose **Test on a page**. The project's page uses another connection grid, so a new page of the 4 mm grid shows
    with a test copy of the symbol selected, ready to wire.
11. Choose **File ▸ Save** (`Ctrl+S`).
    The title bar no longer marks the project as changed.

## What you learnt

- A drawing keeps its own undo history; saving it does not change placed copies. A changed symbol is saved as the
  next version, 1.0.1 after 1.0.0.
- Pins must lie on the connection grid; Save refuses a pin off it, and says which.
- **Back** keeps the drawing and its undo history: New symbol's **Go on with the drawing in hand** returns to it. In a
  later session, open the symbol from the **Symbols** shelf of Parts, or import `power.symbol.json`.
- What you know from a page works on the drawing: a click selects, a dragged rectangle selects several things, the
  arrow keys move them a grid step, and `Delete`, `Ctrl+C`, `Ctrl+V` and `Ctrl+D` do what they do on a page.
- On a page, **Draw ▸ Place ▸ New symbol** draws a symbol for that page, on its grid, and its **Save and place**
  puts it there.

The same workflow is available to AI tools and repeatable scripts in
[`symbol-editor.ewui`](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/book/tutorial-scripts/symbol-editor.ewui).
Run it in a fresh directory:

```text
elektrawatt --headless --script symbol-editor.ewui --config-dir config
```

The script creates the project beside itself, saves the symbol into My parts and `power.symbol.json` beside your
libraries, and opens the drawing again. See the [reference](../reference/symbol-editor.md) for JSON, actions and
import compatibility.
