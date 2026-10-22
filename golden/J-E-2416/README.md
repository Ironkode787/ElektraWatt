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
output, so a change to doc 20 and to the tables lands in one commit. Doc 20 §8.2's Via column is part of this
contract: the script reads only the junction tokens J1 … J6, "idem", "IP …" and "; 2 wires on …" there, and the
printed notation of the wire list ("star at", "chain on") lives in doc 20's prose and in `expected/csv/wires.csv`
(decision log ID-379). The fixture choices that doc 20 gives no cell for
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

**Exception: channels.** A row of `channels.csv` has the ID `ew_ids::v5::plc_channel_id(module_id, number)` =
`v5(module, "channel" ␟ number)` (doc 10 §2.4, ADR-0004 amendment 2), not `v5(ns, "channels|{key}")`: the identity of
a channel is its module and number, so two branches that add the same module mint the same channel IDs. The record's
`source_ref.key` is still `channels|{key}`; the `key` column is only a label (`<module key>:<number>`) that must be
unique. Two rows with the same module and number are a duplicate channel: the second is dropped with EW-IMP-002.

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
(`device`, `terminal`, `cable`, `enclosure`, `mounting_plate`, `configured`, …), `type_designation` O, `description` O, `unit` O
(`piece`, `m`, `set`), `test_data` O (boolean: confidence `test` on every section), `tech_data` O (typed technical data,
below).

- ▲U-45: the manufacturer is referenced as `LibRef v5(ns, "manufacturers|<name>")`.
- ▲U-46: the part use of `-3W1` is quantity 1, unit m.
- The 29 rows are the 27 components of §14.1 and the two configured products of §14.2 (kind `configured`, unit
  `set`, the order code as part number and key). No device uses a configured part: `assembly_configs.csv` names
  them.

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
| `tech_data` | O | typed technical data of the device (below) |
| `plc_station` | O | `plc_config_projects` key: the device is a PLC station of that configuration project (emits the `plc_station` aspect; doc 20 §11.2 "Station", doc 10 §2.3) |
| `plc_role` | O | `StationRole` in snake_case (`cpu`, `head_station`, `io_device`, `valve_terminal`, `io_link_master`, `drive`, `external_partner`); required when `plc_station` is set, malformed without it |
| `plc_order` | O | integer: the station's address-allocation order (`PlcStation.order`); malformed without `plc_station` |
| `plc_module` | O | boolean: the device is a PLC module (emits the default `plc_module` aspect; doc 20 §11.2 "Modules") |

**Technical data** (`tech_data` of `parts.csv` and `devices.csv`, FR-DEV-009, U-32) is a `;`-separated list of
`<key>=<value>` items; a value is a quantity `<decimal> <unit>` with the unit in the ASCII spelling of the format (`10 A`,
`15 degC`) or an identifier (`C`). The slots print the values of the device's first part use together with the device's
own, which win by key (`ew_derive::tech_data_text`): "C10" for a `characteristic` and a `rated_current` in ampere
(the MCB notation), otherwise `<value> <unit>` ("40 A", "15 °C").

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
| `kind` | O | `wire` (empty), `core`, `bridge`, `bus` |
| `core` | O | `conductors` key (kind `core`) |
| `bridge_group`, `bridge_kind`, `bridge_shaft` | O | bridges (kind `bridge`): group number, `plug_in` …, shaft (default 1) |
| `csa`, `colour` | O | the CDP: explicit cross-section (mm², Dec3) and colour |
| `show_marker` | O | boolean |
| `cable` | O | `devices` key of a cable (kind `bus` only: `ConnectionKind::Bus(BusLine { cable })`, doc 10 §2.7, §3.8); a `cable` cell on another kind is malformed, a key that is not a cable device is unresolved |

A `bus` connection joins two CPs of a bus segment. A bus cable is drawn single-line (FR-CBL-008, doc 20 §10): `DP.1`
(page `3`, `ip:DP@3` to `-3X1:A1`, cable `-3W1`) is the whole cable, from the external end to the cable entry In of
the plug, and `DP.2` (page empty, `-3X1:A1` to `-201U1:A`, no cable) is the plug on the DP port, whose mating is not
drawn (doc 10 §2.7, doc 20 §11.1 "Physical segment"). Both `wire` cells are empty (a bus line has no wire ID).

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
`frequency` O, `earthing` O (`pelv`, `selv`, `floating`), `circuit_class` O (`mains`, `control`, `analogue`, `bus`, `pe`).
The row `DP` (doc 20 §7.3, §11.1) is the bus potential: `at` is the DP port's CP `-201U1:A`, `circuit_class` `bus`, every
other cell empty (no voltage; the colour comes from the cable).

### `bonds.csv` → `bond` (doc 20 §7.4)

`key` R, `a` R, `b` R (endpoints), `kind` R (`pelv_0v_pe`, …), `via` O (`connections` key).

### `plc_config_projects.csv` → `plc_config_project` (doc 20 §11.2, doc 10 §2.2)

`key` R, `name` R (identifier), `profile` R (identifier `id@version`, the pinned address profile, doc 10 §2.5), `cpu` O
(`devices` key; an unknown key drops the row, and the `plc_station` aspects that named the project with it). One row,
`PLC1`, profile `siemens-tia-intl@2027.1`: doc 20 §11.2 "Configuration project: one, address profile Siemens TIA
(international)".

### `part_plc.csv` → `part.sections.plc` (doc 20 §11.2, doc 10 §2.6)

The PLC section of the part copy named by `key` (a `parts.csv` key; a key with no part is unresolved).

| Column | | Meaning |
|---|---|---|
| `key` | R | the part key |
| `role` | R | `station` or `module` |
| `io` | O | `;`-list of `<dir>:<bits>` or `<dir>:<bits>:<class>`; `dir` is `in` or `out`, `class` `digital` (default) or `analog` |
| `channel_count` | O | integer: the channels a module instantiates (numbers 0 … count − 1) |
| `channel_kind` | O | `ChannelKind` in snake_case (`di`, `do`, `ai`, `ao`, `counter`, `io_link_port`, `power_supply`); required exactly when `channel_count` is set |
| `data_type` | O | identifier (`Bool`); needs `channel_count` |
| `designation` | O | the automatic designation pattern, which may contain `{n}`; needs `channel_count` |
| `bit_of` | O | the channel map as a map-language expression (`n`), stored as `MapSpec::Expr` |
| `unbound` | O | `ChannelState` of channels neither bound nor claimed (`spare`, `reserved`, `deactivated`) |

One row: part `533361`, `module`, `out:8`, 8 `do` channels, `Bool`, `{n}`, `n`, `deactivated`: doc 20 §11.2
"Channel map (part 533361): channel n → bit n, 8 DO channels per module" and doc 10 §2.6.

### `channels.csv` → `channel` (doc 20 §11.2, §11.3; doc 10 §2.4)

