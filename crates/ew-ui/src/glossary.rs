// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The glossary pane (WP-S2.08 part 6, D16; FR-UI-045) in the bottom dock: the shared filter field (doc 16a §6.5),
//! then the table of doc 16a §6.7 with the columns EW term, EPLAN, QET, Definition and Actions. The header row is
//! `type.small` SemiBold in `color.text.secondary` on `color.surface.sunken`; a row is `type.small` with `space.4`
//! cell padding and a 1-px `color.border.subtle` line under it; the definition wraps. A row's actions are Subtle
//! buttons: an action that arms a tool reads "Insert" (named `Insert: <the action>`), any other its menu label; a
//! disabled one keeps its place and says why in its tooltip. Without a matching term the pane shows the empty state.
//!
//! **Accessibility.** The pane is a `Group` named "Glossary"; the field is named by its hint; each row is a `Group`
//! named by its term, its other cells its description (`EPLAN: …; QET: …; <definition>`, "none" for an empty cell).
//! In the field, Up and Down move a row cursor (`glossary.row`; the row selected for AccessKit) and Enter runs the
//! cursor row's first enabled action (FT-1281).

use egui::{Sense, Ui, Vec2};
use ew_viewmodel::shell::{GlossaryRow, ShellModel};
use ew_viewmodel::strings::{self, Key};
use ew_viewmodel::{actions, key};
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::{self, Kind, Tip as _};
use crate::icons::IconId;
use crate::theme::{self, Tokens};

/// The share of the table's width each text column takes at least (EW term, EPLAN, QET; the definition takes the rest
/// after the actions, whose column is as wide as its widest button, so a button never wraps).
pub const TEXT_SHARE: f32 = 0.15;

/// The share a text column takes at most when its longest word needs more than [`TEXT_SHARE`] (W49.2 F8).
pub const TEXT_SHARE_MAX: f32 = 0.25;

/// The widths of the columns of a table `width` wide (EW term, EPLAN, QET, Definition, Actions): each text column
/// [`TEXT_SHARE`] of the width, or the width its longest word needs (`longest`, with the cell padding) up to
/// [`TEXT_SHARE_MAX`], so a term wraps between words, never inside one, where the pane allows it (doc 16a §6.7); the
/// actions `actions`; the definition the rest.
pub fn column_widths(width: f32, longest: [f32; 3], actions: f32) -> [f32; 5] {
    let text = longest.map(|w| w.max(width * TEXT_SHARE).min(width * TEXT_SHARE_MAX).ceil());
    let definition = (width - text.iter().sum::<f32>() - actions).max(0.0);
    [text[0], text[1], text[2], definition, actions]
}

/// The text of a glossary button for action `id`, and its accessible name: "Insert" for an action that arms a tool,
/// else the action's menu label.
pub(crate) fn action_words(id: &str) -> Option<(String, String)> {
    let meta = actions::find(id)?;
    let label = strings::text(Key::run_time(&meta.label_key())).to_owned();
    Some(if id.starts_with("tool.") {
        let insert = strings::text(key!("glossary-insert")).to_owned();
        (insert, strings::format(key!("glossary-insert-name"), &[&label]))
    } else {
        (label.clone(), label)
    })
}

