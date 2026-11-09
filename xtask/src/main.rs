// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The development commands of ElektraWatt: one command that checks a change, one that regenerates the generated
//! files, and the repository rules they share.
//!
//! Run through the cargo alias in `.cargo/config.toml`: `cargo xtask <command>`. The loop is described in
//! `CONTRIBUTING.md`. Each rule reads its policy from a data file under `ci/`.

#![forbid(unsafe_code)]
// Build tooling, not a product path: `CARGO` names the cargo binary, and tests name scratch directories by the clock.
#![allow(clippy::disallowed_methods)]

mod anonymise;
mod api_guard;
mod blind;
#[cfg(test)]
mod book_pot;
mod check;
mod deps;
mod docs_gen;
mod features;
mod generate;
mod icons;
mod licence;
mod raw_read;
mod repo;
mod shell_keys;
mod symbols;
mod ui_strings;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};

const USAGE: &str = "\
usage: cargo xtask <command>

the loop:
  check [-p <package>]... [--no-test]
                       checks a change: formats every Rust file, then clippy and the tests of the named packages
                       (the whole workspace without -p) with all features, then the repository rules. --no-test
                       leaves the tests out
  gen [--check]        regenerates every generated file: the text keys, the command, CLI and rule snapshots, the
                       keymaps, the book's contents, the icon table, the lint list and the QET symbol edition;
                       --check writes nothing and fails when a file xtask writes itself is stale (the snapshots
                       are compared by their tests)
  rules                the repository rules alone, in seconds: the crate layers and the licence boundary
                       (ci/dep-rules.toml, ci/licence-map.toml), test-only features, raw reads of the model,
                       confidential tokens (ci/denylist.sha256), UI texts outside the text keys, and `gen --check`

single steps:
  docs-gen [--check]   the generated reference pages of the user guide (docs/book/src/reference/)
  gen-icons [--check]  the icon table of ew-ui from assets/icons/
  symbols-qet10 [--check]
                       seals the library documents and generates the QET edition (content/symbols/qet10)
  api-guard [--base <rev>]
                       compares the published surfaces (command and result schemas, the CLI, queries, api_v1, the
                       file format) with a base revision and names each breaking change
  denylist-hash <phrase>
                       prints the digests of a confidential phrase for ci/denylist.sha256
  blind <subcommand>   the persona harness (`cargo xtask blind help`)
  help                 this text
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let root = workspace_root();
    let result = match command {
        "check" => check::run(&args[1..], &root),
        "gen" => match flag(&args[1..], "--check") {
            Some(check) => generate::run(&root, check),
            None => usage("gen takes only --check"),
        },
        "rules" => rules(&root),
        "docs-gen" => match flag(&args[1..], "--check") {
            Some(check) => docs_gen::run(&root, check),
            None => usage("docs-gen takes only --check"),
        },
        "gen-icons" => match flag(&args[1..], "--check") {
            Some(check) => icons::run(&root, check),
            None => usage("gen-icons takes only --check"),
        },
        "symbols-qet10" => match flag(&args[1..], "--check") {
            Some(check) => symbols::run(&root, check),
            None => usage("symbols-qet10 takes only --check"),
        },
        "api-guard" => api_guard::run(&args[1..], &root),
        "denylist-hash" => match args.get(1) {
            Some(phrase) => {
                denylist_hash(phrase);
                Ok(true)
            }
            None => usage("denylist-hash needs the phrase as one argument"),
        },
        "blind" => blind::run(&args[1..], &root),
        "help" | "-h" | "--help" => {
            print!("{USAGE}");
            Ok(true)
        }
        other => usage(&format!("unknown command `{other}`")),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// `Some(true)` for exactly `[name]`, `Some(false)` for no argument, `None` for anything else.
fn flag(args: &[String], name: &str) -> Option<bool> {
    match args {
        [] => Some(false),
        [one] if one == name => Some(true),
        _ => None,
    }
}

fn usage(problem: &str) -> Result<bool> {
    eprint!("{problem}\n\n{USAGE}");
    Ok(false)
}

/// The workspace root: the parent of this crate's directory.
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}

/// The cargo binary that invoked us (falls back to `cargo` on the PATH).
pub(crate) fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Runs `cargo metadata --locked` for the whole workspace.
pub(crate) fn metadata() -> Result<cargo_metadata::Metadata> {
    cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo())
        .manifest_path(workspace_root().join("Cargo.toml"))
        .other_options(vec!["--locked".to_owned()])
        .exec()
        .context("running `cargo metadata --locked`")
}

/// Runs `cargo metadata --locked --all-features` for the workspace at `root`: the resolved graph with every
/// optional dependency, as the licence boundary requires.
pub(crate) fn metadata_all_features(root: &Path) -> Result<cargo_metadata::Metadata> {
    cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo())
        .manifest_path(root.join("Cargo.toml"))
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .other_options(vec!["--locked".to_owned()])
        .exec()
        .context("running `cargo metadata --locked --all-features`")
}

/// The violations a rule found; empty means it passed.
type Violations = Vec<String>;

/// A repository rule: the violations it finds at a root.
type Rule = fn(&Path) -> Result<Violations>;

/// The repository rules, each with its name. They read files, git and `cargo metadata` only: nothing is built.
const RULES: &[(&str, Rule)] = &[
    ("layers", check_deps),
    ("features", check_features),
    ("raw-read", raw_read::violations),
    ("licences", check_licence_map_at),
    ("anonymisation", |root| check_anonymisation_at(root, false)),
    ("ui-strings", ui_strings::check),
];

