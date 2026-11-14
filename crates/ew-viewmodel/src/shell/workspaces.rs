// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Named workspaces (UX/UI doc 16 section 2.5, 2.6 and 3.4): the dock layout of the
//! project view, the workspace files `workspaces/<name>.json` in the configuration directory, and the actions that
//! load, save and reset them.
//!
//! The built-in workspaces are Schematic (the page list and the page view), PLC and Fluid (doc 16 §2.6,
//! `Layout::built_in`); a built-in workspace without a file has its layout, and a saved file of its name replaces it.
//! The active workspace's file is rewritten
//! one second after the last layout change (`persist_layout_if_due`, driven by the frame's clock) and at shutdown.
//! A workspace file is only written in the configuration directory; a headless run without one writes nothing.

use std::path::PathBuf;

use serde_json::{Map, Value};

use super::{Dialog, Mode, Outcome, ShellModel, failed, string_param};
use crate::key;
use crate::layout::Layout;
use crate::settings::{BUILT_IN_WORKSPACES, valid_workspace_name};
use crate::strings;

/// How long after the last layout change the workspace file is written, in seconds (doc 16 section 3.4).
pub const PERSIST_DELAY: f64 = 1.0;

/// The dock layout of the project view and the workspace it belongs to.
pub(super) struct PaneLayoutState {
    /// The dock layout of the project view.
    pub(super) layout: Layout,
    /// The bottom-dock panes `view.bottom_dock` closed, reopened by the next open (doc 16a §5.7; view state only).
    pub(super) bottom_closed: Vec<crate::layout::PaneKind>,
    /// The name of the active workspace.
    pub(super) workspace: String,
    /// Bumped when the shell replaces the layout (a workspace was loaded or reset): the frame rebuilds its tree.
    pub(super) version: u64,
    /// The time (egui seconds) of the last layout change that is not on disk yet.
    pub(super) changed_at: Option<f64>,
}

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
        &self.panes.workspace
    }

    /// The workspaces the user can switch to: the built-in ones first, then the saved ones by name.
    pub fn workspace_names(&self) -> Vec<String> {
        let mut names: Vec<String> = BUILT_IN_WORKSPACES.iter().map(|n| (*n).to_owned()).collect();
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
                .filter(|n| valid_workspace_name(n) && !BUILT_IN_WORKSPACES.contains(&n.as_str()))
                .collect();
            saved.sort();
            names.extend(saved);
        }
        if !names.contains(&self.panes.workspace) {
            names.push(self.panes.workspace.clone());
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
            _ => return Layout::built_in(name).ok_or(ReadError::Missing),
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
        // The configuration directory, which the same call may have made.
        if let Some(config) = dir.parent() {
            crate::settings::user_only_dir(config);
        }
        crate::settings::user_only_dir(&dir);
        crate::settings::write_user_only(&dir.join(format!("{name}.json")), layout.to_json(name).as_bytes())
    }

    /// Reads the active workspace at start; a missing file is the built-in layout, a bad one is the built-in layout
    /// with a banner (the file stays as it is).
    pub(super) fn load_active_workspace(&mut self) {
        match self.read_workspace(&self.panes.workspace.clone()) {
            Ok(layout) => self.panes.layout = layout,
            Err(ReadError::Missing) => {}
            Err(ReadError::Bad(_)) => {
                self.banner = Some(strings::format(
                    key!("banner-workspace-bad"),
                    &[&self.panes.workspace],
                ));
            }
        }
    }

    pub(crate) fn act_workspace_load(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "name") {
            Err(e) => return e,
            Ok(Some(name)) => name.to_owned(),
            Ok(None) => return failed(key!("error-missing-param"), &["name"]),
        };
        if !valid_workspace_name(&name) {
            return failed(key!("error-bad-param"), &["name"]);
        }
        if self.differs_only_in_case(&name) {
            return failed(key!("error-workspace-case"), &[]);
        }
        match self.read_workspace(&name) {
            Ok(layout) => {
                self.panes.layout = layout;
                self.panes.workspace = name.clone();
                self.settings.workspace = name;
                self.save_settings();
                self.panes.version += 1;
                self.panes.changed_at = None;
                // The canvas's pane changes size with the layout: each page fits again once it has its new size.
                if let Some(area) = &mut self.area {
                    for c in area
                        .groups
                        .iter_mut()
                        .flat_map(|g| &mut g.tabs)
                        .filter_map(|t| t.canvas.as_mut())
                    {
                        c.fit_again();
                    }
                }
                Outcome::Done
            }
            Err(ReadError::Missing) => failed(key!("error-workspace-missing"), &[&name]),
            Err(ReadError::Bad(why)) => failed(key!("error-workspace-bad"), &[&name, &why]),
        }
    }

    pub(crate) fn act_workspace_save(&mut self, p: &Map<String, Value>) -> Outcome {
        let name = match string_param(p, "name") {
            Err(e) => return e,
            Ok(Some(name)) => name.to_owned(),
            Ok(None) => {
                return match self.mode {
                    Mode::Headless => failed(key!("error-missing-param"), &["name"]),
                    Mode::Gui => self.open_dialog(Dialog::SaveWorkspace {
                        text: self.panes.workspace.clone(),
                    }),
                };
            }
        };
        if !valid_workspace_name(&name) {
            self.banner = Some(strings::text(key!("error-workspace-name")).to_owned());
            return failed(key!("error-workspace-name"), &[]);
        }
        if self.differs_only_in_case(&name) {
            self.banner = Some(strings::text(key!("error-workspace-case")).to_owned());
            return failed(key!("error-workspace-case"), &[]);
        }
        self.dialog = None;
        if self.write_workspace(&name, &self.panes.layout).is_err() {
            return failed(key!("error-workspace-write"), &[]);
        }
        self.panes.workspace = name.clone();
        self.settings.workspace = name;
        self.panes.changed_at = None;
        self.save_settings();
        Outcome::Done
    }

    pub(crate) fn act_workspace_reset(&mut self) -> Outcome {
        // A built-in workspace goes back to its own layout, a saved one to Schematic's.
        self.panes.layout = Layout::built_in(&self.panes.workspace).unwrap_or_else(Layout::schematic);
        self.panes.version += 1;
        // The reset layout is the active workspace's from now on: it is written like any change.
        self.panes.changed_at = Some(f64::NEG_INFINITY);
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
        if layout != self.panes.layout {
            self.panes.layout = layout;
            self.panes.changed_at = Some(now);
            // A pane dragged out or in: the property grid fetches or drops its sheet.
            self.sync_properties();
            self.sync_find();
            // The Preview tab closed with a card pending cancels it; the history panel fetches or
            // drops its view.
            self.sync_preview();
            self.sync_history();
        }
    }

    /// Replaces the layout by the shell's own change (View ▸ Properties): the frame rebuilds its tree and the change is
    /// written like any other.
    pub(super) fn replace_layout(&mut self, layout: Layout) {
        if layout != self.panes.layout {
            self.panes.layout = layout;
            self.panes.version += 1;
            self.panes.changed_at = Some(f64::NEG_INFINITY);
        }
    }

    /// Writes the active workspace's file when the last layout change is `PERSIST_DELAY` old; `true` when it
    /// did. A failed write keeps the change pending and raises a banner once.
    pub fn persist_layout_if_due(&mut self, now: f64) -> bool {
        match self.panes.changed_at {
            Some(t) if now - t >= PERSIST_DELAY => {}
            _ => return false,
        }
        self.flush_layout()
    }

    /// Writes the active workspace's file if a change is pending.
    pub(super) fn flush_layout(&mut self) -> bool {
        if self.panes.changed_at.is_none() {
            return false;
        }
        if self
            .write_workspace(&self.panes.workspace, &self.panes.layout)
            .is_err()
        {
            self.panes.changed_at = None;
            self.banner = Some(strings::text(key!("banner-workspace-write")).to_owned());
            return false;
        }
        self.panes.changed_at = None;
        true
    }

    /// Whether a layout change waits for its file to be written.
    pub fn layout_pending(&self) -> bool {
        self.panes.changed_at.is_some()
    }
}
