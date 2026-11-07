// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The picker popup (WP-S2.04 §7.1): what can be placed on the focused page, a filter field, one row per symbol
//! and per template function (or, choosing a terminal, one per strip: FT-421), disabled rows with their reasons. Up, Down, Home and End move the row (`picker.row`),
//! Enter chooses (`picker.choose`), typing filters (`picker.filter`), Escape closes it (`ui.dismiss`, the topmost
//! popup). Every change is a registry action, so the headless mode drives the same model.

use egui::{Align2, AtomExt as _, Button, Id, Key, Modifiers, ScrollArea, TextEdit, Window};
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::{PickerMode, PickerRow};
use serde_json::json;

use crate::app::Event;
use crate::components::Tip as _;

/// Paints the picker when it is open. `just_opened` puts the keyboard focus into its filter field.
pub fn show(ctx: &egui::Context, shell: &ShellModel, just_opened: bool, events: &mut Vec<Event>) {
    let Some(picker) = shell.picker() else {
        // ID-1439: the next picker opens at its own width.
        ctx.data_mut(|d| d.remove::<f32>(picker_width_id()));
        return;
    };
    // The list's keys are consumed before the filter field sees them; not while a dialog over the picker has them
    // (FT-1090: Enter in the New terminal strip form creates the strip).
    let (down, up, home, end, enter) = if shell.dialog().is_some() {
        (false, false, false, false, false)
    } else {
        ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Home),
                i.consume_key(Modifiers::NONE, Key::End),
                i.consume_key(Modifiers::NONE, Key::Enter),
            )
        })
    };
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
    // FT-1375: the symbols for a part without a function template name the part.
    let part_title = picker
        .part
        .and_then(|p| picker.catalogue.parts.get(p))
        .map(|p| strings::format(key!("picker-title-part-symbol"), &[&p.part_number]));
    let title = match picker.mode {
        _ if part_title.is_some() => part_title.as_deref().unwrap_or_default(),
        PickerMode::Exchange => strings::text(key!("picker-title-exchange")),
        PickerMode::Place => strings::text(key!("picker-title")),
        // FT-890: the Device button (`tool.place_part`) picks a part, so its dialog names a device.
        PickerMode::Parts => strings::text(key!("picker-title-device")),
        PickerMode::Strip => strings::text(key!("picker-title-strip")),
        PickerMode::BlackBox => strings::text(key!("picker-title-black-box")),
    };
    let t = crate::theme::current(ctx);
    // ID-1427 (W52 F2): the table uses the space it has. The picker's content is as wide as its widest name, its part
    // number column and its function column need, at least its grown width; the window then keeps `space.6` free on
    // each side. ID-1439 (C52-p8#3): measured over the step's rows without the filter, and never narrower than the
    // width it opened at, so typing does not move the field.
    let spacing = ctx.global_style().spacing.clone();
    let margin = spacing.window_margin.leftf() + spacing.window_margin.rightf();
    let need = {
        let measure = |text: &str, font: &crate::theme::TypeToken| {
            ctx.fonts_mut(|f| {
                f.layout_no_wrap(text.to_owned(), crate::theme::font(font), crate::theme::NONE)
                    .size()
                    .x
            })
        };
        let mut all = picker.clone();
        all.filter.clear();
        all.refilter();
        let (mut name_w, mut number_w, mut meta_w) = (0.0_f32, 0.0_f32, 0.0_f32);
        for row in &all.rows {
            let (name, second, meta, _) = row_texts(&all, *row);
            let meta = column_reason(&all, *row).unwrap_or(meta);
            name_w = name_w.max(measure(&name, &t.type_body));
            number_w = number_w.max(second.as_deref().map_or(0.0, |n| measure(n, &t.type_small)));
            meta_w = meta_w.max(measure(&meta, &t.type_small));
        }
        // The row's button padding and its gaps between the columns, the scroll bar.
        name_w
            + number_w
            + meta_w
            + 2.0 * spacing.button_padding.x
            + 3.0 * spacing.item_spacing.x
            + spacing.scroll.bar_width
            + spacing.scroll.bar_outer_margin
    };
    let window = ctx.content_rect().width();
    let most = (window - 2.0 * t.space_6 - margin).max(0.0);
    let opened = ctx.data(|d| d.get_temp::<f32>(picker_width_id())).unwrap_or(0.0);
    let width = crate::dialog::grown(t.size_palette, crate::palette::PALETTE_SHARE, window)
        .max(need.min(most))
        .max(opened.min(most));
    ctx.data_mut(|d| d.insert_temp(picker_width_id(), width));
    Window::new(title)
        .id(Id::unique("place-picker"))
        .anchor(
            Align2::CENTER_TOP,
            [0.0, t.size_tab_row + t.size_command_row + t.space_4],
        )
        .collapsible(false)
        .resizable(false)
        // C39 (doc 16a §2): the picker grows with the window as the palette does; the description takes the gain.
        .default_width(width)
        .min_width(width)
        .max_width(width)
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
                    .hint_text(strings::text(key!("picker-filter-hint")))
                    .desired_width(f32::INFINITY),
            );
            if just_opened {
                edit.request_focus();
            }
            if filter != picker.filter {
                events.push(Event::Action("picker.filter", json!({ "text": filter })));
            }
            // FT-605: a terminal chosen in a project without strips opens the strip chooser on an empty state that
            // says why and offers New terminal strip (doc 16a §6.17), whose form offers the tag (FT-1090, ID-1181:
            // `strip.new {dialog}`); its Create lists the new strip as the current row.
            if picker.mode == PickerMode::Strip && picker.empty_text() == Some(key!("picker-empty-strips")) {
                let new = crate::components::empty_state(
                    ui,
                    crate::icons::IconId::ew_terminal_strip,
                    strings::text(key!("picker-empty-strips")),
                    strings::text(key!("picker-empty-strips-text")),
                    Some(strings::text(key!("picker-new-strip"))),
                );
                if new.is_some_and(|r| r.clicked()) {
                    events.push(Event::Action("strip.new", json!({"dialog": true})));
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
                        let meta = column_reason(picker, *row).unwrap_or(meta);
                        number_w = number_w.max(second.as_deref().map_or(0.0, width));
                        meta_w = meta_w.max(width(&meta));
                    }
                    // The function column takes at most a third of the row (a long disabled reason elides there,
                    // whole in the row's tooltip), so the description keeps the most room.
                    let meta_w = meta_w.min(ui.available_width() / 3.0);
                    let small_h = t.type_small.line;
                    for (i, row) in picker.rows.iter().enumerate() {
                        // FT-1513 (ID-1456): the group's heading above its first part row, "In this project" or
                        // "From libraries", in `type.body.strong` over the manufacturers' headers.
                        if let Some(g) = picker.group_before(i) {
                            ui.add_space(t.space_2);
                            ui.label(
                                egui::RichText::new(strings::text(g))
                                    .font(crate::theme::font(&t.type_body_strong))
                                    .color(t.text_primary.color32()),
                            );
                        }
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
                        let reason = picker.disabled_words(*row);
                        let enabled = reason.is_none();
                        let meta = column_reason(picker, *row).unwrap_or(meta);
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
                        // FT-1143 (ID-1225): the function column cuts its text with an ellipsis at its width (the
                        // name is the row's one shrinking atom), whole in the row's tooltip.
                        atoms.push_right(
                            egui::RichText::new(&meta)
                                .font(crate::theme::font(&t.type_small))
                                .color(meta_colour.color32())
                                .atom_size(egui::vec2(meta_w, small_h))
                                .atom_max_width(meta_w)
                                .atom_align(egui::Align2::RIGHT_CENTER),
                        );
                        let tooltip = if !matches!(row, PickerRow::Strip(_)) {
                            format!("{tooltip}\n{}", reason.as_deref().unwrap_or(&meta))
                        } else {
                            tooltip
                        };
                        let button = Button::new(atoms)
                            .wrap_mode(egui::TextWrapMode::Truncate)
                            .selected(i == picker.row)
                            .frame_when_inactive(false)
                            .min_size(egui::vec2(ui.available_width(), t.size_row));
                        // ID-1477 (1): a symbol on another grid stays greyed but takes a click, which makes it the
                        // current row: its whole reason and Place on a new page with its grid show under the list.
                        let other_grid = matches!(row, PickerRow::Symbol(k)
                            if picker.catalogue.symbols[*k].grid_mismatch.is_some());
                        let response = ui.add_enabled(enabled || other_grid, button);
                        // FT-1255: egui maps `selected` to `toggled` only; AccessKit's selected state.
                        ui.ctx()
                            .accesskit_node_builder(response.id, |n| n.set_selected(i == picker.row));
                        // A disabled row says its reason in its tooltip too (egui opens `.tip` on enabled widgets
                        // only).
                        let response = if enabled || other_grid {
                            response.tip(&tooltip)
                        } else {
                            response.disabled_tip(&tooltip)
                        };
                        if response.clicked() {
                            events.push(Event::Action("picker.row", json!({ "to": i })));
                            if enabled {
                                events.push(Event::Action("picker.choose", json!({})));
                            }
                        }
                    }
                });
            // ID-1477 (1): the current row on another connection grid: its reason whole, never cut, and the way out.
            if let Some(row @ PickerRow::Symbol(k)) = picker.rows.get(picker.row).copied()
                && picker.catalogue.symbols[k].grid_mismatch.is_some()
            {
                ui.add_space(t.space_2);
                ui.label(
                    egui::RichText::new(picker.disabled_words(row).unwrap_or_default())
                        .color(t.text_secondary.color32()),
                );
                let new_page = strings::text(key!("action-picker-new-page"));
                if crate::components::button(ui, crate::components::Kind::Secondary, new_page, true, 0.0).clicked() {
                    events.push(Event::Action("picker.new_page", json!({})));
                }
            }
        });
}

