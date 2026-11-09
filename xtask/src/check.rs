// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check [-p <package>]... [--no-test]`: the one command that checks a change.
//!
//! In this order, each step printed with its result: rustfmt over every Rust file (it formats, it does not only
//! compare), clippy with warnings as errors, the tests and the doctests, and the repository rules. The scope of
//! clippy and the tests is the packages named with `-p`, or the whole workspace; all features are on, so the pixel
//! snapshots of `ew-ui` run too. A failed step does not stop the later ones.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// rustfmt takes this many files per call (the Windows command line is limited).
const RUSTFMT_CHUNK: usize = 100;

/// What clippy prints for a `disallowed-methods` or `disallowed-types` path that names nothing: only a warning,
/// which `-D warnings` does not turn into an error, so the generated `clippy.toml` would guard nothing.
const UNRESOLVED_CLIPPY_PATH: &str = "does not refer to a reachable";

struct Args {
    packages: Vec<String>,
    test: bool,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut out = Args {
        packages: Vec::new(),
        test: true,
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-p" | "--package" => out
                .packages
                .push(rest.next().context("-p needs a package name")?.clone()),
            "--no-test" => out.test = false,
            other => bail!("check: unknown argument `{other}`"),
        }
    }
    Ok(out)
}

/// `check`.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    let args = parse(args)?;
    let scope: Vec<String> = if args.packages.is_empty() {
        vec!["--workspace".to_owned()]
    } else {
        args.packages
            .iter()
            .flat_map(|p| ["-p".to_owned(), p.clone()])
            .collect()
    };
    let mut summary: Vec<(&str, bool)> = Vec::new();

    println!("==> fmt");
    summary.push(("fmt", format(root)?));

    println!("==> clippy");
    summary.push(("clippy", clippy(root, &scope)?));

    if args.test {
        println!("==> tests");
        summary.push((
            "tests",
            cargo(root, &["nextest", "run", "--all-features"], &scope)?,
        ));
        let with_lib = doctest_scope(&args.packages)?;
        if !with_lib.is_empty() {
            println!("==> doctests");
            summary.push((
                "doctests",
                cargo(root, &["test", "--doc", "--all-features", "--quiet"], &with_lib)?,
            ));
        }
    }

    println!("==> rules");
    summary.push(("rules", crate::rules(root)?));

    println!("\nsummary:");
    for (name, passed) in &summary {
        println!("  {name:<10} {}", if *passed { "ok" } else { "FAILED" });
    }
    Ok(summary.iter().all(|(_, passed)| *passed))
}

/// Runs `cargo <head> <scope>` in `root`; whether it succeeded.
fn cargo(root: &Path, head: &[&str], scope: &[String]) -> Result<bool> {
    Ok(Command::new(crate::cargo())
        .current_dir(root)
        .args(head)
        .args(scope)
        .status()
        .with_context(|| format!("starting `cargo {}`", head.join(" ")))?
        .success())
}

/// The doctest scope: the whole workspace, or the named packages that have a library target (cargo refuses
/// `--doc` for a package without one).
fn doctest_scope(packages: &[String]) -> Result<Vec<String>> {
    if packages.is_empty() {
        return Ok(vec!["--workspace".to_owned()]);
    }
    let metadata = crate::metadata()?;
    Ok(metadata
        .workspace_packages()
        .iter()
        .filter(|p| packages.iter().any(|n| n == p.name.as_str()))
        .filter(|p| p.targets.iter().any(|t| t.is_lib() || t.is_proc_macro()))
        .flat_map(|p| ["-p".to_owned(), p.name.to_string()])
        .collect())
}

/// Clippy over the scope, every target and feature, warnings as errors; also fails on a lint-list path that clippy
/// could not resolve.
fn clippy(root: &Path, scope: &[String]) -> Result<bool> {
    let mut command = Command::new(crate::cargo());
    command
        .current_dir(root)
        .arg("clippy")
        .args(scope)
        .args(["--all-targets", "--all-features", "--", "-D", "warnings"])
        .stderr(Stdio::piped());
    let mut child = command.spawn().context("starting `cargo clippy`")?;
    let mut unresolved = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines() {
            let line = line?;
            eprintln!("{line}");
            if line.contains(UNRESOLVED_CLIPPY_PATH) {
                unresolved.push(line);
            }
        }
    }
    let status = child.wait()?;
    for line in &unresolved {
        eprintln!("clippy: clippy.toml names a path that does not resolve: {line}");
    }
    Ok(status.success() && unresolved.is_empty())
}

