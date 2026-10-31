// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The panel layout records (doc 03 §16, doc 12 §2.2; WP-S4.01 D3): `layout_space` (rank 50), `enclosure` (51),
//! `mounting_surface` (52), `carrier` (53) and `placement` (54), all in `layout/<space>.jsonl`.
//!
//! Typed within the unreleased format 0.1 under ADR-0011 amendment 4 (decision log ID-334): the kinds were reserved
//! with their ranks and ownership, no build wrote them before, and only the members WP-S4.01 uses are typed. Every
//! other member doc 03 §16 names stays verbatim in `extra` (`install`, `thermal`, `policy`, `install_type`, `doors`,
//! `max_depth`, `segment`, `variant`), as do the surface kinds other than `plate`, the carrier kinds `busbar` and
//! `surface`, and the `placement` variant of [`CarrierRef`] (each an `Unknown` variant, kept as read).
//!
//! Geometry is integer nanometres in the frame of the mounting surface: `u` to the right, `v` up, `w` out of the
//! surface (doc 03 §16). A placement's pose is relative to its carrier (doc 12 §2.3), so moving a rail moves its
//! devices; label text, footprints and world positions are queries, never stored.
//!
//! ```
//! use ew_schema::{CarrierRef, Record};
//!
//! let line = r#"{"$k":"placement","id":"0193a2c4-9b10-7e21-8c44-1d2e3f405162","device":"0192f1c4-5a10-7c3e-8b21-6f0d9a4e2b10","carrier":{"carrier":"0193a2b0-41c2-7d0a-9f11-2a3b4c5d6e70"},"pose":{"t":[96000000,0,0],"q":[1000000000,0,0,0]}}"#;
//! let record: Record = serde_json::from_str(line).unwrap();
//! let Record::Placement(p) = &record else { panic!("a placement") };
//! assert!(matches!(p.carrier, CarrierRef::Carrier(_)) && !p.locked && p.part.is_none());
//! assert_eq!(serde_json::to_string(&record).unwrap(), line);
//! ```

