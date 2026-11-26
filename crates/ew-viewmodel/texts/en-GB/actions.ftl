# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: the menus, the actions and the commands' titles, the palette and the keymap. One
# message per key; `{ $p0 }`, `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback
# of every other.

# Menus and submenus.
menu-file = File
menu-edit = Edit
menu-view = View
menu-page = Page

# The app menu's tab menus (doc 16a §9.2); Insert became Draw, Tools dissolved.
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

# The human titles of the commands: what the window says for a command it previews
# ("Preview of Place device"), verb first, sentence case; the key is `command-<name>` with every `.` of the
# name as `-` (`command_title`). The parameters, the describe text and the scripts keep the command ID.
command-assembly-attach = Attach assembly
command-bond-create = Bond potentials
command-bus-logical_net-create = Create logical bus network
command-bus-physical_net-create = Create physical bus network
command-bus-place_ports = Draw a device's bus ports
command-bus-cable-set = Set the cable and plugs of a bus line
command-box-black = Draw black box
command-box-reshape = Reshape box
command-box-redraw = Redraw boxes
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
command-layout-apply = Route wires
command-device-create = Create device
command-device-link = Link device
command-device-place = Place device
command-edit-delete = Delete
command-edit-paste = Paste
command-external_ref-create = Create external reference
command-ip-supply_in = Mark where the supply comes in
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
command-page-copy = Copy pages
command-part-assign = Assign part
command-part-placeholder = Assign a part of unknown type
command-parts-import = Import parts
command-plc-bind_channel = Bind PLC channel
command-plc-config_project-create = Create PLC configuration project
command-plc-set_module_part = Set PLC module part
command-plc-set_profile = Set PLC address profile
command-plc-set_start = Set PLC module start
command-plc-freeze = Freeze PLC addresses
command-plc-readdress = Re-address PLC modules
command-plc-convert_manual_to_auto = Make manual PLC addresses automatic
command-plc-add_module = Add PLC module
command-potential-define = Define potential
command-project-assets-purge = Purge unused assets
command-project-checks-set = Set checks
command-project-company-set = Set company
command-project-settings-set = Set project settings
step-settings-drawing = Set drawing data
step-settings-frames = Set frames
step-settings-languages = Set languages
step-settings-supply = Set supply
command-project-typeface-set = Set the project's typeface
command-rep-exchange_symbol = Exchange symbol
command-rep-join_device = Join device
command-rep-move = Move
command-rep-move_to_new_page = Move to a new page
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
command-terminal-face_card = Terminal wires to card side
command-terminal-insert = Insert terminal
command-terminal-move = Move terminal
command-valve-add = Add valve
command-valve-set_occupancy = Set valve position
command-valve_terminal-generate_overview = Generate a valve terminal's overview
command-valve_terminal-draw_slots = Draw a valve terminal as its positions
command-valve_terminal-draw_valves = Draw a valve terminal's valves in it
command-fluid-port-close = Close fluid port
command-fluid-medium-edit = Edit medium
command-fluid-medium-define = Define medium at port
command-fluid-tube-set = Set tube
command-fluid-length-set = Set tube length
command-part-edit_copy = Edit part
command-workshop-stock = Set the workshop stock
command-part-delete = Delete a part of the project's own
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

command-page-surface_view-set = Set panel view
command-page-set_grid = Change a page's connection grid
command-layout-place_assembly = Place assembly
command-layout-auto_fill = Auto-fill rail
command-layout-enclosure-create_from_part = Create enclosure from part
command-layout-space-edit = Rename layout space
command-layout-enclosure-edit = Move enclosure
command-layout-surface-edit = Edit plate

# Actions.
action-file-open = Open…

# The canvas tools.
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

# The place-and-arrange tools.
action-tool-place = Symbol…
action-tool-place-part = Device from part…
action-place-pick = Pick a symbol to place
action-picker-filter = Filter the picker
action-picker-row = Move the picker's row
action-picker-choose = Choose the picker's row
action-picker-new-page = Place on a new page with its grid
action-picker-category = Choose a category of the browser
action-picker-open = Open or close a part's functions

