// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Structure box tool (`structure_box`; WP-S2.04 §10.5, §10.7; FR-PRJ-020): a rectangle by two fine-snapped
//! corners or a polygon by its points, then its style, caption, location node and external reference in the tool
//! strip, committed as `box.structure`.
//!
//! **The states.** All points are page nm, fine-snapped ([`crate::tools::Snapper::snap_fine`], `FineKind::Graphic`);
//! the overlay is in sheet nm. A click is a primary `Press` at the snapped point or `Activate` at the current one;
//! `Back` removes the last fixed point; `tool.finish` closes the box. Two corners that differ in both axes make the
//! rectangle `(x0, y0) (x1, y0) (x1, y1) (x0, y1)` with `x0 < x1` and `y0 < y1`, top-left first and clockwise on the
//! sheet; three or more distinct points make the polygon as clicked. The fields persist across commits.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | move to p | armed (at p) | `Transient`: a grid marker |
//! | armed | a click at p | outline (`points [p]`, at p) | `Transient` |
//! | outline | move to p | outline (at p) | `Transient`: the closed rectangle `points[0]`–at, or the closed polyline points + at, dashed |
//! | outline | a click at p ≠ the last point | outline (p appended) | `Transient` |
//! | outline | `Key(Finish)` with exactly 2 points differing in x and in y | typing (the rectangle) | `Transient` |
//! | outline | `Key(Finish)` with ≥ 3 distinct points | typing (the points as clicked) | `Transient` |
//! | outline | `Key(Finish)` otherwise | outline | `None` |
//! | outline | `Back` | outline with the last point removed; none left: armed | `Transient`; from the last point `Cancel` |
//! | typing | `Activate` or `Key(Finish)` | armed (at = the outline's first vertex) | `Commit(box.structure …)` |
//! | typing | `Dwell { ms ≥ 80 }` | typing | `Preview(box.structure …)` |
//! | outline | `Dwell { ms ≥ 80 }` | outline | `None` |
//! | typing | any move, `Press`, `Release`, `Back` | typing | `None` |
//! | outline or typing | `Escape` | armed | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |

use ew_api::AnyCommand;
use serde_json::{Map, Value, json};

use super::{
    Button, CanvasEvent, FineKind, Ghost, MarkerKind, Overlay, OverlayItem, Style, Token, Tool, ToolCx,
    ToolKey, ToolOutput,
};

/// The distinct fixed points of `points`.
fn distinct(points: &[(i64, i64)]) -> usize {
    points.iter().collect::<std::collections::BTreeSet<_>>().len()
}

/// The structure box tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    /// No point fixed yet, or the point the next box starts at (page nm).
    Armed { at: Option<(i64, i64)> },
    /// The fixed points and the current one (page nm).
    Outline { points: Vec<(i64, i64)>, at: (i64, i64) },
    /// The finished outline, waiting for its fields and Enter (page nm).
    Typing { outline: Vec<(i64, i64)> },
}

/// The Structure box tool (Shift+B): a rectangle or polygon, its fields in the tool strip, Enter places the box.
#[derive(Clone, Debug)]
pub struct StructureBoxTool {
    state: State,
    style: &'static str,
    caption: Option<String>,
    location: Option<(String, String)>,
    external: Option<(String, String)>,
}

impl StructureBoxTool {
    /// The tool, armed, dotted, without caption, location or external reference.
    pub fn new() -> StructureBoxTool {
        StructureBoxTool {
            state: State::Armed { at: None },
            style: "dotted",
            caption: None,
            location: None,
            external: None,
        }
    }

    /// The line style: `dotted` or `dash_dot` (the host checks the word).
    pub fn set_style(&mut self, style: &'static str) {
        self.style = style;
    }

    /// The caption; `None` or an empty text clears it.
    pub fn set_caption(&mut self, caption: Option<String>) {
        self.caption = caption.filter(|c| !c.is_empty());
    }

