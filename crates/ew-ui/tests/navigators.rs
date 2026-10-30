// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigators in the window without rendering (WP-S2.06 part 1, acceptance checks NV4, NV5, NV7, NV9 and NV11):
//! the Devices pane with its AccessKit tree and the chip line, a drag of 106 before 103 that opens the reorder card and
//! Apply, a drag of an unplaced function onto the canvas that arms the Place tool, the virtualised painting and the
//! exact counts while scrolling; the accessible-name walker after every step (`settle`).

mod support;

use egui::{Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigator::RowKind;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Focus, Outcome};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// A drag with the primary button from `from` to `to` in steps, one frame each.
fn drag(h: &mut Harness<'_, App>, from: Pos2, to: Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(from));
    h.run_ok();
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    for step in 1..=8 {
        let t = step as f32 / 8.0;
        h.input_mut()
            .events
            .push(egui::Event::PointerMoved(from + (to - from) * t));
        h.run_ok();
    }
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    settle(h);
}

/// The key of the row labelled `label` with role `role` in an open navigator.
fn row_key(h: &Harness<'_, App>, kind: NavKind, role: NavRole, label: &str) -> String {
    let shell = h.state().shell();
    let t = shell
        .nav(kind)
        .and_then(|n| n.table())
        .expect("an open navigator");
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

/// A point on the first tree item whose name starts with `name`: near its left end (a row's text may be wider than the
/// pane, so its centre can lie outside the clip).
fn item_centre(h: &Harness<'_, App>, name: &str) -> Pos2 {
    h.get_all_by_role(egui::Role::TreeItem)
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.starts_with(name)))
        .unwrap_or_else(|| panic!("no tree item {name}"))
        .rect()
        .left_center()
        + egui::vec2(12.0, 0.0)
}

/// NV5 in the window (FR-UI-007, D16; §6.5, §6.8): View ▸ Navigators ▸ Devices docks the pane and focuses it; its rows
/// are tree items named `<label>, <role>[, <badges>]`; a click on -201U3 with 102 shown keeps 102, selects the device
/// and shows the chips as buttons named `Open 201-E4` and `Open 203-B4`; Enter opens 201; a selection made on the
/// canvas marks the representation's row while the focus stays on the canvas.
#[test]
fn nv5_the_devices_pane_and_the_chip_line() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    settle(&mut h);
    support::open_menu(&mut h, &["View", "Panels"]);
    settle(&mut h);
    support::menu_node(&h, "Navigators").click();
    settle(&mut h);
    support::menu_node(&h, "Devices").click();
    settle(&mut h);
    settle(&mut h);
    assert!(h.state().shell().navigator_open(NavKind::Devices));
    assert_eq!(
        h.state().shell().focus(),
        Some(Focus::Navigator(NavKind::Devices))
    );
    assert!(
        h.query_by_label("Devices navigator").is_some(),
        "the Tree is named"
    );
    // -201U3 is under -201U1: reveal it by its row, then click it.
    let key = row_key(&h, NavKind::Devices, NavRole::Device, "-201U3");
    dispatch(&mut h, "navigator.row", json!({"kind": "devices", "key": key}));
    settle(&mut h);
    settle(&mut h);
    let at = item_centre(&h, "-201U3, device");
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    settle(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(shell.selection().items.len(), 1);
    let chips: Vec<String> = shell.chips().iter().map(|c| c.text()).collect();
    assert_eq!(chips, ["201-E4", "203-B4"]);
    assert!(h.query_by_label("Open 201-E4").is_some());
    assert!(h.query_by_label("Open 203-B4").is_some());
    // FT-531: the chips are doc 16a §6.24's pill jump chips, 20 px high (egui's button was the row height).
    let chip = h.get_by_role_and_label(egui::Role::Button, "Open 201-E4").rect();
    assert!((chip.height() - 20.0).abs() < 0.5, "jump chip {chip:?}");
    let item = h
        .get_all_by_role(egui::Role::TreeItem)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("-201U3, device"))
        })
        .unwrap();
    assert_eq!(item.accesskit_node().is_selected(), Some(true));
    // Enter opens 201 (the list has the focus after the click).
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().shell().current_folio(), Some("201"));
    // A chip opens its page with the drawable selected.
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(
        &mut h,
        "navigator.select",
        json!({"kind": "devices", "keys": [key]}),
    );
    settle(&mut h);
    h.get_by_label("Open 203-B4").click();
    settle(&mut h);
    assert_eq!(h.state().shell().current_folio(), Some("203"));
    let rep = row_key(&h, NavKind::Devices, NavRole::Representation, "203-B4");
    // The canvas origin: the representation's row is marked, the focus stays on the canvas.
    dispatch(&mut h, "ui.focus", json!({"area": "canvas"}));
    let id = h.state().shell().selection().items[0].0.to_string();
    dispatch(&mut h, "select.set", json!({"ids": [id]}));
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.nav(NavKind::Devices).unwrap().marked().contains(&rep));
    assert!(matches!(shell.focus(), Some(Focus::Canvas(_))));
    h.state_mut().shell_mut().shutdown();
}

