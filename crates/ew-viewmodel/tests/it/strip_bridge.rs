// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the view model: `strip.bridge` (Edit ▸ Bridge terminals)
//! on the sample's -X1021 of folio 102, the bridge group the action derives, one undo step per bridge, and the refusals
//! of a selection that is not terminals of one strip; and the chain over three or more terminals in one undo
//! step and the side-1 end of terminals without a bridge shaft. Every test works on a writable temporary copy of
//! the sample.

use crate::common;

use std::path::Path;

use common::{SAMPLE, Tmp, actual_size, cursor, done, refused, select_ids, tables};
use ew_api::Session;
use ew_api::output::TableRep;
use ew_viewmodel::actions::{ActionItem, Menu, menu_model};
use ew_viewmodel::canvas::{HOVER_SLOP_PT, pick};
use ew_viewmodel::shell::ShellModel;
use serde_json::{Value, json};

/// The symbol the sample's terminals use.
const TERMINAL_SYMBOL: &str = "ew.iec.terminal@qet10";

/// What the bridge rule works on in the sample, read from it ([`endpoints`]).
struct Endpoints {
    /// -X1021's terminal functions 1 to 4, in sequence order.
    x: Vec<String>,
    /// -X1021's strip device.
    strip: String,
    /// BR1's three connections, over -X1021 1–2, 2–3 and 3–4.
    br1: Vec<String>,
}

/// The endpoints the bridge rule relies on, from folio 102 shown in `s` (the functions by the labels the page prints)
/// and the sample itself: every function's device is a terminal of -X1021 with the expected sequence, of a part with
/// no bridge shaft (EW-GEN-TRM-1), so a bridge ends on its side-1 CP, index 0; BR1's three connections are the sample's
/// bridge over them. A difference stops the test before its own assertions.
fn endpoints(s: &ShellModel) -> Endpoints {
    let t = tables(s);
    let x: Vec<String> = (1..=4)
        .map(|k| {
            t.cps
                .iter()
                .find(|c| c.label == format!("-X1021:{k}/a"))
                .unwrap_or_else(|| panic!("-X1021:{k} on folio 102"))
                .function
                .to_string()
        })
        .collect();
    let (session, _) =
        Session::open_read_only(Path::new(SAMPLE), "test").expect("the sample opens read-only");
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
        assert!(
            copy.item.cp_pattern.iter().all(|cp| {
                serde_json::to_value(&cp.clamp).map_or(true, |clamp| clamp.get("shaft").is_none())
            }),
            "{function}'s part has no bridge shaft"
        );
        (
            term.seq,
            term.designation.as_str().to_owned(),
            term.strip.as_uuid().to_string(),
        )
    };
    let strip = terminal(&x[0]).2;
    for (k, function) in x.iter().enumerate() {
        let (seq, designation, of) = terminal(function);
        assert_eq!(seq, k as u32 + 1, "{function}'s sequence");
        assert_eq!(designation, (k + 1).to_string());
        assert_eq!(of, strip);
    }
    let br1: Vec<String> = (0..3)
        .map(|k| {
            let (id, c) = q
                .connections()
                .iter()
                .find(|(_, c)| {
                    serde_json::to_value(&c.a).ok() == Some(json!({"cp": [x[k], 0]}))
                        && serde_json::to_value(&c.b).ok() == Some(json!({"cp": [x[k + 1], 0]}))
                })
                .unwrap_or_else(|| panic!("the bridge over -X1021 {} and {}", k + 1, k + 2));
            assert_eq!(
                serde_json::to_value(&c.kind).expect("the kind's JSON"),
                json!({"bridge": {"kind": "plug_in", "group": 1, "shaft": 1}})
            );
            id.as_uuid().to_string()
        })
        .collect();
    Endpoints { x, strip, br1 }
}

/// The representations of `functions` on the page shown, in the same order.
fn reps_of(s: &ShellModel, functions: &[String]) -> Vec<TableRep> {
    let t = tables(s);
    functions
        .iter()
        .map(|function| {
            t.reps
                .iter()
                .find(|r| r.functions.len() == 1 && r.functions[0].to_string() == *function)
                .unwrap_or_else(|| panic!("no representation of {function}"))
                .clone()
        })
        .collect()
}

/// Deletes the connections with these ID texts (`edit.delete`).
fn delete_connections(s: &mut ShellModel, connections: &[String]) {
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

/// The `strip.bridge` item of the app menu (Draw, Connect).
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

/// The largest group among the bridge connections of the strip `strip` in the project at `dir`.
fn largest_bridge_group(dir: &Path, strip: &str) -> u64 {
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
            == Some(strip)
        {
            max = max.max(group);
        }
    }
    max
}

