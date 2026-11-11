// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Preview tab (D11; FR-UI-022), restyled to doc 16a §6.23.
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
use ew_viewmodel::key;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::{Value, json};

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
/// `size.preview_list` rows on show, then it scrolls. Returns the height its rows would take while it is closed (0
/// while open).
fn list(ui: &mut Ui, title: &str, rows: &[Row], open: bool) -> f32 {
    if rows.is_empty() {
        return 0.0;
    }
    let t = components::tokens(ui);
    let row = t.size_row;
    let id = ui.scope_id().with(("preview-list", title));
    if !components::section_header(ui, id, title, Some(rows.len()), open) {
        return row * rows.len().min(t.size_preview_list_rows as usize) as f32;
    }
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
    0.0
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
            // The change word, the kind and the fields in the UI language.
            let (change, kind) = ew_viewmodel::preview::entity_words(e);
            let mut r = vec![
                (format!("{change} "), t.text_tertiary),
                (format!("{kind} "), t.text_secondary),
                (e.label.clone(), t.text_primary),
            ];
            // The fields in words, a content hash left out (`field_words`).
            let fields = ew_viewmodel::preview::field_words(&e.kind, &e.fields);
            if !fields.is_empty() {
                r.push((format!(" ({})", fields.join(", ")), t.text_tertiary));
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
    // A page whose folio changes as "106 → 104".
    let pages = pane
        .pages()
        .iter()
        .map(|(p, after)| match after {
            Some(a) => arrow(p, a),
            None => vec![(p.clone(), t.text_primary)],
        })
        .collect();
    [entities, tags, xrefs, pages]
}

/// A part card's field diff ("Description: old → new"), the devices that use the copy, the
/// parts list rows that go and come, and a fork's note, one wrapped `type.small` label each.
fn part_block(ui: &mut Ui, p: &ew_viewmodel::properties::PartCard) {
    let t = components::tokens(ui);
    let line = |ui: &mut Ui, text: &str, colour: Rgba| {
        ui.add(
            egui::Label::new(
                RichText::new(text)
                    .font(theme::font(&t.type_small))
                    .color(colour.color32()),
            )
            .wrap(),
        );
    };
    for (member, old, new) in &p.fields {
        line(
            ui,
            &strings::format(key!("part-card-field"), &[member, old, new]),
            t.text_primary,
        );
    }
    if !p.devices.is_empty() {
        line(
            ui,
            &strings::format(key!("part-card-devices"), &[&p.devices.join(", ")]),
            t.text_secondary,
        );
    }
    for r in &p.removed {
        line(
            ui,
            &strings::format(key!("part-card-removed"), &[r]),
            t.text_secondary,
        );
    }
    for r in &p.added {
        line(
            ui,
            &strings::format(key!("part-card-added"), &[r]),
            t.text_secondary,
        );
    }
    if p.fork {
        line(ui, strings::text(key!("part-edit-fork-note")), t.text_secondary);
    }
}

/// FT-396: the paste choice card's copies ("FX-MCB-1PN (Fixture): this project 1.1.0, pasted
/// 1.0.0, patch"), one wrapped `type.small` label each, then its two choices as Secondary buttons, Use the project's
/// copy first (`paste.resolve`); a disabled choice keeps its place and says why in its tooltip and under the buttons.
fn paste_block(ui: &mut Ui, c: &ew_viewmodel::properties::PasteChoice, events: &mut Vec<Event>) {
    use components::Tip;
    let t = components::tokens(ui);
    for row in &c.conflicts {
        let text = ew_viewmodel::properties::PasteChoice::conflict_text(row);
        let response = ui.add(
            egui::Label::new(
                RichText::new(&text)
                    .font(theme::font(&t.type_small))
                    .color(t.text_primary.color32()),
            )
            .wrap(),
        );
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
    }
    ui.add_space(t.space_2);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_2;
        // The first enabled choice is the Primary button: Use the project's copy when it is
        // enabled (it changes no library copy and is one undo step), else Update the project's copy.
        let primary = if c.adapt.is_none() { "adapt" } else { "update" };
        for (word, key, reason) in [
            ("adapt", key!("paste-adapt"), c.adapt),
            ("update", key!("paste-update"), c.update),
        ] {
            let kind = if word == primary && reason.is_none() {
                components::Kind::Primary
            } else {
                components::Kind::Secondary
            };
            let response = components::button(ui, kind, strings::text(key), reason.is_none(), 0.0);
            let response = match reason {
                Some(r) => response.disabled_tip(strings::text(r)),
                None => response,
            };
            if response.clicked() {
                events.push(Event::Action(
                    "paste.resolve",
                    serde_json::json!({ "choice": word }),
                ));
            }
        }
    });
    for reason in [c.adapt, c.update].into_iter().flatten() {
        ui.label(
            RichText::new(strings::text(reason))
                .font(theme::font(&t.type_small))
                .color(t.text_secondary.color32()),
        );
    }
}

