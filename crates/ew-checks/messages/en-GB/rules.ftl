# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The rule messages in en-GB (doc 15 §3.8; WP-S1.16 D6): the mandatory language and the fallback of every other.
# Every registered rule has `rule-<id lower case>` with `.title` (the catalogue title) and `.hint`. A rule whose
# findings carry arguments also has the message's value, which renders the finding's message byte for byte.

## EW-CORE: integrity, pages, cables, parts, interruption points, texts, the rule system

rule-ew-core-008 =
    .title = Dangling reference
    .hint = Restore the referenced record, or remove the reference with the quick fix.

rule-ew-core-021 =
    .title = Folio outside its section
    .hint = Renumber the page into its section's folio range, or move it to the section that owns the range.

rule-ew-core-022 =
    .title = Folio collision
    .hint = Give one of the pages another folio; two pages never share a folio.

rule-ew-core-023 =
    .title = Title-block override
    .hint = Remove the override to print the bound project value again, or keep it if the page needs its own text.

rule-ew-core-025 =
    .title = TOC fixpoint not reached
    .hint = Shorten the generated table of contents or its page entries so that its page count settles.

rule-ew-core-026 =
    .title = Report coverage gap
    .hint = Add a report block of that kind, or exclude the records from the report on purpose.

rule-ew-core-031 =
    .title = Invalid core colour
    .hint = Choose a colour code of the core colour standard, for example BN instead of BR.

rule-ew-core-032 =
    .title = Core differs from the part
    .hint = Use the core as the cable part defines it, or choose the part that has this core.

rule-ew-core-033 = { $cable } has no routed, manual or part length
    .title = Cable without length
    .hint = Enter a manual length, route the cable in the layout, or assign a part with a fixed length.

rule-ew-core-034 = the external connection { $connection } has no cable: { $count ->
        [one] wire { $wires } is
       *[many] wires { $wires } are
    } not a cable core; add the cable and assign its cores
    .title = External connection without cable
    .hint = Add the cable that leaves the location and assign its cores to the wires.

rule-ew-core-036 =
    .title = Core assigned twice
    .hint = Assign the core to one connection only, or use another core of the cable.

rule-ew-core-037 =
    .title = Shield not connected
    .hint = Connect the shield to an earthing point, or mark it as not connected on purpose.

rule-ew-core-041 = { $device } has no part; assign one, or mark it as supplied by others
    .title = Device without part
    .hint = Assign a part from the library, or mark the device as supplied by others.

rule-ew-core-042 =
    .title = Incomplete part data
    .hint = Complete the part's data in the library and update the embedded copy.

rule-ew-core-043 = placeholder part "{ $part }" (part number TBD) is used by { $users }; choose the part before release
    .title = Placeholder part
    .hint = Replace the placeholder with the real part before the release.

rule-ew-core-044 =
    .title = Open Tbd value
    .hint = Enter the value that is still marked TBD before the release.

rule-ew-core-045 =
    .title = Auxiliary contacts exceed the part
    .hint = Remove contacts from the drawing, or choose a part with enough main and auxiliary contacts.

rule-ew-core-047 =
    .title = Device not in the schematic
    .hint = Place a representation of the device in the schematic, or delete the device.

rule-ew-core-048 =
    .title = Order codes do not cover the children
    .hint = Add the missing children to an order code of the assembly, or remove them from the assembly.

rule-ew-core-051 =
    .title = Unpaired interruption point
    .hint = Place the matching interruption point with the same name, or remove this one.

rule-ew-core-052 =
    .title = Ambiguous interruption point
    .hint = Rename the interruption points so that each name pairs exactly one source with its targets.

rule-ew-core-053 =
    .title = Cross-reference cell invalid
    .hint = Restore the default anchor policy, or move the anchor into a cell that holds the graphics.

rule-ew-core-061 = text style { $style } prints with a cap height of { $height } mm, below the minimum of { $minimum } mm
    .title = Text below minimum height
    .hint = Enlarge the text style, or change the texts to a style that prints at least at the minimum height.

rule-ew-core-063 =
    .title = Unstyled text size
    .hint = Give the text one of the project's text styles instead of a free size.

rule-ew-core-065 =
    .title = Description lint
    .hint = Move ratings and technical data out of the description into their typed fields.

rule-ew-core-071 =
    .title = Graphic line on a CP
    .hint = Replace the graphic line by a connection, so that it takes part in the connectivity.

rule-ew-core-081 =
    .title = Waiver matches nothing
    .hint = Remove the waiver, or correct its target to the finding it was meant for.

