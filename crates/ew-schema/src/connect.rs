// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Connectivity records (ADR-0007, doc 03 §9, §12, doc 08 §2): `connection` (rank 34), `junction` (35),
//! `conductor` (36), `potential_def` (38) and `bond` (41).
//!
//! Strings are NFC (doc 05 §3.1): `MlText` and `Ident` members reject other text, and plain `String`
//! members are normalised with every other string by the loader's [`normalize_nfc`](crate::normalize_nfc)
//! pass before typed reading.

use ew_ids::Id;
use ew_mltext::Ident;
use ew_units::{Dec3, Nm, Orient2, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra, is_default, is_false, is_v1, v1};
use crate::proxy::{Dec3S, IdentS, NmS, Orient2S, Pt2S, UuidS};
use crate::track::open_enum;
use crate::value::{ColourSpec, Csa, Endpoint, PartRef, SourceRef};
use crate::{Device, Page};

// ---------------------------------------------------------------------------------------------------------
// Connection

/// One explicit logical edge with a route (ADR-0007, doc 03 §9, rank 34). Page-owned when drawn; a
/// page-less connection (strip-editor bridge, internal link) lives in the shard of the device owning `a`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Connection {
    /// The record ID; also the wire ID of the physical wire it carries.
    #[schemars(with = "UuidS")]
    pub id: Id<Connection>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// First endpoint.
    pub a: Endpoint,
    /// Second endpoint.
    pub b: Endpoint,
    /// The page it is drawn on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub page: Option<Id<Page>>,
    /// Orthogonal route in nm, in model order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<Pt2S>")]
    pub route: Vec<Pt2>,
    /// The connection kind; default a wire.
    #[serde(default, skip_serializing_if = "is_default")]
    pub kind: ConnectionKind,
    /// Explicit per-connection values (the CDP, level 7).
    #[serde(default, skip_serializing_if = "WireOverrides::is_empty")]
    pub overrides: WireOverrides,
    /// The wire number; default unassigned.
    #[serde(default, skip_serializing_if = "is_default")]
    pub number: NumberSpec,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The kind of a connection (doc 08 §2.3). `Fluid(FluidLine)` (doc 11) and `SingleLine` (1.0) are additive
/// variants of their work packages; until then a file's value is kept as [`ConnectionKind::Unknown`], which
/// joins no net (doc 05 §10.3 rule 3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "ConnectionKind")]
pub enum ConnectionKind {
    /// One conductor (the default).
    #[default]
    Wire,
    /// A cable core or shield.
    Core(#[schemars(with = "UuidS")] Id<Conductor>),
    /// A terminal bridge; joins potentials, never a wire.
    Bridge(BridgeSpec),
    /// A single-line bus connection between bus-port CPs.
    Bus(BusLine),
    /// Excluded from nets.
    Overview,
    /// A link between CPs of two functions of one part that no FnDef declares.
    Internal,
    /// A kind of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A terminal bridge (doc 08 §2.3): the connections of one group are one bridge object.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct BridgeSpec {
    /// Bridge kind.
    pub kind: BridgeKind,
    /// Group number on the strip.
    pub group: u16,
    /// The shaft.
    pub shaft: u8,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Kind of a terminal bridge.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "BridgeKind")]
