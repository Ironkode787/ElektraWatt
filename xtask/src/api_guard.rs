// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `api-guard [--base <rev>]`: the API stability guard (NFR-MAINT-10, doc 18 CI-PR-14, WP-S1.22 D14).
//!
//! Five committed snapshots describe the published surfaces (WP-S1.22 D13), each written by its owner's test and read
//! here as plain data, so that this permissive-layer tool links no GPL crate; the file format's published schemas are a
//! sixth surface, below:
//!
//! | Surface | Snapshot | Item |
//! |---|---|---|
//! | `commands` | `crates/ew-commands/schemas/` (`index.json` and one parameter schema per command) | `<name>` or `<name>.<property path>` |
//! | `cli` | `bin/ew/api/cli.json` | `<path words>`, `<path words> <--flag>` or `<path words> exit <code>` |
//! | `results` | `bin/ew/schemas/` (`index.json` and one schema per result) | `<schema name>` or `<schema name> <property path>` |
//! | `queries` | `crates/ew-api/api/queries.json` | `<name>` or `<name>.<property path>` |
//! | `api_v1` | `crates/ew-sqlview/api/api_v1.json` | `<view>` or `<view>.<column>` |
//!
//! The guard reads each snapshot at the merge base of `--base` (default `main`) and `HEAD`, and in the working tree; a
//! snapshot absent at the base counts as empty. It classifies every change:
//!
//! - **breaking:** a removed command, query, subcommand path, flag, result schema, view or column (a rename is a
//!   removal plus an addition); a flag that changed whether it takes a value or lost `repeatable`; an exit code
//!   removed or its meaning changed; a result schema name changed or removed for a path; a removed property of a
//!   command's parameters, a result or a query output, or a changed type of one or of a column; a lowered
//!   `params_version` or `output_version`; a property newly required in a command's parameters (the orchestrator's
//!   ruling 4, cycle 42: inputs only, an output only gains);
//! - **additive:** everything else.
//!
//! Property paths run through `properties` (`a.b`) and `items` (`a[]`) from the top level and from each `$defs` entry,
//! which is keyed by its definition name (`Def.a`). Description texts are not a surface and are not compared.
//!
//! Before 1.0 every surface is Preview (doc 17 §11): a breaking change passes only when
//! `docs/release-notes/unreleased.md` gains, in the same diff (base to working tree), a line `- api-break: <surface>
//! <item>` that ends there or continues with a space (the orchestrator's ruling 5). From the 1.0 freeze ([`FROZEN`]) a
//! break also needs the pull request's label `api-break` and a `docs/adr/` reference in its description, read from the
//! event file at `$GITHUB_EVENT_PATH` as docs.yml's RFC step reads them.
//!
//! **The sixth surface, `format`** (WP-S6.08 D9; ADR-0011 amendment 6). Each format folder `schemas/<f>/` is compared
//! with its base, the oldest final release tag (`v<X.Y.Z>`, no `-rc`) whose tree holds `schemas/<f>/index.json`;
//! without one the format is not released and the guard prints "format <f>: not released; nothing to compare". Its
//! items are `index.json`, `<kind>` (a schema file or an index entry) and `<kind> <path>` (a property path as above).
//! Against the base, **breaking**, which always fails with its own line and no `api-break` line lets pass: a schema
//! file or a kind removed; a property removed, its type changed or newly required; an `enum` value, a `const` or a
//! `oneOf`/`anyOf` branch removed; a kind's `file`, `rank`, `v` or `schema` changed (a reserved kind that gains them is
//! additive), or the index's `format`; any file of `fixtures/format/<f>/<project>/` changed, added or removed (the
//! `<project>.expected/` folders may change). A new value or branch of a node the base holds passes only when
//! `docs/release-notes/unreleased.md` gains `- format-variant: <kind> <path> <value>` since the base tag (the reviewer
//! checks the type's `Unknown(Value)` arm, ADR-0011 amendment 1). A new optional property and a new kind's schema are
//! additive.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Flipped at the 1.0 freeze: a breaking change then also needs the pull request's label and an ADR reference.
pub(crate) const FROZEN: bool = false;

/// The default base revision.
const DEFAULT_BASE: &str = "main";
/// The command-parameter schemas of `ew-commands`.
const COMMANDS_DIR: &str = "crates/ew-commands/schemas";
/// The CLI snapshot of `ew`.
const CLI: &str = "bin/ew/api/cli.json";
/// The result schemas of `ew`.
const RESULTS_DIR: &str = "bin/ew/schemas";
/// The query registry snapshot of `ew-api`.
const QUERIES: &str = "crates/ew-api/api/queries.json";
/// The `api_v1` snapshot of `ew-sqlview`.
const API_V1: &str = "crates/ew-sqlview/api/api_v1.json";
/// The release notes of the next release.
const RELEASE_NOTES: &str = "docs/release-notes/unreleased.md";
/// The published schemas of the file format, one folder per format (WP-S6.08 D9).
const SCHEMAS_DIR: &str = "schemas";
/// The frozen copies of each released format's projects (ADR-0011 amendment 6).
const FIXTURES_DIR: &str = "fixtures/format";
/// The sixth surface's name.
const FORMAT_SURFACE: &str = "format";
/// The nesting depth up to which property paths are followed (a bound on hostile input).
const MAX_DEPTH: usize = 32;

/// The failure without a pull request after the freeze.
const NO_PULL_REQUEST: &str = "api-guard: a breaking change after the freeze needs a pull request labelled api-break \
                               whose description references an ADR (docs/adr/...)";
/// The failure without the label after the freeze.
const NO_LABEL: &str =
    "api-guard: a breaking change after the freeze needs the label api-break on its pull request";
/// The failure without an ADR reference after the freeze.
const NO_ADR: &str = "api-guard: a breaking change after the freeze needs an ADR reference (docs/adr/...) in its pull request's description";

/// The snapshot files of one side: path relative to the root to its text (LF line ends).
type Files = BTreeMap<String, String>;

/// One classified change.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Change {
    breaking: bool,
    surface: &'static str,
    item: String,
    what: String,
}

impl Change {
    fn line(&self) -> String {
        format!(
            "{}: {} {}: {}",
            if self.breaking { "breaking" } else { "additive" },
            self.surface,
            self.item,
            self.what
        )
    }
}

/// What the guard found: the lines it prints and the failure lines (empty when it passes).
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
    /// One line per change, then the summary.
    pub lines: Vec<String>,
    /// The failure text, one line per element; empty when the guard passes.
    pub failure: Vec<String>,
}

// ---------------------------------------------------------------------------------------------------------
// Entry points

/// `api-guard [--base <rev>]`.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    let base = match args {
        [] => DEFAULT_BASE,
        [flag, rev] if flag == "--base" => rev.as_str(),
        _ => bail!("usage: cargo xtask api-guard [--base <rev>]"),
    };
    let report = guard_at(root, base)?;
    for line in &report.lines {
        println!("{line}");
    }
    for line in &report.failure {
        eprintln!("{line}");
    }
    Ok(report.failure.is_empty())
}

/// The guard against the merge base of `base` and `HEAD`, with [`FROZEN`] and the event file of
/// `$GITHUB_EVENT_PATH`.
pub(crate) fn guard_at(root: &Path, base: &str) -> Result<Report> {
    let merge_base = crate::affected::merge_base(root, base)?;
    let event = if FROZEN {
        read_event(std::env::var_os("GITHUB_EVENT_PATH").as_deref().map(Path::new))?
    } else {
        None
    };
    guard(root, &merge_base, FROZEN, event.as_ref())
}

/// The guard of the working tree at `root` against the commit `base`; `frozen` and the pull request `event` stand in
/// for [`FROZEN`] and the event file.
fn guard(root: &Path, base: &str, frozen: bool, event: Option<&Value>) -> Result<Report> {
    let before = base_files(root, base)?;
    let after = working_files(root)?;
    let mut changes = classify(
        &Snapshot::parse(&before, "the base")?,
        &Snapshot::parse(&after, "the working tree")?,
    );
    let notes = |files: &Files| files.get(RELEASE_NOTES).cloned().unwrap_or_default();
    let format = format_guard(root, &notes(&after))?;
    changes.extend(format.changes);
    let mut report = report(&changes, &notes(&before), &notes(&after), frozen, event);
    let summary = report.lines.len() - 1;
    report.lines.splice(summary..summary, format.notes);
    report.failure.extend(format.failure);
    Ok(report)
}

/// The pull request event at `path`, if a path is given.
fn read_event(path: Option<&Path>) -> Result<Option<Value>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let value = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(value))
}

