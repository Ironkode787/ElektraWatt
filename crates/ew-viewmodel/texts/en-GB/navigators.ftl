# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: the navigators, their columns, chips and badges, tables and the find pane. One
# message per key; `{ $p0 }`, `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback
# of every other.

# FT-1636: the browser's ratings column and its connection points, and the tree's function categories, one per first
# level of an FnDef category (the identifier sets' categories, docs/design/07-symbols-parts-macros.md §2.4).
column-rating = Ratings
# FT-1264 and FT-1289 (ID-1315): the navigators' column headers, by key (`column-<navigator>-<key>` where a navigator's header differs), their fixed badge words, the Messages pane's groups and the accessory kinds.
column-folio = Folio
column-title = Title
column-type = Type
column-frame = Frame
column-generated = Generated
column-changed-in = Changed in
column-designation = Designation
column-kind = Kind
column-function-text = Function text
column-part = Part
column-location = Location (+)
column-no = No.
column-from = From
column-from-cp = CP
column-to = To
column-to-cp = CP
column-potential = Potential
column-colour = Colour
column-cross-section = mm²
column-source = Source
column-via = Via
column-pages = Pages
column-strip = Strip
column-pos = Pos
column-terminal = Terminal
column-label = Function text
column-group = Group
column-side1 = Side 1 (clamp a)
column-side2 = Side 2 (clamp b)
column-bridge = Bridge
column-bridge-part = Bridge part
column-state = State
column-cable = Cable
column-core = Core
column-length = Length
column-shield = Shield
column-pair = Pair
column-target = Target
column-symbol = Symbol
column-module = Module
column-channel = Channel
column-process-tag = Process tag
column-placed-on = Placed on
column-station = Station
column-slot = Slot
column-rack = Rack
column-data-type = Data type
column-signal-range = Signal range
column-safety = Safety
column-tubes-tag = Tag
column-tubes-from-role = Role
column-tubes-to-role = Role
column-tubes-medium = Medium
column-tubes-line-kind = Line
column-tubes-size = OD×ID
column-tubes-material = Material
column-tubes-colour = Colour
column-tubes-part = Tube part
column-tubes-length-source = Source
column-tubes-fitting-a = Fitting A
column-tubes-fitting-b = Fitting B
column-tubes-location = Location change
column-tubes-page = Page
column-media-medium = Medium
column-media-sources = Sources
column-media-p-nom = Pressure
column-media-quality = Quality
column-media-consumers = Consumers
column-media-consumer-count = Count
column-media-default-tube = Default tube
column-comment = Comment
column-voltage = Voltage
column-class = Class
column-aliases = Aliases
column-members = Members
column-name = Name
column-page-cell = Page cell
column-direction = Direction
column-partner = Partner
column-external = External
column-usage = Usage
column-qty = Qty
column-unit = Unit
column-manufacturer = Manufacturer
column-part-number = Part number
column-description = Description
column-tags = Tags
column-folios = Folios
column-first-folio = First folio
column-section = Section
column-size = W × H (mm)
column-mount = Mount
column-version = Version
column-library = Library
column-devices = Devices
column-variables = Variables
column-tier = Tier
column-licence = Licence
column-access = Access
column-used-by = Used by
column-severity = Severity
column-rule = Rule
column-message = Message
column-object = Object
column-fix = Fix
column-connections-part = Wire part
column-cables-part = Cable type
column-potentials-kind = Type
nav-badge-unplaced = not drawn
# rc.5: what a badge word means, its hover text.
nav-badge-unplaced-tip = No page draws it yet: no symbol or box pin carries it.
nav-badge-spare-tip = A spare: kept free, nothing is wired to it. Wire it, or set its state in Properties.
nav-badge-frozen = frozen
nav-badge-spare = spare
nav-badge-main = main
nav-badge-aux = aux
nav-badge-external = external
nav-badge-strip = strip
nav-badge-assembly = assembly
nav-badge-unknown-size = unknown size
nav-badge-waived = waived
nav-badge-fix = fix
nav-badge-error = error
nav-badge-warning = warning
nav-badge-info = info
nav-badge-generated = generated
nav-badge-qet-grid = QET 10 px
nav-badge-spares = { $p0 } spare
nav-badge-pending = pending
nav-badge-end = end
nav-badge-terminated = terminated
nav-badge-not-terminated = not terminated
nav-badge-master = master
nav-badge-passes-through = passes the net through
nav-badge-not-drawn = not drawn
nav-badge-earth-connection = earth connection
list-none = none
list-and = and
badge-stale = Stale
badge-failed = Failed
badge-done = Done
badge-cancelled = Cancelled
empty-jobs = No jobs yet
empty-jobs-text = Exports you start appear here with their progress.
empty-preview-text = Renumbering, a quick fix or an import shows its changes here before you apply them.
empty-history-text = Your changes appear here; Undo steps back through them.
empty-find-text = Try other words, or search more fields and folios.
empty-find-start = Find in the project
empty-find-start-text = Type what to find; the matches appear here.
empty-notifications-text = Exports, recoveries and edits of many objects report here.
empty-messages = No findings
empty-messages-text = Check project lists the errors and warnings of the project here.

