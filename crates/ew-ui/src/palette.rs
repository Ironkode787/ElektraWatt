// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command palette (WP-S2.01 part 1, spec section 5.3; FR-UI-012), restyled by WP-S2.17 part 4 (doc 16a §6.14);
//! its behaviour is WP-S2.08 part 1's: Ctrl+K opens a popover with a search field; the shell ranks the entries
//! (`ew_viewmodel::palette`: fuzzy score, glossary aliases, recents, the prefixes `>`, `@`, `#`, `:` and `?`);
//! disabled entries follow the enabled ones with their reason; Enter runs the selected enabled entry or opens its
//! parameter form (one row per parameter, Enter runs it, Escape returns to the list); Escape closes the palette.
//!
//! **Look.** No title: `size.palette` wide (560 px), anchored under the command bar's search field (or centred at the
//! window's top fifth when opened by key), `color.surface.raised`, `elevation.2`, `radius.lg`; a 36-px search field
//! with a search icon; result rows of `size.palette_row` (32 px): the action's icon, the label with the matched
//! characters in `type.body.strong`, the breadcrumb in `color.text.tertiary` ("Draw · Place"), the key as key chips at
//! the right; the selected row on the hover overlay; a disabled row in `color.text.disabled` with its reason in
//! `color.text.secondary`.
//!
//! **Accessibility.** Each row is a button named by its label (and its reason when disabled), carrying its shortcut.

use egui::text::LayoutJob;
use egui::{Align, Align2, Id, Key, Layout, Modifiers, Rect, ScrollArea, Sense, TextEdit, Vec2};
use ew_viewmodel::actions::{PaletteEntry, PaletteRun};
use ew_viewmodel::palette::{self, PaletteForm};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;

use crate::app::Event;
use crate::icons::{self, IconId};
use crate::theme::{self, Tokens};

/// The label of `entry` with the characters the query's best alignment matches (WP-S2.08 D2) in `type.body.strong`;
/// `query` is the text after the prefix.
fn label_job(t: &Tokens, label: &str, query: &str, colour: egui::Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let normal = egui::TextFormat::simple(theme::font(&t.type_body), colour);
    let strong = egui::TextFormat::simple(theme::font(&t.type_body_strong), colour);
    let hits = palette::matched(query, label);
    let mut run = String::new();
    let mut run_strong = false;
    for (i, c) in label.chars().enumerate() {
        let is_strong = hits.binary_search(&i).is_ok();
        if is_strong != run_strong && !run.is_empty() {
            let format = if run_strong {
                strong.clone()
            } else {
                normal.clone()
            };
            job.append(&std::mem::take(&mut run), 0.0, format);
        }
        run_strong = is_strong;
        run.push(c);
    }
    if !run.is_empty() {
        job.append(&run, 0.0, if run_strong { strong } else { normal });
    }
    job
}

/// One result row; returns whether it was clicked.
fn row(ui: &mut egui::Ui, t: &Tokens, entry: &PaletteEntry, query: &str, selected: bool) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, t.size_palette_row), Sense::click());
    let name = match &entry.reason {
        Some(reason) if !entry.enabled => format!("{}. {reason}", entry.label),
        _ => entry.label.clone(),
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, entry.enabled, &name));
    if !entry.shortcut.is_empty() {
        let shortcut = crate::keys::display(&entry.shortcut);
        ui.ctx()
            .accesskit_node_builder(response.id, |n| n.set_keyboard_shortcut(shortcut));
    }
    let painter = ui.painter();
    if selected || (entry.enabled && response.hovered()) {
        painter.rect_filled(rect, theme::radius(t.radius_sm), t.state_hover.color32());
    }
    let (text, icon_colour) = if entry.enabled {
        (t.text_primary, t.text_secondary)
    } else {
        (t.text_disabled, t.text_disabled)
    };
    let mut x = rect.left() + t.space_3;
    let icon = match &entry.run {
        PaletteRun::Action(id) => ew_viewmodel::actions::find(id)
            .and_then(|m| m.icon)
            .and_then(IconId::from_key),
        _ => None,
    };
    if let Some(id) = icon {
        let r = Rect::from_center_size(
            egui::pos2(x + t.icon_sm / 2.0, rect.center().y),
            Vec2::splat(t.icon_sm),
        );
        icons::paint(painter, id, r, icon_colour.color32(), t.icon_stroke);
    }
    x += t.icon_sm + t.space_3;
    let galley = painter.layout_job(label_job(t, &entry.label, query, text.color32()));
    let label_w = galley.size().x;
    painter.galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        text.color32(),
    );
    x += label_w + t.space_3;
    // The key chips first, at the right; the tail (the reason or the breadcrumb) takes the room up to them and elides
    // with an ellipsis, its whole text in the row's tooltip (M39: a disabled reason was clipped with no ellipsis).
    let mut tail_right = rect.right() - t.space_3;
    if !entry.shortcut.is_empty() {
        let chip_area = Rect::from_min_max(
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.right() - t.space_3, rect.bottom()),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(chip_area)
                .layout(Layout::right_to_left(Align::Center)),
        );
        crate::components::key_chip(&mut child, &entry.shortcut);
        tail_right = child.min_rect().left() - t.space_3;
    }
    let (tail, tail_colour) = match &entry.reason {
        Some(reason) if !entry.enabled => (reason.clone(), t.text_secondary),
        _ => (entry.breadcrumb.clone(), t.text_tertiary),
    };
    let mut job = LayoutJob::single_section(
        tail.clone(),
        egui::TextFormat::simple(theme::font(&t.type_small), tail_colour.color32()),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width((tail_right - x).max(0.0));
    let galley = ui.painter().layout_job(job);
    let elided = galley.elided;
    ui.painter().galley(
        egui::pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        tail_colour.color32(),
    );
    if elided {
        response.clone().on_hover_text(tail);
    }
    response.clicked() && entry.enabled
}

