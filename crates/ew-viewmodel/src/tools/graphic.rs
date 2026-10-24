// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Graphic tool (`graphic`; WP-S2.04 §10.5, §10.7, FR-SYM-023): lines, polylines, rectangles, circles, arcs and
//! polygons, by fine-snapped points, committed as `item.place` graphics.
//!
//! **The states.** All points are page nm, fine-snapped ([`crate::tools::Snapper::snap_fine`],
//! `FineKind::Graphic`); the overlay is in sheet nm. A click is a primary `Press` at the snapped point or `Activate`
//! at the current one; `Back` removes the last fixed point; `tool.finish` ends a polyline (two distinct points) or a
//! polygon (three).
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | move (`PointerMove`, `CursorMove`) | armed | `Transient`: a grid marker |
//! | armed | a click at p | drawing (`points [p]`, at p) | `Transient` |
//! | drawing | move to p | drawing (at p) | `Transient` |
//! | drawing, Line | a click at p ≠ `points[0]` | armed (at p) | `Commit` line `{a: points[0], b: p}` |
//! | drawing, Rect | a click at p with p.x ≠ `points[0]`.x and p.y ≠ `points[0]`.y | armed | `Commit` rect `{min, max}` |
//! | drawing, Circle | a click at p with `round_distance(points[0], p) > 0` | armed | `Commit` circle `{centre, r}` |
//! | drawing, Arc, one point | a click at p ≠ `points[0]` | drawing (`points [c, p]`) | `Transient` |
//! | drawing, Arc, two points | a click at p | armed | `Commit` arc when `r > 0` and the sweep is not 0, else `None` |
//! | drawing, Polyline or Polygon | a click at p ≠ the last point | drawing (p appended) | `Transient` |
//! | drawing, Polyline or Polygon | `Key(Finish)` with enough distinct points | armed | `Commit` `{points}` |
//! | drawing | `Key(Finish)` otherwise; a click that does not satisfy its row | drawing | `None` |
//! | drawing | `Back` | drawing with the last point removed; with none left: armed (at kept) | `Transient`; `Cancel` |
//! | armed or drawing, Arc | `Key(ToggleDirection)` (D) | same | the sweep's direction toggled; `Transient` |
//! | armed or drawing, not Arc | `Key(ToggleDirection)` | same | `None` |
//! | drawing | `Dwell { ms ≥ 80 }` | drawing | `Preview` of the command a click at `at` would commit now |
//! | drawing | `Escape` | armed (at kept) | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! **The arc's direction.** The tool draws its sweep clockwise by default (negative on the sheet) and anticlockwise
//! while the tool's toggle (D) is set or Shift is held on the end press; the tool strip and the ghost say which.

use ew_api::AnyCommand;
use serde_json::{Value, json};

use super::{
    Button, CanvasEvent, FineKind, Ghost, MarkerKind, Overlay, OverlayItem, Style, Token, Tool, ToolCx,
    ToolKey, ToolOutput, rect_of,
};

/// The six shapes of the Graphic tool, in the Insert ▸ Graphic order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphicShape {
    /// A straight line.
    Line,
    /// An open polyline.
    Polyline,
    /// An axis-parallel rectangle.
    Rect,
    /// A circle.
    Circle,
    /// A circular arc.
    Arc,
    /// A closed polygon.
    Polygon,
}

impl GraphicShape {
    /// The shapes in the order of the menu.
    pub const ALL: [GraphicShape; 6] = [
        GraphicShape::Line,
        GraphicShape::Polyline,
        GraphicShape::Rect,
        GraphicShape::Circle,
        GraphicShape::Arc,
        GraphicShape::Polygon,
    ];

    /// The shape's word: the menu item's suffix, the ghost's `shape` and `item.place`'s parameter.
    pub fn word(self) -> &'static str {
        match self {
            GraphicShape::Line => "line",
            GraphicShape::Polyline => "polyline",
            GraphicShape::Rect => "rect",
            GraphicShape::Circle => "circle",
            GraphicShape::Arc => "arc",
            GraphicShape::Polygon => "polygon",
        }
    }

    /// The shape of its word.
    pub fn of_word(w: &str) -> Option<GraphicShape> {
        GraphicShape::ALL.into_iter().find(|s| s.word() == w)
    }
}

