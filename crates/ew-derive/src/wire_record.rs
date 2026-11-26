// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `wire_record_set()`: every physical wire of the project once, as the workshop reads it (doc 13 §2.4).
//!
//! The wire list, the terminal diagrams, the cable and potential overviews, the SQL view and the checks read the
//! project's wires through this one query: each physical wire of [`PhysicalWiresOf`] once (a bond wire belongs to two
//! potentials), oriented From/To by [`orient_wire`] (doc 08 §3.8), with its effective cross-section, colour and wire
//! part ([`WirePropsOf`], doc 08 §4) and its printed number ([`WireNumbers`]), in the numbering order of doc 08 §5.2:
//! the order of `wire_numbers`, a wire it does not list last, by ID.

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{ColourSpec, Connection, Csa, PartRef};

use crate::numbering::{WireNumbers, orient_wire};
use crate::potentials::PotentialPartition;
use crate::q::Q;
use crate::query::{Policy, Query};
use crate::wire_props::{Prov, WirePropsOf};
use crate::wires::{PhysicalWire, PhysicalWiresOf};

/// One physical wire as the workshop reads it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WireRecord {
    /// The wire, oriented: `ends[0]` is the From end.
    pub wire: PhysicalWire,
    /// The effective cross-section with its source.
    pub csa: Option<Prov<Csa>>,
    /// The effective colour with its source.
    pub colour: Option<Prov<ColourSpec>>,
    /// The wire part the inheritance chain names.
    pub wire_part: Option<PartRef>,
    /// The printed number (`101.1`); `None` for a cable core and a wire drawn on no page.
    pub number: Option<String>,
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
        let mut by_id: BTreeMap<Id<Connection>, WireRecord> = BTreeMap::new();
        for p in partition.potentials() {
            let physical = q.get::<PhysicalWiresOf>(p.id);
            let props = q.get::<WirePropsOf>(p.id);
            for w in &physical.wires {
                by_id.entry(w.id).or_insert_with(|| {
                    let mut wire = w.clone();
                    orient_wire(q, &mut wire);
                    let props = props.wires.get(&w.id).cloned().unwrap_or_default();
                    WireRecord {
                        wire,
                        csa: props.csa,
                        colour: props.colour,
                        wire_part: props.wire_part.map(|p| p.value),
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
        Arc::new(keyed.into_iter().map(|(_, r)| r).collect())
    }
}
