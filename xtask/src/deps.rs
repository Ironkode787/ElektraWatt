// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-deps`: the crate-dependency rules of ARCH §3 and NFR-MAINT-01, driven by `ci/dep-rules.toml`.
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
    /// Permitted direct workspace-to-workspace edges, per dependent package.
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
    /// Features that may only be enabled on the listed dependency kinds (`cargo xtask check-features`).
    #[serde(default)]
    pub banned_features: Vec<crate::features::BannedFeature>,
    /// Crates that may only be dev-dependencies (`cargo xtask check-features`).
    #[serde(default)]
    pub banned_crates: Vec<String>,
    /// The source-level guard for a gated feature's readers (`cargo xtask check-raw-read`).
    #[serde(default)]
    pub raw_read: Option<crate::raw_read::RawReadPolicy>,
}

/// Layer membership of the workspace packages.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layers {
    /// MIT OR Apache-2.0 crates; closed under dependencies.
    pub permissive: Vec<String>,
    /// GPL application crates and binaries.
    pub gpl: Vec<String>,
    /// Development-only packages.
    pub tools: Vec<String>,
    /// Architecture crates that do not exist in the workspace yet.
    #[serde(default)]
    pub planned: Vec<String>,
}

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

/// Every workspace package is in exactly one layer and has an `allowed_edges` entry; the policy names no
/// package that is neither in the workspace nor planned.
fn check_policy_consistency(policy: &Policy, graph: &Graph, v: &mut Vec<String>) {
    let layers = &policy.layers;
    let mut layer_of: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (layer, names) in [
        ("permissive", &layers.permissive),
        ("gpl", &layers.gpl),
        ("tools", &layers.tools),
    ] {
        for name in names {
            layer_of.entry(name).or_default().push(layer);
        }
    }
    let planned: BTreeSet<&str> = layers.planned.iter().map(String::as_str).collect();

    for name in &graph.workspace {
        match layer_of.get(name.as_str()) {
            None => v.push(format!(
                "policy: workspace package `{name}` is in no layer of [layers]"
            )),
            Some(ls) if ls.len() > 1 => v.push(format!(
                "policy: package `{name}` is in more than one layer: {}",
                ls.join(", ")
            )),
            Some(_) => {}
        }
        if !policy.allowed_edges.contains_key(name) {
            v.push(format!(
                "policy: workspace package `{name}` has no [allowed_edges] entry"
            ));
        }
        if planned.contains(name.as_str()) {
            v.push(format!(
                "policy: `{name}` is listed as planned but exists in the workspace; move it to a layer"
            ));
        }
    }

    let unknown = |name: &str, place: &str, v: &mut Vec<String>| {
        if !graph.is_workspace(name) {
            v.push(format!(
                "policy: {place} names `{name}`, which is not a workspace package"
            ));
        }
    };
    for name in layer_of.keys() {
        unknown(name, "[layers]", v);
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
    let known_or_planned = |name: &str, place: &str, v: &mut Vec<String>| {
        if !graph.is_workspace(name) && !planned.contains(name) {
            v.push(format!(
                "policy: {place} names `{name}`, which is neither a workspace package nor planned"
            ));
        }
    };
    for rule in &policy.external_owner {
        for owner in &rule.owners {
            known_or_planned(owner, "[[external_owner]] owners", v);
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

/// Every workspace-to-workspace edge is allowed, and no permissive crate depends on a non-permissive one.
fn check_edges(policy: &Policy, graph: &Graph, v: &mut Vec<String>) {
    let empty = Vec::new();
    for e in graph.edges.iter().filter(|e| graph.is_workspace(&e.to)) {
        let allowed = policy.allowed_edges.get(&e.from).unwrap_or(&empty);
        let dev_allowed = policy.allowed_dev_edges.get(&e.from).unwrap_or(&empty);
        let ok = allowed.contains(&e.to) || (e.kind == Kind::Dev && dev_allowed.contains(&e.to));
        if !ok {
            v.push(format!(
                "edge: `{}` -> `{}` ({}) is not in [allowed_edges] (ARCH §3 dependency graph)",
                e.from,
                e.to,
                e.kind.label()
            ));
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
        planned = ["g-viewer"]

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
        owners = ["g-ui", "app", "g-viewer"]
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
            "edge: `g-model` -> `g-sql` (dependency) is not in [allowed_edges]",
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
    fn permissive_depending_on_gpl_fails_even_as_dev_dependency() {
        let v = with(("p-schema", "g-model", Kind::Dev));
        assert_violation(
            &v,
            "licence boundary: permissive crate `p-schema` depends on non-permissive `g-model`",
        );
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
    fn rusqlite_outside_owner_fails() {
        let v = with(("g-ui", "rusqlite", Kind::Normal));
        assert_violation(&v, "external: `g-ui` depends on `rusqlite`");
    }

    #[test]
    fn egui_wildcard_outside_owner_fails() {
        let v = with(("g-model", "egui_extras", Kind::Dev));
        assert_violation(
            &v,
            "external: `g-model` depends on `egui_extras` (dev-dependency)",
        );
    }

    #[test]
    fn hyphenated_egui_crate_outside_owner_fails() {
        let v = with(("g-model", "egui-winit", Kind::Normal));
        assert_violation(&v, "external: `g-model` depends on `egui-winit` (dependency)");
        // The owner may use it.
        let mut edges = clean_edges();
        edges.push(("g-ui", "egui-winit", Kind::Normal));
        assert_eq!(check(&policy(), &graph(&edges)), Vec::<String>::new());
    }

    /// The real owner rules cover the hyphenated sub-crates and the SQLite system crate.
    #[test]
    fn real_owner_rules_cover_hyphenated_crates() {
        let root = crate::workspace_root();
        let policy = Policy::load(&root.join("ci/dep-rules.toml")).expect("policy loads");
        let covered = |name: &str| {
            policy
                .external_owner
                .iter()
                .any(|r| r.crates.iter().any(|p| wildcard_match(p, name)))
        };
        for name in [
            "egui-winit",
            "egui-kittest",
            "egui_extras",
            "wgpu-types",
            "wgpu-core",
            "wgpu-hal",
            "libsqlite3-sys",
        ] {
            assert!(
                covered(name),
                "{name} is not covered by an [[external_owner]] rule"
            );
        }
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

    #[test]
    fn workspace_package_missing_from_policy_fails() {
        let mut g = graph(&clean_edges());
        g.workspace.insert("g-new".into());
        let v = check(&policy(), &g);
        assert_violation(&v, "policy: workspace package `g-new` is in no layer");
        assert_violation(
            &v,
            "policy: workspace package `g-new` has no [allowed_edges] entry",
        );
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
        assert_violation(
            &v,
            "names `g-nowhere`, which is neither a workspace package nor planned",
        );
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

    /// NFR-MAINT-01 acceptance: the real workspace satisfies the real policy.
    #[test]
    fn real_workspace_passes() {
        let root = crate::workspace_root();
        let policy = Policy::load(&root.join("ci/dep-rules.toml")).expect("policy loads");
        let graph = Graph::from_metadata(&crate::metadata().expect("cargo metadata runs"));
        assert!(
            graph.workspace.contains("ew-model"),
            "metadata lists the workspace"
        );
        assert_eq!(check(&policy, &graph), Vec::<String>::new());
    }

    /// The real policy rejects a GPL crate seeded into a permissive crate, as a dev-dependency too
    /// (NFR-MAINT-02 acceptance in dependency-rule form).
    #[test]
    fn real_policy_rejects_seeded_gpl_dependency_in_ew_format() {
        let root = crate::workspace_root();
        let policy = Policy::load(&root.join("ci/dep-rules.toml")).expect("policy loads");
        let mut graph = Graph::from_metadata(&crate::metadata().expect("cargo metadata runs"));
        graph.edges.push(Edge {
            from: "ew-format".into(),
            to: "ew-model".into(),
            kind: Kind::Dev,
        });
        let v = check(&policy, &graph);
        assert_violation(
            &v,
            "licence boundary: permissive crate `ew-format` depends on non-permissive `ew-model`",
        );
    }
}
