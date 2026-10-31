// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `settle [--check] [--merge] [--all] [--base <rev>]`: the mechanical upkeep of a change in one command.
//!
//! Every generator of a derived reference file already exists (rustfmt, the bless tests, `docs-gen`, `gen-icons`,
//! `check-raw-read --bless`, the fuzz smoke table, `golden_changes.py`, the blind self-tests); this runs them in one
//! order, driven by one table ([`STEPS`]). Each step names the paths that trigger it, its action and the outputs it may
//! rewrite.
//!
//! - **Triggers.** The files changed since the merge base of `--base` (default `main`) and `HEAD`, uncommitted and
//!   untracked ones included (`affected`'s change detection), plus every file an earlier step rewrote in this run. A
//!   step is triggered by a changed file that matches one of its globs, or that lies in a workspace package its
//!   `deps_of` packages are built from (the package and its normal and build dependencies; a change of the root
//!   `Cargo.toml`, `Cargo.lock` or `rust-toolchain.toml` counts too). `--all` runs every step.
//! - **Outputs.** Before and after each step the changed files of the working tree (against `HEAD`) are hashed; a
//!   step that changes a file outside its listed outputs fails. Settle writes only derived reference files that are
//!   a pure function of the code: never golden outputs, UI snapshots, page output, acceptance or fixture files or the
//!   describe-text expectations, which are decisions a change makes and lists.
//! - **`--check`** writes nothing (a step that writes in this mode fails) and exits 1 when a step would rewrite a
//!   file or fails. It runs no self-test: the verify replays them.
//! - **`--merge`** adds the review gates `check-disallowed-methods --bless` and `check-ordered-collections --bless`,
//!   each printing its allow list's change per crate and the sites added or removed. They never run without it.
//! - **Output.** One line per step, `<step>: unchanged | rewrote <files> | would rewrite <what> | FAILED <reason> |
//!   skipped (no trigger)`, repeated as one block at the end; the exit code is 0 when nothing failed (and, with
//!   `--check`, nothing would change).
//!
//! The outputs that a regeneration step rewrites entirely from its sources carry `merge=regen` in `.gitattributes`
//! (a test keeps the two equal): with the driver configured (`git config merge.regen.driver true`, which keeps one
//! side) a merge never stops on them, and settle then regenerates them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::affected::{self, Members};
use crate::ordered::{self, Rule};

/// The default base revision of the change detection.
const DEFAULT_BASE: &str = "main";
/// The files outside every package that every build reads: a change of one triggers every `deps_of` step.
const BUILD_WIDE: &[&str] = &["Cargo.toml", "Cargo.lock", "rust-toolchain.toml"];
/// The line `settle --check` prints for the self-tests.
const SELFTESTS_NOT_CHECKED: &str = "selftests: not checked here (the verify replays them)";
/// The files rustfmt takes per call (relative paths: a long worktree path must not reach the command line limit).
const RUSTFMT_CHUNK: usize = 32;

/// When a step runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// A regeneration step: it rewrites its outputs (`--check` checks them).
    Regen,
    /// A check: settle reports its failure but cannot repair it.
    CheckOnly,
    /// A review gate that rewrites a list, only with `--merge`.
    MergeOnly,
}

/// What a step does.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    /// rustfmt on the changed `.rs` files (every tracked one with `--all`), per file, never a workspace `cargo fmt`.
    Rustfmt,
    /// A bless test: `<env>=1 cargo nextest run -p <package> --all-features --test <test> [-E <filter>]`; without
    /// the variable (`--check`) the test compares.
    BlessTest {
        package: &'static str,
        test: &'static str,
        filter: Option<&'static str>,
        env: &'static str,
    },
    /// A bless test among the unit tests of a binary: `<env>=1 cargo nextest run -p <package> --all-features --bin
    /// <bin> [-E <filter>]` (the CLI snapshot, whose table is a module of the `ew` binary).
    BlessBin {
        package: &'static str,
        bin: &'static str,
        filter: Option<&'static str>,
        env: &'static str,
    },
    /// An xtask generator `run(root, check)`: `docs-gen`, `gen-icons`.
    Xtask(fn(&Path, bool) -> Result<bool>),
    /// `check-raw-read --bless` (`check-raw-read` with `--check`).
    RawRead,
    /// `fuzz-smoke-table` (FT-335).
    FuzzTable,
    /// `python docs/plan/tools/golden_changes.py --write` (`--check`).
    GoldenChanges,
    /// `blind selftest` or `blind selftest-visual`, which re-bless their recording.
    Selftest { visual: bool },
    /// `symbols-qet10 --check`.
    Symbols,
    /// `cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked`.
    FuzzBuild,
    /// `check-ordered-collections --bless` or `check-disallowed-methods --bless` with the review print.
    AllowList(&'static Rule),
}

/// One step of the table.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Step {
    /// The step's name in the output.
    pub name: &'static str,
    /// When it runs.
    pub phase: Phase,
    /// Globs (`*` within a path segment, `**` across segments) of the changed files that trigger it.
    pub triggers: &'static [&'static str],
    /// Packages whose source and dependencies trigger it (see the module documentation).
    pub deps_of: &'static [&'static str],
    /// Globs of the files it may rewrite or remove.
    pub outputs: &'static [&'static str],
    /// The outputs it rewrites only in part (hand-written lines kept): not `merge=regen`. Read by
    /// `whole_file_outputs` (a test function), which a test compares with `.gitattributes`.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "the table's data for the .gitattributes test")
    )]
    pub partial: &'static [&'static str],
    /// What it does.
    pub action: Action,
}

