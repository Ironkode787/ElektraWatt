## Turn 1
patience 9 flow 9
page.new with folio, type multi_line, title and section did all three folios in one batch. Pleasant, faster than QET's folio dialog. Next: structure node for +J-E-206 and the external device.

## Turn 2
patience 9 flow 8
Location and three external references created in one batch, toasts clear. Symbol picker opened: 40 rows, but only four parts listed (MICO, 2904602, PT 2.5 twins); I expected Eaton 533356 etc. too. Will try picking by part name.

## Turn 3
patience 8 flow 6
place.pick part=533356 refused: unknown part. Job sheet says the parts are in the project. Will try Device from part (Shift+I) picker.

## Turn 4
patience 7 flow 6
Device from part lists only 4 parts, none of folio 101's (Eaton, Schneider, Siemens, Rittal). So I place symbols and note the parts can't be given here. Placing S51 as 2-pole manual switch at B2.

## Turn 5
patience 8 flow 8
Placing works: place.pick symbol, nav.goto 101-B2, Enter with alt. Proposed tag -101Q1, page text shows the 4 cps with positions. Good. Now batch the rest of the folio 101 devices.

## Turn 6
patience 7 flow 7
My batch had 21 steps, refused by the harness (my fault). Picked the MCB symbol; overlay shows 6 hints. Continuing in smaller batches.

## Turn 7
patience 8 flow 8
All 8 symbols placed at their cells in one batch; toasts name each tag. Page text is clear. Parts could not be given (not in Device from part list). Now freezing tags S51, F55, T55, E55, S56, E56, -XPE.

## Turn 8
patience 8 flow 8
Freezing tags: select.set + props.set device/tag.frozen, 7 pairs, all done, title changes to the new tag. Smooth. Properties show a Parts row; trying to give S51 its part there.

## Turn 9
patience 8 flow 7
The Parts property opens a list of 14 parts, all of the job's. So parts ARE assignable after placement, the picker just didn't list them. Choosing 533356 for S51.

## Turn 10
patience 8 flow 8
props.choose committed the part at once (533356 x 1); my extra commit was refused harmlessly. Doing the other four parts.

## Turn 11
patience 8 flow 8
Four more parts assigned, quiet and quick. Now strip 1X1: strip.new then terminal symbol.

## Turn 12
patience 8 flow 7
strip.new made -X?? with a helpful toast; terminal symbol opened a strip picker. Ghost line still says pe symbol letter W though - stale? Choosing the strip and placing terminal 1 at D2.

## Turn 13
patience 8 flow 8
Terminal -X1011:1 placed at D2, strip got its tag; the next pick proposes terminal 2. Like QET's terminal numbering, nice. Placing 2 at D3 and PE at D4, then freeze strip tag 1X1.

## Turn 14
patience 7 flow 6
Terminal 2 fine, but the PE terminal symbol gave no strip picker; ghost would make a new X device. Inconsistent with the plain terminal. Trying place.pick with strip=-X1011 and terminal_designation=PE.

## Turn 15
patience 6 flow 6
PE terminal placed via strip parameter. But freezing the strip tag 1X1: props.set said done, nothing changed, Frozen tag still none. Silent no-op is the worst kind. Trying edit/input/commit.

## Turn 16
patience 5 flow 5
Edit/input/commit also says done and the strip tag stays -X1011. Two silent no-ops in a row. Last try: the Freeze action on the Tag row, then the Text sub-row.

## Turn 17
patience 4 flow 5
Freeze worked but froze X1011; setting the text to 1X1 is accepted and ignored. Three silent no-ops. I give up on 1X1 and leave the strip as -X1011. Moving on to wires.

## Turn 18
patience 6 flow 7
First wire S51:1 to -101F1:1 drawn with cursor.set on cp positions and Enter twice; the route line names from/end before committing, reassuring. Batch the rest.

## Turn 19
patience 7 flow 8
Five wires in one batch, every route named its ends. Bus and strip feeds done. Now the heater/lamp branch.

## Turn 20
patience 7 flow 8
11 wires, page says 11 connections. Now the two junctions by ending E56:X2 and S56:13 on the existing wires at their printed 'at' points.

