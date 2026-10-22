// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Named workspaces (WP-S2.01 part 2, FT-309; UX/UI doc 16 section 2.5, 2.6 and 3.4): the dock layout of the
//! project view, the workspace files `workspaces/<name>.json` in the configuration directory, and the actions that
//! load, save and reset them.
//!
//! The built-in workspace is Schematic (the page list and the page view). The active workspace's file is rewritten
//! one second after the last layout change (`persist_layout_if_due`, driven by the frame's clock) and at shutdown.
//! A workspace file is only written in the configuration directory; a headless run without one writes nothing.

use std::path::PathBuf;

use serde_json::{Map, Value};

use super::{Dialog, Mode, Outcome, ShellModel, failed, string_param};
use crate::layout::Layout;
use crate::settings::{DEFAULT_WORKSPACE, valid_workspace_name};
use crate::strings;

/// How long after the last layout change the workspace file is written, in seconds (doc 16 section 3.4).
pub const PERSIST_DELAY: f64 = 1.0;

/// Why a workspace could not be read.
enum ReadError {
    Missing,
    Bad(String),
}

impl ShellModel {
    fn workspaces_dir(&self) -> Option<PathBuf> {
        self.config.config_dir.as_ref().map(|d| d.join("workspaces"))
    }

    /// The active workspace's name.
    pub fn workspace(&self) -> &str {
        &self.workspace
    }

    /// The workspaces the user can switch to: Schematic first, then the saved ones by name.
    pub fn workspace_names(&self) -> Vec<String> {
        let mut names = vec![DEFAULT_WORKSPACE.to_owned()];
        if let Some(dir) = self.workspaces_dir()
            && let Ok(entries) = std::fs::read_dir(&dir)
        {
            let mut saved: Vec<String> = entries
                .filter_map(Result::ok)
                .filter_map(|e| {
                    let path = e.path();
                    (path.extension().is_some_and(|x| x == "json"))
                        .then(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                        .flatten()
                })
                .filter(|n| valid_workspace_name(n) && n != DEFAULT_WORKSPACE)
                .collect();
            saved.sort();
            names.extend(saved);
        }
        if !names.contains(&self.workspace) {
            names.push(self.workspace.clone());
        }
        names
    }

    /// Whether a workspace exists whose name is `name` in other letter case: on a case-insensitive file system both
    /// names would be one file.
    fn differs_only_in_case(&self, name: &str) -> bool {
        self.workspace_names()
            .iter()
            .any(|n| n != name && n.to_lowercase() == name.to_lowercase())
    }

    fn read_workspace(&self, name: &str) -> Result<Layout, ReadError> {
        let file = self.workspaces_dir().map(|d| d.join(format!("{name}.json")));
        let text = match file.as_ref().map(std::fs::read_to_string) {
            Some(Ok(text)) => text,
            _ if name == DEFAULT_WORKSPACE => return Ok(Layout::schematic()),
            _ => return Err(ReadError::Missing),
        };
        Layout::parse(&text)
            .map(|(_, layout)| layout)
            .map_err(|e| ReadError::Bad(e.to_string()))
    }

    fn write_workspace(&self, name: &str, layout: &Layout) -> std::io::Result<()> {
        let Some(dir) = self.workspaces_dir() else {
            return Ok(());
        };
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join(format!("{name}.json.tmp"));
        std::fs::write(&tmp, layout.to_json(name))?;
        std::fs::rename(&tmp, dir.join(format!("{name}.json")))
    }

    /// Reads the active workspace at start; a missing file is the built-in layout, a bad one is the built-in layout
    /// with a banner (the file stays as it is).
    pub(super) fn load_active_workspace(&mut self) {
        match self.read_workspace(&self.workspace.clone()) {
            Ok(layout) => self.layout = layout,
            Err(ReadError::Missing) => {}
            Err(ReadError::Bad(_)) => {
                self.banner = Some(strings::format("banner-workspace-bad", &[&self.workspace]));
            }
        }
    }

    pub(super) fn act_workspace_load(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "name") {
            Err(e) => return e,
            Ok(Some(name)) => name.to_owned(),
            Ok(None) => return failed("error-missing-param", &["name"]),
        };
        if !valid_workspace_name(&name) {
            return failed("error-bad-param", &["name"]);
        }
        if self.differs_only_in_case(&name) {
            return failed("error-workspace-case", &[]);
        }
        match self.read_workspace(&name) {
            Ok(layout) => {
                self.layout = layout;
                self.workspace = name.clone();
                self.settings.workspace = name;
                self.save_settings();
                self.layout_version += 1;
                self.layout_changed_at = None;
                Outcome::Done
            }
            Err(ReadError::Missing) => failed("error-workspace-missing", &[&name]),
            Err(ReadError::Bad(why)) => failed("error-workspace-bad", &[&name, &why]),
        }
    }