# The Join chooser.
action-place-join = Join device…
action-chooser-row = Move the chooser's row
action-chooser-type = Type ahead in the chooser
action-chooser-choose = Choose the chooser's row
action-strip-new = Terminal strip
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
action-part-give-size = Give it a size…
action-part-details = Part details
action-part-edit-card = Edit part…
action-tool-wire = Wire
action-canvas-activate-alt = Click with Alt at the grid cursor
action-tool-next-proposal = Next autoconnect proposal
action-tool-connect-on-move = Connect while moving
action-layout-route = Route
tooltip-layout-route = Route the wires of the selection, or of the page. Symbols stay where they are.
action-tool-wire-dot = Dot at the joint
action-route-handle = Next handle of the selected wire
action-route-handle-move = Move the wire's handle
action-junction-open = Junction sequence…
action-junction-member = Move a junction member
action-junction-style = Junction style
action-junction-wiring = Junction wiring
action-project-crossings = Show crossings as hops
action-project-wire-numbers = Show wire numbers on the pages
action-project-wire-numbering = Number wires by potential
action-tool-fluid-line = Tube
action-fluid-tube-kind = Line kind: next
action-nav-show = Show
action-fluid-draw-terminal = Draw the valve terminal on a fluid page…

action-tool-ip = Interruption point
action-tool-black-box = Black box…
action-tool-ip-direction = Interruption point direction
action-tool-star-source = Star source
action-ip-name = Interruption point name

