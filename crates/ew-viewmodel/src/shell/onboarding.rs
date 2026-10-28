// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! First run, the company-standard wizard, the quick switches and F1 (WP-S2.01 part 2: FT-308, FT-311, FT-312).
//!
//! The first-run flow and the wizard are screens of their own: `ShellModel` holds their state and the actions
//! `first_run.*` and `wizard.*` change it, so the headless mode drives them like the window does. Their answers go
//! into the settings (first run) or the draft file and, at the end, the basic project (wizard). Nothing here
//! opens a connection.

use std::path::{Path, PathBuf};

use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Recovery, Session};
use serde_json::{Map, Value, json};

use super::{
    Dialog, DocRequest, Mode, Outcome, Prompt, ShellModel, dir_name, failed, flag, open_error_key,
    string_param,
};
use crate::first_run::{self, FirstRun, FirstRunStep, START_CHOICES};
use crate::help;
use crate::settings::{CANVAS_BACKENDS, KEYMAPS, LANGUAGES, ORIGINS, THEMES};
use crate::strings;
use crate::wizard::{CHECK_SCHEME_FIELD, DRAFT_FILE, Draft, Step, Wizard, run_commands};

fn bool_value(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) if s == "on" => Some(true),
        Value::String(s) if s == "off" => Some(false),
        _ => None,
    }
}

impl ShellModel {
    // -----------------------------------------------------------------------------------------------------------
    // Settings and help

    /// Sets one quick-switch setting; the reason of a failure is an [`Outcome`].
    fn apply_setting(&mut self, key: &str, value: &Value) -> Result<(), Outcome> {
        let text = value.as_str();
        let choice = |choices: &[&str]| {
            text.filter(|t| choices.contains(t))
                .map(str::to_owned)
                .ok_or_else(|| failed("error-bad-param", &["value"]))
        };
        match key {
            "language" => self.settings.language = choice(&LANGUAGES)?,
            "keymap" => self.settings.keymap = choice(&KEYMAPS)?,
            "theme" => self.settings.theme = choice(&THEMES)?,
            "canvas_backend" => self.settings.canvas_backend = choice(&CANVAS_BACKENDS)?,
            "comes_from" => {
                self.settings.comes_from = match text {
                    Some("") => String::new(),
                    _ => choice(&ORIGINS)?,
                }
            }
            "update_check" => {
                self.settings.update_check =
                    bool_value(value).ok_or_else(|| failed("error-bad-param", &["value"]))?;
            }
            "basic_project" => {
                self.settings.basic_project = match value {
                    Value::Null => None,
                    Value::String(s) if s.is_empty() => None,
                    Value::String(s) => Some(s.clone()),
                    _ => return Err(failed("error-bad-param", &["value"])),
                }
            }
            _ => return Err(failed("error-bad-param", &["key"])),
        }
        self.save_settings();
        Ok(())
    }

