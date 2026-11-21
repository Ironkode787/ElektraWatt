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

- behaviour-change: Export DXF… on the Output tab and in the Output menu (`file.export_dxf {dir}`) writes the DXF set
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
