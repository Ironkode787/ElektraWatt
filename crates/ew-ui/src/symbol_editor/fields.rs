// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The fields of the symbol editor's Properties: each commits on Enter, on losing the focus or at a drag's end, never
//! per keystroke, and Escape cancels the entry.

use egui::Ui;

/// A text of the document that commits on Enter or focus loss, never per keystroke (M25-p3#4): the typing stays in
/// the field until then. `allow_empty` false keeps the previous text when the field is left empty, so a designation
/// can be cleared and retyped.
pub(crate) fn committed_text(
    ui: &mut Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    name: &str,
    caption: Option<&str>,
    value: &mut String,
    width: f32,
    allow_empty: bool,
) -> bool {
    let id = ui.make_persistent_id(salt);
    let (source, mut buffer) = ui
        .data_mut(|d| d.get_temp::<(String, String)>(id))
        .filter(|(source, _)| source == value)
        .unwrap_or_else(|| (value.clone(), value.clone()));
    let caption = caption.map(|c| ui.label(c));
    let mut response = ui.add(
        egui::TextEdit::singleline(&mut buffer)
            .id(id.with("input"))
            .desired_width(width),
    );
    if let Some(caption) = caption {
        response = response.labelled_by(caption.id);
    }
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::TextInput, ui.is_enabled(), name);
        info.current_text_value = Some(buffer.clone());
        info
    });
    // M25-p5#2 (doc 16a §6.8): Escape cancels the entry; it is the field's, so the tool stays.
    let escape = response.lost_focus() && cancelled(ui);
    let commit =
        response.lost_focus() && !escape && buffer != *value && (allow_empty || !buffer.trim().is_empty());
    if response.lost_focus() && !commit {
        buffer.clone_from(value);
    }
    ui.data_mut(|d| d.insert_temp(id, (source, buffer.clone())));
    if commit {
        *value = buffer;
    }
    commit
}

/// Whether the field that lost the focus this frame lost it to Escape, which then is the field's and goes no further
/// (doc 16a §6.8: Escape cancels).
pub(super) fn cancelled(ui: &Ui) -> bool {
    ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

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
