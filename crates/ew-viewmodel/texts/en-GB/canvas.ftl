# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: the page canvas: its tools and their hints, the HUD, the picker, previews, wires,
# boxes, pages and macros. One message per key; `{ $p0 }`, `{ $p1 }`, ... are the arguments, always strings. The
# source language and the fallback of every other.

tool-place = Place
tool-exchange = Change symbol
hint-place = Click or { $p0 } to place; { $p1 }, { $p2 }, { $p3 } and { $p4 } turn it; { $p5 } ends
hint-choose-strip = Choose the terminal strip in the list; Esc cancels
hint-place-proposals = Click or { $p0 } to place and connect the proposed wires; Alt+Click or { $p1 } places without them; { $p2 } tries the next target; { $p3 } ends
hint-select-move = Release, click or { $p0 } to move; { $p1 } cancels
hint-box-pins-room = The box goes no smaller: its pins and their names need the room, and a step to spare at each corner. Release, click or { $p0 } to resize; { $p1 } cancels
hint-exchange = Click or { $p0 } to change the symbol; { $p1 } cancels
move-readout = dx { $p0 } dy { $p1 } mm
place-target-new = new { $p0 }
place-target-join = join { $p0 }
place-target-function = function
place-target-part = part { $p0 }
place-target-variant = { $p0 } · { $p1 }
place-target-terminal = terminal { $p1 } on { $p0 }
place-target-new-strip = the new strip (no tag yet)
place-target-strip-new = { $p0 } (new)
place-variant = Variant { $p0 } · { $p1 }
place-target-box = box
picker-title = Insert symbol
picker-title-device = Insert device
# The line under each browser's title: what it lists, and where the other thing is.
picker-head-symbol = A symbol is a drawing without a part. To place a part with its functions, use Device.
picker-head-device = A device is a part of the catalogue with its functions. To draw without a part, use Symbol.
picker-title-exchange = Change symbol
picker-filter-hint = Search names, part numbers and keywords
picker-no-picture = No symbol yet: choose one to place this part
picker-badge-embedded = embedded
picker-badge-starter = starter
picker-badge-library = library
picker-part-function = { $p0 } of { $p1 } { $p2 }
picker-main = main
picker-variant = Variant
picker-comes-with = Comes with
picker-accessory = Accessory
picker-comes-with-line = { $p0 } { $p1 }, { $p2 } { $p3 }
picker-places = Places the { $p0 }
picker-drawn-as-box = Drawn as a device box
picker-holds = { $p0 } functions
picker-empty = Nothing matches the filter
picker-empty-parts = No parts in this project yet
picker-empty-parts-filter = No part matches the filter. To give a placed symbol its part, select it and use Edit ▸ Choose part…
picker-part-symbol = Choose a symbol…
picker-title-part-symbol = Choose a symbol for { $p0 }
# The line under the title of the symbol chooser of a part without a symbol.
picker-part-symbol-note = This part has no symbol of its own: choose the symbol to place it with.
picker-empty-black-box = No black-box symbols or parts to place
picker-empty-page = Nothing can be placed on this page
picker-group-project = In this project
picker-group-library = Library
# F28: the device browser's trades.
picker-trade-electrical = Electrical
picker-trade-plc-bus = PLC and bus
picker-trade-pneumatics = Pneumatics
picker-trade-enclosure = Enclosure and mounting
picker-group-libraries = From libraries
picker-cps = Connection points: { $p0 }
chooser-join-question = Join device
chooser-new-device = New device
chooser-kind-slot = slot { $p0 }
chooser-kind-auxiliary = auxiliary
chooser-kind-level = next level
hud-keys-place-join = { $p0 } rotates · { $p1 } variant · { $p2 } Join device… · { $p3 } done
tool-strip-new = New strip

