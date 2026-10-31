// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The revision dialogs of Check ▸ Revision (WP-S1.18 part 5, §9): Approve revision, Release revision, Withdraw
//! revision and Set revision author, and the offer of an incomplete release when a project opens; with the release's
//! housekeeping warnings in Messages (FT-842).
//!
//! Each dialog is a form of its own action with parameters (as `project.purge_assets` with `confirm`): without
//! `confirm` the action opens the form, or fills the open one with the parameters given; `cancel` closes it; with
//! `confirm` it validates the fields (the typed label, the role, the reason, the date the window passed, the git
//! consent), then submits the `HumanOnly` command.
//!
//! **Who may confirm** (cycle 41, ruling 1: "the `HumanOnly` gate is per submission"). The submission carries the
//! shell's own origin ([`ShellModel::origin`]): `Origin::Gui` only in the window a person drives. The headless shell and
//! a window a script drives ([`ShellModel::new_scripted`], the visual driver) open, fill, validate and frame the
//! dialogs, but their confirm is refused with `error-revision-person`, and the session judges the same origin again
//! (`Session::approve_as`, `release_as`, `complete_release_as`, `discard_incomplete_release_as`, and `execute_with` for
//! withdraw and set author), so no parameter or script line reaches a release.

use std::path::{Path, PathBuf};

use ew_api::checks::{Finding, IntegrityItem};
use ew_api::output::Parallelism;
use ew_api::{
    AnyCommand, Approval, IncompleteRelease, PartyRole, ReleaseContext, ReleaseError, ReleaseOptions,
    ReleasePlan, ReleaseTrouble, Submit, TroubleKind,
};
use serde_json::{Map, Value};

use super::creation::{FormDialog, FormField};
use super::{Dialog, Outcome, ShellModel, failed, string_param};
use crate::strings;

/// The action of Approve revision.
pub(crate) const APPROVE: &str = "revision.approve";
/// The action of Release revision.
pub(crate) const RELEASE: &str = "revision.release";
/// The action of Withdraw revision.
pub(crate) const WITHDRAW: &str = "revision.withdraw";
/// The action of Set revision author.
pub(crate) const SET_AUTHOR: &str = "revision.set_author";
/// The action of the incomplete-release offer.
pub(crate) const INCOMPLETE: &str = "revision.incomplete";

/// The party roles a role field offers (FR-PRJ-003), the checker's first.
const ROLES: &[&str] = &[
    "checker",
    "engineer",
    "author",
    "panel_builder",
    "customer",
    "end_user",
];
/// The yes-or-no choice of a check box (FT-558).
const TOGGLE: &[&str] = &["false", "true"];
/// The most findings a dialog lists before "and <n> more".
const LISTED: usize = 8;

/// The revision dialogs' view state.
#[derive(Default)]
pub(crate) struct RevisionState {
    /// The integrity findings of opening the project (the gate joins them, as `ew check` does).
    integrity: Vec<IntegrityItem>,
    /// The open dialog's revision label (its title names it).
    pub(crate) label: String,
    /// The open dialog's paragraphs (at most three).
    pub(crate) body: Vec<String>,
    /// The list under the paragraphs: the gate's findings or the release's steps.
    pub(crate) list: Vec<String>,
    /// Whether the open dialog cannot be confirmed (the gate blocks, or cannot run): it shows Close only.
    pub(crate) blocked: bool,
    /// The date the window passed with the action (`YYYY-MM-DD`; the shell never reads a clock).
    date: Option<String>,
    /// The incomplete release the open offer is about (the found archive's facts).
    pub(crate) incomplete: Option<IncompleteRelease>,
}

impl std::fmt::Debug for RevisionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevisionState")
            .field("label", &self.label)
            .field("blocked", &self.blocked)
            .finish_non_exhaustive()
    }
}

/// The last path component of `path` (a text in words names a file, never its folder path).
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// `text` with every path of `paths` (longest first) replaced by its file name.
fn path_free(text: &str, paths: &[PathBuf]) -> String {
    let mut sorted: Vec<&PathBuf> = paths.iter().collect();
    sorted.sort_by_key(|p| std::cmp::Reverse(p.as_os_str().len()));
    let mut out = text.to_owned();
    for p in sorted {
        let shown = p.display().to_string();
        if !shown.is_empty() {
            out = out.replace(&shown, &file_name(p));
        }
    }
    out
}

