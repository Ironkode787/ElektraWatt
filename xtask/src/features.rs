// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-features`: the feature bans of 18 §5.2 CI-PR-09, driven by `[[banned_features]]` and
//! `banned_crates` in `ci/dep-rules.toml`.
//!
//! Test-only and unsafe features (`ew-commands/testkit`, `ew-viewmodel/testkit`, `ew/test-confirm`, egui
//! `inspection`, rhai `unchecked`) and the `egui_mcp` and `ew-testids` crates must never be enabled on a non-test
//! dependency edge. The check reads the
//! declared dependencies of every workspace package (`cargo metadata`: the features requested on each edge
//! and its kind) and fails when
//!
//! - a banned feature, or any workspace feature that reaches it, is requested on an edge whose kind is not in
//!   the rule's `allowed_kinds` and whose package is not in its `allowed_packages`;
//! - a package outside a rule's non-empty `allowed_packages` has a feature that reaches the banned feature:
//!   directly (`dep/feature` or `dep?/feature`) or through other workspace features, local or of other
//!   workspace packages. An opt-in feature would otherwise be a back door;
//! - a banned crate is a dependency of any kind other than `dev`;
//! - a workspace package's own `default` feature reaches a banned feature (its own, or one forwarded through
//!   a dependency); a package in the rule's `allowed_packages` is exempt for the forwarded case.
//!
//! "Reaches" is followed transitively over the workspace feature graph whose nodes are `(package, feature)`
//! pairs; a violation prints the chain of hops down to the banned feature.
//!
//! `[[uniform_test_features]]` keeps a test-only feature the same in every test build: each workspace package whose
//! test build reaches the crate requests the feature on a dev edge, so no package set builds the crate a second way
//! (T1). A missing request fails and names the manifest line to write.
//!
//! The shipped binaries' resolved graph (`cargo tree -e normal,build` over every workspace package with a binary
//! target) must hold no banned crate and no feature that is banned outside dev edges (doc 18 CI-R-09, the graph
//! half): a third-party crate could otherwise enable one where no workspace edge does.
//!
//! `[[required_features]]` is the opposite rule over the resolved graph: a feature that must stay enabled when
//! its crate is in the build (`wgpu-hal/gles`, which gives `wayland-sys` its `dlopen`), so that no build script
//! probes `pkg-config` for a system library.
//!
//! Building the list of packages from `cargo metadata` is separate from checking it, so the unit tests can
//! feed synthetic graphs with seeded violations.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::deps::Kind;

/// A feature that may only be enabled on dependency edges of the listed kinds or by the listed packages.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannedFeature {
    /// The crate that owns the feature.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The banned feature.
    pub feature: String,
    /// Dependency kinds on which enabling it is allowed (`normal`, `dev`, `build`); empty means nowhere.
    pub allowed_kinds: Vec<Kind>,
    /// Workspace packages that may enable the feature on a dependency edge of any kind.
    #[serde(default)]
    pub allowed_packages: Vec<String>,
}

/// A feature that must stay enabled somewhere in the resolved dependency graph whenever the crate is in it.
///
/// For a feature that keeps the build free of a system library: without `wgpu-hal/gles`, `wayland-sys` loses
/// its `dlopen` feature and its build script probes `pkg-config` for libwayland (WP-S2.01 spec section 4,
/// rule 5: no system package on any CI image).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredFeature {
    /// The crate that owns the feature.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The feature that must be enabled.
    pub feature: String,
    /// Why, in one line (printed with the violation).
    pub reason: String,
}

/// A package of the resolved graph with the features cargo enabled on it (unified over all targets).
#[derive(Debug, Clone)]
pub struct Resolved {
    /// Package name.
    pub name: String,
    /// The enabled features.
    pub features: Vec<String>,
}

/// The resolved packages of `cargo metadata` output (empty without a resolve).
pub fn resolved_from_metadata(metadata: &cargo_metadata::Metadata) -> Vec<Resolved> {
    let Some(resolve) = &metadata.resolve else {
        return Vec::new();
    };
    resolve
        .nodes
        .iter()
        .filter_map(|n| {
            let p = metadata.packages.iter().find(|p| p.id == n.id)?;
            Some(Resolved {
                name: p.name.to_string(),
                features: n.features.iter().map(ToString::to_string).collect(),
            })
        })
        .collect()
}

