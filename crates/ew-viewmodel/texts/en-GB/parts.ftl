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
parts-card-title = Import parts
parts-card-line = { $p0 }: { $p1 } created, { $p2 } skipped, { $p3 } errors
parts-map-title = Map columns
parts-map-intro = Choose what each column of { $p0 } imports as.
parts-map-source = Source
parts-map-saved = Saved map
parts-map-column = Column
parts-map-sample = Sample
parts-map-member = Imports as
parts-map-none = Not imported
parts-map-import = Import
parts-map-cancel = Cancel
parts-map-twice = `{ $p0 }` is chosen for two columns
parts-map-twice-text = { $p0 } is chosen for two columns
parts-map-missing = Map a column to { $p0 }
parts-map-mixed = Map one description, or one per language, not both
parts-member-manufacturer = Manufacturer
parts-member-part-number = Part number
parts-member-type-designation = Type designation
parts-member-order-number = Order number
parts-member-gtin = GTIN
parts-member-description = Description
parts-member-description-lang = Description ({ $p0 })
parts-member-kind = Kind
parts-member-unit = Unit
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
part-card-lost = A bus line ends at a socket the part no longer has: { $p0 }. It keeps its end there; draw it to another socket.
part-card-box = Drawn anew: { $p0 } on page { $p1 }, its pins as the part has them now
part-card-box-moved = Drawn anew: { $p0 } on page { $p1 }, moved back inside the drawing area
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
stock-title = Workshop stock
stock-text = The wire, ferrule and marker parts your shop keeps, in the order you prefer them. A wire takes the first wire part of its size and colour, a wire end the first ferrule that fits it, a label the first marker made for it. Add the generic stock adds the starter's ferrules and markers the stock lacks.
stock-twice = { $p0 } is in the stock already
stock-cut-length = "{ $p0 }" is not a length: give millimetres, as 5 or 2.5.
stock-cut = Cut to (mm)
stock-cut-hint = What the cut list rounds every rail, duct, cable and tube up to; empty: 5 mm.
stock-add = Add
stock-add-field = Part number
stock-up = Up
stock-down = Down
stock-remove = Remove
stock-empty = No stock yet: add parts, or Add the generic stock.
stock-summary = Workshop stock: { $p0 } parts
stock-column-kind = Kind
stock-column-number = Part number
stock-column-description = Description
stock-column-for = For
stock-column-colour = Colour
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
part-pins-and-box = Pins and box…
part-pins-and-box-how = Which pins the device has and where they stand on the box it is drawn as
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
part-row-mm = { $p0 } (mm)
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
parts-title = Parts
parts-new = New part
parts-like-this = Like this
parts-own = Make my own
parts-own-from = Make my own from it
parts-keep = Keep in my parts
parts-remove = Remove
parts-save = Save
parts-save-use = Save and use for { $p0 }
parts-discard = Discard
parts-back = Back
parts-back-without-choosing = Back without choosing
parts-back-without-it = Back without it
parts-strip-choosing = You are choosing a part for { $p0 }.
parts-strip-form = A part for { $p0 }.
parts-new-title = New part
parts-none-open = Open a part from the list, or make a new one.
parts-search-hint = Search number, maker, description, what it is for
parts-search-name = Search parts
parts-scope-all = All
parts-scope-project = In this project
parts-scope-mine = My parts
parts-filter-kind = Kind
parts-filter-kind-all = Every kind
parts-filter-lacking = Lacks something
parts-count = { $p0 ->
    [one] { $p0 } part
   *[other] { $p0 } parts
    }
parts-used-by = { $p0 ->
    [one] { $p0 } device of this project uses it
   *[other] { $p0 } devices of this project use it
    }
parts-changed = Changed, not saved
parts-lives-mine = Mine · in My parts
parts-lives-starter = Starter part
parts-lives-here = Only in this project
parts-lives-new = New, not saved
parts-lock-starter = A starter part. It comes with the program and is the same on every PC, so it cannot be changed here.
parts-lock-library = A part of a library you cannot write, so it cannot be changed here.
parts-ready-for = Ready for
parts-ready-all = Nothing a feature reads is missing.
parts-schematic-picture = How the part is drawn on a schematic page
parts-no-schematic = No symbol draws it yet.
parts-no-front = Give it a width and a height to see its front.
parts-saved-mine = { $p0 } is saved in My parts.
reason-no-parts-place = Parts is not open.
reason-no-part-open = No part is open in Parts.
reason-part-not-yours = The part is not yours: make your own from it.
reason-part-not-changed = Nothing was changed since the part was opened or saved.
reason-part-own-already = The part is yours already: Like this makes another like it.
reason-parts-unsaved = The part has changes that are not saved: Save or Discard them first.
reason-part-remove-used = { $p0 ->
    [one] { $p0 } device of this project uses the part.
   *[other] { $p0 } devices of this project use the part.
    }
