// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The page solve's one bridge to the one measure ([`ew_drafting::measure`], docs/design/06a-drafting.md §1.2): a
//! [`PageGeometry`] in page nm as the layout's page in ticks, and the faults the solve weighs. It goes with the solve
//! (§10, step 2); it is no second measure.
//!
//! Each body is a part of its representation, carrying the representation's connection points as its pins (a black
//! box's outline is a second part of the same ID, so a wire out of a box pin passes the outline on its escape); each
//! text box is a fixed text of its owner; an end is a pin when a connection point has its ID, else a junction.

use std::collections::BTreeMap;

use ew_drafting::consts::TICKS_PER_STEP;
use ew_drafting::{
    DraftPage, End, FixedText, Junction, JunctionId, Part, PartId, Passed, Pin, PinId, Role, TextRef, Wire,
    WireClass, WireId,
};
use ew_ids::Uuid;
use ew_schema::Dir4;
use ew_units::{Grid, Nm};

use super::page::{PageGeometry, Point, Rect};

/// A connection through a point it does not end at (F2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Touch {
    /// The connection.
    pub connection: Uuid,
    /// The connection point's or the junction's ID.
    pub point: Uuid,
}

/// Two connections on one line (F3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Overlap {
    /// The two connections.
    pub connections: [Uuid; 2],
}

/// A connection through a representation's body (F4) or one of its texts (C4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Through {
    /// The connection.
    pub connection: Uuid,
    /// The representation (a text's owner).
    pub rep: Uuid,
}

/// Two connections crossing (C3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Crossing {
    /// The two connections.
    pub connections: [Uuid; 2],
    /// Where, in page nm.
    pub at: Point,
}

/// A connection's bends (C2).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Bends {
    /// The connection.
    pub connection: Uuid,
    /// Its corners.
    pub bends: usize,
}

/// What the page solve, a move's re-solve and their tests read of the one measure, by the IDs of the geometry.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Measures {
    /// F2.
    pub touches: Vec<Touch>,
    /// F3.
    pub overlaps: Vec<Overlap>,
    /// F4.
    pub through_bodies: Vec<Through>,
    /// C4: the texts are the geometry's text boxes.
    pub through_texts: Vec<Through>,
    /// F5: the junctions.
    pub four_way: Vec<Uuid>,
    /// C3.
    pub crossings: Vec<Crossing>,
    /// C2.
    pub bends: Vec<Bends>,
}

impl Measures {
    /// The items that break a rule the solve keeps: F2 to F5 and C4.
    pub fn hard(&self) -> usize {
        self.touches.len()
            + self.overlaps.len()
            + self.through_bodies.len()
            + self.through_texts.len()
            + self.four_way.len()
    }

    /// The bends of every connection together.
    pub fn bend_total(&self) -> usize {
        self.bends.iter().map(|b| b.bends).sum()
    }
}

/// The one measure of `page`'s connections (module documentation).
pub fn measure(page: &PageGeometry) -> Measures {
    let grid = grid_of(page);
    let q = ew_drafting::measure(&draft(page));
    let f = q.findings;
    let nm = |t: i32| {
        grid.nm_of_tick(i64::from(t), TICKS_PER_STEP as u32)
            .map_or(0, |n| n.0)
    };
    Measures {
        touches: f
            .through_points
            .iter()
            .map(|(w, p, _)| Touch {
                connection: w.0,
                point: match p {
                    Passed::Pin(p) => p.0,
                    Passed::Junction(j) => j.0,
                },
            })
            .collect(),
        overlaps: f
            .on_wires
            .iter()
            .map(|([a, b], _)| Overlap {
                connections: [a.0, b.0],
            })
            .collect(),
        through_bodies: f
            .through_parts
            .iter()
            .map(|(w, p, _)| Through {
                connection: w.0,
                rep: p.0,
            })
            .collect(),
        through_texts: f
            .across_texts
            .iter()
            .filter_map(|(w, t, _)| match t {
                TextRef::Fixed(owner, _) => Some(Through {
                    connection: w.0,
                    rep: *owner,
                }),
                TextRef::Label(_) => None,
            })
            .collect(),
        four_way: f.four_way.iter().map(|(j, _, _)| j.0).collect(),
        crossings: f
            .crossings
            .iter()
            .map(|([a, b], at)| Crossing {
                connections: [a.0, b.0],
                at: (nm(at.x), nm(at.y)),
            })
            .collect(),
        bends: f
            .bends
            .iter()
            .map(|(w, n)| Bends {
                connection: w.0,
                bends: *n,
            })
            .collect(),
    }
}

