// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! F18 and F19 of a test in the window, at 1920 × 1080: a station is made as a rack and filled (New station…,
//! its head part and its modules from the catalogue, Create, one undo step), and a bus device without PLC data is
//! given to a PLC from its symbol's menu, whose view then opens and takes a module. The rack is a view in a tab
//! beside the page tabs (doc 10 §6). Not here: a click on the selection that brings its Properties back, the Bus
//! panel brought to the front and Fix all of unconnected pins (`ew-viewmodel`'s `plc_new_project.rs`).

use crate::support;

use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::shell::Dialog;
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

/// The station whose view is on show.
fn station_of(h: &egui_kittest::Harness<'static, ew_ui::app::App>) -> Uuid {
    h.state()
        .shell()
        .station_tab()
        .and_then(|t| t.station)
        .unwrap_or_else(|| panic!("a made station's view: {:?}", h.state().shell().station_tab()))
}

/// A double click on the catalogue's part whose row starts with `number`.
fn take(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>, number: &str) {
    let at = h
        .query_all_by_role(egui::Role::ListItem)
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(number)))
        .unwrap_or_else(|| panic!("the catalogue lists {number}"))
        .rect()
        .center();
    // Two clicks at one place without time between them: a double click.
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    h.run_ok();
    settle(h);
}

/// F18: the PLC panel's New station… opens the view of a station not made yet in a tab beside the page; a double click
/// on the CPU in the catalogue makes it the head, two on the input module put two modules into slots 1 and 2; Create
/// makes the station and its modules, drawn nowhere yet, and the view shows the made station's rack with what is left
/// to do; one Undo takes it all back, and the tab with it.
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
        h.state().shell().dialog().is_none(),
        "no dialog lies over the window"
    );
    assert!(
        h.state().shell().station_draft().is_some(),
        "the new station's view"
    );
    // Nothing is written before Create.
    let revision = h.state().shell().revision();
    take(&mut h, "EW-GEN-CPU-1");
    take(&mut h, "EW-GEN-DI-1");
    take(&mut h, "EW-GEN-DI-1");
    let draft = h.state().shell().station_draft().cloned().expect("the draft");
    assert!(draft.head.is_some(), "the CPU is the head");
    assert_eq!(draft.modules.len(), 2, "two modules, one per double click");
    assert_eq!(h.state().shell().revision(), revision, "still nothing written");
    h.get_by_label("Create").click();
    settle(&mut h);
    let station = station_of(&h);
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
    h.get_by_label("Draw on page 101");
    h.get_by_label("Put it on a new bus page");
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    assert!(h.state().shell().rack_view(station).is_none(), "one undo step");
    assert!(h.state().shell().station_tab().is_none(), "its tab went with it");
    h.state_mut().shell_mut().shutdown();
}

/// F19: a PROFINET device whose part says nothing of a PLC, drawn on the page: its symbol's menu offers Add to a new
/// PLC; chosen, it is a station with an open rack whose view says that its part does not describe its modules, and a
/// double click on a module of the catalogue puts it into its first slot. Its menu then names its PLC and offers
/// Rack….
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
    assert_eq!(station_of(&h), device, "its view opens");
    let device = &device;
    h.get_by_label("Its part does not describe its modules: add them as they are built.");
    take(&mut h, "EW-GEN-DI-1");
    let view = h.state().shell().rack_view(*device).expect("its rack");
    assert_eq!(view.rows.iter().filter(|r| r.module.is_some()).count(), 1);
    dispatch(&mut h, "tab.close", json!({}));
    dispatch(&mut h, "edit.select_all", json!({}));
    settle(&mut h);
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
    ew_viewmodel::properties::find_row(&sheet, "device/derived.plc")
        .map(|(_, r)| ew_viewmodel::properties::value_text(r))
}

/// A bus device is given to a PLC from where a person looks. A managed switch's Properties show
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
        .shown_rows(ew_viewmodel::navigators::NavKind::Bus);
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
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert!(toast.ends_with("belongs to PLC1 now."), "{toast}");
    assert_eq!(
        plc_row(&h).as_deref(),
        Some("PLC1"),
        "the Properties stay on the switch"
    );
    let menu: Vec<String> = canvas_menu(h.state().shell(), MenuSpot::Object, None)
        .into_iter()
        .map(|i| i.label)
        .collect();
    assert!(!menu.iter().any(|l| l == "Rack…"), "{menu:?}");

    let reps = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        h.state()
            .shell()
            .canvas()
            .map(|c| {
                c.page()
                    .tables
                    .reps
                    .iter()
                    .map(|r| (r.id, r.device))
                    .collect::<Vec<_>>()
            })
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
    assert_eq!(Some(station_of(&h)), device, "its view opens");
    h.state_mut().shell_mut().shutdown();
}

