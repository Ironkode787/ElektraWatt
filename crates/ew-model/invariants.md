<!--
SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
SPDX-License-Identifier: GPL-3.0-or-later
-->

# ew-model invariant catalogue

## 1. Scope

This file is the catalogue of [ADR-0009](../../docs/adr/0009-invariant-catalogue.md) decision 5 for the
structural class and for the one delete-time rule of `ew-model` (ADR-0009 amendment 7). It lists every
structural invariant that `ew-model` enforces, and that rule.

Structural invariants are always on. They hold in every committed state. They reject a batch in
`Model::apply_with` and `Model::apply_history` with `ModelError::Invariant`, or with `ModelError::Referenced` for a
forbid link. They are evaluated on the candidate snapshot at the end of the batch, limited to the touched keys
and their index neighbours. Transient states inside a batch are therefore legal. `Model::integrity_check`, run
by `Model::from_records`, evaluates them on every record.

The **delete-time rule** EW-INV-021 is a prevent rule of the model. It judges a removal by an edit, not a
state: a state without what it protects is valid. Two paths apply a batch:

- the **edit path**, `Model::apply_with`, by which every command commits (and the delete plan, which a delete
  command builds its batch from): structural invariants, then the delete-time rule (always strict), then the
  prevent hook;
- the **history path**, `Model::apply_history`, by which undo and redo restore a committed state: structural
  invariants only (ARCH §5.3: undo and redo skip prevent rules).

The integrity check never evaluates the delete-time rule.

- Structural invariants and the delete-time rule are listed in section 3.
- The link table and delete policies are in section 4. Code: [`src/links.rs`](src/links.rs).
- Structural invariants that are not yet evaluated are listed in section 5.
- Code: [`src/invariants.rs`](src/invariants.rs).

The configurable prevent rules are not in this file. They are rule-pack rules of `ew-checks`. They cover terminal designations,
tag, PLC address, interruption-point pairing and conductor colours. They plug into `Model::apply_with`
through `PreventHook` and are governed by `Strictness`. At MVP the shipped prevent rule is EW-TERM-002 (cycle 12, doc 08 §6.8); EW-RDS-004 has no prevent mode at MVP (ADR-0009 amendment 5) and EW-CORE-052's prevent mode is not wired (FT-194).

## 2. ID scheme

Structural invariants are not rules. Doc 15 §2.2 says: "Invariant IDs name structural invariants, not rules".
They therefore take no `EW-<family>` rule ID.

Doc 08 §3.1 already names two of them, CI-01 and CI-04. Those IDs are reused. The others take `EW-INV-nnn`.
They are numbered in evaluation order and are never reused.

The delete-time rule takes the next number, EW-INV-021: `apply_with` evaluates it after the structural
invariants. It is a rule of this catalogue, not of a rule pack. It has no code, Fluent message or registry
entry and never produces a finding: a refused edit is `ModelError::Referenced`, the refusal of a forbid link.
`EW-INV` therefore stays outside the rule families of doc 15 §2.2.

A violation found by the integrity check of an opened project is reported as:

- `EW-CORE-008` for a dangling reference, that is `EW-INV-001`;
- `EW-CORE-012` for any other structural invariant.

This follows doc 05 §6.3 and doc 15 §4.2.

## 3. Structural invariants and the delete-time rule

Every row has the class `Structural`, except EW-INV-021, which has the class `Delete-time`
(`RuleClass::DeleteTime`). Test functions live in `src/tests/invariants.rs` unless a module is named
(`ops::name` is `src/tests/ops.rs`). The test `every_catalogue_rule_has_a_test` checks that every rule of `RULES` has
a row here, and that every test named in its row exists, with at least one accepting and one rejecting test.

