// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Edit frame tool (`frame`, doc 09 §3.9, architecture §9.2): the page's frame and its title block changed on the
//! page. It reads where the scene puts the title block's cells and lines and the rulers ([`ew_api::output::FrameFacts`]
//! of the page) and computes nothing of the frame itself.
//!
//! States: `idle` (nothing chosen: Properties shows the frame), `cell` and `cells` (one or several cells chosen: Shift
//! adds one), `rulers` (the column numbers or the row letters chosen), `line-drag` (a line between two cells dragged:
//! every cell on either side grows or shrinks with it, snapped to 0.5 mm, Alt free, never narrower than 2 mm). A press
//! hands the choice to the host ([`HostStep::FrameChoose`]), which shows it in Properties; a released drag hands the
//! line's new place ([`HostStep::FrameLine`]), which the host writes with `form.edit`. The drawing fades while the tool
//! is active: the overlay's first item asks the window for it.

use ew_api::output::{FrameFacts, SheetRect};

use super::{
    Anchor, Button, CanvasEvent, Hint, HostStep, Overlay, OverlayItem, Style, Token, Tool, ToolCx,
    ToolOutput, rect_of,
};
use crate::key;

/// What a person points at on the frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameHit {
    /// A cell of the title block, by its index.
    Cell(usize),
    /// A line between cells, by its index in the title block's lines.
    Line(usize),
    /// The column numbers or the row letters.
    Rulers,
}

/// What is chosen on the frame.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum FrameChoice {
    /// Nothing: Properties shows the frame.
    #[default]
    Frame,
    /// These cells of the title block, in the order they were chosen.
    Cells(Vec<usize>),
    /// The columns and rows.
    Rulers,
}

/// The reach of a line's grip, screen points.
const LINE_REACH_PT: f64 = 4.0;

/// The snap of a dragged line: 0.5 mm.
pub const LINE_SNAP_NM: i64 = 500_000;

fn inside(r: &SheetRect, p: (i64, i64)) -> bool {
    r[0] <= p.0 && p.0 <= r[2] && r[1] <= p.1 && p.1 <= r[3]
}

/// What `at` points at on the frame of `facts`: a line within `reach` nm first, then a cell, then the rulers.
pub fn frame_hit(facts: &FrameFacts, at: (i64, i64), reach: i128) -> Option<FrameHit> {
    let line = facts
        .lines
        .iter()
        .enumerate()
        .map(|(i, (a, b))| (super::segment_dist2(at, *a, *b), i))
        .filter(|(d, _)| *d <= reach * reach)
        .min()
        .map(|(_, i)| FrameHit::Line(i));
    line.or_else(|| {
        facts
            .cells
            .iter()
            .find(|c| inside(&c.rect, at))
            .map(|c| FrameHit::Cell(c.index))
    })
    .or_else(|| {
        let bands = facts.columns_band.iter().chain(&facts.rows_band);
        bands.clone().any(|r| inside(r, at)).then_some(FrameHit::Rulers)
    })
}

/// A dragged line: its index, whether it is vertical, and where the pointer is now.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct LineDrag {
    line: usize,
    vertical: bool,
    at: (i64, i64),
    free: bool,
}

/// The Edit frame tool.
#[derive(Clone, Debug, Default)]
pub struct FrameTool {
    hover: Option<FrameHit>,
    chosen: FrameChoice,
    drag: Option<LineDrag>,
}

/// The rectangle of a sheet rectangle.
fn rect(r: &SheetRect) -> ew_units::Rect {
    rect_of((r[0], r[1]), (r[2], r[3]))
}

impl FrameTool {
    /// The tool with nothing chosen.
    pub fn new() -> FrameTool {
        FrameTool::default()
    }

    /// What is chosen.
    pub fn chosen(&self) -> &FrameChoice {
        &self.chosen
    }

    /// The overlay: the drawing faded, the hovered cell, line or ruler dashed, the chosen ones solid, a dragged line
    /// with its new place.
    pub fn overlay(&self, facts: &FrameFacts) -> Overlay {
        let mut items = Vec::new();
        if let Some(area) = &facts.drawing_area {
            items.push(OverlayItem::Fade { r: rect(area) });
        }
        let outline = |hit: FrameHit, style: Style, items: &mut Vec<OverlayItem>| match hit {
            FrameHit::Cell(i) => {
                if let Some(c) = facts.cells.iter().find(|c| c.index == i) {
                    items.push(OverlayItem::Rect {
                        r: rect(&c.rect),
                        style,
                    });
                }
            }
            FrameHit::Line(i) => {
                if let Some((a, b)) = facts.lines.get(i) {
                    items.push(OverlayItem::Line { a: *a, b: *b, style });
                }
            }
            FrameHit::Rulers => {
                for band in facts.columns_band.iter().chain(&facts.rows_band) {
                    items.push(OverlayItem::Rect { r: rect(band), style });
                }
            }
        };
        if let Some(h) = self.hover {
            outline(h, Style::dashed(Token::Ghost), &mut items);
        }
        match &self.chosen {
            FrameChoice::Frame => {}
            FrameChoice::Cells(cells) => {
                for i in cells {
                    outline(FrameHit::Cell(*i), Style::solid(Token::Ghost), &mut items);
                }
            }
            FrameChoice::Rulers => outline(FrameHit::Rulers, Style::solid(Token::Ghost), &mut items),
        }
        if let Some(d) = self.drag
            && let Some((a, b)) = facts.lines.get(d.line)
        {
            let (a, b) = if d.vertical {
                ((d.at.0, a.1), (d.at.0, b.1))
            } else {
                ((a.0, d.at.1), (b.0, d.at.1))
            };
            items.push(OverlayItem::Line {
                a,
                b,
                style: Style::solid(Token::Ghost),
            });
            items.push(OverlayItem::Label {
                at: b,
                text: crate::canvas::mm3(if d.vertical { d.at.0 - a.0 } else { d.at.1 - a.1 }),
                anchor: Anchor::Beside,
            });
        }
        Overlay::new(items)
    }

