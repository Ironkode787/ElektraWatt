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
  both header bands, which printed a cell the page does not show; such a page has no cell for the cursor, Go to or the
  lists either (doc 06 §2.8, FT-1815)
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
  From, CP, To, CP, Colour, mm², Length, From end, To end and Pages. It gains Length, From end and To end, and no
  longer prints Potential, Source, Via and Wire part unless they are ticked in Output ▸ List columns…, which also
  offers Length from, From part, To part, From strip length and To strip length. A bridge's kind and terminals stand
  in its From end cell instead of Via. A wire list embedded on a page prints the same columns.
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
  own, and every page that printed them follows. Title block fields… moved into the new Frame group (doc 09 §3.9,
  FT-1814)
- api-add: the command `form.edit`, the one change of a frame or title block copy (forked when it is not the
  project's own, every reference re-pointed); a title block's lines are its cells' edges (`FormDef::cell_edges`)
  (doc 09 §2.5, §3.9, FT-1814)
- behaviour-change: Edit frame's click on the column numbers or row letters chooses them: Properties sets how many
  columns and rows, counted with numbers or letters, from which first label, rows skipping I and O, whether they are
  drawn and the title block's height; the change is previewed with the cross-references and tags that print
  differently and applied in one step, the drawing area's edges kept exactly; a title block that would grow over
  drawing is refused naming the page (doc 09 §3.9, FT-1815)
- api-add: the command `frame.set_grid` (Bulk) (doc 09 §3.9, FT-1815)
- behaviour-change: Edit frame's Properties of the frame give a frame its own title block (Title block: Its own, or
  Shared with the other frames again), give the page its own frame, and Use the frames of a template… takes a
  template's frames, title blocks, frame defaults, the frame of the lists, its fields of its own and its company logo
  and notice into a project made before, previewed and in one step; a page with a frame of its own keeps it (doc 09
  §3.9, FT-1814)
- api-add: the commands `form.make_own` and `frames.adopt` (Bulk) (doc 09 §3.9, FT-1814)
- behaviour-change: a QElectroTech project's own title-block variables (`%{name}` of the project's or a folio's
  properties) come in as fields of the project's own, one value for the project where every folio has the same, else
  one for each page, and the title block's cell prints the field; the printed text is the same, and the value is now
  typed in Title block fields… (doc 14 §4.9, FT-1814)
- behaviour-change: the Preview of a bulk change lists the wire numbers it changes, old → new, beside the tags and the
  cross-references (Columns and rows of a frame among them); a frozen number keeps its own (doc 16 §7.6, FT-1815)
