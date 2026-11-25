// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout's commands that place an assembly with its members, and the auto-fill of a rail.

#[allow(clippy::wildcard_imports)]
use super::*;

// ---------------------------------------------------------------------------------------------------------
// Assemblies and bulk placement

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

/// A device a bulk command measures before it places it: its record, its part, its rail item without a placement yet
/// and the extent.
struct Measured {
    device: std::sync::Arc<Device>,
    part: Option<PartRef>,
    item: RailItem,
    ext: RailExtent,
}

/// Builds a device's item (turned 0°, unlocked; the part as `layout.place` records it), without minting its placement.
fn measure(tx: &TxBuilder<'_>, q: &Q, device: std::sync::Arc<Device>) -> Measured {
    let part = primary_part(tx, &device);
    let (item, ext) = drop_item(q, &device, part.as_ref(), false, false, None);
    Measured {
        device,
        part,
        item,
        ext,
    }
}

/// Mints a measured device's placement: the member it places.
fn mint_member(tx: &mut TxBuilder<'_>, m: Measured) -> Member {
    let id: Id<Placement> = tx.mint();
    Member {
        device: m.device,
        part: m.part,
        id,
        item: RailItem {
            placement: Some(id),
            ..m.item
        },
        ext: m.ext,
    }
}

/// Mints a member's placement and builds its item (turned 0°, unlocked; the part as `layout.place` records it).
fn member(tx: &mut TxBuilder<'_>, q: &Q, device: std::sync::Arc<Device>) -> Member {
    let m = measure(tx, q, device);
    mint_member(tx, m)
}

/// The block of the items of `members` laid at their minimum gaps: each member's offset from the block's left
/// edge, and the item the block resolves as: the block's width, its pads the first member's before and the last
/// member's after (a strip's bracket width counts as its pad), not a strip, no placement. One member is its own item.
fn block_of<'a>(members: impl Iterator<Item = &'a RailItem>) -> (Vec<i128>, RailItem) {
    let items: Vec<&RailItem> = members.collect();
    if let [one] = items.as_slice() {
        return (vec![0], **one);
    }
    let mut offsets = Vec::with_capacity(items.len());
    let mut end = 0i128;
    let mut prev: Option<&RailItem> = None;
    for m in &items {
        let off = prev.map_or(0, |p| end.saturating_add(min_gap(p, m)));
        offsets.push(off);
        end = off.saturating_add(m.width());
        prev = Some(m);
    }
    let pad = [
        items.first().map_or(0, |m| min_gap(&BARE, m)),
        items.last().map_or(0, |m| min_gap(m, &BARE)),
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
pub(super) fn rail_in_sentence(q: &Q, carrier: Id<Carrier>) -> Text {
    carrier_label(q, carrier).map_or_else(
        || text!("refusal-layout-the-rail"),
        crate::words::carrier_in_sentence,
    )
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

/// The root of the assembly a device belongs to: the device when it has children, else its
/// parent; `None` for a device that is neither.
fn assembly_root(q: &Q, device: &Device) -> Option<Id<Device>> {
    if has_children(q, device.id) {
        Some(device.id)
    } else {
        device.parent
    }
}

/// [`assembly_root`] read through the transaction: the device's children through
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
/// order their devices but which they compute on the base snapshot (ADR-0008:
/// every lookup is recorded in the read set): `mountable(dev)`, `unplaced(space)` and its device lists, the location
/// (`StructureOf`) and the sort keys (`DeviceTag`, the drawn page order). These derived inputs cannot be read through
/// the transaction one by one, so the tables they are derived from are recorded whole: a
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

/// Whether the layout expects a device (`mountable(dev)`); a structure cycle counts as not.
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

/// `layout.place_assembly` (`PlaceAssembly`, FR-LAY-061): places the unplaced members of an assembly on a
/// rail in their member order ([`member_order`], the order EW-LAY-016 checks), in one step.
///
/// The assembly of `device` is the device itself when it has children, else its parent's. The members placed are those
/// of [`member_order`] that are mountable (`mountable(dev)`) and have no placement anywhere; each records its
/// primary part, turned 0°, and must fit the rail's profile (an unfit member refuses the whole command with
/// `layout.place`'s text). When no member is on the rail yet, the members go as one block at their minimum gaps
/// whose left edge is `s`, resolved in `mode` (`free` by default, or `push`) as one item. When some members are on the
/// rail, `s` and `mode` are not used: each new member is pushed in after its predecessor (the nearest earlier member on
/// the rail, a member placed by this command included), or before its first placed successor when it has no
/// predecessor; every pushed neighbour is written in the same step.
///
/// Rejected: a device or carrier that does not exist (`/device`, `/carrier`); a duct or a surface ("An assembly is
/// placed on a rail."); `mode: swap`; a device without an assembly; nothing to place; `s` missing when no member is on
/// the rail (`/s`); an unfit member; the refusals of the mode. Interactive; `Bulk` when it places more than 200.
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
                text!("refusal-params-device-gone"),
            ));
        };
        let target = target(tx, name, "/carrier", self.carrier.to_ref())?;
        let (PoseOn::Rail(axis), PlaceOnParam::Carrier(rail)) = (target.on, self.carrier) else {
            return tx.reject(text!("refusal-layout-assembly-on-rail"));
        };
        if self.mode == Some(PlaceMode::Swap) {
            return tx.reject(text!("refusal-layout-assembly-swap"));
        }
        let base_model = tx.base_model().clone();
        let q = Q::new(&base_model);
        let Some(root) = assembly_root_tx(tx, &device) else {
            return tx.reject(text!("refusal-layout-not-assembly", tag(tx, self.device)));
        };
        // How the members are chosen and ordered is in the read set.
        note_members(tx, root);
        note_choice_reads(tx, false);
        let order = member_order(&q, root);
        let devices: Vec<std::sync::Arc<Device>> =
            order.iter().filter_map(|d| tx.get::<Device>(*d)).collect();
        // Override 2: one read set for every device the command reads, their placements included.
        note_rail_reads(tx, rail, &devices.iter().map(|d| d.as_ref()).collect::<Vec<_>>());
        let (on_rail, unplaced) = members_by_place(tx, &q, rail, &devices);
        if unplaced.is_empty() {
            return tx.reject(text!("refusal-layout-assembly-placed", tag(tx, root)));
        }
        let block_s = match (on_rail.is_empty(), self.s) {
            (true, None) => {
                return Err(CmdError::invalid_params(
                    name,
                    "/s",
                    text!(
                        "refusal-layout-no-member-on-rail",
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
        let names: BTreeMap<Id<Placement>, String> = members
            .iter()
            .map(|m| (m.id, tag(tx, m.device.id).to_string()))
            .collect();
        let base = rail_items(&q, rail);
        let mut mem = base.clone();
        let on = AssemblyOnRail {
            q: &q,
            rail,
            axis: &axis,
            order: &order,
            names: &names,
        };
        match block_s {
            Some(s_left) => self.place_block(tx, &on, root, &members, s_left, &mut mem)?,
            None => insert_members(tx, &on, on_rail, &members, &mut mem)?,
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
                label_at: None,
                extra: Extra::new(),
            })?;
        }
        Ok(Outcome::primary((Kind::Placement, primary.cast())))
    }
}

/// What `layout.place_assembly` places an assembly's members by: the base snapshot, the rail and its axis, the order
/// of the members, and the tag of each placement the command mints.
#[derive(Clone, Copy)]
struct AssemblyOnRail<'a> {
    q: &'a Q,
    rail: Id<Carrier>,
    axis: &'a RailAxis,
    order: &'a [Id<Device>],
    names: &'a BTreeMap<Id<Placement>, String>,
}