| ID | Class | Statement | Touched keys considered | Tests |
|----|-------|-----------|-------------------------|-------|
| EW-INV-001 | Structural | Every link field of a record names an existing record of its target kind (link table in section 4). A removed record has no referrer left. A referrer through a forbid link makes the removal `ModelError::Referenced`. A referrer through a cascade or unlink link is a violation: the transaction must include `Model::delete_plan`. A `LibRef` link takes part in neither half: its missing target is not dangling, and neither is a referrer that the removal of its copy leaves behind (its keep-alive is EW-INV-021). The kind-erased link `macro_instance.members[][1]` resolves when a record of any of its member kinds has the ID (section 4). | Outgoing links of every touched record. For every removed key, its referrers through each link other than a `LibRef` link (named base index or the generic referrer index). | `inv001_refs_resolve_ok`<br>`inv001_lib_ref_referrer_left_by_removal_ok`<br>`inv001_refs_resolve_rejected`<br>`inv001_removed_target_with_cascade_referrer_rejected`<br>`inv001_forbid_referenced`<br>`integrity_check_on_load_rejected`<br>`ops::lib_ref_unembedded_target_is_not_a_link` |
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
| EW-INV-010 | Structural | A device carries at most one of the terminal, strip and cable aspects: a terminal is neither a strip nor a cable, and a strip is neither a terminal nor a cable (doc 03, Device and Strip). | Touched devices. | `inv010_terminal_aspects_ok`<br>`inv010_strip_and_cable_alone_ok`<br>`inv010_terminal_aspects_rejected`<br>`inv010_terminal_cable_rejected`<br>`inv010_strip_cable_rejected`<br>`inv010_aspect_added_by_update_rejected` |
| EW-INV-011 | Structural | The parent of a structure node has the same aspect (doc 03, StructureNode). | Touched structure nodes. `structure_children` of touched nodes. | `inv011_structure_same_aspect_ok`<br>`inv011_parent_and_children_changed_together_ok`<br>`inv011_structure_same_aspect_rejected`<br>`inv011_parent_aspect_changed_rejected` |
| EW-INV-012 | Structural | A page of kind Report is never stored. Report pages are generated from report blocks (doc 03, PageType). | Touched pages. | `inv012_no_stored_report_page_ok`<br>`inv012_no_stored_report_page_rejected` |
| EW-INV-013 | Structural | A function of kind `Ip` has exactly one representation. The representation owns it: the function lives in the page of that representation (doc 05 §2.2, row 32) and `Model::delete_plan` removes it with its last representation. Evaluated at the end of the batch, so `ip.place` may create both in one transaction. | Touched functions. The functions named (`functions[]`, `pin_map[].function`) by the old and the new row of every touched representation. | `inv013_ip_with_one_rep_ok`<br>`inv013_ip_without_rep_rejected`<br>`inv013_second_rep_rejected`<br>`inv013_rep_removed_without_function_rejected`<br>`inv013_rep_repointed_rejected`<br>`ops::delete_plan_ip_representation_cascades_its_function` |
| EW-INV-014 | Structural | A page-oriented report block (one without `embedded`) has a `first_folio` (doc 03 §6, ReportBlock; doc 09 §2.2). | Touched report blocks. | `inv014_report_block_folio_ok`<br>`inv014_embedded_without_folio_ok`<br>`inv014_report_block_folio_rejected`<br>`inv014_embedded_removed_rejected` |
| EW-INV-015 | Structural | The device inheritance graph is acyclic. Its edges are terminal → strip (`Device.terminal.strip`) and device → parent (`Device.parent`): the edges along which `device_tag` and `structure_of` inherit designations and structure (doc 03 §8.2 steps 1, 3 and 5). EW-INV-009 covers the parent edges alone; this invariant also rejects a strip whose parent is one of its terminals, and longer mixed cycles, so that no loaded model can make a derivation recurse into itself. | Every touched device, walking both edges depth first. | `inv015_inheritance_acyclic_ok`<br>`inv015_strip_whose_parent_is_its_terminal_rejected`<br>`inv015_three_device_cycle_rejected`<br>`inv015_cycle_rejected_on_load` |
| EW-INV-016 | Structural | At most one revision is in state `Open`: the revision that collects the current edits (doc 03 §18, doc 15 §5.1, §5.3). A released revision is never reopened; the next revision is a new record. | Every revision, when the batch touches one (a project has a handful, so the table is scanned). | `inv016_one_open_revision_ok`<br>`inv016_second_open_revision_rejected`<br>`inv016_reopened_revision_rejected`<br>`inv016_two_open_revisions_rejected_on_load` |
| EW-INV-017 | Structural | Revision labels are unique among the revisions that are not in state `Withdrawn`: a label names one revision, its archive `<label>.ewarchive` and its tag `ew/rev/<label>` (doc 03 §18, ADR-0013). A withdrawal record may repeat the label of the revision it withdraws. | Touched revisions, against every revision. | `inv017_revision_label_unique_ok`<br>`inv017_duplicate_label_rejected`<br>`inv017_relabel_rejected` |
| EW-INV-018 | Structural | A revision's checker, when set, is not its author (four eyes; D-34, FR-REV-005, ADR-0009 decision 2). Author and checker are party roles. That a revision released by EW has a checker at all is the release gate's (EW-REL-001); a historic released revision has neither (section 5). | Touched revisions. | `inv018_revision_four_eyes_ok`<br>`inv018_author_is_checker_rejected` |
| EW-INV-019 | Structural | The page notes of a revision name each page at most once (FR-REV-012: one optional description per changed page). `page_notes[].page` is not a link (section 4, "Not links"): a note keeps naming a page that a later revision deletes. | Touched revisions. | `inv019_page_notes_unique_ok`<br>`inv019_two_notes_on_one_page_rejected` |
| EW-INV-020 | Structural | A revision that was `Released` or `Withdrawn` before the batch is not updated (FR-REV-009, doc 03 §18: released revisions are immutable; later edits belong to the next revision, and a withdrawal is a new record that names the withdrawn revision in `withdraws`, a forbid link). Removing it stays possible (the undo of `revision.create` deletes the record), so does an open revision becoming `Released` (the release command, not built) and a new withdrawal record. `fields.set` and `edit.delete` refuse a closed revision before the model does, with a message at the target. | Touched revisions whose row before the batch was released or withdrawn and that the batch did not remove. The integrity check has no rows before it and never reports this rule. | `inv020_released_revision_delete_ok`<br>`inv020_released_revision_update_rejected` |
| EW-INV-022 | Structural | The page of a page item or a structure box exists (WP-S2.04 part 4a, ADR-0009 amendment 8). It is the half of EW-INV-001 for the cascade links `page_item.page` and `structure_box.page` (a violation is reported as EW-INV-001), and `Model::delete_plan` deletes a page's items and boxes with it (FR-PAG-025). | The outgoing links of every touched page item and structure box; the referrers of a removed page. | `inv022_items_and_boxes_on_their_page_ok`<br>`inv022_item_on_missing_page_rejected`<br>`ops::delete_plan_page_cascades_items_and_boxes` |
| EW-INV-023 | Structural | Every connection a wire marker lists exists and lies on the marker's page (WP-S2.04 part 4a, ADR-0009 amendment 8). The link `page_item.kind.wire_marker.connections[]` is unlink and **load-tolerant** (section 4): a deleted connection leaves the list and a marker left empty is deleted in the same plan, while a listed connection that is missing or on another page is judged by this rule, not by EW-INV-001. A build from before part 4a kept page items verbatim and did not enforce the link, so a loaded project can hold a stale entry: that is a load finding (`Session::load_findings`), not a violation, the derivation and the drawing ignore the entry, and the next edit of the marker may keep it or write a clean list. Therefore the rule judges what an **edit** adds: `Model::apply_with` only; the history path restores committed states (the undo of a cleaning edit restores the stale entries) and the integrity check has no rows before it. | Edit path only. Every touched wire marker: the entries absent from its row before the batch. Every touched connection whose page the batch changed: the markers listing it (generic referrer index). | `inv023_marker_on_its_page_ok`<br>`inv023_stale_list_loads_and_edits_keep_it_ok`<br>`inv023_missing_connection_rejected`<br>`inv023_connection_on_another_page_rejected`<br>`inv023_connection_moved_off_its_page_rejected`<br>`inv023_added_stale_entry_rejected`<br>`ops::delete_plan_connection_unlinks_markers_and_drops_an_empty_one` |
| EW-INV-021 | Delete-time | An edit does not remove an embedded library copy that a record still names through a `LibRef` link (`symbol.fndef`, `part.function_template[].fndef`; section 4): the removal is `ModelError::Referenced`, the refusal of a forbid link, whatever the project's strictness. A state without the copy is valid (the `LibRef` names nothing, NFR-REL-11), so the history path does not evaluate the rule: the undo of `library.embed` restores the state before the embedding (FT-316, ADR-0009 amendment 7), and redo embeds again. `Model::delete_plan` refuses the same removal before a delete command (`edit.delete`, the purge) builds its batch, with the same error. | Every key the batch removed (it existed before the batch and not after it), with its referrers through each `LibRef` link (generic referrer index). Edit path only. | `inv021_unnamed_copy_removed_by_an_edit_ok`<br>`inv021_undo_of_an_embedding_restores_the_state_before_ok`<br>`inv021_named_copy_removed_by_an_edit_referenced`<br>`ops::lib_ref_fndef_copy_in_use_is_referenced`<br>`ops::lib_ref_part_template_fndef_in_use_is_referenced` |

