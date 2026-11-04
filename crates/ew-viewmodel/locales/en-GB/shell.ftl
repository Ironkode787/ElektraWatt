# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the shell in en-GB (WP-S2.08 D9): the source language and the fallback of every other. One message
# per key; `{ $p0 }`, `{ $p1 }`, ... are the arguments of `strings::format`, always strings. `cargo xtask settle`
# generates `src/strings/keys.rs` from this file: the key list that `key!` checks at compile time.

# Menus and submenus.
menu-file = File
menu-edit = Edit
menu-view = View
menu-page = Page

# WP-S2.17 part 2: the app menu's tab menus (doc 16a §9.2); Insert became Draw, Tools dissolved.
menu-project = Project
menu-draw = Draw
menu-check = Check
menu-output = Output
menu-help = Help
submenu-open-recent = Open recent
submenu-ui-scale = UI scale
submenu-theme = Theme
submenu-density = Density
submenu-min-font = Minimum text size
menu-no-recent = (none)
scale-100 = 100 %
scale-125 = 125 %
scale-150 = 150 %
scale-200 = 200 %
scale-250 = 250 %
scale-300 = 300 %
min-font-off = Off
min-font-pt = { $p0 } pt

# The human titles of the commands (cycle 35 play-test): what the window says for a command it previews
# ("Preview of Place device"), verb first, sentence case; the key is `command-<name>` with every `.` of the
# name as `-` (`command_title`). The parameters, the describe text and the scripts keep the command ID.
command-assembly-attach = Attach assembly
command-bond-create = Bond potentials
command-bus-logical_net-create = Create logical bus network
command-bus-physical_net-create = Create physical bus network
command-box-black = Draw black box
command-box-reshape = Reshape box
command-box-structure = Draw structure box
command-cable-create = Create cable
command-check-unwaive = Remove waiver
command-check-waive = Waive finding
command-merge-resolve = Resolve merge conflicts
command-conductor-add = Add conductor
command-connection-delete = Delete wire
command-connection-draw = Draw wire
command-connection-draw_many = Draw wires
command-connection-draw_to_ip = Draw wire to interruption point
command-connection-route = Route wire
command-device-create = Create device
command-device-link = Link device
command-device-place = Place device
command-edit-delete = Delete
command-edit-paste = Paste
command-external_ref-create = Create external reference
command-fields-set = Set property
command-fields-set_many = Set properties
command-ip-arrange = Arrange interruption points
command-ip-continue = Continue interruption point
command-ip-place = Place interruption point
command-item-place = Place item
command-junction-edit = Edit junction
command-library-embed = Embed library item
command-library-update = Update library item
command-macro-insert = Insert macro
command-marker-place = Place wire marker
command-numbering-renumber = Renumber
command-page-create = Create page
command-page-delete = Delete page
command-page-renumber = Renumber pages
command-page-reorder = Reorder pages
command-part-assign = Assign part
command-parts-import = Import parts
command-plc-bind_channel = Bind PLC channel
command-plc-config_project-create = Create PLC configuration project
command-plc-set_module_part = Set PLC module part
command-plc-set_profile = Set PLC address profile
command-plc-set_start = Set PLC module start
command-potential-define = Define potential
command-project-assets-purge = Purge unused assets
command-project-checks-set = Set checks
command-project-company-set = Set company
command-project-drawing-set = Set drawing data
command-project-frames-set = Set frames
command-project-languages-set = Set languages
command-project-settings-set = Set project settings
command-project-supply-set = Set supply
command-rep-exchange_symbol = Exchange symbol
command-rep-join_device = Join device
command-rep-move = Move
command-rep-set_anchor_policy = Set anchor policy
command-rep-transform = Rotate or mirror
command-report_block-create = Create report block
command-revision-approve = Approve revision
command-revision-create = Create revision
command-revision-release = Release revision
command-revision-set_author = Set revision author
command-revision-withdraw = Withdraw revision
command-section-create = Create section
command-structure-create_node = Create structure node
command-terminal-add = Add terminal
command-terminal-designate = Designate terminal
command-terminal-insert = Insert terminal
command-terminal-move = Move terminal
command-valve-add = Add valve
command-part-edit_copy = Edit part
command-layout-space-create = Create layout space
command-layout-space-delete = Delete layout space
command-layout-enclosure-create = Place enclosure
command-layout-enclosure-delete = Delete enclosure
command-layout-carrier-add = Add carrier
command-layout-carrier-edit = Edit carrier
command-layout-carrier-delete = Delete carrier
command-layout-place = Place in layout
command-layout-move = Move placement
command-layout-remove = Remove placement
command-layout-lock = Lock placement

# WP-S4.01 part 3 (D9)
command-page-surface_view-set = Set panel view
command-layout-place_assembly = Place assembly
command-layout-auto_fill = Auto-fill rail
command-layout-enclosure-create_from_part = Create enclosure from part
command-layout-space-edit = Rename layout space
command-layout-enclosure-edit = Move enclosure
command-layout-surface-edit = Edit plate

# Actions.
action-file-open = Open…

# The canvas tools (WP-S2.03 part 2).
action-tool-select = Select
action-tool-measure = Measure
action-cp-next = Next connection point
action-cp-previous = Previous connection point
action-cp-next-unconnected = Next unconnected connection point
action-canvas-activate = Click at the grid cursor
action-tool-back = Tool step back
action-canvas-press = Press on the canvas
action-canvas-release = Release on the canvas
action-canvas-dwell = Pointer at rest
action-canvas-preview-snapshot = Link preview snapshot

# The place-and-arrange tools (WP-S2.04 part 1).
action-tool-place = Symbol…
action-tool-place-part = Device from part…
action-place-pick = Pick a symbol to place
action-picker-filter = Filter the picker
action-picker-row = Move the picker's row
action-picker-choose = Choose the picker's row

# WP-S2.18 part 1: the Join chooser (D2).
action-place-join = Join device…
action-chooser-row = Move the chooser's row
action-chooser-type = Type ahead in the chooser
action-chooser-choose = Choose the chooser's row
action-strip-new = Terminal strip…
action-tool-rotate-cw = Rotate clockwise
action-tool-rotate-ccw = Rotate counter-clockwise
action-tool-mirror-x = Mirror left to right
action-tool-mirror-y = Mirror top to bottom
action-tool-next-variant = Next variant
action-tool-next-rep-type = Next representation type
action-tool-move = Move
action-tool-finish = Finish
action-text-input = Type into the tool
action-edit-delete-selection = Delete
action-rep-exchange = Change symbol…
tool-place = Place
tool-exchange = Change symbol
hint-place = Click or { $p0 } to place; { $p1 }, { $p2 }, { $p3 } and { $p4 } turn it; { $p5 } ends
hint-choose-strip = Choose the terminal strip in the list; Esc cancels
hint-place-proposals = Click or { $p0 } to place and connect the proposed wires; Alt+Click or { $p1 } places without them; { $p2 } tries the next target; { $p3 } ends
hint-select-move = Release, click or { $p0 } to move; { $p1 } cancels
hint-exchange = Click or { $p0 } to change the symbol; { $p1 } cancels
move-readout = dx { $p0 } dy { $p1 } mm
place-target-new = new { $p0 }
place-target-join = join { $p0 }
place-target-function = function
place-target-part = part { $p0 }
place-target-terminal = { $p0 } terminal { $p1 }
place-target-box = box
announce-place = Place { $p0 }: { $p1 }, variant { $p2 }, at { $p3 }
announce-place-no-cell = Place { $p0 }: { $p1 }, variant { $p2 }
picker-title = Insert symbol
picker-title-device = Insert device
picker-title-exchange = Change symbol
picker-filter-hint = Filter by key, part number or function
picker-badge-embedded = embedded
picker-badge-starter = starter
picker-part-function = { $p0 } of { $p1 } { $p2 }
picker-main = main
picker-empty = Nothing matches the filter
picker-empty-parts = No parts in this project yet
picker-empty-strips = No terminal strips in this project yet
picker-empty-strips-text = A terminal goes into a terminal strip. Create one here, then choose it.
picker-new-strip = New terminal strip…
picker-empty-black-box = No black-box symbols or parts to place
picker-empty-page = Nothing can be placed on this page
reason-letter-needed = Give a letter code: the symbol's category maps to none
reason-one-rep = Select exactly one representation
reason-nothing-to-move = Nothing movable is selected
reason-place-only = Only while placing a symbol
reason-place-stacked = An identical symbol already stands here; move the pointer to place another
reason-place-wire-off-sheet = Not placed: this wire runs off the sheet here; move its ends onto the sheet first
reason-nothing-selected = Nothing is selected
error-wire-reference = No wire of the page shown runs "{ $p0 }"; name one as the page text does: wire:<end> -> <end>
error-wire-ambiguous = Several wires of the page shown run "{ $p0 }"; click the one you mean
reason-not-on-page = The selection is drawn on another page; go to that page to delete it
reason-no-picker = The picker is not open

# WP-S2.18 part 1.
reason-no-chooser = No chooser is open
reason-join-not-auxiliary = Only a contact joins an existing device as an auxiliary function; this symbol is not one
chooser-join-question = Join device
chooser-new-device = New device
chooser-kind-slot = slot { $p0 }
chooser-kind-auxiliary = auxiliary
hud-keys-place-join = { $p0 } rotates · { $p1 } variant · { $p2 } Join device… · { $p3 } done
error-join-device = Cannot join { $p0 }: { $p1 }
error-join-not-device = { $p0 } is not a device
error-join-no-slot = { $p0 } has no unplaced { $p1 } function; device={ $p0 } adds one as an auxiliary function
error-join-slots = { $p0 } has { $p1 } unplaced { $p2 } functions; name one of { $p3 }
error-join-unknown-slot = { $p0 } has no unplaced { $p2 } function in slot { $p1 }; its unplaced slots: { $p3 }
error-join-none = none
reason-rep-type-not-allowed = The page's type does not take this symbol

# FT-896: the way out with its key from the keymap; `-unbound` when the action has none.
reason-black-box-tool = This part is placed as a black box: use the Black box tool ({ $p0 })
reason-black-box-tool-unbound = This part is placed as a black box: use the Black box tool
reason-no-symbol-for-function = No symbol for this function on this page
reason-no-catalogue = Nothing can be placed on this page
reason-unknown-symbol = No such symbol, part or function for this page
reason-no-strip = No terminal strip to add to: create one (Insert, Terminal strip). A terminal designation numbers a terminal;              a device's tag is set in its properties (Frozen tag)
reason-designation-needed = Give the terminal's designation
reason-choose-strip = Choose a terminal strip in the list first, or press Esc
reason-ip-exchange = An interruption point keeps its symbol
reason-no-next-rep-type = No other symbol draws this function on this page
reason-no-picker-row = The picker shows no row
reason-not-editable = Symbols are placed on drawn pages of a project that can be changed
banner-host-commit = { $p0 } failed: { $p1 }
tool-strip-new = New strip
reason-page-type-no-symbols = This page type takes no symbols or wires: draw on a schematic page

# FT-892: the same, naming the page's kind.
reason-page-type-no-symbols-divider = A section divider takes no symbols or wires: draw on a schematic page
reason-page-type-no-symbols-title = A title page takes no symbols or wires: draw on a schematic page
reason-page-type-no-symbols-contents = A table of contents takes no symbols or wires: draw on a schematic page
reason-page-type-no-symbols-report = A report page takes no symbols or wires: draw on a schematic page
reason-page-type-no-symbols-panel = A panel layout page takes no symbols or wires: draw on a schematic page

# WP-S2.04 part 2 (wiring; the strip chooser of FT-421). Begin.
picker-title-strip = Choose a terminal strip
picker-title-black-box = Insert black box
picker-strip-row = { $p0 }  next terminal { $p1 }
picker-strip-next = next terminal { $p0 }
action-tool-wire = Wire
action-canvas-activate-alt = Click with Alt at the grid cursor
action-tool-next-proposal = Next autoconnect proposal
action-tool-connect-on-move = Connect while moving
action-tool-route-edit = Edit route
action-junction-open = Junction sequence…
action-junction-member = Move a junction member
action-junction-style = Junction style
action-junction-wiring = Junction wiring
action-project-crossings = Show crossings as hops
tool-wire = Wire
tool-route_edit = Edit route
hint-wire-armed = Click or { $p0 } on a connection point, a junction or a wire to start
hint-wire-routing = Click a connection point; { $p0 } removes the last bend; { $p1 } cancels
hint-wire-direct = No clear route; drawn direct
hint-route-edit = { $p0 } picks the segment, the arrow keys move it, { $p1 } commits; { $p2 } cancels
announce-wire-routing = Wire: routing from { $p0 }. Click a connection point; { $p1 } removes the last bend; { $p2 } cancels
wire-readout = { $p0 } mm
reason-connect-on-place = Proposals connect when a symbol is placed; a move never adds a connection
reason-one-connection = Select exactly one connection
reason-one-junction = Select exactly one junction
reason-no-wire-kind = The Wire tool does not draw this page type's connections yet
reason-no-junction-editor = The junction sequence editor is not open
reason-junction-member = No such member position
junction-title = Junction { $p0 }
junction-style = Style
junction-style-dot = Dot
junction-style-bevel = Bevel
junction-wiring = Wiring
junction-wiring-star = Star
junction-wiring-chain = Chain
junction-wiring-default = Style default
junction-sequence = Sequence (Alt+Up, Alt+Down)
junction-wires = Physical wires
junction-wire-row = { $p0 }: { $p1 } to { $p2 }
junction-orient = Orientation { $p0 } (R)

# WP-S2.04 part 2. End.
# WP-S2.04 part 3 (interruption points and black boxes). Begin.
action-tool-ip = Interruption point
action-tool-black-box = Black box…
action-tool-ip-direction = Interruption point direction
action-tool-star-source = Star source
action-ip-name = Interruption point name

# FT-1022 (ID-1107): the name field's tooltip names the keys that end it.
action-ip-name-hint = Enter or Tab ends the name and gives the keys back to the tool
action-ip-external = External reference
action-ip-continue-on = Continue on page…
action-ip-arrange-selected = Arrange interruption points
action-box-grip = Move a black box grip
action-box-add-cp = Add connection point
tool-ip = Interruption point
tool-black_box = Black box
hint-ip = Click or { $p0 } to place; { $p1 } changes the direction; type a name; { $p2 } ends
hint-ip-external = Click or { $p0 } to place the external connection point; type a name; { $p1 } ends

# FT-896: the Black box tool takes two corners; the armed hint says so before the first click.
hint-box-armed = Click the box's first corner, then its opposite corner; { $p0 } ends
hint-box-outline = Click or { $p0 } at the opposite corner; { $p1 } starts again
hint-box-dropped = Click or { $p0 } at the first corner; { $p1 } again leaves the tool
reason-no-ip-symbol = This page's library has no interruption-point symbol
reason-ip-tool-only = Only while placing an interruption point

# FT-822: a refused interruption point (a prevent rule) keeps its ghost; the HUD and a toast say why.
reason-ip-not-placed = The interruption point was not placed
notice-ip-not-placed = Not placed: { $p0 }

# FIXPT41: the drop on a connection point the IP tool cannot honour (FT-946: an external point on a pin connects).
notice-ip-on-ip = Not placed: the interruption point { $p0 } is already here; place it on a pin or a free grid point

# FT-948 (ID-1150): a drop whose connection point lies on a junction or inside a wire, which it would not join.
notice-ip-on-wire = Not placed: the point would sit on a wire without joining it; place it on a pin or a free grid point

# H40 item 6: EW-CORE-052 in words: what is in the way (the existing source's page) and the way out.
notice-ip-second-source = Not placed: { $p0 } already has a source on page { $p1 }; press { $p2 } for a target, or mark one source as the star

# Cycle-40 review: the same, when `tool.ip_direction` has no key: the action by its title.
notice-ip-second-source-unbound = Not placed: { $p0 } already has a source on page { $p1 }; use { $p2 } for a target, or mark one source as the star
reason-external-never-pairs = An external connection point never pairs, so it has no star source
error-no-external = No external reference is "{ $p0 }"; name one as device:connection, optionally after its location and " / " (+J-E-206 / JE-N1000-03:L). The project's: { $p1 }
external-none = none (create one with Project ▸ External reference…)
# FT-1131 (ID-1229 item 7): the same, when `external_ref.new` has a key: the key from the keymap.
external-none-key = none (create one with Project ▸ External reference…, { $p0 })
reason-external-ambiguous = Several external references print this text: name one by its ID
reason-select-one-ip = Select exactly one interruption point
reason-select-ips = Select at least two interruption points

