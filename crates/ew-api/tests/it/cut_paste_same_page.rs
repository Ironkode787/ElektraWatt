// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A cut and a paste on the same page are a move (doc 06 §3.6): the wires from devices the cut did not take to the
//! terminals it brought back follow the terminals to their new place, as a drag's wires do (`rep.move`), so no wire
//! is drawn to the spot where a terminal stood.

use crate::common;

use std::collections::BTreeMap;

use ew_api::{NewProjectKind, ProjectTemplate, Session, SessionOptions};
use ew_ids::Uuid;
use ew_testids::seeded_ids;
use ew_units::{Grid, Nm};
use serde_json::{Value, json};

/// A point of the QET grid by its column and row (7 937 500 / 3 nm a step, a point rounded to the nm).
fn at(col: i64, row: i64) -> Value {
    json!([(col * 7_937_500 + 1) / 3, (row * 7_937_500 + 1) / 3])
}

/// The functions `device.place` made: those `s` holds now and did not before.
fn placed(s: &mut Session, params: Value, before: &[Uuid]) -> Uuid {
    common::run_cmd(s, "device.place", params);
    s.query()
        .functions()
        .ids()
        .map(|f| f.as_uuid())
        .find(|f| !before.contains(f))
        .expect("a new function")
}

#[test]
fn wires_to_terminals_cut_and_pasted_two_steps_aside_end_at_their_pins() {
    let mut s = Session::new_project(
        ProjectTemplate::Builtin,
        NewProjectKind::Schematic,
        SessionOptions {
            ids: Some(seeded_ids(7)),
            ..SessionOptions::default()
        },
    )
    .expect("a new project");
    let page = s.page_tables("1").expect("page 1").page.to_string();
    for key in ["ew.iec.terminal@qet10", "ew.iec.lamp@qet10"] {
        if let Some(embed) = s.starter_embed(key) {
            common::run_cmd(&mut s, "library.embed", embed);
        }
    }
    let symbol = |s: &Session, key: &str| s.starter_symbol_copy(key).expect("a starter symbol").to_string();
    let (terminal, lamp) = (
        symbol(&s, "ew.iec.terminal@qet10"),
        symbol(&s, "ew.iec.lamp@qet10"),
    );
    let strip = common::run_cmd(&mut s, "device.create", json!({"letter": "X", "strip": {}}))
        .outcome
        .primary
        .map(|k| k.1.as_uuid())
        .expect("the strip");
    // Two lamps over two terminals: each lamp's lower pin (row 13) wired straight down to its terminal's upper pin
    // (row 19).
    let mut terminals = Vec::new();
    for (col, n) in [(40, "1"), (50, "2")] {
        let before: Vec<Uuid> = s.query().functions().ids().map(|f| f.as_uuid()).collect();
        let l = placed(
            &mut s,
            json!({"page": page, "symbol": lamp, "pos": at(col, 10), "target": {"new_device": {"letter": "E"}}}),
            &before,
        );
        let before: Vec<Uuid> = s.query().functions().ids().map(|f| f.as_uuid()).collect();
        let t = placed(
            &mut s,
            json!({"page": page, "symbol": terminal, "pos": at(col, 20),
                "target": {"new_terminal": {"strip": strip.to_string(), "designation": n}}}),
            &before,
        );
        common::run_cmd(
            &mut s,
            "connection.draw",
            json!({"page": page, "a": {"cp": {"function": l.to_string(), "idx": 1}},
                "b": {"cp": {"function": t.to_string(), "idx": 1}}, "route": [at(col, 13), at(col, 19)]}),
        );
        terminals.push(t);
    }
    // Cut the terminals as the window cuts, then paste them two grid steps to the right on the same page.
    let tables = s.page_tables("1").expect("page 1");
    let reps: Vec<Uuid> = tables
        .reps
        .iter()
        .filter(|r| r.functions.iter().any(|f| terminals.contains(f)))
        .map(|r| r.id)
        .collect();
    assert_eq!(reps.len(), 2);
    let fragment = ew_api::window::parse_fragment(&s.copy_fragment(&reps).expect("copied").text)
        .expect("the copy reads")
        .params;
    let targets: Vec<Value> = reps
        .iter()
        .map(|r| json!({"kind": "representation", "id": r.to_string()}))
        .collect();
    common::run_cmd(&mut s, "edit.delete", json!({ "targets": targets }));
    let step = Grid::QET_10PX;
    let x = fragment["anchor"][0].as_i64().expect("an anchor");
    let y = fragment["anchor"][1].as_i64().expect("an anchor");
    let to = json!([
        step.at(step.snap(Nm(x)) + 2).expect("on the page").0,
        step.at(step.snap(Nm(y))).expect("on the page").0
    ]);
    common::run_cmd(
        &mut s,
        "edit.paste",
        json!({"page": page, "at": to, "fragment": fragment}),
    );
    // Every wire that ends at a terminal ends at the pin its pasted symbol draws.
    let tables = s.page_tables("1").expect("page 1");
    let pins: BTreeMap<(Uuid, u16), (i64, i64)> = tables
        .cps
        .iter()
        .filter(|c| terminals.contains(&c.function))
        .map(|c| ((c.function, c.idx), c.at))
        .collect();
    assert_eq!(pins.len(), 4, "the terminals drawn again, two pins each");
    let q = s.query();
    let mut checked = 0;
    for (_, c) in q.connections().iter() {
        for (end, point) in [(&c.a, c.route.first()), (&c.b, c.route.last())] {
            let ew_schema::Endpoint::Cp(f, idx) = end else {
                continue;
            };
            let Some(pin) = pins.get(&(f.as_uuid(), *idx)) else {
                continue;
            };
            let point = point.map(|p| (p.x.0, p.y.0));
            assert_eq!(point, Some(*pin), "the wire ends at its terminal's pin");
            checked += 1;
        }
    }
    assert_eq!(checked, 2, "the two lamp wires");
}
