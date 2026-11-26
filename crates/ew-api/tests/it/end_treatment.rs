// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The treatment of a wire's ends on the sample (doc 13 §4): 101.4, 101.5 and 101.6 leave -X1011:PE clamp a through one
//! star, 2.5, 0.75 and 2.5 mm²: the two of one size share a twin ferrule, the third takes a ferrule of its own, each
//! with its part from the generic stock; the shop's `two_wires` set to a ferrule each gives three ferrules, which the
//! terminal's two-conductor clamp cannot take (EW-POT-010); `project.settings.set` sets the two members in words.

use crate::common;

use ew_api::checks::{CheckMode, PRE_DELIVERY};
use ew_api::output::NavKind;
use ew_api::{AnyCommand, Session};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) -> Result<ew_api::Receipt, ew_api::CmdError> {
    let cmd = AnyCommand {
        name: name.into(),
        params,
    };
    match s.execute(&cmd) {
        Err(ew_api::CmdError::NeedsPreview { .. }) => s.preview(&cmd).and_then(|p| s.commit(p)),
        other => other,
    }
}

/// The cells of the columns `keys` of the wire `no` in the Connections table.
fn cells(s: &Session, no: &str, keys: &[&str]) -> Vec<String> {
    let t = s.navigator_table(NavKind::Connections);
    let row = (t.nodes.iter())
        .find(|n| n.table_row && n.label == no)
        .unwrap_or_else(|| panic!("no wire {no}"));
    keys.iter()
        .map(|k| {
            let c = t.columns.iter().position(|c| c.key == *k).expect("the column");
            row.cells[c].clone()
        })
        .collect()
}

/// The EW-POT-010 findings of a check before delivery, as their messages.
fn over_capacity(s: &Session) -> Vec<String> {
    s.check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-POT-010")
        .map(|f| f.message())
        .collect()
}

#[test]
fn two_wires_of_one_size_on_one_clamp_share_a_twin_ferrule() {
    let mut s = common::sample();
    let keys = ["from_end", "from_part", "from_strip"];
    assert_eq!(
        cells(&s, "101.4", &keys),
        ["twin 2.5 BU · 101.6", "EW-GEN-FER-2X2.5", "10 mm"]
    );
    assert_eq!(cells(&s, "101.6", &keys[..1]), ["twin 2.5 BU · 101.4"]);
    assert_eq!(
        cells(&s, "101.5", &keys),
        ["ferrule 0.75 GY", "EW-GEN-FER-0.75", "8 mm"]
    );
    assert_eq!(over_capacity(&s), Vec::<String>::new());
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.two_wires", "value": "ferrule_each"}),
    )
    .expect("a ferrule each");
    assert_eq!(
        cells(&s, "101.4", &keys),
        ["ferrule 2.5 BU", "EW-GEN-FER-2.5", "10 mm"]
    );
    let over = over_capacity(&s);
    assert!(
        over.iter()
            .any(|m| m.starts_with("-X1011:PE") && m.ends_with("takes 2 conductors; 3 land on it")),
        "{over:?}"
    );
}

#[test]
fn the_workshop_members_are_set_in_words() {
    let mut s = common::sample();
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.ferrules", "value": "not_into_spring_clamps"}),
    )
    .expect("not into spring clamps");
    assert_eq!(
        s.query().project().settings.workshop.ferrules,
        ew_schema::FerruleRule::NotIntoSpringClamps
    );
    let refused = run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.ferrules", "value": "sideways"}),
    )
    .expect_err("an unknown rule");
    assert!(
        format!("{refused}").contains("not_into_spring_clamps"),
        "{refused}"
    );
    run(
        &mut s,
        "project.settings.set",
        json!({"member": "workshop.ferrules", "value": null}),
    )
    .expect("the default");
    assert_eq!(
        s.query().project().settings.workshop.ferrules,
        ew_schema::FerruleRule::EveryFineStrandedEnd
    );
}

/// The copy of the stock part numbered `number`.
fn stock(s: &Session, number: &str) -> String {
    let q = s.query();
    let parts = q.parts();
    q.project()
        .settings
        .workshop
        .stock
        .iter()
        .find(|r| {
            parts
                .get(r.copy)
                .is_some_and(|c| c.item.part_number.as_str() == number)
        })
        .map(|r| r.copy.to_string())
        .unwrap_or_else(|| panic!("no stock part {number}"))
}

