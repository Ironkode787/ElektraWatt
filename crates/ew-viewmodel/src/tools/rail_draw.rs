// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw rail (WP-S4.02 part 4, D11; doc 12 §3.2, §12.1): two clicks on a panel page's surface draw a mounting rail
//! with `layout.carrier.add`. The draw tools' snapping lives here too; the duct tool ([`super::duct_draw`]) shares it.
//!
//! **Snapping** (D11, ruling 5: 15 points on screen, D4's radius). Each coordinate of the pointer snaps on its own, in
//! surface nm: to the nearest snap line within reach (the usable area's edges; for a duct also those edges inset by
//! half its width; the outlines of the ducts; the ends and axes of the rails), else to the plate grid (5 mm, or 1 mm by
//! the tool option). Of the targets in reach the nearest wins, the grid included; at equal distance a duct edge or a
//! usable edge before a rail's end or axis, before the grid (as ID-1044 orders the placing tool's targets). Shift
//! suspends the lines (the grid stays).
//!
//! **The second point** is constrained to the dominant axis of the pointer's displacement from the first (a rail is
//! axis-parallel, D5). The part is the tool option's rail part ([`DrawPart`]), its profile from its rail section; a
//! part without one is refused ("The rail part has no profile; choose a rail part with a rail section").
//!
//! **Ghost** (ruling 4): [`super::Ghost::Line`] from the first point to the pointer, drawn as a solid line in the
//! `ghost` token with one chip giving its length (doc 16a §7.3, §7.4).

use ew_api::AnyCommand;
use ew_api::output::{PanelTables, Uuid};
use serde_json::{Value, json};

use super::layout::{
    LAYOUT_SNAP_PT, clamp64, round_to, sheet_of_surface, surface_of_sheet, surface_per_sheet,
};
use super::{
    Anchor, Button, CanvasEvent, Ghost, Mods, Overlay, OverlayItem, Style, Token, Tool, ToolCx, ToolOutput,
};
use crate::key;
use crate::strings;

/// What the tool options card shows while Draw rail or Draw duct runs (D11).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DrawCard {
    /// `rail` or `duct`.
    pub kind: &'static str,
    /// The part numbers on offer: the project's copies, then the installed libraries' parts.
    pub parts: Vec<String>,
    /// The part number it draws with; `None` without one.
    pub part: Option<String>,
    /// A duct's width and height in millimetres when the part gives none (the fields); `None` otherwise.
    pub size: Option<(String, String)>,
}

/// The part a draw tool draws with (the tool option): a project copy, or a system library's part with the `copies`
/// that embed it (`layout.carrier.add`'s `copies`, D7).
#[derive(Clone, PartialEq, Debug)]
pub struct DrawPart {
    /// The part copy's ID.
    pub part: Uuid,
    /// Its part number (the options card's choice).
    pub number: String,
    /// A rail part's profile (its rail section); `None` without one.
    pub profile: Option<Value>,
    /// A duct part's `W × H` (`mech`), nm.
    pub size: Option<(i128, i128)>,
    /// The library documents to embed first; empty for a project copy.
    pub copies: Vec<Value>,
}

/// One snap line: the coordinate's value, and its class (0 an edge of a duct or the usable area, 1 a rail's end or
/// axis); the grid is class 2.
type Line = (i128, u8);

/// The snap lines of a panel page, per coordinate `(u lines, v lines)` (see the module documentation); `inset` is half
/// a duct's width, which insets the usable edges for a duct's centreline.
pub(crate) fn snap_lines(panel: &PanelTables, inset: Option<i128>) -> (Vec<Line>, Vec<Line>) {
    let mut us: Vec<Line> = Vec::new();
    let mut vs: Vec<Line> = Vec::new();
    if let (Some(u0), Some(u1), Some(v0), Some(v1)) = (
        panel.usable.iter().map(|p| i128::from(p.0)).min(),
        panel.usable.iter().map(|p| i128::from(p.0)).max(),
        panel.usable.iter().map(|p| i128::from(p.1)).min(),
        panel.usable.iter().map(|p| i128::from(p.1)).max(),
    ) {
        us.extend([(u0, 0), (u1, 0)]);
        vs.extend([(v0, 0), (v1, 0)]);
        if let Some(h) = inset {
            us.extend([(u0 + h, 0), (u1 - h, 0)]);
            vs.extend([(v0 + h, 0), (v1 - h, 0)]);
        }
    }
    for c in &panel.carriers {
        if c.kind == "duct" {
            for r in &c.outline {
                us.extend([(i128::from(r.min().x.0), 0), (i128::from(r.max().x.0), 0)]);
                vs.extend([(i128::from(r.min().y.0), 0), (i128::from(r.max().y.0), 0)]);
            }
        } else {
            for p in &c.path {
                us.push((i128::from(p.0), 1));
                vs.push((i128::from(p.1), 1));
            }
        }
    }
    (us, vs)
}

