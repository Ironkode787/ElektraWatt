// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `layout.*` commands (doc 12 §3): layout spaces, enclosures with their plate, carriers and
//! placements, each one undo step. A placement's pose is given explicitly and checked against its carrier kind's
//! degrees of freedom (`ew_derive::layout`); with `mode` `layout.place` and `layout.move`
//! resolve the position on a rail in free, push or swap mode and write every moved neighbour in the same step.
//!
//! The deletes are `Bulk` (previewed on a fork first) when they remove or rehome more than [`LAYOUT_BULK_ABOVE`]
//! placements. An occupied carrier goes only through `layout.carrier.delete` with `then` (doc 12 §3.6): every other
//! delete that would take a placement along is refused by the forbid policy of `placement.carrier`.

use std::collections::{BTreeMap, BTreeSet};

use ew_derive::Q;
use ew_derive::layout::{
    Fit, Mountable, PathError, PlacementGeometry, PoseError, PoseOn, Profile, Quarter, RailAxis, RailExtent,
    RailItem, Resolution, SnapError, SnapMode, Unplaced, carrier_label, cascade, cascade_mirrored,
    check_path, check_pose, clip_offset, fits_rail, member_order, member_slot, min_gap, next_fit,
    plate_frame, rail_extent, rail_items, rehome_pose, resolve,
};
use ew_derive::part_fields::{Feature as PartFeature, PartLack};
use ew_derive::{DeviceTag, DrawnPageOrder, PageRef, StructureOf, StructurePath};
use ew_ids::Id;
use ew_mltext::{Ident, natural_key};
use ew_model::{AnyKey, Model};
use ew_rds::Aspect;
use ew_schema::{
    Carrier, CarrierKind, CarrierParams, CarrierRef, Device, DeviceAspects, DuctParams, Enclosure,
    Corner, EnclosureSection, Extra, FnDef, Function, Kind, LayoutSpace, LibCopy, MechData, MountKind,
    MountingSurface, Open, Page, Part, PartRef, PartUse, Placement, RailParams, Representation, Section,
    Slot, StructureAssignment, StructureNode, SurfaceFeature, SurfaceKind, TagScheme, TagSpec,
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
use ew_l10n::{Text, TextArg};

mod assemblies;
// The holes of a surface: a hole by hand and a derived hole's choice
mod features;
// Route on a panel page (doc 12 §8.5)
mod route;

use assemblies::rail_in_sentence;
pub use assemblies::{AutoFill, FillOrder, PlaceAssembly};
pub(crate) use features::hole_member;
pub use features::{AddFeature, EditFeature, HoleChoiceEdit, PlaceGlands};
pub use route::LayoutRoute;

/// The layout deletes are `Bulk` when they remove or rehome more than this many placements.
pub const LAYOUT_BULK_ABOVE: usize = 200;

/// A present member (a value or `null`) as `Some`.
pub(crate) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}

// ---------------------------------------------------------------------------------------------------------
// Parameter types

/// What a placement sits on: `{"carrier": id}` (a rail) or `{"surface": id}` (the plate itself).
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

/// The kind of a new carrier.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CarrierKindParam {
    /// A mounting (DIN) rail.
    Rail,
    /// A wiring duct.
    Duct,
}

/// A carrier's parameters (doc 12 §2.4), tagged by its kind as stored: `{"rail": {"profile": …}}` or
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

/// How `layout.place` and `layout.move` resolve their position on a rail. Without a mode the pose
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
fn tag(tx: &TxBuilder<'_>, device: Id<Device>) -> TextArg {
    crate::words::device(tx, device)
}

/// A mount kind or a rail profile in words (`ew_derive::text::mount_kind`).
fn mount_words(kind: &MountKind) -> TextArg {
    ew_derive::text::mount_kind(kind).into()
}

/// The part copy of a part use, if the project holds it.
fn part_copy(tx: &TxBuilder<'_>, part: Id<Part>) -> Option<std::sync::Arc<LibCopy<Part>>> {
    tx.get::<LibCopy<Part>>(part)
}

/// The mechanical data of a part copy.
fn mech_of(tx: &TxBuilder<'_>, part: Id<Part>) -> Option<MechData> {
    part_copy(tx, part).and_then(|c| c.item.mech.clone())
}

/// Whether a device is a strip with terminals: it is placed as one placement without a part.
fn is_strip_with_terminals(tx: &TxBuilder<'_>, device: &Device) -> bool {
    device.aspects.strip.is_some() && !tx.terminals_of_strip(device.id).is_empty()
}

/// The part use a placement realises when it names none: the first part use whose part mounts
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
/// means the primary).
fn realised_part(tx: &TxBuilder<'_>, device: &Device, part: Option<&PartRef>) -> Option<PartRef> {
    match part {
        Some(p) => Some(p.clone()),
        None => primary_part(tx, device),
    }
}

/// The clip point of a placement in its part frame: the part's `rail_offset`, else its centre; a strip's left end
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
    /// The kind of the surface a placement sits on itself; `None` on a carrier.
    surface: Option<SurfaceKind>,
}

/// The target of a placement, or a refusal at `at` when the carrier or surface does not exist or cannot carry it.
fn target(tx: &TxBuilder<'_>, name: &'static str, at: &str, on: CarrierRef) -> Result<Target, CmdError> {
    match on {
        CarrierRef::Carrier(c) => {
            let Some(carrier) = tx.get::<Carrier>(c) else {
                return Err(CmdError::invalid_params(
                    name,
                    at,
                    text!("refusal-params-carrier-gone"),
                ));
            };
            match (&carrier.kind, &carrier.params) {
                (CarrierKind::Rail, params) => {
                    let Some(axis) = RailAxis::of(&carrier.path) else {
                        return tx.reject(text!("refusal-layout-rail-path"));
                    };
                    let profile = match params {
                        CarrierParams::Rail(r) => Some(r.profile.clone()),
                        _ => None,
                    };
                    Ok(Target {
                        on: PoseOn::Rail(axis),
                        profile,
                        surface: None,
                    })
                }
                (CarrierKind::Duct, _) => Ok(Target {
                    on: PoseOn::Duct,
                    profile: None,
                    surface: None,
                }),
                (CarrierKind::Unknown(_), _) => tx.reject(text!("refusal-layout-carrier-unknown")),
            }
        }
        CarrierRef::Surface(s) => {
            let Some(surface) = tx.get::<MountingSurface>(s) else {
                return Err(CmdError::invalid_params(
                    name,
                    at,
                    text!("refusal-params-surface-gone"),
                ));
            };
            Ok(Target {
                on: PoseOn::Surface,
                profile: None,
                surface: Some(surface.kind.clone()),
            })
        }
        CarrierRef::Unknown(_) => Err(CmdError::invalid_params(
            name,
            at,
            text!("refusal-layout-placement-on"),
        )),
    }
}

