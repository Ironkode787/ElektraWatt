<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: GPL-3.0-or-later
-->

# ew-model invariant catalogue

## 1. Scope

This file is the catalogue of [ADR-0009](../../docs/adr/0009-invariant-catalogue.md) decision 5 for the
structural class. It lists every structural invariant that `ew-model` enforces.

Structural invariants are always on. They reject a batch in `Model::apply` with `ModelError::Invariant`,
or with `ModelError::Referenced` for a forbid link. They are evaluated on the candidate snapshot at the end
of the batch, limited to the touched keys and their index neighbours. Transient states inside a batch are
therefore legal. `Model::integrity_check`, run by `Model::from_records`, evaluates them on every record.

- Structural invariants are listed in section 3.
- The link table and delete policies are in section 4. Code: [`src/links.rs`](src/links.rs).
- Structural invariants that are not yet evaluated are listed in section 5.
- Code: [`src/invariants.rs`](src/invariants.rs).

Prevent rules are not in this file. They are rule-pack rules of `ew-checks`. They cover terminal designations,
tag, PLC address, interruption-point pairing and conductor colours. They plug into `Model::apply_with`
through `PreventHook` and are governed by `Strictness`.

## 2. ID scheme

Structural invariants are not rules. Doc 15 §2.2 says: "Invariant IDs name structural invariants, not rules".
They therefore take no `EW-<family>` rule ID.

Doc 08 §3.1 already names two of them, CI-01 and CI-04. Those IDs are reused. The others take `EW-INV-nnn`.
They are numbered in evaluation order and are never reused.

A violation found by the integrity check of an opened project is reported as:

- `EW-CORE-008` for a dangling reference, that is `EW-INV-001`;
- `EW-CORE-012` for any other structural invariant.

This follows doc 05 §6.3 and doc 15 §4.2.

## 3. Structural invariants

Every row has the class `Structural`. Test functions live in `src/tests/invariants.rs` unless a module is named.

| ID | Class | Statement | Touched keys considered | Tests |
|----|-------|-----------|-------------------------|-------|
| EW-INV-001 | Structural | Every link field of a record names an existing record of its target kind (link table in section 4). A removed record has no referrer left. A referrer through a forbid link makes the removal `ModelError::Referenced`. A referrer through a cascade or unlink link is a violation: the transaction must include `Model::delete_plan`. | Outgoing links of every touched record. For every removed key, its referrers through each link (named base index or the generic referrer index). | `inv001_refs_resolve_ok`<br>`inv001_refs_resolve_rejected`<br>`inv001_removed_target_with_cascade_referrer_rejected`<br>`inv001_forbid_referenced`<br>`integrity_check_on_load_rejected` |
| EW-INV-002 | Structural | A device that has functions has exactly one function of role Main (FR-DEV-004). A device without functions is allowed in v0. | Touched devices. The old and the new `device` of every touched function. | `inv002_one_main_function_ok`<br>`inv002_zero_functions_ok`<br>`inv002_one_main_function_rejected`<br>`inv002_main_moved_away_rejected` |
| EW-INV-003 | Structural | `Function.device` is None if and only if the function's kind is `Ip` (ADR-0009 amendment 2). | Touched functions. | `inv003_device_iff_not_ip_ok`<br>`inv003_device_iff_not_ip_rejected`<br>`inv003_ip_with_device_rejected` |
| CI-01 | Structural | Both endpoints of a connection exist and differ. Existence is reported through EW-INV-001 (the endpoint links). | Touched connections. | `ci01_endpoints_differ_ok`<br>`ci01_endpoints_differ_rejected` |
| EW-INV-004 | Structural | The functions of a representation exist (through EW-INV-001) and belong to one device. Interruption-point functions are excepted (doc 03, Representation). | Touched representations. `reps_by_function` of touched functions. | `inv004_rep_one_device_ok`<br>`inv004_ip_function_excepted_ok`<br>`inv004_rep_one_device_rejected`<br>`inv004_function_moved_rejected` |
| EW-INV-005 | Structural | Each symbol CP appears at most once in a representation's pin map (doc 03, Representation). | Touched representations. | `inv005_pin_map_unique_ok`<br>`inv005_pin_map_unique_rejected` |
| CI-04 | Structural | Every connection ending at a junction is in its sequence exactly once. Every sequence entry is a connection ending at it. | Touched junctions. The old and the new junction endpoints of touched connections. | `ci04_junction_sequence_ok`<br>`ci04_junction_sequence_rejected`<br>`ci04_rewired_connection_rejected`<br>`ci04_duplicate_entry_rejected`<br>`transient_violation_inside_batch_ok` |
| EW-INV-006 | Structural | The cable of a conductor is a device with the cable aspect. | Touched conductors. `conductors_by_cable` of touched devices. | `inv006_conductor_cable_ok`<br>`inv006_conductor_cable_rejected`<br>`inv006_cable_aspect_removed_rejected` |
| EW-INV-007 | Structural | The strip of a terminal is a device with the strip aspect (doc 03, Terminal). | Touched devices with the terminal aspect. `terminals_by_strip` of touched devices. | `inv007_terminal_strip_ok`<br>`inv007_terminal_strip_rejected`<br>`inv007_strip_aspect_removed_rejected` |
| EW-INV-008 | Structural | Structure node parent chains are acyclic. | The parent chain from every touched structure node. | `inv008_structure_acyclic_ok`<br>`inv008_structure_acyclic_rejected`<br>`inv008_self_parent_rejected` |
| EW-INV-009 | Structural | Device parent chains are acyclic (the assembly graph, ADR-0009 decision 2). | The parent chain from every touched device. | `inv009_device_acyclic_ok`<br>`inv009_device_acyclic_rejected`<br>`inv009_self_parent_rejected` |
| EW-INV-010 | Structural | A device with the terminal aspect has neither the strip nor the cable aspect (doc 03, Device). | Touched devices. | `inv010_terminal_aspects_ok`<br>`inv010_terminal_aspects_rejected` |
| EW-INV-011 | Structural | The parent of a structure node has the same aspect (doc 03, StructureNode). | Touched structure nodes. `structure_children` of touched nodes. | `inv011_structure_same_aspect_ok`<br>`inv011_structure_same_aspect_rejected` |
| EW-INV-012 | Structural | A page of kind Report is never stored. Report pages are generated from report blocks (doc 03, PageType). | Touched pages. | `inv012_no_stored_report_page_ok`<br>`inv012_no_stored_report_page_rejected` |

