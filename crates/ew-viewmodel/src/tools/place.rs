// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Place tool (`place`; WP-S2.04 §7.2, FR-SYM-015, FR-UI-017, FR-DEV-038): a symbol floating at the snapped
//! insertion point, committed with `device.place`.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | floating | `PointerMove` or `CursorMove` to p | floating | `Transient`: the shape's outline (`Paths`) at the grid point nearest to p, a `cp` marker per shape CP, a label with the variant letter, the target and a fresh preview's predicted tag |
//! | floating | `Key(RotateCw, RotateCcw, MirrorX, MirrorY)` | floating | orientation `orient_then(o, 1, false)`, `(3, false)`, `(0, true)`, `(2, true)`; `Transient` |
//! | floating | `Key(NextVariant)` | floating | orientation A→B→…→H→A; `Transient` |
//! | floating | `Dwell { ms ≥ 80 }` | floating | `Preview(device.place …)` |
//! | floating | `Press` primary or `Activate` | floating | `Commit(device.place …)`; sticky: the tool keeps floating |
//! | floating | `Back` | floating | `None` |
//! | floating | `Escape` | — | `None` (the host returns to Select) |
//!
//! `Key(NextRepType)` is the host's (it needs the catalogue): it replaces the shape through
//! [`PlaceTool::set_symbol`]. **The command** is `device.place {page, symbol, pos, orient, target}` with the armed
//! target, except that a `new_device` target whose insertion point lies inside the outline of a black box on the
//! page (even-odd, the boundary inside; the innermost by area, ties by ID) becomes `join_device {device: the box's
//! device, role: aux}` (doc 06 §2.7). A `new_terminal` target advances its designation after each commit when it is
//! decimal digits.

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::geometry::{orient_point, orient_then, variant_letter};
use ew_api::output::SymbolShape;
use ew_api::output::ew_dl::EntityRef;
use serde_json::{Value, json};

use super::{
    Anchor, Button, CanvasEvent, Ghost, MarkerKind, Overlay, OverlayItem, PreviewInfo, Tool, ToolCx, ToolKey,
    ToolOutput,
};
use crate::strings;

/// The JSON spellings of the orientations A to H.
pub const ORIENT_NAMES: [&str; 8] = ["r0", "r90", "r180", "r270", "m0", "m90", "m180", "m270"];

/// What a placement creates (the armed target of §7.1).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlaceTarget {
    /// A new device with this letter.
    NewDevice {
        /// The letter code.
        letter: String,
    },
    /// Another representation of an existing function (`place.pick` with `function`).
    Function {
        /// The function.
        function: String,
    },
    /// A device made from a part (part-first placement).
    Part {
        /// The part copy.
        part: String,
        /// The template entry the representation draws.
        template: u16,
        /// The new device's letter.
        letter: String,
        /// The part number (the label's).
        number: String,
    },
    /// One new terminal of a strip.
    Terminal {
        /// The strip device.
        strip: String,
        /// The strip's tag as the page prints it (the label's, `1X1`); `None` while nothing on the page prints it
        /// (a strip `strip.new` just made: the host reads it from the first terminal's CP label).
        strip_tag: Option<String>,
        /// The terminal's designation.
        designation: String,
    },
}

/// The Place tool.
#[derive(Clone, Debug)]
pub struct PlaceTool {
    shape: Arc<SymbolShape>,
    outline: Arc<[Vec<(i64, i64)>]>,
    orient: u8,
    target: PlaceTarget,
    /// The snapped insertion point: (sheet nm, page nm).
    at: Option<((i64, i64), (i64, i64))>,
    /// The predicted tag of a fresh preview.
    tag: Option<String>,
    /// The last overlay answered.
    last: Overlay,
    /// The target's word at the current point (`join` inside a black box).
    word: &'static str,
    /// Whether a `new_terminal` target waits for its next designation.
    ask: bool,
}

