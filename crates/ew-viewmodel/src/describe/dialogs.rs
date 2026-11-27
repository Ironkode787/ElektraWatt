// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The describe text's lines of the dialog on show, one step per dialog that needs more than a few.

#[allow(clippy::wildcard_imports)]
use super::*;

pub(super) fn dialog(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(d) = model.dialog() else {
        return;
    };
    // The title and the buttons are the dialog's specification (`dialogs::spec`), the window's too;
    // the buttons in the order Primary, the others, the destructive one.
    let spec = crate::dialogs::spec(d, model);
    lines.push(format!("dialog \"{}\"", user(&spec.title)));
    match d {
        Dialog::About => {
            for line in &spec.body {
                lines.push(format!("  label \"{line}\""));
            }
        }
        Dialog::Notices => {
            let text = crate::about::notices_text(model);
            lines.push(format!("  notices: {} lines", text.lines().count()));
        }
        // The editor's rows with a binding (the effective keymap, filtered by the search).
        Dialog::Shortcuts => {
            for row in model.keymap_rows().iter().filter(|r| !r.chords.is_empty()) {
                lines.push(format!(
                    "  {} \"{}\" {}",
                    row.action,
                    row.label,
                    row.chords.join(", ")
                ));
            }
        }
        Dialog::ConfirmDiscard { pending, .. } => {
            for line in &spec.body {
                lines.push(format!("  label \"{line}\""));
            }
            lines.push(format!("  pending {}", pending.action));
        }
        Dialog::ConfirmJournalDiscard
        | Dialog::ConfirmRemovePart { .. }
        | Dialog::ConfirmLeavePart { .. }
        | Dialog::ConfirmReplaceDxf { .. } => {
            for line in &spec.body {
                lines.push(format!("  label \"{line}\""));
            }
        }
        // The PLC import's preview: its summary, the tab on show and its rows with their states and choices.
        Dialog::PlcImport(card) => plc_import_dialog_lines(card, &spec, lines),
        // The paragraphs; the collision dialog's rows with base, mine, theirs and the side chosen.
        Dialog::DiscardStation
        | Dialog::ConfirmUpgrade { .. }
        | Dialog::OpenEwz { .. }
        | Dialog::ImportDrawing { .. }
        | Dialog::ImportEplan { .. }
        | Dialog::ConfirmGitSetup { .. }
        | Dialog::ChooseStation { .. } => {
            for line in &spec.body {
                lines.push(format!("  label \"{}\"", user(line)));
            }
        }
        // The import card's paragraphs, then its groups of findings as the dialog lists them.
        Dialog::ImportQet { groups, .. } => import_qet_dialog_lines(groups, &spec, lines),
        // The proposal's paragraph; the re-import card's summary and its listed rows.
        Dialog::ReimportMatch { .. } => {
            for line in &spec.body {
                lines.push(format!("  label \"{}\"", user(line)));
            }
        }
        Dialog::Reimport { rows, .. } => {
            for line in &spec.body {
                lines.push(format!("  label \"{}\"", user(line)));
            }
            for row in rows.iter().take(crate::dialogs::IMPORT_PAGES_LISTED) {
                lines.push(format!("  row \"{}\"", user(row)));
            }
        }
        Dialog::ExternalChange { rows } => {
            for line in &spec.body {
                lines.push(format!("  label \"{}\"", user(line)));
            }
            for (i, r) in rows.iter().enumerate() {
                let take = match r.take {
                    ew_api::external::Take::Mine => "mine",
                    ew_api::external::Take::Theirs => "theirs",
                };
                lines.push(format!(
                    "  row {} \"{} · {}\" base \"{}\" mine \"{}\" theirs \"{}\" chosen {take}",
                    i + 1,
                    user(&r.record),
                    user(&r.field),
                    user(&r.base),
                    user(&r.mine),
                    user(&r.theirs),
                ));
            }
        }
        // The time of the last change is the window's only (never in the transcript).
        Dialog::Recovery {
            name,
            transactions,
            base_matches,
            failed,
            ..
        } => {
            lines.push(format!(
                "  label \"{}\"",
                user(&strings::format(key!("recovery-text"), &[name]))
            ));
            let base = if *base_matches { "matches" } else { "moved" };
            let mut line = format!("  recovery transactions {transactions} base {base}");
            if let Some(reason) = failed {
                line.push_str(&format!(" failed \"{}\"", user(reason)));
            }
            lines.push(line);
        }
        Dialog::GotoPage { text } => {
            lines.push(format!(
                "  field \"{}\": \"{text}\"",
                strings::text(key!("goto-hint"))
            ));
        }
        Dialog::SaveWorkspace { text } => {
            lines.push(format!(
                "  field \"{}\": \"{text}\"",
                strings::text(key!("save-workspace-hint"))
            ));
        }
        // At most 20 file names, then "and <n> more".
        Dialog::ConfirmPurge { files, .. } => purge_dialog_lines(files, lines),
        // The query, the window of results (50 rows, never the total) and the active row's
        // detail.
        Dialog::PartChoose(c) => part_choose_lines(model, c, lines),
        // The options, then the preview: its page and stamp, the progress, the empty state or the
        // failure.
        Dialog::Print(d) => print_lines(model, d, lines),
        // A creation dialog, one `field` line per field in order; the refusal of the last attempt
        // follows the fields as an `error:` line.
        Dialog::Form(form) => form_dialog_lines(model, form, &spec, lines),
        // New project: the name, the folder, the lines of Start from with what each brings, the pages box where the
        // chosen line has pages, and why Create cannot run.
        Dialog::NewProject(d) => new_project_dialog_lines(d, lines),
        // Templates: each template with what it holds and whether new projects start from it, the selected one, and
        // the line under the list.
        Dialog::Templates(d) => templates_dialog_lines(d, lines),
        // Text styles: each style by its group and name with its height, typeface, bold and the note of a small one;
        // the style it opened at, and the line after a change.
        Dialog::TextStyles(d) => text_styles_dialog_lines(model, d, lines),
        // Rework list…: what it compares with, the work, the headings and the rows, or why there is no list.
        Dialog::Rework(d) => rework_dialog_lines(d, lines),
        // Package…: each member with its tick, what it holds and its form, what it lacks; the check's line and its
        // findings (a shown one with its records); the revision; Without personal data; the file.
        Dialog::Package(d) => {
            for r in &d.rows {
                let tick = if r.on { "[x]" } else { "[ ]" };
                lines.push(format!(
                    "  member {tick} {} \"{}\" \"{}\" \"{}\"",
                    r.member.key(),
                    r.words,
                    r.holds,
                    r.form
                ));
                // Each with its way to the stock: Workshop stock… (`parts.stock`).
                for lack in &r.lacks {
                    lines.push(format!("    lacks \"{lack}\" offers parts.stock"));
                }
            }
            lines.push(format!("  check \"{}\"", d.ready));
            for f in &d.findings {
                lines.push(format!(
                    "  finding {} {} \"{}\"",
                    f.rule,
                    f.severity.name(),
                    f.message
                ));
                if f.shown {
                    lines.push(format!("    shows \"{}\"", f.targets.join(", ")));
                }
            }
            for l in &d.lengths {
                lines.push(format!("  length \"{l}\""));
            }
            lines.push(format!("  revision \"{}\"", d.revision.0));
            let personal = if d.personal { "[x]" } else { "[ ]" };
            lines.push(format!("  personal {personal}"));
            lines.push(format!("  file \"{}\"", user(&d.file_name())));
            for line in &spec.body {
                lines.push(format!("  label \"{line}\""));
            }
        }
        // Labels…: which labels, the label file's profile, each file with its tick and count, or why there is none.
        Dialog::Labels(d) => {
            let which = match &d.which {
                crate::shell::labels::Which::All => "all".to_owned(),
                crate::shell::labels::Which::Release(i) => {
                    format!(
                        "since \"{}\"",
                        d.releases.get(*i).map(|(r, _)| r.label.as_str()).unwrap_or("")
                    )
                }
                crate::shell::labels::Which::File(_) => "since archive file".to_owned(),
            };
            lines.push(format!("  labels {which}"));
            lines.push(format!("  profile \"{}\"", d.profile));
            match &d.files {
                Ok(files) => {
                    for f in files {
                        lines.push(format!(
                            "  file {} \"{}\" \"{}\" {} \"{}\"",
                            if f.on { "[x]" } else { "[ ]" },
                            f.set,
                            f.marker_part,
                            f.count,
                            f.name
                        ));
                    }
                    if !d.misfits.is_empty() {
                        lines.push(format!("  misfits {}", d.misfits.len()));
                    }
                    if d.show_misfits {
                        for m in &d.misfits {
                            lines.push(format!("  misfit \"{m}\""));
                        }
                    }
                }
                Err(why) => lines.push(format!("  why \"{why}\"")),
            }
        }
    }
    for b in spec.buttons() {
        match &b.disabled {
            Some(reason) => lines.push(format!("  button \"{}\" disabled: {reason}", b.label)),
            None => lines.push(format!("  button \"{}\"", b.label)),
        }
    }
}

