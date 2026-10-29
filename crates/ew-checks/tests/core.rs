// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! EW-CORE rules (cable length, interruption-point pairing) and the determinism of a check run. The registered
//! catalogue metadata is pinned by `catalogue::committed_catalogue_matches_the_registry`.

mod common;

use common::*;
use ew_checks::{CheckMode, FULL, Release, Severity, run};
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
fn core033_cases() {
    let mut no_cables = Fx::new();
    no_cables.device(0x101, "K", Some(1));
    rule_table(
        "EW-CORE-033",
        vec![
            Case::reports("a cable without length", cable(None).build(), |c, f| {
                assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                assert_eq!(f[0].code, "003033", "case {c}");
                assert_eq!(f[0].severity, Severity::Error, "case {c}");
                assert_eq!(f[0].discriminator, "", "case {c}");
                assert_eq!(f[0].targets.len(), 1, "case {c}");
                assert_eq!(
                    (f[0].targets[0].kind, f[0].targets[0].id),
                    (Kind::Device, uid(0x6000)),
                    "case {c}"
                );
                assert!(
                    f[0].message.ends_with("has no routed, manual or part length"),
                    "case {c}: {}",
                    f[0].message
                );
                assert!(f[0].subject.contains("W1"), "case {c}: {}", f[0].subject);
            }),
            Case::silent("a cable with a part length", cable(Some("from_part")).build()),
            Case::silent("a model without cables", no_cables.build()),
        ],
    );
}

/// EW-CORE-047: a device whose only function has no representation is a warning (an error in release mode); a
/// drawn device, a terminal and a device without functions are not reported.
#[test]
fn core047_reports_a_device_whose_functions_are_not_drawn() {
    let mut fx = Fx::base();
    fx.drawn_device(0x101, "K", Some(1), P1);
    fx.device(0x102, "G", Some(1));
    fx.push(json!({"$k": "function", "id": us(0x1102), "device": us(0x102), "def": us(FNDEF)}));
    fx.device(0x103, "A", Some(1));
    fx.push(json!({"$k": "device", "id": us(0x104), "tag": {"letter": "X", "counter": 1}, "strip": {}}));
    fx.push(json!({
        "$k": "device", "id": us(0x105), "tag": {"letter": "X"}, "kind": "terminal",
        "terminal": {"strip": us(0x104), "seq": 1, "designation": "1"}
    }));
    fx.push(json!({"$k": "function", "id": us(0x1105), "device": us(0x105), "def": us(FNDEF)}));
    fx.push(json!({"$k": "function", "id": us(0x1104), "device": us(0x104), "def": us(FNDEF)}));
    let m = fx.build();
    let f = findings_of(&m, "EW-CORE-047");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].code, "102047");
    assert_eq!(f[0].severity, Severity::Warning);
    assert_eq!(
        (f[0].targets[0].kind, f[0].targets[0].id),
        (Kind::Device, uid(0x102))
    );
    assert!(
        f[0].message.ends_with("has no representation on any page"),
        "{}",
        f[0].message
    );
    let q = Q::new(&m);
    let release = run(&q, &FULL, CheckMode::Release);
    let r: Vec<_> = release
        .findings
        .iter()
        .filter(|f| f.rule == "EW-CORE-047")
        .collect();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].severity, Severity::Error, "release blocks");
}

/// Cycle 33 review finding 8: EW-CORE-047 asks for a schematic representation; a device drawn only on a panel
/// front is reported, one drawn in a multi-line schematic is not.
#[test]
fn core047_a_device_drawn_only_on_a_panel_front_is_reported() {
    let mut fx = Fx::base();
    fx.drawn_device(0x101, "K", Some(1), P1);
    fx.drawn_device(0x102, "H", Some(1), P1);
    fx.with("rep_type", json!("panel_front"));
    let f = findings_of(&fx.build(), "EW-CORE-047");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(
        (f[0].targets[0].kind, f[0].targets[0].id),
        (Kind::Device, uid(0x102))
    );
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
fn core051_cases() {
    let mut no_cp = Fx::base();
    no_cp.ip_without_cp(0x1101, P1, "NOCP", "source");
    let mut pair = Fx::base();
    pair.func(0x1001, P1);
    pair.func(0x1002, P2);
    pair.ip(0x1101, P1, "X", "source");
    pair.ip(0x1102, P2, "X", "target");
    pair.conn(0x2001, P1, cp(0x1001), cp(0x1101), &Value::Null);
    pair.conn(0x2002, P2, cp(0x1102), cp(0x1002), &Value::Null);
    rule_table(
        "EW-CORE-051",
        vec![
            Case::reports(
                "an interruption point without a partner",
                lonely_ip().build(),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "103051", "case {c}");
                    assert_eq!(f[0].severity, Severity::Error, "case {c}");
                    // The point's function is the target; the discriminator holds no name (doc 15 §2.5).
                    assert_eq!(f[0].targets.len(), 1, "case {c}");
                    assert_eq!(
                        (f[0].targets[0].kind, f[0].targets[0].id),
                        (Kind::Function, uid(0x1101)),
                        "case {c}"
                    );
                    assert_eq!(f[0].discriminator, "unpaired", "case {c}");
                    assert!(f[0].subject.contains("LONELY"), "case {c}: {}", f[0].subject);
                    assert!(f[0].message.contains("no partner"), "case {c}: {}", f[0].message);
                },
            ),
            Case::reports(
                "an interruption point without a connection point",
                no_cp.build(),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].targets.len(), 1, "case {c}");
                    assert_eq!(f[0].targets[0].id, uid(0x1101), "case {c}");
                    assert_eq!(f[0].discriminator, "no-cp", "case {c}");
                    assert!(
                        f[0].message.contains("no connection point"),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent("a pair has no finding", pair.build()),
        ],
    );
}

