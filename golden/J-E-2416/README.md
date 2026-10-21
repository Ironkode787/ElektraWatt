<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: CC0-1.0
-->

# Golden project J-E-2416: source tables

This directory holds **Golden A**, the acceptance target of `docs/design/20-golden-project.md` (doc 20), as plain
CSV tables. `ew import csv` (crate `ew-import-tables`, FR-IMP-022) turns `tables/` into format-level records (build
path (a) of doc 20 §3.2); the checked-in fixture `golden/J-E-2416.ew/` (path (c)) is generated from the same tables (see
"The checked-in project fixture" below).

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

Keys are hashed byte for byte, so a `key` cell must already be in Unicode NFC: a row whose key is not is dropped
with EW-IMP-002 (doc 14 §9 strict mode). Other cells are NFC-normalised when needed.

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

A function's CP list follows the **template-origin rule** (doc 03 §7 "CP instantiation"; `ew-derive` applies the same
rule): a function is template-origin when its device carries a part (`parts` of `devices.csv` / `terminals.csv`, first
match in that order) whose `part_templates.csv` declares a function with the same FnDef and the same role (`main` =
`true` for role main, `false` for aux; first matching template in `n` order). Its CPs are then the template's `cps`
entries of the part's CP pattern, and **the pattern index is the CP index** (terminals of PT 2.5-TWIN: `a` 0, `b` 1,
`c` 2, `s1` 3; PT 2.5-TWIN-PE adds `RF` 4). Otherwise a black box takes every CP of the first part use whose part has
a pattern (pattern indexes), and any other function the CPs of its FnDef. The optional `part` and `template` columns of
`functions.csv` state the expected origin; a row whose rule result differs is dropped with EW-IMP-002.

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
| `numbering_scheme` | O | key of the device-numbering scheme (default `page-based`) |
| `numbering_format` | O | the scheme's format, `{page}{letter}{counter}` (doc 20 §4.4); required with any other `numbering_*` column |
| `numbering_strip_format` | O | format of the rule `{select: {aspect: "strip"}}`, `{letter}{page}{counter}` |
| `numbering_child_separator` | O | `child_separator`, `-` |

The `numbering_*` columns become `ProjectSettings.extra["numbering"]` in the doc 03 §5.1 shape
`{default, schemes: [{key, format, rules, child_separator}]}`. Formats use the tokens `{letter}`, `{counter}`,
`{subcounter}`, `{page}`, `{column}`, `{row}` and never the product sign `-`.

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

`key` R (`<part>:<n>`), `part` R, `n` R (template index, the list order), `fndef` R (FnDef key), `main` O (boolean: the template function's role, main or aux; it must equal the role of the functions that take their
CPs from it), `cps` R (non-empty list of `part_cps` indexes).

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
| `part`, `template` | O | the expected template origin of the CPs (template default 0); checked against the template-origin rule above |
| `cp_aliases` | O | list of `<designation>=<alias>` (CP overrides, FR-DEV-007) |
| `ip_name`, `ip_direction`, `ip_external` | O | interruption points: name, `source`, `target` or `undirected`, `external_refs` key |

### `representations.csv` → `representation` (doc 20 §6.1, §15)

`key` R (the function key, `.2` … for further segments), `functions` R (list of `functions` keys), `page` R
(`pages` key), `symbol` R (symbol key), `cell` R (`<row><column>`, rows `A`–`H`, columns `0`–`16`), `dx`, `dy` O
(offsets in QET px from the cell centre). The symbol's anchor point — `SymbolDef::anchor_point` of `ew-schema`, the
exact integer anchor of its anchor policy (origin, named point, first slot of a kind, graphic centre with ties toward
zero; FR-SYM-010, D-14) — lands at the cell centre plus the offset, so `rep_anchor_cell` derives the table's cell; the
position itself is a point of the page grid. The target point must lie on the page grid (offsets in multiples of 10 px
on the QET grid): a position off the grid is never snapped, the row is dropped with EW-IMP-002 (doc 06 §5.7, decision
log ID-54).

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
the style's default), `sequence` R (list of `connections` keys, feed arm first). The position (cell centre plus offset) must lie on the page grid; otherwise the junction and every
connection ending at it are dropped (EW-IMP-002, never snapped).

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

## Stopgaps

- **Manufacturer.** A part's manufacturer is a `LibRef` to `v5(ns, "manufacturers|<name>")` whose name lives only in its
  `extra.key` member; no manufacturer copy is emitted yet. It stays this way until a manufacturer library copy exists
  (follow-up with the parts work package; recorded by the design lead).

## Test inputs (▲)

The table is generated by `tools/tables_from_doc20.py` from the fixture choices the script makes and is checked by its
`--check` mode.

<!-- test-inputs:begin -->

