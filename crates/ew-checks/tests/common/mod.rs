// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Small model snapshots for the rule tests: records from JSON, a model, a `Q` over it.

#![allow(dead_code)]

mod fx;

#[allow(unused_imports)]
pub use fx::{Ep, FNDEF, FNDEF_IP, FNDEF_IP0, FNDEF_PASS, FNDEF_PE, Fx, P1, P2, cp, model_json};

use ew_derive::ew_ids::{Id, Uuid};
use ew_derive::ew_schema::{AnyId, Kind, Project, Record};
use ew_model::Model;
use serde_json::{Value, json};

/// The ID with this number.
pub fn uid(n: u128) -> AnyId {
    Id::from_uuid(Uuid::from_u128(n))
}

/// The UUID text of [`uid`].
pub fn us(n: u128) -> String {
    Uuid::from_u128(n).to_string()
}

/// A project record, with optional settings.
pub fn project(settings: Value) -> Project {
    let mut p = json!({"id": us(0xF000), "derivation_profile": "2027.1"});
    if !settings.is_null() {
        p["settings"] = settings;
    }
    serde_json::from_value(p).expect("a project record")
}

/// A record from its JSON form (with `$k`).
pub fn record(v: Value) -> Record {
    serde_json::from_value(v).expect("a record")
}

/// A strip device `n` with tag letter X and counter `counter`.
pub fn strip(n: u128, counter: u32) -> Record {
    record(json!({"$k": "device", "id": us(n), "tag": {"letter": "X", "counter": counter}, "strip": {}}))
}

/// A terminal device `n` on strip `strip` with sequence `seq` and designation `designation`.
pub fn terminal(n: u128, strip: u128, seq: u32, designation: &str) -> Record {
    record(json!({
        "$k": "device", "id": us(n), "tag": {"letter": "X"}, "kind": "terminal",
        "terminal": {"strip": us(strip), "seq": seq, "designation": designation}
    }))
}

/// A model of `records` under a default project.
pub fn model(records: Vec<Record>) -> Model {
    model_with(Value::Null, records)
}

/// A model of `records` under a project with `settings`.
pub fn model_with(settings: Value, records: Vec<Record>) -> Model {
    Model::from_records(project(settings), records, []).expect("the model passes the integrity check")
}

/// The findings of `rule` in an online run of the `full` scheme over `m`. Every finding with message arguments is
/// checked to render its message byte for byte from the rule's en-GB pattern (P5-2), so every rule table
/// exercises the patterns of its rule.
pub fn findings_of(m: &Model, rule: &str) -> Vec<ew_checks::Finding> {
    let q = ew_derive::Q::new(m);
    let found: Vec<ew_checks::Finding> = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect();
    assert_messages(&found);
    found
}

/// Asserts that every finding carries its message arguments and renders its message byte for byte from the en-GB
/// pattern, and that every other shipped language renders it from its own pattern (FT-680). An EW-IMP finding is the
/// exception: its message is the text the importer stored in the project.
pub fn assert_messages(findings: &[ew_checks::Finding]) {
    use ew_checks::messages::{DEFAULT_LANGUAGE, LANGUAGES, format};
    for f in findings
        .iter()
        .filter(|f| !ew_checks::rules::imp::IMP.iter().any(|m| m.id == f.rule))
    {
        assert_eq!(
            format(f, DEFAULT_LANGUAGE).as_deref(),
            Some(f.message.as_str()),
            "the en-GB pattern of {} does not render its message from {:?}",
            f.rule,
            f.args
        );
        for lang in LANGUAGES {
            assert!(
                format(f, lang).is_some(),
                "{lang} does not word {}'s finding {:?} from {:?}",
                f.rule,
                f.message,
                f.args
            );
        }
    }
}

/// The check of a case: it receives the case name and the rule's findings.
pub type CaseCheck = Box<dyn Fn(&str, &[ew_checks::Finding])>;