#[test]
fn core052_cases() {
    // Two sources and no star flag: the derivation pairs from the smallest ID; EW-CORE-052 asks for the
    // decision, and no point is reported as unpaired (EW-CORE-051).
    let mut ambiguous_fx = Fx::base();
    for (f, page) in [(0x1001, P1), (0x1002, P2), (0x1003, P2)] {
        ambiguous_fx.func(f, page);
    }
    ambiguous_fx.ip(0x1101, P1, "X", "source");
    ambiguous_fx.ip(0x1102, P2, "X", "source");
    ambiguous_fx.ip(0x1103, P2, "X", "target");
    ambiguous_fx.conn(0x2001, P1, cp(0x1001), cp(0x1101), &Value::Null);
    ambiguous_fx.conn(0x2002, P2, cp(0x1002), cp(0x1102), &Value::Null);
    ambiguous_fx.conn(0x2003, P2, cp(0x1003), cp(0x1103), &Value::Null);
    let ambiguous = ambiguous_fx.build();
    let ambiguous_unpaired = findings_of(&ambiguous, "EW-CORE-051");
    let mut single_fx = Fx::base();
    for (f, page) in [(0x1001, P1), (0x1002, P2), (0x1003, P2)] {
        single_fx.func(f, page);
    }
    single_fx.ip(0x1101, P1, "X", "source");
    single_fx.ip(0x1102, P2, "X", "target");
    single_fx.ip(0x1103, P2, "X", "target");
    single_fx.conn(0x2001, P1, cp(0x1001), cp(0x1101), &Value::Null);
    single_fx.conn(0x2002, P2, cp(0x1002), cp(0x1102), &Value::Null);
    single_fx.conn(0x2003, P2, cp(0x1003), cp(0x1103), &Value::Null);
    let single = single_fx.build();
    let single_unpaired = findings_of(&single, "EW-CORE-051");
    rule_table(
        "EW-CORE-052",
        vec![
            Case::reports(
                "two sources without a star flag are ambiguous",
                ambiguous,
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "103052", "case {c}");
                    assert_eq!(f[0].targets.len(), 3, "case {c}");
                    assert_eq!(f[0].discriminator, "star-source", "case {c}");
                    assert!(
                        f[0].message.contains("source points"),
                        "case {c}: {}",
                        f[0].message
                    );
                },
            ),
            Case::silent("one source is not ambiguous", single),
        ],
    );
    assert!(
        ambiguous_unpaired.is_empty(),
        "case two sources without a star flag: {ambiguous_unpaired:#?}"
    );
    assert!(
        single_unpaired.is_empty(),
        "case one source is not ambiguous: {single_unpaired:#?}"
    );
}

