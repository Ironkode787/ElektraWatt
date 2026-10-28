// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The design language's small components (WP-S2.17 part 4; doc 16a §6), drawn from the tokens of the theme on show:
//! the four button kinds (§6.2), status and count badges and key chips (§6.16), the empty state (§6.17), the inline
//! card with its header, status badge and Apply as the Primary (§6.9, §6.23), the "Preview open" strip (§6.23) and a
//! collapsible section header with a count badge (§6.23). Every other widget of part 4 (dialogs, toasts, menus, the
//! palette, the picker, the Preview tab) is built from these.
//!
//! **Accessibility.** Buttons are named by their label; a badge is a label named by its word (the word, never the
//! colour alone, carries the state); an empty state is a `Group` named by its line; a card's summary is a polite live
//! region, as built.

use egui::{Align, Atom, Button, Layout, Response, RichText, Sense, Ui, Vec2};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::PreviewStatus;
use serde_json::{Value, json};

use crate::app::Event;
use crate::icons::{self, IconId};
use crate::theme::{self, Rgba, Tokens};

/// The kind of a button (doc 16a §6.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// The one recommended action: `color.accent` fill, `color.text.on_accent`, `type.body.strong`.
    Primary,
    /// Every other dialog action: the panel fill, a 1-px `color.border.default`.
    Secondary,
    /// No fill, no border; the hover overlay (toolbar-like actions inside panels, toasts, rows).
    Subtle,
    /// Secondary with `color.error` text (Discard…, Purge).
    Danger,
    /// Primary with the `color.error` fill: the destructive step of a confirmation.
    DangerPrimary,
}

/// The tokens of the theme `ui` shows.
pub(crate) fn tokens(ui: &Ui) -> std::sync::Arc<Tokens> {
    theme::current(ui.ctx())
}

/// An icon atom of `size` in `colour` (the text colour of its widget when `None`).
pub(crate) fn icon_atom(id: IconId, size: f32, colour: Option<Rgba>, stroke: f32) -> Atom<'static> {
    Atom::paint(Vec2::splat(size), move |ui, args| {
        let c = colour.map_or(args.fallback_text_color, Rgba::color32);
        icons::paint(ui.painter(), id, args.rect, c, stroke);
    })
}

/// A button of `kind` labelled `label`, `size.control` high, `radius.md`; `min_width` is 72 px in dialogs (0
/// elsewhere). Disabled buttons keep their place.
pub(crate) fn button(ui: &mut Ui, kind: Kind, label: &str, enabled: bool, min_width: f32) -> Response {
    button_with_icon(ui, kind, None, label, enabled, min_width)
}

/// [`button`] with an `icon.sm` icon at the left, `space.icon` before the label.
pub(crate) fn button_with_icon(
    ui: &mut Ui,
    kind: Kind,
    icon: Option<IconId>,
    label: &str,
    enabled: bool,
    min_width: f32,
) -> Response {
    let t = tokens(ui);
    let (fill, text, border, font) = match kind {
        Kind::Primary => (t.accent, t.text_on_accent, None, &t.type_body_strong),
        Kind::DangerPrimary => (t.error, t.text_on_accent, None, &t.type_body_strong),
        Kind::Secondary => (
            t.surface_panel,
            t.text_primary,
            Some(t.border_default),
            &t.type_body,
        ),
        Kind::Danger => (t.surface_panel, t.error, Some(t.border_default), &t.type_body),
        Kind::Subtle => (
            Rgba {
                a: 0,
                ..t.surface_panel
            },
            t.text_primary,
            None,
            &t.type_body,
        ),
    };
    let text = if enabled { text } else { t.text_disabled };
    let label = RichText::new(label).font(theme::font(font)).color(text.color32());
    let mut b = match icon {
        Some(id) => Button::new((icon_atom(id, t.icon_sm, Some(text), t.icon_stroke), label)),
        None => Button::new(label),
    };
    b = b
        .fill(fill.color32())
        .corner_radius(theme::radius(t.radius_md))
        .min_size(Vec2::new(min_width, t.size_control));
    b = match border {
        Some(c) => b.stroke(theme::stroke(t.border_width, c)),
        None => b.stroke(theme::stroke(0.0, t.border_default)),
    };
    let response = ui.add_enabled(enabled, b);
    // The state overlay (doc 16a §6.1): an explicit fill hides egui's own hover fill.
    crate::command_bar::state_overlay(ui, &t, &response, enabled, t.radius_md);
    response
}

