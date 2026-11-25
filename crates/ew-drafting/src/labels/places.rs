// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The places of each kind of label (docs/design/06a-drafting.md §5.2): a unit's places in order, the corners of a
//! part, the places of each kind owned by a part, of a cable mark's texts and of a wire's number.

use std::collections::BTreeSet;

use crate::consts::{HALF, LABEL_CLEAR, NUMBER_AT, NUMBER_LEVEL, NUMBER_ROOM, TICKS_PER_STEP};
use crate::geom::{self, Seg};
use crate::model::{Label, LabelKind, MarkId, Owner, Places, Pt, Rect, Side, Tick};

use super::{Ground, Shape, Unit};

/// Whether `r` lies in the way out of a pin at `q` that a wire leaves along `side`: on the ray from the pin, within
/// [`LABEL_CLEAR`] of it.
fn in_way(r: Rect, q: Pt, side: Side) -> bool {
    let g = LABEL_CLEAR;
    let across = |lo: Tick, hi: Tick, v: Tick| lo - g < v && v < hi + g;
    match side {
        Side::Up => r.y0 < q.y && across(r.x0, r.x1, q.x),
        Side::Down => r.y1 > q.y && across(r.x0, r.x1, q.x),
        Side::Left => r.x0 < q.x && across(r.y0, r.y1, q.y),
        Side::Right => r.x1 > q.x && across(r.y0, r.y1, q.y),
    }
}

/// A box of size `size` with its top-left corner at `(x, y)`.
/// The place of a mark's name beyond its arrow, past its body `b`, in line with its pin at `q` that leaves along `side`:
/// left of the body where the pin leaves right, and so on; none for a pin without a side.
pub(super) fn mark_name_beyond(b: Rect, (q, side): (Pt, Option<Side>), (w, h): (Tick, Tick)) -> Option<Rect> {
    let g = LABEL_CLEAR;
    match side? {
        Side::Right => Some(bx(b.x0 - g - w, q.y - half(h), (w, h))),
        Side::Left => Some(bx(b.x1 + g, q.y - half(h), (w, h))),
        Side::Down => Some(bx(q.x - half(w), b.y0 - g - h, (w, h))),
        Side::Up => Some(bx(q.x - half(w), b.y1 + g, (w, h))),
    }
}

pub(super) fn bx(x: Tick, y: Tick, size: (Tick, Tick)) -> Rect {
    Rect::sized(Pt::new(x, y), size)
}

fn half(v: Tick) -> Tick {
    v.div_euclid(2)
}

