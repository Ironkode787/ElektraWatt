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

/// Runs `git <args>` in `root` and returns its standard output as bytes (for `-z` listings).
pub(crate) fn git_output(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
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
    Ok(output.stdout)
}

/// The paths of git's `-z` output (NUL-terminated, verbatim).
pub(crate) fn nul_paths(stdout: &[u8]) -> Vec<String> {
    stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// The merge base of `base` and `HEAD` in the repository at `root` (a full commit hash).
pub(crate) fn merge_base(root: &Path, base: &str) -> Result<String> {
    git(root, &["merge-base", base, "HEAD"])?
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_owned)
        .with_context(|| format!("no merge base of `{base}` and HEAD"))
}

/// `text` with the lines between the `begin` and `end` markers replaced by `body`.
pub(crate) fn replace_block(text: &str, begin: &str, end: &str, body: &str, path: &str) -> Result<String> {
    let (Some(b), Some(e)) = (text.find(begin), text.find(end)) else {
        bail!("{path} lacks the markers {begin} and {end}");
    };
    if e < b {
        bail!("{path}: {end} comes before {begin}");
    }
    Ok(format!("{}\n{body}{}", &text[..b + begin.len()], &text[e..]))
}

/// Writes `text` to `path` through a temporary file in the same folder, renamed over it; on failure the old file
/// stays and the temporary file is removed.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let name = path
        .file_name()
        .with_context(|| format!("{} names no file", path.display()))?;
    let tmp = path.with_file_name(format!("{}.xtask-tmp", name.to_string_lossy()));
    let result = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    result.with_context(|| format!("writing {}", path.display()))
}

/// Splits one CSV record (RFC 4180 quoting, no line breaks inside a field).
pub(crate) fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields
}