/// In a new station's view a kind no library part is for says why and what to do, and a bus
/// device in no PLC is offered as the station; chosen, it is a station of the view's PLC and its rack is shown.
#[test]
fn the_new_station_view_offers_a_device_on_the_bus() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    crate::plc_workspace::draw_black_box(&mut h, "EW-GEN-PND-1", (10, 10));
    dispatch(&mut h, "plc.new_station", json!({}));
    dispatch(&mut h, "plc.rack_set", json!({"kind": "compact"}));
    settle(&mut h);
    if h.state().shell().station_catalogue().is_empty() {
        h.get_by_label("No part of this kind is in the libraries. New part… makes one.");
    }
    let (device, tag) = h.state().shell().station_candidates()[0].clone();
    h.get_by_label(&format!("Use {tag}, which is on the bus")).click();
    settle(&mut h);
    assert_eq!(station_of(&h), device);
    assert!(
        h.state().shell().station_draft().is_none(),
        "made: no draft is left"
    );
    h.state_mut().shell_mut().shutdown();
}

/// In a project without a bus-topology page the station's bus side goes onto a new one, asking
/// nothing, which is shown with the station's port on the pointer; with bus pages the button asks which (a
/// side lands on the page the person picks), and the one chosen is shown.
#[test]
fn put_it_on_a_bus_page_makes_the_page_or_asks_which() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 1);
    h.get_by_label("Create").click();
    settle(&mut h);
    let station = station_of(&h);
    h.get_by_label("Put it on a new bus page").click();
    settle(&mut h);
    let shown = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        let shell = h.state().shell();
        shell
            .page_rows()
            .get(shell.page_index())
            .map(|r| (r.folio.clone(), r.kind.clone()))
    };
    let (made, kind) = shown(&h).expect("a page");
    assert_eq!(kind, "bus_topology", "the bus page is made and shown");
    assert!(
        h.state().shell().station_tab().is_none(),
        "the page is on show, the view's tab beside it"
    );
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "900", "type": "bus_topology", "title": "Field bus"}),
    );
    dispatch(&mut h, "plc.rack_editor", json!({"station": station.to_string()}));
    settle(&mut h);
    h.get_by_label("Put it on a bus page…").click();
    settle(&mut h);
    support::menu_node(&h, &format!("{made} Bus"));
    support::menu_node(&h, "900 Field bus").click();
    settle(&mut h);
    assert_eq!(shown(&h).map(|s| s.0).as_deref(), Some("900"));
    h.state_mut().shell_mut().shutdown();
}

/// The topology boxes of `device` on the page shown.
fn topology_boxes(h: &egui_kittest::Harness<'static, ew_ui::app::App>, device: Uuid) -> usize {
    h.state().shell().canvas().map_or(0, |c| {
        c.page()
            .tables
            .reps
            .iter()
            .filter(|r| r.device == Some(device))
            .count()
    })
}

/// A station of kind `kind` with head part `head` made in the new-station dialog and put on a bus page from its rack
/// (a new one, else the first one listed) with two corners of its first box, the first on grid row `row` (a box never
/// lands on another device): the station.
fn station_on_bus_page(
    h: &mut egui_kittest::Harness<'static, ew_ui::app::App>,
    kind: &str,
    head: &str,
    row: i64,
) -> Uuid {
    dispatch(h, "plc.new_station", json!({}));
    dispatch(h, "plc.rack_set", json!({"kind": kind, "head": head}));
    settle(h);
    h.get_by_label("Create").click();
    settle(h);
    let station = station_of(h);
    match h.query_by_label("Put it on a new bus page") {
        Some(new) => new.click(),
        None => {
            h.get_by_label("Put it on a bus page…").click();
            settle(h);
            let first = h.state().shell().bus_pages()[0].clone();
            support::menu_node(h, format!("{} {}", first.0, first.1).trim()).click();
        }
    }
    settle(h);
    crate::navigators::cursor_k(h, (10, row));
    dispatch(h, "canvas.activate", json!({}));
    crate::navigators::cursor_k(h, (18, row + 6));
    dispatch(h, "canvas.activate", json!({}));
    dispatch(h, "tool.select", json!({}));
    settle(h);
    station
}