/// A 24-px Subtle icon button with `icon.sm` in `color.text.secondary` (doc 16a §6.3), named `name`, with `name` as
/// its tooltip.
pub(crate) fn icon_button(ui: &mut Ui, id: IconId, name: &str) -> Response {
    let t = tokens(ui);
    let size = Vec2::splat(t.size_control_inline);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let n = name.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &n));
    crate::command_bar::state_overlay(ui, &t, &response, true, t.radius_sm);
    let colour = if response.hovered() {
        t.text_primary
    } else {
        t.text_secondary
    };
    icons::paint(
        ui.painter(),
        id,
        egui::Rect::from_center_size(rect.center(), Vec2::splat(t.icon_sm)),
        colour.color32(),
        t.icon_stroke,
    );
    response.on_hover_text(name)
}

/// The kind of a status badge (doc 16a §6.16, §6.19).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Badge {
    /// "Stale": the warning style.
    Stale,
    /// "Failed": the error style.
    Failed,
    /// "Done": the success style (a finished job, FT-510).
    Done,
    /// "Cancelled": the neutral style.
    Cancelled,
}

impl Badge {
    /// The badge's word.
    pub(crate) fn word(self) -> &'static str {
        strings::text(match self {
            Badge::Stale => "badge-stale",
            Badge::Failed => "badge-failed",
            Badge::Done => "badge-done",
            Badge::Cancelled => "badge-cancelled",
        })
    }

    fn look(self, t: &Tokens) -> (Rgba, Rgba, IconId) {
        match self {
            Badge::Stale => (t.warning_subtle, t.warning, IconId::alert_triangle),
            Badge::Failed => (t.error_subtle, t.error, IconId::circle_x),
            Badge::Done => (t.success_subtle, t.success, IconId::circle_check),
            Badge::Cancelled => (t.surface_sunken, t.text_secondary, IconId::x),
        }
    }
}

/// A status badge: `radius.sm`, the subtle semantic fill, an icon and the word in `type.caption`; a label node named
/// by the word.
pub(crate) fn status_badge(ui: &mut Ui, badge: Badge) -> Response {
    let t = tokens(ui);
    let (fill, text, icon) = badge.look(&t);
    let word = badge.word();
    let galley = ui
        .painter()
        .layout_no_wrap(word.to_owned(), theme::font(&t.type_caption), text.color32());
    let icon_size = t.type_caption.size + t.space_1;
    let size = Vec2::new(
        t.space_2 + icon_size + t.space_1 + galley.size().x + t.space_2,
        t.size_badge,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, word));
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(t.radius_sm), fill.color32());
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + t.space_2 + icon_size / 2.0, rect.center().y),
        Vec2::splat(icon_size),
    );
    icons::paint(painter, icon, icon_rect, text.color32(), t.icon_stroke);
    painter.galley(
        egui::pos2(
            icon_rect.right() + t.space_1,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        text.color32(),
    );
    response
}

/// A count badge: `radius.pill`, `type.caption`, 18 px high, on `color.surface.sunken` in `color.text.secondary`.
pub(crate) fn count_badge(ui: &mut Ui, count: usize) -> Response {
    let t = tokens(ui);
    let text = count.to_string();
    let galley = ui.painter().layout_no_wrap(
        text.clone(),
        theme::font(&t.type_caption),
        t.text_secondary.color32(),
    );
    let size = Vec2::new(
        (galley.size().x + 2.0 * t.space_icon).max(t.size_badge),
        t.size_badge,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
    ui.painter()
        .rect_filled(rect, theme::radius(t.radius_pill), t.surface_sunken.color32());
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        t.text_secondary.color32(),
    );
    response
}

