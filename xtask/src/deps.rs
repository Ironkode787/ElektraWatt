// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The rule `layers`: the rules between crates of `ci/dep-rules.toml`. A crate's layer is its licence and its
//! dependencies are its manifest's; the policy file holds only what those do not state.
//!
//! The check works on [`Graph`], a plain list of the direct dependencies that every workspace package
//! declares (all kinds, all targets, optional ones included). Building it from `cargo metadata` is separate
//! from checking it, so the unit tests can feed synthetic graphs with seeded violations.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

/// The policy file `ci/dep-rules.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Which layer each workspace package belongs to.
    pub layers: Layers,
    /// Permitted direct workspace-to-workspace edges of the packages that have an entry; a package without one is
    /// not restricted.
    #[serde(default)]
    pub allowed_edges: BTreeMap<String, Vec<String>>,
    /// Extra edges permitted for dev-dependencies only.
    #[serde(default)]
    pub allowed_dev_edges: BTreeMap<String, Vec<String>>,
    /// External crates that only named packages may depend on directly.
    #[serde(default)]
    pub external_owner: Vec<ExternalOwner>,
    /// The importer rule.
    pub importers: Importers,
    /// Crates whose dependents are restricted.
    #[serde(default)]
    pub confined: Vec<Confined>,
    /// Features that may only be enabled on the listed dependency kinds (the feature rule of `cargo xtask rules`).
    #[serde(default)]
    pub banned_features: Vec<crate::features::BannedFeature>,
    /// Features that must stay enabled when their crate is in the build (the feature rule of `cargo xtask rules`).
    #[serde(default)]
    pub required_features: Vec<crate::features::RequiredFeature>,
    /// Crates that may only be dev-dependencies (the feature rule of `cargo xtask rules`).
    #[serde(default)]
    pub banned_crates: Vec<String>,
    /// Test-only features that every test build reaching their crate requests (the feature rule of `cargo xtask rules`).
    #[serde(default)]
    pub uniform_test_features: Vec<crate::features::UniformTestFeature>,
    /// The source-level guard for a gated feature's readers (the raw-read rule of `cargo xtask rules`).
    #[serde(default)]
    pub raw_read: Option<crate::raw_read::RawReadPolicy>,
}

/// Layer membership of the workspace packages.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layers {
    /// MIT OR Apache-2.0 crates; closed under dependencies. Not in the file: [`Policy::with_layers`] reads it from the
    /// manifests' licences.
    #[serde(default)]
    pub permissive: Vec<String>,
    /// GPL application crates and binaries: every other package that is not a tool. From the manifests too.
    #[serde(default)]
    pub gpl: Vec<String>,
    /// Development-only packages.
    pub tools: Vec<String>,
}

/// The licence expression of a permissive crate.
pub const PERMISSIVE: &str = "MIT OR Apache-2.0";

/// Only `owners` may depend directly on an external crate matching one of `crates`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalOwner {
    /// Crate names; `*` matches any run of characters.
    pub crates: Vec<String>,
    /// Packages allowed to depend on them.
    pub owners: Vec<String>,
    /// Where the rule comes from.
    pub reason: String,
}

/// Packages matching `pattern` may depend only on permissive workspace crates.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Importers {
    /// Package-name pattern; `*` matches any run of characters.
    pub pattern: String,
}

/// Restrictions on who may depend on `krate`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Confined {
    /// The confined crate.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The only packages allowed to depend on it directly.
    pub only_direct_dependents: Vec<String>,
    /// Packages that must not reach it through any chain of workspace edges; `@permissive` expands to the
    /// permissive layer.
    pub never_reached_from: Vec<String>,
    /// Manifests outside the workspace (paths from the repository root, such as the fuzz crate's) whose direct
    /// dependencies the rule reads too ([`check_confined_manifests`]).
    #[serde(default)]
    pub manifests: Vec<String>,
    /// Where the rule comes from.
    pub reason: String,
}

impl Policy {
    /// Reads and parses a policy file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// Parses policy TOML.
    pub fn parse(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }

