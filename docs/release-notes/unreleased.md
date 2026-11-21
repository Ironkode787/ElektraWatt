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
- behaviour-change: `ew print --fit` and the print dialog's Scale to fit enlarge a page to the paper too, an A4 set on
  A3 at 141 %, and stamp it "scaled n %" (FT-1826)
- behaviour-change: the terminal diagram's first column is the strip, so two strips' terminals 1 read apart; its CSV
  and XLSX have the column too (FT-1805)
- format-variant: form Align baseline_centre
- format-variant: form Align baseline_left
- format-variant: form Align baseline_right
- format-variant: representation Align baseline_centre
- format-variant: representation Align baseline_left
- format-variant: representation Align baseline_right
- format-variant: symbol Align baseline_centre
- format-variant: symbol Align baseline_left
- format-variant: symbol Align baseline_right
