// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 6 (§11, FR-MAC-006, FT-487): what the Macros and Libraries navigators show besides their rows.
//!
//! **The Libraries navigator** has an Update… button (Secondary) above its rows: `libraries.update` previews
//! `library.update` of the marked copies, else the copy under the cursor, else every copy with an update, and opens
//! the bulk card in the Preview tab. The rows' copy state is the API's column and badge, in words.
//!
//! **The Macros navigator** has a detail block under its rows for the macro under the cursor: its name
//! (`type.body.strong`), version and library, description, "No preview" in place of a thumbnail (a thumbnail is never
//! rendered on the fly), the devices, the variables and Insert macro… (Primary), which arms the macro tool.
//!
//! **Accessibility.** The detail block is a `Group` named after the macro; each line is a label; the buttons are named
//! by their text (NFR-A11Y-01).

use egui::{RichText, Ui};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;
use crate::components::{self, Kind};
use crate::theme;

/// Paints the parts of navigator `kind` that come before its rows (the Libraries navigator's Update…), and the
/// Macros navigator's detail block as a bottom panel; nothing for another kind.
pub(crate) fn show(ui: &mut Ui, shell: &ShellModel, kind: NavKind, events: &mut Vec<Event>) {
    match kind {
        NavKind::Libraries => update_bar(ui, shell, events),
        NavKind::Macros => {
            if let Some(detail) = shell.macro_detail() {
                let t = components::tokens(ui);
                let frame = egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(t.space_4 as i8, t.space_2 as i8));
                egui::Panel::bottom(ui.scope_id().with("macro-detail"))
                    .resizable(false)
                    .frame(frame)
                    .show(ui, |ui| macro_detail(ui, detail, events));
            }
        }
        _ => {}
    }
}

/// The Libraries navigator's Update… button.
fn update_bar(ui: &mut Ui, shell: &ShellModel, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let enabled = ew_viewmodel::actions::find("libraries.update")
        .is_some_and(|m| ew_viewmodel::actions::enabled_with(m, shell, &serde_json::Map::new()).is_ok());
    ui.horizontal(|ui| {
        ui.add_space(t.space_4);
        if components::button(
            ui,
            Kind::Secondary,
            strings::text("action-libraries-update"),
            enabled,
            0.0,
        )
        .clicked()
        {
            events.push(Event::Action("libraries.update", json!({})));
        }
    });
    ui.add_space(t.space_2);
}

/// The detail block of a macro.
fn macro_detail(ui: &mut Ui, d: &ew_viewmodel::ew_api::output::MacroDetail, events: &mut Vec<Event>) {
    let t = components::tokens(ui);
    let id = ui.scope_id().with("macro-detail-group");
    let rect = ui.available_rect_before_wrap();
    let group = ui.interact(rect, id, egui::Sense::hover());
    let name = d.name.clone();
    group.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, &name));
    ui.scope_builder(egui::UiBuilder::new().accessibility_parent(id), |ui| {
        ui.spacing_mut().item_spacing.y = t.space_1;
        let line = |ui: &mut Ui, text: &str, font: &theme::TypeToken, colour: theme::Rgba| {
            ui.add(
                egui::Label::new(
                    RichText::new(text)
                        .font(theme::font(font))
                        .color(colour.color32()),
                )
                .wrap(),
            );
        };
        line(ui, &d.name, &t.type_body_strong, t.text_primary);
        line(
            ui,
            &strings::format("macro-version", &[&d.version, &d.library]),
            &t.type_small,
            t.text_secondary,
        );
        if !d.description.is_empty() {
            line(ui, &d.description, &t.type_small, t.text_primary);
        }
        // §11: the thumbnail, or "No preview" (never rendered on the fly).
        line(
            ui,
            strings::text("macro-no-preview"),
            &t.type_small,
            t.text_tertiary,
        );
        for (title, items) in [
            (strings::text("macro-devices"), &d.devices),
            (strings::text("macro-variables"), &d.variables),
        ] {
            if items.is_empty() {
                continue;
            }
            line(
                ui,
                &format!("{title} ({})", items.len()),
                &t.type_small,
                t.text_secondary,
            );
            for item in items {
                line(ui, item, &t.type_small, t.text_primary);
            }
        }
        if d.reference.is_some() {
            ui.add_space(t.space_1);
            if components::button(
                ui,
                Kind::Primary,
                strings::text("action-macros-insert"),
                true,
                0.0,
            )
            .clicked()
            {
                events.push(Event::Action("macros.insert", json!({})));
            }
        }
    });
}