/// The lines of the PLC import's card.
fn plc_import_dialog_lines(
    card: &crate::shell::plc_exchange::PlcImportCard,
    spec: &crate::dialogs::DialogSpec,
    lines: &mut Vec<String>,
) {
    for line in &spec.body {
        lines.push(format!("  label \"{}\"", user(line)));
    }
    lines.push(format!("  tab {}", card.tab.word()));
    for c in card.changes.iter().chain(&card.conflicts) {
        let choice = card.choices.get(&c.id).map_or(c.default, String::as_str);
        lines.push(format!(
            "  change \"{}\" \"{}\" \"{}\" {} {choice}",
            user(&c.text),
            user(&c.ours),
            user(&c.theirs),
            c.state.word()
        ));
    }
    for r in &card.removed {
        lines.push(format!("  removed \"{}\"", user(r)));
    }
    for p in &card.parking {
        let choice = card.choices.get(&p.id).map(String::as_str).unwrap_or("default");
        lines.push(format!("  parked \"{}\" {choice}", user(&p.text)));
    }
}

/// The lines of the QET import's card.
fn import_qet_dialog_lines(
    groups: &[crate::shell::ImportGroup],
    spec: &crate::dialogs::DialogSpec,
    lines: &mut Vec<String>,
) {
    for line in &spec.body {
        lines.push(format!("  label \"{}\"", user(line)));
    }
    for group in groups {
        lines.push(format!("  group \"{}\"", user(&group.heading)));
        for row in group.rows.iter().take(crate::dialogs::IMPORT_KINDS_LISTED) {
            lines.push(format!("    kind \"{}\"", user(row)));
        }
        if let Some(more) = crate::dialogs::import_more_kinds(group) {
            lines.push(format!("    more \"{}\"", user(&more)));
        }
    }
}

