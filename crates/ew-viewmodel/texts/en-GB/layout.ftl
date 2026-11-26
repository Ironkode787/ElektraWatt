# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of the window in en-GB: the cabinet layout: racks, arranging, enclosures and stations. One message per
# key; `{ $p0 }`, `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback of every
# other.

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
rack-max-modules = It takes at most { $p0 } modules.
rack-slot = Slot
rack-module = Module
rack-part = Part
rack-free = free
rack-blank = blanking plate
rack-empty = empty
rack-position = Position { $p0 }
rack-order-code = { $p0 }: { $p1 }
rack-bad-slot = "{ $p0 }" is no slot: type a number, or rack and slot as 1.3
rack-slot-taken = Slot { $p0 } holds { $p1 } already: choose a free slot
rack-slot-not-free = Slot { $p0 } is no free slot of the station: choose a free one
module-safety-title = Safety data
channel-settings-title = Mode and safety
layout-on-rail = { $p0 } sits on { $p1 } and mounts on a { $p2 } rail, so it moves along a rail only: drag it along { $p1 } or onto another rail.
layout-wrong-rail = { $p0 } mounts on a { $p2 } rail and does not fit { $p1 }: drop it on a { $p2 } rail.
layout-off-rail = { $p0 } mounts on a { $p1 } rail; drop it on a rail.
layout-wrong-turn = { $p0 } turns a placement on the plate; { $p1 } flips one on a rail.
layout-blocked = Blocked by { $p0 }
layout-rail-end = At the rail end
layout-live-rail = { $p0 }, { $p1 } at { $p2 } mm, { $p3 }
layout-live-plate = { $p0 }, plate at { $p1 }, { $p2 } mm
layout-findings = { $p0 ->
        [one] 1 finding
       *[other] { $p0 } findings
    }
layout-findings-none = no findings
layout-mode-free = Free
layout-mode-push = Push
layout-mode-swap = Swap
layout-grid-5 = 5 mm
layout-grid-1 = 1 mm
layout-options-mode = Placing mode
layout-options-grid = Grid step
enclosure-new-title = Create enclosure
enclosure-new-ok = Create
enclosure-new-location = New location
auto-fill-title = Auto-fill rail
auto-fill-ok = Fill
auto-fill-order-schematic = Schematic
auto-fill-order-dt = DT
auto-fill-devices-all = Ticked: the cabinet's devices and those whose part goes on a rail; sensors, cylinders, motors and valve terminals without a location are not. A ticked device without a location takes the cabinet's.
auto-fill-devices-marked = { $p0 ->
        [one] Ticked: the device marked in Unplaced devices
       *[other] Ticked: the { $p0 } devices marked in Unplaced devices
    }.
auto-fill-cause-all-placed = Every device of { $p0 } is placed already.
auto-fill-cause-none = No device has the location { $p0 } yet: give devices this location in Properties, under Structure.
layout-show-item-numbers = Show item numbers
enclosure-new-no-part = No enclosure part is in the project or an installed library; type a part number.
# The Create enclosure dialog draws an enclosure the location has, and off a panel layout page it makes one.
enclosure-draw-here = Draw { $p0 } of { $p1 } on this page
enclosure-draw-new-page = Draw { $p0 } of { $p1 } on a new panel layout page
enclosure-create-new-page = Create and draw on a new panel layout page
enclosure-create-only = Create only
enclosure-not-layout = This page is not a panel layout page. The enclosure is drawn on a new panel layout page after it; Create only makes the enclosure without drawing it.
enclosure-has-one = { $p1 } has the enclosure { $p0 } already: the button draws it.
enclosure-layout-title = Cabinet layout { $p0 }
auto-fill-devices = { $p0 ->
        [one] 1 device
       *[other] { $p0 } devices
    }