picker-title-strip = Choose a terminal strip
picker-title-black-box = Insert black box
picker-strip-next = next terminal { $p0 }
hint-select-wire = Drag a round handle to move a segment or a bend of the wire; { $p0 } chooses a handle for the arrow keys
hint-select-handle-up-down = Handle { $p0 } of { $p1 }, a horizontal segment: ↑ and ↓ move it one grid step; { $p2 } goes to the next handle, { $p3 } back to the wire
hint-select-handle-sideways = Handle { $p0 } of { $p1 }, a vertical segment: ← and → move it one grid step; { $p2 } goes to the next handle, { $p3 } back to the wire
hint-select-handle-bend = Handle { $p0 } of { $p1 }, a bend: the arrow keys move it one grid step; { $p2 } goes to the next handle, { $p3 } back to the wire
tool-wire = Wire
hint-wire-armed = Click or { $p0 } on a connection point, a junction or a wire to start, then on free points to lead the wire through them
hint-wire-routing = Click a connection point to end, a free point for a bend; { $p0 } removes the last bend; { $p1 } cancels
hint-wire-direct = No clear route: the wire stays a dashed line without a route. Click free points to lead it, or route it later with Route.
hint-wire-full-junction = This junction already joins three wires: end on a wire to make a new junction.
# After a click that placed a bend instead of ending the wire.
hint-wire-bend = Bend placed: click a connection point to end the wire.
# A click that took nothing, right beside a connection point.
hint-wire-bend-beside = The click placed a bend beside a connection point, not on it: click the point itself to end the wire there; { $p0 } removes the bend.
hint-wire-start-beside = The click was beside a connection point, not on it: a wire starts on the point itself; click on its ring.
hint-wire-ended-beside = The wire just drawn bends beside a connection point without joining it; { $p0 } takes the wire back.
hint-wire-click-tee = A click here joins the line under the pointer with a bevel; { $p0 } makes the joint a dot; Alt+Click places a bend instead.
hint-wire-click-dot = A click here joins the line under the pointer with a dot; { $p0 } makes the joint a bevel; Alt+Click places a bend instead.
hint-wire-click-start = A click here is the start point: click further from it to place a bend.
hint-wire-over-text = The route crosses a text: keeping clear of it is a far longer way.
hint-wire-round = Something stands on the straight line between these two points, so the wire goes round it. Click free points to lead it another way.
hint-tube-armed = Click or { $p0 } on a port, a junction or a tube to start, then on free points to lead the tube through them
hint-tube-routing = Click a port to end the tube, a free point for a bend; { $p0 } removes the last bend; { $p1 } cancels
hint-tube-bend = Bend placed: click a port to end the tube.
hint-tube-bend-beside = The click placed a bend beside a port, not on it: click the port itself to end the tube there; { $p0 } removes the bend.
hint-tube-start-beside = The click was beside a port, not on it: a tube starts on the port itself; click on its ring.
hint-tube-ended-beside = The tube just drawn bends beside a port without joining it; { $p0 } takes the tube back.
wire-readout = { $p0 } mm
hud-tube = { $p0 } line · tube { $p1 } mm
hud-tube-no-size = { $p0 } line · no default tube
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
ip-choice-source = Source
ip-choice-target = Target
hint-supply = Click or { $p0 } to place the supply marks; { $p1 } ends
tool-box_pin = Pin name
hint-box-pin-add = Type the pin's name; Enter adds it; { $p0 } adds it and the next pin of this side; Esc cancels
hint-box-pin-name = Type the pin's name; Enter takes it; Esc cancels
hint-box-pin-armed = Add pin: the pin goes on the box's edge nearest the pointer; click where it goes, then type its name; Esc cancels
label-box-pin-new = new pin
tool-ip = Interruption point
tool-black_box = Black box
hint-ip = Click or { $p0 } to place; { $p1 } changes the direction; type a name; { $p2 } ends
hint-ip-external = Click or { $p0 } to place the external connection point; type a name; { $p1 } ends

# The Black box tool takes two corners; the armed hint says so before the first click.
box-chip-armed = Black box: click two corners
hint-box-armed = Click the box's first corner, then its opposite corner; { $p0 } ends
hint-box-part-armed = This part has no symbol of its own and is drawn as a box: click its first corner, then its opposite corner; { $p0 } ends
hint-box-place = Click to place; { $p0 } ends
place-target-click = { $p0 }: click to place
hint-box-outline = Click or { $p0 } at the opposite corner; { $p1 } starts again
hint-box-dropped = Click or { $p0 } at the first corner; { $p1 } again leaves the tool
tool-text = Text
tool-text_edit = Edit text
tool-graphic = Graphic
hint-text-armed = Click or { $p0 } where the text goes; { $p1 } ends
hint-text-typing = Type the text; Enter places it; Esc cancels
hint-text-edit = Type the new text; Enter changes it; Esc cancels
hint-graphic-armed = Click or { $p0 } at the first point; { $p1 } ends
hint-graphic-drawing = Click or { $p0 } at the next point; { $p1 } or a double click finishes a polyline or a polygon; { $p2 } removes the last point
chooser-accessory-part = Choose the part
tool-image = Image
hint-image-picking = Choose an image asset of the project first
hint-image-armed = Click or { $p0 } at the first corner; { $p1 } ends
hint-image-placing = Click or { $p0 } at the opposite corner; { $p1 } or { $p2 } drops the first corner
marker-mode-per_wire = As set per wire
marker-mode-all = All wires
marker-mode-deviations = Deviations
marker-mode-explicit = Explicit overrides
marker-mode-none = None
box-style-dotted = Dotted
box-style-dash_dot = Dash-dot
tool-marker = Wire annotation
tool-structure_box = Structure box
marker-crossed = { $p0 } crossed
hint-marker-armed = Press and drag (or { $p0 }, move, { $p0 }) across the wires to mark; { $p1 } ends
hint-marker-crossing = Release (or { $p0 }) to take the crossed wires
hint-marker-typing = Set the cross-section and colours if wanted; { $p0 } places the annotation; { $p1 } cancels
hint-structure-box-armed = Click or { $p0 } at the first corner; { $p1 } ends
hint-structure-box-outline = Click or { $p0 } at the next corner; { $p1 } or a double click closes the box (two corners: a rectangle)
hint-structure-box-typing = Set the style, caption, location and external point; { $p0 } places the box; { $p1 } cancels