# WP-S2.17 part 3/4 (lane K32): the navigators' empty states (doc 16a §6.17), the Messages severity words and
# the row badges of Messages and History (doc 16a §6.24).
empty-nav-text = What you draw on the pages is listed here.
empty-nav-pages-text = New page adds the first page of the project.
empty-nav-filter = Nothing matches "{ $p0 }"
empty-nav-filter-text = Change the filter or clear it to see every row.
empty-nav-clear = Clear filter
empty-nav-pages = No pages yet
empty-nav-devices = No devices yet
empty-nav-connections = No connections yet
empty-nav-strips = No terminal strips yet
empty-nav-cables = No cables yet
# FT-926: PLC cards drawn before a CPU (a card is listed under its CPU, which gives it its addresses).
empty-nav-plc-no-cpu = No CPU yet
empty-nav-plc-no-cpu-text = { $p0 } waits for a CPU: a PLC module is listed here under its station and gets its addresses from it. Make a station with New station…, or place a CPU with Draw ▸ Device (search for CPU); the modules join it.
empty-nav-plc = No PLC stations or modules yet
empty-nav-bus = No bus ports yet
empty-nav-fluid = No tubes yet
empty-nav-tubes = No tubes yet
empty-nav-media = No media yet
empty-nav-potentials = No potentials yet
empty-nav-ips = No interruption points yet
empty-nav-structure = No structure identifiers yet
empty-nav-parts = No parts in use yet
empty-nav-blocks = No report blocks yet
badge-pending = Pending
badge-waived = Waived
badge-fix = Fix
badge-not-undoable = Not undoable
find-field-text = Text
find-text = Find
find-replacement = Replace with
find-regex = Regular expression
find-match-case = Match case
find-whole-word = Whole word
find-in-selection = In selection
find-folios = Folios (comma-separated)
find-run = Find
find-replace-all = Replace all
find-close = Close
find-results = { $p0 ->
        [one] 1 result
       *[other] { $p0 } results
    }
find-no-results = No results
find-truncated = More than 10,000 results: refine the search
find-stale = stale
find-replaced = Replaced { $p0 }; run again to refresh
find-updated-fewer = { $p0 ->
        [one] Preview updated: 1 fewer result
       *[other] Preview updated: { $p0 } fewer results
    }
find-updated-more = { $p0 ->
        [one] Preview updated: 1 more result
       *[other] Preview updated: { $p0 } more results
    }
