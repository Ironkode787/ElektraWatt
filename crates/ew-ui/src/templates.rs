// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The New project and Templates dialogs' own content (doc 16 §3.7, doc 16a §6.12), between the dialog frame's body
//! and its footer. New project: the name, the folder with Browse… and where the project is made, Start from (one line
//! per start, each with what it brings) and Also copy its pages where the chosen start has pages. Templates: the list
//! (name, what it holds, folder), the selected template's actions, the line after an action, and New template… and
//! Add a folder…. Every control dispatches the action the shell's dialog names.

use egui::{RichText, TextEdit, Ui};
use ew_viewmodel::dialogs::{DialogAnswer, DialogButton};
use ew_viewmodel::key;
use ew_viewmodel::shell::{NewProjectDialog, TemplatesDialog};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::{self, Kind, Tip as _};
use crate::theme;

/// A line in `type.small`, secondary, wrapped, under the control above it: indented by `indent`, the line's own height
/// and no gap above it, so that it reads as that control's (window check F3, doc 16a §6.12).
fn under(ui: &mut Ui, t: &theme::Tokens, indent: f32, text: &str) {
    ui.add_space(-ui.spacing().item_spacing.y);
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 0.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            small(ui, t, text);
        });
    });
}

/// A line in `type.small`, secondary, wrapped.
fn small(ui: &mut Ui, t: &theme::Tokens, text: &str) {
    ui.add(egui::Label::new(crate::components::small_secondary(t, text)).wrap());
}

/// A label in the dialog's label column (`size.dialog.label`), then `space.3`, then `field` in the rest of the row.
fn row(ui: &mut Ui, t: &theme::Tokens, label: &str, field: impl FnOnce(&mut Ui, egui::Id)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui
            .allocate_ui_with_layout(
                egui::vec2(t.size_dialog_label, t.size_control),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(egui::vec2(t.size_dialog_label, t.size_control));
                    ui.label(label)
                },
            )
            .inner;
        ui.add_space(t.space_3);
        field(ui, label.id);
    });
}

/// A one-line text field that dispatches `new_project.set {<param>: text}` as it is typed; Enter in it presses
/// Create (`submit`). `focus`: it takes the keyboard focus.
fn text_field(
    ui: &mut Ui,
    value: &str,
    param: &str,
    width: f32,
    label: egui::Id,
    focus: bool,
    events: &mut Vec<Event>,
) -> bool {
    let t = components::tokens(ui);
    let mut text = value.to_owned();
    let edit = ui
        .add(
            TextEdit::singleline(&mut text)
                .desired_width(width)
                .min_size(egui::vec2(0.0, t.size_control)),
        )
        .labelled_by(label);
    if focus {
        edit.request_focus();
    }
    if text != value {
        events.push(Event::Action("new_project.set", json!({ param: text })));
    }
    edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

/// The New project dialog's content; `true` when Enter in a field asks for Create.
pub(crate) fn new_project(
    ui: &mut Ui,
    d: &NewProjectDialog,
    just_opened: bool,
    events: &mut Vec<Event>,
) -> bool {
    let t = components::tokens(ui);
    let mut submit = false;
    row(ui, &t, strings::text(key!("new-project-name")), |ui, label| {
        submit |= text_field(
            ui,
            &d.name,
            "name",
            ui.available_width(),
            label,
            just_opened,
            events,
        );
    });
    row(ui, &t, strings::text(key!("new-project-folder")), |ui, label| {
        let browse = strings::text(key!("new-project-browse"));
        let button = ui.fonts_mut(|f| {
            f.layout_no_wrap(browse.to_owned(), theme::font(&t.type_body), theme::NONE)
                .size()
                .x
        }) + 2.0 * ui.spacing().button_padding.x;
        let width = (ui.available_width() - button - t.space_3).max(0.0);
        submit |= text_field(ui, &d.folder, "folder", width, label, false, events);
        ui.add_space(t.space_3);
        if components::button(ui, Kind::Secondary, browse, None, 0.0).clicked() {
            events.push(Event::Action("new_project.browse", Value::Null));
        }
    });
    // Where the project is made, once there is a name and a folder; the line keeps its room before, so the rows under
    // it do not move when it appears.
    let made_in = if d.name.trim().is_empty() || d.folder.trim().is_empty() {
        " ".to_owned()
    } else {
        strings::format(key!("new-project-made-in"), &[&d.target().to_string_lossy()])
    };
    under(ui, &t, t.size_dialog_label + t.space_3, &made_in);
    ui.add_space(t.space_2);
    ui.label(crate::components::body_strong(
        &t,
        strings::text(key!("new-project-start-from")),
    ));
    // Every start on show with what it brings; a long list scrolls inside the dialog.
    egui::ScrollArea::vertical()
        .max_height(ui.ctx().content_rect().height() * 0.45)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (i, line) in d.starts.iter().enumerate() {
                ui.horizontal(|ui| {
                    let chosen = i == d.chosen;
                    if ui.radio(chosen, line.name.as_str()).clicked() && !chosen {
                        events.push(Event::Action("new_project.set", json!({ "start": i + 1 })));
                    }
                    if line.star {
                        small(
                            ui,
                            &t,
                            &format!("★ {}", strings::text(key!("new-project-for-new"))),
                        );
                    }
                    // Another project… chosen already: another one can be chosen.
                    if line.other
                        && line.dir.is_some()
                        && components::button(
                            ui,
                            Kind::Subtle,
                            strings::text(key!("new-project-other-change")),
                            None,
                            0.0,
                        )
                        .clicked()
                    {
                        events.push(Event::Action("new_project.other", Value::Null));
                    }
                });
                under(ui, &t, t.space_6, &line.what);
            }
        });
    let pages = d.start().pages;
    if pages > 0 {
        ui.add_space(t.space_2);
        let mut on = d.pages;
        let label = strings::format_count(ew_viewmodel::count_key!("new-project-copy-pages"), pages);
        if ui.checkbox(&mut on, label).changed() {
            events.push(Event::Action("new_project.set", json!({ "pages": on })));
        }
        under(ui, &t, t.space_6, strings::text(key!("new-project-pages-note")));
    }
    // Why Create cannot run, or why the last Create failed, over Create: a disabled button said it only on hover; else
    // what Create makes besides the project (a folder not there yet). The line keeps its room while there is nothing
    // to say, so the dialog does not move when one appears.
    let (reason, colour) = match (&d.error, d.problem()) {
        (Some(error), _) => (error.clone(), t.error),
        (None, Some(problem)) => (problem, t.text_secondary),
        (None, None) => (d.note().unwrap_or_else(|| " ".to_owned()), t.text_secondary),
    };
    ui.add_space(t.space_2);
    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
        ui.add(egui::Label::new(RichText::new(reason).color(colour.color32())).wrap());
    });
    submit
}

