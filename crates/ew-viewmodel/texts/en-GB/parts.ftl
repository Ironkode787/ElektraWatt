# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: parts, part cards, libraries and accessories. One message per key; `{ $p0 }`,
# `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback of every other.

# F22: a part's data at a glance.
card-type = Type
card-order-number = Order number
card-size = Size
card-gives = Gives
card-from = From
card-interface-ports = { $p0 }, { $p1 } ports
card-cps = { $p0 ->
        [one] 1 connection point
       *[other] { $p0 } connection points
    }
card-project-only = This project only
card-library-version = { $p0 }, version { $p1 }
card-source-checked = { $p0 } · checked
card-source-unchecked = { $p0 } · not checked
card-no-size = No size given
card-size-beside-rail = Drawn to scale beside a 35 mm rail
# F23: the part editor's Size and mounting.
part-size-hint-dimensions = As seen from the front when mounted: the layout draws the part this size.
part-size-hint-mount = How it is fixed: on a rail, screwed to the plate, in a door cut-out, or not mounted.
part-size-hint-mass = Its weight, for the panel's total.
part-size-hint-clearance = The room the layout keeps free around the part, for heat or wiring.
part-size-mount-none = Not given
accessory-at-terminal = terminal { $p1 } of { $p0 }
origin-eplan = EPLAN
origin-qet = QElectroTech
origin-kicad = KiCad
origin-other = Another tool
card-line = { $p0 } { $p1 } · { $p2 }
card-apply = Apply
card-cancel = Cancel
card-failed = failed: { $p0 }

# cards, badges, empty states and outcome badges (doc 16a §6.16, §6.17, §6.23).
card-strip-failed = { $p0 } failed: { $p1 }
card-as-of = as of rev { $p0 }
card-title-reorder = Reorder pages
card-title-fix = Quick fix { $p0 }
card-title-find = Replace
card-title-properties = Preview of { $p0 }
parts-map-title = Map columns
parts-map-source = Source
parts-map-saved = Saved map
parts-map-column = Column in the file
parts-map-sample = First value
parts-map-member = Goes to
parts-map-none = Not imported
parts-map-import = Import
parts-map-cancel = Cancel
parts-map-twice = `{ $p0 }` is chosen for two columns
parts-map-twice-text = { $p0 } is chosen for two columns
parts-map-missing = Map a column to { $p0 }
parts-map-mixed = Map one description, or one per language, not both
parts-member-description-lang = { $p0 }, { $p1 }
parts-member-in-group = { $p0 } ▸ { $p1 }
parts-member-like = Like (the number of a part it starts as)
parts-member-tech-data = Technical data: { $p0 }
parts-member-free-prop = Free property: { $p0 }
card-table-drop = Leave out
card-reorder = { $p0 } · { $p1 }
card-failed-rule = Rule { $p0 }

# The Libraries navigator's copy detail
library-copy-kind-key = { $p0 } · { $p1 }
library-copy-not-installed = no installed library
part-choose-fits = Only parts that fit
part-choose-view-all = All
part-choose-view-project = In this project
part-choose-view-favourites = Favourites
part-choose-view-recent = Recent
part-choose-inactive = Show deprecated and withdrawn
part-choose-empty = No part matches “{ $p0 }”
part-choose-nothing = No part to choose; change the search.
part-edit-fork-note = Saving creates a project copy of this part; the library is not changed

