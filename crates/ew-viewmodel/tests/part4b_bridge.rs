// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 4b in the view model (acceptance check PI10, FT-411): `strip.bridge` (Edit ▸ Bridge terminals)
//! on the folio-103 demo variant, the bridge group the action derives, one undo step per bridge, and the refusals of
//! a selection that is not terminals of one strip; and (cycle 33) the chain over three or more terminals in one undo
//! step and the side-1 end of terminals without a bridge shaft. Every test works on a writable temporary copy of
//! Golden A.

mod common;

use std::path::Path;

use common::{GOLDEN_A, Tmp, at, done, refused};
use ew_viewmodel::actions::{ActionItem, Menu, menu_model};
use ew_viewmodel::canvas::{HOVER_SLOP_PT, pick};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::ew_api::output::{PageTables, TableRep};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::{Value, json};

/// -X1031's terminal functions 1 to 4, in sequence order (verified in [`assert_golden_endpoints`]).
const X1031: [&str; 4] = [
    "911c8c13-00e3-512e-84bf-c5a9fe29c6fd",
    "941aa213-6e5e-5fe3-8bca-008e8296a60d",
    "677c22f6-eb0b-539c-bcbc-5bc64890dec9",
    "ac9bd12a-9eb6-5719-bf65-38b973d38d05",
];
/// -X1031's strip device.
const X1031_STRIP: &str = "d8221507-61e8-5141-a2cb-95300ef8a242";
/// BR1's three connections, over -X1031 1–2, 2–3 and 3–4.
const BR1: [&str; 3] = [
    "b4459567-b8f3-5099-80d8-fb4e89b237ac",
    "7839b214-ef70-55a7-948d-7f15041b7e04",
    "cc6d528f-3d18-5553-8db0-e15a3fb415da",
];
/// -X1032's terminal 1.
const X1032_1: &str = "7f6b83c0-d75e-58da-864c-7f2fd5ed3b06";
/// The symbol Golden A's terminals use.
const TERMINAL_SYMBOL: &str = "ew.iec.terminal@qet10";

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "cursor.set", at(q, json!({})));
}

fn select_ids(s: &mut ShellModel, ids: &[String]) {
    done(s, "select.set", json!({ "ids": ids }));
}

/// Deletes the connections with these ID texts (`edit.delete`).
fn delete_connections(s: &mut ShellModel, connections: &[&str]) {
    let targets: Vec<Value> = connections
        .iter()
        .map(|id| json!({"kind": "connection", "id": id}))
        .collect();
    done(
        s,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": targets}}),
    );
}

/// A row of `n` free grid points four grid steps apart, inside the box of the page's connection points: no entity
/// and no link under any of them.
fn free_row(s: &ShellModel, n: usize) -> Vec<(i64, i64)> {
    let t = tables(s);
    let c = s.canvas().unwrap();
    let sn = &c.page().snapper;
    let slop = HOVER_SLOP_PT / c.points_per_nm();
    let free = |p: (i64, i64)| {
        let q = sn.page_to_sheet(p);
        c.link_at(q).is_none() && pick(&c.page().entities, Some(q), slop).is_none()
    };
    let min_x = t.cps.iter().map(|c| c.at.0).min().unwrap_or(0);
    let max_x = t.cps.iter().map(|c| c.at.0).max().unwrap_or(0);
    let min_y = t.cps.iter().map(|c| c.at.1).min().unwrap_or(0);
    let max_y = t.cps.iter().map(|c| c.at.1).max().unwrap_or(0);
    let (x0, x1) = (sn.grid_index_of(min_x) + 1, sn.grid_index_of(max_x));
    let (y0, y1) = (sn.grid_index_of(min_y) + 1, sn.grid_index_of(max_y));
    let span = 4 * (n as i64 - 1);
    for ky in y0..y1 {
        for kx in x0..=x1 - span {
            let points: Vec<(i64, i64)> = (0..n)
                .map(|i| (sn.page_of_index(kx + 4 * i as i64), sn.page_of_index(ky)))
                .collect();
            if points.iter().all(|p| free(*p)) {
                return points;
            }
        }
    }
    panic!("no free row of {n} grid points on folio {}", c.page().folio);
}

/// Places one terminal at page point `p` through the Place tool, without autoconnect, and leaves Select.
fn place_terminal_at(s: &mut ShellModel, p: (i64, i64), params: Value) {
    done(s, "place.pick", params);
    cursor(s, p);
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "ui.dismiss", json!({}));
}

