// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules (cable length, interruption-point pairing), the registered catalogue metadata and the
//! determinism of a check run.

mod common;

use common::*;
use ew_checks::{CheckMode, FULL, RuleKind, ScopeKind, Severity, run};
use ew_derive::Q;
use ew_derive::ew_schema::Kind;
use serde_json::{Value, json};

fn cable(length: Option<&str>) -> Fx {
    let mut fx = Fx::new();
    let mut c = json!({"$k": "device", "id": us(0x6000), "tag": {"letter": "W", "counter": 1}, "cable": {}});
    if let Some(l) = length {
        c["cable"]["length"] = json!(l);
    }
    fx.push(c);
    fx
}

#[test]
fn core033_reports_a_cable_without_length() {
    let f = findings_of(&cable(None).build(), "EW-CORE-033");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "003033");
    assert_eq!(f[0].severity, Severity::Error);
    assert_eq!(f[0].discriminator, "");
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(
        (f[0].targets[0].kind, f[0].targets[0].id),
        (Kind::Device, uid(0x6000))
    );
    assert!(
        f[0].message.ends_with("has no routed, manual or part length"),
        "{}",
        f[0].message
    );
    assert!(f[0].subject.contains("W1"), "{}", f[0].subject);
}

#[test]
fn core033_a_cable_with_a_part_length_has_no_finding() {
    assert!(findings_of(&cable(Some("from_part")).build(), "EW-CORE-033").is_empty());
}

#[test]
fn core033_a_model_without_cables_has_no_finding() {
    let mut fx = Fx::new();
    fx.device(0x101, "K", Some(1));
    assert!(findings_of(&fx.build(), "EW-CORE-033").is_empty());
}

/// A function wired to an interruption point `name` with no partner.
fn lonely_ip() -> Fx {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.ip(0x1101, P1, "LONELY", "source");
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1101), &Value::Null);
    fx
}

#[test]
fn core051_reports_an_interruption_point_without_a_partner() {
    let f = findings_of(&lonely_ip().build(), "EW-CORE-051");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "103051");
    assert_eq!(f[0].severity, Severity::Error);
    // The note names the point as "(<function id>)": that function is the target.
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(
        (f[0].targets[0].kind, f[0].targets[0].id),
        (Kind::Function, uid(0x1101))
    );
    assert!(f[0].discriminator.contains(&us(0x1101)), "{}", f[0].discriminator);
    assert_eq!(f[0].discriminator, f[0].subject);
    assert!(f[0].message.contains("no partner"), "{}", f[0].message);
}

#[test]
fn core051_reports_an_interruption_point_without_a_connection_point() {
    let mut fx = Fx::base();
    fx.ip_without_cp(0x1101, P1, "NOCP", "source");
    let f = findings_of(&fx.build(), "EW-CORE-051");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(f[0].targets[0].id, uid(0x1101));
    assert!(f[0].message.contains("no connection point"), "{}", f[0].message);
}

#[test]
fn core051_a_pair_has_no_finding() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P2);
    fx.ip(0x1101, P1, "X", "source");
    fx.ip(0x1102, P2, "X", "target");
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1101), &Value::Null);
    fx.conn(0x2002, P2, cp(0x1102), cp(0x1002), &Value::Null);
    assert!(findings_of(&fx.build(), "EW-CORE-051").is_empty());
}

#[test]
fn core051_a_group_note_without_an_id_has_no_target() {
    // Two sources and no star flag: the group note names the point's name only.
    let mut fx = Fx::base();
    for (f, page) in [(0x1001, P1), (0x1002, P2), (0x1003, P2)] {
        fx.func(f, page);
    }
    fx.ip(0x1101, P1, "X", "source");
    fx.ip(0x1102, P2, "X", "source");
    fx.ip(0x1103, P2, "X", "target");
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1101), &Value::Null);
    fx.conn(0x2002, P2, cp(0x1002), cp(0x1102), &Value::Null);
    fx.conn(0x2003, P2, cp(0x1003), cp(0x1103), &Value::Null);
    let f = findings_of(&fx.build(), "EW-CORE-051");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(f[0].targets.is_empty());
    assert!(f[0].message.contains("source points"), "{}", f[0].message);
}

