# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The words of the domain's kinds in en-GB (architecture §18): the checks, the lists, the commands and the session
# print them through `ew_derive::text`.

## A port's role, as the tube lists print it

port-role-supply = supply
port-role-working = working
port-role-exhaust = exhaust
port-role-pilot = pilot
port-role-pilot-exhaust = pilot exhaust
port-role-external-pilot = external pilot
port-role-consumer = consumer
port-role-sensing = sensing
port-role-other = port

## A line kind, as the tube lists print it

line-kind-working = working
line-kind-pilot = pilot
line-kind-exhaust = exhaust
line-kind-drain = drain
line-kind-flexible = flexible
line-kind-other = line

## A tube by its ends, and a junction as a tube's end

tube-ends = { $p0 } → { $p1 }
tube-end-junction = tee
junction-in = the junction in { $p0 }

## A medium the derivation names

medium-vacuum = vacuum (from { $p0 })
medium-exhaust = exhaust
medium-mixed = mixed

## A page's scale after its title in its title block ("Panel layout · Scale 1:10")

title-scale = Scale { $p0 }
title-scaled = { $p0 } · Scale { $p1 }

## A PLC channel, as the fluid findings name it

channel-of-module = channel { $p0 } of { $p1 }
channel-kind-di = DI
channel-kind-do = DO
channel-kind-ai = AI
channel-kind-ao = AO
channel-kind-counter = counter
channel-kind-io-link = IO-Link
channel-kind-supply = supply
channel-kind-multi-function = multi-function
channel-kind-other = other
channel-no-address = no address

## The category of an open item, as the list of open items prints it

open-item-tbd-value = TBD value
open-item-device-without-part = Device without part
open-item-unplaced-device = Unplaced device
open-item-unconnected-cp = Unconnected connection point
open-item-missing-length = Missing length

## A text style's name, as the Text styles page lists it and the symbol editor offers it

text-style-tag-main = Device tag
text-style-data-small = Small text
text-style-cp-small = Pin designation
text-style-pin = Pin text
text-style-xref-device = Cross-reference
text-style-marker = Wire marker
text-style-terminal = Terminal number
text-style-text-normal = Normal text
text-style-field-label = Field label
text-style-box-title = Box title
text-style-cover-table = Cover table
text-style-cover-title = Cover title
text-style-cover-subtitle = Cover subtitle
text-style-section-id = Section number
text-style-section-subtitle = Section subtitle
text-style-imported = Imported text of { $p0 } pt
text-style-imported-bold = Imported bold text of { $p0 } pt
text-style-own = A text style of this project

## A potential type and a circuit class, as the lists print them and the checks name them

potential-type-code = { $p0 }
potential-type-signal = signal
potential-type-undefined = undefined
potential-type-other = other
circuit-class-mains = mains
circuit-class-control = control
circuit-class-analogue = analogue
circuit-class-bus = bus
circuit-class-pe = PE
circuit-class-other = other

# Why a library's PLC profile is no address profile at hand (plc::unreadable_profile).
lib-profile-unreadable = the address profile { $p0 } cannot be read: { $p1 }

# Where a wire's property comes from: as the SQL view's provenance column words it (prop-source-*), and as the
# Properties grid's via chip words it (prop-via-*); a project default reads alike in both.
prop-source-override = override on the wire
prop-source-core = core { $p0 } of { $p1 }
prop-source-cable-core = cable core
prop-source-potential = potential definition
prop-source-colour-profile = colour profile { $p0 }
prop-source-rating = protected by { $p0 }, rated { $p1 } A · { $p2 }
prop-source-size-profile = size profile { $p0 }
prop-source-project = project default
prop-source-built-in = built-in default of derivation profile { $p0 }
prop-via-core = from cable { $p0 }, core { $p1 }
prop-via-potential = potential definition { $p0 }
prop-via-colour-profile = potential { $p0 } · colour profile '{ $p1 }'
prop-via-rating = potential { $p0 } · protected by { $p1 }, rated { $p2 } A · { $p3 }
prop-via-size-profile = potential { $p0 } · size profile '{ $p1 }' · class { $p2 }
prop-via-built-in = built-in default: the project has no profile that sets it
prop-via-override = override

## Where a length comes from, as the wire list, the tube list and the cable overview print it

length-source-typed = typed
length-source-cable = cable
length-source-from-part = from part
length-source-missing = missing
length-source-routed = routed
length-source-stale = stale
length-source-site = site

## A wire's way in its cabinet and its length (doc 12 §8, §9)

