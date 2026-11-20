// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC workspace in the window (WP-S2.13, doc 16 §2.6, doc 10 §6), at 1920 × 1080: View ▸ Workspaces ▸ PLC shows
//! the PLC navigator in the left column and the PLC grid in the bottom dock; the grid's filter takes the bus facet and
//! the safety flag (FT-738); a sorted Address header keeps its whole word (FT-776); a project with two CPUs lists a
//! new card under "Modules without a station", and a CPU row's menu attaches it (FT-1672).

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
    // The workspace's grid makes the canvas shorter: the page fits it again, whole.
    let canvas = h.state().shell().canvas().expect("the canvas");
    assert!(canvas.is_fitted() && !canvas.is_first_view(), "Fit page");
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

/// The persona runs of the third 0.3.0 candidate: a symbol name typed into the PLC grid read "CLAMP_RETR…", with no
/// way to see it whole. A column that cuts a new text grows to show it.
#[test]
fn the_plc_grids_symbol_column_grows_to_a_long_name() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "view.workspace_load", json!({"name": "PLC"}));
    settle(&mut h);
    let width = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        h.get_by_label_contains("Sort by Symbol").rect().width()
    };
    let before = width(&h);
    let row = h
        .state()
        .shell()
        .shown_rows(NavKind::PlcGrid)
        .into_iter()
        .find(|r| r.role == "channel")
        .expect("a channel row")
        .key;
    let set = json!({"kind": "plc_grid", "column": "Symbol", "value": "CLAMP_RETRACTED_AT_THE_FRONT_STOP", "rows": [row]});
    dispatch(&mut h, "table.set", set);
    settle(&mut h);
    settle(&mut h);
    assert!(width(&h) > before + 40.0, "{before} -> {}", width(&h));
    h.state_mut().shell_mut().shutdown();
}

/// The PLC grid's close button in the PLC workspace closes the bottom dock (it highlighted and left the grid on show);
/// View ▸ Bottom dock (Ctrl+J) brings the grid back.
#[test]
fn the_plc_grids_close_button_closes_the_dock() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "view.workspace_load", json!({"name": "PLC"}));
    settle(&mut h);
    let grid = PaneKind::Navigator(NavKind::PlcGrid);
    assert!(h.state().shell().layout().panes().contains(&grid));
    h.get_by_label("Close the bottom dock").click();
    settle(&mut h);
    assert!(
        !h.state().shell().layout().panes().contains(&grid),
        "the grid is closed"
    );
    dispatch(&mut h, "view.bottom_dock", json!({}));
    settle(&mut h);
    assert!(h.state().shell().layout().panes().contains(&grid), "and back");
    h.state_mut().shell_mut().shutdown();
}

/// A new project with two CPUs and an output card that joined neither, the PLC panel on show.
fn two_cpus_and_a_card(tmp: &Tmp) -> egui_kittest::Harness<'static, ew_ui::app::App> {
    let mut h = harness(tmp, Scripted::default(), None);
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
        // The card on a page of its own: beside a station it would join that one (the persona run m3-p4).
        if part == "EW-GEN-DO-1" {
            dispatch(
                &mut h,
                "page.new",
                json!({"folio": "102", "type": "multi_line", "title": "Cards"}),
            );
            settle(&mut h);
            wait_for_page(&mut h);
        }
        dispatch(&mut h, "tool.black_box", json!({"part": part, "template": 0}));
        crate::navigators::cursor_k(&mut h, a);
        dispatch(&mut h, "canvas.activate", json!({}));
        crate::navigators::cursor_k(&mut h, b);
        dispatch(&mut h, "canvas.activate", json!({}));
        dispatch(&mut h, "tool.select", json!({}));
        // Each CPU's view opens when it is placed; the card on a page of neither asks which station takes it.
        // Left with Escape, the card waits.
        let shell = h.state().shell();
        assert!(
            shell.dialog().is_some() || shell.station_tab().is_some(),
            "{part}: the station's view, or the question"
        );
        dispatch(&mut h, "ui.dismiss", json!({}));
    }
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    h
}

/// The pointer's press and release of `button` at `at`.
fn click_at(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, at: egui::Pos2, button: egui::PointerButton) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_ok();
    }
    settle(h);
}

