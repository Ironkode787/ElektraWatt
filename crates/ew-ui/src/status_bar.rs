// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The status bar (doc 16a §5.8): `size.status` high on `color.surface.chrome` under a 1-px
//! `color.border.default` line. The state stands at the left; the findings, the job, the selection, the cursor, the
//! grid, the zoom, the save state, the renderer and the notifications stand at the right in that order
//! (`ew_viewmodel::status::bar`); the revision stands beside the state, in its short words where the bar is too narrow for
//! every field's words. Each field is `size.status_field` high with `space.3` padding in `type.small`; an
//! exception stands out (the findings in their severity's colour, Saved in `color.success`, a CPU renderer in
//! `color.warning`). A card waiting for Apply shows the Preview, the findings open Messages, the job opens Jobs, the
//! notifications open the log: those are buttons with the hover overlay, the rest are labels.

use egui::{Align, Layout, Rect, Sense, Ui, pos2, vec2};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::status::{StatusField, StatusKind, Tone};
use serde_json::json;

use crate::app::Event;
use crate::command_bar::{paint_icon, state_overlay};
use crate::components::Tip as _;
use crate::theme::{self, Tokens, TypeToken};

/// What a click on a field of `kind` asks for.
fn click(kind: StatusKind) -> Option<Event> {
    match kind {
        // A card waiting for Apply: the Preview tab in front.
        StatusKind::Waiting => Some(Event::Action("preview.show", json!({ "on": true }))),
        StatusKind::Findings => Some(Event::Action("messages.show", json!({ "on": true }))),
        StatusKind::Job => Some(Event::Action("jobs.show", json!({ "on": true }))),
        StatusKind::Notifications => Some(Event::Action("notifications.show", json!({}))),
        // The revision overview, every revision with its date and what changed.
        StatusKind::Revision => Some(Event::Action("lists.revisions", json!({}))),
        // Set up merge driver.
        StatusKind::Driver => Some(Event::Action("git.setup", json!({}))),
        _ => None,
    }
}

/// The icon before a field's text (doc 16a §5.8), if any. One rule for the save state, every state has its icon
/// and the icon takes the field's colour: Saved a check in `color.success`, Unsaved changes a dot in
/// `color.text.primary`, Read-only a lock in `color.warning`.
fn icon(field: &StatusField) -> Option<&'static str> {
    match (field.kind, field.tone) {
        (StatusKind::Waiting, _) => Some("eye"),
        (StatusKind::Findings, Tone::Error) => Some("alert-circle"),
        (StatusKind::Findings, Tone::Warning) => Some("alert-triangle"),
        (StatusKind::Findings, _) => Some("info-circle"),
        (StatusKind::Save, Tone::Success) => Some("check"),
        (StatusKind::Save, Tone::Strong) => Some("circle-dot"),
        (StatusKind::Save, Tone::Warning) => Some("lock"),
        (StatusKind::Notifications, _) => Some("bell"),
        (StatusKind::Revision, _) => Some("versions"),
        (StatusKind::Driver, _) => Some("alert-triangle"),
        _ => None,
    }
}

/// The text a field shows: the notifications field shows the count beside its bell (its name keeps the words); the
/// revision field, in a bar too `narrow` for every field's words, its short words ("V2.0 open"), the whole in its
/// tooltip.
fn shown_text(shell: &ShellModel, field: &StatusField, narrow: bool) -> String {
    match field.kind {
        StatusKind::Notifications => shell.notifications().len().to_string(),
        StatusKind::Revision if narrow => {
            ew_viewmodel::status::revision_words(shell).map_or_else(|| field.text.clone(), |(_, short)| short)
        }
        _ => field.text.clone(),
    }
}

/// The width a field takes with `text`: its words, its icon and its `space.3` padding.
fn width(ui: &Ui, t: &Tokens, f: &StatusField, text: String) -> f32 {
    let (colour, ty) = look(t, f.tone);
    let galley = ui.painter().layout_no_wrap(text, theme::font(&ty), colour);
    let icon_w = icon(f).map_or(0.0, |_| t.icon_sm + t.space_icon);
    galley.size().x + icon_w + 2.0 * t.space_3
}

