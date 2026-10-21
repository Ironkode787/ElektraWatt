// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules (cable length, interruption-point pairing), the registered catalogue metadata and the
//! determinism of a check run.

mod common;

use common::*;
use ew_checks::{CheckMode, FULL, Release, RuleKind, ScopeKind, Severity, run};
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
    // The point's function is the target; the discriminator holds no name (doc 15 §2.5).
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!(
        (f[0].targets[0].kind, f[0].targets[0].id),
        (Kind::Function, uid(0x1101))
    );
    assert_eq!(f[0].discriminator, "unpaired");
    assert!(f[0].subject.contains("LONELY"), "{}", f[0].subject);
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
    assert_eq!(f[0].discriminator, "no-cp");
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
fn core052_two_sources_without_a_star_flag_are_ambiguous_not_unpaired() {
    // Two sources and no star flag: the derivation pairs from the smallest ID; EW-CORE-052 asks for the
    // decision, and no point is reported as unpaired (EW-CORE-051).
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
    let m = fx.build();
    assert!(findings_of(&m, "EW-CORE-051").is_empty());
    let f = findings_of(&m, "EW-CORE-052");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "103052");
    assert_eq!(f[0].targets.len(), 3);
    assert_eq!(f[0].discriminator, "star-source");
    assert!(f[0].message.contains("source points"), "{}", f[0].message);
}

#[test]
fn core052_one_source_is_not_ambiguous() {
    let mut fx = Fx::base();
    for (f, page) in [(0x1001, P1), (0x1002, P2), (0x1003, P2)] {
        fx.func(f, page);
    }
    fx.ip(0x1101, P1, "X", "source");
    fx.ip(0x1102, P2, "X", "target");
    fx.ip(0x1103, P2, "X", "target");
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1101), &Value::Null);
    fx.conn(0x2002, P2, cp(0x1002), cp(0x1102), &Value::Null);
    fx.conn(0x2003, P2, cp(0x1003), cp(0x1103), &Value::Null);
    let m = fx.build();
    assert!(findings_of(&m, "EW-CORE-052").is_empty());
    assert!(findings_of(&m, "EW-CORE-051").is_empty());
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
        ("EW-CORE-052", "103052", "Ambiguous interruption point", Conflict, (true, false, true), Error, Project),
        ("EW-POT-001", "005001", "Type or voltage conflict", Conflict, (true, false, true), Error, Potential),
        ("EW-POT-008", "005008", "PE island", Conflict, (true, false, true), Warning, Project),
        ("EW-POT-011", "005011", "Junction feed ambiguous", Conflict, (true, false, false), Warning, Potential),
        ("EW-POT-012", "005012", "Ring in the drawing graph", Hygiene, (true, false, false), Warning, Potential),
        ("EW-POT-014", "005014", "Physical-wire conflict", Conflict, (true, false, true), Error, Potential),
        ("EW-RDS-004", "017004", "Duplicate designation", Conflict, (true, false, true), Error, Project),
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

// EW-CORE-023 title-block override.

fn page_with_override(over: Option<Value>) -> ew_model::Model {
    let mut fx = Fx::base();
    if let Some(o) = over {
        let page = fx
            .records
            .iter_mut()
            .find(|r| r["id"] == json!(us(P1)))
            .expect("page 1");
        page["description_override"] = o;
    }
    fx.build()
}

#[test]
fn core023_reports_a_page_with_a_description_override() {
    let m = page_with_override(Some(json!({"en": "Override"})));
    let f = findings_of(&m, "EW-CORE-023");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].rule, "EW-CORE-023");
    assert_eq!(f[0].code, "101023");
    assert_eq!(f[0].severity, Severity::Info);
    assert_eq!(f[0].discriminator, "description_override");
    assert_eq!(f[0].targets.len(), 1);
    assert_eq!((f[0].targets[0].kind, f[0].targets[0].id), (Kind::Page, uid(P1)));
    assert!(f[0].message.contains("folio 1"), "{}", f[0].message);
    assert!(f[0].message.contains("Override"), "{}", f[0].message);
    let fix = f[0].fix.as_ref().expect("a quick fix");
    assert_eq!(fix.command, "fields.set");
    assert_eq!(fix.params["path"], json!("description_override"));
    assert_eq!(fix.params["value"], Value::Null);
}

#[test]
fn core023_a_page_without_override_has_no_finding() {
    assert!(findings_of(&page_with_override(None), "EW-CORE-023").is_empty());
}

// EW-CORE-053 cross-reference cell invalid.

