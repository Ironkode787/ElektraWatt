// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a device's parts bring with them (doc 07 §2.5 "Accessories", FR-PRT-005, FR-REP-051): the one path.
//!
//! [`PartAccessories`] (`part_accessories(device)`) gives every accessory of a device from its parts' declarations,
//! with the device's choices applied (`Device.accessory_choices`): the `unit` accessories of its part uses and, for the
//! device of an enclosure, of its carriers by their length; for a strip, the `strip` accessories of its terminals'
//! parts ([`strip_accessories`], each after the last terminal that names it) and its end brackets ([`strip_bracket`],
//! counted per rail group by `group_accessories`, two on no rail). The parts list, the strip's length on its rail
//! (`strip_layout`), the end brackets of the layout and the checks read it and nothing else.
//!
//! An accessory whose copy the project does not hold is left out (a reference that names nothing, doc 05 §13.2), as
//! a part line whose copy is missing is.

use std::collections::BTreeSet;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{
    AccessoryChoice, AccessoryKind, AccessoryPer, Carrier, CarrierRef, Device, LibCopy, MountingSurface,
    Part, PartAccessory, PartKind, PartRef, Placement,
};
use ew_units::Dec3;

use crate::Q;
use crate::layout::{GroupAccessories, strip_terminals};
use crate::query::{Policy, Query};

/// One accessory of a device ([`PartAccessories`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DerivedAccessory {
    /// The part counted: the choice's part, else the accessory copy the declaration names.
    pub part: PartRef,
    /// The accessory copy the declaration names (what a choice is keyed by).
    pub of: Id<Part>,
    /// The quantity in the counted part's order unit.
    pub qty: Dec3,
    /// The parts it comes with, in the order they name it (a strip's end plate named by two terminal parts).
    pub with: Vec<Id<Part>>,
    /// Per what it counts.
    pub per: AccessoryPer,
    /// What it is to a strip.
    pub kind: Option<AccessoryKind>,
    /// For a strip's `strip` accessory, the terminal it stands after; `None` for any other.
    pub after: Option<Id<Device>>,
}

/// The copy ID an accessory declaration names: `v5(NS_LIBCOPY, library ‖ item)`.
fn copy_of(a: &PartAccessory) -> Id<Part> {
    ew_ids::v5::lib_copy_id(a.part.library.as_uuid(), a.part.item.as_uuid())
}

/// What `choices` make of the declared accessory `a`: the part counted, or `None` when it is not taken (a choice
/// without a part, or an optional accessory nobody took).
fn taken(choices: &[AccessoryChoice], a: &PartAccessory) -> Option<PartRef> {
    let of = copy_of(a);
    match choices.iter().find(|c| c.of == of) {
        Some(c) => c.part.clone(),
        None if a.optional => None,
        None => Some(PartRef::new(of)),
    }
}

/// The `unit` accessories of `units` pieces (or metres) of part `part`, with `choices` applied.
fn unit_accessories(
    q: &Q,
    part: Id<Part>,
    units: Dec3,
    choices: &[AccessoryChoice],
) -> Vec<DerivedAccessory> {
    let parts = q.parts();
    let Some(copy) = parts.get(part) else {
        return Vec::new();
    };
    copy.item
        .accessories
        .iter()
        .filter(|a| a.per == AccessoryPer::Unit)
        .filter_map(|a| {
            let counted = taken(choices, a)?;
            parts.get(counted.copy)?;
            Some(DerivedAccessory {
                part: counted,
                of: copy_of(a),
                qty: a.qty.checked_mul(units).unwrap_or(Dec3(i64::MAX)),
                with: vec![part],
                per: AccessoryPer::Unit,
                kind: a.kind.clone(),
                after: None,
            })
        })
        .collect()
}

/// The `unit` accessories of the carriers of the device of an enclosure, by each carrier's length in metres, with the
/// device's choices applied: a duct's cover is as long as the duct. `carriers` are the carriers to count.
pub fn carrier_accessories(q: &Q, device: &Device, carriers: &[Id<Carrier>]) -> Vec<DerivedAccessory> {
    let table = q.table::<Carrier>();
    let mut out: Vec<DerivedAccessory> = Vec::new();
    for c in carriers {
        let Some(part) = table.get(*c).and_then(|rec| rec.part.clone()) else {
            continue;
        };
        let metres = crate::layout::metres_of_nm(crate::layout::carrier_length(q, *c).unwrap_or(0));
        for a in unit_accessories(q, part.copy, metres, &device.accessory_choices) {
            match out.iter_mut().find(|o| o.part == a.part && o.of == a.of) {
                Some(o) => o.qty = Dec3(o.qty.0.saturating_add(a.qty.0)),
                None => out.push(a),
            }
        }
    }
    out
}

