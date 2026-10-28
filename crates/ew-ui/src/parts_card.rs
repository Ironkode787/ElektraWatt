// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The inline preview card of File, Import, Parts table... (FT-410): a window anchored at the top centre while the
//! card's origin is an import. It shows the counts of the preview ("parts.csv: 29 created, 0 skipped, 0 errors"),
//! the stale badge or the failed line like the find pane's card, and Apply and Cancel.
//!
//! **Input.** Apply and Cancel dispatch `preview.apply` and `preview.cancel`; Ctrl+Enter applies the card from
//! anywhere (`App::route_keys`). **Accessibility.** The line is a polite live region.

use egui::{Align2, Window};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;
use ew_viewmodel::tools::PreviewStatus;
use serde_json::json;

use crate::app::Event;

/// Paints the import card when it is open.
pub fn show(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let (Some(card), Some(rev)) = (shell.properties().card(), shell.revision()) else {
        return;
    };
    let CardOrigin::Import {
        source,
        created,
        skipped,
    } = &card.origin
    else {
        return;
    };
    Window::new(strings::text("parts-card-title"))
        .anchor(Align2::CENTER_TOP, [0.0, 60.0])
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let mut line = strings::format(
                "parts-card-line",
                &[source, &created.to_string(), &skipped.to_string(), "0"],
            );
            match (&card.preview.result, card.preview.status(rev)) {
                (Err(m), _) => line = strings::format("card-failed", &[m]),
                (_, PreviewStatus::Stale) => line.push_str(&format!(" [{}]", strings::text("card-stale"))),
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
                    events.push(Event::Action("preview.apply", json!({})));
                }
                if ui.button(strings::text("card-cancel")).clicked() {
                    events.push(Event::Action("preview.cancel", json!({})));
                }
            });
        });
}
