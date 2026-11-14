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

/// The kinds of findings the import card lists per group; the rest is counted in one line ([`import_more_kinds`]).
pub const IMPORT_KINDS_LISTED: usize = 5;

/// The line that counts the kinds of a group beyond [`IMPORT_KINDS_LISTED`]; `None` when all are listed.
pub fn import_more_kinds(group: &crate::shell::ImportGroup) -> Option<String> {
    let more = group
        .rows
        .len()
        .checked_sub(IMPORT_KINDS_LISTED)
        .filter(|n| *n > 0)?;
    Some(strings::format_count(
        crate::count_key!("import-card-more-kinds"),
        more,
    ))
}

fn close() -> DialogButton {
    DialogButton::new(key!("dialog-close"), DialogAnswer::Dismiss)
}

fn cancel() -> DialogButton {
    DialogButton::new(key!("dialog-cancel"), DialogAnswer::Dismiss)
}

/// A small dialog (440 px) with `title`, `body` and `primary` alone, which a dialog's other buttons extend.
fn small(title: String, body: Vec<String>, primary: DialogButton) -> DialogSpec {
    DialogSpec {
        title,
        body,
        primary,
        secondary: Vec::new(),
        danger: None,
        asks: false,
        form: false,
    }
}

/// The specification of `dialog` in the state of `model`.
pub fn spec(dialog: &Dialog, model: &ShellModel) -> DialogSpec {
    match dialog {
        Dialog::About => about_spec(model),
        Dialog::Notices => DialogSpec {
            form: true,
            ..small(
                strings::text(key!("notices-title")).to_owned(),
                Vec::new(),
                close(),
            )
        },
        // The keymap editor; import and export a keymap file beside Close.
        Dialog::Shortcuts => shortcuts_spec(),
        // Quitting with an unsaved symbol draft asks Save symbol, Discard or Cancel; so does an editor
        // action that drops the draft, in words of its own.
        Dialog::ConfirmDiscard {
            symbol: true,
            pending,
        } => symbol_discard_spec(pending, model),
        // The quit and close prompt (doc 16a §6.12): Save, Cancel, and Don't save apart.
        Dialog::ConfirmDiscard { .. } => discard_spec(model),
        Dialog::ConfirmCheckScheme { scheme } => check_scheme_spec(scheme),
        Dialog::ConfirmJournalDiscard => journal_discard_spec(),
        Dialog::ConfirmPurge { files, .. } => purge_spec(files),
        // Choose (Enter, the active row) and Cancel; the list and the filters are the dialog's.
        // An empty search is said once, in the list (not here); Choose is disabled while the list is
        // empty, with its reason.
        Dialog::PartChoose(c) => part_choose_spec(c),
        // Save (Enter) and Cancel; a fork says that the library is not changed.
        Dialog::PartEdit(e) => part_edit_spec(e),
        // Apply (the choices, Enter), All mine, All theirs, Decide later (Escape); the list of
        // fields with base, mine and theirs is the dialog's.
        Dialog::ExternalChange { rows } => {
            external_change_spec(rows.len(), &crate::shell::collision_count(rows))
        }
        // D11: Upgrade and save (Enter) or Cancel.
        Dialog::ConfirmUpgrade { from } => upgrade_spec(from),
        // The import card (Create project, Cancel): what is made, the verdict, a note; its groups of findings are
        // listed by the dialog.
        Dialog::ImportQet {
            file,
            summary,
            verdict,
            note,
            groups,
        } => import_card_spec(file, summary, verdict, note.as_deref(), !groups.is_empty()),
        // The proposal T6 (Re-import, Cancel).
        Dialog::ReimportMatch {
            path,
            file,
            source,
            percent,
        } => reimport_match_spec(path, file, source, *percent),
        // The re-import card (Apply, Cancel) with T8, its rows listed by the dialog.
        Dialog::Reimport {
            file,
            summary,
            rows,
            nothing,
        } => reimport_spec(file, summary, rows, *nothing),
        // D24: Extract and open (Enter), Open read-only, Cancel.
        Dialog::OpenEwz { path, name } => open_ewz_spec(path, name),
        Dialog::ImportDrawing {
            path,
            name,
            kind,
            folio,
            unitless,
            converter,
        } => import_drawing_spec(path, name, *kind, folio.as_deref(), *unitless, *converter),
        Dialog::ImportEplan { path, name, step } => import_eplan_spec(path, name, step),
        // D28: Set up (Enter) or Cancel.
        Dialog::ConfirmGitSetup { folder } => git_setup_spec(folder),
        // Print… the Primary (Enter), Open in PDF viewer and Cancel; both print
        // buttons disabled, with the reason, until the set is built.
        Dialog::Print(_) => print_spec(model),
        // Recover, Open read-only (also Escape), Discard… apart; after a failed recovery Open read-only leads.
        Dialog::Recovery {
            name,
            transactions,
            base_matches,
            failed,
            ..
        } => recovery_spec(name, *transactions, *base_matches, failed.as_ref()),
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
        Dialog::Form(form) => form_spec(form, model),
    }
}

