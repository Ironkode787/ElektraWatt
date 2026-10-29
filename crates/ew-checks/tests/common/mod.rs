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

/// The findings of `rule` in an online run of the `full` scheme over `m`.
pub fn findings_of(m: &Model, rule: &str) -> Vec<ew_checks::Finding> {
    let q = ew_derive::Q::new(m);
    ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

/// One case of a rule table: a named model and the check of the rule's findings over it.
pub struct Case {
    pub name: &'static str,
    pub model: Model,
    pub check: Box<dyn Fn(&str, &[ew_checks::Finding])>,
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

/// The touched key of a device.
pub fn dev_key(n: u128) -> (Kind, AnyId) {
    (Kind::Device, uid(n))
}