    /// The policy with its layers read from the workspace's manifests: a package whose licence is [`PERMISSIVE`] is
    /// permissive, every other one GPL; the tools are in neither.
    pub fn with_layers(mut self, metadata: &cargo_metadata::Metadata) -> Self {
        self.layers.permissive.clear();
        self.layers.gpl.clear();
        for p in metadata.workspace_packages() {
            let name = p.name.to_string();
            if self.layers.tools.contains(&name) {
                continue;
            }
            if p.license.as_deref() == Some(PERMISSIVE) {
                self.layers.permissive.push(name);
            } else {
                self.layers.gpl.push(name);
            }
        }
        self.layers.permissive.sort();
        self.layers.gpl.sort();
        self
    }

    fn is_permissive(&self, name: &str) -> bool {
        self.layers.permissive.iter().any(|p| p == name)
    }
}

/// The kind of a declared dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// `[dependencies]`.
    Normal,
    /// `[dev-dependencies]`.
    Dev,
    /// `[build-dependencies]`.
    Build,
}

impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Kind::Normal => "dependency",
            Kind::Dev => "dev-dependency",
            Kind::Build => "build-dependency",
        }
    }
}

/// One declared, direct dependency of a workspace package.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    /// The dependent workspace package.
    pub from: String,
    /// The package depended on (its real name, also when renamed in the manifest).
    pub to: String,
    /// Dependency kind.
    pub kind: Kind,
}

/// The workspace packages and their declared direct dependencies.
#[derive(Debug, Default)]
pub struct Graph {
    /// Names of the workspace packages.
    pub workspace: BTreeSet<String>,
    /// Their declared direct dependencies, sorted.
    pub edges: Vec<Edge>,
}

impl Graph {
    /// Builds the graph from `cargo metadata` output.
    pub fn from_metadata(metadata: &cargo_metadata::Metadata) -> Self {
        let packages = metadata.workspace_packages();
        let workspace: BTreeSet<String> = packages.iter().map(|p| p.name.to_string()).collect();
        let mut edges: Vec<Edge> = packages
            .iter()
            .flat_map(|p| {
                p.dependencies.iter().map(move |d| Edge {
                    from: p.name.to_string(),
                    to: d.name.clone(),
                    kind: match d.kind {
                        cargo_metadata::DependencyKind::Development => Kind::Dev,
                        cargo_metadata::DependencyKind::Build => Kind::Build,
                        _ => Kind::Normal,
                    },
                })
            })
            .collect();
        edges.sort();
        edges.dedup();
        Graph { workspace, edges }
    }

    fn is_workspace(&self, name: &str) -> bool {
        self.workspace.contains(name)
    }
}

/// `*` matches any (possibly empty) run of characters; everything else matches literally.
pub fn wildcard_match(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || name.len() < first.len() + last.len() || !name.ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle) {
            Some(i) => rest = &rest[i + middle.len()..],
            None => return false,
        }
    }
    true
}

/// Checks `graph` against `policy` and returns every violation, sorted.
pub fn check(policy: &Policy, graph: &Graph) -> Vec<String> {
    let mut v = Vec::new();
    check_policy_consistency(policy, graph, &mut v);
    check_edges(policy, graph, &mut v);
    check_external_owners(policy, graph, &mut v);
    check_importers(policy, graph, &mut v);
    check_confined(policy, graph, &mut v);
    v.sort();
    v.dedup();
    v
}

/// The policy names no package that is not in the workspace.
fn check_policy_consistency(policy: &Policy, graph: &Graph, v: &mut Vec<String>) {
    let unknown = |name: &str, place: &str, v: &mut Vec<String>| {
        if !graph.is_workspace(name) {
            v.push(format!(
                "policy: {place} names `{name}`, which is not a workspace package"
            ));
        }
    };
    for name in &policy.layers.tools {
        unknown(name, "[layers] tools", v);
    }
    for (table, edges) in [
        ("[allowed_edges]", &policy.allowed_edges),
        ("[allowed_dev_edges]", &policy.allowed_dev_edges),
    ] {
        for (from, tos) in edges {
            unknown(from, table, v);
            for to in tos {
                unknown(to, table, v);
            }
        }
    }
    for rule in &policy.external_owner {
        for owner in &rule.owners {
            unknown(owner, "[[external_owner]] owners", v);
        }
    }
    for rule in &policy.confined {
        unknown(&rule.krate, "[[confined]] crate", v);
        for name in &rule.only_direct_dependents {
            unknown(name, "[[confined]] only_direct_dependents", v);
        }
        for name in &rule.never_reached_from {
            if name != "@permissive" {
                unknown(name, "[[confined]] never_reached_from", v);
            }
        }
    }
}

