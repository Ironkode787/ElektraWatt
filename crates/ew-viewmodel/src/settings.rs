// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application settings file `settings.toml` (WP-S2.01 part 1, spec section 5.1; FR-UI-033, FR-UI-049).
//!
//! `version = 1`, `recent_projects` (at most [`MAX_RECENT`], most recent first, no duplicates), `ui_scale` (one
//! of [`UI_SCALES`]) and, since part 2, the first-run answers and quick switches: `first_run_done`, `language`,
//! `keymap`, `theme`, `density` (WP-S2.17), `comes_from`, `update_check` (an opt-in, off by default, NFR-PRIV-03), the default basic
//! project `basic_project` (FR-PRJ-009), the active workspace `workspace` (FR-UI-002) and, since WP-S2.02 part 2,
//! the canvas renderer `canvas_backend` (`auto`, or `cpu` for View, Use the CPU renderer) and, since cycle 37, the
//! author name `author_name` of the Waive prompt (FT-674; written only once set) and, since WP-S2.07 part 4a (D18), the
//! part dialog's `[parts] favourites` (sorted) and `recent` (most recent first, at most [`MAX_RECENT_PARTS`]), each a
//! list of `<library ID>/<item ID>` texts, written only once set. Unknown keys are kept
//! and written back; a `toml::Table` holds them sorted, so the file is deterministic. An unreadable file yields
//! the defaults and is copied to `settings.toml.bad-<n>`.

use std::path::{Path, PathBuf};

/// The recent-projects list holds at most this many paths.
pub const MAX_RECENT: usize = 10;

/// The recently used parts list holds at most this many library items (WP-S2.07 D18).
pub const MAX_RECENT_PARTS: usize = 20;

/// The UI scales the user can choose, as factors on the native scale.
pub const UI_SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// The file name inside the settings directory.
pub const FILE_NAME: &str = "settings.toml";

/// The UI languages of this version (WP-S2.08 adds the others).
pub const LANGUAGES: [&str; 1] = ["en"];

/// The keymap presets the settings can name (WP-S2.08 part 2, D6): EW and KiCad-like (`keymap::Preset`); the user's
/// `keymap.toml` beside `settings.toml` replaces bindings of either.
pub const KEYMAPS: [&str; 2] = ["ew", "kicad"];

/// The themes of the design language (doc 16a §4, WP-S2.17 part 1): follow the system (light or dark), light, dark and
/// the two high-contrast themes.
pub const THEMES: [&str; 5] = ["system", "light", "dark", "hc-dark", "hc-light"];

/// The densities (doc 16a §4.13, WP-S2.17 D6): Comfortable (the default) and Compact.
pub const DENSITIES: [&str; 2] = ["comfortable", "compact"];

/// What the user comes from (first-run step 2, doc 16 section 9.1); the empty text is "not answered".
pub const ORIGINS: [&str; 4] = ["eplan", "qet", "kicad", "other"];

/// The values of `canvas_backend`: the switch rule decides (`auto`, the default), or always the CPU canvas (`cpu`;
/// WP-S2.02 §7.2 row 2).
pub const CANVAS_BACKENDS: [&str; 2] = ["auto", "cpu"];

/// The name of the built-in workspace (doc 16 section 2.6) and the default of `workspace`.
pub const DEFAULT_WORKSPACE: &str = "Schematic";

