// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The holes of a surface (doc 12-3d §10): the sizes the named holes are made at ([`HOLE_SIZES`]), how a hole is made
//! ([`made`]), a rail's and a duct's fixing holes ([`carrier_fixings`]) and [`SurfaceFeatures`], every hole of a
//! surface with its owners, numbered in drilling order. Nothing of it is stored but the holes a person added
//! (`SurfaceFeature`) and the holes he moved or left out (`hole_choices`).

use std::sync::Arc;

use ew_ids::Id;
use ew_schema::{
    Carrier, CarrierFixing, CarrierKind, FixingRule, Hole, HoleChoice, MountingSurface, Placement, Screw,
    SurfaceFeature, SurfaceKind, WorkshopSettings,
};
use ew_units::{Nm, Pt2};

use super::queries::PlacementGeometry;
use crate::Q;
use crate::query::{Policy, Query};

/// Which holes a row of [`HOLE_SIZES`] sizes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SizeFamily {
    /// A named cut-out (16, 22, 30 mm), punched.
    Cutout,
    /// A gland's clearance hole (M12 … M63).
    Gland,
    /// A tapped fixing's tap drill.
    Tap,
    /// A fixing's clearance hole for a screw through the sheet.
    Through,
    /// A self-tapping screw's pilot hole.
    Pilot,
}

/// One row of the table of the named holes' sizes (doc 12-3d §10.2): each nominal size in millimetres with the
/// diameter it is made at in micrometres, where the numbers come from and whether they were checked against it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HoleSizes {
    /// The holes it sizes.
    pub family: SizeFamily,
    /// (nominal size in mm, made diameter in µm).
    pub sizes: &'static [(u32, i64)],
    /// The source of the numbers.
    pub source: &'static str,
    /// Whether the numbers were checked against the source.
    pub verified: bool,
}

/// The sizes the named holes are made at (doc 12-3d §10.2): data of the program, none checked against its standard
/// yet; a shop whose punch differs types a round hole of its own size.
pub const HOLE_SIZES: [HoleSizes; 5] = [
    HoleSizes {
        family: SizeFamily::Cutout,
        sizes: &[(16, 16_200), (22, 22_300), (30, 30_500)],
        source: "the mounting holes of control-circuit devices, IEC 60947-5-1",
        verified: false,
    },
    HoleSizes {
        family: SizeFamily::Gland,
        sizes: &[
            (12, 12_500),
            (16, 16_500),
            (20, 20_500),
            (25, 25_500),
            (32, 32_500),
            (40, 40_500),
            (50, 50_500),
            (63, 63_500),
        ],
        source: "the clearance hole of a metric cable gland, the thread and 0.5 mm (EN 62444; ISO 261 threads)",
        verified: false,
    },
    HoleSizes {
        family: SizeFamily::Tap,
        sizes: &[(3, 2_500), (4, 3_300), (5, 4_200), (6, 5_000), (8, 6_800)],
        source: "the tap drill of the coarse thread (ISO 261 pitch, DIN 336)",
        verified: false,
    },
    HoleSizes {
        family: SizeFamily::Through,
        sizes: &[(3, 3_400), (4, 4_500), (5, 5_500), (6, 6_600), (8, 9_000)],
        source: "ISO 273, medium clearance",
        verified: false,
    },
    HoleSizes {
        family: SizeFamily::Pilot,
        sizes: &[(3, 2_700), (4, 3_700), (5, 4_600), (6, 5_500), (8, 7_400)],
        source: "a thread-forming screw's pilot hole in sheet steel, about 0.9 × the thread",
        verified: false,
    },
];

/// The diameter `family` makes the nominal size `mm` at; `None` for a size the table has not.
pub fn hole_size(family: SizeFamily, mm: u32) -> Option<Nm> {
    HOLE_SIZES
        .iter()
        .find(|r| r.family == family)?
        .sizes
        .iter()
        .find(|(n, _)| *n == mm)
        .map(|(_, um)| Nm(um * 1_000))
}

