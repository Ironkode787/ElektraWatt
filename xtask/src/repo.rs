// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers for the repository-wide checks: running git and listing the committed files.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Runs `git <args>` in `dir` and returns its standard output.
pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .with_context(|| format!("starting `git {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout)
        .with_context(|| format!("`git {}` printed invalid UTF-8", args.join(" ")))
}

/// The files of the repository at `root`, as `/`-separated paths relative to it, sorted.
///
/// This is `git ls-files` plus the untracked files that are not ignored, so that a local run before
/// `git add` sees new files too; in CI the checkout has no untracked files and the list equals `git ls-files`.
/// Files deleted from the working tree and nested repositories (listed with a trailing `/`) are skipped.
pub fn list_files(root: &Path) -> Result<Vec<String>> {
    let out = git(
        root,
        &["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
    )?;
    let mut files: Vec<String> = out
        .split('\0')
        .filter(|p| !p.is_empty() && !p.ends_with('/'))
        .filter(|p| root.join(p).is_file())
        .map(str::to_owned)
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}