/// The report of `changes`: the change lines and the summary, and the failure when a break lacks its release-note line
/// added between `notes_before` and `notes_after`, or, when `frozen`, the label or the ADR reference of `event`.
fn report(
    changes: &[Change],
    notes_before: &str,
    notes_after: &str,
    frozen: bool,
    event: Option<&Value>,
) -> Report {
    let mut lines: Vec<String> = changes.iter().map(Change::line).collect();
    let breaking: Vec<&Change> = changes.iter().filter(|c| c.breaking).collect();
    lines.push(format!(
        "api-guard: {} breaking, {} additive",
        breaking.len(),
        changes.len() - breaking.len()
    ));

    // A released format's break fails on its own text, whatever the notes say ([`format_guard`]).
    let breaking: Vec<&Change> = breaking
        .into_iter()
        .filter(|c| c.surface != FORMAT_SURFACE)
        .collect();
    let before: BTreeSet<&str> = notes_before.lines().collect();
    let added: Vec<&str> = notes_after.lines().filter(|l| !before.contains(l)).collect();
    let uncovered: Vec<&Change> = breaking
        .iter()
        .copied()
        .filter(|c| !covered(c.surface, &c.item, &added))
        .collect();
    let mut failure = Vec::new();
    if !uncovered.is_empty() {
        failure.push(format!(
            "api-guard: {} breaking change(s) without a release-note line in {RELEASE_NOTES}; add for each: - \
             api-break: <surface> <item>",
            uncovered.len()
        ));
        let mut seen = BTreeSet::new();
        for c in uncovered {
            let line = format!("- api-break: {} {}", c.surface, c.item);
            if seen.insert(line.clone()) {
                failure.push(line);
            }
        }
    }
    if frozen && !breaking.is_empty() {
        match event
            .and_then(|e| e.get("pull_request"))
            .filter(|p| p.is_object())
        {
            None => failure.push(NO_PULL_REQUEST.to_owned()),
            Some(pr) => {
                let labelled = pr.get("labels").and_then(Value::as_array).is_some_and(|l| {
                    l.iter()
                        .any(|l| l.get("name").and_then(Value::as_str) == Some("api-break"))
                });
                if !labelled {
                    failure.push(NO_LABEL.to_owned());
                }
                if !pr
                    .get("body")
                    .and_then(Value::as_str)
                    .is_some_and(|b| b.contains("docs/adr/"))
                {
                    failure.push(NO_ADR.to_owned());
                }
            }
        }
    }
    Report { lines, failure }
}

/// Whether one of the `added` release-note lines covers the break of `item` on `surface`: it starts
/// `- api-break: <surface> <item>` and ends there or continues with a space (the orchestrator's ruling 5).
fn covered(surface: &str, item: &str, added: &[&str]) -> bool {
    let prefix = format!("- api-break: {surface} {item}");
    added.iter().any(|line| {
        let line = line.trim_end();
        line.strip_prefix(&prefix)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

// ---------------------------------------------------------------------------------------------------------
// Reading the snapshots

/// Whether `path` is one of the snapshot files: a JSON file directly in a schema directory, or a named file.
fn is_snapshot_file(path: &str) -> bool {
    [COMMANDS_DIR, RESULTS_DIR].iter().any(|dir| {
        path.strip_prefix(dir)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|name| !name.contains('/') && name.ends_with(".json"))
    }) || [CLI, QUERIES, API_V1, RELEASE_NOTES].contains(&path)
}

/// The snapshot files of the working tree.
fn working_files(root: &Path) -> Result<Files> {
    let mut paths = Vec::new();
    for dir in [COMMANDS_DIR, RESULTS_DIR] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            paths.push(format!("{dir}/{}", entry.file_name().to_string_lossy()));
        }
    }
    paths.extend([CLI, QUERIES, API_V1, RELEASE_NOTES].map(str::to_owned));
    let mut files = Files::new();
    for path in paths.into_iter().filter(|p| is_snapshot_file(p)) {
        match std::fs::read_to_string(root.join(&path)) {
            Ok(text) => {
                files.insert(path, text.replace("\r\n", "\n"));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("reading {path}")),
        }
    }
    Ok(files)
}

/// The snapshot files of the commit `rev`: `git ls-tree` lists them, one `git cat-file --batch` reads them.
fn base_files(root: &Path, rev: &str) -> Result<Files> {
    let listed = crate::repo::git(
        root,
        &[
            "ls-tree",
            "-r",
            "--name-only",
            rev,
            "--",
            COMMANDS_DIR,
            RESULTS_DIR,
            CLI,
            QUERIES,
            API_V1,
            RELEASE_NOTES,
        ],
    )?;
    let paths: Vec<&str> = listed.lines().filter(|p| is_snapshot_file(p)).collect();
    read_blobs(root, rev, &paths)
}