/// The `strip` accessories of a strip (doc 07 §2.5): once per accessory its terminals' parts name, standing after the
/// last terminal that names it, in the order of the terminals they stand after, with the strip's choices applied. Empty
/// for a device that is not a strip.
pub fn strip_accessories(q: &Q, strip: &Device) -> Vec<DerivedAccessory> {
    if strip.aspects.strip.is_none() {
        return Vec::new();
    }
    let parts = q.parts();
    let mut out: Vec<DerivedAccessory> = Vec::new();
    for t in strip_terminals(q, strip.id) {
        for u in &t.parts {
            let Some(copy) = parts.get(u.part.copy) else {
                continue;
            };
            for a in copy
                .item
                .accessories
                .iter()
                .filter(|a| a.per == AccessoryPer::Strip)
            {
                let of = copy_of(a);
                let with = match out.iter().position(|o| o.of == of) {
                    Some(i) => {
                        let mut prev = out.remove(i);
                        if !prev.with.contains(&u.part.copy) {
                            prev.with.push(u.part.copy);
                        }
                        prev.with
                    }
                    None => vec![u.part.copy],
                };
                let Some(counted) = taken(&strip.accessory_choices, a) else {
                    continue;
                };
                if parts.get(counted.copy).is_none() {
                    continue;
                }
                out.push(DerivedAccessory {
                    part: counted,
                    of,
                    qty: a.qty,
                    with,
                    per: AccessoryPer::Strip,
                    kind: a.kind.clone(),
                    after: Some(t.id),
                });
            }
        }
    }
    out
}

/// Where a strip's end bracket comes from ([`strip_bracket`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BracketFrom {
    /// The strip's choice of the bracket its terminals' parts declare.
    Choice,
    /// A part use of kind `end_bracket` (a project made before accessories), its index in `Device.parts`: the rails
    /// count it on that part use's line, as before.
    PartUse(usize),
    /// The `rail_group` accessory of the first of its terminals whose part names one.
    Declared,
}

/// A strip's end bracket ([`strip_bracket`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StripBracket {
    /// The bracket part.
    pub part: PartRef,
    /// Its `mech.width` in nanometres (absent or negative: 0): the width the strip declares to its rail.
    pub width: i128,
    /// Where it comes from.
    pub from: BracketFrom,
    /// The declaration it answers (`None` for the older part use): its accessory copy, quantity per bracket, kind and
    /// the part it comes with.
    pub declared: Option<DeclaredBracket>,
}

/// The `rail_group` declaration a strip's end bracket answers ([`StripBracket::declared`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DeclaredBracket {
    /// The accessory copy the declaration names.
    pub of: Id<Part>,
    /// The quantity per bracket.
    pub qty: Dec3,
    /// What it is to a strip.
    pub kind: Option<AccessoryKind>,
    /// The part it comes with.
    pub with: Id<Part>,
}

