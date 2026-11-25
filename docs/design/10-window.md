# PLC, I/O and fieldbus: the window

A chapter of [PLC, I/O and fieldbus](10-plc-and-bus.md), whose table says where each section lives.

---

## 6. UI and UX touchpoints

| Surface | Behaviour | Release | Requirements |
|---|---|---|---|
| PLC navigator, tree | configuration project → station (role icon) → rack → module (slot) → channel; bus ports under the station; badges for unplaced, `Tbd` and finding count; drag a channel onto a pin or coil to bind | MVP | FR-PLC-020, FR-UI-007 |
| PLC grid (table mode) | the columns of FR-PLC-021; filters unassigned, duplicate, spare, unplaced, safety, per bus, per configuration project, `Tbd`; fill-down, pattern fill (`TA05_V_{n:02}`), Excel paste, each edit one command | MVP Should | FR-PLC-021, FR-UI-008 |
| Property grid | address with provenance: *"Q0.1 · derived: -201U3 output start Q0 (auto, packed, slot 1) + 533361 channel map n → bit 1"*; function text *"TA05-V-15 · process tag of -203Q1, driven by -201U3-Q1"*; override marker and "reset to derived" | MVP | FR-UI-010 |
| Re-address preview | old → new address per channel, affected folios, symbol changes, rendered candidate pages; accept or cancel | MVP Should | FR-PLC-014, FR-UI-022 |
| Message panel | EW-PLC-012 jumps to the coil on 203; quick fixes "rebind to map channel", "set -3X1 termination On", "clean symbol", "re-address module"; waivers need a reason | MVP | FR-UI-021 |
| PLC workspace | PLC navigator, grid, bus navigator, exchange dialog and the fluid pages side by side | 1.0 | FR-UI-003, FR-UI-006 |
| Rack/slot editor | drag modules from the device type into allowed slots; valve-terminal slot list next to the stored order code; live EW-PLC-011 | 1.0 | FR-PLC-028 |
| Bus navigator | logical nets → ports; physical nets → derived segments with ends; termination toggles; rate at the net | 1.0 | FR-UI-006 |
| Exchange dialog | format and version (CAEX 2.15 or 3.0 read; AR APC 1.4.0 or 1.2.0), station scope, document date, personal-data option; import preview tabs: added, removed, changed, conflicts, parking | 1.0 | FR-PLC-029 … -036, FR-EXP-009 |

The PLC navigator's tree is configuration project → station (role word) → rack (only once a module has an authored rack position; a module keeps its row key with or without it) → module → channel, with the station's bus ports (the functions with a `bus_port` of the station and the devices under it) after its modules; a project with a station lists the cards no station takes under "Cards without a CPU", and a station's row menu attaches them with one `assembly.attach`. The grid has 18 columns, data type, signal range, safety (read-only in the cell: "edit in the property grid") and rack among them, the flags spare, unplaced, unassigned, tbd, duplicate and safety and the facets `config:<name>` and `bus:<net>` (the logical net's name in lower case with hyphens for spaces, so that one word types it). A channel row dropped on a CP binds it with one `plc.bind_channel`, which attaches a device beside the card first (`attach_at`).

**The PLC workspace** is the built-in workspace PLC (View ▸ Workspaces): the Schematic layout with the PLC navigator shown in the left column and the **PLC grid** in the bottom dock. The PLC grid is its own navigator kind (`plc_grid`) over the PLC navigator's table, always in table mode, so that the tree and the grid are on show together; Properties stays under the navigator as in every workspace (doc 16a §5.4), not at the right as §2.6 of doc 16 drew it. A channel's Function text cell is the function text of the device wired to it (§3.4): every gesture of the grid (typing, paste, fill down, pattern fill, clear) sets it there, one `fields.set` per gesture as for any cell, and a cell of a channel nothing is wired to is refused in the station view's words.

