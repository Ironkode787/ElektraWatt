// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.16 part 5 on Golden A: the Fluent messages (P5-2), waiver recording with W-1 (P5-3) and the built-pack subset
//! of AC-21 against `tests/expected/findings_mvp.csv` (P5-4, D10).
//!
//! W-1 lives in a fixture built from a **copy** of the checked-in Golden A (`golden/J-E-2416.ew`): the test opens the
//! copy, records W-1 with `check.waive` from the GUI origin, saves (the store writes `waivers.jsonl`) and reopens it.
//! Nothing under `golden/` changes (WP-S1.16 §10.3).

mod golden_common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_api::checks::{self, CheckMode, CheckReport, Finding, FindingState, PRE_DELIVERY};
use ew_api::{AnyCommand, CmdError, Origin, Recovery, Session, SessionOptions, Submit};
use ew_store::StoreOptions;

/// The built packs of AC-21's subset (doc 15 §4): WP-S1.16's EW-CORE, EW-IEC81346 (EW-RDS and EW-NUM), EW-TERM and
/// EW-POT, and WP-S1.21 part 3's EW-PLC (EF-02 joins the subset).
const FOUR_PACKS: [&str; 5] = ["EW-CORE", "EW-IEC81346", "EW-TERM", "EW-POT", "EW-PLC"];

/// The findings of the four packs that doc 20 §18.1 does not list, each with the follow-up that removes it (FT-638): the
/// process valves are supplied by others and exempt once `supply_scope` exists (FT-209, WP-S7.01; decision log ID-322).
const KNOWN_EXCESS: [(&str, &str, &str); 8] = [
    ("EW-CORE-041", "=P01+FIELD-203Q1", "FT-638 (FT-209 supply_scope)"),
    ("EW-CORE-041", "=P01+FIELD-203Q2", "FT-638 (FT-209 supply_scope)"),
    ("EW-CORE-041", "=P01+FIELD-203Q3", "FT-638 (FT-209 supply_scope)"),
    ("EW-CORE-041", "=P01+FIELD-203Q4", "FT-638 (FT-209 supply_scope)"),
    ("EW-CORE-041", "=P01+FIELD-203Q5", "FT-638 (FT-209 supply_scope)"),
    ("EW-CORE-041", "=P01+FIELD-203Q6", "FT-638 (FT-209 supply_scope)"),
    ("EW-CORE-041", "=P01+FIELD-203Q7", "FT-638 (FT-209 supply_scope)"),
    ("EW-CORE-041", "=P01+FIELD-203Q8", "FT-638 (FT-209 supply_scope)"),
];

/// W-1's reason (doc 20 §18.1 EF-19).
const W1_REASON: &str = "PE stud is part of NSYPMM107";

// ---------------------------------------------------------------------------------------------------------
// The expected file

/// One row of `findings_mvp.csv`.
#[derive(Debug)]
struct Row {
    ef: String,
    rule: String,
    severity: String,
    count: usize,
    subjects: Vec<String>,
    pending: String,
}

/// Splits one CSV line into fields: commas outside double quotes separate; `""` inside quotes is a quote.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let (mut quoted, mut chars) = (false, line.chars().peekable());
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                chars.next();
                out.last_mut().expect("a field").push('"');
            }
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(String::new()),
            (c, _) => out.last_mut().expect("a field").push(c),
        }
    }
    out
}

fn expected_rows() -> Vec<Row> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/expected/findings_mvp.csv");
    let text = std::fs::read_to_string(&path).expect("tests/expected/findings_mvp.csv");
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some("ef,rule,severity,count,subjects,targets,defect,pending")
    );
    lines
        .map(|l| {
            let f = fields(l);
            assert_eq!(f.len(), 8, "{l}");
            Row {
                ef: f[0].clone(),
                rule: f[1].clone(),
                severity: f[2].clone(),
                count: f[3].parse().expect("a count"),
                subjects: f[4]
                    .split("; ")
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
                pending: f[7].clone(),
            }
        })
        .collect()
}

