// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route edit (WP-S2.04 §8.4, FR-CON-007): a segment of a connection's route moved perpendicular to itself in grid
//! steps, its two neighbours stretching, committed as `connection.route`.
//!
//! **Pointer** (Select's `dragging (segment)` rows, [`SelectTool`](super::SelectTool)): a press on a segment of a
//! *selected* connection that is neither its first nor its last segment drags it; a horizontal segment moves
//! vertically and a vertical one horizontally; the readout shows the offset; the release commits, Escape cancels.
//!
//! **Keyboard** ([`RouteEditTool`], `tool.route_edit` on one selected connection): Tab cycles the movable segments,
//! the grid cursor's row or column is the segment's new position, Enter commits. On a route-less connection the
//! first edit starts from its drawn polyline, so the commit stores it with the moved segment; Enter without a move
//! stores the drawn polyline as it is (the remedy of §8.3: "give this wire a route first").
//!
//! The first and last segments never move, so the route's ends stay on their connection points.

use ew_api::AnyCommand;
use ew_api::output::TableConnection;
use ew_api::output::ew_dl::EntityRef;
use serde_json::json;

use super::autoconnect::dedup;
use super::move_keys::readout;
use super::wire::PageGridMap;
use super::{Anchor, CanvasEvent, Overlay, OverlayItem, Style, Token, Tool, ToolCx, ToolKey, ToolOutput};

/// The movable segments of a polyline: the indices of its axis-parallel segments that are neither the first nor the
/// last.
pub fn movable_segments(line: &[(i64, i64)]) -> Vec<usize> {
    let n = line.len();
    if n < 4 {
        return Vec::new();
    }
    (1..n - 2)
        .filter(|&i| {
            let (a, b) = (line[i], line[i + 1]);
            (a.0 == b.0) != (a.1 == b.1)
        })
        .collect()
}

/// The polyline with segment `seg` moved by `k` grid steps perpendicular to itself (a horizontal segment's y, a
/// vertical one's x, each from its own grid index), tidied: a segment moved onto a neighbour's coordinate leaves no
/// duplicate or collinear vertex (fix round of WP-S2.04 part 2, item 5; the ends stay); `None` for a segment that is
/// not axis-parallel.
pub fn moved_route(line: &[(i64, i64)], seg: usize, k: i64, grid: PageGridMap) -> Option<Vec<(i64, i64)>> {
    let (a, b) = (*line.get(seg)?, *line.get(seg + 1)?);
    let mut out = line.to_vec();
    if a.1 == b.1 && a.0 != b.0 {
        let y = grid.coord(grid.index(a.1) + k);
        out[seg].1 = y;
        out[seg + 1].1 = y;
    } else if a.0 == b.0 && a.1 != b.1 {
        let x = grid.coord(grid.index(a.0) + k);
        out[seg].0 = x;
        out[seg + 1].0 = x;
    } else {
        return None;
    }
    Some(super::wire::clean(out))
}

/// Whether segment `seg` of `line` is horizontal.
pub fn horizontal(line: &[(i64, i64)], seg: usize) -> bool {
    line.get(seg)
        .zip(line.get(seg + 1))
        .is_some_and(|(a, b)| a.1 == b.1)
}

/// A segment of a connection: the connection, its deduplicated polyline and the segment's index.
pub type SegmentRef = (EntityRef, Vec<(i64, i64)>, usize);

/// The movable segment of the selected connections under the sheet point `at` within 4 points (§7.3's second
/// row): `(connection, its deduplicated polyline, segment index)`, the nearest, ties by connection ID.
pub fn segment_at(cx: &ToolCx<'_>, at: (i64, i64)) -> Option<SegmentRef> {
    let r = cx.nm_of_points(crate::canvas::HOVER_SLOP_PT).floor().min(9.0e15) as i128;
    let mut best: Option<(i128, SegmentRef)> = None;
    for c in &cx.page.tables.connections {
        if !cx.selection.items.iter().any(|e| e.0 == c.id) {
            continue;
        }
        let line = dedup(&c.polyline);
        for i in movable_segments(&line) {
            let d = super::segment_dist2(
                at,
                cx.snapper.page_to_sheet(line[i]),
                cx.snapper.page_to_sheet(line[i + 1]),
            );
            let id = EntityRef(c.id);
            if d <= r * r
                && best
                    .as_ref()
                    .is_none_or(|(bd, (bid, _, _))| d < *bd || (d == *bd && id < *bid))
            {
                best = Some((d, (id, line.clone(), i)));
            }
        }
    }
    best.map(|(_, s)| s)
}

/// The steps of a segment drag from the sheet point `from` to `to`: the perpendicular axis only.
pub fn segment_steps(
    cx: &ToolCx<'_>,
    line: &[(i64, i64)],
    seg: usize,
    from: (i64, i64),
    to: (i64, i64),
) -> i64 {
    let (kx, ky) = super::move_keys::steps_between(cx, from, to);
    if horizontal(line, seg) { ky } else { kx }
}

/// The overlay of a segment moved by `k` steps: the new route dashed and the readout at the segment.
pub fn segment_overlay(cx: &ToolCx<'_>, line: &[(i64, i64)], seg: usize, k: i64) -> Overlay {
    let grid = PageGridMap::new(cx.page.tables.grid);
    let route = moved_route(line, seg, k, grid).unwrap_or_else(|| line.to_vec());
    let pts: Vec<(i64, i64)> = route.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect();
    let steps = if horizontal(line, seg) { (0, k) } else { (k, 0) };
    let mut items = vec![OverlayItem::Polyline {
        points: pts.clone(),
        style: Style::dashed(Token::Ghost),
    }];
    if let Some(&at) = pts.get(seg) {
        items.push(OverlayItem::Label {
            at,
            text: readout(cx, steps),
            anchor: Anchor::Right,
        });
    }
    Overlay::new(items)
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

    fn status_hint(&self) -> &'static str {
        "hint-route-edit"
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_interior_axis_parallel_segments_move_and_their_neighbours_stretch() {
        let z = vec![(0, 0), (5, 0), (5, 10), (10, 10)];
        assert_eq!(movable_segments(&z), vec![1]);
        assert!(
            movable_segments(&[(0, 0), (5, 0), (5, 10)]).is_empty(),
            "an L has none"
        );
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
    }
}
