<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC0-1.0
-->

# Golden project J-E-2416: source tables

This directory holds **Golden A**, the acceptance target of `docs/design/20-golden-project.md` (doc 20), as plain
CSV tables. `ew import csv` (crate `ew-import-tables`, FR-IMP-022) turns `tables/` into format-level records (build
path (a) of doc 20 §3.2); the checked-in fixture `golden/J-E-2416.ew/` (path (c)) follows once the JSONL writer of
WP-S1.09 exists.

## Licence and anonymisation

- Everything under `golden/` is **CC0-1.0** (ADR-0024, doc 19 §2.1 row L8). The CSV files cannot carry a header;
  `REUSE.toml` annotates them.
- The tables hold **no** customer, end-user or person names, addresses, project or drawing numbers, company logos
  or notice texts. Party, person and title-block values are the literal placeholders of doc 20 §1.2 (`<customer>`,
  `<end user>`, `<project ref>`, `<drawing id>`, `<engineer>`, `<checker>`, `<address>`, `<country>`,
  `<confidentiality notice>`), exactly as doc 20 §3.5 requires. The dates are the synthetic dates of doc 20.
- Device tags, part numbers, manufacturers, wiring and order codes are the anonymised technical content that doc 20
  already publishes; doc 20 is the **only** source of these tables. Nothing is taken from the private reference set.

## How the tables are made

The tables are **generated**, never edited by hand:

```text
python -I golden/J-E-2416/tools/tables_from_doc20.py          # writes tables/*.csv
python -I golden/J-E-2416/tools/tables_from_doc20.py --check  # exit 1 if a committed table differs
```

The script parses the markdown tables of doc 20 and maps them through per-table column mappings. Where doc 20 gives a
value only in prose, as a ▲ test input, or where a value is a choice of this fixture (a test-input cell position, a
junction position), the mapping writes it explicitly with a comment citing the section. Symbol and function keys
follow doc 07 §9.1 (the starter set mapped to the golden folios); CP patterns of black-box and terminal parts follow
doc 07 §9.2 and doc 03 §11. A test of `ew-import-tables` fails when a committed table differs from the script's
output, so a change to doc 20 and to the tables lands in one commit. The fixture choices that doc 20 gives no cell for
are registered as ▲U-38 … ▲U-48 in doc 20 §23.1.

## CSV dialect

- UTF-8 without BOM, NFC, LF line ends, one final LF, no empty lines.
- First line: the header. Delimiter `,`. A field is quoted with `"` exactly when it contains `,`, `"`, LF or CR, or
  begins or ends with a space; a `"` inside is doubled. Every row has as many fields as the header.
- Columns are found by name; their order is free. A missing **required** column refuses the table; a missing
  optional column reads as empty; an unknown column is reported and ignored.
- Rows are in **natural order of the `key` column** (FR-NUM-018, the order of `ew_mltext::natural_key`). Keys are
  unique per table.
- An empty field means "absent". Lists are `;`-separated without spaces around `;`. Booleans are `true` / `false`
  (empty is `false`). Decimals are canonical `Dec3` text (`230`, `0.75`, `2.5`): no exponent, no trailing zeros, at
  most three decimals. Dates are `YYYY-MM-DD`. Enumerations are the `snake_case` names of the `ew-schema` variants.
- Text cells (titles, descriptions, function texts) are language-neutral text (`MlText::neutral`).

## Identity

Every record emitted for a table row has the ID

```text
id = v5(ns, "{table}|{key}")      ns = lineage_namespace("csv", Fixed("golden"))
```

(ADR-0004 amendment 4, doc 14 §2.3, doc 20 §3.2), where `{table}` is the file stem (`devices`) and `{key}` the row's
`key` cell byte for byte. Every record carries `source_ref = {lineage, key: "{table}|{key}"}`. The import lineage
record itself is `v5(ns, "import_lineage|golden")` with importer `csv`, source name `tables` and anchor
`Fixed("golden")`.

Library items:

- Starter symbols, function definitions and forms are referenced **by key** (`ew.iec.mcb-1pn@qet10`, `ew.fn.ip`,
  `ew.frame.qet-a4-landscape-grid`) and embedded as the built-in copies (`BUILTIN_LIBRARY`, doc 05 §13.2:
  `item_id = v5(BUILTIN_LIBRARY, key)`, copy `id = v5(NS_LIBCOPY, library ‖ item)`). Golden A sits on the QET 10 px
  grid, so it places the `@qet10` edition (doc 20 §2.1 rule 4, doc 07 §9.1).
