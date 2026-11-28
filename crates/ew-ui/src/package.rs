// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Package… dialog's own content (doc 13-package §8.5; doc 16a §6.12): two panes. Left, What goes in: a row per
//! member with its tick, its words, what it holds and its form, Change… on the label files' row, and a line for what a
//! member lacks. Right, always in view: Before it goes to the workshop (the release check's line, every finding with
//! Show and its fix, Open Messages), the revision with its chip, Without personal data. Below the panes: the file with
//! Choose…. Write (draft) package and Cancel are the dialog's buttons.

use egui::Ui;
use ew_viewmodel::ew_api::PackageMember;
use ew_viewmodel::key;
use ew_viewmodel::shell::package::{PackageDialog, PackageFinding};
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components;
use crate::theme::Tokens;

/// The left pane's share of the panes' width.
const LEFT_SHARE: f32 = 0.66;

/// The dialog's content.
pub(crate) fn body(ui: &mut Ui, d: &PackageDialog, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let gap = t.space_4;
    let width = ui.available_width();
    let left = ((width - gap) * LEFT_SHARE).floor();
    let right = width - gap - left;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.allocate_ui_with_layout(
            egui::vec2(left, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(left);
                components::card_frame(&t).show(ui, |ui| goes_in(ui, &t, d, events));
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(right, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(right);
                ui.spacing_mut().item_spacing.y = t.space_3;
                components::card_frame(&t).show(ui, |ui| before(ui, &t, d, events));
                components::card_frame(&t).show(ui, |ui| revision(ui, &t, d));
                components::card_frame(&t).show(ui, |ui| personal(ui, &t, d, events));
            },
        );
    });
    ui.add_space(t.space_3);
    ui.horizontal(|ui| {
        let label = ui.label(components::body_secondary(
            &t,
            strings::text(key!("package-file")),
        ));
        ui.add_space(t.space_3);
        let mut path = d.path.display().to_string();
        let field_width = (ui.available_width() - 120.0).max(120.0);
        // The path is chosen with Choose…, not typed.
        ui.add(
            egui::TextEdit::singleline(&mut path)
                .interactive(false)
                .desired_width(field_width),
        )
        .labelled_by(label.id);
        if ui.button(strings::text(key!("package-choose"))).clicked() {
            events.push(Event::Action("package.file", json!({})));
        }
    });
}

/// A pane's heading.
fn heading(ui: &mut Ui, t: &Tokens, key: ew_viewmodel::strings::Key<'static>) {
    ui.label(components::body_strong(t, strings::text(key)));
    ui.add_space(t.space_2);
}

/// A cell `width` wide at the row's top, its content wrapped in it.
fn cell<R>(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 0.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            add(ui)
        },
    )
    .inner
}

/// The widest of the texts in the font, rounded up.
fn widest(ui: &Ui, font: &egui::FontId, texts: &mut dyn Iterator<Item = &str>) -> f32 {
    texts
        .map(|x| components::text_width(ui, x, font.clone()))
        .fold(0.0_f32, f32::max)
        .ceil()
}

/// Left: What goes in: a row per member, divided by a line. The member with its tick, its form and Change… take the
/// width their words need; what a member holds takes the rest, so that it stands on one line.
fn goes_in(ui: &mut Ui, t: &Tokens, d: &PackageDialog, events: &mut Vec<Event>) {
    ui.set_width(ui.available_width());
    heading(ui, t, key!("package-goes-in"));
    // The columns stand apart as the Labels… and Rework list… tables' do.
    let gap = t.space_4;
    let full = ui.available_width();
    let body = crate::theme::font(&t.type_body);
    let tick = ui.spacing().icon_width + ui.spacing().icon_spacing;
    let w1 = tick + widest(ui, &body, &mut d.rows.iter().map(|r| r.words.as_str()));
    let w3 = widest(
        ui,
        &body,
        &mut d
            .rows
            .iter()
            .filter(|r| r.member != PackageMember::Labels)
            .map(|r| r.form.as_str()),
    );
    // The label files' Change… stands in the form column, where the other rows have their form.
    let change = strings::text(key!("package-change"));
    let w3 = w3.max(widest(ui, &body, &mut std::iter::once(change)));
    // The lack lines' offers (Edit part…, Workshop stock…) stand at the right, as wide as the widest.
    let link = widest(
        ui,
        &body,
        &mut d
            .rows
            .iter()
            .flat_map(|r| r.lacks.iter().map(|l| l.offer.as_str())),
    );
    let w2 = (full - w1 - w3 - 2.0 * gap).max(0.0);
    for r in &d.rows {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            cell(ui, w1, |ui| {
                let mut on = r.on;
                // A member without content has no file: its tick is greyed with why.
                let tick = ui
                    .add_enabled(
                        r.content,
                        egui::Checkbox::new(&mut on, components::body(t, &r.words)),
                    )
                    .on_disabled_hover_text(&r.holds);
                if tick.changed() {
                    events.push(Event::Action(
                        "package.tick",
                        json!({ "member": r.member.key(), "on": on }),
                    ));
                }
            });
            let holds = if r.content {
                components::body_secondary(t, &r.holds)
            } else {
                components::small_tertiary(t, &r.holds)
            };
            // The label files' row: what they hold, then the profile in a line, with Change… (the form Label file).
            let labels = r.member == PackageMember::Labels;
            cell(ui, w2, |ui| {
                ui.add(egui::Label::new(holds).wrap());
                if labels {
                    ui.add(egui::Label::new(components::small_secondary(t, &r.form)).wrap());
                }
            });
            cell(ui, w3, |ui| {
                if !labels {
                    ui.add(egui::Label::new(components::body_secondary(t, &r.form)).wrap());
                } else if ui.link(change).clicked() {
                    events.push(Event::Action("labels.columns", json!({})));
                }
            });
        });
        // What it lacks of part data, under what it holds, with the way to it the shell gives: Edit part… at a part's
        // missing field, or Workshop stock…, where a person adds a marker that marks the set.
        for lack in &r.lacks {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                cell(ui, w1, |_| {});
                cell(ui, (w2 + w3 - link).max(0.0), |ui| {
                    ui.horizontal_top(|ui| {
                        icon(ui, t, crate::icons::IconId::alert_triangle, t.warning);
                        ui.add(egui::Label::new(components::small_secondary(t, &lack.text)).wrap());
                    });
                });
                if ui.link(&lack.offer).clicked() {
                    events.push(Event::Action(lack.action.0, lack.action.1.clone()));
                }
            });
        }
        ui.separator();
    }
}

