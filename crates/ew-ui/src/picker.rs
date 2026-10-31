// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The picker popup (WP-S2.04 §7.1): what can be placed on the focused page, a filter field, one row per symbol
//! and per template function (or, choosing a terminal, one per strip: FT-421), disabled rows with their reasons. Up, Down, Home and End move the row (`picker.row`),
//! Enter chooses (`picker.choose`), typing filters (`picker.filter`), Escape closes it (`ui.dismiss`, the topmost
//! popup). Every change is a registry action, so the headless mode drives the same model.

use egui::{Align2, AtomExt as _, Button, Id, Key, Modifiers, ScrollArea, TextEdit, Window};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{PickerMode, PickerRow};
use serde_json::json;

use crate::app::Event;
use crate::components::Tip as _;

/// Paints the picker when it is open. `just_opened` puts the keyboard focus into its filter field.
pub fn show(ctx: &egui::Context, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    let Some(picker) = shell.picker() else {
        return;
    };
    // The list's keys are consumed before the filter field sees them.
    let (down, up, home, end, enter) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::Home),
            i.consume_key(Modifiers::NONE, Key::End),
            i.consume_key(Modifiers::NONE, Key::Enter),
        )
    });
    for (hit, params) in [
        (down, json!({"step": 1})),
        (up, json!({"step": -1})),
        (home, json!({"to": 0})),
        (end, json!({"to": -1})),
    ] {
        if hit {
            events.push(Event::Action("picker.row", params));
        }
    }
    if enter {
        events.push(Event::Action("picker.choose", json!({})));
    }
    let title = match picker.mode {
        PickerMode::Exchange => strings::text("picker-title-exchange"),
        PickerMode::Place | PickerMode::Parts => strings::text("picker-title"),
        PickerMode::Strip => strings::text("picker-title-strip"),
        PickerMode::BlackBox => strings::text("picker-title-black-box"),
    };
    let t = crate::theme::current(ctx);
    Window::new(title)
        .id(Id::unique("place-picker"))
        .anchor(
            Align2::CENTER_TOP,
            [0.0, t.size_tab_row + t.size_command_row + t.space_4],
        )
        .collapsible(false)
        .resizable(false)
        .default_width(t.size_palette)
        .show(ctx, |ui| {
            let mut filter = picker.filter.clone();
            // Cycle 37 (window check W37): the focus is taken before the field reads this frame's input, so the key
            // typed in the frame that first shows the picker reaches the filter ("relay", not "elay"); it is asked for
            // again once the field is laid out, so it outlives the window's first (sizing) pass.
            let id = ui.make_persistent_id("picker-filter");
            if just_opened {
                ui.memory_mut(|m| m.request_focus(id));
            }
            let edit = ui.add(
                TextEdit::singleline(&mut filter)
                    .id(id)
                    .hint_text(strings::text("picker-filter-hint"))
                    .desired_width(f32::INFINITY),
            );
            if just_opened {
                edit.request_focus();
            }
            if filter != picker.filter {
                events.push(Event::Action("picker.filter", json!({ "text": filter })));
            }
            // FT-605: a terminal chosen in a project without strips opens the strip chooser on an empty state that
            // says why and offers New terminal strip (doc 16a §6.17); `strip.new` lists the new strip as the current
            // row.
            if picker.mode == PickerMode::Strip && picker.empty_text() == Some("picker-empty-strips") {
                let new = crate::components::empty_state(
                    ui,
                    crate::icons::IconId::ew_terminal_strip,
                    strings::text("picker-empty-strips"),
                    strings::text("picker-empty-strips-text"),
                    Some(strings::text("picker-new-strip")),
                );
                if new.is_some_and(|r| r.clicked()) {
                    events.push(Event::Action("strip.new", json!({})));
                }
            } else if let Some(empty) = picker.empty_text() {
                ui.add_space(t.space_3);
                ui.label(egui::RichText::new(strings::text(empty)).color(t.text_secondary.color32()));
            }
            ScrollArea::vertical()
                .max_height(t.size_row * 13.0)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // M39: the part number and the function sit in columns that stay put, as wide as their widest
                    // text over the rows; the description takes the rest and elides (whole in the tooltip).
                    let small = crate::theme::font(&t.type_small);
                    let width = |text: &str| {
                        ui.painter()
                            .layout_no_wrap(text.to_owned(), small.clone(), crate::theme::NONE)
                            .size()
                            .x
                    };
                    let (mut number_w, mut meta_w) = (0.0_f32, 0.0_f32);
                    for row in &picker.rows {
                        let (_, second, meta, _) = row_texts(picker, *row);
                        let meta = picker
                            .disabled(*row)
                            .map_or(meta, |r| strings::text(r).to_owned());
                        number_w = number_w.max(second.as_deref().map_or(0.0, width));
                        meta_w = meta_w.max(width(&meta));
                    }
                    // The function column takes at most a third of the row (a long disabled reason elides there,
                    // whole in the row's tooltip), so the description keeps the most room.
                    let meta_w = meta_w.min(ui.available_width() / 3.0);
                    let small_h = t.type_small.line;
                    for (i, row) in picker.rows.iter().enumerate() {
                        // FT-425: the manufacturer's header above its first part row.
                        if let Some(m) = picker.header_before(i) {
                            ui.add_space(t.space_2);
                            ui.label(
                                egui::RichText::new(m)
                                    .font(crate::theme::font(&crate::theme::TypeToken {
                                        semibold: true,
                                        ..t.type_small
                                    }))
                                    .color(t.text_secondary.color32()),
                            );
                        }
                        let (name, second, meta, tooltip) = row_texts(picker, *row);
                        let reason = picker.disabled(*row);
                        let meta = match reason {
                            Some(r) => strings::text(r).to_owned(),
                            None => meta,
                        };
                        let enabled = reason.is_none();
                        let text_colour = if enabled { t.text_primary } else { t.text_disabled };
                        // Name in body text, the library or function in tertiary (secondary on the selected row).
                        let meta_colour = if i == picker.row {
                            t.text_secondary
                        } else {
                            t.text_tertiary
                        };
                        // FT-712: a part row leads with its description, cut with an ellipsis at the row's width
                        // (whole in the tooltip), and shows its part number second in secondary text.
                        let mut atoms = egui::Atoms::new(
                            egui::RichText::new(&name)
                                .color(text_colour.color32())
                                .atom_shrink(true),
                        );
                        atoms.push_right(egui::Atom::grow());
                        if number_w > 0.0 {
                            atoms.push_right(
                                second_text(&t, second.as_deref().unwrap_or(""), enabled)
                                    .atom_size(egui::vec2(number_w, small_h))
                                    .atom_align(egui::Align2::LEFT_CENTER),
                            );
                        }
                        atoms.push_right(
                            egui::RichText::new(&meta)
                                .font(crate::theme::font(&t.type_small))
                                .color(meta_colour.color32())
                                .atom_size(egui::vec2(meta_w, small_h))
                                .atom_align(egui::Align2::RIGHT_CENTER),
                        );
                        let button = Button::new(atoms)
                            .wrap_mode(egui::TextWrapMode::Truncate)
                            .selected(i == picker.row)
                            .frame_when_inactive(false)
                            .min_size(egui::vec2(ui.available_width(), t.size_row));
                        let response = ui.add_enabled(enabled, button);
                        // A disabled row says its reason in its tooltip too (egui opens `.tip` on enabled widgets
                        // only).
                        let response = if enabled {
                            response.tip(&tooltip)
                        } else {
                            response.disabled_tip(&tooltip)
                        };
                        if response.clicked() {
                            events.push(Event::Action("picker.row", json!({ "to": i })));
                            events.push(Event::Action("picker.choose", json!({})));
                        }
                    }
                });
        });
}

