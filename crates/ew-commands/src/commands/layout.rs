// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `layout.*` commands (WP-S4.01 D6, doc 12 §3): layout spaces, enclosures with their plate, carriers and
//! placements, each one undo step. No snapping and no push: a placement's pose is given explicitly and checked
//! against its carrier kind's degrees of freedom (`ew_derive::layout`, D5); WP-S4.02 adds the gestures.
//!
//! The deletes are `Bulk` (previewed on a fork first) when they remove or rehome more than [`LAYOUT_BULK_ABOVE`]
//! placements. An occupied carrier goes only through `layout.carrier.delete` with `then` (doc 12 §3.6): every other
//! delete that would take a placement along is refused by the forbid policy of `placement.carrier` (D4).

use std::collections::BTreeSet;

use ew_derive::layout::{
    Fit, PathError, PoseError, PoseOn, Profile, RailAxis, check_path, check_pose, clip_offset, fits_rail,
    plate_frame, rehome_pose,
};
use ew_ids::Id;
use ew_mltext::Ident;
use ew_model::{AnyKey, Model};
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Device, DuctParams, Enclosure, Extra, Function, Kind,
    LayoutSpace, LibCopy, MechData, MountKind, MountingSurface, Open, Part, PartRef, Placement, RailParams,
    StructureNode, SurfaceKind,
};
use ew_units::{Nm, Pose3, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::command::{Command, Outcome};
use crate::error::CmdError;
use crate::meta::{CommandMeta, Cost};
use crate::proxy::{IdentS, NmS, Pose3S, SurfacePt2S, UuidS};
use crate::tx::TxBuilder;

/// The layout deletes are `Bulk` when they remove or rehome more than this many placements (D6).
pub const LAYOUT_BULK_ABOVE: usize = 200;

/// A present member (a value or `null`) as `Some`.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

// ---------------------------------------------------------------------------------------------------------
// Parameter types

/// What a placement sits on (D3): `{"carrier": id}` (a rail) or `{"surface": id}` (the plate itself).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaceOnParam {
    /// A rail (a duct carries no placements).
    Carrier(#[schemars(with = "UuidS")] Id<Carrier>),
    /// The mounting surface itself.
    Surface(#[schemars(with = "UuidS")] Id<MountingSurface>),
}

impl PlaceOnParam {
    /// The stored form.
    pub fn to_ref(self) -> CarrierRef {
        match self {
            PlaceOnParam::Carrier(c) => CarrierRef::Carrier(c),
            PlaceOnParam::Surface(s) => CarrierRef::Surface(s),
        }
    }
}

/// The kind of a new carrier (D3).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CarrierKindParam {
    /// A mounting (DIN) rail.
    Rail,
    /// A wiring duct.
    Duct,
}

/// A carrier's parameters (D3, doc 12 §2.4), tagged by its kind as stored: `{"rail": {"profile": …}}` or
/// `{"duct": {"width": …, "height": …}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CarrierParamsParam {
    /// A rail.
    Rail {
        /// The rail profile: `ts35x7_5`, `ts35x15`, `ts32` or `ts15`.
        profile: MountKind,
    },
    /// A duct.
    Duct {
        /// The width in the surface plane, in nanometres.
        #[schemars(with = "NmS")]
        width: Nm,
        /// The height out of the surface, in nanometres.
        #[schemars(with = "NmS")]
        height: Nm,
        /// The slot pitch, in nanometres.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "Option<NmS>")]
        slot_pitch: Option<Nm>,
        /// Whether the duct has a cover.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cover: Option<bool>,
    },
}

/// What `layout.carrier.delete` does with the carrier's placements (doc 12 §3.6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CarrierThen {
    /// The placements are removed with the carrier.
    RemovePlacements,
    /// Each placement moves to the carrier's surface at the equivalent surface pose.
    RehomeToSurface,
}

// ---------------------------------------------------------------------------------------------------------
// Shared reads and checks

/// `"<tag>"` of a device.
fn tag(tx: &TxBuilder<'_>, device: Id<Device>) -> String {
    crate::words::device(tx, device)
}

/// The part copy of a part use, if the project holds it.
fn part_copy(tx: &TxBuilder<'_>, part: Id<Part>) -> Option<std::sync::Arc<LibCopy<Part>>> {
    tx.get::<LibCopy<Part>>(part)
}

/// The mechanical data of a part copy.
fn mech_of(tx: &TxBuilder<'_>, part: Id<Part>) -> Option<MechData> {
    part_copy(tx, part).and_then(|c| c.item.mech.clone())
}

