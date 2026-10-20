<!-- SPDX-FileCopyrightText: 2026 The ElektraWatt contributors -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ew-import-qet

QElectroTech importer behind `ew-interop-api` (doc 14 §4): standalone element definitions (`.elmt`,
`QetElementImporter`) and whole projects (`.qet`, `QetProjectImporter`, lenient mode). The CLI entry is
`ew import qet <file.qet> [--content DIR] [--report md|csv|json]`; the library entry is
`ew_api::import::import_qet`.

## Project import (spike, cycle 11)

The project importer reads the whole `<project>` under the project limits of `ImportContext::limits` (doc 14
§11: input size, depth, node, attribute and text limits apply to the whole file; the per-definition caps of
the element reader apply to each embedded definition), maps it to `ew-schema` records and reports everything
it cannot map as `EW-IMP` findings. It never aborts on an item; only an unreadable file is an `ImportError`.
`ew_import_qet::migration::MigrationReport` groups the findings by code with counts.

### Mapping decisions

| QET source | ElektraWatt target | Notes |
|---|---|---|
| file | `ImportLineage` anchored `FirstImportSha256` (QET 0.100 has no project UUID); project ID `v5(ns, "qet:project")` unless the context names one | doc 14 §2.3 |
| `<project title>` | `Project.master.description` | |
| project property `saveddate` (ISO date) | `Project.master.edited` | other properties in `Project.extra["qet.project_properties"]`; `savedfilepath` dropped (a local path, EW-IMP-004) |
| `<newdiagrams><report label>` | `Project.extra["qet.report_label"]` | EW's default xref format applies (EW-IMP-008); `%f` also EW-IMP-117 |
| `<diagram>` | `Page { kind: multi_line, grid: QET 10 px }` | source key `qet:folio:<printed folio>`, a repeated folio `qet:folio-order:<position>` (EW-IMP-008) |
| `folio` attribute | `Folio.number` (whole numbers), else leading digits + `Folio.label` | `%id`, `%total`, `%autonum` substituted; `%id/%total` prints the position only (EW-IMP-009) |
| `title` | `Page.title` | |
| border 17 × 60 px / 8 × 80 px | form `ew.frame.qet-a4-landscape-grid` | other borders: default frame (EW-IMP-004) |
| `author`, `date`, `indexrev`, other title-block attributes, folio properties | majority value in `Project.extra["qet.titleblock"]` / `["qet.properties"]`, deviations in `Page.extra` | differing `author`, `date`, `indexrev` and properties: EW-IMP-104 (D-05) |
| free texts, shapes, images, `graphics_table` | not imported | no page-item or report-block mapping yet (EW-IMP-004 per folio) |
| embedded `<definition>` | `SymbolDef` + generated `FnDef` in the per-project "QET import" library | by the element reader; only definitions that placed elements use are embedded and reported |
| element of link type `simple`, `master`, `thumbnail` | `Device` + main `Function` + `Representation` | keys `qet:el:<key>`, `…#fn`, `…#rep` |
| element of link type `slave` linked to a master | auxiliary `Function` of the master's device + `Representation` | no master: own device, promoted to main (EW-IMP-116) |
| `next_report` / `previous_report` | interruption point (`ew.fn.ip`, `Source` / `Target`) + `Representation` | name: label, else first `UserText` dynamic text, else the linked partner's; else `IP-<folio>-<n>` (EW-IMP-115); several sources of one name: EW-IMP-115 (chain mode not set) |
| element of link type `terminal` | terminal `Device` (`kind: terminal`, `Terminal { strip, seq, designation }`) in a strip `Device` (`Strip` aspect) | see "Strips" |
| element with one terminal, no label, ≥ 3 conductors | `Junction { style: dot, wiring: star }` | EW-IMP-118 |
| element without definition or position | dropped (EW-IMP-003) | its wires are lost connections |
| element position `x`, `y` | `Representation.pos`, exact (`793 750 / 3` nm per px) | |
| `orientation` 0–3 | `Orient2` R0, R90, R180, R270 (clockwise) | |
| `label` (rule R1 `-{page}{letter}{counter}` on its own folio) | derived `TagSpec { letter, counter }` | |
| any other `label` | `FrozenTag { text, sign }` (sign hidden without a leading `-`) | EW-IMP-103 (strips excluded) |
| `label` with structure prefix (`+…-X1`) | product part as above, prefix in `Device.extra["qet.structure"]` | structure nodes not created (gap) |
| no `label` | `TagSpec { letter: prefix attribute, else "A", counter: none }` | EW-IMP-112 count per folio |
| `function` (else `description`) | `Device.function_text` | |
| other element information (`manufacturer`, `designation`, `comment`, `formula` …) | `Device.extra["qet.info"]` (slaves: `Function.extra`) | EW-IMP-008 per name with a count; parts, tech data and formulas not mapped |
| instance dynamic texts, text groups | not imported | the symbol's slots print derived values |
| conductor | `Connection { kind: wire }` between function CPs or junctions | ends by UUID, else per-folio terminal ID; unresolved or identical ends dropped (EW-IMP-003, lost) |
| `<segment>`s | `Connection.route` (interior points) | conductors without segments get EW's provisional route (QET's auto-route not ported) |
| `num` | `NumberSpec::Frozen` (non-empty), else `Unassigned` | |
| `function`, `tension_protocol`, `conductor_color`, `conductor_section` | `Connection.extra["qet.*"]` | EW-IMP-008 per field; no potential definitions or wire overrides yet |
| `titleblocktemplates`, `terminal_strips` and other top-level members | not imported | EW-IMP-004 |

### Strips (projects without QET 0.100 strip data)

Terminal elements are grouped per folio into lines: a column per x for elements turned a quarter
(orientation 1 or 3), a row per y otherwise. An element whose label is a strip designation (optional
structure prefix, optional `-`, digits, letters, digits) starts a run of that strip; the following elements
of the line belong to it. Elements before the first strip label of a line form an inferred strip
(EW-IMP-005). Runs with one designation are one strip (EW-IMP-109 when drawn in several segments). A
terminal's designation is its first `UserText` dynamic text, else its non-strip label, else its sequence
number (EW-IMP-008).

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
auto-route; the IP chain-mode sort; external connection points; title-block template forms; QET 0.100
`terminal_strips` data; empty terminal designations (MF-08 needs an empty designation, which `Ident` cannot
hold); the starter-symbol exchange.