/// The settings.
#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    /// The recent projects, most recent first (paths as text; a path that no longer exists stays until it is
    /// opened and fails).
    pub recent_projects: Vec<String>,
    /// The UI scale, one of [`UI_SCALES`].
    pub ui_scale: f32,
    /// Whether the first-run flow has been finished or skipped (doc 16 section 9.1).
    pub first_run_done: bool,
    /// The UI language, one of [`LANGUAGES`].
    pub language: String,
    /// The keymap preset, one of [`KEYMAPS`].
    pub keymap: String,
    /// The theme, one of [`THEMES`].
    pub theme: String,
    /// The density, one of [`DENSITIES`] (WP-S2.17 D6).
    pub density: String,
    /// What the user comes from, one of [`ORIGINS`], or empty.
    pub comes_from: String,
    /// The update-check opt-in (off by default; this version has no update check, NFR-PRIV-03).
    pub update_check: bool,
    /// The default basic project (a project directory), if the user made or chose one (FR-PRJ-009).
    pub basic_project: Option<String>,
    /// The active workspace (doc 16 section 2.6).
    pub workspace: String,
    /// The canvas renderer, one of [`CANVAS_BACKENDS`].
    pub canvas_backend: String,
    /// The property grid's collapsed categories, `<kind>/<category>` (`device/identity`), sorted (`[properties]
    /// collapsed`, WP-S2.05 §6.1.2).
    pub properties_collapsed: Vec<String>,
    /// Whether the property grid shows the Advanced rows (`[properties] show_advanced`, WP-S2.17 D11).
    pub properties_show_advanced: bool,
    /// The author name (FT-674): the display name a waiver records as its author, prefilled in the Waive prompt and
    /// saved back when a waiver is committed; `None` until one is typed (`author_name`, written only when set).
    pub author_name: Option<String>,
    /// The main window's last size, position and maximised state (`[window]`, WP-S2.17 part 3 item 9, FT-348); `None`
    /// until a window has closed.
    pub window: Option<crate::window::WindowGeometry>,
    /// The favourite parts, `<library ID>/<item ID>`, sorted (`[parts] favourites`, WP-S2.07 D18).
    pub parts_favourites: Vec<String>,
    /// The recently used parts, `<library ID>/<item ID>`, most recent first, at most [`MAX_RECENT_PARTS`] (`[parts]
    /// recent`).
    pub parts_recent: Vec<String>,
    /// Keys this version does not know, kept and written back.
    extra: toml::Table,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            recent_projects: Vec::new(),
            ui_scale: 1.0,
            first_run_done: false,
            language: LANGUAGES[0].to_owned(),
            keymap: KEYMAPS[0].to_owned(),
            theme: THEMES[0].to_owned(),
            density: DENSITIES[0].to_owned(),
            comes_from: String::new(),
            update_check: false,
            basic_project: None,
            workspace: DEFAULT_WORKSPACE.to_owned(),
            canvas_backend: CANVAS_BACKENDS[0].to_owned(),
            properties_collapsed: Vec::new(),
            properties_show_advanced: false,
            author_name: None,
            window: None,
            parts_favourites: Vec::new(),
            parts_recent: Vec::new(),
            extra: toml::Table::new(),
        }
    }
}

/// What reading the settings found.
#[derive(Clone, PartialEq, Debug)]
pub struct Loaded {
    /// The settings (defaults when the file is absent or bad).
    pub settings: Settings,
    /// The name the unreadable file was copied to (`settings.toml.bad-1`), when there was one.
    pub bad_copy: Option<String>,
}

/// The index of `scale` in [`UI_SCALES`].
pub fn scale_index(scale: f32) -> Option<usize> {
    UI_SCALES.iter().position(|s| (s - scale).abs() < 1e-3)
}

impl Settings {
    /// Adds `path` as the most recent project: first in the list, no duplicate, at most [`MAX_RECENT`].
    pub fn push_recent(&mut self, path: &str) {
        self.recent_projects.retain(|p| p != path);
        self.recent_projects.insert(0, path.to_owned());
        self.recent_projects.truncate(MAX_RECENT);
    }

    /// Removes `path` from the recent projects.
    pub fn remove_recent(&mut self, path: &str) {
        self.recent_projects.retain(|p| p != path);
    }

    /// Adds or removes the favourite part `item` (`<library ID>/<item ID>`, D18); the list stays sorted. `true` when
    /// it is a favourite afterwards.
    pub fn toggle_favourite_part(&mut self, item: &str) -> bool {
        if let Some(i) = self.parts_favourites.iter().position(|f| f == item) {
            self.parts_favourites.remove(i);
            false
        } else {
            self.parts_favourites.push(item.to_owned());
            self.parts_favourites.sort();
            true
        }
    }

    /// Records the use of the part `item` (`<library ID>/<item ID>`, D18): first in the list, no duplicate, at most
    /// [`MAX_RECENT_PARTS`].
    pub fn record_part_use(&mut self, item: &str) {
        self.parts_recent.retain(|p| p != item);
        self.parts_recent.insert(0, item.to_owned());
        self.parts_recent.truncate(MAX_RECENT_PARTS);
    }