/// One coordinate `x` snapped: the nearest line within `reach`, or the nearest grid point; at equal distance the lower
/// class wins (lines before the grid).
fn snap_one(x: i128, lines: &[Line], reach: i128, grid: i128) -> i128 {
    let mut best = ((round_to(x, grid) - x).abs(), 2u8, round_to(x, grid));
    for (v, class) in lines {
        let d = (v - x).abs();
        if d <= reach && (d, *class) < (best.0, best.1) {
            best = (d, *class, *v);
        }
    }
    best.2
}

/// A surface point snapped by [`snap_one`] in each coordinate; with `free` (Shift) only to the grid.
pub(crate) fn snap_point(
    p: (i128, i128),
    lines: &(Vec<Line>, Vec<Line>),
    reach: i128,
    grid: i128,
    free: bool,
) -> (i128, i128) {
    let none: Vec<Line> = Vec::new();
    let (us, vs) = if free {
        (&none, &none)
    } else {
        (&lines.0, &lines.1)
    };
    (snap_one(p.0, us, reach, grid), snap_one(p.1, vs, reach, grid))
}

/// `p` constrained to the dominant axis of its displacement from `from` (ties: horizontal).
pub(crate) fn dominant(from: (i128, i128), p: (i128, i128)) -> (i128, i128) {
    if (p.0 - from.0).abs() >= (p.1 - from.1).abs() {
        (p.0, from.1)
    } else {
        (from.0, p.1)
    }
}

/// The snap reach in surface nm at the sheet point `at` (15 points at the zoom, D4).
pub(crate) fn reach_at(cx: &ToolCx<'_>, panel: &PanelTables, at: (i64, i64)) -> i128 {
    let r = cx.nm_of_points(LAYOUT_SNAP_PT) * surface_per_sheet(cx, panel, at);
    if r.is_finite() {
        r.clamp(0.0, 9.0e15) as i128
    } else {
        0
    }
}

/// Millimetres as the project prints them.
pub(crate) fn mm_of(panel: &PanelTables, nm: i128) -> String {
    let sign = if nm < 0 { "-" } else { "" };
    let a = nm.unsigned_abs();
    let (int, frac) = (a / 1_000_000, a % 1_000_000);
    let text = if frac == 0 {
        format!("{sign}{int}")
    } else {
        let digits = format!("{frac:06}");
        format!("{sign}{int}.{}", digits.trim_end_matches('0'))
    };
    if panel.decimal_comma {
        text.replace('.', ",")
    } else {
        text
    }
}

/// A point of the surface as a JSON pair, each coordinate saturated to the `i64` range (C44-p2#1), so that an
/// `i128` beyond `i64` still makes a JSON number; the tools refuse such a point first ([`drawable`]).
pub(crate) fn point_json(p: (i128, i128)) -> Value {
    json!([clamp64(p.0), clamp64(p.1)])
}

/// The largest coordinate magnitude a carrier path takes, nm: `layout.carrier.add`'s range (2^53 - 1).
const DRAWABLE: i128 = (1 << 53) - 1;

/// Whether every point lies inside the drawable range. The draw tools refuse a point outside it in words, with
/// `reason-point-outside-drawing`, before they send the command (ID-1106); the command's own range text stays for
/// scripts.
pub(crate) fn drawable(points: &[(i128, i128)]) -> bool {
    points
        .iter()
        .all(|p| p.0.abs() <= DRAWABLE && p.1.abs() <= DRAWABLE)
}

/// The midpoint of two sheet points, computed in `i128` so that two far points cannot overflow (C44-p2#1).
pub(crate) fn midpoint(a: (i64, i64), b: (i64, i64)) -> (i64, i64) {
    let mid = |x: i64, y: i64| clamp64((i128::from(x) + i128::from(y)) / 2);
    (mid(a.0, b.0), mid(a.1, b.1))
}

/// The Draw rail tool (D11).
#[derive(Clone, Debug)]
pub struct RailDrawTool {
    part: Option<DrawPart>,
    /// The first point, surface nm.
    start: Option<(i128, i128)>,
    /// The snapped pointer, surface nm.
    cursor: Option<(i128, i128)>,
    /// The last sheet point and modifiers.
    last: Option<((i64, i64), Mods)>,
}

