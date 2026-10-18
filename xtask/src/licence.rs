// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-licence-map`: the licence-map and licence-boundary check of docs/design/19-open-source-governance.md
//! §5.1 (rules LR-01 … LR-04, NFR-LIC-01, NFR-LIC-02, NFR-LIC-10, NFR-MAINT-02), driven by `ci/licence-map.toml`.
//!
//! The check runs in four parts over plain inputs, so the unit tests can seed violations on synthetic data:
//! - [`check_policy`]: the map itself is consistent (valid SPDX expressions, GPL-2.0 only in its one row);
//! - [`check_files`]: every file maps to a rule, its licence expression (SPDX header or `REUSE.toml`
//!   annotation) is allowed there, its copyright line is the collective notice, and GPL-2.0 appears only where
//!   the map permits it;
//! - [`check_crates`]: each workspace crate's `license` field equals the expression of its row;
//! - [`check_boundary`]: the full dependency closure of every permissive crate has only allowlisted licences.
//!
//! The rule that only `ew-api` depends on `ew-import-qet`, and that neither `ew-plugin-host` nor a permissive
//! crate reaches it (LR-04), is the `[[confined]]` rule of `ci/dep-rules.toml`, checked by `check-deps`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The header tag that carries a file's licence expression (split so that REUSE does not read this source file
/// as declaring a licence).
const LICENCE_TAG: &str = concat!("SPDX-", "License-Identifier:");
/// The header tag that carries a copyright notice.
const COPYRIGHT_TAG: &str = concat!("SPDX-", "FileCopyrightText:");
/// How many lines from the top of a file count as its header.
const HEADER_LINES: usize = 10;
/// The holder of the collective copyright notice (NFR-LIC-10).
const COLLECTIVE_HOLDER: &str = "The ElektraWatt contributors";

/// The policy file `ci/licence-map.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenceMap {
    /// The path rules; the longest matching glob wins.
    pub path: Vec<PathRule>,
    /// Where GPL-2.0 may appear.
    pub gpl2: Gpl2,
    /// Where third-party copyright notices may appear.
    pub notices: Notices,
    /// The licence allowlist of the permissive layer's dependency closure.
    pub boundary: Boundary,
}

/// One row of the map.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathRule {
    /// REUSE-style glob (`*` stops at `/`, `**` does not).
    pub glob: String,
    /// The SPDX expressions a file under this glob may carry.
    pub allowed: Vec<String>,
    /// The glob covers a workspace crate whose `license` field must equal the single allowed expression.
    #[serde(default, rename = "crate")]
    pub krate: bool,
}

/// LR-04: `GPL-2.0` appears only in files matching `only_in`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gpl2 {
    /// Glob of the ported-QET directory.
    pub only_in: String,
}

/// NFR-LIC-10 and 19 §2.3: files that keep third-party copyright notices above the collective one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notices {
    /// Globs of the files that may carry copyright lines other than the collective notice.
    pub third_party: Vec<String>,
}

/// LR-03 and NFR-MAINT-02: the licences allowed in the permissive layer's dependency closure.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    /// SPDX licence identifiers, each optionally `WITH` an exception.
    pub allow: Vec<String>,
}

impl LicenceMap {
    /// Reads and parses the map.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// Parses map TOML.
    pub fn parse(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    /// The rule that governs `path`: the longest matching glob. `Err` names the tied globs when two rules of
    /// the same length with different allowed sets match.
    pub fn rule_for(&self, path: &str) -> Option<Result<&PathRule, String>> {
        let mut best: Vec<&PathRule> = Vec::new();
        for rule in &self.path {
            if !glob_match(&rule.glob, path) {
                continue;
            }
            let len = rule.glob.chars().count();
            match best.first().map(|b| b.glob.chars().count()) {
                Some(best_len) if len < best_len => {}
                Some(best_len) if len == best_len => best.push(rule),
                _ => best = vec![rule],
            }
        }
        let first = *best.first()?;
        if best
            .iter()
            .any(|r| r.allowed != first.allowed || r.krate != first.krate)
        {
            let globs: Vec<&str> = best.iter().map(|r| r.glob.as_str()).collect();
            return Some(Err(globs.join("`, `")));
        }
        Some(Ok(first))
    }
}

/// REUSE glob matching (REUSE specification 3.3): `*` matches any run of characters except `/`, `**` any run
/// including `/`, `\` escapes the next character; everything else matches literally.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    enum Tok {
        Lit(char),
        Star,
        DoubleStar,
    }
    let mut toks = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => toks.push(Tok::Lit(chars.next().unwrap_or('\\'))),
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                toks.push(Tok::DoubleStar);
            }
            '*' => toks.push(Tok::Star),
            c => toks.push(Tok::Lit(c)),
        }
    }

    // reach[j]: the tokens so far can match the first j characters of the path.
    let s: Vec<char> = path.chars().collect();
    let mut reach = vec![false; s.len() + 1];
    reach[0] = true;
    for tok in &toks {
        let mut next = vec![false; s.len() + 1];
        match tok {
            Tok::Lit(c) => {
                for j in 0..s.len() {
                    if reach[j] && s[j] == *c {
                        next[j + 1] = true;
                    }
                }
            }
            Tok::Star => {
                for j in 0..=s.len() {
                    if reach[j] {
                        next[j] = true;
                        let mut k = j;
                        while k < s.len() && s[k] != '/' {
                            k += 1;
                            next[k] = true;
                        }
                    }
                }
            }
            Tok::DoubleStar => {
                if let Some(first) = reach.iter().position(|&r| r) {
                    next[first..].iter_mut().for_each(|n| *n = true);
                }
            }
        }
        reach = next;
    }
    reach[s.len()]
}