/// Whether `p` lies inside the polygon (even-odd rule, the boundary inside), exactly in `i128`.
pub fn inside_polygon(p: (i64, i64), poly: &[(i64, i64)]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let (px, py) = (i128::from(p.0), i128::from(p.1));
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (ax, ay, bx, by) = (i128::from(a.0), i128::from(a.1), i128::from(b.0), i128::from(b.1));
        // On the edge: collinear and within the bounding box.
        let cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
        if cross == 0 && px >= ax.min(bx) && px <= ax.max(bx) && py >= ay.min(by) && py <= ay.max(by) {
            return true;
        }
        if (ay > py) != (by > py) {
            // x of the edge at py, compared without division: px < ax + (py − ay)(bx − ax)/(by − ay).
            let lhs = (px - ax) * (by - ay);
            let rhs = (py - ay) * (bx - ax);
            let crosses = if by > ay { lhs < rhs } else { lhs > rhs };
            if crosses {
                inside = !inside;
            }
        }
    }
    inside
}

/// Twice the absolute area of a polygon (shoelace), in `i128`.
fn area2(poly: &[(i64, i64)]) -> i128 {
    let n = poly.len();
    let mut s: i128 = 0;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        s += i128::from(a.0) * i128::from(b.1) - i128::from(b.0) * i128::from(a.1);
    }
    s.abs()
}

/// The next designation after `d` when it is decimal digits (`9` → `10`), else `None`.
pub fn next_designation(d: &str) -> Option<String> {
    if d.is_empty() || !d.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    d.parse::<u64>()
        .ok()
        .and_then(|n| n.checked_add(1))
        .map(|n| n.to_string())
}

impl PlaceTool {
    /// The tool floating `shape` with `target`, at orientation A.
    pub fn new(shape: Arc<SymbolShape>, target: PlaceTarget) -> PlaceTool {
        let outline: Arc<[Vec<(i64, i64)>]> = shape.outline.clone().into();
        PlaceTool {
            shape,
            outline,
            orient: 0,
            target,
            at: None,
            tag: None,
            last: Overlay::default(),
            word: "",
            ask: false,
        }
    }

    /// The floating symbol's shape.
    pub fn shape(&self) -> &Arc<SymbolShape> {
        &self.shape
    }

    /// The armed target.
    pub fn target(&self) -> &PlaceTarget {
        &self.target
    }

    /// The orientation `0..=7`.
    pub fn orient(&self) -> u8 {
        self.orient
    }

    /// Replaces the shape of the same symbol read at a new revision; the overlay follows on the next move.
    pub fn replace_shape(&mut self, shape: Arc<SymbolShape>) {
        self.outline = shape.outline.clone().into();
        self.shape = shape;
    }

    /// Replaces the floating symbol (the next representation type, Shift+V), keeping the target, the orientation
    /// and the point; answers the new overlay.
    pub fn set_symbol(&mut self, shape: Arc<SymbolShape>, cx: &ToolCx<'_>) -> ToolOutput {
        self.outline = shape.outline.clone().into();
        self.shape = shape;
        self.tag = None;
        self.transient(cx)
    }

    /// The target the command sends at the insertion point `pos` (page nm): a `new_device` target inside a black
    /// box joins the box's device.
    fn effective_target(&self, cx: &ToolCx<'_>, pos: (i64, i64)) -> (Value, &'static str, String) {
        match &self.target {
            PlaceTarget::NewDevice { letter } => {
                let mut best: Option<(i128, EntityRef, &ew_api::output::TableRep)> = None;
                for r in &cx.page.tables.reps {
                    let (Some(b), Some(_)) = (&r.black_box, r.device) else {
                        continue;
                    };
                    if !inside_polygon(pos, &b.outline) {
                        continue;
                    }
                    let a = area2(&b.outline);
                    if best.is_none_or(|(ba, bid, _)| a < ba || (a == ba && EntityRef(r.id) < bid)) {
                        best = Some((a, EntityRef(r.id), r));
                    }
                }
                match best {
                    Some((_, _, r)) => {
                        let device = r.device.unwrap_or_default();
                        let tag = cx
                            .page
                            .tables
                            .cps
                            .iter()
                            .find(|c| c.rep == r.id)
                            .and_then(|c| c.label.rsplit_once(':').map(|(t, _)| t.to_owned()))
                            .unwrap_or_else(|| strings::text("place-target-box").to_owned());
                        (
                            json!({"join_device": {"device": device.to_string(), "role": "aux"}}),
                            "join",
                            strings::format("place-target-join", &[&tag]),
                        )
                    }
                    None => (
                        json!({"new_device": {"letter": letter}}),
                        "new",
                        strings::format("place-target-new", &[letter]),
                    ),
                }
            }
            PlaceTarget::Function { function } => (
                json!({"existing_function": {"function": function}}),
                "function",
                strings::text("place-target-function").to_owned(),
            ),
            PlaceTarget::Part {
                part,
                template,
                letter,
                number,
            } => (
                json!({"from_part": {"part": part, "template": template, "letter": letter}}),
                "part",
                strings::format("place-target-part", &[number]),
            ),
            PlaceTarget::Terminal {
                strip,
                strip_tag,
                designation,
            } => (
                json!({"new_terminal": {"strip": strip, "designation": designation}}),
                "terminal",
                strings::format(
                    "place-target-terminal",
                    &[&self.strip_label(strip_tag), &self.designation_label(designation)],
                ),
            ),
        }
    }