- Parts are project-local test-catalogue items: `library_id` = the project ID, `item_id = v5(ns, "parts|{key}")`.

## CP and endpoint notation

Cells that name a connection point, junction or interruption point use the notation of doc 20 §1.2:

| Form | Meaning | Example |
|---|---|---|
| `<device>:<designation>` | the CP of the device with that designation; unique over the device's functions | `S51:2`, `-102F2:Out1` |
| `<device>:<designation>#<n>` | the n-th CP (1-based, in function and CP order) with that designation | `-102G1:−#1`, `F55:N#2` |
| `<strip>:<terminal>/<sel>` | a terminal CP: `<sel>` is a clamp letter (`a`, `b`, `c`), a shaft `s<n>`, `rail` (rail foot), `field` or `internal` (the side facing the field, `Strip.field_side`, or the other one) | `-X1031:12/b`, `1X1:PE/field` |
| `ip:<function key>` | the single CP of an interruption point | `ip:L-1@101` |
| `@<junction key>` | a junction | `@J5` |

A function's CP list is its part pattern when the function row names a part template (`part`, `template`), else the
CPs of its FnDef; CP indexes are positions in that list.

## Tables

`R` = required column, `O` = optional. Record kinds are those of `ew-schema` v0.

### `project.csv` → `project` (doc 20 §4.1, §4.4) — one row

| Column | | Meaning |
|---|---|---|
| `key` | R | the object / cabinet ID `J-E-2416` (also `master.object_id`) |
| `document_title` | O | cover subtitle |
| `description` | O | project description |
| `long_description` | O | cover long description |
| `project_ref`, `drawing_id` | O | placeholders `<project ref>`, `<drawing id>` |
| `manufacturing_year` | O | integer |
| `created`, `edited` | O | dates |
| `supply_phases` | O | integer (a supply is emitted when this is set) |
| `supply_neutral`, `supply_pe` | O | booleans |
| `supply_earthing` | O | `tn_s` … |
| `supply_u_nom`, `supply_frequency` | O | Dec3 (V, Hz) |
| `supply_origin` | O | `external_refs` key |
| `control_voltage` | O | Dec3, unit V |
| `rated_consumption` | O | Dec3, unit A |
| `identifier_set` | O | identifier |
| `date_format`, `decimal_separator` | O | `dd_mm_yyyy`, `point` |
| `derivation_profile` | R | `2027.1` |

### `parties.csv` → `project.parties` (doc 20 §4.2)

`key` R (the role), `role` R (`customer`, `end_user`, `panel_builder`, `author`, `checker`, `engineer`), `name` O,
`street` O, `postcode` O, `city` O, `country` O, `personal_data` O (boolean). Rows in key order become the
`parties` list in that order.

- ▲U-47: the panel-builder party has no name, and there is no Author party (the author is the revision author).
- ▲U-48: the end-user address is stored as street `<address>`, country `<country>`.

### `structure.csv` → `structure_node` (doc 20 §4.3)

`key` R (`=P01`, `+J-E-2416`), `aspect` R (`function`, `location`, …), `code` R (without the sign), `aliases` O
(list), `parent` O (`structure` key), `description` O, `order` O (integer).

### `external_refs.csv` → `external_ref` (doc 20 §4.3, §15)

`key` R (the external point name, `J-E-206-L`), `location` R (`structure` key), `device` R (`JE-N1000-03`),
`connection` O (`L`, `N`, `PE`, `DP`), `project` O.

### `sections.csv` → `section` (doc 20 §4.3, §5)

`key` R (the number), `number` R, `folio_from` R, `folio_to` R, `title` O, `divider` O (boolean).

### `pages.csv` → `page` (doc 20 §5) — stored pages only

