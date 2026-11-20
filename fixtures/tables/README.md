<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC-BY-4.0
-->

# The test tables

CSV tables of one small control cabinet in the format of the table importer (`ew-import-tables`, `ew import csv`),
one file per table and every table the importer reads. They are test inputs written by hand, not the output of a
tool: the importer's integration tests (`crates/ew-import-tables/tests/it/`: assembly, bus, colour modes, hostile
input, layout, parts) read them through `common::fixture_inputs()`, and change a row in memory where a test needs a
defect.

What they hold: the cover page, the bus topology page (3), the supply pages 101 and 102 with their wires, strips,
junctions and interruption points, and the cabinet layout page 902; the valve terminal -201U1 with its eleven
members, 28 valves, two order codes and its PROFIBUS port; the cable -106W1 with its cores; the parts with their
connection points, templates and PLC data; the enclosure -UC1 with its plate, rails, ducts and placements. The data
was cut from the reference cabinet the tests were first written on, so its tags and part numbers are that cabinet's.

The rule they keep: a strict and a lenient import of the whole set gives records and no finding. Some interruption
points on 101 and 102 have no partner, because the pages of their partners are not part of the set; that is a
finding of the checks of an opened project, not of the import. A change that makes the import report something
fails the tests that assert an empty report (`assembly`, `bus`, `colour_modes`).

What they do not hold: the generated report pages (only their report blocks), the distribution pages 103 and 106, the
overview and pneumatic pages with
their field devices (but -203Q1, which a valve drives), and the projects other tests open (`fixtures/format/`, the
sample project).