# FT-896: today's menu path and the key from the keymap; `-unbound` when `tool.place` has none.
reason-not-black-box = This is not a black box: place it with Draw ▸ Symbol… ({ $p0 })
reason-not-black-box-unbound = This is not a black box: place it with Draw ▸ Symbol…
reason-one-black-box = Select exactly one black box
reason-grip-not-axis = Only a horizontal or vertical edge moves
reason-box-too-small = A black box keeps a width and a height
reason-box-inside-out = A grip cannot pass the opposite side of the black box
reason-no-grip = The black box has no such grip
reason-cp-off-outline = Put the grid cursor on the black box's outline
reason-ip-end-grid = End the wire on an empty grid point to end it in an interruption point
reason-ip-end-segment = Draw a segment before ending the wire in an interruption point
reason-ip-symbol = The interruption-point symbol has not exactly one connection point

# WP-S2.04 part 3. End.
# WP-S2.04 part 4b (text, text edit and graphic tools). Begin.
submenu-graphic = Graphic
action-tool-text = Text
action-tool-graphic = Graphic
action-text-bind = Text binding
action-text-edit = Edit text
action-strip-bridge = Bridge terminals
graphic-line = Line
graphic-polyline = Polyline
graphic-rect = Rectangle
graphic-circle = Circle
graphic-arc = Arc
graphic-polygon = Polygon
graphic-anticlockwise = anticlockwise
graphic-clockwise = clockwise
tool-text = Text
tool-text_edit = Edit text
tool-graphic = Graphic
hint-text-armed = Click or { $p0 } where the text goes; { $p1 } ends
hint-text-typing = Type the text; Enter places it; Esc cancels
hint-text-edit = Type the new text; Enter changes it; Esc cancels
hint-graphic-armed = Click or { $p0 } at the first point; { $p1 } ends
hint-graphic-drawing = Click or { $p0 } at the next point; { $p1 } or a double click finishes a polyline or a polygon; { $p2 } removes the last point
reason-text-tool-only = Choose the Text tool first
reason-text-bound = This text shows a project or page field; change that field instead
reason-select-one-text = Select exactly one free text
reason-bridge-selection = Select two or more terminals of one strip
reason-bridge-strips = The selected terminals belong to more than one strip; select terminals of one strip
reason-bridge-no-shaft = Each terminal needs a bridge shaft on its part or a side-1 connection point
reason-bridge-exists = These terminals are bridged already

# WP-S2.07 part 1: the strip editor (§6.9, §6.10).
action-strip-edit = Strip editor
action-strip-insert = Insert terminal
action-strip-delete-rows = Delete rows
action-strip-move = Move terminals
action-strip-add-accessory = Add accessory
action-strip-unbridge = Remove bridge
reason-not-strip-editor = Open the strip editor first
reason-no-terminal-def = The project has no terminal definition to insert
reason-strip-mixed-rows = Mark only terminals or only accessories
reason-strip-move-none = The terminals cannot move further
reason-no-accessory-part = The project has no part for this accessory; embed one first
reason-unbridge-none = No bridge joins the selected terminals
reason-unbridge-split = Unbridge from an end of the bridge, or the whole bridge
reason-strip-delete = Deleting terminals also removes their wires and bridges
error-accessory-part-ambiguous = Several parts fit: { $p0 }; give part
table-accessory-row = accessory row
table-pos-move = move rows with Alt+Up and Alt+Down
table-no-bridge = the terminal has no bridge
toast-strip-inserted = Inserted { $p0 } terminals before { $p1 }
toast-strip-inserted-one = Inserted 1 terminal before { $p1 }
toast-strip-appended = Inserted { $p0 } terminals at the end of { $p1 }
toast-strip-appended-one = Inserted 1 terminal at the end of { $p1 }
toast-strip-moved = Moved { $p0 } terminals of { $p1 }
toast-strip-moved-one = Moved 1 terminal of { $p1 }
toast-accessory-added = Added { $p0 } after { $p1 }
toast-accessory-added-first = Added { $p0 } at the start of { $p1 }
toast-accessory-added-before = Added { $p0 } before { $p1 }
toast-accessories-removed = Removed { $p0 } accessories
toast-accessories-removed-one = Removed 1 accessory
toast-unbridged = Removed { $p0 } bridges
toast-unbridged-one = Removed 1 bridge
chooser-accessory-part = Choose the part

# WP-S2.07 part 2: swap cores, and Delete and F2 in navigator trees (§7, FT-490).
action-cable-swap-cores = Swap cores
action-navigator-delete = Delete rows
action-navigator-rename = Rename
reason-not-cables = Open the Cables navigator first
reason-swap-two-cores = Mark two cores of one cable
reason-swap-cores-unwired = Neither core has a connection
reason-nav-not-deletable = These rows stand for nothing that can be deleted
reason-nav-not-renamable = This row has no name to rename
reason-nav-delete = Deleting also removes what depends on the rows
toast-cores-swapped = Swapped cores { $p0 } and { $p1 } of { $p2 }

# G-PP triage row 7: Draw ▸ Bond.
action-bond-draw = Bond
submenu-bond = Bond
bond-kind-pelv_0v_pe = 0 V–PE bond (PELV)
bond-kind-fe_pe = FE–PE bond
bond-kind-equipotential = Equipotential bond
bond-kind-shield = Shield bond
reason-bond-selection = Select the one wire between the two potentials: the bond joins its two ends
reason-bond = A bond is previewed first
error-bond-end = No connection point of the page shown is printed "{ $p0 }"; name it as the page text does
error-bond-end-ambiguous = Several connection points of the page shown are printed "{ $p0 }"
error-bond-same-end = Both ends of the bond are "{ $p0 }"; name two different connection points
# FT-1202 (ID-1270): Draw ▸ Define potential…, beside Bond.
action-potential-draw = Define potential…
reason-potential-point = Select one wire, junction or terminal
error-potential-type = "{ $p0 }" is not a potential type; type L1, L2, L3, L, N, PE, PEN, FE, L+, L-, M or signal.
error-potential-voltage = "{ $p0 }" is not a voltage; type it as 24 V DC, 230 V AC or 24.

# WP-S2.04 part 5 (images, §11).
action-tool-image = Image…
action-image-pick = Image asset
action-image-file = Image file
prompt-open-image-title = Insert an image
error-image-file = Cannot read the image file { $p0 }: { $p1 }.
error-image-too-large = The image { $p0 } is larger than { $p1 } KiB (the limit of an image the project stores).
error-image-refused = The image { $p0 } is not accepted: { $p1 }.
tool-image = Image
hint-image-picking = Choose an image asset of the project first
hint-image-armed = Click or { $p0 } at the first corner; { $p1 } ends
hint-image-placing = Click or { $p0 } at the opposite corner; { $p1 } or { $p2 } drops the first corner
reason-image-tool-only = Choose the Image tool first
error-image-asset-missing = The project holds no image asset { $p0 }.

# WP-S2.04 part 4b: the Wire marker tool, the Structure box tool and the marker mode.
submenu-wire-markers = Wire markers
action-tool-marker = Wire marker
action-marker-fields = Marker fields
action-marker-csa = Cross-section
action-marker-colours = Colours
action-tool-structure-box = Structure box
action-structure-box-fields = Structure box fields
action-structure-box-style = Style
action-structure-box-caption = Caption
action-structure-box-location = Location
action-project-marker-mode = Wire markers
marker-mode-per_wire = As set per wire
marker-mode-all = All wires
marker-mode-deviations = Deviations
marker-mode-explicit = Explicit overrides
marker-mode-none = None
box-style-dotted = Dotted
box-style-dash_dot = Dash-dot
tool-marker = Wire marker
tool-structure_box = Structure box
marker-crossed = { $p0 } crossed
hint-marker-armed = Press and drag (or { $p0 }, move, { $p0 }) across the wires to mark; { $p1 } ends
hint-marker-crossing = Release (or { $p0 }) to take the crossed wires
hint-marker-typing = Set the cross-section and colours if wanted; { $p0 } places the marker; { $p1 } cancels
hint-structure-box-armed = Click or { $p0 } at the first corner; { $p1 } ends
hint-structure-box-outline = Click or { $p0 } at the next corner; { $p1 } or a double click closes the box (two corners: a rectangle)
hint-structure-box-typing = Set the style, caption, location and external point; { $p0 } places the box; { $p1 } cancels
reason-marker-tool-only = Choose the Wire marker tool first
reason-structure-box-tool-only = Choose the Structure box tool first

# WP-S2.04 part 4b. End.
tool-select = Select
tool-measure = Measure
hint-select = Click to select; drag for a rubber band
hint-measure-armed = Click or { $p0 } at the first point
hint-measure-measuring = Click or { $p0 } at the second point; { $p1 } starts again
measure-label = dx { $p0 } dy { $p1 } mm, { $p2 } mm
announce-tool = { $p0 }: { $p1 }
announce-selected = { $p0 } selected
# WP-S2.08 D13: one selected record's live description (tag, kind, folio; a wire's number, potential and ends; a page
# item's kind and text).
announce-device = { $p0 }, { $p1 }, folio { $p2 }
announce-device-unplaced = { $p0 }, { $p1 }
announce-device-connected = { $p0 }, { $p1 }, { $p2 }
announce-device-kind = Device
announce-cps = { $p0 } connection points
announce-cps-one = 1 connection point
announce-wires = { $p0 } wires
announce-wires-one = 1 wire
announce-wire = { $p0 }, { $p1 }, { $p2 } to { $p3 }
announce-wire-no-potential = { $p0 }, { $p1 } to { $p2 }
announce-wire-unnumbered = Unnumbered wire
announce-end-junction = a junction
announce-item = { $p0 }, { $p1 }
announce-item-graphic = Graphic
announce-item-free_text = Text
announce-item-wire_marker = Wire marker
announce-item-image = Image
announce-item-surface_view = Surface view
announce-item-unknown = Page item
preview-chip = Preview of { $p0 }
preview-chip-stale = Preview of { $p0 } (stale)
preview-chip-failed = Preview of { $p0 } failed: { $p1 }
preview-chip-change = { $p0 } → { $p1 }
link-preview-title = { $p0 }
# FT-1123 (ID-1225): the line under the link preview's title, with the key of `nav.follow`.
link-preview-go = Click the link or press { $p0 } to go there

# WP-S2.17 part 5 (FT-390): the link preview's accessible node.
link-preview-node = Link preview: page { $p0 }, cell { $p1 }
link-preview-node-page = Link preview: page { $p0 }

# FT-902: the tooltip of a page link under the pointer, with its target's cell or folio.
tip-link = Go to { $p0 }

# FT-1092 (ID-1184; doc 06 §3.10): what the Pending chips and the Messages header's "pending n" count.
tip-pending = Findings on the page you are editing that wait until you leave it or run a check
canvas-cursor-live-cp = , connection point { $p0 }
reason-no-cp = The page has no connection point
reason-no-unconnected-cp = The page has no unconnected connection point
reason-no-cursor = No grid cursor is shown (the arrow keys show it)
reason-no-preview = No link preview is shown
action-file-open-recent = Open recent project
action-file-save = Save
action-file-save-as = Save as…
action-file-export-pdf = Export PDF…

# WP-S3.07 part 3 (D10, ruling 4 of ID-1180): the print pre-dialog.
action-file-print = Print…
action-print-set = Set the print options
action-print-page = Show a print preview page
action-print-send = Print the print set
action-file-close = Close project
action-app-quit = Quit
action-edit-undo = Undo
action-edit-redo = Redo
action-page-next = Next page
action-page-previous = Previous page
action-page-first = First page
action-page-last = Last page
action-page-goto = Go to page…
action-view-fit-page = Fit page
action-view-zoom-in = Zoom in
action-view-zoom-out = Zoom out
action-view-zoom-actual = Actual size
action-view-pan-left = Pan left
action-view-pan-right = Pan right
action-view-pan-up = Pan up
action-view-pan-down = Pan down
action-view-grid = Grid
action-view-frame-stats = Frame statistics
action-view-cpu-canvas = Use the CPU renderer for the canvas
banner-gpu-init = The GPU canvas could not start ({ $p0 }); the canvas draws on the CPU.
banner-gpu-lost = The GPU canvas stopped ({ $p0 }); the canvas draws on the CPU for the rest of this session.
banner-gpu-no-software-adapter = --no-gpu: no software adapter was found, so the window uses { $p0 }; the canvas draws on the CPU.
action-view-zoom-to = Zoom to
action-view-pan = Pan the canvas
action-view-canvas-resize = Resize the canvas
action-canvas-pointer = Point at the canvas
action-canvas-snapshot = Write canvas snapshot
submenu-pan = Pan
reason-zoom-max = The zoom is at its largest (5000 %)
reason-zoom-min = The zoom is at its smallest (5 %)
status-cursor = x { $p0 } y { $p1 } mm
status-zoom = { $p0 } %
status-grid-qet = QET 10 px
status-grid-mm = { $p0 } mm
status-grid-off = Grid off
canvas-name = Page { $p0 }, { $p1 }
canvas-description = { $p0 }, zoom { $p1 }
canvas-stats-fps = { $p0 } fps
canvas-stats-frame = frame p99 { $p0 } ms
canvas-stats-phase = { $p0 } p99 { $p1 } ms
canvas-stats-tiles = tiles pending { $p0 }
canvas-stats-backend = backend { $p0 }
message-no-canvas = The page cannot be shown: { $p0 }
action-view-ui-scale = UI scale
action-view-theme = Theme
action-view-density = Density
action-view-min-font = Minimum text size
action-view-reduce-motion = Reduce motion

# WP-S2.17 part 2: the command bar (doc 16a §5).
action-view-command-tab = Command tab
action-view-command-bar = Collapse command bar
submenu-command-tab = Command tab
group-project = Project
group-parts = Parts
group-standards = Standards
group-import = Import
group-pages = Pages
group-order = Order
group-go = Go
group-select = Select
group-place = Place
group-connect = Connect
group-annotate = Annotate
group-arrange = Arrange
group-run = Run
group-findings = Findings
group-numbering = Numbering
group-trace = Trace
group-pdf = PDF
group-reports = Reports
group-lists = Lists
group-jobs = Jobs
group-zoom = Zoom
group-show = Show
group-panels = Panels
group-layout = Layout
group-appearance = Appearance
bar-properties = Properties
bar-structure = Structure
bar-import-parts = Import parts
bar-company-standard = Company standard
bar-import-qet = Import QET
bar-move-up = Move up
bar-move-down = Move down
bar-go-to = Go to
bar-follow-reference = Follow reference
bar-device = Device
bar-wire = Wire
bar-interruption = Interruption
bar-rotate = Rotate
bar-mirror = Mirror
bar-renumber = Renumber
bar-check-project = Check project
bar-navigators = Navigators
bar-workspaces = Workspaces
next-step-draw = Check project
next-step-check = Export PDF
commandbar-name = Command bar
commandbar-tabs = Command tabs
commandbar-app = ElektraWatt
commandbar-menu = Menu
commandbar-quick-access = Quick access
commandbar-search = Search commands and devices
commandbar-search-key = Ctrl K
commandbar-more = More
commandbar-next = Next step
commandbar-variants = Variants
hud-name = Tool

# FT-964 (ID-1150): the place of a key in a hint, a HUD prompt or a toast when the keymap binds its action to none.
key-none = (no key)

# FT-894: the pick stack in words.
hud-stack = { $p0 } objects overlap here · click again to select the next
hud-selected = { $p0 } selected · { $p2 } deletes · { $p1 } rotates
hud-keys-place = { $p0 } rotates · { $p1 } variant · { $p2 } done
hud-place-wires = connects { $p0 }; Alt+Click or { $p1 } places without

# FIXPT41: the IP tool's ghost on a pin.
hud-ip-connects = connects to { $p0 }

# FT-1023 (ID-1107): the proposal's base is taken; the HUD and the placement toast say the name it gets.
hud-ip-taken = { $p0 } is taken, so this point is { $p1 }.

# FT-899: the IP tool's role in words, so "target" and "star source" explain themselves.
hud-ip-source = source: feeds its targets
hud-ip-target = target: fed by its source
hud-ip-undirected = undirected: pairs either way
hud-ip-star = star source: feeds every point of the name

