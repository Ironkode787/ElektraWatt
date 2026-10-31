# PLC, I/O and fieldbus

**Status:** Draft v0.7 · **Date:** 2026-10-31 (v0.7: cycle-40 specification lane S40: the part's bus section `sections.bus` and normaliser N5, specified, §2.6; decision log ID-920; v0.6: cycle-38 design pass, part 2: the IEC bit address without its size, EW-PLC-001 per overlap group and a black-box card's pins in `plc.bind_channel` as built, §2.5, §3.9, §3.10; decision log ID-884; v0.5: cycle-38 design pass: the pin texts, the addressing commands, the MVP EW-PLC rules and the PLC navigator as built, §3.6, §3.9, §3.10, §6; decision log ID-870, ID-874, ID-876, ID-877; v0.4: cycle-21 design pass: the map-language grammar as normative text, no subtraction, §2.6; decision log ID-523 to ID-525; v0.3: cycle-18 design pass: the solenoid-position map and the pin-designation map as built, derivation step 4, N1 to N3 and `plc.set_module_part`, the FR-PLC-051 rewording, §2.4, §2.6, §2.10, §3.2, §3.3, §3.9, §4; decision log ID-464 to ID-469; v0.2: cycle-13 design pass: the PLC record kinds, channels and the DP bus potential as built, §2.2–§2.4, §2.6, §2.10, §3.2, §3.3, §3.8, §5; decision log ID-333 to ID-337) · **Part of:** ElektraWatt design document set

**Purpose.** This document specifies how ElektraWatt (EW) models PLC configuration projects, stations, modules,
channels and fieldbus networks. It defines how EW derives addresses, symbols, function texts and targets instead of
storing them as text, how it exchanges I/O data with TIA Portal, CODESYS and spreadsheets, and which rules check the
result. It elaborates the PLC, BUS and PLC-exchange EXP rows of [02b](02b-requirements-extended.md) on top of the
[domain model](03-domain-model.md) §13–§14 and the accepted [architecture](04-architecture.md) (ADR-0001, ADR-0003,
ADR-0006). The running example and acceptance case is the reference project's Festo valve terminal -201U1 on
folios 3, 106 and 201–206.