| Column | | Meaning |
|---|---|---|
| `key` | R | a label, `<module key>:<number>` (`-201U3:1`); the ID is not derived from it (see "Identity") |
| `module` | R | `devices` key of the module |
| `number` | R | integer 0 … 65535 |
| `kind` | R | `ChannelKind` (as in `part_plc.csv`) |
| `address` | O | `auto` (default, empty), `tbd` or `excluded`; anything else is malformed: manual addresses are not a fixture form |
| `state` | O | `ChannelState`; empty: derived (doc 10 §3.2) |
| `symbol` | O | identifier |
| `comment` | O | text |
| `binding` | O | the signal CP in the endpoint notation above (`-201U3-Q1:14`); a junction is malformed |
| `target` | O | `functions` key (a manual target override) |

56 rows: the seven modules of §11.3 in manifold order, numbers 0 … 7, kind `do`. The odd channel 2p + 1 is bound to
coil 14 of valve position p + 1 of its module (`-201U3:1` → `-201U3-Q1:14`; doc 20 §11.2 "Bindings (as drawn)", the
Bit 1/3/5/7 columns of §11.3); the even channels have no binding. No state, symbol, comment or target: states are
derived and the symbols are empty in Golden A.

### `device_links.csv` → `device_link` (doc 20 §6.2)

`key` R (`<from>><to>`, `-201U3-Q1>-203Q1`), `from` R, `to` R (`devices` keys; the two must differ), `kind` R (`drives`
or `linkage`). Eight rows: the valves `-201U3-Q1 … Q4` and `-201U4-Q1 … Q4` drive the field devices `-203Q1 … -203Q8`
(§6.2 "Driven by (DeviceLink)").

### `assembly_configs.csv` → `device.assembly` (doc 20 §14.2, doc 03 §7; D-36, D-41)

`key` R (`<assembly>:<n>`, `-201U1:1`), `assembly` R (the `devices` key of the assembly device, `-201U1`), `n` R (the
order-code index, 1, 2, …; the configs are written in `n` order), `order_code` R (stored verbatim), `label` O (text),
`part` O (the `parts` key of the configured part, here the order code), `covers` O (`;`-separated `devices` keys: the
devices whose own part lines the order code replaces in the assembly BOM). Two rows. Covers are explicit, never
transitive, and each covered device is the assembly device or a descendant of it (`parent` of `devices.csv`). A row
that names an unknown device or part is dropped with EW-IMP-003; a repeated `n` for one assembly, a device listed
twice or a covered device outside the assembly is dropped with EW-IMP-002.

### `revisions.csv` → reserved `revision` records (doc 20 §4.1, doc 03 §18)

`key` R (the label, `V1.0`), `date` O, `description` O, `state` R (`open`, `released`, `withdrawn`), `author` O and
`checker` O (a `parties` key or name; the record keeps the party's role string, the only party reference form of
doc 03 §18 so far), `changed_pages` O (list of `pages` keys; the page notes of the record). The records are
`Record::Unknown` values of kind `revision` (the kind is reserved in `ew-schema` v0, written to `revisions.jsonl`);
the title-block query of `ew-derive` reads them leniently (`revision.current.*`).

### `profiles.csv`, `colour_profile.csv` and `size_profile.csv` → `ProjectSettings.extra["colour_profile"]`, `["size_profile"]` (doc 20 §7.1, §7.2)

`ew-schema` v0 has no typed settings for the profiles; they are the provisional JSON members of doc 03 §5.1 that the
wire-property chain of `ew-derive` reads (`crates/ew-import-tables/src/emit/profiles.rs`). The importer holds no
profile data of its own: everything is in these three tables.

- `profiles.csv`: one row per profile, `profile` R (`colour` or `size`), `key` R (the profile id), `name` R. The
  colour profile row also has `basis` (`company_convention`), `basis_note` (the basis text of doc 20 §7.1, profile
  metadata), `standards` (the standards text the cover prints, doc 20 U-35: "IEC60757 / EN60204-1"), `dbu_code`
  (`<code>:<base>:<shade>`, the company code the profile declares: `DBU:BU:dark`) and `verified`; the size profile
  row `pe_class`, `class_rules` and `bond_rule` (the last two as JSON text) and `verified`. A colour or size table
  without its row in `profiles.csv` is ignored and reported (EW-IMP-003).

- `colour_profile.csv`: `key` R (row number), `voltage_class`, `function`, `potential_type`, `colour` (the printed
  colour name) and `code` (the colour as printed in the wire list: `BK`, `GNYE` = GN + YE, the company code `DBU`);
  the selector columns `ptype` (a potential type), `ac_dc`, `u_min`, `u_max` (V) and `analogue` (`true`). A row
  becomes `{group, function, potential_type, select, colour, colour_name}`.
