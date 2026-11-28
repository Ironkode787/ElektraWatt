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

The workshop stock is the list of wire, ferrule, marker and cable gland parts your shop keeps, in the order you prefer
them. A wire takes the first wire part of its size and colour, a wire end the first ferrule that fits it, a label the
first marker made for it, a cable that leaves the cabinet the first gland that seals it; the workshop lists count them
from the wires.

Open it with **Project ▸ Edit stock…**, with the Edit stock… link under the project's Properties, or in Parts with
**Workshop stock** beside the scopes: the list shows the stock's parts in their order, under a heading per kind that
says the rule ("Ferrules · the first that fits"). Drag a row to another place among its kind to change the order; click
a row to open its part.

Open a part of the stock: a line under its heading says where it stands ("number 1 of the 13 of its kind"), with
**Up**, **Down** and **Take out**. A wire, ferrule, marker or gland not in the stock, your own or a starter part, says so
there with **Add to the stock**, which puts it first of its kind: you add a part because you prefer it. Each change is one step, which **Undo** takes back. **Add from my parts…** shows My
parts to open one of yours; **Add the generic stock** (also Project ▸ Add the generic stock) adds the starter's
ferrules, markers and glands the stock lacks, after its own parts.

**Use this stock for new projects** keeps the stock, in its order, for every project you make next; until you use it
a new project starts with the starter's generic ferrules, markers and glands. **Cut to**, what the cut list rounds
every rail, duct, cable and tube up to, is a row of the project's Properties under Workshop; empty, it is 5 mm.
