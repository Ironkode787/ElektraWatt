// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where the outcome of an act of the window is said (architecture §9.1): the one function that runs a step and
//! places its failure or refusal, in the banner, the open dialog, a toast and the status line, or nowhere when the
//! screen shows it already (one fact in one place, doc 16a P3). The window calls it for every act it dispatches, and
//! the headless script runner for every step, so a script sees what a person sees.

use super::{Outcome, ShellModel};
use crate::{key, strings};

/// The banner [`ShellModel::run_said`] raised for a failure, while it stands: the next failure replaces it, where a
/// banner the shell raised itself keeps precedence; with the action that failed, whose next success clears it.
#[derive(Default)]
pub(super) struct Reported {
    /// The banner's text.
    message: Option<String>,
    /// The action whose failure raised it: the open palette form's when the form ran it.
    by: Option<String>,
    /// Where the step running now said its failure or refusal itself ([`ShellModel::said_at`]).
    placed: Option<Said>,
}

/// A place on screen where a step says its failure or refusal itself, so that it is not said again (doc 16a P3).
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Said {
    /// Under the Properties row of this key.
    Row(String),
    /// Under the fields of the palette's parameter form.
    Form,
    /// Under the rack of a station's view.
    Station,
    /// With the symbol editor's draft.
    Editor,
    /// In a toast that offers the way on (a refusal for a part's missing datum: Edit part…).
    Toast,
}

impl ShellModel {
    /// Records that the step running now said its failure or refusal at `place`.
    pub(super) fn said_at(&mut self, place: Said) {
        self.reported.placed = Some(place);
    }

    /// Shows `message` under the Properties row `row`, and records that it is said there.
    pub(super) fn error_under_row(&mut self, row: impl Into<String>, message: impl AsRef<str>) {
        let row = row.into();
        self.props.set_error(row.clone(), message);
        self.said_at(Said::Row(row));
    }

    /// Runs `step` on the shell, says its outcome where the person looks and returns it. A banner that changed across
    /// the step was the shell's own (raised, replaced or cleared for its own reason): the banner this raised before is
    /// then forgotten. `drew_row` answers whether the Properties pane drew the row of a key in its last frame (a
    /// refusal it shows under that row is not said again; headless no row is drawn).
    pub fn run_said(
        &mut self,
        action: Option<&str>,
        step: impl FnOnce(&mut ShellModel) -> Outcome,
        drew_row: impl Fn(&str) -> bool,
    ) -> Outcome {
        let before = self.banner().map(str::to_owned);
        // The palette form's run is reported as its action, and the palette's run of
        // its selected action without a form as that action.
        let form = self.palette().and_then(|p| match &p.form {
            Some(f) => Some(f.action),
            None => match self.palette_entries().get(p.selected)?.run {
                crate::actions::PaletteRun::Action(id) => Some(id),
                _ => None,
            },
        });
        // What the screen shows before the step, so that a refusal the step said there itself is not said again.
        let toast = self.notifications().entries().next().map(|n| n.serial);
        let notice = self.tool_notice().map(str::to_owned);
        let dialog = self.dialog().cloned();
        let generation = self.dialog_generation();
        self.reported.placed = None;
        let outcome = step(self);
        let said_outcome = outcome.clone();
        let banner_changed = self.banner() != before.as_deref();
        if banner_changed {
            self.reported.message = None;
        }
        let said = banner_changed
            || self.notifications().entries().next().map(|n| n.serial) != toast
            || self.tool_notice().is_some_and(|n| Some(n) != notice.as_deref())
            || (dialog.is_some()
                && self.dialog().is_some()
                && (self.dialog_generation() != generation || self.dialog() != dialog.as_ref()));
        self.report(outcome, action.or(form), said, drew_row);
        said_outcome
    }

    /// A failure or incomplete Cut refusal becomes a banner when the shell shows none of its own; a banner this raised
    /// before is replaced by the new failure (a second refusal kept the first's banner, which named the wrong
    /// point). One fact in one place (doc 16a P3): a refusal the navigator's inline field shows under itself (an F2
    /// tag's) or that the Properties pane shows under a drawn row (`action` a
    /// `props.*` one whose dispatch set the row's error) is not made a banner as well.
    ///
    /// A failure the open palette form shows under its fields is not made a banner either, and the
    /// banner this raised for an action's failure clears when that action next succeeds.
    ///
    /// Any other refusal is said where the person looks ([`ShellModel::say_refusal`]: beside the open dialog's buttons,
    /// else in a toast and the status line), unless the step said it on screen itself (`said`: a banner, a toast, the
    /// HUD's notice, the dialog's own line) or one of the places above shows it (a button that is not greyed
    /// and was refused stayed silent).
    fn report(
        &mut self,
        outcome: Outcome,
        action: Option<&str>,
        said: bool,
        drew_row: impl Fn(&str) -> bool,
    ) {
        if self.nav_rename_error().is_some() {
            return;
        }
        let ours = self.reported.message.is_some() && self.banner() == self.reported.message.as_deref();
        let mut refused = false;
        let message = match outcome {
            Outcome::Failed { message } => message,
            Outcome::Refused { reason: reason_key } if reason_key == key!("reason-cut-incomplete-items") => {
                strings::render(&reason_key)
            }
            Outcome::Refused { reason: reason_key } => {
                refused = true;
                strings::render(&reason_key)
            }
            // An action that opened its parameter form has not run: its banner stays.
            Outcome::Done
                if ours
                    && action.is_some()
                    && self.reported.by.as_deref() == action
                    && !self
                        .palette()
                        .and_then(|p| p.form.as_ref())
                        .is_some_and(|f| Some(f.action) == action) =>
            {
                self.dismiss_banner();
                self.reported.message = None;
                self.reported.by = None;
                return;
            }
            _ => return,
        };
        // What the step said itself, under the palette's form, under a drawn Properties row (a `props.*` action's),
        // under a station's rack, with the symbol editor's draft or in a toast that offers the way on, is not made a
        // banner as well (P3).
        match self.reported.placed.take() {
            Some(Said::Row(row)) if action.is_some_and(|a| a.starts_with("props.")) && drew_row(&row) => {
                return;
            }
            Some(Said::Form | Said::Station | Said::Editor | Said::Toast) => return,
            _ => {}
        }
        if refused {
            // The pointer's moves over a canvas (`canvas.pointer`, sent while it moves) are no act of the person.
            if !said && action != Some("canvas.pointer") && self.tool_notice() != Some(message.as_str()) {
                self.say_refusal(message);
            }
            return;
        }
        // A state banner lies under a message: the message shows over it.
        if self.message_banner().is_none() || ours {
            self.show_banner(&message);
            self.reported.message = Some(message);
            self.reported.by = action.map(str::to_owned);
        }
    }
}