impl PlaceAssembly {
    /// `layout.place_assembly` when no member is on the rail: the new members as one block, its left end at `s_left`,
    /// resolved by the command's mode.
    fn place_block(
        &self,
        tx: &TxBuilder<'_>,
        on: &AssemblyOnRail<'_>,
        root: Id<Device>,
        members: &[Member],
        s_left: i128,
        mem: &mut Vec<RailItem>,
    ) -> Result<(), CmdError> {
        let (offsets, block) = block_of(members.iter().map(|m| &m.item));
        let item = match members {
            [one] => tag(tx, one.device.id),
            _ => text!("refusal-layout-the-assembly", tag(tx, root)).into(),
        };
        let wording = Wording {
            carrier: on.rail,
            mode: self.mode.unwrap_or(PlaceMode::Free),
            item,
            names: on.names,
        };
        let r = resolve_items(tx, on.q, mem, on.axis.length, block, s_left, &wording)?;
        apply_moved(mem, &r.moved);
        for (m, off) in members.iter().zip(offsets) {
            mem.push(item_at(&m.item, r.s_left.saturating_add(off)));
        }
        Ok(())
    }
}

/// The assembly's members by where they are: the placements of each member on `rail`, and the members not placed
/// anywhere that can be mounted.
#[allow(clippy::type_complexity)]
fn members_by_place(
    tx: &TxBuilder<'_>,
    q: &Q,
    rail: Id<Carrier>,
    devices: &[std::sync::Arc<Device>],
) -> (
    BTreeMap<Id<Device>, Vec<Id<Placement>>>,
    Vec<std::sync::Arc<Device>>,
) {
    let mut on_rail: BTreeMap<Id<Device>, Vec<Id<Placement>>> = BTreeMap::new();
    let mut unplaced = Vec::new();
    for d in devices {
        let placements = tx.placements_of_device(d.id);
        if placements.is_empty() {
            if is_mountable(q, d.id) {
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
    (on_rail, unplaced)
}

/// `layout.place_assembly` when some members are on the rail: each new member, in member
/// order, is pushed in after its predecessor (its doubled centre the smallest value above the predecessor's, so that
/// [`resolve`] starts it at the predecessor's end plus the minimum gap), else before its first placed successor; the
/// in-memory list `mem` carries every member placed so far, so a member this command placed and a later insertion
/// pushes is re-posed in memory and inserted once, at its final place.
fn insert_members(
    tx: &TxBuilder<'_>,
    rail_of: &AssemblyOnRail<'_>,
    mut on: BTreeMap<Id<Device>, Vec<Id<Placement>>>,
    members: &[Member],
    mem: &mut Vec<RailItem>,
) -> Result<(), CmdError> {
    let AssemblyOnRail {
        q,
        rail,
        axis,
        order,
        names,
    } = *rail_of;
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
        // The slot is `ew_derive::layout::member_slot`, which the layout tool's ghost reads.
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

/// The order `layout.auto_fill` places its devices in.
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

/// `layout.auto_fill`'s default list: the unplaced devices of the rail's space whose primary part does not
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

/// `layout.auto_fill` (`AutoFill`, FR-LAY-013): places a list of devices on a rail, next-fit from the
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
            return tx.reject(text!("refusal-layout-auto-fill-rail"));
        };
        let base_model = tx.base_model().clone();
        let q = Q::new(&base_model);
        // The default list, the refusals' inputs and the sort keys are in the
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
                            text!("refusal-params-device-gone"),
                        ));
                    }
                    if !seen.insert(*d) {
                        return Err(CmdError::invalid_params(
                            name,
                            format!("/devices/{i}"),
                            text!("refusal-params-device-twice"),
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
        let mut notices: Vec<Option<Text>> = vec![None; devices.len()];
        let mut candidates: Vec<Candidate> = Vec::new();
        for (i, d) in devices.iter().enumerate() {
            match fill_refusal(tx, &q, d, space.as_deref(), target.profile.as_ref()) {
                Some(why) => notices[i] = Some(text!("refusal-layout-fill-notice", tag(tx, d.id), why)),
                None => candidates.push((i, std::sync::Arc::clone(d))),
            }
        }
        sort_candidates(tx, &q, self.order, &mut candidates);
        let mut mem = rail_items(&q, carrier);
        let mut last: Option<RailItem> = None;
        let mut placed: Vec<(Member, Pose3)> = Vec::new();
        for unit in fill_units(tx, &q, candidates) {
            let measured: Vec<Measured> = unit
                .iter()
                .map(|(_, d)| measure(tx, &q, std::sync::Arc::clone(d)))
                .collect();
            let (offsets, item) = block_of(measured.iter().map(|m| &m.item));
            let floor = last.map_or(i128::MIN, |l| l.s1.saturating_add(min_gap(&l, &item)));
            let Some(s) = next_fit(&mem, axis.length, &item, floor) else {
                for (i, d) in &unit {
                    notices[*i] = Some(text!("refusal-layout-fill-no-room", tag(tx, d.id), rail.clone()));
                }
                continue;
            };
            // The placement IDs are minted after the fit, only for the members placed
            let members: Vec<Member> = measured.into_iter().map(|m| mint_member(tx, m)).collect();
            for (m, off) in members.into_iter().zip(offsets) {
                let at = item_at(&m.item, s.saturating_add(off));
                mem.push(at);
                last = Some(at);
                let pose = rail_pose(tx, &axis, at.s0.saturating_add(m.ext.clip), &Pose3::IDENTITY)?;
                check_pose_on(tx, &target, &pose)?;
                placed.push((m, pose));
            }
        }
        let notices: Vec<Text> = notices.into_iter().flatten().collect();
        if placed.is_empty() {
            let why = if notices.is_empty() {
                text!("refusal-layout-auto-fill-no-device").into()
            } else {
                crate::words::list(notices.into_iter().map(TextArg::from).collect())
            };
            return tx.reject(text!("refusal-layout-auto-fill-nothing", rail, why));
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
                label_at: None,
                extra: Extra::new(),
            })?;
        }
        Ok(Outcome {
            primary: Some((Kind::Carrier, carrier.cast())),
            counts: None,
            notices,
        })
    }
}

/// Why `layout.auto_fill` does not place a listed device, after the tag and a colon: placed already, of
/// another space, not mountable, or a mount kind the rail's profile refuses; `None` when it is a candidate.
fn fill_refusal(
    tx: &TxBuilder<'_>,
    q: &Q,
    device: &Device,
    space: Option<&LayoutSpace>,
    profile: Option<&MountKind>,
) -> Option<Text> {
    if !tx.placements_of_device(device.id).is_empty() {
        return Some(text!("refusal-layout-fill-placed"));
    }
    let location = q
        .try_get::<StructureOf>(device.id)
        .ok()
        .and_then(|s| s.node(Aspect::Location));
    if space.is_none_or(|s| Some(s.location) != location) {
        return Some(text!("refusal-layout-fill-other-space"));
    }
    if !is_mountable(q, device.id) {
        return Some(text!("refusal-layout-fill-not-mountable"));
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
        text!(
            "refusal-layout-fill-no-fit",
            mount_words(&mount),
            mount_words(profile)
        )
    } else {
        text!("refusal-layout-fill-not-rail", mount_words(&mount))
    })
}

/// Sorts `layout.auto_fill`'s candidates in `order`.
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
/// the place of the first of them, in [`member_order`].
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