/// A mount kind as text for refusals ("TS 35×7.5", "screw").
fn mount_label(kind: &MountKind) -> String {
    match kind {
        MountKind::Ts35x7_5 => "TS 35×7.5".into(),
        MountKind::Ts35x15 => "TS 35×15".into(),
        MountKind::Ts32 => "TS 32".into(),
        MountKind::Ts15 => "TS 15".into(),
        MountKind::Screw => "screw-mounted".into(),
        MountKind::BusbarAdapter => "busbar adapter".into(),
        MountKind::DoorCutout => "door cut-out".into(),
        MountKind::OnParent => "mounted on its parent".into(),
        MountKind::None => "not mounted".into(),
        MountKind::Unknown(v) => v.to_string(),
    }
}

/// Whether a device is a strip with terminals (D8 (3)): it is placed as one placement without a part.
fn is_strip_with_terminals(tx: &TxBuilder<'_>, device: &Device) -> bool {
    device.aspects.strip.is_some() && !tx.terminals_of_strip(device.id).is_empty()
}

/// The part use a placement realises when it names none (D6, D8 (5) and (7)): the first part use whose part mounts
/// (a mount kind other than `none` and `on_parent`), else the first whose part has no mount kind (an unknown
/// footprint). `None` for a strip with terminals, a device without parts and one whose parts all mount on their
/// parent or not at all.
fn primary_part(tx: &TxBuilder<'_>, device: &Device) -> Option<PartRef> {
    if is_strip_with_terminals(tx, device) {
        return None;
    }
    let kind_of = |u: &ew_schema::PartUse| mech_of(tx, u.part.copy).and_then(|m| m.mount_kind);
    let mounts = device
        .parts
        .iter()
        .find(|u| kind_of(u).is_some_and(|k| !matches!(k, MountKind::None | MountKind::OnParent)));
    mounts
        .or_else(|| device.parts.iter().find(|u| kind_of(u).is_none()))
        .map(|u| u.part.clone())
}

/// The part a placement realises for the checks: its own, else the device's primary part (an imported `None` still
/// means the primary, D6).
fn realised_part(tx: &TxBuilder<'_>, device: &Device, part: Option<&PartRef>) -> Option<PartRef> {
    match part {
        Some(p) => Some(p.clone()),
        None => primary_part(tx, device),
    }
}

/// The clip point of a placement in its part frame (D5): the part's `rail_offset`, else its centre; a strip's left end
/// at the middle of its tallest terminal; `(0, 0)` for an unknown footprint.
fn placement_clip(tx: &TxBuilder<'_>, device: &Device, part: Option<&PartRef>) -> (i128, i128) {
    if part.is_none() && is_strip_with_terminals(tx, device) {
        let tallest = tx
            .terminals_of_strip(device.id)
            .into_iter()
            .filter_map(|t| tx.get::<Device>(t))
            .filter_map(|t| primary_part(tx, &t))
            .filter_map(|p| mech_of(tx, p.copy).and_then(|m| m.height))
            .max();
        return tallest.map_or((0, 0), |h| (0, i128::from(h.0) / 2));
    }
    let part = realised_part(tx, device, part);
    clip_offset(part.and_then(|p| mech_of(tx, p.copy)).as_ref())
}

/// What a placement on `on` is checked against: the carrier's pose rule and, for a rail, its profile.
struct Target {
    on: PoseOn,
    profile: Option<MountKind>,
}

/// The target of a placement, or a refusal at `at` when the carrier or surface does not exist or cannot carry it.
fn target(tx: &TxBuilder<'_>, name: &'static str, at: &str, on: CarrierRef) -> Result<Target, CmdError> {
    match on {
        CarrierRef::Carrier(c) => {
            let Some(carrier) = tx.get::<Carrier>(c) else {
                return Err(CmdError::invalid_params(
                    name,
                    at,
                    "the carrier is not in the project",
                ));
            };
            match (&carrier.kind, &carrier.params) {
                (CarrierKind::Rail, params) => {
                    let Some(axis) = RailAxis::of(&carrier.path) else {
                        return tx.reject(
                            "the rail's path is not two points on one horizontal or vertical line; edit the rail first",
                        );
                    };
                    let profile = match params {
                        CarrierParams::Rail(r) => Some(r.profile.clone()),
                        _ => None,
                    };
                    Ok(Target {
                        on: PoseOn::Rail(axis),
                        profile,
                    })
                }
                (CarrierKind::Duct, _) => Ok(Target {
                    on: PoseOn::Duct,
                    profile: None,
                }),
                (CarrierKind::Unknown(_), _) => {
                    tx.reject("the carrier is of a kind this build does not know; it carries no placements")
                }
            }
        }
        CarrierRef::Surface(s) => {
            if !tx.exists::<MountingSurface>(s) {
                return Err(CmdError::invalid_params(
                    name,
                    at,
                    "the surface is not in the project",
                ));
            }
            Ok(Target {
                on: PoseOn::Surface,
                profile: None,
            })
        }
        CarrierRef::Unknown(_) => Err(CmdError::invalid_params(
            name,
            at,
            "a placement sits on a carrier or on a surface",
        )),
    }
}

