# SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
# SPDX-License-Identifier: GPL-3.0-or-later

# The rule messages in en-GB (doc 15 §3.8): the mandatory language and the fallback of every other.
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
            [function] a function of device { $tag }
            [connection-on] connection from { $from } to { $to } on folio { $page }
            [connection] connection from { $from } to { $to }
           *[other] { $kind }
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
            [yes] { $sep_csa }cross-section { DECIMAL($csa_core) } ≠ { DECIMAL($csa_part) }
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

rule-ew-core-033 = { $cable } has no length yet: type it in its Properties, or route the cable in the layout
    .title = Cable without length
    .hint = Type the length in the cable's Properties, route the cable in the layout, or choose a cable part with a fixed length. It is an error only in the check for delivery.

rule-ew-core-034 = the external connection { $far ->
        [unknown] from a place not known to { $inside }
       *[known] { $connection }
    } has no cable: { $count ->
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

rule-ew-core-041 = { $device } has no part, so the parts list cannot order it. Choose one in its Properties (Part).
    .title = Device without part
    .hint = Open the message: the device's Properties open. Choose its part there (Part), from the library or a part of your own.

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

rule-ew-core-047 = { $device } is not drawn on any page, so its tag shows ? where the page number belongs{ $part ->
        [yes] , and the parts list still counts it
       *[no] {""}
    }. Draw it on a page, or delete it if it is not needed.
    .title = Device not drawn
    .hint = Open the message: the device's Properties open. Place it on a page from the Devices list (drag it onto the page), or delete it.

rule-ew-core-085 = { $device } names the variant { $variant }, which { $part } does not hold
    .title = Variant not in the part
    .hint = Choose a variant the part holds in Properties ▸ Parts ▸ Variant; the lists print the part until then.

rule-ew-core-048 = { $device } is covered by { $form ->
        [covered-twice] { $count } order codes of { $assembly }; its parts would print { $count } times
       *[uncovered] no order code of { $assembly }; the assembly BOM lists its own parts beside the configured products
    }
    .title = Order codes do not cover the children
    .hint = Add the missing children to an order code of the assembly, or remove them from the assembly.

rule-ew-core-051 = interruption point { $name } { $form ->
        [no-cp] has no connection point; it pairs with nothing
        [target] at { $at } is a target without a source: place its source where the potential leaves, or turn this mark into the source
        [source] at { $at } is a source without a target: place a target where the potential arrives, or turn this mark into a target
       *[unpaired] has no partner; its wire stays open-ended
    }
    .title = Unpaired interruption point
    .hint = A potential comes from one source and goes on at its targets, each a point of the same name: place the one this mark lacks, or change its Direction in Properties.

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

rule-ew-core-061 = { $name } prints { DECIMAL($height) } mm high, too small to read on paper ({ DECIMAL($minimum) } mm at least). Make it larger in Text styles.
    .title = Text below minimum height
    .hint = Open the message: Text styles opens at the style. Make it at least 1.2 mm high there, or press Set to 1.2 mm.

rule-ew-core-063 = a text on folio { $page } has the size { $style }, which no text style binds; give it a text style
    .title = Unstyled text size
    .hint = Give the text one of the project's text styles instead of a free size.

rule-ew-core-065 = description "{ $text }" of part { $part }{ $form ->
        [rating] {" "}holds the rating { $matches }: move it to the technical data
       *[unit-format] : unit formatting { $matches } (a space between number and unit, units as written in SI)
    }
    .title = Description lint
    .hint = Move ratings and technical data out of the description into their typed fields.

rule-ew-core-067 = { $line ->
        [tube] Tube
       *[wire] Wire
    } { $wire } crosses { $point ->
        [junction] the junction in { $pin }
        [junction-off-grid] a junction
       *[cp] { $pin }
    } without joining it, so on paper it looks connected. Route it again so that it passes beside.
    .title = Wire through a connection point
    .hint = Open the message: the wire is selected on its page. Press Route, or drag its round handles so that it keeps clear of the point.

rule-ew-core-068 = { $line ->
        [tube] Tube
       *[wire] Wire
    } { $wire } lies on top of { $other } on page { $page }, so on paper the two cannot be told apart. Route one of them again.
    .title = Wire along another wire
    .hint = Open the message: both wires are selected on their page. Press Route, or drag one wire's handles, so that each runs on a line of its own.

rule-ew-core-069 = { $line ->
        [tube] Tube
       *[wire] Wire
    } { $wire } runs through { $symbol ->
        [ip] interruption point { $device }
       *[device] { $device }
    }.
    .title = Wire through a symbol
    .hint = Select the wire and use Route, or move the symbol out of its way.

rule-ew-core-070 = { $line ->
        [tube] Tube
       *[wire] Wire
    } { $wire } runs through { $owner ->
        [free-text] the free text "{ $text }"
        [structure-box] the caption "{ $text }" of a structure box
        [ip] the text "{ $text }" of interruption point { $device }
        [wire] the number "{ $text }" of { $device }
        [mark] the cable mark "{ $text }"
       *[symbol] the text "{ $text }" of { $device }
    }.
    .title = Wire through a text
    .hint = Select the wire and use Route, or move the text.

rule-ew-core-071 = a graphic line ends on { $cp }, which joins nothing electrically; draw a connection instead
    .title = Graphic line on a CP
    .hint = Replace the graphic line by a connection, so that it takes part in the connectivity.

rule-ew-core-072 = { $place ->
        [off-grid] A junction on folio { $page } joins four wires: draw two T junctions instead.
       *[cell] The junction at { $cell } joins four wires: draw two T junctions instead.
    }
    .title = Four-way junction
    .hint = Move one branch to a junction of its own a grid step away, so that each junction joins three wires.

rule-ew-core-073 = { $line ->
        [tube] Tube
       *[wire] Wire
    } { $wire } has no route: it is drawn as a dashed line.
    .title = Unrouted wire
    .hint = Run Route; where it finds no clear way, drag the wire's handles or move the symbols.

rule-ew-core-074 = { $line ->
        [tube] Tube
       *[wire] Wire
    } { $wire } and { $other } turn on one point { $place ->
        [off-grid] on folio { $page }
       *[cell] at { $cell }
    } without a junction, so on paper they look joined. Route one of them again.
    .title = Wires meet without joining
    .hint = Open the message: both wires are selected. Route one of them again, or drag a bend a grid step away.

rule-ew-core-075 = { $owner ->
        [free-text] The free text "{ $text }"
        [structure-box] The caption "{ $text }" of a structure box
        [ip] The text "{ $text }" of interruption point { $device }
        [wire] The number "{ $text }" of { $device }
        [mark] The cable mark "{ $text }"
       *[symbol] The text "{ $text }" of { $device }
    } lies on the text "{ $other }" { $place ->
        [off-grid] on folio { $page }
       *[cell] at { $cell }
    }, so neither can be read. Move one of them.
    .title = Text on a text
    .hint = Drag one of the texts to a free place, or move its symbol; a text you moved stays where you put it.

rule-ew-core-076 = { $owner ->
        [free-text] The free text "{ $text }"
        [structure-box] The caption "{ $text }" of a structure box
        [ip] The text "{ $text }" of interruption point { $device }
        [wire] The number "{ $text }" of { $device }
        [mark] The cable mark "{ $text }"
       *[symbol] The text "{ $text }" of { $device }
    } lies on { $symbol ->
        [ip] interruption point { $part }
       *[device] { $part }
    } { $place ->
        [off-grid] on folio { $page }
       *[cell] at { $cell }
    }. Move the text or the symbol.
    .title = Text on a symbol
    .hint = Drag the text to a free place, or move the symbol it lies on.

rule-ew-core-077 = { $symbol ->
        [ip] Interruption point { $device }
       *[device] { $device }
    } { $form ->
        [outside] stands outside the drawing area of page { $page }. Move it into the frame.
       *[part] lies on { $other_symbol ->
            [ip] interruption point { $other }
           *[device] { $other }
        }. Move one of them.
    }
    .title = Symbol on a symbol
    .hint = Open the message: the symbols are selected. Drag one of them to a free place.

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

## EW-FL: fluid power

rule-ew-fl-001 = { $form ->
        [many] { $valve }: working ports { $ports } are open; connect, plug or blank them
       *[one] { $valve }: working port { $port } is open; connect, plug or blank it
    }
    .title = Working port open
    .hint = Draw the working line, or close the port with a plug, a silencer or a blanking.

rule-ew-fl-002 = { $form ->
        [excess] { $valve }: solenoid { $solenoid } is bound to { $count } channels: { $channels }
        [undeclared] { $valve }: { $channel } drives a coil that the part does not declare
       *[unbound] { $valve }: no PLC output drives solenoid { $solenoid }; bind one to it
    }
    .title = Solenoids and their outputs
    .hint = Give each solenoid the valve's part names one PLC output (on a valve terminal with a bus node, a channel of the valve terminal; on a multipin valve terminal, the output wired to its plug pin), and no output to another coil.

rule-ew-fl-004 = { $form ->
        [blank] { $module }: position { $position } is closed by a blanking plate, { $device }
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

rule-ew-fl-007 = { $missing ->
        [material] tube { $tube } is { $size } mm but has no material; give the default tube of its medium or of the project a material, or set it on the tube
       *[spec] tube { $tube } has no tube specification; give the medium or the project a default tube, or set it on the line
    }
    .title = Tube without specification
    .hint = Give the medium or the project a default tube, or set it on the tube.

rule-ew-fl-008 = { $form ->
        [gallery] { $terminal }: gallery { $gallery } ({ $count } exhaust ports) has no silencer, collector line or closure
       *[ports] exhaust { $ports } has no silencer, collector line or closure
    }
    .title = Exhaust open
    .hint = Fit a silencer, draw a collector line or close the exhaust.

rule-ew-fl-009 = { $valve }: the coil takes { $voltage }, but { $feeds } feed it
    .title = Coil voltage
    .hint = Choose a valve whose coil takes the voltage of its supply, or feed it from a potential of that voltage.

rule-ew-fl-010 = { $form ->
        [valve] { $device }: no compressed air reaches port { $port }. Draw the supply line to it, or use Define medium at port there.
       *[gallery] { $device }: no compressed air reaches its valves, because nothing feeds the valve terminal's supply. Draw the supply line to its supply port, or use Define medium at port there.
    }
    .title = No supply
    .hint = Draw the line that brings the air from the supply to the port, or right-click the port and choose Define medium at port.

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
       *[empty] { $module }: position { $position } is empty; fit a valve, or a blanking plate (right-click the position: Blanking plate)
    }
    .title = Occupancy
    .hint = Give each valve position one valve or blanking plate (right-click the position: Blanking plate), and each module its own position.

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
        [off_rail] { $device } is placed at { DECIMAL($s) } mm on the rail at { $rail }, outside its { DECIMAL("0.0") } to { DECIMAL($length) } mm; move it onto the rail
        [rail_turn] { $device } is turned on the rail at { $rail } by an angle other than 0° or 180°; turn it back
        [off_axis] { $device } is placed beside the axis of the rail at { $rail }; move it onto the rail
        [profile] { $device } (part { $part }) does not fit the rail at { $rail } ({ $profile }); place it on a rail it fits
        [surface_turn] { $device } is turned on the mounting plate by an angle other than a multiple of 90°; turn it back
        [off_surface] { $device } is placed off the plane of the mounting plate; place it on the plate
        [surface] { $device } (part { $part }) does not mount on the plate itself; place it on a rail
        [overflow] { $device } occupies { DECIMAL($from) } to { DECIMAL($to) } mm on the rail at { $rail }, beyond its { DECIMAL("0.0") } to { DECIMAL($length) } mm; shorten it or move it
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
        } of { $rail } by { DECIMAL($by) } mm.
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
        [input] { $channel }: the { $kind } channel reads the coil of { $target }; an input reads a sensor, check the wiring or the channel kind
       *[output] { $channel }: the { $kind } channel drives the contact of { $target }; an output drives an actuator, check the wiring or the channel kind
    }
    .title = Channel kind mismatch
    .hint = Wire the channel to a function of its kind, or change the channel kind.

