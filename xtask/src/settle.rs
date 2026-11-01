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
//!   step is triggered by a changed file that matches one of its globs, or that lies in one of its `deps_of`
//!   packages or in a workspace package one of them depends on directly (a normal or build edge; a change of the
//!   root `Cargo.toml`, `Cargo.lock` or `rust-toolchain.toml` counts too). `--all` runs every step.
//! - **The safety net of the direct triggers.** A change two or more edges below a `deps_of` package does not
//!   trigger its step here. The merge point's affected verify runs every bless test in compare mode for every
//!   package the change reaches, so a derived file such a change alters fails there and is settled then.
//! - **One nested nextest call.** The triggered bless steps (`BlessTest`, `BlessBin`) run together as one
//!   `cargo nextest run --profile lane --all-features -p … -E '<union of their binaries and filters>'`, with every
//!   triggered step's bless variable set (none with `--check`). A failure is named by its test binary's id and
//!   given to the step(s) of that binary; a failure without one (a build error) is given to every step of the call.
//!   A `cargo nextest list` of the same call comes first: a step whose term matches no test fails. A step whose
//!   `reads` name another triggered step's outputs (cli-schemas reads the command schemas) waits for a later call,
//!   and runs again when they change after it ran; a call whose rewrites trigger a further bless step is followed
//!   by another call.
//! - **Outputs.** Before and after each step (each bless call) the changed files of the working tree (against
//!   `HEAD`) are hashed; a step that changes a file outside its listed outputs fails. A bless call's changed files
//!   must match the union of its steps' outputs, and each is reported under the step whose outputs it matches; a
//!   file outside them fails every step of the call. Settle writes only derived reference files that are a pure
//!   function of the code: never golden outputs, UI snapshots, page output, acceptance or fixture files or the
//!   describe-text expectations, which are decisions a change makes and lists.
//! - **`--check`** writes nothing (a step that writes in this mode fails) and exits 1 when a step would rewrite a
//!   file or fails. It is the lane's mode: it runs no self-test (the verify replays them), and without `--merge` no
//!   `check-brief` and no MSRV check (the merge point runs them); its summary says so.
//! - **`--merge`** adds the review gates `check-disallowed-methods --bless` and `check-ordered-collections --bless`,
//!   each printing its allow list's change per crate and the sites added or removed, and the two merge-point checks:
//!   `check-brief` for each open design brief (`tools/blind/tasks/brief-*`; every one on a self-test trigger, else
//!   the briefs whose files changed), on a fresh `cargo build -p elektrawatt -p ew`; and the MSRV check of the
//!   permissive crates (on `Cargo.lock`, a changed `rust-version`, or a changed `.rs` file or manifest of a
//!   permissive crate); and, on any change, `records --write` (the records follow the commit trailers, so they are
//!   regenerated where the history is final; `records.md` is written whole). The MSRV check runs as CI's job does:
//!   in a copy of the workspace under
//!   `target/settle-msrv`, removed afterwards, the MSRV-aware resolver writes a lockfile and `cargo +<msrv> check
//!   --locked` builds the members whose licence is `MIT OR Apache-2.0` (xtask aside); both cargo calls take the
//!   copy's `--manifest-path`, so the committed `Cargo.lock` is never touched. None of these run without `--merge`.
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
/// The line `settle --check` prints, without `--merge`, for the merge-point checks.
const MERGE_CHECKS_NOT_CHECKED: &str = "check-brief, msrv: not checked here (settle --merge runs them)";
/// The folder of the open design briefs: `<BRIEF_DIR>/brief-<name>/`.
const BRIEF_DIR: &str = "tools/blind/tasks";
/// The licence of the permissive crates, whose MSRV the merge point checks.
const PERMISSIVE: &str = "MIT OR Apache-2.0";
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
    /// `blind check-brief` for each open design brief the change reaches, after `cargo build -p elektrawatt -p ew`.
    CheckBrief,
    /// The MSRV check of the permissive crates (see the module documentation).
    Msrv,
    /// `records --write` (`--check`), in-process: `docs/plan/records.md` and the status block of doc 01 §3.8.
    Records,
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
    /// Output globs of other steps that its action reads at run time, written as those steps write them (a bless
    /// step reading another bless step's outputs runs in a later nested call, and again when they change after it).
    pub reads: &'static [&'static str],
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
const RECORDS: &str = crate::records::RECORDS;
const GLOBAL_PLAN: &str = crate::records::GLOBAL_PLAN;

// map: settle-steps: a new generated file or upkeep step goes in this table, in order, with triggers and outputs.
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
        reads: &[],
        action: Action::Rustfmt,
    },
    Step {
        name: "command-schemas",
        phase: Phase::Regen,
        triggers: &[COMMAND_SCHEMAS],
        deps_of: &["ew-commands"],
        outputs: &[COMMAND_SCHEMAS],
        partial: &[],
        reads: &[],
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
        // `registry_subcommands_print_the_committed_files` reads the command schemas that command-schemas rewrites.
        reads: &[COMMAND_SCHEMAS],
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
        reads: &[],
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
        reads: &[],
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
        reads: &[],
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
        reads: &[],
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
        reads: &[],
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
        reads: &[],
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
        reads: &[],
        action: Action::Xtask(crate::docs_gen::run),
    },
    Step {
        name: "gen-icons",
        phase: Phase::Regen,
        triggers: &["assets/icons/**", "xtask/src/icons.rs", ICONS],
        deps_of: &[],
        outputs: &[ICONS],
        partial: &[],
        reads: &[],
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
        reads: &[],
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
        reads: &[],
        action: Action::FuzzTable,
    },
    Step {
        name: "golden-change-list",
        phase: Phase::Regen,
        triggers: &[
            "golden/J-E-2416/tables/**",
            "docs/design/20-golden-project.md",
            "docs/plan/06-decision-log.md",
            crate::records::DECISIONS,
            "docs/plan/tools/golden_changes.py",
            GOLDEN_LIST,
        ],
        deps_of: &[],
        outputs: &[GOLDEN_LIST],
        partial: &[GOLDEN_LIST],
        reads: &[],
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
        reads: &[],
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
        reads: &[],
        action: Action::Selftest { visual: true },
    },
    Step {
        name: "symbols-qet10",
        phase: Phase::CheckOnly,
        triggers: &["content/symbols/**", "content/fndefs/**", "xtask/src/symbols.rs"],
        deps_of: &[],
        outputs: &[],
        partial: &[],
        reads: &[],
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
        reads: &[],
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
        reads: &[],
        action: Action::AllowList(&ordered::METHODS),
    },
    Step {
        name: "ordered-collections",
        phase: Phase::MergeOnly,
        triggers: &["**/*.rs", "**/Cargo.toml", "ci/ordered-collections-allow.toml"],
        deps_of: &[],
        outputs: &["ci/ordered-collections-allow.toml"],
        partial: &[],
        reads: &[],
        action: Action::AllowList(&ordered::TYPES),
    },
    // The checks that left the lanes (cycle 43): the merge point runs them.
    Step {
        name: "check-brief",
        phase: Phase::MergeOnly,
        // The two self-test steps' triggers, and every brief's files (`briefs_to_check` picks the briefs).
        triggers: &[
            "tools/blind/HELP.md",
            "tools/blind/tasks/selftest/**",
            BLIND_REFERENCE,
            SELFTEST_RECORDING,
            "xtask/src/blind/**",
            "tools/blind/HELP-VISUAL.md",
            "tools/blind/tasks/selftest-visual/**",
            SELFTEST_VISUAL_RECORDING,
            "tools/blind/tasks/brief-*/**",
        ],
        deps_of: &[],
        outputs: &[],
        partial: &[],
        reads: &[],
        action: Action::CheckBrief,
    },
    Step {
        name: "msrv",
        phase: Phase::MergeOnly,
        // And a changed `rust-version` line, or a permissive crate's source or manifest (`msrv_triggered`).
        triggers: &["Cargo.lock"],
        deps_of: &[],
        outputs: &[],
        partial: &[],
        reads: &[],
        action: Action::Msrv,
    },
    // RD43 R11 (ID-1051): the records come from the history (the commit trailers), so they are regenerated at the
    // merge point, on any change. records.md is written whole (merge=regen); doc 01 only in its status block.
    Step {
        name: "records",
        phase: Phase::MergeOnly,
        triggers: &["**"],
        deps_of: &[],
        outputs: &[RECORDS, GLOBAL_PLAN],
        partial: &[GLOBAL_PLAN],
        reads: &[],
        action: Action::Records,
    },
];

