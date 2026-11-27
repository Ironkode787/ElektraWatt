// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application settings file `settings.toml` (FR-UI-033, FR-UI-049).
//!
//! `version = 1`, `recent_projects` (at most [`MAX_RECENT`], most recent first, no duplicates), `ui_scale` (one
//! of [`UI_SCALES`]) and, since part 2, the first-run answers and quick switches: `first_run_done`, `language`,
//! `keymap`, `theme`, `density`, `comes_from`, `update_check` (an opt-in, off by default, NFR-PRIV-03), the templates
//! `[[templates]]` (each `dir` and `name`, FR-PRJ-009) with the one new projects start from, `basic_project` (the
//! key of one template path before the list; such a file loads as a list of that one), the active workspace
//! `workspace` (FR-UI-002) and
//! the canvas renderer `canvas_backend` (`auto`, or `cpu` for View, Use the CPU renderer) and the
//! author name `author_name` of the Waive prompt (written only once set), the DWG converter `dwg_converter` (written
//! only once chosen) and the
//! part dialog's `[parts] favourites` (sorted) and `recent` (most recent first, at most [`MAX_RECENT_PARTS`]), each a
//! list of `<library ID>/<item ID>` texts, written only once set, and the minimum text size
//! `min_font_pt` and `reduce_motion`, each written only once set, the spacing of a new terminal row in grid steps
//! `terminal_spacing` (written only once chosen), and the tips' switch `tips`
//! (written only when off) and `tips_seen` (the tips shown once, written only once one was). Unknown keys are kept
//! and written back; a `toml::Table` holds them sorted, so the file is deterministic. An unreadable file yields
//! the defaults and is copied to `settings.toml.bad-<n>`.

use std::path::{Path, PathBuf};

/// The recent-projects list holds at most this many paths.
pub const MAX_RECENT: usize = 10;

/// The recently used parts list holds at most this many library items.
pub const MAX_RECENT_PARTS: usize = 20;

/// The UI scales the user can choose, as factors on the native scale (to 300 %, FR-UI-027).
pub const UI_SCALES: [f32; 6] = [1.0, 1.25, 1.5, 2.0, 2.5, 3.0];

/// The label of the UI scale `scale` (`100 %`): its per cent in the pattern `scale`, so that every list of scales
/// (View ▸ UI scale, the first run) takes its labels from the one list of scales, [`UI_SCALES`].
pub fn scale_label(scale: f32) -> String {
    crate::strings::format(crate::key!("scale"), &[&(scale * 100.0).round().to_string()])
}

/// The choices of View ▸ Minimum text size: off, then 12, 14 and 16 pt.
pub const MIN_FONT_CHOICES: [u8; 4] = [0, 12, 14, 16];

/// The values of `min_font_pt` besides 0 (off): 8 to 16 pt.
pub const MIN_FONT_RANGE: std::ops::RangeInclusive<u8> = 8..=16;

/// The file name inside the settings directory.
pub const FILE_NAME: &str = "settings.toml";

/// The UI languages of this version: English (en-GB) and Dutch (nl-NL).
pub const LANGUAGES: [&str; 2] = ["en", "nl"];

/// The keymap presets the settings can name: EW and KiCad-like (`keymap::Preset`); the user's
/// `keymap.toml` beside `settings.toml` replaces bindings of either.
pub const KEYMAPS: [&str; 2] = ["ew", "kicad"];

/// The themes of the design language (doc 16a §4): follow the system (light or dark), light, dark and
/// the two high-contrast themes.
pub const THEMES: [&str; 5] = ["system", "light", "dark", "hc-dark", "hc-light"];

/// The densities (doc 16a §4.13): Comfortable (the default) and Compact.
pub const DENSITIES: [&str; 2] = ["comfortable", "compact"];

/// What the user comes from (first-run step 2, doc 16 section 9.1); the empty text is "not answered".
pub const ORIGINS: [&str; 4] = ["eplan", "qet", "kicad", "other"];

/// The word of `option` among the choices of the setting `setting` that the first run and the start page offer
/// ([`LANGUAGES`] by their own names, [`ORIGINS`] for `comes_from`, [`THEMES`], [`KEYMAPS`]); `?` for any other.
pub fn choice_word(setting: &str, option: &str) -> &'static str {
    use crate::key;
    crate::strings::text(match (setting, option) {
        ("language", "en") => key!("language-en"),
        ("language", "nl") => key!("language-nl"),
        ("comes_from", "eplan") => key!("origin-eplan"),
        ("comes_from", "qet") => key!("origin-qet"),
        ("comes_from", "kicad") => key!("origin-kicad"),
        ("comes_from", "other") => key!("origin-other"),
        ("theme", "system") => key!("theme-system"),
        ("theme", "light") => key!("theme-light"),
        ("theme", "dark") => key!("theme-dark"),
        ("theme", "hc-dark") => key!("theme-hc-dark"),
        ("theme", "hc-light") => key!("theme-hc-light"),
        ("keymap", "ew") => key!("keymap-ew"),
        ("keymap", "kicad") => key!("keymap-kicad"),
        _ => return "?",
    })
}

/// The values of `canvas_backend`: the switch rule decides (`auto`, the default), or always the CPU canvas (`cpu`).
pub const CANVAS_BACKENDS: [&str; 2] = ["auto", "cpu"];