rule-ew-plc-005 = { $channel }: the symbol { $symbol } is not an IEC 61131-3 identifier, which configuration project { $cfg } requires; use letters, digits and single underscores
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
        [occupied] { $module } and { $other } sit in the same slot { $slot } of { $station }; move one of them to a free slot
        [slot] { $module } sits in slot { $slot } of { $station }, but its part { $part } allows slots { $allowed }; move it to an allowed slot
        [max_modules] { $station } has { $have } modules, more than the { $limit } its part allows; remove modules
        [input] { $station } has { $have } input bytes, more than the { $limit } its part allows; remove modules
       *[output] { $station } has { $have } output bytes, more than the { $limit } its part allows; remove modules
    }
    .title = Slot rules
    .hint = Put each module in a slot of its own that its part allows, and keep the station within its limits.

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

rule-ew-plc-017 = { $device } sits in the PLC as a module, but its part { $part } describes no PLC module; take it out of the rack (a bus device joins a PLC as a station of its own)
    .title = Not a PLC module
    .hint = Take the device out of the rack; add a bus device to a PLC as a station from its menu.

rule-ew-plc-018 = { $device } is drawn, but none of its channels is bound to its pin: its addresses stand at no pin and a device wired to it is no channel's target; bind them
    .title = Module drawn with its channels unbound
    .hint = Bind the channels to their pins with the fix; a module drawn now is bound at once.