/// Places a new representation of each function in `functions`, one row of free grid points, and returns their page
/// table rows in the same order.
fn place_terminals(s: &mut ShellModel, functions: &[&str]) -> Vec<TableRep> {
    let points = free_row(s, functions.len());
    for (function, point) in functions.iter().zip(&points) {
        place_terminal_at(
            s,
            *point,
            json!({"function": function, "symbol": TERMINAL_SYMBOL}),
        );
    }
    let t = tables(s);
    functions
        .iter()
        .map(|function| {
            t.reps
                .iter()
                .find(|r| r.functions.len() == 1 && r.functions[0].to_string() == *function)
                .unwrap_or_else(|| panic!("no placed representation of {function}"))
                .clone()
        })
        .collect()
}

/// The `strip.bridge` item of the app menu (WP-S2.17 part 2: Draw, Connect).
fn edit_item(s: &ShellModel, id: &str) -> ActionItem {
    let draw = menu_model(s)
        .into_iter()
        .find(|m| m.menu == Menu::Draw)
        .expect("the Draw menu");
    draw.items()
        .into_iter()
        .find(|item| item.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("no {id} in the Draw menu"))
}

/// The golden endpoints the bridge rule relies on: every function's device is the terminal aspect of -X1031 (or
/// -X1032), with the expected strip and sequence and a bridge shaft at CP index 3; BR1's three connections are the
/// golden bridge. A difference here stops the test before its own assertions.
fn assert_golden_endpoints() {
    let (session, _) =
        Session::open_read_only(Path::new(GOLDEN_A), "test").expect("Golden A opens read-only");
    let q = session.query();
    let terminal = |function: &str| {
        let (_, f) = q
            .functions()
            .iter()
            .find(|(id, _)| id.as_uuid().to_string() == function)
            .unwrap_or_else(|| panic!("the function {function}"));
        let device = q
            .devices()
            .get(f.device.expect("the terminal's device"))
            .expect("the device");
        let term = device.aspects.terminal.as_ref().expect("the terminal aspect");
        let part = device.parts.first().expect("the terminal's part");
        let copy = q
            .parts()
            .iter()
            .find(|(id, _)| id.as_uuid() == part.part.copy.as_uuid())
            .expect("the part copy")
            .1;
        let cp = copy
            .item
            .cp_pattern
            .iter()
            .find(|cp| cp.idx == 3)
            .expect("CP 3 of the part pattern");
        assert_eq!(
            serde_json::to_value(&cp.clamp).expect("the clamp's JSON"),
            json!({"shaft": 1}),
            "{function} has its bridge shaft at CP 3"
        );
        (term, device)
    };
    for (k, function) in X1031.iter().enumerate() {
        let (term, _) = terminal(function);
        assert_eq!(term.seq, k as u32 + 1, "{function}'s sequence");
        assert_eq!(term.designation.as_str(), (k + 1).to_string());
        assert_eq!(term.strip.as_uuid().to_string(), X1031_STRIP);
    }
    let (term, _) = terminal(X1032_1);
    assert_ne!(term.strip.as_uuid().to_string(), X1031_STRIP);
    for (k, connection) in BR1.iter().enumerate() {
        let (_, c) = q
            .connections()
            .iter()
            .find(|(id, _)| id.as_uuid().to_string() == *connection)
            .unwrap_or_else(|| panic!("the bridge connection {connection}"));
        assert_eq!(
            serde_json::to_value(&c.kind).expect("the kind's JSON"),
            json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 1}})
        );
        assert_eq!(
            serde_json::to_value(&c.a).expect("the a end's JSON"),
            json!({"cp": [X1031[k], 3]})
        );
        assert_eq!(
            serde_json::to_value(&c.b).expect("the b end's JSON"),
            json!({"cp": [X1031[k + 1], 3]})
        );
    }
}

/// The largest group among -X1031's bridge connections in the project at `dir`.
fn largest_x1031_bridge_group(dir: &Path) -> u64 {
    let (session, _) = Session::open_read_only(dir, "test").expect("the project copy opens read-only");
    let q = session.query();
    let mut max = 0;
    for (_, c) in q.connections().iter() {
        let Ok(kind) = serde_json::to_value(&c.kind) else {
            continue;
        };
        let Some(group) = kind
            .get("bridge")
            .and_then(|b| b.get("group"))
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let Some(a_function) = c.a.function() else {
            continue;
        };
        let Some(function) = q.functions().get(a_function) else {
            continue;
        };
        let Some(device) = function.device.and_then(|d| q.devices().get(d)) else {
            continue;
        };
        if device
            .aspects
            .terminal
            .as_ref()
            .map(|t| t.strip.as_uuid().to_string())
            .as_deref()
            == Some(X1031_STRIP)
        {
            max = max.max(group);
        }
    }
    max
}

