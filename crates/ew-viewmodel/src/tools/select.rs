// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Select tool (`select`; WP-S2.03 §7.6 with WP-S2.04 §7.3's drag rows and keyboard move and §7.4's
//! transforms; doc 06 §4.3).
//!
//! A press is classified by the pick of WP-S2.04 §6.6 in this order: a link; a segment of a **selected** connection
//! that is neither its first nor its last segment (part 2, §8.4); a **movable** entity (a representation); any other
//! entity (a connection, a junction); empty canvas.
//!
//! | State | Event (condition) | Next | Output |
//! |---|---|---|---|
//! | idle | press on a link | idle | `Follow` on release within 3 points |
//! | idle | press on a segment of a selected connection (not its first or last) | dragging (segment) | `None` |
//! | dragging (segment) | move | dragging (segment) | `Transient`: the route with the segment moved perpendicular to itself in grid steps, its neighbours stretched, the readout |
//! | dragging (segment) | `Dwell { ms ≥ 80 }` | dragging (segment) | `Preview(connection.route)` when moved |
//! | dragging (segment) | release | idle | `Commit(connection.route {connection, route})` when moved, else `Select` (as a click on the connection) |
//! | idle | press on a movable entity | pressed (movable) | `None` |
//! | idle | press on another entity | pressed (other) | `None` |
//! | idle | press on empty canvas | rubber-band | `Transient` (the band) |
//! | pressed (either) | release within 3 points | idle | `Select`: replace with the entity; Shift adds; Ctrl toggles |
//! | pressed (movable) | move beyond 3 points | dragging | `Transient`: the ghost of the moved entities' rectangles offset by the step displacement, rubber bands and the readout; the moved set is the selection's representations when the pressed entity is selected, else the pressed entity alone |
//! | pressed (other) | move beyond 3 points | rubber-band | `Transient` |
//! | rubber-band | move | rubber-band | `Transient`: solid while the drag goes right (encloses), dashed left (touches) |
//! | rubber-band | release | idle | `Select`: left to right the entities inside the band, right to left those that touch it; Shift adds |
//! | dragging | move | dragging | `Transient`; steps `k = round(page displacement × den / num)` per axis |
//! | dragging | `Dwell { ms ≥ 80 }` | dragging | `Preview(rep.move {reps, steps})` when the steps are not zero |
//! | dragging | release | idle | `Commit(rep.move {reps, steps})` when the steps are not zero, else `Select` (as a click) |
//! | dragging or moving | `Escape` | idle | `Cancel` |
//! | idle with representations selected | [`SelectTool::start_move`] (M) | moving | `Transient`; the host puts the grid cursor on the primary entity's insertion point first |
//! | moving | `CursorMove` | moving | steps = the cursor's grid displacement since M; `Transient` |
//! | moving | `Dwell`, `Activate` | moving, idle | as dragging: `Preview`, `Commit` (zero steps: `Cancel`) |
//! | idle with representations selected | `Key(RotateCw, RotateCcw, MirrorX, MirrorY, NextVariant)` | idle | `Commit(rep.transform …)` (§7.4; V on exactly one) |
//! | any | `Activate` | idle | as a press and release at the grid cursor |
//! | any | `Escape` | idle | `Cancel`, or `None` from idle |
//!
//! Connections and junctions in a selection are never moved. Picking uses WP-S2.02's entity rectangles (an entity
//! may have several: it lies inside a band when all do, and touches it when one does) plus the exact segment
//! distance of connections (§6.6). A release on empty canvas within 3 points clears the selection (Shift keeps it).

use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::output::ew_dl::{Destination, EntityRef};

use super::move_keys::{move_command, move_overlay, moved_set, readout, steps_between};
use super::route_edit::{moved_route, route_command, segment_at, segment_overlay, segment_steps};
use super::transform::{selected_reps, transform_command};
use super::wire::PageGridMap;
use super::{
    Button, CanvasEvent, EntityKind, Ghost, Mods, Overlay, OverlayItem, SelectionChange, Style, Token, Tool,
    ToolCx, ToolKey, ToolOutput, entity_kind, rect_of,
};

/// How far a press may travel and still be a click, in screen points.
pub const CLICK_SLOP_PT: f64 = 3.0;