- `size_profile.csv`: `key` R (colour code), `control` and `mains` (Dec3 mm²) and `order` (the position of the code in
  the cover's size tables: doc 20 §7.2 lists DBU, WH, BK, BU, GNYE). Each class becomes a row `{class, csa, codes}`:
  its one cross-section and the colour codes that have it, each `{code, order}`; the `profile_tables` report sorts
  them by `order`.

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
| `parts.csv` | `tech_data` | `5SY4204-7; 533356; 2904602; A9F78116; A9P52610; MICO 4.10` | `(per part: characteristic, rated_current, channel_current, output_voltage)` | §6.1 (Function column), §3.4 row 8 | U-32 | the ratings are typed technical data of the part: the MCB characteristic and rated current ('MCB 1P+N C10'), the rated current of the switch and the PSU, the PSU's output voltage (plain key output_voltage: no typed key exists yet), the channel current of the load distribution; notation 'C10', '40 A', '6 A' of U-32 |
| `devices.csv` | `tech_data` | `T55` | `setpoint=15 degC` | §6.1 (Function column), §3.4 row 8 | U-32 | the setpoint of the thermostat is typed technical data of the device ('15 °C' of U-32) |
| `parts.csv` | `manufacturer; test_data` | `TBD DIN rail; TBD wiring duct; TBD end bracket; TBD plug-in bridge` | `TBD; true` | §14.1 rows 24-27 | U-27 | the manufacturer and part number of the rails, ducts, end brackets and bridges are unknown (TBD): the parts are test parts (confidence test), keyed by their role |
| `parts.csv` | `kind` | `TBD DIN rail` | `rail` | §14.1 row 24 | U-27 | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `TBD wiring duct` | `duct` | §14.1 row 25 | U-27 | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `TBD end bracket` | `end_bracket` | §14.1 row 26 | U-27 | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `TBD plug-in bridge` | `accessory` | §14.1 row 27 | U-27 | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `UK 5 N; USLKG 5; PT 2.5-TWIN; PT 2.5-TWIN-PE` | `terminal` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `NEBU-G78W5-K-2-N-LE5; 6XV1830-0EH10` | `cable` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `NSYPLA1074G` | `enclosure` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `parts.csv` | `kind` | `NSYPMM107` | `mounting_plate` | §14.1 | — | the component BOM gives no part kind; the kind follows the part's role in the project |
| `part_cps.csv` | `potential_role` | `550208:2` | `fe` | §6.1 row 19 | U-18 | pin 3 is the functional earth, printed 'PE': its role is a ▲ |
| `part_templates.csv` | `main` | `550208:0` | `false` | §6.1 rows 19, 28 | — | 550208 is the power block of -201U1 and -202U1, whose main function is the overview module |
| `cable_cores.csv` | `core` | `6XV1830-0EH10:SH` | `SH` | §6.1 row 18, §10 | U-21 | the shield core is named SH; the cores A, B and the shield carry no colour or cross-section |
| `conductors.csv` | `core` | `-3W1:SH` | `SH` | §6.1 row 18, §10 | U-21 | the shield conductor takes the shield core of the cable part |
| `devices.csv` | `parts` | `-UC1` | `TBD DIN rail*2.192; TBD wiring duct*3.81` | §13, §14.1 rows 24, 25 | U-26, U-27 | the quantity of the rails and ducts is a ▲ (length from carriers): the paper lengths of the carriers table at the 1:10 scale of folio 902 (the five ducts D1 ... D5 added, and three rails of the width of D4), in metres |
| `devices.csv` | `parts` | `1X1; -X1031; -X1032` | `1X1: TBD end bracket*2 / -X1031: TBD end bracket; TBD plug-in bridge*5 / -X1032: TBD end bracket; TBD plug-in bridge` | §13 (R2), §14.1 row 26 | U-27 | the end brackets sit on the strips as part uses (a bracket belongs to the strip before it, the first to the first strip), the bridges of §8.3 as one part use per bridge on its strip: the parts list prints them after the legend rows |
| `devices.csv` | `frozen; frozen_sign` | `-XPE; -UC1; -UC1-plate` | `frozen = the tag without its sign (XPE, UC1, UC1-plate); frozen_sign = shown` | §2.1 rule 1, §6.1 rows 9, 31, 32 | U-34 | per-class numbering rules are not modelled yet: the tags are frozen overrides with the sign shown, so Golden B's include_frozen renumbers -XPE as §2.2 expects |
| `devices.csv` | `key` | `-UC1-plate` | `-UC1-plate` | §6.1 row 32 | — | doc 20 gives the mounting plate no tag ('mounting plate, child of -UC1'): the key is the fixture's name |
| `devices.csv` | `cable_length` | `-106W1; -106W2` | `from_part` | §10 | U-18 | the 2 m length is read from the part's type code; the source of the length is a ▲ |
| `potentials.csv` | `at; circuit_class` | `DP` | `-201U1:A; bus` | §7.3, §11.1 | — | the Members cell names the port as -201U1:DP; the definition point is the port function's first CP A (the one CP that a single-line bus connection uses, FR-CBL-008); class bus from the Type cell, no voltage (the colour comes from the cable) |
| `potentials.csv` | `frequency` | `L; N; L-S; N-S; L-1; N-1; L-55; N-55` | `50` | §4.1, §7.3 | — | the Type cell of §7.3 gives no frequency; every AC potential takes the 50 Hz of the §4.1 Supply |
| `potentials.csv` | `ptype; voltage; ac_dc; frequency; circuit_class` | `N-55T; N-55S` | (empty) | §7.3 | — | N switched: naming only; doc 03 §9.4 derives the type, including 'switched', across the PassThrough{opens: true} pair |
| `colour_profile.csv` | `ptype; ac_dc; u_min; u_max; analogue` | all | `(per row: the selector of the voltage class and the potential type)` | §7.1 | — | doc 20 prints the voltage class and the potential type as labels; the selector of a row (the profile of doc 08 §4.2) is 24 VDC = DC up to 120 V, 230 VAC = AC 50 to 250 V, 400 VAC = AC 250 to 1000 V; a PE row selects by type only, an analogue row by type and the analogue attribute |
| `size_profile.csv` | `order` | all | `(the row number of the table of section 7.2)` | §7.2 | — | the standard-size tables of the cover list the codes in the order of doc 20 section 7.2 (DBU, WH, GNYE; BK, BU, GNYE), not in key order: the order is a column |
| `profiles.csv` | `key` | `colour` | `company.dbu-wh-24v` | §7.1 | — | the profile id of the shipped company profile of doc 08 section 4.2; the size profile's id is its name ('0.75/1.5', read from §7.2) with the company prefix |
| `profiles.csv` | `verified` | all | `true` | §7.1, §7.2 | — | the golden profiles are verified data (doc 08 section 4.2) |
| `profiles.csv` | `class_rules; pe_class; bond_rule` | `size` | `(the rules of doc 08 §4.2 as JSON)` | §7.2 | — | doc 20 section 7.2 gives the circuit classes in prose; the class rules, the PE class rule and the bond rule are those of the company size profile of doc 08 section 4.2 |
| `profiles.csv` | `standards` | `colour` | `IEC60757 / EN60204-1` | §23.1 (U-35) | U-35 | the cover prints the standards text of the reference ('IEC60757 / EN60204-1', kept); the basis note stays metadata |
| `revisions.csv` | `checker` | `V2.0` | (empty) | §4.1 | U-03, U-04 | the checker of V2.0 is unset in Golden A (unknown) |
| `representations.csv` | `dx` | `1X1:1; 1X1:2; 1X1:PE` | `-20; 0; 20` | §6.1 row 8 | — | test input (fixture choice): the three terminal symbols of 1X1 sit side by side in the strip's cell |
| `representations.csv` | `cell` | `-201U3-Q1 … -202U2-Q4` | `row D, columns c-3, c-1, c+1, c+3 around the module's aux column c` | §6.1 rows 33-60, §15 | — | test input (fixture choice): doc 20 names only the pages 203-206 of the valves, not their cells |
| `representations.csv` | `cell` | `-203Q1 … -203Q8` | `row G (§6.1 Main rep.), column of the driving valve` | §6.1 rows 61-68, §6.2 | U-07 | test input (fixture choice): the column of a process valve is that of the valve that drives it |
| `representations.csv` | `cell` | `-203Q1#actuator … -203Q8#actuator` | `row F, column of the driving valve` | §6.2 | U-07 | test input (fixture choice): the actuator is drawn above its process valve |
| `representations.csv` | `dy` | `J-E-206-L@101; J-E-206-N@101; J-E-206-PE@101` | `-20; 0; 20` | §15 | — | test input (fixture choice): the three mains points share the cell 101-G0 and are stacked |
| `representations.csv` | `dx; dy` | `interruption points that share a cell` | `k-th member of a coincident group (natural key order): +(0,0), +(0,20), +(0,-20), +(20,0) px` | §15 | — | ▲ test input (fixture choice): separates coincident interruption points; the cell is what doc 20 §15 asserts |
| `representations.csv` | `arrangement` | `-201U3-Q1 … -202U2-Q4; -203Q1 … -203Q8; -203Q1#actuator … -203Q8#actuator` | `ports` | §6.2 | — | test input (fixture choice): the symbol's arrangement without the Tag slot (FR-SYM-012) |
| `connections.csv` | `a; b` | `DP.1; DP.2` | `A1 of the In entry of -3X1; A of the port -201U1#bus` | §10 (-3W1), §11.1 | — | a bus cable is drawn single-line (FR-CBL-008): one bus connection per hop, between one representative CP of each end (A1 of the In entry, A of the DP port) |
| `connections.csv` | `page` | `DP.2` | (empty) | §11.1 | — | the plug -3X1 is plugged onto the port and the mating is not drawn: the plug-on connection has no page (doc 10 §2.7) |
| `junctions.csv` | `cell` | `J1; J2; J3; J4; J5; J6` | `C11; C13; C6; D7; D5; D6` | §8.3 | — | test input (fixture choice): doc 20 gives the folio of a junction, not its cell |
| `plc_config_projects.csv` | `name` | `PLC1` | `PLC1` | §11.2 | — | doc 20 gives the configuration project no name: the name PLC1 is the default name of a new project (doc 10 §2.2) |
| `plc_config_projects.csv` | `profile` | `PLC1` | `siemens-tia-intl@2027.1` | §11.2 | — | doc 20 names the profile 'Siemens TIA (international)': the reference is the id and version of the built-in profile of that label (doc 10 §2.5) |
| `part_plc.csv` | `io; designation; data_type; unbound` | `533361` | `out:8; {n}; Bool; deactivated` | §11.2, §11.3 | — | doc 20 gives the channel count, the kind and the map; the width of the output range (one bit per channel), the designation pattern, the data type and the state of unbound channels are those of the PLC section of part 533361 in doc 10 §2.6 |
| `assembly_configs.csv` | `label` | `-201U1:1; -201U1:2` | `Order Code <n>` | §14.2; doc 09 §3.4 PART DATA | — | the label of order code n: the reference prints each PART DATA entry as the label line "Order Code n:" above the order code |
| `assembly_configs.csv` | `covers` | `-201U1:1` | `-201U1` | §14.2 row A1 | U-24 | the Covers cell lists 195740 and 550208 of -201U1, the head's own parts: A1 covers the head device itself |
| `assembly_configs.csv` | `covers` | `-201U1:2` | `the 11 children and the 28 valves` | §14.2 row A2 | — | covers are explicit device lists, never transitive: A2 names the 28 valves (devices under -201U3 … -201U8 and -202U2) next to the children -201U2 … -202U3, in natural key order |
| `assembly_configs.csv` | `covers` | `-201U1:2` | `-202U1` | §14.2 rows A1, A2 | U-24 | which order code covers -202U1 (part 550208) is unknown: the fixture follows the table, whose A2 cell spans -201U2 … -202U3, so A2 |
| `devices.csv` | `key` | `-UC1-plate` | `-UC1-plate` | §14.1 row 23 | — | the Tags cell of row 23 reads '-UC1 (plate)': the expected parts list prints the plate under its fixture key |

<!-- test-inputs:end -->

## Not covered yet

Doc 20 §11 (PLC and bus) is covered for the bus connections (`DP.1`, `DP.2`), the DP potential, the configuration
project, the station and module aspects, the PLC section of part 533361, the 56 channels and the device links (§6.2).
Not covered yet from §11: the `BusPort`, `LogicalNet` and `PhysicalNet` records and `bus_segments` (doc 10 §2.7,
§3.8), the solenoid-position map and the supply groups of part 533361 (doc 10 §2.6; derivation step 4 of doc 10 §3.2
has no input), the valve function-text slot fallback (FT-80) and the station address `Tbd`.

§12 (fluid lines F1–F16) and §13 (layout space, carriers, placements) need record kinds that `ew-schema` v0 reserves
but does not implement; their tables follow with those work packages. §14.1 (component BOM) is covered through
`parts.csv` and the part uses of `devices.csv` and `terminals.csv`; §14.2 (the configured products and the split of
the order codes) through the two configured parts of `parts.csv` and `assembly_configs.csv`. The BOM rows themselves
are derived.

Also not covered yet: the 13-14 signal-contact function of the MICO 4.10 template (its CPs 13 and 14 belong to the
black box) and "supplied by others" for the process valves -203Q1 … -203Q8 (FR-DEV-037).

## Expected page output (candidates)

`expected/` holds what the page output of the golden model must reproduce (doc 20 §3.1, §3.3). It is **generated**,
never edited by hand, by `crates/ew-api/tests/page_output.rs`, through the session's export job (`ew_api::output`,
FT-59: the product's page views and one memoised `page_display_list` family per layer; since cycle 15 no page view is
test glue):

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
  size profiles of doc 20 §7.1, §7.2. Same dialect as above. The kinds also derive and paginate inside the report
  engine's `Q`, so their page text files carry the same rows (the bootstrap emits the profiles, so the wire list 907
  prints the sizes, colours and sources `cdp`, `rule`, `core` and `bond`).