| Table | Column(s) | Key(s) | Value | Doc 20 | U-nn | Reason |
|---|---|---|---|---|---|---|
| `project.csv` | `supply_earthing` | all | `tn_s` | §4.1 | U-02 | the Supply cell names TN-S; the earthing system is a ▲ |
| `project.csv` | `supply_origin` | all | `J-E-206-L` | §4.3, §15 | U-06 | the supply origin is the external point of the L supply, named only in prose |
| `project.csv` | `derivation_profile` | all | `2027.1` | §4.4 | — | doc 20 names "the first shipped profile"; the fixture pins its version label |
| `structure.csv` | `description` | all | (empty) | §4.3 | U-05 | the Description column is commentary, not a stored value (the =P01 description is unknown) |
| `pages.csv` | `scale` | `902` | `10` | §13 | — | folio 902 is a view of the plate at 1:10, the scale is a ▲ |
| `report_blocks.csv` | `title; include` | all | `(text of the §16 cell)` | §16 | — | the whole 'Page description' column is a ▲: titles and the include filters are read from it unchanged |
| `parts.csv` | `kind` | `UK 5 N; USLKG 5; PT 2.5-TWIN; PT 2.5-TWIN-PE` | `terminal` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `NEBU-G78W5-K-2-N-LE5; 6XV1830-0EH10` | `cable` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `NSYPLA1074G` | `enclosure` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `NSYPMM107` | `mounting_plate` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `part_cps.csv` | `potential_role` | `550208:2` | `fe` | §6.1 row 19 | U-18 | pin 3 is the functional earth, printed 'PE': its role is a ▲ |
| `part_templates.csv` | `main` | `550208:0` | `false` | §6.1 rows 19, 28 | — | 550208 is the power block of -201U1 and -202U1, whose main function is the overview module |
| `cable_cores.csv` | `core` | `6XV1830-0EH10:SH` | `SH` | §6.1 row 18, §10 | U-21 | the shield core is named SH; the cores A, B and the shield carry no colour or cross-section |
| `conductors.csv` | `core` | `-3W1:SH` | `SH` | §6.1 row 18, §10 | U-21 | the shield conductor takes the shield core of the cable part |
| `devices.csv` | `frozen; frozen_sign` | `-XPE; -UC1; -UC1-plate` | `frozen = the tag without its sign (XPE, UC1, UC1-plate); frozen_sign = shown` | §2.1 rule 1, §6.1 rows 9, 31, 32 | U-34 | per-class numbering rules are not modelled yet: the tags are frozen overrides with the sign shown, so Golden B's include_frozen renumbers -XPE as §2.2 expects |
| `devices.csv` | `key` | `-UC1-plate` | `-UC1-plate` | §6.1 row 32 | — | doc 20 gives the mounting plate no tag ('mounting plate, child of -UC1'): the key is the fixture's name |
| `devices.csv` | `cable_length` | `-106W1; -106W2` | `from_part` | §10 | U-18 | the 2 m length is read from the part's type code; the source of the length is a ▲ |
| `potentials.csv` | `frequency` | `L; N; L-S; N-S; L-1; N-1; L-55; N-55` | `50` | §4.1, §7.3 | — | the Type cell of §7.3 gives no frequency; every AC potential takes the 50 Hz of the §4.1 Supply |
| `potentials.csv` | `ptype; voltage; ac_dc; frequency; circuit_class` | `N-55T; N-55S` | (empty) | §7.3 | — | N switched: naming only; doc 03 §9.4 derives the type, including 'switched', across the PassThrough{opens: true} pair |
| `revisions.csv` | `checker` | `V2.0` | (empty) | §4.1 | U-03, U-04 | the checker of V2.0 is unset in Golden A (unknown) |
| `representations.csv` | `dx` | `1X1:1; 1X1:2; 1X1:PE` | `-20; 0; 20` | §6.1 row 8 | — | test input (fixture choice): the three terminal symbols of 1X1 sit side by side in the strip's cell |
| `representations.csv` | `cell` | `-201U3-Q1 … -202U2-Q4` | `row D, columns c-3, c-1, c+1, c+3 around the module's aux column c` | §6.1 rows 33-60, §15 | — | test input (fixture choice): doc 20 names only the pages 203-206 of the valves, not their cells |
| `representations.csv` | `cell` | `-203Q1 … -203Q8` | `row G (§6.1 Main rep.), column of the driving valve` | §6.1 rows 61-68, §6.2 | U-07 | test input (fixture choice): the column of a process valve is that of the valve that drives it |
| `representations.csv` | `cell` | `-203Q1#actuator … -203Q8#actuator` | `row F, column of the driving valve` | §6.2 | U-07 | test input (fixture choice): the actuator is drawn above its process valve |
| `representations.csv` | `dy` | `J-E-206-L@101; J-E-206-N@101; J-E-206-PE@101` | `-20; 0; 20` | §15 | — | test input (fixture choice): the three mains points share the cell 101-G0 and are stacked |
| `representations.csv` | `dx; dy` | `interruption points that share a cell` | `k-th member of a coincident group (natural key order): +(0,0), +(0,20), +(0,-20), +(20,0) px` | §15 | — | ▲ test input (fixture choice): separates coincident interruption points; the cell is what doc 20 §15 asserts |
| `representations.csv` | `arrangement` | `-201U3-Q1 … -202U2-Q4; -203Q1 … -203Q8; -203Q1#actuator … -203Q8#actuator` | `ports` | §6.2 | — | test input (fixture choice): the symbol's arrangement without the Tag slot (FR-SYM-012) |
| `junctions.csv` | `cell` | `J1; J2; J3; J4; J5; J6` | `C11; C13; C6; D7; D5; D6` | §8.3 | — | test input (fixture choice): doc 20 gives the folio of a junction, not its cell |

