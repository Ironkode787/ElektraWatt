# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The rule messages in en-GB (doc 15 §3.8; WP-S1.16 D6): the mandatory language and the fallback of every other.
# Every registered rule has `rule-<id lower case>` with `.title` (the catalogue title) and `.hint`. A rule whose
# findings carry arguments also has the message's value, which renders the finding's message byte for byte. An
# integrity rule of opening a project (`INTEGRITY`) has its `.title` and `.hint`, since its findings reach the Messages
# pane.

## EW-CORE: integrity, pages, cables, parts, interruption points, texts, the rule system

rule-ew-core-001 =
    .title = Manifest damaged
    .hint = Save the project to write a new manifest, or take either side's version from git.

rule-ew-core-002 =
    .title = Mixed generation
    .hint = Restore the project's files from one save (git, the sync client or a backup), or replay a crashed session with ew recover.

rule-ew-core-003 =
    .title = Listed file missing
    .hint = Restore the file from git, the sync client or a backup, or let a cloud folder finish downloading and open the project again.

rule-ew-core-004 =
    .title = Orphan file
    .hint = Compare the file with the project, merge what you need as an external change, then delete the file.

rule-ew-core-005 =
    .title = JSON syntax error
    .hint = Repair the project with ew fmt --repair, or correct the line in a text editor.

rule-ew-core-006 =
    .title = Git conflict markers
    .hint = Resolve the conflict in git, or settle its conflict records with ew merge resolve.

rule-ew-core-007 =
    .title = Schema violation
    .hint = Correct the line in a text editor by the format's reference, or restore it from git.

rule-ew-core-008 = { $form ->
        [duplicate_number] { $module } holds a second channel numbered { $number }; the module keeps one channel per number, so this one prints nowhere
        [channel] channel { $channel } belongs to { $module }, which is not a PLC module; delete the channel or make { $module } a module again
       *[form] folio { $page }: { $names ->
            [both] the frame for page type { $type } names form copy { $type_copy }, and the default frame names form copy { $default_copy }
            [type] the frame for page type { $type } names form copy { $type_copy }
           *[default] the default frame names form copy { $default_copy }
        }, which the project does not hold; the page takes the next frame
    }
    .title = Dangling reference
    .hint = Restore the referenced record, or remove the reference with the quick fix.

rule-ew-core-009 =
    .title = Duplicate record ID
    .hint = Delete the extra line in a text editor, or give its record a new ID where both are wanted.

rule-ew-core-010 =
    .title = Record in the wrong file
    .hint = Save the project: the save moves the record to its file.

rule-ew-core-011 = { $form ->
        [file] { $file ->
            [page] the file of folio { $name }
            [layout] the layout file of { $name }
            [a-layout] a layout file
            [a-page] a page file
            [import] an import's file
           *[path] { $name }
        }: the merge could not read { $side ->
            [ours] ours
            [theirs] theirs
           *[other] one side
        }; its lines are held in a conflict record
       *[record] { $target ->
            [project] the project
            [device] device { $tag }
            [page] folio { $page }
            [a-page] a page
            [function] a function of device { $tag }
            [a-function] a function
            [connection-on] connection from { $from } to { $to } on folio { $page }
            [connection] connection from { $from } to { $to }
            [a-connection] a connection
           *[other] a { $kind }
        }: { $side ->
            [ours-deleted] ours deleted it and theirs changed { $members }
            [theirs-deleted] theirs deleted it and ours changed { $members }
           *[differ] ours and theirs differ in { $members }
        }
    }
    .title = Conflict records present
    .hint = Take ours or theirs for each conflict with ew merge resolve, or delete the record that conflicts.

rule-ew-core-012 =
    .title = Structural invariant violated
    .hint = Let the next change of the record restore the invariant, or restore the file from git.

rule-ew-core-014 =
    .title = Library copy modified
    .hint = Embed the copy again from the library, or keep the change on purpose.

rule-ew-core-015 =
    .title = Asset missing
    .hint = Import the asset again or restore the file from git, and delete the damaged file.

rule-ew-core-016 =
    .title = Data from a newer format
    .hint = Open the project in the newer ElektraWatt that wrote it, or upgrade ElektraWatt.

rule-ew-core-017 =
    .title = Malformed known variant
    .hint = Enter the value again in Properties, which shows the raw value.

rule-ew-core-018 =
    .title = Unknown derivation profile
    .hint = Open the project in a build that ships the profile.

rule-ew-core-019 =
    .title = Reader limit exceeded
    .hint = Split or repair the file outside ElektraWatt, and check where the project came from.

rule-ew-core-021 = folio { $page } lies outside the folio range { $from }–{ $to } of section { $section }; renumber the page or move it to the section whose range holds { $number }
    .title = Folio outside its section
    .hint = Renumber the page into its section's folio range, or move it to the section that owns the range.

rule-ew-core-022 = folio { $page } is printed by { $count } pages ({ $kinds }); give each page its own folio
    .title = Folio collision
    .hint = Give one of the pages another folio; two pages never share a folio.

rule-ew-core-023 = folio { $page } overrides the project description in its title block with "{ $text }"
    .title = Title-block override
    .hint = Remove the override to print the bound project value again, or keep it if the page needs its own text.

rule-ew-core-025 = the table of contents did not settle its page count; it uses { $pages } pages and its last page lists every remaining row
    .title = TOC fixpoint not reached
    .hint = Shorten the generated table of contents or its page entries so that its page count settles.

rule-ew-core-026 = { $family ->
        [terminal_diagram] strip { $device } is in no terminal diagram
        [cable_overview_diagrams] cable { $device } is in no cable overview and diagrams
       *[plc_overview] PLC station { $device } is in no PLC overview
    }: no report block of that kind covers it; add one or include it
    .title = Report coverage gap
    .hint = Add a report block of that kind, or exclude the records from the report on purpose.

rule-ew-core-031 = core { $core } has the colour { $colour }: { $codes } is not an IEC 60757 code or a company code of the colour profile
    .title = Invalid core colour
    .hint = Choose a colour code of the core colour standard, for example BN instead of BR.