/// A macro card's value set (a chooser of "Defaults" and the macro's value sets, which previews
/// the card again with `macros.value_set`) and its conflicts, shown open and also on a refused preview (the rule's
/// text first), so the user decides on them before Apply.
fn macro_block(ui: &mut Ui, m: &ew_viewmodel::properties::MacroInsert, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    if !m.value_sets.is_empty() {
        ui.horizontal(|ui| {
            let label = ui.label(
                RichText::new(strings::text(key!("macro-value-set")))
                    .font(theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            );
            let defaults = strings::text(key!("macro-value-set-defaults"));
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
            components::combo_expanded(ui, &combo.response);
            combo.response.labelled_by(label.id);
        });
    }
    macro_values(ui, m, events);
    if m.conflicts.is_empty() {
        components::live_line(ui, strings::text(key!("macro-no-conflicts")), false);
        return;
    }
    let id = ui.scope_id().with("macro-conflicts");
    if components::section_header(
        ui,
        id,
        strings::text(key!("macro-conflicts")),
        Some(m.conflicts.len()),
        true,
    ) {
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

/// A macro card's ad-hoc values, one row per variable: the label "<name> (<type in words>)" in `type.small`
/// `color.text.secondary` in a column as wide as the widest label (at most half the row), the field beside it as wide as a form dialog's field (W40a item 4: the row is a form dialog's body,
/// [`crate::dialog::form_width`], which grows with the window; never the whole preview panel), with the value set's
/// (or the default's) value as its placeholder, and the command's refusal of a typed value under the field at the
/// field's width in `color.error`. The field keeps what is typed while it has the focus and sends `macros.value` when
/// it loses it with another text (the card is previewed again; a blank field sends none, so the placeholder's value
/// applies).
fn macro_values(ui: &mut Ui, m: &ew_viewmodel::properties::MacroInsert, events: &mut Vec<Event>) {
    if m.fields.is_empty() {
        return;
    }
    let t = components::tokens(ui);
    ui.label(
        RichText::new(strings::text(key!("macro-values")))
            .font(theme::font(&t.type_small))
            .color(t.text_secondary.color32()),
    );
    let font = theme::font(&t.type_small);
    let labels: Vec<String> = m
        .fields
        .iter()
        .map(|f| {
            strings::format(
                key!("macro-value-label"),
                &[&f.name, &strings::var_type_word(&f.ty)],
            )
        })
        .collect();
    let widest = labels
        .iter()
        .map(|l| {
            ui.fonts_mut(|fonts| {
                fonts
                    .layout_no_wrap(l.clone(), font.clone(), theme::NONE)
                    .size()
                    .x
            })
        })
        .fold(0.0_f32, f32::max)
        .ceil();
    let window = ui.ctx().content_rect().width();
    let (label_width, field_width) = value_columns(&t, widest, window, ui.available_width());
    for (f, label_text) in m.fields.iter().zip(&labels) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.space_3;
            let label = ui
                .allocate_ui_with_layout(
                    egui::vec2(label_width, t.size_control),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_min_size(egui::vec2(label_width, t.size_control));
                        ui.add(
                            egui::Label::new(
                                RichText::new(label_text)
                                    .font(font.clone())
                                    .color(t.text_secondary.color32()),
                            )
                            .wrap(),
                        )
                    },
                )
                .inner;
            let id = ui.scope_id().with(("macro-value", m.instance, &f.name));
            let focused = ui.memory(|mem| mem.has_focus(id));
            let mut text = if focused {
                ui.data(|d| d.get_temp::<String>(id))
                    .unwrap_or_else(|| f.value.clone())
            } else {
                f.value.clone()
            };
            let edit = ui
                .add(
                    egui::TextEdit::singleline(&mut text)
                        .id(id)
                        .hint_text(RichText::new(&f.placeholder).color(t.text_tertiary.color32()))
                        .desired_width(field_width)
                        .min_size(egui::vec2(0.0, t.size_control)),
                )
                .labelled_by(label.id);
            if edit.changed() {
                ui.data_mut(|d| d.insert_temp(id, text.clone()));
            }
            if edit.lost_focus() {
                ui.data_mut(|d| d.remove::<String>(id));
                if text.trim() != f.value {
                    events.push(Event::Action(
                        "macros.value",
                        serde_json::json!({ "name": f.name, "value": text.trim() }),
                    ));
                }
            }
        });
        if let Some(error) = &f.error {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(label_width + t.space_3);
                ui.vertical(|ui| {
                    ui.set_max_width(field_width);
                    ui.add(
                        egui::Label::new(
                            RichText::new(error)
                                .font(theme::font(&t.type_small))
                                .color(t.error.color32()),
                        )
                        .wrap(),
                    );
                });
            });
        }
    }
}