pub enum BridgeKind {
    /// Plug-in bridge.
    PlugIn,
    /// Saddle jumper.
    Saddle,
    /// Wire bridge.
    Wire,
    /// Bridge between levels.
    InterLevel,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A bus line and the cable that carries it (FR-CBL-008).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct BusLine {
    /// The cable device.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub cable: Option<Id<Device>>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Wire properties (doc 08 §4.1): the values one level of the inheritance chain may set.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct WireProps {
    /// Cross-section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csa: Option<Csa>,
    /// Colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// Wire part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_part: Option<PartRef>,
    /// Insulation type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub insulation: Option<Ident>,
    /// Wire function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub function: Option<Ident>,
    /// Shielded and twisted flags.
    #[serde(default, skip_serializing_if = "WireFlags::is_empty")]
    pub flags: WireFlags,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl WireProps {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == WireProps::default()
    }
}

/// The explicit per-connection values (doc 03 §9, the CDP), rendered as a wire marker on request.
///
/// Not in v0: `ends` (per-end overrides, doc 13 §2.3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct WireOverrides {
    /// Cross-section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csa: Option<Csa>,
    /// Colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// Wire part; on the first bridge of a group, the bridge part (level-6 data).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_part: Option<PartRef>,
    /// Insulation type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub insulation: Option<Ident>,
    /// Wire function.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub function: Option<Ident>,
    /// Shielded and twisted flags.
    #[serde(default, skip_serializing_if = "WireFlags::is_empty")]
    pub flags: WireFlags,
    /// Show the override as a wire marker.
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_marker: bool,
    /// Manual wire length on the carrier connection (doc 12 §9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub length: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl WireOverrides {
    /// Whether nothing is overridden.
    pub fn is_empty(&self) -> bool {
        *self == WireOverrides::default()
    }
}

/// Shielded and twisted flags of a wire.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct WireFlags {
    /// Shielded.
    #[serde(default, skip_serializing_if = "is_false")]
    pub shielded: bool,
    /// Twisted.
    #[serde(default, skip_serializing_if = "is_false")]
    pub twisted: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl WireFlags {
    /// Whether no flag is set.
    pub fn is_empty(&self) -> bool {
        *self == WireFlags::default()
    }
}

