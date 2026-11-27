<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# Unreleased changes

The changes since the last release, 0.3.5, one line each, and nowhere else: a change that a user, a script or a
reader of the format can see writes its line here, in the commit that makes it. A line starts with its kind:
`api-add`, `api-break`, `behaviour-change`, `dependency`, `format-variant` or `text-change`; a fix is a
`behaviour-change`. `cargo xtask api-guard` reads the `format-variant` lines.

When a release branch is cut, `docs/release-notes/<X.Y.Z>.md` is written from these lines, with its summary
(`## In short`, at most 30 one-line bullets for a draughtsman: docs/release/README.md, section 3), and the lines go
from this file on both branches. From then on a fix to a candidate writes its line in `<X.Y.Z>.md`, and every other
change here.

- behaviour-change: Label file columns… keeps what a newer build wrote into the export profile: a delimiter it does not
  know, left as it is, stays (the edit was refused), and so do the profile's unknown members and those of a column it
  keeps (they were dropped) (doc 13 §5.4)
- behaviour-change: Labels for these changes… (and `ew export labels --since`) writes a new strip's marker, the strip
  whose every terminal the rework list adds; it left it out (doc 13 §5.4)
- behaviour-change: a terminal of several levels gets a terminal marker per level, each with the level's designation,
  in the labels list, the label files and the rework list; it got one (doc 13 §5.3)
- behaviour-change: a row of the labels list and of the tube labels is what it labels (the wire or tube with its
  end, the terminal, strip, cable or device) with that record as its source, so a block's left-out records stay out
  and a row opens its record again, and the For cell names a tube in the list's language; an existing tube-label
  block prints its tubes as before (doc 13 §5.3)
- behaviour-change: a wire's own values (its typed length, colour, size, flags, annotation and each end set by hand)
  follow the wire through every change of the wiring: a wire drawn onto it, with a bevel as with a dot, leaves it all
  of them and takes none; a junction whose wiring changes, a T that dissolves, a device moved to another page with its
  wires kept and a device placed on a star's feed arm leave each wire its values and each end at its clamp, held once;
  a wire whose clamps change keeps what divides but no typed length. A value typed for a wire is read before one on a
  stretch it shares with another wire, where it was hidden behind it (doc 13 §2.3)
