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

## The word before a page's scale in its title block ("Scale 1:10")

title-scale = Scale

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
prop-via-core = cable { $p0 }, core { $p1 }
prop-via-potential = potential definition { $p0 }
prop-via-colour-profile = potential { $p0 } · colour profile '{ $p1 }'
prop-via-rating = potential { $p0 } · protected by { $p1 }, rated { $p2 } A · { $p3 }
prop-via-size-profile = potential { $p0 } · size profile '{ $p1 }' · class { $p2 }
prop-via-built-in = built-in default ({ $p0 })
prop-via-override = override

## Where a length comes from, as the wire list, the tube list and the cable overview print it

length-source-typed = typed
length-source-cable = cable
length-source-from-part = from part
length-source-missing = missing

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
