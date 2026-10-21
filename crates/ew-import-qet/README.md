<!-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ew-import-qet

QElectroTech importer behind `ew-interop-api` (doc 14 §4): standalone element definitions (`.elmt`,
`QetElementImporter`) and whole projects (`.qet`, `QetProjectImporter`, lenient mode). The CLI entry is
`ew import qet <file.qet> [--content DIR] [--report md|csv|json] [--report-out FILE] [--strict]`; the library
entry is `ew_api::import::import_qet`. A lenient import that completes exits 0; `--strict` exits 1 when a
connection was lost or an error finding exists. With the report on stdout, the summary goes to stderr.

## Project import (spike, cycle 11)

The project importer reads the whole `<project>` under the project limits of `ImportContext::limits` (doc 14
§11: input size, depth, node, attribute and text limits apply to the whole file; the per-definition caps of
the element reader apply to each embedded definition), maps it to `ew-schema` records and reports everything
it cannot map as `EW-IMP` findings. It never aborts on an item; only an unreadable file is an `ImportError`.
`ew_import_qet::migration::MigrationReport` groups the findings by code with counts.

### Mapping decisions

| QET source | ElektraWatt target | Notes |
|---|---|---|
| file | `ImportLineage` anchored `QetProjectUuid` (QET 0.200+ `<project uuid>`), else `FirstImportSha256`; project ID `v5(ns, "qet:project")` unless the context names one | doc 14 §2.3 |
| `<project title>` | `Project.master.description` | |
| project property `saveddate` (ISO date) | `Project.master.edited` | other properties in `Project.extra["qet.project_properties"]`; `savedfilepath` dropped (a local path, EW-IMP-004) |
| `<newdiagrams><report label>` | `Project.extra["qet.report_label"]` | EW's default xref format applies (EW-IMP-008); `%f` also EW-IMP-117 |
| `<diagram>` | `Page { kind: multi_line, grid: QET 10 px }` | source key `qet:folio:<printed folio>`, a repeated folio `qet:folio-order:<position>` (EW-IMP-008) |
| `folio` attribute | `Folio.number` (whole numbers; `01` keeps its text as `Folio.label`), else leading digits + `Folio.label` | `%id`, `%total`, `%autonum` substituted; `%id/%total` prints the position only (EW-IMP-009); the folio key of source keys is the printed folio |
| `title` | `Page.title` | |
| border 17 × 60 px / 8 × 80 px | form `ew.frame.qet-a4-landscape-grid` | other borders: default frame (EW-IMP-004) |
| `author`, `date`, `indexrev`, other title-block attributes, folio properties | a value equal on **all** folios goes to `Project.extra["qet.titleblock"]` / `["qet.properties"]`; every other value stays in `Page.extra` (one pass, doc 14 §4.9) | differing `author`, `date`, `indexrev` and properties: EW-IMP-104 (D-05) |
| free texts, shapes, images, `graphics_table` | not imported | no page-item or report-block mapping yet (EW-IMP-004 per folio) |
| embedded `<definition>` | `SymbolDef` + generated `FnDef` in the per-project "QET import" library | by the element reader; only definitions that placed elements use are embedded and reported |
| element of link type `simple`, `master` | `Device` + main `Function` + `Representation` | keys `qet:el:<key>`, `…#fn`, `…#rep`; `<key>` is the UUID, `<uuid>#<folio key>#<n>` for a UUID repeated by copy-paste (EW-IMP-120; links prefer the copy on the same folio), `<folio key>#el#<n>` without UUID |
| element of link type `thumbnail` | not imported (a graphic, doc 14 §4.3) | EW-IMP-004 |
| element of link type `slave` linked to a master | auxiliary `Function` of the master's device + `Representation` | no master: own device, promoted to main (EW-IMP-116) |
| `next_report` / `previous_report` | interruption point (`ew.fn.ip`, `Source` / `Target`) + `Representation` | a linked pair gets one name: source label, target label, source `UserText`, target `UserText`; differing end names: EW-IMP-115 with both texts; no name: one `IP-<folio>-<n>` per pair (EW-IMP-115); several pairs of one name: `IpSpec.sort` 2i / 2i + 1 and EW-IMP-115 (the project needs chain mode) |
| element of link type `terminal` | terminal `Device` (`kind: terminal`, `Terminal { strip, seq, designation }`) in a strip `Device` (`Strip` aspect) | see "Strips" |
| element with one terminal, no label, ≥ 3 conductors | `Junction { style: dot, wiring: star }` | EW-IMP-118 |
| element without definition or position | dropped (EW-IMP-003) | its wires are lost connections |
| element position `x`, `y` | `Representation.pos`, exact (`793 750 / 3` nm per px) | on QET's default border, an element exactly on an inner column or row line (`x = 20 + k × 60`, `y = 20 + k × 80` px) moves 2 nm left or up: QET's `convertPosition` puts a line point into the left column or upper row (ceiling), EW's cell rule into the right column or lower row, and 2 nm leave the cell rule's 1 nm snap window, so cross-reference cells print as QET prints them (owner ruling, review of cycle 12) |
| `orientation` 0–3 | `Orient2` R0, R90, R180, R270 (clockwise) | |
| `label` (rule R1 `-{page}{letter}{counter}` on its own folio, upper-case letters, counter without leading zeros) | derived `TagSpec { letter, counter }` | labels EW would print differently (`-101K01`, `-101f1`) stay frozen |
| any other `label` | `FrozenTag { text, sign }` (sign hidden without a leading `-`) | EW-IMP-103 (strips excluded) |
| `label` with structure prefix (`+…-X1`) | product part as above, prefix in `Device.extra["qet.structure"]` | structure nodes not created (gap) |
| no `label` | `TagSpec { letter: prefix attribute, else "A", counter: none }`, prints `?` (owner ruling) | EW-IMP-122 (proposed new code: element without label) count per folio |
| `function` (else `description`) | `Device.function_text` | |
| other element information (`manufacturer`, `designation`, `comment`, `formula` …) | `Device.extra["qet.info"]` (slaves: `Function.extra`) | EW-IMP-008 per name with a count; parts, tech data and formulas not mapped |
| instance dynamic texts, text groups | not imported | the symbol's slots print derived values |
| conductor | `Connection { kind: wire }` between function CPs or junctions | ends by UUID, else per-folio terminal ID; a UUID repeated on the folio is decided by the route geometry, else the first copy (EW-IMP-120); unresolved or identical ends dropped (EW-IMP-003, lost) |
| the imported model | lenient check findings of the `migration` scheme (EW-RDS-004, EW-TERM-002 and every other `pre-delivery` rule of `ew-checks`) in the migration report | `ew_api::import::lenient_findings` |
| `<segment>`s | `Connection.route` (interior points) | conductors without segments get EW's provisional route (QET's auto-route not ported) |
| `num` | `NumberSpec::Frozen` (non-empty), else `Unassigned` | |
| `function`, `tension_protocol`, `conductor_color`, `conductor_section` | `Connection.extra["qet.*"]` | EW-IMP-008 per field; no potential definitions or wire overrides yet |
| `titleblocktemplates`, `terminal_strips` (QET 0.100+ strips and bridges) and other top-level members | not imported | EW-IMP-004; bridges stored in `<terminal_strips>` are dropped (the pilot files have none) |