/// The Select tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    Idle,
    /// A press on a link, followed on a release within 3 points.
    Link {
        dest: Destination,
        at: (i64, i64),
    },
    /// A press on an entity.
    Pressed {
        entity: EntityRef,
        at: (i64, i64),
        mods: Mods,
        movable: bool,
    },
    /// A rubber band from `from` to `to`.
    Band {
        from: (i64, i64),
        to: (i64, i64),
        mods: Mods,
    },
    /// A drag of movable entities by grid steps.
    Dragging {
        pressed: EntityRef,
        mods: Mods,
        moved: Vec<EntityRef>,
        from: (i64, i64),
        steps: (i64, i64),
        readout: String,
    },
    /// A drag of a segment of a selected connection (WP-S2.04 §8.4).
    Segment {
        connection: EntityRef,
        mods: Mods,
        /// The connection's deduplicated polyline and the dragged segment's index.
        line: Vec<(i64, i64)>,
        seg: usize,
        from: (i64, i64),
        steps: i64,
    },
    /// The keyboard move (M): the grid cursor's displacement since entry.
    Moving {
        moved: Vec<EntityRef>,
        from: (i64, i64),
        steps: (i64, i64),
        readout: String,
    },
}

/// The Select tool.
#[derive(Clone, Debug)]
pub struct SelectTool {
    state: State,
    /// The grid cursor as the last `CursorMove` placed it.
    cursor: Option<(i64, i64)>,
    /// The selection's generation and the page's revision when a drag or the keyboard move began: a change of
    /// either makes the pending move stale (fix round of WP-S2.04 part 1, item 11).
    origin: (u64, u64),
    /// The autoconnect proposals of the moved CPs at the current steps, as the describe text names them (shown only).
    proposals: Vec<(String, Option<String>)>,
}

impl Default for SelectTool {
    fn default() -> SelectTool {
        SelectTool::new()
    }
}

/// Whether `inner` lies inside `outer`, edges included.
fn inside(inner: Rect, outer: Rect) -> bool {
    let (a, b) = (inner.min(), inner.max());
    let (c, d) = (outer.min(), outer.max());
    a.x >= c.x && a.y >= c.y && b.x <= d.x && b.y <= d.y
}

/// Whether two rectangles share a point, edges included.
fn touches(a: Rect, b: Rect) -> bool {
    a.min().x <= b.max().x && b.min().x <= a.max().x && a.min().y <= b.max().y && b.min().y <= a.max().y
}

/// The entities a band selects: left to right (`encloses`) those whose rectangles all lie inside it, right to left
/// those with a rectangle that touches it; in reading order of the centre of each entity's rectangles together (y,
/// then x), ties by ID (FT-439, decision log ID-640). Not in paint order: the scene paints records in ID order, and
/// the IDs one transaction mints are not in creation order, so the selection (and the aliases it prints) could list
/// two new records the other way round in a replay.
pub fn band_selection(entities: &[(EntityRef, Rect)], band: Rect, encloses: bool) -> Vec<EntityRef> {
    let mut order: Vec<(EntityRef, Rect)> = Vec::new();
    for (e, r) in entities {
        match order.iter_mut().find(|(x, _)| x == e) {
            Some((_, union)) => *union = union.union(*r),
            None => order.push((*e, *r)),
        }
    }
    let mut picked: Vec<((i64, i64), EntityRef)> = order
        .into_iter()
        .filter(|(e, _)| {
            let mut rects = entities.iter().filter(|(x, _)| x == e).map(|(_, r)| *r);
            if encloses {
                rects.all(|r| inside(r, band))
            } else {
                rects.any(|r| touches(r, band))
            }
        })
        .map(|(e, r)| {
            let (x, y) = crate::canvas::rect_centre(r);
            ((y, x), e)
        })
        .collect();
    picked.sort();
    picked.into_iter().map(|(_, e)| e).collect()
}

impl SelectTool {
    /// The tool in its first state.
    pub fn new() -> SelectTool {
        SelectTool {
            state: State::Idle,
            cursor: None,
            origin: (0, 0),
            proposals: Vec::new(),
        }
    }

    /// The overlay of a move of `moved` by `steps`; the proposals of the moved CPs are kept for the ghost (shown only).
    fn move_out(&mut self, cx: &ToolCx<'_>, moved: &[EntityRef], steps: (i64, i64)) -> ToolOutput {
        self.proposals = super::move_keys::move_proposals(cx, moved, steps).lines();
        ToolOutput::Transient(move_overlay(cx, moved, steps))
    }

