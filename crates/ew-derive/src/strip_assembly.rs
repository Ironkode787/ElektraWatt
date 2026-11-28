// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `assembly_rows(strip)`: what the assembly list prints of one strip (doc 13 §6.1, FR-MFG-016): where it goes and how
//! long it is, then every item in the order the fitter mounts it from the rail's start.
//!
//! The items are `strip_layout`'s (the terminals by `seq`, the accessories their parts stand after a terminal, the
//! strip's own) with the end brackets the rail gives the strip (`group_accessories`): one before it when it starts a
//! run, one after it. A strip that abuts the one before it starts against that strip's bracket and names it. A strip on
//! no rail counts two brackets, as `part_accessories` does. The items read from the rail's start, the end nearest its
//! surface's datum corner, as the rail's own block counts (`rail_assembly`): a strip turned on its rail, or on a rail
//! measured from its other end, lists its last terminal first. The bridges are the list's own block (`bridge_groups`), not read here.

use ew_ids::Id;
use ew_schema::{Carrier, CarrierKind, CarrierRef, Device, LibCopy, Part, PartRef, Placement};

use crate::Q;
use crate::accessories::{StripBracket, strip_bracket};
use crate::cut_list::cut_to;
use crate::layout::{
    BracketRole, CarrierLabelKind, GroupAccessories, Quarter, RailOccupancy, StripItemKind, StripLayout,
    carrier_label, carrier_length, carrier_order, from_rail_start, places_strip, rail_measured_back,
};

/// What one row of a strip's mounting order is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AssemblyItemKind {
    /// An end bracket of the strip ([`strip_bracket`]).
    EndBracket,
    /// An item of `strip_layout`: a terminal, an accessory its terminals' parts stand there, the strip's own.
    Item(StripItemKind),
}

/// One row of a strip's mounting order ([`StripAssembly::items`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssemblyItem {
    /// What it is.
    pub kind: AssemblyItemKind,
    /// Its part: a terminal's primary part, the accessory's or the bracket's part.
    pub part: Option<PartRef>,
    /// Its width along the strip in nanometres, `mech.width`; `None` when its part has none.
    pub width: Option<i128>,
}

/// Where a strip stands on its rail ([`StripAssembly::on`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OnRail {
    /// The rail.
    pub rail: Id<Carrier>,
    /// The rail's number among its surface's rails (doc 12 §5.4: "Rail 1").
    pub number: u32,
    /// Where the strip starts with its end brackets, in nanometres from the rail's start: the end nearest its
    /// surface's datum corner ([`crate::layout::RailStart`]), as the rail's own block measures.
    pub from: i128,
    /// Where it ends with its end brackets.
    pub to: i128,
    /// The rail's cut length: its path length rounded up to the project's cut increment ([`cut_to`]); `None` for a
    /// rail without a derivable length.
    pub cut: Option<i128>,
    /// The strip is turned on the rail: its first terminal stands at the right.
    pub turned: bool,
    /// The strip it abuts: this one starts against that strip's end bracket and has none of its own before it.
    pub abuts: Option<Id<Device>>,
}

/// What the assembly list prints of one strip (see the module documentation).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StripAssembly {
    /// The strip.
    pub strip: Id<Device>,
    /// Where it stands: its first placement as a whole on a rail (by the rails' legend order); `None` on no rail.
    pub on: Option<OnRail>,
    /// Its terminals.
    pub terminals: usize,
    /// Its length with its end brackets, in nanometres; the items without a width count none.
    pub length: i128,
    /// An item has no width, so the length is at least [`StripAssembly::length`].
    pub width_missing: bool,
    /// Whether the strip has end brackets at all ([`strip_bracket`]).
    pub bracketed: bool,
    /// The items in mounting order from the rail's start.
    pub items: Vec<AssemblyItem>,
}

/// The width of a part, `mech.width`; `None` without one.
fn width_of(q: &Q, part: &PartRef) -> Option<i128> {
    q.table::<LibCopy<Part>>()
        .get(part.copy)
        .and_then(|c| c.item.mech.as_ref())
        .and_then(|m| m.width)
        .map(|w| i128::from(w.0))
}

/// The strip's placement as a whole on a rail that comes first in the rails' legend order, then by placement ID.
fn rail_placement(q: &Q, strip: &Device) -> Option<(Id<Placement>, Id<Carrier>)> {
    let placements = q.table::<Placement>();
    let carriers = q.table::<Carrier>();
    q.idx()
        .placements_of_device(strip.id)
        .filter_map(|p| {
            let pl = placements.get(p)?;
            let CarrierRef::Carrier(c) = pl.carrier else {
                return None;
            };
            let rec = carriers.get(c)?;
            (rec.kind == CarrierKind::Rail && places_strip(q, strip, pl.part.as_ref()))
                .then(|| (carrier_order(c, rec), p, c))
        })
        .min()
        .map(|(_, p, c)| (p, c))
}

