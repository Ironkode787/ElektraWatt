// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The occupancy of a page (docs/design/06a-drafting.md §3.3): one structure over the half grid of the page, built
//! once per run and changed as wires are laid and lifted. The search ([`crate::search`]) and the structured phases of
//! the router read it.
//!
//! A **node** is a point of the half grid, `(HALF·i, HALF·j)` ticks; a **move** joins a node to its neighbour on the
//! right or below. Per node it holds how many parts' bodies hold it strictly inside (grown by [`BODY_GAP`], and
//! apart by half a step for the search on the half grid), what reserves it (a pin's point, its stub of [`STUB`] along
//! its side, a junction), and how many wire vertices lie on it; per move, how many wires run along it, how many cross
//! it (a wire across its line, its coordinate in the move's half-open span, the far node included), and the texts it
//! runs through with the ticks inside. A point off the half grid takes its nearest node.
//!
//! The structure covers the drawing area, every node a search may take (its last area is the drawing area): a trunk
//! laid far beyond the pins it taps, or a route the search took round a group of parts, is held for the jobs after it.
//! A node outside the area holds nothing. A page whose half grid exceeds four times [`MAX_NODES`] has no occupancy
//! (the router then routes nothing).

use std::collections::BTreeMap;

use ew_ids::Uuid;

use crate::consts::{BODY_GAP, HALF, MAX_NODES, STUB};
use crate::geom;
use crate::model::{DraftPage, JunctionId, LabelId, PartId, PinId, Pt, Rect, Tick};

/// A node of the half grid by its indices: it stands at `(HALF·i, HALF·j)` ticks.
pub type Node = (i32, i32);

/// A move between two neighbouring nodes: from the node to the one on its right (`false`) or below it (`true`).
pub type Move = (Node, bool);

/// The node nearest to `p`.
pub fn node_of(p: Pt) -> Node {
    let near = |v: Tick| (i64::from(v) + i64::from(HALF / 2)).div_euclid(i64::from(HALF));
    (sat(near(p.x)), sat(near(p.y)))
}

/// The point of node `n`.
pub fn point_of(n: Node) -> Pt {
    Pt::new(n.0.saturating_mul(HALF), n.1.saturating_mul(HALF))
}

fn sat(v: i64) -> i32 {
    i32::try_from(v).unwrap_or(if v < 0 { i32::MIN } else { i32::MAX })
}

/// What reserves a node for its own wires.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Holder {
    /// A pin: its point (`stub: false`) or a node of its stub; `wired` when a wire of the page ends at it.
    Pin {
        /// The pin.
        pin: PinId,
        /// A node of its stub rather than its point.
        stub: bool,
        /// Whether a wire of the page ends at it.
        wired: bool,
    },
    /// A junction.
    Junction(JunctionId),
}

/// A text over the page, as the search prices it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Text {
    /// Its box.
    pub rect: Rect,
    /// The record that prints it.
    pub owner: Uuid,
    /// The label it is, when the text placer may move it; `None` for a fixed text.
    pub label: Option<LabelId>,
}

/// The occupancy of one page (module documentation).
#[derive(Clone, Debug)]
pub struct Occupancy {
    lo: Node,
    hi: Node,
    width: usize,
    /// Per node: the bodies grown by a step, and by half a step, that hold it strictly inside.
    body: [Vec<u8>; 2],
    /// Per node: the wire vertices on it.
    vertex: Vec<u16>,
    /// Per move right and down from a node: the wires along it.
    along: [Vec<u16>; 2],
    /// Per move right and down from a node: the wires across it.
    cross: [Vec<u16>; 2],
    /// Per move right and down from a node: 0, or one more than the index of its list of texts in `text_lists`.
    text_at: [Vec<u32>; 2],
    /// The lists of `(text, ticks inside)` of the moves that run through texts.
    text_lists: Vec<Vec<(u32, u8)>>,
    /// The texts, in the order given.
    pub texts: Vec<Text>,
    /// What reserves a node, by node index.
    holders: BTreeMap<usize, Vec<Holder>>,
    /// The parts with a body (pins and not an outline drawn round others), with their bounds on the page, by ID.
    pub bodies: Vec<(PartId, Rect, Vec<Pt>)>,
    /// The outlines drawn round other parts (a valve terminal round its valves), on the page: a body too, which a wire
    /// leaves straight out of a pin inside it.
    pub frames: Vec<Rect>,
}