const COMMAND_SCHEMAS: &str = "crates/ew-commands/schemas/**";
const CLI_SCHEMAS: &str = "bin/ew/schemas/**";
const CATALOGUE: &str = "crates/ew-checks/catalogue/rules.json";
const KEYMAPS: [&str; 3] = [
    "crates/ew-viewmodel/keymaps/ew.toml",
    "crates/ew-viewmodel/keymaps/kicad.toml",
    "crates/ew-viewmodel/keymaps/sheet.json",
];
const BLIND_REFERENCE: &str = "tools/blind/reference/**";
const CLI_SNAPSHOT: &str = "bin/ew/api/cli.json";
const API_V1_SNAPSHOT: &str = "crates/ew-sqlview/api/api_v1.json";
const QUERY_SNAPSHOT: &str = "crates/ew-api/api/queries.json";
const BOOK_PAGES: [&str; 8] = [
    "docs/book/src/reference/commands/**",
    "docs/book/src/reference/rules/**",
    "docs/book/src/reference/glossary.md",
    "docs/book/src/reference/keymaps.md",
    "docs/book/src/reference/cli/**",
    "docs/book/src/reference/queries.md",
    "docs/book/src/reference/api-v1.md",
    "docs/book/src/reference/format/**",
];
const BOOK_SUMMARY: &str = "docs/book/src/SUMMARY.md";
const ICONS: &str = "crates/ew-ui/src/icons/generated.rs";
const CLIPPY_TOML: &str = "clippy.toml";
const FUZZ_TABLE: &str = crate::workflows::FUZZ_TABLE;
const GOLDEN_LIST: &str = "docs/plan/gate-records/golden-change-list.md";
const SELFTEST_RECORDING: &str = "tools/blind/recordings/selftest/**";
const SELFTEST_VISUAL_RECORDING: &str = "tools/blind/recordings/selftest-visual/**";

