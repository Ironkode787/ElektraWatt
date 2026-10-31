// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Repository policy checks for ElektraWatt and the local mirror of the per-PR CI set.
//!
//! Run through the cargo alias in `.cargo/config.toml`: `cargo xtask <command>`. Each check reads its policy
//! from a data file under `ci/`, so the code stays generic and can be copied to another project.

#![forbid(unsafe_code)]
// Build tooling, not a product path: the perf gate dates its report and reads its runner and assert switches
// from the environment, `CARGO` names the cargo binary, and tests name scratch directories by the clock.
#![allow(clippy::disallowed_methods)]

mod affected;
mod anonymise;
mod blind;
mod build_scripts;
mod dco;
mod deps;
mod docs_gen;
mod features;
mod governance;
mod icons;
mod licence;
mod ordered;
mod perf;
mod plan_counts;
mod raw_read;
mod repo;
mod symbols;
mod test_layout;
mod vet_imports;
mod workflows;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use anyhow::{Context, Result, bail};

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  check-deps           crate-dependency rules of ARCH §3 (policy: ci/dep-rules.toml)
  check-build-scripts [--bless]
                       every package with a build script is allowlisted at its version
                       (ci/build-scripts.allowlist, one name@version per line); --bless (or --write) re-versions
                       the entries of crates whose version changed and drops stale ones (it never adds a crate),
                       prints what changed and then checks
  check-vet-imports [--bless]
                       supply-chain/imports.lock carries no auditor identity (no who, notes or user-name key, no
                       name in a user-id comment, no e-mail address); --bless (or --write) scrubs the file, runs
                       `cargo vet fmt` and then checks
  check-features       banned features and crates only on dev edges (ci/dep-rules.toml: banned_features)
  check-raw-read [--bless]
                       no raw read of ew-model outside its readers (ci/dep-rules.toml: raw_read): the generated
                       clippy.toml and the lexical rules; --bless (or --write) regenerates clippy.toml first
  check-ordered-collections [--bless]
                       the allow sites of the HashMap/HashSet ban (clippy `disallowed-types`, FT-271): prints
                       them per crate and compares them with ci/ordered-collections-allow.toml (crate and item
                       path, no line); --bless (or --write) rewrites the list
  check-disallowed-methods [--bless]
                       the allow sites of the wall-clock, environment and randomness ban (clippy
                       `disallowed-methods`, FT-317), and the calls in the reader crates, which opt out of the
                       lint: prints them per crate and compares them with ci/disallowed-methods-allow.toml;
                       --bless (or --write) rewrites the list
  check-test-layout    each package's integration tests are modules of one test binary (tests/it/main.rs); a
                       top-level file under tests/ is only for a separate binary that a workflow, xtask or
                       nextest names (the list in xtask/src/test_layout.rs); every `--test <name>` in a text file
                       names a test binary of its `-p` package (of any package without `-p`)
  check-workflows      actions pinned by commit SHA, read-only default token, no pull_request_target,
                       credential-free checkouts, secrets only in `release` (.github/workflows/)
  check-licence-map    path licences, collective notice, crate licences, permissive-layer licence boundary
                       (ci/licence-map.toml, REUSE.toml)
  check-anonymisation [--require-entries]
                       no committed word n-gram is on the confidential-token list (ci/denylist.sha256);
                       --require-entries fails when the list has no digests
  check-governance-files
                       the governance files of NFR-DOC-08 exist and are not empty
  check-dco <range>    every non-merge commit in the git range has a Signed-off-by trailer
  denylist-hash <phrase>
                       print the digests of the phrase's 1-, 2- and 3-grams for ci/denylist.sha256
  symbols-qet10 [--check]
                       seal the native library documents and generate the QET edition (content/symbols/qet10);
                       --check changes nothing and fails when the committed files differ
  docs-gen [--check]   the generated reference pages of the user documentation book (docs/book/src/reference/):
                       one page per command (crates/ew-commands/schemas/) and per rule pack
                       (crates/ew-checks/catalogue/rules.json); --check changes nothing and fails when a
                       committed page differs; both modes fail when a WP count stated by hand in
                       docs/plan/01-global-plan.md or docs/plan/03-*.md differs from docs/plan/work-packages.csv
  gen-icons [--check]  the icon table of ew-ui (crates/ew-ui/src/icons/generated.rs) from the vendored Tabler
                       Icons (assets/icons/tabler/) and the project's glyphs (assets/icons/ew/); --check changes
                       nothing and fails when the committed table differs
  acceptance [--milestone] <report.md>
                       the acceptance runner of gate M1 over the golden project (WP-S6.01): writes the
                       Markdown report and a JUnit file (target/nextest/ci/junit.xml) and exits non-zero when an
                       entry fails or is not built (crates/ew-api/tests/acceptance.rs); --milestone runs the
                       release profile with EW_PERF_ASSERT=1 so that AC-17 is measured
  affected [--base <rev>]
                       the workspace crates changed since the merge base of <rev> (default main) and HEAD,
                       uncommitted and untracked files included, with their reverse dependencies (cargo
                       metadata), and the targeted `cargo nextest run -p ...` (with the features the crates'
                       tests require) and `cargo clippy -p ...` commands for them; for between-wave and
                       fix-round verifies only, a gate runs the full suite
  perf-gate [--classes S,M] [--quick] [--report <file>] [--baseline <file>] [--from <bench-output>]...
            [--bless] [--enforce] [--dry-run]
                       the performance and memory gate of WP-S6.03 (nightly, .github/workflows/perf.yml): runs
                       the ew-testkit benches over the fixture classes (--classes, default S,M) and the ew-ui
                       canvas bench once per gate run (class GP, report-only), compares the
                       medians and the memory estimates with ci/perf-baseline.toml (15 % threshold, 16 MiB
                       memory floor), reads the peak RSS (Linux) and writes a Markdown report; it fails on a
                       regression or a baseline value missing from the run only with EW_PERF_ASSERT set or
                       --enforce (the hosted runners are report-only), and always when a bench does not run (the
                       report then has a 'Bench failures' section); --bless writes the baseline; --from reads
                       one captured bench output instead of running a bench (one file per flag, repeat it)
  blind <subcommand>   the blind-drafter harness (WP-S6.13): an agent that never sees pixels drives the headless
                       mode through registered UI actions; the run folder lives outside the repository.
                       help | start | do | look | note | handback | stop | report | resume | compose | bless |
                       replay | selftest | bless-oracle | check-task (`cargo xtask blind help` lists them with their
                       arguments)
  ci                   fmt, clippy and nextest (default and all features), doctests and the repository checks
                       above (except check-dco), with a summary; the default-features nextest step is labelled
                       'nextest (no acceptance, no fuzzer)' (the headless acceptance tests and the command fuzzer
                       run in nextest-all-features)
  help                 this text
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");
    let result = match command {
        "check-deps" => run_check("check-deps", check_deps),
        "check-build-scripts" => match args.get(1).map(String::as_str) {
            None => run_check("check-build-scripts", check_build_scripts),
            Some("--bless" | "--write") => build_scripts::run_bless(&workspace_root()),
            Some(other) => {
                eprint!(
                    "check-build-scripts: unknown argument `{other}`

{USAGE}"
                );
                Ok(false)
            }
        },
        "check-vet-imports" => match args.get(1).map(String::as_str) {
            None => run_check("check-vet-imports", check_vet_imports),
            Some("--bless" | "--write") => vet_imports::run(&workspace_root()),
            Some(other) => {
                eprint!(
                    "check-vet-imports: unknown argument `{other}`

{USAGE}"
                );
                Ok(false)
            }
        },
        "check-features" => run_check("check-features", check_features),
        "check-raw-read" => match args.get(1).map(String::as_str) {
            None => raw_read::run(&workspace_root(), false),
            Some("--bless" | "--write") => raw_read::run(&workspace_root(), true),
            Some(other) => {
                eprint!(
                    "check-raw-read: unknown argument `{other}`

{USAGE}"
                );
                Ok(false)
            }
        },
        "check-ordered-collections" => match args.get(1).map(String::as_str) {
            None => ordered::run(&workspace_root(), &ordered::TYPES, false),
            Some("--bless" | "--write") => ordered::run(&workspace_root(), &ordered::TYPES, true),
            Some(other) => {
                eprint!(
                    "check-ordered-collections: unknown argument `{other}`

{USAGE}"
                );
                Ok(false)
            }
        },
        "check-disallowed-methods" => match args.get(1).map(String::as_str) {
            None => ordered::run(&workspace_root(), &ordered::METHODS, false),
            Some("--bless" | "--write") => ordered::run(&workspace_root(), &ordered::METHODS, true),
            Some(other) => {
                eprint!(
                    "check-disallowed-methods: unknown argument `{other}`

{USAGE}"
                );
                Ok(false)
            }
        },
        "check-test-layout" => run_check("check-test-layout", check_test_layout),
        "check-workflows" => run_check("check-workflows", check_workflows),
        "check-licence-map" => run_check("check-licence-map", check_licence_map),
        "check-anonymisation" => match args.get(1).map(String::as_str) {
            None => run_check("check-anonymisation", check_anonymisation),
            Some("--require-entries") => run_check("check-anonymisation", check_anonymisation_strict),
            Some(other) => {
                eprint!("check-anonymisation: unknown argument `{other}`\n\n{USAGE}");
                Ok(false)
            }
        },
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
        "symbols-qet10" => match args.get(1).map(String::as_str) {
            None => symbols::run(&workspace_root(), false),
            Some("--check") => symbols::run(&workspace_root(), true),
            Some(other) => {
                eprint!("symbols-qet10: unknown argument `{other}`\n\n{USAGE}");
                Ok(false)
            }
        },
        "docs-gen" => match args.get(1).map(String::as_str) {
            None => docs_gen::run(&workspace_root(), false),
            Some("--check") => docs_gen::run(&workspace_root(), true),
            Some(other) => {
                eprint!(
                    "docs-gen: unknown argument `{other}`

{USAGE}"
                );
                Ok(false)
            }
        },
        "gen-icons" => match args.get(1).map(String::as_str) {
            None => icons::run(&workspace_root(), false),
            Some("--check") => icons::run(&workspace_root(), true),
            Some(other) => {
                eprint!(
                    "gen-icons: unknown argument `{other}`

{USAGE}"
                );
                Ok(false)
            }
        },
        "acceptance" => match &args[1..] {
            [report] => acceptance(report, false),
            [flag, report] if flag == "--milestone" => acceptance(report, true),
            _ => {
                eprint!("acceptance needs the report file, optionally after --milestone\n\n{USAGE}");
                Ok(false)
            }
        },
        "affected" => affected::run(&args[1..], &workspace_root()),
        "perf-gate" => perf::run(&args[1..]),
        "blind" => blind::run(&args[1..], &workspace_root()),
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
    // All features: a build script behind a non-default feature is still part of the supply chain.
    let with_build_script = build_scripts::packages_with_build_script(&metadata_all_features(&root)?);
    Ok(build_scripts::check(&allowlist, &with_build_script))
}