/// The angle of `p` seen from `c` in 10⁻³ degree, counter-clockwise on the sheet (y grows downwards, so a point above
/// `c` is 90000), in `0..360_000`: `round(atan2(-(p.y - c.y), p.x - c.x) in degrees × 1000)` (f64, halves away from
/// zero), `rem_euclid(360_000)`; 0 for `p == c`.
pub fn angle_mdeg(c: (i64, i64), p: (i64, i64)) -> i32 {
    if p == c {
        return 0;
    }
    let dx = p.0 as f64 - c.0 as f64;
    let dy = p.1 as f64 - c.1 as f64;
    let deg = (-dy).atan2(dx).to_degrees() * 1000.0;
    (deg.round() as i64).rem_euclid(360_000) as i32
}

/// The integer square root of a non-negative `n`.
fn isqrt(n: i128) -> i128 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// The distance of two points rounded to the nearest nm (exact: `r = isqrt(d²)` in `i128`, plus 1 when `d² − r² > r`).
pub fn round_distance(a: (i64, i64), b: (i64, i64)) -> i64 {
    let dx = i128::from(b.0) - i128::from(a.0);
    let dy = i128::from(b.1) - i128::from(a.1);
    let d2 = dx * dx + dy * dy;
    let r = isqrt(d2);
    let r = if d2 - r * r > r { r + 1 } else { r };
    i64::try_from(r).unwrap_or(i64::MAX)
}

/// The number of distinct points.
fn distinct(points: &[(i64, i64)]) -> usize {
    points.iter().collect::<std::collections::BTreeSet<_>>().len()
}

/// The Graphic tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    /// No point fixed yet, or the point the next shape starts at.
    Armed { at: Option<(i64, i64)> },
    /// The fixed points and the current one (page nm).
    Drawing { points: Vec<(i64, i64)>, at: (i64, i64) },
}

/// The Graphic tool (L: a line; Insert ▸ Graphic for the others).
#[derive(Clone, Debug)]
pub struct GraphicTool {
    shape: GraphicShape,
    state: State,
    anticlockwise: bool,
}

impl GraphicTool {
    /// The tool, armed, drawing `shape`.
    pub fn new(shape: GraphicShape) -> GraphicTool {
        GraphicTool {
            shape,
            state: State::Armed { at: None },
            anticlockwise: false,
        }
    }

    /// The shape it draws.
    pub fn shape(&self) -> GraphicShape {
        self.shape
    }

    /// The fixed points of the shape in progress (`0` while armed).
    fn fixed(&self) -> &[(i64, i64)] {
        match &self.state {
            State::Armed { .. } => &[],
            State::Drawing { points, .. } => points,
        }
    }

    /// The current point (page nm).
    fn point(&self) -> Option<(i64, i64)> {
        match self.state {
            State::Armed { at } => at,
            State::Drawing { at, .. } => Some(at),
        }
    }

