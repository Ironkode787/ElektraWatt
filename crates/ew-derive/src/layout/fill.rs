// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `duct_fill(duct)`: how full a duct is with the ways stored through it (doc 12 §8.6), and the two tables it reads:
//! a wire's outer diameter by its cross-section ([`WIRE_OD`]) and the share of a duct's nominal size that is its inner
//! area where its part gives none ([`INNER_SHARE`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{Carrier, CommittedRoute, Connection, Csa, WayStep};
use ew_units::{Dec3, Nm};

use super::ways::{DuctLine, fill_limit, stored_way};
use crate::query::{Policy, Query};
use crate::wire_record::{WireRecord, WireRecordSet};
use crate::{Q, wire_props::Prov};

/// A wire's outer diameter by its cross-section in mm², where its wire part gives none (doc 12 §8.6; PM §6.7: H05V-K to
/// 1 mm², H07V-K above). Data, not verified: a fill that read it says so.
pub const WIRE_OD: [(Dec3, Nm); 7] = [
    (Dec3(500), Nm(2_500_000)),
    (Dec3(750), Nm(2_700_000)),
    (Dec3(1_000), Nm(2_900_000)),
    (Dec3(1_500), Nm(3_300_000)),
    (Dec3(2_500), Nm(3_900_000)),
    (Dec3(4_000), Nm(4_600_000)),
    (Dec3(6_000), Nm(5_300_000)),
];

/// The share of a duct's nominal width × height that is its inner area where its part gives no inner size: 85/100
/// (PM §5.3; unverified).
pub const INNER_SHARE: (i128, i128) = (85, 100);

/// π/4 in 10⁻⁹, for a wire's area `π/4 · OD²` in whole square nanometres.
const QUARTER_PI_E9: i128 = 785_398_163;

/// Where a duct's inner area comes from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InnerFrom {
    /// Its part's inner width × inner height (`sections.duct`).
    Part,
    /// [`INNER_SHARE`] of its nominal width × height: its part gives no inner size.
    Share,
}

/// A duct's inner area in square nanometres, and where it comes from.
pub fn inner_area(q: &Q, carrier: Id<Carrier>) -> (i128, InnerFrom) {
    let Some(c) = q.table::<Carrier>().get(carrier) else {
        return (0, InnerFrom::Share);
    };
    let inner = c
        .part
        .as_ref()
        .and_then(|p| q.parts().get(p.copy))
        .and_then(|p| p.item.sections.duct.as_ref())
        .and_then(|d| Some((d.inner_width?, d.inner_height?)));
    if let Some((w, h)) = inner {
        return (i128::from(w.0) * i128::from(h.0), InnerFrom::Part);
    }
    match &c.params {
        ew_schema::CarrierParams::Duct(d) => (
            i128::from(d.width.0) * i128::from(d.height.0) * INNER_SHARE.0 / INNER_SHARE.1,
            InnerFrom::Share,
        ),
        _ => (0, InnerFrom::Share),
    }
}

/// A wire's outer diameter and whether it is the table's default: its wire part's `sections.wire.outer_diameter`, else
/// [`WIRE_OD`] by its cross-section; `None` without either.
pub fn wire_od(q: &Q, r: &WireRecord) -> Option<(Nm, bool)> {
    let own = r
        .wire_part
        .as_ref()
        .and_then(|p| q.parts().get(p.copy))
        .and_then(|p| p.item.sections.wire.as_ref())
        .and_then(|w| w.outer_diameter)
        .filter(|d| d.0 > 0);
    if let Some(d) = own {
        return Some((d, false));
    }
    let Some(Prov {
        value: Csa::Mm2(csa), ..
    }) = &r.csa
    else {
        return None;
    };
    WIRE_OD.iter().find(|(c, _)| c == csa).map(|(_, d)| (*d, true))
}

/// A wire's area `π/4 · OD²` in square nanometres and whether its diameter is the table's default ([`wire_od`]).
pub fn wire_area(q: &Q, r: &WireRecord) -> Option<(i128, bool)> {
    let (d, default) = wire_od(q, r)?;
    let d = i128::from(d.0);
    // π/4 · d² taken by the billions of d² and the rest: d² of any `i64` fits an `i128`, d² · 785,398,163 does not
    // (a diameter of 466 m, within the format's ±(2^53 − 1) nm, overflowed it).
    let (whole, rest) = (d * d / 1_000_000_000, d * d % 1_000_000_000);
    Some((whole * QUARTER_PI_E9 + rest * QUARTER_PI_E9 / 1_000_000_000, default))
}