/// The lines of the dialog that confirms the purge of unused files.
fn purge_dialog_lines(files: &[String], lines: &mut Vec<String>) {
    lines.push(format!(
        "  label \"{}\"",
        strings::text(key!("confirm-purge-text"))
    ));
    for f in files.iter().take(crate::shell::PURGE_LISTED) {
        lines.push(format!("  item \"{}\"", user(f)));
    }
    if files.len() > crate::shell::PURGE_LISTED {
        let more = (files.len() - crate::shell::PURGE_LISTED).to_string();
        lines.push(format!(
            "  label \"{}\"",
            strings::format(key!("confirm-purge-more"), &[&more])
        ));
    }
}

/// The lines of a form dialog.
fn form_dialog_lines(
    model: &ShellModel,
    form: &crate::shell::FormDialog,
    spec: &crate::dialogs::DialogSpec,
    lines: &mut Vec<String>,
) {
    // A revision dialog's paragraphs and its list (the gate's findings, the release's steps)
    // before its fields.
    // The Auto-fill rail dialog's line about its devices too.
    for line in &spec.body {
        lines.push(format!("  label \"{}\"", user(line)));
    }
    if form.action.starts_with("revision.") {
        for item in model.revision_list() {
            lines.push(format!("  item \"{}\"", user(item)));
        }
    }
    for f in form.fields.iter().filter(|f| form.shows(&f.name)) {
        // A run-time choice by its words, never its value (an ID).
        let value = f
            .options
            .iter()
            .find(|(v, _)| *v == f.value)
            .map_or(f.value.as_str(), |(_, w)| w.as_str());
        lines.push(format!("  field \"{}\": \"{}\"", f.label_text(), value));
    }
    if let Some(e) = &form.error {
        lines.push(format!("  error: {e}"));
    }
}

