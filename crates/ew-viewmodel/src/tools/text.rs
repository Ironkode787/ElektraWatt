// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Text tool (`text`; WP-S2.04 §10.5, §10.7) and text edit (F2, a double click on a selected free text), with the
//! six bindable names a free text may show instead of its own text (§10.2 item 2).
//!
//! **The Text tool.** All points are page nm, fine-snapped ([`crate::tools::Snapper::snap_fine`],
//! `FineKind::Text`); the overlay is in sheet nm. A bound text has nothing to type: a click places it at once with
//! `bind` instead of `text`.
//!
//! | State | Event | Next | Output |
//! |---|---|---|---|
//! | armed | `PointerMove`, `CursorMove` | armed (at the snapped point) | `Transient`: a grid marker |
//! | armed, no binding | `Press` primary or `Activate` with a point | typing (at, "") | `Transient`: the marker |
//! | armed, a binding | `Press` primary or `Activate` with a point | armed | `Commit(item.place {free_text: {bind, at}})` |
//! | armed | `Activate` before any point | armed | `None` |
//! | typing | `Text(s)` | typing (text = `s`) | `Transient`: the marker and a label (none for an empty text) |
//! | typing | `Activate` or `Key(Finish)`, text not blank | armed (at kept) | `Commit(item.place {free_text: {text, at}})` |
//! | typing | `Activate` or `Key(Finish)`, text blank | typing | `None` |
//! | typing | `Dwell { ms ≥ 80 }`, text not blank | typing | `Preview` of that `item.place` |
//! | typing | `PointerMove`, `CursorMove`, `Press`, `Release`, `Back` | typing | `None` (the text stays at its point) |
//! | typing | `Escape` | armed (at kept) | `Cancel` |
//! | armed | `Escape` | — | `None` (the host returns to Select) |
//!
//! **Text edit.** One state, `typing`: `Text(s)` shows a label at the item's point; Enter (or `tool.finish`) commits
//! `fields.set {kind: page_item, targets: [id], path: "kind.free_text.text", value}` — unchanged text and a blank text
//! commit nothing (the first finishes the edit). A dwell previews a changed, non-blank text.

use ew_api::AnyCommand;
use serde_json::json;

use super::{
    Anchor, Button, CanvasEvent, FineKind, Ghost, MarkerKind, Overlay, OverlayItem, Tool, ToolCx, ToolKey,
    ToolOutput,
};

/// The six bindable names of a free text (WP-S2.04 §10.2 item 2), in this order.
pub const BINDABLE: [&str; 6] = [
    "project.description",
    "project.document_title",
    "project.drawing_id",
    "page.title",
    "page.folio",
    "page.full_name",
];

/// The Text tool's state.
#[derive(Clone, PartialEq, Eq, Debug)]
enum State {
    /// No point fixed yet, or the point the next text goes to.
    Armed { at: Option<(i64, i64)> },
    /// The text is being typed at this point (page nm).
    Typing { at: (i64, i64), text: String },
}

/// The Text tool (T): clicks a free text's point, types its text, Enter places it.
#[derive(Clone, Debug)]
pub struct TextTool {
    state: State,
    bind: Option<String>,
}

impl TextTool {
    /// The tool, armed, without a binding.
    pub fn new() -> TextTool {
        TextTool {
            state: State::Armed { at: None },
            bind: None,
        }
    }

    /// The chosen binding, while one is set.
    pub fn bind(&self) -> Option<&str> {
        self.bind.as_deref()
    }

    /// Sets or clears the binding; the host checks the name against [`BINDABLE`].
    pub fn set_bind(&mut self, bind: Option<String>) {
        self.bind = bind;
    }

    /// The point the text goes to (page nm).
    fn point(&self) -> Option<(i64, i64)> {
        match &self.state {
            State::Armed { at } => *at,
            State::Typing { at, .. } => Some(*at),
        }
    }

    /// The text typed so far (`""` while armed).
    fn text(&self) -> &str {
        match &self.state {
            State::Armed { .. } => "",
            State::Typing { text, .. } => text,
        }
    }

    /// The overlay at the current point: a grid marker and, while typing, the label.
    pub fn refresh(&mut self, cx: &ToolCx<'_>) -> Overlay {
        let Some(at) = self.point() else {
            return Overlay::default();
        };
        let at = cx.snapper.page_to_sheet(at);
        let mut items = vec![OverlayItem::Marker {
            at,
            kind: MarkerKind::Grid,
        }];
        if !self.text().is_empty() {
            items.push(OverlayItem::Label {
                at,
                text: self.text().to_owned(),
                anchor: Anchor::Right,
            });
        }
        Overlay::new(items)
    }

    /// The `item.place` of the text or the binding at `at` (page nm).
    fn command(&self, cx: &ToolCx<'_>, at: (i64, i64), text: &str) -> Box<AnyCommand> {
        let free = match &self.bind {
            Some(b) => json!({"bind": b, "at": [at.0, at.1]}),
            None => json!({"text": text, "at": [at.0, at.1]}),
        };
        Box::new(AnyCommand {
            name: "item.place".into(),
            params: json!({
                "page": cx.page.tables.page.to_string(),
                "kind": {"free_text": free},
            }),
        })
    }

