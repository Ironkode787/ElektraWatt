# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: the window's frame: the command bar, the rail, the panes and tabs, the status bar,
# the bars of the panes, the View settings and the messages of a script. One message per key; `{ $p0 }`, `{ $p1 }`,
# ... are the arguments, always strings. The source language and the fallback of every other.

# A UI scale in per cent.
scale = { $p0 } %
min-font-off = Off
min-font-pt = { $p0 } pt
status-handle-moves-up-down = This handle moves up and down only: press ↑ or ↓.
status-handle-moves-sideways = This handle moves left and right only: press ← or →.
status-handle-cannot-move = The handle cannot go that way: the wire's ends stay on their connection points and its corners stay square.
status-handle-refused = The handle did not move: { $p0 }
bar-supply = Supply
status-cursor = x { $p0 } y { $p1 } mm
status-zoom = { $p0 } %
status-grid-qet = QET 10 px
status-grid-mm = { $p0 } mm
status-grid-off = Grid off
message-no-canvas = The page cannot be shown: { $p0 }
bar-properties = Properties
bar-structure = Structure

bar-move-up = Move up
bar-move-down = Move down
bar-go-to = Go to
bar-follow-reference = Follow reference
bar-device = Device
bar-symbol-editor = Symbol editor
bar-terminal = Terminal
bar-marker = Marker
bar-wire = Wire
bar-interruption = Interruption
bar-rotate = Rotate
bar-mirror = Mirror
bar-renumber = Renumber devices
bar-check-project = Check project
bar-navigators = Navigators
bar-workspaces = Workspaces
commandbar-name = Command bar
commandbar-tabs = Command tabs
commandbar-app = ElektraWatt
commandbar-menu = Menu
commandbar-search = Search commands and devices
commandbar-search-key = Ctrl K
commandbar-more = More
commandbar-variants = Variants
zoom-control = Zoom
message-no-snapshot = The snapshot needs the page renderer, which this runner does not have.
message-expected-refusal = expected a refusal; the step was accepted
message-expect-refused-inner = script.expect_refused needs `action`, an action ID, and `params` as an object
message-expected-failure = expected a failure; the step did not fail
message-expect-failed-inner = script.expect_failed needs `action`, an action ID other than script.expect_refused and script.expect_failed, and `params` as an object

# The window, the start page, the page view.
window-title = { $p0 } - ElektraWatt
window-title-start = ElektraWatt

# The status bar.
status-hint-start = No project open
status-page = { $p0 } ({ $p1 } of { $p2 })
status-saved = saved
status-unsaved = *
status-read-only = read-only: { $p0 }
status-language = EN
workspace-schematic = Schematic
workspace-plc = PLC
workspace-fluid = Fluid power
language-en = English
language-nl = Nederlands
theme-system = Follow the system
theme-light = Light
theme-dark = Dark
theme-hc-dark = High contrast dark
theme-hc-light = High contrast light
density-comfortable = Comfortable
density-compact = Compact
pane-pages = Pages
pane-page-view = Page view

# The property grid.
pane-properties = Properties
bar-lists-parts = Parts list
pane-jobs = Jobs
status-notifications = Notifications { $p0 }
status-job = { $p0 } { $p1 } %
status-job-phase = { $p0 }: { $p1 }

# Doc 16a §6.5: the filter field's trailing clear button.
filter-clear = Clear the filter

# find and replace.
pane-find = Find and replace
status-tip-zoom = The wheel zooms about the pointer; Shift+wheel pans sideways; drag with the middle button, or with Space held, to pan; { $p0 } fits the page. View ▸ Mouse wheel scrolls makes the wheel pan.
status-tip-zoom-scrolls = The wheel pans; Ctrl+wheel zooms about the pointer; drag with the middle button, or with Space held, to pan; { $p0 } fits the page.
bar-draw-plug = Draw plug
bar-plug-ports = Plug ports

# The tab group's menu of every tab.
dock-all-tabs = All tabs

# Doc 16a §5.7: the bottom dock's close button.
dock-close = Close the bottom dock
chips-label = Not on this page:
tab-title = { $p0 } { $p1 }
tab-close-name = Close { $p0 }
status-cursor-cell = { $p0 } · x { $p1 } y { $p2 } mm
status-hint-link = Click to go to { $p0 }

# The status bar's link hint, with the key of `nav.follow` from the keymap.
status-hint-follow = Click or { $p1 }: go to { $p0 }
status-hint-follow-none = Click or use Page ▸ Follow cross-reference: go to { $p0 }
status-hint-link-external = Link to another project

# The Preview tab, the renumber dialogs, the history panel and the purge step.
pane-preview = Preview
pane-history = History
status-put-away-undo = Undo put away what was on the pointer and undid nothing; Undo again takes back the last step.
status-put-away-redo = Redo put away what was on the pointer and redid nothing; Redo again does the next step again.

