// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The inline preview card of File, Import, Parts table… (FT-410): a card floating at the top centre while the card's
//! origin is an import. WP-S2.17 part 4 (doc 16a §6.9, §6.23): "Import parts" with its revision and the Stale or Failed
//! status badge, the counts of the preview ("parts.csv: 29 created, 0 skipped, 0 errors") as its summary, and Apply
//! (Primary) and Cancel; `elevation.1`, as it floats.
//!
//! **Input.** Apply and Cancel dispatch `preview.apply` and `preview.cancel`; Ctrl+Enter applies the card from
//! anywhere (`App::route_keys`). **Accessibility.** The summary is a polite live region.

use egui::{Align2, Id};
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::strings;

use crate::app::Event;
use crate::components::{InlineCard, inline_card};

/// Paints the import card when it is open.
pub fn show(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(card) = shell.properties().card() else {
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
    let t = crate::theme::current(ctx);
    let summary = strings::format(
        "parts-card-line",
        &[source, &created.to_string(), &skipped.to_string(), "0"],
    );
    egui::Area::new(Id::unique("parts-card"))
        .anchor(
            Align2::CENTER_TOP,
            [0.0, t.size_tab_row + t.size_command_row + t.space_4],
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_width(t.size_dialog_small);
            egui::Frame::new()
                .shadow(crate::theme::shadow(&t.elevation_1))
                .corner_radius(crate::theme::radius(t.radius_md))
                .show(ui, |ui| {
                    let inline = InlineCard {
                        title: strings::text("parts-card-title"),
                        summary: &summary,
                        apply: "preview.apply",
                        strip: true,
                    };
                    inline_card(ui, shell, inline, events, |_, _| {});
                });
        });
}