    /// A move: fine-snap the point and refresh the marker (typing ignores moves).
    fn moved(&mut self, at: (i64, i64), alt: bool, cx: &ToolCx<'_>) -> ToolOutput {
        let p = cx.snapper.snap_fine(at, FineKind::Text, alt).1;
        match &mut self.state {
            State::Armed { at } => *at = Some(p),
            State::Typing { .. } => return ToolOutput::None,
        }
        ToolOutput::Transient(self.refresh(cx))
    }

    /// A click at the current point.
    fn click(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Armed { at: Some(at) } = self.state else {
            return ToolOutput::None;
        };
        if let Some(bind) = self.bind.clone() {
            // A bound text has nothing to type: the click places it (the lane's ruling).
            return ToolOutput::Commit(self.command(cx, at, &bind));
        }
        self.state = State::Typing {
            at,
            text: String::new(),
        };
        ToolOutput::Transient(self.refresh(cx))
    }

    /// Enter or `tool.finish` while typing: commit a non-blank text.
    fn finish(&mut self, cx: &ToolCx<'_>) -> ToolOutput {
        let State::Typing { at, text } = &self.state else {
            return ToolOutput::None;
        };
        if text.trim().is_empty() {
            return ToolOutput::None;
        }
        let (at, text) = (*at, text.clone());
        self.state = State::Armed { at: Some(at) };
        ToolOutput::Commit(self.command(cx, at, &text))
    }
}

impl Default for TextTool {
    fn default() -> TextTool {
        TextTool::new()
    }
}

impl Tool for TextTool {
    fn name(&self) -> &'static str {
        "text"
    }

    fn state(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "armed",
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
            CanvasEvent::Activate => self.click(cx),
            CanvasEvent::Text(text) => {
                if let State::Typing { text: t, .. } = &mut self.state {
                    *t = text.clone();
                    ToolOutput::Transient(self.refresh(cx))
                } else {
                    ToolOutput::None
                }
            }
            CanvasEvent::Key(ToolKey::Finish) => self.finish(cx),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match &self.state {
                State::Typing { at, text } if !text.trim().is_empty() => {
                    ToolOutput::Preview(self.command(cx, *at, text))
                }
                _ => ToolOutput::None,
            },
            CanvasEvent::Escape => match &self.state {
                State::Typing { at, .. } => {
                    self.state = State::Armed { at: Some(*at) };
                    ToolOutput::Cancel
                }
                State::Armed { .. } => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        match self.state {
            State::Armed { .. } => "hint-text-armed",
            State::Typing { .. } => "hint-text-typing",
        }
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Text {
            at_nm: self.point(),
            text: self.text().to_owned(),
            bind: self.bind.clone(),
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// Text edit (F2, or a double click on a selected free text): changes a free text's own text in one `fields.set`.
#[derive(Clone, Debug)]
pub struct TextEditTool {
    /// The item's UUID text (`fields.set` takes it as a target).
    item: String,
    /// The label's point on the page (the item's bounds' top-left corner), nm.
    at: (i64, i64),
    original: String,
    text: String,
    finished: bool,
}

impl TextEditTool {
    /// The tool editing `item`'s text, which starts as `text`.
    pub fn new(item: String, at: (i64, i64), text: String) -> TextEditTool {
        TextEditTool {
            item,
            at,
            original: text.clone(),
            text,
            finished: false,
        }
    }

    /// Whether the host returned to Select (the text was committed, or Enter found it unchanged).
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// The `fields.set` of the typed text, when it is changed and not blank.
    fn command(&self) -> Option<Box<AnyCommand>> {
        if self.text == self.original || self.text.trim().is_empty() {
            return None;
        }
        Some(Box::new(AnyCommand {
            name: "fields.set".into(),
            params: json!({
                "kind": "page_item",
                "targets": [self.item.as_str()],
                "path": "kind.free_text.text",
                "value": self.text.as_str(),
            }),
        }))
    }

    /// Enter or `tool.finish`.
    fn finish(&mut self) -> ToolOutput {
        if self.text.trim().is_empty() {
            return ToolOutput::None;
        }
        if self.text == self.original {
            self.finished = true;
            return ToolOutput::Cancel;
        }
        match self.command() {
            Some(cmd) => ToolOutput::Commit(cmd),
            None => ToolOutput::None,
        }
    }
}

impl Tool for TextEditTool {
    fn name(&self) -> &'static str {
        "text_edit"
    }

    fn state(&self) -> &'static str {
        "typing"
    }

    fn on_event(&mut self, ev: &CanvasEvent, cx: &ToolCx<'_>) -> ToolOutput {
        match ev {
            CanvasEvent::Text(text) => {
                self.text = text.clone();
                ToolOutput::Transient(Overlay::new(vec![OverlayItem::Label {
                    at: cx.snapper.page_to_sheet(self.at),
                    text: text.clone(),
                    anchor: Anchor::Right,
                }]))
            }
            CanvasEvent::Activate | CanvasEvent::Key(ToolKey::Finish) => self.finish(),
            CanvasEvent::Dwell { ms } if *ms >= super::PREVIEW_DWELL_MS => match self.command() {
                Some(cmd) => ToolOutput::Preview(cmd),
                None => ToolOutput::None,
            },
            _ => ToolOutput::None,
        }
    }

    fn on_commit(&mut self, done: bool) {
        if done {
            self.finished = true;
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-text-edit"
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn ghost(&self) -> Option<Ghost> {
        Some(Ghost::Text {
            at_nm: Some(self.at),
            text: self.text.clone(),
            bind: None,
        })
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