/// The outputs that the regeneration steps (and the merge-only `records` step) rewrite entirely from their sources,
/// in table order: the files that `.gitattributes` gives `merge=regen`.
#[cfg(test)]
pub(crate) fn whole_file_outputs() -> Vec<&'static str> {
    STEPS
        .iter()
        .filter(|s| s.phase == Phase::Regen || matches!(s.action, Action::Records))
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

/// The packages whose change triggers a step with `deps_of` `names`: themselves and the workspace packages they
/// depend on directly (normal and build edges). Not transitive: the module documentation names the safety net.
fn direct_inputs(names: &[&str], members: &Members) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for name in names {
        out.insert((*name).to_owned());
        if let Some(m) = members.get(*name) {
            out.extend(m.deps.iter().cloned());
        }
    }
    out
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
    let inputs = direct_inputs(step.deps_of, members);
    files
        .iter()
        .any(|f| BUILD_WIDE.contains(&f.as_str()) || owner(f, members).is_some_and(|o| inputs.contains(o)))
}

/// Whether a manifest's `rust-version` lines differ between its `old` and `new` text (an absent file is empty).
fn changes_rust_version(old: &str, new: &str) -> bool {
    let lines = |text: &str| -> Vec<String> {
        text.lines()
            .map(str::trim)
            .filter(|l| l.starts_with("rust-version"))
            .map(|l| l.split_whitespace().collect::<String>())
            .collect()
    };
    lines(old) != lines(new)
}

