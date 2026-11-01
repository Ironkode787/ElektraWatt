// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `lane-check --base <rev> [--only <step>] [--plan] [--warm [-p <pkg>]...]`: a lane's hand-back checks
//! (`.claude/agents/lane.md`, "Hand-back checks"), run once and chosen from the diff against `<rev>`.
//!
//! - **The diff.** The changed files are those of [`affected::changed_files`] since the merge base of `<rev>` and
//!   `HEAD` (committed, uncommitted and untracked). A file belongs to the deepest member whose directory holds it;
//!   `ci/` belongs to `xtask`. A diff with no crate (only `affected`'s inert paths, or only the workspace-wide files
//!   such as `Cargo.lock`, `golden/` or the root `Cargo.toml`) skips the cargo steps: a workspace-wide file reaches
//!   every crate, and the whole workspace is the merge point's verify, not a lane's.
//! - **The steps,** in this order:
//!   1. `fmt`: `cargo fmt` over the changed crates. It writes; it is not a check.
//!   2. `clippy`: the changed crates, `--all-targets --all-features -- -D warnings`. An unresolvable
//!      `disallowed-methods` path in clippy's output fails the step as in `cargo xtask ci`.
//!   3. `doc`: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` over the changed crates, in one call.
//!      `--all-features`, as every cargo command of a lane, so the dependencies' check builds are clippy's.
//!   4. `nextest`: one run of the `lane` profile with `--all-features` over the nextest set (below), with the
//!      `required-features` of each package's test targets named as `affected::commands` names them (redundant
//!      beside `--all-features`, and harmless).
//!   5. `settle`: `settle --check --base <rev>` as a child process of this executable, so a parallel change of
//!      settle is picked up by the build that runs it.
//!   6. `policy`: the checks of lane.md step 6, each by its trigger (see [`policy_commands`]). The cheap static
//!      checks `check-raw-read` and `check-disallowed-methods` run on any changed `.rs` file, since an allow attribute
//!      or a raw-model read is not visible in a path; `check-test-layout` runs on any changed path under a `tests/`
//!      directory and on any changed file that mentions `--test`.
//!   7. `fuzz`: `cargo check` of the fuzz workspace, when `crates/ew-testkit/tests/command_fuzz/**`, `fuzz/**` or the
//!      `Cargo.toml` of a crate in a fuzz target's closure (`ci/fuzz-smoke-targets.toml`) changed.
//! - **The nextest set, and its bound.** The changed crates, plus their **direct** reverse dependencies through
//!   normal, build and dev edges (one step, never the transitive closure, never `--workspace`: the closure is the
//!   merge point's `affected` run), plus the additions of lane.md's step-4 triggers ([`TRIGGERS`]). A package that
//!   only a trigger adds and whose trigger names test binaries runs only those binaries (an `-E` filter); a changed
//!   crate or a direct dependant runs whole.
//! - **New `tests/it` files.** Before the nextest step (and `--warm`), for each package whose diff adds a file under
//!   `tests/it/` (added or untracked), the modification time of its `tests/it/main.rs` is set to now: a binary that
//!   reads its modules with `automod::dir!` is not rebuilt for a new file alone, because cargo does not track that
//!   directory read.
//! - **Output.** A step's output goes whole to `target/lane-check/<step>.log`; a failed step's log is printed whole,
//!   never cut. Then one summary line per step: `lane-check: <step>: ok | FAILED | skipped (<why>)`, and
//!   `target/lane-check.json` with each step's name, status, commands, seconds and log path. The durations are
//!   reported, never asserted (owner's rule). The exit code is 0 only when no step failed.
//! - **`--plan`** prints the selection and the exact commands and runs nothing. **`--only <step>`** runs one step with
//!   the same selection. **`--warm`** builds the nextest set's test binaries with `cargo test --no-run` and the same
//!   `-p` set and features, so the later nextest runs reuse them; `--warm -p A -p B` takes an explicit set (a lane's
//!   start, when the diff is still empty).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::affected::{self, Members};

const USAGE: &str =
    "usage: cargo xtask lane-check --base <rev> [--only <step>] [--plan] [--warm [-p <pkg>]...]";

/// The steps, in their order.
pub(crate) const STEPS: [&str; 7] = ["fmt", "clippy", "doc", "nextest", "settle", "policy", "fuzz"];

/// The table of the fuzz targets and their path-dependency closures.
const FUZZ_TABLE: &str = "ci/fuzz-smoke-targets.toml";

/// The source of the command fuzzer, which the fuzz workspace includes as modules.
const COMMAND_FUZZ: &str = "crates/ew-testkit/tests/command_fuzz/";

/// One trigger of lane.md's step 4: a change of one of `crates` adds `add` to the nextest set. An added package
/// with binaries runs only those binaries (unless the diff selects it whole); with none, it runs whole.
pub(crate) struct Trigger {
    /// What lane.md calls the change.
    pub what: &'static str,
    /// The packages whose change fires it.
    pub crates: &'static [&'static str],
    /// The packages it adds, each with the test binaries it restricts the package to (none: the whole package).
    pub add: &'static [(&'static str, &'static [&'static str])],
}