/// A part row's part number (FT-712, FT-759): visibly secondary, `type.small` in `color.text.secondary`
/// (`color.text.disabled` on a disabled row).
fn second_text(t: &crate::theme::Tokens, second: &str, enabled: bool) -> egui::RichText {
    let colour = if enabled {
        t.text_secondary
    } else {
        t.text_disabled
    };
    egui::RichText::new(second)
        .font(crate::theme::font(&t.type_small))
        .color(colour.color32())
}

/// A row's name, the text after it (a part row's part number, FT-712), its library or function line (a part's function
/// in words, FT-664), and its tooltip with the full name and the raw IDs (WP-S2.17 part 4, §8 item 6).
fn row_texts(
    picker: &ew_viewmodel::tools::Picker,
    row: PickerRow,
) -> (String, Option<String>, String, String) {
    match row {
        PickerRow::Symbol(k) => {
            let s = &picker.catalogue.symbols[k];
            let library = if s.copy.is_some() {
                strings::text("picker-badge-embedded")
            } else {
                strings::text("picker-badge-starter")
            };
            // The name is the key's last segment without its grid variant (`ew.iec.mcb-1p@qet10` → `mcb-1p`).
            let name = ew_viewmodel::tools::symbol_name(&s.key).to_owned();
            (
                name,
                None,
                library.to_owned(),
                format!("{}\n{}", s.key, s.fndef_key),
            )
        }
        PickerRow::PartFn(p, k) => {
            let part = &picker.catalogue.parts[p];
            let f = &part.functions[k];
            let mut meta = strings::format(
                "picker-part-function",
                &[
                    &(k + 1).to_string(),
                    &part.functions.len().to_string(),
                    &function_words(&f.kind_text, &f.fndef_key),
                ],
            );
            if f.main {
                meta.push(' ');
                meta.push_str(strings::text("picker-main"));
            }
            // FT-664: the function in words, never an ID; the tooltip adds the FnDef key that scripts name. FT-712:
            // the description leads (the part number when the part has none), the part number follows it, and the
            // tooltip starts with the whole description.
            let (name, second) = if part.description.is_empty() {
                (part.part_number.clone(), None)
            } else {
                (part.description.clone(), Some(part.part_number.clone()))
            };
            let mut tip = String::new();
            if !part.description.is_empty() {
                tip.push_str(&part.description);
                tip.push('\n');
            }
            tip.push_str(&format!(
                "{} {}\n{}\n{}",
                part.manufacturer, part.part_number, f.kind_text, f.fndef_key
            ));
            (name, second, meta, tip)
        }
        PickerRow::Strip(k) => {
            let st = &picker.catalogue.strips[k];
            (
                st.tag.clone(),
                None,
                strings::format("picker-strip-next", &[&st.next_designation]),
                st.tag.clone(),
            )
        }
    }
}

