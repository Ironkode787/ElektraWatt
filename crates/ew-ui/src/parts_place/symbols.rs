// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbols in the place (doc 07 §5.3, §5.6): the Symbols shelf of the list, and the pieces table of On the
//! schematic with each piece's Symbol cell.

use super::*;

/// The Symbols shelf (doc 07 §5.3): the symbols by name, those he drew lately first under Lately drawn, each with
/// what it is and where it lives; a click opens it in the drawing.
pub(super) fn symbol_shelf(ui: &mut Ui, place: &PartsPlace, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    ui.label(components::small_secondary(
        &t,
        strings::format_count(count_key!("parts-symbols-count"), place.symbols.len()),
    ));
    ui.add_space(t.space_2);
    ScrollArea::vertical()
        .id_salt("parts-symbols")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let (recent, rest): (Vec<_>, Vec<_>) =
                place.symbols.iter().take(LIST_ROWS).partition(|s| s.recent);
            for (word, rows) in [
                (strings::text(key!("parts-group-drawn-lately")), recent),
                (strings::text(key!("parts-group-symbols")), rest),
            ] {
                if rows.is_empty() {
                    continue;
                }
                let _ = list_group(ui, word, rows.len(), false);
                for s in rows {
                    if symbol_row(ui, s).clicked() {
                        events.push(Event::Action("parts.open", json!({ "symbol": s.address })));
                    }
                }
            }
        });
}

/// A row of the Symbols shelf: the symbol's name and what it is, where it lives at the right.
pub(super) fn symbol_row(ui: &mut Ui, s: &ew_viewmodel::shell::parts_place::SymbolFound) -> egui::Response {
    let t = theme::current(ui.ctx());
    let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 2.0 * t.size_row), Sense::click());
    let painter = ui.painter_at(rect);
    if r.hovered() {
        painter.rect_filled(rect, theme::SQUARE, t.state_hover.color32());
    }
    let left = rect.left() + t.space_3;
    painter.text(
        pos2(left, rect.top() + t.space_2),
        egui::Align2::LEFT_TOP,
        &s.name,
        theme::font(&t.type_body_strong),
        t.text_primary.color32(),
    );
    painter.text(
        pos2(left, rect.center().y + t.space_1),
        egui::Align2::LEFT_TOP,
        &s.what,
        theme::font(&t.type_small),
        t.text_secondary.color32(),
    );
    painter.text(
        pos2(rect.right() - t.space_3, rect.top() + t.space_2),
        egui::Align2::RIGHT_TOP,
        &s.lives,
        theme::font(&t.type_small),
        t.text_secondary.color32(),
    );
    let name = format!("{}, {}", s.name, s.what);
    r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// On the schematic of a part drawn with symbols (doc 07 §5.4, picture A): its pieces as a table that reads, What it is,