## 4. Link table and delete policies

Every ID-valued field of the persisted schema is either a link or listed as not a link. Links are in the table
below and in `LINKS` in [`src/links.rs`](src/links.rs). Fields that are not links are in `NOT_LINKS`.
The field-mask test `tests::field_mask` compares both lists with the JSON Schemas in `schemas/0.1`.

Policies follow doc 03 §4.2. Where doc 03 is silent, the policy is forbid for master data and library copies,
and unlink for optional cross-references. `cascade` and `unlink` are computed by `Model::delete_plan`.
Tests for it are in `src/tests/ops.rs`. `apply_with` and `apply_history` never execute them.

**Ownership cascade.** Besides the link policies, `Model::delete_plan` removes an interruption-point function
together with its last representation (EW-INV-013, doc 05 §2.2 row 32). The function's connections then cascade
through `connection.a`/`b`, and its potential definitions and bonds are dropped. Tests:
`delete_plan_page_cascades_page_owned_rows`, `delete_plan_ip_representation_cascades_its_function`,
`delete_plan_ip_function_cascades_its_representation`, `delete_plan_device_rep_keeps_device_function`.

**`LibRef` links.** `symbol.fndef` and `part.function_template[].fndef` are `LibRef{library, item}`. Within a project
a `LibRef` resolves to the embedded copy of `(library, item)` (doc 07 §3.1), whose ID is `v5(NS_LIBCOPY, library ‖
item)` (doc 05 §13.2). The link's target is that copy ID, computed from the referring row alone. The field is a link
exactly while the copy is embedded (`LinkDecl::lib_ref`):

