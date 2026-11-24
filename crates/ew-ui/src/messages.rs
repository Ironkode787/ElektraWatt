// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel's own parts of the navigator pane (restyled to
//! doc 16a §6.24): the header with the scheme and one count badge per severity, the quick-fix card, the detail block
//! of the cursor's finding with its Fix button and rule page link, and the severity icon and word of the
//! finding rows (FR-UI-025: an icon, a colour and the word).
//!
//! **Icons.** Error `circle-x` in `color.error`, warning `alert-triangle` in `color.warning.icon`, info `info-circle`
//! in `color.accent` (doc 16a §6.24), at `size.severity_icon`: three distinct shapes. A pending row draws its icon,
//! word and title in `color.text.tertiary` and carries the "Pending" status badge, so no state rests on colour alone.
//!
//! **Accessibility.** The header is one `Label` node named by the as-built sentence (`<scheme> · E 1 · W 2 · I 0 ·
//! waived 0 · pending 1`), a polite live region; the badges it paints are not separate nodes.

use crate::components::{self, Badge, Kind};
use crate::icons::{self, IconId};
use crate::theme::{self, Rgba, Tokens};
use egui::{Color32, RichText, Sense, Ui, Vec2};
use ew_viewmodel::key;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{FindingLook, ShellModel};
use ew_viewmodel::strings;
use serde_json::{Value, json};

use crate::app::Event;
use crate::components::Tip as _;

/// A severity as the window draws it (doc 16a §6.24), read from the view-model's word: `error`, `warning`, `pending`
/// (the Properties header's pending chip); any other word is an info.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Severity {
    Error,
    Warning,
    Info,
    Pending,
}

impl Severity {
    /// The severity of the view-model's word `word`.
    pub(crate) fn of(word: &str) -> Severity {
        match word {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "pending" => Severity::Pending,
            _ => Severity::Info,
        }
    }

    /// The colour of its icon: error, the warning icon colour, info (the accent).
    pub(crate) fn icon_colour(self, t: &Tokens) -> Rgba {
        match self {
            Severity::Error => t.error,
            Severity::Warning => t.warning_icon,
            Severity::Info | Severity::Pending => t.info,
        }
    }

    /// The colour of its word: `color.error`, `color.warning` (the text tone), `color.accent`.
    fn word_colour(self, t: &Tokens) -> Rgba {
        match self {
            Severity::Error => t.error,
            Severity::Warning => t.warning,
            Severity::Info | Severity::Pending => t.info,
        }
    }

    /// Its icon (doc 16a §6.24): `circle-x`, `alert-triangle`, `info-circle`; the Properties header's pending chip
    /// `pencil`, as the pending badge.
    pub(crate) fn icon(self) -> IconId {
        match self {
            Severity::Error => IconId::circle_x,
            Severity::Warning => IconId::alert_triangle,
            Severity::Pending => IconId::pencil,
            Severity::Info => IconId::info_circle,
        }
    }

    /// The word of a finding row ("Error", "Warning", "Info").
    pub(crate) fn word(self) -> &'static str {
        strings::text(match self {
            Severity::Error => key!("severity-error"),
            Severity::Warning => key!("severity-warning"),
            Severity::Info | Severity::Pending => key!("severity-info"),
        })
    }

    /// The fill and the text colour of its count badge: its subtle fill and its text tone.
    pub(crate) fn count_colours(self, t: &Tokens) -> (Rgba, Rgba) {
        match self {
            Severity::Error => (t.error_subtle, t.error),
            Severity::Warning => (t.warning_subtle, t.warning),
            Severity::Pending => (t.surface_sunken, t.text_secondary),
            Severity::Info => (t.info_subtle, t.info),
        }
    }
}

/// The colour of a finding row's icon: the severity's token, the tertiary text while it is pending (doc 16a §6.24).
pub(crate) fn icon_colour(ui: &Ui, look: FindingLook) -> Color32 {
    colour_of(&theme::current(ui.ctx()), look)
}

/// The colour rule of [`icon_colour`] over a theme's tokens: the severity's icon colour, the tertiary text while
/// pending.
fn colour_of(t: &Tokens, look: FindingLook) -> Color32 {
    if look.pending {
        return t.text_tertiary.color32();
    }
    Severity::of(look.severity).icon_colour(t).color32()
}