/// Refuses a part whose mount kind does not fit the target rail's profile (doc 12 §3.3: "-102F1 (TS 35) does not fit a
/// TS 32 rail"), on a surface a part that mounts on its parent or not at all, a part in a cut-out on the plate or a
/// rail ("-102S1 goes in a cut-out of a door or a wall: place it on the door's page") and a rail part on a door, a wall,
/// the top or the bottom itself ("-101F1 mounts on a TS 35×7.5 rail: drop it on a rail"); a rail part on the plate is
/// EW-LAY-010's, since `rehome_to_surface` makes it. A part without a mount kind, a strip and a partless device are not
/// judged here.
fn check_fit(
    tx: &TxBuilder<'_>,
    device: Id<Device>,
    part: Option<&PartRef>,
    target: &Target,
) -> Result<(), CmdError> {
    let kind = part.and_then(|p| mech_of(tx, p.copy)).and_then(|m| m.mount_kind);
    let on_plate_or_rail = target.profile.is_some() || target.surface == Some(SurfaceKind::Plate);
    if kind == Some(MountKind::DoorCutout) && on_plate_or_rail {
        return tx.reject(text!("refusal-layout-cutout", tag(tx, device)));
    }
    if let (Some(k), Some(surface)) = (&kind, &target.surface)
        && Profile::of(k).is_some()
        && !matches!(surface, SurfaceKind::Plate | SurfaceKind::Unknown(_))
    {
        return tx.reject(text!(
            "refusal-layout-rail-on-face",
            tag(tx, device),
            mount_words(k)
        ));
    }
    if matches!(target.on, PoseOn::Surface) {
        let copy = part.and_then(|p| part_copy(tx, p.copy));
        if let Some(copy) = copy.filter(|c| {
            c.item
                .mech
                .as_ref()
                .and_then(|m| m.mount_kind.as_ref())
                .is_some_and(|k| matches!(k, MountKind::None | MountKind::OnParent))
        }) {
            return tx.reject(text!(
                "refusal-layout-part-no-surface",
                copy.item.part_number.as_str()
            ));
        }
    }
    let Some(profile) = &target.profile else {
        return Ok(());
    };
    let mount = part.and_then(|p| mech_of(tx, p.copy)).and_then(|m| m.mount_kind);
    if fits_rail(mount.as_ref(), profile) == Fit::DoesNotFit {
        let mount = mount
            .as_ref()
            .map_or_else(|| TextArg::Plain(String::new()), mount_words);
        return tx.reject(text!(
            "refusal-layout-does-not-fit",
            tag(tx, device),
            mount,
            mount_words(profile)
        ));
    }
    Ok(())
}

/// Refuses a pose outside the target's degrees of freedom or off its rail.
fn check_pose_on(tx: &TxBuilder<'_>, target: &Target, pose: &Pose3) -> Result<(), CmdError> {
    check_pose(target.on, pose).or_else(|e: PoseError| tx.reject(pose_words(&e)))
}

/// A pose off its carrier in words ([`PoseError`]'s text, keyed).
fn pose_words(e: &PoseError) -> Text {
    match e {
        PoseError::OffRailAxis => text!("refusal-layout-pose-rail-axis"),
        PoseError::RailTurn => text!("refusal-layout-pose-rail-turn"),
        PoseError::OffRail(s, l) => text!(
            "refusal-layout-pose-off-rail",
            ew_derive::mm_exact(*s),
            ew_derive::mm_exact(*l)
        ),
        PoseError::OffSurface => text!("refusal-layout-pose-surface-plane"),
        PoseError::SurfaceTurn => text!("refusal-layout-pose-surface-turn"),
        PoseError::OnDuct => text!("refusal-layout-pose-duct"),
    }
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
                    text!("refusal-layout-rail-profile"),
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
                        text!("refusal-layout-rail-part-profile", mount_words(&section.profile)),
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
                        text!("refusal-layout-duct-length"),
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
            text!("refusal-layout-carrier-kind-unknown"),
        )),
        (CarrierKind::Rail, _) => Err(CmdError::invalid_params(
            name,
            "/params",
            text!("refusal-layout-rail-params"),
        )),
        (CarrierKind::Duct, _) => Err(CmdError::invalid_params(
            name,
            "/params",
            text!("refusal-layout-duct-params"),
        )),
    }
}

/// Checks a carrier path (`/path`).
fn checked_path(name: &'static str, kind: &CarrierKind, path: &[Pt2]) -> Result<(), CmdError> {
    check_path(kind, path).map_err(|e: PathError| {
        CmdError::invalid_params(
            name,
            "/path",
            text!("refusal-params-detail", crate::error::path_error_text(&e)),
        )
    })
}

/// Refuses a part copy the project does not hold (`at`).
fn require_part(tx: &TxBuilder<'_>, name: &'static str, at: &str, part: Id<Part>) -> Result<(), CmdError> {
    if tx.exists::<LibCopy<Part>>(part) {
        Ok(())
    } else {
        Err(CmdError::invalid_params(
            name,
            at,
            text!("refusal-layout-part-copy-missing", part.to_string()),
        ))
    }
}

// ---------------------------------------------------------------------------------------------------------
// Layout spaces

/// `layout.space.create` (`CreateLayoutSpace`): creates the layout space of a location (FR-LAY-001).
///
/// Rejected: a node that does not exist (`/location`) or is not a location; a location that already has a layout space
/// (one per location at the MVP, the refusal names the first). Interactive. Outcome: the new space.
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
            // The space in words, by its name, else as the layout space of its location; never by its ID.
            let code = node.code.as_str();
            return tx.reject(match first.name.as_ref() {
                Some(n) => text!("refusal-layout-space-named-exists", code, n.as_str()),
                None => text!("refusal-layout-space-exists", code),
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
            text!("refusal-params-node-gone"),
        ));
    };
    if node.aspect != Open::Known(ew_rds::Aspect::Location) {
        return tx.reject(text!("refusal-layout-space-not-location", node.code.as_str()));
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

/// `layout.space.delete` (`DeleteLayoutSpace`): deletes a layout space with its enclosure, surfaces and carriers
/// (the cascades). Refused while a placement sits in it (the forbid policy of `placement.carrier`: empty its
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
                text!("refusal-params-space-gone"),
            ));
        }
        tx.delete((Kind::LayoutSpace, self.space.cast()))?;
        Ok(Outcome::default())
    }
}

// ---------------------------------------------------------------------------------------------------------
// Enclosures

/// `layout.enclosure.create` (`CreateEnclosure`, FR-LAY-002): places an enclosure device in a layout space and
/// creates its mounting plate.
///
/// The device's first part use whose part carries an enclosure section gives the outer `W × H`; the section's `plate`
/// names the plate part. The `plate` surface's `device` is the first child of the enclosure device (in ID order) that
/// uses the plate part, if any; its frame centres the plate in the enclosure and its usable polygon is the plate
/// rectangle (`ew_derive::layout::plate_frame`; the plate size from its mounting-plate section, else its `mech`).
/// `pose` defaults to the identity, `pe_bond` (the plate's) to false.
///
/// Rejected: a space or device that does not exist (`/space`, `/device`); a space that already has an enclosure (one
/// per space at the MVP); a device whose parts carry no enclosure section ("the device has no enclosure part").
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
                text!("refusal-params-space-gone"),
            ));
        }
        let Some(device) = tx.get::<Device>(self.device) else {
            return Err(CmdError::invalid_params(
                name,
                "/device",
                text!("refusal-params-device-gone"),
            ));
        };
        if let Some(first) = tx.iter::<Enclosure>().into_iter().find(|e| e.space == self.space) {
            // The refusal names the next step, as the location's does.
            let tag = tag(tx, first.device);
            return tx.reject(text!("refusal-layout-space-has-enclosure", tag));
        }
        let Some(section) = device
            .parts
            .iter()
            .filter_map(|u| part_copy(tx, u.part.copy))
            .find_map(|c| c.item.sections.enclosure.clone())
        else {
            return tx.reject(text!("refusal-layout-no-enclosure-part", tag(tx, self.device)));
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
/// `layout.enclosure.create_from_part`): the plate part from the section, its size from its mounting-plate
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
        datum: None,
        edge_margin: None,
        hinge: None,
        extra: Extra::new(),
    })?;
    Ok(enclosure)
}