# W42b: the IP tool's ghost label of a starred point ("<name> <direction>" otherwise).
ghost-ip-star = { $p0 } star source
hud-tool-options = Tool options
zoom-control = Zoom
action-tools-palette = Command palette
action-help-shortcuts = Keyboard shortcuts
action-help-about = About ElektraWatt
action-command-run = Run model command
action-view-snapshot = Write page snapshot
action-ui-describe = Describe the user interface
action-script-expect-refused = Expect a refusal
action-ui-dismiss = Dismiss popup or dialog

# Why an action is disabled.
reason-read-only = The project is open read-only
reason-point-outside-drawing = That point lies too far away to draw; check the page's scale and datum.
reason-row-computed = This value is computed from the drawing; it cannot be edited here.
reason-action-not-offered = That action does not apply to this row; its menu shows the ones that do.
props-refused-read-only = This row cannot be edited: { $p0 }.
reason-nothing-to-undo = Nothing to undo
reason-nothing-to-redo = Nothing to redo
reason-no-project = No project is open
reason-no-tag-proposal = No refused tag edit has a proposal to accept
reason-no-pages = The project has no pages
reason-no-parts-list = The project has no parts list: it needs a parts list report block
reason-no-next-page = This is the last page
reason-no-previous-page = This is the first page
reason-no-recent = There are no recent projects
reason-no-page = No page is shown
reason-parameters-later = Parameter forms come with a later version; run it from a script

# WP-S2.08 D5 (a): an action with a file or reference parameter in the palette.
reason-palette-needs-file = Needs a file or a selection: use its menu

# Why a project is read-only.
readonly-requested = opened read-only on request
readonly-locked = another session holds the project
readonly-not-writable = the project cannot be written
readonly-integrity = integrity findings
readonly-format = a file format this version does not write
readonly-lock-lost = the lock was taken over
readonly-recovery = unsaved changes from an earlier session

# Banners and messages.
banner-recovery = This project has unsaved changes from a session that ended unexpectedly. It is open read-only. File, Discard unsaved changes of the earlier session deletes them and opens the project for editing; to recover them, reopen the project and choose Recover in the recovery dialog.
banner-read-only = This project is open read-only: { $p0 }.
banner-findings = { $p0 } integrity findings were found when the project was opened.
banner-settings-bad = The settings file could not be read; defaults are used and the old file was kept as { $p0 }.
banner-settings-write = The settings could not be written.
banner-recent-removed = { $p0 } could not be opened and was removed from the recent projects.
banner-dismiss = Dismiss
message-no-snapshot = The snapshot needs the page renderer, which this runner does not have.
message-expected-refusal = expected a refusal; the step was accepted
message-expect-refused-inner = script.expect_refused needs `action`, an action ID, and `params` as an object
error-unknown-action = Unknown action `{ $p0 }`.
error-missing-param = Missing parameter `{ $p0 }`.
error-bad-param = Parameter `{ $p0 }` is not valid.
error-unknown-alias = No record has the alias `{ $p0 }` in this session.
error-params-object = The parameters are a JSON object.
error-open = Cannot open { $p0 }: { $p1 }.
error-save = Saving failed: { $p0 }.
error-save-as = Save as failed: { $p0 }.
error-export = Export failed: { $p0 }.
error-write = The file could not be written.
error-unsaved = The project has unsaved changes; the step needs "discard": true.
error-unsaved-at-end = The script ended with unsaved changes; their journal was kept for recovery. End a script that edits with              "app.quit" and "discard": true, or save first.
banner-no-file-dialog = The file dialog is not available: it needs the xdg-desktop-portal service on the session bus. Start the              portal for your desktop (for example xdg-desktop-portal-gtk or -kde) or open the project as an argument.
error-no-folio = No page has the folio { $p0 }.
error-part-fndef-missing = The part needs the function definitions { $p0 }, which neither the project nor the starter library holds.
error-command = { $p0 }

# FT-779: a refused parameter named by its field's words: the command's title, the field, the refusal.
error-command-field = { $p0 }: { $p1 }: { $p2 }
error-param-item = { $p0 }, item { $p1 }
error-param-whole = The parameters
error-render = The page could not be drawn: { $p0 }
openerr-io = the files cannot be read
openerr-not-dir = it is not a project directory
openerr-format = the file format is not supported
openerr-migration = it needs a migration
openerr-busy = a save is in progress
openerr-no-project = it holds no project
openerr-model = the project data is not valid
openerr-takeover = the lock could not be taken over
openerr-exists = the directory already holds a project
openerr-other = the project cannot be opened
saveerr-read-only = the project is read-only
saveerr-lock-lost = the lock was taken over
saveerr-external = the project changed on disk
saveerr-failed = a file could not be written; nothing changed on disk
saveerr-incomplete = the save is incomplete and completes on the next open
saveerr-pending = an earlier save is still incomplete
saveerr-generation-limit = this project has been saved the greatest number of times its format allows; save it to a new folder (Save as…), which starts the count again
saveerr-other = the project could not be written

# The window, the start page, the page view.
window-title = { $p0 } - ElektraWatt
window-title-start = ElektraWatt
start-title = ElektraWatt
start-open = Open…
start-recent = Recent projects
start-update-check = This version has no update check
pages-title = Pages
page-rendering = Rendering page { $p0 }…
page-no-pages = The project has no pages.

# The status bar.
status-hint-start = No project open
status-page = { $p0 } ({ $p1 } of { $p2 })
status-saved = saved
status-unsaved = *
status-read-only = read-only: { $p0 }
status-language = EN

# The palette and the dialogs.
palette-title = Command palette
palette-hint = Type a command…
palette-no-match = No matching command
palette-form-hint = Enter applies · Esc goes back
dialog-close = Close
dialog-save = Save
dialog-discard = Discard
dialog-cancel = Cancel
dialog-go = Go
confirm-title = Save changes to { $p0 } before closing?
dialog-dont-save = Don't save
confirm-text = The project has unsaved changes.
goto-title = Go to page or cell
goto-hint = Folio or cell
shortcuts-title = Keyboard shortcuts
about-title = About ElektraWatt
about-version = Version { $p0 }
about-page-view = Page view: CPU renderer
about-privacy = This version has no update check and sends no crash reports.
about-licence = Licence: GPL-3.0-or-later
about-repository = Repository: { $p0 }
prompt-open-title = Open a project directory
prompt-save-as-title = Choose a new, empty directory
prompt-export-title = Export PDF
prompt-pdf-name = project.pdf

# The text of the describe output.
describe-menu = menu { $p0 }
describe-item = item { $p0 } "{ $p1 }"

# Part 2: menus, actions and reasons.
submenu-workspace = Workspace
action-file-new = New project…
action-file-open-sample = Open sample project
action-file-import-qet = Import QElectroTech project…
action-parts-import = Parts table…
action-file-discard-journal = Discard unsaved changes of the earlier session…
action-view-workspace-load = Load workspace
action-view-workspace-save = Save layout as…
action-view-workspace-reset = Reset layout
action-tools-company-standard = Company standard wizard…
action-help-documentation = Documentation
action-help-tutorials = Tutorials
action-help-first-run = First-run setup…
action-help-notices = Third-party notices
action-settings-set = Change a setting
action-first-run-set = Answer a first-run question
action-first-run-next = First run: next step
action-first-run-back = First run: previous step
action-first-run-finish = First run: finish
action-wizard-set = Wizard: set a field
action-wizard-next = Wizard: next step
action-wizard-back = Wizard: previous step
action-wizard-goto = Wizard: go to a step
action-wizard-reset = Wizard: start over
action-wizard-cancel = Wizard: close
action-wizard-finish = Wizard: create the basic project
action-wizard-apply-check-scheme = Wizard: apply the check scheme
reason-wizard-finished = The basic project is written; close the wizard to start another one
reason-wizard-not-finished = Create the basic project first
reason-check-scheme-written = The check scheme is applied already
reason-human-only = Only a person applies this, in the window: it decides what the checks report (ADR-0008)
reason-human-only-history = Only a person undoes or redoes this step, in the window: it holds a human-only command that decides what the checks report (ADR-0008 amendment 3)
banner-check-scheme-applied = The check scheme { $p0 } was applied to { $p1 }.
wizard-apply-check-scheme = Apply the check scheme
wizard-kept-apply = kept: apply after finishing
wizard-close = Close
confirm-check-scheme-title = Apply the check scheme?
confirm-check-scheme-text = Apply the check scheme { $p0 } to the basic project? The check scheme decides which rules the release gate runs and what the checks report.
dialog-apply = Apply
reason-no-journal = The project has no journal of an earlier session
reason-no-wizard = The company-standard wizard is not open
reason-no-first-run = The first-run setup is not open
readonly-sample = the sample project
tooltip-learn-more = F1: learn more

# Part 2: banners and errors.
banner-sample = This is the sample project. It is open read-only; Save as makes a copy of your own.
banner-tutorial-copy = A fresh copy of the sample project was made for the tutorials.
banner-imported = Imported { $p0 }: { $p1 } pages, { $p2 } devices, { $p3 } findings in the migration report.
banner-import-no-content = The starter library was not found; symbols the project refers to may be missing.
banner-journal-discarded = The unsaved changes of the earlier session were discarded. The project is open for editing.
banner-workspace-bad = The workspace file { $p0 } could not be read; the built-in layout is used and the file was left as it is.
banner-workspace-write = The workspace file could not be written.
banner-wizard-done = The company standard was written to { $p0 }. New projects start from it.
error-new = Cannot create the project: { $p0 }.
error-no-sample = The sample project is not installed.
error-import = Cannot import { $p0 }: { $p1 }.
error-import-read = Cannot read { $p0 }.
error-import-write = Cannot write the imported project: { $p0 }.
error-confirm-needed = The step discards unsaved changes and needs "confirm": true.
error-workspace-missing = There is no workspace named { $p0 }.
error-workspace-bad = The workspace { $p0 } cannot be used: { $p1 }.
error-workspace-name = A workspace name has 1 to 40 letters, digits, spaces, - or _.
error-workspace-write = The workspace file could not be written.
error-wizard-write = The basic project could not be written: { $p0 }.

# Part 2: the start page, the panes and the dialogs.
start-new = New project
start-template-builtin = Template: the built-in basic project
start-template = Template: { $p0 }
start-sample = Open sample project
start-import-qet = Import QElectroTech project
start-company-standard = Create company standard
start-edit-standard = Edit company standard
start-tutorials = Tutorials
start-quick-language = Language
start-quick-keymap = Keymap
start-privacy-change = Change
language-en = English
language-nl = Nederlands
keymap-ew = EW
keymap-kicad = KiCad-like
theme-system = Follow the system
theme-light = Light
theme-dark = Dark
theme-hc-dark = High contrast dark
theme-hc-light = High contrast light
density-comfortable = Comfortable
density-compact = Compact
origin-eplan = EPLAN
origin-qet = QElectroTech
origin-kicad = KiCad
origin-other = Another tool
pane-pages = Pages
pane-page-view = Page view

# The property grid (WP-S2.05 part 1).
pane-properties = Properties
action-props-show = Properties
action-props-focus = Properties of selection
action-props-project = Project properties
action-select-set = Select records
action-props-row = Move the property cursor
action-props-category = Collapse or expand a property category
action-props-edit = Edit the property
action-props-input = Type into the property field
action-props-commit = Commit the property
action-props-cancel = Revert the property
action-props-set = Set a property
action-props-accept-tag-proposal = Accept the proposed tag
action-props-choose = Choose
action-props-row-menu = Property actions

# FT-1030 (ID-1107): a link row of the grid followed.
action-props-follow = Go to the linked record
reason-row-no-link = The property row links to nothing
error-device-not-drawn = { $p0 } is drawn on no page.
action-props-filter = Filter the properties
props-filter-hint = Filter properties
props-finding-tip = { $p0 }: { $p1 }
props-findings-count = { $p0 } { $p1 }
props-findings-chip = { $p0 }: { $p1 }
props-action-edit = Edit
props-action-clear = Clear
props-action-override = Override
props-action-reset = Reset to derived
props-action-freeze = Freeze
props-action-unfreeze = Unfreeze
props-action-make-variable = Make variable
action-props-action = Run a property action
action-props-list = Change a list property
action-props-mltext = Texts per language
action-preview-apply = Apply the preview
action-preview-cancel = Cancel the preview
action-toast-undo = Undo the last edit
action-toast-dismiss = Close the notification
reason-no-sheet = The property grid shows nothing
reason-no-row = No property row
reason-not-editing = No property is being edited
reason-no-choices = No choices are open
reason-no-actions = The row has no actions
reason-not-mltext = The row is not a multi-language text
reason-no-card = No preview is open
reason-no-toast = No notification is shown
reason-changed-meanwhile = Changed meanwhile: edit again
reason-bulk = More than 500 records
reason-identifiers = Identifiers change
reason-bulk-command = The command is previewed first
reason-rebuilt = Rebuilt on the current values
error-no-row = There is no property row { $p0 }
error-no-part = The project has no part with the part number { $p0 }: choose one of the project's parts from the list
props-ro-generated = generated by block '{ $p0 }'
props-ro-identity = identity
props-ro-action = changed with { $p0 }
props-ro-command = changed by { $p0 }
props-ro-human-only = human only: decides check outcomes; change it in { $p0 }
props-ro-no-command = no command edits this member yet
props-refused-on = on { $p0 }: { $p1 }
props-target-n = target { $p0 }
props-gone = The record no longer exists
props-a-record = a record
props-options = Options of { $p0 }
props-gone-title = Object no longer exists
props-none = No common fields: the selection holds { $p0 }
props-empty = Nothing to show
props-member-varies = The member differs between the selected records; set the whole value first
props-cannot-compose = This value cannot be set here
props-cannot-freeze = There is no number to freeze
props-changed-meanwhile = changed meanwhile: edit again
props-varies = ‹varies› ({ $p0 })
props-notes = Note: { $p0 }
props-items = { $p0 } items
props-row-name = { $p0 }: { $p1 }
props-neutral = Neutral
toast-set = Set { $p0 } on { $p1 } { $p2 }
toast-tag-proposal = Tag { $p0 }: online numbering gave counter { $p1 }
props-tag-proposal = { $p0 }. Use { $p1 } to accept it.
props-tag-proposal-script = { $p0 }. Use { $p1 } to accept it: props.accept_tag_proposal takes it.
props-use-tag = Use { $p0 }
props-join-tag = Join { $p0 }
props-join-proposal = { $p0 } exists. Join it as an auxiliary contact?
props-join-proposal-script = { $p0 } exists. Join it as an auxiliary contact? props.accept_tag_proposal joins it.
toast-joined = Joined { $p0 } ({ $p1 }) at { $p2 }
toast-joined-dropped = { $p0 }; { $p1 }
count-wire = { $p0 } wire
count-wires = { $p0 } wires
count-box-pin = { $p0 } connection point
count-box-pins = { $p0 } connection points

# FT-893: the strip takes its tag with its first terminal; the example is the tag it would take on the page on
# show (`-no-example` off a schematic page).
toast-strip-new = Created a terminal strip. It takes its tag, such as { $p0 }, when its first terminal is placed: Draw ▸ Symbol… and the terminal symbol, whose strip picker lists it.

# FT-1090: a tag given in the New terminal strip form is the strip's from the start.
toast-strip-new-tagged = Created terminal strip { $p0 }. Place its terminals with Draw ▸ Symbol… and the terminal symbol, whose strip picker lists it.
toast-strip-new-no-example = Created a terminal strip. It takes its tag when its first terminal is placed: Draw ▸ Symbol… and the terminal symbol, whose strip picker lists it.
toast-export-pdf = Exported { $p0 }: { $p1 } pages, { $p2 } KiB

# P-2: a second export of the same file in this session says so, so the toast visibly changes.
toast-export-pdf-again = Exported { $p0 } again: { $p1 } pages, { $p2 } KiB
toast-structure-new = Created the { $p0 } node { $p1 }
toast-external-ref-new = Created the external reference { $p0 }; the Interruption point tool links an external point to it by this label
toast-ip-deleted-wire = Deleted the interruption point { $p0 } and the wire connected to it
toast-ip-deleted-wires = Deleted the interruption point { $p0 } and the { $p1 } wires connected to it
toast-ips-deleted-wire = Deleted the interruption points { $p0 } and the wire connected to them
toast-ips-deleted-wires = Deleted the interruption points { $p0 } and the { $p1 } wires connected to them

