// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The check engine through the session (doc 15 §3): the `pre-delivery` scheme on the sample, memoised against cold
//! and under the oracle, the shipped prevent rules as the commit validator (D-12, and the leniency towards duplicates
//! a model already holds, FR-CHK-011), and the quick fixes and rules of the packs on defects the set-ups make.
//!
//! The sample is clean under the pre-delivery checks (two infos of EW-NUM-001 on the enclosure's frozen tags), so a
//! rule's findings are made by an edit of a copy of its records; the messages of the rules are pinned whole in
//! `ew-checks` (`tests/it/pot_pack.rs`, `messages.rs`).

use crate::common;

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

/// The sample's records edited by `edit`, as a session with `memo`.
fn sample_with(memo: MemoMode, edit: impl FnOnce(&mut Vec<Record>)) -> Session {
    let (project, mut records, lib) = common::sample_records();
    edit(&mut records);
    Session::from_records(
        project,
        records,
        lib,
        SessionOptions {
            memo,
            ..SessionOptions::default()
        },
    )
    .expect("the edited sample is a valid model")
}

/// The device tagged `tag`, as its ID text.
fn id_of(tag: &str) -> String {
    common::sample()
        .resolve_reference(&format!("tag:{tag}"))
        .unwrap_or_else(|e| panic!("{tag}: {e:?}"))
        .to_string()
}

/// The ID of the sample's wire numbered `number`.
fn wire(number: &str) -> String {
    common::sample()
        .navigator_table(ew_api::output::NavKind::Connections)
        .nodes
        .iter()
        .find(|n| n.table_row && n.label == number)
        .map(|n| n.records[0].to_string())
        .unwrap_or_else(|| panic!("no wire {number}"))
}

/// The sample under pre-delivery in release mode: no finding at all (its enclosure's tags are the scheme's), and
/// in particular nothing of FR-CHK-016 (designations) and FR-CHK-017 (interruption points).
#[test]
fn the_sample_is_clean_under_pre_delivery() {
    let s = common::sample();
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let by_rule = counts(&report);
    assert_eq!(by_rule, BTreeMap::new(), "{:#?}", report.findings);
    assert_eq!((report.counts.errors, report.counts.warnings), (0, 0));
    assert_eq!(report.exit_code(), 0);
}

