// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock in Parts (doc 07 §5.10, picture v11): In this project narrowed to Workshop stock, its parts in
//! its order under a heading per kind that says the rule, each numbered and dragged to another place among its kind;
//! on a stock part's sheet the line that says where it stands with Up, Down and Take out, or Add to the stock; beside
//! the sheet the card with Add from my parts…, Add the generic stock and Use this stock for new projects.

use egui::{RichText, ScrollArea, Ui};
use ew_viewmodel::ew_api::parts::{PartFound, PartScope};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::parts_place::PartsPlace;
use ew_viewmodel::shell::stock::{StockPlace, stock_rule};
use ew_viewmodel::{key, strings};
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::icons::IconId;
use crate::theme;

/// The kinds of the stock, in the view's order, as the file spells them.
const KINDS: [&str; 4] = ["wire", "ferrule", "marker", "gland"];

/// A stock row being dragged: its part, as `parts.open` names it, and its kind.
#[derive(Clone)]
struct StockDrag {
    part: String,
    kind: String,
}

/// The chip Workshop stock of the filter row: In this project narrowed to the stock, and back.
pub(super) fn chip(ui: &mut Ui, place: &PartsPlace, events: &mut Vec<Event>) {
    let on = place.scope == PartScope::Stock;
    if ui
        .selectable_label(on, strings::text(key!("parts-stock-chip")))
        .clicked()
    {
        let scope = if on { PartScope::Project } else { PartScope::Stock };
        events.push(Event::Action("parts.find", json!({ "scope": scope.word() })));
    }
}

/// The list of the stock: the count line, then a group per kind with its rule, each row numbered in the stock's order
/// and dragged to another place among its kind (`parts.stock {op: move}`).
pub(super) fn list(ui: &mut Ui, place: &PartsPlace, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let all = strings::format_count(ew_viewmodel::count_key!("parts-count"), place.found.len());
    ui.label(components::small_secondary(
        &t,
        strings::format(key!("parts-stock-count"), &[&all]),
    ));
    ui.add_space(t.space_2);
    let open = place.sheet.as_ref().map(|o| o.sheet.copy);
    ScrollArea::vertical()
        .id_salt("parts-stock-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for kind in KINDS {
                let rows: Vec<&PartFound> = place.found.iter().filter(|f| f.offer.kind == kind).collect();
                if rows.is_empty() {
                    continue;
                }
                let word = strings::format(
                    key!("parts-stock-group"),
                    &[
                        &ew_viewmodel::shell::part_dialog::part_kind_plural(kind),
                        stock_rule(kind).unwrap_or_default(),
                    ],
                );
                let _ = super::list_group(ui, &word, rows.len(), false);
                for (i, row) in rows.into_iter().enumerate() {
                    let param = row.param();
                    let r = super::list_row(ui, row, open == Some(row.copy), Some(i + 1));
                    if r.clicked() {
                        events.push(Event::Action("parts.open", json!({ "part": param })));
                    }
                    let drag = ui.interact(r.rect, r.id.with("stock-drag"), egui::Sense::drag());
                    drag.dnd_set_drag_payload(StockDrag {
                        part: param.clone(),
                        kind: kind.to_owned(),
                    });
                    // Where a dragged row of the same kind would go: a line over this one.
                    if r.dnd_hover_payload::<StockDrag>()
                        .is_some_and(|d| d.kind == kind && d.part != param)
                    {
                        ui.painter()
                            .hline(r.rect.x_range(), r.rect.top(), theme::stroke(2.0, t.accent));
                    }
                    if let Some(d) = r.dnd_release_payload::<StockDrag>()
                        && d.kind == kind
                        && d.part != param
                    {
                        events.push(Event::Action(
                            "parts.stock",
                            json!({ "op": "move", "part": d.part, "to": i + 1 }),
                        ));
                    }
                }
            }
        });
}