/// The text of each of `paths` at the commit `rev` (LF line ends), read by one `git cat-file --batch`.
fn read_blobs(root: &Path, rev: &str, paths: &[&str]) -> Result<Files> {
    if paths.is_empty() {
        return Ok(Files::new());
    }
    let mut child = Command::new("git")
        .args(["cat-file", "--batch"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting `git cat-file --batch`")?;
    let input: String = paths.iter().map(|p| format!("{rev}:{p}\n")).collect();
    let mut stdin = child.stdin.take().context("the stdin of `git cat-file`")?;
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child
        .wait_with_output()
        .context("running `git cat-file --batch`")?;
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("writing to `git cat-file --batch` panicked"))?
        .context("writing to `git cat-file --batch`")?;
    if !output.status.success() {
        bail!(
            "`git cat-file --batch` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let mut files = Files::new();
    let mut rest: &[u8] = &output.stdout;
    for &path in paths {
        let end = rest
            .iter()
            .position(|b| *b == b'\n')
            .with_context(|| format!("`git cat-file` printed no header for {rev}:{path}"))?;
        let header = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        let size: usize = match header.split(' ').collect::<Vec<_>>()[..] {
            [_, "blob", size] => size
                .parse()
                .with_context(|| format!("`git cat-file` header `{header}`"))?,
            _ => bail!("`git cat-file` could not read {rev}:{path}: {header}"),
        };
        if rest.len() < size + 1 {
            bail!("`git cat-file` printed {rev}:{path} truncated");
        }
        let text =
            String::from_utf8(rest[..size].to_vec()).with_context(|| format!("{rev}:{path} is not UTF-8"))?;
        files.insert(path.to_owned(), text.replace("\r\n", "\n"));
        rest = &rest[size + 1..];
    }
    Ok(files)
}

/// The five surfaces of one side, keyed by name.
#[derive(Default)]
struct Snapshot {
    /// Command name to (registry entry, parameter schema).
    commands: BTreeMap<String, (Value, Value)>,
    /// Path words to entry.
    cli: BTreeMap<String, Value>,
    /// Result schema name to schema.
    results: BTreeMap<String, Value>,
    /// Query name to entry (its output schema included).
    queries: BTreeMap<String, Value>,
    /// View name to view.
    views: BTreeMap<String, Value>,
}

impl Snapshot {
    /// The surfaces of `files`; `side` names the side in an error.
    fn parse(files: &Files, side: &str) -> Result<Snapshot> {
        let json = |path: &str| -> Result<Option<Value>> {
            files
                .get(path)
                .map(|text| serde_json::from_str(text).with_context(|| format!("parsing {path} of {side}")))
                .transpose()
        };
        let list = |value: &Option<Value>, key: &str| -> Vec<Value> {
            value
                .as_ref()
                .and_then(|v| v.get(key))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let name = |entry: &Value, key: &str| entry.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
        let mut s = Snapshot::default();

        let index = json(&format!("{COMMANDS_DIR}/index.json"))?;
        for entry in list(&index, "commands") {
            let schema = json(&format!("{COMMANDS_DIR}/{}", name(&entry, "schema")))?.unwrap_or(Value::Null);
            s.commands.insert(name(&entry, "name"), (entry, schema));
        }
        for entry in list(&json(CLI)?, "cli") {
            s.cli.insert(name(&entry, "path"), entry);
        }
        let index = json(&format!("{RESULTS_DIR}/index.json"))?;
        for entry in list(&index, "results") {
            let schema = json(&format!("{RESULTS_DIR}/{}", name(&entry, "schema")))?.unwrap_or(Value::Null);
            s.results.insert(name(&entry, "name"), schema);
        }
        for entry in list(&json(QUERIES)?, "queries") {
            s.queries.insert(name(&entry, "name"), entry);
        }
        for view in list(&json(API_V1)?, "views") {
            s.views.insert(name(&view, "name"), view);
        }
        Ok(s)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Classification

/// A compact JSON text of a value.
fn compact(v: Option<&Value>) -> String {
    v.map_or_else(|| "nothing".to_owned(), Value::to_string)
}

fn bool_of(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Every change between `before` and `after`, in surface order (commands, cli, results, queries, api_v1) and, within
/// a surface, in item order.
fn classify(before: &Snapshot, after: &Snapshot) -> Vec<Change> {
    let mut out = Vec::new();
    commands(&mut out, before, after);
    cli(&mut out, before, after);
    results(&mut out, before, after);
    queries(&mut out, before, after);
    api_v1(&mut out, before, after);
    out
}

fn push(
    out: &mut Vec<Change>,
    breaking: bool,
    surface: &'static str,
    item: impl Into<String>,
    what: impl Into<String>,
) {
    out.push(Change {
        breaking,
        surface,
        item: item.into(),
        what: what.into(),
    });
}

/// A version member that may only rise: lowered is breaking, raised additive.
fn version(
    out: &mut Vec<Change>,
    surface: &'static str,
    item: &str,
    key: &str,
    before: &Value,
    after: &Value,
) {
    let (b, a) = (
        before.get(key).and_then(Value::as_u64),
        after.get(key).and_then(Value::as_u64),
    );
    if let (Some(b), Some(a)) = (b, a) {
        if a < b {
            push(out, true, surface, item, format!("{key} lowered from {b} to {a}"));
        } else if a > b {
            push(out, false, surface, item, format!("{key} raised from {b} to {a}"));
        }
    }
}

fn commands(out: &mut Vec<Change>, before: &Snapshot, after: &Snapshot) {
    const S: &str = "commands";
    for (name, (entry, schema)) in &before.commands {
        let Some((new_entry, new_schema)) = after.commands.get(name) else {
            push(out, true, S, name, "removed");
            continue;
        };
        version(out, S, name, "params_version", entry, new_entry);
        for key in [
            "aliases",
            "cost",
            "exposure",
            "confirmation",
            "undoable",
            "stability",
        ] {
            if entry.get(key) != new_entry.get(key) {
                push(
                    out,
                    false,
                    S,
                    name,
                    format!(
                        "{key} changed from {} to {}",
                        compact(entry.get(key)),
                        compact(new_entry.get(key))
                    ),
                );
            }
        }
        schema_changes(out, S, &|p| format!("{name}.{p}"), schema, new_schema, true);
    }
    for name in after
        .commands
        .keys()
        .filter(|n| !before.commands.contains_key(*n))
    {
        push(out, false, S, name, "added");
    }
}

fn cli(out: &mut Vec<Change>, before: &Snapshot, after: &Snapshot) {
    const S: &str = "cli";
    for (path, entry) in &before.cli {
        let Some(new_entry) = after.cli.get(path) else {
            push(out, true, S, path, "removed");
            continue;
        };
        let by = |e: &Value, list: &str, key: &str| -> BTreeMap<String, Value> {
            e.get(list)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|i| {
                            (
                                i.get(key).map_or_else(String::new, |k| {
                                    k.as_str().map_or_else(|| k.to_string(), str::to_owned)
                                }),
                                i.clone(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let (flags, new_flags) = (by(entry, "flags", "name"), by(new_entry, "flags", "name"));
        for (flag, f) in &flags {
            let item = format!("{path} {flag}");
            let Some(nf) = new_flags.get(flag) else {
                push(out, true, S, item, "removed");
                continue;
            };
            match (bool_of(f, "value"), bool_of(nf, "value")) {
                (false, true) => push(out, true, S, &item, "now takes a value"),
                (true, false) => push(out, true, S, &item, "no longer takes a value"),
                _ => {}
            }
            match (bool_of(f, "repeatable"), bool_of(nf, "repeatable")) {
                (true, false) => push(out, true, S, &item, "no longer repeatable"),
                (false, true) => push(out, false, S, &item, "now repeatable"),
                _ => {}
            }
        }
        for flag in new_flags.keys().filter(|f| !flags.contains_key(*f)) {
            push(out, false, S, format!("{path} {flag}"), "added");
        }
        let (exits, new_exits) = (by(entry, "exits", "code"), by(new_entry, "exits", "code"));
        for (code, x) in &exits {
            let item = format!("{path} exit {code}");
            match new_exits.get(code) {
                None => push(out, true, S, item, "removed"),
                Some(nx) if nx.get("meaning") != x.get("meaning") => push(
                    out,
                    true,
                    S,
                    item,
                    format!(
                        "meaning changed from {} to {}",
                        compact(x.get("meaning")),
                        compact(nx.get("meaning"))
                    ),
                ),
                Some(_) => {}
            }
        }
        for code in new_exits.keys().filter(|c| !exits.contains_key(*c)) {
            push(out, false, S, format!("{path} exit {code}"), "added");
        }
        let result = |e: &Value| e.get("result").and_then(Value::as_str).map(str::to_owned);
        match (result(entry), result(new_entry)) {
            (Some(b), Some(a)) if a != b => {
                push(out, true, S, path, format!("result changed from {b} to {a}"))
            }
            (Some(b), None) => push(out, true, S, path, format!("result {b} removed")),
            (None, Some(a)) => push(out, false, S, path, format!("result {a} added")),
            _ => {}
        }
        if entry.get("stability") != new_entry.get("stability") {
            push(
                out,
                false,
                S,
                path,
                format!(
                    "stability changed from {} to {}",
                    compact(entry.get("stability")),
                    compact(new_entry.get("stability"))
                ),
            );
        }
    }
    for path in after.cli.keys().filter(|p| !before.cli.contains_key(*p)) {
        push(out, false, S, path, "added");
    }
}

fn results(out: &mut Vec<Change>, before: &Snapshot, after: &Snapshot) {
    const S: &str = "results";
    for (name, schema) in &before.results {
        let Some(new_schema) = after.results.get(name) else {
            push(out, true, S, name, "removed");
            continue;
        };
        schema_changes(out, S, &|p| format!("{name} {p}"), schema, new_schema, false);
    }
    for name in after.results.keys().filter(|n| !before.results.contains_key(*n)) {
        push(out, false, S, name, "added");
    }
}

fn queries(out: &mut Vec<Change>, before: &Snapshot, after: &Snapshot) {
    const S: &str = "queries";
    for (name, entry) in &before.queries {
        let Some(new_entry) = after.queries.get(name) else {
            push(out, true, S, name, "removed");
            continue;
        };
        version(out, S, name, "output_version", entry, new_entry);
        if entry.get("key") != new_entry.get("key") {
            push(
                out,
                false,
                S,
                name,
                format!(
                    "key changed from {} to {}",
                    compact(entry.get("key")),
                    compact(new_entry.get("key"))
                ),
            );
        }
        let output = |e: &Value| e.get("output").cloned().unwrap_or(Value::Null);
        schema_changes(
            out,
            S,
            &|p| format!("{name}.{p}"),
            &output(entry),
            &output(new_entry),
            false,
        );
    }
    for name in after.queries.keys().filter(|n| !before.queries.contains_key(*n)) {
        push(out, false, S, name, "added");
    }
}

fn api_v1(out: &mut Vec<Change>, before: &Snapshot, after: &Snapshot) {
    const S: &str = "api_v1";
    let preview = |out: &mut Vec<Change>, item: &str, b: &Value, a: &Value| match (
        bool_of(b, "preview"),
        bool_of(a, "preview"),
    ) {
        (true, false) => push(out, false, S, item, "no longer a Preview addition"),
        (false, true) => push(out, false, S, item, "now a Preview addition"),
        _ => {}
    };
    let columns = |v: &Value| -> BTreeMap<String, Value> {
        v.get("columns")
            .and_then(Value::as_array)
            .map(|cols| {
                cols.iter()
                    .map(|c| {
                        (
                            c.get("name").and_then(Value::as_str).unwrap_or("").to_owned(),
                            c.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    for (name, view) in &before.views {
        let Some(new_view) = after.views.get(name) else {
            push(out, true, S, name, "removed");
            continue;
        };
        preview(out, name, view, new_view);
        let (cols, new_cols) = (columns(view), columns(new_view));
        for (col, c) in &cols {
            let item = format!("{name}.{col}");
            let Some(nc) = new_cols.get(col) else {
                push(out, true, S, item, "removed");
                continue;
            };
            if c.get("type") != nc.get("type") {
                push(
                    out,
                    true,
                    S,
                    &item,
                    format!(
                        "type changed from {} to {}",
                        type_text(c.get("type")),
                        type_text(nc.get("type"))
                    ),
                );
            }
            preview(out, &item, c, nc);
        }
        for col in new_cols.keys().filter(|c| !cols.contains_key(*c)) {
            push(out, false, S, format!("{name}.{col}"), "added");
        }
    }
    for name in after.views.keys().filter(|n| !before.views.contains_key(*n)) {
        push(out, false, S, name, "added");
    }
}

/// A `type` value as text: a string bare, a list joined with "or", anything else as JSON.
fn type_text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|i| i.as_str().map_or_else(|| i.to_string(), str::to_owned))
            .collect::<Vec<_>>()
            .join(" or "),
        other => compact(other),
    }
}

/// The type of a property for comparison: its `type`, the definition its `$ref` names, or the types of its `anyOf`
/// or `oneOf` alternatives; `None` when it states none of these (it is then not compared).
fn type_signature(v: &Value) -> Option<String> {
    if v.get("type").is_some() {
        return Some(type_text(v.get("type")));
    }
    if let Some(r) = v.get("$ref").and_then(Value::as_str) {
        return Some(r.rsplit('/').next().unwrap_or(r).to_owned());
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(alts) = v.get(key).and_then(Value::as_array) {
            let parts: Option<Vec<String>> = alts.iter().map(type_signature).collect();
            return parts.map(|p| p.join(" or "));
        }
    }
    None
}

/// The property paths of a schema.
#[derive(Default)]
struct Shape {
    /// Property path to its type signature.
    types: BTreeMap<String, Option<String>>,
    /// The required property paths.
    required: BTreeSet<String>,
    /// The paths that hold properties: the top level (`""`), each definition and each property.
    objects: BTreeSet<String>,
}

fn shape_of(schema: &Value) -> Shape {
    let mut shape = Shape::default();
    shape.objects.insert(String::new());
    walk(schema, "", &mut shape, 0);
    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for (name, def) in defs {
            shape.objects.insert(name.clone());
            walk(def, name, &mut shape, 0);
        }
    }
    shape
}

fn walk(node: &Value, prefix: &str, shape: &mut Shape, depth: usize) {
    if depth >= MAX_DEPTH {
        return;
    }
    if let Some(props) = node.get("properties").and_then(Value::as_object) {
        let required: BTreeSet<&str> = node
            .get("required")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for (name, prop) in props {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            shape.types.insert(path.clone(), type_signature(prop));
            shape.objects.insert(path.clone());
            if required.contains(name.as_str()) {
                shape.required.insert(path.clone());
            }
            walk(prop, &path, shape, depth + 1);
        }
    }
    if let Some(items) = node.get("items").filter(|i| i.is_object()) {
        let path = format!("{prefix}[]");
        shape.types.insert(path.clone(), type_signature(items));
        shape.objects.insert(path.clone());
        walk(items, &path, shape, depth + 1);
    }
}

/// The path that holds `path`: `a.b` → `a`, `a[]` → `a`, `a` → `""`.
fn parent(path: &str) -> &str {
    if let Some(p) = path.strip_suffix("[]") {
        return p;
    }
    path.rsplit_once('.').map_or("", |(p, _)| p)
}

/// The property changes of a schema; `inputs` when it is a command's parameters (a newly required property breaks).
fn schema_changes(
    out: &mut Vec<Change>,
    surface: &'static str,
    item: &dyn Fn(&str) -> String,
    before: &Value,
    after: &Value,
    inputs: bool,
) {
    let (b, a) = (shape_of(before), shape_of(after));
    for (path, ty) in &b.types {
        let Some(new_ty) = a.types.get(path) else {
            push(out, true, surface, item(path), "removed");
            continue;
        };
        if let (Some(ty), Some(new_ty)) = (ty, new_ty)
            && ty != new_ty
        {
            push(
                out,
                true,
                surface,
                item(path),
                format!("type changed from {ty} to {new_ty}"),
            );
        }
        match (b.required.contains(path), a.required.contains(path)) {
            (false, true) => push(out, inputs, surface, item(path), "newly required"),
            (true, false) => push(out, false, surface, item(path), "no longer required"),
            _ => {}
        }
    }
    for path in a.types.keys().filter(|p| !b.types.contains_key(*p)) {
        if inputs && a.required.contains(path) && b.objects.contains(parent(path)) {
            push(out, true, surface, item(path), "added as required");
        } else {
            push(out, false, surface, item(path), "added");
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The format surface (WP-S6.08 D9)

/// The guard's lines and failures of the `format` surface.
#[derive(Default)]
struct FormatReport {
    /// One classified change per item, as the other surfaces'.
    changes: Vec<Change>,
    /// A format that is not released: "nothing to compare".
    notes: Vec<String>,
    /// S5, S6 and S7 of WP-S6.08 §8, one line each.
    failure: Vec<String>,
}

/// The `format` surface: each format folder `schemas/<f>/` of the working tree or of a release, against its base, the
/// oldest final release tag whose tree holds `schemas/<f>/index.json`, with `notes_after` the working tree's release
/// notes. Without a base the format is not released and nothing is compared.
fn format_guard(root: &Path, notes_after: &str) -> Result<FormatReport> {
    let mut out = FormatReport::default();
    let mut base: BTreeMap<String, String> = BTreeMap::new();
    for tag in final_tags(root)? {
        let listed = crate::repo::git(root, &["ls-tree", "-r", "--name-only", &tag, "--", SCHEMAS_DIR])?;
        for f in listed.lines().filter_map(|p| {
            p.strip_prefix(SCHEMAS_DIR)?
                .strip_prefix('/')?
                .strip_suffix("/index.json")
        }) {
            base.entry(f.to_owned()).or_insert_with(|| tag.clone());
        }
    }
    let mut formats: BTreeSet<String> = base.keys().cloned().collect();
    if let Ok(entries) = std::fs::read_dir(root.join(SCHEMAS_DIR)) {
        for entry in entries.flatten() {
            if entry.path().join("index.json").is_file() {
                formats.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    for f in formats {
        match base.get(&f) {
            None => out
                .notes
                .push(format!("format {f}: not released; nothing to compare")),
            Some(tag) => compare_format(root, &f, tag, notes_after, &mut out)?,
        }
    }
    Ok(out)
}

/// The final release tags `v<X.Y.Z>` (no `-rc` or other suffix), oldest version first.
fn final_tags(root: &Path) -> Result<Vec<String>> {
    let listed = crate::repo::git(root, &["tag", "--list", "v*"])?;
    let mut tags: Vec<((u64, u64, u64), String)> = listed
        .lines()
        .filter_map(|t| {
            let mut parts = t.strip_prefix('v')?.split('.');
            let mut number = || -> Option<u64> {
                let p = parts.next()?;
                (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse().ok())?
            };
            let version = (number()?, number()?, number()?);
            parts.next().is_none().then(|| (version, t.to_owned()))
        })
        .collect();
    tags.sort();
    Ok(tags.into_iter().map(|(_, t)| t).collect())
}

/// The format `f` of the working tree against its release `tag`: the index, every schema, the frozen fixture.
fn compare_format(root: &Path, f: &str, tag: &str, notes_after: &str, out: &mut FormatReport) -> Result<()> {
    let dir = format!("{SCHEMAS_DIR}/{f}");
    let listed = crate::repo::git(root, &["ls-tree", "-r", "--name-only", tag, "--", &dir])?;
    let paths: Vec<&str> = listed
        .lines()
        .filter(|p| {
            p.strip_prefix(dir.as_str())
                .and_then(|rest| rest.strip_prefix('/'))
                .is_some_and(|name| !name.contains('/') && name.ends_with(".json"))
        })
        .chain([RELEASE_NOTES])
        .collect();
    let mut before = read_blobs(root, tag, &paths)?;
    let notes_before = before.remove(RELEASE_NOTES).unwrap_or_default();
    let json = |text: &str, path: &str, side: &str| -> Result<Value> {
        serde_json::from_str(text).with_context(|| format!("parsing {path} of {side}"))
    };
    let mut changes: Vec<(bool, String, String)> = Vec::new();
    let mut variants: Vec<(String, String)> = Vec::new();
    for (path, text) in &before {
        let name = &path[dir.len() + 1..];
        let item = name.strip_suffix(".schema.json").unwrap_or(name);
        let after = match std::fs::read_to_string(root.join(path)) {
            Ok(after) => after,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                changes.push((true, item.to_owned(), "removed".to_owned()));
                continue;
            }
            Err(e) => return Err(e).with_context(|| format!("reading {path}")),
        };
        let (b, a) = (json(text, path, tag)?, json(&after, path, "the working tree")?);
        if name == "index.json" {
            index_changes(&b, &a, &mut changes);
        } else {
            node_changes(item, &b, &a, &mut changes, &mut variants);
        }
    }
    let new: BTreeSet<String> = std::fs::read_dir(root.join(&dir))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json") && !before.contains_key(&format!("{dir}/{name}")))
        .collect();
    for name in new {
        let item = name.strip_suffix(".schema.json").unwrap_or(&name).to_owned();
        changes.push((false, item, "added".to_owned()));
    }
    let ending = "a released format changes only additively (ADR-0011 amendment 6)";
    for (breaking, item, what) in changes {
        if breaking {
            out.failure
                .push(format!("format {f}: {item}: {what} since {tag}; {ending}"));
        }
        push(
            &mut out.changes,
            breaking,
            FORMAT_SURFACE,
            format!("{f} {item}"),
            what,
        );
    }
    let gained: BTreeSet<&str> = notes_before.lines().collect();
    let added: Vec<&str> = notes_after.lines().filter(|l| !gained.contains(l)).collect();
    for (item, value) in variants {
        let line = format!("- format-variant: {item} {value}");
        let covered = added.iter().any(|l| {
            l.trim_end()
                .strip_prefix(&line)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
        });
        if !covered {
            out.failure.push(format!(
                "format {f}: {item}: the new value {value} since {tag} needs the line \"{line}\" in {RELEASE_NOTES}"
            ));
        }
    }
    // The frozen copy: every byte of `fixtures/format/<f>/<project>/`, its `.expected` folders aside.
    let fixtures = format!("{FIXTURES_DIR}/{f}");
    let diff = crate::repo::git(
        root,
        &["diff", "--name-only", "--no-renames", tag, "--", &fixtures],
    )?;
    let new = crate::repo::git(
        root,
        &["ls-files", "--others", "--exclude-standard", "--", &fixtures],
    )?;
    let changed: BTreeSet<&str> = diff.lines().chain(new.lines()).collect();
    for path in changed {
        let below = path
            .strip_prefix(fixtures.as_str())
            .and_then(|r| r.strip_prefix('/'));
        if below.is_some_and(|b| !b.split('/').next().is_some_and(|p| p.ends_with(".expected"))) {
            push(
                &mut out.changes,
                true,
                FORMAT_SURFACE,
                format!("{f} {path}"),
                "changed",
            );
            out.failure.push(format!(
                "format {f}: {path} changed since {tag}; a released format's fixture never changes"
            ));
        }
    }
    Ok(())
}

/// The changes of the format's `index.json`: its `format`, and each kind's `file`, `rank`, `v` and `schema` break (but a
/// reserved kind that gains its `v` and schema, as a new kind's schema, and a changed `status`); a kind removed breaks,
/// a kind added does not.
fn index_changes(before: &Value, after: &Value, out: &mut Vec<(bool, String, String)>) {
    if before.get("format") != after.get("format") {
        out.push((
            true,
            "index.json".to_owned(),
            format!(
                "format changed from {} to {}",
                compact(before.get("format")),
                compact(after.get("format"))
            ),
        ));
    }
    let kinds = |v: &Value| -> BTreeMap<String, Value> {
        v.get("kinds")
            .and_then(Value::as_array)
            .map(|ks| {
                ks.iter()
                    .map(|k| {
                        (
                            k.get("kind").and_then(Value::as_str).unwrap_or("").to_owned(),
                            k.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let (b, a) = (kinds(before), kinds(after));
    for (kind, k) in &b {
        let Some(nk) = a.get(kind) else {
            out.push((true, kind.clone(), "removed from index.json".to_owned()));
            continue;
        };
        for key in ["file", "rank", "v", "schema", "status"] {
            let (old, new) = (k.get(key), nk.get(key));
            if old != new {
                let breaking = match key {
                    "file" | "rank" => true,
                    "status" => false,
                    _ => !old.is_none_or(Value::is_null),
                };
                out.push((
                    breaking,
                    kind.clone(),
                    format!("{key} changed from {} to {}", compact(old), compact(new)),
                ));
            }
        }
    }
    for kind in a.keys().filter(|k| !b.contains_key(*k)) {
        out.push((false, kind.clone(), "added to index.json".to_owned()));
    }
}

/// One node of a format schema: its type, whether it is required, and its values (`enum` values, `const`, and the
/// `oneOf` and `anyOf` branches).
#[derive(Default)]
struct FormatNode {
    ty: Option<String>,
    required: bool,
    values: BTreeSet<String>,
}

/// The nodes of a format schema by property path (`a.b`, `a[]`, from the top level `""` and from each `$defs` entry
/// by its name), as the other surfaces name them.
fn format_nodes(schema: &Value) -> BTreeMap<String, FormatNode> {
    let mut nodes = BTreeMap::new();
    format_walk(schema, "", &mut nodes, 0);
    if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
        for (name, def) in defs {
            format_walk(def, name, &mut nodes, 0);
        }
    }
    nodes
}

/// A value as the guard names it: a string bare, anything else as JSON.
fn value_word(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// A `oneOf` or `anyOf` branch as the guard names it: its `const`, the definition its `$ref` names, `{<members>}` for an
/// object, else its type.
fn branch_word(b: &Value) -> String {
    if let Some(c) = b.get("const") {
        return value_word(c);
    }
    if let Some(r) = b.get("$ref").and_then(Value::as_str) {
        return r.rsplit('/').next().unwrap_or(r).to_owned();
    }
    if let Some(props) = b.get("properties").and_then(Value::as_object) {
        return format!("{{{}}}", props.keys().cloned().collect::<Vec<_>>().join(","));
    }
    match b.get("type") {
        Some(t) => type_text(Some(t)),
        None => compact(Some(b)),
    }
}

fn format_walk(node: &Value, path: &str, nodes: &mut BTreeMap<String, FormatNode>, depth: usize) {
    if depth >= MAX_DEPTH {
        return;
    }
    let entry = nodes.entry(path.to_owned()).or_default();
    if entry.ty.is_none() {
        entry.ty = match (node.get("type"), node.get("$ref").and_then(Value::as_str)) {
            (Some(t), _) => Some(type_text(Some(t))),
            (None, Some(r)) => Some(r.rsplit('/').next().unwrap_or(r).to_owned()),
            (None, None) => None,
        };
    }
    if let Some(values) = node.get("enum").and_then(Value::as_array) {
        entry.values.extend(values.iter().map(value_word));
    }
    if let Some(c) = node.get("const") {
        entry.values.insert(value_word(c));
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(branches) = node.get(key).and_then(Value::as_array) {
            nodes
                .entry(path.to_owned())
                .or_default()
                .values
                .extend(branches.iter().map(branch_word));
            // The members of an object branch are the node's own.
            for b in branches.iter().filter(|b| b.get("const").is_none()) {
                format_members(b, path, nodes, depth + 1);
            }
        }
    }
    format_members(node, path, nodes, depth);
}

/// The `properties` and `items` of `node` at `path`.
fn format_members(node: &Value, path: &str, nodes: &mut BTreeMap<String, FormatNode>, depth: usize) {
    if let Some(props) = node.get("properties").and_then(Value::as_object) {
        let required: BTreeSet<&str> = node
            .get("required")
            .and_then(Value::as_array)
            .map(|r| r.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for (name, prop) in props {
            let child = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            format_walk(prop, &child, nodes, depth + 1);
            if required.contains(name.as_str()) {
                nodes.entry(child).or_default().required = true;
            }
        }
    }
    if let Some(items) = node.get("items").filter(|i| i.is_object()) {
        format_walk(items, &format!("{path}[]"), nodes, depth + 1);
    }
}

/// The changes of one schema of kind `kind`: a node removed, retyped or newly required breaks, a value removed breaks;
/// a new value of a node that both sides hold is additive and goes into `variants` (it needs its release-note line).
fn node_changes(
    kind: &str,
    before: &Value,
    after: &Value,
    out: &mut Vec<(bool, String, String)>,
    variants: &mut Vec<(String, String)>,
) {
    let (b, a) = (format_nodes(before), format_nodes(after));
    let item = |path: &str| {
        if path.is_empty() {
            kind.to_owned()
        } else {
            format!("{kind} {path}")
        }
    };
    for (path, node) in &b {
        let Some(new) = a.get(path) else {
            out.push((true, item(path), "removed".to_owned()));
            continue;
        };
        if let (Some(ty), Some(new_ty)) = (&node.ty, &new.ty)
            && ty != new_ty
        {
            out.push((true, item(path), format!("type changed from {ty} to {new_ty}")));
        }
        match (node.required, new.required) {
            (false, true) => out.push((true, item(path), "newly required".to_owned())),
            (true, false) => out.push((false, item(path), "no longer required".to_owned())),
            _ => {}
        }
        for value in node.values.difference(&new.values) {
            out.push((true, item(path), format!("value {value} removed")));
        }
        for value in new.values.difference(&node.values) {
            out.push((false, item(path), format!("value {value} added")));
            variants.push((item(path), value.clone()));
        }
    }
    for (path, node) in a.iter().filter(|(p, _)| !b.contains_key(*p)) {
        if node.required && b.contains_key(parent(path)) {
            out.push((true, item(path), "added as required".to_owned()));
        } else {
            out.push((false, item(path), "added".to_owned()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A scratch repository at a fresh temporary directory; removed by [`Scratch::remove`].
    struct Scratch {
        dir: std::path::PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!("ew-xtask-api-guard-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let s = Scratch { dir };
            s.git(&["init", "-q"]);
            s
        }

        /// Runs git without the user's configuration.
        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .args([
                    "-c",
                    "user.name=xtask",
                    "-c",
                    "user.email=xtask",
                    "-c",
                    "core.autocrlf=false",
                ])
                .args(args)
                .current_dir(&self.dir)
                .env("GIT_CONFIG_GLOBAL", self.dir.join("no-global-config"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        }

        fn write(&self, rel: &str, text: &str) {
            let path = self.dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }

        fn write_json(&self, rel: &str, value: &Value) {
            self.write(
                rel,
                &format!("{}\n", serde_json::to_string_pretty(value).unwrap()),
            );
        }

        fn remove_file(&self, rel: &str) {
            std::fs::remove_file(self.dir.join(rel)).unwrap();
        }

        /// Commits every file and returns the commit.
        fn commit(&self, message: &str) -> String {
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", message]);
            self.git(&["rev-parse", "HEAD"])
        }

        fn remove(self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn command_entry(name: &str) -> Value {
        json!({"aliases": [], "confirmation": "none", "cost": "interactive", "exposure": "edit", "name": name,
            "params_version": 1, "schema": format!("{name}.schema.json"), "stability": "preview", "undoable": true})
    }

    fn params(required: &[&str]) -> Value {
        json!({"type": "object", "properties": {"x": {"type": "string"}}, "required": required})
    }

    fn cli_snapshot(flags: Value) -> Value {
        json!({"cli": [{"path": "export pdf", "flags": flags,
            "exits": [{"code": 0, "meaning": "success"}], "result": "ew.export-pdf-result/1", "stability": "preview"}]})
    }

    fn query(name: &str) -> Value {
        json!({"name": name, "key": "tag:", "example": "tag:-101F1", "description": "", "output_version": 1,
            "output": {"type": "object", "properties": {"tag": {"type": "string"}}, "required": ["tag"]}})
    }

    fn wire_view(second: &str) -> Value {
        json!({"api_status": "preview", "views": [{"name": "api_v1_wire", "row": "wire", "preview": false,
        "columns": [
            {"name": "id", "type": "TEXT", "doc": "the wire UUID", "preview": false},
            {"name": second, "type": "TEXT", "doc": "the printed wire number", "preview": false}
        ]}]})
    }

    const NOTES: &str = "# Unreleased changes\n\n- api-break: api_v1 api_v1_wire.number (an earlier break)\n";

    /// The base state of the five snapshots and the release notes.
    fn base_state(s: &Scratch) {
        s.write_json(
            "crates/ew-commands/schemas/index.json",
            &json!({"commands": [command_entry("a.set"), command_entry("b.drop")]}),
        );
        s.write_json("crates/ew-commands/schemas/a.set.schema.json", &params(&[]));
        s.write_json("crates/ew-commands/schemas/b.drop.schema.json", &params(&[]));
        s.write_json(
            CLI,
            &cli_snapshot(json!([{"name": "--out", "value": true, "repeatable": false}])),
        );
        s.write_json(
            "bin/ew/schemas/index.json",
            &json!({"results": [{"command": "export pdf", "name": "ew.export-pdf-result/1",
                "schema": "ew.export-pdf-result-1.schema.json"}]}),
        );
        s.write_json(
            "bin/ew/schemas/ew.export-pdf-result-1.schema.json",
            &json!({"title": "ew.export-pdf-result/1", "type": "object",
                "properties": {"pages": {"type": "integer"}}, "required": ["pages"]}),
        );
        s.write_json(
            QUERIES,
            &json!({"$schema": "ew.queries-result/1", "queries": [query("device_tag")]}),
        );
        s.write_json(API_V1, &wire_view("number"));
        s.write(RELEASE_NOTES, NOTES);
    }

    /// AG-2: in a scratch repository with two commits of the five snapshots, a removed command is breaking, a renamed
    /// `api_v1_wire` column is the old column breaking and the new one additive, a new flag and a new query are
    /// additive, and a property newly required in a command's parameters is breaking (the orchestrator's ruling 4,
    /// cycle 42: "a property newly required in a command's parameter schema (inputs)" breaks). Without the release-note
    /// lines the guard fails, listing the lines to add.
    #[test]
    fn the_guard_classifies_removals_renames_and_additions() {
        let s = Scratch::new("classify");
        base_state(&s);
        let base = s.commit("base");
        s.write_json(
            "crates/ew-commands/schemas/index.json",
            &json!({"commands": [command_entry("a.set")]}),
        );
        s.remove_file("crates/ew-commands/schemas/b.drop.schema.json");
        s.write_json("crates/ew-commands/schemas/a.set.schema.json", &params(&["x"]));
        s.write_json(
            CLI,
            &cli_snapshot(json!([
                {"name": "--out", "value": true, "repeatable": false},
                {"name": "--json", "value": false, "repeatable": false}
            ])),
        );
        s.write_json(
            QUERIES,
            &json!({"$schema": "ew.queries-result/1", "queries": [query("cell_of"), query("device_tag")]}),
        );
        s.write_json(API_V1, &wire_view("wire_number"));
        s.commit("change");
        let report = guard(&s.dir, &base, false, None);
        s.remove();
        assert_eq!(
            report.unwrap(),
            Report {
                lines: vec![
                    "breaking: commands a.set.x: newly required".to_owned(),
                    "breaking: commands b.drop: removed".to_owned(),
                    "additive: cli export pdf --json: added".to_owned(),
                    "additive: queries cell_of: added".to_owned(),
                    "breaking: api_v1 api_v1_wire.number: removed".to_owned(),
                    "additive: api_v1 api_v1_wire.wire_number: added".to_owned(),
                    "api-guard: 3 breaking, 3 additive".to_owned(),
                ],
                failure: vec![
                    "api-guard: 3 breaking change(s) without a release-note line in docs/release-notes/unreleased.md; \
                     add for each: - api-break: <surface> <item>"
                        .to_owned(),
                    "- api-break: commands a.set.x".to_owned(),
                    "- api-break: commands b.drop".to_owned(),
                    "- api-break: api_v1 api_v1_wire.number".to_owned(),
                ],
            }
        );
    }

    /// The rename of AG-3 between two commits, with `notes` as the release notes of the second.
    fn rename_report(
        name: &str,
        notes_before: &str,
        notes_after: &str,
        frozen: bool,
        event: Option<&Value>,
    ) -> Report {
        let s = Scratch::new(name);
        base_state(&s);
        s.write(RELEASE_NOTES, notes_before);
        let base = s.commit("base");
        s.write_json(API_V1, &wire_view("wire_number"));
        s.write(RELEASE_NOTES, notes_after);
        s.commit("rename");
        let report = guard(&s.dir, &base, frozen, event);
        s.remove();
        report.unwrap()
    }

    const RENAME_LINES: [&str; 3] = [
        "breaking: api_v1 api_v1_wire.number: removed",
        "additive: api_v1 api_v1_wire.wire_number: added",
        "api-guard: 1 breaking, 1 additive",
    ];

    fn rename_failure() -> Vec<String> {
        vec![
            "api-guard: 1 breaking change(s) without a release-note line in docs/release-notes/unreleased.md; add for \
             each: - api-break: <surface> <item>"
                .to_owned(),
            "- api-break: api_v1 api_v1_wire.number".to_owned(),
        ]
    }

    const NOTES_WITHOUT: &str = "# Unreleased changes\n";
    const NOTES_WITH: &str = "# Unreleased changes\n\n- api-break: api_v1 api_v1_wire.number (renamed wire_number; WP-S1.22 D14)\n";

    /// AG-3: the rename fails with the exact text without the release-note line, passes once the second commit adds
    /// `- api-break: api_v1 api_v1_wire.number`, and a line already present at the base does not cover it (the
    /// orchestrator's ruling 5, cycle 42: a break is covered by a line "added in the diff").
    #[test]
    fn a_break_needs_a_release_note_line_added_in_the_diff() {
        let lines: Vec<String> = RENAME_LINES.map(str::to_owned).to_vec();
        assert_eq!(
            rename_report("without", NOTES_WITHOUT, NOTES_WITHOUT, false, None),
            Report {
                lines: lines.clone(),
                failure: rename_failure()
            }
        );
        assert_eq!(
            rename_report("with", NOTES_WITHOUT, NOTES_WITH, false, None),
            Report {
                lines: lines.clone(),
                failure: vec![]
            }
        );
        assert_eq!(
            rename_report("present", NOTES, NOTES, false, None),
            Report {
                lines,
                failure: rename_failure()
            }
        );
    }

    /// AG-4: with the freeze (a parameter standing in for `FROZEN`), the break also needs the pull request's label
    /// `api-break` and a `docs/adr/` reference, read from fixture event files.
    #[test]
    fn after_the_freeze_a_break_needs_the_label_and_an_adr() {
        let dir = std::env::temp_dir().join(format!("ew-xtask-api-guard-events-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let event = |name: &str, value: Value| {
            let path = dir.join(name);
            std::fs::write(&path, value.to_string()).unwrap();
            read_event(Some(&path)).unwrap().unwrap()
        };
        let labelled = |labels: &[&str], body: &str| {
            json!({"pull_request": {"labels": labels.iter().map(|l| json!({"name": l})).collect::<Vec<_>>(),
                "body": body}})
        };
        let no_label = event("no-label.json", labelled(&["docs"], "See docs/adr/0042-x.md."));
        let no_adr = event("no-adr.json", labelled(&["api-break"], "Renames a column."));
        let both = event(
            "both.json",
            labelled(&["docs", "api-break"], "See docs/adr/0042-x.md."),
        );
        let push = event("push.json", json!({"before": "0", "after": "1"}));
        let missing = read_event(None).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(missing.is_none());

        let lines: Vec<String> = RENAME_LINES.map(str::to_owned).to_vec();
        let failure = |f: &[&str]| -> Report {
            Report {
                lines: lines.clone(),
                failure: f.iter().map(|l| (*l).to_owned()).collect(),
            }
        };
        let run = |event: Option<&Value>| rename_report("frozen", NOTES_WITHOUT, NOTES_WITH, true, event);
        assert_eq!(
            run(Some(&no_label)),
            failure(&[
                "api-guard: a breaking change after the freeze needs the label api-break on its pull request"
            ])
        );
        assert_eq!(
            run(Some(&no_adr)),
            failure(&[
                "api-guard: a breaking change after the freeze needs an ADR reference (docs/adr/...) in its pull \
                 request's description"
            ])
        );
        assert_eq!(run(Some(&both)), failure(&[]));
        let no_pr = "api-guard: a breaking change after the freeze needs a pull request labelled api-break whose \
                     description references an ADR (docs/adr/...)";
        assert_eq!(run(Some(&push)), failure(&[no_pr]));
        assert_eq!(run(None), failure(&[no_pr]));
        // Without the release-note line both failures are reported.
        let mut both_missing = rename_failure();
        both_missing.push(no_pr.to_owned());
        assert_eq!(
            rename_report("frozen-bare", NOTES_WITHOUT, NOTES_WITHOUT, true, None).failure,
            both_missing
        );
    }

    /// The release-note grammar (the orchestrator's ruling 5, cycle 42: a line "starts `- api-break: <surface> <item>`
    /// and continues with the end of the line, a space or \" (\""): an item that is a word prefix of the line's item
    /// is covered too (the CLI's items hold spaces), an item that merely shares characters is not.
    #[test]
    fn a_release_note_line_covers_its_item_only() {
        let added = [
            "- api-break: cli lib list --json (prints an object)",
            "- api-break: api_v1 api_v1_wire.numberx",
        ];
        assert!(covered("cli", "lib list --json", &added));
        assert!(covered("cli", "lib list", &added));
        assert!(covered("cli", "lib", &added));
        assert!(!covered("cli", "li", &added));
        assert!(!covered("cli", "lib list --js", &added));
        assert!(!covered("api_v1", "api_v1_wire.number", &added));
        assert!(covered("api_v1", "api_v1_wire.numberx", &added));
        assert!(covered(
            "api_v1",
            "api_v1_wire.numberx",
            &["- api-break: api_v1 api_v1_wire.numberx   "]
        ));
        assert!(!covered(
            "api_v1",
            "api_v1_wire.numberx",
            &["  - api-break: api_v1 api_v1_wire.numberx"]
        ));
    }

    /// The CLI's breaks beyond a removal: a flag that takes a value or no longer repeats, an exit code's meaning, a
    /// changed result; and the additive counterparts.
    #[test]
    fn cli_flags_exits_and_results_are_classified() {
        let entry = |flags: Value, exits: Value, result: Value| {
            let mut s = Snapshot::default();
            s.cli.insert(
                "lib list".to_owned(),
                json!({"path": "lib list", "flags": flags, "exits": exits, "result": result, "stability": "preview"}),
            );
            s
        };
        let before = entry(
            json!([{"name": "--company", "value": true, "repeatable": true}, {"name": "--json", "value": false, "repeatable": false}]),
            json!([{"code": 0, "meaning": "success"}, {"code": 2, "meaning": "usage"}]),
            json!("ew.lib-list-result/1"),
        );
        let after = entry(
            json!([{"name": "--company", "value": true, "repeatable": false}, {"name": "--json", "value": true, "repeatable": true}]),
            json!([{"code": 0, "meaning": "listed"}, {"code": 1, "meaning": "none"}]),
            json!("ew.lib-list-result/2"),
        );
        let lines: Vec<String> = classify(&before, &after).iter().map(Change::line).collect();
        assert_eq!(
            lines,
            [
                "breaking: cli lib list --company: no longer repeatable",
                "breaking: cli lib list --json: now takes a value",
                "additive: cli lib list --json: now repeatable",
                "breaking: cli lib list exit 0: meaning changed from \"success\" to \"listed\"",
                "breaking: cli lib list exit 2: removed",
                "additive: cli lib list exit 1: added",
                "breaking: cli lib list: result changed from ew.lib-list-result/1 to ew.lib-list-result/2",
            ]
        );
        let none = entry(json!([]), json!([]), Value::Null);
        let lines: Vec<String> = classify(&before, &none).iter().map(Change::line).collect();
        assert_eq!(
            lines,
            [
                "breaking: cli lib list --company: removed",
                "breaking: cli lib list --json: removed",
                "breaking: cli lib list exit 0: removed",
                "breaking: cli lib list exit 2: removed",
                "breaking: cli lib list: result ew.lib-list-result/1 removed",
            ]
        );
    }

    /// Schema properties: through `properties`, `items` and `$defs`; a removed or retyped property breaks a result, a
    /// newly required one does not; a lowered version breaks; a changed column type breaks.
    #[test]
    fn schema_properties_versions_and_column_types_are_classified() {
        let mut before = Snapshot::default();
        let mut after = Snapshot::default();
        before.results.insert(
            "ew.x-result/1".to_owned(),
            json!({"type": "object", "properties": {"rows": {"type": "array", "items": {"$ref": "#/$defs/Row"}},
                "n": {"type": "integer"}},
                "$defs": {"Row": {"type": "object", "properties": {"tag": {"type": "string"}, "gone": {"type": "string"}}}}}),
        );
        after.results.insert(
            "ew.x-result/1".to_owned(),
            json!({"type": "object", "properties": {"rows": {"type": "array", "items": {"$ref": "#/$defs/Row"}},
                "n": {"type": "string"}}, "required": ["n"],
                "$defs": {"Row": {"type": "object", "properties": {"tag": {"type": ["string", "null"]}}}}}),
        );
        before.queries.insert(
            "q".to_owned(),
            json!({"name": "q", "key": "-", "output_version": 2, "output": {}}),
        );
        after.queries.insert(
            "q".to_owned(),
            json!({"name": "q", "key": "tag:", "output_version": 1, "output": {}}),
        );
        let view =
            |ty: &str| json!({"name": "api_v1_x", "columns": [{"name": "n", "type": ty, "preview": false}]});
        before.views.insert("api_v1_x".to_owned(), view("TEXT"));
        after.views.insert("api_v1_x".to_owned(), view("INTEGER"));
        let lines: Vec<String> = classify(&before, &after).iter().map(Change::line).collect();
        assert_eq!(
            lines,
            [
                "breaking: results ew.x-result/1 Row.gone: removed",
                "breaking: results ew.x-result/1 Row.tag: type changed from string to string or null",
                "breaking: results ew.x-result/1 n: type changed from integer to string",
                "additive: results ew.x-result/1 n: newly required",
                "breaking: queries q: output_version lowered from 2 to 1",
                "additive: queries q: key changed from \"-\" to \"tag:\"",
                "breaking: api_v1 api_v1_x.n: type changed from TEXT to INTEGER",
            ]
        );
    }

    /// A new command property: optional is additive, required under an existing object breaks (ruling 4); a lowered
    /// `params_version` breaks.
    #[test]
    fn command_parameters_follow_ruling_4() {
        let snapshot = |schema: Value, version: u64| {
            let mut s = Snapshot::default();
            let mut entry = command_entry("a.set");
            entry["params_version"] = json!(version);
            s.commands.insert("a.set".to_owned(), (entry, schema));
            s
        };
        let before = snapshot(
            json!({"type": "object", "properties": {"x": {"type": "string"}}}),
            2,
        );
        let after = snapshot(
            json!({"type": "object", "properties": {"x": {"type": "string"}, "y": {"type": "string"},
                "z": {"type": "string"}}, "required": ["y"]}),
            1,
        );
        let lines: Vec<String> = classify(&before, &after).iter().map(Change::line).collect();
        assert_eq!(
            lines,
            [
                "breaking: commands a.set: params_version lowered from 2 to 1",
                "breaking: commands a.set.y: added as required",
                "additive: commands a.set.z: added",
            ]
        );
    }

    /// Hostile input: a deeply nested schema is followed only to the depth bound.
    #[test]
    fn nesting_is_bounded() {
        let mut node = json!({"type": "string"});
        for _ in 0..200 {
            node = json!({"type": "object", "properties": {"a": node}});
        }
        let shape = shape_of(&node);
        assert_eq!(shape.types.len(), MAX_DEPTH);
    }

    /// The released format of the `format` tests: an index of three kinds (one reserved), two schemas, and the frozen
    /// copy of a project with its expected folder.
    fn format_state(s: &Scratch) {
        s.write_json(
            "schemas/0.1/index.json",
            &json!({"format": "0.1", "kinds": [
                {"file": "devices.jsonl", "kind": "device", "rank": 30, "schema": "device.schema.json",
                    "status": "implemented", "v": 1},
                {"file": "project.jsonl", "kind": "medium", "rank": 14, "schema": null, "status": "reserved", "v": null},
                {"file": "pins.jsonl", "kind": "pin", "rank": 31, "schema": "pin.schema.json", "status": "implemented",
                    "v": 1}
            ]}),
        );
        s.write_json("schemas/0.1/device.schema.json", &device_schema(|_| {}));
        s.write_json(
            "schemas/0.1/pin.schema.json",
            &json!({"type": "object", "properties": {"n": {"type": "integer"}}}),
        );
        s.write(
            "fixtures/format/0.1/p.ew/manifest.json",
            "{\"format\": \"0.1\"}\n",
        );
        s.write(
            "fixtures/format/0.1/p.ew.expected/manifest.json",
            "{\"format\": \"0.1\"}\n",
        );
        s.write(RELEASE_NOTES, NOTES_WITHOUT);
    }

    /// The device schema of [`format_state`], changed by `edit`.
    fn device_schema(edit: impl FnOnce(&mut Value)) -> Value {
        let mut schema = json!({"type": "object", "required": ["tag"],
            "properties": {"tag": {"type": "string"}, "note": {"type": "string"}, "side": {"$ref": "#/$defs/Side"},
                "mode": {"type": "string", "enum": ["a", "b"]}},
            "$defs": {"Side": {"oneOf": [{"const": "left", "type": "string"}, {"const": "right", "type": "string"},
                {"type": "object", "properties": {"at": {"type": "integer"}}, "required": ["at"]}]}}});
        edit(&mut schema);
        schema
    }

    /// The guard of the second commit of `s` against its first, `base`.
    fn format_report(s: &Scratch, base: &str) -> Report {
        s.commit("change");
        guard(&s.dir, base, false, None).unwrap()
    }

    /// RL-6 (WP-S6.08 D9): without a final release tag (a candidate's `-rc` tag is none) the format is not released:
    /// the guard says so and passes, whatever changed.
    #[test]
    fn a_format_without_a_release_tag_is_not_compared() {
        let s = Scratch::new("format-unreleased");
        format_state(&s);
        let base = s.commit("base");
        s.git(&["tag", "v0.1.0-rc.1"]);
        s.remove_file("schemas/0.1/pin.schema.json");
        s.write("fixtures/format/0.1/p.ew/manifest.json", "{}\n");
        let report = format_report(&s, &base);
        s.remove();
        assert_eq!(
            report,
            Report {
                lines: vec![
                    "format 0.1: not released; nothing to compare".to_owned(),
                    "api-guard: 0 breaking, 0 additive".to_owned(),
                ],
                failure: vec![],
            }
        );
    }

    /// RL-6 (WP-S6.08 D9): against the release tag `v0.1.0` each breaking change of D9's list fails with S5, a new value
    /// without its `- format-variant:` line with S6 (with it, it passes), a changed fixture with S7; a new optional
    /// property, a reserved kind's new schema and a changed expected folder pass. The base is the oldest final tag.
    #[test]
    fn a_released_format_changes_only_additively() {
        let s = Scratch::new("format-released");
        format_state(&s);
        let base = s.commit("base");
        s.git(&["tag", "v0.1.0-rc.1"]);
        s.git(&["tag", "v0.1.0"]);
        s.write("README.md", "later\n");
        s.commit("later");
        s.git(&["tag", "v0.2.0"]);
        s.write_json(
            "schemas/0.1/index.json",
            &json!({"format": "0.2", "kinds": [
                {"file": "devices.jsonl", "kind": "device", "rank": 32, "schema": "device.schema.json",
                    "status": "implemented", "v": 1},
                {"file": "project.jsonl", "kind": "medium", "rank": 14, "schema": "medium.schema.json",
                    "status": "implemented", "v": 1}
            ]}),
        );
        s.write_json("schemas/0.1/medium.schema.json", &json!({"type": "object"}));
        s.remove_file("schemas/0.1/pin.schema.json");
        s.write_json(
            "schemas/0.1/device.schema.json",
            &device_schema(|d| {
                let props = d["properties"].as_object_mut().unwrap();
                props.remove("note");
                props.insert("tag".to_owned(), json!({"type": "integer"}));
                props.insert("mode".to_owned(), json!({"type": "string", "enum": ["a", "c"]}));
                props.insert("extra".to_owned(), json!({"type": "string"}));
                props.insert("level".to_owned(), json!({"type": "integer"}));
                d["required"] = json!(["tag", "level"]);
                d["$defs"]["Side"]["oneOf"][1] = json!({"const": "up", "type": "string"});
            }),
        );
        s.write(
            "fixtures/format/0.1/p.ew/manifest.json",
            "{\"format\": \"0.2\"}\n",
        );
        s.write(
            "fixtures/format/0.1/p.ew.expected/manifest.json",
            "{\"format\": \"0.2\"}\n",
        );
        s.write(
            RELEASE_NOTES,
            "# Unreleased changes\n\n- format-variant: device Side up (a new side)\n",
        );
        let report = format_report(&s, &base);
        s.remove();
        let lines = [
            "breaking: format 0.1 device Side: value right removed",
            "additive: format 0.1 device Side: value up added",
            "breaking: format 0.1 device mode: value b removed",
            "additive: format 0.1 device mode: value c added",
            "breaking: format 0.1 device note: removed",
            "breaking: format 0.1 device tag: type changed from string to integer",
            "additive: format 0.1 device extra: added",
            "breaking: format 0.1 device level: added as required",
            "breaking: format 0.1 index.json: format changed from \"0.1\" to \"0.2\"",
            "breaking: format 0.1 device: rank changed from 30 to 32",
            "additive: format 0.1 medium: v changed from null to 1",
            "additive: format 0.1 medium: schema changed from null to \"medium.schema.json\"",
            "additive: format 0.1 medium: status changed from \"reserved\" to \"implemented\"",
            "breaking: format 0.1 pin: removed from index.json",
            "breaking: format 0.1 pin: removed",
            "additive: format 0.1 medium: added",
            "breaking: format 0.1 fixtures/format/0.1/p.ew/manifest.json: changed",
            "api-guard: 10 breaking, 7 additive",
        ];
        let s5 = |item: &str, what: &str| {
            format!(
                "format 0.1: {item}: {what} since v0.1.0; a released format changes only additively (ADR-0011 amendment 6)"
            )
        };
        assert_eq!(
            report,
            Report {
                lines: lines.map(str::to_owned).to_vec(),
                failure: vec![
                    s5("device Side", "value right removed"),
                    s5("device mode", "value b removed"),
                    s5("device note", "removed"),
                    s5("device tag", "type changed from string to integer"),
                    s5("device level", "added as required"),
                    s5("index.json", "format changed from \"0.1\" to \"0.2\""),
                    s5("device", "rank changed from 30 to 32"),
                    s5("pin", "removed from index.json"),
                    s5("pin", "removed"),
                    "format 0.1: device mode: the new value c since v0.1.0 needs the line \"- format-variant: device \
                     mode c\" in docs/release-notes/unreleased.md"
                        .to_owned(),
                    "format 0.1: fixtures/format/0.1/p.ew/manifest.json changed since v0.1.0; a released format's \
                     fixture never changes"
                        .to_owned(),
                ],
            }
        );
    }

    /// RL-6 (WP-S6.08 D9): a new value with its line, a new optional property and a changed expected folder pass.
    #[test]
    fn a_new_value_with_its_line_and_a_new_optional_property_pass() {
        let s = Scratch::new("format-additive");
        format_state(&s);
        let base = s.commit("base");
        s.git(&["tag", "v0.1.0"]);
        s.write_json(
            "schemas/0.1/device.schema.json",
            &device_schema(|d| {
                let props = d["properties"].as_object_mut().unwrap();
                props.insert(
                    "mode".to_owned(),
                    json!({"type": "string", "enum": ["a", "b", "c"]}),
                );
                props.insert("extra".to_owned(), json!({"type": "string"}));
            }),
        );
        s.write("fixtures/format/0.1/p.ew.expected/manifest.json", "{}\n");
        s.write(
            RELEASE_NOTES,
            "# Unreleased changes\n\n- format-variant: device mode c (FT-0, a third mode)\n",
        );
        let report = format_report(&s, &base);
        s.remove();
        assert_eq!(
            report,
            Report {
                lines: [
                    "additive: format 0.1 device mode: value c added",
                    "additive: format 0.1 device extra: added",
                    "api-guard: 0 breaking, 2 additive",
                ]
                .map(str::to_owned)
                .to_vec(),
                failure: vec![],
            }
        );
    }
}
