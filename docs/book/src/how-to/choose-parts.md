# Choose parts

This guide gives a device its part, the orderable article with its manufacturer and order number, and shows where the
project's parts are listed. A device without a part is reported by the check EW-CORE-041 and is missing from the parts
list.

## What you need

- A project open for writing, with a schematic page shown.
- The starter parts that come with EW, or a library of your own.

## Give a device its part

1. Click the device on the page, for example a lamp.
2. Choose **Edit ▸ Choose part…**. The property grid's **Parts** row offers the same in its row menu.
3. In the part selection, type a few letters of the part, such as `lamp`, choose the part (EW-GEN-LMP-1 for the
   starter lamp) and confirm with **Choose**.

The device's **Parts** row now shows the part, and the warning EW-CORE-041 for the device disappears from the
Messages pane. **Project ▸ Parts** (`F9`) opens the part in Parts, where a part of your own is changed and a starter part is
made your own.

## Place a device with its part

When you know the part before you draw, place the device from it, so that the device has its part from the start:

1. Choose **Draw ▸ Place ▸ Device from part…** (`Shift+I`).
2. In the browser, double-click the part, for example the terminal EW-GEN-TRM-1; its row places the part's main
   function. To place another function, open the part with its arrow and double-click the function.
3. Click where the device goes on the page.

## Read the parts list

- **Output ▸ Open parts list** opens the parts-list page of the project, when it has one: a project made from a
  template of the template wizard (**Project ▸ New template…**), and the sample project, have one.
- **View ▸ Panels ▸ Navigators ▸ Parts in use** lists the parts the project uses with their devices, in any project.

The part data comes from the library: [Edit part](../reference/window/dialogs.md#edit-part) changes the project's
copy, and the [glossary](../reference/glossary.md#part) relates EW's part to EPLAN's article and QElectroTech's
manufacturer reference.

## The workshop stock

The workshop stock is the list of wire, ferrule and marker parts your shop keeps, in the order you prefer them. A
wire takes the first wire part of its size and colour, a wire end the first ferrule that fits it, a label the first
marker made for it; the workshop lists count them from the wires.

Open it with **Project ▸ Edit stock…**, or with the Edit stock… link under the project's Properties. Each row can move
up or down or be removed; type a part number and press **Add** to add a part, held or from a library. **Cut to** is
what the cut list rounds every rail, duct, cable and tube up to; leave it empty for 5 mm. **Apply** writes the stock in
one step.

A new project starts with the starter's generic ferrules and markers. A project made before has none: **Add the generic
stock** (in the dialog, or Project ▸ Add the generic stock) adds those the stock lacks, after its own parts.
