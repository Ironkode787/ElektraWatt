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
//! `cargo xtask all-features-set` prints it as `-p <package>` arguments for CI, and nothing when it is empty. The graph
//! features are those `cargo tree --target <host>` resolves, for the host platform alone, so a feature that only another
//! platform's edge switches on (a `cfg(windows)` edge, for the Linux job) counts as off (FT-1152; `cargo metadata`'s
//! `--filter-platform` drops such an edge but keeps the features it switches on).

use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The workspace packages of `metadata` that declare a feature which `on` (package name to the features the default
/// graph switches on, no `--all-features`) leaves off.
pub(crate) fn set(
    metadata: &cargo_metadata::Metadata,
    on: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .filter(|p| {
            let enabled = on.get(p.name.as_str());
            p.features
                .keys()
                .any(|declared| !enabled.is_some_and(|fs| fs.contains(declared)))
        })
        .map(|p| p.name.to_string())
        .collect()
}

/// The features the default test build switches on for `triple` in the workspace at `root`, per package:
/// `cargo tree --workspace --target <triple> -e normal,build,dev` (with `--locked` when `locked`).
fn enabled(root: &Path, triple: &str, locked: bool) -> Result<BTreeMap<String, BTreeSet<String>>> {
    let mut command = std::process::Command::new(crate::cargo());
    command.current_dir(root).arg("tree");
    if locked {
        command.arg("--locked");
    }
    command.args([
        "--workspace",
        "--target",
        triple,
        "-e",
        "normal,build,dev",
        "--prefix",
        "none",
        "--format",
        "{p}|{f}",
    ]);
    let output = command.output().context("running `cargo tree`")?;
    if !output.status.success() {
        anyhow::bail!(
            "`cargo tree` failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(crate::features::parse_tree(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

/// The set of the workspace for the host platform.
pub(crate) fn workspace_set() -> Result<BTreeSet<String>> {
    let root = crate::workspace_root();
    Ok(set(
        &crate::metadata()?,
        &enabled(&root, &host_triple()?, true)?,
    ))
}

/// The host's target triple: the `host:` line of `rustc -vV` (`RUSTC` when set).
fn host_triple() -> Result<String> {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
    let output = std::process::Command::new(&rustc)
        .arg("-vV")
        .output()
        .with_context(|| format!("running `{rustc} -vV`"))?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(|triple| triple.trim().to_owned())
        .with_context(|| format!("`{rustc} -vV` named no host"))
}

/// The set as `-p` arguments, in package order: `-p`, `a`, `-p`, `b`, or none.
pub(crate) fn argument_list(set: &BTreeSet<String>) -> Vec<String> {
    set.iter().flat_map(|p| ["-p".to_owned(), p.clone()]).collect()
}

/// The set as one line of `-p` arguments: `-p a -p b`, or the empty string.
pub(crate) fn arguments(set: &BTreeSet<String>) -> String {
    argument_list(set).join(" ")
}

/// The part of `set` whose members have a library target with doctests, the packages `cargo test --doc` takes
/// (FT-1155; lane-check's doc step filters the same way).
pub(crate) fn with_doctests(
    set: &BTreeSet<String>,
    members: &crate::affected::Members,
) -> BTreeSet<String> {
    set.iter()
        .filter(|p| members.get(*p).is_some_and(|m| m.doctests))
        .cloned()
        .collect()
}

/// The workspace's set with doctests ([`with_doctests`] of [`workspace_set`]).
pub(crate) fn workspace_doctest_set() -> Result<BTreeSet<String>> {
    let root = crate::workspace_root();
    let members = crate::affected::members_of(&crate::metadata_all_features(&root)?)?;
    Ok(with_doctests(&workspace_set()?, &members))
}

/// `cargo xtask all-features-set [--doctests]`: prints the `-p` arguments (a line, or nothing when the set is empty);
/// `--doctests` keeps the members with a library target with doctests alone.
pub(crate) fn run(args: &[String]) -> Result<bool> {
    let set = match args {
        [] => workspace_set()?,
        [flag] if flag == "--doctests" => workspace_doctest_set()?,
        _ => {
            eprintln!("all-features-set takes no argument but --doctests");
            return Ok(false);
        }
    };
    let arguments = arguments(&set);
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
            self.set_for(&host_triple().unwrap())
        }

        /// The set for `triple`, as [`workspace_set`] computes it (no lock file here, so no `--locked`).
        fn set_for(&self, triple: &str) -> Vec<String> {
            let metadata = cargo_metadata::MetadataCommand::new()
                .cargo_path(crate::cargo())
                .manifest_path(self.dir.join("Cargo.toml"))
                .current_dir(&self.dir)
                .exec()
                .unwrap();
            let on = enabled(&self.dir, triple, false).unwrap();
            set(&metadata, &on).into_iter().collect()
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

    /// FT-1152: a feature that only a `cfg(windows)` edge switches on counts as off in a Linux graph and as on in a
    /// Windows one.
    #[test]
    fn a_feature_that_only_another_platforms_edge_switches_on_counts_as_off() {
        let f = Fixture::new(
            "all-features-platform",
            &[
                ("win_on", "[features]\nhook = []\n"),
                (
                    "win_user",
                    "[target.'cfg(windows)'.dependencies]\nwin_on = { path = \"../win_on\", features = [\"hook\"] }\n",
                ),
            ],
        );
        assert_eq!(f.set_for("x86_64-unknown-linux-gnu"), ["win_on"]);
        assert_eq!(f.set_for("x86_64-pc-windows-msvc"), Vec::<String>::new());
    }

    /// FT-1155: the doctest part keeps the members with a library target with doctests and drops a binary-only one.
    #[test]
    fn the_doctest_part_drops_the_members_without_a_library() {
        let set: BTreeSet<String> = ["a-lib", "a-bin", "ew-ui"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let member = |doctests| crate::affected::Member {
            doctests,
            ..Default::default()
        };
        let members: crate::affected::Members = [
            ("a-lib".to_owned(), member(true)),
            ("a-bin".to_owned(), member(false)),
            ("ew-ui".to_owned(), member(true)),
        ]
        .into_iter()
        .collect();
        let part: Vec<String> = with_doctests(&set, &members).into_iter().collect();
        assert_eq!(part, ["a-lib", "ew-ui"]);
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