/// -X1031 and BR1 deleted, four terminals placed, three bridges drawn.
fn three_bridges(tmp: &Tmp) -> (ShellModel, Vec<TableRep>, Vec<u64>) {
    let mut s = shell(tmp, "103");
    delete_connections(&mut s, &BR1);
    let reps = place_terminals(&mut s, &X1031);
    let mut groups = Vec::new();
    for k in 0..3 {
        select_ids(&mut s, &[reps[k].id.to_string(), reps[k + 1].id.to_string()]);
        let rev = s.revision();
        done(&mut s, "strip.bridge", json!({}));
        assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
        let last = s.recorded_commands().last().expect("the commit");
        assert_eq!(last.cmd, "connection.draw");
        groups.push(last.params["kind"]["bridge"]["group"].as_u64().expect("a group"));
    }
    (s, reps, groups)
}

// ---------------------------------------------------------------------------------------------------------------
// PI10: three bridges join four terminals in one group

#[test]
fn pi10_three_bridges_join_four_terminals_in_one_group() {
    assert_golden_endpoints();
    let tmp = Tmp::new("bridge-pi10");
    let mut s = shell(&tmp, "103");
    delete_connections(&mut s, &BR1);
    let reps = place_terminals(&mut s, &X1031);
    // The remaining -X1031 bridges (BR2 to BR5) are read from the saved modified copy, not from pristine Golden A.
    done(&mut s, "file.save", json!({}));
    let remaining = largest_x1031_bridge_group(&tmp.golden_copy());
    let mut groups = Vec::new();
    for k in 0..3 {
        // The first step selects the higher sequence first; the rule still puts :1 in `a` and :2 in `b`.
        let selection = if k == 0 {
            [reps[1].id.to_string(), reps[0].id.to_string()]
        } else {
            [reps[k].id.to_string(), reps[k + 1].id.to_string()]
        };
        select_ids(&mut s, &selection);
        let rev = s.revision();
        done(&mut s, "strip.bridge", json!({}));
        assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
        let last = s.recorded_commands().last().expect("the commit");
        assert_eq!(last.cmd, "connection.draw");
        assert_eq!(last.params["a"], json!({"cp": {"function": X1031[k], "idx": 3}}));
        assert_eq!(
            last.params["b"],
            json!({"cp": {"function": X1031[k + 1], "idx": 3}})
        );
        assert!(last.params.get("page").is_none(), "{}", last.params);
        assert!(last.params.get("route").is_none(), "{}", last.params);
        let group = last.params["kind"]["bridge"]["group"].as_u64().expect("a group");
        assert_eq!(
            last.params["kind"],
            json!({"bridge": {"kind": "plug_in", "group": group, "shaft": 1}})
        );
        groups.push(group);
    }
    let group0 = groups[0];
    assert_eq!(groups[1], group0, "the second bridge joins the first group");
    assert_eq!(groups[2], group0, "the third bridge joins the first group");
    assert_eq!(
        group0,
        remaining + 1,
        "the next free group after the remaining bridges"
    );
    // BR2 to BR5 remain on -X1031 in groups 2 to 5, so the first new bridge takes group 6.
    assert_eq!(group0, 6, "BR2 to BR5 remain in groups 2 to 5");
}

// ---------------------------------------------------------------------------------------------------------------
// PI10: every bridge is one undo step

#[test]
fn each_bridge_is_one_undo_step() {
    assert_golden_endpoints();
    let tmp = Tmp::new("bridge-undo");
    let (mut s, reps, _) = three_bridges(&tmp);
    let rev = s.revision();
    let steps = s.recorded_commands().len();
    done(&mut s, "edit.undo", json!({}));
    // An undo restores a snapshot and commits a new revision; one undo step took back exactly the third bridge.
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(s.recorded_commands().len(), steps - 1);
    // The third bridge is undone: :3 and :4 can be bridged again.
    select_ids(&mut s, &[reps[2].id.to_string(), reps[3].id.to_string()]);
    assert!(
        edit_item(&s, "strip.bridge").enabled,
        "the undone pair is bridgeable"
    );
    // The second bridge still stands: :2 and :3 are refused.
    select_ids(&mut s, &[reps[1].id.to_string(), reps[2].id.to_string()]);
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
    done(&mut s, "edit.redo", json!({}));
    select_ids(&mut s, &[reps[2].id.to_string(), reps[3].id.to_string()]);
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
}

