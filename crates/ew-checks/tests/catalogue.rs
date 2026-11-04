// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The committed rule catalogue `crates/ew-checks/catalogue/rules.json` equals what the registry and the shipped
//! schemes generate (NFR-DOC-06).
//!
//! The file is the data source of the generated rule reference of the user documentation
//! (`cargo xtask docs-gen`, `docs/book/src/reference/rules/`): `xtask` is a permissive-layer development tool and
//! may not link this GPL crate, so it reads this snapshot instead, as it reads the command schemas of
//! `ew-commands`. The test regenerates the text in memory and fails when the committed file differs;
//! `EW_CHECKS_BLESS=1 cargo test -p ew-checks --test catalogue` rewrites it.

// Test target: bless and diagnostic environment flags (EW_*), which only choose whether a committed file is rewritten or a line is printed; the compared result never depends on them.
#![allow(clippy::disallowed_methods)]

use std::path::PathBuf;

use ew_checks::{CheckMode, Modes, OPEN_ITEM_RULES, SCHEMES, rules};
use serde_json::{Value, json};

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("catalogue/rules.json")
}

/// The mode letters of the catalogue (doc 15 §3.1) in a fixed order, as words.
fn modes(m: Modes) -> Vec<&'static str> {
    [
        (m.online, "online"),
        (m.offline, "offline"),
        (m.prevent, "prevent"),
        (m.release, "release"),
        (m.export, "export"),
        (m.gate, "gate"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect()
}

/// The catalogue text: two-space indentation, keys sorted, LF and a final newline.
fn generated() -> String {
    let rules: Vec<Value> = rules()
        .iter()
        .map(|r| {
            let m = r.meta();
            let schemes: Vec<Value> = SCHEMES
                .iter()
                .map(|s| {
                    json!({
                        "name": s.name,
                        "online": s.enabled(m, CheckMode::Online),
                        "release": s.enabled(m, CheckMode::Release),
                    })
                })
                .collect();
            json!({
                "id": m.id,
                "code": m.code,
                "pack": m.pack,
                "title": m.title,
                "kind": m.kind.name(),
                "modes": modes(m.modes),
                "severity": m.severity.name(),
                "release_severity": m.release_severity.map(|s| s.name()),
                "scope": m.scope.name(),
                "supersedes": m.supersedes,
                "release": m.release.name(),
                "waivable": m.waivable(),
                "schemes": schemes,
                // WP-S3.06 D8 and the orchestrator's ruling 6 (cycle 42): "`\"open_item\": null` or
                // `{\"category\": \"<words>\", \"discriminator\": \"<d>\" | null}` from `OPEN_ITEM_RULES`".
                "open_item": OPEN_ITEM_RULES.iter().find(|o| o.rule == m.id).map(|o| json!({
                    "category": o.category,
                    "discriminator": o.discriminator,
                })),
                // WP-S7.05 D4, D6: the rule's page reference and the en-GB hint, for the rule's page of the guide.
                "docs": m.docs,
                "hint": ew_checks::messages::hint(m.id, ew_checks::messages::DEFAULT_LANGUAGE),
            })
        })
        .collect();
    let schemes: Vec<Value> = SCHEMES
        .iter()
        .map(|s| json!({ "name": s.name, "purpose": s.purpose, "lenient": s.lenient }))
        .collect();
    let mut text = serde_json::to_string_pretty(&json!({ "rules": rules, "schemes": schemes }))
        .expect("the catalogue serialises");
    text.push('\n');
    text
}

#[test]
fn committed_catalogue_matches_the_registry() {
    let generated = generated();
    if std::env::var_os("EW_CHECKS_BLESS").is_some() {
        let path = path();
        std::fs::create_dir_all(path.parent().expect("the catalogue has a directory"))
            .expect("create the catalogue directory");
        std::fs::write(&path, generated).expect("write the catalogue");
        return;
    }
    let committed = std::fs::read_to_string(path()).unwrap_or_default();
    assert!(
        committed == generated,
        "catalogue/rules.json is out of date; run `EW_CHECKS_BLESS=1 cargo test -p ew-checks --test catalogue` \
         and commit the result (then `cargo xtask docs-gen`)"
    );
}

/// OI-4 (WP-S3.06 D8): the committed snapshot's `open_item` marks exactly the open-item rules, each with its category,
/// and EW-BUS-001 with the discriminator `address`.
#[test]
fn the_catalogue_marks_exactly_the_open_item_rules() {
    let committed: Value = serde_json::from_str(&std::fs::read_to_string(path()).expect("the catalogue"))
        .expect("the catalogue is JSON");
    let marked: Vec<(String, Value)> = committed["rules"]
        .as_array()
        .expect("rules")
        .iter()
        .filter(|r| !r["open_item"].is_null())
        .map(|r| (r["id"].as_str().expect("id").to_owned(), r["open_item"].clone()))
        .collect();
    let mut want: Vec<(String, Value)> = OPEN_ITEM_RULES
        .iter()
        .map(|o| {
            (
                o.rule.to_owned(),
                json!({"category": o.category, "discriminator": o.discriminator}),
            )
        })
        .collect();
    want.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(marked, want);
    let bus = marked
        .iter()
        .find(|(id, _)| id == "EW-BUS-001")
        .expect("EW-BUS-001 is an open-item rule");
    assert_eq!(
        bus.1,
        json!({"category": "TBD value", "discriminator": "address"})
    );
}