    /// The location node: its UUID text and the text the user gave.
    pub fn set_location(&mut self, location: Option<(String, String)>) {
        self.location = location;
    }

    /// The external reference: its UUID text and its printed text.
    pub fn set_external(&mut self, external: Option<(String, String)>) {
        self.external = external;
    }

    /// The fixed points of the box in progress.
    fn fixed(&self) -> &[(i64, i64)] {
        match &self.state {
            State::Armed { .. } => &[],
            State::Outline { points, .. } => points,
            State::Typing { outline } => outline,
        }
    }

    /// The current point (page nm).
    fn point(&self) -> Option<(i64, i64)> {
        match self.state {
            State::Armed { at } => at,
            State::Outline { at, .. } => Some(at),
            State::Typing { .. } => None,
        }
    }

    /// The closed outline of `points`: the first vertex again at the end.
    fn closed(points: &[(i64, i64)]) -> Vec<(i64, i64)> {
        let mut out = points.to_vec();
        if let Some(first) = points.first() {
            out.push(*first);
        }
        out
    }

    /// The overlay of the state: the grid marker, and the box in progress dashed.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let sheet = |p: (i64, i64)| cx.snapper.page_to_sheet(p);
        let mut items = Vec::new();
        match &self.state {
            State::Outline { points, at } => {
                let mut pts: Vec<(i64, i64)> = points.iter().map(|p| sheet(*p)).collect();
                if points.len() == 1 {
                    let p0 = points[0];
                    pts = vec![
                        sheet(p0),
                        sheet((at.0, p0.1)),
                        sheet(*at),
                        sheet((p0.0, at.1)),
                        sheet(p0),
                    ];
                } else {
                    pts.push(sheet(*at));
                    if let Some(first) = pts.first().copied() {
                        pts.push(first);
                    }
                }
                items.push(OverlayItem::Polyline {
                    points: pts,
                    style: Style::dashed(Token::Ghost),
                });
            }
            State::Typing { outline } => {
                let pts: Vec<(i64, i64)> = Self::closed(outline).iter().map(|p| sheet(*p)).collect();
                items.push(OverlayItem::Polyline {
                    points: pts,
                    style: Style::dashed(Token::Ghost),
                });
            }
            State::Armed { .. } => {}
        }
        let Some(at) = self.point() else {
            return Overlay::new(items);
        };
        items.push(OverlayItem::Marker {
            at: sheet(at),
            kind: MarkerKind::Grid,
        });
        Overlay::new(items)
    }

    /// The overlay at the current point, after a field changed.
    pub fn refresh(&mut self, cx: &ToolCx<'_>) -> Overlay {
        self.overlay(cx)
    }

    /// The `box.structure` of the finished outline.
    fn command(&self, cx: &ToolCx<'_>, outline: &[(i64, i64)]) -> Box<AnyCommand> {
        let points: Vec<Value> = outline.iter().map(|p| json!([p.0, p.1])).collect();
        let mut assigns = Map::new();
        if let Some((id, _)) = &self.location {
            assigns.insert("location".into(), Value::from(id.clone()));
        }
        let mut params = Map::new();
        params.insert("page".into(), Value::from(cx.page.tables.page.to_string()));
        params.insert("outline".into(), Value::Array(points));
        params.insert("style".into(), Value::from(self.style));
        params.insert("assigns".into(), Value::Object(assigns));
        if let Some(caption) = &self.caption
            && !caption.is_empty()
        {
            params.insert("caption".into(), Value::from(caption.clone()));
        }
        if let Some((id, _)) = &self.external {
            params.insert("external".into(), Value::from(id.clone()));
        }
        Box::new(AnyCommand {
            name: "box.structure".into(),
            params: Value::Object(params),
        })
    }

    /// A move to `at`: fine-snapped with the pointer's Alt (typing ignores moves).
    fn moved(&mut self, at: (i64, i64), alt: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.snap_fine(at, FineKind::Graphic, alt).1;
        match &mut self.state {
            State::Armed { at } => *at = Some(p),
            State::Outline { at, .. } => *at = p,
            State::Typing { .. } => return ToolOutput::None,
        }
        ToolOutput::Transient(self.overlay(cx))
    }

    /// A click at the current point.
    fn click(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        match &self.state {
            State::Armed { at: Some(p) } => {
                self.state = State::Outline {
                    points: vec![*p],
                    at: *p,
                };
                ToolOutput::Transient(self.overlay(cx))
            }
            State::Armed { at: None } => ToolOutput::None,
            State::Outline { points, at } => {
                if points.last() == Some(at) {
                    return ToolOutput::None;
                }
                let p = *at;
                let mut points = points.clone();
                points.push(p);
                self.state = State::Outline { points, at: p };
                ToolOutput::Transient(self.overlay(cx))
            }
            State::Typing { .. } => ToolOutput::None,
        }
    }

    /// `Key(Finish)`: closes the outline into the typing state.
    fn finish(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Outline { points, .. } = &self.state else {
            return ToolOutput::None;
        };
        let outline: Vec<(i64, i64)> =
            if points.len() == 2 && points[0].0 != points[1].0 && points[0].1 != points[1].1 {
                let (a, b) = (points[0], points[1]);
                let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
                let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
                vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
            } else if distinct(points) >= 3 {
                points.clone()
            } else {
                return ToolOutput::None;
            };
        self.state = State::Typing { outline };
        ToolOutput::Transient(self.overlay(cx))
    }

    /// `Back`: removes the last fixed point; with none left, the tool is armed again.
    fn back(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Outline { mut points, at } = self.state.clone() else {
            return ToolOutput::None;
        };
        points.pop();
        if points.is_empty() {
            self.state = State::Armed { at: Some(at) };
            ToolOutput::Cancel
        } else {
            self.state = State::Outline { points, at };
            ToolOutput::Transient(self.overlay(cx))
        }
    }

    /// The dwell: the typing box's preview.
    fn preview(&self, cx: &ToolCx<'_>) -> ToolOutput {
        match &self.state {
            State::Typing { outline } => ToolOutput::Preview(self.command(cx, outline)),
            _ => ToolOutput::None,
        }
    }
}