# The rails, Properties' Main and Advanced, the status bar (doc 16a §5.4, §5.8, §6.8).
rail-project = Project
rail-symbols = Symbol browser
rail-parts = Device browser
rail-pages = Pages
rail-devices = Devices
rail-views = Pages or devices
status-ready = Ready
# The state field after Help ▸ Tutorials without an installed guide.
status-guide-missing = The guide is not installed with this copy of the program
status-selected = { $p0 } selected
status-findings-errors = { $p0 } errors
status-findings-error = 1 error
status-findings-warnings = { $p0 } warnings
status-findings-warning = 1 warning
status-findings-infos = { $p0 } infos
status-findings-info = 1 info

# The pending findings, counted apart; they end when the tab leaves the page.
status-findings-pending = { $p0 } pending on this page
status-findings-pending-one = 1 pending on this page
status-renderer-why = The canvas draws on the CPU
status-bar-saved = Saved
status-bar-unsaved = Unsaved changes
status-bar-read-only = Read-only
splitter-name = Resize { $p0 } and { $p1 }
bar-rail = Rail
bar-duct = Duct
bar-auto-fill = Auto-fill rail
tab-title-list-page = { $p0 } · page { $p1 } of { $p2 }
bar-enclosure = Enclosure
language-name = English
pane-glossary = Glossary
status-driver-missing = Merge driver not configured
status-driver-tip = Set up merge driver: git then merges this project's files record by record
bar-cable = Cable
bar-report = Report

# A card that waits for Apply, in the status bar and on the Preview tab.
status-waiting = { $p0 ->
        [one] 1 change waits for Apply
       *[other] { $p0 } changes wait for Apply
    }
status-waiting-open = A preview is open
status-waiting-tip = Show the Preview, where Apply makes the changes
bar-new-station = New station
bar-rack = Rack
bar-attach-cards = Attach modules
bar-plc-export = Export to PLC
bar-plc-import = Import from PLC
bar-addresses = Addresses
bar-addresses-at-terminals = Addresses at terminals
bar-instrument-list = Instrument list
bar-bus-page = New bus page
bar-fluid-page = Fluid page
bar-fluid-symbol = Fluid symbol
bar-guide = Guide
commandbar-workspaces = Workspace
status-panel-front = The { $p0 } panel was open already: it is in front now.
bar-plc-module-replace = Replace
bar-plc-module-remove = Remove
bar-plc-module-draw = Draw on page
bar-plc-module-move = Move to page
bar-plc-module-goto = Go to page
bar-plc-module-place = Place here
bar-plc-module-left = Move left
bar-plc-module-right = Move right

# New project, the templates and the check level.
window-title-template = Template: { $p0 } - ElektraWatt
bar-templates = Templates
bar-save-as-template = Save as template
bar-check-level = Check level

# Text styles (Project ▸ Text styles…).
bar-text-styles = Text styles

# F1 opens the help for what is under the hand; the status bar says what it opens.
status-help = F1: { $p0 }

# A name with its shortcut, in a tooltip ("Back (Alt+Left)").
name-with-key = { $p0 } ({ $p1 })
# A control that cannot run, in its tooltip: its name, then why.
name-refused = { $p0 }: { $p1 }
# A name with how many it holds ("Tags (11)").
name-with-count = { $p0 } ({ $p1 })
# A disabled palette entry as a reader names it: its label, then why.
palette-entry-refused = { $p0 }. { $p1 }
# The accessible name of a command's chevron.
commandbar-variants-of = { $p0 } Variants
# The first-run step line: its number, how many there are, and its title.
firstrun-step = { $p0 } / { $p1 }: { $p2 }
# The accessible name of a navigator row's name field.
navigator-rename-row = Rename { $p0 }
# A job's heading: its kind and what it makes.
job-name = { $p0 } { $p1 }
# The keymap's "where" cell with what it notes ("Canvas · Changed").
keymap-where-note = { $p0 } · { $p1 }
# A refused key: the key, then the action it was for.
keymap-chord-action = { $p0 } ({ $p1 })
# The accessible name of an undone history entry.
history-entry-redo = { $p0 }, { $p1 }
# A changed record's line: its change word, its kind, its label and its changed fields in brackets.
preview-change-line = { $p0 } { $p1 } { $p2 }{ $p3 }
# A table cell's accessible name: its column and its text; with a manual address, that it is manual.
table-cell-name = { $p0 } { $p1 }
table-cell-manual = { $p0 }, { $p1 }
# An inline card's strip: its title and its summary.
card-strip = { $p0 } · { $p1 }
