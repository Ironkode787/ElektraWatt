# Draw a six-pin power block

In this tutorial you draw a reusable symbol: a power block with six connections, designated 1 to 5 and FE. You check
it on the connection grid, preview its eight orientations, save it to a library, keep a portable copy of the draft and
test the symbol on a new page of a project. At the end you have a library with the symbol and a saved project
with a test copy, ready to wire.

## What you need

- ElektraWatt installed and started; see [Install ElektraWatt](../how-to/install.md).
- An empty folder.

## Steps

1. Choose **File ▸ New project…** (`Ctrl+N`) and pick an empty folder named `symbol-scratch.ew`.
2. Choose **Draw ▸ Place ▸ Symbol editor…**. The editor takes the place of the page and shows its start card: choose
   **New symbol**. With nothing selected, **Properties** on the right shows the symbol's own rows: type **Power block**
   as its **Name**, and under **More** the **Key** `example.power`.
3. Choose **Rectangle** and draw the body from `(4, 0)` to `(24, 28)` mm: drag from corner to corner, or click both
   corners. The rectangle is selected, and its rows in **Properties** (x, y, width, height) take exact numbers.
4. Choose **Connection point**. The card at the drawing's top left holds its **Designation** and **Direction**: set
   the direction to **Facing left** and click six connection points at x = 0 mm and y = 4, 8, 12, 16, 20 and 24 mm;
   the designation counts on from 1, and the last one is retyped as FE in its **Designation** row. Choose **Line**
   and draw a short lead from each connection point to the body's left edge, at x = 4 mm.
5. Choose **Text field**. On its card choose what the page writes there: the number of each connection point, placed
   beside that point, and the **Device tag**, placed above the body.
   The outline on the left lists the connection points, the texts and the shapes, and says **Ready to save**. FE is a
   printed designation: before you use the symbol, select the FE point and give it its **Potential**, and never
   confuse it with PE.
6. Click B to H under **Turned and mirrored**: the drawing shows the symbol turned and mirrored, every pin keeping its
   number and turning its direction with the drawing. Choose A to edit again.
7. Select the first connection point (click it in the drawing or in the outline) and change its **x (mm)** to 4.5.
   **To do** says "Connection point 1 is off the connection grid. Move it onto the grid before saving."; a click on
   the sentence selects the point. Press `Ctrl+Z` to undo the change. Fine drawing coordinates, such as 0.5 mm,
   remain valid.
8. Click empty paper, so that nothing is selected, type `symbols.ewlib` as the **Library folder** and choose **Save**:
   a name without a folder is a library in your user library folder. A message says where the symbol was saved.
9. Open **Draft** in the command bar and choose **Export draft**, and name the file `power.symbol.json`; it is written
   beside the library. The file is the portable draft, with the identity the save gave it.
10. Choose **Test on a page**.
    The editor makes way for a new page with a test copy of the symbol selected, ready to wire.
11. Choose **File ▸ Save** (`Ctrl+S`).
    The title bar no longer marks the project as changed.

## What you learnt

- The symbol editor keeps its own draft and undo history; saving it to a library does not change placed copies.
  A changed symbol is saved as the next version, 1.0.1 after 1.0.0.
- Connection points must lie on the 4 mm connection grid; the editor refuses to save a point off it, and says which.
- **Close editor** keeps the draft and its undo history: **Draw ▸ Place ▸ Symbol editor…** returns to it. In a later
  session, choose **Start**, then **Import a file…**, and open `power.symbol.json`: its next save updates the same
  symbol.
- What you know from a page works on the drawing: a click selects, a dragged rectangle selects several things, the
  arrow keys move them a grid step, and `Delete`, `Ctrl+C`, `Ctrl+V` and `Ctrl+D` do what they do on a page.

The same workflow is available to AI tools and repeatable scripts in
[`symbol-editor.ewui`](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/book/tutorial-scripts/symbol-editor.ewui).
Run it in a fresh directory:

```text
elektrawatt --headless --script symbol-editor.ewui --config-dir config
```

The script creates the project beside itself and the library and `power.symbol.json` in your user library folder,
and reopens the draft. See the [reference](../reference/symbol-editor.md) for JSON, actions and import compatibility.