#[test]
fn clean_model_has_no_finding_from_any_shipped_rule() {
    let mut fx = Fx::base();
    fx.func(0x1001, P1);
    fx.func(0x1002, P1);
    fx.conn(0x2001, P1, cp(0x1001), cp(0x1002), &Value::Null);
    // Both connection points of each function connected (EW-POT-009).
    fx.conn(0x2002, P1, Ep::Cp(0x1001, 1), Ep::Cp(0x1002, 1), &Value::Null);
    // Both devices have a part (EW-CORE-041).
    fx.part(0x7100, "P-1", &json!({"description": "Relay"}));
    fx.with_parts(0x21001, &[0x7100]).with_parts(0x21002, &[0x7100]);
    let m = fx.build();
    let report = run(&Q::new(&m), &FULL, CheckMode::Online);
    // EW-CORE-061 reports the project's text styles below the minimum cap height, and the built-in table has two
    // (`cp.small`, `marker`) in every project until project text styles exist (WP-S1.12).
    let findings: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule != "EW-CORE-061")
        .collect();
    assert!(findings.is_empty(), "{findings:#?}");
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
    // Two own locations: an unlocated rail-bonded terminal is EW-POT-008 (amended by cycle 33).
    for (n, code) in [(0x901u128, "A"), (0x902, "B")] {
        fx.push(json!({"$k": "structure_node", "id": us(n), "aspect": "location", "code": code}));
    }
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
fn core023_cases() {
    rule_table(
        "EW-CORE-023",
        vec![
            Case::reports(
                "a page with a description override",
                page_with_override(Some(json!({"en": "Override"}))),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].rule, "EW-CORE-023", "case {c}");
                    assert_eq!(f[0].code, "101023", "case {c}");
                    assert_eq!(f[0].severity, Severity::Info, "case {c}");
                    assert_eq!(f[0].discriminator, "description_override", "case {c}");
                    assert_eq!(f[0].targets.len(), 1, "case {c}");
                    assert_eq!(
                        (f[0].targets[0].kind, f[0].targets[0].id),
                        (Kind::Page, uid(P1)),
                        "case {c}"
                    );
                    assert!(f[0].message.contains("folio 1"), "case {c}: {}", f[0].message);
                    assert!(f[0].message.contains("Override"), "case {c}: {}", f[0].message);
                    let fix = f[0]
                        .fix
                        .as_ref()
                        .unwrap_or_else(|| panic!("case {c}: a quick fix"));
                    assert_eq!(fix.command, "fields.set", "case {c}");
                    assert_eq!(fix.params["path"], json!("description_override"), "case {c}");
                    assert_eq!(fix.params["value"], Value::Null, "case {c}");
                },
            ),
            Case::silent("a page without override", page_with_override(None)),
        ],
    );
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
fn core053_cases() {
    rule_table(
        "EW-CORE-053",
        vec![
            Case::silent(
                "an anchor at the graphics centre",
                main_and_aux("graphic_centre", true),
            ),
            Case::reports(
                "an origin anchor outside the graphics is an empty cell",
                main_and_aux("origin", true),
                |c, f| {
                    assert_eq!(f.len(), 1, "case {c}: {f:#?}");
                    assert_eq!(f[0].code, "103053", "case {c}");
                    assert_eq!(f[0].severity, Severity::Warning, "case {c}");
                    assert_eq!(f[0].discriminator, "", "case {c}");
                    assert_eq!(f[0].page.map(|p| p.to_string()), Some(us(P1)), "case {c}");
                    assert_eq!(f[0].targets.len(), 2, "case {c}");
                    assert_eq!(f[0].targets[0].kind, Kind::Representation, "case {c}");
                    assert!(
                        f[0].message.contains(": empty cell"),
                        "case {c}: {}",
                        f[0].message
                    );
                    assert!(
                        f[0].message.starts_with(&f[0].subject),
                        "case {c}: {}",
                        f[0].message
                    );
                    let fix = f[0]
                        .fix
                        .as_ref()
                        .unwrap_or_else(|| panic!("case {c}: a quick fix"));
                    assert_eq!(fix.command, "rep.set_anchor_policy", "case {c}");
                    assert_eq!(fix.params["anchor"], json!("graphic_centre"), "case {c}");
                },
            ),
            Case::silent(
                "a symbol without graphics is not judged",
                main_and_aux("origin", false),
            ),
        ],
    );
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

/// EW-CORE-065 in a release run over a part description: (name, description, the sorted discriminators).
#[test]
fn core065_release_cases() {
    let cases: [(&str, &str, &[&str]); 2] = [
        ("a plain description has no finding", "Thermostat", &[]),
        (
            "a rating and a glued unit are two warnings",
            "Miniature circuit breaker, 10A C",
            &["rating", "unit-format"],
        ),
    ];
    for (name, description, want) in cases {
        let f = release_findings(&part_model(description), "EW-CORE-065");
        let mut found: Vec<&str> = f.iter().map(|f| f.discriminator.as_str()).collect();
        found.sort_unstable();
        assert_eq!(found, want, "case {name}: {f:#?}");
        for finding in &f {
            assert_eq!(finding.code, "104065", "case {name}");
            assert_eq!(finding.severity, Severity::Warning, "case {name}");
            assert_eq!(finding.targets.len(), 1, "case {name}");
            assert_eq!(
                (finding.targets[0].kind, finding.targets[0].id),
                (Kind::Part, uid(0x8000)),
                "case {name}"
            );
        }
    }
}

#[test]
fn core065_does_not_run_online() {
    let m = part_model("Miniature circuit breaker, 10A C");
    assert!(findings_of(&m, "EW-CORE-065").is_empty());
}