    /// The band overlay from `from` to `to`.
    fn band(from: (i64, i64), to: (i64, i64)) -> ToolOutput {
        let style = if to.0 >= from.0 {
            Style::solid(Token::RubberBand)
        } else {
            Style::dashed(Token::RubberBand)
        };
        ToolOutput::Transient(Overlay::new(vec![OverlayItem::Rect {
            r: rect_of(from, to),
            style,
        }]))
    }

    /// A click on an entity.
    fn click_entity(entity: EntityRef, mods: Mods) -> ToolOutput {
        ToolOutput::Select(if mods.ctrl {
            SelectionChange::Toggle(entity)
        } else if mods.shift {
            SelectionChange::Add(vec![entity])
        } else {
            SelectionChange::Replace(vec![entity])
        })
    }

    /// A finished band.
    fn finish_band(from: (i64, i64), to: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        let picked = band_selection(&cx.page.entities, rect_of(from, to), to.0 >= from.0);
        ToolOutput::Select(if mods.shift {
            SelectionChange::Add(picked)
        } else {
            SelectionChange::Replace(picked)
        })
    }

    /// Whether a pending drag or keyboard move no longer stands (fix round of WP-S2.04 part 1, item 11): the
    /// selection changed under it, the page's revision changed (a delete, an undo, a redo, another commit) or one of
    /// its representations is gone. The host then escapes it, so no stale representation reaches a preview.
    pub fn pending_move_stale(&self, cx: &ToolCx<'_>) -> bool {
        let moved = match &self.state {
            State::Dragging { moved, .. } | State::Moving { moved, .. } => moved,
            State::Segment { .. } => return self.origin.1 != cx.page.rev,
            _ => return false,
        };
        self.origin != (cx.selection.generation, cx.page.rev)
            || moved
                .iter()
                .any(|e| !cx.page.tables.reps.iter().any(|r| r.id == e.0))
    }

    /// Starts the keyboard move (M, WP-S2.04 §7.3) of the selection's representations from the grid cursor and
    /// answers its first overlay; `None` (and nothing changes) without representations selected, without a grid
    /// cursor or in another state than idle.
    pub fn start_move(&mut self, cx: &ToolCx<'_>) -> Option<ToolOutput> {
        if self.state != State::Idle {
            return None;
        }
        let moved = selected_reps(cx.selection, cx.page);
        let from = self.cursor?;
        if moved.is_empty() {
            return None;
        }
        let out = self.move_out(cx, &moved, (0, 0));
        self.origin = (cx.selection.generation, cx.page.rev);
        self.state = State::Moving {
            moved,
            from,
            steps: (0, 0),
            readout: readout(cx, (0, 0)),
        };
        Some(out)
    }

    fn press(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        if let Some(link) = cx.link_at(at) {
            self.state = State::Link { dest: link.dest, at };
            return ToolOutput::None;
        }
        if let Some((connection, line, seg)) = segment_at(cx, at) {
            self.origin = (cx.selection.generation, cx.page.rev);
            self.state = State::Segment {
                connection,
                mods,
                line,
                seg,
                from: at,
                steps: 0,
            };
            return ToolOutput::None;
        }
        if let Some(entity) = cx.pick(at) {
            let movable = entity_kind(&cx.page.tables, entity) == Some(EntityKind::Rep);
            self.state = State::Pressed {
                entity,
                at,
                mods,
                movable,
            };
            return ToolOutput::None;
        }
        self.state = State::Band {
            from: at,
            to: at,
            mods,
        };
        SelectTool::band(at, at)
    }

