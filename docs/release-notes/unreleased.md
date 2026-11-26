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
- api-add: commands `workshop.stock {stock, copies?, cut_increment?}` and `part.delete {copy}`; `part.assign` takes
  `variant`, `device.place`'s `from_part` and `new_terminal` take `part_variant`, `part.edit_copy` changes a part's
  `accessories` and `variants`, and `fields.set` sets a device's `accessory_choices`
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