/// The memoised check equals the cold one (two independent halves of one claim, split so that nextest runs them in
/// parallel; the oracle half is `the_check_memoised_equals_cold_and_the_oracle_is_quiet_oracle`). The rules run over
/// the memo store and over a cold query; the layout rules' findings, which the session joins from the pages it draws,
/// are left out of both.
#[test]
fn the_check_memoised_equals_cold_and_the_oracle_is_quiet_memo() {
    let memo = common::sample();
    let first = checks::run(&memo.query(), &PRE_DELIVERY, CheckMode::Release);
    let again = checks::run(&memo.query(), &PRE_DELIVERY, CheckMode::Release);
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

/// The oracle is quiet over the full online check and the pre-delivery check of the sample, re-running one memoised
/// read in 16 cold per gate (a fixed, deterministic sample). The value equality of the whole report is the `_memo`
/// half's; every read runs through the oracle in the deep nightly (`nightly_the_check_oracle_is_quiet_on_every_read`).
#[test]
fn the_check_memoised_equals_cold_and_the_oracle_is_quiet_oracle() {
    check_oracle_is_quiet(ew_derive::Oracle::OneIn(
        std::num::NonZeroU32::new(16).expect("n is not zero"),
    ));
}

/// The deep nightly's twin of `the_check_memoised_equals_cold_and_the_oracle_is_quiet_oracle`: every memoised read
/// of both checks re-run cold.
#[test]
#[ignore = "deep run: the check oracle on every read"]
fn nightly_the_check_oracle_is_quiet_on_every_read() {
    check_oracle_is_quiet(ew_derive::Oracle::Every);
}

fn check_oracle_is_quiet(sample: ew_derive::Oracle) {
    let oracle = sample_with(MemoMode::Oracle, |_| {});
    let _ = oracle.take_oracle_mismatches();
    let store = oracle.memo().expect("memoising session");
    store.set_oracle(sample);
    let checks_before = store.total_stats().total().oracle_checks;
    let _ = oracle.check(&checks::FULL, CheckMode::Online);
    let _ = oracle.check(&PRE_DELIVERY, CheckMode::Release);
    assert!(oracle.take_oracle_mismatches().is_empty());
    assert!(
        store.total_stats().total().oracle_checks > checks_before,
        "the oracle checked reads of the checks"
    );
}

/// The sample with -X1021:4 renamed to "3", so the strip holds a duplicate before any edit (as a lenient import would
/// commit it).
fn session_with_duplicate() -> (Session, String) {
    let (t3, t4) = (id_of("-X1021:3"), id_of("-X1021:4"));
    let s = sample_with(MemoMode::On, |records| {
        for r in records.iter_mut() {
            if let Record::Device(d) = r
                && d.id.to_string() == t4
                && let Some(t) = d.aspects.terminal.as_mut()
            {
                t.designation = "3".parse().expect("an identifier");
            }
        }
    });
    (s, t3)
}

#[test]
fn an_existing_duplicate_is_reported_but_does_not_block_other_edits() {
    let (mut s, t3) = session_with_duplicate();
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
        params: serde_json::json!({"assignments": [{"terminal": t3, "designation": "9"}]}),
    };
    s.execute(&cmd).expect("the re-designation commits");
    let report = s.check(&checks::MIGRATION, CheckMode::Release);
    assert_eq!(counts(&report).get("EW-TERM-002"), None, "{:#?}", report.findings);
}

