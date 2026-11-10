// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! First run, the company-standard wizard, the quick switches and F1.
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
use crate::key;
use crate::settings::{CANVAS_BACKENDS, DENSITIES, KEYMAPS, ORIGINS, THEMES};
use crate::strings;
use crate::wizard::{CHECK_SCHEME_FIELD, DRAFT_FILE, Draft, Step, Wizard, run_commands};

/// The first-run flow, the start-up offer and the company-standard wizard of the shell.
#[derive(Default)]
pub(super) struct OnboardingState {
    /// The first-run flow, while it is open.
    pub(super) first_run: Option<FirstRun>,
    /// The start-up offer: leftover journals and the crash log of an earlier session.
    pub(super) startup: Option<super::startup::StartupOffer>,
    /// The log folder `startup.open_log_folder` asks the window to open: its `file://` URL and the folder.
    pub(super) folder_request: Option<(String, PathBuf)>,
    /// The company-standard wizard, while it is open.
    pub(super) wizard: Option<Wizard>,
}

/// The command bar's tab and the collapse state of the command bar and the rail.
pub(super) struct ChromeState {
    /// The command bar's active tab (`view.command_tab`); a session state, Draw at start.
    pub(super) tab: crate::command_bar::Tab,
    /// Whether the command bar shows the tab row only (`view.command_bar`, Ctrl+F1).
    pub(super) bar_collapsed: bool,
    /// Whether the left column is collapsed to its rail (`rail.show` on the shown entry); a session state.
    pub(super) rail_collapsed: bool,
}

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
        // A value that is not one of the choices names them (doc 16a §8 item 5).
        let choice = |choices: &[&str]| {
            text.filter(|t| choices.contains(t))
                .map(str::to_owned)
                .ok_or_else(|| {
                    let given = text.map_or_else(|| value.to_string(), str::to_owned);
                    failed(key!("error-not-one-of"), &[&given, &choices.join(", ")])
                })
        };
        match key {
            // The languages View ▸ Language offers; the texts follow at once.
            "language" => {
                let tags: Vec<&str> = strings::languages().into_iter().map(|(tag, _)| tag).collect();
                let language = choice(&tags)?;
                // A toast keeps the words of its language, so a switch takes the toasts on show off the
                // screen (the log keeps them).
                if language != self.settings.language {
                    self.notes.clear_visible();
                }
                self.settings.language = language;
                strings::set_locale(&self.settings.language);
                self.refetch_for_language();
                // The Place tool's label in the new language at once, not at the pointer's next move.
                let relabelled = self.with_tool(|t, cx| {
                    t.as_any_mut()
                        .and_then(|a| a.downcast_mut::<crate::tools::PlaceTool>())
                        .and_then(|p| p.relabel(cx))
                });
                if let Some(Some(crate::tools::ToolOutput::Transient(o))) = relabelled {
                    self.tools.overlay = o;
                }
            }
            "keymap" => self.settings.keymap = choice(&KEYMAPS)?,
            "theme" => self.settings.theme = choice(&THEMES)?,
            "density" => self.settings.density = choice(&DENSITIES)?,
            "canvas_backend" => self.settings.canvas_backend = choice(&CANVAS_BACKENDS)?,
            "comes_from" => {
                self.settings.comes_from = match text {
                    Some("") => String::new(),
                    _ => choice(&ORIGINS)?,
                }
            }
            "update_check" => {
                self.settings.update_check =
                    bool_value(value).ok_or_else(|| failed(key!("error-bad-param"), &["value"]))?;
            }
            "basic_project" => {
                self.settings.basic_project = match value {
                    Value::Null => None,
                    Value::String(s) if s.is_empty() => None,
                    Value::String(s) => Some(s.clone()),
                    _ => return Err(failed(key!("error-bad-param"), &["value"])),
                }
            }
            _ => return Err(failed(key!("error-bad-param"), &["key"])),
        }
        self.save_settings();
        Ok(())
    }

    /// The command bar's active tab.
    pub fn command_tab(&self) -> crate::command_bar::Tab {
        self.chrome.tab
    }

    /// Whether the command bar is collapsed to its tab row.
    pub fn command_bar_collapsed(&self) -> bool {
        self.chrome.bar_collapsed
    }

    /// `view.command_tab { tab }`: shows the tab's groups in the command bar; an unknown tab is refused with
    /// `error-bad-param`.
    pub(crate) fn act_command_tab(&mut self, p: &Map<String, Value>) -> Outcome {
        match string_param(p, "tab") {
            Err(e) => e,
            Ok(None) => failed(key!("error-missing-param"), &["tab"]),
            Ok(Some(word)) => match crate::command_bar::Tab::from_word(word) {
                Some(tab) => {
                    self.chrome.tab = tab;
                    Outcome::Done
                }
                None => failed(key!("error-bad-param"), &["tab"]),
            },
        }
    }

    /// `view.command_bar { collapsed? }`: collapses the command bar to its tab row or restores it; without the
    /// parameter it toggles (Ctrl+F1, a double-click on a tab).
    pub(crate) fn act_command_bar(&mut self, p: &Map<String, Value>) -> Outcome {
        self.chrome.bar_collapsed = match p.get("collapsed") {
            None => !self.chrome.bar_collapsed,
            Some(Value::Bool(b)) => *b,
            Some(_) => return failed(key!("error-bad-param"), &["collapsed"]),
        };
        Outcome::Done
    }

    /// `view.theme { value }` and `view.density { value }`: the setting `key` from the `value`
    /// parameter; an unknown value is refused with `error-bad-param`.
    pub(crate) fn act_view_setting(&mut self, key: &str, p: &Map<String, Value>) -> Outcome {
        let Some(value) = p.get("value") else {
            return failed(key!("error-missing-param"), &["value"]);
        };
        match self.apply_setting(key, value) {
            Ok(()) => Outcome::Done,
            Err(e) => e,
        }
    }

    pub(crate) fn act_settings_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let key = match string_param(p, "key") {
            Err(e) => return e,
            Ok(Some(k)) => k.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["key"]),
        };
        let Some(value) = p.get("value") else {
            return failed(key!("error-missing-param"), &["value"]);
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
        self.request_doc_target(help::html_target(anchor));
    }

    /// Asks the window to open `target`, a page of the built book (`how-to/x.html#fragment`): in a Dutch window the
    /// Dutch book's page (`<docs_dir>/nl/<target>`) when the book has it, else the English one.
    pub(super) fn request_doc_target(&mut self, target: String) {
        let (page, url) = self
            .config
            .resources
            .docs_dir
            .as_deref()
            .filter(|d| d.is_dir())
            .map(|d| {
                let page = target.split_once('#').map_or(target.as_str(), |(page, _)| page);
                let nl = d.join("nl");
                if strings::locale() == "nl" && nl.join(page).is_file() {
                    (format!("nl/{page}"), file_url(&nl, &target))
                } else {
                    (page.to_owned(), file_url(d, &target))
                }
            })
            .unzip();
        self.last_help = Some(target.clone());
        self.doc_request = Some(DocRequest { target, url, page });
    }

    pub(crate) fn act_documentation(&mut self, p: &Map<String, Value>) -> Outcome {
        let topic = match string_param(p, "topic") {
            Err(e) => return e,
            Ok(Some(t)) if help::anchor(t).is_some() => t.to_owned(),
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["topic"]),
            // F1 on a finding or rule row of the Messages pane opens the rule's page (§7.3).
            Ok(None) => match self.messages_help_target() {
                Some(target) => {
                    self.request_doc_target(target);
                    return Outcome::Done;
                }
                None => help::context_topic(self).to_owned(),
            },
        };
        self.request_doc(&topic);
        Outcome::Done
    }

    // -----------------------------------------------------------------------------------------------------------
    // First run

    /// Opens the first-run flow when it has never been finished or skipped (the window calls this at start; the
    /// headless mode never does).
    pub fn start_first_run_if_needed(&mut self) {
        if !self.settings.first_run_done && self.onboarding.first_run.is_none() {
            self.onboarding.first_run = Some(FirstRun {
                step: FirstRunStep::Language,
            });
        }
    }

    pub(crate) fn act_first_run_open(&mut self, p: &Map<String, Value>) -> Outcome {
        let step = match string_param(p, "step") {
            Err(e) => return e,
            Ok(None) => FirstRunStep::Language,
            Ok(Some(name)) => match ["language", "origin", "privacy", "appearance", "start"]
                .iter()
                .position(|n| *n == name)
                .and_then(FirstRunStep::from_index)
            {
                Some(s) => s,
                None => return failed(key!("error-bad-param"), &["step"]),
            },
        };
        self.palette = None;
        self.dialog = None;
        self.onboarding.first_run = Some(FirstRun { step });
        Outcome::Done
    }

    pub(crate) fn act_first_run_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let field = match string_param(p, "field") {
            Err(e) => return e,
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["field"]),
        };
        let Some(value) = p.get("value") else {
            return failed(key!("error-missing-param"), &["value"]);
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

    pub(crate) fn act_first_run_step(&mut self, delta: isize) -> Outcome {
        if let Some(run) = &mut self.onboarding.first_run {
            let index = run.step.index().saturating_add_signed(delta);
            run.step = FirstRunStep::from_index(index).unwrap_or(run.step);
        }
        Outcome::Done
    }

    pub(crate) fn act_first_run_finish(&mut self, p: &Map<String, Value>) -> Outcome {
        let start = match string_param(p, "start") {
            Err(e) => return e,
            Ok(None) => "none",
            Ok(Some(s)) if START_CHOICES.contains(&s) => s,
            Ok(Some(_)) => return failed(key!("error-bad-param"), &["start"]),
        };
        self.onboarding.first_run = None;
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
        let (Some(path), Some(w)) = (self.draft_path(), &self.onboarding.wizard) else {
            return;
        };
        // A finished wizard has written its basic project and removed its draft.
        if w.finished.is_some() {
            return;
        }
        let text = w.draft.to_json(w.step);
        let failed_text = strings::text(key!("wizard-error-draft-write")).to_owned();
        let written = path
            .parent()
            .map_or(Ok(()), |dir| {
                std::fs::create_dir_all(dir)?;
                crate::settings::user_only_dir(dir);
                Ok(())
            })
            .and_then(|()| crate::settings::write_user_only(&path, text.as_bytes()));
        if written.is_err()
            && let Some(w) = &mut self.onboarding.wizard
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
        if self.onboarding.wizard.is_none() {
            return;
        }
        let sample = self.sample_for_preview();
        let version = self.config.app_version.clone();
        if let Some(w) = &mut self.onboarding.wizard {
            w.ensure_preview(sample.as_deref(), &version);
        }
    }

    /// The display list of the wizard's preview page.
    ///
    /// # Errors
    ///
    /// The text of why there is none (no wizard, no sample project, a refused command).
    pub fn wizard_preview_content(&self) -> Result<ew_api::output::ExportPages, String> {
        match &self.onboarding.wizard {
            Some(w) => w.preview_content(),
            None => Err(strings::text(key!("reason-no-wizard")).to_owned()),
        }
    }

    pub(crate) fn act_wizard_open(&mut self) -> Outcome {
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
            wizard.error = Some(strings::text(key!("wizard-notice-logo-gone")).to_owned());
        }
        self.onboarding.wizard = Some(wizard);
        self.wizard_prepare_preview();
        Outcome::Done
    }

    pub(crate) fn act_wizard_set(&mut self, p: &Map<String, Value>) -> Outcome {
        let field = match string_param(p, "field") {
            Err(e) => return e,
            Ok(Some(f)) => f.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["field"]),
        };
        let value = match string_param(p, "value") {
            Err(e) => return e,
            Ok(Some(v)) => v.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["value"]),
        };
        let Some(w) = &mut self.onboarding.wizard else {
            return Outcome::refused(key!("reason-no-wizard"));
        };
        if field == "preview" {
            match value.as_str() {
                "cover" => w.preview_cover = true,
                "page" => w.preview_cover = false,
                _ => return failed(key!("error-bad-param"), &["value"]),
            }
            return Outcome::Done;
        }
        if w.finished.is_some() {
            return Outcome::refused(key!("reason-wizard-finished"));
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
        if let Some(w) = &mut self.onboarding.wizard {
            w.step = step;
            w.error = None;
        }
        self.persist_draft();
    }

    pub(crate) fn act_wizard_step(&mut self, delta: isize) -> Outcome {
        if let Some(w) = &self.onboarding.wizard {
            let index = w.step.index().saturating_add_signed(delta);
            let step = Step::from_index(index).unwrap_or(w.step);
            self.set_wizard_step(step);
        }
        Outcome::Done
    }

    pub(crate) fn act_wizard_goto(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(index) = p.get("step").and_then(Value::as_u64) else {
            return failed(key!("error-missing-param"), &["step"]);
        };
        match usize::try_from(index).ok().and_then(Step::from_index) {
            Some(step) => {
                self.set_wizard_step(step);
                Outcome::Done
            }
            None => failed(key!("error-bad-param"), &["step"]),
        }
    }

    pub(crate) fn act_wizard_reset(&mut self) -> Outcome {
        if let Some(w) = &mut self.onboarding.wizard {
            *w = Wizard::new(Draft::default(), Step::Company);
        }
        if let Some(path) = self.draft_path() {
            let _ = std::fs::remove_file(path);
        }
        self.wizard_prepare_preview();
        Outcome::Done
    }

    /// Closes the wizard; the draft stays for the next time.
    pub(crate) fn act_wizard_cancel(&mut self) -> Outcome {
        self.persist_draft();
        self.onboarding.wizard = None;
        Outcome::Done
    }

    pub(crate) fn act_wizard_finish(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some(draft) = self.onboarding.wizard.as_ref().map(|w| w.draft.clone()) else {
            return Outcome::refused(key!("reason-no-wizard"));
        };
        if self
            .onboarding
            .wizard
            .as_ref()
            .is_some_and(|w| w.finished.is_some())
        {
            return Outcome::refused(key!("reason-wizard-finished"));
        }
        let dir = match string_param(p, "dir") {
            Err(e) => return e,
            Ok(Some(dir)) => PathBuf::from(dir),
            Ok(None) => return self.need_path("wizard.finish", Prompt::NewFolder, "dir", p),
        };
        if let Some(outcome) = self.require_free_dir(&dir) {
            if let Some(w) = &mut self.onboarding.wizard {
                w.error = self.banner.clone();
            }
            return outcome;
        }
        let name = dir_name(&dir);
        match self.write_basic_project(&draft, &dir) {
            Ok(unstable) => {
                self.warn_unreleased(unstable);
                self.settings.basic_project = Some(dir.to_string_lossy().into_owned());
                self.save_settings();
                if let Some(path) = self.draft_path() {
                    let _ = std::fs::remove_file(path);
                }
                // The wizard stays on the review step: the check scheme is applied after finishing, by a person
                // (`wizard.apply_check_scheme`).
                if let Some(w) = &mut self.onboarding.wizard {
                    w.finished = Some(dir.clone());
                    w.step = Step::Review;
                    w.error = None;
                }
                self.banner = Some(strings::format(key!("banner-wizard-done"), &[&name]));
                Outcome::Done
            }
            Err(message) => {
                if let Some(w) = &mut self.onboarding.wizard {
                    w.error = Some(message.clone());
                }
                Outcome::Failed { message }
            }
        }
    }

    /// Applies the draft's check scheme to the finished basic project: `project.checks.set` is `HumanOnly`
    /// (ADR-0008 amendment 3, it decides check outcomes), so only a person in the window applies it, after a
    /// confirmation that names the scheme. The headless mode refuses it (the human-only reason in the transcript).
    pub(crate) fn act_wizard_apply_check_scheme(&mut self, p: &Map<String, Value>) -> Outcome {
        let Some((dir, scheme)) = self.onboarding.wizard.as_ref().and_then(|w| {
            let dir = w.finished.clone()?;
            Some((dir, w.draft.get(CHECK_SCHEME_FIELD).to_owned()))
        }) else {
            return Outcome::refused(key!("reason-wizard-not-finished"));
        };
        if !self
            .onboarding
            .wizard
            .as_ref()
            .is_some_and(Wizard::can_apply_check_scheme)
        {
            return Outcome::refused(key!("reason-check-scheme-written"));
        }
        if self.mode == Mode::Headless {
            return Outcome::refused(key!("reason-human-only"));
        }
        if !flag(p, "confirm") {
            return self.open_dialog(Dialog::ConfirmCheckScheme { scheme });
        }
        self.dialog = None;
        let name = dir_name(&dir);
        let written = self
            .open_for_editing_with(&dir, Recovery::Keep)
            .map_err(|e| strings::format(key!("error-wizard-write"), &[strings::text(open_error_key(&e))]))
            .and_then(|(mut session, _report)| {
                let cmd = AnyCommand {
                    name: "project.checks.set".to_owned(),
                    params: json!({ "release_scheme": scheme }),
                };
                let result = session
                    .execute(&cmd)
                    .map_err(|e| {
                        strings::format(
                            key!("wizard-error-command"),
                            // In the window's language.
                            &["project.checks.set", &super::tools::command_error_words(&e)],
                        )
                    })
                    .and_then(|_| {
                        session
                            .save()
                            .map(|_| ())
                            .map_err(|_| strings::format(key!("error-wizard-write"), &[&name]))
                    });
                session.close();
                result
            });
        match written {
            Ok(()) => {
                if let Some(w) = &mut self.onboarding.wizard {
                    w.checks_written = true;
                    w.error = None;
                }
                self.banner = Some(strings::format(
                    key!("banner-check-scheme-applied"),
                    // ID-1389 (W51 finding D): the scheme's word, as the question names it.
                    &[
                        strings::text(strings::Key::run_time(&format!("choice-check_scheme-{scheme}"))),
                        &name,
                    ],
                ));
                Outcome::Done
            }
            Err(message) => {
                if let Some(w) = &mut self.onboarding.wizard {
                    w.error = Some(message.clone());
                }
                Outcome::Failed { message }
            }
        }
    }

    /// Builds the basic project of `draft` from the built-in one through model commands and writes it to `dir`; whether
    /// the manifest written is marked `unstable`.
    fn write_basic_project(&self, draft: &Draft, dir: &Path) -> Result<bool, String> {
        let commands = draft.commands().map_err(|key| strings::text(key).to_owned())?;
        let mut session = Session::new_project(
            ProjectTemplate::Builtin,
            NewProjectKind::Basic,
            self.session_options(),
        )
        .map_err(|e| strings::format(key!("error-wizard-write"), &[strings::text(open_error_key(&e))]))?;
        run_commands(&mut session, "wizard", &commands)?;
        session
            .create(dir, self.store_options())
            .map_err(|e| strings::format(key!("error-wizard-write"), &[strings::text(open_error_key(&e))]))?;
        let unstable = session.store().is_some_and(|s| s.writes_unstable());
        session.close();
        Ok(unstable)
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
