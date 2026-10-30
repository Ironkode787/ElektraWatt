// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bodies of the part selection dialog and the part editor (WP-S2.07 part 4a, D19 to D21) inside the 640-px form
//! dialog frame (`crate::dialog`).
//!
//! **Choose a part.** The search field (first focus), a filter row (Only parts that fit, the four views as a segmented
//! control, Show deprecated and withdrawn), the result list and the active row's detail line. Typing edits the search
//! (`part.search`), Up and Down move the active row, Enter chooses it (`part.pick` with its 1-based `index`), Escape
//! cancels. Rows: the favourite star (a button named "Favourite", `part.favourite`), the part number in
//! `type.body.strong`, the manufacturer, the elided description, the source badge and the fit badge. AccessKit: the
//! list is a `ListBox` named "Parts" whose rows are `ListBoxOption`s with position and set size.
//!
//! **Edit part.** One row per editable field, the label in the dialog's label column (named by it for AccessKit), the
//! field's message under it in `color.error`; the technical values as `key` fields; the functions, the connection
//! points and the source read-only. Every change is `part.field`; Enter saves (`part.save`).

use egui::{Key, RichText, ScrollArea, Sense, TextEdit, Ui, vec2};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::part_dialog::{PART_ROWS, PartChooser, PartEditor};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::theme;

fn active_id() -> egui::Id {
    egui::Id::unique("part-choose-active")
}

/// The part dialog's active row (0-based), kept in the window's memory.
pub(crate) fn active(ctx: &egui::Context) -> usize {
    ctx.data(|d| d.get_temp(active_id()).unwrap_or(0))
}

fn set_active(ctx: &egui::Context, i: usize) {
    ctx.data_mut(|d| d.insert_temp(active_id(), i));
}

/// The part selection dialog's body (D19).
pub(crate) fn chooser(
    ui: &mut Ui,
    shell: &ShellModel,
    c: &PartChooser,
    just_opened: bool,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let ctx = ui.ctx().clone();
    if just_opened {
        set_active(&ctx, 0);
    }
    let mut text = c.query.text.clone();
    let search = ui.add(
        TextEdit::singleline(&mut text)
            .hint_text(strings::text("part-choose-search"))
            .desired_width(ui.available_width())
            .min_size(vec2(0.0, t.size_control)),
    );
    search.widget_info(|| {
        egui::WidgetInfo::text_edit(
            true,
            c.query.text.as_str(),
            text.as_str(),
            strings::text("part-choose-search"),
        )
    });
    if just_opened {
        search.request_focus();
    }
    if text != c.query.text {
        set_active(&ctx, 0);
        events.push(Event::Action("part.search", json!({ "text": text })));
    }
    ui.add_space(t.space_2);
    ui.horizontal_wrapped(|ui| {
        let mut fits = c.query.fits_only;
        if ui
            .checkbox(&mut fits, strings::text("part-choose-fits"))
            .changed()
        {
            events.push(Event::Action("part.search", json!({ "fits": fits })));
        }
        ui.add_space(t.space_3);
        for v in ew_viewmodel::ew_api::OfferView::ALL {
            let key = format!("part-choose-view-{}", v.word());
            if ui
                .selectable_label(c.query.view == v, strings::text(&key))
                .clicked()
            {
                set_active(&ctx, 0);
                events.push(Event::Action("part.search", json!({ "view": v.word() })));
            }
        }
        ui.add_space(t.space_3);
        let mut inactive = c.query.include_inactive;
        if ui
            .checkbox(&mut inactive, strings::text("part-choose-inactive"))
            .changed()
        {
            events.push(Event::Action("part.search", json!({ "inactive": inactive })));
        }
    });
    ui.add_space(t.space_2);
    let n = c.offers.len().min(PART_ROWS);
    let mut act = active(&ctx).min(n.saturating_sub(1));
    let (up, down, enter) = ui.input(|i| {
        (
            i.key_pressed(Key::ArrowUp),
            i.key_pressed(Key::ArrowDown),
            i.key_pressed(Key::Enter),
        )
    });
    if down && act + 1 < n {
        act += 1;
    }
    if up && act > 0 {
        act -= 1;
    }
    let list_name = strings::text("part-choose-list");
    let height = t.size_row * 10.0;
    let list_id = ui.scope_id().with("part-choose-list");
    let list_rect = egui::Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), height));
    let list = ui.interact(list_rect, list_id, Sense::hover());
    list.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ListBox, true, list_name));
    let mut picked: Option<usize> = None;
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(list_id), |ui| {
        ScrollArea::vertical()
            .id_salt("part-choose-scroll")
            .max_height(height)
            .min_scrolled_height(height)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for (i, o) in c.offers.iter().take(n).enumerate() {
                    let selected = i == act;
                    ui.horizontal(|ui| {
                        let star = ui.add(
                            egui::Button::new(if o.favourite { "\u{2605}" } else { "\u{2606}" }).frame(false),
                        );
                        star.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::Role::Button,
                                true,
                                strings::text("part-choose-favourite"),
                            )
                        });
                        if star.clicked() {
                            events.push(Event::Action("part.favourite", json!({ "index": i + 1 })));
                        }
                        let mut job = egui::text::LayoutJob::default();
                        let strong = egui::TextFormat {
                            font_id: theme::font(&t.type_body_strong),
                            color: t.text_primary.color32(),
                            ..Default::default()
                        };
                        let plain = egui::TextFormat {
                            font_id: theme::font(&t.type_body),
                            color: t.text_secondary.color32(),
                            ..Default::default()
                        };
                        let small = egui::TextFormat {
                            font_id: theme::font(&t.type_small),
                            color: t.text_tertiary.color32(),
                            ..Default::default()
                        };
                        job.append(&o.part_number, 0.0, strong);
                        job.append(&format!("  {}  ", o.manufacturer), 0.0, plain.clone());
                        let description: String = o.description.chars().take(40).collect();
                        job.append(&description, 0.0, plain);
                        let mut badges = vec![PartChooser::source(o)];
                        badges.extend(c.badge(o));
                        job.append(&format!("  {}", badges.join(" \u{b7} ")), 0.0, small);
                        let row = ui.add(
                            egui::Button::selectable(selected, job).wrap_mode(egui::TextWrapMode::Truncate),
                        );
                        let name = format!("{} ({}) \u{b7} {}", o.part_number, o.manufacturer, o.description);
                        row.widget_info(|| {
                            egui::WidgetInfo::selected(egui::Role::ListBoxOption, true, selected, &name)
                        });
                        ui.ctx().accesskit_node_builder(row.id, |node| {
                            node.set_selected(selected);
                            node.set_position_in_set(i + 1);
                            node.set_size_of_set(n);
                        });
                        if row.clicked() {
                            act = i;
                        }
                        if row.double_clicked() {
                            picked = Some(i);
                        }
                        if selected && (up || down) {
                            row.scroll_to_me(None);
                        }
                    });
                }
            });
    });
    if n == 0 {
        ui.label(
            RichText::new(strings::format("part-choose-empty", &[&c.query.text]))
                .color(t.text_secondary.color32()),
        );
    }
    if let Some(detail) = shell.part_offer_detail(act) {
        ui.add_space(t.space_2);
        ui.label(
            RichText::new(detail)
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
    }
    set_active(&ctx, act);
    if enter && n > 0 {
        picked = Some(act);
    }
    if let Some(i) = picked {
        events.push(Event::Action("part.pick", json!({ "index": i + 1 })));
    }
}

