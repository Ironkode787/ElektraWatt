// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `lane-check --base <rev> [-p <pkg>]... [--only <step>] [--plan] [--warm]`: a lane's hand-back checks
//! (`.claude/agents/lane.md`, "Hand-back checks"), run once and chosen from the diff against `<rev>` and the lane's
//! declared package set (`-p`, its edit-loop set).
//!
//! - **The diff.** The changed files are those of [`affected::changed_files`] since the merge base of `<rev>` and
//!   `HEAD` (committed, uncommitted and untracked). A file belongs to the deepest member whose directory holds it;
//!   `ci/` belongs to `xtask`. fmt, clippy and doc run over the changed crates and the declared set; with neither
//!   (only `affected`'s inert paths changed, say) they are skipped.
//! - **Workspace-wide files** (any changed file outside the members that `affected` does not call inert:
//!   `Cargo.lock`, `golden/`, `content/`, `schemas/`, `assets/`, `clippy.toml`, ...) reach crates that no path names.
//!   Without a declared set the nextest step fails closed: `FAILED (workspace-wide files changed: <files>; name the
//!   packages they reach with -p)`. With `-p`, the declared set is the lane's answer and the steps run over it, except
//!   for the roots of [`GUARDED_ROOTS`]: a package whose `.rs` files name a changed root (a scan of the tracked files,
//!   [`root_readers`]; `xtask`, which names them as policy, aside) must be in the set, or the nextest step fails
//!   closed naming the missing ones (C43-p2#6).
//! - **The steps,** in this order:
//!   1. `fmt`: `cargo fmt`, then settle's per-file rustfmt over the diff's `.rs` files (`settle::rust_files`: never a
//!      module under `#[rustfmt::skip]`), which reaches the test modules `automod::dir!` includes and `cargo fmt`
//!      does not see. It writes; it is not a check. When it changed a file of the diff, it reports
//!      `rewrote <files> (commit them)` (not a failure).
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
//!      directory and on any changed file that mentions `--test`; `map --check` (the shared-list markers) on any
//!      changed `.rs` file, `Cargo.toml` or `ci/dep-rules.toml`.
//!   7. `fuzz`: `cargo check` of the fuzz workspace, when `crates/ew-testkit/tests/command_fuzz/**`, `fuzz/**` or the
//!      `Cargo.toml` of a crate in a fuzz target's closure (`ci/fuzz-smoke-targets.toml`) changed.
//! - **The nextest set, and its bound.** The changed crates, plus their **direct** reverse dependencies through
//!   normal, build and dev edges (one step, never the transitive closure, never `--workspace`: the closure is the
//!   merge point's `affected` run), plus the additions of lane.md's step-4 triggers ([`TRIGGERS`]), plus the declared
//!   set. A package that only a trigger adds and whose trigger names test binaries runs only those binaries (an `-E`
//!   filter); a changed crate, a direct dependant or a declared package runs whole.
//! - **New `tests/it` files.** Before the nextest step (and `--warm`), for each package whose diff adds a file under
//!   `tests/it/` (added or untracked), the modification time of its `tests/it/main.rs` is set to now: a binary that
//!   reads its modules with `automod::dir!` is not rebuilt for a new file alone, because cargo does not track that
//!   directory read.
//! - **Output.** A step's output goes whole to `target/lane-check/<step>.log`; a failed step's log is printed whole,
//!   never cut. Then one summary line per step: `lane-check: <step>: ok | FAILED | skipped (<why>)`, and
//!   `target/lane-check.json` with each step's name, status, commands, seconds and log path. The durations are
//!   reported, never asserted (owner's rule). The exit code is 0 only when no step failed.
//! - **`--plan`** prints the selection and the exact commands and runs nothing. **`--only <step>`** runs one step with
//!   the same selection. **`--warm`** builds exactly the nextest step's packages (the derived set and the declared
//!   one; at a lane's start the derived part is empty) with `cargo test --no-run` and the same features, so the
//!   later nextest runs reuse the test binaries.

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
    "usage: cargo xtask lane-check --base <rev> [-p <pkg>]... [--only <step>] [--plan] [--warm]";

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
    /// rustfmt on the files of the arguments, as settle runs it.
    Rustfmt,
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

    fn rustfmt(files: &[String]) -> Self {
        Self {
            env: None,
            program: Program::Rustfmt,
            args: files.to_vec(),
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
            Program::Rustfmt => "rustfmt",
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
    /// The changed `.rs` files rustfmt takes (`settle::rust_files`): the fmt step's per-file pass.
    pub rustfmt: Vec<String>,
    /// For each root of [`GUARDED_ROOTS`] the diff touches, the packages whose `.rs` files name it ([`root_readers`]).
    pub readers: BTreeMap<&'static str, BTreeSet<String>>,
}

/// The workspace-wide roots whose readers must be in a declared set (C43-p2#6): the golden files, the format schemas
/// and the shipped content.
pub(crate) const GUARDED_ROOTS: [&str; 3] = ["golden/", "schemas/0.1/", "content/"];

/// Whether `text` names the repository root `root` (`golden/`) as a path: `"golden/`, `/golden/`, `"golden"` or
/// `/golden"`.
fn names_root(text: &str, root: &str) -> bool {
    let bare = root.trim_end_matches('/');
    [
        format!("\"{bare}/"),
        format!("/{bare}/"),
        format!("\"{bare}\""),
        format!("/{bare}\""),
    ]
    .iter()
    .any(|p| text.contains(p.as_str()))
}

/// For each of `roots`, the members whose tracked `.rs` files (`files`, with their text from `read`) name it; `xtask`
/// names the roots as policy, not as data, and stays out.
pub(crate) fn root_readers(
    roots: &[&'static str],
    files: &[String],
    members: &Members,
    read: &dyn Fn(&str) -> Option<String>,
) -> BTreeMap<&'static str, BTreeSet<String>> {
    let mut out: BTreeMap<&'static str, BTreeSet<String>> =
        roots.iter().map(|r| (*r, BTreeSet::new())).collect();
    if roots.is_empty() {
        return out;
    }
    for file in files.iter().filter(|f| f.ends_with(".rs")) {
        let Ok(Some(owner)) = affected::owner(file, members) else {
            continue;
        };
        if owner == "xtask" || roots.iter().all(|r| out[r].contains(owner)) {
            continue;
        }
        let Some(text) = read(file) else { continue };
        for r in roots {
            if names_root(&text, r) {
                out.get_mut(r).map(|s| s.insert(owner.to_owned()));
            }
        }
    }
    out
}

/// What one step of the plan does.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StepRun {
    /// Runs these commands.
    Run(Vec<Cmd>),
    /// Is skipped, for this reason.
    Skip(String),
    /// Fails without running, for this reason (it fails closed).
    Fail(String),
}

