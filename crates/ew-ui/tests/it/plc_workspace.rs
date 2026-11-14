// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC workspace in the window (WP-S2.13, doc 16 §2.6, doc 10 §6), at 1920 × 1080: View ▸ Workspaces ▸ PLC shows
//! the PLC navigator in the left column and the PLC grid in the bottom dock; the grid's filter takes the bus facet and
//! the safety flag (FT-738); a sorted Address header keeps its whole word (FT-776); a project with two CPUs lists a
//! new card under "Cards without a CPU", and a CPU row's menu attaches it (FT-1672).

use crate::support;

use egui_kittest::kittest::{NodeT as _, Queryable};
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
    assert!(
        rows.iter().all(|r| r.text != "Cards without a CPU"),
        "no card waits"
    );
    let module = rows
        .iter()
        .find(|r| r.role == NavRole::Module.word() && r.label == card)
        .expect("the card as a module");
    assert_eq!(module.parent.as_deref(), Some(station.key.as_str()));
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert_eq!(toast, format!("Attached {card} to {}.", station.label));
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    assert!(
        rows.iter().any(|r| r.text == "Cards without a CPU"),
        "one undo step"
    );
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
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert!(
        toast.starts_with("Added the report block Bus overview from folio "),
        "{toast}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A new project with page 101 and the generic CPU drawn on it, the PLC navigator shown.
pub(crate) fn new_project_with_cpu(tmp: &Tmp) -> egui_kittest::Harness<'static, ew_ui::app::App> {
    let mut h = harness(tmp, Scripted::default(), None);
    settle(&mut h);
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("rack.ew").to_string_lossy(), "template": "builtin"}),
    );
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(
        &mut h,
        "tool.black_box",
        json!({"part": "EW-GEN-CPU-1", "template": 0}),
    );
    crate::navigators::cursor_k(&mut h, (10, 10));
    dispatch(&mut h, "canvas.activate", json!({}));
    crate::navigators::cursor_k(&mut h, (16, 18));
    dispatch(&mut h, "canvas.activate", json!({}));
    dispatch(&mut h, "tool.select", json!({}));
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    h
}

/// FR-PLC-028 from nothing: Rack and slots… on a new project's CPU row opens the editor; the digital output card
/// chosen from the module parts goes into slot 1 and then into slot 2 with Add here; a slot typed that is no slot is
/// refused in words under the slots; the navigator lists both cards under the CPU.
#[test]
fn modules_are_added_in_the_rack_and_slot_editor() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = new_project_with_cpu(&tmp);
    let station = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Station.word())
        .expect("the CPU's row");
    h.get_by_label(&station.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Rack and slots…").click();
    settle(&mut h);
    let id = match h.state().shell().dialog() {
        Some(Dialog::Rack { station, .. }) => *station,
        other => panic!("{other:?}"),
    };
    h.get_by_label_contains("Rack and slots of");
    h.get_by_role_and_label(egui::Role::ComboBox, "Module part to add")
        .click();
    settle(&mut h);
    h.get_by_label_contains("EW-GEN-DO-1").click();
    settle(&mut h);
    h.get_by_label("Add here 1").click();
    settle(&mut h);
    h.get_by_label("Add here 2").click();
    settle(&mut h);
    let view = h.state().shell().rack_view(id).expect("the view");
    let cards: Vec<(u32, &str)> = view
        .rows
        .iter()
        .filter(|r| r.module.is_some())
        .map(|r| (r.slot, r.part.split(' ').next().unwrap_or_default()))
        .collect();
    assert_eq!(cards, [(1, "EW-GEN-DO-1"), (2, "EW-GEN-DO-1")]);
    let first = view.rows[0].tag.clone();
    let name = format!("Slot of {first}");
    h.get_by_label(&name).click();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("x".to_owned()));
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    h.get_by_label_contains("is no slot: type a number");
    support::menu_node(&h, "Close").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let under: Vec<&str> = rows
        .iter()
        .filter(|r| r.role == NavRole::Module.word() && r.parent.as_deref() == Some(station.key.as_str()))
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(under.len(), 2, "{under:?}");
    h.state_mut().shell_mut().shutdown();
}