    /// The command at the current point; `None` while the tool asks for a designation.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<AnyCommand>> {
        if self.ask {
            return None;
        }
        let (_, pos) = self.at?;
        let (target, _, _) = self.effective_target(cx, pos);
        Some(Box::new(AnyCommand {
            name: "device.place".into(),
            params: json!({
                "page": cx.page.tables.page.to_string(),
                "symbol": self.shape.copy.to_string(),
                "pos": [pos.0, pos.1],
                "orient": ORIENT_NAMES[usize::from(self.orient % 8)],
                "target": target,
            }),
        }))
    }

    /// The label text: the variant letter, the target and the predicted tag.
    fn label(&self, cx: &ToolCx<'_>, pos: (i64, i64)) -> String {
        let (_, _, target) = self.effective_target(cx, pos);
        let mut text = format!("{} {target}", variant_letter(self.orient));
        if let Some(tag) = &self.tag {
            text.push(' ');
            text.push_str(tag);
        }
        text
    }

    /// The overlay at the current point.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let Some((sheet, pos)) = self.at else {
            return Overlay::default();
        };
        let mut items = vec![OverlayItem::Paths {
            paths: Arc::clone(&self.outline),
            transform: (self.orient, pos),
        }];
        for cp in &self.shape.cps {
            let o = orient_point(self.orient, cp.at);
            items.push(OverlayItem::Marker {
                at: cx.snapper.page_to_sheet((pos.0 + o.0, pos.1 + o.1)),
                kind: MarkerKind::Cp,
            });
        }
        items.push(OverlayItem::Label {
            at: sheet,
            text: self.label(cx, pos),
            anchor: Anchor::Above,
        });
        Overlay::new(items)
    }

    fn transient(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let o = self.overlay(cx);
        if let Some((_, pos)) = self.at {
            self.word = self.effective_target(cx, pos).1;
        }
        self.last = o.clone();
        ToolOutput::Transient(o)
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap_grid(at);
        let next = Some((s.at_nm, s.page_nm));
        if next != self.at {
            self.tag = None;
        }
        self.at = next;
        self.transient(cx)
    }

    fn turn(&mut self, rotate: u8, mirror: bool, cx: &ToolCx<'_>) -> ToolOutput {
        self.orient = orient_then(self.orient, rotate, mirror);
        self.tag = None;
        self.transient(cx)
    }

    fn commit(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let Some(cmd) = self.command(cx) else {
            return ToolOutput::None;
        };
        self.tag = None;
        ToolOutput::Commit(cmd)
    }

    /// The label of a strip tag (`(new strip)` while unknown).
    fn strip_label(&self, tag: &Option<String>) -> String {
        tag.clone()
            .unwrap_or_else(|| strings::text("strip-tag-unknown").to_owned())
    }

    /// The label of the designation (`?` while the tool asks for the next one).
    fn designation_label(&self, d: &str) -> String {
        if self.ask { "?".to_owned() } else { d.to_owned() }
    }

    /// Whether a `new_terminal` target waits for the next designation (§7.2: after a commit with a designation that
    /// is not decimal digits the next commit asks again; `text.input` or a new `place.pick` gives it).
    pub fn needs_designation(&self) -> bool {
        self.ask
    }

    /// The tag of the strip of a `new_terminal` target, once the page prints it.
    pub fn set_strip_tag(&mut self, tag: String) {
        if let PlaceTarget::Terminal { strip_tag, .. } = &mut self.target {
            *strip_tag = Some(tag);
        }
    }

    /// Whether the target is a `new_terminal` whose strip's tag is not known yet.
    pub fn strip_tag_unknown(&self) -> bool {
        matches!(&self.target, PlaceTarget::Terminal { strip_tag: None, .. })
    }
}

