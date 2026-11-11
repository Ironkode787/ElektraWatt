// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The company-standard wizard screen (UX/UI doc 16 section 9.2, FR-PRJ-010): the steps on the left, each with a
//! check once done; in the middle the step's title, what it is and whether it can be changed later, its fields and
//! its Result, the worked example of what the fields make (live while a field is typed in), above a bar with Back
//! and the step's one Primary button (doc 16a §6.2); on the right the Example, the cover page or a schematic page
//! rendered by the CPU renderer with the current choices.
//!
//! Every control dispatches a `wizard.*` action. A text field commits when it loses the keyboard focus (Enter or a
//! click elsewhere), so the example page, which is rebuilt from its project, follows each committed value; the
//! Result follows each key.

use egui::{Align, Layout, Panel, RichText, TextEdit, Ui, Vec2};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::wizard::{CHECK_SCHEME_FIELD, FieldKind, FieldMeta, Step, Wizard, fields_of};
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::{self, Kind, Tip};
use crate::icons::IconId;
use crate::page_view::PageView;
use crate::theme::{self, Rgba};

fn set(events: &mut Vec<Event>, field: &str, value: &str) {
    events.push(Event::Action(
        "wizard.set",
        json!({ "field": field, "value": value }),
    ));
}

/// A text field that commits on losing the focus; the text being typed is kept in egui's temporary data and
/// returned while the field has the focus. Its placeholder is an example value (`wizard-placeholder-<field>`), when
/// the field has one.
fn text_field(ui: &mut Ui, events: &mut Vec<Event>, meta: &FieldMeta, value: &str) -> Option<String> {
    let id = ui.make_persistent_id(("wizard-field", meta.key));
    let shown: String = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| value.to_owned());
    let mut buffer = shown;
    let caption = ui.label(strings::text(meta.label_key));
    let mut edit = if meta.kind == FieldKind::Multiline {
        TextEdit::multiline(&mut buffer)
            .desired_rows(4)
            .desired_width(f32::INFINITY)
    } else {
        TextEdit::singleline(&mut buffer).desired_width(f32::INFINITY)
    };
    if let Some(example) = strings::lookup(&format!("wizard-placeholder-{}", meta.key)) {
        edit = edit.hint_text(example);
    }
    let response = ui.add(edit).labelled_by(caption.id);
    if response.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(id));
        if buffer != value {
            set(events, meta.key, &buffer);
        }
        None
    } else if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, buffer.clone()));
        Some(buffer)
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
        None
    }
}

/// A choice field: a radio group, one choice per line.
fn choice_field(ui: &mut Ui, events: &mut Vec<Event>, meta: &FieldMeta, choices: &[&str], value: &str) {
    ui.label(strings::text(meta.label_key));
    for choice in choices {
        let text = strings::text(strings::Key::run_time(&format!("choice-{}-{choice}", meta.key)));
        if ui.radio(*choice == value, text).clicked() && *choice != value {
            set(events, meta.key, choice);
        }
    }
}

/// The step's title, what it is, whether it can be changed later, then its fields and its Result (or, on the
/// review step, every field with its value).
fn step_form(ui: &mut Ui, wizard: &Wizard, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let step = wizard.step();
    // The Result beside the fields where two reading measures fit, else under them; the step's texts as wide as
    // the two together.
    let measure = t.size_text_measure;
    let wide = ui.available_width() >= 2.0 * measure + t.space_6;
    let width = if wide {
        2.0 * measure + t.space_6
    } else {
        measure.min(ui.available_width())
    };
    ui.scope(|ui| {
        ui.set_max_width(width);
        ui.label(
            RichText::new(format!("{}. {}", step.number(), strings::text(step.title_key())))
                .font(theme::font(&t.type_display)),
        );
        ui.add_space(t.space_2);
        ui.label(strings::text(step.hint_key()));
        ui.add_space(t.space_1);
        ui.label(
            RichText::new(strings::text(step.later_key()))
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
    });
    ui.add_space(t.space_5);
    if step == Step::Review {
        egui::Grid::new("wizard-review")
            .num_columns(3)
            .striped(true)
            .show(ui, |ui| {
                for line in wizard.review() {
                    ui.label(line.label);
                    ui.label(line.value);
                    // The check level is applied after finishing, by a person (its command is HumanOnly).
                    ui.weak(strings::text(if line.written {
                        key!("wizard-written")
                    } else if line.key == CHECK_SCHEME_FIELD {
                        key!("wizard-kept-apply")
                    } else {
                        key!("wizard-kept")
                    }));
                    ui.end_row();
                }
            });
        return;
    }
    let fields = |ui: &mut Ui, events: &mut Vec<Event>| {
        let mut typed: Option<(&str, String)> = None;
        for meta in fields_of(step) {
            let value = wizard.draft().get(meta.key).to_owned();
            match meta.kind {
                FieldKind::Choice(choices) => choice_field(ui, events, meta, choices, &value),
                FieldKind::Text | FieldKind::Multiline => {
                    if let Some(text) = text_field(ui, events, meta, &value) {
                        typed = Some((meta.key, text));
                    }
                }
            }
            ui.add_space(t.space_4);
        }
        if let Some(error) = wizard.error() {
            ui.colored_label(t.error.color32(), error);
            ui.add_space(t.space_4);
        }
        typed
    };
    let result = |ui: &mut Ui, typed: Option<(&str, String)>| {
        let lines = wizard.example(typed.as_ref().map(|(f, text)| (*f, text.as_str())));
        components::card_frame(&t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(strings::text(key!("wizard-example-heading")))
                    .font(theme::font(&t.type_body_strong)),
            );
            ui.add_space(t.space_2);
            for line in lines {
                ui.label(line);
            }
        });
    };
    if wide {
        ui.horizontal_top(|ui| {
            let typed = ui
                .vertical(|ui| {
                    ui.set_width(measure);
                    fields(ui, events)
                })
                .inner;
            ui.add_space(t.space_6);
            ui.vertical(|ui| {
                ui.set_width(measure);
                result(ui, typed);
            });
        });
    } else {
        ui.set_max_width(width);
        let typed = fields(ui, events);
        result(ui, typed);
    }
}

