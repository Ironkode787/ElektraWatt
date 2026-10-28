// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The history panel (WP-S2.09 part 3, §8.4, D13; FR-UI-023) and its Show changes popup, in the design language of
//! doc 16a §6.24 (WP-S2.17 part 3, DK11, DK12; lane K32).
//!
//! **The panel** lists the shell's history lines newest first ([`ew_viewmodel::undo_history::HistoryPane::lines`]).
//! The header ("12 steps · cursor at 9", then FT-518's "oldest kept: step `<k>` · `<a>` MB of `<b>` MB") is in
//! `type.small` `color.text.secondary`. An entry is a `size.row` row: a 16-px state icon column (the current entry
//! `arrow-right` in `color.accent`, an undone entry `arrow-back-up` in `color.history.undone`, a done entry none), the
//! entry number right-aligned in a 28-px column (`type.small`, `color.text.tertiary`), the label in `type.body`
//! (`type.body.strong` for the current entry, `color.history.undone` for an undone one), the origin and the pages in
//! `type.small` `color.text.tertiary`, a "merged n" badge and the "Not undoable" status badge (`lock`). The row
//! cursor takes the Selected state (`color.accent.subtle`). The row actions are 24-px icon buttons in
//! `color.text.secondary`: Undo to here (`history`; Redo to here, `arrow-forward-up`, on an undone entry) and Show
//! changes (`list-details`). A marker line is a separator row: its text in `type.small` `color.text.tertiary`, then a
//! 1-px `color.border.subtle` rule to the row's end. Without steps the panel shows the empty state (doc 16a §6.17).
//!
//! **Keys (FT-519).** The panel takes the keyboard focus (Tab, a click on a row): Up and Down move the row cursor
//! (`history.row`), Enter runs Undo to here on it (`history.undo_to`). A row's actions show on the cursor row, on
//! hover and while one of them has the focus (16a §6.25 follow-up 5); on the other rows they keep their nodes (Tab
//! still reaches them) but are not painted. A double click on a row is Undo to here.
//!
//! **Show changes** is the dialog frame of doc 16a §6.12, small and non-modal: "Changes of step n", the entry's records
//! (at most 100) as list rows, Close as a Secondary button; Escape closes.
//!
//! **Accessibility.** The panel is a `Group` named "History"; each entry is a `Group` named by its label and state
//! (", undone" for an undone entry); a marker line is a `Splitter` node named by its text; the icon buttons are named
//! by their actions, with the name as the tooltip.

use egui::{Align, Context, Key, Layout, Modifiers, Rect, RichText, Sense, Ui, Vec2};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::ew_api::output::HistoryRow;
use ew_viewmodel::undo_history::{EntryState, HistoryLine, MAX_CHANGES};
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Badge, Kind};
use crate::icons::{self, IconId};
use crate::theme::{self, Rgba, Tokens};

/// The width of the number column (doc 16a §6.24).
const NUMBER_COLUMN: f32 = 28.0;

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

/// The state icon of an entry (doc 16a §6.24): the current entry `arrow-right` in `color.accent`, an undone one
/// `arrow-back-up` in `color.history.undone`, a done one none.
pub(crate) fn state_icon(t: &Tokens, state: EntryState) -> Option<(IconId, Rgba)> {
    match state {
        EntryState::Now => Some((IconId::arrow_right, t.accent)),
        EntryState::Redo => Some((IconId::arrow_back_up, t.history_undone)),
        EntryState::Done => None,
    }
}

/// The colour of an entry's label: `color.history.undone` for an undone entry (`color.text.secondary` while it is the
/// row cursor, on the Selected fill, §4.15), else `color.text.primary`.
fn label_colour(t: &Tokens, state: EntryState, selected: bool) -> Rgba {
    match state {
        EntryState::Redo if selected => t.text_secondary,
        EntryState::Redo => t.history_undone,
        EntryState::Now | EntryState::Done => t.text_primary,
    }
}