impl Tool for PlaceTool {
    fn name(&self) -> &'static str {
        "place"
    }

    fn state(&self) -> &'static str {
        "floating"
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, .. } | CanvasEvent::CursorMove { at } => self.moved(*at, cx),
            CanvasEvent::Key(ToolKey::RotateCw) => self.turn(1, false, cx),
            CanvasEvent::Key(ToolKey::RotateCcw) => self.turn(3, false, cx),
            CanvasEvent::Key(ToolKey::MirrorX) => self.turn(0, true, cx),
            CanvasEvent::Key(ToolKey::MirrorY) => self.turn(2, true, cx),
            CanvasEvent::Key(ToolKey::NextVariant) => {
                self.orient = (self.orient + 1) % 8;
                self.tag = None;
                self.transient(cx)
            }
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command(cx) {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => {
                let _ = self.moved(*at, cx);
                self.commit(cx)
            }
            CanvasEvent::Activate => self.commit(cx),
            CanvasEvent::Text(text) => match &mut self.target {
                PlaceTarget::Terminal { designation, .. } if !text.trim().is_empty() => {
                    *designation = text.trim().to_owned();
                    self.ask = false;
                    self.transient(cx)
                }
                _ => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn on_commit(&mut self, done: bool) {
        // The designation advances only after the host committed (fix round, item 2): a refused commit keeps it.
        if !done {
            return;
        }
        if let PlaceTarget::Terminal { designation, .. } = &mut self.target {
            match next_designation(designation) {
                Some(next) => *designation = next,
                None => self.ask = true,
            }
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-place"
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        let strip = match &self.target {
            PlaceTarget::Terminal {
                strip_tag,
                designation,
                ..
            } => Some(format!(
                "{}:{}",
                self.strip_label(strip_tag),
                self.designation_label(designation)
            )),
            _ => None,
        };
        let (target, letter) = match &self.target {
            PlaceTarget::NewDevice { letter } => {
                let word = if self.word == "join" { "join" } else { "new" };
                (word, Some(letter.clone()))
            }
            PlaceTarget::Function { .. } => ("function", None),
            PlaceTarget::Part { letter, .. } => ("part", Some(letter.clone())),
            PlaceTarget::Terminal { .. } => ("terminal", None),
        };
        Some(Ghost::Place {
            symbol: self.shape.key.clone(),
            variant: variant_letter(self.orient),
            rep_type: self.shape.rep_type.clone(),
            at_nm: self.at.map(|(_, p)| p),
            target,
            letter,
            strip,
            tag: self.tag.clone(),
            hints: 0,
        })
    }

    fn on_preview(&mut self, info: &PreviewInfo) -> Option<Overlay> {
        if !info.ok {
            return None;
        }
        let tag = info
            .tags
            .iter()
            .find(|(before, _)| before.is_empty())
            .or_else(|| info.tags.first())
            .map(|(_, after)| after.clone())?;
        // The label of the last overlay gains the tag (the overlay is otherwise what the last move drew).
        let mut o = self.last.clone();
        for item in &mut o.items {
            if let OverlayItem::Label { text, .. } = item {
                text.push(' ');
                text.push_str(&tag);
            }
        }
        self.tag = Some(tag);
        self.last = o.clone();
        Some(o)
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_point_in_polygon_test_is_even_odd_with_the_boundary_inside() {
        let square = [(0, 0), (10, 0), (10, 10), (0, 10)];
        assert!(inside_polygon((5, 5), &square));
        assert!(inside_polygon((0, 5), &square), "on an edge");
        assert!(inside_polygon((10, 10), &square), "a vertex");
        assert!(!inside_polygon((11, 5), &square));
        assert!(!inside_polygon((5, -1), &square));
        // A concave L.
        let l = [(0, 0), (10, 0), (10, 4), (4, 4), (4, 10), (0, 10)];
        assert!(inside_polygon((2, 8), &l));
        assert!(!inside_polygon((8, 8), &l));
        assert_eq!(area2(&square), 200);
    }

    #[test]
    fn decimal_designations_advance() {
        assert_eq!(next_designation("9").as_deref(), Some("10"));
        assert_eq!(next_designation("PE"), None);
        assert_eq!(next_designation(""), None);
    }
}