- `geometry/frame.json`: the printed frame figures of the two frame forms with the QET title block, in mm on the
  sheet (3 decimals): sheet, outer, drawing area, header band, column and row edges, labels, title block, line width.
- `parts_list_14_1.csv`: the 27 rows of doc 20 §14.1 as the parts list prints them (quantity as a number, the unit in
  its own column, the fixture values where the table has a ▲), **generated by `tools/tables_from_doc20.py`** like
  the tables. AC-07 compares folio 901 with it cell by cell; its accepted differences are listed and commented in
  `crates/ew-api/tests/acceptance/checks.rs` (`AC07_ACCEPTED`).
- `png/<folio>.png`: the sheet at 150 dpi. They are **candidates awaiting the first PNG approval (FT-10, an owner
  action)** and are compared with the §3.3 tolerance (at most 0.5 % of the pixels differ by more than 16 per
  channel), not byte for byte.
- `pdf/structure.json`: the writer's PDF structure of the whole page order (page count, `/PageLabels`, outline
  tree, link, plain-text-link, dropped-`.notdef`-glyph and destination counts, embedded fonts), compared exactly
  (§3.3 "PDF structure"; 92 links: one per listed cross-reference item (64) and one per bound channel row of
  the PLC overview to its coil (28, doc 10 §5)); `pdf/sha256.txt` holds the SHA-256 of the whole PDF, which
  `ew export pdf golden/J-E-2416.ew --out <file>` writes byte for byte (G-M1-01; `bin/ew/tests/cli.rs` runs it on
  one and four threads with no display, no starter content and empty user directories). Re-blessed in cycle 15: the
  PDF's document ID is the export job's, the hash of `ew-pdf/1 <project id> <profile> <date> all ew <version>
  sha256:<digest>` (doc 09 §4.3 rule 2; the digest covers the sheet display lists of every page and the bundled font
  hashes, so it is the same over the tables and the fixture), where the cycle-14 harness hashed a test label; the
  page content is unchanged (the same pages with the old ID give the old digest).