/// The key chips of a shortcut (`Ctrl+Shift+P` → "Ctrl", "Shift", "P" as one chip "Ctrl Shift P"; doc 16a §6.16, §8
/// item 7): `radius.sm`, a 1-px `color.border.default`, `type.caption` in `color.text.tertiary`, `space.2` padding.
pub(crate) fn key_chip(ui: &mut Ui, shortcut: &str) -> Response {
    let t = tokens(ui);
    let text = crate::keys::display(shortcut).replace('+', " ");
    let galley = ui
        .painter()
        .layout_no_wrap(text, theme::font(&t.type_caption), t.text_tertiary.color32());
    let size = Vec2::new(galley.size().x + 2.0 * t.space_2, t.size_badge);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_stroke(
        rect,
        theme::radius(t.radius_sm),
        theme::stroke(t.border_width, t.border_default),
        egui::StrokeKind::Inside,
    );
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        t.text_tertiary.color32(),
    );
    response
}

/// The size of [`caption_chip`] for `text`, and its laid-out text.
pub(crate) fn caption_chip_layout(ui: &Ui, t: &Tokens, text: &str) -> (std::sync::Arc<egui::Galley>, Vec2) {
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        theme::font(&t.type_caption),
        t.text_secondary.color32(),
    );
    let size = galley.size() + Vec2::new(2.0 * t.space_2, t.space_1);
    (galley, size)
}

/// Paints a caption chip laid out by [`caption_chip_layout`] into `rect` (no node: a caller that paints the chip
/// inside another widget names it in that widget's accessible name).
pub(crate) fn paint_caption_chip(
    painter: &egui::Painter,
    t: &Tokens,
    rect: egui::Rect,
    galley: std::sync::Arc<egui::Galley>,
) {
    painter.rect_filled(rect, theme::radius(t.radius_sm), t.surface_sunken.color32());
    painter.galley(
        rect.center() - galley.size() / 2.0,
        galley,
        t.text_secondary.color32(),
    );
}

/// A caption chip, the badge of a tree row and a canvas tab (doc 16a §6.6, §6.16; "QET grid", "generated"):
/// `type.caption` in `color.text.secondary` on `color.surface.sunken`, `radius.sm`; a `Label` node named by the text.
pub(crate) fn caption_chip(ui: &mut Ui, text: &str) -> Response {
    let t = tokens(ui);
    let (galley, size) = caption_chip_layout(ui, &t, text);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let name = text.to_owned();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &name));
    paint_caption_chip(ui.painter(), &t, rect, galley);
    response
}

/// A property row's provenance chip (doc 16a §6.8, the mockup's `.chip`): a pill (`radius.pill`) `size.chip` high
/// with a 1-px `color.border.default`, the source's word in `type.caption` and `colour` (`color.text.tertiary`, or
/// `color.text.secondary` on a selected row), its right edge's middle at `right_centre`. No node: the row's
/// description carries the chip's whole text. Returns its rectangle.
pub(crate) fn paint_provenance_chip(
    ui: &Ui,
    t: &Tokens,
    word: &str,
    colour: Rgba,
    right_centre: egui::Pos2,
) -> egui::Rect {
    let galley = ui
        .painter()
        .layout_no_wrap(word.to_owned(), theme::font(&t.type_caption), colour.color32());
    let size = Vec2::new(galley.size().x + 2.0 * t.space_icon, t.size_chip);
    let rect = egui::Rect::from_min_size(
        egui::pos2(right_centre.x - size.x, right_centre.y - size.y / 2.0),
        size,
    );
    let painter = ui.painter();
    painter.rect_stroke(
        rect,
        theme::radius(t.radius_pill),
        theme::stroke(t.border_width, t.border_default),
        egui::StrokeKind::Inside,
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, colour.color32());
    rect
}

/// An empty state (doc 16a §6.17), centred in the space left: a 32-px icon in `color.text.tertiary`, the line in
/// `type.body.strong`, the sentence in `color.text.secondary`, and at most one action (a Primary button). A `Group`
/// node named by the line. Returns the action's response.
pub(crate) fn empty_state(
    ui: &mut Ui,
    icon: IconId,
    line: &str,
    sentence: &str,
    action: Option<&str>,
) -> Option<Response> {
    let t = tokens(ui);
    let rect = ui.available_rect_before_wrap();
    let id = ui.scope_id().with(("empty-state", line));
    let node = ui.interact(rect, id, Sense::hover());
    node.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, line));
    let mut clicked = None;
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(id), |ui| {
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add_space(t.space_6);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(2.0 * t.icon_md - t.space_3), Sense::hover());
            icons::paint(ui.painter(), icon, r, t.text_tertiary.color32(), t.icon_stroke_lg);
            ui.add_space(t.space_3);
            ui.label(
                RichText::new(line)
                    .font(theme::font(&t.type_body_strong))
                    .color(t.text_primary.color32()),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(sentence)
                        .font(theme::font(&t.type_body))
                        .color(t.text_secondary.color32()),
                )
                .wrap(),
            );
            if let Some(label) = action {
                ui.add_space(t.space_3);
                clicked = Some(button(ui, Kind::Primary, label, true, 0.0));
            }
        });
    });
    clicked
}