/// Refuses a part whose mount kind does not fit the target rail's profile (doc 12 §3.3: "-102F1 (TS 35) does not fit a
/// TS 32 rail"). A part without a mount kind, a strip and a partless device are not judged here.
fn check_fit(
    tx: &TxBuilder<'_>,
    device: Id<Device>,
    part: Option<&PartRef>,
    target: &Target,
) -> Result<(), CmdError> {
    let Some(profile) = &target.profile else {
        return Ok(());
    };
    let mount = part.and_then(|p| mech_of(tx, p.copy)).and_then(|m| m.mount_kind);
    if fits_rail(mount.as_ref(), profile) == Fit::DoesNotFit {
        let mount = mount.as_ref().map(mount_label).unwrap_or_default();
        return tx.reject(format!(
            "{} ({mount}) does not fit a {} rail",
            tag(tx, device),
            mount_label(profile)
        ));
    }
    Ok(())
}

/// Refuses a pose outside the target's degrees of freedom or off its rail (D5).
fn check_pose_on(tx: &TxBuilder<'_>, target: &Target, pose: &Pose3) -> Result<(), CmdError> {
    check_pose(target.on, pose).or_else(|e: PoseError| tx.reject(e.to_string()))
}

/// The number of placements a delete plan of `keys` would remove (0 when the plan cannot be made: `run` refuses it).
fn placements_removed(model: &Model, keys: &[AnyKey]) -> usize {
    model.delete_plan_many(keys).map_or(0, |plan| {
        plan.iter()
            .filter(|op| op.key().0 == Kind::Placement && op.after().is_none())
            .count()
    })
}

/// `Bulk` above [`LAYOUT_BULK_ABOVE`] placements.
fn bulk_above(placements: usize) -> Cost {
    if placements > LAYOUT_BULK_ABOVE {
        Cost::Bulk
    } else {
        Cost::Interactive
    }
}

/// Checks a carrier's parameters against its kind (`/params`), the rail profile, and a rail part's profile.
fn checked_params(
    tx: &TxBuilder<'_>,
    name: &'static str,
    kind: &CarrierKind,
    part: Option<Id<Part>>,
    params: &CarrierParamsParam,
) -> Result<CarrierParams, CmdError> {
    match (kind, params) {
        (CarrierKind::Rail, CarrierParamsParam::Rail { profile }) => {
            let Some(p) = Profile::of(profile) else {
                return Err(CmdError::invalid_params(
                    name,
                    "/params/rail/profile",
                    "a rail profile is ts35x7_5, ts35x15, ts32 or ts15",
                ));
            };
            let rail = part
                .and_then(|c| part_copy(tx, c))
                .and_then(|c| c.item.sections.rail.clone());
            if let Some(section) = rail {
                if Profile::of(&section.profile).is_some_and(|q| q != p) {
                    return Err(CmdError::invalid_params(
                        name,
                        "/params/rail/profile",
                        format!(
                            "the rail's part has profile {}; the profile comes from its rail section",
                            mount_label(&section.profile)
                        ),
                    ));
                }
            }
            Ok(CarrierParams::Rail(RailParams {
                profile: profile.clone(),
                extra: Extra::new(),
            }))
        }
        (
            CarrierKind::Duct,
            CarrierParamsParam::Duct {
                width,
                height,
                slot_pitch,
                cover,
            },
        ) => {
            for (field, v) in [
                ("width", Some(*width)),
                ("height", Some(*height)),
                ("slot_pitch", *slot_pitch),
            ] {
                if v.is_some_and(|v| v <= Nm::ZERO) {
                    return Err(CmdError::invalid_params(
                        name,
                        format!("/params/duct/{field}"),
                        "the length must be greater than 0",
                    ));
                }
            }
            Ok(CarrierParams::Duct(DuctParams {
                width: *width,
                height: *height,
                slot_pitch: *slot_pitch,
                cover: *cover,
                extra: Extra::new(),
            }))
        }
        (CarrierKind::Unknown(_), _) => Err(CmdError::invalid_params(
            name,
            "/params",
            "the carrier is of a kind this build does not know; its parameters are kept as they are",
        )),
        (CarrierKind::Rail, _) => Err(CmdError::invalid_params(
            name,
            "/params",
            "the parameters of a rail are `rail`",
        )),
        (CarrierKind::Duct, _) => Err(CmdError::invalid_params(
            name,
            "/params",
            "the parameters of a duct are `duct`",
        )),
    }
}

/// Checks a carrier path (`/path`, D5).
fn checked_path(name: &'static str, kind: &CarrierKind, path: &[Pt2]) -> Result<(), CmdError> {
    check_path(kind, path).map_err(|e: PathError| CmdError::invalid_params(name, "/path", e.to_string()))
}