- Everything under `golden/` is CC0-1.0 (`REUSE.toml`, annotation L8).

**Known gaps for the FT-10 approval.** The candidates cover the 36 pages of `page_order` with the report engine
installed (`Session::query()`), so Prev/Next, the outline and the page labels include the generated pages. All 72
page wires are drawn (21 on 101, 19 on 102, 20 on 103, 12 on 106), and the single-line bus cable -3W1 on folio 3
(`DP.1`, from the external point DP to -3X1; the plug-on connection `DP.2` is page-less and not drawn; the route is
the provisional router's). Still open:

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
  (FR-REP-042) are not drawn yet. The dividers 100, 200 and 900 show frame and title block only.
- **Page count.** The page order has the 36 pages of doc 20 §5: 901 paginates into 901 and 901.1, 907 into 907,
  907.1 and 907.2, and 908 into 908, 908.1 and 908.2 (FT-111).
- **PLC overview (908).** The 56 channels of -201U1 (doc 20 §11.3) on three pages of 23, 23 and 10 rows, derived by
  `plc_rows(-201U1)` from the channel records, the module aspects, the PLC section of part 533361, the address
  profile of the configuration project and the device links: addresses Q0.0 … Q6.7, 8 used, 20 spare and 28
  deactivated, the valve as target, the function text (the driven field device's process tag, or "Spare") and the
  process tag in separate columns, and the frame cell of the coil's representation (203-D1 …), which links to the
  coil's CP (`ew:cp:<uuid>`, a named destination at the valve's position, since the valve symbol does not draw the
  internal coil 14; 28 bound rows, doc 10 §5). The columns are those of doc 10 §5 (module, part, address, channel,
  designation, kind, symbol, function text, process tag, target, state, page); doc 20 §11.3 names a subset.
  Symbols are empty (Golden A; templates are 1.0). The station address "TBD" is not printed (no `BusPort` record
  yet). The TOC's and the parts list's row links are not drawn yet (FR-REP-042).
- **Parts list.** 901 holds the 27 rows of doc 20 §14.1 (23 on 901, the 4 TBD rows on 901.1). Rows 24 to 27 come
  from four TBD test parts (the part number TBD, U-27) used by -UC1 (rail 2.192 m, duct 3.81 m from the carriers of
  §13) and by the strips (4 end brackets, 6 bridges of §8.3). The rows after the legend rows of the enclosure and
  the plate follow the layout order (rail, duct, then the layout-only accessories), but the folio column of the
  accessory rows lists the folios of their strips (101, 103 and 103), not 902 as doc 20 does: the layout
  placements of the brackets and bridges are not in the model yet.
- **TOC pages.** The title blocks of 2 and 2.1 print `=P01` and `+J-E-2416`: a generated page of a block outside
  every section takes the structure all drawn pages share (`page_ref_structure`, FT-160).
- **Folio 1.** The embedded `profile_tables` block draws the colour table (the standards text of the profile,
  "IEC60757 / EN60204-1", under it; the basis note stays metadata, U-35), the two standard-size tables of doc 20
  §17.3 and the note "If size or sign is not indicated in diagram, standard size or sign is to be used." under
  them, generated from the profiles in the project settings (the report kind `profile_tables` supplies the titles,
  the caption and the note as rows; `ew-scene` `embedded` draws them, FR-REP-028). The note is printed whenever the
  cover has a table: the wire-marker mode "only explicit overrides" that doc 20 §7.2 gives as its reason is not a
  setting of this format yet. The table geometry is the proportions of the reference cover relative to the block's
  rectangle. The rest of the cover is the frame form `ew.frame.qet-a4-landscape-cover` (`content/forms`, generated by
  `crates/ew-scene/tests/forms_content.rs`, D-30; `pages.csv` gives it to the title page): the no-grid frame plus the
  heavy rule (1.305 mm at y 13.70), the light rule (0.522 mm at y 45.02), the title "J-E-2416" and subtitle
  "Electrical Definition" (`{project.object}`, `{project.document_title}`) and the project data box of doc 20
  §17.3 (13.70-118.09 × 47.63-175.50, line 0.418, shadow 2.61, separators at y 84.16, 120.70, 154.62). The box prints
  the properties its cells bind: the customer ("Company / Customer" `{party.customer.name}`, never the end user),
  the long description, the drawing identification and project reference, the end user, address and country, "Postcode /
  City" (the golden end user has none), the supply, the creation and edit dates and the engineer. Three rows of the
  reference have no placeholder path yet and are not printed: the manufacturing year, the control current and the
  consumption (`title_block` resolves none of them, `Quantity` has no text form; FT-240). The cell layout (label column 14.70-55.00,
  value column 55.00-117.10, equal slots per group) and the logo (no asset) are provisional (FT-202, FT-203).
- **Title block.** The revision label ("Revision: V2.0") and the author ("Author: <engineer>") come from the
  revision records on every page; `ew.titleblock.qet-a4` has no Notice cell yet and the logo is not bound.
- **Black boxes.** The box outline is the 10 × 6 grid-step symbol rectangle; the inner graphics ("AC ~ / 24 VDC"
  on -102G1, the "24VDC" and "GND" fields of -102F2) are not drawn yet. The part texts print: the manufacturer and
  the part number above the box ("Murrelektronik", "MICO 4.10"; the manufacturer slot is new, FT-162).
- **Part data at the symbols.** The slots of the symbols print from the part data (FT-162, D-18, D-33): the
  manufacturer name, the part number and the technical data of the MCBs, the main switch (Eaton, 533356, "40 A") and
  the load distribution on 101 and 102, and the thermostat's setpoint and part number ("15 °C", "SK 3110.000"; its
  part-number slot is new). The typed ratings of the golden parts are test inputs of doc 20 U-32.
- **Strip segments.** The dashed segment rectangle does not stretch over the terminals the segment draws.
- **Folio 101.** The terminals of 1X1 print only their designations (ID-162), but "PE" of 1X1:PE still crosses the
  conductor of terminal 2 (the terminals are 20 px apart); the external-arrow text "J-E-206-PE" starts 0.27 mm left
  of the drawing area.
- **Interruption points.** The texts of two points that share a cell overlap: PE-1.1 and PE-1.4 at 103-G6,
  0VDC-AIR3 and PE-AIR2 at 106-B1, 0VDC-AIR1 and PE-AIR1 at 106-E1.
