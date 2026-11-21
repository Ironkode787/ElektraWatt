// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! How a mark is drawn from its wires (docs/design/06a-drafting.md §3.10): a cable mark's line reaches past the outer
//! crossings of its cores and carries a tick and a core name at each, a marker's text stands beyond its line's end, and
//! a wire's own marker stands at the middle of the wire's longest segment. So a mark is where its wires' routes put it,
//! and [`draw_marks`] sets its line and its fixed texts from them (a mark with a [`Follow`]): the adapter on the page as
//! stored, the measure and the placer on a plan's page, so that the plan is judged on the page the scene will draw.
//!
//! A wire's drawn line is its route, or for a wire without one the line it is drawn with dashed
//! ([`crate::unrouted_line`]). Its crossing with a mark's line is the one nearest the mark's point along the line, a
//! point on a slanted segment by its proportion (as the scene finds it in nm).

use std::collections::BTreeMap;

use crate::model::{Anchor, DraftPage, End, FixedText, Follow, Mark, MarkAt, Pt, Rect, Tick, WireId};

/// Where the anchors of a mark stand on the page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Anchors {
    /// The mark's point.
    pub point: Pt,
    /// The first and the last crossing along its line (the point where it crosses no wire).
    pub first: Pt,
    /// See `first`.
    pub last: Pt,
    /// Each wire's crossing with the line, by wire; a wire the line misses has none.
    pub crossings: BTreeMap<WireId, Pt>,
}

impl Anchors {
    /// Where `anchor` stands; `None` for the crossing of a wire the line misses.
    pub fn at(&self, anchor: Anchor) -> Option<Pt> {
        match anchor {
            Anchor::Point => Some(self.point),
            Anchor::Crossing(w) => self.crossings.get(&w).copied(),
            Anchor::First => Some(self.first),
            Anchor::Last => Some(self.last),
        }
    }
}

/// The line wire `w` is drawn with on `page`: its route, or the dashed line of a wire without one.
fn drawn(page: &DraftPage, w: WireId) -> Option<Vec<Pt>> {
    let wire = page.wires.iter().find(|x| x.id == w)?;
    if !wire.route.is_empty() {
        return Some(wire.route.clone());
    }
    let end = |e: End| match e {
        End::Pin(pin) => page.parts.iter().find_map(|p| {
            let q = p.pins.iter().find(|q| q.id == pin)?;
            let at = p.at.plus(q.at);
            Some(((i64::from(at.x), i64::from(at.y)), q.side))
        }),
        End::Junction(j) => page
            .junctions
            .iter()
            .find(|x| x.id == j)
            .map(|x| ((i64::from(x.at.x), i64::from(x.at.y)), None)),
    };
    let (a, b) = (end(wire.ends[0])?, end(wire.ends[1])?);
    crate::route::unrouted_line(a, b)
        .into_iter()
        .map(|(x, y)| Some(Pt::new(Tick::try_from(x).ok()?, Tick::try_from(y).ok()?)))
        .collect()
}

/// The middle of the longest segment of `line` (the first of equal ones).
fn middle(line: &[Pt]) -> Option<Pt> {
    let mut best: Option<(i128, Pt)> = None;
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (i128::from(b.x) - i128::from(a.x), i128::from(b.y) - i128::from(a.y));
        let len = dx * dx + dy * dy;
        if best.is_none_or(|(l, _)| len > l) {
            let mid = |p: Tick, q: Tick| p + (q - p).div_euclid(2);
            best = Some((len, Pt::new(mid(a.x, b.x), mid(a.y, b.y))));
        }
    }
    best.map(|(_, p)| p)
}

/// The coordinate along a mark's line (horizontal: `x`) at which `line` crosses it, of its crossings the nearest to
/// `at`; `None` where it does not cross.
fn crossing(line: &[Pt], at: Pt, horizontal: bool) -> Option<Tick> {
    let split = |p: Pt| {
        if horizontal {
            (i128::from(p.x), i128::from(p.y))
        } else {
            (i128::from(p.y), i128::from(p.x))
        }
    };
    let (target, axis) = split(at);
    let mut best: Option<i128> = None;
    for w in line.windows(2) {
        let ((a_al, a_ac), (b_al, b_ac)) = (split(w[0]), split(w[1]));
        if a_ac == b_ac || axis < a_ac.min(b_ac) || axis > a_ac.max(b_ac) {
            continue;
        }
        let x = a_al + (axis - a_ac) * (b_al - a_al) / (b_ac - a_ac);
        if best.is_none_or(|b| (x - target).abs() < (b - target).abs()) {
            best = Some(x);
        }
    }
    best.and_then(|x| Tick::try_from(x).ok())
}