// ---------------------------------------------------------------------------------------------------------------
// PI10: the group follows an existing bridge

#[test]
fn a_bridge_extends_the_group_of_an_existing_bridge() {
    assert_golden_endpoints();
    let tmp = Tmp::new("bridge-extend");
    let mut s = shell(&tmp, "103");
    delete_connections(&mut s, &BR1[2..]);
    let reps = place_terminals(&mut s, &X1031[2..]);
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    done(&mut s, "strip.bridge", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.params["kind"]["bridge"]["group"], 1, "{}", last.params);
}

// ---------------------------------------------------------------------------------------------------------------
// PI10: bridging twice is refused

#[test]
fn bridging_twice_is_refused() {
    assert_golden_endpoints();
    // The bridge drawn first counts.
    let tmp = Tmp::new("bridge-twice");
    let mut s = shell(&tmp, "103");
    delete_connections(&mut s, &BR1);
    let reps = place_terminals(&mut s, &X1031[..2]);
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    done(&mut s, "strip.bridge", json!({}));
    let rev = s.revision();
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
    assert_eq!(s.revision(), rev, "a refused bridge commits nothing");
    // Golden A's own BR1 1–2 counts.
    let tmp = Tmp::new("bridge-twice-golden");
    let mut s = shell(&tmp, "103");
    let reps = place_terminals(&mut s, &X1031[..2]);
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
}

// ---------------------------------------------------------------------------------------------------------------
// PI10, cycle 33: the selection must be two or more terminals of one strip