/// lane.md's step-4 triggers. The scene is `ew-scene`; rendered output is `ew-scene`, `ew-render-cpu`,
/// `ew-render2d` and `ew-ui`; the shell is `ew-viewmodel`; the CLI is `ew`; commands are `ew-commands`; checks,
/// check rules and Golden A's findings are `ew-checks`.
pub(crate) const TRIGGERS: &[Trigger] = &[
    Trigger {
        what: "the scene, rendered output, the shell, ew-ui, commands, checks or the CLI",
        crates: &[
            "ew-scene",
            "ew-render-cpu",
            "ew-render2d",
            "ew-viewmodel",
            "ew-ui",
            "ew-commands",
            "ew-checks",
            "ew",
        ],
        add: &[("elektrawatt", &[]), ("ew", &[])],
    },
    Trigger {
        what: "commands or check rules",
        crates: &["ew-commands", "ew-checks"],
        add: &[("ew-testkit", &["command_fuzz", "golden_dsl"])],
    },
    Trigger {
        what: "rendered output or Golden A's findings",
        crates: &["ew-scene", "ew-render-cpu", "ew-render2d", "ew-ui", "ew-checks"],
        add: &[("ew-ui", &["snapshots"])],
    },
];

/// The program a command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Program {
    /// The cargo that invoked xtask.
    Cargo,
    /// This executable (shown as `cargo xtask`).
    Xtask,
}

/// One command of a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cmd {
    env: Option<(&'static str, &'static str)>,
    program: Program,
    args: Vec<String>,
}

impl Cmd {
    fn cargo(args: &[&str]) -> Self {
        Self {
            env: None,
            program: Program::Cargo,
            args: args.iter().map(|a| (*a).to_owned()).collect(),
        }
    }

    fn xtask(args: &[&str]) -> Self {
        Self {
            program: Program::Xtask,
            ..Self::cargo(args)
        }
    }

    fn packages(mut self, packages: &BTreeSet<String>) -> Self {
        for p in packages {
            self.args.push("-p".to_owned());
            self.args.push(p.clone());
        }
        self
    }

    fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

/// The command as a shell line: an argument with a space, a quote or a parenthesis in single quotes.
impl fmt::Display for Cmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((key, value)) = self.env {
            write!(f, "{key}=\"{value}\" ")?;
        }
        f.write_str(match self.program {
            Program::Cargo => "cargo",
            Program::Xtask => "cargo xtask",
        })?;
        for a in &self.args {
            if a.is_empty() || a.contains([' ', '\'', '"', '(', ')', '|', '&']) {
                write!(f, " '{a}'")?;
            } else {
                write!(f, " {a}")?;
            }
        }
        Ok(())
    }
}

/// The diff as the selection reads it.
#[derive(Debug, Default)]
pub(crate) struct Diff {
    /// Every changed file (tracked or untracked), `/`-separated.
    pub files: Vec<String>,
    /// The files the diff adds (added since the merge base, or untracked).
    pub added: BTreeSet<String>,
    /// The changed files whose text mentions `--test`.
    pub mentions_test: BTreeSet<String>,
}

/// One step of the plan: its commands, or why it is skipped.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StepPlan {
    pub name: &'static str,
    pub run: Result<Vec<Cmd>, String>,
}

/// What the diff selects.
#[derive(Debug)]
pub(crate) struct Plan {
    /// The members with a changed file.
    pub changed: BTreeSet<String>,
    /// Changed files outside every member that reach the whole workspace.
    pub workspace_wide: Vec<String>,
    /// The packages of the nextest step.
    pub nextest_set: BTreeSet<String>,
    /// The `tests/it/main.rs` files to touch before nextest builds.
    pub touch: Vec<String>,
    /// The steps, in [`STEPS`] order.
    pub steps: Vec<StepPlan>,
}

impl Plan {
    /// One line per step: `<step>: <command>` per command (the nextest step's touched files first, as
    /// `nextest: touch <file>`), or `<step>: skipped (<why>)`.
    pub(crate) fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in &self.steps {
            match &s.run {
                Ok(cmds) => {
                    if s.name == "nextest" {
                        out.extend(self.touch.iter().map(|t| format!("nextest: touch {t}")));
                    }
                    out.extend(cmds.iter().map(|c| format!("{}: {c}", s.name)));
                }
                Err(why) => out.push(format!("{}: skipped ({why})", s.name)),
            }
        }
        out
    }
}

/// The `--features` argument for the `required-features` of the packages' test targets, if any.
fn test_features(set: &BTreeSet<String>, members: &Members) -> Option<String> {
    let features: Vec<String> = set
        .iter()
        .filter_map(|p| members.get(p).map(|m| (p, m)))
        .flat_map(|(p, m)| m.test_features.iter().map(move |f| format!("{p}/{f}")))
        .collect();
    if features.is_empty() {
        None
    } else {
        Some(features.join(","))
    }
}