## 4. Link table and delete policies

Every ID-valued field of the persisted schema is either a link or listed as not a link. Links are in the table
below and in `LINKS` in [`src/links.rs`](src/links.rs). Fields that are not links are in `NOT_LINKS`.
The field-mask test `tests::field_mask` compares both lists with the JSON Schemas in `schemas/0.1`.

Policies follow doc 03 §4.2. Where doc 03 is silent, the policy is forbid for master data and library copies,
and unlink for optional cross-references. `cascade` and `unlink` are computed by `Model::delete_plan`.
Tests for it are in `src/tests/ops.rs`. `apply` never executes them.

Fields are written as `kind.path`. "referrers" means the generic referrer index.

| Field | Target | Policy | Index |
|-------|--------|--------|-------|
| project.source_ref.lineage | import_lineage | forbid | referrers |
| project.supply.origin.source | external_ref | unlink (cleared) | referrers |
| structure_node.parent | structure_node | forbid | structure_children |
| structure_node.source_ref.lineage | import_lineage | forbid | referrers |
| section.source_ref.lineage | import_lineage | forbid | referrers |
| external_ref.location | structure_node | forbid | referrers |
| external_ref.source_ref.lineage | import_lineage | forbid | referrers |
| page.folio.section | section | forbid | pages_by_section |
| page.form | form | forbid | referrers |
| page.structure.* (seven slots) | structure_node | forbid | referrers |
| page.source_ref.lineage | import_lineage | forbid | referrers |
| device.parent | device | forbid | device_children |
| device.parts[].part.copy | part | forbid | referrers |
| device.strip.accessories[].part.copy | part | forbid | referrers |
| device.structure.* (seven slots) | structure_node | forbid | referrers |
| device.terminal.strip | device | forbid | terminals_by_strip |
| device.source_ref.lineage | import_lineage | forbid | referrers |
| function.device | device | cascade | functions_by_device |
| function.def | fndef | forbid | referrers |
| function.kind.ip.external | external_ref | unlink (cleared) | referrers |
| function.source_ref.lineage | import_lineage | forbid | referrers |
| representation.functions[] | function | cascade (representation removed) | reps_by_function |
| representation.pin_map[].function | function | cascade (representation removed) | reps_by_function |
| representation.page | page | cascade | reps_by_page |
| representation.symbol | symbol | forbid | referrers |
| representation.source_ref.lineage | import_lineage | forbid | referrers |
| connection.a / connection.b (CP) | function | cascade | connections_by_endpoint |
| connection.a / connection.b (junction) | junction | cascade | connections_by_endpoint |
| connection.page | page | cascade | connections_by_page |
| connection.kind.core | conductor | unlink (Core reverts to Wire) | referrers |
| connection.kind.bus.cable | device | unlink (cleared) | referrers |
| connection.overrides.wire_part.copy | part | forbid | referrers |
| connection.source_ref.lineage | import_lineage | forbid | referrers |
| junction.page | page | cascade | junctions_by_page |
| junction.sequence[] | connection | unlink (entry removed) | referrers |
| junction.source_ref.lineage | import_lineage | forbid | referrers |
| conductor.cable | device | cascade | conductors_by_cable |
| conductor.source_ref.lineage | import_lineage | forbid | referrers |
| potential_def.at (CP or junction) | function / junction | unlink (definition dropped) | potential_defs_by_endpoint |
| potential_def.props.wire_part.copy | part | forbid | referrers |
| potential_def.source_ref.lineage | import_lineage | forbid | referrers |
| bond.a / bond.b (CP or junction) | function / junction | unlink (bond dropped) | bonds_by_endpoint |
| bond.via | connection | unlink (cleared) | referrers |
| bond.source_ref.lineage | import_lineage | forbid | referrers |
| report_block.form | form | forbid | referrers |
| report_block.section | section | forbid | referrers |
| report_block.embedded.page | page | cascade | referrers |
| report_block.source_ref.lineage | import_lineage | forbid | referrers |
| form.title_block | form | forbid | referrers |
| form.continuation | form | forbid | referrers |

