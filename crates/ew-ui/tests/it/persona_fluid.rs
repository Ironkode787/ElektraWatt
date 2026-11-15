// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The persona round's pneumatics from nothing (M3, run m3-p2): a QElectroTech user drew a valve terminal, a cylinder,
//! a supply and three tubes on a new project, by the window's own path (clicks, keys, menus) at 1920 × 1080.

use crate::persona_round::{click, key, new_project, starting_with, typed};
use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::ew_api::output::{NavNode, Uuid};
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input};
use serde_json::json;

fn right_click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    }
}

/// A click at `at` after the pointer rested there, so that a tool's ghost follows it first.
fn click_at(d: &mut Driver, at: (i64, i64)) {
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
}

/// A new project with the fluid page 201 shown, fitted, and the Fluid panel in the left column.
fn fluid_page(tmp: &Tmp) -> Driver {
    let mut d = new_project(tmp);
    let page = json!({"folio": "201", "type": "fluid", "title": "Clamp pneumatics"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "201"}));
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "fluid"}));
    d.settle();
    d
}

/// The Fluid navigator's nodes.
fn fluid_nodes(d: &mut Driver) -> Vec<NavNode> {
    d.settle();
    d.shell_mut()
        .nav(NavKind::Fluid)
        .and_then(|n| n.table())
        .map(|t| t.nodes.clone())
        .unwrap_or_default()
}

/// The device of the Fluid navigator's device row labelled `tag`.
fn fluid_device(d: &mut Driver, tag: &str) -> Uuid {
    fluid_nodes(d)
        .into_iter()
        .find(|n| n.role == NavRole::Device && n.label == tag)
        .and_then(|n| n.records.first().copied())
        .unwrap_or_else(|| panic!("no device row {tag}"))
}

/// The window pixel of the Fluid panel's row labelled `label`, by its accessible name.
fn row_of(d: &mut Driver, label: &str) -> (i64, i64) {
    let name = d
        .shell_mut()
        .shown_rows(NavKind::Fluid)
        .into_iter()
        .find(|r| r.label == label)
        .map(|r| r.name)
        .unwrap_or_else(|| panic!("no row {label}"));
    d.find(&name).unwrap_or_else(|| panic!("row {name} not on show"))
}

/// Places the starter valve terminal from the device browser as a box from `a` to `b`, then Escape.
fn place_terminal(d: &mut Driver, a: (i64, i64), b: (i64, i64)) {
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("valve terminal"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(d, "Valve terminal, 4"),
        mods: Modifiers::NONE,
    });
    d.settle();
    click_at(d, a);
    click_at(d, b);
    d.input(&key("escape"));
    d.settle();
}

/// Places the starter symbol `symbol` at `at`, then Escape.
fn place_symbol(d: &mut Driver, symbol: &str, at: (i64, i64)) {
    let picked = d.shell_mut().dispatch("place.pick", &json!({ "symbol": symbol }));
    assert_eq!(picked, Outcome::Done, "{symbol}");
    click_at(d, at);
    d.input(&key("escape"));
    d.settle();
}

/// The window pixel of the connection point `designation` of `device` on the page shown.
fn port(d: &mut Driver, device: Uuid, designation: &str) -> (i64, i64) {
    let canvas = d.shell_mut().canvas().unwrap();
    let tables = std::sync::Arc::clone(&canvas.page().tables);
    let snapper = canvas.page().snapper.clone();
    let cp = tables
        .cps
        .iter()
        .find(|c| {
            c.label.rsplit(':').next() == Some(designation)
                && tables
                    .reps
                    .iter()
                    .any(|r| r.id == c.rep && r.device == Some(device))
        })
        .unwrap_or_else(|| panic!("no port {designation}"));
    let sheet = snapper.page_to_sheet(cp.at);
    let pixel = sheet_pixel(d);
    pixel(sheet)
}

/// The window pixel of a sheet point, from two pointer positions the canvas reports.
fn sheet_pixel(d: &mut Driver) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    let probe = |d: &mut Driver, at: (i64, i64)| {
        d.input(&Input::Hover { at });
        d.settle();
        let p = crate::persona_round::state(d)["canvas"]["pointer_nm"].clone();
        (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())
    };
    let (f0, f1) = ((700, 400), (1000, 700));
    let (s0, s1) = (probe(d, f0), probe(d, f1));
    move |sheet: (i64, i64)| {
        (
            f0.0 + (sheet.0 - s0.0) * (f1.0 - f0.0) / (s1.0 - s0.0),
            f0.1 + (sheet.1 - s0.1) * (f1.1 - f0.1) / (s1.1 - s0.1),
        )
    }
}

/// Draws a tube with the Wire tool from port `a` to port `b` (window pixels), then Escape.
fn draw_tube(d: &mut Driver, a: (i64, i64), b: (i64, i64)) {
    let _ = d.shell_mut().dispatch("tool.wire", &json!({}));
    click_at(d, a);
    click_at(d, b);
    d.input(&key("escape"));
    d.settle();
}

/// The fluid lines of the page shown.
fn tubes(d: &mut Driver) -> usize {
    let canvas = d.shell_mut().canvas().unwrap();
    canvas
        .page()
        .tables
        .connections
        .iter()
        .filter(|c| c.kind == "fluid")
        .count()
}