/// NV7 in the window (FT-360; D8): dragging 106 onto 103 in the Pages navigator opens the reorder card with "106 →
/// 103, 103 → 106"; Apply commits one `page.reorder`; one undo restores the folios.
#[test]
fn nv7_dragging_106_before_103_opens_the_card_and_apply_commits() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    // WP-S2.17 part 3 (D10): Properties sits under Pages, so the page list has half the column; a taller window
    // keeps folio 106 laid out.
    h.set_size(egui::Vec2::new(1000.0, 1000.0));
    settle(&mut h);
    let rows = h.state().shell().page_rows().to_vec();
    let title = |f: &str| rows.iter().find(|r| r.folio == f).unwrap().title.clone();
    let (t103, t106) = (title("103"), title("106"));
    let from = item_centre(&h, &format!("106 {t106}, page"));
    let to = item_centre(&h, &format!("103 {t103}, page"));
    drag(&mut h, from, to);
    let card = h.state().shell().properties().card().map(|c| c.origin.word());
    assert_eq!(card, Some("reorder"));
    assert!(
        h.query_by_label_contains("Reorder pages · 106 → 103, 103 → 106 · ")
            .is_some(),
        "the card's line"
    );
    // WP-S2.09 part 3: the Preview tab shows the card too; its chip and the tab each have the button.
    h.get_all_by_label("Apply").next().unwrap().click();
    settle(&mut h);
    let rows = h.state().shell().page_rows().to_vec();
    let title = |f: &str| rows.iter().find(|r| r.folio == f).unwrap().title.clone();
    assert_eq!(
        (title("103"), title("106")),
        (t106.clone(), t103.clone()),
        "the keys swapped"
    );
    assert!(h.state().shell().can_undo());
    dispatch(&mut h, "edit.undo", json!({}));
    let rows = h.state().shell().page_rows().to_vec();
    let title = |f: &str| rows.iter().find(|r| r.folio == f).unwrap().title.clone();
    assert_eq!((title("103"), title("106")), (t103, t106));
    assert!(!h.state().shell().can_undo(), "one undo step");
    h.state_mut().shell_mut().shutdown();
}

/// NV9 (§6.7): a device made by `command.run` (`device.create` with a main FnDef, so its function has no
/// representation) shows its function row with the badge `unplaced`; dragging the row onto the canvas of folio 102
/// arms the Place tool with that function (`place.pick {function}`).
#[test]
fn nv9_an_unplaced_function_is_dragged_onto_the_canvas() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let def = {
        let s = ew_ui::ew_viewmodel::ew_api::Session::open_read_only(&dir, "test")
            .expect("the copy opens")
            .0;
        let q = s.query();
        let (_, f) = q.functions().iter().next().expect("a function");
        f.def.to_string()
    };
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "device.create", "params": {"letter": "Q", "counter": 990, "main_fndef": def}}),
    );
    dispatch(&mut h, "navigator.show", json!({"kind": "devices"}));
    settle(&mut h);
    let shell = h.state().shell();
    let t = shell.nav(NavKind::Devices).unwrap().table().unwrap().clone();
    let (fi, f) = t
        .nodes
        .iter()
        .enumerate()
        .find(|(_, n)| {
            n.role == NavRole::Function
                && n.badges.iter().any(|b| b == "unplaced")
                && n.parent
                    .is_some_and(|p| t.nodes[p as usize].label.contains("Q990"))
        })
        .expect("the new device's unplaced function row");
    let _ = fi;
    let key = f.key.clone();
    dispatch(
        &mut h,
        "navigator.select",
        json!({"kind": "devices", "keys": [key]}),
    );
    settle(&mut h);
    settle(&mut h);
    let from = h
        .get_all_by_role(egui::Role::TreeItem)
        .find(|n| n.accesskit_node().is_selected() == Some(true))
        .expect("the marked row")
        .rect()
        .left_center()
        + egui::vec2(12.0, 0.0);
    let canvas = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas")
        .rect()
        .center();
    drag(&mut h, from, canvas);
    assert_eq!(
        h.state().shell().tool().name(),
        "place",
        "the Place tool is armed"
    );
    h.state_mut().shell_mut().shutdown();
}