# FT-709: a delete that leaves a device without a symbol keeps the device; the toast says where it stays.
toast-device-unplaced = { $p0 } has no symbol on a page now; it stays in the Devices navigator and the parts list
toast-device-unplaced-no-part = { $p0 } has no symbol on a page now; it stays in the Devices navigator
toast-devices-unplaced = { $p0 } have no symbol on a page now; they stay in the Devices navigator and the parts list
toast-devices-unplaced-no-part = { $p0 } have no symbol on a page now; they stay in the Devices navigator
toast-device-retagged = { $p0 } (now { $p1 })
toast-autoconnect = { $p0 } and connected { $p1 }: { $p2 }. Alt+Click or { $p3 } places without connecting
toast-placed = Placed { $p0 } at { $p1 }
toast-placed-no-cell = Placed { $p0 }
toast-placed-in-wire = { $p0 }, connected into the wire it was dropped on

# FIXPT41: an interruption point dropped on a pin ({3} the key of `canvas.activate_alt`).
toast-ip-connected = Placed the interruption point { $p0 } at { $p1 } and connected it to { $p2 }. Alt+Click or { $p3 } places it without connecting
toast-ip-connected-no-cell = Placed the interruption point { $p0 } and connected it to { $p2 }. Alt+Click or { $p3 } places it without connecting
toast-ip-external-connected = Placed the external point { $p0 } at { $p1 } and connected it to { $p2 }. Alt+Click or { $p3 } places it without connecting.
toast-ip-external-connected-no-cell = Placed the external point { $p0 } and connected it to { $p2 }. Alt+Click or { $p3 } places it without connecting.
terminal-in-sentence = { $p0 } terminal { $p1 }

# FX38d: a pin whose designation the device shares, by its position in words ("-1A4 pin - (2nd)").
pin-in-sentence = { $p0 } pin { $p1 } ({ $p2 })
toast-undo = Undo
toast-close = Close

# WP-S2.09 part 1: the jobs, the notifications and the recovery dialog.
toast-open = Open
toast-export-cancelled = Export cancelled; no file written
toast-export-failed = Export failed: { $p0 }; check that the folder is writable and the file is not open in another program
toast-export-internal = Export failed: an internal error
toast-recovered = Recovered { $p0 } changes; save to keep them
toast-recovered-one = Recovered 1 change; save to keep it

# FT-549: the counted outcomes.
toast-bridged = Added { $p0 } bridges between { $p1 } terminals
toast-bridged-one = Added 1 bridge between { $p1 } terminals
toast-check = Check: { $p0 }
toast-marker-count = { $p0 } wires carry a marker
toast-marker-count-one = 1 wire carries a marker
check-error = 1 error
check-errors = { $p0 } errors
check-warning = 1 warning
check-warnings = { $p0 } warnings
check-info = 1 info
check-infos = { $p0 } infos
check-waived-one = 1 waived
check-waived = { $p0 } waived
check-pending-one = 1 pending
check-pending = { $p0 } pending
marker-export = Exported PDF to { $p0 }
marker-recovered = Recovered { $p0 } changes from the journal
marker-recovered-one = Recovered 1 change from the journal
action-jobs-show = Jobs
action-lists-parts = Open parts list
bar-lists-parts = Parts list
action-jobs-cancel = Cancel job
action-jobs-wait = Wait for job
action-jobs-open-result = Open result
action-notifications-show = Notifications
action-notifications-run = Run notification action
action-journal-recover = Recover
action-journal-keep = Open read-only
pane-jobs = Jobs
reason-dialog-open = A dialog is open: answer it first
reason-file-exists = The file exists; pass replace to overwrite it
reason-export-running = An export is running
reason-export-into-project = The file would be inside the open project's folder; choose another folder
reason-no-running-job = No job is running
reason-job-finished = The job has finished
reason-no-result = No export has written a file
reason-no-opener = The headless mode opens no file
reason-action-expired = The action is no longer valid
reason-no-toast-undo = The notification offers no Undo
reason-no-recovery = No recovery is pending
reason-recovery-failed = The recovery failed: open read-only or discard the changes
error-unknown-notification = There is no notification { $p0 }; the log holds { $p1 }.
error-no-such-action = The notification offers no { $p0 } action.
error-unknown-job = There is no job { $p0 }; the jobs are { $p1 }.
error-no-thread = the job could not be started
list-none = none
list-and = and
job-kind-pdf = PDF export
job-kind-print = Print set

# WP-S3.07 part 3 (D10, rulings 3 and 4 of ID-1180): the print pre-dialog, its preview and its toasts.
print-title = Print
print-range = Range
print-range-all = All
print-range-current = Current page
print-range-sections = Sections
print-range-pages = Pages
print-section-row = Section { $p0 }: { $p1 }
print-paper = Paper
print-paper-sheet = Sheet size
print-fit = Scale to fit
print-mono = Monochrome
print-preview = Preview
print-page-of = { $p0 } of { $p1 }
print-previous = Previous
print-next = Next
print-button = Print…
print-viewer-button = Open in PDF viewer
print-empty-title = No pages to print
print-empty-text = Tick a section or type the pages to print
print-building = Building the print set… { $p0 } %
print-writing = Writing the print set…
print-failed = The print set could not be built: { $p0 }
# FT-1124 (ID-1225): the refusal of the Pages text, under the field.
print-pages-error = The pages could not be read. Type folio labels or ranges, separated by commas.
print-unavailable = Printing is not available in this window. Export a PDF and print it from your PDF viewer.
toast-print-printed = Sent { $p0 } to the printer
toast-print-opened = Opened { $p0 } in your PDF viewer
toast-print-fallback-printer = Printing through the print dialog failed, so { $p0 } went to the default printer.
toast-print-fallback-viewer = Printing could not start, so { $p0 } opened in your PDF viewer to print from there
toast-print-failed = Printing failed: { $p0 }. Export a PDF and print it from your PDF viewer.
reason-no-print-dialog = Open the print dialog first
reason-print-not-built = The print set is still being built
reason-print-no-pages = No pages are chosen
reason-print-failed = The print set could not be built
job-cause-in-use = the file is in use
job-cause-no-folder = the folder does not exist
job-cause-denied = access to the folder or the file was denied
job-cause-full = the disk is full
job-cause-internal = an internal error
job-phase-building = building pages
job-phase-drawing = drawing pages
job-phase-writing = writing the file
job-as-of = as of rev { $p0 }
job-progress = page { $p0 } of { $p1 }
job-running = running
job-elapsed = { $p0 } s
job-cancel = Cancel
job-done = { $p0 } pages, { $p1 } KiB
job-now = exported rev { $p0 }; the model is now at rev { $p1 }
job-cancelled = cancelled; no file written
job-failed = failed: { $p0 }
jobs-empty = No jobs
status-notifications = Notifications { $p0 }
status-job = { $p0 } { $p1 } %
status-job-phase = { $p0 }: { $p1 }
notifications-title = Notifications
notifications-empty = No notifications
recovery-title = Recover unsaved changes?
recovery-text = { $p0 } has unsaved changes from a session that ended unexpectedly.
recovery-count = Unsaved changes: { $p0 }
recovery-last-change = Last change: { $p0 }
recovery-last-change-utc = { $p0 } UTC
elapsed-just-now = just now
elapsed-minutes = { $p0 } minutes ago
elapsed-minutes-one = 1 minute ago
elapsed-hours = { $p0 } hours ago
elapsed-hours-one = 1 hour ago
elapsed-days = { $p0 } days ago
elapsed-days-one = 1 day ago
recovery-moved = The project changed on disk since.
recovery-discard = Discard…
recovery-failed-moved = the project changed on disk in the same places; recovery needs the merge panel
recovery-failed-replay = the journal could not be replayed
recovery-failed = Recover failed: { $p0 }
open-failed = The system could not open the file.
card-line = { $p0 } { $p1 } · { $p2 } changes · { $p3 } tags, { $p4 } cross-reference texts
card-apply = Apply
card-cancel = Cancel
card-stale = stale
card-failed = failed: { $p0 }

# WP-S2.17 part 4: cards, badges, empty states and outcome badges (doc 16a §6.16, §6.17, §6.23).
card-strip-failed = { $p0 } failed: { $p1 }
card-as-of = as of rev { $p0 }
badge-stale = Stale
badge-failed = Failed
badge-done = Done
badge-cancelled = Cancelled
card-title-reorder = Reorder pages
card-title-fix = Quick fix { $p0 }
card-title-find = Replace
card-title-properties = Preview of { $p0 }
card-title-pattern = Pattern fill
empty-jobs = No jobs yet
empty-jobs-text = Exports you start appear here with their progress.
empty-preview-text = Renumbering, a quick fix or an import shows its changes here before you apply them.
empty-history-text = Your changes appear here; Undo steps back through them.
empty-find-text = Nothing in the project matches what you searched for.
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

# Doc 16a §6.5: the filter field's trailing clear button.
filter-clear = Clear the filter
empty-nav-pages = No pages yet
empty-nav-devices = No devices yet
empty-nav-connections = No connections yet
empty-nav-strips = No terminal strips yet
empty-nav-cables = No cables yet
empty-nav-plc = No PLC cards yet
empty-nav-potentials = No potentials yet
empty-nav-ips = No interruption points yet
empty-nav-structure = No structure identifiers yet
empty-nav-parts = No parts in use yet
empty-nav-blocks = No report blocks yet
severity-error = Error
severity-warning = Warning
severity-info = Info
severity-errors = Errors
severity-warnings = Warnings
severity-infos = Infos
badge-pending = Pending
badge-waived = Waived
badge-fix = Fix
badge-not-undoable = Not undoable
toast-dismiss = Dismiss
toast-name = Notification

# WP-S2.05 part 2: find and replace.
pane-find = Find and replace
action-find-open = Find…
action-find-open-replace = Find and replace…
action-find-close = Close the find pane
action-find-text = Type the search text
action-find-replacement = Type the replacement
action-find-option = Toggle a find option
action-find-fields = Choose the fields to search
action-find-folios = Filter the search by folio
action-find-run = Find
action-find-row = Move the result cursor
action-find-jump = Show the result

# WP-S6.13 part 1
action-page-describe = Describe the page content
action-find-replace-all = Replace all
find-field-function-text = Function text
find-field-tag = Tag
find-field-property = Property
find-field-text = Text
find-field-title = Title
find-field-description = Description
find-field-process-tag = Process tag
find-field-part-number = Part number
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
find-results = { $p0 } results
find-no-results = No results
find-truncated = More than 10,000 results: refine the search
find-stale = stale
find-replaced = Replaced { $p0 }; run again to refresh
find-updated-fewer = Preview updated: { $p0 } fewer results
find-updated-more = Preview updated: { $p0 } more results
find-updated-same = Preview updated: the same results
find-not-replaced = not replaced: { $p0 }
find-card-line = { $p0 } results on { $p1 } records · { $p2 } tags, { $p3 } cross-reference texts
reason-no-find = The find pane is closed
reason-find-no-replace = Open Find and replace to replace
reason-find-truncated = More than 10,000 results: refine the search before replacing
reason-find-nothing = No result is replaced
reason-find-no-results = No result is listed
reason-find-no-selection = Nothing is selected
reason-find-replace = A replace is previewed first

# FT-410: the parts table import.
reason-parts-import = An import is previewed first
error-parts-import = Parts table refused: { $p0 }
banner-part-imported = Imported 1 part from { $p2 } ({ $p1 } skipped).
banner-parts-imported = Imported { $p0 } parts from { $p2 } ({ $p1 } skipped).
parts-card-title = Import parts
parts-card-line = { $p0 }: { $p1 } created, { $p2 } skipped, { $p3 } errors

# FT-649: the column mapping step of the parts import.
action-parts-map = Map a column of the parts table
action-parts-map-import = Import the mapped parts table
reason-no-parts-map = No parts table is being mapped
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
banner-parts-none = No new parts in { $p0 }: { $p1 } already in the project, { $p2 } rejected.
toast-parts-read = Read { $p0 }: { $p1 } new, { $p2 } already in the project, { $p3 } rejected.
error-find-field = There is no find field { $p0 }
error-find-option = There is no find option { $p0 }

# WP-S2.05 part 3: the clipboard.
action-edit-cut = Cut
action-edit-copy = Copy
action-edit-paste-clip = Paste
action-edit-duplicate = Duplicate
action-edit-copy-tags = Copy tag list
action-clipboard-text = Set the clipboard text
reason-cut-incomplete-items = Cut would lose page items or marker connections; include every marker wire or use Copy
reason-copy-pages = The selection spans several pages: copy from one page
reason-copy-generated = Generated content is not copied
reason-clipboard-empty = The clipboard is empty
reason-not-a-fragment = The clipboard holds no ElektraWatt fragment
reason-paste-page-kind = Nothing can be pasted on a { $p0 } page
reason-no-tags = No device is selected
tool-paste = Paste
hint-paste = Click or { $p0 } to paste; { $p1 } ends
label-paste = paste { $p0 } records
label-paste-dropped = , { $p0 } connections dropped
announce-copied = Copied { $p0 } records
banner-paste-refused = Not pasted: { $p0 }
mltext-replace = Replace the neutral text
mltext-keep = Keep it
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
category-identity = Identity
category-part = Part
category-electrical = Electrical
category-connection_points = Connection points
category-texts = Texts
category-structure = Structure
category-placement = Placement
category-terminal = Terminal
category-cable = Cable
category-plc = PLC
category-assembly = Assembly
category-other = Other
category-extra = Extra
dialog-save-workspace = Save layout
save-workspace-title = Save layout as
save-workspace-hint = Workspace name
confirm-journal-title = Discard unsaved changes?
confirm-journal-text = An earlier session of this project ended with unsaved changes. Discarding them deletes that work.
notices-title = Third-party notices
notices-generated-missing = The notices of the Rust crates are generated for every release (THIRD-PARTY-NOTICES.txt next to the application); this build has none.
about-privacy-file = PRIVACY.md lists every data flow and what is stored on this computer. This version makes no network connection and sends nothing.
about-privacy-button = Privacy (PRIVACY.md)
privacy-open-failed = PRIVACY.md could not be opened in a browser; it is at { $p0 }.
about-disclaimer = ElektraWatt's checks do not replace engineering verification by a competent person.
about-support-prerelease = Support: pre-release build; only the latest 0.x release is supported.
about-support-until = Support: security and data-loss fixes until { $p0 }.
about-support-ended = Support: this release line no longer receives security fixes (ended { $p0 }).
about-notices-button = Third-party notices
about-docs-button = Documentation
doc-missing = The user guide is not installed with this build; build it with mdbook (docs/book) and install it next to the application.
prompt-open-qet-title = Open a QElectroTech project
prompt-open-parts-title = Import parts table
prompt-new-project-title = Create the project in a new directory
prompt-basic-project-title = Write the basic project to a new directory

# Part 2: the first-run flow.
firstrun-title = Welcome to ElektraWatt
firstrun-step-language = Language
firstrun-step-origin = I come from
firstrun-step-privacy = Privacy
firstrun-step-appearance = Appearance
firstrun-step-start = Where to start
firstrun-text-language = Choose the language of the application.
firstrun-text-origin = Which tool do you come from? This sets the keymap preset and the vocabulary of the palette.
firstrun-text-privacy = ElektraWatt makes no network connection by itself. This version has no update check and sends no crash reports.
firstrun-text-appearance = Choose the theme and the size of the user interface.
firstrun-theme = Theme
firstrun-scale = Scale
firstrun-text-start = What do you want to do first?
firstrun-start-wizard = Create the company standard
firstrun-start-sample = Open the sample project
firstrun-start-qet = Import a QElectroTech project
firstrun-start-new = New project from the built-in basic project
firstrun-start-none = Start with the start page
firstrun-skip = Skip
firstrun-next = Next
firstrun-back = Back
firstrun-finish = Finish