/// How a hole is made (doc 12-3d §10.2, §10.3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Made {
    /// Drilled at `d`: a round hole, a fixing through the sheet or a self-tapping screw's pilot.
    Drill {
        /// The drill.
        d: Nm,
    },
    /// Drilled at the tap drill and tapped.
    Tap {
        /// The tap drill.
        drill: Nm,
        /// The thread.
        screw: Screw,
    },
    /// Punched at `d`: a named cut-out or a gland's hole.
    Punch {
        /// The punch.
        d: Nm,
    },
    /// Cut as a rectangle.
    Cut {
        /// Width.
        w: Nm,
        /// Height.
        h: Nm,
        /// Corner radius.
        r: Option<Nm>,
    },
}

impl Made {
    /// The order of the tools (doc 12-3d §10.4, rule 6): the drills, the smallest first (a tapped hole by its tap
    /// drill, after a plain drill of that size), then the punches by size, then the rectangles by size.
    fn tool(&self) -> (u8, i64, i64, i64) {
        match self {
            Made::Drill { d } => (0, d.0, 0, 0),
            Made::Tap { drill, .. } => (0, drill.0, 1, 0),
            Made::Punch { d } => (1, d.0, 0, 0),
            Made::Cut { w, h, r } => (2, w.0, h.0, r.map_or(0, |r| r.0)),
        }
    }

    /// The hole's extent across, for the gaps of EW-LAY-012: the diameter, a rectangle's width and height.
    pub fn extent(&self) -> (Nm, Nm) {
        match self {
            Made::Drill { d } | Made::Punch { d } | Made::Tap { drill: d, .. } => (*d, *d),
            Made::Cut { w, h, .. } => (*w, *h),
        }
    }
}

/// How the hole `hole` of a surface of kind `kind` is made under the workshop's fixing rule `rule` (doc 12-3d §10.3):
/// a fixing on the mounting plate as the rule says (tapped by default), in a door, a wall, the top or the bottom
/// through the sheet; a cut-out and a gland punched at the table's size; `None` for a size or a hole of a newer format
/// the table has not.
pub fn made(hole: &Hole, kind: &SurfaceKind, rule: &FixingRule) -> Option<Made> {
    Some(match hole {
        Hole::Round { d } => Made::Drill { d: *d },
        Hole::Fixing(s) => {
            let mm = s.mm()?;
            let on_plate = matches!(kind, SurfaceKind::Plate | SurfaceKind::Unknown(_));
            match (on_plate, rule) {
                (true, FixingRule::SelfTapping) => Made::Drill {
                    d: hole_size(SizeFamily::Pilot, mm)?,
                },
                (true, FixingRule::Through) | (false, _) => Made::Drill {
                    d: hole_size(SizeFamily::Through, mm)?,
                },
                (true, FixingRule::Tapped | FixingRule::Unknown(_)) => Made::Tap {
                    drill: hole_size(SizeFamily::Tap, mm)?,
                    screw: s.clone(),
                },
            }
        }
        Hole::Cutout(c) => Made::Punch {
            d: hole_size(SizeFamily::Cutout, c.mm()?)?,
        },
        Hole::Gland(g) => Made::Punch {
            d: hole_size(SizeFamily::Gland, g.mm()?)?,
        },
        Hole::Rect { w, h, r } => Made::Cut { w: *w, h: *h, r: *r },
        Hole::Unknown(_) => return None,
    })
}

/// A rail's fixing when the workshop sets none (doc 12-3d §10.3; the program's assumption, unchecked): M5, at most
/// 300 mm apart, 25 mm from each end.
pub fn rail_fixing_default() -> CarrierFixing {
    CarrierFixing {
        screw: Screw::M5,
        pitch: Nm::from_mm(300),
        from_end: Nm::from_mm(25),
        extra: Default::default(),
    }
}