/// The name of the Schematic workspace (doc 16 section 2.6) and the default of `workspace`.
pub const DEFAULT_WORKSPACE: &str = "Schematic";

/// The workspaces the program ships (doc 16 §2.6), each with its layout (`Layout::built_in`) and its label
/// (`workspace-<name>` in lower case).
pub const BUILT_IN_WORKSPACES: [&str; 3] = [DEFAULT_WORKSPACE, "PLC", "Fluid"];

/// The settings. Serde reads and writes every member of the file's top level, a missing one at its default
/// ([`Settings::default`]) and one of the wrong type making the file bad; a value it cannot take falls back
/// ([`Settings::lenient`]). The members of `[properties]`, `[parts]` and `[window]` are read and written beside the
/// other members of those tables, which are kept.
#[derive(Clone, PartialEq, Debug, serde::Deserialize, serde::Serialize)]
#[serde(default)]
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
    /// The density, one of [`DENSITIES`].
    pub density: String,
    /// What the user comes from, one of [`ORIGINS`], or empty.
    pub comes_from: String,
    /// The update-check opt-in (off by default; this version has no update check, NFR-PRIV-03).
    pub update_check: bool,
    /// The template new projects start from (a project directory of [`Settings::templates`]), if the user chose one
    /// (FR-PRJ-009).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic_project: Option<String>,
    /// The templates the user keeps, in the order they were added (`[[templates]]`); never in the recent projects.
    #[serde(deserialize_with = "templates", skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<TemplateEntry>,
    /// The active workspace (doc 16 section 2.6).
    pub workspace: String,
    /// The canvas renderer, one of [`CANVAS_BACKENDS`].
    pub canvas_backend: String,
    /// The property grid's collapsed categories, `<kind>/<category>` (`device/identity`), sorted (`[properties]
    /// collapsed`).
    #[serde(skip)]
    pub properties_collapsed: Vec<String>,
    /// Whether the property grid shows the Advanced rows (`[properties] show_advanced`).
    #[serde(skip)]
    pub properties_show_advanced: bool,
    /// The author name: the display name a waiver records as its author, prefilled in the Waive prompt and
    /// saved back when a waiver is committed; `None` until one is typed (`author_name`, written only when set).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author_name: Option<String>,
    /// The program of the ODA File Converter the user chose for DWG imports (FR-IMP-036, `dwg_converter`, written only
    /// when set); never bundled or downloaded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dwg_converter: Option<String>,
    /// The main window's last size, position and maximised state (`[window]`, item 9); `None`
    /// until a window has closed.
    #[serde(skip)]
    pub window: Option<crate::window::WindowGeometry>,
    /// The favourite parts, `<library ID>/<item ID>`, sorted (`[parts] favourites`).
    #[serde(skip)]
    pub parts_favourites: Vec<String>,
    /// The recently used parts, `<library ID>/<item ID>`, most recent first, at most [`MAX_RECENT_PARTS`] (`[parts]
    /// recent`).
    #[serde(skip)]
    pub parts_recent: Vec<String>,
    /// The minimum text size in points (`min_font_pt`): 0 is off, else one of [`MIN_FONT_RANGE`]; a value
    /// out of range loads as 0. Written only when set.
    #[serde(deserialize_with = "font_size", skip_serializing_if = "is_zero")]
    pub min_font_pt: u8,
    /// Reduce motion (`reduce_motion`, NFR-A11Y-04); `None` while unset, which is on with the CPU canvas
    /// and off otherwise (`ShellModel::reduce_motion`). Written only when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce_motion: Option<bool>,
    /// Whether the plain mouse wheel scrolls the canvas, Ctrl with the wheel zooming
    /// (`wheel_scrolls`); off, the default, the wheel zooms. Written only when on.
    #[serde(skip_serializing_if = "is_false")]
    pub wheel_scrolls: bool,
    /// Whether the one-time tips may be shown (`tips`; on unless the file says `tips = false`).
    #[serde(skip_serializing_if = "is_true")]
    pub tips: bool,
    /// The tips already shown, by their word ([`crate::shell::Tip::word`]), in the order they were shown
    /// (`tips_seen`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tips_seen: Vec<String>,
    /// The spacing of the next terminal row in grid steps (Terminal…'s Spacing, Spacing… on placed terminals), kept
    /// between sessions (`terminal_spacing`); `None` until one is chosen. Written only when set.
    #[serde(deserialize_with = "spacing", skip_serializing_if = "Option::is_none")]
    pub terminal_spacing: Option<u32>,
    /// The symbols edited lately in the symbol editor, by their library address (`library:<library>:<item>`), most
    /// recent first, at most [`MAX_RECENT`] (`symbols_recent`, written only once one was edited).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub symbols_recent: Vec<String>,
    /// What the symbol he saved last is, by its word of What it is (`coil`), which a new symbol starts as
    /// (`symbol_last_kind`, doc 07 §5.6); `None` until one was saved, a new symbol is then a normally open contact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_last_kind: Option<String>,
    /// The drawing grid he drew a symbol on last, an index of the drawing grids offered (0: 0.5 mm) (`symbol_drawing_grid`);
    /// `None` until one was chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_drawing_grid: Option<usize>,
    /// Whether the drawing snapped to its grid when he drew last (`symbol_snap`); `None` until chosen: on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_snap: Option<bool>,
    /// The scope the Symbols shelf of Parts showed last (`all`, `project`, `mine`; `symbols_scope`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols_scope: Option<String>,
    /// Keys this version does not know, kept and written back.
    #[serde(flatten)]
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
            templates: Vec::new(),
            workspace: DEFAULT_WORKSPACE.to_owned(),
            canvas_backend: CANVAS_BACKENDS[0].to_owned(),
            properties_collapsed: Vec::new(),
            properties_show_advanced: false,
            author_name: None,
            dwg_converter: None,
            window: None,
            parts_favourites: Vec::new(),
            parts_recent: Vec::new(),
            min_font_pt: 0,
            reduce_motion: None,
            wheel_scrolls: false,
            tips: true,
            tips_seen: Vec::new(),
            symbols_recent: Vec::new(),
            symbol_last_kind: None,
            symbol_drawing_grid: None,
            symbol_snap: None,
            symbols_scope: None,
            terminal_spacing: None,
            extra: toml::Table::new(),
        }
    }
}

