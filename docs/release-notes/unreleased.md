<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Unreleased changes

The changes since the last release, 0.3.0, one line each, and nowhere else: a change that a user, a script or a
reader of the format can see writes its line here, in the commit that makes it. A line starts with its kind:
`api-add`, `api-break`, `behaviour-change`, `dependency`, `format-variant` or `text-change`; a fix is a
`behaviour-change`. `cargo xtask api-guard` reads the `format-variant` lines.

When a release branch is cut, `docs/release-notes/<X.Y.Z>.md` is written from these lines, with its summary
(`## In short`, at most 30 one-line bullets for a draughtsman: docs/release/README.md, section 3), and the lines go
from this file on both branches. From then on a fix to a candidate writes its line in `<X.Y.Z>.md`, and every other
change here.

- text-change: the help of a device's tag counter, frozen tag, parts and technical data, a strip's Label mode, a
  page's Scale, Grid and Number, a section's rows, a report block's Title, Form, Continuation, Sort, Group, Section and
  First folio and a party's address says what the program does with the value, with an example, in English and Dutch,
  where it repeated the row's label; a report block's Continuation said it was a form, and it is the numbering of the
  further pages (FT-2065)
- behaviour-change: a project's supply is set in its Properties and by `project.supply.set`, member by member: its
  phases, neutral, PE, earthing, nominal voltage, frequency, upstream rating and short-circuit current, previewed as
  one step; a new project, which has none, takes one with the first value set (1 phase + N + PE, 230 V, 50 Hz), and a
  value no supply has (four phases, 0 V) is refused in words (doc 06 §3.1, ADR-0008 amendment 6, FT-1928)
- behaviour-change: Arrange and route and Route keep a wire into an interruption point or supply mark on the mark's
  line: the layout's measure prices a trunk laid beside a mark's line, off it, at 32 steps, so a plan that shortens
  the wiring that way is no longer offered (doc 06a §1.2, C8, FT-1909)
- behaviour-change: Draw on a page from the station view sets a card of inputs beside the controller's box where its
  line has room, not under it, so the supply wires from the head no longer cross its first channels (doc 06a §7.5,
  FT-1894)
- behaviour-change: the text placer keeps a wire number, a tag or any other text it places off a strip's drawn bridge
  line, as it keeps off the frame (doc 06a §2.2, FT-2021)
- api-add: Fit the layout to the sheet (a layout page's context menu and Page ▸ Order, `layout.fit_sheet`)
  gives a layout page made before its legend became a page the whole sheet in one step: the fitting scale and the
  plate centred, no device moved (doc 12 §5.1, FT-2030)
- behaviour-change: a legend list's Properties show the layout page it follows, not the First folio and Section
  rows nothing reads, and `ew export report --block` takes the printed folio of a block's page (`902.1`) (doc 12
  §5.4, FT-2032)
- behaviour-change: `report_block.create {layout_page}` refuses a second legend list for a layout page that has one,
  in words ("layout 902 has its legend list already …"), as the window offers none: both printed 902.1 and on, and
  the DXF, SVG and PNG exports refused the two folios of one file name (doc 12 §5.4)
- behaviour-change: Properties no longer shows a record's schema version as a row labelled `$v` (under Identity on
  a device, a page and most other records) (doc 16 §6.1, FT-1939)
- behaviour-change: a layout's legend prints a length with its unit ("0.47 m" for a rail or a duct), in a box and as
  a page; a count of pieces prints bare, and the CSV keeps the number beside its Unit column (doc 12 §5.4, FT-1938)
- text-change: the guide's picture of a station's rack shows the station view as it is, To do under the cards and the
  channels' Function text column, and its text names that column (FT-2000)
- text-change: the help of the Project's rows (the pointer resting on a row's name, and F1) says what the program
  does with the value, its unit and an example, in English and Dutch: the title block's or the cover page's cell that
  prints it, or that the program keeps it as a note (Control voltage, Rated consumption, the supply's earthing and
  short-circuit current), where it repeated the row's label (FT-1809)
- behaviour-change: a three-wire PNP proximity sensor is among the starter parts (EW-GEN-PRX-1, 24 V DC), on its own
  symbol under the position sensors, its pins the cores BN (L+), BK (the output) and BU (M) (doc 07 §2.3, FT-1840)
- text-change: every starter part has a Dutch description and every starter symbol a Dutch name, so a Dutch
  project's parts list prints "Doorgangsklem" where it printed "Feed-through terminal"; the starter parts library is
  0.19.0, each part one patch version further (doc 07 §2.1, FT-1934)
- behaviour-change: a page's grid changed (`page.set_grid`) and a paste or a macro fitted to another grid move their
  symbols and junctions onto it together: where the nearest grid points would put two things on one another, or
  close a gap of a step or more that a wire could take, the later one moves on by whole steps, so no symbol ends on
  another, a row keeps its order and two pins more than a step apart keep a grid line between them for a wire (doc 06
  §8 item 3, FT-1817)
- behaviour-change: a drawn box goes no narrower than the pin texts of its left and right sides need, as it goes no
  lower than those of its top and bottom sides: a grip stops there (doc 06 §8 item 13, FT-1839)
- behaviour-change: a drag that starts on a pin's label of a symbol that is not selected moves the label alone, as a
  drag at its tag does; it moved the symbol (doc 06 §2.5, FT-1796)
- behaviour-change: the readout of a move (`dx 0.000 dy 4.000 mm`) says what the move does on the page; on a page
  drawn smaller onto its sheet it read a part of a step short (`dy 3.945`) (FT-1818)
- behaviour-change: a wire drawn to a drawn box's edge where no pin is keeps its ghost on the page while the new pin's
  name is asked, until it is given or the naming is cancelled (FT-1836)
