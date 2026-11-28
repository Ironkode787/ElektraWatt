// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Preview tab (FR-UI-022), restyled to doc 16a §6.23.
//!
//! **The tab** shows the open card: a header row with "Preview: `<command>`", "as of rev `<r>`" and the Stale or Failed
//! status badge at the left and Apply (Primary) and Cancel at the right, so the actions stay visible above long lists;
//! the summary line under it (records, tags, cross-reference texts, pages; an import's created and skipped counts);
//! then the lists **Entities, Tags, Xrefs, Pages** as collapsible sections with a chevron and a count badge, each
//! virtualised and at most `size.preview_list` rows high before it scrolls: the one that carries the change first and
//! open (Tags for a renumber, Entities otherwise), then the others closed, Xrefs and Pages last. List rows are
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
                // Left-aligned in the row (`add_sized` centred them).
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

/// The rows of the five lists: entities, tags, cross-reference texts, wire numbers, pages.
fn rows(t: &Tokens, shell: &ShellModel) -> [Vec<Row>; 5] {
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
            // The change word, the kind and the fields in the UI language, a content hash left out.
            let [change, kind, label, fields] = ew_viewmodel::preview::change_line(e);
            // A record said by its words alone (a wire's way) has no change word and no kind.
            let mut r: Vec<_> = [
                (format!("{change} "), t.text_tertiary),
                (format!("{kind} "), t.text_secondary),
            ]
            .into_iter()
            .filter(|(s, _)| !s.trim().is_empty())
            .chain([(label, t.text_primary)])
            .collect();
            if !fields.is_empty() {
                r.push((fields, t.text_tertiary));
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
    let wires = pane.wires().iter().map(|(a, b)| arrow(a, b)).collect();
    // A page whose folio changes as "106 → 104".
    let pages = pane
        .pages()
        .iter()
        .map(|(p, after)| match after {
            Some(a) => arrow(p, a),
            None => vec![(p.clone(), t.text_primary)],
        })
        .collect();
    [entities, tags, xrefs, wires, pages]
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
    for r in &p.changed {
        line(
            ui,
            &strings::format(key!("part-card-changed"), &[r]),
            t.text_secondary,
        );
    }
    // The parts kept in My parts with it are said before Apply writes them.
    if !p.kept.is_empty() {
        line(
            ui,
            &strings::format(key!("part-card-kept"), &[&p.kept.join(", ")]),
            t.text_primary,
        );
    }
    // A bus line at a socket the part no longer has keeps its end there: said in the primary colour.
    if !p.lost.is_empty() {
        line(
            ui,
            &strings::format(key!("part-card-lost"), &[&p.lost.join(", ")]),
            t.text_primary,
        );
    }
    // A box drawn anew is said in the primary colour: the page changes where the person may not look.
    for b in &p.boxes {
        line(ui, b, t.text_primary);
    }
    if p.fork {
        line(ui, strings::text(key!("part-edit-fork-note")), t.text_secondary);
    }
}

/// The paste choice card's copies ("FX-MCB-1PN (Fixture): this project 1.1.0, pasted
/// 1.0.0, patch"), one wrapped `type.small` label each, then its two choices as Secondary buttons, Use the project's
/// copy first (`paste.resolve`); a disabled choice keeps its place and says why in its tooltip and under the buttons.
fn paste_block(ui: &mut Ui, c: &ew_viewmodel::properties::PasteChoice, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    for row in &c.conflicts {
        let text = ew_viewmodel::properties::PasteChoice::conflict_text(row);
        let response =
            ui.add(egui::Label::new(crate::components::styled(&text, &t.type_small, t.text_primary)).wrap());
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
            let response = components::button(ui, kind, strings::text(key), reason.map(strings::text), 0.0);
            if response.clicked() {
                events.push(Event::Action(
                    "paste.resolve",
                    serde_json::json!({ "choice": word }),
                ));
            }
        }
    });
    for reason in [c.adapt, c.update].into_iter().flatten() {
        ui.label(crate::components::small_secondary(&t, strings::text(reason)));
    }
}

