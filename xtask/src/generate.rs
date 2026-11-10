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
//!   contents and reference pages, the icon table, the lint list, the QET symbol edition, and the persona harness's
//!   action list. `--check` compares the ones that need no build.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

/// Where the reference is written, relative to the root.
pub(crate) const REFERENCE_DIR: &str = "target/reference";

/// The tests that write the reference: the package, then the arguments that choose the test target and the test.
const REFERENCE: &[(&str, &[&str])] = &[
    // commands/: one parameter schema per command and index.json
    ("ew-commands", &["--test", "schemas"]),
    // rules.json
    ("ew-checks", &["--test", "catalogue"]),
    // keymap-sheet.json
    ("ew-viewmodel", &["--test", "keymaps", "the_keymap_sheet"]),
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
];

/// `gen [--check]`.
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    if check {
        return check_own(root);
    }
    println!("==> gen: workspace-hack");
    let mut ok = workspace_hack(root)?;
    println!("==> gen: reference");
    ok &= reference(root)?;
    println!("==> gen: blind-reference");
    ok &= blind_reference(root)?;
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
/// left out: comparing them needs the reference, and so a build.
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
    for (package, args) in REFERENCE {
        let (target, test) = args.split_at(2);
        let mut command = Command::new(crate::cargo());
        command
            .current_dir(root)
            .env("EW_REFERENCE_DIR", &dir)
            .args(["test", "--quiet", "--all-features", "-p", package])
            .args(target)
            .arg("--")
            .args(test);
        let status = command
            .status()
            .with_context(|| format!("running the reference test of {package}"))?;
        if !status.success() {
            eprintln!("gen: the reference test of {package} ({}) FAILED", args.join(" "));
            return Ok(false);
        }
    }
    Ok(true)
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

/// The persona harness's committed action list (`tools/blind/reference/actions.txt`), rewritten by its test.
fn blind_reference(root: &Path) -> Result<bool> {
    Ok(Command::new(crate::cargo())
        .current_dir(root)
        .env("EW_BLESS", "1")
        .args([
            "test",
            "--quiet",
            "--all-features",
            "-p",
            "ew-viewmodel",
            "--test",
            "blind_reference",
        ])
        .status()
        .context("running the blind reference test")?
        .success())
}
