// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The property grid pane (WP-S2.05 part 1, spec §6.9): the shell's sheet drawn as the inline preview card, the header
//! and one block per section with collapsible groups; each row its label, its value or editor, the provenance chip
//! (an icon glyph and words, never colour alone) and its actions. The toast of a commit with several targets is one of
//! the window's toasts since WP-S2.09 part 1 (`crate::notifications`).
//!
//! WP-S2.17 part 3 (D11; doc 16a §6.8): the header's title in `type.title` and its subtitle in `type.small`; the Main
//! group (the kind's primary fields) first, then the categories; the Advanced rows (`$v`, ids, source references) only
//! after "Show advanced (n)" at the foot. A value cell edits in place (a click, with a hover border); an empty value
//! shows an em dash; the provenance chip only for a value that is not authored, its whole text in the tooltip. The
//! row's actions are one "…" button (the actions menu, `props.row_menu`), shown on the cursor row, on hover and on
//! focus: no edit buttons.
//!
//! **Input.** Every gesture is a registry action (`props.*`, `preview.*`); the pane builds no command.
//! While the grid has the shell's focus and no field is being edited: Up, Down, Home and End move the row cursor,
//! Left and Right collapse and expand the focused category, Enter or F2 edits, typing a character starts the edit with
//! it, Insert, Delete, Alt+Up and Alt+Down change a list. In the field: Enter or Tab commit (as does leaving it),
//! Escape reverts, Shift+Enter opens the multi-language popup.
//!
//! **Accessibility.** A row is a node with the role `Group`, named by its label, with the value, the provenance and
//! the error in its description; the editor is a text input labelled by the field; the card is a polite live region.
//! The pane announces nothing on a selection change.