/// The persona round's re-test: a row takes a click across its width, not only on its name (a right-click beside a
/// short tag did nothing), and a waiting card's own row offers Attach to `<station>` for each station that takes
/// cards, which attaches that card alone.
#[test]
fn a_waiting_cards_row_takes_a_click_beside_its_name_and_attaches_to_a_station() {
    let tmp = Tmp::new();
    let mut h = two_cpus_and_a_card(&tmp);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let group = rows
        .iter()
        .find(|r| r.text == "Modules without a station")
        .expect("the group of cards without a station");
    let card = rows
        .iter()
        .find(|r| r.parent.as_deref() == Some(group.key.as_str()))
        .expect("the card under it")
        .clone();
    let second = rows
        .iter()
        .filter(|r| r.role == NavRole::Station.word())
        .nth(1)
        .expect("the second CPU's row")
        .clone();
    let name = h.get_by_label(&card.name).rect();
    let beside = egui::pos2(name.right() + 40.0, name.center().y);
    click_at(&mut h, beside, egui::PointerButton::Primary);
    let marked = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        h.state()
            .shell()
            .shown_rows(NavKind::Plc)
            .into_iter()
            .find(|r| r.key == card.key)
            .is_some_and(|r| r.marked)
    };
    assert!(marked(&h), "a click beside the name selects the row");
    click_at(&mut h, beside, egui::PointerButton::Secondary);
    support::menu_node(&h, &format!("Attach to {}", second.label)).click();
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let module = rows
        .iter()
        .find(|r| r.role == NavRole::Module.word() && r.label == card.label)
        .expect("the card as a module");
    assert_eq!(module.parent.as_deref(), Some(second.key.as_str()));
    h.state_mut().shell_mut().shutdown();
}

/// FT-1672: in a new project with two CPUs a new card joins neither; the PLC navigator lists it under "Cards without a
/// CPU", and Attach cards without a station on the first CPU's row makes it that CPU's module, one undo step.
#[test]
fn a_card_without_a_cpu_is_attached_from_the_cpus_row() {
    let tmp = Tmp::new();
    let mut h = two_cpus_and_a_card(&tmp);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let group = rows
        .iter()
        .find(|r| r.text == "Modules without a station")
        .expect("the group of cards without a station");
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
    support::menu_node(&h, "Attach modules without a station").click();
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    assert!(
        rows.iter().all(|r| r.text != "Modules without a station"),
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
        rows.iter().any(|r| r.text == "Modules without a station"),
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
    crate::support::combos(&h)
        .into_iter()
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
    draw_black_box(&mut h, "EW-GEN-CPU-1", (10, 10));
    // A station placed directly opens its view in a tab beside the page, so that it is filled there.
    let tabs = &h
        .state()
        .shell()
        .area()
        .expect("the canvas area")
        .focused_group()
        .tabs;
    assert!(
        tabs.iter()
            .any(|t| t.station.as_ref().is_some_and(|s| s.station.is_some())),
        "the CPU's view is a tab beside the page"
    );
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    h
}

/// The station whose view is on show.
pub(crate) fn station_of(
    h: &egui_kittest::Harness<'static, ew_ui::app::App>,
) -> ew_ui::ew_viewmodel::ew_api::output::Uuid {
    h.state()
        .shell()
        .station_tab()
        .and_then(|t| t.station)
        .expect("a station's view")
}

/// The centre of the widget named `label` in the station's view.
pub(crate) fn at(h: &egui_kittest::Harness<'static, ew_ui::app::App>, label: &str) -> egui::Pos2 {
    h.get_all_by_label(label)
        .next()
        .unwrap_or_else(|| panic!("no widget named {label:?}"))
        .rect()
        .center()
}

/// A double click on the catalogue's part whose row starts with `number`.
pub(crate) fn take(h: &mut egui_kittest::Harness<'static, ew_ui::app::App>, number: &str) {
    let at = h
        .query_all_by_role(egui::Role::ListItem)
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(number)))
        .unwrap_or_else(|| panic!("the catalogue lists {number}"))
        .rect()
        .center();
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
    h.run();
    settle(h);
}

/// A drag with the primary button from `from` to `to`, in steps.
pub(crate) fn drag(
    h: &mut egui_kittest::Harness<'static, ew_ui::app::App>,
    from: egui::Pos2,
    to: egui::Pos2,
) {
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
    settle(h);
}

/// Draws the starter part `number` as a black box from grid cell `at` (six by eight cells) on the page shown, then
/// takes the Select tool.
pub(crate) fn draw_black_box(
    h: &mut egui_kittest::Harness<'static, ew_ui::app::App>,
    number: &str,
    at: (i64, i64),
) {
    wait_for_page(h);
    dispatch(h, "tool.black_box", json!({"part": number, "template": 0}));
    crate::navigators::cursor_k(h, at);
    dispatch(h, "canvas.activate", json!({}));
    crate::navigators::cursor_k(h, (at.0 + 6, at.1 + 8));
    dispatch(h, "canvas.activate", json!({}));
    dispatch(h, "tool.select", json!({}));
    // A station placed on a page opens its view in a tab; Escape goes back to the page, the tab stays beside it.
    if h.state().shell().station_tab().is_some() {
        dispatch(h, "ui.dismiss", json!({}));
    }
    settle(h);
}

