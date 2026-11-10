// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start-up dialog (FR-UI-050's 0.1.0 part): the unsaved work of an earlier session and the crash
//! log of a session that ended with a panic, in the dialog frame of WP-S2.17 part 4 ([`crate::dialog`]). The title, the
//! crash line and the buttons are the offer's specification ([`StartupOffer::spec`], which `ui.describe` lists too);
//! between them one row per project with its path under it (with several projects a choice, which the Primary Recover
//! opens), and the refusal of the last answer. The Primary takes the first focus (doc 16a §6.12); Escape is
//! Not now (`App::route_keys`).

use egui::{Id, RichText, Ui};
use ew_viewmodel::dialogs::DialogAnswer;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::startup::StartupOffer;
use serde_json::json;

use crate::app::Event;

/// The memory of the dialog: whether it was shown in the last frame with the same choice, and the project chosen
/// (1-based).
fn state_id() -> Id {
    Id::unique("ew-startup-offer")
}

/// Paints the start-up offer, if one is pending, as a modal dialog.
pub(crate) fn show(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(offer) = shell.startup_offer() else {
        ctx.data_mut(|d| d.remove::<(bool, usize)>(state_id()));
        return;
    };
    let (was_shown, chosen) = ctx
        .data(|d| d.get_temp::<(bool, usize)>(state_id()))
        .unwrap_or((false, 1));
    let chosen = chosen.clamp(1, offer.projects.len().max(1));
    let mut spec = offer.spec();
    // The Primary Recover opens the project chosen in the list.
    if !offer.projects.is_empty() {
        spec.primary.answer = DialogAnswer::Action("startup.recover", json!({ "project": chosen }));
    }
    let mut choice = chosen;
    let clicked = crate::dialog::show_sized(ctx, &spec, None, !was_shown, |ui: &mut Ui| {
        let t = crate::theme::current(ui.ctx());
        let several = offer.projects.len() > 1;
        for (i, p) in offer.projects.iter().enumerate() {
            let row = RichText::new(StartupOffer::project_row(p))
                .font(crate::theme::font(&t.type_body))
                .color(t.text_primary.color32());
            if several {
                if ui.radio(choice == i + 1, row).clicked() {
                    choice = i + 1;
                }
            } else {
                ui.label(row);
            }
            // D11, doc 05 §8.3: the project's path under its row.
            ui.label(
                RichText::new(p.path.to_string_lossy())
                    .font(crate::theme::font(&t.type_small))
                    .color(t.text_secondary.color32()),
            );
        }
        if let Some(refusal) = &offer.refusal {
            ui.colored_label(t.error.color32(), refusal);
        }
    });
    // A new choice gives the Primary the focus again (as at the first show), so Enter recovers that project.
    ctx.data_mut(|d| d.insert_temp(state_id(), (choice == chosen, choice)));
    if let Some(DialogAnswer::Action(action, params)) = clicked {
        events.push(Event::Action(action, params));
    }
}
