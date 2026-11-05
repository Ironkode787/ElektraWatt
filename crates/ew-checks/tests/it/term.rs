// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-TERM rules: online, release and prevent behaviour on small snapshots.

use crate::common;

use common::*;
use ew_checks::{CheckMode, DRAFTING, MIGRATION, PRE_DELIVERY, Scope, prevent, run};
use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_derive::ew_schema::{AnyId, Kind};
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
fn term002_cases() {
    let duplicated = model(three_terminals(["1", "2", "2"]));
    let duplicated_run = model(three_terminals(["1", "2", "2"]));
    let clean = model(three_terminals(["1", "2", "3"]));
    let clean_run = model(three_terminals(["1", "2", "3"]));
    let clean_term001 = model(three_terminals(["1", "2", "3"]));
    rule_table(
        "EW-TERM-002",
        vec![
            // One finding per duplicated designation, with both terminals as targets.
            Case::reports("a designation duplicated in one strip", duplicated, |c, f| {
                assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                assert_eq!(f[0].targets.len(), 2, "case {c}");
                assert_eq!(f[0].code, "001002", "case {c}");
                assert!(
                    f[0].message.contains("designation 2"),
                    "case {c}: {}",
                    f[0].message
                );
            }),
            Case::silent("a clean strip", clean),
        ],
    );
    // The pre-delivery release run of the duplicated strip: the finding and the exit code.
    let report = run(&Q::new(&duplicated_run), &PRE_DELIVERY, CheckMode::Release);
    let term: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "EW-TERM-002")
        .collect();
    assert_eq!(
        term.len(),
        1,
        "case a designation duplicated in one strip: {:#?}",
        report.findings
    );
    assert_eq!(
        term[0].targets.len(),
        2,
        "case a designation duplicated in one strip"
    );
    assert_eq!(
        term[0].code, "001002",
        "case a designation duplicated in one strip"
    );
    assert!(
        term[0].message.contains("designation 2"),
        "case a designation duplicated in one strip: {}",
        term[0].message
    );
    assert_eq!(
        report.counts.errors, 1,
        "case a designation duplicated in one strip"
    );
    assert_eq!(
        report.exit_code(),
        1,
        "case a designation duplicated in one strip"
    );
    // The migration release run of a clean strip.
    let report = run(&Q::new(&clean_run), &MIGRATION, CheckMode::Release);
    assert!(
        report.findings.iter().all(|f| f.rule != "EW-TERM-002"),
        "case a clean strip (migration run)"
    );
    assert!(
        report.rules.iter().any(|r| r.id == "EW-TERM-002"),
        "case a clean strip (migration run)"
    );
    rule_table(
        "EW-TERM-001",
        vec![
            // `Ident` cannot be empty, so no snapshot can hold an undesignated terminal; the rule runs and is
            // silent, and it is registered for lenient data and the catalogue.
            Case::silent("a clean strip", clean_term001),
        ],
    );
    let meta = ew_checks::rule("EW-TERM-001").expect("registered").meta();
    assert_eq!(meta.code, "001001", "case a clean strip (EW-TERM-001 meta)");
    assert!(
        meta.modes.prevent && meta.modes.online && meta.modes.release,
        "case a clean strip (EW-TERM-001 meta)"
    );
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

/// One row of `term002_prevent_cases`: the case name, the drawing before and after, the touched keys and the
/// check of the violations.
type PreventRow = (
    &'static str,
    ew_model::Model,
    ew_model::Model,
    Vec<(ew_derive::ew_schema::Kind, ew_derive::ew_schema::AnyId)>,
    fn(&str, &[ew_checks::Finding]),
);