/// The macro card's value columns (W40a item 4): the label column as wide as the widest label, at most half the
/// row; the field as wide as a form dialog's field, the rest of a form dialog's
/// body ([`crate::dialog::form_width`] less its margins, which grows with the window), never wider than the panel.
fn value_columns(t: &Tokens, widest: f32, window: f32, available: f32) -> (f32, f32) {
    let body = (crate::dialog::form_width(t, window) - 2.0 * t.space_5).min(available);
    let label = widest.min(body / 2.0);
    (label, (body - label - t.space_3).max(0.0))
}

/// The table import card in the Preview tab (D14): the inline card titled "Import `<file>`" with
/// the counts as its summary ("5 changed, 0 kept, 1 conflict: the project's value is kept, …"), then the file's cells
/// ("-X1031:1 · Function text · "Valve" → "Valve 1" · applied") and the table's rows the file lacks, each a list of
/// its own; Apply commits the import's one command, Cancel drops it.
fn table_import(
    ui: &mut Ui,
    shell: &ShellModel,
    c: &ew_viewmodel::properties::TableImportCard,
    events: &mut Vec<Event>,
) {
    let t = components::tokens(ui);
    let title = strings::format(key!("table-import-title"), &[&c.source]);
    let summary = c.counts_line();
    let row = |text: String| vec![(text, t.text_primary)];
    let cells: Vec<Row> = c
        .import
        .cells
        .iter()
        .map(|cell| row(ew_viewmodel::properties::TableImportCard::cell_line(cell)))
        .collect();
    let missing: Vec<Row> = c.import.missing.iter().map(|m| row(m.clone())).collect();
    let card = components::InlineCard {
        title: &title,
        summary: &summary,
        apply: "preview.apply",
        strip: false,
    };
    // The card keeps its Apply and Cancel in view; the lists under it scroll, as the other cards' lists do.
    ui.add_space(t.space_3);
    let pad = theme::margin_px(t.space_4);
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: pad,
            right: pad,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            // With a conflict the switch "Take the file's values" reads the file again.
            let switch = c.import.counts().conflicts > 0 || c.take_file;
            components::inline_card(ui, shell, card, events, |ui, events| {
                let mut on = c.take_file;
                if switch
                    && ui
                        .checkbox(&mut on, strings::text(key!("action-preview-take-file")))
                        .changed()
                {
                    events.push(Event::Action("preview.take_file", json!({ "on": on })));
                }
            })
        });
    ui.add_space(t.space_2);
    let shown = egui::ScrollArea::vertical()
        .id_salt("table-import-lists")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            list(ui, strings::text(key!("table-import-cells")), &cells, true);
            list(
                ui,
                strings::text(key!("table-import-missing-rows")),
                &missing,
                false,
            );
        });
    report(ui, &shown, 0.0);
}

