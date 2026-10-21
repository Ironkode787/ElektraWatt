// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Devices with their typed aspects and functions (ADR-0006, doc 03 §7, §10–§12): `device` (rank 31) and
//! `function` (32).
//!
//! Strings are NFC (doc 05 §3.1): `MlText` and `Ident` members reject other text, and plain `String`
//! members are normalised with every other string by the loader's [`normalize_nfc`](crate::normalize_nfc)
//! pass before typed reading.

use ew_ids::Id;
use ew_mltext::{Ident, MlText};
use ew_units::Nm;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::json::{Extra, de_extra, is_default, is_false, is_v1, v1};
use crate::library::{FnDef, Side};
use crate::proxy::{IdentS, MlTextS, NmS, UuidS};
use crate::set::{KeyedList, keyed};
use crate::track::{ValueDe, buffer, de_member, open_enum, raise};
use crate::value::{PartRef, PartUse, SourceRef, StructureAssignment, TechValue};
use crate::{ExternalRef, SchemaError};

// ---------------------------------------------------------------------------------------------------------
// Device

/// Everything with one tag; the single owner of parts (ADR-0006, doc 03 §7, rank 31). The printed tag is
/// derived from `tag`, the numbering scheme and the main representation; it is stored nowhere.
///
/// Aspects follow the known members, flattened in the order of doc 05 §3.1: `strip`, `terminal`, `cable`,
/// `assembly`, `plc_station`, `plc_module`, then `bus_node` and `valve_terminal`. Typed: `strip`, `terminal`,
/// `cable`, `plc_station` and `plc_module`; `assembly` is held verbatim in its own slot so that it keeps its
/// place before `plc_station`; `bus_node` and `valve_terminal` arrive with their work packages and are kept
/// in `extra` until then, which places them after the typed aspects.
/// `user_props` has no field-level definition yet and is kept in `extra` too.
///
/// Deserialisation takes the aspects out of the object first and reads each at its own path, because
/// serde reads flattened fields from a buffer that loses the paths of errors inside them.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(rename = "Device")]
pub struct Device {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Device>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The only stored tag data (doc 03 §8.2).
    pub tag: TagSpec,
    /// UI and numbering hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<DeviceKind>,
    /// Parent device (assemblies, child tags).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub parent: Option<Id<Device>>,
    /// Order inside the parent (manifold position).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u32>,
    /// Own structure assignment (FR-PRJ-019).
    #[serde(default, skip_serializing_if = "StructureAssignment::is_empty")]
    pub structure: StructureAssignment,
    /// Part uses; parts attach only to devices.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PartUse>,
    /// Manual function text (the automatic text falls back to the path function text, FR-DEV-011).
    #[serde(default, skip_serializing_if = "MlText::is_empty")]
    #[schemars(with = "MlTextS")]
    pub function_text: MlText,
    /// Process tag, never renumbered (FR-DEV-012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub process_tag: Option<Ident>,
    /// Technical data, keyed by `key` (FR-DEV-009).
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub tech_data: KeyedList<TechValue>,
    /// Lifecycle state; default used.
    #[serde(default, skip_serializing_if = "is_default")]
    pub state: DeviceState,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// The typed aspects, flattened.
    #[serde(flatten)]
    pub aspects: DeviceAspects,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl Serialize for Device {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Device::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Device {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut value = buffer(deserializer)?;
        let aspects = match &mut value {
            Value::Object(map) => DeviceAspects::take(map).map_err(raise)?,
            _ => DeviceAspects::default(),
        };
        let mut device = Device::deserialize(ValueDe(&value)).map_err(|e| raise(e.0))?;
        device.aspects = aspects;
        Ok(device)
    }
}

impl DeviceAspects {
    /// Removes the known aspects from a device object and reads them.
    fn take(map: &mut Map<String, Value>) -> Result<DeviceAspects, SchemaError> {
        fn aspect<T: DeserializeOwned>(
            map: &mut Map<String, Value>,
            key: &str,
        ) -> Result<Option<T>, SchemaError> {
            match map.remove(key) {
                Some(value) => de_member(key, &value),
                None => Ok(None),
            }
        }
        Ok(DeviceAspects {
            strip: aspect(map, "strip")?,
            terminal: aspect(map, "terminal")?,
            cable: aspect(map, "cable")?,
            assembly: map.remove("assembly"),
            plc_station: aspect(map, "plc_station")?,
            plc_module: aspect(map, "plc_module")?,
        })
    }
}

/// The stored tag data (doc 03 §8.2).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TagSpec {
    /// The letter code ("F", "X").
    #[schemars(with = "IdentS")]
    pub letter: Ident,
    /// The counter, assigned once at commit by online numbering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counter: Option<u32>,
    /// The numbering scheme; default the project's.
    #[serde(default, skip_serializing_if = "is_default")]
    pub scheme: TagScheme,
    /// A deliberate override of the derived product designation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<FrozenTag>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl TagSpec {
    /// A tag with a letter and an optional counter under the default scheme.
    pub fn new(letter: Ident, counter: Option<u32>) -> TagSpec {
        TagSpec {
            letter,
            counter,
            scheme: TagScheme::Default,
            frozen: None,
            extra: Extra::new(),
        }
    }
}

