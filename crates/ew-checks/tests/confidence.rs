// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 3, EM-9 (FR-PRT-031, FT-323's marker): a rule that reads a library section whose confidence is not
//! `verified` marks its finding (`Finding.unverified_data`). The rule here is a fixture: the PLC and bus rules set the
//! marker when WP-S1.21 builds them.

mod common;

use common::{model_json, us};
use ew_checks::{Finding, Modes, Release, Rule, RuleKind, RuleMeta, Scope, ScopeKind, Severity, Target};
use ew_derive::Q;
use ew_derive::ew_schema::Kind;
use serde_json::{Value, json};

const FIXTURE: RuleMeta = RuleMeta {
    id: "EW-TEST-001",
    code: "999001",
    pack: "EW-TEST",
    title: "A part's PLC section is read",
    kind: RuleKind::Completeness,
    modes: Modes::letters("o"),
    severity: Severity::Info,
    release_severity: None,
    scope: ScopeKind::Project,
    supersedes: &[],
    release: Release::Mvp,
};

/// Reports every part copy and reads its `plc` section's confidence.
struct ReadsPlcSection;

impl Rule for ReadsPlcSection {
    fn meta(&self) -> &'static RuleMeta {
        &FIXTURE
    }

    fn run(&self, q: &Q, _scope: &Scope) -> Vec<Finding> {
        q.parts()
            .iter()
            .map(|(id, copy)| {
                let confidence = ew_derive::part_confidence(copy, "plc");
                Finding::new(
                    &FIXTURE,
                    [Target::new(Kind::Part, id)],
                    "plc",
                    copy.item.part_number.as_str(),
                    "the PLC section was read",
                )
                .reading(&confidence)
            })
            .collect()
    }
}

fn part(n: u128, number: &str, confidence: Value) -> Value {
    json!({
        "$k": "part", "id": us(n), "library_id": us(0x7000), "item_id": us(n + 0x100), "version": "1.0.0",
        "sha256": format!("sha256:{}", "0".repeat(64)),
        "meta": {"confidence": confidence},
        "manufacturer": {"library": us(0x7000), "item": us(0x7100), "version_req": "*"},
        "part_number": number,
    })
}

#[test]
fn a_rule_reading_a_test_section_marks_its_finding() {
    let lib = [
        part(0x10, "TEST-PLC", json!({"plc": "test"})),
        part(0x11, "VERIFIED", json!({"plc": "verified"})),
        part(0x12, "UNSTATED", json!({})),
        part(
            0x13,
            "HEAD",
            json!({"plc.channels": "verified", "plc": "unverified"}),
        ),
    ];
    let m = model_json(Value::Null, &[], &lib);
    let q = Q::new(&m);
    let marks: Vec<(String, bool)> = ReadsPlcSection
        .run(&q, &Scope::Project)
        .into_iter()
        .map(|f| (f.subject, f.unverified_data))
        .collect();
    assert_eq!(
        marks,
        [
            ("TEST-PLC".to_owned(), true),
            ("VERIFIED".to_owned(), false),
            ("UNSTATED".to_owned(), false),
            ("HEAD".to_owned(), true),
        ]
    );
}

#[test]
fn a_finding_is_unmarked_unless_a_rule_marks_it() {
    let f = Finding::new(&FIXTURE, [], "", "", "");
    assert!(!f.unverified_data);
    assert!(f.marked_unverified().unverified_data);
}