/// A package with an `[allowed_edges]` entry depends on no workspace package outside it, and no permissive crate
/// depends on a non-permissive one.
fn check_edges(policy: &Policy, graph: &Graph, v: &mut Vec<String>) {
    let empty = Vec::new();
    for e in graph.edges.iter().filter(|e| graph.is_workspace(&e.to)) {
        if let Some(allowed) = policy.allowed_edges.get(&e.from) {
            let dev_allowed = policy.allowed_dev_edges.get(&e.from).unwrap_or(&empty);
            let ok = allowed.contains(&e.to) || (e.kind == Kind::Dev && dev_allowed.contains(&e.to));
            if !ok {
                v.push(format!(
                    "edge: `{}` -> `{}` ({}) is not in its [allowed_edges] entry",
                    e.from,
                    e.to,
                    e.kind.label()
                ));
            }
        }
        if policy.is_permissive(&e.from) && !policy.is_permissive(&e.to) {
            v.push(format!(
                "licence boundary: permissive crate `{}` depends on non-permissive `{}` ({}) (ADR-0024 decision 2)",
                e.from,
                e.to,
                e.kind.label()
            ));
        }
    }
}

/// External crates covered by an owner rule are direct dependencies of their owners only.
fn check_external_owners(policy: &Policy, graph: &Graph, v: &mut Vec<String>) {
    for e in graph.edges.iter().filter(|e| !graph.is_workspace(&e.to)) {
        for rule in &policy.external_owner {
            if rule.crates.iter().any(|p| wildcard_match(p, &e.to)) && !rule.owners.contains(&e.from) {
                v.push(format!(
                    "external: `{}` depends on `{}` ({}), allowed only in {} ({})",
                    e.from,
                    e.to,
                    e.kind.label(),
                    rule.owners.join(", "),
                    rule.reason
                ));
            }
        }
    }
}

/// Importers depend on permissive workspace crates only.
fn check_importers(policy: &Policy, graph: &Graph, v: &mut Vec<String>) {
    for e in graph.edges.iter().filter(|e| graph.is_workspace(&e.to)) {
        if wildcard_match(&policy.importers.pattern, &e.from) && !policy.is_permissive(&e.to) {
            v.push(format!(
                "importer: `{}` depends on non-permissive workspace crate `{}` ({}) (ARCH §3 rule 4)",
                e.from,
                e.to,
                e.kind.label()
            ));
        }
    }
}

/// Confined crates: restricted direct dependents and forbidden transitive reach.
fn check_confined(policy: &Policy, graph: &Graph, v: &mut Vec<String>) {
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for e in graph.edges.iter().filter(|e| graph.is_workspace(&e.to)) {
        adjacency.entry(&e.from).or_default().insert(&e.to);
    }
    for rule in &policy.confined {
        for e in graph.edges.iter().filter(|e| e.to == rule.krate) {
            if !rule.only_direct_dependents.contains(&e.from) {
                v.push(format!(
                    "confined: `{}` depends on `{}` ({}); only {} may ({})",
                    e.from,
                    rule.krate,
                    e.kind.label(),
                    rule.only_direct_dependents.join(", "),
                    rule.reason
                ));
            }
        }
        let sources = rule.never_reached_from.iter().flat_map(|name| {
            if name == "@permissive" {
                policy.layers.permissive.clone()
            } else {
                vec![name.clone()]
            }
        });
        for source in sources {
            if let Some(path) = find_path(&adjacency, &source, &rule.krate) {
                v.push(format!(
                    "confined: `{source}` reaches `{}` via {} ({})",
                    rule.krate,
                    path.join(" -> "),
                    rule.reason
                ));
            }
        }
    }
}

