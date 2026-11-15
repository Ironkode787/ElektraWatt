// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F18 and F19 of the rc.1 test in the window, at 1920 × 1080: a station is made as a rack and filled (New station…,
//! its head part, Add module… with a count, Create, one undo step), and a bus device without PLC data is given to a
//! PLC from its symbol's menu, whose rack then opens and takes a module.

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_ui::ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_ui::ew_viewmodel::ew_api::output::Uuid;
use ew_ui::ew_viewmodel::shell::Dialog;
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle, wait_for_page};

/// A new project with page 101, the PLC panel shown.
fn new_project(tmp: &Tmp) -> egui_kittest::Harness<'static, ew_ui::app::App> {
    let mut h = harness(tmp, Scripted::default(), None);
    settle(&mut h);
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("rack-first.ew").to_string_lossy(), "template": "builtin"}),
    );
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    h
}

/// F18: the PLC panel's New station… opens the rack dialog in its new-station state; the CPU chosen as its head part,
/// Add module… on slot 1 with a count of two puts two input modules into slots 1 and 2; Create makes the station and
/// its modules, drawn nowhere yet, and the dialog shows the made station's rack with what comes next; one Undo takes
/// it all back.
#[test]
fn a_station_is_made_as_a_rack_and_filled() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    // The panel's + (its empty state offers the same).
    h.get_all_by_label("New station…")
        .next()
        .expect("New station…")
        .click();
    settle(&mut h);
    assert!(
        matches!(
            h.state().shell().dialog(),
            Some(Dialog::Rack { draft: Some(_), .. })
        ),
        "{:?}",
        h.state().shell().dialog()
    );
    h.get_by_label("New station");
    let cpu = h
        .state()
        .shell()
        .station_parts()
        .into_iter()
        .find(|p| p.text.starts_with("EW-GEN-CPU-1"))
        .expect("the starter CPU among the PLC head parts");
    dispatch(
        &mut h,
        "plc.rack_set",
        json!({"head": cpu.copy.to_string(), "count": 2}),
    );
    settle(&mut h);
    h.get_by_label("Add module… 1").click();
    settle(&mut h);
    let di = h
        .state()
        .shell()
        .module_parts()
        .into_iter()
        .find(|p| p.text.starts_with("EW-GEN-DI-1"))
        .expect("the input module");
    h.get_by_label(&format!("{} · {}", di.text, di.gives)).click();
    settle(&mut h);
    let draft = h.state().shell().station_draft().cloned().expect("the draft");
    assert_eq!(draft.modules, vec![di.copy, di.copy], "two modules at once");
    support::menu_node(&h, "Create").click();
    settle(&mut h);
    let station = match h.state().shell().dialog() {
        Some(Dialog::Rack {
            station: Some(s),
            draft: None,
            ..
        }) => *s,
        other => panic!("{other:?}"),
    };
    let view = h.state().shell().rack_view(station).expect("its rack");
    let modules: Vec<(u32, &str)> = view
        .rows
        .iter()
        .filter(|r| r.module.is_some())
        .map(|r| (r.slot, r.gives.as_str()))
        .collect();
    assert_eq!(modules, [(1, "8 DI"), (2, "8 DI")]);
    assert!(
        view.rows.iter().all(|r| r.module.is_none() || !r.drawn),
        "drawn nowhere yet"
    );
    h.get_by_label("Draw on this page");
    h.get_by_label("Put it on a bus page");
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    assert!(h.state().shell().rack_view(station).is_none(), "one undo step");
    h.state_mut().shell_mut().shutdown();
}

/// F19: a PROFINET device whose part says nothing of a PLC, drawn on the page: its symbol's menu offers Add to a new
/// PLC; chosen, it is a station with an open rack whose dialog says that its part does not describe its modules, and
/// Add module… puts a module into its first slot. Its menu then names its PLC and offers Rack….
#[test]
fn a_bus_device_is_given_to_a_plc_from_its_menu() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    crate::plc_workspace::draw_black_box(&mut h, "EW-GEN-PND-1", (10, 10));
    assert!(h.state().shell().dialog().is_none(), "no station yet");
    dispatch(&mut h, "edit.select_all", json!({}));
    settle(&mut h);
    // The symbol's menu, as a right-click on it opens it.
    let menu = canvas_menu(h.state().shell(), MenuSpot::Object);
    let add = menu
        .iter()
        .find(|i| i.label == "Add to a new PLC")
        .unwrap_or_else(|| panic!("{:?}", menu.iter().map(|i| &i.label).collect::<Vec<_>>()));
    assert!(add.enabled, "{:?}", add.reason);
    let device: Uuid = add.params["device"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .expect("the device");
    dispatch(&mut h, add.id, add.params.clone());
    settle(&mut h);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::Rack { station: Some(s), .. }) if *s == device),
        "{:?}",
        h.state().shell().dialog()
    );
    let device = &device;
    h.get_by_label("Its part does not describe its modules: add them as they are built.");
    h.get_by_label("Add module… 1").click();
    settle(&mut h);
    let di = h
        .state()
        .shell()
        .module_parts()
        .into_iter()
        .find(|p| p.text.starts_with("EW-GEN-DI-1"))
        .expect("the input module");
    h.get_by_label(&format!("{} · {}", di.text, di.gives)).click();
    settle(&mut h);
    let view = h.state().shell().rack_view(*device).expect("its rack");
    assert_eq!(view.rows.iter().filter(|r| r.module.is_some()).count(), 1);
    dispatch(&mut h, "ui.dismiss", json!({}));
    let menu: Vec<String> = canvas_menu(h.state().shell(), MenuSpot::Object)
        .into_iter()
        .map(|i| i.label)
        .collect();
    assert!(
        menu.iter().any(|l| l == "Rack…") && menu.iter().any(|l| l.starts_with("PLC: PLC")),
        "{menu:?}"
    );
    assert!(!menu.iter().any(|l| l.starts_with("Add to")), "{menu:?}");
    h.state_mut().shell_mut().shutdown();
}