/// A valve terminal with a PROFINET bus node goes onto a bus page from its rack as the CPU
/// does, and a CPU with a PROFIBUS and a PROFINET interface stands there as both boxes, which the toast says.
#[test]
fn every_station_kind_goes_onto_a_bus_page_with_all_its_interfaces() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let terminal = station_on_bus_page(&mut h, "valve_terminal", "EW-GEN-VTM-2", 10);
    assert_eq!(topology_boxes(&h, terminal), 1, "the terminal's bus node");
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    let cpu = station_on_bus_page(&mut h, "cpu", "EW-GEN-CPU-2", 24);
    assert_eq!(topology_boxes(&h, cpu), 2, "DP and X1");
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert_eq!(toast, "Placed as 2 boxes, one for each bus interface.");
    h.state_mut().shell_mut().shutdown();
}

/// An empty bus-topology page says its three steps
/// where the pointer's hint stands, and a CPU's box on it shows its addresses in Main, named as its bus names them.
#[test]
fn a_bus_page_says_its_steps_and_a_box_its_addresses() {
    use ew_viewmodel::properties::{RowGroup, group_of, row_label};
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let cpu = station_on_bus_page(&mut h, "cpu", "EW-GEN-CPU-2", 10);
    dispatch(&mut h, "select.set", json!({"ids": []}));
    settle(&mut h);
    let prompt = ew_viewmodel::command_bar::hud(h.state().shell()).prompt;
    assert!(prompt.starts_with("1 Put the stations here"), "{prompt}");
    let rep = h
        .state()
        .shell()
        .canvas()
        .and_then(|c| {
            c.page()
                .tables
                .reps
                .iter()
                .find(|r| r.device == Some(cpu))
                .map(|r| r.id)
        })
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
        main.iter()
            .any(|l| l == "Station address" || l == "Device name and IP"),
        "{main:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A new station's view with the starter CPU as its head part and `modules` input modules in its rack, not made yet.
fn draft_with(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>, modules: u64) {
    dispatch(h, "plc.new_station", json!({}));
    dispatch(h, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    for _ in 0..modules {
        dispatch(h, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    }
    settle(h);
}

/// The view of a station drawn nowhere names it by its part, not by a tag of question marks, in
/// its tab and in its header, and holds still while its slots are filled: its columns and its catalogue stand where
/// they stood before the first module.
#[test]
fn the_station_view_names_its_station_and_holds_still() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    dispatch(&mut h, "plc.new_station", json!({}));
    dispatch(&mut h, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    settle(&mut h);
    h.get_by_label("Create").click();
    settle(&mut h);
    h.get_by_label("EW-GEN-CPU-1 · rack");
    let columns = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        // The view's own texts, not the command bar's buttons of the same name.
        ["Gives", "Addresses", "Drawn on", "Catalogue"].map(|l| {
            h.get_all_by_label(l)
                .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Label)
                .map(|n| n.rect().left())
                .next()
                .unwrap_or_else(|| panic!("the view's {l}"))
        })
    };
    let before = columns(&h);
    take(&mut h, "EW-GEN-DI-1");
    settle(&mut h);
    let after = columns(&h);
    for (a, b) in before.iter().zip(&after) {
        assert!((a - b).abs() < 1.0, "the columns moved: {before:?} → {after:?}");
    }
    h.state_mut().shell_mut().shutdown();
}

/// Draw on this page put the station on page 1 without a word. From a view opened on a page that is no
/// electrical page, Draw on an electrical page… lists the schematic pages by folio and title, and New page…, which
/// makes a schematic page and draws there; the page drawn on is shown, and the station's tab stays beside it.
#[test]
fn draw_on_a_page_lists_the_pages() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "201", "type": "fluid", "title": "Pneumatics"}),
    );
    draft_with(&mut h, 1);
    h.get_by_label("Create").click();
    settle(&mut h);
    let station = station_of(&h);
    h.get_by_label("Draw on an electrical page…").click();
    settle(&mut h);
    support::menu_node(&h, "101 Control");
    support::menu_node(&h, "New page…").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.station_tab().is_none(), "the page is on show");
    let row = shell.page_rows().get(shell.page_index()).expect("a page");
    assert_eq!(row.kind, "multi_line", "a new schematic page");
    assert_ne!(row.folio, "101");
    assert_eq!(topology_boxes(&h, station), 1, "the station drawn there");
    let tabs = shell.area().expect("the canvas area").focused_group();
    assert!(
        tabs.tabs.iter().any(|t| t.station.is_some()),
        "the station's tab stays open"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The channel rows of the PLC panel: (channel, module, badges).
fn channel_rows(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>) -> Vec<(Uuid, Uuid, Vec<String>)> {
    use ew_viewmodel::navigators::NavKind;
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
                .filter(|n| n.role == ew_viewmodel::ew_api::output::NavRole::Channel)
                .map(|n| (n.records[0], n.records[1], n.badges.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The tutorials lane: modules drawn from the rack are bound to their pins as modules drawn by hand: no channel stays
/// unplaced; a module's own Draw on a page… draws it, bound.
#[test]
fn modules_drawn_from_the_rack_are_bound_to_their_pins() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 2);
    h.get_by_label("Create").click();
    settle(&mut h);
    let station = station_of(&h);
    let modules: Vec<Uuid> = h
        .state()
        .shell()
        .rack_view(station)
        .expect("the rack")
        .rows
        .iter()
        .filter_map(|r| r.module)
        .collect();
    // The second module's own Draw on a page…, then the To do list's for the rest.
    dispatch(
        &mut h,
        "plc.rack_slot",
        json!({"slot": 2, "op": "draw", "folio": "101"}),
    );
    settle(&mut h);
    assert!(h.state().shell().station_tab().is_none(), "page 101 is on show");
    assert!(topology_boxes(&h, modules[1]) == 1, "the module's box");
    dispatch(&mut h, "plc.rack_editor", json!({"station": station.to_string()}));
    settle(&mut h);
    h.get_by_label("Draw on page 101").click();
    settle(&mut h);
    let rows = channel_rows(&mut h);
    assert_eq!(rows.len(), 16, "two modules of eight channels");
    assert!(
        rows.iter().all(|(_, _, b)| !b.iter().any(|w| w == "unplaced")),
        "{rows:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// After Create, Draw on page 101 (the page the view was opened from) puts the station's box and each module's on
/// that page in one undo step, the input modules in slot order along the foot, a row over the first where the
/// drawing area is full, the station above them; then nothing is left to draw: the view's To do list no longer
/// offers it, and each row names its page.
#[test]
fn draw_on_a_page_draws_the_station_and_its_modules() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 3);
    h.get_by_label("Create").click();
    settle(&mut h);
    let station = station_of(&h);
    // The boxes from the foot upward, each row left to right.
    let drawn = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        let reps = h
            .state()
            .shell()
            .canvas()
            .map(|c| c.page().tables.reps.clone())
            .unwrap_or_default();
        let mut boxes: Vec<(i64, i64, Uuid)> = reps
            .iter()
            .filter_map(|r| Some((-r.body[1], r.body[0], r.device?)))
            .collect();
        boxes.sort();
        boxes.into_iter().map(|(_, _, d)| d).collect::<Vec<_>>()
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
    h.get_by_label("Draw on page 101").click();
    settle(&mut h);
    assert!(h.state().shell().station_tab().is_none(), "the page is shown");
    let mut expected = modules;
    expected.push(station);
    assert_eq!(
        drawn(&h),
        expected,
        "the modules in slot order from the foot, the station above them"
    );
    // The boxes' captions (tags, part numbers) stand inside the frame's drawing area, off its column numbers.
    {
        let shell = h.state().shell();
        let page = shell.canvas().expect("page 101").page();
        let cells = page.cells.as_ref().expect("the frame's cells");
        assert!(!page.texts.is_empty(), "the boxes' tags and part numbers");
        for t in page.texts.iter() {
            for corner in [
                (t.rect.min().x.0, t.rect.min().y.0),
                (t.rect.max().x.0, t.rect.max().y.0),
            ] {
                assert!(
                    cells.cell_at(page.snapper.page_to_sheet(corner)).is_some(),
                    "a text on the frame's bands: {t:?}"
                );
            }
        }
    }
    dispatch(&mut h, "plc.rack_editor", json!({"station": station.to_string()}));
    settle(&mut h);
    assert!(
        h.query_by_label("Draw on page 101").is_none(),
        "nothing is left to draw"
    );
    assert_eq!(
        h.query_all_by_label("Page 101").count(),
        4,
        "the station and its three modules each name their page"
    );
    dispatch(&mut h, "tab.close", json!({}));
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    assert!(drawn(&h).is_empty(), "one undo step takes every box back");
    h.state_mut().shell_mut().shutdown();
}

/// docs/design/06a-drafting.md §7.5 and §9.3 row 7: drawn from the station view, an input module is one row of pins on
/// the upper edge of a low, wide box at the foot of the drawing area, `CARD_PIN_PITCH` apart in channel order, its
/// supply pin on its left side; an output module the same on the lower edge of a box in the upper half of the page.
#[test]
fn a_module_drawn_from_the_station_view_is_one_row_of_pins_along_one_edge() {
    use ew_viewmodel::ew_api::drafting::consts::{CARD_PIN_PITCH, TICKS_PER_STEP};
    use ew_viewmodel::ew_api::geometry::Dir;
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 1);
    dispatch(&mut h, "plc.rack_add", json!({"part": "EW-GEN-DO-1"}));
    settle(&mut h);
    h.get_by_label("Create").click();
    settle(&mut h);
    let station = station_of(&h);
    let modules: Vec<Uuid> = h
        .state()
        .shell()
        .rack_view(station)
        .expect("the rack")
        .rows
        .iter()
        .filter_map(|r| r.module)
        .collect();
    h.get_by_label("Draw on page 101").click();
    settle(&mut h);
    let channels = channel_rows(&mut h);
    let shell = h.state().shell();
    let page = shell.canvas().expect("page 101").page();
    let area = ew_viewmodel::tools::ip::drawing_area(page).expect("the frame's drawing area");
    let (num, den) = page.tables.grid;
    let step = |v: i64| (v * den + num / 2).div_euclid(num);
    let pitch = i64::from(CARD_PIN_PITCH / TICKS_PER_STEP);
    let card = |device: Uuid| {
        page.tables
            .reps
            .iter()
            .find(|r| r.device == Some(device))
            .and_then(|r| r.black_box.clone())
            .expect("the module's box")
    };
    for (module, edge) in [(modules[0], Dir::Up), (modules[1], Dir::Down)] {
        let b = card(module);
        let (top, bottom, left) = b
            .outline
            .iter()
            .fold((i64::MAX, i64::MIN, i64::MAX), |(t, d, l), p| {
                (t.min(p.1), d.max(p.1), l.min(p.0))
            });
        let row: Vec<(u16, (i64, i64))> = b.cps.iter().filter(|c| c.2 == edge).map(|c| (c.0, c.1)).collect();
        assert_eq!(
            row.len(),
            channels.iter().filter(|c| c.1 == module).count(),
            "every channel's pin in the row: {b:?}"
        );
        let y = if edge == Dir::Up { top } else { bottom };
        assert!(row.iter().all(|(_, p)| p.1 == y), "on one edge: {row:?}");
        assert!(
            row.windows(2).all(|w| w[0].0 < w[1].0),
            "in channel order: {row:?}"
        );
        let steps: Vec<i64> = row.iter().map(|(_, p)| step(p.0)).collect();
        assert!(steps.windows(2).all(|w| w[1] - w[0] == pitch), "{steps:?}");
        assert!(
            b.cps
                .iter()
                .filter(|c| c.2 != edge)
                .all(|c| c.2 == Dir::Left && c.1.0 == left),
            "the supply pins on the left side: {b:?}"
        );
        if edge == Dir::Up {
            assert!(
                bottom <= area[3] && area[3] - bottom < bottom - top,
                "at the foot of the drawing area: {b:?} {area:?}"
            );
        } else {
            assert!(
                bottom < (area[1] + area[3]) / 2,
                "in the upper half: {b:?} {area:?}"
            );
        }
    }
    h.state_mut().shell_mut().shutdown();
}

/// New part… at the catalogue's foot presets what the view was asked for: for Remote I/O a station part with a
/// device's bus interface, for PLC (CPU) one whose interface leads the bus, for a station with its head a module
/// part; saved, the new head part is the draft's head.
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
    let rack = h.state().shell().shown_rack().expect("the draft's rack");
    assert_eq!(rack.head.module, Some(head));
    assert!(
        rack.head.part.starts_with("RIO-9"),
        "the new part is the head: {}",
        rack.head.part
    );

    // With its head chosen the view lists modules, and New part… starts a module part.
    h.get_by_label("New part…").click();
    settle(&mut h);
    assert_eq!(draft_text(&h, "plc.role"), "module");
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);

    dispatch(&mut h, "plc.rack_set", json!({"kind": "cpu"}));
    settle(&mut h);
    h.get_by_label("New part…").click();
    settle(&mut h);
    assert_eq!(draft_text(&h, "bus.0.role"), "controller");
    h.state_mut().shell_mut().shutdown();
}