    /// The overlay of the state: the marker, and the shape in progress dashed.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let Some(at) = self.point() else {
            return Overlay::default();
        };
        let sheet = |p: (i64, i64)| cx.snapper.page_to_sheet(p);
        let mut items = Vec::new();
        if let State::Drawing { points, .. } = &self.state {
            let style = Style::dashed(Token::Ghost);
            let p0 = points[0];
            match self.shape {
                GraphicShape::Line => items.push(OverlayItem::Line {
                    a: sheet(p0),
                    b: sheet(at),
                    style,
                }),
                GraphicShape::Rect => items.push(OverlayItem::Polyline {
                    points: vec![
                        sheet(p0),
                        sheet((at.0, p0.1)),
                        sheet(at),
                        sheet((p0.0, at.1)),
                        sheet(p0),
                    ],
                    style,
                }),
                GraphicShape::Circle => {
                    let r = round_distance(p0, at);
                    items.push(OverlayItem::Rect {
                        r: rect_of(sheet((p0.0 - r, p0.1 - r)), sheet((p0.0 + r, p0.1 + r))),
                        style,
                    });
                    items.push(OverlayItem::Line {
                        a: sheet(p0),
                        b: sheet(at),
                        style,
                    });
                }
                GraphicShape::Arc => {
                    if let Some(s) = points.get(1) {
                        items.push(OverlayItem::Line {
                            a: sheet(p0),
                            b: sheet(*s),
                            style,
                        });
                    }
                    items.push(OverlayItem::Line {
                        a: sheet(p0),
                        b: sheet(at),
                        style,
                    });
                }
                GraphicShape::Polyline => {
                    let mut pts: Vec<(i64, i64)> = points.iter().map(|p| sheet(*p)).collect();
                    pts.push(sheet(at));
                    items.push(OverlayItem::Polyline { points: pts, style });
                }
                GraphicShape::Polygon => {
                    let mut pts: Vec<(i64, i64)> = points.iter().map(|p| sheet(*p)).collect();
                    pts.push(sheet(at));
                    pts.push(sheet(p0));
                    items.push(OverlayItem::Polyline { points: pts, style });
                }
            }
        }
        items.push(OverlayItem::Marker {
            at: sheet(at),
            kind: MarkerKind::Grid,
        });
        Overlay::new(items)
    }

    /// The `item.place` of `shape` (the `ShapeParam` object).
    fn command(&self, cx: &ToolCx<'_>, shape: Value) -> Box<AnyCommand> {
        Box::new(AnyCommand {
            name: "item.place".into(),
            params: json!({
                "page": cx.page.tables.page.to_string(),
                "kind": {"graphic": {"shape": shape}},
            }),
        })
    }

    /// The command a click at the current point would commit now.
    fn click_command(&self) -> Option<Value> {
        let State::Drawing { points, at } = &self.state else {
            return None;
        };
        let at = *at;
        let p0 = points[0];
        match self.shape {
            GraphicShape::Line if at != p0 => Some(json!({"line": {
                "a": [p0.0, p0.1], "b": [at.0, at.1]}})),
            GraphicShape::Rect if at.0 != p0.0 && at.1 != p0.1 => {
                Some(json!({"rect": {"min": [p0.0, p0.1], "max": [at.0, at.1]}}))
            }
            GraphicShape::Circle => {
                let r = round_distance(p0, at);
                (r > 0).then(|| json!({"circle": {"centre": [p0.0, p0.1], "r": r}}))
            }
            GraphicShape::Arc if points.len() == 1 => None,
            GraphicShape::Arc => {
                let s = points[1];
                let r = round_distance(p0, s);
                let a0 = angle_mdeg(p0, s);
                let a1 = angle_mdeg(p0, at);
                let ccw = (a1 - a0).rem_euclid(360_000);
                if r == 0 || ccw == 0 {
                    return None;
                }
                let sweep = if self.anticlockwise { ccw } else { ccw - 360_000 };
                Some(json!({"arc": {"centre": [p0.0, p0.1], "r": r, "start": a0, "sweep": sweep}}))
            }
            GraphicShape::Polyline | GraphicShape::Polygon => {
                let mut with_at = points.clone();
                if with_at.last() != Some(&at) {
                    with_at.push(at);
                }
                let n = distinct(&with_at);
                let enough = match self.shape {
                    GraphicShape::Polyline => n >= 2,
                    _ => n >= 3,
                };
                if !enough {
                    return None;
                }
                let pts: Vec<Value> = with_at.iter().map(|p| json!([p.0, p.1])).collect();
                let mut shape = serde_json::Map::new();
                shape.insert(self.shape.word().to_owned(), json!({"points": pts}));
                Some(Value::Object(shape))
            }
            _ => None,
        }
    }

    /// A click: start the shape, or extend or commit it.
    fn click(&mut self, shift: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Drawing { points, at } = self.state.clone() else {
            let State::Armed { at: Some(p) } = self.state else {
                return ToolOutput::None;
            };
            self.state = State::Drawing {
                points: vec![p],
                at: p,
            };
            return ToolOutput::Transient(self.overlay(cx));
        };
        let p0 = points[0];
        match self.shape {
            GraphicShape::Line if at != p0 => {
                self.commit(cx, json!({"line": {"a": [p0.0, p0.1], "b": [at.0, at.1]}}), at)
            }
            GraphicShape::Rect if at.0 != p0.0 && at.1 != p0.1 => self.commit(
                cx,
                json!({"rect": {"min": [p0.0, p0.1], "max": [at.0, at.1]}}),
                at,
            ),
            GraphicShape::Circle => {
                let r = round_distance(p0, at);
                if r > 0 {
                    self.commit(cx, json!({"circle": {"centre": [p0.0, p0.1], "r": r}}), at)
                } else {
                    ToolOutput::None
                }
            }
            GraphicShape::Arc if points.len() == 1 => {
                if at == p0 {
                    return ToolOutput::None;
                }
                self.state = State::Drawing {
                    points: vec![p0, at],
                    at,
                };
                ToolOutput::Transient(self.overlay(cx))
            }
            GraphicShape::Arc => {
                let s = points[1];
                let r = round_distance(p0, s);
                let a0 = angle_mdeg(p0, s);
                let a1 = angle_mdeg(p0, at);
                let ccw = (a1 - a0).rem_euclid(360_000);
                if r == 0 || ccw == 0 {
                    return ToolOutput::None;
                }
                let sweep = if self.anticlockwise || shift {
                    ccw
                } else {
                    ccw - 360_000
                };
                self.commit(
                    cx,
                    json!({"arc": {"centre": [p0.0, p0.1], "r": r, "start": a0, "sweep": sweep}}),
                    at,
                )
            }
            GraphicShape::Polyline | GraphicShape::Polygon => {
                if points.last() == Some(&at) {
                    return ToolOutput::None;
                }
                let mut points = points;
                points.push(at);
                self.state = State::Drawing { points, at };
                ToolOutput::Transient(self.overlay(cx))
            }
            _ => ToolOutput::None,
        }
    }

    /// Commits the shape and goes armed at `at`.
    fn commit(&mut self, cx: &ToolCx<'_>, shape: Value, at: (i64, i64)) -> ToolOutput {
        self.state = State::Armed { at: Some(at) };
        self.anticlockwise = false;
        ToolOutput::Commit(self.command(cx, shape))
    }

    /// `Key(Finish)`: ends a polyline (two distinct points) or a polygon (three).
    fn finish(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Drawing { points, at } = self.state.clone() else {
            return ToolOutput::None;
        };
        let n = distinct(&points);
        let enough = match self.shape {
            GraphicShape::Polyline => n >= 2,
            GraphicShape::Polygon => n >= 3,
            _ => false,
        };
        if !enough {
            return ToolOutput::None;
        }
        let pts: Vec<Value> = points.iter().map(|p| json!([p.0, p.1])).collect();
        let mut shape = serde_json::Map::new();
        shape.insert(self.shape.word().to_owned(), json!({"points": pts}));
        self.commit(cx, Value::Object(shape), at)
    }

    /// The dwell: the command a click at the current point would commit now.
    fn preview(&self, cx: &ToolCx<'_>) -> ToolOutput {
        match self.click_command() {
            Some(shape) => ToolOutput::Preview(self.command(cx, shape)),
            None => ToolOutput::None,
        }
    }

    /// `Back`: removes the last fixed point; with none left, the tool is armed again.
    fn back(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Drawing { mut points, at } = self.state.clone() else {
            return ToolOutput::None;
        };
        points.pop();
        if points.is_empty() {
            self.state = State::Armed { at: Some(at) };
            self.anticlockwise = false;
            ToolOutput::Cancel
        } else {
            self.state = State::Drawing { points, at };
            ToolOutput::Transient(self.overlay(cx))
        }
    }

    /// A move to `at`: fine-snapped with the pointer's Alt.
    fn moved(&mut self, at: (i64, i64), alt: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.snap_fine(at, FineKind::Graphic, alt).1;
        match &mut self.state {
            State::Armed { at } => *at = Some(p),
            State::Drawing { at, .. } => *at = p,
        }
        ToolOutput::Transient(self.overlay(cx))
    }
}

