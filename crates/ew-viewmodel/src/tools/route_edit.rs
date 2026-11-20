// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route edit (FR-CON-007): a segment of a connection's route moved perpendicular to itself in grid
//! steps, or a bend moved in both directions, the neighbours stretching, committed as `connection.route`.
//!
//! **Pointer** (Select's `dragging (segment)` rows, [`SelectTool`](super::SelectTool)): while one connection is
//! selected, Select shows a round handle on every segment and every bend of its line ([`handles_overlay`]), the one
//! a press would take larger. A press on a segment of a *selected* connection drags it: a horizontal segment moves
//! vertically and a vertical one horizontally. A press on a bend drags the corner, which moves its two segments. The
//! readout shows the offset; the release commits (one undo step), Escape cancels. On a route-less connection the drag
//! starts from its drawn polyline. The keyboard route editor (Edit route) is gone since rc.3: Route
//! (`layout.route`) routes selected wires anew.
//!
//! **Keyboard** (rc.3): with one wire selected, Tab and Shift+Tab give a handle the focus along the wire from the end
//! a person reads first ([`reads_backwards`]; `route.handle`, [`handle_parts`]); it shows as the one under the pointer
//! does, and the status line names it (the n-th of the wire's handles, a segment and the arrows that move it, or a
//! bend). The arrow keys move it one grid step (`route.handle_move`), one undo step each; an arrow that cannot move it
//! (along its segment, or where the route would not stay square on its ends) says so in the status line. Esc gives the
//! focus back to the wire.
//!
//! **The ends stay.** The route's ends are on their connection points and never move. A first or last segment that
//! is moved keeps a stub of one grid step at its end (none when the segment is no longer than a step) and jogs
//! after it, so a straight wire dragged sideways becomes a bracket round what was in its way. (Until M2.5-Fix only
//! the segments between the first and the last could be moved, so a straight wire and an L had nothing to drag.)

use ew_api::AnyCommand;
use ew_api::output::TableConnection;
use ew_api::output::ew_dl::EntityRef;
use serde_json::json;

use super::autoconnect::dedup;
use super::move_keys::readout;
use super::wire::PageGridMap;
use super::{Anchor, Overlay, OverlayItem, Style, Token, ToolCx};

/// What of a route a drag moves: the segment from vertex `n` to the next, or the bend at the interior vertex `n`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoutePart {
    /// A segment, moved perpendicular to itself.
    Segment(usize),
    /// A bend, moved in both directions.
    Bend(usize),
}

/// The movable segments of a polyline: the indices of its axis-parallel segments.
pub fn movable_segments(line: &[(i64, i64)]) -> Vec<usize> {
    (0..line.len().saturating_sub(1))
        .filter(|&i| {
            let (a, b) = (line[i], line[i + 1]);
            (a.0 == b.0) != (a.1 == b.1)
        })
        .collect()
}

/// The polyline with segment `seg` moved by `k` grid steps perpendicular to itself (a horizontal segment's y, a
/// vertical one's x, each from its own grid index), tidied: a segment moved onto a neighbour's coordinate leaves no
/// duplicate or collinear vertex. The ends stay (see the module documentation); `None` for a segment that is not
/// axis-parallel.
pub fn moved_route(line: &[(i64, i64)], seg: usize, k: i64, grid: PageGridMap) -> Option<Vec<(i64, i64)>> {
    move_segment(line, seg, k, grid).map(|(out, _)| super::wire::clean(out))
}

/// A moved polyline before it is tidied, with the place in it of each vertex of the polyline it was made from.
type Moved = (Vec<(i64, i64)>, Vec<usize>);

