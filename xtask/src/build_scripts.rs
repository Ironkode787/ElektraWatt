// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-build-scripts`: every package in the dependency graph (workspace or external) that has a
//! `custom-build` target (a `build.rs`) must be listed in `ci/build-scripts.allowlist` (ADR-0023 decision 1,
//! NFR-SEC-10). The graph is the all-features metadata, so a build script behind a non-default feature is
//! seen too. The allowlist holds one crate name per line; `#` starts a comment.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result};

/// Reads the allowlist file.
pub fn load_allowlist(path: &Path) -> Result<BTreeSet<String>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(parse_allowlist(&text))
}

/// Parses allowlist text: one name per line, `#` comments, blank lines ignored.
pub fn parse_allowlist(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Names of every package in the metadata (all platforms) that has a build script.
pub fn packages_with_build_script(metadata: &cargo_metadata::Metadata) -> BTreeSet<String> {
    metadata
        .packages
        .iter()
        .filter(|p| {
            p.targets
                .iter()
                .any(|t| t.kind.contains(&cargo_metadata::TargetKind::CustomBuild))
        })
        .map(|p| p.name.to_string())
        .collect()
}

/// Returns one violation per package with a build script that is not allowlisted.
pub fn check(allowlist: &BTreeSet<String>, with_build_script: &BTreeSet<String>) -> Vec<String> {
    with_build_script
        .difference(allowlist)
        .map(|name| format!("package `{name}` has a build script but is not in ci/build-scripts.allowlist"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_ignores_comments() {
        let list = parse_allowlist("# header\n\nserde  # derive support\n  anyhow\n#gone\n");
        assert_eq!(list, BTreeSet::from(["serde".to_owned(), "anyhow".to_owned()]));
    }

    #[test]
    fn unlisted_build_script_fails() {
        let allow = parse_allowlist("serde\n");
        let found = BTreeSet::from(["serde".to_owned(), "downloader-sys".to_owned()]);
        let v = check(&allow, &found);
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("`downloader-sys`"), "{v:?}");
    }

    #[test]
    fn listed_build_scripts_pass() {
        let allow = parse_allowlist("serde\nanyhow\nunused-entry\n");
        let found = BTreeSet::from(["serde".to_owned(), "anyhow".to_owned()]);
        assert!(check(&allow, &found).is_empty());
    }

    /// The real dependency graph (all features) matches the committed allowlist.
    #[test]
    fn real_workspace_passes() {
        let root = crate::workspace_root();
        let allow = load_allowlist(&root.join("ci/build-scripts.allowlist")).expect("allowlist loads");
        let found = packages_with_build_script(
            &crate::metadata_all_features(&root).expect("cargo metadata --all-features runs"),
        );
        assert_eq!(check(&allow, &found), Vec::<String>::new());
    }
}