- while the copy exists, an **edit** that removes it is `ModelError::Referenced`: the delete plan refuses it, and
  `Model::apply_with` rejects a batch that removes it anyway (EW-INV-021, a delete-time rule);
- a `LibRef` to an item the project does not embed is not a dangling reference, so a project opens without its
  libraries (NFR-REL-11). The removal of the copy therefore leaves a valid state, and undo and redo
  (`Model::apply_history`) may remove it: the undo of `library.embed` restores the state before the embedding
  (FT-316). Crash recovery onto a moved base judges its net removals against that base the same way: a copy the
  replay removes while a record of the result names it is a collision (`ew_store::journal::replay`);
- a second copy minted by "keep both" (`‖ version`) is never the resolution of a `LibRef`.

The decision is deterministic: the index entry depends on the row only, and the link's state on the snapshot only.

**The load-tolerant link** (`LinkDecl::load_tolerant`, WP-S2.04 part 4a). `page_item.kind.wire_marker.connections[]`
is an ordinary unlink link for removals: deleting a connection takes it out of every marker's list, and a marker left
empty is dropped (deleted in the same plan); a batch that removes a listed connection without the plan is EW-INV-001.
Its presence half is EW-INV-023's: a listed connection that does not exist, or lies on another page, is judged for
the entries an edit adds, never on load, because a build from before part 4a kept the records verbatim and a stale
entry is a load finding (ADR-0009 amendment 8).

