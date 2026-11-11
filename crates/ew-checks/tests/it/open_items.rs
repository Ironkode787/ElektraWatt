// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The open-items list on small fixtures: the open items are the open findings of the
//! open-item rules that `run` reports in release mode of the project's release scheme (the orchestrator's ruling 2,
//! "`open_items` calls `run_rules` ... and keeps the findings that are `FindingState::Open` and
//! `open_item_of` some row"), and the release scheme is the project's when it ships, else `pre-delivery` (ruling 4).

use crate::common;

use common::*;
use ew_checks::{
    CHECKS_MEMBER, CheckMode, DRAFTING, Finding, FindingState, PRE_DELIVERY, Scheme, open_item_of,
    open_items, project_scheme_name, release_scheme_of, run,
};
use ew_derive::Q;
use ew_model::Model;
use serde_json::{Value, json};

/// Two drawn devices without a part (EW-CORE-041 each; their connection points are unconnected, EW-POT-009), under
/// the project settings `settings`, with the waiver records `waivers`.
fn two_devices(settings: Value, waivers: &[Value]) -> Model {
    let mut fx = Fx::base();
    fx.settings = settings;
    fx.drawn_device(0x301, "K", Some(1), P1);
    fx.drawn_device(0x302, "Q", Some(1), P1);
    for w in waivers {
        fx.push(w.clone());
    }
    fx.build()
}

/// The open findings of `run(scheme, Release)` that `open_item_of` places.
fn expected(q: &Q, scheme: &Scheme) -> Vec<Finding> {
    run(q, scheme, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.state == FindingState::Open && open_item_of(f).is_some())
        .collect()
}

fn printed(findings: &[Finding]) -> Vec<String> {
    findings
        .iter()
        .map(|f| format!("{} {}", f.rule, f.subject))
        .collect()
}

/// D7: the open items equal the open findings of the release run of `pre-delivery`, in its order; a waived finding
/// leaves the list.
#[test]
fn the_open_items_are_the_open_findings_of_the_release_run() {
    let m = two_devices(Value::Null, &[]);
    let q = Q::new(&m);
    let items = open_items(&q).expect("no cycle");
    assert_eq!(items, expected(&q, &PRE_DELIVERY));
    assert_eq!(
        printed(&items),
        [
            "EW-POT-009 -1K1:1",
            "EW-POT-009 -1K1:2",
            "EW-POT-009 -1Q1:1",
            "EW-POT-009 -1Q1:2",
            "EW-CORE-041 -1K1",
            "EW-CORE-041 -1Q1",
        ]
    );
    let k1 = items[4].clone();

    // -1K1's finding waived: it leaves the list, every other item stays.
    let waived = two_devices(
        Value::Null,
        &[json!({
            "$k": "waiver", "id": us(0x9001), "rule": "EW-CORE-041", "target": {"finding": k1.fingerprint.to_string()},
            "reason": {"en-GB": "accepted for the test"}, "author": "engineer", "date": "2025-04-14"
        })],
    );
    let q = Q::new(&waived);
    let after = open_items(&q).expect("no cycle");
    assert_eq!(after, expected(&q, &PRE_DELIVERY));
    let mut want = items.clone();
    want.retain(|f| f.fingerprint != k1.fingerprint);
    assert_eq!(printed(&after), printed(&want));
}

/// Ruling 4: the release scheme is the one the project names when this build ships it, else `pre-delivery`; the
/// online member is read apart; the list runs in the release scheme.
#[test]
fn the_release_scheme_is_the_projects_else_pre_delivery() {
    let none = two_devices(Value::Null, &[]);
    let q = Q::new(&none);
    assert_eq!(release_scheme_of(&q), &PRE_DELIVERY);
    assert_eq!(project_scheme_name(&q, CheckMode::Release), None);

    let drafting = two_devices(
        json!({ (CHECKS_MEMBER): {"release_scheme": "drafting", "active_scheme": "full"} }),
        &[],
    );
    let q = Q::new(&drafting);
    assert_eq!(
        project_scheme_name(&q, CheckMode::Release).as_deref(),
        Some("drafting")
    );
    assert_eq!(
        project_scheme_name(&q, CheckMode::Online).as_deref(),
        Some("full")
    );
    assert_eq!(release_scheme_of(&q), &DRAFTING);
    assert_eq!(open_items(&q).expect("no cycle"), expected(&q, &DRAFTING));

    let unknown = two_devices(json!({ (CHECKS_MEMBER): {"release_scheme": "company-x"} }), &[]);
    let q = Q::new(&unknown);
    assert_eq!(
        project_scheme_name(&q, CheckMode::Release).as_deref(),
        Some("company-x")
    );
    assert_eq!(release_scheme_of(&q), &PRE_DELIVERY);
}