/// The as-built text of an entry: number, label, origin, pages, "merged n" and "not undoable" (its accessible name).
fn entry_text(row: &HistoryRow, origin: &str) -> String {
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
    text
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
        let t = theme::current(ui.ctx());
        if view.rows.is_empty() {
            // WP-S2.17 part 4 (CP5, doc 16a §6.17): the empty state.
            components::empty_state(
                ui,
                IconId::history,
                strings::text("history-empty"),
                strings::text("empty-history-text"),
                None,
            );
            return;
        }
        let small = |ui: &mut Ui, text: String| {
            ui.label(
                RichText::new(text)
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            );
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = t.space_4;
            small(
                ui,
                strings::format(
                    "history-header",
                    &[&view.rows.len().to_string(), &view.cursor.to_string()],
                ),
            );
            // FT-518: the oldest step kept and the memory the steps hold against the bound.
            let mb = |b: usize| format!("{:.1}", b as f64 / 1_000_000.0);
            let oldest = view.rows.first().map_or(1, |r| r.index + 1);
            small(
                ui,
                strings::format(
                    "history-header-kept",
                    &[&oldest.to_string(), &mb(view.bytes), &mb(view.max_bytes)],
                ),
            );
        });
        egui::ScrollArea::vertical()
            .id_salt("history-rows")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for line in history.lines(shell.markers()) {
                    match line {
                        HistoryLine::Marker(m) => marker_row(ui, &t, &m.text),
                        HistoryLine::Entry { row, state, origin } => {
                            let selected = history.row_cursor() == Some(row.index);
                            let entry = Entry {
                                row,
                                state,
                                origin,
                                selected,
                                focused: focused_row == Some(row.index),
                            };
                            let out = entry_row(ui, &t, &entry);
                            if out.focused {
                                focused_now = Some(row.index);
                            }
                            if out.row_clicked {
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
                            if out.undo {
                                events.push(Event::Action(
                                    "history.undo_to",
                                    json!({ "entry": row.index + 1 }),
                                ));
                            }
                            if out.changes {
                                events.push(Event::Action(
                                    "history.changes",
                                    json!({ "entry": row.index + 1 }),
                                ));
                            }
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

/// A marker line as a separator row: the text in `type.small` `color.text.tertiary`, then a 1-px `color.border.subtle`
/// rule to the row's end; a `Splitter` node named by the text.
fn marker_row(ui: &mut Ui, t: &Tokens, text: &str) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), t.size_row), Sense::hover());
    let name = text.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Splitter, true, &name));
    let painter = ui.painter_at(rect);
    let galley = painter.layout_no_wrap(
        text.to_owned(),
        theme::font(&t.type_small),
        t.text_tertiary.color32(),
    );
    let x = rect.left() + t.space_4 + t.icon_sm + t.space_2 + NUMBER_COLUMN + t.space_3;
    let y = rect.center().y;
    painter.galley(
        egui::pos2(x, y - galley.size().y / 2.0),
        galley.clone(),
        t.text_tertiary.color32(),
    );
    let line_from = x + galley.size().x + t.space_3;
    if line_from < rect.right() - t.space_4 {
        let line = Rect::from_min_max(
            egui::pos2(line_from, y - t.border_width / 2.0),
            egui::pos2(rect.right() - t.space_4, y + t.border_width / 2.0),
        );
        painter.rect_filled(line, theme::radius(t.radius_none), t.border_subtle.color32());
    }
}

/// What an entry row shows.
struct Entry<'a> {
    row: &'a HistoryRow,
    state: EntryState,
    origin: &'a str,
    /// Whether it is the row cursor.
    selected: bool,
    /// Whether one of its actions had the keyboard focus in the last frame.
    focused: bool,
}

/// What happened on an entry row.
#[derive(Default)]
struct EntryOut {
    row_clicked: bool,
    undo: bool,
    changes: bool,
    focused: bool,
}

/// One entry row (doc 16a §6.24).
fn entry_row(ui: &mut Ui, t: &Tokens, e: &Entry<'_>) -> EntryOut {
    let mut out = EntryOut::default();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), t.size_row), Sense::click());
    let text = entry_text(e.row, e.origin);
    let name = if e.state == EntryState::Redo {
        format!("{text}, {}", strings::text("history-redo"))
    } else {
        text
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
    if e.selected {
        ui.painter()
            .rect_filled(rect, theme::radius(t.radius_sm), t.accent_subtle.color32());
    }
    crate::command_bar::state_overlay(ui, t, &response, true, t.radius_sm);
    let hovered = ui.rect_contains_pointer(rect);
    let on_show = e.selected || hovered || e.focused;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(t.space_4, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing.x = t.space_2;
    // The state icon column.
    let (icon_rect, _) = child.allocate_exact_size(Vec2::splat(t.icon_sm), Sense::hover());
    if let Some((id, colour)) = state_icon(t, e.state) {
        icons::paint(child.painter(), id, icon_rect, colour.color32(), t.icon_stroke);
    }
    // The number, right-aligned in its column.
    let (number_rect, _) = child.allocate_exact_size(Vec2::new(NUMBER_COLUMN, t.size_row), Sense::hover());
    let number = child.painter().layout_no_wrap(
        (e.row.index + 1).to_string(),
        theme::font(&t.type_small),
        t.text_tertiary.color32(),
    );
    child.painter().galley(
        egui::pos2(
            number_rect.right() - number.size().x,
            number_rect.center().y - number.size().y / 2.0,
        ),
        number,
        t.text_tertiary.color32(),
    );
    child.add_space(t.space_1);
    // The actions at the right first, so the text takes what is left.
    child.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = t.space_1;
        // FT-519: the actions keep their nodes on every row and are painted only when on show.
        let mut actions = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(ui.max_rect())
                .layout(Layout::right_to_left(Align::Center)),
        );
        if !on_show {
            actions.set_opacity(0.0);
        }
        // W31-7: `history.undo_to` redoes up to an undone entry (§8.4), so its row offers Redo to here; the others keep
        // Undo to here (§8.8).
        let (walk, walk_icon) = match e.state {
            EntryState::Redo => ("history-redo-to", IconId::arrow_forward_up),
            EntryState::Now | EntryState::Done => ("action-history-undo-to", IconId::history),
        };
        let changes = components::icon_button(
            &mut actions,
            IconId::list_details,
            strings::text("action-history-changes"),
        );
        let undo = components::icon_button(&mut actions, walk_icon, strings::text(walk));
        out.focused = undo.has_focus() || changes.has_focus();
        out.undo = undo.clicked() || response.double_clicked();
        out.changes = changes.clicked();
        ui.advance_cursor_after_rect(actions.min_rect());
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = t.space_2;
            let font = if e.state == EntryState::Now {
                &t.type_body_strong
            } else {
                &t.type_body
            };
            ui.add(
                egui::Label::new(
                    RichText::new(&e.row.label)
                        .font(theme::font(font))
                        .color(label_colour(t, e.state, e.selected).color32()),
                )
                .selectable(false)
                .truncate(),
            );
            let mut meta = e.origin.to_owned();
            if !e.row.pages.is_empty() {
                meta.push_str(&format!(" · {}", e.row.pages.join(", ")));
            }
            ui.add(
                egui::Label::new(
                    RichText::new(meta)
                        .font(theme::font(&t.type_small))
                        .color(t.text_tertiary.color32()),
                )
                .selectable(false)
                .truncate(),
            );
            if e.row.merged > 1 {
                components::caption_badge(
                    ui,
                    &strings::format("history-merged", &[&e.row.merged.to_string()]),
                );
            }
            if !e.row.step {
                components::status_badge(ui, Badge::NotUndoable);
            }
        });
    });
    out.row_clicked = response.clicked();
    out
}