/// Where the anchors of mark `m` stand on `page` (module documentation); `None` for a mark without a [`Follow`] or a
/// wire's own marker whose wire is not drawn.
pub fn anchors(page: &DraftPage, m: &Mark) -> Option<Anchors> {
    let follow: &Follow = m.follow.as_ref()?;
    let point = match follow.at {
        MarkAt::Point(p) => p,
        MarkAt::Middle => middle(&drawn(page, *m.wires.first()?)?)?,
    };
    let on_line = |v: Tick| {
        if m.horizontal {
            Pt::new(v, point.y)
        } else {
            Pt::new(point.x, v)
        }
    };
    let crossings: BTreeMap<WireId, Pt> = m
        .wires
        .iter()
        .filter_map(|w| Some((*w, on_line(crossing(&drawn(page, *w)?, point, m.horizontal)?))))
        .collect();
    let along = |p: &Pt| if m.horizontal { p.x } else { p.y };
    let mine = along(&point);
    let first = crossings.values().map(along).min().unwrap_or(mine);
    let last = crossings.values().map(along).max().unwrap_or(mine);
    Some(Anchors {
        point,
        first: on_line(first),
        last: on_line(last),
        crossings,
    })
}

/// Every mark of `page` that follows its wires ([`Follow`]) drawn where their routes put it: its line (from the first
/// crossing to the last, with every piece) and its fixed texts (its text pieces, in place of the fixed texts it owned).
pub fn draw_marks(page: &mut DraftPage) {
    let mut drawn: Vec<(usize, Rect, Vec<Rect>)> = Vec::new();
    for (k, m) in page.marks.iter().enumerate() {
        let (Some(follow), Some(a)) = (&m.follow, anchors(page, m)) else {
            continue;
        };
        let mut line = Rect::new(a.first, a.last);
        let mut texts = Vec::new();
        for piece in &follow.pieces {
            let Some(at) = a.at(piece.anchor) else { continue };
            let r = piece.rect.moved(at);
            line = line.union(r);
            if piece.text {
                texts.push(r);
            }
        }
        drawn.push((k, line, texts));
    }
    if drawn.is_empty() {
        return;
    }
    for (k, line, texts) in drawn {
        let owner = page.marks[k].id.0;
        page.marks[k].line = line;
        page.fixed_texts.retain(|t| t.owner != owner);
        page.fixed_texts
            .extend(texts.into_iter().map(|rect| FixedText { owner, rect }));
    }
    page.fixed_texts.sort_by_key(|t| (t.owner, t.rect));
}

/// Where a plan moves mark `m` from (§3.10, `Plan::marks`): its point, or the corner of a mark that stands as it is;
/// `None` for a wire's own marker, which its wire's route sets.
pub(crate) fn mark_point(m: &Mark) -> Option<Pt> {
    match m.follow.as_ref().map(|f| f.at) {
        Some(MarkAt::Point(p)) => Some(p),
        Some(MarkAt::Middle) => None,
        None => Some(m.line.top_left()),
    }
}

/// Mark `m` moved by `d` (a plan's move, §3.10): its point, or the line of a mark that stands as it is. A mark that
/// follows its wires is drawn again by [`draw_marks`].
pub(crate) fn move_mark(m: &mut Mark, d: Pt) {
    match m.follow.as_mut().map(|f| &mut f.at) {
        Some(MarkAt::Point(p)) => *p = p.plus(d),
        Some(MarkAt::Middle) => {}
        None => m.line = m.line.moved(d),
    }
}

/// `page` with its marks drawn ([`draw_marks`]), borrowed as it is where none follows its wires.
pub(crate) fn with_marks(page: &DraftPage) -> std::borrow::Cow<'_, DraftPage> {
    if page.marks.iter().any(|m| m.follow.is_some()) {
        let mut out = page.clone();
        draw_marks(&mut out);
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(page)
    }
}
