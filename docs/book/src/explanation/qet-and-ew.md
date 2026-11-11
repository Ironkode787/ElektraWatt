# QElectroTech and EW

If you come from QElectroTech, EW draws pages the same way, but it knows more about what you draw: every symbol is a
function of a device, every line is a connection that carries a potential, and terminals, cables and reports follow
from the drawing. This page takes each QElectroTech concept in turn: the EW term, what is the same, what differs, and
where to find it in EW's window. The [glossary](../reference/glossary.md) lists every term in one table, and each
QElectroTech term below is also an alias in the command palette. To bring a QElectroTech project over, see
[Importing a QElectroTech project](../tutorials/import-qet.md).

## Element, Element instance

**EW term:** [Function](../reference/glossary.md#function)

**The same:** you pick a symbol from the collection and place it on a folio. **What differs:** the placed symbol is a
function of a device, not a free element; the device holds the tag, the part and the other functions. **In the
window:** **Draw ▸ Place ▸ Symbol…**.

## Element label

**EW term:** [Device](../reference/glossary.md#device)

**The same:** a label such as -K1 names the element. **What differs:** in EW the label is the device's tag, derived
from the numbering scheme and the structure, so you do not type it; all functions of one device share it.
**In the window:** **Draw ▸ Arrange ▸ Renumber devices…**.

## Manufacturer reference

**EW term:** [Part](../reference/glossary.md#part)

**The same:** the element can name the article you order. **What differs:** a part is a record with its
manufacturer, order number and data, embedded in the project and listed in the parts list. **In the window:** **Edit ▸
Choose part…**. See [Choose parts](../how-to/choose-parts.md).

## Conductor text

**EW term:** [Wire marker](../reference/glossary.md#wire-marker)

**The same:** a text on the wire shows its number or function. **What differs:** the number is a property of the
wire, derived by the numbering scheme; the wire marker only shows it. **In the window:** **Draw ▸ Annotate ▸ Wire
marker**.

## Folio report

**EW term:** [Interruption point](../reference/glossary.md#interruption-point)

**The same:** a folio report continues a conductor on another folio, with a reference to it. **What differs:** an
interruption point continues a potential by its name and is paired by EW; an unpaired point is a finding.
**In the window:** **Draw ▸ Connect ▸ Interruption point**.

## Template project

**EW term:** [Basic project](../reference/glossary.md#basic-project)

**The same:** a new project starts from a template. **What differs:** the basic project also holds the company's
check scheme and numbering, and the company standard wizard makes your own. **In the window:** **File ▸ New project…**.

## Title-block template

**EW term:** [Frame and title block](../reference/glossary.md#frame-and-title-block)

**The same:** a template draws the title block and fills its fields. **What differs:** the fields are bound to the
project, the revision and the page, so a value is set once. **In the window:** **Project ▸ Company standard wizard…**.

## Master/slave, Master, Slave

**EW term:** [Main and auxiliary function](../reference/glossary.md#main-and-auxiliary-function)

**The same:** a coil is the master and its contacts are slaves, linked with cross-references. **What differs:** the
contacts are functions of the coil's device, and the part says how many it has; drawing more is a finding.
**In the window:** **Draw ▸ Place ▸ Symbol…**.

## Conductor

**EW term:** [Connection](../reference/glossary.md#connection)

**The same:** a conductor joins two terminals of elements. **What differs:** a connection belongs to a potential
and becomes part of a physical wire with a cross-section, colour and number, which the wire list and the terminal
diagram show. **In the window:** **Draw ▸ Connect ▸ Wire**.

## Terminal strip

**EW term:** [Terminal strip](../reference/glossary.md#terminal-strip)

**The same:** terminals are grouped in strips with a terminal-strip editor. **What differs:** a terminal strip is a
device, its terminals are its functions, and the terminal diagram is generated from it. **In the window:** **Draw ▸
Place ▸ Terminal strip…** and **Edit ▸ Strip editor**. See
[A terminal strip and a cable](../tutorials/terminal-strip-and-cable.md).

## Plant, Location

**EW term:** [Structure node](../reference/glossary.md#structure-node)

**The same:** plant and location codes group the elements and print in their labels. **What differs:** a structure
node is a record, and a device's tag carries it, so renaming the node renames every tag below it. **In the window:**
**Project ▸ Setup ▸ Structure node…**.

## Rectangle

**EW term:** [Black box](../reference/glossary.md#black-box)

**The same:** a rectangle stands for a device that has no symbol of its own. **What differs:** a black box has
connection points on its outline, so wires connect to it. **In the window:** **Draw ▸ Place ▸ Black box…**.

## Dotted shape

**EW term:** [Structure box](../reference/glossary.md#structure-box)

**The same:** a dotted frame groups what it encloses. **What differs:** a structure box assigns its contents to a
function or location, which their tags then carry. **In the window:** **Draw ▸ Annotate ▸ Structure box**.

## Summary, Nomenclature, Folio list

**EW term:** [Report block](../reference/glossary.md#report-block)

**The same:** the folio list and the nomenclature are generated pages. **What differs:** a report block is always
current, and EW offers more of them: terminal diagrams, cable lists and the parts list. **In the window:** **View ▸
Panels ▸ Navigators ▸ Report blocks**. See [Reports and PDF](../tutorials/reports-and-pdf.md).

## Cross-reference

**EW term:** [Cross-reference](../reference/glossary.md#cross-reference)

**The same:** a reference points from a coil to its contacts and from a folio report to its partner. **What differs:**
EW computes it from the model, so it cannot go stale. **In the window:** **Page ▸ Go ▸ Follow cross-reference**.
