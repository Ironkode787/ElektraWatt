// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The texts and buttons of every modal dialog of the shell (doc 16a §6.12): one
//! [`DialogSpec`] per [`Dialog`], so the window's dialog frame and `ui.describe` show the same title, body and buttons,
//! and the rules of the design language are tested headless (CP1).
//!
//! **Rules** (doc 16a §6.2, §6.12, §8 item 9). Exactly one Primary, the button Enter presses; the Primary is never
//! destructive: in the confirmation of a destructive step (Purge, Discard) the Primary is Cancel and the destructive
//! step is the [`DialogSpec::danger`] button, apart at the left. A dialog that asks a question
//! ([`DialogSpec::asks`]) has a title that ends with "?"; a task dialog ("Go to page or cell") does not. Escape
//! cancels every dialog through `ui.dismiss`.

use serde_json::{Value, json};

use crate::key;
use crate::shell::{ConfirmAnswer, Dialog, PURGE_LISTED, ShellModel};
use crate::strings;
use crate::strings::Key;

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
    /// Why it is disabled (the window draws it disabled with this text on hover); `None` while it can be pressed.
    pub disabled: Option<String>,
}

impl DialogButton {
    fn new(key: Key<'_>, answer: DialogAnswer) -> DialogButton {
        DialogButton {
            label: strings::text(key).to_owned(),
            answer,
            disabled: None,
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

/// The most pages the import card lists, and the most rows the re-import card lists, before its "and N more" line.
pub const IMPORT_PAGES_LISTED: usize = 12;

fn close() -> DialogButton {
    DialogButton::new(key!("dialog-close"), DialogAnswer::Dismiss)
}

fn cancel() -> DialogButton {
    DialogButton::new(key!("dialog-cancel"), DialogAnswer::Dismiss)
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
                DialogButton::new(key!("about-privacy-button"), DialogAnswer::OpenPrivacy),
                DialogButton::new(
                    key!("about-notices-button"),
                    DialogAnswer::Action("help.notices", Value::Null),
                ),
                DialogButton::new(
                    key!("about-docs-button"),
                    DialogAnswer::Action("help.documentation", json!({ "topic": "about" })),
                ),
            ],
            ..small(
                strings::text(key!("about-title")).to_owned(),
                crate::about::about_lines(model),
                close(),
            )
        },
        Dialog::Notices => DialogSpec {
            form: true,
            ..small(
                strings::text(key!("notices-title")).to_owned(),
                Vec::new(),
                close(),
            )
        },
        // The keymap editor; import and export a keymap file beside Close.
        Dialog::Shortcuts => DialogSpec {
            form: true,
            secondary: vec![
                DialogButton::new(
                    key!("action-keymap-import"),
                    DialogAnswer::Action("keymap.import", Value::Null),
                ),
                DialogButton::new(
                    key!("action-keymap-export"),
                    DialogAnswer::Action("keymap.export", Value::Null),
                ),
            ],
            ..small(
                strings::text(key!("shortcuts-title")).to_owned(),
                Vec::new(),
                close(),
            )
        },
        // Quitting with an unsaved symbol draft asks Save symbol, Discard or Cancel; ID-1472: so does an editor
        // action that drops the draft, in words of its own.
        Dialog::ConfirmDiscard {
            symbol: true,
            pending,
        } => DialogSpec {
            title: strings::format(
                key!("symbol-confirm-title"),
                &[&model
                    .kept_symbol_editor()
                    .map(|e| crate::shell::symbol_editor::name_words(&e.draft.symbol.name))
                    .unwrap_or_default()],
            ),
            body: vec![
                strings::text(if pending.action == "app.quit" {
                    key!("symbol-confirm-text")
                } else {
                    key!("symbol-confirm-replace")
                })
                .to_owned(),
            ],
            primary: DialogButton::new(
                key!("symbol-save-symbol"),
                DialogAnswer::Confirm(ConfirmAnswer::Save),
            ),
            secondary: vec![DialogButton::new(
                key!("dialog-cancel"),
                DialogAnswer::Confirm(ConfirmAnswer::Cancel),
            )],
            danger: Some(DialogButton::new(
                key!("symbol-discard"),
                DialogAnswer::Confirm(ConfirmAnswer::Discard),
            )),
            asks: true,
            form: false,
        },
        // The quit and close prompt (doc 16a §6.12): Save, Cancel, and Don't save apart.
        Dialog::ConfirmDiscard { .. } => DialogSpec {
            title: strings::format(key!("confirm-title"), &[model.project_name().unwrap_or("")]),
            body: vec![strings::text(key!("confirm-text")).to_owned()],
            primary: DialogButton::new(key!("dialog-save"), DialogAnswer::Confirm(ConfirmAnswer::Save)),
            secondary: vec![DialogButton::new(
                key!("dialog-cancel"),
                DialogAnswer::Confirm(ConfirmAnswer::Cancel),
            )],
            danger: Some(DialogButton::new(
                key!("dialog-dont-save"),
                DialogAnswer::Confirm(ConfirmAnswer::Discard),
            )),
            asks: true,
            form: false,
        },
        Dialog::ConfirmCheckScheme { scheme } => DialogSpec {
            secondary: vec![cancel()],
            asks: true,
            ..small(
                strings::text(key!("confirm-check-scheme-title")).to_owned(),
                // ID-1389 (W51 finding D): the scheme's word, as the wizard's choice shows it.
                vec![strings::format(
                    key!("confirm-check-scheme-text"),
                    &[strings::text(Key::run_time(&format!(
                        "choice-check_scheme-{scheme}"
                    )))],
                )],
                DialogButton::new(
                    key!("dialog-apply"),
                    DialogAnswer::Action("wizard.apply_check_scheme", json!({ "confirm": true })),
                ),
            )
        },
        Dialog::ConfirmJournalDiscard => DialogSpec {
            danger: Some(DialogButton::new(
                key!("dialog-discard"),
                DialogAnswer::Confirm(ConfirmAnswer::Discard),
            )),
            asks: true,
            ..small(
                strings::text(key!("confirm-journal-title")).to_owned(),
                vec![strings::text(key!("confirm-journal-text")).to_owned()],
                DialogButton::new(
                    key!("dialog-cancel"),
                    DialogAnswer::Confirm(ConfirmAnswer::Cancel),
                ),
            )
        },
        Dialog::ConfirmPurge { files, .. } => {
            let mut body = vec![strings::text(key!("confirm-purge-text")).to_owned()];
            if files.len() > PURGE_LISTED {
                let more = (files.len() - PURGE_LISTED).to_string();
                body.push(strings::format(key!("confirm-purge-more"), &[&more]));
            }
            DialogSpec {
                danger: Some(DialogButton::new(
                    key!("dialog-purge"),
                    DialogAnswer::Action("project.purge_assets", json!({ "confirm": true })),
                )),
                asks: true,
                ..small(
                    strings::text(key!("confirm-purge-title")).to_owned(),
                    body,
                    cancel(),
                )
            }
        }
        // Choose (Enter, the active row) and Cancel; the list and the filters are the dialog's.
        // W45.1 F2: an empty search is said once, in the list (not here); Choose is disabled while the list is
        // empty, with its reason.
        Dialog::PartChoose(c) => {
            let mut choose = DialogButton::new(
                key!("dialog-part-choose-button"),
                DialogAnswer::Action("part.pick", Value::Null),
            );
            if c.offers.is_empty() {
                choose.disabled = Some(strings::text(key!("part-choose-nothing")).to_owned());
            }
            DialogSpec {
                secondary: vec![cancel()],
                form: true,
                ..small(c.title(), Vec::new(), choose)
            }
        }
        // Save (Enter) and Cancel; a fork says that the library is not changed.
        Dialog::PartEdit(e) => {
            let body = if e.draft.fork {
                vec![strings::text(key!("part-edit-fork-note")).to_owned()]
            } else {
                Vec::new()
            };
            DialogSpec {
                secondary: vec![cancel()],
                form: true,
                ..small(
                    e.title(),
                    body,
                    DialogButton::new(
                        key!("dialog-save"),
                        DialogAnswer::Action("part.save", Value::Null),
                    ),
                )
            }
        }
        // Apply (the choices, Enter), All mine, All theirs, Decide later (Escape); the list of
        // fields with base, mine and theirs is the dialog's.
        Dialog::ExternalChange { rows } => DialogSpec {
            secondary: vec![
                DialogButton::new(
                    key!("external-all-mine"),
                    DialogAnswer::Action("external.apply", json!({ "take": "mine" })),
                ),
                DialogButton::new(
                    key!("external-all-theirs"),
                    DialogAnswer::Action("external.apply", json!({ "take": "theirs" })),
                ),
                DialogButton::new(key!("external-later"), DialogAnswer::Dismiss),
            ],
            asks: true,
            form: true,
            ..small(
                strings::text(key!("external-title")).to_owned(),
                vec![strings::format(
                    if rows.len() == 1 {
                        key!("external-text-one")
                    } else {
                        key!("external-text")
                    },
                    &[&crate::shell::collision_count(rows)],
                )],
                DialogButton::new(
                    key!("external-apply"),
                    DialogAnswer::Action("external.apply", Value::Null),
                ),
            )
        },
        // D11: Upgrade and save (Enter) or Cancel.
        Dialog::ConfirmUpgrade { from } => DialogSpec {
            secondary: vec![cancel()],
            asks: true,
            ..small(
                strings::text(key!("upgrade-title")).to_owned(),
                vec![
                    strings::format(key!("upgrade-text"), &[from, ew_api::FORMAT]),
                    strings::text(key!("upgrade-backup")).to_owned(),
                ],
                DialogButton::new(
                    key!("upgrade-button"),
                    DialogAnswer::Action("file.upgrade", Value::Null),
                ),
            )
        },
        // The import card (Apply, Cancel) with its summary, its pages listed by the dialog; then the
        // checkpoint confirmation (Create project, Cancel).
        Dialog::ImportQet {
            file,
            folder,
            summary,
            pages,
            devices,
            confirm: false,
        } => {
            let _ = (folder, devices);
            let mut body = vec![summary.clone()];
            if pages.len() > IMPORT_PAGES_LISTED {
                let more = (pages.len() - IMPORT_PAGES_LISTED).to_string();
                body.push(strings::format(key!("import-card-more"), &[&more]));
            }
            DialogSpec {
                secondary: vec![cancel()],
                form: true,
                ..small(
                    strings::format(key!("import-card-title"), &[file]),
                    body,
                    DialogButton::new(
                        key!("import-card-apply"),
                        DialogAnswer::Action("file.import_qet_apply", Value::Null),
                    ),
                )
            }
        }
        Dialog::ImportQet {
            file,
            folder,
            pages,
            devices,
            confirm: true,
            ..
        } => DialogSpec {
            secondary: vec![cancel()],
            asks: true,
            ..small(
                strings::format(key!("import-confirm-title"), &[file]),
                vec![strings::format(
                    key!("import-confirm-text"),
                    &[
                        folder,
                        &strings::format_count(crate::count_key!("import-card-pages"), pages.len()),
                        &strings::format_count(crate::count_key!("import-card-devices"), *devices),
                    ],
                )],
                DialogButton::new(
                    key!("import-confirm-create"),
                    DialogAnswer::Action("file.import_qet_apply", json!({ "confirm": true })),
                ),
            )
        },
        // The proposal T6 (Re-import, Cancel).
        Dialog::ReimportMatch {
            path,
            file,
            source,
            percent,
        } => DialogSpec {
            secondary: vec![cancel()],
            asks: true,
            ..small(
                strings::text(key!("reimport-match-title")).to_owned(),
                vec![strings::format(
                    key!("reimport-match-text"),
                    &[file, source, &percent.to_string()],
                )],
                DialogButton::new(
                    key!("reimport-match-accept"),
                    DialogAnswer::Action("file.reimport_qet", json!({ "file": path, "accept": true })),
                ),
            )
        },
        // The re-import card (Apply, Cancel) with T8, its rows listed by the dialog.
        Dialog::Reimport {
            file,
            summary,
            rows,
            nothing,
        } => {
            let mut body = vec![summary.clone()];
            if rows.len() > IMPORT_PAGES_LISTED {
                let more = (rows.len() - IMPORT_PAGES_LISTED).to_string();
                body.push(strings::format(key!("reimport-card-more"), &[&more]));
            }
            let mut apply = DialogButton::new(
                key!("reimport-card-apply"),
                DialogAnswer::Action("file.reimport_qet_apply", Value::Null),
            );
            // ID-1427 (W52 F8): nothing to re-import, Apply disabled with its reason (as a table import's).
            if *nothing {
                apply.disabled = Some(strings::text(key!("reason-table-import-nothing")).to_owned());
            }
            DialogSpec {
                secondary: vec![cancel()],
                form: true,
                ..small(strings::format(key!("reimport-card-title"), &[file]), body, apply)
            }
        }
        // D24: Extract and open (Enter), Open read-only, Cancel.
        Dialog::OpenEwz { path, name } => DialogSpec {
            secondary: vec![
                DialogButton::new(
                    key!("ewz-read-only"),
                    DialogAnswer::Action("file.open_ewz", json!({ "path": path, "how": "read_only" })),
                ),
                cancel(),
            ],
            asks: true,
            ..small(
                strings::text(key!("ewz-title")).to_owned(),
                vec![strings::format(key!("ewz-text"), &[name])],
                DialogButton::new(
                    key!("ewz-extract"),
                    DialogAnswer::Action("file.open_ewz", json!({ "path": path, "how": "extract" })),
                ),
            )
        },
        // D28: Set up (Enter) or Cancel.
        Dialog::ConfirmGitSetup { folder } => DialogSpec {
            secondary: vec![cancel()],
            asks: true,
            ..small(
                strings::text(key!("git-setup-title")).to_owned(),
                vec![strings::format(key!("git-setup-text"), &[folder])],
                DialogButton::new(
                    key!("git-setup-button"),
                    DialogAnswer::Action("git.setup", json!({ "confirm": true })),
                ),
            )
        },
        // Print… the Primary (Enter), Open in PDF viewer and Cancel; both print
        // buttons disabled, with the reason, until the set is built.
        Dialog::Print(_) => {
            let reason = model.print_disabled_reason();
            let mut print = DialogButton::new(
                key!("print-button"),
                DialogAnswer::Action("print.send", json!({ "via": "printer" })),
            );
            let mut viewer = DialogButton::new(
                key!("print-viewer-button"),
                DialogAnswer::Action("print.send", json!({ "via": "viewer" })),
            );
            print.disabled.clone_from(&reason);
            viewer.disabled = reason;
            DialogSpec {
                secondary: vec![viewer, cancel()],
                form: true,
                ..small(strings::text(key!("print-title")).to_owned(), Vec::new(), print)
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
                strings::format(key!("recovery-text"), &[name]),
                strings::format(key!("recovery-count"), &[&transactions.to_string()]),
            ];
            if !base_matches {
                body.push(strings::text(key!("recovery-moved")).to_owned());
            }
            let keep = DialogButton::new(
                key!("action-journal-keep"),
                DialogAnswer::Action("journal.keep", Value::Null),
            );
            let (primary, secondary) = if failed.is_none() {
                (
                    DialogButton::new(
                        key!("action-journal-recover"),
                        DialogAnswer::Action("journal.recover", Value::Null),
                    ),
                    vec![keep],
                )
            } else {
                (keep, Vec::new())
            };
            DialogSpec {
                title: strings::text(key!("recovery-title")).to_owned(),
                body,
                primary,
                secondary,
                danger: Some(DialogButton::new(
                    key!("recovery-discard"),
                    DialogAnswer::Action("file.discard_journal", Value::Null),
                )),
                asks: true,
                form: false,
            }
        }
        Dialog::GotoPage { .. } => DialogSpec {
            secondary: vec![cancel()],
            ..small(
                strings::text(key!("goto-title")).to_owned(),
                Vec::new(),
                DialogButton::new(key!("dialog-go"), DialogAnswer::Submit),
            )
        },
        Dialog::SaveWorkspace { .. } => DialogSpec {
            secondary: vec![cancel()],
            ..small(
                strings::text(key!("save-workspace-title")).to_owned(),
                Vec::new(),
                DialogButton::new(key!("dialog-save-workspace"), DialogAnswer::Submit),
            )
        },
        Dialog::Form(form) if form.action.starts_with("revision.") => revision_spec(form, model),
        Dialog::Form(form) => DialogSpec {
            secondary: vec![cancel()],
            form: true,
            ..small(
                strings::text(form.title_key).to_owned(),
                // WP-S4.02 part 4 (§8): the Auto-fill rail dialog says which devices it fills with.
                match form.action {
                    "layout.auto_fill_rail" => vec![model.fill_devices_text()],
                    "layout.enclosure_new" => model.enclosure_form_notes(),
                    _ => Vec::new(),
                },
                DialogButton {
                    // W44.2 F1: Create enclosure's Create is disabled while its location or part is empty.
                    disabled: match form.action {
                        "layout.enclosure_new" => model.enclosure_create_reason(),
                        _ => None,
                    },
                    ..DialogButton::new(form.ok_key, DialogAnswer::Submit)
                },
            )
        },
    }
}