/// `REUSE.toml`: only the parts this check needs.
#[derive(Debug, Default, Deserialize)]
pub struct ReuseToml {
    /// The `[[annotations]]` tables, in file order.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

/// One `[[annotations]]` table of `REUSE.toml`.
#[derive(Debug, Deserialize)]
pub struct Annotation {
    /// One glob or a list of globs.
    pub path: OneOrMany,
    /// `closest` (the default), `aggregate` or `override`.
    #[serde(default)]
    pub precedence: Option<String>,
    /// Copyright notices.
    #[serde(rename = "SPDX-FileCopyrightText", default)]
    pub copyright: Option<OneOrMany>,
    /// The licence expression.
    #[serde(rename = "SPDX-License-Identifier", default)]
    pub licence: Option<String>,
}

/// A TOML value that is a string or an array of strings.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    /// A single string.
    One(String),
    /// An array of strings.
    Many(Vec<String>),
}

impl OneOrMany {
    fn items(&self) -> Vec<&str> {
        match self {
            OneOrMany::One(s) => vec![s.as_str()],
            OneOrMany::Many(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

impl ReuseToml {
    /// Reads `REUSE.toml`; a missing file means no annotations.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// The annotation that applies to `path`: the last matching one, as the REUSE specification provides.
    fn annotation_for(&self, path: &str) -> Option<&Annotation> {
        self.annotations
            .iter()
            .rev()
            .find(|a| a.path.items().iter().any(|g| glob_match(g, path)))
    }
}

/// The SPDX information in a file's header (its first [`HEADER_LINES`] lines).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Header {
    /// The licence expression, if the header has one.
    pub licence: Option<String>,
    /// The copyright notices, in order.
    pub copyright: Vec<String>,
}

/// Reads the SPDX tags from the first lines of `text`. A tag's value ends at the end of the line; a trailing
/// comment terminator (`-->`, `*/`) is dropped.
pub fn read_header(text: &str) -> Header {
    let value = |line: &str, tag: &str| -> Option<String> {
        let at = line.find(tag)?;
        let v = line[at + tag.len()..].trim();
        let v = v
            .strip_suffix("-->")
            .or_else(|| v.strip_suffix("*/"))
            .unwrap_or(v);
        Some(v.trim().to_owned())
    };
    let mut header = Header::default();
    for line in text.lines().take(HEADER_LINES) {
        if let Some(v) = value(line, COPYRIGHT_TAG) {
            header.copyright.push(v);
        }
        if header.licence.is_none() {
            header.licence = value(line, LICENCE_TAG);
        }
    }
    header
}

/// A file to check: its repository path and the SPDX information of its header.
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// `/`-separated path relative to the repository root.
    pub path: String,
    /// What its header declares.
    pub header: Header,
}

/// Whether `notice` is the collective notice: a four-digit year, one space, the collective holder.
pub fn is_collective_notice(notice: &str) -> bool {
    match notice.split_once(' ') {
        Some((year, holder)) => {
            year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit()) && holder == COLLECTIVE_HOLDER
        }
        None => false,
    }
}

/// The map is consistent: every allowed expression and every boundary entry parses as SPDX, a `crate = true`
/// rule has exactly one allowed expression, and GPL-2.0 is allowed only inside the `[gpl2]` glob.
pub fn check_policy(map: &LicenceMap) -> Vec<String> {
    let mut v = Vec::new();
    for rule in &map.path {
        for expr in &rule.allowed {
            if let Err(err) = spdx::Expression::parse(expr) {
                v.push(format!(
                    "policy: rule `{}` allows `{expr}`, which is not a valid SPDX expression: {err}",
                    rule.glob
                ));
            }
            if expr.contains("GPL-2.0") && !glob_inside(&rule.glob, &map.gpl2.only_in) {
                v.push(format!(
                    "policy: rule `{}` allows `{expr}`; GPL-2.0 is allowed only under `{}` (LR-04)",
                    rule.glob, map.gpl2.only_in
                ));
            }
        }
        if rule.krate && rule.allowed.len() != 1 {
            v.push(format!(
                "policy: crate rule `{}` must allow exactly one expression (the crate's `license`)",
                rule.glob
            ));
        }
    }
    for entry in &map.boundary.allow {
        if let Err(err) = spdx::Licensee::parse(entry) {
            v.push(format!(
                "policy: [boundary] entry `{entry}` is not a licence identifier (with optional WITH exception): {err}"
            ));
        }
    }
    v
}

/// Whether every path matched by `inner` is also matched by `outer`, for the simple case the map uses: `outer`
/// ends in `/**` and `inner` starts with its prefix.
fn glob_inside(inner: &str, outer: &str) -> bool {
    match outer.strip_suffix("**") {
        Some(prefix) => inner.starts_with(prefix),
        None => inner == outer,
    }
}

/// Rules LR-01, LR-02 (files), LR-04 (GPL-2.0 placement) and NFR-LIC-10 over `files`. Files under `LICENSES/`
/// are exempt, as the REUSE specification provides.
pub fn check_files(map: &LicenceMap, reuse: &ReuseToml, files: &[FileInfo]) -> Vec<String> {
    let mut v = Vec::new();
    for file in files {
        let path = file.path.as_str();
        if path.starts_with("LICENSES/") {
            continue;
        }
        let rule = match map.rule_for(path) {
            None => {
                v.push(format!(
                    "{path}: unmapped path; add a row to ci/licence-map.toml (19 §2.1) or do not commit the file"
                ));
                continue;
            }
            Some(Err(tied)) => {
                v.push(format!(
                    "{path}: ambiguous licence map, rules `{tied}` match with the same length"
                ));
                continue;
            }
            Some(Ok(rule)) => rule,
        };

        // The effective SPDX information: header, annotation or both, by the annotation's precedence.
        let annotation = reuse.annotation_for(path);
        let precedence = annotation
            .and_then(|a| a.precedence.as_deref())
            .unwrap_or("closest");
        let header_has_info = file.header.licence.is_some() || !file.header.copyright.is_empty();
        let (use_header, use_annotation) = match (annotation, precedence) {
            (None, _) => (true, false),
            (Some(_), "override") => (false, true),
            (Some(_), "aggregate") => (true, true),
            (Some(_), _) => (header_has_info, !header_has_info),
        };
        let mut licences: Vec<String> = Vec::new();
        let mut notices: Vec<String> = Vec::new();
        if use_header {
            licences.extend(file.header.licence.clone());
            notices.extend(file.header.copyright.iter().cloned());
        }
        if let (true, Some(a)) = (use_annotation, annotation) {
            licences.extend(a.licence.clone());
            if let Some(c) = &a.copyright {
                notices.extend(c.items().into_iter().map(str::to_owned));
            }
        }

        // LR-01 and LR-02.
        if licences.is_empty() {
            v.push(format!(
                "{path}: no SPDX licence expression in the header (first {HEADER_LINES} lines) or in REUSE.toml; \
                 expected {}",
                expected(rule)
            ));
        }
        for expr in &licences {
            if !rule.allowed.iter().any(|a| a == expr) {
                v.push(format!(
                    "{path}: licence `{expr}` is not allowed by rule `{}`; expected {}",
                    rule.glob,
                    expected(rule)
                ));
            }
            // LR-04.
            if expr.contains("GPL-2.0") && !glob_match(&map.gpl2.only_in, path) {
                v.push(format!(
                    "{path}: GPL-2.0 appears outside `{}` (LR-04)",
                    map.gpl2.only_in
                ));
            }
        }

        // NFR-LIC-10: the collective notice, and no other notice except in third-party files.
        let third_party = map.notices.third_party.iter().any(|g| glob_match(g, path));
        if !notices.iter().any(|n| is_collective_notice(n)) {
            v.push(format!(
                "{path}: missing the collective copyright notice `<year> {COLLECTIVE_HOLDER}` (NFR-LIC-10)"
            ));
        }
        if !third_party {
            for n in notices.iter().filter(|n| !is_collective_notice(n)) {
                v.push(format!(
                    "{path}: copyright notice `{n}` is not the collective notice `<year> {COLLECTIVE_HOLDER}`; \
                     only files listed in [notices].third_party keep other notices (NFR-LIC-10)"
                ));
            }
        }
    }
    v
}

fn expected(rule: &PathRule) -> String {
    let list: Vec<String> = rule.allowed.iter().map(|a| format!("`{a}`")).collect();
    format!("{} (rule `{}`)", list.join(" or "), rule.glob)
}

/// A workspace crate: its name, its manifest path relative to the repository root and its `license` field.
#[derive(Debug, Clone)]
pub struct CrateInfo {
    /// Package name.
    pub name: String,
    /// `/`-separated path of its Cargo.toml relative to the repository root.
    pub manifest: String,
    /// The `license` field.
    pub license: Option<String>,
}

/// LR-02 for crates: each workspace crate's manifest falls under a `crate = true` rule whose single allowed
/// expression equals the crate's `license` field.
pub fn check_crates(map: &LicenceMap, crates: &[CrateInfo]) -> Vec<String> {
    let mut v = Vec::new();
    for c in crates {
        let rule = match map.rule_for(&c.manifest) {
            Some(Ok(rule)) => rule,
            // Reported by check_files.
            _ => continue,
        };
        if !rule.krate {
            v.push(format!(
                "crate `{}` ({}): its rule `{}` is not a crate rule (`crate = true`)",
                c.name, c.manifest, rule.glob
            ));
            continue;
        }
        let want = rule.allowed.first().map(String::as_str);
        if c.license.as_deref() != want {
            v.push(format!(
                "crate `{}`: Cargo.toml `license` is {}, but rule `{}` requires `{}`",
                c.name,
                c.license
                    .as_deref()
                    .map_or("missing".to_owned(), |l| format!("`{l}`")),
                rule.glob,
                want.unwrap_or("?")
            ));
        }
    }
    v
}

/// One package of the resolved dependency graph.
#[derive(Debug, Clone)]
pub struct Package {
    /// Package name.
    pub name: String,
    /// Version.
    pub version: String,
    /// The `license` expression, if any.
    pub license: Option<String>,
    /// Whether it has a `license-file` instead.
    pub license_file: bool,
}

/// The resolved dependency graph, keyed by an opaque package id.
#[derive(Debug, Default)]
pub struct DepGraph {
    /// Every package.
    pub packages: BTreeMap<String, Package>,
    /// Resolved dependencies of every package (all kinds, all targets).
    pub deps: BTreeMap<String, Vec<String>>,
    /// Workspace package name to id.
    pub workspace: BTreeMap<String, String>,
}

impl DepGraph {
    /// Builds the graph from `cargo metadata` output (which must include `resolve`).
    pub fn from_metadata(metadata: &cargo_metadata::Metadata) -> Result<Self> {
        let mut graph = DepGraph::default();
        for p in &metadata.packages {
            graph.packages.insert(
                p.id.repr.clone(),
                Package {
                    name: p.name.to_string(),
                    version: p.version.to_string(),
                    license: p.license.clone(),
                    license_file: p.license_file.is_some(),
                },
            );
        }
        for p in metadata.workspace_packages() {
            graph.workspace.insert(p.name.to_string(), p.id.repr.clone());
        }
        let resolve = metadata
            .resolve
            .as_ref()
            .context("`cargo metadata` returned no dependency resolution")?;
        for node in &resolve.nodes {
            graph.deps.insert(
                node.id.repr.clone(),
                node.deps.iter().map(|d| d.pkg.repr.clone()).collect(),
            );
        }
        Ok(graph)
    }
}

/// LR-03 and NFR-MAINT-02: every package reachable from a permissive crate (through any dependency kind;
/// `cargo metadata` resolves dev-dependencies for workspace members only, which is the closure that matters) has
/// a `license` expression satisfiable from `allow`.
pub fn check_boundary(allow: &[String], permissive: &[String], graph: &DepGraph) -> Vec<String> {
    let mut v = Vec::new();
    let licensees: Vec<spdx::Licensee> = allow
        .iter()
        .filter_map(|a| spdx::Licensee::parse(a).ok())
        .collect();
    for root_name in permissive {
        let Some(root) = graph.workspace.get(root_name) else {
            v.push(format!(
                "licence boundary: permissive crate `{root_name}` is not a workspace package"
            ));
            continue;
        };
        // Breadth-first search, remembering the parent of each package for the error message.
        let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
        let mut seen: BTreeSet<&str> = BTreeSet::from([root.as_str()]);
        let mut queue: VecDeque<&str> = VecDeque::from([root.as_str()]);
        while let Some(id) = queue.pop_front() {
            if id != root.as_str() {
                if let Some(problem) = licence_problem(&graph.packages[id], &licensees) {
                    let mut chain = vec![graph.packages[id].name.as_str()];
                    let mut at = id;
                    while let Some(&p) = parent.get(at) {
                        chain.push(graph.packages[p].name.as_str());
                        at = p;
                    }
                    chain.reverse();
                    let pkg = &graph.packages[id];
                    v.push(format!(
                        "licence boundary: permissive crate `{root_name}` reaches `{} {}`, {problem} (via {})",
                        pkg.name,
                        pkg.version,
                        chain.join(" -> ")
                    ));
                }
            }
            for dep in graph.deps.get(id).into_iter().flatten() {
                if seen.insert(dep.as_str()) {
                    parent.insert(dep.as_str(), id);
                    queue.push_back(dep.as_str());
                }
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

/// Why `pkg` may not be in the permissive closure, or `None` if it may.
fn licence_problem(pkg: &Package, licensees: &[spdx::Licensee]) -> Option<String> {
    let Some(license) = pkg.license.as_deref() else {
        return Some(if pkg.license_file {
            "which declares only a `license-file`, no SPDX `license` expression".to_owned()
        } else {
            "which declares no `license`".to_owned()
        });
    };
    match spdx::Expression::parse_mode(license, spdx::ParseMode::LAX) {
        Err(err) => Some(format!("whose licence `{license}` does not parse: {err}")),
        Ok(expr) => {
            if expr.evaluate(|req| licensees.iter().any(|l| l.satisfies(req))) {
                None
            } else {
                Some(format!(
                    "whose licence `{license}` is not satisfiable from the [boundary] allowlist"
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lic(expr: &str) -> String {
        format!("{LICENCE_TAG} {expr}")
    }

    fn cr(notice: &str) -> String {
        format!("{COPYRIGHT_TAG} {notice}")
    }

    const MAP: &str = r#"
[[path]]
glob = "crates/p-*/**"
allowed = ["MIT OR Apache-2.0"]
crate = true

[[path]]
glob = "crates/g-model/**"
allowed = ["GPL-3.0-or-later"]
crate = true

[[path]]
glob = "crates/g-qet/**"
allowed = ["GPL-3.0-or-later"]
crate = true

[[path]]
glob = "crates/g-qet/src/qet_port/**"
allowed = ["GPL-2.0-or-later"]

[[path]]
glob = "docs/**"
allowed = ["CC-BY-4.0"]

[[path]]
glob = "*.md"
allowed = ["CC-BY-4.0"]

[[path]]
glob = "Cargo.lock"
allowed = ["CC0-1.0"]

[gpl2]
only_in = "crates/g-qet/src/qet_port/**"

[notices]
third_party = ["crates/g-qet/src/qet_port/**", "THIRD.md"]

[boundary]
allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "MPL-2.0", "Unicode-3.0"]
"#;

    const REUSE: &str = r#"
version = 1
[[annotations]]
path = ["Cargo.lock", "docs/**.md"]
precedence = "aggregate"
SPDX-FileCopyrightText = "2026 The ElektraWatt contributors"
SPDX-License-Identifier = "CC0-1.0"

[[annotations]]
path = "docs/**.md"
precedence = "aggregate"
SPDX-FileCopyrightText = "2026 The ElektraWatt contributors"
SPDX-License-Identifier = "CC-BY-4.0"
"#;

    fn map() -> LicenceMap {
        LicenceMap::parse(MAP).unwrap()
    }

    fn reuse() -> ReuseToml {
        toml::from_str(REUSE).unwrap()
    }

    fn file(path: &str, licence: Option<&str>, copyright: &[&str]) -> FileInfo {
        let mut text = String::new();
        for c in copyright {
            text.push_str(&format!("// {}\n", cr(c)));
        }
        if let Some(l) = licence {
            text.push_str(&format!("// {}\n", lic(l)));
        }
        text.push_str("\nfn main() {}\n");
        FileInfo {
            path: path.to_owned(),
            header: read_header(&text),
        }
    }

    const NOTICE: &str = "2026 The ElektraWatt contributors";

    fn clean_files() -> Vec<FileInfo> {
        vec![
            file("crates/p-units/src/lib.rs", Some("MIT OR Apache-2.0"), &[NOTICE]),
            file("crates/g-model/src/lib.rs", Some("GPL-3.0-or-later"), &[NOTICE]),
            file(
                "crates/g-qet/src/qet_port/element.rs",
                Some("GPL-2.0-or-later"),
                &["2006 Upstream Authors", NOTICE],
            ),
            file("README.md", Some("CC-BY-4.0"), &[NOTICE]),
            // Covered by REUSE.toml only.
            file("Cargo.lock", None, &[]),
            file("docs/design/x.md", None, &[]),
            file("LICENSES/MIT.txt", None, &[]),
        ]
    }

    fn assert_violation(violations: &[String], needle: &str) {
        assert!(
            violations.iter().any(|v| v.contains(needle)),
            "expected a violation containing {needle:?}, got {violations:#?}"
        );
    }

    #[test]
    fn glob_semantics_follow_reuse() {
        assert!(glob_match("*.md", "README.md"));
        assert!(!glob_match("*.md", "docs/README.md"));
        assert!(glob_match("docs/**", "docs/a/b/c.csv"));
        assert!(glob_match("docs/**.md", "docs/README.md"));
        assert!(glob_match("docs/**.md", "docs/adr/0001-x.md"));
        assert!(!glob_match("docs/**.md", "docs/plan/tools/x.py"));
        assert!(glob_match("crates/p-*/**", "crates/p-units/src/lib.rs"));
        assert!(!glob_match("crates/p-*/**", "crates/g-model/src/lib.rs"));
        assert!(!glob_match("crates/ew-dl/**", "crates/ew-dlx/src/lib.rs"));
        assert!(glob_match("a\\*b", "a*b"));
        assert!(!glob_match("a\\*b", "axb"));
    }

    #[test]
    fn header_reading() {
        let text = format!(
            "<!--\n{}\n{}\n-->\n# Title\n",
            cr("2026 The ElektraWatt contributors"),
            lic("CC-BY-4.0")
        );
        let h = read_header(&text);
        assert_eq!(h.licence.as_deref(), Some("CC-BY-4.0"));
        assert_eq!(h.copyright, vec!["2026 The ElektraWatt contributors"]);
        let html = format!("<!-- {} -->\n", lic("CC0-1.0"));
        assert_eq!(read_header(&html).licence.as_deref(), Some("CC0-1.0"));
        // Beyond the header window: not read.
        let late = format!("{}{}\n", "x\n".repeat(HEADER_LINES), lic("MIT"));
        assert_eq!(read_header(&late).licence, None);
    }

    #[test]
    fn collective_notice() {
        assert!(is_collective_notice("2026 The ElektraWatt contributors"));
        assert!(!is_collective_notice("2026 Some Person"));
        assert!(!is_collective_notice("The ElektraWatt contributors"));
        assert!(!is_collective_notice("26 The ElektraWatt contributors"));
    }

    #[test]
    fn clean_inputs_pass() {
        assert_eq!(check_policy(&map()), Vec::<String>::new());
        assert_eq!(
            check_files(&map(), &reuse(), &clean_files()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn seeded_unmapped_path_fails() {
        // (1) A committed third-party fixture in an unmapped directory (LR-11).
        let mut files = clean_files();
        files.push(file("fixtures/vendor.aml", Some("MIT"), &[NOTICE]));
        assert_violation(
            &check_files(&map(), &reuse(), &files),
            "fixtures/vendor.aml: unmapped path",
        );
    }

    #[test]
    fn seeded_wrong_licence_fails() {
        // (1) A GPL header in a permissive crate, and a file without any licence information.
        let mut files = clean_files();
        files.push(file(
            "crates/p-format/src/lib.rs",
            Some("GPL-3.0-or-later"),
            &[NOTICE],
        ));
        files.push(file("crates/g-model/src/x.rs", None, &[NOTICE]));
        let v = check_files(&map(), &reuse(), &files);
        assert_violation(
            &v,
            "crates/p-format/src/lib.rs: licence `GPL-3.0-or-later` is not allowed by rule `crates/p-*/**`",
        );
        assert_violation(&v, "crates/g-model/src/x.rs: no SPDX licence expression");
    }

    #[test]
    fn seeded_wrong_reuse_annotation_fails() {
        // (1) REUSE.toml annotates a path with a licence its row does not allow (aggregate precedence keeps both).
        let text = format!(
            "{REUSE}\n[[annotations]]\npath = \"README.md\"\nprecedence = \"aggregate\"\nSPDX-FileCopyrightText = \"{NOTICE}\"\nSPDX-License-Identifier = \"MIT\"\n"
        );
        let reuse: ReuseToml = toml::from_str(&text).unwrap();
        assert_violation(
            &check_files(&map(), &reuse, &clean_files()),
            "README.md: licence `MIT` is not allowed by rule `*.md`",
        );
    }

    #[test]
    fn seeded_missing_or_personal_copyright_fails() {
        // (2) NFR-LIC-10: no collective notice; a personal notice outside the third-party list.
        let mut files = clean_files();
        files.push(file(
            "crates/g-model/src/a.rs",
            Some("GPL-3.0-or-later"),
            &["2026 Some Person"],
        ));
        files.push(file(
            "crates/g-model/src/b.rs",
            Some("GPL-3.0-or-later"),
            &["2026 Some Employer", NOTICE],
        ));
        let v = check_files(&map(), &reuse(), &files);
        assert_violation(
            &v,
            "crates/g-model/src/a.rs: missing the collective copyright notice",
        );
        assert_violation(
            &v,
            "crates/g-model/src/b.rs: copyright notice `2026 Some Employer` is not the collective notice",
        );
        // Third-party files may keep other notices above the collective one.
        let ok = vec![file("THIRD.md", Some("CC-BY-4.0"), &["Upstream Org", NOTICE])];
        assert_eq!(check_files(&map(), &reuse(), &ok), Vec::<String>::new());
    }

    #[test]
    fn seeded_gpl2_outside_qet_port_fails() {
        // (3) LR-04, in the files and in the map itself.
        let mut files = clean_files();
        files.push(file(
            "crates/g-qet/src/lib.rs",
            Some("GPL-2.0-or-later"),
            &[NOTICE],
        ));
        assert_violation(
            &check_files(&map(), &reuse(), &files),
            "crates/g-qet/src/lib.rs: GPL-2.0 appears outside `crates/g-qet/src/qet_port/**`",
        );
        let text = MAP.replace(
            "glob = \"crates/g-model/**\"\nallowed = [\"GPL-3.0-or-later\"]",
            "glob = \"crates/g-model/**\"\nallowed = [\"GPL-2.0-or-later\"]",
        );
        assert_violation(
            &check_policy(&LicenceMap::parse(&text).unwrap()),
            "policy: rule `crates/g-model/**` allows `GPL-2.0-or-later`; GPL-2.0 is allowed only under",
        );
    }

    #[test]
    fn seeded_crate_licence_mismatch_fails() {
        // (4) LR-02 for crates.
        let crates = vec![
            CrateInfo {
                name: "p-units".into(),
                manifest: "crates/p-units/Cargo.toml".into(),
                license: Some("MIT OR Apache-2.0".into()),
            },
            CrateInfo {
                name: "p-format".into(),
                manifest: "crates/p-format/Cargo.toml".into(),
                license: Some("GPL-3.0-or-later".into()),
            },
            CrateInfo {
                name: "g-model".into(),
                manifest: "crates/g-model/Cargo.toml".into(),
                license: None,
            },
        ];
        let v = check_crates(&map(), &crates);
        assert_eq!(v.len(), 2, "{v:#?}");
        assert_violation(
            &v,
            "crate `p-format`: Cargo.toml `license` is `GPL-3.0-or-later`, but rule `crates/p-*/**` requires `MIT OR Apache-2.0`",
        );
        assert_violation(&v, "crate `g-model`: Cargo.toml `license` is missing");
    }

    #[test]
    fn ambiguous_rules_are_reported() {
        let text = format!("{MAP}\n[[path]]\nglob = \"docs/**\"\nallowed = [\"CC0-1.0\"]\n");
        let map = LicenceMap::parse(&text).unwrap();
        assert_violation(
            &check_files(&map, &reuse(), &clean_files()),
            "docs/design/x.md: ambiguous licence map",
        );
    }

    fn pkg(name: &str, license: Option<&str>, license_file: bool) -> Package {
        Package {
            name: name.into(),
            version: "1.0.0".into(),
            license: license.map(str::to_owned),
            license_file,
        }
    }

    /// p-format -> p-schema -> serde; p-format -dev-> wasm-ish (Apache-2.0 WITH LLVM-exception);
    /// g-model -> gpl-crate. Edges carry no kind: the check follows every kind.
    fn graph(extra: &[(&str, &str)], extra_pkgs: Vec<Package>) -> DepGraph {
        let mut g = DepGraph::default();
        for p in [
            pkg("p-format", Some("MIT OR Apache-2.0"), false),
            pkg("p-schema", Some("MIT OR Apache-2.0"), false),
            pkg("serde", Some("MIT OR Apache-2.0"), false),
            pkg(
                "unicode-ident",
                Some("(MIT OR Apache-2.0) AND Unicode-3.0"),
                false,
            ),
            pkg("wasm-ish", Some("Apache-2.0 WITH LLVM-exception"), false),
            pkg("imbl", Some("MPL-2.0+"), false),
            pkg("old-style", Some("MIT/Apache-2.0"), false),
            pkg("g-model", Some("GPL-3.0-or-later"), false),
            pkg("gpl-crate", Some("GPL-3.0-only"), false),
        ]
        .into_iter()
        .chain(extra_pkgs)
        {
            g.packages.insert(p.name.clone(), p);
        }
        for ws in ["p-format", "p-schema", "g-model"] {
            g.workspace.insert(ws.into(), ws.into());
        }
        let edges = [
            ("p-format", "p-schema"),
            ("p-format", "wasm-ish"),
            ("p-schema", "serde"),
            ("p-schema", "unicode-ident"),
            ("p-schema", "imbl"),
            ("p-schema", "old-style"),
            ("g-model", "gpl-crate"),
            ("g-model", "p-schema"),
        ];
        for (from, to) in edges.iter().chain(extra) {
            g.deps.entry(from.to_string()).or_default().push(to.to_string());
        }
        g
    }

    fn boundary(g: &DepGraph) -> Vec<String> {
        check_boundary(
            &map().boundary.allow,
            &["p-format".to_owned(), "p-schema".to_owned()],
            g,
        )
    }

    #[test]
    fn clean_boundary_passes() {
        assert_eq!(boundary(&graph(&[], vec![])), Vec::<String>::new());
    }

    #[test]
    fn seeded_gpl_dependency_of_permissive_crate_fails() {
        // (5) NFR-MAINT-02: a GPL crate reached through a workspace crate (as a dev-dependency edge would be).
        let v = boundary(&graph(&[("p-format", "g-model")], vec![]));
        assert_violation(
            &v,
            "permissive crate `p-format` reaches `g-model 1.0.0`, whose licence `GPL-3.0-or-later` is not satisfiable",
        );
        assert_violation(&v, "reaches `gpl-crate 1.0.0`");
        assert_violation(&v, "(via p-format -> g-model -> gpl-crate)");
    }

    #[test]
    fn seeded_licence_expressions_are_evaluated() {
        // (5) AND needs every operand; LGPL or a bare exception mismatch fails; OR needs one.
        let pkgs = vec![
            pkg("and-gpl", Some("MIT AND GPL-2.0-or-later"), false),
            pkg("or-gpl", Some("MIT OR GPL-2.0-or-later"), false),
            pkg("lgpl", Some("LGPL-2.1-or-later"), false),
            pkg("no-licence", None, false),
            pkg("file-only", None, true),
        ];
        let v = boundary(&graph(
            &[
                ("p-schema", "and-gpl"),
                ("p-schema", "or-gpl"),
                ("p-schema", "lgpl"),
                ("p-schema", "no-licence"),
                ("p-schema", "file-only"),
            ],
            pkgs,
        ));
        assert_violation(&v, "reaches `and-gpl 1.0.0`");
        assert_violation(&v, "reaches `lgpl 1.0.0`");
        assert_violation(&v, "reaches `no-licence 1.0.0`, which declares no `license`");
        assert_violation(
            &v,
            "reaches `file-only 1.0.0`, which declares only a `license-file`",
        );
        assert!(!v.iter().any(|m| m.contains("or-gpl")), "{v:#?}");
    }

    #[test]
    fn real_repository_passes() {
        let root = crate::workspace_root();
        let violations = crate::check_licence_map_at(&root).unwrap();
        assert_eq!(violations, Vec::<String>::new());
    }
}