/// [`moved_route`] before it is tidied, with the place in it of each vertex of `line` (a moved vertex's: its moved
/// copy; an end's: the moved copy of its stub).
fn move_segment(line: &[(i64, i64)], seg: usize, k: i64, grid: PageGridMap) -> Option<Moved> {
    let n = line.len();
    let (a, b) = (*line.get(seg)?, *line.get(seg + 1)?);
    let across = a.1 == b.1 && a.0 != b.0;
    if !across && !(a.0 == b.0 && a.1 != b.1) {
        return None;
    }
    let shift = |p: (i64, i64)| {
        if across {
            (p.0, grid.coord(grid.index(p.1) + k))
        } else {
            (grid.coord(grid.index(p.0) + k), p.1)
        }
    };
    // The stub point of the end `e` of the segment whose other vertex is `o`: one grid step from `e` towards `o`, or
    // `e` itself when the segment is no longer than that.
    let stub = |e: (i64, i64), o: (i64, i64)| {
        let (from, to) = if across { (e.0, o.0) } else { (e.1, o.1) };
        let at = grid.coord(grid.index(from) + (to - from).signum());
        if (at - from).abs() >= (to - from).abs() {
            e
        } else if across {
            (at, e.1)
        } else {
            (e.0, at)
        }
    };
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(n + 4);
    let mut map: Vec<usize> = Vec::with_capacity(n);
    for (j, p) in line.iter().enumerate() {
        if j == seg && seg == 0 {
            // The first end stays: the end, its stub, the stub moved.
            let s = stub(a, b);
            out.push(a);
            if s != a {
                out.push(s);
            }
            map.push(out.len());
            out.push(shift(s));
        } else if j == seg + 1 && seg + 2 == n {
            // The last end stays: the stub moved, the stub, the end.
            let s = stub(b, a);
            map.push(out.len());
            out.push(shift(s));
            if s != b {
                out.push(s);
            }
            out.push(b);
        } else {
            map.push(out.len());
            out.push(if j == seg || j == seg + 1 { shift(*p) } else { *p });
        }
    }
    Some((out, map))
}

/// The polyline with the bend at the interior vertex `i` moved by `(kx, ky)` grid steps: its horizontal segment moves
/// by `ky` and its vertical one by `kx`, each as [`moved_route`] moves a segment, so the ends stay and every
/// segment stays axis-parallel. `None` for a vertex that is no interior bend of two axis-parallel segments.
pub fn moved_bend(
    line: &[(i64, i64)],
    i: usize,
    (kx, ky): (i64, i64),
    grid: PageGridMap,
) -> Option<Vec<(i64, i64)>> {
    if i == 0 || i + 1 >= line.len() {
        return None;
    }
    let steps = |l: &[(i64, i64)], seg: usize| if horizontal(l, seg) { ky } else { kx };
    let (first, map) = match steps(line, i - 1) {
        0 => {
            // Unmoved, but checked as a segment that could move.
            move_segment(line, i - 1, 0, grid)?;
            (line.to_vec(), (0..line.len()).collect::<Vec<usize>>())
        }
        k => move_segment(line, i - 1, k, grid)?,
    };
    let after = *map.get(i)?;
    let second = match steps(&first, after) {
        0 => None,
        // A bend moved onto the next vertex has no second segment left to move.
        k => move_segment(&first, after, k, grid).map(|(out, _)| out),
    };
    Some(super::wire::clean(second.unwrap_or(first)))
}

/// The polyline with `part` moved by `steps` grid steps: a segment by the step across it, a bend by both.
pub fn moved_part(
    line: &[(i64, i64)],
    part: RoutePart,
    steps: (i64, i64),
    grid: PageGridMap,
) -> Option<Vec<(i64, i64)>> {
    match part {
        RoutePart::Segment(seg) => {
            let k = if horizontal(line, seg) { steps.1 } else { steps.0 };
            moved_route(line, seg, k, grid)
        }
        RoutePart::Bend(i) => moved_bend(line, i, steps, grid),
    }
}

/// Whether segment `seg` of `line` is horizontal.
pub fn horizontal(line: &[(i64, i64)], seg: usize) -> bool {
    line.get(seg)
        .zip(line.get(seg + 1))
        .is_some_and(|(a, b)| a.1 == b.1)
}

/// A part of a connection: the connection, its deduplicated polyline and the part.
pub type PartRef = (EntityRef, Vec<(i64, i64)>, RoutePart);