    pub(super) fn act_settings_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => return failed("error-missing-param", &["key"]),
        };
        let Some(value) = p.get("value") else {
            return failed("error-missing-param", &["value"]);
        };
        match self.apply_setting(&key, value) {
            Ok(()) => Outcome::Done,
            Err(e) => e,
        }
    }

    /// Asks the window to open the documentation page of `topic`.
    pub(super) fn request_doc(&mut self, topic: &str) {
        let Some(anchor) = help::anchor(topic).or_else(|| help::anchor("home")) else {
            return;
        };
        let target = help::html_target(anchor);
        let url = self
            .config
            .resources
            .docs_dir
            .as_deref()
            .filter(|d| d.is_dir())
            .map(|d| file_url(d, &target));
        self.last_help = Some(target.clone());
        self.doc_request = Some(DocRequest { target, url });
    }

    pub(super) fn act_documentation(&mut self, p: &Map<String, Value>) -> Outcome {
        let topic = match string_param(p, "topic") {
            Err(e) => return e,
            Ok(Some(t)) if help::anchor(t).is_some() => t.to_owned(),
            Ok(Some(_)) => return failed("error-bad-param", &["topic"]),
            Ok(None) => help::context_topic(self).to_owned(),
        };
        self.request_doc(&topic);
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // First run

    /// Opens the first-run flow when it has never been finished or skipped (the window calls this at start; the
    /// headless mode never does).
    pub fn start_first_run_if_needed(&mut self) {
        if !self.settings.first_run_done && self.first_run.is_none() {
            self.first_run = Some(FirstRun {
                step: FirstRunStep::Language,
            });
        }
    }

    pub(super) fn act_first_run_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let step = match string_param(p, "step") {
            Err(e) => return e,
            Ok(None) => FirstRunStep::Language,
            Ok(Some(name)) => match ["language", "origin", "privacy", "appearance", "start"]
                .iter()
                .position(|n| *n == name)
                .and_then(FirstRunStep::from_index)
            {
                Some(s) => s,
                None => return failed("error-bad-param", &["step"]),
            },
        };
        self.palette = None;
        self.dialog = None;
        self.first_run = Some(FirstRun { step });
        Outcome::Done
    }

    pub(super) fn act_first_run_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let field = match string_param(p, "field") {
            Err(e) => return e,
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed("error-missing-param", &["field"]),
        };
        let Some(value) = p.get("value") else {
            return failed("error-missing-param", &["value"]);
        };
        if field == "ui_scale" {
            return self.dispatch("view.ui_scale", &serde_json::json!({ "scale": value }));
        }
        if let Err(e) = self.apply_setting(&field, value) {
            return e;
        }
        if field == "comes_from" {
            // Step 2 sets the keymap preset (doc 16 section 9.1).
            self.settings.keymap = first_run::keymap_for_origin(&self.settings.comes_from).to_owned();
            self.save_settings();
        }
        Outcome::Done
    }

    pub(super) fn act_first_run_step(&mut self, delta: isize) -> Outcome {
        if let Some(run) = &mut self.first_run {
            let index = run.step.index().saturating_add_signed(delta);
            run.step = FirstRunStep::from_index(index).unwrap_or(run.step);
        }
        Outcome::Done
    }

    pub(super) fn act_first_run_finish(&mut self, p: &Map<String, Value>) -> Outcome {
        let start = match string_param(p, "start") {
            Err(e) => return e,
            Ok(None) => "none",
            Ok(Some(s)) if START_CHOICES.contains(&s) => s,
            Ok(Some(_)) => return failed("error-bad-param", &["start"]),
        };
        self.first_run = None;
        self.settings.first_run_done = true;
        self.save_settings();
        match first_run::start_action(start) {
            Some(action) => self.dispatch(action, &Value::Null),
            None => Outcome::Done,
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // The company-standard wizard

    fn draft_path(&self) -> Option<PathBuf> {
        self.config.config_dir.as_ref().map(|d| d.join(DRAFT_FILE))
    }

    /// Writes the draft to the configuration directory (doc 16 section 9.2: kept until the wizard finishes).
    fn persist_draft(&mut self) {
        let (Some(path), Some(w)) = (self.draft_path(), &self.wizard) else {
            return;
        };
        // A finished wizard has written its basic project and removed its draft.
        if w.finished.is_some() {
            return;
        }
        let text = w.draft.to_json(w.step);
        let failed_text = strings::text("wizard-error-draft-write").to_owned();
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, text));
        if written.is_err()
            && let Some(w) = &mut self.wizard
        {
            // The wizard says so itself; it is not the settings file that failed.
            w.error = Some(failed_text);
        }
    }

    fn sample_for_preview(&self) -> Option<PathBuf> {
        self.config.resources.sample_dir.clone().filter(|d| d.is_dir())
    }

    /// Brings the wizard's live preview up to date with its draft (opens the sample project and applies the
    /// settings commands when they changed). The frame calls it before it paints; the actions that change the
    /// draft call it too, so `ui.describe` and `view.snapshot` see the current preview.
    pub fn wizard_prepare_preview(&mut self) {
        if self.wizard.is_none() {
            return;
        }
        let sample = self.sample_for_preview();
        let version = self.config.app_version.clone();
        if let Some(w) = &mut self.wizard {
            w.ensure_preview(sample.as_deref(), &version);
        }
    }

    /// The display list of the wizard's preview page.
    ///
    /// # Errors
    ///
    /// The text of why there is none (no wizard, no sample project, a refused command).
    pub fn wizard_preview_content(&self) -> Result<ew_api::output::ExportPages, String> {
        match &self.wizard {
            Some(w) => w.preview_content(),
            None => Err(strings::text("reason-no-wizard").to_owned()),
        }
    }

    pub(super) fn act_wizard_open(&mut self) -> Outcome {
        self.palette = None;
        self.dialog = None;
        let saved = self
            .draft_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| Draft::from_json(&t));
        let (draft, step) = saved.unwrap_or_else(|| (Draft::default(), Step::Company));
        let lost = draft.logo_lost();
        let mut wizard = Wizard::new(draft, step);
        if lost {
            wizard.error = Some(strings::text("wizard-notice-logo-gone").to_owned());
        }
        self.wizard = Some(wizard);
        self.wizard_prepare_preview();
        Outcome::Done
    }

    pub(super) fn act_wizard_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let field = match string_param(p, "field") {
            Err(e) => return e,
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed("error-missing-param", &["field"]),
        };
        let value = match string_param(p, "value") {
            Err(e) => return e,
            Ok(Some(v)) => v.to_owned(),
            Ok(None) => return failed("error-missing-param", &["value"]),
        };
        let Some(w) = &mut self.wizard else {
            return Outcome::Refused {
                reason_key: "reason-no-wizard",
            };
        };
        if field == "preview" {
            match value.as_str() {
                "cover" => w.preview_cover = true,
                "page" => w.preview_cover = false,
                _ => return failed("error-bad-param", &["value"]),
            }
            return Outcome::Done;
        }
        if w.finished.is_some() {
            return Outcome::Refused {
                reason_key: "reason-wizard-finished",
            };
        }
        match w.draft.set(&field, &value) {
            Ok(()) => {
                w.error = None;
                self.persist_draft();
                self.wizard_prepare_preview();
                Outcome::Done
            }
            Err(key) => {
                let message = strings::text(key).to_owned();
                w.error = Some(message.clone());
                Outcome::Failed { message }
            }
        }
    }

    fn set_wizard_step(&mut self, step: Step) {
        if let Some(w) = &mut self.wizard {
            w.step = step;
            w.error = None;
        }
        self.persist_draft();
    }

    pub(super) fn act_wizard_step(&mut self, delta: isize) -> Outcome {
        if let Some(w) = &self.wizard {
            let index = w.step.index().saturating_add_signed(delta);
            let step = Step::from_index(index).unwrap_or(w.step);
            self.set_wizard_step(step);
        }
        Outcome::Done
    }

    pub(super) fn act_wizard_goto(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(index) = p.get("step").and_then(Value::as_u64) else {
            return failed("error-missing-param", &["step"]);
        };
        match usize::try_from(index).ok().and_then(Step::from_index) {
            Some(step) => {
                self.set_wizard_step(step);
                Outcome::Done
            }
            None => failed("error-bad-param", &["step"]),
        }
    }

    pub(super) fn act_wizard_reset(&mut self) -> Outcome {
        if let Some(w) = &mut self.wizard {
            *w = Wizard::new(Draft::default(), Step::Company);
        }
        if let Some(path) = self.draft_path() {
            let _ = std::fs::remove_file(path);
        }
        self.wizard_prepare_preview();
        Outcome::Done
    }

    /// Closes the wizard; the draft stays for the next time.
    pub(super) fn act_wizard_cancel(&mut self) -> Outcome {
        self.persist_draft();
        self.wizard = None;
        Outcome::Done
    }

    pub(super) fn act_wizard_finish(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(draft) = self.wizard.as_ref().map(|w| w.draft.clone()) else {
            return Outcome::Refused {
                reason_key: "reason-no-wizard",
            };
        };
        if self.wizard.as_ref().is_some_and(|w| w.finished.is_some()) {
            return Outcome::Refused {
                reason_key: "reason-wizard-finished",
            };
        }
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("wizard.finish", Prompt::NewFolder, "dir", p),
        };
        if let Some(outcome) = self.require_free_dir(&dir) {
            if let Some(w) = &mut self.wizard {
                w.error = self.banner.clone();
            }
            return outcome;
        }
        let name = dir_name(&dir);
        match self.write_basic_project(&draft, &dir) {
            Ok(()) => {
                self.settings.basic_project = Some(dir.to_string_lossy().into_owned());
                self.save_settings();
                if let Some(path) = self.draft_path() {
                    let _ = std::fs::remove_file(path);
                }
                // The wizard stays on the review step: the check scheme is applied after finishing, by a person
                // (`wizard.apply_check_scheme`, FT-336).
                if let Some(w) = &mut self.wizard {
                    w.finished = Some(dir.clone());
                    w.step = Step::Review;
                    w.error = None;
                }
                self.banner = Some(strings::format("banner-wizard-done", &[&name]));
                Outcome::Done
            }
            Err(message) => {
                if let Some(w) = &mut self.wizard {
                    w.error = Some(message.clone());
                }
                Outcome::Failed { message }
            }
        }
    }

    /// Applies the draft's check scheme to the finished basic project (FT-336): `project.checks.set` is `HumanOnly`
    /// (ADR-0008 amendment 3, it decides check outcomes), so only a person in the window applies it, after a
    /// confirmation that names the scheme. The headless mode refuses it (the human-only reason in the transcript).
    pub(super) fn act_wizard_apply_check_scheme(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some((dir, scheme)) = self.wizard.as_ref().and_then(|w| {
            let dir = w.finished.clone()?;
            Some((dir, w.draft.get(CHECK_SCHEME_FIELD).to_owned()))
        }) else {
            return Outcome::Refused {
                reason_key: "reason-wizard-not-finished",
            };
        };
        if !self.wizard.as_ref().is_some_and(Wizard::can_apply_check_scheme) {
            return Outcome::Refused {
                reason_key: "reason-check-scheme-written",
            };
        }
        if self.mode == Mode::Headless {
            return Outcome::Refused {
                reason_key: "reason-human-only",
            };
        }
        if !flag(p, "confirm") {
            return self.open_dialog(Dialog::ConfirmCheckScheme { scheme });
        }
        self.dialog = None;
        let name = dir_name(&dir);
        let written = self
            .open_for_editing_with(&dir, Recovery::Keep)
            .map_err(|e| strings::format("error-wizard-write", &[strings::text(open_error_key(&e))]))
            .and_then(|(mut session, _report)| {
                let cmd = AnyCommand {
                    name: "project.checks.set".to_owned(),
                    params: json!({ "release_scheme": scheme }),
                };
                let result = session
                    .execute(&cmd)
                    .map_err(|e| {
                        strings::format("wizard-error-command", &["project.checks.set", &e.to_string()])
                    })
                    .and_then(|_| {
                        session
                            .save()
                            .map(|_| ())
                            .map_err(|_| strings::format("error-wizard-write", &[&name]))
                    });
                session.close();
                result
            });
        match written {
            Ok(()) => {
                if let Some(w) = &mut self.wizard {
                    w.checks_written = true;
                    w.error = None;
                }
                self.banner = Some(strings::format("banner-check-scheme-applied", &[&scheme, &name]));
                Outcome::Done
            }
            Err(message) => {
                if let Some(w) = &mut self.wizard {
                    w.error = Some(message.clone());
                }
                Outcome::Failed { message }
            }
        }
    }

    /// Builds the basic project of `draft` from the built-in one through model commands and writes it to `dir`.
    fn write_basic_project(&self, draft: &Draft, dir: &Path) -> Result<(), String> {
        let commands = draft.commands().map_err(|key| strings::text(key).to_owned())?;
        let mut session = Session::new_project(
            ProjectTemplate::Builtin,
            NewProjectKind::Basic,
            self.session_options(),
        )
        .map_err(|e| strings::format("error-wizard-write", &[strings::text(open_error_key(&e))]))?;
        run_commands(&mut session, "wizard", &commands)?;
        session
            .create(dir, self.store_options())
            .map_err(|e| strings::format("error-wizard-write", &[strings::text(open_error_key(&e))]))?;
        session.close();
        Ok(())
    }
}