/// One template of the Templates list: its folder and the name the list shows.
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize)]
pub struct TemplateEntry {
    /// The project folder.
    pub dir: String,
    /// The name the list, the New project dialog and the band of an open template show.
    pub name: String,
}

impl TemplateEntry {
    /// The template of folder `dir` named `name`, else by its folder's own name.
    pub fn new(dir: String, name: Option<String>) -> TemplateEntry {
        let name = name
            .map(|n| n.trim().to_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| {
                Path::new(&dir)
                    .file_name()
                    .map_or_else(|| dir.clone(), |n| n.to_string_lossy().into_owned())
            });
        TemplateEntry { dir, name }
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

    /// Whether the folder `dir` is one of the templates.
    pub fn is_template(&self, dir: &str) -> bool {
        self.templates.iter().any(|t| t.dir == dir)
    }

    /// Adds the folder `dir` to the templates under `name` (else its folder's name), at the end of the list, unless
    /// it is there already; it leaves the recent projects. Its index in the list.
    pub fn add_template(&mut self, dir: &str, name: Option<String>) -> usize {
        self.remove_recent(dir);
        match self.templates.iter().position(|t| t.dir == dir) {
            Some(i) => i,
            None => {
                self.templates.push(TemplateEntry::new(dir.to_owned(), name));
                self.templates.len() - 1
            }
        }
    }

    /// Takes the template at `index` out of the list (its folder stays on the disk); new projects no longer start
    /// from it.
    pub fn remove_template(&mut self, index: usize) -> Option<TemplateEntry> {
        (index < self.templates.len()).then(|| {
            let entry = self.templates.remove(index);
            if self.basic_project.as_deref() == Some(entry.dir.as_str()) {
                self.basic_project = None;
            }
            entry
        })
    }

    /// Adds or removes the favourite part `item` (`<library ID>/<item ID>`); the list stays sorted. `true` when
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

    /// Records that the symbol at `address` was edited: first in the list, no duplicate, at most [`MAX_RECENT`].
    pub fn record_symbol_edit(&mut self, address: &str) {
        self.symbols_recent.retain(|a| a != address);
        self.symbols_recent.insert(0, address.to_owned());
        self.symbols_recent.truncate(MAX_RECENT);
    }

    /// Records the use of the part `item` (`<library ID>/<item ID>`): first in the list, no duplicate, at most
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
        let mut settings: Settings = toml::Value::Table(table).try_into().ok()?;
        settings.lenient();
        settings.read_tables()?;
        Some(settings)
    }

