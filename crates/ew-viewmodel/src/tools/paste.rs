// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The paste tool (WP-S2.05 part 3, spec §8.5): a parsed fragment floats at the snapped grid point under the pointer or
//! the grid cursor and is pasted with one `edit.paste`.
//!
//! State `floating` (not sticky). `PointerMove` or `CursorMove` → `Transient`: one ghost rectangle per ghost entry of
//! the fragment at the snapped anchor (the grid point, never a connection point: `snap_grid`), a label `paste <n>
//! records` with the dropped connections, and after a preview the predicted tags; `Dwell` of 80 ms and more →
//! `Preview(edit.paste {page, at, fragment})` (the semantic ghost: the derived diff's tags); `Press` or `Activate` →
//! `Commit`, after which the host returns to Select with the pasted representations selected; `Escape` answers
//! nothing, so the Escape chain ends the tool (the revision is unchanged). The tool reads no `Session`: the fragment was
//! parsed once by the action that armed it.

use std::sync::Arc;

use ew_api::AnyCommand;
use ew_api::output::ParsedFragment;
use serde_json::json;

use super::{
    Anchor, Button, CanvasEvent, DynCommand, Ghost, Overlay, OverlayItem, PREVIEW_DWELL_MS, PreviewInfo,
    Tool, ToolCx, ToolOutput, rect_of,
};
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
    /// The overlay of the last move, whose label a preview completes.
    last: Overlay,
    /// The held copies of another version the paste uses as the project holds them (`edit.paste`'s `adapt`,
    /// WP-S2.07 D22); empty for a plain paste.
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
            last: Overlay::default(),
            adapt: Vec::new(),
        }
    }

    /// The tool pasting with `adapt` (FT-396's Use the project's copy, WP-S2.07 D22).
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
        self.moved(cx.snapper.page_to_sheet(page), cx)
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
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
                text: self.label(),
                anchor: Anchor::Above,
            },
        ])
    }

    /// The label: `paste <n> records`, the dropped connections, the predicted tags.
    fn label(&self) -> String {
        let mut text = strings::format("label-paste", &[&self.fragment.records.to_string()]);
        if self.dropped > 0 {
            text.push_str(&strings::format(
                "label-paste-dropped",
                &[&self.dropped.to_string()],
            ));
        }
        if !self.tags.is_empty() {
            text.push_str(&format!(": {}", self.tags.join(", ")));
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
                self.command(cx).map_or(ToolOutput::None, ToolOutput::Commit)
            }
            CanvasEvent::Activate => self.command(cx).map_or(ToolOutput::None, ToolOutput::Commit),
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-paste"
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Paste {
            records: self.fragment.records,
            at_nm: self.at.map(|(_, page)| page),
            dropped: self.dropped,
        })
    }

    fn on_preview(&mut self, info: &PreviewInfo) -> Option<Overlay> {
        self.tags = if info.ok {
            info.tags.iter().map(|(_, after)| after.clone()).collect()
        } else {
            Vec::new()
        };
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
