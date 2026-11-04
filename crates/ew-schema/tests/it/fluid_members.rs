// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fluid members typed inside format 0.1 (WP-S1.20 D2, FL-1): `ConnectionKind::Fluid` with its line kind,
//! `CpDef.port_role`, `CpOverride.closure`, `PartSections.fluid` and `TextSlot.fallback`. Each round-trips byte-equal,
//! an unknown value of each new enum survives verbatim as `Unknown`, and a default member is omitted.

use crate::common;

use common::fill;
use ew_schema::{ClosureKind, ConnectionKind, FluidPortRole, LibDocument, LineKind, Record, SlotFallback};

fn record(line: &str) -> Record {
    let r: Record = serde_json::from_str(line).unwrap_or_else(|e| panic!("parse failed: {e}\n{line}"));
    assert_eq!(serde_json::to_string(&r).unwrap(), line, "not canonical");
    r
}

fn lib(line: &str) -> LibDocument {
    let d: LibDocument = serde_json::from_str(line).unwrap_or_else(|e| panic!("parse failed: {e}\n{line}"));
    assert_eq!(serde_json::to_string(&d).unwrap(), line, "not canonical");
    d
}

fn connection(kind: &str) -> String {
    fill(&format!(
        r#"{{"$k":"connection","id":"@CON_W7","a":{{"cp":["@FN_S51",0]}},"b":{{"cp":["@FN_F1",1]}},"kind":{kind}}}"#
    ))
}

fn line_kind(r: &Record) -> &LineKind {
    let Record::Connection(c) = r else {
        panic!("a connection");
    };
    let ConnectionKind::Fluid(f) = &c.kind else {
        panic!("a fluid line: {:?}", c.kind);
    };
    &f.line_kind
}

