// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cloud-synced folders (doc 05 §15 rows 7 to 11; WP-S1.17 D25, decision log ID-1284 ruling 10): whether a project lies
//! in a folder that a sync client (OneDrive, Dropbox, iCloud Drive, Google Drive, Box) keeps, from environment
//! variables and paths only, with the standard library.
//!
//! - **OneDrive** on Windows: the folders the variables `OneDrive`, `OneDriveCommercial` and `OneDriveConsumer` name.
//! - **macOS**: the folders below `~/Library/CloudStorage/` (named after their provider, `OneDrive-Personal`,
//!   `Dropbox`, `GoogleDrive-<account>`, `Box-Box`; only the part before the first `-` is read, never the account) and
//!   `~/Library/Mobile Documents/` (iCloud Drive).
//! - **Dropbox**: the `path` of each account in Dropbox's `info.json` (`%APPDATA%\Dropbox\`, `%LOCALAPPDATA%\Dropbox\`,
//!   `~/.dropbox/`), read through the [`Vfs`] seam and capped at [`INFO_JSON_MAX`] bytes.
//!
//! The environment is the caller's: a program reads it at its boundary and passes it in (doc 18 §3.5).
//!
//! A path is inside a folder when it starts with it, component by component, as given (no link is resolved). Online-only
//! placeholders (Files On-Demand, iCloud stubs) are not detected: de-scoped under WP-S1.17 §14.4.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::vfs::Vfs;

/// The most bytes of Dropbox's `info.json` read.
pub const INFO_JSON_MAX: u64 = 64 * 1024;

/// The sync service a cloud-synced folder belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CloudService {
    /// Microsoft OneDrive.
    OneDrive,
    /// Dropbox.
    Dropbox,
    /// Apple iCloud Drive.
    ICloud,
    /// Google Drive.
    GoogleDrive,
    /// Box.
    Box,
    /// Another provider of macOS's `~/Library/CloudStorage/`.
    Other,
}

impl CloudService {
    /// The service's own name, as it names itself (a proper name, the same in every language); `None` for
    /// [`CloudService::Other`].
    pub fn name(self) -> Option<&'static str> {
        match self {
            CloudService::OneDrive => Some("OneDrive"),
            CloudService::Dropbox => Some("Dropbox"),
            CloudService::ICloud => Some("iCloud Drive"),
            CloudService::GoogleDrive => Some("Google Drive"),
            CloudService::Box => Some("Box"),
            CloudService::Other => None,
        }
    }
}

/// The cloud-synced folder `path` lies in, if any, with the environment `env` (a variable's value by name) and the
/// file system `vfs` (for Dropbox's `info.json`).
pub fn detect(vfs: &dyn Vfs, env: &dyn Fn(&str) -> Option<OsString>, path: &Path) -> Option<CloudService> {
    let var = |name: &str| env(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    for name in ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"] {
        if var(name).is_some_and(|root| path.starts_with(&root)) {
            return Some(CloudService::OneDrive);
        }
    }
    if let Some(home) = var("HOME") {
        if let Ok(rest) = path.strip_prefix(home.join("Library").join("CloudStorage")) {
            let folder = rest
                .components()
                .next()?
                .as_os_str()
                .to_string_lossy()
                .into_owned();
            let provider = folder.split('-').next().unwrap_or_default();
            return Some(match provider {
                "OneDrive" => CloudService::OneDrive,
                "Dropbox" => CloudService::Dropbox,
                "GoogleDrive" => CloudService::GoogleDrive,
                "Box" => CloudService::Box,
                "iCloud" | "iCloudDrive" => CloudService::ICloud,
                _ => CloudService::Other,
            });
        }
        if path.starts_with(home.join("Library").join("Mobile Documents")) {
            return Some(CloudService::ICloud);
        }
    }
    let infos = [
        var("APPDATA").map(|d| d.join("Dropbox").join("info.json")),
        var("LOCALAPPDATA").map(|d| d.join("Dropbox").join("info.json")),
        var("HOME").map(|d| d.join(".dropbox").join("info.json")),
    ];
    for info in infos.into_iter().flatten() {
        if dropbox_roots(vfs, &info)
            .iter()
            .any(|root| path.starts_with(root))
        {
            return Some(CloudService::Dropbox);
        }
    }
    None
}

/// The account folders Dropbox's `info.json` at `info` names (`{"personal": {"path": …}, "business": {"path": …}}`);
/// none when it is absent, too large or not that JSON.
fn dropbox_roots(vfs: &dyn Vfs, info: &Path) -> Vec<PathBuf> {
    if vfs.stat(info).map_or(true, |s| s.len > INFO_JSON_MAX) {
        return Vec::new();
    }
    let Ok(bytes) = vfs.read(info) else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Object(accounts)) = serde_json::from_slice(&bytes) else {
        return Vec::new();
    };
    accounts
        .values()
        .filter_map(|a| a.get("path")?.as_str())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mem::{DirSync, MemVfs};

    fn env_of(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |name| {
            vars.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn each_kind_of_cloud_folder_is_found_and_other_folders_are_not() {
        let vfs = MemVfs::new(DirSync::Implicit);
        vfs.put(
            Path::new("/appdata/Dropbox/info.json"),
            br#"{"personal":{"path":"/box/Dropbox","host":1}}"#,
        );
        let env = env_of(&[
            ("OneDrive", "/users/u/OneDrive"),
            ("HOME", "/home/u"),
            ("APPDATA", "/appdata"),
        ]);
        let found = |p: &str| detect(&vfs, &env, Path::new(p));
        assert_eq!(found("/users/u/OneDrive/a/p.ew"), Some(CloudService::OneDrive));
        assert_eq!(found("/users/u/OneDriveX/p.ew"), None);
        assert_eq!(
            found("/home/u/Library/CloudStorage/GoogleDrive-someone/p.ew"),
            Some(CloudService::GoogleDrive)
        );
        assert_eq!(
            found("/home/u/Library/CloudStorage/Other-x/p.ew"),
            Some(CloudService::Other)
        );
        assert_eq!(
            found("/home/u/Library/Mobile Documents/com~apple~CloudDocs/p.ew"),
            Some(CloudService::ICloud)
        );
        assert_eq!(found("/box/Dropbox/work/p.ew"), Some(CloudService::Dropbox));
        assert_eq!(found("/home/u/projects/p.ew"), None);
        assert_eq!(
            detect(&vfs, &env_of(&[]), Path::new("/users/u/OneDrive/p.ew")),
            None
        );
    }
}