/// The line under the heading of a stock part's sheet: where it stands in the stock, with Up, Down and Take out; or
/// that it is not in it, with Add to the stock. Nothing for a part the stock does not hold.
pub(super) fn strip(ui: &mut Ui, shell: &ShellModel, read_only: bool, events: &mut Vec<Event>) {
    let Some(at) = shell.open_stock_place() else {
        return;
    };
    let t = theme::current(ui.ctx());
    let refusal = read_only.then(|| strings::text(key!("reason-read-only")));
    ui.add_space(t.space_2);
    egui::Frame::NONE
        .fill(t.accent_subtle.color32())
        .corner_radius(theme::radius(t.radius_md))
        .inner_margin(egui::Margin::symmetric(t.space_3 as i8, t.space_2 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let line = match at {
                StockPlace::In { place, of } => {
                    strings::format(key!("parts-stock-in"), &[&place.to_string(), &of.to_string()])
                }
                StockPlace::Out => strings::text(key!("parts-stock-out")).to_owned(),
            };
            // The buttons at the right, the line in the room they leave, wrapped.
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut button =
                        |ui: &mut Ui, words: &str, off: Option<&str>, params: serde_json::Value| {
                            if components::button(ui, components::Kind::Secondary, words, off, 0.0).clicked()
                            {
                                events.push(Event::Action("parts.stock", params));
                            }
                        };
                    match at {
                        StockPlace::In { place, of } => {
                            // Right to left: Take out at the right edge.
                            button(
                                ui,
                                strings::text(key!("parts-stock-take-out")),
                                refusal,
                                json!({ "op": "remove" }),
                            );
                            button(
                                ui,
                                strings::text(key!("parts-stock-down")),
                                refusal.or((place >= of).then(|| strings::text(key!("parts-stock-last")))),
                                json!({ "op": "move", "to": place + 1 }),
                            );
                            button(
                                ui,
                                strings::text(key!("parts-stock-up")),
                                refusal.or((place <= 1).then(|| strings::text(key!("parts-stock-first")))),
                                json!({ "op": "move", "to": place.saturating_sub(1).max(1) }),
                            );
                        }
                        StockPlace::Out => button(
                            ui,
                            strings::text(key!("parts-stock-add")),
                            refusal,
                            json!({ "op": "add" }),
                        ),
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(line).wrap());
                    });
                });
            });
        });
}

/// The card beside the sheet in the Workshop stock view: what the stock is, Add from my parts…, Add the generic stock,
/// Use this stock for new projects, and where Cut to is set.
pub(super) fn card(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let read_only = (!shell.is_writable()).then(|| strings::text(key!("reason-read-only")));
    egui::Frame::NONE
        .fill(t.surface_panel.color32())
        .stroke(theme::stroke(t.border_width, t.border_subtle))
        .corner_radius(theme::radius(t.radius_md))
        .inner_margin(egui::Margin::same(t.space_3 as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(strings::text(key!("parts-stock-title"))).font(theme::font(&t.type_label)),
            );
            ui.add_space(t.space_1);
            ui.add(
                egui::Label::new(components::body_secondary(
                    &t,
                    strings::text(key!("parts-stock-text")),
                ))
                .wrap(),
            );
            ui.add_space(t.space_2);
            let mut row =
                |ui: &mut Ui, icon: Option<IconId>, words: &str, off: Option<&str>, event: Event| {
                    if components::button_with_icon(ui, components::Kind::Secondary, icon, words, off, 0.0)
                        .clicked()
                    {
                        events.push(event);
                    }
                    ui.add_space(t.space_1);
                };
            row(
                ui,
                Some(IconId::plus),
                strings::text(key!("parts-stock-add-mine")),
                None,
                Event::Action("parts.find", json!({ "scope": PartScope::Mine.word() })),
            );
            row(
                ui,
                None,
                strings::text(key!("action-workshop-add-generic")),
                read_only,
                Event::Action("parts.stock", json!({ "op": "generic" })),
            );
            row(
                ui,
                None,
                strings::text(key!("parts-stock-default")),
                None,
                Event::Action("parts.stock", json!({ "op": "default" })),
            );
            ui.add_space(t.space_1);
            ui.add(
                egui::Label::new(components::small_secondary(
                    &t,
                    strings::text(key!("parts-stock-cut")),
                ))
                .wrap(),
            );
        });
}