/// FR-PLC-028 from nothing (R1, build my PLC): Rack… on a new project's CPU row opens its view in a tab; a double click
/// on the digital output card in the catalogue puts it into slot 1, a second into slot 2, and each stands in the
/// drawn rack and in the table with its part number and description, what it gives and its addresses. The module
/// under the cursor is the window's selection. The navigator lists both modules under the CPU.
#[test]
fn modules_are_added_in_the_station_view() {
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
    support::menu_node(&h, "Rack…").click();
    settle(&mut h);
    let id = station_of(&h);
    assert!(h.state().shell().dialog().is_none(), "the view is no dialog");
    take(&mut h, "EW-GEN-DO-1");
    take(&mut h, "EW-GEN-DO-1");
    let view = h.state().shell().rack_view(id).expect("the view");
    let modules: Vec<(u32, &str, &str, &str)> = view
        .rows
        .iter()
        .filter(|r| r.module.is_some())
        .map(|r| (r.slot, r.part.as_str(), r.gives.as_str(), r.addresses.as_str()))
        .collect();
    let part = "EW-GEN-DO-1 · PLC digital output card, 8 channels";
    assert_eq!(
        modules,
        [(1, part, "8 DO", "Q0.0–Q0.7"), (2, part, "8 DO", "Q1.0–Q1.7")]
    );
    // What the rows say stands on the screen: the part in words, what it gives, its addresses.
    assert_eq!(h.query_all_by_label(part).count(), 2, "the part of each module");
    for text in ["8 DO", "Q0.0–Q0.7", "Q1.0–Q1.7"] {
        assert!(h.query_all_by_label(text).next().is_some(), "{text}");
    }
    // The module added last is under the cursor, and it is the window's selection.
    let second = view.rows[1].module.expect("the module in slot 2");
    assert_eq!(h.state().shell().station_device(), Some(second));
    assert_eq!(
        h.state().shell().selection().items,
        vec![ew_dl::EntityRef(second)]
    );
    dispatch(&mut h, "tab.close", json!({}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let under: Vec<&str> = rows
        .iter()
        .filter(|r| r.role == NavRole::Module.word() && r.parent.as_deref() == Some(station.key.as_str()))
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(under.len(), 2, "{under:?}");
    h.state_mut().shell_mut().shutdown();
}

/// A long station at 1920 × 1080: with twenty-two modules the catalogue and the To do list stay inside the window (the
/// rack scrolls sideways, the table down); the view reads its rack once per revision; a slot a module holds is refused
/// in words at that slot and adds nothing.
#[test]
fn a_long_station_stays_usable_and_a_taken_slot_is_refused() {
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
    support::menu_node(&h, "Rack…").click();
    settle(&mut h);
    let id = station_of(&h);
    for _ in 1..=22 {
        dispatch(&mut h, "plc.rack_add", json!({ "part": "EW-GEN-DI-1" }));
    }
    settle(&mut h);
    let modules = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        let view = h.state().shell().rack_view(id).expect("the rack");
        view.rows.iter().filter(|r| r.module.is_some()).count()
    };
    assert_eq!(modules(&h), 22);
    // FT-1763: the tab's title and the view read one rack view per revision, not one each a frame.
    let made = h.state().shell().session_calls().rack_view;
    h.run_steps(3);
    assert_eq!(
        h.state().shell().session_calls().rack_view,
        made,
        "no new rack view without a change"
    );
    let window = h.ctx.content_rect();
    for label in ["Catalogue", "New part…", "Drawn on"] {
        let rect = h.get_all_by_label(label).next().expect(label).rect();
        assert!(
            window.contains_rect(rect),
            "{label} {rect:?} is out of the window {window:?}"
        );
    }
    let refused = h
        .state_mut()
        .shell_mut()
        .dispatch("plc.rack_add", &json!({ "slot": 3, "part": "EW-GEN-DI-1" }));
    assert!(
        matches!(refused, ew_ui::ew_viewmodel::shell::Outcome::Failed { .. }),
        "{refused:?}"
    );
    settle(&mut h);
    assert_eq!(modules(&h), 22, "a taken slot adds nothing");
    h.get_by_label_contains("Slot 3 holds");
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
    support::menu_node(&h, "Rack…").click();
    settle(&mut h);
    h.get_by_label("-201U1 · rack");
    let code = format!("Order Code 1: {}", "51E-F13GCQP-D+H2GG");
    assert!(
        h.query_all_by_label_contains(&code).count() >= 1,
        "the stored order code"
    );
    // Each module stands in the rack as a card and as a row, by its slot and its tag.
    for tag in ["-201U3", "-201U8", "-202U2"] {
        let places = h
            .query_all_by_label_contains(&format!(", {tag}"))
            .filter(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with("Slot ")))
            .count();
        assert_eq!(places, 2, "{tag}: a card and a row");
    }
    // Each sub-base lists its valve positions as rows (rc.2).
    assert!(h.query_all_by_label_contains("Position 1").count() >= 7);
    let shown = h.state().shell().station_shown().expect("the view");
    assert!(
        shown.todo.iter().all(|t| !t.error),
        "no finding of the slot rule: {:?}",
        shown.todo
    );
    h.state_mut().shell_mut().shutdown();
}

