// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Preview tab (WP-S2.09 part 3, §8.1, D11; FR-UI-022), restyled by WP-S2.17 part 4 (doc 16a §6.23).
//!
//! **The tab** shows the open card: a header row with "Preview: `<command>`", "as of rev `<r>`" and the Stale or Failed
//! status badge at the left and Apply (Primary) and Cancel at the right, so the actions stay visible above long lists;
//! the summary line under it (records, tags, cross-reference texts, pages; an import's created and skipped counts);
//! then the lists **Entities, Tags, Xrefs, Pages** as collapsible sections with a chevron and a count badge, each
//! virtualised and at most `size.preview_list` rows high before it scrolls: the one that carries the change first and
//! open (Tags for a renumber, Entities otherwise), then the others closed (W31-2), Xrefs and Pages last. List rows are
//! `size.row` in `type.small`: an entity row shows the change word in `color.text.tertiary` in a 64-px column, the
//! kind in `color.text.secondary`, the label in `color.text.primary` and the changed fields in tertiary; a tag or
//! cross-reference row "old → new" with the arrow in tertiary, a cross-reference with its folio first. Without a card
//! the tab shows its empty state ("No preview is open"). The lists are the shell's ([`ShellModel::preview_pane`]).
//!
//! **The strip** that stands in for an inline card while the tab is open is [`crate::components::card_strip`].
//!
//! **Accessibility.** The tab is a `Group` named "Preview"; the title is a polite live region whose value is the header
//! ("Preview: `<command>` · as of rev `<r>`", with "stale" when it is); the buttons are named by their text.

use egui::text::LayoutJob;
use egui::{Align, Layout, RichText, Ui};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::Value;

use crate::app::Event;
use crate::components::{self, CardStatus};
use crate::icons::IconId;
use crate::theme::{self, Rgba, Tokens};

/// One list row: its segments, each with its colour.
type Row = Vec<(String, Rgba)>;

/// The text of a row (its accessible name).
fn row_text(row: &Row) -> String {
    row.iter().map(|(s, _)| s.as_str()).collect()
}

/// The rows of one list in a collapsible section titled `title` with a count badge, open at first when `open`; at most
/// `size.preview_list` rows on show, then it scrolls.
fn list(ui: &mut Ui, title: &str, rows: &[Row], open: bool) {
    if rows.is_empty() {
        return;
    }
    let id = ui.scope_id().with(("preview-list", title));
    if !components::section_header(ui, id, title, rows.len(), open) {
        return;
    }
    let t = components::tokens(ui);
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
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add_space(t.space_4);
                        let mut job = LayoutJob::default();
                        for (s, c) in r {
                            job.append(
                                s,
                                0.0,
                                egui::TextFormat::simple(theme::font(&t.type_small), c.color32()),
                            );
                        }
                        let text = row_text(r);
                        let response = ui.add(egui::Label::new(job).truncate());
                        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
                    },
                );
            }
        });
}

/// The rows of the four lists: entities, tags, cross-reference texts, pages.
fn rows(t: &Tokens, shell: &ShellModel) -> [Vec<Row>; 4] {
    let pane = shell.preview_pane();
    let arrow = |before: &str, after: &str| -> Row {
        vec![
            (before.to_owned(), t.text_primary),
            (" → ".to_owned(), t.text_tertiary),
            (after.to_owned(), t.text_primary),
        ]
    };
    let entities = pane
        .entities()
        .iter()
        .map(|e| {
            // The change word in a 64-px column (padded to its width in the row's font).
            let mut r = vec![
                (format!("{} ", e.change), t.text_tertiary),
                (format!("{} ", e.kind), t.text_secondary),
                (e.label.clone(), t.text_primary),
            ];
            if !e.fields.is_empty() {
                r.push((format!(" ({})", e.fields.join(", ")), t.text_tertiary));
            }
            r
        })
        .collect();
    let tags = pane.tags().iter().map(|(a, b)| arrow(a, b)).collect();
    let xrefs = pane
        .xrefs()
        .iter()
        .map(|x| {
            let mut r = vec![(format!("{}: ", x.folio), t.text_secondary)];
            r.extend(arrow(&x.before, &x.after));
            r
        })
        .collect();
    let pages = pane
        .pages()
        .iter()
        .map(|p| vec![(p.clone(), t.text_primary)])
        .collect();
    [entities, tags, xrefs, pages]
}