# The part card in the Preview tab: the field diff, the devices and the parts list rows.
part-card-field = { $p0 }: { $p1 } → { $p2 }
part-card-devices = Used by { $p0 }
part-card-removed = Parts list row removed: { $p0 }
part-card-added = Parts list row added: { $p0 }
part-card-changed = Parts list row changed: { $p0 }
part-card-kept = Kept in My parts with it, their uses in this project moved to them: { $p0 }.
part-card-lost = A bus line ends at a socket the part no longer has: { $p0 }. It keeps its end there; draw it to another socket.
part-card-box = Drawn anew: { $p0 } on page { $p1 }, its pins as the part has them now
part-card-box-moved = Drawn anew: { $p0 } on page { $p1 }, moved back inside the drawing area
part-card-symbol = Drawn anew: { $p0 } on page { $p1 }, its bus symbol's sockets as the part's ports give them now
part-card-symbol-added = Drawn anew: { $p0 } on page { $p1 }, its bus symbol with { $p2 } added
part-card-own-symbol-lacks = Your bus symbol of { $p0 } does not draw { $p1 }: draw it there, or use the program's symbol again
part-card-own-symbol-extra = Your bus symbol of { $p0 } draws { $p1 }, which the network ports no longer have
part-card-symbol-removed = Drawn anew: { $p0 } on page { $p1 }, its bus symbol without { $p2 }
part-change-interface = Interface { $p0 }
part-change-ports = { $p0 ->
        [one] 1 port
       *[other] { $p0 } ports
    }
part-change-other = Other part data
part-change-other-changed = changed
part-field-functions = Functions
part-field-cps = Connection points
part-field-source = Source
part-choose-search = Search parts
part-choose-list = Parts
part-choose-favourite = Favourite
part-choose-detail = { $p0 } · version { $p1 } · { $p2 }
part-choose-detail-successor = { $p0 } · version { $p1 } · { $p2 }, successor { $p3 }
part-choose-detail-functions = { $p0 } · functions { $p1 }
part-choose-target-bridge = the bridge { $p0 }
part-choose-target-accessory = the { $p0 } { $p1 }
part-choose-target-devices = { $p0 } devices
part-none = none
part-form-message = { $p0 }: { $p1 }
part-placeholder-description = What it is
part-placeholder-note = The parts list names it TBD with these words; choose the real part for one of its devices later and every device that has it takes that part.
part-placeholder-use = Use as placeholder
part-placeholder-empty = Type what the part is first
part-hint-ports = One row per interface of the data sheet. Controller or master leads the bus (a CPU); device or slave is a node with an address (a head station, a drive, a managed switch); passive has no address and passes the net through (a bus plug, an unmanaged switch). Ports: the number of sockets of one interface, such as 2 or 8; the interface keeps one address for all of them.
part-bus-interface = Interface
part-bus-port_label = Port label
part-bus-bus = Bus
part-bus-role = Role
part-bus-ports = Ports
part-bus-termination = Termination switch
part-bus-termination-none = None
part-bus-add = Add interface
part-bus-remove = Remove interface { $p0 }
part-bus-none = No bus interface yet: add one for a device on a bus.
part-drawn-with = Drawn with
stock-summary = Workshop stock: { $p0 } parts
parts-stock-chip = Workshop stock
parts-stock-chip-hint = Shows the workshop stock of this project in its order. It is changed here: drag a row to another place, or open a part and use Up, Down, Take out or Add to the stock.
parts-stock-count = { $p0 } in the stock of this project
# A heading of the Workshop stock view: the kind's plural, then the rule by which a part of it is taken.
parts-stock-group = { $p0 } · { $p1 }
parts-stock-rule-wire = the first of its size and colour
parts-stock-rule-ferrule = the first that fits
parts-stock-rule-marker = the first made for it
parts-stock-rule-gland = the first that seals
parts-stock-in = In this project's workshop stock: number { $p0 } of the { $p1 } of its kind, taken before those after it.
parts-stock-out = Not in this project's workshop stock.
parts-stock-add = Add to the stock
parts-stock-up = Up
parts-stock-down = Down
parts-stock-first = It is the first of its kind already.
parts-stock-last = It is the last of its kind already.
parts-stock-take-out = Take out
parts-stock-title = The workshop stock of this project
parts-stock-text = The wire, ferrule, marker and gland parts the shop keeps, in the order it prefers them. Drag a row in the list to another place to change the order.
parts-stock-add-mine = Add from my parts…
parts-stock-default = Use this stock for new projects
parts-stock-cut = Cut to (the length a piece is rounded up to) is a setting of the project: Project properties ▸ Workshop.
parts-stock-kept = { $p0 ->
    [one] This stock, { $p0 } part, is kept for new projects.
   *[other] This stock, { $p0 } parts in its order, is kept for new projects.
    }
