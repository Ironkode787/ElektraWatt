// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Preview tab (WP-S2.09 part 3, §8.1, D11; FR-UI-022) and the one-line chip that replaces the inline cards while
//! it is open.
//!
//! **The tab** shows the open card: the header "Preview: `<command>` · as of rev `<r>`" with the stale badge, the summary
//! (records, tags, cross-reference texts, pages; an import's created and skipped counts), Apply and Cancel, then the
//! lists in a scroll area, each virtualised: the one that carries the change first and open (Tags for a renumber,
//! Entities otherwise), then the others closed (W31-2), Xrefs and Pages last. Without a card it says "No preview is open". The lists are the shell's
//! ([`ShellModel::preview_pane`]); the tab reads nothing of its own.
//!
//! **The chip** ([`chip`]): "Preview open: `<summary>`" with Apply, Cancel and Show in the origin pane, in place of its
//! inline card.
//!
//! **Accessibility.** The tab is a `Group` named "Preview"; the header is a polite live region; the buttons are named
//! by their text.

use egui::{RichText, Ui};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::PreviewStatus;
use serde_json::{Value, json};

use crate::app::Event;

/// The rows of one list, virtualised, in a collapsing section headed `title (<n>)`, open at first when `open`.
fn list(ui: &mut Ui, title: &str, rows: &[String], open: bool) {
    if rows.is_empty() {
        return;
    }
    egui::CollapsingHeader::new(format!("{title} ({})", rows.len()))
        .id_salt(title)
        .default_open(open)
        .show(ui, |ui| {
            // Rows of `size.row`, at most `size.preview_list` of them on show (doc 16a §6.22).
            let t = crate::theme::current(ui.ctx());
            let row = t.size_row;
            egui::ScrollArea::vertical()
                .id_salt(title)
                .max_height(row * t.size_preview_list_rows as f32)
                .auto_shrink([false, true])
                .show_rows(ui, row, rows.len(), |ui, range| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for r in &rows[range] {
                        let width = ui.available_width();
                        // Left-aligned in the row (W31-2: `add_sized` centred them).
                        ui.allocate_ui_with_layout(
                            egui::vec2(width, row),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| ui.add(egui::Label::new(r).truncate()),
                        );
                    }
                });
        });
}

/// Paints the Preview tab of `shell`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let rect = ui.available_rect_before_wrap();
    let pane_id = ui.scope_id().with("preview-pane");
    let pane = ui.interact(rect, pane_id, egui::Sense::hover());
    pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text("pane-preview")));
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
        let (Some(card), Some(rev)) = (shell.properties().card(), shell.revision()) else {
            ui.label(strings::text("preview-none"));
            return;
        };
        let mut header = strings::format(
            "preview-header",
            &[&card.preview.name, &card.preview.base_rev.to_string()],
        );
        let preview = match &card.preview.result {
            Ok(p) => p,
            Err(m) => {
                ui.colored_label(
                    crate::theme::current(ui.ctx()).error.color32(),
                    strings::format("card-failed", &[m]),
                );
                buttons(ui, &card.origin, events);
                return;
            }
        };
        if card.preview.status(rev) == PreviewStatus::Stale {
            header.push_str(&format!(" [{}]", strings::text("preview-stale")));
        }
        let response = ui.label(RichText::new(&header).strong());
        let live = header.clone();
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_live(egui::accesskit::Live::Polite);
            n.set_value(live);
        });
        let pane = shell.preview_pane();
        ui.label(strings::format(
            "preview-summary",
            &[
                &preview.diff().len().to_string(),
                &pane.tags().len().to_string(),
                &pane.xrefs().len().to_string(),
                &pane.pages().len().to_string(),
            ],
        ));
        if let CardOrigin::Import { created, skipped, .. } = &card.origin {
            ui.label(strings::format(
                "preview-summary-import",
                &[&created.to_string(), &skipped.to_string()],
            ));
        }
        buttons(ui, &card.origin, events);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("preview-lists")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let entities: Vec<String> = pane
                    .entities()
                    .iter()
                    .map(|e| {
                        let mut t = format!("{} {} {}", e.change, e.kind, e.label);
                        if !e.fields.is_empty() {
                            t.push_str(&format!(" ({})", e.fields.join(", ")));
                        }
                        t
                    })
                    .collect();
                let tags: Vec<String> = pane.tags().iter().map(|(a, b)| format!("{a} → {b}")).collect();
                let xrefs: Vec<String> = pane
                    .xrefs()
                    .iter()
                    .map(|x| format!("{}: {} → {}", x.folio, x.before, x.after))
                    .collect();
                // W31-2: the list that carries the change comes first and open (Tags for a renumber, the changed
                // entities otherwise); the others follow, closed.
                let renumber = card.preview.name.ends_with("renumber") && !tags.is_empty();
                if renumber {
                    list(ui, strings::text("preview-tags"), &tags, true);
                    list(ui, strings::text("preview-entities"), &entities, false);
                } else {
                    list(ui, strings::text("preview-entities"), &entities, true);
                    list(ui, strings::text("preview-tags"), &tags, false);
                }
                list(ui, strings::text("preview-xrefs"), &xrefs, false);
                list(ui, strings::text("preview-pages"), pane.pages(), false);
            });
    });
}

/// The action Apply dispatches for a card of `origin`: the Pages navigator's own for a reorder card.
fn apply_action(origin: &CardOrigin) -> &'static str {
    match origin {
        // WP-S2.09 part 2: the quick-fix card is the Messages pane's, applied like the reorder card.
        CardOrigin::Reorder { .. } | CardOrigin::QuickFix { .. } => "navigator.apply",
        _ => "preview.apply",
    }
}

/// Apply and Cancel.
fn buttons(ui: &mut Ui, origin: &CardOrigin, events: &mut Vec<Event>) {
    ui.horizontal(|ui| {
        if ui.button(strings::text("preview-apply")).clicked() {
            events.push(Event::Action(apply_action(origin), Value::Null));
        }
        if ui.button(strings::text("preview-cancel")).clicked() {
            events.push(Event::Action("preview.cancel", json!({})));
        }
    });
}

/// The one-line chip of an origin pane while the Preview tab shows its card: "Preview open: <line>" with Apply, Cancel
/// and Show (which brings the tab to the front). `false` (and nothing painted) while the tab is not open, so the pane
/// paints its inline card.
pub(crate) fn chip(ui: &mut Ui, shell: &ShellModel, line: &str, events: &mut Vec<Event>) -> bool {
    let Some(card) = shell.properties().card().filter(|_| shell.preview_shown()) else {
        return false;
    };
    let text = strings::format("preview-open-chip", &[line]);
    ui.horizontal_wrapped(|ui| {
        let response = ui.label(&text);
        let live = text.clone();
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_live(egui::accesskit::Live::Polite);
            n.set_value(live);
        });
        if ui.small_button(strings::text("preview-apply")).clicked() {
            events.push(Event::Action(apply_action(&card.origin), Value::Null));
        }
        if ui.small_button(strings::text("preview-cancel")).clicked() {
            events.push(Event::Action("preview.cancel", json!({})));
        }
        if ui.small_button(strings::text("preview-show")).clicked() {
            events.push(Event::Action("preview.show", json!({ "on": true })));
        }
    });
    true
}
