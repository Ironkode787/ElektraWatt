# WP-S1.20: Fluid power model, the MVP (closed specification)

**Status:** complete in cycle 50, built in the seven steps by four lanes and its fix round FIX120 (decision log ID-1340, ID-1353; the PERT re-estimated by ID-1316 O1); specified in cycle 50 (documents only), an M2 package built whole by one lane (ID-1281) in the seven steps
of §6, with one hand-back point (§11); about 3.7 pw against the PERT of 3.1 pw, which stays until the orchestrator
rules on §13 O1 · **Date:** 2026-11-04 · **Work package:**
[WP-S1.20](../02-work-packages.md#wp-s120-fluid-power-model-mvp) · **Decision log:** ID-1311 · **Part of:**
ElektraWatt plan, specifications

**Purpose.** Golden A's pneumatic folios 201 to 206 are drawn, but their fluid side is not data: a valve port has no
role, a fluid line has no connection kind, a valve's solenoid is a non-physical CP inside its valve function (the
interim rule of decision log ID-469), no valve position has an occupancy, no `Drives` link is made from a drawn line,
and no EW-FL rule exists, so AC-21 still leaves EF-13 and EF-14 pending (FT-589) and AC-32's MVP part cannot pass.
This document closes doc 11's MVP row ([fluid power](../../design/11-fluid-power.md) §12) to decisions, a build order
and acceptance, against the code at `c97e3a15` (§2.1). Valve models, media, galleries and tubes are WP-S1.25's. Its
size is an estimate, not a commitment (ID-1210); every time is not measured (owner's rule). Where this document and a
design document differ, this document is the narrower and wins for the lane; the design documents stay the target and
are updated at the design pass (§13). A choice the lane meets that this document does not settle is escalated, never
guessed.

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [The texts](#5-the-texts)
6. [Build order](#6-build-order)
7. [Acceptance](#7-acceptance)
8. [Golden A's expected changes](#8-golden-as-expected-changes)
9. [Neighbours, files owned and what a lane may not decide](#9-neighbours-files-owned-and-what-a-lane-may-not-decide)
10. [Risks](#10-risks)
11. [Size, routing and the hand-back point](#11-size-routing-and-the-hand-back-point)
12. [Follow-ups, proposed](#12-follow-ups-proposed)
13. [Open questions and the documents to update](#13-open-questions-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package every valve port of Golden A carries its role, a solenoid valve is a valve function
plus one coil function per solenoid its part declares, a valve position is a valve (used, spare, reserved or not
fitted), a blanking plate or empty, a fluid line is a connection of kind `Fluid` with a line kind that joins fluid ports
only, the first line from a valve working port to an actuator links the valve to the device it drives, a used valve
prints the driven device's process tag, folios 201 and 202 draw one glyph per valve position, Golden A carries its 16
lines F1 to F16, the five MVP EW-FL rules report on Golden A exactly as doc 20 §18.1 says (EF-13, EF-14), AC-21 compares
the whole `findings_mvp.csv`, and AC-32's MVP part passes.

**Non-goals** (each with its package or follow-up):
- No valve model, no media, no medium definition, no gallery, no tube specification, no length, no line tag, no tube
  counter (N-FL-5), no fluid interruption point, no fluid report, no fluid BOM line, and none of EW-FL-005 to -010,
  -012 and -014 (WP-S1.25, 1.0). `FluidLine.tube` and `FluidLine.length` stay in the line's `extra`.
- No `ValveTerminal` aspect (its one member, `pilot_supply`, is read by the galleries of 1.0): a valve terminal is the
  parent of modules that carry a solenoid-position map (D8). The aspect stays in `extra` (WP-S1.25).
- No push insert of N-FL-2 and no `assembly.detach` (FT-273, WP-S1.13's remainder; §13 O3), no generated
  overview pages (`valve_terminal.generate_overview`, FR-FLU-033, 1.0).
- No derived mechanical linkage line between an actuator and its process valve (FR-SYM-014, WP-S3.01).
- No QET pneumatic import: mapping table, text recognisers and the reconstruction assistant (FR-IMP-017, FR-IMP-018,
  FR-IMP-039, WP-S3.12); no Golden C change.
- No fluid navigator, fluid workspace or slot table (FR-UI-003, FR-UI-006, FR-FLU-015, 1.0); no device-navigator
  context actions (FP-2); no autoconnect on fluid ports and no line-kind picker in the Wire tool (FP-3); the bus half of
  FT-434 stays with WP-S2.04.
- No new Rust dependency; no change to `deny.toml`, `supply-chain/` or `Cargo.lock`.

## 2. Inputs and the code as it is

| Input | What the lane takes from it |
|---|---|
| [Fluid power](../../design/11-fluid-power.md) §2.1, §2.2, §2.4, §2.7, §3.1 to §3.3, §4.1 to §4.3, §5.1, §5.5, §5.6, §6, §7, §10, §11, §12 | Records, port roles, line kinds, solenoid valves, valve terminals, field devices, invariants, normalisers, commands, the partition, the overview, the rule pack, the golden facts, the MVP row |
| [Domain model](../../design/03-domain-model.md) §15; [project format](../../design/05-project-format-and-persistence.md) §2.2, §3.1, §10.3; ADR-0011 amendments 1, 2 and 4 | The fluid records, member order, the additive rules |
| [Schematic editor](../../design/06-schematic-editor.md) §3.2 (rows 1, 13, 16) | The normaliser registry |
| [Symbols, parts and macros](../../design/07-symbols-parts-macros.md) §2.3, §2.4, §2.5, §4.5, §9.2 | Slots, FnDef CPs, part sections, the update classification, 571334's solenoids |
| [Checks](../../design/15-checks-revisions-collaboration.md) §2.2, §3.6, §4.5.3 | Pack EW-FL (class 120, codes `1200nn`), the rule rows |
| [Golden project](../../design/20-golden-project.md) §6.2, §12, §18.1, §21 (AC-21, AC-32), §23.1 | The valves, field devices, lines F1 to F16, EF-13, EF-14 |
| ADR-0004 amendment 2, ADR-0009 (decisions 2 to 4), ADR-0019 | Template-function IDs, structural and prevent rules, packs |
| Specifications | [WP-S1.21](WP-S1.21-plc-and-bus-model.md) D2, D7, D16 (normaliser pattern, the `PlcAddress` slot); [WP-S1.16](WP-S1.16-mvp-rule-packs.md) D10 (the AC-21 subset), its rule-page and message pattern |
| Follow-ups | FT-263, FT-273, FT-322, FT-434, FT-589, FT-744 |

### 2.1 What exists at `c97e3a15` (the base contract)

The lane relies on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used for |
|---|---|---|
| `ConnectionKind`: `wire`, `core`, `bridge`, `bus`, `overview`, `internal`, `Unknown`; no `fluid` (its doc comment names `Fluid(FluidLine)` as additive) | `crates/ew-schema/src/connect.rs:66-88` | D2 |
| Electrical nets join `wire`, `core`, `bus`, `internal` only (`joins_net`, a whitelist) | `crates/ew-derive/src/nets.rs:83` | D12 |
| `Discipline` (`electrical`, `pneumatic`, `vacuum`, `hydraulic`, `bus`); `CpDef` ends with `physical`; "Not in v0: `valve_model` and `CpDef.port_role`" | `crates/ew-schema/src/library.rs:616-631`, `:868`, `:926-962` | D2, D3 |
| `TextSlot` ends with `style`, then `extra`; `SlotKind::FunctionText` prints the device's manual function text | `crates/ew-schema/src/library.rs:712-739`; `crates/ew-api/src/output/view.rs:770-775` | D14 |
| `PartSections` ends with `bus`, then `extra`; no `fluid` | `crates/ew-schema/src/library.rs:1492-1517` | D2, D4 |
| `CpOverride` ends with `unused`; "Not in v0: `CpOverride.closure`"; `DeviceKind::BlankingPlate`; `DeviceState` used, spare, reserved, not fitted; `Device.process_tag`; `bus_node` and `valve_terminal` kept in `extra` | `crates/ew-schema/src/device.rs:34-36`, `:76`, `:223-253`, `:564`, `:686-707` | D2, D8, D9 |
| Template-origin CPs pair with the FnDef's CPs by position when the counts match and take discipline and physical from the FnDef CP | `crates/ew-derive/src/cps.rs:60-82` | D3, D4 |
| The interim declaration rule: `coil_cp_idx(def, s)`, the first non-physical CP named s of a FnDef; `driven_device` (`Drives` from the device, smallest link ID) | `crates/ew-derive/src/plc.rs:316-326`, `:1169-1175` | D5, D13 |
| `valve.add {module, position?, part, letter?, fndef?, structure?}`: child tag counter = position, held position refused, main function only from `fndef`; its refusals name record IDs | `crates/ew-commands/src/commands/assembly.rs:181-279` | D7, FP-6 |
| `device.link {from, to, kind}` refuses a self-link and a duplicate | `crates/ew-commands/src/commands/assembly.rs:285-344` | D11 |
| `connection.draw {a, b, page?, route, kind}` | `crates/ew-commands/src/commands/connection.rs:131-216` | D11, D18 |
| Normaliser `ORDER`: `plc_aspects`, `cp_instantiation`, `channel_instantiation`, `online_numbering`, `connection_split`, `junction_cleanup`, `wire_numbering`, `solenoid_binding`, `module_pin_binding`, `bus_ports`, `revision_ensure_open` | `crates/ew-commands/src/normalise/mod.rs:68-80` | D6, D15 |
| N2 binds a declared coil to the channel the position map assigns; its notices cite EW-FL-002 | `crates/ew-commands/src/normalise/solenoid_binding.rs:1-44`, `:295` | D5 |
| The structural catalogue ends at EW-INV-023 | `crates/ew-model/src/invariants.rs:94-102` | D11 |
| The prevent hook runs `ew-checks`' prevent rules; release mode runs rules whose modes include release or prevent; pre-delivery switches EW-FL-008 and EW-FL-014 off | `crates/ew-api/src/lib.rs:404-422`; `crates/ew-checks/src/scheme.rs:39-47`, `:104-108` | D16 |
| The rule catalogue has no EW-FL row; EF-13 and EF-14 are pending on WP-S1.20 | `crates/ew-checks/catalogue/rules.json`; `crates/ew-api/tests/expected/findings_mvp.csv:14-15` | D16 |
| The Wire tool refuses a fluid page with `reason-no-wire-kind` | `crates/ew-viewmodel/src/shell/wiring.rs:129-141` | D18 |
| Starter content: `ew.fn.valve-5-2-sol-spring` 0.1.0 (CPs 1 to 5 pneumatic, CP 5 "14" non-physical), `ew.fn.cylinder-double-acting` (A, B pneumatic), `ew.fn.process-valve-2way` (1, 2), `ew.fn.valve-coil` (A1, A2); the symbol `ew.iso.valve-5-2-sol-spring` 0.2.0 (five CPs; slots `function_text`, `plc_address`); `ew.ovw.module` (24 × 16 mm) and `ew.ovw.valve-top` (6 × 12 mm) 0.1.0; `EW-GEN-VLV-1` 0.1.1 in `ew-starter-parts` 0.3.0 (template: `ew.fn.valve-coil` main on A1, A2, the 5/2 valve on pattern CPs 2 to 7) | `content/fndefs/`, `content/symbols/`, `content/libraries/ew-starter-parts.ewlib/` | D3, D4, D14, D19 |
| Golden A: 28 valves of part 571334 with one function each on the 5/2 FnDef, 8 used and 20 spare ("Spare" as manual text); field devices -203Q1 … -203Q8 in +FIELD with process tags; 8 `Drives` links; 28 channel bindings to `<valve>:14`; 533361's PLC section with 4 positions and the map `14:2p;12:2p+1`; no fluid connection; the valves drawn with arrangement `ports`; folios 201 and 202 print module tags, part texts, "∞" lists and PART DATA, no valve glyph; folio 203 prints no process tag | `golden/J-E-2416/tables/` (`devices.csv`, `device_links.csv`, `channels.csv`, `part_plc.csv`, `connections.csv`, `representations.csv`); `golden/J-E-2416/expected/text/201.csv` to `206.csv` | D19, D20 |
| Golden A's tables are generated from doc 20 by `tables_from_doc20.py`; AC-15's path (b) builds the valves with `valve.add` | `golden/J-E-2416/tools/tables_from_doc20.py`; `crates/ew-testkit/src/golden/translate.rs:157-160`, `:498-516` | D20 |

## 3. Requirements traced

| Requirement | What this package delivers | Acceptance |
|---|---|---|
| FR-FLU-001 | `CpDef.port_role` with doc 11 §2.2's roles; the starter fluid FnDefs carry them (D2, D3) | FL-2 |
| FR-FLU-002 | `Fluid` connections with a line kind; FI-01 as EW-INV-024 and refusals; pilot and drain dashed; no electrical net (D2, D11, D12, D17, D18) | FL-3, FL-15 |
| FR-FLU-003 | The coil functions from the part (`fluid.solenoids`, N-FL-1), the declaration rule of D5; the derived address and the cross-references as built | FL-4, FL-5 |
| FR-FLU-004 | Valve children with occupancy (`valve_slots`), `valve.set_occupancy`, the overview glyphs (D8, D9, D19) | FL-6, FL-8, FL-13 |
| FR-FLU-005 | The `PlcAddress` slot through the coil functions; the function-text slot's process-tag fallback (D5, D14) | FL-5, FL-10 |
| FR-FLU-006 (Should) | Golden A's F1 to F16 to the actuators in +FIELD (D20) | FL-11 |
| FR-FLU-017 | EW-FL-002 and EW-FL-004 online (and EW-FL-004 in release, D16) | FL-12 |
| FR-FLU-018 (Should) | EW-FL-001 | FL-9, FL-12 |
| FR-FLU-027 | EW-FL-013 as a prevent rule (FI-02, FI-07) | FL-6, FL-7 |
| FR-FLU-028 (Should) | `CpOverride.closure` and `fluid.port.close`; closed ports exempt from EW-FL-001 (D2, D10) | FL-9 |
| FR-FLU-029 (Should) | N-FL-4 `drives_link`, the tracked target, EW-FL-011 (D13, D15, D16) | FL-10 |
| FR-CHK-005 (EW-FL part) | Pack EW-FL with its MVP subset EW-FL-001, -002, -004, -011, -013 (D16) | FL-12 |
| AC-32 (MVP part) | 28 valve children, overviews 201 and 202 with 4 valves on -202U2, 16 lines F1 to F16 to 8 field devices, derived address texts | FL-13 |

## 4. Decisions

Each is a lead ruling that the owner may reverse.

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **The size.** About 3.7 pw by the steps of §6 against the PERT of 3.1 pw; the CSV is unchanged until §13 O1 is ruled. **De-scope rule:** when the lane is projected more than 20 % over 3.7 pw, these leave as follow-ups without a further ruling, in this order: the property-grid line-kind row (D18; the kind stays settable by script), the Blank and Empty glyphs (D19; Golden A has only valves), the Wire tool on fluid pages (D18) | The MVP row of doc 11 §12 plus FT-263's remainder and FT-744 are more than the entry's scope line names |
| D2 | **The format, typed within the unreleased 0.1** (ADR-0011 amendment 1 and amendment 4; doc 03 §15, doc 07 §2.4 and §2.5, `connect.rs:66` name every one): (a) `FluidPortRole` = `supply1, working2, working4, exhaust3, exhaust5, pilot10, pilot12, pilot14, pilot_exhaust82, pilot_exhaust84, external_pilot, consumer, sensing, generic`, `Unknown`; `CpDef.port_role?` appended after `physical`; (b) `ConnectionKind::Fluid(FluidLine)`, `FluidLine {line_kind, extra}`, `LineKind` = `working` (default, omitted), `pilot, exhaust, drain, flexible`, `Unknown`, written `{"fluid": {}}` or `{"fluid": {"line_kind": "pilot"}}`; (c) `CpOverride.closure?` appended after `unused`: `PortClosure {kind, part?, extra}`, `ClosureKind` = `plug, silencer, blanked`, `Unknown`, `part` a `PartRef` (optional: the plug's part enters the BOM at 1.0); (d) `PartSections.fluid?` appended after `bus`: `FluidSection {solenoids: Vec<Ident>, extra}` (default-empty list omitted), doc 11 §2.6's other members staying in its `extra`; (e) `TextSlot.fallback?` appended after `style`: `SlotFallback` = `process_tag`, `Unknown`. No format bump, no `$v` bump; `schemas/0.1` re-blessed with additions only | Every member has a reader in this package and is optional with a catch-all, so a file without them is unchanged byte for byte, and an older 0.1 build keeps them in `extra`. `closure.part` is a new link to a part copy (EW-INV-021's keep-alive); an older 0.1 build does not enforce it, which amendment 4 accepts for an unreleased format. No migration (owner, 2026-10-31: the app is not in use) |
| D3 | **Port roles in the starter FnDefs.** `ew.fn.valve-5-2-sol-spring` at its next version (the class doc 07 §4.5 gives: a CP disappears) carries ports 1 to 5 with `supply1, working2, exhaust3, working4, exhaust5` and loses CP 5 ("14"); `ew.fn.cylinder-double-acting` gives A and B `consumer` at its next version; `ew.fn.process-valve-2way` stays as it is (its ports are process ports, doc 11 §9.1, and no MVP rule reads them). `FunctionCps` gives each CP its role from the FnDef CP, template-origin CPs included (the rule of `cps.rs:60-82` for discipline). New starter FnDef `ew.fn.valve-solenoid` (CC-BY-SA-4.0, own drawing): contact kind `coil`, the category of `ew.fn.valve-coil`, one CP `{idx 0, designation "14", physical: false}` | The roles are data that the rules read (FR-FLU-001); the solenoid leaves the valve body for its own coil function (FR-FLU-003, FI-03). Published versions are immutable: the lane checks `git show main:<path>` before each bump |
| D4 | **A valve part declares its solenoids** in `fluid.solenoids` and carries one `function_template` entry on a FnDef of contact kind `coil` per solenoid, the k-th such entry being solenoid k. An internal coil (valve-terminal valve) is on `ew.fn.valve-solenoid`; a stand-alone valve's physical coil is on its own coil FnDef. Golden A's 571334 gains `fluid.solenoids = ["14"]` and the template `[the 5/2 valve (main), ew.fn.valve-solenoid]`; `EW-GEN-VLV-1` at its next version gains `fluid.solenoids = ["14"]`, its valve entry names pattern CPs 2 to 6 and pattern CP 7 ("14") goes, its entry order (coil main) staying as it is; `ew-starter-parts` and the starter library move to their next versions | The coil functions come from the part, so placement, `part.assign`, `valve.add`, paste and macros agree; the coil FnDef copy arrives in the part's embedding closure, because `ew-commands` cannot read a library. EW-GEN-VLV-1's order stays so that WP-S2.18's placement of it is unchanged |
| D5 | **The declaration rule replaces ID-469's.** A valve declares solenoid s exactly when s is in `fluid.solenoids` of its valve part (the first part use whose copy has a fluid section with solenoids); its coil CP is the first non-physical CP with the effective designation s (`FunctionCps`, overrides applied) among the device's functions on a FnDef of contact kind `coil`, in function-ID order. `ew_derive::coil_cp_idx` becomes a device-level `solenoid_coil(device, s) -> Option<(function, idx)>`, read by `SolenoidPositions`, N2 and the `PlcAddress` slot. A physical coil gives no coil CP: a stand-alone valve's channel is wired and found by target tracking at 1.0 (FR-PLC-018) | One rule for the derivation, the normaliser and the rules; the FnDef CP rule goes with the interim parameter it served |
| D6 | **N-FL-1, normaliser `valve_functions`** (doc 06 row 1), first in `ORDER`. For a device of the transaction that is new or whose `parts` changed and that has a valve part (D5): (a) without a function on a FnDef of contact kind `valve`, the part's valve template entry gives one; (b) each template coil entry without its function gives one, as part-first placement does (ID `v5(device, template_index)`, ADR-0004 amendment 2; role from the entry; `template_entry` its index); an internal coil whose solenoid differs from its FnDef CP's designation gets a designation override on CP 0; (c) a coil function of the device that is no entry of the current valve part's template (the old part's, after a part change) is removed, a channel bound to it is unbound, notice T20; (d) the valve function is never replaced or removed: a new part with another valve FnDef gives notice T21; (e) a part without a fluid section changes nothing. Idempotent; notices name records in words | Doc 11 §10's edge case 2 (571334 changed to a double-solenoid part) works by data; replacing a drawn valve function would delete its representation and lines, which a normaliser must not do silently (FP-8) |
| D7 | **`valve.add`** with a part whose template has a valve entry ignores `fndef` (N-FL-1 makes the functions); `fndef` stays for a part without a template (ID-394's removal at the command's next schema version is unchanged). The slot-count bound is EW-FL-013's prevent rule (D16). Its refusals that name record IDs are FP-6 | The smallest change that makes the command follow the part |
| D8 | **Occupancy, `valve_slots(module)`** (doc 11 §5.6), a memo query in `ew-derive`. A module is a device whose part (`plc_module.part_use`, else the first part use with `plc.positions`) has a solenoid-position map; its count n is `plc.positions.count`, the one stored position count (doc 11 §2.6's `fluid.positions` is not typed at the MVP, §13 O5). For p in 1 … n: `Valve {device, state}` (a child on a function of contact kind `valve` at position p; of two, the lower ID), `Blank {device}` (a child of kind `blanking_plate`), `Empty`. Children at a position outside 1 … n and second children at a position are listed apart for EW-FL-013. A valve terminal is the parent of such modules (no `ValveTerminal` aspect) | Every reader (EW-FL-004, EW-FL-013, the glyphs, `valve.set_occupancy`) takes one value; the PLC section already holds the count for every MVP module |
| D9 | **`valve.set_occupancy {module, position, occupancy, copies?}`** (doc 11 §4.3; Interactive, one undo step). `occupancy` is `{"valve": {"state": "used" \| "spare" \| "reserved" \| "not_fitted", "part"?: <copy ID>}}`, `{"blank": {"part"?: <copy ID>}}` or `"empty"`; `copies` embeds documents as `part.assign` does. *valve:* a held valve takes the state (a `part` other than its own refused, T8); a blank is removed and, as on an empty position, a valve is made as `valve.add` makes it (`part` required, T7). *blank:* a valve at the position is removed with its delete plan (functions, representations, the connections at its CPs, its `Drives` links; bindings to its coils unbound), notices T24; then a child `{kind: blanking_plate, tag: child scheme, letter "Q", counter = position, parts: [part]?}` with the part's template functions when a part is given, none otherwise. *empty:* the child is removed as above. Refused: a module without positions (T6), a position outside 1 … n (T5) | One previewable command per gesture (ADR-0008); the plate keeps the position's child tag, so the reports and overview name a position the same whether it holds a valve or a plate; a plate without a part is a device without a part, which EW-CORE-041 reports honestly |
| D10 | **`fluid.port.close {function, idx, closure?}`** (Interactive, one undo step): sets `CpOverride.closure` (`kind`, `part?`), or clears it without `closure`. Refused: a CP that is not a fluid port (T9), a fluid port with a fluid line (T10), a part copy not held | FR-FLU-028; a plug on a connected port contradicts the drawing |
| D11 | **FI-01 is structural: EW-INV-024** in `ew-model`'s catalogue (ADR-0009 decision 2's class; its amendment at the design pass, §13 O4): "a `fluid` connection ends at fluid CPs of one medium family (pneumatic and vacuum together, hydraulic alone) or at junctions whose every connection is `fluid`; no other connection except `overview` ends at a fluid CP" (`Unknown` kinds are exempt). `connection.draw` and the Wire tool refuse first with T1 to T3. FI-06 is a refusal of `device.link` for `drives` (T4), not an invariant (FP-7) | Every path (paste, scripts, imports) is guarded, and the user reads a sentence instead of an invariant ID |
| D12 | **The fluid partition `fluid_nets()`** (doc 11 §5.1 without galleries, IPs or `Short` links), a memo query: union-find over fluid CPs, `fluid` connections and their junctions; a net's ID is its minimum member CP ID. `joins_net` stays a whitelist without `fluid`, and every other match over `ConnectionKind` (wire lists, numbering, cables, conductors, terminals, the editing names) leaves `fluid` out of wires (FR-PLC-006's rule: a fluid line is not a wire) | The partition feeds N-FL-4, the tracked target and EW-FL-001 and -011; the electrical outputs stay byte-equal |
| D13 | **The tracked target** (doc 11 §3.3), cold queries: `reached_device(valve)`, the single device other than the valve and its parent chain with a CP in a fluid net of the valve's working ports (`working2`, `working4`), else none; `tracked_target(valve)`: the `Drives` target (`driven_device`), else `reached_device`, else none; `tracked_process_tag(valve)`: the target's process tag, else the valve's own | One definition for the slot, the I/O list's columns as built and EW-FL-011 |
| D14 | **The function-text slot's fallback.** A slot with `fallback: process_tag` prints the device's function text as today and, when that is empty, `tracked_process_tag`. The symbol `ew.iso.valve-5-2-sol-spring` (both editions) at its next version sets it on its `function_text` slot and names the FnDef's new version. Reports keep function text and process tag in their own columns | Doc 11 §3.1 and Open question 5: one value, one place; an opt-in per symbol, so no other symbol's text changes |
| D15 | **N-FL-4, normaliser `drives_link`** (doc 06 row 16), after `bus_ports`. A `fluid` connection new in the transaction whose fluid net then holds a working port of a valve function and a `consumer` port of exactly one other device (not in the valve's parent chain) adds `DeviceLink {from: valve, to: that device, kind: drives}` with ID `v5(valve, "drives" ␟ device)`, unless the valve has a `drives` link; notice T22. Two or more such devices: no link, notice T23 | FR-FLU-029 by data, deterministic across branches; an existing link is the user's |
| D16 | **The five rules**, each with its catalogue row, rule page and Fluent messages (en-GB, nl-NL; WP-S1.16's pattern), pack EW-FL registered with the MVP content, on in every shipped scheme (`drafting`, `pre-delivery`, `migration`, `full`), no supersession. **EW-FL-001** `120001`, completeness, device, o r, warning: per placed valve function of a valve not in state `not_fitted`, its `working2` and `working4` ports whose fluid net has no other member and that carry no closure, one finding listing them (T14). **EW-FL-002** `120002`, conflict, device, o r: per valve that declares solenoids and has internal coils, a coil CP bound by more than one channel or a channel bound to a coil the part does not declare is an error (T15); a declared solenoid without a bound internal coil on a valve in state `used` a warning (T16); spare, reserved and not fitted give none; physical coils are skipped. **EW-FL-004** `120004`, hygiene, device, **o r**, info: one finding per `Valve {state: spare}` and per `Blank` position (T17). **EW-FL-011** `120011`, conflict, device, o, warning: a valve with a `drives` link whose target differs from a `reached_device` that exists (T18). **EW-FL-013** `120013`, conflict, device, p o, error (empty: warning): per module, a second child at a position and a child outside 1 … n (errors, T13, T12), each `Empty` position (warning, T19), and second module children at one position under one parent (FI-07, error, T13); prevent: the two error cases of FI-02 and FI-07 refuse a strict transaction with T12 or T13 and the proposal of §5 | Doc 15 §4.5.3 and doc 11 §6 as catalogued, with one change: EW-FL-004 runs in release too, because AC-21's release run counts EF-14's 20 infos (doc 20 §18.1) and a rule of modes `o` alone never runs there (`scheme.rs:39-47`); §13 O2 |
| D17 | **Line styles.** The scene draws a `fluid` connection in the page's wire stroke, solid for `working`, `exhaust` and `flexible` (the arc of doc 11 §2.4 is FP-4), with doc 07 §2.3's `dashed` pattern for `pilot` and `drain`; the hover text and the property grid name the kind (NFR-A11Y-03). The patterns stay `verified = false` (NFR-COMP-05) | FR-FLU-002's acceptance; one pattern for pilot and drain as doc 11 Open question 14 recommends |
| D18 | **Drawing in the window** (FT-434's fluid half): on a fluid page `wire_kind` gives `fluid` and the Wire tool draws `{"fluid": {}}` lines; autoconnect stays off there. A selected fluid line shows the property-grid row "Line kind" (§5), set by `fields.set` path `line_kind` (refused on another connection, T11), one undo step | The pilot draws pneumatics in the project; the picker and autoconnect are FP-3 |
| D19 | **The overview's valve glyphs** (FT-263's remainder: doc 11 §5.5 step 3; steps 1, 2 and 4 are built since cycle 15). For each overview representation of a module with positions the scene derives one glyph per position from `valve_slots`: `Valve`, a rectangle with a circle and the valve part's part number; `Blank`, the rectangle with both diagonals; `Empty`, the rectangle dashed. Rules: glyph p is the p-th from the left, its width the module glyph's width ÷ n, its height twice that, the circle's radius a quarter of the width (24 mm ÷ 4 gives `ew.ovw.valve-top`'s 6 × 12 mm); the band stands above the module glyph; no glyph or glyph text overlaps a text of the module glyph or of a neighbour, for which the module symbol's text slots may move at a new version of both editions; the glyphs are derived scene items, never records, each with `v5(rep, child)` as its entity reference (an empty position: `v5(rep, "position" ␟ p)`) | D-10 cannot be drawn: a module shows exactly n glyphs. Built-in geometry needs no symbol copy in the project, which the scene could not embed |
| D20 | **Golden A, two golden changes**, both through `tables_from_doc20.py` from doc 20 and its bless, with AC-15's path (b) following: **GA-1** (step 5): 571334's copy with `fluid.solenoids` and its template (D4), 28 coil functions, the 28 bindings on the coil CPs, the FnDef and symbol copies at their new versions (D3, D14); **GA-2** (step 7): the 16 lines F1 to F16 of doc 20 §12 (`fluid`, `working`, on folio 203, from ports 2 and 4 of -201U3-Q1 … -201U4-Q4 to A and B of the actuators of -203Q1 … -203Q8; FT-744), the glyphs (D19), EF-13 and EF-14 no longer pending and AC-21 on the whole CSV (FT-589's last rows). A route that doc 20 gives no cell for is a fixture choice registered as the next ▲U row of doc 20 §23.1 | AC-32 asks for the lines; doc 20 is the only source of the tables (their README) |

No decision changes an ADR; D11 adds an invariant that ADR-0009's catalogue records (§13 O4).

## 5. The texts

Every text is asserted whole (en-GB). `<…>` are filled as `words` names records: a device by its tag, a CP as
`-201U3-Q1:2`, a channel as "channel 3 of -201U3", a connection as the line's words. The nl-NL texts are the lane's
translation.

| # | Where | Text |
|---|---|---|
| T1 | `connection.draw`, a fluid line to a non-fluid end | `a fluid line joins fluid ports only; <cp> is not a fluid port` |
| T2 | another kind to a fluid port | `<cp> is a fluid port; only a fluid line joins it` |
| T3 | a hydraulic port with a pneumatic or vacuum port | `a fluid line does not join a hydraulic port to a pneumatic or vacuum port: <cp a>, <cp b>` |
| T4 | `device.link`, `drives` to the valve's parent chain | `<from> cannot drive <to>, which holds it` |
| T5 | `valve.set_occupancy`, position | `<module> has <n> valve positions; position <p> does not exist` |
| T6 | `valve.set_occupancy`, not a module | `<device> has no valve positions` |
| T7 | `valve.set_occupancy`, valve without part | `position <p> of <module> holds no valve; name the valve part` |
| T8 | `valve.set_occupancy`, another part | `position <p> of <module> holds <valve>; assign the new part to <valve> instead` |
| T9 | `fluid.port.close`, not fluid | `<cp> is not a fluid port` |
| T10 | `fluid.port.close`, connected | `<cp> has a fluid line; delete the line before closing the port` |
| T11 | `fields.set line_kind` | `<connection> is not a fluid line` |
| T12 | EW-FL-013, out of range (finding; the refusal appends `; free positions: <list>` or `; every position is taken`) | `<module>: position <p> is outside its <n> valve positions` |
| T13 | EW-FL-013, duplicate | `<parent>: position <p> holds both <a> and <b>` |
| T14 | EW-FL-001 | one port: `<valve>: working port <port> is open; connect, plug or blank it`; two: `<valve>: working ports <port> and <port> are open; connect, plug or blank them` |
| T15 | EW-FL-002, error | `<valve>: solenoid <s> is bound to <k> channels: <channel>, <channel>`; undeclared: `<valve>: <channel> drives a coil that the part does not declare` |
| T16 | EW-FL-002, warning | `<valve>: solenoid <s> has no channel` |
| T17 | EW-FL-004 | `<module>: position <p> holds a spare valve, <valve>`; `<module>: position <p> holds a blanking plate, <plate>` |
| T18 | EW-FL-011 | `<valve> drives <target>, but its working-port lines reach <device>` |
| T19 | EW-FL-013, empty | `<module>: position <p> is empty; fit a valve or a blanking plate` |
| T20 | N-FL-1 notice | `solenoid <s> of <valve> went with the old part; <channel> is unbound` |
| T21 | N-FL-1 notice | `<valve> keeps its drawn valve function; the new part has another valve type: replace the symbol to change it` |
| T22 | N-FL-4 notice | `<valve> now drives <device>` |
| T23 | N-FL-4 notice | `the working-port lines of <valve> reach <device> and <device>; link the driven device by hand` |
| T24 | `valve.set_occupancy` notices | `removed <valve> from position <p> of <module>`; with lines: `removed <valve> from position <p> of <module> with <k> fluid lines`; per binding: `<channel> is unbound: its coil went with <valve>` |
| T25 | Property grid (window) | row `Line kind`; values `Working`, `Pilot`, `Exhaust`, `Drain`, `Flexible` (keys in `strings.rs`) |

## 6. Build order

One lane builds all seven steps in this order and commits at the end of each. Steps 1 to 4 change no Golden A byte.

1. **The format** (D2): the members of `ew-schema`, their round-trip and unknown-value tests, `schemas/0.1` re-blessed;
   `FunctionCps` with `port_role` (D3's derivation half).
2. **Fluid lines and the partition** (D11, D12, D17): EW-INV-024 with its unit tests, T1 to T3 in `connection.draw`,
   `fluid_nets`, every `ConnectionKind` match reviewed (grep `ConnectionKind::`), the dashed pilot and drain.
3. **Commands and the window** (D9, D10, D18): `valve.set_occupancy`, `fluid.port.close`, `fields.set line_kind`, the
   Wire tool on fluid pages and the property-grid row; command schemas regenerated; frames of §7 FL-15.
4. **Links and the fallback mechanism** (D13, D14, D15): `reached_device`, `tracked_target`, `tracked_process_tag`,
   the slot's `fallback`, N-FL-4 with the `ORDER` test, T4 in `device.link`; fixtures only.
5. **The declaration switch and GA-1** (D3 to D7, D20 GA-1): N-FL-1 first in `ORDER`, `solenoid_coil` replacing
   `coil_cp_idx` in `SolenoidPositions`, N2 and the slot, `valve.add`, the content at its new versions (FnDefs, the
   valve symbol in both editions, `EW-GEN-VLV-1`, `ew-starter-parts`, the starter library), Golden A's tables and
   bless, path (b), the `Golden:` trailer. The interim rule and its data go in this one commit, so no commit derives
   Golden A's addresses from neither rule.
6. **The rules** (D8, D16): `valve_slots`, EW-FL-001, -002, -004, -011 and -013 with the prevent half, the catalogue,
   messages, rule pages (`cargo xtask docs-gen`), each on a fixture of its defect.
7. **AC-32 and GA-2** (D19, D20 GA-2): the glyphs, F1 to F16 in Golden A, the AC-21 switch, the AC-32 test and its
   acceptance-runner entry, the `Golden:` trailer; frames of folios 201 to 203 at both sizes.

## 7. Acceptance

- **FL-1** Each new member and enum round-trips through load and save; an unknown `LineKind`, `FluidPortRole`,
  `ClosureKind` and `SlotFallback` value survives verbatim; a project without them is byte-equal after a save;
  `schemas/0.1`'s diff holds additions only.
- **FL-2** (FR-FLU-001) Every one of Golden A's 28 valves has a valve function whose CPs are 1 to 5 with the roles
  `supply1, working2, exhaust3, working4, exhaust5` and a coil function with one non-physical CP "14".
- **FL-3** (FR-FLU-002) A fluid line from -201U3-Q1:2 to an electrical CP is refused with T1; a wire to a fluid port
  with T2; a hydraulic–pneumatic line (fixture) with T3; a pasted fragment with such a line fails EW-INV-024; a pilot
  line's scene item has the dashed pattern and a working line none; a `fluid` connection changes no electrical net.
- **FL-4** (FR-FLU-003) Part-first placement of `EW-GEN-VLV-1` gives its valve and its physical coil and no internal
  coil; `valve.add` of a single-solenoid fixture part on a module gives the valve function and coil "14", which N2
  binds by the map; `part.assign` (replace) of a bistable fixture part adds coil "12" (its override) and N2 binds
  channel 2p + 1 when free; assigning the single-solenoid part again removes coil 12 with T20; each one undo step; a
  second run of the normalisers changes nothing.
- **FL-5** (FR-FLU-003, FR-FLU-005) On Golden A -201U3-Q1 prints Q0.1 from its coil function's channel;
  `plc.set_start` of -201U3 re-renders it in the same commit; folio 201 still lists "203-B4" under -201U3.
- **FL-6** (FR-FLU-004, FR-FLU-027) Golden A has 28 valve children with derived tags, channel states 8 used, 20 spare,
  28 deactivated; `valve_slots(-202U2)` gives four `Valve` positions; `valve.add` on -202U2 is refused in a strict
  project with T12 plus "; every position is taken" and reported by EW-FL-013 in a lenient one.
- **FL-7** EW-FL-013 on fixtures: a duplicate (T13), an out-of-range child (T12), an empty position (T19, warning),
  two modules at one position (T13); the prevent refusals carry §5's proposals.
- **FL-8** `valve.set_occupancy`: spare on a used valve sets the state; blank on a valve removes it with T24 and its
  bindings and makes the plate; empty removes the plate; valve with a part on an empty position makes a valve; T5 to T8
  whole; each one undo step.
- **FL-9** (FR-FLU-028) Plugging -201U5-Q1:2 and :4 leaves 19 EW-FL-001 findings; clearing the closures gives 20; T9,
  T10 whole.
- **FL-10** (FR-FLU-029) On Golden A without -201U3-Q1's `drives` link and F1, F2: drawing F1 adds the link (T22) and
  folio 203 prints "TA05-V-15" at -201U3-Q1; drawing F2 adds no second link; F1 and F2 re-pointed to a test actuator
  -203Q9 give EW-FL-011 (T18); a line reaching two actuators gives T23 and no link.
- **FL-11** (FR-FLU-006) F1 to F16 join ports 2 and 4 of the 8 used valves to A and B of the actuators of
  -203Q1 … -203Q8 in +FIELD; `tracked_target` of each used valve is its field device.
- **FL-12** (FR-FLU-017, FR-FLU-018, FR-CHK-005) `ew check --scheme pre-delivery` on Golden A: EW-FL-001 20 warnings
  (each "working ports 2 and 4"), EW-FL-004 20 infos, none of EW-FL-002, -011, -013; AC-21 compares the whole
  `findings_mvp.csv` with the totals 42 errors, 32 warnings, 31 infos, 1 waived; EW-FL-002 fires on fixtures of excess
  (error) and of an unbound used valve (warning); `cargo xtask docs-gen --check` is clean.
- **FL-13** (AC-32, MVP part) An acceptance test and runner entry: 28 valve children; folio 201 draws 24 valve glyphs
  printing "571334" and folio 202 four on -202U2; the break marks and "∞" lists as built; 16 lines F1 to F16 to 8 field
  devices; the 28 Q addresses of folios 203 to 206.
- **FL-14** Golden A's exported bytes differ exactly as §8 lists; Golden A's wire list and wire numbers are unchanged;
  AC-15's path (b) equals path (a); two runs give equal bytes.
- **FL-15** (window) On a fluid page of a new project the Wire tool draws a working line between two valve ports; the
  Line kind row sets `pilot`, the line turns dashed, undo restores it; frames at 1280 × 800 and 1920 × 1080; folios 201
  and 203 at both sizes after step 7.
- **FL-16** The hand-back checks of the lane's package set pass (clippy, fmt, doc, nextest, settle, policy, api-guard:
  the two new commands are additions).

## 8. Golden A's expected changes

| Change | GA-1 (step 5) | GA-2 (step 7) |
|---|---|---|
| `golden/J-E-2416/tables/` | `functions.csv` +28 coil rows; `channels.csv` 28 bindings on the coil CPs; `part_templates.csv` +2 rows (571334); a fluid-section table or column for 571334 | `connections.csv` +16 `fluid` rows (F1 to F16) |
| `golden/J-E-2416.ew/` | device shards of the 28 valves (coil functions); module shards (channel bindings); `lib/` copies: 571334, the 5/2 FnDef, the cylinder FnDef, `ew.fn.valve-solenoid` (new), `ew.iso.valve-5-2-sol-spring@qet10` | the page file of folio 203 (16 connections) |
| Printed outputs (PDF, SVG, PNG, text) | folio 203: "TA05-V-15", "TA05-V-33", "TA05-V-18", "TA05-V-17", "TA05-V-36", "TA05-V-38", "TA05-V-16", "TA05-V-37" in the function-text slots of the 8 used valves | folio 203: the 16 lines; folios 201 (24 glyphs, "571334" each) and 202 (4 glyphs); the module symbol's moved texts if D19 moves them |
| Findings | none | `findings_mvp.csv`: EF-13 and EF-14 lose `pending`; counts unchanged |
| Unchanged | every report row (BOM, wire list, terminal and cable lists, the PLC overview and the I/O list), folios other than 201 to 206, Golden C | the same |

Any other changed byte stops the lane (§9.3 item 2). The re-bless list goes into each commit message.

## 9. Neighbours, files owned and what a lane may not decide

### 9.1 Neighbours

| Neighbour | It owns | This package |
|---|---|---|
| WP-S1.13 | FT-273 (push insert, `assembly.detach`), the assembly aspect, break marks and "∞" lists | Takes N-FL-1, `valve.set_occupancy` and FT-263's glyphs from its status line (02:1128) |
| WP-S1.21 | N2, N3, `ChannelAddresses`, the `PlcAddress` slot, EW-PLC-012 | Changes N2's coil lookup only (D5) |
| WP-S1.25 | Valve models, media, galleries, tubes, line tags, the 1.0 rules and reports, `ValveTerminal` | Leaves `FluidLine.tube`, `FluidLine.length` and the fluid section's other members in `extra` |
| WP-S2.04 | The Wire tool and FT-434's bus half | Arms the Wire tool on fluid pages (D18) |
| WP-S2.18 | Placing `EW-GEN-VLV-1` from the window | Keeps its entry order (D4) |
| WP-S3.01 | FR-SYM-014's linkage | None |
| WP-S3.12 | The QET pneumatic import | None |

### 9.2 Files owned

`crates/ew-schema/src/{connect,library,device}.rs`; `schemas/0.1/`; `crates/ew-model/src/invariants.rs` (and the
catalogue text it generates); `crates/ew-derive/src/{cps,nets,plc}.rs` and a new fluid module; `crates/ew-commands/src/normalise/{mod,solenoid_binding}.rs`,
new `valve_functions.rs` and `drives_link.rs`, `commands/{assembly,connection,fields}.rs`, a new fluid command module,
`crates/ew-commands/schemas/`; `crates/ew-checks/src/rules/` (a new fluid module), its registry, `catalogue/` and
`messages/`; `crates/ew-api/src/output/view.rs` (the fallback); `crates/ew-scene` (fluid strokes, the glyphs);
`crates/ew-viewmodel` (wiring, the property-grid row, strings); `content/fndefs/`, `content/symbols/` (the valve and,
if D19 needs it, the module symbol, both editions), `content/libraries/ew-starter-parts.ewlib/`,
`crates/ew-library` (the starter list and its tests); `golden/J-E-2416/tools/tables_from_doc20.py`,
`golden/J-E-2416/tables/`, `golden/J-E-2416.ew/` (re-bless only; never opened for writing by hand),
`golden/J-E-2416/expected/`; `crates/ew-testkit/src/golden/`; `crates/ew-api/tests/expected/findings_mvp.csv`;
doc 20 §23.1 (one ▲U row, D20). Read-only: `deny.toml`, `supply-chain/`, `Cargo.lock`, every other document.

### 9.3 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a format member, variant or link beyond D2 seems needed, or a member must change type;
2. a Golden A byte changes beyond §8, or AC-21's totals change;
3. a rule's ID, code, scope, modes or severity would differ from D16, or a text from §5;
4. a published library version would have to change (`git show main:<path>`);
5. a new dependency, or a change to Golden C, the QET importer or another package's files (§9.1), seems needed;
6. a fit number of D19 contradicts the fixture (the glyph pitch against the module glyph's width, a glyph that cannot
   avoid a text): it reports the probe.

The lane decides freely: query and module names, the structure of the normalisers and rules, test fixtures, the
nl-NL wording, the glyph band's offsets within D19's rules, and the property-grid keys.

## 10. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 A new `ConnectionKind` variant reaches a match that treats an unknown kind as a wire (lists, numbering, exports) | Medium · high | Step 2 reviews every match by grep; FL-14 pins the wire list and numbers |
| R2 GA-1 reaches more outputs than §8 lists (`api_v1` function rows, query snapshots, the device navigator) | Medium · medium | A listed snapshot that only counts records is re-blessed and named; a printed byte outside §8 stops (§9.3) |
| R3 The content chain (FnDef → symbol → part → libraries → Golden A's copies) breaks a placement or update test of another package | Medium · medium | Step 5 lands the chain in one commit; WP-S2.18's placement of `EW-GEN-VLV-1` keeps its order (D4) |
| R4 AC-15's path (b) cannot map the coil functions' IDs | Low · medium | Path (b) creates them by `valve.add` (N-FL-1) and maps them as it maps the other template functions |
| R5 Folios 201 and 202 have no room for the glyph band | Low · medium | D19's rules and the stop of §9.3 item 6 |
| R6 Size (D1) | Medium · medium | The de-scope rule of D1; §13 O1 |

## 11. Size, routing and the hand-back point

**Size.** About 3.7 pw: step 1 0.4, step 2 0.5, step 3 0.5, step 4 0.4, step 5 0.8, step 6 0.6, step 7 0.5. The CSV's
PERT of 3.1 pw (O 2 · L 3 · P 4.5) stays until §13 O1 is ruled.

**Routing** (`.claude/orchestration.md` rule 0): one lane, **Opus at `high`** (the file format, a structural
invariant, a normaliser that removes functions, two golden changes), after WP-S1.13's and WP-S1.21's built parts (both
on the working branch).

**Hand-back point** (ID-1282). The lane hands back at the end of **step 5**, the first golden change committed with its
trailer, when it then stands above about 450k tokens; a lane that stands there before it begins step 5 hands back at
the end of step 4 instead. Its hand-off note then holds: the steps done with their commits; the new versions chosen for
each content item and the `git show main:` checks behind them; whether GA-1 landed, with its re-blessed files; the
`ConnectionKind` matches reviewed in step 2 and how each treats `fluid`; the `ORDER` as it stands; the open items of
this document's §7 still red; the test filters (`-E 'test(/fluid/)'`, the AC-21 and AC-32 acceptance tests, the
`golden_dsl` and `command_fuzz` binaries of `ew-testkit`, `ew-ui`'s snapshots); and every choice of §9.3's free list
already made, so the successor does not remake it.

## 12. Follow-ups, proposed

The design pass opens the rows. **FT-263**, **FT-322**, **FT-744** and FT-589's fluid rows close with step 7;
FT-434's fluid half with step 3.
- **FP-1** Starter parts for a generic valve-terminal module and valve, so a new project draws a valve terminal
  without the golden parts.
- **FP-2** The device navigator's context actions (add valve, set spare or blank, close ports; doc 11 §7).
- **FP-3** Autoconnect on fluid ports and a line-kind picker in the Wire tool (doc 11 §7).
- **FP-4** The flexible line's arc (doc 11 §2.4).
- **FP-5** `fluid.positions` for a module without a PLC section (doc 11 §2.6).
- **FP-6** `valve.add`'s refusals name record IDs (`assembly.rs:241-248`); name the module and the holder in words.
- **FP-7** FI-06 as a structural invariant beside D11's `device.link` refusal.
- **FP-8** Replacing a valve function on a part change, previewed (doc 11 §4.2 N-FL-1's "creates or removes the valve
  function").

## 13. Open questions and the documents to update

**Open questions for the orchestrator.**
- **O1** The size: the steps add up to about 3.7 pw against the PERT of 3.1 pw. Options: (1) re-estimate to
  O 2.5 · L 3.5 · P 5.5 (PERT 3.7); (2) keep 3.1 and rely on D1's de-scope rule. Recommendation: (1); the package is
  built whole (ID-1281), so a true size schedules it better than a de-scope does.
- **O2** EW-FL-004's modes: doc 11 §6 and doc 15 §4.5.3 give `o`, while doc 20 §18.1 counts EF-14 in the pre-delivery
  release run (AC-21). Options: (1) `o r` (D16); (2) keep `o` and take EF-14 out of `findings_mvp.csv` (31 infos
  become 11, a doc 20 and AC-21 change). Recommendation: (1).
- **O3** FT-273 (push insert, `assembly.detach`) is due before AC-32 but is WP-S1.13's remainder, and AC-32's MVP
  criterion does not need it. Options: (1) it stays with WP-S1.13, built before G-M2 by that package's lane; (2) it
  joins this package (about 0.3 pw more). Recommendation: (1).
- **O4** D11 makes FI-01 a structural invariant, EW-INV-024. Options: (1) structural, recorded as an ADR-0009
  amendment at the design pass (doc 11 §4.1 classes FI-01 as S); (2) a prevent rule of pack EW-FL. Recommendation:
  (1).
- **O5** D8 reads a module's position count from `plc.positions.count` instead of doc 11 §2.6's `fluid.positions`.
  Options: (1) as D8, with FP-5 for modules without a PLC section; (2) type `fluid.positions` now (a member, Golden A's
  533361 copy changes). Recommendation: (1).

**Documents to update** (the design pass after the lane): doc 11 §2.1 and §2.7 (the closure's part optional), §2.6
(`solenoids` with the template's coil entries; the position count from the PLC section), §3.1 (D5's rule as built;
ID-469's rule gone), §4.1 (FI-01 as EW-INV-024; FI-06 a `device.link` refusal), §4.2 (N-FL-1 and N-FL-4 as built),
§4.3 (`valve.set_occupancy` and `fluid.port.close` as built), §5.5 (the glyphs), §6 and §11.2 (EW-FL-004 `o r`), §12;
doc 03 §15 (the closure); doc 06 §3.2 (rows 1 and 16 built, `ORDER`); doc 07 §2.3 (`TextSlot.fallback`), §2.5 (the
fluid section), §9 (the content versions); doc 15 §4.5.3 (EW-FL-004's modes); doc 20 §6.2 and §12 (F1 to F16 drawn),
§18.1 (EF-13 and EF-14 no longer pending); ADR-0009 (an amendment for EW-INV-024); ADR-0011 amendment 4 ("applied
again" for the fluid members); doc 05 §10.3 (the same note); doc 02's WP-S1.13 status line (what this package took).

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.20, WP-S1.13, WP-S1.21, WP-S1.25, WP-S2.04, WP-S2.18, WP-S3.01,
  WP-S3.12; FT-263, FT-273, FT-322, FT-434, FT-589, FT-744.
- [Decisions](../decisions.md): ID-1210, ID-1281, ID-1282, ID-1311; [decision log](../06-decision-log.md): ID-334,
  ID-373, ID-392 to ID-394, ID-466, ID-467, ID-469, ID-878.
- [Fluid power](../../design/11-fluid-power.md); [domain model](../../design/03-domain-model.md) §15;
  [project format](../../design/05-project-format-and-persistence.md) §2.2, §3.1, §10.3;
  [schematic editor](../../design/06-schematic-editor.md) §3.2;
  [symbols, parts and macros](../../design/07-symbols-parts-macros.md) §2.3 to §2.5, §4.5, §9;
  [checks](../../design/15-checks-revisions-collaboration.md) §2.2, §4.5.3;
  [golden project](../../design/20-golden-project.md) §6.2, §12, §18.1, §21, §23.1;
  [requirements](../../design/02b-requirements-extended.md) §3 (FLU).
- [ADR-0004](../../adr/0004-identity-uuidv7-uuidv5.md), [ADR-0009](../../adr/0009-invariant-catalogue.md),
  [ADR-0011](../../adr/0011-format-versioning-and-migration.md), [ADR-0019](../../adr/0019-rule-engine-and-rule-packs.md).
- The specifications of [WP-S1.21](WP-S1.21-plc-and-bus-model.md) and [WP-S1.16](WP-S1.16-mvp-rule-packs.md).