/// P2-A: a valve terminal could not be built from the window. From nothing: the starter terminal placed from the device
/// browser is a valve terminal (the Fluid panel lists it); Add valve at position 1 on its box's menu and at position 2
/// on its row's menu each make a valve child whose symbol follows the pointer to its place; a cylinder, the air supply
/// and three tubes join them; Generate overview…, greyed in words before, draws the overview page.
#[test]
fn p2a_a_valve_terminal_is_built_from_nothing_and_its_overview_generated() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    // Greyed with its reason while no valve terminal is selected.
    let reason =
        ew_ui::ew_viewmodel::actions::find("fluid.generate_overview").map(|a| (a.enabled)(d.shell_mut()));
    assert_eq!(
        reason
            .and_then(Result::err)
            .map(ew_ui::ew_viewmodel::strings::text),
        Some("Select a valve terminal first: on its page or in the Fluid panel")
    );
    place_terminal(&mut d, (760, 300), (1260, 520));
    let nodes = fluid_nodes(&mut d);
    assert!(
        nodes
            .iter()
            .any(|n| n.role == NavRole::Section && n.label == "Valve terminals"),
        "the Fluid panel lists the valve terminal"
    );
    let terminal = nodes
        .iter()
        .find(|n| n.role == NavRole::Device)
        .map(|n| n.label.clone())
        .expect("the terminal's row");
    // Add valve at position 1 on the box's menu: the valve's symbol follows the pointer, a click places it.
    d.input(&right_click((1010, 330)));
    d.settle();
    d.input(&click(starting_with(&d, "Add valve at position 1")));
    d.settle();
    click_at(&mut d, (860, 440));
    d.input(&key("escape"));
    d.settle();
    // Add valve at position 2 on the terminal's row in the Fluid panel.
    let row = row_of(&mut d, &terminal);
    d.input(&right_click(row));
    d.settle();
    d.input(&click(starting_with(&d, "Add valve at position 2")));
    d.settle();
    click_at(&mut d, (1100, 440));
    d.input(&key("escape"));
    d.settle();
    let nodes = fluid_nodes(&mut d);
    let at = nodes.iter().position(|n| n.label == terminal).unwrap();
    let valves: Vec<&str> = nodes
        .iter()
        .filter(|n| n.parent == u32::try_from(at).ok() && n.role == NavRole::Device)
        .map(|n| n.label.as_str())
        .collect();
    let (q1, q2) = (format!("{terminal}-Q1"), format!("{terminal}-Q2"));
    assert_eq!(valves, [q1.as_str(), q2.as_str()], "the terminal's two valves");
    let drawn = |d: &mut Driver, device: Uuid| {
        d.shell_mut()
            .canvas()
            .unwrap()
            .page()
            .tables
            .reps
            .iter()
            .any(|r| r.device == Some(device))
    };
    let (v1, v2) = (fluid_device(&mut d, &q1), fluid_device(&mut d, &q2));
    assert!(drawn(&mut d, v1) && drawn(&mut d, v2), "both valves drawn");
    // A cylinder and the air supply (the persona's supply was greyed for want of a letter code).
    place_symbol(&mut d, "ew.iso.cylinder-double-acting@qet10", (900, 800));
    d.input(&click(d.find("Symbol").unwrap()));
    d.settle();
    d.input(&typed("supply point"));
    d.settle();
    d.input(&key("enter"));
    d.settle();
    click_at(&mut d, (1400, 800));
    d.input(&key("escape"));
    d.settle();
    let reps = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let device_of = |key: &str| {
        reps.reps
            .iter()
            .find(|r| r.symbol_key.starts_with(key))
            .and_then(|r| r.device)
            .unwrap_or_else(|| panic!("no {key}"))
    };
    let (cylinder, supply) = (device_of("ew.iso.cylinder"), device_of("ew.iso.air-supply"));
    // Three tubes with the Wire tool.
    for (a, b) in [
        ((v1, "4"), (cylinder, "B")),
        ((v1, "2"), (cylinder, "A")),
        ((supply, "1"), (v1, "1")),
    ] {
        let (pa, pb) = (port(&mut d, a.0, a.1), port(&mut d, b.0, b.1));
        draw_tube(&mut d, pa, pb);
    }
    assert_eq!(tubes(&mut d), 3);
    // Generate overview… on the terminal's row: the preview, then Apply draws the overview page.
    let row = row_of(&mut d, &terminal);
    d.input(&right_click(row));
    d.settle();
    d.input(&click(starting_with(&d, "Generate overview…")));
    d.settle();
    let notices = d.shell_mut().preview_pane().notices().to_vec();
    let page = notices
        .iter()
        .find(|n| n.contains("(new page)") && n.contains(&terminal))
        .and_then(|n| n.strip_prefix("page "))
        .and_then(|n| n.split([':', ' ']).next())
        .unwrap_or_else(|| panic!("a new overview page: {notices:?}"))
        .to_owned();
    d.input(&click(d.find("Apply").expect("the card's Apply")));
    d.settle();
    let _ = d.shell_mut().dispatch("page.goto", &json!({ "folio": page }));
    d.settle();
    let head = fluid_device(&mut d, &terminal);
    let overview = d.shell_mut().canvas().is_some_and(|c| {
        c.page()
            .tables
            .reps
            .iter()
            .any(|r| r.rep_type == "overview" && r.device == Some(head))
    });
    assert!(overview, "the terminal drawn on its overview page {page}");
    d.shell_mut().shutdown();
}

/// The device a symbol of the page shown draws, by its symbol key's start (`ew.iso.cylinder`).
fn device_drawn(d: &mut Driver, key: &str) -> Uuid {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    tables
        .reps
        .iter()
        .find(|r| r.symbol_key.starts_with(key))
        .and_then(|r| r.device)
        .unwrap_or_else(|| panic!("no {key}"))
}

/// A cylinder and the air supply joined by one tube, the tube selected.
fn one_tube(d: &mut Driver) {
    place_symbol(d, "ew.iso.cylinder-double-acting@qet10", (900, 600));
    place_symbol(d, "ew.iso.air-supply@qet10", (1300, 800));
    let (cylinder, supply) = (
        device_drawn(d, "ew.iso.cylinder"),
        device_drawn(d, "ew.iso.air-supply"),
    );
    let (a, b) = (port(d, supply, "1"), port(d, cylinder, "A"));
    draw_tube(d, a, b);
    one_tube_selected(d);
}

/// Selects the page's first tube.
fn one_tube_selected(d: &mut Driver) {
    let line = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .find(|c| c.kind == "fluid")
        .map(|c| c.id.to_string())
        .expect("the tube");
    let _ = d.shell_mut().dispatch("select.set", &json!({ "ids": [line] }));
    d.settle();
}

/// P2-C: no tube part existed, and a tube's Tube part offered the valves. It offers the starter tubes, and only
/// tubes; choosing one, a library part the project does not hold yet, embeds it and gives the tube its size.
#[test]
fn p2c_the_tube_part_offers_tubes_and_takes_one_from_the_library() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    let edit = json!({"row": "connection/tube_part"});
    assert_eq!(d.shell_mut().dispatch("props.edit", &edit), Outcome::Done);
    let choices: Vec<String> = d
        .shell_mut()
        .properties()
        .choices()
        .map(|c| c.items.iter().map(|(t, _)| t.clone()).collect())
        .unwrap_or_default();
    assert!(!choices.is_empty(), "the starter tubes");
    assert!(
        choices.iter().all(|c| c.starts_with("EW-GEN-TUB-")),
        "{choices:?}"
    );
    let set = json!({"row": "connection/tube_part", "value": "EW-GEN-TUB-6"});
    assert_eq!(d.shell_mut().dispatch("props.set", &set), Outcome::Done);
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({"kind": "tubes"}));
    d.settle();
    let table = d
        .shell_mut()
        .nav(NavKind::Tubes)
        .and_then(|n| n.table())
        .cloned()
        .expect("the tube table");
    let size = table.columns.iter().position(|c| c.key == "size").unwrap();
    assert_eq!(
        table.nodes[0].cells[size], "6×4",
        "the tube takes the part's size"
    );
    d.shell_mut().shutdown();
}