/// A macro card's value set (a chooser of "Defaults" and the macro's value sets, which previews
/// the card again with `macros.value_set`) and its conflicts, shown open and also on a refused preview (the rule's
/// text first), so the user decides on them before Apply.
/// The rows of an Arrange and route card (docs/design/06a-drafting.md §7.3): the page's figures now and then, the
/// symbols that move and the wires you drew, each with its tick (unticking plans again, `layout.arrange_tick`; the
/// heading of the wires you drew ticks or unticks them all), what is left as it is (a symbol kept in place with its
/// pin), the wires without a way, and the hint.
/// A cabinet's Route card (doc 12 §12.5): its sections, Ducts (one over the limit first), Lengths, Typed by hand, Made on
/// site and Without a way, each a small tertiary heading over its rows; a row hovered or clicked shows its way alone on
/// the canvas (a click keeps it); the tick Route every wire again under them.
fn cabinet_block(ui: &mut Ui, c: &ew_viewmodel::shell::cabinet::CabinetCard, events: &mut Vec<Event>) {
    let hovered = card_columns(ui, &c.columns(), events);
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::unique(CABINET_HOVER), hovered));
}

/// The width a column of a card's groups takes at least, in points: below it for each column the groups stand one
/// under the other.
const CARD_COLUMN_MIN: f32 = 260.0;

/// A card's groups in columns (doc 16a §6.9): side by side where the card has [`CARD_COLUMN_MIN`] points for each
/// column, so that a wider window gives each column more room, else one under the other. Returns the row under the
/// pointer, by what it is of.
fn card_columns(
    ui: &mut Ui,
    columns: &ew_viewmodel::properties::CardColumns,
    events: &mut Vec<Event>,
) -> Option<ew_viewmodel::ew_api::output::Uuid> {
    let t = components::tokens(ui);
    let n = columns.0.len().max(1);
    let mut hovered = None;
    if ui.available_width() >= CARD_COLUMN_MIN * n as f32 {
        ui.columns(n, |cols| {
            for (ui, groups) in cols.iter_mut().zip(&columns.0) {
                for g in groups {
                    card_group(ui, &t, g, events, &mut hovered);
                }
            }
        });
    } else {
        for g in columns.groups() {
            card_group(ui, &t, g, events, &mut hovered);
        }
    }
    hovered
}

