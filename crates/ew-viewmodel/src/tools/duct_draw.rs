// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw duct (WP-S4.02 part 4, D11; doc 12 §3.2, §12.1): clicks add the vertices of an orthogonal centreline, each
//! segment on the dominant axis of its displacement; Backspace removes the last vertex; a double click
//! (`tool.finish`), or Enter on the last vertex, finishes with `layout.carrier.add`.
//!
//! **Snapping** is the rail tool's ([`super::rail_draw`]) with the usable edges also inset by half the duct's width
//! (so a duct lies flush with the plate edge) and the outlines of the other ducts (T joints): Golden A's D1 runs at
//! v = 880 mm under the plate's top edge, and D2 starts at (20, 860) mm under D1 (D11).
//!
//! **Size.** The part's `mech` `W × H`, else the tool option's fields (40 × 60 mm by default, then the last used).
//!
//! **Enter.** A click at the grid cursor adds its point as the next vertex; Enter where the last vertex already is
//! finishes a centreline of two or more vertices, so the keyboard draws as the pointer does.

use ew_api::AnyCommand;
use ew_api::output::PanelTables;
use serde_json::{Value, json};

use super::layout::{sheet_of_surface, surface_of_sheet};
use super::rail_draw::{
    DrawPart, dominant, drawable, midpoint, mm_of, point_json, reach_at, snap_lines, snap_point,
};
use super::{
    Anchor, Button, CanvasEvent, Ghost, Mods, Overlay, OverlayItem, Style, Token, Tool, ToolCx, ToolKey,
    ToolOutput,
};
use crate::key;
use crate::strings;

/// The duct size the tool options start with: 40 × 60 mm (D11).
pub const DEFAULT_DUCT_SIZE: (i128, i128) = (40_000_000, 60_000_000);

/// The most vertices a centreline takes (hostile input stays bounded).
const MAX_VERTICES: usize = 256;

/// The Draw duct tool (D11).
#[derive(Clone, Debug)]
pub struct DuctDrawTool {
    part: Option<DrawPart>,
    /// The tool option's width and height, nm (used when the part gives none).
    option_size: (i128, i128),
    /// The fixed vertices, surface nm.
    points: Vec<(i128, i128)>,
    /// The snapped pointer, surface nm.
    cursor: Option<(i128, i128)>,
    /// The last sheet point and modifiers.
    last: Option<((i64, i64), Mods)>,
}

impl DuctDrawTool {
    /// The tool drawing with `part` (none: a duct without a part) at the option size `size`.
    pub fn new(part: Option<DrawPart>, size: (i128, i128)) -> DuctDrawTool {
        DuctDrawTool {
            part,
            option_size: size,
            points: Vec::new(),
            cursor: None,
            last: None,
        }
    }

    /// The part it draws with.
    pub fn part(&self) -> Option<&DrawPart> {
        self.part.as_ref()
    }

    /// Changes the part (the tool options card).
    pub fn set_part(&mut self, part: Option<DrawPart>) {
        self.part = part;
    }

    /// Changes the option size (the tool options card's fields).
    pub fn set_option_size(&mut self, size: (i128, i128)) {
        self.option_size = size;
    }

    /// The width and height it draws: the part's, else the option's.
    pub fn size(&self) -> (i128, i128) {
        self.part
            .as_ref()
            .and_then(|p| p.size)
            .unwrap_or(self.option_size)
    }

    /// The fixed vertices.
    pub fn vertices(&self) -> &[(i128, i128)] {
        &self.points
    }

    fn point(&self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>, panel: &PanelTables) -> (i128, i128) {
        let raw = surface_of_sheet(cx, panel, at);
        let p = snap_point(
            raw,
            &snap_lines(panel, Some(self.size().0 / 2)),
            reach_at(cx, panel, at),
            cx.layout.grid(),
            mods.shift,
        );
        match self.points.last() {
            Some(s) => dominant(*s, p),
            None => p,
        }
    }

    fn overlay(&self, cx: &ToolCx<'_>, panel: &PanelTables) -> Overlay {
        let mut pts: Vec<(i128, i128)> = self.points.clone();
        if let Some(c) = self.cursor
            && pts.last() != Some(&c)
        {
            pts.push(c);
        }
        let Some(&end) = pts.last() else {
            return Overlay::default();
        };
        let sheet: Vec<(i64, i64)> = pts.iter().map(|p| sheet_of_surface(cx, panel, *p)).collect();
        let mut items = vec![OverlayItem::Marker {
            at: sheet_of_surface(cx, panel, end),
            kind: super::MarkerKind::Grid,
        }];
        if sheet.len() > 1 {
            items.push(OverlayItem::Polyline {
                points: sheet.clone(),
                style: Style::solid(Token::Ghost),
            });
            let (w, h) = self.size();
            let a = sheet[sheet.len() - 2];
            let b = sheet[sheet.len() - 1];
            items.push(OverlayItem::Label {
                at: midpoint(a, b),
                text: strings::format(key!("draw-duct-size"), &[&mm_of(panel, w), &mm_of(panel, h)]),
                anchor: Anchor::Above,
            });
        }
        Overlay::new(items)
    }

