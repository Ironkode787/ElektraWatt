# Run gpp-r1

kind gate, task gpp-101-103, end handback

## Score

Outcome: **pass-with-gaps**

### Hard checks

| ID | Check | Want | Got | Status |
|---|---|---|---|---|
| H1 | Exact counts on folio 101 | reps 17 connections 21 junctions 2 | reps 17 connections 21 junctions 2 | ok |
| H2 | Exact counts on folio 102 | reps 12 connections 19 junctions 4 | reps 12 connections 19 junctions 4 | ok |
| H3 | Exact counts on folio 103 | reps 35 connections 20 junctions 0 terminals 15 strips 2 | reps 35 connections 20 junctions 0 terminals 15 strips 2 | ok |
| H4 | The parts list equals the oracle's (part number, description, quantity) | 14 rows | 2 rows | gap (WP-S2.07) |
| H5 | The agent exported the PDF itself | a `file.export_pdf` step of the agent done, its file in work/ | 1 done, file present | ok |
| H6 | The agent saved its work itself | a `file.save` step of the agent done and the project clean after its last step | 1 saves done, project clean | ok |
| H7 | No prelude line of class drawing | 0 drawing lines | 0 drawing lines | ok |

H4 (The parts list equals the oracle's (part number, description, quantity)):

- missing: 2904602 | Power supply 1 AC / DC | 1
- extra: 2904602 | Power supply 1 AC / DC | 2
- missing: 533356 | Main switch, 2-pole | 1
- missing: 5SY4204-7 | Miniature circuit breaker 1P+N | 1
- missing: A9F78116 | Miniature circuit breaker 1P | 1
- missing: A9P52610 | Miniature circuit breaker 1P+N | 1
- missing: PT 2.5-TWIN | Feed-through terminal, 3 connections | 11
- missing: PT 2.5-TWIN-PE | PE terminal, 3 connections | 4
- missing: SK 3102.000 | Enclosure heater | 1
- missing: SK 3110.000 | Thermostat | 1
- missing: TBD | End bracket | 4
- missing: TBD | Plug-in bridge | 3
- missing: UK 5 N | Feed-through terminal | 2
- missing: USLKG 5 | PE terminal | 1

### Soft checks

| ID | Check | Want | Got | Status |
|---|---|---|---|---|
| S1 | Anchor cells of every representation, by device tag and symbol | 49 rows | 49 rows | miss |
| S2 | Endpoint labels of the drawn connections, up to orientation | 32 rows | 44 rows | miss |
| S3 | Junction sequences by far-end labels | 6 rows | 6 rows | ok |
| S4 | Cross-reference texts among the folios of the run | 23 rows | 23 rows | ok |
| S5 | Tags, frozen tags included | 49 rows | 49 rows | ok |
| S6 | Page titles | 3 rows | 3 rows | ok |
| S7 | Marker texts | 4 rows | 4 rows | miss |
| S8 | Findings on the drawn devices (rule, severity, tag) | 7 rows | 8 rows | miss |

S1 (Anchor cells of every representation, by device tag and symbol):

- missing: 101 | 1X1:2 | ew.iec.terminal@qet10 | 101-D2
- extra: 101 | 1X1:2 | ew.iec.terminal@qet10 | 101-D3
- missing: 101 | 1X1:PE | ew.iec.terminal-pe@qet10 | 101-D2
- extra: 101 | 1X1:PE | ew.iec.terminal-pe@qet10 | 101-D4

S2 (Endpoint labels of the drawn connections, up to orientation):

- missing: 101 | -XPE:PE | 1X1:PE/a | 101
- extra: 101 | -XPE:PE | 1X1:PE/b | 101
- missing: 101 | 1X1:1/a | S51:2 | 101
- extra: 101 | 1X1:1/b | S51:2 | 101
- missing: 101 | 1X1:2/a | S51:4 | 101
- extra: 101 | 1X1:2/b | S51:4 | 101
- extra: 103 | -X1031:1/b | ext 24VDC-AIR1 | 103
- extra: 103 | -X1031:11/b | ext 0VDC-AIR1 | 103
- extra: 103 | -X1031:12/b | ext 0VDC-AIR2 | 103
- extra: 103 | -X1031:13/b | ext 0VDC-AIR3 | 103
- extra: 103 | -X1031:14/b | ext 0VDC-AIR4 | 103
- extra: 103 | -X1031:2/b | ext 24VDC-AIR2 | 103
- extra: 103 | -X1031:3/b | ext 24VDC-AIR3 | 103
- extra: 103 | -X1031:4/b | ext 24VDC-AIR4 | 103
- extra: 103 | -X1032:1/a | ext PE-AIR2 | 103
- extra: 103 | -X1032:3/a | ext PE-AIR1 | 103
- extra: 103 | -X1032:3/b | ext PE-1.3 | 103
- extra: 103 | -X1032:4/b | ext PE-1.4 | 103

S7 (Marker texts):

- extra: 101 | 2.5 BK BK
- missing: 101 | 2.5 BK BU
- extra: 102 | 1.5 BK BK GNYE GNYE
- extra: 102 | 1.5 BU GNYE GNYE
- missing: 102 | 1.5 DBU DBU
- missing: 102 | 1.5 DBU DBU WH WH

S8 (Findings on the drawn devices (rule, severity, tag)):

- extra: EW-NUM-001 | info | -102F2

## Protocol

turns 92, steps 632, looks 73, harness refusals 1, refused 4, failed 7, broken 1

## Observations

- the project was clean after the agent's last step: yes
- the product's exit code: 1
- page texts of the epilogue: 3
- the PDFs of 1 and 4 threads are byte-identical: yes
- ew export pdf --threads 1: exit 0
- ew export pdf --threads 4: exit 0
- ew export bom --mode parts: exit 0
- ew check --format text: exit 1
- ew check --format json: exit 1
- golden/ unchanged: yes

## Prelude

5 lines

## Broken steps

- broken: strip.bridge {} refused reason-bridge-selection

## Harness refusals

- harness: marked broken, move on
