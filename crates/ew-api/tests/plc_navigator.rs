// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC navigator and grid on Golden A (WP-S2.07 part 3, §8; acceptance PG1, PG4, PG5): the tree's levels and badges,
//! the grid's facets (`spare`, `unplaced`, `unassigned`, `tbd`, `duplicate`, `config:<name>`) before and after the
//! edits that set them, and the typed editors of a channel's address and target and a module's slot and start.

mod common;

use ew_api::output::{InputError, NavKind, NavNode, NavRole, NavTable};
use ew_api::{AnyCommand, Session};
use serde_json::{Value, json};

fn run(s: &mut Session, name: &str, params: Value) {
    s.execute(&AnyCommand {
        name: name.into(),
        params,
    })
    .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

fn channels(t: &NavTable) -> Vec<&NavNode> {
    t.nodes.iter().filter(|n| n.role == NavRole::Channel).collect()
}

/// The channel rows carrying the flag or facet `name` (with `value`, or any value for a flag).
fn with<'t>(t: &'t NavTable, name: &str, value: &str) -> Vec<&'t NavNode> {
    channels(t)
        .into_iter()
        .filter(|n| n.facets.iter().any(|(f, v)| f == name && v == value))
        .collect()
}

fn cell<'t>(t: &'t NavTable, n: &'t NavNode, key: &str) -> &'t str {
    let i = t.columns.iter().position(|c| c.key == key).unwrap();
    &n.cells[i]
}

/// The channel row whose module prints `module` and whose number is `number`.
fn channel<'t>(t: &'t NavTable, module: &str, number: u16) -> &'t NavNode {
    channels(t)
        .into_iter()
        .find(|n| cell(t, n, "module") == module && cell(t, n, "channel") == number.to_string())
        .unwrap_or_else(|| panic!("no channel {module} {number}"))
}

fn set(s: &mut Session, targets: &[&NavNode], path: &str, value: Value) {
    let ids: Vec<String> = targets.iter().map(|n| n.records[0].to_string()).collect();
    run(
        s,
        "fields.set",
        json!({"kind": "channel", "targets": ids, "path": path, "value": value}),
    );
}

/// PG1: the configuration project above -201U1, its role word as the station's badge, the module levels, and the
/// `unplaced` badge on exactly the channels without a binding (the deactivated ones on Golden A, which no coil binds),
/// beside `spare` on the 20 spare channels.
#[test]
fn pg1_the_tree_levels_and_badges() {
    let s = common::golden_session();
    let t = s.navigator_table(NavKind::Plc);
    let top: Vec<&NavNode> = t.nodes.iter().filter(|n| n.parent.is_none()).collect();
    assert_eq!(top.len(), 1);
    assert_eq!(top[0].role, NavRole::ConfigProject);
    assert!(!top[0].label.is_empty(), "the configuration project's name");
    let station = &t.nodes[t.nodes.iter().position(|n| n.role == NavRole::Station).unwrap()];
    assert_eq!(station.parent, Some(0));
    assert_eq!(station.badges.len(), 1, "the station's role word");
    let all = channels(&t);
    assert_eq!(all.len(), 56);
    for n in &all {
        let state = cell(&t, n, "state");
        let unplaced = n.badges.iter().any(|b| b == "unplaced");
        assert_eq!(unplaced, cell(&t, n, "placed_on").is_empty(), "{:?}", n.cells);
        assert_eq!(unplaced, state == "deactivated", "{:?}", n.cells);
        assert_eq!(n.badges.iter().any(|b| b == "spare"), state == "spare");
        assert_eq!(n.records.len(), 2, "the channel and its module");
        assert_eq!(n.kinds, ["channel", "device"]);
    }
}

/// PG4: each facet on Golden A, then after the edit that sets it. FR-PLC-021: `spare` shows exactly 20 rows.
#[test]
fn pg4_each_facet_on_golden_a() {
    let mut s = common::golden_session();
    let t = s.navigator_table(NavKind::Plc);
    assert_eq!(with(&t, "spare", "").len(), 20, "FR-PLC-021");
    assert_eq!(with(&t, "unplaced", "").len(), 28);
    let config = t.nodes[0].label.clone();
    assert_eq!(with(&t, "config", &config).len(), 56);
    assert!(with(&t, "unassigned", "").is_empty());
    assert!(with(&t, "tbd", "").is_empty());
    assert!(with(&t, "duplicate", "").is_empty());
    let u3_0 = channel(&t, "-201U3", 0).clone();
    let u3_1 = channel(&t, "-201U3", 1).clone();
    let u4_0 = channel(&t, "-201U4", 0).clone();
    let u4_1 = channel(&t, "-201U4", 1).clone();
    let u5_2 = channel(&t, "-201U5", 2).clone();
    let other = cell(&t, &u4_1, "address").to_owned();

    set(&mut s, &[&u3_0], "address", json!("excluded"));
    set(&mut s, &[&u3_1], "address", json!("tbd"));
    // EW-PLC-007: one symbol on two channels.
    set(&mut s, &[&u4_0, &u5_2], "symbol", json!("B1_Start"));
    // EW-PLC-001's overlap: a manual address equal to another channel's.
    set(&mut s, &[&u3_1], "address", json!(other));
    let t = s.navigator_table(NavKind::Plc);
    let keys = |v: Vec<&NavNode>| -> Vec<String> { v.into_iter().map(|n| n.key.clone()).collect() };
    assert_eq!(keys(with(&t, "unassigned", "")), std::slice::from_ref(&u3_0.key));
    assert_eq!(cell(&t, channel(&t, "-201U3", 0), "address"), "");
    let mut dup = keys(with(&t, "duplicate", ""));
    dup.sort();
    let mut want = vec![
        u3_1.key.clone(),
        u4_0.key.clone(),
        u4_1.key.clone(),
        u5_2.key.clone(),
    ];
    want.sort();
    assert_eq!(dup, want, "the duplicate symbol and the overlapping address");

    set(&mut s, &[&u3_1], "address", json!("tbd"));
    let t = s.navigator_table(NavKind::Plc);
    assert_eq!(keys(with(&t, "tbd", "")), std::slice::from_ref(&u3_1.key));
    assert_eq!(cell(&t, channel(&t, "-201U3", 1), "address"), "TBD");
    assert_eq!(cell(&t, channel(&t, "-201U4", 0), "symbol"), "B1_Start");
}

