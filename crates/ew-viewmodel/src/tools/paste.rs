// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The paste tool: a parsed fragment floats at the snapped grid point under the pointer or
//! the grid cursor and is pasted with one `edit.paste`.
//!
//! State `floating` (not sticky). `PointerMove` or `CursorMove` → `Transient`: one ghost rectangle per ghost entry of
//! the fragment, its middle at the point (moved in to stand two grid steps inside the frame's drawing area where it
//! fits, its symbols' numbers clear of the frame) and its anchor snapped to the grid (never to a connection point:
//! `snap_grid`; Duplicate starts with the anchor at its point, Paste in place with the anchor where the copy's was), a
//! label `paste <n> records` with the dropped connections, and after a preview the predicted tags; where a part of it
//! would lie outside the frame's drawing area (a free text or graphic: outside the sheet) the label says so and `Press`
//! or `Activate` is refused with that reason (a paste ended off the sheet and
//! nothing said so); the host floats it at the pointer (else the view's middle) as soon as it is armed, so the ghost
//! shows before the first move; `Dwell` of 80 ms and more →
//! `Preview(edit.paste {page, at, fragment})` (the semantic ghost: the derived diff's tags); `Press` or `Activate` →
//! `Commit`, after which the host returns to Select with the pasted representations selected; `Escape` answers
//! nothing, so the Escape chain ends the tool (the revision is unchanged). The tool reads no `Session`: the fragment was
//! parsed once by the action that armed it.

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::window::ParsedFragment;
use serde_json::json;

use super::{
    Anchor, Button, CanvasEvent, DynCommand, Ghost, Overlay, OverlayItem, PREVIEW_DWELL_MS, PreviewInfo,
    Tool, ToolCx, ToolOutput, rect_of,
};
use crate::key;
use crate::strings;

/// The paste tool.
pub struct PasteTool {
    /// The fragment, as `parse_fragment` read it.
    fragment: Arc<ParsedFragment>,
    /// The connections the copy dropped (0 for a fragment from elsewhere).
    dropped: usize,
    /// The snapped anchor: (sheet nm, page nm), `None` before the first pointer or cursor move.
    at: Option<((i64, i64), (i64, i64))>,
    /// The tags the last accepted preview predicts (the derived diff's new tags), for the label.
    tags: Vec<String>,
    /// What the last accepted preview says the paste does besides (`Outcome::notices`: the wires it removes with the
    /// devices it brings back), for the label.
    notices: Vec<String>,
    /// The overlay of the last move, whose label a preview completes.
    last: Overlay,
    /// The held copies of another version the paste uses as the project holds them (`edit.paste`'s `adapt`); empty for a plain paste.
    adapt: Vec<ew_api::output::Uuid>,
}

impl PasteTool {
    /// A tool floating `fragment`.
    pub fn new(fragment: Arc<ParsedFragment>, dropped: usize) -> PasteTool {
        PasteTool {
            fragment,
            dropped,
            at: None,
            tags: Vec::new(),
            notices: Vec::new(),
            last: Overlay::default(),
            adapt: Vec::new(),
        }
    }

    /// The tool pasting with `adapt` (Use the project's copy).
    #[must_use]
    pub fn adapting(mut self, adapt: Vec<ew_api::output::Uuid>) -> PasteTool {
        self.adapt = adapt;
        self
    }

    /// The fragment's record count.
    pub fn records(&self) -> usize {
        self.fragment.records
    }

    /// Moves the ghost's anchor to the page point `page` (duplicate's start, two grid steps right and down of the
    /// copied anchor), snapped to the grid.
    pub fn start_at(&mut self, page: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        self.anchor_at(cx.snapper.page_to_sheet(page), cx)
    }

    /// The fragment floated with its middle at the sheet point `at` (the host's first ghost).
    pub fn float_at(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        self.moved(at, cx)
    }

    /// The fragment with its anchor where the copy's anchor was: the same page coordinates (Paste in place), snapped
    /// to the page's grid.
    pub fn in_place(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let a = &self.fragment.params["anchor"];
        let anchor = (a[0].as_i64().unwrap_or(0), a[1].as_i64().unwrap_or(0));
        self.start_at(anchor, cx)
    }