const SYM_X: u128 = 0x510;

/// A device with a main function on page 1 and an auxiliary one on page 2 whose symbol draws a line 30 to
/// 50 mm right of its origin, under `anchor`; `graphics` switches the line off.
fn main_and_aux(anchor: &str, graphics: bool) -> ew_model::Model {
    main_and_aux_at(anchor, graphics, 26_000_000, None)
}

/// [`main_and_aux`] with the auxiliary representation at `aux_x` and, when given, folio number `folio2` for
/// page 2.
fn main_and_aux_at(anchor: &str, graphics: bool, aux_x: i64, folio2: Option<u32>) -> ew_model::Model {
    let mut fx = Fx::base();
    if let Some(n) = folio2 {
        let page = fx
            .records
            .iter_mut()
            .find(|r| r["id"] == json!(us(P2)))
            .expect("page 2");
        page["folio"] = json!({"number": n});
    }
    let sym = fx
        .lib
        .iter_mut()
        .find(|d| d["id"] == json!(us(SYM_X)))
        .expect("the symbol");
    sym["anchor"] = json!(anchor);
    if graphics {
        sym["graphics"] = json!([{"line": {"from": [30_000_000, 0], "to": [50_000_000, 0]}}]);
    }
    // The main function's own symbol is judged clean (graphics centre), so only the auxiliary one is in
    // question.
    let mut main_sym = fx
        .lib
        .iter()
        .find(|d| d["id"] == json!(us(SYM_X)))
        .cloned()
        .expect("the symbol");
    main_sym["id"] = json!(us(0x520));
    main_sym["item_id"] = json!(us(0x1_0520));
    main_sym["sha256"] = json!(format!("sha256:{}", "cd".repeat(32)));
    main_sym["anchor"] = json!("graphic_centre");
    main_sym["graphics"] = json!([{"line": {"from": [30_000_000, 0], "to": [50_000_000, 0]}}]);
    fx.lib.push(main_sym);
    fx.device(0x2000, "K", Some(1));
    for (n, page, role, x, symbol) in [
        (0x1001u128, P1, None, 25_000_000i64, 0x520),
        (0x1002, P2, Some("aux"), aux_x, SYM_X),
    ] {
        let mut f = json!({"$k": "function", "id": us(n), "device": us(0x2000), "def": us(FNDEF)});
        if let Some(r) = role {
            f["role"] = json!(r);
        }
        fx.push(f);
        fx.push(json!({
            "$k": "representation", "id": us(n + 0x10_000), "functions": [us(n)], "page": us(page),
            "symbol": us(symbol), "pos": [x, 40_000_000],
        }));
    }
    fx.build()
}

#[test]
fn core053_an_anchor_at_the_graphics_centre_has_no_finding() {
    let m = main_and_aux("graphic_centre", true);
    assert!(findings_of(&m, "EW-CORE-053").is_empty());
}

#[test]
fn core053_an_origin_anchor_outside_the_graphics_is_an_empty_cell() {
    let m = main_and_aux("origin", true);
    let f = findings_of(&m, "EW-CORE-053");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "103053");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(f[0].discriminator, "");
    assert_eq!(f[0].page.map(|p| p.to_string()), Some(us(P1)));
    assert_eq!(f[0].targets.len(), 2);
    assert_eq!(f[0].targets[0].kind, Kind::Representation);
    assert!(f[0].message.contains(": empty cell"), "{}", f[0].message);
    assert!(f[0].message.starts_with(&f[0].subject), "{}", f[0].message);
    let fix = f[0].fix.as_ref().expect("a quick fix");
    assert_eq!(fix.command, "rep.set_anchor_policy");
    assert_eq!(fix.params["anchor"], json!("graphic_centre"));
}

#[test]
fn core053_an_anchor_outside_the_frame_is_reported_with_the_same_fingerprint() {
    let inside = findings_of(&main_and_aux("origin", true), "EW-CORE-053");
    // 400 mm to the right lies beyond the A4 drawing area: the anchor has no cell.
    let outside = findings_of(&main_and_aux_at("origin", true, 400_000_000, None), "EW-CORE-053");
    assert_eq!((inside.len(), outside.len()), (1, 1), "{inside:#?} {outside:#?}");
    assert!(
        outside[0].message.contains(": outside the frame"),
        "{}",
        outside[0].message
    );
    assert_eq!(outside[0].discriminator, "");
    assert_eq!(
        outside[0].fingerprint, inside[0].fingerprint,
        "moving the target keeps the fingerprint"
    );
}

