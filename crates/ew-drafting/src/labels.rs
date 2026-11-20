// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The one text placer (docs/design/06a-drafting.md §5): [`settle_labels`] gives every label of a page its place
//! from the places of its kind, against the parts' bodies, the wires, the fixed texts and the labels placed before
//! it. The scene calls it when it draws a page, and [`crate::measure`] calls it, so the canvas, the PDF and the checks
//! see one page.
//!
//! # The places (§5.2)
//!
//! A label's places are the ones its adapter gives ([`Places::At`]: a symbol's slot as its author set it), those of
//! them outside the drawing area moved in to stand [`LABEL_CLEAR`] inside its line, "one line lower" for the second
//! of a row (the two-rows rule), then the places of its kind, each once. For a part whose pins leave up or down (or
//! that has none with a side) the corners are: upper left (right of the box against the body's left edge, its top
//! level with the body's top), upper right (beyond the pins' numbers at their first places), lower left, lower right
//! (the same), centred above (when no wire leaves upward), centred below, above the upper left and upper right
//! corners, centred below one line lower; for a part whose pins leave sideways: centred above, centred below, above
//! and below at either end, centred below one line lower. Every gap to the owner is [`LABEL_CLEAR`].
//!
//! | Kind | Places after the adapter's |
//! |---|---|
//! | pin number | right of the pin's line at its outer end, left of it (above and below for a pin leaving sideways) |
//! | tag, cross-reference, data, contact list | the corners |
//! | function text | centred below, right of the body at its middle, then the corners |
//! | tag of a drawn box | above the upper left corner ending at the left edge; above the upper edge from the corner; above the upper right corner; inside at the upper left |
//! | data of a drawn box | left of the box under its top, under the box's data before it; right of it; inside under the tag; above the box over its tag |
//! | name of a mark | in line with the pin beyond the body (the side away from the wire); centred above; centred below; centred above one line higher |
//! | cross-reference of a mark | beside the wire past the pin, under it then over it; the same beyond the arrow; centred under the arrow, then over it |
//! | name of a terminal | right below its centre, left below it (with wires across it: left below, left above); beside the body, left then right |
//! | strip designation | left of the terminal at its middle; centred above it; above it, ending at its left edge |
//! | wire number | along the wire ([`along_wire`]) |
//! | cable tag and type | the type before the mark's line and the tag after it; both after it (tag over type); both before it |
//!
//! **The block rule.** The tag, cross-references, data and contact list of one part that stand on one side of its
//! body (left of its middle, right of it, or across it) and one under the other are one block: it is placed as one
//! label of their joined box, with the places of its first member's kind, and its members keep their places in it. A
//! member that overlaps another at its first place, or stands beside the block, is placed on its own.
//!
//! **The two-rows rule.** A tag (of a symbol, a drawn box or a terminal) or a contact list whose first place reaches
//! within [`LABEL_CLEAR`] of the first place of a label of its kind level with it, on another part, has "one line
//! lower" (its height and [`LABEL_CLEAR`] down) as its second place: the first of a row keeps its place, the second
//! steps down, the third finds its place free again: two rows.
//!
//! # The choice (§5.3)
//!
//! Labels (blocks) are placed in the order of their kind's priority, then of their owner's place (top, then left),
//! then of their ID. Each takes its first place that lies in the drawing area and off the frame's bands and is clear
//! by [`LABEL_CLEAR`] of every part's body but its own (an outline drawn round other parts is no body), every wire
//! segment, every fixed text and every label placed before it. At its author's place it only must not lie on a fixed
//! text or a text of its own part, and a pin's number there may stand on another part's body; two texts of one part
//! at their author's places, one over the other or one a pin's number, do not count against each other. A label
//! stands nowhere it reads as another part's (nearer another part of the same body size than its own); a label of a
//! part without pins stays inside its outline or clear of it; a place that is the first place of a label not yet
//! placed is taken only when no other is clear. A text whose author's place lies on a text of its own part placed
//! before it has the place under that text second.
//!
//! A label without a clear place takes its place of least cost ([`LABEL_CLASH`] for each text and body it lies on
//! and for leaving the drawing area, [`TEXT_CROSSED`] and [`TEXT_STEP`] per tick for each wire across it,
//! [`LABEL_RANK`] for each place down its list; ties to the earlier place). One repair round follows: for each such
//! label, each label it lies on is tried at its first other clear place, and that label then at its first clear
//! place; the change is kept when the labels' cost on the page is lower. Every list is read in a fixed order, so the
//! result does not depend on the order of the page's lists.

use std::collections::{BTreeMap, BTreeSet};

use crate::consts::{LABEL_CLASH, LABEL_CLEAR, LABEL_RANK, TEXT_CROSSED, TEXT_STEP, TICKS_PER_STEP};
use crate::geom::{self, Buckets, Seg};
use crate::model::{
    DraftPage, End, Label, LabelId, LabelKind, MarkId, Owner, Part, PartId, PinId, Places, Pt, Rect, Side,
    Tick, WireId,
};

/// Where the text placer set a label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settled {
    /// The label.
    pub id: LabelId,
    /// Its box on the page.
    pub rect: Rect,
    /// Its place down its list: 0 at its first place.
    pub rank: u32,
    /// It reads turned a quarter, from the bottom (a wire number along a vertical stretch): its box is its size
    /// turned.
    pub turned: bool,
}

/// One label, or the block of labels of one side of a part, as the placer moves it.
#[derive(Clone, Debug)]
struct Unit {
    /// The labels (by index into the page's labels) and the offset of each one's box from the unit's box.
    members: Vec<(usize, Pt)>,
    size: (Tick, Tick),
    kind: LabelKind,
    owner: Owner,
    /// The adapter's places on the page.
    given: Vec<Rect>,
    along: bool,
    /// The two-rows rule gives it "one line lower" as its second place.
    stagger: bool,
}