<!-- test-inputs:end -->

## Not covered yet

Doc 20 §11 (PLC and bus: channels, the bus segment, the DP potential), §12 (fluid lines F1–F16), §13 (layout space,
carriers, placements) and §14.2 (configured products) need record kinds that `ew-schema` v0 reserves but does not
implement; their tables follow with those work packages. §14.1 (component BOM) is covered through `parts.csv` and the
part uses of `devices.csv` and `terminals.csv`; the BOM rows themselves are derived.

Also not covered yet: the 13-14 signal-contact function of the MICO 4.10 template (its CPs 13 and 14 belong to the
black box), "supplied by others" for the process valves -203Q1 … -203Q8 (FR-DEV-037), and the `DP` port of -201U1
(doc 20 §6.1 row 19).

## Expected page output (candidates)

`expected/` holds what the page output of the golden model must reproduce (doc 20 §3.1, §3.3). It is **generated**,
never edited by hand, by `crates/ew-api/tests/page_output.rs`:

```text
EW_BLESS=1 cargo nextest run -p ew-api --test page_output     # rewrites expected/ from the model
cargo nextest run -p ew-api --test page_output                # compares (no EW_BLESS)
```

- `text/<folio>.csv`: every glyph run of every exported layer on the sheet, in paint order, one file per folio of
  `page_order` (dialect as above). Columns `seq,layer,entity,text,x_mm,y_mm,size_pt,angle_mdeg,box_x0_mm,box_y0_mm,
  box_x1_mm,box_y1_mm`. The **anchor** of §3.3 recorded is the run origin (the baseline start, `x_mm,y_mm`, 3 decimals)
  together with the run's layout box; `size_pt` has 2 decimals; `angle_mdeg` is the run's own angle in [0, 360000).
- `rows/<block>.csv` (903 … 907, 909): the printed rows of the connectivity report blocks, written by
  `crates/ew-api/tests/connectivity_reports.rs` (`EW_BLESS=1 cargo nextest run -p ew-api --test
  connectivity_reports`). First column `row` is `data` or `heading`, then the columns of the kind as printed (neutral
  language). They are read from a `Q` without the report engine and over the golden tables **with** the colour and
  size profiles of doc 20 §7.1, §7.2 (the page goldens have none yet). Same dialect as above. The kinds also
  derive and paginate inside the report engine's `Q`, so their page text files carry the rows of the golden
  tables without those profiles.
- `geometry/frame.json`: the printed frame figures of the two frame forms with the QET title block, in mm on the
  sheet (3 decimals): sheet, outer, drawing area, header band, column and row edges, labels, title block, line width.
- `png/<folio>.png`: the sheet at 150 dpi. They are **candidates awaiting the first PNG approval (FT-10, an owner
  action)** and are compared with the §3.3 tolerance (at most 0.5 % of the pixels differ by more than 16 per
  channel), not byte for byte.
- `pdf/structure.json`: the writer's PDF structure of the whole page order (page count, `/PageLabels`, outline
  tree, link, plain-text-link, dropped-`.notdef`-glyph and destination counts, embedded fonts), compared exactly
  (§3.3 "PDF structure"; 64 links, one per listed cross-reference item); `pdf/sha256.txt` holds the SHA-256 of
  the whole PDF.
- Everything under `golden/` is CC0-1.0 (`REUSE.toml`, annotation L8).

**Known gaps for the FT-10 approval.** The candidates cover the 33 pages of `page_order` with the report engine
installed (`Session::query()`), so Prev/Next, the outline and the page labels include the generated pages. All 72
page wires are drawn (21 on 101, 19 on 102, 20 on 103, 12 on 106). Still open:

- **Router.** Wires use the provisional orthogonal router (doc 06 §5.4 owns the real one), so routes overlap: on
  101 the S51 → -101F1 L and N routes run collinear along row B; on 102 the feeds of -102G1 run along the box's top
  edge and over -102F1, whose tag "-102F1" is covered by a wire and its "1.5" marker; on 103 the side-1 feeds of
  -X1031 and -X1032 run collinear along the top of each segment and over the strip tags, and the column-1
  interruption points (0VDC, 24VDC-1 … 4, PE-AIR2, PE-0VDC, PE-AIR1) are joined by one collinear vertical route that
  reads as 24 V shorted to 0 V and PE; on 106 the twelve feeds of the CPX power boxes run along the box edges.