/// Paints the glossary pane of `shell`.
pub(crate) fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let rect = ui.available_rect_before_wrap();
    let pane_id = ui.scope_id().with("glossary-pane");
    let pane = ui.interact(rect, pane_id, Sense::hover());
    pane.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text(key!("pane-glossary")))
    });
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
        ui.add_space(t.space_2);
        let mut filter = shell.glossary_filter().to_owned();
        let field = components::filter_field(ui, &mut filter, strings::text(key!("glossary-search")));
        if field.changed() {
            events.push(Event::Action("glossary.filter", json!({ "text": filter })));
        }
        // FT-1281: in the field, Up and Down move the row cursor and Enter runs the cursor row's first enabled action.
        if field.has_focus() {
            for (key, step) in [(egui::Key::ArrowDown, 1), (egui::Key::ArrowUp, -1)] {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                    events.push(Event::Action("glossary.row", json!({ "step": step })));
                }
            }
        }
        if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let rows = shell.glossary_rows();
            let first = shell
                .glossary_row()
                .and_then(|r| rows.get(r))
                .and_then(|r| r.actions.iter().filter_map(|a| actions::find(a)).find(|m| (m.enabled)(shell).is_ok()));
            if let Some(meta) = first {
                events.push(Event::Action(meta.id, Value::Null));
            }
        }
        ui.add_space(t.space_2);
        let rows = shell.glossary_rows();
        if rows.is_empty() {
            // Doc 16a §6.17, the navigators' form: the search quoted, and Clear.
            let line = strings::format(key!("empty-nav-filter"), &[shell.glossary_filter()]);
            let clear = components::empty_state(
                ui,
                IconId::search,
                &line,
                strings::text(key!("empty-nav-filter-text")),
                Some(strings::text(key!("empty-nav-clear"))),
            );
            if clear.is_some_and(|b| b.clicked()) {
                events.push(Event::Action("glossary.filter", json!({ "text": "" })));
            }
            return;
        }
        let width = ui.available_width();
        // The widest button's label in `type.body` (the Subtle button's font) with its padding, or the header's title.
        let pad = ui.spacing().button_padding.x;
        let measure = |text: String, font: &theme::TypeToken| {
            ui.painter()
                .layout_no_wrap(text, theme::font(font), t.text_primary.color32())
                .size()
                .x
        };
        let widest = rows
            .iter()
            .flat_map(|r| &r.actions)
            .filter_map(|a| action_words(a))
            .map(|(text, _)| measure(text, &t.type_body) + 2.0 * pad)
            .fold(
                measure(strings::text(key!("glossary-actions")).to_owned(), &t.type_small),
                f32::max,
            );
        let actions_w = (widest + 2.0 * t.space_4).ceil();
        // The longest word of each text column on show, in its font, with the cell padding.
        let strong = theme::TypeToken {
            semibold: true,
            ..t.type_small
        };
        let longest_word = |texts: &mut dyn Iterator<Item = &String>, font: &theme::TypeToken| {
            texts
                .flat_map(|s| s.split_whitespace())
                .map(|w| measure(w.to_owned(), font))
                .fold(0.0, f32::max)
                + 2.0 * t.space_4
        };
        let longest = [
            longest_word(&mut rows.iter().map(|r| &r.term), &strong),
            longest_word(&mut rows.iter().map(|r| &r.eplan), &t.type_small),
            longest_word(&mut rows.iter().map(|r| &r.qet), &t.type_small),
        ];
        let widths = column_widths(width, longest, actions_w);
        header(ui, &t, &widths);
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let cursor = shell.glossary_row();
                for (i, r) in rows.iter().enumerate() {
                    row(ui, &t, shell, r, &widths, cursor == Some(i), events);
                }
            });
    });
}

/// The left edges of the columns of `widths`, from `left`.
fn column_lefts(left: f32, widths: &[f32; 5]) -> [f32; 5] {
    let mut out = [left; 5];
    for i in 1..5 {
        out[i] = out[i - 1] + widths[i - 1];
    }
    out
}

/// The header row (doc 16a §6.7): `size.table_header` high on `color.surface.sunken`, the titles in `type.small`
/// SemiBold `color.text.secondary`, a 1-px `color.border.subtle` line under it.
fn header(ui: &mut Ui, t: &Tokens, widths: &[f32; 5]) {
    let titles = [
        key!("glossary-term"),
        key!("glossary-eplan"),
        key!("glossary-qet"),
        key!("glossary-definition"),
        key!("glossary-actions"),
    ];
    let font = theme::TypeToken {
        semibold: true,
        ..t.type_small
    };
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), t.size_table_header),
        Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::ZERO, t.surface_sunken.color32());
    painter.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        theme::stroke(t.border_width, t.border_subtle),
    );
    for ((title, w), x) in titles.iter().zip(widths).zip(column_lefts(rect.left(), widths)) {
        let galley = painter.layout(
            strings::text(*title).to_owned(),
            theme::font(&font),
            t.text_secondary.color32(),
            (w - 2.0 * t.space_4).max(0.0),
        );
        let y = rect.center().y - galley.size().y / 2.0;
        painter.galley(egui::pos2(x + t.space_4, y), galley, t.text_secondary.color32());
    }
}