impl RailDrawTool {
    /// The tool drawing with `part` (`None`: the project and the libraries have no rail part; a click is refused).
    pub fn new(part: Option<DrawPart>) -> RailDrawTool {
        RailDrawTool {
            part,
            start: None,
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

    /// The point the pointer at `at` stands for: snapped, and constrained after the first point.
    fn point(&self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>, panel: &PanelTables) -> (i128, i128) {
        let raw = surface_of_sheet(cx, panel, at);
        let p = snap_point(
            raw,
            &snap_lines(panel, None),
            reach_at(cx, panel, at),
            cx.layout.grid(),
            mods.shift,
        );
        match self.start {
            Some(s) => dominant(s, p),
            None => p,
        }
    }

    fn overlay(&self, cx: &ToolCx<'_>, panel: &PanelTables) -> Overlay {
        let Some(c) = self.cursor else {
            return Overlay::default();
        };
        let b = sheet_of_surface(cx, panel, c);
        let mut items = vec![OverlayItem::Marker {
            at: b,
            kind: super::MarkerKind::Grid,
        }];
        if let Some(s) = self.start {
            let a = sheet_of_surface(cx, panel, s);
            items.push(OverlayItem::Line {
                a,
                b,
                style: Style::solid(Token::Ghost),
            });
            let length = (c.0 - s.0).abs() + (c.1 - s.1).abs();
            items.push(OverlayItem::Label {
                at: midpoint(a, b),
                text: strings::format(key!("draw-length"), &[&mm_of(panel, length)]),
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

    fn click(&mut self, at: (i64, i64), mods: Mods, cx: &ToolCx<'_>) -> ToolOutput {
        let Some(panel) = cx.page.panel.as_deref() else {
            return ToolOutput::Refused(key!("reason-no-panel-page"));
        };
        let Some(part) = self.part.clone() else {
            return ToolOutput::Refused(key!("reason-no-rail-part"));
        };
        let Some(profile) = part.profile.clone() else {
            return ToolOutput::Refused(key!("reason-rail-no-profile"));
        };
        self.last = Some((at, mods));
        let p = self.point(at, mods, cx, panel);
        self.cursor = Some(p);
        let Some(start) = self.start else {
            self.start = Some(p);
            return ToolOutput::Transient(self.overlay(cx, panel));
        };
        if p == start {
            return ToolOutput::Transient(self.overlay(cx, panel));
        }
        if !drawable(&[start, p]) {
            return ToolOutput::Refused(key!("reason-point-outside-drawing"));
        }
        let mut params = json!({
            "surface": panel.surface.to_string(),
            "kind": "rail",
            "part": part.part.to_string(),
            "path": [point_json(start), point_json(p)],
            "params": {"rail": {"profile": profile}},
        });
        if !part.copies.is_empty() {
            params["copies"] = Value::Array(part.copies);
        }
        ToolOutput::Commit(Box::new(AnyCommand {
            name: "layout.carrier.add".into(),
            params,
        }))
    }
}

impl Tool for RailDrawTool {
    fn name(&self) -> &'static str {
        "rail_draw"
    }

    fn state(&self) -> &'static str {
        if self.start.is_some() { "drawing" } else { "armed" }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, mods } => self.track(*at, *mods, cx),
            CanvasEvent::CursorMove { at } => self.track(*at, Mods::default(), cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                mods,
            } => self.click(*at, *mods, cx),
            CanvasEvent::Activate => match self.last {
                Some((at, mods)) => self.click(at, mods, cx),
                None => ToolOutput::None,
            },
            CanvasEvent::Back | CanvasEvent::Escape if self.start.is_some() => {
                self.start = None;
                match cx.page.panel.as_deref() {
                    Some(panel) if matches!(ev, CanvasEvent::Back) => {
                        ToolOutput::Transient(self.overlay(cx, panel))
                    }
                    _ => ToolOutput::Cancel,
                }
            }
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> super::Hint {
        if self.start.is_some() {
            super::Hint::new(key!("hint-rail-draw-end"), &[super::BACK, super::ESCAPE])
        } else {
            super::Hint::new(key!("hint-rail-draw"), &[super::ESCAPE])
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        let c = self.cursor?;
        Some(Ghost::Line {
            kind: "rail",
            from: self.start.unwrap_or(c),
            to: self.start.map(|_| c),
        })
    }

    fn on_commit(&mut self, done: bool) {
        if done {
            self.start = None;
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{dominant, snap_one};

    const MM: i128 = 1_000_000;

    /// D11's snapping on one coordinate: a line in reach wins over a farther grid point, the grid over a line out of
    /// reach, and at equal distance the line.
    #[test]
    fn a_line_in_reach_beats_the_grid_and_ties_go_to_the_line() {
        let lines = [(879 * MM, 0u8)];
        assert_eq!(snap_one(879 * MM + 400_000, &lines, 2 * MM, 5 * MM), 879 * MM);
        assert_eq!(
            snap_one(877 * MM, &lines, MM, 5 * MM),
            875 * MM,
            "out of reach: the grid"
        );
        let tie = [(880 * MM, 0u8)];
        assert_eq!(snap_one(881 * MM, &tie, 2 * MM, 5 * MM), 880 * MM);
        assert_eq!(snap_one(882 * MM, &[], 2 * MM, MM), 882 * MM, "the 1 mm grid");
    }

    #[test]
    fn the_second_point_keeps_the_dominant_axis() {
        assert_eq!(dominant((40, 740), (610, 760)), (610, 740));
        assert_eq!(dominant((40, 740), (50, 100)), (40, 100));
    }
}
