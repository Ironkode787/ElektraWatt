// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F18 and F19 of the rc.1 test in the window, at 1920 × 1080: a station is made as a rack and filled (New station…,
//! its head part, Add module… with a count, Create, one undo step), and a bus device without PLC data is given to a
//! PLC from its symbol's menu, whose rack then opens and takes a module.

use crate::support;

use egui_kittest::kittest::{NodeT as _, Queryable};
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
    let menu = canvas_menu(h.state().shell(), MenuSpot::Object, None);
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
    let menu: Vec<String> = canvas_menu(h.state().shell(), MenuSpot::Object, None)
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

/// The PLC row of the selected device's Properties: its text.
fn plc_row(h: &egui_kittest::Harness<'static, ew_ui::app::App>) -> Option<String> {
    let sheet = h.state().shell().properties().sheet()?.clone();
    ew_ui::ew_viewmodel::properties::find_row(&sheet, "device/derived.plc")
        .map(|(_, r)| ew_ui::ew_viewmodel::properties::value_text(r))
}

/// The rc.2 persona run: a bus device is given to a PLC from where a person looks. A managed switch's Properties show
/// a PLC row "in no PLC"; its Bus panel row says the same; choosing New PLC… in the row makes it belong to a new PLC
/// without a rack (a toast with Undo says so, the Properties stay on it and name the PLC, its menu has no Rack…). A
/// generic PROFINET device's row then offers that PLC, and choosing it opens its rack as before.
#[test]
fn a_bus_device_is_given_to_a_plc_from_its_properties() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    crate::plc_workspace::draw_black_box(&mut h, "EW-GEN-ESW-8", (10, 10));
    dispatch(&mut h, "edit.select_all", json!({}));
    dispatch(&mut h, "props.show", json!({"on": true}));
    settle(&mut h);
    assert_eq!(plc_row(&h).as_deref(), Some("in no PLC: Add to PLC…"));
    dispatch(&mut h, "rail.show", json!({"pane": "bus"}));
    settle(&mut h);
    let rows = h
        .state_mut()
        .shell_mut()
        .shown_rows(ew_ui::ew_viewmodel::navigators::NavKind::Bus);
    assert!(
        rows.iter().any(|r| r.badges.iter().any(|b| b == "in no PLC")),
        "{:?}",
        rows.iter().map(|r| &r.text).collect::<Vec<_>>()
    );
    dispatch(&mut h, "props.row", json!({"to": "device/derived.plc"}));
    dispatch(&mut h, "props.edit", json!({}));
    settle(&mut h);
    h.get_by_label("New PLC…").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "a switch has no rack");
    let toast = h.state().shell().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.ends_with("belongs to PLC1 now"), "{toast}");
    assert_eq!(plc_row(&h).as_deref(), Some("PLC1"), "the Properties stay on the switch");
    let menu: Vec<String> = canvas_menu(h.state().shell(), MenuSpot::Object, None)
        .into_iter()
        .map(|i| i.label)
        .collect();
    assert!(!menu.iter().any(|l| l == "Rack…"), "{menu:?}");

    let reps = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        h.state()
            .shell()
            .canvas()
            .map(|c| c.page().tables.reps.iter().map(|r| (r.id, r.device)).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let before = reps(&h);
    crate::plc_workspace::draw_black_box(&mut h, "EW-GEN-PND-1", (40, 10));
    let (rep, device) = reps(&h)
        .into_iter()
        .find(|r| !before.contains(r))
        .expect("the new box");
    dispatch(&mut h, "select.set", json!({"ids": [rep.to_string()]}));
    dispatch(&mut h, "props.row", json!({"to": "device/derived.plc"}));
    dispatch(&mut h, "props.edit", json!({}));
    settle(&mut h);
    h.get_all_by_role(egui::Role::ListBoxOption)
        .find(|n| n.accesskit_node().label().as_deref() == Some("PLC1"))
        .expect("PLC1 among the choices")
        .click();
    settle(&mut h);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::Rack { station: Some(s), .. }) if Some(*s) == device),
        "{:?}",
        h.state().shell().dialog()
    );
    h.state_mut().shell_mut().shutdown();
}

/// The rc.2 persona run: in the new-station dialog a kind no library part is for says why and what to do, and a bus
/// device in no PLC is offered as the station; chosen, it is a station of the dialog's PLC and its rack is shown.
#[test]
fn the_new_station_dialog_offers_a_device_on_the_bus() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    crate::plc_workspace::draw_black_box(&mut h, "EW-GEN-PND-1", (10, 10));
    dispatch(&mut h, "plc.new_station", json!({}));
    dispatch(&mut h, "plc.rack_set", json!({"kind": "compact"}));
    settle(&mut h);
    if h.state().shell().station_parts().is_empty() {
        h.get_by_label("No part of this kind is in the libraries. New part… makes one.");
    }
    let (device, tag) = h.state().shell().station_candidates()[0].clone();
    h.get_by_label(&format!("Use {tag}, which is on the bus")).click();
    settle(&mut h);
    assert!(
        matches!(h.state().shell().dialog(), Some(Dialog::Rack { station: Some(s), draft: None, .. }) if *s == device),
        "{:?}",
        h.state().shell().dialog()
    );
    h.state_mut().shell_mut().shutdown();
}

