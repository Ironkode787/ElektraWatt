// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `lane-check --base <rev> [-p <pkg>]... [--only <step>] [--plan] [--warm] [--fresh]
//! [--trailers "<Key>: <items>"]...`: a lane's hand-back checks (`.claude/agents/lane.md`, "Hand-back checks"), run
//! once and chosen from the diff against `<rev>` and the lane's declared package set (`-p`, its edit-loop set).
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
//! - **The steps,** in this order ([`STEPS`]; ID-1103: the two writing steps first, so that a rewrite costs no long
//!   step):
//!   1. `fmt`: `cargo fmt`, then settle's per-file rustfmt over the diff's `.rs` files (`settle::rust_files`: never a
//!      module under `#[rustfmt::skip]`), which reaches the test modules `automod::dir!` includes and `cargo fmt`
//!      does not see. It writes; it is not a check. Its snapshot holds every `.rs` file it can rewrite (FT-1013): the
//!      formatted crates' files, in the diff or not, and every file below the directory of a per-file target (the
//!      child modules rustfmt reaches). When it changed one, it reports `rewrote <files> (commit them)`.
//!   2. `settle`: `settle --base <rev>`, settle's write mode, as a child process of this executable, so a parallel
//!      change of settle is picked up by the build that runs it. Its snapshot is the worktree: each file that differs
//!      from `HEAD`, with a digest of its bytes, before and after (settle's text is not read). When it changed one, it
//!      reports `rewrote <files> (commit them)`; a failure without a rewrite is `FAILED`.
//!   3. `clippy`: the changed crates, `--all-targets --all-features -- -D warnings`. An unresolvable
//!      `disallowed-methods` path in clippy's output fails the step as in `cargo xtask ci`.
//!   4. `doc`: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` over the changed crates, in one call.
//!      `--all-features`, as every cargo command of a lane, so the dependencies' check builds are clippy's. Then
//!      `cargo test --doc --all-features` over those of them that have a library target with doctests (FT-1006).
//!   5. `nextest`: one run of the `lane` profile with `--all-features` over the nextest set (below), with the
//!      `required-features` of each package's test targets named as `affected::commands` names them (redundant
//!      beside `--all-features`, and harmless).
//!   6. `policy`: the checks of lane.md step 6, each by its trigger (see [`policy_commands`]). The cheap static
//!      checks `check-raw-read` and `check-disallowed-methods` run on any changed `.rs` file, since an allow attribute
//!      or a raw-model read is not visible in a path; `check-test-layout` runs on any changed path under a `tests/`
//!      directory and on any changed file that mentions `--test`; `check-unsafe` on any changed `.rs` file,
//!      `Cargo.toml` or `ci/unsafe-allowlist.toml`; `map --check` (the shared-list markers) on any
//!      changed `.rs` file, `Cargo.toml` or `ci/dep-rules.toml`; `api-guard --base <rev>` when one of
//!      [`API_SNAPSHOTS`] changed (FT-1007).
//!   7. `trailers`: in-process over the non-merge commits of `<merge base>..HEAD` ([`trailers::commits`]). It fails
//!      on each record line of a commit's message that stands outside git's trailer block ([`trailers::outside_block`],
//!      FT-1104), with or without `--trailers`. With `--trailers "<Key>: <items>"` (repeatable; the items separated by
//!      commas) it also passes only when every item appears among the comma-separated items of a trailer of its key
//!      (in any case) in some commit of the range, whole or as the first word of one (`Golden: ID-n <reason>`), and
//!      else fails naming each missing pair once, in the order given: `FAILED (missing: Decision: ID-1083;
//!      FT-Closes: FT-231)` ([`missing_trailers`]).
//!   8. `fuzz`: `cargo check` of the fuzz workspace, when `crates/ew-testkit/tests/command_fuzz/**`, `fuzz/**` or the
//!      `Cargo.toml` of a crate in a fuzz target's closure (`ci/fuzz-smoke-targets.toml`) changed.
//! - **A rewrite stops the run** (ID-1103, [`run_steps`]). Once fmt or settle changed the worktree, whether it passed
//!   (`rewrote <files> (commit them)`) or failed (`FAILED (rewrote <files>)`), no later step runs: each reads `not run
//!   (commit the rewrites and run again)`, since it would check a tree that must be committed first, and the run
//!   fails. A failed step does not stop the run, so one run reports every failure.
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
//!   never cut. Then the memory's line and one summary line per step: `lane-check: <step>: ok | rewrote <files>
//!   (commit them) | FAILED | skipped (<why>) | not run (<why>)`, and `target/lane-check.json` (written all or nothing)
//!   with each step's name, status, commands, seconds and log path, and the records of the memory. The durations are
//!   reported, never asserted (owner's rule). The exit code is 0 only when no step failed and none was left unrun (a
//!   rewrite alone, as with `--only fmt`, is no failure).
//! - **The memory** (ID-1077). A step that passed is recorded in `target/lane-check.json`: its name, its exact commands
//!   (program, arguments, environment), the merge base's hash, the tree it ran on (`HEAD^{tree}`) and the inputs
//!   outside the tree that can change its outcome: the environment variables of [`INPUT_ENV_PREFIXES`] (`EW_BLESS`,
//!   `RUSTFLAGS`, `PROPTEST_CASES`, ...), a digest of each cargo configuration file outside the tree, the output of
//!   `rustc -vV` and `cargo -V` (a `rustup override` changes them), and the version of `cargo nextest` or `cargo vet`
//!   when the step runs it. Not held: the contents of folders named by variables, and git-ignored files; run `--fresh`
//!   after changing them. A run on a clean worktree (no tracked change, no untracked file outside
//!   `.gitignore`) skips a step whose record matches and prints it as `ok (from memory; --fresh runs it again)`. A
//!   dirty worktree records nothing and skips nothing; once a step changed the worktree (a rewrite stops the run; any
//!   other change), no later step is skipped or recorded. A failure forgets the step's record. A step that runs `cargo
//!   deny` is never recorded: the advisory database comes from the network. The trailers step is never recorded or
//!   skipped: commit messages change without the tree changing. `--fresh` runs every step and records anew.
//! - **`--plan`** prints the selection, the memory's line and the exact commands (a step it would skip as
//!   `<step>: from memory: <command>`) and runs nothing. **`--only <step>`** runs one step with the same selection.
//!   **`--warm`** builds exactly the nextest step's packages (the derived set and the declared one; at a lane's start
//!   the derived part is empty) with `cargo test --no-run` and the same features, so the later nextest runs reuse the
//!   test binaries; it neither reads nor writes the memory.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use crate::affected::{self, Members};
use crate::trailers::{self, Commit};

const USAGE: &str = "usage: cargo xtask lane-check --base <rev> [-p <pkg>]... [--only <step>] [--plan] [--warm] \
                     [--fresh] [--trailers \"<Key>: <items>\"]...";

/// The steps, in their order (ID-1103): the writing steps first, so that a rewrite stops the run before a long step.
pub(crate) const STEPS: [&str; 8] = [
    "fmt", "settle", "clippy", "doc", "nextest", "policy", "trailers", "fuzz",
];

/// The status of a step that does not run because an earlier step rewrote files.
const NOT_RUN: &str = "not run";

/// Why a step after a rewrite does not run.
const COMMIT_THE_REWRITES: &str = "commit the rewrites and run again";

/// One `--trailers "<Key>: <items>"`: a trailer key and the items that the commits since the merge base must carry
/// under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Wanted {
    /// The trailer key, as given (compared without regard to case).
    pub key: String,
    /// The items, in the order given.
    pub items: Vec<String>,
}

impl fmt::Display for Wanted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.items.join(", "))
    }
}

/// The value of `--trailers`: `<Key>: <item>[, <item>]...`, a key without spaces and at least one item, none empty.
fn parse_wanted(value: &str) -> Option<Wanted> {
    let (key, items) = value.split_once(':')?;
    let key = key.trim();
    let items: Vec<String> = items.split(',').map(|i| i.trim().to_owned()).collect();
    if key.is_empty() || key.contains(char::is_whitespace) || items.iter().any(String::is_empty) {
        return None;
    }
    Some(Wanted {
        key: key.to_owned(),
        items,
    })
}

/// Whether the trailer item `have` carries the wanted item `want`: whole, or as its first word (`Golden: ID-n
/// <reason>` carries `ID-n`); two record IDs compare by kind and number (`FT-07` is `FT-7`).
fn carries(have: &str, want: &str) -> bool {
    let first = have.split_whitespace().next().unwrap_or_default();
    [have, first].into_iter().any(|h| {
        h == want
            || trailers::parse_ref(h)
                .zip(trailers::parse_ref(want))
                .is_some_and(|(a, b)| a == b)
    })
}

/// The pairs `<Key>: <item>` of `wanted` that no commit of `commits` carries, each once, in the order given: an item
/// is carried when a trailer of its key (in any case) holds it among its comma-separated items ([`carries`]), in
/// any commit of the range, so the items of one key may be split across commits.
pub(crate) fn missing_trailers(wanted: &[Wanted], commits: &[Commit]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in wanted {
        for item in &w.items {
            let carried = commits
                .iter()
                .flat_map(|c| &c.trailers)
                .filter(|(k, _)| k.eq_ignore_ascii_case(&w.key))
                .flat_map(|(_, v)| v.split(','))
                .any(|have| carries(have.trim(), item));
            let pair = format!("{}: {item}", w.key);
            if !carried && !out.contains(&pair) {
                out.push(pair);
            }
        }
    }
    out
}