/// The persona round: Freeze addresses was found only through the guide, and after Apply nothing said what it did.
/// In the PLC workspace of a new project, the PLC panel's Addresses menu holds Freeze addresses…; Apply toasts the
/// count, and the frozen address carries the manual mark in the panel and in the grid, while a spare channel's does not.
#[test]
fn freeze_addresses_is_in_the_plc_panels_addresses_menu_and_marks_what_it_froze() {
    let tmp = Tmp::new();
    let mut h = new_project_with_cpu(&tmp);
    dispatch(
        &mut h,
        "tool.black_box",
        json!({"part": "EW-GEN-DO-1", "template": 0}),
    );
    crate::navigators::cursor_k(&mut h, (24, 10));
    dispatch(&mut h, "canvas.activate", json!({}));
    crate::navigators::cursor_k(&mut h, (30, 18));
    dispatch(&mut h, "canvas.activate", json!({}));
    dispatch(&mut h, "tool.select", json!({}));
    dispatch(&mut h, "view.workspace_load", json!({"name": "PLC"}));
    settle(&mut h);
    let channels: Vec<String> = h
        .state()
        .shell()
        .nav(NavKind::Plc)
        .and_then(|n| n.table())
        .map(|t| {
            t.nodes
                .iter()
                .filter(|n| n.role == NavRole::Channel)
                .map(|n| n.records[0].to_string())
                .collect()
        })
        .unwrap_or_default();
    assert!(channels.len() > 1, "{channels:?}");
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "channel", "targets": [channels[0]], "path": "state", "value": "reserved"}}),
    );
    settle(&mut h);
    h.get_all_by_label("Addresses")
        .next()
        .expect("the PLC panel's Addresses menu")
        .click();
    settle(&mut h);
    support::menu_node(&h, "Freeze addresses…").click();
    settle(&mut h);
    support::off_chrome(&h, "Apply")
        .expect("the card's Apply")
        .click();
    settle(&mut h);
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert_eq!(
        toast,
        "Froze 1 channel address of PLC1; spare channels keep their automatic addresses."
    );
    let manual = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| -> Vec<String> {
        h.state()
            .shell()
            .shown_rows(NavKind::Plc)
            .into_iter()
            .filter(|r| r.badge_codes.iter().any(|b| b == "manual"))
            .map(|r| r.key)
            .collect()
    };
    let frozen = manual(&h);
    assert!(
        frozen.len() == 1 && frozen[0].ends_with(&format!("channel:{}", channels[0])),
        "the frozen one alone: {frozen:?}"
    );
    assert_eq!(
        h.get_all_by_role(egui::accesskit::Role::Cell)
            .filter(|n| n
                .accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Address ") && l.contains(", manual")))
            .count(),
        1,
        "one address cell of the grid says manual"
    );
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

/// The PLC exchange from the PLC menu (doc 10 §4, §6): Export… asks the format in its form and the file, writes
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
    support::open_menu(&mut h, &["PLC"]);
    settle(&mut h);
    h.get_by_label("Export to PLC software…").click();
    settle(&mut h);
    assert!(matches!(h.state().shell().dialog(), Some(Dialog::Form(f)) if f.action == "plc.export"));
    h.get_by_label("Export").click();
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
        toast.starts_with("Exported plc.aml: 1 station, 7 modules, 56 channels"),
        "{toast}"
    );
    // The project's own file read back: nothing differs from the exchange just recorded.
    support::open_menu(&mut h, &["PLC"]);
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
    h.get_by_label_contains("Conflicts (").click();
    settle(&mut h);
    h.get_by_label("Nothing in this tab.");
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

