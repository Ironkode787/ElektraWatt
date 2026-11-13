// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `api-guard`: the guard of the published file-format surface (ADR-0011 amendment 6).
//!
//! Each format folder `schemas/<f>/` is compared with its base, the oldest final release tag (`v<X.Y.Z>`, no `-rc`)
//! whose tree holds `schemas/<f>/index.json`; without one the format is not released and the guard prints `format <f>:
//! not released; nothing to compare`. The items are `index.json`, `<kind>` (a schema file or an index entry) and
//! `<kind> <path>` (a property path through `properties` as `a.b` and through `items` as `a[]`, from the top level and
//! from each `$defs` entry by its name). Description texts are not compared.
//!
//! Against the base, **breaking**, which always fails with its own line: a schema file or a kind removed; a property
//! removed, its type changed or newly required; an `enum` value, a `const` or a `oneOf`/`anyOf` branch removed; a
//! constraint keyword of a node (`format`, `minimum`, `exclusiveMinimum`, `maximum`, `exclusiveMaximum`, `minLength`,
//! `maxLength`, `minItems`, `maxItems`, `pattern`, `additionalProperties`, `uniqueItems`, `minProperties`,
//! `maxProperties`, `propertyNames`) added, removed or changed, also on a `oneOf`/`anyOf` branch (the item
//! `<kind> <path>|<branch>`), and an `enum` or `const` given to a node that had none; a kind's `file`, `rank`, `v` or
//! `schema` changed (a reserved kind that gains them is additive), or the index's `format`; any file of
//! `fixtures/format/<f>/<project>/` changed, added or removed (the `<project>.expected/` folders may change). The walk
//! also enters `prefixItems` (`a[0]`), a map's value schema (`additionalProperties` as a schema, `a{}`) and its key
//! schema (`propertyNames`, `a{key}`), and sees through a `$defs` definition renamed with its content unchanged. A new
//! value or branch of a node the base holds passes only when `docs/release-notes/unreleased.md` gains `- format-variant:
//! <kind> <path> <value>` (the reviewer checks the type's `Unknown(Value)` arm, ADR-0011 amendment 1); this line check
//! compares with the **newest** final tag whose tree holds the format, among those whose version is not above the
//! working tree's `[workspace.package].version`, since the promote empties the notes. A new optional property and a
//! new kind's schema are additive.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// The release notes of the next release.
const RELEASE_NOTES: &str = "docs/release-notes/unreleased.md";
/// The published schemas of the file format, one folder per format.
const SCHEMAS_DIR: &str = "schemas";
/// The frozen copies of each released format's projects (ADR-0011 amendment 6).
const FIXTURES_DIR: &str = "fixtures/format";
/// The sixth surface's name.
const FORMAT_SURFACE: &str = "format";
/// The nesting depth up to which property paths are followed (a bound on hostile input).
const MAX_DEPTH: usize = 32;

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

/// `api-guard`. The base of each format is its own release tag.
pub(crate) fn run(args: &[String], root: &Path) -> Result<bool> {
    if !args.is_empty() {
        bail!("usage: cargo xtask api-guard");
    }
    let report = guard(root)?;
    for line in &report.lines {
        println!("{line}");
    }
    for line in &report.failure {
        eprintln!("{line}");
    }
    Ok(report.failure.is_empty())
}

/// The guard of the working tree at `root` against the release tags.
fn guard(root: &Path) -> Result<Report> {
    let notes_after = std::fs::read_to_string(root.join(RELEASE_NOTES))
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let format = format_guard(root, &notes_after)?;
    let mut lines: Vec<String> = format.changes.iter().map(Change::line).collect();
    let breaking = format.changes.iter().filter(|c| c.breaking).count();
    lines.push(format!(
        "api-guard: {breaking} breaking, {} additive",
        format.changes.len() - breaking
    ));
    let summary = lines.len() - 1;
    lines.splice(summary..summary, format.notes);
    Ok(Report {
        lines,
        failure: format.failure,
    })
}