/// The colour of a severity word: the severity's word colour; the tertiary text while pending.
fn word_colour(t: &Tokens, look: FindingLook) -> Rgba {
    if look.pending {
        return t.text_tertiary;
    }
    Severity::of(look.severity).word_colour(t)
}

/// The severity of a severity group row of the Messages tree (`severity:<word>`), `None` for any other row.
pub(crate) fn group_severity(key: &str) -> Option<&'static str> {
    match key.rsplit('/').next()? {
        "severity:error" => Some("error"),
        "severity:warning" => Some("warning"),
        "severity:info" => Some("info"),
        _ => None,
    }
}

/// Paints a severity icon of `size.severity_icon` in its colour (doc 16a §6.24).
pub(crate) fn severity_icon(ui: &mut Ui, look: FindingLook) {
    let t = theme::current(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(t.size_severity_icon), Sense::hover());
    let colour = icon_colour(ui, look);
    icons::paint(
        ui.painter(),
        Severity::of(look.severity).icon(),
        rect,
        colour,
        t.icon_stroke,
    );
}

/// A finding row's severity: the icon, then the word in `type.small` SemiBold in the severity's text colour (doc 16a
/// §5.7's severity cell). The row's accessible name already carries the word, so this paints no node.
pub(crate) fn severity_cell(ui: &mut Ui, look: FindingLook) {
    let t = theme::current(ui.ctx());
    severity_icon(ui, look);
    let font = theme::font(&theme::TypeToken {
        semibold: true,
        ..t.type_small
    });
    let colour = word_colour(&t, look).color32();
    let galley = ui
        .painter()
        .layout_no_wrap(Severity::of(look.severity).word().to_owned(), font, colour);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x, t.size_severity_icon), Sense::hover());
    ui.painter().galley(
        egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        colour,
    );
}

/// The status badge of a finding row's badge word (doc 16a §6.24): `pending`, `waived` and `fix`; `None` for the
/// others (the severity word, which the severity cell paints).
pub(crate) fn finding_badge(word: &str) -> Option<Badge> {
    match word {
        "pending" => Some(Badge::Pending),
        "waived" => Some(Badge::Waived),
        "fix" => Some(Badge::Fix),
        _ => None,
    }
}

/// One count badge of the header (doc 16a §6.24, §6.16): `radius.pill`, the severity's subtle fill, its icon at 12 px
/// and the number in `type.caption` in its text colour. Paints no node.
fn count_badge(ui: &mut Ui, t: &Tokens, severity: Severity, count: usize) {
    let (fill, text) = severity.count_colours(t);
    let galley = ui
        .painter()
        .layout_no_wrap(count.to_string(), theme::font(&t.type_caption), text.color32());
    let icon = t.type_caption.size + t.space_1;
    let size = Vec2::new(
        t.space_2 + icon + t.space_1 + galley.size().x + t.space_2,
        t.size_badge,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(t.radius_pill), fill.color32());
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + t.space_2 + icon / 2.0, rect.center().y),
        Vec2::splat(icon),
    );
    icons::paint(
        painter,
        severity.icon(),
        icon_rect,
        severity.icon_colour(t).color32(),
        t.icon_stroke,
    );
    painter.galley(
        egui::pos2(
            icon_rect.right() + t.space_1,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        text.color32(),
    );
}

/// "<n> <Word>" of a severity, singular for one ("1 Warning", "3 Warnings"): a count chip's word.
pub(crate) fn count_words(severity: Severity, n: usize) -> String {
    // Pending findings count under one word, as the Messages header counts them ("2 Pending").
    let words = if severity == Severity::Pending {
        strings::text(key!("badge-pending"))
    } else if n == 1 {
        severity.word()
    } else {
        strings::text(match severity {
            Severity::Error => key!("severity-errors"),
            Severity::Warning => key!("severity-warnings"),
            Severity::Info | Severity::Pending => key!("severity-infos"),
        })
    };
    strings::format(key!("props-findings-count"), &[&n.to_string(), words])
}

/// One chip for `findings.len()` findings of one severity (always one chip per
/// severity): the icon and "<n> <Word(s)>" ("1 Warning", "3 Warnings"), a `Label` node named by that and each
/// "<rule>: <message>", the same lines as the tooltip.
pub(crate) fn severity_count_chip(
    ui: &mut Ui,
    severity: Severity,
    findings: &[(&str, &str)],
) -> egui::Response {
    let word = count_words(severity, findings.len());
    let lines: Vec<String> = findings
        .iter()
        .map(|(rule, message)| strings::format(key!("props-finding-tip"), &[rule, message]))
        .collect();
    let name = strings::format(key!("props-findings-chip"), &[&word, &lines.join("; ")]);
    paint_severity_chip(ui, severity, &word, &name, &chip_tip(severity, &lines))
}

