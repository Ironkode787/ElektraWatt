# Job sheet J-E-2416: the power supply folios

You are a draftsman on the control cabinet J-E-2416. A colleague has left you this job sheet: the folios of the
schematic below, to draw in the new project that is already open, and the drawing set to hand in as a saved project
and a PDF. Everything you need is on this sheet; draw what it says, where it says it.

Run `{run}` is a `{kind}` run with the caps {turns} turns, {steps} steps, {looks} looks and {refusals} refused steps.

## The project as you find it

The project was made with the company standard: frame QET-compatible A4 landscape with 17 columns 0 to 16 and 8 rows
A to H, sections 1 "Wiring Definition" (100-199), 2 "Pneumatics" (200-299) and 9 "Nomenclature" (900-999). The
parts of the devices below are already in the project as copies. None of the folios below exists yet.

## The job, in order

1. **Folios.** Create each folio below as a multi-line schematic in section 1, with its title.
2. **Each folio:** place the devices at the cells given, type the tags as written, give each device its part where EW
   lets you (if it does not, say so in your note and go on), then draw the connections and junctions. Draw a wire
   from end to end as the table says; where a table names an interruption point (IP), draw the wire to the IP on
   this folio and place the IP at the cell given. An IP has a name; the potential continues on the folio where its
   other end is. A folio of the schematic that is not below is not drawn: the IPs stand for it.
3. **Look at your work** with the page description after each folio, and correct what differs from the tables.
4. **Save the project and export the PDF** (name the file `set.pdf`). Both are your own steps.

How to read the tables: the cell is the anchor cell of the symbol (folio and cell of the frame); a tag with "(frozen)"
is typed as written and stays; "field" and "internal" name the two sides of a terminal, a and b the clamps of a
terminal; the sizes and colours follow the colour and size profile of the project (DBU/WH 24 V) and are not drawn,
except the cross sections set by hand, which print as wire markers (below). In the IP tables ▷ marks an outgoing end
and ◁ an incoming one. In marker mode "only explicit overrides" exactly four marker groups print: below and above S51
on folio 101, below -102G1 and above -102F2 on folio 102. A marker prints the cross section and the colours of its
wires.

<!-- folio 101 -->
## Folio 101: Power Supply 1 - Main Current

The supply from the upstream cabinet +J-E-206 arrives on strip 1X1, passes the main switch S51 and the miniature
circuit breakers -101F1 and F55, feeds the PSU of folio 102 through the IPs L-1 and N-1, and feeds the enclosure
heater E55 and the cabinet lamp E56 through the thermostat T55 and the door switch S56 (contacts in N).

**Structure first.** The three external points of this folio print "+J-E-206 / JE-N1000-03". Create the location
+J-E-206 (the upstream cabinet; external references only) and the external device JE-N1000-03 in it with the
connections L, N and PE, so that the external points can name it.

**Devices**

| Tag | Function | Part | Cell | Connection points |
|---|---|---|---|---|
| S51 (frozen) | Main switch, 2-pole, 40 A, rotary handle | Eaton 533356 | 101-B2 | 1, 2 (pole 1); 3, 4 (pole 2); fed on 2 and 4 |
| -101F1 | MCB 1P+N C10, feeds the PSU | Schneider Electric A9P52610 | 101-B8 | 1, 2 (L pole); N#1 in, N#2 out (both printed "N") |
| F55 (frozen) | MCB 1P+N C4, feeds heater and lamp | Siemens 5SY4204-7 | 101-B12 | as -101F1 |
| T55 (frozen) | Thermostat, setpoint 15 °C, contact in N | Rittal SK 3110.000 | 101-C12 | 5, 3 |
| E55 (frozen) | Enclosure heater | Rittal SK 3102.000 | 101-E12 | L, N, PE |
| S56 (frozen) | Door switch, contact in N | none | 101-D14 | 13, 14 |
| E56 (frozen) | Cabinet lamp | none | 101-E14 | X1, X2 |
| 1X1 (frozen) | Incoming supply strip | terminals UK 5 N ×2, USLKG 5 ×1 | 101-D2 | terminals 1, 2, PE; field and internal side |
| -XPE | Main PE point (plate PE stud), drawn as earth symbol without tag | none | 101-D5 | PE |

-XPE is drawn as an earth symbol without a printed tag. The three external points J-E-206-L, J-E-206-N and J-E-206-PE
stand at 101-G0 and print "+J-E-206 / JE-N1000-03".

**Interruption points**

| Name | One end | Other end |
|---|---|---|
| L-1 | 101-H16 ▷ | 102-A1 ▷ |
| N-1 | 101-H16 ▷ | 102-A1 ▷ |
| PE-1.1 | 101-G15 ▷ | 103-G6 ◁ |

**Connections** (every wire as it is drawn on this folio)

