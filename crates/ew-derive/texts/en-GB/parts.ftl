# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The words of a part's declared fields (`ew_derive::part_fields`, doc 07 §2.5, §5.4, §5.7): each field's word, the
# groups of a part's sheet, the features that read a part, what each says while a field is empty, and why a member is
# not asked for. One key per field, whichever surface shows it: Parts, the reference page, the command line.

## The groups of a part's sheet, in its order

part-group-identity = Identity
part-group-schematic = On the schematic
part-group-cabinet = In the cabinet
part-group-connections = Connections
part-group-enclosure = Enclosure
part-group-plate = Mounting plate
part-group-rail = Rail
part-group-duct = Wiring duct
part-group-cable = Cable
part-group-wire = Wire
part-group-ferrule = Ferrule or lug
part-group-marker = Marker
part-group-gland = Cable gland
part-group-valve = Valve
part-group-tube = Tube
part-group-plc = PLC
part-group-ports = Network ports
part-group-technical = Technical data
part-group-goes-with = Goes with it
part-group-about = About

## Identity

part-field-kind = Kind
part-field-manufacturer = Maker
part-field-part-number = Part number
part-field-type-designation = Type
part-field-order-number = Order number
part-field-gtin = GTIN
part-field-description = Description
part-field-category = Listed under
part-field-unit = Sold by

## About

part-field-meta-key = Key
part-field-meta-licence = Licence
part-field-meta-sources = Came from
part-field-meta-as-of = Changed
part-field-meta-confidence = Confidence
part-field-meta-status = Status
part-field-meta-foreign-terms = Words of another language
part-field-free-props-key = Property
part-field-free-props-value = Value
part-field-alternatives-kind = Alternative
part-field-alternatives-part = Alternative part
part-field-macros-rep-type = Macro for
part-field-macros-macro = Macro

## On the schematic

part-field-function-template-fndef = What it is
part-field-function-template-main = Carries the tag
part-field-function-template-cps = Pins of the piece
part-field-contacts-kind = Contact
part-field-contacts-cps = Pins of the contact
part-field-contacts-seq = Sequence digit
part-field-contacts-special = Special contact
part-field-cp-pattern-idx = Order
part-field-cp-pattern-designation = Pin
part-field-cp-pattern-potential-role = Carries
part-field-cp-pattern-level = Level
part-field-cp-pattern-side = Side
part-field-cp-pattern-clamp = Clamp place
part-field-box-width = Box width
part-field-box-height = Box height
part-field-box-pins-idx = Pin on the box
part-field-box-pins-side = Edge
part-field-box-pins-at = From the corner

## In the cabinet

part-field-mech-width = Width
part-field-mech-height = Height
part-field-mech-depth = Depth
part-field-mech-mount-kind = Mounts on
part-field-mech-mass = Mass
part-field-mech-clearance-top = Keep free above
part-field-mech-clearance-bottom = Keep free below
part-field-mech-clearance-left = Keep free left
part-field-mech-clearance-right = Keep free right
part-field-mech-clearance-front = Keep free in front
part-field-mech-rail-offset = Rail line
part-field-mech-front-shape = Front
part-field-mech-front-2d = Outline
part-field-mech-holes-x = X
part-field-mech-holes-y = Y
part-field-mech-holes-hole = Hole

## Connections

part-field-cp-pattern-clamp-type = Clamp
part-field-cp-pattern-csa-min = Takes from
part-field-cp-pattern-csa-max = Takes up to
part-field-cp-pattern-max-conductors = Wires
part-field-cp-pattern-twin-ferrule = Twin ferrule

## The kinds' own groups

