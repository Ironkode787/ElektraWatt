// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The four library tiers and where they live (WP-S1.19 D4; doc 07 §2.1; doc 05 §1, §13.1).

use std::path::{Path, PathBuf};

/// A library tier (doc 07 §2.1), in search order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Tier {
    /// The installation: the compiled-in starter `ew-starter` and `content/libraries/*.ewlib`. Read-only.
    System,
    /// Installed packs, `<data dir>/packs/*.ewlib`. Read-only.
    Community,
    /// The directories of the settings key `libraries.company`, in order. Writable.
    Company,
    /// `<data dir>/libraries/*.ewlib`. Writable.
    User,
}

impl Tier {
    /// Whether the registry accepts writes to libraries of this tier (D4).
    pub fn writable(self) -> bool {
        matches!(self, Tier::Company | Tier::User)
    }

    /// The tier's name: `system`, `community`, `company`, `user`.
    pub fn name(self) -> &'static str {
        match self {
            Tier::System => "system",
            Tier::Community => "community",
            Tier::Company => "company",
            Tier::User => "user",
        }
    }

    /// The tier of a name of [`Tier::name`].
    pub fn from_name(name: &str) -> Option<Tier> {
        Some(match name {
            "system" => Tier::System,
            "community" => Tier::Community,
            "company" => Tier::Company,
            "user" => Tier::User,
            _ => return None,
        })
    }
}

/// Where the tiers live (WP-S1.19 §5.2).
///
/// Every library path names either a library directory itself (it holds `library.json`) or a directory whose
/// `*.ewlib` children are libraries, read in byte order of their names. The compiled-in starter is always the first
/// system library.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LibraryTiers {
    /// System library paths, typically `<installation>/content/libraries`.
    pub system: Vec<PathBuf>,
    /// The community packs, `<data dir>/packs`.
    pub community: PathBuf,
    /// The company libraries of the settings key `libraries.company`, in order.
    pub company: Vec<PathBuf>,
    /// The user libraries, `<data dir>/libraries`.
    pub user: PathBuf,
    /// The cache directory that holds the search index (and nothing else of this crate).
    pub cache: PathBuf,
}

impl LibraryTiers {
    /// The tiers under a data directory and a cache directory: `<data>/packs`, `<data>/libraries`, `cache`.
    pub fn under(data: &Path, cache: &Path, system: Vec<PathBuf>, company: Vec<PathBuf>) -> LibraryTiers {
        LibraryTiers {
            system,
            community: data.join("packs"),
            company,
            user: data.join("libraries"),
            cache: cache.to_path_buf(),
        }
    }

    /// The tiers of this user: [`default_data_dir`] and [`default_cache_dir`]; `None` when the environment names
    /// neither directory.
    pub fn for_user(system: Vec<PathBuf>, company: Vec<PathBuf>) -> Option<LibraryTiers> {
        Some(LibraryTiers::under(
            &default_data_dir()?,
            &default_cache_dir()?,
            system,
            company,
        ))
    }
}

/// An environment variable as a path, `None` when unset or empty.
// Program boundary: the data and cache directories are read once when a front end builds its tiers.
#[allow(clippy::disallowed_methods)]
fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The OS data directory of doc 05 §1 (user libraries and community packs): `EW_DATA_DIR` when set, else
/// `%LOCALAPPDATA%\ElektraWatt\data` on Windows, `~/Library/Application Support/ElektraWatt/data` on macOS and
/// `$XDG_DATA_HOME/elektrawatt` (else `~/.local/share/elektrawatt`) elsewhere; `None` when the environment names
/// none.
pub fn default_data_dir() -> Option<PathBuf> {
    if let Some(dir) = env_path("EW_DATA_DIR") {
        return Some(dir);
    }
    if cfg!(windows) {
        env_path("LOCALAPPDATA").map(|p| p.join("ElektraWatt").join("data"))
    } else if cfg!(target_os = "macos") {
        env_path("HOME").map(|p| p.join("Library/Application Support/ElektraWatt/data"))
    } else {
        env_path("XDG_DATA_HOME")
            .map(|p| p.join("elektrawatt"))
            .or_else(|| env_path("HOME").map(|p| p.join(".local/share/elektrawatt")))
    }
}

/// The OS cache directory of doc 05 §1 (the derived search index): `EW_CACHE_DIR` when set, else
/// `%LOCALAPPDATA%\ElektraWatt\cache` on Windows, `~/Library/Caches/ElektraWatt` on macOS and
/// `$XDG_CACHE_HOME/elektrawatt` (else `~/.cache/elektrawatt`) elsewhere; `None` when the environment names none.
pub fn default_cache_dir() -> Option<PathBuf> {
    if let Some(dir) = env_path("EW_CACHE_DIR") {
        return Some(dir);
    }
    if cfg!(windows) {
        env_path("LOCALAPPDATA").map(|p| p.join("ElektraWatt").join("cache"))
    } else if cfg!(target_os = "macos") {
        env_path("HOME").map(|p| p.join("Library/Caches/ElektraWatt"))
    } else {
        env_path("XDG_CACHE_HOME")
            .map(|p| p.join("elektrawatt"))
            .or_else(|| env_path("HOME").map(|p| p.join(".cache/elektrawatt")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_company_and_user_tiers_are_writable() {
        assert!(!Tier::System.writable());
        assert!(!Tier::Community.writable());
        assert!(Tier::Company.writable());
        assert!(Tier::User.writable());
        for t in [Tier::System, Tier::Community, Tier::Company, Tier::User] {
            assert_eq!(Tier::from_name(t.name()), Some(t));
        }
        assert!(Tier::System < Tier::User, "search order");
    }

    #[test]
    fn tiers_under_a_data_directory() {
        let t = LibraryTiers::under(Path::new("d"), Path::new("c"), vec![], vec![PathBuf::from("x")]);
        assert_eq!(t.community, Path::new("d").join("packs"));
        assert_eq!(t.user, Path::new("d").join("libraries"));
        assert_eq!(t.cache, Path::new("c"));
        assert_eq!(t.company, [PathBuf::from("x")]);
    }
}