/// The frame of a card (doc 16a §6.9): `color.surface.panel`, a 1-px `color.border.default`, `radius.md`, `space.4`
/// padding.
pub(crate) fn card_frame(t: &Tokens) -> egui::Frame {
    egui::Frame::new()
        .fill(t.surface_panel.color32())
        .stroke(theme::stroke(t.border_width, t.border_default))
        .corner_radius(theme::radius(t.radius_md))
        .inner_margin(egui::Margin::same(t.space_4 as i8))
}

/// The status of a card: as of its revision, stale, or failed with its sentence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum CardStatus {
    /// Its preview is of the current revision.
    Fresh,
    /// The model moved since the preview.
    Stale,
    /// The preview failed, with the failure's sentence.
    Failed(String),
}

/// The status of the open card in `shell`.
pub(crate) fn card_status(shell: &ShellModel) -> CardStatus {
    let (Some(card), Some(rev)) = (shell.properties().card(), shell.revision()) else {
        return CardStatus::Fresh;
    };
    match (&card.preview.result, card.preview.status(rev)) {
        (Err(m), _) => CardStatus::Failed(m.clone()),
        (_, PreviewStatus::Stale) => CardStatus::Stale,
        _ => CardStatus::Fresh,
    }
}

/// A card's header row: the title in `type.body.strong`, "as of rev `<r>`" in `type.small` `color.text.tertiary`,
/// then the Stale or Failed status badge (doc 16a §6.23).
pub(crate) fn card_header(ui: &mut Ui, title: &str, rev: Option<u64>, status: &CardStatus) {
    let t = tokens(ui);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        ui.label(
            RichText::new(title)
                .font(theme::font(&t.type_body_strong))
                .color(t.text_primary.color32()),
        );
        if let Some(rev) = rev {
            ui.label(
                RichText::new(strings::format("card-as-of", &[&rev.to_string()]))
                    .font(theme::font(&t.type_small))
                    .color(t.text_tertiary.color32()),
            );
        }
        match status {
            CardStatus::Fresh => {}
            CardStatus::Stale => {
                status_badge(ui, Badge::Stale);
            }
            CardStatus::Failed(_) => {
                status_badge(ui, Badge::Failed);
            }
        }
    });
}

/// A polite live region in `type.small` `color.text.secondary` (a card's summary line), or the failure's sentence in
/// `color.error`.
pub(crate) fn live_line(ui: &mut Ui, text: &str, error: bool) {
    let t = tokens(ui);
    let colour = if error { t.error } else { t.text_secondary };
    let response = ui.add(
        egui::Label::new(
            RichText::new(text)
                .font(theme::font(&t.type_small))
                .color(colour.color32()),
        )
        .wrap(),
    );
    let live = text.to_owned();
    ui.ctx().accesskit_node_builder(response.id, |n| {
        n.set_live(egui::accesskit::Live::Polite);
        n.set_value(live);
    });
}

/// Apply (Primary) and Cancel (Secondary), right-aligned (doc 16a §6.9); Apply dispatches `apply` and Cancel
/// `preview.cancel`.
pub(crate) fn apply_cancel(ui: &mut Ui, apply: &'static str, apply_params: Value, events: &mut Vec<Event>) {
    apply_cancel_with(ui, (apply, apply_params), ("preview.cancel", json!({})), events);
}

