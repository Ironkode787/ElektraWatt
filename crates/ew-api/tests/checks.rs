// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The check engine through the session (doc 15 §3): the `pre-delivery` scheme on Golden A, memoised
//! against cold and under the oracle, and the shipped prevent rules as the commit validator (D-12, and the
//! leniency towards duplicates a model already holds, FR-CHK-011).
//!
//! The Golden A expectations are the rules the MVP engine implements so far; the full target is doc 20
//! §18.1 (42 errors, 32 warnings, 28 infos, 1 waived). `golden_a_pre_delivery_summary` prints the per-rule
//! counts for the comparison.

mod golden_common;

use std::collections::BTreeMap;

use ew_api::checks::{self, CheckMode, PRE_DELIVERY};
use ew_api::{AnyCommand, CmdError, MemoMode, Session, SessionOptions};
use ew_schema::Record;

fn counts(report: &checks::CheckReport) -> BTreeMap<&'static str, usize> {
    let mut out = BTreeMap::new();
    for f in report.findings.iter().filter(|f| f.is_open()) {
        *out.entry(f.rule).or_default() += 1;
    }
    out
}

#[test]
fn golden_a_pre_delivery_summary() {
    let batch = golden_common::batch();
    let s = golden_common::session(&batch);
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let by_rule = counts(&report);
    println!(
        "pre-delivery on Golden A: {:?}; rules evaluated: {:?}",
        report.counts,
        report.rules.iter().map(|r| r.id).collect::<Vec<_>>()
    );
    for f in &report.findings {
        println!(
            "  {} {} {} {}: {}",
            f.severity, f.rule, f.code, f.subject, f.message
        );
    }
    // doc 20 §18.1: "every other rule of the MVP packs reports nothing on Golden A", in particular
    // FR-CHK-016 (designations) and FR-CHK-017 (22 pairs, 4 external points).
    for zero in [
        "EW-TERM-001",
        "EW-TERM-002",
        "EW-RDS-004",
        "EW-CORE-051",
        "EW-POT-001",
    ] {
        assert_eq!(by_rule.get(zero), None, "{zero}: {:#?}", report.findings);
    }
    // EF-08: -3W1 has no length.
    assert_eq!(by_rule.get("EW-CORE-033"), Some(&1), "{:#?}", report.findings);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == "EW-CORE-033" && f.subject.ends_with("-3W1"))
    );
    assert_eq!(report.exit_code(), i32::from(report.counts.errors > 0));
}

#[test]
fn golden_a_check_memoised_equals_cold_and_the_oracle_is_quiet() {
    let batch = golden_common::batch();
    let memo = golden_common::session(&batch);
    let first = memo.check(&PRE_DELIVERY, CheckMode::Release);
    let again = memo.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(
        first, again,
        "a second run over the memo store gives the same report"
    );
    let cold = checks::run(
        &memo.query_over(&memo.world().model),
        &PRE_DELIVERY,
        CheckMode::Release,
    );
    assert_eq!(first, cold, "memoised and cold runs agree");

    let oracle = golden_common::session_oracle(&batch);
    let _ = oracle.check(&checks::FULL, CheckMode::Online);
    let _ = oracle.check(&PRE_DELIVERY, CheckMode::Release);
    assert!(oracle.take_oracle_mismatches().is_empty());
}

/// The golden batch with -X1031:4 renamed to "3", so the strip holds a duplicate before any edit (as a
/// lenient import would commit it).
fn session_with_duplicate(
    batch: &ew_import_tables::ew_interop_api::ImportBatch,
) -> (Session, String, String) {
    let ids = golden_common::ids_by_key(batch);
    let (t3, t4) = (
        ids["terminals|-X1031:3"].clone(),
        ids["terminals|-X1031:4"].clone(),
    );
    let s = golden_common::session_with(batch, |_, records| {
        for r in records.iter_mut() {
            if let Record::Device(d) = r
                && d.id.to_string() == t4
                && let Some(t) = d.aspects.terminal.as_mut()
            {
                t.designation = "3".parse().expect("an identifier");
            }
        }
    });
    (s, t3, t4)
}

#[test]
fn an_existing_duplicate_is_reported_but_does_not_block_other_edits() {
    let batch = golden_common::batch();
    let (mut s, t3, _) = session_with_duplicate(&batch);
    let report = s.check(&checks::MIGRATION, CheckMode::Release);
    assert_eq!(
        counts(&report).get("EW-TERM-002"),
        Some(&1),
        "{:#?}",
        report.findings
    );
    // Editing a member of the duplicate without touching its designation commits (FR-CHK-011).
    let cmd = AnyCommand {
        name: "fields.set".into(),
        params: serde_json::json!({"kind": "device", "targets": [t3], "path": "function_text", "value": {"en-GB": "x"}}),
    };
    s.execute(&cmd)
        .expect("an edit that leaves the duplicate alone commits");
    // Resolving it with terminal.designate commits too, and the finding goes.
    let cmd = AnyCommand {
        name: "terminal.designate".into(),
        params: serde_json::json!({"assignments": [{"terminal": t3, "designation": "21"}]}),
    };
    s.execute(&cmd).expect("the re-designation commits");
    let report = s.check(&checks::MIGRATION, CheckMode::Release);
    assert_eq!(counts(&report).get("EW-TERM-002"), None, "{:#?}", report.findings);
}

#[test]
fn a_new_duplicate_is_prevented_with_the_rule_and_a_remedy() {
    let batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let mut s = golden_common::session(&batch);
    let cmd = AnyCommand {
        name: "terminal.designate".into(),
        params: serde_json::json!({"assignments": [{"terminal": ids["terminals|-X1031:3"], "designation": "4"}]}),
    };
    let rev = s.world().rev;
    match s.execute(&cmd) {
        Err(CmdError::Prevented { rule, finding, keys }) => {
            assert_eq!(rule, "EW-TERM-002");
            assert!(finding.contains("taken"), "{finding}");
            assert!(finding.contains("swap 3"), "{finding}");
            assert_eq!(keys.len(), 2);
        }
        other => panic!("expected Prevented, got {other:?}"),
    }
    assert_eq!(s.world().rev, rev, "a prevented command changes nothing");
}

#[test]
fn a_session_with_its_own_hook_does_not_install_the_shipped_rules() {
    struct AcceptAll;
    impl ew_model::PreventHook for AcceptAll {
        fn check(
            &self,
            _: &ew_model::Model,
            _: &ew_model::Model,
            _: &[ew_model::AnyKey],
        ) -> Result<(), ew_model::PreventViolation> {
            Ok(())
        }
    }
    let batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let options = SessionOptions {
        prevent: Some(Box::new(AcceptAll)),
        memo: MemoMode::Off,
        ..SessionOptions::default()
    };
    let mut s = Session::from_records(project.unwrap(), records, batch.lib.clone(), options).unwrap();
    let cmd = AnyCommand {
        name: "terminal.designate".into(),
        params: serde_json::json!({"assignments": [{"terminal": ids["terminals|-X1031:3"], "designation": "4"}]}),
    };
    s.execute(&cmd).expect("the custom hook accepts");
}