/// The violations of the required features: a crate of the graph that lacks one. A crate that is not in the
/// graph at all is not a violation (the rule only guards builds that contain it).
pub fn check_required(required: &[RequiredFeature], resolved: &[Resolved]) -> Vec<String> {
    let mut v = Vec::new();
    for r in required {
        for p in resolved.iter().filter(|p| p.name == r.krate) {
            if !p.features.contains(&r.feature) {
                v.push(format!(
                    "required feature: `{}` is in the build without its feature `{}`: {}",
                    r.krate, r.feature, r.reason
                ));
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

/// A test-only feature that every test build compiles the same way (T1; doc 18 §2.2).
///
/// Cargo keeps one build of a crate per feature set. A test-only feature that only some packages request on their
/// dev edges is on in one package set and off in the next, so after an edit the crate and every crate above it
/// compile twice. The rule: every workspace package whose test build reaches the crate (the crate itself included)
/// requests the feature on a dev-dependency edge. The feature stays banned outside dev edges by its
/// `[[banned_features]]` rule, so no shipped build has it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UniformTestFeature {
    /// The crate that owns the feature.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The test-only feature.
    pub feature: String,
}

/// The violations of the uniform test features: a package whose test build reaches the crate without requesting
/// the feature on a dev edge. Each names the manifest and the line to write.
pub fn check_uniform(rules: &[UniformTestFeature], packages: &[Package]) -> Vec<String> {
    let by_name: BTreeMap<&str, &Package> = packages.iter().map(|p| (p.name.as_str(), p)).collect();
    let mut v = Vec::new();
    for rule in rules {
        let Some(target) = by_name.get(rule.krate.as_str()) else {
            v.push(format!(
                "uniform test feature: `{}` is not a workspace package; remove its rule from ci/dep-rules.toml",
                rule.krate
            ));
            continue;
        };
        if !target.features.contains_key(&rule.feature) {
            v.push(format!(
                "uniform test feature: `{}` has no feature `{}`; remove its rule from ci/dep-rules.toml",
                rule.krate, rule.feature
            ));
            continue;
        }
        for p in packages {
            if !test_build_reaches(p, &rule.krate, &by_name) {
                continue;
            }
            let dev: Vec<&Dep> = p
                .deps
                .iter()
                .filter(|d| d.kind == Kind::Dev && d.name == rule.krate)
                .collect();
            if dev.iter().any(|d| d.features.contains(&rule.feature)) {
                continue;
            }
            let line = format!(
                "{} = {{ path = \"{}\", features = [\"{}\"] }}",
                rule.krate,
                relative_dir(&p.dir, &target.dir),
                rule.feature
            );
            let manifest = if p.dir.is_empty() {
                "Cargo.toml".to_owned()
            } else {
                format!("{}/Cargo.toml", p.dir)
            };
            if dev.is_empty() {
                v.push(format!(
                    "uniform test feature: the test build of `{}` reaches `{}` without its feature `{}`; add `{line}` \
                     under `[dev-dependencies]` in {manifest}",
                    p.name, rule.krate, rule.feature
                ));
            } else {
                v.push(format!(
                    "uniform test feature: the dev-dependency `{}` of `{}` does not request its feature `{}`; make it \
                     `{line}` in {manifest}",
                    rule.krate, p.name, rule.feature
                ));
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

/// Whether the test build of `p` (its own dependencies of every kind, then the normal and build dependencies of
/// those, transitively) compiles the workspace package `krate`. A package's test build always compiles itself.
fn test_build_reaches(p: &Package, krate: &str, by_name: &BTreeMap<&str, &Package>) -> bool {
    if p.name == krate {
        return true;
    }
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut stack: Vec<&str> = p.deps.iter().map(|d| d.name.as_str()).collect();
    while let Some(name) = stack.pop() {
        if name == krate {
            return true;
        }
        if !seen.insert(name) {
            continue;
        }
        if let Some(q) = by_name.get(name) {
            stack.extend(
                q.deps
                    .iter()
                    .filter(|d| d.kind != Kind::Dev)
                    .map(|d| d.name.as_str()),
            );
        }
    }
    false
}

/// The path from the workspace directory `from` to `to` (both `/`-separated and relative to the root), as a
/// manifest's `path = "…"` writes it.
fn relative_dir(from: &str, to: &str) -> String {
    let from: Vec<&str> = from.split('/').filter(|c| !c.is_empty()).collect();
    let to: Vec<&str> = to.split('/').filter(|c| !c.is_empty()).collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&to[common..]);
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// The violations of the shipping graph (doc 18 CI-R-09, the graph half): a banned crate, or a feature banned
/// outside dev edges, in the resolved graph of the shipped binaries (`cargo tree -e normal,build`), where a third
/// party could still enable it. `graph` maps each package name to its enabled features. Rules with
/// `allowed_packages` (`ew-model/raw-read`) and rules that allow normal edges are not test-only and are skipped.
pub fn check_shipping(
    rules: &Rules<'_>,
    graph: &BTreeMap<String, std::collections::BTreeSet<String>>,
    roots: &str,
) -> Vec<String> {
    let mut v = Vec::new();
    for krate in rules.crates.iter().filter(|c| graph.contains_key(*c)) {
        v.push(format!(
            "shipping graph: `{krate}` is in the build of {roots}; it may only be a dev-dependency (18 CI-R-09)"
        ));
    }
    for ban in rules
        .features
        .iter()
        .filter(|b| b.allowed_packages.is_empty() && !b.allowed_kinds.contains(&Kind::Normal))
    {
        if graph.get(&ban.krate).is_some_and(|fs| fs.contains(&ban.feature)) {
            v.push(format!(
                "shipping graph: `{}/{}` is enabled in the build of {roots}; it is test-only (18 CI-R-09)",
                ban.krate, ban.feature
            ));
        }
    }
    v.sort();
    v
}

/// The arguments of [`shipping_graph`]'s `cargo tree` call over `packages`: every target (`--target all`), normal
/// and build edges, one `<package>|<features>` line per package.
fn shipping_graph_args(packages: &[&str]) -> Vec<String> {
    let mut args: Vec<String> = ["tree", "--locked", "--target", "all"]
        .map(str::to_owned)
        .to_vec();
    for p in packages {
        args.extend(["-p".to_owned(), (*p).to_owned()]);
    }
    args.extend(["-e", "normal,build", "--prefix", "none", "--format", "{p}|{f}"].map(str::to_owned));
    args
}

/// The resolved graph of the shipped binaries for every target (`cargo tree --target all -e normal,build` over
/// `packages`, C43-p2#7: a platform-gated dependency counts too), as a map
/// of package name to its enabled features.
pub fn shipping_graph(
    root: &std::path::Path,
    packages: &[&str],
) -> anyhow::Result<BTreeMap<String, std::collections::BTreeSet<String>>> {
    let mut cmd = std::process::Command::new(crate::cargo());
    cmd.current_dir(root).args(shipping_graph_args(packages));
    let output = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("running `cargo tree`: {e}"))?;
    if !output.status.success() {
        anyhow::bail!(
            "`cargo tree` failed ({}):\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(parse_tree(&String::from_utf8_lossy(&output.stdout)))
}

/// Parses `cargo tree --prefix none --format "{p}|{f}"` lines into package name → enabled features.
pub(crate) fn parse_tree(stdout: &str) -> BTreeMap<String, std::collections::BTreeSet<String>> {
    let mut graph = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    for line in stdout.lines() {
        let Some((package, features)) = line.split_once('|') else {
            continue;
        };
        let Some(name) = package.split_whitespace().next() else {
            continue;
        };
        graph.entry(name.to_owned()).or_default().extend(
            features
                .split(',')
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .map(str::to_owned),
        );
    }
    graph
}

/// The policy of the check, borrowed from `ci/dep-rules.toml`.
pub struct Rules<'a> {
    /// Banned features.
    pub features: &'a [BannedFeature],
    /// Crates that may only appear as dev-dependencies.
    pub crates: &'a [String],
}

/// One declared dependency of a workspace package.
#[derive(Debug, Clone)]
pub struct Dep {
    /// The name the manifest uses for it (the rename, if any).
    pub alias: String,
    /// The package depended on.
    pub name: String,
    /// Dependency kind.
    pub kind: Kind,
    /// Features requested on this edge.
    pub features: Vec<String>,
}

/// A workspace package: its declared dependencies and its own `[features]` table.
#[derive(Debug, Clone)]
pub struct Package {
    /// Package name.
    pub name: String,
    /// Its directory relative to the workspace root, `/`-separated (`crates/ew-ui`; empty for the root).
    pub dir: String,
    /// Declared direct dependencies.
    pub deps: Vec<Dep>,
    /// The package's `[features]` table.
    pub features: BTreeMap<String, Vec<String>>,
}

/// Builds the workspace packages from `cargo metadata` output.
pub fn from_metadata(metadata: &cargo_metadata::Metadata) -> Vec<Package> {
    metadata
        .workspace_packages()
        .iter()
        .map(|p| Package {
            name: p.name.to_string(),
            dir: p
                .manifest_path
                .parent()
                .and_then(|d| d.strip_prefix(&metadata.workspace_root).ok())
                .map(|d| d.as_str().replace('\\', "/"))
                .unwrap_or_default(),
            deps: p
                .dependencies
                .iter()
                .map(|d| Dep {
                    alias: d.rename.clone().unwrap_or_else(|| d.name.clone()),
                    name: d.name.clone(),
                    kind: match d.kind {
                        cargo_metadata::DependencyKind::Development => Kind::Dev,
                        cargo_metadata::DependencyKind::Build => Kind::Build,
                        _ => Kind::Normal,
                    },
                    features: d.features.clone(),
                })
                .collect(),
            features: p.features.clone().into_iter().collect(),
        })
        .collect()
}

/// Checks `packages` against `rules` and returns every violation, sorted.
pub fn check(rules: &Rules<'_>, packages: &[Package]) -> Vec<String> {
    let reaches: Vec<Reach> = rules
        .features
        .iter()
        .map(|ban| Reach::compute(ban, packages))
        .collect();
    let mut v = Vec::new();
    for p in packages {
        for d in &p.deps {
            if d.kind != Kind::Dev && rules.crates.contains(&d.name) {
                v.push(format!(
                    "feature ban: `{}` has `{}` as a {}; the crate may only be a dev-dependency (18 §5.2 CI-PR-09)",
                    p.name,
                    d.name,
                    d.kind.label()
                ));
            }
        }
        for (ban, reach) in rules.features.iter().zip(&reaches) {
            check_edges(ban, reach, p, &mut v);
            check_forwarding(ban, reach, p, &mut v);
            check_default_features(ban, reach, p, &mut v);
        }
    }
    v.sort();
    v.dedup();
    v
}

/// A `(package, feature)` node of the workspace feature graph.
type Node = (String, String);

/// The nodes that enable one banned feature through any chain of workspace features, each with its next hop
/// toward the banned feature (`None` for the banned feature itself).
struct Reach {
    next: BTreeMap<Node, Option<Node>>,
}

impl Reach {
    /// Computes the set as a deterministic fixed point; a cycle of features adds nothing and terminates.
    fn compute(ban: &BannedFeature, packages: &[Package]) -> Self {
        let mut sorted: Vec<&Package> = packages.iter().collect();
        sorted.sort_by(|a, b| a.name.cmp(&b.name));
        let mut next: BTreeMap<Node, Option<Node>> = BTreeMap::new();
        next.insert((ban.krate.clone(), ban.feature.clone()), None);
        loop {
            let mut changed = false;
            for p in &sorted {
                for (f, items) in &p.features {
                    let node = (p.name.clone(), f.clone());
                    if next.contains_key(&node) {
                        continue;
                    }
                    let hop = items
                        .iter()
                        .map(|item| leads_to(p, item))
                        .find(|target| next.contains_key(target));
                    if let Some(target) = hop {
                        next.insert(node, Some(target));
                        changed = true;
                    }
                }
            }
            if !changed {
                return Self { next };
            }
        }
    }

    fn contains(&self, package: &str, feature: &str) -> bool {
        self.next.contains_key(&(package.to_owned(), feature.to_owned()))
    }

    /// The hop after `node`, if it is in the set and not the banned feature itself.
    fn hop(&self, node: &Node) -> Option<&Node> {
        self.next.get(node)?.as_ref()
    }

    /// The hops from `(package, feature)` down to the banned feature, joined with ` -> `.
    fn chain(&self, package: &str, feature: &str) -> String {
        let mut node = (package.to_owned(), feature.to_owned());
        let mut out = vec![format!("{}/{}", node.0, node.1)];
        while let Some(next) = self.hop(&node) {
            out.push(format!("{}/{}", next.0, next.1));
            node = next.clone();
        }
        out.join(" -> ")
    }
}

/// The node a `[features]` item of `p` enables: `dep/feature` (or `dep?/feature`) leads to the feature of the
/// dependency's package, any other item to a feature of `p` itself.
fn leads_to(p: &Package, item: &str) -> Node {
    match forwarded(p, item) {
        Some((krate, feature)) => (krate.to_owned(), feature.to_owned()),
        None => (
            p.name.clone(),
            item.strip_prefix("dep:").unwrap_or(item).to_owned(),
        ),
    }
}

/// A dependency edge of a kind or package the ban does not allow must not request a feature that reaches
/// the banned one.
fn check_edges(ban: &BannedFeature, reach: &Reach, p: &Package, v: &mut Vec<String>) {
    if ban.allowed_packages.contains(&p.name) {
        return;
    }
    for d in p.deps.iter().filter(|d| !ban.allowed_kinds.contains(&d.kind)) {
        for f in d.features.iter().filter(|f| reach.contains(&d.name, f)) {
            if d.name == ban.krate && *f == ban.feature {
                v.push(format!(
                    "feature ban: `{}` enables `{}/{}` on a {}; allowed on: {} (18 §5.2 CI-PR-09)",
                    p.name,
                    d.name,
                    ban.feature,
                    d.kind.label(),
                    allowed_label(ban)
                ));
            } else {
                v.push(format!(
                    "feature ban: `{}` enables `{}/{f}` on a {}, which enables `{}/{}` ({}); allowed on: {} (18 §5.2 CI-PR-09)",
                    p.name,
                    d.name,
                    d.kind.label(),
                    ban.krate,
                    ban.feature,
                    reach.chain(&d.name, f),
                    allowed_label(ban)
                ));
            }
        }
    }
}

fn allowed_label(ban: &BannedFeature) -> String {
    let kinds = if ban.allowed_kinds.is_empty() {
        "nowhere".to_owned()
    } else {
        ban.allowed_kinds
            .iter()
            .map(|k| k.label())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if ban.allowed_packages.is_empty() {
        kinds
    } else {
        format!("{kinds}; in packages: {}", ban.allowed_packages.join(", "))
    }
}

/// Splits a `[features]` item of the form `dep/feature` or `dep?/feature` (with an optional `dep:` prefix)
/// into the crate it names, resolving a rename through the package's dependencies, and the feature.
fn forwarded<'a>(p: &'a Package, item: &'a str) -> Option<(&'a str, &'a str)> {
    let item = item.strip_prefix("dep:").unwrap_or(item);
    let (dep, feature) = item.split_once('/')?;
    let dep = dep.strip_suffix('?').unwrap_or(dep);
    let krate = p
        .deps
        .iter()
        .find(|d| d.alias == dep)
        .map_or(dep, |d| d.name.as_str());
    Some((krate, feature))
}

/// A package outside the `allowed_packages` of a package-restricted ban must not have a feature that reaches
/// the banned feature, directly or through other workspace features. `default` is left to
/// [`check_default_features`].
fn check_forwarding(ban: &BannedFeature, reach: &Reach, p: &Package, v: &mut Vec<String>) {
    if ban.allowed_packages.is_empty() || ban.allowed_packages.contains(&p.name) || p.name == ban.krate {
        return;
    }
    for f in p.features.keys().filter(|f| *f != "default") {
        let node = (p.name.clone(), f.clone());
        let Some(hop) = reach.hop(&node) else {
            continue;
        };
        let direct = hop.0 == ban.krate && hop.1 == ban.feature;
        let chain = if direct {
            String::new()
        } else {
            format!(" ({})", reach.chain(&p.name, f))
        };
        v.push(format!(
            "feature ban: `{}` forwards `{}/{}` from its feature `{f}`{chain}; only {} may enable it",
            p.name,
            ban.krate,
            ban.feature,
            ban.allowed_packages.join(", ")
        ));
    }
}

/// The package's `default` feature, followed through the workspace features, must not reach a banned feature.
fn check_default_features(ban: &BannedFeature, reach: &Reach, p: &Package, v: &mut Vec<String>) {
    if !reach.contains(&p.name, "default") {
        return;
    }
    if ban.krate == p.name {
        v.push(format!(
            "feature ban: the `default` features of `{}` enable its banned feature `{}`",
            p.name, ban.feature
        ));
        return;
    }
    if ban.allowed_packages.contains(&p.name) {
        return;
    }
    // The first hop that leaves the package, and the feature of the package it leaves from.
    let mut node = (p.name.clone(), "default".to_owned());
    while let Some(next) = reach.hop(&node) {
        if next.0 != p.name {
            let direct = next.0 == ban.krate && next.1 == ban.feature;
            let chain = if direct {
                String::new()
            } else {
                format!(" ({})", reach.chain(&p.name, "default"))
            };
            v.push(format!(
                "feature ban: the `default` features of `{}` enable `{}/{}` (via `{}`){chain}",
                p.name, next.0, next.1, node.1
            ));
            return;
        }
        node = next.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ban(krate: &str, feature: &str, allowed: &[Kind]) -> BannedFeature {
        BannedFeature {
            krate: krate.into(),
            feature: feature.into(),
            allowed_kinds: allowed.to_vec(),
            allowed_packages: Vec::new(),
        }
    }

    fn raw_read_ban() -> BannedFeature {
        BannedFeature {
            allowed_packages: ["ew-derive", "ew-commands", "ew-store"]
                .map(str::to_owned)
                .to_vec(),
            ..ban("ew-model", "raw-read", &[Kind::Dev])
        }
    }

    fn dep(name: &str, kind: Kind, features: &[&str]) -> Dep {
        Dep {
            alias: name.into(),
            name: name.into(),
            kind,
            features: features.iter().map(|f| (*f).to_owned()).collect(),
        }
    }

    fn package(name: &str, deps: Vec<Dep>, features: &[(&str, &[&str])]) -> Package {
        Package {
            name: name.into(),
            dir: format!("crates/{name}"),
            deps,
            features: features
                .iter()
                .map(|(k, vs)| ((*k).to_owned(), vs.iter().map(|s| (*s).to_owned()).collect()))
                .collect(),
        }
    }

    fn run(packages: &[Package]) -> Vec<String> {
        let crates = vec!["egui_mcp".to_owned()];
        let bans = vec![
            ban("ew-ids", "testkit", &[Kind::Dev]),
            ban("rhai", "unchecked", &[]),
            raw_read_ban(),
        ];
        check(
            &Rules {
                features: &bans,
                crates: &crates,
            },
            packages,
        )
    }

    #[test]
    fn clean_graph_passes() {
        let packages = [
            package("a", vec![dep("ew-ids", Kind::Normal, &["serde"])], &[]),
            package("b", vec![dep("ew-ids", Kind::Dev, &["testkit"])], &[]),
            package("c", vec![dep("egui_mcp", Kind::Dev, &[])], &[]),
            package("ew-ids", vec![], &[("testkit", &[]), ("default", &[])]),
        ];
        assert_eq!(run(&packages), Vec::<String>::new());
    }

    #[test]
    fn banned_feature_on_normal_edge_fails_but_dev_edge_passes() {
        let v = run(&[package("a", vec![dep("ew-ids", Kind::Normal, &["testkit"])], &[])]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`a` enables `ew-ids/testkit` on a dependency; allowed on: dev-dependency"),
            "{v:#?}"
        );
        let v = run(&[package("a", vec![dep("ew-ids", Kind::Build, &["testkit"])], &[])]);
        assert!(v[0].contains("on a build-dependency"), "{v:#?}");
        let v = run(&[package("a", vec![dep("ew-ids", Kind::Dev, &["testkit"])], &[])]);
        assert_eq!(v, Vec::<String>::new());
    }

    #[test]
    fn feature_allowed_nowhere_fails_on_dev_edge_too() {
        let v = run(&[package("a", vec![dep("rhai", Kind::Dev, &["unchecked"])], &[])]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`rhai/unchecked`") && v[0].contains("allowed on: nowhere"),
            "{v:#?}"
        );
    }

    #[test]
    fn default_feature_reaching_a_banned_feature_fails() {
        // Forwarded to a dependency directly, through a local feature with `?` and a rename, and as the
        // banned crate's own default.
        let direct = package(
            "a",
            vec![dep("ew-ids", Kind::Dev, &[])],
            &[("default", &["ew-ids/testkit"])],
        );
        let v = run(&[direct]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`default` features of `a` enable `ew-ids/testkit`"),
            "{v:#?}"
        );

        let mut renamed = dep("rhai", Kind::Normal, &[]);
        renamed.alias = "script".into();
        let indirect = package(
            "b",
            vec![renamed],
            &[("default", &["fast"]), ("fast", &["script?/unchecked"])],
        );
        let v = run(&[indirect]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("`rhai/unchecked` (via `fast`)"), "{v:#?}");

        let own = package("ew-ids", vec![], &[("default", &["testkit"]), ("testkit", &[])]);
        let v = run(&[own]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("banned feature `testkit`"), "{v:#?}");

        // An opt-in feature that forwards it is fine.
        let opt_in = package(
            "c",
            vec![dep("ew-ids", Kind::Dev, &[])],
            &[("default", &[]), ("fixtures", &["ew-ids/testkit"])],
        );
        assert_eq!(run(&[opt_in]), Vec::<String>::new());
    }

    #[test]
    fn raw_read_on_a_normal_edge_of_another_package_fails() {
        for kind in [Kind::Normal, Kind::Build] {
            let v = run(&[package(
                "ew-scene",
                vec![dep("ew-model", kind, &["raw-read"])],
                &[],
            )]);
            assert_eq!(v.len(), 1, "{v:#?}");
            assert!(
                v[0].contains("`ew-scene` enables `ew-model/raw-read`")
                    && v[0].contains(
                        "allowed on: dev-dependency; in packages: ew-derive, ew-commands, ew-store"
                    ),
                "{v:#?}"
            );
        }
    }

    #[test]
    fn raw_read_forwarded_by_another_package_fails() {
        let mut renamed = dep("ew-model", Kind::Normal, &[]);
        renamed.alias = "model".into();
        for (deps, item) in [
            (vec![dep("ew-model", Kind::Normal, &[])], "ew-model/raw-read"),
            (vec![dep("ew-model", Kind::Normal, &[])], "ew-model?/raw-read"),
            (vec![renamed], "model/raw-read"),
        ] {
            let v = run(&[package("ew-scene", deps, &[("debug", &[item])])]);
            assert_eq!(v.len(), 1, "{item}: {v:#?}");
            assert!(
                v[0].contains("`ew-scene` forwards `ew-model/raw-read` from its feature `debug`; only "),
                "{v:#?}"
            );
        }
    }

    #[test]
    fn raw_read_and_forwarding_features_pass_where_allowed() {
        let derive_forwarding = |features: &[(&str, &[&str])]| {
            package("ew-derive", vec![dep("ew-model", Kind::Normal, &[])], features)
        };
        let cases: [(&str, Vec<Package>); 6] = [
            (
                "raw-read in an allowed package",
                ["ew-derive", "ew-commands", "ew-store"]
                    .map(|n| package(n, vec![dep("ew-model", Kind::Normal, &["raw-read"])], &[]))
                    .to_vec(),
            ),
            (
                "raw-read on a dev edge",
                vec![package(
                    "ew-scene",
                    vec![dep("ew-model", Kind::Dev, &["raw-read"])],
                    &[],
                )],
            ),
            (
                "raw-read forwarded by an allowed package",
                vec![derive_forwarding(&[
                    ("default", &["ew-model/raw-read"]),
                    ("debug", &["ew-model/raw-read"]),
                ])],
            ),
            (
                "an allowed package exposing a forwarding feature",
                vec![derive_forwarding(&[
                    ("raw", &["ew-model/raw-read"]),
                    ("default", &["raw"]),
                ])],
            ),
            (
                "a dev edge to a forwarding feature",
                vec![
                    derive_forwarding(&[("raw", &["ew-model/raw-read"])]),
                    package("ew-reports", vec![dep("ew-derive", Kind::Dev, &["raw"])], &[]),
                ],
            ),
            (
                "a forwarding cycle",
                vec![package("cyc", vec![], &[("a", &["b"]), ("b", &["a"])])],
            ),
        ];
        for (case, packages) in cases {
            assert_eq!(run(&packages), Vec::<String>::new(), "{case}");
        }
    }

    #[test]
    fn banned_crate_default_enabling_its_own_feature_fails() {
        let own = package(
            "ew-model",
            vec![],
            &[("default", &["raw-read"]), ("raw-read", &[])],
        );
        let v = run(&[own]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("banned feature `raw-read`"), "{v:#?}");
    }

    #[test]
    fn raw_read_forwarded_two_levels_fails() {
        let derive = package(
            "ew-derive",
            vec![dep("ew-model", Kind::Normal, &[])],
            &[("raw", &["ew-model/raw-read"])],
        );
        // Requested on an edge.
        let v = run(&[
            derive.clone(),
            package("ew-reports", vec![dep("ew-derive", Kind::Normal, &["raw"])], &[]),
        ]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`ew-reports` enables `ew-derive/raw`")
                && v[0].contains("ew-derive/raw -> ew-model/raw-read"),
            "{v:#?}"
        );
        // Forwarded from an own feature.
        let v = run(&[
            derive.clone(),
            package(
                "ew-reports",
                vec![dep("ew-derive", Kind::Normal, &[])],
                &[("x", &["ew-derive/raw"])],
            ),
        ]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`ew-reports` forwards `ew-model/raw-read` from its feature `x`")
                && v[0].contains("ew-reports/x -> ew-derive/raw -> ew-model/raw-read"),
            "{v:#?}"
        );
        // Three hops through a local feature, on a build edge.
        let deep = package(
            "ew-derive",
            vec![dep("ew-model", Kind::Normal, &[])],
            &[("raw", &["inner"]), ("inner", &["ew-model?/raw-read"])],
        );
        let v = run(&[
            deep,
            package("ew-reports", vec![dep("ew-derive", Kind::Build, &["raw"])], &[]),
        ]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`ew-reports` enables `ew-derive/raw` on a build-dependency")
                && v[0].contains("ew-derive/raw -> ew-derive/inner -> ew-model/raw-read"),
            "{v:#?}"
        );
    }

    #[test]
    fn testkit_through_an_opt_in_feature_on_a_normal_edge_fails() {
        let fix = package(
            "fix",
            vec![dep("ew-ids", Kind::Dev, &[])],
            &[("default", &[]), ("fixtures", &["ew-ids/testkit"])],
        );
        assert_eq!(run(std::slice::from_ref(&fix)), Vec::<String>::new());
        let app = package("app", vec![dep("fix", Kind::Normal, &["fixtures"])], &[]);
        let v = run(&[fix, app]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`app` enables `fix/fixtures`") && v[0].contains("fix/fixtures -> ew-ids/testkit"),
            "{v:#?}"
        );
    }

    fn uniform(krate: &str, feature: &str) -> UniformTestFeature {
        UniformTestFeature {
            krate: krate.to_owned(),
            feature: feature.to_owned(),
        }
    }

    /// commands <- api <- ui (normal edges); commands requests its own `testkit`, api and ui request it on a dev edge.
    fn uniform_graph() -> Vec<Package> {
        vec![
            package(
                "ew-commands",
                vec![dep("ew-commands", Kind::Dev, &["testkit"])],
                &[("testkit", &[])],
            ),
            package(
                "ew-api",
                vec![
                    dep("ew-commands", Kind::Normal, &[]),
                    dep("ew-commands", Kind::Dev, &["testkit"]),
                ],
                &[],
            ),
            package(
                "ew-ui",
                vec![
                    dep("ew-api", Kind::Normal, &[]),
                    dep("ew-commands", Kind::Dev, &["testkit"]),
                ],
                &[],
            ),
            package("ew-units", vec![], &[]),
        ]
    }

    #[test]
    fn uniform_test_feature_clean_graph_passes() {
        assert_eq!(
            check_uniform(&[uniform("ew-commands", "testkit")], &uniform_graph()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn uniform_test_feature_missing_dev_edge_names_the_line() {
        let mut packages = uniform_graph();
        packages[2].deps.pop();
        let mut app = package("elektrawatt", vec![dep("ew-ui", Kind::Normal, &[])], &[]);
        app.dir = "bin/elektrawatt".to_owned();
        packages.push(app);
        assert_eq!(
            check_uniform(&[uniform("ew-commands", "testkit")], &packages),
            [
                "uniform test feature: the test build of `elektrawatt` reaches `ew-commands` without its feature \
                 `testkit`; add `ew-commands = { path = \"../../crates/ew-commands\", features = [\"testkit\"] }` \
                 under `[dev-dependencies]` in bin/elektrawatt/Cargo.toml",
                "uniform test feature: the test build of `ew-ui` reaches `ew-commands` without its feature \
                 `testkit`; add `ew-commands = { path = \"../ew-commands\", features = [\"testkit\"] }` under \
                 `[dev-dependencies]` in crates/ew-ui/Cargo.toml",
            ]
        );
    }

    #[test]
    fn uniform_test_feature_on_a_dev_edge_without_it_fails() {
        let mut packages = uniform_graph();
        packages[1].deps[1].features.clear();
        assert_eq!(
            check_uniform(&[uniform("ew-commands", "testkit")], &packages),
            [
                "uniform test feature: the dev-dependency `ew-commands` of `ew-api` does not request its feature \
              `testkit`; make it `ew-commands = { path = \"../ew-commands\", features = [\"testkit\"] }` in \
              crates/ew-api/Cargo.toml"
            ]
        );
    }

    #[test]
    fn uniform_test_feature_needs_the_crates_own_dev_edge() {
        let mut packages = uniform_graph();
        packages[0].deps.clear();
        assert_eq!(
            check_uniform(&[uniform("ew-commands", "testkit")], &packages),
            [
                "uniform test feature: the test build of `ew-commands` reaches `ew-commands` without its feature \
              `testkit`; add `ew-commands = { path = \".\", features = [\"testkit\"] }` under \
              `[dev-dependencies]` in crates/ew-commands/Cargo.toml"
            ]
        );
    }

    #[test]
    fn uniform_test_feature_follows_dev_edges_one_step_only() {
        // A dev-dependency's normal edges are in the test build; a dev-dependency's dev edges are not.
        let packages = vec![
            package(
                "ew-commands",
                vec![dep("ew-commands", Kind::Dev, &["testkit"])],
                &[("testkit", &[])],
            ),
            package(
                "ew-derive",
                vec![dep("ew-commands", Kind::Dev, &["testkit"])],
                &[],
            ),
            package("ew-reports", vec![dep("ew-derive", Kind::Normal, &[])], &[]),
            package("ew-scene", vec![dep("ew-reports", Kind::Dev, &[])], &[]),
            package("ew-mfg", vec![dep("ew-script", Kind::Dev, &[])], &[]),
            package("ew-script", vec![dep("ew-commands", Kind::Normal, &[])], &[]),
        ];
        assert_eq!(
            check_uniform(&[uniform("ew-commands", "testkit")], &packages),
            [
                "uniform test feature: the test build of `ew-mfg` reaches `ew-commands` without its feature \
                 `testkit`; add `ew-commands = { path = \"../ew-commands\", features = [\"testkit\"] }` under \
                 `[dev-dependencies]` in crates/ew-mfg/Cargo.toml",
                "uniform test feature: the test build of `ew-script` reaches `ew-commands` without its feature \
                 `testkit`; add `ew-commands = { path = \"../ew-commands\", features = [\"testkit\"] }` under \
                 `[dev-dependencies]` in crates/ew-script/Cargo.toml",
            ]
        );
    }

    #[test]
    fn uniform_test_feature_rule_for_a_missing_crate_or_feature_fails() {
        assert_eq!(
            check_uniform(&[uniform("ew-gone", "testkit")], &uniform_graph()),
            [
                "uniform test feature: `ew-gone` is not a workspace package; remove its rule from ci/dep-rules.toml"
            ]
        );
        assert_eq!(
            check_uniform(&[uniform("ew-units", "testkit")], &uniform_graph()),
            [
                "uniform test feature: `ew-units` has no feature `testkit`; remove its rule from ci/dep-rules.toml"
            ]
        );
    }

    #[test]
    fn relative_dirs_as_a_manifest_writes_them() {
        assert_eq!(
            relative_dir("crates/ew-ui", "crates/ew-commands"),
            "../ew-commands"
        );
        assert_eq!(
            relative_dir("bin/ew", "crates/ew-commands"),
            "../../crates/ew-commands"
        );
        assert_eq!(relative_dir("crates/ew-commands", "crates/ew-commands"), ".");
    }

    /// The shipping graph's `cargo tree` covers every target (`--target all`) and no dev edge.
    #[test]
    fn the_shipping_graph_command_covers_every_target() {
        assert_eq!(
            shipping_graph_args(&["elektrawatt", "ew"]),
            [
                "tree",
                "--locked",
                "--target",
                "all",
                "-p",
                "elektrawatt",
                "-p",
                "ew",
                "-e",
                "normal,build",
                "--prefix",
                "none",
                "--format",
                "{p}|{f}",
            ]
        );
    }

    #[test]
    fn shipping_graph_with_a_test_only_crate_or_feature_fails() {
        let bans = vec![
            ban("ew-commands", "testkit", &[Kind::Dev]),
            ban("rhai", "unchecked", &[]),
            raw_read_ban(),
            ban("regex", "std", &[Kind::Normal, Kind::Dev]),
        ];
        let crates = vec!["ew-testids".to_owned()];
        let rules = Rules {
            features: &bans,
            crates: &crates,
        };
        let graph = parse_tree(
            "ew-commands v0.1.0 (/w/crates/ew-commands)|\n\
             ew-model v0.1.0 (/w/crates/ew-model)|raw-read\n\
             regex v1.13.1|std\n",
        );
        assert_eq!(check_shipping(&rules, &graph, "app"), Vec::<String>::new());
        let graph = parse_tree(
            "ew-commands v0.1.0 (/w/crates/ew-commands)|default,testkit\n\
             ew-testids v0.1.0 (/w/crates/ew-testids)|\n",
        );
        assert_eq!(
            check_shipping(&rules, &graph, "app"),
            [
                "shipping graph: `ew-commands/testkit` is enabled in the build of app; it is test-only (18 CI-R-09)",
                "shipping graph: `ew-testids` is in the build of app; it may only be a dev-dependency (18 CI-R-09)",
            ]
        );
    }

    fn resolved(name: &str, features: &[&str]) -> Resolved {
        Resolved {
            name: name.to_owned(),
            features: features.iter().map(|f| (*f).to_owned()).collect(),
        }
    }

    #[test]
    fn the_desktop_shipping_graph_enables_the_native_clipboard() {
        let root = crate::workspace_root();
        let policy = crate::deps::Policy::load(&root.join("ci/dep-rules.toml")).expect("policy loads");
        for krate in ["eframe", "egui-winit"] {
            assert!(
                policy
                    .required_features
                    .iter()
                    .any(|r| r.krate == krate && r.feature == "clipboard"),
                "{krate}/clipboard is required by the policy"
            );
        }
        for target in ["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"] {
            let output = std::process::Command::new(crate::cargo())
                .current_dir(&root)
                .args([
                    "tree",
                    "--locked",
                    "-p",
                    "elektrawatt",
                    "--target",
                    target,
                    "-e",
                    "normal,build",
                    "--prefix",
                    "none",
                    "--format",
                    "{p}|{f}",
                ])
                .output()
                .expect("cargo tree runs");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success(),
                "shipping graph for {target} failed ({}):\nstdout:\n{stdout}\nstderr:\n{stderr}",
                output.status
            );
            let mut graph = BTreeMap::<&str, std::collections::BTreeSet<&str>>::new();
            for line in stdout.lines() {
                let Some((package, features)) = line.split_once('|') else {
                    continue;
                };
                let Some(name) = package.split_whitespace().next() else {
                    continue;
                };
                graph.entry(name).or_default().extend(
                    features
                        .split(',')
                        .map(str::trim)
                        .filter(|feature| !feature.is_empty()),
                );
            }
            for (krate, feature) in [
                ("eframe", "clipboard"),
                ("egui-winit", "clipboard"),
                ("egui-winit", "arboard"),
            ] {
                assert!(
                    graph.get(krate).is_some_and(|fs| fs.contains(feature)),
                    "shipping graph for {target} must enable {krate}/{feature}"
                );
            }
            assert!(graph.contains_key("arboard"), "{target} must contain arboard");
            if target == "x86_64-unknown-linux-gnu" {
                assert!(
                    graph
                        .get("egui-winit")
                        .is_some_and(|fs| fs.contains("smithay-clipboard")),
                    "{target} must enable egui-winit/smithay-clipboard"
                );
                assert!(
                    graph.contains_key("smithay-clipboard"),
                    "{target} must contain smithay-clipboard"
                );
            }
            for feature in ["default_fonts", "glow", "persistence"] {
                assert!(
                    !graph.get("eframe").is_some_and(|fs| fs.contains(feature)),
                    "shipping graph for {target} must keep eframe/{feature} off"
                );
            }
        }
    }

    fn required(krate: &str, feature: &str) -> RequiredFeature {
        RequiredFeature {
            krate: krate.to_owned(),
            feature: feature.to_owned(),
            reason: "no pkg-config".to_owned(),
        }
    }

    #[test]
    fn a_required_feature_must_be_on_when_the_crate_is_in_the_graph() {
        let rules = [required("wayland-sys", "dlopen")];
        assert!(check_required(&rules, &[resolved("wayland-sys", &["client", "dlopen"])]).is_empty());
        // The crate is not in the graph: nothing to guard.
        assert!(check_required(&rules, &[resolved("other", &[])]).is_empty());
        let v = check_required(&rules, &[resolved("wayland-sys", &["client"])]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(
            v[0].contains("`wayland-sys`") && v[0].contains("`dlopen`") && v[0].contains("no pkg-config")
        );
    }
}