/// Apply (Primary) and Cancel (Secondary), right-aligned in the platform's order (doc 16a §6.12: Windows Apply then
/// Cancel, macOS and Linux Cancel then Apply), each dispatching its action with its parameters.
pub(crate) fn apply_cancel_with(
    ui: &mut Ui,
    apply: (&'static str, Value),
    cancel: (&'static str, Value),
    events: &mut Vec<Event>,
) {
    let t = tokens(ui);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = t.space_3;
        // Right to left: the last button of the reading order is added first.
        let order: [bool; 2] = if cfg!(target_os = "windows") {
            [false, true]
        } else {
            [true, false]
        };
        for apply_now in order {
            if apply_now {
                if button(ui, Kind::Primary, strings::text("card-apply"), true, 0.0).clicked() {
                    events.push(Event::Action(apply.0, apply.1.clone()));
                }
            } else if button(ui, Kind::Secondary, strings::text("card-cancel"), true, 0.0).clicked() {
                events.push(Event::Action(cancel.0, cancel.1.clone()));
            }
        }
    });
}

/// What an inline card shows: its title and its summary line.
pub(crate) struct InlineCard<'a> {
    /// The title ("Reorder pages").
    pub title: &'a str,
    /// The summary line (the polite live region).
    pub summary: &'a str,
    /// The action Apply dispatches (`preview.apply`, `navigator.apply`).
    pub apply: &'static str,
    /// Whether the "Preview open" strip stands in for the card while the Preview tab shows it (not the table card).
    pub strip: bool,
}

/// An inline card (doc 16a §6.9, §6.23) of the open card of `shell`: the header with the revision and the status badge,
/// the summary line (or the failure's sentence in `color.error`), `body`, then Apply (Primary) and Cancel. While the
/// Preview tab shows the card, the "Preview open" strip stands in for it ([`card_strip`]).
pub(crate) fn inline_card(
    ui: &mut Ui,
    shell: &ShellModel,
    card: InlineCard<'_>,
    events: &mut Vec<Event>,
    body: impl FnOnce(&mut Ui, &mut Vec<Event>),
) {
    let status = card_status(shell);
    let strip_line = match &status {
        CardStatus::Failed(m) => strings::format("card-strip-failed", &[card.title, m]),
        _ => format!("{} · {}", card.title, card.summary),
    };
    // WP-S2.09 part 3 (§8.1): while the Preview tab shows the card, the strip stands in for it.
    if card.strip && card_strip(ui, shell, &strip_line, events) {
        return;
    }
    let t = tokens(ui);
    let rev = shell.properties().card().map(|c| c.preview.base_rev);
    card_frame(&t).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = t.space_2;
        card_header(ui, card.title, rev, &status);
        match &status {
            CardStatus::Failed(m) => live_line(ui, m, true),
            _ => live_line(ui, card.summary, false),
        }
        body(ui, events);
        apply_cancel(ui, card.apply, Value::Null, events);
    });
}

/// The action Apply dispatches for a card of `origin`: the navigator's own for a reorder, quick-fix or table card.
pub(crate) fn apply_action(origin: &CardOrigin) -> &'static str {
    match origin {
        // WP-S2.09 part 2: the quick-fix card is the Messages pane's, applied like the reorder card.
        CardOrigin::Reorder { .. } | CardOrigin::QuickFix { .. } => "navigator.apply",
        _ => "preview.apply",
    }
}