/// The numbering scheme of a tag.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TagScheme")]
pub enum TagScheme {
    /// The project's active scheme (the default).
    #[default]
    Default,
    /// A named scheme.
    Named(#[schemars(with = "IdentS")] Ident),
    /// Parent designation, separator, letter and counter (FR-NUM-013).
    Child,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A frozen product designation (doc 03 §8.5): "S51" printed without the product sign.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FrozenTag {
    /// The product designation text.
    #[schemars(with = "IdentS")]
    pub text: Ident,
    /// Whether pages print the "-" sign; full designations always carry it.
    #[serde(default, skip_serializing_if = "is_default")]
    pub sign: TagSign,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Whether a frozen tag prints its product sign on pages.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TagSign")]
pub enum TagSign {
    /// "-S51" (the default).
    #[default]
    Shown,
    /// "S51".
    Hidden,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A device's UI and numbering hint. Doc 03 enumerates only these kinds so far; others are kept as
/// [`DeviceKind::Unknown`] until the list is decided.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "DeviceKind")]
pub enum DeviceKind {
    /// Power supply.
    Psu,
    /// Terminal.
    Terminal,
    /// Blank manifold or valve-terminal position (doc 11 §2.7).
    BlankingPlate,
    /// A kind of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Device lifecycle state (FR-DEV-026).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "DeviceState")]
pub enum DeviceState {
    /// In use (the default).
    #[default]
    Used,
    /// Spare.
    Spare,
    /// Reserved.
    Reserved,
    /// Not fitted.
    NotFitted,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The typed aspects of a device (ADR-0006), flattened into the device record in this order.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct DeviceAspects {
    /// Terminal strip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strip: Option<Strip>,
    /// Terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<Terminal>,
    /// Cable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cable: Option<Cable>,
    /// The assembly aspect (doc 03 §6, doc 11 §2.7), kept verbatim until it is typed; it is written in its
    /// doc 05 §3.1 place, before `plc_station`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<Value>,
    /// PLC station (doc 10 §2.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plc_station: Option<crate::PlcStation>,
    /// PLC module (doc 10 §2.3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plc_module: Option<crate::PlcModule>,
}

/// The `Strip` aspect (doc 03 §11).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Strip {
    /// What terminals print; default the designation.
    #[serde(default, skip_serializing_if = "is_default")]
    pub label_mode: LabelMode,
    /// Terminal designation scheme (FR-NUM-024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub designation_scheme: Option<Ident>,
    /// Accessories in mounting order (FR-TRM-016).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accessories: Vec<StripAccessory>,
    /// Where the strip tag prints; default once per segment.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tag_print: TagPrint,
    /// Range captions ("Pneumatics", "Spare"; FR-TRM-010).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<TerminalGroup>,
    /// The terminal side facing the field; default side 2 (FR-TRM-012).
    #[serde(default = "side2", skip_serializing_if = "is_side2")]
    pub field_side: Side,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

fn side2() -> Side {
    Side::Side2
}

fn is_side2(side: &Side) -> bool {
    *side == Side::Side2
}

/// What the terminals of a strip print.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "LabelMode")]
pub enum LabelMode {
    /// The terminal designation (the default).
    #[default]
    Designation,
    /// The terminal's function text ("1+", FR-TRM-021).
    FunctionText,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// One strip accessory placed after terminal `after_seq`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct StripAccessory {
    /// The terminal sequence number it follows.
    pub after_seq: u32,
    /// The accessory kind.
    pub kind: AccessoryKind,
    /// The part.
    pub part: PartRef,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Kind of a strip accessory.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "AccessoryKind")]
pub enum AccessoryKind {
    /// End plate.
    EndPlate,
    /// End clamp (end bracket).
    EndClamp,
    /// Partition plate.
    Partition,
    /// Marker carrier.
    Marker,
    /// Cover.
    Cover,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Where a strip prints its tag (FR-TRM-011).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "TagPrint")]
pub enum TagPrint {
    /// Once per strip segment (the default).
    #[default]
    PerSegment,
    /// Once per page.
    PerPage,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A caption over a range of terminals.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct TerminalGroup {
    /// First terminal sequence number.
    pub from_seq: u32,
    /// Last terminal sequence number.
    pub to_seq: u32,
    /// The caption.
    #[schemars(with = "MlTextS")]
    pub label: MlText,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The `Terminal` aspect (doc 03 §11): a terminal is a device.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Terminal {
    /// The strip device.
    #[schemars(with = "UuidS")]
    pub strip: Id<Device>,
    /// Drawn and mounting order, independent of the designation (FR-TRM-003).
    pub seq: u32,
    /// The designation, unique within the strip (D-11, D-12).
    #[schemars(with = "IdentS")]
    pub designation: Ident,
    /// Levels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levels: Vec<LevelSpec>,
    /// Bonded to the mounting rail (PE terminals, FR-TRM-017).
    #[serde(default, skip_serializing_if = "is_false")]
    pub rail_bonded: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One level of a multi-level terminal.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LevelSpec {
    /// Level number.
    pub level: u8,
    /// Role; default main.
    #[serde(default, skip_serializing_if = "is_default")]
    pub role: LevelRole,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The role of a terminal level.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "LevelRole")]
pub enum LevelRole {
    /// Main level (the default).
    #[default]
    Main,
    /// Auxiliary level.
    Aux,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The `Cable` aspect (doc 03 §12).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct Cable {
    /// Where the length comes from; default missing (EW-CORE-033).
    #[serde(default, skip_serializing_if = "is_default")]
    pub length: LengthSource,
    /// How cores are identified (FR-CBL-006).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_id_mode: Option<CoreIdMode>,
    /// Pre-assembled cable (FR-CBL-013).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pre_assembled: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The source of a cable length (FR-CBL-009). The committed sources `Estimated` and `Routed` (1.0) are
/// additive variants of a later format.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "LengthSource")]
pub enum LengthSource {
    /// No length yet (the default); blocks release.
    #[default]
    Missing,
    /// Entered by hand.
    Manual(#[schemars(with = "NmS")] Nm),
    /// From the cable part (pre-assembled length).
    FromPart,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// How the cores of a cable are identified.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "CoreIdMode")]
pub enum CoreIdMode {
    /// By number.
    Number,
    /// By colour.
    Colour,
    /// By number and colour.
    Both,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Function

/// One logical element (doc 03 §7, rank 32): a pole set, a contact, a coil, a PSU, an interruption point.
/// Its connection points are instantiated from the definition or the part's pattern; only overrides are
/// stored. `device` is `None` if and only if the kind is `Ip` (ADR-0006 amendment 1).
///
/// Not in v0: `bus_port` (bus, doc 03 §14) and `CpOverride.closure` (fluid ports, doc 03 §15).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Function {
    /// The record ID: UUIDv7, or `v5(device, template_index)` from a part's function template.
    #[schemars(with = "UuidS")]
    pub id: Id<Function>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The device; `None` only for interruption points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub device: Option<Id<Device>>,
    /// The function definition copy.
    #[schemars(with = "UuidS")]
    pub def: Id<FnDef>,
    /// Main or auxiliary; default main (exactly one main per device).
    #[serde(default, skip_serializing_if = "is_default")]
    pub role: FnRole,
    /// Normal, interruption point or black box; default normal.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: FunctionKind,
    /// CP overrides, keyed by `idx` (FR-DEV-008).
    #[serde(default, skip_serializing_if = "KeyedList::is_empty")]
    pub cp_overrides: KeyedList<CpOverride>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Main or auxiliary function.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "FnRole")]
pub enum FnRole {
    /// The main function (the default).
    #[default]
    Main,
    /// An auxiliary function.
    Aux,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The kind of a function.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "FunctionKind")]
pub enum FunctionKind {
    /// An ordinary function (the default).
    #[default]
    Normal,
    /// An interruption point.
    Ip(IpSpec),
    /// A black box whose CPs come from the part's pattern.
    BlackBox,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// An interruption point (doc 03 §10).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct IpSpec {
    /// The pairing name ("L-1").
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// Signal direction, independent of the arrow glyph (FR-XRF-009).
    pub direction: IpDirection,
    /// The star source of its name in star mode.
    #[serde(default, skip_serializing_if = "is_false")]
    pub star_source: bool,
    /// Chain order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<i32>,
    /// An external connection point's target (FR-XRF-012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub external: Option<Id<ExternalRef>>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Signal direction of an interruption point.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "IpDirection")]
pub enum IpDirection {
    /// Source.
    Source,
    /// Target.
    Target,
    /// Undirected.
    Undirected,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// An override of one instantiated connection point (FR-DEV-008).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CpOverride {
    /// The CP index.
    pub idx: u16,
    /// Printed designation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub designation: Option<Ident>,
    /// Alias ("1 / L1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub alias: Option<Ident>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<MlTextS>")]
    pub description: Option<MlText>,
    /// Unused by design.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unused: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

keyed!(CpOverride, idx: u16);

open_enum!(
    TagScheme,
    TagSign,
    DeviceKind,
    DeviceState,
    LabelMode,
    AccessoryKind,
    TagPrint,
    LevelRole,
    LengthSource,
    CoreIdMode,
    FnRole,
    FunctionKind,
    IpDirection,
);