length-routed = Routed: { $p0 } of way, { $p1 } reserve at each end, rounded up to { $p2 }
length-routed-door = Routed: { $p0 } of way, { $p1 } reserve at each end and a { $p2 } door loop, rounded up to { $p3 }
length-stale = Stale: { $p0 }. Route gives it a new way
length-no-way-yet = No way yet: Route on page { $p0 } gives it one, or type its length here
length-no-way = No way: { $p0 }
length-typed-routed = Typed by hand. Route gives it { $p0 }: clear the length to take it.
length-typed-by-hand = typed by hand
length-with-cable = cut with its cable { $p0 }
length-none = no length: type it here, or the fitter measures the wire
way-moved = { $p0 } moved
way-end-moved = its end moved from { $p0 } to { $p1 }
way-unplaced = { $p0 } is no longer placed
way-duct-changed = { $p0 } changed
way-duct-gone = a duct of its way is gone
no-way-unplaced = { $p0 } is not placed: place it, then route again
no-way-no-duct = { $p0 } meets no duct: draw a duct above or below { $p1 }, or type the length
no-way-no-duct-here = { $p0 } meets no duct: draw a duct above or below its device, or type the length
no-way-off-plate = { $p0 } is on { $p1 }: Route lays wires on the plate and into the doors; type the length
no-way-other-plate = { $p0 } is on { $p1 }, not on the plate Route lays wires on; type the length
no-way-two-doors = { $p0 } and { $p1 } are in two doors: Route lays no wire from door to door; type the length
no-way-ducts-apart = the duct at { $p0 } and the one at { $p1 } do not meet: join them with a duct, or type the length
no-way-door-size = { $p0 } gives no size, so Route finds no hinge edge: type the length
no-way-open-end = its end at the interruption point { $p0 } is open: give the point its partner, or type the length
no-way-no-location = its end { $p0 } is on a device without a location: give the device its location, then route again
way-pin-moved = { $p0 } stands elsewhere on its device: its part or its drawing changed
way-duct-or-door = { $p0 } or { $p1 } changed
way-direct = direct
way-door = door
duct-inner-area-part = { $p0 } mm² · the part's inner size
duct-inner-area-share = { $p0 } mm² · 0.85 × { $p1 } × { $p2 }: the part gives no inner size
duct-fill = { $p0 } % where fullest · the limit is { $p1 } %
duct-fill-over = { $p0 } % where fullest · over the limit of { $p1 } %
duct-fill-default = with default diameters (unverified)
duct-fill-without = { $p0 } without a diameter not counted
duct-wires-select = Select them
way-show-on = Show on { $p0 }
way-line = { $p0 }.
way-line-direct = From { $p0 } straight to { $p1 } over the fronts.
way-line-from = From { $p0 }
way-line-into = into { $p0 }
way-line-along = along it
way-line-along-end = along it to its end
way-line-hinge = across the hinge onto the plate
way-line-hinge-to = across the hinge to { $p0 }
way-line-to = out to { $p0 }
way-route = Route
placement-ways-hold-one = 1 holds
placement-ways-hold = { $p0 } hold
placement-ways-stale-one = 1 no longer holds
placement-ways-stale = { $p0 } no longer hold
way-site-end-in = its end { $p0 } is in { $p1 }
way-site-end = its end { $p0 } is outside the cabinet
placement-reaches = { $p0 } mm from the plate
placement-reaches-room = { $p0 } mm from the plate; the closed door is { $p1 } mm away
placement-depth-unknown = depth not known: the closed door is not checked for it

## The rework list: what the fitter does, and what changed