/// A row of buttons, each dispatching its action.
fn buttons(ui: &mut Ui, t: &theme::Tokens, list: &[DialogButton], events: &mut Vec<Event>) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        for b in list {
            let r = components::button(ui, Kind::Secondary, &b.label, b.disabled.as_deref(), 0.0);
            let r = match b.answer {
                DialogAnswer::Action("templates.remove", _) => {
                    r.tip(strings::text(key!("templates-remove-tip")))
                }
                _ => r,
            };
            if r.clicked()
                && let DialogAnswer::Action(action, params) = &b.answer
            {
                events.push(Event::Action(action, params.clone()));
            }
        }
    });
}

/// The Templates dialog's content: the list, the selected template's actions (or its name being changed), the line
/// after an action, and New template… and Add a folder….
pub(crate) fn templates(ui: &mut Ui, d: &TemplatesDialog, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let (row_actions, list_actions) = ew_viewmodel::dialogs::templates_actions(d);
    if d.rows.is_empty() {
        small(ui, &t, strings::text(key!("templates-none")));
    }
    egui::ScrollArea::vertical()
        .max_height(ui.ctx().content_rect().height() * 0.45)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (i, row) in d.rows.iter().enumerate() {
                let selected = i == d.selected;
                let frame = egui::Frame::new()
                    .fill(if selected {
                        t.accent_subtle.color32()
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .corner_radius(theme::radius(t.radius_md))
                    .inner_margin(egui::Margin::same(theme::margin_px(t.space_2)));
                let response = frame
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = t.space_1;
                        ui.horizontal(|ui| {
                            if d.renaming == Some(i) {
                                rename_field(ui, &t, i, &row.name, events);
                            } else {
                                ui.label(crate::components::body_strong(&t, &row.name));
                            }
                            if row.star {
                                small(ui, &t, &format!("★ {}", strings::text(key!("templates-for-new"))));
                            }
                        });
                        small(ui, &t, &row.what);
                        ui.label(crate::components::small_tertiary(
                            &t,
                            strings::format(key!("templates-folder"), &[&row.dir]),
                        ));
                        // The selected template's actions stand in its own row, so they read as its (not as
                        // buttons under the list that do not say which template they act on).
                        if selected && !row_actions.is_empty() {
                            ui.add_space(t.space_1);
                            buttons(ui, &t, &row_actions, events);
                        }
                    })
                    .response;
                // Only another row takes a click: over the selected one a click would land on the frame, not on its
                // name field or its buttons.
                let response = if selected {
                    response
                } else {
                    response.interact(egui::Sense::click())
                };
                response.widget_info(|| {
                    egui::WidgetInfo::selected(egui::Role::ListItem, true, selected, &row.name)
                });
                if response.clicked() {
                    events.push(Event::Action("templates.select", json!({ "index": i + 1 })));
                }
            }
        });
    if let Some(note) = &d.note {
        small(ui, &t, note);
    }
    ui.separator();
    buttons(ui, &t, &list_actions, events);
}

/// The selected template's name as a field: Enter, Save name or leaving the field renames it (`templates.rename`),
/// Escape leaves it.
fn rename_field(ui: &mut Ui, t: &theme::Tokens, i: usize, name: &str, events: &mut Vec<Event>) {
    let id = egui::Id::unique(("template-rename", i));
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| name.to_owned());
    let edit = ui.add(
        TextEdit::singleline(&mut text)
            .desired_width(t.size_text_measure.min(ui.available_width() * 0.5))
            .min_size(egui::vec2(0.0, t.size_control)),
    );
    let label = strings::text(key!("form-name"));
    ui.ctx().accesskit_node_builder(edit.id, |n| n.set_label(label));
    if !edit.has_focus() && !edit.lost_focus() && ui.data(|d| d.get_temp::<String>(id)).is_none() {
        edit.request_focus();
    }
    ui.data_mut(|d| d.insert_temp(id, text.clone()));
    // Enter, Tab or a click elsewhere: the name typed is kept; Escape leaves it as it was.
    let left = edit.lost_focus() && !ui.input(|i| i.key_pressed(egui::Key::Escape));
    let save = components::button(
        ui,
        Kind::Primary,
        strings::text(key!("templates-rename-ok")),
        None,
        0.0,
    )
    .clicked();
    if left || save {
        ui.data_mut(|d| d.remove::<String>(id));
        events.push(Event::Action(
            "templates.rename",
            json!({ "index": i + 1, "name": text }),
        ));
    }
}
