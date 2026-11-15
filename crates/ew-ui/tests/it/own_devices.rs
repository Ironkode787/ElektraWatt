// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Devices with ports of one's own, as a person met them on a new project (the persona run m3-p4): a station placed
//! on a bus-topology page stands there with its bus ports. The window's own path: clicks and keys at 1920 × 1080.

use crate::own_parts::{
    cp_labels, cursor_k, make_head_station, new_project, page_texts, place_box, press_cp, records,
};
use crate::persona_round::key;
use crate::support::Tmp;

use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::Driver;
use serde_json::json;

/// A new project with a bus-topology page shown.
fn bus_page(d: &mut Driver) {
    let page = json!({"folio": "2", "type": "bus_topology", "title": "Network"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
}

/// The rows under the Bus navigator's group `group` (its key segment): their keys and labels.
fn bus_group(d: &mut Driver, group: &str) -> Vec<(String, String)> {
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "bus"}));
    d.settle();
    let rows = d.shell_mut().shown_rows(NavKind::Bus);
    rows.iter()
        .filter(|r| r.parent.as_deref().is_some_and(|p| p.ends_with(group)))
        .map(|r| (r.key.clone(), r.label.clone()))
        .collect()
}

/// Item 2: the Bus panel lists what is not drawn yet. The starter CPU drawn on a schematic page has its DP and X1 in
/// its part and on no page: the group Ports not drawn yet lists both, each row's menu offers Place port; Enter on the
/// DP row on a bus-topology page draws its box, after which the row is gone and the port is listed without a net.
#[test]
fn the_bus_panel_lists_the_ports_not_drawn_yet_and_places_them() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (10, 10), (20, 16));
    let tag = cpu[0].split(':').next().unwrap().to_owned();
    let undrawn = bus_group(&mut d, "ports:undrawn");
    let labels: Vec<&str> = undrawn.iter().map(|(_, l)| l.as_str()).collect();
    assert_eq!(labels, [format!("{tag} DP"), format!("{tag} X1")], "{undrawn:?}");
    // Its menu offers Place port, which waits for a bus-topology page and says so.
    let node = d
        .shell_mut()
        .nav(NavKind::Bus)
        .and_then(|n| n.node_of(&undrawn[0].0).map(|(_, n)| n.clone()))
        .expect("the DP row");
    let menu = ew_ui::ew_viewmodel::command_bar::navigator_row_menu(d.shell_mut(), NavKind::Bus, &node);
    let place = menu.iter().find(|i| i.label == "Place port DP").expect("Place port DP");
    assert!(!place.enabled && place.reason.is_some(), "not on a schematic page: {place:?}");
    bus_page(&mut d);
    let open = json!({"kind": "bus", "key": undrawn[0].0});
    assert_eq!(d.shell_mut().dispatch("navigator.open", &open), Outcome::Done);
    for k in [(10, 10), (18, 16)] {
        cursor_k(&mut d, k);
        let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
        d.settle();
    }
    d.input(&key("Escape"));
    d.settle();
    let left: Vec<String> = bus_group(&mut d, "ports:undrawn").into_iter().map(|(_, l)| l).collect();
    assert_eq!(left, [format!("{tag} X1")], "the DP is drawn");
    let loose: Vec<String> = bus_group(&mut d, "ports:none").into_iter().map(|(_, l)| l).collect();
    assert_eq!(loose, [format!("{tag} DP")], "and listed without a net");
}

/// Item 4: the address is typed where a person looks, with its form said. The Bus table's Address cell of the CPU's
/// X1 says under its editor that a PROFINET address is a name and an IP address, with an example, and its Termination
/// cell what it takes; typed so, the box prints it. The X1 box selected on the page shows its function first in
/// Properties, with the port's Address as a row whose editor says the same form.
#[test]
fn the_address_of_a_port_says_its_form_in_the_table_and_in_properties() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    bus_page(&mut d);
    place_box(&mut d, "EW-GEN-CPU-2", (10, 10), (20, 16));
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "bus"}));
    let _ = d
        .shell_mut()
        .dispatch("navigator.table", &json!({"kind": "bus", "on": true}));
    d.settle();
    let row = d
        .shell_mut()
        .shown_rows(NavKind::Bus)
        .into_iter()
        .find(|r| r.role == NavRole::BusPort.word() && r.label.ends_with(" X1"))
        .expect("the CPU's X1 row")
        .key;
    let edit = json!({"kind": "bus", "row": row, "column": "address"});
    assert_eq!(d.shell_mut().dispatch("table.edit", &edit), Outcome::Done);
    d.settle();
    let hint = d.shell_mut().table_edit_hint(NavKind::Bus).unwrap_or_default();
    assert!(
        hint.contains("device name") && hint.contains("plc1 192.168.0.1"),
        "the Address cell says its form: {hint:?}"
    );
    d.input(&key("Escape"));
    let edit = json!({"kind": "bus", "row": row, "column": "termination"});
    let _ = d.shell_mut().dispatch("table.edit", &edit);
    d.settle();
    let hint = d.shell_mut().table_edit_hint(NavKind::Bus).unwrap_or_default();
    assert!(hint.starts_with("on or off"), "the Termination cell's words: {hint:?}");
    d.input(&key("Escape"));
    d.settle();
    let set = json!({"kind": "bus", "column": "Address", "value": "plc1 192.168.0.1", "rows": [row]});
    assert_eq!(d.shell_mut().dispatch("table.set", &set), Outcome::Done);
    d.settle();
    assert!(page_texts(&mut d).iter().any(|t| t == "Node plc1 192.168.0.1"));
    // The X1 box, selected: its function's section comes first, with the Address row and its form.
    let all = records(&mut d);
    let port = all
        .iter()
        .find(|v| v["bus_port"]["interface"] == "X1")
        .and_then(|v| v["id"].as_str())
        .expect("the X1 function")
        .to_owned();
    let rep = all
        .iter()
        .find(|v| v["functions"][0] == port.as_str())
        .and_then(|v| v["id"].as_str())
        .expect("the X1 box")
        .to_owned();
    assert_eq!(d.shell_mut().dispatch("select.set", &json!({"ids": [rep]})), Outcome::Done);
    d.settle();
    let first = d
        .shell_mut()
        .properties()
        .sheet()
        .and_then(|s| s.sections.first().map(|s| s.kind.clone()));
    assert_eq!(first.as_deref(), Some("function"), "the port's section first");
    let row = json!({"row": "function/bus_port.address"});
    assert_eq!(d.shell_mut().dispatch("props.edit", &row), Outcome::Done);
    d.settle();
    let hint = d.shell_mut().property_edit_hint().unwrap_or_default();
    assert!(hint.contains("plc1 192.168.0.1"), "Properties says the form: {hint:?}");
}

