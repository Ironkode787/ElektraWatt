// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE-011 "Conflict records present" over the model (WP-S1.17 part 5, decision log ID-1279): one release-mode
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

/// FT-1291 (ID-1315 ruling 7): a conflict on a connection whose end is a junction names that end in words, in en-GB
/// and in nl-NL (whose message falls back to en-GB's words, FT-680).
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
    for lang in ["en-GB", "nl-NL"] {
        assert_eq!(ew_checks::render(&found[0], lang), "connection from a junction to -1K4097:1 on folio 1: ours and theirs differ in wire_colour", "{lang}");
    }
}