- **Report pages.** The generated pages (2, 2.1, 901, 903 … 911) draw their `tables` layer (FT-53): the house
  report table of doc 20 §16 (x 16.31 to 277.27 mm, rows of 6.52 mm, header #C0C0C0, 8.88 pt), column widths by
  the columns' weights (no data area in the golden report forms, so the widths are the kinds' choice: only the TOC's
  follow the reference). A page of a block that is not derived prints its reason in one row. Row links
  (FR-REP-042) and the cover tables are not drawn yet. The dividers 100, 200 and 900 show frame and title block only.
- **Page count.** Doc 20 §5 counts 36 pages; the page order has 33 (907 paginates into 907, 907.1 and 907.2).
  Three pages are missing for reasons outside the table drawing: 901.1 (the TBD parts of BOM rows 24 to 27 are not
  in the tables, so 901 holds its 23 rows; FT-110) and 908.1 and 908.2 (the PLC overview has no channel records;
  FT-111).
- **TOC pages.** The title blocks of 2 and 2.1 print no `=P01` and `+J-E-2416` (14 rows against 16 on 903): the TOC
  pages' structure binding in `ew-derive` is still open (doc 20 §5 gives every page both signs).
- **Folio 1.** The cover content of doc 20 §17.3 is not drawn.
- **Title block.** The Author cell is empty, `ew.titleblock.qet-a4` has no Notice cell yet, and the logo and the
  revision label are not bound.
- **Black boxes.** The box outline is the 10 × 6 grid-step symbol rectangle; the inner graphics ("AC ~ / 24 VDC"
  on -102G1, the "24VDC" and "GND" fields of -102F2) and the part-number slot are not filled yet.
- **Strip segments.** The dashed segment rectangle does not stretch over the terminals the segment draws.
- **Folio 101.** The terminals of 1X1 print only their designations (ID-162), but "PE" of 1X1:PE still crosses the
  conductor of terminal 2 (the terminals are 20 px apart); the external-arrow text "J-E-206-PE" starts 0.27 mm left
  of the drawing area.
- **Interruption points.** The texts of two points that share a cell overlap: PE-1.1 and PE-1.4 at 103-G6,
  0VDC-AIR3 and PE-AIR2 at 106-B1, 0VDC-AIR1 and PE-AIR1 at 106-E1.
- **Folios 203–206.** Used valves print no text in the function-text slot: the process-tag fallback through
  `DeviceLink{Drives}` (§6.2) needs the `DeviceLink` record, which schema v0 does not implement.
- **Cross-references, folio 101.** The three external texts ("+J-E-206 / JE-N1000-03") start left of the frame
  and overlap the next point's name; the N-1 wire crosses "102-A1".
- **Cross-references, folio 103.** The texts overlap neighbouring arrows: 0VDC and 24VDC-1, 24VDC-AIR4 and
  24VDC-AIR2, 24VDC-AIR2 and 24VDC-AIR3, PE-AIR2 and PE-0VDC, PE-0VDC and PE-AIR1, PE-1.3 and PE-1.1; "101-G15"
  and "106-A1" collide at y 153.87 mm.
- **Cross-references, folio 106.** "103-B14" and "103-G1" collide at B1, "103-A14" and "103-G1" at E1 (two points
  share each cell).

## Connectivity report rows

`expected/rows/<block>.csv` (see above) holds what the four connectivity kinds print for the golden model. They were
compared with doc 20 by hand, table by table; the counts agree (907: 44 wires, a heading and 6 bridges = 51 rows
on three pages of 23, 23 and 5 rows, as doc 20 §16 counts them since decision log ID-234; 906: 3 cables, 13 cores and
4 headings = 20 rows; 903, 904 and 905: 3, 20 and 5 terminals), and the bridges BR1 … BR6 with their terminals and potentials, the terminals' groups, parts, states, bridges and targets of
doc 20 §9, the cable cores with colours and cross-sections, and the wire ends of doc 20 §8.2 except the points below.
The expectations were not adjusted to the output; each difference is a gap of the derivation or the model, listed
here, with the follow-up task of the [work breakdown](../../docs/plan/02-work-packages.md) Appendix B that closes it:

- **Wire list (907).**
  - Rows are in reading order (page, then the frame-grid cell of an end where a representation draws it, then the
    tags and CPs, then the wire ID), not in the W1 … W44 order of doc 20 (W is a fixture label). The numbering order
    of FR-NUM-028 needs the cells of terminals in strip segments, which no query gives (FT-100).
  - The "No." column is empty: the wire numbering of doc 08 §5 has no query yet (FT-100).
  - From and To follow doc 08 §3.8 (FR-CON-040) without its flow-distance step (`ew-derive` has no per-CP distance):
    on one page the end with the smaller anchor cell is From. W1 to W3 (the supply to 1X1) and W4, W5 (1X1 to S51)
    print the other way round (FT-101).
  - Repeated CP designations print plainly (`-101F1:N`, `-102G1:+`, `-102G1:−`, `-102F2:GND`), without the `#1`,
    `#2` of doc 20; a terminal end prints the clamp in the CP column (`-X1031:1` and `b`), not as `1/b` (FT-102).
  - The Source column prints `cdp`, `rule`, `core`, `bond` or `built-in` (size first, then colour, joined when they
    differ), as doc 20 §8.1 now states; its "(rule)" and "(profile)" are both the source `rule`.
  - The Via column names the cable and core and the interruption points; doc 20 also names the junction (J1 … J6),
    "chain" and the EF case, which are not derived values.
  - W33 to W44 (the cores of -106W1 and -106W2 and the PE legs) print the source `rule/core` or `rule`: the size
    comes from the size profile, while doc 20 §10 says core cross-sections come from the cable part (the
    conductors of the fixture carry none; FT-107).
  - The page goldens are made without the colour and size profiles (the importer does not emit them yet, decision
    log ID-90): their colours and sizes are the built-in defaults. `rows/907.csv` has the profiles.