/// A duct's fixing when the workshop sets none: M4, at most 300 mm apart, 25 mm from each end.
pub fn duct_fixing_default() -> CarrierFixing {
    CarrierFixing {
        screw: Screw::M4,
        pitch: Nm::from_mm(300),
        from_end: Nm::from_mm(25),
        extra: Default::default(),
    }
}

/// The places of the fixing holes along a straight piece of length `l` (doc 12-3d §10.3): one at `e` from each end and
/// `n − 2` between them, evenly spaced, `n = ⌈(l − 2e) / p⌉ + 1`, each rounded to the whole millimetre (half up); a
/// piece no longer than `2e` (or a pitch of 0) gets one hole at its middle. In nanometres from the piece's start.
pub fn fixings(l: i128, e: i128, p: i128) -> Vec<i128> {
    let mm = |x: i128| (x + 500_000).div_euclid(1_000_000) * 1_000_000;
    let span = l - 2 * e;
    if span <= 0 || p <= 0 {
        return vec![mm(l / 2)];
    }
    let n = (span + p - 1) / p + 1;
    (0..n).map(|i| mm(e + span * i / (n - 1))).collect()
}

/// The fixing holes of a rail on its axis and of a duct on its centreline (doc 12-3d §10.3), piece by piece along its
/// path, as the workshop's Rail fixing and Duct fixing say (else [`rail_fixing_default`], [`duct_fixing_default`]); a
/// carrier of a newer kind has none.
pub fn carrier_fixings(carrier: &Carrier, settings: &WorkshopSettings) -> Vec<(Pt2, Hole)> {
    let fixing = match carrier.kind {
        CarrierKind::Rail => settings.rail_fixing.clone().unwrap_or_else(rail_fixing_default),
        CarrierKind::Duct => settings.duct_fixing.clone().unwrap_or_else(duct_fixing_default),
        CarrierKind::Unknown(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for w in carrier.path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (
            i128::from(b.x.0) - i128::from(a.x.0),
            i128::from(b.y.0) - i128::from(a.y.0),
        );
        // The pieces of a layout path run along the axes; a slanted one is measured as its longer leg.
        let l = dx.abs().max(dy.abs());
        if l == 0 {
            continue;
        }
        for s in fixings(l, i128::from(fixing.from_end.0), i128::from(fixing.pitch.0)) {
            let x = i128::from(a.x.0) + dx * s / l;
            let y = i128::from(a.y.0) + dy * s / l;
            if let (Ok(x), Ok(y)) = (i64::try_from(x), i64::try_from(y)) {
                out.push((Pt2::new(Nm(x), Nm(y)), Hole::Fixing(fixing.screw.clone())));
            }
        }
    }
    out
}

/// Who a hole of a surface is for (doc 12-3d §10.4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Owner {
    /// A placement's part: its hole at `index` of `mech.holes`.
    Placement {
        /// The placement.
        p: Id<Placement>,
        /// The hole's index in the part's pattern.
        index: u16,
    },
    /// A rail's or a duct's fixing at `index` along its path.
    Carrier {
        /// The carrier.
        c: Id<Carrier>,
        /// The hole's index along the path.
        index: u16,
    },
    /// A hole a person added, or a cable's gland ([`SurfaceFeature`]).
    ByHand {
        /// The feature.
        feature: Id<SurfaceFeature>,
    },
}

/// One hole of a surface ([`SurfaceFeatures`]).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Feature {
    /// Its number in the drilling order, H1 … Hn.
    pub no: u32,
    /// Its centre in the surface frame (a rectangle's centre).
    pub at: Pt2,
    /// The hole as its owner names it (a rectangle turned with its part).
    pub hole: Hole,
    /// How it is made.
    pub made: Made,
    /// Its owners, in order; two holes made alike that meet are one with both owners.
    pub owners: Vec<Owner>,
}