rule-ew-core-032 = core { $core } { $form ->
        [foreign] is not a core of the cable part { $part }
       *[differs] differs from the core { $def } of the cable part { $part }: { $colour ->
            [yes] { $sep_colour }colour { $colour_core } ≠ { $colour_part }
           *[no] {""}
        }{ $csa ->
            [yes] { $sep_csa }cross-section { $csa_core } ≠ { $csa_part }
           *[no] {""}
        }{ $kind ->
            [yes] { $sep_kind }kind { $kind_core } ≠ { $kind_part }
           *[no] {""}
        }{ $pair ->
            [yes] { $sep_pair }pair { $pair_core } ≠ { $pair_part }
           *[no] {""}
        }
    }
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

rule-ew-core-036 = core { $core } is assigned to { $count } wires ({ $wires }); a core carries one wire
    .title = Core assigned twice
    .hint = Assign the core to one connection only, or use another core of the cable.

rule-ew-core-037 = shield { $core } is connected at neither end; connect it at one end or both
    .title = Shield not connected
    .hint = Connect the shield to an earthing point, or mark it as not connected on purpose.

rule-ew-core-041 = { $device } has no part; assign one, or mark it as supplied by others
    .title = Device without part
    .hint = Assign a part from the library, or mark the device as supplied by others.

rule-ew-core-042 = part { $part } of { $device } has no { $missing ->
        [description] description
        [manufacturer] manufacturer
       *[both] description, no manufacturer
    }
    .title = Incomplete part data
    .hint = Complete the part's data in the library and update the embedded copy.

rule-ew-core-043 = placeholder part "{ $part }" (part number TBD) is used by { $users }; choose the part before release
    .title = Placeholder part
    .hint = Replace the placeholder with the real part before the release.

rule-ew-core-044 = { $form ->
        [bus] { $end ->
            [connector] a connector
            [device] a device:{ $port }
            [ip] end { $port }
            [segment] a segment end
           *[named] { $port }
        }: the station address is TBD; enter it before release
        [start] { $module }: the { $which ->
            [input] input
           *[output] output
        } start address is TBD; enter it before release
       *[channel] { $module }: the address of channel { $number } is TBD; enter it before release
    }
    .title = Open Tbd value
    .hint = Enter the value that is still marked TBD before the release.

rule-ew-core-045 = { $device } uses { $need_no } NO, { $need_nc } NC, { $need_co } changeover contacts; its parts have { $have_no } NO, { $have_nc } NC, { $have_co } changeover
    .title = Auxiliary contacts exceed the part
    .hint = Remove contacts from the drawing, or choose a part with enough main and auxiliary contacts.

rule-ew-core-047 = { $device } has { $part ->
        [yes] a part but no representation on any page; the parts list still counts it
       *[no] no representation on any page
    }
    .title = Device not in the schematic
    .hint = Place a representation of the device in the schematic, or delete the device.

rule-ew-core-048 = { $device } is covered by { $form ->
        [covered-twice] { $count } order codes of { $assembly }; its parts would print { $count } times
       *[uncovered] no order code of { $assembly }; the assembly BOM lists its own parts beside the configured products
    }
    .title = Order codes do not cover the children
    .hint = Add the missing children to an order code of the assembly, or remove them from the assembly.

rule-ew-core-051 = interruption point { $name } { $form ->
        [no-cp] has no connection point; it pairs with nothing
       *[unpaired] has no partner; its wire stays open-ended
    }
    .title = Unpaired interruption point
    .hint = Place the matching interruption point with the same name, or remove this one.

rule-ew-core-052 = interruption point { $name }: { $form ->
        [flagged] { $count } points are flagged as star source; keep the flag on one of them
       *[sources] { $count } source points and no star source; set all but one to target, or flag one as the star source
    }
    .title = Ambiguous interruption point
    .hint = Keep one source per name: set the other sources to target, or flag exactly one point as the star source.