find-updated-same = Preview updated: the same results
find-not-replaced = not replaced: { $p0 }
find-card-line = { $p0 } results on { $p1 } records · { $p2 } tags, { $p3 } cross-reference texts
chip-authored = = authored
chip-inherited = ^ inherited · { $p0 }
chip-derived = f derived · { $p0 }
chip-overridden = ! overridden · { $p0 }
chip-frozen = * frozen · { $p0 }
chip-library = # library · { $p0 }
chip-generated = ~ generated by block '{ $p0 }'
chip-varies = ≠ varies ({ $p0 })
chip-word-page = page
chip-word-inherited = inherited
chip-word-derived = derived
chip-word-override = override
chip-word-frozen = frozen
chip-word-library = library
chip-word-generated = generated
chip-word-varies = varies
chip-word-profile = profile
chip-word-no-supply = no supply
chip-word-potential = potential
chip-word-cable = cable
chip-word-default = default
# FT-1338 (ID-1392): the phrases of a provenance chip's text that ew-api builds in English.
chip-via-frozen-sign-hidden = frozen (sign hidden)
chip-via-frozen = frozen
chip-via-numbering = numbering
chip-via-main-representation = main representation
chip-via-strip-groups = strip groups
chip-via-function-definition = function definition
chip-via-override-cdp = override (CDP)
chip-via-wire-numbering = wire numbering
chip-via-connectivity = connectivity
chip-via-project-description = project description
chip-via-cross-reference-format = cross-reference format
chip-via-project-default = project default
chip-via-override = override
chip-via-primary-part = primary part
chip-via-layout-label = layout label
chip-via-part = part
chip-via-rail-length = rail length less placements and end brackets
chip-via-path = path
chip-via-override-on-another = override on another connection
chip-via-child-of = child of { $p0 }
chip-via-terminal-of-strip = terminal of strip { $p0 }
chip-via-numbering-scheme = numbering scheme '{ $p0 }'
chip-via-structure-box-on-page = structure box on page { $p0 }
chip-via-page = page { $p0 }
chip-via-override-on = override on { $p0 }
chip-via-project-description-is = project description is '{ $p0 }'
chip-via-cable-core = from cable { $p0 }, core { $p1 }
chip-via-potential-definition-at = potential definition at { $p0 }
chip-via-potential-definition = potential definition { $p0 }
chip-via-potential = potential { $p0 }
chip-via-colour-profile = colour profile '{ $p0 }'
chip-via-size-profile = size profile '{ $p0 }'
chip-via-class = class { $p0 }
chip-via-no-supply-upstream = no supply found upstream of { $p0 }, so its rating is not used
chip-via-built-in-default = built-in default: the project has no profile that sets it
chip-via-medium-definition-at = medium defined at { $p0 }
chip-via-through-valves = carried through the valves
chip-via-no-medium = no medium defined
chip-via-project-default-tube = the project's default tube
chip-via-medium = medium { $p0 }
chip-via-tube-part = tube part { $p0 }
chip-via-no-tube-spec = no default tube, medium tube or tube part
chip-via-no-terminal = on no valve terminal
nav-show-solenoid = { $p0 }, solenoid { $p1 }
chip-via-plug = its plug
chip-via-plc-output = PLC output { $p0 }
chip-via-no-plc-output = no PLC output yet
chip-via-valve-part = the valve's part
chip-via-earth-connection = an earth symbol: a connection to earth, no device, so no tag
table-nothing = No cell can be applied.
table-no-row = no row
table-no-column = no column
table-not-clearable = cannot be cleared
table-paste-too-large = too large: a paste takes at most { $p0 } cells of at most { $p1 } characters
table-paste-unclosed-quote = unclosed quote in row { $p0 }
table-pattern-unclosed = unclosed brace: a `{"{"}` has no `{"}"}` (write `{"{{"}` for a literal brace)
table-pattern-stray-close = a `{"}"}` closes nothing (write `{"}}"}` for a literal brace)
table-pattern-unknown = unknown expression {"{"}{ $p0 }{"}"}: use {"{"}n{"}"}, {"{"}n*2+1{"}"}, {"{"}n:02{"}"}, {"{"}col:<column>{"}"} or {"{"}prev{"}"}
table-pattern-number = the number in {"{"}{ $p0 }{"}"} is too large
table-pattern-column = unknown column { $p0 } in {"{"}col:{ $p0 }{"}"}
table-row-overflow = the counter overflows
table-row-negative = the counter is negative
table-row-too-long = the text is longer than 10000 characters

# The card's title of a gesture: "Preview: paste 12 cells on 12 terminals · 2 invalid".
table-preview-title = Preview: { $p0 } { $p1 } { $p2 } on { $p3 } { $p4 }
table-preview-invalid = { $p0 } · { $p1 } invalid
table-cell = cell
table-cells = cells
table-import-title = Import { $p0 }
table-import-nothing = Nothing to import: the file matches the project.
table-import-counts = { $p0 }, { $p1 }, { $p2 }, { $p3 }, { $p4 }, { $p5 }
table-import-changed = { $p0 ->
        [one] 1 cell changed
       *[other] { $p0 } cells changed
    }
table-import-kept = { $p0 ->
        [one] 1 cell kept
       *[other] { $p0 } cells kept
    }
table-import-refused = { $p0 ->
        [one] 1 cell refused
       *[other] { $p0 } cells refused
    }
table-import-unknown = { $p0 ->
        [one] 1 unknown row
       *[other] { $p0 } unknown rows
    }
table-import-missing = { $p0 ->
        [one] 1 row not in the file
       *[other] { $p0 } rows not in the file
    }
table-import-conflicts = { $p0 } conflicts
table-import-conflict-kept = 1 conflict: the project's value is kept
table-import-conflicts-kept = { $p0 } conflicts: the project's values are kept
table-import-conflict-taken = 1 conflict: the file's value is taken
table-import-conflicts-taken = { $p0 } conflicts: the file's values are taken
table-import-cell = { $p0 } · { $p1 } · "{ $p2 }" → "{ $p3 }" · { $p4 }
table-import-cell-applied = Apply takes the file's value
table-import-cell-kept = kept: changed in the project only
table-import-cell-conflict = conflict: the project's value is kept
table-import-cell-conflict-taken = conflict: Apply takes the file's value
table-import-cell-read-only = read-only
table-import-cell-refused = refused
table-import-cells = Cells
table-import-missing-rows = Rows not in the file

# WP-S2.06 part 3: the table widget (§8.1).
table-toggle = Table
table-name = { $p0 } table
table-columns = Columns
table-move-left-name = Move { $p0 } left
table-move-right-name = Move { $p0 } right
table-sort-by = Sort by { $p0 }