rule-ew-saf-001 = { $form ->
        [missing] { $module } is an F-module without an F-destination address; enter it
        [range] { $module }: the F-destination address { $address } is outside { $min }..{ $max }, the range of its device description; enter an address inside it
       *[duplicate] the F-destination address { $address } is used by { $modules }; give each F-module its own address
    }
    .title = F-address
    .hint = Give every F-module its own F-destination address inside the range of its device description.

rule-ew-saf-002 = { $form ->
        [incomplete] { $channel } is a 1oo2 channel without a partner that names it back; pair it with the other channel of the sensor
       *[devices] the 1oo2 pair { $channel } and { $other } reads two devices, { $first } and { $second }; wire both channels to the one two-channel sensor
    }
    .title = 1oo2 pair
    .hint = Pair each 1oo2 channel with the other channel of the same two-channel sensor.

rule-ew-saf-003 = { $channel }: the sensor { $sensor } is not supplied from the module's test-pulse pin; wire its supply to that pin
    .title = Test pulse
    .hint = Supply the sensor of an F-DI channel from the module's test-pulse pin.

rule-ew-prc-001 = { $form ->
        [scheme] { $device }: the process tag { $tag } does not match the project's scheme { $scheme }; correct the tag
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
        [cable] { $port } is connected by { $cable }, which is not a cable for its bus; use a bus cable
       *[connector] { $port } ({ $connector }) is mated with { $other } ({ $other_connector }); the connector types differ
    }
    .title = Wrong bus cable
    .hint = Connect bus ports with a cable for their bus, and mate connectors of one type.