/// Refuses a part copy the project does not hold (`at`).
fn require_part(tx: &TxBuilder<'_>, name: &'static str, at: &str, part: Id<Part>) -> Result<(), CmdError> {
    if tx.exists::<LibCopy<Part>>(part) {
        Ok(())
    } else {
        Err(CmdError::invalid_params(
            name,
            at,
            format!("part copy {part} is not held by the project"),
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Layout spaces

/// `layout.space.create` (`CreateLayoutSpace`, D6): creates the layout space of a location (FR-LAY-001).
///
/// Rejected: a node that does not exist (`/location`) or is not a location; a location that already has a layout space
/// (one per location at the MVP, D3; the refusal names the first). Interactive. Outcome: the new space.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateLayoutSpace {
    /// The location structure node (+J-E-2416).
    #[schemars(with = "UuidS")]
    pub location: Id<StructureNode>,
    /// The display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub name: Option<Ident>,
}

impl Command for CreateLayoutSpace {
    const NAME: &'static str = "layout.space.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(node) = tx.get::<StructureNode>(self.location) else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/location",
                "the structure node is not in the project",
            ));
        };
        if node.aspect != Open::Known(ew_rds::Aspect::Location) {
            return tx.reject(format!(
                "node {} is not a location; a layout space lays out a location",
                node.code.as_str()
            ));
        }
        if let Some(first) = tx
            .iter::<LayoutSpace>()
            .into_iter()
            .find(|s| s.location == self.location)
        {
            let named = first.name.as_ref().map_or_else(
                || first.id.to_string(),
                |n| format!("\"{}\" ({})", n.as_str(), first.id),
            );
            return tx.reject(format!(
                "location {} already has layout space {named}; a location has one layout space",
                node.code.as_str()
            ));
        }
        let id: Id<LayoutSpace> = tx.mint();
        tx.insert(LayoutSpace {
            id,
            v: 1,
            location: self.location,
            name: self.name.clone(),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::LayoutSpace, id.cast())))
    }
}

/// `layout.space.delete` (`DeleteLayoutSpace`, D6): deletes a layout space with its enclosure, surfaces and carriers
/// (the cascades of D4). Refused while a placement sits in it (the forbid policy of `placement.carrier`: empty its
/// carriers with `layout.carrier.delete` first). Interactive; `Bulk` above 200 placements. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteLayoutSpace {
    /// The layout space.
    #[schemars(with = "UuidS")]
    pub space: Id<LayoutSpace>,
}

impl Command for DeleteLayoutSpace {
    const NAME: &'static str = "layout.space.delete";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, model: &Model) -> Cost {
        bulk_above(placements_removed(
            model,
            &[(Kind::LayoutSpace, self.space.cast())],
        ))
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<LayoutSpace>(self.space) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/space",
                "the layout space is not in the project",
            ));
        }
        tx.delete((Kind::LayoutSpace, self.space.cast()))?;
        Ok(Outcome::default())
    }
}

// ---------------------------------------------------------------------------------------------------------
// Enclosures

/// `layout.enclosure.create` (`CreateEnclosure`, D6, FR-LAY-002): places an enclosure device in a layout space and
/// creates its mounting plate.
///
/// The device's first part use whose part carries an enclosure section gives the outer `W × H`; the section's `plate`
/// names the plate part. The `plate` surface's `device` is the first child of the enclosure device (in ID order) that
/// uses the plate part, if any; its frame centres the plate in the enclosure and its usable polygon is the plate
/// rectangle (D5: `ew_derive::layout::plate_frame`; the plate size from its mounting-plate section, else its `mech`).
/// `pose` defaults to the identity, `pe_bond` (the plate's) to false.
///
/// Rejected: a space or device that does not exist (`/space`, `/device`); a space that already has an enclosure (one
/// per space at the MVP, D3); a device whose parts carry no enclosure section ("the device has no enclosure part").
/// Interactive. Outcome: the new enclosure.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateEnclosure {
    /// The layout space.
    #[schemars(with = "UuidS")]
    pub space: Id<LayoutSpace>,
    /// The enclosure device (-UC1), which carries the enclosure part.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// The enclosure's pose in the space; default the identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Pose3S>")]
    pub pose: Option<Pose3>,
    /// Whether the plate is bonded to protective earth; default false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pe_bond: Option<bool>,
}