impl Occupancy {
    /// The occupancy of `page` with its parts, pins and junctions, and `texts`; no wire is laid yet. `wired`: whether a
    /// wire of the page ends at a pin. `None` for a page too large (module documentation).
    pub fn new(page: &DraftPage, texts: Vec<Text>, wired: &dyn Fn(PinId) -> bool) -> Option<Occupancy> {
        let mut parts: Vec<_> = page.parts.iter().collect();
        parts.sort_by_key(|p| p.id);
        let area = page.area;
        let ceil = |v: Tick| sat(i64::from(v).div_euclid(i64::from(HALF)) + 1);
        let floor = |v: Tick| sat(i64::from(v).div_euclid(i64::from(HALF)));
        let lo = (floor(area.x0), floor(area.y0));
        let hi = (ceil(area.x1).max(lo.0), ceil(area.y1).max(lo.1));
        let width = usize::try_from(i64::from(hi.0) - i64::from(lo.0) + 1).ok()?;
        let height = usize::try_from(i64::from(hi.1) - i64::from(lo.1) + 1).ok()?;
        let nodes = width.checked_mul(height)?;
        if nodes > 4 * MAX_NODES {
            return None;
        }
        let mut occ = Occupancy {
            lo,
            hi,
            width,
            body: [vec![0; nodes], vec![0; nodes]],
            vertex: vec![0; nodes],
            along: [vec![0; nodes], vec![0; nodes]],
            cross: [vec![0; nodes], vec![0; nodes]],
            text_at: [vec![0; nodes], vec![0; nodes]],
            text_lists: Vec::new(),
            texts: Vec::new(),
            holders: BTreeMap::new(),
            bodies: Vec::new(),
            frames: Vec::new(),
        };
        for p in parts {
            if p.frame_only {
                occ.frames.push(p.bounds());
            }
            if p.frame_only || !p.pins.is_empty() {
                let b = p.bounds();
                if !p.frame_only {
                    occ.bodies
                        .push((p.id, b, p.pins.iter().map(|q| p.at.plus(q.at)).collect()));
                }
                for (k, grow) in [(0, BODY_GAP), (1, HALF)] {
                    occ.each_inside(b.grown(grow), |occ, n| {
                        occ.body[k][n] = occ.body[k][n].saturating_add(1);
                    });
                }
            }
            let mut pins: Vec<_> = p.pins.iter().collect();
            pins.sort_by_key(|q| q.id);
            for pin in pins {
                let at = p.at.plus(pin.at);
                let w = wired(pin.id);
                occ.hold(
                    node_of(at),
                    Holder::Pin {
                        pin: pin.id,
                        stub: false,
                        wired: w,
                    },
                );
                if let Some(side) = pin.side {
                    let u = side.unit();
                    for d in [HALF, STUB] {
                        occ.hold(
                            node_of(at.plus(Pt::new(u.x * d, u.y * d))),
                            Holder::Pin {
                                pin: pin.id,
                                stub: true,
                                wired: w,
                            },
                        );
                    }
                }
            }
        }
        let mut junctions: Vec<_> = page.junctions.iter().collect();
        junctions.sort_by_key(|j| j.id);
        for j in junctions {
            occ.hold(node_of(j.at), Holder::Junction(j.id));
        }
        for t in texts {
            occ.add_text(t);
        }
        Some(occ)
    }

    /// The index of node `n`; `None` outside the covered box.
    pub fn index(&self, n: Node) -> Option<usize> {
        if n.0 < self.lo.0 || n.0 > self.hi.0 || n.1 < self.lo.1 || n.1 > self.hi.1 {
            return None;
        }
        let x = usize::try_from(n.0 - self.lo.0).ok()?;
        let y = usize::try_from(n.1 - self.lo.1).ok()?;
        Some(y * self.width + x)
    }

    /// Calls `f` with the index of every covered node strictly inside `r`.
    fn each_inside(&mut self, r: Rect, mut f: impl FnMut(&mut Occupancy, usize)) {
        let first = |v: Tick| sat(i64::from(v).div_euclid(i64::from(HALF)) + 1);
        let last = |v: Tick| sat((i64::from(v) - 1).div_euclid(i64::from(HALF)));
        for j in first(r.y0).max(self.lo.1)..=last(r.y1).min(self.hi.1) {
            for i in first(r.x0).max(self.lo.0)..=last(r.x1).min(self.hi.0) {
                if let Some(n) = self.index((i, j)) {
                    f(self, n);
                }
            }
        }
    }

    fn hold(&mut self, n: Node, h: Holder) {
        if let Some(k) = self.index(n) {
            let list = self.holders.entry(k).or_default();
            if !list.contains(&h) {
                list.push(h);
                list.sort();
            }
        }
    }

    /// Takes the junction `j` from `from` to `to`.
    pub fn move_junction(&mut self, j: JunctionId, from: Pt, to: Pt) {
        if let Some(k) = self.index(node_of(from))
            && let Some(list) = self.holders.get_mut(&k)
        {
            list.retain(|h| *h != Holder::Junction(j));
        }
        self.hold(node_of(to), Holder::Junction(j));
    }