/// Formats every Rust file of the repository with rustfmt, the test modules that `automod::dir!` declares and
/// `cargo fmt` therefore never sees included. Prints the files it changed.
fn format(root: &Path) -> Result<bool> {
    let files = rust_files(root)?;
    let before = crate::repo::git(root, &["status", "--porcelain", "--", "*.rs"])?;
    for chunk in files.chunks(RUSTFMT_CHUNK) {
        let output = Command::new("rustfmt")
            .args(chunk)
            .current_dir(root)
            .output()
            .context("running rustfmt")?;
        if !output.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim_end());
            return Ok(false);
        }
    }
    let after = crate::repo::git(root, &["status", "--porcelain", "--", "*.rs"])?;
    if before == after {
        println!("fmt: {} files, nothing to format", files.len());
    } else {
        println!(
            "fmt: formatted; the changed Rust files are now:\n{}",
            after.trim_end()
        );
    }
    Ok(true)
}

/// The `.rs` files rustfmt takes: every tracked one and every untracked one git does not ignore, but never a module
/// its parent declares under `#[rustfmt::skip]` (rustfmt given the file itself would not see the attribute).
fn rust_files(root: &Path) -> Result<Vec<String>> {
    let all: Vec<String> = crate::repo::list_files(root)?
        .into_iter()
        .filter(|f| f.ends_with(".rs"))
        .collect();
    let mut skipped = Vec::new();
    for parent in &all {
        if let Ok(text) = std::fs::read_to_string(root.join(parent)) {
            skipped.extend(skipped_modules(parent, &text));
        }
    }
    Ok(all
        .into_iter()
        .filter(|f| {
            !skipped
                .iter()
                .any(|m| f == &format!("{m}.rs") || f.starts_with(&format!("{m}/")))
        })
        .collect())
}

/// The module paths (without `.rs`, relative to the root) that the file `parent` declares under `#[rustfmt::skip]`
/// (`#[rustfmt::skip]` and then, on the next non-empty line, `mod name;` with any visibility): `dir/name` for a
/// parent `dir/lib.rs`, `dir/main.rs` or `dir/mod.rs`, `dir/stem/name` for a parent `dir/stem.rs`.
fn skipped_modules(parent: &str, text: &str) -> Vec<String> {
    let (dir, stem) = match parent.rsplit_once('/') {
        Some((d, f)) => (d.to_owned(), f.trim_end_matches(".rs").to_owned()),
        None => (String::new(), parent.trim_end_matches(".rs").to_owned()),
    };
    let base = if ["lib", "main", "mod"].contains(&stem.as_str()) {
        dir
    } else if dir.is_empty() {
        stem
    } else {
        format!("{dir}/{stem}")
    };
    let mut out = Vec::new();
    let mut armed = false;
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            continue;
        }
        if armed {
            let decl = line
                .split_whitespace()
                .skip_while(|w| w.starts_with("pub"))
                .collect::<Vec<_>>();
            if let ["mod", name] = decl.as_slice()
                && let Some(name) = name.strip_suffix(';')
            {
                out.push(if base.is_empty() {
                    name.to_owned()
                } else {
                    format!("{base}/{name}")
                });
            }
        }
        armed = line == "#[rustfmt::skip]";
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{parse, skipped_modules};

    #[test]
    fn modules_under_rustfmt_skip_are_found() {
        assert_eq!(
            skipped_modules(
                "crates/a/src/lib.rs",
                "#[rustfmt::skip]\npub mod generated;\nmod b;\n"
            ),
            vec!["crates/a/src/generated".to_owned()]
        );
        assert_eq!(
            skipped_modules("crates/a/src/icons.rs", "#[rustfmt::skip]\n\nmod table;\n"),
            vec!["crates/a/src/icons/table".to_owned()]
        );
        assert_eq!(
            skipped_modules("src/lib.rs", "#[rustfmt::skip]\nfn f() {}\nmod a;\n"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn the_arguments_parse() {
        let args = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let parsed = parse(&args(&["-p", "ew-ui", "--no-test", "-p", "ew"])).unwrap();
        assert_eq!(parsed.packages, ["ew-ui", "ew"]);
        assert!(!parsed.test);
        assert!(parse(&args(&["--fast"])).is_err());
        assert!(parse(&args(&["-p"])).is_err());
    }
}