tool-select = Select
tool-measure = Measure
hint-select = Click to select; drag for a rubber band
hint-select-cp = A wire starts at this connection point: { $p0 }, then a click on it
hint-measure-armed = Click or { $p0 } at the first point
hint-measure-measuring = Click or { $p0 } at the second point; { $p1 } starts again
measure-label = dx { $p0 } dy { $p1 } mm, { $p2 } mm
preview-chip = Preview of { $p0 }
preview-chip-stale = Preview of { $p0 } (stale)
preview-chip-failed = Preview of { $p0 } failed: { $p1 }
preview-chip-change = { $p0 } → { $p1 }
link-preview-title = { $p0 }
# The line under the link preview's title, with the key of `nav.follow`.
link-preview-go = Click the link or press { $p0 } to go there

# The link preview's accessible node.
link-preview-node = Link preview: page { $p0 }, cell { $p1 }
link-preview-node-page = Link preview: page { $p0 }

# The tooltip of a page link under the pointer, with its target's cell or folio.
tip-link = Click to go to { $p0 }

# What the Pending chips and the Messages header's "pending n" count.
tip-pending = Findings on the page you are editing that wait until you leave it or run a check
canvas-cursor-live-cp = , connection point { $p0 }
marker-export-dxf = Exported DXF to { $p0 }
canvas-name = Page { $p0 }, { $p1 }
canvas-description = { $p0 }, zoom { $p1 }
canvas-stats-fps = { $p0 } fps
canvas-stats-frame = frame p99 { $p0 } ms
canvas-stats-phase = { $p0 } p99 { $p1 } ms
canvas-stats-tiles = tiles pending { $p0 }
canvas-stats-backend = backend { $p0 }
hud-name = Tool

