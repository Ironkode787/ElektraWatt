// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The check engine through the session (doc 15 §3): the `pre-delivery` scheme on Golden A, memoised
//! against cold and under the oracle, and the shipped prevent rules as the commit validator (D-12, and the
//! leniency towards duplicates a model already holds, FR-CHK-011).
//!
//! The Golden A expectations are the rules the MVP engine implements so far; the full target is doc 20
//! §18.1 (42 errors, 32 warnings, 31 infos, 1 waived). `golden_a_pre_delivery_summary` prints the per-rule
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

/// The memoised check equals the cold one (two independent halves of one claim, split so that nextest runs
/// them in parallel; the oracle half is `golden_a_check_memoised_equals_cold_and_the_oracle_is_quiet_oracle`).
#[test]
fn golden_a_check_memoised_equals_cold_and_the_oracle_is_quiet_memo() {
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
}

/// The oracle is quiet over the full online check and the pre-delivery check of Golden A.
#[test]
fn golden_a_check_memoised_equals_cold_and_the_oracle_is_quiet_oracle() {
    let batch = golden_common::batch();
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

/// The open EW-POT-001 and EW-POT-002 findings of a session (pre-delivery, online).
fn pot_findings(s: &Session) -> Vec<checks::Finding> {
    s.check(&PRE_DELIVERY, CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.is_open() && (f.rule == "EW-POT-001" || f.rule == "EW-POT-002"))
        .collect()
}

/// The names of the potentials of a session's model, sorted.
fn potential_names(s: &Session) -> Vec<String> {
    let q = ew_derive::Q::new(&s.world().model);
    let part = q.get::<ew_derive::PotentialPartition>(());
    let mut names: Vec<String> = part
        .potentials()
        .map(|p| q.get::<ew_derive::PotentialNameOf>(p.id).name.text.clone())
        .collect();
    names.sort();
    names
}

/// WP-S1.16 P1-2 (D2, D3; FT-551): Golden A without its bond B1 (D-22: W27 is then a plain wire, 0 V and PE merge)
/// reports EW-POT-001 with the fix "Declare 0 V–PE bond (PELV)" on W27, the wire on the PE side; running the fix's
/// command as `messages.fix` does leaves 0V and PE two potentials, no EW-POT-001 and EW-POT-002 satisfied by the one
/// bond; undo brings the finding back.
#[test]
fn the_0v_pe_bond_fix_declares_b1_on_w27_and_undo_restores_the_finding() {
    let batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let mut s = golden_common::session_with(&batch, |_, records| {
        records.retain(|r| !matches!(r, Record::Bond(_)));
    });
    let names = potential_names(&s);
    assert!(
        !names.contains(&"0V".to_owned()) || !names.contains(&"PE".to_owned()),
        "without B1 0V and PE are one potential: {names:?}"
    );
    let before = pot_findings(&s);
    let conflict: Vec<&checks::Finding> = before.iter().filter(|f| f.rule == "EW-POT-001").collect();
    assert_eq!(conflict.len(), 1, "{before:#?}");
    let fix = conflict[0]
        .fix
        .clone()
        .unwrap_or_else(|| panic!("the bond fix: {:#?}", conflict[0]));
    assert_eq!(fix.label, "Declare 0 V–PE bond (PELV)");
    assert_eq!(fix.command, "bond.create");
    assert_eq!(fix.params["kind"], "pelv_0v_pe");
    assert_eq!(
        fix.params["via"],
        ids["connections|W27"].as_str(),
        "{}",
        fix.params
    );

    // The command path of `messages.fix`: the finding's command with its parameters.
    let cmd = AnyCommand {
        name: fix.command.clone(),
        params: fix.params.clone(),
    };
    s.execute(&cmd).expect("the fix commits");
    let names = potential_names(&s);
    assert!(
        names.contains(&"0V".to_owned()) && names.contains(&"PE".to_owned()),
        "0V and PE are two potentials again: {names:?}"
    );
    assert!(
        pot_findings(&s).is_empty(),
        "no EW-POT-001, and EW-POT-002 is satisfied by the one bond: {:#?}",
        pot_findings(&s)
    );

    s.undo(&ew_api::Origin::Gui).expect("undo");
    let after = pot_findings(&s);
    assert_eq!(
        after.iter().map(|f| f.fingerprint).collect::<Vec<_>>(),
        before.iter().map(|f| f.fingerprint).collect::<Vec<_>>(),
        "undo restores the finding"
    );
}

// ---------------------------------------------------------------------------------------------------------
// WP-S1.16 part 2: the EW-POT pack on Golden A (doc 20 §18.1; P2-2).

/// The open findings of `rule` in a run, as `(severity, subject)` sorted.
fn subjects(report: &checks::CheckReport, rule: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = report
        .findings
        .iter()
        .filter(|f| f.is_open() && f.rule == rule)
        .map(|f| (f.severity.name().to_owned(), f.subject.clone()))
        .collect();
    out.sort();
    out
}

/// The expected open findings of `rule` on Golden A (doc 20 §18.1's rows), as `(severity, subject)` sorted.
fn expected(ef: &[(&str, &str)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = ef
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect();
    out.sort();
    out
}

/// P2-2 / doc 20 §18.1: the EW-POT pack of the pre-delivery scheme reports on Golden A exactly the rows of EF-01
/// (EW-POT-013: T55, S56), EF-06 (EW-POT-006: W6, numbered 101.8), EF-09 (EW-POT-009: -102F2:13, :14), EF-10
/// (EW-POT-003: -201U1:3, -202U1:3) and EF-11 (EW-POT-005: the BU cores W36 and W41 of -106W1 and -106W2), and nothing for EW-POT-001, -002, -004, -008, -014, -015 and -017; the online-only rules
/// EW-POT-011, -012 and -016 report nothing in the drafting scheme.
#[test]
fn golden_a_ew_pot_pack_reports_the_ef_rows() {
    let batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let s = golden_common::session(&batch);
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let first = |rule: &str, subject: &str| -> checks::Finding {
        report
            .findings
            .iter()
            .find(|f| f.rule == rule && f.subject == subject)
            .cloned()
            .unwrap_or_else(|| panic!("{rule} on {subject}: {:#?}", report.findings))
    };
    assert_eq!(
        subjects(&report, "EW-POT-013"),
        expected(&[("error", "T55"), ("error", "S56")]),
        "EF-01"
    );
    assert_eq!(
        first("EW-POT-013", "T55").message,
        "T55 switches the neutral N-55 → N-55T; load E55 stays connected to L-55."
    );
    assert_eq!(
        first("EW-POT-013", "S56").message,
        "S56 switches the neutral N-55 → N-55S; load E56 stays connected to L-55."
    );
    assert_eq!(
        subjects(&report, "EW-POT-006"),
        expected(&[("error", "101.8")]),
        "EF-06"
    );
    let w6 = first("EW-POT-006", "101.8");
    assert_eq!(
        w6.message,
        "PE wire 101.8 has 1.5 mm², below the 2.5 mm² required for the 2.5 mm² line conductors at 1X1 (the sizing \
         table is not verified against the standard)"
    );
    assert_eq!(
        w6.targets[0].id.to_string(),
        ids["connections|W6"],
        "the finding is W6"
    );
    assert_eq!(
        subjects(&report, "EW-POT-009"),
        expected(&[("warning", "-102F2:13"), ("warning", "-102F2:14")]),
        "EF-09"
    );
    assert_eq!(
        subjects(&report, "EW-POT-003"),
        expected(&[("warning", "-201U1:3"), ("warning", "-202U1:3")]),
        "EF-10"
    );
    assert_eq!(
        subjects(&report, "EW-POT-005"),
        expected(&[("warning", "-106W1/BU"), ("warning", "-106W2/BU")]),
        "EF-11"
    );
    let cores: std::collections::BTreeSet<String> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-POT-005")
        .map(|f| f.targets[0].id.to_string())
        .collect();
    assert_eq!(
        cores,
        [ids["connections|W36"].clone(), ids["connections|W41"].clone()]
            .into_iter()
            .collect(),
        "EF-11 is W36 and W41"
    );
    for zero in [
        "EW-POT-001",
        "EW-POT-002",
        "EW-POT-004",
        "EW-POT-008",
        "EW-POT-014",
        "EW-POT-015",
        "EW-POT-017",
    ] {
        assert_eq!(subjects(&report, zero), Vec::<(String, String)>::new(), "{zero}");
    }
    let online = s.check(&checks::DRAFTING, CheckMode::Online);
    for zero in ["EW-POT-011", "EW-POT-012", "EW-POT-016"] {
        assert_eq!(subjects(&online, zero), Vec::<(String, String)>::new(), "{zero}");
    }
}

/// D-38 (P2-2): W6 at 2.5 mm² clears EW-POT-006.
#[test]
fn golden_a_d38_a_2_5_mm2_pe_clears_pot006() {
    let batch = golden_common::batch();
    let w6 = golden_common::ids_by_key(&batch)["connections|W6"].clone();
    let s = golden_common::session_with(&batch, |_, records| {
        for r in records.iter_mut() {
            if let Record::Connection(c) = r
                && c.id.to_string() == w6
            {
                c.overrides.csa = Some(ew_schema::Csa::Mm2("2.5".parse().expect("a decimal")));
            }
        }
    });
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert_eq!(subjects(&report, "EW-POT-006"), Vec::<(String, String)>::new());
}

/// D-21, MVP part (P2-2): clearing `rail_bonded` on the -X1032 terminals cuts their PE off the location's rail, so
/// +J-E-2416 holds two PE potentials: one EW-POT-008 warning naming both.
#[test]
fn golden_a_d21_unbonded_x1032_is_one_pe_island() {
    let batch = golden_common::batch();
    let ids = golden_common::ids_by_key(&batch);
    let strip = ids["devices|-X1032"].clone();
    let s = golden_common::session_with(&batch, |_, records| {
        for r in records.iter_mut() {
            if let Record::Device(d) = r
                && let Some(t) = d.aspects.terminal.as_mut()
                && t.strip.to_string() == strip
            {
                t.rail_bonded = false;
            }
        }
    });
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let islands: Vec<&checks::Finding> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-POT-008")
        .collect();
    assert_eq!(islands.len(), 1, "{islands:#?}");
    assert_eq!(islands[0].subject, "+J-E-2416");
    assert!(
        islands[0]
            .message
            .starts_with("+J-E-2416 holds 2 PE potentials: "),
        "{}",
        islands[0].message
    );
}