    fn release(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        let state = std::mem::replace(&mut self.state, State::Idle);
        match state {
            State::Idle => ToolOutput::None,
            State::Link { dest, at: from } => {
                if cx.within(from, at, CLICK_SLOP_PT) {
                    ToolOutput::Follow(dest)
                } else {
                    ToolOutput::None
                }
            }
            State::Pressed {
                entity,
                at: from,
                mods: pressed,
                ..
            } => {
                let held = Mods {
                    shift: pressed.shift || mods.shift,
                    ctrl: pressed.ctrl || mods.ctrl,
                    alt: pressed.alt || mods.alt,
                };
                if cx.within(from, at, CLICK_SLOP_PT) {
                    SelectTool::click_entity(entity, held)
                } else {
                    SelectTool::finish_band(from, at, held, cx)
                }
            }
            State::Band {
                from, mods: pressed, ..
            } => {
                let held = Mods {
                    shift: pressed.shift || mods.shift,
                    ..pressed
                };
                SelectTool::finish_band(from, at, held, cx)
            }
            State::Dragging {
                pressed,
                mods: held,
                moved,
                from,
                ..
            } => {
                let steps = steps_between(cx, from, at);
                if steps == (0, 0) {
                    SelectTool::click_entity(pressed, held)
                } else {
                    ToolOutput::Commit(move_command(&moved, steps))
                }
            }
            moving @ State::Moving { .. } => {
                self.state = moving;
                ToolOutput::None
            }
            State::Segment {
                connection,
                mods: held,
                line,
                seg,
                from,
                ..
            } => {
                let k = segment_steps(cx, &line, seg, from, at);
                let grid = PageGridMap::new(cx.page.tables.grid);
                match moved_route(&line, seg, k, grid).filter(|_| k != 0) {
                    Some(route) => ToolOutput::Commit(route_command(connection, &route)),
                    None => SelectTool::click_entity(connection, held),
                }
            }
        }
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        match self.state.clone() {
            State::Idle | State::Moving { .. } => ToolOutput::None,
            State::Segment {
                connection,
                mods,
                line,
                seg,
                from,
                ..
            } => {
                let steps = segment_steps(cx, &line, seg, from, at);
                let out = ToolOutput::Transient(segment_overlay(cx, &line, seg, steps));
                self.state = State::Segment {
                    connection,
                    mods,
                    line,
                    seg,
                    from,
                    steps,
                };
                out
            }
            State::Link { at: from, .. } => {
                if !cx.within(from, at, CLICK_SLOP_PT) {
                    self.state = State::Idle;
                }
                ToolOutput::None
            }
            State::Pressed {
                entity,
                at: from,
                mods,
                movable,
            } => {
                if cx.within(from, at, CLICK_SLOP_PT) {
                    return ToolOutput::None;
                }
                if movable {
                    let moved = moved_set(cx, entity);
                    let steps = steps_between(cx, from, at);
                    let out = self.move_out(cx, &moved, steps);
                    self.origin = (cx.selection.generation, cx.page.rev);
                    self.state = State::Dragging {
                        pressed: entity,
                        mods,
                        moved,
                        from,
                        steps,
                        readout: readout(cx, steps),
                    };
                    return out;
                }
                self.state = State::Band { from, to: at, mods };
                SelectTool::band(from, at)
            }
            State::Band { from, mods, .. } => {
                self.state = State::Band { from, to: at, mods };
                SelectTool::band(from, at)
            }
            State::Dragging {
                pressed,
                mods,
                moved,
                from,
                ..
            } => {
                let steps = steps_between(cx, from, at);
                let out = self.move_out(cx, &moved, steps);
                self.state = State::Dragging {
                    pressed,
                    mods,
                    moved,
                    from,
                    steps,
                    readout: readout(cx, steps),
                };
                out
            }
        }
    }

    fn key(&mut self, key: ToolKey, cx: &ToolCx<'_>) -> ToolOutput {
        if self.state != State::Idle {
            return ToolOutput::None;
        }
        let reps = selected_reps(cx.selection, cx.page);
        let orient = match reps.as_slice() {
            [one] => cx
                .page
                .tables
                .reps
                .iter()
                .find(|r| r.id == one.0)
                .map(|r| r.orient),
            _ => None,
        };
        match transform_command(key, &reps, orient) {
            Some(cmd) => ToolOutput::Commit(cmd),
            None => ToolOutput::None,
        }
    }
}

