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
//! - a banned feature is requested on an edge whose kind is not in the rule's `allowed_kinds` and whose
//!   package is not in its `allowed_packages`;
//! - a package outside a rule's non-empty `allowed_packages` forwards the banned feature from any of its own
//!   features (`dep/feature` or `dep?/feature`): an opt-in feature would otherwise be a back door;
//! - a banned crate is a dependency of any kind other than `dev`;
//! - a workspace package's own `default` feature reaches a banned feature (its own, or one forwarded to a
//!   dependency as `dep/feature` or `dep?/feature`); a package in the rule's `allowed_packages` is exempt for
//!   the forwarded case.
//!
//! Building the list of packages from `cargo metadata` is separate from checking it, so the unit tests can
//! feed synthetic graphs with seeded violations.

use std::collections::{BTreeMap, BTreeSet};

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
            for ban in rules.features.iter().filter(|b| b.krate == d.name) {
                if d.features.contains(&ban.feature)
                    && !ban.allowed_kinds.contains(&d.kind)
                    && !ban.allowed_packages.contains(&p.name)
                {
                    v.push(format!(
                        "feature ban: `{}` enables `{}/{}` on a {}; allowed on: {} (18 §5.2 CI-PR-09)",
                        p.name,
                        d.name,
                        ban.feature,
                        d.kind.label(),
                        allowed_label(ban)
                    ));
                }
            }
        }
        check_forwarding(rules, p, &mut v);
        check_default_features(rules, p, &mut v);
    }
    v.sort();
    v.dedup();
    v
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

/// A package outside the `allowed_packages` of a package-restricted ban must not forward the banned feature
/// from any of its features.
fn check_forwarding(rules: &Rules<'_>, p: &Package, v: &mut Vec<String>) {
    for ban in rules
        .features
        .iter()
        .filter(|b| !b.allowed_packages.is_empty() && !b.allowed_packages.contains(&p.name))
    {
        for (f, items) in &p.features {
            for item in items {
                if forwarded(p, item) == Some((ban.krate.as_str(), ban.feature.as_str())) {
                    v.push(format!(
                        "feature ban: `{}` forwards `{}/{}` from its feature `{f}`; only {} may enable it",
                        p.name,
                        ban.krate,
                        ban.feature,
                        ban.allowed_packages.join(", ")
                    ));
                }
            }
        }
    }
}

/// The package's `default` feature, followed through its own features, must not reach a banned feature.
fn check_default_features(rules: &Rules<'_>, p: &Package, v: &mut Vec<String>) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut todo = vec!["default"];
    while let Some(feature) = todo.pop() {
        if !seen.insert(feature) {
            continue;
        }
        let banned_own = rules
            .features
            .iter()
            .any(|b| b.krate == p.name && b.feature == feature);
        if banned_own {
            v.push(format!(
                "feature ban: the `default` features of `{}` enable its banned feature `{feature}`",
                p.name
            ));
        }
        for item in p.features.get(feature).into_iter().flatten() {
            match forwarded(p, item) {
                Some((krate, forwarded)) => {
                    if rules.features.iter().any(|b| {
                        b.krate == krate && b.feature == forwarded && !b.allowed_packages.contains(&p.name)
                    }) {
                        v.push(format!(
                            "feature ban: the `default` features of `{}` enable `{krate}/{forwarded}` (via `{feature}`)",
                            p.name
                        ));
                    }
                }
                None => todo.push(item.strip_prefix("dep:").unwrap_or(item)),
            }
        }
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
}