rule-ew-core-083 =
    .title = Rule not evaluated
    .hint = Report the defect; the rule's findings in that scope are missing until it is fixed.

rule-ew-core-084 =
    .title = Rule pack not evaluated
    .hint = Restore the pack's data file or trust the project, so that the pack can run.

## EW-NUM: numbering

rule-ew-num-001 = { $frozen }: frozen, deviates from scheme (rule-derived value { $derived })
    .title = Tag deviates from the scheme
    .hint = Unfreeze the tag to take the derived value, or keep it frozen if the deviation is intended.

rule-ew-num-002 =
    .title = Frozen value deviates
    .hint = Unfreeze the value to take the derived one, or keep it frozen if the deviation is intended.

## EW-PLC: PLC addresses and bindings

rule-ew-plc-001 = { $form ->
        [alignment] { $channel }: the address { $address } does not start at a multiple of { $align } bytes, the profile's word alignment (an unverified convention)
       *[overlap] addresses overlap in configuration project { $cfg }: { $channels }{ $more ->
            [none] {""}
           *[other] ; and { $more } more channels
        }; change addresses until none overlap
    }
    .title = Address overlap
    .hint = Change the channels' addresses until none overlap, or let them take automatic addresses.

rule-ew-plc-002 = { $channel }: the address { $address } is outside the I/O range of { $module } ({ $range ->
        [none] no byte of area { $area }
       *[bytes] bytes { $first } to { $last } of area { $area }
    })
    .title = Address outside the module
    .hint = Give the channel an address inside its module's range, or correct the module's start.

rule-ew-plc-003 = { $subject }: { $what ->
        [input] the input start
        [output] the output start
       *[address] the manual address
    } names bit { $bit }, outside the { $width } bits of a unit
    .title = Bit index out of range
    .hint = Choose a bit inside the width of a unit.

rule-ew-plc-007 = the symbol { $symbol } is used by { $count } channels of configuration project { $cfg }: { $channels }; give each channel its own symbol
    .title = Duplicate symbol
    .hint = Give each channel its own symbol.

rule-ew-plc-008 = { $channel } is used but has no address: { $reason ->
        [station] its module belongs to no PLC station; place a CPU first or attach the module to a station
        [excluded] it is excluded from addressing; give it an address
       *[other] no address can be derived for it; check the module's start and its part's PLC section
    }
    .title = Used channel without address
    .hint = Give the channel an address, or mark it unused.

rule-ew-plc-012 = { $form ->
        [manual] { $channel }: the manual address { $address } differs from the derived address { $derived }; set the address to auto, or keep the deviation deliberately
       *[binding] { $channel } ({ $address }) drives solenoid { $solenoid } of { $valve }, which the position map of part { $part } assigns to { $expected } ({ $expected_address }); rebind it to the map's channel, or correct the part's PLC section
    }
    .title = Binding differs from the part map
    .hint = Rebind the channel the part's map assigns, or correct the part's PLC section.

rule-ew-plc-015 = { $subject }: the { $what ->
        [input] input start address
        [output] output start address
       *[address] channel address
    } is TBD; enter it before release
    .title = Tbd address
    .hint = Enter the address before release.

## EW-POT: potentials

rule-ew-pot-001 = { $conflicts }{ $more ->
        [none] {""}
       *[other] ; and { $more } more member CPs
    }{ $hint ->
        [bond] ; declare the bond with Draw ▸ Bond
       *[none] {""}
    }
    .title = Type or voltage conflict
    .hint = Separate the potentials, correct the definition, or declare the bond that joins them.

rule-ew-pot-002 =
    .title = PELV or SELV bonding
    .hint = Declare exactly one 0 V–PE bond for a PELV potential, and none for a SELV potential.

rule-ew-pot-003 = { $cp } has the role FE and is wired to the PE potential { $potential }; declare an FE–PE bond or connect it to functional earth
    .title = FE pin on PE
    .hint = Declare an FE–PE bond, or connect the pin to functional earth.

rule-ew-pot-004 =
    .title = GNYE on a non-PE potential
    .hint = Use green-yellow only for protective conductors; give the wire another colour.

rule-ew-pot-005 = wire { $wire } is BU on the potential { $potential }{ $type ->
        [none] {""}
       *[other] {" "}(type { $type })
    }; in a project with a neutral, BU marks the neutral conductor
    .title = BU on a non-N conductor
    .hint = Use another colour for the wire, or a cable whose blue core carries the neutral.