/// The steps in their order: rustfmt, the command schemas, the CLI schemas, the rule catalogue, the keymaps, the
/// blind reference, the CLI, `api_v1` and query snapshots, `docs-gen`, `gen-icons`, the clippy list, the fuzz table, the golden change list and the two
/// self-tests; then the checks; then the review gates of `--merge`.
pub(crate) const STEPS: &[Step] = &[
    Step {
        name: "rustfmt",
        phase: Phase::Regen,
        triggers: &["**/*.rs"],
        deps_of: &[],
        outputs: &["**/*.rs"],
        partial: &["**/*.rs"],
        action: Action::Rustfmt,
    },
    Step {
        name: "command-schemas",
        phase: Phase::Regen,
        triggers: &[COMMAND_SCHEMAS],
        deps_of: &["ew-commands"],
        outputs: &[COMMAND_SCHEMAS],
        partial: &[],
        action: Action::BlessTest {
            package: "ew-commands",
            test: "schemas",
            filter: None,
            env: "EW_COMMANDS_BLESS",
        },
    },
    Step {
        name: "cli-schemas",
        phase: Phase::Regen,
        triggers: &[CLI_SCHEMAS],
        deps_of: &["ew"],
        outputs: &[CLI_SCHEMAS],
        partial: &[],
        action: Action::BlessTest {
            package: "ew",
            test: "it",
            filter: Some("test(/^conventions::/)"),
            env: "EW_CLI_BLESS",
        },
    },
    Step {
        name: "rule-catalogue",
        phase: Phase::Regen,
        triggers: &[CATALOGUE],
        deps_of: &["ew-checks"],
        outputs: &[CATALOGUE],
        partial: &[],
        action: Action::BlessTest {
            package: "ew-checks",
            test: "catalogue",
            filter: None,
            env: "EW_CHECKS_BLESS",
        },
    },
    Step {
        name: "keymaps",
        phase: Phase::Regen,
        triggers: &KEYMAPS,
        deps_of: &["ew-viewmodel"],
        outputs: &KEYMAPS,
        partial: &[],
        action: Action::BlessTest {
            package: "ew-viewmodel",
            test: "keymaps",
            filter: None,
            env: "EW_BLESS",
        },
    },
    Step {
        name: "blind-reference",
        phase: Phase::Regen,
        triggers: &[BLIND_REFERENCE],
        deps_of: &["ew-viewmodel"],
        outputs: &[BLIND_REFERENCE],
        partial: &[],
        action: Action::BlessTest {
            package: "ew-viewmodel",
            test: "blind_reference",
            filter: None,
            env: "EW_BLESS",
        },
    },
    // WP-S1.22 part 5 (the orchestrator's ruling 2, cycle 42): the published-surface snapshots that the API guard and
    // docs-gen read.
    Step {
        name: "cli-snapshot",
        phase: Phase::Regen,
        triggers: &[CLI_SNAPSHOT],
        deps_of: &["ew"],
        outputs: &[CLI_SNAPSHOT],
        partial: &[],
        action: Action::BlessBin {
            package: "ew",
            bin: "ew",
            filter: Some("test(/^catalogue::/)"),
            env: "EW_CLI_BLESS",
        },
    },
    Step {
        name: "api-v1-snapshot",
        phase: Phase::Regen,
        triggers: &[API_V1_SNAPSHOT],
        deps_of: &["ew-sqlview"],
        outputs: &[API_V1_SNAPSHOT],
        partial: &[],
        action: Action::BlessTest {
            package: "ew-sqlview",
            test: "it",
            filter: Some("test(/^api_v1::/)"),
            env: "EW_BLESS",
        },
    },
    Step {
        name: "query-snapshot",
        phase: Phase::Regen,
        triggers: &[QUERY_SNAPSHOT],
        deps_of: &["ew-api"],
        outputs: &[QUERY_SNAPSHOT],
        partial: &[],
        action: Action::BlessTest {
            package: "ew-api",
            test: "it",
            filter: Some("test(the_query_snapshot_equals_the_registry)"),
            env: "EW_BLESS",
        },
    },
    Step {
        name: "docs-gen",
        phase: Phase::Regen,
        triggers: &[
            COMMAND_SCHEMAS,
            CATALOGUE,
            CLI_SNAPSHOT,
            CLI_SCHEMAS,
            QUERY_SNAPSHOT,
            API_V1_SNAPSHOT,
            "schemas/0.1/**",
            "crates/ew-viewmodel/glossary/**",
            "crates/ew-viewmodel/keymaps/sheet.json",
            "docs/book/src/**",
            "docs/plan/work-packages.csv",
            "docs/plan/01-global-plan.md",
            "docs/plan/03-*.md",
            "xtask/src/docs_gen.rs",
            "xtask/src/plan_counts.rs",
        ],
        deps_of: &[],
        outputs: &[
            BOOK_PAGES[0],
            BOOK_PAGES[1],
            BOOK_PAGES[2],
            BOOK_PAGES[3],
            BOOK_PAGES[4],
            BOOK_PAGES[5],
            BOOK_PAGES[6],
            BOOK_PAGES[7],
            BOOK_SUMMARY,
        ],
        partial: &[BOOK_SUMMARY],
        action: Action::Xtask(crate::docs_gen::run),
    },
    Step {
        name: "gen-icons",
        phase: Phase::Regen,
        triggers: &["assets/icons/**", "xtask/src/icons.rs", ICONS],
        deps_of: &[],
        outputs: &[ICONS],
        partial: &[],
        action: Action::Xtask(crate::icons::run),
    },
    Step {
        name: "clippy-list",
        phase: Phase::Regen,
        triggers: &[
            "**/Cargo.toml",
            "crates/ew-model/**",
            "ci/dep-rules.toml",
            "xtask/src/raw_read.rs",
            CLIPPY_TOML,
        ],
        deps_of: &[],
        outputs: &[CLIPPY_TOML],
        partial: &[],
        action: Action::RawRead,
    },
    Step {
        name: "fuzz-table",
        phase: Phase::Regen,
        triggers: &[
            "fuzz/**",
            "crates/*/Cargo.toml",
            FUZZ_TABLE,
            ".github/workflows/deep-nightly.yml",
            "xtask/src/workflows.rs",
        ],
        deps_of: &[],
        outputs: &[FUZZ_TABLE],
        partial: &[FUZZ_TABLE],
        action: Action::FuzzTable,
    },
    Step {
        name: "golden-change-list",
        phase: Phase::Regen,
        triggers: &[
            "golden/J-E-2416/tables/**",
            "docs/design/20-golden-project.md",
            "docs/plan/06-decision-log.md",
            "docs/plan/tools/golden_changes.py",
            GOLDEN_LIST,
        ],
        deps_of: &[],
        outputs: &[GOLDEN_LIST],
        partial: &[GOLDEN_LIST],
        action: Action::GoldenChanges,
    },
    Step {
        name: "selftest",
        phase: Phase::Regen,
        triggers: &[
            "tools/blind/HELP.md",
            "tools/blind/tasks/selftest/**",
            BLIND_REFERENCE,
            SELFTEST_RECORDING,
            "xtask/src/blind/**",
        ],
        deps_of: &[],
        outputs: &[SELFTEST_RECORDING],
        partial: &[],
        action: Action::Selftest { visual: false },
    },
    Step {
        name: "selftest-visual",
        phase: Phase::Regen,
        triggers: &[
            "tools/blind/HELP-VISUAL.md",
            "tools/blind/tasks/selftest-visual/**",
            BLIND_REFERENCE,
            SELFTEST_VISUAL_RECORDING,
            "xtask/src/blind/**",
        ],
        deps_of: &[],
        outputs: &[SELFTEST_VISUAL_RECORDING],
        partial: &[],
        action: Action::Selftest { visual: true },
    },
    Step {
        name: "symbols-qet10",
        phase: Phase::CheckOnly,
        triggers: &["content/symbols/**", "content/fndefs/**", "xtask/src/symbols.rs"],
        deps_of: &[],
        outputs: &[],
        partial: &[],
        action: Action::Symbols,
    },
    Step {
        name: "fuzz-build",
        phase: Phase::CheckOnly,
        triggers: &[
            "fuzz/**",
            "crates/ew-testkit/tests/command_fuzz/**",
            "Cargo.lock",
            FUZZ_TABLE,
        ],
        deps_of: &[],
        outputs: &[],
        partial: &[],
        action: Action::FuzzBuild,
    },
    Step {
        name: "disallowed-methods",
        phase: Phase::MergeOnly,
        triggers: &[
            "**/*.rs",
            "**/Cargo.toml",
            "ci/disallowed-methods-allow.toml",
            "ci/dep-rules.toml",
        ],
        deps_of: &[],
        outputs: &["ci/disallowed-methods-allow.toml"],
        partial: &[],
        action: Action::AllowList(&ordered::METHODS),
    },
    Step {
        name: "ordered-collections",
        phase: Phase::MergeOnly,
        triggers: &["**/*.rs", "**/Cargo.toml", "ci/ordered-collections-allow.toml"],
        deps_of: &[],
        outputs: &["ci/ordered-collections-allow.toml"],
        partial: &[],
        action: Action::AllowList(&ordered::TYPES),
    },
];

/// The outputs that the regeneration steps rewrite entirely from their sources, in table order: the files that
/// `.gitattributes` gives `merge=regen`.
#[cfg(test)]
pub(crate) fn whole_file_outputs() -> Vec<&'static str> {
    STEPS
        .iter()
        .filter(|s| s.phase == Phase::Regen)
        .flat_map(|s| s.outputs.iter().filter(|o| !s.partial.contains(o)).copied())
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// Globs and triggers