    /// Whether a part of the fragment at its anchor lies outside the frame's drawing area (`ip::drawing_area`): a free
    /// text or graphic (the fragment's `free`) only where it would leave the sheet, since the Text tool puts texts in the
    /// frame's bands too.
    fn outside(&self, cx: &ToolCx<'_>) -> bool {
        let (Some((_, page)), Some(area)) = (self.at, super::ip::drawing_area(cx.page)) else {
            return false;
        };
        let (w, h) = cx.page.sheet;
        let (a, b) = (cx.snapper.sheet_to_page((0, 0)), cx.snapper.sheet_to_page((w, h)));
        let sheet = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
        self.fragment.ghost.iter().enumerate().any(|(i, g)| {
            let area = if self.fragment.free.contains(&i) {
                sheet
            } else {
                area
            };
            let (x0, x1) = (
                page.0.saturating_add(g[0].min(g[2])),
                page.0.saturating_add(g[0].max(g[2])),
            );
            let (y0, y1) = (
                page.1.saturating_add(g[1].min(g[3])),
                page.1.saturating_add(g[1].max(g[3])),
            );
            x0 < area[0] || y0 < area[1] || x1 > area[2] || y1 > area[3]
        })
    }

    /// The fragment's middle relative to its anchor (page nm): the centre of its ghost rectangles' bounds.
    fn middle(&self) -> (i64, i64) {
        let g = &self.fragment.ghost;
        let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
            g.iter().map(|r| r[0].min(r[2])).min(),
            g.iter().map(|r| r[1].min(r[3])).min(),
            g.iter().map(|r| r[0].max(r[2])).max(),
            g.iter().map(|r| r[1].max(r[3])).max(),
        ) else {
            return (0, 0);
        };
        (x0 / 2 + x1 / 2, y0 / 2 + y1 / 2)
    }

    /// The pointer or the grid cursor at the sheet point `at`: the fragment floats with its middle there, snapped to
    /// the grid (it hung from its top-left corner, so a paste landed below and right of the click).
    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let page = cx.snapper.sheet_to_page(at);
        let (mx, my) = self.middle();
        let anchor = self.inside((page.0.saturating_sub(mx), page.1.saturating_sub(my)), cx);
        self.anchor_at(cx.snapper.page_to_sheet(anchor), cx)
    }

    /// The anchor `anchor` (page nm) moved so that the block's symbols and wires stand two grid steps inside the
    /// frame's drawing area, their numbers clear of it, where the block fits in it;
    /// a block wider or taller than the area stays where it is on that axis, and is
    /// refused there.
    fn inside(&self, anchor: (i64, i64), cx: &ToolCx<'_>) -> (i64, i64) {
        let Some(area) = super::ip::drawing_area(cx.page) else {
            return anchor;
        };
        let fixed = || {
            self.fragment
                .ghost
                .iter()
                .enumerate()
                .filter(|(i, _)| !self.fragment.free.contains(i))
                .map(|(_, g)| g)
        };
        let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
            fixed().map(|r| r[0].min(r[2])).min(),
            fixed().map(|r| r[1].min(r[3])).min(),
            fixed().map(|r| r[0].max(r[2])).max(),
            fixed().map(|r| r[1].max(r[3])).max(),
        ) else {
            return anchor;
        };
        let step = cx.snapper.page_of_index(2).saturating_abs();
        let clamp = |a: i64, lo: i64, hi: i64, b0: i64, b1: i64| {
            let (min, max) = (
                lo.saturating_add(step).saturating_sub(b0),
                hi.saturating_sub(step).saturating_sub(b1),
            );
            if min <= max { a.clamp(min, max) } else { a }
        };
        (
            clamp(anchor.0, area[0], area[2], x0, x1),
            clamp(anchor.1, area[1], area[3], y0, y1),
        )
    }

    /// The anchor at the sheet point `at`, snapped to the grid.
    fn anchor_at(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap_grid(at);
        let next = Some((s.at_nm, s.page_nm));
        if next != self.at {
            self.tags.clear();
        }
        self.at = next;
        self.last = self.overlay(cx);
        ToolOutput::Transient(self.last.clone())
    }

    /// The ghost rectangles at the anchor and the label.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let Some((sheet, page)) = self.at else {
            return Overlay::default();
        };
        let rects = self
            .fragment
            .ghost
            .iter()
            .map(|g| {
                let a = cx
                    .snapper
                    .page_to_sheet((page.0.saturating_add(g[0]), page.1.saturating_add(g[1])));
                let b = cx
                    .snapper
                    .page_to_sheet((page.0.saturating_add(g[2]), page.1.saturating_add(g[3])));
                rect_of(a, b)
            })
            .collect();
        Overlay::new(vec![
            OverlayItem::Ghost {
                rects,
                offset: (0, 0),
            },
            OverlayItem::Label {
                at: sheet,
                text: if self.outside(cx) {
                    format!(
                        "{} · {}",
                        self.label(),
                        strings::text(key!("label-paste-outside"))
                    )
                } else {
                    self.label()
                },
                anchor: Anchor::Beside,
            },
        ])
    }

    /// The label: `paste <n> records`, the dropped connections, the predicted tags, and the preview's notices (the wires
    /// a paste that brings cut devices back removes on another page).
    /// What the block holds and what it leaves out, in a person's words ("paste 21 records, 5 connections
    /// dropped"): "Paste 3 symbols and 4 wires; 2 wires left out: their other end was not copied". Symbols are the
    /// representations, wires the connections, other items the texts, graphics and boxes; the records behind them
    /// (devices, functions, junctions) are not counted.
    fn label(&self) -> String {
        let count = |kinds: &[&str]| {
            self.fragment.params["records"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|r| r["$k"].as_str().is_some_and(|k| kinds.contains(&k)))
                .count()
        };
        let parts: Vec<String> = [
            (
                crate::count_key!("label-paste-symbols"),
                count(&["representation"]),
            ),
            (crate::count_key!("label-paste-wires"), count(&["connection"])),
            (
                crate::count_key!("label-paste-other"),
                count(&["page_item", "structure_box"]),
            ),
        ]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(k, n)| strings::format_count(k, n))
        .collect();
        let what = parts
            .into_iter()
            .reduce(|a, b| strings::format(key!("label-paste-and"), &[&a, &b]))
            .unwrap_or_default();
        let mut text = strings::format(key!("label-paste"), &[&what]);
        if self.dropped > 0 {
            text.push_str(&strings::format_count(
                crate::count_key!("label-paste-dropped"),
                self.dropped,
            ));
        }
        if !self.tags.is_empty() {
            text.push_str(&format!(": {}", self.tags.join(", ")));
        }
        for n in &self.notices {
            text.push_str(&format!("; {n}"));
        }
        text
    }

    /// `edit.paste {page, at, fragment, adapt?}` at the anchor; `None` before the first move or without a page.
    fn command(&self, cx: &ToolCx<'_>) -> Option<Box<dyn DynCommand>> {
        let (_, at) = self.at?;
        let page = cx.page.tables.page;
        if page.is_nil() {
            return None;
        }
        let mut params =
            json!({"page": page.to_string(), "at": [at.0, at.1], "fragment": self.fragment.params});
        if !self.adapt.is_empty() {
            params["adapt"] = json!(self.adapt.iter().map(ToString::to_string).collect::<Vec<_>>());
        }
        Some(Box::new(AnyCommand {
            name: "edit.paste".into(),
            params,
        }))
    }
}