// ---------------------------------------------------------------------------------------------------------
// Reading the snapshots

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

// ---------------------------------------------------------------------------------------------------------
// Classification

/// A compact JSON text of a value.
fn compact(v: Option<&Value>) -> String {
    v.map_or_else(|| "nothing".to_owned(), Value::to_string)
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

/// The path that holds `path`: `a.b` → `a`, `a[]` → `a`, `a` → `""`.
fn parent(path: &str) -> &str {
    if let Some(p) = path.strip_suffix("[]") {
        return p;
    }
    path.rsplit_once('.').map_or("", |(p, _)| p)
}

// ---------------------------------------------------------------------------------------------------------
// The format surface

/// The guard's lines and failures of the `format` surface.
#[derive(Default)]
struct FormatReport {
    /// One classified change per item, as the other surfaces'.
    changes: Vec<Change>,
    /// A format that is not released: "nothing to compare".
    notes: Vec<String>,
    /// The failures, one line each.
    failure: Vec<String>,
}

/// The `format` surface: each format folder `schemas/<f>/` of the working tree or of a release, against its base, the
/// oldest final release tag whose tree holds `schemas/<f>/index.json`, with `notes_after` the working tree's release
/// notes. Without a base the format is not released and nothing is compared. The line check of a new value compares
/// with the newest such tag whose version is not above the working tree's.
fn format_guard(root: &Path, notes_after: &str) -> Result<FormatReport> {
    let mut out = FormatReport::default();
    let mut base: BTreeMap<String, String> = BTreeMap::new();
    let mut line_base: BTreeMap<String, String> = BTreeMap::new();
    let tree = tree_version(root);
    for tag in final_tags(root)? {
        let listed = crate::repo::git(root, &["ls-tree", "-r", "--name-only", &tag, "--", SCHEMAS_DIR])?;
        for f in listed.lines().filter_map(|p| {
            p.strip_prefix(SCHEMAS_DIR)?
                .strip_prefix('/')?
                .strip_suffix("/index.json")
        }) {
            base.entry(f.to_owned()).or_insert_with(|| tag.clone());
            if tree.is_none_or(|v| tag_version(&tag).is_some_and(|t| t <= v)) {
                line_base.insert(f.to_owned(), tag.clone());
            }
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
            Some(tag) => {
                let line_tag = line_base.get(&f).unwrap_or(tag);
                compare_format(root, &f, (tag, line_tag), notes_after, &mut out)?;
            }
        }
    }
    Ok(out)
}

/// The final release tags `v<X.Y.Z>` (no `-rc` or other suffix), oldest version first.
fn final_tags(root: &Path) -> Result<Vec<String>> {
    let listed = crate::repo::git(root, &["tag", "--list", "v*"])?;
    let mut tags: Vec<((u64, u64, u64), String)> = listed
        .lines()
        .filter_map(|t| Some((tag_version(t)?, t.to_owned())))
        .collect();
    tags.sort();
    Ok(tags.into_iter().map(|(_, t)| t).collect())
}

/// The version of a final release tag `v<X.Y.Z>`; `None` for any other name.
fn tag_version(tag: &str) -> Option<(u64, u64, u64)> {
    version_of(tag.strip_prefix('v')?)
}

/// `X.Y.Z` as numbers; `None` for anything else (a pre-release suffix included).
fn version_of(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.split('.');
    let mut number = || -> Option<u64> {
        let p = parts.next()?;
        (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())).then(|| p.parse().ok())?
    };
    let version = (number()?, number()?, number()?);
    parts.next().is_none().then_some(version)
}

/// The working tree's `[workspace.package].version` from its `Cargo.toml`; `None` without one that reads.
fn tree_version(root: &Path) -> Option<(u64, u64, u64)> {
    let text = std::fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let doc: toml::Table = toml::from_str(&text).ok()?;
    let version = doc.get("workspace")?.get("package")?.get("version")?.as_str()?;
    version_of(version)
}