rework-remove-device = Remove device
rework-remove-terminal = Remove terminal
rework-remove-cable = Remove cable
rework-pull = Pull
rework-replace = Replace
rework-move-end = Move end
rework-remake-end = Re-make end
rework-check-length = Check length
rework-remark = Re-mark
rework-relabel = Re-label
rework-mount-device = Mount device
rework-add-terminal = Add terminal
rework-add-cable = Add cable
rework-add = Add
rework-change-size = size { $p0 } → { $p1 }
rework-change-colour = colour { $p0 } → { $p1 }
rework-change-wire-part = wire part { $p0 } → { $p1 }
rework-change-end = end { $p0 } → { $p1 }
rework-change-remake = at { $p0 }: { $p1 } → { $p2 }
rework-change-remark = marker at { $p0 }: { $p1 } → { $p2 }
rework-change-relabel = { $p0 } → { $p1 }
rework-change-length = length { $p0 } → { $p1 }
rework-changes-between = { "; " }
rework-reason-partner-pulled = its twin ferrule's partner { $p0 } at { $p1 } is pulled
rework-reason-partner-resized = its twin ferrule's partner { $p0 } at { $p1 } changed size
rework-reason-partner-left = its twin ferrule's partner { $p0 } no longer lands on { $p1 }
rework-reason-joined = { $p0 } now lands on { $p1 } as well
rework-reason-site-apart-partner = its twin ferrule's partner { $p0 } at { $p1 } is made on site, this wire in the shop
rework-reason-site-apart-own = this wire is made on site, its twin ferrule's partner { $p0 } at { $p1 } in the shop
rework-reason-clamp-pulled = { $p0 }, which shared a ferrule at { $p1 }, is pulled
rework-reason-clamp-resized = { $p0 }, which shared a ferrule at { $p1 }, changed size
rework-reason-clamp-left = { $p0 }, which shared a ferrule at { $p1 }, no longer lands there
rework-reason-clamp-part = the part at { $p0 } says other things of its clamp
rework-reason-setting = the workshop's { $p0 } changed
workshop-setting-reserve = Reserve per end
workshop-setting-door-loop = Door loop
workshop-setting-cut-to = Cut to
workshop-setting-ferrules = Ferrules
workshop-setting-two-wires = Two wires in one clamp
workshop-setting-stock = Stock
rework-reason-part-shared = its wire part is set on a connection it shares with { $p0 }
rework-reason-part-potential = its potential { $p0 } sets its wire part
rework-reason-part-rules = the project's wiring rules give its wire part
rework-reason-part-cable = its cable { $p0 } gives its wire part
rework-reason-part-stock = the workshop's Stock gives its wire part by its size and colour
rework-reason-device-gone = { $p0 } is removed
rework-reason-size-shared = its size is set on a connection it shares with { $p0 }
rework-reason-colour-shared = its colour is set on a connection it shares with { $p0 }
rework-reason-size-potential = its potential { $p0 } sets its size
rework-reason-colour-potential = its potential { $p0 } sets its colour
rework-reason-size-rules = the project's wiring rules give its size
rework-reason-colour-rules = the project's wiring rules give its colour
rework-reason-size-cable = its cable { $p0 } gives its size
rework-reason-colour-cable = its cable { $p0 } gives its colour
rework-reason-way-gone = its way was laid for the wiring of the release: Route lays it again
rework-reason-laid = Route laid its way
rework-reason-rerouted = Route laid its way again
rework-reason-way-stale = its way no longer holds: { $p0 }

## An end's treatment, as the wire list's end cell prints it (doc 13 §4.4)

treatment-ferrule = ferrule
treatment-ferrule-size = ferrule { $p0 }
treatment-twin = twin { $p0 } · { $p1 }
treatment-twin-alone = twin { $p0 }
treatment-ring-lug = ring lug
treatment-ring-lug-stud = ring lug { $p0 }
treatment-bare = bare
treatment-connector = connector
treatment-unknown = ?

## The label sets (doc 13 §5.3)

label-set-wire-markers = Wire markers
label-set-terminal-markers = Terminal markers
label-set-strip-markers = Strip markers
label-set-cable-markers = Cable markers
label-set-device-labels = Device labels
label-set-tube-markers = Tube markers
label-set-plates = Plates

## A strip accessory's kind (07 §2.5): the strip editor's rows and the assembly list's Item

accessory-end-plate = End plate
accessory-end-clamp = End bracket
accessory-partition = Partition plate
accessory-marker = Marker carrier
accessory-cover = Cover
accessory-other = Accessory
accessory-end-plate-plural = End plates
accessory-end-clamp-plural = End brackets
accessory-partition-plural = Partition plates
accessory-marker-plural = Marker carriers
accessory-cover-plural = Covers
accessory-other-plural = Accessories
## The surfaces of a cabinet (doc 12 §2.8)

