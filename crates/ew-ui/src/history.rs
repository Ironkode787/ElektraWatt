// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The history panel (WP-S2.09 part 3, §8.4, D13; FR-UI-023) and its Show changes popup.
//!
//! **The panel** lists the shell's history lines newest first ([`ew_viewmodel::undo_history::HistoryPane::lines`]):
//! an entry with its state (undone entries greyed), label, origin, pages, `merged <k>` and "not undoable", and Undo to
//! here and Show changes; a marker line in italics. The header says how many steps are kept and how much memory they
//! hold (window only). The row cursor of `history.row` is highlighted; a double click on a row is Undo to here. The
//! row cursor's keys (Up, Down, Enter) are the scripts' `history.row` and `history.undo_to`; the window reaches the
//! rows' buttons with Tab.
//!
//! **Show changes** is a window of the entry's records (at most 100) with Close (Escape).
//!
//! **Accessibility.** The panel is a `Group` named "History"; each entry is a `Group` named by its label and state.

use egui::{Context, RichText, Ui};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::undo_history::{EntryState, HistoryLine, MAX_CHANGES};
use serde_json::json;

use crate::app::Event;

/// Paints the history panel of `shell`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let rect = ui.available_rect_before_wrap();
    let pane_id = ui.scope_id().with("history-pane");
    let pane = ui.interact(rect, pane_id, egui::Sense::hover());
    pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text("pane-history")));
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
        let history = shell.history_pane();
        let Some(view) = history.view() else {
            return;
        };
        if view.rows.is_empty() {
            ui.label(strings::text("history-empty"));
        } else {
            ui.label(strings::format(
                "history-header",
                &[&view.rows.len().to_string(), &view.cursor.to_string()],
            ));
        }
        egui::ScrollArea::vertical()
            .id_salt("history-rows")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for line in history.lines(shell.markers()) {
                    match line {
                        HistoryLine::Marker(m) => {
                            ui.label(RichText::new(&m.text).italics());
                        }
                        HistoryLine::Entry { row, state, origin } => {
                            let mut text = format!("{}  {}  ({origin})", row.index + 1, row.label);
                            if !row.pages.is_empty() {
                                text.push_str(&format!("  {}", row.pages.join(", ")));
                            }
                            if row.merged > 1 {
                                text.push_str(&format!(
                                    "  {}",
                                    strings::format("history-merged", &[&row.merged.to_string()])
                                ));
                            }
                            if !row.step {
                                text.push_str(&format!("  {}", strings::text("history-not-undoable")));
                            }
                            let mut rich = RichText::new(&text);
                            match state {
                                EntryState::Redo => rich = rich.weak(),
                                EntryState::Now => rich = rich.strong(),
                                EntryState::Done => {}
                            }
                            let selected = history.row_cursor() == Some(row.index);
                            ui.horizontal(|ui| {
                                let r = ui.selectable_label(selected, rich);
                                let name = if state == EntryState::Redo {
                                    format!("{text}, {}", strings::text("history-redo"))
                                } else {
                                    text.clone()
                                };
                                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
                                if r.double_clicked()
                                    || ui.small_button(strings::text("action-history-undo-to")).clicked()
                                {
                                    events.push(Event::Action(
                                        "history.undo_to",
                                        json!({ "entry": row.index + 1 }),
                                    ));
                                }
                                if ui.small_button(strings::text("action-history-changes")).clicked() {
                                    events.push(Event::Action(
                                        "history.changes",
                                        json!({ "entry": row.index + 1 }),
                                    ));
                                }
                            });
                        }
                    }
                }
            });
    });
}

/// The Show changes popup while it is open.
pub fn show_changes(ctx: &Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some((index, records)) = shell.history_pane().changes() else {
        return;
    };
    egui::Window::new(strings::format(
        "history-changes-title",
        &[&(index + 1).to_string()],
    ))
    .collapsible(false)
    .resizable(true)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for e in records.iter().take(MAX_CHANGES) {
                let mut t = format!("{} {} {}", e.change, e.kind, e.label);
                if !e.fields.is_empty() {
                    t.push_str(&format!(" ({})", e.fields.join(", ")));
                }
                ui.label(t);
            }
        });
        if ui.button(strings::text("toast-close")).clicked() {
            events.push(Event::Action("ui.dismiss", json!({})));
        }
    });
}
