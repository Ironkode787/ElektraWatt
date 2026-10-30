// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The black box tool (`black_box`; WP-S2.04 §9.4, FR-DEV-013, FR-DEV-039) and the grips of a selected black box.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | `PointerMove`, `CursorMove` | armed | `Transient`: a grid marker at the snapped point |
//! | armed | `Press` or `Activate` | outline | the first corner is fixed; `Transient` |
//! | outline | `PointerMove`, `CursorMove` | outline | `Transient`: the rectangle to the snapped point (grown to the right until the default CPs fit), a marker per CP |
//! | outline | `Dwell { ms ≥ 80 }` | outline | `Preview(box.black …)` |
//! | outline | `Press` or `Activate` | armed | `Commit(box.black …)` (a rectangle without width or height commits nothing) |
//! | outline | `Escape` | armed | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! **The command** is `box.black {page, symbol: the black-box symbol, pos: the top-left corner, box: {outline: the
//! rectangle relative to pos (v0 top-left, then clockwise), cps}, target}`. **Default CPs** for a part target: the
//! template entry's CPs in template order, the first `⌈n/2⌉` on the top edge and the rest on the bottom edge, left to
//! right, leaving up and down; when an edge is too short the rectangle grows to the right until they fit one grid step
//! apart with one grid step to spare at each corner. On a wider box the CPs of an edge spread over its drawn length
//! (FT-550): one common pitch, the largest whole number of grid steps that fits `m + 1` times into the edge, the group
//! centred (half a step to the left when it cannot be centred exactly). Without a part, no CPs.
//!
//! **Grips** ([`grip_points`], [`reshape_by_grip`]): for an outline of `n` vertices in stored order, grips `0 … n−1`
//! are the vertices and grips `n … 2n−1` the midpoints of the edges `v0v1, …, v(n−1)v0`. On a rectangle a vertex grip
//! moves that corner and keeps the rectangle, an edge grip moves that edge perpendicular to itself; on any other
//! outline a vertex grip moves that vertex alone and an edge grip translates an axis-aligned edge (another edge is
//! refused, `reason-grip-not-axis`). Select drags them (and a box CP along the outline) and commits `box.reshape`.

use ew_api::AnyCommand;
use ew_api::geometry::{Dir, orient_dir, orient_point};
use ew_api::output::TableRep;
use ew_api::output::ew_dl::EntityRef;
use serde_json::{Value, json};

use super::wire::PageGridMap;
use super::{
    Anchor, Button, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, Style, Token, Tool, ToolCx,
    ToolOutput, rect_of,
};

/// What a black box creates.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum BoxTarget {
    /// A new device with this letter.
    NewDevice {
        /// The letter code.
        letter: String,
    },
    /// A device made from a part, the box drawing a template entry that is a black box.
    Part {
        /// The part copy's ID text.
        part: String,
        /// The template entry.
        template: u16,
        /// The new device's letter.
        letter: String,
        /// The part number (the label's).
        number: String,
        /// The entry's CPs in template order: (function CP index, designation).
        cps: Vec<(u16, String)>,
    },
}

/// A box CP on the page: its function CP index, its point and its direction.
pub type BoxCpAt = (u16, (i64, i64), Dir);

/// The laid-out box: the insertion point (page nm), the outline relative to it and the CPs `(idx, relative point,
/// direction)`.
pub type BoxLayout = ((i64, i64), Vec<(i64, i64)>, Vec<(u16, (i64, i64), Dir)>);