use ew_ids::Id;
use ew_mltext::Ident;
use ew_units::{Nm, Pose3, Pt2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::json::{Extra, de_extra, is_false, is_v1, v1};
use crate::library::MountKind;
use crate::proxy::{IdentS, NmS, Pose3S, SurfacePt2S, UuidS};
use crate::track::open_enum;
use crate::value::PartRef;
use crate::{Device, Function, StructureNode};

// ---------------------------------------------------------------------------------------------------------
// Layout space and enclosure

/// One cabinet or bayed suite (doc 03 §16, rank 50, `layout/<own id>.jsonl`; FR-LAY-001): the anchor of every
/// other layout record. At the MVP a location holds at most one layout space (WP-S4.01 D3, refused by
/// `layout.space.create`, reported by EW-LAY-010 on loaded data).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct LayoutSpace {
    /// The record ID; it names the layout file.
    #[schemars(with = "UuidS")]
    pub id: Id<LayoutSpace>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The location structure node the space lays out (+J-E-2416); deleting the node is refused while the space
    /// exists (delete policy forbid).
    #[schemars(with = "UuidS")]
    pub location: Id<StructureNode>,
    /// The display name; absent: none was given (the views show the location).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<IdentS>")]
    pub name: Option<Ident>,
    /// Unknown members (`install`, `thermal`, `policy` until their packages type them) and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// An enclosure (doc 03 §16, rank 51, the layout file of its space): the cabinet body, tagged by its device, which
/// carries the enclosure part (-UC1, NSYPLA1074G), so the part reaches the BOM by the one path (ADR-0006). At the
/// MVP a space holds at most one enclosure (WP-S4.01 D3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Enclosure {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Enclosure>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The layout space (delete policy cascade).
    #[schemars(with = "UuidS")]
    pub space: Id<LayoutSpace>,
    /// The device that carries the enclosure part (delete policy forbid: the device cannot vanish under its
    /// enclosure).
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// The pose in the space: origin at the bottom-left-rear outside corner, x right, y up, z towards the door.
    #[schemars(with = "Pose3S")]
    pub pose: Pose3,
    /// Unknown members (`install_type`, `doors`) and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Mounting surface

/// A mounting surface of an enclosure (doc 03 §16, rank 52, the layout file of its enclosure's space): the plane
/// carriers and placements sit on. A 2D layout page is a view of one surface (FR-LAY-010). At the MVP an enclosure
/// holds at most one `plate` surface (WP-S4.01 D3).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MountingSurface {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<MountingSurface>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The enclosure (delete policy cascade).
    #[schemars(with = "UuidS")]
    pub enclosure: Id<Enclosure>,
    /// The kind of surface.
    pub kind: SurfaceKind,
    /// The device of a separate surface part (the mounting plate NSYPMM107, a child of the enclosure's device);
    /// deleting that device clears the member (delete policy unlink).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub device: Option<Id<Device>>,
    /// The surface frame in the enclosure: `u`, `v` in the plane, `w` outward.
    #[schemars(with = "Pose3S")]
    pub frame: Pose3,
    /// The usable polygon in the surface frame; empty: none recorded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<SurfacePt2S>")]
    pub usable: Vec<Pt2>,
    /// Whether the surface is bonded to PE (FR-LAY-058).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pe_bond: bool,
    /// Unknown members (`max_depth`) and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The kind of a mounting surface (doc 03 §16). Format 0.1 types the mounting plate; door, side, roof, floor and
/// gland surfaces are kept verbatim as [`SurfaceKind::Unknown`] until their packages type them.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "SurfaceKind")]
pub enum SurfaceKind {
    /// The mounting plate.
    Plate,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

// ---------------------------------------------------------------------------------------------------------
// Carrier

/// A carrier on a mounting surface (doc 03 §16, rank 53, the layout file of its surface's space): a DIN rail or a
/// wiring duct. Its placements' poses are relative to it, so moving a rail moves its devices.
///
/// At the MVP carriers are axis-parallel (WP-S4.01 D5): a rail's `path` is two points on one horizontal or vertical
/// line, a duct's an orthogonal polyline; the commands refuse other angles.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Carrier {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Carrier>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The mounting surface (delete policy cascade; changing it moves the carrier and its placements to that
    /// surface's layout file).
    #[schemars(with = "UuidS")]
    pub surface: Id<MountingSurface>,
    /// The kind of carrier.
    pub kind: CarrierKind,
    /// The layout-only part, counted in the BOM by length (FR-LAY-008); its copy cannot be deleted while used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartRef>,
    /// The path in the surface frame: a rail's start and end, a duct's centreline vertices.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(with = "Vec<SurfacePt2S>")]
    pub path: Vec<Pt2>,
    /// The kind's parameters (doc 12 §2.4).
    pub params: CarrierParams,
    /// Whether the carrier is bonded to PE (FR-LAY-058).
    #[serde(default, skip_serializing_if = "is_false")]
    pub pe_bond: bool,
    /// Unknown members and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// The kind of a carrier (doc 03 §16). Format 0.1 types rails and ducts; busbar systems and the surface itself are
/// kept verbatim as [`CarrierKind::Unknown`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "CarrierKind")]
pub enum CarrierKind {
    /// A mounting (DIN) rail.
    Rail,
    /// A wiring duct.
    Duct,
    /// A value of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// The parameters of a carrier (doc 12 §2.4), tagged by the kind: `{"rail": {…}}` or `{"duct": {…}}`.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "CarrierParams")]
pub enum CarrierParams {
    /// A rail's parameters.
    Rail(RailParams),
    /// A duct's parameters.
    Duct(DuctParams),
    /// A form of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

/// A rail's parameters (doc 12 §2.4).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct RailParams {
    /// The rail profile (TS 35×7.5); the mount kinds that fit it are a table in `ew-derive` (doc 12 §3.3).
    pub profile: MountKind,
    /// Unknown members (`perforated`).
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// A duct's parameters (doc 12 §2.4): the nominal size, from which the label "W×H" ("40×60") is generated.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct DuctParams {
    /// The width across the duct.
    #[schemars(with = "NmS")]
    pub width: Nm,
    /// The height out of the plate.
    #[schemars(with = "NmS")]
    pub height: Nm,
    /// The slot pitch; absent: from the part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<NmS>")]
    pub slot_pitch: Option<Nm>,
    /// Whether the duct has its cover; absent: not recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<bool>,
    /// Unknown members (`inner_w`, `inner_h`, `finger_w`, joint styles, `class_set`).
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

// ---------------------------------------------------------------------------------------------------------
// Placement

/// A placement (doc 03 §16 `Placement3D`, doc 12 §2.3, rank 54, the layout file of its carrier's space): a device,
/// or one of its functions, on a carrier at a pose relative to the carrier.
///
/// It stores no tag text (labels are `layout_label(placement)`, D-02) and no geometry: the footprint comes from the
/// part. A placement cannot outlive its device (delete policy cascade, EW-LAY-002) and keeps its carrier (forbid).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Placement {
    /// The record ID.
    #[schemars(with = "UuidS")]
    pub id: Id<Placement>,
    /// Per-kind schema version `$v`, omitted when 1.
    #[serde(rename = "$v", default = "v1", skip_serializing_if = "is_v1")]
    pub v: u16,
    /// The placed device (delete policy cascade).
    #[schemars(with = "UuidS")]
    pub device: Id<Device>,
    /// One function placed on its own (a door handle); absent: the whole device (delete policy cascade).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<UuidS>")]
    pub function: Option<Id<Function>>,
    /// The part use of the device this placement realises; absent: the primary mountable part. `layout.place`
    /// always writes it (WP-S4.01 D6); no delete policy: a placement whose part use left the device is reported
    /// by EW-LAY-003, never changed silently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<PartRef>,
    /// The carrier or surface it sits on (delete policy forbid).
    pub carrier: CarrierRef,
    /// The pose relative to the carrier: on a rail `t = (s, 0, 0)` with `q` identity or 180° about `w`; on a
    /// surface `t = (u, v, 0)` rotated by k·90° about `w` (WP-S4.01 D5).
    #[schemars(with = "Pose3S")]
    pub pose: Pose3,
    /// Excluded from push, swap and auto-fill (FR-LAY-006).
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Unknown members (`segment`, `variant`) and plugin keys.
    #[serde(flatten, deserialize_with = "de_extra")]
    pub extra: Extra,
}

/// What a placement sits on (doc 12 §2.3 `CarrierRef`): `{"carrier": id}` or `{"surface": id}`. Piggy-back
/// mounting on another placement (1.0) is kept verbatim as [`CarrierRef::Unknown`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[serde(remote = "Self")]
#[schemars(rename = "CarrierRef")]
pub enum CarrierRef {
    /// A rail or duct.
    Carrier(#[schemars(with = "UuidS")] Id<Carrier>),
    /// The mounting surface itself.
    Surface(#[schemars(with = "UuidS")] Id<MountingSurface>),
    /// A form of a newer format, kept verbatim.
    #[serde(skip)]
    Unknown(Value),
}

open_enum!(SurfaceKind, CarrierKind, CarrierParams, CarrierRef);
