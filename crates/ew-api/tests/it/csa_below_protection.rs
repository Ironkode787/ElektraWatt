// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire's size against its protection (doc 08 §4.2, §8.1): a cross-section set by hand below what the rating of
//! the wire's protecting device needs by the current table is EW-POT-022, whose fix takes the proposed size; and a
//! wire behind a protective device that nothing feeds says on its Cross-section row why the rating is not used.

use crate::common;

use ew_api::checks::{self, CheckMode};
use ew_derive::ew_ids::Id;
use ew_derive::{ConnWireProps, PropSource, SizeRule};
use ew_schema::Connection;
use serde_json::json;

/// The wires of `s` whose size comes from a 16 A rating (the sample's C16 breakers).
fn behind_breakers(s: &ew_api::Session) -> Vec<Id<Connection>> {
    let q = s.query();
    let ids: Vec<Id<Connection>> = q.connections().ids().collect();
    ids.into_iter()
        .filter(|id| {
            q.get::<ConnWireProps>(*id).iter().any(|(wire, props)| {
                *wire == *id
                    && props.csa.as_ref().is_some_and(|c| {
                        matches!(
                            c.source,
                            PropSource::SizeRow {
                                rule: SizeRule::Rating { .. },
                                ..
                            }
                        )
                    })
            })
        })
        .collect()
}

/// The open EW-POT-022 findings of `s` on the connection `wire`.
fn findings_on(s: &ew_api::Session, wire: Id<Connection>) -> Vec<ew_api::checks::Finding> {
    s.check(&checks::DRAFTING, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.is_open() && f.rule == "EW-POT-022" && f.targets.iter().any(|t| t.id == wire.cast()))
        .collect()
}

fn set_csa(s: &mut ew_api::Session, wire: Id<Connection>, value: serde_json::Value) {
    common::run_cmd(
        s,
        "fields.set",
        json!({"kind": "connection", "targets": [wire.as_uuid().to_string()], "path": "overrides.csa", "value": value}),
    );
}

#[test]
fn a_size_set_by_hand_below_its_breakers_rating_is_pot022_and_its_fix_takes_the_proposal() {
    let mut s = common::sample();
    let wire = *behind_breakers(&s).first().expect("a wire behind a C16 breaker");
    assert!(findings_on(&s, wire).is_empty(), "a derived size is no finding");
    // 0.5 mm², below the table's first row, falls short of 16 A as 1.5 mm² does; 2.5 mm² carries it.
    for (size, short) in [("0.5", true), ("1.5", true), ("2.5", false), ("4", false)] {
        set_csa(&mut s, wire, json!(size));
        let found = findings_on(&s, wire);
        assert_eq!(found.len(), usize::from(short), "{size} mm²: {found:?}");
        if let Some(f) = found.first() {
            assert!(f.message().contains("2.5 mm²"), "the proposed size: {}", f.message());
            assert!(f.message().contains("16 A"), "the rating: {}", f.message());
        }
    }
    set_csa(&mut s, wire, json!("0.5"));
    let fix = findings_on(&s, wire)
        .into_iter()
        .next()
        .and_then(|f| f.fix)
        .expect("the finding's fix");
    // The wire's size without the setting is the proposal, so the fix takes the setting away.
    assert_eq!(fix.params["value"], serde_json::Value::Null);
    common::run_cmd(&mut s, &fix.command, fix.params.clone());
    assert!(findings_on(&s, wire).is_empty(), "after the fix");
}