    fn add_text(&mut self, t: Text) {
        let r = t.rect.grown(-1);
        let k = u32::try_from(self.texts.len()).unwrap_or(u32::MAX);
        self.texts.push(t);
        if r.w() <= 0 || r.h() <= 0 {
            return;
        }
        let span = |lo: Tick, hi: Tick| {
            sat(i64::from(lo).div_euclid(i64::from(HALF)))
                ..=sat((i64::from(hi) - 1).div_euclid(i64::from(HALF)))
        };
        let inside = |lo: Tick, hi: Tick| {
            sat(i64::from(lo).div_euclid(i64::from(HALF)) + 1)
                ..=sat((i64::from(hi) - 1).div_euclid(i64::from(HALF)))
        };
        // Moves right on the rows strictly inside, over the columns they share with the box; and moves down.
        for down in [false, true] {
            let (rows, cols) = if down {
                (inside(r.x0, r.x1), span(r.y0, r.y1))
            } else {
                (inside(r.y0, r.y1), span(r.x0, r.x1))
            };
            let (c0, c1) = if down { (r.y0, r.y1) } else { (r.x0, r.x1) };
            for line in rows {
                for c in cols.clone() {
                    let a = c.saturating_mul(HALF);
                    let ticks = a.saturating_add(HALF).min(c1) - a.max(c0);
                    if ticks <= 0 {
                        continue;
                    }
                    let n = if down { (line, c) } else { (c, line) };
                    let Some(idx) = self.index(n) else { continue };
                    let slot = &mut self.text_at[usize::from(down)][idx];
                    if *slot == 0 {
                        self.text_lists.push(Vec::new());
                        *slot = u32::try_from(self.text_lists.len()).unwrap_or(u32::MAX);
                    }
                    let list = usize::try_from(*slot - 1).unwrap_or(0);
                    if let Some(l) = self.text_lists.get_mut(list) {
                        l.push((k, u8::try_from(ticks).unwrap_or(u8::MAX)));
                    }
                }
            }
        }
    }

    /// Lays the wire drawn along `route` (`add`), or lifts it again.
    pub fn lay(&mut self, route: &[Pt], add: bool) {
        let pts = geom::clean(route);
        let bump = |v: &mut u16| {
            *v = if add {
                v.saturating_add(1)
            } else {
                v.saturating_sub(1)
            }
        };
        for p in &pts {
            if let Some(k) = self.index(node_of(*p)) {
                bump(&mut self.vertex[k]);
            }
        }
        for (a, b) in geom::segments(&pts) {
            if a == b || (a.x != b.x && a.y != b.y) {
                continue;
            }
            let horizontal = a.y == b.y;
            // (line, from, to) along the segment's axis.
            let (line, s0, s1) = if horizontal {
                (a.y, a.x.min(b.x), a.x.max(b.x))
            } else {
                (a.x, a.y.min(b.y), a.y.max(b.y))
            };
            let first = sat(i64::from(s0).div_euclid(i64::from(HALF)));
            let last = sat((i64::from(s1) - 1).div_euclid(i64::from(HALF)));
            // Along: the moves on its line that share a stretch with it.
            if line.rem_euclid(HALF) == 0 {
                let l = line / HALF;
                for c in first..=last {
                    let n = if horizontal { (c, l) } else { (l, c) };
                    if let Some(k) = self.index(n) {
                        bump(&mut self.along[usize::from(!horizontal)][k]);
                    }
                }
            }
            // Across: the moves of the other axis whose line lies strictly inside its extent and whose span holds its
            // line, the far node included.
            let m = sat((i64::from(line) - 1).div_euclid(i64::from(HALF)));
            let strict = sat(i64::from(s0).div_euclid(i64::from(HALF)) + 1)
                ..=sat((i64::from(s1) - 1).div_euclid(i64::from(HALF)));
            for c in strict {
                let n = if horizontal { (c, m) } else { (m, c) };
                if let Some(k) = self.index(n) {
                    bump(&mut self.cross[usize::from(horizontal)][k]);
                }
            }
        }
    }

    /// The bodies holding node `n`, grown by a step (`half: false`) or by half a step.
    pub fn bodies_at(&self, n: Node, half: bool) -> u8 {
        self.index(n).map_or(0, |k| self.body[usize::from(half)][k])
    }

    /// The wire vertices on node `n`.
    pub fn vertices_at(&self, n: Node) -> u16 {
        self.index(n).map_or(0, |k| self.vertex[k])
    }

    /// What reserves node `n`.
    pub fn holders_at(&self, n: Node) -> &[Holder] {
        self.index(n)
            .and_then(|k| self.holders.get(&k))
            .map_or(&[], Vec::as_slice)
    }

    /// The wires along move `m`.
    pub fn along(&self, (n, down): Move) -> u16 {
        self.index(n).map_or(0, |k| self.along[usize::from(down)][k])
    }

    /// The wires across move `m`.
    pub fn across(&self, (n, down): Move) -> u16 {
        self.index(n).map_or(0, |k| self.cross[usize::from(down)][k])
    }

    /// The texts move `m` runs through, with the ticks inside each.
    pub fn texts_on(&self, (n, down): Move) -> &[(u32, u8)] {
        self.index(n)
            .and_then(|k| {
                let slot = self.text_at[usize::from(down)][k];
                (slot > 0).then(|| usize::try_from(slot - 1).ok()).flatten()
            })
            .and_then(|l| self.text_lists.get(l))
            .map_or(&[], Vec::as_slice)
    }
}