/// `layout.enclosure.delete` (`DeleteEnclosure`): deletes an enclosure with its surfaces and carriers (the cascades
/// of the layout); the enclosure device stays. Refused while a placement sits in it (the forbid policy of `placement.carrier`).
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
                text!("refusal-params-enclosure-gone"),
            ));
        }
        tx.delete((Kind::Enclosure, self.enclosure.cast()))?;
        Ok(Outcome::default())
    }
}

// ---------------------------------------------------------------------------------------------------------
// Carriers

/// `layout.carrier.add` (`AddCarrier`): adds a rail or a duct to a mounting surface.
///
/// The path is in the surface frame (`u` right, `v` up) and axis-parallel: a rail two points on one horizontal or
/// vertical line, a duct an orthogonal polyline. A rail's profile comes from its part's rail section when it has one
/// (a different `profile` is refused). Rejected: a surface or part copy that does not exist (`/surface`, `/part`);
/// parameters of the other kind (`/params`); a profile that is not a rail profile, a duct length ≤ 0 (`/params/…`); a
/// path of the wrong number of points, a segment without length or at another angle (`/path`). `copies`
/// embeds library documents first, in the same transaction, as `device.place` does (`/copies/i`), so that a
/// library rail or duct part can be used directly. Interactive. Outcome: the new carrier.
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
    /// The library documents to embed first (`lib/` JSON form): the part and its closure; empty when the
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
        // The copies first, so that the part they bring resolves below.
        embed_closure(name, tx, &self.copies)?;
        if !tx.exists::<MountingSurface>(self.surface) {
            return Err(CmdError::invalid_params(
                name,
                "/surface",
                text!("refusal-params-surface-gone"),
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
            hole_choices: Vec::new(),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Carrier, id.cast())))
    }
}

/// `layout.carrier.edit` (`EditCarrier`): changes a carrier's surface, part, path, parameters or PE bond.
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
                text!("refusal-params-carrier-gone"),
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
                text!("refusal-layout-carrier-edit-no-member"),
            ));
        }
        if let Some(surface) = self.surface {
            if !tx.exists::<MountingSurface>(surface) {
                return Err(CmdError::invalid_params(
                    name,
                    "/surface",
                    text!("refusal-params-surface-gone"),
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

/// `layout.carrier.delete` (`DeleteCarrier`, doc 12 §3.6): deletes a carrier; the only way to delete an occupied
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
                text!("refusal-params-carrier-gone"),
            ));
        };
        let placements = tx.placements_on_carrier(self.carrier);
        match (self.then, placements.is_empty()) {
            (_, true) => {}
            (None, false) => {
                return tx.reject(text!(
                    "refusal-layout-carrier-holds",
                    placements.len().to_string()
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
                    return tx.reject(text!("refusal-layout-rehome-rail-only"));
                };
                for p in placements {
                    let row = tx.require::<Placement>(p)?;
                    let device = tx.require::<Device>(row.device)?;
                    let clip = placement_clip(tx, &device, row.part.as_ref());
                    let Some(pose) = rehome_pose(&axis, &row.pose, clip) else {
                        return tx.reject(text!("refusal-layout-rehome-off-rail", tag(tx, row.device)));
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
// Modes on a rail

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

/// Records, through the transaction, the reads the rail's items come from: the
/// carrier, every placement on it, their devices with their part copies, and the terminals of the strips with theirs.
/// The items themselves are computed on the base snapshot ([`TxBuilder::base_model`], whose reads are not recorded),
/// so these reads make a concurrent edit of a neighbour on the rail conflict at merge (ADR-0008). Called once per
/// command with every device the command places or moves: each one's part copies and its
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
        return tx.reject(pose_words(&PoseError::OffRail(s, axis.length)));
    };
    Ok(Pose3 {
        t: [Nm(x), Nm::ZERO, Nm::ZERO],
        q: like.q,
    })
}

/// How a mode's refusal names what it resolves (the texts): the rail, whether it swaps, the item, and the devices of
/// placements this command is about to insert (they are not in the transaction yet).
struct Wording<'n> {
    carrier: Id<Carrier>,
    /// The mode resolved (swap words its refusals as swapping).
    mode: PlaceMode,
    /// The dropped item: a device's tag, or "the assembly -201U1" for an assembly's block.
    item: TextArg,
    /// The tag of each placement minted by the command and not inserted yet.
    names: &'n BTreeMap<Id<Placement>, String>,
}

/// The refusal of a mode, in words: the rail by its label in a sentence ("rail 1"), devices by their tags.
fn snap_text(tx: &TxBuilder<'_>, q: &Q, w: &Wording<'_>, e: SnapError) -> Text {
    let item = w.item.clone();
    let swap = w.mode == PlaceMode::Swap;
    let placed = |p: Option<Id<Placement>>| match p.and_then(|p| w.names.get(&p)) {
        Some(name) => TextArg::Plain(name.clone()),
        None => p.and_then(|p| tx.get::<Placement>(p)).map_or_else(
            || text!("refusal-layout-a-placement").into(),
            |pl| tag(tx, pl.device),
        ),
    };
    match e {
        SnapError::NoGap { width } => {
            let rail = rail_in_sentence(q, w.carrier);
            text!(
                "refusal-layout-snap-no-gap",
                rail,
                item,
                ew_derive::mm_exact(width)
            )
        }
        SnapError::MovedLocked => text!("refusal-layout-snap-unlock", item),
        SnapError::Locked(p) if swap => text!("refusal-layout-snap-swap-locked", placed(p)),
        SnapError::Locked(p) => text!("refusal-layout-snap-push-locked", placed(p)),
        SnapError::PastEnd {
            neighbour: Some(p),
            by,
        } if swap => text!(
            "refusal-layout-snap-swap-past-end",
            placed(Some(p)),
            ew_derive::mm_exact(by)
        ),
        SnapError::PastEnd {
            neighbour: Some(p),
            by,
        } => text!(
            "refusal-layout-snap-push-past-end",
            placed(Some(p)),
            ew_derive::mm_exact(by)
        ),
        SnapError::PastEnd { neighbour: None, by } => {
            text!(
                "refusal-layout-snap-item-past-end",
                sentence_start(&item),
                ew_derive::mm_exact(by)
            )
        }
        SnapError::SwapAcrossRails => text!("refusal-layout-snap-swap-across"),
        SnapError::SwapNoTarget => text!("refusal-layout-snap-swap-no-target"),
    }
}

/// An item at the start of a sentence ("the assembly …" as "The assembly …"): a plain text with its first letter in
/// upper case, a keyed one by its sentence-start key.
fn sentence_start(item: &TextArg) -> TextArg {
    match item {
        TextArg::Plain(text) => {
            let mut chars = text.chars();
            TextArg::Plain(match chars.next() {
                Some(c) => c.to_uppercase().chain(chars).collect(),
                None => String::new(),
            })
        }
        TextArg::Text(t) => match (t.key(), t.args()) {
            ("refusal-layout-the-assembly", [root]) => {
                text!("refusal-layout-the-assembly-start", root.clone()).into()
            }
            ("refusal-words-device-gone", []) => text!("refusal-words-device-gone-start").into(),
            _ => item.clone(),
        },
        // A number or a name as it is.
        other => other.clone(),
    }
}

/// The rail item of a device about to be placed or moved: its extent from [`rail_extent`] (the shape
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
            ew_derive::strip_bracket(q, device).map(|b| b.width)
        } else {
            None
        },
        pad: ext.pad,
        locked,
        placement,
    };
    (item, ext)
}