/// The page's grid (a 4 mm step where its numbers make no grid).
fn grid_of(page: &PageGeometry) -> Grid {
    u64::try_from(page.grid.0)
        .ok()
        .zip(u64::try_from(page.grid.1).ok())
        .and_then(|(n, d)| Grid::new(n, d).ok())
        .unwrap_or(Grid::MM4)
}

/// `page` as the layout's page in ticks (module documentation).
fn draft(page: &PageGeometry) -> DraftPage {
    let grid = grid_of(page);
    let per = TICKS_PER_STEP as u32;
    let tick =
        |v: i64| i32::try_from(grid.tick_of(Nm(v), per).0).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX });
    let pt = |p: Point| ew_drafting::Pt::new(tick(p.0), tick(p.1));
    let rect = |r: Rect| {
        let clamp = |v: i64| i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX });
        let (x0, x1) = grid.tick_span(Nm(r[0]), Nm(r[2]), per);
        let (y0, y1) = grid.tick_span(Nm(r[1]), Nm(r[3]), per);
        ew_drafting::Rect {
            x0: clamp(x0),
            y0: clamp(y0),
            x1: clamp(x1),
            y1: clamp(y1),
        }
    };
    let mut pins: BTreeMap<Uuid, Vec<Pin>> = BTreeMap::new();
    for cp in &page.cps {
        pins.entry(cp.rep).or_default().push(Pin {
            id: PinId(cp.id),
            at: pt(cp.at),
            side: match cp.dir {
                Some(Dir4::Up) => Some(ew_drafting::Side::Up),
                Some(Dir4::Down) => Some(ew_drafting::Side::Down),
                Some(Dir4::Left) => Some(ew_drafting::Side::Left),
                Some(Dir4::Right) => Some(ew_drafting::Side::Right),
                _ => None,
            },
            off_grid: false,
        });
    }
    let part = |rep: Uuid, body: ew_drafting::Rect| Part {
        id: PartId(rep),
        at: ew_drafting::Pt::default(),
        body,
        frame_only: false,
        pins: pins.get(&rep).cloned().unwrap_or_default(),
        unit: 0,
        role: Role::Other,
        locked: false,
        off_grid: false,
    };
    let mut parts: Vec<Part> = page.bodies.iter().map(|b| part(b.rep, rect(b.rect))).collect();
    for rep in pins.keys() {
        if !page.bodies.iter().any(|b| b.rep == *rep) {
            parts.push(part(*rep, ew_drafting::Rect::default()));
        }
    }
    let junction = |id: Uuid| page.junctions.iter().any(|j| j.id == id);
    let end = |id: Uuid| {
        if junction(id) {
            End::Junction(JunctionId(id))
        } else {
            End::Pin(PinId(id))
        }
    };
    DraftPage {
        area: rect(page.drawing_area.unwrap_or(page.sheet)),
        parts,
        junctions: page
            .junctions
            .iter()
            .map(|j| Junction {
                id: JunctionId(j.id),
                at: pt(j.at),
            })
            .collect(),
        wires: page
            .connections
            .iter()
            .map(|c| Wire {
                id: WireId(c.id),
                ends: [end(c.ends[0]), end(c.ends[1])],
                route: if c.routed {
                    c.polyline.iter().map(|p| pt(*p)).collect()
                } else {
                    Vec::new()
                },
                computed: false,
                class: WireClass::Wire,
                supply: c.supply,
            })
            .collect(),
        fixed_texts: page
            .texts
            .iter()
            .map(|t| FixedText {
                owner: t.rep,
                rect: rect(t.rect),
            })
            .collect(),
        ..DraftPage::default()
    }
}
