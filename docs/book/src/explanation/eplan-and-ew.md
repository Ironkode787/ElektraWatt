# EPLAN and EW

If you come from EPLAN Electric P8, most of EW will feel familiar: a project of pages, devices with functions, parts
from a database, potentials and reports that update themselves. The words are mostly the same too. This page takes
each EPLAN concept in turn: the EW term, what is the same, what differs, and where to find it in EW's window. The
[glossary](../reference/glossary.md) lists every term in one table, and each EPLAN term below is also an alias in the
command palette.

## Function

**EW term:** [Function](../reference/glossary.md#function)

**The same:** a function is one symbol's worth of a device: a coil, a contact, a pin group, placed on a page.
**What differs:** a function always belongs to a device in EW; there is no function without a device tag, and placing
a symbol creates its device at once. **In the window:** Draw ▸ Place ▸ Symbol….

## Device, Device tag, DT, BMK

**EW term:** [Device](../reference/glossary.md#device)

**The same:** a device is the physical object with its device tag (DT, the German BMK), made of functions that may sit
on several pages. **What differs:** EW derives the tag from the project's numbering scheme and the structure, so a
tag is never typed text: renumbering changes every page, report and label at once, and a tag you want to keep is
frozen. **In the window:** Draw ▸ Place ▸ Device from part… and Draw ▸ Arrange ▸ Renumber devices….

## Part, Article

**EW term:** [Part](../reference/glossary.md#part)

**The same:** a part is the orderable article with its manufacturer, order number and technical data, assigned to a
device. **What differs:** the project embeds a copy of each part it uses, so it opens and prints without the library;
updating the copy from the library is a command you run. **In the window:** Edit ▸ Choose part…. See
[Choose parts](../how-to/choose-parts.md).

## CDP, Connection definition point

**EW term:** [Wire marker](../reference/glossary.md#wire-marker)

**The same:** you set a wire's cross-section, colour or number at a point on the wire. **What differs:** in EW the
value belongs to the wire itself, and the wire marker is only the label that shows it on the page; potentials and
profiles give the default, and a value set on the wire is an override. **In the window:** Draw ▸ Connect ▸ Wire
marker.

## Interruption point

**EW term:** [Interruption point](../reference/glossary.md#interruption-point)

**The same:** an interruption point continues a potential at another point of the same name, in star or chain mode,
with cross-references. **What differs:** the pairing is checked: a point without a partner and a star with two sources
are findings in the Messages pane. **In the window:** Draw ▸ Connect ▸ Interruption point.

## Basic project

**EW term:** [Basic project](../reference/glossary.md#basic-project)

**The same:** a new project starts from a basic project that holds the company's frames, settings and structure.
**What differs:** EW ships one basic project and the company standard wizard makes your own from it. **In the
window:** File ▸ New project… and Project ▸ Company standard wizard….

## Form, Plot frame

**EW term:** [Frame and title block](../reference/glossary.md#frame-and-title-block)

**The same:** the frame and title block print around every page, filled from the project's and the page's data.
**What differs:** report pages and drawing pages take their frame from the project's frame settings by page type, and
a title-block value is bound to the project, its revision or its page, not typed per page. **In the window:**
Project ▸ Company standard wizard….

## Message management

**EW term:** [Messages](../reference/glossary.md#messages)

**The same:** the checks report messages that you can sort, jump to and accept. **What differs:** the checks run
while you draw, not only on demand, and each finding names its rule, which has a page in the
[rule reference](../reference/rules/index.md); accepting a finding is a waiver with a reason. **In the window:**
Check ▸ Messages.

## Main function, Auxiliary function

**EW term:** [Main and auxiliary function](../reference/glossary.md#main-and-auxiliary-function)

**The same:** a coil is the main function, its contacts are auxiliary functions, and the contact image lists them
with cross-references. **What differs:** a contact joins its coil's device when you place it on the coil's tag; the
contacts the part offers are checked against the ones you draw. **In the window:** Draw ▸ Place ▸ Symbol….

## Connection

**EW term:** [Connection](../reference/glossary.md#connection)

**The same:** connections join connection points and follow the drawing. **What differs:** EW builds the physical
wires from the drawn connections, junctions and interruption points, so the wire list follows the drawing without an
update step. **In the window:** Draw ▸ Connect ▸ Wire.

## Terminal strip

**EW term:** [Terminal strip](../reference/glossary.md#terminal-strip)

**The same:** a terminal strip is a device whose terminals are functions, with an editor for their order, bridges and
accessories. **What differs:** the strip editor is a table you edit cell by cell, and every edit is one undo step.
**In the window:** Draw ▸ Place ▸ Terminal strip… and Edit ▸ Strip editor. See
[A terminal strip and a cable](../tutorials/terminal-strip-and-cable.md).

## Structure identifier, Identifier block, Higher-level function, Mounting location

**EW term:** [Structure node](../reference/glossary.md#structure-node)

**The same:** the higher-level function (`=`) and the mounting location (`+`) structure the project, and a device's
full tag carries them. **What differs:** a structure node is a record of its own, so renaming it renames every tag
under it; EW keeps a code as one token and never splits it again. **In the window:** Project ▸ Project ▸ Structure
node….

## Black box

**EW term:** [Black box](../reference/glossary.md#black-box)

**The same:** a black box stands for a device that has no symbol, with connection points on its outline.
**What differs:** nothing much; a black box is a function of its device like any symbol. **In the window:** Draw ▸
Place ▸ Black box….

## Structure box

**EW term:** [Structure box](../reference/glossary.md#structure-box)

**The same:** a dashed frame that assigns what it encloses to a function or location. **What differs:** nothing
much; the assignment shows in the tags at once. **In the window:** Draw ▸ Annotate ▸ Structure box.

## Report, Update reports, Embedded report

**EW term:** [Report block](../reference/glossary.md#report-block)

**The same:** report pages such as the terminal diagram, the cable list and the parts list are generated from the
project. **What differs:** there is no update step: a report block is regenerated whenever the project changes, and
its pages are always current. **In the window:** View ▸ Panels ▸ Navigators ▸ Report blocks. See
[Reports and PDF](../tutorials/reports-and-pdf.md).

## Potential, Potential definition point

**EW term:** [Potential](../reference/glossary.md#potential)

**The same:** a potential is a connected electrical node with a name, a type and a voltage, set by a definition on
one of its connections. **What differs:** potentials are checked: two definitions that disagree, a protective
conductor with the wrong colour or a switched neutral are findings. **In the window:** Draw ▸ Connect ▸ Define
potential…. See [Define a supply potential](../how-to/define-supply-potential.md).

## Cross-reference, Contact image

**EW term:** [Cross-reference](../reference/glossary.md#cross-reference)

**The same:** cross-references point from a coil to its contacts and from an interruption point to its partner, and
the contact image under a coil lists the contacts. **What differs:** a cross-reference is computed, never text, so it
cannot go stale; one that points outside the frame is a finding. **In the window:** Page ▸ Go ▸ Follow
cross-reference.