/// The lines of the New project dialog.
fn new_project_dialog_lines(d: &crate::shell::NewProjectDialog, lines: &mut Vec<String>) {
    lines.push(format!(
        "  field \"{}\": \"{}\"",
        strings::text(key!("new-project-name")),
        user(&d.name)
    ));
    lines.push(format!(
        "  field \"{}\": \"{}\"",
        strings::text(key!("new-project-folder")),
        user(&d.folder)
    ));
    for (i, line) in d.starts.iter().enumerate() {
        lines.push(format!(
            "  start {}{}{} \"{}\": \"{}\"",
            i + 1,
            if i == d.chosen { " (chosen)" } else { "" },
            if line.star { " (for new projects)" } else { "" },
            user(&line.name),
            user(&line.what)
        ));
    }
    if d.start().pages > 0 {
        lines.push(format!(
            "  check \"{}\": {}",
            user(&strings::format_count(
                crate::count_key!("new-project-copy-pages"),
                d.start().pages
            )),
            if d.pages { "on" } else { "off" }
        ));
    }
    for e in d.problem().iter().chain(&d.error) {
        lines.push(format!("  error: {e}"));
    }
    if let Some(note) = d.note().filter(|_| d.error.is_none()) {
        lines.push(format!("  note: {note}"));
    }
}

/// The lines of the Templates dialog.
fn templates_dialog_lines(d: &crate::shell::TemplatesDialog, lines: &mut Vec<String>) {
    if d.rows.is_empty() {
        lines.push(format!("  label \"{}\"", strings::text(key!("templates-none"))));
    }
    for (i, row) in d.rows.iter().enumerate() {
        lines.push(format!(
            "  template {}{}{} \"{}\": \"{}\"",
            i + 1,
            if i == d.selected { " (selected)" } else { "" },
            if row.star { " (for new projects)" } else { "" },
            user(&row.name),
            user(&row.what)
        ));
    }
    let (row, list) = crate::dialogs::templates_actions(d);
    for b in row.iter().chain(&list) {
        lines.push(format!("  button \"{}\"", user(&b.label)));
    }
    if let Some(note) = &d.note {
        lines.push(format!("  note \"{}\"", user(note)));
    }
}

/// The lines of the Rework list… dialog.
fn rework_dialog_lines(d: &crate::shell::rework::ReworkDialog, lines: &mut Vec<String>) {
    use crate::shell::rework::Against;
    let against = match &d.against {
        Against::Nothing => String::new(),
        Against::Release(i) => d.releases.get(*i).map(|(_, w)| user(w)).unwrap_or_default(),
        Against::File(path) => user(
            &path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ),
    };
    lines.push(format!(
        "  field \"{}\": \"{against}\"",
        strings::text(key!("rework-compare-with"))
    ));
    match &d.list {
        Ok(list) => {
            lines.push(format!("  label \"{}\"", user(&list.work)));
            if !list.rows.is_empty() {
                lines.push(format!("  header \"{}\"", user(&list.header.join("\" \""))));
            }
            for row in &list.rows {
                lines.push(format!("  row \"{}\"", user(&row.join("\" \""))));
            }
        }
        Err(why) => lines.push(format!("  note \"{}\"", user(why))),
    }
}