    fn track(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        self.last = Some((at, mods));
        let Some(panel) = cx.page.panel.as_deref() else {
            return ToolOutput::None;
        };
        self.cursor = Some(self.point(at, mods, cx, panel));
        ToolOutput::Transient(self.overlay(cx, panel))
    }

    /// A click (or Enter, `finish_on_repeat`): the next vertex; Enter on the last vertex finishes.
    fn click(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>, finish_on_repeat: bool) -> ToolOutput {
        let Some(panel) = cx.page.panel.as_deref() else {
            return ToolOutput::Refused(key!("reason-no-panel-page"));
        };
        self.last = Some((at, mods));
        let p = self.point(at, mods, cx, panel);
        self.cursor = Some(p);
        if self.points.last() == Some(&p) {
            return if finish_on_repeat {
                self.finish(panel)
            } else {
                ToolOutput::Transient(self.overlay(cx, panel))
            };
        }
        if self.points.len() >= MAX_VERTICES {
            return ToolOutput::Refused(key!("reason-duct-vertices"));
        }
        self.points.push(p);
        ToolOutput::Transient(self.overlay(cx, panel))
    }

    /// Finishes the centreline: `layout.carrier.add` of a duct; refused with fewer than two vertices.
    fn finish(&mut self, panel: &PanelTables) -> ToolOutput {
        if self.points.len() < 2 {
            return ToolOutput::Refused(key!("reason-duct-two-points"));
        }
        if !drawable(&self.points) {
            return ToolOutput::Refused(key!("reason-point-outside-drawing"));
        }
        let (w, h) = self.size();
        let mut params = json!({
            "surface": panel.surface.to_string(),
            "kind": "duct",
            "path": self.points.iter().map(|p| point_json(*p)).collect::<Vec<_>>(),
            "params": {"duct": {"width": w, "height": h}},
        });
        if let Some(part) = &self.part {
            params["part"] = Value::String(part.part.to_string());
            if !part.copies.is_empty() {
                params["copies"] = Value::Array(part.copies.clone());
            }
        }
        ToolOutput::Commit(Box::new(AnyCommand {
            name: "layout.carrier.add".into(),
            params,
        }))
    }
}

impl Tool for DuctDrawTool {
    fn name(&self) -> &'static str {
        "duct_draw"
    }

    fn state(&self) -> &'static str {
        if self.points.is_empty() {
            "armed"
        } else {
            "drawing"
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, mods } => self.track(*at, *mods, cx),
            CanvasEvent::CursorMove { at } => self.track(*at, Mods::default(), cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => self.click(*at, *mods, cx, false),
            CanvasEvent::Activate => match self.last {
                Some((at, mods)) => self.click(at, mods, cx, true),
                None => ToolOutput::None,
            },
            CanvasEvent::Key(ToolKey::Finish) => match cx.page.panel.as_deref() {
                Some(panel) => self.finish(panel),
                None => ToolOutput::None,
            },
            CanvasEvent::Back if !self.points.is_empty() => {
                self.points.pop();
                match cx.page.panel.as_deref() {
                    Some(panel) => {
                        // The pointer's point follows the new last vertex's axis.
                        if let Some((at, mods)) = self.last {
                            self.cursor = Some(self.point(at, mods, cx, panel));
                        }
                        ToolOutput::Transient(self.overlay(cx, panel))
                    }
                    None => ToolOutput::None,
                }
            }
            CanvasEvent::Escape if !self.points.is_empty() => {
                self.points.clear();
                ToolOutput::Cancel
            }
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> super::Hint {
        if self.points.is_empty() {
            super::Hint::new(key!("hint-duct-draw"), &[super::ESCAPE])
        } else {
            super::Hint::new(key!("hint-duct-draw-next"), &[super::ENTER, super::BACK])
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn finishes_on_double_click(&self) -> bool {
        true
    }

    fn ghost(&self) -> Option<Ghost> {
        let c = self.cursor?;
        Some(Ghost::Line {
            kind: "duct",
            from: self.points.last().copied().unwrap_or(c),
            to: (!self.points.is_empty()).then_some(c),
        })
    }

    fn on_commit(&mut self, done: bool) {
        if done {
            self.points.clear();
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