/// `strip_bracket(strip)`: the end bracket of a strip (doc 12 §3.4), first match wins: the strip's choice of the
/// bracket its terminals declare (a choice without a part: no bracket), an end bracket it holds as a part use (a
/// project made before accessories), the `rail_group` accessory of the first of its terminals (in strip order) whose
/// part names one. `None` for a strip without one, which then has no brackets and declares no width to its rail, and for
/// a device that is not a strip.
pub fn strip_bracket(q: &Q, strip: &Device) -> Option<StripBracket> {
    strip.aspects.strip.as_ref()?;
    let parts = q.table::<LibCopy<Part>>();
    let width = |p: Id<Part>| {
        parts
            .get(p)
            .and_then(|c| c.item.mech.as_ref())
            .and_then(|m| m.width)
            .map_or(0, |w| i128::from(w.0).max(0))
    };
    let declared = strip_terminals(q, strip.id).into_iter().find_map(|t| {
        t.parts.iter().find_map(|u| {
            let copy = parts.get(u.part.copy)?;
            let a = copy
                .item
                .accessories
                .iter()
                .find(|a| a.per == AccessoryPer::RailGroup)?;
            Some((a.clone(), u.part.copy))
        })
    });
    if let Some((a, with)) = &declared {
        let of = copy_of(a);
        if let Some(choice) = strip.accessory_choices.iter().find(|c| c.of == of) {
            let part = choice.part.clone()?;
            parts.get(part.copy)?;
            return Some(StripBracket {
                width: width(part.copy),
                part,
                from: BracketFrom::Choice,
                declared: Some(DeclaredBracket {
                    of,
                    qty: a.qty,
                    kind: a.kind.clone(),
                    with: *with,
                }),
            });
        }
    }
    let older = strip.parts.iter().position(|u| {
        parts
            .get(u.part.copy)
            .is_some_and(|c| c.item.kind == PartKind::EndBracket)
    });
    if let Some(i) = older {
        let part = strip.parts[i].part.clone();
        return Some(StripBracket {
            width: width(part.copy),
            part,
            from: BracketFrom::PartUse(i),
            declared: None,
        });
    }
    let (a, with) = declared?;
    if a.optional {
        return None;
    }
    let of = copy_of(&a);
    parts.get(of)?;
    Some(StripBracket {
        part: PartRef::new(of),
        width: width(of),
        from: BracketFrom::Declared,
        declared: Some(DeclaredBracket {
            of,
            qty: a.qty,
            kind: a.kind.clone(),
            with,
        }),
    })
}

/// The end brackets the rails attribute to a strip (`group_accessories`), over every rail its placements are on, with
/// the surfaces of those rails; `None` when no placement of it holds a bracket (not placed, or placed on a surface or a
/// duct).
pub fn placed_brackets(q: &Q, strip: Id<Device>) -> Option<(usize, BTreeSet<Id<MountingSurface>>)> {
    let placements = q.table::<Placement>();
    let carriers: BTreeSet<Id<Carrier>> = q
        .idx()
        .placements_of_device(strip)
        .filter_map(|p| match placements.get(p)?.carrier {
            CarrierRef::Carrier(c) => Some(c),
            _ => None,
        })
        .collect();
    let table = q.table::<Carrier>();
    let mut count = 0usize;
    let mut surfaces = BTreeSet::new();
    for c in carriers {
        let n = q.get::<GroupAccessories>(c).count_of(strip);
        if n > 0 {
            count = count.saturating_add(n);
            if let Some(rec) = table.get(c) {
                surfaces.insert(rec.surface);
            }
        }
    }
    (count > 0).then_some((count, surfaces))
}

/// `part_accessories(device)`: every accessory of a device (see the module documentation), in order: the `unit`
/// accessories of its part uses (`Device.parts` order), those of its enclosures' carriers, then for a strip its `strip`
/// accessories and its end brackets (`rail_group`: the brackets the rails attribute to it, two on no rail, each of the
/// declared quantity; none for an end bracket held as a part use, which its own line counts). Empty for a missing
/// device and a device that is not fitted.
#[derive(Debug)]
pub struct PartAccessories;

impl Query for PartAccessories {
    type Key = Id<Device>;
    type Value = Vec<DerivedAccessory>;
    const FAMILY: &'static str = "part_accessories";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<DerivedAccessory>> {
        let Some(dev) = q.devices().get(*key).cloned() else {
            return Arc::new(Vec::new());
        };
        if dev.state == ew_schema::DeviceState::NotFitted {
            return Arc::new(Vec::new());
        }
        let mut out: Vec<DerivedAccessory> = dev
            .parts
            .iter()
            .flat_map(|u| unit_accessories(q, u.part.copy, u.qty, &dev.accessory_choices))
            .collect();
        out.extend(carrier_accessories(
            q,
            &dev,
            &crate::bom::enclosure_carriers(q, *key),
        ));
        out.extend(strip_accessories(q, &dev));
        if let Some(b) = strip_bracket(q, &dev)
            && let Some(DeclaredBracket { of, qty, kind, with }) = b.declared
        {
            let n = placed_brackets(q, *key).map_or(2, |(n, _)| n);
            let n = Dec3::from_int(i64::try_from(n).unwrap_or(i64::MAX)).unwrap_or(Dec3(i64::MAX));
            out.push(DerivedAccessory {
                part: b.part,
                of,
                qty: qty.checked_mul(n).unwrap_or(Dec3(i64::MAX)),
                with: vec![with],
                per: AccessoryPer::RailGroup,
                kind,
                after: None,
            });
        }
        Arc::new(out)
    }
}