/// A severity chip's tooltip: its findings, one "<rule>: <message>" line each; the Pending chip says first what it
/// counts, then lists them (item 8).
pub(crate) fn chip_tip(severity: Severity, lines: &[String]) -> String {
    let list = lines.join("\n");
    if severity == Severity::Pending {
        format!("{}\n{list}", strings::text(key!("tip-pending")))
    } else {
        list
    }
}

/// Paints a severity chip with `word`, its node named `name`, `tip` as the tooltip.
fn paint_severity_chip(ui: &mut Ui, severity: Severity, word: &str, name: &str, tip: &str) -> egui::Response {
    let t = theme::current(ui.ctx());
    let (fill, text) = severity.count_colours(&t);
    let galley = ui
        .painter()
        .layout_no_wrap(word.to_owned(), theme::font(&t.type_caption), text.color32());
    let icon = t.type_caption.size + t.space_1;
    let size = Vec2::new(
        t.space_2 + icon + t.space_1 + galley.size().x + t.space_2,
        t.size_badge,
    );
    // The Properties header's chip is a button that opens Messages filtered to the record.
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(t.radius_sm), fill.color32());
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + t.space_2 + icon / 2.0, rect.center().y),
        Vec2::splat(icon),
    );
    icons::paint(
        painter,
        severity.icon(),
        icon_rect,
        severity.icon_colour(&t).color32(),
        t.icon_stroke,
    );
    painter.galley(
        egui::pos2(
            icon_rect.right() + t.space_1,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        text.color32(),
    );
    response.tip(tip)
}

/// The header (doc 16a §6.24): the scheme in `type.small` `color.text.secondary`, one count badge per severity, then
/// "waived n" and "pending n" in `color.text.tertiary`; one `Label` node named by the as-built sentence, a polite live
/// region. Then the quick-fix card.
pub(crate) fn header(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    if let Some(c) = shell.messages_counts() {
        let scheme = shell.messages_scheme().unwrap_or_default();
        let line = strings::format(
            key!("messages-header"),
            &[
                scheme,
                &c.errors.to_string(),
                &c.warnings.to_string(),
                &c.infos.to_string(),
                &c.waived.to_string(),
                &c.pending.to_string(),
            ],
        );
        let t = theme::current(ui.ctx());
        let row = ui.horizontal(|ui| {
            ui.set_min_height(t.size_row);
            ui.spacing_mut().item_spacing.x = t.space_2;
            // The pane's `space.4` gutter.
            ui.add_space(t.space_4);
            let small = |ui: &mut Ui, text: &str, colour: Rgba| {
                let galley = ui.painter().layout_no_wrap(
                    text.to_owned(),
                    theme::font(&t.type_small),
                    colour.color32(),
                );
                let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
                ui.painter().galley(rect.min, galley, colour.color32());
                response
            };
            // A project with an import offers its migration report beside its scheme.
            if shell.messages_offers_migration() {
                let migration = shell.messages_shows_migration();
                let choices = [
                    (
                        shell.messages_project_scheme().unwrap_or_default(),
                        !migration,
                        false,
                    ),
                    (strings::text(key!("messages-scheme-migration")), migration, true),
                ];
                for (label, chosen, on) in choices {
                    let text = RichText::new(label).font(theme::font(&t.type_small));
                    if ui.selectable_label(chosen, text).clicked() && !chosen {
                        events.push(Event::Action("messages.migration", json!({ "on": on })));
                    }
                }
            } else {
                small(ui, scheme, t.text_secondary);
            }
            count_badge(ui, &t, Severity::Error, c.errors);
            count_badge(ui, &t, Severity::Warning, c.warnings);
            count_badge(ui, &t, Severity::Info, c.infos);
            let waived = strings::format(key!("messages-header-waived"), &[&c.waived.to_string()]);
            small(ui, &waived, t.text_tertiary);
            let pending = strings::format(key!("messages-header-pending"), &[&c.pending.to_string()]);
            // "pending n" says what it counts, in its tooltip (the Pending chips' text).
            let response = small(ui, &pending, t.text_tertiary);
            response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &pending));
            response.tip(strings::text(key!("tip-pending")));
        });
        let id = ui.scope_id().with("messages-header");
        let node = ui.interact(row.response.rect, id, Sense::hover());
        let name = line.clone();
        node.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
        ui.ctx().accesskit_node_builder(id, |n| {
            n.set_live(egui::accesskit::Live::Polite);
            n.set_value(line);
        });
        // While findings are pending, one sentence under the header says what pending means.
        if c.pending > 0 {
            ui.horizontal(|ui| {
                ui.add_space(t.space_4);
                ui.add(
                    egui::Label::new(crate::components::small_secondary(
                        &t,
                        strings::text(key!("messages-pending-note")),
                    ))
                    .wrap(),
                );
            });
        }
    }
    fix_card(ui, shell, events);
}