part-deleted = { $p0 } deleted. Undo brings it back.
part-removed-own = Removed { $p0 } from your parts library: the next projects no longer offer it.
part-removed-own-kept = Removed { $p0 } from your parts library: the next projects no longer offer it. This project keeps its copy for { $p1 }.
part-hint-plc = What the part is in the PLC: a station (a CPU, a head station such as an IM 152, a valve terminal) has a rack; a module sits in a slot of a station's rack. A station whose bus interfaces lead the bus is a CPU; one that only answers on the bus is a head station.
part-plc-role = PLC role
part-plc-role-none = None
part-plc-kind = Channel kind
part-plc-kind-none = None
part-plc-count = Channels
part-plc-kind2 = Second channel kind
part-plc-count2 = Its channels
part-plc-data_type = Data type
part-plc-designation = Channel name
part-plc-pin_of = Pin pattern
part-plc-pin_of-hint = The pin of channel n: {"{"}n+1{"}"} gives pins 1, 2, 3 …; X2:{"{"}n+1{"}"} gives X2:1, X2:2 …
part-plc-l_plus = Supply L+ pin
part-plc-m = Supply M pin
part-plc-slots = Allowed slots
part-plc-slots-hint = The slots the module may stand in, as numbers and ranges, such as 1-8, 10; empty: any slot. Most modules is how many modules a station takes.
part-plc-supply-hint = A list gives a pin of each, by commas or spaces, such as 1L+, 2L+; one M pin serves every L+ pin.
part-plc-supply-pins = Supply pins: { $p0 }
part-plc-max_modules = Most modules
part-plc-pins = Channel { $p0 } → pin { $p1 } … channel { $p2 } → pin { $p3 }
part-plc-pins-none = Pins: once the count is given
part-plc-io-in = Occupies { $p0 } input bits
part-plc-io-out = Occupies { $p0 } output bits
part-plc-io-none = Bits: once the kind and count are given
card-paste-versions-title = The pasted records use other part versions
library-state-tip-current = The project's copy is the library's version.
library-state-tip-update_available = The library has a newer version of this item. Update… shows the changes first; Apply makes them.
library-state-tip-deprecated = The library no longer offers this item; where it names a successor, use that instead.
library-state-tip-source_missing = This item exists in this project only: no installed library holds it, such as an imported frame or a part made in the project. That is not a fault, but there is nothing to update it from.
library-state-tip-local_fork = The project's copy was changed here and is kept apart from its library item; no update replaces it.
libraries-update-count = { $p0 ->
        [one] Update 1 item…
       *[other] Update { $p0 } items…
    }
libraries-update-tip = Shows the changes in the Preview first; Apply makes them.
part-pins-and-box-how = Where the pins stand on the box the device is drawn as
part-pins = Pins
part-pin-add = Add pin
part-pin-name = Name of pin { $p0 }
part-pin-role = Potential of pin { $p0 }
part-pin-remove = Remove pin { $p0 }
part-save-another = Save and add another size
part-cable-cores = Number of cores
part-cable-csa = Cross-section (mm²)
part-cable-pe = With protective conductor (green-yellow)
part-cable-marking = Cores marked by
part-cable-marking-hd308 = Colours (HD 308)
part-cable-marking-numbers = Numbers
part-cable-marking-own = My own
part-cable-core = Core
part-cable-colour = Colour
part-cable-row = { $p0 } of core { $p1 }
part-cable-shield = Shielded
part-cable-diameter = Outer diameter (mm)
part-cable-reads = Reads as
part-bus-part-name = Name
part-cable-bus = Made for the bus
part-cable-bus-none = No bus
part-plug-outlet = Cable outlet
part-plug-straight = straight cable outlet
part-plug-angled = angled cable outlet
part-plug-outlet-straight = Straight
part-plug-outlet-angled = Angled (90°)
part-plug-socket = With programming socket
part-plug-resistor = With terminating resistor (switchable)
part-plug-description = { $p0 } plug
part-plug-with-socket = programming socket
part-plug-with-resistor = switchable terminating resistor
part-section-name = { $p0 } section
part-hint-accessories = What this part brings with it. Per piece: for each terminal or device of this part. Per strip: once for each strip whose terminals use this part, after the last of them. Per rail group: the end brackets of a run of strips on one rail, one before it and one after each strip.
part-hint-variants = The colours or versions of this part that a device chooses in Properties. Each gives its own part number, order number or description, so that a list tells them apart.
part-hint-connections = The clamps of the pins: the wires each takes. A clamp without a range takes any wire.
part-hint-cabinet = The front as the panel page draws it, and the holes a door or a plate gets for the part: X and Y are a hole's centre, from the front's bottom-left corner.
part-hint-gland = The gland's thread and the cables it seals: a cable whose outer diameter lies between them takes this gland when it is the first such in the workshop stock.
part-front-outline = Outline
part-front-outline-none = None: the shape is drawn
part-front-outline-lines = { $p0 ->
    [one] imported from a DXF drawing, 1 line
   *[other] imported from a DXF drawing, { $p0 } lines
    }