/// Item 3 of the round: three tubes drawn with the Wire tool from valves standing in their terminal's box stayed
/// unrouted ("no route keeps clear of symbols", EW-CORE-073 "Wire … has no route"). The box around the valves is no
/// obstacle to a route from a port inside it: the tube takes a path as a wire does.
#[test]
fn item3_a_tube_from_a_valve_inside_its_terminals_box_is_routed() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_terminal(&mut d, (760, 300), (1260, 520));
    d.input(&right_click((1010, 330)));
    d.settle();
    d.input(&click(starting_with(&d, "Add valve at position 1")));
    d.settle();
    click_at(&mut d, (900, 440));
    d.input(&key("escape"));
    d.settle();
    place_symbol(&mut d, "ew.iso.cylinder-double-acting@qet10", (900, 800));
    let valve = device_drawn(&mut d, "ew.iso.valve");
    let cylinder = device_drawn(&mut d, "ew.iso.cylinder");
    let (a, b) = (port(&mut d, valve, "4"), port(&mut d, cylinder, "B"));
    draw_tube(&mut d, a, b);
    let routed = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .filter(|c| c.kind == "fluid")
        .map(|c| c.stored_route)
        .collect::<Vec<_>>();
    assert_eq!(routed, [true], "the tube is routed");
    d.shell_mut().shutdown();
}

/// The connection point `designation` of `device` on the page shown as a `connection.draw` end.
fn cp_end(d: &mut Driver, device: Uuid, designation: &str) -> serde_json::Value {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let cp = tables
        .cps
        .iter()
        .find(|c| {
            c.label.rsplit(':').next() == Some(designation)
                && tables
                    .reps
                    .iter()
                    .any(|r| r.id == c.rep && r.device == Some(device))
        })
        .unwrap_or_else(|| panic!("no port {designation}"));
    json!({"cp": {"function": cp.function.to_string(), "idx": cp.idx}})
}

/// The persona's page: a valve at position 1 of a valve terminal drawn as a box around it, a cylinder below, and a tube
/// from the valve's port 4 to the cylinder's port B stored without a route (as the Wire tool stored it when the box
/// blocked every route); the tube's ID and the window pixel of the middle of its vertical run below the box.
fn unrouted_tube(d: &mut Driver) -> (String, (i64, i64)) {
    place_terminal(d, (760, 300), (1260, 520));
    d.input(&right_click((1010, 330)));
    d.settle();
    d.input(&click(starting_with(d, "Add valve at position 1")));
    d.settle();
    click_at(d, (900, 440));
    d.input(&key("escape"));
    d.settle();
    place_symbol(d, "ew.iso.cylinder-double-acting@qet10", (800, 850));
    let (valve, cylinder) = (
        device_drawn(d, "ew.iso.valve"),
        device_drawn(d, "ew.iso.cylinder"),
    );
    let page = d.shell_mut().canvas().unwrap().page().tables.page.to_string();
    let draw = json!({"cmd": "connection.draw", "params": {
        "a": cp_end(d, valve, "4"), "b": cp_end(d, cylinder, "B"), "page": page,
        "kind": {"fluid": {}}, "route": []}});
    assert_eq!(d.shell_mut().dispatch("command.run", &draw), Outcome::Done);
    d.settle();
    let canvas = d.shell_mut().canvas().unwrap();
    let c = canvas
        .page()
        .tables
        .connections
        .iter()
        .find(|c| c.kind == "fluid")
        .unwrap()
        .clone();
    assert!(!c.stored_route, "unrouted");
    // Its vertical run: the segment of the drawn line whose x is the valve port's, between the box and the cylinder.
    let (a, b) = (port(d, valve, "4"), port(d, cylinder, "B"));
    (c.id.to_string(), (a.0, (a.1.max(520) + b.1) / 2))
}

/// P2-F: a click on a dashed (unrouted) tube selected nothing, and a right-click there opened the page's menu. A click
/// on the line an unrouted tube is drawn with selects it, on the persona's page.
#[test]
fn p2f_a_click_on_an_unrouted_tube_selects_it() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let (line, _) = unrouted_tube(&mut d);
    let polyline = d.shell_mut().canvas().unwrap().page().tables.connections[0]
        .polyline
        .clone();
    // At the persona's zoom, 196 %, centred on the line's longest segment.
    let (p, q) = polyline
        .windows(2)
        .map(|w| (w[0], w[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .unwrap();
    let mid = ((p.0 + q.0) / 2, (p.1 + q.1) / 2);
    let snapper = d.shell_mut().canvas().unwrap().page().snapper.clone();
    let sheet = snapper.page_to_sheet(mid);
    let zoom = json!({"percent": 196, "x_mm": sheet.0 as f64 / 1e6, "y_mm": sheet.1 as f64 / 1e6});
    let _ = d.shell_mut().dispatch("view.zoom_to", &zoom);
    d.settle();
    let pixel = sheet_pixel(&mut d);
    let at = pixel(sheet);
    let at = (at.0 + 1, at.1);
    d.input(&click(at));
    d.settle();
    assert_eq!(
        crate::persona_round::state(&mut d)["canvas"]["selection"],
        json!([line]),
        "the click at {at:?} selects the tube drawn along {polyline:?}"
    );
    // Check project: the finding calls it a tube.
    d.input(&click(d.find("Check project").unwrap()));
    d.settle();
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({"kind": "messages"}));
    d.settle();
    let messages: Vec<String> = d
        .shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .map(|t| {
            t.nodes
                .iter()
                .filter(|n| n.role == NavRole::Finding && n.cells[1] == "EW-CORE-073")
                .map(|n| n.cells[2].clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !messages.is_empty() && messages.iter().all(|m| m.starts_with("Tube ")),
        "{messages:?}"
    );
    d.shell_mut().shutdown();
}

/// The messages of rule `rule` after Check project.
fn findings(d: &mut Driver, rule: &str) -> Vec<String> {
    d.input(&click(d.find("Check project").unwrap()));
    d.settle();
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({"kind": "messages"}));
    d.settle();
    d.shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .map(|t| {
            t.nodes
                .iter()
                .filter(|n| n.role == NavRole::Finding && n.cells[1] == rule)
                .map(|n| n.cells[2].clone())
                .collect()
        })
        .unwrap_or_default()
}

/// P2-D: after Fluid settings named a 6×4 default tube, EW-FL-007 still said "no tube specification" on tubes the tube
/// table showed as 6×4, and Apply said nothing. The rule asks a material as well; now Apply's toast says the default
/// tube reaches the tubes drawn and lacks a material, the finding names the material, and a material clears it.
#[test]
fn p2d_the_default_tube_and_ew_fl_007_agree() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    let settings = json!({"od": "6", "id": "4"});
    assert_eq!(d.shell_mut().dispatch("fluid.settings", &settings), Outcome::Done);
    d.settle();
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        toast.contains("6 × 4 mm") && toast.contains("drawn already") && toast.contains("without a material"),
        "{toast}"
    );
    let messages = findings(&mut d, "EW-FL-007");
    assert!(
        messages.len() == 1 && messages[0].contains("is 6×4 mm but has no material"),
        "{messages:?}"
    );
    let settings = json!({"material": "pu"});
    assert_eq!(d.shell_mut().dispatch("fluid.settings", &settings), Outcome::Done);
    d.settle();
    assert!(findings(&mut d, "EW-FL-007").is_empty(), "the tube is specified");
    d.shell_mut().shutdown();
}