/// The dock follows the Preview tab's lists; `closed` is the height the closed lists' rows would add.
fn report<R>(ui: &Ui, shown: &egui::scroll_area::ScrollAreaOutput<R>, closed: f32) {
    crate::dock::report_content(
        ui,
        ew_viewmodel::layout::PaneKind::Preview,
        shown.inner_rect,
        shown.content_size.y + closed,
    );
}

/// Paints the Preview tab of `shell`.
pub fn show(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let rect = ui.available_rect_before_wrap();
    let pane_id = ui.scope_id().with("preview-pane");
    let pane = ui.interact(rect, pane_id, egui::Sense::hover());
    pane.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Group, true, strings::text(key!("pane-preview")))
    });
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(pane_id), |ui| {
        let t = components::tokens(ui);
        let Some(card) = shell.properties().card() else {
            components::empty_state(
                ui,
                IconId::eye,
                strings::text(key!("preview-none")),
                strings::text(key!("empty-preview-text")),
                None,
            );
            return;
        };
        // A table import is the inline card with its counts and its cells.
        if let CardOrigin::TableImport(c) = &card.origin {
            table_import(ui, shell, c, events);
            return;
        }
        let status = components::card_status(shell);
        let choice = matches!(card.origin, CardOrigin::PasteVersions(_));

        let pad = t.space_4;
        // The header row: title, revision and badge at the left; Apply and Cancel at the right.
        ui.add_space(t.space_3);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.spacing_mut().item_spacing.x = t.space_3;
            let command = card.command();
            // A question card (the paste choice) previews nothing: its head is the command alone,
            // with no "Preview:" and no revision.
            let title = if choice {
                command.to_owned()
            } else {
                strings::format(key!("preview-title"), &[command])
            };
            let response = ui.label(
                RichText::new(&title)
                    .font(theme::font(&t.type_body_strong))
                    .color(t.text_primary.color32()),
            );
            let mut header = if choice {
                title.clone()
            } else {
                strings::format(
                    key!("preview-header"),
                    &[command, &card.preview.base_rev.to_string()],
                )
            };
            if status == CardStatus::Stale {
                header.push_str(&format!(" [{}]", strings::text(key!("preview-stale"))));
            }
            ui.ctx().accesskit_node_builder(response.id, |n| {
                n.set_live(egui::accesskit::Live::Polite);
                n.set_value(header);
            });
            if !choice {
                ui.label(
                    RichText::new(strings::format(
                        key!("card-as-of"),
                        &[&card.preview.base_rev.to_string()],
                    ))
                    .font(theme::font(&t.type_small))
                    .color(t.text_tertiary.color32()),
                );
            }
            match &status {
                CardStatus::Stale => {
                    components::status_badge(ui, components::Badge::Stale);
                }
                // The paste choice card asks; it has not failed, so it wears no Failed badge.
                CardStatus::Failed(_) if !choice => {
                    components::status_badge(ui, components::Badge::Failed);
                }
                CardStatus::Failed(_) => {}
                CardStatus::Fresh => {}
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(pad);
                // A question card's answers are its choices; the head keeps Cancel only, with
                // no Apply that could never run.
                if choice {
                    let cancel = components::button(
                        ui,
                        components::Kind::Secondary,
                        strings::text(key!("card-cancel")),
                        true,
                        0.0,
                    );
                    if cancel.clicked() {
                        events.push(Event::Action("preview.cancel", serde_json::json!({})));
                    }
                    return;
                }
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
            // The lead in words (a macro card: what blocks the apply); the rule code follows, secondary.
            let lead = card.failed_lead();
            let text = lead
                .as_ref()
                .map_or_else(|| strings::format(key!("card-failed"), &[m]), |l| l.text.clone());
            inset(ui, &mut |ui| components::live_line(ui, &text, !choice));
            if let Some(rule) = lead.and_then(|l| l.rule) {
                inset(ui, &mut |ui| {
                    ui.label(
                        RichText::new(strings::format(key!("card-failed-rule"), &[&rule]))
                            .font(theme::font(&t.type_small))
                            .color(t.text_secondary.color32()),
                    );
                });
            }
            if let CardOrigin::Macro(m) = &card.origin {
                inset(ui, &mut |ui| macro_block(ui, m, events));
            }
            if let CardOrigin::PasteVersions(c) = &card.origin {
                inset(ui, &mut |ui| paste_block(ui, c, events));
            }
            return;
        }
        // One summary line, shared with the text interface.
        let summary = shell.preview_summary().unwrap_or_default();
        // An import's counts are the toast's sentence; the tab shows the summary and the lists only.
        inset(ui, &mut |ui| components::live_line(ui, &summary, false));
        if let CardOrigin::Macro(m) = &card.origin {
            inset(ui, &mut |ui| macro_block(ui, m, events));
        }
        if let CardOrigin::Part(p) = &card.origin {
            inset(ui, &mut |ui| part_block(ui, p));
        }
        // A route card's connections left unrouted, one row each with its reason.
        if let CardOrigin::Route { rows, .. } = &card.origin {
            for row in rows {
                inset(ui, &mut |ui| {
                    ui.label(
                        RichText::new(row)
                            .font(theme::font(&t.type_small))
                            .color(t.text_secondary.color32()),
                    );
                });
            }
        }
        ui.add_space(t.space_2);
        let [entities, tags, xrefs, pages] = rows(&t, shell);
        let shown = egui::ScrollArea::vertical()
            .id_salt("preview-lists")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // W31-2: the list that carries the change comes first and open (Tags for a renumber, the changed
                // entities otherwise); the others follow, closed.
                let renumber = card.preview.name.ends_with("renumber") && !tags.is_empty();
                let mut closed = 0.0;
                if renumber {
                    closed += list(ui, strings::text(key!("preview-tags")), &tags, true);
                    closed += list(ui, strings::text(key!("preview-entities")), &entities, false);
                } else {
                    closed += list(ui, strings::text(key!("preview-entities")), &entities, true);
                    closed += list(ui, strings::text(key!("preview-tags")), &tags, false);
                }
                closed += list(ui, strings::text(key!("preview-xrefs")), &xrefs, false);
                closed += list(ui, strings::text(key!("preview-pages")), &pages, false);
                closed
            });
        // Window check F2 (doc 16a §6.23, P2): while the lists are scrolled, a 1-px `color.border.subtle` edge at the
        // top of their area parts the rows passing under it from the fixed summary line.
        if shown.state.clamped_offset().y > 0.0 {
            ui.painter().hline(
                shown.inner_rect.x_range(),
                shown.inner_rect.top() + t.border_width / 2.0,
                theme::stroke(t.border_width, t.border_subtle),
            );
        }
        // The dock is sized for every list open, so that a list opened or closed keeps the headers where
        // they are (a click on the next header hit the one before, which the grown dock had moved there); past the
        // dock's room the lists scroll.
        report(ui, &shown, shown.inner);
    });
}