**The kind-erased link** (`LinkDecl::kind_erased`, WP-S1.19 part 1). `macro_instance.members[][1]` stores the
records a placed macro created as bare IDs (`(fragment-local, created)` pairs, doc 03 §7), so the link names a record
of any kind of `MACRO_MEMBER_KINDS` (page, device, function, representation, connection, junction, conductor, channel,
potential definition, bond, device link, page item, structure box, report block). It resolves when a record of one of
those kinds has the ID; the generic referrer index keys it under the nominal `to` (page); the removal of a record of
any of the kinds consults it. Its policy is unlink: a deleted member leaves the list and the instance stays, also
when the list is left empty (it is still the record of the placement). Tests: `macro_links::*`.

**Reading the model.** The untracked read API (`Model::table`, `row`, `lib`, `indexes`, `referrers`, `delete_plan`
and the table and index getters) is behind the cargo feature `raw-read` of `ew-model` (ARCH §3 rule 1, ADR-0002
decision 1). `delete_plan` is part of it because its ops carry the current rows (`before`); `apply_with` and `apply_history`
only consume ops and stay public. Only
`ew-derive`, `ew-commands` and `ew-store` enable it on a normal dependency edge; `cargo xtask check-features`
enforces this. Prevent rules do not read the model directly: `PreventHook::check(&Model, &Model, &[AnyKey])`
(base, candidate, touched keys) is implemented by `ew-api`, which builds the tracked query contexts of the base
and the candidate model through `ew-derive` and calls the prevent rules of `ew-checks` with them (`ew-commands`
may not depend on `ew-checks`, ci/dep-rules.toml). `ew-checks` reads through queries only (ADR-0019) and does not
enable `raw-read`.

Fields are written as `kind.path`. "referrers" means the generic referrer index.

| Field | Target | Policy | Index |
|-------|--------|--------|-------|
| project.source_ref.lineage | import_lineage | forbid | referrers |
| project.supply.origin.source | external_ref | unlink (cleared) | referrers |
| plc_config_project.cpu | device | unlink (cleared) | referrers |
| plc_config_project.source_ref.lineage | import_lineage | forbid | referrers |
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
| device.assembly.configs[].part.copy | part | forbid | referrers |
| device.assembly.configs[].covers[] | device | unlink (entry removed) | referrers |
| device.structure.* (seven slots) | structure_node | forbid | referrers |
| device.terminal.strip | device | forbid | terminals_by_strip |
| device.plc_station.config_project | plc_config_project | forbid | referrers |
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
| channel.module | device | cascade | referrers |
| channel.binding (CP) | function | unlink (cleared) | referrers |
| channel.binding (junction) | junction | unlink (cleared) | referrers |
| channel.target | function | unlink (cleared) | referrers |
| channel.source_ref.lineage | import_lineage | forbid | referrers |
| potential_def.at (CP or junction) | function / junction | unlink (definition dropped) | potential_defs_by_endpoint |
| potential_def.props.wire_part.copy | part | forbid | referrers |
| potential_def.source_ref.lineage | import_lineage | forbid | referrers |
| bond.a / bond.b (CP or junction) | function / junction | unlink (bond dropped) | bonds_by_endpoint |
| bond.via | connection | unlink (cleared) | referrers |
| bond.source_ref.lineage | import_lineage | forbid | referrers |
| device_link.from / device_link.to | device | cascade | referrers |
| device_link.source_ref.lineage | import_lineage | forbid | referrers |
| page_item.page | page | cascade | page_items_by_page |
| page_item.kind.wire_marker.connections[] | connection | unlink (entry removed; a marker left empty is dropped), load-tolerant (EW-INV-023) | referrers |
| structure_box.page | page | cascade | structure_boxes_by_page |
| structure_box.assigns.* (seven slots) | structure_node | forbid | referrers |
| structure_box.external | external_ref | unlink (cleared) | referrers |
| report_block.form | form | forbid | referrers |
| report_block.section | section | forbid | referrers |
| report_block.embedded.page | page | cascade | referrers |
| report_block.source_ref.lineage | import_lineage | forbid | referrers |
| macro_instance.macro | macro | forbid | referrers |
| macro_instance.members[][1] | any of `MACRO_MEMBER_KINDS` (kind-erased) | unlink (member removed; the instance stays) | referrers |
| macro_instance.page | page | cascade | referrers |
| revision.withdraws | revision | forbid | referrers |
| revision.source_ref.lineage | import_lineage | forbid | referrers |
| form.title_block | form | forbid | referrers |
| form.continuation | form | forbid | referrers |
| symbol.fndef.item (with `.library`) | fndef (copy `v5(NS_LIBCOPY, library ‖ item)`) | forbid at delete time (EW-INV-021), while the copy is embedded | referrers |
| part.function_template[].fndef.item (with `.library`) | fndef (copy `v5(NS_LIBCOPY, library ‖ item)`) | forbid at delete time (EW-INV-021), while the copy is embedded | referrers |