part-field-enclosure-width = Width
part-field-enclosure-height = Height
part-field-enclosure-depth = Depth
part-field-enclosure-plate = Mounting plate
part-field-enclosure-doors-hinge = Hinge
part-field-enclosure-doors-width = Door width
part-field-enclosure-edge-margin = Edge margin
part-field-enclosure-usable-depth = Depth behind the door
part-field-enclosure-wall = Wall thickness
part-field-plate-width = Width
part-field-plate-height = Height
part-field-plate-thickness = Thickness
part-field-rail-profile = Profile
part-field-rail-stock-length = Bar length
part-field-duct-inner-width = Inner width
part-field-duct-inner-height = Inner height
part-field-duct-slot-pitch = Slot pitch
part-field-cable-cores-core = Core
part-field-cable-cores-colour = Core colour
part-field-cable-cores-csa = Core cross-section
part-field-cable-cores-shield = Shield
part-field-cable-cores-pair = Pair
part-field-cable-outer-diameter = Outer diameter
part-field-bus-cable-for = Made for
part-field-wire-csa = Cross-section
part-field-wire-colour = Colour
part-field-wire-class = Conductor class
part-field-ferrule-csa-min = Takes from
part-field-ferrule-csa-max = Takes up to
part-field-ferrule-twin = Twin ferrule, for two wires
part-field-ferrule-insulated = Insulated collar
part-field-ferrule-sleeve = Sleeve length
part-field-ferrule-colour = Colour
part-field-ferrule-stud = Stud of a ring lug
part-field-marker-marks = Marks
part-field-marker-lines = Lines
part-field-marker-chars = Characters per line
part-field-gland-thread = Thread
part-field-gland-clamp-min = Seals from
part-field-gland-clamp-max = Seals up to
part-field-gland-nut = Nut across
part-field-fluid-coil-voltage = Coil voltage
part-field-fluid-coil-env = Coil current
part-field-fluid-coil-power = Coil power
part-field-fluid-solenoids = Solenoids
part-field-fluid-positions = Positions
part-field-fluid-pins-pin = Plug pin
part-field-fluid-pins-solenoid = Solenoid of the pin
part-field-fluid-galleries-gallery = Gallery
part-field-fluid-galleries-separates = Separates
part-field-fluid-galleries-tap = Tap
part-field-fluid-transform-outlet = Regulator outlet
part-field-fluid-transform-pressure = Regulator pressure
part-field-fluid-transform-vacuum = Ejector vacuum port
part-field-fluid-ports-designation = Port
part-field-fluid-ports-position = Position of the port
part-field-fluid-ports-push-in = Push-in for a tube of
part-field-fluid-ports-qn = Flow
part-field-fluid-ports-thread = Port thread
part-field-fluid-fitting-thread = Fitting thread
part-field-fluid-fitting-tube = Fitting for a tube of
part-field-tube-od = Outer diameter
part-field-tube-id = Inner diameter
part-field-tube-material = Material
part-field-tube-colour = Colour
part-field-tube-bend = Smallest bend radius
part-field-tube-pressure = Pressure rating
part-field-tube-t-min = Lowest temperature
part-field-tube-t-max = Highest temperature

## PLC

part-field-plc-role = What it is in the PLC
part-field-plc-channels-kind = Channels
part-field-plc-channels-count = Number of channels
part-field-plc-channels-type = Data type
part-field-plc-channels-pins = Pin pattern
part-field-plc-more-kind = Second channels
part-field-plc-more-count = Number of second channels
part-field-plc-more-type = Data type of the second channels
part-field-plc-more-pins = Pin pattern of the second channels
part-field-plc-io-dir = Direction
part-field-plc-io-class = Signal
part-field-plc-io-bits = Bits
part-field-plc-supply-kind = Supply
part-field-plc-supply-feeds = Feeds
part-field-plc-supply-plus = L+ pin
part-field-plc-supply-m = M pin
part-field-plc-allowed-slots = Slots it fits
part-field-plc-max-modules = Most modules
part-field-plc-max-inputs = Most input bytes
part-field-plc-max-outputs = Most output bytes
part-field-plc-channel-ma = Current per channel
part-field-plc-group-ma = Current per group
part-field-plc-module-ma = Current of the module
part-field-plc-iolink-class = IO-Link port class
part-field-plc-iolink-inputs = IO-Link input bytes
part-field-plc-iolink-outputs = IO-Link output bytes
part-field-plc-iolink-cable = Longest IO-Link cable
part-field-plc-positions = Valve positions
part-field-plc-positions-channel = Channel of a position
part-field-plc-positions-solenoid = Solenoid of a position
part-field-plc-unbound = A channel without a signal is
part-field-plc-bit-of = Bit of a channel
part-field-plc-pin-of = Pin of a channel

