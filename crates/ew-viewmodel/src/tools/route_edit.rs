// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route edit (FR-CON-007): a segment of a connection's route moved perpendicular to itself in grid
//! steps, or a bend moved in both directions, the neighbours stretching, committed as `connection.route`.
//!
//! **Pointer** (Select's `dragging (segment)` rows, [`SelectTool`](super::SelectTool)): while one connection is
//! selected, Select shows a handle on every segment and every bend of its line ([`handles_overlay`]). A press on a
//! segment of a *selected* connection drags it: a horizontal segment moves vertically and a vertical one
//! horizontally. A press on a bend drags the corner, which moves its two segments. The readout shows the offset;
//! the release commits, Escape cancels.
//!
//! **Keyboard** ([`RouteEditTool`], `tool.route_edit` on one selected connection): Tab cycles the segments,
//! the grid cursor's row or column is the segment's new position, Enter commits. On a route-less connection the
//! first edit starts from its drawn polyline, so the commit stores it with the moved segment; Enter without a move
//! stores the drawn polyline as it is.
//!
//! **The ends stay.** The route's ends are on their connection points and never move. A first or last segment that
//! is moved keeps a stub of one grid step at its end (none when the segment is no longer than a step) and jogs
//! after it, so a straight wire dragged sideways becomes a bracket round what was in its way. (Until M2.5-Fix only
//! the segments between the first and the last could be moved, so a straight wire and an L had nothing to drag.)

use crate::key;
use ew_api::AnyCommand;
use ew_api::output::TableConnection;
use ew_api::output::ew_dl::EntityRef;
use serde_json::json;

use super::autoconnect::dedup;
use super::move_keys::readout;
use super::wire::PageGridMap;
use super::{
    Anchor, CanvasEvent, Overlay, OverlayItem, Style, Token, Tool, ToolCx, ToolKey, ToolOutput, rect_of,
};

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

/// The steps of a segment drag from the sheet point `from` to `to`: the perpendicular axis only.
pub fn segment_steps(
    cx: &ToolCx<'_>,
    line: &[(i64, i64)],
    seg: usize,
    from: (i64, i64),
    to: (i64, i64),
) -> i64 {
    let (kx, ky) = part_steps(cx, line, RoutePart::Segment(seg), from, to);
    kx + ky
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

/// The overlay of a segment moved by `k` steps: the new route dashed and the readout at the segment.
pub fn segment_overlay(cx: &ToolCx<'_>, line: &[(i64, i64)], seg: usize, k: i64) -> Overlay {
    let steps = if horizontal(line, seg) { (0, k) } else { (k, 0) };
    part_overlay(cx, line, RoutePart::Segment(seg), steps)
}

/// The handles of a selected connection's line: a solid square on the middle of every segment that moves and a dashed
/// square on every bend, 3 points across each half, as the grips of a selected black box (shapes, not colour, tell
/// them apart; NFR-A11Y-03). They show where a drag takes hold; a press anywhere on a segment drags it too.
pub fn handles_overlay(cx: &ToolCx<'_>, c: &TableConnection) -> Overlay {
    let line = dedup(&c.polyline);
    let r = cx.nm_of_points(3.0).round() as i64;
    let square = |p: (i64, i64), style: Style| OverlayItem::Rect {
        r: rect_of((p.0 - r, p.1 - r), (p.0 + r, p.1 + r)),
        style,
    };
    let sheet = |p: (i64, i64)| cx.snapper.page_to_sheet(p);
    let mut items: Vec<OverlayItem> = movable_segments(&line)
        .into_iter()
        .map(|i| {
            let (a, b) = (sheet(line[i]), sheet(line[i + 1]));
            square(
                (a.0 + (b.0 - a.0) / 2, a.1 + (b.1 - a.1) / 2),
                Style::solid(Token::Ghost),
            )
        })
        .collect();
    let unit = PageGridMap::new((1, 1));
    items.extend(
        (1..line.len().saturating_sub(1))
            .filter(|i| moved_bend(&line, *i, (0, 0), unit).is_some())
            .map(|i| square(sheet(line[i]), Style::dashed(Token::Ghost))),
    );
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

/// The keyboard route editor (`tool.route_edit`): see the module documentation.
#[derive(Clone, Debug)]
pub struct RouteEditTool {
    connection: EntityRef,
    line: Vec<(i64, i64)>,
    stored: bool,
    /// The movable segments and the index of the current one.
    segments: Vec<usize>,
    current: usize,
    /// The grid cursor when the tool was armed or the segment changed, and the steps since.
    from: Option<(i64, i64)>,
    steps: i64,
}

impl RouteEditTool {
    /// The editor of connection row `c` (its drawn polyline; `stored` whether it has a stored route), the grid cursor
    /// at `cursor`.
    pub fn new(c: &TableConnection, cursor: Option<(i64, i64)>) -> RouteEditTool {
        let line = dedup(&c.polyline);
        RouteEditTool {
            connection: EntityRef(c.id),
            segments: movable_segments(&line),
            line,
            stored: c.stored_route,
            current: 0,
            from: cursor,
            steps: 0,
        }
    }

    /// The connection it edits.
    pub fn connection(&self) -> EntityRef {
        self.connection
    }

    /// The current segment's index in the polyline, if it has a movable segment.
    pub fn segment(&self) -> Option<usize> {
        self.segments.get(self.current).copied()
    }

    /// The route the editor would commit.
    pub fn route(&self, cx: &ToolCx<'_>) -> Vec<(i64, i64)> {
        let grid = PageGridMap::new(cx.page.tables.grid);
        self.segment()
            .and_then(|s| moved_route(&self.line, s, self.steps, grid))
            .unwrap_or_else(|| self.line.clone())
    }

    /// The overlay: the route with the current segment moved, or the drawn polyline.
    pub fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        match self.segment() {
            Some(s) => segment_overlay(cx, &self.line, s, self.steps),
            None => Overlay::new(vec![OverlayItem::Polyline {
                points: self.line.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect(),
                style: Style::dashed(Token::Ghost),
            }]),
        }
    }

    /// The command of Enter or a dwell: the moved route; the drawn polyline of a route-less connection unmoved;
    /// `None` for a stored route unmoved.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        if self.steps == 0 && self.stored {
            return None;
        }
        Some(route_command(self.connection, &self.route(cx)))
    }
}

impl Tool for RouteEditTool {
    fn name(&self) -> &'static str {
        "route_edit"
    }

    fn state(&self) -> &'static str {
        "armed"
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::CursorMove { at } => {
                let from = *self.from.get_or_insert(*at);
                if let Some(s) = self.segment() {
                    self.steps = segment_steps(cx, &self.line, s, from, *at);
                }
                ToolOutput::Transient(self.overlay(cx))
            }
            CanvasEvent::Key(ToolKey::NextProposal) => {
                if !self.segments.is_empty() {
                    self.current = (self.current + 1) % self.segments.len();
                    self.steps = 0;
                    self.from = None;
                }
                ToolOutput::Transient(self.overlay(cx))
            }
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Activate => match self.command(cx) {
                Some(cmd) => ToolOutput::Commit(cmd),
                None => ToolOutput::Cancel,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> super::Hint {
        super::Hint::new(
            key!("hint-route-edit"),
            &["tool.next_proposal", super::ENTER, super::ESCAPE],
        )
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
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
