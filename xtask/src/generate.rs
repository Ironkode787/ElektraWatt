// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gen [--check]`: every generated file of the repository, written by one command in one order.
//!
//! Two kinds of output:
//! - **The reference** (`target/reference/`, never committed): what the product's registries say about themselves
//!   as JSON, for the user guide's reference pages. xtask may not link the product (it is permissive, the product
//!   GPL), so each registry's own crate has a test that writes its file when `EW_REFERENCE_DIR` is set, and
//!   [`reference`] runs those tests.
//! - **Committed files** that a generator writes from data in the tree: the workspace-hack crate, the book's
//!   contents and reference pages, the maps of the actions and the crates (`docs/map/`), the icon table, the lint list, the QET symbol edition and the sample project's folder. `--check` compares the
//!   ones that need no build. The guide's translation catalogue is not among them: `cargo xtask book-po` makes it,
//!   when the guide is translated and before a release.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

/// Where the reference is written, relative to the root.
pub(crate) const REFERENCE_DIR: &str = "target/reference";

/// The test that writes `actions.txt`, the persona harness's action list; `check` of the whole workspace runs it
/// alone before the tests.
const ACTIONS_TEST: (&str, &[&str]) = (
    "ew-viewmodel",
    &["--test", "it", "blind_reference::the_action_list"],
);

/// The tests that write the reference: the package, then the arguments that choose the test target and the test.
const REFERENCE: &[(&str, &[&str])] = &[
    // commands/: one parameter schema per command and index.json
    ("ew-commands", &["--test", "it", "schemas::"]),
    // rules.json
    ("ew-checks", &["--test", "it", "catalogue::"]),
    // keymap-sheet.json
    ("ew-viewmodel", &["--test", "it", "keymaps::the_keymap_sheet"]),
    // properties.json
    (
        "ew-viewmodel",
        &["--test", "it", "help_reference::the_properties_reference"],
    ),
    // actions.txt: the persona harness's action list
    ACTIONS_TEST,
    // cli.json
    ("ew", &["--bin", "ew", "catalogue::tests::the_cli_reference"]),
    // results/: one schema per `--json` result and index.json
    ("ew", &["--test", "it", "conventions::the_result_schemas"]),
    // api_v1.json
    ("ew-sqlview", &["--test", "it", "api_v1::the_api_v1_reference"]),
    // queries.json
    ("ew-api", &["--test", "it", "queries::the_query_list"]),
];

/// A generator of xtask: `(root, check) -> up to date or written`.
type Own = fn(&Path, bool) -> Result<bool>;

/// The generators of xtask that write a committed file from data in the tree, in their order.
const OWN: &[(&str, Own)] = &[
    ("gen-maps", maps),
    ("gen-icons", crate::icons::run),
    // The rule `raw-read` compares the list; here it is only written.
    ("lint-list", |root, check| {
        if check {
            Ok(true)
        } else {
            crate::raw_read::run(root, true)
        }
    }),
    ("symbols-qet10", crate::symbols::run),
    // Comparing needs a build: the test itself compares the folder; here it is only written.
    ("sample", sample),
    // Not here: the guide's translation catalogue (`cargo xtask book-po`). It is made when the guide is translated
    // and before a release, not with every change of a page: made here, every change of a page rewrote the
    // template and the translations in every branch, and they met at every merge.
];

/// The test that writes the sample project's folder `fixtures/sample/sample.ew` from its UI script when `EW_BLESS=1`
/// is set: the package, then the arguments that choose the test target and the test.
const SAMPLE_TEST: (&str, &[&str]) = (
    "ew-viewmodel",
    &[
        "--test",
        "it",
        "sample::the_committed_sample_is_what_its_script_makes",
    ],
);

/// `sample`: runs [`SAMPLE_TEST`] with `EW_BLESS=1`, which writes the folder. In check mode it is up to date by
/// definition: comparing needs a build, and the test compares it.
fn sample(root: &Path, check: bool) -> Result<bool> {
    if check {
        return Ok(true);
    }
    test_with(root, ("EW_BLESS", "1".as_ref()), SAMPLE_TEST.0, SAMPLE_TEST.1)
}

/// `gen-maps [--check]`: the action map and the crate map under `docs/map/`, read from the source.
pub(crate) fn maps(root: &Path, check: bool) -> Result<bool> {
    if !check {
        crate::action_map::write(root)?;
        crate::crate_map::write(root)?;
        println!(
            "gen-maps: wrote {} and {}",
            crate::action_map::OUTPUT,
            crate::crate_map::OUTPUT
        );
        return Ok(true);
    }
    let mut problems = crate::action_map::violations(root)?;
    problems.extend(crate::crate_map::violations(root)?);
    if problems.is_empty() {
        println!("gen-maps: ok");
    }
    for p in &problems {
        eprintln!("  - {p}");
    }
    Ok(problems.is_empty())
}