    /// The settings of a TOML text; `None` when it is not valid TOML or a known key has the wrong type.
    pub fn parse(text: &str) -> Option<Settings> {
        let mut table: toml::Table = toml::from_str(text).ok()?;
        table.remove("version");
        let recent = match table.remove("recent_projects") {
            None => Vec::new(),
            Some(toml::Value::Array(items)) => items
                .into_iter()
                .map(|v| match v {
                    toml::Value::String(s) => Some(s),
                    _ => None,
                })
                .collect::<Option<Vec<String>>>()?,
            Some(_) => return None,
        };
        let ui_scale = match table.remove("ui_scale") {
            None => 1.0,
            Some(toml::Value::Float(f)) => f as f32,
            Some(toml::Value::Integer(i)) => i as f32,
            Some(_) => return None,
        };
        let first_run_done = take_bool(&mut table, "first_run_done")?;
        let update_check = take_bool(&mut table, "update_check")?;
        let language = take_choice(&mut table, "language", &LANGUAGES)?;
        let keymap = take_choice(&mut table, "keymap", &KEYMAPS)?;
        let theme = take_choice(&mut table, "theme", &THEMES)?;
        let density = take_choice(&mut table, "density", &DENSITIES)?;
        let canvas_backend = take_choice(&mut table, "canvas_backend", &CANVAS_BACKENDS)?;
        let comes_from = match take_string(&mut table, "comes_from")? {
            Some(v) if ORIGINS.contains(&v.as_str()) => v,
            _ => String::new(),
        };
        let basic_project = take_string(&mut table, "basic_project")?.filter(|v| !v.is_empty());
        let author_name = take_string(&mut table, "author_name")?
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        let workspace = take_string(&mut table, "workspace")?
            .filter(|v| valid_workspace_name(v))
            .unwrap_or_else(|| DEFAULT_WORKSPACE.to_owned());
        // `[properties] collapsed`: a list of texts; other members of the table are kept.
        let mut properties_collapsed: Vec<String> = Vec::new();
        let mut properties_show_advanced = false;
        if let Some(toml::Value::Table(props)) = table.get_mut("properties") {
            // WP-S2.17 part 3 (D11): `[properties] show_advanced`.
            match props.remove("show_advanced") {
                None => {}
                Some(toml::Value::Boolean(on)) => properties_show_advanced = on,
                Some(_) => return None,
            }
            match props.remove("collapsed") {
                None => {}
                Some(toml::Value::Array(items)) => {
                    for v in items {
                        match v {
                            toml::Value::String(s) => properties_collapsed.push(s),
                            _ => return None,
                        }
                    }
                }
                Some(_) => return None,
            }
            if props.is_empty() {
                table.remove("properties");
            }
        }
        properties_collapsed.sort();
        properties_collapsed.dedup();
        // WP-S2.07 D18: `[parts] favourites` and `recent`, lists of texts; other members of the table are kept.
        let mut parts_favourites: Vec<String> = Vec::new();
        let mut parts_recent: Vec<String> = Vec::new();
        if let Some(toml::Value::Table(parts)) = table.get_mut("parts") {
            for (key, list) in [
                ("favourites", &mut parts_favourites),
                ("recent", &mut parts_recent),
            ] {
                match parts.remove(key) {
                    None => {}
                    Some(toml::Value::Array(items)) => {
                        for v in items {
                            match v {
                                toml::Value::String(s) if !list.contains(&s) => list.push(s),
                                toml::Value::String(_) => {}
                                _ => return None,
                            }
                        }
                    }
                    Some(_) => return None,
                }
            }
            if parts.is_empty() {
                table.remove("parts");
            }
        }
        parts_favourites.sort();
        parts_recent.truncate(MAX_RECENT_PARTS);
        // WP-S2.17 part 3 (FT-348): `[window]`. Known members this version cannot read are dropped, not the whole
        // file: the window then opens at its default size. Other members of the table are kept (R1-6).
        let window = match table.remove("window") {
            Some(toml::Value::Table(mut w)) => {
                let geometry = crate::window::WindowGeometry::from_toml(&w);
                for key in crate::window::KEYS {
                    w.remove(key);
                }
                if !w.is_empty() {
                    table.insert("window".to_owned(), toml::Value::Table(w));
                }
                geometry
            }
            Some(other) => {
                table.insert("window".to_owned(), other);
                None
            }
            None => None,
        };
        let mut settings = Settings {
            recent_projects: Vec::new(),
            ui_scale: if scale_index(ui_scale).is_some() {
                ui_scale
            } else {
                1.0
            },
            first_run_done,
            language,
            keymap,
            theme,
            density,
            comes_from,
            update_check,
            basic_project,
            workspace,
            canvas_backend,
            properties_collapsed,
            properties_show_advanced,
            author_name,
            window,
            parts_favourites,
            parts_recent,
            extra: table,
        };
        // The list is normalised on read: no duplicates, at most MAX_RECENT, first occurrence wins.
        for path in recent {
            if !settings.recent_projects.contains(&path) && settings.recent_projects.len() < MAX_RECENT {
                settings.recent_projects.push(path);
            }
        }
        Some(settings)
    }

