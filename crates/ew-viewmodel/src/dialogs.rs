// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The texts and buttons of every modal dialog of the shell (WP-S2.17 part 4, §8 item 1; doc 16a §6.12): one
//! [`DialogSpec`] per [`Dialog`], so the window's dialog frame and `ui.describe` show the same title, body and buttons,
//! and the rules of the design language are tested headless (CP1).
//!
//! **Rules** (doc 16a §6.2, §6.12, §8 item 9). Exactly one Primary, the button Enter presses; the Primary is never
//! destructive: in the confirmation of a destructive step (Purge, Discard) the Primary is Cancel and the destructive
//! step is the [`DialogSpec::danger`] button, apart at the left. A dialog that asks a question
//! ([`DialogSpec::asks`]) has a title that ends with "?"; a task dialog ("Go to page or cell") does not. Escape
//! cancels every dialog through `ui.dismiss`.

use serde_json::{Value, json};

use crate::shell::{ConfirmAnswer, Dialog, PURGE_LISTED, ShellModel};
use crate::strings;

/// What a dialog button does.
#[derive(Clone, PartialEq, Debug)]
pub enum DialogAnswer {
    /// Dispatches a registry action with its parameters.
    Action(&'static str, Value),
    /// Answers the unsaved-changes or the journal-discard question.
    Confirm(ConfirmAnswer),
    /// Closes the dialog (`ui.dismiss`).
    Dismiss,
    /// Submits the dialog's field or fields (Go, Save layout, a form's Create or Renumber): the window supplies what
    /// is typed.
    Submit,
    /// Opens the privacy statement in the browser (a window-only step of About).
    OpenPrivacy,
}

/// One button of a dialog.
#[derive(Clone, PartialEq, Debug)]
pub struct DialogButton {
    /// Its label, a verb (doc 16a §8 item 9).
    pub label: String,
    /// What it does.
    pub answer: DialogAnswer,
}

impl DialogButton {
    fn new(key: &str, answer: DialogAnswer) -> DialogButton {
        DialogButton {
            label: strings::text(key).to_owned(),
            answer,
        }
    }

    /// Whether the button closes the dialog without acting (Cancel, Close, Open read-only).
    pub fn cancels(&self) -> bool {
        matches!(
            self.answer,
            DialogAnswer::Dismiss | DialogAnswer::Confirm(ConfirmAnswer::Cancel)
        ) || matches!(self.answer, DialogAnswer::Action("journal.keep", _))
    }
}

/// The texts and buttons of one dialog.
#[derive(Clone, PartialEq, Debug)]
pub struct DialogSpec {
    /// The title: the question or the task.
    pub title: String,
    /// The body's paragraphs in `type.body`, at most three short ones (lists and fields are the dialog's own).
    pub body: Vec<String>,
    /// The one recommended action; Enter presses it; never destructive.
    pub primary: DialogButton,
    /// The other actions, in their order after the Primary.
    pub secondary: Vec<DialogButton>,
    /// The destructive action, left-aligned and apart (Don't save, Discard…, Purge).
    pub danger: Option<DialogButton>,
    /// Whether the title asks a question (it then ends with "?").
    pub asks: bool,
    /// Whether the dialog is a form (640 px wide) rather than a small dialog (440 px).
    pub form: bool,
}

impl DialogSpec {
    /// Every button in the order `ui.describe` lists them: the Primary, the others, the destructive one.
    pub fn buttons(&self) -> Vec<&DialogButton> {
        let mut all = vec![&self.primary];
        all.extend(&self.secondary);
        all.extend(&self.danger);
        all
    }