/// Paints the palette when it is open. `just_opened` puts the keyboard focus into its search field. `anchor` is the
/// command bar's search field (WP-S2.17 part 2, doc 16a §5.2): the palette opens under it, right-aligned with it.
pub fn show(
    ctx: &egui::Context,
    shell: &ShellModel,
    just_opened: bool,
    anchor: Option<Rect>,
    events: &mut Vec<Event>,
) {
    let Some(state) = shell.palette() else {
        return;
    };
    // The keys of the list are consumed before the text field sees them.
    let (down, up, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    if down {
        events.push(Event::PaletteMove(1));
    }
    if up {
        events.push(Event::PaletteMove(-1));
    }
    if enter {
        events.push(Event::PaletteRun);
    }
    let t = theme::current(ctx);
    let form = state.form.as_ref();
    let entries = if form.is_some() {
        Vec::new()
    } else {
        shell.palette_entries()
    };
    let (_, rest) = palette::split_prefix(&state.query);
    // The form's first row takes the focus once, when the form opens.
    let form_key = Id::unique("command-palette-form");
    let form_opened = ctx.data_mut(|d| {
        let was: Option<&'static str> = d.get_temp(form_key).flatten();
        let now = form.map(|f| f.action);
        d.insert_temp(form_key, now);
        now.is_some() && was != now
    });
    let area = egui::Area::new(Id::unique("command-palette")).order(egui::Order::Foreground);
    let area = match anchor {
        Some(field) => area
            .pivot(Align2::RIGHT_TOP)
            .fixed_pos(field.right_bottom() + Vec2::new(0.0, t.space_1)),
        None => area.pivot(Align2::CENTER_TOP).fixed_pos(egui::pos2(
            ctx.content_rect().center().x,
            ctx.content_rect().height() / 5.0,
        )),
    };
    area.show(ctx, |ui| {
        egui::Frame::new()
            .fill(t.surface_raised.color32())
            .stroke(theme::stroke(t.border_raised_width, t.border_default))
            .corner_radius(theme::radius(t.radius_lg))
            .shadow(theme::shadow(&t.elevation_2))
            .inner_margin(egui::Margin::same(t.space_2 as i8))
            .show(ui, |ui| {
                ui.set_width(t.size_palette - 2.0 * t.space_2);
                ui.spacing_mut().item_spacing.y = t.space_1;
                // The 36-px search field with its icon.
                let field_h = t.size_control + t.space_3;
                ui.horizontal(|ui| {
                    ui.set_min_height(field_h);
                    ui.add_space(t.space_2);
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(t.icon_sm), Sense::hover());
                    icons::paint(
                        ui.painter(),
                        IconId::search,
                        r,
                        t.text_secondary.color32(),
                        t.icon_stroke,
                    );
                    let mut query = state.query.clone();
                    let edit = ui.add(
                        TextEdit::singleline(&mut query)
                            .hint_text(strings::text("palette-hint"))
                            .frame(egui::Frame::NONE)
                            .font(theme::font(&t.type_body))
                            .desired_width(f32::INFINITY),
                    );
                    if just_opened && form.is_none() {
                        edit.request_focus();
                    }
                    if query != state.query {
                        events.push(Event::PaletteQuery(query));
                    }
                });
                let (line, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), t.border_width), Sense::hover());
                ui.painter()
                    .rect_filled(line, theme::SQUARE, t.border_subtle.color32());
                if let Some(form) = form {
                    form_rows(ui, &t, form, form_opened, events);
                } else if entries.is_empty() {
                    ui.add_space(t.space_3);
                    ui.horizontal(|ui| {
                        ui.add_space(t.space_3);
                        ui.label(
                            egui::RichText::new(shell.palette_empty_text()).color(t.text_secondary.color32()),
                        );
                    });
                    ui.add_space(t.space_3);
                }
                // The list asks for its own rows' height (up to ten): an egui area lays out in the size it had the
                // frame before, so without it a list that grows (a reopened palette, a shorter query) stayed as short
                // as the previous one (WP-S2.08 part 1's window check).
                let rows = entries.len().min(10) as f32;
                ScrollArea::vertical()
                    .max_height(t.size_palette_row * 10.0)
                    .min_scrolled_height(t.size_palette_row * rows)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, entry) in entries.iter().enumerate() {
                            if row(ui, &t, entry, rest, i == state.selected) {
                                events.push(Event::PaletteClick(i));
                            }
                        }
                    });
            });
    });
}

