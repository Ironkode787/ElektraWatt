// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An earth symbol has no tag (doc 03 §8.2): it takes no counter of its letter, so the cables of its page
//! keep theirs; it prints none; and no check asks for its letter or finds it twice.

use crate::common;

use ew_api::{AnyCommand, Session};
use ew_ids::Id;
use ew_schema::Device;
use serde_json::json;

/// Places the protective earth symbol on folio 103 at `x` as a new device of the cables' letter `W`.
fn place_earth(s: &mut Session, x: i64) -> Id<Device> {
    let key = "ew.iec.pe@qet10";
    if let Some(embed) = s.starter_embed(key) {
        s.execute(&AnyCommand {
            name: "library.embed".into(),
            params: embed,
        })
        .expect("the symbol embeds");
    }
    let symbol = s.starter_symbol_copy(key).expect("the earth symbol").to_string();
    let page = s.page_tables("103").expect("folio 103").page.to_string();
    let placed = s
        .execute(&AnyCommand {
            name: "device.place".into(),
            params: json!({"page": page, "symbol": symbol, "pos": [x, (40 * 7_937_500 + 1) / 3],
                "target": {"new_device": {"letter": "W"}}}),
        })
        .expect("the earth symbol is placed");
    let device = placed
        .created
        .iter()
        .find(|k| k.0 == ew_schema::Kind::Device)
        .expect("a new device")
        .1;
    Id::from_uuid(device.as_uuid())
}

#[test]
fn an_earth_symbol_takes_no_counter_and_prints_no_tag() {
    let mut s = common::sample();
    let a = place_earth(&mut s, 158_750_000);
    let b = place_earth(&mut s, 174_625_000);
    let q = s.query();
    for d in [a, b] {
        let dev = q.devices().get(d).expect("placed");
        assert_eq!(dev.tag.counter, None, "it holds no counter of the cables' letter");
        assert_eq!(ew_derive::short_tag(&q, d), "", "it prints no tag");
        let tag = q.get::<ew_derive::DeviceTag>(d);
        assert_eq!(
            tag.as_ref().as_ref().map(|t| t.source.clone()),
            Some(ew_derive::TagSource::EarthConnection)
        );
    }
    // Neither earth symbol is told its letter, nor found twice, nor called a designation that does not parse.
    let report = ew_checks::run(&q, &ew_checks::FULL, ew_checks::CheckMode::Online);
    let on_earth: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule.starts_with("EW-RDS-") || f.rule == "EW-NUM-001")
        .filter(|f| f.targets.iter().any(|t| t.id == a.cast() || t.id == b.cast()))
        .collect();
    assert!(on_earth.is_empty(), "{on_earth:#?}");
}