#[cfg(test)]
mod value_column_tests {
    use super::*;

    /// W40a item 4: the field is a form dialog's field: it grows with the window, never fills a wide panel, and the
    /// label column follows its widest label.
    #[test]
    fn the_value_field_is_as_wide_as_a_form_field() {
        let t = crate::theme::tokens(crate::theme::Theme::Light, crate::theme::Density::Comfortable);
        let (l_floor, f_floor) = value_columns(&t, 40.0, 1280.0, 1160.0);
        let (l_wide, f_wide) = value_columns(&t, 40.0, 1920.0, 1160.0);
        assert_eq!(l_floor, 40.0);
        assert_eq!(l_wide, 40.0);
        assert!(
            f_wide > f_floor,
            "the field grows with the window: {f_floor} → {f_wide}"
        );
        assert!(f_wide + l_wide < 1160.0 * 0.8, "not the whole panel: {f_wide}");
        // A long label widens its column; a narrow panel bounds the row.
        let (l_long, _) = value_columns(&t, t.size_dialog_label + 60.0, 1920.0, 1160.0);
        assert_eq!(l_long, t.size_dialog_label + 60.0);
        let (l_narrow, f_narrow) = value_columns(&t, 40.0, 1920.0, 300.0);
        assert!(l_narrow + t.space_3 + f_narrow <= 300.0 + 0.01);
    }
}