rule-ew-pot-006 = PE wire { $wire } has { $pe } mm², below the { $required } mm² required for the { $line } mm² line conductors at { $at }{ $verified ->
        [yes] {""}
       *[no] {" "}(the sizing table is not verified against the standard)
    }
    .title = PE undersized
    .hint = Increase the protective conductor's cross-section to the required size.

rule-ew-pot-008 =
    .title = PE island
    .hint = Connect the PE potentials of the location, for example through the rail bonding of the terminals.

rule-ew-pot-009 = { $cp } is not connected{ $unused ->
        [yes] {" "}(marked unused)
       *[no] ; connect it or mark it unused
    }
    .title = Unconnected CP
    .hint = Connect the connection point, or mark it unused.

rule-ew-pot-011 = on { $potential } it is unclear which wire feeds a junction or a group of interruption points; draw one wire into it from the supply side
    .title = Junction feed ambiguous
    .hint = Draw the junction so that one end feeds it, or set its wiring order.

rule-ew-pot-012 =
    .title = Ring in the drawing graph
    .hint = Remove the wire that closes the ring, or confirm that the ring is intended.

rule-ew-pot-013 = { $device } switches the neutral { $from } → { $to }{ $loads ->
        [one] ; load { $load } stays connected to { $line }
       *[none] {""}
    }.
    .title = N switching
    .hint = Move the contact into the line conductor, or use a device that opens L and N together.

rule-ew-pot-014 =
    .title = Physical-wire conflict
    .hint = Give the physical wire one set of properties, or split it into separate wires.

rule-ew-pot-015 =
    .title = Wire-number conflict
    .hint = Give the wires of one potential the same wire number, or separate the potentials.

rule-ew-pot-016 =
    .title = Redundant override
    .hint = Remove the override; it sets the value the wire takes anyway.

rule-ew-pot-017 =
    .title = Two potential names
    .hint = Keep one definition of the potential, or rename the interruption points to one name.

rule-ew-pot-021 = { $cp } is on { $potential }, which reaches no supply: the load is fed but not returned; connect it to the return conductor (0 V, N)
    .title = Load not returned
    .hint = Wire the load's return side to the return conductor of its supply (0 V, N), or link it to the module's return pin.

## EW-RDS (EW-IEC81346): reference designations

rule-ew-rds-001 =
    .title = Designation does not parse
    .hint = Correct the designation to the reference designation syntax of IEC 81346.

rule-ew-rds-002 =
    .title = Separator in a structure code
    .hint = Remove the separator from the code, or use a structure level instead.

rule-ew-rds-003 = { $device }: the letter code { $letter } does not match its function category { $category }; the identifier set { $set } suggests { $suggested }{ $verified ->
        [yes] {""}
       *[no] {" "}(the set is not verified against the standard)
    }
    .title = Letter code against function
    .hint = Use the suggested letter code, or keep the tag if the classification is intended.

rule-ew-rds-004 =
    .title = Duplicate designation
    .hint = Give one of the devices another designation.

rule-ew-rds-007 =
    .title = Reserved letters
    .hint = Avoid the letters I and O in a letter code; choose another letter.

rule-ew-rds-009 =
    .title = Confusable character
    .hint = Replace the character by its plain Latin form.

rule-ew-rds-010 =
    .title = Structure-code variants
    .hint = Merge the structure nodes whose codes differ only in spelling.

## EW-REL: the release gate

rule-ew-rel-001 =
    .title = Four eyes
    .hint = Set the author, then let another party approve the saved revision (ew revision approve); approve again after a content edit.

rule-ew-rel-002 =
    .title = Stale committed computation
    .hint = Recompute the stale result and commit it again before the release.

rule-ew-rel-004 =
    .title = Revision label order
    .hint = Give the revision a label in the project's label format that follows every earlier label.

## EW-TERM: terminals

rule-ew-term-001 =
    .title = Terminal without designation
    .hint = Give the terminal a designation.

rule-ew-term-002 =
    .title = Duplicate designation
    .hint = Renumber the strip, or give one of the terminals another designation.

rule-ew-term-003 =
    .title = Spare wired on the field side
    .hint = Remove the field-side wire, or make the terminal a used terminal.

rule-ew-term-004 =
    .title = Bridge group invalid
    .hint = Bridge only neighbouring terminals of one strip, or move the terminals together.

rule-ew-term-005 =
    .title = Rail bonding on a non-PE terminal
    .hint = Clear the rail bonding, or make the terminal a PE terminal.
