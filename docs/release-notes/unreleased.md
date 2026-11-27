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

- behaviour-change: the part editor lists a part's groups as the sheet of Parts names them (Identity, On the schematic,
  In the cabinet, Connections, the kind's own group, Technical data, Goes with it), each with the fields one declaration
  of a part's fields gives (doc 07 §2.5); an enclosure, a mounting plate and a rail of one's own now have their
  rows (size, plate, doors, edge margin, profile), a valve its coil and a tube its data; the GTIN stands in Identity
  after the order number, and Parts' list finds a part of the project by it as the libraries' search does
- behaviour-change: `part.edit_copy` takes a change of any member of a part; it refused the mechanical data, the
  contacts, the sections of a library copy's fork and the free properties (doc 07 §5.4)
- text-change: the guide's reference gains the page Parts ▸ Fields, every field of a part with what reads it
- behaviour-change: Parts is a screen of its own (Project ▸ Parts, `F9`; doc 07 §5) and replaces the part editor
  dialog: the list of the project's, your own and the libraries' parts with its scopes and filters, the open part's
  sheet with its groups one under another (two columns at 2560 × 1440), how it is drawn and its front beside it, and
  Ready for, what the program's features still lack of it. Edit part…, New part…, New cable type…, New bus cable…,
  New plug… and a station's New part… open it; Back (or Escape) returns. A starter part is never changed: Make my own
  from it; a part only in this project has Keep in my parts; Remove… takes a part out of My parts or of the project.
  The tick "Also add to my parts library" goes: a part you make is in My parts
- api-break: the window's part actions are `parts.open`, `parts.find`, `parts.new`, `parts.field`, `parts.row`,
  `parts.outline`, `parts.save`, `parts.discard`, `parts.own`, `parts.keep`, `parts.remove` and `parts.back`;
  `part.edit`, `part.new`, `part.field`, `part.save`, `part.section`, `part.table_row`, `part.bus_row`,
  `part.outline_import`, `part.outline_remove`, `part.delete` and `part.remove_own` go
- behaviour-change: New part… asks first what the part is (doc 07 §5.5): eighteen cards in four groups, from Device
  with symbols to Other pneumatic device, each presetting its kind and asking only what that kind needs; A part like
  it… and Like this start a part with everything another has but its number. A new terminal's pins get their clamp
  rows at once, a new wire, ferrule or marker takes its own data, Parts finds cable glands, and a part that exists is
  refused with its maker and number ("Example CON-18-24 is in My parts or in this project already")
- api-break: `parts.new` takes `start` (a card's or a form's start: `device`, `terminal`, `cpu`, `plug`, …) and
  `like`; its `kind`, `from`, `plc`, `cable` and `plug` go
- behaviour-change: the sheet of Parts is built from the design language's components (doc 16a §6.27): fields two to
  a line with the unit after the value, a group's one line beside its heading, the room kept free as one line of four;
  a part drawn with symbols is a table of its pieces (what it is, its pins, its symbol) and a terminal a grid of its
  levels, each written into the part's functions on Save; Connections puts pins with the same clamp on one row
- behaviour-change: the list of Parts counts its parts, groups them (Lately opened, then by what they are) and filters
  them by maker; the heading names the devices that use the part, each a way to it; Back with changes not saved asks
  Save, Discard or Stay; a cabinet's mounting plate is made in passing from its field (New plate…)
- behaviour-change: Ready for names every feature that reads the part, with a tick, what it lacks, or why it does not
  apply; a feature no longer says a part lacks what its kind is never read for (a contactor's PLC ratings, a
  cylinder's coil, a plug's footprint)
