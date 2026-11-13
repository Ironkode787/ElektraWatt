// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! PLC records (doc 03 §13, doc 10 §2.2–§2.6): `plc_config_project` (rank 11), `channel` (37), the device
//! aspects `plc_station` and `plc_module`, the part's `plc` section, and the functional `device_link` (42;
//! doc 03 §7, doc 11 §3.1).
//!
//! Implemented members are those of the MVP (doc 20 §11.2, §11.3, §6.2) and of the 1.0 layer of WP-S1.24: the
//! symbol policy, the safety data of doc 10 §2.9, a channel's data type, signal range and port mode, and the
//! part's allowed slots, station limits, IO-Link port window and output ratings; and of the exchange of WP-S3.16: a
//! configuration project's ownership and exchange stamp, a station's firmware and a module's type identification.
//! The other members of doc 10 §2.2–§2.4 (allocation, device descriptions, auxiliary pins) are additive and stay in
//! `extra` until their work packages.

use std::borrow::Cow;

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::json::{Extra, de_extra, is_default, is_false, is_true, is_v1, r#true, v1};
use crate::proxy::{IdentS, MlTextS, UuidS};
use crate::track::{buffer, de_here, invalid, kept_unknown, malformed, not_a_variant, open_enum, raise};
use crate::value::{Date, Endpoint, Sha256, SourceRef};
use crate::{Device, Function};

// ---------------------------------------------------------------------------------------------------------
// Configuration project

/// One target PLC-software project (doc 10 §2.2, rank 11, `project.jsonl`): it scopes address and symbol
/// uniqueness (FR-PLC-002) and pins the address profile that renders every address (FR-PLC-008).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlcConfigProject {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<PlcConfigProject>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The name ("PLC1").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The pinned address profile, `id@version` ("siemens-tia-intl@2027.1"; doc 10 §2.5).
    #[schemars(with = "IdentS")]
    pub profile: Ident,
    /// The CPU device; absent when the master sits outside the project (Golden A).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub cpu: Option<Id<Device>>,
    /// The symbol policy (doc 10 §2.2, §3.4, FR-PLC-017, FR-PLC-047); absent: no automatic symbol, strict IEC
    /// identifiers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbols: Option<SymbolPolicy>,
    /// Who owns which field group in the exchange with PLC software (doc 10 §2.2, FR-PLC-033); absent: the default
    /// owners and nothing protected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ownership: Option<Ownership>,
    /// The stamp of the last exchange (doc 10 §4.2); its snapshot is in `exchange/<id>.jsonl`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange: Option<ExchangeStamp>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The ownership of a configuration project's exchanged fields (doc 10 §2.2, §3.9, FR-PLC-033): the owner of each
/// configurable field group, and the protection of the PLC-owned groups.
///
/// An older 0.1 build keeps the member verbatim and does not enforce the protection: a PLC-owned field it edits is
/// an ordinary difference in the next import's three-way merge, so the file stays consistent.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Ownership {
    /// The owner of the module start addresses; absent: the PLC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_starts: Option<Owner>,
    /// The owner of symbols and comments; absent: shared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Owner>,
    /// The date of the first applied PLC import: from it on, an edit of a PLC-owned group is refused until
    /// `plc.unlock` lifts the protection. Absent: nothing is protected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported: Option<Date>,
    /// The groups whose protection `plc.unlock` lifted; the next applied import protects them again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unlocked: Vec<FieldGroup>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl Ownership {
    /// The owner of `group` under `ownership` (a configuration project's member; absent: the defaults): module
    /// starts are the PLC's and symbols shared unless set otherwise; the PLC's own data is always the PLC's.
    pub fn owner(ownership: Option<&Ownership>, group: &FieldGroup) -> Owner {
        let set = match group {
            FieldGroup::ModuleStarts => ownership.and_then(|o| o.module_starts.clone()),
            FieldGroup::Symbols => ownership.and_then(|o| o.symbols.clone()),
            FieldGroup::PlcData | FieldGroup::Unknown(_) => None,
        };
        set.unwrap_or(match group {
            FieldGroup::Symbols => Owner::Shared,
            _ => Owner::Plc,
        })
    }

    /// The date since which `group` is protected from edits: the first applied import, when the PLC owns the group
    /// and `plc.unlock` has not lifted the protection.
    pub fn protected_since(ownership: Option<&Ownership>, group: &FieldGroup) -> Option<Date> {
        let o = ownership?;
        (Ownership::owner(ownership, group) == Owner::Plc && !o.unlocked.contains(group))
            .then_some(o.imported)
            .flatten()
    }
}