    pub(super) fn act_workspace_save(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "name") {
            Err(e) => return e,
            Ok(Some(name)) => name.to_owned(),
            Ok(None) => {
                return match self.mode {
                    Mode::Headless => failed("error-missing-param", &["name"]),
                    Mode::Gui => self.open_dialog(Dialog::SaveWorkspace {
                        text: self.workspace.clone(),
                    }),
                };
            }
        };
        if !valid_workspace_name(&name) {
            self.banner = Some(strings::text("error-workspace-name").to_owned());
            return failed("error-workspace-name", &[]);
        }
        if self.differs_only_in_case(&name) {
            self.banner = Some(strings::text("error-workspace-case").to_owned());
            return failed("error-workspace-case", &[]);
        }
        self.dialog = None;
        if self.write_workspace(&name, &self.layout).is_err() {
            return failed("error-workspace-write", &[]);
        }
        self.workspace = name.clone();
        self.settings.workspace = name;
        self.layout_changed_at = None;
        self.save_settings();
        Outcome::Done
    }

    pub(super) fn act_workspace_reset(&mut self) -> Outcome {
        self.layout = Layout::schematic();
        self.layout_version += 1;
        // The reset layout is the active workspace's from now on: it is written like any change.
        self.layout_changed_at = Some(f64::NEG_INFINITY);
        Outcome::Done
    }

    /// Sets the text of the save-workspace dialog's field.
    pub fn set_workspace_text(&mut self, text: &str) {
        if let Some(Dialog::SaveWorkspace { text: t }) = &mut self.dialog {
            *t = text.to_owned();
        }
    }

    /// The user changed the layout in the frame (a splitter, a tab drag): remembers it and the time `now` (seconds
    /// on the frame's clock). Does not bump [`ShellModel::layout_version`], because the frame already shows it.
    pub fn set_layout(&mut self, layout: Layout, now: f64) {
        if layout != self.layout {
            self.layout = layout;
            self.layout_changed_at = Some(now);
        }
    }

    /// Writes the active workspace's file when the last layout change is `PERSIST_DELAY` old; `true` when it
    /// did. A failed write keeps the change pending and raises a banner once.
    pub fn persist_layout_if_due(&mut self, now: f64) -> bool {
        match self.layout_changed_at {
            Some(t) if now - t >= PERSIST_DELAY => {}
            _ => return false,
        }
        self.flush_layout()
    }

    /// Writes the active workspace's file if a change is pending.
    pub(super) fn flush_layout(&mut self) -> bool {
        if self.layout_changed_at.is_none() {
            return false;
        }
        if self.write_workspace(&self.workspace, &self.layout).is_err() {
            self.layout_changed_at = None;
            self.banner = Some(strings::text("banner-workspace-write").to_owned());
            return false;
        }
        self.layout_changed_at = None;
        true
    }

    /// Whether a layout change waits for its file to be written.
    pub fn layout_pending(&self) -> bool {
        self.layout_changed_at.is_some()
    }
}
