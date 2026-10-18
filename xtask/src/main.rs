// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Repository policy checks for ElektraWatt and the local mirror of the per-PR CI set.
//!
//! Run through the cargo alias in `.cargo/config.toml`: `cargo xtask <command>`. Each check reads its policy
//! from a data file under `ci/`, so the code stays generic and can be copied to another project.

#![forbid(unsafe_code)]

mod anonymise;
mod build_scripts;
mod dco;
mod deps;
mod governance;
mod licence;
mod repo;
mod workflows;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  check-deps           crate-dependency rules of ARCH §3 (policy: ci/dep-rules.toml)
  check-build-scripts  every package with a build script is allowlisted (ci/build-scripts.allowlist)
  check-workflows      actions pinned by commit SHA, read-only default token (.github/workflows/)
  check-licence-map    path licences, collective notice, crate licences, permissive-layer licence boundary
                       (ci/licence-map.toml, REUSE.toml)
  check-anonymisation  no committed word n-gram is on the confidential-token list (ci/denylist.sha256)
  check-governance-files
                       the governance files of NFR-DOC-08 exist and are not empty
  check-dco <range>    every non-merge commit in the git range has a Signed-off-by trailer
  denylist-hash <phrase>
                       print the digests of the phrase's 1-, 2- and 3-grams for ci/denylist.sha256
  ci                   fmt, clippy, nextest and the repository checks above (except check-dco), with a summary
  help                 this text
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let result = match command {
        "check-deps" => run_check("check-deps", check_deps),
        "check-build-scripts" => run_check("check-build-scripts", check_build_scripts),
        "check-workflows" => run_check("check-workflows", check_workflows),
        "check-licence-map" => run_check("check-licence-map", check_licence_map),
        "check-anonymisation" => run_check("check-anonymisation", check_anonymisation),
        "check-governance-files" => run_check("check-governance-files", check_governance_files),
        "check-dco" => match args.get(1) {
            Some(range) => check_dco(range),
            None => {
                eprint!("check-dco needs a git revision range, e.g. `origin/main..HEAD`\n\n{USAGE}");
                Ok(false)
            }
        },
        "denylist-hash" => match args.get(1) {
            Some(phrase) => {
                denylist_hash(phrase);
                Ok(true)
            }
            None => {
                eprint!("denylist-hash needs the phrase as one argument\n\n{USAGE}");
                Ok(false)
            }
        },
        "ci" => ci(),
        "help" | "-h" | "--help" => {
            print!("{USAGE}");
            Ok(true)
        }
        other => {
            eprint!("unknown command `{other}`\n\n{USAGE}");
            Ok(false)
        }
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

/// The workspace root: the parent of this crate's directory.
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the workspace root")
        .to_path_buf()
}