**The station's view.** A station opens as **a view in a canvas tab**
beside the page tabs (`area::StationTab` on a `Tab`; the tab keeps the page it was opened from, shows no canvas, and
`ShellModel::canvas` is `None` there, so no tool draws on a page that is not on show). It opens from Rack… (the PLC
tab, a station's menu), a double click on a station in the PLC panel, placing a station's part, and New station…; a
jump to a page from the view (a link, Go to page, Draw on a page…) shows the page in a page tab and leaves the
station's tab in the strip, and `Esc` in the view goes back to the page. One tab per station; a tab whose station is
gone (deleted, its making undone) closes. The view shows, from `Session::rack_view` (cached per revision and station):

- **the rack drawn**, one card per place left to right: the head (`RackView::head`, the station itself; a station
  that is its own module has no second row), each slot with its module (what it gives, its tag, its part number, the
  folio it is drawn on or "not drawn") or free as an outline, and a valve terminal's positions (`RackView::cells`,
  `terminal_cells`), a valve with its part. Free slots stand only as far as the station's part takes modules: a part
  whose limits say no module, or a valve terminal whose part maps its valve positions and names no module limit, has
  no slot, its positions follow the head, and the catalogue greys the cards with "This station takes no modules";
- **the same places as a table**: slot, module, part (number and description; the part that describes the module for
  the PLC, else its first part; a module without a part says No part with Choose part…), gives, addresses, drawn on
  (a link to the page, else Draw on a page…);
- **the channels** of the module under the cursor (`Session::module_channels`: address, symbol name, the device
  wired to it as a link to its drawing); a click on a Symbol or Function text cell opens it (`plc.rack_channel`), and
  the cell keeps the keyboard cursor, ringed as a table's cursor cell, when its field closes: F2 opens the cell the
  cursor stands on and the arrows walk the cells, as in the window's other tables, until a place chosen in the rack
  takes the keys back;
- **To do** (`ShellModel::station_todo`), the first of the lists under the rack, clear of the toasts at their bottom
  right (doc 16a §6.10): EW-PLC-011's findings on the station with the error icon and word and Show,
  a module without a part (Choose part…), then each side of the station on no page of its kind: the devices no
  electrical page draws (`Session::off_electrical_pages`; a drawing on a bus page or as positions on a fluid page does
  not count), a station with bus interfaces on no bus page, a valve terminal on no fluid page; the electrical side's
  button draws each card on a page of its own, and All on one page… beside it lists the schematic pages and New page…
  ([10](10-plc-data.md) §2.3); each other button acts on the page the view stands on when it is of that kind, on a new
  page when the project has none, else lists the pages of the kind and New page…; a valve terminal without an overview page (Make the overview page, `plc.rack_next
  {step: overview}`). An overview page belongs with the terminal's fluid diagram, as the reference set's 201 and 202
  share their section with its fluid pages 203–206: the first one follows the terminal's first fluid page (the
  first, by section and folio, that draws the terminal or a device under it) in that page's section, at the folio doc
  06 §5.6 proposes after it (202 after 201), never in the wiring section where the terminal's box or bus port stands;
  a terminal no fluid page draws yet follows its own page. Further pages follow the last overview page (doc
  11 §5.5);
- **the header**: the station's tag changes in place (a click, or F2 on the head; F2 or a click on the tag of the
  card under the cursor for a card), through the Properties Tag row's path (`plc.rack_rename`); a tag another device
  has is refused under the rack; the PLC's name at its right likewise (`plc.rack_rename {plc}`, `fields.set name` of the
  configuration project; a name another PLC has is refused);
