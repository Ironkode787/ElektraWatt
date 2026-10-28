# Draw a page from the text interface

This guide is for drawing without a screen: a headless UI script (`elektrawatt --headless --script`), or any client
that reads the interface as text (`ui.describe`) and the content of a page as text (`page.describe`). Every step below
is an action of the same registry as the window's menus; the parameters are those the action list prints.

## Read the page

`page.describe` (with `folio`, or the page shown) lists the page in reading order: a `rep` line per drawn symbol with
its tag, cell, symbol and position, its `cp` lines (connection points, the wires drawn to each, the potential), a
`wire` line per drawn connection and a `junction` line per junction.

- **Positions are the positions you click.** Every `at <x> <y> mm` is a point of the sheet, the same coordinates
  `canvas.press`, `canvas.pointer` and `cursor.set` take: `cursor.set` with a `cp` line's position puts the grid cursor
  on that connection point, and the cursor line names it.
- **A `wire` line ends with a point on the wire** (`at <x> <y> mm`) where a click with the Select tool picks that wire
  and nothing else. A wire whose drawn route runs along another wire or through a symbol for its whole length has no
  such point; select it by its ends instead: `select.set` with `refs` `["wire:-101F1:1 -> F55:1"]` (the ends as the
  `wire` line prints them, either way round; a junction end is `junction <n>`).

## Place symbols

- `place.pick` (or Insert, Symbol... and the picker) arms the Place tool; `nav.goto` with a cell (`101-B2`) or
  `cursor.set` moves the ghost, and `canvas.activate` (Enter) places it.
- **Autoconnect.** When the ghost's free connection points face free connection points on the page, the ghost proposes
  wires to them: `ui.describe` prints them as `canvas proposals <n> "<cp>" -> "<target>"`, and the status hint says that
  Enter connects them. `canvas.activate` with `alt` (Alt+Enter, Alt+Click) places the symbol without connecting;
  `tool.next_proposal` (Tab) tries the next target along each ray. After a placement that connected, the toast names
  the wires it drew; one Undo removes the symbol with its wires.

## Freeze a tag

Select the device (`select.set` with `refs` `["tag:-101S1"]`) and set its frozen tag as the page should print it:
`props.set` with `row` `device/tag.frozen` and `value` `S51` prints `S51`; `-S51` prints `-S51` (the sign shown). An
empty value unfreezes it. The derived tag row's Freeze action and the `tag.frozen.text` and `tag.frozen.sign` rows do
the same in several steps.

## Terminal strips

`strip.new` creates an empty terminal strip; the toast names it. It draws nothing until its first terminal is placed:
choose the terminal symbol in the symbol picker, and a second picker lists the strips (`row strip "<tag>" terminal
"<next>" @<n>`); choose the strip there. The terminal takes the row's proposed designation unless the choice names one
(`picker.choose` with `designation` `PE`, on either picker). A strip without terminals prints its tag with question marks in it (`-X??`) because
its counter is given with its first terminal; the `@n` at the end of its row selects it (`select.set` with `ids`
`["@n"]`), and `edit.delete_selection` deletes a strip that has no terminals.

A terminal's connection points print `strip:terminal/clamp` (`-X1031:1/b`). Where two clamps of one level carry the same
letter, as on a terminal without a part, the label adds `#1` for side 1 (internal) and `#2` for side 2 (field):
`1X1:1/a#1` and `1X1:1/a#2`.

## External points

1. `structure.new` with `kind` `location` and the location (`+J-E-206`), then `external_ref.new` with the location, the
   device and the connection, once per connection. Each says in the toast what it created; the external reference's
   label is `<device>:<connection>` (`JE-N1000-03:L`).
2. With the Interruption point tool armed, `ip.external` takes that label, or the label after the location as the page
   prints it (`+J-E-206 / JE-N1000-03:L`); the ghost becomes an external point. A text that no reference has fails with
   a message that lists the labels the project has.

## Draw wires

With the Wire tool armed, put the grid cursor on a connection point (its `cp` position), press Enter, move to the other
end and press Enter. The `canvas route` line names the start (`from`) and the end (`end`) before you commit.

- `cp.next` and `cp.previous` step the cursor through the page's connection points in reading order of the cell
  **column by column** (column 0 to 16, and in a column row A to H), then by position; this differs from the page
  text's order, which goes row by row. Going to a `cp` position with `cursor.set` needs no walk.
- **Ending a wire on a wire** makes a junction there and splits that wire. The route's `end` names the wire it would
  split by its ends (`@101-C12 F55:2 -> E55:L`): check it before Enter when two wires run through one cell.

## Delete

`edit.delete_selection` (Delete) deletes what is selected on the page shown: a representation, a wire, a junction or
an item. A device selected by its tag is deleted as its representations on the page shown (the device stays, its
functions unplaced); a device that no page draws, such as an empty terminal strip, is deleted itself. A device drawn
only on another page is refused (`reason-not-on-page`).

## Save and export

`file.save` saves. `file.export_pdf` with a file name writes the PDF; the toast names the file, its page count and its
size.