# The name field's tooltip names the keys that end it.
action-ip-name-hint = Enter or Tab ends the name and gives the keys back to the tool
action-ip-external = External reference
action-ip-continue-on = Continue on page…
# The selected interruption points are where the supply comes in from another drawing set.
action-ip-supply-in = Supply comes in here…
# H9: the supply marks' Potential panel: a row per mark placed, its conductor and voltage.
action-ip-supply-potential = Potential…
action-ip-supply-apply = Apply the potentials
action-ui-app-menu = App menu
action-ui-context-menu = Context menu
# Supply on the Draw tab: what comes in from the mains, placed as supply marks in one go.
action-supply-place = Supply…
action-ip-arrange-selected = Arrange interruption points
action-box-grip = Move a black box grip
action-box-add-cp = Add pin
action-box-centre-pins = Centre the pins
action-box-remove-pin = Remove pin
context-box-pin-part = Add pin (in the part's device box)…
context-box-pin-remove = Remove pin { $p0 }
action-page-fit-grid = Change the connection grid…

submenu-graphic = Graphic
action-tool-text = Text
action-tool-graphic = Graphic
action-text-bind = Text binding
action-text-edit = Edit text
action-strip-bridge = Bridge terminals

# The strip editor (§6.9, §6.10).
action-strip-edit = Strip editor
action-strip-insert = Insert terminal
action-strip-move = Move terminals
action-strip-add-accessory = Add accessory
action-strip-unbridge = Remove bridge

# swap cores, and Delete and F2 in navigator trees (§7).
action-cable-swap-cores = Swap cores
action-navigator-delete = Delete rows
action-navigator-rename = Rename

# G-PP triage row 7: Draw ▸ Bond.
action-bond-draw = Bond
submenu-bond = Bond
# Draw ▸ Define potential…, beside Bond.
action-potential-draw = Define potential…
action-potential-draw-row = Define potential…
# The card of a potential reasoned for the selected wire.
action-potential-offer = Answer the offered potential

action-tool-image = Image…
action-image-pick = Image asset
action-image-file = Image file

# The Wire marker tool, the Structure box tool and the marker mode.
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
action-file-open-recent = Open recent project
action-file-save = Save
action-file-save-as = Save as…
action-file-export-pdf = Export PDF…

# The pages as a DXF set (doc 09 §4.9), a folder of page files and the pictures they name.
action-file-export-dxf = Export DXF…

# The print pre-dialog.
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
action-view-zoom-to = Zoom to
action-view-pan = Pan the canvas
action-view-canvas-resize = Resize the canvas
action-canvas-pointer = Point at the canvas
action-canvas-snapshot = Write canvas snapshot
submenu-pan = Pan
action-view-ui-scale = UI scale
action-view-theme = Theme
action-view-density = Density
action-view-min-font = Minimum text size
action-view-reduce-motion = Reduce motion
# The plain mouse wheel scrolls the canvas, Ctrl with the wheel zooms; off, the wheel zooms.
action-view-wheel-scrolls = Mouse wheel scrolls
action-view-tips = Show tips

# The command bar (doc 16a §5).
action-view-command-tab = Command tab
action-view-command-bar = Collapse command bar
submenu-command-tab = Command tab
group-project = Setup
group-parts = Parts
group-standards = Templates
group-import = Import
group-pages = Pages
group-order = Order
group-go = Go
group-select = Select and wire
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
group-workshop = Workshop
group-jobs = Jobs
group-zoom = Zoom
group-show = Show
group-panels = Panels
group-layout = Layout
group-appearance = Appearance
action-tool-place-terminal = Terminal…

# The place of a key in a hint, a HUD prompt or a toast when the keymap binds its action to none.
key-none = (no key)
action-tools-palette = Command palette
action-help-shortcuts = Keyboard shortcuts
action-help-about = About ElektraWatt
action-command-run = Run model command
action-view-snapshot = Write page snapshot
action-ui-describe = Describe the user interface
action-ui-actions = List the actions
action-script-expect-refused = Expect a refusal
action-script-expect-failed = Expect a failure
action-ui-dismiss = Dismiss popup or dialog

# The palette and the dialogs.
palette-title = Command palette
palette-hint = Type a command…
palette-no-match = No matching command
palette-form-hint = Enter applies · Esc goes back
palette-form-hint-preview = Enter shows the preview: nothing changes before Apply · Esc goes back
palette-switch-on = On, Enter switches it off
palette-switch-off = Off, Enter switches it on
shortcuts-title = Keyboard shortcuts

# The text of the describe output.
describe-menu = menu { $p0 }
describe-item = item { $p0 } "{ $p1 }"

# Part 2: menus, actions and reasons.
submenu-workspace = Workspace
action-file-new = New project…
action-file-open-sample = Open sample project
action-file-import-qet = Import QElectroTech project…
action-parts-import = Parts table…
action-place-variant = Variant of the part placed
action-workshop-add-generic = Add the generic stock
action-workshop-edit-stock = Edit stock…
action-workshop-stock-row = Move or remove a stock part
action-workshop-stock-add = Add a stock part
action-workshop-stock-cut = Cut to
action-workshop-stock-apply = Apply the stock
action-file-discard-journal = Discard unsaved changes of the earlier session…
action-view-workspace-load = Load workspace
action-view-workspace-save = Save layout as…
action-view-workspace-reset = Reset layout
action-tools-company-standard = New template…
action-help-documentation = What is this?
action-help-tutorials = Tutorials
# Help ▸ Tutorials, one entry per tutorial of the guide, by its page's title.
submenu-tutorials = Tutorials
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
action-wizard-finish = Wizard: create the template
tooltip-learn-more = F1: help
keymap-ew = EW
keymap-kicad = KiCad-like
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

# A link row of the grid followed.
action-props-follow = Go to the linked record
action-props-filter = Filter the properties
action-cp-unused = Mark not used
action-props-action = Run a property action
action-props-list = Change a list property
action-props-mltext = Texts per language
action-preview-apply = Apply the preview
action-preview-cancel = Cancel the preview
action-preview-take-file = Take the file's values
action-toast-undo = Undo the last edit
action-toast-dismiss = Close the notification
action-jobs-show = Jobs
action-lists-parts = Open parts list
action-lists-wires = Open wire list
action-rework-show = Rework list…
action-rework-archive = Compare with an archive file
action-rework-export = Export the rework list
action-labels-show = Labels…
action-labels-tick = Tick a label file
action-labels-write = Write the label files
action-labels-columns = Label file columns…
action-workshop-settings = Workshop settings
action-jobs-cancel = Cancel job
action-jobs-wait = Wait for job
action-jobs-open-result = Open result
action-notifications-show = Notifications
action-notifications-run = Run notification action
action-journal-recover = Recover
action-journal-keep = Open read-only
action-startup-recover = Recover a project of an earlier session
action-startup-open-log-folder = Open log folder
action-startup-delete-log = Delete crash log
action-startup-dismiss = Not now
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

action-page-describe = Describe the page content
action-find-replace-all = Replace all

# The column mapping step of the parts import.
action-parts-map = Map a column of the parts table
action-parts-map-import = Import the mapped parts table

# The clipboard.
action-edit-cut = Cut
action-edit-copy = Copy
action-edit-paste-clip = Paste
action-edit-paste-in-place = Paste in place
action-edit-move-to-page = Move to page…
action-edit-duplicate = Duplicate
action-edit-select-all = Select all
action-edit-copy-tags = Copy tag list
action-clipboard-text = Set the clipboard text

# The viewer.
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

# The navigators.
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

# A channel dragged from the PLC navigator onto a connection point.
action-navigator-bind = Bind channel
action-plc-attach-cards = Attach modules without a station
action-plc-attach-to = Attach to { $p0 }
action-fluid-generate-overview = Generate overview…
action-fluid-valve-add = Add valve
action-fluid-cell-fill = Fill position…
action-fluid-swap = Swap
action-fluid-draw-solenoid = Draw solenoid
action-fluid-draw-plug = Draw the plug on an electrical page…
action-fluid-plug-ports = Plug the open ports
action-fluid-go-output = Go to output
action-plc-rack-editor = Rack…
action-plc-rack-add = Put a part into the rack
action-plc-rack-move = Move module to a slot
action-fluid-set-medium = Set medium
action-fluid-medium-new = New medium…
action-plc-instrument-list = Import instrument list…
action-plc-symbol-policy = Symbol policy…
action-plc-module-safety = Safety data…
action-plc-channel-settings = Mode and safety…
action-plc-process-tag-scheme = Process-tag scheme…
action-fluid-settings = Fluid settings…
action-project-plc-addresses-at-terminals = Channel addresses at terminals
action-plc-preview = Preview PLC addressing
action-plc-export = Export to PLC software…
action-plc-import = Import from PLC software…
action-plc-import-tab = Import tab
action-plc-import-choose = Import choice
action-plc-import-apply = Apply the PLC import
menu-plc-readdress = Re-address…
menu-plc-convert-manual-to-auto = Make manual addresses automatic…
menu-plc-freeze = Freeze addresses…
action-fluid-draw-slots = Draw as slots
action-fluid-draw-valves = Draw its valves in the valve terminal
action-navigator-apply = Apply the preview
action-pages-reorder = Reorder pages
action-pages-move-up = Move page up
action-pages-move-down = Move page down

# highlighting.
action-navigator-highlight = Highlight
action-view-clear-highlights = Clear highlights

# table mode.
action-navigator-table = Table
action-table-columns = Columns
action-table-cell = Move the cell cursor
action-table-edit = Edit cell
action-table-input = Type into the cell
action-table-commit = Commit the cell
action-table-set = Set cells of the marked rows, else of the cursor row (the column by key or header label)
action-table-fill-down = Fill down
action-table-pattern-fill = Pattern fill…
action-table-paste = Paste cells
action-table-copy = Copy cells
action-table-clear = Clear cells
action-table-resolve = Choose a reading
action-table-drop = Leave a cell out

# A navigator table's export to a file and the round trip's import.
action-navigator-export-table = Export table…
action-navigator-import-table = Import table…
submenu-navigators = Navigators

# The message panel (§7.3, §7.6).
action-messages-show = Messages
action-messages-next = Next finding
action-messages-previous = Previous finding
action-messages-fix = Apply quick fix…
action-messages-check = Check project

# Waive… and Unwaive; the check toast's waived and pending counts in its title.
action-messages-waive = Waive finding…
action-messages-unwaive = Unwaive finding
action-ui-focus = Focus an area
action-ui-focus-next = Next area
action-ui-focus-previous = Previous area

# Creating pages, structure nodes and external references.
action-page-new = New page…
action-page-title-block = Title block fields…
action-page-grid = Connection grid…
action-structure-new = Structure node…
action-external-ref-new = External reference…
action-section-new = New section…
action-preview-show = Preview
action-history-show = History
action-view-bottom-dock = Bottom dock
action-history-row = Move in the history
action-history-undo-to = Undo to here
action-history-changes = Show changes
action-pages-renumber = Renumber pages…
action-pages-copy = Copy pages…
action-devices-renumber = Renumber devices…
action-project-purge-assets = Purge unused images…
action-rail-entry = Show navigator by number
action-rail-show = Show navigator
action-props-show-advanced = Show advanced properties
action-macros-insert = Insert macro…
action-macros-place = Place a macro
action-macros-value-set = Choose the macro's value set
action-macros-save = Save as macro…
action-libraries-update = Update…
action-props-make-variable = Make variable
action-part-choose = Choose part…
action-part-add = Add part…
action-part-search = Search parts
action-part-pick = Choose
action-part-placeholder = Type not known yet…
action-part-favourite = Favourite
action-part-edit = Edit part…
action-part-field = Edit field
action-part-save = Save
action-part-new = New part…
action-part-bus-row = Add or remove a bus interface
action-part-delete = Delete part…
action-part-section = Show a section of the part
action-part-table-row = Add or remove a row of the part's table
action-part-remove-own = Remove from my parts library…
action-paste-resolve = Resolve paste

# The keymap files, their rules and the editor.
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
keymap-reset-default = The action has its default keys; record a key to change them
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
keymap-in-supply-panel = in the Potential panel
keymap-in-symbol-editor = in the symbol editor
keymap-none = No action matches the search.
keymap-context-global = Everywhere
keymap-context-canvas = Canvas
keymap-context-navigator = Navigator
keymap-context-properties = Properties
keymap-context-find = Find
keymap-context-supply-panel = Potential panel
keymap-context-symbol-editor = Symbol editor

# The macro card's ad-hoc values and Save as macro's value-set table.
action-macros-value = Type a value of the macro card's variable

# The revision dialogs of the Check menu (Approve, Release, Withdraw, Set author),
# the incomplete-release offer at open, and the release's housekeeping warnings in Messages.
action-revision-approve = Approve revision…
action-revision-release = Release revision…
action-revision-withdraw = Withdraw revision…
action-revision-set-author = Set revision author…
action-revision-incomplete = Settle the incomplete release…
submenu-revision = Revision
action-tool-place-layout = Place device
action-layout-next = Next placement on the rail
action-layout-previous = Previous placement on the rail
action-layout-nudge = Nudge placement
action-layout-rotate = Turn placement
action-layout-flip = Flip placement
action-layout-lock-selection = Lock
action-layout-unlock-selection = Unlock
action-layout-options = Placing options

# Draw ▸ Layout, Project ▸ Enclosure…, the draw tools and
# the set-up dialogs.
group-panel-layout = Layout
action-layout-choose-plate = Mounting plate…
action-layout-locate-devices = Give location…
action-tool-rail-draw = Rail
action-tool-duct-draw = Duct
action-layout-auto-fill-rail = Auto-fill rail…
action-layout-draw-options = Drawing options
action-layout-enclosure-new = Enclosure…
action-layout-labels = Labels
action-devices-set-location = Location…

# View ▸ Language, one item per UI language under the language's own name
# (`language-name`, the message of that language's file).
submenu-language = Language
action-view-language = Language
# The glossary pane, the one-time tips and the wire-colour view.
action-help-glossary = Glossary
action-glossary-filter = Search the glossary
action-glossary-row = Move the glossary cursor
action-view-wire-colours = Wire colours

# External change, cloud folders, .ewz, the upgrade, the merge driver.
command-external-apply = External change
action-file-open-ewz = Open .ewz…
action-file-import = Import…
action-file-import-drawing = Import drawing…
action-file-import-eplan = Import EPLAN data…
action-file-export-ewz = Export .ewz…
action-project-check-disk = Check for changes on disk
action-file-upgrade = Upgrade and save
action-external-apply = Apply the change on disk
action-external-choose = Choose a side
action-external-review = Choose again between yours and the disk's
action-library-update-damaged = Update from library
action-git-setup = Set up merge driver
action-file-import-qet-apply = Apply the import

# The re-import of a QElectroTech file into the open project: the proposal (T6), the refusal (T7),
# the re-import card (T8) and its rows.
action-file-reimport-qet = Re-import QElectroTech project…
action-file-reimport-qet-apply = Apply the re-import

# The Messages pane's migration report (T11) beside the project's scheme.
action-messages-migration = Migration report

# Draw ▸ Connect ▸ New cable… and Connect cores….
action-cable-new = New cable…
action-cable-connect-cores = Connect cores…

# Draw ▸ Connect ▸ Cable…: the selected wires as a cable's cores, in one form.
action-cable-from-wires = Cable…

# F20: Cable… on a bus line: its cable and the plug at each end.
action-bus-cable = Bus cable…

# Output ▸ Reports ▸ Report block….
action-report-block-new = Report block…
action-symbol-new = Symbol editor…
action-symbol-open = Open symbol…
action-symbol-fork = Fork symbol…
# The browsers' Edit symbol and Copy and edit.
action-symbol-edit = Edit symbol
action-symbol-copy-edit = Copy and edit
action-symbol-close = Close symbol editor
action-symbol-replace = Replace symbol draft
action-symbol-undo = Undo symbol edit
action-symbol-redo = Redo symbol edit
action-symbol-variant = Preview symbol variant
action-symbol-validate = Validate symbol
action-symbol-draw = Draw symbol graphic
action-symbol-cp = Edit symbol connection point
action-symbol-slot = Add symbol text slot
action-symbol-export = Export symbol draft…
action-symbol-save = Save symbol to library
action-symbol-place = Place edited symbol
action-symbol-candidate = Choose imported symbol
action-symbol-function = Assign function definition
action-symbol-link = Add mechanical link point
action-symbol-scratch = Test symbol on a new page
action-symbol-new-page = Place on a new page with its grid
action-symbol-path = Draw a polyline or polygon
action-symbol-map-cp = Map a symbol connection point to a function pin
action-file-save-project = Save project
action-symbol-directory = Choose symbol library folder
action-symbol-tool = Choose symbol drawing tool
action-symbol-role = Set the potential role of a symbol connection point
command-plc-record_exchange = Record PLC exchange
command-plc-apply_exchange = Apply PLC import
command-plc-unlock = Unlock PLC-owned fields
command-plc-apply_instrument_list = Apply instrument list
# Persona round (M3): the Output tab's Report and PLC exchange, and where lists are made.
group-exchange = PLC exchange
# A PLC station is made as a rack and filled.
command-plc-remove_from_rack = Take out of the PLC rack
command-plc-add_station = Make PLC station
action-plc-new-station = New station…
action-plc-rack-set = Choose for the new station
action-plc-rack-slot = Change a slot
action-plc-rack-create = Create station
action-plc-rack-next = Next step for the station
action-plc-rack-rename = Change a tag in the station
action-plc-rack-channel = Name a channel in the station
action-plc-add-to-plc = Add to PLC
# The PLC and Fluid tabs of the command bar and the workspace switch.
menu-plc = PLC
menu-fluid = Fluid
group-station = Station
group-addresses = Addresses
group-bus = Bus
group-plc-lists = Lists
group-plc-workspace = Workspace
group-tubes = Tubes
group-fluid-place = Place
group-media = Media
group-fluid-lists = Lists
group-fluid-workspace = Workspace
action-bus-page-new = New bus page…
action-bus-place-port = Place port
action-bus-panel = Bus panel
action-plc-grid = PLC grid
action-plc-workspace = PLC workspace
action-plc-guide = PLC guide
action-fluid-page-new = New fluid page…
action-fluid-symbol = Fluid symbol…
action-fluid-valve-terminal = Valve terminal…
action-fluid-air-supply = Air supply…
action-fluid-media = Medium table
action-fluid-tubes = Tube table
action-fluid-workspace = Fluid workspace
action-fluid-guide = Fluid power guide
submenu-plc-addressing = Addressing
submenu-guides = Guides
submenu-tutorials-what = Lessons you follow step by step
submenu-guides-what = How to do one job
action-help-guide = Guide

# A layout page that can be read: item numbers, the legend, labels that move.
action-page-add-legend = Add legend
action-layout-reset-label = Reset label position
action-layout-reset-labels = Reset label positions
command-layout-labels_reset = Reset layout labels
# The legend as a page of its own after its layout page (doc 12 §5.4).
action-report-block-legend-to-page = Move the legend to its own page
# An older layout page given the whole sheet in one step (doc 12 §5.1).
action-layout-fit-sheet = Fit the layout to the sheet
command-layout-label_move = Move layout label

# A symbol's text moved on its own (doc 06 §2.5).
action-rep-hide-text = Hide text
action-rep-reset-texts = Put the text back
command-rep-text_set = Move a symbol's text
command-rep-texts_reset = Reset text positions

# A symbol kept in place: Arrange and route leaves it where it stands (doc 06a §2.4).
action-rep-keep-in-place = Keep in place
action-text-function-over-path = Function text over its path
action-text-strip-name = Name the terminal strip here
action-symbol-discipline = Set what the symbol is drawn for: electrical, pneumatic, vacuum, hydraulic or bus
command-plc-draw_station = Draw PLC station
command-plc-bind_pins = Bind channels to their pins
action-messages-fix-all = Fix all
command-check-fix_all = Fix all of a kind
action-bus-put-station = Put a station on this page…
submenu-bus-stations = Put a station on this page
submenu-bus-one-port = Or one port
context-edit-box = Edit this device's box…
action-part-box = Pins and box…
action-part-pin = Add, remove or change a pin of the part
action-part-box-size = Size the part's device box
action-part-box-select = Select pins of the device box
action-part-box-details = Part data…
action-part-redraw-boxes = Redraw the part's boxes
action-symbol-select = Select things of the symbol
action-symbol-move = Move the selection of the symbol
action-symbol-nudge = Move the selection of the symbol a grid step
action-symbol-delete = Delete the selection of the symbol
action-symbol-copy = Copy the selection of the symbol
action-symbol-cut = Cut the selection of the symbol
action-symbol-paste = Paste into the symbol
action-symbol-duplicate = Duplicate the selection of the symbol
action-symbol-align = Line up the selection of the symbol
action-symbol-grip = Drag a handle of the selected shape
action-symbol-grid = Set the symbol editor's drawing grid
action-symbol-start = The symbol editor's start card
action-symbol-prop = Set a property of the symbol or of its selection
action-plc-rack-select = Select a place in the rack
action-plc-module-replace = Replace module…
action-plc-module-remove = Remove module
action-plc-module-draw = Draw module on a page…
action-plc-module-move = Move module to another page…
action-plc-module-goto = Go to the module's page
action-plc-module-place = Place here
action-plc-card-redraw = Draw the card in one row
action-plc-card-terminals-face = Terminal wires to card side
action-plc-module-left = Move module left
action-plc-module-right = Move module right

# The spacing of a terminal row.
action-terminals-spacing = Spacing…
action-templates-open = Templates…
action-project-save-as-template = Save as template…
action-project-check-level = Check level…
action-new-project-set = New project: set a field
action-new-project-browse = New project: choose the folder
action-new-project-other = New project: choose a project to start from
action-templates-select = Templates: select a template
action-templates-use = Templates: use for new projects
action-templates-edit = Templates: edit a template
action-templates-rename = Templates: rename a template
action-templates-remove = Templates: remove from the list
action-templates-add = Templates: add a folder
action-text-styles-open = Text styles…
action-text-styles-set = Text styles: set a style
action-text-styles-reset = Text styles: reset
action-text-styles-standard = Text styles: standard sizes
action-text-styles-typeface = Text styles: choose a font file
action-text-styles-typeface-clear = Text styles: back to the typeface of the program
action-section-edit = Edit section…
action-wizard-folder = Choose another folder…
# Arrange and route (docs/design/06a-drafting.md §7.1, §7.3).
action-layout-arrange = Arrange and route
tooltip-layout-arrange = Line up the symbols of the selection, or of the page, and route their wires. You see the result before anything changes.
action-layout-arrange-tick = Tick or untick rows of the arranging preview
action-layout-arrange-as-is = Show the page as it is

# Doc 09 §3.8: Output ▸ List columns… and Rows of your own… of the list on show.
action-report-block-columns = List columns…
action-report-block-rows = Rows of your own…
action-field-new = New field of your own…
action-field-rename = Rename…
action-field-remove = Remove
command-user_prop-define = Make a field of your own
command-user_prop-rename = Rename a field of your own
command-user_prop-remove = Remove a field of your own
group-frame = Frame
action-frame-edit = Edit frame
action-frame-choose = Choose on the frame
action-frame-split = Split the cell
action-frame-join = Join the cells
action-frame-clear = Clear the cell
action-frame-picture = Picture…
command-form-edit = Change a frame or title block
command-frame-set_grid = Set the columns and rows of a frame
action-frame-adopt = Use the frames of a template…
action-frame-own-page = Give this page its own frame
command-form-make_own = Make a frame or title block of its own
command-frames-adopt = Use the frames of a template
dialog-preview = Preview
