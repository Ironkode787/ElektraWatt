// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The history panel (WP-S2.09 part 3, §8.4, D13; FR-UI-023) and its Show changes popup.
//!
//! **The panel** lists the shell's history lines newest first ([`ew_viewmodel::undo_history::HistoryPane::lines`]):
//! an entry with its state (undone entries greyed), label, origin, pages, `merged <k>` and "not undoable", and Undo to
//! here and Show changes; a marker line in italics. The header says how many steps are kept and how much memory they
//! hold (window only; FT-518: "oldest kept: step `<k>` · `<a>` MB of `<b>` MB"). The row cursor of `history.row` is
//! highlighted; a double click on a row is Undo to here.
//!
//! **Keys (FT-519).** The panel takes the keyboard focus (Tab, a click on a row): Up and Down move the row cursor
//! (`history.row`), Enter runs Undo to here on it (`history.undo_to`). A row's Undo to here and Show changes show on
//! the cursor row, on hover and while one of them has the focus (16a §6.25 follow-up 5); on the other rows they keep
//! their nodes (Tab still reaches them) but are not painted.
//!
//! **Show changes** is a window of the entry's records (at most 100) with Close (Escape).
//!
//! **Accessibility.** The panel is a `Group` named "History"; each entry is a `Group` named by its label and state.

use egui::{Context, Key, Modifiers, RichText, Ui};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::undo_history::{EntryState, HistoryLine, MAX_CHANGES};
use serde_json::json;

use crate::app::Event;

/// The panel's focusable node (one History pane at a time).
fn pane_id() -> egui::Id {
    egui::Id::unique("ew-history-pane")
}

/// FT-519: the row cursor's keys while the panel has the keyboard focus (called before the frame's other keys):
/// Down and Up move the row cursor, Enter runs Undo to here on it; `true` when a key was taken.
pub(crate) fn keys(ctx: &Context, events: &mut Vec<Event>) -> bool {
    if !ctx.memory(|m| m.has_focus(pane_id())) {
        return false;
    }
    let pressed = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
    if pressed(Key::ArrowDown) {
        events.push(Event::Action("history.row", json!({ "step": 1 })));
    } else if pressed(Key::ArrowUp) {
        events.push(Event::Action("history.row", json!({ "step": -1 })));
    } else if pressed(Key::Enter) {
        events.push(Event::Action("history.undo_to", json!({})));
    } else {
        return false;
    }
    true
}

/// Paints the history panel of `shell`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let rect = ui.available_rect_before_wrap();
    let pane_id = pane_id();
    let pane = ui.interact(rect, pane_id, egui::Sense::focusable_noninteractive());
    pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text("pane-history")));
    // FT-519: the panel holds the shell's focus like the other panes (F6, a click, Tab).
    let ours = shell.focus() == Some(ew_viewmodel::shell::Focus::History);
    if ours && !pane.has_focus() && !ui.ctx().text_edit_focused() && shell.dialog().is_none() {
        pane.request_focus();
    }
    if pane.gained_focus() && !ours {
        events.push(Event::Action("ui.focus", json!({"area": "history"})));
    }
    if pane.has_focus() {
        // FT-519: the arrows stay with the panel (`keys`).
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                pane_id,
                egui::EventFilter {
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
    }
    // The row whose action had the focus in the last frame keeps its actions on show.
    let focus_key = pane_id.with("focused-row");
    let focused_row: Option<usize> = ui.data(|d| d.get_temp(focus_key));
    let mut focused_now = None;
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
            // FT-518: the oldest step kept and the memory the steps hold against the bound.
            let t = crate::theme::current(ui.ctx());
            let mb = |b: usize| format!("{:.1}", b as f64 / 1_000_000.0);
            let oldest = view.rows.first().map_or(1, |r| r.index + 1);
            ui.label(
                RichText::new(strings::format(
                    "history-header-kept",
                    &[&oldest.to_string(), &mb(view.bytes), &mb(view.max_bytes)],
                ))
                .font(crate::theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
            );
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
                            let selected = history.row_cursor() == Some(row.index);
                            match state {
                                // An undone entry in `color.history.undone`; selected, in text.secondary (§4.15);
                                // italic either way, so a selected undone row still reads as undone (W31-7).
                                EntryState::Redo => {
                                    rich = rich.italics();
                                    let t = crate::theme::current(ui.ctx());
                                    let colour = if selected {
                                        t.text_secondary
                                    } else {
                                        t.history_undone
                                    };
                                    rich = rich.color(colour.color32());
                                }
                                EntryState::Now => rich = rich.strong(),
                                EntryState::Done => {}
                            }
                            ui.horizontal(|ui| {
                                let hovered = ui.rect_contains_pointer(egui::Rect::from_min_size(
                                    ui.max_rect().min,
                                    egui::vec2(ui.max_rect().width(), ui.spacing().interact_size.y),
                                ));
                                let on_show = selected || hovered || focused_row == Some(row.index);
                                let r = ui.selectable_label(selected, rich);
                                if r.clicked() {
                                    // The click moves the row cursor there (`history.row` steps toward older entries
                                    // from the cursor row, else from the newest applied one) and focuses the panel.
                                    events.push(Event::Action("ui.focus", json!({"area": "history"})));
                                    let last = view.rows.len().saturating_sub(1);
                                    let from = history
                                        .row_cursor()
                                        .unwrap_or(view.cursor.saturating_sub(1).min(last));
                                    let step = from as i64 - row.index as i64;
                                    events.push(Event::Action("history.row", json!({ "step": step })));
                                }
                                let name = if state == EntryState::Redo {
                                    format!("{text}, {}", strings::text("history-redo"))
                                } else {
                                    text.clone()
                                };
                                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
                                // W31-7: `history.undo_to` redoes up to an undone entry (§8.4), so its row offers
                                // Redo to here; the others keep Undo to here (§8.8).
                                let walk = match state {
                                    EntryState::Redo => "history-redo-to",
                                    EntryState::Now | EntryState::Done => "action-history-undo-to",
                                };
                                // FT-519: the actions keep their nodes on every row and are painted only when on
                                // show.
                                if !on_show {
                                    ui.set_opacity(0.0);
                                }
                                let undo = ui.small_button(strings::text(walk));
                                let changes = ui.small_button(strings::text("action-history-changes"));
                                if undo.has_focus() || changes.has_focus() {
                                    focused_now = Some(row.index);
                                }
                                if r.double_clicked() || undo.clicked() {
                                    events.push(Event::Action(
                                        "history.undo_to",
                                        json!({ "entry": row.index + 1 }),
                                    ));
                                }
                                if changes.clicked() {
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
    if focused_now != focused_row {
        ui.data_mut(|d| match focused_now {
            Some(i) => {
                d.insert_temp(focus_key, i);
            }
            None => d.remove::<usize>(focus_key),
        });
        ui.ctx().request_repaint();
    }
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