/// The box with corners `a` and `b` (page nm, grid points) and the CPs `cps` laid out on its top and bottom edges;
/// `None` for a rectangle without height, or without width when no CP widens it.
pub fn layout(grid: PageGridMap, a: (i64, i64), b: (i64, i64), cps: &[(u16, String)]) -> Option<BoxLayout> {
    let (ax, bx) = (grid.index(a.0), grid.index(b.0));
    let (ay, by) = (grid.index(a.1), grid.index(b.1));
    let (kx0, mut kx1) = (ax.min(bx), ax.max(bx));
    let (ky0, ky1) = (ay.min(by), ay.max(by));
    if ky0 == ky1 {
        return None;
    }
    let n = cps.len();
    let top = n.div_ceil(2);
    let bottom = n - top;
    if n > 0 {
        let need = i64::try_from(top.max(bottom)).unwrap_or(i64::MAX / 2) + 1;
        if kx1 - kx0 < need {
            kx1 = kx0 + need;
        }
    }
    if kx0 == kx1 {
        return None;
    }
    let pos = (grid.coord(kx0), grid.coord(ky0));
    let (w, h) = (grid.coord(kx1) - pos.0, grid.coord(ky1) - pos.1);
    let outline = vec![(0, 0), (w, 0), (w, h), (0, h)];
    let width = kx1 - kx0;
    let mut out = Vec::with_capacity(n);
    for (i, (idx, _)) in cps.iter().enumerate() {
        let (k, m, y, dir) = if i < top {
            (i, top, 0, Dir::Up)
        } else {
            (i - top, bottom, h, Dir::Down)
        };
        let k = i64::try_from(k).unwrap_or(0);
        let m = i64::try_from(m).unwrap_or(1);
        out.push((*idx, (grid.coord(kx0 + spread(width, m, k)) - pos.0, y), dir));
    }
    Some((pos, outline, out))
}

/// The grid offset of CP `k` of `m` on an edge `width` grid steps long (FT-550): the CPs one common pitch apart, the
/// largest whole number of grid steps that fits `m + 1` times into the edge (at least one), and the group centred on
/// the edge (half a step to the left when it cannot be centred exactly). Every CP lies at least one grid step from both
/// corners whenever `width ≥ m + 1`, which the layout guarantees.
fn spread(width: i64, m: i64, k: i64) -> i64 {
    let pitch = (width / (m + 1)).max(1);
    let start = (width - (m - 1) * pitch) / 2;
    start + k * pitch
}

/// The JSON of a box CP (`{idx, pos, dir}`).
fn cp_json(idx: u16, pos: (i64, i64), dir: Dir) -> Value {
    json!({"idx": idx, "pos": [pos.0, pos.1], "dir": dir.word()})
}

/// The tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    Armed { at: Option<(i64, i64)> },
    Outline { first: (i64, i64), at: (i64, i64) },
}

/// The black box tool.
#[derive(Clone, Debug)]
pub struct BlackBoxTool {
    /// The black-box symbol copy's ID text.
    symbol: String,
    target: BoxTarget,
    state: State,
    /// The library documents the black box embeds (`box.black`'s `copies`, FT-656); the host sends only those the
    /// project still lacks.
    copies: Vec<Value>,
}

impl BlackBoxTool {
    /// The tool, armed, drawing `target` with the black-box symbol copy `symbol` (its ID text).
    pub fn new(symbol: String, target: BoxTarget) -> BlackBoxTool {
        BlackBoxTool {
            symbol,
            target,
            state: State::Armed { at: None },
            copies: Vec::new(),
        }
    }

    /// The tool with the library documents its black boxes embed (FT-656).
    pub fn with_copies(mut self, copies: Vec<Value>) -> BlackBoxTool {
        self.copies = copies;
        self
    }

    /// The armed target.
    pub fn target(&self) -> &BoxTarget {
        &self.target
    }

    fn cps(&self) -> &[(u16, String)] {
        match &self.target {
            BoxTarget::Part { cps, .. } => cps,
            BoxTarget::NewDevice { .. } => &[],
        }
    }

    /// The target's label text (`new F`, `part MICO 4.10`).
    fn label(&self) -> String {
        match &self.target {
            BoxTarget::NewDevice { letter } => crate::strings::format("place-target-new", &[letter]),
            BoxTarget::Part { number, .. } => crate::strings::format("place-target-part", &[number]),
        }
    }