/// The schemas of the format folder `dir` and the release notes at `tag`.
fn format_blobs(root: &Path, dir: &str, tag: &str) -> Result<(Files, String)> {
    let listed = crate::repo::git(root, &["ls-tree", "-r", "--name-only", tag, "--", dir])?;
    let paths: Vec<&str> = listed
        .lines()
        .filter(|p| {
            p.strip_prefix(dir)
                .and_then(|rest| rest.strip_prefix('/'))
                .is_some_and(|name| !name.contains('/') && name.ends_with(".json"))
        })
        .chain([RELEASE_NOTES])
        .collect();
    let mut blobs = read_blobs(root, tag, &paths)?;
    let notes = blobs.remove(RELEASE_NOTES).unwrap_or_default();
    Ok((blobs, notes))
}

/// The format `f` of the working tree against its release `tag`: the index, every schema, the frozen fixture; a new
/// value against `line_tag` and its notes.
fn compare_format(
    root: &Path,
    f: &str,
    (tag, line_tag): (&str, &str),
    notes_after: &str,
    out: &mut FormatReport,
) -> Result<()> {
    let dir = format!("{SCHEMAS_DIR}/{f}");
    let (before, mut notes_before) = format_blobs(root, &dir, tag)?;
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
        } else if line_tag == tag {
            node_changes(item, &b, &a, &mut changes, &mut variants);
        } else {
            // With a newer line base, the values new since the oldest base are reported in `changes` only; S6 reads
            // the line base's below.
            node_changes(item, &b, &a, &mut changes, &mut Vec::new());
        }
    }
    if line_tag != tag {
        let (line_before, line_notes) = format_blobs(root, &dir, line_tag)?;
        notes_before = line_notes;
        for (path, text) in line_before.iter().filter(|(p, _)| !p.ends_with("/index.json")) {
            let name = &path[dir.len() + 1..];
            let item = name.strip_suffix(".schema.json").unwrap_or(name);
            let Ok(after) = std::fs::read_to_string(root.join(path)) else {
                continue;
            };
            let (b, a) = (
                json(text, path, line_tag)?,
                json(&after, path, "the working tree")?,
            );
            node_changes(item, &b, &a, &mut Vec::new(), &mut variants);
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
                "format {f}: {item}: the new value {value} since {line_tag} needs the line \"{line}\" in {RELEASE_NOTES}"
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

/// One node of a format schema: its type, whether it is required, its values (`enum` values, `const`, and the `oneOf`
/// and `anyOf` branches), its constraint keywords and whether it has an `enum` or a `const`.
#[derive(Default)]
struct FormatNode {
    ty: Option<String>,
    required: bool,
    values: BTreeSet<String>,
    constraints: BTreeMap<&'static str, String>,
    enumerated: Option<&'static str>,
    /// A `oneOf` or `anyOf` branch's own constraints, at `<path>|<branch>`: the branch itself is the
    /// node's value, so only its constraints are compared here.
    branch: bool,
}

/// The constraint keywords the format guard compares: any change to one breaks.
const CONSTRAINTS: [&str; 15] = [
    "format",
    "minimum",
    "exclusiveMinimum",
    "maximum",
    "exclusiveMaximum",
    "minLength",
    "maxLength",
    "minItems",
    "maxItems",
    "pattern",
    "additionalProperties",
    "uniqueItems",
    "minProperties",
    "maxProperties",
    "propertyNames",
];

/// Notes the [`CONSTRAINTS`] keywords of `node` in `entry`, a schema as `a schema` (its content is walked).
fn note_constraints(node: &Value, entry: &mut FormatNode) {
    for key in CONSTRAINTS {
        if let Some(v) = node.get(key) {
            let word = if v.is_object() {
                "a schema".to_owned()
            } else {
                compact(Some(v))
            };
            entry.constraints.insert(key, word);
        }
    }
}

/// `before` with each `$defs` definition that `after` holds under a new name, its content unchanged, renamed to that
/// name, every local `$ref` to it with it: a rename alone is no change.
fn renamed_defs(before: &Value, after: &Value) -> Value {
    fn rewrite(v: &mut Value, renames: &BTreeMap<String, String>) {
        match v {
            Value::Object(map) => {
                if let Some(Value::String(r)) = map.get_mut("$ref")
                    && let Some(new) = r.strip_prefix("#/$defs/").and_then(|n| renames.get(n))
                {
                    *r = format!("#/$defs/{new}");
                }
                map.values_mut().for_each(|child| rewrite(child, renames));
            }
            Value::Array(items) => items.iter_mut().for_each(|i| rewrite(i, renames)),
            _ => {}
        }
    }
    let mut out = before.clone();
    let (Some(old), Some(new)) = (
        before.get("$defs").and_then(Value::as_object),
        after.get("$defs").and_then(Value::as_object),
    ) else {
        return out;
    };
    let renames: BTreeMap<String, String> = old
        .iter()
        .filter(|(name, _)| !new.contains_key(*name))
        .filter_map(|(name, def)| {
            new.iter()
                .find(|(n, d)| !old.contains_key(*n) && *d == def)
                .map(|(n, _)| (name.clone(), n.clone()))
        })
        .collect();
    if renames.is_empty() {
        return out;
    }
    if let Some(defs) = out.get_mut("$defs").and_then(Value::as_object_mut) {
        for (from, to) in &renames {
            if let Some(def) = defs.remove(from) {
                defs.insert(to.clone(), def);
            }
        }
    }
    rewrite(&mut out, &renames);
    out
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
        entry.enumerated = Some("enum");
    }
    if let Some(c) = node.get("const") {
        entry.values.insert(value_word(c));
        entry.enumerated.get_or_insert("const");
    }
    note_constraints(node, entry);
    for key in ["oneOf", "anyOf"] {
        if let Some(branches) = node.get(key).and_then(Value::as_array) {
            nodes
                .entry(path.to_owned())
                .or_default()
                .values
                .extend(branches.iter().map(branch_word));
            // The members of an object branch are the node's own; its constraints are its own item (D29).
            for b in branches.iter().filter(|b| b.get("const").is_none()) {
                let branch = nodes.entry(format!("{path}|{}", branch_word(b))).or_default();
                branch.branch = true;
                note_constraints(b, branch);
                format_members(b, path, nodes, depth + 1);
            }
        }
    }
    format_members(node, path, nodes, depth);
}

/// The `properties`, `items`, `prefixItems` (`a[0]`), a map's value schema (`additionalProperties` as a schema, `a{}`)
/// and its key schema (`propertyNames`, `a{key}`, D29) of `node` at `path`.
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
    if let Some(prefix) = node.get("prefixItems").and_then(Value::as_array) {
        for (i, item) in prefix.iter().enumerate() {
            format_walk(item, &format!("{path}[{i}]"), nodes, depth + 1);
        }
    }
    if let Some(values) = node.get("additionalProperties").filter(|v| v.is_object()) {
        format_walk(values, &format!("{path}{{}}"), nodes, depth + 1);
    }
    if let Some(names) = node.get("propertyNames").filter(|v| v.is_object()) {
        format_walk(names, &format!("{path}{{key}}"), nodes, depth + 1);
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
    let (b, a) = (format_nodes(&renamed_defs(before, after)), format_nodes(after));
    let item = |path: &str| {
        if path.is_empty() {
            kind.to_owned()
        } else {
            format!("{kind} {path}")
        }
    };
    for (path, node) in &b {
        let Some(new) = a.get(path) else {
            // A branch removed is its node's value removed, reported there.
            if !node.branch {
                out.push((true, item(path), "removed".to_owned()));
            }
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
        // A constraint added, removed or changed, and a first `enum` or `const`, break either old files
        // or old readers.
        let keys: BTreeSet<&str> = node
            .constraints
            .keys()
            .chain(new.constraints.keys())
            .copied()
            .collect();
        for key in keys {
            let what = match (node.constraints.get(key), new.constraints.get(key)) {
                (Some(old), Some(now)) if old == now => continue,
                (Some(old), Some(now)) => format!("{key} changed from {old} to {now}"),
                (None, Some(now)) => format!("{key} {now} added"),
                (Some(old), None) => format!("{key} {old} removed"),
                (None, None) => continue,
            };
            out.push((true, item(path), what));
        }
        if let (None, Some(kind)) = (node.enumerated, new.enumerated) {
            out.push((true, item(path), format!("{kind} added")));
            continue;
        }
        for value in node.values.difference(&new.values) {
            out.push((true, item(path), format!("value {value} removed")));
        }
        for value in new.values.difference(&node.values) {
            out.push((false, item(path), format!("value {value} added")));
            variants.push((item(path), value.clone()));
        }
    }
    for (path, node) in a.iter().filter(|(p, n)| !b.contains_key(*p) && !n.branch) {
        // The one member of a new object branch (`{profinet}`, an open enum's new payload variant) comes with the
        // branch, a new value that needs its `format-variant` line; it is no member newly required of the old values.
        let name = path.rsplit('.').next().unwrap_or(path);
        let branch = format!("{}|{{{name}}}", parent(path));
        let new_variant = a.contains_key(&branch) && !b.contains_key(&branch);
        if node.required && b.contains_key(parent(path)) && !new_variant {
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

    const NOTES_WITHOUT: &str = "# Unreleased changes\n";

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
    fn format_report(s: &Scratch) -> Report {
        s.commit("change");
        guard(&s.dir).unwrap()
    }

    /// RL-6: without a final release tag (a candidate's `-rc` tag is none) the format is not released:
    /// the guard says so and passes, whatever changed.
    #[test]
    fn a_format_without_a_release_tag_is_not_compared() {
        let s = Scratch::new("format-unreleased");
        format_state(&s);
        s.commit("base");
        s.git(&["tag", "v0.1.0-rc.1"]);
        s.remove_file("schemas/0.1/pin.schema.json");
        s.write("fixtures/format/0.1/p.ew/manifest.json", "{}\n");
        let report = format_report(&s);
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

    /// RL-6: against the release tag `v0.1.0` each breaking change of D9's list fails with S5, a new value
    /// without its `- format-variant:` line with S6 (with it, it passes), a changed fixture with S7; a new optional
    /// property, a reserved kind's new schema and a changed expected folder pass. The base is the oldest final tag, the
    /// line check's the newest (D22).
    #[test]
    fn a_released_format_changes_only_additively() {
        let s = Scratch::new("format-released");
        format_state(&s);
        s.commit("base");
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
        let report = format_report(&s);
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
                    "format 0.1: device mode: the new value c since v0.2.0 needs the line \"- format-variant: device \
                     mode c\" in docs/release-notes/unreleased.md"
                        .to_owned(),
                    "format 0.1: fixtures/format/0.1/p.ew/manifest.json changed since v0.1.0; a released format's \
                     fixture never changes"
                        .to_owned(),
                ],
            }
        );
    }

    /// RL-6: a new value with its line, a new optional property and a changed expected folder pass.
    #[test]
    fn a_new_value_with_its_line_and_a_new_optional_property_pass() {
        let s = Scratch::new("format-additive");
        format_state(&s);
        s.commit("base");
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
        let report = format_report(&s);
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

    /// The line check reads the newest final tag not above the tree's version. A value added
    /// between `v0.1.0` and `v0.2.0` with its line, the notes emptied after the second promote, passes; one added after
    /// `v0.2.0` without its line fails with S6 naming `v0.2.0`. At version 0.1.1 (a `v0.1.1` cut on the release
    /// branch) the line base is `v0.1.0`, and both values need their lines.
    #[test]
    fn the_line_check_reads_the_newest_release_not_above_the_tree() {
        let s = Scratch::new("format-line-base");
        format_state(&s);
        s.write("Cargo.toml", "[workspace.package]\nversion = \"0.1.0\"\n");
        s.commit("base");
        s.git(&["tag", "v0.1.0"]);
        let modes = |values: Value| {
            device_schema(|d| {
                d["properties"]["mode"] = json!({"type": "string", "enum": values});
            })
        };
        s.write_json("schemas/0.1/device.schema.json", &modes(json!(["a", "b", "c"])));
        s.write(
            RELEASE_NOTES,
            "# Unreleased changes\n\n- format-variant: device mode c (a third mode)\n",
        );
        s.write("Cargo.toml", "[workspace.package]\nversion = \"0.2.0\"\n");
        s.commit("0.2.0");
        s.git(&["tag", "v0.2.0"]);
        // The promote empties the notes to their head (step 9).
        s.write(RELEASE_NOTES, NOTES_WITHOUT);
        s.write_json(
            "schemas/0.1/device.schema.json",
            &modes(json!(["a", "b", "c", "d"])),
        );
        let s6 = |value: &str, tag: &str| {
            format!(
                "format 0.1: device mode: the new value {value} since {tag} needs the line \"- format-variant: \
                 device mode {value}\" in docs/release-notes/unreleased.md"
            )
        };
        let report = format_report(&s);
        assert_eq!(report.failure, [s6("d", "v0.2.0")]);
        s.write("Cargo.toml", "[workspace.package]\nversion = \"0.1.1\"\n");
        let report = format_report(&s);
        s.remove();
        assert_eq!(report.failure, [s6("c", "v0.1.0"), s6("d", "v0.1.0")]);
    }

    /// A constraint added to a property, to a `prefixItems` item and to a map's value schema,
    /// and a first `enum`, each fail with S5; a `$defs` definition renamed with its content unchanged is no change.
    #[test]
    fn a_released_format_keeps_its_constraints() {
        let s = Scratch::new("format-constraints");
        format_state(&s);
        let with = |edit: &dyn Fn(&mut Value)| {
            device_schema(|d| {
                let props = d["properties"].as_object_mut().unwrap();
                props.insert(
                    "pos".to_owned(),
                    json!({"type": "array", "prefixItems": [{"type": "integer"}, {"type": "integer"}]}),
                );
                props.insert(
                    "labels".to_owned(),
                    json!({"type": "object", "additionalProperties": {"type": "string"}}),
                );
                edit(d);
            })
        };
        s.write_json("schemas/0.1/device.schema.json", &with(&|_| {}));
        s.commit("base");
        s.git(&["tag", "v0.1.0"]);
        s.write_json(
            "schemas/0.1/device.schema.json",
            &with(&|d| {
                d["properties"]["note"]["maxLength"] = json!(40);
                d["properties"]["tag"]["enum"] = json!(["x"]);
                d["properties"]["pos"]["prefixItems"][1]["minimum"] = json!(0);
                d["properties"]["labels"]["additionalProperties"]["pattern"] = json!("^[a-z]+$");
                let side = d["$defs"].as_object_mut().unwrap().remove("Side").unwrap();
                d["$defs"]["Edge"] = side;
                d["properties"]["side"] = json!({"$ref": "#/$defs/Edge"});
            }),
        );
        let report = format_report(&s);
        s.remove();
        let changes = [
            ("device labels{}", "pattern \"^[a-z]+$\" added"),
            ("device note", "maxLength 40 added"),
            ("device pos[1]", "minimum 0 added"),
            ("device tag", "enum added"),
        ];
        let mut lines: Vec<String> = changes
            .iter()
            .map(|(item, what)| format!("breaking: format 0.1 {item}: {what}"))
            .collect();
        lines.push("api-guard: 4 breaking, 0 additive".to_owned());
        assert_eq!(
            report,
            Report {
                lines,
                failure: changes
                    .iter()
                    .map(|(item, what)| {
                        format!(
                            "format 0.1: {item}: {what} since v0.1.0; a released format changes only additively \
                             (ADR-0011 amendment 6)"
                        )
                    })
                    .collect(),
            }
        );
    }

    /// A constraint of a `oneOf` branch (`additionalProperties`), of an `anyOf`
    /// branch (`minProperties` changed, `maxProperties` added) and of a map's key schema (`propertyNames`' `pattern`)
    /// each fail with S5 under its own item.
    #[test]
    fn a_released_format_keeps_its_branch_and_key_constraints() {
        let s = Scratch::new("format-branch-constraints");
        format_state(&s);
        let with = |edit: &dyn Fn(&mut Value)| {
            device_schema(|d| {
                d["$defs"]["Text"] = json!({"anyOf": [{"type": "string"}, {"type": "object",
                    "additionalProperties": {"type": "string"}, "minProperties": 1,
                    "propertyNames": {"pattern": "^[a-z]+$"}}]});
                d["properties"]["label"] = json!({"$ref": "#/$defs/Text"});
                edit(d);
            })
        };
        s.write_json("schemas/0.1/device.schema.json", &with(&|_| {}));
        s.commit("base");
        s.git(&["tag", "v0.1.0"]);
        s.write_json(
            "schemas/0.1/device.schema.json",
            &with(&|d| {
                d["$defs"]["Side"]["oneOf"][2]["additionalProperties"] = json!(false);
                let object = &mut d["$defs"]["Text"]["anyOf"][1];
                object["minProperties"] = json!(2);
                object["maxProperties"] = json!(9);
                object["propertyNames"]["pattern"] = json!("^[a-z]{2}$");
            }),
        );
        let report = format_report(&s);
        s.remove();
        let changes = [
            ("device Side|{at}", "additionalProperties false added"),
            (
                "device Text{key}",
                "pattern changed from \"^[a-z]+$\" to \"^[a-z]{2}$\"",
            ),
            ("device Text|object", "maxProperties 9 added"),
            ("device Text|object", "minProperties changed from 1 to 2"),
        ];
        let mut lines: Vec<String> = changes
            .iter()
            .map(|(item, what)| format!("breaking: format 0.1 {item}: {what}"))
            .collect();
        lines.push("api-guard: 4 breaking, 0 additive".to_owned());
        assert_eq!(
            report,
            Report {
                lines,
                failure: changes
                    .iter()
                    .map(|(item, what)| {
                        format!(
                            "format 0.1: {item}: {what} since v0.1.0; a released format changes only additively \
                             (ADR-0011 amendment 6)"
                        )
                    })
                    .collect(),
            }
        );
    }

    /// A new payload variant of an open enum (a single-member object branch) adds its member as required only within
    /// its own branch: a new value with its line, not a member the old values must now carry.
    #[test]
    fn a_new_payload_variant_is_a_new_value_not_a_required_member() {
        let node = |branches: Value| json!({"$defs": {"Net": {"oneOf": branches}}, "properties": {"a": {"$ref": "#/$defs/Net"}}});
        let before =
            node(json!([{"type": "object", "properties": {"p": {"type": "integer"}}, "required": ["p"]}]));
        let after = node(json!([
            {"type": "object", "properties": {"p": {"type": "integer"}}, "required": ["p"]},
            {"type": "object", "properties": {"n": {"type": "string"}}, "required": ["n"]},
        ]));
        let (mut out, mut variants) = (Vec::new(), Vec::new());
        node_changes("k", &before, &after, &mut out, &mut variants);
        assert!(out.iter().all(|(breaking, _, _)| !breaking), "{out:?}");
        assert_eq!(variants.len(), 1, "{variants:?}");
    }
}
