# How-to guides

A how-to guide is where you look up one job when you know what you want to do; a [tutorial](../tutorials/index.md)
is a lesson you follow once, from start to end, to learn how EW works.

A guide gives short steps for one goal and no background. If you are new to EW, or new to its PLC, bus or pneumatic
pages, start with a tutorial.

## Available now

- [Install ElektraWatt](install.md): the Windows setup, silent installs, the Linux AppImage, the CLI archives and
  `EW_CONTENT_DIR`, upgrading and uninstalling.
- [Use the application window](application-window.md): the start page, new, sample and imported projects, recovery,
  the command palette, workspaces, the first-run setup, the company-standard wizard and F1.
- [Draw a page from the text interface](text-interface.md): reading a page as text, placing, autoconnect, frozen tags,
  terminal strips, external points, wires and junctions, delete and export, without a screen.
- [Use the command line on the golden project](headless-workflow.md): the `ew` program checks a project, writes its
  pages as PDF and SVG, exports the cable list and the bill of materials, and queries it in SQL. You need a checkout
  of the repository and the `ew` program.
- [Define a supply potential](define-supply-potential.md): a potential's name, type and voltage from a selected wire,
  junction or terminal.
- [Choose parts](choose-parts.md): a device's part from the property grid or the part selection, a device placed
  from its part, and where the parts are listed.
- [Draw a panel layout](panel-layout.md): a panel layout page, its enclosure and mounting plate, rails and ducts,
  and the devices placed on them.
- [Work in the PLC workspace](plc-workspace.md): the PLC and Bus navigators, the PLC grid, the rack and slot
  editor, freezing and re-addressing, and the exchange with PLC software.
- [Make your own PLC or bus part](own-parts.md): a card, a head station such as an IM 152, a switch such as a
  SCALANCE or a device on PROFINET, typed from its data sheet in the part editor and kept in My parts.
- [Work in the Fluid workspace](fluid-workspace.md): the Fluid navigator, the tube and medium tables, a medium
  and a tube on a line, and the overview page of a valve terminal.
- [Upgrade a project](upgrade-a-project.md): what a release reads and writes, opening a project of an older format,
  the backup of the first save, `ew upgrade`, and what changes in the format from 0.1.0.

More follow the [command-line how-to](headless-workflow.md), for example replaying a
command log with `ew apply`, checking a project in a CI job, and exporting for a single page range.

## Writing a how-to guide

A guide names its goal in the title ("Export one page range as SVG"), lists what you need, gives numbered steps and
ends when the goal is met. Facts that belong to a command or a rule go to the [reference](../reference/index.md), and
are linked, not repeated.
