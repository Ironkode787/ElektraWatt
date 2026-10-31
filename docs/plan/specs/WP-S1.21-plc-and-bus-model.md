# WP-S1.21: PLC and bus model, the remaining scope (closed specification)

**Status:** specified in cycle 36 (documents only), an M2 package, for what remains after the cycle-13, -16 and -18
work, in five parts: part 1 PLC records from the window and the channel members (0.9 pw, D2 to D6), part 2 the derived
PLC texts on the pages (0.6 pw, D7), part 3 addressing commands, the EW-PLC rules and AC-29 (1.1 pw, D8 to D10), part 4
the bus model, the EW-BUS rules and AC-31 (1.2 pw, D11, D12), part 5 the I/O list, card supply groups and the model
follow-ups (0.5 pw, D13); part 1 first; parts 2 and 3 after it in any order or side by side; part 4 after part 1;
part 5 last; WP-S2.18 part 4 (`brief-plc-io`) waits for parts 1 and 2 and WP-S2.07 part 3 · **Parts 1 and 2 built in cycle 37** (lane A37, decision log ID-854; lane G37, ID-860; Golden A unchanged, its folios 203 to 206 wait for FT-698) · **Part 3 built in cycle 38** (lane A38, decision log ID-870; PA-2 de-scoped, FT-725; the prevent half of EW-PLC-001 and -007 not built, FT-726); Golden A prints its Q addresses since FT-698 (lane F38, ID-874) · **Part 4 built in cycle 39** (lane A39, decision log ID-896; BU-1 to BU-6 and BU-8, AC-31 passes; BU-7 waits for a part-level bus port, FT-770; a golden change: folio 3 prints "Node TBD", folio 909's DP row has 4 members; the master JE-N1000-03 is the external partner behind folio 3's interruption point DP, not a device record) · **Part 4b specified in cycle 40** (lane S40, decision log ID-920; documents only): BU-7's part-level bus port, `PartSections.bus` with its port list (D15), normaliser N5 that gives a part's ports to the device's port-less bus-port functions and never rewrites one (D16), and EW-GEN-CPU-1's `DP` port at library 0.2.1 with no golden change (D17); BU-7 is closed by BU-7a to BU-7k of §8a, built by lane I40 (0.35 pw) · **Part 4b built in cycle 40** (lane I40, decision log ID-927; BU-7a to BU-7f and BU-7h to BU-7k met, BU-7g in part: its `rep.join_device` clause is refused by WP-S2.18 D2, FT-841; FT-770 closed; §8a.1's compatibility claim corrected in both directions; no golden change) · **Date:** 2026-10-31 ·
**Work package:** [WP-S1.21](../02-work-packages.md#wp-s121-plc-and-bus-model-with-addressing-mvp) ·
**Decision log:** ID-835 · **Part of:** ElektraWatt plan, specifications

**Purpose.** WP-S1.21 has been built ahead of its package for the golden project since cycle 13: the address crate,
the PLC record kinds, the queries and the PLC overview (cycle 13), the PLC commands and normaliser N1 (cycle 16), and
the part-data binding with N2, N3 and `plc.set_module_part` (cycle 18). What it never had is a specification of the
rest. This document closes that: the PLC chain from the window (a drafter places a CPU and two cards and gets a
station, modules, channels and addresses with no command typed), the channel members as editable fields, the derived
addresses on the pages, the addressing commands and the MVP EW-PLC rules with AC-29, the bus model with the MVP EW-BUS
rules and AC-31, and the remaining MVP rows. It is written against the code at `adb0df1a` (§2.1). Where this document
and a design document differ, this document is the narrower and wins for the lanes; the design documents stay the
target, and the documents of §13 are updated at the design pass after each part. A choice a lane meets that this
document does not settle is escalated, never guessed (`CLAUDE.md`, rule 2).

---

**Contents**
1. [Goal and non-goals](#1-goal-and-non-goals)
2. [Inputs and the code as it is](#2-inputs-and-the-code-as-it-is)
3. [Requirements traced](#3-requirements-traced)
4. [Decisions](#4-decisions)
5. [Part 1: PLC records from the window and the channel members](#5-part-1-plc-records-from-the-window-and-the-channel-members)
6. [Part 2: the derived PLC texts on the pages](#6-part-2-the-derived-plc-texts-on-the-pages)
7. [Part 3: addressing commands, the EW-PLC rules and AC-29](#7-part-3-addressing-commands-the-ew-plc-rules-and-ac-29)
8. [Part 4: the bus model, the EW-BUS rules and AC-31](#8-part-4-the-bus-model-the-ew-bus-rules-and-ac-31);
   8a. [Part 4b: the part-level bus port (BU-7)](#8a-part-4b-the-part-level-bus-port-bu-7)
9. [Part 5: the I/O list, card supply groups and the model follow-ups](#9-part-5-the-io-list-card-supply-groups-and-the-model-follow-ups)
10. [Files owned and what a lane may not decide](#10-files-owned-and-what-a-lane-may-not-decide)
11. [Risks](#11-risks)
12. [Size, order and routing](#12-size-order-and-routing)
13. [Follow-ups and the documents to update](#13-follow-ups-and-the-documents-to-update)
14. [Sources](#sources)

---

## 1. Goal and non-goals

**Goal.** At the end of the package a drafter in a new project places the generic CPU and a digital input and a
digital output card from the window, and the project holds a configuration project `PLC1`, a station with two
positional modules, sixteen channels with derived addresses (I0.0 … I0.7, Q0.0 … Q0.7 under the TIA profile) bound to
the card pins, and those addresses printed at the pins; a channel's symbol, comment, state and address mode are fields
of the property grid; a start or profile change is one command; the MVP EW-PLC and EW-BUS rules report on Golden A as
doc 20 §18.1 says (EF-02, EF-03, EF-16), AC-29 and AC-31 pass, and folio 3 prints the station address "TBD" from a
typed bus port.

**Non-goals** (each with its package):
- No PLC navigator tree, drag-bind or PLC grid (FR-PLC-020, FR-PLC-021): WP-S2.07 part 3 builds them on part 1's
  fields. No `brief-plc-io` (WP-S2.18 part 4, FT-593).
- No target tracking, automatic symbols, freeze, ownership protection, safety data, process-tag scheme, Rockwell
  rendering, PROFINET, IO-Link or EtherCAT structure, and no 1.0 EW-PLC, EW-SAF, EW-PRC, EW-BUS or EW-IOL rule
  (WP-S1.24). No exchange formats (WP-S3.16, WP-S3.17) and so no exchange records in `plc.set_module_part`'s preview
  (FT-324 stays open). No rack/slot editor and no bus navigator (1.0).
- No N4 logical-net inheritance (FR-BUS-024, 1.0); logical nets are set explicitly at MVP.
- No N-FL-1 (FT-322, WP-S1.20) and no fluid rules (WP-S1.20).
- No change to Golden A's exported bytes except part 4's listed golden change (D11).

## 2. Inputs and the code as it is

| Input | What the lanes take from it |
|---|---|
| [PLC and bus](../../design/10-plc-and-bus.md) §2.2 to §2.7, §2.10, §3.1 to §3.4, §3.6, §3.8 to §3.10, §5, §6, §9, §10 | The records, the derived queries, address derivation A1 and A2, states A3, normalisers, representations, segment derivation A5, the commands, the rule catalogue, the row sources, the touchpoints, the golden folios, the MVP column |
| [Schematic editor](../../design/06-schematic-editor.md) §3.2 | The normaliser registry and its order |
| [Checks](../../design/15-checks-revisions-collaboration.md) §2.3, §3.6, §4.5.1, §4.5.2 | Scopes (`cfg`, `lnet`, `pnet`), supersession, the pack rows |
| [Golden project](../../design/20-golden-project.md) §11, §18.1, §22 | Folios 3, 908, 908.1, 908.2, 909; EF-02, EF-03, EF-16; AC-29, AC-31 |
| [Project format](../../design/05-project-format-and-persistence.md) §2.2 | Kinds and ranks |
| ADR-0004, ADR-0008, ADR-0009, ADR-0011 amendment 4, ADR-0019 | v5 IDs of channels, one command per gesture and preview, invariants, typing within format 0.1 (decision log ID-334), rules and packs |
| Specifications | [WP-S1.16](WP-S1.16-mvp-rule-packs.md) (rule pages, Fluent messages, the AC-21 subset, D10), [WP-S2.07](WP-S2.07-domain-tables-and-part-selection.md) §8, [WP-S2.18](WP-S2.18-basic-elements-from-the-window.md) D5 |
| Follow-ups | FT-224, FT-225, FT-226, FT-227, FT-270, FT-320, FT-321, FT-323, FT-350, FT-352, FT-589 |

### 2.1 What exists at `adb0df1a` (the base contract)

The lanes rely on exactly these items. If one differs, the lane stops and reports it.

| Item | Where | Used here for |
|---|---|---|
| `PlcConfigProject` (name, profile, cpu), the aspects `PlcStation` (config_project, role, order) and `PlcModule` (slot, start, part_use, integrated), `Channel` (module, number, designation, kind, address `AddrSpec`, state, symbol, comment, binding, target), `PlcSection` (role, io, channels, `bit_of`, `pin_of`, positions), `DeviceLink` | `crates/ew-schema/src/plc.rs` | The records every part reads |
| `ew-plcaddr`: addresses, the TIA profile, unit intervals, the map language | `crates/ew-plcaddr/src/` | Parsing and rendering addresses |
| `ModuleOrder`, `ModuleStarts` (A1), `ChannelAddresses` (A2), `ChannelStates` (A3), `ChannelTargets` (direct targets), `ChannelTexts`, `SolenoidPositions`, `PlcRows`, the channel indexes (FT-319) | `crates/ew-derive/src/plc.rs`, `channel_index.rs` | Every derived value |
| `plc.config_project.create`, `plc.bind_channel`, `plc.set_module_part` (Bulk), `assembly.attach`, `device.link` | `crates/ew-commands/src/commands/plc.rs`, `assembly.rs` | The command base |
| Normalisers N1 (`channel_instantiation`, row 3), N2 (`solenoid_binding`, row 13), N3 (`module_pin_binding`, row 14) | `crates/ew-commands/src/normalise/` | Part 1 adds a row before row 3 |
| `plc_overview` (908, 908.1, 908.2) and the PLC navigator's table view over its rows | `crates/ew-reports/src/kinds/plc_overview.rs`; `crates/ew-api/src/output/navigators.rs` | Unchanged; part 5 adds `io_list` |
| `SlotKind::PlcAddress` (declared, not resolved) | `crates/ew-schema/src/library.rs` | Part 2 resolves it (FT-227) |
| The generic cards EW-GEN-DI-1 and EW-GEN-DO-1 (8 channels each, `bit_of: "n"`, `pin_of: "{n+1}"`, role `module`, black boxes) in `ew-starter-parts` 0.1.0 | `content/libraries/ew-starter-parts.ewlib`; `crates/ew-library/tests/starter_parts.rs` | Part 1's oracle; part 1 adds the CPU part |
| `part_confidence(copy, section)` and `Finding.unverified_data` (FT-323's marker half) | `ew-derive`, `ew-checks` (ID-820) | EW-PLC-012's marker |
| EW-CORE-008 `channel` (orphan channels, ID-823) | `crates/ew-checks/src/rules/` | FT-350's finding (D10) |
| The interim single-line DP model (`DP.1`, `DP.2`, the DP potential with circuit class `bus`; ID-336) | Golden A, `ew-derive` | Part 4 replaces it |

### 2.2 Gaps found (why the parts are shaped as they are)

| Gap at `adb0df1a` | Consequence |
|---|---|
| Placing a part with a PLC section gives a device without `plc_module` or `plc_station`; nothing creates a configuration project; a new project has none, so a card placed from the window has no channels and no address | Part 1: normaliser N0 and the configuration project on demand (D2 to D4) |
| `ew-starter-parts` has cards but no station part | Part 1 adds EW-GEN-CPU-1 (D5) |
| The channel members are not settable (FT-226), so WP-S2.07 part 3's grid has nothing to edit | Part 1 (D6) |
| `PlcAddress` slots print nothing; the coils on 203 to 206 print no address (FT-227) | Part 2 (D7) |
| No `plc.set_start` or `plc.set_profile`; `ModuleStarts` re-derives every module of a configuration project on any edit (FT-224) | Part 3 (D8, D9) |
| No EW-PLC or EW-BUS rule is built (FT-192's PLC rows, FT-589) | Parts 3 and 4 (D10, D12) |
| No `BusPort`, `LogicalNet`, `PhysicalNet` or `bus_segments` (FT-225) | Part 4 (D11) |
| No `io_list` row source; card supply groups stay in `PlcSection.extra`; FT-320, FT-321, FT-352 open | Part 5 (D13) |

## 3. Requirements traced

The 33 rows of the package. "Built" names the cycle that delivered the row; the rest are delivered here.

| Requirement | What this package delivers | Part | Acceptance |
|---|---|---|---|
| FR-PLC-001 | Built (cycle 13); stations from placement (N0) | 1 | PW-1 |
| FR-PLC-002 | Built (cycle 13); the configuration project on demand | 1 | PW-2 |
| FR-PLC-003 | Built (cycle 13); modules from placement and attachment to the station | 1 | PW-1, PW-3 |
| FR-PLC-004 | Built (cycle 13); the channel members settable | 1 | PW-5 |
| FR-PLC-005, FR-PLC-006 | Built (cycles 16 and 18); unchanged | — | — |
| FR-PLC-007 | Built (cycle 13); addresses printed from the derived value only | 2 | PT-1 |
| FR-PLC-008 | Built (TIA, cycle 13); the IEC profile and `plc.set_profile` | 3 | PA-3 |
| FR-PLC-011 | Built (cycle 18) | — | AC-29 (PA-6) |
| FR-PLC-012 | Built (A1, A2); the per-module cut-off (FT-224) | 3 | PA-6 |
| FR-PLC-013 | `Manual`, `Excluded`, `Tbd` through `fields.set` and `plc.set_start`; EW-PLC-015 | 1, 3 | PW-5, PA-1, PA-4 |
| FR-PLC-014 | `plc.set_start`; `plc.readdress` and `plc.convert_manual_to_auto` (MVP Should; de-scope candidates, §12) | 3 | PA-1, PA-2 |
| FR-PLC-016 | Symbol and comment settable; EW-PLC-007 | 1, 3 | PW-5, PA-4 |
| FR-PLC-019 | Built (A3); explicit state settable | 1 | PW-5 |
| FR-PLC-022 | The overview (folio 908) built in cycle 13; "every representation of a channel renders the same data": the `PlcAddress` slot and the black-box pin addresses (part 2, cycle 37), Golden A's valve coils by a proposed follow-up | 2 | PT-1, PT-2 |
| FR-PLC-023 | The `io_list` row source | 5 | PX-1 |
| FR-PLC-024 | Typed supply groups in `PlcSection` | 5 | PX-2 |
| FR-PLC-044 | EW-PLC-001, -002, -003, -008 | 3 | PA-4 |
| FR-PLC-045 | EW-PLC-012 with the unverified-data marker | 3 | PA-5, PA-6 |
| FR-PLC-051 | Built (cycle 18, as reworded) | — | — |
| FR-PLC-052 | The PLC overview box (MVP Should; de-scope candidate) | 5 | PX-3 |
| FR-PLC-053 | Built (cycle 18) | — | — |
| FR-PLC-055 | Built; re-asserted for placement | 1 | PW-6 |
| FR-BUS-001 | The `BusPort` aspect; a port from the part's bus section (part 4b, cycle 40) | 4, 4b | BU-1, BU-7a to BU-7k |
| FR-BUS-002 | `LogicalNet` | 4 | BU-1 |
| FR-BUS-003 | `PhysicalNet` and `bus_segments` (A5) | 4 | BU-2 |
| FR-BUS-004 | Connector termination as a property, its rendered text | 4 | BU-3 |
| FR-BUS-005 | The external partner placeholder, excluded from the BOM | 4 | BU-4 |
| FR-BUS-006 | Folio 3 drawn from the bus model ("Node TBD") | 4 | BU-5 |
| FR-BUS-009 | PROFIBUS station address 0–125 (unverified data), the net's rate | 4 | BU-1 |
| FR-BUS-015 | EW-BUS-001 | 4 | BU-6 |
| FR-BUS-016 | EW-BUS-004 | 4 | BU-6 |
| FR-CHK-005 | The EW-PLC and EW-BUS MVP subsets (doc 10 §10) | 3, 4 | PA-4, BU-6 |

## 4. Decisions

Each is a lead ruling that the owner may reverse. The design pass records them under ID-835 (and the as-built
entries after each part).

| # | Decision | Reason and trade-off |
|---|---|---|
| D1 | **Remaining scope only.** The cycle-13, -16 and -18 work is taken as **2.0 pw** of the 6.3 pw PERT (an estimate from those lanes; the cycles did not size them as package parts); the five parts fill the remaining **4.3 pw** with no reserve and the de-scope rule of §12. The CSV is unchanged | The package was built in slices for the golden project; a spec of the rest is what the lanes need, and re-estimating the whole would change nothing the schedule reads |
| D2 | **Normaliser N0 "PLC aspects from the part"**, a new registry row (doc 06 §3.2) that runs before row 3 (N1). A device of the transaction that gained a part use (or whose `part_use` changed) and carries neither `plc_module` nor `plc_station` gets, from the PLC section of that part (the derivation's rule: `plc_module.part_use`, else the first part use with a PLC section): role `module` → `plc_module {slot: positional, start: auto, part_use}`; role `station` → `plc_station {config_project, role: cpu}` and, when the section declares I/O ranges, `plc_module {integrated: true}` on the same device. N0 never removes or rewrites an aspect; removing one stays `fields.set`. It runs for every command through the registry (`device.place`, `device.create`, `part.assign`, `fields.set` on `parts`, `edit.paste`, later `macro.insert`) | One rule for every path, so the window, scripts and macros agree; no aspect is ever silently changed, so Golden A, whose devices carry their aspects, is untouched |
| D3 | **The configuration project on demand.** When N0 makes a station and the project has no configuration project, the same transaction creates one: name `PLC<n>` (the lowest free n), profile the shipped TIA international profile (FR-PLC-008's default), `cpu` the station when its role is `cpu`, ID `v5(station, "plc_config_project")`. With one configuration project the station joins it; with several, the first by (name, ID), with a notice naming it | A drafter never meets a "no configuration project" state; a v5 ID keeps replays and undo exact |
| D4 | **A module placed without a station.** When N0 makes a module on a device with no station ancestor: if the project has **exactly one** station, the module is attached as that station's last child (`parent` = the station, `position` = the highest child position + 1, `assembly.attach`'s semantics, in the same transaction) with the notice "attached to <tag> as module <n>"; with none, the module stays unaddressed with the notice "no PLC station: place a CPU first" (EW-PLC-008 then reports its used channels); with several, it stays unattached with the notice "attach the module to a station". Undo reverts the attachment with the placement | The common small panel has one CPU, and the drafter then gets addresses without knowing the device tree; the explicit `assembly.attach` covers every other case. The trade-off is an implicit parent change, which the notice states |
| D5 | **A generic CPU part.** `ew-starter-parts` gains EW-GEN-CPU-1 (CC0, manufacturer "Generic", a black box on `ew.fn.blackbox` with the supply CPs `L+` and `M`, PLC section role `station`, no I/O ranges, no bus port until part 4 gives it a typed `DP` port), the library at version 0.2.0, generated by the existing test (`EW_BLESS=1`); WP-S2.18's D5 list gains it | The cards need a station; a station part without on-board I/O keeps the oracle's addresses at byte 0 for both cards |
| D6 | **The channel members through `fields.set`** (FT-226, FT-270's remainder): `designation`, `address` (the profile's text, `auto`, `excluded`, `tbd`), `state` (`spare`, `reserved`, `deactivated`, or empty for derived), `symbol`, `comment` (multilingual), `target`; and `plc_config_project.name`, `plc_config_project.profile` (through `plc.set_profile` only, D8), `device_link` members. An address text is parsed by the configuration project's profile; a text that does not parse is refused with the profile's example ("expected an address like Q0.1"). EW-PLC-007 (duplicate symbol) is a check, refusing only in its prevent mode under a strict scheme (ADR-0019). `KindName` gains the three kinds, and affected pages ignore channel rows no page draws (FT-226) | The grid of WP-S2.07 part 3 routes scalar edits to `fields.set` (WP-S2.06 D10); one write path for grid, property grid and scripts |
| D7 | **Derived PLC texts** (FT-227, doc 10 §3.6). The `PlcAddress` slot resolves on a function with a bound CP to its channel's address, on a fluid function whose device has bound coils to the coil addresses in solenoid order 14, 12 joined by " / ", on a module pin to its channel's address; an unresolved slot prints nothing (no placeholder). A black-box module prints each bound pin's channel address beside the pin designation in `data.small`. No address is stored as text (FR-PLC-007) | Every representation renders the same determined value; the black box is how the generic cards are drawn |
| D8 | **Commands.** `plc.set_start {modules, dir, spec}` (Interactive; `auto`, a manual start parsed by the profile, `excluded`, `tbd`) and `plc.set_profile {config_project, profile}` (Interactive; a shipped profile ID only; changes rendering only, FR-PLC-007). `plc.readdress {scope}` and `plc.convert_manual_to_auto {scope}` (Bulk, previewed, MVP Should) are built last in part 3 and are its de-scope candidates | The MVP column of doc 10 §3.9; Should items carry the de-scope risk |
| D9 | **The per-module cut-off** (FT-224): `ModuleStarts` stays one query per configuration project, but `ChannelAddresses(module)` reads only that module's start (a projection with value cut-off), so a start change of one module re-runs only the modules whose start value changed | AC-29's "only its 8 channels" is a memo fact, asserted by recompute counts |
| D10 | **The MVP EW-PLC rules** EW-PLC-001, -002, -003, -007, -008, -012 and -015 (doc 10 §3.10, doc 15 §4.5.1), each with its catalogue row (present), rule page and Fluent message (WP-S1.16's pattern); EW-PLC-012 with discriminators `binding` and `manual`, both quick fixes of doc 10 §3.10 and the unverified-data marker from `part_confidence(copy, "plc")` (FT-323's PLC half); EW-PLC-015 superseding EW-CORE-044 for starts and channel addresses (doc 15 §3.6). **FT-350:** two channels of one module with one number are reported by EW-CORE-008 with discriminator `duplicate_number` (an integrity finding, no new rule ID); the channel whose ID is `v5(module, "channel", number)` wins over a foreign ID, else the lower ID | The MVP subset as catalogued; FT-350's case is a record-integrity defect like the orphan channel EW-CORE-008 already reports |
| D11 | **The bus model, typed within format 0.1** (FT-225; ADR-0011 amendment 4, ID-334): the `BusPort` aspect on a function and the record kinds `logical_net` and `physical_net` in `project.jsonl` with doc 10 §2.7's members; `NetAddress` types the PROFIBUS variant only (others stay `Unknown` until WP-S1.24); `AddrValue` is `value`, `tbd` or `not_applicable`. `bus_segments` implements A5; the DP potential reads segments instead of the interim single-line model. **Golden A changes (a golden change the lead decides under the owner's standing delegation of 2026-10-27, re-blessed and recorded):** typed records for the DP logical and physical net, the `-201U1:DP` port (slave, address `tbd`), the `-3X1` connector port (passive, termination on) and the external partner JE-N1000-03; folio 3 prints "Node TBD" and folio 909's DP row lists 4 members, as doc 20 §11 already states; every other exported byte unchanged, proven by the byte-equal tests | The interim model was always a stopgap (ID-336); typing now, before 0.1's release, costs no migration |
| D12 | **The MVP EW-BUS rules** EW-BUS-001 (superseding EW-CORE-044 for bus addresses) and EW-BUS-004 (an external end gives one info). FT-589's EW-PLC-012, EW-BUS-001 and EW-BUS-004 rows join the AC-21 subset of WP-S1.16 D10 | AC-31 and EF-03, EF-16 |
| D13 | **Part 5:** `io_list` as a report kind with FR-PLC-023's columns (one row per channel in module order); `PlcSection.supply_groups` typed (`{cps, feeds, kind: load | electronics}`, FR-PLC-024), read by the potential rules as typed roles; FT-320 (a valve leaving its module unbinds its coil with a notice), FT-321 (N3 re-pins after a part change), FT-352 (N2 through the parent index); the PLC overview box (FR-PLC-052, Should) last, as a placed box of kind `plc_overview` on an overview page, and its de-scope candidate | The remaining MVP rows and the follow-ups due before G-M2 |
| D14 | **Parts 2 to 5 in the detail they need now.** Part 1 is closed to the field; later parts are closed to their items, decisions and acceptance; each part is re-read against the delivered earlier parts at dispatch without reopening D1 to D13 | Part 1 dispatches first (cycle 36); WP-S2.18 part 4 waits for it |
| D15 | **The part-level bus port** (cycle 40, FT-770; §8a.1): `PartSections` gains `bus: Option<BusSection>` after `mounting_plate`; `BusSection {ports: Vec<PartBusPort>, extra}`; `PartBusPort {interface, port_label?, bus: BusSystem, role: PortRole, termination?: OnOff, extra}`, required `interface`, `bus` and `role`, the enums of D11 with their `Unknown` catch-alls. No address, net, connector, signal map or address range is typed: the address and the nets belong to the drawn port, and the 1.0 part facts stay in `extra` (FT-829). Additive within format 0.1 (ADR-0011 amendment 4, ID-334): doc 10 §2.6 names the section, and it adds no cascade semantics | The smallest shape whose every typed member has a reader (N5, D16); the function's `BusPort` cannot be reused in a library part because it requires an address and names project nets |
| D16 | **Part → function: N5 "bus ports from the part"** (§8a.2), a normaliser (`bus_ports`, after `module_pin_binding`). A device whose bus part changed (or that is new), or a bus-port function new to a device with a bus part, gets the part's ports that no function of the device carries yet, given in list order to the device's port-less functions whose FnDef has contact kind `bus_port`, by ID: `{interface, port_label, bus, role, termination}` from the part, address `not_applicable` for a passive port and `tbd` otherwise, no nets. N5 never rewrites or removes a port; a carried port that differs from the part's gives a notice. No EW-BUS rule reads the part port at the MVP; the check of a disagreement is 1.0 (FT-829) | One rule for every path, as N0's (D2): placement, `part.assign`, `fields.set parts`, `rep.join_device`, paste and macros agree. A port is engineered data (address, nets, termination), so the part seeds it once and never overwrites it. A part port without a bus-port function seeds nothing and says nothing, so placing a CPU adds no open item until the drafter draws its port |
| D17 | **The data** (§8a.3): EW-GEN-CPU-1 gains `bus.ports = [{interface: DP, bus: profibus_dp, role: master}]` and nothing else (part version 0.1.1, `ew-starter-parts` 0.2.1; 0.2.0 is published on `main` and immutable). No other starter part has a bus interface. No Golden A part gains a bus section, so no golden change | BU-7 met by data the window can use (place the DP port symbol on the CPU and it is typed); Golden A's DP ports are already typed on its functions (D11), and its parts' signal maps are unverified |

No decision changes an ADR.

## 5. Part 1: PLC records from the window and the channel members

**Scope.** D2 to D6, FT-226, FT-270's remainder. **0.9 pw.**

**Build list.**
1. `ew-commands/src/normalise/plc_aspects.rs`: N0 as D2 to D4, a new `ORDER` row before row 3, with the order test
   updated; notices through the existing merge.
2. The configuration project creation of D3 inside N0 (not a new command; `plc.config_project.create` stays the
   explicit form).
3. `fields.set` paths of D6 in `ew-commands/src/commands/fields.rs` and the field catalogue rows (`SETTABLE`) for the
   channel kind, so the property grid shows a selected channel's rows (selection of a channel is WP-S2.07 part 3's;
   here a test selects it through `select.set`).
4. EW-GEN-CPU-1 in `crates/ew-library/tests/starter_parts.rs` and the regenerated `ew-starter-parts.ewlib` (D5).
5. The builder DSL (`ew-testkit`) gains no verb; path (b) keeps its explicit PLC commands.

**Acceptance.**
- **PW-1** In a new project, placing EW-GEN-CPU-1, then EW-GEN-DI-1, then EW-GEN-DO-1 from the picker (three
  placements through `Session`) gives: one station (role `cpu`), two modules attached to it at positions 1 and 2,
  sixteen channels with v5 IDs, each bound to the card pin `n+1` by N3, and `ChannelAddresses` I0.0 … I0.7 and
  Q0.0 … Q0.7.
- **PW-2** The first placement creates `PLC1` with the TIA profile and `cpu` = the CPU device; a second CPU joins
  `PLC1` with the notice of D3.
- **PW-3** A card placed in a project without a station gets no parent, its channels exist, its addresses are absent,
  and the notice reads "no PLC station: place a CPU first"; with two stations it stays unattached with D4's notice.
- **PW-4** Each placement is one undo step; undo of the first removes the station, `PLC1` and nothing else; redo
  restores identical IDs.
- **PW-5** `fields.set` on a channel: `symbol "B1_Start"`, `comment`, `state spare`, `address "I4.0"` (Manual;
  refused for "I4.9" with the profile's example), `address excluded`, `address tbd`, `address auto`; each one undo
  step; the row values of `PlcRows` follow.
- **PW-6** Renumbering the devices after PW-1 changes no address (FR-PLC-055).
- **PW-7** Golden A and AC-15 path (b): every exported byte and the acceptance result unchanged. N0 may fire in path
  (b) (amended in cycle 37, decision log ID-854: path (b) creates the seven I/O modules with their parts and no aspect,
  so N0 gives them their `plc_module` and channels at creation and the script's aspect steps become no-ops; a scripted
  creation should get N0); equal bytes are the criterion, and a byte change still stops the lane (§10.2 item 3).
- **PW-8** Twice-run determinism; `cargo nextest run -p ew-commands -p ew-library -p ew-api`, the matching clippy and
  `cargo fmt --all --check`; `cargo xtask check-deps`.

## 6. Part 2: the derived PLC texts on the pages

**Scope.** D7, FT-227. **0.6 pw.**

- The `PlcAddress` slot resolution in `ew-derive` (a query `plc_address_text(function)` over `ChannelAddresses`,
  `channels_by_cp` and the device's coils) and its use in `ew-scene`'s slot texts.
- Black-box module pins: the address beside the pin designation (D7), in the box renderer.
- `ui.describe` reports the printed address of a hovered pin (no new word unless the vocabulary needs one; a new word
  is counted and listed).

**Acceptance.** **PT-1** after PW-1, the DI card's pins print I0.0 … I0.7 beside 1 … 8 in the page scene; a `fields.set
address "I4.0"` on channel 0 changes the print, undo restores it. **PT-2** (restated in cycle 37: no symbol of `content/`
or Golden A carried a `plc_address` slot, so the original premise was false) the starter coil, valve coil, pushbutton
and selector-switch symbols gain a `plc_address` slot (version 0.2.0); in a new project with the generic CPU and the
output card, a starter coil attached to the card and bound to its channel 0 prints Q0.0 in that slot; a `fields.set
address` of the channel changes the print and undo restores it. **PT-3** a bistable valve prints "Q0.0 / Q0.1" (a
fixture). **PT-4** (restated) Golden A stays byte-for-byte unchanged: its embedded symbols carry no `plc_address` slot
and none of its black boxes has a bound pin. Folios 203 to 206 print their Q addresses once Golden A's embedded lamp,
position-switch and valve symbols gain the slot through a library update (a follow-up proposed in cycle 37, a
golden change). *As built (cycle 38, FT-698, decision log ID-874):* Golden A prints its Q addresses since FT-698: the starter lamp, position switch and 5/2 valve gained the `plc_address` slot at 0.2.0 (`ew-starter` 0.3.0), Golden A's embedded copies followed through the bootstrap re-bless (a golden change, equal to what `update_from_library` gives), and folios 203 to 206 print Q0.1 … Q6.7, the addresses of folio 908's 28 bound channels; folio 101's lamp and position switch are unbound and print nothing. Since FT-707 (ID-876) the black boxes' pins also print their designations, as stored, before the address ("1 I0.0"). **PT-5** targeted checks as PW-8 for `ew-derive`, `ew-scene`, `ew-api`.

## 7. Part 3: addressing commands, the EW-PLC rules and AC-29

**Scope.** D8 to D10, FT-224, FT-323's PLC half, FT-350, FT-589's EW-PLC-012 row. **1.1 pw.**

**Acceptance.** **PA-1** `plc.set_start` on -201U5 output to `Q10` re-addresses its 8 channels and no other, one undo
step. **PA-2** `plc.readdress` previews old → new for a selection and commits as one step; `plc.convert_manual_to_auto`
drops manual values equal to the derived value and lists the others (both de-scope candidates). **PA-3**
`plc.set_profile` to the IEC profile renders `%QX0.1`; the project files differ in that one member. **PA-4** each
MVP EW-PLC rule fires on a fixture of its defect and is silent on Golden A except EW-PLC-012; EW-PLC-015 supersedes
EW-CORE-044 for a `Tbd` start (one finding). **PA-5** EW-PLC-012 carries `unverified_data` for part 533361 (confidence
`test`). **PA-6 (AC-29)** folio 908 equals doc 20 §11.3 (56 channels); EW-PLC-012 gives 28 findings under H1 and 0
under H2 by a part-data edit only; changing -201U5's start recomputes only its 8 channel addresses (memo counts).
**PA-7** two channels with one number give EW-CORE-008 `duplicate_number` once, the v5 one wins. **PA-8** the AC-21
subset grows by EW-PLC-012 (EF-02); the rule pages and messages exist (`cargo xtask docs-gen` clean). **PA-9**
targeted checks for `ew-commands`, `ew-derive`, `ew-checks`.

*As built (cycle 38, lane A38, decision log ID-870; 7e283ab9, b2ef7020, eeca6037).* PA-1 and PA-3 to PA-8 pass; PA-5 on an in-memory edit, because Golden A's part 533361 carries no `test` mark (FT-728). **PA-2 is not met:** `plc.readdress` and `plc.convert_manual_to_auto` left under §12's de-scope rule, because both need A1 over the whole configuration project inside one transaction, which ADR-0008 amendment 4 forbids (FT-725). **D6's refusal is not built:** EW-PLC-001 and EW-PLC-007 report online and in release mode (the catalogue gives them the prevent letter, and `CheckMode::runs` runs prevent-letter rules in release mode); the refusal of the edit under a strict scheme is FT-726 (wording corrected by the cycle-38 review, w2). **D10:** EW-PLC-012 has the rebind quick fix; "open the part's PLC section" is FT-727. As built: the IEC 61131-3 profile prints `%QX0.1`; `ModuleStartOf` gives D9's cut-off and `AddressIndex` lists every address of a configuration project with its overlaps (both memo families; AC-16 at 77 families); the check scopes are `Module` and `Cfg`; a duplicate number is won by the channel whose ID is v5 of its module and number, in the index and in N1; EW-PLC-007 compares symbols exactly; findings name channels as `<full tag>:<designation>`. FT-721's tag forms of `plc.bind_channel` (`<module tag>#<number>` or `<module tag>:<designation>`; `<tag>` or `<tag>:<slot>`) are resolved in `run_script` before the line runs. The cycle-38 review sends the IEC profile's refusal of `%Q0.1` (doc 10 §2.5; FT-746) and EW-PLC-001's pairwise overlap index (FT-747) to the fix round.

*As built (cycle-38 fix round FX38a, decision log ID-884; 72d86b0f, 5ed7bdc1, f2d460e3).* (1) The IEC 61131-3 profile reads a bit address without its size as X: `%Q0.1` parses as `%QX0.1`, and with the prefix optional `Q0.1` parses too; a unit address without its size stays refused (`%Q10`), because doc 10 §2.5's row covers only the bit case (FT-746). (2) `AddressIndex` holds overlap groups, each connected run of overlapping channels in one area once (N channels on one address are one group, not N(N−1)/2 pairs); EW-PLC-001 emits one finding per group, every member a target, the subject the first in address order, and its text names five channels with their addresses, then "and N more channels", in en-GB and nl-NL (FT-747). `plc.bind_channel` checks a CP against the CPs the function draws (`common::function_cp_indices`), so a black-box card's pins bind (FT-750).

## 8. Part 4: the bus model, the EW-BUS rules and AC-31

**Scope.** D11, D12, FT-225, FT-589's EW-BUS rows. **1.2 pw.**

**Acceptance.** **BU-1** the kinds round-trip through load and save, `schemas/0.1` re-blessed additively; an unknown
`NetAddress` variant survives a load and save verbatim. **BU-2** `bus_segments` on Golden A gives one segment, order
external end · -3W1 · -3X1 · -201U1:DP (FR-BUS-003). **BU-3** setting -3X1's termination off changes its rendered
text to "Switch to "OFF"" and gives EW-BUS-004 an error; on gives no error (FR-BUS-004). **BU-4** JE-N1000-03 is the
master in the network list and absent from the BOM (FR-BUS-005). **BU-5** folio 3 prints "Node TBD"; folio 909's DP
row has 4 members; every other exported byte of Golden A equal; the golden change listed. **BU-6 (AC-31)** EW-BUS-004
reports no error with -3X1 on and one info for the external end; EW-BUS-001 reports the `tbd` station address once
(EF-03), superseding EW-CORE-044; the AC-21 subset gains EF-03 and EF-16. **BU-7** the generic CPU's `DP` port typed
(D5): not met in cycle 39 (FT-770); specified in cycle 40 as part 4b, §8a, and closed by its BU-7a to BU-7k. **BU-8** targeted checks for `ew-schema`, `ew-format`, `ew-derive`, `ew-checks`, `ew-import-tables`.

## 8a. Part 4b: the part-level bus port (BU-7)

**Scope.** D15 to D17, FT-770; specified in cycle 40 (lane S40, decision log ID-920), built by lane I40. **0.35 pw**,
the remainder of part 4's BU-7, which part 4's 1.2 pw sized as 0.05 for the CPU part (the CSV is unchanged, D1).
Written against the code at `a3b25fed`.

**Base contract.** I40 relies on exactly these items; if one differs, it stops and reports.

| Item | Where |
|---|---|
| `BusPort`, `BusSystem`, `PortRole`, `AddrValue`, `OnOff` with their `Unknown(Value)` catch-alls (D11); `Function.bus_port` | `crates/ew-schema/src/bus.rs`, `device.rs` |
| `PartSections` with `cable_cores`, `plc`, `enclosure`, `duct`, `rail`, `mounting_plate`, then `extra` (a `BTreeMap`) | `crates/ew-schema/src/library.rs` |
| `ContactKind::BusPort`; the starter FnDefs `ew.fn.dp-port` and `ew.fn.dp-connector` (contact kind `bus_port`) and the starter symbol `ew.bus.dp-port` on `ew.fn.dp-port` | `crates/ew-schema/src/library.rs`; `content/fndefs/`, `content/symbols/` |
| The normaliser registry, `ORDER` ending `solenoid_binding`, `module_pin_binding`, `revision_ensure_open`; `TxBuilder::notice` keeps each text once | `crates/ew-commands/src/normalise/mod.rs`, `tx.rs` |
| `rep.join_device` moves a function to another device; `device.place` with `join_device` adds one | `crates/ew-commands/src/commands/join.rs`, `device.rs` |
| EW-GEN-CPU-1 (`plc = {role: station}`, part version 0.1.0) and `LIBRARY_VERSION = "0.2.0"` in the generator; `main` publishes 0.2.0 | `crates/ew-library/tests/starter_parts.rs`; `git show main:content/libraries/ew-starter-parts.ewlib/library.json` |

### 8a.1 The member (D15)

`crates/ew-schema/src/bus.rs` gains two structs, and `PartSections` gains one member, appended after
`mounting_plate` (doc 07 §2.5's order: a copy's `sha256` is taken over its compact JSON in member order, so the place
is fixed now):

```rust
pub struct PartSections { /* … mounting_plate, */
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<crate::BusSection>,          // sections.bus (doc 10 §2.6)
    /* extra */ }

pub struct BusSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<PartBusPort>,                 // the part's bus interfaces, in the part's order
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,                            // 1.0 device-description facts (rates, ident number), FT-829
}

pub struct PartBusPort {                         // members in this order; the first four as in `BusPort`
    pub interface: Ident,                        // required: "DP", "X1"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_label: Option<Ident>,               // "P1R" (a multi-port interface)
    pub bus: BusSystem,                          // required
    pub role: PortRole,                          // required: the role a port made from it takes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub termination: Option<OnOff>,              // present: the port has a termination switch, in this state
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,                            // connector, signals, address range… (1.0, FT-829)
}
```

- **Names and values.** JSON keys `bus`, `ports`, `interface`, `port_label`, `bus`, `role`, `termination`. `bus` takes
  `profibus_dp`, `profinet`, `ethercat`, `ethernet_ip`, `io_link`, `asi`, `canopen`, `modbus_tcp`; `role` `master`,
  `slave`, `controller`, `device`, `passive`; `termination` `on`, `off`; any other value is kept as `Unknown(Value)`
  and written back verbatim. No member has a unit. `ports` is omitted when empty, so `{"ports": []}` is written `{}`
  (the canonical form; doc 07 §2.5's "not canonical" rule applies).
- **Required.** `interface`, `bus` and `role`, as in the function's `BusPort` (D11); a port without one fails the
  copy's load with the deserialiser's error naming the member, as a PLC section without `role` does.
- **A list.** A part has as many ports as interfaces (a CPU with MPI and DP; a PROFINET device's `X1` `P1R` and `X1`
  `P2R`). The schema enforces no uniqueness; every reader takes the first port of an (`interface`, `port_label`) pair
  and ignores later ones (the part linter's report of the duplicate is FT-351).
- **Not typed** (kept verbatim in the port's or the section's `extra`): the station address and the nets, which belong
  to the drawn port, not to the part; the address range and supported rates (FR-BUS-009 "range from data", FR-PLC-025's
  GSD facts); the connector type (`connector`, EW-BUS-009); the signal map (`signals`, FR-BUS-023). All are 1.0 and
  have no reader before WP-S1.24 (FT-829).
- **Connection points.** A part port is carried by a function of the device whose FnDef has contact kind `bus_port`
  (D16); that function's CPs are the port's connection points. A part names no function or CP, so a part needs no
  template entry for its port, as Golden A's -201U1:DP shows (an auxiliary `ew.fn.dp-port` function outside its part's
  template).
- **Compatibility.** Additive within format 0.1 (ADR-0011 amendment 4, ID-334). *Amended in cycle 40 (decision log
  ID-927; the lane's finding, the review's part-3 finding 6):* the claim that both builds write the same bytes was
  wrong. An older 0.1 build keeps `bus` in `PartSections.extra` and writes the section's members with sorted keys,
  while this build writes D15's member order. **Older → this build:** such a copy is reported not canonical and
  rewritten in D15's order at the next save, its stored `sha256` kept. **This build → a pre-cycle-40 build:** the older
  build re-serialises the section with sorted keys, so its `library_copy_hash` differs from the stored hash and
  EW-CORE-014 fires on every copy of EW-GEN-CPU-1 0.1.1. Both are accepted within 0.1 under ID-334, as for the PLC and
  enclosure sections: no 0.1 build ever wrote a part bus section, so nothing existing is affected. The member carries
  no cascade semantics, so it would stay additive after a format release.
- `HeapSize` for both structs (`crates/ew-commands/src/heap.rs`); `schemas/0.1` re-blessed.

### 8a.2 The relation part → function: normaliser N5 (D16)

N5 "bus ports from the part", registry ID `bus_ports`, `crates/ew-commands/src/normalise/bus_ports.rs`, in `ORDER`
after `module_pin_binding` and before `revision_ensure_open` (doc 06 §3.2: between rows 14 and 15, so N4's
logical-net inheritance, 1.0, will find the port; after online numbering, so its notices print numbered tags).

- **The bus part** of a device: its first part use (in `Device.parts` order) whose part copy has a `bus` section with
  at least one port.
- **A bus-port function:** a function whose FnDef copy has contact kind `bus_port`.
- **A carried port:** a part port is carried when a function of the device has a `bus_port` with the same `interface`
  and `port_label` (compared exactly).

**Trigger.** (i) A device of the transaction that is new, or whose bus part names another part copy than in the base
snapshot (`device.place` and `box.black` from a part, `device.create`, `part.assign` in every mode, `fields.set` on
`parts`, `edit.paste`, `macro.insert`). (ii) A bus-port function that is new in the transaction or whose `device`
changed in it (`device.place` with `join_device`, `rep.join_device`), on a device with a bus part.

**Effect.** For each triggered device:
1. The part's ports in list order, the first of each (`interface`, `port_label`) pair; a port whose `bus` or `role` is
   `Unknown` is left out with notice **N5-c** (trigger (i) only).
2. Of those, the ports that no function of the device carries, in list order, are paired with the device's port-less
   bus-port functions in ID order (under trigger (ii) alone, only the new or moved functions are candidates, so a port a
   drafter removed from an older function stays removed), and each pair gives the function
   `bus_port = {interface, port_label, bus, role, address, termination, logical_net: none, physical_net: none}` with
   `address` = `not_applicable` for role `passive`, else `tbd`, and `termination` the part's (absent when the part has
   none). Surplus ports and surplus functions are left as they are, without a notice: a CPU placed before its port is
   drawn has nothing to report.
3. Under trigger (i), each carried port whose `bus` or `role` differs from the part port's keeps its value, with notice
   **N5-b**.

**Notices** (exact texts; `<port>` is the device's tag as `crate::words` prints it, a colon and the interface, then a
space and the port label when there is one: `-A1:DP`):
- **N5-b** `<port> keeps its bus port; part <number> declares <bus> <role>`, with the stored names (`profinet
  controller`).
- **N5-c** `bus port <interface> of part <number> is of a newer format and was not applied`.

**Never rewritten.** N5 never changes or removes an existing `bus_port`; removing one stays `fields.set bus_port null`,
and the function stays port-less until a trigger finds it a candidate again (a new part, or the function moved to
another device). On `part.assign replace` (and `replace_all`, `fields.set parts`) the carried ports stay, the new
part's uncarried ports go to port-less functions, and a disagreement is a notice. Removing the bus part removes no
port. Deleting the function or device deletes its port as before. `library.update` keeps a copy's ID, so it does not
trigger N5 (as for N0; FT-830).

**Undo and preview.** N5's writes are part of the command's transaction: they appear in its preview and undo and redo
with it as one step, with identical IDs. N5 is idempotent: after a run every paired function carries its port, so a
confirming pass changes nothing and repeats no notice.

**Rules.** No EW-BUS rule reads the part port at the MVP: EW-BUS-001 and EW-BUS-004 read `Function.bus_port`, and a
seeded `tbd` address is reported by EW-CORE-044 outside every logical net and by EW-BUS-001 inside one (ID-896
decision 1). The check that a function's port disagrees with its part's is 1.0 (FT-829).

### 8a.3 The data (D17)

- **EW-GEN-CPU-1** gains, beside `plc = {role: station}`,
  `"bus": {"ports": [{"interface": "DP", "bus": "profibus_dp", "role": "master"}]}`, and nothing else: no CP, no
  template entry (the box would otherwise draw the port's pins, because a black box takes the whole part pattern), no
  termination (the plug carries it), no connector, signal map or range (a generic part claims no pinout). Master,
  because a CPU's DP interface is the master by default, and Golden A already exercises the slave. Part version
  **0.1.1** (`BUMPED`), library **0.2.1** (`LIBRARY_VERSION`): 0.2.0 is published on `main` and a library refuses other
  content under a published version (doc 05 §13.1); a patch step, as FT-659's part changes took. Regenerated with
  `EW_BLESS=1 cargo nextest run -p ew-library --test starter_parts`, run twice.
- **No other starter part** gains a port (none has a bus interface).
- **Golden A:** no part gains a `bus` section; its -201U1:DP and -3X1 ports are typed on their functions (D11) and its
  parts 195740 and the plug have unverified signal maps and no template entry that would carry a port. **No golden
  change**; Golden A's parts gain their ports with FT-829.

### 8a.4 Build list

1. `ew-schema`: `BusSection` and `PartBusPort` in `bus.rs`, re-exported from `lib.rs`; `PartSections.bus` in
   `library.rs`; `schemas/0.1` re-blessed (the generator in `crates/ew-schema/src/schema.rs`).
2. `ew-commands`: `HeapSize` for both structs; N5 in `normalise/bus_ports.rs`, registered in `registry()` and `ORDER`,
   the module documentation's table gaining its row, the order test updated.
3. `ew-library`: EW-GEN-CPU-1's section, `BUMPED` and `LIBRARY_VERSION` in `tests/starter_parts.rs`; the regenerated
   `content/libraries/ew-starter-parts.ewlib/**` and `src/starter_parts.rs`.
4. Tests of §8a.5; no new command, refusal, rule, message, DSL verb or fuzzer arm.

### 8a.5 Acceptance

- **BU-7a** (`ew-schema`) A part document whose `sections.bus` holds a port with every typed member, unknown port
  members (`connector`, `signals`, `address_range`), an unknown section member (`rates`) and a second port with
  `bus: "sercos"`, `role: "observer"`, `termination: "auto"` deserialises with the typed members typed and the rest in
  `extra` or `Unknown`, and serialises byte-equal (compact JSON; members in §8a.1's order, then `extra`). A part without
  `bus` serialises as before; `{"bus": {"ports": []}}` serialises as `{"bus": {}}`.
- **BU-7b** A port without `interface`, one without `bus` and one without `role` each fail to deserialise, the error
  naming the member.
- **BU-7c** `schemas/0.1` re-blessed; the diff only adds (`BusSection`, `PartBusPort`, the `bus` property of
  `PartSections`); no existing definition changes otherwise.
- **BU-7d** (`ew-format`) A project whose `lib/` holds a part copy with BU-7a's section loads and saves byte-equal and
  its stored `sha256` verifies; `ew-library` reads a library whose part has the section.
- **BU-7e** (`ew-library`) EW-GEN-CPU-1 is at 0.1.1 with `sections.bus.ports` = one port `DP`, `profibus_dp`,
  `master`, no `port_label` or `termination`; its CP pattern, function template and PLC section equal 0.1.0's; the
  library is 0.2.1; the generator test passes and a second `EW_BLESS=1` run changes nothing.
- **BU-7f** (`ew-api`, through `Session`) In a new project, placing EW-GEN-CPU-1 from the picker gives no bus port
  and no notice. Placing the starter symbol `ew.bus.dp-port` with `join_device {device: the CPU, role: aux}` gives the
  new function `bus_port = {interface: "DP", bus: "profibus_dp", role: "master", address: "tbd"}` with no termination
  and no nets, in the placement's one undo step: undo removes the function and its port, redo restores identical IDs;
  the placement's preview lists the port. The checks then report one EW-CORE-044 finding with discriminator
  `bus_address` for that port.
- **BU-7g** (`ew-commands`) A device with a black box and two `ew.fn.dp-port` functions and no part: `part.assign`
  EW-GEN-CPU-1 gives the lower-ID function the port and leaves the other port-less; `part.assign` onto a device without
  a bus-port function gives no port and no notice; `rep.join_device` moving a port-less `ew.fn.dp-port` function onto a
  CPU device whose port is not carried gives it the port. *As built (cycle 40, ID-927): met in part.* The `part.assign`
  clauses pass; the `rep.join_device` clause cannot be reached, because WP-S2.18 D2 lets only a contact join a device as
  an auxiliary function, so the join is refused before N5 runs; extending D2 is FT-841.
- **BU-7h** (never rewritten) After `fields.set` `bus_port.address` to `{"value": {"profibus": 2}}` on BU-7f's port:
  `part.assign replace` to a fixture part whose port is `{interface: DP, bus: profinet, role: controller}` leaves the
  port unchanged with the notice `<port> keeps its bus port; part <number> declares profinet controller`;
  `part.assign replace_all` to a part without a bus section and `fields.set parts []` leave it unchanged with no
  notice; after `fields.set bus_port null`, an unrelated edit of the device leaves the function port-less.
- **BU-7i** A fixture part whose port has `bus: "sercos"`: placement or assignment seeds nothing, with the notice `bus
  port DP of part <number> is of a newer format and was not applied`. A fixture connector part with a passive port and
  `termination: on` on an `ew.fn.dp-connector` function seeds `address: not_applicable`, `termination: on`.
- **BU-7j** `ORDER` has `bus_ports` between `module_pin_binding` and `revision_ensure_open`; the order test asserts
  it. Golden A and AC-15 path (b): every exported byte and the acceptance result unchanged (N5 a no-op: no part of
  Golden A has a bus section); twice-run determinism.
- **BU-7k** `cargo nextest run -p ew-schema -p ew-format -p ew-library -p ew-commands -p ew-api`, the matching
  clippy, `cargo fmt --all --check`, `cargo xtask check-deps`.

### 8a.6 Files owned and stop rules

**Files owned (I40):** `crates/ew-schema/src/{bus.rs,library.rs,lib.rs}`, `crates/ew-schema/tests/**`,
`schemas/0.1/**` (re-bless only); `crates/ew-format/tests/**`; `crates/ew-commands/src/normalise/{mod.rs,bus_ports.rs}`,
`crates/ew-commands/src/heap.rs`, `crates/ew-commands/tests/**`; `crates/ew-library/tests/starter_parts.rs`,
`crates/ew-library/src/starter_parts.rs`, `content/libraries/ew-starter-parts.ewlib/**`; `crates/ew-api/tests/**`.
Read-only: everything else, `golden/**` included.

**I40 stops and reports instead of choosing when:**
1. any exported byte of Golden A, AC-15's path (b) or the acceptance result changes;
2. a member beyond D15's would be typed, a required member beyond `interface`, `bus` and `role` added, or `bus` placed
   elsewhere than after `mounting_plate`;
3. N5 would need to change or remove an existing port, set a net, seed an address other than `tbd` or
   `not_applicable`, or give a port to a function whose FnDef's contact kind is not `bus_port`;
4. a new command, refusal, rule, catalogue row or message would be needed, or a test outside the owned files changes
   its expectation;
5. `main` does not publish `ew-starter-parts` 0.2.0, or 0.2.1 is already published;
6. a new workspace package, dependency edge or external crate would be needed.

I40 decides freely: private helper names, the module layout inside the owned files, fixture parts and test names.

## 9. Part 5: the I/O list, card supply groups and the model follow-ups

**Scope.** D13, FT-320, FT-321, FT-352. **0.5 pw.**

**Acceptance.** **PX-1** `io_list` on Golden A lists 56 rows in module order with FR-PLC-023's columns; `ew export
report --block <io_list block> --format csv` writes them. **PX-2** the supply groups of -201U3 typed from its part;
the potential rules read them (no finding change on Golden A). **PX-3** a PLC overview box of -201U3 placed on an
overview page lists its 8 channels with addresses and cross-references (de-scope candidate). **PX-4** moving a valve
out of its module unbinds its coil in the same transaction with a notice (FT-320); a module re-parted to a part with
another `pin_of` re-pins its channels (FT-321); N2 finds valve children through the parent index (FT-352), same
results as before on Golden A. **PX-5** targeted checks for the changed crates.

## 10. Files owned and what a lane may not decide

### 10.1 Files owned

**Part 1:** `crates/ew-commands/src/normalise/**`, `crates/ew-commands/src/commands/{fields.rs,plc.rs,mod.rs}`,
`crates/ew-commands/tests/**`; the field catalogue in `crates/ew-api/src/output/fields.rs`, `crates/ew-api/tests/**`;
`crates/ew-library/tests/starter_parts.rs`, `crates/ew-library/src/starter_parts.rs`,
`content/libraries/ew-starter-parts.ewlib/**`. **Parts 2 to 5:** named by crate in their scope and fixed by each
part's dispatch brief; golden files only under D11 and PT-4. **Part 4b:** §8a.6 (no golden file).

Shared append-only files (registry lists, `params.rs`, fuzzer arms, DSL verbs, the strings table, `schemas/0.1`,
the rule catalogue's generated pages) may be touched by other lanes; the second to integrate rebases. Read-only for
every lane: other crates, `golden/**` except as named, `ci/**`, `.github/**`, `docs/**` except generated reference
pages. Each lane commits on its own branch and does not merge or push.

### 10.2 What a lane may not decide

A lane stops and reports instead of choosing when:
1. a change needs a new workspace package, a new dependency edge in `ci/dep-rules.toml` or a new external crate;
2. a member or kind beyond D6, D11 and D15 would be typed, or a required member added (part 4b: §8a.6);
3. any exported byte of Golden A, AC-15's path (b) or the acceptance result changes outside PT-4 and BU-5;
4. a golden fact here (the 56 channels, H1/H2's 28/0, the segment order, EF-02, EF-03, EF-16) is contradicted by the
   data;
5. N0 would need to change or remove an existing aspect, or attach a module to a station other than D4's single
   station;
6. a command beyond D8's, or a refusal beyond D6's address parse and D8's profile check, would be added;
7. the work of a later part is reached (D14).

The lane decides freely: private helper names, module layout inside the owned files, the exact notice texts beyond
those quoted here, test fixtures.

## 11. Risks

| Risk | Likelihood · impact | Mitigation |
|---|---|---|
| R1 N0 fires during path (b) or an import and changes Golden A | Medium · high | N0 never touches a device that already carries an aspect; PW-7 and §10.2 item 3 stop the lane |
| R2 The implicit attachment of D4 surprises a user with two PLCs | Low · medium | Only with exactly one station; the notice names it; undo reverts; the R0 usability round may reopen D4 |
| R3 The bus typing breaks the DP potential (folio 909) | Medium · medium | BU-5's byte-equal tests; D11 lists the only intended changes |
| R4 Part 3 overruns with the Should commands | Medium · low | They are built last and leave first (§12) |
| R5 The golden address texts of PT-2 move other texts | Low · medium | PT-4's per-folio byte comparison; the lane lists every changed folio |
| R6 N5 seeds a port the drafter did not want (part 4b) | Low · low | Only on a port-less bus-port function, only from the part's uncarried ports; one undo step; never on a CPU placed alone (no bus-port function); `fields.set bus_port null` removes it for good |

## 12. Size, order and routing

**Size.** 4.3 pw of the 6.3 pw PERT (D1): part 1 **0.9** (N0 and the configuration project 0.35, channel fields
0.3, the CPU part 0.05, tests 0.2), part 2 **0.6** (slot resolution 0.25, black-box pins 0.2, tests 0.15), part 3
**1.1** (commands 0.25, cut-off 0.15, seven rules with pages 0.5, Should commands 0.1, tests 0.1), part 4 **1.2**
(kinds and schemas 0.35, `bus_segments` 0.3, golden change 0.2, two rules 0.2, tests 0.15), part 4b **0.35** (cycle
40: the member and schemas 0.1, N5 0.15, the CPU part 0.02, tests 0.08; over part 4's 0.05 for BU-7, recorded, the CSV
unchanged), part 5 **0.5**. **De-scope
rule:** when a part is projected (at its lane's midpoint report) or found (at its review) more than 20 % over its
size, these items leave as follow-ups without a further ruling, in this order: the PLC overview box (FR-PLC-052),
`plc.convert_manual_to_auto`, `plc.readdress`, the IEC profile's rendering (PA-3 then tests TIA only); the lane
reports which ones left.

**Order and routing** (Opus or Haiku only, ID-700): part 1 first, **Opus at `high`** (a core normaliser and
commands; cycle 36); part 2 after part 1, **Opus at `medium`**; part 3 after part 1, side by side with part 2,
**Opus at `high`**; part 4 after part 1, **Opus at `xhigh`** (format typing and a golden change); part 4b after part 4,
**Opus at `high`** (a format member and a core normaliser, no golden change; cycle 40, lane I40); part 5 last, **Opus
at `medium`**. **Unblocks:** WP-S2.18 part 4 after parts 1 and 2 and WP-S2.07 part 3; WP-S1.20 (fluid) reads part 1's
aspects; WP-S1.24 builds on parts 3 and 4.

## 13. Follow-ups and the documents to update

**Follow-ups.** FT-224, FT-225, FT-226, FT-227, FT-270, FT-320, FT-321, FT-350, FT-352 and the PLC and bus rows of
FT-589 close with their parts; FT-323 closes with part 3 for the PLC section and stays open for the bus section until
WP-S1.24; FT-324 and FT-322 stay with their packages. A de-scoped item becomes a new FT at the design pass.
**Part 4b** (cycle 40): FT-770 closes with it; it opens FT-829 (the part port at 1.0: the disagreement check, the
address range, rates, connector and signal map, Golden A's part ports), FT-830 (`library.update` triggers neither N0
nor N5) and FT-831 (the window offers a part's ports as a placement).

**Documents.** **After part 1:** doc 06 §3.2 (row N0 and `ORDER`), doc 10 §3.3 (as built), doc 10 §2.2 (the
configuration project on demand); [WP-S2.18](WP-S2.18-basic-elements-from-the-window.md) D5 (EW-GEN-CPU-1).
**After part 2:** doc 10 §3.6 (as built). **After part 3:** doc 10 §3.9 and §3.10 (as built), doc 15 §4.5.1 (rule
pages), doc 06 §3.1 (the command count). **After part 4:** doc 10 §2.7 and §3.8 (as built), doc 05 §2.2 (the kinds),
doc 03 §14, doc 20 §11 and the golden change list. **After part 4b:** doc 10 §2.6 (as built; specified there in
cycle 40), doc 07 §2.5 (`PartSections.bus` in the typed list), doc 03 §7 (the part record's sections), doc 06 §3.2
(N5, between rows 14 and 15), [WP-S2.18](WP-S2.18-basic-elements-from-the-window.md) D5 (EW-GEN-CPU-1's port).
**After part 5:** doc 09 (`io_list`), doc 10 §5.

---

## Sources

- [Work breakdown](../02-work-packages.md): WP-S1.21, WP-S1.24, WP-S1.20, WP-S2.07, WP-S2.18; Appendix B (FT-192,
  FT-224 to FT-227, FT-270, FT-320 to FT-324, FT-350 to FT-352, FT-589, FT-593).
- [Decision log](../06-decision-log.md): ID-333 to ID-337, ID-395, ID-399, ID-464 to ID-469, ID-524, ID-700,
  ID-812, ID-820, ID-823, ID-829, ID-835.
- [PLC and bus](../../design/10-plc-and-bus.md); [schematic editor](../../design/06-schematic-editor.md) §3.2;
  [checks](../../design/15-checks-revisions-collaboration.md) §2.3, §3.6, §4.5;
  [golden project](../../design/20-golden-project.md) §11, §18.1, §22.
- [ADR-0004](../../adr/0004-identity-uuidv7-uuidv5.md), [ADR-0008](../../adr/0008-transactions-undo-preview.md),
  [ADR-0011](../../adr/0011-format-versioning-and-migration.md),
  [ADR-0019](../../adr/0019-rule-engine-and-rule-packs.md).