/// The revision dialogs (forms, 0.45 of the window). Approve, Withdraw and Set author: the step is the
/// Primary, then Cancel. Release (destructive: it cannot be undone): Cancel is the Primary and Release the danger
/// button. The incomplete-release offer asks a question: Not now is the Primary, Complete release the other button
/// and Discard archive the danger one, each only where the release allows it, and each opens its confirmation (Cancel
/// the Primary, the act the danger button). A dialog whose gate blocks shows Close only. Each confirming button is its
/// action with `confirm` (the window adds the date it reads).
fn revision_spec(form: &crate::shell::FormDialog, model: &ShellModel) -> DialogSpec {
    let (body, label, can_confirm) = model
        .revision_dialog()
        .map_or((Vec::new(), String::new(), false), |(b, l, c)| {
            (b.to_vec(), l.to_owned(), c)
        });
    let title = strings::format(form.title_key, &[&label]);
    let confirm =
        |key: Key<'_>| DialogButton::new(key, DialogAnswer::Action(form.action, json!({ "confirm": true })));
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
        // Cycle 41, FIX41b ruling 2: "completing an incomplete release asks, like a release": the confirmation of
        // Complete release or Discard archive has Cancel as the Primary and its act as the danger button.
        if let Some(how) = model.settle_stage() {
            return DialogSpec {
                primary: cancel(),
                danger: Some(DialogButton::new(
                    form.ok_key,
                    DialogAnswer::Action(form.action, json!({ "how": how, "confirm": true })),
                )),
                ..base
            };
        }
        let (complete, discard) = model.incomplete_choices();
        let mut secondary = Vec::new();
        if complete {
            secondary.push(DialogButton::new(
                key!("dialog-complete-release"),
                DialogAnswer::Action(form.action, json!({ "how": "complete" })),
            ));
        }
        return DialogSpec {
            primary: DialogButton::new(key!("dialog-not-now"), DialogAnswer::Dismiss),
            secondary,
            danger: discard.then(|| {
                DialogButton::new(
                    key!("dialog-discard-archive"),
                    DialogAnswer::Action(form.action, json!({ "how": "discard" })),
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