fn check_vet_imports() -> Result<Violations> {
    vet_imports::violations(&workspace_root())
}

fn check_features() -> Result<Violations> {
    let root = workspace_root();
    let policy = deps::Policy::load(&root.join("ci/dep-rules.toml"))?;
    let rules = features::Rules {
        features: &policy.banned_features,
        crates: &policy.banned_crates,
    };
    let metadata = metadata()?;
    let mut violations = features::check(&rules, &features::from_metadata(&metadata));
    violations.extend(features::check_required(
        &policy.required_features,
        &features::resolved_from_metadata(&metadata),
    ));
    Ok(violations)
}

fn check_raw_read() -> Result<Violations> {
    raw_read::violations(&workspace_root())
}

fn check_ordered_collections() -> Result<Violations> {
    ordered::violations(&workspace_root(), &ordered::TYPES)
}

fn check_disallowed_methods() -> Result<Violations> {
    ordered::violations(&workspace_root(), &ordered::METHODS)
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
    check_anonymisation_at(&workspace_root(), false)
}

/// `check-anonymisation --require-entries`: the empty-list warning becomes a failure.
fn check_anonymisation_strict() -> Result<Violations> {
    check_anonymisation_at(&workspace_root(), true)
}

/// `check-anonymisation` for the repository at `root` (NFR-PRIV-09). Binary files (a NUL byte in the first
/// 8 KiB) are skipped; every other file is decoded by [`anonymise::decode`]. An empty denylist prints a
/// warning and passes (the entries are an owner action), unless `require_entries` is set.
pub(crate) fn check_anonymisation_at(root: &Path, require_entries: bool) -> Result<Violations> {
    let list = root.join("ci/denylist.sha256");
    let text = std::fs::read_to_string(&list).with_context(|| format!("reading {}", list.display()))?;
    let denylist = anonymise::parse_denylist(&text).with_context(|| format!("parsing {}", list.display()))?;
    if denylist.is_empty() {
        if require_entries {
            return Ok(vec![
                "ci/denylist.sha256 has no entries; the guard is inactive (--require-entries, NFR-PRIV-09)"
                    .to_owned(),
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

/// `check-test-layout`: no loose integration-test file outside the one `tests/it` binary of its package.
fn check_test_layout() -> Result<Violations> {
    let root = workspace_root();
    let mut violations = test_layout::check(&root);
    violations.extend(test_layout::check_commands_in_repo(&root)?);
    Ok(violations)
}

fn check_governance_files() -> Result<Violations> {
    Ok(governance::check(&workspace_root()))
}

/// `docs-gen --check` as an in-process check: the generated reference pages of the book are current.
fn check_docs_gen() -> Result<Violations> {
    docs_gen::violations(&workspace_root())
}

/// `gen-icons --check` as an in-process check: the generated icon table of ew-ui is current.
fn check_gen_icons() -> Result<Violations> {
    icons::violations(&workspace_root())
}

/// `acceptance [--milestone] <report.md>`: runs the `#[ignore]`d test `gate` of
/// `crates/ew-api/tests/acceptance.rs` with nextest and its `ci` profile (which writes the JUnit file of doc 18
/// §3.10 step 5). The test evaluates the M1 acceptance criteria over the golden project, writes the Markdown
/// report to `report` and fails when an entry fails or is not built. A stale report is deleted first, so a
/// report that exists afterwards is this run's; a build failure leaves none. With `milestone` the run uses the
/// release profile and sets `EW_PERF_ASSERT=1`, which turns AC-17 into measured entries. The test binary runs
/// from the workspace root, so a relative `report` is made absolute against the caller's directory first.
fn acceptance(report: &str, milestone: bool) -> Result<bool> {
    let report = std::path::absolute(report).with_context(|| format!("resolving {report}"))?;
    match std::fs::remove_file(&report) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("deleting the stale report {}", report.display())),
    }
    let mut command = Command::new(cargo());
    command
        .current_dir(workspace_root())
        .env("EW_ACCEPTANCE_REPORT", &report)
        .args([
            "nextest",
            "run",
            "--locked",
            "--profile",
            "ci",
            "-p",
            "ew-api",
            "--test",
            "acceptance",
        ])
        .args(["--run-ignored", "only", "-E", "test(=gate)"]);
    if milestone {
        command
            .env("EW_PERF_ASSERT", "1")
            .args(["--cargo-profile", "release"]);
    }
    let status = command.status().context("running `cargo nextest`")?;
    if report.exists() {
        println!("acceptance: report written to {}", report.display());
        let junit = workspace_root().join("target/nextest/ci/junit.xml");
        if junit.exists() {
            println!("acceptance: JUnit written to {}", junit.display());
        }
    } else {
        eprintln!("acceptance: no report was written (the build or the run failed before the gate wrote it)");
    }
    Ok(status.success())
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

/// What clippy prints for a `disallowed-methods` or `disallowed-types` path that names nothing: only a warning, which
/// `-D warnings` does not turn into an error.
const UNRESOLVED_CLIPPY_PATH: &str = "does not refer to a reachable";

/// The lines of clippy's stderr that report an unresolvable `disallowed-methods` path (the generated
/// `clippy.toml` of `check-raw-read` must never contain one).
fn unresolved_clippy_paths(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| line.contains(UNRESOLVED_CLIPPY_PATH))
        .map(str::to_owned)
        .collect()
}

/// Runs `command` with its stderr captured line by line and echoed to this process's stderr as it arrives.
fn run_echoing_stderr(command: &mut Command) -> Result<(std::process::ExitStatus, String)> {
    let mut child = command.stderr(Stdio::piped()).spawn()?;
    let mut captured = String::new();
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines() {
            let line = line?;
            eprintln!("{line}");
            captured.push_str(&line);
            captured.push('\n');
        }
    }
    Ok((child.wait()?, captured))
}

/// The nextest filterset of the first (default-features) run of a CI test job: the two slow test binaries run in
/// the all-features run only, so once per job. They are the headless acceptance tests of
/// `crates/ew-api/tests/acceptance.rs` (about 2 min; FT-268, decision log ID-381) and the command fuzzer of
/// `crates/ew-testkit/tests/command_fuzz` (`fuzz_smoke`, 15 to 19 s in a debug build; FT-306, ID-437).
/// `.config/nextest.toml` documents the rule and `.github/workflows/ci.yml` repeats the filter; a test below keeps
/// the three equal.
const ONCE_PER_JOB_FILTER: &str =
    "not (package(ew-api) and binary(acceptance)) and not (package(ew-testkit) and binary(command_fuzz))";

/// Runs the local mirror of the per-PR CI set and prints a summary. Every step runs, even after a failure,
/// so one invocation shows everything that needs fixing.
fn ci() -> Result<bool> {
    let cargo_steps: [(&str, &[&str]); 10] = [
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
        (
            "clippy-all-features",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        // The cargo-fuzz crate (FT-297) is its own workspace and is built here on the stable toolchain, as the `lint`
        // job does (`cargo` takes the toolchain from the working directory, not from fuzz/rust-toolchain.toml).
        (
            "fuzz-fmt",
            &["fmt", "--manifest-path", "fuzz/Cargo.toml", "--check"],
        ),
        (
            "fuzz-check",
            &[
                "check",
                "--manifest-path",
                "fuzz/Cargo.toml",
                "--all-targets",
                "--locked",
            ],
        ),
        (
            "fuzz-clippy",
            &[
                "clippy",
                "--manifest-path",
                "fuzz/Cargo.toml",
                "--all-targets",
                "--locked",
                "--",
                "-D",
                "warnings",
            ],
        ),
        // The headless acceptance tests and the command fuzzer run once, in the all-features run below (FT-268,
        // FT-306).
        (
            "nextest (no acceptance, no fuzzer)",
            &[
                "nextest",
                "run",
                "--workspace",
                "--locked",
                "-E",
                ONCE_PER_JOB_FILTER,
            ],
        ),
        (
            "nextest-all-features",
            &["nextest", "run", "--workspace", "--all-features", "--locked"],
        ),
        // nextest does not run doctests.
        ("doctests", &["test", "--doc", "--workspace", "--locked"]),
        (
            "doctests-all-features",
            &["test", "--doc", "--workspace", "--all-features", "--locked"],
        ),
    ];
    let checks: [Check; 14] = [
        ("check-deps", check_deps),
        ("check-features", check_features),
        ("check-raw-read", check_raw_read),
        ("check-ordered-collections", check_ordered_collections),
        ("check-disallowed-methods", check_disallowed_methods),
        ("check-build-scripts", check_build_scripts),
        ("check-vet-imports", check_vet_imports),
        ("check-workflows", check_workflows),
        ("check-licence-map", check_licence_map),
        ("check-anonymisation", check_anonymisation),
        ("check-governance-files", check_governance_files),
        ("docs-gen-check", check_docs_gen),
        ("gen-icons-check", check_gen_icons),
        ("check-test-layout", check_test_layout),
    ];

    let mut summary: Vec<(&str, bool)> = Vec::new();
    for (name, args) in cargo_steps {
        println!("==> cargo {}", args.join(" "));
        let mut command = Command::new(cargo());
        command.args(args).current_dir(workspace_root());
        let passed = if args[0] == "clippy" {
            let (status, stderr) = run_echoing_stderr(&mut command)
                .with_context(|| format!("running `cargo {}`", args.join(" ")))?;
            let unresolved = unresolved_clippy_paths(&stderr);
            for line in &unresolved {
                eprintln!("{name}: unresolvable disallowed path (ADR-0001 amendment 2): {line}");
            }
            status.success() && unresolved.is_empty()
        } else {
            command
                .status()
                .with_context(|| format!("starting `cargo {}`", args.join(" ")))?
                .success()
        };
        summary.push((name, passed));
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
        println!("  {:<26} {}", name, if *passed { "ok" } else { "FAILED" });
    }
    let failed = summary.iter().filter(|(_, passed)| !passed).count();
    if failed > 0 {
        bail!("{failed} of {} steps failed", summary.len());
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{ONCE_PER_JOB_FILTER, unresolved_clippy_paths, workspace_root};

    /// The heavy test binaries of the all-features run, balanced by measured time (FT-387): `test-windows-1` runs
    /// exactly `WINDOWS_PART1_BINARIES`, `test-windows-2` exactly `WINDOWS_PART2_BINARIES`, and `test-windows-3` the
    /// complement of both, so the acceptance tests and the command fuzzer run in part 1 and nowhere else. `ci/README.md`
    /// (CI-PR-03) records the durations the lists were chosen from; a binary that moves between the lists is a change
    /// to that table and to `ci.yml`.
    const WINDOWS_PART1_BINARIES: [&str; 8] = [
        "elektrawatt::canvas",
        "ew-api::acceptance",
        "ew-derive::incremental_partition",
        "ew-import-qet::hostile_input",
        "ew-store::incremental",
        "ew-store::purge",
        "ew-testkit::command_fuzz",
        "ew::cli",
    ];
    const WINDOWS_PART2_BINARIES: [&str; 10] = [
        "elektrawatt::headless",
        "elektrawatt::part2",
        "ew-api::checks",
        "ew-api::memo_oracle",
        "ew-api::page_output",
        "ew-store::fault_injection",
        "ew-testkit::golden_dsl",
        "ew-testkit::spike_memo",
        "ew-ui::canvas",
        "ew-viewmodel::part2",
    ];

    /// FT-268, FT-306, FT-330, FT-353, FT-387: the acceptance tests and the command fuzzer run once per job. The filtered
    /// default-features run is in test-linux-default; the unfiltered all-features run is in test-linux (the macOS jobs
    /// were removed by ID-880); test-windows-1, test-windows-2 and test-windows-3 run the all-features archive in three
    /// disjoint parts that together cover every test: part 1 is `WINDOWS_PART1_BINARIES` (which contains the
    /// acceptance tests and the fuzzer), part 2 is `WINDOWS_PART2_BINARIES`, part 3 is the complement of both.
    /// The nextest config documents the same filter.
    #[test]
    fn the_acceptance_tests_and_the_fuzzer_run_once_per_ci_job() {
        let ci = std::fs::read_to_string(workspace_root().join(".github/workflows/ci.yml")).unwrap();
        let nextest_runs: Vec<&str> = ci
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("run: cargo nextest run"))
            .collect();
        let (archive, direct): (Vec<&str>, Vec<&str>) =
            nextest_runs.iter().partition(|l| l.contains("--archive-file"));
        let (all_features, default): (Vec<&str>, Vec<&str>) =
            direct.iter().partition(|l| l.contains("--all-features"));
        assert_eq!(
            (default.len(), all_features.len(), archive.len()),
            (1, 1, 3),
            "the filtered default-features run in test-linux-default, the unfiltered all-features run in test-linux, the three partitions of the Windows archive: {nextest_runs:#?}"
        );
        for line in default {
            assert!(line.contains(&format!("-E '{ONCE_PER_JOB_FILTER}'")), "{line}");
        }
        for line in all_features {
            assert!(
                !line.contains(" -E "),
                "the all-features run runs everything: {line}"
            );
        }
        // FT-387: parts 1 and 2 are the two explicit lists, part 3 is the complement of both; together a complete and
        // disjoint cover, and the acceptance tests and the fuzzer run in part 1 only.
        for needle in ["ew-api::acceptance", "ew-testkit::command_fuzz"] {
            assert!(
                WINDOWS_PART1_BINARIES.contains(&needle),
                "{needle} belongs to part 1"
            );
        }
        for binary in WINDOWS_PART1_BINARIES {
            assert!(
                !WINDOWS_PART2_BINARIES.contains(&binary),
                "{binary} is in both lists"
            );
        }
        let filter = |binaries: &[&str]| {
            binaries
                .iter()
                .map(|binary| format!("binary_id(={binary})"))
                .collect::<Vec<_>>()
                .join(" or ")
        };
        let (one, two) = (filter(&WINDOWS_PART1_BINARIES), filter(&WINDOWS_PART2_BINARIES));
        let expected = [
            format!("-E '{one}' --profile ci"),
            format!("-E '{two}' --profile ci"),
            format!("-E 'not ({one} or {two})' --profile ci"),
        ];
        for (k, (line, filter)) in archive.iter().zip(&expected).enumerate() {
            assert!(
                line.contains("--workspace-remap .") && line.contains(filter.as_str()),
                "part {} of 3 runs the filter {filter}: {line}",
                k + 1
            );
            assert!(
                !line.contains("--partition"),
                "part {} of 3 is not count-partitioned: {line}",
                k + 1
            );
        }
        // Every name in the lists is a test binary that exists, so a rename cannot silently empty a part.
        for binary in WINDOWS_PART1_BINARIES.iter().chain(&WINDOWS_PART2_BINARIES) {
            let (krate, name) = binary.split_once("::").unwrap();
            let root = workspace_root();
            let found = [root.join("crates").join(krate), root.join("bin").join(krate)]
                .iter()
                .map(|package| package.join("tests"))
                .any(|tests| {
                    tests.join(format!("{name}.rs")).is_file() || tests.join(name).join("main.rs").is_file()
                });
            assert!(found, "{binary} is not a test binary of {krate}");
        }
        // FT-353, ID-880: the doctests run in test-linux (default features and all features), nowhere else.
        let doc_runs: Vec<&str> = ci
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with("run: cargo test --doc"))
            .collect();
        let doc_all_features = doc_runs.iter().filter(|l| l.contains("--all-features")).count();
        assert_eq!(
            (doc_runs.len() - doc_all_features, doc_all_features),
            (1, 1),
            "doctests: test-linux only, not test-windows (FT-353, ID-880): {doc_runs:#?}"
        );
        let config = std::fs::read_to_string(workspace_root().join(".config/nextest.toml")).unwrap();
        assert!(
            config.contains(&format!("-E '{ONCE_PER_JOB_FILTER}'")),
            "the nextest config documents the filter"
        );
    }

    /// FT-353: the Windows tests run from a nextest archive, unpacked elsewhere than the compile-time path of
    /// `env!("CARGO_BIN_EXE_<name>")`. A test starts a binary through its crate's `bin_exe()` helper (which reads
    /// `NEXTEST_BIN_EXE_<name>` first); the only allowed `env!("CARGO_BIN_EXE_` site is the helper's fallback line,
    /// marked `// bin-exe-fallback`.
    #[test]
    fn tests_start_binaries_through_the_nextest_aware_helper() {
        fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    rust_files(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push(path);
                }
            }
        }
        let root = workspace_root();
        let mut files = Vec::new();
        for group in ["bin", "crates"] {
            let Ok(entries) = std::fs::read_dir(root.join(group)) else {
                continue;
            };
            for entry in entries.flatten() {
                rust_files(&entry.path().join("tests"), &mut files);
            }
        }
        assert!(!files.is_empty(), "no test sources found");
        let mut bare = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file).unwrap();
            for (n, line) in text.lines().enumerate() {
                if line.contains("env!(\"CARGO_BIN_EXE_") && !line.contains("// bin-exe-fallback") {
                    bare.push(format!("{}:{}", file.display(), n + 1));
                }
            }
        }
        assert!(
            bare.is_empty(),
            "use the crate's bin_exe() helper (NEXTEST_BIN_EXE_<name> first, then the marked fallback): {bare:#?}"
        );
    }

    #[test]
    fn an_unresolvable_disallowed_path_is_caught() {
        let warning = "warning: `ew_model::Model::gone` does not refer to a reachable function";
        let stderr = format!(
            "    Checking app v0.0.0\n{warning}\n  |\n  = help: add `allow-invalid = true` to the entry\n    Finished `dev` profile\n"
        );
        assert_eq!(unresolved_clippy_paths(&stderr), [warning]);
    }

    #[test]
    fn clean_output_and_a_real_disallowed_use_are_not_caught() {
        assert!(unresolved_clippy_paths("").is_empty());
        let clean = "    Checking app v0.0.0\n    Finished `dev` profile [unoptimized] target(s) in 1.2s\n";
        assert!(unresolved_clippy_paths(clean).is_empty());
        let error = "error: use of a disallowed method `ew_model::Model::row`\n  --> src/lib.rs:3:7\n   = note: raw reader of ew-model\n";
        assert!(unresolved_clippy_paths(error).is_empty());
    }
}