### Strips (projects without QET 0.100 strip data)

A terminal element joins a strip by (doc 14 §4.11, §14 item 5): its label prefix (`X1031:1+`, optional
structure prefix); else the strip header heading its line (an element labelled with a strip designation;
lines are per folio, a column per x for elements turned a quarter, else a row per y); else the folio's
fallback strip `X?<page>` (EW-IMP-005). Strips are keyed by their own designation including the structure
prefix (`qet:strip:<structure>-<product>`; `+A-X1` and `+B-X1` differ), the fallback by its folio key, never by a
line number, so re-imports keep strip IDs. One designation drawn in several lines is one strip (EW-IMP-109).
A terminal's designation is its label suffix, else its first `UserText` dynamic text, else a non-strip label,
else its sequence number (EW-IMP-008).

### Function semantics and symbols

Every placed element keeps the graphics of its embedded definition. Its function semantics come from the
element reader: `kindInformations` (coil, protection, commutator, contacts, terminals), then the shipped
category-path table, else a generic black box (EW-IMP-119, once per used definition). Exchanging QET-import
symbols for starter-library `@qet10` symbols (`symbol-map.csv`, Golden C fix step 3) is not part of this
importer yet.

### Gaps (spike)

Formula evaluation and EW-IMP-101; rule inference R2/R3 and `AdoptRule`; structure nodes and EW-IMP-102;
parts from element information; text-to-data recognisers (EW-IMP-107/108); `graphics_table` → report blocks;
free texts, shapes and images; cables; conductor properties → potential definitions and overrides; QET's
auto-route; switching the project to chain mode; external connection points; title-block template forms; QET 0.100
`terminal_strips` data; empty terminal designations (MF-08 needs an empty designation, which `Ident` cannot
hold); the starter-symbol exchange.

### Follow-up tasks

- FT: read QET 0.100+ `<terminal_strips>` (`terminal_strip_data`, `physical_terminal`, `real_terminal`,
  `terminal_strip_bridge`, doc 14 §4.11) into strips, terminal levels and bridge connections; today the
  member is reported as not imported and its bridges are dropped.