/// One group of a card: its heading with its count, its rows (the first cell, the middle ones cut at the room left,
/// the last at the right; a line under a row in the secondary colour), and its tick.
fn card_group(
    ui: &mut Ui,
    t: &theme::Tokens,
    g: &ew_viewmodel::properties::CardGroup,
    events: &mut Vec<Event>,
    hovered: &mut Option<ew_viewmodel::ew_api::output::Uuid>,
) {
    if !g.heading.is_empty() {
        ui.add_space(t.space_2);
        ui.horizontal(|ui| {
            ui.label(components::small_tertiary(t, &g.heading));
            if let Some(n) = g.count {
                ui.label(components::small_secondary(t, n.to_string()));
            }
        });
    }
    for r in &g.rows {
        let colour = if r.picked { t.accent } else { t.text_primary };
        let text = |s: &str| components::styled(s, &t.type_small, colour);
        let response = ui
            .horizontal(|ui| {
                let (first, rest) = r
                    .cells
                    .split_first()
                    .map_or((None, &[][..]), |(f, r)| (Some(f), r));
                if let Some(f) = first {
                    ui.add(egui::Label::new(text(f)).extend());
                }
                let (middle, last) = match rest {
                    [] => (&[][..], None),
                    [one] => (&[][..], Some(one)),
                    [m @ .., l] => (m, Some(l)),
                };
                let last_w = last.map_or(0.0, |l| {
                    ui.painter()
                        .layout_no_wrap(l.clone(), theme::font(&t.type_small), colour.color32())
                        .size()
                        .x
                });
                if !middle.is_empty() {
                    let w = (ui.available_width() - last_w - t.space_2).max(0.0);
                    ui.allocate_ui(egui::vec2(w, ui.available_height()), |ui| {
                        ui.add(egui::Label::new(text(&middle.join(" · "))).truncate());
                    });
                }
                // The last cell keeps to the room the first leaves: a long reason ("its end -103F1:1 is on a device
                // without a location: …") is cut short, whole on hover, never drawn over the wire's number.
                if let Some(l) = last {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(egui::Label::new(text(l)).truncate());
                    });
                }
            })
            .response;
        if let Some(note) = &r.note {
            ui.add(egui::Label::new(components::small_secondary(t, note)).wrap());
        }
        let response = response.interact(egui::Sense::click());
        if response.hovered() {
            *hovered = r.id.or(*hovered);
        }
        if response.clicked()
            && let Some((action, params)) = &r.action
        {
            events.push(Event::Action(action, params.clone()));
        }
    }
    if let Some(tick) = &g.tick {
        ui.add_space(t.space_2);
        let mut on = tick.on;
        if ui
            .checkbox(&mut on, components::small_secondary(t, &tick.words))
            .changed()
        {
            let mut params = serde_json::Map::new();
            params.insert(tick.param.to_owned(), Value::Bool(on));
            events.push(Event::Action(tick.action, Value::Object(params)));
        }
    }
}

/// The key under which the Preview tab tells the canvas which row of a cabinet's Route card the pointer is on.
pub(crate) const CABINET_HOVER: &str = "cabinet-route-hover";

fn arrange_block(ui: &mut Ui, a: &ew_viewmodel::properties::ArrangeCard, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let small = |ui: &mut Ui, text: &str, colour: Rgba| {
        ui.add(
            egui::Label::new(
                RichText::new(text)
                    .font(theme::font(&t.type_small))
                    .color(colour.color32()),
            )
            .wrap(),
        );
    };
    small(ui, &a.now, t.text_secondary);
    small(ui, &a.then, t.text_primary);
    let heading = |ui: &mut Ui, key: ew_viewmodel::strings::Key<'static>| {
        ui.add_space(t.space_2);
        small(ui, strings::text(key), t.text_tertiary);
    };
    let mut row = 0;
    for (key, rows, wires) in [
        (key!("arrange-heading-parts"), &a.parts, false),
        (key!("arrange-heading-wires"), &a.wires, true),
    ] {
        if rows.is_empty() {
            continue;
        }
        if wires {
            ui.add_space(t.space_2);
            // Ticked when every row is, half when some are (a click ticks them all).
            let all = rows.iter().all(|r| r.ticked);
            let some = rows.iter().any(|r| r.ticked);
            let mut on = all;
            let label = crate::components::small_tertiary(&t, strings::text(key));
            if ui
                .add(egui::Checkbox::new(&mut on, label).indeterminate(some && !all))
                .changed()
            {
                events.push(Event::Action("layout.arrange_tick", json!({ "all": !all })));
            }
        } else {
            heading(ui, key);
        }
        for r in rows {
            ui.horizontal(|ui| {
                let mut on = r.ticked;
                if ui.checkbox(&mut on, RichText::new(&r.name).strong()).changed() {
                    events.push(Event::Action("layout.arrange_tick", json!({ "row": row })));
                }
                small(ui, &r.words, t.text_secondary);
            });
            row += 1;
        }
    }
    if !a.left.is_empty() || !a.ways.is_empty() {
        heading(ui, key!("arrange-heading-left"));
        for r in &a.left {
            ui.horizontal(|ui| {
                if r.kept {
                    let size = theme::font(&t.type_small).size;
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                    ui.painter().extend(crate::icons::shapes(
                        IconId::pin,
                        rect,
                        t.text_secondary.color32(),
                        1.0,
                    ));
                }
                small(ui, &r.name, t.text_primary);
                small(ui, &r.words, t.text_secondary);
            });
        }
        for w in &a.ways {
            small(ui, w, t.text_secondary);
        }
    }
    ui.add_space(t.space_2);
    small(ui, strings::text(key!("arrange-hint")), t.text_tertiary);
    small(ui, strings::text(key!("arrange-hold-b")), t.text_tertiary);
}

