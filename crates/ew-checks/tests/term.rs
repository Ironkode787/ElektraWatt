// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-TERM rules: online, release and prevent behaviour on small snapshots.

mod common;

use common::*;
use ew_checks::{CheckMode, DRAFTING, MIGRATION, PRE_DELIVERY, Scope, prevent, run};
use ew_derive::Q;
use ew_derive::ew_ids::Id;
use serde_json::json;

const STRIP: u128 = 0x100;

fn three_terminals(d: [&str; 3]) -> Vec<ew_derive::ew_schema::Record> {
    vec![
        strip(STRIP, 1),
        terminal(0x101, STRIP, 1, d[0]),
        terminal(0x102, STRIP, 2, d[1]),
        terminal(0x103, STRIP, 3, d[2]),
    ]
}

#[test]
fn term002_reports_one_finding_per_duplicated_designation() {
    let m = model(three_terminals(["1", "2", "2"]));
    let q = Q::new(&m);
    let report = run(&q, &PRE_DELIVERY, CheckMode::Release);
    let term: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-TERM-002")
        .collect();
    assert_eq!(term.len(), 1, "{:#?}", report.findings);
    assert_eq!(term[0].targets.len(), 2);
    assert_eq!(term[0].code, "001002");
    assert!(term[0].message.contains("designation 2"), "{}", term[0].message);
    assert_eq!(report.counts.errors, 1);
    assert_eq!(report.exit_code(), 1);
}

#[test]
fn term002_clean_strip_has_no_finding() {
    let m = model(three_terminals(["1", "2", "3"]));
    let q = Q::new(&m);
    let report = run(&q, &MIGRATION, CheckMode::Release);
    assert!(report.findings.iter().all(|f| f.rule != "EW-TERM-002"));
    assert!(report.rules.iter().any(|r| r.id == "EW-TERM-002"));
}

#[test]
fn term002_honours_the_case_setting() {
    let records = three_terminals(["a", "A", "3"]);
    let sensitive = model(records.clone());
    let insensitive = model_with(json!({"formatting": {"tag_case": "insensitive"}}), records);
    let count = |m: &ew_model::Model| {
        let q = Q::new(m);
        run(&q, &DRAFTING, CheckMode::Online)
            .findings
            .iter()
            .filter(|f| f.rule == "EW-TERM-002")
            .count()
    };
    assert_eq!(count(&sensitive), 0);
    assert_eq!(count(&insensitive), 1);
}

#[test]
fn term002_fingerprint_is_stable_across_renaming_the_strip() {
    let a = model(three_terminals(["1", "2", "2"]));
    let mut records = three_terminals(["1", "2", "2"]);
    records[0] = strip(STRIP, 7);
    let b = model(records);
    let fp = |m: &ew_model::Model| {
        let q = Q::new(m);
        run(&q, &PRE_DELIVERY, CheckMode::Release).findings[0].fingerprint
    };
    assert_eq!(fp(&a), fp(&b));
}

#[test]
fn term002_prevent_refuses_a_new_duplicate() {
    let before = model(three_terminals(["1", "2", "3"]));
    let after = model(three_terminals(["1", "2", "2"]));
    let v = prevent(&Q::new(&before), &Q::new(&after), &[dev_key(0x103)]);
    assert_eq!(v.len(), 1, "{v:#?}");
    assert_eq!(v[0].rule, "EW-TERM-002");
    assert!(v[0].message.contains("taken"), "{}", v[0].message);
    assert!(v[0].message.contains("use 3"), "{}", v[0].message);
    assert!(v[0].message.contains("swap 3"), "{}", v[0].message);
}

#[test]
fn term002_prevent_accepts_a_swap() {
    let before = model(three_terminals(["1", "2", "3"]));
    let after = model(three_terminals(["1", "3", "2"]));
    let v = prevent(
        &Q::new(&before),
        &Q::new(&after),
        &[dev_key(0x102), dev_key(0x103)],
    );
    assert!(v.is_empty(), "{v:#?}");
}

