// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The bus navigator in the window (FR-UI-006, doc 10 §6), at 1920 × 1080: on Golden A the rail's Bus shows the
//! logical net with its master port and the physical net's derived segment with its ends, the terminated connector
//! and the cable; its table edits a port's address as typed (FT-856), which Properties shows in words; a new
//! project's CPU row offers Place port DP, which places the port joined to the CPU (FT-831); the Wire tool draws on a
//! bus-topology page (FT-434).

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_viewmodel::navigators::{NavKind, NavRole};
use serde_json::json;
use support::{Tmp, dispatch, on_golden, settle};

/// The four pointer events of a double click at `at`, in one frame.
pub(crate) fn double_click(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, at: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false, true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(h);
}

#[test]
fn the_bus_navigator_lists_nets_ports_and_segments_and_edits_an_address() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "3");
    h.get_by_label("Bus").click();
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Bus);
    let texts: Vec<&str> = rows.iter().map(|r| r.text.as_str()).collect();
    for want in ["Logical nets", "PROFIBUS DP", "Physical nets", "DP", "Segment 1"] {
        assert!(texts.contains(&want), "{want}: {texts:?}");
    }
    assert!(rows.iter().any(|r| r.label == "-201U1 DP"), "the station's port");
    // Every member names itself: the segment's external interruption point, which has no device, by its name.
    assert!(rows.iter().all(|r| !r.label.trim().is_empty()), "{texts:?}");
    // The plug stands under the net it passes through (FT-1767) and, with its termination, in the segment.
    assert!(
        rows.iter().any(|r| r.label == "-3X1 DP"
            && r.key.starts_with("nets:logical/")
            && r.badges.iter().any(|b| b == "passes the net through")),
        "the plug in its net"
    );
    let connector = rows
        .iter()
        .find(|r| r.label == "-3X1 DP" && r.key.starts_with("nets:physical/"))
        .expect("the connector in the segment");
    assert!(
        connector.badges.iter().any(|b| b == "terminated"),
        "{:?}",
        connector.badges
    );
    assert!(
        rows.iter()
            .any(|r| r.role == NavRole::Cable.word() && r.label == "-3W1")
    );
    // The table: the station port's address cell typed as 3 is PROFIBUS station address 3.
    dispatch(&mut h, "navigator.table", json!({"kind": "bus", "on": true}));
    settle(&mut h);
    let at = h
        .get_all_by_label_contains("Address TBD")
        .next()
        .expect("the address cell of -201U1 DP")
        .rect()
        .center();
    // The editor opens with the cell's text selected, so what is typed replaces "TBD" (it gave "TBD3").
    double_click(&mut h, at);
    h.event(egui::Event::Text("3".to_owned()));
    settle(&mut h);
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert!(
        h.query_all_by_label_contains("Address 3,").count() >= 1,
        "the cell reads 3"
    );
    assert_eq!(
        h.query_all_by_label_contains("Address TBD").count(),
        0,
        "both rows of the port read 3"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-831: a new project's generic CPU has a DP port in its part that no symbol carries; its row offers Place port DP,
/// which arms the Place tool joined to the CPU; a click places the port, typed from the part, and the bus navigator
/// lists it.
#[test]
fn a_cpus_row_places_its_dp_port() {
    let tmp = Tmp::new();
    let mut h = crate::plc_workspace::new_project_with_cpu(&tmp);
    // A port symbol is drawn on a bus-topology page.
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "3", "type": "bus_topology", "title": "Bus"}),
    );
    support::wait_for_page(&mut h);
    let station = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Station.word())
        .expect("the CPU's row");
    h.get_by_label(&station.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Place port DP").click();
    settle(&mut h);
    // On a bus-topology page the port is a box with its socket (doc 10 §2.7): two corners.
    for corner in [(30, 30), (38, 36)] {
        crate::navigators::cursor_k(&mut h, corner);
        dispatch(&mut h, "canvas.activate", json!({}));
    }
    dispatch(&mut h, "tool.select", json!({}));
    dispatch(&mut h, "rail.show", json!({"pane": "bus"}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Bus);
    assert!(
        rows.iter()
            .any(|r| r.role == NavRole::BusPort.word() && r.label.ends_with(" DP")),
        "{:?}",
        rows.iter().map(|r| r.text.clone()).collect::<Vec<_>>()
    );
    // The row offers no second port.
    let station = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Station.word());
    if let Some(station) = station {
        dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
        settle(&mut h);
        h.get_by_label(&station.name).click_secondary();
        settle(&mut h);
        assert!(h.query_by_label("Place port DP").is_none());
    }
    h.state_mut().shell_mut().shutdown();
}

/// FT-434: on a bus-topology page the Wire tool arms, with the bus kind.
#[test]
fn the_wire_tool_draws_bus_lines_on_a_bus_topology_page() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "3");
    assert_eq!(h.state().shell().wire_kind(), Ok(Some("bus")));
    h.get_by_role_and_label(egui::Role::Tab, "Draw").click();
    settle(&mut h);
    support::bar_button(&h, "Wire").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "wire");
    h.state_mut().shell_mut().shutdown();
}