/// The hole of a pattern at `index` after its owner's choice: moved by its offset, `None` where it is left out.
pub fn chosen(at: Pt2, index: usize, choices: &[HoleChoice]) -> Option<Pt2> {
    let Some(c) = choices.iter().find(|c| usize::from(c.index) == index) else {
        return Some(at);
    };
    if c.omit {
        return None;
    }
    match c.offset {
        Some(o) => Some(Pt2::new(
            Nm(at.x.0.checked_add(o.x.0)?),
            Nm(at.y.0.checked_add(o.y.0)?),
        )),
        None => Some(at),
    }
}

/// The holes of a placement's pattern at their places, before its choices: the part's `mech.holes` turned with the
/// placement (a rectangle's sides swapped by a quarter turn) and moved to its pose.
pub fn placement_holes(q: &Q, p: Id<Placement>) -> Vec<(Pt2, Hole)> {
    let geom = q.get::<PlacementGeometry>(p);
    let Some(g) = geom.as_ref() else {
        return Vec::new();
    };
    let quarter = g.turn.k() % 2 == 1;
    g.holes
        .iter()
        .map(|(at, hole)| {
            let hole = match hole {
                Hole::Rect { w, h, r } if quarter => Hole::Rect { w: *h, h: *w, r: *r },
                other => other.clone(),
            };
            (*at, hole)
        })
        .collect()
}

/// Two holes meet when their centres lie within this of each other: 0.5 mm (doc 12-3d §10.4, rule 5).
const MEET: i128 = 500_000;

/// `surface_features(surface)`: every hole of a surface (doc 12-3d §10.4) with its centre, its hole, how it is made and
/// its owners, numbered in drilling order: the holes of the placements on it and on its carriers, the carriers' fixing
/// holes, its `SurfaceFeature`s; each owner's choices applied; two holes made alike within 0.5 mm one with both
/// owners; then by tool (the drills by size, the punches by size, the rectangles by size), within a tool row by row
/// from the bottom (a row: one height to the whole millimetre), the first row left to right, the next right to left;
/// ties by the first owner. A hole whose size the table has not is not listed. Empty for a missing surface.
#[derive(Debug)]
pub struct SurfaceFeatures;

impl Query for SurfaceFeatures {
    type Key = Id<MountingSurface>;
    type Value = Vec<Feature>;
    const FAMILY: &'static str = "surface_features";
    const POLICY: Policy = Policy::Memo;

    fn run(q: &Q, key: &Self::Key) -> Arc<Vec<Feature>> {
        Arc::new(surface_features(q, *key))
    }
}

fn surface_features(q: &Q, surface: Id<MountingSurface>) -> Vec<Feature> {
    let Some(s) = q.table::<MountingSurface>().get(surface) else {
        return Vec::new();
    };
    let workshop = &q.project().settings.workshop;
    let mut raw: Vec<(Pt2, Hole, Owner)> = Vec::new();
    let carriers: Vec<Id<Carrier>> = q.idx().carriers_of_surface(surface).collect();
    let mut placements: Vec<Id<Placement>> = q.idx().placements_on_surface(surface).collect();
    for c in &carriers {
        placements.extend(q.idx().placements_by_carrier(*c));
    }
    placements.sort();
    placements.dedup();
    let placement_table = q.table::<Placement>();
    for p in placements {
        let choices = placement_table
            .get(p)
            .map(|x| x.hole_choices.as_slice())
            .unwrap_or_default();
        for (i, (at, hole)) in placement_holes(q, p).into_iter().enumerate() {
            if let (Some(at), Ok(index)) = (chosen(at, i, choices), u16::try_from(i)) {
                raw.push((at, hole, Owner::Placement { p, index }));
            }
        }
    }
    let carrier_table = q.table::<Carrier>();
    for c in carriers {
        let Some(carrier) = carrier_table.get(c) else {
            continue;
        };
        for (i, (at, hole)) in carrier_fixings(carrier, workshop).into_iter().enumerate() {
            if let (Some(at), Ok(index)) = (chosen(at, i, &carrier.hole_choices), u16::try_from(i)) {
                raw.push((at, hole, Owner::Carrier { c, index }));
            }
        }
    }
    let feature_table = q.table::<SurfaceFeature>();
    for f in q.idx().features_of_surface(surface) {
        if let Some(x) = feature_table.get(f) {
            raw.push((x.at, x.hole.clone(), Owner::ByHand { feature: f }));
        }
    }
    raw.sort_by_key(|(_, _, o)| *o);
    // Made, then merged where two made alike meet.
    let mut holes: Vec<Feature> = Vec::new();
    for (at, hole, owner) in raw {
        let Some(m) = made(&hole, &s.kind, &workshop.fixing) else {
            continue;
        };
        let meets = |f: &Feature| {
            let (dx, dy) = (
                i128::from(f.at.x.0) - i128::from(at.x.0),
                i128::from(f.at.y.0) - i128::from(at.y.0),
            );
            f.made == m && dx * dx + dy * dy <= MEET * MEET
        };
        match holes.iter_mut().find(|f| meets(f)) {
            Some(f) => f.owners.push(owner),
            None => holes.push(Feature {
                no: 0,
                at,
                hole,
                made: m,
                owners: vec![owner],
            }),
        }
    }
    number(&mut holes);
    holes
}