/// At 1920 × 1080 the choices of what the station is stand on one line, and with twelve modules Create, the To do list
/// and the catalogue's New part… stay inside the window: the rack scrolls sideways, the lists under it down.
#[test]
fn the_new_station_view_fits_at_1920() {
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
    let window = h.ctx.content_rect();
    for label in ["Create", "New part…", "Catalogue"] {
        let rect = h.get_by_label(label).rect();
        assert!(
            window.contains_rect(rect),
            "{label} {rect:?} is out of the window {window:?}"
        );
    }
    h.state_mut().shell_mut().shutdown();
}

/// A station of kind `kind` with head part `head` made in a new station's view and drawn nowhere; its tab closed.
fn made_station(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>, kind: &str, head: &str) -> Uuid {
    dispatch(h, "plc.new_station", json!({}));
    dispatch(h, "plc.rack_set", json!({"kind": kind, "head": head}));
    settle(h);
    h.get_by_label("Create").click();
    settle(h);
    let station = station_of(h);
    dispatch(h, "tab.close", json!({}));
    settle(h);
    station
}

/// The Put a station on this page form that is open: its choices (value, words) and the one chosen.
fn put_station_form(h: &egui_kittest::Harness<'static, ew_ui::app::App>) -> (Vec<(String, String)>, String) {
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) if f.action == "bus.put_station" => {
            let field = &f.fields[0];
            (field.options.clone(), field.value.clone())
        }
        other => panic!("no Put a station form: {other:?}"),
    }
}

