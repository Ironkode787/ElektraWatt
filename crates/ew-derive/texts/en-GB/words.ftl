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