rack-new-title = New station
rack-create = Create
rack-kind = What it is
rack-kind-cpu = PLC (CPU)
rack-kind-remote = Remote I/O
rack-kind-valve-terminal = Valve terminal
rack-kind-compact = Compact device
rack-choose-head = Choose the station's head part
rack-new-part = New part…
rack-plc = The PLC it belongs to (a remote station: the PLC it serves)
rack-new-plc = New PLC
rack-gives = Gives
rack-addresses = Addresses
rack-new-module = new
rack-no-part-fits = No module part may sit in this slot
rack-addresses-moved = The modules' addresses moved with their slots.
rack-next-overview = Make the overview page
rack-next-bus = Put it on a bus page…
rack-next-fluid = Draw on a fluid page…
# Each side of a station is drawn on a page of its kind, from its own line under To do.
rack-next-electrical = Draw on an electrical page…
rack-draw-here = Draw on page { $p0 }
rack-put-here = Put it on page { $p0 }
rack-draw-new-electrical = Draw on a new electrical page
rack-put-new-bus = Put it on a new bus page
rack-draw-new-fluid = Draw on a new fluid page
rack-todo-electrical-side = The electrical side of { $p0 }, its supply and outputs as a box, is on no electrical page yet.
# A station's tag, or a card's, changed in the station's view.
rack-rename-station = Tag { $p0 }: click to change it
rack-rename-module = Change the tag of { $p0 }
rack-draft-tag = Tag
rack-draft-tag-field = Tag of the new station
rack-draft-tag-page = given by the page it is drawn on
rack-draft-tag-tip = Click to give it a tag of its own, which Create keeps frozen. Left empty, the page it is drawn on gives it its tag.
rack-tag-chip = Tag { $p0 }
rack-rename-tip = Click to change the tag (F2)
rack-rename-field = Tag of { $p0 }
rack-rename-plc = PLC { $p0 }: click to change its name
rack-rename-plc-tip = Click to change the PLC's name
rack-rename-plc-field = Name of PLC { $p0 }
rack-rename-plc-taken = { $p0 } is the name of another PLC of the project already: type another name, or Esc to keep this one.
rack-rename-plc-bad = { $p0 } cannot be a PLC's name: type letters, digits and - or _, without spaces.
rack-rename-taken = { $p0 } is the tag of another device already: type another tag, or Esc to keep this one.
rack-not-described = Its part does not describe its modules: add them as they are built.
choose-station-title = Which station takes { $p0 }?
choose-station-text = The module is drawn on a page without a station. Choose the station whose rack it goes into; Cancel leaves it waiting under Modules without a station.
choose-station-button = { $p0 }
rack-kind-cpu-tip = A PLC with its CPU, which leads the bus
rack-kind-remote-tip = Remote I/O station (interface module on a bus)
rack-kind-valve-terminal-tip = Valve terminal with a bus node
rack-kind-compact-tip = Compact device with channels of its own
rack-no-head-part = No part of this kind is in the libraries. New part… makes one.
rack-use-device = Use { $p0 }, which is on the bus
rack-bus-page-title = Bus
rack-no-bus-interface = { $p0 }'s part has no bus interface, so it has no place on a bus page: give it a part with a bus node
rack-bus-drawn = Every bus interface of { $p0 } is drawn on page { $p1 } already
rack-next-draw-on = Draw on page { $p0 } { $p1 }
rack-draw-new-page = New page…
rack-draw-page-title = PLC
rack-card-page-title = PLC { $p0 }
rack-page-choice = { $p0 } { $p1 }
put-station-title = Put a station on this page
put-station-here = Put { $p0 } here
put-station-new = A new station