/// The nextest set of the changed crates, and the test binaries a trigger-only package is restricted to.
pub(crate) fn nextest_set(
    changed: &BTreeSet<String>,
    members: &Members,
) -> (BTreeSet<String>, BTreeMap<String, BTreeSet<&'static str>>) {
    let mut whole = changed.clone();
    for (name, m) in members {
        let direct = m
            .deps
            .iter()
            .chain(m.dev_deps.iter())
            .any(|d| d != name && changed.contains(d));
        if direct {
            whole.insert(name.clone());
        }
    }
    let mut restricted: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
    let mut added_whole = BTreeSet::new();
    for trigger in TRIGGERS {
        if !trigger.crates.iter().any(|c| changed.contains(*c)) {
            continue;
        }
        for (package, binaries) in trigger.add {
            if !members.contains_key(*package) || whole.contains(*package) {
                continue;
            }
            if binaries.is_empty() {
                added_whole.insert((*package).to_owned());
            } else {
                restricted
                    .entry((*package).to_owned())
                    .or_default()
                    .extend(binaries.iter().copied());
            }
        }
    }
    for p in &added_whole {
        restricted.remove(p);
    }
    whole.extend(added_whole);
    let mut all = whole;
    all.extend(restricted.keys().cloned());
    (all, restricted)
}

/// The nextest filter that restricts each trigger-only package to its binaries.
fn restriction_filter(restricted: &BTreeMap<String, BTreeSet<&'static str>>) -> Option<String> {
    if restricted.is_empty() {
        return None;
    }
    let clauses: Vec<String> = restricted
        .iter()
        .map(|(p, bins)| {
            let bins: Vec<String> = bins.iter().map(|b| format!("binary(={b})")).collect();
            format!("(not package(={p}) or {})", bins.join(" or "))
        })
        .collect();
    Some(clauses.join(" and "))
}

/// `cargo test --no-run` over `set` with the features of the nextest step: the warm-up build.
pub(crate) fn warm_command(set: &BTreeSet<String>, members: &Members) -> Cmd {
    let mut cmd = Cmd::cargo(&["test", "--no-run"])
        .packages(set)
        .arg("--all-features");
    if let Some(f) = test_features(set, members) {
        cmd = cmd.arg("--features").arg(f);
    }
    cmd
}

/// The policy checks of lane.md step 6 that the diff triggers, in lane.md's order.
pub(crate) fn policy_commands(diff: &Diff) -> Vec<Cmd> {
    let any = |pred: &dyn Fn(&str) -> bool| diff.files.iter().any(|f| pred(f));
    let manifest = |f: &str| f == "Cargo.toml" || f.ends_with("/Cargo.toml");
    let cargo_toml = any(&|f| manifest(f) || f == "ci/dep-rules.toml");
    let rust = any(&|f| f.ends_with(".rs") || f == "clippy.toml" || f == "ci/dep-rules.toml");
    let licence = !diff.added.is_empty() || any(&|f| f == "ci/licence-map.toml" || f == "REUSE.toml");
    let tests = !diff.mentions_test.is_empty()
        || any(&|f| f.starts_with("tests/") || f.contains("/tests/") || f == "xtask/src/test_layout.rs");
    let dependency =
        any(&|f| manifest(f) || f == "Cargo.lock" || f == "deny.toml" || f.starts_with("supply-chain/"));
    let mut cmds = Vec::new();
    if cargo_toml {
        cmds.push(Cmd::xtask(&["check-deps"]));
        cmds.push(Cmd::xtask(&["check-features"]));
    }
    if licence {
        cmds.push(Cmd::xtask(&["check-licence-map"]));
    }
    if rust {
        cmds.push(Cmd::xtask(&["check-raw-read"]));
        cmds.push(Cmd::xtask(&["check-disallowed-methods"]));
    }
    if tests {
        cmds.push(Cmd::xtask(&["check-test-layout"]));
    }
    if dependency {
        cmds.push(Cmd::cargo(&["deny", "--locked", "check"]));
        cmds.push(Cmd::cargo(&["vet", "--locked"]));
    }
    cmds
}

/// Whether the diff reaches the fuzz workspace.
fn fuzz_triggered(diff: &Diff, members: &Members, fuzz_closure: &BTreeSet<String>) -> bool {
    diff.files.iter().any(|f| {
        f.starts_with(COMMAND_FUZZ)
            || f.starts_with("fuzz/")
            || fuzz_closure
                .iter()
                .filter_map(|c| members.get(c))
                .any(|m| *f == format!("{}/Cargo.toml", m.dir))
    })
}

/// Names at most three files, then how many more.
fn some_files(files: &[String]) -> String {
    let mut s = files.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    if files.len() > 3 {
        s.push_str(&format!(" and {} more", files.len() - 3));
    }
    s
}

