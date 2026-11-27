# Tutorials

A tutorial is a lesson you follow once, from start to end, to learn how EW works; a [how-to
guide](../how-to/index.md) is where you look up one job later, when you know what you want to do.

A tutorial takes you through one complete task, with every step spelled out and a visible result at the end. You do
not need to understand why a step works to do it; the [explanation](../explanation/index.md) section and the
[reference](../reference/index.md) say why and what exactly.

Each tutorial runs as a test of EW's own build, so its steps, menu paths and keys are the ones the program has.

## Available now

1. [A first schematic page with potentials](first-page.md): a new project, a transformer and two lamps placed and
   wired, the potentials named, the Potentials navigator and the Messages pane read. You need EW installed and an
   empty folder.
2. [A terminal strip and a cable](terminal-strip-and-cable.md): on the project of the first tutorial, a terminal strip
   placed and wired on both sides, a cable created from its part with the field wires as its cores, the strip editor
   and the Cables navigator read. You need the project you saved at the end of the first tutorial.
3. [Reports and PDF](reports-and-pdf.md): on a copy of the sample project, its report pages read, a parts list and a
   terminal overview added, the project exported as one PDF and the print preview read. You need EW installed; the
   copy is made in your projects folder, and a project you have open stays open.
4. [Importing a QElectroTech project](import-qet.md): a small QElectroTech project imported, its migration report
   read, its missing wire drawn and its parts chosen, until the report holds only the record of the import. You need
   EW installed and the tutorial's file [lamp-circuit.qet](files/lamp-circuit.qet).
5. [Draw a six-pin power block](symbol-editor.md): a power block drawn in Parts, checked on the
   connection grid, its eight orientations previewed, saved to a library and tested on a new page. You need EW
   installed and an empty folder.
6. [A PLC with its modules](plc-station.md): a CPU with an input and an output module made in the New station dialog
   and drawn on a page, a position switch and a lamp wired to channels, their addresses and symbols in the PLC grid,
   the PLC overview page and the export to PLC software. You need EW installed and an empty folder.
7. [A bus network](bus-network.md): a bus-topology page with a CPU, a remote I/O station and a switch from the
   generic parts, the bus lines, the node addresses, a bus cable with its plugs, a frequency converter of your own
   made in Parts, and the check. You need EW installed and an empty folder.
8. [A pneumatic circuit](pneumatic-circuit.md): a fluid page with an air supply and its medium, a service unit, a 5/2
   solenoid valve and a double-acting cylinder with flow controls, the tubes, the valve's solenoid wired to a PLC
   output, a valve terminal with its positions filled, and the tube list. You need EW installed and an empty folder.

The `ew` command-line program has a how-to of its own: [Use the command line on the sample
project](../how-to/headless-workflow.md).

## Planned

The documentation requirements (NFR-DOC-03) name the tutorial that follows at version 1.0: 2D/3D layout with the
wire list.
