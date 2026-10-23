// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The shell's focus follows egui's for the panes with their own fields (the find pane, the property grid): a widget
//! that gains egui's focus while the shell's focus is elsewhere makes its pane the shell's focus, so the keys of the
//! canvas context (Tab for the wire proposals, Shift+Enter for `tool.finish`) are not offered while the user types
//! in the pane.

use egui::Response;
use serde_json::json;

use crate::app::Event;

/// Reports `ui.focus {"area": area}` when `response` gains the focus and the pane (`ours`) is not the shell's focus.
pub(crate) fn report_focus(response: &Response, ours: bool, area: &'static str, events: &mut Vec<Event>) {
    if response.gained_focus() && !ours {
        events.push(Event::Action("ui.focus", json!({ "area": area })));
    }
}