use egui::{Key, Modifiers, RichText, Sense, TextEdit, Ui, Vec2};
use ew_viewmodel::properties::{
    ChoiceKind, FieldType, PropRow, PropertySheet, Provenance, RowAction, RowValue, SheetSection,
    advanced_count, chip_text, group_key, groups, is_header, read_only_reason, row_label, value_text,
};
use ew_viewmodel::shell::{Focus, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

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
    /// The options list box of the row being edited and where it was drawn in the last frame: a pointer press on it
    /// takes the field's focus, and that focus loss must not commit the buffer before the click chooses.
    choices_rect: Option<(String, egui::Rect)>,
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
        // A field is being typed into (its own keys), unless choices wait for a number key or a popup is open; a choice
        // editor's options keep the field focused (Up, Down and Enter are the field's).
        let options = props.choices().is_some_and(|c| c.kind == ChoiceKind::Options);
        let editing =
            props.edit().is_some() && (props.choices().is_none() || options) && props.popup().is_none();
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
            let advanced = shell.properties_show_advanced();
            let t = crate::theme::current(ui.ctx());
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for section in &sheet.sections {
                        if sheet.sections.len() > 1 {
                            ui.label(
                                RichText::new(format!(
                                    "{} ({})",
                                    strings::kind_plural(&section.kind),
                                    section.targets.len()
                                ))
                                .font(crate::theme::font(&t.type_body_strong)),
                            );
                        }
                        for (group, rows) in groups(section, advanced) {
                            let key = group_key(&section.kind, group);
                            let shut = collapsed.contains(&key);
                            if group_heading(ui, &t, group.title(), &key, shut) {
                                events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                                events.push(Event::Action(
                                    "props.category",
                                    json!({"category": group.word(), "expanded": shut}),
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
                    // The foot: "Show advanced (n)" or "Hide advanced (n)".
                    let n = advanced_count(sheet);
                    if n > 0 {
                        ui.add_space(t.space_2);
                        let key = if advanced {
                            "props-hide-advanced"
                        } else {
                            "props-show-advanced"
                        };
                        let text = strings::format(key, &[&n.to_string()]);
                        let link = ui.add(
                            egui::Button::new(
                                RichText::new(text)
                                    .color(t.accent.color32())
                                    .font(crate::theme::font(&t.type_small)),
                            )
                            .frame(false),
                        );
                        if link.clicked() {
                            events.push(Event::Action("props.show_advanced", json!({ "on": !advanced })));
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
        } else if pressed(Modifiers::NONE, Key::Space) {
            // Space toggles a boolean row's checkbox.
            if let Some((key, on)) = row.as_deref().and_then(|k| bool_row(shell, k)) {
                events.push(Event::Action(
                    "props.set",
                    json!({"row": key, "value": if on { "no" } else { "yes" }}),
                ));
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

    /// The header (§6.2): title, subtitle, location, finding chips (doc 16a §6.8: the title in `type.title`, the
    /// subtitle and the location in `type.small`, secondary).
    fn header(ui: &mut Ui, sheet: &PropertySheet) {
        let t = crate::theme::current(ui.ctx());
        ui.label(RichText::new(&sheet.header.title).font(crate::theme::font(&t.type_title)));
        let small = |s: &str| {
            RichText::new(s)
                .font(crate::theme::font(&t.type_small))
                .color(t.text_secondary.color32())
        };
        if let Some(s) = &sheet.header.subtitle {
            ui.label(small(s));
        }
        if let Some(l) = &sheet.header.location {
            ui.label(small(l));
        }
        for f in &sheet.header.findings {
            ui.label(format!("[{}] {} {}", f.severity, f.rule, f.message));
        }
        if let Some(r) = &sheet.read_only {
            ui.label(RichText::new(r).italics());
        }
        ui.separator();
    }

    /// The inline preview card (§6.6; WP-S2.17 part 4, doc 16a §6.23): "Preview of `<command>`" with its revision and
    /// status badge, the summary as a polite live region, the first tag changes, Apply and Cancel.
    fn card(&self, ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
        let Some(card) = shell.properties().card() else {
            return;
        };
        // A replace card is the find pane's (WP-S2.05 part 2).
        if !matches!(card.origin, ew_viewmodel::properties::CardOrigin::Properties) {
            return;
        }
        let kind = card.edit.kind.clone();
        let title = strings::format("card-title-properties", &[&card.preview.name]);
        let summary = strings::format(
            "card-line",
            &[
                &card.targets.to_string(),
                &ew_viewmodel::properties::kind_plural(&kind, card.targets),
                &card.changes.to_string(),
                &card.preview.derived.tags.len().to_string(),
                &card.preview.derived.xrefs.len().to_string(),
            ],
        );
        let tags: Vec<String> = card
            .preview
            .derived
            .tags
            .iter()
            .take(20)
            .map(|tag| format!("{} → {}", tag.before, tag.after))
            .collect();
        let inline = crate::components::InlineCard {
            title: &title,
            summary: &summary,
            apply: "preview.apply",
            strip: true,
        };
        crate::components::inline_card(ui, shell, inline, events, |ui, _| {
            for tag in &tags {
                ui.label(tag);
            }
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
        // D11: the chip stands only by a value that is not authored.
        let show_chip = !header && !matches!(row.provenance, Provenance::Authored);
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
        // Doc 16a §6.8: rows of `size.row`, the label column `size.property_label`, value cells
        // `size.control.inline`; a nested row is indented by `space.3` per level (16a sets no property indent).
        let t = crate::theme::current(ui.ctx());
        let (row_height, label_width, indent) = (t.size_row, t.size_property_label, t.space_3);
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, row_height), Sense::hover());
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
        child.spacing_mut().interact_size.y = t.size_control_inline;
        child.add_space(indent * f32::from(row.depth));
        let selected = focused == Some(row.key.as_str());
        let label_button = child.add_sized(
            [
                label_width - indent * f32::from(row.depth).min(6.0),
                t.size_control_inline,
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
            if let (FieldType::Bool, true) = (row.ty.inner(), row.actions.contains(&RowAction::Edit)) {
                // A checkbox (§6.5): a click sets the other value.
                let mut on = matches!(&row.value, RowValue::One { json, .. } if json.as_bool() == Some(true));
                let was = on;
                let response = child.checkbox(&mut on, "");
                let name = label.clone();
                response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, was, &name));
                if on != was {
                    events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                    events.push(Event::Action(
                        "props.set",
                        json!({"row": row.key, "value": if on { "yes" } else { "no" }}),
                    ));
                }
            } else {
                // The value cell: a click edits in place (doc 16a §6.8); an empty value is an em dash.
                let editable = !header && row.actions.contains(&RowAction::Edit);
                let actions_w = if row.actions.is_empty() {
                    0.0
                } else {
                    t.size_control_inline
                };
                let chip_w = if show_chip { t.size_property_label } else { 0.0 };
                let cell_w = (child.available_width() - actions_w - chip_w).max(t.size_control_inline);
                let (cell, response) = child.allocate_exact_size(
                    Vec2::new(cell_w, t.size_control_inline),
                    if editable { Sense::click() } else { Sense::hover() },
                );
                let shown = if value.is_empty() && !header {
                    strings::text("props-empty-value").to_owned()
                } else {
                    middle(&value)
                };
                // The cell's node: the value (a button while a click edits it), inside the row's group.
                let cell_name = shown.clone();
                let role = if editable {
                    egui::Role::Button
                } else {
                    egui::Role::Label
                };
                response.widget_info(|| egui::WidgetInfo::labeled(role, true, &cell_name));
                if editable && response.hovered() {
                    child.painter().rect_stroke(
                        cell,
                        crate::theme::radius(t.radius_sm),
                        crate::theme::stroke(t.border_width, t.border_default),
                        egui::StrokeKind::Inside,
                    );
                }
                let colour = if value.is_empty() {
                    t.text_tertiary
                } else {
                    t.text_primary
                };
                child.painter().with_clip_rect(cell).text(
                    egui::pos2(cell.left() + t.space_2, cell.center().y),
                    egui::Align2::LEFT_CENTER,
                    &shown,
                    crate::theme::font(&t.type_body),
                    colour.color32(),
                );
                if value.chars().count() > MAX_SHOWN {
                    response.clone().on_hover_text(&value);
                }
                if response.clicked() {
                    events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                    events.push(Event::Action("props.row", json!({"to": row.key})));
                    events.push(Event::Action("props.edit", json!({})));
                }
            }
            if show_chip {
                // The chip: its kind ("f derived"), the whole text in the tooltip.
                let short = chip.split(" · ").next().unwrap_or_default().to_owned();
                child
                    .label(
                        RichText::new(short)
                            .font(crate::theme::font(&t.type_caption))
                            .color(t.text_secondary.color32()),
                    )
                    .on_hover_text(&chip);
            }
            if !row.actions.is_empty() {
                // The actions menu: one "…" button on the cursor row, on hover and on focus.
                let id = group_id.with("actions");
                let (r, _) = child.allocate_exact_size(Vec2::splat(t.size_control_inline), Sense::hover());
                let name = strings::format("props-row-menu", &[&label]);
                let response = crate::command_bar::button_response(
                    &child,
                    id,
                    r,
                    crate::command_bar::Spec {
                        name: &name,
                        shortcut: "",
                        enabled: true,
                        checked: None,
                        sense: Sense::click(),
                    },
                );
                let hovered = child.rect_contains_pointer(rect);
                if selected || hovered || response.has_focus() {
                    crate::command_bar::state_overlay(&child, &t, &response, true, t.radius_sm);
                    let icon = egui::Rect::from_center_size(r.center(), Vec2::splat(t.icon_sm));
                    crate::command_bar::paint_icon(
                        &child,
                        "dots",
                        icon,
                        t.text_secondary.color32(),
                        t.icon_stroke,
                    );
                }
                let response = response.on_hover_text(&name);
                if response.clicked() {
                    events.push(Event::Action("ui.focus", json!({"area": "properties"})));
                    events.push(Event::Action("props.row", json!({"to": row.key})));
                    events.push(Event::Action("props.row_menu", json!({})));
                }
            }
        }
        // Below the row: its error, its choices, the notes of the last commit, the popups.
        if let Some(e) = error {
            ui.label(RichText::new(e).italics());
        }
        if let Some(c) = props.choices().filter(|c| c.row == row.key) {
            // The options of a choice editor (a combo for enums and colours, a filtered picker for parts and structure
            // nodes) and the readings of an ambiguous number: a list box, Up and Down move the highlight, Enter or a
            // click chooses (§6.5, §6.9).
            let list_id = ui.scope_id().with(("prop-choices", &row.key));
            let list_rect = ui.available_rect_before_wrap();
            let list = ui.interact(
                egui::Rect::from_min_size(list_rect.min, Vec2::new(list_rect.width(), 1.0)),
                list_id,
                Sense::hover(),
            );
            let list_name = strings::format("props-options", &[&label]);
            list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListBox, true, &list_name));
            ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("prop-choices-scroll", &row.key))
                    .max_height(t.size_row * t.size_chooser_rows as f32)
                    .show(ui, |ui| {
                        let top = ui.min_rect().min;
                        for (i, (text, _)) in c.items.iter().enumerate() {
                            let selected = c.index == Some(i);
                            let response = ui.add(egui::Button::selectable(selected, text));
                            let name = text.clone();
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, selected, &name)
                            });
                            if selected {
                                response.scroll_to_me(None);
                            }
                            if response.clicked() {
                                events.push(Event::Action("props.choose", json!({ "index": i })));
                            }
                        }
                        let rect = egui::Rect::from_min_max(top, ui.min_rect().max);
                        self.choices_rect = Some((row.key.clone(), ui.clip_rect().intersect(rect)));
                    });
            });
        } else if self.choices_rect.as_ref().is_some_and(|(r, _)| *r == row.key) {
            self.choices_rect = None;
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
        // A choice editor's options: Up and Down move the highlight; Enter then chooses it.
        let choices = shell.properties().choices().filter(|c| c.row == row.key);
        let highlighted = choices.is_some_and(|c| c.index.is_some());
        if choices.is_some() {
            if ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown))
            {
                events.push(Event::Action("props.choose", json!({"step": 1})));
            } else if ui
                .ctx()
                .input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowUp))
            {
                events.push(Event::Action("props.choose", json!({"step": -1})));
            }
        }
        let response = ui.add(
            TextEdit::singleline(&mut self.buffer)
                .desired_width(f32::INFINITY)
                .hint_text(label),
        );
        crate::focus::report_focus(
            &response,
            shell.focus() == Some(Focus::Properties),
            "properties",
            events,
        );
        let name = label.to_owned();
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::TextInput, true, &name));
        if fresh {
            response.request_focus();
        }
        if response.changed() {
            events.push(Event::Action("props.input", json!({ "text": self.buffer })));
        }
        // A pointer press on the options takes the field's focus: the click chooses, the focus loss commits nothing.
        let on_options = ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| {
            self.choices_rect
                .as_ref()
                .is_some_and(|(r, rect)| *r == row.key && rect.contains(p))
        });
        if response.lost_focus() && on_options && !escape {
            return;
        }
        if response.lost_focus() {
            self.editing = None;
            if escape {
                events.push(Event::Action("props.cancel", json!({})));
            } else if shift_enter {
                events.push(Event::Action("props.mltext", json!({"row": row.key})));
            } else if highlighted {
                events.push(Event::Action("props.choose", json!({})));
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

/// The focused row's key and value when it is an editable boolean (Space toggles it).
fn bool_row(shell: &ShellModel, key: &str) -> Option<(String, bool)> {
    let sheet = shell.properties().sheet()?;
    let (_, row) = ew_viewmodel::properties::find_row(sheet, key)?;
    if !matches!(row.ty.inner(), FieldType::Bool) || !row.actions.contains(&RowAction::Edit) {
        return None;
    }
    let on = matches!(&row.value, RowValue::One { json, .. } if json.as_bool() == Some(true));
    Some((row.key.clone(), on))
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
            | Key::Space
    );
    (plain && grid) || alt_arrow
}

/// A group's heading (doc 16a §6.8): a chevron and the title in `type.label`, a button that collapses or expands the
/// group; `true` when clicked.
fn group_heading(ui: &mut Ui, t: &crate::theme::Tokens, title: &str, key: &str, shut: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), t.size_row), Sense::hover());
    let id = ui.scope_id().with(("prop-group", key));
    let response = ui.interact(rect, id, Sense::click());
    let name = title.to_owned();
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, !shut, &name));
    crate::command_bar::state_overlay(ui, t, &response, true, t.radius_sm);
    let chevron = if shut { "chevron-right" } else { "chevron-down" };
    let icon = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - t.icon_sm / 2.0),
        Vec2::splat(t.icon_sm),
    );
    crate::command_bar::paint_icon(ui, chevron, icon, t.text_secondary.color32(), t.icon_stroke);
    let text = if t.type_label.upper {
        title.to_uppercase()
    } else {
        title.to_owned()
    };
    ui.painter().text(
        egui::pos2(icon.right() + t.space_icon, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        crate::theme::font(&t.type_label),
        t.text_secondary.color32(),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            crate::theme::radius(t.radius_sm),
            crate::theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Inside,
        );
    }
    response.clicked()
}
