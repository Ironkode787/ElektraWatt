// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `wire_record_set()`: every physical wire of the project once, as the workshop reads it (doc 13 §2.4).
//!
//! The wire list, the terminal diagrams, the cable and potential overviews, the SQL view and the checks read the
//! project's wires through this one query: each physical wire of [`PhysicalWiresOf`] once (a bond wire belongs to two
//! potentials), oriented From/To by [`orient_wire`] (doc 08 §3.8), with its effective cross-section, colour and wire
//! part ([`WirePropsOf`], doc 08 §4), its printed number ([`WireNumbers`]), its length ([`wire_length`], doc 13 §3.3)
//! and which of its ends the shop does not make (a site end, doc 13 §2.1), and the treatment and part of each end
//! ([`crate::treatment`], doc 13 §4), in the numbering order of doc 08 §5.2: the order of `wire_numbers`, a wire it does
//! not list last, by ID.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ew_ids::Id;
use ew_rds::Aspect;
use ew_schema::{ColourSpec, Connection, Csa, Device, LayoutSpace, Part, PartRef, StructureNode};
use ew_units::Nm;

use crate::marking::{MarkedBy, MarkerText, NotMarked, mark};
use crate::numbering::{WireNumbers, orient_wire};
use crate::potentials::PotentialPartition;
use crate::q::Q;
use crate::query::{Policy, Query};
use crate::structure::StructureOf;
use crate::treatment::{EndTreatment, TreatedBy, treat};
use crate::wire_props::{Prov, WirePropsOf, wire_part_of};
use crate::wires::{EndKind, PhysicalWire, PhysicalWiresOf, WireEnd};

/// One physical wire as the workshop reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WireRecord {
    /// The wire, oriented: `ends[0]` is the From end.
    pub wire: PhysicalWire,
    /// The effective cross-section with its source.
    pub csa: Option<Prov<Csa>>,
    /// The effective colour with its source.
    pub colour: Option<Prov<ColourSpec>>,
    /// The wire part the wire is cut from: the one its inheritance chain names, else the workshop stock's
    /// ([`wire_part_of`]).
    pub wire_part: Option<PartRef>,
    /// The printed number (`101.1`); `None` for a cable core and a wire drawn on no page.
    pub number: Option<String>,
    /// Its length (doc 13 §3.3).
    pub length: WireLength,
    /// What the shop reads of the wire as a whole.
    pub flags: WireRecordFlags,
    /// Its ends: From, To.
    pub ends: [EndRecord; 2],
}

/// A wire's length (doc 13 §3.3). The layout adds the lengths it estimates and routes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireLength {
    /// Typed by hand: a constituent connection's `WireOverrides.length`, the carrier's first.
    Typed(Nm),
    /// A cable's core: cut with its cable.
    WithCable(Id<Device>),
    /// Neither.
    Missing,
}

impl WireLength {
    /// The length in nanometres, when there is one.
    pub fn value(self) -> Option<Nm> {
        match self {
            WireLength::Typed(n) => Some(n),
            WireLength::WithCable(_) | WireLength::Missing => None,
        }
    }
}

/// What the shop reads of a wire as a whole.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct WireRecordFlags {
    /// A field wire: an end of it is a site end ([`EndRecord::site`]).
    pub field: bool,
}

/// One end of a wire as the shop reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EndRecord {
    /// What the fitter puts on it (doc 13 §4).
    pub treatment: EndTreatment,
    /// The ferrule's or the ring lug's part from the workshop stock (doc 13 §4.4); `None` where the stock holds none.
    pub part: Option<Id<Part>>,
    /// A site end, made on site, not in the shop (doc 13 §2.1): an external end, or an end on a device whose location is
    /// the location of no layout space while the project has a layout space.
    pub site: bool,
    /// Who decided the treatment.
    pub treated_by: TreatedBy,
    /// Whether the part was set by hand.
    pub part_by_hand: bool,
    /// Where the end is set by hand (doc 13 §2.3): the wire's first connection that no other wire stands for and whether
    /// in its `end_a` (the end reached through its endpoint `a`) or `end_b`; `None` for an end made outside the project
    /// and for a wire without a connection of its own.
    pub hand_at: Option<(Id<Connection>, bool)>,
    /// What its marker says (doc 13 §5.1), shortened to the wire markers' part (§5.2).
    pub marker: MarkerText,
    /// Who decided the marker.
    pub marked_by: MarkedBy,
    /// Whether the marker's text fits its marker part (EW-MFG-004 where not).
    pub marker_fits: bool,
}