impl Command for CreateEnclosure {
    const NAME: &'static str = "layout.enclosure.create";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        if !tx.exists::<LayoutSpace>(self.space) {
            return Err(CmdError::invalid_params(
                name,
                "/space",
                "the layout space is not in the project",
            ));
        }
        let Some(device) = tx.get::<Device>(self.device) else {
            return Err(CmdError::invalid_params(
                name,
                "/device",
                "the device is not in the project",
            ));
        };
        if let Some(first) = tx.iter::<Enclosure>().into_iter().find(|e| e.space == self.space) {
            return tx.reject(format!(
                "the layout space already has enclosure {}; a layout space has one enclosure",
                tag(tx, first.device)
            ));
        }
        let Some(section) = device
            .parts
            .iter()
            .filter_map(|u| part_copy(tx, u.part.copy))
            .find_map(|c| c.item.sections.enclosure.clone())
        else {
            return tx.reject(format!(
                "{}: the device has no enclosure part",
                tag(tx, self.device)
            ));
        };
        let plate: Option<Id<Part>> = section
            .plate
            .as_ref()
            .map(|r| ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid()));
        let plate_size = plate.and_then(|p| part_copy(tx, p)).map_or((None, None), |c| {
            match &c.item.sections.mounting_plate {
                Some(s) if s.width.is_some() && s.height.is_some() => (s.width, s.height),
                _ => c.item.mech.as_ref().map_or((None, None), |m| (m.width, m.height)),
            }
        });
        let surface_device = plate.and_then(|p| {
            tx.iter::<Device>()
                .into_iter()
                .find(|d| d.parent == Some(self.device) && d.parts.iter().any(|u| u.part.copy == p))
                .map(|d| d.id)
        });
        let (frame, usable) = plate_frame((section.width, section.height), plate_size);
        let enclosure: Id<Enclosure> = tx.mint();
        tx.insert(Enclosure {
            id: enclosure,
            v: 1,
            space: self.space,
            device: self.device,
            pose: self.pose.unwrap_or(Pose3::IDENTITY),
            extra: Extra::new(),
        })?;
        let surface: Id<MountingSurface> = tx.mint();
        tx.insert(MountingSurface {
            id: surface,
            v: 1,
            enclosure,
            kind: SurfaceKind::Plate,
            device: surface_device,
            frame,
            usable,
            pe_bond: self.pe_bond.unwrap_or(false),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Enclosure, enclosure.cast())))
    }
}

/// `layout.enclosure.delete` (`DeleteEnclosure`, D6): deletes an enclosure with its surfaces and carriers (the cascades
/// of D4); the enclosure device stays. Refused while a placement sits in it (the forbid policy of `placement.carrier`).
/// Interactive; `Bulk` above 200 placements. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteEnclosure {
    /// The enclosure.
    #[schemars(with = "UuidS")]
    pub enclosure: Id<Enclosure>,
}

impl Command for DeleteEnclosure {
    const NAME: &'static str = "layout.enclosure.delete";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, model: &Model) -> Cost {
        bulk_above(placements_removed(
            model,
            &[(Kind::Enclosure, self.enclosure.cast())],
        ))
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<Enclosure>(self.enclosure) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/enclosure",
                "the enclosure is not in the project",
            ));
        }
        tx.delete((Kind::Enclosure, self.enclosure.cast()))?;
        Ok(Outcome::default())
    }
}

// ---------------------------------------------------------------------------------------------------------
// Carriers

/// `layout.carrier.add` (`AddCarrier`, D6): adds a rail or a duct to a mounting surface.
///
/// The path is in the surface frame (`u` right, `v` up) and axis-parallel (D5): a rail two points on one horizontal or
/// vertical line, a duct an orthogonal polyline. A rail's profile comes from its part's rail section when it has one
/// (a different `profile` is refused). Rejected: a surface or part copy that does not exist (`/surface`, `/part`);
/// parameters of the other kind (`/params`); a profile that is not a rail profile, a duct length ≤ 0 (`/params/…`); a
/// path of the wrong number of points, a segment without length or at another angle (`/path`). Interactive. Outcome:
/// the new carrier.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddCarrier {
    /// The mounting surface.
    #[schemars(with = "UuidS")]
    pub surface: Id<MountingSurface>,
    /// `rail` or `duct`.
    pub kind: CarrierKindParam,
    /// The carrier's part copy (counted in the BOM by part 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub part: Option<Id<Part>>,
    /// The path in the surface frame.
    #[schemars(with = "Vec<SurfacePt2S>")]
    pub path: Vec<Pt2>,
    /// The parameters of the kind.
    pub params: CarrierParamsParam,
    /// Whether the carrier is bonded to protective earth; default false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pe_bond: Option<bool>,
}

fn carrier_kind(kind: CarrierKindParam) -> CarrierKind {
    match kind {
        CarrierKindParam::Rail => CarrierKind::Rail,
        CarrierKindParam::Duct => CarrierKind::Duct,
    }
}

impl Command for AddCarrier {
    const NAME: &'static str = "layout.carrier.add";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        if !tx.exists::<MountingSurface>(self.surface) {
            return Err(CmdError::invalid_params(
                name,
                "/surface",
                "the surface is not in the project",
            ));
        }
        if let Some(part) = self.part {
            require_part(tx, name, "/part", part)?;
        }
        let kind = carrier_kind(self.kind);
        let params = checked_params(tx, name, &kind, self.part, &self.params)?;
        checked_path(name, &kind, &self.path)?;
        let id: Id<Carrier> = tx.mint();
        tx.insert(Carrier {
            id,
            v: 1,
            surface: self.surface,
            kind,
            part: self.part.map(PartRef::new),
            path: self.path.clone(),
            params,
            pe_bond: self.pe_bond.unwrap_or(false),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Carrier, id.cast())))
    }
}