/// The pack of a rule ID: the registered rule's, else the ID's middle part (`EW-PLC-012` → `EW-PLC`).
fn pack_of(rule: &str) -> String {
    checks::rule(rule).map_or_else(
        || rule.rsplit_once('-').map_or(rule, |(p, _)| p).to_owned(),
        |r| r.meta().pack.to_owned(),
    )
}

/// The file states the totals of doc 20 §18.1 (42 errors, 32 warnings, 31 infos, 1 waived); every pending row is of
/// another pack and names its package, every other row lists its subjects, one per finding.
#[test]
fn findings_mvp_csv_holds_doc20_section_18_1() {
    let rows = expected_rows();
    let total = |sev: &str| -> usize { rows.iter().filter(|r| r.severity == sev).map(|r| r.count).sum() };
    assert_eq!(
        (total("error"), total("warning"), total("info"), total("waived")),
        (42, 32, 31, 1)
    );
    let efs: Vec<&str> = rows.iter().map(|r| r.ef.as_str()).collect();
    let want: Vec<String> = (1..=19).map(|n| format!("EF-{n:02}")).collect();
    assert_eq!(efs, want);
    for r in &rows {
        if r.pending.is_empty() {
            assert!(FOUR_PACKS.contains(&pack_of(&r.rule).as_str()), "{r:?}");
            assert_eq!(r.subjects.len(), r.count, "{r:?}");
        } else {
            assert!(!FOUR_PACKS.contains(&pack_of(&r.rule).as_str()), "{r:?}");
            assert!(r.pending.starts_with("WP-"), "{r:?}");
            assert!(
                checks::rule(&r.rule).is_none(),
                "{} is built: compare its row (FT-589)",
                r.rule
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// The W-1 fixture

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a directory");
    for e in std::fs::read_dir(from).expect("a readable directory") {
        let e = e.expect("an entry");
        let target = to.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            std::fs::copy(e.path(), &target).expect("a copied file");
        }
    }
}

fn open(dir: &Path, state: &Path) -> Session {
    let mut options = StoreOptions::new("wp-s1-16-part5");
    options.state_dir = Some(state.to_path_buf());
    let (session, _) = Session::open(dir, options, SessionOptions::default(), Recovery::Keep)
        .unwrap_or_else(|e| panic!("{}: {e:?}", dir.display()));
    session
}

/// The open EW-CORE-041 finding on -XPE (EF-19's finding before W-1).
fn xpe(report: &CheckReport) -> Finding {
    report
        .findings
        .iter()
        .find(|f| f.rule == "EW-CORE-041" && f.subject == "=P01+J-E-2416-XPE")
        .cloned()
        .unwrap_or_else(|| panic!("EW-CORE-041 on -XPE: {:#?}", report.findings))
}

fn waive(fingerprint: &str) -> AnyCommand {
    AnyCommand {
        name: "check.waive".into(),
        params: serde_json::json!({
            "rule": "EW-CORE-041", "fingerprint": fingerprint,
            "reason": {"en-GB": W1_REASON}, "author": "engineer", "date": "2025-04-14"
        }),
    }
}

/// A copy of Golden A with W-1 recorded and saved, and the directory it lives in (removed by the caller).
fn golden_a_with_w1(tag: &str) -> (PathBuf, PathBuf) {
    let copy = ew_testdir::fresh_dir(&format!("ew-part5-{tag}"));
    let state = ew_testdir::fresh_dir(&format!("ew-part5-{tag}-state"));
    copy_dir(&golden_common::repo().join("golden/J-E-2416.ew"), &copy);
    let mut s = open(&copy, &state);
    let fp = xpe(&s.check(&PRE_DELIVERY, CheckMode::Release))
        .fingerprint
        .to_string();
    s.execute_with(&waive(&fp), Submit::from(Origin::Gui))
        .expect("W-1 commits from the GUI");
    s.save().expect("the copy saves");
    s.close();
    assert!(
        copy.join("waivers.jsonl").is_file(),
        "the store wrote waivers.jsonl"
    );
    (copy, state)
}

/// P5-3: on a copy of Golden A, `check.waive` records W-1 with its reason; after a save and a reopen the EW-CORE-041
/// finding on -XPE is waived by it (EF-19): one waived, one warning fewer, nothing else changed; a script cannot
/// waive (`HumanOnly`); `check.unwaive` brings the warning back.
#[test]
fn w1_waives_ef19_through_the_file_and_only_a_human_can_waive() {
    let batch = golden_common::batch();
    let plain = golden_common::session(&batch).check(&PRE_DELIVERY, CheckMode::Release);
    let (copy, state) = golden_a_with_w1("w1");
    let text = std::fs::read_to_string(copy.join("waivers.jsonl")).expect("waivers.jsonl");
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(
        text.contains(W1_REASON) && text.contains("\"rule\":\"EW-CORE-041\""),
        "{text}"
    );

    let mut s = open(&copy, &state);
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    let w = xpe(&report);
    let FindingState::Waived { waiver } = &w.state else {
        panic!("-XPE is not waived: {w:#?}");
    };
    assert_eq!(report.counts.waived, 1);
    assert_eq!(report.counts.warnings + 1, plain.counts.warnings);
    assert_eq!(
        (report.counts.errors, report.counts.infos),
        (plain.counts.errors, plain.counts.infos)
    );
    assert!(
        !report.findings.iter().any(|f| f.rule == "EW-CORE-081"),
        "W-1 matches its finding"
    );
    assert!(
        checks::report::to_text(&report).contains(&format!("[waived: {waiver}]")),
        "the report lists the waived instance"
    );

    // A script may not waive or unwaive.
    let err = s
        .execute_with(
            &waive(&"0".repeat(32)),
            Submit::from(Origin::CmdLog {
                file: "replay.ewcmd".into(),
            }),
        )
        .unwrap_err();
    assert!(matches!(err, CmdError::Exposure { .. }), "{err:?}");
    let unwaive = AnyCommand {
        name: "check.unwaive".into(),
        params: serde_json::json!({ "waiver": waiver }),
    };
    let err = s
        .execute_with(
            &unwaive,
            Submit::from(Origin::Mcp {
                client: "agent".into(),
            }),
        )
        .unwrap_err();
    assert!(matches!(err, CmdError::Exposure { .. }), "{err:?}");
    s.execute_with(&unwaive, Submit::from(Origin::Gui))
        .expect("the GUI unwaives");
    let after = s.check(&PRE_DELIVERY, CheckMode::Release);
    assert!(xpe(&after).is_open());
    assert_eq!(after.counts, plain.counts);
    s.close();
    let _ = std::fs::remove_dir_all(&copy);
    let _ = std::fs::remove_dir_all(&state);
}

/// P5-4 / D10 (AC-21, built-pack subset): `ew check --scheme pre-delivery` on the W-1 copy of Golden A reports, for the
/// packs EW-CORE, EW-IEC81346, EW-TERM, EW-POT (EW-CORE-083 and -084 included) and EW-PLC (WP-S1.21 part 3, PA-8), exactly the rows of
/// `findings_mvp.csv` that are not pending, plus the known excess of FT-638; the pending rows name their packages.
#[test]
fn ac21_four_pack_subset_equals_findings_mvp_csv() {
    let (copy, state) = golden_a_with_w1("ac21");
    let s = open(&copy, &state);
    let report = s.check(&PRE_DELIVERY, CheckMode::Release);
    s.close();
    let _ = std::fs::remove_dir_all(&copy);
    let _ = std::fs::remove_dir_all(&state);

    // What the report holds of the four packs, as (rule, severity or "waived") → subjects.
    let mut got: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for f in &report.findings {
        if !FOUR_PACKS.contains(&pack_of(f.rule).as_str()) {
            continue;
        }
        if KNOWN_EXCESS
            .iter()
            .any(|(r, subject, _)| *r == f.rule && *subject == f.subject && f.is_open())
        {
            continue;
        }
        let state = match f.state {
            FindingState::Waived { .. } => "waived".to_owned(),
            FindingState::Open => f.severity.name().to_owned(),
        };
        got.entry((f.rule.to_owned(), state))
            .or_default()
            .push(f.subject.clone());
    }
    for v in got.values_mut() {
        v.sort();
    }
    let mut want: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut pending = Vec::new();
    for r in expected_rows() {
        if r.pending.is_empty() {
            let mut subjects = r.subjects.clone();
            subjects.sort();
            want.insert((r.rule.clone(), r.severity.clone()), subjects);
        } else {
            pending.push(format!("{} {} ({})", r.ef, r.rule, r.pending));
        }
    }
    assert_eq!(got, want, "the four-pack subset of AC-21");
    // The known excess is still there (FT-638 re-counts it when FT-209 lands).
    for (rule, subject, ft) in KNOWN_EXCESS {
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.rule == rule && f.subject == subject && f.is_open()),
            "{rule} on {subject} is gone: update KNOWN_EXCESS and {ft}"
        );
    }
    assert_eq!(
        pending,
        [
            "EF-03 EW-BUS-001 (WP-S1.21)",
            "EF-05 EW-LAY-001 (WP-S4.01)",
            "EF-13 EW-FL-001 (WP-S1.20)",
            "EF-14 EW-FL-004 (WP-S1.20)",
            "EF-16 EW-BUS-004 (WP-S1.21)",
        ]
    );
    // The rule system is quiet (EW-CORE-083, -084 run and report nothing).
    for system in ["EW-CORE-083", "EW-CORE-084"] {
        assert!(report.rules.iter().any(|r| r.id == system), "{system} ran");
    }
    assert_eq!(report.exit_code(), 1, "doc 20 §18.1: `ew check` exits 1");
}

