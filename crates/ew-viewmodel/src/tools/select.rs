// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Select tool (`select`; WP-S2.03 §7.6 with WP-S2.04 §7.3's drag rows and keyboard move and §7.4's
//! transforms; doc 06 §4.3).
//!
//! A press is classified by the pick of WP-S2.04 §6.6 in this order: a link; a segment of a **selected** connection
//! that is neither its first nor its last segment (part 2, §8.4); a **movable** entity (a representation, and from
//! WP-S2.04 part 4b a page item or a structure box); any other entity (a connection, a junction); empty canvas.
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
//! | pressed (movable) | move beyond 3 points | dragging | `Transient`: the ghost of the moved entities' rectangles offset by the step displacement, rubber bands and the readout; the moved set is the selection's movable entities when the pressed entity is selected, else the pressed entity alone |
//! | pressed (other) | move beyond 3 points | rubber-band | `Transient` |
//! | rubber-band | move | rubber-band | `Transient`: solid while the drag goes right (encloses), dashed left (touches) |
//! | rubber-band | release | idle | `Select`: left to right the entities inside the band, right to left those that touch it; Shift adds |
//! | dragging | move | dragging | `Transient`; steps `k = round(page displacement × den / num)` per axis |
//! | dragging | `Dwell { ms ≥ 80 }` | dragging | `Preview(rep.move {reps, items, steps})` when the steps are not zero |
//! | dragging | release | idle | `Commit(rep.move {reps, items, steps})` when the steps are not zero, else `Select` (as a click) |
//! | dragging or moving | `Escape` | idle | `Cancel` |
//! | idle with movable entities selected | [`SelectTool::start_move`] (M) | moving | `Transient`; the host puts the grid cursor on the primary entity's anchor first |
//! | moving | `CursorMove` | moving | steps = the cursor's grid displacement since M; `Transient` |
//! | moving | `Dwell`, `Activate` | moving, idle | as dragging: `Preview`, `Commit` (zero steps: `Cancel`) |
//! | idle with representations selected | `Key(RotateCw, RotateCcw, MirrorX, MirrorY, NextVariant)` | idle | `Commit(rep.transform …)` (§7.4; V on exactly one) |
//! | any | `Activate` | idle | as a press and release at the grid cursor |
//! | any | `Escape` | idle | `Cancel`, or `None` from idle |
//!
//! **WP-S2.04 part 3** (§9.4): with exactly one black box selected, a press within 4 points of one of its grips (a
//! vertex first, then a box CP, then an edge's midpoint) drags it (`dragging`): a move shows the reshaped outline, a
//! dwell previews and a release after a move commits `box.reshape` (a grip without `cps`, so the CPs re-project; a
//! CP with `cps`, the CP at the grid point of the outline nearest to the pointer); a release without a move is a
//! click on the box; Escape cancels.
//!
//! Connections and junctions in a selection are never moved; the movable entities are representations, page items and
//! structure boxes. Picking uses WP-S2.02's entity rectangles (an entity may have several: it lies inside a band
//! when all do, and touches it when one does) plus the exact segment distance of connections and structure-box
//! outlines (§6.6). A release on empty canvas within 3 points clears the selection (Shift keeps it).
//!
//! **FT-500** (picking where routes overlap). The pick is the first of the pick stack ([`super::pick_stack`]): a
//! wire's drawn line goes before the representation rectangle the press is in when it is strictly nearer than that
//! rectangle's centre. A press there that becomes a drag still moves the representation. A plain click repeated within
//! 3 points of the last plain click's press, on the same page and revision, while the selection is still exactly what
//! that click selected, selects the next entity of the pick stack, the last wrapping to the first; this also holds for
//! the click that a press on a selected connection's segment becomes, and for `Activate`. Shift and Ctrl clicks never
//! walk, and any other release ends the walk.

use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::output::ew_dl::{Destination, EntityRef};