/// PG5: the typed editors. A channel's address passes its text to `fields.set`, which reads it by the profile; a
/// target is a device tag (its main function); a slot a number, `<rack>.<slot>` or `auto`; a module's start the
/// profile's start text for `plc.set_start`.
#[test]
fn pg5_the_typed_editors() {
    let mut s = common::golden_session();
    let parse = |s: &Session, kind: &str, path: &str, text: &str| {
        s.parse_field_input(kind, path, text).map(|p| p.value)
    };
    assert_eq!(parse(&s, "channel", "address", " I4.0 ").unwrap(), json!("I4.0"));
    assert_eq!(parse(&s, "channel", "address", "tbd").unwrap(), json!("tbd"));
    assert!(matches!(
        parse(&s, "channel", "address", ""),
        Err(InputError::Invalid(_))
    ));
    assert_eq!(parse(&s, "channel", "target", "").unwrap(), Value::Null);
    assert!(matches!(
        parse(&s, "channel", "target", "-999Z9"),
        Err(InputError::Invalid(_))
    ));
    assert_eq!(
        parse(&s, "device", "plc_module.slot", "auto").unwrap(),
        json!("positional")
    );
    assert_eq!(
        parse(&s, "device", "plc_module.slot", "3").unwrap(),
        json!({"rack": {"rack": 0, "slot": 3}})
    );
    assert_eq!(
        parse(&s, "device", "plc_module.slot", "1.4.2").unwrap(),
        json!({"rack": {"rack": 1, "slot": 4, "subslot": 2}})
    );
    assert!(matches!(
        parse(&s, "device", "plc_module.slot", "x"),
        Err(InputError::Invalid(_))
    ));
    assert_eq!(
        parse(&s, "device", "plc_module.start.output", "Q10").unwrap(),
        json!("Q10")
    );

    let t = s.navigator_table(NavKind::Plc);
    let u5_0 = channel(&t, "-201U5", 0).clone();
    let u5 = u5_0.records[1];
    // The slot through `fields.set` on the module (the route of the Slot column).
    let slot = parse(&s, "device", "plc_module.slot", "9").unwrap();
    run(
        &mut s,
        "fields.set",
        json!({"kind": "device", "targets": [u5.to_string()], "path": "plc_module.slot", "value": slot}),
    );
    // The start through `plc.set_start`.
    let spec = parse(&s, "device", "plc_module.start.output", "Q10").unwrap();
    run(
        &mut s,
        "plc.set_start",
        json!({"modules": [u5.to_string()], "dir": "output", "spec": spec}),
    );
    // A target by its device's tag.
    let target = parse(&s, "channel", "target", "-201U3").unwrap();
    set(&mut s, &[&u5_0], "target", target);
    let t = s.navigator_table(NavKind::Plc);
    let row = t.nodes.iter().find(|n| n.key == u5_0.key).unwrap();
    assert_eq!(cell(&t, row, "slot"), "9");
    assert_eq!(cell(&t, row, "address"), "Q10.0");
    assert_eq!(cell(&t, row, "target"), "-201U3");
}

/// The grid's routes (WP-S2.07 §8, ruling 6): the channel members through `fields.set`, the slot on the module; the
/// derived columns read-only.
#[test]
fn the_grid_columns_and_routes() {
    let s = common::golden_session();
    let t = s.navigator_table(NavKind::Plc);
    let route = |k: &str| {
        t.columns
            .iter()
            .find(|c| c.key == k)
            .and_then(|c| c.route.clone())
            .map(|(a, b)| format!("{a}/{b}"))
    };
    for k in ["designation", "address", "symbol", "state", "target", "comment"] {
        assert_eq!(route(k), Some(format!("channel/{k}")));
    }
    assert_eq!(route("slot").as_deref(), Some("device/plc_module.slot"));
    for k in ["function_text", "process_tag", "placed_on"] {
        let c = t.columns.iter().find(|c| c.key == k).unwrap();
        assert!(c.derived && c.route.is_none(), "{k}");
    }
    for k in ["station", "module", "channel", "kind"] {
        assert_eq!(route(k), None, "{k}");
    }
    assert!(t.columns[0].key_column);
}