**Forbid links: structural or delete-time** (ADR-0009 amendment 7). A forbid link is structural when its missing
target is a dangling reference in any state: then no committed state holds a referrer without its target, so a
restore of a committed state (undo, redo) never removes a target that a referrer of that state names, and the
forbid only chooses the refusal (`Referenced`, not a dangling `Invariant`) and keeps the delete plan from
cascading. A forbid link is delete-time when its missing target is valid, so that a removal can be wrong while the
state after it is right. Every forbid link stores its target's ID, except the two `LibRef` links:

| Forbid links | Target | Ruling | Why |
|-------|--------|--------|-----|
| `source_ref.lineage` of the 18 kinds that carry it | import_lineage | structural | A stored ID; the lineage anchors the identity (the v5 namespace) of its records |
| structure_node.parent, external_ref.location, page.structure.*, device.structure.* and structure_box.assigns.* (seven slots each) | structure_node | structural | Stored IDs; structure and designations derive along them (doc 03 §4.2: merged, never deleted under its users) |
| page.folio.section, report_block.section | section | structural | Stored IDs; folios and report placement derive from them |
| device.parent, device.terminal.strip | device | structural | Stored IDs; the device and inheritance graphs (EW-INV-007, EW-INV-009, EW-INV-015) |
| device.plc_station.config_project | plc_config_project | structural | A stored ID; the station's addressing derives from it |
| device.parts[].part.copy, device.strip.accessories[].part.copy, device.assembly.configs[].part.copy, connection.overrides.wire_part.copy, potential_def.props.wire_part.copy | part | structural | `PartRef.copy` stores the copy ID; the command that uses a part embeds it in the same transaction (FR-PRT-013), so a part use never names an item that is not embedded |
| function.def | fndef | structural | A stored copy ID; the function's CPs instantiate from it |
| representation.symbol | symbol | structural | A stored copy ID; the representation draws it |
| macro_instance.macro | macro | structural | A stored copy ID; the instance records which macro version was placed (WP-S1.19 part 1) |
| page.form, report_block.form, form.title_block, form.continuation | form | structural | Stored copy IDs; a form is embedded with the forms it names (the closure of `library.embed`), and a page or report block cannot render without its form |
| revision.withdraws | revision | structural | History: a withdrawal record names the revision it withdraws (FR-REV-009) |
| symbol.fndef.item, part.function_template[].fndef.item | fndef | delete-time (EW-INV-021) | `LibRef`: the target ID is computed from the referring row, and a `LibRef` to an item the project does not embed names nothing (NFR-REL-11) |

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
| revision.page_notes[].page | History: a released revision is immutable (FR-REV-009), so its notes keep naming a page that a later revision deletes; a note on a page that does not exist selects nothing |
| library_id, item_id of part, symbol, fndef and form | Library identity of the copy, not a reference |
| meta.status.deprecated.successor.* of part, symbol, fndef and form | `LibRef` to a library item offered by "update from library"; never resolved to an embedded copy |
| part.manufacturer.* | `LibRef` to a manufacturer record, which has no library-copy table in v0 |
| symbol.fndef.library, part.function_template[].fndef.library | The library half of the `LibRef` links above (the `.item` field carries the link) |

