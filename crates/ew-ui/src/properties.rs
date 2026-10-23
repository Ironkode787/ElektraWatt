// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid pane (WP-S2.05 part 1, spec §6.9): the shell's sheet drawn as the inline preview card, the toast,
//! the header and one block per section with collapsible categories; each row its label, its value or editor, the
//! provenance chip (an icon glyph and words, never colour alone) and its action buttons.
//!
//! **Input.** Every gesture is a registry action (`props.*`, `preview.*`, `toast.*`); the pane builds no command.
//! While the grid has the shell's focus and no field is being edited: Up, Down, Home and End move the row cursor,
//! Left and Right collapse and expand the focused category, Enter or F2 edits, typing a character starts the edit with
//! it, Insert, Delete, Alt+Up and Alt+Down change a list. In the field: Enter or Tab commit (as does leaving it),
//! Escape reverts, Shift+Enter opens the multi-language popup.
//!
//! **Accessibility.** A row is a node with the role `Group`, named by its label, with the value, the provenance and
//! the error in its description; the editor is a text input labelled by the field; the card and the toast are polite
//! live regions. The pane announces nothing on a selection change.

use egui::{Key, Modifiers, RichText, Sense, TextEdit, Ui, Vec2};
use ew_viewmodel::properties::{
    PropRow, PropertySheet, RowValue, SheetSection, categories, chip_text, collapse_key, is_header,
    read_only_reason, row_label, value_text,
};
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// The height of a row in points.
const ROW_HEIGHT: f32 = 22.0;

/// The width of a row's label column in points.
const LABEL_WIDTH: f32 = 120.0;

/// The indent of one nesting level in points.
const INDENT: f32 = 10.0;

/// A text longer than this many characters is ellipsised in the middle (the whole text in the tooltip).
const MAX_SHOWN: usize = 48;

/// The grid pane's state between frames.
#[derive(Default)]
pub struct PropertiesPane {
    /// The editor's text (the shell's buffer is set from it).
    buffer: String,
    /// The row the editor was opened for.
    editing: Option<String>,
    /// The multi-language popup's texts by language (`None`: neutral).
    mltext: Vec<(Option<String>, String)>,
}

/// `text` ellipsised in the middle at [`MAX_SHOWN`] characters.
fn middle(text: &str) -> String {
    let n = text.chars().count();
    if n <= MAX_SHOWN {
        return text.to_owned();
    }
    let head: String = text.chars().take(MAX_SHOWN / 2 - 1).collect();
    let tail: String = text.chars().skip(n - (MAX_SHOWN / 2 - 1)).collect();
    format!("{head}…{tail}")
}

impl PropertiesPane {
    /// A pane with no editor.
    pub fn new() -> PropertiesPane {
        PropertiesPane::default()
    }