part-front-import = Import outline…
part-front-remove = Remove
part-front-preview = Preview of the front and its holes
part-table-add-accessories = Add accessory
part-table-add-variants = Add variant
part-table-add-mech-holes = Add hole
part-table-add-sections-enclosure-doors = Add door
part-table-add-configured-components = Add component part
part-table-add-sections-fluid-ports = Add port
part-table-remove = Remove row { $p0 }
part-table-empty = None yet

## Parts, the one place for parts (doc 07 §5)

parts-tab-part = Part
parts-tab-drawing = Drawing
parts-title = Parts
parts-new = New part
parts-like-this = Like this
parts-own = Make my own
parts-own-from = Make my own from it
parts-keep = Keep in my parts
parts-remove = Remove
parts-save = Save
parts-save-keep = Keep in my parts and save
parts-save-use = Save and use for { $p0 }
parts-discard = Discard
parts-back = Back
parts-back-without-choosing = Back without choosing
parts-back-without-it = Back without it
parts-to-the-part = To the part
parts-done = Done
parts-save-place = Save and place on { $p0 }
parts-strip-symbol-page = A new symbol for page { $p0 }. Saved, it is placed there.
parts-strip-symbol-like = A symbol like { $p0 }'s, for page { $p1 }. Saved, it takes the place of { $p0 }'s.
parts-strip-symbol-piece = You are changing the symbol of the piece { $p0 } of { $p1 }. The symbol it came from stays as it is; yours is kept with your parts and draws this piece only.
parts-strip-bus-symbol = You are drawing your own bus symbol for { $p0 }. It draws every device of the part on a bus page; parts without one keep the program's.
parts-strip-box = Arranging the box of { $p0 }. Drag a pin along its edge or onto another edge; drag an edge to size the box.
parts-symbol-lock-starter = A starter symbol. It comes with the program and is the same on every PC, so it cannot be changed here.
parts-symbol-lock-library = A symbol of { $p0 }, which you cannot write, so it cannot be changed here.
parts-symbol-lock-project = A symbol only in this project, from no library you have, so it cannot be changed here.
reason-no-drawing-in-hand = No drawing is in hand.
reason-no-symbol-saved = You have saved no symbol yet.
reason-no-symbol-selected = Select a symbol on the page first.
reason-piece-no-symbol = No symbol draws this piece yet: draw a new one.
reason-symbol-is-yours = The symbol is yours already.
parts-strip-choosing = You are choosing a part for { $p0 }.
parts-strip-form = A part for { $p0 }.
parts-back-to-messages = Back to Messages
parts-strip-finding = Opened from a finding.
parts-strip-finding-page = Opened from a finding on page { $p0 }.
parts-new-title = New part
parts-what-is-it = What is it? The answer decides what the sheet asks for, and nothing else is asked.
parts-start-from = Or start from
parts-like-card = A part like it…
parts-like-card-line = Pick a part in the list: yours opens with everything but its number.
parts-like-which = Open the part to start from in the list: the new part has everything it has but its number.
parts-none-open = Open a part from the list, or make a new one.
parts-search-hint = Search number, maker, description, what it is for
parts-search-name = Search parts
parts-scope-all = All
parts-scope-project = In this project
parts-scope-mine = My parts
parts-scope-my-symbols = My symbols
parts-shelf-parts = Parts
parts-shelf-symbols = Symbols
parts-group-drawn-lately = Lately drawn
parts-group-symbols = Symbols
parts-symbols-count = { $p0 ->
    [one] { $p0 } symbol
   *[other] { $p0 } symbols
    }