/// The wire number of a connection (doc 03 §8.7).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "NumberSpec")]
pub enum NumberSpec {
    /// Not numbered; prints "?" (the default).
    #[default]
    Unassigned,
    /// A counter assigned by the numbering rule: `{"assigned": {"counter": n}}`.
    Assigned(AssignedNumber),
    /// A frozen number (FR-NUM-011).
    Frozen(#[schemars(with = "IdentS")] Ident),
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The payload of [`NumberSpec::Assigned`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AssignedNumber {
    /// The counter.
    pub counter: u32,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl AssignedNumber {
    /// The counter `counter` with no extra members.
    pub fn new(counter: u32) -> AssignedNumber {
        AssignedNumber {
            counter,
            extra: Extra::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Junction

/// An explicit T-node (ADR-0007 amendment 2, doc 03 §9, rank 35). The sequence is the wiring order; the
/// style is a display choice.
///
/// Not in v0: `fitting` (fluid tees, doc 11 §5.1).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Junction {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Junction>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The page.
    #[schemars(with = "UuidS")]
    pub page: Id<Page>,
    /// Position.
    #[schemars(with = "Pt2S")]
    pub at: Pt2,
    /// Dot or bevel.
    pub style: JunctionStyle,
    /// Star or chain; absent means the style's default (Dot: star, Bevel: chain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wiring: Option<JunctionWiring>,
    /// The connections in wiring order; arm 1 is the feed arm.
    #[schemars(with = "Vec<UuidS>")]
    pub sequence: Vec<Id<Connection>>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// How a junction is drawn.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "JunctionStyle")]
pub enum JunctionStyle {
    /// A dot.
    Dot,
    /// A bevelled corner: `{"bevel": {"orient": …}}`.
    Bevel(BevelSpec),
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The payload of [`JunctionStyle::Bevel`].
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct BevelSpec {
    /// Orientation of the bevel; default `r0`.
    #[serde(default, skip_serializing_if = "is_default")]
    #[schemars(with = "Orient2S")]
    pub orient: Orient2,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The physical wires behind a junction (doc 03 §9.2).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "JunctionWiring")]
pub enum JunctionWiring {
    /// The first CP is wired to every other CP.
    Star,
    /// Consecutive CPs are wired.
    Chain,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Conductor

/// One cable core or shield (doc 03 §12, rank 36), in the cable's shard.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Conductor {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Conductor>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The cable device.
    #[schemars(with = "UuidS")]
    pub cable: Id<Device>,
    /// Core identification ("1", "BK", "A").
    #[schemars(with = "IdentS")]
    pub core: Ident,
    /// Core colour; absent while unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// Core cross-section; absent while unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csa: Option<Csa>,
    /// A shield rather than a core.
    #[serde(default, skip_serializing_if = "is_false")]
    pub shield: bool,
    /// Twisted pair number (FR-CBL-019).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<u8>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Potential definition and bond

/// Names and types a potential (doc 03 §9, doc 08 §2.2, rank 38). The golden potentials L-S … N-55S are
/// naming-only definitions (`ptype` absent).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct PotentialDef {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<PotentialDef>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The definition point.
    pub at: Endpoint,
    /// The potential name.
    #[schemars(with = "IdentS")]
    pub name: Ident,
    /// The type; absent means naming only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ptype: Option<PotentialType>,
    /// Voltage (V).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub voltage: Option<Dec3>,
    /// AC/DC, frequency and earthing.
    #[serde(default, skip_serializing_if = "PotentialEnv::is_empty")]
    pub env: PotentialEnv,
    /// Circuit class; derived when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circuit_class: Option<CircuitClass>,
    /// Wire properties at the potential level (level 3).
    #[serde(default, skip_serializing_if = "WireProps::is_empty")]
    pub props: WireProps,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Potential types (doc 03 §9.1, FR-CON-010).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PotentialType")]
pub enum PotentialType {
    /// Phase L1.
    L1,
    /// Phase L2.
    L2,
    /// Phase L3.
    L3,
    /// Single-phase line.
    L,
    /// Neutral.
    N,
    /// Protective earth.
    Pe,
    /// Combined PE and N.
    Pen,
    /// Functional earth.
    Fe,
    /// DC positive.
    LPlus,
    /// DC negative.
    LMinus,
    /// DC mid or 0 V.
    M,
    /// Signal.
    Signal,
    /// Undefined.
    Undefined,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Circuit classes (doc 03 §9.1, FR-CON-022).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "CircuitClass")]
pub enum CircuitClass {
    /// Mains.
    Mains,
    /// Control.
    Control,
    /// Analogue.
    Analogue,
    /// Bus.
    Bus,
    /// Protective earth.
    Pe,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The electrical environment of a potential definition.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PotentialEnv {
    /// AC or DC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ac_dc: Option<AcDc>,
    /// Frequency (Hz).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub frequency: Option<Dec3>,
    /// PELV, SELV or floating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earthing: Option<PotentialEarthing>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl PotentialEnv {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == PotentialEnv::default()
    }
}

/// AC or DC.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "AcDc")]
pub enum AcDc {
    /// Alternating current.
    Ac,
    /// Direct current.
    Dc,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// Earthing of an extra-low-voltage potential.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "PotentialEarthing")]
pub enum PotentialEarthing {
    /// Protective extra-low voltage.
    Pelv,
    /// Safety extra-low voltage.
    Selv,
    /// Floating.
    Floating,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// An intentional link of two potentials that does not merge them (doc 03 §9.6, rank 41).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Bond {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Bond>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// First endpoint.
    pub a: Endpoint,
    /// Second endpoint.
    pub b: Endpoint,
    /// Bond kind.
    pub kind: BondKind,
    /// The physical wire that realises the bond (W27 for PE-0VDC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub via: Option<Id<Connection>>,
    /// Import provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// Bond kinds.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(remote = "Self")]
#[schemars(rename = "BondKind")]
pub enum BondKind {
    /// The one 0 V–PE bond of a PELV system.
    #[serde(rename = "pelv_0v_pe")]
    Pelv0vPe,
    /// FE to PE.
    #[serde(rename = "fe_pe")]
    FePe,
    /// Equipotential bonding.
    #[serde(rename = "equipotential")]
    Equipotential,
    /// Shield bonding.
    #[serde(rename = "shield")]
    Shield,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

open_enum!(
    ConnectionKind,
    BridgeKind,
    NumberSpec,
    JunctionStyle,
    JunctionWiring,
    PotentialType,
    CircuitClass,
    AcDc,
    PotentialEarthing,
    BondKind,
);
