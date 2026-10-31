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
//!
//! The check also reads every `--test <name>` in the repository's text files (code, Markdown, TOML, YAML,
//! JSON, scripts): `<name>` must be a test binary of the package that the command names with `-p`, or of any
//! package when it names none. A command that names a module of `it` instead fails with the command to write.

use std::path::Path;

use anyhow::Result;

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

/// A package and its test binaries, as cargo discovers them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The package name (`[package] name`), which `-p` takes.
    pub name: String,
    /// The test binaries: each top-level `tests/*.rs`, each `tests/<dir>/main.rs` and each `[[test]]` name.
    pub binaries: Vec<String>,
    /// The modules of the package's `it` binary (`tests/it/*.rs` other than `main.rs`).
    pub it_modules: Vec<String>,
}

/// The extensions of the text files whose `--test` commands are checked.
const COMMAND_FILE_EXTENSIONS: &[&str] = &[
    "rs", "md", "toml", "yml", "yaml", "json", "py", "sh", "ps1", "txt",
];

/// This file: its tests hold stale commands on purpose.
const SELF_PATH: &str = "xtask/src/test_layout.rs";

/// How far back from a `--test` the `cargo` of its command may start, in bytes.
const COMMAND_WINDOW: usize = 400;

/// The packages under `root` (`bin/*`, `crates/*` and the separate `fuzz` workspace), sorted by name.
pub fn packages(root: &Path) -> Vec<Package> {
    let mut dirs = Vec::new();
    for parent in PACKAGE_PARENTS {
        if let Ok(entries) = std::fs::read_dir(root.join(parent)) {
            dirs.extend(entries.flatten().map(|e| e.path()));
        }
    }
    dirs.push(root.join("fuzz"));
    let mut out = Vec::new();
    for dir in dirs {
        let Ok(manifest) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let Ok(manifest) = manifest.parse::<toml::Table>() else {
            continue;
        };
        let Some(name) = manifest
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        else {
            continue;
        };
        let tests = dir.join("tests");
        let mut binaries: Vec<String> = manifest
            .get("test")
            .and_then(|t| t.as_array())
            .into_iter()
            .flatten()
            .filter_map(|t| t.get("name")?.as_str().map(str::to_owned))
            .collect();
        for e in std::fs::read_dir(&tests).into_iter().flatten().flatten() {
            let path = e.path();
            let file = e.file_name().to_string_lossy().into_owned();
            if path.is_file() {
                if let Some(stem) = file.strip_suffix(".rs") {
                    binaries.push(stem.to_owned());
                }
            } else if path.join("main.rs").is_file() {
                binaries.push(file);
            }
        }
        binaries.sort();
        binaries.dedup();
        let mut it_modules: Vec<String> = std::fs::read_dir(tests.join("it"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".rs").map(str::to_owned))
            .filter(|m| m != "main")
            .collect();
        it_modules.sort();
        out.push(Package {
            name: name.to_owned(),
            binaries,
            it_modules,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Checks the `--test` commands of every text file of the repository at `root`.
pub fn check_commands_in_repo(root: &Path) -> Result<Vec<String>> {
    let packages = packages(root);
    let mut v = Vec::new();
    for path in crate::repo::list_files(root)? {
        let checked = path
            .rsplit_once('.')
            .is_some_and(|(_, ext)| COMMAND_FILE_EXTENSIONS.contains(&ext));
        if !checked || path == SELF_PATH {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&path)) else {
            continue;
        };
        v.extend(check_commands(&path, &text, &packages));
    }
    Ok(v)
}

/// Checks the `--test <name>` commands in `text` (the file `path`) against `packages`.
///
/// The command's package is the `-p <name>` (or `--package <name>`) between the nearest `cargo` before the
/// `--test` (within the same paragraph) and the `--test`. A package name that is no package (a placeholder such
/// as `<crate>`) is not judged; a command without one passes when any package has the binary.
pub fn check_commands(path: &str, text: &str, packages: &[Package]) -> Vec<String> {
    let mut v = Vec::new();
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(found) = text[from..].find("--test") {
        let at = from + found;
        from = at + "--test".len();
        let before_ok =
            at == 0 || !matches!(bytes[at - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_');
        let Some(&next) = bytes.get(from) else {
            break;
        };
        if !before_ok || !matches!(next, b' ' | b'\t' | b'=' | b'\r' | b'\n') {
            continue;
        }
        let Some(name) = binary_name(&text[from..]) else {
            continue;
        };
        let package = command_package(&text[..at]);
        let line = text[..at].matches('\n').count() + 1;
        let message = match package {
            Some(p) => match packages.iter().find(|k| k.name == p) {
                None => None,
                Some(k) if k.binaries.iter().any(|b| b == name) => None,
                Some(k) => Some(stale_message(path, line, name, Some(k))),
            },
            None if packages.iter().any(|k| k.binaries.iter().any(|b| b == name)) => None,
            None => Some(stale_message(
                path,
                line,
                name,
                packages.iter().find(|k| k.it_modules.iter().any(|m| m == name)),
            )),
        };
        v.extend(message);
    }
    v
}

/// The binary name after a `--test`: past `=` or white space (and a comment leader where the command wraps).
fn binary_name(rest: &str) -> Option<&str> {
    let mut rest = rest.strip_prefix('=').unwrap_or(rest);
    let trimmed = rest.trim_start_matches([' ', '\t']);
    if trimmed.starts_with(['\r', '\n']) {
        rest = trimmed
            .trim_start_matches(['\r', '\n'])
            .trim_start_matches([' ', '\t']);
        for leader in ["//!", "///", "//", "#", ">"] {
            if let Some(r) = rest.strip_prefix(leader) {
                rest = r.trim_start_matches([' ', '\t']);
                break;
            }
        }
    } else {
        rest = trimmed;
    }
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty() && !name.starts_with('-')).then_some(name)
}

/// The `-p` package of the command that ends at the end of `before`, if it names one.
fn command_package(before: &str) -> Option<&str> {
    let mut start = before.len().saturating_sub(COMMAND_WINDOW);
    while !before.is_char_boundary(start) {
        start += 1;
    }
    let mut window = &before[start..];
    if let Some(i) = window.rfind("\n\n").or_else(|| window.rfind("\r\n\r\n")) {
        window = &window[i..];
    }
    let command = &window[window.rfind("cargo")?..];
    let tokens: Vec<&str> = command
        .split_whitespace()
        .map(|t| t.trim_matches(|c| matches!(c, '`' | '"' | '\'' | '\\')))
        .collect();
    let mut package = None;
    for (i, token) in tokens.iter().enumerate() {
        if *token == "-p" || *token == "--package" {
            package = tokens.get(i + 1).copied();
        } else if let Some(p) = token.strip_prefix("--package=") {
            package = Some(p);
        }
    }
    package
}

/// The violation for a `--test` that names no test binary; `package` is the command's package, or (without
/// `-p`) the package whose `it` binary has a module of that name.
fn stale_message(path: &str, line: usize, name: &str, package: Option<&Package>) -> String {
    match package {
        Some(k) if k.it_modules.iter().any(|m| m == name) => format!(
            "{path}:{line}: `--test {name}` names no test binary of {pkg}: `{name}` is a module of its test binary \
             `it`; write `--test it -E 'test(/^{name}::/)'` (cargo nextest) or `--test it {name}::` (cargo test)",
            pkg = k.name
        ),
        Some(k) => format!(
            "{path}:{line}: `--test {name}` names no test binary of {pkg}; name one of its test binaries: {list}",
            pkg = k.name,
            list = k.binaries.join(", ")
        ),
        None => format!(
            "{path}:{line}: `--test {name}` names no test binary of any package; name an existing one"
        ),
    }
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
        assert_eq!(check_commands_in_repo(root).unwrap(), Vec::<String>::new());
    }

    /// Two packages: one with an `it` binary and a separate one, one with a `[[test]]` entry only.
    fn fixture_packages() -> Vec<Package> {
        vec![
            Package {
                name: "ew-api".to_owned(),
                binaries: vec!["acceptance".to_owned(), "it".to_owned()],
                it_modules: vec!["golden_c".to_owned(), "golden_fixture".to_owned()],
            },
            Package {
                name: "ew-viewmodel".to_owned(),
                binaries: vec!["it".to_owned(), "jobs".to_owned()],
                it_modules: vec!["wiring".to_owned()],
            },
        ]
    }

    #[test]
    fn a_test_command_naming_a_module_of_it_fails_with_the_command_to_write() {
        let text = "intro\nrun `EW_BLESS_GOLDEN_FIXTURE=1 cargo nextest run -p ew-api --test golden_fixture --locked`\n";
        assert_eq!(
            check_commands("golden/README.md", text, &fixture_packages()),
            [
                "golden/README.md:2: `--test golden_fixture` names no test binary of ew-api: `golden_fixture` is a \
                 module of its test binary `it`; write `--test it -E 'test(/^golden_fixture::/)'` (cargo nextest) \
                 or `--test it golden_fixture::` (cargo test)"
            ]
        );
    }

    #[test]
    fn a_wrapped_command_and_an_unknown_binary_fail() {
        let text = "//! `cargo nextest run -p ew-api --test\n//! golden_c` runs it.\n\
                    cargo test -p ew-viewmodel --test=keymaps\n";
        assert_eq!(
            check_commands("a.rs", text, &fixture_packages()),
            [
                "a.rs:1: `--test golden_c` names no test binary of ew-api: `golden_c` is a module of its test \
                 binary `it`; write `--test it -E 'test(/^golden_c::/)'` (cargo nextest) or `--test it golden_c::` \
                 (cargo test)",
                "a.rs:3: `--test keymaps` names no test binary of ew-viewmodel; name one of its test binaries: it, \
                 jobs",
            ]
        );
    }

    #[test]
    fn a_command_without_a_package_is_judged_against_every_package() {
        let text = "cargo test --release --test wiring\n\ncargo test --test nowhere\n";
        assert_eq!(
            check_commands("b.md", text, &fixture_packages()),
            [
                "b.md:1: `--test wiring` names no test binary of ew-viewmodel: `wiring` is a module of its test \
                 binary `it`; write `--test it -E 'test(/^wiring::/)'` (cargo nextest) or `--test it wiring::` \
                 (cargo test)",
                "b.md:3: `--test nowhere` names no test binary of any package; name an existing one",
            ]
        );
    }

    #[test]
    fn valid_commands_placeholders_and_other_flags_pass() {
        let text = "cargo nextest run -p ew-api --test it -E 'test(/^golden_c::/)'\n\
                    cargo nextest run -p ew-api --test acceptance\n\
                    cargo test -p ew-viewmodel --test jobs\n\
                    cargo nextest run --test jobs\n\
                    cargo nextest run -p <crate> --test anything\n\
                    cargo test -p ew-api --test <name>\n\
                    cargo test -p ew-api --tests -- --test-threads 1\n\
                    .args([\"--test\", \"golden_c\"])\n";
        assert_eq!(
            check_commands("c.md", text, &fixture_packages()),
            Vec::<String>::new()
        );
    }
}