### Not links

The following ID-valued fields are not links.

| Field | Reason |
|-------|--------|
| import_lineage.namespace | Computed v5 namespace, not a record |
| import_lineage.anchor.qet_project_uuid | Foreign project UUID |
| import_lineage.last_run | `import_run` is a reserved kind without a table in v0 |
| import_lineage.anchor.copy.of | the namespace of the original lineage, not a record ID |
| report_block.manual.include[] / exclude[] | Kind-erased selection; a stale ID selects nothing |
| report_block.source.sql_query.file | Asset ID of a query file |
| library_id, item_id, meta.status.deprecated.successor.* of part, symbol, fndef and form; part.manufacturer.*, part.function_template[].fndef.*, symbol.fndef.* | Library identity, or LibRef into an external library |

Each kind's own `id` is identity, not a link.

## 5. Structural invariants not evaluated in v0

These structural invariants are known but not evaluated yet.

| Invariant | Source | Why not yet |
|-----------|--------|-------------|
| A placement references a device | ADR-0009 decision 2 | The layout kinds are reserved until WP-S4.01 |
| A channel belongs to exactly one module | ADR-0009 decision 2 | The channel kind is reserved until the PLC work packages |
| A released revision has a checker who is not its author (D-34) | ADR-0009 decision 2 | The revision kind is reserved |
| CI-02 connection kinds join CPs of the right discipline | doc 08 §3.1 | CP disciplines come from instantiated CPs, which are derived data (WP-S1.05 to WP-S1.08) |
| CI-03 a bridge joins terminal CPs of one strip | doc 08 §3.1 | Needs the CP-to-terminal resolution of the derivation layer |
| CI-04, second sentence: a junction has at least three connections | doc 08 §3.1 | Restored by the junction clean-up normaliser in the same transaction (WP-S1.04). A delete plan cannot dissolve junctions |
| CI-07 overview connections take part in no net | doc 08 §3.1 | A property of the net derivation, not of stored data |
| Function CP overrides name existing CP indices | doc 03, Function | CPs are instantiated from the FnDef or the part pin pattern (derived) |
| Assembly `covers` lists only descendants; device links have existing, distinct endpoints | doc 03, Assembly and DeviceLink | The assembly aspect and the device_link kind are not in v0 |
| A form cell binds exactly one role; placeholders valid for the form kind | doc 03, Party and FormDef | Evaluated by the forms engine |