- **the catalogue** at the right: the place catalogue's parts (`CataloguePart::plc`, the device browser's list and
  its filter matching) that fit the station, by group (`PlcGroup`), the parts that do not fit where the cursor stands
  greyed with the reason; New part… at its foot. For a station without its head New part… starts as the head (a
  station part with the kind's bus row), and the row goes when the role is made Module while it is as the preset made
  it; saved, the part stands in the catalogue, and it is the station's head only when the empty head was the place
  chosen. A library PLC part without a function template, which the place
  catalogue does not offer because nothing draws it (`PlaceCatalogue::plc_without_functions`), is listed greyed too,
  with what to do in the part editor (a pin pattern or supply pins give it its box); taking it is refused in those words.

Side by side, a view in the group without the focus shows the same (`ShellModel::station_shown_of`, one kept per group)
but the view's bar, which acts on the window's selection, and leaves out the catalogue where its pane is too narrow for
the table's columns and the catalogue both; what is done in it gives it the focus.

**The cursor** (`StationCursor`: the head, a slot, a valve position) is set by a click on a card or a row or by the
arrow keys (`plc.rack_select`), and the device under it is the window's selection: Properties shows it, and the
view's own bar above the rack acts on it (`plc.module_replace`, `plc.module_remove`, `plc.module_draw`,
`plc.module_move`, `plc.module_goto`, `plc.module_left`, `plc.module_right`, each `plc.rack_slot`'s operation or
`nav.show`; declared actions of the view's own controls); a right click gives the same commands, drawn as the window's
other menus (Replace, Draw on a page and Move to page a list beside their entry, the part the module has checked),
Delete empties the slot. *Move to page* puts a drawn module's or head's boxes on electrical pages on the
schematic page chosen where they stand, with one `rep.move` (one undo step): the device, its channels, bindings and
slot stay, and a wire between its pins and something that stays behind is kept as a pair of interruption points,
which a preview lists before anything moves (doc 06 §3.4's cross-page move); a cut and its
paste bring a module with a set slot, start, part use or integrated I/O back as itself, while a card without settings
goes with its cut and its paste makes a new one (doc 06 §3.4, §3.6). Its entries read as moves ("To page 101 …", "To a new page") and
leave out the page the module is drawn on; To a new page makes the page after the module's own in its section, its
folio the one New page proposes after it, with that page's type, frame, structure and grid, and moves
the boxes there in the same undo step (`rep.move_to_new_page`), the toast naming the page. On a valve
position they speak of the valve (Replace valve, Empty the position), and a move or a drawing of its own, which a
position does not take, is greyed with the reason; on a free place each says to take a part from the catalogue. They
are no buttons of the PLC tab: with six more the tab folds at 1920 × 1080 in Dutch and in the long-text locale (doc
16a §5.3, law 3). On a free place the station is selected. A double click on a catalogue part puts
it into the cursor's free slot, else the first free one, a drag onto the place it is let go over, onto a module
replacing its part (`plc.rack_add`); a card dragged to a free slot moves there and onto another module changes places
with it (`plc.rack_move`: `fields.set plc_module.slot`, `fields.set_many`, or `assembly.attach` for two modules
without slot numbers of their own). A refusal stays in the view at the place it was refused at. Every step is a
declared command and one undo step. **New station…** is the same view on a `StationDraft` (what it is, its PLC, its
head and its modules from the catalogue); nothing is written before Create (`plc.add_station`), and closing the tab
of a draft with something chosen asks. Each double click fills the next slot.

**Adding a module.** A part put into a slot of the station's view is `plc.add_module {station, part, rack?, slot?, copies?}` (a child of the station at the position after its last child, with the station's letter and the child tag scheme, an authored `rack.slot` or positional; a library part embedded in the same step); a module's slot is `fields.set plc_module.slot`. EW-PLC-011's prevent half refuses a slot another module holds (a positional module's number is a slot of rack 0), a slot outside the part's `allowed_slots` and a station beyond its limits.

**The bus navigator** (rail entry Bus, beside the PLC navigator in the PLC workspace): Logical nets → each `LogicalNet` (its name and rate, `1.5 Mbit/s`) → its ports (`net_members`, the master port badged `master`); Physical nets → each `PhysicalNet` → its segments (`bus_segments`, A5) → their members in walk order (a port badged `end` at an end and `terminated` or `not terminated`, an external interruption point badged `external`, a cable by its tag); then Ports without a net. A port row is a table row (device, interface, address, termination, bus, role, nets); its address cell and the Properties row take the address as the reports print it (`3`, `cpx-1 192.168.0.10` with `mask` and `gateway`, `pos 2 alias 1001`, `port 3`, `TBD`, empty for none), its termination cell on or off; a net's row selects the net, whose rate and master Properties shows (Properties names a port's nets by their names and its address in those words). **A part's ports not drawn yet**: a station's row offers Place port `<interface>` for each port of its part that no function carries and that a starter symbol draws (the DP port for PROFIBUS DP), arming the Place tool joined to the station; a bus port FnDef joins a device as an auxiliary function, as a contact does, and N5 types the port from the part. **Place here**: the PLC navigator's row of a station or a card (a channel's row for its card) a side of which no page draws yet, and the card's menu in the station view, offer Place here (`plc.module_place`): on an electrical page its box on the pointer (the black box tool's `plc.draw_station` of one box, laid out by the part's black-box entry), on a bus page its bus ports, on a fluid page a valve terminal's positions; on a page of another kind it is greyed with the page to open. **The Wire tool on a bus-topology page** draws `Bus` connections; autoconnect stays off there, as on fluid pages.

