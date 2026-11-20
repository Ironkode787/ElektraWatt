// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Devices with ports of one's own, as a person met them on a new project (the persona run m3-p4): a station placed
//! on a bus-topology page stands there with its bus ports. The window's own path: clicks and keys at 1920 × 1080.

use crate::own_parts::{
    cp_labels, cursor_k, edit_part, fill, make_card, make_head_station, make_port_device, new_project,
    page_texts, place_box, press, press_cp, records,
};
use crate::relays_paste_and_ip_wires::{key, typed};
use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input};
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
    let menu = ew_viewmodel::command_bar::navigator_row_menu(d.shell_mut(), NavKind::Bus, &node);
    let place = menu
        .iter()
        .find(|i| i.label == "Place port DP")
        .expect("Place port DP");
    assert!(
        !place.enabled && place.reason.is_some(),
        "not on a schematic page: {place:?}"
    );
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
    let left: Vec<String> = bus_group(&mut d, "ports:undrawn")
        .into_iter()
        .map(|(_, l)| l)
        .collect();
    assert_eq!(left, [format!("{tag} X1")], "the DP is drawn");
    let loose: Vec<String> = bus_group(&mut d, "ports:none")
        .into_iter()
        .map(|(_, l)| l)
        .collect();
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
    assert!(
        hint.starts_with("on or off"),
        "the Termination cell's words: {hint:?}"
    );
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
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({"ids": [rep]})),
        Outcome::Done
    );
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
    assert!(
        hint.contains("plc1 192.168.0.1"),
        "Properties says the form: {hint:?}"
    );
}

/// Item 5: the card of a corrected part speaks words. A managed switch of one's own with eight ports, drawn and wired
/// at P8, is corrected to six: the card lists "Interface X1: 8 ports → 6 ports" and the parts list row's changed
/// column, no JSON or CSV, and names the bus line at P8 as losing its socket. After Apply the drawn box shows the
/// sockets P1 … P6, as the editor's note says.
#[test]
fn the_card_of_a_corrected_part_speaks_words_and_names_a_line_that_loses_its_socket() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_port_device(&mut d, "SW-8", "Managed switch 8 ports", "8");
    let parts_list = json!({"kind": "parts_summary", "folio": "901"});
    assert_eq!(
        d.shell_mut().dispatch("report_block.new", &parts_list),
        Outcome::Done
    );
    bus_page(&mut d);
    let switch = place_box(&mut d, "SW-8", (40, 15), (64, 22));
    let drive = place_box(&mut d, "EW-GEN-PND-1", (40, 45), (55, 52));
    d.input(&key("W"));
    d.settle();
    press_cp(&mut d, &switch[7]);
    press_cp(&mut d, &drive[0]);
    d.input(&key("Escape"));
    d.input(&key("Escape"));
    d.settle();
    edit_part(&mut d, "SW-8");
    press(&mut d, "Description");
    d.input(&key("Ctrl+A"));
    d.input(&typed("Managed switch 6 ports"));
    press(&mut d, "Ports 1");
    d.input(&key("Ctrl+A"));
    d.input(&typed("6"));
    d.settle();
    press(&mut d, "Save");
    let card = match d.shell_mut().properties().card().map(|c| c.origin.clone()) {
        Some(ew_viewmodel::properties::CardOrigin::Part(p)) => *p,
        other => panic!("the part card: {other:?}"),
    };
    assert!(
        card.fields.contains(&(
            "Interface X1".to_owned(),
            "8 ports".to_owned(),
            "6 ports".to_owned()
        )),
        "{:?}",
        card.fields
    );
    let all: Vec<String> = card
        .fields
        .iter()
        .map(|(m, a, b)| format!("{m} {a} {b}"))
        .chain(card.changed.clone())
        .chain(card.removed.clone())
        .chain(card.added.clone())
        .collect();
    assert!(
        all.iter().all(|t| !t.contains('{') && !t.contains(",1,")),
        "no JSON and no CSV: {all:?}"
    );
    assert!(
        card.changed
            .iter()
            .any(|c| c.contains("8 ports → Managed switch 6 ports")),
        "the parts list row's changed column: {card:?}"
    );
    assert_eq!(card.lost, [switch[7].clone()], "the line at P8 loses its socket");
    assert_eq!(d.shell_mut().dispatch("preview.apply", &json!({})), Outcome::Done);
    d.settle();
    let tag = switch[0].split(':').next().unwrap().to_owned();
    let drawn: Vec<String> = cp_labels(&mut d)
        .into_iter()
        .filter(|l| l.starts_with(&format!("{tag}:")))
        .collect();
    let six: Vec<String> = (1..=6).map(|n| format!("{tag}:P{n}")).collect();
    assert_eq!(
        drawn, six,
        "the drawn box follows: the sockets the part still has"
    );
}

