// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Package… (doc 13-package §8.5): the dialog "Package for the workshop" in two panes, from the
//! session's plan of the package (`Session::package_plan`). Left, What goes in: a row per member with its tick, what it
//! holds and its form, the label files' profile with Change…, and a line for what a member lacks of part data. Right,
//! always in view: Before it goes to the workshop (the release check's line, then every finding, those that stop a
//! release first, each with Show and its fix as a button), the revision, Without personal data. Below: the file. The
//! Primary writes the package as a job (Write draft package, Write package), as Export PDF runs; a file of that name
//! asks first.

use std::path::PathBuf;

use ew_api::PackageMember;
use ew_api::checks::Severity;
use serde_json::{Map, Value, json};

use super::jobs::{absolute_target, bool_param, inside};
use super::{Dialog, Mode, Outcome, Prompt, ShellModel, failed, string_param};
use crate::jobs::JobKind;
use crate::key;
use crate::strings;

/// One member the dialog offers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PackageRow {
    /// The member.
    pub member: PackageMember,
    /// Its words ("Wire list").
    pub words: String,
    /// What it holds ("54 wires, 2 bridges"), or why it is not written ("No surface of this project has holes yet").
    pub holds: String,
    /// Whether it has content: a member without is not written, and its row has no tick.
    pub content: bool,
    /// Its form ("Excel"; the label files' profile in a line).
    pub form: String,
    /// Whether it goes in.
    pub on: bool,
    /// What it lacks of part data, each in a sentence.
    pub lacks: Vec<String>,
}

/// One finding of the release check as the dialog lists it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PackageFinding {
    /// Its fingerprint: the key of Show and of its fix.
    pub key: String,
    /// Its rule (`EW-LAY-011`).
    pub rule: &'static str,
    /// Its severity.
    pub severity: Severity,
    /// Its message in the window's language.
    pub message: String,
    /// Its records by their numbers and tags, which Show lists.
    pub targets: Vec<String>,
    /// Its fix's words ("Route"); `None` without one.
    pub fix: Option<String>,
    /// Whether Show lists its records.
    pub shown: bool,
}

/// The dialog "Package for the workshop".
#[derive(Clone, PartialEq, Debug)]
pub struct PackageDialog {
    /// The members, in the package's order.
    pub rows: Vec<PackageRow>,
    /// The check's line: "Ready: nothing stands in the way." or what stood.
    pub ready: String,
    /// Whether nothing stood.
    pub clean: bool,
    /// The release check's findings, errors first.
    pub findings: Vec<PackageFinding>,
    /// The wires without a length that no finding names, a line per reason with its count (an end made on site, a
    /// project without a panel layout), as Contents says them.
    pub lengths: Vec<String>,
    /// Whether the package is a draft (its chip, its Primary).
    pub draft: bool,
    /// The revision's line ("V1.0 is open, not released.") and what it means for the package.
    pub revision: (String, String),
    /// The line under Without personal data: what it leaves out; `Err` with why the box is greyed.
    pub personal_note: Result<String, String>,
    /// Without personal data.
    pub personal: bool,
    /// The file the package is written to.
    pub path: PathBuf,
    /// Why Write package is greyed: a release package with an error of the release check.
    pub refused: Option<String>,
    /// The file exists: the dialog asks whether to replace it.
    pub asks_replace: bool,
}

impl PackageDialog {
    /// The members left out: those unticked, and those without content.
    pub fn left_out(&self) -> Vec<&'static str> {
        self.rows
            .iter()
            .filter(|r| !r.on)
            .map(|r| r.member.key())
            .collect()
    }

    /// The parameters of `package.export` the dialog's Primary sends.
    pub fn export_params(&self, replace: bool) -> Value {
        json!({
            "leave_out": self.left_out(),
            "personal": self.personal,
            "path": self.path.to_string_lossy(),
            "replace": replace,
        })
    }

    /// The file's name.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// The member of kind `key` a parameter names.
fn member_param(p: &Map<String, Value>) -> Result<PackageMember, Outcome> {
    match string_param(p, "member")? {
        Some(k) => PackageMember::parse(k)
            .filter(|m| m.optional())
            .ok_or_else(|| failed(key!("error-bad-param"), &["member"])),
        None => Err(failed(key!("error-missing-param"), &["member"])),
    }
}