/// One step of the plan.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StepPlan {
    pub name: &'static str,
    pub run: StepRun,
}

/// What the diff selects.
#[derive(Debug)]
pub(crate) struct Plan {
    /// The members with a changed file.
    pub changed: BTreeSet<String>,
    /// The packages of fmt, clippy and doc: the changed members and the declared set.
    pub crates: BTreeSet<String>,
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
    /// `nextest: touch <file>`), `<step>: skipped (<why>)` or `<step>: FAILED (<why>)`.
    pub(crate) fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in &self.steps {
            match &s.run {
                StepRun::Run(cmds) => {
                    if s.name == "nextest" {
                        out.extend(self.touch.iter().map(|t| format!("nextest: touch {t}")));
                    }
                    out.extend(cmds.iter().map(|c| format!("{}: {c}", s.name)));
                }
                StepRun::Skip(why) => out.push(format!("{}: skipped ({why})", s.name)),
                StepRun::Fail(why) => out.push(format!("{}: FAILED ({why})", s.name)),
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
    let markers = any(&|f| f.ends_with(".rs") || manifest(f) || f == "ci/dep-rules.toml");
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
    if markers {
        cmds.push(Cmd::xtask(&["map", "--check"]));
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

/// The plan for `diff` against `base`, with the lane's declared package set (`-p`, the edit-loop set).
pub(crate) fn plan(
    diff: &Diff,
    members: &Members,
    fuzz_closure: &BTreeSet<String>,
    base: &str,
    declared: &BTreeSet<String>,
) -> Plan {
    let selection = affected::select(&diff.files, members);
    let changed = selection.changed;
    let workspace_wide = selection.workspace_wide;
    let (mut nextest_set, mut restricted) = nextest_set(&changed, members);
    for p in declared {
        restricted.remove(p);
    }
    nextest_set.extend(declared.iter().cloned());
    let mut crates = changed.clone();
    crates.extend(declared.iter().cloned());

    let skip = if crates.is_empty() && diff.files.is_empty() {
        Some("no changed file".to_owned())
    } else if crates.is_empty() {
        Some("no crate in the diff".to_owned())
    } else {
        None
    };
    // Workspace-wide files with no declared set: the lane must name the packages they reach (fails closed).
    let wide_without_set = if workspace_wide.is_empty() {
        None
    } else if declared.is_empty() {
        Some(format!(
            "workspace-wide files changed: {}; name the packages they reach with -p",
            some_files(&workspace_wide)
        ))
    } else {
        // C43-p2#6: the declared set must hold every package that names a changed guarded root.
        let missing: Vec<String> = GUARDED_ROOTS
            .iter()
            .filter(|r| workspace_wide.iter().any(|f| f.starts_with(**r)))
            .filter_map(|r| {
                let absent: Vec<&str> = diff
                    .readers
                    .get(r)
                    .into_iter()
                    .flatten()
                    .filter(|p| !declared.contains(*p))
                    .map(String::as_str)
                    .collect();
                (!absent.is_empty()).then(|| format!("{r} changed and {} name it", absent.join(", ")))
            })
            .collect();
        (!missing.is_empty()).then(|| format!("{}; add them with -p", missing.join("; ")))
    };
    let cargo_step = |cmds: Vec<Cmd>| match &skip {
        Some(why) => StepRun::Skip(why.clone()),
        None => StepRun::Run(cmds),
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
        .packages(&crates)
        .arg("--all-features");
    doc.env = Some(("RUSTDOCFLAGS", "-D warnings"));
    let policy = policy_commands(diff);

    let mut fmt = vec![Cmd::cargo(&["fmt"]).packages(&crates)];
    fmt.extend(
        diff.rustfmt
            .chunks(crate::settle::RUSTFMT_CHUNK)
            .map(Cmd::rustfmt),
    );
    let steps = vec![
        StepPlan {
            name: "fmt",
            run: cargo_step(fmt),
        },
        StepPlan {
            name: "clippy",
            run: cargo_step(vec![
                Cmd::cargo(&["clippy"])
                    .packages(&crates)
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
            run: match wide_without_set {
                Some(why) => StepRun::Fail(why),
                None => cargo_step(vec![nextest]),
            },
        },
        StepPlan {
            name: "settle",
            run: if diff.files.is_empty() {
                StepRun::Skip("no changed file".to_owned())
            } else {
                StepRun::Run(vec![Cmd::xtask(&["settle", "--check", "--base", base])])
            },
        },
        StepPlan {
            name: "policy",
            run: if policy.is_empty() {
                StepRun::Skip("no trigger".to_owned())
            } else {
                StepRun::Run(policy)
            },
        },
        StepPlan {
            name: "fuzz",
            run: if fuzz_triggered(diff, members, fuzz_closure) {
                StepRun::Run(vec![Cmd::cargo(&[
                    "check",
                    "--manifest-path",
                    "fuzz/Cargo.toml",
                    "--all-targets",
                    "--locked",
                ])])
            } else {
                StepRun::Skip("no trigger".to_owned())
            },
        },
    ];
    Plan {
        changed,
        crates,
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

/// The contents of the changed `.rs` files that exist: what `fmt` may rewrite.
fn rust_snapshot(root: &Path, files: &[String]) -> BTreeMap<String, Vec<u8>> {
    files
        .iter()
        .filter(|f| f.ends_with(".rs"))
        .filter_map(|f| std::fs::read(root.join(f)).ok().map(|b| (f.clone(), b)))
        .collect()
}

/// The outcome of one executed step.
struct Outcome {
    name: &'static str,
    /// `ok`, `FAILED`, `skipped` or `rewrote` (fmt changed files).
    status: &'static str,
    /// Why it was skipped or failed without running, or the files fmt rewrote.
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
            Program::Rustfmt => Command::new("rustfmt"),
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

/// The summary line of one step: `lane-check: <step>: <status>`, with the reason or the rewritten files.
fn summary_line(o: &Outcome) -> String {
    match (o.status, &o.why) {
        ("rewrote", Some(files)) => format!("lane-check: {}: rewrote {files} (commit them)", o.name),
        (status, Some(why)) => format!("lane-check: {}: {status} ({why})", o.name),
        (status, None) => format!("lane-check: {}: {status}", o.name),
    }
}

/// The files whose bytes differ between the snapshots before and after the fmt step.
fn rewrote_files(before: &BTreeMap<String, Vec<u8>>, after: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    after
        .iter()
        .filter(|(f, bytes)| before.get(*f) != Some(*bytes))
        .map(|(f, _)| f.clone())
        .collect()
}

/// Prints the summary block and writes `target/lane-check.json`.
fn summarise(root: &Path, base: &str, outcomes: &[Outcome], write_json: bool) -> Result<bool> {
    println!();
    for o in outcomes {
        println!("{}", summary_line(o));
    }
    if write_json {
        let steps: Vec<serde_json::Value> = outcomes
            .iter()
            .map(|o| {
                serde_json::json!({
                    "name": o.name,
                    "status": o.status,
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
    let members = affected::members_of(&crate::metadata_all_features(root)?)?;
    let touched: Vec<&'static str> = GUARDED_ROOTS
        .iter()
        .copied()
        .filter(|r| files.iter().any(|f| f.starts_with(r)))
        .collect();
    let tracked = if touched.is_empty() {
        Vec::new()
    } else {
        affected::nul_paths(&affected::git_output(root, &["ls-files", "-z", "--", "*.rs"])?)
    };
    let read = |f: &str| std::fs::read_to_string(root.join(f)).ok();
    let diff = Diff {
        mentions_test: mentioning_test(root, &files),
        added: added_files(root, &merge_base)?,
        rustfmt: crate::settle::rust_files(root, &files.iter().cloned().collect(), false)?,
        readers: root_readers(&touched, &tracked, &members, &read),
        files,
    };
    for p in &a.packages {
        if !members.contains_key(p) {
            bail!("`{p}` is not a workspace member");
        }
    }
    let plan = plan(&diff, &members, &fuzz_closure(root)?, &a.base, &a.packages);

    if a.warm {
        // Exactly the nextest step's set: the derived set and the declared one.
        let set = &plan.nextest_set;
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
        println!("lane-check: declared set (-p): {}", join_or_none(&a.packages));
        println!(
            "lane-check: fmt, clippy and doc over: {}",
            join_or_none(&plan.crates)
        );
        if !plan.workspace_wide.is_empty() {
            println!(
                "lane-check: workspace-wide files (nextest needs -p for them): {}",
                plan.workspace_wide.join(", ")
            );
        }
        println!(
            "lane-check: nextest set (changed crates, direct dependants, lane.md triggers, -p): {}",
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
        let not_run = |status: &'static str, why: &String| Outcome {
            name: s.name,
            status,
            why: Some(why.clone()),
            commands: Vec::new(),
            seconds: 0.0,
            log: None,
        };
        let outcome = match &s.run {
            StepRun::Skip(why) => not_run("skipped", why),
            StepRun::Fail(why) => not_run("FAILED", why),
            StepRun::Run(cmds) => {
                if s.name == "nextest" {
                    touch(root, &plan.touch)?;
                }
                let before = (s.name == "fmt").then(|| rust_snapshot(root, &diff.files));
                let mut outcome = execute(root, s.name, cmds)?;
                if let Some(before) = before {
                    let rewrote = rewrote_files(&before, &rust_snapshot(root, &diff.files));
                    if outcome.status == "ok" && !rewrote.is_empty() {
                        outcome.status = "rewrote";
                        outcome.why = Some(rewrote.join(", "));
                    }
                }
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
            ..Diff::default()
        }
    }

    fn lines(d: &Diff) -> Vec<String> {
        lines_with(d, &[])
    }

    /// The plan's lines with the declared set `-p <declared>`.
    fn lines_with(d: &Diff, declared: &[&str]) -> Vec<String> {
        plan(d, &members(), &set(&["ew-format"]), BASE, &set(declared)).lines()
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
                "policy: cargo xtask map --check",
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
                "policy: cargo xtask map --check",
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
        let p = plan(&d, &members(), &BTreeSet::new(), BASE, &BTreeSet::new());
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
                "policy: cargo xtask map --check",
                "fuzz: skipped (no trigger)",
            ]
        );
        // A changed (not added) file touches nothing.
        let d = diff(&["crates/ew-leaf/tests/it/new_case.rs"], &[]);
        assert!(
            plan(&d, &members(), &BTreeSet::new(), BASE, &BTreeSet::new())
                .touch
                .is_empty()
        );
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
                "policy: cargo xtask map --check",
                "fuzz: cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked",
            ]
        );
    }

    #[test]
    fn a_cargo_lock_change_without_a_declared_set_fails_closed() {
        assert_eq!(
            lines(&diff(&["Cargo.lock"], &[])),
            [
                "fmt: skipped (no crate in the diff)",
                "clippy: skipped (no crate in the diff)",
                "doc: skipped (no crate in the diff)",
                "nextest: FAILED (workspace-wide files changed: Cargo.lock; name the packages they reach with -p)",
                "settle: cargo xtask settle --check --base 4762316c",
                "policy: cargo deny --locked check",
                "policy: cargo vet --locked",
                "fuzz: skipped (no trigger)",
            ]
        );
    }

    /// C43-p1#2: a re-blessed golden file or a schema runs no test unless the lane names the packages; with a crate
    /// in the diff as well, only nextest fails closed.
    #[test]
    fn workspace_wide_files_need_the_declared_set() {
        let d = diff(
            &[
                "golden/J-E-2416/x.csv",
                "schemas/0.1/a.json",
                "content/l/b.toml",
                "clippy.toml",
            ],
            &[],
        );
        assert_eq!(
            lines(&d)[3],
            "nextest: FAILED (workspace-wide files changed: golden/J-E-2416/x.csv, schemas/0.1/a.json, content/l/b.toml \
             and 1 more; name the packages they reach with -p)"
        );
        let mixed = diff(&["crates/ew-leaf/src/lib.rs", "golden/J-E-2416/x.csv"], &[]);
        let l = lines(&mixed);
        assert_eq!(l[0], "fmt: cargo fmt -p ew-leaf");
        assert_eq!(
            l[3],
            "nextest: FAILED (workspace-wide files changed: golden/J-E-2416/x.csv; name the packages they reach \
             with -p)"
        );
        assert_eq!(
            &lines_with(&diff(&["golden/J-E-2416/x.csv"], &[]), &["ew-api"])[..4],
            [
                "fmt: cargo fmt -p ew-api",
                "clippy: cargo clippy -p ew-api --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-api --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-api --all-features",
            ]
        );
    }

    /// C43-p1#4: the declared set joins every cargo step, and a declared package runs whole even where a trigger
    /// alone would restrict it to some binaries.
    #[test]
    fn the_declared_set_joins_the_derived_set() {
        let l = lines_with(
            &diff(&["crates/ew-commands/src/wire.rs"], &[]),
            &["ew-testkit", "ew-leaf"],
        );
        assert_eq!(
            &l[..4],
            [
                "fmt: cargo fmt -p ew-commands -p ew-leaf -p ew-testkit",
                "clippy: cargo clippy -p ew-commands -p ew-leaf -p ew-testkit --all-targets --all-features -- -D \
                 warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-commands -p ew-leaf -p ew-testkit \
                 --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p elektrawatt -p ew -p ew-api -p ew-commands \
                 -p ew-leaf -p ew-testkit --all-features",
            ]
        );
        // At a lane's start the diff is empty: the declared set alone.
        let p = plan(
            &diff(&[], &[]),
            &members(),
            &BTreeSet::new(),
            BASE,
            &set(&["ew-viewmodel"]),
        );
        assert_eq!(
            warm_command(&p.nextest_set, &members()).to_string(),
            "cargo test --no-run -p ew-viewmodel --all-features --features ew-viewmodel/testkit"
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

    /// Ruling 4 (RD43 R18a): `map --check` runs when the diff has a Rust file, a `Cargo.toml` or the dependency
    /// rules, and not for documents alone.
    #[test]
    fn the_marker_check_runs_with_rust_files_or_a_manifest() {
        let map = Cmd::xtask(&["map", "--check"]);
        for files in [
            &["crates/ew-leaf/src/lib.rs"][..],
            &["crates/ew-leaf/Cargo.toml"],
            &["ci/dep-rules.toml"],
        ] {
            assert!(policy_commands(&diff(files, &[])).contains(&map), "{files:?}");
        }
        assert_eq!(
            lines(&diff(&["docs/design/04-architecture.md"], &[]))[5],
            "policy: skipped (no trigger)"
        );
    }

    /// Ruling 5: the fmt step runs settle's per-file rustfmt over the diff's `.rs` files, which reaches a test module
    /// that `automod::dir!` includes (no `mod` line names it, so `cargo fmt` does not see it), and never a module under
    /// `#[rustfmt::skip]`.
    #[test]
    fn the_fmt_step_formats_the_automod_modules_of_the_diff() {
        let root = crate::workspace_root();
        let automod = "crates/ew-api/tests/it/report_kinds.rs";
        let skipped = "crates/ew-ui/src/icons/generated.rs";
        let files: BTreeSet<String> = set(&[automod, skipped, "crates/ew-api/src/gone.rs"]);
        let rustfmt = crate::settle::rust_files(&root, &files, false).unwrap();
        assert_eq!(rustfmt, [automod]);
        let mut d = diff(&[automod, skipped], &[]);
        d.rustfmt = rustfmt;
        let p = plan(&d, &members(), &BTreeSet::new(), BASE, &BTreeSet::new());
        assert_eq!(
            p.steps[0],
            StepPlan {
                name: "fmt",
                run: StepRun::Run(vec![
                    Cmd::cargo(&["fmt", "-p", "ew-api", "-p", "ew-ui"]),
                    Cmd::rustfmt(&[automod.to_owned()]),
                ]),
            }
        );
        assert_eq!(
            p.lines()[1],
            "fmt: rustfmt crates/ew-api/tests/it/report_kinds.rs"
        );
    }

    /// C43-p1#5: the fmt step reports the files it rewrote, whole, and is not a failure.
    #[test]
    fn the_fmt_step_reports_the_files_it_rewrote() {
        let snapshot = |pairs: &[(&str, &str)]| -> BTreeMap<String, Vec<u8>> {
            pairs
                .iter()
                .map(|(f, b)| ((*f).to_owned(), b.as_bytes().to_vec()))
                .collect()
        };
        let before = snapshot(&[
            ("a.rs", "fn a(){}"),
            ("b.rs", "fn b() {}\n"),
            ("c.rs", "fn c(){}"),
        ]);
        let after = snapshot(&[
            ("a.rs", "fn a() {}\n"),
            ("b.rs", "fn b() {}\n"),
            ("c.rs", "fn c() {}\n"),
        ]);
        let rewrote = rewrote_files(&before, &after);
        assert_eq!(rewrote, ["a.rs", "c.rs"]);
        let outcome = Outcome {
            name: "fmt",
            status: "rewrote",
            why: Some(rewrote.join(", ")),
            commands: Vec::new(),
            seconds: 0.0,
            log: None,
        };
        assert_eq!(
            summary_line(&outcome),
            "lane-check: fmt: rewrote a.rs, c.rs (commit them)"
        );
        assert!(rewrote_files(&before, &before).is_empty());
    }

    /// C43-p2#6: with `-p`, a changed golden, schema or content file needs every package that names its root in the
    /// set; the nextest step fails closed naming the missing ones, root by root.
    #[test]
    fn a_guarded_root_needs_its_readers_in_the_declared_set() {
        let mut d = diff(
            &[
                "golden/J-E-2416/x.csv",
                "content/l/b.toml",
                "crates/ew-leaf/src/lib.rs",
            ],
            &[],
        );
        d.readers = [
            ("golden/", set(&["ew-api", "ew-testkit", "ew"])),
            ("content/", set(&["ew-api", "ew-ui"])),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            lines_with(&d, &["ew-api"])[3],
            "nextest: FAILED (golden/ changed and ew, ew-testkit name it; content/ changed and ew-ui name it; add \
             them with -p)"
        );
        assert_eq!(
            lines_with(&d, &["ew-api", "ew-testkit", "ew", "ew-ui"])[3],
            "nextest: cargo nextest run --profile lane --cargo-quiet -p ew -p ew-api -p ew-leaf -p ew-testkit -p ew-ui \
             --all-features"
        );
    }

    /// The readers of a root come from a scan of the members' `.rs` files: a path literal names the root, a word in
    /// prose does not, and `xtask` stays out.
    #[test]
    fn the_readers_of_a_root_are_found_by_a_scan() {
        let texts: BTreeMap<&str, &str> = [
            ("crates/ew-api/tests/it/a.rs", r#"repo().join("golden/J-E-2416")"#),
            ("crates/ew-checks/src/lib.rs", "// the golden rule: content first"),
            (
                "crates/ew-format/src/lib.rs",
                r#"concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/0.1/x")"#,
            ),
            ("bin/ew/tests/it/b.rs", r#"root.join("content").join("x")"#),
            ("xtask/src/lane_check.rs", r#""golden/""#),
        ]
        .into_iter()
        .collect();
        let files: Vec<String> = texts.keys().map(|f| (*f).to_owned()).collect();
        let read = |f: &str| texts.get(f).map(|t| (*t).to_owned());
        let found = root_readers(&GUARDED_ROOTS, &files, &members(), &read);
        assert_eq!(
            found,
            [
                ("golden/", set(&["ew-api"])),
                ("schemas/0.1/", set(&["ew-format"])),
                ("content/", set(&["ew"])),
            ]
            .into_iter()
            .collect::<BTreeMap<_, _>>()
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
            parse(&s(&["--base", "main", "-p"])).unwrap_err().to_string(),
            format!("-p needs a package\n{USAGE}")
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
