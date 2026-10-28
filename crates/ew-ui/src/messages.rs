// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel's own parts of the navigator pane (WP-S2.09 part 2, §7.3, §7.4): the header line with the scheme
//! and the counts, the quick-fix card, the detail block of the cursor's finding with its Fix button and rule page link,
//! the footer, and the severity icons of the finding rows (FR-UI-025: an icon, a colour and the word).
//!
//! **Icons.** Error a filled circle in the theme's error colour, warning a filled triangle in its warning colour, info a
//! filled square in its info colour (the hyperlink colour); a pending row draws the same shape outlined in the muted
//! text colour, and its text is muted too. The word is the row's badge, so no state rests on colour alone.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{FindingLook, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;

/// The side of a severity icon in points.
const ICON: f32 = 10.0;

/// The colour of a finding row's icon: the severity's theme colour, the muted text colour while it is pending.
pub(crate) fn icon_colour(ui: &Ui, look: FindingLook) -> Color32 {
    colour_of(ui.visuals(), look)
}

/// The colour rule of [`icon_colour`] over a theme's visuals.
fn colour_of(v: &egui::Visuals, look: FindingLook) -> Color32 {
    if look.pending {
        return v.weak_text_color();
    }
    match look.severity {
        "error" => v.error_fg_color,
        "warning" => v.warn_fg_color,
        _ => v.hyperlink_color,
    }
}

/// Paints a finding row's severity icon (§7.3) in a square of its own.
pub(crate) fn severity_icon(ui: &mut Ui, look: FindingLook) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(ICON), Sense::hover());
    let colour = icon_colour(ui, look);
    let painter = ui.painter();
    let c = rect.center();
    let r = ICON / 2.0 - 1.0;
    let stroke = Stroke::new(1.5, colour);
    match look.severity {
        "error" => {
            if look.pending {
                painter.circle_stroke(c, r, stroke);
            } else {
                painter.circle_filled(c, r, colour);
            }
        }
        "warning" => {
            let points = vec![
                Pos2::new(c.x, c.y - r),
                Pos2::new(c.x + r, c.y + r),
                Pos2::new(c.x - r, c.y + r),
            ];
            let fill = if look.pending {
                Color32::TRANSPARENT
            } else {
                colour
            };
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
        _ => {
            let square = Rect::from_center_size(c, Vec2::splat(2.0 * r));
            if look.pending {
                painter.rect_stroke(square, 0.0, stroke, egui::StrokeKind::Inside);
            } else {
                painter.rect_filled(square, 0.0, colour);
            }
        }
    }
}

/// The header line (`<scheme> · E <n> · W <n> · I <n> · waived <n> · pending <n>`) and the quick-fix card.
pub(crate) fn header(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    if let Some(c) = shell.messages_counts() {
        let line = strings::format(
            "messages-header",
            &[
                shell.messages_scheme().unwrap_or_default(),
                &c.errors.to_string(),
                &c.warnings.to_string(),
                &c.infos.to_string(),
                &c.waived.to_string(),
                &c.pending.to_string(),
            ],
        );
        let response = ui.label(&line);
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_live(egui::accesskit::Live::Polite);
            n.set_value(line);
        });
    }
    fix_card(ui, shell, events);
}

/// The quick-fix card (§7.4): what the fix changes, Apply and Cancel.
fn fix_card(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let (Some(card), Some(rev)) = (shell.properties().card(), shell.revision()) else {
        return;
    };
    let CardOrigin::QuickFix { rule } = &card.origin else {
        return;
    };
    ui.group(|ui| {
        let mut line = strings::format(
            "card-fix",
            &[
                rule,
                &card.changes.to_string(),
                &card.preview.derived.tags.len().to_string(),
                &card.preview.derived.xrefs.len().to_string(),
            ],
        );
        match (&card.preview.result, card.preview.status(rev)) {
            (Err(m), _) => line = strings::format("card-failed", &[m]),
            (_, ew_viewmodel::tools::PreviewStatus::Stale) => {
                line.push_str(&format!(" [{}]", strings::text("card-stale")));
            }
            _ => {}
        }
        // WP-S2.09 part 3 (§8.1): while the Preview tab shows the card, a one-line chip stands in for it.
        if crate::preview::chip(ui, shell, &line, events) {
            return;
        }
        let response = ui.label(&line);
        ui.ctx().accesskit_node_builder(response.id, |n| {
            n.set_live(egui::accesskit::Live::Polite);
            n.set_value(line);
        });
        ui.horizontal(|ui| {
            if ui.button(strings::text("card-apply")).clicked() {
                events.push(Event::Action("navigator.apply", Value::Null));
            }
            if ui.button(strings::text("card-cancel")).clicked() {
                events.push(Event::Action("preview.cancel", Value::Null));
            }
        });
    });
}

/// The detail block of the cursor's finding and the footer, below the rows.
pub(crate) fn detail_and_footer(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    if let Some(d) = shell.messages_detail() {
        ui.separator();
        ui.label(egui::RichText::new(strings::format("messages-detail-rule", &[&d.rule, &d.title])).strong());
        ui.label(strings::format("messages-detail-kind", &[&d.kind, &d.severity]));
        ui.label(&d.message);
        ui.horizontal(|ui| {
            if let Some(fix) = &d.fix {
                ui.label(strings::format("messages-detail-fix", &[fix]));
                let enabled = shell.is_writable();
                if ui
                    .add_enabled(enabled, egui::Button::new(strings::text("messages-fix-button")))
                    .clicked()
                {
                    events.push(Event::Action("messages.fix", Value::Null));
                }
            }
            if ui.link(strings::text("messages-rule-page")).clicked() {
                events.push(Event::Action(
                    "ui.focus",
                    json!({"area": "navigator", "kind": NavKind::Messages.word()}),
                ));
                events.push(Event::Action("help.documentation", Value::Null));
            }
        });
    }
    ui.separator();
    ui.label(egui::RichText::new(strings::text("messages-footer")).weak());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_colour_is_the_severitys_theme_colour_and_muted_while_pending() {
        for v in [egui::Visuals::light(), egui::Visuals::dark()] {
            let look = |severity, pending| FindingLook { severity, pending };
            assert_eq!(colour_of(&v, look("error", false)), v.error_fg_color);
            assert_eq!(colour_of(&v, look("warning", false)), v.warn_fg_color);
            assert_eq!(colour_of(&v, look("info", false)), v.hyperlink_color);
            for s in ["error", "warning", "info"] {
                assert_eq!(colour_of(&v, look(s, true)), v.weak_text_color(), "{s} pending");
            }
            assert_ne!(v.error_fg_color, v.warn_fg_color, "error and warning differ");
        }
    }
}