parts-new-symbol = New symbol
parts-piece-change = Change it for this part…
parts-piece-draw-new = Draw a new symbol…
parts-arrange = Arrange…
parts-drawn-as-head = Drawn as
parts-piece-mine-not-saved = mine, not saved
parts-use-for-all = Use it for every part with this piece…
parts-use-for-all-none = No other part of yours has this piece.
parts-use-for-all-card = It draws this piece of: { $p0 }.
parts-use-for-all-held = { $p0 ->
    [one] This project holds { $p0 } of them: its copy follows, in one undo step.
   *[other] This project holds { $p0 } of them: their copies follow, in one undo step.
    }
parts-apply = Apply
parts-cancel = Cancel
parts-drawn-with = Parts drawn with it
parts-drawn-with-none = None yet.
reason-no-piece-drawing = Draw a piece's symbol first: Draw a new symbol… or Change it for this part… in its Symbol cell.
toast-symbol-used-for = The symbol now draws this piece of { $p0 } other { $p0 ->
    [one] part
   *[other] parts
    }.
parts-strip-other-grid = This symbol is on { $p0 }; page { $p1 } is on { $p2 }: it is placed as a copy drawn on the page's grid.
parts-piece-symbol-name = { $p0 } of { $p1 }
parts-bus-symbol-name = { $p0 } · bus symbol
parts-add-group = + { $p0 }
parts-new-part-with-it = New part drawn with it…
parts-filter-kind = Kind
parts-filter-kind-all = Every kind
parts-filter-lacking = Lacks something
parts-count = { $p0 ->
    [one] { $p0 } part
   *[other] { $p0 } parts
    }
parts-used-by = { $p0 ->
    [one] { $p0 } more uses it
   *[other] { $p0 } more use it
    }
parts-uses-it = { $p0 } uses it
parts-changed = Changed, not saved
parts-lives-mine = Mine · in My parts
parts-lives-starter = Starter part
parts-lives-here = Only in this project
parts-lives-mine-too = { $p0 } · also in My parts
parts-names-missing = It names a part that no library you have holds and this project does not hold ({ $p0 }): keep that part in My parts from the project that holds it, or name another.
parts-lives-new = New, not saved
parts-lock-starter = A starter part. It comes with the program and is the same on every PC, so it cannot be changed here.
parts-lock-library = A part of a library you cannot write, so it cannot be changed here.
parts-ready-for = Ready for
parts-ready-all = Nothing a feature reads is missing.
parts-schematic-picture = How the part is drawn on a schematic page
parts-no-schematic = No symbol draws it yet.
parts-not-drawn = Not drawn on a schematic page: it is chosen where it is used.
parts-on-a-bus-page = On a bus page
parts-bus-symbol-picture = How the part is drawn on a bus page
parts-bus-symbol-lacks = Bus symbol: { $p0 } is not drawn
parts-bus-symbol-extra = Bus symbol: { $p0 } is drawn, which the network ports no longer have
parts-make-own-bus-symbol = Make my own from it…
parts-use-program-bus-symbol = Use the program's symbol again
parts-no-front = Give it a width and a height to see its front.
parts-saved-mine = { $p0 } is saved in My parts.
parts-saved-kept = Kept in My parts with it: { $p0 }.
parts-saved-project-refused = My parts holds { $p0 } now, but this project stays on its own copies: { $p1 }. Keep in my parts brings the project onto My parts' copies.
reason-no-parts-place = Parts is not open.
reason-no-part-open = No part is open in Parts.
reason-part-not-yours = The part is not yours: make your own from it.
reason-part-no-bus-ports = The part has no network ports: add them under Network ports first.
reason-part-not-changed = Nothing was changed since the part was opened or saved.
reason-part-own-already = The part is yours already: Like this makes another like it.
reason-parts-unsaved = The part has changes that are not saved: Save or Discard them first.
reason-parts-card-waits = What Save changes in this project waits on the card beside the part: apply or cancel it first.
reason-part-remove-used = { $p0 ->
    [one] { $p0 } device of this project uses the part.
   *[other] { $p0 } devices of this project use the part.
    }