/// WP-S1.19 part 6 (§11): a macro card's value set (a chooser of "Defaults" and the macro's value sets, which previews
/// the card again with `macros.value_set`) and its conflicts, shown open and also on a refused preview (the rule's
/// text first), so the user decides on them before Apply.
fn macro_block(ui: &mut Ui, m: &ew_viewmodel::properties::MacroInsert, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    if !m.value_sets.is_empty() {
        ui.horizontal(|ui| {
            let label = ui.label(
                RichText::new(strings::text("macro-value-set"))
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            );
            let defaults = strings::text("macro-value-set-defaults");
            let shown = m.value_set.as_deref().unwrap_or(defaults);
            let combo = egui::ComboBox::from_id_salt("macro-value-set")
                .selected_text(shown)
                .icon(components::combo_chevron)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(m.value_set.is_none(), defaults).clicked() {
                        events.push(Event::Action("macros.value_set", serde_json::json!({})));
                    }
                    for v in &m.value_sets {
                        if ui.selectable_label(m.value_set.as_ref() == Some(v), v).clicked() {
                            events.push(Event::Action(
                                "macros.value_set",
                                serde_json::json!({ "name": v }),
                            ));
                        }
                    }
                });
            combo.response.labelled_by(label.id);
        });
    }
    if m.conflicts.is_empty() {
        components::live_line(ui, strings::text("macro-no-conflicts"), false);
        return;
    }
    let id = ui.scope_id().with("macro-conflicts");
    if components::section_header(ui, id, strings::text("macro-conflicts"), m.conflicts.len(), true) {
        for c in &m.conflicts {
            let response = ui.add(
                egui::Label::new(
                    RichText::new(c)
                        .font(theme::font(&t.type_small))
                        .color(t.text_primary.color32()),
                )
                .wrap(),
            );
            let text = c.clone();
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
        }
    }
}

/// Paints the Preview tab of `shell`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let rect = ui.available_rect_before_wrap();
    let pane_id = ui.scope_id().with("preview-pane");
    let pane = ui.interact(rect, pane_id, egui::Sense::hover());
    pane.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text("pane-preview")));
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
        let t = components::tokens(ui);
        let Some(card) = shell.properties().card() else {
            components::empty_state(
                ui,
                IconId::eye,
                strings::text("preview-none"),
                strings::text("empty-preview-text"),
                None,
            );
            return;
        };
        let status = components::card_status(shell);

        let pad = t.space_4;
        // The header row: title, revision and badge at the left; Apply and Cancel at the right.
        ui.add_space(t.space_3);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.spacing_mut().item_spacing.x = t.space_3;
            let command = strings::command_title(&card.preview.name);
            let title = strings::format("preview-title", &[command]);
            let response = ui.label(
                RichText::new(&title)
                    .font(theme::font(&t.type_body_strong))
                    .color(t.text_primary.color32()),
            );
            let mut header =
                strings::format("preview-header", &[command, &card.preview.base_rev.to_string()]);
            if status == CardStatus::Stale {
                header.push_str(&format!(" [{}]", strings::text("preview-stale")));
            }
            ui.ctx().accesskit_node_builder(response.id, |n| {
                n.set_live(egui::accesskit::Live::Polite);
                n.set_value(header);
            });
            ui.label(
                RichText::new(strings::format(
                    "card-as-of",
                    &[&card.preview.base_rev.to_string()],
                ))
                .font(theme::font(&t.type_small))
                .color(t.text_tertiary.color32()),
            );
            match &status {
                CardStatus::Stale => {
                    components::status_badge(ui, components::Badge::Stale);
                }
                CardStatus::Failed(_) => {
                    components::status_badge(ui, components::Badge::Failed);
                }
                CardStatus::Fresh => {}
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(pad);
                let blocked = card.preview.result.as_ref().err().map(String::as_str);
                components::apply_cancel(
                    ui,
                    components::apply_action(&card.origin),
                    Value::Null,
                    blocked,
                    events,
                );
            });
        });
        let inset = |ui: &mut Ui, f: &mut dyn FnMut(&mut Ui)| {
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| f(ui));
            });
        };
        if let CardStatus::Failed(m) = &status {
            inset(ui, &mut |ui| {
                components::live_line(ui, &strings::format("card-failed", &[m]), true)
            });
            if let CardOrigin::Macro(m) = &card.origin {
                inset(ui, &mut |ui| macro_block(ui, m, events));
            }
            return;
        }
        // FT-580: one summary line, shared with the text interface.
        let summary = shell.preview_summary().unwrap_or_default();
        // FT-719: an import's counts are the toast's sentence; the tab shows the summary and the lists only.
        inset(ui, &mut |ui| components::live_line(ui, &summary, false));
        if let CardOrigin::Macro(m) = &card.origin {
            inset(ui, &mut |ui| macro_block(ui, m, events));
        }
        ui.add_space(t.space_2);
        let [entities, tags, xrefs, pages] = rows(&t, shell);
        egui::ScrollArea::vertical()
            .id_salt("preview-lists")
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
                list(ui, strings::text("preview-pages"), &pages, false);
            });
    });
}