/// The one resolver over a given item list (split out of [`resolve_on_rail`]): [`resolve`] with its refusal
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

/// Resolves a placement dropped on a rail with a mode: the rail's items from `rail_occupancy` on the base
/// snapshot (the same [`rail_items`] the tool's ghost reads), the placement's own extent from [`rail_extent`], then
/// [`resolve_items`]; the pose's `t.x` is the intended clip point, turned into the footprint's left edge and back.
///
/// **One placement per transaction**: the items come from
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

/// Where a mode applies: `Some((rail, axis, mode))` for a mode on a rail; `None` without a mode, and for free on a
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
        (PoseOn::Surface, _) if mode != PlaceMode::Free => tx.reject(text!("refusal-layout-modes-on-rail")),
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

/// `layout.place` (`PlaceDevice`, doc 12 §3.3): places a device on a rail or on a mounting surface at an explicit
/// pose, or, with `mode` on a rail, where the mode resolves it.
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
/// use whose part mounts, else the first without a mount kind); a strip with terminals and a device without a part
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
    /// How the position is resolved on a rail: `free`, `push` or `swap`; absent: the pose as given.
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
                text!("refusal-params-device-gone"),
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
                    text!("refusal-layout-function-not-device"),
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
                        text!("refusal-layout-part-unused", tag(tx, self.device)),
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
            return tx.reject(text!("refusal-layout-already-placed", tag(tx, self.device)));
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
            label_at: None,
            hole_choices: Vec::new(),
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::Placement, id.cast())))
    }
}

/// `layout.move` (`MovePlacement`): gives a placement another pose, optionally on another rail or surface.
///
/// The pose is checked as `layout.place` checks it; the fit of the part to a rail's profile only when the carrier
/// changes. Without `mode`, or with `mode: free`, a locked placement moves too (the lock excludes it from being moved
/// by another placement's push, swap and auto-fill, doc 12 §2.3). With `mode` on a rail the placement is first taken
/// off the rail and resolved as `layout.place` resolves a device; `swap` exchanges it with the placement
/// under its centre on the same rail, the segment between them re-laid with every gap kept where it still meets the
/// new minimum gap. Push and swap of a locked placement are refused ("Unlock F55 to push or swap it."), and never move
/// another locked placement. With `part` the placement realises that part use of its device instead
/// (EW-LAY-003's quick fix "Realise the primary part"): checked as `layout.place` checks a part, its fit to a rail's
/// profile checked when the part changes, and with `mode` the resolver reads the new part's extent; without `part`
/// the realised part is unchanged. Rejected: a placement, carrier or surface that does not exist (`/placement`,
/// `/carrier`); a part the device does not use (`/part`); a part that would give the device a second placement of one
/// (device, part, function); an unfit part; a pose outside the carrier kind's degrees of freedom or off the rail; the
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
    /// How the position is resolved on a rail: `free`, `push` or `swap`; absent: the pose as given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PlaceMode>,
    /// The part copy of another part use of the device to realise; absent: the realised part stays.
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
                text!("refusal-params-placement-gone"),
            ));
        };
        let carrier = self
            .carrier
            .map_or_else(|| current.carrier.clone(), PlaceOnParam::to_ref);
        let target = target(tx, name, "/carrier", carrier.clone())?;
        // Another part use of the device, checked as `layout.place` checks it.
        let new_part = match self.part {
            None => None,
            Some(copy) => {
                let device = tx.require::<Device>(current.device)?;
                let Some(u) = device.parts.iter().find(|u| u.part.copy == copy) else {
                    return Err(CmdError::invalid_params(
                        name,
                        "/part",
                        text!("refusal-layout-part-unused", tag(tx, current.device)),
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
                    return tx.reject(text!("refusal-layout-already-placed", tag(tx, current.device)));
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

/// `layout.remove` (`RemovePlacement`): removes a placement; the device becomes unplaced. Rejected: a placement
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
                text!("refusal-params-placement-gone"),
            ));
        }
        tx.delete((Kind::Placement, self.placement.cast()))?;
        Ok(Outcome::default())
    }
}

/// `layout.lock` (`LockPlacement`): locks or unlocks a placement (a locked one is excluded from push, swap and
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
                text!("refusal-params-placement-gone"),
            ));
        }
        let locked = self.locked;
        tx.update::<Placement>(self.placement, |p| p.locked = locked)?;
        Ok(Outcome::primary((Kind::Placement, self.placement.cast())))
    }
}

/// `layout.label_move` (`MoveLayoutLabel`, doc 12 §5.3): moves a placement's label (or its item-number balloon) on its
/// layout pages alone, or puts it back. `at` is the label's centre as an offset from the centre of the placement's
/// footprint, in the surface's frame (`[u, v]` nm, `v` up), so the label moves with the placement; absent puts the
/// label back where the label rules place it. Rejected: a placement that does not exist (`/placement`). Interactive.
/// Outcome: the placement.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MoveLayoutLabel {
    /// The placement.
    #[schemars(with = "UuidS")]
    pub placement: Id<Placement>,
    /// The label's centre from the footprint's centre; absent: back to the rules' place.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<SurfacePt2S>")]
    pub at: Option<Pt2>,
}

impl Command for MoveLayoutLabel {
    const NAME: &'static str = "layout.label_move";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if !tx.exists::<Placement>(self.placement) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/placement",
                text!("refusal-params-placement-gone"),
            ));
        }
        let at = self.at;
        tx.update::<Placement>(self.placement, |p| p.label_at = at)?;
        Ok(Outcome::primary((Kind::Placement, self.placement.cast())))
    }
}

/// `layout.labels_reset` (`ResetLayoutLabels`, doc 12 §5.3): puts the labels of the placements back where the label
/// rules place them, in one step (a layout page's Reset label positions); a placement whose label was not moved is left
/// untouched. Rejected: an empty `placements` list (`/placements`); a missing placement is not found. Interactive.
/// Outcome: none.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetLayoutLabels {
    /// The placements, at least one.
    #[schemars(with = "Vec<UuidS>")]
    #[schemars(length(min = 1))]
    pub placements: Vec<Id<Placement>>,
}

impl Command for ResetLayoutLabels {
    const NAME: &'static str = "layout.labels_reset";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        if self.placements.is_empty() {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/placements",
                text!("refusal-params-no-placement"),
            ));
        }
        for id in &self.placements {
            if tx.require::<Placement>(*id)?.label_at.is_some() {
                tx.update::<Placement>(*id, |p| p.label_at = None)?;
            }
        }
        Ok(Outcome::default())
    }
}

/// When `part.assign` adds the one copy `new` to `device` (modes `replace` and `replace_all`, the part not used
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
    // Only the first replaced part use's placements follow the new copy.
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