    /// The layout of the current rectangle.
    fn current(&self, cx: &ToolCx<'_>) -> Option<BoxLayout> {
        let State::Outline { first, at } = self.state else {
            return None;
        };
        layout(PageGridMap::new(cx.page.tables.grid), first, at, self.cps())
    }

    /// The `box.black` of the current rectangle.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        let (pos, outline, cps) = self.current(cx)?;
        let target = match &self.target {
            BoxTarget::NewDevice { letter } => json!({"new_device": {"letter": letter}}),
            BoxTarget::Part {
                part,
                template,
                letter,
                ..
            } => json!({"from_part": {"part": part, "template": template, "letter": letter}}),
        };
        let mut b = json!({
            "outline": outline.iter().map(|p| vec![p.0, p.1]).collect::<Vec<_>>(),
        });
        if !cps.is_empty() {
            b["cps"] = Value::Array(cps.iter().map(|(i, p, d)| cp_json(*i, *p, *d)).collect());
        }
        let mut params = json!({
            "page": cx.page.tables.page.to_string(),
            "symbol": self.symbol,
            "pos": [pos.0, pos.1],
            "box": b,
            "target": target,
        });
        if !self.copies.is_empty() {
            params["copies"] = Value::Array(self.copies.clone());
        }
        Some(Box::new(AnyCommand {
            name: "box.black".into(),
            params,
        }))
    }

    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        match self.state {
            State::Armed { at: None } => Overlay::default(),
            State::Armed { at: Some(at) } => Overlay::new(vec![OverlayItem::Marker {
                at: cx.snapper.page_to_sheet(at),
                kind: MarkerKind::Grid,
            }]),
            State::Outline { at, .. } => {
                let mut items = Vec::new();
                if let Some((pos, outline, cps)) = self.current(cx) {
                    let mut pts: Vec<(i64, i64)> = outline
                        .iter()
                        .map(|p| cx.snapper.page_to_sheet((pos.0 + p.0, pos.1 + p.1)))
                        .collect();
                    if let Some(&first) = pts.first() {
                        pts.push(first);
                    }
                    items.push(OverlayItem::Polyline {
                        points: pts,
                        style: Style::dashed(Token::Ghost),
                    });
                    for (_, p, _) in &cps {
                        items.push(OverlayItem::Marker {
                            at: cx.snapper.page_to_sheet((pos.0 + p.0, pos.1 + p.1)),
                            kind: MarkerKind::Cp,
                        });
                    }
                }
                items.push(OverlayItem::Label {
                    at: cx.snapper.page_to_sheet(at),
                    text: self.label(),
                    anchor: Anchor::Right,
                });
                Overlay::new(items)
            }
        }
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.snap_grid(at).page_nm;
        self.state = match self.state {
            State::Armed { .. } => State::Armed { at: Some(p) },
            State::Outline { first, .. } => State::Outline { first, at: p },
        };
        ToolOutput::Transient(self.overlay(cx))
    }

    fn click(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        match self.state {
            State::Armed { at: None } => ToolOutput::None,
            State::Armed { at: Some(at) } => {
                self.state = State::Outline { first: at, at };
                ToolOutput::Transient(self.overlay(cx))
            }
            State::Outline { at, .. } => match self.command(cx) {
                Some(cmd) => {
                    self.state = State::Armed { at: Some(at) };
                    ToolOutput::Commit(cmd)
                }
                None => ToolOutput::None,
            },
        }
    }
}

