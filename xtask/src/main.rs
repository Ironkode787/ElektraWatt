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

mod action_map;
mod anonymise;
mod api_guard;
mod blind;
mod book_po;
mod ceiling;
mod check;
mod code_map;
mod crate_map;
mod deps;
mod design_docs;
mod docs_gen;
mod features;
mod generate;
mod greyed;
mod icons;
mod licence;
mod raw_read;
mod repo;
mod symbols;
mod translations;
mod ui_strings;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};

/// A section of the help text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Loop,
    Reading,
    Steps,
}

impl Section {
    /// The section's heading.
    fn heading(self) -> &'static str {
        match self {
            Section::Loop => "the loop:",
            Section::Reading => "reading the code:",
            Section::Steps => "single steps:",
        }
    }
}

/// One command of `cargo xtask`: its name, its arguments as the help prints them, what it does, and how it runs. The
/// help text and the dispatch both read [`COMMANDS`], so a command is in the help as soon as it runs.
struct Command {
    name: &'static str,
    section: Section,
    args: &'static str,
    does: &'static str,
    run: fn(&[String], &Path) -> Result<bool>,
}

/// Every command, in the order of the help text.
const COMMANDS: &[Command] = &[
    Command {
        name: "check",
        section: Section::Loop,
        args: "[-p <package>]... [--no-test] [--deep] [-- <test filter>...]",
        does: "checks a change: formats every Rust file, then clippy and the tests of the named packages (the whole \
               workspace without -p) with all features, then the repository rules. --deep adds the tests marked for \
               the deep run; --no-test leaves the tests out; a filter after -- (nextest's: name substrings or -E \
               <expression>) narrows them and leaves the doctests out. A change of tools/blind/, content/ or xtask/ \
               replays the harness's briefs on the built programs; the whole workspace replays them whatever it \
               touches and runs docs-gen --check. A check that passes writes down its tree, and on the same tree \
               runs nothing",
        run: check::run,
    },
    Command {
        name: "gen",
        section: Section::Loop,
        args: "[--check]",
        does: "regenerates every generated file: the reference of the commands, rules, keymaps, CLI, queries and views \
               (target/reference, not committed), the book's contents and reference pages, the maps of the actions \
               and the crates (docs/map/), the icon table, the lint list and the QET symbol edition; --check writes \
               nothing and fails when a committed file that needs no build to compare is stale",
        run: |args, root| with_check(args, "gen", |check| generate::run(root, check)),
    },
    Command {
        name: "rules",
        section: Section::Loop,
        args: "",
        does: "the repository rules alone, in seconds (each is listed below), then `gen --check`; it builds nothing",
        run: |_, root| rules(root),
    },
    Command {
        name: "outline",
        section: Section::Reading,
        args: "[--tests] <path>...",
        does: "every item of a Rust file (or of each under a directory) with its line span, signature and first doc \
               sentence; a tests module as one line unless --tests",
        run: code_map::outline,
    },
    Command {
        name: "where",
        section: Section::Reading,
        args: "[--in <crate>] <name>...",
        does: "a name's definitions with their spans, its row of the action map, then every use as line numbers \
               grouped by file",
        run: code_map::find,
    },
    Command {
        name: "docs-gen",
        section: Section::Steps,
        args: "[--check]",
        does: "the reference (target/reference) and from it the generated reference pages of the user guide \
               (docs/book/src/reference/) and the lists of its SUMMARY.md; --check writes no page and fails when \
               SUMMARY.md or a rule's notes differ from the sources",
        run: |args, root| {
            with_check(args, "docs-gen", |check| {
                generate::reference(root).and_then(|ok| Ok(ok && docs_gen::run(root, check)?))
            })
        },
    },
    Command {
        name: "gen-maps",
        section: Section::Steps,
        args: "[--check]",
        does: "the action map and the crate map (docs/map/), read from the source in seconds",
        run: |args, root| with_check(args, "gen-maps", |check| generate::maps(root, check)),
    },
    Command {
        name: "gen-icons",
        section: Section::Steps,
        args: "[--check]",
        does: "the icon table of ew-ui from assets/icons/",
        run: |args, root| with_check(args, "gen-icons", |check| icons::run(root, check)),
    },
    Command {
        name: "book-po",
        section: Section::Steps,
        args: "[--check]",
        does: "the user guide's translation catalogue (docs/book/po/: the template, and each language merged with it) \
               from its pages; --check writes nothing and says whether it is stale",
        run: |args, root| with_check(args, "book-po", |check| book_po::run(root, check)),
    },
    Command {
        name: "symbols-qet10",
        section: Section::Steps,
        args: "[--check]",
        does: "seals the library documents and generates the QET edition (content/symbols/qet10)",
        run: |args, root| with_check(args, "symbols-qet10", |check| symbols::run(root, check)),
    },
    Command {
        name: "api-guard",
        section: Section::Steps,
        args: "",
        does: "compares the published file format (schemas/, fixtures/format/) with its release and names each \
               breaking change",
        run: api_guard::run,
    },
    Command {
        name: "denylist-hash",
        section: Section::Steps,
        args: "<phrase>",
        does: "prints the digests of a confidential phrase for ci/denylist.sha256",
        run: |args, _| match args {
            [phrase] => {
                denylist_hash(phrase);
                Ok(true)
            }
            _ => usage("denylist-hash needs the phrase as one argument"),
        },
    },
    Command {
        name: "blind",
        section: Section::Steps,
        args: "<subcommand>",
        does: "the persona harness (`cargo xtask blind help`)",
        run: blind::run,
    },
    Command {
        name: "help",
        section: Section::Steps,
        args: "",
        does: "this text",
        run: |_, _| {
            print!("{}", help());
            Ok(true)
        },
    },
];