/// FR-PLC-028 on Golden A: the valve terminal's editor lists its seven modules in manifold order, each with its valve
/// positions, beside the stored order codes, and no EW-PLC-011 finding.
#[test]
fn the_valve_terminal_shows_its_positions_beside_its_order_codes() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "201");
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let station = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Station.word())
        .expect("-201U1's row");
    h.get_by_label(&station.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Rack and slots…").click();
    settle(&mut h);
    h.get_by_label("Rack and slots of -201U1");
    let code = format!("Order Code 1: {}", "51E-F13GCQP-D+H2GG");
    assert!(
        h.query_all_by_label_contains(&code).count() >= 1,
        "the stored order code"
    );
    for tag in ["-201U3", "-201U8", "-202U2"] {
        h.get_by_label(&format!("Slot of {tag}"));
    }
    assert!(h.query_all_by_label_contains("Valve positions: 1 ").count() >= 7);
    h.get_by_label("The slots meet the station's rules.");
    h.state_mut().shell_mut().shutdown();
}

/// FR-PLC-014 in the window: a module started by hand at Q10 is re-addressed from its row's Re-address…: the Preview
/// tab lists each channel's old and new address, and Apply moves them back to the automatic start in one step.
#[test]
fn a_module_is_re_addressed_with_its_old_to_new_preview() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let module = h
        .state()
        .shell()
        .nav(NavKind::Plc)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Module && n.label == "-201U5")
                .map(|n| n.records[0])
        })
        .expect("-201U5");
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "plc.set_start", "params": {"modules": [module.to_string()], "dir": "output", "spec": "Q10"}}),
    );
    settle(&mut h);
    // The navigator's filter (the text field at the left column's top) narrows the tree to the module.
    h.get_all_by_role(egui::accesskit::Role::TextInput)
        .filter(|n| n.rect().left() < 600.0 && n.rect().top() > 100.0)
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the navigator's filter")
        .click();
    h.run_ok();
    h.event(egui::Event::Text("201U5".to_owned()));
    settle(&mut h);
    let row = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Module.word() && r.label == "-201U5")
        .expect("-201U5's row");
    h.get_by_label(&row.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Re-address…").click();
    settle(&mut h);
    let notices = h.state().shell().preview_pane().notices().to_vec();
    assert!(
        notices
            .iter()
            .any(|n| n.starts_with("channel 0 of -201U5: Q10.0 → ")),
        "{notices:?}"
    );
    h.get_by_label_contains("channel 0 of -201U5: Q10.0");
    support::off_chrome(&h, "Apply")
        .expect("the card's Apply")
        .click();
    settle(&mut h);
    let t = h
        .state()
        .shell()
        .nav(NavKind::Plc)
        .and_then(|n| n.table())
        .cloned()
        .unwrap();
    let address = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Channel && n.cells.get(5).is_some_and(|m| m == "-201U5"))
        .map(|n| n.cells[0].clone())
        .unwrap_or_default();
    assert!(!address.starts_with("Q10"), "{address}");
    h.state_mut().shell_mut().shutdown();
}

/// The PLC exchange from the Output menu (doc 10 §4, §6): Export… asks the format in its form and the file, writes
/// Golden A's configuration as AML and says what it holds; Import… of that file previews it by tab, with nothing in
/// conflict or parked, and Apply records the import (the merge's cases are WP-S3.16's tests).
#[test]
fn the_plc_exchange_exports_and_previews_an_import() {
    use ew_ui::app::Pick;
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let file = tmp.0.join("plc.aml");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(file.clone()));
    platform.answers.borrow_mut().push_back(Pick::Path(file.clone()));
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, platform, Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    support::open_menu(&mut h, &["Output"]);
    settle(&mut h);
    h.get_by_label("Export to PLC software…").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "plc.export"));
    h.get_by_label("Create").click();
    for _ in 0..10 {
        settle(&mut h);
    }
    assert!(file.exists(), "the AML file is written");
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert!(
        toast.starts_with("Exported plc.aml: 1 station(s), 7 module(s), 56 channel(s)"),
        "{toast}"
    );
    // The project's own file read back: nothing differs from the exchange just recorded.
    support::open_menu(&mut h, &["Output"]);
    settle(&mut h);
    h.get_by_label("Import from PLC software…").click();
    for _ in 0..5 {
        settle(&mut h);
    }
    let card = match h.state().shell().dialog() {
        Some(Dialog::PlcImport(card)) => card.clone(),
        other => panic!("{other:?}"),
    };
    assert!(card.conflicts.is_empty() && card.parking.is_empty(), "{card:?}");
    h.get_by_label_contains("Nothing changes until you apply.");
    h.get_by_label_contains("Removed (").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::PlcImport(c)) if c.tab.word() == "removed"));
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none());
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert_eq!(toast, "Imported plc.aml.");
    h.state_mut().shell_mut().shutdown();
}

