// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The golden project as bytes: the base of the loader's and the open path's inputs and of their seed corpora.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The golden project directory below the repository root.
const GOLDEN: &str = "golden/J-E-2416.ew";

/// The repository root: the nearest ancestor of this crate's manifest directory that holds the golden
/// project (the crate sits one level below it, in `fuzz/`).
pub fn repo() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find(|dir| dir.join(GOLDEN).is_dir())
        .unwrap_or(manifest)
        .to_path_buf()
}

fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display())) {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            walk(&path, base, out);
        } else {
            let rel = path
                .strip_prefix(base)
                .expect("below the base")
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read(&path).expect("a readable file"));
        }
    }
}

/// Every file of the golden project, by relative path with `/`, `manifest.json` included.
pub fn files() -> &'static BTreeMap<String, Vec<u8>> {
    static FILES: OnceLock<BTreeMap<String, Vec<u8>>> = OnceLock::new();
    FILES.get_or_init(|| {
        let dir = repo().join(GOLDEN);
        let mut out = BTreeMap::new();
        walk(&dir, &dir, &mut out);
        assert!(!out.is_empty(), "the golden project {} is missing", dir.display());
        out
    })
}

/// The paths of the golden project's files except the manifest, in path order: what a file selector of an
/// input indexes.
pub fn paths() -> &'static [String] {
    static PATHS: OnceLock<Vec<String>> = OnceLock::new();
    PATHS.get_or_init(|| {
        files()
            .keys()
            .filter(|p| *p != "manifest.json")
            .cloned()
            .collect()
    })
}

/// The golden project as the loader reads it (its records and library documents), read once: the base of the sessions
/// of the WP-S6.04 targets.
fn loaded() -> &'static ew_format::Loaded {
    static LOADED: OnceLock<ew_format::Loaded> = OnceLock::new();
    LOADED.get_or_init(|| {
        let limits = ew_format::Limits::default();
        let manifest = ew_format::Manifest::parse(&files()["manifest.json"], &limits).ok();
        let files: BTreeMap<ew_format::FilePath, Vec<u8>> = paths()
            .iter()
            .map(|p| {
                (
                    ew_format::FilePath::new(p).expect("a project path"),
                    files()[p].clone(),
                )
            })
            .collect();
        ew_format::load(manifest.as_ref(), &files, &limits)
    })
}

/// A new in-memory session of the golden project: its model from the loader's records (no store, no disk), the IDs it
/// makes from `seed` (deterministic), no memo.
pub fn session(seed: u64) -> ew_api::Session {
    let loaded = loaded();
    ew_api::Session::from_records(
        loaded
            .project
            .clone()
            .expect("the golden project has its project record"),
        loaded.records.clone(),
        loaded.lib.clone(),
        ew_api::SessionOptions {
            ids: Some(Box::new(ew_testkit::synth::SynthSource::new(seed))),
            memo: ew_api::MemoMode::Off,
            ..ew_api::SessionOptions::default()
        },
    )
    .expect("the golden project passes the model's integrity check")
}

/// The position of `path` in [`paths`] (for the seed corpora).
pub fn index_of(path: &str) -> u8 {
    let at = paths()
        .iter()
        .position(|p| p == path)
        .unwrap_or_else(|| panic!("the golden project has no file {path}"));
    u8::try_from(at).expect("fewer than 256 golden files")
}