/// `layout.carrier.edit` (`EditCarrier`, D6): changes a carrier's surface, part, path, parameters or PE bond.
///
/// Moving it to another surface moves its placements with it (their poses are relative to it), into the other space's
/// layout file when the surface belongs to another space. `part: null` removes the part. The path and parameters are
/// checked as `layout.carrier.add` checks them, against the carrier's kind (which never changes). A placement that a
/// shorter rail or another profile leaves off its rail or unfit is kept and reported by EW-LAY-010. Rejected: a carrier,
/// surface or part copy that does not exist (`/carrier`, `/surface`, `/part`); no member given (at the root); the
/// path or parameters of `layout.carrier.add`. Interactive. Outcome: the carrier.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditCarrier {
    /// The carrier.
    #[schemars(with = "UuidS")]
    pub carrier: Id<Carrier>,
    /// Another mounting surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub surface: Option<Id<MountingSurface>>,
    /// Another part copy; `null` removes the part.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<UuidS>")]
    pub part: Option<Option<Id<Part>>>,
    /// Another path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<SurfacePt2S>>")]
    pub path: Option<Vec<Pt2>>,
    /// Other parameters, of the carrier's kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<CarrierParamsParam>,
    /// Whether the carrier is bonded to protective earth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pe_bond: Option<bool>,
}

impl Command for EditCarrier {
    const NAME: &'static str = "layout.carrier.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        let Some(current) = tx.get::<Carrier>(self.carrier) else {
            return Err(CmdError::invalid_params(
                name,
                "/carrier",
                "the carrier is not in the project",
            ));
        };
        if self.surface.is_none()
            && self.part.is_none()
            && self.path.is_none()
            && self.params.is_none()
            && self.pe_bond.is_none()
        {
            return Err(CmdError::invalid_params(
                name,
                "",
                "give at least one member (`surface`, `part`, `path`, `params`, `pe_bond`)",
            ));
        }
        if let Some(surface) = self.surface {
            if !tx.exists::<MountingSurface>(surface) {
                return Err(CmdError::invalid_params(
                    name,
                    "/surface",
                    "the surface is not in the project",
                ));
            }
        }
        if let Some(Some(part)) = self.part {
            require_part(tx, name, "/part", part)?;
        }
        let part = match self.part {
            Some(p) => p,
            None => current.part.as_ref().map(|p| p.copy),
        };
        let params = match &self.params {
            Some(p) => Some(checked_params(tx, name, &current.kind, part, p)?),
            None => None,
        };
        if let Some(path) = &self.path {
            checked_path(name, &current.kind, path)?;
        }
        let mut next = (*current).clone();
        if let Some(surface) = self.surface {
            next.surface = surface;
        }
        if let Some(p) = self.part {
            next.part = p.map(PartRef::new);
        }
        if let Some(path) = &self.path {
            next.path = path.clone();
        }
        if let Some(params) = params {
            next.params = params;
        }
        if let Some(pe) = self.pe_bond {
            next.pe_bond = pe;
        }
        if next != *current {
            tx.put(next);
        }
        Ok(Outcome::primary((Kind::Carrier, self.carrier.cast())))
    }
}

/// `layout.carrier.delete` (`DeleteCarrier`, D6, doc 12 §3.6): deletes a carrier; the only way to delete an occupied
/// one.
///
/// `then` says what happens to its placements: `remove_placements` removes them; `rehome_to_surface` moves each to the
/// carrier's surface at the equivalent surface pose (the part keeps its place and turn, `ew_derive::layout::rehome_pose`;
/// EW-LAY-010 then reports a rail device off a rail). An empty carrier needs no `then`. Rejected: a carrier that does
/// not exist (`/carrier`); an occupied carrier without `then`; `rehome_to_surface` for a carrier that is not a valid
/// rail or a placement whose pose is off it. Interactive; `Bulk` above 200 placements. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeleteCarrier {
    /// The carrier.
    #[schemars(with = "UuidS")]
    pub carrier: Id<Carrier>,
    /// What happens to its placements: `remove_placements` or `rehome_to_surface`; needed when it has any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub then: Option<CarrierThen>,
}