#[test]
fn term002_prevent_cases() {
    let cases: [PreventRow; 5] = [
        (
            "a new duplicate is refused",
            model(three_terminals(["1", "2", "3"])),
            model(three_terminals(["1", "2", "2"])),
            vec![dev_key(0x103)],
            |c, v| {
                assert_eq!(v.len(), 1, "case {c}: {v:#?}");
                assert_eq!(v[0].rule, "EW-TERM-002", "case {c}");
                assert!(v[0].message.contains("taken"), "case {c}: {}", v[0].message);
                assert!(v[0].message.contains("use 3"), "case {c}: {}", v[0].message);
                assert!(v[0].message.contains("swap 3"), "case {c}: {}", v[0].message);
                // FT-1417: the prevent finding carries its pattern's arguments: nl-NL words it, never falls back.
                assert_eq!(
                    ew_checks::messages::format(&v[0], ew_checks::messages::DEFAULT_LANGUAGE).as_deref(),
                    Some(v[0].message.as_str()),
                    "case {c}"
                );
                assert_eq!(
                    ew_checks::messages::format(&v[0], "nl-NL").as_deref(),
                    Some(
                        "aanduiding 2 is al bezet op -?X1 (2 klemmen); gebruik 3 of verwissel 3 \u{2194} 2 in één \
                         transactie (terminal.designate)"
                    ),
                    "case {c}"
                );
            },
        ),
        (
            "a swap is accepted",
            model(three_terminals(["1", "2", "3"])),
            model(three_terminals(["1", "3", "2"])),
            vec![dev_key(0x102), dev_key(0x103)],
            |c, v| assert!(v.is_empty(), "case {c}: {v:#?}"),
        ),
        // A lenient import committed "2" twice; editing the third terminal leaves the duplicate alone.
        (
            "an existing duplicate left alone",
            model(three_terminals(["1", "2", "2"])),
            model(three_terminals(["9", "2", "2"])),
            vec![dev_key(0x101)],
            |c, v| assert!(v.is_empty(), "case {c}: {v:#?}"),
        ),
        // Touching a member without changing its designation is not a new violation either.
        (
            "a member touched without a designation change",
            model(three_terminals(["1", "2", "2"])),
            model(three_terminals(["1", "2", "2"])),
            vec![dev_key(0x102)],
            |c, v| assert!(v.is_empty(), "case {c}: {v:#?}"),
        ),
        (
            "a new terminal with a taken designation is refused",
            model(three_terminals(["1", "2", "3"])),
            {
                let mut taken_records = three_terminals(["1", "2", "3"]);
                taken_records.push(terminal(0x104, STRIP, 4, "1"));
                model(taken_records)
            },
            vec![dev_key(0x104)],
            |c, v| {
                assert_eq!(v.len(), 1, "case {c}");
                assert_eq!(v[0].targets.len(), 2, "case {c}");
            },
        ),
    ];
    for (name, before, after, touched, check) in cases {
        let v = prevent(&Q::new(&before), &Q::new(&after), &touched);
        check(name, &v);
    }
}

/// FT-1399 (ID-1392): EW-TERM-001 runs in prevent mode too (doc 15 §4.4). `Ident` refuses an empty or blank
/// designation, so no snapshot holds the violation; a batch that designates, renumbers or adds terminals passes it.
#[test]
fn term001_prevent_passes_every_designated_batch() {
    let meta = ew_checks::rule("EW-TERM-001").expect("registered").meta();
    assert!(meta.modes.prevent);
    assert_eq!(
        ew_derive::ew_mltext::Ident::from_input(" ")
            .map(|i| i.as_str().to_owned())
            .ok(),
        None,
        "a blank designation is no identifier"
    );
    let before = model(three_terminals(["1", "2", "3"]));
    let mut records = three_terminals(["3", "1", "2"]);
    records.push(terminal(0x104, STRIP, 4, "4"));
    let after = model(records);
    let touched = [dev_key(0x101), dev_key(0x102), dev_key(0x103), dev_key(0x104)];
    let v = prevent(&Q::new(&before), &Q::new(&after), &touched);
    assert!(v.iter().all(|f| f.rule != "EW-TERM-001"), "{v:#?}");
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
fn term005_cases() {
    let without_pe = rail_strip(false, true);
    let with_pe = rail_strip(true, true);
    let unflagged = rail_strip(false, false);
    rule_table(
        "EW-TERM-005",
        vec![
            Case::reports("a rail-bonded terminal without a PE CP", without_pe, |c, f| {
                assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                assert_eq!(f[0].code, "001005", "case {c}");
                assert_eq!(f[0].severity, ew_checks::Severity::Error, "case {c}");
                assert_eq!(f[0].discriminator, "", "case {c}");
                assert_eq!(f[0].targets.len(), 1, "case {c}");
                assert_eq!(f[0].targets[0].id, uid(0x4001), "case {c}");
                assert!(f[0].message.contains("no PE CP"), "case {c}: {}", f[0].message);
            }),
            Case::silent("a terminal with a PE CP", with_pe),
            Case::silent("a terminal without the rail-bonded flag", unflagged),
        ],
    );
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
    // The project-wide rules report on the strip without items too (EW-CORE-061's two text styles, EW-CORE-026's
    // strip without a terminal diagram): the items add to those counts.
    let base = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release);
    assert_eq!(base.counts.errors, 0, "{:#?}", base.findings);
    let release = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release).with_integrity(items());
    assert_eq!(release.counts.errors, 1, "{:#?}", release.findings);
    assert_eq!(release.counts.warnings, base.counts.warnings);
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

// ---------------------------------------------------------------------------------------------------------
// EW-TERM-006

const TERM_STRIP: u128 = 0x300;
const TERM_T: u128 = 0x301;
const TERM_FN: u128 = 0x320;
const TERM_K1: u128 = 0x10;
const TERM_K2: u128 = 0x40;
const TERM_WIRE: u128 = 0x900;