/// Percent-encodes every byte of `path` that is not unreserved (`A-Z a-z 0-9 - . _ ~`) or a path separator, a
/// Windows drive's colon after one letter excepted, so that nothing in a directory name can end the URL or be read as
/// part of a command line (`&`, `^`, `%`, quotes, spaces, `#`, `?`, non-ASCII).
pub(super) fn encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for (i, byte) in path.bytes().enumerate() {
        let drive_colon = byte == b':'
            && i == 1
            && path.as_bytes()[0].is_ascii_alphabetic()
            && matches!(path.as_bytes().get(2), None | Some(b'/'));
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') || drive_colon {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The `file://` URL of `target` (a page and heading of the book, ours and safe) below the built book `dir`.
fn file_url(dir: &Path, target: &str) -> String {
    let path = dir.to_string_lossy().replace('\\', "/");
    // A verbatim Windows path (`\\?\C:\...`, what `canonicalize` gives) is the plain one.
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    let path = encode_path(path.trim_end_matches('/'));
    if path.starts_with('/') {
        format!("file://{path}/{target}")
    } else {
        format!("file:///{path}/{target}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_are_made_for_unix_and_windows_paths() {
        assert_eq!(
            file_url(Path::new("/opt/ew/docs"), "a/b.html#c"),
            "file:///opt/ew/docs/a/b.html#c"
        );
        assert_eq!(
            file_url(Path::new("C:\\Program Files\\EW\\docs"), "x.html"),
            "file:///C:/Program%20Files/EW/docs/x.html"
        );
        assert_eq!(
            file_url(Path::new("\\\\?\\C:\\EW\\docs"), "x.html"),
            "file:///C:/EW/docs/x.html",
            "a verbatim path is the plain one"
        );
    }

    #[test]
    fn nothing_in_a_directory_name_can_leave_the_url() {
        let url = file_url(
            Path::new("/tmp/a&b^c%d \"e\" #f?g'h;i|j<k>l$m`n/\u{e9}"),
            "p.html#x",
        );
        assert_eq!(
            url,
            "file:///tmp/a%26b%5Ec%25d%20%22e%22%20%23f%3Fg%27h%3Bi%7Cj%3Ck%3El%24m%60n/%C3%A9/p.html#x"
        );
        // Only the unreserved characters, the separators and the target's own `#` are left.
        let (path, fragment) = url.split_once("p.html#").unwrap();
        assert!(
            path.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~/%:".contains(&b)),
            "{url}"
        );
        assert_eq!(fragment, "x");
        assert_eq!(encode_path("C:/x y"), "C:/x%20y");
        assert_eq!(encode_path("a:b"), "a%3Ab", "a colon is kept only as a drive's");
    }
}
