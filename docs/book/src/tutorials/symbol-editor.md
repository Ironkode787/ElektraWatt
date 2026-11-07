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
2. Choose **Draw ▸ Place ▸ Symbol editor…**. Type **Power block** as its **Name**, and in **Library** type the key
   `example.power`.
   The editor takes the place of the page, with its own command bar.
3. In **Drawing**, choose **Rectangle** and draw the body from `(4, 0)` to `(24, 28)` mm: click both corners, or type
   them under **Draw by coordinates** and choose **Add**.
4. Choose **Connection point**, set its direction to **Facing left**, and add six connection points at x = 0 mm and
   y = 4, 8, 12, 16, 20 and 24 mm, with the designations 1, 2, 3, 4, 5 and FE. Choose **Line** and draw a short lead
   from each connection point to the body's left edge, at x = 4 mm.
5. Choose **Text slot**. Add a **Pin designation** field beside each connection point, with that point's pin number,
   and a **Device tag** field above the body.
   The validation says **Ready to save**. FE is a printed designation: before you use the symbol, give the FE pin its
   electrical role under **Electrical data**, **Function definition**, and never confuse it with PE.
6. Click B to H under **Variants**: the large preview shows the symbol turned and mirrored, every pin keeping its
   number and turning its direction with the drawing. Choose A to edit again.
7. In **Electrical data**, under **Connection points**, change the first point's x to 4.5 mm.
   The validation says "Connection point 1 is off the connection grid. Move it onto the grid before saving." Press
   `Ctrl+Z` to undo the change. Fine drawing coordinates, such as 0.5 mm, remain valid.
8. In **Library**, type `symbols.ewlib` as the **Library folder** and choose **Save to library**: a name without a
   folder is a library in your user library folder. The status bar says **Saved**.
9. Under **Open or import**, type `power.symbol.json` as the **Symbol file** and choose **Export draft**; it is written
   beside the library. The file is the portable draft, with the identity the save gave it.
10. Choose **Test on a new page**.
    The editor makes way for a new page with a test copy of the symbol selected, ready to wire.
11. Choose **File ▸ Save** (`Ctrl+S`).
    The title bar no longer marks the project as changed.

## What you learnt

- The symbol editor keeps its own draft and undo history; saving it to a library does not change placed copies.
  Increase the symbol's version before you save changed content under the same key.
- Connection points must lie on the 4 mm connection grid; the editor refuses to save a point off it, and says which.
- **Close editor** keeps the draft and its undo history: **Draw ▸ Place ▸ Symbol editor…** returns to it. In a later session,
  open `power.symbol.json` under **Open or import**: its next save updates the same symbol.

The same workflow is available to AI tools and repeatable scripts in
[`symbol-editor.ewui`](https://github.com/Ironkode787/ElektraWatt/blob/main/docs/book/tutorial-scripts/symbol-editor.ewui).
Run it in a fresh directory:

```text
elektrawatt --headless --script symbol-editor.ewui --config-dir config
```

The script creates the project beside itself and the library and `power.symbol.json` in your user library folder,
and reopens the draft. See the [reference](../reference/symbol-editor.md) for JSON, actions and import compatibility.