/// A template function in words (M39): its kind text, or, where that is only the FnDef key (a FnDef whose category
/// has fewer than two entries, `ew.fn.blackbox`), the key's last segment as words (`Blackbox`).
fn function_words(kind_text: &str, fndef_key: &str) -> String {
    if kind_text != fndef_key {
        return kind_text.to_owned();
    }
    let word = ew_viewmodel::tools::symbol_name(fndef_key).replace(['-', '_'], " ");
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => kind_text.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use crate::theme::{Density, Theme, font, tokens};

    /// M39: a function whose kind text is its FnDef key reads as words, never as the key; a real kind text stays.
    #[test]
    fn function_keys_read_as_words() {
        assert_eq!(
            super::function_words("ew.fn.blackbox", "ew.fn.blackbox"),
            "Blackbox"
        );
        assert_eq!(
            super::function_words("ew.fn.dp-connector", "ew.fn.dp-connector"),
            "Dp connector"
        );
        assert_eq!(
            super::function_words("NO contact 13-14", "ew.fn.contact-no"),
            "NO contact 13-14"
        );
    }

    /// FT-759: the part number after a part row's description is `type.small` in `color.text.secondary`.
    #[test]
    fn the_part_number_is_small_and_secondary() {
        let t = tokens(Theme::Light, Density::Comfortable);
        let ctx = egui::Context::default();
        let style = ctx.global_style();
        let job = egui::WidgetText::from(super::second_text(&t, "3RT2016-1AP01", true)).into_layout_job(
            &style,
            egui::FontSelection::Default,
            egui::Align::Min,
        );
        let fmt = &job.sections[0].format;
        assert_eq!(fmt.font_id, font(&t.type_small));
        assert_eq!(fmt.color, t.text_secondary.color32());
        assert!(t.type_small.size < t.type_body.size);
    }
}