/// The quick-fix card (doc 16a §6.23): "Quick fix `<rule>`" with its revision and status badge,
/// what the fix changes, Apply and Cancel.
fn fix_card(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(card) = shell.properties().card() else {
        return;
    };
    let CardOrigin::QuickFix { rule } = &card.origin else {
        return;
    };
    let title = strings::format(key!("card-title-fix"), &[rule]);
    let summary = ew_viewmodel::properties::fix_line(card);
    let card = crate::components::InlineCard {
        title: &title,
        summary: &summary,
        apply: "navigator.apply",
        strip: true,
    };
    crate::components::inline_card(ui, shell, card, events, |_, _| {});
}

/// The detail block of the cursor's finding, below the rows (doc 16a §6.24): a 1-px `color.border.subtle` line, the
/// rule ID and title in `type.body.strong`, the kind and severity in `type.small` `color.text.secondary`, the message
/// in `type.body`, "Quick fix: `<label>`" in `type.small` with **Fix…** as a Secondary button (disabled with its reason
/// as the tooltip on a read-only project), and at the title line's right end **Waive…** (on a waived finding
/// "Waived" in `color.text.secondary` with **Unwaive**) and **Rule page**, links in `type.small` `color.accent`, so the
/// waiver costs the block no height.
pub(crate) fn detail(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(d) = shell.messages_detail() else {
        return;
    };
    let t = theme::current(ui.ctx());
    rule_line(ui, &t);
    ui.spacing_mut().item_spacing.y = t.space_1;
    ui.add_space(t.space_1);
    let enabled = shell.is_writable();
    let disabled_reason = |id: &str| {
        ew_viewmodel::actions::find(id)
            .and_then(|a| a.enabled_now(shell).err())
            .map(strings::text)
    };
    let small_link = |ui: &mut Ui, key: strings::Key<'_>, id: &str| {
        let colour = if enabled { t.accent } else { t.text_disabled };
        let link = ui.add_enabled(
            enabled,
            egui::Link::new(
                RichText::new(strings::text(key))
                    .font(theme::font(&t.type_small))
                    .color(colour.color32()),
            ),
        );
        match disabled_reason(id) {
            Some(r) if !enabled => link.on_disabled_hover_text(r),
            _ => link,
        }
    };
    // The Rule page link at the right end of the title line, the title cut with an ellipsis before it (whole
    // in its tooltip), so the block keeps its link in view in a short pane. Waive… (or "Waived" with
    // Unwaive) stands before it on the same line.
    let title = strings::format(key!("messages-detail-rule"), &[&d.rule, &d.title]);
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 0.0;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = t.space_3;
                let link = ui.add(egui::Link::new(crate::components::styled(
                    strings::text(key!("messages-rule-page")),
                    &t.type_small,
                    t.accent,
                )));
                if link.clicked() {
                    events.push(Event::Action(
                        "ui.focus",
                        json!({"area": "navigator", "kind": NavKind::Messages.word()}),
                    ));
                    events.push(Event::Action("help.documentation", Value::Null));
                }
                if d.waived {
                    if small_link(ui, key!("messages-unwaive-button"), "messages.unwaive").clicked() {
                        events.push(Event::Action("messages.unwaive", Value::Null));
                    }
                    ui.label(crate::components::small_secondary(
                        &t,
                        strings::text(key!("messages-detail-waived")),
                    ));
                } else if small_link(ui, key!("messages-waive-button"), "messages.waive").clicked() {
                    // The window reads the date; the shell never does.
                    events.push(Event::Action(
                        "messages.waive",
                        json!({ "date": crate::clock::Clock::of(ui.ctx()).today() }),
                    ));
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(crate::components::body_strong(&t, &title)).truncate());
                });
            });
        });
    });
    ui.label(crate::components::small_secondary(&t, d.kind_line()));
    ui.add(egui::Label::new(crate::components::body(&t, &d.message)).wrap());
    // Where the finding asks for a step the window can take the person to (Choose a cable type…).
    if let Some(offer) = shell
        .nav_cursor(ew_viewmodel::navigators::NavKind::Messages)
        .and_then(|k| shell.finding_offer(&k))
        .filter(|_| !d.waived)
        && components::button(ui, Kind::Secondary, &offer.label, enabled && offer.enabled, 0.0).clicked()
    {
        events.push(Event::Action(offer.id, offer.params.clone()));
    }
    if let (Some(fix), false) = (&d.fix, d.waived) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = t.space_3;
            ui.label(crate::components::small_secondary(
                &t,
                strings::format(key!("messages-detail-fix"), &[fix]),
            ));
            let button = components::button(
                ui,
                Kind::Secondary,
                strings::text(key!("messages-fix-button")),
                enabled,
                0.0,
            );
            let button = match disabled_reason("messages.fix") {
                Some(r) if !enabled => button.disabled_tip(r),
                _ => button,
            };
            if button.clicked() {
                events.push(Event::Action("messages.fix", Value::Null));
            }
            // Every finding of its group with a fix of this kind: Fix all like this (n)….
            if let Some((group, n)) = shell
                .nav_cursor(ew_viewmodel::navigators::NavKind::Messages)
                .and_then(|k| shell.fix_all_offer(&k))
            {
                let label = strings::format(key!("messages-fix-all-like"), &[&n.to_string()]);
                if components::button(ui, Kind::Secondary, &label, enabled, 0.0).clicked() {
                    events.push(Event::Action(
                        "messages.fix_all",
                        serde_json::json!({ "key": group }),
                    ));
                }
            }
        });
    }
}