/// Two corners of a box on the page shown, then the name of the tool in front.
fn two_corners(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>) -> String {
    crate::navigators::cursor_k(h, (10, 10));
    dispatch(h, "canvas.activate", json!({}));
    crate::navigators::cursor_k(h, (18, 16));
    dispatch(h, "canvas.activate", json!({}));
    settle(h);
    h.state().shell().tool().name().to_owned()
}

/// A CPU stands on a schematic page and is wanted on the bus page. Placing its part
/// from the device browser there asks first, "Put <station> here" chosen, or "A new station"; Put here draws the station
/// the project has, a box for each interface, and the tool gives way instead of taking it again. The Black box tool on
/// the bus page then offers the station still missing there, not the one placed last.
#[test]
fn a_station_the_project_has_is_put_on_the_bus_page_not_made_again() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let cpu = made_station(&mut h, "cpu", "EW-GEN-CPU-2");
    let terminal = made_station(&mut h, "valve_terminal", "EW-GEN-VTM-2");
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "900", "type": "bus_topology", "title": "Field bus"}),
    );
    dispatch(&mut h, "page.goto", json!({"folio": "900"}));
    settle(&mut h);

    // The part of the CPU placed as the device browser places it: asked first, the CPU chosen, the terminal (another
    // part) not offered.
    dispatch(&mut h, "place.pick", json!({"part": "EW-GEN-CPU-2"}));
    settle(&mut h);
    let (choices, chosen) = put_station_form(&h);
    assert_eq!(chosen, cpu.to_string(), "{choices:?}");
    assert!(
        choices.iter().any(|(v, w)| v == "new" && w == "A new station"),
        "{choices:?}"
    );
    assert!(
        !choices.iter().any(|(v, _)| *v == terminal.to_string()),
        "{choices:?}"
    );
    // "A new station" places the part as before, a new device.
    dispatch(&mut h, "bus.put_station", json!({"station": "new"}));
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    assert_eq!(h.state().shell().tool().name(), "black_box");
    dispatch(&mut h, "tool.select", json!({}));
    // Asked again, Put here.
    dispatch(&mut h, "place.pick", json!({"part": "EW-GEN-CPU-2"}));
    settle(&mut h);
    put_station_form(&h);
    support::menu_node(&h, "Put here").click();
    settle(&mut h);
    let tool = two_corners(&mut h);
    assert_eq!(topology_boxes(&h, cpu), 2, "the CPU the project has, DP and X1");
    assert_eq!(tool, "select", "the tool does not take the CPU again");

    // The Black box tool on the bus page offers what is not on it yet: the terminal, not the CPU.
    dispatch(&mut h, "tool.black_box", json!({}));
    settle(&mut h);
    let (choices, chosen) = put_station_form(&h);
    assert_eq!(chosen, terminal.to_string(), "{choices:?}");
    assert!(!choices.iter().any(|(v, _)| *v == cpu.to_string()), "{choices:?}");
    h.state_mut().shell_mut().shutdown();
}