/// The plan for `diff` against `base`.
pub(crate) fn plan(diff: &Diff, members: &Members, fuzz_closure: &BTreeSet<String>, base: &str) -> Plan {
    let selection = affected::select(&diff.files, members);
    let changed = selection.changed;
    let workspace_wide = selection.workspace_wide;
    let (nextest_set, restricted) = nextest_set(&changed, members);

    let no_crate = if diff.files.is_empty() {
        Some("no changed file".to_owned())
    } else if changed.is_empty() && workspace_wide.is_empty() {
        Some("no crate in the diff".to_owned())
    } else if changed.is_empty() {
        Some(format!(
            "no crate in the diff; the merge point verifies the workspace-wide {}",
            some_files(&workspace_wide)
        ))
    } else {
        None
    };
    let cargo_step = |cmds: Vec<Cmd>| match &no_crate {
        Some(why) => Err(why.clone()),
        None => Ok(cmds),
    };

    let mut touch: Vec<String> = Vec::new();
    for file in &diff.added {
        for p in &nextest_set {
            let it = format!("{}/tests/it/", members[p].dir);
            let main = format!("{it}main.rs");
            if file.starts_with(&it) && *file != main && !touch.contains(&main) {
                touch.push(main);
            }
        }
    }
    touch.sort();

    let mut nextest = Cmd::cargo(&["nextest", "run", "--profile", "lane", "--cargo-quiet"])
        .packages(&nextest_set)
        .arg("--all-features");
    if let Some(f) = test_features(&nextest_set, members) {
        nextest = nextest.arg("--features").arg(f);
    }
    if let Some(filter) = restriction_filter(&restricted) {
        nextest = nextest.arg("-E").arg(filter);
    }
    let mut doc = Cmd::cargo(&["doc", "--no-deps"])
        .packages(&changed)
        .arg("--all-features");
    doc.env = Some(("RUSTDOCFLAGS", "-D warnings"));
    let policy = policy_commands(diff);

    let steps = vec![
        StepPlan {
            name: "fmt",
            run: cargo_step(vec![Cmd::cargo(&["fmt"]).packages(&changed)]),
        },
        StepPlan {
            name: "clippy",
            run: cargo_step(vec![
                Cmd::cargo(&["clippy"])
                    .packages(&changed)
                    .arg("--all-targets")
                    .arg("--all-features")
                    .arg("--")
                    .arg("-D")
                    .arg("warnings"),
            ]),
        },
        StepPlan {
            name: "doc",
            run: cargo_step(vec![doc]),
        },
        StepPlan {
            name: "nextest",
            run: cargo_step(vec![nextest]),
        },
        StepPlan {
            name: "settle",
            run: if diff.files.is_empty() {
                Err("no changed file".to_owned())
            } else {
                Ok(vec![Cmd::xtask(&["settle", "--check", "--base", base])])
            },
        },
        StepPlan {
            name: "policy",
            run: if policy.is_empty() {
                Err("no trigger".to_owned())
            } else {
                Ok(policy)
            },
        },
        StepPlan {
            name: "fuzz",
            run: if fuzz_triggered(diff, members, fuzz_closure) {
                Ok(vec![Cmd::cargo(&[
                    "check",
                    "--manifest-path",
                    "fuzz/Cargo.toml",
                    "--all-targets",
                    "--locked",
                ])])
            } else {
                Err("no trigger".to_owned())
            },
        },
    ];
    Plan {
        changed,
        workspace_wide,
        nextest_set,
        touch,
        steps,
    }
}

/// The parsed arguments.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    base: String,
    only: Option<&'static str>,
    plan: bool,
    warm: bool,
    packages: BTreeSet<String>,
}

fn parse(args: &[String]) -> Result<Args> {
    let mut out = Args::default();
    let mut base = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--base" => {
                base = Some(
                    it.next()
                        .with_context(|| format!("--base needs a revision\n{USAGE}"))?
                        .clone(),
                )
            }
            "--only" => {
                let step = it
                    .next()
                    .with_context(|| format!("--only needs a step\n{USAGE}"))?;
                let Some(step) = STEPS.iter().find(|s| **s == step.as_str()) else {
                    bail!(
                        "unknown step `{step}`; the steps are {}\n{USAGE}",
                        STEPS.join(", ")
                    );
                };
                out.only = Some(step);
            }
            "--plan" => out.plan = true,
            "--warm" => out.warm = true,
            "-p" => {
                out.packages.insert(
                    it.next()
                        .with_context(|| format!("-p needs a package\n{USAGE}"))?
                        .clone(),
                );
            }
            other => bail!("unknown argument `{other}`\n{USAGE}"),
        }
    }
    out.base = base.with_context(|| format!("--base is required\n{USAGE}"))?;
    if !out.packages.is_empty() && !out.warm {
        bail!("-p is for --warm only\n{USAGE}");
    }
    if out.warm && out.only.is_some() {
        bail!("--warm and --only do not combine\n{USAGE}");
    }
    Ok(out)
}