// ---------------------------------------------------------------------------------------------------------
// Messages

/// P5-2: every Golden A finding of the pre-delivery check (release) and of the drafting check (online) keeps the
/// message it printed before the change: the en-GB pattern renders it byte for byte from the finding's arguments, and
/// every rule that reports on Golden A in pre-delivery carries arguments. In nl-NL the findings of those rules render
/// in Dutch, the others in en-GB.
#[test]
fn golden_a_messages_render_byte_equal_in_en_gb_and_in_dutch_where_present() {
    let batch = golden_common::batch();
    let s = golden_common::session(&batch);
    let release = s.check(&PRE_DELIVERY, CheckMode::Release);
    let online = s.check(&checks::DRAFTING, CheckMode::Online);
    for f in release.findings.iter().chain(&online.findings) {
        if f.args.is_empty() {
            continue;
        }
        assert_eq!(
            checks::messages::format(f, "en-GB").as_deref(),
            Some(f.message.as_str()),
            "{} on {}: {:?}",
            f.rule,
            f.subject,
            f.args
        );
        assert_eq!(checks::render(f, "en-GB"), f.message);
    }
    let without: Vec<String> = release
        .findings
        .iter()
        .filter(|f| f.args.is_empty())
        .map(|f| format!("{} {}", f.rule, f.subject))
        .collect();
    assert!(
        without.is_empty(),
        "pre-delivery findings without arguments: {without:#?}"
    );
    for f in &release.findings {
        let nl = checks::render(f, "nl-NL");
        assert_ne!(nl, f.message, "{} on {} has no Dutch text", f.rule, f.subject);
    }
    let t55 = release
        .findings
        .iter()
        .find(|f| f.rule == "EW-POT-013" && f.subject == "T55")
        .expect("EF-01");
    assert_eq!(
        checks::render(t55, "nl-NL"),
        "T55 schakelt de nulleider N-55 → N-55T; belasting E55 blijft verbonden met L-55."
    );
}
