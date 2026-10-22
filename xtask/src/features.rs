// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `check-features`: the feature bans of 18 §5.2 CI-PR-09, driven by `[[banned_features]]` and
//! `banned_crates` in `ci/dep-rules.toml`.
//!
//! Test-only and unsafe features (`ew-ids/testkit`, `ew/test-confirm`, egui `inspection`, rhai `unchecked`)
//! and the `egui_mcp` crate must never be enabled on a non-test dependency edge. The check reads the
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
    fn banned_crate_fails_unless_dev() {
        let v = run(&[package("a", vec![dep("egui_mcp", Kind::Normal, &[])], &[])]);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].contains("`a` has `egui_mcp` as a dependency"), "{v:#?}");
        let v = run(&[package("a", vec![dep("egui_mcp", Kind::Build, &[])], &[])]);
        assert_eq!(v.len(), 1, "{v:#?}");
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
    fn raw_read_in_an_allowed_package_passes() {
        let packages = ["ew-derive", "ew-commands", "ew-store"]
            .map(|n| package(n, vec![dep("ew-model", Kind::Normal, &["raw-read"])], &[]));
        assert_eq!(run(&packages), Vec::<String>::new());
    }

    #[test]
    fn raw_read_on_a_dev_edge_passes() {
        let v = run(&[package(
            "ew-scene",
            vec![dep("ew-model", Kind::Dev, &["raw-read"])],
            &[],
        )]);
        assert_eq!(v, Vec::<String>::new());
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
    fn raw_read_forwarded_by_an_allowed_package_passes() {
        let p = package(
            "ew-derive",
            vec![dep("ew-model", Kind::Normal, &[])],
            &[
                ("default", &["ew-model/raw-read"]),
                ("debug", &["ew-model/raw-read"]),
            ],
        );
        assert_eq!(run(&[p]), Vec::<String>::new());
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
    fn allowed_package_exposing_a_forwarding_feature_passes() {
        let p = package(
            "ew-derive",
            vec![dep("ew-model", Kind::Normal, &[])],
            &[("raw", &["ew-model/raw-read"]), ("default", &["raw"])],
        );
        assert_eq!(run(&[p]), Vec::<String>::new());
    }

    #[test]
    fn dev_edge_to_a_forwarding_feature_passes() {
        let derive = package(
            "ew-derive",
            vec![dep("ew-model", Kind::Normal, &[])],
            &[("raw", &["ew-model/raw-read"])],
        );
        let reports = package("ew-reports", vec![dep("ew-derive", Kind::Dev, &["raw"])], &[]);
        assert_eq!(run(&[derive, reports]), Vec::<String>::new());
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

    #[test]
    fn forwarding_cycle_terminates() {
        let p = package("cyc", vec![], &[("a", &["b"]), ("b", &["a"])]);
        assert_eq!(run(&[p]), Vec::<String>::new());
    }

    /// The real workspace satisfies the real feature bans, and a seeded `ew-ids/testkit` normal edge fails.
    #[test]
    fn real_workspace_passes_and_seeded_violation_fails() {
        let root = crate::workspace_root();
        let policy = crate::deps::Policy::load(&root.join("ci/dep-rules.toml")).expect("policy loads");
        assert!(
            !policy.banned_features.is_empty() && !policy.banned_crates.is_empty(),
            "the policy lists the banned features and crates"
        );
        let rules = Rules {
            features: &policy.banned_features,
            crates: &policy.banned_crates,
        };
        let mut packages = from_metadata(&crate::metadata().expect("cargo metadata runs"));
        assert_eq!(check(&rules, &packages), Vec::<String>::new());
        packages.push(package(
            "seeded",
            vec![dep("ew-ids", Kind::Normal, &["testkit"])],
            &[],
        ));
        assert_eq!(check(&rules, &packages).len(), 1);
    }

    /// The generator features of `uuid` and `rand` (FT-317) are banned outside dev edges by the real policy.
    #[test]
    fn the_generator_features_are_banned_outside_dev_edges() {
        let root = crate::workspace_root();
        let policy = crate::deps::Policy::load(&root.join("ci/dep-rules.toml")).expect("policy loads");
        let rules = Rules {
            features: &policy.banned_features,
            crates: &policy.banned_crates,
        };
        for (krate, feature) in [
            ("uuid", "v4"),
            ("uuid", "v7"),
            ("uuid", "fast-rng"),
            ("rand", "thread_rng"),
        ] {
            let normal = check(
                &rules,
                &[package("p", vec![dep(krate, Kind::Normal, &[feature])], &[])],
            );
            assert_eq!(normal.len(), 1, "{krate}/{feature}: {normal:#?}");
            assert!(normal[0].contains(&format!("`{krate}/{feature}`")), "{normal:#?}");
            let build = check(
                &rules,
                &[package("p", vec![dep(krate, Kind::Build, &[feature])], &[])],
            );
            assert_eq!(build.len(), 1, "{krate}/{feature}: {build:#?}");
            let dev = check(
                &rules,
                &[package("p", vec![dep(krate, Kind::Dev, &[feature])], &[])],
            );
            assert!(dev.is_empty(), "{krate}/{feature}: {dev:#?}");
        }
    }
}