    /// The company library directories of the settings key `libraries.company` (`[libraries] company = [...]`,
    /// WP-S1.19 D4), in order: the company tier. A relative path is read against `config_dir` (the settings'
    /// directory) when there is one; a member that is not a text, and a malformed key, are left out. The key is
    /// kept and written back as it was (it is one of the kept keys, edited in the file).
    pub fn company_libraries(&self, config_dir: Option<&Path>) -> Vec<PathBuf> {
        let Some(toml::Value::Table(libraries)) = self.extra.get("libraries") else {
            return Vec::new();
        };
        let Some(toml::Value::Array(items)) = libraries.get("company") else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|v| v.as_str())
            .filter(|p| !p.trim().is_empty())
            .map(|p| {
                let path = PathBuf::from(p);
                match config_dir {
                    Some(dir) if path.is_relative() => dir.join(path),
                    _ => path,
                }
            })
            .collect()
    }

    /// The TOML text of the settings: `version`, the known keys and the kept unknown ones, keys sorted.
    pub fn to_toml(&self) -> String {
        let mut table = self.extra.clone();
        table.insert("version".to_owned(), toml::Value::Integer(1));
        table.insert(
            "recent_projects".to_owned(),
            toml::Value::Array(
                self.recent_projects
                    .iter()
                    .map(|p| toml::Value::String(p.clone()))
                    .collect(),
            ),
        );
        table.insert(
            "ui_scale".to_owned(),
            toml::Value::Float(f64::from(self.ui_scale)),
        );
        table.insert(
            "first_run_done".to_owned(),
            toml::Value::Boolean(self.first_run_done),
        );
        table.insert("update_check".to_owned(), toml::Value::Boolean(self.update_check));
        for (key, value) in [
            ("language", &self.language),
            ("keymap", &self.keymap),
            ("theme", &self.theme),
            ("density", &self.density),
            ("comes_from", &self.comes_from),
            ("workspace", &self.workspace),
            ("canvas_backend", &self.canvas_backend),
        ] {
            table.insert(key.to_owned(), toml::Value::String(value.clone()));
        }
        match &self.basic_project {
            Some(path) => {
                table.insert("basic_project".to_owned(), toml::Value::String(path.clone()));
            }
            None => {
                table.remove("basic_project");
            }
        }
        match &self.author_name {
            Some(name) => {
                table.insert("author_name".to_owned(), toml::Value::String(name.clone()));
            }
            None => {
                table.remove("author_name");
            }
        }
        // Written only when a category is collapsed or Advanced is shown, so an untouched settings file keeps its
        // bytes.
        if !self.properties_collapsed.is_empty() || self.properties_show_advanced {
            let mut props = match table.remove("properties") {
                Some(toml::Value::Table(t)) => t,
                _ => toml::Table::new(),
            };
            if !self.properties_collapsed.is_empty() {
                props.insert(
                    "collapsed".to_owned(),
                    toml::Value::Array(
                        self.properties_collapsed
                            .iter()
                            .map(|k| toml::Value::String(k.clone()))
                            .collect(),
                    ),
                );
            }
            if self.properties_show_advanced {
                props.insert("show_advanced".to_owned(), toml::Value::Boolean(true));
            }
            table.insert("properties".to_owned(), toml::Value::Table(props));
        }
        // Written only once a window has closed, so a settings file of the headless mode keeps its bytes; merged
        // into the kept members of the table.
        if let Some(w) = &self.window {
            let mut members = match table.remove("window") {
                Some(toml::Value::Table(t)) => t,
                _ => toml::Table::new(),
            };
            members.extend(w.to_toml());
            table.insert("window".to_owned(), toml::Value::Table(members));
        }
        // WP-S2.07 D18: written only once a part is a favourite or was chosen, merged into the kept members.
        if !self.parts_favourites.is_empty() || !self.parts_recent.is_empty() {
            let mut parts = match table.remove("parts") {
                Some(toml::Value::Table(t)) => t,
                _ => toml::Table::new(),
            };
            for (key, list) in [
                ("favourites", &self.parts_favourites),
                ("recent", &self.parts_recent),
            ] {
                if list.is_empty() {
                    parts.remove(key);
                } else {
                    parts.insert(
                        key.to_owned(),
                        toml::Value::Array(list.iter().map(|k| toml::Value::String(k.clone())).collect()),
                    );
                }
            }
            table.insert("parts".to_owned(), toml::Value::Table(parts));
        }
        // A table of plain values always serialises.
        toml::to_string(&table).unwrap_or_default()
    }

    /// Reads `<dir>/settings.toml`. An absent file gives the defaults; an unreadable one gives the defaults and
    /// is copied to `settings.toml.bad-<n>`, `n` the lowest free number.
    pub fn load(dir: &Path) -> Loaded {
        let path = dir.join(FILE_NAME);
        let Ok(bytes) = std::fs::read(&path) else {
            // Absent (the first start), or a file that cannot be read at all (nothing to copy aside): defaults.
            return Loaded {
                settings: Settings::default(),
                bad_copy: None,
            };
        };
        match std::str::from_utf8(&bytes).ok().and_then(Settings::parse) {
            Some(settings) => Loaded {
                settings,
                bad_copy: None,
            },
            None => Loaded {
                settings: Settings::default(),
                bad_copy: copy_aside(&path),
            },
        }
    }

    /// Writes `<dir>/settings.toml` through a temporary file and a rename; creates `dir` if needed.
    ///
    /// # Errors
    ///
    /// The I/O error of the first step that failed.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        write_atomically(dir, &self.to_toml())
    }

    /// Merges the window geometry `geometry` into `<dir>/settings.toml` as the file is on disk now (R1-5): only the
    /// known members of `[window]` change, every other key and table stays as another instance may have written it
    /// meanwhile. Nothing is written when the file already holds this geometry (as whole points), or when it is
    /// unreadable (the next start copies it aside); an absent file gets the defaults with the geometry. Written
    /// through a temporary file and a rename, as [`Settings::save`]. Returns whether it wrote.
    ///
    /// # Errors
    ///
    /// The I/O error of the read or of the first write step that failed.
    pub fn save_window(dir: &Path, geometry: crate::window::WindowGeometry) -> std::io::Result<bool> {
        let bytes = match std::fs::read(dir.join(FILE_NAME)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let fresh = Settings {
                    window: Some(geometry),
                    ..Settings::default()
                };
                fresh.save(dir)?;
                return Ok(true);
            }
            Err(e) => return Err(e),
        };
        let Some(text) = std::str::from_utf8(&bytes).ok() else {
            return Ok(false);
        };
        let (Some(on_disk), Ok(mut table)) = (Settings::parse(text), toml::from_str::<toml::Table>(text))
        else {
            return Ok(false);
        };
        let wanted = geometry.to_toml();
        if on_disk.window.map(|w| w.to_toml()) == Some(wanted.clone()) {
            return Ok(false);
        }
        let mut members = match table.remove("window") {
            Some(toml::Value::Table(t)) => t,
            _ => toml::Table::new(),
        };
        members.extend(wanted);
        table.insert("window".to_owned(), toml::Value::Table(members));
        let text = toml::to_string(&table).map_err(std::io::Error::other)?;
        write_atomically(dir, &text)?;
        Ok(true)
    }
}