| From | To |
|---|---|
| -101F1:1 | F55:1 |
| -101F1:2 | L-1 |
| -101F1:N#1 | F55:N#1 |
| -101F1:N#2 | N-1 |
| 1X1:1 internal | S51:2 |
| 1X1:2 internal | S51:4 |
| 1X1:PE internal | -XPE:PE |
| E55:PE | PE-1.1 |
| F55:2 | J1 |
| F55:N#2 | J2 |
| J-E-206-L | 1X1:1 field |
| J-E-206-N | 1X1:2 field |
| J-E-206-PE | 1X1:PE field |
| J1 | E55:L |
| J1 | E56:X2 |
| J2 | S56:13 |
| J2 | T55:5 |
| S51:1 | -101F1:1 |
| S51:3 | -101F1:N#1 |
| S56:14 | E56:X1 |
| T55:3 | E55:N |

**Junctions**

| Junction | Style | Wiring | Sequence of the far ends |
|---|---|---|---|
| J1 | Dot | Star | F55:2 | E55:L | E56:X2 |
| J2 | Dot | Star | F55:N#2 | T55:5 | S56:13 |

The bevels of the top bus are daisy chains on the CPs of -101F1, not junctions.

**Cross sections set by hand** (2.5 mm², printed as markers): 1X1:1 internal to S51:2, 1X1:2 internal to S51:4, S51:1 to
-101F1:1 and S51:3 to -101F1:N#1.
<!-- /folio -->

<!-- folio 102 -->
## Folio 102: Power Supply 2 - Control Current

The PSU -102G1 takes the mains from folio 101 through the IPs L-1 and N-1, feeds the miniature circuit breaker
-102F1 in the +24 V and the electronic load distribution -102F2, and passes 24 V DC, 0V and PE to folio 103 through the
IPs below.

**Devices** (-102G1 and -102F2 are black boxes from their parts)

| Tag | Function | Part | Cell | Connection points |
|---|---|---|---|---|
| -102G1 | PSU 1 AC / 24 V DC, 20 A | Phoenix Contact 2904602 | 102-B6 | L, N, PE, +#1, +#2, −#1, −#2 |
| -102F1 | MCB 1P B16 in +24 V | Schneider Electric A9F78116 | 102-C7 | 1 (alias L1), 2 (alias T1) |
| -102F2 | Electronic load distribution, 4 × 6 A | Murrelektronik MICO 4.10 | 102-E7 | 24VDC#1, #2, GND#1, #2, 13, 14, Out1 … Out4 |

**Interruption points**

| Name | One end | Other end |
|---|---|---|
| L-1 | 101-H16 ▷ | 102-A1 ▷ |
| N-1 | 101-H16 ▷ | 102-A1 ▷ |
| PE-1.2 | 102-A15 ▷ | 103-G6 ◁ |
| 0VDC | 102-C15 ▷ | 103-A1 ▷ |
| PE-0VDC | 102-D15 ▷ | 103-G1 ▷ |
| 24VDC-1 | 102-H15 | 103-A1 |
| 24VDC-2 | 102-H15 | 103-C1 |
| 24VDC-3 | 102-G15 | 103-D1 |
| 24VDC-4 | 102-G15 | 103-F1 |

**Connections**

| From | To |
|---|---|
| -102F1:2 | J4 |
| -102F2:Out1 | 24VDC-1 |
| -102F2:Out2 | 24VDC-2 |
| -102F2:Out3 | 24VDC-3 |
| -102F2:Out4 | 24VDC-4 |
| -102G1:+#1 | J3 |
| -102G1:+#2 | J3 |
| -102G1:PE | PE-1.2 |
| -102G1:−#1 | J5 |
| -102G1:−#2 | J6 |
| J3 | -102F1:1 |
| J4 | -102F2:24VDC#1 |
| J4 | -102F2:24VDC#2 |
| J5 | -102F2:GND#1 |
| J5 | PE-0VDC |
| J6 | -102F2:GND#2 |
| J6 | 0VDC |
| L-1 | -102G1:L |
| N-1 | -102G1:N |

**Junctions**

| Junction | Style | Wiring | Sequence of the far ends |
|---|---|---|---|
| J3 | Bevel | Chain | -102G1:+#1 | -102F1:1 | -102G1:+#2 |
| J4 | Bevel | Chain | -102F2:24VDC#1 | -102F1:2 | -102F2:24VDC#2 |
| J5 | Bevel | Star | -102G1:−#1 | -102F2:GND#1 | PE-0VDC |
| J6 | Bevel | Star | -102G1:−#2 | -102F2:GND#2 | 0VDC |

J5 and J6 are set to Star by hand (a bevel is a chain by default).

**Cross sections set by hand** (1.5 mm², printed as markers): -102G1:+#1 to -102F1:1 and -102G1:+#2 to -102F1:1,
-102G1:−#1 to -102F2:GND#1 and -102G1:−#2 to -102F2:GND#2, -102F1:2 to -102F2:24VDC#1 and -102F1:2 to -102F2:24VDC#2.
<!-- /folio -->