/// The reason a row's function column shows for a disabled row: none for a symbol on another connection grid, whose
/// reason shows whole under the list when it is the current row (ID-1477 (1): never cut).
fn column_reason(picker: &ew_viewmodel::tools::Picker, row: PickerRow) -> Option<String> {
    match row {
        PickerRow::Symbol(k) if picker.catalogue.symbols[k].grid_mismatch.is_some() => None,
        _ => picker.disabled_words(row),
    }
}

/// The key of the open picker's width (ID-1439): it never narrows while the picker is open.
fn picker_width_id() -> Id {
    Id::unique("place-picker-width")
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
            // ID-1477 (9): a symbol of an installed or user library is never called "starter".
            let library = if s.copy.is_some() {
                strings::text(key!("picker-badge-embedded"))
            } else if s.key.starts_with("library:") {
                strings::text(key!("picker-badge-library"))
            } else {
                strings::text(key!("picker-badge-starter"))
            };
            // ID-1427 (W52 F16): the row names the symbol by its name in words ("Contact, changeover"), in both
            // languages; a symbol without a name by the key's last segment without its grid variant
            // (`ew.iec.mcb-1p@qet10` → `mcb-1p`). FT-1196: the tooltip likewise, never the keys.
            let name = if s.name.is_empty() {
                ew_viewmodel::tools::symbol_name(&s.key).to_owned()
            } else {
                s.name.clone()
            };
            (name.clone(), None, library.to_owned(), name)
        }
        PickerRow::PartFn(p, k) => {
            let part = &picker.catalogue.parts[p];
            let f = &part.functions[k];
            let mut meta = strings::format(
                key!("picker-part-function"),
                &[
                    &(k + 1).to_string(),
                    &part.functions.len().to_string(),
                    &function_words(&f.kind_text, &f.fndef_key),
                ],
            );
            if f.main {
                meta.push(' ');
                meta.push_str(strings::text(key!("picker-main")));
            }
            // FT-664: the function in words, never an ID (FT-1514: in the tooltip too). FT-712:
            // the description leads (the part number when the part has none), the part number follows it, and the
            // tooltip starts with the whole description.
            // FT-1209 (ID-1392): the part number and the rating tell two parts of one kind apart ("Fuse holder with
            // fuse link", "EW-GEN-FUS-1 · 4 A").
            let number = if part.rating.is_empty() {
                part.part_number.clone()
            } else {
                format!("{} \u{b7} {}", part.part_number, part.rating)
            };
            let (name, second) = if part.description.is_empty() {
                (number, None)
            } else {
                (part.description.clone(), Some(number))
            };
            let mut tip = String::new();
            if !part.description.is_empty() {
                tip.push_str(&part.description);
                tip.push('\n');
            }
            // FT-1514 (ID-1415): no FnDef key in a user text; the function's words end the tooltip (the meta line
            // the window adds, which holds the kind text in words).
            tip.push_str(&format!("{} {}", part.manufacturer, part.part_number));
            (name, second, meta, tip)
        }
        // FT-1375: a held part without a function template; its symbol is chosen next.
        PickerRow::Part(p) => {
            let part = &picker.catalogue.parts[p];
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
            tip.push_str(&format!("{} {}", part.manufacturer, part.part_number));
            (
                name,
                second,
                strings::text(key!("picker-part-symbol")).to_owned(),
                tip,
            )
        }
        PickerRow::Strip(k) => {
            let st = &picker.catalogue.strips[k];
            // ID-1278 ruling 7: a strip without a tag yet in words.
            let label = ew_viewmodel::navigators::strip_label(&st.tag);
            (
                label.clone(),
                None,
                strings::format(key!("picker-strip-next"), &[&st.next_designation]),
                label,
            )
        }
    }
}