/// Whether one of the changed manifests among `files` changed a `rust-version` line since `merge_base`.
fn rust_version_changed(root: &Path, merge_base: &str, files: &BTreeSet<String>) -> Result<bool> {
    for f in files
        .iter()
        .filter(|f| *f == "Cargo.toml" || f.ends_with("/Cargo.toml"))
    {
        let old = Command::new("git")
            .args(["show", &format!("{merge_base}:{f}")])
            .current_dir(root)
            .output()
            .context("running git show")?;
        let old = if old.status.success() {
            String::from_utf8_lossy(&old.stdout).into_owned()
        } else {
            String::new()
        };
        let new = std::fs::read_to_string(root.join(f)).unwrap_or_default();
        if changes_rust_version(&old, &new) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The MSRV check's triggers beyond `Cargo.lock`: a changed source or manifest of a permissive crate, or a changed
/// `rust-version` line since `merge_base`.
fn msrv_triggered(root: &Path, merge_base: Option<&str>, files: &BTreeSet<String>) -> Result<bool> {
    let metadata = crate::metadata_all_features(root)?;
    let members = affected::members_of(&metadata)?;
    if permissive_source_changed(files, &members, &msrv_plan(&metadata)?.packages) {
        return Ok(true);
    }
    match merge_base {
        Some(mb) => rust_version_changed(root, mb, files),
        None => Ok(false),
    }
}

/// Whether one of `files` is a `.rs` file or a manifest of a permissive crate (`permissive`, by package name).
fn permissive_source_changed(files: &BTreeSet<String>, members: &Members, permissive: &[String]) -> bool {
    files.iter().any(|f| {
        (f.ends_with(".rs") || f.ends_with("/Cargo.toml"))
            && owner(f, members).is_some_and(|o| permissive.iter().any(|p| p == o))
    })
}

/// The open design briefs (`<BRIEF_DIR>/brief-*`), by folder name.
fn brief_tasks(root: &Path) -> Result<Vec<String>> {
    let dir = root.join(BRIEF_DIR);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("brief-") && entry.path().is_dir() {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

/// The briefs of `tasks` that `check-brief` proves: every one when a self-test step's trigger changed (or with
/// `all`), else those with a changed file in their folder.
fn briefs_to_check(tasks: &[String], files: &BTreeSet<String>, all: bool) -> Vec<String> {
    let selftest_hit = all
        || files.iter().any(|f| {
            STEPS
                .iter()
                .filter(|s| matches!(s.action, Action::Selftest { .. }))
                .any(|s| s.triggers.iter().any(|t| glob_match(t, f)))
        });
    tasks
        .iter()
        .filter(|t| selftest_hit || files.iter().any(|f| f.starts_with(&format!("{BRIEF_DIR}/{t}/"))))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// The MSRV check

/// What the MSRV check builds: the permissive members, and the toolchain of their `rust-version`.
#[derive(Debug, PartialEq, Eq)]
struct MsrvPlan {
    packages: Vec<String>,
    toolchain: String,
}

/// The MSRV plan from the workspace's metadata: the members whose `license` is [`PERMISSIVE`], xtask aside (a
/// development tool, built with the pinned toolchain), and the highest `rust-version` among them as `x.y.z`.
///
/// # Errors
///
/// No permissive member, or one without a `rust-version`.
fn msrv_plan(metadata: &cargo_metadata::Metadata) -> Result<MsrvPlan> {
    let members: BTreeSet<&cargo_metadata::PackageId> = metadata.workspace_members.iter().collect();
    let mut packages = Vec::new();
    let mut msrv: Option<cargo_metadata::semver::Version> = None;
    for p in metadata.packages.iter().filter(|p| members.contains(&p.id)) {
        if p.license.as_deref() != Some(PERMISSIVE) || p.name.as_str() == "xtask" {
            continue;
        }
        let v = p
            .rust_version
            .clone()
            .with_context(|| format!("the permissive crate {} has no rust-version", p.name))?;
        if msrv.as_ref().is_none_or(|m| v > *m) {
            msrv = Some(v);
        }
        packages.push(p.name.to_string());
    }
    packages.sort();
    let msrv = msrv.context("the workspace has no permissive crate")?;
    Ok(MsrvPlan {
        packages,
        toolchain: format!("{}.{}.{}", msrv.major, msrv.minor, msrv.patch),
    })
}

/// The check's command line as printed.
fn msrv_command(plan: &MsrvPlan) -> String {
    format!(
        "cargo +{} check --locked -p {}",
        plan.toolchain,
        plan.packages.join(" -p ")
    )
}

/// The MSRV check in a copy of the workspace's manifests and sources under `target/settle-msrv/src`, removed
/// afterwards whatever the result; the build directory `target/settle-msrv/target` stays warm.
fn msrv(root: &Path) -> Result<Outcome> {
    let plan = msrv_plan(&crate::metadata_all_features(root)?)?;
    let base = root.join("target/settle-msrv");
    let copy = base.join("src");
    if copy.exists() {
        std::fs::remove_dir_all(&copy).with_context(|| format!("removing {}", copy.display()))?;
    }
    let result = msrv_in(root, &copy, &base.join("target"), &plan);
    let removed = if copy.exists() {
        std::fs::remove_dir_all(&copy).with_context(|| format!("removing {}", copy.display()))
    } else {
        Ok(())
    };
    let outcome = result?;
    removed?;
    Ok(outcome)
}

fn msrv_in(root: &Path, copy: &Path, target: &Path, plan: &MsrvPlan) -> Result<Outcome> {
    let listed = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "crates",
            "bin",
            "xtask",
        ])
        .current_dir(root)
        .output()
        .context("running git ls-files")?;
    if !listed.status.success() {
        bail!("git ls-files failed: {}", tail(&listed));
    }
    for rel in listed
        .stdout
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(String::from_utf8_lossy)
    {
        let from = root.join(rel.as_ref());
        if !from.is_file() {
            continue; // removed in the working tree
        }
        let to = copy.join(rel.as_ref());
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::copy(&from, &to).with_context(|| format!("copying {rel}"))?;
    }
    // The rustup proxy, with the toolchain named on the command line: none of the variables `cargo xtask` inherited
    // from the pinned toolchain may reach it.
    // Cargo is pointed at the copy's manifest: it never walks up to the workspace and its committed lockfile.
    let manifest = copy.join("Cargo.toml");
    if !manifest.is_file() {
        bail!("the copy of the workspace has no {}", manifest.display());
    }
    let cargo = |args: &[&str]| {
        let mut c = Command::new("cargo");
        c.arg(format!("+{}", plan.toolchain))
            .args(args)
            .arg("--manifest-path")
            .arg(&manifest)
            .current_dir(copy)
            .env("CARGO_TARGET_DIR", target)
            .env("CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS", "fallback");
        for v in ["RUSTUP_TOOLCHAIN", "CARGO", "RUSTC", "RUSTDOC"] {
            c.env_remove(v);
        }
        c
    };
    let lock = captured(
        cargo(&["generate-lockfile"]),
        &format!("cargo +{} generate-lockfile", plan.toolchain),
    )?;
    if !lock.ok {
        return Ok(lock);
    }
    let mut args = vec!["check", "--locked"];
    for p in &plan.packages {
        args.extend(["-p", p.as_str()]);
    }
    captured(cargo(&args), &msrv_command(plan))
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

/// Whether `step` is a bless step: one of the nested nextest call.
fn is_bless(step: &Step) -> bool {
    matches!(step.action, Action::BlessTest { .. } | Action::BlessBin { .. })
}

/// A bless step's package, nextest binary id, filter and bless variable.
fn bless_parts(step: &Step) -> Option<(&'static str, String, Option<&'static str>, &'static str)> {
    match step.action {
        Action::BlessTest {
            package,
            test,
            filter,
            env,
        } => Some((package, format!("{package}::{test}"), filter, env)),
        Action::BlessBin {
            package,
            bin,
            filter,
            env,
        } => Some((package, format!("{package}::bin/{bin}"), filter, env)),
        _ => None,
    }
}

/// A bless step's filterset term: its binary, and its filter when it has one.
fn term(step: &Step) -> String {
    match bless_parts(step) {
        Some((_, id, Some(f), _)) => format!("(binary_id({id}) & {f})"),
        Some((_, id, None, _)) => format!("binary_id({id})"),
        None => String::new(),
    }
}

/// The binary ids with at least one test that matches the filter and is not ignored, from the JSON of
/// `cargo nextest list --message-format json`.
fn matched_binaries(json: &str) -> Result<BTreeSet<String>> {
    let v: serde_json::Value = serde_json::from_str(json).context("reading nextest's test list")?;
    let suites = v
        .get("rust-suites")
        .and_then(serde_json::Value::as_object)
        .context("nextest's test list has no `rust-suites`")?;
    Ok(suites
        .iter()
        .filter(|(_, suite)| {
            suite
                .get("testcases")
                .and_then(serde_json::Value::as_object)
                .is_some_and(|cases| {
                    cases
                        .values()
                        .any(|c| c["filter-match"]["status"] == "matches" && c["ignored"] != true)
                })
        })
        .map(|(id, _)| id.clone())
        .collect())
}

/// The nested nextest call of a group of bless steps: its arguments after `cargo`, the bless variables it sets
/// (none when `check`) and the command line as printed.
fn group_command(group: &[&Step], check: bool) -> (Vec<String>, Vec<&'static str>, String) {
    let mut packages = BTreeSet::new();
    let mut envs = BTreeSet::new();
    let mut terms = Vec::new();
    for s in group {
        let Some((package, _, _, env)) = bless_parts(s) else {
            continue;
        };
        packages.insert(package);
        envs.insert(env);
        terms.push(term(s));
    }
    let filterset = terms.join(" | ");
    let mut args: Vec<String> = ["nextest", "run", "--profile", "lane", "--all-features"]
        .map(str::to_owned)
        .to_vec();
    for p in &packages {
        args.extend(["-p".to_owned(), (*p).to_owned()]);
    }
    let envs: Vec<&'static str> = if check {
        Vec::new()
    } else {
        envs.into_iter().collect()
    };
    let shown = format!(
        "{}cargo {} -E '{filterset}'",
        envs.iter().map(|e| format!("{e}=1 ")).collect::<String>(),
        args.join(" ")
    );
    args.extend(["-E".to_owned(), filterset]);
    (args, envs, shown)
}

/// The nextest statuses of a failed test (the first word of its line).
const FAILED_STATUSES: [&str; 6] = ["FAIL", "SIGSEGV", "SIGABRT", "SIGKILL", "TIMEOUT", "ABORT"];

/// The steps of `group` that a failed call's output names: a line that starts with a failure status and holds the
/// step's binary id as a word. Each with its lines.
fn failed_steps<'s>(group: &[&'s Step], output: &str) -> BTreeMap<&'s str, Vec<String>> {
    let mut out: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for line in output.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if !words.first().is_some_and(|w| FAILED_STATUSES.contains(w)) {
            continue;
        }
        for s in group {
            let Some((_, id, _, _)) = bless_parts(s) else {
                continue;
            };
            if words.contains(&id.as_str()) {
                let lines = out.entry(s.name).or_default();
                if !lines.iter().any(|l| l == line.trim()) {
                    lines.push(line.trim().to_owned());
                }
            }
        }
    }
    out
}