impl Tool for BlackBoxTool {
    fn name(&self) -> &'static str {
        "black_box"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "armed",
            State::Outline { .. } => "outline",
        }
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, .. } | CanvasEvent::CursorMove { at } => self.moved(*at, cx),
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => {
                let _ = self.moved(*at, cx);
                self.click(cx)
            }
            CanvasEvent::Activate => self.click(cx),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Escape => match self.state {
                State::Outline { at, .. } => {
                    self.state = State::Armed { at: Some(at) };
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "hint-box-armed",
            State::Outline { .. } => "hint-box-outline",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        let outline = match self.state {
            State::Outline { first, at } => {
                // The rectangle's four vertices, or none while it has no area (no height, or no width and no CP to
                // widen it; the corners are grid points, so equal coordinates are equal grid indices).
                if first.1 == at.1 || (first.0 == at.0 && self.cps().is_empty()) {
                    0
                } else {
                    4
                }
            }
            State::Armed { .. } => 0,
        };
        Some(Ghost::Box {
            outline,
            cps: self.cps().len(),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

// -------------------------------------------------------------------------------------------------------------------
// Grips

/// The inverse of orientation `o` (`0..=7`): the quarter turns swap, the mirrored ones are their own inverse.
pub fn orient_inverse(o: u8) -> u8 {
    match o % 8 {
        1 => 3,
        3 => 1,
        other => other,
    }
}

/// The grip points of an outline (page nm): the vertices, then the midpoints of the edges `v0v1, …, v(n−1)v0`.
pub fn grip_points(outline: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let n = outline.len();
    let mut out = outline.to_vec();
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        out.push((a.0 + (b.0 - a.0) / 2, a.1 + (b.1 - a.1) / 2));
    }
    out
}

/// Whether an outline is an axis-aligned rectangle as the black box tool stores it (four vertices, `v0v1` and
/// `v2v3` horizontal, `v1v2` and `v3v0` vertical).
pub fn is_rectangle(o: &[(i64, i64)]) -> bool {
    o.len() == 4 && o[0].1 == o[1].1 && o[1].0 == o[2].0 && o[2].1 == o[3].1 && o[3].0 == o[0].0
}

/// Twice the signed area of a polygon (positive: clockwise on the sheet, whose y axis points down).
pub fn signed_area2(poly: &[(i64, i64)]) -> i128 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            i128::from(a.0) * i128::from(b.1) - i128::from(b.0) * i128::from(a.1)
        })
        .sum()
}

/// The outline after grip `grip` moved to `to` (page nm, absolute). Errors: `reason-grip-not-axis` for an edge grip
/// of an edge that is neither horizontal nor vertical; `reason-box-too-small` for an outline without area;
/// `reason-no-grip` for a grip the outline does not have.
pub fn reshape_by_grip(
    outline: &[(i64, i64)],
    grip: usize,
    to: (i64, i64),
) -> Result<Vec<(i64, i64)>, &'static str> {
    let n = outline.len();
    if n < 3 || grip >= 2 * n {
        return Err("reason-no-grip");
    }
    let mut out = outline.to_vec();
    if grip < n {
        let i = grip;
        if is_rectangle(outline) {
            for j in [(i + n - 1) % n, (i + 1) % n] {
                if outline[j].0 == outline[i].0 {
                    out[j].0 = to.0;
                }
                if outline[j].1 == outline[i].1 {
                    out[j].1 = to.1;
                }
            }
        }
        out[i] = to;
    } else {
        let i = grip - n;
        let j = (i + 1) % n;
        let (a, b) = (outline[i], outline[j]);
        if a.1 == b.1 && a.0 != b.0 {
            out[i].1 = to.1;
            out[j].1 = to.1;
        } else if a.0 == b.0 && a.1 != b.1 {
            out[i].0 = to.0;
            out[j].0 = to.0;
        } else {
            return Err("reason-grip-not-axis");
        }
    }
    let (before, after) = (signed_area2(outline), signed_area2(&out));
    if after == 0 {
        return Err("reason-box-too-small");
    }
    // A grip never turns the box inside out (its CPs would then point inwards; fix round, item 6).
    if before.signum() != after.signum() {
        return Err("reason-box-inside-out");
    }
    Ok(out)
}