/// The rc.2 persona run: Put it on a bus page in a project without a bus-topology page makes one, asking nothing, and
/// shows it with the station's port on the pointer; with two bus pages the button asks which, and the one chosen is
/// shown.
#[test]
fn put_it_on_a_bus_page_makes_the_page_or_asks_which() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 1);
    support::menu_node(&h, "Create").click();
    settle(&mut h);
    let station = match h.state().shell().dialog() {
        Some(Dialog::Rack { station: Some(s), .. }) => *s,
        other => panic!("{other:?}"),
    };
    h.get_by_label("Put it on a bus page").click();
    settle(&mut h);
    let shown = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        let shell = h.state().shell();
        shell.page_rows().get(shell.page_index()).map(|r| (r.folio.clone(), r.kind.clone()))
    };
    let (made, kind) = shown(&h).expect("a page");
    assert_eq!(kind, "bus_topology", "the bus page is made and shown");
    assert!(h.state().shell().dialog().is_none(), "{:?}", h.state().shell().dialog());
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "900", "type": "bus_topology", "title": "Field bus"}),
    );
    dispatch(&mut h, "plc.rack_editor", json!({"station": station.to_string()}));
    settle(&mut h);
    h.get_by_label("Put it on a bus page").click();
    settle(&mut h);
    support::menu_node(&h, &format!("{made} Bus"));
    support::menu_node(&h, "900 Field bus").click();
    settle(&mut h);
    assert_eq!(shown(&h).map(|s| s.0).as_deref(), Some("900"));
    h.state_mut().shell_mut().shutdown();
}

/// The topology boxes of `device` on the page shown.
fn topology_boxes(h: &egui_kittest::Harness<'static, ew_ui::app::App>, device: Uuid) -> usize {
    h.state()
        .shell()
        .canvas()
        .map_or(0, |c| c.page().tables.reps.iter().filter(|r| r.device == Some(device)).count())
}

/// A station of kind `kind` with head part `head` made in the new-station dialog and put on a bus page from its rack
/// with two corners of its first box: the station.
fn station_on_bus_page(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>, kind: &str, head: &str) -> Uuid {
    dispatch(h, "plc.new_station", json!({}));
    dispatch(h, "plc.rack_set", json!({"kind": kind, "head": head}));
    settle(h);
    support::menu_node(h, "Create").click();
    settle(h);
    let station = match h.state().shell().dialog() {
        Some(Dialog::Rack { station: Some(s), .. }) => *s,
        other => panic!("{other:?}"),
    };
    h.get_by_label("Put it on a bus page").click();
    settle(h);
    crate::navigators::cursor_k(h, (10, 10));
    dispatch(h, "canvas.activate", json!({}));
    crate::navigators::cursor_k(h, (18, 16));
    dispatch(h, "canvas.activate", json!({}));
    dispatch(h, "tool.select", json!({}));
    settle(h);
    station
}

/// The rc.2 persona runs: a valve terminal with a PROFINET bus node goes onto a bus page from its rack as the CPU
/// does, and a CPU with a PROFIBUS and a PROFINET interface stands there as both boxes, which the toast says.
#[test]
fn every_station_kind_goes_onto_a_bus_page_with_all_its_interfaces() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let terminal = station_on_bus_page(&mut h, "valve_terminal", "EW-GEN-VTM-2");
    assert_eq!(topology_boxes(&h, terminal), 1, "the terminal's bus node");
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    let cpu = station_on_bus_page(&mut h, "cpu", "EW-GEN-CPU-2");
    assert_eq!(topology_boxes(&h, cpu), 2, "DP and X1");
    let toast = h.state().shell().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(toast, "Placed as 2 boxes, one for each bus interface.");
    h.state_mut().shell_mut().shutdown();
}