#[test]
fn a_new_duplicate_is_prevented_with_the_rule_and_a_remedy() {
    let mut s = common::sample();
    let cmd = AnyCommand {
        name: "terminal.designate".into(),
        params: serde_json::json!({"assignments": [{"terminal": id_of("-X1021:3"), "designation": "4"}]}),
    };
    let rev = s.world().rev;
    match s.execute(&cmd) {
        Err(CmdError::Prevented {
            rule, finding, keys, ..
        }) => {
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
    let (project, records, lib) = common::sample_records();
    let options = SessionOptions {
        prevent: Some(Box::new(AcceptAll)),
        memo: MemoMode::Off,
        ..SessionOptions::default()
    };
    let mut s = Session::from_records(project, records, lib, options).unwrap();
    let cmd = AnyCommand {
        name: "terminal.designate".into(),
        params: serde_json::json!({"assignments": [{"terminal": id_of("-X1021:3"), "designation": "4"}]}),
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

/// The sample without its bond B1 (D-22: the wire 101.5 from -X1011:PE to the 0 V terminal is then a plain wire, 0 V
/// and PE merge) reports EW-POT-001 with the fix "Declare 0 V–PE bond (PELV)" on the wire the bond named; running the
/// fix's command as `messages.fix` does leaves 0V and PE two potentials, no EW-POT-001 and EW-POT-002 satisfied by
/// the one bond; undo brings the finding back.
#[test]
fn the_0v_pe_bond_fix_declares_the_bond_on_its_wire_and_undo_restores_the_finding() {
    let (_, records, _) = common::sample_records();
    let via = records
        .iter()
        .find_map(|r| match r {
            Record::Bond(b) => serde_json::to_value(b).ok().map(|v| v["via"].clone()),
            _ => None,
        })
        .expect("the sample's bond B1");
    let mut s = sample_with(MemoMode::On, |records| {
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
    assert_eq!(fix.params["via"], via, "{}", fix.params);

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

/// EW-POT-009's message says "connect it or mark it unused", and its quick fix does the second. The sample without the lamp -102E1's 0 V wire (102.9) has that pin unconnected; the fix
/// marks it unused with one `fields.set`; the finding is then an info without a fix.
#[test]
fn the_unconnected_pins_fix_marks_it_unused() {
    let lamp_wire = wire("102.9");
    let mut s = sample_with(MemoMode::On, |records| {
        records.retain(|r| !matches!(r, Record::Connection(c) if c.id.to_string() == lamp_wire));
    });
    let open = |s: &Session| -> Vec<checks::Finding> {
        s.check(&PRE_DELIVERY, CheckMode::Release)
            .findings
            .into_iter()
            .filter(|f| f.rule == "EW-POT-009")
            .collect()
    };
    let before = open(&s);
    let warnings = |fs: &[checks::Finding]| fs.iter().filter(|f| f.fix.is_some()).count();
    let n = warnings(&before);
    assert!(n >= 1, "{before:#?}");
    let fix = before.iter().find_map(|f| f.fix.clone()).expect("a fix");
    assert_eq!(
        (fix.label.to_string().as_str(), fix.command.as_str()),
        ("Mark not used", "fields.set")
    );
    s.execute(&AnyCommand {
        name: fix.command,
        params: fix.params,
    })
    .expect("the fix commits");
    let after = open(&s);
    assert_eq!(after.len(), before.len(), "the finding stays, as an info");
    assert_eq!(warnings(&after), n - 1, "{after:#?}");
}

// ---------------------------------------------------------------------------------------------------------
// The EW-POT pack on the sample.

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

/// The ID of the sample's shield wire of -103W1 on 103.
fn shield() -> String {
    common::sample_shield(&common::sample()).to_string()
}

/// The EW-POT pack of the pre-delivery scheme reports nothing on the sample, the online-only rules nothing in the
/// drafting scheme (the shield takes the 2.5 mm² the rating at its motor proposes, set by nobody); D-38: the
/// shield wire set below its 2.5 mm² line conductors is an undersized PE wire (EW-POT-006, its target that wire), at
/// 2.5 mm² the finding is gone.
#[test]
fn the_ew_pot_pack_is_quiet_and_an_undersized_pe_wire_is_pot006() {
    let s = common::sample();
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    for rule in [
        "EW-POT-001",
        "EW-POT-002",
        "EW-POT-003",
        "EW-POT-004",
        "EW-POT-005",
        "EW-POT-006",
        "EW-POT-008",
        "EW-POT-009",
        "EW-POT-013",
        "EW-POT-014",
        "EW-POT-015",
        "EW-POT-017",
        "EW-POT-021",
    ] {
        assert!(
            report.rules.iter().any(|r| r.id == rule),
            "{rule} ran in the pre-delivery scheme"
        );
        assert_eq!(subjects(&report, rule), Vec::<(String, String)>::new(), "{rule}");
    }
    let online = s.check(&checks::DRAFTING, CheckMode::Online);
    for zero in ["EW-POT-011", "EW-POT-012", "EW-POT-016"] {
        assert_eq!(subjects(&online, zero), Vec::<(String, String)>::new(), "{zero}");
    }
    let shield = shield();
    let bare = sample_with(MemoMode::On, |records| {
        for r in records.iter_mut() {
            if let Record::Connection(c) = r
                && c.id.to_string() == shield
            {
                c.overrides.csa = Some(ew_schema::Csa::Mm2(ew_units::Dec3(1_500)));
            }
        }
    });
    let report = bare.check(&PRE_DELIVERY, CheckMode::Release);
    let found: Vec<&checks::Finding> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-POT-006")
        .collect();
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, checks::Severity::Error);
    assert_eq!(
        found[0].targets[0].id.to_string(),
        shield,
        "the finding is the shield wire"
    );
}

/// D-21, MVP part: the sample with the PE wire of its power supply -101G1 (the star's branch from -X1011:PE, which the
/// connections navigator numbers 101.6) deleted holds a PE potential that reaches neither the incoming PE nor an earth
/// symbol: one EW-POT-008 warning naming it by its pin and its page.
#[test]
fn a_pe_branch_deleted_leaves_one_pe_island() {
    let pe_wire = wire("101.6");
    let mut s = common::sample();
    // The wire goes by the delete command, which keeps its junction's sequence whole.
    let cmd = AnyCommand {
        name: "edit.delete".into(),
        params: serde_json::json!({"targets": [{"kind": "connection", "id": pe_wire}]}),
    };
    match s.execute(&cmd) {
        Err(CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
    .expect("the PE wire is deleted");
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let islands: Vec<&checks::Finding> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-POT-008")
        .collect();
    assert_eq!(islands.len(), 1, "{islands:#?}");
    assert_eq!(
        islands[0].message(),
        "The protective earth at -101G1:PE on page 101 reaches neither the incoming PE nor an earth symbol of +CAB. \
         Connect it to the PE rail or to an earth symbol, or put its PE terminal on a bonded rail."
    );
}

// ---------------------------------------------------------------------------------------------------------
// Cables, parts, devices, pages and texts on the sample.

/// The EW-CORE rules of cables, parts and devices (EW-CORE-008, -031 … -048) and of pages, texts, designations and
/// terminals report nothing on the sample in the pre-delivery scheme, the rule system itself (EW-CORE-083, -084) is
/// quiet, and online in the drafting scheme EW-CORE-037, -063, -071, EW-RDS-002 (the sample's location codes have no
/// separator) and EW-TERM-003 report nothing.
#[test]
fn the_core_rds_and_term_rules_are_quiet_on_the_sample() {
    let s = common::sample();
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    for zero in [
        "EW-CORE-008",
        "EW-CORE-021",
        "EW-CORE-022",
        "EW-CORE-025",
        "EW-CORE-026",
        "EW-CORE-031",
        "EW-CORE-032",
        "EW-CORE-034",
        "EW-CORE-036",
        "EW-CORE-041",
        "EW-CORE-042",
        "EW-CORE-043",
        "EW-CORE-044",
        "EW-CORE-045",
        "EW-CORE-048",
        "EW-CORE-061",
        "EW-CORE-083",
        "EW-NUM-002",
        "EW-RDS-001",
        "EW-RDS-003",
        "EW-RDS-007",
        "EW-RDS-009",
        "EW-RDS-010",
        "EW-TERM-004",
    ] {
        assert!(
            report.rules.iter().any(|r| r.id == zero),
            "{zero} ran in the pre-delivery scheme"
        );
        assert_eq!(subjects(&report, zero), Vec::<(String, String)>::new(), "{zero}");
    }
    let online = s.check(&checks::DRAFTING, CheckMode::Online);
    for zero in [
        "EW-CORE-037",
        "EW-CORE-063",
        "EW-CORE-071",
        "EW-RDS-002",
        "EW-TERM-003",
    ] {
        assert_eq!(subjects(&online, zero), Vec::<(String, String)>::new(), "{zero}");
    }
}

/// D-15 (P3-2): a `Tbd` channel address is an open item: EW-PLC-015 reports it as an error in release
/// mode and supersedes EW-CORE-044's finding for the same value, so it prints once.
#[test]
fn a_tbd_channel_address_is_an_open_item() {
    let mut done = false;
    let s = sample_with(MemoMode::On, |records| {
        for r in records.iter_mut() {
            if let Record::Channel(c) = r
                && !done
            {
                c.address = ew_schema::AddrSpec::Tbd;
                done = true;
            }
        }
    });
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let open: Vec<&checks::Finding> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-044" || f.rule == "EW-PLC-015")
        .collect();
    assert_eq!(open.len(), 1, "{open:#?}");
    assert_eq!(open[0].rule, "EW-PLC-015");
    assert_eq!(open[0].severity, checks::Severity::Error);
    assert_eq!(open[0].discriminator, "address");
    assert!(
        open[0].message().contains("is TBD; enter it before release"),
        "{}",
        open[0].message()
    );
}