/// The outward normal of edge `a`–`b` of a polygon with the signed area `area2`; `None` for an edge that is not
/// axis-aligned.
pub fn outward(a: (i64, i64), b: (i64, i64), area2: i128) -> Option<Dir> {
    let v = ((b.0 - a.0).signum(), (b.1 - a.1).signum());
    if (v.0 != 0 && v.1 != 0) || v == (0, 0) || area2 == 0 {
        return None;
    }
    let n = if area2 > 0 { (v.1, -v.0) } else { (-v.1, v.0) };
    Dir::of_vector(n)
}

/// The grid point on the outline (page nm) nearest to `p`, with the edge's outward normal: per axis-aligned edge the
/// point projected, clamped and snapped along it; ties by the smaller edge index. `None` without such an edge.
pub fn nearest_on_outline(
    grid: PageGridMap,
    outline: &[(i64, i64)],
    p: (i64, i64),
) -> Option<((i64, i64), Dir)> {
    let n = outline.len();
    let area = signed_area2(outline);
    let snap_between = |t: i64, a: i64, b: i64| {
        let (lo, hi) = (a.min(b), a.max(b));
        let t = t.clamp(lo, hi);
        let k = grid.index(t);
        [k - 1, k, k + 1]
            .into_iter()
            .map(|k| grid.coord(k))
            .filter(|c| (lo..=hi).contains(c))
            .min_by_key(|c| ((c - t).abs(), (c - a).abs()))
            .unwrap_or(a)
    };
    let mut best: Option<(i128, usize, (i64, i64), Dir)> = None;
    for i in 0..n {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let q = if a.1 == b.1 && a.0 != b.0 {
            (snap_between(p.0, a.0, b.0), a.1)
        } else if a.0 == b.0 && a.1 != b.1 {
            (a.0, snap_between(p.1, a.1, b.1))
        } else {
            continue;
        };
        let Some(dir) = outward(a, b, area) else {
            continue;
        };
        let d = super::dist2(q, p);
        if best.is_none_or(|(bd, bi, ..)| d < bd || (d == bd && i < bi)) {
            best = Some((d, i, q, dir));
        }
    }
    best.map(|(_, _, q, d)| (q, d))
}

/// Whether `p` lies on an edge of the closed outline (exact).
pub fn on_outline(outline: &[(i64, i64)], p: (i64, i64)) -> bool {
    let n = outline.len();
    (0..n).any(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let cross =
            i128::from(b.0 - a.0) * i128::from(p.1 - a.1) - i128::from(b.1 - a.1) * i128::from(p.0 - a.0);
        cross == 0 && p.0 >= a.0.min(b.0) && p.0 <= a.0.max(b.0) && p.1 >= a.1.min(b.1) && p.1 <= a.1.max(b.1)
    })
}

/// The `box.reshape` of black box `rep` with the absolute outline `outline` and, when given, the absolute CPs
/// `(idx, point, direction)` (page nm), converted to the representation's frame (relative to its position, under the
/// inverse of its orientation).
pub fn reshape_command(rep: &TableRep, outline: &[(i64, i64)], cps: Option<&[BoxCpAt]>) -> Box<AnyCommand> {
    let inv = orient_inverse(rep.orient);
    let rel = |p: (i64, i64)| orient_point(inv, (p.0 - rep.pos.0, p.1 - rep.pos.1));
    let mut params = json!({
        "rep": rep.id.to_string(),
        "outline": outline.iter().map(|p| { let r = rel(*p); vec![r.0, r.1] }).collect::<Vec<_>>(),
    });
    if let Some(cps) = cps {
        params["cps"] = Value::Array(
            cps.iter()
                .map(|(i, p, d)| cp_json(*i, rel(*p), orient_dir(inv, *d)))
                .collect(),
        );
    }
    Box::new(AnyCommand {
        name: "box.reshape".into(),
        params,
    })
}