/// WP-S2.07 §6.8 in the window: in the strip editor (table mode) a terminal inserted into 1X1 (no strip segment, so
/// unplaced) is a drag source; dragging its row onto the canvas of folio 101 arms the Place tool with its function.
#[test]
fn s68_an_unplaced_terminal_row_is_dragged_onto_the_canvas() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    // Tall enough that the strip editor's rows stand clear of the dock's splitter.
    h.set_size(egui::Vec2::new(1400.0, 1100.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(&mut h, "strip.edit", json!({"strip": "1X1"}));
    dispatch(&mut h, "strip.insert", json!({"designation": "9"}));
    settle(&mut h);
    settle(&mut h);
    let from = h
        .get_all_by_role(egui::Role::Cell)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Terminal 9,") || l.starts_with("Terminal 9 "))
        })
        .expect("the new row's Terminal cell")
        .rect()
        .center();
    let canvas = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas")
        .rect()
        .center();
    drag(&mut h, from, canvas);
    assert_eq!(
        h.state().shell().tool().name(),
        "place",
        "the Place tool is armed"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Cycle 37 (window check W37): a device tag refused in the Devices navigator's F2 field shows its refusal under the
/// field only, not again as a banner.
#[test]
fn an_f2_refusal_shows_under_the_field_and_raises_no_banner() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(&mut h, "navigator.show", json!({"kind": "devices"}));
    dispatch(&mut h, "select.set", json!({"refs": ["tag:-102G1"]}));
    settle(&mut h);
    dispatch(&mut h, "navigator.rename", json!({"kind": "devices"}));
    settle(&mut h);
    assert!(h.state().shell().nav_rename().is_some(), "the inline field");
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("-102F1".to_owned()));
    h.run_ok();
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.nav_rename_error().is_some(), "the refusal under the field");
    assert_eq!(shell.banner(), None, "no banner beside it");
    h.state_mut().shell_mut().shutdown();
}

/// WP-S2.07 CT4 in the window (FT-490): F2 on a Potentials row opens the inline editor on its name with the focus;
/// typing (appended at the end) and Enter rename the definition; Delete on a row opens the delete card.
#[test]
fn ct4_f2_renames_a_potential_and_delete_opens_the_card() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "navigator.show", json!({"kind": "potentials"}));
    settle(&mut h);
    settle(&mut h);
    let key = row_key(&h, NavKind::Potentials, NavRole::Potential, "24VDC-1");
    dispatch(&mut h, "navigator.row", json!({"kind": "potentials", "key": key}));
    dispatch(
        &mut h,
        "ui.focus",
        json!({"area": "navigator", "kind": "potentials"}),
    );
    settle(&mut h);
    h.key_press(egui::Key::F2);
    settle(&mut h);
    let r = h.state().shell().nav_rename().expect("the editor").clone();
    assert_eq!(r.text, "24VDC-1");
    let field = h.get_by_role_and_label(egui::Role::TextInput, "Rename 24VDC-1, potential");
    assert!(field.accesskit_node().is_focused(), "the editor has the focus");
    h.get_by_role_and_label(egui::Role::TextInput, "Rename 24VDC-1, potential")
        .type_text("9");
    h.run_ok();
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.nav_rename().is_none());
    assert_eq!(
        shell.recorded_commands().last().map(|c| c.cmd.as_str()),
        Some("fields.set")
    );
    let value = shell.recorded_commands().last().unwrap().params["value"].clone();
    assert_eq!(value, "24VDC-19");
    // Delete on the focused row: the card, nothing committed.
    let n = h.state().shell().recorded_commands().len();
    dispatch(
        &mut h,
        "ui.focus",
        json!({"area": "navigator", "kind": "potentials"}),
    );
    settle(&mut h);
    h.key_press(egui::Key::Delete);
    settle(&mut h);
    let card = h.state().shell().properties().card().expect("the delete card");
    assert_eq!(card.reason, "reason-nav-delete");
    assert_eq!(h.state().shell().recorded_commands().len(), n);
    h.state_mut().shell_mut().shutdown();
}