/// The files the diff adds: added since `merge_base` (tracked) or untracked.
fn added_files(root: &Path, merge_base: &str) -> Result<BTreeSet<String>> {
    let mut added: BTreeSet<String> = affected::nul_paths(&affected::git_output(
        root,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "--diff-filter=A",
            "-z",
            merge_base,
        ],
    )?)
    .into_iter()
    .collect();
    added.extend(affected::nul_paths(&affected::git_output(
        root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?));
    Ok(added)
}

/// The changed files that exist, are at most 1 MiB and mention `--test`.
fn mentioning_test(root: &Path, files: &[String]) -> BTreeSet<String> {
    files
        .iter()
        .filter(|f| {
            let path = root.join(f.as_str());
            std::fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() <= 1 << 20)
                && std::fs::read(&path).is_ok_and(|b| String::from_utf8_lossy(&b).contains("--test"))
        })
        .cloned()
        .collect()
}

/// The union of the fuzz targets' closures in `ci/fuzz-smoke-targets.toml` (empty without the file).
fn fuzz_closure(root: &Path) -> Result<BTreeSet<String>> {
    let path = root.join(FUZZ_TABLE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(BTreeSet::new());
    };
    let table: toml::Table = toml::from_str(&text).with_context(|| format!("parsing {FUZZ_TABLE}"))?;
    let mut out = BTreeSet::new();
    if let Some(targets) = table.get("targets").and_then(toml::Value::as_table) {
        for target in targets.values() {
            if let Some(closure) = target.get("closure").and_then(toml::Value::as_array) {
                out.extend(closure.iter().filter_map(toml::Value::as_str).map(str::to_owned));
            }
        }
    }
    Ok(out)
}

/// Sets the modification time of each file to now.
fn touch(root: &Path, files: &[String]) -> Result<()> {
    for f in files {
        let path = root.join(f);
        File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()))
            .with_context(|| format!("touching {f}"))?;
    }
    Ok(())
}

/// The outcome of one executed step.
struct Outcome {
    name: &'static str,
    status: &'static str,
    why: Option<String>,
    commands: Vec<String>,
    seconds: f64,
    log: Option<String>,
}

/// Runs the commands of one step in sequence, all of them even after a failure, into `target/lane-check/<step>.log`.
fn execute(root: &Path, name: &'static str, cmds: &[Cmd]) -> Result<Outcome> {
    let dir = root.join("target/lane-check");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let log_rel = format!("target/lane-check/{name}.log");
    let log_path = root.join(&log_rel);
    let mut log = File::create(&log_path).with_context(|| format!("creating {log_rel}"))?;
    let start = Instant::now();
    let mut ok = true;
    for cmd in cmds {
        writeln!(log, "==> {cmd}")?;
        let mut command = match cmd.program {
            Program::Cargo => Command::new(crate::cargo()),
            Program::Xtask => Command::new(std::env::current_exe().context("locating the xtask executable")?),
        };
        command
            .args(&cmd.args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log.try_clone()?));
        if let Some((key, value)) = cmd.env {
            command.env(key, value);
        }
        match command.status() {
            Ok(status) if status.success() => {}
            Ok(status) => {
                writeln!(log, "==> exit status: {status}")?;
                ok = false;
            }
            Err(e) => {
                writeln!(log, "==> could not start `{cmd}`: {e}")?;
                ok = false;
            }
        }
    }
    drop(log);
    if name == "clippy" {
        let text = String::from_utf8_lossy(&std::fs::read(&log_path)?).into_owned();
        let unresolved = crate::unresolved_clippy_paths(&text);
        if !unresolved.is_empty() {
            let mut log = File::options().append(true).open(&log_path)?;
            for line in &unresolved {
                writeln!(
                    log,
                    "clippy: unresolvable disallowed path (ADR-0001 amendment 2): {line}"
                )?;
            }
            ok = false;
        }
    }
    Ok(Outcome {
        name,
        status: if ok { "ok" } else { "FAILED" },
        why: None,
        commands: cmds.iter().map(ToString::to_string).collect(),
        seconds: start.elapsed().as_secs_f64(),
        log: Some(log_rel),
    })
}

/// Prints a failed step's log whole.
fn print_failure(root: &Path, outcome: &Outcome) {
    let Some(log) = &outcome.log else { return };
    eprintln!("lane-check: {}: FAILED; the full log is {log}:", outcome.name);
    match std::fs::read(root.join(log)) {
        Ok(bytes) => eprint!("{}", String::from_utf8_lossy(&bytes)),
        Err(e) => eprintln!("lane-check: could not read {log}: {e}"),
    }
}