/// The black box `e` of the page's tables, when it is one.
pub fn black_box_rep<'t>(cx: &'t ToolCx<'_>, e: EntityRef) -> Option<&'t TableRep> {
    cx.page
        .tables
        .reps
        .iter()
        .find(|r| r.id == e.0 && r.black_box.is_some())
}

/// The selection's one black box, when exactly one entity is selected and it is a black box.
pub fn selected_black_box<'t>(cx: &'t ToolCx<'_>) -> Option<&'t TableRep> {
    match cx.selection.items.as_slice() {
        [one] => black_box_rep(cx, *one),
        _ => None,
    }
}

/// The overlay of a selected black box: a solid square per grip and a dashed square per CP, 3 points across each
/// half (shapes, not colour, tell them apart; NFR-A11Y-03).
pub fn grips_overlay(cx: &ToolCx<'_>, rep: &TableRep) -> Overlay {
    let Some(b) = &rep.black_box else {
        return Overlay::default();
    };
    let r = cx.nm_of_points(3.0).round() as i64;
    let square = |p: (i64, i64), style: Style| {
        let s = cx.snapper.page_to_sheet(p);
        OverlayItem::Rect {
            r: rect_of((s.0 - r, s.1 - r), (s.0 + r, s.1 + r)),
            style,
        }
    };
    let mut items: Vec<OverlayItem> = grip_points(&b.outline)
        .into_iter()
        .map(|p| square(p, Style::solid(Token::Ghost)))
        .collect();
    items.extend(
        b.cps
            .iter()
            .map(|(_, p, _)| square(*p, Style::dashed(Token::Ghost))),
    );
    Overlay::new(items)
}

