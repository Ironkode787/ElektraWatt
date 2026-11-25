// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check [-p <package>]... [--no-test] [--deep] [-- <test filter>...]`: the one command that checks a change.
//!
//! In this order, each step printed with its result: rustfmt over every Rust file (it formats, it does not only
//! compare), clippy with warnings as errors, the tests and the doctests, with `--deep` the tests marked for the deep
//! run (`#[ignore]`d; the private corpus is read from `EW_PRIVATE_CORPUS` when it is set), the harness's briefs when
//! the change touches them, and the repository rules. The scope of clippy and the tests is the packages named with
//! `-p`, or the whole workspace; all features are on, so the pixel snapshots of `ew-ui` run too. A failed step does
//! not stop the later ones.
//!
//! **What the paths ask.** A change of the persona harness (`tools/blind/`), the content (`content/`) or `xtask` is
//! checked on the product: the step `briefs` builds `elektrawatt` and `ew`, writes the action list of the reference
//! (`target/reference/actions.txt`) and replays every brief of `tools/blind/tasks/` (`check-brief`), and fails where a
//! program, the list or a hard item is missing. The paths a change touches are those that differ from `main` (since
//! the branch left it) and those not yet committed. A change of a text file needs nothing more: the rule
//! `translations` runs with the rules.
//!
//! **The tree it passed on.** A check that passes writes down the tree it passed on (its object ID, of the working tree
//! with what is not committed) and its scope, in `ew-check-passed` of the repository's git folder, which every worktree
//! shares. A check of a tree and a scope that passed already (the whole workspace covers a package, `--deep` covers
//! one without) says so and runs nothing: a gate does not run again what a lane ran on the same tree.
//!
//! Clippy is its own pass, not the workspace wrapper of the test build: a build through `clippy-driver` is another
//! artefact set, which no plain `cargo build` or `cargo nextest run` of the loop reuses, so the compile it saved in
//! `check` came back in the next plain build (tried on `ew-drafting`: 11 to 21 s saved in `check`, 5 to 13 s more in
//! the `cargo build -p ew` after it, and every workspace crate's artefacts on disk twice).
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
    /// The tests marked for the deep run as well.
    deep: bool,
    /// The test filter after `--`, handed to nextest as it is; empty runs every test.
    filter: Vec<String>,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut out = Args {
        packages: Vec::new(),
        test: true,
        deep: false,
        filter: Vec::new(),
    };
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "-p" | "--package" => out
                .packages
                .push(rest.next().context("-p needs a package name")?.clone()),
            "--no-test" => out.test = false,
            "--deep" => out.deep = true,
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
    if !out.test && out.deep {
        bail!("check: --no-test runs no test, so it takes no --deep");
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
    let mut passed = Passed::of(&args);
    if let Some(passed) = passed.as_mut()
        && passed.recorded(root)?
    {
        println!(
            "check: this tree passed `{}` already (tree {}); nothing runs",
            passed.scope, passed.tree
        );
        return Ok(true);
    }
    let mut summary: Vec<(&str, bool)> = Vec::new();

    println!("==> fmt");
    summary.push(("fmt", format(root)?));

    println!("==> clippy");
    summary.push(("clippy", clippy(root, &scope)?));

    if args.test {
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
        if args.deep {
            println!("==> deep: the tests marked for the deep run");
            summary.push((
                "deep",
                cargo(
                    root,
                    &["nextest", "run", "--all-features", "--run-ignored", "only"],
                    &tests_scope,
                )?,
            ));
        }
        let touched = touched_paths(root)?;
        if let Some(path) = touched.iter().find(|p| asks_for_briefs(p)) {
            println!(
                "==> briefs: `{path}` and the other paths of the change ask for the replay of every brief"
            );
            summary.push(("briefs", briefs(root)?));
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
    let ok = summary.iter().all(|(_, passed)| *passed);
    if ok && let Some(passed) = passed {
        let tree = passed.write(root)?;
        println!("  the tree it passed on: {tree}");
    }
    Ok(ok)
}

/// Whether a change of `path` asks for the replay of the harness's briefs on the product.
fn asks_for_briefs(path: &str) -> bool {
    ["tools/blind/", "content/", "xtask/"]
        .iter()
        .any(|p| path.starts_with(p))
}

/// The paths a change touches: those that differ from where the branch left `main` (none on `main` itself, or without
/// one), and those not yet committed.
fn touched_paths(root: &Path) -> Result<Vec<String>> {
    let git = |args: &[&str]| crate::repo::git(root, args);
    let mut out: Vec<String> = Vec::new();
    if let Ok(base) = git(&["merge-base", "HEAD", "main"]) {
        out.extend(
            git(&["diff", "--name-only", base.trim(), "HEAD"])?
                .lines()
                .map(str::to_owned),
        );
    }
    for line in git(&["status", "--porcelain", "--untracked-files=all"])?.lines() {
        // `XY path` or `XY old -> new`.
        if let Some(path) = line.get(3..) {
            out.push(path.rsplit(" -> ").next().unwrap_or(path).to_owned());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// The step `briefs`: the programs built, the action list written, and every brief of `tools/blind/tasks/` checked
/// and replayed on them; `false`, with what failed, on a build that fails, a list that is not written, or a brief that
/// is not runnable or leaves a hard item unmet.
fn briefs(root: &Path) -> Result<bool> {
    if !cargo(
        root,
        &["build", "--all-features"],
        &["-p".into(), "elektrawatt".into(), "-p".into(), "ew".into()],
    )? {
        eprintln!("briefs: elektrawatt and ew do not build");
        return Ok(false);
    }
    if !crate::generate::write_actions(root)? {
        eprintln!("briefs: the action list of the reference is not written");
        return Ok(false);
    }
    let bin = root.join("target/debug");
    let mut tasks: Vec<String> = std::fs::read_dir(root.join("tools/blind/tasks"))
        .context("reading tools/blind/tasks")?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("brief-"))
        .collect();
    tasks.sort();
    let tmp = std::env::temp_dir().join(format!("ew-check-briefs-{}", std::process::id()));
    let mut ok = true;
    for task in &tasks {
        let mut problems = crate::blind::check_task::check_task(root, task)?;
        let c = crate::blind::brief::check_brief_in(root, task, &bin, &tmp)?;
        problems.extend(c.problems.iter().cloned());
        let unmet: Vec<&str> = c
            .items
            .iter()
            .filter(|(_, kind, decision)| kind == "hard" && decision != "met")
            .map(|(item, _, _)| item.as_str())
            .collect();
        if !problems.is_empty() || !c.runnable() || !unmet.is_empty() {
            ok = false;
            eprintln!("briefs: `{task}` is not proven on this build:\n{}", c.text(task));
            for p in &problems {
                eprintln!("  - {p}");
            }
        } else {
            println!("briefs: `{task}`: every hard item met");
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(ok)
}

/// The tree a check passes on and the scope it was run with, as `ew-check-passed` records them.
struct Passed {
    /// `-p a -p b`, or `--workspace`; with ` --deep` when it ran the deep tests.
    scope: String,
    /// The object ID of the tree, filled in by [`Passed::recorded`].
    tree: String,
}

impl Passed {
    /// The record a check of `args` would write; none for a narrowed check or one without tests, which proves less
    /// than its scope.
    fn of(args: &Args) -> Option<Passed> {
        if !args.test || !args.filter.is_empty() {
            return None;
        }
        let mut packages = args.packages.clone();
        packages.sort();
        packages.dedup();
        let mut scope = if packages.is_empty() {
            "--workspace".to_owned()
        } else {
            packages
                .iter()
                .map(|p| format!("-p {p}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        if args.deep {
            scope.push_str(" --deep");
        }
        Some(Passed {
            scope,
            tree: String::new(),
        })
    }

    /// Whether this tree passed a check whose scope covers this one; fills in the tree.
    fn recorded(&mut self, root: &Path) -> Result<bool> {
        self.tree = working_tree(root)?;
        let text = std::fs::read_to_string(record_path(root)?).unwrap_or_default();
        let (wanted, wanted_deep) = scope_of(&self.scope);
        Ok(text.lines().any(|line| {
            let Some((tree, scope)) = line.split_once(' ') else {
                return false;
            };
            let (have, deep) = scope_of(scope);
            tree == self.tree
                && (deep || !wanted_deep)
                && match (&have, &wanted) {
                    (None, _) => true,
                    (Some(_), None) => false,
                    (Some(have), Some(wanted)) => wanted.is_subset(have),
                }
        }))
    }

    /// Writes this check down for the tree it leaves (after the formatter), and returns that tree.
    fn write(&self, root: &Path) -> Result<String> {
        let tree = working_tree(root)?;
        let path = record_path(root)?;
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        text.push_str(&format!("{tree} {}\n", self.scope));
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(tree)
    }
}

/// The packages of a recorded scope (`None` for the whole workspace) and whether it ran the deep tests.
fn scope_of(scope: &str) -> (Option<std::collections::BTreeSet<String>>, bool) {
    let deep = scope.ends_with(" --deep");
    let scope = scope.trim_end_matches(" --deep");
    if scope == "--workspace" {
        return (None, deep);
    }
    let packages = scope
        .split_whitespace()
        .filter(|w| *w != "-p")
        .map(str::to_owned)
        .collect();
    (Some(packages), deep)
}

/// `ew-check-passed` in the git folder that every worktree of the repository shares.
fn record_path(root: &Path) -> Result<std::path::PathBuf> {
    let common = crate::repo::git(root, &["rev-parse", "--git-common-dir"])?;
    let common = std::path::PathBuf::from(common.trim());
    let common = if common.is_absolute() {
        common
    } else {
        root.join(common)
    };
    Ok(common.join("ew-check-passed"))
}

/// The object ID of the working tree: what is committed with what is not, read through an index of its own, so the
/// repository's own index is not touched.
fn working_tree(root: &Path) -> Result<String> {
    let index = std::env::temp_dir().join(format!("ew-check-index-{}", std::process::id()));
    let _ = std::fs::remove_file(&index);
    let run = |args: &[&str]| -> Result<String> {
        let out = Command::new("git")
            .current_dir(root)
            .env("GIT_INDEX_FILE", &index)
            .args(args)
            .output()
            .with_context(|| format!("running git {}", args.join(" ")))?;
        if !out.status.success() {
            bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    run(&["read-tree", "HEAD"])?;
    run(&["add", "-A"])?;
    let tree = run(&["write-tree"]);
    let _ = std::fs::remove_file(&index);
    tree
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
        assert!(parse(&args(&["--deep"])).unwrap().deep);
        assert!(parse(&args(&["--no-test", "--deep"])).is_err());
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