/// The PLC export's form starts with today's date, which the window reads, and the file it asks for is named for the
/// chosen format: the TIA tag table asks for an `.xlsx` file.
#[test]
fn the_plc_export_asks_for_a_file_of_its_format_dated_today() {
    use ew_ui::app::Pick;
    use ew_ui::ew_viewmodel::shell::Prompt;
    let tmp = Tmp::new();
    let file = tmp.0.join("tags.xlsx");
    let platform = Scripted::default();
    platform.answers.borrow_mut().push_back(Pick::Path(file.clone()));
    let asked = platform.asked.clone();
    let mut h = harness(&tmp, platform, Some(tmp.golden("g.ew")));
    settle(&mut h);
    wait_for_page(&mut h);
    support::open_menu(&mut h, &["PLC"]);
    settle(&mut h);
    h.get_by_label("Export to PLC software…").click();
    settle(&mut h);
    let today = ew_ui::clock::today();
    assert_eq!(form_value(&h, "date"), today, "the form is dated today");
    crate::support::combos(&h)
        .into_iter()
        .next()
        .expect("the Format list")
        .click();
    settle(&mut h);
    h.get_by_label("TIA Portal PLC tags (XLSX)").click();
    settle(&mut h);
    h.get_by_label("Export").click();
    for _ in 0..10 {
        settle(&mut h);
    }
    assert_eq!(asked.borrow().as_slice(), [Prompt::SavePlcExchange("xlsx")]);
    assert!(file.exists(), "the tag table is written");
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
    assert!(
        text.contains(&station.to_string()),
        "the station's ID in the file"
    );
    // A script's export onto an existing file needs `replace`, as the PDF export does.
    let again = json!({"format": "aml-1.4.0", "path": exported.to_string_lossy()});
    assert!(matches!(
        h.state_mut().shell_mut().dispatch("plc.export", &again),
        ew_ui::ew_viewmodel::shell::Outcome::Refused { .. }
    ));
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
    assert!(listed.iter().any(|r| r == "station -201U1"), "{listed:?}");
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    dispatch(&mut h, "plc.import", import);
    settle(&mut h);
    assert_eq!(
        removed(&h),
        Vec::<String>::new(),
        "the station left the last exchange"
    );
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
        (
            "plc_config_project",
            config,
            "symbols",
            json!({"template": "{target.process_tag}"}),
        ),
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
    assert!(
        h.query_by_label("Take the file's").is_none(),
        "no side that cannot be taken"
    );
    h.get_by_label_contains("Comment (de-DE) of TA05_V_15 cannot be taken from the file");
    support::off_chrome(&h, "Apply").expect("Apply").click();
    settle(&mut h);
    assert!(
        !matches!(h.state().shell().dialog(), Some(Dialog::PlcImport(_))),
        "applied"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The PLC import's preview tries Apply: a file that gives -201U4 the start of -201U3 shows Apply's refusal (EW-PLC-001)
/// above the tabs and on the start's row, before Apply is pressed; "The file is complete" makes the preview again with
/// the file complete.
#[test]
fn the_import_preview_says_what_apply_refuses_and_takes_the_file_complete() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let exported = tmp.0.join("plc.aml");
    dispatch(
        &mut h,
        "plc.export",
        json!({"format": "aml-1.4.0", "path": exported.to_string_lossy()}),
    );
    let text = std::fs::read_to_string(&exported).expect("the export");
    let at = text.find("Name=\"-201U4\"").expect("-201U4 in the file");
    let start =
        "<Attribute Name=\"StartAddress\" AttributeDataType=\"xs:int\">\n                <Value>1</Value>";
    let offset = text[at..].find(start).expect("-201U4's start") + at;
    let overlapping = format!(
        "{}{}{}",
        &text[..offset],
        start.replace("<Value>1</Value>", "<Value>0</Value>"),
        &text[offset + start.len()..]
    );
    let file = tmp.0.join("overlap.aml");
    std::fs::write(&file, overlapping).expect("the file");
    dispatch(&mut h, "plc.import", json!({"path": file.to_string_lossy()}));
    settle(&mut h);
    let card = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| match h.state().shell().dialog() {
        Some(Dialog::PlcImport(card)) => (**card).clone(),
        other => panic!("{other:?}"),
    };
    let shown = card(&h);
    let why = shown.refusal.clone().expect("Apply's refusal in the preview");
    assert!(
        shown
            .changes
            .iter()
            .any(|c| c.refused.as_deref() == Some(why.as_str())),
        "the start's row is marked: {:?}",
        shown.changes
    );
    assert_eq!(
        h.get_all_by_label(&why).count(),
        2,
        "above the tabs and under the start's row"
    );
    assert!(!shown.complete);
    h.get_by_label_contains("The file is complete").click();
    settle(&mut h);
    assert!(card(&h).complete, "the preview made again with the file complete");
    h.state_mut().shell_mut().shutdown();
}

/// A settings form's action called with some of its values keeps the others: a value left out is the stored one, and
/// only an empty text clears it (Symbol policy…, Mode and safety…, Fluid settings…). Fluid settings… offers the
/// format's tube materials.
#[test]
fn a_settings_value_left_out_keeps_the_stored_one() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let record = |role: NavRole| {
        h.state()
            .shell()
            .nav(NavKind::Plc)
            .and_then(|n| n.table())
            .and_then(|t| {
                t.nodes
                    .iter()
                    .find(|n| n.role == role)
                    .map(|n| n.records[0].to_string())
            })
            .expect("the row")
    };
    let (config, channel) = (record(NavRole::ConfigProject), record(NavRole::Channel));
    let config_row = rows
        .iter()
        .find(|r| r.role == NavRole::ConfigProject.word())
        .expect("the configuration project")
        .clone();
    dispatch(
        &mut h,
        "plc.symbol_policy",
        json!({"config": config, "template": "{target.dt}_{pin}", "tag_table": "Valves"}),
    );
    dispatch(
        &mut h,
        "plc.symbol_policy",
        json!({"config": config, "tag_table": "Pumps"}),
    );
    h.get_by_label(&config_row.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Symbol policy…").click();
    settle(&mut h);
    assert_eq!(form_value(&h, "template"), "{target.dt}_{pin}", "kept");
    assert_eq!(form_value(&h, "tag_table"), "Pumps");
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(
        &mut h,
        "plc.channel_settings",
        json!({"channel": channel, "evaluation": "1oo1", "requirement": "SIL2"}),
    );
    dispatch(
        &mut h,
        "plc.channel_settings",
        json!({"channel": channel, "requirement": ""}),
    );
    dispatch(&mut h, "plc.channel_settings", json!({"channel": channel}));
    assert_eq!(form_value(&h, "evaluation"), "1oo1", "kept");
    assert_eq!(form_value(&h, "requirement"), "", "cleared by the empty text");
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(
        &mut h,
        "fluid.settings",
        json!({"od": "6", "id": "4", "material": "pu", "line_tags": true}),
    );
    dispatch(&mut h, "fluid.settings", json!({"od": "8"}));
    dispatch(&mut h, "fluid.settings", json!({}));
    assert_eq!(form_value(&h, "od"), "8");
    assert_eq!(form_value(&h, "id"), "4", "kept");
    assert_eq!(form_value(&h, "material"), "pu", "kept");
    assert_eq!(form_value(&h, "line_tags"), "true", "kept");
    let materials: Vec<String> = match h.state().shell().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f
            .fields
            .iter()
            .find(|f| f.name == "material")
            .map(|f| f.options.iter().map(|(w, _)| w.clone()).collect())
            .unwrap_or_default(),
        other => panic!("{other:?}"),
    };
    assert!(
        ["pu", "steel"].iter().all(|m| materials.iter().any(|w| w == m)),
        "{materials:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A click on a channel's row in the PLC navigator selects the channel alone (not its module beside it), and
/// Properties shows the channel.
#[test]
fn a_channel_row_selects_the_channel_alone() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    let row = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Channel.word() && r.label == "Q0.1")
        .expect("Q0.1's row");
    h.get_by_label(&row.name).click();
    settle(&mut h);
    let selected: Vec<String> = h
        .state()
        .shell()
        .selection()
        .items
        .iter()
        .map(|e| e.0.to_string())
        .collect();
    let channel = h
        .state()
        .shell()
        .nav(NavKind::Plc)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Channel && n.label == "Q0.1")
                .map(|n| n.records[0])
        })
        .expect("the channel");
    assert_eq!(selected, [channel.to_string()]);
    assert!(
        h.query_by_label_contains("No common fields").is_none(),
        "Properties shows the channel"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The PLC export form checks its values before it asks for the file: a document date that is no date is refused in
/// the form, which stays open with what was typed, and no file is asked for.
#[test]
fn the_plc_export_form_refuses_a_bad_date_before_the_file() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    support::open_menu(&mut h, &["PLC"]);
    settle(&mut h);
    h.get_by_label("Export to PLC software…").click();
    settle(&mut h);
    type_into(&mut h, "Document date (empty: the project's)", "tomorrow");
    h.get_by_label("Export").click();
    settle(&mut h);
    match h.state().shell().dialog() {
        Some(Dialog::Form(f)) if f.action == "plc.export" => {
            assert!(
                f.error.as_deref().is_some_and(|e| e.contains("is no date")),
                "{:?}",
                f.error
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(form_value(&h, "date"), "tomorrow");
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

/// FT-1752: the project's PLC and fluid settings from the PLC and Fluid menus. Process-tag scheme… and Fluid settings… are
/// forms that start with the project's values and set them in one step each; Channel addresses at terminals is a switch.
#[test]
fn the_process_tag_and_fluid_settings_are_set_from_the_project_menu() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    support::open_menu(&mut h, &["PLC", "Addresses"]);
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
    support::open_menu(&mut h, &["Fluid"]);
    settle(&mut h);
    support::menu_node(&h, "Fluid settings…").click();
    settle(&mut h);
    type_into(&mut h, "Default tube: outer diameter (mm)", "6");
    type_into(&mut h, "Default tube: inner diameter (mm)", "4");
    h.get_by_label("Tag tubes by page and counter").click();
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
    support::open_menu(&mut h, &["PLC", "Addresses"]);
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

/// FT-1752 (R2, change it): in the station's view a module dragged by its card onto a free slot moves there
/// (`fields.set plc_module.slot`), and its addresses follow; one Undo puts it back. Dragged onto another module's
/// card the two change places.
#[test]
fn a_module_is_dragged_to_another_slot_in_the_station_view() {
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
    support::menu_node(&h, "Rack…").click();
    settle(&mut h);
    let id = station_of(&h);
    take(&mut h, "EW-GEN-DO-1");
    let rows = |h: &egui_kittest::Harness<'static, ew_ui::app::App>| {
        h.state().shell().rack_view(id).expect("the rack").rows
    };
    let tag = rows(&h)[0].tag.clone();
    // The free slot the module is dropped on.
    let target = rows(&h)
        .iter()
        .rfind(|r| r.module.is_none())
        .map(|r| r.slot)
        .expect("a free slot");
    let from = at(&h, &format!("Slot 1, {tag}"));
    let to = at(&h, &format!("Free slot {target}"));
    drag(&mut h, from, to);
    let slot_of = |h: &egui_kittest::Harness<'static, ew_ui::app::App>, tag: &str| {
        rows(h).iter().find(|r| r.tag == tag).map(|r| r.slot)
    };
    assert_eq!(
        slot_of(&h, &tag),
        Some(target),
        "moved onto the slot it was dropped on"
    );
    // One undo step puts it back.
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    assert_eq!(slot_of(&h, &tag), Some(1), "back in slot 1");

    // A second module; the first dropped on it: the two change places, and their addresses with them.
    take(&mut h, "EW-GEN-DO-1");
    let before = rows(&h);
    let (first, second) = (before[0].clone(), before[1].clone());
    let from = at(&h, &format!("Slot 1, {}", first.tag));
    let to = at(&h, &format!("Slot 2, {}", second.tag));
    drag(&mut h, from, to);
    let after = rows(&h);
    assert_eq!(
        (after[0].module, after[1].module),
        (second.module, first.module),
        "the two changed places"
    );
    assert_eq!(
        (after[0].addresses.as_str(), after[1].addresses.as_str()),
        (first.addresses.as_str(), second.addresses.as_str()),
        "the addresses follow the slots"
    );
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
        // The CPU's view opens when it is placed; Escape goes back to the page.
        if h.state().shell().station_tab().is_some() {
            dispatch(&mut h, "ui.dismiss", json!({}));
        }
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
        use ew_dl::{Item, ItemKind};
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

/// Draws starter part `part` as a black box from corner cell `a` to corner cell `b`, then the Select tool.
fn black_box(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, part: &str, a: (i64, i64), b: (i64, i64)) {
    dispatch(h, "tool.black_box", json!({"part": part, "template": 0}));
    crate::navigators::cursor_k(h, a);
    dispatch(h, "canvas.activate", json!({}));
    crate::navigators::cursor_k(h, b);
    dispatch(h, "canvas.activate", json!({}));
    dispatch(h, "tool.select", json!({}));
    // A station's view opens when it is placed (a waiting card's chooser likewise); left here.
    if h.state().shell().dialog().is_some() || h.state().shell().station_tab().is_some() {
        dispatch(h, "ui.dismiss", json!({}));
    }
}

/// The PLC panel's row of station `tag`.
fn station_row(
    h: &egui_kittest::Harness<'_, ew_ui::app::App>,
    tag: &str,
) -> ew_ui::ew_viewmodel::shell::ShownRow {
    h.state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Station.word() && r.label == tag)
        .unwrap_or_else(|| panic!("the station row {tag}"))
}

/// Narrows the PLC panel to station `tag` (its filter, so that the row is in sight), right-clicks the station's row
/// and chooses `entry` from its menu; the filter is cleared after.
fn station_menu(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, tag: &str, entry: &str) {
    dispatch(h, "navigator.filter", json!({"kind": "plc", "text": tag}));
    settle(h);
    let row = station_row(h, tag);
    h.get_by_label(&row.name).click_secondary();
    settle(h);
    support::menu_node(h, entry).click();
    settle(h);
    dispatch(h, "navigator.filter", json!({"kind": "plc", "text": ""}));
    settle(h);
}

/// The persona round's job from nothing (an EPLAN designer's): two stations, a controller with two cards and a remote
/// I/O head with one, a PROFIBUS line between them, an address on each, the Bus panel showing the net with both, and
/// the network address list as a page. Every step is the window's: the black box tool, the PLC panel's row menus,
/// Place port DP on a bus-topology page, the Wire tool between the ports, the Bus table's address cells, Report.
#[test]
fn two_stations_and_a_bus_are_built_from_nothing() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("bus.ew").to_string_lossy(), "template": "builtin"}),
    );
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "PLC"}),
    );
    settle(&mut h);
    wait_for_page(&mut h);
    black_box(&mut h, "EW-GEN-CPU-2", (10, 10), (16, 18));
    black_box(&mut h, "EW-GEN-DI-1", (24, 10), (30, 18));
    black_box(&mut h, "EW-GEN-DO-1", (38, 10), (44, 18));
    black_box(&mut h, "EW-GEN-RIO-1", (10, 26), (16, 34));
    black_box(&mut h, "EW-GEN-DI-1", (24, 26), (30, 34));
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    settle(&mut h);
    // The PLC panel: the controller holds its two cards; the head's card waits until it is attached from its row.
    let stations: Vec<String> = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .filter(|r| r.role == NavRole::Station.word())
        .map(|r| r.label)
        .collect();
    let [cpu, head] = stations.as_slice() else {
        panic!("two stations: {stations:?}");
    };
    let (cpu, head) = (cpu.clone(), head.clone());
    station_menu(&mut h, &head, "Attach modules without a station");
    let modules_of = |h: &egui_kittest::Harness<'_, ew_ui::app::App>, tag: &str| {
        let key = station_row(h, tag).key;
        h.state()
            .shell()
            .shown_rows(NavKind::Plc)
            .into_iter()
            .filter(|r| r.role == NavRole::Module.word() && r.parent.as_deref() == Some(key.as_str()))
            .count()
    };
    assert_eq!(modules_of(&h, &cpu), 2, "the controller's two cards");
    assert_eq!(modules_of(&h, &head), 1, "the head's card");
    // A bus-topology page; each station's row places its DP port there.
    dispatch(
        &mut h,
        "page.new",
        json!({"folio": "3", "type": "bus_topology", "title": "PROFIBUS"}),
    );
    wait_for_page(&mut h);
    dispatch(&mut h, "navigator.show", json!({"kind": "plc"}));
    settle(&mut h);
    // On a bus-topology page the port is a box with its socket: two corners.
    for (tag, x) in [(&cpu, 20), (&head, 50)] {
        station_menu(&mut h, tag, "Place port DP");
        for corner in [(x, 30), (x + 8, 36)] {
            crate::navigators::cursor_k(&mut h, corner);
            dispatch(&mut h, "canvas.activate", json!({}));
        }
        dispatch(&mut h, "tool.select", json!({}));
        settle(&mut h);
    }
    // The Wire tool draws the bus line from one port's A to the other's (the canvas takes sheet points).
    let page = h.state().shell().canvas().expect("the topology page").page();
    let ends: Vec<(i64, i64)> = page
        .tables
        .cps
        .iter()
        .filter(|c| c.label.ends_with(":A"))
        .map(|c| page.snapper.page_to_sheet(c.at))
        .collect();
    let [a, b] = ends.as_slice() else {
        panic!("two DP ports: {ends:?}");
    };
    let mm = |p: (i64, i64)| json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6});
    dispatch(&mut h, "tool.wire", json!({}));
    dispatch(&mut h, "canvas.press", mm(*a));
    dispatch(&mut h, "canvas.pointer", mm(*b));
    settle(&mut h);
    dispatch(&mut h, "canvas.press", mm(*b));
    dispatch(&mut h, "tool.finish", json!({}));
    dispatch(&mut h, "tool.select", json!({}));
    settle(&mut h);
    // The Bus panel: the net with both ports; its table takes each port's station address.
    dispatch(&mut h, "rail.show", json!({"pane": "bus"}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Bus);
    let net = rows
        .iter()
        .find(|r| r.text == "PROFIBUS DP")
        .expect("the logical net PROFIBUS DP");
    let members: Vec<&str> = rows
        .iter()
        .filter(|r| r.parent.as_deref() == Some(net.key.as_str()))
        .map(|r| r.label.as_str())
        .collect();
    for tag in [&cpu, &head] {
        assert!(
            members.contains(&format!("{tag} DP").as_str()),
            "{tag} in the net: {members:?}"
        );
    }
    dispatch(&mut h, "navigator.table", json!({"kind": "bus", "on": true}));
    settle(&mut h);
    for address in ["2", "3"] {
        let at = h
            .get_all_by_label_contains("Address TBD")
            .next()
            .expect("a port's address cell")
            .rect()
            .center();
        crate::bus_navigator::double_click(&mut h, at);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        h.event(egui::Event::Text(address.to_owned()));
        settle(&mut h);
        h.key_press(egui::Key::Enter);
        settle(&mut h);
    }
    assert_eq!(
        h.query_all_by_label_contains("Address TBD").count(),
        0,
        "both ports have an address"
    );
    // Report on the Output tab: the network address list as a page.
    h.get_by_role_and_label(egui::Role::Tab, "Output").click();
    settle(&mut h);
    support::bar_button(&h, "Report").click();
    settle(&mut h);
    crate::support::combos(&h)
        .into_iter()
        .next()
        .expect("the Report list")
        .click();
    settle(&mut h);
    h.get_by_label("Network address list").click();
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
        toast.starts_with("Added the report block Network address list from folio "),
        "{toast}"
    );
    assert!(
        h.state().shell().page_rows().iter().any(|r| r.generated),
        "the list's page"
    );
    h.state_mut().shell_mut().shutdown();
}