/// The overlay of a reshape in progress: the new outline dashed and its CPs.
pub fn reshape_overlay(cx: &ToolCx<'_>, outline: &[(i64, i64)], cps: &[(i64, i64)]) -> Overlay {
    let mut pts: Vec<(i64, i64)> = outline.iter().map(|p| cx.snapper.page_to_sheet(*p)).collect();
    if let Some(&first) = pts.first() {
        pts.push(first);
    }
    let mut items = vec![OverlayItem::Polyline {
        points: pts,
        style: Style::dashed(Token::Ghost),
    }];
    items.extend(cps.iter().map(|p| OverlayItem::Marker {
        at: cx.snapper.page_to_sheet(*p),
        kind: MarkerKind::Cp,
    }));
    Overlay::new(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_cps_go_half_on_top_and_grow_the_rectangle() {
        let g = PageGridMap::new((1_000_000, 1));
        let cps: Vec<(u16, String)> = (0..7).map(|i| (i, i.to_string())).collect();
        let (pos, outline, laid) = layout(g, (2_000_000, 3_000_000), (4_000_000, 9_000_000), &cps).unwrap();
        assert_eq!(pos, (2_000_000, 3_000_000));
        // Four on top need five grid steps of width: the rectangle grew from two to five.
        assert_eq!(
            outline,
            vec![(0, 0), (5_000_000, 0), (5_000_000, 6_000_000), (0, 6_000_000)]
        );
        assert_eq!(laid[0], (0, (1_000_000, 0), Dir::Up));
        assert_eq!(laid[3], (3, (4_000_000, 0), Dir::Up));
        assert_eq!(laid[4], (4, (1_000_000, 6_000_000), Dir::Down));
        assert_eq!(laid[6], (6, (3_000_000, 6_000_000), Dir::Down));
        assert!(layout(g, (0, 0), (5_000_000, 0), &cps).is_none(), "no height");
        assert!(
            layout(g, (0, 0), (0, 5_000_000), &[]).is_none(),
            "no width and no CP"
        );
    }

    /// FT-550: on a box drawn wider than the CPs need, the CPs spread over the edge one common whole-grid pitch apart,
    /// the group centred, instead of packed from the left corner.
    #[test]
    fn the_default_cps_spread_over_the_drawn_edge_on_the_grid() {
        let g = PageGridMap::new((1_000_000, 1));
        let cps: Vec<(u16, String)> = (0..5).map(|i| (i, i.to_string())).collect();
        // Twelve steps wide: three on top at pitch 3 (12 / 4), from 3; two below at pitch 4 (12 / 3), from 4.
        let (_, outline, laid) = layout(g, (0, 0), (12_000_000, 6_000_000), &cps).unwrap();
        assert_eq!(outline[1], (12_000_000, 0), "the drawn width is kept");
        let xs: Vec<(i64, Dir)> = laid.iter().map(|(_, p, d)| (p.0 / 1_000_000, *d)).collect();
        assert_eq!(
            xs,
            [
                (3, Dir::Up),
                (6, Dir::Up),
                (9, Dir::Up),
                (4, Dir::Down),
                (8, Dir::Down)
            ]
        );
        // Every CP is a grid point strictly inside its edge, for any width the layout allows.
        for width in 1..40 {
            for m in 1..8 {
                let w = width.max(m + 1);
                let offsets: Vec<i64> = (0..m).map(|k| spread(w, m, k)).collect();
                assert!(
                    offsets[0] >= 1 && offsets[offsets.len() - 1] < w,
                    "{w} {m} {offsets:?}"
                );
                assert!(offsets.windows(2).all(|p| p[1] > p[0]), "{w} {m} {offsets:?}");
            }
        }
    }

    #[test]
    fn grips_keep_a_rectangle_and_move_edges_perpendicular() {
        let r = vec![(0, 0), (10, 0), (10, 5), (0, 5)];
        assert_eq!(grip_points(&r)[4..], [(5, 0), (10, 2), (5, 5), (0, 3)]);
        // Corner 2 to (12, 7): its neighbours follow on one axis each.
        assert_eq!(
            reshape_by_grip(&r, 2, (12, 7)).unwrap(),
            vec![(0, 0), (12, 0), (12, 7), (0, 7)]
        );
        // The right edge (grip 5 = edge v1v2) to x = 14.
        assert_eq!(
            reshape_by_grip(&r, 5, (14, 99)).unwrap(),
            vec![(0, 0), (14, 0), (14, 5), (0, 5)]
        );
        assert_eq!(reshape_by_grip(&r, 5, (0, 0)), Err("reason-box-too-small"));
        // The right edge past the left one would turn the box inside out.
        assert_eq!(reshape_by_grip(&r, 5, (-4, 0)), Err("reason-box-inside-out"));
        assert_eq!(reshape_by_grip(&r, 2, (-3, 7)), Err("reason-box-inside-out"));
        assert_eq!(reshape_by_grip(&r, 8, (0, 0)), Err("reason-no-grip"));
        // A triangle: a vertex alone; its diagonal edge refuses.
        let t = vec![(0, 0), (10, 0), (10, 5)];
        assert_eq!(
            reshape_by_grip(&t, 0, (2, 0)).unwrap(),
            vec![(2, 0), (10, 0), (10, 5)]
        );
        assert_eq!(reshape_by_grip(&t, 5, (0, 0)), Err("reason-grip-not-axis"));
    }

    #[test]
    fn the_nearest_outline_point_is_on_the_grid_with_the_outward_normal() {
        let g = PageGridMap::new((1_000_000, 1));
        let r = vec![(0, 0), (10_000_000, 0), (10_000_000, 5_000_000), (0, 5_000_000)];
        assert_eq!(
            nearest_on_outline(g, &r, (3_400_000, 4_600_000)),
            Some(((3_000_000, 5_000_000), Dir::Down))
        );
        assert_eq!(
            nearest_on_outline(g, &r, (10_300_000, 2_200_000)),
            Some(((10_000_000, 2_000_000), Dir::Right))
        );
        assert!(on_outline(&r, (10_000_000, 2_000_000)));
        assert!(!on_outline(&r, (9_000_000, 2_000_000)));
        assert_eq!(orient_inverse(1), 3);
        assert_eq!(orient_inverse(5), 5);
    }
}