/// One labelled row of the editor: the label column, then the field; `true` when Enter was pressed in it.
fn labelled_field(
    ui: &mut Ui,
    label_text: &str,
    value: &str,
    focus: bool,
    on_change: impl FnOnce(String),
) -> bool {
    let t = theme::current(ui.ctx());
    let mut enter = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let label = ui
            .allocate_ui_with_layout(
                vec2(t.size_dialog_label, t.size_control),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.set_min_size(vec2(t.size_dialog_label, t.size_control));
                    ui.add(egui::Label::new(label_text).truncate())
                },
            )
            .inner;
        ui.add_space(t.space_3);
        let mut text = value.to_owned();
        let edit = ui
            .add(
                TextEdit::singleline(&mut text)
                    .desired_width(ui.available_width())
                    .min_size(vec2(0.0, t.size_control)),
            )
            .labelled_by(label.id);
        if focus {
            edit.request_focus();
        }
        enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if text != value {
            on_change(text);
        }
    });
    enter
}

/// The part editor's body (D20, D21).
pub(crate) fn editor(ui: &mut Ui, e: &PartEditor, just_opened: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let d = &e.draft;
    let mut save = false;
    ScrollArea::vertical()
        .id_salt("part-edit-scroll")
        .max_height(t.size_row * 16.0)
        .show(ui, |ui| {
            for (i, f) in ew_viewmodel::ew_api::DRAFT_FIELDS.iter().enumerate() {
                let label = strings::text(&format!("part-field-{f}"));
                save |= labelled_field(ui, label, d.text(f), just_opened && i == 0, |text| {
                    events.push(Event::Action("part.field", json!({ "field": f, "value": text })));
                });
                if let Some(m) = d.message(f) {
                    ui.colored_label(
                        t.error.color32(),
                        ew_viewmodel::shell::part_dialog::message_text(m.key, &m.args),
                    );
                }
            }
            ui.add_space(t.space_3);
            ui.label(RichText::new(strings::text("part-field-tech")).font(theme::font(&t.type_body_strong)));
            for (k, v) in &d.tech {
                let field = format!("tech:{k}");
                save |= labelled_field(ui, k, v, false, |text| {
                    events.push(Event::Action(
                        "part.field",
                        json!({ "field": field, "value": text }),
                    ));
                });
                if let Some(m) = d.message(&format!("tech:{k}")) {
                    ui.colored_label(
                        t.error.color32(),
                        ew_viewmodel::shell::part_dialog::message_text(m.key, &m.args),
                    );
                }
            }
            ui.add_space(t.space_3);
            let none = strings::text("part-none");
            let read_only = |ui: &mut Ui, label: &str, value: String| {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(t.size_dialog_label, t.size_row), egui::Label::new(label));
                    ui.add_space(t.space_3);
                    ui.label(RichText::new(value).color(t.text_secondary.color32()));
                });
            };
            read_only(
                ui,
                strings::text("part-field-functions"),
                if d.functions.is_empty() {
                    none.to_owned()
                } else {
                    d.functions.join(", ")
                },
            );
            read_only(
                ui,
                strings::text("part-field-cps"),
                if d.cps.is_empty() {
                    none.to_owned()
                } else {
                    d.cps.join(", ")
                },
            );
            let source = match &d.library {
                Some(name) => format!("{name} \u{b7} {}", d.version),
                None => format!(
                    "{} \u{b7} {}",
                    strings::text("part-choose-view-project"),
                    d.version
                ),
            };
            read_only(ui, strings::text("part-field-source"), source);
            if let Some(err) = &e.error {
                ui.colored_label(t.error.color32(), err);
            }
        });
    if save {
        events.push(Event::Action("part.save", serde_json::Value::Null));
    }
}