/// The corners of a part ([`candidates`]), for a unit `w` by `h` whose first line is `line` high, of a part whose
/// tag's first place has its top at `tag_top`.
fn corners(s: &Shape, (w, h): (Tick, Tick), (line, tag_top): (Tick, Tick)) -> Vec<Rect> {
    let (b, g, size) = (s.body, LABEL_CLEAR, (w, h));
    let cx = b.centre().x;
    let above = bx(cx - half(w), b.y0 - g - h, size);
    let below = bx(cx - half(w), b.y1 + g, size);
    // Centred below, one line lower: past a text that stands right under the body. One line, not the block's height:
    // a block of several lines goes no further from its part than one line past that text.
    let lower = bx(cx - half(w), b.y1 + 2 * g + line, size);
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
        // The upper corners half a step lower: clear of a wire that runs along the level of the upper pins.
        out.push(bx(b.x0 - g - w, b.y0 + HALF, size));
        out.push(bx(s.right + g, b.y0 + HALF, size));
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
            // Centred above, past the tag over the body: where wires run under a part lying flat (a switch across the
            // phases of a supply), its data stand over its tag.
            bx(cx - half(w), tag_top.min(b.y0) - g - h, size),
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

/// The places of the kind of a unit owned by a part (§5.2), after the adapter's:
///
/// | Kind | Places after the adapter's |
/// |---|---|
/// | pin number | right of the pin's line at its outer end, left of it (above and below for a pin leaving sideways) |
/// | tag, cross-reference, data, contact list | the corners |
/// | function text | centred below, right of the body at its middle, then the corners |
/// | tag of a drawn box | above the upper left corner ending at the left edge; above the upper edge from the corner; above the upper right corner; inside at the upper left |
/// | data of a drawn box | left of the box under its top, under the box's data before it; right of it; inside under the tag; above the box over its tag |
/// | name of a mark | in line with the pin beyond the body (the side away from the wire); beyond the body over the pin's line, then under it (a mark pin to pin with its neighbour, nearer than a name is tall); centred above; centred below; centred above one line higher |
/// | cross-reference of a mark | beside the wire past the pin, under it then over it; the same past the number of a pin the mark is set on; the same beyond the arrow; centred under the arrow, then over it; under and over its name where the name stands (`name`), aligned to it on the arrow's side |
/// | name of a terminal | right below its centre, left below it (with wires across it: left below, left above); beside the body, left then right |
/// | strip designation | left of the terminal at its middle; centred above it; the same past the terminal's name; above it, ending at its left edge |
/// | wire number | along the wire ([`along_wire`]) |
/// | cable tag and type | the type before the mark's line and the tag after it; both after it (tag over type); both before it |
fn part_places(ground: &Ground<'_>, unit: &Unit, s: &Shape, name: Option<Rect>) -> Vec<Rect> {
    let (b, g) = (s.body, LABEL_CLEAR);
    let (w, h) = unit.size;
    let c = b.centre();
    let first_pin = s.pins.first().copied();
    let line = ground.page.labels[unit.members[0].0].size.1;
    let tag_top = first_place(ground, unit.owner, LabelKind::Tag).map_or(b.y0 - g - line, |r| r.y0);
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
            out.extend(corners(s, unit.size, (line, tag_top)));
            out
        }
        LabelKind::BoxTag => {
            // A place in the way out of a pin a wire leaves (over a top pin, beside a side pin) comes after the places
            // clear of every such way, so that the wires and tubes into the pins need not go round the caption.
            let mut out = vec![
                bx(b.x0 - w, b.y0 - g - h, unit.size),
                bx(b.x0, b.y0 - g - h, unit.size),
                bx(b.x1, b.y0 - g - h, unit.size),
                bx(b.x0 + g, b.y0 + g, unit.size),
            ];
            out.sort_by_key(|r| s.ways.iter().any(|(q, side)| in_way(*r, *q, *side)));
            out
        }
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
            let beyond = first_pin.and_then(|pin| mark_name_beyond(b, pin, unit.size));
            // Beyond the body over the pin's line and under it, for a pin that leaves sideways: of two marks nearer
            // than a name is tall, one name stands in line and the other one steps aside.
            let off_line = beyond
                .zip(first_pin)
                .filter(|(_, (_, side))| matches!(side, Some(Side::Left | Side::Right)));
            beyond
                .into_iter()
                .chain(
                    off_line.into_iter().flat_map(|(r, (q, _))| {
                        [bx(r.x0, q.y - g - h, unit.size), bx(r.x0, q.y + g, unit.size)]
                    }),
                )
                .chain([
                    bx(c.x - half(w), b.y0 - g - h, unit.size),
                    bx(c.x - half(w), b.y1 + g, unit.size),
                    // Above the arrow, one line higher: past a text that stands right over it.
                    bx(c.x - half(w), b.y0 - 2 * (g + h), unit.size),
                ])
                .collect()
        }
        // Beside the wire past the pin, under it then over it, and the same past the number of a pin the mark is set
        // on; then the same beyond the arrow, where the wire turns at once; then centred under the arrow and over it,
        // where the mark's name stands beyond it in line; then under and over the name where it stands, read from its
        // box (placed, else at its first place), aligned to it on the arrow's side.
        LabelKind::MarkXref => {
            let name = name.or_else(|| first_place(ground, unit.owner, LabelKind::MarkName));
            let by_name = |toward_left: bool| {
                name.into_iter().flat_map(move |n| {
                    let x = if toward_left { n.x1 - w } else { n.x0 };
                    [bx(x, n.y1 + g, unit.size), bx(x, n.y0 - g - h, unit.size)]
                })
            };
            match first_pin {
                Some((q, Some(Side::Right))) => {
                    let past = numbers_at(ground, unit.owner, q)
                        .iter()
                        .filter(|r| r.x0 >= q.x)
                        .map(|r| r.x1)
                        .max();
                    let mut out = vec![
                        bx(q.x + g, q.y + g, unit.size),
                        bx(q.x + g, q.y - g - h, unit.size),
                    ];
                    out.extend(
                        past.into_iter()
                            .flat_map(|x| [bx(x + g, q.y + g, unit.size), bx(x + g, q.y - g - h, unit.size)]),
                    );
                    out.extend([
                        bx(b.x0 - g - w, q.y + g, unit.size),
                        bx(b.x0 - g - w, q.y - g - h, unit.size),
                        bx(c.x - half(w), b.y1 + g, unit.size),
                        bx(c.x - half(w), b.y0 - g - h, unit.size),
                    ]);
                    out.extend(by_name(true));
                    out
                }
                Some((q, Some(Side::Left))) => {
                    let past = numbers_at(ground, unit.owner, q)
                        .iter()
                        .filter(|r| r.x1 <= q.x)
                        .map(|r| r.x0)
                        .min();
                    let mut out = vec![
                        bx(q.x - g - w, q.y + g, unit.size),
                        bx(q.x - g - w, q.y - g - h, unit.size),
                    ];
                    out.extend(past.into_iter().flat_map(|x| {
                        [
                            bx(x - g - w, q.y + g, unit.size),
                            bx(x - g - w, q.y - g - h, unit.size),
                        ]
                    }));
                    out.extend([
                        bx(b.x1 + g, q.y + g, unit.size),
                        bx(b.x1 + g, q.y - g - h, unit.size),
                        bx(c.x - half(w), b.y1 + g, unit.size),
                        bx(c.x - half(w), b.y0 - g - h, unit.size),
                    ]);
                    out.extend(by_name(false));
                    out
                }
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
                _ => corners(s, unit.size, (line, tag_top)),
            }
        }
        LabelKind::StripTag => {
            let name = first_place(ground, unit.owner, LabelKind::TerminalName).map_or(b.y0 - g, |r| r.y0);
            vec![
                bx(b.x0 - g - w, c.y - half(h), unit.size),
                bx(c.x - half(w), b.y0 - g - h, unit.size),
                // Centred above, over the terminal's name where it stands over it (a terminal turned a quarter, doc 07
                // §4.1).
                bx(c.x - half(w), name.min(b.y0) - g - h, unit.size),
                // Above it, ending at its left edge: clear of a wire that leaves the terminal upward.
                bx(b.x0 - g - w, b.y0 - g - h, unit.size),
                // Centred below it, then right of it at its middle: where a wire comes to the terminal round its left
                // and over its top, so that the wire need not run through the designation.
                bx(c.x - half(w), b.y1 + g, unit.size),
                bx(b.x1 + g, c.y - half(h), unit.size),
            ]
        }
        LabelKind::Tag
        | LabelKind::Xref
        | LabelKind::Data
        | LabelKind::ContactList
        | LabelKind::WireNumber
        | LabelKind::CableType
        | LabelKind::CableTag => corners(s, unit.size, (line, tag_top)),
    }
}