/// Prints the summary block and writes `target/lane-check.json`.
fn summarise(root: &Path, base: &str, outcomes: &[Outcome], write_json: bool) -> Result<bool> {
    println!();
    for o in outcomes {
        match &o.why {
            Some(why) => println!("lane-check: {}: skipped ({why})", o.name),
            None => println!("lane-check: {}: {}", o.name, o.status),
        }
    }
    if write_json {
        let steps: Vec<serde_json::Value> = outcomes
            .iter()
            .map(|o| {
                serde_json::json!({
                    "name": o.name,
                    "status": if o.why.is_some() { "skipped" } else { o.status },
                    "why": o.why,
                    "command": o.commands,
                    "seconds": o.seconds,
                    "log": o.log,
                })
            })
            .collect();
        let json = serde_json::json!({ "base": base, "steps": steps });
        let path = root.join("target/lane-check.json");
        std::fs::create_dir_all(root.join("target"))?;
        std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&json)?))
            .with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(outcomes.iter().all(|o| o.status != "FAILED"))
}

/// `cargo xtask lane-check`.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    let a = parse(args)?;
    let merge_base = affected::merge_base(root, &a.base)?;
    let files = affected::changed_files(root, &merge_base)?;
    let diff = Diff {
        mentions_test: mentioning_test(root, &files),
        added: added_files(root, &merge_base)?,
        files,
    };
    let members = affected::members_of(&crate::metadata_all_features(root)?)?;
    for p in &a.packages {
        if !members.contains_key(p) {
            bail!("`{p}` is not a workspace member");
        }
    }
    let plan = plan(&diff, &members, &fuzz_closure(root)?, &a.base);

    if a.warm {
        let set = if a.packages.is_empty() {
            &plan.nextest_set
        } else {
            &a.packages
        };
        if set.is_empty() {
            println!("lane-check: warm: skipped (no package: name them with -p)");
            return Ok(true);
        }
        let cmd = warm_command(set, &members);
        if a.plan {
            for t in &plan.touch {
                println!("warm: touch {t}");
            }
            println!("warm: {cmd}");
            return Ok(true);
        }
        touch(root, &plan.touch)?;
        let outcome = execute(root, "warm", &[cmd])?;
        if outcome.status == "FAILED" {
            print_failure(root, &outcome);
        }
        return summarise(root, &a.base, &[outcome], false);
    }

    if a.plan {
        let short = &merge_base[..merge_base.len().min(12)];
        println!(
            "lane-check: {} changed file(s) since {} (merge base {short})",
            diff.files.len(),
            a.base
        );
        println!("lane-check: changed crates: {}", join_or_none(&plan.changed));
        if !plan.workspace_wide.is_empty() {
            println!(
                "lane-check: workspace-wide files (the merge point verifies them): {}",
                plan.workspace_wide.join(", ")
            );
        }
        println!(
            "lane-check: nextest set (changed crates, direct dependants, lane.md triggers): {}",
            join_or_none(&plan.nextest_set)
        );
        for t in TRIGGERS
            .iter()
            .filter(|t| t.crates.iter().any(|c| plan.changed.contains(*c)))
        {
            println!("lane-check: lane.md trigger: {}", t.what);
        }
        for line in plan.lines() {
            let step = line.split(':').next().unwrap_or_default();
            if a.only.is_none_or(|o| o == step) {
                println!("{line}");
            }
        }
        return Ok(true);
    }

    let mut outcomes = Vec::new();
    for s in &plan.steps {
        if a.only.is_some_and(|o| o != s.name) {
            continue;
        }
        let outcome = match &s.run {
            Err(why) => Outcome {
                name: s.name,
                status: "skipped",
                why: Some(why.clone()),
                commands: Vec::new(),
                seconds: 0.0,
                log: None,
            },
            Ok(cmds) => {
                if s.name == "nextest" {
                    touch(root, &plan.touch)?;
                }
                let outcome = execute(root, s.name, cmds)?;
                if outcome.status == "FAILED" {
                    print_failure(root, &outcome);
                }
                outcome
            }
        };
        outcomes.push(outcome);
    }
    summarise(root, &a.base, &outcomes, true)
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
    use crate::affected::Member;

    const BASE: &str = "4762316c";

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| (*s).to_owned()).collect()
    }

    /// ew-model <- ew-commands <- ew-api <- {ew, ew-viewmodel}; ew-api dev-depends on ew-testkit, which depends on
    /// ew-api; ew-checks depends on ew-model; ew-viewmodel <- ew-ui <- elektrawatt, and elektrawatt dev-depends on
    /// ew-viewmodel; ew-viewmodel's tests need `testkit`; ew-format is in a fuzz closure; ew-leaf and xtask stand
    /// alone.
    fn members() -> Members {
        let m = |dir: &str, deps: &[&str], dev: &[&str], features: &[&str]| Member {
            dir: dir.to_owned(),
            deps: set(deps),
            dev_deps: set(dev),
            test_features: set(features),
        };
        [
            ("ew-model", m("crates/ew-model", &[], &[], &[])),
            ("ew-format", m("crates/ew-format", &[], &[], &[])),
            (
                "ew-commands",
                m("crates/ew-commands", &["ew-model"], &["ew-commands"], &[]),
            ),
            ("ew-checks", m("crates/ew-checks", &["ew-model"], &[], &[])),
            (
                "ew-api",
                m(
                    "crates/ew-api",
                    &["ew-commands", "ew-checks"],
                    &["ew-testkit"],
                    &[],
                ),
            ),
            ("ew-testkit", m("crates/ew-testkit", &["ew-api"], &[], &[])),
            ("ew", m("bin/ew", &["ew-api"], &[], &[])),
            (
                "ew-viewmodel",
                m("crates/ew-viewmodel", &["ew-api"], &[], &["testkit"]),
            ),
            ("ew-ui", m("crates/ew-ui", &["ew-viewmodel"], &[], &[])),
            (
                "elektrawatt",
                m("bin/elektrawatt", &["ew-ui"], &["ew-viewmodel"], &[]),
            ),
            ("ew-leaf", m("crates/ew-leaf", &[], &[], &[])),
            ("xtask", m("xtask", &[], &[], &[])),
        ]
        .into_iter()
        .map(|(n, m)| (n.to_owned(), m))
        .collect()
    }

    fn diff(files: &[&str], added: &[&str]) -> Diff {
        Diff {
            files: files.iter().map(|s| (*s).to_owned()).collect(),
            added: set(added),
            mentions_test: BTreeSet::new(),
        }
    }

    fn lines(d: &Diff) -> Vec<String> {
        plan(d, &members(), &set(&["ew-format"]), BASE).lines()
    }

    #[test]
    fn a_docs_only_diff_skips_the_cargo_steps_and_runs_settle() {
        assert_eq!(
            lines(&diff(&["docs/plan/01-global-plan.md", "README.md"], &[])),
            [
                "fmt: skipped (no crate in the diff)",
                "clippy: skipped (no crate in the diff)",
                "doc: skipped (no crate in the diff)",
                "nextest: skipped (no crate in the diff)",
                "settle: cargo xtask settle --check --base 4762316c",
                "policy: skipped (no trigger)",
                "fuzz: skipped (no trigger)",
            ]
        );
    }

    #[test]
    fn an_empty_diff_skips_everything() {
        let l = lines(&diff(&[], &[]));
        assert_eq!(l[0], "fmt: skipped (no changed file)");
        assert_eq!(l[4], "settle: skipped (no changed file)");
    }

    #[test]
    fn a_leaf_crate_runs_its_own_steps_and_the_static_checks() {
        assert_eq!(
            lines(&diff(&["crates/ew-leaf/src/lib.rs"], &[])),
            [
                "fmt: cargo fmt -p ew-leaf",
                "clippy: cargo clippy -p ew-leaf --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-leaf --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-leaf --all-features",
                "settle: cargo xtask settle --check --base 4762316c",
                "policy: cargo xtask check-raw-read",
                "policy: cargo xtask check-disallowed-methods",
                "fuzz: skipped (no trigger)",
            ]
        );
    }

    /// ew-model's direct dependants run (ew-commands, ew-checks), the transitive ones do not (ew-api); ew-checks and
    /// ew-commands are not changed, so their triggers do not fire.
    #[test]
    fn a_crate_with_dependants_adds_only_the_direct_ones() {
        let l = lines(&diff(&["crates/ew-model/src/table.rs"], &[]));
        assert_eq!(
            l[3],
            "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-checks -p ew-commands -p ew-model \
             --all-features"
        );
        assert_eq!(
            l[1],
            "clippy: cargo clippy -p ew-model --all-targets --all-features -- -D warnings"
        );
    }

    /// The shell: ew-viewmodel's dependants (ew-ui, and elektrawatt by its dev edge) plus `ew` by lane.md's trigger,
    /// with the viewmodel's test features.
    #[test]
    fn the_shell_adds_the_binaries_and_its_test_features() {
        let l = lines(&diff(&["crates/ew-viewmodel/src/strings.rs"], &[]));
        assert_eq!(
            l[3],
            "nextest: cargo nextest run --profile lane --cargo-quiet -p elektrawatt -p ew -p ew-ui -p ew-viewmodel \
             --all-features --features ew-viewmodel/testkit"
        );
    }

    /// A command change: ew-api is a direct dependant, ew-testkit is not, so it runs only the two binaries the
    /// trigger names; the binaries of the trigger for the CLI and the shell run whole.
    #[test]
    fn a_command_change_restricts_the_testkit_to_its_binaries() {
        let l = lines(&diff(&["crates/ew-commands/src/wire.rs"], &[]));
        assert_eq!(
            l[3],
            "nextest: cargo nextest run --profile lane --cargo-quiet -p elektrawatt -p ew -p ew-api -p ew-commands \
             -p ew-testkit --all-features -E '(not package(=ew-testkit) or binary(=command_fuzz) or \
             binary(=golden_dsl))'"
        );
    }

    #[test]
    fn a_changed_cargo_toml_runs_the_dependency_checks() {
        assert_eq!(
            lines(&diff(&["crates/ew-leaf/Cargo.toml"], &[])),
            [
                "fmt: cargo fmt -p ew-leaf",
                "clippy: cargo clippy -p ew-leaf --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-leaf --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-leaf --all-features",
                "settle: cargo xtask settle --check --base 4762316c",
                "policy: cargo xtask check-deps",
                "policy: cargo xtask check-features",
                "policy: cargo deny --locked check",
                "policy: cargo vet --locked",
                "fuzz: skipped (no trigger)",
            ]
        );
        // A fuzzed crate's manifest reaches the fuzz workspace too.
        let l = lines(&diff(&["crates/ew-format/Cargo.toml"], &[]));
        assert_eq!(
            l.last().unwrap(),
            "fuzz: cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked"
        );
    }

    #[test]
    fn a_new_tests_it_file_touches_the_binary_and_runs_the_layout_and_licence_checks() {
        let d = diff(
            &["crates/ew-leaf/tests/it/new_case.rs"],
            &["crates/ew-leaf/tests/it/new_case.rs"],
        );
        let p = plan(&d, &members(), &BTreeSet::new(), BASE);
        assert_eq!(p.touch, ["crates/ew-leaf/tests/it/main.rs"]);
        assert_eq!(
            &p.lines()[3..],
            [
                "nextest: touch crates/ew-leaf/tests/it/main.rs",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-leaf --all-features",
                "settle: cargo xtask settle --check --base 4762316c",
                "policy: cargo xtask check-licence-map",
                "policy: cargo xtask check-raw-read",
                "policy: cargo xtask check-disallowed-methods",
                "policy: cargo xtask check-test-layout",
                "fuzz: skipped (no trigger)",
            ]
        );
        // A changed (not added) file touches nothing.
        let d = diff(&["crates/ew-leaf/tests/it/new_case.rs"], &[]);
        assert!(plan(&d, &members(), &BTreeSet::new(), BASE).touch.is_empty());
    }

    #[test]
    fn a_command_fuzz_change_runs_the_fuzz_workspace_check() {
        let l = lines(&diff(&["crates/ew-testkit/tests/command_fuzz/step.rs"], &[]));
        assert_eq!(
            l,
            [
                "fmt: cargo fmt -p ew-testkit",
                "clippy: cargo clippy -p ew-testkit --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-testkit --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-api -p ew-testkit --all-features",
                "settle: cargo xtask settle --check --base 4762316c",
                "policy: cargo xtask check-raw-read",
                "policy: cargo xtask check-disallowed-methods",
                "policy: cargo xtask check-test-layout",
                "fuzz: cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked",
            ]
        );
    }

    #[test]
    fn a_cargo_lock_change_runs_the_supply_chain_checks_and_no_cargo_step() {
        assert_eq!(
            lines(&diff(&["Cargo.lock"], &[])),
            [
                "fmt: skipped (no crate in the diff; the merge point verifies the workspace-wide Cargo.lock)",
                "clippy: skipped (no crate in the diff; the merge point verifies the workspace-wide Cargo.lock)",
                "doc: skipped (no crate in the diff; the merge point verifies the workspace-wide Cargo.lock)",
                "nextest: skipped (no crate in the diff; the merge point verifies the workspace-wide Cargo.lock)",
                "settle: cargo xtask settle --check --base 4762316c",
                "policy: cargo deny --locked check",
                "policy: cargo vet --locked",
                "fuzz: skipped (no trigger)",
            ]
        );
    }

    #[test]
    fn a_file_mentioning_test_runs_the_layout_check() {
        let mut d = diff(&["docs/x.md"], &[]);
        d.mentions_test = set(&["docs/x.md"]);
        assert_eq!(policy_commands(&d), [Cmd::xtask(&["check-test-layout"])]);
    }

    #[test]
    fn the_warm_command_uses_the_nextest_features() {
        assert_eq!(
            warm_command(&set(&["ew-ui", "ew-viewmodel"]), &members()).to_string(),
            "cargo test --no-run -p ew-ui -p ew-viewmodel --all-features --features ew-viewmodel/testkit"
        );
    }

    #[test]
    fn arguments_need_a_base_and_known_steps() {
        let s = |a: &[&str]| a.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            parse(&s(&["--plan"])).unwrap_err().to_string(),
            format!("--base is required\n{USAGE}")
        );
        assert_eq!(
            parse(&s(&["--base", "main", "--only", "lint"]))
                .unwrap_err()
                .to_string(),
            format!(
                "unknown step `lint`; the steps are fmt, clippy, doc, nextest, settle, policy, fuzz\n{USAGE}"
            )
        );
        assert_eq!(
            parse(&s(&["--base", "main", "-p", "ew-ui"]))
                .unwrap_err()
                .to_string(),
            format!("-p is for --warm only\n{USAGE}")
        );
        assert_eq!(
            parse(&s(&["--base", "main", "--warm", "-p", "ew-ui", "-p", "ew"])).unwrap(),
            Args {
                base: "main".to_owned(),
                warm: true,
                packages: set(&["ew", "ew-ui"]),
                ..Args::default()
            }
        );
    }
}