rule-ew-bus-010 = the ports joined with { $port } belong to the logical nets { $nets }; put every port of the segment in one logical net
    .title = Net mismatch
    .hint = Put every port that bus connections join in one logical net.

rule-ew-iol-001 = { $form ->
        [mode] { $channel } is in mode { $mode }, but the IO-Link device { $device } is connected; set the port to IO-Link
       *[class] { $channel } is a class-{ $port_class } port, but { $device } needs class { $device_class }; connect it to a class-B port
    }
    .title = IO-Link port class
    .hint = Connect each IO-Link device to a port of its class, in IO-Link mode.

rule-ew-iol-002 = { $channel }: { $form ->
        [window] the device's { $have } { $dir } bits exceed the port window of { $room } bits; use a master with a larger window
       *[cable] the cable { $cable } is { $metres } m long, more than the { $max } m an IO-Link port allows; shorten it
    }
    .title = IO-Link limits
    .hint = Keep each IO-Link device within its port's window and cable length.

## EW-POT: potentials

rule-ew-pot-001 = { $form ->
        [flow] { $device } carries { $from } through to { $to }: a contact or a fuse passes one potential on, it cannot change it into another. Correct the type on one side, or put a power supply or a transformer between them.
       *[attrs] { $conflicts }{ $more ->
            [none] {""}
           *[other] {" "}{ $more } more connection points are wired the same way.
        }
    }
    .title = Type or voltage conflict
    .hint = Take away the wire that joins two different potentials, or correct the definition of one of them. A 0 V joined to earth some other way than by a wire drawn straight to an earth point needs its bond declared (Draw ▸ Connect ▸ Bond).

pot-conflict-type = { $p0 } ({ $p1 }) is wired to { $p2 } ({ $p3 }){ $p4 }: a wire joins two different potentials. If one of them is typed wrongly, correct its type; otherwise the wire goes to the wrong place: move it or take it away.
pot-conflict-voltage = { $p0 } ({ DECIMAL($p1) }) is wired to { $p2 } ({ DECIMAL($p3) }){ $p4 }: one conductor cannot carry two voltages. If one voltage is typed wrongly, correct it; otherwise the wire goes to the wrong place: move it or take it away.
pot-conflict-environment = { $p0 } ({ $p1 }) is wired to { $p2 } ({ $p3 }){ $p4 }: one conductor cannot be both. If one setting is wrong, correct it; otherwise the wire goes to the wrong place: move it or take it away.
pot-conflict-class = { $p0 } ({ $p1 }) is wired to { $p2 } ({ $p3 }){ $p4 }: one conductor cannot be in two circuit classes. If one class is wrong, correct it; otherwise the wire goes to the wrong place: move it or take it away.
pot-conflict-role = { $p0 } ({ $p1 }) is wired to { $p2 } ({ $p3 }){ $p4 }: a wire joins two different potentials. If the pin is wired to the wrong conductor, move its wire; if { $p2 } is typed wrongly, correct its type.
pot-conflict-roles = { $p0 } ({ $p1 }) is wired to { $p2 } ({ $p3 }){ $p4 }: a wire joins two different potentials, so one of the two pins is wired to the wrong conductor. Move that wire to the right one, or take it away.
pot-conflict-earth = { $p0 } (0 V) is joined to the protective earth { $p2 }{ $p4 }, but not by a wire drawn from the 0 V straight to an earth point. Select the wire that joins them and choose Draw ▸ Connect ▸ Bond, or take that wire away.
pot-conflict-on-page = {" "}on page { $p0 }