/// One piece of a duct between two points where ways enter or leave it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FillPiece {
    /// Where it starts along the duct's path.
    pub from: i128,
    /// Where it ends.
    pub to: i128,
    /// The area of the wires along it, in square nanometres.
    pub area: i128,
    /// How many wires run along it.
    pub wires: usize,
}

/// The value of [`DuctFill`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DuctFillV {
    /// The inner area in square nanometres and where it comes from.
    pub inner: (i128, InnerFrom),
    /// The fill limit in force, in percent.
    pub limit: u8,
    /// The pieces along the duct, in order.
    pub pieces: Vec<FillPiece>,
    /// The fullest piece (the first of equals); `None` for a duct without pieces.
    pub fullest: Option<usize>,
    /// The wires whose stored way runs along the duct, in ID order.
    pub wires: Vec<Id<Connection>>,
    /// Whether a wire's diameter is the table's default (unverified).
    pub default_diameter: bool,
    /// The wires without a diameter, not counted.
    pub without_diameter: usize,
}

impl DuctFillV {
    /// The fullest piece's area over the inner area, as `(area, inner)`.
    pub fn fill(&self) -> (i128, i128) {
        let area = self.fullest.map_or(0, |i| self.pieces[i].area);
        (area, self.inner.0)
    }

    /// Whether the fullest piece is over the limit.
    pub fn over(&self) -> bool {
        let (area, inner) = self.fill();
        area * 100 > i128::from(self.limit) * inner
    }
}

/// `duct_fill(duct)`: per piece of the duct between the points where stored ways enter or leave it, the area of the
/// wires along it, and the fullest piece (doc 12 §8.6). A way whose wire is no longer a wire of the project is not read.
/// `None` for a carrier that is no duct.
#[derive(Debug)]
pub struct DuctFill;

impl Query for DuctFill {
    type Key = Id<Carrier>;
    type Value = Option<DuctFillV>;
    const FAMILY: &'static str = "duct_fill";
    const POLICY: Policy = Policy::Cold;

    fn run(q: &Q, key: &Self::Key) -> Arc<Option<DuctFillV>> {
        let Some(line) = DuctLine::of(q, *key) else {
            return Arc::new(None);
        };
        let records = q.get::<WireRecordSet>(());
        let by_id: BTreeMap<Id<Connection>, &WireRecord> = records.iter().map(|r| (r.wire.id, r)).collect();
        let mut spans: Vec<(i128, i128, i128)> = Vec::new();
        let mut wires = Vec::new();
        let (mut default_diameter, mut without) = (false, 0);
        for (_, route) in q.table::<CommittedRoute>().iter() {
            let Some(r) = by_id.get(&route.wire) else { continue };
            if stored_way(q, &r.wire).is_none_or(|w| w.id != route.id) {
                continue;
            }
            let along: Vec<(i128, i128)> = route
                .way
                .iter()
                .filter_map(|s| match s {
                    WayStep::Duct(d) if d.carrier == *key => {
                        let (a, b) = (i128::from(d.from.0), i128::from(d.to.0));
                        Some((a.min(b), a.max(b)))
                    }
                    _ => None,
                })
                .collect();
            if along.is_empty() {
                continue;
            }
            wires.push(route.wire);
            match wire_area(q, r) {
                Some((area, default)) => {
                    default_diameter |= default;
                    spans.extend(along.into_iter().map(|(a, b)| (a, b, area)));
                }
                None => without += 1,
            }
        }
        wires.sort();
        let mut cuts: Vec<i128> = vec![0, line.len];
        cuts.extend(spans.iter().flat_map(|s| [s.0, s.1]));
        cuts.retain(|x| (0..=line.len).contains(x));
        cuts.sort_unstable();
        cuts.dedup();
        let pieces: Vec<FillPiece> = cuts
            .windows(2)
            .map(|w| {
                let inside: Vec<&(i128, i128, i128)> =
                    spans.iter().filter(|s| s.0 <= w[0] && w[1] <= s.1).collect();
                FillPiece {
                    from: w[0],
                    to: w[1],
                    area: inside.iter().map(|s| s.2).sum(),
                    wires: inside.len(),
                }
            })
            .collect();
        let fullest = pieces
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.area.cmp(&b.1.area).then(b.0.cmp(&a.0)))
            .map(|(i, _)| i);
        Arc::new(Some(DuctFillV {
            inner: inner_area(q, *key),
            limit: fill_limit(q),
            pieces,
            fullest,
            wires,
            default_diameter,
            without_diameter: without,
        }))
    }
}
