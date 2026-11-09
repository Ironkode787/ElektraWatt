// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `gen [--check]`: every generated file of the repository, written by one command in one order.
//!
//! Two kinds of step. An *own* step is a generator inside xtask (`shell_keys`, `docs_gen`, `icons`, `raw_read`,
//! `symbols`): it writes its file, or with `--check` compares it. A *snapshot* step is a test of a product crate
//! that compares a committed file with what the code produces and rewrites it when its environment variable is set;
//! `gen` runs that one test with the variable set. `--check` leaves the snapshot steps to the test run, which
//! compares the same files.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

/// What a step runs.
enum Step {
    /// A generator of xtask: `(root, check) -> up to date or written`.
    Own(fn(&Path, bool) -> Result<bool>),
    /// One nextest call that rewrites committed snapshots: the package, the target arguments, an optional filter
    /// and the environment variable that makes the test write.
    Snapshot {
        package: &'static str,
        target: &'static [&'static str],
        filter: Option<&'static str>,
        env: &'static str,
    },
}

/// The steps in their order: the text keys first, since every later step builds `ew-viewmodel`; the command
/// schemas before the CLI schemas and the book, which read them.
const STEPS: &[(&str, Step)] = &[
    ("shell-keys", Step::Own(crate::shell_keys::run)),
    (
        "command-schemas",
        Step::Snapshot {
            package: "ew-commands",
            target: &["--test", "schemas"],
            filter: None,
            env: "EW_COMMANDS_BLESS",
        },
    ),
    (
        "cli-schemas",
        Step::Snapshot {
            package: "ew",
            target: &["--test", "it"],
            filter: Some("test(/^conventions::/)"),
            env: "EW_CLI_BLESS",
        },
    ),
    (
        "rule-catalogue",
        Step::Snapshot {
            package: "ew-checks",
            target: &["--test", "catalogue"],
            filter: None,
            env: "EW_CHECKS_BLESS",
        },
    ),
    (
        "keymaps",
        Step::Snapshot {
            package: "ew-viewmodel",
            target: &["--test", "keymaps"],
            filter: None,
            env: "EW_BLESS",
        },
    ),
    (
        "blind-reference",
        Step::Snapshot {
            package: "ew-viewmodel",
            target: &["--test", "blind_reference"],
            filter: None,
            env: "EW_BLESS",
        },
    ),
    (
        "cli-snapshot",
        Step::Snapshot {
            package: "ew",
            target: &["--bin", "ew"],
            filter: Some("test(/^catalogue::/)"),
            env: "EW_CLI_BLESS",
        },
    ),
    (
        "api-v1-snapshot",
        Step::Snapshot {
            package: "ew-sqlview",
            target: &["--test", "it"],
            filter: Some("test(/^api_v1::/)"),
            env: "EW_BLESS",
        },
    ),
    (
        "query-snapshot",
        Step::Snapshot {
            package: "ew-api",
            target: &["--test", "it"],
            filter: Some("test(the_query_snapshot_equals_the_registry)"),
            env: "EW_BLESS",
        },
    ),
    ("docs-gen", Step::Own(crate::docs_gen::run)),
    ("gen-icons", Step::Own(crate::icons::run)),
    // The rule `raw-read` compares the list; here it is only written.
    (
        "lint-list",
        Step::Own(|root, check| {
            if check {
                Ok(true)
            } else {
                crate::raw_read::run(root, true)
            }
        }),
    ),
    ("symbols-qet10", Step::Own(crate::symbols::run)),
];

/// `gen [--check]`.
pub(crate) fn run(root: &Path, check: bool) -> Result<bool> {
    if check {
        let ok = check_own(root)?;
        println!("gen: the snapshots are compared by their tests (`cargo xtask check`)");
        return Ok(ok);
    }
    let mut ok = true;
    for (name, step) in STEPS {
        println!("==> gen: {name}");
        let passed = match step {
            Step::Own(run) => run(root, false)?,
            Step::Snapshot {
                package,
                target,
                filter,
                env,
            } => {
                let mut command = Command::new(crate::cargo());
                command
                    .current_dir(root)
                    .env(env, "1")
                    .args(["nextest", "run", "--cargo-quiet", "--all-features", "-p", package])
                    .args(*target);
                if let Some(filter) = filter {
                    command.args(["-E", filter]);
                }
                command
                    .status()
                    .with_context(|| format!("running the snapshot test of {name}"))?
                    .success()
            }
        };
        if !passed {
            eprintln!("gen: {name} FAILED");
            ok = false;
        }
    }
    let changed = crate::repo::git(root, &["status", "--porcelain"])?;
    if changed.trim().is_empty() {
        println!("gen: nothing changed");
    } else {
        println!(
            "gen: the working tree now differs from HEAD in:\n{}",
            changed.trim_end()
        );
    }
    Ok(ok)
}

/// The own steps in check mode: `true` when every file xtask writes itself is up to date.
pub(crate) fn check_own(root: &Path) -> Result<bool> {
    let mut ok = true;
    for (name, step) in STEPS {
        if let Step::Own(run) = step
            && !run(root, true)?
        {
            eprintln!("gen: {name} is stale; run `cargo xtask gen`");
            ok = false;
        }
    }
    Ok(ok)
}
