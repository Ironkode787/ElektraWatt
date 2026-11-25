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

## A medium the derivation names

medium-vacuum = vacuum (from { $p0 })
medium-exhaust = exhaust

## A PLC channel, as the fluid findings name it

channel-of-module = channel { $p0 } of { $p1 }
channel-no-address = no address

## The category of an open item, as the list of open items prints it

open-item-tbd-value = TBD value
open-item-device-without-part = Device without part
open-item-unplaced-device = Unplaced device
open-item-unconnected-cp = Unconnected connection point
open-item-missing-length = Missing length