/// The bar at the foot of the step: Close and Start over as quiet buttons at the left, Back and the step's Primary
/// (Next, or Create on the last step) at the right; once the basic project is written, Apply the check level and
/// Close.
fn footer(ui: &mut Ui, wizard: &Wizard, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let mut push = |action: &'static str| events.push(Event::Action(action, Value::Null));
    let size = Vec2::new(ui.available_width(), t.size_control);
    ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        if wizard.finished_dir().is_none() {
            if components::button(ui, Kind::Subtle, strings::text(key!("wizard-cancel")), true, 0.0)
                .tip(strings::text(key!("wizard-cancel-tip")))
                .clicked()
            {
                push("wizard.cancel");
            }
            if components::button(ui, Kind::Subtle, strings::text(key!("wizard-reset")), true, 0.0)
                .tip(strings::text(key!("wizard-reset-tip")))
                .clicked()
            {
                push("wizard.reset");
            }
        }
        // Right to left: the rightmost button first.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if wizard.finished_dir().is_some() {
                let apply = wizard.can_apply_check_scheme();
                if apply
                    && components::button(
                        ui,
                        Kind::Primary,
                        strings::text(key!("wizard-apply-check-scheme")),
                        true,
                        0.0,
                    )
                    .clicked()
                {
                    push("wizard.apply_check_scheme");
                }
                let kind = if apply { Kind::Secondary } else { Kind::Primary };
                if components::button(ui, kind, strings::text(key!("wizard-close")), true, 0.0).clicked() {
                    push("wizard.cancel");
                }
                return;
            }
            let (label, action) = if wizard.step() == Step::Review {
                (key!("wizard-create"), "wizard.finish")
            } else {
                (key!("wizard-next"), "wizard.next")
            };
            if components::button(ui, Kind::Primary, strings::text(label), true, 0.0).clicked() {
                push(action);
            }
            if wizard.step() != Step::Company
                && components::button(ui, Kind::Secondary, strings::text(key!("wizard-back")), true, 0.0)
                    .clicked()
            {
                push("wizard.back");
            }
        });
    });
}

/// Paints the wizard into `ui`; `preview` is the page view that draws the example page.
pub fn show(ui: &mut Ui, shell: &ShellModel, preview: &mut PageView, events: &mut Vec<Event>) {
    let Some(wizard) = shell.wizard() else { return };
    let t = components::tokens(ui);
    Panel::left("wizard-steps")
        .resizable(false)
        .default_size(220.0)
        .show(ui, |ui| {
            ui.add_space(t.space_3);
            ui.label(RichText::new(strings::text(key!("wizard-title"))).font(theme::font(&t.type_title)));
            ui.add_space(t.space_3);
            for step in Step::ALL {
                let done = wizard.is_done(step);
                // The check of a done step; the others keep its room, so the titles line up.
                let mark = Rgba {
                    a: if done { t.success.a } else { 0 },
                    ..t.success
                };
                let text = format!("{}. {}", step.number(), strings::text(step.title_key()));
                let row = ui.selectable_label(
                    step == wizard.step(),
                    (
                        components::icon_atom(IconId::check, t.icon_sm, Some(mark), t.icon_stroke),
                        text,
                    ),
                );
                if done && row.ctx.has_accesskit_node(row.id) {
                    row.ctx.accesskit_node_builder(row.id, |node| {
                        node.set_description(strings::text(key!("wizard-step-done")));
                    });
                }
                if row.clicked() && step != wizard.step() {
                    events.push(Event::Action("wizard.goto", json!({ "step": step.index() })));
                }
            }
        });
    Panel::right("wizard-preview")
        .resizable(true)
        .default_size(440.0)
        .show(ui, |ui| {
            ui.add_space(t.space_3);
            ui.label(
                RichText::new(strings::text(key!("wizard-preview-title"))).font(theme::font(&t.type_title)),
            );
            ui.horizontal(|ui| {
                for (cover, key) in [
                    (true, key!("wizard-preview-cover")),
                    (false, key!("wizard-preview-page")),
                ] {
                    let selected = wizard.preview_is_cover() == cover;
                    if ui.selectable_label(selected, strings::text(key)).clicked() && !selected {
                        set(events, "preview", if cover { "cover" } else { "page" });
                    }
                }
            });
            ui.separator();
            let folio = wizard.preview_folio();
            preview.show_source(ui, wizard.preview_rev(), folio, &|_| {
                shell.wizard_preview_content()
            });
        });
    Panel::bottom("wizard-bar").show(ui, |ui| {
        ui.add_space(t.space_3);
        footer(ui, wizard, events);
        ui.add_space(t.space_3);
    });
    egui::CentralPanel::default().show(ui, |ui| {
        // The scroll area takes the panel's width, so its bar sits at the panel's edge and not on the fields.
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(t.space_3);
                step_form(ui, wizard, events);
                ui.add_space(t.space_4);
            });
    });
}