/// The "Preview open" strip of an origin pane while the Preview tab shows its card (doc 16a §6.23): one line
/// `size.card_strip` high on `color.card.strip`, `radius.md`, `space.3` padding: the `eye` icon, "Preview open:
/// `<line>`" in `type.small` with an ellipsis and a tooltip when cut, then Apply (Subtle, its label in `color.accent`
/// SemiBold), Cancel and Show (Subtle). `false` (and nothing painted) while the tab is not open.
pub(crate) fn card_strip(ui: &mut Ui, shell: &ShellModel, line: &str, events: &mut Vec<Event>) -> bool {
    let Some(card) = shell.properties().card().filter(|_| shell.preview_shown()) else {
        return false;
    };
    let t = tokens(ui);
    let text = strings::format("preview-open-chip", &[line]);
    egui::Frame::new()
        .fill(t.card_strip.color32())
        .corner_radius(theme::radius(t.radius_md))
        .inner_margin(egui::Margin::symmetric(t.space_3 as i8, t.space_1 as i8))
        .show(ui, |ui| {
            ui.set_min_height(t.size_card_strip - 2.0 * t.space_1);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.space_2;
                let (r, _) = ui.allocate_exact_size(Vec2::splat(t.icon_sm), Sense::hover());
                icons::paint(
                    ui.painter(),
                    IconId::eye,
                    r,
                    t.text_secondary.color32(),
                    t.icon_stroke,
                );
                // The buttons first from the right, so the text takes what is left and is cut with an ellipsis.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = t.space_1;
                    let small = |ui: &mut Ui, key: &str, accent: bool| {
                        let (font, colour) = if accent {
                            (&t.type_label, t.accent)
                        } else {
                            (&t.type_small, t.text_primary)
                        };
                        let font = if accent {
                            theme::font(&theme::TypeToken {
                                semibold: true,
                                ..t.type_small
                            })
                        } else {
                            theme::font(font)
                        };
                        let b = Button::new(
                            RichText::new(strings::text(key))
                                .font(font)
                                .color(colour.color32()),
                        )
                        .fill(theme::NONE)
                        .stroke(theme::stroke(0.0, t.border_default))
                        .corner_radius(theme::radius(t.radius_sm))
                        .min_size(Vec2::new(0.0, t.size_control_inline));
                        let r = ui.add(b);
                        crate::command_bar::state_overlay(ui, &t, &r, true, t.radius_sm);
                        r
                    };
                    if small(ui, "preview-show", false).clicked() {
                        events.push(Event::Action("preview.show", json!({ "on": true })));
                    }
                    if small(ui, "preview-cancel", false).clicked() {
                        events.push(Event::Action("preview.cancel", json!({})));
                    }
                    if small(ui, "preview-apply", true).clicked() {
                        events.push(Event::Action(apply_action(&card.origin), Value::Null));
                    }
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let response = ui.add(
                            egui::Label::new(
                                RichText::new(&text)
                                    .font(theme::font(&t.type_small))
                                    .color(t.text_primary.color32()),
                            )
                            .truncate(),
                        );
                        let live = text.clone();
                        ui.ctx().accesskit_node_builder(response.id, |n| {
                            n.set_live(egui::accesskit::Live::Polite);
                            n.set_value(live);
                        });
                    });
                });
            });
        });
    true
}

/// A collapsible section header in the style of the property categories (doc 16a §6.8, §6.23): `size.row` high, a
/// chevron, the title in `type.small` SemiBold `color.text.secondary`, and a count badge. A button node named by the
/// title with its expanded state. Returns whether the section is open.
pub(crate) fn section_header(
    ui: &mut Ui,
    id: egui::Id,
    title: &str,
    count: usize,
    default_open: bool,
) -> bool {
    let t = tokens(ui);
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(default_open);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, t.size_row), Sense::click());
    if response.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_temp(id, open));
    }
    // The accessible name carries the count the badge paints ("Tags (11)").
    let name = format!("{title} ({count})");
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    ui.ctx()
        .accesskit_node_builder(response.id, |n| n.set_expanded(open));
    crate::command_bar::state_overlay(ui, &t, &response, true, t.radius_sm);
    let painter = ui.painter();
    let chevron = if open {
        IconId::chevron_down
    } else {
        IconId::chevron_right
    };
    let c = egui::Rect::from_min_size(
        egui::pos2(rect.left() + t.space_1, rect.center().y - t.icon_sm / 2.0),
        Vec2::splat(t.icon_sm),
    );
    icons::paint(painter, chevron, c, t.text_secondary.color32(), t.icon_stroke);
    let font = theme::font(&theme::TypeToken {
        semibold: true,
        ..t.type_small
    });
    let galley = painter.layout_no_wrap(title.to_owned(), font, t.text_secondary.color32());
    let tx = c.right() + t.space_icon;
    let text_w = galley.size().x;
    painter.galley(
        egui::pos2(tx, rect.center().y - galley.size().y / 2.0),
        galley,
        t.text_secondary.color32(),
    );
    // The count badge after the title.
    let badge_rect = egui::Rect::from_min_max(
        egui::pos2(tx + text_w + t.space_2, rect.top()),
        rect.right_bottom(),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(badge_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.set_clip_rect(rect);
    count_badge(&mut child, count);
    open
}
