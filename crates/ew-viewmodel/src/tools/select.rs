// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Select tool (`select`; WP-S2.03 §7.6, doc 06 §4.3 without the drag-move rows, which are WP-S2.04's).
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | idle | press on a link | idle | `Follow` on release within 3 points |
//! | idle | press on an entity | pressed | `None` |
//! | idle | press on empty canvas | rubber-band | `Transient` (the band) |
//! | pressed | release within 3 points | idle | `Select`: replace with the entity; Shift adds; Ctrl toggles |
//! | pressed | move beyond 3 points | rubber-band | `Transient` |
//! | rubber-band | move | rubber-band | `Transient`: solid while the drag goes right (encloses), dashed left (touches) |
//! | rubber-band | release | idle | `Select`: left to right the entities inside the band, right to left those that touch it; Shift adds |
//! | any | `Activate` | idle | as a press and release at the grid cursor |
//! | any | `Escape` | idle | `Cancel`, or `None` from idle |
//!
//! Picking uses WP-S2.02's entity rectangles: the smallest containing the point inflated by 4 points, ties to the
//! later in paint order. An entity may have several rectangles (nested items of one record): it lies inside a band
//! when all of them do, and touches it when one of them does; a band selects in paint order of each entity's first
//! rectangle. A release on empty canvas within 3 points is a click on nothing: it clears the selection (Shift keeps
//! it). A press on a link that moves beyond 3 points is dropped.

use ew_api::output::ew_dl::ew_units::Rect;
use ew_api::output::ew_dl::{Destination, EntityRef};

use super::{
    Button, CanvasEvent, Mods, Overlay, OverlayItem, SelectionChange, Style, Token, Tool, ToolCx, ToolOutput,
    rect_of,
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
    },
    /// A rubber band from `from` to `to`.
    Band {
        from: (i64, i64),
        to: (i64, i64),
        mods: Mods,
    },
}

/// The Select tool.
#[derive(Clone, Debug)]
pub struct SelectTool {
    state: State,
    /// The grid cursor as the last `CursorMove` placed it.
    cursor: Option<(i64, i64)>,
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
/// those with a rectangle that touches it; in paint order of each entity's first rectangle.
pub fn band_selection(entities: &[(EntityRef, Rect)], band: Rect, encloses: bool) -> Vec<EntityRef> {
    let mut order: Vec<EntityRef> = Vec::new();
    for (e, _) in entities {
        if !order.contains(e) {
            order.push(*e);
        }
    }
    order
        .into_iter()
        .filter(|e| {
            let mut rects = entities.iter().filter(|(x, _)| x == e).map(|(_, r)| *r);
            if encloses {
                rects.all(|r| inside(r, band))
            } else {
                rects.any(|r| touches(r, band))
            }
        })
        .collect()
}

impl SelectTool {
    /// The tool in its first state.
    pub fn new() -> SelectTool {
        SelectTool {
            state: State::Idle,
            cursor: None,
        }
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

    fn press(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        if let Some(link) = cx.link_at(at) {
            self.state = State::Link { dest: link.dest, at };
            return ToolOutput::None;
        }
        if let Some(entity) = cx.entity_at(at) {
            self.state = State::Pressed { entity, at, mods };
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
        }
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        match self.state.clone() {
            State::Idle => ToolOutput::None,
            State::Link { at: from, .. } => {
                if !cx.within(from, at, CLICK_SLOP_PT) {
                    self.state = State::Idle;
                }
                ToolOutput::None
            }
            State::Pressed { at: from, mods, .. } => {
                if cx.within(from, at, CLICK_SLOP_PT) {
                    return ToolOutput::None;
                }
                self.state = State::Band { from, to: at, mods };
                SelectTool::band(from, at)
            }
            State::Band { from, mods, .. } => {
                self.state = State::Band { from, to: at, mods };
                SelectTool::band(from, at)
            }
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
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match *ev {
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                self.state = State::Idle;
                self.press(at, mods, cx)
            }
            CanvasEvent::Release {
                at,
                button: Button::Primary,
                mods,
            } => self.release(at, mods, cx),
            CanvasEvent::Press { .. } | CanvasEvent::Release { .. } => ToolOutput::None,
            CanvasEvent::PointerMove { at, .. } => self.moved(at, cx),
            CanvasEvent::CursorMove { at } => {
                self.cursor = Some(at);
                ToolOutput::None
            }
            CanvasEvent::Activate => {
                let Some(at) = self.cursor else {
                    return ToolOutput::None;
                };
                self.state = State::Idle;
                match self.press(at, Mods::default(), cx) {
                    ToolOutput::None | ToolOutput::Transient(_) => self.release(at, Mods::default(), cx),
                    other => other,
                }
            }
            CanvasEvent::Escape => {
                if self.state == State::Idle {
                    ToolOutput::None
                } else {
                    self.state = State::Idle;
                    ToolOutput::Cancel
                }
            }
            CanvasEvent::Dwell { .. } | CanvasEvent::Back => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-select"
    }

    fn wants_hover(&self) -> bool {
        true
    }
}