- **Folios 203–206.** Used valves print no text in the function-text slot: the `DeviceLink{Drives}` records exist (the
  PLC overview reads them), but the page view's slot does not apply the process-tag fallback of §6.2 yet (FT-80).
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
  - Rows are in the numbering order of FR-NUM-028 (doc 08 §5.2): the page of the From end, the From end's frame
    cell (column, then row), the To end's page and cell (the page first), then the wire ID; the cable cores sort in
    the same order. The
    order and the numbers are the `wire_numbers` query of `ew-derive` (FT-100), not the W1 … W44 order of doc 20
    (W is a fixture label). The cell of an end is the cell of the CP's drawn point on the page of the constituent
    that ends there: the symbol CP, the box CP of a black box, or the point of the terminal in its strip segment
    (doc 08 §6.7); the strip-segment and black-box points are restated in `ew-derive` from the page scene's rules.
  - The "No." column prints `{page}.{counter}` (counter scope page) on the 34 wires W1 … W34 and nothing on the 10
    cable cores (FR-NUM-019). `expected/csv/wires.csv` holds the exact counters, also where two wires have equal
    cells (a tie): the wire ID decides there, and the query keeps the counter range of the group
    (`WireNumber::tie`) for a table that wants to print it. The page of every number equals doc
    20's "No." column, which now holds the computed numbers (W29 is 102.10; doc 20 §8.1 said the order could not be
    stated in advance); `expected/wire_numbers_8_2.csv` is that column, written by `tools/tables_from_doc20.py`, and
    AC-09 compares the runner's numbers with it, with no known deviation since FT-101 (cycle 14: the From ends
    follow the flow-distance step, so W1 to W5 start at the supply side and their numbers, and those of W7 and W9,
    are final). Numbers are not printed on pages
    (FR-CON-026 follows with the number display) and not stored: `NumberWires` and the stored `NumberSpec` are the
    numbering schemes of FR-NUM-020.
  - From and To follow doc 08 §3.8 (FR-CON-040): on one page the end with the smaller flow distance is From
    (`cp_flow_distance`, FT-101), then the end with the smaller anchor cell (a terminal drawn only by a strip
    segment has no anchor: the cell of its CP in the segment counts). Every From/To pair equals doc 20 §8.2.
  - Repeated CP designations print plainly (`-101F1:N`, `-102G1:+`, `-102G1:−`, `-102F2:GND`), without the `#1`,
    `#2` of doc 20; a terminal end prints the clamp in the CP column (`-X1031:1` and `b`), not as `1/b` (FT-102).
  - The Source column prints `cdp`, `rule`, `core`, `bond` or `built-in` (size first, then colour, joined when they
    differ), as doc 20 §8.1 now states; its "(rule)" and "(profile)" are both the source `rule`.
  - The Via column names the cable and core, the junction by its place and wiring ("101-C11 star at F55:2",
    "102-C6 chain on -102F1:1"), the daisy chains ("chain on -101F1:1", D-20) and the interruption points; doc 20
    names the junction by its fixture key (J1 … J6, not a derived value) and the EF case, which the list does not
    print.
  - W33 to W44 (the cores of -106W1 and -106W2 and the PE legs) print the source `rule/core` or `rule`: the size
    comes from the size profile, while doc 20 §10 says core cross-sections come from the cable part (the
    conductors of the fixture carry none; FT-107).
  - Since cycle 12 the page goldens are built with the colour and size profiles (FT-65 closed): the wire-list pages
    print the same sources as `rows/907.csv`.
- **Potential overview (909).** 19 potentials and B1 (20 rows). The PROFIBUS `DP` potential (FT-105) is joined by its
  two `Bus` connections and named by its definition at -201U1:A: class "bus", no type or voltage, "(cable)" for
  colour and size (its bus connection carries the cable -3W1, doc 20 §7.3), 3 members (the port -201U1:DP, the plug
  -3X1 and the external point; doc 20 also lists the cable -3W1, an edge of the bus graph under doc 10 §3.8 A5, not a
  connection point) and page 3 (its drawn bus connection). The rows are in natural order of the name, not in the
  order of doc 20.
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
  field side of 1X1 first (`Strip.field_side` is not applied). Targets carry no wire number (W29 …; FT-100 gives
  the number, FT-103 prints it) and no `#n` (FT-102).
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

It holds 169 files (about 0.8 MB): `manifest.json`, `project.jsonl`, `structure.jsonl`, `report-blocks.jsonl`,
`revisions.jsonl` (the two revisions), 13 page files in `pages/`, 79 of the 256 anchor shards in `devices/` and 72
library copies in `lib/` (fndefs, forms, parts, symbols). It holds no `waivers.jsonl`,
`layout/`, `lib/manufacturers/` or `assets/` yet, because the bootstrap emits no records of those kinds: the
waiver, layout and manufacturer kinds are reserved in `ew-schema` v0.

The test `the_fixture_is_byte_identical_to_the_bootstrap` (`crates/ew-api/tests/golden_fixture.rs`) regenerates
the files and compares them with the directory byte for byte. It ignores only the manifest's `generator` member
(doc 05 §17.1). The other tests of that file open the fixture with `ew-store` (edit, and read-only without a
lock) and check the findings, the counts and the portable file names.

## The builder DSL (path (b))

