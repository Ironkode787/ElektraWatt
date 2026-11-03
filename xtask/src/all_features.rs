// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `all-features-set`: the workspace packages whose all-features test run covers more than the default run (FT-1132,
//! ID-1217).
//!
//! The default run (`cargo nextest run --workspace`) builds every package with the features that cargo resolves for
//! it, a dev edge's included (a test build); the all-features run adds the features it leaves off. A package with a
//! feature that is off in the default run is in the set: only there does `--all-features` change a package's own
//! code, so the all-features run (CI's `test-linux`, `cargo xtask ci`, `cargo xtask affected`) needs those packages
//! alone. The set is computed from the manifests' features and the edges that switch them on, never kept by hand.
//! `cargo xtask all-features-set` prints it as `-p <package>` arguments for CI, and nothing when it is empty.

use anyhow::{Context, Result};
use std::collections::BTreeSet;

/// The workspace packages that declare a feature which the resolved default graph of `metadata` (no
/// `--all-features`) leaves off.
pub(crate) fn set(metadata: &cargo_metadata::Metadata) -> Result<BTreeSet<String>> {
    let resolve = metadata
        .resolve
        .as_ref()
        .context("`cargo metadata` gave no dependency graph")?;
    let mut out = BTreeSet::new();
    for package in metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
    {
        let node = resolve
            .nodes
            .iter()
            .find(|n| n.id == package.id)
            .with_context(|| format!("{} is not in the dependency graph", package.name))?;
        if package
            .features
            .keys()
            .any(|declared| !node.features.iter().any(|on| on.as_str() == declared.as_str()))
        {
            out.insert(package.name.to_string());
        }
    }
    Ok(out)
}

/// The set of the workspace, from `cargo metadata --locked`.
pub(crate) fn workspace_set() -> Result<BTreeSet<String>> {
    set(&crate::metadata()?)
}

/// The set as `-p` arguments, in package order: `-p`, `a`, `-p`, `b`, or none.
pub(crate) fn argument_list(set: &BTreeSet<String>) -> Vec<String> {
    set.iter().flat_map(|p| ["-p".to_owned(), p.clone()]).collect()
}

/// The set as one line of `-p` arguments: `-p a -p b`, or the empty string.
pub(crate) fn arguments(set: &BTreeSet<String>) -> String {
    argument_list(set).join(" ")
}

/// `cargo xtask all-features-set`: prints the `-p` arguments (a line, or nothing when the set is empty).
pub(crate) fn run() -> Result<bool> {
    let arguments = arguments(&workspace_set()?);
    if !arguments.is_empty() {
        println!("{arguments}");
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace of path packages in a fresh folder: each `(name, manifest tail)` is a member; removed on drop.
    struct Fixture {
        dir: std::path::PathBuf,
    }

    impl Fixture {
        fn new(tag: &str, members: &[(&str, &str)]) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "ew-xtask-{tag}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let names: Vec<String> = members.iter().map(|(n, _)| format!("\"{n}\"")).collect();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("Cargo.toml"),
                format!(
                    "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
                    names.join(", ")
                ),
            )
            .unwrap();
            for (name, tail) in members {
                std::fs::create_dir_all(dir.join(name).join("src")).unwrap();
                std::fs::write(dir.join(name).join("src/lib.rs"), "").unwrap();
                std::fs::write(
                    dir.join(name).join("Cargo.toml"),
                    format!(
                        "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\n{tail}\n"
                    ),
                )
                .unwrap();
            }
            Self { dir }
        }

        fn set(&self) -> Vec<String> {
            let metadata = cargo_metadata::MetadataCommand::new()
                .cargo_path(crate::cargo())
                .manifest_path(self.dir.join("Cargo.toml"))
                .current_dir(&self.dir)
                .exec()
                .unwrap();
            set(&metadata).unwrap().into_iter().collect()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.dir).ok();
        }
    }

    /// FT-1132: a feature that only a dev edge switches on counts as on, a feature that a normal edge or `default`
    /// switches on too, and one that nothing switches on is off; a package without features is never in the set.
    #[test]
    fn a_feature_that_nothing_switches_on_puts_its_package_in_the_set() {
        let f = Fixture::new(
            "all-features-set",
            &[
                ("dev_on", "[features]\nhook = []\n"),
                (
                    "dev_user",
                    "[dev-dependencies]\ndev_on = { path = \"../dev_on\", features = [\"hook\"] }\n",
                ),
                ("normal_on", "[features]\nhook = []\n"),
                (
                    "normal_user",
                    "[dependencies]\nnormal_on = { path = \"../normal_on\", features = [\"hook\"] }\n",
                ),
                ("default_on", "[features]\ndefault = [\"hook\"]\nhook = []\n"),
                ("never_on", "[features]\nhook = []\n"),
                (
                    "half_on",
                    "[features]\ndefault = [\"hook\"]\nhook = []\nextra = []\n",
                ),
                ("plain", ""),
            ],
        );
        assert_eq!(f.set(), ["half_on", "never_on"]);
    }

    /// FT-1132: `-p` arguments in package order, nothing for an empty set.
    #[test]
    fn the_set_prints_as_package_arguments() {
        let set: BTreeSet<String> = ["ew-ui", "a-b"].into_iter().map(str::to_owned).collect();
        assert_eq!(arguments(&set), "-p a-b -p ew-ui");
        assert_eq!(arguments(&BTreeSet::new()), "");
    }

    /// FT-1132: on this workspace `ew-ui` (`snapshots`, which nothing switches on) is in the set, and the packages
    /// whose only features the dev or normal edges switch on (`testkit`, `raw-read`) are not; membership only, the
    /// set grows.
    #[test]
    fn the_workspace_set_holds_ew_ui_and_not_the_packages_whose_features_are_on() {
        let set = workspace_set().unwrap();
        assert!(set.contains("ew-ui"), "{set:?}");
        for on in ["ew-commands", "ew-model", "ew-viewmodel"] {
            assert!(
                !set.contains(on),
                "{on} has no feature that the default run leaves off: {set:?}"
            );
        }
    }
}