rule-ew-pot-002 = { $form ->
        [selv] { $potential } is a SELV circuit, but its 0 V is bonded to protective earth{ $bonds ->
            [one] {""}
           *[some] {" "}in { $count } places
        }. A SELV circuit is never earthed: take the bond away, or make the circuit PELV.
       *[pelv] { $potential } is a PELV circuit{ $bonds ->
            [none] , but its 0 V is not bonded to protective earth. Draw one wire from the 0 V to a protective earth symbol (PE) or to the PE rail; if the circuit is not meant to be earthed, make it SELV.
            [functional] , but its 0 V goes to functional earth (FE) only, which is no protective bond. Draw the wire from the 0 V to a protective earth symbol (PE) or to the PE rail instead.
            [joined] , but its 0 V is joined to protective earth without a bond. Select the wire to earth and choose Draw ▸ Connect ▸ Bond.
           *[some] {" "}whose 0 V is bonded to protective earth in { $count } places; a PELV circuit is earthed at one point only. Keep one bond to earth and take the others away.
        }
    }
    .title = PELV or SELV bonding
    .hint = Bond the 0 V of a PELV circuit to protective earth at exactly one point (a wire from the 0 V to an earth point), and the 0 V of a SELV circuit nowhere.

rule-ew-pot-003 = { $cp_shared ->
        [yes] { $cp_tag } pin { $cp_pin } ({ NUMBER($cp_nth, type: "ordinal") ->
            [one] { $cp_nth }st
            [two] { $cp_nth }nd
            [few] { $cp_nth }rd
           *[other] { $cp_nth }th
        })
       *[no] { $cp }
    } is a functional earth (FE) pin and is wired to the protective earth { $potential }. Wire it to functional earth, or, where it should be earthed, select its wire and choose Draw ▸ Connect ▸ Bond (FE–PE).
    .title = FE pin on PE
    .hint = Wire the pin to functional earth, or declare the FE–PE bond on its wire.

rule-ew-mfg-004 = The marker of { $wire } at { $at } does not fit its marker part: { $text } needs { $form ->
        [chars] { $needs } characters on a line of { $limit }
       *[lines] { $needs } lines where it has { $limit }
    }
    .title = Marker text does not fit
    .hint = Choose a marker part of the stock with longer or more lines, set the end's marker by hand, or mark the wire by its number.

rule-ew-mfg-008 = The end of { $wire } at { $at } cannot be made: { $reason ->
       *[fine_into_idc] a fine-stranded wire into an insulation-displacement clamp
    }
    .title = End cannot be made
    .hint = Give the wire a part of a solid or stranded wire, or land it on a clamp that takes fine strands.

rule-ew-mfg-009 = The end of { $wire } at { $at } set by hand does not fit: { $form ->
        [size] a ferrule for { $range } mm² on a { $csa } mm² wire
        [twin] a twin ferrule with no second wire
       *[colour] a ferrule of colour { $colour } where the stock's for it is { $stock }
    }
    .title = End set by hand does not fit
    .hint = Reset the end to the rule, choose a part of the wire's size, or set the other wire on the clamp to a twin ferrule as well.

rule-ew-mfg-022 = Wire { $wire } is cut from { $part }, whose { $form ->
        [size] size is not the wire's { DECIMAL($csa) } mm²
        [colour] colour is not the wire's { $colour }
       *[both] size and colour are not the wire's { DECIMAL($csa) } mm² { $colour }
    }
    .title = Wire part does not match the wire
    .hint = Give the wire a part of its size and colour, or leave its part empty so that it takes the first one of the workshop stock; the lists print the wire's own size and colour.

rule-ew-pot-004 = Wire { $wire } is green-yellow but carries { $potential }{ $typed ->
        [yes] {" "}({ $type })
       *[no] {""}
    }: green-yellow is for protective earth only. If the colour was set by hand on the wire, clear it there; otherwise give the wire another colour in its Properties.
    .title = GNYE on a non-PE potential
    .hint = Use green-yellow only for protective conductors; give the wire another colour.

rule-ew-pot-005 = Wire { $wire } is blue but carries { $potential }{ $type ->
        [none] {""}
       *[other] {" "}({ $type })
    }: in a project with a neutral, blue marks the neutral. If the colour was set by hand on the wire, clear it there; otherwise give the wire another colour, or use a cable whose blue core carries the neutral.
    .title = BU on a non-N conductor
    .hint = Use another colour for the wire, or a cable whose blue core carries the neutral.