/// The first places, on the page, of the numbers of the pins of other parts than `owner` that stand on `q`: the pin a
/// mark is set on.
fn numbers_at(ground: &Ground<'_>, owner: Owner, q: Pt) -> Vec<Rect> {
    ground
        .page
        .labels
        .iter()
        .filter_map(|l| {
            let LabelKind::PinNumber(pin) = l.kind else {
                return None;
            };
            let &(at, _, part) = ground.pins.get(&pin)?;
            if at != q || Owner::Part(part) == owner {
                return None;
            }
            let p = ground.parts.get(&part)?;
            match &l.places {
                Places::At(first) => first.first().map(|a| Rect::sized(p.at.plus(*a), l.size)),
                Places::AlongWire => None,
            }
        })
        .collect()
}

/// The first place of `owner`'s first label of kind `kind` on the page, where it has one.
fn first_place(ground: &Ground<'_>, owner: Owner, kind: LabelKind) -> Option<Rect> {
    let Owner::Part(p) = owner else { return None };
    let part = ground.parts.get(&p)?;
    let label = ground
        .page
        .labels
        .iter()
        .filter(|l| l.owner == owner && l.kind == kind)
        .min_by_key(|l| l.id)?;
    match &label.places {
        Places::At(at) => at.first().map(|a| Rect::sized(part.at.plus(*a), label.size)),
        Places::AlongWire => None,
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

/// The places of a cable mark's tag or type text ([`part_places`]).
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
    let need = i64::from(w) + i64::from(NUMBER_ROOM);
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
                    (gap > 0 && gap <= NUMBER_LEVEL && centre - half(w) >= lo && centre + half(w) <= hi)
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
            for (n, d) in NUMBER_AT {
                let c = lo + Tick::try_from(i64::from(len) * n / d).unwrap_or(0);
                out.push(place(s, clamp(c), far));
            }
        }
    }
    out
}