/// The things a label keeps clear of, filed in buckets.
struct Ground<'a> {
    page: &'a DraftPage,
    parts: BTreeMap<PartId, &'a Part>,
    /// Each pin on the page: its point, its side and its part.
    pins: BTreeMap<PinId, (Pt, Option<Side>, PartId)>,
    /// The pins a wire ends at.
    wired: BTreeSet<PinId>,
    bodies: Vec<(PartId, Rect)>,
    body_grid: Buckets,
    segs: Vec<(WireId, Seg)>,
    seg_grid: Buckets,
    fixed: Vec<Rect>,
    fixed_grid: Buckets,
}

impl<'a> Ground<'a> {
    fn of(page: &'a DraftPage) -> Ground<'a> {
        let mut parts = BTreeMap::new();
        let mut pins = BTreeMap::new();
        let mut bodies = Vec::new();
        let mut body_grid = Buckets::default();
        let mut sorted: Vec<&Part> = page.parts.iter().collect();
        sorted.sort_by_key(|p| p.id);
        for p in sorted {
            parts.insert(p.id, p);
            for pin in &p.pins {
                pins.insert(pin.id, (p.at.plus(pin.at), pin.side, p.id));
            }
            if !p.frame_only {
                body_grid.insert(bodies.len(), p.bounds());
                bodies.push((p.id, p.bounds()));
            }
        }
        let mut wired = BTreeSet::new();
        let mut segs = Vec::new();
        let mut seg_grid = Buckets::default();
        let mut wires: Vec<_> = page.wires.iter().collect();
        wires.sort_by_key(|w| w.id);
        for w in wires {
            for e in w.ends {
                if let End::Pin(p) = e {
                    wired.insert(p);
                }
            }
            for s in geom::segments(&geom::clean(&w.route)) {
                seg_grid.insert(segs.len(), geom::seg_box(s));
                segs.push((w.id, s));
            }
        }
        let mut fixed: Vec<Rect> = page.fixed_texts.iter().map(|t| t.rect).collect();
        fixed.sort();
        let mut fixed_grid = Buckets::default();
        for (i, r) in fixed.iter().enumerate() {
            fixed_grid.insert(i, *r);
        }
        Ground {
            page,
            parts,
            pins,
            wired,
            bodies,
            body_grid,
            segs,
            seg_grid,
            fixed,
            fixed_grid,
        }
    }

    /// The part whose body a label of `owner` may stand on: its own. A part without pins (a mark drawn as a symbol, a
    /// module's glyph) holds its texts inside its body or keeps them beside it: one across its outline is in its way
    /// ([`Ground::across_own`]).
    fn own_part(&self, owner: Owner) -> Option<PartId> {
        match owner {
            Owner::Part(p) => Some(p),
            _ => None,
        }
    }

    /// Whether `r` lies across the outline of its owner's body, the owner a part without pins.
    fn across_own(&self, owner: Owner, r: Rect) -> bool {
        let Owner::Part(p) = owner else { return false };
        self.parts
            .get(&p)
            .is_some_and(|x| x.pins.is_empty() && x.bounds().overlaps(r) && !x.bounds().holds(r))
    }

    /// Whether `r` lies in the drawing area and off the frame's bands.
    fn inside(&self, r: Rect) -> bool {
        self.page.area.holds(r) && !self.page.keep_out.iter().any(|k| k.overlaps(r))
    }

    /// Whether a label of `owner` at `r` is clear of every body but its own part's (with `bodies`), every wire and every
    /// fixed text; at its author's place (`at_first`) a fixed text only must not lie on it.
    fn clear(&self, owner: Owner, r: Rect, (bodies, at_first): (bool, bool)) -> bool {
        let g = r.grown(LABEL_CLEAR);
        let own = self.own_part(owner);
        self.inside(r)
            && !(bodies
                && self
                    .body_grid
                    .near(g)
                    .into_iter()
                    .any(|i| Some(self.bodies[i].0) != own && self.bodies[i].1.overlaps(g)))
            && !self
                .seg_grid
                .near(g)
                .into_iter()
                .any(|i| geom::crosses(self.segs[i].1, g))
            && !self.fixed_grid.near(g).into_iter().any(|i| {
                if at_first {
                    geom::texts_meet(self.fixed[i], r)
                } else {
                    self.fixed[i].overlaps(g)
                }
            })
            && !self.across_own(owner, r)
            && self.reads_as_own(owner, r)
    }

    /// Whether a label of a part at `r` stands nearer its part than any other part of its kind (a part with a body of
    /// the same size: the terminals of a strip, the contacts of a row), as §1.3 asks of every label.
    fn reads_as_own(&self, owner: Owner, r: Rect) -> bool {
        let Owner::Part(p) = owner else { return true };
        let Some(own) = self.parts.get(&p).map(|x| x.bounds()) else {
            return true;
        };
        let d = geom::gap(r, own);
        !self.body_grid.near(r.grown(d)).into_iter().any(|i| {
            self.bodies[i].0 != p
                && self.bodies[i].1.size() == own.size()
                && geom::gap(r, self.bodies[i].1) < d
        })
    }

    /// What a label of `owner` at `r` costs against the ground: [`LABEL_CLASH`] for each body but its own and each
    /// fixed text it lies on and for leaving the drawing area; [`TEXT_CROSSED`] and [`TEXT_STEP`] per tick inside for
    /// each wire across it. In sixteenths of a step.
    fn cost(&self, owner: Owner, r: Rect) -> i64 {
        let own = self.own_part(owner);
        let step = i64::from(TICKS_PER_STEP);
        let mut clashes = i64::from(!self.inside(r)) + i64::from(self.across_own(owner, r));
        clashes += self
            .body_grid
            .near(r)
            .into_iter()
            .filter(|&i| Some(self.bodies[i].0) != own && self.bodies[i].1.overlaps(r))
            .count() as i64;
        clashes += self
            .fixed_grid
            .near(r)
            .into_iter()
            .filter(|&i| self.fixed[i].overlaps(r))
            .count() as i64;
        let mut across: BTreeMap<WireId, i64> = BTreeMap::new();
        for i in self.seg_grid.near(r) {
            if let Some(s) = geom::stretch_inside(self.segs[i].1, r) {
                *across.entry(self.segs[i].0).or_default() += geom::length(s);
            }
        }
        clashes * LABEL_CLASH * step
            + across
                .values()
                .map(|t| TEXT_CROSSED * step + TEXT_STEP * t)
                .sum::<i64>()
    }
}

/// The geometry of a part that its labels' places follow from.
struct Shape {
    body: Rect,
    /// Its pins on the page with their sides.
    pins: Vec<(Pt, Option<Side>)>,
    /// Its pins leave up or down, or none has a side.
    upright: bool,
    /// A wire leaves it upward.
    wire_up: bool,
    /// The right edge of the body and of its pins' numbers at their first places, which a text at a right corner
    /// stands beyond.
    right: Tick,
}

impl Shape {
    fn of(ground: &Ground<'_>, part: &Part) -> Shape {
        let pins: Vec<(Pt, Option<Side>)> = part.pins.iter().map(|p| (part.at.plus(p.at), p.side)).collect();
        let upright =
            pins.iter().any(|(_, s)| s.is_some_and(Side::vertical)) || pins.iter().all(|(_, s)| s.is_none());
        let wire_up = part
            .pins
            .iter()
            .any(|p| p.side == Some(Side::Up) && ground.wired.contains(&p.id));
        let body = part.bounds();
        let right = ground
            .page
            .labels
            .iter()
            .filter(|l| l.owner == Owner::Part(part.id) && matches!(l.kind, LabelKind::PinNumber(_)))
            .filter_map(|l| match &l.places {
                Places::At(at) => at.first().map(|a| Rect::sized(part.at.plus(*a), l.size).x1),
                Places::AlongWire => None,
            })
            .fold(body.x1, Tick::max);
        Shape {
            body,
            pins,
            upright,
            wire_up,
            right,
        }
    }
}

/// A box of size `size` with its top-left corner at `(x, y)`.
fn bx(x: Tick, y: Tick, size: (Tick, Tick)) -> Rect {
    Rect::sized(Pt::new(x, y), size)
}

fn half(v: Tick) -> Tick {
    v.div_euclid(2)
}

/// The corners of a part (module documentation).
fn corners(s: &Shape, (w, h): (Tick, Tick)) -> Vec<Rect> {
    let (b, g, size) = (s.body, LABEL_CLEAR, (w, h));
    let cx = b.centre().x;
    let above = bx(cx - half(w), b.y0 - g - h, size);
    let below = bx(cx - half(w), b.y1 + g, size);
    // Centred below, one line lower: past a text that stands right under the body.
    let lower = bx(cx - half(w), b.y1 + 2 * g + h, size);
    if s.upright {
        let mut out = vec![
            bx(b.x0 - g - w, b.y0, size),
            bx(s.right + g, b.y0, size),
            bx(b.x0 - g - w, b.y1 - h, size),
            bx(s.right + g, b.y1 - h, size),
        ];
        if !s.wire_up {
            out.push(above);
        }
        out.push(below);
        // Above the upper corners, clear of the wires that leave upward between them.
        out.push(bx(b.x0 - w, b.y0 - g - h, size));
        out.push(bx(b.x1, b.y0 - g - h, size));
        out.push(lower);
        out
    } else {
        vec![
            above,
            below,
            bx(b.x0, b.y0 - g - h, size),
            bx(b.x1 - w, b.y0 - g - h, size),
            bx(b.x0, b.y1 + g, size),
            bx(b.x1 - w, b.y1 + g, size),
            lower,
        ]
    }
}

/// The places of a pin's number: beside the line its wire leaves along, at the pin's outer end.
fn pin_places(q: Pt, side: Option<Side>, (w, h): (Tick, Tick)) -> Vec<Rect> {
    let (g, size) = (LABEL_CLEAR, (w, h));
    match side {
        Some(Side::Up) | None => vec![bx(q.x + g, q.y, size), bx(q.x - g - w, q.y, size)],
        Some(Side::Down) => vec![bx(q.x + g, q.y - h, size), bx(q.x - g - w, q.y - h, size)],
        Some(Side::Left) => vec![bx(q.x, q.y - g - h, size), bx(q.x, q.y + g, size)],
        Some(Side::Right) => vec![bx(q.x - w, q.y - g - h, size), bx(q.x - w, q.y + g, size)],
    }
}

/// The places of the kind of a unit owned by a part (module documentation).
fn part_places(ground: &Ground<'_>, unit: &Unit, s: &Shape) -> Vec<Rect> {
    let (b, g) = (s.body, LABEL_CLEAR);
    let (w, h) = unit.size;
    let c = b.centre();
    let first_pin = s.pins.first().copied();
    match unit.kind {
        LabelKind::PinNumber(pin) => match ground.pins.get(&pin) {
            Some(&(q, side, _)) => pin_places(q, side, unit.size),
            None => Vec::new(),
        },
        LabelKind::TerminalName => {
            // Right below the point and left below it, turned a quarter with a terminal whose wires run across; then
            // beside the body in line with the point.
            let mut out = if s.upright {
                vec![
                    bx(c.x + g, c.y + g, unit.size),
                    bx(c.x - g - w, c.y + g, unit.size),
                ]
            } else {
                vec![
                    bx(c.x - g - w, c.y + g, unit.size),
                    bx(c.x - g - w, c.y - g - h, unit.size),
                ]
            };
            out.push(bx(b.x0 - g - w, c.y - half(h), unit.size));
            out.push(bx(b.x1 + g, c.y - half(h), unit.size));
            out
        }
        LabelKind::PathFunction => {
            let mut out = vec![
                bx(c.x - half(w), b.y1 + g, unit.size),
                bx(b.x1 + g, c.y - half(h), unit.size),
            ];
            out.extend(corners(s, unit.size));
            out
        }
        LabelKind::BoxTag => vec![
            bx(b.x0 - w, b.y0 - g - h, unit.size),
            bx(b.x0, b.y0 - g - h, unit.size),
            bx(b.x1, b.y0 - g - h, unit.size),
            bx(b.x0 + g, b.y0 + g, unit.size),
        ],
        LabelKind::BoxData => {
            let tag = sibling(ground, unit.owner, |k| k == LabelKind::BoxTag).map_or(0, |(_, th)| th);
            // Line under line: the box's data texts before this one (by ID) stand above it.
            let me = ground.page.labels[unit.members[0].0].id;
            let over: Tick = ground
                .page
                .labels
                .iter()
                .filter(|l| l.owner == unit.owner && l.kind == LabelKind::BoxData && l.id < me)
                .map(|l| l.size.1 + g)
                .sum();
            vec![
                bx(b.x0 - g - w, b.y0 + g + over, unit.size),
                bx(b.x1 + g, b.y0 + g + over, unit.size),
                bx(b.x0 + g, b.y0 + 2 * g + tag + over, unit.size),
                // Above the box over its tag, where a box with wires on all sides keeps a free line.
                bx(b.x0, b.y0 - 2 * g - tag - h - over, unit.size),
            ]
        }
        LabelKind::MarkName => {
            let beyond = match first_pin {
                Some((q, Some(Side::Right))) => Some(bx(b.x0 - g - w, q.y - half(h), unit.size)),
                Some((q, Some(Side::Left))) => Some(bx(b.x1 + g, q.y - half(h), unit.size)),
                Some((q, Some(Side::Down))) => Some(bx(q.x - half(w), b.y0 - g - h, unit.size)),
                Some((q, Some(Side::Up))) => Some(bx(q.x - half(w), b.y1 + g, unit.size)),
                _ => None,
            };
            beyond
                .into_iter()
                .chain([
                    bx(c.x - half(w), b.y0 - g - h, unit.size),
                    bx(c.x - half(w), b.y1 + g, unit.size),
                    // Above the arrow, one line higher: past a text that stands right over it.
                    bx(c.x - half(w), b.y0 - 2 * (g + h), unit.size),
                ])
                .collect()
        }
        // Beside the wire past the pin, under it then over it; then the same beyond the arrow, where the wire turns at
        // once; then centred under the arrow and over it, where the mark's name stands beyond it in line.
        LabelKind::MarkXref => match first_pin {
            Some((q, Some(Side::Right))) => vec![
                bx(q.x + g, q.y + g, unit.size),
                bx(q.x + g, q.y - g - h, unit.size),
                bx(b.x0 - g - w, q.y + g, unit.size),
                bx(b.x0 - g - w, q.y - g - h, unit.size),
                bx(c.x - half(w), b.y1 + g, unit.size),
                bx(c.x - half(w), b.y0 - g - h, unit.size),
            ],
            Some((q, Some(Side::Left))) => vec![
                bx(q.x - g - w, q.y + g, unit.size),
                bx(q.x - g - w, q.y - g - h, unit.size),
                bx(b.x1 + g, q.y + g, unit.size),
                bx(b.x1 + g, q.y - g - h, unit.size),
                bx(c.x - half(w), b.y1 + g, unit.size),
                bx(c.x - half(w), b.y0 - g - h, unit.size),
            ],
            Some((q, Some(Side::Up))) => vec![
                bx(q.x + g, q.y - g - h, unit.size),
                bx(q.x - g - w, q.y - g - h, unit.size),
                bx(q.x + g, b.y1 + g, unit.size),
                bx(q.x - g - w, b.y1 + g, unit.size),
            ],
            Some((q, Some(Side::Down))) => vec![
                bx(q.x + g, q.y + g, unit.size),
                bx(q.x - g - w, q.y + g, unit.size),
                bx(q.x + g, b.y0 - g - h, unit.size),
                bx(q.x - g - w, b.y0 - g - h, unit.size),
            ],
            _ => corners(s, unit.size),
        },
        LabelKind::StripTag => vec![
            bx(b.x0 - g - w, c.y - half(h), unit.size),
            bx(c.x - half(w), b.y0 - g - h, unit.size),
            // Above it, ending at its left edge: clear of a wire that leaves the terminal upward.
            bx(b.x0 - g - w, b.y0 - g - h, unit.size),
        ],
        LabelKind::Tag
        | LabelKind::Xref
        | LabelKind::Data
        | LabelKind::ContactList
        | LabelKind::WireNumber
        | LabelKind::CableType
        | LabelKind::CableTag => corners(s, unit.size),
    }
}

/// The size of the first label of `owner` whose kind `pick` takes, as `(w, h)`.
fn sibling(ground: &Ground<'_>, owner: Owner, pick: impl Fn(LabelKind) -> bool) -> Option<(Tick, Tick)> {
    let mut labels: Vec<&Label> = ground
        .page
        .labels
        .iter()
        .filter(|l| l.owner == owner && pick(l.kind))
        .collect();
    labels.sort_by_key(|l| l.id);
    labels.first().map(|l| l.size)
}

/// The places of a cable mark's tag or type text (module documentation).
fn mark_places(ground: &Ground<'_>, unit: &Unit, mark: MarkId) -> Vec<Rect> {
    let Some(m) = ground.page.marks.iter().find(|m| m.id == mark) else {
        return Vec::new();
    };
    let (l, g, size) = (m.line, LABEL_CLEAR, unit.size);
    let (w, h) = size;
    let c = l.centre();
    let tag = unit.kind == LabelKind::CableTag;
    let other = sibling(ground, unit.owner, |k| {
        k == if tag {
            LabelKind::CableType
        } else {
            LabelKind::CableTag
        }
    })
    .unwrap_or((0, 0));
    if m.horizontal {
        let (before, after) = (l.x0 - g - w, l.x1 + g);
        if tag {
            vec![
                bx(after, c.y - half(h), size),
                bx(after, c.y - half(g) - h, size),
                bx(before, c.y - half(g) - h, size),
            ]
        } else {
            vec![
                bx(before, c.y - half(h), size),
                bx(after, c.y + half(g), size),
                bx(before, c.y + half(g), size),
            ]
        }
    } else if tag {
        vec![
            bx(c.x - half(w), l.y1 + g, size),
            bx(c.x - half(w), l.y0 - 2 * g - other.1 - h, size),
        ]
    } else {
        vec![
            bx(c.x - half(w), l.y0 - g - h, size),
            bx(c.x - half(w), l.y1 + 2 * g + other.1, size),
        ]
    }
}

/// The places of a wire's number along its route (§5.2): on each straight stretch at least the text's length and two
/// steps long, longest first: right of a vertical stretch turned to read from the bottom, above a horizontal one;
/// first level with the number of a parallel wire beside it placed before (within [`TRACK_PITCH`](crate::consts)
/// twice), then at the stretch's middle, then at its quarters; then the same on the other side of the line. With no
/// stretch so long, the middle of the longest. Each with whether it is turned.
fn along_wire(route: &[Pt], (w, h): (Tick, Tick), placed: &[(Rect, bool, Seg)]) -> Vec<(Rect, bool)> {
    let g = LABEL_CLEAR;
    let mut stretches: Vec<(usize, Seg)> = geom::runs(&geom::clean(route))
        .into_iter()
        .filter(|(a, b)| a != b && (a.x == b.x || a.y == b.y))
        .enumerate()
        .collect();
    stretches.sort_by_key(|(i, s)| (-geom::length(*s), *i));
    let need = i64::from(w) + 2 * i64::from(TICKS_PER_STEP);
    let long: Vec<Seg> = stretches
        .iter()
        .filter(|(_, s)| geom::length(*s) >= need)
        .map(|(_, s)| *s)
        .collect();
    let chosen: Vec<Seg> = if long.is_empty() {
        stretches.first().map(|(_, s)| vec![*s]).unwrap_or_default()
    } else {
        long
    };
    // The box centred at `along` on a stretch, on the primary side (`far` false) or the other.
    let place = |(a, b): Seg, along: Tick, far: bool| -> (Rect, bool) {
        if a.x == b.x {
            let size = (h, w);
            let x = if far { a.x - g - h } else { a.x + g };
            (bx(x, along - half(w), size), true)
        } else {
            let y = if far { a.y + g } else { a.y - g - h };
            (bx(along - half(w), y, (w, h)), false)
        }
    };
    let mut out: Vec<(Rect, bool)> = Vec::new();
    for far in [false, true] {
        for &s in &chosen {
            let (a, b) = s;
            let vertical = a.x == b.x;
            let (lo, hi) = if vertical {
                (a.y.min(b.y), a.y.max(b.y))
            } else {
                (a.x.min(b.x), a.x.max(b.x))
            };
            let line = if vertical { a.x } else { a.y };
            // Level with a number placed beside it on a parallel stretch.
            let mut level: Vec<(Tick, Tick)> = placed
                .iter()
                .filter(|(_, turned, _)| *turned == vertical)
                .filter_map(|(r, _, (pa, _))| {
                    let other = if vertical { pa.x } else { pa.y };
                    let gap = (other - line).abs();
                    let centre = if vertical { r.centre().y } else { r.centre().x };
                    (gap > 0
                        && gap <= 2 * crate::consts::TRACK_PITCH
                        && centre - half(w) >= lo
                        && centre + half(w) <= hi)
                        .then_some((gap, centre))
                })
                .collect();
            level.sort_unstable();
            out.extend(level.into_iter().map(|(_, c)| place(s, c, far)));
            let len = hi - lo;
            let margin = half(w) + TICKS_PER_STEP;
            let clamp = |c: Tick| {
                if hi - margin < lo + margin {
                    lo + half(len)
                } else {
                    c.clamp(lo + margin, hi - margin)
                }
            };
            for (n, d) in [(1, 2), (1, 4), (3, 4)] {
                let c = lo + Tick::try_from(i64::from(len) * n / d).unwrap_or(0);
                out.push(place(s, clamp(c), far));
            }
        }
    }
    out
}

/// The labels of `page` where the text placer sets them (module documentation), by label ID; a label without a place
/// (an owner not on the page, a wire without a route and no place given) is left out.
pub fn settle_labels(page: &DraftPage) -> Vec<Settled> {
    let ground = Ground::of(page);
    let mut units = units(&ground);
    stagger(&mut units);
    // The order of §5.3.
    let mut order: Vec<usize> = (0..units.len()).collect();
    let key = |u: &Unit| {
        let at = match u.owner {
            Owner::Part(p) => ground.parts.get(&p).map(|p| p.at),
            Owner::Wire(w) => page
                .wires
                .iter()
                .find(|x| x.id == w)
                .and_then(|x| x.route.iter().min_by_key(|p| (p.y, p.x)).copied()),
            Owner::Mark(m) => page.marks.iter().find(|x| x.id == m).map(|x| x.line.top_left()),
        }
        .or_else(|| u.given.first().map(|r| r.top_left()))
        .unwrap_or_default();
        (u.kind.priority(), at.y, at.x, page.labels[u.members[0].0].id)
    };
    order.sort_by_key(|&i| key(&units[i]));
    let mut placed: Vec<Option<(Rect, u32, bool)>> = vec![None; units.len()];
    let mut lists: Vec<Vec<(Rect, bool)>> = vec![Vec::new(); units.len()];
    let mut grid = Buckets::default();
    // The first places the adapters gave: a label that has to leave its own does not take another's.
    let mut firsts = Buckets::default();
    for (u, unit) in units.iter().enumerate() {
        if let Some(r) = unit.given.first() {
            firsts.insert(u, *r);
        }
    }
    let mut numbers: Vec<(Rect, bool, Seg)> = Vec::new();
    let mut clashing: Vec<usize> = Vec::new();
    for &u in &order {
        let mut list = candidates(&ground, &units[u], &numbers);
        if list.is_empty() {
            continue;
        }
        // Line under line: a text whose author's place lies on a text of its own part placed before it (a turned
        // symbol sets them on one row) has the place under that text second.
        if let (Some(first), Owner::Part(_)) = (units[u].given.first(), units[u].owner)
            && !matches!(units[u].kind, LabelKind::PinNumber(_))
            && let Some(bottom) = (0..units.len())
                .filter(|&v| v != u && units[v].owner == units[u].owner)
                .filter_map(|v| placed[v].map(|(r, _, _)| r))
                .filter(|r| geom::texts_meet(*r, *first) && !geom::stacked(*r, *first))
                .map(|r| r.y1)
                .max()
        {
            let under = (bx(first.x0, bottom + LABEL_CLEAR, first.size()), false);
            if !list.contains(&under) {
                list.insert(1.min(list.len()), under);
            }
        }
        let clear = list.iter().enumerate().position(|(k, (r, _))| {
            let at_first = authors(&units[u], k);
            ground.clear(units[u].owner, *r, (bodies(&units[u], at_first), at_first))
                && labels_clear(&units, &placed, &grid, u, (*r, at_first))
                && (at_first || reserved(&units, &placed, &firsts, u, *r) == 0)
        });
        let k = clear.unwrap_or_else(|| {
            (0..list.len())
                .min_by_key(|&k| {
                    let taken = if authors(&units[u], k) {
                        0
                    } else {
                        reserved(&units, &placed, &firsts, u, list[k].0)
                    };
                    let cost = unit_cost(&ground, &units, &placed, &grid, u, list[k].0, k);
                    (cost + taken as i64 * LABEL_CLASH * i64::from(TICKS_PER_STEP), k)
                })
                .unwrap_or(0)
        });
        if clear.is_none() {
            clashing.push(u);
        }
        let (r, turned) = list[k];
        placed[u] = Some((r, k as u32, turned));
        grid.insert(u, r);
        if units[u].along
            && let Owner::Wire(w) = units[u].owner
            && let Some(s) = nearest_stretch(page, w, r)
        {
            numbers.push((r, turned, s));
        }
        lists[u] = list;
    }
    repair(&ground, &units, &lists, &clashing, &mut placed, &mut grid);
    let mut out: Vec<Settled> = Vec::new();
    for (u, unit) in units.iter().enumerate() {
        let Some((r, rank, turned)) = placed[u] else {
            continue;
        };
        for &(i, offset) in &unit.members {
            let rect = if unit.members.len() == 1 {
                r
            } else {
                Rect::sized(r.top_left().plus(offset), page.labels[i].size)
            };
            out.push(Settled {
                id: page.labels[i].id,
                rect,
                rank,
                turned,
            });
        }
    }
    out.sort_by_key(|s| s.id);
    out
}

/// The stretch of wire `w`'s route that the number at `r` stands beside: the nearest one.
fn nearest_stretch(page: &DraftPage, w: WireId, r: Rect) -> Option<Seg> {
    let wire = page.wires.iter().find(|x| x.id == w)?;
    let c = r.centre();
    geom::runs(&geom::clean(&wire.route))
        .into_iter()
        .min_by_key(|(a, b)| {
            let box_ = Rect::new(*a, *b);
            let dx = (box_.x0 - c.x).max(c.x - box_.x1).max(0);
            let dy = (box_.y0 - c.y).max(c.y - box_.y1).max(0);
            i64::from(dx) + i64::from(dy)
        })
}

/// Whether a unit at a place keeps clear of the other parts' bodies: every label but a pin's number at its author's
/// place, which belongs to its pin wherever the author drew the symbol.
fn bodies(unit: &Unit, at_first: bool) -> bool {
    !(at_first && matches!(unit.kind, LabelKind::PinNumber(_)))
}

/// The first places of the units not yet placed that `r` comes within [`LABEL_CLEAR`] of.
fn reserved(
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    firsts: &Buckets,
    u: usize,
    r: Rect,
) -> usize {
    let g = r.grown(LABEL_CLEAR);
    firsts
        .near(g)
        .into_iter()
        .filter(|&v| v != u && placed[v].is_none() && units[v].given.first().is_some_and(|f| f.overlaps(g)))
        .count()
}

/// Whether place `k` of a unit is the one its adapter gave first: its author's.
fn authors(unit: &Unit, k: usize) -> bool {
    k == 0 && !unit.given.is_empty()
}

/// Whether two placed units of one owner stand where each one's author put it, one over the other or one of them a
/// pin's number: the symbol author's layout, and they do not count against each other (their boxes may overlap by the
/// fonts' line gaps; two texts that the author's layout, turned, sets on one row do count).
fn both_authors(
    units: &[Unit],
    (u, at_u, r): (usize, bool, Rect),
    (v, at_v, o): (usize, bool, Rect),
) -> bool {
    at_u && at_v && units[u].owner == units[v].owner && authors_layout((units[u].kind, r), (units[v].kind, o))
}

/// Whether two texts of one owner, both where their author put them, stand as the author's layout: one over the other,
/// or one of them a pin's number.
pub(crate) fn authors_layout((a, r): (LabelKind, Rect), (b, o): (LabelKind, Rect)) -> bool {
    let pin = |k: LabelKind| matches!(k, LabelKind::PinNumber(_));
    pin(a) || pin(b) || geom::stacked(r, o)
}

/// Whether `r` (its unit's author's place or not) keeps [`LABEL_CLEAR`] from every placed label but unit `u`. At its
/// author's place it only must not lie on a label of its own part, and not even that where both stand at their
/// author's places as the author laid them out ([`both_authors`]).
fn labels_clear(
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    grid: &Buckets,
    u: usize,
    (r, at_first): (Rect, bool),
) -> bool {
    let g = r.grown(LABEL_CLEAR);
    !grid.near(g).into_iter().any(|v| {
        let Some((o, k, _)) = placed[v] else { return false };
        if v == u || both_authors(units, (u, at_first, r), (v, authors(&units[v], k as usize), o)) {
            return false;
        }
        if at_first && units[v].owner == units[u].owner {
            geom::texts_meet(o, r)
        } else {
            o.overlaps(g)
        }
    })
}

/// What unit `u` costs at `r`, `k` places down its list, against the ground and the labels placed but itself.
fn unit_cost(
    ground: &Ground<'_>,
    units: &[Unit],
    placed: &[Option<(Rect, u32, bool)>],
    grid: &Buckets,
    u: usize,
    r: Rect,
    k: usize,
) -> i64 {
    let step = i64::from(TICKS_PER_STEP);
    let on_labels = grid
        .near(r)
        .into_iter()
        .filter(|&v| {
            v != u
                && placed[v].is_some_and(|(o, vk, _)| {
                    !both_authors(
                        units,
                        (u, authors(&units[u], k), r),
                        (v, authors(&units[v], vk as usize), o),
                    ) && geom::texts_meet(o, r)
                })
        })
        .count() as i64;
    ground.cost(units[u].owner, r) + on_labels * LABEL_CLASH * step + k as i64 * LABEL_RANK * step
}

/// The labels' cost on the page, every placed unit at its place.
fn total(ground: &Ground<'_>, units: &[Unit], placed: &[Option<(Rect, u32, bool)>], grid: &Buckets) -> i64 {
    (0..units.len())
        .filter_map(|u| placed[u].map(|(r, k, _)| unit_cost(ground, units, placed, grid, u, r, k as usize)))
        .sum()
}

/// The one repair round of §5.3.
fn repair(
    ground: &Ground<'_>,
    units: &[Unit],
    lists: &[Vec<(Rect, bool)>],
    clashing: &[usize],
    placed: &mut [Option<(Rect, u32, bool)>],
    grid: &mut Buckets,
) {
    for &u in clashing {
        let Some((ur, _, _)) = placed[u] else { continue };
        let under: Vec<usize> = grid
            .near(ur)
            .into_iter()
            .filter(|&v| v != u && placed[v].is_some_and(|(o, _, _)| o.overlaps(ur)))
            .collect();
        for v in under {
            let (Some((vr, vk, vt)), Some(uplace)) = (placed[v], placed[u]) else {
                continue;
            };
            let before = total(ground, units, placed, grid);
            let Some(nk) = (0..lists[v].len()).find(|&k| {
                k != vk as usize
                    && ground.clear(
                        units[v].owner,
                        lists[v][k].0,
                        (bodies(&units[v], authors(&units[v], k)), authors(&units[v], k)),
                    )
                    && labels_clear(units, placed, grid, v, (lists[v][k].0, authors(&units[v], k)))
            }) else {
                continue;
            };
            let (nr, nt) = lists[v][nk];
            grid.remove(v, vr);
            placed[v] = Some((nr, nk as u32, nt));
            grid.insert(v, nr);
            // The label it stood on takes its first clear place now.
            if let Some(uk) = (0..lists[u].len()).find(|&k| {
                ground.clear(
                    units[u].owner,
                    lists[u][k].0,
                    (bodies(&units[u], authors(&units[u], k)), authors(&units[u], k)),
                ) && labels_clear(units, placed, grid, u, (lists[u][k].0, authors(&units[u], k)))
            }) {
                grid.remove(u, uplace.0);
                placed[u] = Some((lists[u][uk].0, uk as u32, lists[u][uk].1));
                grid.insert(u, lists[u][uk].0);
            }
            if total(ground, units, placed, grid) < before {
                break;
            }
            // Not better: both go back.
            if let Some((r, _, _)) = placed[u] {
                grid.remove(u, r);
            }
            grid.remove(v, nr);
            placed[u] = Some(uplace);
            placed[v] = Some((vr, vk, vt));
            grid.insert(u, uplace.0);
            grid.insert(v, vr);
        }
    }
}

/// The places of a unit, each once, in order: the adapter's, the kind's, and for the two-rows rule one line lower
/// second.
fn candidates(ground: &Ground<'_>, unit: &Unit, numbers: &[(Rect, bool, Seg)]) -> Vec<(Rect, bool)> {
    let mut list: Vec<(Rect, bool)> = unit.given.iter().map(|r| (*r, false)).collect();
    // A given place that leaves the drawing area, moved in by the least distance, to stand LABEL_CLEAR off the frame's
    // line as a text beside a line does.
    let area = ground.page.area;
    let into = |r: Rect| {
        let dx = (area.x0 + LABEL_CLEAR - r.x0).max(0) - (r.x1 - area.x1 + LABEL_CLEAR).max(0);
        let dy = (area.y0 + LABEL_CLEAR - r.y0).max(0) - (r.y1 - area.y1 + LABEL_CLEAR).max(0);
        r.moved(Pt::new(dx, dy))
    };
    let moved_in: Vec<(Rect, bool)> = unit
        .given
        .iter()
        .filter(|r| {
            !area.holds(**r) && r.w() + 2 * LABEL_CLEAR <= area.w() && r.h() + 2 * LABEL_CLEAR <= area.h()
        })
        .map(|r| (into(*r), false))
        .collect();
    list.extend(moved_in);
    if unit.stagger
        && let Some(first) = unit.given.first()
    {
        list.push((first.moved(Pt::new(0, first.h() + LABEL_CLEAR)), false));
    }
    match unit.owner {
        Owner::Part(p) => {
            if let Some(part) = ground.parts.get(&p) {
                let shape = Shape::of(ground, part);
                list.extend(part_places(ground, unit, &shape).into_iter().map(|r| (r, false)));
            }
        }
        Owner::Mark(m) => list.extend(mark_places(ground, unit, m).into_iter().map(|r| (r, false))),
        Owner::Wire(w) if unit.along => {
            if let Some(wire) = ground.page.wires.iter().find(|x| x.id == w) {
                list.extend(along_wire(&wire.route, unit.size, numbers));
            }
        }
        Owner::Wire(_) => {}
    }
    let mut seen: BTreeSet<(Rect, bool)> = BTreeSet::new();
    list.retain(|c| seen.insert(*c));
    list
}

/// Whether a kind joins the block of its side of its part: every text of a part but a pin's number.
fn blocks(kind: LabelKind) -> bool {
    !matches!(kind, LabelKind::PinNumber(_))
}

/// The units of the page: the blocks of the block rule and every other label alone.
fn units(ground: &Ground<'_>) -> Vec<Unit> {
    let page = ground.page;
    let mut order: Vec<usize> = (0..page.labels.len()).collect();
    order.sort_by_key(|&i| page.labels[i].id);
    let origin = |owner: Owner| match owner {
        Owner::Part(p) => ground.parts.get(&p).map(|p| p.at),
        Owner::Mark(m) => page.marks.iter().find(|x| x.id == m).map(|x| x.line.top_left()),
        Owner::Wire(_) => Some(Pt::default()),
    };
    let alone = |i: usize| {
        let l = &page.labels[i];
        let (given, along) = match &l.places {
            Places::At(at) => (
                origin(l.owner).map_or_else(Vec::new, |o| {
                    at.iter().map(|p| Rect::sized(o.plus(*p), l.size)).collect()
                }),
                false,
            ),
            Places::AlongWire => (Vec::new(), true),
        };
        Unit {
            members: vec![(i, Pt::default())],
            size: l.size,
            kind: l.kind,
            owner: l.owner,
            given,
            along,
            stagger: false,
        }
    };
    // The block candidates by part and side: their first places on the page.
    // Per part and side: (priority, top, ID, index, first place) of each text there.
    type Side = (u8, Tick, LabelId, usize, Rect);
    let mut sides: BTreeMap<(PartId, i8), Vec<Side>> = BTreeMap::new();
    let mut out: Vec<Unit> = Vec::new();
    for &i in &order {
        let l = &page.labels[i];
        let first = match (&l.places, l.owner) {
            (Places::At(at), Owner::Part(p)) if blocks(l.kind) => at.first().and_then(|a| {
                let part = ground.parts.get(&p)?;
                Some((p, Rect::sized(part.at.plus(*a), l.size), part.bounds()))
            }),
            _ => None,
        };
        match first {
            Some((p, r, body)) => {
                let cx = body.centre().x;
                let side = if r.x1 <= cx {
                    -1
                } else if r.x0 >= cx {
                    1
                } else {
                    0
                };
                sides
                    .entry((p, side))
                    .or_default()
                    .push((l.kind.priority(), r.y0, l.id, i, r));
            }
            None => out.push(alone(i)),
        }
    }
    for (_, mut members) in sides {
        members.sort();
        let mut block: Vec<(usize, Rect)> = Vec::new();
        let mut rest: Vec<usize> = Vec::new();
        for (_, _, _, i, r) in members {
            let joins = match block.iter().map(|(_, b)| *b).reduce(Rect::union) {
                None => true,
                // Line under line: beside none of the block, under or over it with at most a quarter step between.
                Some(u) => {
                    let line = TICKS_PER_STEP / 4;
                    !block.iter().any(|(_, b)| geom::texts_meet(*b, r))
                        && r.x0 < u.x1
                        && u.x0 < r.x1
                        && r.y0 <= u.y1 + line
                        && u.y0 <= r.y1 + line
                }
            };
            if joins {
                block.push((i, r));
            } else {
                rest.push(i);
            }
        }
        if block.len() == 1 {
            out.push(alone(block[0].0));
        } else if let Some(union) = block.iter().map(|(_, b)| *b).reduce(Rect::union) {
            let lead = &page.labels[block[0].0];
            out.push(Unit {
                members: block
                    .iter()
                    .map(|(i, b)| (*i, b.top_left().minus(union.top_left())))
                    .collect(),
                size: union.size(),
                kind: lead.kind,
                owner: lead.owner,
                given: vec![union],
                along: false,
                stagger: false,
            });
        }
        out.extend(rest.into_iter().map(alone));
    }
    out
}

/// The two-rows rule (module documentation).
fn stagger(units: &mut [Unit]) {
    let rows = |k: LabelKind| {
        matches!(
            k,
            LabelKind::Tag | LabelKind::BoxTag | LabelKind::TerminalName | LabelKind::ContactList
        )
    };
    let firsts: Vec<Option<(LabelKind, Owner, Rect)>> = units
        .iter()
        .map(|u| {
            u.given
                .first()
                .filter(|_| rows(u.kind))
                .map(|r| (u.kind, u.owner, *r))
        })
        .collect();
    for (i, unit) in units.iter_mut().enumerate() {
        let Some((kind, owner, r)) = firsts[i] else {
            continue;
        };
        let reach = r.grown(LABEL_CLEAR);
        unit.stagger = firsts.iter().enumerate().any(|(j, o)| {
            j != i
                && o.is_some_and(|(k, w, s)| {
                    k == kind && w != owner && s.y0 == r.y0 && s.h() == r.h() && s.overlaps(reach)
                })
        });
    }
}