/// Devices -K16 (function 0x10, CPs at (0, 16 mm) and (0, 17 mm)) and -K64 (function 0x40, CPs at (0, 64 mm) and
/// (0, 65 mm)) on P1, strip -1X1 with terminal 5 whose function is drawn at `terminal_at` (CP 1 1 mm below CP 0), and
/// the wires `wires`, each `(id, a, b, route)`.
fn term006_fx(terminal_at: [i64; 2], wires: &[(u128, Ep, Ep, Vec<[i64; 2]>)]) -> Fx {
    let mut fx = Fx::base();
    fx.func(TERM_K1, P1);
    fx.func(TERM_K2, P1);
    fx.device(TERM_STRIP, "X", Some(1)).with("strip", json!({}));
    fx.drawn_terminal(TERM_T, TERM_STRIP, "5", TERM_FN, P1, terminal_at);
    for (id, a, b, route) in wires {
        let extra = if route.is_empty() {
            json!({})
        } else {
            json!({ "route": route })
        };
        fx.conn(*id, P1, *a, *b, &extra);
    }
    fx
}

/// The check of a case that reports terminal -1X1:5 once, on wire `wire` printed as `number`: the whole finding.
fn term006_reports(wire: u128, number: &'static str) -> impl Fn(&str, &[ew_checks::Finding]) + 'static {
    move |c: &str, f: &[ew_checks::Finding]| {
        assert_eq!(f.len(), 1, "case {c}: {f:#?}");
        let message =
            format!("-1X1:5 lies on wire {number} without joining it; connect it to the wire or move it");
        assert_eq!(
            (
                f[0].code,
                f[0].severity,
                f[0].subject.as_str(),
                f[0].message.as_str()
            ),
            ("001006", ew_checks::Severity::Warning, "-1X1:5", message.as_str()),
            "case {c}"
        );
        let mut targets: Vec<(Kind, AnyId)> = f[0].targets.iter().map(|t| (t.kind, t.id)).collect();
        targets.sort();
        assert_eq!(
            targets,
            [(Kind::Device, uid(TERM_T)), (Kind::Connection, uid(wire))],
            "case {c}"
        );
    }
}

/// FT-1020 (decision log ID-1104): a terminal whose connection points lie on a wire it does not join is one warning,
/// on a wire without a route (drawn straight between its ends) as on a routed one; once the wire is split at the
/// terminal, or with the terminal beside it, there is none. The message leads with the terminal (ID-1110), so the
/// Messages row, which prints the subject before a message that does not lead with it, names it once.
#[test]
fn term006_a_terminal_lying_on_a_wire_it_does_not_join() {
    let k1 = Ep::Cp(TERM_K1, 1);
    let k2 = Ep::Cp(TERM_K2, 0);
    let over = |route: Vec<[i64; 2]>| term006_fx([0, 32_000_000], &[(TERM_WIRE, k1, k2, route)]);
    let split = term006_fx(
        [0, 32_000_000],
        &[
            (TERM_WIRE, k1, Ep::Cp(TERM_FN, 0), Vec::new()),
            (0x901, Ep::Cp(TERM_FN, 1), k2, Vec::new()),
        ],
    );
    let beside = term006_fx([4_000_000, 32_000_000], &[(TERM_WIRE, k1, k2, Vec::new())]);
    rule_table(
        "EW-TERM-006",
        vec![
            Case::reports(
                "a wire without a route",
                over(Vec::new()).build(),
                term006_reports(TERM_WIRE, "1.1"),
            ),
            Case::reports(
                "a routed wire",
                over(vec![[0, 17_000_000], [0, 64_000_000]]).build(),
                term006_reports(TERM_WIRE, "1.1"),
            ),
            Case::silent("the wire split at the terminal", split.build()),
            Case::silent("the terminal beside the wire", beside.build()),
        ],
    );
    // The release run reports it too.
    let m = over(Vec::new()).build();
    let report = run(&Q::new(&m), &PRE_DELIVERY, CheckMode::Release);
    assert_eq!(
        report.findings.iter().filter(|f| f.rule == "EW-TERM-006").count(),
        1,
        "{:#?}",
        report.findings
    );
}

/// C45-p1#3 and #4 (decision log ID-1110): each branch of EW-TERM-006, with the whole finding. A horizontal wire (the
/// horizontal lines), a slanted route segment (tested one by one), an L-shaped path without a route (its corner from
/// the leaving direction: a wrong corner misses the terminal), a cable core (no wire number: "?"); silent when the wire
/// ends at another function of the terminal's device, or at a junction standing on the terminal's CP that a connection
/// joins to the terminal; a junction on the CP that is not joined to it, or one joined to it beside its CPs, does not
/// join the wire.
#[test]
fn term006_every_branch() {
    const MM: i64 = 1_000_000;
    const H1: u128 = 0x50;
    const H2: u128 = 0x60;
    const L: u128 = 0x70;
    const FN2: u128 = 0x321;
    const W2: u128 = 0x901;
    const W3: u128 = 0x902;
    const J: u128 = 0x950;
    let k1 = Ep::Cp(TERM_K1, 1);
    let k2 = Ep::Cp(TERM_K2, 0);
    let on_cp = [0, 32 * MM];
    let junction = |fx: &mut Fx, at: [i64; 2], sequence: &[u128]| {
        let sequence: Vec<String> = sequence.iter().map(|c| us(*c)).collect();
        fx.push(json!({
            "$k": "junction", "id": us(J), "page": us(P1), "at": at, "style": "dot", "sequence": sequence,
        }));
    };

    let mut horizontal = term006_fx(
        [20 * MM, 80 * MM],
        &[(TERM_WIRE, Ep::Cp(H1, 0), Ep::Cp(H2, 0), Vec::new())],
    );
    horizontal.func_at(H1, P1, [0, 80 * MM]);
    horizontal.func_at(H2, P1, [40 * MM, 80 * MM]);
    let slanted = term006_fx(
        on_cp,
        &[(TERM_WIRE, k1, k2, vec![[-8 * MM, 24 * MM], [8 * MM, 40 * MM]])],
    );
    let mut l_shaped = term006_fx([20 * MM, 48 * MM], &[(TERM_WIRE, k1, Ep::Cp(L, 0), Vec::new())]);
    l_shaped.func_at(L, P1, [40 * MM, 48 * MM]);
    let mut core = term006_fx(on_cp, &[]);
    core.push(json!({"$k": "device", "id": us(0x6000), "tag": {"letter": "W", "counter": 1}, "cable": {}}));
    core.push(json!({"$k": "conductor", "id": us(0x6001), "cable": us(0x6000), "core": "1"}));
    core.conn(TERM_WIRE, P1, k1, k2, &json!({"kind": {"core": us(0x6001)}}));
    let mut other_function = term006_fx(on_cp, &[(TERM_WIRE, k1, Ep::Cp(FN2, 0), Vec::new())]);
    other_function.drawn_function(FN2, TERM_T, P1, [0, 96 * MM]);
    for r in &mut other_function.records {
        if r["$k"] == "function" && r["id"] == json!(us(FN2)) {
            r["role"] = json!("aux");
        }
    }
    let mut joined_junction = term006_fx(
        on_cp,
        &[
            (TERM_WIRE, k1, Ep::J(J), Vec::new()),
            (W2, Ep::J(J), Ep::Cp(TERM_FN, 0), Vec::new()),
            (W3, Ep::J(J), k2, Vec::new()),
        ],
    );
    junction(&mut joined_junction, on_cp, &[TERM_WIRE, W2, W3]);
    let mut lone_junction = term006_fx(on_cp, &[(TERM_WIRE, k1, Ep::J(J), Vec::new())]);
    junction(&mut lone_junction, on_cp, &[TERM_WIRE]);
    let mut junction_beside = term006_fx(
        on_cp,
        &[
            (TERM_WIRE, k1, Ep::J(J), Vec::new()),
            (W2, Ep::J(J), Ep::Cp(TERM_FN, 0), Vec::new()),
        ],
    );
    junction(&mut junction_beside, [4 * MM, 32 * MM], &[TERM_WIRE, W2]);

    rule_table(
        "EW-TERM-006",
        vec![
            Case::reports(
                "a horizontal wire",
                horizontal.build(),
                term006_reports(TERM_WIRE, "1.1"),
            ),
            Case::reports(
                "a slanted route segment",
                slanted.build(),
                term006_reports(TERM_WIRE, "1.1"),
            ),
            Case::reports(
                "an L-shaped path without a route",
                l_shaped.build(),
                term006_reports(TERM_WIRE, "1.1"),
            ),
            Case::reports(
                "a cable core, without a wire number",
                core.build(),
                term006_reports(TERM_WIRE, "?"),
            ),
            Case::silent(
                "a wire ending at another function of the terminal's device",
                other_function.build(),
            ),
            Case::silent(
                "a junction on the terminal's CP, joined to the terminal",
                joined_junction.build(),
            ),
            // The wire ends at a junction no other connection meets; the wire numbering gives it no number.
            Case::reports(
                "a junction on the terminal's CP, not joined to it",
                lone_junction.build(),
                term006_reports(TERM_WIRE, "?"),
            ),
            Case::reports(
                "a junction joined to the terminal beside its CPs",
                junction_beside.build(),
                term006_reports(TERM_WIRE, "1.1"),
            ),
        ],
    );
}