#[test]
fn the_selection_must_be_terminals_of_one_strip() {
    assert_golden_endpoints();
    let tmp = Tmp::new("bridge-selection");
    let mut s = shell(&tmp, "103");
    let reps = place_terminals(&mut s, &[X1031[0], X1032_1]);
    // One terminal.
    select_ids(&mut s, &[reps[0].id.to_string()]);
    assert_eq!(
        refused(&mut s, "strip.bridge", json!({})),
        "reason-bridge-selection"
    );
    let item = edit_item(&s, "strip.bridge");
    assert!(!item.enabled);
    assert_eq!(
        item.reason.as_deref(),
        Some("Select two or more terminals of one strip")
    );
    // A terminal and a non-terminal representation.
    let other = tables(&s)
        .reps
        .iter()
        .find(|r| r.ip.is_some())
        .expect("an interruption point on 103")
        .id
        .to_string();
    select_ids(&mut s, &[reps[0].id.to_string(), other]);
    assert_eq!(
        refused(&mut s, "strip.bridge", json!({})),
        "reason-bridge-selection"
    );
    // Two terminals of different strips: their own reason.
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    let rev = s.revision();
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-strips");
    assert_eq!(s.revision(), rev, "a refused bridge commits nothing");
    let item = edit_item(&s, "strip.bridge");
    assert!(!item.enabled);
    assert_eq!(
        item.reason.as_deref(),
        Some("The selected terminals belong to more than one strip; select terminals of one strip")
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Cycle 33 (G-PP ruling #4): four terminals are one chain in one undo step

#[test]
fn four_terminals_draw_the_chain_in_one_undo_step() {
    assert_golden_endpoints();
    let tmp = Tmp::new("bridge-chain");
    let mut s = shell(&tmp, "103");
    delete_connections(&mut s, &BR1);
    let reps = place_terminals(&mut s, &X1031);
    // Selected out of order: the chain follows the strip's order, :1-:2, :2-:3, :3-:4.
    let selection: Vec<String> = [3, 0, 2, 1].iter().map(|k| reps[*k].id.to_string()).collect();
    select_ids(&mut s, &selection);
    assert!(edit_item(&s, "strip.bridge").enabled);
    let rev = s.revision();
    let steps = s.recorded_commands().len();
    done(&mut s, "strip.bridge", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one transaction");
    assert_eq!(s.recorded_commands().len(), steps + 1, "one recorded step");
    let last = s.recorded_commands().last().expect("the commit").clone();
    assert_eq!(last.cmd, "connection.draw_many");
    let connections = last.params["connections"].as_array().expect("the entries");
    assert_eq!(connections.len(), 3, "{}", last.params);
    for (k, c) in connections.iter().enumerate() {
        assert_eq!(c["a"], json!({"cp": {"function": X1031[k], "idx": 3}}));
        assert_eq!(c["b"], json!({"cp": {"function": X1031[k + 1], "idx": 3}}));
        // BR2 to BR5 remain on -X1031 in groups 2 to 5, so the chain takes group 6.
        assert_eq!(
            c["kind"],
            json!({"bridge": {"kind": "plug_in", "group": 6, "shaft": 1}})
        );
        assert!(c.get("page").is_none() && c.get("route").is_none(), "{c}");
    }
    // Every pair is bridged now.
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
    // One undo takes back the whole chain: each pair can be bridged again.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.recorded_commands().len(), steps);
    for k in 0..3 {
        select_ids(&mut s, &[reps[k].id.to_string(), reps[k + 1].id.to_string()]);
        assert!(
            edit_item(&s, "strip.bridge").enabled,
            "pair {k} is bridgeable after the undo"
        );
    }
    // Redo puts it back.
    done(&mut s, "edit.redo", json!({}));
    select_ids(&mut s, &selection);
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
}

#[test]
fn a_chain_draws_only_the_pairs_not_bridged_yet() {
    assert_golden_endpoints();
    let tmp = Tmp::new("bridge-chain-gap");
    let mut s = shell(&tmp, "103");
    // BR1 loses its middle bridge :2-:3; :1-:2 and :3-:4 stay in group 1.
    delete_connections(&mut s, &BR1[1..2]);
    let reps = place_terminals(&mut s, &X1031);
    let selection: Vec<String> = reps.iter().map(|r| r.id.to_string()).collect();
    select_ids(&mut s, &selection);
    done(&mut s, "strip.bridge", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "connection.draw", "one missing pair is one bridge");
    assert_eq!(last.params["a"], json!({"cp": {"function": X1031[1], "idx": 3}}));
    assert_eq!(last.params["b"], json!({"cp": {"function": X1031[2], "idx": 3}}));
    assert_eq!(last.params["kind"]["bridge"]["group"], 1, "{}", last.params);
}

// ---------------------------------------------------------------------------------------------------------------
// Cycle 33 (G-PP ruling #4): terminals without a part shaft bridge at their lowest side-1 CP

/// A new strip on 103 with `n` terminals placed by symbol (no part, so no shaft): their page table rows, in
/// designation order 1 to `n`.
fn partless_strip(s: &mut ShellModel, n: usize) -> Vec<TableRep> {
    done(s, "strip.new", json!({"letter": "X"}));
    // The new strip's ID, through the strip chooser's catalogue (it has no terminals yet).
    done(s, "place.pick", json!({"symbol": TERMINAL_SYMBOL}));
    let strip = {
        let picker = s.picker().expect("the strip chooser");
        picker
            .catalogue
            .strips
            .iter()
            .find(|st| st.next_designation == "1")
            .expect("the new strip")
            .id
            .to_string()
    };
    done(s, "ui.dismiss", json!({}));
    let before: Vec<_> = tables(s).reps.iter().map(|r| r.id).collect();
    let points = free_row(s, n);
    for point in points {
        place_terminal_at(s, point, json!({"symbol": TERMINAL_SYMBOL, "strip": strip}));
    }
    let designations: Vec<String> = s
        .recorded_commands()
        .iter()
        .filter(|c| c.cmd == "device.place")
        .filter_map(|c| {
            c.params["target"]["new_terminal"]["designation"]
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    let expected: Vec<String> = (1..=n).map(|k| k.to_string()).collect();
    assert_eq!(designations, expected);
    let placed = tables(s);
    let reps: Vec<TableRep> = placed
        .reps
        .iter()
        .filter(|r| !before.contains(&r.id))
        .cloned()
        .collect();
    assert_eq!(reps.len(), n);
    reps
}

/// The function ID text of a placed terminal representation.
fn function_of(rep: &TableRep) -> String {
    assert_eq!(rep.functions.len(), 1);
    rep.functions[0].to_string()
}

#[test]
fn partless_terminals_bridge_at_their_side1_cp() {
    let tmp = Tmp::new("bridge-partless");
    let mut s = shell(&tmp, "103");
    let reps = partless_strip(&mut s, 2);
    select_ids(&mut s, &[reps[1].id.to_string(), reps[0].id.to_string()]);
    let rev = s.revision();
    done(&mut s, "strip.bridge", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "connection.draw");
    // The starter terminal's side-1 CP is index 0 (clamp a); the new strip has no bridge, so group 1.
    assert_eq!(
        last.params["a"],
        json!({"cp": {"function": function_of(&reps[0]), "idx": 0}})
    );
    assert_eq!(
        last.params["b"],
        json!({"cp": {"function": function_of(&reps[1]), "idx": 0}})
    );
    assert_eq!(
        last.params["kind"],
        json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 1}})
    );
}

/// Folio 103's BR1, BR5 and BR6 as the G-PP drafter drew them: on new strips whose terminals were placed by symbol
/// and carry no part, a chain of four, a pair and a chain of three, each in one step and its own group.
#[test]
fn folio_103_bridges_draw_on_terminals_placed_by_symbol() {
    let tmp = Tmp::new("bridge-gpp-103");
    let mut s = shell(&tmp, "103");
    let reps = partless_strip(&mut s, 9);
    let mut groups = Vec::new();
    for (range, cmd) in [
        (0..4, "connection.draw_many"),
        (4..6, "connection.draw"),
        (6..9, "connection.draw_many"),
    ] {
        let selection: Vec<String> = reps[range.clone()].iter().map(|r| r.id.to_string()).collect();
        select_ids(&mut s, &selection);
        let rev = s.revision();
        done(&mut s, "strip.bridge", json!({}));
        assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
        let last = s.recorded_commands().last().expect("the commit");
        assert_eq!(last.cmd, cmd);
        let entries: Vec<Value> = match last.params.get("connections") {
            Some(list) => list.as_array().expect("the entries").clone(),
            None => vec![last.params.clone()],
        };
        assert_eq!(entries.len(), range.len() - 1);
        let functions: Vec<String> = reps[range].iter().map(function_of).collect();
        for (k, e) in entries.iter().enumerate() {
            assert_eq!(e["a"], json!({"cp": {"function": functions[k], "idx": 0}}));
            assert_eq!(e["b"], json!({"cp": {"function": functions[k + 1], "idx": 0}}));
        }
        groups.push(entries[0]["kind"]["bridge"]["group"].as_u64().expect("a group"));
    }
    assert_eq!(groups, [1, 2, 3], "each chain takes the next free group");
}

/// Cycle 33 review finding 9 (pinned, behaviour unchanged): a part added after a part-less bridge. Two part-less
/// terminals are bridged at their side-1 CP (index 0), stored as a plug-in bridge on shaft 1; once both carry
/// PT 2.5-TWIN, whose shaft-1 entry is CP index 3, Bridge terminals is not refused as "already bridged": it draws a
/// second bridge between the shaft CPs, in a new group, and the side-1 bridge stays.
#[test]
fn a_part_added_after_a_partless_bridge_draws_a_second_bridge_on_the_shaft() {
    let tmp = Tmp::new("bridge-partless-then-part");
    let mut s = shell(&tmp, "103");
    let reps = partless_strip(&mut s, 2);
    let ids = [reps[0].id.to_string(), reps[1].id.to_string()];
    select_ids(&mut s, &ids);
    done(&mut s, "strip.bridge", json!({}));
    let first = s.recorded_commands().last().expect("the first bridge").clone();
    assert_eq!(first.params["a"]["cp"]["idx"], 0, "{}", first.params);
    assert_eq!(
        first.params["kind"],
        json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 1}})
    );
    for id in &ids {
        select_ids(&mut s, std::slice::from_ref(id));
        done(
            &mut s,
            "props.set",
            json!({"row": "device/parts", "value": "PT 2.5-TWIN"}),
        );
    }
    select_ids(&mut s, &ids);
    let rev = s.revision();
    done(&mut s, "strip.bridge", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
    let second = s.recorded_commands().last().expect("the second bridge").clone();
    assert_eq!(second.cmd, "connection.draw");
    assert_eq!(
        second.params["a"],
        json!({"cp": {"function": function_of(&reps[0]), "idx": 3}})
    );
    assert_eq!(
        second.params["b"],
        json!({"cp": {"function": function_of(&reps[1]), "idx": 3}})
    );
    assert_eq!(
        second.params["kind"],
        json!({"bridge": {"kind": "plug_in", "group": 2, "shaft": 1}}),
        "a new group: the side-1 bridge's ends are not the chain's ends"
    );
}