/// Doc 12 §3.3's push: after terminals were inserted into `strip`, each placement of the
/// strip as a whole on a rail (turned 0°) that grew shifts the items after it on the rail by the overlap, cascading
/// ([`cascade`] with the grown strip first), in the same transaction. The grown width is the strip's terminals as the
/// transaction holds them, side by side; the positions and the other strips' widths are the transaction's too
/// ([`in_transaction`]), so one command's pushes cascade from one another. A strip turned 180° grows left and
/// pushes the items before it, leftwards: the same cascade runs on the rail's coordinates mirrored
/// ([`cascade_mirrored`]), the gap between two strips being the bracket of the strip on the left in the real frame, as
/// EW-LAY-010 judges it; past the rail's start the positions are written all the same.
///
/// - Free space after the strip is used first; an item moves only as far as its minimum gap to its moved neighbour
///   requires (between two strips, the end bracket).
/// - A locked item stops the cascade: the items before it move, it and those after it stay, and EW-LAY-010 reports the
///   overlap. Past the rail's end the positions are written all the same, and EW-LAY-010 reports `bracket_outside`. The
///   command is never refused for the layout's sake.
/// - A strip whose footprint is unknown before or after (a terminal without a part or without a width and height)
///   pushes nothing, so `terminal.add`, which inserts terminals without a part, never pushes.
pub(crate) fn push_after_growth(tx: &mut TxBuilder<'_>, strip: Id<Device>) -> Result<(), CmdError> {
    let Some(width) = strip_width(tx, strip) else {
        return Ok(());
    };
    let base_model = tx.base_model().clone();
    let q = Q::new(&base_model);
    let mut writes = Vec::new();
    for p in tx.placements_of_device(strip) {
        let Some(row) = tx.get::<Placement>(p) else {
            continue;
        };
        let CarrierRef::Carrier(c) = row.carrier else {
            continue;
        };
        let known = q
            .get::<PlacementGeometry>(p)
            .as_ref()
            .as_ref()
            .is_some_and(|g| !g.unknown);
        let turned = match Quarter::of(row.pose.q) {
            Some(Quarter::NONE) => false,
            Some(Quarter::HALF) => true,
            _ => continue,
        };
        if row.part.is_some() || !known {
            continue;
        }
        let Some(axis) = tx.get::<Carrier>(c).and_then(|r| RailAxis::of(&r.path)) else {
            continue;
        };
        if let Some(d) = tx.get::<Device>(strip) {
            note_rail_reads(tx, c, &[d.as_ref()]);
        }
        let mut items = rail_items(&q, c);
        in_transaction(tx, &q, p, &mut items);
        if turned {
            // The mirrored frame: `s` becomes `−s`, the pads swap sides, and the items before the strip come after it.
            for i in &mut items {
                *i = RailItem {
                    s0: -i.s1,
                    s1: -i.s0,
                    pad: [i.pad[1], i.pad[0]],
                    ..*i
                };
            }
        }
        items.sort_by_key(|i| (i.s0, i.s1, i.placement));
        let Some(k) = items.iter().position(|i| i.placement == Some(p)) else {
            continue;
        };
        let grown = RailItem {
            s1: items[k].s0 + width,
            ..items[k]
        };
        if grown.s1 <= items[k].s1 {
            continue;
        }
        // The rail's end (its start, mirrored) refuses nothing here: EW-LAY-010 reports it.
        let rest = &items[k + 1..];
        // Mirrored, the gaps stay the real frame's.
        let push = if turned { cascade_mirrored } else { cascade };
        let moves = match push(grown, rest, i128::MAX, true) {
            Ok(moves) => moves,
            Err(SnapError::Locked(Some(locked))) => {
                let stop = rest.iter().position(|i| i.placement == Some(locked)).unwrap_or(0);
                push(grown, &rest[..stop], i128::MAX, true).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        };
        for (moved, s0) in moves {
            let (Some(pl), Some(old)) = (
                tx.get::<Placement>(moved),
                rest.iter().find(|i| i.placement == Some(moved)),
            ) else {
                continue;
            };
            // Mirrored, the item's real start moves by the opposite of its mirrored start's move.
            let by = if turned { old.s0 - s0 } else { s0 - old.s0 };
            let s = i128::from(pl.pose.t[0].0) + by;
            writes.push((moved, rail_pose(tx, &axis, s, &pl.pose)?));
        }
    }
    move_neighbours(tx, writes)
}

/// The width of `strip`'s terminals side by side as the transaction holds them; `None` when one has no part or no
/// width and height.
fn strip_width(tx: &TxBuilder<'_>, strip: Id<Device>) -> Option<i128> {
    let mut width = 0i128;
    for t in tx.terminals_of_strip(strip) {
        let (w, _) = tx
            .get::<Device>(t)
            .and_then(|d| primary_part(tx, &d))
            .and_then(|p| mech_of(tx, p.copy))
            .and_then(|m| Some((m.width?, m.height?)))?;
        width += i128::from(w.0);
    }
    Some(width)
}

/// The rail's items (computed on the base snapshot `q`) where the transaction holds them, so that
/// a command that grows several strips cascades each push from the earlier ones: an item whose placement moved along
/// the rail (its turn unchanged) moves by as much, and another strip placed as a whole takes the width of its terminals
/// as the transaction holds them, at its clip point (its left end, turned 180° its right end). The placement `grown`
/// keeps its width before the change, which its growth is measured against.
fn in_transaction(tx: &TxBuilder<'_>, q: &Q, grown: Id<Placement>, items: &mut [RailItem]) {
    let base = q.table::<Placement>();
    for i in items.iter_mut() {
        let Some(ip) = i.placement else {
            continue;
        };
        let (Some(now), Some(was)) = (tx.get::<Placement>(ip), base.get(ip)) else {
            continue;
        };
        if now.pose.q != was.pose.q {
            continue;
        }
        let by = i128::from(now.pose.t[0].0) - i128::from(was.pose.t[0].0);
        i.s0 = i.s0.saturating_add(by);
        i.s1 = i.s1.saturating_add(by);
        if ip == grown || !i.strip {
            continue;
        }
        let Some(w) = strip_width(tx, now.device) else {
            continue;
        };
        match Quarter::of(now.pose.q) {
            Some(Quarter::NONE) => i.s1 = i.s0.saturating_add(w),
            Some(Quarter::HALF) => i.s0 = i.s1.saturating_sub(w),
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// Set-up and edits

/// An identifier this module spells as a literal.
fn ident(name: &'static str, text: &str) -> Result<Ident, CmdError> {
    Ident::from_input(text).map_err(|e| {
        CmdError::invalid_params(
            name,
            "",
            text!("refusal-params-detail", crate::error::ident_error_text(&e)),
        )
    })
}

/// A device's part use of one piece of `part`.
fn one_of(part: Id<Part>) -> PartUse {
    PartUse {
        part: PartRef::new(part),
        qty: Dec3::ONE,
        extra: Extra::new(),
    }
}

/// `layout.enclosure.create_from_part` (`CreateEnclosureFromPart`, FR-LAY-002): sets up a location's
/// enclosure from an enclosure part in one step.
///
/// `copies` (the `lib/` JSON form, as `device.place` takes them) are embedded first. Then, in order: the location's
/// layout space (created without a name when the location has none); the enclosure device, tag letter UC with the
/// lowest counter `n ≥ 1` whose tag `UC<n>` no device holds (a counter of the letter, or a frozen tag), not frozen: the
/// scheme's tag, which prints no page for an enclosure's device (`-UC1`; online numbering never renumbers a page-less
/// device), its location the given one and its function `function`, one piece of `part`; when the part's enclosure
/// section names a plate part, the plate device, the child U1 of the enclosure device (`-UC1-U1`) with
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
                text!("refusal-layout-part-copy-missing", self.part.to_string()),
            ));
        };
        let Some(section) = copy.item.sections.enclosure.clone() else {
            let lack = PartLack::of(self.part, &copy.item, PartFeature::Cabinet);
            return tx.reject_lacking(
                text!(
                    "refusal-layout-no-enclosure-section",
                    copy.item.part_number.as_str()
                ),
                lack,
            );
        };
        if let Some(f) = self.function {
            let Some(fnode) = tx.get::<StructureNode>(f) else {
                return Err(CmdError::invalid_params(
                    name,
                    "/function",
                    text!("refusal-params-node-gone"),
                ));
            };
            if fnode.aspect != Open::Known(Aspect::Function) {
                return Err(CmdError::invalid_params(
                    name,
                    "/function",
                    text!("refusal-layout-node-not-function", fnode.code.as_str()),
                ));
            }
        }
        let existing = self.space_without_enclosure(tx, &node)?;
        let plate = plate_copy_id(&section);
        if plate.is_some_and(|p| !tx.exists::<LibCopy<Part>>(p)) {
            // The copy is not held, so its part number is not known here.
            return tx.reject(text!("refusal-layout-plate-part-missing"));
        }
        let space = match existing {
            Some(s) => s,
            None => insert_space(tx, self.location, None)?,
        };
        // The scheme's tags, not frozen ones: the enclosure UC<n>, whose page-less tag prints no page, with
        // the lowest n ≥ 1 that no device's tag UC<n> holds (another enclosure's counter, or a frozen UC<n> of an
        // older project); its plate the child U1.
        let letter = ident(name, "UC")?;
        let n = free_enclosure_counter(tx, &letter);
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
            frozen: None,
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
                letter: ident(name, "U")?,
                counter: Some(1),
                scheme: TagScheme::Child,
                frozen: None,
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

impl CreateEnclosureFromPart {
    /// The layout space of the location, when it has one: refused when it holds an enclosure already (a location has
    /// one enclosure).
    fn space_without_enclosure(
        &self,
        tx: &TxBuilder<'_>,
        node: &StructureNode,
    ) -> Result<Option<Id<LayoutSpace>>, CmdError> {
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
                // "Location +{location} already has enclosure {tag}; a location has one
                // enclosure. Choose another location, or edit {tag}."
                let tag = tag(tx, e.device);
                return tx.reject(text!(
                    "refusal-layout-location-has-enclosure",
                    codes.join("+"),
                    tag
                ));
            }
        }
        Ok(existing)
    }
}