#[test]
fn term002_prevent_ignores_an_existing_duplicate_left_alone() {
    // A lenient import committed "2" twice; editing the third terminal leaves the duplicate alone.
    let before = model(three_terminals(["1", "2", "2"]));
    let after = model(three_terminals(["9", "2", "2"]));
    let v = prevent(&Q::new(&before), &Q::new(&after), &[dev_key(0x101)]);
    assert!(v.is_empty(), "{v:#?}");
    // Touching a member without changing its designation is not a new violation either.
    let v = prevent(&Q::new(&before), &Q::new(&before), &[dev_key(0x102)]);
    assert!(v.is_empty(), "{v:#?}");
}

#[test]
fn term002_prevent_refuses_a_new_terminal_with_a_taken_designation() {
    let before = model(three_terminals(["1", "2", "3"]));
    let mut records = three_terminals(["1", "2", "3"]);
    records.push(terminal(0x104, STRIP, 4, "1"));
    let after = model(records);
    let v = prevent(&Q::new(&before), &Q::new(&after), &[dev_key(0x104)]);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].targets.len(), 2);
}

#[test]
fn term001_clean_strip_has_no_finding() {
    // `Ident` cannot be empty, so no snapshot can hold an undesignated terminal; the rule runs and is
    // silent, and it is registered for lenient data and the catalogue.
    let m = model(three_terminals(["1", "2", "3"]));
    assert!(findings_of(&m, "EW-TERM-001").is_empty());
    let meta = ew_checks::rule("EW-TERM-001").expect("registered").meta();
    assert_eq!(meta.code, "001001");
    assert!(meta.modes.prevent && meta.modes.online && meta.modes.release);
}

/// A strip with a rail-bonded terminal; `pe` gives it a function with a PE CP, `bonded` the flag.
fn rail_strip(pe: bool, bonded: bool) -> ew_model::Model {
    let mut fx = Fx::base();
    fx.push(json!({"$k": "device", "id": us(STRIP), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    fx.rail_terminal(0x4001, STRIP, None, pe.then_some(0x1001));
    if !bonded {
        let t = fx
            .records
            .iter_mut()
            .find(|r| r["id"] == json!(us(0x4001)))
            .expect("the terminal");
        t["terminal"]["rail_bonded"] = json!(false);
    }
    fx.build()
}

#[test]
fn term005_reports_a_rail_bonded_terminal_without_a_pe_cp() {
    let f = findings_of(&rail_strip(false, true), "EW-TERM-005");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "001005");
    assert_eq!(f[0].severity, ew_checks::Severity::Error);
    assert_eq!(f[0].discriminator, "");
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(f[0].targets[0].id, uid(0x4001));
    assert!(f[0].message.contains("no PE CP"), "{}", f[0].message);
}

#[test]
fn term005_a_terminal_with_a_pe_cp_or_without_the_flag_has_no_finding() {
    assert!(findings_of(&rail_strip(true, true), "EW-TERM-005").is_empty());
    assert!(findings_of(&rail_strip(false, false), "EW-TERM-005").is_empty());
}

#[test]
fn term005_is_online_only() {
    // Release mode does not run it (the catalogue lists mode o only).
    let m = rail_strip(false, true);
    let report = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release);
    assert!(report.findings.iter().all(|f| f.rule != "EW-TERM-005"));
}

#[test]
fn check_findings_memo_is_per_rule_and_scope() {
    let m = model(three_terminals(["1", "1", "3"]));
    let q = Q::new(&m);
    let f = q.get::<ew_checks::CheckOf>(("EW-TERM-002", Scope::Strip(Id::from_uuid(uid(STRIP).as_uuid()))));
    assert_eq!(f.len(), 1);
    // A scope of another kind gives nothing.
    let none = q.get::<ew_checks::CheckOf>(("EW-TERM-002", Scope::Project));
    assert!(none.is_empty());
}

#[test]
fn a_waiver_matches_by_fingerprint_and_an_unmatched_one_is_reported() {
    let records = three_terminals(["1", "2", "2"]);
    let m = model(records.clone());
    let fp = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release).findings[0].fingerprint;
    let mut with_waivers = records;
    with_waivers.push(record(json!({
        "$k": "waiver", "id": us(0x900), "rule": "EW-TERM-002",
        "target": {"finding": fp.to_string()}, "reason": {"en-GB": "accepted for the test"}
    })));
    with_waivers.push(record(json!({
        "$k": "waiver", "id": us(0x901), "rule": "EW-TERM-002",
        "target": {"finding": "00000000000000000000000000000000"}, "reason": {"en-GB": "matches nothing"}
    })));
    let m = model(with_waivers);
    let report = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release);
    assert_eq!(report.counts.waived, 1, "{:#?}", report.findings);
    assert_eq!(report.counts.errors, 0);
    assert_eq!(report.counts.infos, 1, "EW-CORE-081 for the second waiver");
    assert!(report.findings.iter().any(|f| f.rule == "EW-CORE-081"));
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn term002_skips_the_importers_fallback_strip_but_counts_real_strips() {
    // The QET importer's fallback strip `X?<page>` (EW-IMP-005) gathers unlabelled terminals: not one strip.
    let fallback = record(json!({
        "$k": "device", "id": us(0x200),
        "tag": {"letter": "X", "frozen": {"text": "X?103"}}, "strip": {}
    }));
    let mut records = three_terminals(["1", "2", "2"]);
    records.push(fallback);
    records.push(terminal(0x201, 0x200, 1, "1"));
    records.push(terminal(0x202, 0x200, 2, "1"));
    let m = model(records);
    let report = run(&Q::new(&m), &MIGRATION, CheckMode::Release);
    let term: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-TERM-002")
        .collect();
    assert_eq!(term.len(), 1, "only the real strip: {term:#?}");
    assert!(term[0].targets.iter().all(|t| t.id != uid(0x201)));
}