/// Item 8 of the round: a Wire click between two ports that already sit on one line did nothing and said nothing. A
/// second tube between the two ends of a tube, or from a port to a point of its own tube, is refused in words where
/// the tool's hint stands, and the routing waits for another end.
#[test]
fn item8_a_tube_between_ports_already_on_one_line_says_so() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_symbol(&mut d, "ew.iso.cylinder-double-acting@qet10", (900, 600));
    place_symbol(&mut d, "ew.iso.air-supply@qet10", (1300, 800));
    let (cylinder, supply) = (
        device_drawn(&mut d, "ew.iso.cylinder"),
        device_drawn(&mut d, "ew.iso.air-supply"),
    );
    let (a, b) = (port(&mut d, supply, "1"), port(&mut d, cylinder, "A"));
    draw_tube(&mut d, a, b);
    assert_eq!(tubes(&mut d), 1);
    let _ = d.shell_mut().dispatch("tool.wire", &json!({}));
    // Apart in time, so that the second click is no double click.
    d.input(&Input::Wait { ms: 800 });
    click_at(&mut d, b);
    d.input(&Input::Wait { ms: 800 });
    click_at(&mut d, a);
    let state = crate::persona_round::state(&mut d);
    assert_eq!(tubes(&mut d), 1, "no second tube");
    assert_eq!(
        state["status"][0],
        json!("These two points already sit on one line; click a point of another line, or Esc")
    );
    assert_eq!(state["canvas"]["tool"]["state"], json!("routing"));
    // From a port to a point of its own tube (the persona's second click landed on the supply tube's line).
    d.input(&key("escape"));
    d.settle();
    let polyline = d.shell_mut().canvas().unwrap().page().tables.connections[0]
        .polyline
        .clone();
    let (p, q) = polyline
        .windows(2)
        .map(|w| (w[0], w[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .unwrap();
    let snapper = d.shell_mut().canvas().unwrap().page().snapper.clone();
    let pixel = sheet_pixel(&mut d);
    let mid = pixel(snapper.page_to_sheet(((p.0 + q.0) / 2, (p.1 + q.1) / 2)));
    let _ = d.shell_mut().dispatch("tool.wire", &json!({}));
    d.input(&Input::Wait { ms: 800 });
    click_at(&mut d, a);
    d.input(&Input::Wait { ms: 800 });
    click_at(&mut d, mid);
    assert_eq!(tubes(&mut d), 1, "no tube to its own line");
    assert_eq!(
        crate::persona_round::state(&mut d)["status"][0],
        json!("These two points already sit on one line; click a point of another line, or Esc")
    );
    d.shell_mut().shutdown();
}

/// The text and the provenance chip of the Properties row `key` of the selection.
fn prop_row(d: &mut Driver, key: &str) -> (String, String) {
    d.settle();
    let sheet = d.shell_mut().properties().sheet().cloned().expect("the sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|s| s.rows.iter())
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no row {key}"))
        .clone();
    let text = match &row.value {
        ew_ui::ew_viewmodel::properties::RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    };
    (text, ew_ui::ew_viewmodel::properties::chip_text(&row.provenance))
}

/// P2-E: the medium could not be found from a tube (Properties showed Line kind, Tube part, Tube length), and Fluid
/// settings… and New medium… were found only through the command search. The Fluid panel offers both; a tube's
/// Properties show its medium with where it is defined, set it by a choice, and show its effective tube with its source.
#[test]
fn p2e_a_tubes_properties_show_and_set_its_medium_and_its_tube() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    assert!(
        d.find("Fluid settings…").is_some(),
        "Fluid settings… on the Fluid panel"
    );
    assert!(d.find("New medium…").is_some(), "New medium… on the Fluid panel");
    let medium = json!({"name": "Compressed air", "p_nom": "6"});
    assert_eq!(d.shell_mut().dispatch("fluid.medium_new", &medium), Outcome::Done);
    let settings = json!({"od": "6", "id": "4", "material": "pu"});
    assert_eq!(d.shell_mut().dispatch("fluid.settings", &settings), Outcome::Done);
    one_tube_selected(&mut d);
    let edit = json!({"row": "connection/derived.medium"});
    assert_eq!(d.shell_mut().dispatch("props.edit", &edit), Outcome::Done);
    let (name, id) = d
        .shell_mut()
        .properties()
        .choices()
        .and_then(|c| c.items.first().cloned())
        .expect("the project's medium as a choice");
    assert_eq!(name, "Compressed air");
    let set = json!({"row": "connection/derived.medium", "value": id});
    assert_eq!(d.shell_mut().dispatch("props.set", &set), Outcome::Done);
    one_tube_selected(&mut d);
    let (text, chip) = prop_row(&mut d, "connection/derived.medium");
    assert_eq!(text, "Compressed air");
    assert!(chip.contains("medium defined at"), "{chip}");
    let (text, chip) = prop_row(&mut d, "connection/derived.tube");
    assert_eq!(text, "6×4 mm PU");
    assert!(chip.contains("the project's default tube"), "{chip}");
    d.shell_mut().shutdown();
}

/// P2-G: the Fluid panel's table said "No fluid lines yet" over three working lines drawn, and "tube", "working line"
/// and "fluid line" read as three things. The panel's table lists the tubes drawn, its empty state and a tube's
/// Properties say "tube".
#[test]
fn p2g_the_fluid_panels_table_lists_the_tubes_and_says_tube() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let table = json!({"kind": "fluid", "on": true});
    assert_eq!(d.shell_mut().dispatch("navigator.table", &table), Outcome::Done);
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    assert!(
        d.texts().iter().any(|t| t == "No tubes yet"),
        "the empty table says tubes"
    );
    one_tube(&mut d);
    let rows = d.shell_mut().shown_rows(NavKind::Fluid);
    assert_eq!(rows.len(), 1, "the one tube drawn: {rows:?}");
    let header = d.shell_mut().properties().sheet().map(|s| s.header.title.clone());
    assert_eq!(header.as_deref(), Some("Tube (Working)"));
    d.shell_mut().shutdown();
}

/// Places the starter symbol the symbol browser finds for `filter` at each of `points`, then Escape.
fn place_from_browser(d: &mut Driver, filter: &str, points: &[(i64, i64)]) {
    d.input(&click(d.find("Symbol").unwrap()));
    d.settle();
    d.input(&typed(filter));
    d.settle();
    d.input(&key("enter"));
    d.settle();
    for p in points {
        click_at(d, *p);
    }
    d.input(&key("escape"));
    d.settle();
}

/// The devices of the page shown drawn with a symbol whose key starts with `key`, left to right, then top to bottom.
fn devices_drawn(d: &mut Driver, key: &str) -> Vec<Uuid> {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let mut reps: Vec<_> = tables
        .reps
        .iter()
        .filter(|r| r.symbol_key.starts_with(key))
        .collect();
    reps.sort_by_key(|r| r.pos);
    reps.iter().filter_map(|r| r.device).collect()
}

/// The Valve position row of `valve` (its text) and whether its chip names the terminal it is a child of.
fn seat(d: &mut Driver, valve: Uuid) -> (String, bool) {
    let _ = d
        .shell_mut()
        .dispatch("select.set", &json!({ "ids": [valve.to_string()] }));
    let (text, chip) = prop_row(d, "device/derived.position");
    (text, chip.contains("child of"))
}

/// R2-A of the re-test: the persona placed valve symbols into the valve terminal's box from the symbol browser, and the
/// check said all four positions were empty; the Position row stood under Structure and typing into it did nothing.
/// A valve placed inside the box is the terminal's valve at the next free position, with a toast that names it; its
/// Valve position row stands in Main and moves it; a loose valve dragged into the box joins it too.
#[test]
fn r2a_a_valve_placed_inside_a_valve_terminals_box_is_its_valve() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_terminal(&mut d, (760, 300), (1260, 520));
    place_from_browser(&mut d, "valve-5-2-sol-spring", &[(860, 440), (1100, 440)]);
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.contains("is the valve at position 2 of"), "{toast}");
    let valves = devices_drawn(&mut d, "ew.iso.valve-5-2");
    assert_eq!(valves.len(), 2);
    assert_eq!(seat(&mut d, valves[0]), ("1".to_owned(), true));
    assert_eq!(seat(&mut d, valves[1]), ("2".to_owned(), true));
    // The row stands in Main, without a filter, and moves the valve to a free position.
    let row = d
        .shell_mut()
        .properties()
        .sheet()
        .and_then(|s| {
            s.sections
                .iter()
                .flat_map(|s| s.rows.iter())
                .find(|r| r.key == "device/derived.position")
                .cloned()
        })
        .expect("the Valve position row");
    assert!(
        ew_ui::ew_viewmodel::properties::is_primary("device", &row),
        "Valve position is a Main row"
    );
    let set = json!({"row": "device/derived.position", "value": "4"});
    assert_eq!(d.shell_mut().dispatch("props.set", &set), Outcome::Done);
    assert_eq!(seat(&mut d, valves[1]).0, "4");
    let set = json!({"row": "device/derived.position", "value": "first"});
    assert!(matches!(
        d.shell_mut().dispatch("props.set", &set),
        Outcome::Failed { .. }
    ));
    // A loose valve, dragged into the box, takes the next free position.
    place_from_browser(&mut d, "valve-5-2-sol-spring", &[(1000, 800)]);
    let loose = devices_drawn(&mut d, "ew.iso.valve-5-2")
        .into_iter()
        .find(|v| !valves.contains(v))
        .unwrap();
    assert_eq!(seat(&mut d, loose), ("Absent".to_owned(), false), "loose below the box");
    let _ = d.shell_mut().dispatch("tool.select", &json!({}));
    d.input(&Input::Drag {
        from: (1000, 800),
        to: (1000, 440),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    d.settle();
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.contains("is the valve at position 2 of"), "{toast}");
    assert_eq!(seat(&mut d, loose), ("2".to_owned(), true));
    d.shell_mut().shutdown();
}