/// The trailers step's status and reason from the record lines it found outside a trailer block
/// ([`trailers::outside_block`]) and the pairs it found missing ([`missing_trailers`]).
fn trailers_status(outside: &[String], missing: &[String]) -> (&'static str, Option<String>) {
    let mut why: Vec<String> = outside.to_vec();
    if !missing.is_empty() {
        why.push(format!("missing: {}", missing.join("; ")));
    }
    if why.is_empty() {
        ("ok", None)
    } else {
        ("FAILED", Some(why.join("; ")))
    }
}

/// What the trailers step checks, as the plan prints it.
fn trailers_text(wanted: &[Wanted]) -> String {
    let mut text = "the commits since the merge base keep their record lines in the trailer block".to_owned();
    if !wanted.is_empty() {
        let wanted: Vec<String> = wanted.iter().map(ToString::to_string).collect();
        text.push_str(&format!(" and carry {}", wanted.join("; ")));
    }
    text
}

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

/// Whether `text` names the repository root `root` as a path: a literal that starts with it or holds it after a `/`
/// (`"golden/x`, `"../../golden/x`, `"../../golden"`, `"schemas/0.1"`), or a path join of its first directory:
/// `join("golden")` for a one-directory root; for a deeper one, a parent-relative join of the root itself
/// (`"../../schemas")` or `"/../../schemas")`, the rest of the path joined after it), so that neither a crate's own
/// `join("schemas")` nor another directory of that name (`"../../crates/ew-commands/schemas")`) counts (C43-p7#6). A
/// bare word in a string (`Fixed("golden")`) is no path.
fn names_root(text: &str, root: &str) -> bool {
    let bare = root.trim_end_matches('/');
    let mut patterns = vec![format!("\"{bare}/"), format!("/{bare}/"), format!("/{bare}\"")];
    let first = match bare.split_once('/') {
        None => {
            patterns.push(format!("join(\"{bare}\")"));
            None
        }
        Some((first, _)) => {
            patterns.push(format!("\"{bare}\""));
            Some(first)
        }
    };
    patterns.iter().any(|p| text.contains(p.as_str())) || first.is_some_and(|f| parent_relative_join(text, f))
}