#[test]
fn a_waiver_for_a_rule_that_did_not_run_is_not_evaluated() {
    let mut records = three_terminals(["1", "2", "3"]);
    // EW-POT-007 is not implemented (a 1.0 rule); EW-TERM-005 does not run in release mode.
    for (n, rule) in [(0x910, "EW-POT-007"), (0x911, "EW-TERM-005")] {
        records.push(record(json!({
            "$k": "waiver", "id": us(n), "rule": rule,
            "target": {"finding": "00000000000000000000000000000001"}, "reason": {"en-GB": "not evaluated here"}
        })));
    }
    let m = model(records);
    let report = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release);
    let f: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-081")
        .collect();
    assert_eq!(f.len(), 2, "{f:#?}");
    for x in &f {
        assert!(x.message.contains("not evaluated"), "{}", x.message);
        assert!(x.discriminator.starts_with("not-evaluated"));
        assert_eq!(x.severity, ew_checks::Severity::Info);
    }
    assert_eq!(report.exit_code(), 0);
}

#[test]
fn integrity_items_join_the_counts_in_release_mode_only() {
    let m = model(three_terminals(["1", "2", "3"]));
    let items = || {
        vec![
            ew_checks::IntegrityItem {
                rule: "EW-CORE-005".into(),
                severity: ew_checks::Severity::Error,
                location: "pages/p.jsonl:3".into(),
                message: "JSON syntax error".into(),
            },
            // EW-CORE-004 runs on open only (n), not in release mode.
            ew_checks::IntegrityItem {
                rule: "EW-CORE-004".into(),
                severity: ew_checks::Severity::Warning,
                location: "stray.jsonl".into(),
                message: "orphan file".into(),
            },
            ew_checks::IntegrityItem {
                rule: "EW-NOPE-001".into(),
                severity: ew_checks::Severity::Error,
                location: String::new(),
                message: "unknown".into(),
            },
        ]
    };
    let release = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release).with_integrity(items());
    assert_eq!(release.counts.errors, 1, "{:#?}", release.findings);
    assert_eq!(release.counts.warnings, 0);
    assert_eq!(release.exit_code(), 1);
    assert!(
        release
            .findings
            .iter()
            .any(|f| f.rule == "EW-CORE-005" && f.code == "100005")
    );
    assert!(release.rules.iter().any(|r| r.id == "EW-CORE-005"));
    let online = run(&Q::new(&m), &DRAFTING, CheckMode::Online).with_integrity(items());
    assert_eq!(online.exit_code(), 0);
}