<!-- folio 103 -->
## Folio 103: Power Supply 3 - Control Current Distribution

Folio 103 distributes 24 V and 0 V on the strip -X1031 and PE on the strip -X1032. Place the terminals one by one: the
strips are -X1031 (24 V / 0 V distribution strip, part PT 2.5-TWIN) and -X1032 (PE distribution strip, rail-bonded,
part PT 2.5-TWIN-PE). This job places only the terminals that a connection below ends at: -X1031 terminals
1, 2, 3, 4, 5, 7, 9, 11, 12, 13, 14 and -X1032 terminals 1, 2, 3, 4.
The 24 V terminals stand in the area of 103-A3, the 0 V terminals in the area of 103-A10 and the PE terminals in
the area of 103-F3.

**Terminals**

| Terminal | Printed | Group label | Potential |
|---|---|---|---|
| -X1031:1 | 1+ | Pneumatics | 24VDC-1 |
| -X1031:2 | 1+ | Pneumatics | 24VDC-1 |
| -X1031:3 | 1+ | Pneumatics | 24VDC-1 |
| -X1031:4 | 1+ | Pneumatics | 24VDC-1 |
| -X1031:5 | 2+ | Spare | 24VDC-2 |
| -X1031:7 | 3+ | Spare | 24VDC-3 |
| -X1031:9 | 4+ | Spare | 24VDC-4 |
| -X1031:11 | 1- | Pneumatics | 0V |
| -X1031:12 | 1- | Pneumatics | 0V |
| -X1031:13 | 1- | Pneumatics | 0V |
| -X1031:14 | 1- | Pneumatics | 0V |
| -X1032:1 | PE | - | PE |
| -X1032:2 | PE | - | PE |
| -X1032:3 | PE | - | PE |
| -X1032:4 | PE | - | PE |

The terminals have three clamps; a is the feed side, b the outgoing side.

**Interruption points**

| Name | One end | Other end |
|---|---|---|
| PE-1.1 | 101-G15 ▷ | 103-G6 ◁ |
| PE-1.2 | 102-A15 ▷ | 103-G6 ◁ |
| 0VDC | 102-C15 ▷ | 103-A1 ▷ |
| PE-0VDC | 102-D15 ▷ | 103-G1 ▷ |
| 24VDC-1 | 102-H15 | 103-A1 |
| 24VDC-2 | 102-H15 | 103-C1 |
| 24VDC-3 | 102-G15 | 103-D1 |
| 24VDC-4 | 102-G15 | 103-F1 |
| 24VDC-AIR1 | 103-A6 | 106-E1 |
| 24VDC-AIR2 | 103-B6 | 106-E1 |
| 24VDC-AIR3 | 103-B6 | 106-B1 |
| 24VDC-AIR4 | 103-B6 | 106-B1 |
| 0VDC-AIR1 | 103-A14 | 106-E1 |
| 0VDC-AIR2 | 103-B14 | 106-F1 |
| 0VDC-AIR3 | 103-B14 | 106-B1 |
| 0VDC-AIR4 | 103-B14 | 106-C1 |
| PE-AIR1 | 106-E1 ◁ | 103-G1 ▷ |
| PE-AIR2 | 106-B1 ◁ | 103-G1 ▷ |
| PE-1.3 | 106-D1 ◁ | 103-G6 ◁ |
| PE-1.4 | 106-A1 ◁ | 103-G6 ◁ |

**Connections**

| From | To |
|---|---|
| -X1031:1/b | 24VDC-AIR1 |
| -X1031:11/b | 0VDC-AIR1 |
| -X1031:12/b | 0VDC-AIR2 |
| -X1031:13/b | 0VDC-AIR3 |
| -X1031:14/b | 0VDC-AIR4 |
| -X1031:2/b | 24VDC-AIR2 |
| -X1031:3/b | 24VDC-AIR3 |
| -X1031:4/b | 24VDC-AIR4 |
| -X1032:1/a | PE-AIR2 |
| -X1032:3/a | PE-AIR1 |
| -X1032:3/b | PE-1.3 |
| -X1032:4/b | PE-1.4 |
| 0VDC | -X1031:11/a |
| 24VDC-1 | -X1031:1/a |
| 24VDC-2 | -X1031:5/a |
| 24VDC-3 | -X1031:7/a |
| 24VDC-4 | -X1031:9/a |
| PE-0VDC | -X1032:2/a |
| PE-1.1 | -X1032:1/b |
| PE-1.2 | -X1032:2/b |

**Bridges** between the placed terminals of a strip (plug-in bridges). BR2, BR3 and BR4 join terminals that this job
does not place, so they stay undrawn; BR5 and BR6 join the placed terminals only.

| Bridge | Strip | Terminals | Potential |
|---|---|---|---|
| BR1 | -X1031 | 1-4 | 24VDC-1 |
| BR5 | -X1031 | 11-14 | 0V |
| BR6 | -X1032 | 1-4 | PE |
<!-- /folio -->