/// The direct dependencies a manifest declares (its `[dependencies]`, `[dev-dependencies]` and
/// `[build-dependencies]`, also under a `[target.'cfg(…)']` table), each by its real name, also when renamed.
///
/// # Errors
///
/// The text is no TOML.
pub fn manifest_edges(text: &str) -> Result<Vec<Edge>> {
    let doc: toml::Table = toml::from_str(text)?;
    let from = doc
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or("?")
        .to_owned();
    let mut edges = Vec::new();
    // The manifest's own tables, then each target's.
    let targets = doc
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
        .filter_map(toml::Value::as_table);
    for tables in std::iter::once(&doc).chain(targets) {
        for (table, kind) in [
            ("dependencies", Kind::Normal),
            ("dev-dependencies", Kind::Dev),
            ("build-dependencies", Kind::Build),
        ] {
            for (name, spec) in tables
                .get(table)
                .and_then(toml::Value::as_table)
                .into_iter()
                .flatten()
            {
                let to = spec.get("package").and_then(toml::Value::as_str).unwrap_or(name);
                edges.push(Edge {
                    from: from.clone(),
                    to: to.to_owned(),
                    kind,
                });
            }
        }
    }
    Ok(edges)
}

/// The confined crates in the manifests outside the workspace that their rules name (`manifests`): a
/// direct dependency on the confined crate from a package the rule does not allow is a violation.
///
/// # Errors
///
/// A named manifest does not read or is no TOML.
pub fn check_confined_manifests(policy: &Policy, root: &Path) -> Result<Vec<String>> {
    let mut v = Vec::new();
    for rule in &policy.confined {
        for manifest in &rule.manifests {
            let text = std::fs::read_to_string(root.join(manifest))
                .with_context(|| format!("reading {manifest}"))?;
            let edges = manifest_edges(&text).with_context(|| format!("parsing {manifest}"))?;
            for e in edges.iter().filter(|e| e.to == rule.krate) {
                if !rule.only_direct_dependents.contains(&e.from) {
                    v.push(format!(
                        "confined: `{}` ({manifest}) depends on `{}` ({}); only {} may ({})",
                        e.from,
                        rule.krate,
                        e.kind.label(),
                        rule.only_direct_dependents.join(", "),
                        rule.reason
                    ));
                }
            }
        }
    }
    Ok(v)
}

/// The names an importer's source may not use (NFR-SEC-18): importers read only the bytes they are given,
/// never a file, the network, a process or the environment.
pub const IMPURE_NAMES: [&str; 7] = [
    "std::fs",
    "std::net",
    "std::process",
    "std::env",
    "File::",
    "TcpStream",
    "Command::new",
];

/// The `std` module a grouped import `use std::{…}` names among its direct members (`fs`, `env`, `net`, `process`), as
/// its [`IMPURE_NAMES`] entry. `depth` is the group's brace depth, carried from line to line (0: outside a group).
fn grouped_std_module(code: &str, depth: &mut i64) -> Option<&'static str> {
    let rest = if *depth > 0 {
        code
    } else {
        &code[code.find("std::{")? + "std::".len()..]
    };
    // The last non-blank character before the current word: a member starts after `{` or `,`.
    let mut prev = if *depth > 0 { ',' } else { ' ' };
    let mut word = String::new();
    let mut found = None;
    for c in rest.chars().chain([' ']) {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            if *depth == 1 && found.is_none() && matches!(prev, '{' | ',') {
                found = match word.as_str() {
                    "fs" => Some("std::fs"),
                    "env" => Some("std::env"),
                    "net" => Some("std::net"),
                    "process" => Some("std::process"),
                    _ => None,
                };
            }
            word.clear();
            prev = 'a';
        }
        match c {
            '{' => *depth += 1,
            '}' => *depth -= 1,
            _ => {}
        }
        if *depth <= 0 {
            *depth = 0;
            break;
        }
        if !c.is_whitespace() {
            prev = c;
        }
    }
    found
}

/// The lines of `text` (the source file `file`) that name one of [`IMPURE_NAMES`], or one of their modules inside a
/// grouped `use std::{…}`, as violations. Comment lines and `#[cfg(test)]` items (to their closing brace or `;`, on the
/// attribute's own line too) are skipped.
pub fn impure_lines(file: &str, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Inside a `#[cfg(test)]` item: its brace depth, and whether its first brace was seen.
    let mut skip: Option<(i64, bool)> = None;
    // Inside a grouped `use std::{…}`: its brace depth.
    let mut group = 0;
    for (n, line) in text.lines().enumerate() {
        let mut code = line.trim_start();
        if skip.is_none() {
            if let Some(rest) = code.strip_prefix("#[cfg(test)]") {
                // The item may start on the attribute's own line.
                skip = Some((0, false));
                code = rest.trim_start();
            }
        }
        if let Some((depth, opened)) = skip.as_mut() {
            for c in code.chars() {
                match c {
                    '{' => (*depth, *opened) = (*depth + 1, true),
                    '}' => *depth -= 1,
                    _ => {}
                }
            }
            if (*opened && *depth <= 0) || (!*opened && code.ends_with(';')) {
                skip = None;
            }
            continue;
        }
        if code.starts_with("//") {
            continue;
        }
        let grouped = grouped_std_module(code, &mut group);
        if let Some(name) = IMPURE_NAMES
            .iter()
            .copied()
            .find(|name| code.contains(*name))
            .or(grouped)
        {
            out.push(format!(
                "importer purity (NFR-SEC-18): {file}:{} names `{name}`; an importer reads only the bytes it is given",
                n + 1
            ));
        }
    }
    out
}