fn macro_block(ui: &mut Ui, m: &ew_viewmodel::properties::MacroInsert, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    if !m.value_sets.is_empty() {
        ui.horizontal(|ui| {
            let label = ui.label(crate::components::small_secondary(
                &t,
                strings::text(key!("macro-value-set")),
            ));
            let defaults = strings::text(key!("macro-value-set-defaults"));
            let shown = m.value_set.as_deref().unwrap_or(defaults);
            let combo = crate::components::choice(
                ui,
                egui::ComboBox::from_id_salt("macro-value-set").selected_text(shown),
                |ui| {
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
                },
            );
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
            let response =
                ui.add(egui::Label::new(crate::components::styled(c, &t.type_small, t.text_primary)).wrap());
            let text = c.clone();
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
        }
    }
}

/// A macro card's ad-hoc values, one row per variable: the label "<name> (<type in words>)" in `type.small`
/// `color.text.secondary` in a column as wide as the widest label (at most half the row), the field beside it as wide as a form dialog's field (the row is a form dialog's body,
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
    ui.label(crate::components::small_secondary(
        &t,
        strings::text(key!("macro-values")),
    ));
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
                    ui.add(egui::Label::new(crate::components::styled(error, &t.type_small, t.error)).wrap());
                });
            });
        }
    }
}

/// The macro card's value columns: the label column as wide as the widest label, at most half the
/// row; the field as wide as a form dialog's field, the rest of a form dialog's
/// body ([`crate::dialog::form_width`] less its margins, which grows with the window), never wider than the panel.
fn value_columns(t: &Tokens, widest: f32, window: f32, available: f32) -> (f32, f32) {
    let body = (crate::dialog::form_width(t, window) - 2.0 * t.space_5).min(available);
    let label = widest.min(body / 2.0);
    (label, (body - label - t.space_3).max(0.0))
}

/// The table import card in the Preview tab: the inline card titled "Import `<file>`" with
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

/// The strongest opacity of the accent behind a new card's header, a tint under its text and buttons.
const HIGHLIGHT_ALPHA: f32 = 0.18;