use super::black_box;
use super::move_keys::{move_command, move_overlay, moved_set, readout, steps_between};
use super::route_edit::{moved_route, route_command, segment_at, segment_overlay, segment_steps};
use super::transform::{selected_movable, selected_reps, transform_command};
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
        /// The entity a drag from here moves: the pressed entity when it is movable, the representation whose
        /// rectangle the press is in when a wire's line took the press (FT-500), else none (a drag is a band).
        drag: Option<EntityRef>,
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
    /// A drag of a grip of the selected black box (WP-S2.04 §9.4): `to` the grid point it is at.
    Grip {
        rep: EntityRef,
        grip: usize,
        to: Option<(i64, i64)>,
    },
    /// A drag of a CP of the selected black box along its outline (WP-S2.04 §9.4).
    BoxCp {
        rep: EntityRef,
        idx: u16,
        to: Option<((i64, i64), ew_api::geometry::Dir)>,
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
    /// The last plain click and what it selected, for the repeated click that walks the pick stack (FT-500).
    last: Option<LastClick>,
}

/// A plain click (no Shift, no Ctrl) of the Select tool: where it pressed, on which page and revision, and the entity
/// it selected.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct LastClick {
    at: (i64, i64),
    page: ew_api::output::Uuid,
    rev: u64,
    picked: EntityRef,
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
            last: None,
        }
    }

    /// A click pressed at `at` on `entity` (the pick, or the selected connection whose segment was pressed). A plain
    /// click repeated where the last plain click pressed (within [`CLICK_SLOP_PT`]), on the same page and revision,
    /// while the selection is still exactly what that click selected, selects the entity after that one in the pick
    /// stack at `at`, the last wrapping to the first (FT-500); otherwise `entity`. Shift and Ctrl clicks never walk.
    fn click(
        &mut self,
        last: Option<LastClick>,
        entity: EntityRef,
        at: (i64, i64),
        mods: Mods,
        cx: &ToolCx<'_>,
    ) -> ToolOutput {
        if mods.shift || mods.ctrl {
            return SelectTool::click_entity(entity, mods);
        }
        let page = cx.page.tables.page;
        let repeat = last.filter(|l| {
            l.page == page
                && l.rev == cx.page.rev
                && cx.within(l.at, at, CLICK_SLOP_PT)
                && cx.selection.items.as_slice() == [l.picked]
        });
        let picked = match repeat {
            Some(l) => {
                let stack = cx.pick_stack(at);
                match stack.iter().position(|e| *e == l.picked) {
                    Some(i) => stack[(i + 1) % stack.len()],
                    None => entity,
                }
            }
            None => entity,
        };
        self.last = Some(LastClick {
            // A walk keeps the place of its first click, so that small pointer drift never moves it away.
            at: repeat.map_or(at, |l| l.at),
            page,
            rev: cx.page.rev,
            picked,
        });
        SelectTool::click_entity(picked, mods)
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
    /// its entities is gone (none of the representations, page items and structure boxes holds it). The host then
    /// escapes it, so no stale entity reaches a preview.
    pub fn pending_move_stale(&self, cx: &ToolCx<'_>) -> bool {
        let moved = match &self.state {
            State::Dragging { moved, .. } | State::Moving { moved, .. } => moved,
            State::Segment { .. } | State::Grip { .. } | State::BoxCp { .. } => {
                return self.origin.1 != cx.page.rev;
            }
            _ => return false,
        };
        let tables = &cx.page.tables;
        self.origin != (cx.selection.generation, cx.page.rev)
            || moved.iter().any(|e| {
                !tables.reps.iter().any(|r| r.id == e.0)
                    && !tables.items.iter().any(|i| i.id == e.0)
                    && !tables.structure_boxes.iter().any(|b| b.id == e.0)
            })
    }

    /// Starts the keyboard move (M, WP-S2.04 §7.3) of the selection's movable entities from the grid cursor and
    /// answers its first overlay; `None` (and nothing changes) without a movable entity selected, without a grid
    /// cursor or in another state than idle.
    pub fn start_move(&mut self, cx: &ToolCx<'_>) -> Option<ToolOutput> {
        if self.state != State::Idle {
            return None;
        }
        let moved = selected_movable(cx.selection, cx.page);
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

    /// A grip or a CP of the selected black box within 4 points of `at` (vertex grips first, then CPs, then the edges'
    /// midpoints; the nearest of each kind, ties by index).
    fn box_handle(cx: &ToolCx<'_>, at: (i64, i64)) -> Option<State> {
        let rep = black_box::selected_black_box(cx)?;
        let b = rep.black_box.as_ref()?;
        let n = b.outline.len();
        let near = |p: (i64, i64)| {
            let s = cx.snapper.page_to_sheet(p);
            cx.within(s, at, crate::canvas::HOVER_SLOP_PT)
                .then(|| super::dist2(s, at))
        };
        let grips = black_box::grip_points(&b.outline);
        let best = |range: std::ops::Range<usize>| {
            range
                .filter_map(|k| near(grips[k]).map(|d| (d, k)))
                .min()
                .map(|(_, k)| k)
        };
        let e = EntityRef(rep.id);
        if let Some(grip) = best(0..n) {
            return Some(State::Grip {
                rep: e,
                grip,
                to: None,
            });
        }
        if let Some((_, idx)) = b
            .cps
            .iter()
            .filter_map(|(idx, p, _)| near(*p).map(|d| (d, *idx)))
            .min()
        {
            return Some(State::BoxCp {
                rep: e,
                idx,
                to: None,
            });
        }
        best(n..2 * n).map(|grip| State::Grip {
            rep: e,
            grip,
            to: None,
        })
    }

    /// The command of a grip or CP drag at its current point; `Ok(None)` while it changes nothing or cannot be done,
    /// `Err` with the refusal the grip itself produces (an inside-out box).
    fn box_command(&self, cx: &ToolCx<'_>) -> Result<Option<Box<ew_api::AnyCommand>>, &'static str> {
        Ok(match &self.state {
            State::Grip { rep, grip, to } => {
                let Some(r) = black_box::black_box_rep(cx, *rep) else {
                    return Ok(None);
                };
                let Some(b) = r.black_box.as_ref() else {
                    return Ok(None);
                };
                let Some(to) = *to else { return Ok(None) };
                let new = black_box::reshape_by_grip(&b.outline, *grip, to)?;
                (new != b.outline).then(|| black_box::reshape_command(r, &new, None))
            }
            State::BoxCp { rep, idx, to } => {
                let Some(r) = black_box::black_box_rep(cx, *rep) else {
                    return Ok(None);
                };
                let Some(b) = r.black_box.as_ref() else {
                    return Ok(None);
                };
                let Some((p, dir)) = *to else { return Ok(None) };
                let mut cps = b.cps.clone();
                let Some(c) = cps.iter_mut().find(|c| c.0 == *idx) else {
                    return Ok(None);
                };
                if c.1 == p {
                    return Ok(None);
                }
                *c = (*idx, p, dir);
                Some(black_box::reshape_command(r, &b.outline, Some(&cps)))
            }
            _ => None,
        })
    }

    /// A grip or CP drag moved to `at`: the snapped point and the overlay of the reshaped box.
    fn box_drag(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.snap_grid(at).page_nm;
        let grid = PageGridMap::new(cx.page.tables.grid);
        let overlay = match &mut self.state {
            State::Grip { rep, grip, to } => {
                *to = Some(p);
                let Some(r) = black_box::black_box_rep(cx, *rep) else {
                    return ToolOutput::Transient(Overlay::default());
                };
                let Some(b) = r.black_box.as_ref() else {
                    return ToolOutput::Transient(Overlay::default());
                };
                let new = match black_box::reshape_by_grip(&b.outline, *grip, p) {
                    Ok(new) => new,
                    // An inside-out grip is the same refusal on the drag and on its release; the box stays as it is.
                    Err(reason_key) => return ToolOutput::Refused(reason_key),
                };
                Some(black_box::reshape_overlay(cx, &new, &[]))
            }
            State::BoxCp { rep, idx, to } => {
                let r = black_box::black_box_rep(cx, *rep);
                r.and_then(|r| {
                    let b = r.black_box.as_ref()?;
                    let hit = black_box::nearest_on_outline(grid, &b.outline, p)?;
                    *to = Some(hit);
                    let cps: Vec<(i64, i64)> = b
                        .cps
                        .iter()
                        .map(|c| if c.0 == *idx { hit.0 } else { c.1 })
                        .collect();
                    Some(black_box::reshape_overlay(cx, &b.outline, &cps))
                })
            }
            _ => None,
        };
        ToolOutput::Transient(overlay.unwrap_or_default())
    }

    fn press(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        if let Some(link) = cx.link_at(at) {
            self.state = State::Link { dest: link.dest, at };
            return ToolOutput::None;
        }
        // WP-S2.04 part 3: a grip or a CP of the selected black box.
        if let Some(state) = SelectTool::box_handle(cx, at) {
            self.origin = (cx.selection.generation, cx.page.rev);
            self.state = state;
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
        let stack = cx.pick_stack(at);
        if let Some(&entity) = stack.first() {
            let tables = &cx.page.tables;
            let movable = |e: EntityRef| {
                matches!(
                    entity_kind(tables, e),
                    Some(EntityKind::Rep | EntityKind::Item | EntityKind::StructureBox)
                )
            };
            // A wire's line picked inside a representation's rectangle (FT-500): a drag still moves the representation,
            // as before the wires took the press there.
            let drag = if movable(entity) {
                Some(entity)
            } else if entity_kind(tables, entity) == Some(EntityKind::Connection) {
                stack
                    .iter()
                    .copied()
                    .find(|e| entity_kind(tables, *e) != Some(EntityKind::Connection))
                    .filter(|e| entity_kind(tables, *e) == Some(EntityKind::Rep))
            } else {
                None
            };
            self.state = State::Pressed {
                entity,
                at,
                mods,
                drag,
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
        if matches!(self.state, State::Grip { .. } | State::BoxCp { .. }) {
            // A release without a move between is a click on the black box.
            let cmd = self.box_command(cx);
            let rep = match &self.state {
                State::Grip { rep, .. } | State::BoxCp { rep, .. } => *rep,
                _ => return ToolOutput::None,
            };
            self.state = State::Idle;
            self.last = None;
            return match cmd {
                Ok(Some(c)) => ToolOutput::Commit(c),
                Ok(None) => SelectTool::click_entity(rep, mods),
                Err(reason_key) => ToolOutput::Refused(reason_key),
            };
        }
        let state = std::mem::replace(&mut self.state, State::Idle);
        // Every release but a click forgets the last click; a click sets it again.
        let last = self.last.take();
        match state {
            State::Idle | State::Grip { .. } | State::BoxCp { .. } => ToolOutput::None,
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
                    self.click(last, entity, from, held, cx)
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
                    self.click(last, pressed, from, held, cx)
                } else {
                    ToolOutput::Commit(move_command(&cx.page.tables, &moved, steps))
                }
            }
            moving @ State::Moving { .. } => {
                self.state = moving;
                self.last = last;
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
                    None => self.click(last, connection, from, held, cx),
                }
            }
        }
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        match self.state.clone() {
            State::Idle | State::Moving { .. } => ToolOutput::None,
            State::Grip { .. } | State::BoxCp { .. } => self.box_drag(at, cx),
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
                drag,
            } => {
                if cx.within(from, at, CLICK_SLOP_PT) {
                    return ToolOutput::None;
                }
                if let Some(dragged) = drag {
                    let moved = moved_set(cx, dragged);
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
            State::Dragging { .. } | State::Segment { .. } | State::Grip { .. } | State::BoxCp { .. } => {
                "dragging"
            }
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
                        ToolOutput::Commit(move_command(&cx.page.tables, &moved, steps))
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
                    ToolOutput::Preview(move_command(&cx.page.tables, moved, *steps))
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
                State::Grip { .. } | State::BoxCp { .. } => match self.box_command(cx) {
                    Ok(Some(cmd)) => ToolOutput::Preview(cmd),
                    Ok(None) => ToolOutput::None,
                    Err(reason_key) => ToolOutput::Refused(reason_key),
                },
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
            State::Dragging { .. }
            | State::Moving { .. }
            | State::Segment { .. }
            | State::Grip { .. }
            | State::BoxCp { .. } => "hint-select-move",
            _ => "hint-select",
        }
    }

    fn wants_hover(&self) -> bool {
        true
    }

    /// A double click on a free text opens it for editing (the host turns `tool.finish` into text edit, §10.5).
    fn finishes_on_double_click(&self) -> bool {
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
