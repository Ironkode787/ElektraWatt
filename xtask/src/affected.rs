// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `affected [--base <rev>]`: the workspace crates a change touches and the targeted verify commands for them.
//!
//! For the between-wave and fix-round verifies only; a cycle or milestone gate still runs the full suite
//! (`cargo xtask ci`, CLAUDE.md "Testing policy").
//!
//! - **Changed files.** `git diff --name-only <merge-base>` (committed and uncommitted changes of tracked files
//!   since the merge base of `--base`, default `main`, and `HEAD`) plus the untracked files that git does not
//!   ignore.
//! - **Changed crates.** A file inside a member's directory belongs to that member (the deepest one). A file
//!   that no test reads (`docs/`, `.github/`, `.claude/`, `LICENSES/`, a Markdown file at the root) touches no
//!   crate; `ci/` is the policy data of `xtask`; any other file outside the members (`Cargo.toml`, `Cargo.lock`,
//!   `.cargo/`, `.config/`, `rust-toolchain.toml`, `clippy.toml`, `golden/`, `content/`, `assets/`, …) may change
//!   every build or test, so it selects the whole workspace.
//! - **Reverse dependencies.** From `cargo metadata --all-features`: the members whose library depends on a
//!   changed member through normal or build edges, transitively (their code changed with it), and then the
//!   members with a dev edge to any of those (their tests changed). A dev edge does not carry on: a crate that
//!   depends on a dev-dependent crate is not affected.
//! - **Features.** A test or binary target with `required-features` (ew-viewmodel's `jobs` tests need
//!   `testkit`) is skipped by nextest without them, so the nextest command enables them per package.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// The default base revision.
const DEFAULT_BASE: &str = "main";

/// Top-level paths that no build or test reads.
const INERT: &[&str] = &["docs/", ".github/", ".claude/", "LICENSES/"];

/// Path prefix of the policy data that only `xtask` reads.
const XTASK_DATA: &str = "ci/";

/// One workspace member as the selection needs it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Member {
    /// Directory relative to the workspace root, `/`-separated, without a trailing slash (`crates/ew-model`).
    pub dir: String,
    /// Members this one's library or build script depends on (normal and build edges).
    pub deps: BTreeSet<String>,
    /// Members only its tests, examples and benches depend on (dev edges).
    pub dev_deps: BTreeSet<String>,
    /// The `required-features` of its test and binary targets.
    pub test_features: BTreeSet<String>,
}

/// The workspace members by package name.
pub(crate) type Members = BTreeMap<String, Member>;

/// Why the selection holds what it holds.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    /// Members with a changed file.
    pub changed: BTreeSet<String>,
    /// Changed files outside every member that select the whole workspace.
    pub workspace_wide: Vec<String>,
    /// Every member to verify: the changed ones and their reverse dependencies.
    pub affected: BTreeSet<String>,
}