    /// Paints the grid of `shell` into `ui`.
    pub(crate) fn show(&mut self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let props = shell.properties();
        // The pane's own focusable node: the grid's keys while it has the shell's focus.
        let rect = ui.available_rect_before_wrap();
        let pane_id = ui.scope_id().with("property-grid");
        let pane = ui.interact(rect, pane_id, Sense::focusable_noninteractive());
        let pane_name = strings::text("pane-properties").to_owned();
        pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &pane_name));
        let ours = shell.focus() == Some(Focus::Properties);
        let editing = props.edit().is_some() && props.choices().is_none() && props.popup().is_none();
        if ours && !editing && !pane.has_focus() && !ui.ctx().text_edit_focused() && shell.dialog().is_none()
        {
            pane.request_focus();
        }
        if pane.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    pane_id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        tab: false,
                        escape: false,
                    },
                );
            });
            if !editing {
                self.keys(ui, shell, events);
            }
        }
        if pane.gained_focus() && !ours {
            events.push(Event::Action("ui.focus", json!({"area": "properties"})));
        }
        ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
            self.card(ui, shell, events);
            self.toast(ui, shell, events);
            let Some(sheet) = props.sheet() else {
                ui.label(strings::text("props-empty"));
                return;
            };
            Self::header(ui, sheet);
            if sheet.sections.is_empty() {
                let text = if sheet.missing > 0 && sheet.header.items == 0 {
                    strings::text("props-gone-title").to_owned()
                } else if sheet.header.items == 0 {
                    strings::text("props-empty").to_owned()
                } else {
                    let kinds: Vec<String> = sheet.header.kinds.iter().map(|(k, _)| k.clone()).collect();
                    strings::format("props-none", &[&kinds.join(", ")])
                };
                ui.label(text);
                return;
            }
            let focused = shell.properties_row();
            let collapsed = shell.properties_collapsed();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for section in &sheet.sections {
                        ui.label(
                            RichText::new(format!(
                                "{} ({})",
                                strings::kind_plural(&section.kind),
                                section.targets.len()
                            ))
                            .strong(),
                        );
                        for (category, rows) in categories(section) {
                            let key = collapse_key(&section.kind, category);
                            let shut = collapsed.contains(&key);
                            let title = format!(
                                "{} {}",
                                if shut { "+" } else { "-" },
                                strings::text(&format!("category-{}", category.word()))
                            );
                            if ui.add(egui::Button::new(title).frame(false)).clicked() {
                                events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                                events.push(Event::Action(
                                    "props.category",
                                    json!({"category": category.word(), "expanded": shut}),
                                ));
                            }
                            if shut {
                                continue;
                            }
                            for row in rows {
                                self.row(ui, shell, sheet, section, row, focused.as_deref(), events);
                            }
                        }
                    }
                });
        });
    }

    /// The grid's keys while it has the focus and no field is edited (§6.9).
    fn keys(&self, ui: &Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let pressed = |m: Modifiers, k: Key| ui.ctx().input_mut(|i| i.consume_key(m, k));
        let row = shell.properties_row();
        // The actions popup: Up, Down and Enter.
        if let Some(ew_viewmodel::properties::Popup::Actions { index, .. }) = shell.properties().popup() {
            if pressed(Modifiers::NONE, Key::ArrowDown) {
                events.push(Event::Action("props.row_menu", json!({"step": 1})));
            } else if pressed(Modifiers::NONE, Key::ArrowUp) {
                events.push(Event::Action("props.row_menu", json!({"step": -1})));
            } else if pressed(Modifiers::NONE, Key::Enter) {
                events.push(Event::Action("props.choose", json!({ "index": index })));
            }
            return;
        }
        // The choices under a field: their number keys (1 to 9) choose.
        if let Some(c) = shell.properties().choices() {
            let keys = [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ];
            for (i, k) in keys.iter().enumerate().take(c.items.len()) {
                if pressed(Modifiers::NONE, *k) {
                    events.push(Event::Action("props.choose", json!({ "index": i })));
                    return;
                }
            }
        }
        if pressed(Modifiers::ALT, Key::ArrowUp) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "up"})));
        } else if pressed(Modifiers::ALT, Key::ArrowDown) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "down"})));
        } else if pressed(Modifiers::NONE, Key::ArrowDown) {
            events.push(Event::Action("props.row", json!({"step": 1})));
        } else if pressed(Modifiers::NONE, Key::ArrowUp) {
            events.push(Event::Action("props.row", json!({"step": -1})));
        } else if pressed(Modifiers::NONE, Key::Home) {
            events.push(Event::Action("props.row", json!({"to": "first"})));
        } else if pressed(Modifiers::NONE, Key::End) {
            events.push(Event::Action("props.row", json!({"to": "last"})));
        } else if pressed(Modifiers::NONE, Key::ArrowLeft) {
            events.push(Event::Action("props.category", json!({"expanded": false})));
        } else if pressed(Modifiers::NONE, Key::ArrowRight) {
            events.push(Event::Action("props.category", json!({"expanded": true})));
        } else if pressed(Modifiers::NONE, Key::Enter) {
            if shell.properties().card().is_none() {
                events.push(Event::Action("props.edit", json!({})));
            }
        } else if pressed(Modifiers::NONE, Key::Insert) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "add"})));
        } else if pressed(Modifiers::NONE, Key::Delete) {
            events.push(Event::Action("props.list", json!({"row": row, "op": "remove"})));
        } else {
            // A typed character on a focused editable row starts the edit with it.
            let typed = ui.ctx().input_mut(|i| {
                let mut text = None;
                i.events.retain(|e| match e {
                    egui::Event::Text(t) if text.is_none() && !t.trim().is_empty() => {
                        text = Some(t.clone());
                        false
                    }
                    _ => true,
                });
                text
            });
            if let Some(t) = typed {
                events.push(Event::Action("props.input", json!({ "text": t })));
            }
        }
    }

    /// The header (§6.2): title, subtitle, location, finding chips.
    fn header(ui: &mut Ui, sheet: &PropertySheet) {
        ui.label(RichText::new(&sheet.header.title).strong().size(15.0));
        if let Some(s) = &sheet.header.subtitle {
            ui.label(s);
        }
        if let Some(l) = &sheet.header.location {
            ui.label(l);
        }
        for f in &sheet.header.findings {
            ui.label(format!("[{}] {} {}", f.severity, f.rule, f.message));
        }
        if let Some(r) = &sheet.read_only {
            ui.label(RichText::new(r).italics());
        }
        ui.separator();
    }

    /// The inline preview card (§6.6): a polite live region with Apply and Cancel.
    fn card(&self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let (Some(card), Some(rev)) = (shell.properties().card(), shell.revision()) else {
            return;
        };
        egui::Frame::group(ui.style()).show(ui, |ui| {
            let kind = card.edit.kind.clone();
            let mut line = strings::format(
                "card-line",
                &[
                    &card.preview.name,
                    &card.targets.to_string(),
                    &ew_viewmodel::properties::kind_plural(&kind, card.targets),
                    &card.changes.to_string(),
                    &card.preview.derived.tags.len().to_string(),
                    &card.preview.derived.xrefs.len().to_string(),
                ],
            );
            match (&card.preview.result, card.preview.status(rev)) {
                (Err(m), _) => line = strings::format("card-failed", &[m]),
                (_, ew_viewmodel::tools::PreviewStatus::Stale) => {
                    line.push_str(&format!(" [{}]", strings::text("card-stale")));
                }
                _ => {}
            }
            let response = ui.label(&line);
            let live = line.clone();
            ui.ctx().accesskit_node_builder(response.id, |n| {
                n.set_live(egui::accesskit::Live::Polite);
                n.set_value(live);
            });
            for tag in card.preview.derived.tags.iter().take(20) {
                ui.label(format!("{} -> {}", tag.before, tag.after));
            }
            ui.horizontal(|ui| {
                if ui.button(strings::text("card-apply")).clicked() {
                    events.push(Event::Action("preview.apply", json!({})));
                }
                if ui.button(strings::text("card-cancel")).clicked() {
                    events.push(Event::Action("preview.cancel", json!({})));
                }
            });
        });
    }

    /// The toast (§6.6): a polite live region with Undo and Close; it never takes the focus.
    fn toast(&self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let Some(toast) = shell.properties().toast() else {
            return;
        };
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let response = ui.label(&toast.text);
                let live = toast.text.clone();
                ui.ctx().accesskit_node_builder(response.id, |n| {
                    n.set_live(egui::accesskit::Live::Polite);
                    n.set_value(live);
                });
                if toast.undo && ui.button(strings::text("toast-undo")).clicked() {
                    events.push(Event::Action("toast.undo", json!({})));
                }
                if ui.button(strings::text("toast-close")).clicked() {
                    events.push(Event::Action("toast.dismiss", json!({})));
                }
            });
        });
    }

    /// One row: a `Group` node with the label, the value or the editor, the chip and the actions.
    #[allow(clippy::too_many_arguments)]
    fn row(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        sheet: &PropertySheet,
        section: &SheetSection,
        row: &PropRow,
        focused: Option<&str>,
        events: &mut Vec<Event>,
    ) {
        let props = shell.properties();
        let label = row_label(section, row);
        let header = is_header(section, row);
        let value = if header { String::new() } else { value_text(row) };
        let reason = read_only_reason(sheet, section, row);
        let chip = if header {
            String::new()
        } else {
            chip_text(&row.provenance)
        };
        let error = props
            .error()
            .filter(|(r, _)| *r == row.key)
            .map(|(_, m)| m.to_owned());
        let mut description = format!("{value}; {chip}");
        if let Some(r) = &reason {
            description.push_str(&format!("; {r}"));
        }
        if let Some(e) = &error {
            description.push_str(&format!("; {e}"));
        }
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::hover());
        let group_id = ui.scope_id().with(("prop-row", &row.key));
        let group = ui.interact(rect, group_id, Sense::hover());
        let name = label.clone();
        group.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
        let desc = description.clone();
        ui.ctx().accesskit_node_builder(group_id, |n| {
            n.set_description(desc);
        });
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .accessibility_parent(group_id)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.add_space(INDENT * f32::from(row.depth));
        let selected = focused == Some(row.key.as_str());
        let label_button = child.add_sized(
            [
                LABEL_WIDTH - INDENT * f32::from(row.depth).min(6.0),
                ROW_HEIGHT - 2.0,
            ],
            egui::Button::selectable(selected, middle(&label)).frame(false),
        );
        if label_button.clicked() {
            events.push(Event::Action("ui.focus", json!({"area": "properties"})));
            events.push(Event::Action("props.row", json!({"to": row.key})));
        }
        let editing = props.edit().is_some_and(|e| e.row == row.key);
        if editing {
            self.editor(&mut child, shell, row, &label, events);
        } else {
            let shown = child.label(middle(&value));
            if value.chars().count() > MAX_SHOWN {
                shown.on_hover_text(&value);
            }
            if !chip.is_empty() {
                child.label(RichText::new(middle(&chip)).small());
            }
            for action in &row.actions {
                let text = action.word();
                if child.small_button(text).clicked() {
                    events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                    events.push(Event::Action(
                        "props.action",
                        json!({"row": row.key, "action": text}),
                    ));
                }
            }
        }
        // Below the row: its error, its choices, the notes of the last commit, the popups.
        if let Some(e) = error {
            ui.label(RichText::new(e).italics());
        }
        if let Some(c) = props.choices().filter(|c| c.row == row.key) {
            for (i, (text, _)) in c.items.iter().enumerate() {
                if ui.button(text).clicked() {
                    events.push(Event::Action("props.choose", json!({ "index": i })));
                }
            }
        }
        match props.popup() {
            Some(ew_viewmodel::properties::Popup::Actions { row: r, index }) if *r == row.key => {
                for (i, a) in row.actions.iter().enumerate() {
                    if ui.add(egui::Button::selectable(i == *index, a.word())).clicked() {
                        events.push(Event::Action("props.choose", json!({ "index": i })));
                    }
                }
            }
            Some(ew_viewmodel::properties::Popup::MlText { row: r }) if *r == row.key => {
                self.mltext_popup(ui, sheet, row, events);
            }
            _ => {
                self.mltext.clear();
            }
        }
        if selected && !props.notes().is_empty() {
            for n in props.notes() {
                ui.label(RichText::new(strings::format("props-notes", &[n])).small());
            }
        }
    }

    /// The row's editor: a text input labelled by the field. Enter, Tab and leaving it commit; Escape reverts;
    /// Shift+Enter opens the multi-language popup.
    fn editor(
        &mut self,
        ui: &mut Ui,
        shell: &ShellModel,
        row: &PropRow,
        label: &str,
        events: &mut Vec<Event>,
    ) {
        let Some(edit) = shell.properties().edit() else {
            return;
        };
        let fresh = self.editing.as_deref() != Some(row.key.as_str());
        if fresh || (!ui.ctx().text_edit_focused() && self.buffer != edit.buffer) {
            self.buffer.clone_from(&edit.buffer);
            self.editing = Some(row.key.clone());
        }
        let shift_enter = ui.input(|i| i.modifiers.shift && i.key_pressed(Key::Enter));
        let escape = ui.input(|i| i.key_pressed(Key::Escape));
        let response = ui.add(
            TextEdit::singleline(&mut self.buffer)
                .desired_width(f32::INFINITY)
                .hint_text(label),
        );
        let name = label.to_owned();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &name));
        if fresh {
            response.request_focus();
        }
        if response.changed() {
            events.push(Event::Action("props.input", json!({ "text": self.buffer })));
        }
        if response.lost_focus() {
            self.editing = None;
            if escape {
                events.push(Event::Action("props.cancel", json!({})));
            } else if shift_enter {
                events.push(Event::Action("props.mltext", json!({"row": row.key})));
            } else {
                events.push(Event::Action("props.commit", json!({})));
            }
        }
    }

    /// The multi-language popup (§6.5): a "Neutral" row and one row per project language, source first; Enter in a
    /// row sets that text.
    fn mltext_popup(&mut self, ui: &mut Ui, sheet: &PropertySheet, row: &PropRow, events: &mut Vec<Event>) {
        if self.mltext.is_empty() {
            let current = match &row.value {
                RowValue::One { json, .. } => json.clone(),
                _ => serde_json::Value::Null,
            };
            self.mltext
                .push((None, current.as_str().unwrap_or_default().to_owned()));
            for lang in &sheet.languages {
                let text = current
                    .get(lang)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                self.mltext.push((Some(lang.clone()), text));
            }
        }
        for (lang, text) in &mut self.mltext {
            ui.horizontal(|ui| {
                let name = lang
                    .clone()
                    .unwrap_or_else(|| strings::text("props-neutral").to_owned());
                ui.label(&name);
                let response = ui.add(TextEdit::singleline(text).hint_text(name.clone()));
                response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &name));
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    let mut params = json!({"row": row.key, "text": text});
                    if let Some(l) = lang {
                        params["lang"] = json!(l);
                    }
                    events.push(Event::Action("props.mltext", params));
                }
            });
        }
    }
}

/// Whether `s` is one of the grid's own keys (the arrows, Home, End, Enter, Insert, Delete), which a global chord must
/// not take while the grid has the focus.
pub fn is_grid_key(s: &egui::KeyboardShortcut) -> bool {
    let plain = s.modifiers == Modifiers::NONE;
    let alt_arrow = s.modifiers == Modifiers::ALT && matches!(s.logical_key, Key::ArrowUp | Key::ArrowDown);
    let grid = matches!(
        s.logical_key,
        Key::ArrowUp
            | Key::ArrowDown
            | Key::ArrowLeft
            | Key::ArrowRight
            | Key::Home
            | Key::End
            | Key::Enter
            | Key::Insert
            | Key::Delete
    );
    (plain && grid) || alt_arrow
}