/// Doc 10 §4.4: Apply does what the preview of a complete file showed. The station of Golden A's exchange is renamed in
/// the file (another ID, another designation), so the file holds a station the project lacks and lacks the one it
/// has: imported as complete, the Removed tab lists the project's station, and Apply takes it out of the last exchange,
/// so the same file previewed again removes nothing.
#[test]
fn a_complete_imports_apply_removes_what_its_preview_listed() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let station = h
        .state()
        .shell()
        .nav(NavKind::Plc)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Station)
                .map(|n| n.records[0])
        })
        .expect("-201U1");
    let exported = tmp.0.join("plc.aml");
    dispatch(
        &mut h,
        "plc.export",
        json!({"format": "aml-1.4.0", "path": exported.to_string_lossy()}),
    );
    let text = std::fs::read_to_string(&exported).expect("the export");
    assert!(text.contains(&station.to_string()), "the station's ID in the file");
    let foreign = text
        .replace(&station.to_string(), "tia-station")
        .replace("<Value>-201U1</Value>", "<Value>-209U1</Value>");
    let file = tmp.0.join("renamed.aml");
    std::fs::write(&file, foreign).expect("the file");
    let import = json!({"path": file.to_string_lossy(), "complete": true});
    let removed = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| match h.state().shell().dialog() {
        Some(Dialog::PlcImport(card)) => card.removed.clone(),
        other => panic!("{other:?}"),
    };
    dispatch(&mut h, "plc.import", import.clone());
    settle(&mut h);
    let listed = removed(&h);
    assert!(
        listed.iter().any(|r| r == "station -201U1"),
        "{listed:?}"
    );
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    dispatch(&mut h, "plc.import", import);
    settle(&mut h);
    assert_eq!(removed(&h), Vec::<String>::new(), "the station left the last exchange");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1756, doc 10 §4.5: a conflict shows the side Apply takes without a choice, and offers the file's value only where
/// it can be taken. The file gives a channel's plain comment a comment in a language, which never replaces it: the
/// conflict's row has "Keep the project's" chosen and, in place of "Take the file's", the reason; Apply keeps it.
#[test]
fn a_conflict_shows_its_default_and_never_offers_what_cannot_be_taken() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let record = |h: &egui_kittest::Harness<'_, ew_ui::app::App>, role: NavRole, cell: &str| {
        h.state()
            .shell()
            .nav(NavKind::Plc)
            .and_then(|n| n.table())
            .and_then(|t| {
                t.nodes
                    .iter()
                    .find(|n| n.role == role && (cell.is_empty() || n.cells.iter().any(|c| c == cell)))
                    .map(|n| n.records[0].to_string())
            })
            .expect("the row")
    };
    // Symbols from the process tags, so that the file has tags with their comments.
    let config = record(&h, NavRole::ConfigProject, "");
    let channel = record(&h, NavRole::Channel, "TA05-V-15");
    for (kind, target, path, value) in [
        ("plc_config_project", config, "symbols", json!({"template": "{target.process_tag}"})),
        ("channel", channel, "comment", json!("Feed")),
    ] {
        dispatch(
            &mut h,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": kind, "targets": [target], "path": path, "value": value}}),
        );
    }
    let exported = tmp.0.join("plc.aml");
    dispatch(
        &mut h,
        "plc.export",
        json!({"format": "aml-1.4.0", "path": exported.to_string_lossy()}),
    );
    let text = std::fs::read_to_string(&exported).expect("the export");
    assert!(text.contains("<Value>Feed</Value>"), "the comment in the file");
    let file = tmp.0.join("german.aml");
    let german = text.replace(
        "<Value>Feed</Value>",
        "<Attribute Name=\"aml-lang=de-DE\"><Value>Zufuhr</Value></Attribute>",
    );
    std::fs::write(&file, german).expect("the file");
    dispatch(&mut h, "plc.import", json!({"path": file.to_string_lossy()}));
    settle(&mut h);
    h.get_by_label_contains("Conflicts (1)").click();
    settle(&mut h);
    let keep = h.get_by_label("Keep the project's");
    assert_eq!(
        keep.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True),
        "the default side is chosen"
    );
    assert!(h.query_by_label("Take the file's").is_none(), "no side that cannot be taken");
    h.get_by_label_contains("Comment (de-DE) of TA05_V_15 cannot be taken from the file");
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        !matches!(h.state().shell().dialog(), Some(Dialog::PlcImport(_))),
        "applied"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The value of field `name` of the open form.