    /// The values read that this version cannot take, at their fallbacks, and the lists normalised: a scale or a
    /// choice it does not offer at its default (an unknown origin as none), a workspace that cannot be a file name at
    /// the default one, blank texts as unset, a list without duplicates (the first one kept) and cut to its length;
    /// the template of a file written before the list (one path) a list of that one, and the recent projects without
    /// duplicates or templates, at most [`MAX_RECENT`].
    fn lenient(&mut self) {
        if scale_index(self.ui_scale).is_none() {
            self.ui_scale = 1.0;
        }
        for (value, choices) in [
            (&mut self.language, &LANGUAGES[..]),
            (&mut self.keymap, &KEYMAPS[..]),
            (&mut self.theme, &THEMES[..]),
            (&mut self.density, &DENSITIES[..]),
            (&mut self.canvas_backend, &CANVAS_BACKENDS[..]),
        ] {
            if !choices.contains(&value.as_str()) {
                choices[0].clone_into(value);
            }
        }
        if !ORIGINS.contains(&self.comes_from.as_str()) {
            self.comes_from.clear();
        }
        if !valid_workspace_name(&self.workspace) {
            DEFAULT_WORKSPACE.clone_into(&mut self.workspace);
        }
        self.basic_project = self.basic_project.take().filter(|v| !v.is_empty());
        self.author_name = self
            .author_name
            .take()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());
        self.dwg_converter = self.dwg_converter.take().filter(|v| !v.trim().is_empty());
        dedup(&mut self.tips_seen);
        dedup(&mut self.symbols_recent);
        self.symbols_recent.truncate(MAX_RECENT);
        // A word, a grid or a scope this version does not offer reads as none.
        let what = crate::shell::symbol_editor::WHAT;
        self.symbol_last_kind = (self.symbol_last_kind.take()).filter(|w| what.iter().any(|(k, ..)| k == w));
        self.symbol_drawing_grid =
            (self.symbol_drawing_grid).filter(|g| *g < crate::shell::symbol_editor::GRID_STEPS.len());
        self.symbols_scope = (self.symbols_scope.take())
            .filter(|s| ew_api::parts::PartScope::ALL.iter().any(|p| p.word() == s));
        // The template of a file written before the list (one path) is a list of that one.
        if let Some(dir) = self.basic_project.clone() {
            self.add_template(&dir, None);
        }
        // The list is normalised on read: no duplicates, at most MAX_RECENT, first occurrence wins; a template is
        // no recent project.
        let recent = std::mem::take(&mut self.recent_projects);
        for path in recent {
            if !self.recent_projects.contains(&path)
                && !self.is_template(&path)
                && self.recent_projects.len() < MAX_RECENT
            {
                self.recent_projects.push(path);
            }
        }
    }

    /// Takes the members of `[properties]`, `[parts]` and `[window]` out of the kept keys; `None` when one has the
    /// wrong type. Other members of those tables are kept.
    fn read_tables(&mut self) -> Option<()> {
        let table = &mut self.extra;
        // `[properties] collapsed`: a list of texts; other members of the table are kept.
        if let Some(toml::Value::Table(props)) = table.get_mut("properties") {
            // `[properties] show_advanced`.
            match props.remove("show_advanced") {
                None => {}
                Some(toml::Value::Boolean(on)) => self.properties_show_advanced = on,
                Some(_) => return None,
            }
            match props.remove("collapsed") {
                None => {}
                Some(toml::Value::Array(items)) => {
                    for v in items {
                        match v {
                            toml::Value::String(s) => self.properties_collapsed.push(s),
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
        self.properties_collapsed.sort();
        self.properties_collapsed.dedup();
        // `[parts] favourites` and `recent`, lists of texts; other members of the table are kept.
        if let Some(toml::Value::Table(parts)) = table.get_mut("parts") {
            for (key, list) in [
                ("favourites", &mut self.parts_favourites),
                ("recent", &mut self.parts_recent),
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
        self.parts_favourites.sort();
        self.parts_recent.truncate(MAX_RECENT_PARTS);
        // `[window]`. Known members this version cannot read are dropped, not the whole
        // file: the window then opens at its default size. Other members of the table are kept.
        self.window = match table.remove("window") {
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
        Some(())
    }

    /// The company library directories of the settings key `libraries.company` (`[libraries] company = [...]`),
    /// in order: the company tier. A relative path is read against `config_dir` (the settings'
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
        // A table of plain values always serialises.
        let mut table = toml::Table::try_from(self).unwrap_or_default();
        table.insert("version".to_owned(), toml::Value::Integer(1));
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
        // Written only once a part is a favourite or was chosen, merged into the kept members.
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

    /// Merges the window geometry `geometry` into `<dir>/settings.toml` as the file is on disk now: only the
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
    // The configuration directory is user-only.
    user_only_dir(dir);
    write_user_only(&dir.join(FILE_NAME), text.as_bytes())
}

/// The options that create a file user-only: mode 0600 on Unix. Every file EW creates in its
/// configuration directory is created through them, beside the directory's 0700 ([`user_only_dir`]).
pub fn user_only_options() -> std::fs::OpenOptions {
    let mut options = std::fs::OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options
}

/// Writes `bytes` as the file `path` all or nothing ([`ew_api::file`]), its temporary file created user-only
/// ([`user_only_options`]).
///
/// # Errors
///
/// The I/O error of the first step that failed.
pub fn write_user_only(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use ew_api::file::{Staged, temp_beside};
    Staged::write(temp_beside(path), path, bytes, &user_only_options())?.commit()
}

/// `[[templates]]`: each a `dir` and a `name` (texts; another type makes the file bad); an entry without a folder is
/// left out, one without a name takes its folder's, a folder given twice is kept once.
fn templates<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<TemplateEntry>, D::Error> {
    #[derive(serde::Deserialize)]
    struct Entry {
        dir: Option<String>,
        name: Option<String>,
    }
    let mut out: Vec<TemplateEntry> = Vec::new();
    for entry in <Vec<Entry> as serde::Deserialize>::deserialize(d)? {
        if let Some(dir) = entry.dir.filter(|d| !d.trim().is_empty())
            && !out.iter().any(|t| t.dir == dir)
        {
            out.push(TemplateEntry::new(dir, entry.name));
        }
    }
    Ok(out)
}

/// `min_font_pt`: an integer; a value out of range loads as off. A float of an integral value (`14.0`) loads as
/// that size, any other float as off; another type makes the file bad.
fn font_size<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    let in_range = |i: i64| {
        u8::try_from(i)
            .ok()
            .filter(|pt| MIN_FONT_RANGE.contains(pt))
            .unwrap_or(0)
    };
    match <toml::Value as serde::Deserialize>::deserialize(d)? {
        toml::Value::Integer(i) => Ok(in_range(i)),
        toml::Value::Float(f) if f.fract() == 0.0 && f.abs() < 1e6 => Ok(in_range(f as i64)),
        toml::Value::Float(_) => Ok(0),
        _ => Err(serde::de::Error::custom("min_font_pt is no number")),
    }
}

/// `terminal_spacing`: an integer; a spacing of no grid step or of more than a whole number holds is no spacing, and
/// the default stays.
fn spacing<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let n = <i64 as serde::Deserialize>::deserialize(d)?;
    Ok(u32::try_from(n).ok().filter(|n| *n > 0))
}

/// Whether a size is off (written only when set).
fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// Whether a switch is off (written only when on).
fn is_false(b: &bool) -> bool {
    !*b
}

/// Whether a switch is on (written only when off).
fn is_true(b: &bool) -> bool {
    *b
}

/// `list` without its duplicates, the first of each kept, in order.
fn dedup(list: &mut Vec<String>) {
    let mut seen: Vec<String> = Vec::with_capacity(list.len());
    list.retain(|v| {
        let new = !seen.contains(v);
        if new {
            seen.push(v.clone());
        }
        new
    });
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
/// copy failed. The copy is written user-only through [`write_aside`], whatever the source's mode.
pub(crate) fn copy_aside_as(path: &Path, file_name: &str) -> Option<String> {
    let dir = path.parent()?;
    let bytes = std::fs::read(path).ok()?;
    let free = (1..=BAD_COPIES).find(|n| !dir.join(format!("{file_name}.bad-{n}")).exists())?;
    write_aside(dir, file_name, free, &bytes)
}

/// As [`copy_aside_as`], but a copy with the same bytes that is already there is named instead of copying again: a
/// refused `keymap.toml`, which is read and refused on every start while the user's file stays, is
/// copied aside once. Every existing `.bad-<n>` is looked through for the same bytes before the first free
/// name is written, so a gap in the numbers (a copy the user deleted) never copies the same file twice.
pub(crate) fn copy_aside_once_as(path: &Path, file_name: &str) -> Option<String> {
    let dir = path.parent()?;
    let bytes = std::fs::read(path).ok()?;
    let names = (1..=BAD_COPIES).map(|n| format!("{file_name}.bad-{n}"));
    // Only a copy of the same length is read, so the look stays bounded by the file's own size.
    let same = names.clone().find(|name| {
        let target = dir.join(name);
        std::fs::metadata(&target).is_ok_and(|m| m.is_file() && m.len() == bytes.len() as u64)
            && std::fs::read(&target).is_ok_and(|held| held == bytes)
    });
    if same.is_some() {
        return same;
    }
    let free = (1..=BAD_COPIES).find(|n| !dir.join(format!("{file_name}.bad-{n}")).exists())?;
    write_aside(dir, file_name, free, &bytes)
}

/// The most `.bad-<n>` copies of one file.
const BAD_COPIES: u32 = 999;

/// The most attempts at a fresh temporary name for a copy aside.
const TEMP_ATTEMPTS: u32 = 64;

/// Writes `bytes` to `<file_name>.bad-<first>` in `dir`, or to the next free `.bad-<n>` when a file is at that name
/// by the time it is written (another instance's copy made meanwhile); the name, or `None`.
///
/// The bytes go to a fresh temporary file first, created only if nothing is at its name (the process and a counter
/// name it, so no other file or link is written through and two instances never share one; mode `0o600` on Unix),
/// which is then hard-linked to the target: a link never replaces a file that is there, and a failed write leaves no
/// half-written `.bad-<n>` that a later look would take for a different file. Where the file system has no hard links
/// (FAT, exFAT, some network shares), the link fails with another error, and the bytes are written to the target as a
/// new user-only file instead, which never replaces a file either. The temporary file is removed in
/// every case; any failure but a taken name gives up.
fn write_aside(dir: &Path, file_name: &str, first: u32, bytes: &[u8]) -> Option<String> {
    write_aside_linking(dir, file_name, first, bytes, |from, to| {
        std::fs::hard_link(from, to)
    })
}

/// As [`write_aside`], with `link` making the hard link (a seam for a file system without links).
fn write_aside_linking(
    dir: &Path,
    file_name: &str,
    first: u32,
    bytes: &[u8],
    link: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Option<String> {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let (tmp, mut file) = (0..TEMP_ATTEMPTS).find_map(|_| {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!("{file_name}.aside-{}-{n}.tmp", std::process::id()));
        user_only_options()
            .create_new(true)
            .open(&tmp)
            .ok()
            .map(|f| (tmp, f))
    })?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let linked = written.ok().and_then(|()| {
        let mut no_links = false;
        for n in first..=BAD_COPIES {
            let name = format!("{file_name}.bad-{n}");
            let target = dir.join(&name);
            let made = if no_links {
                write_new(&target, bytes)
            } else {
                match link(&tmp, &target) {
                    Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                        no_links = true;
                        write_new(&target, bytes)
                    }
                    made => made,
                }
            };
            match made {
                Ok(()) => return Some(name),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return None,
            }
        }
        None
    });
    let _ = std::fs::remove_file(&tmp);
    linked
}

/// Writes `bytes` as the new user-only file `path`, created only if nothing is at its name; a file this call created
/// and could not write in full is removed, so no half-written copy stays.
fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut file = user_only_options().create_new(true).open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if written.is_err() {
        let _ = std::fs::remove_file(path);
    }
    written
}

/// Makes the directory `dir` user-only on Unix, mode 0700, when its mode is wider (NFR-PRIV-05: the
/// configuration directory, the state directory's `crashes/`). On Windows the directories inherit the per-user ACL of
/// `%APPDATA%` and `%LOCALAPPDATA%`, and nothing is done. A failure is ignored: the directory keeps its mode.
pub fn user_only_dir(dir: &Path) {
    tighten(dir, 0o700);
}

/// Makes the file `file` user-only on Unix, mode 0600, as [`user_only_dir`] does a directory (a crash log).
pub fn user_only_file(file: &Path) {
    tighten(file, 0o600);
}

#[cfg(unix)]
fn tighten(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    if std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o777 & !mode != 0) {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
}

#[cfg(not(unix))]
fn tighten(_path: &Path, _mode: u32) {}

/// An environment variable as a path, `None` when unset or empty.
// Program boundary: the settings directory override is read once at start-up and passed in.
#[allow(
    clippy::disallowed_methods,
    reason = "the environment is read once at start-up"
)]
pub(crate) fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The user's projects folder: `Documents/ElektraWatt` in the home directory (`USERPROFILE` on Windows, `HOME`
/// elsewhere); `None` without one.
pub fn default_projects_dir() -> Option<PathBuf> {
    let home = if cfg!(windows) {
        env_path("USERPROFILE")
    } else {
        env_path("HOME")
    };
    home.map(|h| h.join("Documents").join("ElektraWatt"))
}

/// The settings directory: `EW_CONFIG_DIR` when set, else `%APPDATA%\ElektraWatt` on Windows and
/// `$XDG_CONFIG_HOME/elektrawatt` (else `~/.config/elektrawatt`) on Linux and any other system (macOS is not supported); `None` when the environment names none.
pub fn default_config_dir() -> Option<PathBuf> {
    let var = env_path;
    if let Some(dir) = var("EW_CONFIG_DIR") {
        return Some(dir);
    }
    if cfg!(windows) {
        var("APPDATA").map(|p| p.join("ElektraWatt"))
    } else {
        var("XDG_CONFIG_HOME")
            .map(|p| p.join("elektrawatt"))
            .or_else(|| var("HOME").map(|p| p.join(".config/elektrawatt")))
    }
}

#[cfg(test)]
mod parts_tests {
    use super::*;

    /// PS3: favourites sorted, recent most recent first without duplicates and at most 20; unknown members of
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

    fn temp_dir(tag: &str) -> ew_testdir::Tmp {
        ew_testdir::Tmp::new(&format!("ew-settings-{tag}"))
    }

    /// With `.bad-1` another file, `.bad-2` missing and `.bad-3` the same bytes, the file is named as
    /// `.bad-3` and not copied again into the gap; a new file takes the gap, the first free name.
    #[test]
    fn a_copy_aside_once_finds_the_same_bytes_past_a_gap() {
        let dir = temp_dir("bad-gap");
        let file = dir.join("keymap.toml");
        std::fs::write(&file, "bad = [").unwrap();
        std::fs::write(dir.join("keymap.toml.bad-1"), "other").unwrap();
        std::fs::write(dir.join("keymap.toml.bad-3"), "bad = [").unwrap();
        assert_eq!(
            copy_aside_once_as(&file, "keymap.toml").as_deref(),
            Some("keymap.toml.bad-3")
        );
        assert!(
            !dir.join("keymap.toml.bad-2").exists(),
            "no second copy in the gap"
        );
        std::fs::write(&file, "new = [").unwrap();
        assert_eq!(
            copy_aside_once_as(&file, "keymap.toml").as_deref(),
            Some("keymap.toml.bad-2")
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("keymap.toml.bad-2")).unwrap(),
            "new = ["
        );
        assert!(
            !dir.join("keymap.toml.bad-2.tmp").exists(),
            "no temporary file left"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A `.bad-<n>` another instance made after the look is never replaced: the copy takes the
    /// next free name, the other files keep their bytes, and no temporary file is left.
    #[test]
    fn a_copy_aside_never_replaces_a_copy_made_meanwhile() {
        let dir = temp_dir("bad-race");
        std::fs::write(dir.join("keymap.toml.bad-1"), "made meanwhile").unwrap();
        std::fs::write(dir.join("keymap.toml.bad-2"), "made meanwhile too").unwrap();
        assert_eq!(
            write_aside(&dir, "keymap.toml", 1, b"bad = [").as_deref(),
            Some("keymap.toml.bad-3")
        );
        let read = |n: u32| std::fs::read_to_string(dir.join(format!("keymap.toml.bad-{n}"))).unwrap();
        assert_eq!(
            [read(1), read(2), read(3)],
            ["made meanwhile", "made meanwhile too", "bad = ["]
        );
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["keymap.toml.bad-1", "keymap.toml.bad-2", "keymap.toml.bad-3"],
            "no temporary file left"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Where the file system refuses the hard link (FAT, exFAT), the copy is still made, as a
    /// new file that takes the next free name past one made meanwhile, and no temporary file is left.
    #[test]
    fn a_copy_aside_is_made_where_the_link_is_refused() {
        let dir = temp_dir("bad-nolink");
        std::fs::write(dir.join("settings.toml.bad-1"), "made meanwhile").unwrap();
        let refuse = |_: &Path, _: &Path| Err(std::io::Error::from(std::io::ErrorKind::Unsupported));
        assert_eq!(
            write_aside_linking(&dir, "settings.toml", 1, b"bad = [", refuse).as_deref(),
            Some("settings.toml.bad-2")
        );
        let read = |n: u32| std::fs::read_to_string(dir.join(format!("settings.toml.bad-{n}"))).unwrap();
        assert_eq!([read(1), read(2)], ["made meanwhile", "bad = ["]);
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["settings.toml.bad-1", "settings.toml.bad-2"],
            "no temporary file left"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(dir.join("settings.toml.bad-2"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file with members this version does not know (at the top, in a table of its own and inside the known
    /// tables), members missing, and values it cannot take (a choice it does not offer, a size out of range, a
    /// duplicated list entry, an empty template folder) reads leniently and is written back as these bytes: the
    /// unknown members kept, the missing ones at their defaults, the wrong values at their fallbacks, keys sorted. The
    /// file written reads back to the same settings and the same bytes.
    #[test]
    fn a_file_with_unknown_missing_and_wrong_members_is_read_and_written_as_these_bytes() {
        let file = "version = 1\nzzz = \"later\"\ntheme = \"purple\"\nlanguage = \"nl\"\nui_scale = 1.7\n\
            min_font_pt = 30\nterminal_spacing = 0\nrecent_projects = [\"b\", \"a\", \"b\"]\n\
            tips_seen = [\"wheel\", \"wheel\"]\ncomes_from = \"vi\"\nworkspace = \"../x\"\nreduce_motion = true\n\
            [future]\nkey = 3\n[window]\nwidth = 800\nheight = 600\nmonitor = \"left\"\n\
            [parts]\nfavourites = [\"b/2\", \"a/1\", \"a/1\"]\nkeep = 1\n\
            [properties]\ncollapsed = [\"device/z\", \"device/a\", \"device/z\"]\nnote = \"x\"\n\
            [[templates]]\ndir = \"\"\nname = \"none\"\n[[templates]]\ndir = \"/w/t.ew\"\n";
        let read = Settings::parse(file).expect("a lenient read");
        let written = read.to_toml();
        assert_eq!(
            written,
            "canvas_backend = \"auto\"\ncomes_from = \"\"\ndensity = \"comfortable\"\nfirst_run_done = false\n\
            keymap = \"ew\"\nlanguage = \"nl\"\nrecent_projects = [\"b\", \"a\"]\nreduce_motion = true\n\
            theme = \"system\"\ntips_seen = [\"wheel\"]\nui_scale = 1.0\nupdate_check = false\nversion = 1\n\
            workspace = \"Schematic\"\nzzz = \"later\"\n\n[future]\nkey = 3\n\n[parts]\nfavourites = [\"a/1\", \"b/2\"]\n\
            keep = 1\n\n[properties]\ncollapsed = [\"device/a\", \"device/z\"]\nnote = \"x\"\n\n[[templates]]\n\
            dir = \"/w/t.ew\"\nname = \"t.ew\"\n\n[window]\nmonitor = \"left\"\n"
        );
        let again = Settings::parse(&written).expect("its own file");
        assert_eq!(again, read);
        assert_eq!(again.to_toml(), written);
        // Every member set: its file, and the same settings read back.
        let full = Settings {
            recent_projects: vec!["/w/a.ew".to_owned()],
            ui_scale: 1.25,
            first_run_done: true,
            language: "nl".to_owned(),
            keymap: "kicad".to_owned(),
            theme: "dark".to_owned(),
            density: "compact".to_owned(),
            comes_from: "eplan".to_owned(),
            update_check: true,
            basic_project: Some("/w/t.ew".to_owned()),
            templates: vec![TemplateEntry::new(
                "/w/t.ew".to_owned(),
                Some("Company".to_owned()),
            )],
            workspace: "PLC".to_owned(),
            canvas_backend: "cpu".to_owned(),
            properties_collapsed: vec!["device/identity".to_owned()],
            properties_show_advanced: true,
            author_name: Some("A. Engineer".to_owned()),
            dwg_converter: Some("/opt/oda".to_owned()),
            window: crate::window::WindowGeometry::from_toml(
                &toml::from_str("x = 10\ny = 20\nwidth = 800\nheight = 600\nmaximized = false\n").unwrap(),
            ),
            parts_favourites: vec!["a/1".to_owned()],
            parts_recent: vec!["b/2".to_owned()],
            min_font_pt: 14,
            reduce_motion: Some(false),
            wheel_scrolls: true,
            tips: false,
            tips_seen: vec!["wheel".to_owned()],
            terminal_spacing: Some(4),
            symbols_recent: vec!["library:l:s".to_owned()],
            symbol_last_kind: Some("coil".to_owned()),
            symbol_drawing_grid: Some(1),
            symbol_snap: Some(false),
            symbols_scope: Some("mine".to_owned()),
            extra: toml::Table::new(),
        };
        let written = full.to_toml();
        assert_eq!(
            written,
            "author_name = \"A. Engineer\"\nbasic_project = \"/w/t.ew\"\ncanvas_backend = \"cpu\"\n\
            comes_from = \"eplan\"\ndensity = \"compact\"\ndwg_converter = \"/opt/oda\"\nfirst_run_done = true\n\
            keymap = \"kicad\"\nlanguage = \"nl\"\nmin_font_pt = 14\nrecent_projects = [\"/w/a.ew\"]\n\
            reduce_motion = false\nsymbol_drawing_grid = 1\nsymbol_last_kind = \"coil\"\nsymbol_snap = false\n\
            symbols_recent = [\"library:l:s\"]\nsymbols_scope = \"mine\"\nterminal_spacing = 4\ntheme = \"dark\"\n\
            tips = false\ntips_seen = [\"wheel\"]\nui_scale = 1.25\nupdate_check = true\nversion = 1\n\
            wheel_scrolls = true\nworkspace = \"PLC\"\n\n[parts]\nfavourites = [\"a/1\"]\nrecent = [\"b/2\"]\n\n\
            [properties]\ncollapsed = [\"device/identity\"]\nshow_advanced = true\n\n[[templates]]\n\
            dir = \"/w/t.ew\"\nname = \"Company\"\n\n[window]\nheight = 600\nmaximized = false\nwidth = 800\nx = 10\n\
            y = 20\n"
        );
        assert_eq!(Settings::parse(&written), Some(full));
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

    /// The author name round-trips, is trimmed, and is written only once set (an untouched file keeps its
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
            templates: vec![TemplateEntry::new("company.ew".to_owned(), None)],
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
        // The density and the high-contrast themes round-trip; an unknown density falls back.
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
        // Dutch is kept; a language this version does not ship loads as English.
        assert_eq!(Settings::parse("language = \"nl\"\n").unwrap().language, "nl");
        assert_eq!(Settings::parse("language = \"de\"\n").unwrap().language, "en");
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

    /// An earlier settings file names one template path: it loads as a list of that one, still used for
    /// new projects, and the template leaves the recent projects; a list round-trips with its names.
    #[test]
    fn one_template_path_becomes_a_list_of_one() {
        let old = Settings::parse(
            "basic_project = \"/w/company.ew\"\nrecent_projects = [\"/w/job.ew\", \"/w/company.ew\"]\n",
        )
        .unwrap();
        assert_eq!(
            old.templates,
            [TemplateEntry {
                dir: "/w/company.ew".to_owned(),
                name: "company.ew".to_owned()
            }]
        );
        assert_eq!(old.basic_project.as_deref(), Some("/w/company.ew"));
        assert_eq!(old.recent_projects, ["/w/job.ew"]);
        let mut s = old.clone();
        s.add_template("/w/panel.ew", Some("Panel standard".to_owned()));
        let back = Settings::parse(&s.to_toml()).unwrap();
        assert_eq!(back.templates, s.templates);
        assert_eq!(back.templates[1].name, "Panel standard");
        // Taken out of the list, the template is no longer used for new projects.
        let mut s = back;
        s.remove_template(0);
        assert_eq!(s.basic_project, None);
        assert!(!s.is_template("/w/company.ew"));
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

    /// 250 % and 300 % are scales, each with its label, and `settings.toml` round-trips both; an unknown
    /// scale falls back to one.
    #[test]
    fn the_scales_reach_300_percent_and_round_trip() {
        for scale in [2.5, 3.0] {
            assert!(scale_index(scale).is_some(), "{scale}");
            let s = Settings {
                ui_scale: scale,
                ..Settings::default()
            };
            assert_eq!(Settings::parse(&s.to_toml()).unwrap().ui_scale, scale);
        }
        let labels: Vec<String> = UI_SCALES.iter().map(|s| scale_label(*s)).collect();
        assert_eq!(labels, ["100 %", "125 %", "150 %", "200 %", "250 %", "300 %"]);
        assert_eq!(Settings::parse("ui_scale = 1.1\n").unwrap().ui_scale, 1.0);
        assert_eq!(Settings::parse("ui_scale = 2\n").unwrap().ui_scale, 2.0);
    }

    /// `min_font_pt` (0 off, else 8 to 16; out of range loads as 0) and `reduce_motion` (unset, on or
    /// off) round-trip and are written only once set.
    #[test]
    fn the_minimum_text_size_and_reduce_motion_round_trip() {
        let text = Settings::default().to_toml();
        assert!(
            !text.contains("min_font_pt") && !text.contains("reduce_motion"),
            "{text}"
        );
        for (pt, motion) in [(8, Some(true)), (14, Some(false)), (16, None)] {
            let s = Settings {
                min_font_pt: pt,
                reduce_motion: motion,
                ..Settings::default()
            };
            let back = Settings::parse(&s.to_toml()).unwrap();
            assert_eq!((back.min_font_pt, back.reduce_motion), (pt, motion));
        }
        for out_of_range in [
            "min_font_pt = 7\n",
            "min_font_pt = 17\n",
            "min_font_pt = -1\n",
            "min_font_pt = 300\n",
        ] {
            assert_eq!(
                Settings::parse(out_of_range).unwrap().min_font_pt,
                0,
                "{out_of_range}"
            );
        }
        assert_eq!(
            Settings::parse("min_font_pt = \"14\"\n"),
            None,
            "a known key of the wrong type"
        );
        // A float of an integral value loads as that size, any other as off, and the
        // file's other settings are kept.
        for (text, pt) in [("14.0", 14), ("14.5", 0), ("17.0", 0), ("nan", 0), ("inf", 0)] {
            let s = Settings::parse(&format!(
                "min_font_pt = {text}\nlanguage = \"nl\"\nui_scale = 1.5\n"
            ))
            .unwrap_or_else(|| panic!("{text} refuses the file"));
            assert_eq!(
                (s.min_font_pt, s.language.as_str(), s.ui_scale),
                (pt, "nl", 1.5),
                "{text}"
            );
        }
        assert_eq!(
            Settings::parse("reduce_motion = 1\n"),
            None,
            "a known key of the wrong type"
        );
    }

    /// DK6: `[window]` is written only once a window recorded it, read back equal, and a table this version
    /// cannot read is dropped without making the file bad; members of the table this version does not know are kept.
    #[test]
    fn the_window_table() {
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

    /// Two instances' settings on one file. The later exit merges only its window geometry into the file on
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