/// The places of a unit, each once, in order: the adapter's, the kind's, and for the two-rows rule one line lower
/// second (§5.2).
///
/// A label's places are the ones its adapter gives ([`Places::At`]: a symbol's slot as its author set it), those of
/// them outside the drawing area moved in to stand [`LABEL_CLEAR`] inside its line (not onto its own part's body),
/// "one line lower" for the second
/// of a row (the two-rows rule), then the places of its kind, each once. For a part whose pins leave up or down (or
/// that has none with a side) the corners are: upper left (right of the box against the body's left edge, its top
/// level with the body's top), upper right (beyond the pins' numbers at their first places), lower left, lower right
/// (the same), centred above (when no wire leaves upward), centred below, above the upper left and upper right
/// corners, centred below one line lower, the upper corners half a step lower; for a part whose pins leave sideways: centred above, centred below, above
/// and below at either end, centred below one line lower, centred above past its tag. Every gap to the owner is
/// [`LABEL_CLEAR`]. `name` is where the name of a mark whose cross-reference `unit` is stands, where it is placed.
pub(super) fn candidates(
    ground: &Ground<'_>,
    unit: &Unit,
    (numbers, name): (&[(Rect, bool, Seg)], Option<Rect>),
) -> Vec<(Rect, bool)> {
    let mut list: Vec<(Rect, bool)> = unit.given.iter().map(|r| (*r, false)).collect();
    // A given place that leaves the drawing area, moved in by the least distance, to stand LABEL_CLEAR off the frame's
    // line as a text beside a line does; not where that sets it on its own part's body, which its author kept it off (a
    // symbol placed near the area's edge: its tag and data go to the kind's places beside it).
    let area = ground.page.area;
    let into = |r: Rect| {
        let dx = (area.x0 + LABEL_CLEAR - r.x0).max(0) - (r.x1 - area.x1 + LABEL_CLEAR).max(0);
        let dy = (area.y0 + LABEL_CLEAR - r.y0).max(0) - (r.y1 - area.y1 + LABEL_CLEAR).max(0);
        r.moved(Pt::new(dx, dy))
    };
    let own = match unit.owner {
        Owner::Part(p) => ground.parts.get(&p).map(|x| x.bounds()),
        _ => None,
    };
    let moved_in: Vec<(Rect, bool)> = unit
        .given
        .iter()
        .filter(|r| {
            !area.holds(**r) && r.w() + 2 * LABEL_CLEAR <= area.w() && r.h() + 2 * LABEL_CLEAR <= area.h()
        })
        .filter(|r| own.is_none_or(|b| r.overlaps(b) || !into(**r).overlaps(b)))
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
                list.extend(
                    part_places(ground, unit, &shape, name)
                        .into_iter()
                        .map(|r| (r, false)),
                );
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
    // Where every place leaves the drawing area, each moved in by the least distance, onto its own part's body too: a
    // text never stands outside the area (§5.3).
    if !list.iter().any(|(r, _)| ground.inside(*r)) {
        let inward: Vec<(Rect, bool)> = list
            .iter()
            .filter(|(r, _)| r.w() + 2 * LABEL_CLEAR <= area.w() && r.h() + 2 * LABEL_CLEAR <= area.h())
            .map(|(r, turned)| (into(*r), *turned))
            .collect();
        list.extend(inward);
    }
    let mut seen: BTreeSet<(Rect, bool)> = BTreeSet::new();
    list.retain(|c| seen.insert(*c));
    list
}