fn form_value(h: &egui_kittest::Harness<'_, ew_ui::app::App>, name: &str) -> String {
    match h.state().shell().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f
            .fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.clone())
            .unwrap_or_default(),
        other => panic!("{other:?}"),
    }
}

/// Types `text` into the form field labelled `label`.
fn type_into(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, label: &str, text: &str) {
    h.get_by_label(label).click();
    h.run_ok();
    h.event(egui::Event::Text(text.to_owned()));
    settle(h);
}

/// FT-1752: the project's PLC and fluid settings from the Project menu. Process-tag scheme… and Fluid settings… are
/// forms that start with the project's values and set them in one step each; Channel addresses at terminals is a switch.
#[test]
fn the_process_tag_and_fluid_settings_are_set_from_the_project_menu() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    support::menu_node(&h, "Process-tag scheme…").click();
    settle(&mut h);
    type_into(
        &mut h,
        "Scheme (regular expression with named groups)",
        "(?P<area>[A-Z]{2}\\d\\d)-(?P<kind>[A-Z])-(?P<number>\\d+)",
    );
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    dispatch(&mut h, "plc.process_tag_scheme", json!({}));
    assert!(
        form_value(&h, "scheme").starts_with("(?P<area>"),
        "the form shows the scheme set"
    );
    dispatch(&mut h, "ui.dismiss", json!({}));
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    support::menu_node(&h, "Fluid settings…").click();
    settle(&mut h);
    type_into(&mut h, "Default tube: outer diameter (mm)", "6");
    type_into(&mut h, "Default tube: inner diameter (mm)", "4");
    h.get_by_label("Tag fluid lines by page and counter").click();
    settle(&mut h);
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    dispatch(&mut h, "fluid.settings", json!({}));
    assert_eq!(form_value(&h, "od"), "6");
    assert_eq!(form_value(&h, "id"), "4");
    assert_eq!(form_value(&h, "line_tags"), "true");
    dispatch(&mut h, "ui.dismiss", json!({}));
    assert!(!h.state().shell().plc_addresses_at_terminals_on());
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    support::menu_node(&h, "Channel addresses at terminals").click();
    settle(&mut h);
    assert!(h.state().shell().plc_addresses_at_terminals_on());
    h.state_mut().shell_mut().shutdown();
}