/// `rules`: every repository rule, then `gen --check`. Prints one line per rule and each violation.
pub(crate) fn rules(root: &Path) -> Result<bool> {
    let mut ok = true;
    for (name, rule) in RULES {
        let violations = rule(root)?;
        if violations.is_empty() {
            println!("{name}: ok");
        } else {
            ok = false;
            eprintln!("{name}: {} violation(s)", violations.len());
            for v in &violations {
                eprintln!("  - {v}");
            }
        }
    }
    Ok(generate::check_own(root)? && ok)
}

/// The crate layers and dependency rules of `ci/dep-rules.toml`.
fn check_deps(root: &Path) -> Result<Violations> {
    let policy = deps::Policy::load(&root.join("ci/dep-rules.toml"))?;
    let graph = deps::Graph::from_metadata(&metadata()?);
    let mut violations = deps::check(&policy, &graph);
    violations.extend(deps::check_confined_manifests(&policy, root)?);
    violations.extend(deps::importer_purity(root)?);
    Ok(violations)
}

/// Test-only features and crates never reach a shipped binary (`ci/dep-rules.toml`: `banned_features`,
/// `banned_crates`, `uniform_test_features`, `required_features`).
fn check_features(root: &Path) -> Result<Violations> {
    let policy = deps::Policy::load(&root.join("ci/dep-rules.toml"))?;
    let rules = features::Rules {
        features: &policy.banned_features,
        crates: &policy.banned_crates,
    };
    let metadata = metadata()?;
    let packages = features::from_metadata(&metadata);
    let mut violations = features::check(&rules, &packages);
    violations.extend(features::check_uniform(&policy.uniform_test_features, &packages));
    violations.extend(features::check_required(
        &policy.required_features,
        &features::resolved_from_metadata(&metadata),
    ));
    // The shipped binaries: every workspace package with a binary target, the development tools aside.
    let shipped: Vec<&str> = metadata
        .workspace_packages()
        .iter()
        .filter(|p| !policy.layers.tools.iter().any(|t| *t == p.name.as_str()))
        .filter(|p| p.targets.iter().any(|t| t.is_bin()))
        .map(|p| p.name.as_str())
        .collect();
    let graph = features::shipping_graph(root, &shipped)?;
    violations.extend(features::check_shipping(&rules, &graph, &shipped.join(", ")));
    Ok(violations)
}

/// Path licences, the collective notice, crate licences and the permissive-layer licence boundary
/// (`ci/licence-map.toml`, `REUSE.toml`).
pub(crate) fn check_licence_map_at(root: &Path) -> Result<Violations> {
    let map = licence::LicenceMap::load(&root.join("ci/licence-map.toml"))?;
    let reuse = licence::ReuseToml::load(&root.join("REUSE.toml"))?;
    let mut files = Vec::new();
    for path in repo::list_files(root)? {
        let bytes = std::fs::read(root.join(&path)).with_context(|| format!("reading {path}"))?;
        let header = licence::read_header(&String::from_utf8_lossy(&bytes));
        files.push(licence::FileInfo { path, header });
    }

    let mut violations = licence::check_policy(&map);
    violations.extend(licence::check_files(&map, &reuse, &files));

    let metadata = metadata_all_features(root)?;
    let crates: Vec<licence::CrateInfo> = metadata
        .workspace_packages()
        .iter()
        .map(|p| licence::CrateInfo {
            name: p.name.to_string(),
            manifest: p
                .manifest_path
                .strip_prefix(&metadata.workspace_root)
                .map(|m| m.as_str().replace('\\', "/"))
                .unwrap_or_else(|_| p.manifest_path.to_string()),
            license: p.license.clone(),
        })
        .collect();
    violations.extend(licence::check_crates(&map, &crates));

    // One source of truth for the permissive layer: ci/dep-rules.toml.
    let layers = deps::Policy::load(&root.join("ci/dep-rules.toml"))?.layers;
    let graph = licence::DepGraph::from_metadata(&metadata)?;
    violations.extend(licence::check_boundary(
        &map.boundary.allow,
        &layers.permissive,
        &graph,
    ));
    Ok(violations)
}

/// No committed word n-gram is on the confidential-token list (`ci/denylist.sha256`). Binary files (a NUL byte in
/// the first 8 KiB) are skipped. An empty list prints a warning and passes, unless `require_entries` is set.
pub(crate) fn check_anonymisation_at(root: &Path, require_entries: bool) -> Result<Violations> {
    let list = root.join("ci/denylist.sha256");
    let text = std::fs::read_to_string(&list).with_context(|| format!("reading {}", list.display()))?;
    let denylist = anonymise::parse_denylist(&text).with_context(|| format!("parsing {}", list.display()))?;
    if denylist.is_empty() {
        if require_entries {
            return Ok(vec![
                "ci/denylist.sha256 has no entries; the guard is inactive".to_owned(),
            ]);
        }
        eprintln!("{}", anonymise::EMPTY_WARNING);
    }
    let mut files = Vec::new();
    for path in repo::list_files(root)? {
        let bytes = std::fs::read(root.join(&path)).with_context(|| format!("reading {path}"))?;
        if let Some(text) = anonymise::decode(&bytes) {
            files.push((path, text));
        }
    }
    Ok(anonymise::check(&files, &denylist))
}

/// `denylist-hash <phrase>`: prints the digests in the denylist format. Comments give only n and the word
/// position, never the words.
fn denylist_hash(phrase: &str) {
    println!(
        "# Keep the distinctive n-grams only: a common word on its own would block every file containing it."
    );
    for (n, first, hex) in anonymise::phrase_digests(phrase) {
        println!("# {n}-gram starting at word {first}");
        println!("{hex}");
    }
}