/// Item 1: the starter CPU and a head station of one's own (such as an IM 152), each placed from the device browser on
/// a bus-topology page, stand there with their bus ports and without their supply pins: the CPU as its interfaces DP
/// and X1, the head station as its DP, each socket named by its interface. A Wire between the two DP ports makes one
/// PROFIBUS net of both.
#[test]
fn a_station_placed_on_a_bus_page_stands_with_its_bus_ports() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_head_station(&mut d, "HEAD-DP");
    bus_page(&mut d);
    let cpu = place_box(&mut d, "EW-GEN-CPU-2", (10, 10), (20, 16));
    assert_eq!(cpu.len(), 2, "the CPU's DP and X1, a socket each: {cpu:?}");
    let head = place_box(&mut d, "HEAD-DP", (50, 10), (60, 16));
    assert_eq!(head.len(), 1, "the head station's DP: {head:?}");
    let supply = cp_labels(&mut d)
        .into_iter()
        .filter(|l| l.ends_with(":L+") || l.ends_with(":M"))
        .count();
    assert_eq!(supply, 0, "no supply pin on the bus page");
    let texts = page_texts(&mut d);
    for interface in ["DP", "X1"] {
        assert!(texts.iter().any(|t| t == interface), "{interface}: {texts:?}");
    }
    // The DP socket of the CPU is its DP port's first pin.
    let cpu_dp = cpu.iter().find(|l| l.ends_with(":A")).expect("the CPU's DP socket");
    d.input(&key("W"));
    d.settle();
    press_cp(&mut d, cpu_dp);
    press_cp(&mut d, &head[0]);
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "bus"}));
    d.settle();
    let rows = d.shell_mut().shown_rows(NavKind::Bus);
    let nets: Vec<&str> = rows
        .iter()
        .filter(|r| r.role == NavRole::Net.word())
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(nets, ["PROFIBUS DP"], "one net: {rows:?}");
    let members = rows
        .iter()
        .filter(|r| r.role == NavRole::BusPort.word() && r.key.starts_with("nets:logical/"))
        .filter(|r| r.label.ends_with(" DP"))
        .count();
    assert_eq!(members, 2, "the CPU's and the head station's DP: {rows:?}");
    // The CPU's supply pins are its place on a schematic page: its unplaced black box, dropped there from its row,
    // is drawn by the black box tool with L+ and M, a box of the same device.
    let all = records(&mut d);
    let part = all
        .iter()
        .find(|v| v["part_number"] == "EW-GEN-CPU-2")
        .and_then(|v| v["id"].as_str())
        .expect("the CPU's part copy")
        .to_owned();
    let device = all
        .iter()
        .find(|v| v["parts"][0]["part"]["copy"] == part.as_str())
        .and_then(|v| v["id"].as_str())
        .expect("the CPU")
        .to_owned();
    assert_eq!(
        d.shell_mut().dispatch("page.goto", &json!({"folio": "1"})),
        Outcome::Done
    );
    d.settle();
    let before = cp_labels(&mut d);
    assert_eq!(
        d.shell_mut()
            .dispatch("place.pick", &json!({"function": device})),
        Outcome::Done,
        "the black box tool takes the unplaced supply box"
    );
    for k in [(10, 10), (20, 16)] {
        cursor_k(&mut d, k);
        let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
        d.settle();
    }
    d.input(&key("Escape"));
    d.settle();
    let pins: Vec<String> = cp_labels(&mut d)
        .into_iter()
        .filter(|l| !before.contains(l))
        .collect();
    // The tag follows the main function's page (page-based tags): the bus page's DP box takes it too.
    let tag = pins.first().and_then(|l| l.split(':').next()).unwrap_or_default().to_owned();
    assert_eq!(pins, [format!("{tag}:L+"), format!("{tag}:M")], "the supply pins");
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "2"}));
    d.settle();
    assert!(
        cp_labels(&mut d).contains(&format!("{tag}:A")),
        "the same CPU's DP on the bus page"
    );
}
