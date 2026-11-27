// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `consumable_rows()`: what to order of what the wires consume (doc 13 §3.6, FR-MFG-049), beside the parts list. It
//! reads, of the wire design and nothing else, each wire record (`wire_record_set`: its size, colour, wire part, length
//! and whether it is a field wire), each end the shop makes (its treatment and that treatment's part) and each label
//! (`label_rows`: its set, marker part and copies).
//!
//! - **Wire**: one row per wire part, else size and colour; the wires and their summed lengths, the wires without a
//!   length counted apart. A cable's core is cut with its cable and a field wire on site: neither adds to the metres.
//! - **Ferrules and lugs**: one row per ferrule or lug part, the ends it is crimped on, a twin ferrule once for its two
//!   wires (the wire whose ID is the lower counts it); an end the treatment gives a ferrule or a lug and the stock no
//!   part counts under its size and form without one. A site end is not the shop's.
//! - **Markers**: one row per label set and marker part, the labels it prints (their copies).
//!
//! Nothing is rounded: a wire's length is the wire list's, and an order quantity belongs to the order list.

use ew_ids::Id;
use ew_mltext::natural_key;
use ew_schema::{ColourSpec, Csa, Part, PartRef};

use crate::Q;
use crate::labels::{LabelRows, LabelSet};
use crate::part_text::printed_number;
use crate::treatment::EndTreatment;
use crate::wire_record::{WireLength, WireRecordSet};

/// One row of the Wire group.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WireUse {
    /// The wire part; `None`: the wires are listed by size and colour.
    pub part: Option<PartRef>,
    /// The size (of a row without a part).
    pub csa: Option<Csa>,
    /// The colour (of a row without a part).
    pub colour: Option<ColourSpec>,
    /// The wires.
    pub wires: usize,
    /// The summed lengths of those with a length, in nanometres.
    pub length: i128,
    /// The wires without a length.
    pub without_length: usize,
    /// Whether a summed length is along a way that no longer holds (doc 12 §8.5): the sum prints with "≈".
    pub stale: bool,
}

/// What an end is given ([`EndUse::form`]).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum EndForm {
    /// A ferrule.
    Ferrule,
    /// A twin ferrule, for two wires.
    TwinFerrule,
    /// A ring lug.
    RingLug,
}

/// One row of the Ferrules and lugs group.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EndUse {
    /// The ferrule's or lug's part; `None`: the stock has none for these ends.
    pub part: Option<Id<Part>>,
    /// What the ends are given.
    pub form: EndForm,
    /// The sizes of the wires it is crimped on, each once, in the order met.
    pub sizes: Vec<Csa>,
    /// How many are crimped (a twin ferrule once).
    pub count: usize,
}

/// One row of the Markers group.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MarkerUse {
    /// The label set.
    pub set: LabelSet,
    /// The marker part; `None`: the stock has none that marks the set.
    pub part: Option<Id<Part>>,
    /// The labels printed on it (their copies).
    pub labels: usize,
}

/// What the wires consume, by group ([`consumable_rows`]).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Consumables {
    /// The Wire group, by size, colour, then part number.
    pub wires: Vec<WireUse>,
    /// The Ferrules and lugs group: the parts by number, then the ends without a part by form and size.
    pub ends: Vec<EndUse>,
    /// The Markers group, by label set, then in the order of the first label.
    pub markers: Vec<MarkerUse>,
}

/// A cross-section's square millimetres, for the order of the rows; `None` for another unit.
fn mm2(c: &Csa) -> Option<ew_units::Dec3> {
    match c {
        Csa::Mm2(d) => Some(*d),
        _ => None,
    }
}

/// `consumable_rows()`: what the wires consume (see the module documentation).
pub fn consumable_rows(q: &Q) -> Consumables {
    let records = q.get::<WireRecordSet>(());
    let mut out = Consumables::default();
    for r in records.iter() {
        let csa = r.csa.as_ref().map(|c| c.value.clone());
        if !r.flags.field && !matches!(r.length, WireLength::WithCable(_)) {
            let (part, size, colour) = match &r.wire_part {
                Some(p) => (Some(p.clone()), None, None),
                None => (None, csa.clone(), r.colour.as_ref().map(|c| c.value.clone())),
            };
            let row = match out
                .wires
                .iter_mut()
                .find(|w| w.part == part && w.csa == size && w.colour == colour)
            {
                Some(w) => w,
                None => {
                    out.wires.push(WireUse {
                        part,
                        csa: size,
                        colour,
                        wires: 0,
                        length: 0,
                        without_length: 0,
                        stale: false,
                    });
                    out.wires.last_mut().expect("just pushed")
                }
            };
            row.wires += 1;
            row.stale |= !r.length.exact();
            match r.length.value() {
                Some(n) => row.length = row.length.saturating_add(i128::from(n.0)),
                None => row.without_length += 1,
            }
        }
        for end in r.ends.iter().filter(|e| !e.site) {
            let form = match &end.treatment {
                EndTreatment::Ferrule { .. } => EndForm::Ferrule,
                EndTreatment::TwinFerrule { partner, .. } => {
                    // A twin ferrule is crimped once on its two wires: the wire with the lower ID counts it.
                    if partner.is_some_and(|p| p < r.wire.id) {
                        continue;
                    }
                    EndForm::TwinFerrule
                }
                EndTreatment::RingLug => EndForm::RingLug,
                _ => continue,
            };
            let row = match out.ends.iter_mut().find(|e| {
                e.part == end.part
                    && (end.part.is_some() || (e.form == form && e.sizes.first() == csa.as_ref()))
            }) {
                Some(e) => e,
                None => {
                    out.ends.push(EndUse {
                        part: end.part,
                        form,
                        sizes: Vec::new(),
                        count: 0,
                    });
                    out.ends.last_mut().expect("just pushed")
                }
            };
            row.count += 1;
            if let Some(c) = &csa
                && !row.sizes.contains(c)
            {
                row.sizes.push(c.clone());
            }
        }
    }
    out.wires.sort_by_cached_key(|w| {
        crate::cut_list::wire_order(q, w.part.as_ref(), w.csa.as_ref(), w.colour.as_ref())
    });
    out.ends.sort_by_cached_key(|e| {
        (
            e.part.is_none(),
            e.part.map(|p| natural_key(&printed_number(q, &PartRef::new(p)))),
            e.form,
            e.sizes.first().and_then(mm2),
        )
    });
    for l in q.get::<LabelRows>(()).iter() {
        match out
            .markers
            .iter_mut()
            .find(|m| m.set == l.set && m.part == l.marker_part)
        {
            Some(m) => m.labels += usize::from(l.copies),
            None => out.markers.push(MarkerUse {
                set: l.set,
                part: l.marker_part,
                labels: usize::from(l.copies),
            }),
        }
    }
    out.markers.sort_by_key(|m| m.set);
    out
}
