// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check [-p <package>]... [--no-test] [-- <test filter>...]`: the one command that checks a change.
//!
//! In this order, each step printed with its result: rustfmt over every Rust file (it formats, it does not only
//! compare), clippy with warnings as errors, the tests and the doctests, and the repository rules. Before the tests of
//! the whole workspace the action list of the reference is written (`target/reference/actions.txt`), which the
//! harness's replay of its briefs on the built product reads. The scope of clippy and the tests is the packages named
//! with `-p`, or the whole workspace; all features are on, so the pixel snapshots of `ew-ui` run too. A failed step
//! does not stop the later ones.
//!
//! What follows `--` narrows the tests: it is handed to `cargo nextest run` as its filters (name substrings, or
//! `-E <expression>`), the doctests do not run, and the summary says so. Format, clippy and the rules run in full.

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
    /// The test filter after `--`, handed to nextest as it is; empty runs every test.
    filter: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut out = Args {
        packages: Vec::new(),
        test: true,
        filter: Vec::new(),
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-p" | "--package" => out
                .packages
                .push(rest.next().context("-p needs a package name")?.clone()),
            "--no-test" => out.test = false,
            "--" => {
                out.filter = rest.by_ref().cloned().collect();
                if out.filter.is_empty() {
                    bail!("check: `--` needs a test filter after it");
                }
            }
            other => bail!("check: unknown argument `{other}`"),
        }
    }
    if !out.test && !out.filter.is_empty() {
        bail!("check: --no-test runs no test, so it takes no test filter");
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
        // The harness's replay of its briefs on the built product reads the action list of the reference: written
        // anew, never trusted from an older tree. Only the whole workspace builds the product; the harness's own
        // tests read a small list of their own.
        if args.packages.is_empty() {
            println!(
                "==> actions.txt: written from the registry for the harness's replay (target/reference)"
            );
            if !crate::generate::write_actions(root)? {
                summary.push(("actions", false));
            }
        }
        println!("==> tests");
        let tests_scope: Vec<String> = scope.iter().chain(&args.filter).cloned().collect();
        summary.push((
            "tests",
            cargo(root, &["nextest", "run", "--all-features"], &tests_scope)?,
        ));
        let with_lib = doctest_scope(&args.packages)?;
        if !with_lib.is_empty() && args.filter.is_empty() {
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
    if !args.filter.is_empty() {
        println!(
            "  the tests were narrowed to `{}`; the doctests did not run",
            args.filter.join(" ")
        );
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
///
/// The chunks are compared side by side (`--check`, which writes nothing), and only the files that differ are then
/// written: rustfmt also formats the modules a given file declares, so two chunks written at once could both write
/// one file. One chunk after another took most of a lane's `check` before the build started (25 to 47 s, 6 s side
/// by side).
fn format(root: &Path) -> Result<bool> {
    let files = rust_files(root)?;
    let before = crate::repo::git(root, &["status", "--porcelain", "--", "*.rs"])?;
    let checks = std::thread::scope(|scope| {
        let runs: Vec<_> = files
            .chunks(RUSTFMT_CHUNK)
            .map(|chunk| {
                scope.spawn(move || {
                    Command::new("rustfmt")
                        .args(["--check", "--files-with-diff"])
                        .args(chunk)
                        .current_dir(root)
                        .output()
                })
            })
            .collect();
        runs.into_iter()
            .map(|run| run.join().expect("a rustfmt thread panicked"))
            .collect::<std::io::Result<Vec<_>>>()
    })
    .context("running rustfmt --check")?;
    let mut differ = Vec::new();
    for output in checks {
        // A file that differs is a failed `--check` with its name on stdout; an error also says why on stderr.
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() && !stderr.trim().is_empty() {
            eprintln!("{}", stderr.trim_end());
            return Ok(false);
        }
        differ.extend(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_owned),
        );
    }
    for chunk in differ.chunks(RUSTFMT_CHUNK) {
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
        assert!(parsed.filter.is_empty());
        assert!(parse(&args(&["--fast"])).is_err());
        assert!(parse(&args(&["-p"])).is_err());
        // Everything after `--` is the test filter, as nextest takes it.
        let parsed = parse(&args(&["-p", "ew", "--", "-E", "test(=a) | test(b)", "-p"])).unwrap();
        assert_eq!(parsed.packages, ["ew"]);
        assert_eq!(parsed.filter, ["-E", "test(=a) | test(b)", "-p"]);
        assert!(parse(&args(&["-p", "ew", "--"])).is_err());
        assert!(parse(&args(&["--no-test", "--", "a"])).is_err());
    }
}