/// A loose 5/2 valve, a double-acting cylinder and the air supply with three tubes: supply to port 1, port 2 to A, port
/// 4 to B; the valve, the tubes in that order.
fn valve_and_three_tubes(d: &mut Driver) -> (Uuid, Vec<Uuid>) {
    place_symbol(d, "ew.iso.valve-5-2-sol-spring@qet10", (1000, 650));
    place_symbol(d, "ew.iso.cylinder-double-acting@qet10", (1000, 350));
    place_symbol(d, "ew.iso.air-supply@qet10", (700, 850));
    let (valve, cylinder, supply) = (
        device_drawn(d, "ew.iso.valve-5-2"),
        device_drawn(d, "ew.iso.cylinder"),
        device_drawn(d, "ew.iso.air-supply"),
    );
    let mut lines = Vec::new();
    for (a, b) in [
        ((supply, "1"), (valve, "1")),
        ((valve, "2"), (cylinder, "A")),
        ((valve, "4"), (cylinder, "B")),
    ] {
        let (pa, pb) = (port(d, a.0, a.1), port(d, b.0, b.1));
        let before: Vec<Uuid> = d.shell_mut().canvas().unwrap().page().tables.connections.iter().map(|c| c.id).collect();
        draw_tube(d, pa, pb);
        let new = d
            .shell_mut()
            .canvas()
            .unwrap()
            .page()
            .tables
            .connections
            .iter()
            .find(|c| c.kind == "fluid" && !before.contains(&c.id))
            .map(|c| c.id)
            .expect("the tube drawn");
        lines.push(new);
    }
    (valve, lines)
}

