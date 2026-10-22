// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The application settings file `settings.toml` (WP-S2.01 part 1, spec section 5.1; FR-UI-033, FR-UI-049).
//!
//! `version = 1`, `recent_projects` (at most [`MAX_RECENT`], most recent first, no duplicates) and `ui_scale` (one
//! of [`UI_SCALES`]). Unknown keys are kept and written back; a `toml::Table` holds them sorted, so the file is
//! deterministic. An unreadable file yields the defaults and is copied to `settings.toml.bad-<n>`.

use std::path::{Path, PathBuf};

/// The recent-projects list holds at most this many paths.
pub const MAX_RECENT: usize = 10;

/// The UI scales the user can choose, as factors on the native scale.
pub const UI_SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

/// The file name inside the settings directory.
pub const FILE_NAME: &str = "settings.toml";

/// The settings.
#[derive(Clone, PartialEq, Debug)]
pub struct Settings {
    /// The recent projects, most recent first (paths as text; a path that no longer exists stays until it is
    /// opened and fails).
    pub recent_projects: Vec<String>,
    /// The UI scale, one of [`UI_SCALES`].
    pub ui_scale: f32,
    /// Keys this version does not know, kept and written back.
    extra: toml::Table,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            recent_projects: Vec::new(),
            ui_scale: 1.0,
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
        let mut settings = Settings {
            recent_projects: Vec::new(),
            ui_scale: if scale_index(ui_scale).is_some() {
                ui_scale
            } else {
                1.0
            },
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
    fn an_unknown_scale_falls_back_to_one() {
        let s = Settings::parse("ui_scale = 1.1\n").unwrap();
        assert_eq!(s.ui_scale, 1.0);
        let s = Settings::parse("ui_scale = 2\n").unwrap();
        assert_eq!(s.ui_scale, 2.0);
    }
}