/// One case of a rule table: a named model and the check of the rule's findings over it.
pub struct Case {
    pub name: &'static str,
    pub model: Model,
    pub check: CaseCheck,
}

impl Case {
    /// A case on which the rule reports nothing.
    pub fn silent(name: &'static str, model: Model) -> Case {
        Case {
            name,
            model,
            check: Box::new(|c, f| assert!(f.is_empty(), "case {c}: expected no finding, got {f:#?}")),
        }
    }

    /// A case whose findings `check` judges; `check` receives the case name for its failure messages.
    pub fn reports(
        name: &'static str,
        model: Model,
        check: impl Fn(&str, &[ew_checks::Finding]) + 'static,
    ) -> Case {
        Case {
            name,
            model,
            check: Box::new(check),
        }
    }
}

/// Runs `rule` (online, `full` scheme) over the model of every case and applies the case's check.
pub fn rule_table(rule: &str, cases: Vec<Case>) {
    for c in &cases {
        let f = findings_of(&c.model, rule);
        (c.check)(c.name, &f);
    }
}

/// The findings of `rule` in a release run of the `full` scheme.
pub fn release_findings_of(m: &Model, rule: &str) -> Vec<ew_checks::Finding> {
    ew_checks::run(
        &ew_derive::Q::new(m),
        &ew_checks::FULL,
        ew_checks::CheckMode::Release,
    )
    .findings
    .into_iter()
    .filter(|f| f.rule == rule)
    .collect()
}

/// The findings of `rule` rendered in nl-NL, sorted.
pub fn dutch(m: &Model, rule: &str) -> Vec<String> {
    let mut out: Vec<String> = findings_of(m, rule)
        .iter()
        .map(|f| ew_checks::messages::render(f, "nl-NL"))
        .collect();
    out.sort();
    out
}

/// `(subject, message)` of each finding, in report order.
pub fn texts(f: &[ew_checks::Finding]) -> Vec<(String, String)> {
    f.iter().map(|f| (f.subject.clone(), f.message.clone())).collect()
}

/// The `(kind, id)` of each target of a finding.
pub fn targets(f: &ew_checks::Finding) -> Vec<(Kind, AnyId)> {
    f.targets.iter().map(|t| (t.kind, t.id)).collect()
}

/// A target of kind `kind` with ID number `n`.
pub fn target(kind: Kind, n: u128) -> ew_checks::Target {
    ew_checks::Target::new(kind, uid(n))
}

/// An empty JSON object.
pub fn none() -> Value {
    json!({})
}

/// The fixture with the record and library IDs of each pair swapped everywhere they occur.
pub fn permuted(fx: &Fx, pairs: &[(u128, u128)]) -> Model {
    let swap = |v: &Value| -> Value {
        let mut text = v.to_string();
        for (i, (a, b)) in pairs.iter().enumerate() {
            let tmp = format!("@swap{i}@");
            text = text
                .replace(&us(*a), &tmp)
                .replace(&us(*b), &us(*a))
                .replace(&tmp, &us(*b));
        }
        serde_json::from_str(&text).expect("JSON")
    };
    let mut out = fx.clone();
    out.records = fx.records.iter().map(swap).collect();
    out.lib = fx.lib.iter().map(swap).collect();
    out.build()
}

/// Every finding of a release and an online run of the `full` scheme as printed: rule, severity, subject, message,
/// in report order.
pub fn printed(m: &Model) -> Vec<String> {
    let q = ew_derive::Q::new(m);
    [ew_checks::CheckMode::Online, ew_checks::CheckMode::Release]
        .into_iter()
        .flat_map(|mode| ew_checks::run(&q, &ew_checks::FULL, mode).findings)
        .map(|f| format!("{} {} {}: {}", f.rule, f.severity, f.subject, f.message))
        .collect()
}

/// The touched key of a device.
pub fn dev_key(n: u128) -> (Kind, AnyId) {
    (Kind::Device, uid(n))
}