/// NV4 and NV11 in the window (§9 item 4, §11): the Devices pane paints at most the rows in view plus two; opening it
/// makes one `navigator_table` call; scrolling makes none and builds no rows.
#[test]
fn nv11_scrolling_the_devices_pane_paints_the_rows_in_view_and_calls_nothing() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    let calls = h.state().shell().session_calls().navigator_table;
    dispatch(&mut h, "navigator.show", json!({"kind": "devices"}));
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().session_calls().navigator_table, calls + 1);
    let total = h.state().shell().shown_rows(NavKind::Devices).len();
    let tree = h.get_by_label("Devices navigator").rect();
    let in_view = (tree.height() / ew_ui::navigator::row_height(&h.ctx)).ceil() as usize;
    let (built, painted) = h.state().navigator_pane_stats(NavKind::Devices).unwrap();
    assert!(total > in_view + 2, "{total} rows, {in_view} in view");
    assert!(painted <= in_view + 2, "{painted} painted, {in_view} in view");
    h.get_by_label("Devices navigator").scroll_down();
    for _ in 0..5 {
        h.run_ok();
    }
    let (built_after, painted_after) = h.state().navigator_pane_stats(NavKind::Devices).unwrap();
    assert_eq!(built_after, built, "scrolling builds no rows");
    assert!(painted_after <= in_view + 2);
    assert_eq!(h.state().shell().session_calls().navigator_table, calls + 1);
    // The Pages navigator's rows stay as built (D5): the rows of section 1 are items of its tree.
    let shell = h.state().shell();
    assert!(
        shell
            .navigator_rows()
            .iter()
            .any(|r| matches!(r.kind, RowKind::Page(_)))
    );
    h.state_mut().shell_mut().shutdown();
}

/// R30 F9 in the window: a double click on the paged Wire List row of the Report blocks navigator goes to folio 907.
#[test]
fn r30_double_clicking_a_paged_report_block_goes_to_its_first_folio() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    // WP-S2.17 part 2: the command bar (85 px) is taller than the menu bar and the tool strip it replaced; at 700 px
    // the virtualised pane no longer lays out the Wire List row at all, so this window is 800 px high; WP-S2.17 part 3
    // (D10) gives the navigator half the column (Properties under it), so 1200 px.
    h.set_size(egui::Vec2::new(1000.0, 1200.0));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(&mut h, "navigator.show", json!({"kind": "blocks"}));
    // FT-567: the list ends on a whole row, so a row under the fold is not laid out at all (it was cut, and so in the
    // tree); the filter brings the Wire List row up.
    dispatch(
        &mut h,
        "navigator.filter",
        json!({"kind": "blocks", "text": "Wire List"}),
    );
    settle(&mut h);
    settle(&mut h);
    // WP-S2.17 part 1: rows of `size.row` (28 px) put the row under the pane's fold in this window; scroll it in.
    h.get_all_by_role(egui::Role::TreeItem)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Wire List"))
        })
        .unwrap()
        .scroll_to_me();
    settle(&mut h);
    let at = item_centre(&h, "Wire List");
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false, true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().current_folio(), Some("907"));
    assert_eq!(
        h.state().shell().selection().items.len(),
        1,
        "the block is selected"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Doc 16a §6.5: the shared filter field has a trailing clear button only while it holds text; the button empties the
/// filter and leaves the keyboard focus in the field.
#[test]
fn the_filter_field_clear_button_empties_the_filter_and_keeps_the_focus() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    assert!(
        h.query_by_label("Clear the filter").is_none(),
        "an empty field has no clear button"
    );
    dispatch(&mut h, "pages.filter", json!({"text": "10"}));
    settle(&mut h);
    settle(&mut h);
    h.get_by_label("Clear the filter").click();
    settle(&mut h);
    assert_eq!(h.state().shell().navigator().filter, "");
    assert!(
        h.get_all_by_role(egui::Role::TextInput).any(|n| n.is_focused()),
        "the field keeps the focus"
    );
    assert!(h.query_by_label("Clear the filter").is_none());
    h.state_mut().shell_mut().shutdown();
}