/// The part of the selected connections under the sheet point `at` within 4 points (§7.3's second row): a bend
/// before a segment (a bend lies on two of them), of each kind the nearest, ties by connection ID.
pub fn part_at(cx: &ToolCx<'_>, at: (i64, i64)) -> Option<PartRef> {
    let r = cx.nm_of_points(crate::canvas::HOVER_SLOP_PT).floor().min(9.0e15) as i128;
    let mut bend: Option<(i128, PartRef)> = None;
    let mut segment: Option<(i128, PartRef)> = None;
    let nearer = |best: &Option<(i128, PartRef)>, d: i128, id: EntityRef| {
        d <= r * r
            && best
                .as_ref()
                .is_none_or(|(bd, (bid, _, _))| d < *bd || (d == *bd && id < *bid))
    };
    for c in &cx.page.tables.connections {
        if !cx.selection.items.iter().any(|e| e.0 == c.id) {
            continue;
        }
        let line = dedup(&c.polyline);
        let id = EntityRef(c.id);
        for i in 1..line.len().saturating_sub(1) {
            let p = cx.snapper.page_to_sheet(line[i]);
            let d = super::segment_dist2(at, p, p);
            if nearer(&bend, d, id) && moved_bend(&line, i, (0, 0), PageGridMap::new((1, 1))).is_some() {
                bend = Some((d, (id, line.clone(), RoutePart::Bend(i))));
            }
        }
        for i in movable_segments(&line) {
            let d = super::segment_dist2(
                at,
                cx.snapper.page_to_sheet(line[i]),
                cx.snapper.page_to_sheet(line[i + 1]),
            );
            if nearer(&segment, d, id) {
                segment = Some((d, (id, line.clone(), RoutePart::Segment(i))));
            }
        }
    }
    bend.or(segment).map(|(_, s)| s)
}

/// The part of a selected connection a press at the sheet point `at` takes ([`part_at`]), but none inside a
/// representation's rectangle, where a drag moves the representation (FT-500): Select's press, and the handle the
/// window marks under the pointer.
pub fn press_part(cx: &ToolCx<'_>, at: (i64, i64)) -> Option<PartRef> {
    let in_rep = cx
        .pick_stack(at)
        .iter()
        .any(|e| super::entity_kind(&cx.page.tables, *e) == Some(super::EntityKind::Rep));
    if in_rep { None } else { part_at(cx, at) }
}

/// Which way a handle of `part` of `line` moves: a horizontal segment's up and down, a vertical one's sideways, a
/// bend's both ways (the pointer's shape over it).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GripMove {
    /// Up and down (a horizontal segment).
    UpDown,
    /// Sideways (a vertical segment).
    Sideways,
    /// Both ways (a bend).
    Free,
}

/// The way `part` of `line` moves ([`GripMove`]).
pub fn grip_move(line: &[(i64, i64)], part: RoutePart) -> GripMove {
    match part {
        RoutePart::Segment(seg) if horizontal(line, seg) => GripMove::UpDown,
        RoutePart::Segment(_) => GripMove::Sideways,
        RoutePart::Bend(_) => GripMove::Free,
    }
}

/// The steps of a drag of `part` from the sheet point `from` to `to`: for a segment the step across it only.
pub fn part_steps(
    cx: &ToolCx<'_>,
    line: &[(i64, i64)],
    part: RoutePart,
    from: (i64, i64),
    to: (i64, i64),
) -> (i64, i64) {
    let (kx, ky) = super::move_keys::steps_between(cx, from, to);
    match part {
        RoutePart::Segment(seg) if horizontal(line, seg) => (0, ky),
        RoutePart::Segment(_) => (kx, 0),
        RoutePart::Bend(_) => (kx, ky),
    }
}