impl Default for StructureBoxTool {
    fn default() -> StructureBoxTool {
        StructureBoxTool::new()
    }
}

impl Tool for StructureBoxTool {
    fn name(&self) -> &'static str {
        "structure_box"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "armed",
            State::Outline { .. } => "outline",
            State::Typing { .. } => "typing",
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
                self.click(cx)
            }
            CanvasEvent::Activate => match &self.state {
                State::Typing { outline } => {
                    let outline = outline.clone();
                    let at = outline.first().copied();
                    self.state = State::Armed { at };
                    ToolOutput::Commit(self.command(cx, &outline))
                }
                _ => self.click(cx),
            },
            CanvasEvent::Key(ToolKey::Finish) => match &self.state {
                State::Typing { outline } => {
                    let outline = outline.clone();
                    let at = outline.first().copied();
                    self.state = State::Armed { at };
                    ToolOutput::Commit(self.command(cx, &outline))
                }
                _ => self.finish(cx),
            },
            CanvasEvent::Back => self.back(cx),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => self.preview(cx),
            CanvasEvent::Escape => match &self.state {
                State::Outline { at, .. } => {
                    self.state = State::Armed { at: Some(*at) };
                    ToolOutput::Cancel
                }
                State::Typing { outline } => {
                    self.state = State::Armed {
                        at: outline.first().copied(),
                    };
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "hint-structure-box-armed",
            State::Outline { .. } => "hint-structure-box-outline",
            State::Typing { .. } => "hint-structure-box-typing",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    /// A double click closes the box (the drawing hint says so).
    fn finishes_on_double_click(&self) -> bool {
        true
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::StructureBox {
            outline: self.fixed().len(),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