/// One finding as a dialog lists it: its subject and message in words (no rule ID).
fn finding_line(f: &Finding) -> String {
    if f.subject.is_empty() || f.message.starts_with(&f.subject) {
        f.message.clone()
    } else {
        strings::format("revision-finding", &[&f.subject, &f.message])
    }
}

/// The listed findings: at most [`LISTED`], then "and <n> more".
fn finding_list<'a>(findings: impl ExactSizeIterator<Item = &'a Finding>) -> Vec<String> {
    let total = findings.len();
    let mut list: Vec<String> = findings.take(LISTED).map(finding_line).collect();
    if total > LISTED {
        list.push(strings::format("revision-more", &[&(total - LISTED).to_string()]));
    }
    list
}

/// A party role from its file name (`checker`); `None` for an unknown word.
fn role_of(word: &str) -> Option<PartyRole> {
    match serde_json::from_value::<PartyRole>(Value::String(word.to_owned())) {
        Ok(PartyRole::Unknown(_)) | Err(_) => None,
        Ok(r) => Some(r),
    }
}

/// A party role in words (`panel builder`).
fn role_words(role: &PartyRole) -> String {
    ew_api::role_name(role).replace('_', " ")
}

/// A form field.
fn field(
    name: &'static str,
    label_key: &'static str,
    choices: &'static [&'static str],
    value: String,
) -> FormField {
    FormField {
        name,
        label_key,
        choices,
        value,
    }
}