/// Whether `path` (relative, `/`-separated) matches `pattern`: `*` matches within a path segment, a `**` segment
/// any number of segments (none included).
pub(crate) fn glob_match(pattern: &str, path: &str) -> bool {
    fn segments(p: &[&str], s: &[&str]) -> bool {
        match p.split_first() {
            None => s.is_empty(),
            Some((&"**", rest)) => (0..=s.len()).any(|i| segments(rest, &s[i..])),
            Some((first, rest)) => s
                .split_first()
                .is_some_and(|(head, tail)| wild(first.as_bytes(), head.as_bytes()) && segments(rest, tail)),
        }
    }
    fn wild(p: &[u8], s: &[u8]) -> bool {
        match p.split_first() {
            None => s.is_empty(),
            Some((b'*', rest)) => (0..=s.len()).any(|i| wild(rest, &s[i..])),
            Some((c, rest)) => s.split_first().is_some_and(|(h, t)| h == c && wild(rest, t)),
        }
    }
    let p: Vec<&str> = pattern.split('/').collect();
    let s: Vec<&str> = path.split('/').collect();
    segments(&p, &s)
}

/// The packages `names` are built from: themselves and their normal and build dependencies, transitively.
fn build_closure(names: &[&str], members: &Members) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack: Vec<String> = names.iter().map(|n| (*n).to_owned()).collect();
    while let Some(name) = stack.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(m) = members.get(&name) {
            stack.extend(m.deps.iter().cloned());
        }
    }
    seen
}

/// The member a file belongs to: the deepest member directory that holds it.
fn owner<'m>(file: &str, members: &'m Members) -> Option<&'m str> {
    members
        .iter()
        .filter(|(_, m)| {
            file.strip_prefix(m.dir.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(|(_, m)| m.dir.len())
        .map(|(name, _)| name.as_str())
}

/// Whether one of `files` triggers `step`. `members` is needed only for a step with `deps_of`; `None` then counts
/// as triggered (the caller loads the members whenever a step has `deps_of`).
pub(crate) fn triggered(step: &Step, files: &BTreeSet<String>, members: Option<&Members>) -> bool {
    if files
        .iter()
        .any(|f| step.triggers.iter().any(|t| glob_match(t, f)))
    {
        return true;
    }
    if step.deps_of.is_empty() {
        return false;
    }
    let Some(members) = members else {
        return true;
    };
    let closure = build_closure(step.deps_of, members);
    files
        .iter()
        .any(|f| BUILD_WIDE.contains(&f.as_str()) || owner(f, members).is_some_and(|o| closure.contains(o)))
}

// ---------------------------------------------------------------------------------------------------------
// Running

/// Writes `text` to `path` all or nothing: a temporary file beside it, renamed over it; a failure leaves the old
/// file and removes the temporary one.
///
/// # Errors
///
/// A write or a rename that fails.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let name = path
        .file_name()
        .with_context(|| format!("{} names no file", path.display()))?;
    let tmp = path.with_file_name(format!("{}.settle-tmp", name.to_string_lossy()));
    let result = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result.with_context(|| format!("writing {}", path.display()))
}

/// The changed files of the working tree against `HEAD` (tracked and untracked, not ignored) with a digest of
/// each (`None` for a removed file).
fn snapshot(root: &Path) -> Result<BTreeMap<String, Option<[u8; 32]>>> {
    let mut out = BTreeMap::new();
    for rel in affected::changed_files(root, "HEAD")? {
        let digest = std::fs::read(root.join(&rel))
            .ok()
            .map(|bytes| Sha256::digest(&bytes).into());
        out.insert(rel, digest);
    }
    Ok(out)
}

/// The files whose content differs between two snapshots (a file absent from one is at its `HEAD` content there).
fn snapshot_diff(
    before: &BTreeMap<String, Option<[u8; 32]>>,
    after: &BTreeMap<String, Option<[u8; 32]>>,
) -> Vec<String> {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    keys.into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .cloned()
        .collect()
}

/// What a step's action returned: whether it passed, and a detail for the step's line when it did not.
struct Outcome {
    ok: bool,
    detail: String,
}

impl Outcome {
    fn of(ok: bool, detail: &str) -> Outcome {
        Outcome {
            ok,
            detail: detail.to_owned(),
        }
    }
}

/// The last lines of a command's output, for a failure's line.
fn tail(output: &std::process::Output) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(12)..].join("\n    ")
}

/// Runs `command` with its output captured; prints the tail when it fails.
fn captured(mut command: Command, what: &str) -> Result<Outcome> {
    let output = command.output().with_context(|| format!("running {what}"))?;
    if output.status.success() {
        Ok(Outcome::of(true, ""))
    } else {
        Ok(Outcome {
            ok: false,
            detail: format!("{what} failed:\n    {}", tail(&output)),
        })
    }
}