impl Tool for SelectTool {
    fn name(&self) -> &'static str {
        "select"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Idle | State::Link { .. } => "idle",
            State::Pressed { .. } => "pressed",
            State::Band { .. } => "rubber-band",
            State::Dragging { .. } | State::Segment { .. } => "dragging",
            State::Moving { .. } => "moving",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                if matches!(self.state, State::Moving { .. }) {
                    return ToolOutput::None;
                }
                self.state = State::Idle;
                self.press(*at, *mods, cx)
            }
            CanvasEvent::Release {
                at,
                button: Button::Primary,
                mods,
            } => self.release(*at, *mods, cx),
            CanvasEvent::Press { .. } | CanvasEvent::Release { .. } => ToolOutput::None,
            CanvasEvent::PointerMove { at, .. } => self.moved(*at, cx),
            CanvasEvent::CursorMove { at } => {
                self.cursor = Some(*at);
                if let State::Moving { moved, from, .. } = self.state.clone() {
                    let steps = steps_between(cx, from, *at);
                    let out = self.move_out(cx, &moved, steps);
                    self.state = State::Moving {
                        moved,
                        from,
                        steps,
                        readout: readout(cx, steps),
                    };
                    return out;
                }
                ToolOutput::None
            }
            CanvasEvent::Activate => {
                if let State::Moving { moved, steps, .. } = self.state.clone() {
                    self.state = State::Idle;
                    return if steps == (0, 0) {
                        ToolOutput::Cancel
                    } else {
                        ToolOutput::Commit(move_command(&moved, steps))
                    };
                }
                let Some(at) = self.cursor else {
                    return ToolOutput::None;
                };
                self.state = State::Idle;
                match self.press(at, Mods::default(), cx) {
                    ToolOutput::None | ToolOutput::Transient(_) => self.release(at, Mods::default(), cx),
                    other => other,
                }
            }
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match &self.state {
                State::Dragging { moved, steps, .. } | State::Moving { moved, steps, .. }
                    if *steps != (0, 0) =>
                {
                    ToolOutput::Preview(move_command(moved, *steps))
                }
                State::Segment {
                    connection,
                    line,
                    seg,
                    steps,
                    ..
                } if *steps != 0 => {
                    let grid = PageGridMap::new(cx.page.tables.grid);
                    match moved_route(line, *seg, *steps, grid) {
                        Some(route) => ToolOutput::Preview(route_command(*connection, &route)),
                        None => ToolOutput::None,
                    }
                }
                _ => ToolOutput::None,
            },
            CanvasEvent::Escape => {
                if self.state == State::Idle {
                    ToolOutput::None
                } else {
                    self.state = State::Idle;
                    ToolOutput::Cancel
                }
            }
            CanvasEvent::Key(key) => self.key(*key, cx),
            CanvasEvent::Dwell { .. } | CanvasEvent::Back | CanvasEvent::Text(_) => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Dragging { .. } | State::Moving { .. } | State::Segment { .. } => "hint-select-move",
            _ => "hint-select",
        }
    }

    fn wants_hover(&self) -> bool {
        true
    }

    fn ghost(&self) -> Option<Ghost> {
        match &self.state {
            State::Dragging {
                moved,
                steps,
                readout,
                ..
            }
            | State::Moving {
                moved,
                steps,
                readout,
                ..
            } => Some(Ghost::Move {
                count: moved.len(),
                steps: *steps,
                readout: readout.clone(),
                proposals: self.proposals.clone(),
            }),
            _ => None,
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_api::output::ew_dl::ew_units::{Nm, Pt2};

    fn rect(x0: i64, y0: i64, x1: i64, y1: i64) -> Rect {
        Rect::from_points(Pt2::new(Nm(x0), Nm(y0)), Pt2::new(Nm(x1), Nm(y1)))
    }

    fn e(n: u128) -> EntityRef {
        EntityRef(format!("{n:032x}").parse().unwrap())
    }

    /// FT-439: a band lists what it selects in reading order of each entity's centre (y, then x), ties by ID; never in
    /// paint order, which follows the IDs.
    #[test]
    fn a_band_selects_in_reading_order_not_in_paint_order() {
        // Painted by ID: 1 (lower right), 2 (upper left, two rectangles), 3 (upper right), 4 and 5 at one point.
        let entities = vec![
            (e(1), rect(50, 50, 60, 60)),
            (e(2), rect(0, 0, 10, 10)),
            (e(3), rect(50, 0, 60, 10)),
            (e(2), rect(0, 20, 10, 30)),
            (e(5), rect(70, 70, 80, 80)),
            (e(4), rect(70, 70, 80, 80)),
        ];
        // Entity 2's centre is that of its two rectangles together, (5, 15): after 3's (55, 5).
        assert_eq!(
            band_selection(&entities, rect(-5, -5, 100, 100), true),
            [e(3), e(2), e(1), e(4), e(5)]
        );
        assert_eq!(
            band_selection(&entities, rect(-5, -5, 100, 12), false),
            [e(3), e(2)]
        );
        assert_eq!(band_selection(&entities, rect(-5, -5, 100, 12), true), [e(3)]);
    }
}