/// The help text, from [`COMMANDS`] and [`RULES`]: each command with its arguments, then what it does, and each rule
/// with what it catches, wrapped to 120 columns.
fn help() -> String {
    const INDENT: usize = 23;
    const WIDTH: usize = 120;
    let item = |out: &mut String, head: String, text: &str| {
        let mut line = if head.len() + 1 < INDENT {
            format!("{head:<INDENT$}")
        } else {
            out.push_str(&head);
            out.push('\n');
            " ".repeat(INDENT)
        };
        for word in text.split_whitespace() {
            if line.len() > INDENT && line.len() + 1 + word.len() > WIDTH {
                out.push_str(line.trim_end());
                out.push('\n');
                line = " ".repeat(INDENT);
            }
            if line.len() > INDENT {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push_str(&line);
        out.push('\n');
    };
    let mut out = String::from("usage: cargo xtask <command>\n");
    for section in [Section::Loop, Section::Reading, Section::Steps] {
        out.push_str(&format!("\n{}\n", section.heading()));
        for c in COMMANDS.iter().filter(|c| c.section == section) {
            let head = if c.args.is_empty() {
                format!("  {}", c.name)
            } else {
                format!("  {} {}", c.name, c.args)
            };
            item(&mut out, head, c.does);
        }
    }
    out.push_str("\nthe repository rules (`rules`, and the last step of `check`), each with what it catches:\n");
    for (name, catches, _) in RULES {
        item(&mut out, format!("  {name}"), catches);
    }
    out
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let name = match args.first().map(String::as_str).unwrap_or("help") {
        "-h" | "--help" => "help",
        other => other,
    };
    let root = workspace_root();
    let result = match COMMANDS.iter().find(|c| c.name == name) {
        Some(c) => (c.run)(args.get(1..).unwrap_or_default(), &root),
        None => usage(&format!("unknown command `{name}`")),
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

/// A command whose one optional argument is `--check`: `run` with whether it was given; any other argument is a usage
/// error.
fn with_check(args: &[String], name: &str, run: impl FnOnce(bool) -> Result<bool>) -> Result<bool> {
    match flag(args, "--check") {
        Some(check) => run(check),
        None => usage(&format!("{name} takes only --check")),
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
    eprint!("{problem}\n\n{}", help());
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
fn metadata_all_features(root: &Path) -> Result<cargo_metadata::Metadata> {
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

/// What the rules read of the workspace: read once by [`rules`] and handed to each rule.
pub(crate) struct Workspace {
    /// `cargo metadata --locked`: the declared edges and the resolve with the default features.
    metadata: cargo_metadata::Metadata,
    /// `cargo metadata --locked --all-features`: the resolve with every optional dependency, which the licence
    /// boundary needs; the required features need the default resolve above, so both are read.
    metadata_all_features: cargo_metadata::Metadata,
    /// `ci/dep-rules.toml`, its layers read from the manifests.
    pub(crate) policy: deps::Policy,
    /// Every file of the repository ([`repo::list_files`]): its path and its bytes.
    files: Vec<(String, Vec<u8>)>,
}

impl Workspace {
    /// Reads the workspace at `root`.
    fn read(root: &Path) -> Result<Self> {
        let metadata = metadata()?;
        let policy = deps::Policy::load(&root.join("ci/dep-rules.toml"))?.with_layers(&metadata);
        let mut files = Vec::new();
        for path in repo::list_files(root)? {
            let bytes = std::fs::read(root.join(&path)).with_context(|| format!("reading {path}"))?;
            files.push((path, bytes));
        }
        Ok(Self {
            metadata_all_features: metadata_all_features(root)?,
            metadata,
            policy,
            files,
        })
    }

    /// The `cargo metadata --locked` of the workspace.
    pub(crate) fn metadata(&self) -> &cargo_metadata::Metadata {
        &self.metadata
    }
}

/// A repository rule: the violations it finds at a root.
type Rule = fn(&Path, &Workspace) -> Result<Violations>;

/// The repository rules, each with its name and what it catches (the help prints both). They read files, git and
/// `cargo metadata` only: nothing is built.
const RULES: &[(&str, &str, Rule)] = &[
    (
        "layers",
        "an edge against the order of the crates or a confinement (ci/dep-rules.toml)",
        check_deps,
    ),
    (
        "features",
        "a test-only feature or crate in a shipped program",
        check_features,
    ),
    (
        "raw-read",
        "a raw read of the model outside its readers, and a stale lint list",
        raw_read::violations,
    ),
    (
        "licences",
        "the licence of every path and crate, and a permissive crate that reaches a GPL one (ci/licence-map.toml)",
        check_licence_map_at,
    ),
    (
        "anonymisation",
        "a confidential token (ci/denylist.sha256)",
        |root, ws| check_anonymisation_at(root, &ws.files),
    ),
    (
        "ui-strings",
        "a literal text with a letter in the window",
        |root, _| ui_strings::check(root),
    ),
    (
        "greyed",
        "a control the window greys without a reason in words",
        |root, _| greyed::check(root),
    ),
    (
        "design-docs",
        "history in a design document, or one over its size ceiling",
        |root, _| design_docs::check(root),
    ),
    (
        "translations",
        "a message of the Dutch guide's introduction or tutorials untranslated",
        |root, _| translations::check(root),
    ),
    (
        "size-ceiling",
        "a source file over the size ceiling of 3,000 lines",
        |_, ws| Ok(ceiling::check(&ws.files)),
    ),
];

/// `rules`: every repository rule, then `gen --check`. Prints one line per rule and each violation.
pub(crate) fn rules(root: &Path) -> Result<bool> {
    let ws = Workspace::read(root)?;
    let mut ok = true;
    for (name, _, rule) in RULES {
        let violations = rule(root, &ws)?;
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

/// The order of the crates, the licence layers and the dependency rules of `ci/dep-rules.toml`.
fn check_deps(root: &Path, ws: &Workspace) -> Result<Violations> {
    let policy = &ws.policy;
    let graph = deps::Graph::from_metadata(&ws.metadata);
    let mut violations = deps::check(policy, &graph);
    violations.extend(deps::check_confined_manifests(policy, root)?);
    violations.extend(deps::importer_purity(root)?);
    Ok(violations)
}

/// Test-only features and crates never reach a shipped binary (`ci/dep-rules.toml`: `banned_features`,
/// `banned_crates`, `uniform_test_features`, `required_features`).
fn check_features(root: &Path, ws: &Workspace) -> Result<Violations> {
    let (metadata, policy) = (&ws.metadata, &ws.policy);
    let rules = features::Rules {
        features: &policy.banned_features,
        crates: &policy.banned_crates,
    };
    let packages = features::from_metadata(metadata);
    let mut violations = features::check(&rules, &packages);
    violations.extend(features::check_uniform(&policy.uniform_test_features, &packages));
    violations.extend(features::check_required(
        &policy.required_features,
        &features::resolved_from_metadata(metadata),
    ));
    // The shipped binaries: every workspace package with a binary target, the development tools aside.
    let shipped: Vec<&str> = metadata
        .workspace_packages()
        .iter()
        .filter(|p| !policy.tools().iter().any(|t| *t == p.name.as_str()))
        .filter(|p| p.targets.iter().any(|t| t.is_bin()))
        .map(|p| p.name.as_str())
        .collect();
    let graph = features::shipping_graph(root, &shipped)?;
    violations.extend(features::check_shipping(&rules, &graph, &shipped.join(", ")));
    Ok(violations)
}

/// Path licences, the collective notice, crate licences and the permissive-layer licence boundary
/// (`ci/licence-map.toml`, `REUSE.toml`).
fn check_licence_map_at(root: &Path, ws: &Workspace) -> Result<Violations> {
    let map = licence::LicenceMap::load(&root.join("ci/licence-map.toml"))?;
    let reuse = licence::ReuseToml::load(&root.join("REUSE.toml"))?;
    let files: Vec<licence::FileInfo> = ws
        .files
        .iter()
        .map(|(path, bytes)| licence::FileInfo {
            path: path.clone(),
            header: licence::read_header(&String::from_utf8_lossy(bytes)),
        })
        .collect();

    let mut violations = licence::check_policy(&map);
    violations.extend(licence::check_files(&map, &reuse, &files));

    let metadata = &ws.metadata_all_features;
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

    // The permissive layer is the crates whose manifest says so.
    let graph = licence::DepGraph::from_metadata(metadata)?;
    violations.extend(licence::check_boundary(
        &map.boundary.allow,
        &ws.policy.permissive,
        &graph,
    ));
    Ok(violations)
}

/// No word n-gram of the repository's `files` is on the confidential-token list (`ci/denylist.sha256`). Binary files
/// (a NUL byte in the first 8 KiB) are skipped. A list without entries is a violation: the guard would match nothing.
pub(crate) fn check_anonymisation_at(root: &Path, files: &[(String, Vec<u8>)]) -> Result<Violations> {
    let list = root.join("ci/denylist.sha256");
    let text = std::fs::read_to_string(&list).with_context(|| format!("reading {}", list.display()))?;
    let denylist = anonymise::parse_denylist(&text).with_context(|| format!("parsing {}", list.display()))?;
    if denylist.is_empty() {
        return Ok(vec![
            "ci/denylist.sha256 has no entries; the guard is inactive".to_owned(),
        ]);
    }
    let texts: Vec<(String, String)> = files
        .iter()
        .filter_map(|(path, bytes)| anonymise::decode(bytes).map(|text| (path.clone(), text)))
        .collect();
    Ok(anonymise::check(&texts, &denylist))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The help is made from the commands that run and the rules: `book-po`, which the hand-kept text left out, is in
    /// it, `check` names `--deep`, and each command's and each rule's line starts with its name.
    #[test]
    fn the_help_names_every_command_and_rule_that_runs() {
        let text = help();
        assert!(text.contains("\n  book-po [--check]"), "{text}");
        assert!(text.contains("[--deep]"), "{text}");
        for name in COMMANDS.iter().map(|c| c.name).chain(RULES.iter().map(|r| r.0)) {
            assert!(text.contains(&format!("\n  {name}")), "{name}");
        }
    }
}