rule-ew-pot-006 = PE wire { $wire } has { DECIMAL($pe) } mm², below the { DECIMAL($required) } mm² required for the { DECIMAL($line) } mm² line conductors at { $at }{ $verified ->
        [yes] {""}
       *[no] {" "}(the sizing table is not verified against the standard)
    }. Make it at least { DECIMAL($required) } mm² in its Properties.
    .title = PE undersized
    .hint = Increase the protective conductor's cross-section to the required size.

rule-ew-pot-008 = { $form ->
        [terminal] { $terminal } is bonded to the rail but has no location, and the project has { $locations } own locations; the terminal joins no location's PE node
       *[island] The protective earth at { $at } on page { $page } reaches neither the incoming PE nor an earth symbol{ $location ->
            [main] {""}
           *[node] {" "}of { $node }
        }. Connect it to the PE rail or to an earth symbol, or put its PE terminal on a bonded rail.
    }
    .title = PE island
    .hint = Every protective earth must reach the incoming PE: connect each PE conductor to the PE rail or to an earth symbol (an earth symbol stands for the cabinet's earth), or put its PE terminals on a bonded rail.

rule-ew-pot-009 = { $cp_shared ->
        [yes] { $cp_tag } pin { $cp_pin } ({ NUMBER($cp_nth, type: "ordinal") ->
            [one] { $cp_nth }st
            [two] { $cp_nth }nd
            [few] { $cp_nth }rd
           *[other] { $cp_nth }th
        })
       *[no] { $cp }
    } has no wire{ $unused ->
        [yes] {" "}(marked not used)
       *[no] { $near ->
            [yes] , and the mark { $mark } stands right beside it without joining it. Join the two with the quick fix, or draw a wire to it.
           *[no] . Draw one to it, or right-click it and choose Mark not used.
        }
    }
    .title = Unconnected CP
    .hint = Open the message: its page opens with the device selected. Draw a wire to the connection point, or right-click it and choose Mark not used when it stays free on purpose.

rule-ew-pot-010 = { $form ->
        [size] { $cp } takes { $range } mm²; { $wire } of { $csa } mm² lands on it
       *[count] { $cp } takes { $max ->
            [one] 1 conductor
           *[other] { $max } conductors
        }; { $count } land on it
    }
    .title = Clamp capacity exceeded
    .hint = Land a wire on another clamp, give two wires of one size a twin ferrule where the clamp takes one, or land a wire of another size on a clamp made for it.

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
        [ring] The wire { $wire } closes a ring: its wires lead back to where they started, so it is left out of the wire list. Take one wire of the ring away, or confirm that the ring is meant.
       *[other] A wire closes a ring in the drawing and is left out of the wire list. Take one wire of the ring away, or confirm that the ring is meant.
    }
    .title = Wires in a ring
    .hint = Remove the wire that closes the ring, or confirm that the ring is intended.

rule-ew-pot-013 = { $device } { $form ->
        [isolating] is an N-isolating terminal on { $from } → { $to }: right as drawn, the neutral is opened only to test it.
       *[switching] opens the neutral { $from } → { $to } without the phase{ $loads ->
            [one] {" "}(load { $load } stays connected to { $line })
            [many] {" "}({ $stranded })
           *[none] {""}
        }: what is behind it stays live while it looks switched off. Put the contact in the phase, or use a device that opens phase and neutral together, such as a two-pole breaker.
    }
    .title = N switching
    .hint = Move the contact into the line conductor, or use a device that opens L and N together.

rule-ew-pot-014 = { $form ->
        [conductor] A section of wire { $wire } is drawn on another cable core; the wire keeps the core it starts on. Draw the whole wire on one core.
        [override] A section of wire { $wire } sets a value the wire already has from another section; the first value applies. Set it in one place.
       *[other] A section of a physical wire conflicts with another section of it. Give the wire one set of properties.
    }
    .title = Physical-wire conflict
    .hint = Give the physical wire one set of properties, or split it into separate wires.

rule-ew-pot-015 = Wire number { $number } is printed on { $count } wires that are not one conductor
    .title = Wire-number conflict
    .hint = Give each wire its own number, or join the wires if they are one conductor.

rule-ew-pot-016 = Wire { $wire } has { $member ->
        [csa] its cross-section { DECIMAL($value) }
        [colour] its colour { $value }
        [wire_part] its wire part
        [insulation] its insulation
        [function] its wire function
        [shielded] shielded
       *[twisted] twisted
    } set by hand, which it gets anyway. Remove that setting.
    .title = Redundant override
    .hint = Remove the override; it sets the value the wire takes anyway.

rule-ew-pot-017 = { $first } has more than one name: { $names } are wired together. Give them one name, or take away the wire that joins them.
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