# A station's view: its rack as a tab beside the page tabs (doc 10 §6).
station-tab-title = { $p0 } · rack
station-discard-title = Close the new station?
station-discard-text = The station is not made yet. Closing its tab loses the head part and the modules you chose.
station-discard-close = Close without making it
rack-move-to = To page { $p0 } { $p1 }
rack-move-to-new-page = To a new page
rack-valve-on-slot = A valve goes onto a valve position, not into a slot
rack-module-on-position = A module goes into a slot, not onto a valve position
rack-part-not-in-slot = Not allowed in slot { $p0 }: it goes into slot { $p1 }
rack-part-no-module = { $p0 } is a station's head, not a module: a slot takes a module
rack-no-free-position = Every valve position of { $p0 } is filled: empty one first
rack-full = Every slot of the station is taken: take a module out first
rack-takes-no-modules = This station takes no modules: its part has no slots for them
rack-part-no-functions = Not offered: nothing draws this part, as it has no pins. In the part editor, give it a Pin pattern or its supply pins under PLC, then save it
rack-todo = To do
rack-todo-none = Nothing to do: the slots meet the station's rules and every module is drawn.
rack-todo-create = Create makes the station with these modules. Nothing is written before.
rack-todo-show = Show
rack-todo-no-part = { $p0 } has no part.
rack-todo-undrawn = On no electrical page yet: { $p0 }. A tag gets its number when it is drawn on a page.
rack-todo-card-rest = The rest of { $p0 } is on no page yet: the module is wider than one page.
rack-card-rest = The rest of { $p0 } does not fit on this page: Rack… shows its line under To do, which draws it on a page you choose.
rack-card-no-room = Page { $p1 } has no room for { $p0 }: Rack… shows its line under To do, which draws it on another page.
rack-tag-unnumbered = { $p0 } gets its number when it is drawn on a page: its line under To do draws it.
rack-todo-bus = The bus side of { $p0 }, its bus ports, stands on no bus page yet.
rack-todo-fluid = The pneumatic side of { $p0 }, its valves on their positions, is on no fluid page yet.
rack-todo-valves = Valves of { $p0 } are not drawn in its positions, or its supply ports are missing.
rack-todo-overview = An overview page lists its positions and valves.
rack-group-cpu = CPUs
rack-group-interface = Interface modules
rack-group-valve-terminal = Valve terminals
rack-group-di = Digital inputs
rack-group-do = Digital outputs
rack-group-analogue = Analogue
rack-group-other = Other modules
rack-group-valve = Valves
rack-catalogue = Catalogue
rack-catalogue-filter = Filter the catalogue
rack-catalogue-hint = Double-click a part to put it into the selected slot, or drag it onto a slot.
rack-catalogue-heads = Choose the station's head: double-click a part.
rack-catalogue-empty = No part of the libraries fits here. New part… makes one.
rack-catalogue-no-hit = No part matches the filter.
rack-belongs-to = Belongs to PLC: { $p0 }
rack-drawn-on = Drawn on
rack-page = Page { $p0 }
rack-no-part = No part
rack-choose-part = Choose part…
rack-draw-own-pages = Draw each card on a page of its own
rack-draw-one-page = All on one page…
rack-draw-on-page = Draw on a page…
rack-head-row = Head
rack-word-cpu = CPU
rack-word-interface = IM
rack-word-valve-terminal = VT
rack-word-valve = Valve
rack-word-blank = Blank
rack-free-slot = Free slot { $p0 }
rack-free-position = Empty position { $p0 }
rack-place-slot = Slot { $p0 }, { $p1 }
rack-place-position = Position { $p0 }, { $p1 }
rack-place-head = Head, { $p0 }
rack-channels-of = Channels of { $p0 }
rack-channels-none = { $p0 } has no channels.
rack-channels-select = Select a module to see its channels.
rack-channels = Channels
rack-channel = Channel
rack-channel-address = Address
rack-channel-symbol = Symbol name
rack-channel-wired = Wired to
rack-channel-goto = Go to { $p0 }
rack-channel-function = Function text
rack-channel-symbol-of = Symbol name of { $p0 }
rack-channel-function-of = Function text of { $p0 }
rack-channel-edit-tip = Click to type it: Enter takes it, Tab goes on to the next channel, Esc leaves it.
rack-channel-function-tip = The function text of the device wired to it. Click to type it: Enter takes it, Tab goes on to the next channel, Esc leaves it.
rack-channel-bad = "{ $p0 }" cannot be taken here.
rack-put-blank = Blanking plate
rack-draw-on-new-page = Draw on a new page
arrange-lead = Arrange and route: { $p0 }. { $p1 }; { $p2 }. Nothing has changed yet.
arrange-symbols = { $p0 ->
        [one] 1 symbol moves
       *[other] { $p0 } symbols move
    }
arrange-across = { $p0 }, { $p1 } of them across the sheet
arrange-wires = { $p0 ->
        [one] 1 wire is routed
       *[other] { $p0 } wires are routed
    }
arrange-yours = { $p0 }, { $p1 } of them drawn by you
arrange-figures = { $p0 }, { $p1 }, { $p2 }, { $p3 }, { $p4 }
arrange-crossings = { $p0 ->
        [one] 1 crossing
       *[other] { $p0 } crossings
    }
arrange-bends = { $p0 ->
        [one] 1 bend
       *[other] { $p0 } bends
    }
arrange-straight = { $p0 ->
        [one] 1 wire straight
       *[other] { $p0 } wires straight
    }
arrange-through = { $p0 ->
        [one] 1 wire through a text
       *[other] { $p0 } wires through a text
    }
arrange-faults = { $p0 ->
        [one] 1 fault
       *[other] { $p0 } faults
    }
arrange-now = Now: { $p0 }
arrange-then = Then: { $p0 }
arrange-heading-parts = Symbols
arrange-heading-wires = Wires you drew yourself
arrange-heading-left = Left as it is
arrange-right = { $p0 ->
        [one] 1 step right
       *[other] { $p0 } steps right
    }
