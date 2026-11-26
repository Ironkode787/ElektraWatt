// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The domain sections of a part (FR-PRT-010, doc 07 §2.5): `Part.sections` and the section types, each owned by its
//! area (the cable cores, the PLC and bus sections, the carrier sections, the fluid section and the sections of the
//! parts a wire consumes).

use ew_mltext::Ident;
use ew_units::{Dec3, Nm};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hole::Gland;
use crate::json::{Extra, de_extra, is_false};
use crate::layout::Hinge;
use crate::library::{FluidPortRole, MountKind};
use crate::proxy::{Dec3S, IdentS, NmS};
use crate::track::open_enum;
use crate::value::{ColourSpec, Csa, LibRef};

/// Domain sections of a part (FR-PRT-010). Typed: the cable cores, the PLC section, the carrier sections of
/// FR-PRT-024 (enclosure, duct, rail, mounting plate), the bus and fluid sections, the sections of the parts a wire
/// consumes (wire, ferrule, marker, FR-PRT-035) and a cable's and a gland's; the other sections stay in `extra`.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PartSections {
    /// Cores of a cable part, from which conductors are generated (FR-CBL-004).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cable_cores: Vec<CableCore>,
    /// The PLC section (doc 10 §2.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plc: Option<crate::PlcSection>,
    /// The enclosure section: outer and wall dimensions and the mounting plate (doc 12 §2.5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enclosure: Option<EnclosureSection>,
    /// The duct section: inner dimensions and slot pitch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duct: Option<DuctSection>,
    /// The rail section: profile and stock length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail: Option<RailSection>,
    /// The mounting-plate section: its dimensions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mounting_plate: Option<PlateSection>,
    /// The bus section: the part's bus interfaces (doc 10 §2.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<crate::BusSection>,
    /// The fluid section: the solenoids a valve part declares (doc 11 §2.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fluid: Option<FluidSection>,
    /// The wire section of a wire part (FR-PRT-035): the stock a wire is cut from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<WireSection>,
    /// The ferrule section of a ferrule part (FR-PRT-035).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ferrule: Option<FerruleSection>,
    /// The marker section of a marker part (FR-PRT-035).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<MarkerSection>,
    /// The cable section of a cable part: its outer diameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cable: Option<CableSection>,
    /// The gland section of a gland part (doc 12 §3.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gland: Option<GlandSection>,
    /// Unknown members (other sections).
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The wire section of a part (FR-PRT-035, doc 07 §2.5 "The parts a wire consumes").
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct WireSection {
    /// The cross-section in mm².
    #[schemars(with = "Dec3S")]
    pub csa: Dec3,
    /// The colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// The conductor class of IEC 60228 (1, 2, 5, 6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<u8>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The ferrule section of a part (FR-PRT-035): the cross-sections it takes, a twin ferrule, a ring lug's stud.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FerruleSection {
    /// The smallest cross-section it takes, in mm².
    #[schemars(with = "Dec3S")]
    pub csa_min: Dec3,
    /// The largest cross-section it takes, in mm².
    #[schemars(with = "Dec3S")]
    pub csa_max: Dec3,
    /// A twin ferrule: two wires in one.
    #[serde(default, skip_serializing_if = "is_false")]
    pub twin: bool,
    /// An insulated ferrule.
    #[serde(default, skip_serializing_if = "is_false")]
    pub insulated: bool,
    /// The sleeve length: the strip length the wire list prints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub sleeve: Option<Nm>,
    /// The colour the fitter takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// The stud of a ring lug (`M4`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub stud: Option<Ident>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The marker section of a part (FR-PRT-035): what it marks and how much text it holds.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MarkerSection {
    /// What the marker marks.
    pub marks: MarkTarget,
    /// The lines of text it holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<u8>,
    /// The characters per line it holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chars: Option<u8>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// What a marker part marks.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "MarkTarget")]
pub enum MarkTarget {
    /// A wire.
    Wire,
    /// A terminal.
    Terminal,
    /// A device.
    Device,
    /// A cable.
    Cable,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The fluid section of a part (doc 11 §2.6). The solenoids of a valve part, the k-th being the k-th template entry on
/// a FnDef of contact kind `coil`, and the 1.0 members, each optional and additive within
/// format 0.1 (ADR-0011 amendment 6); `overview` stays in `extra`.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FluidSection {
    /// The solenoids the part declares ("14", "12").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<IdentS>")]
    pub solenoids: Vec<Ident>,
    /// Physical port data by designation; on a valve module an entry with `position = p` supplies the data of that
    /// port of the valve at position p and wins over the valve part's own entry (EW-FL-006).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ports: Vec<FluidPortData>,
    /// The valve positions of a valve module (occupancy, FI-02); absent: `plc.positions.count`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions: Option<u16>,
    /// A multipin valve terminal's plug (doc 10 §2.3): which pin of the part's CP pattern each solenoid of the
    /// valve at position p is wired through, as `{solenoid, pin}` with `pin` an expression in p (`2p-1`); the plug's
    /// common pin is a pattern CP named in no entry. Empty for any other part. Additive within format 0.1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<SolenoidPin>,
    /// The coil data of a solenoid valve (EW-FL-009).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coil: Option<CoilData>,
    /// The manifold galleries the part taps or separates (doc 11 §5.2); a module without entries is transparent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub galleries: Vec<GalleryData>,
    /// The data of a tube part (doc 11 §5.4 level 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tube: Option<TubeData>,
    /// The data of a fitting part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fitting: Option<FittingData>,
    /// How the part changes the medium it passes (doc 11 §5.3): a regulator or an ejector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<FluidTransform>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One entry of a multipin valve terminal's plug ([`FluidSection::pins`]): the pin of one solenoid as a function of the
/// position index p (0 … count − 1), as a PLC module's position map gives its channel.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct SolenoidPin {
    /// The solenoid ("14", "12").
    #[schemars(with = "IdentS")]
    pub solenoid: Ident,
    /// The pin's number at position index p: a `p` expression ("2p+1") or a list indexed by p.
    pub pin: crate::plc::MapSpec,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The physical data of one fluid port of a part (doc 11 §2.6).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct FluidPortData {
    /// The port designation ("2").
    #[schemars(with = "IdentS")]
    pub designation: Ident,
    /// On a valve module: the valve position whose port this entry describes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<u16>,
    /// The thread ("G1/8").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub thread: Option<Ident>,
    /// The outer diameter of the tube a push-in port takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub push_in_od: Option<Nm>,
    /// The nominal flow (l/min).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub qn: Option<Dec3>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The coil data of a solenoid valve part (doc 11 §2.6).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct CoilData {
    /// The rated voltage (V).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub voltage: Option<Dec3>,
    /// AC or DC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<crate::AcDc>,
    /// The power (W).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub power_w: Option<Dec3>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One gallery of a manifold part (doc 11 §5.2): the gallery, named by the role of the valve ports it joins, the
/// designation of the port of the part that taps it, and whether the part separates it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GalleryData {
    /// The gallery: `supply1`, `exhaust3`, `exhaust5`, `pilot_exhaust82`, `pilot_exhaust84`.
    pub gallery: FluidPortRole,
    /// The designation of the part's port that taps the gallery (a supply plate's "1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub tap: Option<Ident>,
    /// The part closes the gallery: the segment ends before it and a new one starts.
    #[serde(default, skip_serializing_if = "is_false")]
    pub separates: bool,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The data of a tube part (doc 11 §2.6, §5.4 level 3).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct TubeData {
    /// The outer diameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub od: Option<Nm>,
    /// The inner diameter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub id: Option<Nm>,
    /// The material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<crate::TubeMaterial>,
    /// The colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// The pressure rating (bar).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Dec3S>")]
    pub p_rating: Option<Dec3>,
    /// The temperature range (°C).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub t_range: Option<crate::TempRange>,
    /// The smallest bend radius.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub min_bend_radius: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The data of a fitting part (doc 11 §2.6).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct FittingData {
    /// The port thread it screws into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub port_thread: Option<Ident>,
    /// The outer diameter of the tube it takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub tube_od: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// How a part changes the medium that reaches one of its ports (doc 11 §5.3). The path itself is the FnDef's
/// valve model.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "FluidTransform")]
pub enum FluidTransform {
    /// A pressure regulator: the medium at `outlet` is the root medium at `p_set`.
    Regulator {
        /// The outlet port's designation.
        #[schemars(with = "IdentS")]
        outlet: Ident,
        /// The set pressure (bar, gauge).
        #[schemars(with = "Dec3S")]
        p_set: Dec3,
    },
    /// An ejector: the medium at `vacuum` is vacuum from the root medium.
    Ejector {
        /// The vacuum port's designation.
        #[schemars(with = "IdentS")]
        vacuum: Ident,
    },
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The enclosure section of a part (FR-PRT-024, doc 12 §2.5), lengths in nanometres.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct EnclosureSection {
    /// Outer width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// Outer height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub height: Option<Nm>,
    /// Outer depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub depth: Option<Nm>,
    /// Wall thickness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub wall: Option<Nm>,
    /// The mounting plate part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plate: Option<LibRef>,
    /// The doors, left to right (doc 12 §2.8); empty: one door over the whole front, hinged on the left.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doors: Vec<DoorSpec>,
    /// The strip along its doors' and walls' edges where nothing is cut; absent: none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub edge_margin: Option<Nm>,
    /// The clear depth between the plate and the closed door.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub usable_depth: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// One door of an enclosure part ([`EnclosureSection::doors`]).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DoorSpec {
    /// The side it is hinged on.
    pub hinge: Hinge,
    /// Its width; absent: the enclosure's width shared equally between its doors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The cable section of a cable part (doc 12 §2.5).
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct CableSection {
    /// The cable's outer diameter: what its gland must seal (doc 12 §3.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub outer_diameter: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The gland section of a gland part (doc 12 §2.5, §3.9).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct GlandSection {
    /// Its thread.
    pub thread: Gland,
    /// The smallest cable diameter it seals.
    #[schemars(with = "NmS")]
    pub clamp_min: Nm,
    /// The largest cable diameter it seals.
    #[schemars(with = "NmS")]
    pub clamp_max: Nm,
    /// The room its nut takes, across its corners; absent: 1.4 × the thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub nut: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The duct section of a part (FR-PRT-024, doc 12 §2.5), lengths in nanometres.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct DuctSection {
    /// Inner width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub inner_width: Option<Nm>,
    /// Inner height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub inner_height: Option<Nm>,
    /// Slot pitch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub slot_pitch: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The rail section of a part (FR-PRT-024, doc 12 §2.5).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RailSection {
    /// The rail profile.
    pub profile: MountKind,
    /// The stock length in nanometres.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub stock_length: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The mounting-plate section of a part (FR-PRT-024, doc 12 §2.5), lengths in nanometres.
#[derive(Clone, PartialEq, Eq, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub struct PlateSection {
    /// Width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// Height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub height: Option<Nm>,
    /// Thickness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub thickness: Option<Nm>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

impl PartSections {
    /// Whether nothing is set.
    pub fn is_empty(&self) -> bool {
        *self == PartSections::default()
    }
}

/// One core of a cable part.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct CableCore {
    /// Core identification.
    #[schemars(with = "IdentS")]
    pub core: Ident,
    /// Colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<ColourSpec>,
    /// Cross-section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csa: Option<Csa>,
    /// A shield.
    #[serde(default, skip_serializing_if = "is_false")]
    pub shield: bool,
    /// Pair number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<u8>,
    /// Unknown members.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

open_enum!(FluidTransform, MarkTarget);