rule-ew-pot-022 = Wire { $wire } has its cross-section { DECIMAL($size) } set by hand, below the { DECIMAL($needed) } that the { DECIMAL($current) } A rating of { $device } needs by the current table (IEC 60204-1 Table 6, method B2). Take the proposed size.
    .title = Cross-section below its protection
    .hint = Take the proposed size, or a larger one, or protect the wire by a device of a lower rating.

## EW-RDS (EW-IEC81346): reference designations

rule-ew-rds-001 = { $device } does not parse as an IEC 81346 designation: { $reason ->
        [empty] empty reference designation
        [prefix] missing prefix sign (such as '-' or '+')
        [unexpected] unexpected character { $char } (U+{ $code })
        [empty-level] prefix sign or separator without a level
        [invalid] character { $char } (U+{ $code }) not allowed here
        [whitespace] white space inside the designation
        [order] aspects out of canonical order
        [repeated] aspect appears more than once
        [empty-cp] empty connection point after ':'
        [cp-without-product] connection point without a product designation
        [empty-signal] empty signal designation after ';'
       *[other] { $detail }
    }
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

rule-ew-rds-007 = { $device }: the letter code { $letter } uses reserved letters: { $io_form ->
        [none] {""}
       *[other] { $io } is not used in letter codes
    }{ $class_form ->
        [after] ; the identifier set { $set } reserves the class letter { $class }
        [alone] the identifier set { $set } reserves the class letter { $class }
       *[none] {""}
    }{ $other ->
        [yes] ; the identifier set reserves it
       *[no] {""}
    }
    .title = Reserved letters
    .hint = Avoid the letters I and O in a letter code; choose another letter.

rule-ew-rds-009 = the { $kind ->
        [letter] letter code
        [frozen] frozen tag
       *[designation] terminal designation
    } { $text } of { $device } contains non-Latin characters that look like Latin ones: { $chars }; search and cross-references cannot find it; retype it in Latin letters
    .title = Confusable character
    .hint = Replace the character by its plain Latin form.

rule-ew-rds-010 = the structure codes { $codes } differ only in separators or case; merge them into one node and keep the other spellings as aliases
    .title = Structure-code variants
    .hint = Merge the structure nodes whose codes differ only in spelling.

## EW-REL: the release gate

rule-ew-rel-001 = revision { $revision }: { $form ->
        [author] author not set; set it with Check ▸ Revision ▸ Set revision author…
        [checker] checker not set; the checker approves the revision
        [four-eyes] the checker is the author ({ $role ->
            [customer] customer
            [end_user] end user
            [panel_builder] panel builder
            [author] author
            [checker] checker
            [engineer] engineer
           *[other] { $role }
        }); four eyes need another party
        [sign-off] sign-off missing; the checker approves the saved revision
       *[stale] the sign-off does not match the saved content (edited after approval); approve it again
    }
    .title = Four eyes
    .hint = Set the author, then let another party approve the saved revision; approve again after a content edit.