/// An icon of `id` in `colour` at the body text's size.
fn icon(ui: &mut Ui, t: &Tokens, id: crate::icons::IconId, colour: crate::theme::Rgba) {
    let size = t.type_body.size;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    crate::icons::paint(ui.painter(), id, rect, colour.color32(), t.icon_stroke);
}

/// Right: Before it goes to the workshop.
fn before(ui: &mut Ui, t: &Tokens, d: &PackageDialog, events: &mut Vec<Event>) {
    ui.set_width(ui.available_width());
    heading(ui, t, key!("package-before"));
    ui.horizontal_wrapped(|ui| {
        let (glyph, colour) = if d.clean {
            (crate::icons::IconId::circle_check, t.success)
        } else if d
            .findings
            .iter()
            .any(|f| f.severity == ew_viewmodel::ew_api::checks::Severity::Error)
        {
            (crate::icons::IconId::circle_x, t.error)
        } else {
            (crate::icons::IconId::alert_triangle, t.warning)
        };
        icon(ui, t, glyph, colour);
        ui.label(components::body(t, &d.ready));
        if !d.clean && ui.link(strings::text(key!("package-open-messages"))).clicked() {
            events.push(Event::Action("package.messages", json!({})));
        }
    });
    for f in &d.findings {
        ui.add_space(t.space_2);
        finding(ui, t, f, events);
    }
    // The wires without a length that no finding names, a line per reason.
    for l in &d.lengths {
        ui.add_space(t.space_2);
        ui.horizontal_top(|ui| {
            icon(ui, t, crate::icons::IconId::info_circle, t.text_secondary);
            ui.add(egui::Label::new(components::body(t, l)).wrap());
        });
    }
    ui.add_space(t.space_2);
    ui.add(
        egui::Label::new(components::small_secondary(
            t,
            strings::text(key!("package-check-note")),
        ))
        .wrap(),
    );
}

/// One finding: its rule, its message, whether it stops a release; Show with its records and its fix.
fn finding(ui: &mut Ui, t: &Tokens, f: &PackageFinding, events: &mut Vec<Event>) {
    let error = f.severity == ew_viewmodel::ew_api::checks::Severity::Error;
    let fill = if error { t.error_subtle } else { t.surface_sunken };
    components::band_frame(t, fill).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_wrapped(|ui| {
            ui.label(components::small_secondary(t, f.rule).strong());
            if error {
                ui.label(components::styled(
                    strings::text(key!("package-stops")),
                    &t.type_small,
                    t.error,
                ));
            }
        });
        ui.add(egui::Label::new(components::body(t, &f.message)).wrap());
        if f.shown && !f.targets.is_empty() {
            ui.add(egui::Label::new(components::small_secondary(t, f.targets.join(", "))).wrap());
        }
        ui.horizontal(|ui| {
            if !f.targets.is_empty() {
                let word = if f.shown {
                    key!("package-hide")
                } else {
                    key!("package-show")
                };
                if ui.link(strings::text(word)).clicked() {
                    events.push(Event::Action(
                        "package.show_finding",
                        json!({ "key": f.key, "on": !f.shown }),
                    ));
                }
            }
            if let Some(fix) = &f.fix
                && ui.button(fix).clicked()
            {
                events.push(Event::Action("package.fix", json!({ "key": f.key })));
            }
        });
    });
}

/// Right: the revision, a chip that says draft or released, and what it means for the package.
fn revision(ui: &mut Ui, t: &Tokens, d: &PackageDialog) {
    ui.set_width(ui.available_width());
    heading(ui, t, key!("package-revision"));
    ui.horizontal_wrapped(|ui| {
        let chip = if d.draft {
            key!("package-chip-draft")
        } else {
            key!("package-chip-released")
        };
        components::caption_chip(ui, strings::text(chip));
        ui.label(components::body(t, &d.revision.0));
    });
    ui.add(egui::Label::new(components::small_secondary(t, &d.revision.1)).wrap());
}

/// Right: Without personal data, greyed with why while no party is marked as personal data.
fn personal(ui: &mut Ui, t: &Tokens, d: &PackageDialog, events: &mut Vec<Event>) {
    ui.set_width(ui.available_width());
    heading(ui, t, key!("package-personal"));
    let mut on = d.personal;
    let enabled = d.personal_note.is_ok();
    let note = match &d.personal_note {
        Ok(n) | Err(n) => n,
    };
    let check = ui
        .add_enabled(
            enabled,
            egui::Checkbox::new(&mut on, strings::text(key!("package-personal-check"))),
        )
        .on_disabled_hover_text(note);
    if check.changed() {
        events.push(Event::Action("package.personal", json!({ "on": on })));
    }
    ui.add(egui::Label::new(components::small_secondary(t, note)).wrap());
}