/// The member a changed file belongs to, `Some("")`-free: `Ok(Some(name))`, `Ok(None)` for an inert file, and
/// `Err(())` for a file that selects the whole workspace.
fn owner<'m>(file: &str, members: &'m Members) -> Result<Option<&'m str>, ()> {
    let deepest = members
        .iter()
        .filter(|(_, m)| {
            file.strip_prefix(m.dir.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|(_, m)| m.dir.len())
        .map(|(name, _)| name.as_str());
    if let Some(name) = deepest {
        return Ok(Some(name));
    }
    if INERT.iter().any(|p| file.starts_with(p)) || (!file.contains('/') && file.ends_with(".md")) {
        return Ok(None);
    }
    if file.starts_with(XTASK_DATA) && members.contains_key("xtask") {
        return Ok(Some("xtask"));
    }
    Err(())
}

/// Selects the members to verify for the changed files.
pub(crate) fn select(files: &[String], members: &Members) -> Selection {
    let mut selection = Selection::default();
    for file in files {
        match owner(file, members) {
            Ok(Some(name)) => {
                selection.changed.insert(name.to_owned());
            }
            Ok(None) => {}
            Err(()) => selection.workspace_wide.push(file.clone()),
        }
    }
    if !selection.workspace_wide.is_empty() {
        selection.affected = members.keys().cloned().collect();
        return selection;
    }
    // The libraries that changed with the changed members: closure over normal and build edges.
    let mut libs = selection.changed.clone();
    loop {
        let more: Vec<String> = members
            .iter()
            .filter(|(name, m)| !libs.contains(*name) && m.deps.iter().any(|d| libs.contains(d)))
            .map(|(name, _)| name.clone())
            .collect();
        if more.is_empty() {
            break;
        }
        libs.extend(more);
    }
    // Their test-only dependants.
    let dev: Vec<String> = members
        .iter()
        .filter(|(name, m)| !libs.contains(*name) && m.dev_deps.iter().any(|d| libs.contains(d)))
        .map(|(name, _)| name.clone())
        .collect();
    libs.extend(dev);
    selection.affected = libs;
    selection
}

/// The verify commands for the selection: nextest with the test features, and clippy as CLAUDE.md runs it.
pub(crate) fn commands(selection: &Selection, members: &Members) -> Vec<String> {
    if selection.affected.is_empty() {
        return Vec::new();
    }
    let everything = selection.affected.len() == members.len();
    let packages = if everything {
        "--workspace".to_owned()
    } else {
        selection
            .affected
            .iter()
            .map(|p| format!("-p {p}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let features: Vec<String> = selection
        .affected
        .iter()
        .flat_map(|p| members[p].test_features.iter().map(move |f| format!("{p}/{f}")))
        .collect();
    let features = if features.is_empty() {
        String::new()
    } else {
        format!(" --features {}", features.join(","))
    };
    vec![
        format!("cargo nextest run {packages}{features}"),
        format!("cargo clippy {packages} --all-targets --all-features -- -D warnings"),
    ]
}

/// The members of the workspace at `root`, from `cargo metadata --all-features`.
fn members_of(metadata: &cargo_metadata::Metadata) -> Result<Members> {
    use cargo_metadata::DependencyKind;
    let root = metadata.workspace_root.as_std_path();
    let ids: BTreeSet<&cargo_metadata::PackageId> = metadata.workspace_members.iter().collect();
    let name_of: BTreeMap<&cargo_metadata::PackageId, String> = metadata
        .packages
        .iter()
        .filter(|p| ids.contains(&p.id))
        .map(|p| (&p.id, p.name.to_string()))
        .collect();
    let resolve = metadata
        .resolve
        .as_ref()
        .context("`cargo metadata` gave no dependency graph")?;
    let mut members = Members::new();
    for package in metadata.packages.iter().filter(|p| ids.contains(&p.id)) {
        let dir = package
            .manifest_path
            .parent()
            .context("a manifest path has a parent")?
            .as_std_path();
        let dir = dir
            .strip_prefix(root)
            .with_context(|| format!("{} is outside the workspace", dir.display()))?;
        let dir = dir
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let test_features = package
            .targets
            .iter()
            .filter(|t| t.is_test() || t.is_bin())
            .flat_map(|t| t.required_features.iter().cloned())
            .collect();
        let mut member = Member {
            dir,
            test_features,
            ..Member::default()
        };
        let node = resolve
            .nodes
            .iter()
            .find(|n| n.id == package.id)
            .with_context(|| format!("{} is not in the dependency graph", package.name))?;
        for dep in &node.deps {
            let Some(name) = name_of.get(&dep.pkg) else {
                continue;
            };
            for kind in &dep.dep_kinds {
                if kind.kind == DependencyKind::Development {
                    member.dev_deps.insert(name.clone());
                } else {
                    member.deps.insert(name.clone());
                }
            }
        }
        members.insert(package.name.to_string(), member);
    }
    Ok(members)
}

/// Runs git in `root` and returns its standard output as lines.
fn git_lines(root: &Path, args: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

/// `affected [--base <rev>]`.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    let base = match args {
        [] => DEFAULT_BASE,
        [flag, rev] if flag == "--base" => rev.as_str(),
        _ => bail!("usage: cargo xtask affected [--base <rev>]"),
    };
    let merge_base = git_lines(root, &["merge-base", base, "HEAD"])?
        .into_iter()
        .next()
        .with_context(|| format!("no merge base of `{base}` and HEAD"))?;
    let mut files = git_lines(root, &["diff", "--name-only", &merge_base])?;
    files.extend(git_lines(root, &["ls-files", "--others", "--exclude-standard"])?);
    files.sort();
    files.dedup();

    let members = members_of(&crate::metadata_all_features(root)?)?;
    let selection = select(&files, &members);
    let short = &merge_base[..merge_base.len().min(12)];
    println!(
        "affected: {} changed file(s) since {base} (merge base {short})",
        files.len()
    );
    if !selection.workspace_wide.is_empty() {
        println!(
            "affected: the whole workspace, because of {}",
            selection.workspace_wide.join(", ")
        );
    } else {
        println!("affected: changed crates: {}", join_or_none(&selection.changed));
        println!(
            "affected: to verify ({} crates, with reverse dependencies): {}",
            selection.affected.len(),
            join_or_none(&selection.affected)
        );
    }
    for command in commands(&selection, &members) {
        println!("{command}");
    }
    Ok(true)
}

fn join_or_none(names: &BTreeSet<String>) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.iter().cloned().collect::<Vec<_>>().join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    /// model <- commands <- api; api dev-depends on testkit; testkit depends on api; viewmodel depends on api and
    /// needs `testkit` for its tests; derive dev-depends on model; xtask stands alone.
    fn members() -> Members {
        let m = |dir: &str, deps: &[&str], dev: &[&str], features: &[&str]| Member {
            dir: dir.to_owned(),
            deps: set(deps),
            dev_deps: set(dev),
            test_features: set(features),
        };
        [
            ("ew-model", m("crates/ew-model", &[], &[], &[])),
            ("ew-commands", m("crates/ew-commands", &["ew-model"], &[], &[])),
            (
                "ew-api",
                m("crates/ew-api", &["ew-commands"], &["ew-testkit"], &[]),
            ),
            ("ew-testkit", m("crates/ew-testkit", &["ew-api"], &[], &[])),
            (
                "ew-viewmodel",
                m("crates/ew-viewmodel", &["ew-api"], &[], &["testkit"]),
            ),
            ("ew-derive", m("crates/ew-derive", &[], &["ew-model"], &[])),
            ("ew-leaf", m("crates/ew-leaf", &[], &["ew-derive"], &[])),
            ("xtask", m("xtask", &[], &[], &[])),
        ]
        .into_iter()
        .map(|(n, m)| (n.to_owned(), m))
        .collect()
    }

    fn files(f: &[&str]) -> Vec<String> {
        f.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_leaf_change_selects_only_that_crate() {
        let s = select(&files(&["crates/ew-viewmodel/src/lib.rs"]), &members());
        assert_eq!(s.changed, set(&["ew-viewmodel"]));
        assert_eq!(s.affected, set(&["ew-viewmodel"]));
        assert!(s.workspace_wide.is_empty());
        assert_eq!(
            commands(&s, &members()),
            [
                "cargo nextest run -p ew-viewmodel --features ew-viewmodel/testkit",
                "cargo clippy -p ew-viewmodel --all-targets --all-features -- -D warnings",
            ]
        );
    }

    #[test]
    fn a_core_change_selects_its_reverse_dependencies_through_normal_and_dev_edges() {
        let s = select(&files(&["crates/ew-model/src/table.rs"]), &members());
        assert_eq!(s.changed, set(&["ew-model"]));
        // ew-derive dev-depends on ew-model: its tests are affected; ew-leaf dev-depends on ew-derive, whose
        // library did not change: not affected.
        assert_eq!(
            s.affected,
            set(&[
                "ew-model",
                "ew-commands",
                "ew-api",
                "ew-testkit",
                "ew-viewmodel",
                "ew-derive"
            ])
        );
        assert_eq!(
            commands(&s, &members())[0],
            "cargo nextest run -p ew-api -p ew-commands -p ew-derive -p ew-model -p ew-testkit -p ew-viewmodel \
             --features ew-viewmodel/testkit"
        );
    }

    #[test]
    fn a_dev_dependency_change_selects_the_dependant_tests() {
        // ew-testkit changed: ew-api's tests use it (dev edge), and nothing depends on ew-api through it.
        let s = select(&files(&["crates/ew-testkit/src/lib.rs"]), &members());
        assert_eq!(s.affected, set(&["ew-testkit", "ew-api"]));
    }

    #[test]
    fn inert_files_select_nothing_and_ci_data_selects_xtask() {
        let s = select(
            &files(&[
                "docs/plan/01-global-plan.md",
                ".github/workflows/ci.yml",
                "README.md",
                ".claude/x.json",
            ]),
            &members(),
        );
        assert!(s.affected.is_empty() && s.workspace_wide.is_empty(), "{s:?}");
        assert!(commands(&s, &members()).is_empty());
        let s = select(&files(&["ci/dep-rules.toml"]), &members());
        assert_eq!(s.affected, set(&["xtask"]));
    }

    #[test]
    fn a_file_outside_the_members_selects_the_workspace() {
        let s = select(
            &files(&[
                "crates/ew-model/src/lib.rs",
                "Cargo.lock",
                "golden/J-E-2416/x.csv",
            ]),
            &members(),
        );
        assert_eq!(s.workspace_wide, files(&["Cargo.lock", "golden/J-E-2416/x.csv"]));
        assert_eq!(s.affected.len(), members().len());
        assert_eq!(
            commands(&s, &members()),
            [
                "cargo nextest run --workspace --features ew-viewmodel/testkit",
                "cargo clippy --workspace --all-targets --all-features -- -D warnings",
            ]
        );
    }

    #[test]
    fn a_file_belongs_to_the_deepest_member_and_not_to_a_name_prefix() {
        let mut m = members();
        m.insert(
            "ew-model-extra".to_owned(),
            Member {
                dir: "crates/ew-model-extra".to_owned(),
                ..Member::default()
            },
        );
        m.insert(
            "nested".to_owned(),
            Member {
                dir: "crates/ew-model/nested".to_owned(),
                ..Member::default()
            },
        );
        assert_eq!(
            owner("crates/ew-model-extra/src/lib.rs", &m),
            Ok(Some("ew-model-extra"))
        );
        assert_eq!(owner("crates/ew-model/nested/src/lib.rs", &m), Ok(Some("nested")));
        assert_eq!(owner("crates/ew-model/Cargo.toml", &m), Ok(Some("ew-model")));
    }

    #[test]
    fn the_workspace_metadata_gives_the_viewmodel_test_features() {
        let metadata = crate::metadata_all_features(&crate::workspace_root()).unwrap();
        let members = members_of(&metadata).unwrap();
        assert_eq!(members["ew-viewmodel"].test_features, set(&["testkit"]));
        assert_eq!(members["ew-viewmodel"].dir, "crates/ew-viewmodel");
        assert!(
            members["ew-api"].deps.contains("ew-model"),
            "{:?}",
            members["ew-api"]
        );
        assert!(members.contains_key("xtask"));
    }
}
