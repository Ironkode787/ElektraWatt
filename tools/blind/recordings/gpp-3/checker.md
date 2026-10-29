1. Verdict per folio
101: nearly complete. reps 17 / connections 21 / junctions 2, as the sheet. But strip 1X1 is tagged -X1011, its terminals are spread over D2, D3, D4 and not stacked at D2, and J-E-206-PE sits at 101-G1, not G0.
102: nearly complete. reps 12 / connections 19 / junctions 4, as the sheet. J4 is wrong: a wire runs -102F2:24VDC to -102F2:24VDC, and -102F1:2 to -102F2:24VDC is missing. The J4 sequence is -102F1:2 | 24VDC#1 | 24VDC#2; the sheet wants 24VDC#1 | -102F1:2 | 24VDC#2.
103: nearly complete. reps 35 / connections 20 / junctions 0 / terminals 15 / strips 2, as the sheet. Five IPs are one cell off (see 2).
2. Misses against the task sheet
- 101 1X1 (frozen): typed as -X1011 (tags -X1011:1/2/PE), and not stacked at 101-D2 (they sit at D2, D3, D4).
- 101 J-E-206-PE: at G1, the sheet says G0.
- 102 J4: the -102F1:2 feed into the junction is lost (24VDC#1 is wired to 24VDC#2 instead), and the far-end order is wrong.
- 103 IPs one cell off: 24VDC-AIR4 at B7 (sheet B6), 0VDC-AIR4 at B15 (B14), PE-1.3 and PE-1.4 at G7 (G6), PE-AIR2 at G2 (G1).
- Markers: 0 of the 4 hand-set cross-section markers print (101 below/above S51 at 2.5; 102 below -102G1 and above -102F2 at 1.5). Either the overrides were not set or marker mode is wrong.
- Parts list: 10 rows, 14 expected. The 1X1 terminals UK 5 N x2 and USLKG 5 x1, the end brackets and the plug-in bridges are missing. The score marks this as a known product gap (WP-S2.07).
- Potentials and the bond B1: I could not confirm these. The check names potentials such as -102G1:+ and -X1011:PE:1, which suggests several sheet names (+24V, PE) were not given.
3. Findings worth fixing
- EW-POT-008 (two PE potentials, -X1011:PE:1 and PE-1.1): fix. Name PE and join it.
- EW-POT-001 on PE-1.1 (roles M meet Pe): fix. This points to the 0V/PE bond being set up wrong or not at all.
- EW-POT-005 on 102.1 and 102.4 (BU on the +24V potential): fix. The potential is unnamed or untyped, so the colour profile goes wrong.
- EW-POT-013 on F55, S56, T55 and -101F1 (switches the neutral): expected, because the design switches N on purpose. The -101F1 one is extra against the oracle; it may come from the N-1 naming.
- EW-CORE-051 x12 (IPs without a partner, to folio 106): expected, because 106 is not drawn.
- EW-POT-009 on -102F2:13/14: expected; mark them unused.
- EW-NUM-001 infos on the frozen tags: expected. The ones on -102G1 and -102F2 are extra; those tags should not be frozen.
4. What a reviewer would mark up
- The incoming strip tag -X1011 instead of 1X1 breaks the customer's numbering.
- Terminals spread down column D instead of stacked at D2.
- IPs at cells next to the agreed grid break the reading order and the cross-reference alignment.
- No cross-section markers, so the 2.5 and 1.5 mm² wires look the same as the default ones.
- J4 wiring makes it look as if -102F1 does not feed -102F2, which is wrong to build from.
5. Confidence
High on the counts, tags, cells, J4 and the markers: they come from the harness score and the check report. Medium on the potentials and the bond, which I judged from the check findings only; I did not read every potential line. I could not check the PDF's appearance or the label legibility from text.