/// The colour and the type of a tone.
fn look(t: &Tokens, tone: Tone) -> (egui::Color32, TypeToken) {
    let strong = TypeToken {
        semibold: true,
        ..t.type_small
    };
    match tone {
        Tone::Normal => (t.text_secondary.color32(), t.type_small),
        Tone::Error => (t.error.color32(), strong),
        Tone::Warning => (t.warning.color32(), t.type_small),
        Tone::Success => (t.success.color32(), t.type_small),
        Tone::Strong => (t.text_primary.color32(), t.type_small),
        Tone::Muted => (t.text_tertiary.color32(), t.type_small),
    }
}

/// Paints one field into `ui`'s layout; a click on a clickable field pushes its event.
fn field(
    ui: &mut Ui,
    t: &Tokens,
    shell: &ShellModel,
    f: &StatusField,
    narrow: bool,
    events: &mut Vec<Event>,
) {
    let (colour, ty) = look(t, f.tone);
    let font = theme::font(&ty);
    let text = shown_text(shell, f, narrow);
    let tip = if f.kind == StatusKind::Revision && text != f.text {
        format!(
            "{}
{}",
            f.text, f.tip
        )
    } else {
        f.tip.clone()
    };
    let galley = ui.painter().layout_no_wrap(text, font, colour);
    let icon = icon(f);
    let icon_w = icon.map_or(0.0, |_| t.icon_sm + t.space_icon);
    let size = vec2(galley.size().x + icon_w + 2.0 * t.space_3, t.size_status_field);
    let event = click(f.kind);
    let sense = if event.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let name = f.text.clone();
    let role = if event.is_some() {
        egui::Role::Button
    } else {
        egui::Role::Label
    };
    response.widget_info(|| egui::WidgetInfo::labeled(role, true, &name));
    if event.is_some() {
        state_overlay(ui, t, &response, true, t.radius_sm);
    }
    let mut x = rect.left() + t.space_3;
    if let Some(key) = icon {
        let r = Rect::from_min_size(
            pos2(x, rect.center().y - t.icon_sm / 2.0),
            vec2(t.icon_sm, t.icon_sm),
        );
        paint_icon(ui, key, r, colour, t.icon_stroke);
        x += icon_w;
    }
    ui.painter()
        .galley(pos2(x, rect.center().y - galley.size().y / 2.0), galley, colour);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            theme::radius(t.radius_sm),
            theme::stroke(t.focus_width, t.focus),
            egui::StrokeKind::Inside,
        );
    }
    let response = if tip.is_empty() {
        response
    } else {
        response.tip(tip.as_str())
    };
    if response.clicked()
        && let Some(e) = event
    {
        events.push(e);
    }
}

/// Paints the status bar of `shell` into `ui` (the bottom panel); `hide_job` leaves the job field out (a job younger
/// than 200 ms).
pub fn show(ui: &mut Ui, shell: &ShellModel, hide_job: bool, events: &mut Vec<Event>) {
    let t = theme::current(ui.ctx());
    let rect = ui.max_rect();
    ui.painter().hline(
        rect.x_range(),
        rect.top() + t.border_width / 2.0,
        theme::stroke(t.border_width, t.border_default),
    );
    let fields = ew_viewmodel::status::bar(shell, hide_job);
    // The state and the revision stand at the left, the rest at the right.
    let left = fields
        .iter()
        .take_while(|f| matches!(f.kind, StatusKind::State | StatusKind::Revision))
        .count();
    let (left, rest) = fields.split_at(left);
    // Where the bar is too narrow for every field's words, the revision says its short words.
    let narrow = fields
        .iter()
        .map(|f| width(ui, &t, f, shown_text(shell, f, false)))
        .sum::<f32>()
        > rect.width();
    // The fields' own `space.3` padding is the bar's gutter.
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.horizontal_centered(|ui| {
            for f in left {
                field(ui, &t, shell, f, narrow, events);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for f in rest.iter().rev() {
                    field(ui, &t, shell, f, narrow, events);
                }
            });
        });
    });
}