`ew_testkit::golden` (WP-S6.01, cycle 15) holds both golden builders: `bootstrap(repo)` is path (a), the CSV
bootstrap above, and `build(batch, sink)` is path (b), which constructs the same project **through commands
only**. It starts from the `shell(batch)`: the project row (without its import provenance, the supply's
external-reference origin and the four settings members that `project.settings.set` edits) and the embedded library
copies, because no command creates a project row (`ew new`); `library.embed` exists (cycle 16, FT-167) but the
shell keeps the copies, which include the project-local parts that are created in the project, not embedded. Then
it issues, in dependency order, one command per record, with the bootstrap batch as its typed description (the same
source of truth as path (a)): `project.settings.set`, `structure.create_node`, `section.create`,
`external_ref.create`, `project.supply.set` (the supply's origin), `page.create`, `device.create`, `terminal.add`,
`cable.create`, the device tree (`assembly.attach` of the 11 modules under -201U1 and the mounting plate under
-UC1, `valve.add` of the 28 valves), `plc.config_project.create`, the `plc_station`, `plc_module` and `assembly`
aspects (`fields.set`; a new module gets its 8 channels from the part data, normaliser N1), `device.place`,
`box.black`, `ip.place`, `connection.draw` (a junction is created by the connection that splits another one at
its point), `junction.edit`, `potential.define`, `bond.create`, `report_block.create`, `plc.bind_channel` (coil
14 of position p → channel 2p + 1, as drawn), `device.link` and `fields.set` for the members the creating
commands do not take. The commands mint their own IDs, so path (b) agrees with path (a) by content
(doc 20 §3.2: every column except `id` and `source_ref`); with the same ID source the build is deterministic, and
its command log (`Built::command_log`, doc 17 §3.5) replays with `ew apply` to the same model.

The builder never injects a record. What no command writes yet is left out and listed (`Built::unwritten`,
pinned by `crates/ew-testkit/tests/golden_dsl.rs`): only the 2 revisions remain, a reserved kind without a schema
type that no command can create (FT-165). 606 commands build the rest (23 command names; about two seconds in a
debug build). The same test file checks that path (b) holds the records of path (a) record for record, up to the
IDs the commands minted (`source_ref` aside, the import lineage being path (a)'s own).

**Status (AC-15 path (b): `pass`, cycle 16).** The six acceptance SQL files over the built project return `expected/csv/`, and so do `ew export bom --mode parts` (`bom_parts.csv`) and `--mode assembly` (`bom_assembly.csv`, compared since the valve-terminal assembly is built by commands, FT-169). Until cycle 16 the parts list lacked the 28 valves (571334) and the mounting plate (NSYPMM107) and printed the seven modules one by one. The cable markers (representation type graphic on the multi-line folio 106 and the bus-topology folio 3) and the seven CPX black boxes drawn multi-line on the fluid folios 203 … 206 are placed under the owner ruling of cycle 15 that amends decision log ID-55 (doc 06 §2.2): before it, the page-type rule refused them, a cable without its marker had no page and printed `-?W1`, and four of the six files differed.

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
- `ew apply [<project>] <file.ewcmd> [--preview] [--one-step]` (cycle 15, FT-156) replays a log on a project
  directory and saves it; `--preview` prints the changed records, tags and cross-references and saves nothing;
  `--one-step` saves only when every line commits. `bin/ew/tests/cli.rs` previews and applies
  `b2-renumber-pages.ewcmd` on a copy of the fixture.
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
| `ac16-freshness.ewcmd` | §21 AC-16: `fields.set` renames T55 to T57 (`tag.frozen`), `rep.move` moves -102F2 by one frame cell (15.875 mm), `page.renumber` makes folio 103 folio 104, `project.settings.set` sets the 24 V DC positive colour row to RD ("Red") | run edit by edit by the acceptance runner (`crates/ew-api/tests/acceptance/freshness.rs`) with the oracle on: one commit per line, the outputs that each edit affects change and nothing else, `ew verify-derived` (`ew_api::verify`) finds no difference after each edit, four undos restore the records byte for byte |

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
  is reproduced. MF-05 (EW-IMP-105, FT-173), MF-06 (EW-IMP-106, FT-174), MF-14 and MF-20 (EW-IMP-110 and
  EW-IMP-112, FT-175), MF-15 (EW-IMP-111, FT-176), MF-26 (EW-IMP-114, FT-177) and MF-17 (EW-IMP-121, FT-178) have
  no detection in the importer yet.
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
| 1, MF-16 | bond `pelv_0v_pe` between -102G1:−#1 and -X1032:2 via W27 | a bond command (FT-164) |
| 1, MF-04 | revisions V1.0 and V2.0, per-page values dropped | a revision-create command (FT-165) |
| 1, MF-02 | structure nodes =P01 and +J-E-2416 assigned to every device and page | a reference to a node created earlier in the log (`structure.create_node` exists; the `structure:<code>` reference, FT-171) |
| 1, MF-11 | core colour BR → BN on -106W1, -106W2 | cables with conductors in the import (FT-135) |
| 1, MF-05 | `description_override` of folio 101 removed | the import keeps the description in `Page.extra` (EW-IMP-104), so there is no override to drop; the importer must map it to `description_override` with EW-IMP-105, after which `fields.set` (`description_override` = null) does it |
| 1, MF-12 | manufacturers mapped to the master; "UK5N / ULSKG5" split into UK 5 N (terminals 1, 2) and USLKG 5 (PE) | parts from element information (FT-131) and a manufacturer master |
| 1, MF-13 | anchor policy "graphics" for the bus-port symbol of -201U1 on folio 3, so the master prints "∞ 3-B2, 106-G3" | the anchor is a member of the imported symbol copy (`SymbolDef.anchor`); `project.settings.set` has no anchor member and no command edits a symbol copy (the anchor-policy command, FT-172) |
| 2, MF-06 | the title-block "Customer" cell bound to the party role of ▲U-01 | title-block forms from the QET template (FT-139) and a party-binding command |
| 3 | starter symbols instead of the QET-import symbols | `rep.exchange_symbol` (catalogued, not built; FT-127) |
| 4 | sections 1, 2, 9 with dividers; folios 100, 200, 900 removed; report blocks 2, 901, 903 … 911 | moving a page into a section (`page.move`, not built); the frame form without grid is not in the imported library and no command embeds a library copy (moving a page into a section FT-166; embedding a library copy FT-167) |
| 5 | starter parts on every device with part data | part records in the project and a part-assignment path (FT-168) |
| 6 | the valve-terminal assembly with its 28 child valves bound to channels | an assembly command (valve-terminal assembly, FT-169) |
| 7 | layout items bound, rails and carriers | layout-page items in the import (FT-134) |
| 8 | text styles (`tag.main` …) | a text-style command (FT-170) |

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
  doc 20 §20.3 requires C fixed ≡ A, so the anchor quick fix belongs to the script; the anchor-policy command is FT-172); the 28 child valves and
  their cross-references (step 6, FT-169); the dividers and report pages 2.1, 903 … 911 with their rows (step 4,
  FT-166, FT-167).

## Acceptance runner

`cargo xtask acceptance [--milestone] <report.md>` runs the gate-M1 runner (doc 20 §21, §22; doc 18 §3.10) over the
golden project with the oracle on and writes a Markdown table with one row per named check, a JUnit file
(`target/nextest/ci/junit.xml`, nextest profile `ci`) and the exit status. It runs the `#[ignore]`d test `gate` of
`crates/ew-api/tests/acceptance.rs` (about 2 minutes in a debug build); a stale report is deleted first.

A row has one of four statuses:

| Status | Meaning | Blocks the gate |
|---|---|---|
| `pass` | the criterion holds | no |
| `fail` | it does not hold; the reason says what differs | yes |
| `not_built` | the feature the criterion needs does not exist yet (the reason names it) | yes, exactly like `fail`, listed separately |
| `not_applicable` | it cannot be evaluated headless or at this delivery (GUI, pilot, an external tool that is not installed, a milestone-only measurement); the reason is required | no |

A criterion with a part that holds and a part that is missing has one row per part. The runner evaluates AC-01 …
AC-14, AC-18 and AC-20, lists AC-15 … AC-17 and AC-39 with their status and reason, and covers the §19 rows gated M1.
It compares with the committed files through the functions that write them (`tests/page_output/`,
`tests/golden_common/`): the text CSVs, the frame geometry, the PNGs (15 folios at 0.5 %), the PDF structure and
SHA-256, and `expected/csv/`. AC-17 (open, check and PDF export times) is `not_applicable` outside a milestone run;
`--milestone` builds in the release profile with `EW_PERF_ASSERT=1` and measures it.

The ordinary tests of the file run the runner once, without the raster comparison and the measurements, and compare
its report with the committed `expected/acceptance_headless.md` (so two runs must print the same text); they also keep
`expected/csv/` current:

    EW_BLESS=1 cargo nextest run -p ew-api --test acceptance --locked -E 'test(headless_report) | test(expected_csv)'

`expected/csv/` holds the report rows of the acceptance pairs (doc 20 §3.1): `terminals_1X1.csv`,
`terminals_X1031.csv`, `terminals_X1032.csv` (903 … 905), `cables.csv` (906), `wires.csv` (907),
`potentials.csv` (909), `bom_parts.csv` (901 and 901.1) and `bom_assembly.csv` (the parts summary 901 in assembly
mode), byte-equal to `expected/rows/<folio>.csv` (`rows/901.assembly.csv` for the assembly list). The runner
compares the derived rows with them. Since cycle 14 the exports `ew export cables` (`cables.csv`) and
`ew export bom --mode parts` (`bom_parts.csv`) write these files byte for byte through the same CSV writer
(`ew_reports::csv`); the runner calls the library entry over the committed fixture, `bin/ew/tests/cli.rs` runs the
binary. Since cycle 15 `ew export bom --mode assembly` writes `bom_assembly.csv` (§14.2): -201U1's typed assembly aspect
(`tables/assembly_configs.csv`, D-41) replaces rows 16-21 by A1 and A2, rows 22-27 renumber 16-21, and the "Covers"
column names the covered devices; `crates/ew-api/tests/export_bom.rs` and the AC-07 assembly entry test it.
The SQL half of each pair (`acceptance/*.sql` over the `api_v1` views) runs since cycle 13 (WP-S1.10, decision log
ID-328) and returns `expected/csv/` row by row.

**The written PDF read back (cycle 14).** The runner reads the golden PDF with the in-repo reader
`tests/page_output/pdf_read.rs` (indirect objects, Flate streams, the page tree, content streams and the fonts'
ToUnicode maps; test code only) and checks three entries against it:

- AC-13 "text extraction keeps spaces": every page's text layer equals its printed glyph runs (the rows of its text
  CSV) run by run, spaces included, and folio 101 reads "SK 3110.000" (D-33, FR-OUT-008).
