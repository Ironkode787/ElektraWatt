# Run gpp-r2

kind gate, task gpp-101-103, end handback

## Score

Outcome: **pass-with-gaps**

### Hard checks

| ID | Check | Want | Got | Status |
|---|---|---|---|---|
| H1 | Exact counts on folio 101 | reps 17 connections 21 junctions 2 | reps 17 connections 21 junctions 2 | ok |
| H2 | Exact counts on folio 102 | reps 12 connections 19 junctions 4 | reps 12 connections 19 junctions 4 | ok |
| H3 | Exact counts on folio 103 | reps 35 connections 20 junctions 0 terminals 15 strips 2 | reps 35 connections 20 junctions 0 terminals 15 strips 2 | ok |
| H4 | The parts list equals the oracle's (part number, description, quantity) | 14 rows | 12 rows | gap (WP-S2.07) |
| H5 | The agent exported the PDF itself | a `file.export_pdf` step of the agent done, its file in work/ | 1 done, file present | ok |
| H6 | The agent saved its work itself | a `file.save` step of the agent done and the project clean after its last step | 1 saves done, project clean | ok |
| H7 | No prelude line of class drawing | 0 drawing lines | 0 drawing lines | ok |

H4 (The parts list equals the oracle's (part number, description, quantity)):

- missing: TBD | End bracket | 4
- missing: TBD | Plug-in bridge | 3

### Soft checks

| ID | Check | Want | Got | Status |
|---|---|---|---|---|
| S1 | Anchor cells of every representation, by device tag and symbol | 49 rows | 49 rows | miss |
| S2 | Endpoint labels of the drawn connections, up to orientation | 32 rows | 44 rows | miss |
| S3 | Junction sequences by far-end labels | 6 rows | 6 rows | ok |
| S4 | Cross-reference texts among the folios of the run | 23 rows | 23 rows | ok |
| S5 | Tags, frozen tags included | 49 rows | 49 rows | ok |
| S6 | Page titles | 3 rows | 3 rows | ok |
| S7 | Marker texts | 4 rows | 0 rows | miss |
| S8 | Findings on the drawn devices (rule, severity, tag) | 7 rows | 7 rows | ok |

S1 (Anchor cells of every representation, by device tag and symbol):

- missing: 101 | 1X1:2 | ew.iec.terminal@qet10 | 101-D2
- extra: 101 | 1X1:2 | ew.iec.terminal@qet10 | 101-D3
- missing: 101 | 1X1:PE | ew.iec.terminal-pe@qet10 | 101-D2
- extra: 101 | 1X1:PE | ew.iec.terminal-pe@qet10 | 101-D4

S2 (Endpoint labels of the drawn connections, up to orientation):

- missing: 101 | 1X1:1/a | ext J-E-206-L | 101
- extra: 101 | 1X1:1/b | ext J-E-206-L | 101
- missing: 101 | 1X1:2/a | ext J-E-206-N | 101
- extra: 101 | 1X1:2/b | ext J-E-206-N | 101
- missing: 101 | 1X1:PE/a | ext J-E-206-PE | 101
- extra: 101 | 1X1:PE/b | ext J-E-206-PE | 101
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

- missing: 101 | 2.5 BK BU
- missing: 101 | 2.5 BK BU
- missing: 102 | 1.5 DBU DBU
- missing: 102 | 1.5 DBU DBU WH WH

## Protocol

turns 68, steps 664, looks 25, harness refusals 0, refused 3, failed 1, broken 0

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

none

## Harness refusals

none
