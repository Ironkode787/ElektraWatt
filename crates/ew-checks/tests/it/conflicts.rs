// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE-011 "Conflict records present" over the model: one release-mode
//! error per conflict record, its target and file named in words, never by ID; nothing in online mode.

use crate::common;

use common::*;
use ew_checks::{CheckMode, FULL, Severity, run};
use ew_derive::Q;
use serde_json::json;

const PAGE: u128 = 0x100;
const DEVICE: u128 = 0x200;
const SPACE: u128 = 0x300;
const LOCATION: u128 = 0x500;

fn conflict_model() -> ew_model::Model {
    let file_conflict = |n: u128, path: String| {
        record(
            json!({"$k": "conflict", "id": us(n), "kind": "file", "path": path, "side": "theirs", "lines": []}),
        )
    };
    model(vec![
        record(json!({
            "$k": "page", "id": us(PAGE), "folio": {"number": 102}, "kind": "multi_line",
            "structure": {}, "title": {"en": "page"}, "grid": {"num": 4_000_000, "den": 1},
        })),
        record(json!({"$k": "device", "id": us(DEVICE), "tag": {"letter": "K", "counter": 1}})),
        record(json!({"$k": "structure_node", "id": us(LOCATION), "aspect": "location", "code": "J-E-2416"})),
        record(json!({"$k": "layout_space", "id": us(SPACE), "location": us(LOCATION), "name": "+J-E-2416"})),
        record(json!({
            "$k": "conflict", "id": us(DEVICE), "kind": "device", "base": {},
            "ours": "deleted", "theirs": {"function_text": "Relay", "tech_data": {"coil": "24 V"}}
        })),
        record(json!({
            "$k": "conflict", "id": us(PAGE), "kind": "page", "base": {"title": {"en": "page"}},
            "ours": {"title": {"en": "Supply"}}, "theirs": {"title": {"en": "Mains"}}
        })),
        file_conflict(0x401, format!("pages/{}.jsonl", us(PAGE))),
        file_conflict(0x402, format!("layout/{}.jsonl", us(SPACE))),
        file_conflict(0x403, "devices/0a.jsonl".to_owned()),
    ])
}

#[test]
fn ew_core_011_reports_each_conflict_in_words_as_a_release_error() {
    let m = conflict_model();
    let q = Q::new(&m);
    let mut found: Vec<(String, String, Severity)> = run(&q, &FULL, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-011")
        .map(|f| (f.subject, f.message, f.severity))
        .collect();
    let e = |subject: &str, message: &str| (subject.to_owned(), message.to_owned(), Severity::Error);
    let mut expected = vec![
        e(
            "device -?K1",
            "device -?K1: ours deleted it and theirs changed function_text, tech_data",
        ),
        e("folio 102", "folio 102: ours and theirs differ in title.en"),
        e(
            "the file of folio 102",
            "the file of folio 102: the merge could not read theirs; its lines are held in a conflict record",
        ),
        e(
            "the layout file of +J-E-2416",
            "the layout file of +J-E-2416: the merge could not read theirs; its lines are held in a conflict record",
        ),
        e(
            "devices/0a.jsonl",
            "devices/0a.jsonl: the merge could not read theirs; its lines are held in a conflict record",
        ),
    ];
    found.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    expected.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    assert_eq!(found, expected);
    let online = run(&q, &FULL, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-011")
        .count();
    assert_eq!(online, 0);
}

/// A conflict on a connection whose end is a junction names that end in words, in en-GB
/// and in nl-NL (FT-680, FT-912).
#[test]
fn ew_core_011_names_a_junction_end_in_words() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.conn(0x2001, P1, Ep::J(0x3001), cp(0x1001), &json!({}));
    fx.junction(0x3001, P1, &[0x2001]);
    fx.push(json!({
        "$k": "conflict", "id": us(0x2001), "kind": "connection", "base": {},
        "ours": {"wire_colour": "BK"}, "theirs": {"wire_colour": "BU"}
    }));
    let m = fx.build();
    let q = Q::new(&m);
    let found: Vec<_> = run(&q, &FULL, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-011")
        .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    for (lang, text) in [
        (
            "en-GB",
            "connection from a junction to -1K4097:1 on folio 1: ours and theirs differ in wire_colour",
        ),
        (
            "nl-NL",
            "verbinding van een aftakking naar -1K4097:1 op folio 1: onze en hun versie verschillen in wire_colour",
        ),
    ] {
        assert_eq!(ew_checks::render(&found[0], lang), text, "{lang}");
    }
}

/// EW-CORE-011: a record theirs deleted and ours changed is named so.
#[test]
fn ew_core_011_names_a_record_theirs_deleted() {
    let mut fx = Fx::base();
    fx.device(DEVICE, "K", Some(1));
    fx.push(json!({
        "$k": "conflict", "id": us(DEVICE), "kind": "device", "base": {},
        "ours": {"function_text": "Relay"}, "theirs": "deleted"
    }));
    let m = fx.build();
    let q = Q::new(&m);
    let found: Vec<String> = run(&q, &FULL, CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-011")
        .map(|f| f.message)
        .collect();
    assert_eq!(
        found,
        ["device -?K1: theirs deleted it and ours changed function_text"]
    );
}

/// EW-CORE-013 (doc 15 §6.4): after a merge, the model holding a conflict record, two devices that share a tag are one
/// group: both devices its targets, the uniqueness rule its discriminator. Without merge evidence the rule is silent.
#[test]
fn ew_core_013_groups_a_tag_two_devices_share_after_a_merge() {
    let fx = |merged: bool| {
        let mut fx = Fx::base();
        fx.drawn_device(0x2001, "K", Some(1), P1);
        fx.drawn_device(0x2002, "K", Some(1), P1);
        if merged {
            fx.push(json!({
                "$k": "conflict", "id": us(0x2001), "kind": "device", "base": {},
                "ours": {"function_text": "Relay"}, "theirs": {"function_text": "Contactor"}
            }));
        }
        fx
    };
    assert!(findings_of(&fx(false).build(), "EW-CORE-013").is_empty());
    let f = findings_of(&fx(true).build(), "EW-CORE-013");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].discriminator, "EW-RDS-004");
    assert_eq!(f[0].targets.len(), 2);
    // No tag is frozen: the oldest keeps it, the one other must change, and the fix renumbers it.
    let arg = |name: &str| {
        f[0].args
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!((arg("keeper"), arg("others")), (Some("oldest"), Some("one")));
    assert!(f[0].fix.is_some(), "{f:#?}");
    // The device whose tag is frozen keeps it, though its ID is the newer.
    let mut frozen = fx(true);
    frozen.rec_mut(&us(0x2002))["tag"]["frozen"] = json!({"text": "1K1", "sign": "hidden"});
    let f = findings_of(&frozen.build(), "EW-CORE-013");
    let arg = |name: &str| {
        f[0].args
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(arg("keeper"), Some("frozen"));
    // The fix renumbers the other, the older.
    let fix = f[0].fix.as_ref().expect("the fix");
    assert_eq!(fix.params["scope"]["selection"], json!([us(0x2001)]));
}