/// Writes `text` as `<dir>/settings.toml` through a temporary file and a rename; creates `dir` if needed.
fn write_atomically(dir: &Path, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(FILE_NAME);
    let tmp = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &path)
}

/// Removes a boolean key: absent is `false`, another type makes the file bad (`None`).
fn take_bool(table: &mut toml::Table, key: &str) -> Option<bool> {
    match table.remove(key) {
        None => Some(false),
        Some(toml::Value::Boolean(b)) => Some(b),
        Some(_) => None,
    }
}

/// Removes a string key: `Some(None)` when absent, `None` (a bad file) for another type.
fn take_string(table: &mut toml::Table, key: &str) -> Option<Option<String>> {
    match table.remove(key) {
        None => Some(None),
        Some(toml::Value::String(s)) => Some(Some(s)),
        Some(_) => None,
    }
}

/// Removes a string key that names one of `choices`; absent or unknown gives the first choice.
fn take_choice(table: &mut toml::Table, key: &str, choices: &[&str]) -> Option<String> {
    let value = take_string(table, key)?;
    Some(match value {
        Some(v) if choices.contains(&v.as_str()) => v,
        _ => choices[0].to_owned(),
    })
}

/// Whether `name` is a name that Windows reserves for a device (`CON`, `NUL`, `COM1` ...), in any letter case:
/// a file of that name cannot be made there, so it is no workspace name on any system.
fn is_windows_device_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            upper
                .strip_prefix(prefix)
                .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
        })
}

/// Whether `name` can be a workspace name: 1 to 40 characters, letters, digits, space, `-` and `_`, no leading
/// or trailing space. The name is the file name of `workspaces/<name>.json`.
pub fn valid_workspace_name(name: &str) -> bool {
    !is_windows_device_name(name)
        && !name.is_empty()
        && name.chars().count() <= 40
        && name == name.trim()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
}

/// Copies the bad settings file to `settings.toml.bad-<n>` (the lowest free `n`); the name, or `None` when the
/// copy failed.
fn copy_aside(path: &Path) -> Option<String> {
    copy_aside_as(path, FILE_NAME)
}

/// Copies the bad file `path` to `<file_name>.bad-<n>` beside it (the lowest free `n`); the name, or `None` when the
/// copy failed. WP-S2.08 part 2: a refused `keymap.toml` is kept the same way.
pub(crate) fn copy_aside_as(path: &Path, file_name: &str) -> Option<String> {
    let dir = path.parent()?;
    for n in 1..=999 {
        let name = format!("{file_name}.bad-{n}");
        let target: PathBuf = dir.join(&name);
        if !target.exists() {
            return std::fs::copy(path, &target).ok().map(|_| name);
        }
    }
    None
}

