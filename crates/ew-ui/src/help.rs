// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the window names under the pointer for F1 (`ew_viewmodel::help`): a widget with a topic (a button of the
//! command bar, a menu item, a row of a list or of Properties, a part of a screen) offers it with the words it is named
//! by while the pointer is on it. Of the offers of a frame the smallest rectangle wins, so a row answers before the
//! pane it stands in whatever the order they are painted in. The frame hands the winner to the shell
//! (`ShellModel::set_help_under`), where F1 and the status bar read it.

use egui::{Context, Id, Rect, Response, Ui};
use ew_viewmodel::help::HelpUnder;

/// An offer of this frame: its rectangle's area and what it names.
#[derive(Clone)]
struct Offer {
    area: f32,
    under: HelpUnder,
}

/// Where the frame's offer is kept.
fn id() -> Id {
    Id::unique("ew-help-under")
}

/// Offers what `name` gives (the topic, then its words) for `rect` of `ui` while the pointer is on it (and on no
/// layer above it); `name` runs only then.
pub(crate) fn offer(ui: &Ui, rect: Rect, name: impl FnOnce() -> (String, String)) {
    if !ui.rect_contains_pointer(rect) {
        return;
    }
    keep(ui.ctx(), rect.area(), name);
}

/// [`offer`] for a widget's response.
pub(crate) fn offer_for(response: &Response, name: impl FnOnce() -> (String, String)) {
    if response.contains_pointer() {
        keep(&response.ctx, response.rect.area(), name);
    }
}

/// Keeps the offer when it is the smallest of the frame so far.
fn keep(ctx: &Context, area: f32, name: impl FnOnce() -> (String, String)) {
    let smaller = ctx.data(|d| d.get_temp::<Offer>(id()).is_none_or(|o| area <= o.area));
    if smaller {
        let (topic, subject) = name();
        ctx.data_mut(|d| {
            d.insert_temp(
                id(),
                Offer {
                    area,
                    under: HelpUnder { topic, subject },
                },
            );
        });
    }
}

/// Takes the frame's winning offer.
pub(crate) fn take(ctx: &Context) -> Option<HelpUnder> {
    ctx.data_mut(|d| {
        let offer = d.get_temp::<Offer>(id());
        d.remove::<Offer>(id());
        offer
    })
    .map(|o| o.under)
}
