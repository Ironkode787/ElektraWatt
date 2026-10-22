// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Replays the committed inputs through the targets' `check` functions without the fuzzer: every seed of
//! `corpus/<target>/` must pass (a seed that fails is a defect, or a property that is wrong), every input of
//! `regressions/<target>/` is a reported finding and is expected to fail until its defect is fixed (the test
//! lists them and asserts only that they do not abort the process), and the seeds must get as far as the
//! properties they were made for (the target is not vacuous). Run from `fuzz/`: `cargo test --release`.

use std::path::{Path, PathBuf};

use ew_fuzz::loader;
use ew_fuzz::open_path::{self, Outcome};

fn inputs(dir: &str) -> Vec<(String, Vec<u8>)> {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join(dir);
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_file() && path.file_name().is_some_and(|n| n != ".gitkeep") {
                let name = path
                    .file_name()
                    .expect("a file name")
                    .to_string_lossy()
                    .into_owned();
                out.push((name, std::fs::read(&path).expect("a readable seed")));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_loader_seeds_pass_and_reach_the_round_trip() {
    let seeds = inputs("corpus/jsonl_loader");
    assert!(seeds.len() >= 8, "the loader's seed corpus is missing");
    let mut round_tripped = 0;
    let mut valid = 0;
    for (name, bytes) in &seeds {
        let outcome = loader::check(bytes);
        eprintln!("{name}: {outcome:?}");
        round_tripped += usize::from(outcome.round_tripped);
        valid += usize::from(outcome.valid);
    }
    assert!(
        round_tripped >= 4,
        "only {round_tripped} loader seeds reached the round trip"
    );
    assert!(
        valid >= 2,
        "only {valid} loader seeds are valid inputs (the strong round-trip check)"
    );
}

#[test]
fn the_migration_chain_seeds_pass_and_rewrite_old_shapes() {
    let seeds = inputs("corpus/migration_chain");
    assert!(seeds.len() >= 15, "the migration chain's seed corpus is missing");
    let outcomes: Vec<(String, Outcome)> = seeds
        .iter()
        .map(|(name, bytes)| (name.clone(), open_path::check(bytes)))
        .collect();
    for (name, outcome) in &outcomes {
        eprintln!("{name}: {outcome:?}");
    }
    let get = |name: &str| {
        outcomes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, o)| *o)
            .expect("a seed")
    };
    assert_eq!(get("golden-as-it-is"), Outcome::Reopened { rewritten: 0 });
    assert_eq!(get("golden-read-only"), Outcome::ReadOnly);
    // The older build's revisions open, and the save rewrites their file (the typed revisions, FT-165).
    assert!(
        matches!(get("old-shape-revisions"), Outcome::Reopened { rewritten } if rewritten >= 1),
        "{:?}",
        get("old-shape-revisions")
    );
    // The findings of cycle 22. A BOM before a library copy is a shape: the project opens with the golden model,
    // and the save rewrites the copy (and the revision whose member order the input changed as well).
    let bom = open_path::decode(&inputs_named(&seeds, "lib-document-bom"));
    assert!(bom.ops.iter().all(|op| open_path::shape_only(*op)), "{bom:?}");
    assert!(
        matches!(get("lib-document-bom"), Outcome::Reopened { rewritten } if rewritten >= 1),
        "{:?}",
        get("lib-document-bom")
    );
    assert_eq!(get("lib-document-bom-read-only"), Outcome::ReadOnly);
    // A swap of two lines inside a library document is no shape (it moved `sha256` into `meta`): the oracle no
    // longer asks for the golden model, and the project still opens, read-only as the mode byte asks.
    let swap = open_path::decode(&inputs_named(&seeds, "lib-document-line-swap"));
    assert!(!swap.ops.iter().all(|op| open_path::shape_only(*op)), "{swap:?}");
    assert_eq!(get("lib-document-line-swap"), Outcome::ReadOnly);
}

fn inputs_named(seeds: &[(String, Vec<u8>)], name: &str) -> Vec<u8> {
    seeds
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, bytes)| bytes.clone())
        .unwrap_or_else(|| panic!("no seed {name}"))
}

#[test]
fn the_command_seeds_pass_and_commit_commands() {
    let seeds = inputs("corpus/command_sequence");
    assert!(seeds.len() >= 4, "the command target's seed corpus is missing");
    for (name, bytes) in &seeds {
        let committed = ew_fuzz::command::check(bytes);
        eprintln!("{name}: {committed} commands committed");
        assert!(
            committed >= 1,
            "{name}: no command committed, the generator is degenerate"
        );
    }
}

/// The reported findings: each input of `regressions/<target>/` is replayed in this ordinary test process, where a
/// panic is caught and the generator's own failure text (the seed, the step, the oracle and the command log of
/// `command_sequence`) is printed, which libFuzzer's aborting panic hook never lets the fuzz target do. Nothing is
/// asserted about the outcome: a finding fails by definition, until its defect is fixed and the input becomes a plain
/// test of the crate that was fixed. Run with `--nocapture`.
#[test]
fn the_regressions_are_replayed_and_reported() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    type Check = fn(&[u8]);
    let targets: [(&str, Check); 3] = [
        ("regressions/jsonl_loader", |b| {
            let _ = loader::check(b);
        }),
        ("regressions/migration_chain", |b| {
            let _ = open_path::check(b);
        }),
        ("regressions/command_sequence", |b| {
            let _ = ew_fuzz::command::check(b);
        }),
    ];
    for (dir, check) in targets {
        for (name, bytes) in inputs(dir) {
            let failed = catch_unwind(AssertUnwindSafe(|| check(&bytes))).is_err();
            eprintln!(
                "regression {dir}/{name}: {} bytes, {}",
                bytes.len(),
                if failed {
                    "fails (a reported finding)"
                } else {
                    "passes (fixed: make it a plain test of the crate)"
                }
            );
        }
    }
}