## The sheet of Parts (doc 16a §6.27): a group's one line beside its heading, and the room kept free round a part
part-line-schematic = How the pages draw it: its pieces or its box, and its pins
part-line-cabinet = Its size, how it mounts, its front and its holes
part-line-connections = Pins with the same clamp stand on one row; open a row to change one pin
part-line-ports = One row per interface of the data sheet
part-line-plc = What it is in the PLC: a station with a rack, or a module in a slot
part-line-technical = The values the program computes with
part-line-goes-with = What it brings with it, and the forms it is sold in
parts-free = Keep free
parts-free-top = above
parts-free-bottom = below
parts-free-left = left
parts-free-right = right
parts-count-line = { $p0 } · { $p1 } mine · { $p2 } in this project
# The count line of Parts opened with no project.
parts-count-line-alone = { $p0 } · { $p1 } mine
parts-group-recent = Lately opened
parts-chip-mine = mine
parts-chip-starter = starter
parts-chip-here = only this project
parts-filter-maker = Maker
parts-filter-maker-all = Every maker
parts-lock-read-only = This project is open read-only: its parts are shown here, and none is changed.

## On the schematic as tables (doc 07 §5.4): a part's pieces and a terminal's levels
parts-pieces-none = No piece yet: add what the part is drawn as, a coil, a contact, a lamp, each with its pins.
parts-piece-what = What it is
parts-piece-pins = Pins
parts-piece-symbol = Symbol
parts-piece-choose = Choose what it is
parts-piece-what-of = What piece { $p0 } is
parts-piece-what-type = Type to narrow the list
parts-piece-pins-of = Pins of piece { $p0 }
parts-piece-symbol-of = Symbol of piece { $p0 }
parts-piece-starter = { $p0 } · starter
parts-piece-add = Add a piece
parts-levels = Levels
parts-levels-n = { $p0 } levels
parts-level = Level
parts-level-left = Left clamp
parts-level-right = Right clamp
parts-level-joined = Joined inside
parts-level-left-of = Left clamp of level { $p0 }
parts-level-right-of = Right clamp of level { $p0 }
parts-levels-note = Each level passes through on its own. The terminal diagram prints a row per level.
parts-filter-maker-name = Filter by maker
parts-strip-in-passing = A new part for { $p0 }, its { $p1 }.
parts-new-plate = New plate…
parts-new-accessory = New part…
parts-size-mm = { $p0 } mm
parts-size-mount-depth = { $p0 } · { $p1 } mm deep
parts-size-depth = { $p0 } mm deep
parts-connections-pins = Pins
parts-connections-open = Open the row of pins { $p0 }
parts-connections-close = Close the row of pins { $p0 }
parts-unit-mm = mm
parts-unit-mm2 = mm²
part-face-drawing = From the drawing
part-face-drawn = { $p0 } · drawing
parts-import = Import
parts-scope-import = This import
parts-set-strip = An import is a door into Parts: what it brings stands in the list, unsaved, until you save it to My parts.
parts-set-project = This project holds copies of { $p0 } of these parts; { $p1 } of them would change.
parts-set-update = Update them…
parts-set-group = From { $p0 }
parts-set-row = Row { $p0 }
parts-set-rows = { $p0 ->
    [one] { $p0 } row
   *[other] { $p0 } rows
    }