#[test]
fn a_fluid_line_types_its_kind_and_omits_working() {
    assert_eq!(
        line_kind(&record(&connection(r#"{"fluid":{}}"#))),
        &LineKind::Working
    );
    assert_eq!(
        line_kind(&record(&connection(r#"{"fluid":{"line_kind":"pilot"}}"#))),
        &LineKind::Pilot
    );
    for (word, kind) in [
        ("exhaust", LineKind::Exhaust),
        ("drain", LineKind::Drain),
        ("flexible", LineKind::Flexible),
    ] {
        let line = connection(&format!(r#"{{"fluid":{{"line_kind":"{word}"}}}}"#));
        assert_eq!(line_kind(&record(&line)), &kind);
    }
    // A working line written explicitly is read and written in the canonical form, without the default.
    let explicit = connection(r#"{"fluid":{"line_kind":"working"}}"#);
    let r: Record = serde_json::from_str(&explicit).unwrap();
    assert_eq!(serde_json::to_string(&r).unwrap(), connection(r#"{"fluid":{}}"#));
}

#[test]
fn an_unknown_line_kind_and_the_tube_survive_verbatim() {
    let r = record(&connection(
        r#"{"fluid":{"line_kind":"capillary","length":1000}}"#,
    ));
    assert_eq!(line_kind(&r), &LineKind::Unknown(serde_json::json!("capillary")));
    let Record::Connection(c) = &r else { unreachable!() };
    let ConnectionKind::Fluid(f) = &c.kind else {
        unreachable!()
    };
    assert_eq!(f.extra.keys().map(String::as_str).collect::<Vec<_>>(), ["length"]);
}

#[test]
fn a_closure_types_its_kind_and_part_and_keeps_an_unknown_kind() {
    let line = fill(concat!(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","cp_overrides":["#,
        r#"{"idx":1,"closure":{"kind":"plug","part":{"copy":"@PART_TWIN"}}},"#,
        r#"{"idx":3,"closure":{"kind":"silencer"}},"#,
        r#"{"idx":4,"unused":true,"closure":{"kind":"welded","note":"x"}}]}"#
    ));
    let Record::Function(f) = record(&line) else {
        panic!("a function");
    };
    let closures: Vec<_> = f
        .cp_overrides
        .iter()
        .map(|o| o.closure.clone().expect("a closure"))
        .collect();
    assert_eq!(closures[0].kind, ClosureKind::Plug);
    assert!(closures[0].part.is_some());
    assert_eq!(closures[1].kind, ClosureKind::Silencer);
    assert_eq!(closures[1].part, None);
    assert_eq!(
        closures[2].kind,
        ClosureKind::Unknown(serde_json::json!("welded"))
    );
    assert_eq!(
        closures[2].extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["note"]
    );
    let blanked = fill(
        r#"{"$k":"function","id":"@FN_S51","device":"@DEV_S51","def":"@FNDEF_MCB","cp_overrides":[{"idx":0,"closure":{"kind":"blanked"}}]}"#,
    );
    let Record::Function(f) = record(&blanked) else {
        panic!("a function");
    };
    assert_eq!(
        f.cp_overrides
            .iter()
            .next()
            .unwrap()
            .closure
            .as_ref()
            .unwrap()
            .kind,
        ClosureKind::Blanked
    );
}

#[test]
fn port_roles_are_typed_and_an_unknown_role_is_kept() {
    let words = [
        ("supply1", FluidPortRole::Supply1),
        ("working2", FluidPortRole::Working2),
        ("working4", FluidPortRole::Working4),
        ("exhaust3", FluidPortRole::Exhaust3),
        ("exhaust5", FluidPortRole::Exhaust5),
        ("pilot10", FluidPortRole::Pilot10),
        ("pilot12", FluidPortRole::Pilot12),
        ("pilot14", FluidPortRole::Pilot14),
        ("pilot_exhaust82", FluidPortRole::PilotExhaust82),
        ("pilot_exhaust84", FluidPortRole::PilotExhaust84),
        ("external_pilot", FluidPortRole::ExternalPilot),
        ("consumer", FluidPortRole::Consumer),
        ("sensing", FluidPortRole::Sensing),
        ("generic", FluidPortRole::Generic),
        ("leak", FluidPortRole::Unknown(serde_json::json!("leak"))),
    ];
    let cps: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(i, (w, _))| {
            format!(r#"{{"idx":{i},"designation":"{i}","discipline":"pneumatic","port_role":"{w}"}}"#)
        })
        .collect();
    let line = fill(&format!(
        r#"{{"$k":"fndef","id":"@FNDEF_MCB","library_id":"@LIB","item_id":"@ITEM_FN","version":"1.0.0","sha256":"@HASH","category":["Valve","Directional","5/2"],"contact_kind":"valve","cps":[{}]}}"#,
        cps.join(",")
    ));
    let LibDocument::Fndef(d) = lib(&line) else {
        panic!("a fndef");
    };
    let roles: Vec<_> = d.item.cps.iter().map(|c| c.port_role.clone().unwrap()).collect();
    assert_eq!(roles, words.map(|(_, r)| r).to_vec());
}

#[test]
fn a_fluid_section_types_its_solenoids_and_keeps_the_rest() {
    let line = fill(concat!(
        r#"{"$k":"part","id":"@PART_CONT","library_id":"@LIB","item_id":"@ITEM_CONT","version":"1.0.0","sha256":"@HASH","#,
        r#""manufacturer":{"library":"@LIB","item":"@ITEM_MFR","version_req":"^1"},"part_number":"571334","#,
        r#""sections":{"fluid":{"solenoids":["14","12"],"valve_model":{"positions":2}}}}"#
    ));
    let LibDocument::Part(p) = lib(&line) else {
        panic!("a part");
    };
    let fluid = p.item.sections.fluid.as_ref().expect("typed");
    assert_eq!(
        fluid.solenoids.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        ["14", "12"]
    );
    assert_eq!(
        fluid.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["valve_model"]
    );
    // An empty solenoid list is omitted.
    let empty = line.replace(r#""solenoids":["14","12"],"#, "");
    let LibDocument::Part(p) = lib(&empty) else {
        panic!("a part");
    };
    assert!(p.item.sections.fluid.unwrap().solenoids.is_empty());
}

#[test]
fn a_slot_fallback_is_typed_and_an_unknown_fallback_is_kept() {
    let symbol = |fallback: &str| {
        fill(&format!(
            r#"{{"$k":"symbol","id":"@SYM_MCB","library_id":"@LIB","item_id":"@ITEM_SYM","version":"1.0.0","sha256":"@HASH","fndef":{{"library":"@LIB","item":"@ITEM_FN","version_req":"^1"}},"grid":{{"num":4000000,"den":1}},"text_slots":[{{"kind":"function_text","pos":[0,0],"align":"top_left","fallback":"{fallback}"}}]}}"#
        ))
    };
    let LibDocument::Symbol(s) = lib(&symbol("process_tag")) else {
        panic!("a symbol");
    };
    assert_eq!(s.item.text_slots[0].fallback, Some(SlotFallback::ProcessTag));
    let LibDocument::Symbol(s) = lib(&symbol("loop_tag")) else {
        panic!("a symbol");
    };
    assert_eq!(
        s.item.text_slots[0].fallback,
        Some(SlotFallback::Unknown(serde_json::json!("loop_tag")))
    );
}