/// The Tubes table's cell of column `column` in the row of tube `line`.
fn tube_cell(d: &mut Driver, line: Uuid, column: &str) -> String {
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({"kind": "tubes"}));
    d.settle();
    let table = d
        .shell_mut()
        .nav(NavKind::Tubes)
        .and_then(|n| n.table())
        .cloned()
        .expect("the tube table");
    let at = table.columns.iter().position(|c| c.key == column).unwrap();
    table
        .nodes
        .iter()
        .find(|n| n.records.first() == Some(&line))
        .map(|n| n.cells[at].clone())
        .unwrap_or_else(|| panic!("no row of {line}"))
}

/// R2-G of the re-test: a medium set on two tubes at once said "set on 2 connections", but only the first kept it: the
/// other's row in the tube list stayed empty, the Media list counted one tube too few and the selection read "varies".
/// Set on the two working tubes, which lie in different nets, both carry it, the Media list counts them, and the
/// selection reads the medium with both places it is defined at.
#[test]
fn r2g_a_medium_set_on_two_tubes_at_once_holds_on_both() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let (_, lines) = valve_and_three_tubes(&mut d);
    let medium = json!({"name": "Compressed air", "p_nom": "6"});
    assert_eq!(d.shell_mut().dispatch("fluid.medium_new", &medium), Outcome::Done);
    let pick = json!({ "ids": [lines[1].to_string(), lines[2].to_string()] });
    let _ = d.shell_mut().dispatch("select.set", &pick);
    d.settle();
    let edit = json!({"row": "connection/derived.medium"});
    assert_eq!(d.shell_mut().dispatch("props.edit", &edit), Outcome::Done);
    let (_, id) = d
        .shell_mut()
        .properties()
        .choices()
        .and_then(|c| c.items.first().cloned())
        .expect("the project's medium as a choice");
    let set = json!({"row": "connection/derived.medium", "value": id});
    assert_eq!(d.shell_mut().dispatch("props.set", &set), Outcome::Done);
    let _ = d.shell_mut().dispatch("select.set", &pick);
    let (text, chip) = prop_row(&mut d, "connection/derived.medium");
    assert_eq!(text, "Compressed air");
    assert!(!chip.contains("varies"), "{chip}");
    for l in &lines[1..] {
        assert_eq!(tube_cell(&mut d, *l, "medium"), "Compressed air");
    }
    let carried = lines
        .iter()
        .filter(|l| tube_cell(&mut d, **l, "medium") == "Compressed air")
        .count();
    let badge = fluid_nodes(&mut d)
        .into_iter()
        .find(|n| n.label == "Compressed air")
        .and_then(|n| n.badges.first().cloned())
        .expect("the medium's row");
    assert_eq!(badge, format!("{carried} lines"), "the Media row counts the tubes that carry it");
    d.shell_mut().shutdown();
}

/// The texts of the choices under the Properties row being edited.
fn choice_texts(d: &mut Driver) -> Vec<String> {
    d.shell_mut()
        .properties()
        .choices()
        .map(|c| c.items.iter().map(|(t, _)| t.clone()).collect())
        .unwrap_or_default()
}

/// R2-F of the re-test: with no medium in the project, a tube's Medium was a text box that refused every text and
/// offered no list, and the air supply's Properties said nothing of a medium. The Medium row offers New medium… (the
/// only entry, then the last); the medium the form makes is the tube's; the air supply shows the medium it supplies,
/// and sets it there too.
#[test]
fn r2f_new_medium_from_the_tubes_medium_and_the_supplys_medium() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    let edit = json!({"row": "connection/derived.medium"});
    assert_eq!(d.shell_mut().dispatch("props.edit", &edit), Outcome::Done);
    assert_eq!(choice_texts(&mut d), ["New medium…"]);
    let choose = json!({"index": 0});
    assert_eq!(d.shell_mut().dispatch("props.choose", &choose), Outcome::Done);
    d.settle();
    let action = match d.shell_mut().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f.action,
        other => panic!("{other:?}"),
    };
    assert_eq!(action, "fluid.medium_new");
    // A frame to show the form (a dispatch asks for none).
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    // The first field has the focus: the name, Tab, the pressure, Enter.
    d.input(&typed("Compressed air"));
    d.input(&key("tab"));
    d.input(&typed("6"));
    d.input(&key("enter"));
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "the form is done");
    one_tube_selected(&mut d);
    let (text, _) = prop_row(&mut d, "connection/derived.medium");
    assert_eq!(text, "Compressed air", "the new medium is the tube's");
    assert_eq!(d.shell_mut().dispatch("props.edit", &edit), Outcome::Done);
    assert_eq!(choice_texts(&mut d), ["Compressed air", "New medium…"]);
    let _ = d.shell_mut().dispatch("props.cancel", &json!({}));
    // The air supply says what it supplies, in Main.
    let supply = device_drawn(&mut d, "ew.iso.air-supply");
    let _ = d
        .shell_mut()
        .dispatch("select.set", &json!({ "ids": [supply.to_string()] }));
    let (text, chip) = prop_row(&mut d, "device/derived.medium");
    assert_eq!(text, "Compressed air");
    assert!(chip.contains("medium defined at"), "{chip}");
    d.shell_mut().shutdown();
}

/// R2-B of the re-test: a second 5/2 valve placed right above the first had its exhausts 3 and 5 joined to the first's
/// working ports 2 and 4 by autoconnect, because the ports faced each other. A working port never takes another valve's
/// exhaust port (EW-FL-012's conflict): the stacked valve is placed without tubes.
#[test]
fn r2b_autoconnect_never_joins_an_exhaust_to_a_working_port() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_symbol(&mut d, "ew.iso.valve-5-2-sol-spring@qet10", (1000, 650));
    place_symbol(&mut d, "ew.iso.valve-5-2-sol-spring@qet10", (1000, 500));
    assert_eq!(devices_drawn(&mut d, "ew.iso.valve-5-2").len(), 2);
    assert_eq!(tubes(&mut d), 0, "no exhaust joined to a working port");
    d.shell_mut().shutdown();
}