/// The lines of the Text styles dialog.
fn text_styles_dialog_lines(
    model: &ShellModel,
    d: &crate::shell::text_styles::TextStylesDialog,
    lines: &mut Vec<String>,
) {
    for row in model.text_style_rows() {
        lines.push(format!(
            "  style \"{}\"{} {}: {} mm, {}{}{}",
            user(&row.name),
            if d.focus.as_deref() == Some(row.key.as_str()) {
                " (opened at)"
            } else {
                ""
            },
            crate::shell::text_styles::group_words(row.group),
            row.height,
            crate::shell::text_styles::face_words(&row.face),
            if row.bold { ", bold" } else { "" },
            row.small.as_ref().map(|s| format!(" ({s})")).unwrap_or_default(),
        ));
    }
    if let Some(note) = &d.note {
        lines.push(format!("  note \"{}\"", user(note)));
    }
}

/// The start-up offer's lines: its title, the crash line, one row per project, the refusal of the last
/// answer and the buttons, as the window's dialog shows them.
pub(super) fn startup_lines(model: &ShellModel, lines: &mut Vec<String>) {
    let Some(offer) = model.startup_offer() else {
        return;
    };
    let spec = offer.spec();
    lines.push(format!("startup \"{}\"", user(&spec.title)));
    for line in &spec.body {
        lines.push(format!("  label \"{}\"", user(line)));
    }
    for (i, p) in offer.projects.iter().enumerate() {
        lines.push(format!(
            "  project {} \"{}\" path \"{}\"",
            i + 1,
            user(&crate::shell::startup::StartupOffer::project_row(p)),
            user(&p.path.to_string_lossy())
        ));
    }
    if let Some(e) = &offer.refusal {
        lines.push(format!("  error: {}", user(e)));
    }
    for b in spec.buttons() {
        lines.push(format!("  button \"{}\"", b.label));
    }
}

/// The print pre-dialog's lines: `range`, the section rows or the Pages field with its refusal,
/// `paper`, the two check boxes, then the preview.
fn print_lines(model: &ShellModel, d: &crate::shell::print::PrintDialog, lines: &mut Vec<String>) {
    use crate::shell::print::{PrintBuild, PrintDialog, PrintRange};
    lines.push(format!(
        "  field \"{}\": \"{}\"",
        strings::text(key!("print-range")),
        strings::text(d.range.label_key())
    ));
    if d.range == PrintRange::Sections {
        for s in &d.sections {
            let tick = if s.ticked { "ticked" } else { "not ticked" };
            lines.push(format!("  check \"{}\" {tick}", user(&s.label())));
        }
    }
    if d.range == PrintRange::Pages {
        lines.push(format!(
            "  field \"{}\": \"{}\"",
            strings::text(key!("print-range-pages")),
            user(&d.pages)
        ));
        if let Some(e) = &d.pages_error {
            lines.push(format!("  error: {}", user(e)));
        }
    }
    lines.push(format!(
        "  field \"{}\": \"{}\"",
        strings::text(key!("print-paper")),
        PrintDialog::paper_text(d.paper)
    ));
    for (key, on) in [(key!("print-fit"), d.fit), (key!("print-mono"), d.monochrome)] {
        let tick = if on { "ticked" } else { "not ticked" };
        lines.push(format!("  check \"{}\" {tick}", strings::text(key)));
    }
    match &d.build {
        PrintBuild::Built { .. } => {
            if let (Some(set), Some(caption)) = (model.print_set(), model.print_page_text()) {
                let folio = set.pages.get(d.page).map_or("", |p| p.label.as_str());
                let mut line = format!("  preview \"{caption}\" page {}", user(folio));
                if let Some(Some(stamp)) = set.stamps.get(d.page) {
                    line.push_str(&format!(" stamp \"{stamp}\""));
                }
                lines.push(line);
            }
        }
        PrintBuild::Building => {
            let text = model.print_progress_text().unwrap_or_default();
            lines.push(format!(
                "  preview {} \"{text}\"",
                crate::jobs::JobKind::Print.word()
            ));
        }
        PrintBuild::Empty => lines.push(format!(
            "  preview \"{}\" \"{}\"",
            strings::text(key!("print-empty-title")),
            strings::text(key!("print-empty-text"))
        )),
        PrintBuild::Failed(cause) => lines.push(format!(
            "  preview error: {}",
            user(&strings::format(key!("print-failed"), &[cause]))
        )),
    }
}
