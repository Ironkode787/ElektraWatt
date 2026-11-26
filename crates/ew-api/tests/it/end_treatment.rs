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
