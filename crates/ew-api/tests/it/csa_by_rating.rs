// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire's cross-section is proposed from the rating of the device that protects it (doc 08 §4.2, FT-2055): behind
//! the sample's C16 breakers, whose mains class's 1.5 mm² carries less than 16 A by the current table, the wires take
//! the table's 2.5 mm², naming the breaker; a cross-section the person sets wins.

use crate::common;

use ew_derive::ew_ids::Id;
use ew_derive::{ConnWireProps, PropSource, SizeRule};
use ew_schema::{Connection, Csa};
use ew_units::Dec3;
use serde_json::json;

/// The wires of `s` whose cross-section comes from a protecting device's rating, with that device's rating.
fn rated(s: &ew_api::Session) -> Vec<(Id<Connection>, Csa, Dec3)> {
    let q = s.query();
    let ids: Vec<Id<Connection>> = q.connections().ids().collect();
    let mut out = Vec::new();
    for id in ids {
        for (wire, props) in q.get::<ConnWireProps>(id).iter() {
            if let Some(csa) = &props.csa
                && let PropSource::SizeRow {
                    rule: SizeRule::Rating { current, .. },
                    ..
                } = csa.source
                && *wire == id
            {
                out.push((id, csa.value.clone(), current));
            }
        }
    }
    out
}

#[test]
fn a_wire_behind_a_breaker_takes_the_size_that_carries_its_rating() {
    let mut s = common::sample();
    let wires = rated(&s);
    assert!(!wires.is_empty(), "the wires behind the sample's breakers");
    for (_, csa, current) in &wires {
        assert_eq!(*current, Dec3(16_000), "the sample's breakers are C16");
        assert_eq!(
            *csa,
            Csa::Mm2(Dec3(2_500)),
            "the least size the table gives for 16 A"
        );
    }
    // The person's own cross-section wins over the proposal.
    let (wire, _, _) = wires[0];
    common::run_cmd(
        &mut s,
        "fields.set",
        json!({"kind": "connection", "targets": [wire.as_uuid().to_string()], "path": "overrides.csa", "value": "1.5"}),
    );
    let q = s.query();
    let props = q.get::<ConnWireProps>(wire);
    let (_, own) = props
        .iter()
        .find(|(w, _)| *w == wire)
        .expect("the wire's properties");
    let csa = own.csa.as_ref().expect("a cross-section");
    assert_eq!(csa.value, Csa::Mm2(Dec3(1_500)));
    assert!(matches!(csa.source, PropSource::Override(_)), "{:?}", csa.source);
}

/// A size below the table's first row (0.75 mm²), whose current the table does not give, is not raised: the sample
/// without its size profile, with a project default of 0.5 mm² and its breakers rated 2 A, keeps 0.5 mm² on the wires
/// behind them.
#[test]
fn a_size_below_the_table_is_not_raised() {
    let behind: Vec<Id<Connection>> = rated(&common::sample()).into_iter().map(|(w, _, _)| w).collect();
    let (mut project, mut records, lib) = common::sample_records();
    project.settings.extra.remove("size_profile");
    project
        .settings
        .extra
        .insert("wire_default".into(), json!({"csa": "0.5"}));
    for r in &mut records {
        if let ew_schema::Record::Device(d) = r {
            d.tech_data =
                serde_json::from_value(json!([{"key": "rated_current", "value": {"value": "2", "unit": "A"}}]))
                    .expect("technical data");
        }
    }
    let s = common::session_of(project, records, lib);
    assert_eq!(rated(&s), Vec::new(), "no wire takes its size from a rating");
    let q = s.query();
    for wire in behind {
        let props = q.get::<ConnWireProps>(wire);
        let (_, own) = props
            .iter()
            .find(|(w, _)| *w == wire)
            .expect("the wire's properties");
        let csa = own.csa.as_ref().expect("a cross-section");
        assert_eq!(csa.value, Csa::Mm2(Dec3(500)), "{wire:?}");
        assert_eq!(csa.source, PropSource::ProjectDefault, "{wire:?}");
    }
}