## Network ports

part-field-bus-interface = Interface
part-field-bus-port-label = Port label
part-field-bus-system = Bus system
part-field-bus-role = Role
part-field-bus-ports = Sockets
part-field-bus-termination = Terminating resistor
part-field-bus-connector = Connector
part-field-bus-rates = Rates
part-field-bus-address-min = Lowest address
part-field-bus-address-max = Highest address
part-field-bus-iolink-vendor = IO-Link vendor
part-field-bus-iolink-device = IO-Link device
part-field-bus-iolink-class = IO-Link class
part-field-bus-iolink-type = IO-Link data type
part-field-bus-iolink-inputs = IO-Link input bits
part-field-bus-iolink-outputs = IO-Link output bits
part-field-bus-signals = Signals

## Technical data

part-field-tech-key = Name
part-field-tech-value = Value
part-field-tech-rated-current = Rated current
part-field-tech-load-current = Load current
part-field-tech-power = Power
part-field-tech-voltage = Voltage
part-field-tech-colour = Colour

## Goes with it

part-field-accessories-part = Comes with
part-field-accessories-kind = What it is for
part-field-accessories-qty = Quantity
part-field-accessories-per = Per
part-field-accessories-optional = Optional
part-field-variants-key = Variant
part-field-variants-name = Name of the variant
part-field-variants-part-number = Its part number
part-field-variants-order-number = Its order number
part-field-variants-type = Its type
part-field-variants-description = Its description
part-field-variants-gtin = Its GTIN
part-field-variants-tech-key = Its technical value
part-field-variants-tech-value = Its value
part-field-configured-order-code = Order code
part-field-configured-part = Component
part-field-configured-qty = Component quantity

## The features that read a part (Ready for, doc 07 §5.7)

part-feature-parts-list = Parts list
part-feature-part-choice = Choosing a part
part-feature-search = Search
part-feature-about = Where it came from
part-feature-placing = Schematic
part-feature-contact-image = Contact image
part-feature-potentials = Potentials
part-feature-terminals = Terminal diagram
part-feature-footprint = Cabinet layout
part-feature-rail-fit = Rails
part-feature-clearance = Space around it
part-feature-surfaces = Doors and walls
part-feature-door-reach = Behind the door
part-feature-hole-table = Hole table
part-feature-part-card = Part card
part-feature-cabinet = Cabinet
part-feature-plate-frame = Mounting plate
part-feature-duct-size = Wiring ducts
part-feature-wire-ends = Wire ends and ferrules
part-feature-clamp-fit = Clamp check
part-feature-ferrule-choice = Ferrule choice
part-feature-marker-fit = Markers
part-feature-stock-match = Wire list
part-feature-cable-cores = Cables
part-feature-gland-choice = Cable glands
part-feature-bus = Network
part-feature-plc = PLC
part-feature-output-load = Output load
part-feature-wire-sizing = Wire sizing
part-feature-supply-load = Supply load
part-feature-assembly-list = Assembly list
part-feature-accessories = Accessories
part-feature-fluid = Pneumatics
part-feature-tube-fit = Tube check
part-feature-coil-check = Coil check

## What a reader does without the field ({ $p0 }: the fields lacking, in words), or what the field is for