#[test]
fn core053_the_fingerprint_survives_a_renumber_and_a_move() {
    let base = findings_of(&main_and_aux("origin", true), "EW-CORE-053");
    let renumbered = findings_of(
        &main_and_aux_at("origin", true, 26_000_000, Some(7)),
        "EW-CORE-053",
    );
    let moved = findings_of(&main_and_aux_at("origin", true, 14_000_000, None), "EW-CORE-053");
    assert_eq!((base.len(), renumbered.len(), moved.len()), (1, 1, 1));
    assert_ne!(
        renumbered[0].subject, base[0].subject,
        "the printed item follows the folio"
    );
    assert_eq!(renumbered[0].fingerprint, base[0].fingerprint);
    assert_eq!(moved[0].fingerprint, base[0].fingerprint);
}

#[test]
fn core053_a_symbol_without_graphics_is_not_judged() {
    let m = main_and_aux("origin", false);
    assert!(findings_of(&m, "EW-CORE-053").is_empty());
}

// EW-CORE-065 description lint.

fn lint(text: &str) -> (Vec<String>, Vec<String>) {
    let l = ew_checks::rules::core::lint_description(text);
    (l.ratings, l.formats)
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn core065_lint_description_table() {
    let cases: &[(&str, &[&str], &[&str])] = &[
        ("Miniature circuit breaker 1P+N, C, 10 A", &["10 A"], &[]),
        ("10A C", &["10A"], &["10A"]),
        ("50hz", &["50hz"], &["50hz"]),
        ("Power supply 1 AC / 24 V DC / 20 A", &["24 V", "20 A"], &[]),
        ("Power supply 24 VDC", &["24 VDC"], &[]),
        ("Heater 230 VAC", &["230 VAC"], &[]),
        ("Lamp 24vdc", &["24vdc"], &["24vdc"]),
        ("VMPA1-FB-EMG-8 electronic module, 4 positions", &[], &[]),
        ("Wiring duct 40×60", &[], &[]),
        ("PROFIBUS FC standard cable", &[], &[]),
        ("Connecting cable 7/8\", 5-pin, open end", &[], &[]),
    ];
    for (text, ratings, formats) in cases {
        assert_eq!(lint(text), (strs(ratings), strs(formats)), "{text}");
    }
    let (ratings, formats) = lint("cable 0.75 mm ²");
    assert!(ratings.is_empty());
    assert!(formats.contains(&"0.75 mm ²".to_owned()), "{formats:?}");
}

fn part_model(description: &str) -> ew_model::Model {
    let mut fx = Fx::base();
    fx.lib.push(json!({
        "$k": "part", "id": us(0x8000), "library_id": us(0x7000), "item_id": us(0x1_8000),
        "version": "1.0.0", "sha256": format!("sha256:{}", "ab".repeat(32)),
        "meta": {}, "kind": "device",
        "manufacturer": {"library": us(0x7000), "item": us(0x9000), "version_req": "^1"},
        "part_number": "PN-1", "description": description, "unit": "piece",
    }));
    fx.build()
}

fn release_findings(m: &ew_model::Model, rule: &str) -> Vec<ew_checks::Finding> {
    let q = Q::new(m);
    run(&q, &FULL.with_content(Release::V1_0), CheckMode::Release)
        .findings
        .into_iter()
        .filter(|f| f.rule == rule)
        .collect()
}

#[test]
fn core065_a_plain_description_has_no_finding() {
    assert!(release_findings(&part_model("Thermostat"), "EW-CORE-065").is_empty());
}

#[test]
fn core065_a_rating_and_a_glued_unit_are_two_warnings() {
    let f = release_findings(&part_model("Miniature circuit breaker, 10A C"), "EW-CORE-065");
    let mut found: Vec<&str> = f.iter().map(|f| f.discriminator.as_str()).collect();
    found.sort_unstable();
    assert_eq!(found, ["rating", "unit-format"], "{f:#?}");
    for finding in &f {
        assert_eq!(finding.code, "104065");
        assert_eq!(finding.severity, Severity::Warning);
        assert_eq!(finding.targets.len(), 1);
        assert_eq!(
            (finding.targets[0].kind, finding.targets[0].id),
            (Kind::Part, uid(0x8000))
        );
    }
}

#[test]
fn core065_does_not_run_online() {
    let m = part_model("Miniature circuit breaker, 10A C");
    assert!(findings_of(&m, "EW-CORE-065").is_empty());
}