- behaviour-change: a part of a library that is not installed is only in this project, his to change and to keep;
  Keep in my parts moves every use of the part with what it chose (a device's variant included), in one undo step
- api-break: `part.edit_copy` refuses a starter part ("make a part of your own from it"); `part.keep` is new;
  `parts.back` takes `save`, `discard` and `show`, `parts.find` takes `maker`, `parts.new` takes `for_field`
- behaviour-change: Parts opens with no project too (the start page's Parts, `F9`): the libraries alone, a new part
  saved into My parts; nothing is kept in or removed from a project there
- behaviour-change: another part opened or New part… while the card of a Save waits asks Save, Discard or Stay, as
  Back does; a piece of the schematic table reads what it is without another piece's pins ("NO contact"); the list's
  groups read in the plural; the sheet lists network ports and the PLC section where the start asked for them or they
  hold a value
- api-break: `parts.open` and `parts.new` take `card` (`apply`, `cancel`); `parts.open` needs no project
- behaviour-change: a finding or a refusal that comes from a part's missing datum offers Edit part… (doc 07 §5.7): the
  Messages row of EW-LAY-017 (holes) and EW-LAY-018 (a cable without an outer diameter), and the toast of Surface…
  or Enclosure… refused for a cabinet's missing size; it opens Parts at the field, ringed, with "Opened from a finding
  on page …" and Back to Messages. EW-LAY-017's part without holes, and Ready for's holes, take a part screwed on as
  well as one in a cut-out
- api-break: `parts.open` takes `field` (a field's path, as `mech.holes[].x`) for its `section`; a finding's quick
  fix `parts.open` goes: a finding and a command's refusal carry the part and the field it lacks (`part_lack` in
  `ew check --json`)
- behaviour-change: a bond made, removed or given another via connection, where it changes the wires through the via,
  keeps each wire's own values with the wire as every other change of the wiring does; a wire it joined anew kept the
  typed length of one it was made from (doc 13 §2.3)
- behaviour-change: a wire that ends open, or outside the project, at an interruption point across its pair, with a
  chain or a star behind the pair, shows and takes its ends set by hand at its clamps and keeps them through a change
  of the wiring; they read as the rule's and went at the next change (doc 13 §2.3)
- behaviour-change: Label file… keeps what a newer build wrote into the export profile: a delimiter or a file form it
  does not know, left as it is, stays (the edit was refused), and so do the profile's unknown members and those of a
  column it keeps (they were dropped) (doc 13-package §7.3)
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
  one per label set and marker part (`SAMPLE-E Wire markers EW-GEN-MRK-1.xlsx`), in the columns and the form of the
  project's export profile, else the generic one (an Excel file with Text 1, Text 2, Text 3, Copies, For); `--format`
  sets the form for that export; the CSV has a byte-order mark, CR LF and the
  profile's delimiter and its values as they are, never guarded; `--since` writes only the labels the rework list
  against that release names, `--archive` those against that archive file. Each file is written whole.
- api-add: `project.settings.set` sets `workshop.label_profile` (`{columns: [{key, heading}], delimiter?, file?}`),
  refusing in words a column the labels list does not have, a profile without columns and a form other than `xlsx` and
  `csv`.
- format-variant: `WorkshopSettings` gains `ferrules`, `two_wires`, `marking`, `mark_cores`, `mark_jumpers`,
  `device_labels` and `label_profile` (`ExportProfile`, `ProfileColumn`, `Delimiter`, `FileForm`); `WireOverrides` gains `end_a`
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
  opens it on them); the label file's profile in a line ("Excel · Text 1, Text 2, Text 3, Copies, For") with Change…,
  the form Label file, which sets the columns with their headings and order, Excel or CSV and a CSV's delimiter for the
  whole project (Labels… has no format field of its own); and how many labels do not fit their marker part. Export…
  writes the ticked files into a folder, each whole, and the toast offers Show in folder. Output ▸ Workshop ▸ Workshop
  settings shows the project's Properties at its Workshop rows.
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
- behaviour-change: a panel layout page of a door, a side, the top or the bottom writes its caption under the drawing
  ("Door, seen from outside"; the plate's page has none), every panel layout page marks the corner its holes are
  measured from with "0"; a surface's Properties show its Width, Height, a door's Hinge, its Edge margin and Measured
  from (its own corner or the project's) (doc 12 §2.8, §5.2)
- behaviour-change: the panel page of a project made before this version now shows the fixing holes of its rails and
  ducts, the holes of its parts and the "0" at the corner its holes are measured from; nothing on it moves (doc 12 §5.2)
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
- behaviour-change: a door's, a side's, the top's and the bottom's page: every tag stands above its front (it crossed the
  centre mark of a round front's cut-out), the cut-out under a front is dashed in longer dashes that show at 1:5, the plate's
  rails, ducts and devices stand faint behind a door on the screen (they were missing), the drawing stands higher so
  that its caption is clear of the title block, a gland draws its nut's room dashed and its cable's tag under it, and
  the top's and the bottom's page name the edge that meets the door ("Door's edge")
- behaviour-change: the bar under a panel page counts the glands that wait ("2 glands to place") with Place cable glands
  on a side's, the top's or the bottom's page and Bottom elsewhere; a door's or a wall's page no longer says the
  plate's first steps ("No device has the location +CAB yet" stood under the bottom with its glands placed)
- behaviour-change: Delete takes a selected hole by hand or gland off its surface (`edit.delete` takes
  `surface_feature`); a gland taken off waits again
- behaviour-change: Unplaced devices lists the glands that wait under Glands; dragged onto a side, the top or the
  bottom (with the glands marked beside it) they are placed at the drop, and a single gland dropped near a row of
  glands snaps to its end at the pitch of the nuts. A cable's Properties show its Gland, proposed from its diameter or
  chosen (typed over with a gland part of the project), and its Gland hole (doc 12 §3.9)
- behaviour-change: quick fixes for EW-LAY-017 (Edit part… at Front view and holes, Choose part…) and EW-LAY-018
  (Place on the bottom, Edit part…, Choose gland…, Take M32, Remove the hole); a fix that opens a window runs at once,
  without a card, and joins no Fix all. EW-LAY-018's gland not placed is an error at a release, its other forms stay
  warnings (doc 15 §3.2)
- behaviour-change: Output ▸ Workshop ▸ Drilling… exports the drawings for drilling of the surfaces with holes: per
  surface a template to print at full size (`+CAB Door template.pdf`, A4 or A3, lying in the corner nearest the
  holes, its sheets overlapping with crosses where they meet, each hole with its centre cross and its words, a 100 mm
  bar) and a DXF for a machine (`+CAB Door.dxf`, origin at the datum corner, a layer per way of making a hole:
  `HOLE_4.5`, `TAP_M5`, `CUT_22.3`, `CUT_RECT`, with `OUTLINE`, `MARGIN`, `NUMBERS` and `DATUM`), each file whole
  (doc 13 §9.1)
- api-add: `ew export drilling --dir <folder> [--surface <words or ID>]… [--template a4|a3] [--no-dxf]
  [--no-template]` and the action `drilling.export {surfaces?, template?, dxf?, paper?, dir?}`
- behaviour-change: Surface… lists the cabinet's faces as a table, each with its size and its page, one chosen with
  a radio button (it was a drop-down list)
- behaviour-change: a cable between two cabinets has a gland in each: placing it in one leaves the other waiting,
  EW-LAY-018 says so per cabinet, and `layout.glands_place` refuses a cable named twice. A gland hole whose cable no
  longer leaves its cabinet is EW-LAY-018's (Remove the hole; the hole's context menu: Keep it as a hole)
- behaviour-change: a hole moved or left out names its part's hole: it stays on that hole when the part's holes are
  edited or a library update replaces the copy; one whose hole is gone applies to none, and EW-LAY-017 says so with
  Drop the choices that find no hole (`layout.hole_choice` takes `lost`)
- format-variant: placement `hole_choices[].hole` (the part's hole a choice names; doc 12 §2.2)
- api-add: `fields.set` takes a device's `gland`; `layout.glands_place` takes the sheet point `x_mm`, `y_mm`; `part.edit`
  takes `section`
- format-variant: record committed_route.schema.json (the kind `committed_route`, rank 56, is typed: the stored way
  of a wire in its cabinet;
  `CommittedRoute`, `RouteEnd`, `WayStep`, `DuctStretch`; doc 05 §2); `CpData` gains `face` (`PinFace`),
  `WireSection` gains `outer_diameter`, `WorkshopSettings` gains `door_loop`, `fill_limit` and `reserve`; each absent
  when not set.
- behaviour-change: Route on a panel page lays the cabinet's wires through its ducts (doc 12 §8): each wire with both
  ends in the cabinet gets its way, from its pin into the first duct its line meets (or back across its own device's
  front), along the ducts, out to its other pin, or straight over the fronts between neighbours on one rail; a wire
  whose duct is full to the project's fill limit goes another way where one has room. The way is stored, and the
  wire's length is made from it: its run plus 50 mm reserve at each end, rounded up once to the cut increment. The wire
  list prints it with Length from "routed", a way that no longer holds (a device moved or turned, a duct changed)
  keeps its length printed after "≈" with "stale", and the cut list and the consumables print such a piece and sum
  with "≈". A typed length still wins. A project without stored ways prints what it printed; after Route a field
  wire's Length from says "site". The pins stand on the top or bottom edge of their device by the drawing (a pin a wire
  leaves upwards on its page at the top, any other at the bottom) or by the part's own word.
- api-add: the command `layout.route {space, wires?, all?}` (Bulk) stores the plan of a cabinet's ways, one
  `committed_route` per wire; `Session::cabinet_route(folio, all)` gives Route's card with the preview that stores it;
  `ew route <project> --page <panel page>` routes the cabinet (`--dry-run` prints it; the JSON's reasons `unplaced`,
  `no_duct`, `off_plate`)
- api-add: `project.settings.set` sets `workshop.reserve`, `workshop.fill_limit` and `workshop.door_loop`, refusing in
  words a length below nothing and a limit outside 1 … 100
- behaviour-change: a wire's Properties say how its length is made ("Routed: 577.3 mm of way, 50 mm reserve at each
  end, rounded up to 680 mm"; "Stale: -X1031 moved. Route gives it a new way"; "No way yet: Route on page 902 gives it
  one, or type its length here") and where it runs (Way: "Duct 1", "direct"); a duct's say its inner area and its fill
  where fullest against the limit; a placement's what it reaches from the plate, and its clearance in front
- behaviour-change: Route (Draw ▸ Connect) on a panel page that shows a cabinet opens Route's card in the Preview tab:
  the cabinet's ducts with their fill, each wire's new length (old → new for a way that changes), the typed lengths
  with what Route gives, the wires made on site and those without a way in words, and the tick Route every wire
  again; the canvas draws every planned way side by side in its duct and a chip with each duct's fill, a row hovered
  or clicked shows its way alone; Apply stores the ways in one step ("Routed 41 wires of +CAB."). The panel page's
  HUD names Route as the next step once every device is placed, a move or turn that leaves ways no longer holding
  says so in its toast with Route, and a placement selected on the panel page shows the ways of its wires (dashed
  where they no longer hold). On any other page without a drawing Route is greyed with its reason.
- behaviour-change: Project properties ▸ Workshop holds Reserve per end, Duct fill limit and Door loop with their
  defaults (50 mm, 60 %, 300 mm); the part editor's Pins table has the column Wired at (From the drawing, Top,
  Bottom; an empty cell shows the drawing's edge, "top · drawing"), its Size section the row Clearance in front, a
  wire part's section the row Outer diameter, and a duct part a section Duct with its inner width and height
- behaviour-change: four new checks of a cabinet's ways (doc 15 §4.5.4, §4.5.5): EW-LAY-007 (warning) a footprint or a
  duct inside the room a device's part keeps free, with the side and the amount; EW-LAY-008 (warning) a duct over the
  fill limit where it is fullest; EW-LAY-011 (warning, an error at release) the ways that no longer hold, one finding
  per moved device or changed duct with Route as its fix; EW-MFG-001 (info) the wires of a cabinet without a length,
  in a project with a panel layout only, with Route as its fix
- behaviour-change: Route lays a wire to a device in a door (doc 12 §8.1): through the ducts to the duct's end or corner
  from which the rest is shortest, in free air to the plate's edge on the door's hinge side, along the door to the
  device and up half what it reaches behind the door; its length adds the door loop. Two devices of one door are
  joined directly; a wire between two doors, or to a wall, the top or the bottom, gets no way and says so
- behaviour-change: EW-LAY-006 judges the closed door where the enclosure part gives its usable depth: a device or a
  duct of the plate that reaches further ("-101G1 reaches 117.5 mm from the plate; the closed door is 100 mm away"),
  and a device of the plate and one of the door that stand in front of each other and together take more than the
  room. A placement's Properties on the plate say what it takes and the room to the door; a device in a door without
  a depth says the closed door is not checked for it
- behaviour-change: the sample's cabinet is routed (Route on 902): its wire list, cut list and consumables print every
  cabinet wire's length, 102.8's and 103.2's typed lengths are gone (101.1's 400 mm stays as the typed length that
  wins), and the cut list takes a third page (913.2)
- fix: a wire whose junction is dissolved, and a project made from a template, keep no stored way of a connection that
  is gone
- behaviour-change: Route on a cabinet whose every wire has its way says so and opens its card with Route every wire
  again ticked, so the whole cabinet can be laid again from the window
- behaviour-change: Route's card lays its groups out as the design draws them: Ducts (each with its wires, its fullest
  piece's, its fill, the inner area, the limit and the default diameters assumed), Lengths, and Typed by hand, Made on
  site (with the end outside the cabinet), Without a way and Route every wire again (with its count in words), side by
  side where the window has room; the canvas shows the cabinet whole while the card is open and the view comes back
  when it closes; the wires that leave for the door are noted where they leave the plate
- behaviour-change: a wire's Way row has Show on 902 (its cabinet's page, the wire selected, its way drawn); a wire
  selected in the Connections table shows its way on the panel page; a duct's Wires row has Select them; a selected
  duct shows its fullest piece hatched with its fill chip; a placement's Ways row says how many of its ways no longer
  hold, with Route; the toast of a turn or a nudge that leaves ways no longer holding says what it did first, with Undo
- behaviour-change: a stored way of a wire its cabinet no longer routes (an end re-wired to the field or an external
  point, a wire become a core) is read nowhere, so the wire prints no length or "site" and its duct no longer counts
  it; Route's next run removes it
- behaviour-change: a wire is made on site by one rule in every place that tells it (doc 13 §2.1): with a second
  cabinet (a layout space for +FLD), a wire between the two cabinets stays made on site in the wire list (Length from
  "site"), the package's Contents, the cut list and the consumables, as Route's card says, where they said "no length
  yet: type them"; both its ends are the site's. A wire with an open end or an end on a device without a location is
  unfinished: Route's card names it under Without a way with its reason ("its end at the interruption point L-1 is
  open: …", "its end -103F1:1 is on a device without a location: …"; `ew route --json` `open_end`, `no_location`), no
  longer under Made on site; the lists count it as a wire without a length, and its cable takes no gland for that end
- behaviour-change: Project properties ▸ Workshop sets the holes (doc 12-3d §10.3): Fixing holes (Tapped, Self-tapping
  screws, Through, with a nut), Rail fixing and Duct fixing (Screw, At most apart, From each end), Web between holes and
  Measured from, each shown at its default while unset; a web or a pitch of nothing, a pitch longer than the project's
  longest rail or straight run of duct and a distance from the ends below nothing are refused in words. A changed
  fixing resets the holes moved or left out whose fixing hole no longer stands where it stood, in the same undo step,
  and its card says how many on which rails and ducts before Apply
- api-add: `project.settings.set` sets `workshop.fixing`, `workshop.rail_fixing.screw`, `.pitch` and `.from_end`, the
  same of `workshop.duct_fixing`, `workshop.min_web` and `workshop.datum`; `null` gives each its default
- text-change: a wire without a way says its own reason (the ducts at two pins do not meet, two devices in two doors,
  a device on a second plate or on a named face, a door without a size), and a way that no longer holds its own cause
  (a pin its part or drawing put elsewhere, the duct or the door changed); EW-LAY-011 has these forms; `ew route
  --json` names them `ducts_apart`, `other_plate`, `two_doors` and `door_size`
- text-change: Dutch: "vulgraad" and "maximale vulgraad" for a duct's fill and its limit, "deurlus" for the door loop
- api-add: `ew export package --out <file.zip> [--leave-out <member>]… [--omit-personal-data]` writes the package for
  the workshop (doc 13-package §8): one ZIP with the drawings as one PDF, each workshop list as its Excel export
  (`Lists/`), the label files in the label file's form (`Labels/`) and the drawings for drilling (`Drilling/`), with
  `Contents.txt` and `manifest.json` (`elektrawatt-package/1`: each entry's kind, size and SHA-256). The revision
  decides draft or release: while a revision is open every page of a draft carries "Draft V1.0: not released"; a
  release package with an open error of the release check is refused naming it (exit 2). The same project gives the
  same bytes.
- api-add: `ew_checks::release_check`, the release scheme in release mode, which the release gate runs first and the
  package asks; `ReportKindDef::workshop` marks the lists the package carries; the print set can stamp a draft mark
- behaviour-change: Output ▸ Workshop ▸ Package… (`package.export`) writes the package for the workshop from a dialog
  of two panes: what goes in, each member with its tick, what it holds and its form, the label file's profile with
  Change…, and what a member lacks of part data with Workshop stock…; the release check's findings with Show and their
  fix (Edit part… for a part's missing field), a line per reason for the wires without a length that no finding names,
  Open Messages, the revision (draft or released) and Without personal data; the file with Choose…. It runs as a job
  with its toast; a file of that name is asked about before it is replaced
- behaviour-change: a length in Properties reads as the shortest millimetres with its unit and the project's decimal
  separator, set or unset ("250 mm", "7,5 mm"); a set one read as a bare number with three decimals ("250.000"), as
  the workshop's Cut to, Reserve per end and Door loop did
- text-change: a selection whose records share no field names each kind with its count in the window's language
  ("No common fields: the selection holds 2 symbols, 1 page item"); it listed the kinds' code words
  (`representation, page_item`), and nothing selected reads "0 elementen" in Dutch, not "0 items"
- behaviour-change: the Properties header shows its finding chips as glyphs before the title is cut when the chip
  that says where every value is from ("generated") stands beside them; that chip was left out of the measure
- text-change: every phrase of a Properties chip and the line under a wire's Length and Way rows read in the window's
  language; the older phrases ("its PLC station", "with …", "stored ways", "hole table", "routed", "protected by …",
  a hole's "tap M5", "moved", "left out" and others) stayed English in the Dutch window. The English the window showed
  stays. Where the session's own English differed from the window's, it now reads the window's words:
  `ew query wire_props_of` and the describe text "from cable -101W1, core BN" (was "cable -101W1, core BN") and
  "built-in default: the project has no profile that sets it" (was "built-in default (…)" with the profile's name);
  `ew query device_tag --explain` and the describe text of an earth symbol "an earth symbol: a connection to earth, no
  device, so no tag" (was "an earth connection, no tag"); the describe text alone "the project's marking" (was
  "workshop marking"), "no PLC output yet" (was "no PLC output"), "medium defined at …" (was "medium definition at
  …"), "no medium defined" (was "no medium definition"), "no medium yet: choose one here" (was "no medium yet"), "no
  medium: choose one in the Medium row of its source, the air supply" (was "no medium at its source"), "the project's
  default tube" (was "project default tube") and "no default tube, medium tube or tube part" (was "no tube
  specification"). `ew query tube_props` has words of its own and is unchanged
- api-break: `ew_api::output::Provenance`'s `Inherited::from`, `Derived::via`, `Overridden::note` and `Frozen::note`
  are texts by key (`ew_api::Text`), `Provenance::Told` goes into `Derived`, and `PropRow::note` is a text
- behaviour-change: a wire selected with its every stretch (its row of the Connections table) reads as one wire in
  Properties: its section is headed "Connection (1)", not "Connections (3)", and its Wire number is the wire's, not
  "varies" over the numbers of the stretches it shares with other wires
- behaviour-change: `bond.create` refuses a `via` wire that does not run between the bond's two ends, in words that
  name the two points it joins; such a wire dropped out of the wiring and the wires around it formed anew. A bond a
  saved project holds is read as before
- behaviour-change: the Address row of a PROFIBUS DP port offers Next free address in its menu, by the rule EW-BUS-001's
  quick fix uses (the part's address range, else 1–125), whatever address the port holds; it was offered only as the
  quick fix of an open port's finding
- behaviour-change: Select drags a hole by hand on a panel page: its outline follows the pointer on the grid (free
  with Shift), and the drop moves it in one undo step, its rules running as after typing X and Y; a gland's row dragged
  from Unplaced devices shows the outline of its glands' nuts under the pointer, in the row they are placed in, until
  it is dropped
- api-add: the window's action `tool.place_glands {cables}` (the gland place tool a drag arms), `Session::gland_row`,
  `ew_derive::layout::gland_row`; `PanelTables::features` holds each hole by hand with its centre (`PanelFeature`)
- behaviour-change: in a Dutch project the label files and the drawings for drilling are named in Dutch, in Labels…,
  Drilling…, `ew export labels`, `ew export drilling` and the package ("SAMPLE-E Draadmerkers EW-GEN-MRK-1.xlsx",
  "+CAB Deur boormal.pdf", "+CAB Deur.dxf"); they kept their English names
- text-change: the guide's tutorial scripts wire by pin names (`cursor.set {pin}`), so a moved layout no longer breaks
  them; placing a symbol, drawing a box's corners and a wire's end on the grid stay in millimetres, where the driver
  names no place
- behaviour-change: the sample project has a created and an edited date (2026-11-02, set by its script): its title
  blocks show Creation and Saved, and its package is named and dated by that day, not 1980-01-01
- behaviour-change: a symbol is drawn in Parts (doc 07 §5.6): the symbol editor's screen, its start card and its
  Library folder go. Draw ▸ Place ▸ New symbol (`Ctrl+I`), the symbol picker's foot (Nobody has it? New symbol…) and
  a placed symbol's Draw one like it… open a new drawing at once, for the page in front: its connection grid, What it
  is as you left it (a normally open contact the first time), the device tag's text (and a contact's or a coil's
  cross-reference) where the standard puts them, your drawing grid and Snap. Its next step reads Save and place on
  102 (the Place tool armed with it) or Save and use for -102S1 (the symbol on the page exchanged, its wires kept);
  New symbol's menu has Like the last one, Like the selected one and Go on with the drawing in hand
- behaviour-change: a symbol is saved with your parts, in My parts, its key made from its name; you never choose a
  folder, a function type, a key or a library. Parts gains a Symbols shelf (Lately drawn first, All, In this project,
  My symbols); a symbol that is not yours opens as it is with Make my own from it; a saved one of yours offers New part
  drawn with it…. Save says the first thing To do lists above the drawing
- behaviour-change: a piece of a part names the symbol that draws it (the Symbol cell of Drawn as): Draw a new
  symbol… (the piece's pins standing on the drawing already) and Change it for this part… (your copy of its symbol);
  a page draws the piece with it where the page has it, else with the first symbol of its function
- behaviour-change: a part's box is arranged in Parts (Arrange…, Arrange its box… and Add pin on a placed box): the
  device box editor goes, the pins are the sheet's one list, Done gives the part the box and nothing is saved until
  the part is; Done on a box he changed gives the part its layout (a box nobody arranged keeps none: Place draws it by
  its two corners as before, and saving another field of the part changes none of its boxes), and Save's card names
  each placed box of the part it draws anew (its Apply redraws them in one undo step; Redraw them goes)
- behaviour-change: what the old symbol editor did and where it is done now: What it is offers every starter
  function with pins (also the power supply, terminals and bus ports) and a device box, so a symbol of your own for a
  black-box part's function is drawn there (Ready-made function goes); a symbol's Version, Licence and Source are
  rows of its sheet, in its group Version and licence under its things (closed until opened); Other marks on the Drawing tab sets the cross-reference point at the drawing's centre;
  Discipline is no longer asked: it follows from What it is and the page; a pin stands for the function's next pin
  as it is drawn, and its Function pin row (under More) chooses another (Map to pin goes)
- behaviour-change: a new symbol from a page starts without a name (To do asks for one first), says when What it is
  was taken from the symbol drawn last, names its page's grid, shows the tag a device of it would get (-K?), and its
  strip warns when its grid is not the page's; a piece's symbol is named after the piece and the part, its pane is
  the part's, and Use it for every part with this piece… gives it to your other parts with that piece after a card
  that names them; a symbol of no part lists the parts drawn with it; the box is a pin table with Add a pin and its
  size in steps of a 4 mm page
- behaviour-change: Arrange its box… and the device browser's Arrange… work from a page; Ctrl+S and File ▸ Save on the
  drawing are the strip's Save (a symbol that is not yours is no longer published, a box is finished); Save and place
  keeps a part open in Parts; with no project open a piece's new symbol is given to it
- api-add: `parts.use_for_all {apply?, cancel?}`; `symbol.draw` takes `centre` for the cross-reference point
- api-break: `parts.new_symbol {like?}` and `parts.draw {what?, part?, piece?, from?, done?}` are new, `parts.open`
  takes `symbol` and `parts.find` takes `shelf`; `symbol.new`, `symbol.start`, `symbol.edit`, `symbol.fork`,
  `symbol.copy_edit`, `symbol.replace`, `symbol.save`, `symbol.close`, `symbol.directory`, `symbol.function`,
  `symbol.map_cp`, `symbol.discipline`, `symbol.role`, `part.box`, `part.box_details` and `part.redraw_boxes` go
  (`symbol.prop` sets a pin's discipline and potential; `parts.field symbol.name` and `symbol.what` the symbol's own)
- api-break: the command `box.redraw` goes; `library.update` and `part.edit_copy` take `redraw`, the boxes a part's new
  layout draws anew
- format-variant: a part's function template entry may name its symbol (`function_template[].symbol`, a library
  reference); a project or library without it reads as before
- text-change: Dutch: the drawing's pin tool reads Aansluiting, as a part's pins are aansluitingen
- text-change: a test placement's toast offers Back to the drawing (it said Back to the editor); a symbol whose key the
  library holds for another one is refused with where that one is (Open that symbol from the Symbols shelf), no longer
  asking for a key
- behaviour-change: the pieces table names what a piece is as the starter's symbol of its function names it ("Check
  valve", "Solenoid of a valve, internal coil"), never the function's category entries ("Standard check", "A1A2
  coil"); the sheet's group of pins and clamps reads Connection points (Aansluitpunten), no longer the wires' word
- behaviour-change: Parts finds a part of the project by its order number and by a property of its own, as the
  libraries' search finds a library part (one list of a part's search words)
- behaviour-change: the contents list of a part's sheet offers the groups a part of its kind may have and does not
  show (+ Network ports, + PLC for a device made without them); one added shows, empty
- api-add: `parts.row` takes `group` (`ports`, `plc`), a group the contents list offers
- behaviour-change: an end plate, an end bracket or any accessory a part names that the project has corrected (a
  part of a library of your own changed in the project) is counted as the corrected part: the parts list, the
  assembly list, the checks and the strips on the panel pages read it, so an existing project with such a part now
  counts it and draws its strips as long as the corrected plate and bracket make them; the library's part was counted
- behaviour-change: in a Dutch project the sheets of a drilling template print their words in Dutch (the band, the
  edges to lay it on, the 100 mm bar, each hole's words), and the DXF its datum; they were printed in English
- api-add: `ew_api::output::mm_words` and `mm_with_unit`, the one text of a length in millimetres that Properties shows
- behaviour-change: Import in the bar of Parts reads a file of parts (doc 07 §3.6, §5.9): a table (CSV or Excel), an
  EPLAN parts file (EDZ or parts XML), an outline drawing for the open part, and on the Symbols shelf or the drawing a
  symbol file. A table shows which column is what as the first step, each column mapped to a field of a part by its
  words (Width, Height and Depth to the field of that name the row's kind has; Like to a part a new row starts as); its
  rows stand in the list as This import, each new, changed, the same or refused with its reason in words, a row shown
  beside it as it will be with Ready for; Save n to My parts writes the new and the changed parts. File ▸ Import… goes
  on to Parts for a parts table, an EDZ and a parts XML
- behaviour-change: a table's or an EPLAN file's parts go into My parts, not into the open project: a project takes a
  part when a device uses it, and Undo does not take a write of My parts back. The import card of a parts table (Apply
  put the parts into the project as its own copies, one undo step)
  goes; reading a table of the project tables' form into a project, with its part functions and cores beside it, stays
  the command line's `ew import parts`
- behaviour-change: an EPLAN parts file's dialog (Update n part copies, Add to my parts library) goes: past the terms
  notice the file's parts stand in Parts as a set, Save writes them into My parts, and the update of the open
  project's copies of them is Update them… over the set, with the same card
- behaviour-change: the column mapping card is the EPLAN report's alone; its columns read Column in the file, First
  value, Goes to
- api-break: `parts.import` is Import… of Parts (`path`, `index`, `key`, `discard`, `terms`, `update`); `parts.map`
  maps a column of the set's table as of the report's card; `parts.open` takes `row`, a row of the set shown as it will
  be; `parts.find` takes the scope `import`; `parts.save` and `parts.discard` save and drop the set; `symbol.open` and
  `symbol.candidate` go (Import… on the Symbols shelf, `parts.import {path}` and `parts.import {index}`);
  `file.import_eplan` takes `how` `report`, `map` and `parts` only
- api-break: the parts importer's map members are a part's fields by their declared paths (`Member::Field`), a
  description per language, a technical value, a free property and `like`; the command line's `ew lib import parts`
  refuses a map that names a field beyond the plain ones (maker, part number, type, order number, GTIN, description,
  kind, unit), and its row refusals keep their words
- behaviour-change: a row of a part's accessories or variants left without a value its item needs (a quantity, a
  variant's name) is refused on Save at that cell, in words that name the column and the row ("Name of the variant:
  Row 1 needs a value here"), not with the reader's own message
- behaviour-change: several parts selected in the list of Parts (Ctrl, Shift) show the fields they share on one
  sheet, a value they all have or mixed, and the Connections table where they have the same pins (doc 07 §5.9); a value
  typed goes to all of them, Save saves each into My parts, and the card of the open project's devices names how many
  parts and which fields before Apply, one undo step. A starter part among them is named with Make my own from it; a
  part only in this project is kept in My parts first
- behaviour-change: Export in the bar of Parts writes the selected parts, or the open one, as a table (CSV or Excel)
  that Import reads back: unchanged, every row is the same
- api-add: `parts.open` takes `add` and `range` (Ctrl and Shift in the list); `parts.export {path?, replace?}` is new
- behaviour-change: the workshop stock is a view of Parts (doc 07 §5.10): Project ▸ Edit stock…, the project's
  Properties and the package dialog's Workshop stock… open In this project narrowed to Workshop stock, its parts in
  their order under a heading per kind with its rule, numbered, a row dragged to another place among its kind. The
  sheet of a wire, a ferrule, a marker or a gland says where it stands in the stock with Up, Down and Take out, or Add
  to the stock; each change is one undo step. The stock dialog goes: its rows, Add by number and Apply are the view
  and the sheet's line; its Cut to is the row Cut to of the project's Properties under Workshop
- behaviour-change: Use this stock for new projects keeps the stock, in its order, as a window setting (`shop_stock`);
  a new project from the built-in basic project starts with it, else with the starter's generic stock as before
- behaviour-change: Undo and Redo in Parts bring its list up to date (the stock's order, a part the project holds)
- api-break: `parts.stock {op?, part?, to?}` replaces `workshop.edit_stock`, `workshop.stock_row`,
  `workshop.stock_add`, `workshop.stock_cut` and `workshop.stock_apply`; `parts.find` takes the scope `stock`;
  `project.settings.set` takes the member `workshop.cut_increment`
- text-change: a wire, ferrule or marker named as an accessory is refused with the way to the stock on its sheet in
  Parts, not Project properties
- behaviour-change: a parts table's column mapped to Cable ▸ Number of cores (a column named Cores is mapped already)
  gives a cable that many numbered cores, of the cross-section a column mapped to Cable ▸ Cross-section gives, as the
  Cores form makes them; an empty cell leaves a cable's cores as they are, and a new cable without a count has none.
  Export as a table writes the count and the cross-section of a cable with plain numbered cores and leaves both empty
  for other cores, so a table read back is the same
- behaviour-change: the sheet of several parts reads as the sheet of one: the shared fields under the same group
  headings in the same order, drawn as there, and mixed as the greyed placeholder in the field, which typing replaces
- behaviour-change: on a part's sheet Sold by is a choice of its words (Piece, Metre, Set), a cross-section shows its
  unit (mm²), and a saved part opens with no field in focus (a new part still starts in its first field)
- text-change: a parts table's description columns name their language in the window's language ("Description,
  Dutch"); an import's chips are tinted: new in the accent's tint, refused in the warning's
- behaviour-change: a parts table's row is compared with the part you can read wherever it lives: a starter part, a
  part of another library or a part only in this project that the row repeats unchanged reads "the same" and nothing
  is written; a row that changes one is refused with the way in (Make my own from it, or keep it in My parts first).
  Only a row that names no part you can read is new
- behaviour-change: Save of an import writes the set you looked at: where My parts changed since the table was read,
  nothing is written, the rows are judged anew and a line says so; the column map is saved with the set, not before
- behaviour-change: a changed row of an import names the fields it changes (What was read, and What changes beside
  it with what each holds now and will hold); a Like column is matched by maker and number, and a Like on a row whose
  part exists says it is not used
- behaviour-change: Shift in the list of Parts takes the rows shown between its ends (a folded group's rows are not
  shown); Export after the save dialog's Replace writes the file; a tick box several parts differ in shows mixed
- behaviour-change: an older script's `file.import_eplan {how: update}` or `{how: library}` is refused in words that
  name the way into Parts (`parts.import`, Save, Update them…), not ignored; `parts.find`'s signature names the scope
  `stock`, read from the scopes' one list
- behaviour-change: a part's Technical data offers the named values its kind has (a device's rated current, load
  current, power and voltage, a terminal's colour) as fields to type into, also where the part holds none yet; a part
  whose kind names none and that holds none (a ferrule) has no Technical data group
- behaviour-change: while Parts is shown the status bar no longer shows the page's grid, zoom, pointer and selection
- behaviour-change: New part… while values typed into several parts are not saved asks Save, Discard or Stay; Export
  as a table writes the parts as they are saved and says that what was typed is not in the file
- text-change: Parts counts one part as one ("1 part written to …"), an import's Save reads "<file> saved to My
  parts: 2 new, 0 changed", and Save of several parts counts the parts it wrote
- behaviour-change: in a Dutch project the program's own words that one part of the program says and another prints
  are Dutch wherever they are printed: a panel page's caption under a door and "Deurkant", the assembly list's end
  plate and end bracket, the consumables' set names in For, a tube marker's "… op …", a circuit's class; they were
  printed in English. A text is rendered by the words of the part that owns it, so none falls back to English unseen
- behaviour-change: the package dialog's wire list row and Contents.txt name what the wire list needs of the workshop
  stock and does not find: the ends no ferrule or ring lug of the stock takes, the wires no wire part matches, and a
  field a ferrule, a lug or a wire part lacks (a sleeve length); the label files' row a marker part without its
  characters per line. An existing project's Contents.txt gains these lines; they were printed as blanks unsaid
- text-change: the wire list, the cable overview, the potential overview and the hole table print a size or a length
  as a number under a heading that carries its unit: mm² over `2.5`, Length mm over `400`, From strip length mm and
  To strip length mm over `10`, the cable overview's Length m over `10`, the hole table's X mm and Y mm over `113.5`;
  the Connections and Cables tables show the same cells. Their CSV and Excel files hold these as numbers, so a script
  that read `2.5 mm²` or `400 mm` reads `2.5` or `400`, and the wire list's Length column is now `Length mm`
- behaviour-change: on a page of a Dutch project the size in a wire end's treatment and a hole's diameter print the
  Dutch decimal sign, as every other quantity of the lists does ("huls 1,5 BK", "Ø 3,3"); a twin ferrule's partner
  keeps its wire number as it is ("twin 2,5 BU · 101.6")
- behaviour-change: a Dutch project's lists in Excel are now Dutch, as their pages: the headings, the group and strip
  headings and the program's words in the cells (the list's Excel export, `ew export report --format xlsx` and the
  package's lists alike); numbers stay numbers. Their CSV stays English, and the label files keep their headings in
  both forms. A script that read the English headings of a Dutch project's Excel list reads its CSV instead
- behaviour-change: Export list… (Output ▸ Workshop, and the menu of a list's page) writes the list on show as an Excel
  file in the project's language or, for a `.csv` name, a CSV in English, by the writer of `ew export report` and the
  package; a list could be had as a file only through the package or the command line
- api-add: `lists.export {path?, replace?}`
- behaviour-change: a duct's fill reads in whole percent rounded up, in its chip, Route's card, its Properties and
  EW-LAY-008, so a fill just over the limit reads "61 % · over 60 %", never "60 % · over 60 %"
- behaviour-change: the SQL view's `api_v1_wire.length_nm` and `length_source` hold each wire's length and where it
  comes from as the wire list prints them (typed, cable, routed, stale, missing, site); both were NULL for every wire