# The pick stack in words.
hud-stack = { $p0 } objects overlap here · click again to select the next
hud-selected = { $p0 } selected · { $p2 } deletes · { $p1 } rotates
hud-terminal-sides = clamp { $p0 } is the internal side (the cabinet's wiring), clamp { $p1 } the field side
hud-wire-selected = { $p0 } selected · { $p1 } chooses a handle for the arrow keys · { $p2 } deletes
hud-keys-place = { $p0 } rotates · { $p1 } variant · { $p2 } done
hud-connect-on-move-on = { $p0 } Connect on move: on
hud-connect-on-move-off = { $p0 } Connect on move: off
hud-place-wires = connects { $p0 }; Alt+Click or { $p1 } places without

# FIXPT41: the IP tool's ghost on a pin.
hud-ip-connects = connects to { $p0 }

# The Supply tool's ghost: a mark on a pin joins it.
hud-supply-connects = { $p0 } connects to { $p1 }

# The proposal's base is taken; the HUD and the placement toast say the name it gets.
hud-ip-taken = { $p0 } is taken, so this point is { $p1 }.

# The IP tool's role in words, so "target" and "star source" explain themselves.
hud-ip-source = source: the potential leaves here for its targets; one per name
hud-ip-target = target: the potential arrives here from its source
hud-ip-undirected = undirected: pairs either way
hud-ip-star = star source: feeds every point of the name
# The other half of a name that has one, said before the click (the tool turned to Target without a word).
hud-ip-next-target = next: a target, where { $p0 } arrives (its source is drawn)
hud-ip-next-source = next: the source, where { $p0 } leaves (its targets wait for it)
hud-ip-next-target-lone = next: one more target of { $p0 }, which has no source yet (D makes this point its source)
# While the IP tool's name field holds the keys, D is a letter of the name: the HUD says how to give the keys back.
hud-ip-field-keys = Enter ends the name, then { $p0 }: { $p1 }

# W42b: the IP tool's ghost label of a starred point ("<name> <direction>" otherwise).
ghost-ip-star = { $p0 } star source
pages-title = Pages
page-rendering = Rendering page { $p0 }…
page-no-pages = The project has no pages.
goto-title = Go to page or cell
goto-hint = Folio or cell
canvas-cp-unused = Mark { $p0 } not used
canvas-cp-used = Mark { $p0 } used
# The canvas context menu's entries that fit the thing under the pointer.
canvas-make-cable = Make cable from the selection…
canvas-go-partner = Go to partner { $p0 }
canvas-go-partner-none = Go to partner
# A pin with no wire to select (it touches a terminal directly) is named on its device's menu.
canvas-define-potential-pin = Define potential at { $p0 }…
canvas-ip-direction = Direction: { $p0 }
canvas-add-terminal = Add terminal
canvas-edit-strip = Edit strip
canvas-add-contact = Add contact { $p0 }
canvas-add-contact-none = Add contact
canvas-go-coil = Go to coil { $p0 }
canvas-page-properties = Page properties
canvas-go-to = Go to { $p0 }
canvas-device-delete = Delete
canvas-go-to-none = Go to
tip-wire-unrouted = This wire has no route, so it is drawn as a thin dashed line: move what is in its way and choose Route.
retag-pair = { $p0 } is now { $p1 }
terminal-in-sentence = { $p0 } terminal { $p1 }

# FX38d: a pin whose designation the device shares, by its position in words ("-1A4 pin - (2nd)").
pin-in-sentence = { $p0 } pin { $p1 } ({ $p2 })
marker-export = Exported PDF to { $p0 }
marker-recovered = { $p0 ->
        [one] Recovered 1 change from the journal
       *[other] Recovered { $p0 } changes from the journal
    }
label-paste-outside = some of it outside the drawing area
label-paste-retag = { $p0 } becomes { $p1 }: a tag follows the page its device is drawn on
label-paste = Paste { $p0 }
label-paste-and = { $p0 } and { $p1 }
label-paste-symbols = { $p0 ->
        [one] 1 symbol
       *[other] { $p0 } symbols
    }
label-paste-wires = { $p0 ->
        [one] 1 wire
       *[other] { $p0 } wires
    }
label-paste-other = { $p0 ->
        [one] 1 other item
       *[other] { $p0 } other items
    }
label-paste-dropped = { $p0 ->
        [one] ; 1 wire left out: its other end was not copied
       *[other] ; { $p0 } wires left out: their other end was not copied
    }
tool-paste = Paste
hint-paste = Click or { $p0 } to paste; { $p1 } ends
overview-page-title = Valve terminal { $p0 }
canvas-solenoid-draw = Draw solenoid { $p0 } on an electrical page…
canvas-solenoid-draw-here = Draw solenoid { $p0 } on this page…
canvas-valve-go = Go to the valve { $p0 }
picker-blank = Blanking plate
picker-blank-detail = closes the position, no valve
canvas-plug-draw-here = Draw the plug of { $p0 } on this page…
draw-plug-of = Plug of { $p0 }
draw-page-title = Draw on an electrical page
draw-page-new-title = Solenoids
canvas-solenoid-go = Go to solenoid { $p0 } at { $p1 }
canvas-solenoid-output = Go to the output of solenoid { $p0 } ({ $p1 })
canvas-solenoid-pin = Go to solenoid { $p0 }: pin { $p1 } of the plug of { $p2 }
canvas-plug-draw = Draw the plug of { $p0 } on an electrical page…
canvas-position = Position { $p0 }: { $p1 }
canvas-position-empty = empty
canvas-position-blank = blanking plate
canvas-medium-none-go = No medium: go to the air supply { $p0 } and choose one there
canvas-plug-go = Go to the plug of { $p0 } at { $p1 }
canvas-place-port = Place port { $p0 }
canvas-plc-unlock-module-starts = Unlock module starts
canvas-plc-unlock-symbols = Unlock symbols
canvas-plc-unlock-plc-data = Unlock PLC data
canvas-line-medium = Medium: { $p0 }
canvas-valve-add = Add valve at position { $p0 }
canvas-cell-fill = Fill position { $p0 }…
canvas-cell-replace = Replace at position { $p0 }…
canvas-cell-empty = Empty position { $p0 }
canvas-cell-blank = Blanking plate at position { $p0 }
canvas-valve-used = Set used
canvas-valve-spare = Set spare
canvas-valve-blank = Blank position { $p0 }
canvas-port-plug = Plug port { $p0 }
canvas-line-kind = Line kind: { $p0 }
marker-export-table = Exported the table to { $p0 }
link-preview-go-none = Click the link or use Page ▸ Follow cross-reference to go there
hint-termination = on or off, as the termination switch stands; empty for a port without one
canvas-cursor-description = , cursor { $p0 } x { $p1 } y { $p2 } mm
canvas-cursor-description-no-cell = , cursor x { $p0 } y { $p1 } mm
canvas-cursor-name = Canvas cursor
canvas-cursor-live = { $p0 }, x { $p1 } y { $p2 } mm
canvas-cursor-live-link = , link to { $p0 }
page-grid-fit-title = Change the connection grid
page-grid-fit-ok = Change and fit
page-grid-fit-foot = Page { $p0 } holds { $p1 } items and { $p2 } wires. On the { $p3 } grid, { $p4 } of them move onto it and the wires are drawn anew; Undo takes it all back in one step.
# Draw ▸ Place ▸ Terminal…'s form.
terminal-form-title = Place terminals
terminal-levels-one = 1: a terminal of one level
terminal-levels-two = 2: a block of two levels, one terminal and one part with a number on each level
canvas-choose-part-strip = { $p0 ->
        [one] Choose part for 1 terminal without one…
       *[other] Choose part for { $p0 } terminals without one…
    }
node-candidate-under = { $p0 } (under { $p1 })
node-candidate-top = { $p0 } (top level)
preview-none = No preview is open
preview-header = Preview: { $p0 } · as of rev { $p1 }
preview-title = Preview: { $p0 }
preview-stale = stale
preview-records = { $p0 ->
        [one] 1 item
       *[other] { $p0 } items
    }
preview-parts-new = { $p0 ->
        [one] 1 new part
       *[other] { $p0 } new parts
    }
preview-parts-skipped = { $p0 ->
        [one] 1 already in the project
       *[other] { $p0 } already in the project
    }
preview-import-summary = { $p0 } · { $p1 }
preview-records-made-of = { $p0 }: { $p1 }
preview-move-lead = { $p0 ->
        [one] Moves 1 symbol to page { $p1 }
       *[other] Moves { $p0 } symbols to page { $p1 }
    }
preview-move-lead-new = { $p0 ->
        [one] Moves 1 symbol to the new page { $p1 }
       *[other] Moves { $p0 } symbols to the new page { $p1 }
    }
preview-move-stays = { $p0 ->
        [one] 1 selected text or box stays on page { $p1 }
       *[other] { $p0 } selected texts and boxes stay on page { $p1 }
    }
preview-tags-count = { $p0 ->
        [one] 1 tag
       *[other] { $p0 } tags
    }
preview-xrefs-count = { $p0 ->
        [one] 1 cross-reference text
       *[other] { $p0 } cross-reference texts
    }
preview-changes-count = { $p0 ->
        [one] 1 change
       *[other] { $p0 } changes
    }
preview-pages-count = { $p0 ->
        [one] 1 page
       *[other] { $p0 } pages
    }
preview-entities = Entities
preview-notices = Changes
preview-tags = Tags
preview-xrefs = Cross-references
preview-wires = Wire numbers
preview-pages = Pages
preview-open-chip = Preview open: { $p0 }
preview-show = Show
preview-apply = Apply
preview-cancel = Cancel
canvas-scroll-horizontal = Scroll the page sideways
canvas-scroll-vertical = Scroll the page up and down
marker-saved = Saved
renumber-pages-title = Renumber pages
renumber-devices-title = Renumber devices
# Copy pages… (doc 06 §3.6).
page-copy-title = Copy pages
page-copy-lead = { $p0 ->
        [one] Copy page { $p1 }
       *[other] Copy { $p0 } pages: { $p1 }
    }
page-copy-drawn-elsewhere = Also drawn on pages that are not copied, so their copies draw only what stands on these pages: { $p0 }
page-copy-frozen-cleared = Frozen tags the rename leaves as they are; the copies take the scheme's tag on their new page: { $p0 }
page-copy-continued = Interruption points whose copies continue the points outside the copy: { $p0 }
page-copy-texts = { $p0 ->
        [one] 1 text that names a copied device or interruption point takes the copy's name
       *[other] { $p0 } texts that name a copied device or interruption point take the copy's name
    }
page-copy-texts-left = Texts left as they are, because copied devices shared the tag they name: { $p0 }
page-copy-titles = Page titles that change: { $p0 }
page-copy-bus = Bus devices whose copies need a bus line and an address of their own: { $p0 }
page-copy-title-renamed = { $p0 } → { $p1 }
page-copy-refused = Copy pages: { $p0 }
tool-macro = Macro
hint-macro = Click or { $p0 } to insert the macro; { $p1 } ends
label-macro = Insert { $p0 }
# The words of a text-slots value in the macro value table ("slot 2: at 10; -5 mm, turned 90°,
# hidden; slot 3: shown"), each a word the cell reads back.
macro-slots-default = as the symbol
macro-slots-slot = slot
macro-slots-at = at
macro-slots-turned = turned
macro-slots-hidden = hidden
macro-slots-shown = shown
macro-conflict-tag = The tag { $p0 } is printed by another device
macro-conflict-pairs = { $p0 ->
        [one] { $p1 } pairs with an existing interruption point
       *[other] { $p1 } pairs with { $p0 } existing interruption points
    }
macro-conflict-address = The PLC address { $p0 } is taken
macro-conflict-process-tag = The process tag { $p0 } is in use
macro-conflicts = Conflicts
macro-no-conflicts = No conflicts

# FX38d: a failed macro card leads with what blocks the apply in words; the rule code follows in secondary text.
macro-failed = { $p0 ->
        [one] The macro clashes with this project and cannot be inserted here (1 conflict)
       *[other] The macro clashes with this project and cannot be inserted here ({ $p0 } conflicts)
    }
macro-failed-other = The macro cannot be inserted here: { $p0 }
macro-value-set = Value set
macro-value-set-defaults = Defaults
macro-no-preview = No preview
macro-preview-name = Preview of { $p0 }
macro-devices = Devices
macro-variables = Variables
macro-version = Version { $p0 } · { $p1 }

# Cycle 39 (FIX39b-1): the HUD's short prompt while the paste choice's cards are open.
hud-paste-versions-one = Another version of { $p0 } is in the project; choose in the card below
hud-paste-versions-many = Other versions of { $p0 } are in the project; choose in the card below
hud-paste-update = Apply the update in the card below; the paste follows
preview-field-content = content
preview-change-added = Added
preview-change-changed = Changed
preview-change-removed = Removed

# The change size of a paste choice's conflict row in words (the describe line keeps patch, minor, major).
paste-class-patch = a data change
paste-class-minor = a compatible change
paste-class-major = a breaking change
paste-adapt = Use the project's copy
paste-update = Update the project's copy

# The lane's own additions beside §9.5 (proposed): a conflict row of the card, the action's label and the
# refusal without a choice card.
paste-conflict = { $p0 }: this project { $p1 }, pasted { $p2 }, { $p3 }
macro-values = Values
macro-value-label = { $p0 } ({ $p1 })
# A macro variable's line with its default.
macro-variable-default = { $p0 } = { $p1 }
# A macro variable's type in words: the words the macro insert's refusals use for it.
var-type-text = text
var-type-ident = identifier
var-type-number = number
var-type-bool = true or false
var-type-state = state
var-type-part = part reference
var-type-process-tag = process tag
var-type-channel-binding = channel number
var-type-address = address
var-type-tag-counter = tag counter
var-type-device-ref = device
macro-failed-value = The macro cannot be inserted with the value typed for { $p0 }.
tool-layout_place = Place device
hint-layout-place = Click or release to place; { $p0 } ends
hint-layout-idle = Drag a device from Unplaced devices onto the page
hud-layout-place = Place { $p0 } · Ctrl pushes · Alt swaps · Shift no snap · { $p1 } done

# Each key only where the selected placement takes it.
hud-layout-selected = { $p0 } selected · arrows nudge · { $p1 } deletes
hud-layout-selected-plate = { $p0 } selected · arrows nudge · { $p1 } rotates · { $p2 } deletes
hud-layout-selected-rail = { $p0 } selected · arrows nudge · { $p1 } flips · { $p2 } deletes
hud-layout-place-next = Next: drag a device from Unplaced devices onto the plate, or fill a rail at once
hud-layout-route-next = Next: Route gives each wire its way through the ducts and its length
hud-layout-ways-stale = { $p0 ->
        [one] The way of 1 wire no longer holds: Route gives it a new one
       *[other] The ways of { $p0 } wires no longer hold: Route gives them new ones
    }
hud-list-page = Page { $p0 } of { $p1 } of this list
hud-list-next = Next page { $p0 }
hud-list-previous = Previous page { $p0 }
hud-layout-locate-first = Next: give the cabinet's devices the location { $p0 }, then drag them from Unplaced devices onto the plate
hud-layout-give-location = Give location { $p0 }…
locate-title = Give the cabinet's devices their location
locate-ok = Give location
locate-row-prefixed = { $p0 }: its tag will print { $p1 } in front, as its page draws another location
hud-layout-start = Start with an enclosure: Enclosure on the Draw tab puts it on this page with its mounting plate
tool-rail_draw = Rail
tool-duct_draw = Duct
hint-rail-draw = Click the rail's start, then its end; { $p0 } cancels
hint-rail-draw-end = Click the rail's end; { $p0 } drops the start; { $p1 } cancels
hint-duct-draw = Click the duct's first point, then each next point; a double click or { $p0 } finishes; { $p1 } cancels
hint-duct-draw-next = Click the next point; a double click or { $p0 } finishes; { $p1 } removes the last point
draw-length = { $p0 } mm
draw-duct-size = { $p0 } × { $p1 } mm
draw-options-part = Part
draw-options-width = Width (mm)
draw-options-height = Height (mm)
draw-options-no-part = No part
canvas-labels-tags = Labels: Tags
canvas-labels-item-numbers = Labels: Item numbers
canvas-labels-both = Labels: Both
canvas-labels-tags-or-numbers = Labels: Tags or numbers
devices-location-title = Location of the devices
devices-location-ok = Set location
# Ruling 7: a terminal strip without a tag yet, and the Terminal strips table of a project with strips but no terminal.
strip-no-tag-yet = New terminal strip · no tag yet
tip-line-on-cp = Lines do not connect. Draw a wire instead.
tip-derived-tag = Tags are derived from the structure and the counter.
tip-generated-page = This page is generated from a report block.
tip-wheel-zooms = The mouse wheel zooms the page. It can scroll the page instead, Ctrl with the wheel then zooms (View ▸ Mouse wheel scrolls).
cable-new-title = New cable
connect-cores-title = Connect cores
cable-wires-count = { $p0 ->
        [one] 1 wire
       *[other] { $p0 } wires
    }
bus-cable-lines-count = { $p0 ->
        [one] the bus line
       *[other] { $p0 } bus lines
    }
picker-symbol-other-grid = This symbol is drawn on the { $p0 } connection grid, and this page uses { $p1 }. Place puts it here with its connection points on this page's grid.
route-folio = folio { $p0 }
route-preview-lead = Route: { $p0 }. { $p1 } wires get a new route, { $p2 } of them drawn by you and broken. Nothing has changed yet.
route-reason-wires = No way for { $p0 }: other wires are in its way.
route-reason-symbols = No way for { $p0 }: symbols are in its way.
route-reason-points = No way for { $p0 }: connection points of other wires are in its way.
route-reason-four-way = No way for { $p0 }: it would be a fourth arm at a junction.
route-his = You drew this wire and it is broken; it gets a new route: { $p0 }.
route-against-order = The wire { $p0 } and the others of its potential are wired in another order than their symbols stand: wire them in the order of the symbols for a straight rail.
route-selection-hint = Select nothing to route the whole page.
route-kept = { $p0 ->
        [one] 1 wire keeps the route it has.
       *[other] { $p0 } wires keep the routes they have.
    }
route-preview-lead-his = { $p0 ->
        [one] Route: { $p1 }. 1 wire you drew gets a new route. Nothing has changed yet
       *[other] Route: { $p1 }. { $p0 } wires you drew get a new route. Nothing has changed yet
    }.
route-more = { $p0 } more
drawing-title = Import { $p0 }
drawing-dxf-text = { $p0 } is a DXF drawing. Add it as a new graphic page, or place it under the shown page as an underlay to redraw over.
drawing-dwg-text = { $p0 } is a DWG drawing: it is read through the ODA File Converter, then as a DXF.
drawing-pdf-text = { $p0 } is a PDF. Its first page is placed under the shown page as an underlay to redraw over.
drawing-unitless = The drawing names no unit, so it is read in millimetres.
drawing-converter = ElektraWatt does not include the ODA File Converter. Install it from the Open Design Alliance; its program is chosen once, at the import.
drawing-underlay-text = An underlay is kept outside the project unless it is embedded. A customer's drawing may be confidential: embed it only when the project may carry it.
drawing-page = Add as page
drawing-underlay = Place under page { $p0 }
drawing-embed = Place and embed
canvas-plc-of = PLC: { $p0 }
canvas-plc-add-to = Add to { $p0 }
canvas-plc-add-new = Add to a new PLC
canvas-plc-add = Add to PLC
hint-date = A date, such as { $p0 }
hint-select-legend = The legend fits here: right-click and choose Add legend to list the items beside the layout
keep-in-place-untagged = a symbol without a tag
canvas-medium-none = No medium: choose one on the air supply
hud-bus-page-steps = 1 Put the stations here (right-click ▸ Put a station on this page…) · 2 Connect their ports with Wire · 3 Give their addresses in the Bus panel
box-side-top = Top
box-side-bottom = Bottom
box-side-left = Left
box-side-right = Right
box-side-invalid = Put the pin on the top, bottom, left or right edge.
box-new-part = New part
box-what-line = Device box · { $p0 }
box-line = A box with pins, as a device of this part is drawn
box-pins-from-channels = This part is a PLC card: its pins' names and what they carry come from its channels and supply pins, changed in the part editor under PLC. Where each pin stands on the box, and the box's size, are changed in its device box (Pins and box…).
box-pins-from-ports = This part is a bus device: its pins' names and what they carry are the part's, and its box on a bus page comes from its interfaces and ports, changed in the part editor under Bus interfaces. Where each pin stands on its own box, and that box's size, are changed in its device box (Pins and box…).
box-pins-from-symbol = This part is drawn with a symbol: its connection points are the symbol's. Change them in the symbol.
box-unsaved = This device box has changes that are not saved. Save it, or discard the changes.
box-pin-unknown = The part has no such pin.
box-pin-on-step = The pin stands at { $p0 } mm: a pin stands on a step of { $p1 } mm, clear of the corners, and { $p2 } mm is not one.
box-todo-number = Give the part a part number.
box-todo-manufacturer = Name the part's manufacturer.
box-identity-in-part-editor = The part's identity is changed in the part editor: Part data…
box-pin-name = Name
box-pin-side = Edge
box-pin-at = Along the edge (mm)
box-pin-at-help = The pin's distance from the box's top-left corner along its edge, in steps of the 4 mm connection grid.
box-pin-title = Pin { $p0 }
box-pin-line = A pin of the device: where a wire ends
box-pins-selected = { $p0 } pins selected
box-saved = Saved the device box of { $p0 }. Draw ▸ Device places it with one click.
box-saved-redraw = Saved the device box of { $p0 }. Boxes of it that are drawn the old way: { $p1 }.
box-redraw = Redraw them
box-redrawn = Boxes redrawn: { $p0 } as their part lays them out now, their wires with them.
box-test-armed = Click on the page to place a device of { $p0 }.
box-outline = Device box
box-pins = Pins
box-add-pin = Add pin
box-add-pin-how = Adds a pin on the bottom edge; drag it to its place, or onto another edge
box-add-bus = Add bus interface…
box-bus-library = A library's part keeps the bus interfaces its library gives it; make a part of your own from it to add one
box-pins-empty = No pins yet. Add pin puts the first one on the box.
box-canvas = Device box of { $p0 }
box-part-data = Part data…
box-part-data-how = The part editor: identity, technical data, size
box-select-how = Click a pin to select it; drag it along its edge or onto another one; drag the box's edges to size it; the arrow keys move a pin a grid step, Delete removes it
box-hint = Drag a pin along its edge or onto another one. Drag an edge of the box to size it.
box-close = Close box
box-ready = Ready to save
box-save-how = Saves the part with its pins and its box
box-test-how = Saves the box and arms Device with this part: a click on the page places it
box-copy-new = A copy of your own of { $p0 }: the original stays as it is. The copy keeps the part number { $p0 } until you give it one of its own in Part data…; Save keeps it in your own library.
box-copy-fork = { $p0 } cannot be changed: Save makes a copy of your own in this project, and the devices of this part take it.
terminal-spacing-one = 1 grid step · { $p0 } mm
terminal-spacing-steps = { $p0 } grid steps · { $p1 } mm
terminals-spacing-title = Terminal spacing
terminals-spacing-ok = Space
tool-frame = Edit frame
hint-frame = Click a cell, a line or the numbers · Double-click a cell to type · Esc ends
hint-frame-typing = Type the cell's text · Enter takes it · Esc keeps the cell as it was
frame-card = Page { $p0 } prints on { $p1 }. Every change shows on each page that prints them.
frame-card-project = the project's frame and title block
frame-card-own = a frame and title block of the project's own
frame-split-side = Split side by side
frame-split-above = Split one above the other
frame-join = Join
frame-picture = Picture…
frame-done = Done
frame-align-left = Left
frame-align-centre = Centre
frame-align-right = Right
frame-row-text = Text
frame-row-shows = Shows
frame-row-size = Text size
frame-row-align = Alignment
frame-row-typed-in = Typed in
frame-typed-in = Title block fields…, Project or Page properties
frame-row-size-mm = Width × height
frame-size-mm = { $p0 } × { $p1 } mm
frame-cell-empty = Empty cell
frame-cell-where = { $p0 ->
        [one] Title block cell · prints on { $p0 } page
       *[other] Title block cell · prints on { $p0 } pages
    }
frame-title = Frame
frame-pages = { $p0 ->
        [one] Prints on { $p0 } page
       *[other] Prints on { $p0 } pages
    }
frame-row-paper = Paper
frame-row-title-block = Title block
frame-block-shared = Shared with the other frames
frame-block-own = Its own
frame-row-rulers = Columns and rows
frame-axis-numbers = { $p0 } numbered from { $p1 }
frame-axis-letters = { $p0 } lettered from { $p1 }
shows-notice = Notice
shows-previous = Previous
shows-next = Next
shows-function = = Function
shows-location = + Location
shows-changed-in = Changed in revision
shows-page-count = Number of pages
shows-state = State
shows-name = Name
shows-group-project = Project
shows-group-page = Page
shows-group-revision = Revision
shows-group-people = People
shows-group-own = Fields of your own
shows-group-other = Other
shows-company-logo = Company logo
shows-picture = A picture…
shows-text-only = Text only
toast-frame-forked = The frame and title block are now this project's own; the program's stay as they were.
toast-frame-join-dropped = Join dropped the text { $p0 }.
frame-section-cell = Cell
frame-section-columns = Columns, along the top
frame-section-rows = Rows, along the left
frame-section-both = Both
frame-section-frame = Frame
frame-how-many = How many
frame-counted-with = Counted with
frame-numbers = Numbers
frame-letters = Letters
frame-first = First
frame-skip-io = Skip I and O
frame-drawn = Drawn
frame-yes = Yes
frame-no = No
frame-block-height = Title block height (mm)
frame-row-own-frame = This page
frame-own-frame-give = Give this page its own frame
frame-row-adopt = Templates
frame-template = Template

# Route on a panel page: the card of a cabinet's plan (doc 12 §12.5).
cabinet-route-lead = { $p0 ->
        [one] Route: cabinet { $p1 }. 1 wire gets a way through the ducts; { $p2 } are made on site. Nothing has changed yet.
       *[other] Route: cabinet { $p1 }. { $p0 } wires get a way through the ducts; { $p2 } are made on site. Nothing has changed yet.
    }
cabinet-route-lead-one-site = { $p0 ->
        [one] Route: cabinet { $p1 }. 1 wire gets a way through the ducts; 1 is made on site. Nothing has changed yet.
       *[other] Route: cabinet { $p1 }. { $p0 } wires get a way through the ducts; 1 is made on site. Nothing has changed yet.
    }
cabinet-route-ducts = Ducts
cabinet-route-lengths = Lengths
cabinet-route-typed = Typed by hand
cabinet-route-site-heading = Made on site
cabinet-route-without = Without a way
cabinet-route-duct-wires = { $p0 ->
        [one] 1 wire · fullest piece { $p1 }
       *[other] { $p0 } wires · fullest piece { $p1 }
    }
cabinet-route-duct-limit = limit { $p0 } %
way-chip = { $p0 } · { $p1 }
way-to-door = to { $p0 } in the door
cabinet-route-typed-row = keeps { $p0 }; Route gives { $p1 }
cabinet-route-to-door = { $p0 ->
        [one] 1 wire to the door
       *[other] { $p0 } wires to the door
    }
cabinet-route-chip = { $p0 } %
cabinet-route-chip-over = { $p0 } % · over { $p1 } %
cabinet-route-all = { $p0 ->
        [one] Route every wire again (1 more, whose way holds)
       *[other] Route every wire again ({ $p0 } more, whose ways hold)
    }
cabinet-route-all-on = Route every wire again (0 more: every wire is planned)
