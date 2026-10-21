// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-NUM-001 (tag deviates from the scheme): frozen tags against the rule-derived product designation.

mod common;

use common::*;
use ew_checks::{CheckMode, FULL, Severity, run};
use ew_derive::Q;
use ew_derive::ew_schema::Kind;
use ew_model::Model;
use serde_json::json;

const DEV: u128 = 0x301;

/// A device `S` drawn on page 1, with a frozen tag `frozen` (text) when given.
fn drawn(frozen: Option<&str>) -> Model {
    let mut fx = Fx::base();
    fx.drawn_device(DEV, "S", None, P1);
    if let Some(text) = frozen {
        let d = fx
            .records
            .iter_mut()
            .find(|r| r["id"] == json!(us(DEV)))
            .expect("the device");
        d["tag"]["frozen"] = json!({"text": text, "sign": "hidden"});
    }
    fx.build()
}

/// The rule-derived product designation of the device.
fn rule_value() -> String {
    let m = drawn(None);
    let q = Q::new(&m);
    let id = ew_derive::ew_ids::Id::from_uuid(ew_derive::ew_ids::Uuid::from_u128(DEV));
    ew_derive::rule_product(&q, id).expect("the device exists").0
}

#[test]
fn num001_reports_a_frozen_tag_that_deviates() {
    let rule = rule_value();
    assert_ne!(rule, "S51", "the fixture must not derive the frozen text");
    let m = drawn(Some("S51"));
    let f = findings_of(&m, "EW-NUM-001");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "017101");
    assert_eq!(f[0].severity, Severity::Info);
    assert_eq!(f[0].discriminator, "frozen");
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(
        (f[0].targets[0].kind, f[0].targets[0].id),
        (Kind::Device, uid(DEV))
    );
    assert!(
        f[0].message.contains("frozen, deviates from scheme"),
        "{}",
        f[0].message
    );
    assert!(f[0].message.starts_with("S51"), "{}", f[0].message);
    assert!(f[0].message.contains(&rule), "{}", f[0].message);
}

#[test]
fn num001_stays_info_in_release_mode() {
    let m = drawn(Some("S51"));
    let q = Q::new(&m);
    let f: Vec<_> = run(&q, &FULL, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-NUM-001")
        .collect();
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].severity, Severity::Info);
}

#[test]
fn num001_a_frozen_tag_equal_to_the_rule_value_has_no_finding() {
    let rule = rule_value();
    let m = drawn(Some(&rule));
    assert!(findings_of(&m, "EW-NUM-001").is_empty());
}

#[test]
fn num001_a_device_without_frozen_tag_has_no_finding() {
    assert!(findings_of(&drawn(None), "EW-NUM-001").is_empty());
}