impl Command for DeleteCarrier {
    const NAME: &'static str = "layout.carrier.delete";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, model: &Model) -> Cost {
        bulk_above(model.indexes().placements_by_carrier().get(&self.carrier).count())
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        let Some(carrier) = tx.get::<Carrier>(self.carrier) else {
            return Err(CmdError::invalid_params(
                name,
                "/carrier",
                "the carrier is not in the project",
            ));
        };
        let placements = tx.placements_on_carrier(self.carrier);
        match (self.then, placements.is_empty()) {
            (_, true) => {}
            (None, false) => {
                return tx.reject(format!(
                    "the carrier holds {} placement(s); give `then`: `remove_placements` or `rehome_to_surface`",
                    placements.len()
                ));
            }
            (Some(CarrierThen::RemovePlacements), false) => {
                let keys: Vec<AnyKey> = placements.iter().map(|p| (Kind::Placement, p.cast())).collect();
                tx.delete_many(&keys)?;
            }
            (Some(CarrierThen::RehomeToSurface), false) => {
                let axis = match carrier.kind {
                    CarrierKind::Rail => RailAxis::of(&carrier.path),
                    _ => None,
                };
                let Some(axis) = axis else {
                    return tx.reject(
                        "only the placements of a rail (two points on one horizontal or vertical line) rehome to \
                         its surface",
                    );
                };
                for p in placements {
                    let row = tx.require::<Placement>(p)?;
                    let device = tx.require::<Device>(row.device)?;
                    let clip = placement_clip(tx, &device, row.part.as_ref());
                    let Some(pose) = rehome_pose(&axis, &row.pose, clip) else {
                        return tx.reject(format!(
                            "the placement of {} has a pose off the rail; move or remove it first",
                            tag(tx, row.device)
                        ));
                    };
                    let surface = carrier.surface;
                    tx.update::<Placement>(p, |pl| {
                        pl.carrier = CarrierRef::Surface(surface);
                        pl.pose = pose;
                    })?;
                }
            }
        }
        tx.delete((Kind::Carrier, self.carrier.cast()))?;
        Ok(Outcome::default())
    }
}

// ---------------------------------------------------------------------------------------------------------
// Placements

/// `layout.place` (`PlaceDevice`, D6, doc 12 §3.3): places a device on a rail or on a mounting surface at an explicit
/// pose (no snapping; WP-S4.02 adds the `mode`).
///
/// The placement always records the part use it realises: the given `part` (a copy the device uses; its use's
/// reference is copied, variant included), else the device's primary mountable part at this moment (the first part
/// use whose part mounts, else the first without a mount kind; D8); a strip with terminals and a device without a part
/// record none. `function` places one function of the device separately.
///
/// Rejected: a device, carrier or surface that does not exist (`/device`, `/carrier`); a function that does not exist
/// or is not the device's (`/function`); a part the device does not use (`/part`); a second placement of one (device,
/// part, function); a part whose mount kind does not fit the rail's profile (the table of `ew_derive::layout`); a pose
/// outside the carrier kind's degrees of freedom (a rail: `t = (s, 0, 0)` turned 0° or 180°; a surface: `t = (u, v,
/// 0)` turned `k · 90°`; a duct carries none) or off the rail (`s ∉ [0, L]`). Interactive. Outcome: the placement.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceDevice {
    /// The device.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// The part copy of the part use to realise; default the primary mountable part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub part: Option<Id<Part>>,
    /// One function of the device placed separately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub function: Option<Id<Function>>,
    /// The rail or surface.
    pub carrier: PlaceOnParam,
    /// The pose relative to the carrier.
    #[schemars(with = "Pose3S")]
    pub pose: Pose3,
}

/// The identity of a placement for the duplicate check: (device, part copy and variant, function).
type PlacementKey = (
    Id<Device>,
    Option<(Id<Part>, Option<Ident>)>,
    Option<Id<Function>>,
);

fn placement_key(device: Id<Device>, part: Option<&PartRef>, function: Option<Id<Function>>) -> PlacementKey {
    (device, part.map(|p| (p.copy, p.variant.clone())), function)
}

impl Command for PlaceDevice {
    const NAME: &'static str = "layout.place";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        let Some(device) = tx.get::<Device>(self.device) else {
            return Err(CmdError::invalid_params(
                name,
                "/device",
                "the device is not in the project",
            ));
        };
        let carrier = self.carrier.to_ref();
        let target = target(tx, name, "/carrier", carrier.clone())?;
        if let Some(f) = self.function {
            if tx
                .get::<Function>(f)
                .is_none_or(|f| f.device != Some(self.device))
            {
                return Err(CmdError::invalid_params(
                    name,
                    "/function",
                    "the function is not one of the device's",
                ));
            }
        }
        let part = match self.part {
            Some(copy) => match device.parts.iter().find(|u| u.part.copy == copy) {
                Some(u) => Some(u.part.clone()),
                None => {
                    return Err(CmdError::invalid_params(
                        name,
                        "/part",
                        format!("{} does not use this part", tag(tx, self.device)),
                    ));
                }
            },
            None => primary_part(tx, &device),
        };
        let key = placement_key(self.device, part.as_ref(), self.function);
        let placed: BTreeSet<PlacementKey> = tx
            .placements_of_device(self.device)
            .into_iter()
            .filter_map(|p| tx.get::<Placement>(p))
            .map(|p| placement_key(p.device, p.part.as_ref(), p.function))
            .collect();
        if placed.contains(&key) {
            return tx.reject(format!(
                "{} is already placed with this part and function; move its placement instead",
                tag(tx, self.device)
            ));
        }
        check_fit(tx, self.device, part.as_ref(), &target)?;
        check_pose_on(tx, &target, &self.pose)?;
        let id: Id<Placement> = tx.mint();
        tx.insert(Placement {
            id,
            v: 1,
            device: self.device,
            function: self.function,
            part,
            carrier,
            pose: self.pose,
            locked: false,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Placement, id.cast())))
    }
}