- AC-06 "the PDF text layer included": the PDF text equals the page text through the AC-13 entry; this entry
  counts every device tag on token boundaries ("-K1" is not counted inside "-K10") in each page's text layer
  and printed runs, and scans the text layer for stale tokens.
- AC-01 "vector content within 0.2 mm": on every page the lines of the page's frame form in
  `expected/geometry/frame.json` (outer frame, title-block separator, and for a form that shows its grid the two
  header-band lines and the interior column and row ticks) are stroked segments within 0.2 mm of both endpoints at
  the frame's line width; the media box is the sheet; no other line lies in the header bands or on the outer frame.
  `frame.json` holds the title block's bounding box only, so no check compares the title block's inner lines;
  AC-04 checks the title-block texts. The line count is 12 grid pages × 30 lines (4 outer edges, the separator,
  the 2 band lines, 16 column ticks and 7 row ticks) + 24 no-grid pages × 5 = 480.

**AC-14 (cycle 14; the shipped path since cycle 15).** The SVG export of `ew-export` (one file per page from the
same sheet display lists as the PDF, text as `<text>`) and the parallel page phase (`ew_export::par_map`, results by
page index) of the session's export job (`ew_api::output`, FT-59) are compared with the serial export: the page display lists, PDF and SVG on 2 and 8 threads are byte-identical, every SVG `href` resolves
to a file and an `id` of it, and the model loaded with its records shuffled writes the same PDF, SVG, page text
CSVs, report CSVs and the same files when saved (`Session::create`: the JSONL files and the manifest). The SVG set is not
committed as golden files (doc 18 asks for none); its SHA-256 is in the AC-14 reason of the committed report, so a
change of any SVG byte shows there. The comparison runs on the platform of the run; the gate platform is Linux and
the cross-platform digests are MVP.

The headless report of this revision: 93 entries, 86 pass, 0 fail, 2 not_built (AC-15's builder DSL, D-06's
property grid), 5 not_applicable, 0 oracle mismatches (the full run adds the raster comparison as a pass). Cycle 15 (FT-59) runs the page output through the
export job: the text CSVs, the PDF, the SVG set, AC-16's page texts and AC-17's PDF time come from
`ew_api::output`; the new AC-15 entry exports the checked-in fixture as `ew export pdf` does and finds the digest of
`expected/pdf/sha256.txt`; AC-16 sweeps 65 memoised families (the 14 of the page output and `assembly_break_marks` included). Cycle 15 lane B
closed the AC-07 assembly entry, D-17 (strict and lenient colour import with the quick fix "use BN"), D-36 (rows A1
and A2, the order code on folio 3) and D-41 (one assembly -201U1, break marks after -201U9 and before -202U1). Cycle 14 closed AC-01, AC-06, AC-13 and the two
AC-14 entries, D-03 (EF-15 counts 10: the seven frozen tags of doc 20 §18.1 and the three U-34 frozen test inputs, listed apart), D-14 (restated by the owner ruling: a named anchor policy that leaves the bus port's graphics, set on -201U1's bus representation alone, makes -201U1 print "3-B0", EW-CORE-053 reports it and its quick fix restores "3-B2"), D-20 (the Via notation "star at" and "chain on"), D-22's PELV entry,
D-35 (the §14.1 descriptions without ratings; EW-CORE-065 with the 1.0 content) and the info half of D-06 (a new
entry), and emptied AC-09's deviation list (FT-101). Cycle 14 lane B added the exports (AC-07 parts and AC-11; the
AC-07 assembly entry and D-36 followed in cycle 15), the freshness script
AC-16 (six entries, all pass: `ew verify-derived` is clean after each edit and the memoised outputs equal the cold
ones) and D-30 (the cover). The titles of the terminal-diagram and PLC-overview blocks are templates since lane B
("Terminal Diagram {strip}", "PLC Overview {device} {n}": the short tag of the block's first included strip or
device, resolved at derivation), so a renamed or renumbered strip renames its folios and TOC rows (AC-16's renumber
entry). `expected/geometry/frame.json` lists the stroked lines of a form's own graphics (`graphics`: the cover's two
rules and project data box) next to the frame lines, and AC-01's PDF check expects them. Cycle 13: 61 pass, 0 fail,
18 not_built, 6 not_applicable (decision log ID-338). AC-03 (36 folios, FT-111) and
AC-10 (19 potentials, FT-105) pass, and so do their SQL halves: `acceptance/potentials.sql` prints the DP row's
"(cable)" through the Preview column `api_v1_potential.bus_cable_ids`. An empty terminal designation refused at the parameter layer counts as refused for D-11 (ADR-0008).