/// The opacity of the highlight behind `card`'s header this frame (so that a staged card does not go
/// unseen): a card the tab shows for the first time is highlighted once, fading out over the canvas's arrival time of
/// frame time ([`crate::canvas::arrival_opacity`]; still with reduced motion), then never again; `None` once it is over.
fn header_highlight(ui: &Ui, card: &ew_viewmodel::properties::Card, reduce_motion: bool) -> Option<f32> {
    let id = egui::Id::unique("ew-preview-highlight");
    let key = egui::Id::unique(ew_viewmodel::preview::card_key(card));
    let now = ui.input(|i| i.time);
    let start = ui.ctx().data_mut(|d| match d.get_temp::<(egui::Id, f64)>(id) {
        Some((k, start)) if k == key => start,
        _ => {
            d.insert_temp(id, (key, now));
            now
        }
    });
    let opacity = crate::canvas::arrival_opacity(now - start, reduce_motion)?;
    if reduce_motion {
        // A still highlight needs one more frame, when its moment is over.
        let left = crate::canvas::ARRIVAL_SECONDS - (now - start);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs_f64(left.max(0.0)));
    } else {
        ui.ctx().request_repaint();
    }
    Some(opacity)
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
        // The header row: title, revision and badge at the left; Apply and Cancel at the right. A new card's header is
        // highlighted once (`header_highlight`), under the row's content.
        ui.add_space(t.space_3);
        let highlight = ui.painter().add(egui::Shape::Noop);
        let header = ui.horizontal(|ui| {
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
            let response = ui.label(crate::components::body_strong(&t, &title));
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
                ui.label(crate::components::small_tertiary(
                    &t,
                    strings::format(key!("card-as-of"), &[&card.preview.base_rev.to_string()]),
                ));
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
                        None,
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
        if let Some(opacity) = header_highlight(ui, card, shell.reduce_motion()) {
            let mut fill = t.accent;
            fill.a = (f32::from(fill.a) * HIGHLIGHT_ALPHA * opacity).round() as u8;
            let rect = header.response.rect.expand2(egui::vec2(0.0, t.space_2));
            ui.painter().set(
                highlight,
                egui::Shape::rect_filled(rect, theme::radius(t.radius_md), fill.color32()),
            );
        }
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
                    ui.label(crate::components::small_secondary(
                        &t,
                        strings::format(key!("card-failed-rule"), &[&rule]),
                    ));
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
        ui.add_space(t.space_2);
        let [entities, tags, xrefs, wires, pages] = rows(&t, shell);
        // The command's notices (a re-address's old and new addresses), first and open where it gives any.
        let notices: Vec<Vec<(String, Rgba)>> = shell
            .preview_pane()
            .notices()
            .iter()
            .map(|n| match &n.change {
                Some((a, b)) => vec![
                    (a.clone(), t.text_secondary),
                    (" → ".to_owned(), t.text_tertiary),
                    (b.clone(), t.text_primary),
                ],
                None => vec![(n.words.clone(), t.text_primary)],
            })
            .collect();
        let shown = egui::ScrollArea::vertical()
            .id_salt("preview-lists")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // An Arrange and route card's figures and rows scroll with the lists (wheel and bar), so that the
                // pane keeps its height and leaves the page in view.
                if let CardOrigin::Arrange(a) = &card.origin {
                    inset(ui, &mut |ui| arrange_block(ui, a, events));
                    ui.add_space(t.space_2);
                }
                if let CardOrigin::Cabinet(c) = &card.origin {
                    inset(ui, &mut |ui| cabinet_block(ui, c, events));
                    ui.add_space(t.space_2);
                }
                // A route card's connections left unrouted, one row each with its reason.
                if let CardOrigin::Route { rows, .. } = &card.origin {
                    for row in rows {
                        inset(ui, &mut |ui| {
                            ui.label(crate::components::small_secondary(&t, row));
                        });
                    }
                    ui.add_space(t.space_2);
                }
                // The list that carries the change comes first and open (Tags for a renumber, the changed
                // entities otherwise); the others follow, closed.
                let renumber = card.tags_first() && !tags.is_empty();
                let mut closed = 0.0;
                if !notices.is_empty() {
                    closed += list(ui, strings::text(key!("preview-notices")), &notices, true);
                    closed += list(ui, strings::text(key!("preview-entities")), &entities, false);
                    closed += list(ui, strings::text(key!("preview-tags")), &tags, false);
                } else if renumber {
                    closed += list(ui, strings::text(key!("preview-tags")), &tags, true);
                    closed += list(ui, strings::text(key!("preview-entities")), &entities, false);
                } else {
                    closed += list(ui, strings::text(key!("preview-entities")), &entities, true);
                    closed += list(ui, strings::text(key!("preview-tags")), &tags, false);
                }
                closed += list(ui, strings::text(key!("preview-xrefs")), &xrefs, false);
                closed += list(ui, strings::text(key!("preview-wires")), &wires, false);
                closed += list(ui, strings::text(key!("preview-pages")), &pages, false);
                closed
            });
        // While the lists (doc 16a §6.23) are scrolled, a 1-px `color.border.subtle` edge at the
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

    /// The field is a form dialog's field: it grows with the window, never fills a wide panel, and the
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