/// The items of the PLC tab's Place port button.
fn place_port_list(
    h: &egui_kittest::Harness<'static, ew_ui::app::App>,
) -> Vec<ew_viewmodel::actions::ActionItem> {
    use ew_viewmodel::command_bar::{ButtonKind, Tab, command_bar_of};
    command_bar_of(h.state().shell(), Tab::Plc)
        .groups
        .into_iter()
        .flat_map(|g| g.buttons)
        .find_map(|b| match b.kind {
            ButtonKind::Menu(items) if b.id == "bus.place_port" => Some(items),
            _ => None,
        })
        .expect("Place port's list")
}

/// A bus page's own menu and the PLC tab's Place port list have Put a station on this
/// page, listing what the page does not show yet; chosen, the station stands there whole, and the entry is greyed with
/// its reason once nothing is left to put.
#[test]
fn a_bus_page_offers_to_put_a_station_on_it() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let cpu = made_station(&mut h, "cpu", "EW-GEN-CPU-2");
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "900", "type": "bus_topology", "title": "Field bus"}),
    );
    dispatch(&mut h, "page.goto", json!({"folio": "900"}));
    settle(&mut h);
    let entry = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        canvas_menu(h.state().shell(), MenuSpot::Sheet, None)
            .into_iter()
            .find(|i| i.id == "bus.put_station")
            .expect("the page's menu has Put a station on this page")
    };
    let put = entry(&h);
    assert_eq!(put.label, "Put a station on this page…");
    assert!(put.enabled, "{:?}", put.reason);
    let list = place_port_list(&h);
    assert!(
        list.iter()
            .any(|i| i.id == "bus.put_station" && i.params["station"] == cpu.to_string() && i.enabled),
        "{list:?}"
    );

    dispatch(&mut h, "bus.put_station", json!({}));
    settle(&mut h);
    assert_eq!(put_station_form(&h).1, cpu.to_string());
    support::menu_node(&h, "Put here").click();
    settle(&mut h);
    two_corners(&mut h);
    assert_eq!(topology_boxes(&h, cpu), 2, "DP and X1");

    let put = entry(&h);
    assert!(!put.enabled);
    assert_eq!(
        put.reason.as_deref(),
        Some("Every station and bus device of the project is on this page already")
    );
    assert!(
        !place_port_list(&h)
            .iter()
            .any(|i| i.id == "bus.put_station" && i.enabled),
        "nothing left to put"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A CPU's row dragged from the PLC panel onto the bus page must do something. The drop puts
/// the station there at the drop point, a box per interface, and the tool gives way; a drop that cannot be taken says
/// why: the station is there already, or the page takes no stations.
#[test]
fn a_station_row_dropped_on_the_bus_page_puts_it_there() {
    use ew_viewmodel::navigators::NavKind;
    use ew_viewmodel::shell::DragSource;
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    let cpu = made_station(&mut h, "cpu", "EW-GEN-CPU-2");
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "900", "type": "bus_topology", "title": "Field bus"}),
    );
    dispatch(&mut h, "page.goto", json!({"folio": "900"}));
    settle(&mut h);
    settle(&mut h);
    let drop_row = |h: &mut egui_kittest::Harness<'static, ew_ui::app::App>| {
        let name = h
            .state()
            .shell()
            .shown_rows(NavKind::Plc)
            .into_iter()
            .find(|r| r.drag == Some(DragSource::Station(cpu.to_string())))
            .expect("the CPU's row carries it")
            .name;
        let canvas = h
            .get_all_by_role(egui::Role::Canvas)
            .next()
            .expect("the canvas")
            .accesskit_node()
            .raw_bounds()
            .expect("bounds");
        let to = egui::pos2(
            ((canvas.x0 + canvas.x1) / 2.0) as f32,
            ((canvas.y0 + canvas.y1) / 2.0) as f32,
        );
        let from = crate::navigators::item_centre(h, &name);
        crate::navigators::drag(h, from, to);
    };
    drop_row(&mut h);
    assert_eq!(topology_boxes(&h, cpu), 2, "DP and X1 at the drop point");
    assert_eq!(h.state().shell().tool().name(), "select");
    assert!(
        h.state().shell().dialog().is_none(),
        "its rack does not open again unasked: {:?}",
        h.state().shell().dialog()
    );

    drop_row(&mut h);
    let banner = h.state().shell().banner().unwrap_or_default().to_owned();
    assert!(
        banner.starts_with("Every bus interface of") && banner.ends_with("is drawn on page 900 already"),
        "{banner}"
    );
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    settle(&mut h);
    drop_row(&mut h);
    assert_eq!(
        h.state().shell().banner(),
        Some("A station is put on a bus-topology page: open one, or make one with PLC ▸ New bus page")
    );
    h.state_mut().shell_mut().shutdown();
}