/// `gen [--check]`.
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    if check {
        return check_own(root);
    }
    println!("==> gen: workspace-hack");
    let mut ok = workspace_hack(root)?;
    println!("==> gen: reference");
    ok &= reference(root)?;
    println!("==> gen: docs-gen");
    ok &= crate::docs_gen::run(root, false)?;
    for (name, step) in OWN {
        println!("==> gen: {name}");
        ok &= step(root, false)?;
    }
    let changed = crate::repo::git(root, &["status", "--porcelain"])?;
    if changed.trim().is_empty() {
        println!("gen: nothing changed");
    } else {
        println!(
            "gen: the working tree now differs from HEAD in:\n{}",
            changed.trim_end()
        );
    }
    Ok(ok)
}

/// The generators of xtask in check mode: `true` when every file they write is up to date. The book's contents are
/// left out: comparing them needs the reference, and so a build; `check` of the whole workspace compares them
/// (`docs-gen --check`).
pub(crate) fn check_own(root: &Path) -> Result<bool> {
    let mut ok = true;
    for (name, step) in OWN {
        if !step(root, true)? {
            eprintln!("gen: {name} is stale; run `cargo xtask gen`");
            ok = false;
        }
    }
    Ok(ok)
}

/// Writes the reference under [`REFERENCE_DIR`]: runs each test of [`REFERENCE`] with `EW_REFERENCE_DIR` set. Plain
/// `cargo test`, so that it also runs where nextest is not installed (the release's book build).
pub(crate) fn reference(root: &Path) -> Result<bool> {
    let dir: PathBuf = root.join(REFERENCE_DIR);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    build_reference_tests(root)?;
    for (package, args) in REFERENCE {
        if !reference_test(root, &dir, package, args)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Writes `actions.txt` alone, over the one there is: the harness's replay of its briefs reads it.
pub(crate) fn write_actions(root: &Path) -> Result<bool> {
    let dir: PathBuf = root.join(REFERENCE_DIR);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    reference_test(root, &dir, ACTIONS_TEST.0, ACTIONS_TEST.1)
}

/// Runs one test of [`REFERENCE`] with `EW_REFERENCE_DIR` set to `dir`.
fn reference_test(root: &Path, dir: &Path, package: &str, args: &[&str]) -> Result<bool> {
    test_with(root, ("EW_REFERENCE_DIR", dir.as_os_str()), package, args)
}

/// Runs the test that `args` choose in `package` (plain `cargo test`) with the environment variable `env` set.
fn test_with(root: &Path, env: (&str, &std::ffi::OsStr), package: &str, args: &[&str]) -> Result<bool> {
    let (target, test) = args.split_at(2);
    let status = Command::new(crate::cargo())
        .current_dir(root)
        .env(env.0, env.1)
        .args(["test", "--quiet", "--all-features", "-p", package])
        .args(target)
        .arg("--")
        .args(test)
        .status()
        .with_context(|| format!("running the test of {package}"))?;
    if !status.success() {
        eprintln!("gen: the test of {package} ({}) FAILED", args.join(" "));
    }
    Ok(status.success())
}

/// Builds the test binaries of [`REFERENCE`] in one cargo invocation, so that they compile and link side by side.
/// The runs that follow then find them built: run one after the other on a tree that changed, each waited for its
/// own crate's compile and link in turn, which was most of the time `gen` took. A failure here is left to the
/// runs, which report it against their package.
fn build_reference_tests(root: &Path) -> Result<()> {
    let mut packages: Vec<&str> = REFERENCE.iter().map(|(p, _)| *p).collect();
    packages.sort_unstable();
    packages.dedup();
    let mut command = Command::new(crate::cargo());
    command
        .current_dir(root)
        .args(["test", "--quiet", "--no-run", "--all-features"]);
    for package in packages {
        command.args(["-p", package]);
    }
    command
        .args(["--test", "it", "--bin", "ew"])
        .status()
        .context("building the reference tests")?;
    Ok(())
}

/// The workspace-hack crate `ew-hack` (`.config/hakari.toml`), which gives every `-p` build the same features of the
/// third-party crates, so that changing the package set rebuilds none of them. Skipped with a note where
/// cargo-hakari is not installed: a stale hack only costs build time.
fn workspace_hack(root: &Path) -> Result<bool> {
    let installed = Command::new(crate::cargo())
        .current_dir(root)
        .args(["hakari", "--version"])
        .output()
        .is_ok_and(|o| o.status.success());
    if !installed {
        println!("gen: cargo-hakari is not installed; ew-hack is left as it is");
        return Ok(true);
    }
    for args in [
        &["hakari", "generate"][..],
        &["hakari", "manage-deps", "--yes"][..],
    ] {
        let status = Command::new(crate::cargo())
            .current_dir(root)
            .args(args)
            .status()
            .with_context(|| format!("running cargo {}", args.join(" ")))?;
        if !status.success() {
            return Ok(false);
        }
    }
    Ok(true)
}