/// An environment variable as a path, `None` when unset or empty.
// Program boundary: the settings directory override is read once at start-up and passed in.
#[allow(clippy::disallowed_methods)]
pub(crate) fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The settings directory: `EW_CONFIG_DIR` when set, else `%APPDATA%\ElektraWatt` on Windows,
/// `~/Library/Application Support/ElektraWatt` on macOS and `$XDG_CONFIG_HOME/elektrawatt` (else
/// `~/.config/elektrawatt`) elsewhere; `None` when the environment names none.
pub fn default_config_dir() -> Option<PathBuf> {
    let var = env_path;
    if let Some(dir) = var("EW_CONFIG_DIR") {
        return Some(dir);
    }
    if cfg!(windows) {
        var("APPDATA").map(|p| p.join("ElektraWatt"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|p| p.join("Library/Application Support/ElektraWatt"))
    } else {
        var("XDG_CONFIG_HOME")
            .map(|p| p.join("elektrawatt"))
            .or_else(|| var("HOME").map(|p| p.join(".config/elektrawatt")))
    }
}

#[cfg(test)]
mod parts_tests {
    use super::*;

    /// D18, PS3: favourites sorted, recent most recent first without duplicates and at most 20; unknown members of
    /// `[parts]` kept; an untouched file writes no `[parts]`.
    #[test]
    fn favourites_and_recent_parts_round_trip() {
        assert!(!Settings::default().to_toml().contains("[parts]"));
        let mut s = Settings::parse("[parts]\nkeep = 1\n").unwrap();
        assert!(s.toggle_favourite_part("b/2"));
        assert!(s.toggle_favourite_part("a/1"));
        assert_eq!(s.parts_favourites, ["a/1", "b/2"]);
        assert!(!s.toggle_favourite_part("b/2"));
        for i in 0..21 {
            s.record_part_use(&format!("l/{i}"));
        }
        s.record_part_use("l/5");
        assert_eq!(s.parts_recent.len(), MAX_RECENT_PARTS);
        assert_eq!(s.parts_recent[..2], ["l/5".to_owned(), "l/20".to_owned()]);
        let text = s.to_toml();
        assert!(text.contains("keep = 1"), "{text}");
        let again = Settings::parse(&text).unwrap();
        assert_eq!(
            (again.parts_favourites, again.parts_recent),
            (s.parts_favourites, s.parts_recent)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        crate::shell::fresh_test_dir(&format!("ew-settings-{tag}"))
    }

    #[test]
    fn a_round_trip_keeps_unknown_keys() {
        let dir = temp_dir("roundtrip");
        std::fs::write(
            dir.join(FILE_NAME),
            "version = 1\nzzz = \"later\"\nrecent_projects = [\"a\", \"b\"]\nui_scale = 1.5\n\n[future]\nkey = 3\n",
        )
        .unwrap();
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.bad_copy, None);
        assert_eq!(loaded.settings.recent_projects, ["a", "b"]);
        assert_eq!(loaded.settings.ui_scale, 1.5);
        loaded.settings.save(&dir).unwrap();
        let text = std::fs::read_to_string(dir.join(FILE_NAME)).unwrap();
        assert!(text.contains("zzz = \"later\""), "{text}");
        assert!(text.contains("[future]"), "{text}");
        assert!(text.contains("key = 3"), "{text}");
        let again = Settings::load(&dir);
        assert_eq!(again.settings, loaded.settings);
        // Writing twice gives equal bytes (keys sorted).
        again.settings.save(&dir).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(), text);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Loading a file that is not a usable settings file gives the defaults: an absent file without a copy, a
    /// corrupted one with a numbered copy.
    #[test]
    fn a_corrupted_file_gives_defaults_and_a_numbered_copy() {
        type Case = (&'static str, fn(&str));
        let cases: [Case; 2] = [
            ("an absent file", |c| {
                let dir = temp_dir("absent");
                let loaded = Settings::load(&dir);
                assert_eq!(loaded.settings, Settings::default(), "case {c}");
                assert_eq!(loaded.bad_copy, None, "case {c}");
                let _ = std::fs::remove_dir_all(&dir);
            }),
            ("a corrupted file", |c| {
                let dir = temp_dir("bad");
                std::fs::write(dir.join(FILE_NAME), "this is = not [ toml").unwrap();
                let loaded = Settings::load(&dir);
                assert_eq!(loaded.settings, Settings::default(), "case {c}");
                assert_eq!(
                    loaded.bad_copy.as_deref(),
                    Some("settings.toml.bad-1"),
                    "case {c}"
                );
                assert!(dir.join("settings.toml.bad-1").is_file(), "case {c}");
                // The next bad file takes the next free number.
                let again = Settings::load(&dir);
                assert_eq!(again.bad_copy.as_deref(), Some("settings.toml.bad-2"), "case {c}");
                // A well-formed file with a wrong type for a known key is bad too.
                std::fs::write(dir.join(FILE_NAME), "recent_projects = 3\n").unwrap();
                assert_eq!(
                    Settings::load(&dir).bad_copy.as_deref(),
                    Some("settings.toml.bad-3"),
                    "case {c}"
                );
                let _ = std::fs::remove_dir_all(&dir);
            }),
        ];
        for (name, case) in cases {
            case(name);
        }
    }

    #[test]
    fn the_recent_list_is_capped_deduplicated_and_most_recent_first() {
        let mut s = Settings::default();
        for i in 0..14 {
            s.push_recent(&format!("p{i}"));
        }
        assert_eq!(s.recent_projects.len(), MAX_RECENT);
        assert_eq!(s.recent_projects[0], "p13");
        assert_eq!(s.recent_projects[9], "p4");
        s.push_recent("p6");
        assert_eq!(s.recent_projects[0], "p6");
        assert_eq!(s.recent_projects.iter().filter(|p| *p == "p6").count(), 1);
        assert_eq!(s.recent_projects.len(), MAX_RECENT);
        s.remove_recent("p13");
        assert!(!s.recent_projects.contains(&"p13".to_owned()));
    }

    /// FT-674: the author name round-trips, is trimmed, and is written only once set (an untouched file keeps its
    /// bytes); a blank one reads as unset.
    #[test]
    fn the_author_name_round_trips_and_is_written_only_once_set() {
        assert!(!Settings::default().to_toml().contains("author_name"));
        let s = Settings {
            author_name: Some("A. Engineer".to_owned()),
            ..Settings::default()
        };
        let back = Settings::parse(&s.to_toml()).unwrap();
        assert_eq!(back.author_name.as_deref(), Some("A. Engineer"));
        let padded = Settings::parse("author_name = \"  B  \"\n").unwrap();
        assert_eq!(padded.author_name.as_deref(), Some("B"));
        assert_eq!(
            Settings::parse("author_name = \" \"\n").unwrap().author_name,
            None
        );
        assert_eq!(
            Settings::parse("author_name = 3\n"),
            None,
            "a known key of the wrong type"
        );
    }

    #[test]
    fn the_part_two_keys_round_trip_and_fall_back_to_their_defaults() {
        let dir = temp_dir("part2");
        let defaults = Settings::default();
        assert!(
            !defaults.first_run_done && !defaults.update_check,
            "opt-in and not yet run"
        );
        assert_eq!(defaults.workspace, DEFAULT_WORKSPACE);
        let s = Settings {
            first_run_done: true,
            update_check: true,
            keymap: "kicad".to_owned(),
            theme: "dark".to_owned(),
            comes_from: "qet".to_owned(),
            basic_project: Some("company.ew".to_owned()),
            workspace: "Review".to_owned(),
            canvas_backend: "cpu".to_owned(),
            ..Settings::default()
        };
        s.save(&dir).unwrap();
        let back = Settings::load(&dir);
        assert_eq!(back.bad_copy, None);
        assert_eq!(back.settings, s);
        // An unknown choice falls back to the first; a name that cannot be a file name to the default workspace.
        let loose = Settings::parse(
            "keymap = \"emacs\"\ntheme = \"purple\"\ncomes_from = \"vi\"\nworkspace = \"../x\"\nbasic_project = \"\"\n",
        )
        .unwrap();
        assert_eq!(loose.keymap, "ew");
        assert_eq!(loose.theme, "system");
        assert_eq!(loose.comes_from, "");
        assert_eq!(loose.workspace, DEFAULT_WORKSPACE);
        assert_eq!(loose.basic_project, None);
        assert_eq!(loose.canvas_backend, "auto");
        let metal = Settings::parse("canvas_backend = \"metal\"\n").unwrap();
        assert_eq!(metal.canvas_backend, "auto");
        // WP-S2.17 DL7: the density and the high-contrast themes round-trip; an unknown density falls back.
        for (theme, density) in [("hc-light", "compact"), ("hc-dark", "comfortable")] {
            let s = Settings {
                theme: theme.to_owned(),
                density: density.to_owned(),
                ..Settings::default()
            };
            let back = Settings::parse(&s.to_toml()).unwrap();
            assert_eq!((back.theme.as_str(), back.density.as_str()), (theme, density));
        }
        assert_eq!(loose.density, "comfortable");
        let dense = Settings::parse("density = \"dense\"\n").unwrap();
        assert_eq!(dense.density, "comfortable");
        assert!(Settings::parse("canvas_backend = 1\n").is_none());
        // A key of the wrong type makes the file bad, like the older keys.
        for text in [
            "first_run_done = \"yes\"\n",
            "update_check = 1\n",
            "language = 3\n",
            "basic_project = true\n",
        ] {
            assert!(Settings::parse(text).is_none(), "{text}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_names_are_file_names() {
        for good in [
            "Schematic",
            "My layout",
            "a-b_c",
            "Räume 2",
            "COM10",
            "CONSOLE",
            "COM0",
        ] {
            assert!(valid_workspace_name(good), "{good}");
        }
        for bad in [
            "",
            " a",
            "a ",
            "a/b",
            "a\\b",
            "..",
            "a.b",
            "CON",
            "nul",
            "Prn",
            "AUX",
            "COM1",
            "com9",
            "LPT3",
            &"x".repeat(41),
        ] {
            assert!(!valid_workspace_name(bad), "{bad:?}");
        }
    }

    #[test]
    fn an_unknown_scale_falls_back_to_one() {
        let s = Settings::parse("ui_scale = 1.1\n").unwrap();
        assert_eq!(s.ui_scale, 1.0);
        let s = Settings::parse("ui_scale = 2\n").unwrap();
        assert_eq!(s.ui_scale, 2.0);
    }

    /// DK6 (FT-348): `[window]` is written only once a window recorded it, read back equal, and a table this version
    /// cannot read is dropped without making the file bad; members of the table this version does not know are kept.
    #[test]
    fn dk6_the_window_table() {
        type Case = (&'static str, fn(&str));
        let cases: [Case; 2] = [
            ("written once recorded, a bad table dropped", |c| {
                let mut s = Settings::default();
                assert!(!s.to_toml().contains("[window]"), "case {c}");
                s.window = Some(crate::window::WindowGeometry {
                    x: -1200.0,
                    y: 40.0,
                    width: 1280.0,
                    height: 800.0,
                    maximized: true,
                });
                let text = s.to_toml();
                assert!(text.contains("[window]"), "case {c}: {text}");
                assert_eq!(Settings::parse(&text).unwrap(), s, "case {c}");
                let bad = Settings::parse("theme = \"dark\"\n[window]\nx = \"left\"\n").unwrap();
                assert_eq!(bad.window, None, "case {c}");
                assert_eq!(bad.theme, "dark", "case {c}");
            }),
            ("unknown members are kept", |c| {
                let mut s = Settings::parse(
                    "[window]\nx = 10\ny = 20\nwidth = 800\nheight = 600\nmonitor = \"DP-1\"\n",
                )
                .unwrap();
                assert_eq!(s.window.map(|w| w.width), Some(800.0), "case {c}");
                assert!(
                    s.to_toml().contains("monitor = \"DP-1\""),
                    "case {c}: {}",
                    s.to_toml()
                );
                s.window = s
                    .window
                    .map(|w| crate::window::WindowGeometry { width: 1024.0, ..w });
                let text = s.to_toml();
                assert!(text.contains("monitor = \"DP-1\""), "case {c}: {text}");
                assert!(text.contains("width = 1024"), "case {c}: {text}");
                assert_eq!(Settings::parse(&text).unwrap(), s, "case {c}");
                let bad = Settings::parse("[window]\nx = \"left\"\nmonitor = \"DP-1\"\n").unwrap();
                assert_eq!(bad.window, None, "case {c}");
                let text = bad.to_toml();
                assert!(text.contains("monitor = \"DP-1\""), "case {c}: {text}");
                assert!(!text.contains("left"), "case {c}: {text}");
            }),
        ];
        for (name, case) in cases {
            case(name);
        }
    }

    /// R1-5: two instances' settings on one file. The later exit merges only its window geometry into the file on
    /// disk, so the earlier instance's recent project and theme stay; the same geometry again writes nothing; an
    /// unreadable file is left alone; an absent one gets the defaults with the geometry.
    #[test]
    fn a_window_exit_merges_only_the_geometry() {
        let dir = temp_dir("savewindow");
        std::fs::write(
            dir.join(FILE_NAME),
            "version = 1
[future]
key = 3
",
        )
        .unwrap();
        let mut a = Settings::load(&dir).settings;
        let b = Settings::load(&dir).settings;
        a.push_recent("y");
        a.theme = "dark".to_owned();
        a.save(&dir).unwrap();
        let g = crate::window::WindowGeometry {
            x: -1200.4,
            y: 40.0,
            width: 1280.0,
            height: 800.0,
            maximized: false,
        };
        assert_eq!(b.recent_projects, Vec::<String>::new());
        assert!(Settings::save_window(&dir, g).unwrap());
        let after = Settings::load(&dir).settings;
        assert_eq!(after.recent_projects, ["y"]);
        assert_eq!(after.theme, "dark");
        assert_eq!(after.window.map(|w| w.x), Some(-1200.0));
        assert!(after.to_toml().contains("[future]"));
        // The same geometry (as whole points): nothing written.
        assert!(!Settings::save_window(&dir, crate::window::WindowGeometry { x: -1199.6, ..g }).unwrap());
        // An unreadable file stays as it is.
        std::fs::write(
            dir.join(FILE_NAME),
            "version = = 1
",
        )
        .unwrap();
        assert!(!Settings::save_window(&dir, g).unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.join(FILE_NAME)).unwrap(),
            "version = = 1
"
        );
        // An absent file: the defaults and the geometry.
        std::fs::remove_file(dir.join(FILE_NAME)).unwrap();
        assert!(Settings::save_window(&dir, g).unwrap());
        assert_eq!(
            Settings::load(&dir).settings.window.map(|w| w.width),
            Some(1280.0)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