rule-ew-core-053 = { $item }: { $form ->
        [outside] outside the frame (the target's anchor has no cell)
       *[empty] empty cell (cell { $cell } holds no graphics of the target; its anchor policy places the anchor outside them)
    }
    .title = Cross-reference cell invalid
    .hint = Restore the default anchor policy, or move the anchor into a cell that holds the graphics.

rule-ew-core-061 = text style { $style } prints with a cap height of { $height } mm, below the minimum of { $minimum } mm
    .title = Text below minimum height
    .hint = Enlarge the text style, or change the texts to a style that prints at least at the minimum height.

rule-ew-core-063 = a text on folio { $page } has the size { $style }, which no text style binds; give it a text style
    .title = Unstyled text size
    .hint = Give the text one of the project's text styles instead of a free size.

rule-ew-core-065 = description "{ $text }" of part { $part }{ $form ->
        [rating] {" "}holds the rating { $matches }: move it to the technical data
       *[unit-format] : unit formatting { $matches } (a space between number and unit, units as written in SI)
    }
    .title = Description lint
    .hint = Move ratings and technical data out of the description into their typed fields.

rule-ew-core-067 = Wire { $wire } runs through { $point ->
        [junction] the junction in { $pin }
        [junction-off-grid] a junction
       *[cp] { $pin }
    }, which it does not join.
    .title = Wire through a connection point
    .hint = Route the wire again with Route selection, or change its route with Edit route so that it keeps clear of the point.

rule-ew-core-068 = Wire { $wire } runs along { $other } on folio { $page }.
    .title = Wire along another wire
    .hint = Route one of the wires again with Route selection, so that each runs on a line of its own.

rule-ew-core-069 = Wire { $wire } runs through { $symbol ->
        [ip] interruption point { $device }
       *[device] { $device }
    }.
    .title = Wire through a symbol
    .hint = Route the wire again with Route selection, or move the symbol out of its way.

rule-ew-core-070 = Wire { $wire } runs through { $owner ->
        [free-text] the free text "{ $text }"
        [structure-box] the caption "{ $text }" of a structure box
        [ip] the text "{ $text }" of interruption point { $device }
       *[symbol] the text "{ $text }" of { $device }
    }.
    .title = Wire through a text
    .hint = Route the wire again with Route selection, or move the text.

rule-ew-core-071 = a graphic line ends on { $cp }, which joins nothing electrically; draw a connection instead
    .title = Graphic line on a CP
    .hint = Replace the graphic line by a connection, so that it takes part in the connectivity.

rule-ew-core-072 = { $place ->
        [off-grid] A junction on folio { $page } joins four wires: draw two T junctions instead.
       *[cell] The junction at { $cell } joins four wires: draw two T junctions instead.
    }
    .title = Four-way junction
    .hint = Move one branch to a junction of its own a grid step away, so that each junction joins three wires.

rule-ew-core-073 = Wire { $wire } has no route: Route page draws it.
    .title = Unrouted wire
    .hint = Run Route page or Route selection, or draw the wire's route with Edit route.

rule-ew-core-081 = waiver { $waiver } for { $rule } { $form ->
        [not-evaluated] was not evaluated: the rule did not run in scheme { $scheme } ({ $mode } mode)
       *[unmatched] matches no finding; remove it or correct its target
    }
    .title = Waiver matches nothing
    .hint = Remove the waiver, or correct its target to the finding it was meant for.

rule-ew-core-083 = rule { $rule } { $form ->
        [prevent] could not validate the edit: it stopped with an internal error, so the edit is not committed; report the defect
       *[scope] could not be evaluated for { $scope }: it stopped with an internal error, so its findings there are missing; report the defect
    }
    .title = Rule not evaluated
    .hint = Report the defect; the rule's findings in that scope are missing until it is fixed.

rule-ew-core-084 = pack { $pack } was not evaluated in full: its data table { $file } does not load ({ $error }); { $rules } did not run
    .title = Rule pack not evaluated
    .hint = Restore the pack's data file or trust the project, so that the pack can run.

## EW-FL: fluid power (WP-S1.20 D16)

rule-ew-fl-001 = { $form ->
        [many] { $valve }: working ports { $ports } are open; connect, plug or blank them
       *[one] { $valve }: working port { $port } is open; connect, plug or blank it
    }
    .title = Working port open
    .hint = Draw the working line, or close the port with a plug, a silencer or a blanking.

rule-ew-fl-002 = { $form ->
        [excess] { $valve }: solenoid { $solenoid } is bound to { $count } channels: { $channels }
        [undeclared] { $valve }: { $channel } drives a coil that the part does not declare
       *[unbound] { $valve }: solenoid { $solenoid } has no channel
    }
    .title = Coil bindings against solenoids
    .hint = Bind each solenoid the valve part declares to one channel, and no channel to another coil.

rule-ew-fl-004 = { $form ->
        [blank] { $module }: position { $position } holds a blanking plate, { $device }
       *[valve] { $module }: position { $position } holds a spare valve, { $device }
    }
    .title = Spare positions
    .hint = Keep the spare valve or blanking plate deliberately, or put a used valve at the position.

rule-ew-fl-005 = the fluid net at { $at } carries { $media } directly; keep one medium per net
    .title = Two media in one net
    .hint = Remove one of the medium definitions, or separate the nets.

rule-ew-fl-006 = { $form ->
        [od] tube { $tube } is { $od } mm outside, but port { $port } takes { $port_od } mm
       *[rating] tube { $tube } is rated { $rating } bar, below the { $pressure } bar its net carries
    }
    .title = Tube does not fit
    .hint = Choose a tube that matches the port and the pressure of its medium.

rule-ew-fl-007 = tube { $tube } has no tube specification; give the medium or the project a default tube, or set it on the line
    .title = Line without tube specification
    .hint = Give the medium or the project a default tube, or set the tube on the line.

rule-ew-fl-008 = { $form ->
        [gallery] { $terminal }: gallery { $gallery } ({ $count } exhaust ports) has no silencer, collector line or closure
       *[ports] exhaust { $ports } has no silencer, collector line or closure
    }
    .title = Exhaust open
    .hint = Fit a silencer, draw a collector line or close the exhaust.

rule-ew-fl-009 = { $valve }: the coil takes { $voltage } V, but { $feeds } feed it
    .title = Coil voltage
    .hint = Choose a valve whose coil takes the voltage of its supply, or feed it from a potential of that voltage.

rule-ew-fl-010 = { $form ->
        [valve] { $device }: port { $port } has no supply; define a medium at it or draw the supply line
       *[gallery] { $device }: gallery 1 has no supply; define a medium at a supply port or draw the supply line
    }
    .title = No supply on port 1
    .hint = Define a medium at the supply port, or draw the line that brings it.

rule-ew-fl-011 = { $valve } drives { $target }, but its working-port lines reach { $device }
    .title = Drives target mismatch
    .hint = Link the valve to the device its lines reach, or draw the lines to the device it drives.

rule-ew-fl-012 = { $form ->
        [role] working port { $working } is joined to { $other } with no valve between them
       *[valve] { $ports } are ports of one valve and are joined
    }
    .title = Port-role conflict
    .hint = Draw each working line to a consumer, and join no two ports of one valve.

rule-ew-fl-013 = { $form ->
        [both] { $parent }: position { $position } holds both { $a } and { $b }
        [modules] { $parent }: position { $position } holds both { $a } and { $b }
        [outside] { $module }: position { $position } is outside its { $count } valve positions
        [outside_free] { $module }: position { $position } is outside its { $count } valve positions; free positions: { $free }
        [outside_full] { $module }: position { $position } is outside its { $count } valve positions; every position is taken
       *[empty] { $module }: position { $position } is empty; fit a valve or a blanking plate
    }
    .title = Occupancy
    .hint = Give each valve position one valve or blanking plate, and each module its own position.

rule-ew-fl-014 = tube { $tube } crosses from { $from } to { $to } without a bulkhead
    .title = Tube crosses a location
    .hint = Add a bulkhead fitting where the tube leaves the location, or accept the crossing.

## EW-LAYOUT: panel layout (at the project scope until space and surface scope kinds exist)

rule-ew-lay-001 = { $device } is not placed in the layout of { $location }; place it on a rail or on the mounting plate
    .title = Unplaced device
    .hint = Place the device on a rail or on the mounting plate of its location's layout.

rule-ew-lay-003 = the placement of { $device } realises part { $part }, which the device no longer uses; place the device with one of its parts, or give it the part back
    .title = Part use not on the device
    .hint = Place the device again with one of its parts, or give it back the part its placement realises.

rule-ew-lay-004 = { $device } { $location ->
        [none] has no location
       *[other] is in { $location }
    } but is placed in the layout of { $space }; move it to the layout of its location, or correct its location
    .title = Location mismatch
    .hint = Move the device to the layout of its own location, or correct its location.

rule-ew-lay-006 = { $form ->
        [placements] { $device } overlaps { $other }; move one of them
        [same] two placements of { $device } overlap; move one of them
        [limit] more footprints overlap on the mounting plate of { $enclosure }; the first { $listed } overlaps are listed
        [duct] { $device } overlaps the duct at { $duct }; move the device or the duct
       *[rail] the rail at { $rail } overlaps the duct at { $duct }; move the rail or the duct
    }
    .title = Collision
    .hint = Move one of the overlapping objects until their footprints no longer overlap.

rule-ew-lay-010 = { $form ->
        [on_duct] { $device } is placed on the duct at { $duct }, which carries no devices; place it on a rail
        [off_rail] { $device } is placed at { $s } mm on the rail at { $rail }, outside its 0.0 to { $length } mm; move it onto the rail
        [rail_turn] { $device } is turned on the rail at { $rail } by an angle other than 0° or 180°; turn it back
        [off_axis] { $device } is placed beside the axis of the rail at { $rail }; move it onto the rail
        [profile] { $device } (part { $part }) does not fit the rail at { $rail } ({ $profile }); place it on a rail it fits
        [surface_turn] { $device } is turned on the mounting plate by an angle other than a multiple of 90°; turn it back
        [off_surface] { $device } is placed off the plane of the mounting plate; place it on the plate
        [surface] { $device } (part { $part }) does not mount on the plate itself; place it on a rail
        [overflow] { $device } occupies { $from } to { $to } mm on the rail at { $rail }, beyond its 0.0 to { $length } mm; shorten it or move it
        [outside] { $device } lies outside the usable area of the mounting plate; move it inside
        [bracket_overlap] The end bracket { $side ->
            [before] before
           *[after] after
        } { $device } overlaps { $other_kind ->
            [bracket] the end bracket { $other_side ->
                [before] before
               *[after] after
            } { $other }
           *[placement] { $other }
        } on { $rail }.
        [bracket_outside] The end bracket { $side ->
            [before] before
           *[after] after
        } { $device } passes the { $edge ->
            [start] start
           *[end] end
        } of { $rail } by { $by } mm.
        [path] the path of the { $carrier ->
            [rail] rail at { $at } is not two points on one horizontal or vertical line
            [duct] duct at { $at } is not a line of horizontal and vertical segments
           *[other] carrier at { $at } is not a line of horizontal and vertical segments
        }; edit its path
        [carrier_outside] the { $carrier ->
            [rail] rail
            [duct] duct
           *[other] carrier
        } at { $at } lies outside the usable area of the mounting plate; move it inside
        [spaces] { $location } has a second layout space; a location has one layout space; remove it
        [enclosures] the layout of { $location } has a second enclosure, { $enclosure }; a layout space has one enclosure; remove it
       *[plates] the enclosure { $enclosure } has a second mounting plate; an enclosure has one mounting plate; remove it
    }
    .title = Carrier incompatible
    .hint = Place the device on a carrier it fits, within the carrier and the usable area of the plate.

rule-ew-lay-016 = { $form ->
        [order] the members of { $assembly } on the rail at { $rail } are out of order: { $early } belongs before { $late }; place the parent first, then the members by position
       *[between] { $other } is placed between members of { $assembly } on the rail at { $rail }; keep the members together
    }
    .title = Assembly order
    .hint = Place the assembly's parent first and its members next to it, in the order of their positions.

## EW-NUM: numbering

rule-ew-num-001 = { $frozen }: frozen, deviates from scheme (rule-derived value { $derived })
    .title = Tag deviates from the scheme
    .hint = Unfreeze the tag to take the derived value, or keep it frozen if the deviation is intended.

rule-ew-num-002 = wire number { $number }: frozen, deviates from the rule ({ $derived ->
        [none] the rule derives no number for the wire
       *[value] rule-derived value { $rule }
    })
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

rule-ew-plc-004 = { $form ->
        [input] { $channel }: the { $kind } channel reads the coil of { WORD($target) }; an input reads a sensor, check the wiring or the channel kind
       *[output] { $channel }: the { $kind } channel drives the contact of { WORD($target) }; an output drives an actuator, check the wiring or the channel kind
    }
    .title = Channel kind mismatch
    .hint = Wire the channel to a function of its kind, or change the channel kind.

rule-ew-plc-005 = { $channel }: the symbol { $symbol } is not an IEC 61131-3 identifier, which configuration project { WORD($cfg) } requires; use letters, digits and single underscores
    .title = Invalid symbol
    .hint = Give the channel a symbol of letters, digits and single underscores that does not start with a digit.

rule-ew-plc-009 = { $channel } has { $what ->
        [both] a manual address and a symbol
        [address] a manual address
       *[symbol] a symbol
    } but no connection; connect it, or mark it spare or reserved
    .title = Channel without connection
    .hint = Connect the channel, or mark it spare or reserved.

rule-ew-plc-010 = { $channel }: the data type { $data_type } ({ $bits } bit(s)) does not fit the { $width }-bit { $kind } channel; give the channel a data type of its width
    .title = Data-type width mismatch
    .hint = Give the channel a data type as wide as the channel.

rule-ew-plc-011 = { $form ->
        [slot] { WORD($module) } sits in slot { $slot } of { WORD($station) }, but its part { $part } allows slots { $allowed }; move it to an allowed slot
        [max_modules] { WORD($station) } has { $have } modules, more than the { $limit } its part allows; remove modules
        [input] { WORD($station) } has { $have } input bytes, more than the { $limit } its part allows; remove modules
       *[output] { WORD($station) } has { $have } output bytes, more than the { $limit } its part allows; remove modules
    }
    .title = Slot rules
    .hint = Put each module in a slot its part allows, and keep the station within its limits.

rule-ew-plc-013 = { $form ->
        [fe] { $cp_shared ->
            [yes] { $cp_tag } pin { $cp_pin } ({ NUMBER($cp_nth, type: "ordinal") ->
                [one] { $cp_nth }st
                [two] { $cp_nth }nd
                [few] { $cp_nth }rd
               *[other] { $cp_nth }th
            })
           *[no] { $cp }
        } is the FE pin of a PLC card and is fed from the PE potential { $potential }; connect it to functional earth, or declare an FE–PE bond
        [unconnected] { $cp }, the { $role } pin of the { $group } supply group, is not connected; connect the card's supply
       *[wrong] { $cp }, the { $role } pin of the { $group } supply group, is fed from a potential of type { $type }; feed it from { $role }
    }
    .title = Card supply wrong
    .hint = Connect every supply pin of the card to a potential of its role, and its FE pin to functional earth.

rule-ew-plc-014 = { $subject }: { $form ->
        [channel] the output
        [group] the load supply group
       *[module] the module
    } carries { $load } mA, more than its rating of { $limit } mA; spread the loads or use a stronger output
    .title = Output load exceeds rating
    .hint = Spread the loads over more outputs, or use a stronger output or module.

rule-ew-saf-001 = { $form ->
        [missing] { WORD($module) } is an F-module without an F-destination address; enter it
        [range] { WORD($module) }: the F-destination address { $address } is outside { $min }..{ $max }, the range of its device description; enter an address inside it
       *[duplicate] the F-destination address { $address } is used by { $modules }; give each F-module its own address
    }
    .title = F-address
    .hint = Give every F-module its own F-destination address inside the range of its device description.

rule-ew-saf-002 = { $form ->
        [incomplete] { $channel } is a 1oo2 channel without a partner that names it back; pair it with the other channel of the sensor
       *[devices] the 1oo2 pair { $channel } and { $other } reads two devices, { WORD($first) } and { WORD($second) }; wire both channels to the one two-channel sensor
    }
    .title = 1oo2 pair
    .hint = Pair each 1oo2 channel with the other channel of the same two-channel sensor.

rule-ew-saf-003 = { $channel }: the sensor { WORD($sensor) } is not supplied from the module's test-pulse pin; wire its supply to that pin
    .title = Test pulse
    .hint = Supply the sensor of an F-DI channel from the module's test-pulse pin.

rule-ew-prc-001 = { $form ->
        [scheme] { WORD($device) }: the process tag { $tag } does not match the project's scheme { $scheme }; correct the tag
       *[duplicate] the process tag { $tag } is carried by { $devices }; keep it on the one field device and track it from the others
    }
    .title = Process-tag scheme
    .hint = Give each field device its own process tag in the project's scheme.

## EW-BUS: bus networks

rule-ew-bus-001 = { $form ->
        [duplicate] { $named ->
            [none] { $untagged } ports without a tag
           *[some] { $others ->
                [none] { $ports }
                [one] { $ports } and a port without a tag
               *[other] { $ports } and { $untagged } ports without a tag
            }
        }: station address { $address } is used more than once in logical net { $net }; give each port its own address
       *[tbd] { $end ->
            [connector] a connector
            [device] a device:{ $port }
           *[named] { $port }
        }: the station address is TBD; enter it before release
    }
    .title = Bus address missing or duplicate
    .hint = Enter a station address that is unique in the logical net before release.

rule-ew-bus-004 = { $form ->
        [external] external end { $port } ({ $partner })
       *[other] { $end ->
            [ip] end { $port }
            [connector] a connector
            [device] a device:{ $port }
            [segment] a segment end
           *[named] { $port }
        }
    }: { $form ->
        [external] the termination at the external end is unknown; check that the partner terminates the segment
        [missing] no terminating element at this segment end; terminate the segment here
        [end] the termination is off at the segment end; switch it on
        [middle] the termination is on inside the segment; switch it off
       *[unknown] the termination state is of a newer format; check it
    }
    .title = Termination
    .hint = Switch the termination on at both ends of the segment and off everywhere between them.

rule-ew-bus-002 = { $port }: the station address { $address } is outside { $min }..{ $max }; enter an address inside the range
    .title = Address outside the range
    .hint = Enter a station address inside the range of the bus and of the port's part.

rule-ew-bus-003 = { $form ->
        [name] { $port }: the device name { $name } is not DNS-compatible; use lower-case letters, digits, hyphens and dots
        [subnet] { $port }: the IP address { $ip } is outside the subnet of the master, { $master_ip } with mask { $mask }; enter an address of that subnet
        [duplicate_name] the device name { $value } is used by { $ports } in logical net { $net }; give each port its own
       *[duplicate_ip] the IP address { $value } is used by { $ports } in logical net { $net }; give each port its own
    }
    .title = PROFINET name or IP
    .hint = Give every PROFINET device its own DNS-compatible name and its own IP address in the master's subnet.

rule-ew-bus-005 = { $port } does not support { $rate } bit/s, the rate of logical net { $net }; choose a rate every station supports
    .title = Rate not supported
    .hint = Choose a rate that every station of the logical net supports.

rule-ew-bus-006 = the segment from { $segment } { $form ->
        [stations] has { $count } stations, more than the { $limit } a PROFIBUS segment takes; split it with a repeater
        [length] is { $metres } m long, more than the { $limit } m PROFIBUS allows at { $rate } bit/s; shorten it or lower the rate
       *[stub] branches, and PROFIBUS allows no stubs above 1.5 Mbit/s; run the line through every station
    }
    .title = Segment limits
    .hint = Keep each segment within the station count, length and topology its rate allows.

rule-ew-bus-007 = logical net { $net } has no master; name its master port, or the external partner that is
    .title = No master
    .hint = Name the logical net's master: a port of the project or the external partner.

rule-ew-bus-009 = { $form ->
        [cable] { $port } is connected by { WORD($cable) }, which is not a cable for its bus; use a bus cable
       *[connector] { $port } ({ $connector }) is mated with { $other } ({ $other_connector }); the connector types differ
    }
    .title = Wrong bus cable
    .hint = Connect bus ports with a cable for their bus, and mate connectors of one type.

rule-ew-bus-010 = the ports joined with { $port } belong to the logical nets { $nets }; put every port of the segment in one logical net
    .title = Net mismatch
    .hint = Put every port that bus connections join in one logical net.

rule-ew-iol-001 = { $form ->
        [mode] { $channel } is in mode { $mode }, but the IO-Link device { WORD($device) } is connected; set the port to IO-Link
       *[class] { $channel } is a class-{ $port_class } port, but { WORD($device) } needs class { $device_class }; connect it to a class-B port
    }
    .title = IO-Link port class
    .hint = Connect each IO-Link device to a port of its class, in IO-Link mode.

rule-ew-iol-002 = { $channel }: { $form ->
        [window] the device's { $have } { $dir } bits exceed the port window of { $room } bits; use a master with a larger window
       *[cable] the cable { WORD($cable) } is { $metres } m long, more than the { $max } m an IO-Link port allows; shorten it
    }
    .title = IO-Link limits
    .hint = Keep each IO-Link device within its port's window and cable length.

## EW-POT: potentials

rule-ew-pot-001 = { $form ->
        [flow] pass-through of function { $device } joins type { $from } and type { $to } without a converting function
       *[attrs] { $conflicts }{ $more ->
            [none] {""}
           *[other] ; and { $more } more member CPs
        }{ $hint ->
            [bond] ; declare the bond with Draw ▸ Connect ▸ Bond
           *[none] {""}
        }
    }
    .title = Type or voltage conflict
    .hint = Separate the potentials, correct the definition, or declare the bond that joins them.
    .conflict-type = { $subject } sets type { $value }; { $applied } applies (first definition in natural order)
    .conflict-voltage = { $subject } sets voltage { $value }; { $applied } applies (first definition in natural order)
    .conflict-environment = { $subject } sets environment { $value }; { $applied } applies (first definition in natural order)
    .conflict-class = { $subject } sets circuit class { $value }; { $applied } applies (first definition in natural order)
    .conflict-role = member CP { $subject } has role { $value }; the defined type { $applied } applies
    .conflict-roles = member roles { $value } meet { $applied }; the first in role order applies

rule-ew-pot-002 = { $form ->
        [selv] SELV potential { $potential } is bonded to PE ({ $count } 0 V–PE bond(s)); a SELV circuit must not be earthed
       *[pelv] PELV potential { $potential } { $bonds ->
            [none] has no 0 V–PE bond
           *[some] has { $count } 0 V–PE bonds
        }; a PELV circuit needs exactly one (declare the bond on the 0 V–PE wire)
    }
    .title = PELV or SELV bonding
    .hint = Declare exactly one 0 V–PE bond for a PELV potential, and none for a SELV potential.

rule-ew-pot-003 = { $cp_shared ->
        [yes] { $cp_tag } pin { $cp_pin } ({ NUMBER($cp_nth, type: "ordinal") ->
            [one] { $cp_nth }st
            [two] { $cp_nth }nd
            [few] { $cp_nth }rd
           *[other] { $cp_nth }th
        })
       *[no] { $cp }
    } has the role FE and is wired to the PE potential { $potential }; declare an FE–PE bond or connect it to functional earth
    .title = FE pin on PE
    .hint = Declare an FE–PE bond, or connect the pin to functional earth.

rule-ew-pot-004 = wire { $wire } is GNYE on the potential { $potential }{ $typed ->
        [yes] {" "}(type { $type })
       *[no] {""}
    }; GNYE is reserved for protective conductors
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

rule-ew-pot-008 = { $form ->
        [terminal] { $detail }
       *[island] { $location ->
            [main] the main location
           *[node] { $node }
        } holds { $count } PE potentials: { $potentials }; join them through a bonded rail or a PE wire
    }
    .title = PE island
    .hint = Connect the PE potentials of the location, for example through the rail bonding of the terminals.

rule-ew-pot-009 = { $cp_shared ->
        [yes] { $cp_tag } pin { $cp_pin } ({ NUMBER($cp_nth, type: "ordinal") ->
            [one] { $cp_nth }st
            [two] { $cp_nth }nd
            [few] { $cp_nth }rd
           *[other] { $cp_nth }th
        })
       *[no] { $cp }
    } is not connected{ $unused ->
        [yes] {" "}(marked unused)
       *[no] ; connect it or mark it unused
    }
    .title = Unconnected CP
    .hint = Connect the connection point, or mark it unused.

rule-ew-pot-011 = { $potential }: it is unclear which wire feeds { $place ->
        [junction] the junction { $at ->
            [cell] at { $cell } on folio { $page }
           *[folio] on folio { $page }
        } (wires { $wires })
        [ip] the interruption points { $group } (wires { $wires })
       *[other] a junction or a group of interruption points
    }; draw one wire into it from the supply side
    .title = Junction feed ambiguous
    .hint = Draw the junction so that one end feeds it, or set its wiring order.

rule-ew-pot-012 = { $form ->
        [ring] the connection closes a ring in the drawing graph; it is the highest ID of the ring and forms no physical wire
       *[other] { $detail }
    }
    .title = Ring in the drawing graph
    .hint = Remove the wire that closes the ring, or confirm that the ring is intended.

rule-ew-pot-013 = { $device } { $form ->
        [isolating] is an N-isolating terminal on { $from } → { $to }
       *[switching] switches the neutral { $from } → { $to }{ $loads ->
            [one] ; load { $load } stays connected to { $line }
            [many] ; { $stranded }
           *[none] {""}
        }.
    }
    .title = N switching
    .hint = Move the contact into the line conductor, or use a device that opens L and N together.

rule-ew-pot-014 = { $detail }
    .title = Physical-wire conflict
    .hint = Give the physical wire one set of properties, or split it into separate wires.

rule-ew-pot-015 = wire number { $number } is printed on { $count } wires; a wire number names one numbering unit
    .title = Wire-number conflict
    .hint = Give the wires of one potential the same wire number, or separate the potentials.

rule-ew-pot-016 = the override { $member ->
        [csa] cross-section { $value }
        [colour] colour { $value }
        [wire_part] wire part
        [insulation] insulation
        [function] wire function
        [shielded] shielded
       *[twisted] twisted
    } of wire { $wire } equals the value below it in the inheritance chain; remove the override
    .title = Redundant override
    .hint = Remove the override; it sets the value the wire takes anyway.

rule-ew-pot-017 = the definitions of one potential name it { $names }; the first in natural order ({ $first }) applies
    .title = Two potential names
    .hint = Keep one definition of the potential, or rename the interruption points to one name.

rule-ew-pot-021 = { $cp_shared ->
        [yes] { $cp_tag } pin { $cp_pin } ({ NUMBER($cp_nth, type: "ordinal") ->
            [one] { $cp_nth }st
            [two] { $cp_nth }nd
            [few] { $cp_nth }rd
           *[other] { $cp_nth }th
        })
       *[no] { $cp }
    } is on { $potential }, which reaches no supply: the load is fed but not returned; connect it to the return conductor (0 V, N)
    .title = Load not returned
    .hint = Wire the load's return side to the return conductor of its supply (0 V, N), or link it to the module's return pin.

## EW-RDS (EW-IEC81346): reference designations

rule-ew-rds-001 = { $device } does not parse as an IEC 81346 designation: { $detail }
    .title = Designation does not parse
    .hint = Correct the designation to the reference designation syntax of IEC 81346.

rule-ew-rds-002 = the structure code { $code } contains separator characters; tools that read exported designations may split it into several levels
    .title = Separator in a structure code
    .hint = Remove the separator from the code, or use a structure level instead.

rule-ew-rds-003 = { $device }: the letter code { $letter } does not match its function category { $category }; the identifier set { $set } suggests { $suggested }{ $verified ->
        [yes] {""}
       *[no] {" "}(the set is not verified against the standard)
    }
    .title = Letter code against function
    .hint = Use the suggested letter code, or keep the tag if the classification is intended.

rule-ew-rds-004 = `{ $tag }` is printed by { $count } devices
    .title = Duplicate designation
    .hint = Give one of the devices another designation.

rule-ew-rds-007 = { $device }: the letter code { $letter } uses reserved letters: { $reasons }
    .title = Reserved letters
    .hint = Avoid the letters I and O in a letter code; choose another letter.

rule-ew-rds-009 = the { $what } { $text } of { $device } contains non-Latin characters that look like Latin ones: { $chars }; search and cross-references cannot find it; retype it in Latin letters
    .title = Confusable character
    .hint = Replace the character by its plain Latin form.

rule-ew-rds-010 = the structure codes { $codes } differ only in separators or case; merge them into one node and keep the other spellings as aliases
    .title = Structure-code variants
    .hint = Merge the structure nodes whose codes differ only in spelling.

## EW-REL: the release gate

rule-ew-rel-001 = revision { $revision }: { $form ->
        [author] author not set; set it with Check ▸ Revision ▸ Set revision author…
        [checker] checker not set; the checker approves the revision
        [four-eyes] the checker is the author ({ $role }); four eyes need another party
        [sign-off] sign-off missing; the checker approves the saved revision
       *[stale] the sign-off does not match the saved content (edited after approval); approve it again
    }
    .title = Four eyes
    .hint = Set the author, then let another party approve the saved revision; approve again after a content edit.

rule-ew-rel-002 =
    .title = Stale committed computation
    .hint = Recompute the stale result and commit it again before the release.

rule-ew-rel-004 = revision { $revision }: { $form ->
        [format] the label is not in the project's revision label format { $pattern }
       *[order] the open revision's label does not follow { $later } of revision { $id }; revision labels increase, the open revision last
    }
    .title = Revision label order
    .hint = Give the revision a label in the project's label format that follows every earlier label.

## EW-TERM: terminals

rule-ew-term-001 = a terminal of { $strip } has no designation
    .title = Terminal without designation
    .hint = Give the terminal a designation.

rule-ew-term-002 = { $form ->
        [taken] designation { $designation } is taken on { $end ->
            [untagged] the strip
           *[named] { $strip }
        } ({ $count } terminals){ $remedy ->
            [free] ; use { $free }
            [swap] ; swap { $old } ↔ { $designation } in one transaction (terminal.designate)
            [both] ; use { $free } or swap { $old } ↔ { $designation } in one transaction (terminal.designate)
           *[none] {""}
        }
       *[used] designation { $designation } is used by { $count } terminals of { $end ->
            [untagged] strip
           *[named] { $strip }
        }
    }
    .title = Duplicate designation
    .hint = Renumber the strip, or give one of the terminals another designation.

rule-ew-term-003 = spare terminal { $terminal } is wired on its field side; wire it on the other side or clear its spare state
    .title = Spare wired on the field side
    .hint = Remove the field-side wire, or make the terminal a used terminal.

rule-ew-term-004 = bridge group { $group } of { $strip } is invalid: { $gap ->
        [yes] { $sep_gap }its terminals are not consecutive
       *[no] {""}
    }{ $shafts ->
        [yes] { $sep_shafts }it runs over { $shaft_count } shafts
       *[no] {""}
    }{ $reaches ->
        [yes] { $sep_reaches }it reaches { $reached }
       *[no] {""}
    }{ $overlaps ->
        [yes] { $sep_overlaps }it overlaps bridge group { $overlapped } on its shaft
       *[no] {""}
    }; a bridge group runs over consecutive terminals of one strip on one shaft
    .title = Bridge group invalid
    .hint = Bridge only neighbouring terminals of one strip, or move the terminals together.

rule-ew-term-005 = rail_bonded is set but the terminal has no PE CP; it joins no location PE node
    .title = Rail bonding on a non-PE terminal
    .hint = Clear the rail bonding, or make the terminal a PE terminal.

rule-ew-term-006 = { $terminal } lies on wire { $wire } without joining it; connect it to the wire or move it
    .title = Terminal on a wire it does not join
    .hint = Connect the terminal into the wire, or move it off the wire.

## WP-S1.17 part 8 (COL49): collisions after a merge

rule-ew-core-013 = { $collision } after a merge; { $keeper ->
        [frozen] the record with the frozen tag
       *[oldest] the oldest record
    } keeps it, and { $others ->
        [many] the others
       *[one] the other
    } must change
    .title = Collision after a merge
    .hint = Renumber the newer device with the quick fix, or give one of the records another number.

## EW-IMP: the import findings of the migration report (WP-S3.09 D9)

rule-ew-imp-001 =
    .title = Input limit exceeded; item or file refused
    .hint = Split the source into smaller files, or remove the items above the limit, and import again.

rule-ew-imp-002 =
    .title = Malformed or refused item skipped
    .hint = Correct the item in the source, or draw it again in this project.

rule-ew-imp-003 =
    .title = Unresolvable reference dropped
    .hint = Draw the missing connection or link again in this project.

rule-ew-imp-004 =
    .title = Content dropped by design
    .hint = Nothing to do: this content has no counterpart in a project; redraw it if it is needed.

rule-ew-imp-005 =
    .title = Object inferred, review
    .hint = Check the inferred object and confirm or split it.

rule-ew-imp-006 =
    .title = Font substituted
    .hint = Nothing to do, or choose another text style if the substitute prints differently.

rule-ew-imp-007 =
    .title = Value mapped through an alias table
    .hint = Check the mapped value; revert it or choose another where the alias is wrong.

rule-ew-imp-008 =
    .title = Value kept as foreign or opaque text
    .hint = Map the value to one the project knows, with the quick fix where it offers one.

rule-ew-imp-009 =
    .title = Print-equivalence deviation
    .hint = Compare the derived text with the source's printed text and correct the one that is wrong.

rule-ew-imp-010 =
    .title = Removed in the source, kept here
    .hint = Delete the record if it is no longer needed; otherwise keep it.

rule-ew-imp-011 =
    .title = Changed in the source and here, kept here
    .hint = Compare the source's change with this project's and take over what is needed by hand.

rule-ew-imp-101 =
    .title = Stored label differs from the formula result
    .hint = Use the derived tag, or keep the stored label as a frozen tag.

rule-ew-imp-103 =
    .title = Label kept as a frozen tag
    .hint = Convert the tag to the scheme where it conforms, or keep it frozen.

rule-ew-imp-104 =
    .title = Per-folio title-block value conflict
    .hint = Create the revisions the folios name and drop the per-page values.

rule-ew-imp-109 =
    .title = Strip segments merged into one strip
    .hint = Nothing to do, or split the strip if its segments are separate strips.

rule-ew-imp-115 =
    .title = Folio-report pair names differ
    .hint = Rename one side of the pair, or set the pairing to chain mode.

rule-ew-imp-116 =
    .title = Slave without master, or master above its slaves
    .hint = Link the slave to its master, or reduce the master's links.

rule-ew-imp-117 =
    .title = Default folio-index formula
    .hint = Switch the folio references to the folio label.

rule-ew-imp-118 =
    .title = Junction wiring assumed
    .hint = Set the junction's wiring to Chain where the source wired it so.

rule-ew-imp-119 =
    .title = Element without function semantics
    .hint = Map the symbol to a function definition, or keep the generic black box.

rule-ew-imp-120 =
    .title = Shared element UUID disambiguated
    .hint = Nothing to do: each copy got its own record.

rule-ew-imp-122 =
    .title = Unlabelled element, tag synthesised
    .hint = Give the device a label; its tag prints "?" until then.

rule-ew-imp-102 =
    .title = Structure-code spelling variants
    .hint = Merge the variants into one structure code and keep the others as aliases.

rule-ew-imp-105 =
    .title = Per-folio description kept as an override
    .hint = Drop the override to print the project description again, or keep it.

rule-ew-imp-114 =
    .title = Imported text styles not bound to slot kinds
    .hint = Apply the text style the slot kind names.

# The quick fixes' labels (FT-1482, ID-1457): the window's text of a fix by its key (`QuickFix::key`); the en-GB
# text is the fix's English label, which the CLI and the JSON print.
fix-drop-override = drop override
fix-anchor-graphics-centre = anchor at the graphics centre
fix-renumber-newer = Renumber the newer
fix-unfreeze-tag = Unfreeze the tag
fix-realise-primary-part = Realise the primary part
fix-rebind-map-channel = Rebind to the map's channel
fix-declare-pelv-bond = Declare 0 V–PE bond (PELV)

# The words a rule prints where a record has no name of its own (FT-912): a pattern of another language wraps an
# argument that may hold them in WORD(…), which prints that language's message of the same ID instead.
word-device = device
word-cable = cable
word-without-a-tag = without a tag
word-a-junction = a junction
word-assembly = assembly
word-a-location = a location
word-a-device-without-designation = a device without designation
word-a-missing-part = a missing part
word-an-unknown-profile = an unknown profile
word-a-placement = a placement
word-none = none
word-terminal = terminal
word-strip = strip
word-another-strip = another strip
word-valve = valve
word-the-valve = the valve
word-a-device = a device
word-a-module = a module
word-a-part = a part
word-a-configuration-project = a configuration project
word-no-address = no address
word-a-role-of-a-newer-format = a role of a newer format
word-junction = junction
word-a-drawn-page = a drawn page
word-a-section-divider = a section divider
word-a-report-page = a report page
word-the-strip = the strip
word-a-device-that-is-not-a-terminal = a device that is not a terminal
word-the-project = the project
word-a-page = a page
word-a-logical-bus-net = a logical bus net
word-a-physical-bus-net = a physical bus net
word-the-junction-in = the junction in { $at }
word-a-connection-point-of-another-page = a connection point of another page
word-its-target = its target
word-the-station = the station
word-a-station = a station
word-an-f-module = an F-module
word-the-sensor = the sensor
word-the-cable = the cable
word-the-device = the device

rule-ew-imp-251 =
    .title = Drawing without a unit, read in the chosen unit
    .hint = Check the drawing's size; import it again in the right unit if it is wrong.

rule-ew-imp-252 =
    .title = SHX font drawn in a bundled font
    .hint = Check the texts: their widths may differ from the SHX font's.

rule-ew-imp-253 =
    .title = Drawing entity not imported
    .hint = Draw what the entity showed by hand, or import a 2D export of the drawing.

rule-ew-imp-254 =
    .title = External file not opened
    .hint = Place the file the drawing refers to yourself if the page needs it.

rule-ew-imp-255 =
    .title = Hatch pattern reduced to its outline
    .hint = Draw the hatch's fill by hand if the page needs it.

rule-ew-imp-257 =
    .title = DWG converted by the ODA File Converter
    .hint = Compare the page with the DWG: the conversion is the converter's.
rule-ew-plc-016 = configuration project { $cfg } changed since the { $format ->
        [aml] AML
        [tia_xlsx] tag table
        [plcopen] PLCopen
       *[other] PLC
    } { $direction ->
        [import] import
       *[export] export
    } of { $date }: { $count } object(s) differ ({ $objects }{ $more ->
        [none] {""}
       *[other] {" "}and { $more } more
    }); export it again, or import the PLC's file
    .title = Exchange drift
    .hint = Export the configuration project again, or import the file of the PLC software.

rule-ew-prc-002 = { $form ->
        [device] instrument tag { $tag } has no device any more; apply the instrument list again, or remove the tag from it
       *[channel] instrument tag { $tag } ({ $signal }) has no channel: no spare { $signal } channel was free; add a module or free a channel, then apply the instrument list again
    }
    .title = Open instrument tag
    .hint = Add a module or free a channel of the tag's kind, then apply the instrument list again.
