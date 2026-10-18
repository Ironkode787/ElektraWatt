// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The field-mask exhaustiveness test (ARCH §4.1, ADR-0001 "Verification"): every ID-valued member of an
//! implemented kind's JSON Schema (`schemas/0.1/<kind>.schema.json`) must be declared, either as a link in
//! [`LINKS`], as a non-link in [`NOT_LINKS`], or as the kind's own `id`; and every declaration must exist
//! in the schema. Adding an ID field to a record without declaring it fails CI.
//!
//! Schema paths use the syntax of [`crate::LinkDecl::field`]: object properties joined with `.`, array
//! items append `[]`, tuple items `[i]`; `anyOf`/`oneOf`/`allOf` branches are transparent.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ew_schema::{Kind, KindStatus};
use serde_json::Value;

use crate::{IndexName, LINKS, NOT_LINKS, Policy};

const UNDECLARED_HINT: &str = "declare it in LINKS (with a policy and an index decision) or in NOT_LINKS with a reason (crates/ew-model/src/links.rs)";

fn implemented() -> impl Iterator<Item = Kind> {
    Kind::ALL
        .into_iter()
        .filter(|k| k.status() == KindStatus::Implemented)
}

fn load_schema(kind: Kind) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/0.1")
        .join(format!("{}.schema.json", kind.name()));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("schema {} must exist: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("schema {} must be JSON: {e}", path.display()))
}

fn walk(defs: &Value, node: &Value, path: &str, out: &mut BTreeSet<String>, stack: &mut Vec<String>) {
    let Some(obj) = node.as_object() else { return };
    if let Some(r) = obj.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap_or(r);
        if name == "Uuid" {
            out.insert(path.to_owned());
            return;
        }
        if !stack.iter().any(|s| s == name) {
            stack.push(name.to_owned());
            walk(defs, &defs[name], path, out, stack);
            stack.pop();
        }
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        for branch in obj.get(key).and_then(Value::as_array).into_iter().flatten() {
            walk(defs, branch, path, out, stack);
        }
    }
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        for (name, sub) in props {
            let p = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            walk(defs, sub, &p, out, stack);
        }
    }
    if let Some(items) = obj.get("items").filter(|v| v.is_object()) {
        walk(defs, items, &format!("{path}[]"), out, stack);
    }
    for (i, sub) in obj
        .get("prefixItems")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        walk(defs, sub, &format!("{path}[{i}]"), out, stack);
    }
    if let Some(extra) = obj.get("additionalProperties").filter(|v| v.is_object()) {
        walk(defs, extra, &format!("{path}{{}}"), out, stack);
    }
}

/// Every path at which an implemented kind's schema holds a bare `Uuid`, prefixed with the kind name.
fn schema_paths() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for k in implemented() {
        let schema = load_schema(k);
        let mut paths = BTreeSet::new();
        walk(&schema["$defs"], &schema, "", &mut paths, &mut Vec::new());
        out.extend(paths.into_iter().map(|p| format!("{}.{p}", k.name())));
    }
    out
}

/// `LINKS ∪ NOT_LINKS ∪ {every implemented kind's id}` as paths.
fn expected_paths() -> BTreeSet<String> {
    let mut out: BTreeSet<String> = LINKS.iter().map(|l| l.path()).collect();
    out.extend(NOT_LINKS.iter().map(|(kind, field, _)| format!("{kind}.{field}")));
    out.extend(implemented().map(|k| format!("{}.id", k.name())));
    out
}

#[test]
fn every_id_field_is_declared() {
    let expected = expected_paths();
    let undeclared: Vec<String> = schema_paths().difference(&expected).cloned().collect();
    assert!(
        undeclared.is_empty(),
        "undeclared ID fields: {undeclared:#?}\n{UNDECLARED_HINT}"
    );
}

#[test]
fn every_declaration_exists_in_the_schema() {
    let schema = schema_paths();
    let stale: Vec<String> = expected_paths().difference(&schema).cloned().collect();
    assert!(
        stale.is_empty(),
        "declarations without a schema field (stale): {stale:#?}"
    );
}

#[test]
fn link_paths_are_unique() {
    let mut seen = BTreeSet::new();
    let mut dup = Vec::new();
    let paths = LINKS
        .iter()
        .map(|l| l.path())
        .chain(NOT_LINKS.iter().map(|(kind, field, _)| format!("{kind}.{field}")));
    for p in paths {
        if !seen.insert(p.clone()) {
            dup.push(p);
        }
    }
    assert!(dup.is_empty(), "paths declared more than once: {dup:#?}");
}

#[test]
fn links_are_well_formed() {
    let implemented: BTreeSet<Kind> = implemented().collect();
    for l in LINKS {
        assert!(
            implemented.contains(&l.from),
            "{}: `from` kind is not implemented",
            l.path()
        );
        assert!(
            implemented.contains(&l.to),
            "{}: `to` kind is not implemented",
            l.path()
        );
        assert_eq!(
            l.unlink.is_some(),
            l.policy == Policy::Unlink,
            "{}: an unlink function exists iff the policy is Unlink",
            l.path()
        );
    }
    for (kind, field, reason) in NOT_LINKS {
        assert!(
            !reason.trim().is_empty(),
            "{kind}.{field}: NOT_LINKS reason is empty"
        );
    }
}

#[test]
fn named_indexes_have_consistent_kinds() {
    let mut by_index: BTreeMap<IndexName, Vec<&crate::LinkDecl>> = BTreeMap::new();
    for l in LINKS {
        if let Some(ix) = l.index {
            by_index.entry(ix).or_default().push(l);
        }
    }
    let endpoint_keyed = [
        IndexName::ConnectionsByEndpoint,
        IndexName::PotentialDefsByEndpoint,
        IndexName::BondsByEndpoint,
    ];
    for (ix, links) in &by_index {
        let from = links[0].from;
        assert!(
            links.iter().all(|l| l.from == from),
            "{ix:?}: links come from different kinds"
        );
        if endpoint_keyed.contains(ix) {
            for l in links {
                assert!(
                    matches!(l.to, Kind::Function | Kind::Junction),
                    "{ix:?}: {} targets {:?}, expected Function or Junction",
                    l.path(),
                    l.to
                );
            }
        } else {
            let to = links[0].to;
            assert!(
                links.iter().all(|l| l.to == to),
                "{ix:?}: links target different kinds"
            );
        }
    }
}