/// The release's housekeeping warning in words with the file name only (FT-842).
pub(crate) fn trouble_text(t: &ReleaseTrouble) -> String {
    let key = match t.what {
        TroubleKind::RecordNotRemoved => "release-trouble-not-removed",
        TroubleKind::RecordsNotRead => "release-trouble-not-read",
        TroubleKind::RecordNotUpdated => "release-trouble-not-updated",
    };
    strings::format(
        key,
        &[
            &t.file_name(),
            &path_free(&t.error, std::slice::from_ref(&t.path)),
        ],
    )
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Reading

    /// The open revision's label, if the open project has one.
    pub fn open_revision_label(&self) -> Option<String> {
        self.project.as_ref()?.session.open_revision_label()
    }

    /// The labels of the open project's released revisions that are not withdrawn.
    pub fn released_labels(&self) -> Vec<String> {
        self.project
            .as_ref()
            .map(|p| p.session.released_labels())
            .unwrap_or_default()
    }

    /// Whether an incomplete release was found when the project opened (and is not settled yet).
    pub fn has_incomplete_release(&self) -> bool {
        self.revision.incomplete.is_some()
    }

    /// The list under the open revision dialog's paragraphs: the gate's findings or the release's steps (the window
    /// draws it in `type.small`; `ui.describe` lists it as `item` lines).
    pub fn revision_list(&self) -> &[String] {
        if self.in_revision_form() {
            &self.revision.list
        } else {
            &[]
        }
    }

    /// The open revision dialog's paragraphs (at most three), its revision label and whether it can be confirmed
    /// (`false`: the gate blocks or cannot run, and the dialog shows Close only); `None` without a revision dialog.
    pub fn revision_dialog(&self) -> Option<(&[String], &str, bool)> {
        self.in_revision_form().then_some((
            self.revision.body.as_slice(),
            self.revision.label.as_str(),
            !self.revision.blocked,
        ))
    }

    /// What the incomplete-release offer lets the person do: complete the release with the archive, discard the
    /// archive (each `false` when the release refuses it).
    pub fn incomplete_choices(&self) -> (bool, bool) {
        self.revision
            .incomplete
            .as_ref()
            .map_or((false, false), |f| (f.refusal.is_none(), f.unproven.is_none()))
    }

    /// Whether the open dialog is one of the revision forms.
    fn in_revision_form(&self) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action.starts_with("revision."))
    }

    /// Whether the open dialog is the form of `action`.
    fn in_form_of(&self, action: &str) -> bool {
        matches!(&self.dialog, Some(Dialog::Form(f)) if f.action == action)
    }

    /// The value of field `name` of the open form, trimmed.
    fn form_value(&self, name: &str) -> String {
        match &self.dialog {
            Some(Dialog::Form(f)) => f
                .fields
                .iter()
                .find(|x| x.name == name)
                .map(|x| x.value.trim().to_owned())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// A refusal of the open revision form's fields: it stays in the form.
    fn revision_failed(&mut self, message: String) -> Outcome {
        if self.in_revision_form()
            && let Some(Dialog::Form(f)) = &mut self.dialog
        {
            f.error = Some(message.clone());
        }
        Outcome::Failed { message }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Opening a project, draining the release's warnings

    /// After a project opened (WP-S1.18 part 5): keeps its integrity findings for the gate, shows the release's
    /// housekeeping warnings of opening (FT-842), and offers an incomplete release of the open revision when there
    /// is one and no other dialog is open.
    pub(super) fn revision_opened(&mut self, integrity: Vec<IntegrityItem>) {
        self.revision = RevisionState {
            integrity,
            ..RevisionState::default()
        };
        if self.is_writable() {
            let found = self
                .project
                .as_ref()
                .and_then(|p| p.session.incomplete_release(None).ok().flatten());
            if let Some(found) = found.filter(|f| f.foreign.is_none()) {
                self.revision.incomplete = Some(found);
                if self.dialog.is_none() {
                    self.open_incomplete();
                }
            }
        }
        self.drain_release_troubles();
    }

    /// FT-842 (cycle 41, ruling 2): drains `Session::take_release_troubles` and shows each warning once as a Messages
    /// entry (severity Warning, rule column "Release", the file name only), with one toast that points to Messages. An
    /// empty drain shows nothing.
    pub(super) fn drain_release_troubles(&mut self) {
        let Some(project) = &self.project else {
            return;
        };
        let epoch = project.epoch;
        let mut shown: Vec<String> = match &self.messages.release_notes {
            Some((e, notes)) if *e == epoch => notes.clone(),
            _ => Vec::new(),
        };
        let before = shown.len();
        // Each once: the same warning of the same file (the open's sweep and the incomplete-release scan read the
        // same records) is one entry.
        for text in project.session.take_release_troubles().iter().map(trouble_text) {
            if !shown.contains(&text) {
                shown.push(text);
            }
        }
        let count = shown.len() - before;
        if count == 0 {
            return;
        }
        self.messages.release_notes = Some((epoch, shown));
        // The Messages table is rebuilt with them at its next fetch, whatever the revision.
        if let Some(n) = self.navs.get_mut(&crate::navigators::NavKind::Messages) {
            n.table = None;
        }
        self.fetch_messages_table();
        // Its own key, so it does not replace the release's toast (a toast of a visible key replaces that one).
        self.notify_with(
            strings::format("toast-release-troubles", &[&count.to_string()]),
            "release",
            false,
        );
    }

    /// The shared start of every revision action: a writable project, the `cancel` parameter, and the date the window
    /// passed (kept for the confirm). `Some` when the action is done.
    fn revision_start(&mut self, action: &str, p: &Map<String, Value>) -> Option<Outcome> {
        if !self.has_project() {
            return Some(Outcome::Refused {
                reason_key: "reason-no-project",
            });
        }
        if !self.is_writable() {
            return Some(Outcome::Refused {
                reason_key: "reason-read-only",
            });
        }
        match super::jobs::bool_param(p, "cancel", false) {
            Err(e) => return Some(e),
            Ok(true) => {
                if self.in_form_of(action) {
                    self.dialog = None;
                }
                return Some(Outcome::Done);
            }
            Ok(false) => {}
        }
        match string_param(p, "date") {
            Err(e) => return Some(e),
            Ok(Some(d)) if !d.trim().is_empty() => self.revision.date = Some(d.trim().to_owned()),
            Ok(_) => {}
        }
        None
    }

    /// Fills the open form's fields from the text parameters `names` that `p` carries.
    fn fill_form(&mut self, p: &Map<String, Value>, names: &[&str]) -> Result<(), Outcome> {
        for name in names {
            let value = match p.get(*name) {
                None => continue,
                Some(Value::String(s)) => s.clone(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(_) => return Err(failed("error-bad-param", &[name])),
            };
            if let Some(Dialog::Form(f)) = &mut self.dialog
                && let Some(x) = f.fields.iter_mut().find(|x| x.name == *name)
            {
                x.value = value;
                f.error = None;
            }
        }
        Ok(())
    }

    /// The date the window passed, parsed; the refusal names the missing parameter.
    fn revision_date(&mut self) -> Result<ew_api::catalog::Date, Outcome> {
        match self
            .revision
            .date
            .as_deref()
            .map(str::parse::<ew_api::catalog::Date>)
        {
            Some(Ok(d)) => Ok(d),
            _ => Err(self.revision_failed(strings::format("error-missing-param", &["date"]))),
        }
    }

    /// Cycle 41, ruling 1: the confirm of a `HumanOnly` step is refused to every shell no person drives (the headless
    /// shell, a window a script drives), with the whole text in words; the refusal stays in the dialog.
    fn person_gate(&mut self) -> Result<(), Outcome> {
        if self.person {
            Ok(())
        } else {
            Err(self.revision_failed(strings::text("error-revision-person").to_owned()))
        }
    }

    /// Closes the open revision form after a completed step.
    fn revision_done(&mut self) {
        if self.in_revision_form() {
            self.dialog = None;
        }
        self.revision.body.clear();
        self.revision.list.clear();
        self.revision.blocked = false;
    }

    /// Opens a revision form.
    fn open_revision_form(
        &mut self,
        action: &'static str,
        title_key: &'static str,
        ok_key: &'static str,
        fields: Vec<FormField>,
    ) -> Outcome {
        self.open_dialog(Dialog::Form(FormDialog {
            action,
            title_key,
            ok_key,
            fields,
            error: None,
            table: None,
        }))
    }

    /// The release options of the shell: the default archive root beside the project directory, the `git` executable.
    fn release_options<'g>(git: &'g ew_api::git::GitExe, consent: bool) -> ReleaseOptions<'g> {
        ReleaseOptions {
            archive_root: None,
            git,
            git_consent: consent,
            parallelism: Parallelism::threads(
                std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
            ),
        }
    }

    /// The paths a release text may name: the project directory, its parent and the release's archive.
    fn release_paths(&self, more: &[&Path]) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = more.iter().map(|p| p.to_path_buf()).collect();
        if let Some(p) = &self.project {
            paths.extend(ew_api::archive::default_root(&p.dir));
            paths.push(p.dir.clone());
        }
        paths
    }

    /// A release error in words, with file names instead of paths.
    fn release_error_text(&self, e: &ReleaseError) -> String {
        let mut paths: Vec<&Path> = Vec::new();
        match e {
            ReleaseError::ArchiveRoot { root, .. } => paths.push(root),
            ReleaseError::Incomplete { archive } => paths.push(archive),
            ReleaseError::GitConsent { worktree, .. } => paths.push(worktree),
            ReleaseError::Io { path, .. } => paths.push(path),
            _ => {}
        }
        if let Some(found) = &self.revision.incomplete {
            paths.push(&found.archive);
        }
        path_free(&e.to_string(), &self.release_paths(&paths))
    }

    /// Marks the project saved after a step that saved it (the history panel's marker line, the script record).
    fn revision_saved(&mut self) {
        self.record.saved_steps = self.record.steps.clone();
        if let Some(p) = &self.project {
            let rev = p.session.world().rev.0;
            self.markers.push(super::HistoryMarker {
                rev,
                text: strings::text("marker-saved").to_owned(),
            });
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Approve revision

    /// `revision.approve {role?, typed?, date?, confirm?, cancel?}` (Check ▸ Revision ▸ Approve revision…): the checker's
    /// sign-off on the open revision (`revision.approve` through `Session::approve_as`). The form shows the gate's
    /// result on the saved project (refused while any error other than EW-REL-001 is open, and without an author), the
    /// safety waivers the sign-off acknowledges, the checker's role and the label typed to confirm; the approval is
    /// saved, as `ew revision approve` saves it.
    pub(super) fn act_revision_approve(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(out) = self.revision_start(APPROVE, p) {
            return out;
        }
        let Some(label) = self.open_revision_label() else {
            return Outcome::Refused {
                reason_key: "reason-no-open-revision",
            };
        };
        if !self.in_form_of(APPROVE) {
            self.open_approve(&label);
        }
        if let Err(e) = self.fill_form(p, &["role", "typed"]) {
            return e;
        }
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c,
            Err(e) => return e,
        };
        if !confirm {
            return Outcome::Done;
        }
        if self.revision.blocked {
            let message = self.revision.body.join(" ");
            return self.revision_failed(message);
        }
        let Some(role) = role_of(&self.form_value("role")) else {
            return self.revision_failed(strings::text("error-revision-role").to_owned());
        };
        if self.form_value("typed") != label {
            return self.revision_failed(strings::format("error-revision-typed", &[&label]));
        }
        let date = match self.revision_date() {
            Ok(d) => d,
            Err(e) => return e,
        };
        let gate = match self
            .project
            .as_ref()
            .map(|p| p.session.release_gate(&label, &self.revision.integrity))
        {
            Some(Ok(g)) => g,
            Some(Err(e)) => {
                return self.revision_failed(strings::format("revision-gate-error", &[&e.to_string()]));
            }
            None => return Outcome::Done,
        };
        if let Err(e) = self.person_gate() {
            return e;
        }
        let approval = Approval {
            label: label.clone(),
            checker: role.clone(),
            date,
            waivers_ack: gate.safety_waivers.clone(),
        };
        let origin = self.origin();
        let integrity = std::mem::take(&mut self.revision.integrity);
        let result = match &mut self.project {
            Some(p) => p.session.approve_as(&origin, &approval, &integrity),
            None => return Outcome::Done,
        };
        self.revision.integrity = integrity;
        if let Err(e) = result {
            return self.revision_failed(strings::format("error-revision-approve", &[&e.to_string()]));
        }
        self.revision_done();
        let saved = self.act_save();
        self.notify_with(
            strings::format("toast-revision-approved", &[&label]),
            "edit",
            false,
        );
        saved
    }

    /// Opens the Approve form: the gate's result on the saved project, the author, the waivers.
    fn open_approve(&mut self, label: &str) {
        let Some(project) = &self.project else {
            return;
        };
        let author = project
            .session
            .open_revision_roles()
            .and_then(|(_, author, _)| author);
        let gate = project.session.release_gate(label, &self.revision.integrity);
        let mut body = Vec::new();
        let mut list = Vec::new();
        let mut blocked = true;
        match (&author, gate) {
            (None, _) => body.push(strings::format("revision-no-author", &[label])),
            (_, Err(e)) => body.push(strings::format("revision-gate-error", &[&e.to_string()])),
            (Some(author), Ok(gate)) => {
                let blockers = gate.approval_blockers();
                if blockers.is_empty() {
                    blocked = false;
                    body.push(strings::format(
                        "revision-approve-text",
                        &[label, &role_words(author)],
                    ));
                    if !gate.safety_waivers.is_empty() {
                        body.push(strings::format(
                            "revision-waivers",
                            &[&gate.safety_waivers.len().to_string()],
                        ));
                    }
                } else {
                    body.push(strings::format(
                        "revision-approve-blocked",
                        &[&blockers.len().to_string()],
                    ));
                    list = finding_list(blockers.into_iter());
                }
            }
        }
        let fields = if blocked {
            Vec::new()
        } else {
            vec![
                field("role", "form-revision-checker", ROLES, "checker".to_owned()),
                field("typed", "form-revision-typed", &[], String::new()),
            ]
        };
        self.revision.label = label.to_owned();
        self.revision.body = body;
        self.revision.list = list;
        self.revision.blocked = blocked;
        self.open_revision_form(APPROVE, "revision-approve-title", "dialog-approve", fields);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Release revision

    /// `revision.release {typed?, git?, date?, confirm?, cancel?}` (Check ▸ Revision ▸ Release revision…): the atomic
    /// release of the open revision (doc 05 §14.1, `Session::release_as`). The form lists the gate's blockers (then it
    /// only closes), or the six steps with the archive and the git tag; the label is typed to confirm, and in a git
    /// repository the commit and the tag are agreed to. Cancel is the Primary; Release is the destructive button. An
    /// incomplete release of the label is offered first ([`ShellModel::act_revision_incomplete`]).
    pub(super) fn act_revision_release(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(out) = self.revision_start(RELEASE, p) {
            return out;
        }
        let Some(label) = self.open_revision_label() else {
            return Outcome::Refused {
                reason_key: "reason-no-open-revision",
            };
        };
        if !self.in_form_of(RELEASE) {
            let found = self
                .project
                .as_ref()
                .and_then(|p| p.session.incomplete_release(None).ok().flatten());
            if let Some(found) = found.filter(|f| f.label == label && f.foreign.is_none()) {
                self.revision.incomplete = Some(found);
                return self.open_incomplete();
            }
            self.open_release(&label);
        }
        if let Err(e) = self.fill_form(p, &["typed", "git"]) {
            return e;
        }
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c,
            Err(e) => return e,
        };
        if !confirm {
            return Outcome::Done;
        }
        if self.revision.blocked {
            let message = self.revision.body.join(" ");
            return self.revision_failed(message);
        }
        if self.form_value("typed") != label {
            return self.revision_failed(strings::format("error-revision-typed", &[&label]));
        }
        let git = ew_api::git::GitExe::default();
        let plan = match self.project.as_ref().map(|p| {
            p.session.judge_release(
                &label,
                &self.revision.integrity,
                &Self::release_options(&git, false),
            )
        }) {
            Some(Ok(plan)) => plan,
            Some(Err(e)) => {
                let text = self.release_error_text(&e);
                return self.revision_failed(strings::format("error-revision-release", &[&text]));
            }
            None => return Outcome::Done,
        };
        let consent = self.form_value("git") == "true";
        if let Some((_, tag)) = &plan.git
            && !consent
        {
            return self.revision_failed(strings::format("error-revision-git", &[tag]));
        }
        let date = match self.revision_date() {
            Ok(d) => d,
            Err(e) => return e,
        };
        if let Err(e) = self.person_gate() {
            return e;
        }
        let ctx = ReleaseContext {
            label: label.clone(),
            date,
        };
        let origin = self.origin();
        let integrity = std::mem::take(&mut self.revision.integrity);
        let result = match &mut self.project {
            Some(p) => p
                .session
                .release_as(&origin, &ctx, &integrity, &Self::release_options(&git, consent)),
            None => return Outcome::Done,
        };
        self.revision.integrity = integrity;
        let out = match result {
            Ok(done) => {
                self.revision_done();
                self.revision_saved();
                self.notify_with(
                    strings::format(
                        "toast-revision-released",
                        &[&done.label, &file_name(&done.archive)],
                    ),
                    "edit",
                    false,
                );
                Outcome::Done
            }
            Err(e) => {
                if let ReleaseError::Blocked(list) = &e {
                    self.revision.list = finding_list(list.iter());
                }
                let text = self.release_error_text(&e);
                self.revision_failed(strings::format("error-revision-release", &[&text]))
            }
        };
        self.drain_release_troubles();
        out
    }

    /// Opens the Release form: the blockers, or the six steps and the fields.
    fn open_release(&mut self, label: &str) {
        let git = ew_api::git::GitExe::default();
        let Some(judged) = self.project.as_ref().map(|p| {
            p.session.judge_release(
                label,
                &self.revision.integrity,
                &Self::release_options(&git, false),
            )
        }) else {
            return;
        };
        let mut list = Vec::new();
        let mut fields = Vec::new();
        let (body, blocked) = match judged {
            Ok(plan) => {
                list = self.release_steps(label, &plan);
                fields.push(field("typed", "form-revision-typed", &[], String::new()));
                if plan.git.is_some() {
                    fields.push(field("git", "form-revision-git", TOGGLE, "false".to_owned()));
                }
                (vec![strings::text("revision-release-text").to_owned()], false)
            }
            Err(ReleaseError::Blocked(findings)) => {
                list = finding_list(findings.iter());
                (
                    vec![strings::format(
                        "revision-release-blocked",
                        &[&findings.len().to_string()],
                    )],
                    true,
                )
            }
            Err(e) => (
                vec![strings::format(
                    "revision-gate-error",
                    &[&self.release_error_text(&e)],
                )],
                true,
            ),
        };
        self.revision.label = label.to_owned();
        self.revision.body = body;
        self.revision.list = list;
        self.revision.blocked = blocked;
        self.open_revision_form(RELEASE, "revision-release-title", "dialog-release", fields);
    }

    /// The six steps of a release as the Release form lists them.
    fn release_steps(&self, label: &str, plan: &ReleasePlan) -> Vec<String> {
        let folder = plan.archive.parent().map(file_name).unwrap_or_default();
        let mut steps = vec![
            strings::text("revision-step-gate").to_owned(),
            strings::text("revision-step-freeze").to_owned(),
        ];
        match &plan.git {
            Some((_, tag)) => {
                steps.push(strings::text("revision-step-commit").to_owned());
                steps.push(strings::format(
                    "revision-step-archive",
                    &[&file_name(&plan.archive), &folder],
                ));
                steps.push(strings::format("revision-step-tag", &[tag]));
            }
            None => {
                steps.push(strings::text("revision-step-no-commit").to_owned());
                steps.push(strings::format(
                    "revision-step-archive",
                    &[&file_name(&plan.archive), &folder],
                ));
                steps.push(strings::text("revision-step-no-tag").to_owned());
            }
        }
        steps.push(strings::format("revision-step-stamp", &[label]));
        steps
    }

    // -----------------------------------------------------------------------------------------------------------
    // The incomplete release

    /// `revision.incomplete {how?, confirm?, cancel?}` (Check ▸ Revision ▸ Settle the incomplete release…, and offered
    /// when the project opens): an archive of the open revision that a crashed release left (doc 05 §14.1 "On
    /// failure"). `how` `complete` stamps the revision released with it (`Session::complete_release_as`), `discard`
    /// removes it and its tag (`Session::discard_incomplete_release_as`), each offered only when the release allows it.
    /// Not now (the Primary) closes the offer.
    pub(super) fn act_revision_incomplete(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(out) = self.revision_start(INCOMPLETE, p) {
            return out;
        }
        let found = self
            .project
            .as_ref()
            .and_then(|p| p.session.incomplete_release(None).ok().flatten())
            .filter(|f| f.foreign.is_none());
        let Some(found) = found else {
            self.revision.incomplete = None;
            return Outcome::Refused {
                reason_key: "reason-no-incomplete-release",
            };
        };
        self.revision.incomplete = Some(found);
        if !self.in_form_of(INCOMPLETE) {
            self.open_incomplete();
        }
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c,
            Err(e) => return e,
        };
        if !confirm {
            return Outcome::Done;
        }
        let how = match string_param(p, "how") {
            Err(e) => return e,
            Ok(Some(h @ ("complete" | "discard"))) => h.to_owned(),
            Ok(Some(_)) => return failed("error-bad-param", &["how"]),
            Ok(None) => return failed("error-missing-param", &["how"]),
        };
        if let Err(e) = self.person_gate() {
            return e;
        }
        let git = ew_api::git::GitExe::default();
        let options = Self::release_options(&git, true);
        let origin = self.origin();
        let label = self
            .revision
            .incomplete
            .as_ref()
            .map(|f| f.label.clone())
            .unwrap_or_default();
        let integrity = std::mem::take(&mut self.revision.integrity);
        let result = match &mut self.project {
            Some(p) if how == "complete" => {
                p.session
                    .complete_release_as(&origin, &integrity, &options)
                    .map(|done| {
                        strings::format(
                            "toast-revision-released",
                            &[&done.label, &file_name(&done.archive)],
                        )
                    })
            }
            Some(p) => p
                .session
                .discard_incomplete_release_as(&origin, &options)
                .map(|_| strings::format("toast-revision-discarded", &[&label])),
            None => return Outcome::Done,
        };
        self.revision.integrity = integrity;
        let out = match result {
            Ok(toast) => {
                if how == "complete" {
                    self.revision_saved();
                }
                self.revision.incomplete = None;
                self.revision_done();
                self.notify_with(toast, "edit", false);
                Outcome::Done
            }
            Err(e) => {
                let text = self.release_error_text(&e);
                self.revision_failed(strings::format("error-revision-settle", &[&text]))
            }
        };
        self.drain_release_troubles();
        out
    }

    /// Opens the offer of the incomplete release in hand.
    fn open_incomplete(&mut self) -> Outcome {
        let Some(found) = &self.revision.incomplete else {
            return Outcome::Done;
        };
        let mut body = vec![strings::format(
            "revision-incomplete-text",
            &[&found.label, &file_name(&found.archive)],
        )];
        let paths = self.release_paths(&[&found.archive]);
        match &found.refusal {
            Some(why) => body.push(strings::format(
                "revision-incomplete-refused",
                &[&path_free(why, &paths)],
            )),
            None => body.push(strings::text("revision-incomplete-complete").to_owned()),
        }
        if found.unproven.is_some() {
            body.push(strings::text("revision-incomplete-unproven").to_owned());
        }
        self.revision.label = found.label.clone();
        self.revision.blocked = found.refusal.is_some() && found.unproven.is_some();
        self.revision.body = body;
        self.revision.list = Vec::new();
        self.open_revision_form(
            INCOMPLETE,
            "revision-incomplete-title",
            "dialog-not-now",
            Vec::new(),
        )
    }

    // -----------------------------------------------------------------------------------------------------------
    // Withdraw revision

    /// `revision.withdraw {revision?, reason?, confirm?, cancel?}` (Check ▸ Revision ▸ Withdraw revision…): a withdrawal
    /// record after a released revision (`revision.withdraw`, doc 15 §5.10); the revision (by its label, the last
    /// released one by default) and the reason are required. Undoable as one step; not saved.
    pub(super) fn act_revision_withdraw(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(out) = self.revision_start(WITHDRAW, p) {
            return out;
        }
        let labels = self.released_labels();
        let Some(last) = labels.last().cloned() else {
            return Outcome::Refused {
                reason_key: "reason-no-released-revision",
            };
        };
        if !self.in_form_of(WITHDRAW) {
            self.revision.label = last.clone();
            self.revision.body = vec![strings::text("revision-withdraw-text").to_owned()];
            self.revision.list = Vec::new();
            self.revision.blocked = false;
            self.open_revision_form(
                WITHDRAW,
                "revision-withdraw-title",
                "dialog-withdraw",
                vec![
                    field("revision", "form-revision-revision", &[], last),
                    field("reason", "form-revision-reason", &[], String::new()),
                ],
            );
        }
        if let Err(e) = self.fill_form(p, &["revision", "reason"]) {
            return e;
        }
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c,
            Err(e) => return e,
        };
        if !confirm {
            return Outcome::Done;
        }
        let label = self.form_value("revision");
        let Some(id) = self
            .project
            .as_ref()
            .and_then(|p| p.session.released_revision(&label))
            .filter(|_| labels.contains(&label))
        else {
            return self.revision_failed(strings::format("error-revision-unknown", &[&label]));
        };
        let reason = self.form_value("reason");
        if reason.is_empty() {
            return self.revision_failed(strings::text("error-revision-reason").to_owned());
        }
        if let Err(e) = self.person_gate() {
            return e;
        }
        let cmd = AnyCommand {
            name: WITHDRAW.to_owned(),
            params: serde_json::json!({ "revision": id.to_string(), "reason": reason }),
        };
        match self.submit_revision_command(&cmd) {
            Ok(()) => {
                self.revision_done();
                self.notify_with(
                    strings::format("toast-revision-withdrawn", &[&label]),
                    "edit",
                    true,
                );
                Outcome::Done
            }
            Err(e) => self.revision_failed(strings::format("error-revision-withdraw", &[&e])),
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Set revision author

    /// `revision.set_author {role?, confirm?, cancel?}` (Check ▸ Revision ▸ Set revision author…): the author of the
    /// open revision as a party role (`revision.set_author`, WP-S1.18 D4); never the checker's. Undoable as one step;
    /// not saved.
    pub(super) fn act_revision_set_author(&mut self, p: &Map<String, Value>) -> Outcome {
        if let Some(out) = self.revision_start(SET_AUTHOR, p) {
            return out;
        }
        let Some((id, author, _)) = self
            .project
            .as_ref()
            .and_then(|p| p.session.open_revision_roles())
        else {
            return Outcome::Refused {
                reason_key: "reason-no-open-revision",
            };
        };
        let label = self.open_revision_label().unwrap_or_default();
        if !self.in_form_of(SET_AUTHOR) {
            let current = author
                .as_ref()
                .map_or_else(|| "author".to_owned(), ew_api::role_name);
            self.revision.label = label.clone();
            self.revision.body = vec![strings::format("revision-author-text", &[&label])];
            self.revision.list = Vec::new();
            self.revision.blocked = false;
            self.open_revision_form(
                SET_AUTHOR,
                "revision-author-title",
                "dialog-set-author",
                vec![field("role", "form-revision-author", ROLES, current)],
            );
        }
        if let Err(e) = self.fill_form(p, &["role"]) {
            return e;
        }
        let confirm = match super::jobs::bool_param(p, "confirm", false) {
            Ok(c) => c,
            Err(e) => return e,
        };
        if !confirm {
            return Outcome::Done;
        }
        let Some(role) = role_of(&self.form_value("role")) else {
            return self.revision_failed(strings::text("error-revision-role").to_owned());
        };
        if let Err(e) = self.person_gate() {
            return e;
        }
        let cmd = AnyCommand {
            name: SET_AUTHOR.to_owned(),
            params: serde_json::json!({ "revision": id.to_string(), "role": ew_api::role_name(&role) }),
        };
        match self.submit_revision_command(&cmd) {
            Ok(()) => {
                self.revision_done();
                self.notify_with(
                    strings::format("toast-revision-author", &[&label, &role_words(&role)]),
                    "edit",
                    true,
                );
                Outcome::Done
            }
            Err(e) => self.revision_failed(strings::format("error-revision-author", &[&e])),
        }
    }

    /// Submits a revision command from the shell's own origin (`Origin::Gui` only in the window a person drives; the
    /// session refuses a `HumanOnly` command from any other).
    fn submit_revision_command(&mut self, cmd: &AnyCommand) -> Result<(), String> {
        let origin = self.origin();
        let Some(project) = &mut self.project else {
            return Err(strings::text("reason-no-project").to_owned());
        };
        project
            .session
            .execute_with(cmd, Submit::from(origin))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