- behaviour-change: of two interruption points one grid step apart (24 V and 0 V at a supply's outputs), the lower
  one's name steps beyond its arrow over or under the wire's line, and a point's cross-references stand under and over
  its name where the name stands, so the texts of neighbouring points no longer print over each other (doc 06a §5.2,
  FT-2207)
- behaviour-change: the Supply tool's marks join a pin as the IP tool's point does: the pointer near a pin puts the
  first mark on it, a mark whose connection point lies on a pin joins it in the same undo step, the bar under the page
  names each pin ("L connects to -Q1:1") and Alt places the marks free; a click that would set a mark on another
  interruption point, or with its arrow into a drawn box, is refused in the IP tool's words (doc 06 §3.5, FT-2213)
- api-add: the transcript's canvas ghost of the Supply tool is `{"kind": "supply", "at_nm", "joins"}`, `joins` the
  pin each mark joins by the mark's name (FT-2213)
- behaviour-change: a text placed by hand on a symbol keeps its place on the page and its own alignment through a
  turn, a mirror or a variant change (`rep.transform`), and a turn and its reverse give its position back; it turned
  with the symbol about its insertion point (doc 06 §3.4, FT-2209)
- behaviour-change: the Wire tool clicked where no wire can start (and the Tube tool on a fluid page) says so in the
  bar under the page: "A wire starts at a connection point, a junction or a wire: click one of them to start it"; it
  did nothing (doc 06a §7.4, FT-2209)
- behaviour-change: a list on a frame that draws its column and row strips (the starter A4 and A3 grid frames, an
  imported QET border) stands 1.30 mm below the column strip, as far as it stands right of the row strip, where its
  header row lay on the strip's line; the frame without strips prints its lists as before (doc 09 §3.8, FT-2210)
- behaviour-change: a page that a command makes (New page, a station's page per card, Create enclosure's layout page,
  Move to page's "To a new page", a card moved to a new page) goes to the section for its kind, a title page and a
  table of contents to the front matter; where no section is for the kind the command makes one, as New section
  proposes it and titled with the kind, in the same undo step, and says so; panel layout pages go to Nomenclature.
  They went after the page on show, in its section, which put a new project's pages in the front matter (doc 06
  §2.3, FT-2203)
- api-add: `page.create` and `rep.move_to_new_page` take `new_section`, a section made in the same step as the page
  (as `section.create` takes it), which holds the page's `number`; the outcome's notice names it (doc 06 §2.3,
  FT-2203)
- behaviour-change: `rep.move_to_new_page` puts a page of an explicit `number` in the section whose folio range holds
  it (it took the moved-from page's section), and a page without `title` takes the moved-from page's title (it had
  none) (FT-2203, FT-2202)
- behaviour-change: Copy pages at an offset onto folios no section holds makes the section for the copies over the
  hundred the new folio lies in, numbered after the highest and titled as the source's section, and its preview says
  so; it refused (doc 06 §2.3, FT-2203)
- behaviour-change: Move to page… shows its preview for every move, and its question's foot line says so ("Enter
  shows the preview: nothing changes before Apply"); the preview leads with what moves where ("Moves 4 symbols to the
  new page 5") and says what its count of items is made of, and a preview's cross-reference row names the page of a
  point or a page the change makes; Apply moves and shows the page (doc 06 §3.4, doc 16 §4.3, FT-2202)
- behaviour-change: Undo and Redo show a page the step changed: they stay where the page on show is one the step
  changed, go back to the page, view and selection the step was made with where that page is one it changed, and
  else show the first page it changed; a change made from another page (a quick fix, Properties) went back to the
  page it was made from. A step on a panel layout page (its plate, rails, ducts, placements) names that page in
  the history (doc 16 §8.1, FT-2204)
- text-change: the project check is "Check project" wherever it is named, its button, its menu item and the command
  search (it was "Run checks" outside the button); the search also finds every command by the words on its button
  (FT-2205)
- behaviour-change: Paste in place stands beside Paste in the canvas's right-click menus, over an object and over an
  empty spot (doc 16a §6.13, FT-2205)
- text-change: a switch in the command search says its state and what Enter does ("Off, Enter switches it on"), where
  it said "On" or "Off" alone (FT-2205)
- text-change: the IP tool's words: a source is where "the potential leaves here for its targets", a target where "the
  potential arrives here from its source", with D's hint to match; a point dropped on a pin is named by its pin ("Placed
  24V on -102F2:14"), and the sentence on its missing partner is short, so the toast stands whole at 1920 × 1080
  (FT-2205)
- behaviour-change: a wire's Cross-section row says at the size why no protective device's rating sizes it: chip "no
  supply" ("no supply found upstream, so no protective device sizes it", or the breaker it ends on that no supply
  reaches) or chip "no protection" ("no protective device upstream sizes it"), and a size set by hand behind a breaker
  no supply reaches says the same beside "override"; a click on a derived row with an override types its own value, as
  Enter does (doc 08 §4.2, FT-2211)
- text-change: Draw ▸ Connect ▸ Print wire numbers, its checkbox in the project's Properties and the step Undo names are now Show wire numbers on the pages (Dutch: Draadnummers op de pagina's tonen); its hint reads "Switch on Show wire numbers on the pages to see them."
- text-change: EW-CORE-051 says which end is missing as the interruption point tool does: "place its source where the potential leaves" for a lone target, "place a target where the potential arrives" for a lone source.
- behaviour-change: Every device symbol of the starter IEC and ISO sets prints its device's function text through a shown function-text slot, one line under its other texts; a text hidden by Hide text stays hidden, and an imported element, a symbol of your own or a copy a project holds from before prints only the texts its symbol has.
- behaviour-change: A mirrored or half-turned symbol's wires leave its pins the new way and go round the symbol, never back through a pin or across it; in the window the turned symbol's wires are then routed again where that runs better, in the same undo step.
- behaviour-change: the accessories a person added to a terminal strip by hand (an end cover, a partition plate, a
  strip marker) now count in the strip's width on its rail, as the end plates its terminals' parts bring do: a strip
  grows by each one's width (a 4 mm end cover makes a 22.6 mm strip of three terminals 26.6 mm), the items after it on
  the rail fit that much further on, and where a rail held the strip exactly, EW-LAY-010 now says where it runs past
  the rail's end (doc 12 §2.6)