/// Who owns a field group in an exchange (doc 10 §2.2).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "Owner")]
pub enum Owner {
    /// The schematic: a change of the PLC software is listed and applied only when accepted.
    Ecad,
    /// The PLC software: its change is applied, and the field is protected after the first import.
    Plc,
    /// Both: a change of one side applies, a change of both is a conflict.
    Shared,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A group of exchanged fields with one owner (doc 10 §2.2).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "FieldGroup")]
pub enum FieldGroup {
    /// The start addresses of the modules.
    ModuleStarts,
    /// Symbols and comments.
    Symbols,
    /// What the PLC software configures: firmware, IP addresses, PROFINET names and DP station addresses.
    PlcData,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The stamp of a configuration project's last exchange with PLC software (doc 10 §2.2, §4.2).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ExchangeStamp {
    /// The format of the exchanged file.
    pub format: ExchangeFormat,
    /// The format's version ("1.4.0" of AR APC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// SHA-256 of the exchanged file.
    pub file_sha256: Sha256,
    /// Whether the file was written or read.
    pub direction: ExchangeDirection,
    /// The exchange's document date.
    pub date: Date,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The format of an exchanged file (doc 10 §4.1).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ExchangeFormat")]
pub enum ExchangeFormat {
    /// AutomationML AR APC.
    Aml,
    /// The TIA Portal "PLC Tags" workbook.
    TiaXlsx,
    /// PLCopen XML.
    Plcopen,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The direction of an exchange.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ExchangeDirection")]
pub enum ExchangeDirection {
    /// The file was written.
    Export,
    /// The file was read and applied.
    Import,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// How a configuration project derives and validates channel symbols (doc 10 §2.2, §3.4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SymbolPolicy {
    /// The template of the automatic symbol (`{target.process_tag|target.dt}`, `{target.dt}_{pin}`); absent: a
    /// channel without a manual symbol has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Strict IEC 61131-3 identifiers (the default): the automatic symbol is cleaned into one, and EW-PLC-005
    /// reports a symbol that is not one.
    #[serde(default = "r#true", skip_serializing_if = "is_true")]
    pub strict_iec: bool,
    /// The PLC tag table the symbols are exported to (the TIA "PLC Tags" table name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub tag_table: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Station and module aspects

/// The `PlcStation` aspect (doc 10 §2.3): an addressable participant of a configuration project.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlcStation {
    /// The configuration project.
    #[schemars(with = "UuidS")]
    pub config_project: Id<PlcConfigProject>,
    /// The station's role.
    pub role: StationRole,
    /// Address-allocation order; absent: by ID (never by tag, page or bus address).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    /// The firmware version, which the PLC software owns (doc 10 §2.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub firmware: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The role of a station (doc 10 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "StationRole")]
pub enum StationRole {
    /// A CPU.
    Cpu,
    /// A head station of remote I/O.
    HeadStation,
    /// An I/O device.
    IoDevice,
    /// A valve terminal.
    ValveTerminal,
    /// An IO-Link master.
    IoLinkMaster,
    /// A drive.
    Drive,
    /// A placeholder for a partner outside the project (FR-BUS-005).
    ExternalPartner,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The `PlcModule` aspect (doc 10 §2.3): a card or sub-module of a station, on the station device or on a
/// descendant of it. Its I/O widths and maps come from the part's `plc` section.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PlcModule {
    /// The slot; default positional (the rank among the station's modules by `Device.position`).
    #[serde(default, skip_serializing_if = "is_default")]
    pub slot: SlotPath,
    /// The start address per direction; default `auto` for both.
    #[serde(default, skip_serializing_if = "IoStarts::is_auto")]
    pub start: IoStarts,
    /// Which part use of the device supplies the PLC section (index into `Device.parts`); absent: the first
    /// part use whose part has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_use: Option<u8>,
    /// Created with the station; not deletable alone (FR-PLC-003).
    #[serde(default, skip_serializing_if = "is_false")]
    pub integrated: bool,
    /// The safety data of an F-module (doc 10 §2.9, FR-PLC-041); absent: a standard module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety: Option<SafetyModule>,
    /// The type identification of a module without part data (one created from a PLC import); a module with a
    /// part takes its order number from the part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_ident: Option<TypeIdent>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The type identification of a module (doc 10 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct TypeIdent {
    /// The order number ("6ES7 521-1BL00-0AB0").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub order_number: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The slot of a module (doc 10 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "SlotPath")]
pub enum SlotPath {
    /// The 1-based rank among the station's I/O-module descendants by `Device.position` (the default).
    #[default]
    Positional,
    /// Rack, slot and subslot.
    Rack(RackSlot),
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// An authored rack position.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RackSlot {
    /// The rack.
    pub rack: u16,
    /// The slot.
    pub slot: u16,
    /// The subslot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subslot: Option<u16>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One start per direction (doc 10 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct IoStarts {
    /// The input start.
    #[serde(default, skip_serializing_if = "is_default")]
    pub input: AddrSpec,
    /// The output start.
    #[serde(default, skip_serializing_if = "is_default")]
    pub output: AddrSpec,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl IoStarts {
    /// Whether both starts are `auto` and nothing else is set.
    pub fn is_auto(&self) -> bool {
        *self == IoStarts::default()
    }
}

/// An address specification (doc 10 §2.5): derived, deliberate, excluded or open.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "AddrSpec")]
pub enum AddrSpec {
    /// Module start plus the address-map offset (the default).
    #[default]
    Auto,
    /// A deliberate value; every deviation from the derived value is reported (EW-PLC-012).
    Manual(PlcAddress),
    /// Not addressed ("do not include in addressing").
    Excluded,
    /// A typed open item; prints "TBD" and blocks release (EW-PLC-015).
    Tbd,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A canonical PLC address (doc 10 §2.5), rendered by the configuration project's profile.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlcAddress {
    /// The area.
    pub area: PlcArea,
    /// The access size; default a bit.
    #[serde(default, skip_serializing_if = "is_default")]
    pub size: PlcSize,
    /// The byte.
    pub byte: u32,
    /// The bit, for a bit address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit: Option<u8>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The area of a PLC address.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PlcArea")]
pub enum PlcArea {
    /// Inputs.
    I,
    /// Outputs.
    Q,
    /// Markers.
    M,
    /// Peripheral inputs.
    Pi,
    /// Peripheral outputs.
    Pq,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The access size of a PLC address.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PlcSize")]
pub enum PlcSize {
    /// One bit (the default).
    #[default]
    Bit,
    /// A byte.
    Byte,
    /// A word.
    Word,
    /// A double word.
    #[serde(rename = "dword")]
    DWord,
    /// A long word.
    #[serde(rename = "lword")]
    LWord,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Channel

/// The logical I/O unit (doc 10 §2.4, rank 37, in the shard of its module device). Its ID is
/// `v5(module, "channel", number)` ([`ew_ids::v5::plc_channel_id`]). Address, state, designation, symbol,
/// target and texts are derived where the record leaves them open (doc 10 §3.1–§3.5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Channel {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Channel>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The module device.
    #[schemars(with = "UuidS")]
    pub module: Id<Device>,
    /// The 0-based channel number.
    pub number: u16,
    /// Manual designation; absent: the part's pattern over `number`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub designation: Option<Ident>,
    /// The channel kind.
    pub kind: ChannelKind,
    /// The address specification; default `auto`.
    #[serde(default, skip_serializing_if = "is_default")]
    pub address: AddrSpec,
    /// An explicit state; absent: derived (doc 10 §3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<ChannelState>,
    /// Manual symbol; absent: none before the symbol templates (1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub symbol: Option<Ident>,
    /// The PLC comment.
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub comment: MlText,
    /// The signal CP: a module pin, or the served coil's internal CP (no connection row).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<Endpoint>,
    /// A manual target override (FR-PLC-054).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub target: Option<Id<Function>>,
    /// The data type of the PLC tag in the profile vocabulary ("Bool", "Int"); absent: the part's
    /// (`ChannelSpec.data_type`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub data_type: Option<Ident>,
    /// The signal range of an analogue channel ("4..20 mA").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub signal_range: Option<Ident>,
    /// The mode of a `multi_function` channel (an IO-Link master port, doc 10 §3.8); absent: IO-Link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PortMode>,
    /// The safety data of an F-channel (doc 10 §2.9, FR-PLC-041); absent: a standard channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub safety: Option<SafetyChannel>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The kind of a channel (doc 10 §2.4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ChannelKind")]
pub enum ChannelKind {
    /// Digital input.
    Di,
    /// Digital output.
    Do,
    /// Analogue input.
    Ai,
    /// Analogue output.
    Ao,
    /// Counter.
    Counter,
    /// IO-Link port.
    IoLinkPort,
    /// Power supply.
    PowerSupply,
    /// A multi-function port whose mode (`Channel.mode`) selects IO-Link, DI, DQ or deactivated (doc 10 §3.8,
    /// FR-BUS-012).
    MultiFunction,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The mode of a multi-function channel (doc 10 §3.8, FR-BUS-012).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PortMode")]
pub enum PortMode {
    /// IO-Link communication (the default): the port occupies the part's process-data window.
    #[default]
    IoLink,
    /// A digital input (SIO).
    Di,
    /// A digital output (SIO).
    Dq,
    /// Deactivated: no process data.
    Deactivated,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Safety data (doc 10 §2.9)

/// The safety data of an F-module (doc 10 §2.9): its PROFIsafe addresses and watchdog. SIL and PL are never
/// calculated (FR-PLC-041).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct SafetyModule {
    /// The F-destination address (`F_Dest_Add`); absent: missing (EW-SAF-001).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f_dest: Option<u16>,
    /// The F-source address (`F_Source_Add`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f_source: Option<u16>,
    /// The F-watchdog time in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f_wd_ms: Option<u16>,
    /// The F-destination range of the device description (`1..=65534`); absent: any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<RangeU16>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An inclusive range of 16-bit numbers.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RangeU16 {
    /// The lowest value.
    pub min: u16,
    /// The highest value.
    pub max: u16,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl RangeU16 {
    /// Whether `value` lies in the range.
    pub fn contains(&self, value: u16) -> bool {
        self.min <= value && value <= self.max
    }
}

/// The safety data of an F-channel (doc 10 §2.9).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SafetyChannel {
    /// One-out-of-one or one-out-of-two evaluation.
    pub evaluation: Evaluation,
    /// The other channel of a 1oo2 pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub pair: Option<Id<Channel>>,
    /// The requirement as a field ("SIL 2", "PL d"); never calculated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub requirement: Option<Ident>,
    /// The module's test-pulse (sensor-supply) pin that the channel's sensor must be supplied from (EW-SAF-003);
    /// absent: the channel needs none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_pulse: Option<Endpoint>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The evaluation of a safety channel.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(rename = "Evaluation")]
pub enum Evaluation {
    /// One-out-of-one.
    #[serde(rename = "1oo1")]
    OneOoOne,
    /// One-out-of-two: the channel and its `pair` read one two-channel sensor.
    #[serde(rename = "1oo2")]
    OneOoTwo,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// An explicit channel state (doc 10 §3.2); `used` is never stored, it is derived from the target.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ChannelState")]
pub enum ChannelState {
    /// Spare.
    Spare,
    /// Reserved.
    Reserved,
    /// Deactivated.
    Deactivated,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Part data

/// The PLC section of a part (doc 10 §2.6, level L2 part data). The earth, bus ports and the `meta` confidence are
/// additive and stay in `extra` until their work packages.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PlcSection {
    /// What the part is in the PLC model.
    pub role: PlcRole,
    /// The I/O ranges the part occupies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub io: Vec<IoRange>,
    /// The channels a module instantiates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<ChannelSpec>,
    /// The channel map: channel number → bit offset in the module's range (doc 10 §2.6, "n", "16n" or a
    /// list); absent: linear.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bit_of: Option<MapSpec>,
    /// The state of channels neither bound nor claimed (doc 10 §3.2); absent: spare.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unbound: Option<ChannelState>,
    /// The solenoid-position map of a valve-terminal module (doc 10 §2.6 `[plc.positions]`, §3.2 step 4,
    /// §3.3 normaliser N2); absent: the module serves no valve positions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions: Option<PositionMap>,
    /// The channel → pin map of a module with physical pins (doc 10 §3.3, normaliser N3): channel number →
    /// the designation of the module's connection point ("X2:{n+1}", or a list); absent: the channels have
    /// no pins (internal channels, such as the solenoid outputs of a valve-terminal module).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_of: Option<PinMap>,
    /// The supply groups of the part's power-supply pins (doc 10 §2.6, FR-PLC-024, WP-S1.21 D13): each names the
    /// L+ and M pins of one group, so that the pins take the potential roles `L+` and `M` when neither the part
    /// pattern nor the function definition gives them one (`ew_derive::FunctionCps`). Appended after `pin_of`, so a
    /// part without groups writes the bytes it wrote before (the member order fixes a copy's hash).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supply_groups: Vec<SupplyGroup>,
    /// The slots a module of this part may occupy (doc 10 §2.3, FR-PLC-028, EW-PLC-011); empty: any slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_slots: Vec<u16>,
    /// The limits of a station of this part (`Max_Module`, the maximum I/O lengths; EW-PLC-011).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<StationLimits>,
    /// The port window of an IO-Link master's `multi_function` channels (doc 10 §3.8, FR-BUS-012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iolink: Option<IoLinkWindow>,
    /// The output current ratings of a module (EW-PLC-014, FR-PLC-048).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ratings: Option<OutputRatings>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The limits of a station part (doc 10 §2.8 `Max_Module` and the maximum I/O lengths; EW-PLC-011).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct StationLimits {
    /// The most modules the station takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_modules: Option<u16>,
    /// The most input bytes of all its modules together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_bytes: Option<u32>,
    /// The most output bytes of all its modules together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_bytes: Option<u32>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The port window of an IO-Link master part (doc 10 §3.8): the process-data bytes each port occupies in IO-Link
/// mode, the port class and the longest port cable.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct IoLinkWindow {
    /// The input bytes per port.
    pub input_bytes: u16,
    /// The output bytes per port.
    pub output_bytes: u16,
    /// The port class (A or B).
    pub class: PortClass,
    /// The longest port cable in metres (IO-Link: 20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cable_m: Option<u16>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The class of an IO-Link port (FR-BUS-012): class B adds a separate actuator supply.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PortClass")]
pub enum PortClass {
    /// Class A.
    A,
    /// Class B.
    B,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The output current ratings of a module part, in milliamperes (EW-PLC-014).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct OutputRatings {
    /// The rating of one output channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_ma: Option<u32>,
    /// The rating of one load supply group (the output channels the group feeds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_ma: Option<u32>,
    /// The rating of the whole module.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_ma: Option<u32>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One supply group of a PLC part (doc 10 §2.6 `[[plc.supply]]`, FR-PLC-024, WP-S1.21 D13): the L+ and M pins that
/// feed the electronics or a load group, and the channels the group feeds. The earth pin (`[plc.earth]`) is not part
/// of a group and stays in [`PlcSection::extra`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SupplyGroup {
    /// What the group supplies.
    pub kind: SupplyKind,
    /// The group's L+ and M pins.
    pub cps: SupplyPins,
    /// The channels the group feeds; absent: all of them ([`SupplyGroup::feeds`]). Read by no rule at the MVP
    /// (EW-PLC-014 is 1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feeds: Option<SupplyFeeds>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl SupplyGroup {
    /// The channels the group feeds, `all` when the member is absent.
    pub fn feeds(&self) -> SupplyFeeds {
        self.feeds.clone().unwrap_or(SupplyFeeds::All)
    }
}

/// The L+ and M pins of a supply group, by the designation of the part's connection point ("4", "X1:2").
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SupplyPins {
    /// The pin that takes the potential role `L+`.
    #[schemars(with = "IdentS")]
    pub l_plus: Ident,
    /// The pin that takes the potential role `M`.
    #[schemars(with = "IdentS")]
    pub m: Ident,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// What a supply group supplies (FR-PLC-024).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "SupplyKind")]
pub enum SupplyKind {
    /// The module's electronics.
    Electronics,
    /// A load group (the outputs' load voltage).
    Load,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The channels a supply group feeds.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "SupplyFeeds")]
pub enum SupplyFeeds {
    /// Every channel of the module (the default).
    All,
    /// The input channels.
    Inputs,
    /// The output channels.
    Outputs,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The solenoid-position map of a valve-terminal module part (doc 10 §2.6 `[plc.positions]`): the channel
/// that each solenoid of the valve at position index `p` drives. The position index is 0-based: the valve
/// child at `Device.position` = q sits at p = q − 1 (doc 11 §3.2). Part 533361 of Golden A carries the test
/// input H1, `count = 4`, solenoid 14 → `2p`, solenoid 12 → `2p+1` (doc 20 §11.2).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PositionMap {
    /// The number of valve positions (p = 0 … count − 1).
    pub count: u16,
    /// One entry per solenoid name, in part-data order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub map: Vec<SolenoidChannel>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One entry of a [`PositionMap`]: the channel of one solenoid as a function of the position index.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SolenoidChannel {
    /// The solenoid ("14", "12"), matched against the designation of the valve's coil CP.
    #[schemars(with = "IdentS")]
    pub solenoid: Ident,
    /// The channel number at position index p: a `p` expression ("2p", "2p+1") or a list indexed by p.
    pub channel: MapSpec,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The PLC role of a part.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PlcRole")]
pub enum PlcRole {
    /// A station (CPU, head, bus node).
    Station,
    /// An I/O module.
    Module,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One I/O range of a part.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct IoRange {
    /// The direction.
    pub dir: IoDir,
    /// The width in bits.
    pub bits: u32,
    /// The address class; default digital.
    #[serde(default, skip_serializing_if = "is_default")]
    pub class: IoClass,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The direction of an I/O range.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "IoDir")]
pub enum IoDir {
    /// Inputs.
    In,
    /// Outputs.
    Out,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The address class of an I/O range (the profile keeps one allocation cursor per class).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "IoClass")]
pub enum IoClass {
    /// Digital (the default).
    #[default]
    Digital,
    /// Analogue.
    Analog,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The channels a module part instantiates.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ChannelSpec {
    /// How many (numbers 0 … count − 1).
    pub count: u16,
    /// Their kind.
    pub kind: ChannelKind,
    /// The data type in the profile vocabulary ("Bool").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub data_type: Option<Ident>,
    /// The automatic designation pattern over `{n}` ("{n}", "DO {n}").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub designation: Option<String>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A map of the part-data map language (doc 10 §2.6): an expression string (`"n"`, `"16n"`, `"2p+1"`)
/// or an explicit list. The schema keeps the expression as text; the PLC derivation parses it and reports
/// an unparsable one.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum MapSpec {
    /// An expression.
    Expr(String),
    /// An explicit list, indexed by the variable.
    List(Vec<u32>),
    /// A form of a newer format, kept verbatim.
    Unknown(Value),
}

impl Serialize for MapSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            MapSpec::Expr(s) => s.serialize(serializer),
            MapSpec::List(l) => l.serialize(serializer),
            MapSpec::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for MapSpec {
    /// A string is an expression and an array a list; a one-member object is a form of a newer format
    /// ([`MapSpec::Unknown`]). A list whose elements do not parse is a malformed variant.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = buffer(deserializer)?;
        match &value {
            Value::String(s) => Ok(MapSpec::Expr(s.clone())),
            Value::Array(_) => match de_here::<Vec<u32>>(&value) {
                Ok(l) => Ok(MapSpec::List(l)),
                Err(e) => malformed("list", e, value, MapSpec::Unknown),
            },
            Value::Object(map) if map.len() == 1 => Ok(MapSpec::Unknown(kept_unknown(value))),
            Value::Object(_) => Err(not_a_variant()),
            _ => Err(raise(invalid("expected a map expression or a list"))),
        }
    }
}

impl JsonSchema for MapSpec {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("MapSpec")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A part-data map (doc 10 §2.6): an expression \"<k>n[+<c>]\" or \"<k>p[+<c>]\", or an explicit list.",
            "anyOf": [
                {"type": "string"},
                {"type": "array", "items": {"type": "integer", "minimum": 0, "maximum": 4_294_967_295_u64}}
            ]
        })
    }
}

/// A channel → pin map (doc 10 §3.3, normaliser N3): the designation of the connection point a channel is
/// wired to, by channel number. A pattern is text in which each `{<expression>}` is a map-language
/// expression in `n` (doc 10 §2.6: `{n}`, `{n+1}`, `{2n+1}`), so `"X2:{n+1}"` gives `X2:1` for channel 0;
/// a list gives the designation of channel n at index n. The schema keeps the text; the derivation and N3
/// expand it and treat a pattern they cannot expand as giving no pin.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PinMap {
    /// A designation pattern.
    Pattern(String),
    /// An explicit list of designations, indexed by the channel number.
    List(Vec<Ident>),
    /// A form of a newer format, kept verbatim.
    Unknown(Value),
}

impl Serialize for PinMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            PinMap::Pattern(s) => s.serialize(serializer),
            PinMap::List(l) => l.serialize(serializer),
            PinMap::Unknown(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for PinMap {
    /// A string is a pattern and an array a list of designations; a one-member object is a form of a newer
    /// format ([`PinMap::Unknown`]). A list whose elements are not designations is a malformed variant.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = buffer(deserializer)?;
        match &value {
            Value::String(s) => Ok(PinMap::Pattern(s.clone())),
            Value::Array(_) => match de_here::<Vec<Ident>>(&value) {
                Ok(l) => Ok(PinMap::List(l)),
                Err(e) => malformed("list", e, value, PinMap::Unknown),
            },
            Value::Object(map) if map.len() == 1 => Ok(PinMap::Unknown(kept_unknown(value))),
            Value::Object(_) => Err(not_a_variant()),
            _ => Err(raise(invalid(
                "expected a pin pattern or a list of pin designations",
            ))),
        }
    }
}

impl JsonSchema for PinMap {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("PinMap")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A channel → pin map (doc 10 §3.3): a designation pattern with `{<expression in n>}` placeholders (\"X2:{n+1}\"), or an explicit list of pin designations.",
            "anyOf": [
                {"type": "string"},
                {"type": "array", "items": {"type": "string", "minLength": 1}}
            ]
        })
    }
}

// ---------------------------------------------------------------------------------------------------------
// Device link

/// A functional relation between two devices (doc 03 §7, doc 11 §3.1, rank 42, in the shard of `from`):
/// valve -201U3-Q1 drives field device -203Q1, whose process tag the valve's texts and the PLC overview
/// track.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DeviceLink {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<DeviceLink>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The driving or linked device.
    #[schemars(with = "UuidS")]
    pub from: Id<Device>,
    /// The driven or linked device.
    #[schemars(with = "UuidS")]
    pub to: Id<Device>,
    /// The relation.
    pub kind: DeviceLinkKind,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The kind of a device link.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "DeviceLinkKind")]
pub enum DeviceLinkKind {
    /// `from` drives `to` (a valve and its field device).
    Drives,
    /// A mechanical linkage (FR-SYM-014).
    Linkage,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

open_enum!(
    StationRole,
    SlotPath,
    AddrSpec,
    PlcArea,
    PlcSize,
    ChannelKind,
    ChannelState,
    PlcRole,
    IoDir,
    IoClass,
    DeviceLinkKind,
    SupplyKind,
    SupplyFeeds,
    PortMode,
    Evaluation,
    PortClass,
    Owner,
    FieldGroup,
    ExchangeFormat,
    ExchangeDirection,
);