/// Clicks the Properties header's finding chip (its accessible name `<n> Warning…` or `<n> Pending…`) and answers the
/// finding rows Messages shows then.
fn chip_findings(d: &mut Driver) -> Vec<String> {
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    let chip = d
        .labels()
        .into_iter()
        .find(|(l, _)| {
            l.split_once(' ').is_some_and(|(n, rest)| {
                n.parse::<u32>().is_ok() && (rest.starts_with("Warning") || rest.starts_with("Pending"))
            })
        })
        .map(|(_, at)| at)
        .expect("a finding chip");
    d.input(&click(chip));
    d.settle();
    d.shell_mut()
        .shown_rows(NavKind::Messages)
        .into_iter()
        .filter(|r| r.role == "finding")
        .map(|r| r.label)
        .collect()
}

/// R2-D of the re-test: the "2 Warnings" chip of a selected tube opened Messages filtered by its heading "Tube
/// (Working)", and nothing matched. The chip filters by the tube's name, so Messages lists its findings and no other;
/// a device's chip (another kind) lists the device's.
#[test]
fn r2d_a_tubes_finding_chip_lists_its_findings() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    let rows = chip_findings(&mut d);
    assert!(!rows.is_empty(), "the tube's findings are listed");
    assert!(rows.iter().all(|r| r.contains("-201X1:1 → -201M1:A")), "{rows:?}");
    let cylinder = device_drawn(&mut d, "ew.iso.cylinder");
    let _ = d
        .shell_mut()
        .dispatch("select.set", &json!({ "ids": [cylinder.to_string()] }));
    let rows = chip_findings(&mut d);
    assert!(rows.iter().any(|r| r.starts_with("-201M1 has no part")), "{rows:?}");
    d.shell_mut().shutdown();
}

/// R2-H and R2-I of the re-test: a tube length typed as 350 read "350.000" with no unit, and the report pages made with
/// Output ▸ Report stood in the page tree as bare folios ("912"). The length reads "350 mm"; a report block made without
/// a title of its own is titled by its report's name.
#[test]
fn r2h_a_tube_length_shows_its_unit_and_a_report_page_its_name() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    let set = json!({"row": "connection/tube_length", "value": "350"});
    assert_eq!(d.shell_mut().dispatch("props.set", &set), Outcome::Done);
    one_tube_selected(&mut d);
    assert_eq!(prop_row(&mut d, "connection/tube_length").0, "350 mm");
    let report = json!({"kind": "tube_list", "folio": "912"});
    assert_eq!(d.shell_mut().dispatch("report_block.new", &report), Outcome::Done);
    d.settle();
    let rows: Vec<String> = d
        .shell_mut()
        .shown_rows(NavKind::Pages)
        .into_iter()
        .map(|r| r.label)
        .collect();
    assert!(rows.iter().any(|r| r == "912 Tube list"), "{rows:?}");
    d.shell_mut().shutdown();
}

/// R2-E of the re-test: the supply plate EW-GEN-VTS-1 placed from Device was an empty box with no connection point.
/// Placed into the valve terminal's box it draws its ports 1, 3 and 5 and joins the terminal as its supply plate, so a
/// supply line to its port 1 feeds the valve beside it through gallery 1 (no "port 1 has no supply").
#[test]
fn r2e_the_supply_plate_has_its_ports_and_feeds_the_valves() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_terminal(&mut d, (760, 300), (1260, 520));
    place_from_browser(&mut d, "valve-5-2-sol-spring", &[(1100, 440)]);
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("supply plate"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Valve terminal supply plate"),
        mods: Modifiers::NONE,
    });
    d.settle();
    click_at(&mut d, (860, 440));
    d.input(&key("escape"));
    d.settle();
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.contains("is a supply plate of"), "{toast}");
    let plate = device_drawn(&mut d, "ew.iso.valve-supply-plate");
    let ports: Vec<String> = {
        let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
        let reps: Vec<Uuid> = tables
            .reps
            .iter()
            .filter(|r| r.device == Some(plate))
            .map(|r| r.id)
            .collect();
        tables
            .cps
            .iter()
            .filter(|c| reps.contains(&c.rep))
            .filter_map(|c| c.label.rsplit(':').next().map(str::to_owned))
            .collect()
    };
    assert_eq!(ports, ["1", "3", "5"], "the plate's ports");
    place_symbol(&mut d, "ew.iso.air-supply@qet10", (800, 800));
    let supply = device_drawn(&mut d, "ew.iso.air-supply");
    let (a, b) = (port(&mut d, supply, "1"), port(&mut d, plate, "1"));
    draw_tube(&mut d, a, b);
    assert_eq!(tubes(&mut d), 1);
    assert!(findings(&mut d, "EW-FL-010").is_empty(), "the valve's port 1 is fed through the plate");
    d.shell_mut().shutdown();
}

/// R2-J of the re-test: the job's cylinder and air supply kept their "no part" warnings, for want of a starter part.
/// The starter parts EW-GEN-CYL-1 and EW-GEN-AIR-1 answer them.
#[test]
fn r2j_the_starter_parts_answer_the_cylinder_and_the_air_supply() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    let no_part = |d: &mut Driver| {
        findings(d, "EW-CORE-041")
            .into_iter()
            .filter(|m| m.starts_with("-201M1") || m.starts_with("-201X1"))
            .count()
    };
    assert_eq!(no_part(&mut d), 2, "the cylinder and the supply have no part");
    for (key, part) in [
        ("ew.iso.cylinder", "EW-GEN-CYL-1"),
        ("ew.iso.air-supply", "EW-GEN-AIR-1"),
    ] {
        let device = device_drawn(&mut d, key);
        let _ = d
            .shell_mut()
            .dispatch("select.set", &json!({ "ids": [device.to_string()] }));
        d.settle();
        let set = json!({"row": "device/parts", "value": part});
        assert_eq!(d.shell_mut().dispatch("props.set", &set), Outcome::Done, "{part}");
    }
    assert_eq!(no_part(&mut d), 0, "both have their starter part");
    d.shell_mut().shutdown();
}