/// A 1-px `color.border.subtle` line across the space left.
pub(crate) fn rule_line(ui: &mut Ui, t: &Tokens) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), t.border_width), Sense::hover());
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_none), t.border_subtle.color32());
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
                assert_eq!(
                    word_colour(&t, look(s, true)),
                    t.text_tertiary,
                    "{s} word pending"
                );
            }
            assert_eq!(word_colour(&t, look("error", false)), t.error);
            assert_eq!(word_colour(&t, look("warning", false)), t.warning);
            assert_ne!(t.error, t.warning_icon, "error and warning differ");
        }
    }

    /// Three distinct icon shapes and the words of the three severities; the group keys map to their severity.
    #[test]
    fn three_distinct_shapes_words_and_group_keys() {
        let ids: Vec<IconId> = ["error", "warning", "info"]
            .into_iter()
            .map(|s| Severity::of(s).icon())
            .collect();
        assert_eq!(
            ids,
            [IconId::circle_x, IconId::alert_triangle, IconId::info_circle]
        );
        assert_ne!(ids[0].segments(), ids[1].segments());
        assert_ne!(ids[1].segments(), ids[2].segments());
        assert_ne!(ids[0].segments(), ids[2].segments());
        assert_eq!(
            ["error", "warning", "info"].map(|s| Severity::of(s).word()),
            ["Error", "Warning", "Info"]
        );
        assert_eq!(group_severity("severity:warning"), Some("warning"));
        assert_eq!(group_severity("severity:error/rule:EW-CORE-033"), None);
        assert_eq!(finding_badge("pending"), Some(Badge::Pending));
        assert_eq!(finding_badge("fix"), Some(Badge::Fix));
        assert_eq!(finding_badge("error"), None);
    }

    /// The Pending chip's tooltip says what it counts, then lists its findings; another
    /// chip's lists them only.
    #[test]
    fn the_pending_tip_defines_then_lists() {
        let lines = [
            "EW-CORE-041: -K1 has no part".to_owned(),
            "EW-CORE-033: -3W1 has no length".to_owned(),
        ];
        assert_eq!(
            chip_tip(Severity::Pending, &lines),
            "Findings on the page you are editing that wait until you leave it or run a check\n\
             EW-CORE-041: -K1 has no part\nEW-CORE-033: -3W1 has no length"
        );
        assert_eq!(
            chip_tip(Severity::Warning, &lines),
            "EW-CORE-041: -K1 has no part\nEW-CORE-033: -3W1 has no length"
        );
    }
}