/// The cargo binary that invoked us (falls back to `cargo` on the PATH).
fn cargo() -> String {
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
/// optional dependency, as the licence boundary requires (ADR-0024 decision 2).
fn metadata_all_features(root: &Path) -> Result<cargo_metadata::Metadata> {
    cargo_metadata::MetadataCommand::new()
        .cargo_path(cargo())
        .manifest_path(root.join("Cargo.toml"))
        .features(cargo_metadata::CargoOpt::AllFeatures)
        .other_options(vec!["--locked".to_owned()])
        .exec()
        .context("running `cargo metadata --locked --all-features`")
}

/// The violations a check found; empty means the check passed.
type Violations = Vec<String>;

/// A named in-process policy check.
type Check = (&'static str, fn() -> Result<Violations>);

/// Prints the outcome of one check and returns whether it passed.
fn run_check(name: &str, check: fn() -> Result<Violations>) -> Result<bool> {
    let violations = check()?;
    if violations.is_empty() {
        println!("{name}: ok");
        Ok(true)
    } else {
        eprintln!("{name}: {} violation(s)", violations.len());
        for v in &violations {
            eprintln!("  - {v}");
        }
        Ok(false)
    }
}

fn check_deps() -> Result<Violations> {
    let root = workspace_root();
    let policy = deps::Policy::load(&root.join("ci/dep-rules.toml"))?;
    let graph = deps::Graph::from_metadata(&metadata()?);
    Ok(deps::check(&policy, &graph))
}

fn check_build_scripts() -> Result<Violations> {
    let root = workspace_root();
    let allowlist = build_scripts::load_allowlist(&root.join("ci/build-scripts.allowlist"))?;
    let with_build_script = build_scripts::packages_with_build_script(&metadata()?);
    Ok(build_scripts::check(&allowlist, &with_build_script))
}

fn check_workflows() -> Result<Violations> {
    workflows::check_dir(&workspace_root().join(".github/workflows"))
}

fn check_licence_map() -> Result<Violations> {
    check_licence_map_at(&workspace_root())
}

/// `check-licence-map` for the repository at `root` (19 §5.1).
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

fn check_anonymisation() -> Result<Violations> {
    check_anonymisation_at(&workspace_root())
}

/// `check-anonymisation` for the repository at `root` (NFR-PRIV-09). Files that are not valid UTF-8 are skipped.
pub(crate) fn check_anonymisation_at(root: &Path) -> Result<Violations> {
    let list = root.join("ci/denylist.sha256");
    let text = std::fs::read_to_string(&list).with_context(|| format!("reading {}", list.display()))?;
    let denylist = anonymise::parse_denylist(&text).with_context(|| format!("parsing {}", list.display()))?;
    let mut files = Vec::new();
    for path in repo::list_files(root)? {
        let bytes = std::fs::read(root.join(&path)).with_context(|| format!("reading {path}"))?;
        if let Ok(text) = String::from_utf8(bytes) {
            files.push((path, text));
        }
    }
    Ok(anonymise::check(&files, &denylist))
}

fn check_governance_files() -> Result<Violations> {
    Ok(governance::check(&workspace_root()))
}

/// `check-dco <range>`: prints the commits without a sign-off.
fn check_dco(range: &str) -> Result<bool> {
    let commits = dco::commits(&workspace_root(), range)?;
    let violations = dco::check(&commits);
    if violations.is_empty() {
        println!("check-dco: ok ({} commit(s) in {range})", commits.len());
        Ok(true)
    } else {
        eprintln!(
            "check-dco: {} of {} commit(s) in {range} without sign-off",
            violations.len(),
            commits.len()
        );
        for v in &violations {
            eprintln!("  - {v}");
        }
        Ok(false)
    }
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

/// Runs the local mirror of the per-PR CI set and prints a summary. Every step runs, even after a failure,
/// so one invocation shows everything that needs fixing.
fn ci() -> Result<bool> {
    let cargo_steps: [(&str, &[&str]); 3] = [
        ("fmt", &["fmt", "--all", "--check"]),
        (
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("nextest", &["nextest", "run", "--workspace", "--locked"]),
    ];
    let checks: [Check; 6] = [
        ("check-deps", check_deps),
        ("check-build-scripts", check_build_scripts),
        ("check-workflows", check_workflows),
        ("check-licence-map", check_licence_map),
        ("check-anonymisation", check_anonymisation),
        ("check-governance-files", check_governance_files),
    ];

    let mut summary: Vec<(&str, bool)> = Vec::new();
    for (name, args) in cargo_steps {
        println!("==> cargo {}", args.join(" "));
        let status = Command::new(cargo())
            .args(args)
            .current_dir(workspace_root())
            .status()
            .with_context(|| format!("starting `cargo {}`", args.join(" ")))?;
        summary.push((name, status.success()));
    }
    for (name, check) in checks {
        println!("==> {name}");
        let passed = match run_check(name, check) {
            Ok(passed) => passed,
            Err(err) => {
                eprintln!("{name}: error: {err:#}");
                false
            }
        };
        summary.push((name, passed));
    }

    println!("\nsummary:");
    for (name, passed) in &summary {
        println!("  {:<24} {}", name, if *passed { "ok" } else { "FAILED" });
    }
    let failed = summary.iter().filter(|(_, passed)| !passed).count();
    if failed > 0 {
        bail!("{failed} of {} steps failed", summary.len());
    }
    Ok(true)
}