/// The catalogue columns of every shipped rule of this work package: id, code, title, kind, modes
/// (online, prevent, release), severity, scope.
#[test]
fn shipped_rules_carry_their_catalogue_metadata() {
    use RuleKind::{Completeness, Conflict, Hygiene};
    use ScopeKind::{Cable, Potential, Project, Strip};
    use Severity::{Error, Warning};
    #[rustfmt::skip]
    let table = [
        ("EW-CORE-033", "003033", "Cable without length", Completeness, (true, false, true), Error, Cable),
        ("EW-CORE-051", "103051", "Unpaired interruption point", Completeness, (true, false, true), Error, Project),
        ("EW-POT-001", "005001", "Type or voltage conflict", Conflict, (true, false, true), Error, Potential),
        ("EW-POT-008", "005008", "PE island", Conflict, (true, false, true), Warning, Project),
        ("EW-POT-011", "005011", "Junction feed ambiguous", Conflict, (true, false, false), Warning, Potential),
        ("EW-POT-012", "005012", "Ring in the drawing graph", Hygiene, (true, false, false), Warning, Potential),
        ("EW-POT-014", "005014", "Physical-wire conflict", Conflict, (true, false, true), Error, Potential),
        ("EW-RDS-004", "017004", "Duplicate designation", Conflict, (true, true, true), Error, Project),
        ("EW-TERM-001", "001001", "Terminal without designation", Conflict, (true, true, true), Error, Strip),
        ("EW-TERM-005", "001005", "Rail bonding on a non-PE terminal", Conflict, (true, false, false), Error, Strip),
    ];
    for (id, code, title, kind, (online, prevent, release), severity, scope) in table {
        let m = ew_checks::rule(id)
            .unwrap_or_else(|| panic!("{id} is not registered"))
            .meta();
        assert_eq!(m.code, code, "{id}");
        assert_eq!(m.title, title, "{id}");
        assert_eq!(m.kind, kind, "{id}");
        assert_eq!(
            (m.modes.online, m.modes.prevent, m.modes.release),
            (online, prevent, release),
            "{id}"
        );
        assert_eq!(m.severity, severity, "{id}");
        assert_eq!(m.release_severity, None, "{id}");
        assert_eq!(m.scope, scope, "{id}");
    }
}

#[test]
fn clean_model_has_no_finding_from_any_shipped_rule() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &Value::Null);
    let m = fx.build();
    let report = run(&Q::new(&m), &FULL, CheckMode::Online);
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    assert!(report.rules.len() >= 12, "every online rule ran");
}

#[test]
fn a_run_is_deterministic() {
    // A model with findings from several rules, checked twice over two contexts.
    let mut fx = lonely_ip();
    fx.push(json!({"$k": "device", "id": us(0x6000), "tag": {"letter": "W", "counter": 1}, "cable": {}}));
    fx.drawn_device(0x301, "Q", Some(1), P1);
    fx.drawn_device(0x302, "Q", Some(1), P1);
    fx.push(json!({"$k": "device", "id": us(0x400), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    fx.rail_terminal(0x4001, 0x400, None, None);
    let m = fx.build();
    let first = run(&Q::new(&m), &FULL, CheckMode::Online);
    let second = run(&Q::new(&m), &FULL, CheckMode::Online);
    assert_eq!(first, second);
    let rules: std::collections::BTreeSet<&str> = first.findings.iter().map(|f| f.rule).collect();
    for expected in [
        "EW-CORE-033",
        "EW-CORE-051",
        "EW-POT-008",
        "EW-RDS-004",
        "EW-TERM-005",
    ] {
        assert!(rules.contains(expected), "{expected} missing in {rules:?}");
    }
    // The release run is deterministic too.
    let release = |q: &Q| run(q, &FULL, CheckMode::Release);
    assert_eq!(release(&Q::new(&m)), release(&Q::new(&m)));
}