/// BR1 deleted, its four terminals bridged again pair by pair.
fn three_bridges(tmp: &Tmp) -> (ShellModel, Vec<TableRep>, Vec<u64>) {
    let mut s = actual_size(tmp, "102");
    let e = endpoints(&s);
    delete_connections(&mut s, &e.br1);
    let reps = reps_of(&s, &e.x);
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
fn three_bridges_join_four_terminals_in_one_group() {
    let tmp = Tmp::new("bridge-pi10");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    delete_connections(&mut s, &e.br1);
    let reps = reps_of(&s, &e.x);
    // The remaining -X1021 bridge (BR2) is read from the saved modified copy, not from the pristine sample.
    done(&mut s, "file.save", json!({}));
    let remaining = largest_bridge_group(&tmp.sample_copy(), &e.strip);
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
        assert_eq!(last.params["a"], json!({"cp": {"function": e.x[k], "idx": 0}}));
        assert_eq!(
            last.params["b"],
            json!({"cp": {"function": e.x[k + 1], "idx": 0}})
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
    // BR2 remains on -X1021 in group 2 (the sample's script), so the first new bridge takes group 3.
    assert_eq!(group0, 3, "BR2 remains in group 2");
}

// ---------------------------------------------------------------------------------------------------------------
// PI10: every bridge is one undo step

#[test]
fn each_bridge_is_one_undo_step() {
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
    let tmp = Tmp::new("bridge-extend");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    delete_connections(&mut s, &e.br1[2..]);
    let reps = reps_of(&s, &e.x[2..]);
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    done(&mut s, "strip.bridge", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.params["kind"]["bridge"]["group"], 1, "{}", last.params);
    // One bridge counts its two terminals.
    assert_eq!(
        s.toast().expect("the bridge toast").text,
        "Added 1 bridge between 2 terminals."
    );
}

// ---------------------------------------------------------------------------------------------------------------
// PI10: bridging twice is refused

#[test]
fn bridging_twice_is_refused() {
    // The bridge drawn first counts.
    let tmp = Tmp::new("bridge-twice");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    delete_connections(&mut s, &e.br1);
    let reps = reps_of(&s, &e.x[..2]);
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    done(&mut s, "strip.bridge", json!({}));
    let rev = s.revision();
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
    assert_eq!(s.revision(), rev, "a refused bridge commits nothing");
    // The sample's own BR1 1–2 counts.
    let tmp = Tmp::new("bridge-twice-sample");
    let mut s = actual_size(&tmp, "102");
    let reps = reps_of(&s, &e.x[..2]);
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-exists");
}

// ---------------------------------------------------------------------------------------------------------------
// The selection must be two or more terminals of one strip

#[test]
fn the_selection_must_be_terminals_of_one_strip() {
    let tmp = Tmp::new("bridge-selection");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    let reps = reps_of(&s, &e.x[..1]);
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
        .expect("an interruption point on 102")
        .id
        .to_string();
    select_ids(&mut s, &[reps[0].id.to_string(), other]);
    assert_eq!(
        refused(&mut s, "strip.bridge", json!({})),
        "reason-bridge-selection"
    );
    // Two terminals of different strips: their own reason.
    done(
        &mut s,
        "select.set",
        json!({"refs": ["tag:-X1021:1", "tag:-X1011:L1"]}),
    );
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
// Four terminals are one chain in one undo step

#[test]
fn four_terminals_draw_the_chain_in_one_undo_step() {
    let tmp = Tmp::new("bridge-chain");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    delete_connections(&mut s, &e.br1);
    let reps = reps_of(&s, &e.x);
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
    // The outcome is counted in the toast.
    assert_eq!(
        s.toast().expect("the bridge toast").text,
        "Added 3 bridges between 4 terminals."
    );
    for (k, c) in connections.iter().enumerate() {
        assert_eq!(c["a"], json!({"cp": {"function": e.x[k], "idx": 0}}));
        assert_eq!(c["b"], json!({"cp": {"function": e.x[k + 1], "idx": 0}}));
        // BR2 remains on -X1021 in group 2, so the chain takes group 3.
        assert_eq!(
            c["kind"],
            json!({"bridge": {"kind": "plug_in", "group": 3, "shaft": 1}})
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
    let tmp = Tmp::new("bridge-chain-gap");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    // BR1 loses its middle bridge :2-:3; :1-:2 and :3-:4 stay in group 1.
    delete_connections(&mut s, &e.br1[1..2]);
    let reps = reps_of(&s, &e.x);
    let selection: Vec<String> = reps.iter().map(|r| r.id.to_string()).collect();
    select_ids(&mut s, &selection);
    done(&mut s, "strip.bridge", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "connection.draw", "one missing pair is one bridge");
    assert_eq!(last.params["a"], json!({"cp": {"function": e.x[1], "idx": 0}}));
    assert_eq!(last.params["b"], json!({"cp": {"function": e.x[2], "idx": 0}}));
    assert_eq!(last.params["kind"]["bridge"]["group"], 1, "{}", last.params);
    // One bridge counts its two terminals.
    assert_eq!(
        s.toast().expect("the bridge toast").text,
        "Added 1 bridge between 2 terminals."
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Terminals without a part shaft bridge at their lowest side-1 CP

/// A new strip on 101 with `n` terminals placed by symbol (no part, so no shaft): their page table rows, in
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
    let mut s = actual_size(&tmp, "101");
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

/// The bridges the G-PP drafter drew on folio 103 of the old reference project, on a new strip of 101 whose terminals
/// were placed by symbol and carry no part: a chain of four, a pair and a chain of three, each in one step and its own
/// group.
#[test]
fn bridges_draw_on_terminals_placed_by_symbol() {
    let tmp = Tmp::new("bridge-gpp-103");
    let mut s = actual_size(&tmp, "101");
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

// ---------------------------------------------------------------------------------------------------------------
// Selection by records (D9), the shaft rule and Remove bridge (D11)

/// The Terminal strips row keys of terminals `designations` of strip `strip` (the navigator open in table mode).
fn strip_rows(s: &mut ShellModel, strip: &str, designations: &[&str]) -> Vec<String> {
    done(s, "navigator.show", json!({"kind": "strips"}));
    done(s, "navigator.table", json!({"kind": "strips", "on": true}));
    let n = s
        .nav(ew_viewmodel::navigators::NavKind::Strips)
        .expect("the navigator");
    let t = n.table().expect("its table");
    designations
        .iter()
        .map(|d| {
            t.nodes
                .iter()
                .find(|x| {
                    x.label == *d
                        && x.parent
                            .and_then(|p| t.nodes.get(p as usize))
                            .is_some_and(|p| p.label == strip)
                })
                .unwrap_or_else(|| panic!("no row {strip}:{d}"))
                .key
                .clone()
        })
        .collect()
}

/// SE10: Bridge terminals takes the terminals selected by click (representations), by
/// `select.set` with `tag:` references (devices), by navigator marks (devices) and by functions, with one result each
/// time; terminals of two strips refuse `reason-bridge-strips`.
#[test]
fn bridge_terminals_by_reps_tags_marks_and_functions() {
    let tmp = Tmp::new("bridge-se10");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    delete_connections(&mut s, &e.br1);
    let reps = reps_of(&s, &e.x[..2]);
    let mut results: Vec<(&str, String, Value)> = Vec::new();
    let mut bridge = |s: &mut ShellModel, how: &'static str| {
        let rev = s.revision();
        done(s, "strip.bridge", json!({}));
        assert_eq!(s.revision(), rev.map(|r| r + 1), "{how}: one undo step");
        let last = s.recorded_commands().last().expect("the bridge").clone();
        results.push((how, last.cmd, last.params));
        done(s, "edit.undo", json!({}));
    };
    select_ids(&mut s, &[reps[0].id.to_string(), reps[1].id.to_string()]);
    bridge(&mut s, "representations");
    done(
        &mut s,
        "select.set",
        json!({"refs": ["tag:-X1021:1", "tag:-X1021:2"]}),
    );
    bridge(&mut s, "tags");
    select_ids(&mut s, &[e.x[0].clone(), e.x[1].clone()]);
    bridge(&mut s, "functions");
    let keys = strip_rows(&mut s, "-X1021", &["1", "2"]);
    done(
        &mut s,
        "navigator.select",
        json!({"kind": "strips", "keys": keys}),
    );
    bridge(&mut s, "navigator marks");
    let (_, cmd, params) = &results[0];
    assert_eq!(cmd, "connection.draw");
    assert_eq!(params["a"], json!({"cp": {"function": e.x[0], "idx": 0}}));
    assert_eq!(params["b"], json!({"cp": {"function": e.x[1], "idx": 0}}));
    for (how, c, p) in &results[1..] {
        assert_eq!((c, p), (cmd, params), "{how}");
    }
    // Two strips.
    done(
        &mut s,
        "select.set",
        json!({"refs": ["tag:-X1021:1", "tag:-X1011:L1"]}),
    );
    assert_eq!(refused(&mut s, "strip.bridge", json!({})), "reason-bridge-strips");
}

/// SE11: two terminals whose function definition has no side-1 CP and that carry no part with a shaft-1
/// entry refuse `reason-bridge-no-shaft` (the shaft half, part-placed terminals ending on the shaft CPs, is PI10).
#[test]
fn terminals_without_a_shaft_or_a_side1_cp_refuse() {
    // A function definition of the sample with connection points and none of them on side 1.
    let def = {
        let (session, _) =
            Session::open_read_only(Path::new(SAMPLE), "test").expect("the sample opens read-only");
        let q = session.query();
        q.fndefs()
            .iter()
            .find(|(_, d)| {
                !d.item.cps.is_empty()
                    && d.item
                        .cps
                        .iter()
                        .all(|cp| serde_json::to_value(&cp.side).map_or(true, |side| side != "side1"))
            })
            .map(|(id, _)| id.as_uuid().to_string())
            .expect("a definition without side-1 CPs")
    };
    let tmp = Tmp::new("bridge-se11");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    done(
        &mut s,
        "command.run",
        json!({"cmd": "terminal.add", "params": {"strip": e.strip, "fndef": def,
            "terminals": [{"designation": "N1"}, {"designation": "N2"}]}}),
    );
    let keys = strip_rows(&mut s, "-X1021", &["N1", "N2"]);
    done(
        &mut s,
        "navigator.select",
        json!({"kind": "strips", "keys": keys}),
    );
    let rev = s.revision();
    assert_eq!(
        refused(&mut s, "strip.bridge", json!({})),
        "reason-bridge-no-shaft"
    );
    assert_eq!(s.revision(), rev);
}

/// SE13: Remove bridge on the sample's BR1 (-X1021 1–2, 2–3, 3–4): all four terminals remove the whole
/// group in one `edit.delete` of three connections and one undo step; 3 and 4 remove the end link; 2 and 3 refuse
/// `reason-unbridge-split`; terminals no bridge joins refuse `reason-unbridge-none`.
#[test]
fn se13_remove_bridge() {
    let tmp = Tmp::new("bridge-se13");
    let mut s = actual_size(&tmp, "102");
    let e = endpoints(&s);
    done(&mut s, "navigator.show", json!({"kind": "strips"}));
    let select = |s: &mut ShellModel, terminals: &[&str]| {
        let refs: Vec<String> = terminals.iter().map(|t| format!("tag:-X1021:{t}")).collect();
        done(s, "select.set", json!({ "refs": refs }));
    };
    let targets = |s: &ShellModel| -> Vec<String> {
        let last = s.recorded_commands().last().expect("the removal");
        assert_eq!(last.cmd, "edit.delete");
        let mut ids: Vec<String> = last.params["targets"]
            .as_array()
            .expect("targets")
            .iter()
            .map(|t| {
                assert_eq!(t["kind"], "connection");
                t["id"].as_str().expect("an ID").to_owned()
            })
            .collect();
        ids.sort();
        ids
    };
    // The whole group.
    select(&mut s, &["1", "2", "3", "4"]);
    let rev = s.revision();
    done(&mut s, "strip.unbridge", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
    let mut br1 = e.br1.clone();
    br1.sort();
    assert_eq!(targets(&s), br1);
    assert!(
        s.describe().contains("toast \"Removed 3 bridges.\" undo"),
        "the toast"
    );
    done(&mut s, "edit.undo", json!({}));
    // The end link 3–4.
    select(&mut s, &["3", "4"]);
    done(&mut s, "strip.unbridge", json!({}));
    assert_eq!(targets(&s), [e.br1[2].clone()]);

    done(&mut s, "edit.undo", json!({}));
    // The middle link 2–3 would leave 1–2 and 3–4 apart.
    select(&mut s, &["2", "3"]);
    let rev = s.revision();
    assert_eq!(
        refused(&mut s, "strip.unbridge", json!({})),
        "reason-unbridge-split"
    );
    // No bridge joins 4 and 5 (BR1 and BR2).
    select(&mut s, &["4", "5"]);
    assert_eq!(
        refused(&mut s, "strip.unbridge", json!({})),
        "reason-unbridge-none"
    );
    assert_eq!(s.revision(), rev, "a refusal commits nothing");
}