`key` R (folio label), `number` R, `label` O, `section` O (`sections` key), `kind` R (`title_page`, `bus_topology`,
`multi_line`, `assembly_overview`, `fluid`, `panel_layout`, …), `title` O, `form` R (form key), `function` O and
`location` O (`structure` keys of the page's `=` and `+`), `scale` O (`n` for 1:n), `grid` R (`qet10` or `mm4`).
Generated folios (2, 2.1, 100, 200, 900, 901, 901.1, 903 … 911) are not pages: dividers come from `sections.divider`,
reports from `report_blocks`.

### `report_blocks.csv` → `report_block` (doc 20 §16, §17.3)

`key` R, `kind` R (`toc`, `parts_summary` ▲U-44, `terminal_diagram`, …, `profile_tables`, `part_data`), `form` R (form
key), `section` O, `first_folio` O (required unless embedded), `title` O, `continuation` O (`sub_page`,
`next_folio`), `include` O (list of `devices` keys: the manual selection), `embedded_page` O (`pages` key),
`embedded_rect` O (`x0;y0;x1;y1` in native nm, required with `embedded_page`).

### `parts.csv` → project-local `part` copies (doc 20 §6.1, §14.1)

`key` R (the part number as printed, `PT 2.5-TWIN`), `manufacturer` R (name), `part_number` R, `kind` O
(`device`, `terminal`, `cable`, `enclosure`, `mounting_plate`, …), `type_designation` O, `description` O, `unit` O
(`piece`, `m`, `set`), `test_data` O (boolean: confidence `test` on every section).

- ▲U-45: the manufacturer is referenced as `LibRef v5(ns, "manufacturers|<name>")`.
- ▲U-46: the part use of `-3W1` is quantity 1, unit m.

### `part_cps.csv` → `part.cp_pattern` (doc 07 §9.2, doc 03 §11)

`key` R (`<part>:<idx>`), `part` R, `idx` R, `designation` R, `potential_role` O, `level` O, `side` O (`side1`,
`side2`), `clamp` O (`a` … `z`, `s<n>`, `rail`).

### `part_templates.csv` → `part.function_template` (doc 07 §4.2, §9.2)

`key` R (`<part>:<n>`), `part` R, `n` R (template index, the list order), `fndef` R (FnDef key), `main` O
(boolean), `cps` R (list of `part_cps` indexes).

### `cable_cores.csv` → `part.sections.cable_cores` (doc 20 §10)

`key` R (`<part>:<core>`), `part` R, `n` R (list order), `core` R, `colour` O, `csa` O, `shield` O, `pair` O.

### `devices.csv` → `device` (doc 20 §6) — every device except terminals

| Column | | Meaning |
|---|---|---|
| `key` | R | the Golden A tag as printed (`S51`, `-101F1`, `-201U3-Q1`, `-UC1-plate` for the plate) |
| `letter` | R | `TagSpec.letter` |
| `counter` | O | `TagSpec.counter` |
| `scheme` | O | `default` (empty), `child`, or `named:<name>` |
| `frozen` | O | frozen designation text (`S51`); `frozen_sign` O: `hidden` or `shown` |
| `parent`, `position` | O | parent device key; position inside the parent |
| `function`, `location` | O | `structure` keys of the device's `=` and `+` |
| `kind` | O | `psu`, `blanking_plate` |
| `parts` | O | list of `<part key>` or `<part key>*<qty>` (qty Dec3, default 1) |
| `function_text`, `process_tag` | O | manual function text; process tag |
| `state` | O | `used` (empty), `spare`, `reserved`, `not_fitted` |
| `strip_label_mode` | O | set on strips: `designation`, `function_text` (a strip aspect is emitted when any `strip_*` cell is set) |
| `strip_groups` | O | list of `<from>-<to>:<caption>` |
| `strip_field_side` | O | `side1`, `side2` |
| `cable` | O | `true` on cables (emits the cable aspect) |
| `cable_length` | O | `missing` (empty), `from_part`, or a manual length in mm (Dec3) |

### `terminals.csv` → `device` with the `terminal` aspect (doc 20 §9)

`key` R (`<strip>:<designation>`), `strip` R (`devices` key), `seq` R, `designation` R, `function_text` O, `parts`
O (as in `devices`), `state` O, `rail_bonded` O, `function` O, `location` O. Terminals carry letter `X` and no
counter.

### `functions.csv` → `function` (doc 20 §6, §15; doc 07 §9.1)

| Column | | Meaning |
|---|---|---|
| `key` | R | the device key for the main function; `<device>#<name>` for further functions; `<name>@<folio>` for interruption points |
| `device` | O | `devices` or `terminals` key; empty only for interruption points |
| `fndef` | R | FnDef key |
| `role` | O | `main` (empty), `aux` |
| `kind` | O | `normal` (empty), `black_box`, `ip` |
| `part`, `template` | O | the part template that supplies the CPs (template default 0) |
| `cp_aliases` | O | list of `<designation>=<alias>` (CP overrides, FR-DEV-007) |
| `ip_name`, `ip_direction`, `ip_external` | O | interruption points: name, `source`, `target` or `undirected`, `external_refs` key |

### `representations.csv` → `representation` (doc 20 §6.1, §15)

`key` R (the function key, `.2` … for further segments), `functions` R (list of `functions` keys), `page` R
(`pages` key), `symbol` R (symbol key), `cell` R (`<row><column>`, rows `A`–`H`, columns `0`–`16`), `dx`, `dy` O
(offsets in QET px from the cell centre). The symbol's anchor point is placed at the cell centre plus the offset
(doc 20 §4.4 "xref anchor policy"; an `origin` or `graphic_centre` anchor counts as the insertion point), on the
page grid. A position off the grid is snapped and reported in the cycle-6 code; by decision log ID-71 it becomes an
EW-IMP-002 error that drops the row (task FT-19).

- ▲U-38: valves sit in row D at columns c−3, c−1, c+1, c+3 around the module's auxiliary column c.
- ▲U-39: field devices sit in row G (valve) and row F (actuator), in the column of the driving valve.
- ▲U-41: the 1X1 terminals 1, 2, PE use dx −20, 0, +20 from the cell centre.
- ▲U-42: the external arrows J-E-206-L, -N, -PE use dy −20, 0, +20.
- ▲U-43: 1X1 has no strip segment (fixture choice, doc 20 §23.1).

### `connections.csv` → `connection` (doc 20 §8, §8.3; doc 03 §9.2)

| Column | | Meaning |
|---|---|---|
| `key` | R | the physical-wire ID it carries (`W13`); for the other drawn part of a wire through an IP pair `W<n>.<folio>`; for a junction arm that carries no wire ID `J<n>.feed` / `J<n>.1`; bridges `BR<n>.<i>` |
| `wire` | O | the physical wire of doc 20 §8.2 (documentation; not stored) |
| `page` | O | `pages` key; empty for bridges (strip editor) |
| `a`, `b` | R | endpoints in the notation above |
| `kind` | O | `wire` (empty), `core`, `bridge` |
| `core` | O | `conductors` key (kind `core`) |
| `bridge_group`, `bridge_kind`, `bridge_shaft` | O | bridges (kind `bridge`): group number, `plug_in` …, shaft (default 1) |
| `csa`, `colour` | O | the CDP: explicit cross-section (mm², Dec3) and colour |
| `show_marker` | O | boolean |

Wire IDs follow doc 03 §9.2: through an IP pair the target-side connection carries the wire ID; through a junction
each non-first arm carries the ID of the wire it closes (Star: first CP to every other; Chain: consecutive CPs).

### `junctions.csv` → `junction` (doc 20 §8.3)

`key` R (`J1`), `page` R, `cell` R, `dx`, `dy` O, `style` R (`dot`, `bevel`), `wiring` O (`star`, `chain`; empty is
the style's default), `sequence` R (list of `connections` keys, feed arm first).

- ▲U-40: the junction cells are J1 C11, J2 C13 (folio 101) and J3 C6, J4 D7, J5 D5, J6 D6 (folio 102).

### `conductors.csv` → `conductor` (doc 20 §10)

`key` R (`<cable>:<core>`), `cable` R (`devices` key), `core` R, `colour` O, `csa` O, `shield` O, `pair` O.

### `potentials.csv` → `potential_def` (doc 20 §7.3)

`key` R (the potential name), `at` R (endpoint), `ptype` O (empty: naming only), `voltage` O (Dec3 V), `ac_dc` O,
`frequency` O, `earthing` O (`pelv`, `selv`, `floating`), `circuit_class` O.

### `bonds.csv` → `bond` (doc 20 §7.4)

`key` R, `a` R, `b` R (endpoints), `kind` R (`pelv_0v_pe`, …), `via` O (`connections` key).

### Tables without a record kind in schema v0

These are part of the source of truth and are read, but `ew-schema` v0 has no record shape for them yet; the importer
reports each once (EW-IMP-004, info) and emits nothing:

- `colour_profile.csv` (doc 20 §7.1): `key` (row number), `voltage_class`, `function`, `potential_type`, `colour`,
  `code`.
- `size_profile.csv` (doc 20 §7.2): `key` (colour code), `control`, `mains` (Dec3 mm²).
- `revisions.csv` (doc 20 §4.1): `key` (label), `date`, `description`, `state`, `author`, `checker`, `changed_pages`
  (list).

## Not covered yet

Doc 20 §11 (PLC and bus: channels, the bus segment, the DP potential), §12 (fluid lines F1–F16), §13 (layout space,
carriers, placements) and §14.2 (configured products) need record kinds that `ew-schema` v0 reserves but does not
implement; their tables follow with those work packages. Also not covered yet: the MICO 4.10 13-14 signal-contact
template, FR-DEV-037 "supplied by others" and the -201U1:DP port. §14.1 (component BOM) is covered through `parts.csv` and the
part uses of `devices.csv` and `terminals.csv`; the BOM rows themselves are derived.