parts-set-count-line = { $p0 } · { $p1 } lack something a feature reads
parts-set-columns = Columns
parts-set-read = What was read
parts-set-columns-head = Which column is what
parts-set-columns-line = Mapped once; change a row and the list on the left follows
parts-set-saved-as-label = Saved as
parts-set-saved-as = The next table of this source is read the same way.
parts-set-sizes = Width, Height and Depth go to the field of that name the row's kind has: a cabinet's outer size, a plate's size, a device's size in the cabinet.
parts-set-all-read = Every row makes a part.
parts-set-new = { $p0 } new
parts-set-changed = { $p0 } changed
parts-set-same = { $p0 } the same
parts-set-refused = { $p0 } refused
parts-set-chip-new = new
parts-set-chip-changed = changed
parts-set-chip-same = the same
parts-set-chip-refused = refused
parts-set-save = Save { $p0 } to My parts
parts-set-as-it-will-be = as it will be · { $p0 }
parts-set-ready-for = { $p0 } · Ready for
parts-set-refused-line = Row { $p0 } is refused: { $p1 }
parts-set-same-line = Row { $p0 }, { $p1 }, is the same as the part you have: nothing of it is written.
parts-set-changed-line = Row { $p0 }, { $p1 }, changes: { $p2 }.
parts-set-like-unused = Row { $p0 }, { $p1 }: Like is not used, the part exists.
parts-set-starter = a starter part: Make my own from { $p0 }, then read the table again.
parts-set-library = a part of { $p1 }: Make my own from { $p0 }, then read the table again.
parts-set-here = only in this project: keep it in My parts first, then read the table again.
parts-set-changes = What changes
parts-set-change = { $p0 } → { $p1 }
parts-set-repeats-line = Row { $p0 }, { $p1 }, repeats row { $p2 }.
parts-set-no-number = it has no part number. Give it one in the file and read it again, or leave it out.
parts-set-no-maker = it has no maker. Give it one in the file and read it again, or leave it out.
parts-set-error-cell = the cell of column { $p0 } holds the error { $p1 }.
parts-set-maker-ambiguous = its maker { $p0 } names several makers: { $p1 }.
parts-set-maker-letters = its maker { $p0 } has no letters or digits.
parts-set-number-letters = its part number { $p0 } has no letters or digits.
parts-set-repeats-other = it has the maker and the number of row { $p0 }, with other values.
parts-set-no-version = the part in My parts has no next version to take it.
parts-set-unread = it does not read as a part.
parts-set-field = in column { $p0 }: { $p1 }
parts-set-like = its Like column names { $p0 }, which is no part of My parts or of a library.
parts-export = Export
parts-exported = { $p0 ->
    [one] { $p0 } part written to { $p1 }.
   *[other] { $p0 } parts written to { $p1 }.
    }
parts-exported-saved = { $p0 ->
    [one] { $p0 } part written to { $p1 } as it is saved: what was typed and not saved is not in it.
   *[other] { $p0 } parts written to { $p1 } as they are saved: what was typed and not saved is not in it.
    }
parts-many-title = { $p0 ->
    [one] { $p0 } part
   *[other] { $p0 } parts
    }
parts-many-line = { $p0 } parts selected. A value typed goes to all of them; a field they hold different values in reads mixed, and Save leaves it as each has it.
parts-many-mixed = mixed
parts-many-refused = { $p0 } is not saved: { $p1 }
parts-many-saved = { $p0 ->
    [one] { $p0 } part saved to My parts.
   *[other] { $p0 } parts saved to My parts.
    }
parts-many-parts = The parts selected
parts-many-own = Make my own from { $p0 }
parts-many-cells = They have the same pins. A cell they differ in is empty and stays as each has it.
# A cell of the Connections table the parts selected share: the pin's name, then the column's word.
parts-many-cell = Pin { $p0 } · { $p1 }