/// The overlay of `part` moved by `steps`: the new route dashed and the readout at the part.
pub fn part_overlay(cx: &ToolCx<'_>, line: &[(i64, i64)], part: RoutePart, steps: (i64, i64)) -> Overlay {
    let grid = PageGridMap::new(cx.page.tables.grid);
    let route = moved_part(line, part, steps, grid).unwrap_or_else(|| line.to_vec());
    let pts: Vec<(i64, i64)> = route.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect();
    let mut items = vec![OverlayItem::Polyline {
        points: pts,
        style: Style::dashed(Token::Ghost),
    }];
    let (RoutePart::Segment(at) | RoutePart::Bend(at)) = part;
    if let Some(&at) = line.get(at) {
        items.push(OverlayItem::Label {
            at: cx.snapper.page_to_sheet(at),
            text: readout(cx, steps),
            anchor: Anchor::Right,
        });
    }
    Overlay::new(items)
}

/// The handles of `line` in the order of the wire from its first end: the middle of every segment that moves and
/// every bend between them. Tab and Shift+Tab go through them in this order.
pub fn handle_parts(line: &[(i64, i64)]) -> Vec<RoutePart> {
    let segments = movable_segments(line);
    let unit = PageGridMap::new((1, 1));
    let mut out = Vec::new();
    for i in 0..line.len().saturating_sub(1) {
        if i > 0 && moved_bend(line, i, (0, 0), unit).is_some() {
            out.push(RoutePart::Bend(i));
        }
        if segments.contains(&i) {
            out.push(RoutePart::Segment(i));
        }
    }
    out
}

/// Whether `line` reads from its last point to its first: its last point stands above its first, or level with it and
/// to its left (page y grows downwards). Tab then goes through [`handle_parts`] backwards, from the end a person
/// reads first.
pub fn reads_backwards(line: &[(i64, i64)]) -> bool {
    match (line.first(), line.last()) {
        (Some(a), Some(b)) => (b.1, b.0) < (a.1, a.0),
        _ => false,
    }
}

/// Where the handle of `part` of `line` stands, page nm: a segment's middle or the bend.
pub fn handle_at(line: &[(i64, i64)], part: RoutePart) -> Option<(i64, i64)> {
    match part {
        RoutePart::Segment(i) => {
            let (a, b) = (*line.get(i)?, *line.get(i + 1)?);
            Some((a.0 + (b.0 - a.0) / 2, a.1 + (b.1 - a.1) / 2))
        }
        RoutePart::Bend(i) => line.get(i).copied(),
    }
}

/// The handles of a selected connection's line: a round handle ([`OverlayItem::Grip`]) on the middle of every
/// segment that moves and on every bend ([`handle_parts`]), hot the one a press at `pointer` takes ([`press_part`])
/// and the one at `focus` in [`handle_parts`]' order (the keyboard's, Tab). They show where a drag takes hold; a press
/// anywhere on a segment drags it too.
pub fn handles_overlay(
    cx: &ToolCx<'_>,
    c: &TableConnection,
    pointer: Option<(i64, i64)>,
    focus: Option<usize>,
) -> Overlay {
    let line = dedup(&c.polyline);
    let hot = pointer
        .and_then(|p| press_part(cx, p))
        .filter(|(id, _, _)| id.0 == c.id)
        .map(|(_, _, part)| part);
    let parts = handle_parts(&line);
    let focused = focus.and_then(|k| parts.get(k).copied());
    let items = parts
        .iter()
        .filter_map(|&part| {
            Some(OverlayItem::Grip {
                at: cx.snapper.page_to_sheet(handle_at(&line, part)?),
                hot: hot == Some(part) || focused == Some(part),
            })
        })
        .collect();
    Overlay::new(items)
}

/// The selection's one connection, when exactly one entity is selected and it is a connection of the page.
pub fn selected_connection<'t>(cx: &'t ToolCx<'_>) -> Option<&'t TableConnection> {
    match cx.selection.items.as_slice() {
        [one] => cx.page.tables.connections.iter().find(|c| c.id == one.0),
        _ => None,
    }
}