surface-plate = Mounting plate
surface-door = Door
surface-door-left = Left door
surface-door-right = Right door
surface-door-n = Door { $p0 }
surface-left = Left side
surface-right = Right side
surface-top = Top
surface-bottom = Bottom
surface-other = Surface
surface-of-plate = { $p0 } mounting plate
surface-of-door = { $p0 } door
surface-of-door-left = { $p0 } left door
surface-of-door-right = { $p0 } right door
surface-of-door-n = { $p0 } door { $p1 }
surface-of-left = { $p0 } left side
surface-of-right = { $p0 } right side
surface-of-top = { $p0 } top
surface-of-bottom = { $p0 } bottom
surface-of-other = { $p0 } surface
surface-seen-front = seen from the front
surface-seen-outside = seen from outside
surface-seen-left = seen from the left
surface-seen-right = seen from the right
surface-seen-above = seen from above; the door's edge at the bottom
surface-seen-below = seen from below; the door's edge at the top
surface-caption = { $p0 }, { $p1 }
corner-bottom-left = Bottom left
corner-bottom-right = Bottom right
corner-top-left = Top left
corner-top-right = Top right
hinge-left = Left
hinge-right = Right
hinge-word = Hinge
door-edge-word = Door's edge
template-file = { $p0 } { $p1 } template
template-edge-left = Left edge of the { $p0 }: lay this edge on it
template-edge-right = Right edge of the { $p0 }: lay this edge on it
template-edge-top = Top edge of the { $p0 }: lay this edge on it
template-edge-bottom = Bottom edge of the { $p0 }: lay this edge on it
template-edge-side-left = Left edge of the { $p0 }, at the cabinet's left as you face its door: lay this edge on it
template-edge-side-right = Right edge of the { $p0 }, at the cabinet's right as you face its door: lay this edge on it
template-edge-back = Back edge of the { $p0 }, away from the door: lay this edge on it
template-edge-door = Door's edge of the { $p0 }: lay this edge on it
template-band-what = { $p0 } · { $p1 } { $p2 } · { $p3 }
template-band-sheet = Sheet { $p0 } of { $p1 }, row { $p2 }, column { $p3 }: { $p4 }–{ $p5 } mm from the { $p6 }, { $p7 }–{ $p8 } mm from the { $p9 }
template-band-datum = Hole table: X and Y from the { $p0 } corner, marked 0
template-band-larger = Larger than a sheet, drawn across the sheets with its centre and corners crossed on each: { $p0 }
template-bar = 100 mm: print at actual size (100 %) and measure this bar
drill-datum = 0,0: { $p0 } corner of the { $p1 }, { $p2 }
word-edge-left = left edge
word-edge-right = right edge
word-edge-top = top edge
word-edge-bottom = bottom edge

## How a part mounts, and a carrier's label (doc 12 §2.5)

mount-screw = screw-mounted
mount-busbar-adapter = busbar adapter
mount-door-cutout = in a cut-out
mount-on-parent = mounted on its parent
mount-none = not mounted
carrier-rail = Rail { $p0 }
carrier-duct = Duct { $p0 }
carrier-rail-in-sentence = rail { $p0 }
carrier-duct-in-sentence = duct { $p0 }

## Why a hole's size does not read (doc 12 §2.5)

hole-error-no-number = Give the size in millimetres, as 4.5 or 22.
hole-error-not-a-screw = "{ $p0 }" is no screw size: type one of { $p1 }.
hole-error-not-a-cutout = "{ $p0 }" is no cut-out: type one of { $p1 } mm.
hole-error-not-a-gland = "{ $p0 }" is no gland thread: type one of { $p1 }.
hole-error-not-rect = "{ $p0 }" is no rectangle: type width × height, then R and the corner radius if it has one, as 92 × 92 R 2.
hole-error-not-positive = A size must be greater than 0 mm.

## How a hole is made and what it is (doc 13 §9.2)

made-tap = Ø { $p0 }, tap { $p1 }
hole-what-round = Hole Ø { $p0 }
hole-what-fixing = Fixing { $p0 }
hole-what-cutout = Cut-out { $p0 }
hole-what-gland = Gland { $p0 }
hole-what-rect = Rectangle { $p0 }
corner-in-sentence-bottom-left = bottom-left
corner-in-sentence-bottom-right = bottom-right
corner-in-sentence-top-left = top-left
corner-in-sentence-top-right = top-right
hole-table-heading = { $p0 }, page { $p1 }, { $p2 }, measured from the { $p3 } corner
hole-table-heading-sized = { $p0 } { $p1 } mm, page { $p2 }, { $p3 }, measured from the { $p4 } corner

## Where a device stands (doc 13 §6.1)

rail-start-left = left end
rail-start-right = right end
rail-start-bottom = lower end
rail-start-top = upper end
place-on-rail = { $p0 } · { $p1 }, page { $p2 } · { $p3 }–{ $p4 } mm from its { $p5 }
place-on-surface = { $p0 }, page { $p1 } · X { $p2 }, Y { $p3 } mm from its { $p4 } corner{ $p5 }
place-hole = , in hole { $p0 }
place-holes = , in holes { $p0 }

## A bus station's address as a drawing prints it, its address after the word
bus-node = Node { $p0 }
bus-node-tbd = Node TBD