/// The lowest n ≥ 1 that no device's tag UC<n> holds (another enclosure's counter, or a frozen UC<n> of an older
/// project), `letter` being UC.
fn free_enclosure_counter(tx: &TxBuilder<'_>, letter: &Ident) -> u32 {
    let held: BTreeSet<String> = tx
        .iter::<Device>()
        .into_iter()
        .filter_map(|d| match &d.tag.frozen {
            Some(f) => Some(f.text.as_str().to_owned()),
            None if d.tag.letter == *letter && d.tag.scheme == TagScheme::Default => {
                d.tag.counter.map(|c| format!("UC{c}"))
            }
            None => None,
        })
        .collect();
    (1u32..).find(|n| !held.contains(&format!("UC{n}"))).unwrap_or(1)
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
        accessory_choices: Vec::new(),
        function_text: Default::default(),
        process_tag: None,
        tech_data: Default::default(),
        state: Default::default(),
        source_ref: None,
        aspects: DeviceAspects::default(),
        gland: None,
        extra: Extra::new(),
    }
}

/// `layout.enclosure.set_part` (`SetEnclosurePart`, doc 12 §3.1): gives an enclosure another cabinet part in one step.
///
/// `copies` (the `lib/` JSON form, as `device.place` takes them: the cabinet part with its closure, its plate among
/// it) are embedded first. Then the enclosure's device takes the part (its part uses become one piece of it); where the
/// new part names a plate, the plate device (the child of the enclosure's device that the plate surface names, else a
/// new child `U1` as `layout.enclosure.create_from_part` makes it) takes that plate; a part that names no plate leaves
/// the plate there is. Every surface the layout holds of the enclosure takes the new part's size: the plate surface its
/// frame and usable area as the plate centred in the cabinet ([`plate_frame`]), a door, a side, the top and the bottom
/// their frames as `layout.surface.add` makes them, a door matched by its place among the doors (one the new part
/// does not list keeps its frame). What stands on a surface keeps its place measured from the surface's datum
/// corner: from a corner at the top or the right its poses are moved by the change of the sheet's size.
///
/// Rejected: an enclosure that does not exist (`/enclosure`); a part copy the project does not hold after the embed
/// (`/part`); a part without an enclosure section (with what it lacks); a plate part the project does not hold.
/// **Bulk**: previewed, the card names the sizes that change and what no longer lies inside its surface. Outcome: the
/// enclosure.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetEnclosurePart {
    /// The enclosure.
    #[schemars(with = "UuidS")]
    pub enclosure: Id<Enclosure>,
    /// The cabinet part copy.
    #[schemars(with = "UuidS")]
    pub part: Id<Part>,
    /// The library documents to embed first (`lib/` JSON form); empty when the project holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copies: Vec<Value>,
}

impl Command for SetEnclosurePart {
    const NAME: &'static str = "layout.enclosure.set_part";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME).bulk()
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let name = Self::NAME;
        embed_closure(name, tx, &self.copies)?;
        let Some(enclosure) = tx.get::<Enclosure>(self.enclosure) else {
            return Err(CmdError::invalid_params(
                name,
                "/enclosure",
                text!("refusal-params-enclosure-gone"),
            ));
        };
        require_part(tx, name, "/part", self.part)?;
        let Some(copy) = part_copy(tx, self.part) else {
            return Err(CmdError::invalid_params(
                name,
                "/part",
                text!("refusal-layout-part-copy-missing", self.part.to_string()),
            ));
        };
        let Some(section) = copy.item.sections.enclosure.clone() else {
            let lack = PartLack::of(self.part, &copy.item, PartFeature::Cabinet);
            return tx.reject_lacking(
                text!(
                    "refusal-layout-no-enclosure-section",
                    copy.item.part_number.as_str()
                ),
                lack,
            );
        };
        let plate = plate_copy_id(&section);
        if plate.is_some_and(|p| !tx.exists::<LibCopy<Part>>(p)) {
            return tx.reject(text!("refusal-layout-plate-part-missing"));
        }
        let before = enclosure_section_of(tx, &enclosure).unwrap_or_default();
        let device = enclosure.device;
        let part = self.part;
        tx.update::<Device>(device, |d| d.parts = vec![one_of(part)])?;
        let surfaces: Vec<MountingSurface> = (tx.iter::<MountingSurface>().into_iter())
            .filter(|s| s.enclosure == self.enclosure)
            .map(|s| (*s).clone())
            .collect();
        // The plate's device: the one its surface names, else a new child, as the enclosure made from a part has.
        let plate_device = match plate {
            None => None,
            Some(p) => {
                let named = surfaces
                    .iter()
                    .find(|s| s.kind == SurfaceKind::Plate)
                    .and_then(|s| s.device);
                Some(match named {
                    Some(d) => {
                        tx.update::<Device>(d, |x| x.parts = vec![one_of(p)])?;
                        d
                    }
                    None => {
                        let structure = tx
                            .get::<Device>(device)
                            .map(|d| d.structure.clone())
                            .unwrap_or_default();
                        let child: Id<Device> = tx.mint();
                        let plate_tag = TagSpec {
                            letter: ident(name, "U")?,
                            counter: Some(1),
                            scheme: TagScheme::Child,
                            frozen: None,
                            extra: Extra::new(),
                        };
                        tx.insert(enclosure_device(child, plate_tag, Some(device), &structure, p))?;
                        child
                    }
                })
            }
        };
        let plate_size =
            plate
                .and_then(|p| part_copy(tx, p))
                .map(|c| match &c.item.sections.mounting_plate {
                    Some(s) if s.width.is_some() && s.height.is_some() => (s.width, s.height),
                    _ => c.item.mech.as_ref().map_or((None, None), |m| (m.width, m.height)),
                });
        let doors_before = ew_derive::layout::door_count(&before);
        let doors_after = ew_derive::layout::door_count(&section);
        for s in surfaces {
            let mut next = s.clone();
            match s.kind {
                SurfaceKind::Plate => {
                    // A part that names no plate keeps the plate there is, at its size.
                    if let Some(size) = plate_size {
                        let (frame, usable) = plate_frame((section.width, section.height), size);
                        next.frame = frame;
                        next.usable = usable;
                        next.device = plate_device.or(s.device);
                    }
                }
                SurfaceKind::Door => {
                    let at =
                        (0..doors_before).find(|i| ew_derive::layout::door_left(&before, *i) == s.frame.t[0]);
                    if let Some(i) = at.filter(|i| *i < doors_after) {
                        let size = ew_derive::layout::face_size(&section, &s.kind, Some(i));
                        if let Some((frame, usable)) =
                            ew_derive::layout::surface_frame(&section, &s.kind, Some(i), size)
                        {
                            next.frame = frame;
                            next.usable = usable;
                            next.hinge = Some(ew_derive::layout::door_hinge(&section, i));
                        }
                    }
                }
                SurfaceKind::Left | SurfaceKind::Right | SurfaceKind::Top | SurfaceKind::Bottom => {
                    let size = ew_derive::layout::face_size(&section, &s.kind, None);
                    if let Some((frame, usable)) =
                        ew_derive::layout::surface_frame(&section, &s.kind, None, size)
                    {
                        next.frame = frame;
                        next.usable = usable;
                    }
                }
                _ => {}
            }
            if s.kind != SurfaceKind::Plate {
                next.edge_margin = section.edge_margin;
            }
            // What stands on the surface keeps its place measured from the surface's datum corner: from a corner at the
            // top or the right, its poses follow the change of the sheet's size.
            let datum = (s.datum.clone())
                .or_else(|| tx.project().settings.workshop.datum.clone())
                .unwrap_or_default();
            if let (Some((w0, h0)), Some((w1, h1))) =
                (ew_derive::layout::sheet_size(&s), ew_derive::layout::sheet_size(&next))
            {
                let (dw, dh) = (w1 - w0, h1 - h0);
                let (dx, dy) = match datum {
                    Corner::BottomRight => (dw, Nm::ZERO),
                    Corner::TopLeft => (Nm::ZERO, dh),
                    Corner::TopRight => (dw, dh),
                    Corner::BottomLeft | Corner::Unknown(_) => (Nm::ZERO, Nm::ZERO),
                };
                if dx != Nm::ZERO || dy != Nm::ZERO {
                    shift_on_surface(tx, s.id, dx, dy)?;
                }
            }
            if next != s {
                tx.update::<MountingSurface>(s.id, |x| *x = next)?;
            }
        }
        Ok(Outcome::primary((Kind::Enclosure, self.enclosure.cast())))
    }
}

