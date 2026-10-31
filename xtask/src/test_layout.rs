// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-test-layout`: each package's integration tests build as one test binary, `tests/it/main.rs`, with
//! one module per test file (owner, cycle 41: every test binary links the crate and all its dependencies and
//! writes its own debug file, so a crate links once).
//!
//! A top-level `.rs` file under a package's `tests/` is allowed only when it is one of the separate binaries
//! in [`SEPARATE`] (a workflow, xtask, nextest, a bless command or a data-file header names it), or when it is
//! the package's only loose test file and the package has no `tests/it/`. Subdirectories (helpers such as
//! `tests/common/`, data, a directory binary with its own `main.rs`) are not checked.

use std::path::Path;

/// The separate test binaries, per package directory: the only top-level files a `tests/` folder may keep
/// beside `tests/it/`. Each one is named by binary somewhere else, so it keeps its own binary and name.
pub const SEPARATE: &[(&str, &[&str])] = &[
    (
        "bin/elektrawatt",
        &["blind_replay", "canvas", "headless", "part2"],
    ),
    ("bin/ew", &["cli"]),
    (
        "crates/ew-api",
        &["acceptance", "checks", "golden_b", "memo_oracle", "page_output"],
    ),
    ("crates/ew-checks", &["catalogue"]),
    ("crates/ew-commands", &["perf", "schemas"]),
    ("crates/ew-derive", &["incremental_partition"]),
    ("crates/ew-export", &["raster_exporters"]),
    ("crates/ew-import-qet", &["hostile_input"]),
    ("crates/ew-rds", &["identifier_sets"]),
    ("crates/ew-store", &["fault_injection", "incremental", "purge"]),
    ("crates/ew-testkit", &["golden_dsl", "spike_memo"]),
    ("crates/ew-ui", &["canvas", "gpu", "snapshots"]),
    (
        "crates/ew-viewmodel",
        &[
            "blind_reference",
            "jobs",
            "keymaps",
            "part2",
            "probe",
            "replay_order",
        ],
    ),
];

/// The folders that hold packages.
const PACKAGE_PARENTS: &[&str] = &["bin", "crates"];

/// Checks every package under `root`; the violations name each misplaced file and where it belongs.
pub fn check(root: &Path) -> Vec<String> {
    let mut v = Vec::new();
    let mut packages = Vec::new();
    for parent in PACKAGE_PARENTS {
        let Ok(entries) = std::fs::read_dir(root.join(parent)) else {
            continue;
        };
        for e in entries.flatten() {
            if e.path().join("tests").is_dir() {
                packages.push(format!("{parent}/{}", e.file_name().to_string_lossy()));
            }
        }
    }
    packages.sort();
    for package in &packages {
        let tests = root.join(package).join("tests");
        let mut loose: Vec<String> = std::fs::read_dir(&tests)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_file())
                    .filter_map(|e| e.file_name().to_str()?.strip_suffix(".rs").map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        loose.sort();
        let separate = SEPARATE
            .iter()
            .find(|(p, _)| p == package)
            .map_or(&[][..], |(_, names)| *names);
        for name in separate {
            if !loose.iter().any(|l| l == name) {
                v.push(format!(
                    "{package}/tests/{name}.rs: listed as a separate test binary in xtask/src/test_layout.rs, \
                     but the file does not exist; remove it from the list"
                ));
            }
        }
        let others: Vec<&String> = loose.iter().filter(|l| !separate.contains(&l.as_str())).collect();
        let has_it = tests.join("it").is_dir();
        if others.len() == 1 && !has_it {
            continue;
        }
        for name in others {
            v.push(format!(
                "{package}/tests/{name}.rs: an integration test belongs in the package's one test binary; move \
                 it to {package}/tests/it/{name}.rs and declare it with `mod {name};` in \
                 {package}/tests/it/main.rs (a separate binary is only for a test that a workflow, xtask or \
                 nextest names: add it to the list in xtask/src/test_layout.rs)"
            ));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temporary folder, removed by the caller.
    fn fresh(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ew-xtask-layout-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn touch(root: &Path, rel: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "\n").unwrap();
    }

    #[test]
    fn a_loose_file_beside_the_it_binary_fails_with_where_it_belongs() {
        let dir = fresh("loose");
        touch(&dir, "crates/ew-format/tests/it/main.rs");
        touch(&dir, "crates/ew-format/tests/it/roundtrip.rs");
        touch(&dir, "crates/ew-format/tests/limits.rs");
        touch(&dir, "crates/ew-format/tests/common/mod.rs");
        let v = check(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            v,
            [
                "crates/ew-format/tests/limits.rs: an integration test belongs in the package's one test binary; \
                 move it to crates/ew-format/tests/it/limits.rs and declare it with `mod limits;` in \
                 crates/ew-format/tests/it/main.rs (a separate binary is only for a test that a workflow, xtask \
                 or nextest names: add it to the list in xtask/src/test_layout.rs)"
            ]
        );
    }

    #[test]
    fn separate_binaries_and_a_single_loose_file_pass() {
        let dir = fresh("pass");
        touch(&dir, "bin/ew/tests/cli.rs");
        touch(&dir, "bin/ew/tests/it/main.rs");
        touch(&dir, "crates/ew-text/tests/shaping.rs");
        touch(&dir, "crates/ew-testkit/tests/golden_dsl.rs");
        touch(&dir, "crates/ew-testkit/tests/spike_memo.rs");
        touch(&dir, "crates/ew-testkit/tests/command_fuzz/main.rs");
        let v = check(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(v, Vec::<String>::new());
    }

    #[test]
    fn two_loose_files_without_an_it_binary_fail_and_a_stale_entry_fails() {
        let dir = fresh("two");
        touch(&dir, "crates/ew-dl/tests/a.rs");
        touch(&dir, "crates/ew-dl/tests/b.rs");
        touch(&dir, "crates/ew-rds/tests/other.rs");
        let v = check(&dir);
        std::fs::remove_dir_all(&dir).unwrap();
        let expected: Vec<String> = ["a", "b"]
            .iter()
            .map(|n| {
                format!(
                    "crates/ew-dl/tests/{n}.rs: an integration test belongs in the package's one test binary; \
                     move it to crates/ew-dl/tests/it/{n}.rs and declare it with `mod {n};` in \
                     crates/ew-dl/tests/it/main.rs (a separate binary is only for a test that a workflow, xtask \
                     or nextest names: add it to the list in xtask/src/test_layout.rs)"
                )
            })
            .chain(std::iter::once(
                "crates/ew-rds/tests/identifier_sets.rs: listed as a separate test binary in \
                 xtask/src/test_layout.rs, but the file does not exist; remove it from the list"
                    .to_owned(),
            ))
            .collect();
        assert_eq!(v, expected);
    }

    #[test]
    fn the_workspace_follows_the_layout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert_eq!(check(root), Vec::<String>::new());
    }
}
