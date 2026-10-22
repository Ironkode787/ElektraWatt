// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application settings file `settings.toml` (WP-S2.01 part 1, spec section 5.1; FR-UI-033, FR-UI-049).
//!
//! `version = 1`, `recent_projects` (at most [`MAX_RECENT`], most recent first, no duplicates), `ui_scale` (one
//! of [`UI_SCALES`]) and, since part 2, the first-run answers and quick switches: `first_run_done`, `language`,
//! `keymap`, `theme`, `comes_from`, `update_check` (an opt-in, off by default, NFR-PRIV-03), the default basic
//! project `basic_project` (FR-PRJ-009) and the active workspace `workspace` (FR-UI-002). Unknown keys are kept
//! and written back; a `toml::Table` holds them sorted, so the file is deterministic. An unreadable file yields
//! the defaults and is copied to `settings.toml.bad-<n>`.

use std::path::{Path, PathBuf};

/// The recent-projects list holds at most this many paths.
pub const MAX_RECENT: usize = 10;

/// The UI scales the user can choose, as factors on the native scale.
pub const UI_SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// The file name inside the settings directory.
pub const FILE_NAME: &str = "settings.toml";

/// The UI languages of this version (WP-S2.08 adds the others).
pub const LANGUAGES: [&str; 1] = ["en"];

/// The keymap presets the settings can name. The EW preset is the only one with bindings today; the KiCad-like
/// preset is recorded and takes effect with the keymap files of WP-S2.08 (doc 16 section 4.4).
pub const KEYMAPS: [&str; 2] = ["ew", "kicad"];

/// The themes (egui's own light and dark visuals).
pub const THEMES: [&str; 3] = ["system", "light", "dark"];

/// What the user comes from (first-run step 2, doc 16 section 9.1); the empty text is "not answered".
pub const ORIGINS: [&str; 4] = ["eplan", "qet", "kicad", "other"];

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
    /// What the user comes from, one of [`ORIGINS`], or empty.
    pub comes_from: String,
    /// The update-check opt-in (off by default; this version has no update check, NFR-PRIV-03).
    pub update_check: bool,
    /// The default basic project (a project directory), if the user made or chose one (FR-PRJ-009).
    pub basic_project: Option<String>,
    /// The active workspace (doc 16 section 2.6).
    pub workspace: String,
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
            comes_from: String::new(),
            update_check: false,
            basic_project: None,
            workspace: DEFAULT_WORKSPACE.to_owned(),
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
        let comes_from = match take_string(&mut table, "comes_from")? {
            Some(v) if ORIGINS.contains(&v.as_str()) => v,
            _ => String::new(),
        };
        let basic_project = take_string(&mut table, "basic_project")?.filter(|v| !v.is_empty());
        let workspace = take_string(&mut table, "workspace")?
            .filter(|v| valid_workspace_name(v))
            .unwrap_or_else(|| DEFAULT_WORKSPACE.to_owned());
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
            comes_from,
            update_check,
            basic_project,
            workspace,
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
            ("comes_from", &self.comes_from),
            ("workspace", &self.workspace),
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
        std::fs::create_dir_all(dir)?;
        let path = dir.join(FILE_NAME);
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        std::fs::write(&tmp, self.to_toml())?;
        std::fs::rename(&tmp, &path)
    }
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

/// Whether `name` can be a workspace name: 1 to 40 characters, letters, digits, space, `-` and `_`, no leading
/// or trailing space. The name is the file name of `workspaces/<name>.json`.
pub fn valid_workspace_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 40
        && name == name.trim()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == ' ' || c == '-' || c == '_')
}

/// Copies the bad settings file to `settings.toml.bad-<n>` (the lowest free `n`); the name, or `None` when the
/// copy failed.
fn copy_aside(path: &Path) -> Option<String> {
    let dir = path.parent()?;
    for n in 1..=999 {
        let name = format!("{FILE_NAME}.bad-{n}");
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
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ew-settings-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
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

    #[test]
    fn a_corrupted_file_gives_defaults_and_a_numbered_copy() {
        let dir = temp_dir("bad");
        std::fs::write(dir.join(FILE_NAME), "this is = not [ toml").unwrap();
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.bad_copy.as_deref(), Some("settings.toml.bad-1"));
        assert!(dir.join("settings.toml.bad-1").is_file());
        // The next bad file takes the next free number.
        let again = Settings::load(&dir);
        assert_eq!(again.bad_copy.as_deref(), Some("settings.toml.bad-2"));
        // A well-formed file with a wrong type for a known key is bad too.
        std::fs::write(dir.join(FILE_NAME), "recent_projects = 3\n").unwrap();
        assert_eq!(
            Settings::load(&dir).bad_copy.as_deref(),
            Some("settings.toml.bad-3")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_absent_file_gives_defaults_without_a_copy() {
        let dir = temp_dir("absent");
        let loaded = Settings::load(&dir);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.bad_copy, None);
        let _ = std::fs::remove_dir_all(&dir);
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
        for good in ["Schematic", "My layout", "a-b_c", "Räume 2"] {
            assert!(valid_workspace_name(good), "{good}");
        }
        for bad in ["", " a", "a ", "a/b", "a\\b", "..", "a.b", &"x".repeat(41)] {
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
}