/// `connection.route {connection, route}`.
pub fn route_command(connection: EntityRef, route: &[(i64, i64)]) -> Box<AnyCommand> {
    Box::new(AnyCommand {
        name: "connection.route".into(),
        params: json!({
            "connection": connection.0.to_string(),
            "route": route.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_moves_across_itself_and_its_neighbours_stretch() {
        let z = vec![(0, 0), (5, 0), (5, 10), (10, 10)];
        assert_eq!(movable_segments(&z), vec![0, 1, 2]);
        let g = PageGridMap::new((1, 1));
        assert_eq!(
            moved_route(&z, 1, 3, g),
            Some(vec![(0, 0), (8, 0), (8, 10), (10, 10)])
        );
        let u = vec![(0, 0), (0, 5), (10, 5), (10, 0)];
        assert_eq!(
            moved_route(&u, 1, -2, g),
            Some(vec![(0, 0), (0, 3), (10, 3), (10, 0)])
        );
        // Moved onto the first end's coordinate: the duplicate and the collinear vertex go, the ends stay.
        assert_eq!(moved_route(&z, 1, -5, g), Some(vec![(0, 0), (0, 10), (10, 10)]));
        let w = vec![(0, 0), (5, 0), (5, 10), (8, 10), (8, 20)];
        assert_eq!(
            moved_route(&w, 2, 10, g),
            Some(vec![(0, 0), (5, 0), (5, 20), (8, 20)])
        );
        // A diagonal segment does not move.
        assert_eq!(moved_route(&[(0, 0), (3, 4)], 0, 1, g), None);
    }

    #[test]
    fn an_end_segment_keeps_its_end_and_a_stub() {
        let g = PageGridMap::new((1, 1));
        // A straight wire dragged sideways becomes a bracket: a stub of one step at each end.
        assert_eq!(
            moved_route(&[(0, 0), (10, 0)], 0, 3, g),
            Some(vec![(0, 0), (1, 0), (1, 3), (9, 3), (9, 0), (10, 0)])
        );
        // An L's first segment: the stub at the end, the bend follows.
        let l = vec![(0, 0), (10, 0), (10, 8)];
        assert_eq!(
            moved_route(&l, 0, 2, g),
            Some(vec![(0, 0), (1, 0), (1, 2), (10, 2), (10, 8)])
        );
        // Its last segment.
        assert_eq!(
            moved_route(&l, 1, -4, g),
            Some(vec![(0, 0), (6, 0), (6, 7), (10, 7), (10, 8)])
        );
        // A segment no longer than a step has no stub: it jogs at the end.
        assert_eq!(
            moved_route(&[(0, 0), (1, 0), (1, 5)], 0, 2, g),
            Some(vec![(0, 0), (0, 2), (1, 2), (1, 5)])
        );
    }

    #[test]
    fn a_bend_moves_both_ways_and_the_route_stays_right_angled() {
        let g = PageGridMap::new((1, 1));
        let z = vec![(0, 0), (5, 0), (5, 10), (10, 10)];
        // The first bend: its horizontal segment is the first, so it keeps a stub; its vertical one moves across.
        assert_eq!(
            moved_bend(&z, 1, (2, 3), g),
            Some(vec![(0, 0), (1, 0), (1, 3), (7, 3), (7, 10), (10, 10)])
        );
        // Along one axis only.
        assert_eq!(
            moved_bend(&z, 1, (2, 0), g),
            Some(vec![(0, 0), (7, 0), (7, 10), (10, 10)])
        );
        // No bend at an end, none on a diagonal.
        assert_eq!(moved_bend(&z, 0, (1, 1), g), None);
        assert_eq!(moved_bend(&[(0, 0), (3, 4), (3, 9)], 1, (1, 1), g), None);
        for route in [moved_bend(&z, 1, (2, 3), g), moved_bend(&z, 2, (-1, -4), g)] {
            let route = route.expect("a route");
            assert_eq!((route.first(), route.last()), (z.first(), z.last()));
            assert!(
                route.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1),
                "{route:?}"
            );
        }
    }
}