arrange-left = { $p0 ->
        [one] 1 step left
       *[other] { $p0 } steps left
    }
arrange-up = { $p0 ->
        [one] 1 step up
       *[other] { $p0 } steps up
    }
arrange-down = { $p0 ->
        [one] 1 step down
       *[other] { $p0 } steps down
    }
arrange-a-symbol = a symbol without a tag
arrange-unticked = stays where it is (you unticked it)
arrange-his = gets a new route
arrange-his-unticked = keeps its route (you unticked it)
arrange-kept = kept in place
arrange-place-up = above its pin
arrange-place-down = below its pin
arrange-place-left = left of its pin
arrange-place-right = right of its pin
arrange-in-way-room = no place { $p0 } has room for it; left where it stands
arrange-in-way-part = { $p1 } stands in its place { $p0 }; left where it stands
arrange-in-way-edge = its place { $p0 } lies outside the drawing area; left where it stands
arrange-in-way-text-of = in its place { $p0 } a wire would run through a text of { $p1 }; left where it stands
arrange-in-way-text = in its place { $p0 } a wire would run through a text; left where it stands
arrange-in-way-wire = in its place { $p0 } a wire would run through a connection point or along another wire; left where it stands
arrange-in-way-texts = in its place { $p0 } a text would lie on another text or a symbol; left where it stands
arrange-in-way-crossings = in its place { $p0 } the wires would cross more than they do now; left where it stands
arrange-in-way-no-gain = in its place { $p0 } the page would be no better; left where it stands
arrange-hint = Unticking a row plans the page again with that symbol where it stands, or that wire as it runs: what you see is what Apply does.
arrange-hold-b = Hold B: the page as it is now.
arrange-moved = { $p0 ->
        [one] 1 symbol moved
       *[other] { $p0 } symbols moved
    }
arrange-routed = { $p0 ->
        [one] 1 wire routed
       *[other] { $p0 } wires routed
    }
arrange-blocked = Nothing to arrange: { $p0 }
arrange-blocked-more = { $p0 ->
        [one] And 1 more: hover here, or see Notifications
       *[other] And { $p0 } more: hover here, or see Notifications
    }.
arrange-blocked-symbol = { $p0 }: { $p1 }.
arrange-blocked-unrouted = { $p0 } has no route: it is drawn as a dashed line.
arrange-held-order = The strip at { $p0 } stays as it stands: its terminals are wired to the card in another order than they stand in.
arrange-held-room = The strip at { $p0 } stays as it stands: its card's channels leave too little room for the terminals between them.
arrange-held-part = The strip at { $p0 } stays as it stands: { $p1 } is where it would go.
arrange-held-sheet = The strip at { $p0 } stays as it stands: opened, it would leave the drawing area.
arrange-held-worse = The strip at { $p0 } stays as it stands: opened in its order, its wires would cross more than now. Look at the terminals wired out of that order.
arrange-blocked-faults = { $p0 ->
        [one] 1 fault stays on the page; Check project names it
       *[other] { $p0 } faults stay on the page; Check project names them
    }.

# Surface… (doc 12 §3.8)
surface-chooser-title = Which surface?
surface-chooser-title-of = Which surface of { $p0 }?
surface-chooser-text = A surface is drawn on a page of its own, as it is seen from the side it is worked from.
surface-chooser-field = Surface
surface-chooser-new-page = Draw on a new page
surface-chooser-go = Go to page { $p0 }
surface-chooser-hinged = hinged { $p0 }
surface-chooser-page = page { $p0 }
surface-chooser-new = new page
surface-chooser-line = It is drawn on a new page after the cabinet's plate page, at the scale that fits, with its legend page. One undo step.
surface-chooser-shown = Page { $p0 } shows it.
toast-surface-drawn = { $p0 } is drawn on page { $p1 }, with its legend page.
canvas-surface-entry = Surface: { $p0 }
reason-surface-no-plate-page = The cabinet's mounting plate is on no page yet: draw it first with Enclosure….
error-no-surface = No surface of the cabinet is called “{ $p0 }”.
layout-cutout = { $p0 } goes in a cut-out of a door or a wall: place it on the door's page.
layout-open-door = Open the door
reason-layout-cutout = The part goes in a cut-out of a door or a wall.
hud-layout-door = Door
hud-layout-cutout = { $p0 ->
        [one] 1 device goes in a cut-out
       *[other] { $p0 } devices go in a cut-out
    }