/// The Show changes popup while it is open: the dialog frame of doc 16a §6.12, small and non-modal.
pub fn show_changes(ctx: &Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some((index, records)) = shell.history_pane().changes() else {
        return;
    };
    let t = theme::current(ctx);
    let title = strings::format("history-changes-title", &[&(index + 1).to_string()]);
    let frame = egui::Frame::new()
        .fill(t.surface_raised.color32())
        .stroke(theme::stroke(t.border_raised_width, t.border_default))
        .corner_radius(theme::radius(t.radius_lg))
        .shadow(theme::shadow(&t.elevation_2))
        .inner_margin(egui::Margin::same(t.space_5 as i8));
    egui::Window::new(&title)
        .title_bar(false)
        .frame(frame)
        .collapsible(false)
        .resizable(false)
        .fixed_size(Vec2::new(t.size_dialog_small - 2.0 * t.space_5, 0.0))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = t.space_3;
            // The title is painted: the window's node carries the name.
            let galley = ui.painter().layout_no_wrap(
                title.clone(),
                theme::font(&t.type_title),
                t.text_primary.color32(),
            );
            let (r, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
            ui.painter().galley(r.min, galley, t.text_primary.color32());
            egui::ScrollArea::vertical()
                .max_height(t.size_row * 10.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for e in records.iter().take(MAX_CHANGES) {
                        let mut line = format!("{} {} {}", e.change, e.kind, e.label);
                        if !e.fields.is_empty() {
                            line.push_str(&format!(" ({})", e.fields.join(", ")));
                        }
                        ui.horizontal(|ui| {
                            ui.set_min_height(t.size_row);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(line)
                                        .font(theme::font(&t.type_body))
                                        .color(t.text_primary.color32()),
                                )
                                .truncate(),
                            );
                        });
                    }
                });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let close = components::button(
                    ui,
                    Kind::Secondary,
                    strings::text("toast-close"),
                    true,
                    t.size_button_min_width,
                );
                if close.clicked() {
                    events.push(Event::Action("ui.dismiss", json!({})));
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DK11: the state icons and the undone tone of doc 16a §6.24 in every theme.
    #[test]
    fn dk11_state_icons_and_the_undone_tone() {
        use crate::theme::{Density, Theme, tokens};
        for theme in Theme::ALL {
            let t = tokens(theme, Density::Comfortable);
            assert_eq!(
                state_icon(&t, EntryState::Now),
                Some((IconId::arrow_right, t.accent))
            );
            assert_eq!(
                state_icon(&t, EntryState::Redo),
                Some((IconId::arrow_back_up, t.history_undone))
            );
            assert_eq!(state_icon(&t, EntryState::Done), None);
            assert_eq!(label_colour(&t, EntryState::Redo, false), t.history_undone);
            assert_eq!(label_colour(&t, EntryState::Redo, true), t.text_secondary);
            assert_eq!(label_colour(&t, EntryState::Now, false), t.text_primary);
        }
    }
}