/// Whether `text` holds `"../{dir}")`, `"/../{dir}")` or the like with any number of `../` (at least one) and
/// nothing else between the quote and `dir`.
fn parent_relative_join(text: &str, dir: &str) -> bool {
    let needle = format!("/{dir}\")");
    text.match_indices(needle.as_str()).any(|(i, _)| {
        let mut before = &text[..=i];
        let mut ups = 0;
        while let Some(rest) = before.strip_suffix("../") {
            before = rest;
            ups += 1;
        }
        ups > 0 && (before.ends_with('"') || before.ends_with("\"/"))
    })
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
    /// Checks in-process that the commits since the merge base carry these trailer items (`--trailers`, ID-1103).
    Trailers(Vec<Wanted>),
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
    /// The plan's lines with nothing remembered.
    #[cfg(test)]
    pub(crate) fn lines(&self) -> Vec<String> {
        self.lines_remembering(&BTreeSet::new())
    }

    /// One line per step: `<step>: <command>` per command (the nextest step's touched files first, as
    /// `nextest: touch <file>`), `<step>: from memory: <command>` per command of a step in `remembered` (one that a
    /// run would skip, ID-1077), `trailers: the commits since the merge base carry <Key>: <items>[; ...]`,
    /// `<step>: skipped (<why>)` or `<step>: FAILED (<why>)`.
    pub(crate) fn lines_remembering(&self, remembered: &BTreeSet<&str>) -> Vec<String> {
        let mut out = Vec::new();
        for s in &self.steps {
            match &s.run {
                StepRun::Run(cmds) if remembered.contains(s.name) => {
                    out.extend(cmds.iter().map(|c| format!("{}: from memory: {c}", s.name)));
                }
                StepRun::Run(cmds) => {
                    if s.name == "nextest" {
                        out.extend(self.touch.iter().map(|t| format!("nextest: touch {t}")));
                    }
                    out.extend(cmds.iter().map(|c| format!("{}: {c}", s.name)));
                }
                StepRun::Trailers(wanted) => out.push(format!("{}: {}", s.name, trailers_text(wanted))),
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

/// The published-surface snapshots whose change runs `api-guard` (FT-1007).
pub(crate) const API_SNAPSHOTS: [&str; 2] = ["bin/ew/api/cli.json", "crates/ew-sqlview/api/api_v1.json"];

/// The policy checks of lane.md step 6 that the diff triggers, in lane.md's order, then `api-guard --base <base>`
/// when the diff holds one of [`API_SNAPSHOTS`] (FT-1007), then the dependency checks.
pub(crate) fn policy_commands(diff: &Diff, base: &str) -> Vec<Cmd> {
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
    if any(&|f| f.ends_with(".rs") || manifest(f) || f == "ci/unsafe-allowlist.toml") {
        cmds.push(Cmd::xtask(&["check-unsafe"]));
    }
    if tests {
        cmds.push(Cmd::xtask(&["check-test-layout"]));
    }
    if markers {
        cmds.push(Cmd::xtask(&["map", "--check"]));
    }
    if any(&|f| API_SNAPSHOTS.contains(&f)) {
        cmds.push(Cmd::xtask(&["api-guard", "--base", base]));
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

/// The plan for `diff` against `base`, with the lane's declared package set (`-p`, the edit-loop set) and the trailer
/// items its commits must carry (`--trailers`).
pub(crate) fn plan(
    diff: &Diff,
    members: &Members,
    fuzz_closure: &BTreeSet<String>,
    base: &str,
    declared: &BTreeSet<String>,
    wanted: &[Wanted],
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
    // FT-1006: the doctests of the documented crates that have a library target.
    let doctested: BTreeSet<String> = crates
        .iter()
        .filter(|p| members.get(*p).is_some_and(|m| m.doctests))
        .cloned()
        .collect();
    let mut docs = vec![doc];
    if !doctested.is_empty() {
        docs.push(
            Cmd::cargo(&["test", "--doc"])
                .packages(&doctested)
                .arg("--all-features"),
        );
    }
    let policy = policy_commands(diff, base);

    let mut fmt = vec![Cmd::cargo(&["fmt"]).packages(&crates)];
    fmt.extend(
        diff.rustfmt
            .chunks(crate::settle::RUSTFMT_CHUNK)
            .map(Cmd::rustfmt),
    );
    // ID-1103: the order of `STEPS` (a test keeps the two the same).
    let steps = vec![
        StepPlan {
            name: "fmt",
            run: cargo_step(fmt),
        },
        StepPlan {
            name: "settle",
            run: if diff.files.is_empty() {
                StepRun::Skip("no changed file".to_owned())
            } else {
                StepRun::Run(vec![Cmd::xtask(&["settle", "--base", base])])
            },
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
            run: cargo_step(docs),
        },
        StepPlan {
            name: "nextest",
            run: match wide_without_set {
                Some(why) => StepRun::Fail(why),
                None => cargo_step(vec![nextest]),
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
            name: "trailers",
            // FT-1104: runs without `--trailers` too, for the record lines outside the trailer block.
            run: StepRun::Trailers(wanted.to_vec()),
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
    fresh: bool,
    packages: BTreeSet<String>,
    /// The `--trailers` values, in the order given.
    trailers: Vec<Wanted>,
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
            "--fresh" => out.fresh = true,
            "-p" => {
                out.packages.insert(
                    it.next()
                        .with_context(|| format!("-p needs a package\n{USAGE}"))?
                        .clone(),
                );
            }
            "--trailers" => {
                let value = it
                    .next()
                    .with_context(|| format!("--trailers needs \"<Key>: <items>\"\n{USAGE}"))?;
                let Some(wanted) = parse_wanted(value) else {
                    bail!("--trailers takes \"<Key>: <item>[, <item>]...\", not `{value}`\n{USAGE}");
                };
                out.trailers.push(wanted);
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

/// The `.rs` files the fmt step can rewrite (FT-1013), out of `all` (the tracked and untracked `.rs` files): every
/// file of a formatted crate (`cargo fmt -p` reaches the whole crate, not only the diff), the files of the per-file
/// pass, and every file below the directory of one of those (the child modules rustfmt reaches from it).
fn fmt_scope(
    all: &[String],
    crates: &BTreeSet<String>,
    members: &Members,
    rustfmt: &[String],
) -> Vec<String> {
    let dirs: Vec<&str> = rustfmt
        .iter()
        .filter_map(|f| f.rfind('/').map(|i| &f[..=i]))
        .collect();
    all.iter()
        .filter(|f| {
            rustfmt.contains(f)
                || dirs.iter().any(|d| f.starts_with(d))
                || affected::owner(f, members).is_ok_and(|o| o.is_some_and(|o| crates.contains(o)))
        })
        .cloned()
        .collect()
}

/// The contents of the files that exist, out of `files`: the fmt step's snapshot.
fn rust_snapshot(root: &Path, files: &[String]) -> BTreeMap<String, Vec<u8>> {
    files
        .iter()
        .filter_map(|f| std::fs::read(root.join(f)).ok().map(|b| (f.clone(), b)))
        .collect()
}

/// The settle step's snapshot of the worktree: each file that differs from `HEAD` (tracked, or untracked and not
/// ignored) with a SHA-256 digest of its bytes (`None` when it is gone). Settle can write any of its outputs, so the
/// whole worktree is its scope; a file that is not in a snapshot is at its `HEAD` content there.
fn worktree_snapshot(root: &Path) -> Result<BTreeMap<String, Option<Vec<u8>>>> {
    use sha2::Digest as _;
    Ok(affected::changed_files(root, "HEAD")?
        .into_iter()
        .map(|f| {
            let digest = std::fs::read(root.join(&f))
                .ok()
                .map(|b| sha2::Sha256::digest(&b).to_vec());
            (f, digest)
        })
        .collect())
}

/// The files whose content differs between two snapshots of [`worktree_snapshot`], in either: a file absent from one
/// is at its `HEAD` content there.
fn changed_between(
    before: &BTreeMap<String, Option<Vec<u8>>>,
    after: &BTreeMap<String, Option<Vec<u8>>>,
) -> Vec<String> {
    let files: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    files
        .into_iter()
        .filter(|f| before.get(*f) != after.get(*f))
        .cloned()
        .collect()
}

/// One command as a record holds it: the program, the arguments and the environment it sets.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct RecordedCmd {
    program: String,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

impl From<&Cmd> for RecordedCmd {
    fn from(cmd: &Cmd) -> Self {
        Self {
            program: match cmd.program {
                Program::Cargo => "cargo",
                Program::Xtask => "cargo xtask",
                Program::Rustfmt => "rustfmt",
            }
            .to_owned(),
            args: cmd.args.clone(),
            env: cmd
                .env
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }
}

/// The memory of a step that passed (ID-1077): its exact commands, the merge base and the tree it ran on, and the
/// inputs outside the tree that can change its outcome ([`outside_inputs`]).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    step: String,
    commands: Vec<RecordedCmd>,
    merge_base: String,
    tree: String,
    inputs: BTreeMap<String, String>,
}

/// What a run on a clean worktree compares a record with; `None` on a dirty worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MemoryKey {
    merge_base: String,
    tree: String,
    /// The inputs outside the tree that every step shares ([`outside_inputs`]).
    inputs: BTreeMap<String, String>,
}

/// The environment variables whose names start with one of these are a record's inputs: the project's own switches
/// (`EW_BLESS`, ...), the toolchain's (`RUSTFLAGS`, `RUSTDOCFLAGS`, `RUSTUP_TOOLCHAIN`, ...), cargo's build settings
/// nextest's, and proptest's (`PROPTEST_CASES`, ..., which set its defaults).
const INPUT_ENV_PREFIXES: [&str; 8] = [
    "EW_",
    "RUST",
    "CARGO_BUILD_",
    "CARGO_PROFILE_",
    "CARGO_TARGET_",
    "CARGO_ENCODED_",
    "NEXTEST_",
    "PROPTEST_",
];

/// The toolchain commands whose output is an input of every record: the compiler and cargo that the steps run, as
/// `rustup` resolves them in the worktree (`rust-toolchain.toml`, a `rustup override`, `RUSTUP_TOOLCHAIN`).
const TOOLCHAIN_INPUTS: [Toolchain; 2] = [Toolchain::Rustc, Toolchain::Cargo];

/// One toolchain command of [`TOOLCHAIN_INPUTS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Toolchain {
    /// `rustc -vV` (`RUSTC` when set).
    Rustc,
    /// `cargo -V` (the cargo that runs the steps).
    Cargo,
}

impl Toolchain {
    /// The input's name in a record.
    fn input(self) -> &'static str {
        match self {
            Toolchain::Rustc => "toolchain rustc -vV",
            Toolchain::Cargo => "toolchain cargo -V",
        }
    }

    /// The output of the command in `root`, trimmed; `None` when it cannot run or fails.
    fn output_in(self, root: &Path) -> Option<String> {
        let (program, arg) = match self {
            Toolchain::Rustc => (
                std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned()),
                "-vV",
            ),
            Toolchain::Cargo => (crate::cargo(), "-V"),
        };
        Command::new(program)
            .arg(arg)
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
    }
}

/// The largest `target/lane-check.json` that is read for its records; a larger one holds none.
const MAX_MEMORY_BYTES: u64 = 4 << 20;

/// The inputs outside the tree that every step shares: the environment variables of [`INPUT_ENV_PREFIXES`] (as
/// `env <NAME>`), and a SHA-256 digest of each cargo configuration file that cargo reads (`.cargo/config.toml` or
/// `.cargo/config` in `root` or an ancestor of it, and in `CARGO_HOME`, else the home directory's `.cargo`), as
/// `cargo config <path>`; the one in `root` is in the tree too, which costs nothing; and the output of each
/// [`TOOLCHAIN_INPUTS`] command (`toolchain`; `unknown` when it cannot report one), as `toolchain rustc -vV` and
/// `toolchain cargo -V`.
fn outside_inputs(
    root: &Path,
    env: &[(String, String)],
    toolchain: &mut dyn FnMut(Toolchain) -> Option<String>,
) -> BTreeMap<String, String> {
    use sha2::Digest as _;
    let mut out: BTreeMap<String, String> = env
        .iter()
        .filter(|(k, _)| INPUT_ENV_PREFIXES.iter().any(|p| k.starts_with(p)))
        .map(|(k, v)| (format!("env {k}"), v.clone()))
        .collect();
    for t in TOOLCHAIN_INPUTS {
        let output = toolchain(t).unwrap_or_else(|| "unknown".to_owned());
        out.insert(t.input().to_owned(), output);
    }
    let mut dirs: Vec<std::path::PathBuf> = root.ancestors().map(|d| d.join(".cargo")).collect();
    let home = env
        .iter()
        .find(|(k, _)| k == "CARGO_HOME")
        .map(|(_, v)| std::path::PathBuf::from(v))
        .or_else(|| {
            env.iter()
                .find(|(k, _)| k == "USERPROFILE" || k == "HOME")
                .map(|(_, v)| Path::new(v).join(".cargo"))
        });
    dirs.extend(home);
    for dir in dirs {
        for name in ["config.toml", "config"] {
            let path = dir.join(name);
            if let Ok(bytes) = std::fs::read(&path) {
                let digest: String = sha2::Sha256::digest(&bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                out.insert(format!("cargo config {}", path.display()), digest);
            }
        }
    }
    out
}

/// The external cargo tools whose version is an input of a step that runs them.
const VERSIONED_TOOLS: [&str; 2] = ["nextest", "vet"];

/// Whether a step may be remembered: `cargo deny` reads the advisory database from the network, an input no record
/// holds, so a step that runs it always runs.
fn rememberable(cmds: &[Cmd]) -> bool {
    !cmds
        .iter()
        .any(|c| c.program == Program::Cargo && c.args.first().is_some_and(|a| a == "deny"))
}

/// The record that a pass of `name` with `cmds` under `key` makes, with the versions of the tools it runs
/// (`tool_version`, `None` when the tool cannot report one).
fn record_of(
    name: &str,
    cmds: &[Cmd],
    key: &MemoryKey,
    tool_version: &mut dyn FnMut(&str) -> Option<String>,
) -> Record {
    let mut inputs = key.inputs.clone();
    for cmd in cmds.iter().filter(|c| c.program == Program::Cargo) {
        if let Some(tool) = cmd.args.first().filter(|a| VERSIONED_TOOLS.contains(&a.as_str())) {
            let version = tool_version(tool).unwrap_or_else(|| "unknown".to_owned());
            inputs.insert(format!("cargo {tool} --version"), version);
        }
    }
    Record {
        step: name.to_owned(),
        commands: cmds.iter().map(RecordedCmd::from).collect(),
        merge_base: key.merge_base.clone(),
        tree: key.tree.clone(),
        inputs,
    }
}

/// The records of `target/lane-check.json` by step; none when the file is missing, too large or unreadable.
fn load_records(root: &Path) -> BTreeMap<String, Record> {
    let path = root.join("target/lane-check.json");
    let Ok(meta) = std::fs::metadata(&path) else {
        return BTreeMap::new();
    };
    if meta.len() > MAX_MEMORY_BYTES {
        return BTreeMap::new();
    }
    records_of_json(&std::fs::read_to_string(&path).unwrap_or_default())
}

/// The records of a `target/lane-check.json` text by step: its `records` array, each element that reads as a
/// [`Record`].
fn records_of_json(text: &str) -> BTreeMap<String, Record> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        return BTreeMap::new();
    };
    json.get("records")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| serde_json::from_value::<Record>(r.clone()).ok())
        .map(|r| (r.step.clone(), r))
        .collect()
}

/// Whether the worktree is clean: no tracked change and no untracked file outside `.gitignore`.
fn worktree_clean(root: &Path) -> Result<bool> {
    Ok(affected::git_output(root, &["status", "--porcelain", "-z", "--untracked-files=normal"])?.is_empty())
}

/// The memory key of the worktree at `root`: `None` when it is dirty.
fn memory_key(root: &Path, merge_base: &str) -> Result<Option<MemoryKey>> {
    if !worktree_clean(root)? {
        return Ok(None);
    }
    let tree = String::from_utf8_lossy(&affected::git_output(root, &["rev-parse", "HEAD^{tree}"])?)
        .trim()
        .to_owned();
    let env: Vec<(String, String)> = std::env::vars_os()
        .map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned()))
        .collect();
    Ok(Some(MemoryKey {
        merge_base: merge_base.to_owned(),
        tree,
        inputs: outside_inputs(root, &env, &mut |t| t.output_in(root)),
    }))
}

/// The line that says what the memory does in this run.
fn memory_line(key: Option<&MemoryKey>, fresh: bool) -> String {
    match (key, fresh) {
        (None, _) => {
            "lane-check: memory: the worktree has changes, so no step is skipped or recorded".to_owned()
        }
        (Some(_), true) => {
            "lane-check: memory: --fresh, so every step runs and a pass is recorded anew".to_owned()
        }
        (Some(k), false) => format!(
            "lane-check: memory: the worktree is clean (tree {}), so a step that passed on it with the same commands \
             is skipped",
            &k.tree[..k.tree.len().min(12)]
        ),
    }
}

/// The `--version` output of `cargo <tool>`, cached per tool.
fn tool_version_in(root: &Path, cache: &mut BTreeMap<String, Option<String>>, tool: &str) -> Option<String> {
    cache
        .entry(tool.to_owned())
        .or_insert_with(|| {
            Command::new(crate::cargo())
                .args([tool, "--version"])
                .current_dir(root)
                .stdin(Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        })
        .clone()
}

/// The outcome of one executed step.
struct Outcome {
    name: &'static str,
    /// `ok`, `FAILED`, `skipped`, `rewrote` (fmt or settle passed and changed files) or `not run` (after a rewrite).
    status: &'static str,
    /// Why it was skipped, failed without running or did not run, the files fmt or settle rewrote, or the trailer
    /// items that are missing.
    why: Option<String>,
    commands: Vec<String>,
    seconds: f64,
    log: Option<String>,
    /// Whether the step changed the worktree (fmt's or settle's rewrite, whether it passed or failed): no later step
    /// runs ([`run_steps`]).
    wrote: bool,
}

impl Outcome {
    /// The outcome of a step that ran no command: `status`, for the reason `why`.
    fn without_command(name: &'static str, status: &'static str, why: &str) -> Self {
        Self {
            name,
            status,
            why: Some(why.to_owned()),
            commands: Vec::new(),
            seconds: 0.0,
            log: None,
            wrote: false,
        }
    }

    /// A writing step's outcome (fmt, settle) with the files it rewrote: none leaves it as it was; else it changed
    /// the worktree, and it reads `rewrote <files>` when it passed, `FAILED (rewrote <files>)` when it failed.
    fn with_rewrites(mut self, rewrote: &[String]) -> Self {
        if rewrote.is_empty() {
            return self;
        }
        self.wrote = true;
        if self.status == "ok" {
            self.status = "rewrote";
            self.why = Some(rewrote.join(", "));
        } else {
            self.why = Some(format!("rewrote {}", rewrote.join(", ")));
        }
        self
    }
}

/// The outcomes of `steps`, each run by `run_step` in order, until one changes the worktree ([`Outcome::wrote`]):
/// each later step is `not run (commit the rewrites and run again)`, since it would check a tree that must be
/// committed first (ID-1103). A failed step does not stop the run, so one run reports every failure.
fn run_steps<'a>(
    steps: impl IntoIterator<Item = &'a StepPlan>,
    mut run_step: impl FnMut(&'a StepPlan) -> Result<Outcome>,
) -> Result<Vec<Outcome>> {
    let mut outcomes: Vec<Outcome> = Vec::new();
    for s in steps {
        let outcome = if outcomes.iter().any(|o| o.wrote) {
            Outcome::without_command(s.name, NOT_RUN, COMMIT_THE_REWRITES)
        } else {
            run_step(s)?
        };
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

/// Whether a run passed: no step `FAILED`, and none `not run` after a rewrite.
fn passed(outcomes: &[Outcome]) -> bool {
    outcomes
        .iter()
        .all(|o| o.status != "FAILED" && o.status != NOT_RUN)
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
        wrote: false,
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

/// The text of `target/lane-check.json`: the base, each step's outcome, and the records of the steps that passed.
fn json_text(base: &str, outcomes: &[Outcome], records: &BTreeMap<String, Record>) -> Result<String> {
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
    let records: Vec<&Record> = records.values().collect();
    let json = serde_json::json!({ "base": base, "steps": steps, "records": records });
    Ok(format!("{}\n", serde_json::to_string_pretty(&json)?))
}

/// Prints the summary block (the memory line first, when there is one) and, with `records`, writes
/// `target/lane-check.json` all or nothing.
fn summarise(
    root: &Path,
    base: &str,
    memory: Option<&str>,
    outcomes: &[Outcome],
    records: Option<&BTreeMap<String, Record>>,
) -> Result<bool> {
    println!();
    if let Some(line) = memory {
        println!("{line}");
    }
    for o in outcomes {
        println!("{}", summary_line(o));
    }
    if let Some(records) = records {
        std::fs::create_dir_all(root.join("target"))?;
        crate::settle::write_atomic(
            &root.join("target/lane-check.json"),
            &json_text(base, outcomes, records)?,
        )?;
    }
    Ok(passed(outcomes))
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
    let plan = plan(
        &diff,
        &members,
        &fuzz_closure(root)?,
        &a.base,
        &a.packages,
        &a.trailers,
    );

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
        return summarise(root, &a.base, None, &[outcome], None);
    }

    // ID-1077: the memory of the steps that passed.
    let mut records = load_records(root);
    let mut key = memory_key(root, &merge_base)?;
    let mut versions = BTreeMap::new();
    let mut tool_version = |tool: &str| tool_version_in(root, &mut versions, tool);
    let mut memory = vec![memory_line(key.as_ref(), a.fresh)];

    if a.plan {
        let remembered: BTreeSet<&str> = plan
            .steps
            .iter()
            .filter(|s| remembered_step(&records, s, key.as_ref(), a.fresh, &mut tool_version))
            .map(|s| s.name)
            .collect();
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
        println!("{}", memory[0]);
        for line in plan.lines_remembering(&remembered) {
            let step = line.split(':').next().unwrap_or_default();
            if a.only.is_none_or(|o| o == step) {
                println!("{line}");
            }
        }
        return Ok(true);
    }

    let selected = plan.steps.iter().filter(|s| a.only.is_none_or(|o| o == s.name));
    let outcomes = run_steps(selected, |s| {
        let outcome = match &s.run {
            StepRun::Skip(why) => Outcome::without_command(s.name, "skipped", why),
            StepRun::Fail(why) => Outcome::without_command(s.name, "FAILED", why),
            StepRun::Run(cmds) if remembered_step(&records, s, key.as_ref(), a.fresh, &mut tool_version) => {
                Outcome {
                    commands: cmds.iter().map(ToString::to_string).collect(),
                    ..Outcome::without_command(s.name, "ok", FROM_MEMORY)
                }
            }
            StepRun::Run(cmds) => {
                if s.name == "nextest" {
                    touch(root, &plan.touch)?;
                }
                // The writing steps' snapshots: fmt's of the `.rs` files it can rewrite (FT-1013), settle's of the
                // worktree (ID-1103).
                let scope = if s.name == "fmt" {
                    let all = affected::nul_paths(&affected::git_output(
                        root,
                        &["ls-files", "-co", "--exclude-standard", "-z", "--", "*.rs"],
                    )?);
                    fmt_scope(&all, &plan.crates, &members, &diff.rustfmt)
                } else {
                    Vec::new()
                };
                let before_fmt = (s.name == "fmt").then(|| rust_snapshot(root, &scope));
                let before_settle = if s.name == "settle" {
                    Some(worktree_snapshot(root)?)
                } else {
                    None
                };
                let outcome = execute(root, s.name, cmds)?;
                let rewrote = match (before_fmt, before_settle) {
                    (Some(before), _) => rewrote_files(&before, &rust_snapshot(root, &scope)),
                    (_, Some(before)) => changed_between(&before, &worktree_snapshot(root)?),
                    _ => Vec::new(),
                };
                let outcome = outcome.with_rewrites(&rewrote);
                if outcome.status == "FAILED" {
                    print_failure(root, &outcome);
                }
                // A rewrite by fmt or settle, or any other change of the worktree, ends the memory for the later
                // steps.
                if key.is_some() && (outcome.wrote || !worktree_clean(root)?) {
                    key = None;
                    memory.push(changed_worktree_line(s.name));
                }
                record_step(&mut records, s, outcome.status, key.as_ref(), &mut tool_version);
                outcome
            }
            StepRun::Trailers(wanted) => {
                let start = Instant::now();
                let (status, why) = match trailers::commits(root, &format!("{merge_base}..HEAD")) {
                    Ok(commits) => trailers_status(
                        &trailers::outside_block(&commits),
                        &missing_trailers(wanted, &commits),
                    ),
                    Err(e) => (
                        "FAILED",
                        Some(format!("could not read the commits since the merge base: {e:#}")),
                    ),
                };
                // Never recorded: the commit messages change without the tree changing.
                record_step(&mut records, s, status, key.as_ref(), &mut tool_version);
                Outcome {
                    name: s.name,
                    status,
                    why,
                    commands: vec![trailers_text(wanted)],
                    seconds: start.elapsed().as_secs_f64(),
                    log: None,
                    wrote: false,
                }
            }
        };
        Ok(outcome)
    })?;
    summarise(root, &a.base, Some(&memory.join("\n")), &outcomes, Some(&records))
}

/// The reason of a step skipped from memory.
const FROM_MEMORY: &str = "from memory; --fresh runs it again";

/// The memory line of a step that changed the worktree.
fn changed_worktree_line(step: &str) -> String {
    format!("lane-check: memory: {step} changed the worktree, so no later step is skipped or recorded")
}

/// Whether a run skips `step` from memory: a step that runs commands as [`remembered`] says; never the trailers step,
/// whose input (the commit messages) changes without the tree changing (ID-1103).
fn remembered_step(
    records: &BTreeMap<String, Record>,
    step: &StepPlan,
    key: Option<&MemoryKey>,
    fresh: bool,
    tool_version: &mut dyn FnMut(&str) -> Option<String>,
) -> bool {
    match &step.run {
        StepRun::Run(cmds) => remembered(records, step.name, cmds, key, fresh, tool_version),
        StepRun::Trailers(_) | StepRun::Skip(_) | StepRun::Fail(_) => false,
    }
}

/// After `step` ran with `status`: a step that runs commands updates the memory as [`update_memory`] says; the
/// trailers step is never recorded (ID-1103).
fn record_step(
    records: &mut BTreeMap<String, Record>,
    step: &StepPlan,
    status: &str,
    key: Option<&MemoryKey>,
    tool_version: &mut dyn FnMut(&str) -> Option<String>,
) {
    match &step.run {
        StepRun::Run(cmds) => update_memory(records, step.name, cmds, status, key, tool_version),
        StepRun::Trailers(_) | StepRun::Skip(_) | StepRun::Fail(_) => {}
    }
}

/// Whether a run skips the step `name` with `cmds`: not with `--fresh` or on a dirty worktree (`key` is `None`), never
/// a step that is not [`rememberable`], else when its record matches what a pass now would record.
fn remembered(
    records: &BTreeMap<String, Record>,
    name: &str,
    cmds: &[Cmd],
    key: Option<&MemoryKey>,
    fresh: bool,
    tool_version: &mut dyn FnMut(&str) -> Option<String>,
) -> bool {
    let Some(key) = key else { return false };
    if fresh || !rememberable(cmds) {
        return false;
    }
    let Some(record) = records.get(name) else {
        return false;
    };
    *record == record_of(name, cmds, key, tool_version)
}

/// After the step `name` ran: a pass is recorded while `key` holds (a clean worktree, no fmt rewrite before) and the
/// step is [`rememberable`]; a failure forgets the step's record.
fn update_memory(
    records: &mut BTreeMap<String, Record>,
    name: &str,
    cmds: &[Cmd],
    status: &str,
    key: Option<&MemoryKey>,
    tool_version: &mut dyn FnMut(&str) -> Option<String>,
) {
    match (status, key) {
        ("FAILED", _) => {
            records.remove(name);
        }
        ("ok", Some(key)) if rememberable(cmds) => {
            records.insert(name.to_owned(), record_of(name, cmds, key, tool_version));
        }
        _ => {}
    }
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
            doctests: matches!(dir, "crates/ew-api" | "crates/ew-format"),
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
        plan(d, &members(), &set(&["ew-format"]), BASE, &set(declared), &[]).lines()
    }

    #[test]
    fn a_docs_only_diff_skips_the_cargo_steps_and_runs_settle() {
        assert_eq!(
            lines(&diff(&["docs/plan/01-global-plan.md", "README.md"], &[])),
            [
                "fmt: skipped (no crate in the diff)",
                "settle: cargo xtask settle --base 4762316c",
                "clippy: skipped (no crate in the diff)",
                "doc: skipped (no crate in the diff)",
                "nextest: skipped (no crate in the diff)",
                "policy: skipped (no trigger)",
                "trailers: the commits since the merge base keep their record lines in the trailer block",
                "fuzz: skipped (no trigger)",
            ]
        );
    }

    #[test]
    fn an_empty_diff_skips_everything() {
        let l = lines(&diff(&[], &[]));
        assert_eq!(l[0], "fmt: skipped (no changed file)");
        assert_eq!(l[1], "settle: skipped (no changed file)");
    }

    #[test]
    fn a_leaf_crate_runs_its_own_steps_and_the_static_checks() {
        assert_eq!(
            lines(&diff(&["crates/ew-leaf/src/lib.rs"], &[])),
            [
                "fmt: cargo fmt -p ew-leaf",
                "settle: cargo xtask settle --base 4762316c",
                "clippy: cargo clippy -p ew-leaf --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-leaf --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-leaf --all-features",
                "policy: cargo xtask check-raw-read",
                "policy: cargo xtask check-disallowed-methods",
                "policy: cargo xtask check-unsafe",
                "policy: cargo xtask map --check",
                "trailers: the commits since the merge base keep their record lines in the trailer block",
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
            l[4],
            "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-checks -p ew-commands -p ew-model \
             --all-features"
        );
        assert_eq!(
            l[2],
            "clippy: cargo clippy -p ew-model --all-targets --all-features -- -D warnings"
        );
    }

    /// The shell: ew-viewmodel's dependants (ew-ui, and elektrawatt by its dev edge) plus `ew` by lane.md's trigger,
    /// with the viewmodel's test features.
    #[test]
    fn the_shell_adds_the_binaries_and_its_test_features() {
        let l = lines(&diff(&["crates/ew-viewmodel/src/strings.rs"], &[]));
        assert_eq!(
            l[4],
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
            l[4],
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
                "settle: cargo xtask settle --base 4762316c",
                "clippy: cargo clippy -p ew-leaf --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-leaf --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-leaf --all-features",
                "policy: cargo xtask check-deps",
                "policy: cargo xtask check-features",
                "policy: cargo xtask check-unsafe",
                "policy: cargo xtask map --check",
                "policy: cargo deny --locked check",
                "policy: cargo vet --locked",
                "trailers: the commits since the merge base keep their record lines in the trailer block",
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
        let p = plan(&d, &members(), &BTreeSet::new(), BASE, &BTreeSet::new(), &[]);
        assert_eq!(p.touch, ["crates/ew-leaf/tests/it/main.rs"]);
        assert_eq!(
            &p.lines()[4..],
            [
                "nextest: touch crates/ew-leaf/tests/it/main.rs",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-leaf --all-features",
                "policy: cargo xtask check-licence-map",
                "policy: cargo xtask check-raw-read",
                "policy: cargo xtask check-disallowed-methods",
                "policy: cargo xtask check-unsafe",
                "policy: cargo xtask check-test-layout",
                "policy: cargo xtask map --check",
                "trailers: the commits since the merge base keep their record lines in the trailer block",
                "fuzz: skipped (no trigger)",
            ]
        );
        // A changed (not added) file touches nothing.
        let d = diff(&["crates/ew-leaf/tests/it/new_case.rs"], &[]);
        assert!(
            plan(&d, &members(), &BTreeSet::new(), BASE, &BTreeSet::new(), &[])
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
                "settle: cargo xtask settle --base 4762316c",
                "clippy: cargo clippy -p ew-testkit --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-testkit --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-api -p ew-testkit --all-features",
                "policy: cargo xtask check-raw-read",
                "policy: cargo xtask check-disallowed-methods",
                "policy: cargo xtask check-unsafe",
                "policy: cargo xtask check-test-layout",
                "policy: cargo xtask map --check",
                "trailers: the commits since the merge base keep their record lines in the trailer block",
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
                "settle: cargo xtask settle --base 4762316c",
                "clippy: skipped (no crate in the diff)",
                "doc: skipped (no crate in the diff)",
                "nextest: FAILED (workspace-wide files changed: Cargo.lock; name the packages they reach with -p)",
                "policy: cargo deny --locked check",
                "policy: cargo vet --locked",
                "trailers: the commits since the merge base keep their record lines in the trailer block",
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
            lines(&d)[4],
            "nextest: FAILED (workspace-wide files changed: golden/J-E-2416/x.csv, schemas/0.1/a.json, content/l/b.toml \
             and 1 more; name the packages they reach with -p)"
        );
        let mixed = diff(&["crates/ew-leaf/src/lib.rs", "golden/J-E-2416/x.csv"], &[]);
        let l = lines(&mixed);
        assert_eq!(l[0], "fmt: cargo fmt -p ew-leaf");
        assert_eq!(
            l[4],
            "nextest: FAILED (workspace-wide files changed: golden/J-E-2416/x.csv; name the packages they reach \
             with -p)"
        );
        assert_eq!(
            &lines_with(&diff(&["golden/J-E-2416/x.csv"], &[]), &["ew-api"])[..6],
            [
                "fmt: cargo fmt -p ew-api",
                "settle: cargo xtask settle --base 4762316c",
                "clippy: cargo clippy -p ew-api --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-api --all-features",
                "doc: cargo test --doc -p ew-api --all-features",
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
            &l[..5],
            [
                "fmt: cargo fmt -p ew-commands -p ew-leaf -p ew-testkit",
                "settle: cargo xtask settle --base 4762316c",
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
            &[],
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
        assert_eq!(policy_commands(&d, BASE), [Cmd::xtask(&["check-test-layout"])]);
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
            assert!(
                policy_commands(&diff(files, &[]), BASE).contains(&map),
                "{files:?}"
            );
        }
        assert_eq!(
            lines(&diff(&["docs/design/04-architecture.md"], &[]))[5],
            "policy: skipped (no trigger)"
        );
    }

    /// Ruling 6 of ID-1218: `check-unsafe` runs when the diff has a Rust file, a `Cargo.toml` or the allowlist, and
    /// not for documents alone.
    #[test]
    fn the_unsafe_check_runs_with_rust_files_a_manifest_or_its_allowlist() {
        let check = Cmd::xtask(&["check-unsafe"]);
        for files in [
            &["crates/ew-leaf/src/lib.rs"][..],
            &["crates/ew-leaf/Cargo.toml"],
            &["ci/unsafe-allowlist.toml"],
        ] {
            assert!(
                policy_commands(&diff(files, &[]), BASE).contains(&check),
                "{files:?}"
            );
        }
        assert!(
            !policy_commands(&diff(&["docs/design/18-quality-security-release.md"], &[]), BASE)
                .contains(&check)
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
        let p = plan(&d, &members(), &BTreeSet::new(), BASE, &BTreeSet::new(), &[]);
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
            wrote: false,
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
            lines_with(&d, &["ew-api"])[5],
            "nextest: FAILED (golden/ changed and ew, ew-testkit name it; content/ changed and ew-ui name it; add \
             them with -p)"
        );
        assert_eq!(
            lines_with(&d, &["ew-api", "ew-testkit", "ew", "ew-ui"])[5],
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
            // ew-schema's form: a parent-relative join, then the format version as a constant.
            (
                "crates/ew-leaf/tests/it/schemas.rs",
                r#"dir.join("../../schemas").join(FORMAT)"#,
            ),
            // A word in a string literal and a crate's own `schemas` directory are no repository roots.
            (
                "crates/ew-model/src/lib.rs",
                r#"Fixed("golden"); overrides_with = "content""#,
            ),
            (
                "crates/ew-commands/tests/schemas.rs",
                r#"PathBuf::from(dir).join("schemas")"#,
            ),
            ("xtask/src/lane_check.rs", r#""golden/""#),
            // C43-p7#6: another crate's `schemas` directory is not the root.
            (
                "bin/ew/tests/it/conventions.rs",
                r#"Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/ew-commands/schemas")"#,
            ),
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
                ("schemas/0.1/", set(&["ew-format", "ew-leaf"])),
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
                "unknown step `lint`; the steps are fmt, settle, clippy, doc, nextest, policy, trailers, fuzz\n\
                 {USAGE}"
            )
        );
        assert_eq!(
            parse(&s(&["--base", "main", "--only", "trailers"])).unwrap().only,
            Some("trailers")
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
        assert_eq!(
            parse(&s(&["--base", "main", "--fresh"])).unwrap(),
            Args {
                base: "main".to_owned(),
                fresh: true,
                ..Args::default()
            }
        );
        assert_eq!(
            USAGE,
            "usage: cargo xtask lane-check --base <rev> [-p <pkg>]... [--only <step>] [--plan] [--warm] [--fresh] \
             [--trailers \"<Key>: <items>\"]..."
        );
    }

    /// ID-1103: `--trailers "<Key>: <items>"` is repeatable and keeps its order; a value without a key or an item is
    /// refused whole, as is an unknown argument.
    #[test]
    fn the_trailers_argument_is_parsed() {
        let s = |a: &[&str]| a.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        let wanted = |key: &str, items: &[&str]| Wanted {
            key: key.to_owned(),
            items: items.iter().map(|i| (*i).to_owned()).collect(),
        };
        assert_eq!(
            parse(&s(&[
                "--base",
                "main",
                "--trailers",
                "Decision: ID-1083",
                "--trailers",
                "FT-Closes:FT-231 ,  FT-232",
            ]))
            .unwrap(),
            Args {
                base: "main".to_owned(),
                trailers: vec![
                    wanted("Decision", &["ID-1083"]),
                    wanted("FT-Closes", &["FT-231", "FT-232"]),
                ],
                ..Args::default()
            }
        );
        assert_eq!(
            parse(&s(&["--base", "main", "--trailers"]))
                .unwrap_err()
                .to_string(),
            format!("--trailers needs \"<Key>: <items>\"\n{USAGE}")
        );
        for bad in [
            "ID-1083",
            ": ID-1083",
            "Decision:",
            "Decision: ID-1, , ID-2",
            "Dec ision: ID-1",
        ] {
            assert_eq!(
                parse(&s(&["--base", "main", "--trailers", bad]))
                    .unwrap_err()
                    .to_string(),
                format!("--trailers takes \"<Key>: <item>[, <item>]...\", not `{bad}`\n{USAGE}"),
                "{bad}"
            );
        }
        assert_eq!(
            parse(&s(&["--base", "main", "--trailer", "Decision: ID-1"]))
                .unwrap_err()
                .to_string(),
            format!("unknown argument `--trailer`\n{USAGE}")
        );
    }

    /// ID-1103: the run order is `STEPS`, by name, the writing steps first and the trailers step after policy.
    #[test]
    fn the_steps_run_in_the_order_of_steps() {
        let wanted = [Wanted {
            key: "Decision".to_owned(),
            items: vec!["ID-1103".to_owned()],
        }];
        for p in [
            plan(
                &diff(&["crates/ew-leaf/src/lib.rs"], &[]),
                &members(),
                &BTreeSet::new(),
                BASE,
                &BTreeSet::new(),
                &wanted,
            ),
            plan(
                &diff(&[], &[]),
                &members(),
                &BTreeSet::new(),
                BASE,
                &BTreeSet::new(),
                &[],
            ),
        ] {
            let names: Vec<&str> = p.steps.iter().map(|s| s.name).collect();
            assert_eq!(names, STEPS);
        }
        assert_eq!(
            STEPS,
            [
                "fmt", "settle", "clippy", "doc", "nextest", "policy", "trailers", "fuzz"
            ]
        );
    }

    fn outcome(name: &'static str, status: &'static str) -> Outcome {
        Outcome {
            name,
            status,
            why: None,
            commands: Vec::new(),
            seconds: 0.0,
            log: None,
            wrote: false,
        }
    }

    /// The summary lines of a run of the leaf crate's plan in which `run_step` decides each step's outcome.
    fn run_lines(run_step: impl FnMut(&StepPlan) -> Result<Outcome>) -> (Vec<String>, bool) {
        let p = plan(
            &diff(&["crates/ew-leaf/src/lib.rs"], &[]),
            &members(),
            &BTreeSet::new(),
            BASE,
            &BTreeSet::new(),
            &[],
        );
        let outcomes = run_steps(&p.steps, run_step).unwrap();
        (outcomes.iter().map(summary_line).collect(), passed(&outcomes))
    }

    /// ID-1103: a rewrite by fmt stops the run: no later step runs, each reads `not run`, and the run fails.
    #[test]
    fn a_rewrite_stops_the_run() {
        let mut ran = Vec::new();
        let (lines, ok) = run_lines(|s| {
            ran.push(s.name);
            Ok(match s.name {
                "fmt" => outcome("fmt", "ok").with_rewrites(&["crates/ew-leaf/src/lib.rs".to_owned()]),
                name => outcome(name, "ok"),
            })
        });
        assert_eq!(ran, ["fmt"]);
        assert_eq!(
            lines,
            [
                "lane-check: fmt: rewrote crates/ew-leaf/src/lib.rs (commit them)",
                "lane-check: settle: not run (commit the rewrites and run again)",
                "lane-check: clippy: not run (commit the rewrites and run again)",
                "lane-check: doc: not run (commit the rewrites and run again)",
                "lane-check: nextest: not run (commit the rewrites and run again)",
                "lane-check: policy: not run (commit the rewrites and run again)",
                "lane-check: trailers: not run (commit the rewrites and run again)",
                "lane-check: fuzz: not run (commit the rewrites and run again)",
            ]
        );
        assert!(!ok);
        // A settle that rewrites and fails stops the run too, and reads FAILED with its files.
        let mut ran = Vec::new();
        let (lines, ok) = run_lines(|s| {
            ran.push(s.name);
            Ok(match s.name {
                "settle" => outcome("settle", "FAILED").with_rewrites(&[
                    "bin/ew/api/cli.json".to_owned(),
                    "crates/ew-commands/schemas/a.json".to_owned(),
                ]),
                name => outcome(name, "ok"),
            })
        });
        assert_eq!(ran, ["fmt", "settle"]);
        assert_eq!(
            &lines[..3],
            [
                "lane-check: fmt: ok",
                "lane-check: settle: FAILED (rewrote bin/ew/api/cli.json, crates/ew-commands/schemas/a.json)",
                "lane-check: clippy: not run (commit the rewrites and run again)",
            ]
        );
        assert!(!ok);
        // A lone rewrite (`--only fmt`) is no failure: nothing was left unrun.
        let only_fmt = [outcome("fmt", "ok").with_rewrites(&["a.rs".to_owned()])];
        assert!(passed(&only_fmt));
        // No rewrite: the outcome is unchanged.
        let same = outcome("settle", "ok").with_rewrites(&[]);
        assert_eq!((same.status, same.why, same.wrote), ("ok", None, false));
    }

    /// ID-1103: a failed step does not stop the run, so one run reports every failure.
    #[test]
    fn a_failed_step_does_not_stop_the_run() {
        let mut ran = Vec::new();
        let (lines, ok) = run_lines(|s| {
            ran.push(s.name);
            Ok(match s.name {
                "settle" | "clippy" => outcome(s.name, "FAILED"),
                name => outcome(name, "ok"),
            })
        });
        assert_eq!(ran, STEPS);
        assert_eq!(
            lines,
            [
                "lane-check: fmt: ok",
                "lane-check: settle: FAILED",
                "lane-check: clippy: FAILED",
                "lane-check: doc: ok",
                "lane-check: nextest: ok",
                "lane-check: policy: ok",
                "lane-check: trailers: ok",
                "lane-check: fuzz: ok",
            ]
        );
        assert!(!ok);
        let (_, ok) = run_lines(|s| Ok(outcome(s.name, "ok")));
        assert!(ok);
    }

    /// The settle step's snapshot difference: a rewritten file, a new file and a file restored to its `HEAD` content
    /// count; an unchanged one does not.
    #[test]
    fn the_settle_snapshot_finds_every_rewrite() {
        let snap = |pairs: &[(&str, Option<&str>)]| -> BTreeMap<String, Option<Vec<u8>>> {
            pairs
                .iter()
                .map(|(f, d)| ((*f).to_owned(), d.map(|d| d.as_bytes().to_vec())))
                .collect()
        };
        let before = snap(&[
            ("a.json", Some("1")),
            ("b.json", Some("2")),
            ("c.json", Some("3")),
        ]);
        let after = snap(&[
            ("a.json", Some("1")),
            ("b.json", Some("9")),
            ("d.json", Some("4")),
        ]);
        assert_eq!(changed_between(&before, &after), ["b.json", "c.json", "d.json"]);
        assert!(changed_between(&before, &before).is_empty());
    }

    fn commit(sha: &str, trailers: &[(&str, &str)]) -> Commit {
        Commit {
            sha: sha.to_owned(),
            subject: "feat: x".to_owned(),
            body: String::new(),
            trailers: trailers
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    /// ID-1103: the trailer match. Items of one key may be split across commits; a key matches in any case; an item
    /// under another key does not count; each missing pair is named once, in the order given.
    #[test]
    fn the_trailer_match() {
        let commits = [
            commit("b", &[("Decision", "ID-1083"), ("FT-Closes", "FT-230")]),
            commit(
                "a",
                &[
                    ("decision", "ID-1084, ID-1085"),
                    ("Golden", "ID-1090 a re-blessed frame, and its sheet"),
                    ("Spec", "docs/plan/decisions.md"),
                ],
            ),
        ];
        let wanted = |key: &str, items: &[&str]| Wanted {
            key: key.to_owned(),
            items: items.iter().map(|i| (*i).to_owned()).collect(),
        };
        // Present: split across two commits, the key in another case, the Golden ID before its reason, and
        // FT-0230 as FT-230.
        assert!(
            missing_trailers(
                &[
                    wanted("Decision", &["ID-1083", "ID-1085"]),
                    wanted("DECISION", &["ID-1084"]),
                    wanted("Golden", &["ID-1090"]),
                    wanted("FT-Closes", &["FT-0230"]),
                ],
                &commits
            )
            .is_empty()
        );
        // Missing: an absent item, and an item that only another key carries.
        let missing = missing_trailers(
            &[
                wanted("Decision", &["ID-1083", "FT-230"]),
                wanted("FT-Closes", &["FT-231"]),
                wanted("Decision", &["FT-230"]),
            ],
            &commits,
        );
        assert_eq!(missing, ["Decision: FT-230", "FT-Closes: FT-231"]);
        let failed = |missing: &[String]| {
            let (status, why) = trailers_status(&[], missing);
            Outcome {
                why,
                ..outcome("trailers", status)
            }
        };
        assert_eq!(
            summary_line(&failed(&[
                "Decision: ID-1083".to_owned(),
                "FT-Closes: FT-231".to_owned()
            ])),
            "lane-check: trailers: FAILED (missing: Decision: ID-1083; FT-Closes: FT-231)"
        );
        assert_eq!(summary_line(&failed(&[])), "lane-check: trailers: ok");
        // FT-1104: a record line outside the trailer block fails the step, with or without wanted items.
        let (status, why) = trailers_status(&["a1 feat: x: `Decision: ID-1` stands outside".to_owned()], &[]);
        assert_eq!(
            (status, why.as_deref()),
            ("FAILED", Some("a1 feat: x: `Decision: ID-1` stands outside"))
        );
        // No commit in the range: every item is missing.
        assert_eq!(
            missing_trailers(&[wanted("Decision", &["ID-1103"])], &[]),
            ["Decision: ID-1103"]
        );
    }

    /// ID-1103, FT-1104: the trailers step's plan lines: the record-line check without `--trailers`, else it and what the
    /// commits must carry.
    #[test]
    fn the_trailers_step_in_the_plan() {
        let d = diff(&["docs/x.md"], &[]);
        assert_eq!(
            lines(&d)[6],
            "trailers: the commits since the merge base keep their record lines in the trailer block"
        );
        let wanted = [
            Wanted {
                key: "Decision".to_owned(),
                items: vec!["ID-1103".to_owned(), "ID-1104".to_owned()],
            },
            Wanted {
                key: "FT-Closes".to_owned(),
                items: vec!["FT-231".to_owned()],
            },
        ];
        let p = plan(&d, &members(), &BTreeSet::new(), BASE, &BTreeSet::new(), &wanted);
        assert_eq!(
            p.lines()[6],
            "trailers: the commits since the merge base keep their record lines in the trailer block and carry Decision: \
             ID-1103, ID-1104; FT-Closes: FT-231"
        );
    }

    /// ID-1103: the memory never records the trailers step and never skips it, whatever its outcome and records.
    #[test]
    fn the_memory_never_records_the_trailers_step() {
        let k = key("feedbeef");
        let mut tool = |_: &str| None;
        let trailers = StepPlan {
            name: "trailers",
            run: StepRun::Trailers(vec![Wanted {
                key: "Decision".to_owned(),
                items: vec!["ID-1103".to_owned()],
            }]),
        };
        let mut records = BTreeMap::new();
        record_step(&mut records, &trailers, "ok", Some(&k), &mut tool);
        assert!(records.is_empty());
        // Even a record under its name (none is ever written) does not skip it.
        let cmds = vec![Cmd::cargo(&["clippy"])];
        records.insert("trailers".to_owned(), record_of("trailers", &cmds, &k, &mut tool));
        assert!(!remembered_step(&records, &trailers, Some(&k), false, &mut tool));
        // A step that runs commands is recorded and skipped through the same calls.
        let clippy = StepPlan {
            name: "clippy",
            run: StepRun::Run(cmds),
        };
        record_step(&mut records, &clippy, "ok", Some(&k), &mut tool);
        assert!(remembered_step(&records, &clippy, Some(&k), false, &mut tool));
    }

    /// C43-p7#6, on the repository itself: ew-schema's `join("../../schemas")` reads the format schemas, `ew`'s join
    /// of `ew-commands`' own `schemas` directory does not.
    #[test]
    fn the_schema_readers_of_the_repository() {
        let root = crate::workspace_root();
        let members = affected::members_of(&crate::metadata_all_features(&root).unwrap()).unwrap();
        let tracked =
            affected::nul_paths(&affected::git_output(&root, &["ls-files", "-z", "--", "*.rs"]).unwrap());
        let read = |f: &str| std::fs::read_to_string(root.join(f)).ok();
        let found = root_readers(&["schemas/0.1/"], &tracked, &members, &read);
        assert!(found["schemas/0.1/"].contains("ew-schema"), "{found:?}");
        assert!(!found["schemas/0.1/"].contains("ew"), "{found:?}");
        assert!(names_root(
            r#"concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas")"#,
            "schemas/0.1/"
        ));
        assert!(!names_root(r#"join("schemas")"#, "schemas/0.1/"));
        assert!(!names_root(r#"join("x/../schemas")"#, "schemas/0.1/"));
    }

    /// FT-1007: a change of a published-surface snapshot runs `api-guard` against the base, in the policy step.
    #[test]
    fn an_api_snapshot_change_runs_the_api_guard() {
        let guard = Cmd::xtask(&["api-guard", "--base", BASE]);
        for file in API_SNAPSHOTS {
            assert_eq!(
                policy_commands(&diff(&[file], &[]), BASE),
                std::slice::from_ref(&guard),
                "{file}"
            );
        }
        assert_eq!(
            lines(&diff(&["bin/ew/api/cli.json"], &[]))[5],
            "policy: cargo xtask api-guard --base 4762316c"
        );
        assert!(!policy_commands(&diff(&["crates/ew-api/api/queries.json"], &[]), BASE).contains(&guard));
    }

    /// FT-1006: the doc step runs the doctests of the documented crates that have a library target, and only those.
    #[test]
    fn the_doc_step_runs_the_doctests_of_the_library_crates() {
        let l = lines_with(&diff(&["crates/ew-api/src/lib.rs"], &[]), &["ew", "ew-format"]);
        assert_eq!(
            &l[3..5],
            [
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew -p ew-api -p ew-format --all-features",
                "doc: cargo test --doc -p ew-api -p ew-format --all-features",
            ]
        );
        // No crate with a library target: no doctest command.
        assert_eq!(
            &lines(&diff(&["crates/ew-leaf/src/lib.rs"], &[]))[3..5],
            [
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-leaf --all-features",
                "nextest: cargo nextest run --profile lane --cargo-quiet -p ew-leaf --all-features",
            ]
        );
    }

    /// FT-1013: the fmt snapshot covers every file of a formatted crate (in the diff or not) and every file below the
    /// directory of a per-file rustfmt target (its child modules), not other crates or directories.
    #[test]
    fn the_fmt_snapshot_covers_every_file_the_step_can_rewrite() {
        let all: Vec<String> = [
            "crates/ew-api/src/lib.rs",
            "crates/ew-api/tests/it/outside_the_diff.rs",
            "crates/ew-leaf/src/lib.rs",
            "fuzz/fuzz_targets/a.rs",
            "fuzz/fuzz_targets/a/child.rs",
            "fuzz/other/c.rs",
        ]
        .iter()
        .map(|f| (*f).to_owned())
        .collect();
        assert_eq!(
            fmt_scope(
                &all,
                &set(&["ew-api"]),
                &members(),
                &["fuzz/fuzz_targets/a.rs".to_owned()]
            ),
            [
                "crates/ew-api/src/lib.rs",
                "crates/ew-api/tests/it/outside_the_diff.rs",
                "fuzz/fuzz_targets/a.rs",
                "fuzz/fuzz_targets/a/child.rs",
            ]
        );
    }

    fn key(tree: &str) -> MemoryKey {
        MemoryKey {
            merge_base: "0123abcd".to_owned(),
            tree: tree.to_owned(),
            inputs: [("env EW_BLESS".to_owned(), "0".to_owned())]
                .into_iter()
                .collect(),
        }
    }

    /// ID-1077: a pass on a clean worktree is recorded, and a later run skips the step while its commands, merge base,
    /// tree and outside inputs (the tool versions included) are the same; `--fresh`, a dirty worktree, a failure and
    /// `cargo deny` defeat it.
    #[test]
    fn a_step_that_passed_on_a_clean_tree_is_remembered() {
        let clippy = vec![Cmd::cargo(&["clippy", "-p", "ew-leaf"])];
        let nextest = vec![Cmd::cargo(&["nextest", "run", "-p", "ew-leaf"])];
        let deny = vec![Cmd::cargo(&["deny", "--locked", "check"])];
        let k = key("feedbeef");
        let mut version = "cargo-nextest 0.9.100".to_owned();
        let mut records = BTreeMap::new();
        {
            let mut tool = |_: &str| Some(version.clone());
            assert!(!remembered(
                &records,
                "clippy",
                &clippy,
                Some(&k),
                false,
                &mut tool
            ));
            // A dirty worktree records nothing.
            update_memory(&mut records, "clippy", &clippy, "ok", None, &mut tool);
            assert!(records.is_empty());
            // fmt's rewrite is no pass.
            update_memory(&mut records, "fmt", &clippy, "rewrote", Some(&k), &mut tool);
            assert!(records.is_empty());
            update_memory(&mut records, "clippy", &clippy, "ok", Some(&k), &mut tool);
            update_memory(&mut records, "nextest", &nextest, "ok", Some(&k), &mut tool);
            update_memory(&mut records, "policy", &deny, "ok", Some(&k), &mut tool);
            assert_eq!(records.keys().collect::<Vec<_>>(), ["clippy", "nextest"]);
            assert_eq!(
                records["nextest"].inputs["cargo nextest --version"],
                "cargo-nextest 0.9.100"
            );

            assert!(remembered(
                &records,
                "clippy",
                &clippy,
                Some(&k),
                false,
                &mut tool
            ));
            assert!(remembered(
                &records,
                "nextest",
                &nextest,
                Some(&k),
                false,
                &mut tool
            ));
            assert!(!remembered(
                &records,
                "clippy",
                &clippy,
                Some(&k),
                true,
                &mut tool
            ));
            assert!(!remembered(&records, "clippy", &clippy, None, false, &mut tool));
            assert!(!remembered(
                &records,
                "clippy",
                &clippy,
                Some(&key("other")),
                false,
                &mut tool
            ));
            let mut other_env = k.clone();
            other_env.inputs.insert("env EW_BLESS".to_owned(), "1".to_owned());
            assert!(!remembered(
                &records,
                "clippy",
                &clippy,
                Some(&other_env),
                false,
                &mut tool
            ));
            let other_cmds = vec![Cmd::cargo(&["clippy", "-p", "ew-api"])];
            assert!(!remembered(
                &records,
                "clippy",
                &other_cmds,
                Some(&k),
                false,
                &mut tool
            ));
        }
        version = "cargo-nextest 0.9.101".to_owned();
        let mut tool = |_: &str| Some(version.clone());
        assert!(!remembered(
            &records,
            "nextest",
            &nextest,
            Some(&k),
            false,
            &mut tool
        ));
        // A failure forgets the step, also on a dirty worktree.
        update_memory(&mut records, "clippy", &clippy, "FAILED", None, &mut tool);
        assert_eq!(records.keys().collect::<Vec<_>>(), ["nextest"]);
    }

    /// The memory's lines and a remembered step's summary and plan lines, whole.
    #[test]
    fn the_memory_lines() {
        let k = key("feedbeef0123456789");
        assert_eq!(
            memory_line(Some(&k), false),
            "lane-check: memory: the worktree is clean (tree feedbeef0123), so a step that passed on it with the same \
             commands is skipped"
        );
        assert_eq!(
            memory_line(Some(&k), true),
            "lane-check: memory: --fresh, so every step runs and a pass is recorded anew"
        );
        assert_eq!(
            memory_line(None, false),
            "lane-check: memory: the worktree has changes, so no step is skipped or recorded"
        );
        assert_eq!(
            changed_worktree_line("fmt"),
            "lane-check: memory: fmt changed the worktree, so no later step is skipped or recorded"
        );
        let outcome = Outcome {
            name: "clippy",
            status: "ok",
            why: Some(FROM_MEMORY.to_owned()),
            commands: Vec::new(),
            seconds: 0.0,
            log: None,
            wrote: false,
        };
        assert_eq!(
            summary_line(&outcome),
            "lane-check: clippy: ok (from memory; --fresh runs it again)"
        );
        let p = plan(
            &diff(&["crates/ew-leaf/src/lib.rs"], &[]),
            &members(),
            &BTreeSet::new(),
            BASE,
            &BTreeSet::new(),
            &[],
        );
        assert_eq!(
            &p.lines_remembering(&["clippy"].into_iter().collect())[..4],
            [
                "fmt: cargo fmt -p ew-leaf",
                "settle: cargo xtask settle --base 4762316c",
                "clippy: from memory: cargo clippy -p ew-leaf --all-targets --all-features -- -D warnings",
                "doc: RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps -p ew-leaf --all-features",
            ]
        );
    }

    /// `target/lane-check.json` keeps its fields and adds the records, which a later run reads back; a text that is
    /// no JSON, or a record of another shape, holds no record.
    #[test]
    fn the_json_holds_the_steps_and_the_records() {
        let mut tool = |_: &str| None;
        let cmds = vec![Cmd {
            env: Some(("RUSTDOCFLAGS", "-D warnings")),
            ..Cmd::cargo(&["doc", "--no-deps"])
        }];
        let records: BTreeMap<String, Record> = [(
            "doc".to_owned(),
            record_of("doc", &cmds, &key("feedbeef"), &mut tool),
        )]
        .into_iter()
        .collect();
        let outcome = Outcome {
            name: "doc",
            status: "ok",
            why: None,
            commands: vec![cmds[0].to_string()],
            seconds: 1.5,
            log: Some("target/lane-check/doc.log".to_owned()),
            wrote: false,
        };
        let text = json_text("main", &[outcome], &records).unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&text).unwrap(),
            serde_json::json!({
                "base": "main",
                "steps": [{
                    "name": "doc",
                    "status": "ok",
                    "why": null,
                    "command": ["RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps"],
                    "seconds": 1.5,
                    "log": "target/lane-check/doc.log",
                }],
                "records": [{
                    "step": "doc",
                    "commands": [{
                        "program": "cargo",
                        "args": ["doc", "--no-deps"],
                        "env": { "RUSTDOCFLAGS": "-D warnings" },
                    }],
                    "merge_base": "0123abcd",
                    "tree": "feedbeef",
                    "inputs": { "env EW_BLESS": "0" },
                }],
            })
        );
        assert_eq!(records_of_json(&text), records);
        assert!(records_of_json("not json").is_empty());
        assert!(records_of_json(r#"{"records": [{"step": "doc"}]}"#).is_empty());
    }

    /// The outside inputs: the environment variables that can change an outcome, not the others, and a digest of a
    /// cargo configuration file in `CARGO_HOME`.
    #[test]
    fn the_outside_inputs_are_the_outcome_variables_and_the_cargo_configs() {
        let dir = std::env::temp_dir().join(format!(
            "ew-xtask-lane-check-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = dir.join("cargo-home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("config.toml"), "abc").unwrap();
        let env: Vec<(String, String)> = [
            ("EW_BLESS", "1"),
            ("RUSTFLAGS", "-C debuginfo=1"),
            ("CARGO_PROFILE_DEV_DEBUG", "line-tables-only"),
            ("PATH", "/bin"),
            ("TMP", "/tmp"),
            ("CARGO_HOME", home.to_str().unwrap()),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
        let inputs = outside_inputs(&dir, &env, &mut |_| None);
        std::fs::remove_dir_all(&dir).unwrap();
        let vars: Vec<(&str, &str)> = inputs
            .iter()
            .filter(|(k, _)| k.starts_with("env "))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            vars,
            [
                ("env CARGO_PROFILE_DEV_DEBUG", "line-tables-only"),
                ("env EW_BLESS", "1"),
                ("env RUSTFLAGS", "-C debuginfo=1"),
            ]
        );
        assert_eq!(
            inputs[&format!("cargo config {}", home.join("config.toml").display())],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// C44-p1#4: the output of `rustc -vV` and `cargo -V` and the `PROPTEST_` variables are inputs, so a changed
    /// compiler (a `rustup override`, say) or `PROPTEST_CASES` makes a record stale.
    #[test]
    fn a_changed_rustc_or_proptest_setting_invalidates_a_record() {
        let root = Path::new("/no/such/worktree");
        let rustc_a = "rustc 1.99.0 (abc 2026-10-01)\nhost: x86_64-pc-windows-msvc";
        let rustc_b = "rustc 1.100.0-nightly (def 2026-10-30)\nhost: x86_64-pc-windows-msvc";
        let env = |cases: &str| -> Vec<(String, String)> {
            vec![
                ("PROPTEST_CASES".to_owned(), cases.to_owned()),
                ("PATH".to_owned(), "/bin".to_owned()),
            ]
        };
        let key_of = |rustc: &str, cases: &str| MemoryKey {
            merge_base: "0123abcd".to_owned(),
            tree: "feedbeef".to_owned(),
            inputs: outside_inputs(root, &env(cases), &mut |t| {
                Some(match t {
                    Toolchain::Rustc => rustc.to_owned(),
                    Toolchain::Cargo => "cargo 1.99.0 (abc 2026-09-30)".to_owned(),
                })
            }),
        };
        let base = key_of(rustc_a, "256");
        let toolchain: Vec<(&str, &str)> = base
            .inputs
            .iter()
            .filter(|(k, _)| !k.starts_with("cargo config "))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            toolchain,
            [
                ("env PROPTEST_CASES", "256"),
                ("toolchain cargo -V", "cargo 1.99.0 (abc 2026-09-30)"),
                ("toolchain rustc -vV", rustc_a),
            ]
        );
        let clippy = vec![Cmd::cargo(&["clippy", "-p", "ew-leaf"])];
        let mut tool = |_: &str| None;
        let records: BTreeMap<String, Record> = [(
            "clippy".to_owned(),
            record_of("clippy", &clippy, &base, &mut tool),
        )]
        .into_iter()
        .collect();
        let skips = |k: &MemoryKey, tool: &mut dyn FnMut(&str) -> Option<String>| {
            remembered(&records, "clippy", &clippy, Some(k), false, tool)
        };
        assert!(skips(&key_of(rustc_a, "256"), &mut tool));
        assert!(!skips(&key_of(rustc_b, "256"), &mut tool));
        assert!(!skips(&key_of(rustc_a, "4096"), &mut tool));
        // A toolchain that cannot report itself is an input too ("unknown"), never a match by default.
        let unknown = MemoryKey {
            inputs: outside_inputs(root, &env("256"), &mut |_| None),
            ..base.clone()
        };
        assert_eq!(unknown.inputs["toolchain rustc -vV"], "unknown");
        assert!(!skips(&unknown, &mut tool));
    }
}
