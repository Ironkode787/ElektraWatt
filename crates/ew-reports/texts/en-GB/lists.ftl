# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The words of the lists in en-GB (architecture §18): a list page prints them in the project's language, the CSV
# and the XLSX in English (`ew_reports::words`).

## The heading of a column

column-action = Action
column-address = Address
column-address-or-name = Address or name
column-aliases = Aliases
column-at = At
column-author = Author
column-bridge = Bridge
column-bus-address = Bus address
column-bus-system = Bus system
column-cable = Cable
column-cable-type = Cable type
column-category = Category
column-change = Change
column-changed-pages = Changed pages
column-channel = Channel
column-checker = Checker
column-class = Class
column-code = Code
column-colour = Colour
column-comment = Comment
column-configuration-project = Configuration project
column-connector = Connector
column-consumers = Consumers
column-core = Core
column-count = Count
column-covers = Covers
column-cp = CP
column-cpu = CPU
column-data-type = Data type
column-date = Date
column-deactivated = Deactivated
column-default-tube = Default tube
column-description = Description
column-designation = Designation
column-device = Device
column-device-id = Device ID
column-device-type = Device type
column-dt = DT
column-external = External
column-f-destination = F-destination
column-f-source = F-source
column-fitting-a = Fitting A
column-fitting-b = Fitting B
column-folio = Folio
column-folios = Folios
column-from = From
column-from-end = From end
column-frozen = Frozen
column-function = Function
column-function-aspect = Function (=)
column-function-text = Function text
column-group = Group
column-input-bits = Input bits
column-input-bytes = Input bytes
column-input-start = Input start
column-ip-address = IP address
column-item = Item
column-kind = Kind
column-label = Label
column-label-text = Label text
column-length = Length
column-length-from = Length from
column-level = Level
column-line = Line
column-location-aspect = Location (+)
column-location-change = Location change
column-logical-net = Logical net
column-main-representation = Main representation
column-manufacturer = Manufacturer
column-master = Master
column-material = Material
column-medium = Medium
column-members = Members
column-mm2 = mm²
column-mode = Mode
column-module = Module
column-module-part = Module part
column-no = No.
column-occupancy = Occupancy
column-od-id = OD×ID
column-other = Other
column-output-bits = Output bits
column-output-bytes = Output bytes
column-output-start = Output start
column-page = Page
column-pages = Pages
column-pair = Pair
column-part = Part
column-part-number = Part number
column-parts = Parts
column-pieces = Pieces
column-port = Port
column-pos = Pos
column-position = Position
column-potential = Potential
column-potential-type = Potential type
column-pressure = Pressure
column-process-tag = Process tag
column-qty = Qty
column-quality = Quality
column-rate-bit-s = Rate (bit/s)
column-requirement = Requirement
column-reserved = Reserved
column-revision = Revision
column-role = Role
column-rule = Rule
column-rule-value = Rule value
column-segment = Segment
column-segments = Segments
column-sensor-or-actuator = Sensor or actuator
column-severity = Severity
column-side-1-clamp-a = Side 1 (clamp a)
column-side-2-clamp-b = Side 2 (clamp b)
column-size = Size
column-slot = Slot
column-solenoids = Solenoids
column-source = Source
column-sources = Sources
column-spare = Spare
column-state = State
column-station = Station
column-status = Status
column-strip = Strip
column-subnet-mask = Subnet mask
column-symbol = Symbol
column-table = Table
column-tag = Tag
column-tags = Tags
column-target = Target
column-target-page = Target page
column-target-pin = Target pin
column-tbd = TBD
column-technical-data = Technical data
column-terminal = Terminal
column-terminals = Terminals
column-termination = Termination
column-test-pulse = Test pulse
column-title = Title
column-to = To
column-total-length = Total length
column-tube = Tube
column-tube-part = Tube part
column-type = Type
column-unit = Unit
column-used = Used
column-valve = Valve
column-valve-part = Valve part
column-vendor-id = Vendor ID
column-via = Via
column-voltage = Voltage
column-voltage-class = Voltage class
column-wire-part = Wire part

## The title of a box or a table, and the words of a heading row