- **Potential overview (909).** 18 potentials and B1 (19 rows): the PROFIBUS `DP` potential of doc 20 §7.3 needs the
  `Bus` connections of the PLC work package (FT-105). The rows are in natural order of the name, not in the order of doc 20.
  Members are counted as doc 20 counts them (device CPs, a terminal once, external points); the rails and the plate
  "by declared bonding" are not in the model (PE: 14, doc 20 lists 13 and the bonded carriers). The default colour and
  size are the profile's values for the potential's first wire that has one: empty for the size of `+24V` and
  `+24V-F` (every wire of them carries a CDP size), and for PE
  the size 0.75 of its first wire although doc 20 says "by class". N-55T and N-55S print the type of the partition,
  not the "N switched" of the flow (N-55S has an empty type), and both have an empty voltage and class where doc 20
  §7.3 says 230 V AC (the fixture's definitions are naming only; the overview is to print the values derived
across the pass-through pair, FT-108). The columns "switched", "protected by" and "bonds" of doc 08 §10 are not
  printed. Pages compress consecutive pages to a range (`102–106` for 102, 103 and 106, FR-REP-046), where doc 20
  lists them. The bond row has a name and pages only.
- **Terminal diagrams (903 … 905).** The two sides are printed in side order (side 1, side 2); doc 20 §9 puts the
  field side of 1X1 first (`Strip.field_side` is not applied). Targets carry no wire number (W29 …) and no `#n`
  (FT-103, FT-102).
  Multi-level terminals would need one row per level.
- **Cable overview (906).** Cores are in natural order of the core name (BK, BN, BU, GNYE, WH), not in pin order
  (BK, BU, GNYE, WH, BN): pins are not modelled (FT-104). The length of -106W1 and -106W2 prints "from part" (the part
  has no length value, ▲U-18; FT-106), -3W1 "length missing". The bus cable -3W1 has no ends (its line is a `Bus` connection, not a
  physical wire; doc 20 gives -3X1 In to the external end), and the connector designation of -201U1 X1 is not
  derived.

## The checked-in project fixture (path (c))

`golden/J-E-2416.ew/` is the golden project as an ElektraWatt project directory (doc 05 §2.1): the records of the
CSV bootstrap (path (a)) written by `ew-format`, as generation 1. It is generated and never edited by hand; a
change to `tables/` regenerates it in the same commit (doc 20 §3.2):

    EW_BLESS_GOLDEN_FIXTURE=1 cargo nextest run -p ew-api --test golden_fixture --locked -E 'test(the_fixture_is_byte_identical_to_the_bootstrap)'

The filter runs the comparing test alone: nextest runs the tests of a file in parallel, and the other tests of
`golden_fixture.rs` read the fixture while bless writes it. Bless mode only writes files; it never deletes one. A
file the tables no longer produce is named in the failure message and removed by hand.

It holds 164 files (about 0.8 MB): `manifest.json`, `project.jsonl`, `structure.jsonl`, `report-blocks.jsonl`, 13
page files in `pages/`, 79 of the 256 anchor shards in `devices/` and 68 library copies in `lib/` (fndefs, forms,
parts, symbols). It holds no `revisions.jsonl`, `waivers.jsonl`,
`layout/`, `lib/manufacturers/` or `assets/` yet, because the bootstrap emits no records of those kinds: the
revision, waiver, layout and manufacturer kinds are reserved in `ew-schema` v0.

The test `the_fixture_is_byte_identical_to_the_bootstrap` (`crates/ew-api/tests/golden_fixture.rs`) regenerates
the files and compares them with the directory byte for byte. It ignores only the manifest's `generator` member
(doc 05 §17.1). The other tests of that file open the fixture with `ew-store` (edit, and read-only without a
lock) and check the findings, the counts and the portable file names.

## Variants and command logs

`variants/*.ewcmd` are command logs (doc 17 §3.5): JSON Lines, a header and then one command per line. Format as
built (`crates/ew-api/src/cmdlog.rs`):

```text
{"$schema":"ew.cmdlog/1","label":"Golden B, step B2: …"}
{"cmd":"page.renumber","params":{"scope":{"section":{"ref":"section:1"}},"start":101,"step":1,"keep_gaps":false}}
```

