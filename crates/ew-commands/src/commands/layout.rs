// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `layout.*` commands (WP-S4.01 D6, doc 12 §3): layout spaces, enclosures with their plate, carriers and
//! placements, each one undo step. A placement's pose is given explicitly and checked against its carrier kind's
//! degrees of freedom (`ew_derive::layout`, D5); with WP-S4.02's `mode` (D4, D5) `layout.place` and `layout.move`
//! resolve the position on a rail in free, push or swap mode and write every moved neighbour in the same step.
//!
//! The deletes are `Bulk` (previewed on a fork first) when they remove or rehome more than [`LAYOUT_BULK_ABOVE`]
//! placements. An occupied carrier goes only through `layout.carrier.delete` with `then` (doc 12 §3.6): every other
//! delete that would take a placement along is refused by the forbid policy of `placement.carrier` (D4).

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::Q;
use ew_derive::layout::{
    Fit, Mountable, PathError, PoseError, PoseOn, Profile, Quarter, RailAxis, RailExtent, RailItem,
    Resolution, SnapError, SnapMode, Unplaced, bracket_part, carrier_label, check_path, check_pose,
    clip_offset, fits_rail, member_order, member_slot, min_gap, mm, mount_label, next_fit, plate_frame,
    rail_extent, rail_items, rehome_pose, resolve,
};
use ew_derive::{DeviceTag, DrawnPageOrder, PageRef, StructureOf, StructurePath};
use ew_ids::Id;
use ew_mltext::{Ident, natural_key};
use ew_model::{AnyKey, Model};
use ew_rds::Aspect;
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Device, DeviceAspects, DuctParams, Enclosure,
    EnclosureSection, Extra, FnDef, FrozenTag, Function, Kind, LayoutSpace, LibCopy, MechData, MountKind,
    MountingSurface, Open, Page, Part, PartRef, PartUse, Placement, RailParams, Representation, Section,
    Slot, StructureAssignment, StructureNode, SurfaceKind, TagScheme, TagSign, TagSpec,
};
use ew_units::{Dec3, Nm, Pose3, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::command::{Command, Outcome};
use crate::commands::assign::embed_closure;
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

/// How `layout.place` and `layout.move` resolve their position on a rail (WP-S4.02 D4, D5). Without a mode the pose
/// is taken exactly as given.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PlaceMode {
    /// The placement stays where it fits with its minimum gaps, else goes to the nearest position where it fits;
    /// nothing else moves. On a surface: the explicit pose.
    Free,
    /// The placement is inserted; the placements to its right move right as far as their minimum gaps require.
    Push,
    /// The placement exchanges places with the placement under its centre on the same rail.
    Swap,
}

impl PlaceMode {
    fn snap(self) -> SnapMode {
        match self {
            PlaceMode::Free => SnapMode::Free,
            PlaceMode::Push => SnapMode::Push,
            PlaceMode::Swap => SnapMode::Swap,
        }
    }
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
/// TS 32 rail"), and on a surface a part that mounts on its parent or not at all (ID-1106, FT-1031). A part without a
/// mount kind, a strip and a partless device are not judged here.
fn check_fit(
    tx: &TxBuilder<'_>,
    device: Id<Device>,
    part: Option<&PartRef>,
    target: &Target,
) -> Result<(), CmdError> {
    if matches!(target.on, PoseOn::Surface) {
        let copy = part.and_then(|p| part_copy(tx, p.copy));
        if let Some(copy) = copy.filter(|c| {
            c.item
                .mech
                .as_ref()
                .and_then(|m| m.mount_kind.as_ref())
                .is_some_and(|k| matches!(k, MountKind::None | MountKind::OnParent))
        }) {
            return tx.reject(format!(
                "Part {} does not mount on a surface; choose a part that mounts on it.",
                copy.item.part_number.as_str()
            ));
        }
    }
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
        let node = checked_location(tx, Self::NAME, self.location)?;
        if let Some(first) = tx
            .iter::<LayoutSpace>()
            .into_iter()
            .find(|s| s.location == self.location)
        {
            // ID-900: the space in words, by its name, else as the layout space of its location; never by its ID.
            let code = node.code.as_str();
            return tx.reject(match first.name.as_ref() {
                Some(n) => format!(
                    "location {code} already has layout space \"{}\"; a location has one layout space",
                    n.as_str()
                ),
                None => format!(
                    "the layout space of location {code} already exists; a location has one layout space"
                ),
            });
        }
        let id = insert_space(tx, self.location, self.name.clone())?;
        Ok(Outcome::primary((Kind::LayoutSpace, id.cast())))
    }
}