impl Tool for GraphicTool {
    fn name(&self) -> &'static str {
        "graphic"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "armed",
            State::Drawing { .. } => "drawing",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, mods } => self.moved(*at, mods.alt, cx),
            CanvasEvent::CursorMove { at } => self.moved(*at, false, cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                let _ = self.moved(*at, mods.alt, cx);
                self.click(mods.shift, cx)
            }
            CanvasEvent::Activate => self.click(false, cx),
            CanvasEvent::Key(ToolKey::Finish) => self.finish(cx),
            CanvasEvent::Key(ToolKey::ToggleDirection) => {
                if self.shape == GraphicShape::Arc {
                    self.anticlockwise = !self.anticlockwise;
                    ToolOutput::Transient(self.overlay(cx))
                } else {
                    ToolOutput::None
                }
            }
            CanvasEvent::Back => self.back(cx),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => self.preview(cx),
            CanvasEvent::Escape => match self.state {
                State::Drawing { at, .. } => {
                    self.state = State::Armed { at: Some(at) };
                    self.anticlockwise = false;
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "hint-graphic-armed",
            State::Drawing { .. } => "hint-graphic-drawing",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Graphic {
            shape: self.shape.word(),
            points: self.fixed().len(),
            anticlockwise: self.anticlockwise,
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_mdeg_on_the_axes() {
        let c = (10, 20);
        assert_eq!(angle_mdeg(c, (30, 20)), 0);
        assert_eq!(angle_mdeg(c, (10, 0)), 90_000);
        assert_eq!(angle_mdeg(c, (-30, 20)), 180_000);
        assert_eq!(angle_mdeg(c, (10, 40)), 270_000);
        assert_eq!(angle_mdeg(c, c), 0);
    }

    #[test]
    fn round_distance_rounds_to_the_nearest_nm() {
        assert_eq!(round_distance((0, 0), (3, 4)), 5);
        assert_eq!(round_distance((0, 0), (1, 1)), 1);
        assert_eq!(round_distance((0, 0), (1, 2)), 2);
        assert_eq!(round_distance((0, 0), (0, 0)), 0);
    }
}