/// The rows of an open parameter form (WP-S2.08 D5 (a)): the action's label, then per parameter its label and a
/// field (a check box for a `bool`, a list for an enumeration, a text field otherwise), then the last error. Enter
/// runs the form (the palette's Enter); Escape returns to the list (`ui.dismiss`).
fn form_rows(ui: &mut egui::Ui, t: &Tokens, form: &PaletteForm, opened: bool, events: &mut Vec<Event>) {
    let title = ew_viewmodel::actions::find(form.action).map_or(form.action, |m| strings::text(m.label_key));
    ui.add_space(t.space_2);
    ui.horizontal(|ui| {
        ui.add_space(t.space_3);
        ui.label(
            egui::RichText::new(title)
                .font(theme::font(&t.type_body_strong))
                .color(t.text_primary.color32()),
        );
    });
    let mut first = opened;
    for (i, f) in form.fields.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.set_min_height(t.size_palette_row);
            ui.add_space(t.space_3);
            ui.spacing_mut().item_spacing.x = 0.0;
            let label = ui
                .allocate_ui_with_layout(
                    Vec2::new(t.size_dialog_label, t.size_control),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_size(Vec2::new(t.size_dialog_label, t.size_control));
                        ui.add(egui::Label::new(f.label()).truncate())
                    },
                )
                .inner;
            ui.add_space(t.space_3);
            let choices = f.choices();
            let response = if f.ty == "bool" {
                let mut on = f.value == "true";
                let r = ui
                    .add(egui::Checkbox::without_text(&mut on))
                    .labelled_by(label.id);
                if r.changed() {
                    events.push(Event::PaletteField(i, on.to_string()));
                }
                r
            } else if choices.is_empty() {
                let mut text = f.value.clone();
                let r = ui
                    .add(
                        TextEdit::singleline(&mut text)
                            .desired_width(ui.available_width() - t.space_3)
                            .min_size(Vec2::new(0.0, t.size_control)),
                    )
                    .labelled_by(label.id);
                if text != f.value {
                    events.push(Event::PaletteField(i, text));
                }
                r
            } else {
                let combo = egui::ComboBox::from_id_salt(("palette-form-choice", i))
                    .selected_text(strings::choice_text(&f.value))
                    .width(ui.available_width() - t.space_3)
                    .icon(crate::components::combo_chevron)
                    .show_ui(ui, |ui| {
                        for choice in &choices {
                            if ui
                                .selectable_label(f.value == *choice, strings::choice_text(choice))
                                .clicked()
                            {
                                events.push(Event::PaletteField(i, (*choice).to_owned()));
                            }
                        }
                    });
                combo.response.labelled_by(label.id)
            };
            if first {
                response.request_focus();
                first = false;
            }
        });
    }
    if let Some(error) = &form.error {
        ui.horizontal(|ui| {
            ui.add_space(t.space_3);
            crate::components::live_line(ui, error, true);
        });
    }
    // M39: the form says how it applies and how it goes back, in the hint style (`type.small`, tertiary).
    ui.horizontal(|ui| {
        ui.add_space(t.space_3);
        ui.label(
            egui::RichText::new(strings::text("palette-form-hint"))
                .font(theme::font(&t.type_small))
                .color(t.text_tertiary.color32()),
        );
    });
    ui.add_space(t.space_2);
}