/// `cargo nextest list` with the arguments of a `run` call and `filterset`: the binaries with a matching test, or
/// the failed command's output.
fn list_matches(root: &Path, args: &[String], filterset: &str) -> Result<Result<BTreeSet<String>, String>> {
    let mut list: Vec<String> = args.to_vec();
    list[1] = "list".to_owned();
    let e = list
        .iter()
        .position(|a| a == "-E")
        .context("a run call has a filterset")?;
    list.truncate(e);
    list.extend(["-E".to_owned(), filterset.to_owned()]);
    list.extend(["--message-format".to_owned(), "json".to_owned()]);
    let output = Command::new(crate::cargo())
        .args(&list)
        .current_dir(root)
        .output()
        .context("running cargo nextest list")?;
    if !output.status.success() {
        return Ok(Err(tail(&output)));
    }
    matched_binaries(&String::from_utf8_lossy(&output.stdout)).map(Ok)
}

/// The steps of `group` whose term would run no test: from the binaries the group's list matched, and for a binary
/// that two steps of the group share, from a list of the step's own term.
fn steps_without_tests<'s>(
    root: &Path,
    group: &[&'s Step],
    args: &[String],
    matched: &BTreeSet<String>,
) -> Result<Result<BTreeSet<&'s str>, String>> {
    let id = |s: &Step| bless_parts(s).map(|(_, id, _, _)| id).unwrap_or_default();
    let mut out = BTreeSet::new();
    for s in group {
        let shared = group.iter().filter(|t| id(t) == id(s)).count() > 1;
        let found = if shared {
            match list_matches(root, args, &term(s))? {
                Ok(m) => m.contains(&id(s)),
                Err(e) => return Ok(Err(e)),
            }
        } else {
            matched.contains(&id(s))
        };
        if !found {
            out.insert(s.name);
        }
    }
    Ok(Ok(out))
}

/// Runs a group of bless steps as one nested call; the outcome of each step, in the group's order. A list of the
/// group's tests comes first: a step whose term matches no test fails (it would otherwise pass unseen).
fn bless_group(root: &Path, group: &[&Step], check: bool) -> Result<Vec<Result<Outcome>>> {
    let (args, envs, shown) = group_command(group, check);
    let filterset = args.last().cloned().unwrap_or_default();
    let failed_build = |detail: String| -> Vec<Result<Outcome>> {
        group
            .iter()
            .map(|_| {
                Ok(Outcome {
                    ok: false,
                    detail: format!("{shown} failed:\n    {detail}"),
                })
            })
            .collect()
    };
    let matched = match list_matches(root, &args, &filterset)? {
        Ok(m) => m,
        Err(e) => return Ok(failed_build(e)),
    };
    let empty = match steps_without_tests(root, group, &args, &matched)? {
        Ok(e) => e,
        Err(e) => return Ok(failed_build(e)),
    };
    let mut c = Command::new(crate::cargo());
    c.args(&args).current_dir(root);
    for e in &envs {
        c.env(e, "1");
    }
    let output = c.output().with_context(|| format!("running {shown}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let named = failed_steps(group, &text);
    Ok(group
        .iter()
        .map(|s| {
            if empty.contains(s.name) {
                bail!("its filter `{}` matches no test", term(s));
            }
            Ok(match named.get(s.name) {
                _ if output.status.success() => Outcome::of(true, ""),
                Some(lines) => Outcome {
                    ok: false,
                    detail: format!("{shown} failed:\n    {}", lines.join("\n    ")),
                },
                None if named.is_empty() => Outcome {
                    ok: false,
                    detail: format!("{shown} failed:\n    {}", tail(&output)),
                },
                None => Outcome::of(true, ""),
            })
        })
        .collect())
}

/// A group's changed files by step (a file under each step whose outputs match it) and the files no step's
/// outputs match.
fn attribute<'s>(group: &[&'s Step], changed: &[String]) -> (BTreeMap<&'s str, Vec<String>>, Vec<String>) {
    let mut mine: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut outside = Vec::new();
    for f in changed {
        let owners: Vec<&str> = group
            .iter()
            .filter(|s| s.outputs.iter().any(|o| glob_match(o, f)))
            .map(|s| s.name)
            .collect();
        if owners.is_empty() {
            outside.push(f.clone());
        }
        for o in owners {
            mine.entry(o).or_default().push(f.clone());
        }
    }
    (mine, outside)
}

/// A step's summary line from its outcome, the files it changed (`mine`) and the files outside its outputs.
fn step_line(
    step: &Step,
    check: bool,
    outcome: Result<Outcome>,
    mine: &[String],
    outside: &[String],
) -> String {
    let changed: Vec<&str> = mine.iter().chain(outside).map(String::as_str).collect();
    match outcome {
        _ if check && !changed.is_empty() => {
            format!("{}: FAILED wrote {} in --check", step.name, changed.join(", "))
        }
        _ if !outside.is_empty() => format!(
            "{}: FAILED rewrote files outside its outputs: {}",
            step.name,
            outside.join(", ")
        ),
        Err(e) => format!("{}: FAILED {e:#}", step.name),
        Ok(o) if o.ok && mine.is_empty() => format!("{}: unchanged", step.name),
        Ok(o) if o.ok => format!("{}: rewrote {}", step.name, mine.join(", ")),
        Ok(o) if check && step.phase == Phase::Regen => {
            format!("{}: would rewrite {}", step.name, o.detail)
        }
        Ok(o) => format!("{}: FAILED {}", step.name, o.detail),
    }
}