/// Moves the representation of `device` on the page shown so that its insertion point is `steps` grid steps from the
/// insertion point of `of`'s representation.
fn set_beside(d: &mut Driver, device: Uuid, of: Uuid, steps: (i64, i64)) {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let rep = |dev: Uuid| tables.reps.iter().find(|r| r.device == Some(dev)).unwrap().clone();
    let (r, o) = (rep(device), rep(of));
    let grid = ew_ui::ew_viewmodel::tools::wire::PageGridMap::new(tables.grid);
    let k = |v: i64| grid.index(v);
    let by = [
        k(o.pos.0) + steps.0 - k(r.pos.0),
        k(o.pos.1) + steps.1 - k(r.pos.1),
    ];
    let mv = json!({"cmd": "rep.move", "params": {"reps": [r.id.to_string()], "steps": by}});
    assert_eq!(d.shell_mut().dispatch("command.run", &mv), Outcome::Done);
    d.settle();
}

/// The re-test's page (frames t0022, t0082): a valve terminal's box holding two 5/2 valves, the second eight grid steps
/// above the first, so that its ports 5, 1 and 3 face the first's ports 4 and 2 two steps away, and a double-acting
/// cylinder right of the box at the first valve's height; the first valve and the cylinder.
fn stacked_valves(d: &mut Driver) -> (Uuid, Uuid) {
    place_terminal(d, (760, 300), (1260, 520));
    place_from_browser(d, "valve-5-2-sol-spring", &[(900, 470), (900, 360)]);
    place_symbol(d, "ew.iso.cylinder-double-acting@qet10", (1500, 470));
    let valves = devices_drawn(d, "ew.iso.valve-5-2");
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let y = |v: Uuid| tables.reps.iter().find(|r| r.device == Some(v)).unwrap().pos.1;
    let (lower, upper) = if y(valves[0]) > y(valves[1]) {
        (valves[0], valves[1])
    } else {
        (valves[1], valves[0])
    };
    set_beside(d, upper, lower, (0, -8));
    let cylinder = device_drawn(d, "ew.iso.cylinder");
    set_beside(d, cylinder, lower, (40, 0));
    (lower, cylinder)
}

/// R2-C of the re-test: the tube from port 4 on top of the lower of two stacked valves to the cylinder's B said "No
/// clear route: the wire stays unrouted" and stayed a line through the valve; Route page refused ("no route keeps clear
/// of texts / other connection points"). The tube takes a route.
#[test]
fn r2c_a_tube_from_a_port_facing_another_valve_takes_a_route() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let (valve, cylinder) = stacked_valves(&mut d);
    let (a, b) = (port(&mut d, valve, "4"), port(&mut d, cylinder, "B"));
    let _ = d.shell_mut().dispatch("tool.wire", &json!({}));
    click_at(&mut d, a);
    d.input(&Input::Hover { at: b });
    d.settle();
    let hint = crate::persona_round::state(&mut d)["status"][0].clone();
    click_at(&mut d, b);
    d.input(&key("escape"));
    d.settle();
    let routed: Vec<bool> = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .filter(|c| c.kind == "fluid")
        .map(|c| c.stored_route)
        .collect();
    assert_eq!(routed, [true], "the tube is routed; the hint said {hint}");
    assert_eq!(
        hint,
        json!("The route crosses a text: no route keeps clear of the texts here.")
    );
    d.shell_mut().shutdown();
}

/// R2-C, Route page: the same tube stored without a route (as the Wire tool stored it before) is routed by Route page,
/// whose card says it crosses a text.
#[test]
fn r2c_route_page_routes_the_tube_across_a_text_and_says_so() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let (valve, cylinder) = stacked_valves(&mut d);
    let page = d.shell_mut().canvas().unwrap().page().tables.page.to_string();
    let draw = json!({"cmd": "connection.draw", "params": {
        "a": cp_end(&mut d, valve, "4"), "b": cp_end(&mut d, cylinder, "B"), "page": page,
        "kind": {"fluid": {}}, "route": []}});
    assert_eq!(d.shell_mut().dispatch("command.run", &draw), Outcome::Done);
    d.settle();
    assert_eq!(d.shell_mut().dispatch("route.page", &json!({})), Outcome::Done);
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    assert!(
        d.texts()
            .iter()
            .any(|t| t.starts_with("Routed across a text, as no route keeps clear of the texts:")),
        "the card says what was given up"
    );
    d.input(&click(d.find("Apply").expect("the card's Apply")));
    d.settle();
    let routed: Vec<bool> = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .filter(|c| c.kind == "fluid")
        .map(|c| c.stored_route)
        .collect();
    assert_eq!(routed, [true]);
    d.shell_mut().shutdown();
}

/// R2-C, the bend clicks: the persona's clicks on free points while drawing a tube landed on another tube (a tee and a
/// part tube) and beside the start port (nothing), and nothing said so. The hint line says what a click at the point
/// does: a tee on a line, nothing on the start, else a bend.
#[test]
fn r2c_the_hint_says_what_a_click_will_do_while_a_tube_is_drawn() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    one_tube(&mut d);
    let polyline = d.shell_mut().canvas().unwrap().page().tables.connections[0]
        .polyline
        .clone();
    let (p, q) = polyline
        .windows(2)
        .map(|w| (w[0], w[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .unwrap();
    let snapper = d.shell_mut().canvas().unwrap().page().snapper.clone();
    let pixel = sheet_pixel(&mut d);
    let on_line = pixel(snapper.page_to_sheet(((p.0 + q.0) / 2, (p.1 + q.1) / 2)));
    let cylinder = device_drawn(&mut d, "ew.iso.cylinder");
    let start = port(&mut d, cylinder, "B");
    let _ = d.shell_mut().dispatch("tool.wire", &json!({}));
    click_at(&mut d, start);
    let hint_at = |d: &mut Driver, at: (i64, i64)| {
        d.input(&Input::Hover { at });
        d.settle();
        crate::persona_round::state(d)["status"][0].clone()
    };
    assert_eq!(
        hint_at(&mut d, on_line),
        json!("A click here joins the line under the pointer with a tee; Alt+Click places a bend instead.")
    );
    assert_eq!(
        hint_at(&mut d, (start.0, start.1 - 3)),
        json!("A click here is the start point: click further from it to place a bend.")
    );
    let free = hint_at(&mut d, (start.0 + 200, start.1 + 150));
    assert!(
        free.as_str().is_some_and(|h| h.contains("a free point for a bend")),
        "{free}"
    );
    d.shell_mut().shutdown();
}
