1. Verdict per folio
101: nearly complete. reps 17 / connections 21 / junctions 2, all as on the sheet; J1 and J2 are dot/star with the right sequences. Wrong: the 1X1 clamp sides, two markers and the parts.
102: nearly complete. reps 12 / connections 19 / junctions 4 as on the sheet; J3/J4 bevel chain and J5/J6 bevel star with the right sequences. Wrong: marker colours and the parts of -102F1.
103: incomplete. reps 35 / connections 20 / junctions 0, 15 terminals on 2 strips as on the sheet, but none of the bridges BR1, BR5 and BR6 is drawn (the strip.bridge step was refused and marked broken).
2. Misses against the task sheet
- 103: BR1 (-X1031:1-4), BR5 (-X1031:11-14) and BR6 (-X1032:1-4) are missing. Because of this, -X1031:2/3/4 and 12/13/14 and -X1032:4 each carry their own potential (-X1031:2:2 and so on) and not 24VDC-1, 0V or PE. As a result 24VDC-AIR2..4, 0VDC-AIR2..4 and PE-1.4 are fed by nothing.
- 101: 1X1 field and internal are reversed. The incoming J-E-206 wires land on /a and S51 and -XPE on /b. The score expects internal = a. The sheet only says field and internal, so check the convention, but the page shows the feed on a.
- 101: 1X1:2 is at D3 and 1X1:PE at D4. The sheet gives the strip at 101-D2, so this is acceptable as a strip laid out to the right.
- Parts: only -102G1 and -102F2 carry parts. S51, -101F1, F55, T55, E55, -102F1, both 1X1 terminal types, PT 2.5-TWIN(-PE), end brackets and bridges are all missing from the BOM. The BOM also lists 2904602 with quantity 2 and tags "-?G1, -102G1": a phantom untagged copy of the PSU.
- Markers: 101 prints "2.5 BK BK" where the sheet wants BK BU (the pole-2/N wire is not blue). 102 prints "1.5 BU GNYE GNYE" and "1.5 BK BK GNYE GNYE" where it wants "1.5 DBU DBU" and "1.5 DBU DBU WH WH". The 0V wires take PE colours because 0V and PE merged into the potential PE-1.2. The two 101 markers both sit at cell B2, and from text I cannot tell above from below.
- 101: wire 1X1:1/b -> S51:2 is still on a provisional route (0 bends) and was never stored.
- The external device JE-N1000-03 and the location +J-E-206: the external points exist, but their printed text cannot be seen in the page text.
3. Findings worth fixing
Fix:
- EW-POT-008 on -X1032:1..4: the rail-bonded terminals have no location. The PE strip joins no PE node.
- EW-POT-001 on PE-1.2: 0V and PE are merged with mixed roles. The bond is intended (PE-0VDC), but the role conflict drives the wrong colours and the misleading potential name. Make the bond explicit.
- EW-CORE-051 on 24VDC-AIR1..4, 0VDC-AIR1..4, PE-AIR1/2, PE-1.3/1.4: expected, because their partner folio 106 is not drawn in this job.
- EW-CORE-052 on L-1, N-1, 0VDC, PE-0VDC: two source points. The sheet marks both ends ▷, so this follows from the sheet, but a reviewer should get the sheet changed or choose a star source.
Not worth fixing: EW-NUM-001 infos on the frozen tags (S51, F55, T55, E55, E56, S56, 1X1, XPE). The one on -102F2 is odd, because the sheet does not freeze it: check why it is flagged frozen.
4. What a reviewer would mark up
- Add the three plug-in bridges on 103. Without them the drawing tells the builder that terminals 2-4 and 12-14 are dead.
- Put the 1X1 feed on the field side; the strip is drawn feed-on-a.
- On 101, both 2.5 markers sit in cell B2; check that one is above and one below S51.
- On 102 the four IPs in cell D8/C15 run long horizontal wires across the page (bends 4-5 to x ~198). Routes there may cross; this cannot be checked from text.
- 24VDC-1..4 IPs carry no direction arrows; the sheet gives none either, but the reading would be clearer with ▷/◁.
- The fan-out of the 103-B6/B14 IPs from row A terminals (6-bend routes on 3/b and 13/b) likely makes the wires cross.
- Assign the parts: the BOM is unusable for ordering.
5. Confidence
- Counts, connections, junctions and tags: high (page text plus score H1-H3, S3-S6).
- Missing bridges: high (potentials plus the broken step).
- 1X1 side reversal: medium (the sheet's a/b convention is inferred from the score).
- Markers: high on the text, low on the placement.
- Layout, crossings, external point text and PDF appearance: not checkable from text.
