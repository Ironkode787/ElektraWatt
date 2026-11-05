# Draw a six-pin power block

Create a project from the built-in template. Open **Edit → Symbol editor**, name it **Power block**, and use the
key `example.power`. Draw a rectangle from `(4, 0)` to `(24, 28)` mm. Choose the connection tool, outward direction
Left, and add six pins at x = 0 mm, y = 4, 8, 12, 16, 20 and 24 mm. Set their designations to 1, 2, 3, 4, 5 and FE.
Draw a short lead from each pin to the left edge of the body. FE is a printed designation; assign its electrical
role under **Electrical data → Function definition** after checking the intended device. It must not be confused with PE.

Add a tag text field above the body and a designation field beside each connection, selecting its pin index.
Review A–H in the strip. The large preview follows the selected variant; choose A to resume editing.
Try changing the first pin's x coordinate to 4.5 mm: the editor refuses publication. Undo that edit.
Fine drawing coordinates, such as 0.5 mm, remain valid.

In **Library**, choose a writable `.ewlib` folder and save. Test the symbol on a new scratch page, close the saved draft, and
inspect the placed copy. Reopen the portable draft to continue editing. Increment its semantic version before
publishing changed content under the same key. Previously placed copies keep their original version.

The same workflow is available to AI tools and repeatable scripts in
[`symbol-editor.ewui`](../../tutorial-scripts/symbol-editor.ewui). Run it in a fresh directory:

```text
elektrawatt --headless --script symbol-editor.ewui --config-dir config
```

The script creates a sample project and library alongside itself. It exports `power.symbol.json`, which the
window can open. See the [reference](../reference/symbol-editor.md) for JSON, actions and import compatibility.