/// Runs the action of `step`.
fn execute(step: &Step, root: &Path, check: bool, files: &BTreeSet<String>, all: bool) -> Result<Outcome> {
    match step.action {
        Action::Rustfmt => rustfmt(root, &rust_files(root, files, all)?, check),
        Action::BlessTest { .. } | Action::BlessBin { .. } => bless_group(root, &[step], check)?
            .pop()
            .context("a group of one has one outcome")?,
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
        Action::CheckBrief => {
            let tasks = briefs_to_check(&brief_tasks(root)?, files, all);
            if tasks.is_empty() {
                return Ok(Outcome::of(true, ""));
            }
            crate::blind::selftest::build(root)?;
            let bin = root.join("target/debug");
            let mut failed = Vec::new();
            for task in &tasks {
                println!("check-brief {task}:");
                let c = crate::blind::brief::check_brief(root, task, &bin)?;
                print!("{}", c.text(task));
                if !c.runnable() {
                    failed.push(task.as_str());
                }
            }
            Ok(Outcome {
                ok: failed.is_empty(),
                detail: format!("not runnable: {} (see the report above)", failed.join(", ")),
            })
        }
        Action::Msrv => msrv(root),
        Action::Records => Ok(Outcome::of(
            crate::records::run(root, check)?,
            "run `cargo xtask records --write`",
        )),
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

/// Whether a step runs in this mode, and the line its skip prints (once per line).
#[derive(Debug, PartialEq, Eq)]
enum Gate {
    Run,
    Skip(Option<&'static str>),
}

/// The mode's gate of `step`: `--check` skips the self-tests; the review gates and merge-point checks need
/// `--merge`, and in the lane's mode (`--check` without `--merge`) the merge-point checks say so.
fn gate(step: &Step, a: &Args) -> Gate {
    match step.action {
        Action::Selftest { .. } if a.check => Gate::Skip(Some(SELFTESTS_NOT_CHECKED)),
        Action::CheckBrief | Action::Msrv if !a.merge => {
            Gate::Skip(a.check.then_some(MERGE_CHECKS_NOT_CHECKED))
        }
        _ if step.phase == Phase::MergeOnly && !a.merge => Gate::Skip(None),
        _ => Gate::Run,
    }
}

/// `cargo xtask settle [--check] [--merge] [--all] [--base <rev>]`.
///
/// # Errors
///
/// Bad arguments, or a git or cargo metadata call that fails.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    let a = parse(args)?;
    let mut merge_base: Option<String> = None;
    let mut files: BTreeSet<String> = if a.all {
        BTreeSet::new()
    } else {
        let base = a.base.as_deref().unwrap_or(DEFAULT_BASE);
        let mb = affected::merge_base(root, base)?;
        let files = affected::changed_files(root, &mb)?;
        println!(
            "settle: {} changed file(s) since {base} (merge base {})",
            files.len(),
            &mb[..mb.len().min(12)]
        );
        merge_base = Some(mb);
        files.into_iter().collect()
    };
    let mut members: Option<Members> = None;
    let mut lines: Vec<String> = Vec::new();
    let mut ok = true;
    let mut noted: BTreeSet<&str> = BTreeSet::new();
    let mut blessed = false;
    let emit = |line: String, lines: &mut Vec<String>, ok: &mut bool| {
        if line.contains(": FAILED ") || line.contains(": would rewrite ") {
            *ok = false;
        }
        println!("{line}");
        lines.push(line);
    };
    for step in STEPS {
        if let Gate::Skip(note) = gate(step, &a) {
            if let Some(note) = note {
                if noted.insert(note) {
                    emit(note.to_owned(), &mut lines, &mut ok);
                }
            }
            continue;
        }
        if !a.all && (is_bless(step) || !step.deps_of.is_empty()) && members.is_none() {
            members = Some(affected::members_of(&crate::metadata_all_features(root)?)?);
        }
        if is_bless(step) {
            // The whole bless block at the first bless step: one nested call per round.
            if !blessed {
                blessed = true;
                for line in bless_block(root, &a, &mut files, members.as_ref())? {
                    emit(line, &mut lines, &mut ok);
                }
            }
            continue;
        }
        if !a.all {
            let fired = triggered(step, &files, members.as_ref())
                || (matches!(step.action, Action::Msrv)
                    && msrv_triggered(root, merge_base.as_deref(), &files)?);
            if !fired {
                emit(
                    format!("{}: skipped (no trigger)", step.name),
                    &mut lines,
                    &mut ok,
                );
                continue;
            }
        }
        println!("settle: {} ...", step.name);
        let before = snapshot(root)?;
        let outcome = execute(step, root, a.check, &files, a.all);
        let changed = snapshot_diff(&before, &snapshot(root)?);
        let (mine, outside) = attribute(&[step], &changed);
        let mine = mine.get(step.name).cloned().unwrap_or_default();
        emit(
            step_line(step, a.check, outcome, &mine, &outside),
            &mut lines,
            &mut ok,
        );
        files.extend(changed);
    }
    println!("\nsettle{}:", if a.check { " --check" } else { "" });
    for line in &lines {
        println!("  {line}");
    }
    Ok(ok)
}

/// The bless steps of the next nested call, in table order. A candidate is a step this mode runs that has not run
/// and is triggered by `files` (every one with `--all`), or that ran in round `r` (`done`) and reads a file changed
/// in round `r` or later (`rounds`). A candidate that reads an output of another candidate waits for a later call,
/// unless every candidate waits (a cycle of reads: then all run).
fn next_group(
    a: &Args,
    files: &BTreeSet<String>,
    members: Option<&Members>,
    done: &BTreeMap<&str, usize>,
    rounds: &[Vec<String>],
) -> Vec<&'static Step> {
    let candidates: Vec<&'static Step> = STEPS
        .iter()
        .filter(|s| is_bless(s) && gate(s, a) == Gate::Run)
        .filter(|s| match done.get(s.name) {
            None => a.all || triggered(s, files, members),
            Some(&r) => rounds
                .iter()
                .skip(r)
                .flatten()
                .any(|f| s.reads.iter().any(|g| glob_match(g, f))),
        })
        .collect();
    let ready: Vec<&'static Step> = candidates
        .iter()
        .filter(|c| {
            !candidates
                .iter()
                .any(|p| p.name != c.name && p.outputs.iter().any(|o| c.reads.contains(o)))
        })
        .copied()
        .collect();
    if ready.is_empty() { candidates } else { ready }
}

/// The bless steps in rounds: each round runs [`next_group`] as one nested call (a later round when a step waits
/// for another's outputs, or when a call's rewrites trigger a further step or change what a step that ran reads).
/// Returns their lines in table order (a step that ran twice: its last line); `files` gains every file a call
/// changed.
fn bless_block(
    root: &Path,
    a: &Args,
    files: &mut BTreeSet<String>,
    members: Option<&Members>,
) -> Result<Vec<String>> {
    let block: Vec<&Step> = STEPS
        .iter()
        .filter(|s| is_bless(s) && gate(s, a) == Gate::Run)
        .collect();
    let mut done: BTreeMap<&str, usize> = BTreeMap::new();
    let mut lines: BTreeMap<&str, String> = BTreeMap::new();
    let mut rounds: Vec<Vec<String>> = Vec::new();
    // A bound on the rounds whatever the reads (a cycle of reads would otherwise repeat).
    while rounds.len() < 2 * block.len() {
        let group = next_group(a, files, members, &done, &rounds);
        if group.is_empty() {
            break;
        }
        println!(
            "settle: {} ... (one nextest call)",
            group.iter().map(|s| s.name).collect::<Vec<_>>().join(", ")
        );
        let before = snapshot(root)?;
        let outcomes = bless_group(root, &group, a.check);
        let changed = snapshot_diff(&before, &snapshot(root)?);
        let (mine, outside) = attribute(&group, &changed);
        let outcomes: Vec<Result<Outcome>> = match outcomes {
            Ok(o) => o,
            Err(e) => {
                let e = format!("{e:#}");
                group.iter().map(|_| Err(anyhow::anyhow!("{e}"))).collect()
            }
        };
        for (s, o) in group.iter().zip(outcomes) {
            let m = mine.get(s.name).cloned().unwrap_or_default();
            lines.insert(s.name, step_line(s, a.check, o, &m, &outside));
            done.insert(s.name, rounds.len());
        }
        files.extend(changed.iter().cloned());
        rounds.push(changed);
    }
    Ok(block
        .iter()
        .map(|s| {
            lines
                .remove(s.name)
                .unwrap_or_else(|| format!("{}: skipped (no trigger)", s.name))
        })
        .collect())
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
                "check-brief",
                "msrv",
                "records",
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
        // A step that writes has outputs; a check (the merge-point checks too) has none.
        for s in STEPS {
            let writes = !matches!(
                s.action,
                Action::Symbols | Action::FuzzBuild | Action::CheckBrief | Action::Msrv
            );
            assert_eq!(writes, !s.outputs.is_empty(), "{}", s.name);
            assert_eq!(
                s.phase == Phase::Regen,
                writes && s.phase != Phase::MergeOnly,
                "{}",
                s.name
            );
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
            ("check-brief", "tools/blind/tasks/brief-plc-io/task.md"),
            ("check-brief", "tools/blind/tasks/selftest/agent.jsonl"),
            ("check-brief", "tools/blind/HELP-VISUAL.md"),
            ("msrv", "Cargo.lock"),
            ("golden-change-list", "docs/plan/decisions.md"),
            // Records follow the history: any change triggers them.
            ("records", "README.md"),
            ("records", "crates/ew-x/src/lib.rs"),
        ];
        let empty = Members::new();
        for (name, path) in cases {
            assert!(
                triggered(step(name), &set(&[path]), Some(&empty)),
                "{name} by {path}"
            );
        }
        for s in STEPS.iter().filter(|s| s.name != "records") {
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

    /// A `deps_of` step is triggered by its package, by a package it depends on directly and by the build-wide
    /// files; not by a package further down (the safety net is the merge point's verify), by a package that depends
    /// on it or by a dev-dependency.
    #[test]
    fn a_deps_of_step_follows_its_direct_dependencies() {
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
            "crates/ew-model/src/lib.rs",
            "Cargo.lock",
            "rust-toolchain.toml",
        ] {
            assert!(triggered(schemas, &set(&[f]), Some(&members)), "{f}");
        }
        for f in [
            // Transitive: ew-commands -> ew-model -> ew-units.
            "crates/ew-units/src/lib.rs",
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
        for o in [BOOK_SUMMARY, FUZZ_TABLE, GOLDEN_LIST, GLOBAL_PLAN] {
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

    fn args(check: bool, merge: bool) -> Args {
        Args {
            check,
            merge,
            ..Args::default()
        }
    }

    fn names(steps: &[&Step]) -> Vec<&'static str> {
        steps.iter().map(|s| s.name).collect()
    }

    /// The triggered bless steps join one call, in table order; a step already run does not join again; `--all`
    /// takes every one.
    #[test]
    fn the_triggered_bless_steps_join_one_call() {
        let member = |dir: &str, deps: &[&str]| Member {
            dir: dir.to_owned(),
            deps: deps.iter().map(|s| (*s).to_owned()).collect(),
            ..Member::default()
        };
        let members: Members = [
            ("ew-units", member("crates/ew-units", &[])),
            ("ew-model", member("crates/ew-model", &["ew-units"])),
            ("ew-commands", member("crates/ew-commands", &["ew-model"])),
            ("ew-viewmodel", member("crates/ew-viewmodel", &["ew-commands"])),
            ("ew", member("bin/ew", &["ew-commands"])),
        ]
        .into_iter()
        .map(|(n, m)| (n.to_owned(), m))
        .collect();
        let none: BTreeMap<&str, usize> = BTreeMap::new();
        let lane = args(true, false);
        let files = set(&["crates/ew-viewmodel/src/strings.rs"]);
        assert_eq!(
            names(&next_group(&lane, &files, Some(&members), &none, &[])),
            ["keymaps", "blind-reference"]
        );
        let files = set(&[
            "crates/ew-commands/src/lib.rs",
            "crates/ew-viewmodel/src/strings.rs",
        ]);
        // cli-schemas reads command-schemas' outputs: it waits for the next call.
        assert_eq!(
            names(&next_group(&lane, &files, Some(&members), &none, &[])),
            ["command-schemas", "keymaps", "blind-reference", "cli-snapshot"]
        );
        // A transitive change triggers none.
        let files = set(&["crates/ew-units/src/lib.rs"]);
        assert!(next_group(&lane, &files, Some(&members), &none, &[]).is_empty());
        // The second round: only a step not yet run.
        let done = BTreeMap::from([("keymaps", 0), ("blind-reference", 0)]);
        let files = set(&[
            "crates/ew-viewmodel/src/strings.rs",
            "tools/blind/reference/actions.txt",
        ]);
        assert!(next_group(&lane, &files, Some(&members), &done, &[vec![]]).is_empty());
        let all = Args {
            all: true,
            ..Args::default()
        };
        assert_eq!(
            names(&next_group(&all, &BTreeSet::new(), None, &none, &[])),
            [
                "command-schemas",
                "rule-catalogue",
                "keymaps",
                "blind-reference",
                "cli-snapshot",
                "api-v1-snapshot",
                "query-snapshot",
            ]
        );
    }

    /// command-schemas and cli-schemas both triggered: cli-schemas, which reads the command schemas, runs in the
    /// call after command-schemas', never in the same one; and again when they change after it ran.
    #[test]
    fn a_step_that_reads_another_steps_outputs_runs_after_it() {
        let lane = args(true, false);
        let empty = Members::new();
        let files = set(&[
            "crates/ew-commands/schemas/index.json",
            "bin/ew/schemas/index.json",
        ]);
        let mut done: BTreeMap<&str, usize> = BTreeMap::new();
        assert_eq!(
            names(&next_group(&lane, &files, Some(&empty), &done, &[])),
            ["command-schemas"]
        );
        done.insert("command-schemas", 0);
        let rounds = [vec![]];
        assert_eq!(
            names(&next_group(&lane, &files, Some(&empty), &done, &rounds)),
            ["cli-schemas"]
        );
        done.insert("cli-schemas", 1);
        let rounds = [vec![], vec![]];
        assert!(next_group(&lane, &files, Some(&empty), &done, &rounds).is_empty());
        // A rewrite of the command schemas after cli-schemas ran (round 1) runs it again; command-schemas does not
        // read its own outputs.
        let rounds = [vec![], vec!["crates/ew-commands/schemas/index.json".to_owned()]];
        assert_eq!(
            names(&next_group(&lane, &files, Some(&empty), &done, &rounds)),
            ["cli-schemas"]
        );
        // A rewrite before it ran (round 0) does not.
        let rounds = [vec!["crates/ew-commands/schemas/index.json".to_owned()], vec![]];
        let done = BTreeMap::from([("command-schemas", 0), ("cli-schemas", 1)]);
        assert!(next_group(&lane, &files, Some(&empty), &done, &rounds).is_empty());
        // Every read is the output of another bless step, written as that step writes it.
        for s in STEPS {
            for r in s.reads {
                assert!(
                    STEPS
                        .iter()
                        .any(|o| o.name != s.name && is_bless(o) && o.outputs.contains(r)),
                    "{}: {r}",
                    s.name
                );
            }
        }
    }

    /// The binaries with a matching, not ignored test, from nextest's JSON list; a step whose term matches none
    /// fails with its filter named.
    #[test]
    fn a_term_that_matches_no_test_fails() {
        let json = r#"{"rust-build-meta": {}, "test-count": 3, "rust-suites": {
            "ew::it": {"status": "listed", "testcases": {
                "conventions::a": {"kind": "test", "ignored": false, "filter-match": {"status": "matches"}},
                "other::b": {"kind": "test", "ignored": false,
                    "filter-match": {"status": "mismatch", "reason": "expression"}}}},
            "ew-api::it": {"status": "listed", "testcases": {
                "queries::q": {"kind": "test", "ignored": true, "filter-match": {"status": "matches"}}}},
            "ew-api::perf": {"status": "skipped", "testcases": {}}}}"#;
        assert_eq!(matched_binaries(json).unwrap(), set(&["ew::it"]));
        assert_eq!(
            matched_binaries("{}").unwrap_err().to_string(),
            "nextest's test list has no `rust-suites`"
        );
        let s = step("query-snapshot");
        assert_eq!(
            step_line(
                s,
                true,
                Err(anyhow::anyhow!("its filter `{}` matches no test", term(s))),
                &[],
                &[]
            ),
            "query-snapshot: FAILED its filter `(binary_id(ew-api::it) & \
             test(the_query_snapshot_equals_the_registry))` matches no test"
        );
    }

    /// The MSRV check also fires on a changed source or manifest of a permissive crate, not of another crate, and
    /// not on another file of a permissive crate.
    #[test]
    fn the_msrv_check_fires_on_a_permissive_crates_source() {
        let member = |dir: &str| Member {
            dir: dir.to_owned(),
            ..Member::default()
        };
        let members: Members = [
            ("ew-units", member("crates/ew-units")),
            ("ew-model", member("crates/ew-model")),
        ]
        .into_iter()
        .map(|(n, m)| (n.to_owned(), m))
        .collect();
        let permissive = ["ew-units".to_owned()];
        for f in ["crates/ew-units/src/lib.rs", "crates/ew-units/Cargo.toml"] {
            assert!(
                permissive_source_changed(&set(&[f]), &members, &permissive),
                "{f}"
            );
        }
        for f in [
            "crates/ew-model/src/lib.rs",
            "crates/ew-model/Cargo.toml",
            "crates/ew-units/README.md",
            "xtask/src/settle.rs",
        ] {
            assert!(
                !permissive_source_changed(&set(&[f]), &members, &permissive),
                "{f}"
            );
        }
    }

    /// The nested call's command line: the packages once each, the union of the binaries and filters, and the
    /// bless variables only without `--check`.
    #[test]
    fn a_bless_group_runs_as_one_nextest_call() {
        let group = [step("command-schemas"), step("keymaps"), step("blind-reference")];
        let (args, envs, shown) = group_command(&group, false);
        assert_eq!(
            shown,
            "EW_BLESS=1 EW_COMMANDS_BLESS=1 cargo nextest run --profile lane --all-features -p ew-commands \
             -p ew-viewmodel -E 'binary_id(ew-commands::schemas) | binary_id(ew-viewmodel::keymaps) | \
             binary_id(ew-viewmodel::blind_reference)'"
        );
        assert_eq!(envs, ["EW_BLESS", "EW_COMMANDS_BLESS"]);
        assert_eq!(
            args,
            [
                "nextest",
                "run",
                "--profile",
                "lane",
                "--all-features",
                "-p",
                "ew-commands",
                "-p",
                "ew-viewmodel",
                "-E",
                "binary_id(ew-commands::schemas) | binary_id(ew-viewmodel::keymaps) | \
                 binary_id(ew-viewmodel::blind_reference)",
            ]
        );
        let group = [step("cli-schemas"), step("cli-snapshot"), step("query-snapshot")];
        let (_, envs, shown) = group_command(&group, true);
        assert!(envs.is_empty());
        assert_eq!(
            shown,
            "cargo nextest run --profile lane --all-features -p ew -p ew-api -E '(binary_id(ew::it) & \
             test(/^conventions::/)) | (binary_id(ew::bin/ew) & test(/^catalogue::/)) | (binary_id(ew-api::it) & \
             test(the_query_snapshot_equals_the_registry))'"
        );
    }

    /// A failure is given to the step(s) of its binary id; changed files to the step whose outputs match them.
    #[test]
    fn a_group_names_its_failed_steps_and_their_files() {
        let group = [step("command-schemas"), step("keymaps"), step("cli-snapshot")];
        let output = "    Starting 4 tests across 3 binaries\n\
            \x20       FAIL [   0.012s] ( 1/4) ew-viewmodel::keymaps the_keymaps_are_current\n\
            --- STDOUT: ew-commands::schemas x ---\n\
            \x20       SIGABRT [   0.100s] ew::bin/ew catalogue::the_snapshot_is_current\n\
            \x20       FAIL [   0.012s] ( 1/4) ew-viewmodel::keymaps the_keymaps_are_current\n\
            \x20    Summary [   1.000s] 4 tests run: 2 passed, 2 failed\n";
        let named = failed_steps(&group, output);
        assert_eq!(
            named,
            BTreeMap::from([
                (
                    "cli-snapshot",
                    vec!["SIGABRT [   0.100s] ew::bin/ew catalogue::the_snapshot_is_current".to_owned()]
                ),
                (
                    "keymaps",
                    vec!["FAIL [   0.012s] ( 1/4) ew-viewmodel::keymaps the_keymaps_are_current".to_owned()]
                ),
            ])
        );
        assert!(failed_steps(&group, "error[E0425]: cannot find value `x`\n").is_empty());

        let changed = [
            "crates/ew-commands/schemas/index.json".to_owned(),
            "crates/ew-viewmodel/keymaps/ew.toml".to_owned(),
            "README.md".to_owned(),
        ];
        let (mine, outside) = attribute(&group, &changed);
        assert_eq!(
            mine,
            BTreeMap::from([
                ("command-schemas", vec![changed[0].clone()]),
                ("keymaps", vec![changed[1].clone()]),
            ])
        );
        assert_eq!(outside, ["README.md"]);
        let s = step("keymaps");
        assert_eq!(
            step_line(s, false, Ok(Outcome::of(true, "")), &changed[1..2], &[]),
            "keymaps: rewrote crates/ew-viewmodel/keymaps/ew.toml"
        );
        assert_eq!(
            step_line(s, false, Ok(Outcome::of(true, "")), &changed[1..2], &outside),
            "keymaps: FAILED rewrote files outside its outputs: README.md"
        );
        assert_eq!(
            step_line(s, true, Ok(Outcome::of(false, "x failed")), &[], &[]),
            "keymaps: would rewrite x failed"
        );
    }

    /// The lane's mode skips the self-tests and the merge-point checks, each with its line; `--merge` runs them.
    #[test]
    fn check_skips_check_brief_and_msrv() {
        assert_eq!(
            MERGE_CHECKS_NOT_CHECKED,
            "check-brief, msrv: not checked here (settle --merge runs them)"
        );
        let lane = args(true, false);
        for name in ["check-brief", "msrv"] {
            assert_eq!(
                gate(step(name), &lane),
                Gate::Skip(Some(MERGE_CHECKS_NOT_CHECKED)),
                "{name}"
            );
            assert_eq!(gate(step(name), &args(false, false)), Gate::Skip(None), "{name}");
            assert_eq!(gate(step(name), &args(false, true)), Gate::Run, "{name}");
            assert_eq!(gate(step(name), &args(true, true)), Gate::Run, "{name}");
        }
        assert_eq!(
            gate(step("selftest"), &lane),
            Gate::Skip(Some(SELFTESTS_NOT_CHECKED))
        );
        assert_eq!(gate(step("disallowed-methods"), &lane), Gate::Skip(None));
        assert_eq!(gate(step("keymaps"), &lane), Gate::Run);
    }

    /// The MSRV check fires on `Cargo.lock` and on a changed `rust-version` line, not on another manifest change.
    #[test]
    fn the_msrv_check_fires_on_the_lockfile_and_rust_version() {
        let msrv = step("msrv");
        let empty = Members::new();
        assert!(triggered(msrv, &set(&["Cargo.lock"]), Some(&empty)));
        assert!(!triggered(
            msrv,
            &set(&["crates/ew-units/Cargo.toml"]),
            Some(&empty)
        ));
        let old = "[package]\nname = \"a\"\nversion = \"0.1.0\"\nrust-version.workspace = true\n";
        assert!(changes_rust_version(
            old,
            &old.replace("rust-version.workspace = true", "rust-version = \"1.92\"")
        ));
        assert!(changes_rust_version(
            "rust-version = \"1.85\"\n",
            "rust-version = \"1.86\"\n"
        ));
        assert!(changes_rust_version("", old), "a new manifest");
        assert!(!changes_rust_version(old, &old.replace("0.1.0", "0.2.0")));
        assert!(!changes_rust_version(old, &format!("{old}\n# a comment\n")));
    }

    /// The permissive list is derived from the members' licences (xtask aside, non-members ignored), and the
    /// toolchain from their highest `rust-version`.
    #[test]
    fn the_permissive_crates_come_from_the_licences() {
        let package = |name: &str, license: Option<&str>, rust: Option<&str>| {
            serde_json::json!({
                "name": name,
                "version": "0.0.0",
                "id": format!("path+file:///w/{name}#0.0.0"),
                "source": null,
                "dependencies": [],
                "license": license,
                "targets": [],
                "features": {},
                "manifest_path": format!("/w/{name}/Cargo.toml"),
                "rust_version": rust,
            })
        };
        let metadata: cargo_metadata::Metadata = serde_json::from_value(serde_json::json!({
            "packages": [
                package("ew-units", Some(PERMISSIVE), Some("1.85")),
                package("ew-model", Some("GPL-3.0-or-later"), Some("1.85")),
                package("xtask", Some(PERMISSIVE), Some("1.85")),
                package("ew-dl", Some(PERMISSIVE), Some("1.85.1")),
                package("serde", Some(PERMISSIVE), Some("1.31")),
                package("ew-mltext", None, None),
            ],
            "workspace_members": [
                "path+file:///w/ew-units#0.0.0",
                "path+file:///w/ew-model#0.0.0",
                "path+file:///w/xtask#0.0.0",
                "path+file:///w/ew-dl#0.0.0",
                "path+file:///w/ew-mltext#0.0.0",
            ],
            "resolve": null,
            "workspace_root": "/w",
            "target_directory": "/w/target",
            "version": 1,
        }))
        .unwrap();
        let plan = msrv_plan(&metadata).unwrap();
        assert_eq!(
            plan,
            MsrvPlan {
                packages: vec!["ew-dl".to_owned(), "ew-units".to_owned()],
                toolchain: "1.85.1".to_owned(),
            }
        );
        assert_eq!(
            msrv_command(&plan),
            "cargo +1.85.1 check --locked -p ew-dl -p ew-units"
        );
    }

    /// Every brief on a self-test trigger; else the briefs whose files changed.
    #[test]
    fn check_brief_proves_the_briefs_a_change_reaches() {
        let tasks = ["brief-a".to_owned(), "brief-b".to_owned()];
        assert_eq!(
            briefs_to_check(&tasks, &set(&["tools/blind/tasks/brief-b/task.md"]), false),
            ["brief-b"]
        );
        assert_eq!(
            briefs_to_check(&tasks, &set(&["xtask/src/blind/run.rs"]), false),
            ["brief-a", "brief-b"]
        );
        assert_eq!(
            briefs_to_check(&tasks, &BTreeSet::new(), true),
            ["brief-a", "brief-b"]
        );
        assert!(briefs_to_check(&tasks, &set(&["tools/blind/tasks/brief-c/task.md"]), false).is_empty());
        // Every self-test trigger is a trigger of the step.
        let ours = step("check-brief").triggers;
        for s in STEPS
            .iter()
            .filter(|s| matches!(s.action, Action::Selftest { .. }))
        {
            for t in s.triggers {
                assert!(ours.contains(t), "{t}");
            }
        }
    }
}