/// The connection that lands on the end `end` of wire `no` and the member that holds its end set by hand.
fn end_of(s: &Session, no: &str, end: usize) -> (String, &'static str) {
    let t = s.navigator_table(NavKind::Connections);
    let row = (t.nodes.iter())
        .find(|n| n.table_row && n.label == no)
        .unwrap_or_else(|| panic!("no wire {no}"));
    let (c, at_a) = s.wire_end(&row.records, end).expect("an end of the project");
    (
        c.to_string(),
        if at_a {
            "overrides.end_a"
        } else {
            "overrides.end_b"
        },
    )
}

/// The EW-MFG-009 findings, as their messages and the commands of their quick fixes.
fn misfits(s: &Session) -> Vec<(String, Option<(String, Value)>)> {
    s.check(&PRE_DELIVERY, CheckMode::Release)
        .findings
        .iter()
        .filter(|f| f.rule == "EW-MFG-009")
        .map(|f| {
            (
                f.message(),
                f.fix.as_ref().map(|x| (x.command.clone(), x.params.clone())),
            )
        })
        .collect()
}

/// The sample's 101.4 lands on -101G1:PE with a ring lug set by hand (no ring lug in the stock: no part); a twin ferrule
/// set there instead has no second wire and a ferrule part of 0.75 mm² does not fit the 2.5 mm² wire (EW-MFG-009), whose
/// quick fix gives the rule's ferrule back; a part the project does not hold is refused.
#[test]
fn an_end_set_by_hand_prints_as_set_and_a_misfit_is_reported() {
    let mut s = common::sample();
    let keys = ["to_end", "to_part", "to_strip"];
    assert_eq!(cells(&s, "101.4", &keys), ["ring lug", "", ""]);
    let (holder, member) = end_of(&s, "101.4", 1);
    let set = |s: &mut Session, value: Value| {
        run(
            s,
            "fields.set",
            json!({"kind": "connection", "targets": [holder], "path": member, "value": value}),
        )
    };
    assert_eq!(misfits(&s), Vec::new());
    // Its To end row in Properties: the ring lug, set by hand, with Reset, on the member that holds it.
    let sheet = s.property_sheet(&[holder.parse().expect("an ID")]);
    let row = (sheet.sections.iter().flat_map(|x| x.rows.iter()))
        .find(|r| r.key == "connection/derived.to_end")
        .expect("the To end row");
    assert_eq!(
        (&row.value, &row.provenance, row.path.as_str()),
        (
            &ew_api::output::RowValue::One {
                json: json!("ring_lug"),
                text: "ring_lug".into()
            },
            &ew_api::output::Provenance::Overridden {
                note: "set by hand".into()
            },
            format!("{member}.treatment").as_str()
        )
    );
    assert!(row.actions.contains(&ew_api::output::RowAction::Reset), "{row:?}");
    set(&mut s, json!({"treatment": "twin_ferrule"})).expect("a twin ferrule");
    assert_eq!(cells(&s, "101.4", &keys[..1]), ["twin 2.5 BU"]);
    let found = misfits(&s);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0]
            .0
            .ends_with("set by hand does not fit: a twin ferrule with no second wire"),
        "{found:?}"
    );
    let (command, params) = found[0].1.clone().expect("Reset to the rule");
    run(&mut s, &command, params).expect("the quick fix");
    assert_eq!(cells(&s, "101.4", &keys[..1]), ["ferrule 2.5 BU"]);
    assert_eq!(misfits(&s), Vec::new());
    let small = stock(&s, "EW-GEN-FER-0.75");
    set(&mut s, json!({"treatment": "ferrule", "part": {"copy": small}})).expect("a ferrule part");
    assert_eq!(cells(&s, "101.4", &keys[1..2]), ["EW-GEN-FER-0.75"]);
    let found = misfits(&s);
    assert!(
        found
            .iter()
            .any(|(m, _)| m.ends_with("a ferrule for 0.75 mm² on a 2.5 mm² wire")),
        "{found:?}"
    );
    let unknown = ew_ids::Uuid::from_u128(0x5e47_0000_0000_4000_8000_0000_0000_0042).to_string();
    set(&mut s, json!({"part": {"copy": unknown}})).expect_err("a part the project does not hold");
}