/// About: the program's lines, Close, and the privacy statement, the notices and the documentation beside it.
fn about_spec(model: &ShellModel) -> DialogSpec {
    DialogSpec {
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
    }
}

/// Keyboard shortcuts: a form with Close, Import and Export.
fn shortcuts_spec() -> DialogSpec {
    DialogSpec {
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
    }
}

/// The symbol draft's unsaved changes: Save symbol, Cancel, and Discard apart, in the words of quitting or of the
/// editor action `pending`.
fn symbol_discard_spec(pending: &crate::shell::Pending, model: &ShellModel) -> DialogSpec {
    DialogSpec {
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
    }
}

/// The project's unsaved changes: Save, Cancel, and Don't save apart.
fn discard_spec(model: &ShellModel) -> DialogSpec {
    DialogSpec {
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
    }
}

/// Applying check scheme `scheme`: Apply or Cancel.
fn check_scheme_spec(scheme: &str) -> DialogSpec {
    DialogSpec {
        secondary: vec![cancel()],
        asks: true,
        ..small(
            strings::text(key!("confirm-check-scheme-title")).to_owned(),
            // The scheme's word, as the wizard's choice shows it.
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
    }
}

/// Discarding the journal: Cancel the Primary, Discard apart.
fn journal_discard_spec() -> DialogSpec {
    DialogSpec {
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
    }
}

/// Purging the unused image `files`: Cancel the Primary, Purge apart, the files past the listed ones counted.
fn purge_spec(files: &[String]) -> DialogSpec {
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

/// Choose part: a form with Choose, disabled while the list is empty, and Cancel.
fn part_choose_spec(c: &crate::shell::part_dialog::PartChooser) -> DialogSpec {
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

/// Edit part: a form with Save and Cancel.
fn part_edit_spec(e: &crate::shell::part_dialog::PartEditor) -> DialogSpec {
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

/// The external change colliding with unsaved edits in `rows` fields, `count` in words: a form with Apply, All mine,
/// All theirs and Decide later.
fn external_change_spec(rows: usize, count: &str) -> DialogSpec {
    DialogSpec {
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
                if rows == 1 {
                    key!("external-text-one")
                } else {
                    key!("external-text")
                },
                &[count],
            )],
            DialogButton::new(
                key!("external-apply"),
                DialogAnswer::Action("external.apply", Value::Null),
            ),
        )
    }
}

/// The upgrade of a project of format `from`: Upgrade and save or Cancel.
fn upgrade_spec(from: &str) -> DialogSpec {
    DialogSpec {
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
    }
}

/// The import card of `file`: a form with its summary, the pages past the listed ones counted, Apply and Cancel.
fn import_card_spec(
    file: &str,
    summary: &str,
    verdict: &str,
    note: Option<&str>,
    findings: bool,
) -> DialogSpec {
    let mut body = vec![summary.to_owned(), verdict.to_owned()];
    body.extend(note.map(str::to_owned));
    // Where the findings are read in full, once the project exists.
    if findings {
        body.push(strings::text(key!("import-card-where")).to_owned());
    }
    DialogSpec {
        secondary: vec![cancel()],
        form: true,
        ..small(
            strings::format(key!("import-card-title"), &[file]),
            body,
            DialogButton::new(
                key!("import-confirm-create"),
                DialogAnswer::Action("file.import_qet_apply", json!({ "confirm": true })),
            ),
        )
    }
}

/// The file at `path` matched to an earlier import: Re-import or Cancel.
fn reimport_match_spec(path: &str, file: &str, source: &str, percent: u32) -> DialogSpec {
    DialogSpec {
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
    }
}

/// The re-import card of `file`: a form with its summary, the rows past the listed ones counted, Apply and Cancel.
fn reimport_spec(file: &str, summary: &str, rows: &[String], nothing: bool) -> DialogSpec {
    let mut body = vec![summary.to_owned()];
    if rows.len() > IMPORT_PAGES_LISTED {
        let more = (rows.len() - IMPORT_PAGES_LISTED).to_string();
        body.push(strings::format(key!("reimport-card-more"), &[&more]));
    }
    let mut apply = DialogButton::new(
        key!("reimport-card-apply"),
        DialogAnswer::Action("file.reimport_qet_apply", Value::Null),
    );
    // Nothing to re-import, Apply disabled with its reason (as a table import's).
    if nothing {
        apply.disabled = Some(strings::text(key!("reason-table-import-nothing")).to_owned());
    }
    DialogSpec {
        secondary: vec![cancel()],
        form: true,
        ..small(strings::format(key!("reimport-card-title"), &[file]), body, apply)
    }
}

/// Opening the `.ewz` at `path`: Extract and open, Open read-only or Cancel.
fn open_ewz_spec(path: &str, name: &str) -> DialogSpec {
    DialogSpec {
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
    }
}

/// Importing the drawing `name` (WP-S3.19): Add as page (a DXF or DWG), Place under page <folio> (embedded or not),
/// Cancel. The body says what the file is, an assumed unit, a DWG's converter, and the confidentiality of an underlay.
fn import_drawing_spec(
    path: &str,
    name: &str,
    kind: crate::shell::DrawingKind,
    folio: Option<&str>,
    unitless: bool,
    converter: bool,
) -> DialogSpec {
    use crate::shell::DrawingKind;
    let action = |target: &str, embed: bool| {
        let mut params = json!({ "file": path, "as": target, "embed": embed });
        if unitless {
            // What the body says: a drawing without a unit is read in millimetres.
            params["units"] = json!("mm");
        }
        DialogAnswer::Action("file.import_drawing", params)
    };
    let mut body = vec![strings::format(
        match kind {
            DrawingKind::Dxf => key!("drawing-dxf-text"),
            DrawingKind::Dwg => key!("drawing-dwg-text"),
            DrawingKind::Pdf => key!("drawing-pdf-text"),
        },
        &[name],
    )];
    if unitless {
        body.push(strings::text(key!("drawing-unitless")).to_owned());
    }
    if converter {
        body.push(strings::text(key!("drawing-converter")).to_owned());
    }
    body.push(strings::text(key!("drawing-underlay-text")).to_owned());
    let folio_text = folio.unwrap_or("");
    let mut under = DialogButton::new(key!("drawing-underlay"), action("underlay", false));
    under.label = strings::format(key!("drawing-underlay"), &[folio_text]);
    let mut embed = DialogButton::new(key!("drawing-embed"), action("underlay", true));
    if folio.is_none() {
        let why = strings::text(key!("reason-no-page")).to_owned();
        under.disabled = Some(why.clone());
        embed.disabled = Some(why);
    }
    let (primary, mut secondary) = match kind {
        DrawingKind::Pdf => (under, vec![embed]),
        DrawingKind::Dxf | DrawingKind::Dwg => (
            DialogButton::new(key!("drawing-page"), action("page", false)),
            vec![under, embed],
        ),
    };
    secondary.push(cancel());
    DialogSpec {
        secondary,
        ..small(strings::format(key!("drawing-title"), &[name]), body, primary)
    }
}

/// Importing EPLAN data from `path` (WP-S3.18): the terms notice with Continue; the parts of a file with Update (the
/// copies that would change) and Add to my parts library; a report with a button for each mapping that fits.
fn import_eplan_spec(path: &str, name: &str, step: &crate::shell::EplanStep) -> DialogSpec {
    use crate::shell::EplanStep;
    use ew_api::eplan::ReportTarget;
    let action = |extra: Value| {
        let mut params = json!({ "file": path });
        if let (Some(p), Value::Object(e)) = (params.as_object_mut(), extra) {
            p.extend(e);
        }
        DialogAnswer::Action("file.import_eplan", params)
    };
    let title = strings::format(key!("eplan-title"), &[name]);
    match step {
        EplanStep::Terms => DialogSpec {
            secondary: vec![cancel()],
            ..small(
                strings::text(key!("eplan-terms-title")).to_owned(),
                vec![
                    strings::text(key!("eplan-terms-text")).to_owned(),
                    strings::text(key!("eplan-terms-local")).to_owned(),
                ],
                DialogButton::new(key!("eplan-continue"), action(json!({ "terms": true }))),
            )
        },
        EplanStep::Parts {
            read,
            matched,
            changed,
        } => {
            let mut update = DialogButton::new(key!("eplan-update"), action(json!({ "how": "update" })));
            update.label = strings::format(key!("eplan-update"), &[&changed.to_string()]);
            if *changed == 0 {
                update.disabled = Some(strings::text(key!("reason-eplan-no-change")).to_owned());
            }
            let library = DialogButton::new(key!("eplan-library"), action(json!({ "how": "library" })));
            DialogSpec {
                secondary: vec![library, cancel()],
                ..small(
                    title,
                    vec![
                        strings::format(
                            key!("eplan-parts-text"),
                            &[
                                name,
                                &read.to_string(),
                                &matched.to_string(),
                                &changed.to_string(),
                            ],
                        ),
                        strings::text(key!("eplan-parts-note")).to_owned(),
                    ],
                    update,
                )
            }
        }
        EplanStep::Report { fits, saved } => {
            let mut buttons: Vec<DialogButton> = fits
                .iter()
                .enumerate()
                .map(|(i, (preset, target))| {
                    let label_key = match target {
                        ReportTarget::Devices => key!("eplan-report-devices"),
                        ReportTarget::Terminals => key!("eplan-report-terminals"),
                        ReportTarget::Connections => key!("eplan-report-connections"),
                    };
                    let mut b =
                        DialogButton::new(label_key, action(json!({ "how": "report", "preset": preset })));
                    if *saved && i == 0 {
                        b.label = strings::format(key!("eplan-report-saved"), &[strings::text(label_key)]);
                    }
                    b
                })
                .collect();
            let body = if buttons.is_empty() {
                vec![
                    strings::format(key!("eplan-report-text"), &[name]),
                    strings::text(key!("eplan-report-none")).to_owned(),
                ]
            } else {
                vec![strings::format(key!("eplan-report-text"), &[name])]
            };
            let primary = if buttons.is_empty() {
                cancel()
            } else {
                buttons.remove(0)
            };
            let mut secondary = buttons;
            if !fits.is_empty() {
                secondary.push(cancel());
            }
            DialogSpec {
                secondary,
                ..small(title, body, primary)
            }
        }
    }
}

/// Setting up the merge driver in `folder`: Set up or Cancel.
fn git_setup_spec(folder: &str) -> DialogSpec {
    DialogSpec {
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
    }
}

/// Print: a form with Print…, Open in PDF viewer and Cancel.
fn print_spec(model: &ShellModel) -> DialogSpec {
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

/// Recovering the journal of project `name`: its unsaved `transactions` counted, a moved project said; Recover,
/// Open read-only, Discard… apart.
fn recovery_spec(name: &str, transactions: usize, base_matches: bool, failed: Option<&String>) -> DialogSpec {
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

/// A form of an action's fields: its OK button submits them, beside Cancel.
fn form_spec(form: &crate::shell::FormDialog, model: &ShellModel) -> DialogSpec {
    DialogSpec {
        secondary: vec![cancel()],
        form: true,
        ..small(
            strings::text(form.title_key).to_owned(),
            // The Auto-fill rail dialog says which devices it fills with.
            match form.action {
                "layout.auto_fill_rail" => vec![model.fill_devices_text()],
                "layout.enclosure_new" => model.enclosure_form_notes(),
                "page.title_block" => model.title_block_notes(),
                _ => Vec::new(),
            },
            DialogButton {
                // Create enclosure's Create is disabled while its location or part is empty.
                disabled: match form.action {
                    "layout.enclosure_new" => model.enclosure_create_reason(),
                    _ => None,
                },
                ..DialogButton::new(form.ok_key, DialogAnswer::Submit)
            },
        )
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
        // Completing an incomplete release asks, like a release: the confirmation of
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
