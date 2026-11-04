// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The start-up dialog (WP-S6.05 D11, FR-UI-050's 0.1.0 part): the unsaved work of an earlier session and the crash
//! log of a session that ended with a panic, in the dialog frame of WP-S2.17 part 4 ([`crate::dialog`]). The title, the
//! crash line and the buttons are the offer's specification ([`StartupOffer::spec`], which `ui.describe` lists too);
//! between them one row per project, each with its own Recover when there are several, and the refusal of the last
//! answer. Escape is Not now (`App::route_keys`).

use egui::{RichText, Ui};
use ew_viewmodel::dialogs::DialogAnswer;
use ew_viewmodel::key;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::startup::StartupOffer;
use ew_viewmodel::strings;
use serde_json::json;

use crate::app::Event;

/// Paints the start-up offer, if one is pending, as a modal dialog.
pub(crate) fn show(ctx: &egui::Context, shell: &ShellModel, events: &mut Vec<Event>) {
    let Some(offer) = shell.startup_offer() else {
        return;
    };
    let spec = offer.spec();
    let mut row_clicked = None;
    let clicked = crate::dialog::show_sized(ctx, &spec, None, false, |ui: &mut Ui| {
        let t = crate::theme::current(ui.ctx());
        let several = offer.projects.len() > 1;
        for (i, p) in offer.projects.iter().enumerate() {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(StartupOffer::project_row(p))
                        .font(crate::theme::font(&t.type_body))
                        .color(t.text_primary.color32()),
                );
                if several && ui.button(strings::text(key!("startup-recover"))).clicked() {
                    row_clicked = Some(i + 1);
                }
            });
        }
        if let Some(refusal) = &offer.refusal {
            ui.colored_label(t.error.color32(), refusal);
        }
    });
    if let Some(n) = row_clicked {
        events.push(Event::Action("startup.recover", json!({ "project": n })));
    } else if let Some(DialogAnswer::Action(action, params)) = clicked {
        events.push(Event::Action(action, params));
    }
}