/// The grid cursor of the focused canvas to grid index `k`.
fn cursor_k(h: &mut Harness<'_, App>, k: (i64, i64)) {
    let (x, y) = {
        let c = h.state().shell().canvas().unwrap();
        let sn = &c.page().snapper;
        let s = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
        (s.0 as f64 / 1e6, s.1 as f64 / 1e6)
    };
    dispatch(h, "cursor.set", json!({"x_mm": x, "y_mm": y}));
}

/// WP-S2.07 §8 (FR-PLC-020, PG2) in the window: in a new project with the generic CPU, the digital output card and a
/// starter coil attached to the card, the PLC navigator's row of the card's channel 0 dragged onto the coil's
/// connection point binds it with one `plc.bind_channel`; the coil's CP then prints Q0.0.
#[test]
fn pg2_a_channel_row_dragged_onto_a_connection_point_binds_it() {
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1400.0, 1000.0));
    settle(&mut h);
    dispatch(
        &mut h,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
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
        cursor_k(&mut h, a);
        dispatch(&mut h, "canvas.activate", json!({}));
        cursor_k(&mut h, b);
        dispatch(&mut h, "canvas.activate", json!({}));
        dispatch(&mut h, "tool.select", json!({}));
    }
    dispatch(&mut h, "tool.place", json!({}));
    dispatch(
        &mut h,
        "place.pick",
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
    );
    cursor_k(&mut h, (40, 30));
    dispatch(&mut h, "canvas.activate", json!({"alt": true}));
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "navigator.show", json!({"kind": "plc"}));
    settle(&mut h);
    let (coil, module) = {
        let shell = h.state().shell();
        let t = std::sync::Arc::clone(&shell.canvas().unwrap().page().tables);
        let coil = t
            .reps
            .iter()
            .find(|r| r.black_box.is_none() && r.symbol_key.starts_with("ew.iec.coil"))
            .expect("the coil")
            .clone();
        let table = shell.nav(NavKind::Plc).and_then(|n| n.table()).unwrap();
        let module = table
            .nodes
            .iter()
            .find(|n| n.role == NavRole::Channel)
            .expect("the card's channels")
            .records[1];
        (coil, module)
    };
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "assembly.attach", "params": {
            "parent": module.to_string(),
            "children": [{"device": coil.device.unwrap().to_string(), "position": 1}],
        }}),
    );
    settle(&mut h);
    settle(&mut h);
    let cp = {
        let shell = h.state().shell();
        let t = std::sync::Arc::clone(&shell.canvas().unwrap().page().tables);
        t.cps
            .iter()
            .filter(|c| c.rep == coil.id)
            .min_by_key(|c| c.idx)
            .expect("the coil's CP")
            .clone()
    };
    assert_eq!(cp.plc_address, None);
    let to = {
        let bounds = h
            .get_all_by_role(egui::Role::Canvas)
            .next()
            .expect("the canvas")
            .accesskit_node()
            .raw_bounds()
            .expect("bounds");
        let c = h.state().shell().canvas().unwrap();
        let s = c.page().snapper.page_to_sheet(cp.at);
        let k = c.points_per_nm();
        Pos2::new(
            bounds.x0 as f32 + ((s.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
            bounds.y0 as f32 + ((s.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
        )
    };
    let from = item_centre(&h, "Q0.0, channel");
    let rev = h.state().shell().revision();
    drag(&mut h, from, to);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1), "one command");
    let bound = {
        let shell = h.state().shell();
        let t = std::sync::Arc::clone(&shell.canvas().unwrap().page().tables);
        t.cps
            .iter()
            .find(|c| c.rep == coil.id && c.idx == cp.idx)
            .and_then(|c| c.plc_address.clone())
    };
    assert_eq!(bound.as_deref(), Some("Q0.0"));
    h.state_mut().shell_mut().shutdown();
}
