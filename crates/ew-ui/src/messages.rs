// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel's own parts of the navigator pane (WP-S2.09 part 2, §7.3, §7.4): the header line with the scheme
//! and the counts, the quick-fix card, the detail block of the cursor's finding with its Fix button and rule page link,
//! the footer, and the severity icons of the finding rows (FR-UI-025: an icon, a colour and the word).
//!
//! **Icons.** Error a filled circle in the theme's error colour, warning a filled triangle in its warning colour, info a
//! filled square in its info colour (the hyperlink colour); a pending row draws the same shape outlined in the muted
//! text colour, and its text is muted too. The word is the row's badge, so no state rests on colour alone.

use crate::theme::Tokens;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{FindingLook, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;

/// The colour of a finding row's icon: the severity's token, the tertiary text while it is pending (doc 16a §6.24).
pub(crate) fn icon_colour(ui: &Ui, look: FindingLook) -> Color32 {
    colour_of(&crate::theme::current(ui.ctx()), look)
}

/// The colour rule of [`icon_colour`] over a theme's tokens: error, the warning icon colour, info (the accent).
fn colour_of(t: &Tokens, look: FindingLook) -> Color32 {
    if look.pending {
        return t.text_tertiary.color32();
    }
    match look.severity {
        "error" => t.error,
        "warning" => t.warning_icon,
        _ => t.info,
    }
    .color32()
}

/// Paints a finding row's severity icon (§7.3) in a square of `size.severity_icon`; the shape (circle, triangle,
/// square) is drawn in the inner 10 px of the 16-px box, as built, until the icon set of WP-S2.17 part 6.
pub(crate) fn severity_icon(ui: &mut Ui, look: FindingLook) {
    let t = crate::theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(t.size_severity_icon), Sense::hover());
    let colour = icon_colour(ui, look);
    let painter = ui.painter();
    let c = rect.center();
    let r = t.size_severity_icon / 2.0 - t.space_icon / 2.0;
    let stroke = Stroke::new(t.icon_stroke, colour);
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
            let fill = if look.pending { crate::theme::NONE } else { colour };
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
        _ => {
            let square = Rect::from_center_size(c, Vec2::splat(2.0 * r));
            if look.pending {
                painter.rect_stroke(square, crate::theme::SQUARE, stroke, egui::StrokeKind::Inside);
            } else {
                painter.rect_filled(square, crate::theme::SQUARE, colour);
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

/// The quick-fix card (§7.4; WP-S2.17 part 4, doc 16a §6.23): "Quick fix `<rule>`" with its revision and status badge,
/// what the fix changes, Apply and Cancel.
fn fix_card(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(card) = shell.properties().card() else {
        return;
    };
    let CardOrigin::QuickFix { rule } = &card.origin else {
        return;
    };
    let title = strings::format("card-title-fix", &[rule]);
    let summary = strings::format(
        "card-fix",
        &[
            &card.changes.to_string(),
            &card.preview.derived.tags.len().to_string(),
            &card.preview.derived.xrefs.len().to_string(),
        ],
    );
    let card = crate::components::InlineCard {
        title: &title,
        summary: &summary,
        apply: "navigator.apply",
        strip: true,
    };
    crate::components::inline_card(ui, shell, card, events, |_, _| {});
}

/// The detail block of the cursor's finding, below the rows.
pub(crate) fn detail(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
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
}

/// The footer, at the foot of the pane.
pub(crate) fn footer(ui: &mut Ui) {
    ui.separator();
    ui.label(egui::RichText::new(strings::text("messages-footer")).weak());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_colour_is_the_severitys_token_and_tertiary_while_pending() {
        use crate::theme::{Density, Theme, tokens};
        for theme in Theme::ALL {
            let t = tokens(theme, Density::Comfortable);
            let look = |severity, pending| FindingLook { severity, pending };
            assert_eq!(colour_of(&t, look("error", false)), t.error.color32());
            assert_eq!(colour_of(&t, look("warning", false)), t.warning_icon.color32());
            assert_eq!(colour_of(&t, look("info", false)), t.accent.color32());
            for s in ["error", "warning", "info"] {
                assert_eq!(
                    colour_of(&t, look(s, true)),
                    t.text_tertiary.color32(),
                    "{s} pending"
                );
            }
            assert_ne!(t.error, t.warning_icon, "error and warning differ");
        }
    }
}