/// A longer persona run found no field for the device name and IP: an empty bus-topology page says its three steps
/// where the pointer's hint stands, and a CPU's box on it shows its addresses in Main, named as its bus names them.
#[test]
fn a_bus_page_says_its_steps_and_a_box_its_addresses() {
    use ew_ui::ew_viewmodel::properties::{RowGroup, group_of, row_label};
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let cpu = station_on_bus_page(&mut h, "cpu", "EW-GEN-CPU-2");
    dispatch(&mut h, "select.set", json!({"ids": []}));
    settle(&mut h);
    let prompt = ew_ui::ew_viewmodel::command_bar::hud(h.state().shell()).prompt;
    assert!(prompt.starts_with("1 Place the stations"), "{prompt}");
    let rep = h
        .state()
        .shell()
        .canvas()
        .and_then(|c| c.page().tables.reps.iter().find(|r| r.device == Some(cpu)).map(|r| r.id))
        .expect("a box of the CPU");
    dispatch(&mut h, "select.set", json!({"ids": [rep.to_string()]}));
    dispatch(&mut h, "props.show", json!({"on": true}));
    settle(&mut h);
    let sheet = h.state().shell().properties().sheet().expect("the sheet").clone();
    let main: Vec<String> = sheet
        .sections
        .iter()
        .flat_map(|s| s.rows.iter().map(move |r| (s, r)))
        .filter(|(s, r)| group_of(s, r) == RowGroup::Main)
        .map(|(s, r)| row_label(s, r))
        .collect();
    assert!(
        main.iter().any(|l| l == "Station address" || l == "Device name and IP"),
        "{main:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The new-station dialog with the starter CPU as its head part and `modules` input modules in its rack, not made yet.
fn draft_with(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>, modules: u64) {
    dispatch(h, "plc.new_station", json!({}));
    dispatch(
        h,
        "plc.rack_set",
        json!({"head": "EW-GEN-CPU-1", "count": modules}),
    );
    dispatch(h, "plc.rack_add", json!({"slot": 1, "part": "EW-GEN-DI-1"}));
    settle(h);
}

/// A longer persona run: the rack dialog of a station drawn nowhere names it by its part, not by a tag of question
/// marks, and holds still while its slots are filled: its columns stand where they stood before the first module.
#[test]
fn the_rack_dialog_names_its_station_and_holds_still() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    dispatch(&mut h, "plc.new_station", json!({}));
    dispatch(&mut h, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    settle(&mut h);
    support::menu_node(&h, "Create").click();
    settle(&mut h);
    h.get_by_label_contains("Rack and slots of the station EW-GEN-CPU-1");
    let columns = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        ["Allowed slots", "Addresses", "Close"].map(|l| support::menu_node(h, l).rect().left())
    };
    let before = columns(&h);
    dispatch(&mut h, "plc.rack_add", json!({"slot": 1, "part": "EW-GEN-DI-1"}));
    settle(&mut h);
    settle(&mut h);
    let after = columns(&h);
    for (a, b) in before.iter().zip(&after) {
        assert!((a - b).abs() < 1.0, "the columns moved: {before:?} → {after:?}");
    }
    h.state_mut().shell_mut().shutdown();
}

/// The channel rows of the PLC panel: (channel, module, badges).
fn channel_rows(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>) -> Vec<(Uuid, Uuid, Vec<String>)> {
    use ew_ui::ew_viewmodel::navigators::NavKind;
    dispatch(h, "rail.show", json!({"pane": "plc"}));
    settle(h);
    let shell = h.state_mut().shell_mut();
    let _ = shell.shown_rows(NavKind::Plc);
    shell
        .nav(NavKind::Plc)
        .map(|n| {
            n.table()
                .map(|t| t.nodes.clone())
                .unwrap_or_default()
                .iter()
                .filter(|n| n.role == ew_ui::ew_viewmodel::ew_api::output::NavRole::Channel)
                .map(|n| (n.records[0], n.records[1], n.badges.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The tutorials lane: modules drawn from the rack are bound to their pins as modules drawn by hand: no channel stays
/// unplaced; a module's own Draw… draws it, bound.
#[test]
fn modules_drawn_from_the_rack_are_bound_to_their_pins() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 2);
    support::menu_node(&h, "Create").click();
    settle(&mut h);
    let station = match h.state().shell().dialog() {
        Some(Dialog::Rack { station: Some(s), .. }) => *s,
        other => panic!("{other:?}"),
    };
    let modules: Vec<Uuid> = h
        .state()
        .shell()
        .rack_view(station)
        .expect("the rack")
        .rows
        .iter()
        .filter_map(|r| r.module)
        .collect();
    // The second module's own Draw…, then Draw on this page for the rest.
    dispatch(&mut h, "plc.rack_slot", json!({"slot": 2, "op": "draw"}));
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "{:?}", h.state().shell().dialog());
    assert!(topology_boxes(&h, modules[1]) == 1, "the module's box");
    dispatch(&mut h, "plc.rack_editor", json!({"station": station.to_string()}));
    settle(&mut h);
    h.get_by_label("Draw on this page").click();
    settle(&mut h);
    let rows = channel_rows(&mut h);
    assert_eq!(rows.len(), 16, "two modules of eight channels");
    assert!(
        rows.iter().all(|(_, _, b)| !b.iter().any(|w| w == "unplaced")),
        "{rows:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// After Create, Draw on this page puts the station's box and each module's beside it in slot order on the page, in
/// one undo step; a second time there is nothing left to draw, and that is said in words.
#[test]
fn draw_on_this_page_draws_the_station_and_its_modules() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 3);
    support::menu_node(&h, "Create").click();
    settle(&mut h);
    let station = match h.state().shell().dialog() {
        Some(Dialog::Rack { station: Some(s), .. }) => *s,
        other => panic!("{other:?}"),
    };
    let drawn = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        let reps = h
            .state()
            .shell()
            .canvas()
            .map(|c| c.page().tables.reps.clone())
            .unwrap_or_default();
        let mut boxes: Vec<(i64, Uuid)> = reps.iter().filter_map(|r| Some((r.body[0], r.device?))).collect();
        boxes.sort_by_key(|b| b.0);
        boxes.into_iter().map(|(_, d)| d).collect::<Vec<_>>()
    };
    let modules: Vec<Uuid> = h
        .state()
        .shell()
        .rack_view(station)
        .expect("the rack")
        .rows
        .iter()
        .filter_map(|r| r.module)
        .collect();
    h.get_by_label("Draw on this page").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "the page is shown");
    let mut expected = vec![station];
    expected.extend(modules);
    assert_eq!(
        drawn(&h),
        expected,
        "the station, then its modules in slot order, left to right"
    );
    dispatch(&mut h, "plc.rack_editor", json!({"station": station.to_string()}));
    settle(&mut h);
    h.get_by_label("Draw on this page").click();
    settle(&mut h);
    h.get_by_label_contains("are drawn already");
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    assert!(drawn(&h).is_empty(), "one undo step takes every box back");
    h.state_mut().shell_mut().shutdown();
}

/// New part… presets what the dialog was asked for: from Remote I/O a station part with a device's bus interface, from
/// PLC (CPU) one whose interface leads the bus, from a slot's Add module… a module part; saved, the new head part is
/// the draft's head again.
#[test]
fn new_part_from_the_rack_starts_as_what_was_asked() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let draft_text =
        |h: &egui_kittest::Harness<'static, ew_ui::app::App>, field: &str| match h.state().shell().dialog() {
            Some(Dialog::PartEdit(e)) => e.draft.text(field).to_owned(),
            other => panic!("the part editor: {other:?}"),
        };
    dispatch(&mut h, "plc.new_station", json!({}));
    dispatch(&mut h, "plc.rack_set", json!({"kind": "remote"}));
    settle(&mut h);
    h.get_by_label("New part…").click();
    settle(&mut h);
    assert_eq!(draft_text(&h, "plc.role"), "station");
    assert_eq!(draft_text(&h, "bus.0.role"), "device");
    for (field, value) in [
        ("manufacturer", "Acme"),
        ("part_number", "RIO-9"),
        ("description", "Remote head"),
    ] {
        dispatch(&mut h, "part.field", json!({"field": field, "value": value}));
    }
    dispatch(&mut h, "part.save", json!({}));
    settle(&mut h);
    let head = h
        .state()
        .shell()
        .station_draft()
        .and_then(|d| d.head)
        .expect("back with a head part");
    let parts = h.state().shell().station_parts();
    assert!(
        parts
            .iter()
            .any(|p| p.copy == head && p.text.starts_with("RIO-9")),
        "the new part is the head"
    );

    dispatch(&mut h, "plc.rack_set", json!({"kind": "cpu"}));
    settle(&mut h);
    h.get_by_label("New part…").click();
    settle(&mut h);
    assert_eq!(draft_text(&h, "bus.0.role"), "controller");
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);

    dispatch(&mut h, "plc.new_station", json!({}));
    dispatch(&mut h, "part.new", json!({"plc": "module"}));
    settle(&mut h);
    assert_eq!(draft_text(&h, "plc.role"), "module");
    h.state_mut().shell_mut().shutdown();
}

/// At 1920 × 1080 the choices of what the station is stand on one line, and with twelve modules the free slot's
/// Add module… stays inside the window above Create.
#[test]
fn the_new_station_dialog_fits_at_1920() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 12);
    let rows: Vec<f32> = ["PLC (CPU)", "Remote I/O", "Valve terminal", "Compact device"]
        .iter()
        .map(|l| h.get_by_label(l).rect().center().y)
        .collect();
    assert!(
        rows.iter().all(|y| (*y - rows[0]).abs() < 1.0),
        "one line: {rows:?}"
    );
    let add = h.get_by_label("Add module… 13").rect();
    let create = support::menu_node(&h, "Create").rect();
    assert!(
        add.top() >= 0.0 && add.bottom() <= create.top(),
        "Add module… {add:?} is out of sight above Create {create:?}"
    );
    h.state_mut().shell_mut().shutdown();
}