**Contents**
1. [Scope and boundaries](#1-scope-and-boundaries)
2. [Concepts and data](#2-concepts-and-data)
3. [Behaviour and rules](#3-behaviour-and-rules)
4. [Exchange with PLC software and spreadsheets](#4-exchange-with-plc-software-and-spreadsheets)
5. [Outputs: report row sources](#5-outputs-report-row-sources)
6. [UI and UX touchpoints](#6-ui-and-ux-touchpoints)
7. [CLI and automation](#7-cli-and-automation)
8. [Edge cases and pitfalls](#8-edge-cases-and-pitfalls)
9. [The reference project](#9-the-reference-project)
10. [Scope by release](#10-scope-by-release)
11. [Dependencies](#11-dependencies)
12. [Requirements covered](#requirements-covered), [Open questions](#open-questions)

---

## 1. Scope and boundaries

| Topic | Owner |
|---|---|
| PLC and bus records, derived PLC queries, address profiles, part PLC and bus sections, symbols, target tracking, EW-PLC/EW-SAF/EW-PRC/EW-BUS/EW-IOL rule content, PLC exchange formats, device descriptions | this document |
| Valve fluid function, ports 1–5, tubes, media, EW-FL rules, `valve_terminal_config` report | [11-fluid-power](11-fluid-power.md) |
| Assembly overview generation (201/202), child tags -201U3-Q1, DeviceLink | [03](03-domain-model.md) §7, [06](06-schematic-editor.md), [07](07-symbols-parts-macros.md) |
| Bus line connections, cable -3W1 as a cable, card supply potentials | [08-connections-terminals-cables](08-connections-terminals-cables.md) |
| Forms, pagination, XLSX/CSV mechanics, the `_ew` round-trip sheet | [09-reports-forms-output](09-reports-forms-output.md) |
| Generic import/export contracts, QET `plc_*` element info, EDZ PLC data | [14-interop](14-interop.md) |
| Rule engine, modes, waivers, release gate | [15-checks-revisions-collaboration](15-checks-revisions-collaboration.md) |
| PLC hardware in the cabinet, EtherCAT rail order | [12-panel-layout](12-panel-layout.md) |
| PLC workspace layout, persona P3, journey J4 | [16-ux-ui](16-ux-ui.md), [vision](00-vision-and-scope.md) §5–§6 |

Conventions: Golden A, B and C are the variants of [20-golden-project](20-golden-project.md); U-nn (unknowns),
EF-nn (expected findings), AC-nn (acceptance criteria) and MF-nn (migration findings) are its IDs. ▲ marks a value
that is unknown in the reference set and used as a test input. "(unverified)" marks a vendor fact from the
[PLC research](../research/plc-bus-fluid.md) that must be checked before it becomes a default (NFR-COMP-05).
Questions for the pilot user are cited by their PQ IDs in [01-brainstorm](01-brainstorm.md) §7. Abbreviations
follow [02a](02a-requirements-core.md) §1.2: PBF = [PLC research](../research/plc-bus-fluid.md), ARCH =
[architecture](04-architecture.md), C-n = [gaps and corrections](../research/gaps-and-corrections.md), SC-n =
success criteria of the [vision](00-vision-and-scope.md) §3.2. "EPLAN nnnnn" is an EPLAN property number (PBF §2.1).

---

## 2. Concepts and data

### 2.1 Overview

```mermaid
flowchart LR
  CFG["PlcConfigProject PLC1<br/>profile siemens-tia-intl"] --> ST
  subgraph D1["Device -201U1 (195740 + 550208)"]
    ST["PlcStation<br/>role ValveTerminal"]
    BP["BusNode + BusPort 'DP' on an aux function<br/>address Tbd"]
    AS["Assembly: order codes 1 and 2"]
    VT["ValveTerminal: pilot supply (11-fluid-power)"]
  end
  ST --> MOD["-201U3 … -201U8, -202U2<br/>PlcModule, positional slots 1–7"]
  MOD --> CH["Channel 0 … 7 per module<br/>56 records"]
  PART["Part 533361: io width, channel map,<br/>solenoid-position map"] -.->|maps| CH
  CH -- "binding" --> COIL["coil 14 CP of -201U3-Q1<br/>internal, physical = false"]
  COIL --- FLU["fluid function 5/2, ports 1–5<br/>folio 203"]
  BP --- LN["LogicalNet PROFIBUS DP<br/>master: external partner JE-N1000-03"]
  BP --- PN["PhysicalNet: derived segment<br/>external end · -3W1 · -3X1 · -201U1:DP"]
```

The **channel is the hub** (PBF §2.4). Its binding joins the electrical or internal model, its address the PLC
process image, its symbol the PLC software, its target the field function and, through a `DeviceLink`, the process
tag. Nothing a channel displays is stored as text anywhere else (ARCH §1.2, principle 1). Every record below is an
`ew-schema` record. The fields marked *added* refine the first draft of [03](03-domain-model.md) §13–§14; the
schema-steward batch (ARCH §7.6) adopted them, and 03 §13–§14 lists them (Open questions 3, 4, 6, 15 and 22).

### 2.2 Configuration project

One `PlcConfigProject` exists per target PLC-software project: EPLAN's configuration project, a TIA project or a
CODESYS application. It lives in `project.jsonl` ([05](05-project-format-and-persistence.md) §2.2, rank 11) and scopes
address and symbol uniqueness (FR-PLC-002). A new project contains one default, `PLC1`, with the profile "Siemens TIA
(international)".

```rust
pub struct PlcConfigProject {
    pub name: Ident,                     // "PLC1"
    pub profile: AddressProfileRef,      // "siemens-tia-intl@2027.1" (§2.5)
    pub cpu: Option<Id<Device>>,         // None in Golden A: the DP master sits in +J-E-206
    pub allocation: Allocation,          // added: Packed (default) | SlotBased (1.x)
    pub ownership: Ownership,            // added: an Owner per field group, see the table below (FR-PLC-033)
    pub symbols: SymbolPolicy,           // added: { template: Option<String>, strict_iec: bool, tag_table: Ident }
    pub exchange: Option<ExchangeStamp>, // added: { format, version, file_sha256, direction, rev, date };
                                         //   replaces foreign_ids + last_exchange; body in exchange/ (§4.2)
}
pub enum Owner { Ecad, Plc, Shared }     // added (replaces EcadOwns | PlcOwns); Shared: a one-sided change
                                         //   applies, a two-sided change is a conflict
```

| Field group | Default owner | After the first PLC import | Configurable |
|---|---|---|---|
| Structure: stations, modules, slots, order numbers | Ecad | Ecad | yes |
| DTs, pins, wiring, function texts, parts, process tags | Ecad | Ecad | no |
| Module start addresses | Ecad | **Plc** (protected until unlocked) | yes |
| Symbols, comments | Shared | Shared | yes |
| Firmware, IP addresses, PROFINET names, DP station addresses | Plc | Plc | no |

The defaults follow [02b](02b-requirements-extended.md) Open question 11 and IDEA-067. `Shared` is the refinement
that makes FR-PLC-033's acceptance work: a symbol renamed in TIA and a function text changed in EW merge without a
conflict, while the same symbol renamed on both sides gives one conflict row. Ownership drives the merge table of
§4.4 from the first exchange on. **Protection** (§3.9) starts with the first applied PLC import and covers every
field group whose owner is then `Plc`: before that import, EW edits of the DP station address (Golden A-R sets 3)
are ordinary edits, flagged "export pending".

**As built (cycle 13; decision log ID-333).** `plc_config_project` (rank 11, `project.jsonl`) is typed in `ew-schema` with `name`, `profile` (the pinned reference `id@version`, here `siemens-tia-intl@2027.1`) and `cpu`, plus the import-provenance member `source_ref`. The address grammar lives in the new permissive crate `ew-plcaddr` (addresses, the built-in TIA address profile, unit intervals and the part map language of §2.6). `allocation`, `ownership`, `symbols` and `exchange` are not typed yet and stay in `extra` until their work packages, so the ownership table above and the protection of §3.9 are design only. Golden A has one configuration project, `PLC1`, without a CPU. The kind was already reserved with its rank and ownership, so it was implemented additively within format 0.1 (decision log ID-334). Deleting the CPU device unlinks `cpu`; a configuration project cannot be deleted while a `plc_station` aspect still names it.

### 2.3 Stations and modules

```rust
pub struct PlcStation {                  // device aspect (ARCH §4.3)
    pub config_project: Id<PlcConfigProject>,
    pub role: StationRole,               // Cpu | HeadStation | IoDevice | ValveTerminal | IoLinkMaster | Drive | ExternalPartner
    pub firmware: Option<Ident>,
    pub dd: Option<DeviceDescRef>,       // §2.8
    pub order: Option<u32>,              // added: address-allocation order; never derived from tags
}
pub struct PlcModule {                   // device aspect on the station device or on a child device
    pub slot: SlotPath,                  // Rack{rack, slot, subslot} | Positional
    pub type_ident: TypeIdent,           // { order_number: "533361", dd_module: Option<Ident> }
    pub start: IoStarts,                 // added, one start per direction: { input: AddrSpec, output: AddrSpec }
    pub part_use: Option<u8>,            // added, replaces the `address_map` ref: which PartUse supplies the
                                         //   PLC section (maps included) if the device has several parts
    pub integrated: bool,                // created with the station; not deletable alone (FR-PLC-003)
    pub safety: Option<SafetyModule>,    // added: §2.9
}
```

**Where the aspects go.** The `PlcStation` aspect sits on the device that owns the bus port and the address window.
The `Assembly` aspect sits on the device that carries the configurator order codes. On -201U1 both coincide with
`BusNode` and with the `ValveTerminal` aspect that [11](11-fluid-power.md) owns (ARCH §4.3). A valve terminal
whose bus interface is a separate module (an SI unit, a CPX-AP interface) carries `Assembly` on the head device and
`PlcStation` plus `BusNode` on that module. A multi-pin valve terminal
without a bus node has no station at all: its coils are wired through a multi-pin cable to physical channels of
another module. [03](03-domain-model.md) §13 states the same rule (its Open question 6, resolved).

**Slots.** `SlotPath::Rack` holds rack, slot and subslot for racks and remote I/O, authored or imported from TIA.
`SlotPath::Positional` derives the slot as the 1-based rank of the module among the station's I/O-module descendants,
ordered by `Device.position` (manifold order). Golden A uses `Positional`: -201U3 … -201U8 are slots 1–6 and -202U2
is slot 7. -201U2, -201U9, -202U1 and -202U3 carry no `PlcModule` and do not count. The slot therefore never depends
on a tag or on the page a child is drawn on (D-41).

**A module's I/O widths come from part data** (§2.6). A module may have an input and an output range at once, so
`start` holds one `AddrSpec` per direction. AR APC carries the same list of `{StartAddress, Length, IoType}`.

**As built (cycle 13; decision log ID-333, ID-337).** `plc_station` carries `config_project`, `role` (`StationRole`, all seven variants) and `order`. `plc_module` carries `slot` (`SlotPath`: `positional`, the default, or `rack` with rack, slot and subslot), `start` (`IoStarts`; `auto` for both directions by default), `part_use` and `integrated`. Not typed, and kept in `extra` until their work packages: `PlcStation.firmware`, `PlcStation.dd` (§2.8), `PlcModule.type_ident` (with the `plc_card_list` of §5) and `PlcModule.safety` (§2.9). The aspects are written in the member order of [05](05-project-format-and-persistence.md) §3.1: `assembly` (held verbatim until its work package) before `plc_station` before `plc_module`. The module order of `module_order` is: stations by (`order`, ID); within a station, rack slots first by (rack, slot, subslot), then positional modules in manifold order, whose slot is their 1-based rank. Golden A has seven modules (-201U3 … -201U8, -202U2).

### 2.4 Channels

```rust
pub struct Channel {                     // shard of its module device; id = v5(module_id, "channel", number)
    pub module: Id<Device>,
    pub number: u16,                     // 0-based, = AML Channel/Number
    pub designation: Option<Ident>,      // manual; automatic from the part pattern ("{n}", "DO {n}", "X2:{n+1}")
    pub kind: ChannelKind,               // Di | Do | Ai | Ao | Counter | MultiFunction{mode} | IoLinkPort | PowerSupply
    pub data_type: PlcDataType,          // profile vocabulary + canonical bit width ("Bool" = 1)
    pub address: AddrSpec,               // Auto | Manual(PlcAddress) | Excluded | Tbd
    pub state: Option<ChannelState>,     // added Option: explicit Spare | Reserved | Deactivated;
                                         //   None = derived (§3.2, FR-PLC-019)
    pub symbol: Option<Ident>,           // manual; automatic from the template (§3.4)
    pub comment: MlText,                 // PLC comment; exported as AML/TIA comment
    pub binding: Option<Endpoint>,       // signal CP: a module pin, or the served coil's internal CP
    pub aux_pins: SmallVec<ChannelPin>,  // { role: Supply | M | Shield, cp }
    pub target: Option<Id<Function>>,    // added: manual target override (§3.5, FR-PLC-054)
    pub signal_range: Option<Ident>,     // added: "4..20 mA"
    pub safety: Option<SafetyChannel>,   // added: §2.9
}
```

- **Identity.** Channels are created by a normaliser from the module's part data when the `PlcModule` aspect is
  added (§3.3). They are stored, because they carry authored data, but their IDs are
  `v5(module_id, "channel", number)`, following ADR-0004's "instantiated → v5" rule. Two branches that add the
  same channel produce the same record, and an AML export can use the ID directly (§4.2).
- **Invariants.** S: a channel belongs to exactly one module (ARCH §4.6). P: a CP is in at most one channel
  (FR-PLC-005). P: addresses do not overlap within configuration project and area (EW-PLC-001). P: symbols are unique
  per configuration project (EW-PLC-007). Strict mode rejects; lenient mode (imports) reports (ADR-0009).
- **Determined values.** EPLAN's manual / automatic / determined triple (PBF §2.1, lesson 2) applies to every text a
  channel shows. The determined value is printed and exported; the property grid shows its provenance.

| Field | Manual | Automatic | Release of automatic |
|---|---|---|---|
| Designation | `Channel.designation` | part pattern over `number` | MVP |
| Symbol | `Channel.symbol` | template over the target (§3.4) | 1.0 (FR-PLC-017) |
| Function text | none: it is edited at the target device | the target device's determined function text, else the tracked process tag of the device it drives (§3.4) | MVP for direct targets; 1.0 for traced targets |
| Comment | `Channel.comment` | `{function_text}` at export, if empty | 1.0 |
| Target | `Channel.target` (FR-PLC-054) | target tracking (§3.5) | MVP direct; 1.0 traced (FR-PLC-018) |

**As built (cycle 13; decision log ID-333, ID-335).** `channel` (rank 37, in the shard of its module device) is typed with `module`, `number`, `designation`, `kind`, `address` (`AddrSpec`: `auto`, the default, `manual`, `excluded`, `tbd`), `state`, `symbol`, `comment`, `binding`, `target` and the import-provenance member `source_ref`. `ChannelKind` has `di`, `do`, `ai`, `ao`, `counter`, `io_link_port` and `power_supply`; `multi_function` (§3.8) is additive. Not members of the record, and kept in `extra` until their work packages: `data_type`, `aux_pins`, `signal_range` and `safety`. Until then the data type is the part's (`ChannelSpec.data_type` in the part section, §2.6), not a channel member. **Channel IDs** are `v5(module, "channel", number)` (ADR-0004 amendment 2; `ew_ids::v5::plc_channel_id`); the golden importer maps each `channels.csv` row to that ID, not to the `v5(ns, "channels|key")` of the other golden tables, so two branches that add the same module mint the same channel IDs (decision log ID-335). `source_ref.key` still reads `channels|<label>`. Until cycle 16 channels came only from import; since then normaliser N1 of §3.3 creates them from a module's part data with the same IDs (decision log ID-399), and a channel exists when one with the same module and number exists. Link policy: a channel is deleted with its module; deleting the bound function or the target function unlinks the member. An older 0.1 build keeps channel records verbatim but does not enforce these links; a check for orphan channels is FT-229, and FT-230 tests the older-build case (decision log ID-334).

**As built (cycle 18; decision log ID-464 to ID-468).** A channel's `kind` is instantiated part data: N1 restores the
part's channel kind when the module's part changes (FT-276). Its `binding` is set by N2 (a valve coil, by the
position map), N3 (a module pin, by `pin_of`) or `plc.bind_channel` (§3.3). The automatic designation and `pin_of`
expand the part's pattern with one shared function, `ew_derive::expand_pattern` (FT-318, cycle 21): every
`{<expression in n>}` of §2.6 is expanded (`"X2:{n+1}"` prints `X2:1` for channel 0), and a pattern that does not
expand (an unclosed `{`, a stray `}`, an expression that does not parse, one in `p`, an overflow) gives no
designation and one derivation note per module, as it gives no pin and a notice in N3. The map language has no
subtraction, so `{n-1}` does not parse. Since cycle 21 the derivation finds a module's channels as N1 keys them, by (module, number) and not by their v5
IDs: the channels-by-module index (FT-319, below). Of two channels with one number the lower ID is the channel.

### 2.5 Addresses and address profiles

Addresses are stored canonically as `PlcAddress { area: I | Q | M | PI | PQ, size: Bit | Byte | Word | DWord |
LWord, byte: u32, bit: Option<u8> }` and rendered by the configuration project's profile on demand (FR-PLC-007).
Switching a project from Siemens to IEC notation changes one setting, not 56 records.

| `AddrSpec` | Meaning | Prints | Class ([03](03-domain-model.md) §1.1) |
|---|---|---|---|
| `Auto` | module start + address-map offset (§3.1) | "Q0.1" | D |
| `Manual(a)` | deliberate value; every deviation from the derived value is reported (EW-PLC-012) | "Q0.1" | F |
| `Excluded` | "do not include in addressing" (EPLAN 20380): no address, skipped by allocation | "" | A |
| `Tbd` | typed open item; listed in open items and blocks release (EW-PLC-015, D-15) | "TBD" | A |

**Address profiles are versioned data** (FR-PLC-008). Built-in profiles ship with EW and are never removed, like
derivation profiles (NFR-REL-09). A project pins `id@version`. A user profile file is embedded in the project as a
library copy of kind `plc_profile` in `lib/plc-profiles/` ([05](05-project-format-and-persistence.md) §2.2), so the
project renders identically without the installation that created it (NFR-REL-11).

```toml
# built-in profile "siemens-tia-intl", version 2027.1
[profile]
id         = "siemens-tia-intl"
version    = "2027.1"
label      = "Siemens TIA (international)"
absolute   = true             # false for tag-based platforms (Rockwell, TwinCAT)
base       = 10
unit_bits  = 8
word_align = 2                # bytes; convention (unverified), reported as a warning only

[profile.mnemonics]
area = { I = "I", Q = "Q", M = "M", PI = "PI", PQ = "PQ" }
size = { bit = "", byte = "B", word = "W", dword = "D", lword = "L" }

[profile.render]
bit      = "{area}{byte}.{bit}"   # Q0.1
unit     = "{area}{size}{byte}"   # IW64
drawing  = ""                     # prefix on pages and reports: "Q0.1", as the reference set prints it
exchange = "%"                    # prefix in the TIA XLSX and Openness XML: "%Q0.1"

[profile.parse]
prefixes          = ["", "%"]
peripheral_suffix = ":P"          # accepted on import and reported; EW stores hardware addresses only

[profile.allocation]
scheme   = "packed"
start    = { digital = 0, analog = 0 }   # (unverified) TIA defaults; verified = false
verified = false
```

The IEC profile differs only in data: `bit = "%{area}X{byte}.{bit}"`, `size.bit = "X"`, `drawing = "%"`. The
grammar engine (render templates plus a tokenising parser driven by the mnemonic tables) is the same for every
absolute profile. Tag-based profiles render from the station and slot model instead.

| Profile | Release | Bit output | Word / analog | Addressing | Notes |
|---|---|---|---|---|---|
| `siemens-tia-intl` | MVP | Q0.1 (pages), %Q0.1 (exchange) | IW64 | absolute | Golden A profile |
| `iec-61131-3` | MVP | %QX0.1 | %IW64 | absolute | `%Q0.1` (size omitted) parses as a bit; renders with X |
| `rockwell-logix` | 1.0 | Local:3:O.Data.1 | Local:1:I.Ch0Data | tag-based | `<adapter>:<slot>:…` for remote I/O; member paths from part data (FR-PLC-009) |
| `beckhoff-twincat` | 1.x | `AT %Q*` | `AT %I*` | link-based | No absolute address; the I/O list shows terminal and channel |
| `siemens-de`, `step7-classic` | 1.x | A0.1 / Q4.0 | EW64 / PIW256 | absolute | Mnemonics; slot-based allocation (FR-PLC-010) |
| `mitsubishi`, `omron`, `schneider` | 1.x | Y1 / CIO 100.00 / %Q0.2.1 | D0 / D0 (data registers) / %IW0.3.0 | absolute or topological | Octal or hexadecimal base; word.bit; rack.slot.channel; all (unverified) |

*As built (cycle-38 fix round, decision log ID-884; FT-746).* The `iec-61131-3` profile reads a bit address without
its size as X: `%Q0.1` parses as `%QX0.1`, and because the prefix is optional `Q0.1` parses as the same bit. A unit
address must name its size (`%Q10` is no address), because the row above covers only the bit case.

### 2.6 PLC and bus part data

The PLC and bus sections of a part are level L2 data in [07](07-symbols-parts-macros.md) §2.5, owned by this
document (FR-PRT-010). They carry a confidence (`ItemMeta.confidence`: verified, unverified, test) and a source.
Findings computed from a non-verified section carry the "based on non-verified part data" marker.

```toml
# part 533361 VMPA1-FB-EMG-8: electronic sub-base with 4 valve positions
[plc]
role     = "module"
io       = [{ dir = "out", bits = 8, class = "digital" }]
channels = { count = 8, kind = "do", data_type = "Bool", designation = "{n}" }
bit_of   = "n"                      # channel map: channel n -> bit offset n (linear)
unbound  = "deactivated"            # state of channels neither bound nor claimed (§3.2)

[plc.positions]                     # solenoid-position map; position index p = 0..3
count = 4
map   = [{ solenoid = "14", channel = "2p" }, { solenoid = "12", channel = "2p+1" }]   # hypothesis H1

[plc.meta]
confidence = "test"                 # ▲U-23 until the vendor manual or the pilot's PLC program confirms it (PQ-15)
```

```toml
# part 550208 CPX-M-GE-EV-S-7/8-5POL: interlinking block with 7/8" supply (folio 106)
[[plc.supply]]
group = "El"                        # electronics and sensor supply
pins  = { l_plus = "4", m = "2" }
[[plc.supply]]
group = "Val"                       # load supply of the solenoid outputs
pins  = { l_plus = "5", m = "1" }
feeds = "outputs"
[plc.earth]
pin  = "3"
role = "FE"                         # ▲U-18; D-16

# part 195740 CPX-FB13: PROFIBUS DP bus node
[[bus.ports]]                       # `ports`, a list, since cycle 40 (WP-S1.21 D15; was `port`)
interface = "DP"
bus       = "profibus_dp"               # `BusSystem` (WP-S1.21 D11, D15)
role      = "slave"
connector = "sub-d-9"
signals   = { B = "3", A = "8", DGND = "5", VP = "6", SHIELD = "housing" }   # (unverified)
```

The port's `connector` and `signals` are not typed: a reader keeps them in the port's `extra` until FT-829 types them.

The valve part 571334 declares `solenoids = ["14"]` in its fluid section ([11](11-fluid-power.md)).

- **Map language.** `bit_of` and `channel` accept `"<k>n[+<c>]"` (`"n"`, `"16n"`), `"<k>p[+<c>]"` (`"2p"`,
  `"2p+1"`) or an explicit list (`[1, 3, 5, 7]`). Analog channels use `"16n"` with `size = word`. The part
  linter rejects anything else. There is no scripting: maps are data, deterministic and safe to merge.
  **Grammar** (normative; decision log ID-523). An expression is

  ```text
  expr   = [factor] var ["+" const]
  var    = "n" | "p"
  factor = decimal            ; at least 1
  const  = decimal
  decimal = "0" | nonzero-digit *digit      ; no sign, no leading zero, no space
  ```

  and its value is `factor × var + const` in unsigned 32-bit arithmetic, `factor` defaulting to 1 and `const` to 0;
  an overflow is an error, never a wrap. There is **no subtraction** and no other operator: `"n-1"`, `"-n"`, `"n*2"`,
  `"2 n"` and `"02n"` are invalid, so a part whose pins or bits count from below its channel numbers states them as an
  explicit list. The same expressions appear inside braces in a designation pattern (`ChannelSpec.designation`,
  `pin_of`, §2.4), where only `n` is allowed: `"X2:{n+1}"` is valid and `"X2:{n-1}"` is not. Every reader parses an
  expression with `ew_plcaddr::MapExpr`, and a designation pattern with the one expander built on it,
  `ew_derive::expand_pattern` (FT-318); what a reader does with an invalid one is its rule
  (no address, no designation with a derivation note, no pin with N3's notice), and the part linter's rejection at
  import is FT-351.
- **Supply groups** type the card-supply CPs (FR-PLC-024) and say which channels they feed. EW-PLC-013 and EW-PLC-014
  read them (§3.10).
- **The bus-port signal map** resolves per-core pins where a multi-line representation of the port exists. It
  answers [08](08-connections-terminals-cables.md) Open question 5 (FR-BUS-023).

**As built (cycle 13; decision log ID-333).** The part section is `sections.plc` (`PlcSection`): `role` (`station` or `module`), `io` (a list of `IoRange{dir, bits, class}`), `channels` (`ChannelSpec{count, kind, data_type, designation}`), `bit_of` and `unbound` (the state of channels neither bound nor claimed; absent: spare). `bit_of` is a `MapSpec`: `MapSpec::Expr` holds the expression text of the map language (`"n"`, `"16n"`, `"2p+1"`) and `MapSpec::List` an explicit list. The schema stores the text; `ew-plcaddr` (`MapExpr`) parses it and the derivation gives an unresolved address for a map it cannot parse. Not modelled yet, and kept in `extra`: the solenoid-position map `[plc.positions]` (typed in cycle 18, below), the supply groups, the earth pin, the bus ports and the `meta` confidence. Golden A has one `part_plc` row, part 533361: module, `out:8`, 8 `do` channels, `Bool`, designation `{n}`, `bit_of = "n"`, `unbound = deactivated`.

**As built (cycle 18; decision log ID-464, ID-465; format decisions under ADR-0011 amendments 1 and 4).** Two optional
members are appended to `PlcSection`, in this order, additive within the unreleased format 0.1 (`schemas/0.1`
re-blessed); a build from before keeps them in `extra`.

- **`positions`** (`PositionMap{count, map: [SolenoidChannel{solenoid, channel}]}`) is `[plc.positions]` above. The
  position index p is 0-based: the valve child at `Device.position` q sits at p = q − 1 ([11](11-fluid-power.md)
  §3.2), and p runs over 0 … `count` − 1. `channel` is a map-language expression in `p` (`"2p"`, `"2p+1"`) or a list
  indexed by p; an entry that does not resolve at p, or gives a number not below the channel count, assigns no
  channel. Golden A carries test input H1 in `tables/part_plc.csv` ([20](20-golden-project.md) §11.2).
- **`pin_of`** (`PinMap`) is the channel → pin map that §2.8 names as part data a device description lacks. It maps
  a channel number to a pin **designation**, never to a CP index, because a designation survives a reordering of the
  FnDef's CPs. Its persisted forms are a format contract for every reader:
  - a **string** is a *pattern*: each `{<expression>}` is a map-language expression in `n` (`{n}`, `{n+1}`,
    `{2n+1}`), every other character is literal; `"X2:{n+1}"` gives `X2:1` for channel 0. There is no escape for a
    literal brace;
  - an **array** is a list of designations, the entry at index n being the pin of channel n;
  - a **one-member object** is a form of a newer format, kept verbatim and expanded by no reader of this format.

  A pattern with an unclosed `{` or a stray `}`, an expression in `p` or one that does not parse, an overflow, a list
  index out of range or an empty result gives **no pin**: N3 then leaves the channel unbound with a notice (§3.3).
  The golden importer reads the column `pin_of` of `part_plc.csv` as a pattern and accepts a file without it; Golden
  A's part 533361 has no `pin_of`, because its channels are internal (the solenoid outputs).

The memoised query `solenoid_positions(module)` applies the module part's position map to its valve children: one
slot per position and map entry, with the channel, the valve (the lowest ID when two children hold the position) and
the valve's coil CP. Until N-FL-1, a valve declares solenoid s when one of its functions has a non-physical CP named s
([11](11-fluid-power.md) §3.1, decision log ID-469). The part-data confidence of `[plc.meta]` is FT-323.

**The bus section: specified, cycle 40; built by I40** ([WP-S1.21](../plan/specs/WP-S1.21-plc-and-bus-model.md) §8a,
D15 to D17; decision log ID-920; FT-770). A part's bus interfaces are `sections.bus`, typed within format 0.1
(ADR-0011 amendment 4, ID-334), appended to `PartSections` after `mounting_plate`:

```rust
pub struct BusSection  { pub ports: Vec<PartBusPort>, pub extra: Extra }     // `ports` omitted when empty
pub struct PartBusPort {
    pub interface: Ident,                // required: "DP", "X1"
    pub port_label: Option<Ident>,       // "P1R"
    pub bus: BusSystem,                  // required: §2.7's enum, newer values kept verbatim
    pub role: PortRole,                  // required: the role a port made from it takes
    pub termination: Option<OnOff>,      // present: the port has a termination switch, in this state
    pub extra: Extra,                    // `connector`, `signals`, an address range: 1.0, kept verbatim
}
```

- **What the part holds and what it does not.** The part states which interfaces exist, their bus, role and
  termination switch. The station address and the nets belong to the drawn port (§2.7), never to the part. The
  connector type, the signal map of the example above (FR-BUS-023) and the address range and supported rates
  (FR-BUS-009, the GSD facts of §2.8) are 1.0 data, kept in `extra` with the example's shape until WP-S1.24 types them
  (FT-829). Readers take the first port of an (`interface`, `port_label`) pair.
- **Part to function (N5, "bus ports from the part").** A part names no function or CP. A port is carried by a function
  of the device whose FnDef has contact kind `bus_port` (the DP port and the DP plug), and that function's CPs are the
  port's connection points. The normaliser N5 runs after N3 and before N4: when a device is new or its bus part (the
  first part use with a port) changes, or a bus-port function is added to or moved onto such a device, the part's ports
  that no function of the device carries (by `interface` and `port_label`) go, in list order, to its port-less bus-port
  functions in ID order, as `BusPort {interface, port_label, bus, role, termination}` with address `not_applicable` for
  a passive port and `tbd` otherwise and no nets. N5 never rewrites or removes a port: a carried port that differs in
  bus or role keeps its value with a notice ("-A1:DP keeps its bus port; part … declares profinet controller"), and a
  removed part leaves its ports. Its writes are in the command's preview and undo step. No EW-BUS rule reads the part
  port before 1.0; the disagreement check is FT-829.
- **Data.** The generic CPU EW-GEN-CPU-1 carries one port, `DP`, `profibus_dp`, `master` (`ew-starter-parts` 0.2.1);
  placing the DP port symbol on the CPU types its port. Golden A's parts carry no bus section; its ports are typed on
  its functions (§2.7).

### 2.7 Bus model

```rust
pub struct BusPort {                     // aspect on a Function (03 §14)
    pub interface: Ident,                // "DP", "X1"
    pub port_label: Option<Ident>,       // "P1R"
    pub bus: BusSystem,                  // ProfibusDp | Profinet | EtherCat | EthernetIp | IoLink | Asi | CanOpen | ModbusTcp
    pub role: PortRole,                  // Master | Slave | Controller | Device | Passive
    pub address: AddrValue<NetAddress>,  // Value | Tbd | NotApplicable
    pub termination: Option<OnOff>,      // connectors and devices with a termination switch (FR-BUS-004)
    pub logical_net: Option<Id<LogicalNet>>,
    pub physical_net: Option<Id<PhysicalNet>>,
}
pub enum NetAddress {
    Profibus(u8), Profinet { name: Ident, ip: Option<Ipv4Addr>, mask: Option<Ipv4Addr>, gateway: Option<Ipv4Addr> },
    EtherCat { position: Option<u16>, alias: Option<u16> }, EthernetIp { ip: Ipv4Addr, mask: Ipv4Addr },
    Asi { addr: u8, ab: Option<AsiAb> }, IoLinkPort(u8), CanOpen(u8), ModbusTcp { ip: Ipv4Addr, unit: u8 },
}
pub struct LogicalNet  { pub bus: BusSystem, pub name: Ident, pub master: Option<Endpoint>, pub rate: Option<BusRate>,
                         pub space: AddressSpace }                      // address space as in 03 §14
pub struct PhysicalNet { pub name: Ident, pub medium: Medium,           // added: medium (copper, fibre, …)
                         pub order_hint: Vec<Id<Function>> }            // added; segments are derived (§3.8)
```

- **Logical networks** carry addressing, rate and master (FR-BUS-002). Ports read the rate; they never copy it.
- **Physical networks** are named records. Their **segments are derived** from `Bus` connections, connector
  through-links and plug-on links (`bus_segments`, §3.8). `order_hint` orders members that are not drawn on any
  topology page. [03](03-domain-model.md) §14 derives them the same way (Open question 6): a stored member order
  would be a second copy of what the topology page already states.
- **Bus connectors** are devices of kind `BusConnector` with a port function (role Passive). That function has an
  In entry (A1, B1), an Out entry (A2, B2, optional CPs), a device side and `termination`. -3X1 mates with -201U1's
  DP socket through a `Bus` connection without a cable.
- **Bus cables** are ordinary cables ([08](08-connections-terminals-cables.md) §7.5) drawn single-line as one
  `Connection{kind: Bus(BusLine{cable})}` (FR-CBL-008).
- **External partners** are placeholder devices with `PlcStation{role: ExternalPartner}`, `BusNode`, a `BusPort`
  and an `ExternalRef` (project, location, device), but no parts. They are excluded from the BOM (FR-BUS-005). On
  folio 3, the external interruption point "DP" inside the External Connection box is the physical segment end, and
  its `ExternalRef` names JE-N1000-03 in +J-E-206.

### 2.8 Device types from device descriptions

A device description (DD) is parsed into a **device type** that enriches a part's PLC and bus sections. The DD file
itself is never embedded in a project or shipped with EW (NFR-LIC-06). Only the extracted facts are stored, with a
`DeviceDescRef { kind, file_name, sha256, entry, licence_note }`. The `entry` is the DAP ID, module ident, product
code with revision, or IODD device ID.

| Format | Bus | EW extracts | Release | Requirement | Parser |
|---|---|---|---|---|---|
| GSD | PROFIBUS DP | ident number, supported rates (`*_supp`), `Modular_Station`, `Max_Module`, maximum I/O lengths, module catalogue with config bytes | 1.0 | FR-PLC-025 | `gsd-parser` 0.6.0 vendored with a named owner, or own parser (C-9) |
| GSDML | PROFINET | DAPs, physical, fixed and allowed slots, modules and submodules with I/O data types, interface and port subslots (0x8000, 0x8001 …), order numbers, `DNS_CompatibleName`, PROFIsafe F-parameters | 1.0 | FR-PLC-026 | own, on quick-xml (no crate exists) |
| IODD 1.1 | IO-Link | identity, process data in/out as a struct type, bitrate, SIO support, connector type, wire functions and colours | 1.0 | FR-BUS-013 | own subset parser (`ioddengine` in tests only, C-9) |
| ESI | EtherCAT | product code, revision, PDO entries as channels, `EBusCurrent` | 1.x | FR-PLC-027 | `taktora-ethercat-esi` vendored, or own |
| EDS (CIP) | EtherNet/IP | identity, assemblies, connections, RPI ranges | 1.x | FR-PLC-027 | own INI parser |

**What DDs lack and part data adds:** pin designations, the channel → pin map, the address map, electrical data
and the macro. The rack/slot editor (§6) combines both.

### 2.9 Safety data

```rust
pub struct SafetyModule  { pub f_dest: Option<u16>, pub f_source: Option<u16>, pub f_wd_ms: Option<u16>,
                           pub range: Option<RangeU16> }        // range from GSDML, e.g. 1..=65534
pub struct SafetyChannel { pub evaluation: Evaluation,          // OneOoOne | OneOoTwo
                           pub pair: Option<Id<Channel>>,
                           pub requirement: Option<Ident>,      // "SIL 2", "PL d": a field, never calculated
                           pub test_pulse: Option<Endpoint> }   // the module's sensor-supply pin
```

Safety channels render in a distinct style (hatched channel field plus an "F" marker, never colour alone,
FR-UI-025) in overviews and lists. EW-SAF-001 … EW-SAF-003 check them (§3.10). SIL/PL calculation and F-program
generation are out of scope (FR-PLC-041; [vision](00-vision-and-scope.md) §4: no PLC programming).

### 2.10 Derived queries

All queries live in `ew-derive` and follow the granularity rule: the finest memo key is a module, station,
configuration project or network (ARCH §6.3).

| Query (key) | Returns | Depends on | Initial policy |
|---|---|---|---|
| `module_order(cfg)` | modules in allocation order | stations (`order`, ID), slot paths, `Device.position` | Memo |
| `module_starts(cfg)` | start per module and direction | `module_order`, part I/O widths, `IoStarts`, profile | Memo; early cut-off per module |
| `channel_addresses(module)` (ARCH §6.4) | rendered and canonical address per channel | its `module_starts` entry, part maps, `Channel.address` | Memo |
| `channel_states(module)` | effective state per channel | bindings, position map, valve children, device states | Memo |
| `channel_targets(module)` | target function, passed terminals, ambiguity | signal partition, bindings, `Channel.target`, `DeviceLink` | Memo (traced from 1.0) |
| `channel_texts(module)` | determined designation, symbol, function text, comment | targets, templates, device texts | Memo |
| `address_index(cfg)` | sorted bit intervals per area | `channel_addresses` of every module | Memo |
| `plc_rows(station)` (ARCH §6.4) | report rows | all of the above | Memo |
| `bus_segments(pnet)` | ordered member chains with ends | `Bus` connections, connector links, `order_hint` | Memo |
| `net_members(lnet)` | ports, stations, master | `BusPort.logical_net` | Memo |
| `exchange_image(cfg)` | neutral exchange structure (§4.2) and its hash | all of the above | Memo, because the online rule EW-PLC-016 reads the hash; exporters read it on their fork |

`ew query channel_addresses -201U3 --explain` prints the dependency trail and provenance of each address
(ARCH §6.8).

**As built (cycle 13; decision log ID-333).** `ew-derive` implements `module_order(cfg)`, `module_starts(cfg)`, `channel_addresses(module)`, `channel_states(module)`, `channel_targets(module)`, `channel_texts(module)` and `plc_rows(station)` (the queries `ModuleOrder`, `ModuleStarts`, `ChannelAddresses`, `ChannelStates`, `ChannelTargets`, `ChannelTexts` and `PlcRows`), with algorithms A1 and A2 of §3.1, A3 of §3.2 (without step 4) and the direct targets of §3.5. Not built: `address_index`, `bus_segments`, `net_members` and `exchange_image`. A module reads its own channels by ID, but the per-module early cut-off of `module_starts` is not built: an edit re-runs `channel_addresses` for every module of the configuration project (FT-224; FR-PLC-012, AC-29). `plc_rows` feeds the `plc_overview` report kind (§5).

**As built (cycle 18; decision log ID-464).** One family joins: `solenoid_positions(module)` (`SolenoidPositions`,
Memo), the module part's position map applied to its valve children (§2.6), which `channel_states` reads for step 4
(§3.2) and EW-PLC-012 is to read (FT-192); `ew verify-derived` and AC-16 sweep it (66 families).

**As built (cycle 21; FT-319, FT-318).** Three families join, taking `ew verify-derived` and AC-16 to 69 families:
`channels_by_module()` (`ChannelsByModule`, Memo, incremental over the channel table: every channel by (module, number,
ID)), `module_channel_ids(module)` (`ModuleChannelIds`, Memo: one module's slice of it, the channels by number with the
lowest ID where two share a number, read as a cut-off point) and `channels_by_cp()` (`ChannelsByCp`, Memo, incremental:
every channel bound to a CP, by (function, CP index, ID)). A module reads its channels through its slice and then each
row by ID, so an edit of one channel re-runs the readers of its own module only. A designation or symbol edit changes
no entry of either index and cuts off; a **binding** edit changes an entry of `channels_by_cp` (not of
`channels_by_module`). Step 4 asks the CP → channel index whether a coil is bound and reads its whole value, so a binding
edit of any channel re-runs `channel_states` of every module whose position map claims a channel, and of no other module
(a module whose map claims nothing does not read the index); the counts are pinned by a test. A slice per CP would cut that
fan-out and is not built. In a transaction, N1, N2, N3, `plc.bind_channel` and `plc.set_module_part` read
the model's base indexes with read-your-writes (`TxBuilder::channels_of_module`, `channels_at_cp`: the generic
referrer index of the links `Channel.module` and `Channel.binding`), because a transaction has no memoising `Q` over
its pending state; no whole-table read of the channel table is left on a binding edit. The designation pattern has one
expander (FT-318, §2.4).

---

## 3. Behaviour and rules

### 3.1 Address derivation

**A1. Module starts (positional packed allocation).**

```rust
fn module_starts(q: &Q, cfg: Id<PlcConfigProject>) -> Arc<StartMap> {
    let prof = q.profile(cfg);
    let order = q.get::<ModuleOrder>(cfg);                // stations by (order, id); modules by slot path
    // Pass 1: every Manual range is occupied before any Auto range is placed.
    let mut used = AreaIntervals::from_manual(&order, |m, r| m.start.manual(r.dir).map(|a| prof.range(a, r)));
    let mut cursor = prof.default_starts();               // per (direction, class): digital, analog
    let mut out = StartMap::default();
    for m in order.iter() {
        for r in q.part_io(m) {                           // { dir, bits, class } from the module's part
            let at = prof.align(cursor[r.key()], r.bits);
            let units = r.bits.div_ceil(prof.unit_bits);
            let (start, next) = match m.start.get(r.dir) {
                AddrSpec::Manual(a) => (Start::At(a), at + units), // kept; reserved in pass 1
                AddrSpec::Excluded  => (Start::None,  at + units),
                AddrSpec::Tbd       => (Start::Tbd,   at + units),
                AddrSpec::Auto      => {                  // first free range at or after the cursor, per area
                    let s = used.first_free(r.area(), at, units, &prof);
                    used.insert(r.area(), s, units);
                    (Start::At(prof.byte_address(r.dir, s)), s + units)
                }
            };
            out.insert(m.id, r.dir, start);
            cursor[r.key()] = next;                       // advances for every module, whatever its mode
        }
    }
    Arc::new(out)
}
```

- **Stable cursor.** The cursor advances by each module's own width, whatever its start mode, so a Manual start
  outside the Auto path never moves another module. This makes FR-PLC-012's acceptance hold: changing the start of
  -201U5 to Manual Q10 re-addresses only its 8 channels, and the engine statistics show one `channel_addresses`
  re-execution, while the other six are backdated (ARCH §6.2).
- **Auto never overlaps.** Auto ranges skip occupied ranges of the same area: Manual ranges (pass 1) and ranges
  already placed for another class. The digital and analog cursors of a profile whose default starts coincide
  (`start = { digital = 0, analog = 0 }`, unverified TIA defaults) therefore cannot collide, and an inserted module
  after a PLC import does not land on a PLC-owned start (§3.9). A Manual start set *inside* the Auto path (for
  example -201U5 = Q4) makes the following Auto modules skip it (-201U7 → Q5, -201U8 → Q6, -202U2 → Q7); the
  re-address preview lists every such move. EW-PLC-001 therefore fires only where a Manual value is involved: two
  overlapping Manual starts, or a Manual channel address inside another channel's range.
- **Inserting or widening** a module shifts the following Auto modules, which the preview also lists (§3.9).

**A2. Channel addresses.** `Auto` gives `start(module, dir) + bit_of(number)`, converted into byte and bit by the
profile's `unit_bits`. `Manual(a)` gives `a`. `Excluded` gives nothing, and `Tbd` prints "TBD". For a tag-based
profile, `channel_addresses` yields `ModuleMember { module, member }`, rendered as `Local:3:O.Data.1` from the
station, slot and the part's member pattern.

| Golden A module | Slot | Output start | Channels | Change "-201U5 output start = Manual Q10" |
|---|---|---|---|---|
| -201U3, -201U4 | 1, 2 | Q0, Q1 | Q0.0 … Q1.7 | unchanged |
| -201U5 | 3 | Q2 | Q2.0 … Q2.7 | Q10.0 … Q10.7 (8 changes; the 4 bound coils on folio 204 reprint) |
| -201U6 … -201U8, -202U2 | 4–7 | Q3 … Q6 | Q3.0 … Q6.7 | unchanged |

**Renumbering never re-addresses.** Tags, folios and page order are not inputs of A1 or A2. Golden B1 and B2 leave
every channel address unchanged (FR-PLC-055).

### 3.2 Channel states (A3)

The effective state comes from `channel_states(module)`:

1. An explicit `Channel.state` (Spare, Reserved, Deactivated) wins.
2. `address = Tbd` gives `Tbd`.
3. A bound channel takes the state of its target device (FR-DEV-026): used → `Used`, spare → `Spare`, reserved →
   `Reserved`, not fitted → `Deactivated`.
4. An unbound channel that the position map *claims* gives `Reserved`, and EW-FL-002 reports the unbound solenoid.
   The map claims channel c if it assigns c to solenoid s of position p, a valve child at p declares s, and s is
   unbound or bound to c.
5. Any other unbound channel gives the part's `unbound` state: `Deactivated` for valve-terminal modules, `Spare` by
   default.

On Golden A, every coil is bound "as drawn" to channel 2p + 1. Under H1, channel 2p is assigned to coil 14, but
that coil is bound elsewhere, so the channel is not claimed. Under H2, channel 2p is assigned to coil 12, which no
monostable 571334 declares. **Both hypotheses therefore give 8 used, 20 spare and 28 deactivated channels**
([20](20-golden-project.md) §11.2). The PLC overview lists all 56. New I/O-list blocks default to
`state != Deactivated` and list 28 rows (FR-PLC-019, FR-PLC-023).

**As built (cycle 13; decision log ID-333; step 4 superseded by the cycle-18 paragraph).** `channel_states` applies steps 1, 2, 3 and 5. Step 4 (the position map claims a channel) is not modelled, because the part section has no solenoid-position map yet (FT-228, with EW-PLC-012 from FT-192). On Golden A step 4 changes nothing under either hypothesis, so the counts above (8 used, 20 spare, 28 deactivated) hold.

**As built (cycle 18; FT-228; decision log ID-464).** `channel_states` applies all five steps; step 4 reads
`solenoid_positions(module)`. "s is unbound" means that **no channel of any module** binds the coil CP, whatever that
channel's ID: the CP → channel index is read (before cycle 21 the whole channel table, as N2 and `plc.bind_channel`
read it), so a valve moved to another
module and a duplicate channel never make a channel `Reserved`. Only an unbound channel reaches step 4, so the case
"bound to c" cannot arise there. On Golden A under H1 no channel is claimed and the counts stay 8 used, 20 spare and
28 deactivated (tested under H1 and H2, memoised against cold). EW-FL-002, which step 4 names, and EW-PLC-012 are not
built (FT-192). Since cycle 21 step 4 reads the CP → channel index instead of scanning the channel table per module
(FT-319).

### 3.3 Binding and normalisers

The normalisers run in the commit pipeline (ARCH §5.2); [06](06-schematic-editor.md) §3.2 holds the registry and
their order.

| Normaliser (ARCH §5.2) | Trigger | Effect |
|---|---|---|
| N1 channel instantiation | `PlcModule` added, or the module's part changed | Creates channels 0 … n−1 with v5 IDs from the part's `channels` and gives existing ones the part's channel kind. Surplus channels are removed only after a confirmation that lists their bindings, symbols and exchange records (FR-PLC-053): the preview of the Bulk command `plc.set_module_part` (§3.9); N1 itself never removes a channel and names the surplus in a notice |
| N2 solenoid binding | a valve child added to, moved within or re-parted on a module that has a position map; a coil function added to such a valve (by N-FL-1 of [11](11-fluid-power.md) after a part change); or a channel of that module created in the same transaction | Binds each declared solenoid's coil CP to the channel the map assigns, if that channel is unbound **and no channel binds the coil**; otherwise the coil stays as it is, with a notice (an unbound coil is reported by EW-FL-002, a binding that differs from the map by EW-PLC-012). An existing binding wins: an explicit binding in the command (QET import "as drawn", A7; a macro value set of type `ChannelBinding`, [07](07-symbols-parts-macros.md) §2.7), and the earlier binding of a moved valve's coil. Pairs whose valve and channel the transaction did not touch are never re-examined; a rebinding is the explicit `plc.bind_channel` (FR-PLC-051, as reworded in cycle 18). This is the single implementation, in `ew-commands`; N-FL-3 in [11](11-fluid-power.md) §4.2 is a reference to it |
| N3 module pin binding | a channel created on a module with physical pins | `binding` = the module CP whose designation the part's channel → pin map (`pin_of`, §2.6) gives, on the module's main function, if no other channel binds it (FR-PLC-005); otherwise the channel stays unbound with a notice |
| N4 logical-net inheritance | a `Bus` connection joins a port without a logical net to one that has one | Assigns that net; conflicting nets are left as they are and reported (EW-BUS-010, FR-BUS-024) |

**Bindings point at CPs, never at copies.** For a physical channel, `binding` is the module pin. For an internal
channel (valve-terminal solenoid outputs, IO-Link process data), `binding` is the served coil's internal CP
(`CpDef.physical = false`) directly. No connection record exists, so wire lists, terminal diagrams and wire-length
calculation contain nothing to exclude, while the PLC overview, the I/O list and target tracking read the binding
(FR-PLC-006). `Internal` connections remain for other device-internal links between CPs of one part's functions
([08](08-connections-terminals-cables.md) §2.3).

**Drag-bind** (FR-PLC-020). Dropping channel 1 of -201U3 from the PLC navigator onto the coil of -201U3-Q1 on folio
203 runs `plc.bind_channel{channel, endpoint}`. It clears a previous binding of the same CP in the same transaction
(FR-PLC-005) and is one undo step.

**A7. Imported address texts** (QET pass P7 of [14](14-interop.md) §3.2; FR-IMP-017; MF-19). The QET importer
recognises "Q0.1" next to a coil as a *pending binding* `{cp, address}`; it never writes an address. Once the
module exists (Golden C fix-script step 6), `ApplyImport` resolves each pending binding, parsed with the
configuration project's profile (prefixes "" and "%"):
1. **Derived match.** The coil's module (the valve's parent `PlcModule`) has exactly one unbound channel whose
   derived `Auto` address equals the text: bind the coil to it and keep `Auto`. This is the "as drawn" binding;
   on Golden C all 28 texts resolve this way to channels 2p + 1, and EW-PLC-012 then reports EF-02 under H1.
2. **No derived match.** Bind the coil to the channel the position map assigns, set that channel's address to
   `Manual(text)` so the print is preserved, and let EW-PLC-012 (discriminator `manual`) report the deviation.
3. **Ambiguous or no module.** The text goes to the import review list (FR-IMP-017 uncertain matches); nothing is
   bound.

**As built (cycle 13; decision log ID-333; superseded for N1 by the cycle-16 paragraph below).** The channel-creating normaliser N1 was not built: channels came only from import (the golden importer reads `channels.csv`, §2.4), and a change of a module's part created or removed none. `Channel.binding` is stored as an `Endpoint` (a CP of the module or of the served coil); the golden project binds channel 2p + 1 to coil 14 of valve position p + 1, as drawn.

**As built (cycle 16; FT-270).** N1 is built in `ew-commands` (registry row 3 of [06](06-schematic-editor.md) §3.2):
adding the `plc_module` aspect (`fields.set`), creating a device with it, or changing its parts or `part_use`
creates the missing channels of the part's channel spec with their v5 IDs and the spec's kind; surplus channels
stay. `plc.bind_channel` and `plc.config_project.create` are built (§3.9; the latter has no row there: it creates
the configuration project that `ew new` gives a new project). N2 (the solenoid-position map) and N3 are not built:
the map is not typed part data yet (FT-228), so the golden builder binds the 28 coils with explicit `plc.bind_channel`
commands, which win over the map by definition (FT-275). `plc.bind_channel` refuses a CP outside the channel's module
and its descendants and unbinds another channel on the same CP (FR-PLC-005). N1 keys existing channels on (module,
number), keeps their authored members, applies no kind change (FT-276) and removes no surplus channel until
FR-PLC-053's confirmation exists (FT-277). The channel members are not yet settable by `fields.set` (FT-226).
Decision log ID-395, ID-399.

**As built (cycle 18; FT-275 to FT-277; decision log ID-466 to ID-468).** Rows 3, 13 and 14 of the registry run;
normaliser notices are merged once into the command's outcome.
- **N1** (`channel_instantiation`) picks the module's PLC section by the derivation's rule (`plc_module.part_use`,
  else the first part use whose part has a PLC section), creates the missing channels, and gives an existing channel
  0 … count − 1 the part's channel kind with a notice, keeping its binding; a kind of a newer format is kept with a
  notice. Surplus channels stay and are named in a notice; the derivation reads only the channels below the count.
- **N2** (`solenoid_binding`): for the valve at position q (p = q − 1; of two children at one position the lower ID)
  and each map entry, the channel `solenoid_channel(entry, p)` below the channel count and the coil `coil_cp_idx`
  over the valve's functions in ID order, the same evaluation as `solenoid_positions`. A pair is considered when its
  valve is new in the transaction, moved (parent or position), re-parted or gained a function, or when its channel is
  new in the transaction (N1 created the channels after the valves existed). A moved valve whose coil stays bound
  elsewhere gets a notice. Ties break by ID; modules run in ID order.
- **N3** (`module_pin_binding`): a channel new in the transaction, unbound, on a module whose part has `pin_of`, takes
  the CP of the main function (the lowest ID among the device's functions with role main) whose designation
  `channel_pin(pin_of, number)` gives, searched in the part's CP pattern when it has one, else in the main function's
  FnDef copy, the lowest index first. Channels that existed before the transaction are never touched.
- **FR-PLC-051 as built** (decision log ID-467; the project owner (delegated) accepted the deviation and may reverse
  it): a move keeps an earlier binding, and untouched pairs are never re-examined, so an explicit unbinding stays; a
  rebinding is `plc.bind_channel`. FR-PLC-051 in [02b](02b-requirements-extended.md) is reworded to this.
- **FR-PLC-053** is met through the Bulk command `plc.set_module_part{module, parts?}` (§3.9): it sets the part uses
  and deletes the channels at or above the new count with their bindings, symbols and comments; its preview lists
  them, so committing it is the confirmation (ADR-0008). A part change of a module in the UI goes through this
  command; `fields.set` on `parts` stays the scripting form that never removes a channel.

The golden builder keeps its 28 explicit `plc.bind_channel` commands: under H1 the map assigns coil 14 of position p
to channel 2p, while the drawing binds it to 2p + 1, and an explicit binding wins. Not built: the confidence marker
(FT-323), the exchange records in the surplus preview (FT-324), the unbinding when a valve leaves its module (FT-320),
N3 after a part change (FT-321) and N-FL-1 (FT-322).

### 3.4 Symbols, designations and function texts

**Templates** (1.0, FR-PLC-017) are strings over these tokens: `{target.process_tag}`, `{target.dt}` (the derived
tag without the product sign and suppressed identifiers), `{target.function_text}`, `{pin}`, `{module.dt}`,
`{channel.number}` and `{address}`. `a|b` falls back to `b` when `a` is empty. The result passes through
`ident()`, which in strict IEC mode (FR-PLC-047):
- replaces characters outside `[A-Za-z0-9_]` with `_`;
- collapses runs of underscores;
- strips leading and trailing underscores;
- prefixes `_` when the result starts with a digit. IEC 61131-3 allows a leading underscore followed by a digit
  (verify against Ed. 4); FR-PLC-017's example is `_201U3_Q1_14`.

| Template | Target | Result |
|---|---|---|
| `{target.process_tag\|target.dt}` | -201U3-Q1, which drives -203Q1 (TA05-V-15) | `TA05_V_15` |
| same | spare valve -201U5-Q1 (no process tag) | `_201U5_Q1`, unless the template is `{target.process_tag}` alone, which gives no symbol (Golden A at 1.0) |
| `{target.dt}_{pin}` | -201U3-Q1:14 | `_201U3_Q1_14`; after renumbering -201U3 to -201U4, `_201U4_Q1_14` |

A manual symbol always wins and never follows renumbering. Uniqueness (EW-PLC-007) is tested on the determined,
cleaned symbol, so "TA05-V-15" and "TA05_V_15" on two channels collide.

**Function texts** are never typed at the channel. The function-text column of the PLC overview and the I/O list
prints the target's function text, else the tracked process tag of the device the target drives
(`DeviceLink{Drives}`, [11](11-fluid-power.md) §3.1, §3.3). Used valves carry no function text of their own, so
-201U3-Q1 prints TA05-V-15 from -203Q1; "Spare" stays a manual function text of the spare valves. Reports keep
function text and process tag as separate columns. Editing -203Q1's process tag, or a valve's manual function
text on folio 203, changes folio 908, the I/O list and the AML comment in the same commit (FR-PLC-022).

### 3.5 Target tracking (A4)

```rust
fn channel_targets(q: &Q, module: Id<Device>) -> Arc<TargetMap> {
    let mut out = TargetMap::default();
    for ch in q.channels_of(module) {
        if let Some(f) = ch.target { out.emit(ch, f, Source::Manual, []); continue; }
        let Some(cp) = ch.binding else { out.none(ch); continue; };
        let f = q.cp_function(cp);
        if f.device != Some(module) { out.emit(ch, f.id, Source::Direct, []); continue; } // coil of -201U3-Q1
        let sig = q.signal_of(cp);                 // nets joined by wires, cores, IP pairs, terminal levels, plug pins
        let cands = sig.cps().filter(|c| is_field_function(q, c) && !is_plc_cp(q, c) && !same_device(q, c, module));
        let Some(best) = cands.min_by_key(|c| (category_rank(q, c), page_order(q, c), cell(q, c), c.id)) // deterministic
            else { out.none(ch); continue; };
        out.emit(ch, best.function, Source::Traced, sig.path(cp, best).terminals());     // "-X1031:5"
    }
    Arc::new(out)
}
```

- **Within the signal only.** Tracking follows the signal level of [03](03-domain-model.md) §9.3: wires, cores,
  interruption-point pairs, terminal levels and plug pins. It never crosses bridges, which join potentials, so a DI
  never "finds" every load on a common 24 V rail. FR-PLC-018 says the same: through connections, terminals,
  terminal levels and plug pins.
- **Field functions** are sensors, actuators, coils, lamps, contacts and valve solenoids. Terminals, plugs, cables
  and PLC CPs are passed through, never chosen.
- **Ambiguity** (several candidates) picks deterministically by category rank (coil and actuator before contact),
  then page order, cell and ID. The row is flagged "ambiguous" in the I/O list and in the "why this value?" view
  (FR-UI-011). A manual `target` resolves it (FR-PLC-054).
- **Process tags** follow one `DeviceLink{Drives}` hop: the valve -201U3-Q1 drives the field device -203Q1
  (+FIELD, ▲U-07), so `target.process_tag` of channel 1 is TA05-V-15 (FR-DEV-012, FR-DEV-028).
- **Address at terminals** (FR-PLC-018 option, EPLAN 20854/20855): with the option set, the text slot `PlcAddress`
  of a terminal on a traced path shows the channel address.

At MVP only `Direct` and `Manual` targets exist. This covers every channel of the golden project, because all of
them are internal valve-coil channels.

### 3.6 Representations

| Representation | Page type ([06](06-schematic-editor.md) §2.2) | Shows | Joins nets | Cross-reference |
|---|---|---|---|---|
| Module box with channel pins | MultiLine | pin designation and the address, symbol and function-text slots per channel | yes | main representation of each channel |
| Coil on a fluid valve | Fluid (203–206) | address and function text slots of the valve | no wire (internal CP) | target of the PLC-overview xref |
| PLC overview box (FR-PLC-052) | Overview | every channel of a module or station: number, address, symbol, function text, state | no (`Overview` connections) | per channel to its main representation (EPLAN 20301) |
| Station bus port | BusTopology (3) | DT, order code 1, part type, "Node {address}" | bus nets | aux → main "(201-E1)" |
| Card power connector | MultiLine (106) | pins 1–5 and FE | yes | aux → main "(201-E1)" |
| Assembly front view | AssemblyOverview (201–202) | modules and valves in manifold order | no | "∞ 203-B4" (FR-XRF-016) |
| PLC overview report | Report (908) | 56 rows, a link per row | — | link annotation to the coil's anchor |

- **One main representation per channel:** the representation that draws the binding CP. A valve representation
  draws two functions of one device, the 5/2 fluid function and the coil-14 function, so the coil's main
  representation is the valve symbol on 203.
- **The `PlcAddress` text slot** ([07](07-symbols-parts-macros.md) §2.3) resolves:
  - on a bound CP's function, to that channel's address;
  - on a fluid function whose device has bound coils, to the coil addresses in solenoid order 14, 12 ("Q0.0 /
    Q0.1" on a bistable valve, as in [11](11-fluid-power.md) §3.1);
  - on a module pin, to its channel's address.
- **Placing a channel symbol binds it**; it never copies properties (PBF §2.7, rule 2). Every representation
  renders the same determined values.

*As built (cycles 37 and 38; decision log ID-860, ID-874, ID-876).* The `PlcAddress` slot prints as above; Golden A's embedded valve, lamp and position-switch symbols carry it since FT-698, so folios 203 to 206 print Q0.1 … Q6.7. A black box's pin text is the function CP's designation as stored and, for a CP bound to a channel, the printed address after one space ("1 I0.0"); a designation the symbol's `cp_designation` slot prints is left out. Pin texts never move; a cross-reference text that would overlap one drops below it (FT-707; folios 102 and 106 of Golden A).

### 3.7 Valve terminals

The valve terminal is **one device tree in three disciplines** (ARCH §4.3, REF-098):

| Device | Parts | Aspects and functions | Drawn on |
|---|---|---|---|
| -201U1 | 195740 + 550208 | `PlcStation` (role ValveTerminal), `BusNode`, `Assembly` (order codes 1 and 2), `ValveTerminal` (pilot supply, [11](11-fluid-power.md)); main function = assembly view; aux functions = DP bus port, card power connector | 201 (main), 3, 106 |
| -201U2, -201U9 | 552286, 533354 | children, no I/O | 201 |
| -201U3 … -201U8, -202U2 | 533361 | children with `PlcModule` (positional slots 1–7), 8 channels each | 201, 202 (overview), 203–206 (aux label "(201-E4)") |
| -202U1, -202U3 | 550208, 533354 | children, no I/O; -202U1 carries the second power connector | 202, 106 |
| -201U3-Q1 … -202U2-Q4 | 571334 | 28 valve children: fluid function (ports 1–5) + coil-14 function with internal CP | 203–206 |

- **Spare positions** are real valves in state spare (U-36) whose coils are bound. Their channels are spare. A
  blanked position (no valve) gives deactivated channels.
- **Other vendors** use the same model: SMC EX600 SI units, Festo CPX-AP and VTUX, Bürkert AirLINE, Emerson AES. Only
  their part data differs (PBF §8.2). EW ships the Festo CPX/MPA part data of the golden project as test data.
- **Order codes** (51E-F13GCQP-D+H2GG, 32P-VGL-R-MAHAHAHA…-28MS) are stored verbatim on the assembly. Children are
  built by hand or in the rack/slot editor at 1.0. Configurator decoders that expand an order code into children are
  1.x plugins (IDEA-026). Slot-occupancy validation (EW-FL-003, D-40) is in [11](11-fluid-power.md).

### 3.8 Bus topology, addresses and termination

**A5. Segment derivation.**
1. Build a graph whose nodes are bus-discipline CPs: port functions and connector In, Out and device-side entries.
2. Its edges are `Bus` connections (with or without a cable), each connector's internal through-link and plug-on
   links.
3. Each connected component within one `PhysicalNet` is a segment. For line buses (PROFIBUS, RS-485) the component
   must be a path. A branch is reported by EW-BUS-006 as a stub.
4. Ends are degree-1 nodes. An external interruption point counts as an end.
5. Member order walks from the master end: the end whose port is the logical net's master, or the external end
   whose `ExternalRef` names the master; failing that, from the end with the smallest (`order_hint` position, ID).

**Termination (EW-BUS-004).** At each end, the termination state of the nearest terminating element (connector port
or device port with a switch) must be On, and every other terminating element on the segment must be Off. An
external end whose state is unknown gives one info (FR-BUS-016). On Golden A the segment is external end
(JE-N1000-03, master) · -3W1 · -3X1 (On) · -201U1:DP, the order of FR-BUS-003 and [03](03-domain-model.md) §14. It
gives no error and one info (EF-16). Setting -3X1 Off gives an error, and its rendered
text changes from "Switch to "ON"" to "Switch to "OFF"" because the text is a property arrangement over
`termination` (FR-BUS-004).

**Addresses.**
- `AddrValue::Tbd` renders "TBD", appears in the open-items list and blocks release (EW-BUS-001, FR-CHK-034).
- `NotApplicable` is for ports without addresses (IO-Link device side).
- Ranges, segment lengths and PROFINET name rules are data with a `verified` flag. Unverified constants report at
  most a warning (FR-CHK-046; NFR-COMP-05).

| Bus | Release | Address data | Main checks |
|---|---|---|---|
| PROFIBUS DP | MVP (model, address, termination); 1.0 (rates from GSD) | station address 0–125 (unverified), rate from the logical net (9.6 kbit/s … 12 Mbit/s) | EW-BUS-001, -002, -004, -005, -006 |
| PROFINET IO | 1.0 | device name, IPv4, mask, gateway, P1/P2 neighbours, MRP role | EW-BUS-001, -003, -007 |
| IO-Link | 1.0 | port number; mode IO-Link / DI / DQ / deactivated; class A or B | EW-IOL-001, -002 |
| EtherCAT | 1.0 structure; 1.x E-bus budget | position after the coupler, alias; rail order from layout at 1.0 ([12](12-panel-layout.md)) | EW-BUS-008 (1.x) |
| EtherNet/IP, AS-Interface | 1.x | IP, mask, RPI, DLR; 1–31, 1A–31B | EW-BUS-001, -002 |
| CANopen, Modbus | Later | node ID 1–127; IP + unit ID | EW-BUS-001 |

**IO-Link ports (1.0, FR-BUS-012).** A master port is a channel of kind `MultiFunction{mode}` with modes IO-Link,
DI, DQ and deactivated. The master's part declares the port window (fixed bytes per port, or a configurable maximum
such as 32 bytes ▲). In IO-Link mode the connected device's IODD supplies the process-data lengths and a struct
data type: the Balluff sample's 88 input bits give an 11-byte input window and a struct-typed symbol. The module's
I/O width is the sum of its port windows in their current modes, so a mode change re-runs A1 for the following
Auto modules (§8, item 10). EW-IOL-002 compares the IODD lengths with the window.

**As built (cycle 13; decision log ID-333, ID-336; FT-105).** The `BusPort`, `LogicalNet` and `PhysicalNet` kinds and `bus_segments` are not built (FT-225), so Golden A models the DP line as an interim. A1 of the plug's In entry and A of the DP port are the representative CPs of the single-line bus line. `DP.1` (page 3, cable -3W1) is the whole cable from the external end to -3X1 In. `DP.2` is the plug-on mating, a page-less `Bus` connection without a cable (§2.7). The DP potential is defined at -201U1:A with circuit class `bus` and no voltage; the pages of a bus potential come from its `Bus` connections, because a bus line joins nets but is no wire. Its colour and size come from the cable (`BusLine.cable`), so the colour/size cell of folio 909 prints "(cable)". Folio 909's DP row has 3 members where doc 20 prints 4: the cable is an edge of the segment graph, not a member (A5 above), which is a model limit until FT-225. The station address "TBD" waits for `BusPort` as well (§5).

### 3.9 Commands, freezing and ownership

| Command | Effect | Cost | Release |
|---|---|---|---|
| `plc.set_start{modules, dir, spec}` | sets Auto, Manual, Excluded or Tbd starts | Interactive | MVP |
| `plc.readdress{scope}` | recomputes starts for a selection or a configuration project; previews old → new address, folios and symbol changes; one transaction (FR-PLC-014) | Bulk (preview) | MVP Should |
| `plc.convert_manual_to_auto{scope}` | drops manual values that equal the derived value; lists the others | Bulk | MVP Should |
| `plc.freeze{cfg}` | sets every derived channel address to `Manual(current)` (FR-PLC-015) | Bulk | 1.0 |
| `plc.unlock{scope, group}` | lifts PLC ownership protection for a field group | Interactive | 1.0 |
| `plc.bind_channel{channel, endpoint}` | binds or rebinds (§3.3) | Interactive | MVP |
| `plc.set_module_part{module, parts?}` | changes a module's part uses and removes the channels the new part no longer has, with their bindings; the preview is FR-PLC-053's confirmation (§3.3; built in cycle 18) | Bulk (preview) | MVP |
| `plc.set_profile{cfg, profile}` | changes rendering only; the files differ in one setting (FR-PLC-007) | Interactive | MVP |
| `plc.apply_exchange{cfg, image, resolutions}` | applies an AML or XLSX import with its snapshot (§4.4) | Bulk (preview) | 1.0 |
| `plc.apply_io_list{rows, resolutions}` | applies a CSV/XLSX I/O-list import (§4.7) | Bulk (preview) | MVP Should |

**Re-address preview.** With Auto starts, inserting a module between -201U4 and -201U5 previews 40 address changes
(5 modules × 8 channels), 20 of them on bound coils on folios 204–206. Undo restores everything in one step.

**Protection.** After the first applied PLC import, the field groups owned by `Plc` (§2.2) are protected: module
starts, firmware, IP addresses, PROFINET names and DP station addresses. An edit of a protected field is rejected in
strict mode with the message "address owned by PLC1 (TIA import of <date>)" and an "Unlock" quick fix. In lenient
mode it is reported. Inserting the same module after an import moves nothing: the five later starts are
`Manual` (PLC-owned), so A1 gives the new module the first free byte, Q7, and the preview lists only its 8 channels.

*As built (cycle 38, WP-S1.21 part 3, decision log ID-870).* `plc.set_start` and `plc.set_profile` are built (Interactive; the IEC profile prints `%QX0.1`); a start edit re-derives only its own module's channel addresses (`ModuleStartOf`). `plc.readdress` and `plc.convert_manual_to_auto` are not built: both need A1 over the whole configuration project inside one transaction, which ADR-0008 amendment 4 forbids (FT-725). `plc.bind_channel` also takes tags in a command log: a channel as `<module tag>#<number>` or `<module tag>:<designation>`, a function as `<tag>` or `<tag>:<slot>` (FT-721).

*As built (cycle-38 fix round, decision log ID-884; FT-750).* `plc.bind_channel` checks the CP against the CPs the function draws, by the one accessor the commands share with the join (`common::function_cp_indices`): the template entry's pattern, else a black box's part pattern, else the FnDef. A black-box card's pins therefore bind; before, the check read the generic black-box FnDef, which defines no CPs, and refused every pin (found by the play-test brief `brief-plc-io`, ID-882).

### 3.10 Check rules

The EW-PLC pack ships at MVP with the rules marked MVP and is complete at 1.0 (FR-CHK-005). Rules are registered
Rust functions unless noted. Scopes are `checks(scope)` keys (ARCH §12). EW-PLC-001, -005, -007, -015 and -016 use the
configuration-project scope `cfg`, which [15](15-checks-revisions-collaboration.md) §2.3 lists at this document's
request. Pack membership and the MVP subset match [15](15-checks-revisions-collaboration.md) §4.1 and §4.5.

| Rule | Checks | Scope | Modes | Severity | Release | Requirement | Golden A |
|---|---|---|---|---|---|---|---|
| EW-PLC-001 | Duplicate address as bit-range overlap in configuration project and area (IW0 vs I1.3); word alignment warning | cfg | online, prevent (strict) | error | MVP | FR-PLC-044 | 0 |
| EW-PLC-002 | Address outside the module's I/O range | module | online | error | MVP | FR-PLC-044 | 0 |
| EW-PLC-003 | Bit index outside the profile's unit width | module | online | error | MVP | FR-PLC-044 | 0 |
| EW-PLC-004 | Channel kind ≠ connected function (DO to a proximity sensor) | module | offline | warning | 1.0 | FR-PLC-046 | 0 |
| EW-PLC-005 | Symbol not a valid identifier for the target profile | cfg | export, release | error | 1.0 | FR-PLC-047 | 0 |
| EW-PLC-006 | *Retired.* Spare channels are listed by reports and counts (FR-PLC-019), not as findings | — | — | — | — | — | — |
| EW-PLC-007 | Duplicate determined symbol in a configuration project | cfg | online, prevent | error | MVP | FR-PLC-016 | 0 |
| EW-PLC-008 | Used channel without address | module | online (warning), export (error) | warning | MVP | FR-PLC-044 | 0 |
| EW-PLC-009 | Channel with address or symbol but no connection, not spare | module | online | warning | 1.0 | FR-PLC-046 | 0 |
| EW-PLC-010 | Data-type width ≠ channel width (Bool on a 16-bit AI) | module | online | error | 1.0 | FR-PLC-046 | 0 |
| EW-PLC-011 | Module in a disallowed slot; `Max_Module`; maximum I/O length | station | prevent, online | error | 1.0 | FR-PLC-028 | 0 |
| EW-PLC-012 | Binding or address ≠ part map (discriminators `binding` and `manual`); supersedes EW-NUM-002 (FR-CHK-037) for addresses | module | online | error | MVP | FR-PLC-045 | **28 under H1, 0 under H2 (EF-02)** |
| EW-PLC-013 | Card supply group unconnected or fed from a wrong potential type (FE on PE); supersedes EW-POT-003 (FR-CHK-030) for PLC supply CPs | station | online | warning | 1.0 | FR-PLC-048 | 2 (EF-22) |
| EW-PLC-014 | Summed output load (coil power from part data) > channel, group, module or feeding-channel rating | station | offline | warning | 1.0 | FR-PLC-048 | 0; evaluation lists MICO Out1 (EF-24, D-25) |
| EW-PLC-015 | `Tbd` module start or channel address | cfg | online (info), release (error) | error at release | MVP | FR-PLC-013 | 0 (the `Tbd` bus address is EW-BUS-001) |
| EW-PLC-016 | Exchange drift: the configuration project changed since the last exchange (§4.4) | cfg | online | info | 1.0 | FR-PLC-035 | 0 |
| EW-SAF-001 | F-destination address missing, duplicate in a logical net or in the project, or outside the DD range | project | online | error | 1.0 | FR-PLC-042 | — |
| EW-SAF-002 | 1oo2 pair incomplete or wired to different devices | module | offline | error | 1.0 | FR-PLC-043 | — |
| EW-SAF-003 | F-DI sensor not supplied from the module's test-pulse pin | module | offline | warning | 1.0 | FR-PLC-043 | — |
| EW-PRC-001 | Process tag does not match the scheme; duplicate on different devices | project | online | warning | 1.0 | FR-PLC-049 | 0 |
| EW-PRC-002 | Instrument-list tag without device or channel (open item) | project | online | info | 1.0 | FR-PLC-050 | — |
| EW-BUS-001 | Bus address missing (open item) or duplicate in a logical net | lnet | online, release | error | MVP | FR-BUS-015 | 1 (EF-03) |
| EW-BUS-002 | Address outside the bus range | lnet | online | warning until verified | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-003 | PROFINET name invalid or duplicate; IP duplicate or outside the subnet | lnet | online | error | 1.0 | FR-BUS-017 | — |
| EW-BUS-004 | Termination not On at exactly both segment ends | pnet | online | error; info for unknown external ends | MVP | FR-BUS-016 | 1 info (EF-16) |
| EW-BUS-005 | Rate not supported by a slave (GSD) | lnet | online | error | 1.0 | FR-BUS-009, -018 | 0 (rate unset ▲U-22) |
| EW-BUS-006 | Segment length, station count or stubs over the limit for the rate | pnet | offline | warning | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-007 | Logical net without master (an external partner counts) | lnet | online | warning | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-008 | EtherCAT E-bus current budget | station | online | error | 1.x | FR-BUS-020 | — |
| EW-BUS-009 | Bus port on a non-bus cable or the wrong connector type | pnet | online | warning | 1.0 | FR-BUS-018 | 0 |
| EW-BUS-010 | Ports joined by `Bus` connections but in different logical nets | pnet | online | warning | 1.0 | FR-BUS-024 | 0 |
| EW-IOL-001 | Port class mismatch; IODD device on a port not in IO-Link mode | module | online | error | 1.0 | FR-BUS-019 | — |
| EW-IOL-002 | Process data > port window; cable longer than the maximum | module | online | warning | 1.0 | FR-BUS-019 | — |

**EW-PLC-012 and the odd-bit question (D-26).** Folios 203–206 bind coil 14 of position p to channel 2p + 1. If the
VMPA1-FB-EMG-8 assigns the lower bit to solenoid 14 (H1, suspected but unverified), the printed Q0.1 … Q6.7
address unused coil-12 outputs: a functional bug, not a style choice. EW does not decide this in code. The
solenoid-position map of part 533361 is data with confidence `test`:
- **H1** gives 28 findings, one per binding;
- **H2** ("coil 14 on 2p + 1") gives none;
- switching between them is a part-data edit (AC-29).

Each finding names the bound channel, the channel the map expects, both addresses and the unverified-data marker.
It offers two quick fixes: "rebind to the map's channel" (a previewed command; folios 203–206 then print Q0.0,
Q0.2 …) and "open the part's PLC section". Setting the map's confidence to `verified` needs a source (vendor manual
reference or the pilot's PLC program, PQ-15).

**One defect, one finding.** Following the supersession table of [15](15-checks-revisions-collaboration.md) §3.6
(FR-CHK-045): EW-PLC-012 supersedes EW-NUM-002 (FR-CHK-037) for addresses, EW-PLC-013 supersedes EW-POT-003
(FR-CHK-030) at 1.0 (EF-10 → EF-22), EW-BUS-001 supersedes EW-CORE-044 (FR-CHK-034) for bus addresses, and
EW-PLC-015 supersedes EW-CORE-044 for module starts and channel addresses. EW-PLC-015 covers only module starts and
channel addresses, so it never reports the bus `Tbd`, and each `Tbd` yields exactly one finding. Every rule ID has
its documentation page (NFR-DOC-02).

*As built (cycle 38, WP-S1.21 part 3, decision log ID-870).* EW-PLC-001, -002, -003, -007, -008, -012 and -015 are built in the scopes `module` and `cfg`, with en-GB and nl-NL messages; EW-PLC-007 compares symbols exactly, and findings name a channel as `<full tag>:<designation>`. Their prevent half (EW-PLC-001, -007) is not built (FT-726). EW-PLC-012 offers "rebind to the map's channel" and carries the unverified-data marker; "open the part's PLC section" is FT-727; Golden A reports its 28 findings under H1 (EF-02). Two channels of one module with one number are reported by EW-CORE-008 `duplicate_number`, and the channel whose ID is v5 of its module and number wins (FT-350).

*As built (cycle-38 fix round, decision log ID-884; FT-747).* `address_index(cfg)` holds overlap groups, not pairs: each connected run of channel addresses in one area whose bit ranges overlap in a chain, once, so N channels on one address make one group of N. EW-PLC-001 reports one finding per group, with every member channel as a target and the first in address order as the subject; the text names the first five channels with their addresses and counts the rest (`addresses overlap in configuration project <cfg>: <channel> (<address>), …; and <N> more channels; give all but one of them another address`, the "and N more" clause only beyond five), in en-GB and nl-NL.

---

## 4. Exchange with PLC software and spreadsheets

### 4.1 Formats

| Format | Direction | Version | Release | Requirements | Implementation |
|---|---|---|---|---|---|
| AutomationML AR APC | export, import | CAEX 2.15 write; CAEX 2.15 and 3.0 read; AR APC 1.4.0 | 1.0 Must | FR-PLC-029, -031, -032, -033 | `ew-caex` (permissive) + `ew-plc` |
| AR APC 1.2.0 | export | for older TIA versions and EPLAN | 1.0 Should | FR-PLC-030 | same writer, version table |
| TIA "PLC Tags" XLSX | export, import | 10 fixed columns | 1.0 Must | FR-PLC-036 | dedicated exporter and importer in `ew-plc` (rust_xlsxwriter, calamine), not a machine profile (§4.5) |
| PLCopen XML GVL | export | TC6 v2.01 | 1.0 Must | FR-PLC-037 | quick-xml; `plcopen` crate in tests only (C-9) |
| Generic I/O list | export, import | CSV dialects, XLSX | MVP Should | FR-PLC-038, FR-EXP-002 … -004 | `ew-reports` row source |
| Instrument / I/O list | import | CSV, XLSX | 1.0 Should | FR-PLC-050 | `ew-plc` |
| TIA Selection Tool AML | import | AR APC 1.4.0 (TST 2025) | 1.x Should | FR-PLC-034 | `ew-caex` |
| Rockwell L5X, Beckhoff TE1120 | export | L5X fragment; TE1120 XML/AML | 1.x Could | FR-PLC-039 | `l5x` crate in tests only |
| TIA Openness bridge | outside the Rust core | C#, Windows only | Later Could | FR-PLC-040 | separate tool; the core has no .NET dependency |

Every exporter implements the permissive `Exporter` contract over an `ExportView` of a snapshot fork, never blocks
editing and records the revision it reflects (FR-EXP-001). Release-mode exports refuse a stale Class II input.

*Note (cycle 18).* No exchange record exists yet (the kinds `exchange_record` and `foreign_id` of §4.2 are reserved,
not typed). The confirmation of FR-PLC-053, the preview of `plc.set_module_part` (§3.3), therefore lists the surplus
channels with their bindings and symbols only; it lists their exchange records once §4.2 is built (FT-324).

### 4.2 Exchange image and identity

Parsers and writers work on a **neutral `ExchangeImage`** in the permissive crate `ew-caex`, so third parties can
reuse them without GPL code (ARCH §3):

```rust
pub struct ExchangeImage { pub project: XProject, pub stations: Vec<XStation>, pub tags: Vec<XTag>,
                           pub links: Vec<(XRef, XRef)>, pub networks: Vec<XNetwork> }
pub struct XItem { pub key: XKey, pub position: u32, pub type_name: String, pub order_number: Option<String>,
                   pub firmware: Option<String>, pub built_in: bool, pub addresses: Vec<XAddr>, // {start, bits, io}
                   pub channels: Vec<XChannel>, pub children: Vec<XItem>, pub iec: Option<IecTags> }
pub struct XKey { pub foreign_id: Option<String>, pub ew_id: Option<Uuid> }
```

- **Export** projects the model into an image (`exchange_image(cfg)`) and writes it. Imports parse a file into an
  image, and `ew-plc` (GPL) matches and merges it against the model.
- **EW identity travels with the file** (FR-PLC-056). CAEX `ID`s are the EW UUIDs of station and module
  devices and the v5 channel IDs; tag IDs are `v5(channel, "tag")`. The XLSX and CSV formats carry the channel ID in
  a hidden column. `ProductDesignation IEC` carries the derived tag, `LocationIdentifier IEC` the location. The
  verified samples show these two attributes in TIA Selection Tool exports (PBF §3.1, finding 6); whether TIA Portal
  keeps them, and EW's GUIDs, on a CAx re-export is unverified (§8, item 6) and part of SC-8.
- **The last exchange snapshot** is the merge base. It is stored as canonical JSONL in
  `exchange/<config-project-uuid>.jsonl`: one record per exchanged station, module, channel and tag, holding the
  exchanged field values and the foreign-ID map (kinds `exchange_record` and `foreign_id`, ranks 70 and 71 of
  [05](05-project-format-and-persistence.md) §2.2; included in the as-built archive). A separate file keeps it out
  of `project.jsonl` merges, travels through git, and lets a colleague merge a TIA re-export.

### 4.3 AML AR APC export

| EW | CAEX element (role or interface class) | Attributes |
|---|---|---|
| Configuration project | `InternalElement` (`AutomationProject`) | Name = `PLC1` |
| Station | `InternalElement` (`Device`) | TypeIdentifier (`System:Device.<family>` or `OrderNumber:<MLFB>` in the TIA sample; `OrderNumber:195740` for -201U1, unverified for GSD devices, Open question 9), `ProductDesignation IEC`, `LocationIdentifier IEC` |
| Rack, module, submodule | `InternalElement` (`DeviceItem`) | PositionNumber, TypeName, TypeIdentifier `OrderNumber:533361`, FirmwareVersion, BuiltIn, `Address` list {StartAddress, Length in bits, IoType}, IEC tags |
| Channel | `ExternalInterface` (`…InterfaceClassLib/Channel`) | Type Digital/Analog, IoType, Number, Length |
| Symbol | `ExternalInterface` (`…/Tag`) in a `TagTable` element (under the CPU `DeviceItem` in TIA; without a CPU see Open question 18) | DataType, IoType, LogicalAddress "0.1" (no area letter; the area follows from IoType), Comment with `aml-lang` per `MlText` language |
| Channel ↔ symbol | `InternalLink` | RefPartnerSideA `<module ID>:Channel_DO_1`, RefPartnerSideB `<tag table ID>:TA05_V_15` |
| Bus interface, port, node | `CommunicationInterface`, `CommunicationPort` (Label), `Node`/`NodeEthernet` | NetworkAddress, SubnetMask, IpProtocolSelection; PROFIBUS address names to be fixed from a real DP export (Open question 9) |

Golden A at 1.0, with the symbol template `{target.process_tag}`, shortened (role and attribute declarations
omitted). The element nesting follows the TIA V21 sample (PBF §3.1); the tag-table placement without a CPU is
Open question 18. Both are verified in the exchange test (SC-8):

```xml
<CAEXFile FileName="PLC1.aml" SchemaVersion="2.15">
  <AdditionalInformation AutomationMLVersion="2.0"/>
  <AdditionalInformation><Document DocumentIdentifier="AR APC" Version="1.4.0"/></AdditionalInformation>
  <InstanceHierarchy Name="PLC1">
    <InternalElement ID="…cfg uuid…" Name="PLC1">                          <!-- AutomationProject -->
      <InternalElement ID="…-201U1 uuid…" Name="-201U1">                   <!-- Device -->
        <Attribute Name="TypeIdentifier"><Value>OrderNumber:195740</Value></Attribute>
        <Attribute Name="ProductDesignation IEC"><Value>-201U1</Value></Attribute>
        <Attribute Name="LocationIdentifier IEC"><Value>+J-E-2416</Value></Attribute>
        <InternalElement ID="…-201U3 uuid…" Name="-201U3">                 <!-- DeviceItem, PositionNumber 1 -->
          <Attribute Name="TypeIdentifier"><Value>OrderNumber:533361</Value></Attribute>
          <Attribute Name="Address"><Attribute Name="1">
            <Attribute Name="StartAddress"><Value>0</Value></Attribute>
            <Attribute Name="Length"><Value>8</Value></Attribute>
            <Attribute Name="IoType"><Value>Output</Value></Attribute></Attribute></Attribute>
          <ExternalInterface ID="…ch1 v5…" Name="Channel_DO_1"
              RefBaseClassPath="AutomationProjectConfigurationInterfaceClassLib/Channel"/> <!-- Number 1, Length 1 -->
        </InternalElement>
        <!-- -201U4 … -202U2: StartAddress 1 … 6 -->
      </InternalElement>
      <InternalElement ID="…tag table…" Name="EW_IO">                       <!-- TagTable -->
        <ExternalInterface ID="…tag v5…" Name="TA05_V_15"
            RefBaseClassPath="AutomationProjectConfigurationInterfaceClassLib/Tag"/> <!-- Bool, Output, 0.1 -->
      </InternalElement>
      <InternalLink Name="Link To Tag_1" RefPartnerSideA="…-201U3 uuid…:Channel_DO_1"
                    RefPartnerSideB="…tag table…:TA05_V_15"/>
    </InternalElement>
  </InstanceHierarchy>
</CAEXFile>
```

The full export holds 7 module `DeviceItem`s with 8-bit output ranges, 56 channel interfaces and 8 tags with 8
`InternalLink`s (FR-PLC-029, AC-30). The absolute starts are relative to the node (Q0 … Q6, ▲U-22). TIA applies the
ranges on CAx import. Writer-header dates come from the export's document date (default: the revision date, or
`SOURCE_DATE_EPOCH` in CI; FR-EXP-006), never from the clock (NFR-REL-07). Element and attribute order is fixed
(station order, slot, channel number), so the bytes are stable (FR-EXP-006). The personal-data option (FR-EXP-009)
leaves no person fields in the file.

### 4.4 AML import and the three-way merge

```mermaid
sequenceDiagram
  participant E as Engineer (P1)
  participant EW as EW session
  participant F as AML file
  participant TIA as TIA Portal (P3)
  E->>EW: ew export plc-aml --config PLC1
  EW->>F: write image E0 (CAEX 2.15, AR APC 1.4.0)
  EW->>EW: transaction "Record AML exchange": snapshot S := E0
  F->>TIA: CAx import, the programmer renames tags and sets addresses
  TIA->>F: CAx export
  E->>EW: import F
  EW->>EW: T = parse(F), match T to EW, per-field 3-way (S, E, T) with ownership
  EW-->>E: preview: added, removed, changed, conflicts, parking list
  E->>EW: accept with resolutions
  EW->>EW: one transaction: model edits + snapshot S := T
```

**A6. Matching** (FR-PLC-032), first hit wins:
1. persisted foreign ID or EW UUID;
2. `ProductDesignation IEC` = derived full tag;
3. (station, rack, slot, subslot, channel number);
4. order number + position.

Ambiguous or unmatched foreign objects go to the **parking list**. They are never placed on pages and never created
silently. Each has a "create unplaced" action (TIA's ParkingLot equivalent). The comparison scope is the set of
stations present in the file. A station missing from the file is untouched unless the user ticks "file is complete".

**Per-field resolution** (FR-PLC-033):

| Ours E vs base S | Theirs T vs base S | Owner Ecad | Owner Plc | Owner Shared |
|---|---|---|---|---|
| unchanged | changed | listed as "rejected PLC change", can be accepted | apply T | apply T |
| changed | unchanged | keep E (exported next time) | keep E, flagged "export pending" | keep E |
| changed | changed, T ≠ E | conflict, default keep E | conflict, default take T | conflict, default keep E |
| either | E = T | no change | no change | no change |

- **Imported module starts become `Manual(…)`;** the snapshot records them as PLC-supplied, which is what the
  "address owned by PLC1" message cites. Channel addresses stay `Auto`, so they still follow the part map and
  EW-PLC-012 still applies.
- The preview groups changes per channel and shows the affected folios. The commit is **one undoable transaction**
  that includes the new snapshot, so undo restores both.
- Re-importing an unchanged export gives an empty diff (FR-PLC-032).

**Drift (EW-PLC-016).** The online rule compares `hash(exchange_image(cfg))` with the snapshot's hash and reports
"changed since the AML exchange of <date>". The second clause of FR-PLC-035 (the foreign file is newer) needs file
access, which queries never have, so FR-PLC-035 evaluates it in the exchange dialog and by `ew plc status <file>`
only.

### 4.5 TIA "PLC Tags" XLSX

Sheet "PLC Tags", columns exactly: Name, Path, Data Type, Logical Address, Comment, Hmi Visible, Hmi Accessible,
Hmi Writeable, Typeobject ID, Version ID (FR-PLC-036). The workbook is written and read by a dedicated exporter and
importer in `ew-plc`, not by a machine or label profile of [13](13-manufacturing-outputs.md) §5.4 and §7, because
its round trip needs code. One row per channel with a determined symbol:

| Name | Path | Data Type | Logical Address | Comment | Hmi Visible | Hmi Accessible | Hmi Writeable | Typeobject ID | Version ID |
|---|---|---|---|---|---|---|---|---|---|
| TA05_V_15 | EW_IO | Bool | %Q0.1 | TA05-V-15 (-201U3-Q1) | True | True | True | | |

- The path is `SymbolPolicy.tag_table`. HMI flags are a configuration-project setting; their TIA defaults are
  unverified.
- The comment uses the export language chosen in the dialog. Other `MlText` languages are untouched on re-import.
- Golden A at 1.0 exports 8 rows.
- **Import** matches by Name, then by Logical Address, and previews like §4.4 with the same snapshot. Tags without a
  channel are listed, never created.
- Because the workbook carries absolute addresses, the export warns while the module-start field group is still
  owned by `Ecad` and the starts are `Auto`: "addresses not confirmed by the PLC configuration" (§8, item 1).

### 4.6 PLCopen XML for CODESYS

One GVL `IO_ElektraWatt` per configuration project, with one variable per used channel with a symbol
(FR-PLC-037):

```xml
<globalVars name="IO_ElektraWatt">
  <variable name="TA05_V_15" address="%QX0.1">
    <type><BOOL/></type>
    <documentation><xhtml xmlns="http://www.w3.org/1999/xhtml">TA05-V-15 · -201U3-Q1:14</xhtml></documentation>
  </variable>
</globalVars>
```

- Addresses always use the IEC rendering, whatever the project profile.
- `fileHeader/@creationDateTime` takes the export document date (NFR-REL-07).
- The placement under `instances/configurations/configuration/resource/globalVars` follows the CODESYS-written
  sample of PBF §3.3, and the `address="%IX0.1"` form the OpenPLC sample of PBF §2.5. Whether CODESYS 3.5 imports a
  GVL without its `addData` device block is part of the exchange test (SC-8).
- The file declares the namespace `http://www.plcopen.org/xml/tc6_0201`.
- CI parses the output with the `plcopen` crate as a test-only dependency.

### 4.7 Generic I/O list (CSV and XLSX)

The I/O-list row source (§5) exports with the FR-PLC-023 columns, one row per channel, through the
[09](09-reports-forms-output.md) §4.6 machinery: CSV dialects `excel-nl`, `excel-en`, `rfc4180` (FR-EXP-003) and
XLSX with a hidden `id` column and the hidden `_ew` and `_ew_base` sheets (FR-EXP-004,
[14](14-interop.md) §8.1).

- **Keys.** The row ID is the channel ID. Re-import matches by ID, then by (station, slot, channel number)
  (FR-PLC-038).
- **Three-way in XLSX.** The hidden `_ew_base` sheet holds the exported cell values of the editable columns:
  symbol, comment, signal range and state. Re-import then compares file, base and current model per cell. It
  reports EW edits made since the export without any project-side storage, as FR-EXP-004 asks. CSV has no hidden
  sheet, so a CSV re-import is a two-way diff with a warning.
- **Editable columns** are the manual fields only. Edits to derived columns (address, function text, target) are
  reported as "derived value, not applied" with the reason ("edit the start of -201U3" or "edit the process tag of
  -203Q1").
- **Excel safety.** Tags such as "-201U3" are string cells in XLSX. The CSV presets apply the formula guard of
  NFR-SEC-17 ([09](09-reports-forms-output.md) §4.6).

Golden A: the default block exports 28 rows (§3.2). Editing three comments in Dutch Excel (`excel-nl`) and
re-importing changes exactly three fields in one transaction, which is FR-PLC-038's criterion.

**Instrument-list import** (1.0, FR-PLC-050). Rows hold tag, description, signal type, range, field location and
preferred station. They create placeholder field devices (state "supplied by others") with the process tag and
pre-assign spare channels of the matching kind in station order. A 10-row list with 8 DO tags against the golden
station assigns 8 spare channels of -201U5 and -201U6 and lists 2 tags as EW-PRC-002 open items. Generating whole
schematics from such a list is IDEA-025 in [07](07-symbols-parts-macros.md) and [17](17-extensibility.md).

### 4.8 Later formats

- **L5X** (1.x): a module tree with CatalogNumber, ParentModule, ParentModPortId, ports with slot and IP, and
  `<Tag TagType="Alias" AliasFor="Local:3:O.Data.1">` per symbol. The form per module family is unverified.
- **TwinCAT TE1120** (1.x): terminal order, product codes, and channel ↔ variable links. No absolute addresses.
- **TIA Selection Tool** (1.x): creates stations and modules with the `-UF01` / `-K01` IEC tags it carries and adds
  accessory lines (front connectors, memory cards) to the BOM.
- **TIA Openness** stays out of the core (ARCH §16).

### 4.9 Device-description import

1. **Add the file.** The user adds a GSD, GSDML or IODD file to a company or user catalogue (`ew-library`). EW records
   hash, origin and licence note and keeps the file in the catalogue directory, never in a project.
2. **Parse.** The parser produces a device type in the permissive `ew-devdesc` crate (ARCH §3). Parsers are
   streaming, reject DTDs and external entities, and enforce the limits of NFR-SEC-02. Each has a cargo-fuzz target
   (NFR-SEC-03).
3. **Link.** The user links order numbers to device-type entries: one part ↔ one DAP or module ident. The part
   supplies what the DD lacks (§2.8).
4. **Build.** The rack/slot editor (1.0, FR-PLC-028) places modules into slots, validated against `FixedInSlots`,
   `AllowedInSlots`, `Max_Module` and the maximum I/O lengths (EW-PLC-011). N1 then expands channels.

A CPX-FB13 GSD gives the golden node its supported rates for EW-BUS-005. The ABB FENA GSDML V2.31 sample yields
physical slots 0..2, fixed slot 0, port submodule 32769 and the F_Dest_Add range 1..65534 (FR-PLC-026). The Balluff
IODD creates pins L+, Other, L−, C/Q with core colours BN, WH, BU, BK on the connecting M12 cable (FR-BUS-013).

### 4.10 Determinism, privacy and security of exchange

- **Determinism.** The same model and options give byte-identical AML, XLSX, PLCopen and CSV files on every platform
  and thread count (FR-EXP-006, NFR-REL-07). Order is fixed, IDs are v5, dates come from the export's document date
  (revision date or `SOURCE_DATE_EPOCH`), and XLSX workbook properties are set explicitly.
- **Privacy.** Exports hold no person fields when `--omit-personal-data` is set (AML: FR-EXP-009; XLSX and CSV
  report tables: FR-OUT-025; NFR-PRIV-06).
- **Hostile input.** AML, PLCopen, GSD, GSDML, IODD, ESI, EDS and CSV/XLSX parsers enforce the NFR-SEC-02 limits.
  Imported identifiers are NFC-normalised and confusable-checked (ARCH §4.5). An import never executes content and
  changes the model only through the previewed command.

---

## 5. Outputs: report row sources

Row content is defined here. Forms, pagination and the shared PDF/XLSX/CSV source are in
[09](09-reports-forms-output.md) §3. Rows sort by configuration project, station order, slot and channel number,
with natural sort for tags.

| Block kind | Rows | Columns | Release |
|---|---|---|---|
| `plc_overview` (FR-PLC-022, FR-REP-019) | one per channel of a station or module, all states | module DT, part, address, channel number, designation, kind, symbol, function text (else the tracked process tag, §3.4), process tag, target DT, state, link to the main representation | MVP |
| `io_list` (FR-PLC-023) | one per channel; default filter `state != Deactivated` | configuration project, CPU, station and bus address, rack.slot.subslot, module DT and part, channel number, designation, kind, address, symbol, data type, comment, function text (else the tracked process tag, §3.4), process tag, target DT and pin, target page and cell, passed terminals, signal range, F-address, state | MVP (target columns from 1.0) |
| `plc_card_list` (FR-REP-045) | one per module | station, slot path, module DT, part, type ident, start and length per direction, used / spare / reserved / deactivated counts | 1.0 |
| `network_address_list` (FR-BUS-008) | one per port with an address | logical net, station DT, device type, address or name, IP, mask, master flag, rate, configuration project, `Tbd` flag | 1.0 |
| `bus_overview` (FR-REP-020, FR-BUS-007) | per logical net, then per segment member | position, station DT, port, address, connector DT, termination, cable DT, cable length, external flag | 1.0 |
| `bus_cable_list` (FR-BUS-008) | one per bus cable | cable DT, bus system, part, length (source), from, to, segment | 1.0 |
| `iolink_port_list` (FR-BUS-014) | one per used master port | master DT, port, mode, class, device DT, vendor and device ID, IODD hash, process data in/out bits, cable | 1.0 |
| `safety_io_list` (FR-PLC-041) | one per safety channel | F-destination and F-source address, module DT, channel pair, sensor or actuator DT, requirement field, test-pulse source | 1.0 |

The open-items list (FR-REP-021) receives every `Tbd` start, channel and bus address, every unassigned
instrument-list tag and every used channel without a main representation. The SQL projection exposes
`api_v1_plc_channel` at 1.0 (FR-EXT-007) with the `io_list` columns plus IDs. The D-25 user SQL report "24 V loads
per protective channel" (ARCH §11, ADR-0017) reads `api_v1_cp` and `api_v1_potential`; from 1.0 it sums the load
currents `api_v1_cp.i_load` per channel, which EW-PLC-014 compares with the channel rating (ADR-0017 amendment 3).

Golden A, folio 908: 56 rows on 908, 908.1 and 908.2 (23 rows per page). Station address "TBD"; addresses Q0.0 …
Q6.7; valve DTs; printed function texts TA05-V-15 … (the driven devices' process tags) or "Spare"; states 8 used,
20 spare, 28 deactivated; one link per bound row to the valve on 203–206 (AC-29). The network address list has two
rows: JE-N1000-03 (master, external) and -201U1 (slave, `Tbd` flagged).

**As built (cycle 13; decision log ID-333).** `plc_overview` is a report kind in `ew-reports` over `plc_rows(station)`. Folio 908 prints the column set of the table above (module, part, address, channel number, designation, kind, symbol, function text, process tag, target, state and the page cell), a superset of the table of doc 20 §11.3. Every row whose channel is bound to a coil (8 used and 20 spare) links with `LinkTarget::Cp` to the coil position, the "link to the main representation" of the table; the golden PDF has 92 link annotations, 64 cross-reference links plus these 28. The station address "TBD" is not printed, because there is no `BusPort` record yet (FT-225). The station list is the block's manual selection of stations, else every station in natural tag order.

---

## 6. UI and UX touchpoints

| Surface | Behaviour | Release | Requirements |
|---|---|---|---|
| PLC navigator, tree | configuration project → station (role icon) → rack → module (slot) → channel; bus ports under the station; badges for unplaced, `Tbd` and finding count; drag a channel onto a pin or coil to bind | MVP | FR-PLC-020, FR-UI-007 |
| PLC grid (table mode) | the columns of FR-PLC-021; filters unassigned, duplicate, spare, unplaced, safety, per bus, per configuration project, `Tbd`; fill-down, pattern fill (`TA05_V_{n:02}`), Excel paste, each edit one command | MVP Should | FR-PLC-021, FR-UI-008 |
| Property grid | address with provenance: *"Q0.1 · derived: -201U3 output start Q0 (auto, packed, slot 1) + 533361 channel map n → bit 1"*; function text *"TA05-V-15 · process tag of -203Q1, driven by -201U3-Q1"*; override marker and "reset to derived" | MVP | FR-UI-010 |
| Re-address preview | old → new address per channel, affected folios, symbol changes, rendered candidate pages; accept or cancel | MVP Should | FR-PLC-014, FR-UI-022 |
| Message panel | EW-PLC-012 jumps to the coil on 203; quick fixes "rebind to map channel", "set -3X1 termination On", "clean symbol", "re-address module"; waivers need a reason | MVP | FR-UI-021 |
| PLC workspace | PLC navigator, grid, bus navigator, exchange dialog and the fluid pages side by side | 1.0 | FR-UI-003, FR-UI-006 |
| Rack/slot editor | drag modules from the device type into allowed slots; valve-terminal slot list next to the stored order code; live EW-PLC-011 | 1.0 | FR-PLC-028 |
| Bus navigator | logical nets → ports; physical nets → derived segments with ends; termination toggles; rate at the net | 1.0 | FR-UI-006 |
| Exchange dialog | format and version (CAEX 2.15 or 3.0 read; AR APC 1.4.0 or 1.2.0), station scope, document date, personal-data option; import preview tabs: added, removed, changed, conflicts, parking | 1.0 | FR-PLC-029 … -036, FR-EXP-009 |

*As built (cycle 38, WP-S2.07 part 3, decision log ID-877).* The PLC navigator's tree is configuration project → station (role word) → module → channel, without racks and bus ports (FT-739); the grid has 14 columns without rack, data type, safety and signal range (FT-737), the flags spare, unplaced, unassigned, tbd and duplicate and the config facet, without safety and bus facets (FT-738); a channel row dropped on a CP binds it with one `plc.bind_channel`. Golden A's valve coils have no drawn CP to drop on (FT-740).

- **Persona and journey.** The PLC grid is the main surface of persona P3 and of journey J4
  ([vision](00-vision-and-scope.md) §5, §6).
- **Accessibility.** The PLC grid and navigator are also the accessible representation of folios 203–206
  (FR-UI-026). Safety and state styles combine an icon or pattern with colour (FR-UI-025).
- **Keyboard and vocabulary.** Every action is in the command palette, and EPLAN terms work as aliases
  ("configuration project", "PLC box", "readdress"; NFR-UX-03).
- **Testing.** The usability-test protocol of [16](16-ux-ui.md) includes the J4 tasks at 1.0.

---

## 7. CLI and automation

- **Export.** `ew export plc-aml --config PLC1 [--apc 1.4.0|1.2.0]`, `ew export plc-xlsx` and `ew export plcopen`
  at 1.0 (FR-EXT-003). `ew export report --block 908 --format xlsx` and the `io_list` block at MVP (FR-EXT-027,
  [09](09-reports-forms-output.md) §4.6).
- **Import.** `ew import plc-aml <file> --preview --json` prints the diff of §4.4 and applies it only with
  `--accept <choices.json>` (the per-conflict choices of the preview; 1.0). `ew import io-list <file>` applies a
  generic I/O-list round trip (MVP Should, FR-PLC-038). `ew plc status <file>` reports drift (1.0). The subcommand
  names are fixed in [17](17-extensibility.md) §3.3.
- **Queries and SQL.** `ew query channel_addresses -201U3 --explain`;
  `ew sql "select address, symbol, state from api_v1_plc_channel"` (1.0).
- **Gates.** `ew check --scheme pre-delivery` reports EF-02 and EF-03 on Golden A and exits 1.
- **MCP (off by default).** `ew mcp` reads the same queries (read tools from MVP, Could). From 1.0 an agent's
  re-addressing proposal is a `preview_command(plc.readdress, …)`, committed only through `commit_preview` after
  ask-first approval (ADR-0020, ARCH §10.5).
- **Scripts.** rhai can call `plc.readdress` and read `plc_rows`, never write addresses as text.

---

## 8. Edge cases and pitfalls

1. **Unconfirmed absolute addresses.** Golden starts are relative to the node (▲U-22). Until the PLC side owns the
   starts, exported tag addresses carry a warning, and the I/O list shows a "relative" marker per station.
2. **Word and bit overlap.** `IW0` overlaps `I1.3`. EW-PLC-001 compares bit intervals, never strings, and word
   alignment is a warning only.
3. **Module part changes and station deletion** shift later Auto ranges; both are previewed (§3.3, §3.9), and
   PLC-owned starts never move.
4. **Git merges.** Two branches adding modules to one station cannot create overlapping Auto ranges: Auto starts
   are derived after the merge and skip occupied ranges (§3.1). Overlapping Manual values set on both branches
   surface in the post-merge integrity check (EW-CORE-013, [05](05-project-format-and-persistence.md) §6.3) and in
   EW-PLC-001. v5 channel IDs prevent duplicate channel records.
5. **Symbols.** Cleaning can make two symbols equal ("TA05-V-15" and "TA05_V_15"); EW-PLC-007 tests the cleaned
   value. Leading digits get "_" (FR-PLC-017, FR-PLC-047). TIA quoting rules are opt-in.
6. **Foreign GUIDs.** TIA may replace EW's IDs on re-export (unverified). Matching then falls back to IEC tags, and
   ambiguous cases go to the parking list instead of being guessed.
7. **Partial exports.** A file holding one station never deletes the others (§4.4 scope rule).
8. **Multilingual comments.** AML carries every language. The TIA XLSX carries one, and its re-import touches only
   that language.
9. **Bistable valves and blanking plates.** Both solenoids are claimed and bound by N2. Empty positions give
   deactivated channels. EW-FL-002 compares solenoids with bound channels.
10. **IO-Link mode changes** alter channel kind and data width, and thereby the module's I/O width and the starts
    of later modules. The change is previewed.
11. **External partners.** Their address and termination are often unknown. EW-BUS-004 reports an info, and
    EW-BUS-007 accepts an external master. Plant-wide F-address uniqueness can only be checked within the project
    and against values entered on external partners.
12. **`Tbd` vs `NotApplicable` vs empty.** Only `Tbd` is an open item. An empty value on a used channel is
    EW-PLC-008.
13. **Spreadsheet edits of derived columns** are reported, never applied (§4.7).
14. **Scale.** At XL (10,000 channels) `address_index(cfg)` sorts once per commit that touches addresses; AML is
    written streaming. NFR-PERF-22 budgets AML export and import preview at 10 s each.
15. **Unverified constants.** Bus ranges, segment lengths, IO-Link limits, PROFINET name rules and the analog
    default starts ship with `verified = false` and report at most warnings (FR-CHK-046, NFR-COMP-05).

---

## 9. The reference project

### 9.1 Folio by folio

| Folio | EW model | Derived and printed | Requirement |
|---|---|---|---|
| 3 | -201U1's bus-port function (aux) on a BusTopology page; -3X1 bus connector with `termination = On`; `Bus` line with cable -3W1; external point "DP" with `ExternalRef` JE-N1000-03 in +J-E-206 | "-201U1 (201-E1)"; "Festo / 51E-F13GCQP-D+H2GG / CPX FB13 / Node TBD" from manufacturer, order code 1, part type and `address = Tbd`; "Switch to "ON"" from `termination`; "To J-E-206 / JE-N1000-03" from the ExternalRef | FR-BUS-001 … -006 |
| 106 | the card power connector functions of -201U1 and -202U1 (aux) with typed pins 1–5 and FE; cables -106W1, -106W2 | potentials per pin; FE on PE reported (EF-10 at MVP, EF-22 at 1.0) | FR-PLC-024, FR-PLC-048 |
| 201–202 | assembly overview generated from the children in manifold order | "∞ 203-B4" … "∞ 206-B4"; 4 valves on -202U2 | FR-DEV-023, FR-XRF-016 |
| 203–206 | 28 valve representations, each drawing fluid function + coil function; 28 bindings | Q0.1 … Q6.7 from `channel_addresses`; TA05-V-xx from the driven device's process tag (used valves carry no function text), or the manual function text "Spare" | FR-PLC-011, FR-FLU-005 |
| 908 (new) | `plc_overview` report block | 56 rows on 908–908.2 | FR-PLC-022 |

**As built (cycle 13; decision log ID-333).** The coil addresses on folios 203–206 (Q0.1 … Q6.7) are not printed yet; their text slot is FT-227. Folio 908 is built (§5). Folio 3's "Node TBD" waits for the `BusPort` record (FT-225; §3.8).

### 9.2 Module -201U3 under H1 and H2

| Ch | Bound to | Address | State | Function text | Symbol (1.0) | EW-PLC-012 H1 / H2 |
|---|---|---|---|---|---|---|
| 0 | — | Q0.0 | deactivated | — | — | — / — |
| 1 | -201U3-Q1 coil 14 | Q0.1 | used | TA05-V-15 | TA05_V_15 | error (expects ch 0, Q0.0) / — |
| 2 | — | Q0.2 | deactivated | — | — | — / — |
| 3 | -201U3-Q2 coil 14 | Q0.3 | used | TA05-V-33 | TA05_V_33 | error / — |
| 4 | — | Q0.4 | deactivated | — | — | — / — |
| 5 | -201U3-Q3 coil 14 | Q0.5 | used | TA05-V-18 | TA05_V_18 | error / — |
| 6 | — | Q0.6 | deactivated | — | — | — / — |
| 7 | -201U3-Q4 coil 14 | Q0.7 | used | TA05-V-17 | TA05_V_17 | error / — |

-201U4 repeats the pattern on Q1 with TA05-V-36, -38, -16 and -37. -201U5 … -202U2 have spare valves on Q2 … Q6.
Totals: 8 used, 20 spare, 28 deactivated; EW-PLC-012 gives 28 or 0 findings (AC-29, EF-02). At 1.0 the exports hold
7 modules, 8 tags and 8 links (AC-30), and the bus checks give AC-31.

### 9.3 Defects prevented

| Defect | How this design prevents or reports it |
|---|---|
| D-01 -4W1 / -4X1 in the BOM | -3W1 and -3X1 are real devices; their tags are derived, and the topology page holds no free text |
| D-09 no PLC overview or I/O list | `plc_overview` (908) and `io_list` blocks at MVP; bus reports at 1.0 |
| D-10 overview 5 vs 4 valves | overview generated from the assembly's children; channels and positions come from the same children |
| D-15 "Node TBD"; termination as text | typed `Tbd` (EW-BUS-001 blocks release, EF-03); termination is a property checked by EW-BUS-004 |
| D-16 FE on PE at -201U1:3 | pin role FE in part data; EW-POT-003 (FR-CHK-030, EF-10) at MVP, superseded by EW-PLC-013 (EF-22) at 1.0 |
| D-25 crossed supply pairs, one 6 A channel | supply groups El/Val; EW-PLC-014 load evaluation; load summary (FR-REP-026) |
| D-26 odd-bit addresses | addresses derived from part maps; EW-PLC-012 under H1; the decision is part data with confidence |
| D-27 untagged valves | child devices with derived tags; process tags on field devices; automatic symbols from them |
| D-36 order code vs components | order codes on the `Assembly` aspect; channels from component parts |
| D-40 CPX block inside the MPA section | -202U1 carries no `PlcModule`, so addressing is unaffected; slot validation EW-FL-003 (1.x) |
| D-41 assembly tagged from two pages | positional slots from `Device.position`, never from tags or pages |

---

## 10. Scope by release

| Capability | MVP | 1.0 | 1.x | Later |
|---|---|---|---|---|
| Model | configuration project, station, module, channel; internal channels; card supply typing; bus ports, connectors, external partners | device types from GSD/GSDML/IODD; rack/slot editor; safety data; PROFINET, IO-Link, EtherCAT structure | ESI, EDS; EtherNet/IP, AS-i | CANopen, Modbus |
| Addressing | profiles TIA intl and IEC; Auto, Manual, Excluded, Tbd; part maps; re-address preview (Should) | freeze; Rockwell rendering; ownership protection | further vendor profiles; slot-based allocation | — |
| Texts | manual symbol and comment; function text from direct targets | automatic symbols; target tracking; process-tag scheme | structured (UDT) symbols; tags from PLC data (FR-NUM-027) | — |
| Representations | channel pins, coil binding on fluid pages, assembly overview, PLC overview box (Should) | generated bus topology overview | — | — |
| UI | PLC navigator, grid (Should), drag-bind | PLC workspace, bus navigator, exchange dialog, rack/slot editor | process-tag navigator | — |
| Exchange | generic I/O list CSV/XLSX round trip (Should) | AML AR APC 1.4.0 (+1.2.0) export and import with 3-way merge; TIA tag XLSX; PLCopen GVL; instrument-list import | TST import; L5X; TwinCAT | TIA Openness bridge |
| Checks | EW-PLC-001, -002, -003, -007, -008, -012, -015; EW-BUS-001, -004 | all other EW-PLC, EW-SAF, EW-PRC, EW-BUS, EW-IOL rules | EW-BUS-008 | — |
| Reports | `plc_overview`, `io_list`, open items | `plc_card_list`, bus reports, IO-Link and safety lists | — | — |

The MVP column is what the golden acceptance criteria need: AC-31 at milestone M2 and AC-29 at the MVP release.
AC-30 and the EW-PLC and EW-BUS part of AC-22 close at M3; SC-8 (the exchange test with real TIA and CODESYS
installations) closes by 1.0 ([20](20-golden-project.md) §22; [vision](00-vision-and-scope.md) §3.2).

---

## 11. Dependencies

| Crate | Role here |
|---|---|
| `ew-schema` | `PlcConfigProject`, aspects, `Channel`, bus records, exchange snapshot records |
| `ew-plcaddr` (permissive, ARCH §3) | `PlcAddress`, profile data model, grammar engine, bit-interval arithmetic; used by `ew-derive`, `ew-plc` and the QET importer |
| `ew-caex`, `ew-devdesc` (permissive, ARCH §3) | CAEX 2.15/3.0 read and write, `ExchangeImage`; GSD, GSDML, IODD (ESI, EDS at 1.x) |
| `ew-derive` | the queries of §2.10 |
| `ew-commands` | normalisers N1–N4; `plc.*` commands |
| `ew-checks` | EW-PLC, EW-SAF, EW-PRC, EW-BUS and EW-IOL rules |
| `ew-plc` | exchange orchestration, matching and merge, the TIA "PLC Tags" XLSX exporter and importer, writers for PLCopen and L5X, rack/slot logic |
| `ew-reports`, `ew-scene` | row sources of §5; text slots and property arrangements |
| `ew-viewmodel`, `ew-ui` | navigator, grid, dialogs |
| `ew-import-qet` | emits `plc_*` element info and recognised "Q0.1" texts as pending channel bindings (FR-IMP-011, FR-IMP-017); it never sees `ew-model` (ARCH §3, rule 4). `ApplyImport` in `ew-commands` resolves them by A7 (§3.3) |

| Document | Relation |
|---|---|
| [03](03-domain-model.md) | the records of §13–§14 with this document's refinements, adopted in the schema-steward batch (Open questions 3, 4, 6, 15, 22) |
| [05](05-project-format-and-persistence.md) | kind ranks; the `exchange/` directory (ranks 70, 71) and the `plc_profile` library kind in `lib/plc-profiles/` (Open questions 2, 13) |
| [06](06-schematic-editor.md) | page types, text slots, xref formats |
| [07](07-symbols-parts-macros.md) | part sections L2, confidence, valve typical with `ChannelBinding` value sets |
| [08](08-connections-terminals-cables.md) | `Bus` lines, cables, supply potentials, `Internal` connections |
| [09](09-reports-forms-output.md) | blocks 908 and `io_list`, `_ew` and `_ew_base` sheets, CSV guard |
| [11](11-fluid-power.md) | valve model, `ValveTerminal` aspect, N-FL-3 (= N2), EW-FL-002/003, valve-terminal configuration report |
| [12](12-panel-layout.md) | EtherCAT rail order, placement of PLC hardware |
| [14](14-interop.md) | generic import and export, QET and EDZ PLC mapping |
| [15](15-checks-revisions-collaboration.md) | rule engine, configuration-project scope, release gate |
| [16](16-ux-ui.md), [17](17-extensibility.md), [18](18-quality-security-release.md), [20](20-golden-project.md) | UI, CLI names, fuzzing and determinism jobs, AC-29 … AC-31 |

---

## Requirements covered

- **PLC:** FR-PLC-001 … FR-PLC-056.
- **BUS:** FR-BUS-001 … FR-BUS-024.
- **EXP (PLC exchange):** FR-EXP-001, FR-EXP-002, FR-EXP-003, FR-EXP-004, FR-EXP-006, FR-EXP-009.
- **Core rows with a PLC or bus part (02a):** FR-DEV-012, FR-DEV-023, FR-DEV-024, FR-DEV-026, FR-DEV-028,
  FR-DEV-034, FR-PRT-010, FR-REP-019, FR-REP-020, FR-REP-021, FR-REP-026, FR-REP-045, FR-CHK-005, FR-CHK-030,
  FR-CHK-034, FR-CHK-037, FR-CHK-042, FR-CHK-045, FR-CHK-046, FR-XRF-016, FR-CBL-008, FR-MAC-004, FR-NUM-027.
- **Other 02b rows touched:** FR-FLU-005, FR-IMP-011, FR-IMP-017, FR-UI-003, FR-UI-006, FR-UI-007, FR-UI-008,
  FR-UI-010, FR-UI-011, FR-UI-021, FR-UI-022, FR-UI-025, FR-UI-026, FR-EXT-003, FR-EXT-007, FR-EXT-027.
- **Non-functional:** NFR-REL-07, NFR-REL-09, NFR-REL-11, NFR-SEC-02, NFR-SEC-03, NFR-SEC-17, NFR-LIC-06,
  NFR-PRIV-06, NFR-COMP-05, NFR-PERF-08, NFR-PERF-09, NFR-PERF-22, NFR-DOC-02, NFR-UX-03.

### Proposed new requirements

The ten rows this document proposed are in the catalogues:
- derived effective channel state and the I/O-list default filter → covered by the existing **FR-PLC-019**
  ([02b](02b-requirements-extended.md)), which now carries the derivation;
- solenoid-binding normaliser (N2) → **FR-PLC-051** (02b);
- PLC overview box → **FR-PLC-052** (02b);
- v5 channel IDs and confirmed removal of surplus channels → **FR-PLC-053** (02b);
- manual channel target → **FR-PLC-054** (02b);
- renumbering never changes a derived PLC address → **FR-PLC-055** (02b);
- EW identity in PLC exports → **FR-PLC-056** (02b);
- bus-port signal map → **FR-BUS-023** (02b);
- EW-BUS-010 for ports of different logical nets joined by bus connections → **FR-BUS-024** (02b);
- AML export and import-preview budget → **NFR-PERF-22** ([02c](02c-nonfunctional-requirements.md)).

---

## Open questions

1. **Crate placement of the address grammar.** ARCH §3 puts address grammars in `ew-plc`, which depends on
   `ew-commands`. `ew-derive` (rendering, `channel_addresses`) and the QET importer (parsing "Q0.1") cannot depend
   on it. **Resolved:** ARCH §3 adds the permissive crates `ew-plcaddr` (`PlcAddress`, profiles, grammar engine, bit
   intervals), `ew-caex` and `ew-devdesc`; `ew-plc` keeps exchange orchestration, merge, writers and rack/slot logic
   (§11).
2. **Exchange snapshot storage.** No file in [05](05-project-format-and-persistence.md) held the merge base.
   **Resolved:** 05 §2.2 adds `exchange/<config-project-uuid>.jsonl` (1.0) with the kinds `exchange_record` and
   `foreign_id` (ranks 70, 71), included in the as-built archive; `PlcConfigProject.exchange` keeps only the stamp
   (§4.2).
3. **`PlcModule.start` per direction.** ARCH §4.2 has one `start`, but mixed modules need an input and an output
   start, as AR APC's `Address` list shows. **Resolved:** the schema-steward batch adopts `IoStarts { input, output }`
   ([03](03-domain-model.md) §13).
4. **Station allocation order.** Auto allocation needs an order that does not depend on tags. **Resolved:**
   `PlcStation.order: Option<u32>` is adopted, falling back to the UUIDv7 creation order, never tag, page or bus
   address ([03](03-domain-model.md) §13).
5. **Channel binding vs `Internal` connection.** [03](03-domain-model.md) and [20](20-golden-project.md) bind the
   coil CP directly, while [08](08-connections-terminals-cables.md) §2.3 cites the channel-to-coil link as an
   example of `Internal`. **Resolved:** `Channel.binding` only; `Internal` remains for other device-internal links
   between CPs of one part's functions, and 08 §2.3 uses such a link as its example (§3.3).
6. **Authored vs derived bus segments.** [03](03-domain-model.md) §14 stored `PhysicalNet.segments`, which
   duplicates the topology drawing. **Resolved:** segments are derived by `bus_segments(pnet)`, and `PhysicalNet`
   keeps only `order_hint` for undrawn members and `medium` (03 §14, §2.7).
7. **The odd-bit question (D-26).** Whether VMPA1-FB-EMG-8 puts coil 14 on the even bit decides 28 findings or none
   (PQ-15, U-23). *Recommendation:* keep H1 as the golden test input with confidence `test`; confirm from the
   vendor manual or the pilot's PLC program before 1.0, then set `verified` with the source.
8. **Ownership defaults.** *Recommendation:* the table of §2.2, including `Shared` for symbols and comments; confirm
   with the pilot user's PLC programmers (PQ-19) and show the setting in the PLC navigator.
9. **AR APC PROFIBUS attributes, CPX slot numbering and GSD type identifiers.** The verified TIA sample is PROFINET
   only. How TIA numbers the CPX-FB13's GSD modules is unknown, and so is the `TypeIdentifier` TIA expects for a
   non-Siemens GSD device (the sample shows only `System:Device.…` and `OrderNumber:…`; a GSD-based form is
   plausible but unverified). *Recommendation:* make a TIA V21 export of a DP master system with a CPX-FB13 slave a
   precondition for accepting FR-PLC-031's DP part and AC-30 (02b Open question 12), and take the station and module
   `TypeIdentifier` forms from it.
10. **Leading-digit symbols.** FR-PLC-017's example `201U3_Q1_14` broke FR-PLC-047's strict IEC rule (no leading
    digit). **Resolved:** `ident()` prefixes `_`, and FR-PLC-017's example and criterion use `_201U3_Q1_14`; that
    IEC 61131-3 Ed. 4 still allows a leading underscore before a digit stays to verify (§3.4).
11. **I/O-list rows in FR-PLC-023.** Its criterion expected 56 golden rows; the default filter here gives 28.
    **Resolved:** FR-PLC-023's criterion expects the same 28 rows in PDF and XLSX, 56 with the deactivated filter off
    ([09](09-reports-forms-output.md) Open question 9).
12. **"Foreign file newer" in EW-PLC-016.** Queries cannot read files. **Resolved:** FR-PLC-035 evaluates that
    clause in the exchange dialog and in `ew plc status <file>` only (§4.4).
13. **Storage of address profiles.** [05](05-project-format-and-persistence.md) §2.2 lists the library kinds part,
    symbol, fndef, form and macro, but no address profile. **Resolved:** built-in profiles are versioned and never
    removed, and a user profile is embedded as the library kind `plc_profile` in `lib/plc-profiles/<copy_id>.json`
    (1.0; 05 §2.2, §3.4).
14. **Low-adoption crates (C-9).** gsd-parser, plcopen, l5x, taktora-ethercat-esi and ioddengine are below the
    adoption threshold. *Recommendation:* use plcopen and l5x in tests only; vendor gsd-parser and
    taktora-ethercat-esi under a named owner, or write own parsers; never take them as unreviewed runtime
    dependencies.
15. **Manual target field.** `Channel.target` was not in [03](03-domain-model.md) §13. **Resolved:** adopted in the
    schema-steward batch, with FR-PLC-054 as its requirement.
16. **Exchange test installations.** SC-8 needs TIA Portal V21, an older TIA (V17) for AR APC 1.2.0, and CODESYS
    3.5. The CAEX 2.15 XSD licence for CI validation is unverified. *Recommendation:* ask the pilot user for access
    or sample exports (PQ-19); vendor the XSD only if its licence permits, else validate structurally in CI and with
    the AutomationML editor in the exchange test.
17. **Rule constants.** Bus ranges, segment lengths, IO-Link limits, PROFINET name rules and TIA's default analog
    starts are unverified. **Resolved:** they ship with `verified = false` and report at most warnings (FR-CHK-046);
    the verification pass against PI, IO-Link Community and vendor manuals is scheduled in the global plan's
    verification register before 1.0.
18. **Tag tables without a CPU.** In the TIA sample, `TagTable` elements sit under the CPU `DeviceItem`. Golden A has
    `cpu: None` (the DP master is the external partner JE-N1000-03), so the AML has no CPU to hang the 8 tags on.
    *Recommendation:* write the tag table under the `AutomationProject` element (as in §4.3) and test in SC-8 whether
    TIA accepts it; if not, export the tags only through the TIA "PLC Tags" XLSX (§4.5) whenever `cpu` is `None`, and
    say so in the export dialog.
19. **Target tracking and bridges.** FR-PLC-018 says tracking follows "connections, terminals and bridges", while
    §3.5 (and [03](03-domain-model.md) §9.3) stops at bridges, because a bridge joins potentials and would make a DI
    "find" every load on a 24 V rail. **Resolved:** the signal-level rule stays, and FR-PLC-018 reads "through
    connections, terminals, terminal levels and plug pins" (§3.5); 03 Open question 2 still collects EPLAN users'
    feedback on signal boundaries.
20. **Function texts in spreadsheet round trips.** FR-PLC-038's criterion edits 3 function texts in Excel, but a
    channel's function text is derived from its target (§2.4, §4.7). **Resolved:** FR-PLC-038's criterion edits 3
    comments (§4.7); an "apply to target" action for function-text cells has no requirement row and stays a
    candidate for a later release.
21. **Wording of FR-PLC-045 and FR-PLC-015.** FR-PLC-045's criterion speaks of "imported manual addresses Q0.1 …",
    but A7 (§3.3) imports them as bindings with `Auto` addresses; FR-PLC-015's criterion changes "the address map of
    571334", but the maps live on the module part 533361 (571334 declares only its solenoids). **Resolved:** both
    criteria read "imported bindings" and "the solenoid-position map of 533361"; the finding counts stay 28 / 0.
22. **Schema deltas against [03](03-domain-model.md) §13–§14.** Besides Open questions 3, 4, 6 and 15, this document
    changes `PlcConfigProject.ownership` from `EcadOwns | PlcOwns` to an `Owner` per field group (adding `Shared`),
    replaces `foreign_ids` and `last_exchange` by `exchange: ExchangeStamp` plus the `exchange/` file, adds
    `allocation` and `symbols`, replaces `PlcModule.address_map` by `part_use`, makes `Channel.state` optional
    (derived when absent) and adds `PlcModule.safety`, `Channel.signal_range`, `Channel.safety` and
    `PhysicalNet.medium`. **Resolved:** one schema-steward batch adopts them before `ew-schema` v0 freezes, and
    03 §13–§14 lists them (§2).
23. **Direction of the golden bus segment.** FR-BUS-003 and [03](03-domain-model.md) §14 list the segment master
    first (JE-N1000-03 → -3W1 → -3X1 → -201U1 DP); [20](20-golden-project.md) §11.1 listed it from -201U1.
    **Resolved:** master-first everywhere: A5 orders that way (§3.8), and 20 §11.1 lists the segment
    JE-N1000-03 → -3W1 → -3X1 → -201U1:DP.
24. **Auto allocation around Manual ranges.** A1 places Auto ranges at the first free range at or after the cursor,
    which matches TIA's "next free address" behaviour as far as known (unverified). *Recommendation:* keep first-fit
    with the stable cursor; confirm with the pilot user's PLC programmers (PQ-19) that inserted modules should take
    the next free byte (Q7 in §3.9) rather than force a re-address of PLC-owned starts.