# Part 2: the company-standard wizard.
wizard-title = Company standard
wizard-step-company = Company
wizard-step-frame = Frame and title block
wizard-step-colour = Colour profile
wizard-step-size = Size profile
wizard-step-numbering = Identifiers and numbering
wizard-step-structure = Structure and sections
wizard-step-revisions = Revisions
wizard-step-languages = Languages and formats
wizard-step-reports = Reports and checks
wizard-step-review = Review and create
wizard-hint-company = The panel builder's name, the notice and the logo for the title block.
wizard-hint-frame = The frame and title block of new pages; the cover and the section dividers keep their own frames.
wizard-hint-colour = The wire colours by voltage class and function, with a stated basis.
wizard-hint-size = The cross section of each circuit class.
wizard-hint-numbering = How devices and terminal strips are numbered, and how cross references print.
wizard-hint-structure = The structure nodes of new projects and their sections: one section per line, number, first-last folio, title.
wizard-hint-revisions = The label of the first revision.
wizard-hint-languages = The source language and the formats of printed dates and numbers.
wizard-hint-reports = The report blocks of new projects, and their check scheme. The check scheme is kept in the draft: it decides what the release gate checks, so a person applies it after finishing, not the wizard's script.
wizard-hint-review = Check the choices, then write the basic project to a new directory.
wizard-field-company-name = Company name
wizard-field-company-notice = Notice text for the title block
wizard-field-frame = Frame and title block
wizard-field-colour = Colour profile
wizard-field-size = Size profile
wizard-field-csa-control = Control circuits (mm2)
wizard-field-csa-mains = Mains circuits (mm2)
wizard-field-device-format = Device numbering format
wizard-field-strip-format = Terminal strip numbering format
wizard-field-xref = Cross-reference style
wizard-field-function-node = Function node (=)
wizard-field-location-node = Location node (+)
wizard-field-sections = Sections
wizard-field-first-revision = First revision label
wizard-field-language = Source language
wizard-field-date-format = Date format
wizard-field-decimal = Decimal separator
wizard-field-check-scheme = Check scheme
wizard-field-company-logo = Logo file (SVG; PNG or JPEG print rasterised)
wizard-field-report-blocks = Report blocks
choice-colour-en60204-machine = EN 60204-1 machine (shipped, not yet verified)
choice-colour-iec60445-installation = IEC 60445 installation (shipped, not yet verified)
choice-report_blocks-golden = The standard layout: TOC 2, parts 901, terminals 903, cables 906, wires 907, PLC 908, potentials 909, tags 910, revisions 911
choice-report_blocks-none = None
wizard-error-logo = The logo must be a readable SVG, PNG or JPEG file of at most 512 KiB.
wizard-error-logo-size = The logo file is larger than 512 KiB; choose a smaller file (an SVG logo is usually far smaller).
wizard-error-logo-svg-editing = The logo SVG holds editing data (entities or foreignObject). Save it again as a plain SVG: in Illustrator with "Preserve Illustrator Editing Capabilities" off, in Inkscape as "Plain SVG", in draw.io with "Formatted Text" and "Word Wrap" off.
wizard-error-logo-svg-active = The logo SVG holds a script or an event handler, which a drawing never runs. Remove it and export the logo again as a plain SVG.
wizard-error-logo-svg-external = The logo SVG links or embeds other files (images, fonts, web addresses). Convert text to outlines, remove linked or embedded images and export it again as a plain SVG; a raster logo can be chosen as its own PNG or JPEG file.
wizard-error-logo-svg-complex = The logo SVG is too complex to draw safely (deep nesting, long chains of clones or markers, huge numbers, transform-origin or convolution filters). Simplify it and export it again as a plain SVG with presentation attributes rather than a style sheet.
wizard-notice-logo-gone = The logo file of the saved draft could not be read; choose the logo again.
wizard-logo-raster = { $p0 } (a raster image: it prints rasterised)
choice-frame-qet-grid = QET-compatible A4 landscape, with grid
choice-frame-qet-nogrid = QET-compatible A4 landscape, without grid
choice-colour-sample = DBU/WH 24 V (the sample project's convention)
choice-colour-default = The profile's default
choice-size-sample = 0.75/1.5 (the sample project's profiles, with the cross sections below)
choice-size-default = The profile's default
choice-xref-qet = QET style (102-A1)
choice-xref-eplan = EPLAN style (/102.A1)
choice-language-en = English
choice-date_format-dd_mm_yyyy = dd-mm-yyyy
choice-date_format-iso8601 = yyyy-mm-dd
choice-decimal-point = 0.75
choice-decimal-comma = 0,75
choice-check_scheme-drafting = drafting
choice-check_scheme-pre-delivery = pre-delivery
choice-check_scheme-full = full
wizard-error-field = That is not a field of the wizard.
wizard-error-choice = That is not one of the choices.
wizard-error-csa = A cross section is a number above zero, such as 0.75.
wizard-error-format = A numbering format has { "{" }page{ "}" }, { "{" }letter{ "}" } and { "{" }counter{ "}" }, in any order and with other text between them; braces must pair.
wizard-error-node = A node code has up to 20 letters, digits, - or _.
wizard-error-sections = One section per line: number, first-last folio, title. Numbers and folio ranges must not repeat or overlap.
wizard-error-revision = A revision label has the form V1.0: V, the major number, a point, the minor number.
wizard-error-text = That text is too long.
wizard-error-command = The command { $p0 } was refused: { $p1 }
wizard-written = written to the basic project
wizard-kept = kept in the draft
wizard-next = Next
wizard-back = Back
wizard-cancel = Close (the draft is kept)
wizard-reset = Start over
wizard-create = Create the basic project…
wizard-preview-title = Preview
wizard-preview-cover = Cover
wizard-preview-page = Folio 102
wizard-error-draft-write = The draft could not be saved in the configuration directory.
error-dir-not-empty = The directory is not empty; choose a new, empty directory.
error-workspace-case = A workspace whose name differs only in letter case already exists.
layout-not-json = not JSON: { $p0 }
layout-not-object = a workspace file is a JSON object
layout-schema = `$schema` is not ew.workspace/1
layout-name-missing = `name` is missing
layout-root-missing = `root` is missing
layout-no-page-view = the page view is missing
layout-too-deep = the layout is nested too deeply
layout-too-many = the layout has too many nodes
layout-pane-twice = the pane `{ $p0 }` appears twice
layout-node-object = a node is a JSON object
layout-unknown-member = unknown member `{ $p0 }`
layout-pane-name = `pane` is a name
layout-unknown-pane = unknown pane `{ $p0 }`
layout-tabs-list = `tabs` is a list
layout-tabs-empty = `tabs` is empty
layout-split-dir = `split` is horizontal or vertical
layout-children-list = `children` is a list
layout-children-empty = `children` is empty
layout-child-object = a child is a JSON object
layout-share = `share` is a positive integer
layout-node-missing = `node` is missing
layout-node-kind = a node has `pane`, `tabs` or `split`
wizard-preview-no-sample = The sample project is not installed; the preview needs it.

# The viewer (WP-S2.03 part 1).
menu-window = Window
submenu-page-navigator = Page navigator
action-tab-new = New tab
action-tab-close = Close tab
action-tab-next = Next tab
action-tab-previous = Previous tab
action-tab-focus-other = Other tab group
action-tab-activate = Show tab
action-tab-move = Move tab
action-tab-open = Open page in a new tab
action-view-side-by-side = Side by side
action-nav-goto = Go to page or cell…
action-nav-back = Back
action-nav-forward = Forward
action-nav-follow = Follow cross-reference
action-nav-next-link = Next cross-reference
action-nav-previous-link = Previous cross-reference
action-cursor-left = Cursor left
action-cursor-right = Cursor right
action-cursor-up = Cursor up
action-cursor-down = Cursor down
action-cursor-cell-left = Cursor one cell left
action-cursor-cell-right = Cursor one cell right
action-cursor-cell-up = Cursor one cell up
action-cursor-cell-down = Cursor one cell down
action-cursor-set = Set cursor
action-pages-mode = Page navigator view
action-pages-sort = Page navigator order
action-pages-filter = Filter pages
action-pages-toggle = Expand or collapse
action-pages-row = Move in the page navigator
action-pages-open = Open page

# WP-S2.06 part 1: the navigators.
action-navigator-show = Navigator
action-navigator-view = Navigator view
action-navigator-sort = Navigator order
action-navigator-filter = Filter navigator
action-navigator-toggle = Expand or collapse
action-navigator-row = Move in the navigator
action-navigator-open = Open row
action-navigator-select = Mark rows
action-navigator-mark = Toggle mark
action-navigator-jump = Jump to row

# WP-S2.07 §8: a channel dragged from the PLC navigator onto a connection point.
action-navigator-bind = Bind channel
reason-drop-on-cp = Drop the channel on a connection point

# FX38d: a CP off the channel's card, refused in words before the commit.
reason-bind-off-card = The connection point is not on the channel's card: a channel binds a pin of its card or the coil of a valve on it

# FT-753: the words say what to do.
toast-bind-off-card = { $p0 } is not on the card { $p1 }: wire { $p0 } to a pin of { $p1 } first; the channel binds the card's pin
toast-bind-off-card-untagged = The connection point is not on the card { $p0 }: a channel binds a pin of its card or the coil of a valve on it

# FT-753: a channel dropped on a field device, reached in the model's terms.
reason-bind-wired-pins = The device is wired to more than one pin of the card: drop the channel on the card's pin you mean
toast-bind-wired-pins = { $p0 } is wired to { $p2 } pins of { $p1 } ({ $p3 }): drop the channel on the card's pin you mean
toast-bind-no-position = { $p0 } cannot go on the valve terminal { $p1 }: no valve position is free, or { $p0 } sits on another assembly
toast-bind-done = Bound { $p0 } to { $p1 }
toast-bind-wired = Bound { $p0 } to { $p1 }, where { $p2 } is wired
toast-bind-attached = Attached { $p0 } to { $p1 } at position { $p2 } and bound { $p3 } to { $p4 } (two undo steps)

# FT-757: a channel the bind unbound (FR-PLC-005), appended to the bind's toast.
toast-bind-displaced = ; { $p0 } was bound there and is now unbound
action-navigator-apply = Apply the preview
action-pages-reorder = Reorder pages
action-pages-move-up = Move page up
action-pages-move-down = Move page down

# WP-S2.06 part 2: highlighting.
action-navigator-highlight = Highlight
action-view-clear-highlights = Clear highlights
reason-no-highlights = No potential or net is highlighted
error-highlight-row = Only potentials and connections can be highlighted.

# WP-S2.06 part 3: table mode.
action-navigator-table = Table
action-table-columns = Columns
action-table-cell = Move the cell cursor
action-table-edit = Edit cell
action-table-input = Type into the cell
action-table-commit = Commit the cell
action-table-set = Set cells
action-table-fill-down = Fill down
action-table-pattern-fill = Pattern fill…
action-table-paste = Paste cells
action-table-copy = Copy cells
action-table-clear = Clear cells
action-table-resolve = Choose a reading
action-table-drop = Drop a cell
reason-table-off = Switch to the table first
reason-table-not-editing = No cell is being edited
reason-table-mark-fill = Mark the rows to fill
reason-table-invalid = Some cells are invalid or ambiguous
error-table-blocking = { $p0 } cells are invalid or ambiguous; resolve or drop them first.
error-table-blocking-one = 1 cell is invalid or ambiguous; resolve or drop it first.
error-table-read-only = { $p0 } is read-only here: { $p1 }.
error-table-key-column = The key column { $p0 } cannot be hidden.
error-table-not-ambiguous = Cell { $p0 } has no readings to choose from.
table-nothing = No cell can be applied.
table-no-row = no row
table-no-column = no column

# FT-515/FT-535: a cell whose row has no record of the column's kind (a CP-only potential's definition cells).
table-no-record = no record of this kind
table-define-alone = a potential without a definition is defined on its own row alone

# WP-S1.16 D4 (FT-577): Type or Voltage typed on a potential known only at its CPs, without a name.
table-define-name-first = Name the potential first

# The other read-only reasons of a table cell (`cell_reason`, §8.2's precedence; FT-535 follow-up).
table-reason-derived = derived
table-reason-generated = generated
table-reason-no-command = no command yet
table-reason-property-grid = edit in the property grid
table-not-clearable = cannot be cleared

# WP-S3.08 D9 (ID-1174): a strip-editor or a potential's defining cell in a table round trip.
table-reason-window = edit in the window

# ID-1174 ruling 3: the texts of table mode's pure rule, which `ew-api` hands as keys (`output::table_edit`).
table-same-record = same record as row { $p0 }
table-same-bridge = same bridge as row { $p0 }
table-gesture-edit = edit
table-gesture-fill-down = fill down
table-gesture-pattern-fill = pattern fill
table-gesture-paste = paste
table-gesture-clear = clear

# The card's title of a gesture: "Preview: paste 12 cells on 12 terminals · 2 invalid".
table-preview-title = Preview: { $p0 } { $p1 } { $p2 } on { $p3 } { $p4 }
table-preview-invalid = { $p0 } · { $p1 } invalid
table-cell = cell
table-cells = cells

# WP-S3.08 part 3 (D14, ID-1224): a navigator table's export to a file and the round trip's import.
action-navigator-export-table = Export table…
action-navigator-import-table = Import table…
reason-table-no-file = This table cannot be written to a file; copy its cells instead
reason-table-import = An imported table is always previewed
reason-table-import-stale = The project changed since the file was read; import the file again
error-nav-no-export = The { $p0 } pane's table cannot be exported.
error-table-preset-csv = A preset applies to CSV only; drop it, or choose the format csv.
error-table-preset = There is no CSV preset { $p0 }; the presets are ew, excel-nl, excel-en and rfc4180.
# ID-1252: the preset ew writes no id column, so its file does not import again in the window.
reason-table-preset-ew = The preset ew writes no id column, so the file cannot be imported again; choose excel-nl or excel-en
error-table-import-open = { $p0 } cannot be opened: { $p1 }.
error-table-import-missing = There is no file { $p0 }.
error-table-import-read = The file cannot be read: { $p0 }.
error-table-import-format = The workbook is not a table that ElektraWatt exported; export the table again.
error-table-import-project = The file was exported from another project; export the table from this project again.
error-table-import-table = The file is not a table of this project; export the table again.
error-table-import-id = The file has no id column, by which its rows are matched; export the table again and keep the column.
error-table-import-many = { $p0 } changed cells are more than the 10,000 one import applies; split the file.
prompt-export-table-title = Export table
prompt-table-name = table.xlsx
prompt-open-table-title = Import table
job-kind-table = Table export
job-done-table = { $p0 } KiB
toast-export-table = Exported { $p0 }: { $p1 } KiB
toast-export-table-again = Exported { $p0 } again: { $p1 } KiB
marker-export-table = Exported the table to { $p0 }
toast-table-imported = Imported { $p0 } changed cells from { $p1 }
table-import-title = Import { $p0 }
# ID-1231: the toast of one cell, the refusal with several tables shown and none focused, and the link preview's line
# when `nav.follow` has no key (the command by its menu path, as the menu table holds it).
toast-table-imported-one = Imported 1 changed cell from { $p1 }
reason-table-which = Several tables are shown; click in the one to export or import
link-preview-go-none = Click the link or use Page ▸ Follow cross-reference to go there
table-import-counts = { $p0 } changed, { $p1 } kept, { $p2 }, { $p3 } refused, { $p4 } unknown, { $p5 } missing
table-import-conflicts = { $p0 } conflicts
table-import-conflict-kept = 1 conflict: the project's value is kept
table-import-conflicts-kept = { $p0 } conflicts: the project's values are kept
table-import-cell = { $p0 } · { $p1 } · "{ $p2 }" → "{ $p3 }" · { $p4 }
table-import-cell-applied = applied
table-import-cell-kept = kept: changed in the project only
table-import-cell-conflict = conflict: the project's value is kept
table-import-cell-read-only = read-only
table-import-cell-refused = refused
table-import-cells = Cells
table-import-missing-rows = Rows not in the file

# The word of a navigator's table rows, singular and plural (the Messages pane's rows are findings).
table-row-pages = page
table-rows-pages = pages
table-row-devices = device
table-rows-devices = devices
table-row-connections = connection
table-rows-connections = connections
table-row-strips = terminal
table-rows-strips = terminals
table-row-cables = conductor
table-rows-cables = conductors
table-row-plc = channel
table-rows-plc = channels
table-row-potentials = potential
table-rows-potentials = potentials
table-row-ips = interruption point
table-rows-ips = interruption points
table-row-structure = structure node
table-rows-structure = structure nodes
table-row-parts = part
table-rows-parts = parts
table-row-blocks = report block
table-rows-blocks = report blocks
table-row-unplaced = unplaced device
table-rows-unplaced = unplaced devices
table-row-macros = macro
table-rows-macros = macros
table-row-libraries = library item
table-rows-libraries = library items
table-row-messages = finding
table-rows-messages = findings
card-table-drop = Drop

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
table-pattern-values = Values: { $p0 }
table-resize = Resize { $p0 } column

# FT-509: the tab group's menu of every tab.
dock-all-tabs = All tabs

# Doc 16a §5.7: the bottom dock's close button.
dock-close = Close the bottom dock
submenu-navigators = Navigators
nav-title-devices = Devices
nav-title-connections = Connections
nav-title-strips = Terminal strips
nav-title-cables = Cables
nav-title-plc = PLC
nav-title-potentials = Potentials
nav-title-ips = Interruption points
nav-title-structure = Structure identifiers
nav-title-parts = Parts in use
nav-title-blocks = Report blocks
nav-title-messages = Messages

# WP-S2.09 part 2: the message panel (§7.3, §7.6).
action-messages-show = Messages
action-messages-next = Next finding
action-messages-previous = Previous finding
action-messages-fix = Apply quick fix…
action-messages-check = Run checks
reason-no-fix = The finding has no quick fix
reason-no-findings = There are no findings to show
reason-quick-fix = A quick fix is previewed first
messages-header = { $p0 } · E { $p1 } · W { $p2 } · I { $p3 } · waived { $p4 } · pending { $p5 }
messages-header-waived = waived { $p0 }
messages-header-pending = pending { $p0 }
messages-footer = Checks support, but do not replace, engineering verification.
messages-detail-rule = { $p0 } { $p1 }
messages-detail-kind = Kind: { $p0 } · severity: { $p1 }
messages-detail-fix = Quick fix: { $p0 }
messages-fix-button = Fix…
messages-rule-page = Rule page

# FT-674: Waive… and Unwaive; FT-664: the check toast's waived and pending counts in its title.
action-messages-waive = Waive finding…
action-messages-unwaive = Unwaive finding
messages-waive-button = Waive…
messages-unwaive-button = Unwaive
messages-detail-waived = Waived
waive-title = Waive finding
dialog-waive = Waive
form-reason = Reason
form-author = Author
error-waive-reason = A waiver needs a reason of at least { $p0 } characters.
error-waive-author = A waiver needs the author's name.
reason-not-open-finding = The row is not an open finding
reason-not-waived = The finding is not waived
toast-check-qualified = Check · { $p1 }: { $p0 }
card-fix = { $p0 } records, { $p1 } tags, { $p2 } cross-reference texts
nav-tree-name = { $p0 } navigator
chips-label = Not on this page:
chip-name = Open { $p0 }
card-reorder = { $p0 } · { $p1 } tags, { $p2 } cross-reference texts
error-nav-kind = Unknown navigator { $p0 }; use one of { $p1 }.
error-nav-closed = The { $p0 } navigator is not open; show it first (navigator.show).
error-nav-key = No row { $p0 } is on show in the { $p1 } navigator.
error-unknown-strip = No terminal strip { $p0 } in the project; its strips are { $p1 }. A new strip's tag (-X??) takes its counter with its first terminal
error-nav-no-table = The { $p0 } pane has no table mode; its rows are findings.
error-nav-value = Unknown { $p0 } { $p1 }; use one of { $p2 }.
error-generated-page-order = A generated page has no folio of its own.
error-pages-sections = The marked pages are in different sections; move pages between sections by changing their section first.
reason-no-pages-marked = No page record is marked
reason-page-at-start = The page is the first of its section
reason-page-at-end = The page is the last of its section
reason-no-nav-row = No navigator row
reason-no-navigator = No navigator has the focus
action-ui-focus = Focus an area
action-ui-focus-next = Next area
action-ui-focus-previous = Previous area
nav-mode-flat = Flat list
nav-mode-section = By section
nav-mode-structure = By structure
nav-sort-folio = Sort by folio
nav-sort-type = Sort by type
nav-front-matter = Front matter
nav-section = Section { $p0 } - { $p1 }
nav-none = (none)
nav-filter-hint = Filter
badge-generated = generated
badge-qet-grid = QET grid
badge-current = current
nav-page-name = { $p0 } { $p1 }, page{ $p2 }
nav-node-name = { $p0 }, { $p1 }
tab-title = { $p0 } { $p1 }
tab-close-name = Close { $p0 }
reason-tabs-full = 32 tabs are open
reason-last-tab = The last tab stays open
reason-one-tab = The tab group has one tab
reason-one-group = Only one tab group is shown
reason-history-empty = No page to return to
reason-no-link = No cross-reference here
reason-link-external = The cross-reference leads to another project
error-goto-empty = Type a folio or a cell.
error-goto-not-found = No page or cell matches "{ $p0 }".
error-goto-no-cell = Page { $p0 } has no cell { $p1 }.
status-cursor-cell = { $p0 } · x { $p1 } y { $p2 } mm
status-hint-link = Click or F: go to { $p0 }

# FT-964 (ID-1150): the status bar's link hint, with the key of `nav.follow` from the keymap.
status-hint-follow = Click or { $p1 }: go to { $p0 }
status-hint-link-external = Link to another project
canvas-cursor-description = , cursor { $p0 } x { $p1 } y { $p2 } mm
canvas-cursor-description-no-cell = , cursor x { $p0 } y { $p1 } mm
canvas-cursor-name = Canvas cursor
canvas-cursor-live = { $p0 }, x { $p1 } y { $p2 } mm
canvas-cursor-live-link = , link to { $p0 }

# Creating pages, structure nodes and external references (FT-408, FT-409).
action-page-new = New page…
action-structure-new = Structure node…
action-external-ref-new = External reference…
new-page-title = New page
new-structure-title = New structure node
new-external-title = New external reference
nav-new-page = New page
dialog-create = Create
form-folio = Folio
form-page-type = Type
form-title = Title
form-section = Section number
form-kind = Kind
form-designation = Designation
form-parent = Parent (designation)
form-location = Location (designation)
form-device = Device
form-connection = Connection
form-project = Other project

# FT-1090 (ID-1181): the picker's New terminal strip form.
new-strip-title = New terminal strip
form-tag = Tag
error-strip-tag = "{ $p0 }" is not a tag. Type the tag as the page is to print it, such as { $p1 }.
error-folio-number = A folio starts with its number: "{ $p0 }" does not.
error-page-type = "{ $p0 }" is not a page type; use one of: { $p1 }.
error-structure-kind = "{ $p0 }" is not a structure kind; use one of: { $p1 }.
error-no-node = The project has no { $p0 } node "{ $p1 }".
error-ambiguous-node = { $p0 } nodes share the designation "{ $p1 }": { $p2 }. Give the alias of one of them instead.
node-candidate-under = { $p0 } (under { $p1 })
node-candidate-top = { $p0 } (top level)
error-second-sign = "{ $p0 }" contains a second aspect sign { $p1 }; give one designation
error-wrong-sign = "{ $p0 }" starts with { $p1 }, the sign of the { $p2 } aspect, but this node is in the { $p3 } aspect.

# WP-S2.09 part 3: the Preview tab, the renumber dialogs, the history panel and the purge step.
pane-preview = Preview
pane-history = History
action-preview-show = Preview
action-history-show = History
action-view-bottom-dock = Bottom dock
action-history-row = Move in the history
action-history-undo-to = Undo to here
action-history-changes = Show changes
action-pages-renumber = Renumber pages…
action-devices-renumber = Renumber devices…
action-project-purge-assets = Purge unused images…
preview-none = No preview is open
preview-header = Preview: { $p0 } · as of rev { $p1 }
preview-title = Preview: { $p0 }
preview-stale = stale
preview-summary = { $p0 } · { $p1 } · { $p2 } · { $p3 }
preview-records = { $p0 } records
preview-records-one = 1 record
preview-parts-new = { $p0 } new parts
preview-parts-new-one = 1 new part
preview-parts-skipped = { $p0 } already in the project
preview-parts-skipped-one = 1 already in the project
preview-import-summary = { $p0 } · { $p1 }
preview-tags-count = { $p0 } tags
preview-tags-count-one = 1 tag
preview-xrefs-count = { $p0 } cross-reference texts
preview-xrefs-count-one = 1 cross-reference text
preview-pages-count = { $p0 } pages
preview-pages-count-one = 1 page
preview-entities = Entities
preview-tags = Tags
preview-xrefs = Xrefs
preview-pages = Pages
preview-more = and { $p0 } more
preview-open-chip = Preview open: { $p0 }
preview-show = Show
preview-apply = Apply
preview-cancel = Cancel
history-header = { $p0 } steps · cursor at { $p1 }

# FT-986 (ID-1150): one step in the singular.
history-header-one = { $p0 } step · cursor at { $p1 }
history-empty = Nothing to undo yet
history-not-undoable = not undoable
history-merged = merged { $p0 }
history-changes-title = Changes of step { $p0 }
history-redo = undone
history-redo-to = Redo to here
marker-saved = Saved
renumber-pages-title = Renumber pages
renumber-devices-title = Renumber devices
dialog-renumber = Renumber
form-renumber-section = Section (number, or front)
form-start = Start
form-step = Step
form-keep-gaps = Keep gaps
form-scope = Scope
form-scheme = Scheme
form-include-frozen = Include frozen tags
form-only-nonconforming = Only non-conforming
error-no-section = The project has no pages in section { $p0 }.
error-no-front-matter = The project has no pages outside a section.
error-unknown-entry = There is no history entry { $p0 }; the history holds { $p1 }.
reason-no-devices-selected = No device is selected
reason-history-closed = The history panel is closed
reason-undo-history-empty = The history is empty
reason-nothing-to-purge = No image is unused
reason-no-purge-dialog = Open Purge unused images… first
confirm-purge-title = Purge unused images?
confirm-purge-text = These files are deleted and do not come back by undo.
confirm-purge-more = and { $p0 } more
dialog-purge = Purge
error-purge = The purge was refused: { $p0 }
purge-unreadable = an image file could not be read

# WP-S2.17 part 3: the rails, Properties' Main and Advanced, the status bar (doc 16a §5.4, §5.8, §6.8).
rail-project = Project
rail-symbols = Symbols
rail-parts = Parts
rail-pages = Pages
rail-devices = Devices
rail-views = Pages or devices
action-rail-show = Show navigator
reason-rail-no-palettes = The palettes are not built yet; Symbols and Parts open their pickers
category-main = Main
action-props-show-advanced = Show advanced properties
props-show-advanced = Show advanced ({ $p0 })
props-hide-advanced = Hide advanced ({ $p0 })
props-row-menu = Actions of { $p0 }
status-ready = Ready
status-selected = { $p0 } selected
status-findings-errors = { $p0 } errors
status-findings-error = 1 error
status-findings-warnings = { $p0 } warnings
status-findings-warning = 1 warning
status-findings-infos = { $p0 } infos
status-findings-info = 1 info

# FT-710: the pending findings, counted apart; they end when the tab leaves the page.
status-findings-pending = { $p0 } pending on this page
status-findings-pending-one = 1 pending on this page
status-renderer-why = The canvas draws on the CPU
status-bar-saved = Saved
table-sorted-ascending = { $p0 }, sorted ascending
table-sorted-descending = { $p0 }, sorted descending
props-empty-value = —
status-bar-unsaved = Unsaved changes
status-bar-read-only = Read-only
splitter-name = Resize { $p0 } and { $p1 }
history-header-kept = oldest kept: step { $p0 } · { $p1 } MB of { $p2 } MB

# WP-S1.19 part 6 (§11): the Macros and Libraries navigators, the macro card, Save as macro… and Make variable.
nav-title-macros = Macros
nav-title-libraries = Libraries
empty-nav-macros = No macros yet
empty-nav-macros-text = Save as macro… in the Edit menu stores a selection in a library; its macros are listed here.
empty-nav-libraries = No libraries yet
badge-update-available = Update available
badge-source-missing = Source missing
badge-deprecated = Deprecated
badge-local-fork = Local fork
badge-index-failed = Index failed
action-macros-insert = Insert macro…
action-macros-place = Place a macro
action-macros-value-set = Choose the macro's value set
action-macros-save = Save as macro…
action-libraries-update = Update…
action-props-make-variable = Make variable
tool-macro = Macro
hint-macro = Click or { $p0 } to insert the macro; { $p1 } ends
label-macro = Insert { $p0 }
reason-macro-insert = A macro is inserted
reason-macro-not-installed = Only the project holds this macro; install its library to insert it
reason-library-update = Library items are updated
reason-no-updates = No library item chosen has an update
reason-library-read-only = The library is read-only; save into a user or company library
reason-no-writable-library = No writable library is installed; add a user or company library first
reason-macro-no-key = The name has no letter or digit to make a key from; type a key
reason-library-write-window = Only a person saves a macro, in the window: a library write is not undoable and no script reaches it
reason-variable-one-record = Make variable takes the field of one record
reason-variable-derived = A derived value cannot be a variable
macro-conflict-tag = The tag { $p0 } is printed by another device
macro-conflict-pairs = { $p0 } pairs with { $p1 } existing interruption points
macro-conflict-pairs-one = { $p0 } pairs with an existing interruption point
macro-conflict-address = The PLC address { $p0 } is taken
macro-conflict-process-tag = The process tag { $p0 } is in use
macro-conflicts = Conflicts
macro-no-conflicts = No conflicts

# FX38d: a failed macro card leads with what blocks the apply in words; the rule code follows in secondary text.
macro-failed = The macro clashes with this project and cannot be inserted here ({ $p0 } conflicts)
macro-failed-one = The macro clashes with this project and cannot be inserted here (1 conflict)
macro-failed-other = The macro cannot be inserted here: { $p0 }
card-failed-rule = Rule { $p0 }
macro-value-set = Value set
macro-value-set-defaults = Defaults
macro-no-preview = No preview
form-macro-kind = Kind
form-macro-x = Insertion point x (mm)
form-macro-y = Insertion point y (mm)
macro-preview-name = Preview of { $p0 }
macro-devices = Devices
macro-variables = Variables
macro-version = Version { $p0 } · { $p1 }

# FT-755: the Libraries navigator's copy detail
library-copy-kind-key = { $p0 } · { $p1 }
library-copy-not-installed = no installed library
save-macro-title = Save as macro
dialog-save-macro = Save to library
form-macro-name = Name
form-macro-key = Key
form-macro-description = Description
form-macro-library = Library (cannot be undone)
error-no-library = No installed library is called "{ $p0 }"
error-macro-save = The macro was not saved: { $p0 }
note-macro-saved = Saved { $p0 } { $p1 } into { $p2 }
announce-variable = { $p0 } is a variable of the next macro

# WP-S2.07 part 4a (§9.5): the part selection dialog and the part editor.
reason-part-choose-target = Select a device, a terminal or a part cell first
reason-part-choose-none = Choose a part from the list first
reason-part-not-installed = The library of this part is not installed
reason-part-other-version = The project holds another version of this part; update it from the Libraries navigator first
reason-part-edit-target = Select a device with a part, or a part in the Libraries navigator
reason-part-no-manufacturer = A part needs a manufacturer
reason-part-no-number = A part needs a part number
reason-part-unit = Choose the unit: piece, m or set
reason-part-unchanged = Nothing to save: the part is unchanged
error-part-tech-value = { $p0 }: { $p1 }
error-part-tech-twice = { $p0 } is given twice
error-part-manufacturer = No manufacturer is called “{ $p0 }”; choose one from the list
dialog-part-choose-title = Choose a part for { $p0 }
part-choose-fits = Only parts that fit
part-choose-view-all = All
part-choose-view-project = In this project
part-choose-view-favourites = Favourites
part-choose-view-recent = Recent
part-choose-inactive = Show deprecated and withdrawn
badge-fits-function = Fits { $p0 }'s function
badge-same-category = Same category
part-choose-empty = No part matches “{ $p0 }”
part-choose-nothing = No part to choose; change the search.
part-choose-more = Search all parts…
dialog-part-edit-title = Edit part { $p0 }
part-edit-fork-note = Saving creates a project copy of this part; the library is not changed

# The part card in the Preview tab (D20): the field diff, the devices and the parts list rows.
part-card-field = { $p0 }: { $p1 } → { $p2 }
part-card-devices = Used by { $p0 }
part-card-removed = Parts list row removed: { $p0 }
part-card-added = Parts list row added: { $p0 }
part-field-manufacturer = Manufacturer
part-field-part_number = Part number
part-field-type_designation = Type designation
part-field-order_number = Order number
part-field-gtin = GTIN
part-field-description = Description
part-field-kind = Kind
part-field-category = Category
part-field-unit = Unit
part-field-tech = Technical data
part-field-functions = Functions
part-field-cps = Connection points
part-field-source = Source

# The lane's own additions beside §9.5 (proposed): the dialogs' targets and buttons, the kind, category and
# value messages, the detail line and the favourite of a project-local part.
dialog-part-choose-button = Choose
part-choose-search = Search parts
part-choose-kind = Kind
part-choose-category = Category
part-choose-all = All
part-choose-list = Parts
part-choose-favourite = Favourite
part-choose-detail = { $p0 } · version { $p1 } · { $p2 }
part-choose-detail-successor = { $p0 } · version { $p1 } · { $p2 }, successor { $p3 }
part-choose-detail-functions = { $p0 } · functions { $p1 }
part-choose-target-bridge = the bridge { $p0 }
part-choose-target-accessory = the { $p0 } { $p1 }
part-choose-target-devices = { $p0 } devices
part-none = none
error-part-kind = “{ $p0 }” is not a part kind
error-part-category = A category has three levels: category/group/definition
error-part-value = { $p0 }
error-part-field = The part editor has no field { $p0 }
error-part-unknown = No part is called “{ $p0 }”
reason-part-favourite-local = Only a part from a library can be a favourite
reason-no-part-dialog = Open the part dialog first
reason-no-part-editor = Open the part editor first
reason-part-edit = A part is edited
toast-part-chosen = Chose { $p0 }
action-part-choose = Choose part…
action-part-add = Add part…
action-part-search = Search parts
action-part-pick = Choose
action-part-favourite = Favourite
action-part-edit = Edit part…
action-part-field = Edit field
action-part-save = Save

# WP-S2.07 part 4b (§9.5): FT-396's paste choice.
reason-paste-versions = The pasted records use another version of { $p0 }; choose Use the project's copy or Update the project's copy

# Cycle 39 (FIX39b-1): the HUD's short prompt while the paste choice's cards are open.
hud-paste-versions-one = Another version of { $p0 } is in the project; choose in the card below
hud-paste-versions-many = Other versions of { $p0 } are in the project; choose in the card below
hud-paste-update = Apply the update in the card below; the paste follows
preview-field-content = content

# The change size of a paste choice's conflict row in words (the describe line keeps patch, minor, major).
paste-class-patch = a data change
paste-class-minor = a compatible change
paste-class-major = a breaking change
reason-paste-adapt-major = The versions differ in their connection points; update instead
reason-paste-update-older = The pasted version is not newer than the project's
card-paste-versions-title = The pasted records use other part versions
paste-adapt = Use the project's copy
paste-update = Update the project's copy

# The lane's own additions beside §9.5 (proposed): a conflict row of the card, the action's label and the
# refusal without a choice card.
paste-conflict = { $p0 }: this project { $p1 }, pasted { $p2 }, { $p3 }
action-paste-resolve = Resolve paste
reason-no-paste-choice = No paste waits for a choice

# WP-S2.08 part 2 (D6 to D8): the keymap files, their rules and the editor.
keymap-error-key = Unknown key in "{ $p0 }".
keymap-error-modifier = Unknown or repeated modifier { $p0 } in "{ $p1 }".
keymap-error-toml = It is not valid TOML.
keymap-error-meta = The file has no [meta] table with version = 1.
keymap-error-bind = Binding { $p0 } needs keys, action and context as texts.
keymap-error-context = Binding { $p0 } names the unknown context "{ $p1 }".
keymap-conflict = { $p0 } is bound twice in the { $p1 } context: { $p2 }
keymap-shadow = { $p0 } in the { $p1 } context shadows { $p2 }
keymap-ctrl-alt = { $p0 } uses Ctrl+Alt, which is AltGr on many layouts
keymap-reserved = { $p0 } is reserved by the system
keymap-needs-shift = { $p0 } needs Shift on { $p1 }
keymap-unreachable = { $p0 } has no shortcut that types on { $p1 } without AltGr
banner-keymap-refused = The keymap file was refused: { $p0 }. The { $p1 } preset applies; the file was kept as { $p2 }.
banner-keymap-write = The keymap could not be written.
banner-keymap-unread = The keymap file could not be read ({ $p0 }); the { $p1 } preset applies and the file is left as it is.
banner-keymap-refused-kept = The keymap file was refused: { $p0 }. The { $p1 } preset applies; the file is left as it is because it could not be copied aside.
banner-keymap-kept = The keymap applies for this session but was not written: keymap.toml is left as it is.
reason-keymap-conflict = The shortcut conflicts with another binding
reason-no-keymap-editor = The keyboard shortcuts dialog is not open
error-keymap-import = The keymap file could not be imported: { $p0 }
error-keymap-export = The keymap file could not be written.
error-keymap-action = { $p0 } is not an action with shortcuts.
action-keymap-search = Search shortcuts
action-keymap-record-start = Record a shortcut
action-keymap-record = Set a shortcut
action-keymap-reset = Reset a shortcut
action-keymap-import = Import keymap…
action-keymap-export = Export keymap…
keymap-search-hint = Search actions or keys
keymap-preset = Preset: { $p0 }
keymap-preset-custom = Preset: { $p0 }, with your changes
keymap-custom-name = { $p0 } (changed)
keymap-record = Record
keymap-reset = Reset
keymap-recording = Press a shortcut…
keymap-custom = Changed
keymap-shadows = Shadows
keymap-refused = { $p0 } was not set:
keymap-unused = Unused, the action is unknown: { $p0 } ({ $p1 })
keymap-menu-key = The Menu key cannot be bound: the window system does not report it.

# W40a 5c: a refused recording says what is in the way, with the context as the editor's column names it.
keymap-taken = { $p0 } is already { $p1 } { $p2 }.
keymap-in-global = everywhere
keymap-in-canvas = in the canvas
keymap-in-navigator = in the navigator
keymap-in-properties = in Properties
keymap-in-find = in Find
keymap-none = No action matches the search.
keymap-context-global = Everywhere
keymap-context-canvas = Canvas
keymap-context-navigator = Navigator
keymap-context-properties = Properties
keymap-context-find = Find
prompt-open-keymap-title = Import keymap
prompt-save-keymap-title = Export keymap
prompt-keymap-name = keymap.toml

# WP-S1.19 part 6 (FT-785, FT-786): the macro card's ad-hoc values and Save as macro's value-set table.
action-macros-value = Type a value of the macro card's variable
macro-values = Values
macro-value-label = { $p0 } ({ $p1 })
form-value-sets = Value sets
form-value-set-name = Value set
form-value-set-add = Add value set
form-value-set-remove = Remove value set { $p0 }
form-value-set-new = Set { $p0 }
form-value-sets-none = No variables yet: Make variable on a property row adds a column.
form-value-cell = { $p0 }, value set { $p1 }
error-value-set-name = Value set { $p0 } needs a name.
macro-failed-value = The macro cannot be inserted with the value typed for { $p0 }.
error-value-set = Value set “{ $p0 }”: { $p1 }
error-value-set-cell = Value set “{ $p0 }”, variable “{ $p1 }”: { $p2 }

# WP-S1.18 part 5 (cycle 41): the revision dialogs of the Check menu (Approve, Release, Withdraw, Set author),
# the incomplete-release offer at open, and the release's housekeeping warnings in Messages (FT-842).
action-revision-approve = Approve revision…
action-revision-release = Release revision…
action-revision-withdraw = Withdraw revision…
action-revision-set-author = Set revision author…
action-revision-incomplete = Settle the incomplete release…
submenu-revision = Revision
reason-no-open-revision = The project has no open revision
reason-no-released-revision = The project has no released revision to withdraw
reason-no-incomplete-release = There is no incomplete release to settle
revision-approve-title = Approve revision { $p0 }
revision-release-title = Release revision { $p0 }?
revision-withdraw-title = Withdraw a revision
revision-author-title = Set the author of { $p0 }
revision-incomplete-title = Settle the incomplete release of { $p0 }?
form-revision-checker = Checker's role
form-revision-author = Author's role
form-revision-typed = Type the label to confirm
form-revision-revision = Revision
form-revision-reason = Reason
form-revision-git = Commit and tag in git
dialog-approve = Approve
dialog-release = Release
dialog-withdraw = Withdraw
dialog-set-author = Set author
dialog-complete-release = Complete release
dialog-discard-archive = Discard archive
dialog-not-now = Not now
revision-approve-text = The checker signs off the saved content of { $p0 }, written by the { $p1 }. The release gate runs on the saved project first; the sign-off is saved.
revision-no-author = { $p0 } has no author yet: set it first (Set revision author…), then save.
revision-gate-error = The release gate cannot run: { $p0 }.

# FIX41b rulings 1 and 4: the rule by its title, the noun in the count's number.
revision-approve-blocked = The release gate has { $p0 } errors besides the four-eyes rule; correct or waive them first:
revision-approve-blocked-one = The release gate has 1 error besides the four-eyes rule; correct or waive it first:
revision-release-blocked = The release is blocked by { $p0 } findings of the release gate:
revision-release-blocked-one = The release is blocked by 1 finding of the release gate:
revision-finding = { $p0 }: { $p1 }
revision-more = and { $p0 } more
revision-waivers = Approving also acknowledges { $p0 } safety waivers in force.
revision-release-text = The release runs six steps and cannot be undone:
revision-step-gate = 1. Run the release gate on the saved project
revision-step-freeze = 2. Freeze the revision's numbering and save the project
revision-step-commit = 3. Commit the project folder in git
revision-step-no-commit = 3. No git commit: the project folder is not in a git repository
revision-step-archive = 4. Write and verify the archive { $p0 } in the folder { $p1 }
revision-step-tag = 5. Create the git tag { $p0 }
revision-step-no-tag = 5. No git tag
revision-step-stamp = 6. Stamp { $p0 } released and save
revision-withdraw-text = The withdrawal is a record after the revision it withdraws; the released revision and its archive stay.
revision-author-text = The party with this role is the author of { $p0 }; the checker who approves it must have another role.
revision-incomplete-text = A release of { $p0 } stopped before its stamp and left the archive { $p1 }.
revision-incomplete-complete = Complete release stamps the revision released with it, committing and tagging it when the project folder is in a git repository.
revision-incomplete-refused = It cannot be completed: { $p0 }.
revision-incomplete-unproven = It is not provably this folder's own leftover, so it is not offered for discarding; remove the file by hand if you know it is one.
error-revision-typed = Type the revision label { $p0 } to confirm.
error-revision-role = Choose a role.
error-revision-reason = Give the reason for the withdrawal.
error-revision-unknown = { $p0 } is not a released revision of this project.
error-revision-git = Tick “Commit and tag in git”: the project folder is in a git repository, and the release commits it and creates the tag { $p0 }.
error-revision-person = This needs a person at the window: a revision is approved, released, withdrawn or given its author only by a confirmation made there, never by a script, a replay or a plugin.
error-revision-approve = The approval was refused: { $p0 }.
error-revision-release = The release was refused: { $p0 }.
error-revision-settle = The incomplete release was not settled: { $p0 }.
toast-revision-approved = Approved revision { $p0 }; saved
toast-revision-released = Released revision { $p0 } as { $p1 }
toast-revision-withdrawn = Withdrew revision { $p0 }
toast-revision-author = The { $p1 } is now the author of { $p0 }
toast-revision-discarded = Discarded the incomplete release of { $p0 }
toast-release-troubles = The release left housekeeping warnings ({ $p0 }): see Messages
release-source = Release

# FIX41b ruling 6: the cause in words from the error's kind ({1}: a cause below, or nothing).
release-trouble-not-removed = The release record { $p0 } could not be removed{ $p1 }.
release-trouble-not-read = The release records in the folder { $p0 } could not be read{ $p1 }.
release-trouble-not-updated = The release record of { $p0 } could not be updated with the archive's time{ $p1 }, so a discard after a crash keeps the archive.
release-cause-not-found = : it was not found
release-cause-denied = : permission was denied
release-cause-not-folder = : it is not a folder

# FIX41b ruling 2: completing or discarding an incomplete release asks first.
revision-complete-title = Complete the release of { $p0 }?
revision-complete-text = Completing stamps { $p0 } released with the archive { $p1 } and saves the project. It cannot be undone.
revision-complete-git = The project folder is in a git repository: completing commits it and creates the tag { $p0 }.
revision-discard-title = Discard the archive of { $p0 }?
revision-discard-text = Discarding deletes the archive { $p0 }, and the git tag this release created, if any; { $p1 } stays open. It cannot be undone.

# FIX41b ruling 1: every refusal of the revision dialogs in words (the CLI keeps its own texts).
gate-no-project = the project is not saved in a project folder
gate-unsaved = the project has unsaved changes; save it first
gate-incomplete-save = a save is incomplete; wait for it to finish
gate-no-manifest = the project has not been saved yet; save it first
gate-digest = the saved files of a merged project carry no content fingerprint yet; save the project first
gate-no-open-revision = the project has no open revision
gate-not-open-revision = { $p0 } is not the open revision; the open revision is { $p1 }
revision-waiver = { $p0 } on { $p1 }
revision-waiver-gone = a waiver whose finding the check no longer reports
approve-read-only = the project is open read-only
approve-four-eyes = the { $p0 } is the author's party; the four-eyes rule needs a checker of another party
approve-blocked = the release gate has { $p0 } errors besides the four-eyes rule
approve-blocked-one = the release gate has 1 error besides the four-eyes rule
approve-unacknowledged = the sign-off does not acknowledge the safety waivers in force: { $p0 }
release-error-person = a release needs a person at the window, never a script, a replay or a plugin
release-error-read-only = the project is open read-only
release-error-gate = the release gate cannot run: { $p0 }
release-error-git-consent = the project folder is in a git repository; tick “Commit and tag in git” to agree to the commit and the tag { $p0 }
revision-release-cannot = The release cannot start: { $p0 }.
release-error-blocked = the release gate has { $p0 } findings that block it
release-error-blocked-one = the release gate has 1 finding that blocks it
release-error-archive-root = the archive folder { $p0 } cannot be used: { $p1 }
release-error-incomplete = the archive { $p0 } exists already: an earlier release stopped before its stamp; settle it first (Settle the incomplete release…)
release-error-tag-exists = the git tag { $p0 } exists already, and a release never moves a tag
release-error-git = a git step failed before the archive was written; nothing was released
release-error-freeze = the revision's numbering could not be frozen; nothing was released
release-error-freeze-pages = freezing the numbering would change { $p0 } printed pages; it was undone and nothing was released
release-error-freeze-pages-one = freezing the numbering would change 1 printed page; it was undone and nothing was released
release-error-save = the project could not be saved, so the release stopped
release-error-generation = the saved project could not be read back for the archive, so the release stopped
release-error-output = the PDF could not be written, so the release stopped
release-error-archive = the archive could not be built, or it does not read
release-error-write = the archive could not be written, so the release stopped
release-error-tag = the git tag could not be created; the archive of this run was removed
release-error-stamp = the revision could not be stamped released; the archive and the git tag of this run were removed
release-error-unsettled = the release is saved, but the window could not take its stamp; reopen the project
release-error-undecided = the stamp could not be saved completely; reopen the project: it completes the release, or offers to settle it
release-error-io = { $p0 } could not be used{ $p1 }
release-error-no-incomplete = there is no incomplete release
release-error-not-completable = it cannot be completed: { $p0 }
release-error-foreign = { $p0 }; it is kept
release-error-unproven = { $p0 }; it is kept: complete the release with it, or remove the file by hand if you know it is a leftover
release-error-unproven-kept = { $p0 }; it is kept: remove the file by hand if you know it is a leftover

# WP-S4.02 part 3: the Unplaced devices navigator (FT-486), the layout place tool and Select on a panel page
# (D4, D10; doc 16a §5.5 HUD, §6.17 empty states, §6.20 segmented control, §7.4 chips, §8 wording).
nav-title-unplaced = Unplaced devices
empty-nav-unplaced = No panel layout yet
empty-nav-unplaced-text = Devices are listed here once the location has an enclosure.
empty-nav-unplaced-done = All devices placed
empty-nav-unplaced-done-text = Every device of { $p0 } is placed.
reason-no-panel-page = Open a panel layout page
reason-no-unplaced-device = Choose a device in Unplaced devices
reason-no-placement = Select a placement on a rail or the plate
reason-layout-turn = Turn a placement on the plate; flip one on a rail
reason-layout-blocked = The placement cannot move further that way
reason-layout-refused = The drop is refused; nothing changed
tool-layout_place = Place device
hint-layout-place = Click or release to place; { $p0 } ends
hint-layout-idle = Drag a device from Unplaced devices onto the page
layout-off-rail = { $p0 } mounts on a { $p1 } rail; drop it on a rail.
layout-wrong-turn = { $p0 } turns a placement on the plate; { $p1 } flips one on a rail.
layout-blocked = Blocked by { $p0 }
layout-rail-end = At the rail end
layout-live-rail = { $p0 }, { $p1 } at { $p2 } mm, { $p3 }
layout-live-plate = { $p0 }, plate at { $p1 }, { $p2 } mm
layout-findings = { $p0 } findings
layout-findings-one = 1 finding
layout-findings-none = no findings
hud-layout-place = Place { $p0 } · Ctrl pushes · Alt swaps · Shift no snap · { $p1 } done

# FT-996: each key only where the selected placement takes it.
hud-layout-selected = { $p0 } selected · arrows nudge · { $p1 } deletes
hud-layout-selected-plate = { $p0 } selected · arrows nudge · { $p1 } rotates · { $p2 } deletes
hud-layout-selected-rail = { $p0 } selected · arrows nudge · { $p1 } flips · { $p2 } deletes
layout-mode-free = Free
layout-mode-push = Push
layout-mode-swap = Swap
layout-grid-5 = 5 mm
layout-grid-1 = 1 mm
layout-options-mode = Placing mode
layout-options-grid = Grid step
action-tool-place-layout = Place device
action-layout-next = Next placement on the rail
action-layout-previous = Previous placement on the rail
action-layout-nudge = Nudge placement
action-layout-rotate = Turn placement
action-layout-flip = Flip placement
action-layout-lock-selection = Lock
action-layout-unlock-selection = Unlock
action-layout-options = Placing options

# WP-S4.02 part 4 (D10, D11; §8; doc 16a §5.3, §6.12): Draw ▸ Layout, Project ▸ Enclosure…, the draw tools and
# the set-up dialogs.
group-panel-layout = Layout
bar-rail = Rail
bar-duct = Duct
bar-auto-fill = Auto-fill rail
bar-enclosure = Enclosure
action-tool-rail-draw = Rail
action-tool-duct-draw = Duct
action-layout-auto-fill-rail = Auto-fill rail…
action-layout-draw-options = Drawing options
action-layout-enclosure-new = Enclosure…
tool-rail_draw = Rail
tool-duct_draw = Duct
hint-rail-draw = Click the rail's start; { $p0 } cancels
hint-rail-draw-end = Click the rail's end; { $p0 } drops the start; { $p1 } cancels
hint-duct-draw = Click the duct's first point; { $p0 } cancels
hint-duct-draw-next = Click the next point; a double click or { $p0 } finishes; { $p1 } removes the last point
reason-no-draw-tool = Choose Rail or Duct first
reason-no-rail-part = Add a rail part to the project first
reason-rail-no-profile = The rail part has no profile; choose a rail part with a rail section.
reason-duct-two-points = A duct needs two points
reason-duct-vertices = A duct takes at most 256 points
draw-length = { $p0 } mm
draw-duct-size = { $p0 } × { $p1 } mm
draw-options-part = Part
draw-options-width = Width (mm)
draw-options-height = Height (mm)
draw-options-no-part = No part
enclosure-new-title = Create enclosure
enclosure-new-ok = Create
form-enclosure-location = Location
form-enclosure-part = Enclosure part
error-no-location = The project has no location { $p0 }; choose one of the list.
error-no-enclosure-part = No enclosure part { $p0 } is on offer; choose one of the list.
auto-fill-title = Auto-fill rail
auto-fill-ok = Fill
form-auto-fill-rail = Rail
form-auto-fill-order = Order
auto-fill-order-schematic = Schematic
auto-fill-order-dt = DT
auto-fill-devices-all = Fills with every unplaced device that fits the rail.
auto-fill-devices-marked = Fills with the { $p0 } devices marked in Unplaced devices.
auto-fill-devices-marked-one = Fills with the device marked in Unplaced devices.
error-no-rail = The page has no rail; draw one first.
toast-auto-fill = Placed { $p0 } on { $p1 }.
toast-auto-fill-left = Placed { $p0 } on { $p1 }; { $p2 }.
enclosure-new-no-location = The project has no location yet; add one with { $p0 } first.
enclosure-new-no-part = No enclosure part is in the project or an installed library; type a part number.
auto-fill-devices = { $p0 } devices
auto-fill-devices-one = 1 device

# Cycle 44, FIX44b (W44.2 F1): why the Create enclosure dialog's Create is disabled.
reason-enclosure-location = Choose a location
reason-enclosure-part = Type or choose a part number

# Cycle 44, FIX44d (W44.4 F-d): the heading of a panel page's Surface, Label mode and Datum rows.
category-surface_view = Surface view

# WP-S2.08 part 3 (D10, ID-1219 ruling 5): View ▸ Language, one item per UI language under the language's own name
# (`language-name`, the message of that language's file).
submenu-language = Language
action-view-language = Language
language-name = English
# Cycle 49, the Dutch UI's fix round (ID-1278). Ruling 1: the labels of the palette's parameter forms, `param-<name>`
# (`_` as `-`), the label derived from the name before.
param-by = By
param-discard = Discard
param-index = Index
param-kind = Kind
param-mode = Mode
param-name = Name
param-on = On
param-pt = Pt
param-tab = Tab
param-type = Type
param-value = Value
param-voltage = Voltage
# Ruling 2: the words of a closed set's choice values, `choice-<value>` (`_` as `-`), the words derived before.
choice-false = False
choice-true = True
choice-blocks = Blocks
choice-cables = Cables
choice-check = Check
choice-comfortable = Comfortable
choice-compact = Compact
choice-connections = Connections
choice-devices = Devices
choice-draw = Draw
choice-flat = Flat
choice-folio = Folio
choice-ips = Ips
choice-libraries = Libraries
choice-macros = Macros
choice-messages = Messages
choice-output = Output
choice-page = Page
choice-pages = Pages
choice-parts = Parts
choice-plc = Plc
choice-potentials = Potentials
choice-project = Project
choice-section = Section
choice-strips = Strips
choice-structure = Structure
choice-type = Type
choice-unplaced = Unplaced
choice-view = View
choice-title-page = Title page
choice-table-of-contents = Table of contents
choice-section-divider = Section divider
choice-graphic = Graphic
choice-single-line = Single-line
choice-multi-line = Multi-line
choice-overview = Overview
choice-bus-topology = Bus topology
choice-fluid = Fluid
choice-assembly-overview = Assembly overview
choice-panel-layout = Panel layout
choice-model-view = Model view
choice-external-document = External document
choice-functional-assignment = Functional assignment
choice-function = Function
choice-installation-site = Installation site
choice-location = Location
choice-document-kind = Document kind
choice-user-defined = User-defined
choice-window = Window
choice-checker = Checker
choice-engineer = Engineer
choice-author = Author
choice-panel-builder = Panel builder
choice-customer = Customer
choice-end-user = End user
choice-device = Device
choice-terminal = Terminal
choice-cable = Cable
choice-wire = Wire
choice-ferrule = Ferrule
choice-marker = Marker
choice-duct = Duct
choice-rail = Rail
choice-busbar = Busbar
choice-end-bracket = End bracket
choice-enclosure = Enclosure
choice-mounting-plate = Mounting plate
choice-tube = Tube
choice-accessory = Accessory
choice-configured = Configured
# Ruling 3: the finding detail's kind, by rule kind (`ew_checks::RuleKind`).
finding-kind-safety = Safety
finding-kind-conflict = Conflict
finding-kind-completeness = Completeness
finding-kind-hygiene = Hygiene
finding-kind-integrity = Integrity
# Ruling 6: the count texts' singular.
find-results-one = 1 result
label-paste-one = paste 1 record
find-updated-fewer-one = Preview updated: 1 fewer result
find-updated-more-one = Preview updated: 1 more result
job-done-one = 1 page, { $p1 } KiB
toast-export-pdf-one = Exported { $p0 }: 1 page, { $p2 } KiB
toast-export-pdf-again-one = Exported { $p0 } again: 1 page, { $p2 } KiB
# Ruling 7: a terminal strip without a tag yet, and the Terminal strips table of a project with strips but no terminal.
strip-no-tag-yet = New terminal strip · no tag yet
empty-nav-terminals = No terminals yet
empty-nav-terminals-text = Place a terminal with Draw ▸ Symbol… and the terminal symbol; it goes into the strip you choose there.
# C49-p6#1 and #2 (ID-1278): the choice words of every closed set, and four more singular count texts.
choice-all = All
choice-current = Current
choice-sections = Sections
choice-next = Next
choice-previous = Previous
choice-printer = Printer
choice-viewer = Viewer
choice-left = Left
choice-right = Right
choice-first = First
choice-last = Last
choice-free = Free
choice-push = Push
choice-swap = Swap
choice-schematic = Schematic
choice-dt = Dt
choice-dot = Dot
choice-bevel = Bevel
choice-star = Star
choice-chain = Chain
choice-style-default = Style default
choice-source = Source
choice-target = Target
choice-undirected = Undirected
choice-name = Name
choice-potential = Potential
choice-line = Line
choice-polyline = Polyline
choice-rect = Rect
choice-circle = Circle
choice-arc = Arc
choice-polygon = Polygon
choice-end-plate = End plate
choice-end-clamp = End clamp
choice-partition = Partition
choice-cover = Cover
choice-pelv-0v-pe = Pelv 0v pe
choice-fe-pe = Fe pe
choice-equipotential = Equipotential
choice-shield = Shield
choice-dotted = Dotted
choice-dash-dot = Dash dot
choice-per-wire = Per wire
choice-deviations = Deviations
choice-explicit = Explicit
choice-none = None
choice-selection = Selection
choice-complete = Complete
choice-discard = Discard
choice-language = Language
choice-origin = Origin
choice-privacy = Privacy
choice-appearance = Appearance
choice-start = Start
choice-primary = Primary
choice-middle = Middle
choice-xlsx = Xlsx
choice-csv = Csv
choice-navigator = Navigator
choice-canvas = Canvas
choice-properties = Properties
choice-find = Find
choice-commandbar = Commandbar
choice-history = History
choice-wizard = Wizard
choice-sample = Sample
choice-qet = Qet
choice-new = New
choice-edit = Edit
choice-clear = Clear
choice-override = Override
choice-reset = Reset
choice-freeze = Freeze
choice-unfreeze = Unfreeze
choice-add = Add
choice-remove = Remove
choice-up = Up
choice-down = Down
choice-regex = Regex
choice-match-case = Match case
choice-whole-word = Whole word
choice-in-selection = In selection
choice-bridge-part = Bridge part
choice-accessory-part = Accessory part
choice-favourites = Favourites
choice-recent = Recent
choice-adapt = Adapt
choice-update = Update
choice-report = Report
choice-page-based = Page-based
label-paste-dropped-one = , 1 connection dropped
banner-findings-one = 1 integrity finding was found when the project was opened.
revision-waivers-one = Approving also acknowledges 1 safety waiver in force.
# WP-S2.08 part 6: the glossary pane (D16), the one-time tips (D17) and the wire-colour view (D18).
action-help-glossary = Glossary
action-glossary-filter = Search the glossary
action-view-wire-colours = Wire colours
pane-glossary = Glossary
glossary-term = EW term
glossary-eplan = EPLAN
glossary-qet = QET
glossary-definition = Definition
glossary-actions = Actions
glossary-search = Search terms
glossary-insert = Insert
glossary-insert-name = Insert: { $p0 }
empty-glossary = No term matches
empty-glossary-text = The search covers every column, in the UI language and in English.
tip-line-on-cp = Lines do not connect. Draw a wire instead.
tip-derived-tag = Tags are derived from the structure and the counter.
tip-generated-page = This page is generated from a report block.
toast-convert = Convert to connection
toast-show-block = Show report block
glossary-row-description = EPLAN: { $p0 }; QET: { $p1 }; { $p2 }
glossary-none = none

# WP-S1.17 parts 6 to 8 (COL49): external change, cloud folders, .ewz, the upgrade, the merge driver.
command-external-apply = External change
toast-show-changes = Show changes
external-records = { $p0 } records
external-records-one = 1 record
toast-external = External change applied: { $p0 }
toast-external-folios = External change applied: { $p0 } on folios { $p1 }
toast-external-folio = External change applied: { $p0 } on folio { $p1 }
toast-external-git = {" "}(git { $p0 })
external-absent = (none)
external-whole-record = the whole record
banner-external-collisions = The project changed on disk, and the change collides with your edits. Saving waits until you choose yours or theirs for each field.
banner-external-unreadable = The project on disk changed and cannot be read; your edits are kept. Save a copy, or wait until the disk is complete.
reason-no-external-change = No change on disk is waiting for a choice
error-external-apply = The change on disk could not be applied: { $p0 }.
external-title = Keep your changes or the ones on disk?
external-text = { $p0 } fields were changed here and on disk. Choose yours or theirs for each; saving waits until you do.
external-text-one = 1 field was changed here and on disk. Choose yours or theirs; saving waits until you do.
external-apply = Apply
external-all-mine = All mine
external-all-theirs = All theirs
external-later = Decide later
external-base = Base
external-mine = Mine
external-theirs = Theirs
banner-older-format = This project is in an older format; saving converts it.
upgrade-title = Convert this project to the current format?
upgrade-text = This project is in the older format { $p0 }. Saving converts it to format { $p1 }, which older versions of ElektraWatt cannot open.
upgrade-backup = A backup of the project as it is now is written beside its folder first.
upgrade-button = Upgrade and save
banner-cloud = This project is in a cloud-synced folder ({ $p0 }). Two people can open it within the sync delay; use git or a network share for teams.
banner-cloud-other = This project is in a cloud-synced folder. Two people can open it within the sync delay; use git or a network share for teams.
ewz-title = How do you want to open this .ewz?
ewz-text = { $p0 } holds a whole project. Extract it into a new folder beside it to edit it, or open it read-only.
ewz-extract = Extract and open
ewz-read-only = Open read-only
error-extract-ewz = { $p0 } could not be extracted: it is not a whole project, or the folder beside it cannot be written; nothing was written.
error-extract-exists = { $p0 } could not be extracted: the folder { $p1 } exists beside it, and an .ewz is extracted into a new folder only; nothing was written.
error-export-ewz = Exporting the .ewz failed: { $p0 }.
toast-ewz = Exported { $p0 }
toast-ewz-unsaved = Exported { $p0 } with the saved state; the unsaved changes are not in it
git-setup-title = Set up the merge driver?
git-setup-text = ElektraWatt writes its merge driver and diff into this repository's git configuration, and the managed .gitattributes and .gitignore blocks into the project folder.
git-setup-button = Set up
toast-git-setup = Merge driver set up
error-git-setup = The merge driver could not be set up: { $p0 }.
reason-not-in-git = The project folder is not in a git repository
reason-no-ew = The ew command is not installed beside ElektraWatt
status-driver-missing = Merge driver not configured
status-driver-tip = Set up merge driver: git then merges this project's files record by record
saveerr-external-collisions = the change on disk collides with your edits; choose yours or theirs first
saveerr-external-unreadable = the project on disk changed and cannot be read; your edits are kept
prompt-export-ewz-title = Export .ewz
prompt-ewz-name = project.ewz
prompt-open-ewz-title = Open .ewz
action-file-open-ewz = Open .ewz…
action-file-export-ewz = Export .ewz…
action-project-check-disk = Check for changes on disk
action-file-upgrade = Upgrade and save
action-external-apply = Apply the change on disk
action-external-choose = Choose a side
action-git-setup = Set up merge driver
choice-extract = Extract
choice-read-only = Read only
choice-mine = Mine
choice-theirs = Theirs