Each kind's own `id` is identity, not a link.

## 5. Structural invariants not evaluated in v0

These structural invariants are known but not evaluated yet.

| Invariant | Source | Why not yet |
|-----------|--------|-------------|
| A placement references a device | ADR-0009 decision 2 | The layout kinds are reserved until WP-S4.01 |
| A channel's module carries the `plc_module` aspect | ADR-0009 decision 2; doc 10 §2.4 | The `channel.module` link (cascade) makes every channel belong to exactly one existing device (EW-INV-001); that the device is a module is the orphan-channel check FT-229, not a structural invariant |
| A channel's number is below its module part's channel count | doc 10 §2.4, §3.3; FR-PLC-053 | Not an invariant by design (cycle 18, FT-277): a part change through `fields.set` keeps the surplus channels, with a notice of normaliser N1, because FR-PLC-053 removes them only after a confirmation that lists them, which is the preview of the Bulk command `plc.set_module_part`. The derivation reads only the channels below the count, so a surplus channel prints nowhere |
| A channel's kind is its module part's channel kind | doc 10 §2.4, §2.6 | Restored by normaliser N1 when the module's part or part use changes (FT-276); a kind of a newer format is kept |
| A released revision has a checker who is not its author (D-34) | ADR-0009 decision 2 | The "not its author" half is EW-INV-018. The "has a checker" half does not hold for a historic released revision (Golden A's V1.0 has neither author nor checker): it applies to a revision that EW releases, which the release gate refuses without a valid sign-off (EW-REL-001, `revision.release`, not built). The design pass words ADR-0009 decision 2 accordingly (FT-165 report) |
| Revision labels increase in natural order and follow the project's one label format (FR-REV-002) | doc 15 §4.8 EW-REL-004 | A prevent and release-gate rule of the EW-REL pack, not a structural invariant; the label-format setting does not exist yet. The current revision of a derivation is fixed without it: the open one, else the released ones by date, natural label order and ID (`ew-derive` `title_block`) |
| CI-02 connection kinds join CPs of the right discipline | doc 08 §3.1 | CP disciplines come from instantiated CPs, which are derived data (WP-S1.05 to WP-S1.08) |
| CI-03 a bridge joins terminal CPs of one strip | doc 08 §3.1 | Needs the CP-to-terminal resolution of the derivation layer |
| CI-04, second sentence: a junction has at least three connections | doc 08 §3.1 | Restored by the junction clean-up normaliser in the same transaction (WP-S1.04). A delete plan cannot dissolve junctions |
| CI-07 overview connections take part in no net | doc 08 §3.1 | A property of the net derivation, not of stored data |
| Function CP overrides name existing CP indices | doc 03, Function | CPs are instantiated from the FnDef or the part pin pattern (derived) |
| Device-link endpoints are distinct | doc 03, DeviceLink | The command `device.link` refuses a self-link; the model does not. Existence is EW-INV-001 (the `from` and `to` links). Assembly `covers` outside the assembly's subtree are a check, not an invariant (decision log ID-371, FT-260) |
| A form cell binds exactly one role; placeholders valid for the form kind | doc 03, Party and FormDef | Evaluated by the forms engine |
