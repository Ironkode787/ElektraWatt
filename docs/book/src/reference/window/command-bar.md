# The command bar's buttons

The command bar at the top of the window holds the program's commands in tabs: Project, Page, Draw, PLC, Fluid,
Check, Output and View. A button that cannot be used now is greyed out, and its tooltip says why. `F1` over a button
opens its section here; the tools of the Draw tab have theirs on the [Tools](tools.md) page, and a button with a
dialog of its own opens that dialog's section.

## Project

### Project properties

Shows the project's own rows in Properties: the document title, the drawing number, the parties and the settings the
title blocks print. See [Properties](panels.md#properties).

### Structure node…

Makes a structure node: a function (`=`), a location (`+`, a cabinet such as `+CAB1`) or a product (`-`). Devices and
pages are given their nodes in Properties. See [Structure identifiers](panels.md#structure-identifiers).

### Enclosure…

Makes a cabinet's enclosure from an enclosure part, with its mounting plate, at a location. On a panel layout page the
page shows the plate; a location that has its enclosure already gets it drawn on the page instead (**Draw -UC1 of
+CAB1 on this page**). On another page the button makes a panel layout page after it and draws the enclosure there;
**Create only** makes it without drawing it. See [Draw a panel layout](../../how-to/panel-layout.md#add-the-enclosure).

## Page

### New page…

Makes a page: its number, its type (multi-line, single-line, panel layout, fluid, report and others), its title and
its grid. See [Pages](panels.md#pages).

### New section…

Makes a section of the drawing set: a range of page numbers with a title, such as 100 to 199 "Power". New pages in
that range belong to it.

### Renumber pages…

Gives the pages new numbers in their order, in steps you choose, keeping the cross-references right.

### Move page up

Moves the page shown one place up in the page order.

### Move page down

Moves the page shown one place down in the page order.

### Add legend

On a panel layout page, puts the legend beside the layout: each item number of the page with its tag, quantity, part
number and description. See [Draw a panel layout](../../how-to/panel-layout.md).

### Report block…

Puts a list on a page: a parts list, a terminal diagram, a cable list and the others, generated from the project and
kept up to date. See [Lists on a page](things.md#lists-on-a-page).

### Open parts list

Shows the project's parts list: every part with its quantity, its description and the tags of its devices.

## Draw

### Supply

Places the supply that comes in from the mains in one go: its form asks what comes in (single phase L, N, PE; three
phase with or without N; 24 V DC; your own names) and where it comes from, the marks follow the pointer as one group,
and a click places them, one under the other; their Potential panel then asks what each carries. See
[Supply comes in here](things.md).

### Cable…

Makes a cable of the selected wires, or gives a wire's cable its part, and draws the cable's mark across them. See
[Cables and their marks](things.md#cables-and-their-marks).

### Renumber devices…

Gives the devices new numbers by their place in the drawings (`Ctrl+Shift+N`).

### Symbol editor…

Opens the symbol editor: a new symbol, a copy of a library symbol, or the box of a device. See
[Symbol editor](../symbol-editor.md).

### Auto-fill rail…

On a panel layout page, places the cabinet's devices that are not placed yet on a rail, in the order of the schematic
or of their tags. Devices without a location can be given the cabinet's in the same step. See
[Draw a panel layout](../../how-to/panel-layout.md).

## PLC

### New station…

Makes a PLC station: a CPU or a head station with its rack, chosen from the catalogue. See
[Make a station](../../how-to/plc-workspace.md#make-a-station).

### Rack…

Opens the selected station's view: its rack with a card per slot, the catalogue beside it, and the module's channels.
See [Read and change a rack](../../how-to/plc-workspace.md#read-and-change-a-rack).

### Attach modules without a station

Gives the modules that have no station yet (cards made on their own) the selected CPU's station.

### Preview PLC addressing

Shows the addresses the channels get before they are given: the old and the new address of each channel. See
[Addresses](../../how-to/plc-workspace.md#addresses).

### Symbol policy…

Sets how the channels' symbol names are made: by hand, or from a template such as `{target.dt}_{pin}`. See
[The PLC grid and the symbols](../../how-to/plc-workspace.md#the-plc-grid-and-the-symbols).

### Process-tag scheme…

Sets the form of the process tags the instrument list and the channels carry.

### Channel addresses at terminals

Prints a channel's address (`I0.0`) at the terminals its wire runs through, on or off for the project.

### Export to PLC software…

Writes the project's stations, modules and symbols to a file the PLC software reads. See
[Exchange with the PLC software](../../how-to/plc-workspace.md#exchange-with-the-plc-software).

### Import from PLC software…

Reads a file of the PLC software and shows what it would change before it does. See
[Exchange with the PLC software](../../how-to/plc-workspace.md#exchange-with-the-plc-software).

### Import instrument list…

Reads an instrument list (a table of the field devices with their signals) and shows the channels and devices it
would make before it does.

### New bus page…

Makes a bus topology page, where the bus devices and their ports are drawn. See
[Bus ports and nets](../../how-to/plc-workspace.md#bus-ports-and-nets).

### Place port

Places a port of a bus device that is not drawn yet on the bus page. See
[Bus ports and nets](../../how-to/plc-workspace.md#bus-ports-and-nets).

### Bus panel

Shows the Bus panel: the bus nets and the ports on them. See [Bus](panels.md#bus).

### PLC grid

Shows the PLC grid in the bottom dock: every channel in a row, with its address, its symbol and its device. A column
that cuts a text grows to show it. See [PLC grid](panels.md#plc-grid).

### PLC workspace

Arranges the window for PLC work: the PLC panel on the left, the PLC grid below. See
[Open the workspace](../../how-to/plc-workspace.md#open-the-workspace).

### PLC guide

Opens the guide to the PLC workspace: [Work in the PLC workspace](../../how-to/plc-workspace.md).

## Fluid

### New fluid page…

Makes a fluid page, where the pneumatic or hydraulic circuit is drawn. See
[Draw a circuit](../../how-to/fluid-workspace.md#draw-a-circuit).

### Fluid symbol…

Places a fluid symbol on the fluid page: a valve, a cylinder, a service unit and the others. See
[Valves and cylinders](things.md#valves-and-cylinders).

### Valve terminal…

Makes a valve terminal with its positions, or draws a station's valve terminal on the fluid page. See
[Valve terminals](../../how-to/fluid-workspace.md#valve-terminals).

### Air supply…

Places the air supply: where the compressed air enters the circuit.

### Generate overview…

Makes the overview page of the selected valve terminal: its positions with their valves and order codes.

### Draw the plug on an electrical page…

Draws the multipin plug of the selected valve terminal on an electrical page, with a pin for each solenoid.

### Plug the open ports

Plugs the open working ports of the selected valves in one step, so that no port is left open.

### New medium…

Defines a medium (compressed air, hydraulic oil) with its colour and pattern. See
[The medium](../../how-to/fluid-workspace.md#the-medium).

### Medium table

Shows the media of the project and the tube each medium takes. See [Medium table](panels.md#medium-table).

### Tube table

Shows the tubes of the project with their medium, size and length. See [Tube table](panels.md#tube-table).

### Fluid settings…

Sets the fluid defaults of the project: the default tube of every line and how the tubes are numbered.

### Fluid workspace

Arranges the window for fluid work: the Fluid panel on the left, the tube table below. See
[Open the workspace](../../how-to/fluid-workspace.md#open-the-workspace).

### Fluid power guide

Opens the guide to the Fluid workspace: [Work in the Fluid workspace](../../how-to/fluid-workspace.md).

## Check

### Run checks

Checks the project now and lists the findings in the Messages panel, with the change since the last check. See
[Messages](panels.md#messages).

### Next finding

Goes to the next finding of the Messages panel and selects its things on the page.

### Previous finding

Goes to the previous finding of the Messages panel.

### Highlight

Marks the thing of the selected row of a list (a potential, a wire, a medium) on the pages, in a colour, until
**Clear highlights** (`Ctrl+Shift+H`).

### Clear highlights

Removes every highlight from the pages.

## View

### Back

Goes back to the place shown before the last jump (a cross-reference followed, a page opened).

### Forward

Goes forward again after **Back**.

### Follow cross-reference

Goes to the place the cross-reference under the pointer or selected names.

### Zoom in

Shows the page larger, around the middle of the view.

### Zoom out

Shows the page smaller.

### Actual size

Shows the page at its printed size.

### Fit page

Shows the whole page in the view.

### Side by side

Shows two pages side by side, each in its own group of tabs.

### Grid

Shows or hides the drawing grid on the page.

### Theme

Chooses the window's colours. See [Appearance](../../how-to/application-window.md#appearance).

### Density

Chooses how close the window's rows and buttons stand.

### UI scale

Makes the whole window larger or smaller.

### Show tips

Shows or hides the tips the window gives on the canvas and in empty panels.

### Frame statistics

Shows how long the window takes to draw each frame, for a report of a slow window.