- Line 1 is the header (`$schema` is `ew.cmdlog/1`; `label` is free text; other members are ignored). Every other
  line is `{"cmd": <command name or alias>, "params": {…}}`, the serialised form of `AnyCommand`, with the
  parameters of the command's JSON Schema (`crates/ew-commands/schemas/`). No other member is allowed.
- A `{"ref": "<kind>:<value>"}` object inside `params` is replaced just before its line runs: `section:<number>`
  by the section's ID (the section IDs are UUIDv5 of the tables, so a script cannot spell them portably; this
  reference kind is an implementation addition that doc 17 §3.5 does not list yet) and `id:<uuid>` by the text
  once a record with that ID exists. Since cycle 12, `tag:<tag>` resolves to the one device whose derived tag
  prints `<tag>` (short form such as `-X1031` or `S51`, or the full designation) and `folio:<label>` to the one
  drawn page with that folio label; matching is exact, against the then-current model (the `find` surface of
  doc 17 §3.5, FT-156, is not needed for it). A reference that names nothing or more than one record, such as an
  unnumbered `-101A?` or a duplicate terminal designation, cannot be resolved; such targets use `id:<uuid>`. A
  reference that cannot be resolved stops the replay with exit code 2 (`CmdLogError::Reference`, with the number
  of lines committed before it).
- `Session::run_script(&CmdLog, file)` replays a log: each line commits as its own transaction with origin
  `cmdlog:<file>` (bulk commands as preview then commit), so one `undo` takes back one line. Every command is
  looked up and checked against its exposure class before the first line runs. Exit codes (doc 17 §3.2,
  `CmdLogError::exit_code`): 1 for a line refused by a prevent rule or by the command's domain rule; 2 for a
  malformed log, an unresolvable reference, a refused command, and a line with invalid parameters, a missing record
  or an internal error.

| File | Doc 20 | State |
|---|---|---|
| `b2-renumber-pages.ewcmd` | §2.2 step B2: `page.renumber` of section 1 from 101 (folio 106 becomes 104) | tested by `crates/ew-api/tests/golden_b.rs`: exactly the 2 tags (-106W1, -106W2) and 14 cross-reference texts of §2.2 change, the page order follows, the topology (potential partition, 91 connection records) is identical, one undo restores the records byte for byte |
| `b1-page-based-tags.ewcmd` | §2.2 step B1: `project.settings.set` drops the strip rule of `settings.numbering`, then `numbering.renumber` of the project with `include_frozen: true` (and `only_nonconforming`, see below) | tested by `crates/ew-api/tests/golden_b.rs`, alone and followed by B2: the 10 rows of the tag map hold, no cross-reference text changes, the topology is identical, one undo per line returns to Golden A byte for byte |
| `c-fix.ewcmd` | §20.2, the steps that existing commands can do (see "Golden C as built") | replayed over the private reference import by `crates/ew-api/tests/golden_c.rs` (corpus-gated): all 49 lines commit |

**Step B1 as built.** `include_frozen: true` unfreezes the frozen devices that the scheme numbers (`TagSpec.frozen`
is cleared), and dropping the strip rule makes -X1031 and -X1032 print -103X1 and -103X2. 1X1 has no strip segment
(▲U-43): a strip without a segment takes its main representation, and so its page, from its first placed terminal
(lowest page, then lowest designation, then smallest ID; `ew_derive::MainRep` and `ew_commands::numbering`), so 1X1
becomes -101X1 and -XPE, the second X device on folio 101, -101X2. In Golden A 1X1 is frozen, so its printed tag and
every page output stay as they are. The log keeps conforming counters (`only_nonconforming`), because under the interim smallest-ID
main-representation rule the main representation of -X1031 is its second segment (103-A10), which would order it
after -X1032; doc 20's `order: [column, row]` and scheme-format parameters do not exist (the format change is the
settings line).

## Golden C as built

Golden C (doc 20 §2.3, §20) is the pilot's original QElectroTech file of the reference set, imported in lenient
mode. The file is private (doc 20 §3.5): the repository holds only the expected migration-report counts
(`expected/c-migration-report.csv`), the fix script (`variants/c-fix.ewcmd`) and the corpus-gated test
`crates/ew-api/tests/golden_c.rs`. The test finds the source in `EW_PRIVATE_CORPUS` by its SHA-256 (the import
lineage of a file without a QET project UUID is anchored on that hash, so the `id:` references of the fix script
apply to that exact file). It prints counts, Golden A's own strings and sanitised imported strings only, and it
skips when the variable is unset:

```text
EW_PRIVATE_CORPUS=<dir> cargo nextest run -p ew-api --test golden_c --run-ignored all --no-capture
```

**Migration report (cycle 12).** `expected/c-migration-report.csv` has one row per code: the MF rows of doc 20
§20.1, doc 20's expected count, the count produced as built, and a status. The test asserts the produced counts.

- *Holds:* MF-09 (8 EW-TERM-002 groups), MF-10 (1 EW-IMP-109), MF-22 (1 EW-IMP-006; the substituted family is not
  Tahoma, but the count holds).