- behaviour-change: a selected wire or fluid line marks its two ends with the connection point's ring (doc 16a §7.1,
  FT-1780)
- behaviour-change: a list never ends on a page of one or two rows: the last page of a run holds at least three, the
  page before ending that many rows earlier, so 24 rows on the 23-row form print 21 and 3; the page count stays the
  same (doc 09 §3.2, FT-1842)
- behaviour-change: the potential overview numbers its declared bonds first (B1, B2 … in the order they were made),
  then the drawn ones (an earth wire), so an earth wire drawn before a bond was declared no longer takes its number
  (doc 09 §2.4, FT-1827)
- behaviour-change: List columns… (`report_block.columns`) chooses a legend's printed columns too, with the legend
  selected on its panel page or from its context menu: a legend made before 0.3 shows its four columns ticked and the
  description among the others (doc 09 §3.8, FT-1772)
- behaviour-change: a legend a person adds keeps its positions, tags, quantities and part numbers whole where its box
  is narrow: its description gives way first, wrapping at its words while the box has the height for every row, else
  cut; it draws no drop shadow and a light grey header (doc 12 §5.4, FT-1782)
- behaviour-change: a wire that runs through a text it cannot step round (a fixed text, a tag of an imported page)
  is cut round the text, 0.25 mm clear of it, in the window, the PDF, the PNG, the SVG and the DXF alike; a page
  where no wire runs through a text is drawn as before (ADR-0034 amendment 3, FT-1786)
- behaviour-change: a strip's bridge is drawn on a schematic page where the terminals it joins stand in one row: a
  line across their bridged leads with a dot on each, no record and nothing to select (doc 08 §6.5, FT-1908)
- behaviour-change: an interruption point dropped on a drawn box's pin no longer lies along or in the box: on the
  lower side it stands a grid step below the pin and on the upper side two steps above it, joined by a straight wire;
  a source on the box's left side or a target on its right side, whose arrow would point into the box, is refused in
  words (doc 06 §3.5, FT-1798)