/// Moves what stands on surface `surface` by `(dx, dy)` in its frame: each carrier's path, each placement on the
/// surface itself (one on a rail goes with the rail) and each hole or gland of the surface.
fn shift_on_surface(tx: &mut TxBuilder<'_>, surface: Id<MountingSurface>, dx: Nm, dy: Nm) -> Result<(), CmdError> {
    let moved = |p: Pt2| Pt2::new(Nm(p.x.0 + dx.0), Nm(p.y.0 + dy.0));
    let carriers: Vec<Id<Carrier>> = (tx.iter::<Carrier>().into_iter())
        .filter(|c| c.surface == surface)
        .map(|c| c.id)
        .collect();
    for c in carriers {
        tx.update::<Carrier>(c, |c| c.path.iter_mut().for_each(|p| *p = moved(*p)))?;
    }
    let placements: Vec<Id<Placement>> = (tx.iter::<Placement>().into_iter())
        .filter(|p| p.carrier == CarrierRef::Surface(surface))
        .map(|p| p.id)
        .collect();
    for p in placements {
        tx.update::<Placement>(p, |p| {
            p.pose.t[0] = Nm(p.pose.t[0].0 + dx.0);
            p.pose.t[1] = Nm(p.pose.t[1].0 + dy.0);
        })?;
    }
    let features: Vec<Id<SurfaceFeature>> = (tx.iter::<SurfaceFeature>().into_iter())
        .filter(|f| f.surface == surface)
        .map(|f| f.id)
        .collect();
    for f in features {
        tx.update::<SurfaceFeature>(f, |f| f.at = moved(f.at))?;
    }
    Ok(())
}

/// `layout.space.edit` (`EditLayoutSpace`): renames a layout space; `name: null` clears its name. Nothing else
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
                text!("refusal-params-space-gone"),
            ));
        }
        let Some(name) = self.name.clone() else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/name",
                text!("refusal-layout-space-edit-no-name"),
            ));
        };
        tx.update::<LayoutSpace>(self.space, |s| s.name = name)?;
        Ok(Outcome::primary((Kind::LayoutSpace, self.space.cast())))
    }
}

/// `layout.enclosure.edit` (`EditEnclosure`): moves an enclosure in its layout space. The pose is a translation
/// with the identity rotation at this version (bayed suites are 1.x). Nothing else changes. Rejected: an
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
                text!("refusal-params-enclosure-gone"),
            ));
        }
        if Quarter::of(self.pose.q) != Some(Quarter::NONE) {
            return tx.reject(text!("refusal-layout-enclosure-turn"));
        }
        let pose = self.pose;
        tx.update::<Enclosure>(self.enclosure, |e| e.pose = pose)?;
        Ok(Outcome::primary((Kind::Enclosure, self.enclosure.cast())))
    }
}

/// A face of a cabinet other than the plate, as `layout.surface.add` names it (doc 12 §2.8).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FaceParam {
    /// The mounting plate (refused: it is made with the enclosure).
    Plate,
    /// A door, seen from outside.
    Door,
    /// The left side.
    Left,
    /// The right side.
    Right,
    /// The top.
    Top,
    /// The bottom.
    Bottom,
}

impl FaceParam {
    /// The stored kind.
    pub fn kind(self) -> SurfaceKind {
        match self {
            FaceParam::Plate => SurfaceKind::Plate,
            FaceParam::Door => SurfaceKind::Door,
            FaceParam::Left => SurfaceKind::Left,
            FaceParam::Right => SurfaceKind::Right,
            FaceParam::Top => SurfaceKind::Top,
            FaceParam::Bottom => SurfaceKind::Bottom,
        }
    }
}

/// The enclosure section of an enclosure's device as the transaction holds it.
fn enclosure_section_of(tx: &TxBuilder<'_>, enclosure: &Enclosure) -> Option<EnclosureSection> {
    tx.get::<Device>(enclosure.device)?
        .parts
        .iter()
        .filter_map(|u| part_copy(tx, u.part.copy))
        .find_map(|c| c.item.sections.enclosure.clone())
}

/// `layout.surface.add` (`AddSurface`, doc 12 §3.7): makes one face of an enclosure from its part (doc 12 §2.8): its
/// frame, its sheet (the part's size, or `width` and `height` where given), a door's hinge and the edge margin. A door
/// is the `door`-th of the part's doors from the left (default the first).
///
/// Rejected in words: the plate (it is made with the enclosure); a face the enclosure has already; a door the part
/// does not list; a face whose size the part does not give and the act does not give either; a width or height of
/// zero or less. Interactive. Outcome: the new surface.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddSurface {
    /// The enclosure.
    #[schemars(with = "UuidS")]
    pub enclosure: Id<Enclosure>,
    /// The face.
    pub kind: FaceParam,
    /// A door's place among the part's doors, from the left (0 …); default the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<u16>,
    /// The sheet's width; default the part's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// The sheet's height; default the part's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub height: Option<Nm>,
}