- *Differs:* MF-03 lists 4 frozen labels (S51, F55, T55, E55) against 7: 1X1 is a strip and strips are not listed
  (decision log ID-277), and S56 and E56 are not elements of this file. MF-04 gives 2 EW-IMP-104 entries (one
  revision value differs on one folio; the per-folio description of folio 101 is reported as a folio-property
  conflict) against "4 formats" (the CSV's `doc20 = 1` reads "4 formats on 18 folios" as one condition, an
  interpretation).
- *Doc expectation does not fit this file:* MF-01. The stored labels of -3W1 and -3X1 equal their formulas, so no
  EW-IMP-101 arises; the stale -4W1/-4X1 can only be in the nomenclature table (MF-21).
- *Not detected yet (importer gaps):* MF-02 (FT-130), MF-06 (FT-139), MF-07, MF-18, MF-19 (FT-132), MF-08 (it
  surfaces inside EW-IMP-008, as the terminal numbered "8" for want of a designation, FT-141; the CSV lists MF-08 on
  both rows), MF-11 (FT-135), MF-12 (FT-131), MF-21 (FT-133). The live-rule
  rows MF-13, MF-16, MF-23, MF-24, MF-25 and MF-27 need imports through the pipeline (FT-143); the text "3-B0" itself
  is reproduced. MF-05 (EW-IMP-105), MF-14 (EW-IMP-110), MF-15 (EW-IMP-111), MF-17 (EW-IMP-121), MF-20
  (EW-IMP-112) and MF-26 (EW-IMP-114) have no detection in the importer and no follow-up task yet.
- *Not in §20.1:* EW-IMP-004 (94), EW-IMP-005 (3 fallback strips), EW-IMP-008 (13), EW-IMP-116 (1), EW-IMP-119 (16)
  and EW-IMP-122 (6).

**Fix script.** `variants/c-fix.ewcmd` (49 lines, every line commits) does what existing commands can do: the
numbering scheme of doc 20 §4.4 (`project.settings.set`); -X1031 and -X1032 unfrozen onto the strip rule with their
label mode and groups; terminal designations 1 … n with the old labels as function texts, spare states and rail
bonding (MF-08, MF-09); the page kinds of folios 1, 3, 201 … 206 and 902 and the layout scale; the eight valves of
folio 203 lettered Q and renumbered (-203Q1 … -203Q8). Unique targets use `tag:` and `folio:` references, the
others `id:` references. A `tag:` reference matches the printed short tag or the full designation: the
strips' frozen tags print `X1031` (hidden sign), so `tag:-X1031` resolves through the full designation, which
the import (without structure nodes) prints as `-X1031`. Once structure nodes exist the full form carries the
structure prefix, and a script must use the printed short form or the full designation with its prefix. The steps
of §20.2 that need a command that does not exist:

| §20.2 step | Edit it would make | Missing |
|---|---|---|
| 1, MF-16 | bond `pelv_0v_pe` between -102G1:−#1 and -X1032:2 via W27 | a bond command |
| 1, MF-04 | revisions V1.0 and V2.0, per-page values dropped | a revision-create command |
| 1, MF-02 | structure nodes =P01 and +J-E-2416 assigned to every device and page | a reference to a node created earlier in the log (`structure.create_node` exists) |
| 1, MF-11 | core colour BR → BN on -106W1, -106W2 | cables with conductors in the import (FT-135) |
| 1, MF-05 | `description_override` of folio 101 removed | the import keeps the description in `Page.extra` (EW-IMP-104), so there is no override to drop; the importer must map it to `description_override` with EW-IMP-105, after which `fields.set` (`description_override` = null) does it |
| 1, MF-12 | manufacturers mapped to the master; "UK5N / ULSKG5" split into UK 5 N (terminals 1, 2) and USLKG 5 (PE) | parts from element information (FT-131) and a manufacturer master |
| 1, MF-13 | anchor policy "graphics" for the bus-port symbol of -201U1 on folio 3, so the master prints "∞ 3-B2, 106-G3" | the anchor is a member of the imported symbol copy (`SymbolDef.anchor`); `project.settings.set` has no anchor member and no command edits a symbol copy |
| 2, MF-06 | the title-block "Customer" cell bound to the party role of ▲U-01 | title-block forms from the QET template (FT-139) and a party-binding command |
| 3 | starter symbols instead of the QET-import symbols | `rep.exchange_symbol` (catalogued, not built; FT-127) |
| 4 | sections 1, 2, 9 with dividers; folios 100, 200, 900 removed; report blocks 2, 901, 903 … 911 | moving a page into a section (`page.move`, not built); the frame form without grid is not in the imported library and no command embeds a library copy |
| 5 | starter parts on every device with part data | part records in the project and a part-assignment path |
| 6 | the valve-terminal assembly with its 28 child valves bound to channels | an assembly command |
| 7 | layout items bound, rails and carriers | layout-page items in the import (FT-134) |
| 8 | text styles (`tag.main` …) | a text-style command |

