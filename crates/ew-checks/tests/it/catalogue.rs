// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The rule catalogue that the registry and the shipped schemes generate: the data of the user guide's rule
//! reference (`cargo xtask docs-gen`, `docs/book/src/reference/rules/`). `xtask` is a permissive-layer tool and may
//! not link this GPL crate, so it reads the catalogue as JSON. The file is not committed: `cargo xtask gen` runs this
//! test with `EW_REFERENCE_DIR` set, and the test then writes `rules.json` there.

// Test target: `EW_REFERENCE_DIR` only chooses whether the generated file is also written out.
#![allow(clippy::disallowed_methods)]

use std::path::PathBuf;

use ew_checks::rules::core::INTEGRITY;
use ew_checks::{CheckMode, Modes, OPEN_ITEM_RULES, RuleKind, RuleMeta, SCHEMES, rules};
use serde_json::{Value, json};

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

/// The catalogue text: two-space indentation, keys sorted, LF and a final newline. It lists the registered rules and
/// the integrity rules of opening a project ([`INTEGRITY`]), which reach the Messages pane too, once each in rule-ID order, so
/// that docs-gen writes the page every rule's `docs` names.
fn generated() -> String {
    let mut metas: Vec<&RuleMeta> = rules().iter().map(|r| r.meta()).chain(INTEGRITY).collect();
    metas.sort_by_key(|m| m.id);
    // EW-CORE-008 is both: a registered rule and a row of `INTEGRITY`.
    metas.dedup_by_key(|m| m.id);
    let rules: Vec<Value> = metas
        .into_iter()
        .map(|m| {
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
                // `\"open_item\": null` or
                // `{\"category\": \"<words>\", \"discriminator\": \"<d>\" | null}` from `OPEN_ITEM_RULES`.
                "open_item": OPEN_ITEM_RULES.iter().find(|o| o.rule == m.id).map(|o| json!({
                    "category": o.category,
                    "discriminator": o.discriminator,
                })),
                // The rule's page reference and the en-GB hint, for the rule's page of the guide.
                "docs": m.docs,
                // An integrity finding carries the reader's own text and has no Fluent message: its hint is empty.
                "hint": ew_checks::messages::hint(m.id, ew_checks::messages::DEFAULT_LANGUAGE)
                    .or_else(|| (m.kind == RuleKind::Integrity).then(String::new)),
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
fn the_catalogue_is_written_where_the_reference_is_asked_for() {
    let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("create the reference directory");
    std::fs::write(dir.join("rules.json"), generated()).expect("write the catalogue");
}

/// OI-4: the catalogue's `open_item` marks exactly the open-item rules, each with its category,
/// and EW-BUS-001 with the discriminator `address`.
#[test]
fn the_catalogue_marks_exactly_the_open_item_rules() {
    let committed: Value = serde_json::from_str(&generated()).expect("the catalogue is JSON");
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