/// The `.rs` files rustfmt takes: the existing changed ones, or every tracked and untracked one with `all`; never a
/// module its parent declares under `#[rustfmt::skip]` (rustfmt given the file itself would not see the attribute,
/// and `cargo fmt` leaves such a module alone).
fn rust_files(root: &Path, files: &BTreeSet<String>, all: bool) -> Result<Vec<String>> {
    let tracked = git_ls_rs(root)?;
    let candidates: Vec<String> = if all {
        affected::changed_files(root, "HEAD")?
            .into_iter()
            .chain(tracked.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    } else {
        files.iter().cloned().collect()
    };
    let mut skipped = Vec::new();
    for parent in &tracked {
        if let Ok(text) = std::fs::read_to_string(root.join(parent)) {
            skipped.extend(skipped_modules(parent, &text));
        }
    }
    Ok(candidates
        .into_iter()
        .filter(|f| f.ends_with(".rs") && root.join(f).is_file())
        .filter(|f| {
            !skipped
                .iter()
                .any(|m| f == &format!("{m}.rs") || f.starts_with(&format!("{m}/")))
        })
        .collect())
}

/// The module paths (without `.rs`, relative to the root) that the file `parent` declares under `#[rustfmt::skip]`
/// (`#[rustfmt::skip]` and then, on the next non-empty line, `mod name;` with any visibility): `dir/name` for a
/// parent `dir/lib.rs`, `dir/main.rs` or `dir/mod.rs`, `dir/stem/name` for a parent `dir/stem.rs`. The module's file
/// is `<path>.rs` or under `<path>/`.
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

fn git_ls_rs(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["ls-files", "-z", "--", "*.rs"])
        .current_dir(root)
        .output()
        .context("running git ls-files")?;
    if !output.status.success() {
        bail!("git ls-files failed: {}", tail(&output));
    }
    Ok(output
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect())
}

/// rustfmt over `rs` (relative to `root`), in chunks; with `check` the files that are not formatted.
fn rustfmt(root: &Path, rs: &[String], check: bool) -> Result<Outcome> {
    let run = |files: &[String]| -> Result<std::process::Output> {
        let mut c = Command::new("rustfmt");
        if check {
            c.arg("--check");
        }
        c.args(files)
            .current_dir(root)
            .output()
            .context("running rustfmt")
    };
    let mut unformatted = Vec::new();
    for chunk in rs.chunks(RUSTFMT_CHUNK) {
        let output = run(chunk)?;
        if output.status.success() {
            continue;
        }
        if !check {
            return Ok(Outcome {
                ok: false,
                detail: format!("rustfmt failed:\n    {}", tail(&output)),
            });
        }
        // Name the files: one call per file of the failing chunk.
        for file in chunk {
            if !run(std::slice::from_ref(file))?.status.success() {
                unformatted.push(file.clone());
            }
        }
    }
    if unformatted.is_empty() {
        Ok(Outcome::of(true, ""))
    } else {
        Ok(Outcome {
            ok: false,
            detail: unformatted.join(", "),
        })
    }
}

fn python() -> &'static str {
    if cfg!(windows) { "python" } else { "python3" }
}

/// A bless test: `<env>=1 cargo nextest run -p <package> --all-features <target> [-E <filter>]`, or without the
/// variable when `check`.
fn bless_test(
    root: &Path,
    check: bool,
    package: &str,
    target: [&str; 2],
    filter: Option<&str>,
    env: &str,
) -> Result<Outcome> {
    let mut c = Command::new(crate::cargo());
    c.args(["nextest", "run", "-p", package, "--all-features"])
        .args(target)
        .current_dir(root);
    if let Some(f) = filter {
        c.args(["-E", f]);
    }
    if !check {
        c.env(env, "1");
    }
    let what = format!(
        "{}cargo nextest run -p {package} --all-features {}{}",
        if check { String::new() } else { format!("{env}=1 ") },
        target.join(" "),
        filter.map(|f| format!(" -E '{f}'")).unwrap_or_default()
    );
    captured(c, &what)
}

/// Runs the action of `step`.
fn execute(step: &Step, root: &Path, check: bool, files: &BTreeSet<String>, all: bool) -> Result<Outcome> {
    match step.action {
        Action::Rustfmt => rustfmt(root, &rust_files(root, files, all)?, check),
        Action::BlessTest {
            package,
            test,
            filter,
            env,
        } => bless_test(root, check, package, ["--test", test], filter, env),
        Action::BlessBin {
            package,
            bin,
            filter,
            env,
        } => bless_test(root, check, package, ["--bin", bin], filter, env),
        Action::Xtask(run) => Ok(Outcome::of(run(root, check)?, "see its report above")),
        Action::RawRead => Ok(Outcome::of(
            crate::raw_read::run(root, !check)?,
            "see check-raw-read's report above",
        )),
        Action::FuzzTable => Ok(Outcome::of(
            crate::workflows::run_fuzz_smoke_table(root, check)?,
            FUZZ_TABLE,
        )),
        Action::GoldenChanges => {
            let mut c = Command::new(python());
            c.args([
                "docs/plan/tools/golden_changes.py",
                if check { "--check" } else { "--write" },
            ])
            .current_dir(root);
            let what = format!(
                "{} docs/plan/tools/golden_changes.py {}",
                python(),
                if check { "--check" } else { "--write" }
            );
            captured(c, &what)
        }
        Action::Selftest { visual } => {
            let ok = crate::blind::selftest::run(
                root,
                &crate::blind::selftest::Args {
                    build: true,
                    bin: None,
                    out: None,
                    visual,
                    keep: false,
                },
            )?;
            Ok(Outcome::of(ok, "see the self-test's report above"))
        }
        Action::Symbols => Ok(Outcome::of(
            crate::symbols::run(root, true)?,
            "see its report above; run `cargo xtask symbols-qet10`",
        )),
        Action::FuzzBuild => {
            let mut c = Command::new("cargo");
            c.args([
                "check",
                "--manifest-path",
                "fuzz/Cargo.toml",
                "--all-targets",
                "--locked",
            ])
            .current_dir(root);
            captured(
                c,
                "cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked",
            )
        }
        Action::AllowList(rule) => {
            let ok = if check {
                ordered::run(root, rule, false)?
            } else {
                ordered::bless_with_diff(root, rule)?
            };
            Ok(Outcome::of(ok, "see its report above"))
        }
    }
}