/// A template function in words (M39): its kind text, or, where that is only the FnDef key (a FnDef whose category
/// has fewer than two entries, `ew.fn.blackbox`), the key's last segment as words (`Blackbox`, through
/// `strings::choice_words`).
fn function_words(kind_text: &str, fndef_key: &str) -> String {
    if kind_text != fndef_key {
        return kind_text.to_owned();
    }
    // The last fallback: the key's last segment through the shell's choice words.
    strings::choice_words(&ew_viewmodel::tools::symbol_name(fndef_key).replace('-', "_"))
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

    /// FT-1196: a symbol row's tooltip is in words, its name, never its keys.
    #[test]
    fn a_symbol_rows_tooltip_is_in_words() {
        use ew_viewmodel::ew_api::output::{CatalogueSymbol, PlaceCatalogue};
        use ew_viewmodel::tools::{Picker, PickerMode, PickerRow};
        let symbol = CatalogueSymbol {
            key: "ew.iec.mcb-1p@qet10".to_owned(),
            copy: None,
            fndef_key: "ew.fn.mcb-1p".to_owned(),
            rep_type: "panel_front".to_owned(),
            letter: None,
            black_box: false,
            terminal: false,
            allowed: false,
            grid_mismatch: None,
            words: Vec::new(),
            name: "Miniature circuit breaker, 1-pole".to_owned(),
        };
        let catalogue = PlaceCatalogue {
            symbols: vec![symbol],
            ..PlaceCatalogue::default()
        };
        let picker = Picker::new(PickerMode::Exchange, "101", std::sync::Arc::new(catalogue));
        let (_, _, _, tooltip) = super::row_texts(&picker, PickerRow::Symbol(0));
        assert_eq!(tooltip, "Miniature circuit breaker, 1-pole");
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