# Doc 16a §6.21: the tooltip of a derived column's header icon.
table-derived = Derived; read-only
table-cell-row = , row { $p0 } of { $p1 }
table-cell-read-only = , read-only
table-pattern-title = Pattern fill
table-pattern-hint = Pattern
# ID-1497 (1): the cell editor's line while more than one row is marked.
table-commit-all-hint = Ctrl+Enter sets all { $p0 } marked rows.
table-pattern-values = Values: { $p0 }
table-resize = Resize { $p0 } column
nav-title-devices = Devices
nav-title-connections = Connections
nav-title-strips = Terminal strips
nav-title-cables = Cables
nav-title-plc = PLC
nav-title-plc-grid = PLC grid
nav-title-bus = Bus
nav-title-fluid = Fluid
nav-title-tubes = Tube table
nav-title-media = Medium table
nav-title-potentials = Potentials
nav-title-ips = Interruption points
nav-title-structure = Structure identifiers
nav-title-parts = Parts in use
nav-title-blocks = Report blocks
nav-title-messages = Messages
nav-tree-name = { $p0 } navigator
nav-mode-flat = Flat list
nav-mode-section = By section
nav-mode-structure = By structure
nav-sort-folio = Sort by folio
nav-sort-type = Sort by type
nav-front-matter = Front matter
nav-fluid-terminals = Valve terminals
nav-fluid-devices = Fluid devices
nav-fluid-media = Media
nav-badge-consumers = { $p0 ->
        [one] 1 consumer
       *[other] { $p0 } consumers
    }
nav-badge-cpu = CPU
nav-badge-head-station = head station
nav-badge-io-device = I/O device
nav-badge-valve-terminal = valve terminal
nav-badge-io-link-master = IO-Link master
nav-badge-drive = drive
nav-badge-external-partner = external partner
nav-badge-station = station
nav-section = Section { $p0 } - { $p1 }
nav-rack = Rack { $p0 }
nav-none = (none)
nav-filter-hint = Filter
badge-generated = generated
badge-qet-grid = QET 10 px
badge-current = current
nav-page-name = { $p0 } { $p1 }, page{ $p2 }
nav-node-name = { $p0 }, { $p1 }
nav-new-page = New page
nav-new-section = New section
table-sorted-ascending = { $p0 }, sorted ascending
table-sorted-descending = { $p0 }, sorted descending

# WP-S1.19 part 6 (§11): the Macros and Libraries navigators, the macro card, Save as macro… and Make variable.
nav-title-macros = Macros
nav-title-libraries = Libraries
empty-nav-macros = No macros yet
empty-nav-macros-text = Save as macro… in the Edit menu stores a selection in a library; its macros are listed here.
empty-nav-libraries = No libraries yet
badge-update-available = Update available
badge-index-failed = Index failed
badge-profile-unreadable = PLC profile unreadable
badge-fits-function = Fits { $p0 }'s function
badge-same-category = Same category

# WP-S4.02 part 3: the Unplaced devices navigator (FT-486), the layout place tool and Select on a panel page
# (D4, D10; doc 16a §5.5 HUD, §6.17 empty states, §6.20 segmented control, §7.4 chips, §8 wording).
nav-title-unplaced = Unplaced devices
empty-nav-unplaced = No panel layout yet
empty-nav-unplaced-text = Devices are listed here once the location has an enclosure.
empty-nav-unplaced-done = All devices placed
empty-nav-unplaced-done-text = Every device of { $p0 } is placed.
list-continues-row = The list continues on page { $p0 } ›
# Ruling 6: the count texts' singular.
empty-nav-terminals = No terminals yet
empty-nav-terminals-text = Place a terminal with Draw ▸ Terminal…; it goes into the strip you choose there.

# C49-p8b#2: the strip editor's scope in the filter row.
nav-scope-strip = Strip: { $p0 }
nav-scope-clear = Show every strip
nav-plc-addresses = Addresses
nav-badge-manual = manual
nav-badge-not-drawn-tip = Not drawn on any page yet, so its tag shows ? where the page number belongs. Draw it to give it its place.
nav-guide = Open the guide
chip-via-port-no-medium = no medium: choose one in the Medium row of its source, the air supply
chip-via-no-medium-yet = no medium yet: choose one here
nav-badge-in-no-plc = in no PLC
nav-badge-fix-all = Fix all
chip-go-page = Go to page { $p0 }
chip-go-page-cell = Go to page { $p0 }, { $p1 }
chip-put-here = Put it here
list-columns-title = List columns
list-rows-title = Rows of your own
list-columns-table = Columns
list-columns-column = Column
list-columns-print = Print
list-columns-heading = Heading
list-columns-width = Width
list-columns-order = Order
list-rows-table = Rows the model does not hold, listed with the list's own rows; empty rows are left out
list-rows-row = Row