/// The parsed arguments.
#[derive(Debug, Default, PartialEq, Eq)]
struct Args {
    check: bool,
    merge: bool,
    all: bool,
    base: Option<String>,
}

const USAGE: &str = "usage: cargo xtask settle [--check] [--merge] [--all] [--base <rev>]";

fn parse(args: &[String]) -> Result<Args> {
    let mut out = Args::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--check" => out.check = true,
            "--merge" => out.merge = true,
            "--all" => out.all = true,
            "--base" => {
                out.base = Some(
                    it.next()
                        .with_context(|| format!("--base needs a revision\n{USAGE}"))?
                        .clone(),
                );
            }
            other => bail!("unknown argument `{other}`\n{USAGE}"),
        }
    }
    Ok(out)
}

/// `cargo xtask settle [--check] [--merge] [--all] [--base <rev>]`.
///
/// # Errors
///
/// Bad arguments, or a git or cargo metadata call that fails.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    let a = parse(args)?;
    let mut files: BTreeSet<String> = if a.all {
        BTreeSet::new()
    } else {
        let base = a.base.as_deref().unwrap_or(DEFAULT_BASE);
        let merge_base = affected::merge_base(root, base)?;
        let files = affected::changed_files(root, &merge_base)?;
        println!(
            "settle: {} changed file(s) since {base} (merge base {})",
            files.len(),
            &merge_base[..merge_base.len().min(12)]
        );
        files.into_iter().collect()
    };
    let mut members: Option<Members> = None;
    let mut lines: Vec<String> = Vec::new();
    let mut ok = true;
    let mut selftests_noted = false;
    for step in STEPS {
        if step.phase == Phase::MergeOnly && !a.merge {
            continue;
        }
        if a.check && matches!(step.action, Action::Selftest { .. }) {
            if !selftests_noted {
                lines.push(SELFTESTS_NOT_CHECKED.to_owned());
                println!("{SELFTESTS_NOT_CHECKED}");
                selftests_noted = true;
            }
            continue;
        }
        if !a.all {
            if !step.deps_of.is_empty() && members.is_none() {
                members = Some(affected::members_of(&crate::metadata_all_features(root)?)?);
            }
            if !triggered(step, &files, members.as_ref()) {
                let line = format!("{}: skipped (no trigger)", step.name);
                println!("{line}");
                lines.push(line);
                continue;
            }
        }
        println!("settle: {} ...", step.name);
        let before = snapshot(root)?;
        let outcome = execute(step, root, a.check, &files, a.all);
        let changed = snapshot_diff(&before, &snapshot(root)?);
        let outside: Vec<&String> = changed
            .iter()
            .filter(|f| !step.outputs.iter().any(|o| glob_match(o, f)))
            .collect();
        let line = match outcome {
            _ if a.check && !changed.is_empty() => {
                format!("{}: FAILED wrote {} in --check", step.name, changed.join(", "))
            }
            _ if !outside.is_empty() => format!(
                "{}: FAILED rewrote files outside its outputs: {}",
                step.name,
                outside.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            ),
            Err(e) => format!("{}: FAILED {e:#}", step.name),
            Ok(o) if o.ok && changed.is_empty() => format!("{}: unchanged", step.name),
            Ok(o) if o.ok => format!("{}: rewrote {}", step.name, changed.join(", ")),
            Ok(o) if a.check && step.phase == Phase::Regen => {
                format!("{}: would rewrite {}", step.name, o.detail)
            }
            Ok(o) => format!("{}: FAILED {}", step.name, o.detail),
        };
        if line.contains(": FAILED ") || line.contains(": would rewrite ") {
            ok = false;
        }
        files.extend(changed);
        println!("{line}");
        lines.push(line);
    }
    println!("\nsettle{}:", if a.check { " --check" } else { "" });
    for line in &lines {
        println!("  {line}");
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affected::Member;

    fn set(files: &[&str]) -> BTreeSet<String> {
        files.iter().map(|s| (*s).to_owned()).collect()
    }

    fn step(name: &str) -> &'static Step {
        STEPS.iter().find(|s| s.name == name).unwrap()
    }

    /// The table's order is the brief's: regeneration steps first, in their order, then the checks, then the review
    /// gates.
    #[test]
    fn the_steps_run_in_their_order() {
        let names: Vec<&str> = STEPS.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            [
                "rustfmt",
                "command-schemas",
                "cli-schemas",
                "rule-catalogue",
                "keymaps",
                "blind-reference",
                "cli-snapshot",
                "api-v1-snapshot",
                "query-snapshot",
                "docs-gen",
                "gen-icons",
                "clippy-list",
                "fuzz-table",
                "golden-change-list",
                "selftest",
                "selftest-visual",
                "symbols-qet10",
                "fuzz-build",
                "disallowed-methods",
                "ordered-collections",
            ]
        );
        let phases: Vec<Phase> = STEPS.iter().map(|s| s.phase).collect();
        assert!(phases.windows(2).all(|w| {
            let rank = |p: Phase| match p {
                Phase::Regen => 0,
                Phase::CheckOnly => 1,
                Phase::MergeOnly => 2,
            };
            rank(w[0]) <= rank(w[1])
        }));
        // A step that writes has outputs; a check has none.
        for s in STEPS {
            assert_eq!(s.phase == Phase::CheckOnly, s.outputs.is_empty(), "{}", s.name);
            assert!(s.partial.iter().all(|p| s.outputs.contains(p)), "{}", s.name);
        }
    }

    #[test]
    fn globs_match_within_and_across_segments() {
        assert!(glob_match("**/*.rs", "main.rs"));
        assert!(glob_match("**/*.rs", "crates/a/src/lib.rs"));
        assert!(!glob_match("**/*.rs", "crates/a/src/lib.rsx"));
        assert!(glob_match("crates/*/Cargo.toml", "crates/ew-a/Cargo.toml"));
        assert!(!glob_match("crates/*/Cargo.toml", "crates/ew-a/sub/Cargo.toml"));
        assert!(glob_match("docs/plan/03-*.md", "docs/plan/03-milestones.md"));
        assert!(glob_match(
            "tools/blind/reference/**",
            "tools/blind/reference/actions.txt"
        ));
        assert!(!glob_match(
            "tools/blind/reference/**",
            "tools/blind/references/a.txt"
        ));
        assert!(glob_match("clippy.toml", "clippy.toml"));
        assert!(!glob_match("clippy.toml", "crates/a/clippy.toml"));
    }

    /// Each step is selected by a path its triggers name, and not by an unrelated one.
    #[test]
    fn each_steps_triggers_select_it() {
        let cases: &[(&str, &str)] = &[
            ("rustfmt", "crates/ew-model/src/lib.rs"),
            ("command-schemas", "crates/ew-commands/schemas/index.json"),
            ("cli-schemas", "bin/ew/schemas/ew.apply-result-1.schema.json"),
            ("rule-catalogue", "crates/ew-checks/catalogue/rules.json"),
            ("keymaps", "crates/ew-viewmodel/keymaps/sheet.json"),
            ("blind-reference", "tools/blind/reference/actions.txt"),
            ("docs-gen", "crates/ew-checks/catalogue/rules.json"),
            ("docs-gen", "docs/plan/work-packages.csv"),
            ("cli-snapshot", "bin/ew/api/cli.json"),
            ("api-v1-snapshot", "crates/ew-sqlview/api/api_v1.json"),
            ("query-snapshot", "crates/ew-api/api/queries.json"),
            ("docs-gen", "bin/ew/api/cli.json"),
            ("docs-gen", "bin/ew/schemas/index.json"),
            ("docs-gen", "crates/ew-api/api/queries.json"),
            ("docs-gen", "crates/ew-sqlview/api/api_v1.json"),
            ("docs-gen", "schemas/0.1/part.schema.json"),
            ("gen-icons", "assets/icons/tabler/x.svg"),
            ("clippy-list", "crates/ew-model/Cargo.toml"),
            ("fuzz-table", "crates/ew-format/Cargo.toml"),
            ("fuzz-table", "fuzz/Cargo.toml"),
            ("golden-change-list", "golden/J-E-2416/tables/parts.csv"),
            ("golden-change-list", "docs/plan/06-decision-log.md"),
            ("selftest", "tools/blind/tasks/selftest/agent.jsonl"),
            ("selftest", "tools/blind/reference/actions.txt"),
            ("selftest-visual", "tools/blind/HELP-VISUAL.md"),
            ("symbols-qet10", "content/symbols/k1.json"),
            ("fuzz-build", "crates/ew-testkit/tests/command_fuzz/live.rs"),
            ("disallowed-methods", "crates/ew-x/src/lib.rs"),
            ("ordered-collections", "ci/ordered-collections-allow.toml"),
        ];
        let empty = Members::new();
        for (name, path) in cases {
            assert!(
                triggered(step(name), &set(&[path]), Some(&empty)),
                "{name} by {path}"
            );
        }
        for s in STEPS {
            assert!(
                !triggered(s, &set(&["README.md", "docs/design/01-x.md"]), Some(&empty)),
                "{} by an unrelated file",
                s.name
            );
        }
        // A self-test is not re-blessed by a product change alone; the visual one not by the text task.
        assert!(!triggered(
            step("selftest"),
            &set(&["crates/ew-ui/src/lib.rs"]),
            Some(&empty)
        ));
        assert!(!triggered(
            step("selftest-visual"),
            &set(&["tools/blind/tasks/selftest/agent.jsonl"]),
            Some(&empty)
        ));
    }

    /// A `deps_of` step is triggered by its package, by a package it is built from and by the build-wide files, not
    /// by a package that depends on it or by a dev-dependency.
    #[test]
    fn a_deps_of_step_follows_the_build_closure() {
        let member = |dir: &str, deps: &[&str], dev: &[&str]| Member {
            dir: dir.to_owned(),
            deps: deps.iter().map(|s| (*s).to_owned()).collect(),
            dev_deps: dev.iter().map(|s| (*s).to_owned()).collect(),
            ..Member::default()
        };
        let members: Members = [
            ("ew-units", member("crates/ew-units", &[], &[])),
            (
                "ew-model",
                member("crates/ew-model", &["ew-units"], &["ew-testkit"]),
            ),
            ("ew-commands", member("crates/ew-commands", &["ew-model"], &[])),
            ("ew-testkit", member("crates/ew-testkit", &["ew-commands"], &[])),
            ("ew-api", member("crates/ew-api", &["ew-commands"], &[])),
        ]
        .into_iter()
        .map(|(n, m)| (n.to_owned(), m))
        .collect();
        let schemas = step("command-schemas");
        for f in [
            "crates/ew-commands/src/lib.rs",
            "crates/ew-units/src/lib.rs",
            "Cargo.lock",
        ] {
            assert!(triggered(schemas, &set(&[f]), Some(&members)), "{f}");
        }
        for f in [
            "crates/ew-api/src/lib.rs",
            "crates/ew-testkit/src/lib.rs",
            "golden/x",
        ] {
            assert!(!triggered(schemas, &set(&[f]), Some(&members)), "{f}");
        }
    }

    /// Settle never writes a golden output, a UI snapshot, page output, an acceptance or fixture file or a
    /// describe-text expectation: no output glob of a step matches one, and none names such a folder.
    #[test]
    fn no_step_writes_golden_outputs() {
        let protected = [
            "golden/J-E-2416.ew/project.json",
            "golden/J-E-2416/expected/pages/101.txt",
            "golden/J-E-2416/tables/parts.csv",
            "golden/J-E-2416/README.md",
            "crates/ew-ui/tests/snapshots/canvas.png",
            "crates/ew-ui/tests/snapshots/canvas.diff.png",
            "crates/ew-api/tests/it/snapshots/x.snap",
            "crates/ew-viewmodel/tests/expected/navigators/project.txt",
            "crates/ew-viewmodel/tests/expected/page_describe/101.txt",
            "bin/elektrawatt/tests/expected/pdf/sha256.txt",
            "crates/ew-format/tests/fixtures/a.jsonl",
            "crates/ew-api/tests/acceptance/report.md",
            "tools/blind/recordings/gpp-3/run.json",
        ];
        for s in STEPS {
            for o in s.outputs {
                for p in protected {
                    assert!(!glob_match(o, p), "{}: {o} matches {p}", s.name);
                }
                if *o != "**/*.rs" {
                    for word in [
                        "golden/",
                        "snapshots",
                        "fixtures",
                        "expected",
                        "acceptance",
                        "page-output",
                    ] {
                        assert!(!o.contains(word), "{}: {o}", s.name);
                    }
                }
            }
        }
    }

    /// `.gitattributes` gives exactly the whole-file outputs of the table `merge=regen`, in table order (a pattern
    /// without a slash is written with a leading `/`, so that it matches at the root only).
    #[test]
    fn gitattributes_lists_the_regeneration_outputs() {
        let text = std::fs::read_to_string(crate::workspace_root().join(".gitattributes")).unwrap();
        let listed: Vec<String> = text
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .filter(|l| l.split_whitespace().skip(1).any(|a| a == "merge=regen"))
            .map(|l| {
                let pattern = l.split_whitespace().next().unwrap();
                pattern.strip_prefix('/').unwrap_or(pattern).to_owned()
            })
            .collect();
        assert_eq!(listed, whole_file_outputs());
        for l in text.lines().filter(|l| l.contains("merge=regen")) {
            let pattern = l.split_whitespace().next().unwrap();
            assert!(
                pattern.trim_start_matches('/').contains('/') || pattern.starts_with('/'),
                "{pattern}: a pattern without a slash matches at any level"
            );
        }
        // The partial outputs stay out: hand-written lines must merge as text.
        for o in [BOOK_SUMMARY, FUZZ_TABLE, GOLDEN_LIST] {
            assert!(!whole_file_outputs().contains(&o), "{o}");
        }
    }

    /// A module its parent declares under `#[rustfmt::skip]` is not given to rustfmt: `cargo fmt` leaves it alone,
    /// and rustfmt given the file itself would not see the attribute.
    #[test]
    fn modules_under_rustfmt_skip_are_found() {
        let text =
            "//! x\n#[rustfmt::skip]\nmod data;\n\n#[rustfmt::skip]\n\npub(crate) mod table;\nmod other;\n";
        assert_eq!(
            skipped_modules("crates/ew-rds/src/sets/mod.rs", text),
            ["crates/ew-rds/src/sets/data", "crates/ew-rds/src/sets/table"]
        );
        assert_eq!(
            skipped_modules("crates/ew-ui/src/icons.rs", "#[rustfmt::skip]\nmod generated;\n"),
            ["crates/ew-ui/src/icons/generated"]
        );
        assert_eq!(
            skipped_modules("src/lib.rs", "#[rustfmt::skip]\nfn f() {}\nmod a;\n"),
            Vec::<String>::new()
        );
        // The two in the workspace today.
        let root = crate::workspace_root();
        let rs = rust_files(
            &root,
            &set(&[
                "crates/ew-rds/src/sets/data.rs",
                "crates/ew-ui/src/icons/generated.rs",
                "crates/ew-ui/src/icons/mod.rs",
            ]),
            false,
        )
        .unwrap();
        assert_eq!(rs, ["crates/ew-ui/src/icons/mod.rs"]);
    }

    #[test]
    fn the_arguments_parse() {
        let s = |v: &[&str]| v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            parse(&s(&["--check", "--merge", "--base", "abc"])).unwrap(),
            Args {
                check: true,
                merge: true,
                all: false,
                base: Some("abc".to_owned())
            }
        );
        assert!(parse(&s(&["--all"])).unwrap().all);
        assert_eq!(
            parse(&s(&["--base"])).unwrap_err().to_string(),
            format!("--base needs a revision\n{USAGE}")
        );
        assert_eq!(
            parse(&s(&["--frob"])).unwrap_err().to_string(),
            format!("unknown argument `--frob`\n{USAGE}")
        );
    }

    #[test]
    fn a_snapshot_diff_names_written_added_and_reverted_files() {
        let d = |b: u8| Some([b; 32]);
        let before = BTreeMap::from([
            ("a".to_owned(), d(1)),
            ("b".to_owned(), d(2)),
            ("c".to_owned(), d(3)),
        ]);
        let after = BTreeMap::from([
            ("a".to_owned(), d(1)),
            ("b".to_owned(), d(9)),
            ("e".to_owned(), None),
        ]);
        assert_eq!(snapshot_diff(&before, &after), ["b", "c", "e"]);
    }

    #[test]
    fn an_atomic_write_replaces_the_file_and_leaves_no_temporary() {
        let dir = std::env::temp_dir().join(format!("ew-settle-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.toml");
        std::fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        assert!(!dir.join("t.toml.settle-tmp").exists());
        // A write into a missing folder fails and leaves nothing.
        assert!(write_atomic(&dir.join("missing/t.toml"), "x").is_err());
        assert!(!dir.join("missing").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