impl Tool for PasteTool {
    fn name(&self) -> &'static str {
        "paste"
    }

    fn state(&self) -> &'static str {
        "floating"
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::PointerMove { at, .. } | CanvasEvent::CursorMove { at } => self.moved(*at, cx),
            CanvasEvent::Dwell { ms } if *ms >= PREVIEW_DWELL_MS => match self.command(cx) {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            CanvasEvent::Press {
                at,
                button: Button::Primary,
                ..
            } => {
                let _ = self.moved(*at, cx);
                if self.outside(cx) {
                    return ToolOutput::Refused(key!("reason-paste-outside"));
                }
                self.command(cx).map_or(ToolOutput::None, ToolOutput::Commit)
            }
            CanvasEvent::Activate if self.outside(cx) => ToolOutput::Refused(key!("reason-paste-outside")),
            CanvasEvent::Activate => self.command(cx).map_or(ToolOutput::None, ToolOutput::Commit),
            _ => ToolOutput::None,
        }
    }

    fn follows(&self, cmd: &dyn super::DynCommand) -> super::Follows {
        let _ = cmd;
        // A paste brings back what a cut left unplaced and says what it did; a committed paste returns to Select with
        // the pasted representations selected.
        super::Follows {
            paste: true,
            retag: true,
            ..super::Follows::default()
        }
    }

    fn status_hint(&self) -> super::Hint {
        super::Hint::new(key!("hint-paste"), &[super::ENTER, super::ESCAPE])
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Paste {
            records: self.fragment.records,
            at_nm: self.at.map(|(_, page)| page),
            dropped: self.dropped,
            words: self.label(),
        })
    }

    fn on_preview(&mut self, info: &PreviewInfo) -> Option<Overlay> {
        self.tags = if info.ok {
            info.tags.iter().map(|(_, after)| after.clone()).collect()
        } else {
            Vec::new()
        };
        self.notices.clone_from(&info.notices);
        let label = self.label();
        for item in &mut self.last.items {
            if let OverlayItem::Label { text, .. } = item {
                text.clone_from(&label);
            }
        }
        (!self.last.is_empty()).then(|| self.last.clone())
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