/// `layout.move` (`MovePlacement`, D6): gives a placement another pose, optionally on another rail or surface.
///
/// The pose is checked as `layout.place` checks it; the fit of the part to a rail's profile only when the carrier
/// changes. A locked placement moves too (the lock excludes it from push, swap and auto-fill, doc 12 §2.3). Rejected: a
/// placement, carrier or surface that does not exist (`/placement`, `/carrier`); an unfit part; a pose outside the
/// carrier kind's degrees of freedom or off the rail. Interactive. Outcome: the placement.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovePlacement {
    /// The placement.
    #[schemars(with = "UuidS")]
    pub placement: Id<Placement>,
    /// Another rail or surface; default the current one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrier: Option<PlaceOnParam>,
    /// The new pose relative to the carrier.
    #[schemars(with = "Pose3S")]
    pub pose: Pose3,
}

impl Command for MovePlacement {
    const NAME: &'static str = "layout.move";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        let Some(current) = tx.get::<Placement>(self.placement) else {
            return Err(CmdError::invalid_params(
                name,
                "/placement",
                "the placement is not in the project",
            ));
        };
        let carrier = self
            .carrier
            .map_or_else(|| current.carrier.clone(), PlaceOnParam::to_ref);
        let target = target(tx, name, "/carrier", carrier.clone())?;
        if carrier != current.carrier {
            let device = tx.require::<Device>(current.device)?;
            let part = realised_part(tx, &device, current.part.as_ref());
            check_fit(tx, current.device, part.as_ref(), &target)?;
        }
        check_pose_on(tx, &target, &self.pose)?;
        let pose = self.pose;
        tx.update::<Placement>(self.placement, |p| {
            p.carrier = carrier;
            p.pose = pose;
        })?;
        Ok(Outcome::primary((Kind::Placement, self.placement.cast())))
    }
}

/// `layout.remove` (`RemovePlacement`, D6): removes a placement; the device becomes unplaced. Rejected: a placement
/// that does not exist (`/placement`). Interactive. Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemovePlacement {
    /// The placement.
    #[schemars(with = "UuidS")]
    pub placement: Id<Placement>,
}

impl Command for RemovePlacement {
    const NAME: &'static str = "layout.remove";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<Placement>(self.placement) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/placement",
                "the placement is not in the project",
            ));
        }
        tx.delete((Kind::Placement, self.placement.cast()))?;
        Ok(Outcome::default())
    }
}

/// `layout.lock` (`LockPlacement`, D6): locks or unlocks a placement (a locked one is excluded from push, swap and
/// auto-fill, doc 12 §2.3). Rejected: a placement that does not exist (`/placement`). Interactive. Outcome: the
/// placement.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LockPlacement {
    /// The placement.
    #[schemars(with = "UuidS")]
    pub placement: Id<Placement>,
    /// Whether it is locked.
    pub locked: bool,
}

impl Command for LockPlacement {
    const NAME: &'static str = "layout.lock";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<Placement>(self.placement) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/placement",
                "the placement is not in the project",
            ));
        }
        let locked = self.locked;
        tx.update::<Placement>(self.placement, |p| p.locked = locked)?;
        Ok(Outcome::primary((Kind::Placement, self.placement.cast())))
    }
}

/// D7: when `part.assign` adds the one copy `new` to `device` (modes `replace` and `replace_all`, the part not used
/// before), every placement of the device whose part copy left the device follows `new`, in the same transaction. A
/// placement without a part (the primary) and one whose copy the device still uses stay.
pub(crate) fn follow_part_change(
    tx: &mut TxBuilder<'_>,
    device: Id<Device>,
    new: Id<Part>,
) -> Result<(), CmdError> {
    let dev = tx.require::<Device>(device)?;
    for p in tx.placements_of_device(device) {
        let Some(row) = tx.get::<Placement>(p) else {
            continue;
        };
        let Some(old) = row.part.as_ref() else {
            continue;
        };
        if dev.parts.iter().any(|u| u.part.copy == old.copy) {
            continue;
        }
        tx.update::<Placement>(p, |pl| pl.part = Some(PartRef::new(new)))?;
    }
    Ok(())
}