rule-ew-rel-004 = revision { $revision }: { $form ->
        [format] the label is not in the project's revision label format { $pattern }
       *[order] the open revision's label does not follow { $later }; revision labels increase, the open revision last
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
            [untagged] the strip
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

rule-ew-term-005 = { $terminal } is bonded to the rail but has no PE connection point; the terminal joins no location's PE node
    .title = Rail bonding on a non-PE terminal
    .hint = Clear the rail bonding, or make the terminal a PE terminal.

rule-ew-term-006 = { $terminal } lies on wire { $wire } without joining it; connect it to the wire or move it
    .title = Terminal on a wire it does not join
    .hint = Connect the terminal into the wire, or move it off the wire.

## collisions after a merge

rule-ew-core-013 = { $collision } after a merge; { $keeper ->
        [frozen] the record with the frozen tag
       *[oldest] the oldest record
    } keeps it, and { $others ->
        [many] the others
       *[one] the other
    } must change
    .title = Collision after a merge
    .hint = Renumber the newer device with the quick fix, or give one of the records another number.

## EW-IMP: the import findings of the migration report

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

rule-ew-imp-010 = { $record } was removed in { $file }; it is kept here. Delete it if it is no longer needed.
    .title = Removed in the source, kept here
    .hint = Delete the record if it is no longer needed; otherwise keep it.

rule-ew-imp-011 = { $record } was changed both in { $file } and here; this project's version is kept.
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

# The quick fixes' labels: the window's text of a fix by its key (`QuickFix::key`); the en-GB
# text is the fix's English label, which the CLI and the JSON print.
fix-take-proposed-size = Take the proposed size
fix-drop-override = Drop override
fix-variant-own = Use the part's own number
fix-next-free-address = Give it the next free address, { $p0 }
fix-anchor-graphics-centre = Anchor at the graphics centre
fix-renumber-newer = Renumber the newer
fix-unfreeze-tag = Unfreeze the tag
fix-realise-primary-part = Realise the primary part
fix-rebind-map-channel = Rebind to the map's channel
fix-declare-pelv-bond = Declare 0 V–PE bond (PELV)
fix-mark-cp-unused = Mark not used
fix-reset-end = Reset to the rule
fix-join-mark = Join the pin and the mark
fix-plc-take-out = Take it out of the rack
fix-plc-bind-pins = Bind the channels to their pins
fix-blank-position = Blanking plate on position { $p0 }
fix-draw-valves = Draw its valves in the valve terminal
fix-plug-open-ports = Plug the open ports
fix-text-style-minimum = Set to 1.2 mm

# The words a rule prints where a record has no name of its own: a rule passes them to its pattern by key,
# and each language prints its own.
word-cable = cable
word-without-a-tag = without a tag
word-assembly = assembly
word-a-location = a location
word-a-device-without-designation = a device without designation
word-a-missing-part = a missing part
word-an-unknown-profile = an unknown profile
word-a-connector = a connector
# A scope of EW-CORE-083 by its name.
word-scope-folio = folio { $p0 }
word-scope-potential = potential { $p0 }
word-scope-configuration-project = configuration project { $p0 }
word-scope-logical-net = logical net { $p0 }
word-scope-physical-net = physical net { $p0 }
word-scope-of-kind = a { $p0 } scope
# A load a contact leaves connected (EW-POT-013), and a character that looks like a Latin one (EW-RDS-009).
word-stranded-load = load { $p0 } stays connected to { $p1 }
word-lookalike = { $p0 } (looks like { $p1 })
word-none = none
word-terminal = terminal
word-strip = strip
word-another-strip = another strip
word-valve = valve
word-the-valve = the valve
word-a-module = a module
word-a-role-of-a-newer-format = a role of a newer format
word-a-drawn-page = a drawn page
word-a-section-divider = a section divider
word-a-report-page = a report page
word-the-strip = the strip
word-a-device-that-is-not-a-terminal = a device that is not a terminal
word-the-project = the project
word-a-logical-bus-net = a logical bus net
word-a-physical-bus-net = a physical bus net
word-a-connection-point-of-another-page = a connection point of another page
word-its-target = its target
word-the-station = the station
word-a-station = a station
word-an-f-module = an F-module
word-the-sensor = the sensor
word-the-cable = the cable
word-the-device = the device
word-current-newer = a current of a newer format
word-earthing-floating = floating
word-earthing-newer = an earthing of a newer format
# The text styles by the names the Text styles page lists them under (`messages::text_style_name`).

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
    }{ $date ->
        [none] {""}
       *[other] {" "}of { $date }
    }: { $count } object(s) differ ({ $objects }{ $more ->
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

rule-ew-imp-206 =
    .title = Report column not mapped
    .hint = Map the column in the report's column mapper and import the report again, or leave it out on purpose.

rule-ew-imp-207 =
    .title = Multi-level terminals folded
    .hint = Check the terminal's levels against the report.

## Words another part of the program wrote: the store's integrity findings, the text an importer stored, an importer's quick fix

words-as-written = { $p0 }
# A finding's subject in words (what the message is about), rendered where the message is.
subject-waiver = waiver { $p0 }
subject-ip = interruption point { $p0 }
subject-tbd = TBD { $p0 }
subject-wire = wire { $p0 }
subject-toc = table of contents
subject-folio-style = folio { $p0 }: { $p1 }
subject-pair = { $p0 } { $p1 }
subject-revision = revision { $p0 }
subject-cp = { $p0 }:{ $p1 }
subject-bridge-group = { $p0 } bridge group { $p1 }
subject-rail-at = the rail at { $p0 }
subject-duct-at = the duct at { $p0 }
subject-carrier-at = the carrier at { $p0 }
subject-conflict-target = { $target ->
        [project] the project
        [device] device { $tag }
        [page] folio { $page }
        [function] a function of device { $tag }
        [connection-on] connection from { $from } to { $to } on folio { $page }
        [connection] connection from { $from } to { $to }
       *[other] { $kind }
    }
subject-conflict-file = { $file ->
        [page] the file of folio { $name }
        [layout] the layout file of { $name }
        [a-layout] a layout file
        [a-page] a page file
        [import] an import's file
       *[path] { $name }
    }

## The words that join the items of a list in a finding

join-comma = {", "}
join-and = {" and "}
join-wire = –
join-connection = {" → "}
join-sentence = {" "}
join-semicolon = {"; "}
# Between two reserved letters I and O (EW-RDS-007): each is its own clause in English.
join-reserved-letters = {" is not used in letter codes; "}