**The re-address preview**: a module's row offers Re-address… and Make manual addresses automatic…, a configuration project's row those and Freeze addresses…; each opens the command's bulk card in the Preview tab, whose first list, Changes, is the command's notices (`channel 3 of -201U5: Q10.3 → Q2.3`, the old address in secondary text, the new one in primary), and Apply commits it as one undo step. The notices are the command's English words, as `ew` prints them; the affected folios and the rendered candidate pages of §6's table are not built. A channel's data type and signal range are cells of the PLC grid. Properties shows the symbol policy and a module's safety data as one row each, a group that cannot be typed into, and a channel row selects no record, so each has a form of its row: a configuration project's Symbol policy… (template, tag table, strict IEC identifiers: one `fields.set` of `symbols`), a module's Safety data… (F-source and F-destination address, F-watchdog time: one `fields.set` of `plc_module.safety`, the range kept; all empty makes the module a standard one) and a channel's Mode and safety… (the port's mode, the safety evaluation and requirement: one `fields.set_many` of `mode` and `safety`, the pair and test-pulse pin kept). Each starts with the record's values (`Session::plc_settings`); a number that is no `u16` is refused in words in the form. Project ▸ Process-tag scheme… is a form of the scheme (`project.settings.set process_tag_scheme`, empty removes it) and Project ▸ Channel addresses at terminals a switch of the drawing setting `plc_address_at_terminals`.

**The exchange dialog** is two entries of the Output menu. Export to PLC software… is a form (format and version: AML AR APC 1.4.0 or 1.2.0, the TIA tag workbook, PLCopen XML; the stations by tag, empty for all; the document date, empty for the model's), then the file: `Session::plc_export`, the file written through a temporary file renamed over the target, then `plc.record_exchange`; a toast counts the stations, modules, channels and tags and adds the export's warnings. The personal-data option is not a check box: the writer puts no person field in any file (§4.3), so the window offers no choice that would do nothing (the guide says the file holds none). Import from PLC software… reads the file (the AML reader's size limit) into `Dialog::PlcImport`: tabs Changed, Conflicts, Removed and Parking, each with its count (Added is folded into Parking, whose creatable objects are created unplaced), a conflict's choice between the project's and the file's value, a parked module's or station's Create; Apply merges the file again under the choices and commits `plc.apply_exchange`, one undo step. Reading CAEX 3.0 needs no choice (the reader takes both). After an applied import the configuration project's row offers Unlock module starts, Unlock symbols and Unlock PLC data for each group the import protects (`Session::plc_protected_groups`; one `plc.unlock` each), and Import instrument list… asks for a CSV or workbook (`Session::instrument_list`, which takes the configuration project by name or ID) and previews `plc.apply_instrument_list` in the Preview tab, its reservations and open items the card's changes. Not built: the affected folios.

- **Persona and journey.** The PLC grid is the main surface of persona P3 and of journey J4
  ([vision](00-vision-and-scope.md) §5, §6).
- **Accessibility.** The PLC grid and navigator are also the accessible representation of folios 203–206
  (FR-UI-026). Safety and state styles combine an icon or pattern with colour (FR-UI-025).
- **Keyboard and vocabulary.** Every action is in the command palette, and EPLAN terms work as aliases
  ("configuration project", "PLC box", "readdress"; NFR-UX-03).
- **Testing.** The usability-test protocol of [16](16-ux-ui.md) includes the J4 tasks at 1.0.