/// The chip "Not on this page: 1-A0" is not taken for a code. With the CPU drawn
/// on page 101 and its PLC row selected on a bus page, the chip reads as the jump it is, and Put it here beside it puts
/// the CPU on the bus page.
#[test]
fn the_chip_says_where_it_goes_and_puts_a_station_here() {
    use ew_viewmodel::navigators::NavKind;
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    crate::plc_workspace::draw_black_box(&mut h, "EW-GEN-CPU-2", (10, 10));
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "900", "type": "bus_topology", "title": "Field bus"}),
    );
    dispatch(&mut h, "page.goto", json!({"folio": "900"}));
    settle(&mut h);
    let (key, cpu) = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == "station")
        .map(|r| {
            let id = r.key.rsplit("station:").next().unwrap_or_default().to_owned();
            (r.key, id)
        })
        .expect("the CPU's row");
    for (action, params) in [
        ("navigator.row", json!({"kind": "plc", "key": key})),
        ("navigator.select", json!({"kind": "plc", "keys": [key]})),
    ] {
        dispatch(&mut h, action, params);
    }
    settle(&mut h);
    let words: Vec<String> = h.state().shell().chips().iter().map(|c| c.words()).collect();
    assert!(
        !words.is_empty() && words.iter().all(|w| w.starts_with("Go to page 101, ")),
        "{words:?}"
    );
    h.get_by_label(&words[0]);
    h.get_by_label("Put it here").click();
    settle(&mut h);
    two_corners(&mut h);
    let cpu: Uuid = cpu.parse().expect("the station's ID");
    assert_eq!(topology_boxes(&h, cpu), 2, "DP and X1");
    h.state_mut().shell_mut().shutdown();
}

