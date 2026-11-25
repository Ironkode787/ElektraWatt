// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fields of the symbol editor's Properties: each commits on Enter, on losing the focus or at a drag's end, never
//! per keystroke, and Escape cancels the entry (a text through [`crate::components::committed_text`]).

use egui::Ui;

/// A number of the document named `name`: typed text commits on Enter or focus loss, a drag at its end, never per
/// frame (M25-p3#4). Returns whether `value` changed.
pub(super) fn number(
    ui: &mut Ui,
    value: &mut f64,
    speed: f64,
    range: std::ops::RangeInclusive<f64>,
    name: &str,
) -> bool {
    // M25-p5#3: a slot of its own beside the DragValue's id, whose precise drag value egui keeps as an `f64` too.
    let id = ui.next_auto_id().with("symbol-number");
    let mut v = ui.data(|d| d.get_temp::<f64>(id)).unwrap_or(*value);
    let response = ui.add(
        egui::DragValue::new(&mut v)
            .speed(speed)
            .range(range)
            .update_while_editing(false),
    );
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::drag_value(ui.is_enabled(), v);
        info.label = Some(name.to_owned());
        info
    });
    if response.dragged() {
        ui.data_mut(|d| d.insert_temp(id, v));
        return false;
    }
    ui.data_mut(|d| d.remove::<f64>(id));
    let commit = (response.drag_stopped() || response.changed()) && v != *value;
    if commit {
        *value = v;
    }
    commit
}
/// The key that starts a new line in a text that may hold one; Enter alone commits.
pub(super) fn new_line_key() -> egui::KeyboardShortcut {
    egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, egui::Key::Enter)
}

/// Whether Enter without Shift was pressed in the focused field `input`: the commit of a text that may hold a line
/// break.
pub(super) fn enter_pressed(ui: &Ui, input: egui::Id) -> bool {
    ui.memory(|m| m.has_focus(input))
        && ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(e, egui::Event::Key { key: egui::Key::Enter, pressed: true, modifiers, .. }
                    if !modifiers.shift)
            })
        })
}