/// Puts `holes` in drilling order and numbers them (doc 12-3d §10.4, rule 6).
fn number(holes: &mut [Feature]) {
    let row = |f: &Feature| (i128::from(f.at.y.0) + 500_000).div_euclid(1_000_000);
    holes.sort_by(|a, b| {
        (a.made.tool(), row(a), a.at.x, a.owners.first()).cmp(&(
            b.made.tool(),
            row(b),
            b.at.x,
            b.owners.first(),
        ))
    });
    // Within a tool every other row runs right to left.
    let mut start = 0;
    while start < holes.len() {
        let tool = holes[start].made.tool();
        let end = start
            + holes[start..]
                .iter()
                .take_while(|f| f.made.tool() == tool)
                .count();
        let mut i = start;
        let mut reverse = false;
        while i < end {
            let r = row(&holes[i]);
            let j = i + holes[i..end].iter().take_while(|f| row(f) == r).count();
            if reverse {
                holes[i..j].reverse();
            }
            reverse = !reverse;
            i = j;
        }
        start = end;
    }
    for (i, f) in holes.iter_mut().enumerate() {
        f.no = u32::try_from(i + 1).unwrap_or(u32::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: i128 = 1_000_000;

    #[test]
    fn fixing_holes_are_spread_along_a_piece_from_its_ends() {
        let at = |l: i128| fixings(l * MM, 25 * MM, 300 * MM);
        assert_eq!(at(470), [25 * MM, 235 * MM, 445 * MM]);
        // An exact multiple of the pitch gets no extra hole; a piece no longer than twice the distance one at its middle.
        assert_eq!(at(650), [25 * MM, 325 * MM, 625 * MM]);
        assert_eq!(at(40), [20 * MM]);
    }

    #[test]
    fn a_fixing_is_tapped_on_the_plate_and_through_in_a_door() {
        let m5 = Hole::Fixing(Screw::M5);
        let rule = FixingRule::Tapped;
        assert_eq!(
            made(&m5, &SurfaceKind::Plate, &rule),
            Some(Made::Tap {
                drill: Nm(4_200_000),
                screw: Screw::M5
            })
        );
        assert_eq!(
            made(&m5, &SurfaceKind::Door, &rule),
            Some(Made::Drill { d: Nm(5_500_000) })
        );
        assert_eq!(
            made(&m5, &SurfaceKind::Plate, &FixingRule::SelfTapping),
            Some(Made::Drill { d: Nm(4_600_000) })
        );
        assert_eq!(
            made(&Hole::Cutout(ew_schema::Cutout::D22), &SurfaceKind::Door, &rule),
            Some(Made::Punch { d: Nm(22_300_000) })
        );
    }
}