/// Item 6: the New part dialog holds still. The fields stand where they stood after a person types into the first one
/// (the persona's next clicks hit other fields when the dialog grew and moved up), and an interface row just added
/// stands whole above the footer, with Add interface under it.
#[test]
fn the_new_part_dialog_holds_still_while_one_types_and_adds_rows() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&key("Shift+I"));
    d.settle();
    press(&mut d, "New part…");
    let at = |d: &mut Driver, name: &str| {
        d.labels()
            .into_iter()
            .rfind(|(l, _)| l == name)
            .unwrap_or_else(|| panic!("no {name}"))
            .1
    };
    let before = (at(&mut d, "Part number"), at(&mut d, "Save"));
    fill(&mut d, "Manufacturer", "Acme");
    fill(&mut d, "Part number", "SW-8");
    let after = (at(&mut d, "Part number"), at(&mut d, "Save"));
    assert_eq!(before, after, "the fields and the footer keep their place");
    for _ in 0..4 {
        press(&mut d, "Add interface");
    }
    let (row, add, save) = (
        at(&mut d, "Ports 4"),
        at(&mut d, "Add interface"),
        at(&mut d, "Save"),
    );
    assert!(
        row.1 < add.1 && add.1 < save.1,
        "the new row and Add interface above the footer: {row:?} {add:?} {save:?}"
    );
    assert_eq!(save, before.1, "the footer stays where it was");
}

/// Item 7: the small words. Allowed slots says what it takes, with an example; New page's line under Type says what
/// the chosen type is for (Bus topology: the network), not where lists are made; the command search finds the Bus
/// panel and New page by "network", "bus", "PROFINET" and "PROFIBUS".
#[test]
fn the_small_words_say_what_slots_and_a_bus_page_are_and_the_search_finds_the_network() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&key("Shift+I"));
    d.settle();
    press(&mut d, "New part…");
    press(&mut d, "PLC role");
    press(&mut d, "Module");
    let texts = d.texts();
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("The slots the module may stand in") && t.contains("1-8, 10")),
        "Allowed slots says what it takes: {texts:?}"
    );
    d.input(&key("Escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("ui.dismiss", &json!({}));
    press(&mut d, "New page");
    d.shell_mut().set_form_field("type", "bus_topology");
    d.input(&Input::Hover { at: (900, 600) });
    d.settle();
    let texts = d.texts();
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("The network: the devices as boxes")),
        "the line under Type: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t.starts_with("Lists (parts")),
        "no word of lists"
    );
    d.input(&key("Escape"));
    d.settle();
    for word in ["network", "bus", "PROFINET", "PROFIBUS"] {
        if d.shell_mut().palette().is_none() {
            let _ = d.shell_mut().dispatch("tools.palette", &json!({}));
        }
        d.shell_mut().set_palette_query(word);
        let ids: Vec<String> = d
            .shell_mut()
            .palette_entries()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert!(
            ids.iter().any(|i| i == "bus") && ids.iter().any(|i| i == "page.new"),
            "{word}: {ids:?}"
        );
    }
}

/// The labels of the PLC panel's rows under Cards without a station.
fn waiting(d: &mut Driver) -> Vec<String> {
    let _ = d.shell_mut().dispatch("navigator.show", &json!({"kind": "plc"}));
    d.settle();
    d.shell_mut()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .filter(|r| r.parent.as_deref().is_some_and(|p| p.ends_with("cards:none")))
        .map(|r| r.label)
        .collect()
}