    /// The line `line`'s new coordinate for the pointer at `at`: on the 0.5 mm grid of the sheet unless `free`.
    pub fn snapped(at: i64, free: bool) -> i64 {
        if free {
            at
        } else {
            (at + LINE_SNAP_NM / 2).div_euclid(LINE_SNAP_NM) * LINE_SNAP_NM
        }
    }
}

impl Tool for FrameTool {
    fn name(&self) -> &'static str {
        "frame"
    }

    fn state(&self) -> &'static str {
        if self.drag.is_some() {
            return "line-drag";
        }
        match &self.chosen {
            FrameChoice::Frame => "idle",
            FrameChoice::Cells(c) if c.len() > 1 => "cells",
            FrameChoice::Cells(_) => "cell",
            FrameChoice::Rulers => "rulers",
        }
    }

    fn wants_hover(&self) -> bool {
        true
    }

    fn status_hint(&self) -> Hint {
        Hint::plain(key!("hint-frame"))
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        let Some(facts) = cx.page.frame.clone() else {
            return ToolOutput::Refused(key!("reason-frame-none"));
        };
        let reach = cx.reach_nm(LINE_REACH_PT);
        match ev {
            CanvasEvent::PointerMove { at, mods } => {
                if let Some(d) = &mut self.drag {
                    let v = if d.vertical { at.0 } else { at.1 };
                    let v = FrameTool::snapped(v, mods.alt);
                    d.at = if d.vertical { (v, at.1) } else { (at.0, v) };
                    d.free = mods.alt;
                } else {
                    self.hover = frame_hit(&facts, *at, reach);
                }
                ToolOutput::Transient(self.overlay(&facts))
            }
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => {
                let hit = frame_hit(&facts, *at, reach);
                self.chosen = match (hit, &self.chosen) {
                    (Some(FrameHit::Line(line)), _) => {
                        let (a, b) = facts.lines[line];
                        self.drag = Some(LineDrag {
                            line,
                            vertical: a.0 == b.0,
                            at: *at,
                            free: mods.alt,
                        });
                        return ToolOutput::Transient(self.overlay(&facts));
                    }
                    (Some(FrameHit::Cell(i)), FrameChoice::Cells(cells)) if mods.shift => {
                        let mut cells = cells.clone();
                        match cells.iter().position(|c| *c == i) {
                            Some(p) => {
                                cells.remove(p);
                            }
                            None => cells.push(i),
                        }
                        if cells.is_empty() {
                            FrameChoice::Frame
                        } else {
                            FrameChoice::Cells(cells)
                        }
                    }
                    (Some(FrameHit::Cell(i)), _) => FrameChoice::Cells(vec![i]),
                    (Some(FrameHit::Rulers), _) => FrameChoice::Rulers,
                    (None, _) => FrameChoice::Frame,
                };
                ToolOutput::Host(HostStep::FrameChoose {
                    choice: self.chosen.clone(),
                })
            }
            CanvasEvent::Release { at, mods, .. } => {
                let Some(d) = self.drag.take() else {
                    return ToolOutput::None;
                };
                let v = if d.vertical { at.0 } else { at.1 };
                ToolOutput::Host(HostStep::FrameLine {
                    line: d.line,
                    to: FrameTool::snapped(v, mods.alt),
                })
            }
            CanvasEvent::Key(super::ToolKey::Finish) => match &self.chosen {
                FrameChoice::Cells(c) if c.len() == 1 => ToolOutput::Host(HostStep::FrameType),
                _ => ToolOutput::None,
            },
            CanvasEvent::Escape if self.drag.is_some() => {
                self.drag = None;
                ToolOutput::Transient(self.overlay(&facts))
            }
            CanvasEvent::Escape => ToolOutput::None,
            _ => ToolOutput::None,
        }
    }

    fn finishes_on_double_click(&self) -> bool {
        true
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

impl FrameTool {
    /// Chooses `choice` (from Properties or a script): the overlay follows at the next move.
    pub fn choose(&mut self, choice: FrameChoice) {
        self.chosen = choice;
    }
}