part-says-identity = The parts list cannot name the part until it has { $p0 }.
part-says-printed-list = Printed in the parts list.
part-says-kind = Decides what the sheet asks for and where the part is offered.
part-says-category = Where the part is listed when a part is chosen.
part-says-key = Finds the part by its key.
part-says-about = Shown where the part came from.
part-says-status = A part no longer ordered is offered last.
part-says-pieces = It is drawn on no schematic page until it says what it is drawn as.
part-says-contacts = Drawn in the contact image under its coil.
part-says-pins = The pins a device of the part has, by name.
part-says-carries = The potential the pin carries.
part-says-levels = The terminal diagram draws each level, each passing through on its own.
part-says-box = Drawn as a box with its pins where they stand.
part-says-footprint = Without { $p0 } it stands on the layout as a 20 × 20 mm square marked unknown size.
part-says-duct-size = Without { $p0 } the duct is drawn at the size the fields of Draw duct give.
part-says-depth = Without a depth, what it takes behind a door or a wall is not drawn.
part-says-mount = Without how it mounts, the layout cannot say whether it fits a rail.
part-says-mass = Printed on the part card.
part-says-clearance = Kept free beside it on the layout.
part-says-rail-line = Where it clips onto its rail.
part-says-front = How its front is drawn on a door or a wall.
part-says-holes = It goes in a cut-out but has no holes, so the hole table and the drawings for drilling lack them.
part-says-clamp-type = Without a clamp type the wire list leaves the treatment of the wire ends at these pins open.
part-says-clamp-range = Without a clamp range the check that a wire fits its clamp skips these pins.
part-says-clamp-wires = Without a number of wires the check that a clamp takes the wires landed on it skips these pins.
part-says-twin = Whether two wires may share a twin ferrule here.
part-says-cabinet = Without { $p0 } the cabinet's outline, its doors and its walls have no size on the layout.
part-says-cabinet-depth = Without a depth the cabinet has no side walls, top or bottom to drill.
part-says-plate = Without a mounting plate named, the cabinet's plate has no area to place devices on.
part-says-doors = No door is given: the cabinet has one door, hinged left.
part-says-edge-margin = Without an edge margin the doors and the walls keep no edge free of holes.
part-says-plate-size = Without { $p0 } the plate has no area to place devices on.
part-says-rail-profile = A rail cannot be drawn or offered until it says its profile.
part-says-cores = A cable cannot be drawn with this part until it has its cores.
part-says-core-data = Printed for each core in the cable lists.
part-says-cable-diameter = Without an outer diameter no gland is proposed for a cable of this part.
part-says-cable-for = Offered for a bus line of these systems.
part-says-wire = A wire part without { $p0 } matches no wire: the lists name no part for the wire.
part-says-wire-class = Without a conductor class the wire counts as fine-stranded (class 5).
part-says-ferrule = A ferrule without the cross-sections it takes is chosen for no wire end.
part-says-ferrule-data = Printed in the wire list for the wire ends it takes.
part-says-sleeve = Without a sleeve length the wire list prints no strip length.
part-says-marker = A marker that does not say what it marks is taken for no label.
part-says-marker-size = Without { $p0 } the check that a marker's text fits skips it.
part-says-gland = A gland without { $p0 } is proposed for no cable.
part-says-gland-nut = Without a nut size the gland takes 1.4 × its thread.
part-says-coil = Without { $p0 } the check against the output that drives it skips it.
part-says-fluid = Read by the pneumatic pages and their lists.
part-says-tube = Without { $p0 } the check that the tube fits its ports and the pressure skips it.
part-says-tube-data = Printed in the tube list.
part-says-plc = What the PLC configuration makes of the part.
part-says-ratings = Without { $p0 } the check of the load on its outputs skips it.
part-says-bus = What the network pages and their checks make of the part.
part-says-tech = Printed where a list or a text field names the value.
part-says-rated-current = Sizes the wires it protects.
part-says-load = Counted in the load of the PLC output that drives it.
part-says-terminal-colour = Printed in the assembly list.
part-says-accessories = Counted in the parts list with the part.
part-says-variants = Chosen in Properties; each prints its own number.
part-says-configured = Printed in the parts list as the order code and its components.

## Why a member is not asked for (shown under Also stored when it has a value)

part-stored-not-read = No list prints it and no check reads it.
part-stored-import = A maker's file brought it; no list prints it.
part-stored-not-yet = Nothing reads it yet.

## Joining the fields a sentence names

part-fields-between = {", "}
part-fields-and = {" and "}