/// Item 3: a card goes behind a station where a person puts it. In a project with the starter CPU and a head station
/// of one's own, a card placed beside the head station is attached to it; one placed on a page without a station
/// waits, and its toast says where it gets one. Its row in the PLC panel dragged onto the head station's row attaches
/// it.
#[test]
fn a_card_goes_behind_the_station_it_is_placed_beside_or_dragged_onto() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    make_head_station(&mut d, "HEAD-DP");
    make_card(&mut d, "DI8-24", "1-32");
    place_box(&mut d, "EW-GEN-CPU-2", (10, 10), (20, 16));
    let head = place_box(&mut d, "HEAD-DP", (60, 10), (70, 16));
    let head = head[0].split(':').next().unwrap().to_owned();
    let first = place_box(&mut d, "DI8-24", (74, 10), (88, 16));
    let first = first[0].split(':').next().unwrap().to_owned();
    assert!(
        waiting(&mut d).is_empty(),
        "the card beside the head station is attached"
    );
    let rows = d.shell_mut().shown_rows(NavKind::Plc);
    let key_of = |tag: &str| {
        rows.iter()
            .find(|r| r.label.starts_with(tag))
            .map(|r| r.key.clone())
    };
    let parent = rows
        .iter()
        .find(|r| r.label.starts_with(&first))
        .and_then(|r| r.parent.clone());
    assert_eq!(parent, key_of(&head), "listed under the head station");
    // On a page without a station the card waits, and says where it gets one.
    let page = json!({"folio": "2", "type": "multi_line", "title": "Cards"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    d.settle();
    // The device browser's part, placed as its Place button does (the PLC panel has the keys now).
    let before = cp_labels(&mut d);
    let pick = json!({"part": "DI8-24"});
    assert_eq!(d.shell_mut().dispatch("place.pick", &pick), Outcome::Done);
    for k in [(10, 10), (24, 16)] {
        cursor_k(&mut d, k);
        let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
        d.settle();
    }
    let second = cp_labels(&mut d)
        .into_iter()
        .find(|l| !before.contains(l))
        .expect("the card on page 2");
    let second = second.split(':').next().unwrap().to_owned();
    assert!(
        waiting(&mut d).iter().any(|l| l.starts_with(&second)),
        "{second} waits"
    );
    // F18 of the rc.1 test: with two stations and none on its page, a window asks which station takes it; Cancel
    // leaves it waiting.
    assert!(
        matches!(
            d.shell_mut().dialog(),
            Some(ew_viewmodel::shell::Dialog::ChooseStation { .. })
        ),
        "the station chooser"
    );
    assert_eq!(d.shell_mut().dispatch("ui.dismiss", &json!({})), Outcome::Done);
    d.settle();
    // The PLC panel: the waiting card's row dragged onto the head station's row.
    // The PLC panel on show (Rail ▸ PLC toggles it).
    let find = |d: &mut Driver, tag: &str| {
        // A frame painted after the dispatches, so that the labels are the window's now.
        d.input(&Input::Hover { at: (900, 600) });
        d.settle();
        d.labels().into_iter().find(|(l, _)| l.starts_with(tag))
    };
    if find(&mut d, &second).is_none() {
        let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "plc"}));
        d.settle();
    }
    let row_at = |d: &mut Driver, tag: &str| {
        find(d, tag)
            .unwrap_or_else(|| panic!("no row {tag}: {:?}", d.labels()))
            .1
    };
    let (from, to) = (row_at(&mut d, &second), row_at(&mut d, &head));
    d.input(&Input::Drag {
        from,
        to,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    d.settle();
    assert!(waiting(&mut d).is_empty(), "the dragged card is attached");
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(toast, format!("Attached {second} to {head}."));
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
    // Two boxes with one tag: the toast says why.
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(toast, "Placed as 2 boxes, one for each bus interface.");
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
    let cpu_dp = cpu
        .iter()
        .find(|l| l.ends_with(":A"))
        .expect("the CPU's DP socket");
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
        d.shell_mut().dispatch("place.pick", &json!({"function": device})),
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
    let tag = pins
        .first()
        .and_then(|l| l.split(':').next())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(pins, [format!("{tag}:L+"), format!("{tag}:M")], "the supply pins");
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "2"}));
    d.settle();
    assert!(
        cp_labels(&mut d).contains(&format!("{tag}:A")),
        "the same CPU's DP on the bus page"
    );
}