/// Where `strip`, placed by `placement` on `rail`, stands with its brackets, and which of its two brackets the rail gives
/// it (before, after).
fn on_rail(
    q: &Q,
    strip: Id<Device>,
    placement: Id<Placement>,
    rail: Id<Carrier>,
) -> Option<(OnRail, bool, bool)> {
    let occupancy = q.get::<RailOccupancy>(rail);
    let o = occupancy.iter().find(|o| o.placement == placement)?;
    let brackets = q.get::<GroupAccessories>(rail);
    let own: Vec<_> = brackets.iter().filter(|b| b.placement == placement).collect();
    let before = own.iter().find(|b| b.role == BracketRole::RunStart);
    let after = own.iter().find(|b| b.role == BracketRole::After);
    // Without a bracket of its own before it, a strip in a run starts against the bracket after the strip before it.
    let abuts = if before.is_none() && after.is_some() {
        brackets
            .iter()
            .filter(|b| b.role == BracketRole::After && b.strip != strip && b.s0 <= o.s0)
            .max_by_key(|b| (b.s0, b.s1))
            .map(|b| b.strip)
    } else {
        None
    };
    let flipped = q
        .table::<Placement>()
        .get(placement)
        .is_some_and(|p| Quarter::of(p.pose.q) == Some(Quarter::HALF));
    let number = carrier_label(q, rail)
        .filter(|l| l.kind == CarrierLabelKind::Rail)
        .map_or(0, |l| l.n);
    let (from, to) = from_rail_start(
        q,
        rail,
        (
            before.map_or(o.s0, |b| b.s0.min(o.s0)),
            after.map_or(o.s1, |b| b.s1.max(o.s1)),
        ),
    );
    Some((
        OnRail {
            rail,
            number,
            from,
            to,
            cut: carrier_length(q, rail).map(|l| cut_to(q, l)),
            turned: flipped,
            abuts,
        },
        before.is_some(),
        after.is_some(),
    ))
}

/// `assembly_rows(strip)`: what the assembly list prints of `strip` (see the module documentation); `None` for a
/// missing device and a device that is not a strip.
pub fn assembly_rows(q: &Q, strip: Id<Device>) -> Option<StripAssembly> {
    let dev = q.devices().get(strip).cloned()?;
    dev.aspects.strip.as_ref()?;
    let bracket: Option<StripBracket> = strip_bracket(q, &dev);
    let placed = rail_placement(q, &dev).and_then(|(p, c)| on_rail(q, strip, p, c));
    let (bracket_before, bracket_after) = match &placed {
        Some((_, before, after)) => (*before, *after),
        None => (bracket.is_some(), bracket.is_some()),
    };
    let bracket_item = || AssemblyItem {
        kind: AssemblyItemKind::EndBracket,
        part: bracket.as_ref().map(|b| b.part.clone()),
        width: bracket.as_ref().and_then(|b| width_of(q, &b.part)),
    };
    let mut middle: Vec<AssemblyItem> = q
        .get::<StripLayout>(strip)
        .iter()
        .map(|item| AssemblyItem {
            kind: AssemblyItemKind::Item(item.kind.clone()),
            part: item.part.clone(),
            width: item.width,
        })
        .collect();
    let turned = placed.as_ref().is_some_and(|(on, _, _)| on.turned);
    if turned {
        middle.reverse();
    }
    let mut items = Vec::new();
    if bracket_before {
        items.push(bracket_item());
    }
    items.extend(middle);
    if bracket_after {
        items.push(bracket_item());
    }
    // Along the rail's path so far; from its other end where the rail is measured from there.
    if placed
        .as_ref()
        .is_some_and(|(on, _, _)| rail_measured_back(q, on.rail))
    {
        items.reverse();
    }
    let terminals = items
        .iter()
        .filter(|i| matches!(i.kind, AssemblyItemKind::Item(StripItemKind::Terminal(_))))
        .count();
    let width_missing = items.iter().any(|i| i.width.is_none());
    let summed: i128 = items.iter().filter_map(|i| i.width).fold(0, i128::saturating_add);
    let on = placed.map(|(on, _, _)| on);
    let length = on.as_ref().map_or(summed, |on| on.to.saturating_sub(on.from));
    Some(StripAssembly {
        strip,
        on,
        terminals,
        length,
        width_missing,
        bracketed: bracket.is_some(),
        items,
    })
}
