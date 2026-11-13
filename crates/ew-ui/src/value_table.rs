// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The editable table of a form ([`FormTable`]), Save as macro…'s value sets, in doc 16a §6.21's look without
//! the navigator grid's machinery: a sticky-free header (`size.table_header`, `color.surface.sunken`, the labels in
//! `type.small` SemiBold `color.text.secondary`), the named first column closed by the one vertical line
//! (`color.table.frozen_edge`), body rows `size.row` high with a 1-px `color.border.subtle` line under each, every cell
//! an inline field drawn in its editing look while it has the focus (`color.surface.panel`, a 1-px accent border,
//! `radius.sm`, inset 2 px), a remove button (`x`) at each row's end, and "Add value set" under the table. Columns
//! share the width the first column and the remove button leave, never under `size.table_column.min`; a table wider
//! than the dialog scrolls sideways. Each edit is a view event; the shell keeps the table (no preview, no command).
//!
//! **Accessibility.** A cell field is named "`<column>`, value set `<i>`"; the first column's "Value set, value
//! set `<i>`"; the remove button "Remove value set `<name>`".
//!
//! **Given rows** ([`FormTable::fixed`], the Cable form's wires): the first column is text, not a field, as wide as
//! its longest row (at most half the table), no row is added or removed, a column's header is its name, and a cell field is named "`<column>`, `<first column>`".

use egui::{RichText, TextEdit, Ui};
use ew_viewmodel::key;
use ew_viewmodel::shell::FormTable;
use ew_viewmodel::strings;

use crate::app::Event;
use crate::components;
use crate::icons::IconId;
use crate::theme::{self, Tokens, TypeToken};

/// The SemiBold cut of `type.small` (a header label, doc 16a §6.21).
fn small_strong(t: &Tokens) -> egui::FontId {
    theme::font(&TypeToken {
        semibold: true,
        ..t.type_small
    })
}

/// The width of each variable column: the share of `rest` (the width the first column and the remove button leave),
/// never under `min`.
pub(crate) fn column_width(rest: f32, columns: usize, min: f32) -> f32 {
    if columns == 0 {
        return min;
    }
    (rest / columns as f32).max(min)
}

/// One cell field: frameless, `size.row` high, its editing look while focused. `true` when its text changed (`text`
/// then holds it).
fn cell(ui: &mut Ui, t: &Tokens, width: f32, text: &mut String, name: &str, hint: &str) -> bool {
    let (cell, _) = ui.allocate_exact_size(egui::vec2(width, t.size_row), egui::Sense::hover());
    // The editing fill goes under the text: its slot is taken before the field paints.
    let fill = ui.painter().add(egui::Shape::Noop);
    // The field is the cell less `space.4` at each side, exactly the row high, its text centred on the row (egui would
    // grow it to `interact_size.y` downwards), as the navigator grid's editor (§6.21).
    let field = cell.shrink2(egui::vec2(t.space_4, 0.0));
    ui.spacing_mut().interact_size.y = field.height();
    let edit = ui.place(
        field,
        TextEdit::singleline(text)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .font(theme::font(&t.type_small))
            .text_color(t.text_primary.color32())
            .align(egui::Align2::LEFT_CENTER)
            .desired_width(f32::INFINITY)
            .hint_text(RichText::new(hint).color(t.text_tertiary.color32())),
    );
    if edit.has_focus() {
        let rect = cell.shrink(2.0);
        ui.painter().set(
            fill,
            egui::Shape::rect_filled(rect, theme::radius(t.radius_sm), t.surface_panel.color32()),
        );
        ui.painter().rect_stroke(
            rect,
            theme::radius(t.radius_sm),
            theme::stroke(t.border_width, t.accent),
            egui::StrokeKind::Inside,
        );
    }
    edit.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::TextInput, true, name);
        info.current_text_value = Some(text.clone());
        info
    });
    edit.changed()
}