/// The location node of a new layout space (`layout.space.create`'s refusals, shared with
/// `layout.enclosure.create_from_part`): missing at `/location`, another aspect refused.
fn checked_location(
    tx: &TxBuilder<'_>,
    name: &'static str,
    location: Id<StructureNode>,
) -> Result<std::sync::Arc<StructureNode>, CmdError> {
    let Some(node) = tx.get::<StructureNode>(location) else {
        return Err(CmdError::invalid_params(
            name,
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
    Ok(node)
}

/// Inserts a layout space of `location`.
fn insert_space(
    tx: &mut TxBuilder<'_>,
    location: Id<StructureNode>,
    name: Option<Ident>,
) -> Result<Id<LayoutSpace>, CmdError> {
    let id: Id<LayoutSpace> = tx.mint();
    tx.insert(LayoutSpace {
        id,
        v: 1,
        location,
        name,
        extra: Extra::new(),
    })?;
    Ok(id)
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
            // ID-1106 (FT-1033): the refusal names the next step, as the location's does.
            let tag = tag(tx, first.device);
            return tx.reject(format!(
                "the layout space already has enclosure {tag}; a layout space has one enclosure. \
                 Choose another layout space, or edit {tag}"
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
        let enclosure = insert_enclosure(
            tx,
            self.space,
            self.device,
            &section,
            self.pose.unwrap_or(Pose3::IDENTITY),
            self.pe_bond.unwrap_or(false),
        )?;
        Ok(Outcome::primary((Kind::Enclosure, enclosure.cast())))
    }
}

/// The copy ID of the plate part an enclosure section names (`v5(NS_LIBCOPY, library ‖ item)`).
fn plate_copy_id(section: &EnclosureSection) -> Option<Id<Part>> {
    section
        .plate
        .as_ref()
        .map(|r| ew_ids::v5::lib_copy_id(r.library.as_uuid(), r.item.as_uuid()))
}

/// The enclosure of `device` in `space` and its plate surface, as `layout.enclosure.create` makes them (shared with
/// `layout.enclosure.create_from_part`, WP-S4.02 D7): the plate part from the section, its size from its mounting-plate
/// section, else its `mech`; the surface's device the first child of `device` (in ID order) that uses the plate part;
/// the frame centring the plate in the enclosure ([`plate_frame`]).
fn insert_enclosure(
    tx: &mut TxBuilder<'_>,
    space: Id<LayoutSpace>,
    device: Id<Device>,
    section: &EnclosureSection,
    pose: Pose3,
    pe_bond: bool,
) -> Result<Id<Enclosure>, CmdError> {
    let plate = plate_copy_id(section);
    let plate_size = plate.and_then(|p| part_copy(tx, p)).map_or((None, None), |c| {
        match &c.item.sections.mounting_plate {
            Some(s) if s.width.is_some() && s.height.is_some() => (s.width, s.height),
            _ => c.item.mech.as_ref().map_or((None, None), |m| (m.width, m.height)),
        }
    });
    let surface_device = plate.and_then(|p| {
        tx.iter::<Device>()
            .into_iter()
            .find(|d| d.parent == Some(device) && d.parts.iter().any(|u| u.part.copy == p))
            .map(|d| d.id)
    });
    let (frame, usable) = plate_frame((section.width, section.height), plate_size);
    let enclosure: Id<Enclosure> = tx.mint();
    tx.insert(Enclosure {
        id: enclosure,
        v: 1,
        space,
        device,
        pose,
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
        pe_bond,
        extra: Extra::new(),
    })?;
    Ok(enclosure)
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
/// path of the wrong number of points, a segment without length or at another angle (`/path`). `copies` (WP-S4.02 D7,
/// FT-656's pattern) embeds library documents first, in the same transaction, as `device.place` does (`/copies/i`), so
/// that a library rail or duct part can be used directly. Interactive. Outcome: the new carrier.
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
    /// The library documents to embed first (`lib/` JSON form; WP-S4.02 D7): the part and its closure; empty when the
    /// project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
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
        // WP-S4.02 D7: the copies first, so that the part they bring resolves below.
        embed_closure(name, tx, &self.copies)?;
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
// Modes on a rail (WP-S4.02 D4, D5)

/// One placement dropped on a rail with a mode.
struct Dropped<'d> {
    /// The rail and its axis.
    carrier: Id<Carrier>,
    axis: RailAxis,
    /// The device and the part use the placement realises (`None`: a strip as a whole, or the primary part).
    device: &'d Device,
    part: Option<&'d PartRef>,
    /// The placement being moved (`None` for `layout.place`) and its lock.
    placement: Option<Id<Placement>>,
    locked: bool,
    /// The given pose: `t.x` is the intended clip point.
    pose: Pose3,
    mode: PlaceMode,
}

/// What a mode resolved: the placement's pose and each moved neighbour's new pose.
struct Resolved {
    pose: Pose3,
    neighbours: Vec<(Id<Placement>, Pose3)>,
}

/// Records, through the transaction, the reads the rail's items come from (orchestrator ruling 1 of lane B42): the
/// carrier, every placement on it, their devices with their part copies, and the terminals of the strips with theirs.
/// The items themselves are computed on the base snapshot ([`TxBuilder::base_model`], whose reads are not recorded),
/// so these reads make a concurrent edit of a neighbour on the rail conflict at merge (ADR-0008). Called once per
/// command with every device the command places or moves (lane E42, override 2): each one's part copies and its
/// placements (`placements_of_device`) are read too, so a concurrent placement of the same device conflicts as well.
fn note_rail_reads(tx: &TxBuilder<'_>, carrier: Id<Carrier>, devices: &[&Device]) {
    let note_parts = |d: &Device| {
        for u in &d.parts {
            let _ = tx.get::<LibCopy<Part>>(u.part.copy);
        }
        for t in tx.terminals_of_strip(d.id) {
            if let Some(t) = tx.get::<Device>(t) {
                for u in &t.parts {
                    let _ = tx.get::<LibCopy<Part>>(u.part.copy);
                }
            }
        }
    };
    let _ = tx.get::<Carrier>(carrier);
    for device in devices {
        note_parts(device);
        for p in tx.placements_of_device(device.id) {
            let _ = tx.get::<Placement>(p);
        }
    }
    for p in tx.placements_on_carrier(carrier) {
        let Some(pl) = tx.get::<Placement>(p) else {
            continue;
        };
        if let Some(part) = &pl.part {
            let _ = tx.get::<LibCopy<Part>>(part.copy);
        }
        if let Some(d) = tx.get::<Device>(pl.device) {
            note_parts(&d);
        }
    }
}

/// A rail pose at clip point `s` with the turn of `like`, or the refusal of a pose off the rail when `s` leaves the
/// `Nm` range.
fn rail_pose(tx: &TxBuilder<'_>, axis: &RailAxis, s: i128, like: &Pose3) -> Result<Pose3, CmdError> {
    let Ok(x) = i64::try_from(s) else {
        return tx.reject(PoseError::OffRail(s, axis.length).to_string());
    };
    Ok(Pose3 {
        t: [Nm(x), Nm::ZERO, Nm::ZERO],
        q: like.q,
    })
}

/// How a mode's refusal names what it resolves (D5's texts): the rail, whether it swaps, the item, and the devices of
/// placements this command is about to insert (they are not in the transaction yet).
struct Wording<'n> {
    carrier: Id<Carrier>,
    /// The mode resolved (swap words its refusals as swapping).
    mode: PlaceMode,
    /// The dropped item: a device's tag, or "the assembly -201U1" for an assembly's block.
    item: String,
    /// The tag of each placement minted by the command and not inserted yet.
    names: &'n BTreeMap<Id<Placement>, String>,
}

/// D5's refusal of a mode, in words: the rail by its label in a sentence ("rail 1", ruling 4), devices by their tags.
fn snap_text(tx: &TxBuilder<'_>, q: &Q, w: &Wording<'_>, e: SnapError) -> String {
    let item = &w.item;
    let verb = if w.mode == PlaceMode::Swap {
        "Swapping"
    } else {
        "Pushing"
    };
    let placed = |p: Option<Id<Placement>>| match p.and_then(|p| w.names.get(&p)) {
        Some(name) => name.clone(),
        None => p
            .and_then(|p| tx.get::<Placement>(p))
            .map_or_else(|| "a placement".to_owned(), |pl| tag(tx, pl.device)),
    };
    match e {
        SnapError::NoGap { width } => {
            let rail = carrier_label(q, w.carrier).map_or_else(|| "the rail".to_owned(), |l| l.in_sentence());
            format!("No gap on {rail} is wide enough for {item} ({} mm).", mm(width))
        }
        SnapError::MovedLocked => format!("Unlock {item} to push or swap it."),
        SnapError::Locked(p) => format!("{verb} would move {}, which is locked.", placed(p)),
        SnapError::PastEnd {
            neighbour: Some(p),
            by,
        } => format!(
            "{verb} would move {} past the rail end by {} mm.",
            placed(Some(p)),
            mm(by)
        ),
        SnapError::PastEnd { neighbour: None, by } => {
            format!(
                "{} would pass the rail end by {} mm.",
                sentence_start(item),
                mm(by)
            )
        }
        SnapError::SwapAcrossRails => {
            "Swap works within one rail; use push or free to place it there.".to_owned()
        }
        SnapError::SwapNoTarget => "Drop onto a placement to swap with it.".to_owned(),
    }
}

/// A text with its first letter in upper case (a sentence that starts with "the assembly …").
fn sentence_start(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The rail item of a device about to be placed or moved (D4): its extent from [`rail_extent`] (the shape
/// `placement_geometry` places), its bracket width when it is a strip, its lock and its placement; with the extent, whose
/// `clip` turns a clip point into the footprint's left edge (`s0 = s_clip − clip`).
fn drop_item(
    q: &Q,
    device: &Device,
    part: Option<&PartRef>,
    flipped: bool,
    locked: bool,
    placement: Option<Id<Placement>>,
) -> (RailItem, RailExtent) {
    let ext = rail_extent(q, device, part, flipped);
    let item = RailItem {
        s0: 0,
        s1: ext.width,
        strip: ext.strip,
        w: if ext.strip {
            bracket_part(q, device).map(|(_, w)| w)
        } else {
            None
        },
        pad: ext.pad,
        locked,
        placement,
    };
    (item, ext)
}

/// The one resolver over a given item list (D4; lane E42's split of [`resolve_on_rail`]): [`resolve`] with its refusal
/// worded. `layout.place` and `layout.move` pass the rail's items from the base snapshot; `layout.place_assembly`
/// passes its in-memory list, which holds the placements it has resolved so far in the same transaction.
fn resolve_items(
    tx: &TxBuilder<'_>,
    q: &Q,
    items: &[RailItem],
    length: i128,
    moved: RailItem,
    s_left: i128,
    wording: &Wording<'_>,
) -> Result<Resolution, CmdError> {
    resolve(items, length, moved, s_left, wording.mode.snap())
        .or_else(|e| tx.reject(snap_text(tx, q, wording, e)))
}

/// Resolves a placement dropped on a rail with a mode (D4, D5): the rail's items from `rail_occupancy` on the base
/// snapshot (the same [`rail_items`] the tool's ghost reads), the placement's own extent from [`rail_extent`], then
/// [`resolve_items`]; the pose's `t.x` is the intended clip point, turned into the footprint's left edge and back.
///
/// **One placement per transaction** (cycle-42 review part 1, finding 5): the items come from
/// [`TxBuilder::base_model`], which does not see the writes pending in the same transaction. A command that resolves
/// several placements in one transaction (`layout.place_assembly`) keeps its own in-memory item list and calls
/// [`resolve_items`] with it instead.
fn resolve_on_rail(tx: &TxBuilder<'_>, d: &Dropped<'_>) -> Result<Resolved, CmdError> {
    note_rail_reads(tx, d.carrier, &[d.device]);
    let q = Q::new(tx.base_model());
    let items = rail_items(&q, d.carrier);
    let flipped = Quarter::of(d.pose.q) == Some(Quarter::HALF);
    let (moved, ext) = drop_item(&q, d.device, d.part, flipped, d.locked, d.placement);
    let s_left = i128::from(d.pose.t[0].0) - ext.clip;
    let names = BTreeMap::new();
    let wording = Wording {
        carrier: d.carrier,
        mode: d.mode,
        item: tag(tx, d.device.id),
        names: &names,
    };
    let r = resolve_items(tx, &q, &items, d.axis.length, moved, s_left, &wording)?;
    let pose = rail_pose(tx, &d.axis, r.s_left + ext.clip, &d.pose)?;
    let mut neighbours = Vec::new();
    for (p, s0) in r.moved {
        let (Some(row), Some(old)) = (
            tx.get::<Placement>(p),
            items.iter().find(|i| i.placement == Some(p)),
        ) else {
            continue;
        };
        let s = i128::from(row.pose.t[0].0) + (s0 - old.s0);
        neighbours.push((p, rail_pose(tx, &d.axis, s, &row.pose)?));
    }
    Ok(Resolved { pose, neighbours })
}

/// Where a mode applies (D4): `Some((rail, axis, mode))` for a mode on a rail; `None` without a mode, and for free on a
/// surface (the explicit pose); push or swap on a surface is refused. A duct never gets here: its pose check refuses.
fn mode_on_rail(
    tx: &TxBuilder<'_>,
    target: &Target,
    carrier: &CarrierRef,
    mode: Option<PlaceMode>,
) -> Result<Option<(Id<Carrier>, RailAxis, PlaceMode)>, CmdError> {
    let Some(mode) = mode else {
        return Ok(None);
    };
    match (target.on, carrier) {
        (PoseOn::Rail(axis), CarrierRef::Carrier(c)) => Ok(Some((*c, axis, mode))),
        (PoseOn::Surface, _) if mode != PlaceMode::Free => tx.reject("Push and swap work on a rail."),
        _ => Ok(None),
    }
}

/// Writes the new poses of the neighbours a mode moved (in the command's one transaction: one undo step).
fn move_neighbours(tx: &mut TxBuilder<'_>, neighbours: Vec<(Id<Placement>, Pose3)>) -> Result<(), CmdError> {
    for (p, pose) in neighbours {
        tx.update::<Placement>(p, |row| row.pose = pose)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------
// Placements

/// `layout.place` (`PlaceDevice`, D6, doc 12 §3.3): places a device on a rail or on a mounting surface at an explicit
/// pose, or, with `mode` on a rail, where the mode resolves it (WP-S4.02 D4, D5).
///
/// Without `mode` the pose is taken exactly as given. With `mode` on a rail the pose's `t.x` is the intended clip
/// point: free keeps it where the device fits with its minimum gaps (clearance pads, end brackets) else takes the
/// nearest position where it fits; push inserts it and moves the placements to its right, each as far as its minimum
/// gap requires, in the same transaction; swap is refused (the device is on no rail yet). With `mode` on a surface only
/// `free` is accepted and means the explicit pose. The mode's refusals name the rail by its label and the devices by
/// their tags ("No gap on rail 1 is wide enough for F55 (18 mm).", "Pushing would move -102F2, which is locked.",
/// "Pushing would move S51 past the rail end by 20 mm.").
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
    /// The pose relative to the carrier; with `mode` on a rail, `t.x` is the intended clip point.
    #[schemars(with = "Pose3S")]
    pub pose: Pose3,
    /// How the position is resolved on a rail: `free`, `push` or `swap` (WP-S4.02 D5); absent: the pose as given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PlaceMode>,
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
        let (pose, neighbours) = match mode_on_rail(tx, &target, &carrier, self.mode)? {
            None => (self.pose, Vec::new()),
            Some((rail, axis, mode)) => {
                let r = resolve_on_rail(
                    tx,
                    &Dropped {
                        carrier: rail,
                        axis,
                        device: &device,
                        part: part.as_ref(),
                        placement: None,
                        locked: false,
                        pose: self.pose,
                        mode,
                    },
                )?;
                check_pose_on(tx, &target, &r.pose)?;
                (r.pose, r.neighbours)
            }
        };
        move_neighbours(tx, neighbours)?;
        let id: Id<Placement> = tx.mint();
        tx.insert(Placement {
            id,
            v: 1,
            device: self.device,
            function: self.function,
            part,
            carrier,
            pose,
            locked: false,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Placement, id.cast())))
    }
}

/// `layout.move` (`MovePlacement`, D6): gives a placement another pose, optionally on another rail or surface.
///
/// The pose is checked as `layout.place` checks it; the fit of the part to a rail's profile only when the carrier
/// changes. Without `mode`, or with `mode: free`, a locked placement moves too (the lock excludes it from being moved
/// by another placement's push, swap and auto-fill, doc 12 §2.3). With `mode` on a rail the placement is first taken
/// off the rail and resolved as `layout.place` resolves a device (WP-S4.02 D5); `swap` exchanges it with the placement
/// under its centre on the same rail, the segment between them re-laid with every gap kept where it still meets the
/// new minimum gap. Push and swap of a locked placement are refused ("Unlock F55 to push or swap it."), and never move
/// another locked placement. With `part` (WP-S4.02 D7) the placement realises that part use of its device instead
/// (EW-LAY-003's quick fix "Realise the primary part"): checked as `layout.place` checks a part, its fit to a rail's
/// profile checked when the part changes, and with `mode` the resolver reads the new part's extent; without `part`
/// the realised part is unchanged. Rejected: a placement, carrier or surface that does not exist (`/placement`,
/// `/carrier`); a part the device does not use (`/part`); a part that would give the device a second placement of one
/// (device, part, function); an unfit part; a pose outside the carrier kind's degrees of freedom or off the rail; D5's
/// refusals of the mode. Interactive. Outcome: the placement.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MovePlacement {
    /// The placement.
    #[schemars(with = "UuidS")]
    pub placement: Id<Placement>,
    /// Another rail or surface; default the current one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carrier: Option<PlaceOnParam>,
    /// The new pose relative to the carrier; with `mode` on a rail, `t.x` is the intended clip point.
    #[schemars(with = "Pose3S")]
    pub pose: Pose3,
    /// How the position is resolved on a rail: `free`, `push` or `swap` (WP-S4.02 D5); absent: the pose as given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PlaceMode>,
    /// The part copy of another part use of the device to realise (WP-S4.02 D7); absent: the realised part stays.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub part: Option<Id<Part>>,
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
        // WP-S4.02 D7: another part use of the device, checked as `layout.place` checks it.
        let new_part = match self.part {
            None => None,
            Some(copy) => {
                let device = tx.require::<Device>(current.device)?;
                let Some(u) = device.parts.iter().find(|u| u.part.copy == copy) else {
                    return Err(CmdError::invalid_params(
                        name,
                        "/part",
                        format!("{} does not use this part", tag(tx, current.device)),
                    ));
                };
                let key = placement_key(current.device, Some(&u.part), current.function);
                let taken = tx
                    .placements_of_device(current.device)
                    .into_iter()
                    .filter(|p| *p != self.placement)
                    .filter_map(|p| tx.get::<Placement>(p))
                    .any(|p| placement_key(p.device, p.part.as_ref(), p.function) == key);
                if taken {
                    return tx.reject(format!(
                        "{} is already placed with this part and function; move its placement instead",
                        tag(tx, current.device)
                    ));
                }
                Some(u.part.clone())
            }
        };
        let part_changes = new_part.is_some() && new_part != current.part;
        let part = new_part.clone().or_else(|| current.part.clone());
        if carrier != current.carrier || part_changes {
            let device = tx.require::<Device>(current.device)?;
            let realised = realised_part(tx, &device, part.as_ref());
            check_fit(tx, current.device, realised.as_ref(), &target)?;
        }
        check_pose_on(tx, &target, &self.pose)?;
        let (pose, neighbours) = match mode_on_rail(tx, &target, &carrier, self.mode)? {
            None => (self.pose, Vec::new()),
            Some((rail, axis, mode)) => {
                let device = tx.require::<Device>(current.device)?;
                let r = resolve_on_rail(
                    tx,
                    &Dropped {
                        carrier: rail,
                        axis,
                        device: &device,
                        part: part.as_ref(),
                        placement: Some(self.placement),
                        locked: current.locked,
                        pose: self.pose,
                        mode,
                    },
                )?;
                check_pose_on(tx, &target, &r.pose)?;
                (r.pose, r.neighbours)
            }
        };
        move_neighbours(tx, neighbours)?;
        tx.update::<Placement>(self.placement, |p| {
            p.carrier = carrier;
            p.pose = pose;
            if let Some(part) = new_part {
                p.part = Some(part);
            }
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
/// before), the placements of the device whose part copy left the device follow `new`, in the same transaction. A
/// placement without a part (the primary) and one whose copy the device still uses stay. `before` is the device's part
/// copies in its part-use order before the change.
///
/// When the change replaced two or more placed part uses with the one copy `new` (`replace_all`), only the placements
/// of the first replaced part use (in `before`'s order) follow; the others keep their part, which EW-LAY-003 then
/// reports ("otherwise it stays"), so the placement key (device, part, function) stays unique. A follow onto a key
/// that is already taken is skipped for the same reason.
pub(crate) fn follow_part_change(
    tx: &mut TxBuilder<'_>,
    device: Id<Device>,
    new: Id<Part>,
    before: &[Id<Part>],
) -> Result<(), CmdError> {
    let dev = tx.require::<Device>(device)?;
    let mut left = Vec::new();
    let mut taken = BTreeSet::new();
    for p in tx.placements_of_device(device) {
        let Some(row) = tx.get::<Placement>(p) else {
            continue;
        };
        let Some(old) = row.part.as_ref() else {
            continue;
        };
        if old.copy == new {
            taken.insert(row.function);
        }
        if dev.parts.iter().any(|u| u.part.copy == old.copy) {
            continue;
        }
        left.push((p, old.copy, row.function));
    }
    // FIX40b ruling (C40 review part 2, finding 6): only the first replaced part use's placements follow the new copy.
    let Some(first) = before.iter().find(|c| left.iter().any(|(_, copy, _)| copy == *c)) else {
        return Ok(());
    };
    for (p, copy, function) in left {
        if copy != *first || !taken.insert(function) {
            continue;
        }
        tx.update::<Placement>(p, |pl| pl.part = Some(PartRef::new(new)))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------
// Assemblies and bulk placement (WP-S4.02 D6)

/// A bare non-strip item without pads: what a block's outer pads are measured against.
const BARE: RailItem = RailItem {
    s0: 0,
    s1: 0,
    strip: false,
    w: None,
    pad: [0, 0],
    locked: false,
    placement: None,
};

/// One device a bulk command is about to place: its record, the part it realises, its minted placement and its rail
/// item with the extent.
struct Member {
    device: std::sync::Arc<Device>,
    part: Option<PartRef>,
    id: Id<Placement>,
    item: RailItem,
    ext: RailExtent,
}

/// Mints a member's placement and builds its item (turned 0°, unlocked; the part as `layout.place` records it).
fn member(tx: &mut TxBuilder<'_>, q: &Q, device: std::sync::Arc<Device>) -> Member {
    let part = primary_part(tx, &device);
    let id: Id<Placement> = tx.mint();
    let (item, ext) = drop_item(q, &device, part.as_ref(), false, false, Some(id));
    Member {
        device,
        part,
        id,
        item,
        ext,
    }
}

/// The block of `members` laid at their minimum gaps (D6): each member's offset from the block's left edge, and the
/// item the block resolves as: the block's width, its pads the first member's before and the last member's after (a
/// strip's bracket width counts as its pad), not a strip, no placement. One member is its own item.
fn block_of(members: &[Member]) -> (Vec<i128>, RailItem) {
    if let [one] = members {
        return (vec![0], one.item);
    }
    let mut offsets = Vec::with_capacity(members.len());
    let mut end = 0i128;
    let mut prev: Option<&RailItem> = None;
    for m in members {
        let off = prev.map_or(0, |p| end.saturating_add(min_gap(p, &m.item)));
        offsets.push(off);
        end = off.saturating_add(m.item.width());
        prev = Some(&m.item);
    }
    let pad = [
        members.first().map_or(0, |m| min_gap(&BARE, &m.item)),
        members.last().map_or(0, |m| min_gap(&m.item, &BARE)),
    ];
    (offsets, RailItem { s1: end, pad, ..BARE })
}

/// `item` with its footprint starting at `s0`, its width kept.
fn item_at(item: &RailItem, s0: i128) -> RailItem {
    RailItem {
        s0,
        s1: s0.saturating_add(item.width()),
        ..*item
    }
}

/// Applies a resolution's moved neighbours to an in-memory item list.
fn apply_moved(mem: &mut [RailItem], moved: &[(Id<Placement>, i128)]) {
    for (p, s0) in moved {
        if let Some(i) = mem.iter_mut().find(|i| i.placement == Some(*p)) {
            *i = item_at(i, *s0);
        }
    }
}

/// The new pose of each placement of `base` (the rail's items before the command) whose footprint the in-memory list
/// `mem` moved: its clip point shifted by the same distance (as [`resolve_on_rail`] re-poses a neighbour).
fn moved_poses(
    tx: &TxBuilder<'_>,
    axis: &RailAxis,
    base: &[RailItem],
    mem: &[RailItem],
) -> Result<Vec<(Id<Placement>, Pose3)>, CmdError> {
    let mut out = Vec::new();
    for old in base {
        let Some(p) = old.placement else {
            continue;
        };
        let Some(new) = mem.iter().find(|i| i.placement == Some(p)) else {
            continue;
        };
        if new.s0 == old.s0 {
            continue;
        }
        let Some(row) = tx.get::<Placement>(p) else {
            continue;
        };
        let s = i128::from(row.pose.t[0].0).saturating_add(new.s0.saturating_sub(old.s0));
        out.push((p, rail_pose(tx, axis, s, &row.pose)?));
    }
    Ok(out)
}

/// The rail by its label in a sentence ("rail 3"); "the rail" without one.
fn rail_in_sentence(q: &Q, carrier: Id<Carrier>) -> String {
    carrier_label(q, carrier).map_or_else(|| "the rail".to_owned(), |l| l.in_sentence())
}

/// The full designation of a device as the derived tag prints it (empty without one), for sorting.
fn full_tag(q: &Q, device: Id<Device>) -> String {
    q.try_get::<DeviceTag>(device)
        .ok()
        .and_then(|t| t.as_ref().as_ref().map(|t| t.full.clone()))
        .unwrap_or_default()
}

/// Whether a device has children other than itself.
fn has_children(q: &Q, device: Id<Device>) -> bool {
    q.idx().device_children(device).any(|c| c != device)
}

/// The root of the assembly a device belongs to (D6, ruling 5 (c)): the device when it has children, else its
/// parent; `None` for a device that is neither.
fn assembly_root(q: &Q, device: &Device) -> Option<Id<Device>> {
    if has_children(q, device.id) {
        Some(device.id)
    } else {
        device.parent
    }
}

/// [`assembly_root`] read through the transaction (cycle-42 review part 2, finding 1): the device's children through
/// the `device_children` index, so that a concurrent new member or a member re-parented away conflicts at merge
/// (ADR-0008). The device row itself is the caller's `tx.get`, its parent the row's.
fn assembly_root_tx(tx: &TxBuilder<'_>, device: &Device) -> Option<Id<Device>> {
    if tx.children_of(device.id).iter().any(|c| *c != device.id) {
        Some(device.id)
    } else {
        device.parent
    }
}

/// Records, through the transaction, the member set of the assembly `root` that [`member_order`] reads on the base
/// snapshot: the `device_children` index and each child's row (its position and its tag members).
fn note_members(tx: &TxBuilder<'_>, root: Id<Device>) {
    let _ = tx.get::<Device>(root);
    for c in tx.children_of(root) {
        let _ = tx.get::<Device>(c);
    }
}

/// Records, through the transaction, the inputs from which `layout.place_assembly` and `layout.auto_fill` choose and
/// order their devices but which they compute on the base snapshot (cycle-42 review part 2, finding 1; ADR-0008:
/// every lookup is recorded in the read set): `mountable(dev)`, `unplaced(space)` and its device lists, the location
/// (`StructureOf`) and the sort keys (`DeviceTag`, the drawn page order). These derived inputs cannot be read through
/// the transaction one by one, so the tables they are derived from are recorded whole (orchestrator ruling 1): a
/// spurious conflict only re-runs the command, a missed one would apply stale prepared ops. `placements` adds the
/// placement table, which `unplaced(space)` reads for every device of the space.
fn note_choice_reads(tx: &TxBuilder<'_>, placements: bool) {
    let _ = tx.project();
    let _ = tx.iter::<Device>();
    let _ = tx.iter::<StructureNode>();
    let _ = tx.iter::<Function>();
    let _ = tx.iter::<LibCopy<FnDef>>();
    let _ = tx.iter::<LibCopy<Part>>();
    let _ = tx.iter::<Representation>();
    let _ = tx.iter::<Page>();
    let _ = tx.iter::<Section>();
    let _ = tx.iter::<LayoutSpace>();
    let _ = tx.iter::<Enclosure>();
    let _ = tx.iter::<MountingSurface>();
    if placements {
        let _ = tx.iter::<Placement>();
    }
}

/// Whether the layout expects a device (`mountable(dev)`, D8); a structure cycle counts as not.
fn is_mountable(q: &Q, device: Id<Device>) -> bool {
    q.try_get::<Mountable>(device).is_ok_and(|m| m.is_mountable())
}

/// The members of `root`'s assembly that `layout.place_assembly` places, on a snapshot: mountable and without a
/// placement anywhere, in member order.
fn unplaced_members(q: &Q, root: Id<Device>) -> Vec<Id<Device>> {
    let devices = q.devices();
    member_order(q, root)
        .into_iter()
        .filter(|d| devices.get(*d).is_some())
        .filter(|d| is_mountable(q, *d) && q.idx().placements_of_device(*d).next().is_none())
        .collect()
}

/// `layout.place_assembly` (`PlaceAssembly`, WP-S4.02 D6, FR-LAY-061): places the unplaced members of an assembly on a
/// rail in their member order ([`member_order`], the order EW-LAY-016 checks), in one step.
///
/// The assembly of `device` is the device itself when it has children, else its parent's. The members placed are those
/// of [`member_order`] that are mountable (`mountable(dev)`, D8) and have no placement anywhere; each records its
/// primary part, turned 0°, and must fit the rail's profile (an unfit member refuses the whole command with
/// `layout.place`'s text). When no member is on the rail yet, the members go as one block at their minimum gaps
/// whose left edge is `s`, resolved in `mode` (`free` by default, or `push`) as one item. When some members are on the
/// rail, `s` and `mode` are not used: each new member is pushed in after its predecessor (the nearest earlier member on
/// the rail, a member placed by this command included), or before its first placed successor when it has no
/// predecessor; every pushed neighbour is written in the same step.
///
/// Rejected: a device or carrier that does not exist (`/device`, `/carrier`); a duct or a surface ("An assembly is
/// placed on a rail."); `mode: swap`; a device without an assembly; nothing to place; `s` missing when no member is on
/// the rail (`/s`); an unfit member; D5's refusals of the mode. Interactive; `Bulk` when it places more than 200.
/// Outcome: the first new placement.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaceAssembly {
    /// A device of the assembly: its root or a member.
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// The rail.
    pub carrier: PlaceOnParam,
    /// The block's left edge along the rail, when no member is on it yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub s: Option<Nm>,
    /// How the block is resolved: `free` (the default) or `push`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PlaceMode>,
}

impl Command for PlaceAssembly {
    const NAME: &'static str = "layout.place_assembly";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, model: &Model) -> Cost {
        let q = Q::new(model);
        let devices = q.devices();
        let count = devices
            .get(self.device)
            .and_then(|d| assembly_root(&q, d))
            .map_or(0, |root| unplaced_members(&q, root).len());
        bulk_above(count)
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
        let target = target(tx, name, "/carrier", self.carrier.to_ref())?;
        let (PoseOn::Rail(axis), PlaceOnParam::Carrier(rail)) = (target.on, self.carrier) else {
            return tx.reject("An assembly is placed on a rail.");
        };
        if self.mode == Some(PlaceMode::Swap) {
            return tx.reject("Swap works with one placement; place an assembly in free or push mode.");
        }
        let base_model = tx.base_model().clone();
        let q = Q::new(&base_model);
        let Some(root) = assembly_root_tx(tx, &device) else {
            return tx.reject(format!(
                "{} is not part of an assembly; place it on its own.",
                tag(tx, self.device)
            ));
        };
        // Finding 1 of cycle-42 review part 2: how the members are chosen and ordered is in the read set.
        note_members(tx, root);
        note_choice_reads(tx, false);
        let order = member_order(&q, root);
        let devices: Vec<std::sync::Arc<Device>> =
            order.iter().filter_map(|d| tx.get::<Device>(*d)).collect();
        // Override 2: one read set for every device the command reads, their placements included.
        note_rail_reads(tx, rail, &devices.iter().map(|d| d.as_ref()).collect::<Vec<_>>());
        let mut on_rail: BTreeMap<Id<Device>, Vec<Id<Placement>>> = BTreeMap::new();
        let mut unplaced = Vec::new();
        for d in &devices {
            let placements = tx.placements_of_device(d.id);
            if placements.is_empty() {
                if is_mountable(&q, d.id) {
                    unplaced.push(std::sync::Arc::clone(d));
                }
                continue;
            }
            let here: Vec<Id<Placement>> = placements
                .into_iter()
                .filter(|p| {
                    tx.get::<Placement>(*p)
                        .is_some_and(|p| p.carrier == CarrierRef::Carrier(rail))
                })
                .collect();
            if !here.is_empty() {
                on_rail.insert(d.id, here);
            }
        }
        if unplaced.is_empty() {
            return tx.reject(format!(
                "Every member of {} that mounts is already placed.",
                tag(tx, root)
            ));
        }
        let block_s = match (on_rail.is_empty(), self.s) {
            (true, None) => {
                return Err(CmdError::invalid_params(
                    name,
                    "/s",
                    format!(
                        "no member of {} is on {} yet; give the position s",
                        tag(tx, root),
                        rail_in_sentence(&q, rail)
                    ),
                ));
            }
            (true, Some(s)) => Some(i128::from(s.0)),
            (false, _) => None,
        };
        for d in &unplaced {
            let part = primary_part(tx, d);
            check_fit(tx, d.id, part.as_ref(), &target)?;
        }
        let members: Vec<Member> = unplaced.into_iter().map(|d| member(tx, &q, d)).collect();
        let names: BTreeMap<Id<Placement>, String> =
            members.iter().map(|m| (m.id, tag(tx, m.device.id))).collect();
        let base = rail_items(&q, rail);
        let mut mem = base.clone();
        match block_s {
            Some(s_left) => {
                let (offsets, block) = block_of(&members);
                let item = match members.as_slice() {
                    [one] => tag(tx, one.device.id),
                    _ => format!("the assembly {}", tag(tx, root)),
                };
                let wording = Wording {
                    carrier: rail,
                    mode: self.mode.unwrap_or(PlaceMode::Free),
                    item,
                    names: &names,
                };
                let r = resolve_items(tx, &q, &mem, axis.length, block, s_left, &wording)?;
                apply_moved(&mut mem, &r.moved);
                for (m, off) in members.iter().zip(offsets) {
                    mem.push(item_at(&m.item, r.s_left.saturating_add(off)));
                }
            }
            None => insert_members(tx, &q, rail, &axis, &order, on_rail, &members, &names, &mut mem)?,
        }
        let neighbours = moved_poses(tx, &axis, &base, &mem)?;
        let mut poses = Vec::with_capacity(members.len());
        for m in &members {
            let s0 = mem.iter().find(|i| i.placement == Some(m.id)).map_or(0, |i| i.s0);
            let pose = rail_pose(tx, &axis, s0.saturating_add(m.ext.clip), &Pose3::IDENTITY)?;
            check_pose_on(tx, &target, &pose)?;
            poses.push(pose);
        }
        move_neighbours(tx, neighbours)?;
        let primary = members[0].id;
        for (m, pose) in members.into_iter().zip(poses) {
            tx.insert(Placement {
                id: m.id,
                v: 1,
                device: m.device.id,
                function: None,
                part: m.part,
                carrier: CarrierRef::Carrier(rail),
                pose,
                locked: false,
                extra: Extra::new(),
            })?;
        }
        Ok(Outcome::primary((Kind::Placement, primary.cast())))
    }
}

/// `layout.place_assembly` when some members are on the rail (D6, ruling 5 (d), override 3): each new member, in member
/// order, is pushed in after its predecessor (its doubled centre the smallest value above the predecessor's, so that
/// [`resolve`] starts it at the predecessor's end plus the minimum gap), else before its first placed successor; the
/// in-memory list `mem` carries every member placed so far, so a member this command placed and a later insertion
/// pushes is re-posed in memory and inserted once, at its final place.
#[allow(clippy::too_many_arguments)]
fn insert_members(
    tx: &TxBuilder<'_>,
    q: &Q,
    rail: Id<Carrier>,
    axis: &RailAxis,
    order: &[Id<Device>],
    mut on: BTreeMap<Id<Device>, Vec<Id<Placement>>>,
    members: &[Member],
    names: &BTreeMap<Id<Placement>, String>,
    mem: &mut Vec<RailItem>,
) -> Result<(), CmdError> {
    for m in members {
        let at = order.iter().position(|d| *d == m.device.id).unwrap_or(0);
        let items_of = |mem: &[RailItem], d: &Id<Device>| -> Vec<RailItem> {
            on.get(d).map_or_else(Vec::new, |ps| {
                mem.iter()
                    .filter(|i| i.placement.is_some_and(|p| ps.contains(&p)))
                    .copied()
                    .collect()
            })
        };
        let pred = order[..at]
            .iter()
            .rev()
            .find_map(|d| items_of(mem, d).into_iter().max_by_key(|i| (i.s1, i.s0)));
        // Ruling 4 of WP-S4.02 part 3: the slot is `ew_derive::layout::member_slot`, which the layout tool's ghost reads.
        let succ = match pred {
            Some(_) => None,
            None => order
                .get(at + 1..)
                .unwrap_or_default()
                .iter()
                .find_map(|d| items_of(mem, d).into_iter().min_by_key(|i| (i.s0, i.s1))),
        };
        let s_left = member_slot(pred.as_ref(), succ.as_ref(), &m.item);
        let wording = Wording {
            carrier: rail,
            mode: PlaceMode::Push,
            item: tag(tx, m.device.id),
            names,
        };
        let r = resolve_items(tx, q, mem, axis.length, m.item, s_left, &wording)?;
        apply_moved(mem, &r.moved);
        mem.push(item_at(&m.item, r.s_left));
        on.entry(m.device.id).or_default().push(m.id);
    }
    Ok(())
}

/// The order `layout.auto_fill` places its devices in (WP-S4.02 D6).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FillOrder {
    /// The natural order of the full device tag.
    Dt,
    /// The page order of each device's main representation, then the row and the column of its anchor cell; devices
    /// without a main representation last, by tag.
    Schematic,
}

/// The layout space a carrier belongs to, through its surface and enclosure, on a snapshot.
fn space_of_carrier(q: &Q, carrier: Id<Carrier>) -> Option<Id<LayoutSpace>> {
    let surface = q.table::<Carrier>().get(carrier)?.surface;
    let enclosure = q.table::<MountingSurface>().get(surface)?.enclosure;
    Some(q.table::<Enclosure>().get(enclosure)?.space)
}

/// `layout.auto_fill`'s default list (D6): the unplaced devices of the rail's space whose primary part does not
/// refuse the rail's profile, in the order of `unplaced(space)`.
fn fill_list(q: &Q, carrier: Id<Carrier>) -> Vec<Id<Device>> {
    let Some(space) = space_of_carrier(q, carrier) else {
        return Vec::new();
    };
    let profile = q.table::<Carrier>().get(carrier).and_then(|c| match &c.params {
        CarrierParams::Rail(r) => Some(r.profile.clone()),
        _ => None,
    });
    let devices = q.devices();
    let copies = q.table::<LibCopy<Part>>();
    q.get::<Unplaced>(space)
        .iter()
        .copied()
        .filter(|d| {
            let Some(profile) = &profile else {
                return true;
            };
            let mount = devices
                .get(*d)
                .and_then(|dev| ew_derive::layout::primary_part(q, dev))
                .and_then(|p| copies.get(p.copy).and_then(|c| c.item.mech.clone()))
                .and_then(|m| m.mount_kind);
            fits_rail(mount.as_ref(), profile) != Fit::DoesNotFit
        })
        .collect()
}

/// `layout.auto_fill` (`AutoFill`, WP-S4.02 D6, FR-LAY-013): places a list of devices on a rail, next-fit from the
/// rail start, in one step; placed items never move.
///
/// `devices` defaults to the unplaced devices of the rail's layout space whose primary part fits the rail's profile.
/// `order` is `dt` (the natural order of the full tag) or `schematic` (the page order of the main representation,
/// then the row and the column of its anchor cell; devices without one last, by tag). The unplaced members of one
/// assembly travel as one block (at their minimum gaps, in [`member_order`]) at the place of the first of them in that
/// order. Each item goes to the first left edge at or after the previous item's end plus the minimum gap where it fits
/// with every minimum gap ([`next_fit`]); an unknown footprint is the 20 mm square. A device that is not placed is
/// named in a notice, one per device in list order: "-102F1: already placed", "-K9: not in this rail's layout
/// space", "-UC1: not mountable", "-X5: TS 32 does not fit a TS 35×7.5 rail", "E55: screw-mounted, not on a rail",
/// "-201U9: no room left on rail 3".
///
/// Rejected: a carrier that does not exist (`/carrier`); a duct ("Auto-fill works on a rail."); a listed device that
/// does not exist or is listed twice (`/devices/i`); nothing placed at all ("Nothing was placed on rail 1: " and the
/// notices). Interactive; `Bulk` above 200 devices in the list. Outcome: the carrier, with the notices.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutoFill {
    /// The rail.
    #[schemars(with = "UuidS")]
    pub carrier: Id<Carrier>,
    /// The devices to place; default the unplaced devices of the rail's space that fit it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<UuidS>>")]
    pub devices: Option<Vec<Id<Device>>>,
    /// `dt` or `schematic`.
    pub order: FillOrder,
}

/// The sort key of a device in `schematic` order: without a main representation last, then page position, row,
/// column, the tag's natural key and the ID.
type SchematicKey = (bool, usize, u32, u32, ew_mltext::NaturalKey, Id<Device>);

/// A device `layout.auto_fill` considers, with its index in the list (its notice's place).
type Candidate = (usize, std::sync::Arc<Device>);

impl Command for AutoFill {
    const NAME: &'static str = "layout.auto_fill";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn cost(&self, model: &Model) -> Cost {
        let count = match &self.devices {
            Some(list) => list.len(),
            None => fill_list(&Q::new(model), self.carrier).len(),
        };
        bulk_above(count)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        let carrier = self.carrier;
        let target = target(tx, name, "/carrier", CarrierRef::Carrier(carrier))?;
        let PoseOn::Rail(axis) = target.on else {
            return tx.reject("Auto-fill works on a rail.");
        };
        let base_model = tx.base_model().clone();
        let q = Q::new(&base_model);
        // Finding 1 of cycle-42 review part 2: the default list, the refusals' inputs and the sort keys are in the
        // read set.
        note_choice_reads(tx, true);
        // the space through the transaction, so that a concurrent move of the rail conflicts
        let space = tx
            .get::<Carrier>(carrier)
            .and_then(|c| tx.get::<MountingSurface>(c.surface))
            .and_then(|s| tx.get::<Enclosure>(s.enclosure))
            .and_then(|e| tx.get::<LayoutSpace>(e.space));
        let list = match &self.devices {
            Some(list) => {
                let mut seen = BTreeSet::new();
                for (i, d) in list.iter().enumerate() {
                    if !tx.exists::<Device>(*d) {
                        return Err(CmdError::invalid_params(
                            name,
                            format!("/devices/{i}"),
                            "the device is not in the project",
                        ));
                    }
                    if !seen.insert(*d) {
                        return Err(CmdError::invalid_params(
                            name,
                            format!("/devices/{i}"),
                            "the device is listed twice",
                        ));
                    }
                }
                list.clone()
            }
            None => fill_list(&q, carrier),
        };
        let devices: Vec<std::sync::Arc<Device>> = list.iter().filter_map(|d| tx.get::<Device>(*d)).collect();
        // Override 2: every listed device's placements are in the read set.
        note_rail_reads(
            tx,
            carrier,
            &devices.iter().map(|d| d.as_ref()).collect::<Vec<_>>(),
        );
        let rail = rail_in_sentence(&q, carrier);
        let mut notices: Vec<Option<String>> = vec![None; devices.len()];
        let mut candidates: Vec<Candidate> = Vec::new();
        for (i, d) in devices.iter().enumerate() {
            match fill_refusal(tx, &q, d, space.as_deref(), target.profile.as_ref()) {
                Some(why) => notices[i] = Some(format!("{}: {why}", tag(tx, d.id))),
                None => candidates.push((i, std::sync::Arc::clone(d))),
            }
        }
        sort_candidates(tx, &q, self.order, &mut candidates);
        let mut mem = rail_items(&q, carrier);
        let mut last: Option<RailItem> = None;
        let mut placed: Vec<(Member, Pose3)> = Vec::new();
        for unit in fill_units(tx, &q, candidates) {
            let members: Vec<Member> = unit
                .iter()
                .map(|(_, d)| member(tx, &q, std::sync::Arc::clone(d)))
                .collect();
            let (offsets, item) = block_of(&members);
            let floor = last.map_or(i128::MIN, |l| l.s1.saturating_add(min_gap(&l, &item)));
            let Some(s) = next_fit(&mem, axis.length, &item, floor) else {
                for (i, d) in &unit {
                    notices[*i] = Some(format!("{}: no room left on {rail}", tag(tx, d.id)));
                }
                continue;
            };
            for (m, off) in members.into_iter().zip(offsets) {
                let at = item_at(&m.item, s.saturating_add(off));
                mem.push(at);
                last = Some(at);
                let pose = rail_pose(tx, &axis, at.s0.saturating_add(m.ext.clip), &Pose3::IDENTITY)?;
                check_pose_on(tx, &target, &pose)?;
                placed.push((m, pose));
            }
        }
        let notices: Vec<String> = notices.into_iter().flatten().collect();
        if placed.is_empty() {
            let why = if notices.is_empty() {
                "no device to place.".to_owned()
            } else {
                notices.join("; ")
            };
            return tx.reject(format!("Nothing was placed on {rail}: {why}"));
        }
        for (m, pose) in placed {
            tx.insert(Placement {
                id: m.id,
                v: 1,
                device: m.device.id,
                function: None,
                part: m.part,
                carrier: CarrierRef::Carrier(carrier),
                pose,
                locked: false,
                extra: Extra::new(),
            })?;
        }
        Ok(Outcome {
            primary: Some((Kind::Carrier, carrier.cast())),
            note: None,
            notices,
        })
    }
}

/// Why `layout.auto_fill` does not place a listed device (ruling 8), after the tag and a colon: placed already, of
/// another space, not mountable, or a mount kind the rail's profile refuses; `None` when it is a candidate.
fn fill_refusal(
    tx: &TxBuilder<'_>,
    q: &Q,
    device: &Device,
    space: Option<&LayoutSpace>,
    profile: Option<&MountKind>,
) -> Option<String> {
    if !tx.placements_of_device(device.id).is_empty() {
        return Some("already placed".to_owned());
    }
    let location = q
        .try_get::<StructureOf>(device.id)
        .ok()
        .and_then(|s| s.node(Aspect::Location));
    if space.is_none_or(|s| Some(s.location) != location) {
        return Some("not in this rail's layout space".to_owned());
    }
    if !is_mountable(q, device.id) {
        return Some("not mountable".to_owned());
    }
    let part = primary_part(tx, device);
    let mount = part
        .and_then(|p| mech_of(tx, p.copy))
        .and_then(|m| m.mount_kind)?;
    let profile = profile?;
    if fits_rail(Some(&mount), profile) != Fit::DoesNotFit {
        return None;
    }
    Some(if Profile::of(&mount).is_some() {
        format!(
            "{} does not fit a {} rail",
            mount_label(&mount),
            mount_label(profile)
        )
    } else {
        format!("{}, not on a rail", mount_label(&mount))
    })
}

/// Sorts `layout.auto_fill`'s candidates in `order` (D6).
fn sort_candidates(tx: &TxBuilder<'_>, q: &Q, order: FillOrder, candidates: &mut [Candidate]) {
    match order {
        FillOrder::Dt => candidates.sort_by_cached_key(|(_, d)| (natural_key(&full_tag(q, d.id)), d.id)),
        FillOrder::Schematic => {
            let pages = q.get::<DrawnPageOrder>(());
            let key = |d: &Device| -> SchematicKey {
                let tag_key = natural_key(&full_tag(q, d.id));
                match crate::numbering::main_rep(tx, d.id) {
                    Some(rep) => {
                        let page = pages.position(PageRef::Drawn(rep.page)).unwrap_or(usize::MAX);
                        let (row, column) = crate::numbering::anchor_cell(tx, rep.id)
                            .map_or((u32::MAX, u32::MAX), |c| (c.row, c.column));
                        (false, page, row, column, tag_key, d.id)
                    }
                    None => (true, usize::MAX, u32::MAX, u32::MAX, tag_key, d.id),
                }
            };
            candidates.sort_by_cached_key(|(_, d)| key(d));
        }
    }
}

/// `layout.auto_fill`'s units in candidate order: a device alone, or the candidates of one assembly as one block at
/// the place of the first of them, in [`member_order`] (D6).
fn fill_units(tx: &TxBuilder<'_>, q: &Q, candidates: Vec<Candidate>) -> Vec<Vec<Candidate>> {
    let mut units: Vec<(Option<Id<Device>>, Vec<Candidate>)> = Vec::new();
    let mut unit_of_root: BTreeMap<Id<Device>, usize> = BTreeMap::new();
    for c in candidates {
        match assembly_root_tx(tx, &c.1) {
            Some(root) => match unit_of_root.get(&root) {
                Some(u) => units[*u].1.push(c),
                None => {
                    note_members(tx, root);
                    unit_of_root.insert(root, units.len());
                    units.push((Some(root), vec![c]));
                }
            },
            None => units.push((None, vec![c])),
        }
    }
    units
        .into_iter()
        .map(|(root, mut unit)| {
            if let Some(root) = root {
                let rank: BTreeMap<Id<Device>, usize> = member_order(q, root)
                    .into_iter()
                    .enumerate()
                    .map(|(i, d)| (d, i))
                    .collect();
                unit.sort_by_key(|(_, d)| rank.get(&d.id).copied().unwrap_or(usize::MAX));
            }
            unit
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Set-up and edits (WP-S4.02 D7, FT-840)

/// An identifier this module spells as a literal.
fn ident(name: &'static str, text: &str) -> Result<Ident, CmdError> {
    Ident::from_input(text).map_err(|e| CmdError::invalid_params(name, "", e.to_string()))
}

/// A device's part use of one piece of `part`.
fn one_of(part: Id<Part>) -> PartUse {
    PartUse {
        part: PartRef::new(part),
        qty: Dec3::ONE,
        extra: Extra::new(),
    }
}

/// `layout.enclosure.create_from_part` (`CreateEnclosureFromPart`, WP-S4.02 D7, FR-LAY-002): sets up a location's
/// enclosure from an enclosure part in one step.
///
/// `copies` (the `lib/` JSON form, as `device.place` takes them) are embedded first. Then, in order: the location's
/// layout space (created without a name when the location has none); the enclosure device, tag letter U with the
/// lowest counter `n ≥ 1` whose frozen tag `UC<n>` no device holds, frozen `UC<n>` (ruling 3: a page-less device is
/// never numbered), its location the given one and its function `function`, one piece of `part`; when the part's
/// enclosure section names a plate part, the plate device, a child of the enclosure device frozen `UC<n>-plate` with
/// one piece of the plate part and the same structure; then the enclosure and its plate surface exactly as
/// `layout.enclosure.create` makes them. One undo removes all of it, the embedded copies included.
///
/// Rejected: a location that does not exist or is not a location (`/location`, as `layout.space.create`); a part copy
/// the project does not hold after the embed (`/part`); a part without an enclosure section; a function node that
/// does not exist or is not a function (`/function`); a location that already has an enclosure; a plate part the
/// project does not hold. Interactive. Outcome: the new enclosure.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateEnclosureFromPart {
    /// The location structure node (+J-E-2416).
    #[schemars(with = "UuidS")]
    pub location: Id<StructureNode>,
    /// The enclosure part copy.
    #[schemars(with = "UuidS")]
    pub part: Id<Part>,
    /// The library documents to embed first (`lib/` JSON form): the enclosure part, its plate part and their closure;
    /// empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
    /// The function structure node of the enclosure device (the focused page's function).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub function: Option<Id<StructureNode>>,
    /// The enclosure's pose in the space; default the identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Pose3S>")]
    pub pose: Option<Pose3>,
    /// Whether the plate is bonded to protective earth; default false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pe_bond: Option<bool>,
}

impl Command for CreateEnclosureFromPart {
    const NAME: &'static str = "layout.enclosure.create_from_part";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        embed_closure(name, tx, &self.copies)?;
        let node = checked_location(tx, name, self.location)?;
        require_part(tx, name, "/part", self.part)?;
        let Some(copy) = part_copy(tx, self.part) else {
            return Err(CmdError::invalid_params(
                name,
                "/part",
                format!("part copy {} is not held by the project", self.part),
            ));
        };
        let Some(section) = copy.item.sections.enclosure.clone() else {
            return tx.reject(format!(
                "{} has no enclosure section; choose an enclosure part.",
                copy.item.part_number.as_str()
            ));
        };
        if let Some(f) = self.function {
            let Some(fnode) = tx.get::<StructureNode>(f) else {
                return Err(CmdError::invalid_params(
                    name,
                    "/function",
                    "the structure node is not in the project",
                ));
            };
            if fnode.aspect != Open::Known(Aspect::Function) {
                return Err(CmdError::invalid_params(
                    name,
                    "/function",
                    format!("node {} is not a function", fnode.code.as_str()),
                ));
            }
        }
        let existing = tx
            .iter::<LayoutSpace>()
            .into_iter()
            .find(|s| s.location == self.location)
            .map(|s| s.id);
        if let Some(space) = existing {
            if let Some(e) = tx.iter::<Enclosure>().into_iter().find(|e| e.space == space) {
                // The location's designation with its sign (`+J-E-2416`), as a tag prints it.
                let q = Q::new(tx.base_model());
                let codes = q
                    .try_get::<StructurePath>(self.location)
                    .ok()
                    .map(|p| p.codes.clone())
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| vec![node.code.as_str().to_owned()]);
                // ID-1106 (FT-1033): "Location +{location} already has enclosure {tag}; a location has one
                // enclosure. Choose another location, or edit {tag}."
                let tag = tag(tx, e.device);
                return tx.reject(format!(
                    "location +{} already has enclosure {tag}; a location has one enclosure. \
                     Choose another location, or edit {tag}",
                    codes.join("+"),
                ));
            }
        }
        let plate = plate_copy_id(&section);
        if plate.is_some_and(|p| !tx.exists::<LibCopy<Part>>(p)) {
            // The copy is not held, so its part number is not known here (lane E42: the brief's text named it).
            return tx.reject(
                "the enclosure part names a plate part that the project does not hold; add it to copies",
            );
        }
        let space = match existing {
            Some(s) => s,
            None => insert_space(tx, self.location, None)?,
        };
        // Ruling 3 (override 1): the lowest n ≥ 1 whose frozen tag UC<n> no device holds.
        let frozen: BTreeSet<String> = tx
            .iter::<Device>()
            .into_iter()
            .filter_map(|d| d.tag.frozen.as_ref().map(|f| f.text.as_str().to_owned()))
            .collect();
        let n = (1u32..)
            .find(|n| !frozen.contains(&format!("UC{n}")))
            .unwrap_or(1);
        let letter = ident(name, "U")?;
        let structure = StructureAssignment {
            location: Some(Slot::Node(self.location)),
            function: self.function.map(Slot::Node),
            ..StructureAssignment::default()
        };
        let device: Id<Device> = tx.mint();
        let enclosure_tag = TagSpec {
            letter: letter.clone(),
            counter: Some(n),
            scheme: TagScheme::Default,
            frozen: Some(frozen_tag(name, &format!("UC{n}"))?),
            extra: Extra::new(),
        };
        tx.insert(enclosure_device(
            device,
            enclosure_tag,
            None,
            &structure,
            self.part,
        ))?;
        if let Some(p) = plate {
            let child: Id<Device> = tx.mint();
            let plate_tag = TagSpec {
                letter,
                counter: None,
                scheme: TagScheme::Child,
                frozen: Some(frozen_tag(name, &format!("UC{n}-plate"))?),
                extra: Extra::new(),
            };
            tx.insert(enclosure_device(child, plate_tag, Some(device), &structure, p))?;
        }
        let enclosure = insert_enclosure(
            tx,
            space,
            device,
            &section,
            self.pose.unwrap_or(Pose3::IDENTITY),
            self.pe_bond.unwrap_or(false),
        )?;
        Ok(Outcome::primary((Kind::Enclosure, enclosure.cast())))
    }
}

/// A frozen tag with its sign shown.
fn frozen_tag(name: &'static str, text: &str) -> Result<FrozenTag, CmdError> {
    Ok(FrozenTag {
        text: ident(name, text)?,
        sign: TagSign::Shown,
        extra: Extra::new(),
    })
}

/// The enclosure device or its plate device as `layout.enclosure.create_from_part` makes it: one piece of `part`.
fn enclosure_device(
    id: Id<Device>,
    tag: TagSpec,
    parent: Option<Id<Device>>,
    structure: &StructureAssignment,
    part: Id<Part>,
) -> Device {
    Device {
        id,
        v: 1,
        tag,
        kind: None,
        parent,
        position: None,
        structure: structure.clone(),
        parts: vec![one_of(part)],
        function_text: Default::default(),
        process_tag: None,
        tech_data: Default::default(),
        state: Default::default(),
        source_ref: None,
        aspects: DeviceAspects::default(),
        extra: Extra::new(),
    }
}

/// `layout.space.edit` (`EditLayoutSpace`, FT-840): renames a layout space; `name: null` clears its name. Nothing else
/// changes. Rejected: a space that does not exist (`/space`); `name` absent (`/name`). Interactive. Outcome: the space.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditLayoutSpace {
    /// The layout space.
    #[schemars(with = "UuidS")]
    pub space: Id<LayoutSpace>,
    /// The new display name; `null` clears it.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<IdentS>")]
    pub name: Option<Option<Ident>>,
}

impl Command for EditLayoutSpace {
    const NAME: &'static str = "layout.space.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<LayoutSpace>(self.space) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/space",
                "the layout space is not in the project",
            ));
        }
        let Some(name) = self.name.clone() else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/name",
                "give the name, or null to clear it",
            ));
        };
        tx.update::<LayoutSpace>(self.space, |s| s.name = name)?;
        Ok(Outcome::primary((Kind::LayoutSpace, self.space.cast())))
    }
}

