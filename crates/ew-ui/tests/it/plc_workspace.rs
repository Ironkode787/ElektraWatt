// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC workspace in the window (WP-S2.13, doc 16 §2.6, doc 10 §6), at 1920 × 1080: View ▸ Workspaces ▸ PLC shows
//! the PLC navigator in the left column and the PLC grid in the bottom dock; the grid's filter takes the bus facet and
//! the safety flag (FT-738); a sorted Address header keeps its whole word (FT-776); a project with two CPUs lists a
//! new card under "Cards without a CPU", and a CPU row's menu attaches it (FT-1672).

use crate::support;

use egui_kittest::kittest::Queryable;
use ew_ui::ew_viewmodel::layout::PaneKind;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, on_golden, settle, wait_for_page};

/// The channel rows the PLC grid shows.
fn grid_rows(h: &egui_kittest::Harness<'_, ew_ui::app::App>) -> usize {
    h.state()
        .shell()
        .shown_rows(NavKind::PlcGrid)
        .iter()
        .filter(|r| r.role == "channel")
        .count()
}

/// Types `text` into the PLC grid's filter field (the lowest text field: the grid sits in the bottom dock), in
/// place of what it held.
fn grid_filter(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, text: &str) {
    h.get_all_by_role(egui::accesskit::Role::TextInput)
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the grid's filter field")
        .click();
    h.run_ok();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text(text.to_owned()));
    settle(h);
}

#[test]
fn the_plc_workspace_shows_the_plc_navigator_and_its_grid() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    h.get_by_role_and_label(egui::Role::Tab, "View").click();
    settle(&mut h);
    support::bar_button(&h, "Workspaces").click();
    settle(&mut h);
    support::menu_node(&h, "PLC").click();
    settle(&mut h);
    {
        let shell = h.state().shell();
        assert_eq!(shell.workspace(), "PLC");
        let layout = shell.layout();
        assert_eq!(
            layout.shown_in_place_of(PaneKind::Pages),
            Some(PaneKind::Navigator(NavKind::Plc)),
            "the PLC navigator in the left column"
        );
        assert!(layout.panes().contains(&PaneKind::Navigator(NavKind::PlcGrid)));
        assert!(
            shell.nav(NavKind::PlcGrid).and_then(|n| n.table_mode()).is_some(),
            "the grid is a table"
        );
        assert!(
            shell.nav(NavKind::Plc).and_then(|n| n.table_mode()).is_none(),
            "the navigator is a tree"
        );
    }
    h.get_by_label("PLC grid table");
    assert_eq!(grid_rows(&h), 56);
    // FT-776: the Address header grows by its sort icon when sorted, so its word is not cut.
    let width = |h: &egui_kittest::Harness<'_, ew_ui::app::App>, label: &str| {
        h.get_by_label_contains(label).rect().width()
    };
    let before = width(&h, "Sort by Address");
    h.get_by_label("Sort by Address").click();
    settle(&mut h);
    assert!(width(&h, "Sort by Address, sorted") > before);
    // FT-738: the grid's filter takes the net of the station's bus port and the safety flag.
    grid_filter(&mut h, "bus:profibus-dp");
    assert_eq!(grid_rows(&h), 56, "every channel of -201U1");
    grid_filter(&mut h, "safety");
    assert_eq!(grid_rows(&h), 0, "Golden A has no safety data");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1672: in a new project with two CPUs a new card joins neither; the PLC navigator lists it under "Cards without a
/// CPU", and Attach cards without a CPU on the first CPU's row makes it that CPU's module, one undo step.
#[test]
fn a_card_without_a_cpu_is_attached_from_the_cpus_row() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("two.ew").to_string_lossy(), "template": "builtin"}),
    );
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    settle(&mut h);
    wait_for_page(&mut h);
    for (part, a, b) in [
        ("EW-GEN-CPU-1", (10, 10), (16, 18)),
        ("EW-GEN-CPU-1", (24, 10), (30, 18)),
        ("EW-GEN-DO-1", (38, 10), (44, 18)),
    ] {
        dispatch(&mut h, "tool.black_box", json!({"part": part, "template": 0}));
        crate::navigators::cursor_k(&mut h, a);
        dispatch(&mut h, "canvas.activate", json!({}));
        crate::navigators::cursor_k(&mut h, b);
        dispatch(&mut h, "canvas.activate", json!({}));
        dispatch(&mut h, "tool.select", json!({}));
    }
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let group = rows
        .iter()
        .find(|r| r.text == "Cards without a CPU")
        .expect("the group of cards without a CPU");
    let card = rows
        .iter()
        .find(|r| r.parent.as_deref() == Some(group.key.as_str()))
        .expect("the card under it")
        .label
        .clone();
    let station = rows
        .iter()
        .find(|r| r.role == NavRole::Station.word())
        .expect("a CPU row")
        .clone();
    h.get_by_label(&station.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Attach cards without a CPU").click();
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    assert!(rows.iter().all(|r| r.text != "Cards without a CPU"), "no card waits");
    let module = rows
        .iter()
        .find(|r| r.role == NavRole::Module.word() && r.label == card)
        .expect("the card as a module");
    assert_eq!(module.parent.as_deref(), Some(station.key.as_str()));
    let toast = h.state().shell().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(toast, format!("Attached {card} to {}.", station.label));
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    assert!(rows.iter().any(|r| r.text == "Cards without a CPU"), "one undo step");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1726: Output ▸ Reports ▸ Report block… offers the report kinds read from the kinds themselves, the PLC and bus
/// lists and the fluid lists among them, never an embedded kind; a Bus overview chosen from the list adds its block.
#[test]
fn the_report_block_form_offers_the_plc_bus_and_fluid_lists() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    support::open_menu(&mut h, &["Output"]);
    settle(&mut h);
    h.get_by_label("Report block…").click();
    settle(&mut h);
    let offered: Vec<String> = match h.state().shell().dialog() {
        Some(Dialog::Form(f)) => f.fields[0].options.iter().map(|(k, _)| k.clone()).collect(),
        other => panic!("{other:?}"),
    };
    let kinds = ew_ui::ew_viewmodel::ew_api::output::page_report_kinds();
    assert_eq!(offered.len(), kinds.len());
    for k in ["safety_io_list", "bus_overview", "tube_list", "medium_overview"] {
        assert!(offered.iter().any(|o| o == k), "{k}: {offered:?}");
    }
    assert!(!offered.iter().any(|o| o == "part_data" || o == "check_report"));
    h.get_all_by_role(egui::Role::ComboBox)
        .next()
        .expect("the Report list")
        .click();
    settle(&mut h);
    h.get_by_label("Bus overview").click();
    settle(&mut h);
    h.get_by_label("Create").click();
    for _ in 0..600 {
        h.run_ok();
        if h.state().shell().dialog().is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    settle(&mut h);
    let toast = h.state().shell().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        toast.starts_with("Added the report block Bus overview from folio "),
        "{toast}"
    );
    h.state_mut().shell_mut().shutdown();
}
