// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The macro tool (WP-S1.19 §11, FR-MAC-006): a macro chosen in the Macros navigator floats at the snapped grid point
//! under the pointer or the grid cursor, as the paste tool floats a fragment (its ghost is the macro's extent and the
//! label "Insert `<name>`").
//!
//! State `floating` (not sticky). `PointerMove` or `CursorMove` → `Transient`; `Press` or `Activate` → `Commit` of
//! the host step `macro.place {key, at}`, which is no command: the host composes `macro.insert` there
//! (`Session::insert_macro`, which reads the libraries) and opens its card with the conflicts, and Apply on the card
//! commits it. `Escape` answers nothing, so the Escape chain ends the tool. The tool reads no `Session`.

use ew_api::AnyCommand;
use serde_json::json;

use super::{Anchor, Button, CanvasEvent, Overlay, OverlayItem, Tool, ToolCx, ToolOutput, rect_of};
use crate::strings;

/// The host step a click of the macro tool asks for (not a command; the shell intercepts it by the member `$host` of
/// the output's parameters, [`is_macro_place`]).
pub const MACRO_PLACE: &str = "macro.place";

/// Whether the parameters of a tool's `Commit` are the macro tool's host step (`{"$host": "macro.place", …}`).
pub fn is_macro_place(params: &serde_json::Value) -> bool {
    params.get("$host").and_then(serde_json::Value::as_str) == Some(MACRO_PLACE)
}

/// The macro tool.
pub struct MacroTool {
    /// The Macros navigator's row key of the macro.
    key: String,
    /// The macro's name, for the label.
    name: String,
    /// The ghost about the anchor, `[x0, y0, x1, y1]` in nm.
    extent: Option<[i64; 4]>,
    /// The snapped anchor: (sheet nm, page nm), `None` before the first pointer or cursor move.
    at: Option<((i64, i64), (i64, i64))>,
}

impl MacroTool {
    /// A tool floating the macro of row `key`.
    pub fn new(key: String, name: String, extent: Option<[i64; 4]>) -> MacroTool {
        MacroTool {
            key,
            name,
            extent,
            at: None,
        }
    }

    fn moved(&mut self, at: (i64, i64), cx: &ToolCx<'_>) -> ToolOutput {
        let s = cx.snapper.snap_grid(at);
        self.at = Some((s.at_nm, s.page_nm));
        ToolOutput::Transient(self.overlay(cx))
    }

    /// The ghost rectangle at the anchor and the label.
    fn overlay(&self, cx: &ToolCx<'_>) -> Overlay {
        let Some((sheet, page)) = self.at else {
            return Overlay::default();
        };
        let rects = self
            .extent
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
                text: strings::format("label-macro", &[&self.name]),
                anchor: Anchor::Above,
            },
        ])
    }

    /// The host step at the anchor; `None` before the first move.
    fn place(&self) -> Option<ToolOutput> {
        let (_, at) = self.at?;
        Some(ToolOutput::Commit(Box::new(AnyCommand {
            name: MACRO_PLACE.into(),
            params: json!({"$host": MACRO_PLACE, "key": self.key, "at": [at.0, at.1]}),
        })))
    }
}

impl Tool for MacroTool {
    fn name(&self) -> &'static str {
        "macro"
    }

    fn state(&self) -> &'static str {
        "floating"
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
                self.place().unwrap_or(ToolOutput::None)
            }
            CanvasEvent::Activate => self.place().unwrap_or(ToolOutput::None),
            _ => ToolOutput::None,
        }
    }

    fn status_hint(&self) -> &'static str {
        "hint-macro"
    }

    fn wants_hover(&self) -> bool {
        false
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}