/// The length of a wire (doc 13 §3.3): typed, else a core's cable, else missing.
pub fn wire_length(q: &Q, w: &PhysicalWire) -> WireLength {
    if let Some(n) = w.overrides.length {
        return WireLength::Typed(n);
    }
    match w.core.and_then(|c| q.conductors().get(c)) {
        Some(core) => WireLength::WithCable(core.cable),
        None => WireLength::Missing,
    }
}

/// Whether `end` is a site end (doc 13 §2.1), given the locations of the project's layout spaces (`spaces`, empty in a
/// project without one): an external end; an end on a device whose own location is none of them.
fn site_end(q: &Q, end: &WireEnd, spaces: &BTreeSet<Id<StructureNode>>) -> bool {
    match end.kind {
        EndKind::External => true,
        EndKind::Open => false,
        EndKind::Device => {
            !spaces.is_empty()
                && q.functions()
                    .get(end.cp.function)
                    .and_then(|f| f.device)
                    .and_then(|d| q.try_get::<StructureOf>(d).ok())
                    .and_then(|s| s.node(Aspect::Location))
                    .is_some_and(|node| !spaces.contains(&node))
        }
    }
}

/// `wire_record_set()`: see the module documentation. Policy Cold: the families it reads (the partition, the
/// physical wires, the wire properties and numbers) are memoised themselves.
#[derive(Debug)]
pub struct WireRecordSet;

impl Query for WireRecordSet {
    type Key = ();
    type Value = Vec<WireRecord>;
    const FAMILY: &'static str = "wire_record_set";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, _key: &()) -> Arc<Vec<WireRecord>> {
        let partition = q.get::<PotentialPartition>(());
        let numbers = q.get::<WireNumbers>(());
        let spaces: BTreeSet<Id<StructureNode>> =
            q.table::<LayoutSpace>().iter().map(|(_, s)| s.location).collect();
        let mut by_id: BTreeMap<Id<Connection>, WireRecord> = BTreeMap::new();
        for p in partition.potentials() {
            let physical = q.get::<PhysicalWiresOf>(p.id);
            let props = q.get::<WirePropsOf>(p.id);
            for w in &physical.wires {
                by_id.entry(w.id).or_insert_with(|| {
                    let mut wire = w.clone();
                    orient_wire(q, &mut wire);
                    let props = props.wires.get(&w.id).cloned().unwrap_or_default();
                    let wire_part = wire_part_of(q, &props);
                    let ends = wire.ends.map(|e| EndRecord {
                        treatment: EndTreatment::External,
                        part: None,
                        site: site_end(q, &e, &spaces),
                        treated_by: TreatedBy::Outside,
                        part_by_hand: false,
                        hand_at: None,
                        marker: MarkerText::NotMarked(NotMarked::Outside),
                        marked_by: MarkedBy::Setting,
                        marker_fits: true,
                    });
                    WireRecord {
                        length: wire_length(q, &wire),
                        flags: WireRecordFlags {
                            field: ends.iter().any(|e| e.site),
                        },
                        ends,
                        wire,
                        csa: props.csa,
                        colour: props.colour,
                        wire_part,
                        number: numbers.number(w.id).map(|n| n.label.clone()),
                    }
                });
            }
        }
        let mut keyed: Vec<((usize, Id<Connection>), WireRecord)> = by_id
            .into_values()
            .map(|r| ((numbers.position(r.wire.id).unwrap_or(usize::MAX), r.wire.id), r))
            .collect();
        keyed.sort_by_key(|e| e.0);
        let mut records: Vec<WireRecord> = keyed.into_iter().map(|(_, r)| r).collect();
        treat(q, &mut records);
        mark(q, &mut records);
        Arc::new(records)
    }
}