impl ShellModel {
    /// The ask of a package from the dialog's or a script's parameters: `leave_out` (the members' kinds), `personal`
    /// (without personal data), the date the project's model gives.
    fn package_ask(&self, p: &Map<String, Value>) -> Result<ew_api::PackageAsk, Outcome> {
        let personal = bool_param(p, "personal", false)?;
        let leave_out = match p.get("leave_out") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    v.as_str()
                        .and_then(PackageMember::parse)
                        .filter(|m| m.optional())
                        .ok_or_else(|| failed(key!("error-bad-param"), &["leave_out"]))
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(failed(key!("error-bad-param"), &["leave_out"])),
        };
        Ok(ew_api::PackageAsk {
            leave_out,
            omit_personal: personal,
            date: None,
            integrity: self.revision.integrity().to_vec(),
        })
    }

    /// The dialog over the plan of `ask`, its file at `path` (absent: beside the project, by the plan's name), the
    /// ticks and the Shows of `old` kept.
    fn package_dialog(
        &self,
        ask: &ew_api::PackageAsk,
        path: Option<PathBuf>,
        old: Option<&PackageDialog>,
    ) -> Option<PackageDialog> {
        let project = self.project.as_ref()?;
        let session = &project.session;
        let plan = session.package_plan(ask);
        let lang = strings::rule_language();
        let off = |m: PackageMember| old.is_some_and(|d| d.rows.iter().any(|r| r.member == m && !r.on));
        let rows = plan
            .members
            .iter()
            .map(|m| PackageRow {
                member: m.member,
                words: m.member.words(lang),
                holds: strings::render(
                    m.holds
                        .as_ref()
                        .or(m.why_not.as_ref())
                        .unwrap_or(&ew_api::Text::bare("")),
                ),
                content: m.holds.is_some(),
                form: strings::render(&m.form),
                on: m.holds.is_some() && !off(m.member),
                lacks: m.lacks.iter().map(strings::render).collect(),
            })
            .collect();
        let shown = |key: &str| old.is_some_and(|d| d.findings.iter().any(|f| f.key == key && f.shown));
        let findings: Vec<PackageFinding> = plan
            .findings()
            .into_iter()
            .map(|f| {
                let key = f.fingerprint.to_string();
                PackageFinding {
                    rule: f.rule,
                    severity: f.severity,
                    message: ew_api::checks::render(f, lang),
                    targets: f
                        .targets
                        .iter()
                        .filter_map(|t| session.record_label(t.kind, t.id.as_uuid()))
                        .collect(),
                    // The quick fix's words, else Edit part… of a finding from a part's missing datum, as Messages.
                    fix: match (&f.fix, &f.part_lack) {
                        (Some(q), _) => Some(ew_api::checks::messages::fix_label(q, lang)),
                        (None, Some(_)) => Some(ew_api::checks::messages::lack_label(lang)),
                        (None, None) => None,
                    },
                    shown: shown(&key),
                    key,
                }
            })
            .collect();
        let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
        let ready = if findings.is_empty() {
            strings::text(key!("package-ready")).to_owned()
        } else {
            let mut parts = Vec::new();
            for (s, k) in [
                (Severity::Error, crate::count_key!("package-stood-errors")),
                (Severity::Warning, crate::count_key!("package-stood-warnings")),
                (Severity::Info, crate::count_key!("package-stood-infos")),
            ] {
                if count(s) > 0 {
                    parts.push(strings::format_count(k, count(s)));
                }
            }
            strings::format_counted(
                crate::count_key!("package-stood"),
                findings.len(),
                &[&parts.join(", ")],
            )
        };
        let label = plan.revision.clone().unwrap_or_default();
        let mark = |label: &str| {
            if label.is_empty() {
                strings::text(key!("package-mark-none")).to_owned()
            } else {
                strings::format(key!("package-mark"), &[label])
            }
        };
        let revision = match (&plan.revision, &plan.released_on) {
            (Some(l), Some(on)) => (
                strings::format(key!("package-revision-released"), &[l, on]),
                strings::format(key!("package-revision-released-note"), &[l]),
            ),
            (Some(l), None) => (
                strings::format(key!("package-revision-open"), &[l]),
                strings::format(key!("package-revision-draft-note"), &[&mark(l)]),
            ),
            (None, _) => (
                strings::text(key!("package-revision-none")).to_owned(),
                strings::format(key!("package-revision-draft-note"), &[&mark("")]),
            ),
        };
        let personal_note = if plan.personal.is_empty() {
            Err(strings::text(key!("package-personal-none")).to_owned())
        } else {
            Ok(strings::format(
                key!("package-personal-note"),
                &[&strings::and_list(&plan.personal)],
            ))
        };
        let refused = plan.refused().then(|| {
            strings::format_counted(
                crate::count_key!("package-refused"),
                plan.errors().len(),
                &[&label],
            )
        });
        let path = path.unwrap_or_else(|| {
            project
                .dir
                .parent()
                .map_or_else(|| PathBuf::from(&plan.file_name), |d| d.join(&plan.file_name))
        });
        let lengths = plan
            .without_length
            .iter()
            .filter(|n| !n.in_a_finding())
            .map(|n| strings::render(&n.line()))
            .collect();
        Some(PackageDialog {
            lengths,
            rows,
            clean: findings.is_empty(),
            ready,
            findings,
            draft: plan.draft(),
            revision,
            personal_note,
            personal: ask.omit_personal,
            path,
            refused,
            asks_replace: false,
        })
    }

    /// `package.export {leave_out?, personal?, path?, replace?, wait?}` (Output ▸ Workshop ▸ Package…): in the window
    /// without parameters the dialog opens; else the package is written as a job to `path` (absent: beside the
    /// project, by its name), whole or not at all. A file of that name is replaced only with `replace` (the dialog asks
    /// first). A release package with an error of the release check is refused in words; a draft is written whatever
    /// stood.
    pub(crate) fn act_package_export(&mut self, p: &Map<String, Value>) -> Outcome {
        if self.project.is_none() {
            return Outcome::refused(key!("reason-no-project"));
        }
        let ask = match self.package_ask(p) {
            Ok(a) => a,
            Err(e) => return e,
        };
        let old = match &self.dialog {
            Some(Dialog::Package(d)) => Some((**d).clone()),
            _ => None,
        };
        if p.is_empty() && self.mode == Mode::Gui && old.is_none() {
            let Some(d) = self.package_dialog(&ask, None, None) else {
                return Outcome::refused(key!("reason-no-project"));
            };
            return self.open_dialog(Dialog::Package(Box::new(d)));
        }
        let (replace, wait) = match (bool_param(p, "replace", false), bool_param(p, "wait", true)) {
            (Ok(r), Ok(w)) => (r, w),
            (Err(e), _) | (_, Err(e)) => return e,
        };
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => Some(absolute_target(PathBuf::from(path))),
            Ok(None) => None,
        };
        let Some(d) = self.package_dialog(&ask, path, old.as_ref()) else {
            return Outcome::refused(key!("reason-no-project"));
        };
        if let Some(why) = &d.refused {
            return Outcome::Failed { message: why.clone() };
        }
        let project = or_return!(crate::shell::gated(&self.project));
        // A package renamed over a file of the open project would damage it.
        if inside(&project.dir, &d.path) {
            return Outcome::refused(key!("reason-export-into-project"));
        }
        if d.path.exists() && !replace {
            return match (&self.dialog, self.mode) {
                (Some(Dialog::Package(_)), Mode::Gui) => {
                    self.dialog = Some(Dialog::Package(Box::new(PackageDialog {
                        asks_replace: true,
                        ..d
                    })));
                    Outcome::Done
                }
                _ => Outcome::refused(key!("reason-file-exists")),
            };
        }
        if self
            .jobs
            .entries()
            .any(|e| e.is_running() && matches!(e.kind, JobKind::Package { .. }))
        {
            return Outcome::refused(key!("reason-export-running"));
        }
        let job = match project.session.package_job(&ask) {
            Ok(j) => j,
            Err(e) => {
                return Outcome::Failed {
                    message: e.to_string(),
                };
            }
        };
        let (rev, epoch) = (project.session.world().rev.0, project.epoch);
        let name = d.file_name();
        if matches!(self.dialog, Some(Dialog::Package(_))) {
            self.dialog = None;
        }
        self.count(|c| c.export_job += 1);
        let number = match self
            .jobs
            .start_package(job, d.path.clone(), name, rev, epoch, d.draft)
        {
            Ok(n) => n,
            Err(_) => {
                let message =
                    strings::format(key!("error-export"), &[strings::text(key!("error-no-thread"))]);
                self.banner = Some(message.clone());
                return Outcome::Failed { message };
            }
        };
        if self.mode == Mode::Headless && wait {
            return self.wait_job(number);
        }
        Outcome::Done
    }

    /// The open dialog, mutably; `None` when Package… is not open.
    fn package_open(&mut self) -> Option<&mut PackageDialog> {
        match &mut self.dialog {
            Some(Dialog::Package(d)) => Some(d),
            _ => None,
        }
    }

    /// `package.tick {member, on}`: ticks or unticks a member of the open dialog; a member without content stays out.
    pub(crate) fn act_package_tick(&mut self, p: &Map<String, Value>) -> Outcome {
        let member = match member_param(p) {
            Ok(m) => m,
            Err(e) => return e,
        };
        let Some(on) = p.get("on").and_then(Value::as_bool) else {
            return failed(key!("error-missing-param"), &["on"]);
        };
        let Some(d) = self.package_open() else {
            return Outcome::refused(key!("reason-package-closed"));
        };
        match d.rows.iter_mut().find(|r| r.member == member) {
            Some(r) if r.content => {
                r.on = on;
                d.asks_replace = false;
                Outcome::Done
            }
            Some(_) => Outcome::refused(key!("reason-package-empty-member")),
            None => failed(key!("error-bad-param"), &["member"]),
        }
    }

    /// `package.personal {on}`: Without personal data, greyed while no party is marked as personal data.
    pub(crate) fn act_package_personal(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(on) = p.get("on").and_then(Value::as_bool) else {
            return failed(key!("error-missing-param"), &["on"]);
        };
        let Some(d) = self.package_open() else {
            return Outcome::refused(key!("reason-package-closed"));
        };
        if let Err(why) = &d.personal_note {
            return Outcome::Failed { message: why.clone() };
        }
        d.personal = on;
        d.asks_replace = false;
        Outcome::Done
    }

    /// `package.show_finding {key, on?}`: Show (or Hide) lists the records of the finding `key` under it; without `on`
    /// it switches.
    pub(crate) fn act_package_show_finding(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["key"]),
            Err(e) => return e,
        };
        let on = p.get("on").and_then(Value::as_bool);
        let Some(d) = self.package_open() else {
            return Outcome::refused(key!("reason-package-closed"));
        };
        match d.findings.iter_mut().find(|f| f.key == key) {
            Some(f) => {
                f.shown = on.unwrap_or(!f.shown);
                Outcome::Done
            }
            None => failed(key!("error-bad-param"), &["key"]),
        }
    }

    /// `package.fix {key}`: closes the dialog and runs the quick fix of the release check's finding `key`, as Messages'
    /// fix runs it.
    pub(crate) fn act_package_fix(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["key"]),
            Err(e) => return e,
        };
        let Some(project) = &self.project else {
            return Outcome::refused(key!("reason-no-project"));
        };
        let report = project
            .session
            .package_plan(&ew_api::PackageAsk {
                integrity: self.revision.integrity().to_vec(),
                ..ew_api::PackageAsk::default()
            })
            .check;
        let Some(finding) = report
            .findings
            .into_iter()
            .find(|f| f.fingerprint.to_string() == key)
        else {
            return failed(key!("error-bad-param"), &["key"]);
        };
        if matches!(self.dialog, Some(Dialog::Package(_))) {
            self.dialog = None;
        }
        self.run_finding_fix(finding)
    }

    /// `package.messages` (the dialog's Open Messages): closes the dialog and opens the Messages pane.
    pub(crate) fn act_package_messages(&mut self) -> Outcome {
        if matches!(self.dialog, Some(Dialog::Package(_))) {
            self.dialog = None;
        }
        self.act_messages_show(&Map::from_iter([("on".to_owned(), Value::Bool(true))]))
    }

    /// `package.file {path?}` (the dialog's Choose…): the file the package is written to; without a path the window
    /// asks for one.
    pub(crate) fn act_package_file(&mut self, p: &Map<String, Value>) -> Outcome {
        let path = match string_param(p, "path") {
            Err(e) => return e,
            Ok(Some(path)) => absolute_target(PathBuf::from(path)),
            Ok(None) => return self.need_path("package.file", Prompt::SavePackage, "path", p),
        };
        let Some(d) = self.package_open() else {
            return Outcome::refused(key!("reason-package-closed"));
        };
        d.path = path;
        d.asks_replace = false;
        Outcome::Done
    }

    /// The dialog again over a fresh plan, its ticks, Shows and file kept (after Label file…'s Apply).
    pub(crate) fn package_refreshed(&self, old: &PackageDialog) -> Option<PackageDialog> {
        let ask = ew_api::PackageAsk {
            omit_personal: old.personal,
            integrity: self.revision.integrity().to_vec(),
            ..ew_api::PackageAsk::default()
        };
        self.package_dialog(&ask, Some(old.path.clone()), Some(old))
    }
}
