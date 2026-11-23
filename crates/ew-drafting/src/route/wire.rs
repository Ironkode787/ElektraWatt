// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Wire tool's route between two ends on the router's occupancy and search (docs/design/06a-drafting.md §7.4),
//! and the lines of a wire outside a run: the dashed line of a wire without a route and the placer's plain shape.

use std::collections::BTreeSet;

use ew_ids::Uuid;

use crate::geom;
use crate::model::{DraftPage, End, JunctionId, Owner, PartId, PinId, Pt, Rect, Side, WireId, grid_down};
use crate::search::{self, Ask, Relax, Tip, heading};

use super::Router;

/// An end of the Wire tool's route.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireEnd {
    /// A pin.
    Pin(PinId),
    /// A junction.
    Junction(JunctionId),
    /// A point on a wire, where a junction will be made.
    OnWire(WireId, Pt),
    /// A free point (a fixed bend).
    Point(Pt),
}

/// The Wire tool's route, and whether it runs across a text that cannot step aside.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct WireRoute {
    /// Every vertex from the first end to the second.
    pub route: Vec<Pt>,
    /// Whether it runs across a fixed text.
    pub over_text: bool,
}

/// The tip of a Wire tool's end on the page of `r`.
fn wire_tip(r: &Router, e: WireEnd) -> Option<Tip> {
    match e {
        WireEnd::Pin(p) => r.tip(WireId(Uuid::nil()), End::Pin(p)),
        WireEnd::Junction(j) => r.tip(WireId(Uuid::nil()), End::Junction(j)),
        WireEnd::OnWire(_, at) | WireEnd::Point(at) => Some(Tip::free(at)),
    }
}

/// The router over `page` for a wire not yet drawn, with the parts `without` left out.
fn tool_router(page: &DraftPage, without: &[PartId]) -> Option<Router> {
    let mut page = page.clone();
    if !without.is_empty() {
        let gone: BTreeSet<Uuid> = without.iter().map(|p| p.0).collect();
        page.parts.retain(|p| !without.contains(&p.id));
        page.labels
            .retain(|l| !matches!(l.owner, Owner::Part(p) if gone.contains(&p.0)));
        page.fixed_texts.retain(|t| !gone.contains(&t.owner));
    }
    Router::new(&page, &[])
}

/// Whether the straight line between the Wire tool's ends `a` and `b` crosses nothing on `page` (§7.4): the two ends on
/// one line, a pin end left along its side and entered against it, no turn back after a fixed bend (`arrive`); on the
/// line no pin or junction but at its ends, no wire along it, across it or meeting it but at its ends, no body it runs
/// through or along but an end's own part's (an outline drawn round other parts that holds an end included), no text
/// that cannot step aside, and none along the wire's own route so far (`before`). A pin's stub and the gap round a
/// body close nothing here as they do to the search: a person who draws a rail one step over a row of pins drops each
/// tap down a stub.
fn crosses_nothing(page: &DraftPage, a: &Tip, b: &Tip, arrive: Option<Side>, before: &[Pt]) -> bool {
    let (Some(h), Some(back)) = (heading(a.at, b.at), heading(b.at, a.at)) else {
        return false;
    };
    if a.side.is_some_and(|s| s != h) || b.side.is_some_and(|s| s != back) || arrive == Some(back) {
        return false;
    }
    let line = (a.at, b.at);
    let end = |p: Pt| p == a.at || p == b.at;
    let own = |id: Uuid| [a, b].iter().any(|t| t.part.is_some_and(|(o, _)| o == id));
    let holds_end = |r: Rect| [a.at, b.at].iter().any(|p| r.contains(*p));
    let mut points = page
        .parts
        .iter()
        .flat_map(|p| p.pins.iter().map(|q| p.at.plus(q.at)))
        .chain(page.junctions.iter().map(|j| j.at));
    if points.any(|q| !end(q) && geom::on(q, line)) {
        return false;
    }
    let in_body = page.parts.iter().any(|p| {
        let body = p.bounds();
        let solid = if p.frame_only {
            !holds_end(body)
        } else {
            !p.pins.is_empty() && !own(p.id.0)
        };
        solid && geom::crosses(line, body.grown(1))
    });
    let on_wire = page
        .wires
        .iter()
        .map(|w| w.route.as_slice())
        .chain([before])
        .any(|route| {
            geom::segments(&geom::clean(route))
                .into_iter()
                .any(|s| geom::overlap(line, s).is_some() || geom::meet(line, s).is_some_and(|p| !end(p)))
        });
    let on_text = page
        .fixed_texts
        .iter()
        .any(|t| geom::crosses(line, t.rect.grown(-1)));
    !in_body && !on_wire && !on_text
}