/// A rack run: a new station's tags read -?A? and -?A?-A1 and nothing said why. Its To do says that a tag gets
/// its number when it is drawn on a page, beside the button that draws it, and the hover of a card with such a tag
/// says the same and where that button is.
#[test]
fn a_new_stations_tag_says_where_its_number_comes_from() {
    let tmp = Tmp::new();
    let mut h = new_project(&tmp);
    draft_with(&mut h, 1);
    h.get_by_label("Create").click();
    settle(&mut h);
    let todo = h.state().shell().station_todo();
    assert!(
        todo.iter().any(|t| t
            .text
            .ends_with("A tag gets its number when it is drawn on a page.")),
        "{todo:?}"
    );
    h.get_by_label("Draw on page 101");
    let card = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Slot 1, -?A?"))
        })
        .expect("the card of slot 1, whose tag is question marks")
        .rect();
    h.hover_at(card.center());
    let start = h.ctx.input(|i| i.time);
    let mut tip = false;
    for i in 0..40 {
        h.input_mut().time = Some(start + f64::from(i) * 0.05);
        h.step();
        tip |= h.get_all_by_role(egui::Role::Label).any(|n| {
            n.accesskit_node().value().is_some_and(|v| {
                v.contains("gets its number when it is drawn on a page: its line under To do draws it")
            })
        });
    }
    assert!(tip, "the hover says where the number comes from");
    h.state_mut().shell_mut().shutdown();
}