**C fixed against Golden A.** `golden_c.rs` prints one row per comparison; `HOLDS` lists those that hold and are
asserted. Holding: page titles (13 drawn folios), the tag source of the 35 common device tags, the function texts
and states of the 28 common terminals, and no tag Golden A has is printed twice by C (asserted with their
common-key counts, so a shrinking key set fails). The differences, by class:

- *Source limits of this file:* S56, E56, -XPE and the enclosure -UC1 with its plate are not elements of the file;
  the source of the interruption point PE-1.1 on folio 101 (101-G15) is not either, so its partner on 103-G6 prints
  no text and C has 61 cross-reference texts where the reference prints 63; the file stores no bridge (no
  `<terminal_strips>`, no bridge element), which explains 19 missing bridge connections: Golden A without its
  bridges has 24 potentials, so the bridges account for +6 against A's 18, and C's other connections for −3 (C: 21).
- *QET-model limit (doc 14 §4.2, §4.13):* 33 unlabelled graphic elements (folios 3, 101, 102, 106, 902) and the 10
  terminals of the fallback strips X?101 … X?103 are QET elements where Golden A has no device (hiding them is
  FT-144). The cross-reference cells of elements lying exactly on a frame line now print as QET prints them (102-C15,
  103-G6, (202-E1)): the importer moves such an element into QET's upper row or left column (README of
  `ew-import-qet`, owner ruling of the cycle-12 review).
- *Importer gaps:* no cable cores (20, FT-135), so C has 106 plain wires against 52 wires, 20 cores and 19 bridges,
  and 1 of 17 comparable potentials with equal members; the external points "+J-E-206 / JE-N1000-03" (FT-138); the
  page texts of every shared folio differ (free texts and shapes FT-134, QET-import symbols FT-127, title block
  FT-139); 2 and 901 stay drawn pages (FT-133).
- *Fix-script gaps (missing commands, table above):* "∞ 3-B0, 106-G3" against "∞ 3-B2, 106-G3" (MF-13, step 1:
  doc 20 §20.3 requires C fixed ≡ A, so the anchor quick fix belongs to the script); the 28 child valves and their
  cross-references (step 6); the dividers and report pages 2.1, 903 … 911 with their rows (step 4).

## Acceptance runner

`cargo xtask acceptance [--milestone] <report.md>` runs the gate-M1 runner (doc 20 §21, §22; doc 18 §3.10) over the
golden project with the oracle on and writes a Markdown table with one row per named check, a JUnit file
(`target/nextest/ci/junit.xml`, nextest profile `ci`) and the exit status. It runs the `#[ignore]`d test `gate` of
`crates/ew-api/tests/acceptance.rs` (about 40 s in a debug build); a stale report is deleted first.

A row has one of four statuses:

| Status | Meaning | Blocks the gate |
|---|---|---|
| `pass` | the criterion holds | no |
| `fail` | it does not hold; the reason says what differs | yes |
| `not_built` | the feature the criterion needs does not exist yet (the reason names it) | yes, exactly like `fail`, listed separately |
| `not_applicable` | it cannot be evaluated headless or at this delivery (GUI, pilot, an external tool that is not installed, a milestone-only measurement); the reason is required | no |

A criterion with a part that holds and a part that is missing has one row per part. The runner evaluates AC-01 …
AC-13, AC-18 and AC-20, lists AC-14 … AC-17 and AC-39 with their status and reason, and covers the §19 rows gated M1.
It compares with the committed files through the functions that write them (`tests/page_output/`,
`tests/golden_common/`): the text CSVs, the frame geometry, the PNGs (15 folios at 0.5 %), the PDF structure and
SHA-256, and `expected/csv/`. AC-17 (open, check and PDF export times) is `not_applicable` outside a milestone run;
`--milestone` builds in the release profile with `EW_PERF_ASSERT=1` and measures it.

The ordinary tests of the file run the runner once, without the raster comparison and the measurements, and compare
its report with the committed `expected/acceptance_headless.md` (so two runs must print the same text); they also keep
`expected/csv/` current:

    EW_BLESS=1 cargo nextest run -p ew-api --test acceptance --locked -E 'test(headless_report) | test(expected_csv)'

`expected/csv/` holds the report rows of the acceptance pairs (doc 20 §3.1): `terminals_1X1.csv`,
`terminals_X1031.csv`, `terminals_X1032.csv` (903 … 905), `cables.csv` (906), `wires.csv` (907) and
`potentials.csv` (909), byte-equal to `expected/rows/<folio>.csv`. The runner compares the derived rows with them.
The SQL half of each pair (`acceptance/*.sql` over the `api_v1` views) is `not_built`: SQL projection pending
(WP-S1.10).

The headless report of this revision: 83 entries, 41 pass, 10 fail, 26 not_built, 6 not_applicable (the full run adds
the raster comparison as a pass). The failures are the open gaps listed above (page count 33 against 36, the
revision label and structure cells of the title blocks, the 23-row parts list, the DP potential, the cover tables, the
ratings and part numbers on 101 and 102, the missing uniqueness invariant for terminal designations). An empty terminal designation refused at the parameter layer counts as refused for D-11 (ADR-0008).