    /// Whether the dialog confirms a destructive step: its Primary cancels, its danger button is the step, drawn as
    /// a Primary in `color.error` (doc 16a §6.2).
    pub fn confirms_destruction(&self) -> bool {
        self.danger.is_some() && self.primary.cancels()
    }
}

fn close() -> DialogButton {
    DialogButton::new("dialog-close", DialogAnswer::Dismiss)
}

fn cancel() -> DialogButton {
    DialogButton::new("dialog-cancel", DialogAnswer::Dismiss)
}

/// The specification of `dialog` in the state of `model`.
pub fn spec(dialog: &Dialog, model: &ShellModel) -> DialogSpec {
    let small = |title: String, body: Vec<String>, primary: DialogButton| DialogSpec {
        title,
        body,
        primary,
        secondary: Vec::new(),
        danger: None,
        asks: false,
        form: false,
    };
    match dialog {
        Dialog::About => DialogSpec {
            secondary: vec![
                DialogButton::new("about-privacy-button", DialogAnswer::OpenPrivacy),
                DialogButton::new(
                    "about-notices-button",
                    DialogAnswer::Action("help.notices", Value::Null),
                ),
                DialogButton::new(
                    "about-docs-button",
                    DialogAnswer::Action("help.documentation", json!({ "topic": "about" })),
                ),
            ],
            ..small(
                strings::text("about-title").to_owned(),
                crate::about::about_lines(model),
                close(),
            )
        },
        Dialog::Notices => DialogSpec {
            form: true,
            ..small(strings::text("notices-title").to_owned(), Vec::new(), close())
        },
        // WP-S2.08 D8: the keymap editor; import and export a keymap file beside Close.
        Dialog::Shortcuts => DialogSpec {
            form: true,
            secondary: vec![
                DialogButton::new(
                    "action-keymap-import",
                    DialogAnswer::Action("keymap.import", Value::Null),
                ),
                DialogButton::new(
                    "action-keymap-export",
                    DialogAnswer::Action("keymap.export", Value::Null),
                ),
            ],
            ..small(strings::text("shortcuts-title").to_owned(), Vec::new(), close())
        },
        // The quit and close prompt (doc 16a §6.12): Save, Cancel, and Don't save apart.
        Dialog::ConfirmDiscard { .. } => DialogSpec {
            title: strings::format("confirm-title", &[model.project_name().unwrap_or("")]),
            body: vec![strings::text("confirm-text").to_owned()],
            primary: DialogButton::new("dialog-save", DialogAnswer::Confirm(ConfirmAnswer::Save)),
            secondary: vec![DialogButton::new(
                "dialog-cancel",
                DialogAnswer::Confirm(ConfirmAnswer::Cancel),
            )],
            danger: Some(DialogButton::new(
                "dialog-dont-save",
                DialogAnswer::Confirm(ConfirmAnswer::Discard),
            )),
            asks: true,
            form: false,
        },
        Dialog::ConfirmCheckScheme { scheme } => DialogSpec {
            secondary: vec![cancel()],
            asks: true,
            ..small(
                strings::text("confirm-check-scheme-title").to_owned(),
                vec![strings::format("confirm-check-scheme-text", &[scheme])],
                DialogButton::new(
                    "dialog-apply",
                    DialogAnswer::Action("wizard.apply_check_scheme", json!({ "confirm": true })),
                ),
            )
        },
        Dialog::ConfirmJournalDiscard => DialogSpec {
            danger: Some(DialogButton::new(
                "dialog-discard",
                DialogAnswer::Confirm(ConfirmAnswer::Discard),
            )),
            asks: true,
            ..small(
                strings::text("confirm-journal-title").to_owned(),
                vec![strings::text("confirm-journal-text").to_owned()],
                DialogButton::new("dialog-cancel", DialogAnswer::Confirm(ConfirmAnswer::Cancel)),
            )
        },
        Dialog::ConfirmPurge { files, .. } => {
            let mut body = vec![strings::text("confirm-purge-text").to_owned()];
            if files.len() > PURGE_LISTED {
                let more = (files.len() - PURGE_LISTED).to_string();
                body.push(strings::format("confirm-purge-more", &[&more]));
            }
            DialogSpec {
                danger: Some(DialogButton::new(
                    "dialog-purge",
                    DialogAnswer::Action("project.purge_assets", json!({ "confirm": true })),
                )),
                asks: true,
                ..small(strings::text("confirm-purge-title").to_owned(), body, cancel())
            }
        }
        // WP-S2.07 part 4a (D19): Choose (Enter, the active row) and Cancel; the list and the filters are the dialog's.
        Dialog::PartChoose(c) => {
            let body = if c.offers.is_empty() {
                vec![strings::format("part-choose-empty", &[&c.query.text])]
            } else {
                Vec::new()
            };
            DialogSpec {
                secondary: vec![cancel()],
                form: true,
                ..small(
                    c.title(),
                    body,
                    DialogButton::new(
                        "dialog-part-choose-button",
                        DialogAnswer::Action("part.pick", Value::Null),
                    ),
                )
            }
        }
        // WP-S2.07 part 4a (D20): Save (Enter) and Cancel; a fork says that the library is not changed.
        Dialog::PartEdit(e) => {
            let body = if e.draft.fork {
                vec![strings::text("part-edit-fork-note").to_owned()]
            } else {
                Vec::new()
            };
            DialogSpec {
                secondary: vec![cancel()],
                form: true,
                ..small(
                    e.title(),
                    body,
                    DialogButton::new("dialog-save", DialogAnswer::Action("part.save", Value::Null)),
                )
            }
        }
        // Recover, Open read-only (also Escape), Discard… apart; after a failed recovery Open read-only leads.
        Dialog::Recovery {
            name,
            transactions,
            base_matches,
            failed,
            ..
        } => {
            let mut body = vec![
                strings::format("recovery-text", &[name]),
                strings::format("recovery-count", &[&transactions.to_string()]),
            ];
            if !base_matches {
                body.push(strings::text("recovery-moved").to_owned());
            }
            let keep = DialogButton::new(
                "action-journal-keep",
                DialogAnswer::Action("journal.keep", Value::Null),
            );
            let (primary, secondary) = if failed.is_none() {
                (
                    DialogButton::new(
                        "action-journal-recover",
                        DialogAnswer::Action("journal.recover", Value::Null),
                    ),
                    vec![keep],
                )
            } else {
                (keep, Vec::new())
            };
            DialogSpec {
                title: strings::text("recovery-title").to_owned(),
                body,
                primary,
                secondary,
                danger: Some(DialogButton::new(
                    "recovery-discard",
                    DialogAnswer::Action("file.discard_journal", Value::Null),
                )),
                asks: true,
                form: false,
            }
        }
        Dialog::GotoPage { .. } => DialogSpec {
            secondary: vec![cancel()],
            ..small(
                strings::text("goto-title").to_owned(),
                Vec::new(),
                DialogButton::new("dialog-go", DialogAnswer::Submit),
            )
        },
        Dialog::SaveWorkspace { .. } => DialogSpec {
            secondary: vec![cancel()],
            ..small(
                strings::text("save-workspace-title").to_owned(),
                Vec::new(),
                DialogButton::new("dialog-save-workspace", DialogAnswer::Submit),
            )
        },
        Dialog::Form(form) if form.action.starts_with("revision.") => revision_spec(form, model),
        Dialog::Form(form) => DialogSpec {
            secondary: vec![cancel()],
            form: true,
            ..small(
                strings::text(form.title_key).to_owned(),
                Vec::new(),
                DialogButton::new(form.ok_key, DialogAnswer::Submit),
            )
        },
    }
}

/// WP-S1.18 part 5: the revision dialogs (forms, 0.45 of the window). Approve, Withdraw and Set author: the step is the
/// Primary, then Cancel. Release (destructive: it cannot be undone): Cancel is the Primary and Release the danger
/// button. The incomplete-release offer asks a question: Not now is the Primary, Complete release the other button
/// and Discard archive the danger one, each only where the release allows it. A dialog whose gate blocks shows Close
/// only. Each confirming button is its action with `confirm` (the window adds the date it reads).
fn revision_spec(form: &crate::shell::FormDialog, model: &ShellModel) -> DialogSpec {
    let (body, label, can_confirm) = model
        .revision_dialog()
        .map_or((Vec::new(), String::new(), false), |(b, l, c)| {
            (b.to_vec(), l.to_owned(), c)
        });
    let title = strings::format(form.title_key, &[&label]);
    let confirm =
        |key: &str| DialogButton::new(key, DialogAnswer::Action(form.action, json!({ "confirm": true })));
    let base = DialogSpec {
        asks: title.ends_with('?'),
        title,
        body,
        primary: close(),
        secondary: Vec::new(),
        danger: None,
        form: true,
    };
    if form.action == "revision.incomplete" {
        let (complete, discard) = model.incomplete_choices();
        let mut secondary = Vec::new();
        if complete {
            secondary.push(DialogButton::new(
                "dialog-complete-release",
                DialogAnswer::Action(form.action, json!({ "how": "complete", "confirm": true })),
            ));
        }
        return DialogSpec {
            primary: DialogButton::new("dialog-not-now", DialogAnswer::Dismiss),
            secondary,
            danger: discard.then(|| {
                DialogButton::new(
                    "dialog-discard-archive",
                    DialogAnswer::Action(form.action, json!({ "how": "discard", "confirm": true })),
                )
            }),
            ..base
        };
    }
    if !can_confirm {
        return base;
    }
    if form.action == "revision.release" {
        return DialogSpec {
            primary: cancel(),
            danger: Some(confirm(form.ok_key)),
            ..base
        };
    }
    DialogSpec {
        primary: confirm(form.ok_key),
        secondary: vec![cancel()],
        ..base
    }
}