/// One term's row: the texts wrapped in their columns, `space.2` above and below the tallest, at least `size.row`
/// high; the actions stacked in the last column. The cursor's row (FT-1281) has the hover overlay, as a navigator's
/// does, is selected for AccessKit and is scrolled into view.
fn row(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    r: &GlossaryRow,
    widths: &[f32; 5],
    cursor: bool,
    events: &mut Vec<Event>,
) {
    let strong = theme::TypeToken {
        semibold: true,
        ..t.type_small
    };
    let cells = [
        (&r.term, &strong, t.text_primary),
        (&r.eplan, &t.type_small, t.text_primary),
        (&r.qet, &t.type_small, t.text_primary),
        (&r.definition, &t.type_small, t.text_secondary),
    ];
    let galleys: Vec<_> = cells
        .iter()
        .zip(widths)
        .map(|((text, font, colour), w)| {
            ui.painter().layout(
                (*text).clone(),
                theme::font(font),
                colour.color32(),
                (w - 2.0 * t.space_4).max(0.0),
            )
        })
        .collect();
    let buttons = actions_known(&r.actions);
    let actions_h = buttons as f32 * t.size_control + buttons.saturating_sub(1) as f32 * t.space_1;
    let text_h = galleys.iter().map(|g| g.size().y).fold(0.0, f32::max);
    let height = (text_h.max(actions_h) + 2.0 * t.space_2).max(t.size_row);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let id = ui.scope_id().with(("glossary-row", &r.term));
    let response = ui.interact(rect, id, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &r.term));
    ui.ctx().accesskit_node_builder(id, |n| n.set_selected(cursor));
    if cursor {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::ZERO, t.state_hover.color32());
        response.scroll_to_me(None);
    }
    // The painted cells, for a screen reader: the row's description.
    let none = strings::text(key!("glossary-none"));
    let or_none = |text: &str| {
        if text.is_empty() {
            none.to_owned()
        } else {
            text.to_owned()
        }
    };
    let description = strings::format(
        key!("glossary-row-description"),
        &[&or_none(&r.eplan), &or_none(&r.qet), &r.definition],
    );
    ui.ctx()
        .accesskit_node_builder(id, |n| n.set_description(description));
    let lefts = column_lefts(rect.left(), widths);
    for ((galley, (_, _, colour)), x) in galleys.into_iter().zip(&cells).zip(lefts) {
        ui.painter().galley(
            egui::pos2(x + t.space_4, rect.top() + t.space_2),
            galley,
            colour.color32(),
        );
    }
    // The first button's label centred on the texts' first line, the labels under the column's title.
    let lift = ((t.size_control - t.type_small.line) / 2.0).min(t.space_2);
    let pad = ui.spacing().button_padding.x.min(t.space_4);
    let cell = egui::Rect::from_min_max(
        egui::pos2(lefts[4] + t.space_4 - pad, rect.top() + t.space_2 - lift),
        egui::pos2(rect.right(), rect.bottom()),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(cell)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .accessibility_parent(id),
    );
    child.spacing_mut().item_spacing.y = t.space_1;
    for a in &r.actions {
        action_button(&mut child, shell, a, events);
    }
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        theme::stroke(t.border_width, t.border_subtle),
    );
}

/// How many of `actions` this build knows (each is a button).
fn actions_known(actions: &[String]) -> usize {
    actions.iter().filter(|a| actions::find(a).is_some()).count()
}

/// The button of action `id` (none for an action this build does not know).
fn action_button(ui: &mut Ui, shell: &ShellModel, id: &str, events: &mut Vec<Event>) {
    let (Some(meta), Some((text, name))) = (actions::find(id), action_words(id)) else {
        return;
    };
    let state = (meta.enabled)(shell);
    let b = components::button(ui, Kind::Subtle, &text, state.is_ok(), 0.0);
    b.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, state.is_ok(), &name));
    let b = match state {
        Ok(()) => b.tip(name),
        Err(reason) => b.disabled_tip_beside(strings::text(reason)),
    };
    if b.clicked() {
        events.push(Event::Action(meta.id, Value::Null));
    }
}
