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
| `potentials.csv` | `voltage; ac_dc; frequency; circuit_class` | `24VDC-2; 24VDC-3; 24VDC-4` | (empty) | §7.3 | — | L+, spare: the cell states the type only, no voltage or class |
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
- `geometry/frame.json`: the printed frame figures of the two frame forms with the QET title block, in mm on the
  sheet (3 decimals): sheet, outer, drawing area, header band, column and row edges, labels, title block, line width.
- `png/<folio>.png`: the sheet at 150 dpi. They are **candidates awaiting the first PNG approval (FT-10, an owner
  action)** and are compared with the §3.3 tolerance (at most 0.5 % of the pixels differ by more than 16 per
  channel), not byte for byte.
- `pdf/structure.json`: the writer's PDF structure of the whole page order (page count, `/PageLabels`, outline
  tree, link, plain-text-link and destination counts, embedded fonts), compared exactly (§3.3 "PDF structure"). The
  PDF hash is not compared until cross-references exist.
- Everything under `golden/` is CC0-1.0 (`REUSE.toml`, annotation L8).

**Known gaps for the FT-10 approval.** The candidates cover the 31 pages of `page_order` with the report engine
installed (`Session::query()`), so Prev/Next, the outline and the page labels include the generated pages. All 72
page wires are drawn (21 on 101, 19 on 102, 20 on 103, 12 on 106). Still open:

- **Router.** Wires use the provisional orthogonal router (doc 06 §5.4 owns the real one), so routes overlap: on
  101 the S51 → -101F1 L and N routes run collinear along row B; on 102 the feeds of -102G1 run along the box's top
  edge and over -102F1, whose tag "-102F1" is covered by a wire and its "1.5" marker; on 103 the side-1 feeds of
  -X1031 and -X1032 run collinear along the top of each segment and over the strip tags, and the column-1
  interruption points (0VDC, 24VDC-1 … 4, PE-AIR2, PE-0VDC, PE-AIR1) are joined by one collinear vertical route that
  reads as 24 V shorted to 0 V and PE; on 106 the twelve feeds of the CPX power boxes run along the box edges.
- **Report and divider pages.** The generated pages (2, 2.1, 901, 903 … 911) and the dividers 100, 200 and 900
  show frame and title block only; their content comes with the report forms in the scene. Doc 20 §5 counts 36
  pages; the page order has 31 until the report engine paginates every block (for example 901.1).
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
- **Interruption points.** The texts of two points that share a cell overlap: PE-1.1 and PE-1.4 at 103-G7,
  0VDC-AIR3 and PE-AIR2 at 106-B1, 0VDC-AIR1 and PE-AIR1 at 106-E1.
- **Folios 203–206.** Used valves print no text in the function-text slot: the process-tag fallback through
  `DeviceLink{Drives}` (§6.2) needs the `DeviceLink` record, which schema v0 does not implement.
- **Cross-references.** Cross-reference texts are not drawn, so the PDF has 0 link annotations (§3.4 row 18
  expects 64).