impl Command for AddSurface {
    const NAME: &'static str = "layout.surface.add";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(enclosure) = tx.get::<Enclosure>(self.enclosure) else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/enclosure",
                text!("refusal-params-enclosure-gone"),
            ));
        };
        let kind = self.kind.kind();
        let tag = tag(tx, enclosure.device);
        if kind == SurfaceKind::Plate {
            return tx.reject(text!("refusal-layout-surface-plate", tag));
        }
        let section = enclosure_section_of(tx, &enclosure).unwrap_or_default();
        let doors = ew_derive::layout::door_count(&section);
        let door = (kind == SurfaceKind::Door).then(|| usize::from(self.door.unwrap_or(0)));
        if door.is_some_and(|i| i >= doors) {
            return tx.reject(text!("refusal-layout-surface-no-door", tag, doors.to_string()));
        }
        let made: Vec<MountingSurface> = tx
            .iter::<MountingSurface>()
            .into_iter()
            .filter(|s| s.enclosure == self.enclosure && s.kind == kind)
            .map(|s| (*s).clone())
            .collect();
        // One record per face; of the doors, one per door the part lists: door `i` is the record whose frame stands at
        // its left edge, and no more door records than the part lists doors.
        let made: Vec<MountingSurface> = match door {
            Some(i) if made.len() < doors => {
                let left = ew_derive::layout::door_left(&section, i);
                made.into_iter().filter(|s| s.frame.t[0] == left).collect()
            }
            _ => made,
        };
        if !made.is_empty() {
            let word = ew_derive::text::surface_of(tag.clone(), &kind, door, doors);
            let page = {
                let q = Q::new(tx.base_model());
                made.iter()
                    .find_map(|s| ew_derive::layout::surface_page(&q, s.id))
            };
            return tx.reject(match page.and_then(|p| crate::words::folio(tx, p)) {
                Some(folio) => text!("refusal-layout-surface-exists-page", word, folio),
                None => text!("refusal-layout-surface-exists", word),
            });
        }
        for (value, at) in [(self.width, "/width"), (self.height, "/height")] {
            if value.is_some_and(|v| v <= Nm::ZERO) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    text!("refusal-layout-surface-size"),
                ));
            }
        }
        let part_size = ew_derive::layout::face_size(&section, &kind, door);
        // A face whose size the cabinet's part does not give: Edit part… opens the part at the size it lacks.
        let no_size = |tx: &TxBuilder<'_>| {
            let word = ew_derive::text::surface_word(&kind, door, doors);
            let lack = tx.get::<Device>(enclosure.device).and_then(|d| {
                d.parts.iter().find_map(|u| {
                    let c = part_copy(tx, u.part.copy)?;
                    PartLack::of(u.part.copy, &c.item, PartFeature::Cabinet)
                        .or_else(|| PartLack::of(u.part.copy, &c.item, PartFeature::Surfaces))
                })
            });
            tx.reject_lacking(text!("refusal-layout-surface-no-size", tag.clone(), word), lack)
        };
        let size = match (self.width, self.height, part_size) {
            (Some(w), Some(h), _) => (w, h),
            (w, h, Some((pw, ph))) => (w.unwrap_or(pw), h.unwrap_or(ph)),
            _ => return no_size(tx),
        };
        let Some((frame, usable)) = ew_derive::layout::surface_frame(&section, &kind, door, Some(size))
        else {
            return no_size(tx);
        };
        let hinge = door.map(|i| ew_derive::layout::door_hinge(&section, i));
        let id: Id<MountingSurface> = tx.mint();
        tx.insert(MountingSurface {
            id,
            v: 1,
            enclosure: self.enclosure,
            kind,
            device: None,
            frame,
            usable,
            pe_bond: false,
            hinge,
            datum: None,
            edge_margin: section.edge_margin,
            extra: Extra::new(),
        })?;
        Ok(Outcome::primary((Kind::MountingSurface, id.cast())))
    }
}

/// `layout.surface.edit` (`EditSurface`, doc 12 §3.7): changes a mounting surface: whether it is bonded to protective
/// earth, its sheet's width and height, a door's hinge, the corner its holes are measured from and its edge margin.
/// `datum`, `hinge` and `edge_margin` given as `null` clear the member (the datum then is the project's). A size that
/// leaves a placement outside the sheet is done; EW-LAY-010 reports it. Rejected: a surface that does not exist
/// (`/surface`); a width, height or margin below zero (zero for a size); a hinge on a surface that is no door.
/// Interactive. Outcome: the surface.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditSurface {
    /// The mounting surface.
    #[schemars(with = "UuidS")]
    pub surface: Id<MountingSurface>,
    /// Whether it is bonded to protective earth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pe_bond: Option<bool>,
    /// The sheet's width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub width: Option<Nm>,
    /// The sheet's height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub height: Option<Nm>,
    /// A door's hinge; `null` clears it.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub hinge: Option<Option<ew_schema::Hinge>>,
    /// The corner its holes are measured from; `null`: the project's.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub datum: Option<Option<ew_schema::Corner>>,
    /// The edge margin; `null`: none.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<Option<NmS>>")]
    pub edge_margin: Option<Option<Nm>>,
}

impl Command for EditSurface {
    const NAME: &'static str = "layout.surface.edit";

    fn meta() -> CommandMeta {
        CommandMeta::edit(Self::NAME)
    }

    fn run(&self, tx: &mut TxBuilder<'_>) -> Result<Outcome, CmdError> {
        let Some(surface) = tx.get::<MountingSurface>(self.surface) else {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/surface",
                text!("refusal-params-surface-gone"),
            ));
        };
        for (value, at) in [(self.width, "/width"), (self.height, "/height")] {
            if value.is_some_and(|v| v <= Nm::ZERO) {
                return Err(CmdError::invalid_params(
                    Self::NAME,
                    at,
                    text!("refusal-layout-surface-size"),
                ));
            }
        }
        if self.edge_margin.flatten().is_some_and(|m| m < Nm::ZERO) {
            return Err(CmdError::invalid_params(
                Self::NAME,
                "/edge_margin",
                text!("refusal-layout-surface-margin"),
            ));
        }
        if matches!(self.hinge, Some(Some(_))) && surface.kind != SurfaceKind::Door {
            return tx.reject(text!("refusal-layout-surface-hinge"));
        }
        let size = match (self.width, self.height) {
            (None, None) => None,
            (w, h) => {
                let (ow, oh) = ew_derive::layout::sheet_size(&surface).unwrap_or((Nm::ZERO, Nm::ZERO));
                Some((w.unwrap_or(ow), h.unwrap_or(oh)))
            }
        };
        let edit = self.clone();
        tx.update::<MountingSurface>(self.surface, |s| {
            if let Some(on) = edit.pe_bond {
                s.pe_bond = on;
            }
            if let Some((w, h)) = size {
                s.usable = vec![
                    Pt2::new(Nm::ZERO, Nm::ZERO),
                    Pt2::new(w, Nm::ZERO),
                    Pt2::new(w, h),
                    Pt2::new(Nm::ZERO, h),
                ];
            }
            if let Some(hinge) = edit.hinge {
                s.hinge = hinge;
            }
            if let Some(datum) = edit.datum {
                s.datum = datum;
            }
            if let Some(margin) = edit.edge_margin {
                s.edge_margin = margin;
            }
        })?;
        Ok(Outcome::primary((Kind::MountingSurface, self.surface.cast())))
    }
}