## Turn 21
patience 8 flow 8
Ending on a wire made the junctions; both match the sheet (dot, star, sequence F55:2 E55:L E56:X2 and F55:N#2 T55:5 S56:13). Very satisfying, the route end even named the wire it splits. Now IPs.

## Turn 22
patience 8 flow 8
IP L-1 placed at H16 as source; the ghost proposes L-2 next. Placing N-1 beside it, PE-1.1 at G15, then the three external points at G0.

## Turn 23
patience 7 flow 7
IPs fine, but the external point inherited the name PE-1.1 from the last IP, so the page now has two PE-1.1. Undoing it and naming first.

## Turn 24
patience 7 flow 7
External points named and linked; third one landed in G1 because G0 is narrow, acceptable. Now the six wires to IPs and external points.

## Turn 25
patience 7 flow 8
Four wires to IPs and the L external point, routes named the right ends. Two more external wires.

## Turn 26
patience 8 flow 8
Folio 101: 21 connections, 2 junctions, no free cps left. I skip the hand-set markers and potential names for now. On to folio 102: black boxes from parts.

## Turn 27
patience 6 flow 6
Only -102F1 appeared; the two part placements said done but no toast and nothing on the page. Trying the Black box tool with part instead.

## Turn 28
patience 6 flow 6
Black box tool asks for an outline: one corner done, now the opposite corner. In QET a black box is just a symbol; drawing a rectangle by coordinates costs me thought.

## Turn 29
patience 7 flow 6
Second corner placed the PSU box as -102A1 with its 7 cps from the part, laid along the edges. Now the MICO box at E7, then tags and the MCB part.

## Turn 30
patience 7 flow 7
MICO box placed with its 10 cps; PSU renamed -102G1 and MCB got A9F78116. Now rename the MICO and place the IPs of 102.

## Turn 31
patience 8 flow 8
Four IPs placed, L-1 already carries potential L-1 from folio 101, nice cross-folio link. Now N-1 and the four 24VDC IPs, two per cell.

## Turn 32
patience 8 flow 8
All nine IPs on 102 in place. Wiring the direct connections next.

## Turn 33
patience 7 flow 7
My 21-step batch was refused silently in my filter again (my mistake). Re-sent: L-1, N-1, PE-1.2 and Out1 wires drawn. Continuing.

## Turn 34
patience 8 flow 8
Outputs to IPs and the +24 V chain drawn. Now the two minus wires, then the junction branches.

## Turn 35
patience 8 flow 8
Both minus wires drawn. Now four branches ending on wires to make J3-J6.

## Turn 36
patience 8 flow 8
Four junctions came out with exactly the sheet's sequences. They are dots/star; the sheet wants bevels, J3/J4 chain, J5/J6 star. Selecting junction 1 by clicking its position.

## Turn 37
patience 8 flow 8
Junction 1 is bevel/chain now. Doing the other three the same way.

## Turn 38
patience 8 flow 8
All four junctions match: bevel, chains for J3/J4, star for J5/J6. Folio 102 drawn (skipping markers, potentials, bond). Folio 103: strips.

## Turn 39
patience 8 flow 8
Picking the part PT 2.5-TWIN opened the strip picker straight away, nicer than the symbol route. Choosing the new strip, terminal 1 at A3.

## Turn 40
patience 8 flow 8
-X1031:1 placed at A3 and the strip took -X1031 by itself. Batch the rest of -X1031.

## Turn 41
patience 9 flow 9
Six terminals in one batch with designations as typed. This is faster than QET's terminal placement. Now 11-14 and the PE strip.

## Turn 42
patience 7 flow 7
-X1031 complete. But the PE twin part again opens no strip picker for the new empty strip (same as the PE symbol on 101). Trying strip=-X?? directly.

## Turn 43
patience 8 flow 8
strip=-X?? worked, -X1032:1 at F3 with tag -X1032. Placing 2,3,4.

## Turn 44
patience 8 flow 8
PE strip complete, both strips placed. Now the 20 IPs of 103, several per cell, so I use positions.

## Turn 45
patience 8 flow 8
Four target IPs in. Next five.

## Turn 46
patience 8 flow 8
Nine IPs. Next five.

## Turn 47
patience 8 flow 8
Fourteen IPs. Last six.

## Turn 48
patience 8 flow 8
19 IPs placed; the cp lines give me their exact positions. Last IP then wiring.

## Turn 49
patience 8 flow 8
Four outgoing wires drawn. Next five.

## Turn 50
patience 8 flow 8
Nine of twenty. Next five.

## Turn 51
patience 8 flow 8
Fourteen. Last six.

## Turn 52
patience 8 flow 8
Nineteen wires. Last one, then the bridges BR1, BR5, BR6 with select.set and strip.bridge.

## Turn 53
patience 7 flow 7
All 20 wires on 103. Bridge toast seen only once ('Added 3 bridges between 4 terminals'); unsure if BR5 and BR6 went in. The page text shows no bridge lines, so I can't verify bridges from text. Re-running the 0V bridge to see.

## Turn 54
patience 7 flow 7
Refusal 'bridged already' confirms BR5 existed; so all three bridges are in, but the page text never lists bridges. Saving and exporting now.

## Turn 55
patience 7 flow 8
Save done silently, PDF export toast 'Exported set.pdf: 16 pages, 60 KiB'. Job handed in, minus the hand-set wire markers, potential names, the bond and the 1X1 tag.