title-legend = LEGEND
title-part-data = PART DATA
title-plc-overview = PLC OVERVIEW
title-colour-definitions = CABLE COLOUR DEFINITIONS
title-sizes-control = STANDARD SIZES CONTROL
title-sizes-mains = STANDARD SIZES MAINS
cell-sizes-note = If size or sign is not indicated in diagram, standard size or sign is to be used.
cell-cable-overview = Cable overview
cell-bridges = Bridges
cell-structure = Structure
cell-device = Device

## Why a list has no rows

not-derived-unsupported = Not derived: this build does not make this list
not-derived-cycle = Not derived: the list depends on its own pages

## A value in words, and a value still to be decided or missing

cell-yes = yes
cell-no = no
cell-on = on
cell-off = off
cell-tbd = TBD
cell-piece = pc
cell-length-missing = length missing
cell-part-missing = part missing
cell-cross-section-missing = cross-section missing
cell-colour-missing = colour missing
cell-manufacturer-missing = manufacturer missing
cell-other = other

## The state of a terminal, a channel or a revision

cell-used = used
cell-unused = unused
cell-spare = spare
cell-reserved = reserved
cell-not-fitted = not fitted
cell-deactivated = deactivated
cell-revision-open = open
cell-revision-released = released
cell-revision-withdrawn = withdrawn

## Where a length, a cross-section or a colour comes from

cell-from-cable = (cable)
cell-source-cdp = cdp
cell-source-core = core
cell-source-bond = bond
cell-source-rule = rule
cell-source-potential = potential
cell-source-project = project
cell-source-built-in = built-in
cell-source-pair = { $p0 }/{ $p1 }

## How a wire is joined, and a terminal bridge

cell-join-star = { $p0 } star
cell-join-star-at = { $p0 } star at { $p1 }
cell-join-chain = { $p0 } chain
cell-join-chain-on = { $p0 } chain on { $p1 }
cell-daisy-chain = chain on { $p0 }
cell-via-between = { "; " }
cell-bridge-plug-in = plug-in
cell-bridge-saddle = saddle
cell-bridge-wire = wire
cell-bridge-inter-level = inter-level
cell-bridge-other = bridge
cell-bridge-span = { $p0 } bridge, terminals { $p1 }

## A potential's type and circuit class, and a bond

cell-bond = Bond

## A port's plug, a valve terminal's slot

cell-end-plug = { $p0 } · plug { $p1 }
cell-valve-used = valve, used
cell-valve-spare = valve, spare
cell-valve-reserved = valve, reserved
cell-valve-not-fitted = valve, not fitted
cell-blanking-plate = blanking plate
cell-empty = empty

## A finding's severity, a frozen value against its rule, an order code

cell-severity-info = info
cell-severity-warning = warning
cell-severity-error = error
cell-item-tag = tag
cell-item-wire-number = wire number
cell-matches = matches
cell-deviates = deviates
cell-order-code = Order Code { $p0 }

## The name of a list kind, and the words it is searched for by

report-kind-profile-tables = Profile tables
report-kind-part-data = Part data
report-kind-toc = Table of contents
report-kind-structure-overview = Structure overview
report-kind-parts-summary = Parts summary
report-kind-parts-list = Parts list
report-kind-device-list = Device list
report-kind-terminal-diagram = Terminal diagram
report-kind-terminal-overview = Terminal overview
report-kind-cable-overview-diagrams = Cable overview and diagrams
report-kind-cable-overview-diagrams-words = cable list, cable diagram, cable plan
report-kind-wire-list = Wire list
report-kind-plc-overview = PLC overview
report-kind-io-list = I/O list
report-kind-plc-card-list = PLC card list
report-kind-network-address-list = Network address list
report-kind-bus-overview = Bus overview
report-kind-bus-cable-list = Bus cable list
report-kind-iolink-port-list = IO-Link port list
report-kind-safety-io-list = Safety I/O list
report-kind-potential-overview = Potential overview
report-kind-revision-overview = Revision overview
report-kind-open-items = Open items
report-kind-numbering-report = Numbering report
report-kind-enclosure-legend = Enclosure legend
report-kind-check-report = Check report
report-kind-tube-list = Tube list
report-kind-tube-cut-list = Tube cut list
report-kind-tube-labels = Tube labels
report-kind-valve-terminal-config = Valve-terminal configuration
report-kind-medium-overview = Medium overview
report-kind-sql = SQL query
report-kind-unknown = Unknown kind