/// FT-1752: what the PLC navigator's rows reach. A configuration project's row shows its symbol policy in
/// Properties, a module's its safety data and a channel's its mode and safety data (each a `fields.set` row); after an
/// applied PLC import the configuration project's row unlocks a protected group (`plc.unlock`) and imports an
/// instrument list, previewed.
#[test]
fn a_configuration_projects_row_unlocks_and_imports_an_instrument_list() {
    use ew_ui::app::Pick;
    let tmp = Tmp::new();
    let aml = tmp.0.join("plc.aml");
    let csv = tmp.0.join("instruments.csv");
    let mut list = String::from("Tag;Description;Signal type;Range;Field location;Preferred station\n");
    for n in 40..44 {
        list.push_str(&format!("TA05-V-{n};Valve {n};DO;;Hall 1;-201U1\n"));
    }
    std::fs::write(&csv, list).expect("the list");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(csv.clone()));
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, platform, Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let config = rows
        .iter()
        .find(|r| r.role == NavRole::ConfigProject.word())
        .expect("the configuration project")
        .clone();
    // Symbol policy…: the template and the tag table set in one step.
    h.get_by_label(&config.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Symbol policy…").click();
    settle(&mut h);
    assert_eq!(form_value(&h, "strict_iec"), "true");
    type_into(&mut h, "Template of the automatic symbol", "{target.dt}_{pin}");
    type_into(&mut h, "PLC tag table", "Valves");
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    h.get_by_label(&config.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Symbol policy…").click();
    settle(&mut h);
    assert_eq!(form_value(&h, "tag_table"), "Valves");
    dispatch(&mut h, "ui.dismiss", json!({}));
    // Safety data… of a module and Mode and safety… of a channel.
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let module = rows
        .iter()
        .find(|r| r.role == NavRole::Module.word())
        .expect("a module")
        .clone();
    h.get_by_label(&module.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Safety data…").click();
    settle(&mut h);
    type_into(&mut h, "F-destination address", "x");
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.query_all_by_label_contains("is no address or time").count() >= 1,
        "a refusal in words"
    );
    h.get_by_label("F-destination address").click();
    h.key_press(egui::Key::Backspace);
    h.event(egui::Event::Text("1200".to_owned()));
    settle(&mut h);
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    let channel = rows
        .iter()
        .find(|r| r.role == NavRole::Channel.word())
        .expect("a channel")
        .clone();
    h.get_by_label(&channel.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Mode and safety…").click();
    settle(&mut h);
    type_into(&mut h, "Safety requirement (such as SIL 2 or PL d)", "SIL2");
    h.get_by_role_and_label(egui::Role::ComboBox, "Safety evaluation")
        .click();
    settle(&mut h);
    h.get_by_label("1oo1").click();
    settle(&mut h);
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    h.get_by_label(&channel.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Mode and safety…").click();
    settle(&mut h);
    assert_eq!(form_value(&h, "evaluation"), "1oo1");
    assert_eq!(form_value(&h, "requirement"), "SIL2");
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(
        &mut h,
        "plc.export",
        json!({"format": "aml-1.4.0", "path": aml.to_string_lossy()}),
    );
    dispatch(&mut h, "plc.import", json!({"path": aml.to_string_lossy()}));
    dispatch(&mut h, "plc.import_apply", json!({}));
    settle(&mut h);
    h.get_by_label(&config.name).click_secondary();
    settle(&mut h);
    let unlock: Vec<String> = h
        .query_all_by_label_contains("Unlock ")
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert!(!unlock.is_empty(), "a protected group to unlock");
    support::menu_node(&h, &unlock[0]).click();
    settle(&mut h);
    h.get_by_label(&config.name).click_secondary();
    settle(&mut h);
    assert!(
        h.query_by_label(&unlock[0]).is_none(),
        "{} is unlocked",
        unlock[0]
    );
    support::menu_node(&h, "Import instrument list…").click();
    for _ in 0..4 {
        settle(&mut h);
    }
    let notices = h.state().shell().preview_pane().notices().to_vec();
    assert!(notices.iter().any(|n| n.contains("reserved")), "{notices:?}");
    support::off_chrome(&h, "Apply")
        .expect("the card's Apply")
        .click();
    settle(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// FT-1752: in the rack and slot editor a module dragged by its tag onto a free slot's row moves there, as typing the
/// slot does (`fields.set plc_module.slot`).
#[test]
fn a_module_is_dragged_to_a_free_slot_in_the_rack_editor() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = new_project_with_cpu(&tmp);
    let station = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Station.word())
        .expect("the CPU's row");
    h.get_by_label(&station.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Rack and slots…").click();
    settle(&mut h);
    let id = match h.state().shell().dialog() {
        Some(Dialog::Rack { station, .. }) => *station,
        other => panic!("{other:?}"),
    };
    h.get_by_role_and_label(egui::Role::ComboBox, "Module part to add")
        .click();
    settle(&mut h);
    h.get_by_label_contains("EW-GEN-DO-1").click();
    settle(&mut h);
    h.get_by_label("Add here 1").click();
    settle(&mut h);
    let tag = h.state().shell().rack_view(id).expect("the rack").rows[0]
        .tag
        .clone();
    let module = (id, tag, 1u32);
    let from = h.get_by_label(&module.1).rect().center();
    let to = h
        .get_all_by_label("free")
        .last()
        .expect("a free slot")
        .rect()
        .center();
    h.input_mut().events.push(egui::Event::PointerMoved(from));
    h.run_ok();
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_ok();
    for i in 1..=8 {
        let p = from + (to - from) * (i as f32 / 8.0);
        h.input_mut().events.push(egui::Event::PointerMoved(p));
        h.run_ok();
    }
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut h);
    let view = h.state().shell().rack_view(module.0).expect("the rack");
    let row = view
        .rows
        .iter()
        .find(|r| r.tag == module.1)
        .expect("the module's row");
    assert_ne!(row.slot, module.2, "moved from slot {}", module.2);
    h.state_mut().shell_mut().shutdown();
}

/// FT-1734: the shipped terminal symbol has a `PlcAddress` slot. In a new project the digital output card's pin Q0.0
/// is wired through terminal -X1:1 to a coil; with Project ▸ Channel addresses at terminals on, the terminal prints Q0.0,
/// and without it nothing.
#[test]
fn a_terminal_on_a_traced_path_prints_the_channel_address() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("traced.ew").to_string_lossy(), "template": "builtin"}),
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
        ("EW-GEN-DO-1", (24, 10), (30, 18)),
    ] {
        dispatch(&mut h, "tool.black_box", json!({"part": part, "template": 0}));
        crate::navigators::cursor_k(&mut h, a);
        dispatch(&mut h, "canvas.activate", json!({}));
        crate::navigators::cursor_k(&mut h, b);
        dispatch(&mut h, "canvas.activate", json!({}));
        dispatch(&mut h, "tool.select", json!({}));
    }
    dispatch(&mut h, "strip.new", json!({"tag": "-X1"}));
    for (pick, at) in [
        (
            json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1", "designation": "1"}),
            (40, 26),
        ),
        (json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}), (40, 36)),
    ] {
        dispatch(&mut h, "tool.place", json!({}));
        dispatch(&mut h, "place.pick", pick);
        crate::navigators::cursor_k(&mut h, at);
        dispatch(&mut h, "canvas.activate", json!({"alt": true}));
        dispatch(&mut h, "ui.dismiss", json!({}));
        dispatch(&mut h, "ui.dismiss", json!({}));
    }
    settle(&mut h);
    let (page, card_pin, term, coil) = {
        let c = h.state().shell().canvas().unwrap();
        let t = std::sync::Arc::clone(&c.page().tables);
        let card = t
            .reps
            .iter()
            .find(|r| {
                r.black_box.is_some()
                    && t.cps
                        .iter()
                        .any(|p| p.rep == r.id && p.plc_address.as_deref() == Some("Q0.0"))
            })
            .expect("the card");
        let pin = t
            .cps
            .iter()
            .find(|p| p.rep == card.id && p.plc_address.as_deref() == Some("Q0.0"))
            .expect("pin Q0.0")
            .clone();
        let of = |key: &str| {
            let r = t.reps.iter().find(|r| r.symbol_key.starts_with(key)).expect(key);
            let mut cps: Vec<_> = t.cps.iter().filter(|p| p.rep == r.id).cloned().collect();
            cps.sort_by_key(|p| p.idx);
            cps
        };
        (t.page, pin, of("ew.iec.terminal"), of("ew.iec.coil"))
    };
    let cp = |p: &ew_ui::ew_viewmodel::ew_api::output::TableCp| json!({"cp": {"function": p.function.to_string(), "idx": p.idx}});
    for (a, b) in [(&card_pin, &term[1]), (&term[0], &coil[0])] {
        dispatch(
            &mut h,
            "command.run",
            json!({"cmd": "connection.draw", "params": {"page": page.to_string(), "a": cp(a), "b": cp(b)}}),
        );
    }
    settle(&mut h);
    // The texts "Q0.0" drawn within 10 mm of the terminal's insertion point.
    let at_terminal = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        use ew_ui::ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
        fn runs(items: &[Item], out: &mut Vec<(String, (i64, i64))>) {
            for item in items {
                match &item.kind {
                    ItemKind::Glyphs(run) => out.push((run.text.clone(), (run.origin.x.0, run.origin.y.0))),
                    ItemKind::Group(g) => runs(&g.items, out),
                    _ => {}
                }
            }
        }
        let c = h.state().shell().canvas().unwrap();
        let page = c.page();
        let rep = page
            .tables
            .reps
            .iter()
            .find(|r| r.symbol_key.starts_with("ew.iec.terminal"))
            .expect("the terminal");
        let at = page.snapper.page_to_sheet(rep.pos);
        let mut all = Vec::new();
        runs(&page.list.items, &mut all);
        all.into_iter()
            .filter(|(t, o)| {
                t == "Q0.0" && (o.0 - at.0).abs() < 10_000_000 && (o.1 - at.1).abs() < 10_000_000
            })
            .count()
    };
    assert_eq!(at_terminal(&h), 0, "off by default");
    dispatch(&mut h, "project.plc_addresses_at_terminals", json!({"on": true}));
    settle(&mut h);
    assert_eq!(at_terminal(&h), 1, "the terminal prints the channel's address");
    h.state_mut().shell_mut().shutdown();
}