/// The Wire tool's route from `from` to `to` (§7.4): the straight line between them where it crosses nothing
/// ([`crosses_nothing`]), else the search of [`route`](super::route) against the page as drawn, every routed wire fixed, a wire of
/// the same pin's run shared where that is the better way. `before` is the wire's own route so far, ending at a free
/// first end (its fixed bends; empty for a wire from its start): the route goes on from the heading it arrived with and
/// runs along none of it, so that it never runs back over itself (FT-2097). `None` where no route keeps free of faults.
pub fn route_wire(page: &DraftPage, from: WireEnd, to: WireEnd, before: &[Pt]) -> Option<WireRoute> {
    let mut r = tool_router(page, &[])?;
    let arrive = before.windows(2).last().and_then(|w| heading(w[0], w[1]));
    if before.len() >= 2 {
        r.occ.lay(before, true);
    }
    let (a, b) = (wire_tip(&r, from)?, wire_tip(&r, to)?);
    if crosses_nothing(&r.page, &a, &b, arrive, before) {
        return Some(WireRoute {
            route: vec![a.at, b.at],
            over_text: false,
        });
    }
    let found = search::route(&tool_ask(&r, a, b, arrive))?;
    let over_text = across_text(&r, &found.route, true);
    Some(WireRoute {
        route: found.route,
        over_text,
    })
}

/// Whether `route` from `from` to `to` is one the Wire tool's search could take on `page` with the parts `through` left
/// out (their bodies, texts and pins), clear of every other text: the direct route through a free terminal in line.
pub fn wire_clear(
    page: &DraftPage,
    (from, to): (WireEnd, WireEnd),
    arrive: Option<Side>,
    through: &[PartId],
    route: &[Pt],
) -> bool {
    let Some(r) = tool_router(page, through) else {
        return false;
    };
    let (Some(a), Some(b)) = (wire_tip(&r, from), wire_tip(&r, to)) else {
        return false;
    };
    // The direct route keeps clear of every text: a text in its way sends the wire to the search.
    !across_text(&r, route, false) && search::clear(&tool_ask(&r, a, b, arrive), route)
}

/// The Wire tool's search from `from` to `to` on `r`, the route arriving along `arrive`.
fn tool_ask(r: &Router, from: Tip, to: Tip, arrive: Option<Side>) -> Ask<'_> {
    Ask {
        occ: &r.occ,
        from,
        to,
        arrive,
        area: r.page.area,
        full: &[],
        relax: Relax::default(),
    }
}

/// Whether `route` runs through a text of `r`'s occupancy (inside it by more than a tick): any text, or with `fixed`
/// only one that cannot step aside.
fn across_text(r: &Router, route: &[Pt], fixed: bool) -> bool {
    r.occ.texts.iter().any(|t| {
        (!fixed || t.label.is_none())
            && geom::segments(route)
                .iter()
                .any(|s| geom::stretch_inside(*s, t.rect.grown(-1)).is_some())
    })
}

/// The plain shape of a wire from a pin at `a` leaving along `sa` to one at `b` leaving along `sb`, as the placer asks
/// the text placer about it: one corner where the sides turn a quarter; straight where they face each other on one
/// line, else stepping at the middle, its middle on the whole step at or before the half way; none where both leave on
/// one side (out, across and back: the router's to place). Unlike [`unrouted_line`] it steps where pins that face each
/// other stand off one line.
pub(crate) fn plain_line((a, sa): (Pt, Side), (b, sb): (Pt, Side)) -> Option<Vec<Pt>> {
    Some(if sa.vertical() != sb.vertical() {
        let corner = if sa.vertical() {
            Pt::new(a.x, b.y)
        } else {
            Pt::new(b.x, a.y)
        };
        vec![a, corner, b]
    } else if sa == sb {
        return None;
    } else if sa.across(a) == sa.across(b) {
        vec![a, b]
    } else if sa.vertical() {
        let y = grid_down((a.y + b.y).div_euclid(2));
        vec![a, Pt::new(a.x, y), Pt::new(b.x, y), b]
    } else {
        let x = grid_down((a.x + b.x).div_euclid(2));
        vec![a, Pt::new(x, a.y), Pt::new(x, b.y), b]
    })
}

/// The line of a wire without a route between its two ends `a` and `b` (in any one unit): right-angled, never
/// diagonal. Ends on one horizontal or vertical line are joined straight; else by one corner made of the ends' own
/// coordinates: the first leg along the first end's side, else the last leg along the second end's, else the first leg
/// across. It takes no notice of what lies under it: it is the line an unrouted wire is drawn with, dashed, and no wire.
pub fn unrouted_line(
    (a, a_side): ((i64, i64), Option<Side>),
    (b, b_side): ((i64, i64), Option<Side>),
) -> Vec<(i64, i64)> {
    if a.0 == b.0 || a.1 == b.1 {
        return vec![a, b];
    }
    let vertical_first = match (a_side, b_side) {
        (Some(d), _) => d.vertical(),
        (None, Some(d)) => !d.vertical(),
        (None, None) => false,
    };
    let corner = if vertical_first { (a.0, b.1) } else { (b.0, a.1) };
    vec![a, corner, b]
}