/// Pins and the Symbol that draws it (the starter's for that function), and Add a piece. The pieces make the part's
/// pins and, on Save, its template.
pub(super) fn pieces_section(ui: &mut Ui, open: &OpenSheet, editable: bool, events: &mut Vec<Event>) {
    use ew_viewmodel::ew_api::part_pieces::piece_kinds;
    let d = &open.sheet;
    let t = theme::current(ui.ctx());
    let kinds: Vec<(String, String)> = piece_kinds()
        .into_iter()
        .map(|k| (k.key.to_owned(), strings::render(&k.words)))
        .collect();
    let words = |key: &str| {
        kinds.iter().find(|(k, _)| k == key).map_or_else(
            || strings::text(key!("parts-piece-choose")).to_owned(),
            |(_, w)| w.clone(),
        )
    };
    if d.pieces.is_empty() {
        ui.label(RichText::new(strings::text(key!("parts-pieces-none"))).color(t.text_secondary.color32()));
    } else {
        egui::Grid::new("part-pieces")
            .num_columns(4)
            .spacing(vec2(t.space_4, t.space_2))
            .striped(false)
            .show(ui, |ui| {
                table_header(ui, strings::text(key!("parts-piece-what")));
                table_header(ui, strings::text(key!("parts-piece-pins")));
                table_header(ui, strings::text(key!("parts-piece-symbol")));
                ui.label("");
                ui.end_row();
                for (i, row) in d.pieces.iter().enumerate() {
                    let n = (i + 1).to_string();
                    if editable {
                        let mut picked = None;
                        let name = strings::format(key!("parts-piece-what-of"), &[&n]);
                        // A long list: what is typed at its top narrows it, and Enter takes the first left.
                        let typed_id = ui.make_persistent_id(("part-piece-what-typed", i));
                        let shown = components::choice(
                            ui,
                            egui::ComboBox::from_id_salt(("part-piece-what", i))
                                .selected_text(words(&row.what))
                                .width(t.size_control * 7.0),
                            |ui| {
                                let mut typed: String =
                                    ui.memory(|m| m.data.get_temp(typed_id)).unwrap_or_default();
                                let edit = ui.add(
                                    egui::TextEdit::singleline(&mut typed)
                                        .hint_text(strings::text(key!("parts-piece-what-type"))),
                                );
                                if !edit.has_focus() && typed.is_empty() {
                                    edit.request_focus();
                                }
                                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let needle = typed.to_lowercase();
                                ui.memory_mut(|m| m.data.insert_temp(typed_id, typed));
                                let mut first = None;
                                for (k, w) in kinds.iter().filter(|(_, w)| w.to_lowercase().contains(&needle)) {
                                    first.get_or_insert_with(|| k.clone());
                                    if ui.selectable_label(*k == row.what, w).clicked() {
                                        picked = Some(k.clone());
                                    }
                                }
                                if enter && picked.is_none() {
                                    picked = first;
                                    ui.close();
                                }
                            },
                        );
                        if !egui::ComboBox::is_open(ui.ctx(), shown.response.id) {
                            ui.memory_mut(|m| m.data.remove::<String>(typed_id));
                        }
                        shown
                            .response
                            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
                        if let Some(k) = picked {
                            events.push(Event::Action(
                                "parts.field",
                                json!({ "field": format!("pieces[{i}].what"), "value": k }),
                            ));
                        }
                        let name = strings::format(key!("parts-piece-pins-of"), &[&n]);
                        if let Some(text) = table_text(ui, &row.pins, &name, t.size_control * 4.0) {
                            events.push(Event::Action(
                                "parts.field",
                                json!({ "field": format!("pieces[{i}].pins"), "value": text }),
                            ));
                        }
                    } else {
                        ui.label(words(&row.what));
                        ui.label(&row.pins);
                    }
                    piece_symbol_cell(ui, open, i, editable, events);
                    if editable {
                        let remove = strings::format(key!("part-table-remove"), &[&n]);
                        if ui.small_button("\u{d7}").on_hover_text(&remove).clicked() {
                            events.push(Event::Action(
                                "parts.row",
                                json!({ "table": "pieces", "remove": i + 1 }),
                            ));
                        }
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
    }
    if editable {
        ui.add_space(t.space_2);
        if ui
            .button(format!("+ {}", strings::text(key!("parts-piece-add"))))
            .clicked()
        {
            events.push(Event::Action("parts.row", json!({ "table": "pieces" })));
        }
    }
}

/// The Symbol cell of piece `i` (doc 07 §5.6): the symbol that draws it, the starter's marked as such; on a part of
/// his own a choice of the symbols of what the piece is, and Change it for this part… and Draw a new symbol…, which
/// open the drawing for this piece.
pub(super) fn piece_symbol_cell(
    ui: &mut Ui,
    open: &OpenSheet,
    i: usize,
    editable: bool,
    events: &mut Vec<Event>,
) {
    let t = theme::current(ui.ctx());
    let choices = open.piece_symbols.get(i).map(Vec::as_slice).unwrap_or_default();
    let chosen = open.sheet.pieces.get(i).map_or("", |p| p.symbol.as_str());
    let words = |address: &str| {
        choices
            .iter()
            .find(|(a, _)| a == address)
            .map(|(a, name)| {
                if a.is_empty() {
                    strings::format(key!("parts-piece-starter"), &[name])
                } else {
                    name.clone()
                }
            })
            .unwrap_or_else(|| "\u{2014}".to_owned())
    };
    if !editable {
        ui.label(RichText::new(words(chosen)).color(t.text_secondary.color32()));
        return;
    }
    let mut picked: Option<Event> = None;
    let name = strings::format(key!("parts-piece-symbol-of"), &[&(i + 1).to_string()]);
    components::choice(
        ui,
        egui::ComboBox::from_id_salt(("part-piece-symbol", i))
            .selected_text(words(chosen))
            .width(t.size_control * 7.0),
        |ui| {
            for (a, _) in choices {
                if ui.selectable_label(a == chosen, words(a)).clicked() {
                    picked = Some(Event::Action(
                        "parts.field",
                        json!({ "field": format!("pieces[{i}].symbol"), "value": a }),
                    ));
                }
            }
            ui.separator();
            for (from, key) in [
                ("change", key!("parts-piece-change")),
                ("new", key!("parts-piece-draw-new")),
            ] {
                if ui.button(strings::text(key)).clicked() {
                    picked = Some(Event::Action("parts.draw", json!({ "piece": i, "from": from })));
                }
            }
        },
    )
    .response
    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::ComboBox, true, &name));
    events.extend(picked);
}