/// Paints `table` under a form's fields and pushes its edits.
pub(crate) fn show(ui: &mut Ui, table: &FormTable, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    ui.add_space(t.space_4);
    ui.label(
        RichText::new(strings::text(table.heading_key))
            .font(theme::font(&t.type_body_strong))
            .color(t.text_primary.color32()),
    );
    if table.columns.is_empty() {
        ui.label(
            RichText::new(strings::text(key!("form-value-sets-none")))
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
        return;
    }
    // Given rows are text: the first column is as wide as its longest row, so that a long wire name does not run
    // into the next column (the Cable form's "J-E-206-N → -1X1:2/b"), up to half the table.
    let key_width = if table.fixed {
        let font = theme::font(&t.type_small);
        let widest = table
            .rows
            .iter()
            .map(|row| {
                ui.painter()
                    .layout_no_wrap(row.key.clone(), font.clone(), t.text_primary.color32())
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max);
        (widest + 2.0 * t.space_4)
            .max(t.size_table_column)
            .min(ui.available_width() / 2.0)
    } else {
        t.size_table_column
    };
    let remove_width = if table.fixed {
        0.0
    } else {
        t.size_control_inline + t.space_2
    };
    let rest = ui.available_width() - key_width - remove_width;
    let width = column_width(rest, table.columns.len(), t.size_table_column_min);
    let total = key_width + width * table.columns.len() as f32 + remove_width;
    egui::ScrollArea::horizontal()
        .id_salt("form-value-table")
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            // The header.
            let (header, _) =
                ui.allocate_exact_size(egui::vec2(total, t.size_table_header), egui::Sense::hover());
            let painter = ui.painter_at(header);
            painter.rect_filled(header, egui::CornerRadius::ZERO, t.surface_sunken.color32());
            let font = small_strong(&t);
            let mut x = header.left();
            let labels = std::iter::once((strings::text(table.key_label).to_owned(), key_width)).chain(
                table.columns.iter().map(|(name, ty)| {
                    let label = if ty.is_empty() {
                        name.clone()
                    } else {
                        strings::format(key!("macro-value-label"), &[name, &strings::var_type_word(ty)])
                    };
                    (label, width)
                }),
            );
            for (label, w) in labels {
                painter.text(
                    egui::pos2(x + t.space_4, header.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font.clone(),
                    t.text_secondary.color32(),
                );
                x += w;
            }
            ui.painter().hline(
                header.x_range(),
                header.bottom() - 0.5,
                theme::stroke(1.0, t.border_subtle),
            );
            // The rows.
            let top = header.bottom();
            for (i, row) in table.rows.iter().enumerate() {
                let index = (i + 1).to_string();
                let size = egui::vec2(total, t.size_row);
                let r = ui.allocate_ui_with_layout(
                    size,
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_size(size);
                        if table.fixed {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(key_width, t.size_row), egui::Sense::hover());
                            ui.painter().text(
                                egui::pos2(rect.left() + t.space_4, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &row.key,
                                theme::font(&t.type_small),
                                t.text_primary.color32(),
                            );
                        } else {
                            let mut key = row.key.clone();
                            let name = strings::format(
                                key!("form-value-cell"),
                                &[strings::text(table.key_label), &index],
                            );
                            if cell(ui, &t, key_width, &mut key, &name, "") {
                                events.push(Event::FormTableCell(i, None, key));
                            }
                        }
                        for (c, ((col, ty), value)) in table.columns.iter().zip(&row.cells).enumerate() {
                            let mut text = value.clone();
                            let (name, hint) = if table.fixed {
                                (
                                    strings::format(key!("form-table-cell"), &[col, &row.key]),
                                    String::new(),
                                )
                            } else {
                                (
                                    strings::format(key!("form-value-cell"), &[col, &index]),
                                    strings::var_type_word(ty),
                                )
                            };
                            if cell(ui, &t, width, &mut text, &name, &hint) {
                                events.push(Event::FormTableCell(i, Some(c), text));
                            }
                        }
                        if !table.fixed {
                            ui.add_space(t.space_2);
                            let remove = strings::format(key!("form-value-set-remove"), &[&row.key]);
                            if components::icon_button(ui, IconId::x, &remove).clicked() {
                                events.push(Event::FormTableRemove(i));
                            }
                        }
                    },
                );
                let rect = r.response.rect;
                ui.painter().hline(
                    egui::Rangef::new(rect.left(), rect.left() + total),
                    rect.bottom() - 0.5,
                    theme::stroke(1.0, t.border_subtle),
                );
            }
            // The first column's edge, the grid's one vertical line, from the header to the last row.
            let bottom = ui.min_rect().bottom();
            let edge = header.left() + key_width;
            ui.painter().vline(
                edge,
                egui::Rangef::new(header.top(), bottom.max(top)),
                theme::stroke(1.0, t.table_frozen_edge),
            );
        });
    if table.fixed {
        return;
    }
    ui.add_space(t.space_2);
    if components::button_with_icon(
        ui,
        components::Kind::Subtle,
        Some(IconId::plus),
        strings::text(key!("form-value-set-add")),
        true,
        0.0,
    )
    .clicked()
    {
        events.push(Event::FormTableAdd);
    }
}

#[cfg(test)]
mod tests {
    use super::column_width;

    #[test]
    fn columns_share_the_rest_and_keep_their_minimum() {
        assert_eq!(column_width(300.0, 3, 80.0), 100.0);
        assert_eq!(column_width(100.0, 3, 80.0), 80.0);
        assert_eq!(column_width(100.0, 0, 80.0), 80.0);
    }
}