- behaviour-change: a part says what it brings with it (doc 07 §2.5): the starter terminals bring their end plate per
  strip and their end bracket per rail group, the parts list and the enclosure legend list them, and a strip takes their
  widths on its rail (the sample's -X1031 moves to rail 2); a device or strip may leave one off or take another part
- behaviour-change: a part may have variants (the starter feed-through terminal in blue): a device's Variant row in
  Properties, the strip editor's Part cell and the place browser choose one, the lists print the variant's own number
  on a row of its own, and EW-CORE-085 names a variant the part does not have
- behaviour-change: the project keeps a workshop stock of wire, ferrule and marker parts (a new project starts with the
  starter's generic ferrules and markers); Project ▸ Edit stock… edits it with what the cut list cuts to, Project ▸ Add
  the generic stock adds what it lacks, and EW-MFG-022 reports a wire whose part is of another size or colour
- behaviour-change: the part dialog greys a part that does not fit the device and says why (a function of another
  kind, a pin it lacks, a clamp too small for a wire), after the parts that fit
- behaviour-change: the place browser's row says what choosing a part does ("Places the Coil A1-A2", "Drawn as a
  device box", "2 functions"), a valve on an electrical page places its coil, the detail lists a part's variants and
  what it comes with, and a greyed row says what no symbol draws on the page
- behaviour-change: the part editor shows a part as its sections, listed on the left and one at a time (Identity,
  Technical data, Pins and clamps, Size and mounting, Accessories, Variants, and a wire's, ferrule's or marker's own),
  and Delete part… deletes a part of the project's own that no device uses
- behaviour-change: the strip editor lists the end brackets and end plates the terminals' parts bring at their places,
  read-only; Delete on such a row leaves it off that strip, and a strip's Properties take or leave each one
- api-add: commands `workshop.stock {stock, copies?, cut_increment?}` (a `null` cut increment clears it) and
  `part.delete {copy}`; `part.assign` takes `variant`, `device.place`'s `from_part` and `new_terminal` take
  `part_variant`, `part.edit_copy` changes a part's `accessories` and `variants`, and `fields.set` sets a device's
  `accessory_choices`
- format-variant: `Part` gains `accessories`, `variants` and the sections `wire`, `ferrule`, `marker`; `Device` gains
  `accessory_choices`; `ProjectSettings` gains `workshop`; each absent when empty
- behaviour-change: saving a corrected part whose devices are drawn as boxes now names each box drawn anew, with its
  page, on the Preview card before Apply; a box that would grow past the page's drawing area (a card given a second
  supply pin, running into the title block) goes back along its edge inside the area, its wires with it, and the card
  says which (FT-2208)
- behaviour-change: EW-BUS-001 offers the quick fix "Give it the next free address" on an open PROFIBUS station
  address: the first address after the highest the net holds, within the part's range, two open ports never given
  the same (FT-2208)
- behaviour-change: the device box's pin list has Add bus interface…, which opens the part editor on Bus interfaces
  with a new interface row, so a gateway or a drive of your own gets its bus port from the box (FT-2208)
- behaviour-change: a cross-reference into a page whose frame draws no column and row strips prints the page alone
  ("103" where a frame with strips prints "103-C1"), also in a project brought in from QElectroTech whose border hides
  both header bands, which printed a cell the page does not show. Nothing else changes for such a page: its wire
  numbers, its potentials' names, the order of its chained interruption points, its devices' numbers, the lists, the
  tags, the cursor's cell and Go to read its cells as before (doc 06 §2.8, FT-1815)
- api-add: a frame's columns and rows are counted with numbers or letters (`counting`), the rows from a first
  letter or number of their own (`start_index`) and the columns skip I and O (`skip_io`), additive members of
  `FrameColumns` and `FrameRows` (doc 09 §2.5, FT-1815)
- api-add: a project's fields of its own (doc 09 §2.10, FT-1814): `Project.user_prop_defs` and `Project.user_props`,
  `Page.user_props`, additive members; the commands `user_prop.define`, `user_prop.rename` and `user_prop.remove`;
  `fields.set` writes `user_props` on the project and on a page; a title block's cell prints a field through
  `{project.prop.<key>}` or `{page.prop.<key>}`
- behaviour-change: Project properties and Page properties hold a group Fields of your own with each field's value,
  New field of your own… (also in the Project menu) and a field's Rename… and Remove; Title block fields… lists the
  fields of the project's own its title block prints, the page's first, and says whose value each is (doc 09 §2.10,
  FT-1814)
- behaviour-change: The wire list names a cable core by its cable and core in the No. column (`-103W1 BN`) instead
  of the Via column; its CP columns print a repeated pin with its number (`+#1`, `+#2`) and a terminal's clamp letter
  (`a`, `b`) instead of the terminal's connection number; an end whose location differs from the other end's prints
  its location (`+FLD-104B1` to `+CAB-104A1-A2`); its Wire part column names the part a wire is cut from, the one
  set on the wire or its potential, else the workshop stock's first wire part of the wire's size and colour. The CSV
  and XLSX of the list follow. The targets of the terminal diagrams and the cable overview print a repeated pin with
  its number as well (`-101G1:+#1`).
- behaviour-change: A wire's length typed by hand is shown and typed where the wire is: the wire list has the columns
  Length (`400 mm`; `cable` for a cable core, cut with its cable; `—` without one) and Length from (typed, cable,
  missing) in its CSV and XLSX and in List columns…, the Connections table's Length cell takes a length in
  millimetres or with its unit (`2 m`) and sets it on a connection of that wire alone, and a wire's Properties have a
  Length row that says where its length comes from. The sample project has typed lengths on 101.1, 102.8 and 103.2.
- text-change: The tube list says `typed` (in Dutch `getypt`) for a length typed by hand, as the wire list does, where
  it said `manual` (`handmatig`).
- behaviour-change: The Output tab has a Workshop group with Wire list, which opens the first page of the project's
  wire list and says so ("Opened the wire list on page 907."); a project without one opens Report… on the wire list.
- behaviour-change: Output ▸ Workshop ▸ Rework list… compares the project with a release (the newest whose archive is
  in the folder beside the project, another release, or an archive file) and lists what the fitter changes in the
  built cabinet: the wires to pull, to replace, the ends to move, the lengths to check and the wires to add, each with
  its change in words ("size 1.5 → 2.5 mm²"); Export… writes it as XLSX or CSV. `ew export rework [--since
  <revision> | --archive <file>] [--format csv|xlsx] --out <file>` writes the same list.
- behaviour-change: A wire list page whose block chooses no columns prints other columns now, the fitter's: No.,
  From, CP, To, CP, Colour, mm², Length, From end, To end, From marker, To marker and Pages. It gains Length, From end,
  To end, From marker and To marker, and no longer prints Potential, Source, Via and Wire part unless they are ticked
  in Output ▸ List columns…, which also offers Length from, From part, To part, From strip length and To strip length.
  A bridge's kind and terminals stand in its From end cell instead of Via. A wire list embedded on a page prints the
  same columns.
- behaviour-change: The Connections table opens with the columns the workshop types and reads: No., From, CP, To, CP,
  Colour, mm², Length, From end and To end, and the window's left column widens to show them; Potential, Source, Via,
  Wire part, Pages, Length from and the ends' parts and strip lengths are in its Columns menu. It opened with every
  column.
- fix: A colour or cross-section typed in a wire's row of the Connections table is set on that wire alone; it was set
  on every connection of the wire as well, so the arm a star or a chain shares with other wires changed their colour or
  size too.
- fix: A bridge row's Length cell in the Connections table is read-only, as a bridge has no length; a length typed
  there was set on the bridge's connections.
- behaviour-change: The wire list's From end and To end say what the fitter puts on each end, by a rule table not yet
  confirmed by a panel builder: `ferrule 1.5 BK`, `twin 2.5 BU · 101.6` (two wires of one size on one clamp, where
  it takes one), `ring lug`, `bare`, `connector`, `?` where the end cannot be made, `—` for an end made outside the
  project; From part, To part, From strip length and To strip length name the ferrule from the workshop stock and its
  sleeve. They are in the list's CSV and XLSX, in List columns… and in the Connections table. Project properties ▸
  Workshop has Ferrules (by default none into a spring or push-in clamp, where the end stays bare, and one on every
  other fine-stranded end; or one on every fine-stranded end) and Two wires in one clamp (a twin ferrule, or a ferrule
  each); `project.settings.set` sets them as `workshop.ferrules` and `workshop.two_wires`.
- behaviour-change: Two new checks: EW-POT-010 (error) when more conductors land on a clamp than its part says it
  takes, a twin ferrule counted once; EW-MFG-008 (warning) for an end that cannot be made, a fine-stranded wire into an
  insulation-displacement clamp.
- behaviour-change: A wire's end can be set by hand: in the Connections table's From end or To end cell (ferrule, twin
  ferrule, ring lug or bare; an empty cell gives the rule's back), or in the wire's Properties, whose new sections From
  end and To end show each end's treatment and part with "set by hand" and Reset, or the rule's, unverified, and what
  it assumed, set from any connection of the wire. Each wire's end is its own, also where several wires share a star's
  or a chain's arm to one clamp: it is stored on a connection of that wire alone (`overrides.end_a`, `end_b`: the end
  reached through its endpoint a or b; a part must be one the project holds), stays with its connection point when the
  wire turns round or loses a sibling, and with the part that keeps that end when a device splits the connection. EW-MFG-009
  (warning) reports an end set by hand that does not fit: a ferrule part of another size than the wire's, a twin
  ferrule with no second wire, a ferrule of another colour than the stock's; its quick fix resets the end to the rule.
  The sample project's 101.4 lands on -101G1:PE with a ring lug.
- behaviour-change: EW-POT-010 also reports a wire whose size lies outside its clamp's range, where the part names one.
- behaviour-change: The rework list's Re-make end lists an end that stays on its clamp but is made another way (its
  treatment, a twin ferrule's partner wire or its part): `at -101Q1:1: ferrule 1.5 BK → bare`.
- behaviour-change: Project properties ▸ Workshop shows Ferrules and Two wires in one clamp with the value that
  applies while they are not set, marked as the default.
- behaviour-change: EW-CORE-085's finding offers the quick fix "Use the part's own number": the part uses of that part
  lose the variant key it does not hold, and the Variant row then chooses one it has
- behaviour-change: the part dialog of a terminal strip offers no end brackets: a strip's end brackets come with its
  terminals' parts, and a strip that holds one as a part from before keeps it and its count
- behaviour-change: the terminal overview's Parts column is wider and its Location and Segments columns narrower, so a
  strip with a variant among its terminals' parts keeps the house text size
- behaviour-change: a strip's choice of what its terminals' parts bring (an end plate left off, a cover swapped) now
  holds when the accessory's part or the terminal's part is corrected as the project's own copy; before, the end
  plate came back and the swap was undone without a word
- behaviour-change: Delete part… refuses a part of your own that another part names as its accessory (and so does
  any delete of such a part), naming the part that names it; before, the end plate went, and it dropped out of the
  parts list and the strip widths without a finding
- behaviour-change: a device that takes a variant of its part prints the variant's own number in the enclosure
  legend (page 902 and its CSV and XLSX), the valve terminal list, the I/O list, the PLC overview and the bus lists,
  the strip editor and the SQL view's PLC channels, as the parts list did already; the legend no longer prints one
  number on two rows
- behaviour-change: choosing a part for several devices at once (a strip's terminals without a part, a selection)
  judges the fit on every device, not the first: an offer that does not fit all says for how many and names the first
  ("Does not fit 3 of 12; -X1:4: …"), and Only parts that fit hides only what fits none of them
- behaviour-change: replacing a device's part in its Parts row or in the strip editor's Part cell drops the choices
  of accessories the new part does not bring, as Choose part… did; before, a stale choice stayed and came back to
  life when the old part did
- behaviour-change: in the part dialog a part of another kind names both kinds ("A part of another kind: A1A2 coil,
  not 1P MCB"), and a fitting part whose clamps on the device's wired pins have no size given says "clamp sizes not
  given" in its row
- text-change: the part dialog's sentences about a clamp's size write the decimal separator of the window's
  language ("0,5 tot 1,5 mm²" in Dutch), as the check messages do
- text-change: a strip's Properties name its accessory rows by the accessory kind's own word, so the marker carrier's
  row reads "Marker carrier" ("Markeringsdrager") as the strip editor says it, not "Strip marker"
- behaviour-change: A device placed on a wire with a typed length cuts it into two wires that nobody measured: neither
  keeps the length (the wire list prints `—` for both); each half kept the whole wire's length.
- behaviour-change: The SQL view's `wire.wire_part` names the part a wire is cut from, as the wire list's Wire part
  column does: the part set on the wire or its potential, else the workshop stock's first wire part of the wire's size
  and colour; it was NULL for a wire without a part set on it.
- behaviour-change: The part of a wire's end set by hand is kept as a link: Delete part… refuses a part of the
  project's own that such an end names, naming the wire ("part EW-GEN-FER-2.5 cannot be removed: wire 101.4 still names
  it"), a fork of the part takes the end with it, and a copied wire carries its ends' parts into another project.
- behaviour-change: Page ▸ Frame ▸ Edit frame (also a double click on the title block or the rulers, and the title
  block's menu) changes the page's frame and title block on the page: a cell's text, what it shows (in words, a field
  of your own made from the cell, the company logo, a picture or text only), its text size and alignment; split, join
  and clear cells; drag a line between cells. The first change makes the program's frame and title block the project's
  own, and every page that printed them follows; a new list still starts on the project's copy of the frame without
  rulers, which prints the changed block. Title block fields… moved into the new Frame group (doc 09 §3.9, FT-1814)
- api-add: the command `form.edit`, the one change of a frame or title block copy (forked when it is not the
  project's own, every reference re-pointed); a title block's lines are its cells' edges (`FormDef::cell_edges`)
  (doc 09 §2.5, §3.9, FT-1814)
- behaviour-change: Edit frame's click on the column numbers or row letters chooses them: Properties sets how many
  columns and rows, counted with numbers or letters, from which first label, rows skipping I and O, whether they are
  drawn and the title block's height; the change is previewed with the cross-references and tags that print
  differently and applied in one step, the drawing area's edges kept exactly. The title block's height is the
  block's: every frame that names it (the cover's, the list pages', the frame without rulers) takes it from its rows,
  so the block ends at the border on each; a block that would grow over drawing on a page of any of them is refused
  naming the page, and one that would make a cell lower than 2 mm is refused; the block's cells, pictures and texts
  scale with its height (doc 09 §3.9, FT-1815)
- api-add: the command `frame.set_grid` (Bulk) (doc 09 §3.9, FT-1815)
- behaviour-change: Edit frame's Properties of the frame give a frame its own title block (Title block: Its own, or
  Shared with the other frames again), give the page its own frame, and Use the frames of a template… takes a
  template's frames, title blocks, frame defaults, the frame of the lists, its fields of its own, its company logo
  and notice and the pictures put into its title blocks into a project made before, previewed and in one step; a page
  with a frame of its own keeps it. A picture put into a title block is written with the next save of the project
  (doc 09 §3.9, FT-1814)
- api-add: the commands `form.make_own` and `frames.adopt` (Bulk) (doc 09 §3.9, FT-1814)
- behaviour-change: a QElectroTech project's own title-block variables (`%{name}` of the project's or a folio's
  properties) come in as fields of the project's own, one value for the project where every folio has the same, else
  one for each page, and the title block's cell prints the field; the printed text is the same, and the value is now
  typed in Title block fields…; such a variable is no longer kept a second time in the project's or the page's
  `qet.properties`, nor in the project's `qet.project_properties` (doc 14 §4.9, FT-1814)
- behaviour-change: every region of a QElectroTech title block comes in as a cell, a logo's and an empty one's too,
  so Edit frame can choose it and type in it, and the block's lines are its cells' edges: its first change keeps every
  line (a line beside an empty region went) (doc 14 §4.9, FT-1814)
- behaviour-change: the Preview of a bulk change lists the wire numbers it changes, old → new, beside the tags and the
  cross-references (Columns and rows of a frame among them); a frozen number keeps its own (doc 16 §7.6, FT-1815)
- behaviour-change: Each wire end has a marker, the text of the sleeve on it: by default the far end as seen from this
  end (`-101Q1:1` at -X1011:L1, `+FLD-104B1:13` at a cabinet end of a field wire), or as Project properties ▸ Workshop
  ▸ Marker text sets it (the wire number, both ends on two lines, the potential); a cable's core and a jumper on one
  device carry none unless Mark cable cores or Mark jumpers is set. A text too long for the stock's wire marker is
  shortened (the product sign dropped, the line broken after `:`, `-` or `+`) and else kept whole: EW-MFG-004
  (warning) names it. The wire list prints From marker and To marker by default; a marker is set by hand in the
  Connections table's marker cells or the wire's Properties (`—` for none).
- behaviour-change: A new list kind, Labels (`labels`), lists every label of the workshop in the order the fitter
  applies them: the wire markers, the terminal markers (with the function text on a second line where the strip prints
  function texts) and strip markers, the cable markers (two per cable), the device labels (placed devices in mounting
  order, then the others; two where Project properties ▸ Workshop ▸ Device labels says on the device and beside it), the
  tube markers and the plates of the devices in the door, each with its texts, copies and the stock's marker part that
  marks its set.
- behaviour-change: The tube labels list (`tube_labels`) has the labels list's columns (Set, For, Text 1 to 3, Copies,
  Marker part, Note) where it had Tube, At and Label text.
- behaviour-change: The rework list also lists the labels' work: Remove device, Remove terminal, Remove cable, Re-mark (a
  wire end's marker that says another text), Re-label, Mount device, Add terminal and Add cable.
- behaviour-change: `ew export labels --dir <folder> [--format csv|xlsx] [--since <revision> | --archive <file>]` writes
  the label files,
  one per label set and marker part (`SAMPLE-E Wire markers EW-GEN-MRK-1.csv`), in the columns of the project's export
  profile, else the generic one (Text 1, Text 2, Text 3, Copies, For); the CSV has a byte-order mark, CR LF and the
  profile's delimiter and its values as they are, never guarded; `--since` writes only the labels the rework list
  against that release names, `--archive` those against that archive file. Each file is written whole.
- api-add: `project.settings.set` sets `workshop.label_profile` (`{columns: [{key, heading}], delimiter?}`), refusing in
  words a column the labels list does not have and a profile without columns.
- format-variant: `WorkshopSettings` gains `ferrules`, `two_wires`, `marking`, `mark_cores`, `mark_jumpers`,
  `device_labels` and `label_profile` (`ExportProfile`, `ProfileColumn`, `Delimiter`); `WireOverrides` gains `end_a`
  and `end_b`; `ReportKind` gains `labels`; each absent when not set.
- format-variant: report_block ReportKind labels
- behaviour-change: An end set by hand that a wire keeps on another of its own connections than the first (as a
  change of its carrier leaves it) is the one Properties, Reset, a new edit and EW-MFG-009's quick fix change; they
  wrote on the first connection and missed it.
- behaviour-change: A wire's Length row in Properties writes where the Connections table writes, the wire's own
  connection, from any connection of the wire; on a stretch several wires share (a star's feed arm, a chain's arm at a
  clamp) the Length, end, part and marker rows are read-only and say "several wires share this stretch: set a wire's
  own values in the Connections table". They showed the first wire's values there and wrote them on the shared
  stretch, for every wire through it.
- behaviour-change: A wire's own values follow the wire. When a junction dissolves (a T loses a branch), the wire
  keeps the length typed on it and its ends set by hand, and the value of every other member the removed connection
  held, each end at its clamp; they went with the removed branch. A wire drawn onto a wire leaves the new branch none
  of the old wire's typed length or its end set by hand, which stay the old wire's.
- behaviour-change: Output ▸ Workshop ▸ Labels… lists the label files, one per label set and marker part, each with its
  count and a tick: every label, or those changed since a release (Labels for these changes… in the Rework list…
  opens it on them); the format (CSV or XLSX); the columns with Change…, which sets them with their headings, order and
  delimiter (Label file columns); and how many labels do not fit their marker part. Export… writes the ticked files
  into a folder, each whole, and the toast offers Show in folder. Output ▸ Workshop ▸ Workshop settings shows the
  project's Properties at its Workshop rows.
- behaviour-change: Labels… lists the labels that do not fit their marker part under their count with Show (Hide
  counts them again), each by its label set, marker part and text ("Device labels on EW-GEN-MRK-3: -102S1").
- text-change: The drawing's wire marker is now the wire annotation (Dutch: draadaanduiding) wherever the window says
  it: the Draw tab's Marker button is Annotate (Dutch: Aanduiding), the tool is Wire annotation, a wire's Properties row is Wire
  annotation, Project ▸ Wire markers is Wire annotations, and the refusals and notices that name it say so. The stored
  names (`wire_marker`, `show_marker`, `marker_mode`) stay; "wire marker" now means the workshop's sleeve label.
- behaviour-change: three workshop lists (doc 13 §3.6, §6, §9.3): Assembly lists prints per terminal strip a heading
  that says where it goes and how long it is ("-X1011 · on rail 1 from 0 to 48.2 mm · rail cut to 470 mm · 5
  terminals · 48.2 mm with its end brackets"), then every item in mounting order (end brackets, terminals with part,
  colour and marker, end plates, the strip's own accessories, each with its width), then its bridges in a block of
  their own; Cut list prints the rails, the ducts with their covers, the cables, the wires and the tubes to cut, each
  piece rounded up once to the cut increment (a wire's length as the wire list's); Consumables prints the wires' metres,
  the ferrules and lugs per part (a twin ferrule once) and the markers per label set. Each is a list kind
  (`assembly_list`, `cut_list`, `consumables`) for Report block… and `ew export report --kind`.
- format-variant: report_block ReportKind assembly_list
- format-variant: report_block ReportKind cut_list
- format-variant: report_block ReportKind consumables
- behaviour-change: a `tube_cut_list` block of a project made before prints the cut list's Tubes group under its own
  title: one row per tube part (else material, size and colour) and piece length, where it had one row per tube part
  for every length; each piece rounded up to the project's cut increment (5 mm unless set), the total the sum of the
  rounded pieces; the cut list's columns Part number, Description (the material, size and colour), For, Pieces, Length
  mm (the piece) and Total m, where it had Material, OD×ID, Colour, Tube part, Pieces and Total length. What a person
  set on such a block reads as before: a column choice, sort, grouping or row of one's own under the old keys is read
  as the new columns (part → Part number, material, size and colour → Description, count → Pieces, length → Total),
  and a row is still its first tube, so a row left out stays out. Report block… no longer offers the tube cut list.
- behaviour-change: the wire list's bridge rows print the bridge's part; only a bridge without one prints TBD.
- behaviour-change: EW-MFG-020 reports a strip whose terminals have parts and after whose last terminal no end plate
  stands, with the quick fix Add the end plate … after … where a terminal's part names one; EW-MFG-021 reports a
  bridge without a part (an error in release).
- behaviour-change: Output ▸ Workshop gains Assembly lists, Cut list and Consumables, each of which opens the first page
  of the project's list of that kind, else Report block… on it.
- format-variant: mounting_surface SurfaceKind door (a cabinet's door, seen from outside; doc 12 §2.8)
- format-variant: mounting_surface SurfaceKind left (the left side, seen from the left)
- format-variant: mounting_surface SurfaceKind right (the right side, seen from the right)
- format-variant: mounting_surface SurfaceKind top (the top, seen from above)
- format-variant: mounting_surface SurfaceKind bottom (the bottom, seen from below)
- format-variant: part PartKind gland (a cable gland; doc 12 §3.9)
- format-variant: record surface_feature.schema.json (a hole a person added to a surface, by hand or a cable's gland,
  rank 58 in `layout/<space>.jsonl`; doc 12 §2.2)
- behaviour-change: a cabinet's door, sides, top and bottom are surfaces of their own: Draw ▸ Enclosure ▾ ▸ Surface…
  (and a panel page's context menu, "Surface: Door") asks which face of the cabinet and goes to its page, or makes, in
  one undo step, the face from the enclosure part, a panel layout page after the plate's page with the face drawn as
  it is seen from its side (a door from outside, its hinge ticked, its edge margin dashed) and its legend page; on a
  door's page the plate's devices show faint behind it on the screen (doc 12 §2.8, §3.8, FT-1843)
- behaviour-change: a panel layout page writes its caption under the drawing ("Mounting plate, seen from the front")
  and marks the corner its holes are measured from with "0"; a surface's Properties show its Width, Height, a door's
  Hinge, its Edge margin and Measured from (its own corner or the project's) (doc 12 §2.8, §5.2)
- text-change: the page's Surface row names a surface by its cabinet's tag and its face, "-UC1 mounting plate" (it was
  "-UC1 plate"), in the window's language
- api-add: `layout.surface.add {enclosure, kind, door?, width?, height?}` makes a face of a cabinet from its part, and
  `layout.surface.edit` takes `width`, `height`, `hinge`, `datum` and `edge_margin` beside `pe_bond`, which is now
  optional (doc 12 §3.7)
- behaviour-change: a device whose part goes in a cut-out (a pushbutton, a pilot lamp, a handle) is placed on a door's
  or a wall's page, drawn round where its front is round; dropped on the plate or a rail it is refused in words with
  Open the door, and the bar under a plate's page counts the devices that wait for the door with a Door button. A
  device's further part that goes in a cut-out (a main switch's door handle) is a row of its own in Unplaced and in
  EW-LAY-001 ("-101Q1's part EW-GEN-HDL-1 has no place in the layout"), placed as the device is (doc 12 §2.7, §3.4)
- behaviour-change: a placement on a door's or a wall's page shows its Surface and its centre from the surface's
  measured-from corner in Properties, and on a door's page whether the plate behind it holds something there (doc 12
  §3.4)
- behaviour-change: EW-LAY-010 names a cut-out device on the plate ("goes in a cut-out of a door or a wall"), a rail
  part on a door or a wall, and a device in a face's edge margin (doc 12 §2.7)
- text-change: a mounting's kind ("rail", "cut-out", "screws") and a rail's or duct's name ("Rail 1") in Unplaced,
  Properties and the refusals are in the window's language; they were English in the Dutch window
- behaviour-change: the starter library 0.22.0: the pushbuttons EW-GEN-PB-1, -PB-2 and the pilot lamps EW-GEN-LMP-1 to
  -LMP-4 go in a 22 mm cut-out with their front and depth, the enclosure EW-GEN-ENC-1 has a door hinged left, an edge
  margin of 30 mm and a usable depth of 200 mm, and the door handle EW-GEN-HDL-1 is new; the sample's main switch
  -101Q1 takes the handle, and its door page 904 holds the handle, the lamps and the pushbuttons (doc 12 §3.4)
- api-add: `layout.place` takes `part` (the device's part use to place, its first by default), and the Unplaced
  navigator's rows of a further part use are keyed `device:<device>:<part>` (doc 12 §3.4)
- behaviour-change: the part editor's Front view and holes: the front's Shape (Rectangle or Round), its outline
  imported from a DXF drawing (Import outline…, Remove) and the table Holes, each hole by its X, Y, Kind (Hole,
  Fixing, Cut-out, Gland, Rectangle) and Size, with a preview of the front and its holes; a size that does not read
  for its kind, a hole off the front and a round front of unequal sides are refused in words. It is edited on a new
  part and a part of one's own, and read-only on a library part (doc 07 §5, FT-1771)
- behaviour-change: a project's own part changes its size and mounting in the part editor (`part.edit_copy` takes
  `mech` of a project-local copy); Save was refused
- api-add: the hidden actions `part.outline_import {path?}` and `part.outline_remove`; the part editor's declared
  table names a hole's two cells `hole.kind` and `hole.size` (`mech.holes[0].hole.size`)
- format-variant: report_block ReportKind hole_table (every hole of every surface, numbered in drilling order; doc 13
  §9.2)
- behaviour-change: every hole and cut-out of a surface is derived from what is placed on it and what is added by hand
  (doc 12-3d §10): a part's holes, a rail's and a duct's fixing holes (M5 and M4, at most 300 mm apart, 25 mm from
  each end, unless Project properties ▸ Workshop says otherwise), the holes by hand; made as the workshop says (a
  fixing in the plate tapped by default, in a door through), numbered in drilling order. A panel layout page draws
  them at true size with a centre cross, dashed under a device
- behaviour-change: Add hole here… in a panel page's context menu adds a hole by hand (Kind, Size, X and Y from the
  surface's corner, For); a selected hole by hand shows its Kind, Size, X, Y and For in Properties; a placement's and
  a rail's or duct's Properties list their holes, each moved by typing its X and Y and left out with Leave out; a
  surface's Properties count its holes
- behaviour-change: Output ▸ Workshop ▸ Hole tables shows the project's hole table, else opens Report block… on it;
  the hole table lists each surface's holes after a heading that names its page and the corner they are measured
  from, with X and Y, how each is made ("Ø 4.2, tap M5"), what it is and what it is for. The sample has an earth stud
  on its plate and its hole table on page 925
- behaviour-change: EW-LAY-012 Holes too close (two holes closer than the web between holes, a hole in a sheet's edge
  margin or off the sheet) and EW-LAY-017 Holes not known (a part in a cut-out or screwed to a surface without
  holes, or a device placed there without a part), both warnings
- api-add: `layout.feature.add {surface, at, kind, size, purpose?}`, `layout.feature.edit {feature, at?, kind?, size?,
  purpose?, for_cable?}` and `layout.hole_choice {placement | carrier, index, offset?, omit?}`; the normaliser
  `stale_hole_choices` drops a placement's or a carrier's hole choices when its pattern changes
- behaviour-change: every cable with an end in a cabinet and an end outside it gets a cable gland where it enters the
  cabinet: the first gland of the workshop stock whose clamping range holds the cable's outer diameter (or the one
  chosen for it), on the cable's line of the parts list, in the legend of the surface it is placed in, and in the hole
  table as its gland hole; Draw ▸ Place cable glands puts the cabinet's waiting glands in a row on the side, the top
  or the bottom shown, their nuts apart by the web between holes. EW-LAY-018 Cable gland says what is not placed, not
  known or no longer fits (doc 12 §3.9)
- behaviour-change: the starter library's cable glands EW-GEN-GLD-M16 to -M40 (kind gland, with their thread,
  clamping range and nut) are in a new project's workshop stock after its ferrules and markers, and the stock takes a
  gland; the cables EW-GEN-CBL-2, -17 and -25 give their outer diameter. The part editor has a Gland section for a
  gland's thread, clamping range and nut, on a new part and a part of one's own; a cable type's Diameter is kept in
  its cable section (an older part's technical value `outer_diameter` is still read, and Save moves it there). A gland
  is offered by the part chooser only where a gland is asked for
- behaviour-change: the sample's cabinet has its bottom on page 905 with the glands of -101W1 and -103W1
- api-add: `layout.glands_place {surface, at, cables}` and the action `layout.glands_place {cables?, x?, y?}`
