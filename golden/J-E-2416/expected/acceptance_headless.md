# Acceptance report: golden project J-E-2416, gate M1 (headless)

83 entries: 51 pass, 2 fail, 24 not_built, 6 not_applicable. Oracle mismatches: 0. `fail` and `not_built` block the gate; `not_applicable` does not.

| Criterion | Check | Status | Reason |
|---|---|---|---|
| AC-01 | frame, grid and title-block geometry equals expected/geometry/frame.json | pass | the two frame forms and the QET title block print the committed figures (exact, stricter than 0.2 mm) |
| AC-01 | the printed frame equals the figures of §17.1 | pass | outer frame, header band, column width, row height, first column edge and drawing area within 0.01 mm |
| AC-01 | the written PDF's vector content of every page equals the frame geometry within 0.2 mm | not_built | no per-page comparison of the written PDF exists: the checks above read the frame forms and AC-04 the title-block texts, not the lines of the PDF (vector extraction with the 0.2 mm tolerance of §3.3) |
| AC-02 | every representation anchors in the cell of representations.csv | pass | all 135 anchor cells equal their table cells |
| AC-02 | text of folios 1, 3, 100-106, 200-206, 900 equals expected/text | pass | 15 folios: strings, anchors (3 decimals) and sizes equal the committed text CSVs |
| AC-02 | raster of those folios within 0.5 % of expected/png | not_applicable | the raster comparison was switched off for this run (Options::raster) |
| AC-03 | page order equals the 36 folios of §5 | fail | 34 folios derived against 36 in §5; missing 908.1, 908.2 (908.1, 908.2: no PLC channel records, FT-111) |
| AC-04 | every page prints the bound title-block values | pass | all 34 pages print the six bound values |
| AC-05 | the interruption-point texts equal §15 (22 pairs, 44 texts) | pass | 44 texts equal the "Printed at source / target" columns of §15 |
| AC-05 | the 9 main and 10 auxiliary texts equal §15 by value | pass | the 9 main texts ("∞ ...") and 10 auxiliary texts ("(...)") equal §15 as a multiset |
| AC-05 | 63 linked texts with 64 targets, 4 external texts, "3-B2" | pass | 63 linked texts with 64 link targets and 4 external texts on the drawn pages; -201U1 prints 3-B2 |
| AC-06 | every device tag equals its table key; no stale token in any page text | pass | 96 devices print their table key; no "-4W1", "-4X1", "T51", "JE2416" or core colour "BR" in any page text |
| AC-06 | every printed occurrence of a tag is identical, the PDF text layer included | not_built | the harness has no PDF text extraction (FR-OUT-008): the check above reads the generated page texts, not the text layer of the written PDF |
| AC-07 | folio 901 holds the 27 rows of §14.1 over 901 and 901.1 | pass | 27 rows over 901 and 901.1 |
| AC-07 | bom export --mode assembly equals §14.2 | not_built | the assembly BOM mode and `ew export bom` do not exist yet (FR-EXP-005, FR-REP-005) |
| AC-08 | folios 903-905 equal expected/csv, designations unique per strip | pass | 3, 20 and 5 terminal rows equal the committed CSVs (hand-compared with §9, differences in the README); designations unique per strip |
| AC-09 | folio 907 equals expected/csv/wires.csv: 44 wires, 6 bridges | pass | 44 wire rows and 6 bridge rows equal expected/csv/wires.csv |
| AC-09 | every wire has an effective size and colour | pass | 44 wires have an effective size and colour |
| AC-09 | every wire has a wire number | not_built | 0 of 44 wires have a wire number: the wire numbering query does not exist yet (FT-100) |
| AC-10 | the potential partition holds the 19 potentials of §7.3 and bond B1 | fail | 18 potentials, §7.3 has 19 (the PROFIBUS DP potential needs the Bus connections, FT-105) |
| AC-10 | folio 909 equals expected/csv/potentials.csv | pass | the derived rows equal expected/csv/potentials.csv (the SQL half is a separate entry) |
| AC-11 | folio 906 equals expected/csv/cables.csv: 3 cables, 13 cores | pass | 3 cables and 13 cores equal expected/csv/cables.csv |
| AC-11 | export cables equals §10 | not_built | `ew export cables` does not exist yet (FR-EXP-005) |
| AC-12 | folio 1 prints the colour and size tables generated from the profiles | pass | folio 1 prints the 15 colour rows, the 2 size tables and the basis text of the profiles |
| AC-12 | changing the 24 V colour updates every affected wire and folio 907 | pass | 8 wires take the new colour and nothing else changes; folio 907's rows follow (recomputed from the edited profile) |
| AC-12 | the colour change is one commit | pass | one project.settings.set commit (one undo step, the project record only): 8 wires take RD, nothing else changes, folio 907 follows, one undo restores |
| AC-13 | PDF structure equals expected/pdf (outline, 64 links, page labels, fonts, SHA-256) | pass | 34 pages: outline, 64 links, page labels = folio labels, embedded fonts and SHA-256 equal the committed files; no Tahoma (the folio count is 34 against 36, AC-03) |
| AC-13 | text extraction of the PDF keeps spaces | not_built | the harness has no PDF text extraction (FR-OUT-008); the ToUnicode maps are written, but nothing reads them back |
| AC-13 | PDF/A-2b validated in process by krilla | pass | the 34-page PDF was written under Conformance::PdfA2b, which makes krilla's A2_B archival validator check the document (it refuses an invalid one) |
| AC-13 | veraPDF cross-check of PDF/A-2b | not_applicable | veraPDF is an external tool that is not installed in the headless harness; the CI job that has it runs the cross-check |
| AC-08 | acceptance SQL over the api_v1 views equals expected/csv: terminals_1X1, terminals_X1031, terminals_X1032 | not_built | SQL projection pending (WP-S1.10); the CSV half above compares the derived report rows |
| AC-09 | acceptance SQL over the api_v1 views equals expected/csv: wires | not_built | SQL projection pending (WP-S1.10); the CSV half above compares the derived report rows |
| AC-10 | acceptance SQL over the api_v1 views equals expected/csv: potentials | not_built | SQL projection pending (WP-S1.10); the CSV half above compares the derived report rows |
| AC-11 | acceptance SQL over the api_v1 views equals expected/csv: cables | not_built | SQL projection pending (WP-S1.10); the CSV half above compares the derived report rows |
| AC-18 | step B1: the ten tags of the tag map | pass | the ten tags of the tag map change and nothing else; no cross-reference text changes |
| AC-18 | step B2: 2 tags and 14 cross-reference texts, same topology, undo restores the records | pass | folio 106 becomes 104: 2 tags and 14 texts change, same potentials and connections, one undo restores the records |
| AC-18 | step B2: the preview lists the folio change as one page record | pass | the preview changes one page record (Changed Page ["folio.number"]) and commits nothing |
| AC-18 | step B2: the preview lists the 2 tag and 14 cross-reference changes | pass | the derived diff of the preview lists -106W1 → -104W1, -106W2 → -104W2 and 14 texts "106-…" → "104-…"; nothing is committed |
| AC-18 | step B2: no orphan reference and the same interruption-point pairing after the commit | pass | the model's integrity check (every reference resolves) holds, and all interruption-point and main/auxiliary link targets are the same records |
| AC-20 | renumbering folio 3 to 4 updates -3W1, -3X1 and "3-B2" everywhere; undo restores | pass | -3W1 and -3X1 become -4W1 and -4X1, 1 cross-reference text(s) follow ("3-B2" becomes "4-B2"), undo restores the records |
| AC-20 | renumbering folio 3 to 4 writes only that page's file | pass | the save writes pages/93d03619-9f8d-5086-8b99-1a4f05456f21.jsonl only (and rewrites the manifest) |
| AC-14 | PDF and JSONL are byte-identical across runs and record shuffles | not_applicable | a Linux-only gate criterion, evaluated by tests/page_output.rs (PDF), tests/golden_fixture.rs (JSONL) and the CI Linux job, not by this runner |
| AC-14 | SVG and the parallel export are byte-identical to the serial export | not_built | neither the SVG export nor a parallel export exists yet (FR-OUT-013, FR-OUT-019) |
| AC-15 | build path (a), the CSV bootstrap, equals path (c), the checked-in fixture | not_applicable | evaluated by tests/golden_fixture.rs (byte-identical regeneration), not by this runner |
| AC-15 | build path (b), the builder DSL, produces the same expected CSVs | not_built | the builder DSL (the golden builders of ew-testkit, WP-S6.01) does not exist yet |
| AC-16 | freshness: scripted edits update every output; verify-derived is clean with the oracle on | not_built | the scripted edits (rename T55, move -102F2, folio 103 to 104, 24 V colour) need the tag, move and profile commands and `ew verify-derived`; for the record, the oracle is on for the sessions `oracle` and `profiled` of this run, and off for the sessions that apply edits or scripts (AC-12, AC-18, AC-20, D-06, D-11, D-12) and for the cross-reference scans |
| AC-17 | open <= 0.5 s, check <= 2 s, PDF export <= 3 s | not_applicable | measured at milestone deliveries (cargo xtask acceptance --milestone, release profile) |
| AC-39 | the anonymisation guard passes on golden/, docs/ and expected/ | not_applicable | the guard is `cargo xtask check-anonymisation` (the xtask crate, outside this test crate); the repository checks run it |
| D-01 | renumber folio 3 as 4: -4W1, -4X1 and "4-B2" everywhere, undo restores | pass | Same check as AC-20 "renumbering folio 3 to 4 updates -3W1, -3X1 and "3-B2" everywhere; undo restores": -3W1 and -3X1 become -4W1 and -4X1, 1 cross-reference text(s) follow ("3-B2" becomes "4-B2"), undo restores the records |
| D-01 | renumbering writes only that page's file | pass | Same check as AC-20 "renumbering folio 3 to 4 writes only that page's file": the save writes pages/93d03619-9f8d-5086-8b99-1a4f05456f21.jsonl only (and rewrites the manifest) |
| D-03 | no output prints the structure alias JE2416 | pass | no page text contains "JE2416" |
| D-03 | the numbering report lists the frozen tags (EF-15) | not_built | the numbering report and the check engine's finding EF-15 do not exist yet |
| D-04 | the parts list holds the 27 rows of §14.1 | pass | Same check as AC-07 "folio 901 holds the 27 rows of §14.1 over 901 and 901.1": 27 rows over 901 and 901.1 |
| D-05 | V2.0 on every page | pass | Folio 911 (the revision overview, V1.0 and V2.0) is still a placeholder: the report kind does not read the revision records yet. Same check as AC-04 "every page prints the bound title-block values": all 34 pages print the six bound values |
| D-06 | the project description is one text on every page | pass | all 34 pages print "Description: Expansion Control Cabinets" |
| D-06 | an override set on 101 changes the description of 101 only | pass | the description of 101 becomes the override; 102 keeps the project description |
| D-06 | an override is reported as info and shown in the property grid | not_built | the check engine (info finding) and the property grid do not exist yet |
| D-07 | outline, 64 cross-reference links and page labels | pass | Same check as AC-13 "PDF structure equals expected/pdf (outline, 64 links, page labels, fonts, SHA-256)": 34 pages: outline, 64 links, page labels = folio labels, embedded fonts and SHA-256 equal the committed files; no Tahoma (the folio count is 34 against 36, AC-03) |
| D-08 | W29 takes 0.75 DBU from the profile, W20 1.5 mm2 from its CDP and DBU from the profile | pass | W29 0.75 DBU from the profile; W20 1.5 mm2 from its CDP, DBU from the profile (the explain output and the one-commit update of folios 1 and 907 are AC-12) |
| D-09 | W1-W44 are numbered, unique and stable | not_built | Same check as AC-09 "every wire has a wire number": 0 of 44 wires have a wire number: the wire numbering query does not exist yet (FT-100) |
| D-10 | -202U2 carries 4 valves | pass | -202U2 has 4 child valves (the rendering of folio 202 is covered by AC-02) |
| D-11 | an empty terminal designation is rejected (parameter layer or invariant) | pass | terminals\|-X1031:8: designation `` is refused by the parameter layer (InvalidParams at /value) |
| D-12 | a duplicate designation is rejected by the invariant | pass | terminals\|-X1031:3: designation `4` is refused by the prevent rule EW-TERM-002 |
| D-12 | swapping two designations in one transaction is accepted | pass | one terminal.designate transaction swaps -X1031:3 and -X1031:4 (one undo step); EW-TERM-002 accepts the final state |
| D-14 | the bus port prints "3-B2", never "3-B0" | pass | -201U1 prints "∞ 3-B2, 106-G3" |
| D-14 | forcing the origin anchor reports "3-B0: empty cell" | not_built | no command sets a representation's anchor policy, and the check engine (FR-CHK-035) does not exist yet |
| D-17 | core colour BN is printed, never BR | pass | folio 906 prints core colour BN and no BR |
| D-17 | strict import rejects "BR"; lenient import keeps it as a foreign code with the quick fix "use BN" | not_built | the CSV importer has no strict or lenient mode (ImportContext carries none) and quick fixes do not exist yet |
| D-18 | the typed ratings and manufacturer names print on folios 101 and 102 | pass | all six values print |
| D-20 | W13/W14 are a star at F55:2, W7/W8 a chain on -101F1:1 | not_built | the wire list prints neither the junction nor its wiring style (README, "Wire list (907)"), so the wire list cannot show the notation |
| D-22 | the 0 V to PE connection is the bond B1 | pass | the model holds exactly one bond, B1 (the connectivity test bond_b1_links_0v_and_pe checks its ends) |
| D-22 | replacing B1 by a plain wire is reported | not_built | the check engine (FR-CHK-028) does not exist yet |
| D-24 | 24VDC-1, 0V and PE carry their IP names as aliases | pass | 24VDC-1 with AIR1-AIR4, 0V with 0VDC and AIR1-AIR4, PE with its 7 aliases (the canonical name and aliases of folio 909 are in AC-10) |
| D-27 | 28 valve child tags and 8 field devices with process tags in +FIELD | pass | 28 valve child tags (-201U3-Q1 ...) and 8 field devices with process tags in +FIELD |
| D-29 | every device tag prints in the style tag.main (9.62 pt) | pass | 17 printed device tags on 101, 202 and 203-206 are all 9.62 pt |
| D-30 | the title block binds the end user | pass | all 34 title blocks print "End user: <end user>" |
| D-30 | the cover binds the customer | not_built | the cover (folio 1) is not drawn (README, "Folio 1") |
| D-33 | "SK 3110.000" keeps its space and physical page 6 is labelled "101" | pass | both hold |
| D-35 | BOM descriptions per §14, ratings only at symbols | not_built | the typed-data lint EF-25 of the check engine does not exist yet (the 27-row parts list is D-04) |
| D-36 | assembly rows A1 and A2, order code on folio 3 | not_built | the assembly BOM mode and `ew export bom` do not exist yet (FR-REP-005) |
| D-37 | profiles name their basis; the cover prints it | pass | the colour profile's basis is company_convention and the cover prints "company convention; codes per IEC 60757, DBU is a company code (BU, dark shade)" |
| D-41 | one assembly -201U1 with children on both pages, break marks | not_built | the assembly aspect and the generated overview break marks are not implemented (schema v0 keeps the assembly in `extra`) |
| D-42 | one strip -X1031 with segments at 103-A3 and 103-A10 | pass | one device -X1031 (of 3 strips) with segments at 103-A3 and 103-A10 (the single BOM row is part of D-04) |