- behaviour-change: a Dutch project's lists print the program's own words in their cells in Dutch, as their headings:
  a channel's state and kind, a potential's type and class, "Structuur" and "Apparaat", a revision's state, a bridge
  and its heading ("Bruggen", "insteekbrug, klemmen 1–4"), a wire's source and junction ("ader", "ster bij"), "uit
  artikel", "lengte ontbreekt", "ja" and "nee", "n.t.b.", "st.", "Bestelcode n" and an open item's category, message
  and severity; the boxes on a drawn page (LEGENDA, ARTIKELGEGEVENS, PLC-OVERZICHT, the cover's tables) print their
  titles, headers and cells in the project's language; the CSV and XLSX stay English (doc 09 §3.8, FT-1955)
- behaviour-change: `ew route` names on stderr a file of the project that `manifest.json` does not list, and refuses a
  crashed session's unsaved journal and an older format as the other writers do (FT-1930)
- behaviour-change: the window's finish (FT-1761): a row's right-click menu opens under its row, not over it; a
  greyed entry of the canvas's menu, a command bar button's menu or a panel's More says why on a line under it; the
  check's toast offers Show messages; no tooltip stands over a dialog's backdrop; a report page's Properties say
  "generated" once in their header; Insert symbol opens on a row that can be placed; the Preview tab and the cards
  leave a count of none out, start an entry with a capital and say Cross-references for Xrefs; a medium's badge in
  the Fluid navigator counts its consumers as the medium table does; a medium's Colour stands under Identity; the
  parts import's Map columns card stands in the window's centre (doc 16a §6.8, §6.10, §6.13, §6.23)
- behaviour-change: the symbol editor's start card keeps its ways in where they stand when a draft is in hand, and
  Copy a library symbol… lists the symbols in the symbol browser's order with its picture of the one under the
  pointer (doc 07 §5, FT-1810)
- behaviour-change: the page has scroll bars while part of it is out of view, and the mouse wheel's first zoom
  offers once to make it scroll the page instead (View ▸ Mouse wheel scrolls) (doc 16a §5.5, FT-1830)
- behaviour-change: a table's columns fill its grid: a table without a column that takes the rest shares the room its
  columns leave among them, at 2560 × 1440 as at 1920 (the Bus, Devices and Medium tables) (doc 16a §6.21, FT-1761)
- behaviour-change: Select and Wire lead the Draw, PLC and Fluid tabs alike, so a tool keeps its place when the
  tab changes (Wire left Draw's Connect group, its menu path is Draw ▸ Wire); PLC workspace, Fluid workspace and
  Addresses are small buttons (doc 16a §5.2, FT-1806)
- behaviour-change: the part dialog finds a part by what it is for as well as by its words: its category and its
  main function, read in English and Dutch ("sensor" finds the position and pressure switches, "normally open
  contact" ranks the parts whose main function is one before the contactor), and it chooses no row until the person
  does, so Enter no longer takes a first row nobody looked at (doc 07 §4.3, FT-1845)
- api-add: Export DXF… on the Output tab and in the Output menu (`file.export_dxf {dir}`) writes the DXF set
  of `ew export dxf` into a folder as a job; a folder that holds other files is refused and left as it was (doc 09
  §4.9, FT-1960)
- behaviour-change: a text in many languages that lacks the project's language, or in a project that names none,
  prints in English before falling back to its first language in tag order (an element brought in from QElectroTech
  named in `ar`, `cs`, `de`, `en` … no longer prints, or names its DXF block, in Arabic) (doc 09 §4.7)
- behaviour-change: `ew export dxf` writes AutoCAD 2010 (AC1024, UTF-8) files that are the PDF's pages: a symbol is a
  block named in words (`Coil R0`) holding its own texts, a device's texts are the attributes of its insert (`TAG`,
  `FUNCTION_TEXT`, `CROSS_REFERENCE`, `DESIGNATION_1` …), a cross-reference links to its target's page file
  (`--dwg-links` names the `.dwg`), each text has the width factor of its kerned width in the PDF and a style that
  names its face's family, a line of two faces no longer stacks its parts at the line's start, and the title block's
  PNG or JPEG picture is an `IMAGE` of a file written beside the pages (an SVG one is drawn as paths); `--json` lists
  the pictures (doc 09 §4.9, FT-1877)
- behaviour-change: a dash pattern with a zero entry (as an SVG asset may give one) is dashed on the screen as it is
  in the PDF, the SVG and the DXF; one with a negative entry or no positive one is solid everywhere (the PDF and SVG
  rule)
- behaviour-change: a table import of a CSV whose header follows title lines takes the decimal comma of `excel-nl`
  when the header is separated by semicolons, as the reader reads its cells (the first line decided before)
- behaviour-change: a Terminal… row is one Undo step only until the next commit, a property, table or card edit as
  much as a drawing tool's; after an undone row and as many property edits as the row had terminals, one Undo took
  those edits back together
- behaviour-change: inside the symbol editor the command search and the window's panes ask the same check as the menus:
  a drawing tool that cannot run there is no longer offered as enabled, and Save, which saves the symbol, is no
  longer greyed
- behaviour-change: on a page of the QElectroTech grid a text's box keeps its size wherever its symbol stands, so a
  second press of Arrange and route finds the page the first stored; texts on such pages may stand a sixth of a
  millimetre from where they stood, and a wire beside such a text may take another route (06a §2.1, FT-1848)
- text-change: the example keys of `ew queries` and the guide's query reference name records of the new sample
  project (`-104A1-A2`, `-201K1-Q2`, `-X1011:L1`, `-102Q1` and two of its IDs), as the reference project's records
  are gone; every example answers on the sample
- text-change: the words that named the retired reference project say what is meant: the help of `ew import` and
  `ew export` speaks of the `ew` dialect (it said "golden dialect"), `ew sql schema` no longer cites a deleted design
  document, the argument texts of `ew queries` name the sample's records, and the guide's command-line how-to is "Use
  the command line on the sample project"
- text-change: the company-standard wizard's `report_blocks` value `golden` is `standard` (every list and overview,
  the default); a script that sets `golden` is refused and must set `standard`; there is no alias
- behaviour-change: File ▸ Open sample project and Help ▸ Tutorials open the new sample project, a smaller control
  cabinet the program draws itself from its own script; the release ships it as `samples/sample.ew` in place of
  `samples/J-E-2416.ew`
- behaviour-change: a print cancelled between two pages ends as cancelled; it could end with the printer's refusal
  of the next page's paper ("The handle is invalid"), because the cancel was only read before each band
- behaviour-change: a device takes a second symbol of its own: a symbol of another kind than a contact (a lamp, a
  supply, an element brought in from QElectroTech) joins any device but a terminal or a strip as an auxiliary
  function, by J while placing, by J or Join device… on a placed symbol, by `place.pick device=<tag>` and by
  `rep.join_device`; a contact keeps its rule (a device with a coil or a part naming it), and a terminal, an
  interruption point and a graphic join no device (FT-1860)
- behaviour-change: `place.pick` takes a library symbol's address `library:<library>:<item>` also after its first
  placement, and a symbol it does not find is refused as unknown, no longer as a missing parameter (FT-1860)
- behaviour-change: `ew import parts` and File ▸ Import ▸ Parts table… take a function template that names a function
  definition of the project's own by its key; it was refused as an unknown FnDef (FT-1860)
- behaviour-change: a `symbol-map.csv` beside a QElectroTech file binds its element definitions to starter coils,
  contacts, terminals and interruption points (columns `element`, `function`, `cps`, `tag`, `number`, `xref`): the
  element keeps its drawing, place and texts, its terminals become the function's connection points, its tag and a
  terminal's number text follow the project, so a device of the set can take a part; File ▸ Import…, Re-import and
  `ew import qet` read it, and a wrong row refuses the import in words (doc 14 §4.17, FT-1869)
- behaviour-change: the QElectroTech import makes the wired cabinet a person would draw: a T junction element joins
  its wires as a junction and an earth symbol is an earth point (both were terminals of inferred strips), terminals
  without a header on a second folio go on with the strip of the folio before, cable marks labelled with a W tag
  become a cable whose cores are the wires they cross, and a stored route with a corner between the grid's lines
  keeps its way (the corner moves to the half grid; QET's automatic path replaced it) (FT-1861)
- behaviour-change: the QElectroTech import writes QET's own title-block variables into the project's and the
  pages' fields and the title block prints those fields: `%plant` and `%locmach` become the page's structure, and the
  values every folio shares the open revision's label (`%indexrev`), the designer (`%author`) and the drawing number
  (`%filename`); a variable of one's own and a value the folios do not share stay text (FT-1859)
- behaviour-change: `library.embed` and `report_block.create` take a frame or title-block form of the project's own,
  and a report block's form is set with `fields.set` path `form` (its Form row in Properties) to a form the project
  holds (FT-1859)
- behaviour-change: a wire's frozen number prints: on the page, in the wire list, in `api_v1_wire.number` and in
  Properties' title and Number row, which takes a number typed as the page should print it (`6A03`; empty unfreezes)
  where it refused it as not strict JSON; the frozen wire keeps its place in the count (doc 08 §5.3, FT-1858)
- behaviour-change: Draw ▸ Connect ▸ Number wires by potential (`project.drawing.set {wire_numbering:
  per_potential}`) gives every wire of a potential its potential's name as its code (`D02`, `6A03`), none to a
  protective earth or a potential known only by its fallback name; EW-POT-015 counts potentials as the units then
  (doc 08 §5.2, FT-1858)
- behaviour-change: like parts that stand level take one place for their tags, cross-references and data where one
  is clear for all of them: a row of relays no longer has its tags at different places (doc 06a §5.3, FT-1870)
- behaviour-change: a text clicked with the Text tool on a wire's line stands beside the wire, 1 mm above a
  horizontal line on its baseline or 1 mm right of a vertical one, where it lay on the wire (doc 06 §4.5, FT-1870)
- behaviour-change: a QElectroTech element's own text that shows no slot of its symbol moves with the element and
  goes with it (a text of its symbol), where a move or Arrange and route left it behind (doc 14 §4.4.2, FT-1875)
- api-add: a project prints in a typeface of its own: Project ▸ Text styles… ▸ Choose a font file…
  (`text_styles.typeface`; the command `project.typeface.set {file, name}`, the file in base64 parts of at most
  1 MiB each) takes a TrueType or OpenType file of at most 2.5 MiB, Arial and Calibri among them, into the project in
  one step, and every text of the Standard typeface prints in it in the PDF, the SVG, the PNG,
  the print and on screen; the DXF names the file in its text style; **Use Noto Sans** takes it back. A font whose
  licence forbids embedding is refused (doc 09 §4.8, FT-1872)
- api-add: a free text can stand on its baseline (`anchor: baseline`, `item.place`'s `anchor`), and a symbol's text
  has the alignments `baseline_left`, `baseline_centre` and `baseline_right` (the symbol editor's *On the baseline*),
  so a text keeps its place in any typeface; a project without them prints as before
- behaviour-change: the QElectroTech import stands a page's free texts and an element's texts on QET's own baseline,
  so they lie where QET's own PDF has them in any typeface (an element's static text stood a descent lower in a face
  other than Noto Sans)
- behaviour-change: a QElectroTech title block's cell text stands on the cell's edges, as QET draws it, not 0.5 mm in
  from them (the cell's `ew.cell.inset`, FT-1765)
- behaviour-change: the DXF export writes a text as high as the capitals of its own face: a Technical drawing text is
  0.719 of its em high (it was 0.714, Noto Sans's); the other bundled faces are unchanged
- behaviour-change: a project made in a folder where a project of the same ID was (an import of the same source again,
  Save As onto its path) is no longer offered that project's leftover crash journal: making it deletes the journal
- behaviour-change: a file added to a project by hand that `manifest.json` does not list is named on stderr by the
  `ew` commands that open the project (`warning: <file>: manifest.json does not list it, so it is not read …`), and a
  records file of a library folder that `library.json` does not list is a note of the library in `ew lib list`
- behaviour-change: `ew fmt` rewrites a member written with the value the canonical form leaves out (a symbol's hidden
  text slot with `"visible": false`, a `null`, `[]` or `{}`) instead of refusing the file as a change of content
- behaviour-change: `cable.from_wires` takes `cores` as a list of core names (`["BU", "BN"]`) as it takes a text; a
  list was ignored and the wires got no cores of their own
- behaviour-change: `page.delete` with `delete_devices` deletes a strip whose terminals all go with the pages, instead
  of leaving it empty
- behaviour-change: `strip.new` with the tag of a strip the project has is refused, naming the strip and Draw ▸
  Terminal…, instead of making a second strip of that tag
- behaviour-change: Enclosure… offers every part of the project and the libraries with an enclosure section, whatever
  its kind (a parts import gives `device` when its row names no kind)
- behaviour-change: `place.pick` names an installed library's symbol by its key before its first placement too
- behaviour-change: a part whose function is a black box (the starter's power supply) is placed by the Place tool with
  a symbol of its own drawn for that function; the Place tool refused it as a black box and the Black box tool as a
  symbol
- api-add: the headless action `ui.actions` prints every action with its label, shortcut, parameters and whether it
  runs now (the reason when not) into the transcript's `describe`, as `ui.describe` prints the window
- behaviour-change: `report_block.new` takes a report's kind by its name too (`Tube list`), as its refusal lists them
- behaviour-change: after Project properties (`props.project`) the page's own rows show again on another page, on a
  click on the empty page and on `select.set {"ids": []}`
- behaviour-change: `ew import qet --out a/b.ew` makes the folder `a` when it does not exist; the import ran and then
  wrote nothing
- dependency: `ew-commands` reads the font file `project.typeface.set` takes with skrifa 0.44.0, the version already
  in the graph
- api-add: a list prints the columns its maker chooses, in an order of their own, each with its width and a heading of
  its own, a caption above its table, a legend line under it, and rows the model does not hold, which stay however
  often the list is made again: Output ▸ List columns… and Rows of your own… on a page of the list, or `fields.set`
  on its report block (paths `columns`, `caption`, `legend`, `added_rows`; additive members) (doc 09 §3.8, FT-1874)
- behaviour-change: a list page's table stands in its frame's drawing area and holds as many rows as the frame has
  room for, 36 on A3 (on the starter A4 frame it stands where it did), and a list page prints on the set's paper: when
  the project's frame has another size than the list's, the list prints on the project's frame (FT-1826)
- behaviour-change: a list page prints in the project's language: its column headings (in English and Dutch), its
  terminal states, its decimal sign and the title block's labels (the starter A4 title block has Dutch labels, version
  1.1.0); a Dutch project printed them in English (FT-1874)
- api-add: the starter forms hold an A3 landscape frame with the grid and its title block, which a new project holds
  and the template wizard's Frame offers (FT-1826)
- api-add: a free text may belong to a symbol (`of`, a representation on its page; `item.place`'s
  `free_text.of`): it moves with the symbol and goes with it, and with `bind` `device.function_text` or `strip.tag`
  prints the device's function text or the strip's tag, the strip then named on that page by the text alone; Function
  text over its path and Name the terminal strip here in the canvas menu over one symbol place one (doc 06 §3.8,
  FT-1875)
- api-add: a two-level terminal block with a number on each level is one terminal with one part: Join device… (J) on a
  terminal's symbol offers the other terminals of its strip, and `rep.join_device` makes the terminal the chosen one's
  next level, its symbols, wires and number kept; a level's number is the optional member `designation` of
  `terminal.levels[]`, its symbol prints it, a wire end reads `-X1:2/a`, the terminal diagram has a row per level and
  EW-TERM-002 counts it (doc 08 §6.3, FT-1878)
- api-add: a part whose type is not known yet: Type not known yet… in the part dialog, or `part.placeholder {devices,
  description}`, gives the devices a placeholder part numbered TBD with those words, which the parts list prints as a
  row of its own; a real part chosen for one of them replaces it on all; EW-CORE-043 reports a placeholder as info
  instead of a warning (still an error in release mode) and EW-CORE-042 asks it for no manufacturer (doc 07 §3.3,
  FT-1878)
- behaviour-change: `ew print --fit` and the print dialog's Scale to fit enlarge a page to the paper too, an A4 set on
  A3 at 141 %, and stamp it "scaled n %" (FT-1826)
- behaviour-change: the terminal diagram's first column is the strip, so two strips' terminals 1 read apart; its CSV
  and XLSX have the column too (FT-1805)
- behaviour-change: Report block… makes a new list on the set's own frame and title block: the frame defaults' frame,
  else the frame most pages print on (a set brought in from QElectroTech); a set on the starter forms keeps the
  starter frame (doc 09 §3.8)
- behaviour-change: a page's and a report block's Form row in Properties chooses among the project's forms by name, in
  the window's language; a frame of the project's own reads by its paper and the pages it is on ("Frame A4 landscape,
  on 101, 102, 103 …", "Kader A4 liggend, op 101, 102, 103 …")
- behaviour-change: a list's title the program writes (Report block… without a title, the template wizard's standard
  layout) is in English and Dutch and prints in the project's language, in the title block, the contents, the page
  tab and the PDF outline; a title a person types stays as typed
- text-change: Properties names a report block's printed column by its heading in the window's language ("Column:
  Strip"), never by its key
- api-add: Page ▸ Copy pages… (`pages.copy {pages, offset | after, node, code, rename}`, also on a page row's
  right-click menu; the command `page.copy`) copies pages for a second installation in one undo step: a device comes
  whole (a coil with its contacts), a strip with its terminals and bridges, a cable with its conductors and cores, and
  bonds and page links come along; the copies take new folios (an offset, or after a page), the tags of their new
  pages, a structure node of their own and renamed frozen tags and interruption-point names, and a copy whose
  interruption points would join the points they were copied from is refused in words (doc 06 §3.6, FT-1862); the copy
  reproduces what the project holds (two terminals of one designation, a symbol off the grid) and the check reports it
  in the copy as in the source; an incoming supply drawn as targets only continues; a frozen tag stays frozen where
  the copy stands in a structure node of its own; a free text that is a copied device's tag or a renamed point's name
  prints the copy's
- behaviour-change: a page macro's symbols are inserted where they stood, a symbol off the grid too; it was refused as
  off the grid though nothing moves it
- api-add: Edit ▸ Paste in place (`edit.paste_in_place`) puts the clipboard's block down where it was copied, on any
  page of the same frame (doc 06 §3.6, FT-1862)
- behaviour-change: Paste no longer refuses a block whose free text or graphic lies in the frame's bands, where the
  Text tool puts them; a copied fragment names those in its header (`free`, a member an older version ignores)
- behaviour-change: Save as macro… with `pages` (folios) saves a page macro with nothing selected and no page marked;
  it said "Nothing is selected"
- text-change: Properties titles a wire without a number "Wire without a number" ("Draad zonder nummer"), not "?"
  (FT-1799)
- behaviour-change: a PLC card's or a bus device's device box is edited in the box: each pin is put on its edge and
  moved along it and the box is sized, saved as the part's layout and drawn so by the Device tool, Place here and
  Draw on a page…; the pins' names and what they carry stay the part's, changed in the part editor (doc 07 §2.5,
  FT-1788)
- behaviour-change: on a bus page the boxes of a device with several interfaces move together when one is dragged
  or moved by the keyboard; a row of such boxes drawn near the page's edge stands inside the drawing area, and one
  that would cover another device is refused in words (doc 10 §2.7, FT-1766)
- behaviour-change: a finding of EW-PLC-012 (a binding or an address that differs from the part's map) offers Open
  the part's PLC section… in its row menu, which opens the module's part in the part editor (doc 10 §3.10, FT-1766)
- behaviour-change: a station placed first on a bus page whose box is then drawn on a schematic page takes that
  page's tag, and the toast says so with both tags and why (doc 10 §2.7, FT-1766)
- api-add: Remove from my parts library… in the part editor of a part of My parts (`part.remove_own {confirm}`) asks
  first, then takes the part out of My parts; the next projects no longer offer it, and a project that uses it keeps
  its copy, which the toast says; a script gives `"confirm": true` (doc 10 §2.6, FT-1766)
- behaviour-change: a drawn box follows its corrected part at once: a socket or pin the part adds stands on the box
  after the last one of its edge, the box growing where it needs to, and one the part no longer has leaves the box
  unless a bus line ends there (doc 10 §2.6, FT-1766)
- behaviour-change: a project-local part corrected in the part editor keeps each wire at its pin when its pins are
  numbered again (a card given more channels: the wire at L+ ended at an input); a correction that takes away a pin
  something is connected to is refused in words (doc 10 §2.6, FT-1766)
- behaviour-change: the Wire tool keeps the straight line between two ends in line where it crosses nothing: a rail
  drawn one step over a row of coils stays as drawn (it went round their pins, and the taps then clicked onto the line
  found no wire); where something stands on the line the wire goes round and the tool's hint says so (doc 06a §7.4,
  FT-1864)
- behaviour-change: a connection point drawn in the symbol editor brings the text that writes its number, inside the
  box at the point and turned by its edge as a device box prints a pin's name, so a point on the lower edge is named
  above it, not across it, and the drawing shows what the page prints (doc 07 §5, FT-1871)
- api-add: a part holds a card of two channel kinds, inputs and outputs (`PlcSection.more_channels`, an optional
  member after `ratings`): the part editor's PLC form has a second channel kind and its count, the outputs numbered on
  after the inputs and each kind addressed from bit 0 of its own range (doc 10 §2.6, FT-1766)
- behaviour-change: a wire from a 0 V to an earth symbol is the bond also when the 0 V comes in through interruption
  points from another page, and when only its name says 0 V (`0VDC-6`, an imported supply whose pins say nothing):
  the 0 V stays 0 V and is no longer typed PE as a whole, and Draw ▸ Connect ▸ Bond is not needed (doc 08 §3.7,
  FT-1821, FT-1867); the supply marks' panel proposes such a mark as 0 V (M), where it proposed a plus of 0 V
- api-add: technical data takes a current in milliampere (`mA`), printed "20 mA" on the page and in the lists, typed
  in Properties and read from a parts catalogue; a value typed without a unit is refused with the units listed from
  the one list of units (FT-1867)
- behaviour-change: an earth symbol without a part has no tag unless a frozen one names it: it takes no number of the
  cables' letter W (a page's first cable is W1 again), prints none, is named by its pin (`PE`) in the wire list, the
  messages and Properties, and no check asks for its letter, an imported earth point (letter A) included; its Tag row
  says why it is empty (doc 03 §8.2, FT-1820)
- behaviour-change: an external point's reference holds what is known of the far end: the device or an address alone
  ("Q4.1"), a location alone, or nothing for a far end not known; the point prints what it holds under its mark, and
  Supply comes in here… takes an empty field as not known (`ip.supply_in`, `external_ref.create`: `location` and
  `device` optional); EW-CORE-034 still asks for the cable and names the place by what is known (doc 06 §3.5,
  FT-1876)
- behaviour-change: paste, fill down, pattern fill and clear in the PLC grid's Function text column set the function
  text of the device wired to each channel, as typing does, one command per gesture; they said "no command yet", and
  a channel nothing is wired to is refused in words (doc 10 §6, FT-1846)
- behaviour-change: a valve terminal's first overview page (Make the overview page in its station view, Generate
  overview…, `valve_terminal.generate_overview`) follows the terminal's first fluid page in that page's section; it
  followed the terminal's own page, which could be a schematic or bus page of the wiring section (doc 10 §6, doc 11
  §5.5, FT-1823)
- behaviour-change: in a station's view the channel cell last clicked or typed in keeps the keyboard cursor, ringed:
  F2 opens it and the arrows walk the channel cells, until a place chosen in the rack takes the keys back; F2 changed
  the card's tag there (doc 10 §6, FT-1838)
- behaviour-change: a station's view shows its To do first, right under the rack, where a toast never covers its
  buttons; it stood at the foot of the lists, where the toasts stand (doc 16a §6.10, doc 10 §6, FT-1824)
- behaviour-change: a new layout view (Enclosure…, a page's Surface row) stands its enclosure in the middle of the
  frame's drawing area; on the starter frames it stood about 8 mm up and left of it (FT-1768)
- behaviour-change: Auto-fill rail… lists the cabinet's unplaced devices and the devices without a location, each with
  a tick (a first guess: the cabinet's devices and parts that go on a rail ticked, sensors, cylinders, motors and
  valve terminals without a location not), and places only the ticked ones, a ticked device without a location
  taking the cabinet's in the same undo step; `layout.auto_fill_rail` takes `devices` (IDs or tags) or the guess. Its
  button that gave every device without a location the cabinet's and placed them all, field devices too, is gone
  (doc 12 §3.3, FT-1802)
- behaviour-change: Enclosure… gives the enclosure and its mounting plate the numbering scheme's tags, `-UC1` and
  `-UC1-U1`, not frozen ones, so the check no longer reports them as deviating (EW-NUM-001); a project's existing
  frozen tags stay (doc 03 §8.2, doc 12 §3.1, FT-1899)
- behaviour-change: Route lays three potentials or more over one row of poles on trunks side by side; a trunk, or a
  route, more than two steps beyond everything else on the page was not held, so the next trunk took its line and
  its wires were left without a route (doc 06a §3.3, FT-1931)
- behaviour-change: a second press of Arrange and route, or of Route, offers nothing: a plan is judged with each cable
  mark and wire marker where its new routes will draw it, and Route offers no plan whose marks, set on their new
  routes, put a text on another; a plan kept a cable's core name where the old route crossed the cable's line, and a
  second press routed a wire again (doc 06a §3.10, FT-1880)
- behaviour-change: a wire between two pins that stand nearly in line, one of them off the grid (after a grid change),
  steps across square at the grid line nearest its middle; Route and the Wire tool drew it as one slanted segment
  (doc 06a §3.6, FT-2010)
- behaviour-change: a stored route that misses an end it names (written outside the program, drawn with a hook to its
  pin) is broken to Route: a page-wide Route offers to draw it to its end, ticked in the preview as a wire you drew
  (doc 06a §3.2, FT-1975)
- behaviour-change: a panel layout's legend is a list page of its own right after its layout page (902.1, 902.2 …
  after 902): Enclosure… makes it with the plate, which takes the whole sheet; it runs on over as many pages as it
  needs on the set's own frame, prints the project's language, stands in the contents and the PDF outline and exports
  as CSV and XLSX; Add legend keeps a legend beside the layout only where every row and text fits there whole, else
  makes the page and says so; Move the legend to its own page turns a legend box of a project made before into the
  page in one step of Undo, and such a box prints as before until then (doc 12 §5.4, FT-1920)
- behaviour-change: a new layout labels each device with its tag where the tag fits inside it and with its numbered
  balloon where it does not (Labels: Tags or numbers), whatever the scale; tags, item numbers and both stay choices
  (doc 12 §5.3)
- behaviour-change: on a layout with a legend page, Follow reference over a device, its tag or its balloon (or with it
  selected) goes to its row, and a click on the row's tag goes back to the device, in the exported PDF too (doc 12
  §5.4)
- api-add: `ReportBlock.layout_page`, an optional member that names the layout page a legend list follows, and
  `report_block.create {layout_page}`; `page.surface_view.set {labels: tags_or_numbers}` and `layout.labels {show:
  tags_or_numbers}`; the action `report_block.legend_to_page`; the PDF destinations `ew:pl:<placement>` and
  `ew:lr:<placement>` (doc 09 §2.2, §3.2, §4.2)
- text-change: Project properties name their rows as the title block's cells name them: Project (was Project
  reference), Drawing (Drawing ID), Object (Object ID), Saved (Edited) and Author (Designer; in Dutch Getekend); the
  rows' explanations say which cell prints them, that Document title prints on the cover only, and that the Checked
  cell prints who approves the current revision, not the Checker row (doc 09 §2.7, FT-1803)
- behaviour-change: the Potential row of a wire whose potential no definition names opens Define potential… on Enter,
  F2 or a click on its value, as the wire's menu does; only its "…" menu offered it (doc 16 §6, FT-1779)
- behaviour-change: a part without a symbol of its own, placed with the Place tool, says in its hint that it is drawn
  as a box ("This part has no symbol of its own and is drawn as a box: click its first corner…"); the Black box tool
  armed without a word (doc 06 §3.3, FT-1808)
- behaviour-change: a pin's place typed in the device box between two steps of the 4 mm pin grid, or on a corner,
  says where the pin stands ("The pin stands at 4 mm: a pin stands on a step of 4 mm…"); it went to the step without
  a word. The line over a copy of a starter part says that the copy keeps the starter's part number until it is given
  one of its own (doc 07 §2.5, FT-1811)
- behaviour-change: F1 over the symbol editor's outline with a connection point or a shape selected opens the guide's
  section of that thing; it opened the outline's (doc 16 §13, FT-1812)
- behaviour-change: the Place tool's command bar names a terminal for a strip without a tag yet in words ("terminal 1
  on the new strip (no tag yet)") until the preview predicts its tag, not as "-?X?:1"; the project's Kind row reads
  Schematic for a project whose kind is not written, not a dash (doc 16a §5.5, FT-1813)
- behaviour-change: a legend's Rectangle row in Properties reads its place and size in millimetres
  (`20.000, 15.000 mm · 60.000 × 40.000 mm`) and is typed back that way; it showed the stored nanometres as JSON
  (doc 16 §6, FT-1936)
- behaviour-change: Draw on a page from the station view puts a station's CPU, and any box that is no card, inside the
  drawing area where under what the page draws there is no room: the first clear place from the area's head; it stood
  below the frame, off the sheet (doc 10 §2.3, FT-1907)
- behaviour-change: the Potential panel of a supply mark names its Conductor list and Voltage field for a screen
  reader by column and mark ("Conductor, 24V"); on some marks they had no name (FT-1910)
- behaviour-change: a click on a selected mark's own cross-reference text follows it, as with nothing selected; it
  only selected the mark again (doc 06 §2.5, FT-1911)
- behaviour-change: a wire selected on the page marks every row of a navigator that lists it, as Connections lists a
  wire under its potential and again under a pin or in two wires' rows; only the last of them was marked, so a wire
  such as the sample's 101.16 on page 104 left its own row unmarked (doc 16 §5, FT-1912)
- text-change: in Dutch a function's kind whose generic part has no Dutch words reads as written after its pole count,
  "3P manualswitch" and "3P mcb", not "3P- manualswitch" (FT-1913)
- behaviour-change: a page's Form row in Properties names the frame the page draws with when it names none of its
  own ("Frame QET A4 landscape grid" on the sample's 902); it was empty (doc 09 §3.8, FT-1915)
- text-change: Properties explains the project's Source, Translations and Frames rows and a fluid port's closure Kind
  and Part, in English and Dutch; the Frames row names its frames ("Default: Frame QET A4 landscape grid; Table of
  contents: …"), not their IDs (doc 09 §3.8, FT-1916)
- behaviour-change: a bus line drawn between two ports gives them a physical net as well as the logical one (the one
  they name, else a new one of copper named for the bus system), so its segments, EW-BUS-004 and the bus overview see
  it without `bus.physical_net.create`; EW-BUS-004 checks the termination of PROFIBUS DP and CANopen only, so a drawn
  PROFINET star reports no unterminated ends (doc 10 §2.7, FT-1922)
- behaviour-change: a Messages row of a finding on a cable (EW-CORE-033, a cable without its length) names the page of
  the cable's cores and Enter opens it with the core wires selected; it named no page and opened nothing (doc 16 §5,
  FT-1924)
- text-change: a form table that adds rows names them for a screen reader by its own row word and number: Copy
  pages…'s rename table "New text, text to change 2", "Remove text to change 2", not "value set"; Save as macro…'s
  remove button reads "Remove value set 2", by the row's number as its cells are, not by the set's name (doc 06 §3.6,
  FT-1965)
- behaviour-change: in the symbol editor a connection point moved alone takes the text that writes its number with
  it, and a point given another edge in Properties has that text placed again inside the body for the new edge; the
  text stayed where it stood (doc 07 §5, FT-1990)
- behaviour-change: the device box's pin list keeps its rows while the same pins are edited, so a pin given another
  edge stays where it was in the list until another selection sorts the list by the box again; it jumped to the new
  edge's rows (doc 07 §2.5, FT-2005)
- behaviour-change: the part chooser shows a library part's description in one language, the window's, else English,
  else its first ("Installatieautomaat, 1-polig" in a Dutch window), and still finds it by its words in every
  language; it joined every language with " / ". The device browser and Properties' list of library parts show it in
  the project's language by the same order (doc 07 §4.3, FT-2066)
- behaviour-change: the spacing of a terminal row (Terminal…'s Spacing, Spacing… on placed terminals) is kept between
  sessions as a user setting, `terminal_spacing` in the settings file; it lasted while the project was open (FT-1791)
- format-variant: form Align baseline_centre
- format-variant: form Align baseline_left
- format-variant: form Align baseline_right
- format-variant: representation Align baseline_centre
- format-variant: representation Align baseline_left
- format-variant: representation Align baseline_right
- format-variant: symbol Align baseline_centre
- format-variant: symbol Align baseline_left
- format-variant: symbol Align baseline_right
- format-variant: device Unit mA
- format-variant: part Unit mA
- format-variant: project Unit mA
- format-variant: page_item LayoutLabels tags_or_numbers
- behaviour-change: a project made without its template's pages takes its first page's ID from the session's ID source,
  so two runs of one script make the same project; it took it from the clock (FT-1900)
- behaviour-change: `supply.place` and `ip.supply_in` take a location with or without its sign: `+UP` is the node `UP`;
  it made a node `+UP` that printed `++UP` (doc 06 §3.5, FT-1901)
- behaviour-change: `bus.cable` takes its cable part and its plugs by part number, in any case, as every part parameter
  does; a plug was found only by its record ID and the cable part only with its description (doc 10 §2.7, FT-1902)
- behaviour-change: EW-POT-013 accepts a breaker whose phase pole and neutral pole are two functions of one device (1P+N
  wired with L and N): the poles of one device switch together; it was reported as opening the neutral without the phase
  (doc 08 §8.2, FT-1903)
- behaviour-change: two sessions or branches that each set another override of one wire whose record held none (the
  cross-section here, the colour there) merge both; the cross-section was shown as changed on both sides and the later
  edit refused (doc 05 §11.2, FT-1917)
- behaviour-change: a DXF text read in keeps its own height as written, through the page's page-to-sheet map, beside the
  nearest built-in style: it prints at its size and on its baseline; a text of a height no built-in style has (a
  terminal's) stood up to 0.05 mm off (doc 14 §6.4, FT-1921)
- behaviour-change: a report table of many columns cuts no text that fits at the smallest size: a column whose text
  would be cut takes what it lacks from the room the other columns spare at that size, so an I/O list on the PLC
  overview's form prints the CPU's tag and every part number whole (doc 09 §7 item 5, FT-1923)
- behaviour-change: verifying a project against its PDF reads a kerned pair of a letter and a comma or an ellipsis
  (`r,`, `r…`) as one text: the sample's parts lists' descriptions were two missing texts and one extra (doc 14 §4.15,
  FT-1925)
- behaviour-change: Generate overview names the new pages of an overview that runs over several pages as siblings,
  104.1, 104.2, 104.3; they were 104.1, 104.1.1, 104.1.1.1 (doc 11 §5.5, FT-1926)
- behaviour-change: the valve terminal configuration and the view `api_v1_valve_position` of a terminal that is its own
  valve module (the starter EW-GEN-VTM-1) list a row per position with its valve, part, solenoids and address; its
  valves were listed as modules with those cells empty (doc 11 §8.2, FT-1927)
- behaviour-change: a protective earth that only an earth symbol names (an imported earth point, whose pin is `1`) is
  called PE in the potential list, on the page and in the SQL views; it was called `1` (doc 08 §3.5, FT-1933)
- behaviour-change: a parts catalogue's technical data takes every unit of the format, by its spelling or its symbol:
  `8.5 mm`, `10 pc` and `2 set` were refused as no unit of the format (doc 03 §3.2, FT-1981)
- behaviour-change: New part… makes a card of two channel kinds that go the same way (inputs with analogue inputs,
  outputs with analogue outputs) as one part: the second kind's channels follow the first's bits in the direction's one
  range from the next word; only the first kind again is refused (doc 10 §2.6, FT-1970)
- behaviour-change: a wire on a single phase L or a neutral N whose voltage is not known takes the mains class's
  cross-section (1.5 mm² built in), as a phase L1, L2 or L3 does; it took the control size, 0.75 mm², so the sample's
  N wires were thinner than their phases (doc 08 §4.2, FT-1834)
- text-change: a list page or a box whose rows do not derive prints its reason in words in the project's language ("Not
  derived: this build does not make this list", "Niet afgeleid: …"); it printed it in English with a family or kind name
  in backquotes (doc 09 §2.4, FT-1995)
- behaviour-change: a page's grid changed (`page.set_grid`), a paste or a macro fitted to another grid keep a row of
  three or more like symbols that stood at one pitch at one pitch of whole steps; each went to its own nearest grid
  point, so some stood a step closer than their neighbours (doc 06 §8 item 3, FT-1935)