/// [`impure_lines`] over every `.rs` file under `src/` of `ew-interop-api` and the `ew-import-*` crates of `root`.
///
/// # Errors
///
/// A directory or file that cannot be read.
pub fn importer_purity(root: &Path) -> Result<Vec<String>> {
    fn walk(dir: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();
            if path.is_dir() {
                walk(&path, files)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
        Ok(())
    }
    let mut crates: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("crates"))
        .context("reading crates/")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n == "ew-interop-api" || n.starts_with("ew-import-"))
        })
        .collect();
    crates.sort();
    let mut out = Vec::new();
    for krate in crates {
        let mut files = Vec::new();
        walk(&krate.join("src"), &mut files)?;
        files.sort();
        for file in files {
            let text =
                std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
            let shown = file
                .strip_prefix(root)
                .unwrap_or(&file)
                .display()
                .to_string()
                .replace('\\', "/");
            out.extend(impure_lines(&shown, &text));
        }
    }
    Ok(out)
}

/// Breadth-first search for a path of workspace edges from `from` to `to`; deterministic (sorted adjacency).
fn find_path(adjacency: &BTreeMap<&str, BTreeSet<&str>>, from: &str, to: &str) -> Option<Vec<String>> {
    let mut parent: BTreeMap<&str, &str> = BTreeMap::new();
    let mut queue = std::collections::VecDeque::from([from]);
    let mut seen = BTreeSet::from([from]);
    while let Some(node) = queue.pop_front() {
        for &next in adjacency.get(node).into_iter().flatten() {
            if !seen.insert(next) {
                continue;
            }
            parent.insert(next, node);
            if next == to {
                let mut path = vec![to.to_owned()];
                let mut cur = to;
                while let Some(&p) = parent.get(cur) {
                    path.push(p.to_owned());
                    cur = p;
                }
                path.reverse();
                return Some(path);
            }
            queue.push_back(next);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small policy with the same shape as `ci/dep-rules.toml`.
    const POLICY: &str = r#"
        [layers]
        permissive = ["p-units", "p-schema", "p-api"]
        gpl = ["g-model", "g-sql", "g-ui", "g-host", "g-import-qet", "g-import-x", "g-top", "app"]
        tools = ["xtask"]

        [allowed_edges]
        "p-units" = []
        "p-schema" = ["p-units"]
        "p-api" = ["p-schema"]
        "g-model" = ["p-schema"]
        "g-sql" = ["g-model"]
        "g-ui" = ["g-model"]
        "g-host" = ["g-sql"]
        "g-import-qet" = ["p-api"]
        "g-import-x" = ["p-api"]
        "g-top" = ["g-sql", "g-import-qet", "g-import-x", "g-host"]
        "app" = ["g-ui", "g-top"]
        "xtask" = []

        [[external_owner]]
        crates = ["rusqlite"]
        owners = ["g-sql"]
        reason = "rule 2"

        [[external_owner]]
        crates = ["egui", "egui_*", "egui-*"]
        owners = ["g-ui", "app"]
        reason = "rule 3"

        [importers]
        pattern = "g-import-*"

        [[confined]]
        crate = "g-import-qet"
        only_direct_dependents = ["g-top"]
        never_reached_from = ["g-host", "@permissive"]
        reason = "ADR-0024 decision 4"
    "#;

    fn policy() -> Policy {
        Policy::parse(POLICY).expect("test policy parses")
    }

    /// The graph that exactly matches the test policy, plus some harmless external dependencies.
    fn clean_edges() -> Vec<(&'static str, &'static str, Kind)> {
        vec![
            ("p-schema", "p-units", Kind::Normal),
            ("p-api", "p-schema", Kind::Normal),
            ("g-model", "p-schema", Kind::Normal),
            ("g-sql", "g-model", Kind::Normal),
            ("g-sql", "rusqlite", Kind::Normal),
            ("g-ui", "g-model", Kind::Normal),
            ("g-ui", "egui", Kind::Normal),
            ("g-ui", "egui_tiles", Kind::Normal),
            ("g-host", "g-sql", Kind::Normal),
            ("g-import-qet", "p-api", Kind::Normal),
            ("g-import-x", "p-api", Kind::Normal),
            ("g-top", "g-sql", Kind::Normal),
            ("g-top", "g-import-qet", Kind::Normal),
            ("g-top", "g-import-x", Kind::Normal),
            ("g-top", "g-host", Kind::Normal),
            ("app", "g-ui", Kind::Normal),
            ("app", "g-top", Kind::Normal),
            ("app", "egui", Kind::Normal),
            ("xtask", "serde", Kind::Normal),
            ("p-units", "serde", Kind::Normal),
        ]
    }

    fn graph(edges: &[(&str, &str, Kind)]) -> Graph {
        let p = policy();
        let workspace = p
            .layers
            .permissive
            .iter()
            .chain(&p.layers.gpl)
            .chain(&p.layers.tools)
            .cloned()
            .collect();
        let mut edges: Vec<Edge> = edges
            .iter()
            .map(|&(from, to, kind)| Edge {
                from: from.into(),
                to: to.into(),
                kind,
            })
            .collect();
        edges.sort();
        Graph { workspace, edges }
    }

    fn with(extra: (&'static str, &'static str, Kind)) -> Vec<String> {
        let mut edges = clean_edges();
        edges.push(extra);
        check(&policy(), &graph(&edges))
    }

    fn assert_violation(violations: &[String], needle: &str) {
        assert!(
            violations.iter().any(|v| v.contains(needle)),
            "expected a violation containing {needle:?}, got {violations:#?}"
        );
    }

    #[test]
    fn clean_graph_passes() {
        assert_eq!(check(&policy(), &graph(&clean_edges())), Vec::<String>::new());
    }

    #[test]
    fn unlisted_edge_fails() {
        // g-model is below g-sql in the graph: depending upwards is not listed.
        let v = with(("g-model", "g-sql", Kind::Normal));
        assert_violation(
            &v,
            "edge: `g-model` -> `g-sql` (dependency) is not in its [allowed_edges] entry",
        );
    }

    #[test]
    fn unlisted_dev_edge_fails_unless_dev_allowed() {
        let v = with(("g-model", "g-ui", Kind::Dev));
        assert_violation(&v, "edge: `g-model` -> `g-ui` (dev-dependency)");

        let text = format!("{POLICY}\n[allowed_dev_edges]\n\"g-model\" = [\"g-ui\"]\n");
        let p = Policy::parse(&text).unwrap();
        let mut edges = clean_edges();
        edges.push(("g-model", "g-ui", Kind::Dev));
        assert_eq!(check(&p, &graph(&edges)), Vec::<String>::new());
    }

    #[test]
    fn permissive_depending_on_gpl_fails_even_if_policy_lists_the_edge() {
        let text = POLICY.replace(
            r#""p-api" = ["p-schema"]"#,
            r#""p-api" = ["p-schema", "g-model"]"#,
        );
        let p = Policy::parse(&text).unwrap();
        let mut edges = clean_edges();
        edges.push(("p-api", "g-model", Kind::Normal));
        let v = check(&p, &graph(&edges));
        assert_violation(&v, "licence boundary: permissive crate `p-api`");
    }

    #[test]
    fn external_crates_outside_their_owner_fail() {
        // An exact name, a `egui_*` wildcard (as a dev-dependency) and a hyphenated `egui-*` match.
        for (from, to, kind, needle) in [
            (
                "g-ui",
                "rusqlite",
                Kind::Normal,
                "external: `g-ui` depends on `rusqlite`",
            ),
            (
                "g-model",
                "egui_extras",
                Kind::Dev,
                "external: `g-model` depends on `egui_extras` (dev-dependency)",
            ),
            (
                "g-model",
                "egui-winit",
                Kind::Normal,
                "external: `g-model` depends on `egui-winit` (dependency)",
            ),
        ] {
            assert_violation(&with((from, to, kind)), needle);
        }
        // The owner may use a hyphenated crate.
        let mut edges = clean_edges();
        edges.push(("g-ui", "egui-winit", Kind::Normal));
        assert_eq!(check(&policy(), &graph(&edges)), Vec::<String>::new());
    }

    #[test]
    fn importer_depending_on_gpl_crate_fails() {
        let text = POLICY.replace(
            r#""g-import-x" = ["p-api"]"#,
            r#""g-import-x" = ["p-api", "g-model"]"#,
        );
        let p = Policy::parse(&text).unwrap();
        let mut edges = clean_edges();
        edges.push(("g-import-x", "g-model", Kind::Normal));
        let v = check(&p, &graph(&edges));
        assert_eq!(v.len(), 1, "{v:#?}");
        assert_violation(
            &v,
            "importer: `g-import-x` depends on non-permissive workspace crate `g-model`",
        );
    }

    #[test]
    fn confined_crate_with_other_direct_dependent_fails() {
        let text = POLICY.replace(
            r#""app" = ["g-ui", "g-top"]"#,
            r#""app" = ["g-ui", "g-top", "g-import-qet"]"#,
        );
        let p = Policy::parse(&text).unwrap();
        let mut edges = clean_edges();
        edges.push(("app", "g-import-qet", Kind::Normal));
        let v = check(&p, &graph(&edges));
        assert_eq!(v.len(), 1, "{v:#?}");
        assert_violation(&v, "confined: `app` depends on `g-import-qet`");
    }

    #[test]
    fn confined_crate_reached_transitively_fails() {
        // g-host -> g-top -> g-import-qet: g-host reaches the confined crate without depending on it directly.
        // The policy allows both edges, so only the confined rule can catch it.
        let text = POLICY
            .replace(r#""g-host" = ["g-sql"]"#, r#""g-host" = ["g-sql", "g-top"]"#)
            .replace(
                r#""g-top" = ["g-sql", "g-import-qet", "g-import-x", "g-host"]"#,
                r#""g-top" = ["g-sql", "g-import-qet", "g-import-x"]"#,
            );
        let p = Policy::parse(&text).unwrap();
        let mut edges = clean_edges();
        edges.retain(|e| *e != ("g-top", "g-host", Kind::Normal));
        edges.push(("g-host", "g-top", Kind::Normal));
        let v = check(&p, &graph(&edges));
        assert_eq!(v.len(), 1, "{v:#?}");
        assert_violation(
            &v,
            "confined: `g-host` reaches `g-import-qet` via g-host -> g-top -> g-import-qet",
        );
    }

    #[test]
    fn confined_crate_reached_from_permissive_layer_fails() {
        let v = with(("p-units", "g-top", Kind::Normal));
        assert_violation(
            &v,
            "confined: `p-units` reaches `g-import-qet` via p-units -> g-top -> g-import-qet",
        );
    }

    /// The confined rule reads the manifests it names outside the workspace: a direct edge there,
    /// renamed, a dev-dependency or under a target table, fails.
    #[test]
    fn confined_crate_in_a_manifest_outside_the_workspace_fails() {
        let manifest = "[package]\nname = \"g-fuzz\"\n\n[dependencies]\ng-top = { path = \"../top\" }\nqet = { package = \"g-import-qet\", path = \"../qet\" }\n\n[dev-dependencies]\ng-import-qet = { path = \"../qet\" }\n\n[target.'cfg(unix)'.build-dependencies]\ng-import-qet = { path = \"../qet\" }\n";
        let edges = manifest_edges(manifest).unwrap();
        let to: Vec<(&str, &str, Kind)> = edges
            .iter()
            .map(|e| (e.from.as_str(), e.to.as_str(), e.kind))
            .collect();
        assert_eq!(
            to,
            [
                ("g-fuzz", "g-top", Kind::Normal),
                ("g-fuzz", "g-import-qet", Kind::Normal),
                ("g-fuzz", "g-import-qet", Kind::Dev),
                ("g-fuzz", "g-import-qet", Kind::Build),
            ]
        );
        let root = std::env::temp_dir().join(format!("ew-xtask-confined-{}", std::process::id()));
        std::fs::create_dir_all(root.join("fuzz")).unwrap();
        std::fs::write(root.join("fuzz/Cargo.toml"), manifest).unwrap();
        let mut p = policy();
        p.confined[0].manifests = vec!["fuzz/Cargo.toml".to_owned()];
        let v = check_confined_manifests(&p, &root).unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            v,
            [
                "confined: `g-fuzz` (fuzz/Cargo.toml) depends on `g-import-qet` (dependency); only g-top may (ADR-0024 decision 4)",
                "confined: `g-fuzz` (fuzz/Cargo.toml) depends on `g-import-qet` (dev-dependency); only g-top may (ADR-0024 decision 4)",
                "confined: `g-fuzz` (fuzz/Cargo.toml) depends on `g-import-qet` (build-dependency); only g-top may (ADR-0024 decision 4)",
            ]
        );
    }

    /// A package without an `[allowed_edges]` entry may depend on what its manifest says; the licence boundary
    /// still holds for it.
    #[test]
    fn a_package_without_an_entry_is_not_restricted() {
        let mut g = graph(&clean_edges());
        g.workspace.insert("g-new".into());
        g.edges.push(Edge {
            from: "g-new".into(),
            to: "g-sql".into(),
            kind: Kind::Normal,
        });
        assert_eq!(check(&policy(), &g), Vec::<String>::new());
    }

    #[test]
    fn policy_naming_unknown_crate_fails() {
        let text = POLICY.replace(r#""xtask" = []"#, "\"xtask\" = []\n\"g-gone\" = []");
        let p = Policy::parse(&text).unwrap();
        let v = check(&p, &graph(&clean_edges()));
        assert_violation(
            &v,
            "policy: [allowed_edges] names `g-gone`, which is not a workspace package",
        );

        let text = POLICY.replace(r#"owners = ["g-sql"]"#, r#"owners = ["g-sql", "g-nowhere"]"#);
        let p = Policy::parse(&text).unwrap();
        let v = check(&p, &graph(&clean_edges()));
        assert_violation(&v, "names `g-nowhere`, which is not a workspace package");
    }

    #[test]
    fn wildcard_matching() {
        assert!(wildcard_match("egui_*", "egui_tiles"));
        assert!(wildcard_match("egui_*", "egui_"));
        assert!(!wildcard_match("egui_*", "egui"));
        assert!(!wildcard_match("egui_*", "my_egui_x"));
        assert!(wildcard_match("ew-import-*", "ew-import-qet"));
        assert!(wildcard_match("a*b*c", "a-x-b-y-c"));
        assert!(!wildcard_match("a*b*c", "a-x-c"));
        assert!(wildcard_match("rusqlite", "rusqlite"));
        assert!(!wildcard_match("rusqlite", "rusqlite2"));
    }

    /// A fixture crate that names `std::fs` fails the purity check, in code but not in a comment or a test
    /// module; the workspace's importers pass it.
    #[test]
    fn the_importer_purity_check_finds_file_access_and_passes_the_workspace() {
        // A one-line `#[cfg(test)] use …;` skips only itself, and a grouped import names its module.
        let fixture = "use std::fs;\n// std::net in a comment\nfn f() { let _ = File::open(\"x\"); }\n#[cfg(test)]\nmod tests {\n    use std::env;\n}\nfn g() {}\n#[cfg(test)] use std::net;\nuse std::process;\nuse std::{fs::read, env};\nuse std::{\n    io::Read,\n    env,\n};\nuse std::{collections::HashMap, io::{self, Read}};\n";
        assert_eq!(
            impure_lines("crates/ew-import-x/src/lib.rs", fixture),
            [
                "importer purity (NFR-SEC-18): crates/ew-import-x/src/lib.rs:1 names `std::fs`; an importer reads only the bytes it is given",
                "importer purity (NFR-SEC-18): crates/ew-import-x/src/lib.rs:3 names `File::`; an importer reads only the bytes it is given",
                "importer purity (NFR-SEC-18): crates/ew-import-x/src/lib.rs:10 names `std::process`; an importer reads only the bytes it is given",
                "importer purity (NFR-SEC-18): crates/ew-import-x/src/lib.rs:11 names `std::fs`; an importer reads only the bytes it is given",
                "importer purity (NFR-SEC-18): crates/ew-import-x/src/lib.rs:14 names `std::env`; an importer reads only the bytes it is given",
            ]
        );
    }
}
