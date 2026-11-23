# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The texts of `ew-commands` in en-GB: the words of the exchange with PLC software that a command puts into a text:
# the kinds of objects, the fields and the states of a row (named in `ew-plc`). One message per key; `{ $p0 }`,
# `{ $p1 }`, ... are the arguments, always strings. The source language and the fallback of every other.

plc-import-state-apply = applied
plc-import-state-rejected = rejected: the project owns it
plc-import-state-kept = kept: the project changed it
plc-import-state-pending = pending
plc-import-state-conflict = in conflict
plc-object-station = station
plc-object-module = module
plc-object-channel = channel
plc-object-tag = tag
plc-object-node = bus node
plc-object-instrument = instrument
plc-object-other = object
plc-field-comment-in = comment ({ $p0 })
plc-field-tag-name = symbol
plc-field-name = designation
plc-field-type = type
plc-field-station-type = type identifier
plc-field-location = location
plc-field-firmware = firmware
plc-field-data-type = data type
plc-field-address = address
plc-field-comment = comment
plc-field-start-input = input start
plc-field-start-output = output start
plc-field-bits-input = input length
plc-field-bits-output = output length
plc-field-position = slot
plc-field-order-number = order number
plc-field-type-name = type name
plc-field-io = direction
plc-field-number = number
plc-field-bits = length
plc-field-bus = bus
plc-field-mask = subnet mask
plc-field-router = gateway
plc-field-device-name = device name
plc-field-network = subnet