/// `layout.enclosure.edit` (`EditEnclosure`, FT-840): moves an enclosure in its layout space. The pose is a translation
/// with the identity rotation at this version (ruling 7: bayed suites are 1.x). Nothing else changes. Rejected: an
/// enclosure that does not exist (`/enclosure`); a turned pose. Interactive. Outcome: the enclosure.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditEnclosure {
    /// The enclosure.
    #[schemars(with = "UuidS")]
    pub enclosure: Id<Enclosure>,
    /// The new pose in the space: a translation.
    #[schemars(with = "Pose3S")]
    pub pose: Pose3,
}

impl Command for EditEnclosure {
    const NAME: &'static str = "layout.enclosure.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<Enclosure>(self.enclosure) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/enclosure",
                "the enclosure is not in the project",
            ));
        }
        if Quarter::of(self.pose.q) != Some(Quarter::NONE) {
            return tx.reject("an enclosure's pose is a translation at this version; give it without a turn");
        }
        let pose = self.pose;
        tx.update::<Enclosure>(self.enclosure, |e| e.pose = pose)?;
        Ok(Outcome::primary((Kind::Enclosure, self.enclosure.cast())))
    }
}

/// `layout.surface.edit` (`EditSurface`, FT-840): sets whether a mounting surface (the plate) is bonded to protective
/// earth. Nothing else changes. Rejected: a surface that does not exist (`/surface`). Interactive. Outcome: the surface.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditSurface {
    /// The mounting surface.
    #[schemars(with = "UuidS")]
    pub surface: Id<MountingSurface>,
    /// Whether it is bonded to protective earth.
    pub pe_bond: bool,
}

impl Command for EditSurface {
    const NAME: &'static str = "layout.surface.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<MountingSurface>(self.surface) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/surface",
                "the surface is not in the project",
            ));
        }
        let pe_bond = self.pe_bond;
        tx.update::<MountingSurface>(self.surface, |s| s.pe_bond = pe_bond)?;
        Ok(Outcome::primary((Kind::MountingSurface, self.surface.cast())))
    }
}
