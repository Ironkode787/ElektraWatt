// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel in the window without rendering (WP-S2.09 part 2, acceptance check MS5 and the window half of
//! MS1 and MS4): View ▸ Messages docks the pane at the bottom with its header line, rows and footer; the finding rows
//! are tree items named `<label>, finding, <severity>[, fix][, pending]`; F8 and Shift+F8 walk the findings from the
//! keyboard; the detail block names the cursor's rule and its Rule page link opens the rule's page in the browser; a
//! new unpaired interruption point on the editing page is a pending row; the accessible-name walker after every step.

use crate::support;

use egui::Modifiers;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavNode, NavRole};
use ew_ui::ew_viewmodel::shell::{Focus, Mode, Resources, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, dispatch, harness_with, settle, wait_for_page};

/// A copy of Golden A in the window, with an installed user guide that holds the rule pages.
fn on_golden(tmp: &Tmp, platform: Scripted) -> Harness<'static, App> {
    let docs = tmp.0.join("docs");
    std::fs::create_dir_all(docs.join("reference/rules")).unwrap();
    std::fs::write(docs.join("reference/rules/ew-core-033.html"), "<html></html>").unwrap();
    let config = tmp.config().with_resources(Resources {
        sample_dir: None,
        content_dir: None,
        docs_dir: Some(docs),
        notices_file: None,
        privacy_file: None,
    });
    let dir = tmp.golden("g.ew");
    let mut h = harness_with(ShellModel::new(Mode::Gui, config), platform, Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn table_nodes(h: &Harness<'_, App>) -> Vec<NavNode> {
    h.state()
        .shell()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .map(|t| t.nodes.clone())
        .expect("the Messages pane with its table")
}

/// The finding row of rule `rule` whose subject ends with `subject`.
fn finding(h: &Harness<'_, App>, rule: &str, subject: &str) -> NavNode {
    table_nodes(h)
        .into_iter()
        .find(|n| n.role == NavRole::Finding && n.cells[1] == rule && n.cells[3].ends_with(subject))
        .unwrap_or_else(|| panic!("no {rule} finding on {subject}"))
}

/// The accessible name of the tree item whose name starts with `label`.
fn item_name(h: &Harness<'_, App>, label: &str) -> String {
    h.get_all_by_role(egui::Role::TreeItem)
        .filter_map(|n| n.accesskit_node().label())
        .find(|l| l.starts_with(label))
        .unwrap_or_else(|| panic!("no tree item {label}"))
}

/// Places an interruption point named `name` with direction `target` near the top right of the shown page.
fn place_ip(h: &mut Harness<'_, App>, name: &str) {
    dispatch(h, "tool.ip", json!({}));
    let (x, y) = {
        let shell = h.state().shell();
        let sn = &shell.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(60), sn.page_of_index(3)))
    };
    dispatch(
        h,
        "cursor.set",
        json!({"x_mm": x as f64 / 1e6, "y_mm": y as f64 / 1e6}),
    );
    dispatch(h, "ip.name", json!({ "text": name }));
    for _ in 0..3 {
        let state: Value =
            serde_json::from_str(&ew_ui::ew_viewmodel::script::state_json(h.state().shell())).unwrap();
        if state["canvas"]["ghost"]["direction"] == "target" {
            break;
        }
        dispatch(h, "tool.ip_direction", json!({}));
    }
    let rev = h.state().shell().revision();
    dispatch(h, "canvas.activate", json!({}));
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1), "one IP placed");
    dispatch(h, "ui.dismiss", json!({}));
    settle(h);
}

/// MS5 (FR-UI-025, FR-UI-041; §7.3): the pane from the View menu, its named rows, its header and footer, the keyboard
/// walk, the detail block with the Rule page link, and a pending row.
#[test]
fn the_pane_in_the_window_named_rows_keyboard_detail_and_pending() {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    let opened = platform.opened.clone();
    let mut h = on_golden(&tmp, platform);
    // With the controls and rows at their token sizes (28 points) the docked pane of a 700-point
    // window has no room left for a row below its header; 800 points (the height of the snapshots' window) does.
    h.set_size(egui::Vec2::new(1000.0, 800.0));
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(&mut h, "view.zoom_actual", json!({}));
    settle(&mut h);
    // The Table toggles before the Messages pane opens (it adds none: it has no table mode).
    let toggles = h.query_all_by_label("Table").count();
    // View ▸ Messages docks the pane at the bottom and focuses it.
    support::open_menu(&mut h, &["Check"]);
    settle(&mut h);
    support::menu_node(&h, "Messages").click();
    settle(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.navigator_open(NavKind::Messages));
    assert_eq!(shell.focus(), Some(Focus::Navigator(NavKind::Messages)));
    assert!(
        h.query_by_label("Messages navigator").is_some(),
        "the Tree is named"
    );
    assert_eq!(
        h.query_all_by_label("Table").count(),
        toggles,
        "the Messages pane has no Table toggle"
    );
    let counts = shell.messages_counts().unwrap();
    let header = format!(
        "Drafting · E {} · W {} · I {} · waived {} · pending {}",
        counts.errors, counts.warnings, counts.infos, counts.waived, counts.pending
    );
    assert!(h.query_by_label(&header).is_some(), "the header line {header}");
    assert!(
        h.query_by_label("Checks support, but do not replace, engineering verification.")
            .is_some(),
        "the footer"
    );
    // Every finding row on show is a tree item named `<label>, finding, <severity>…`.
    let items: Vec<String> = h
        .get_all_by_role(egui::Role::TreeItem)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.contains(", finding, "))
        .collect();
    assert!(!items.is_empty(), "finding rows are painted");
    for name in &items {
        let tail = name.rsplit(", finding, ").next().unwrap();
        let severity = tail.split(", ").next().unwrap();
        assert!(["error", "warning", "info"].contains(&severity), "{name}");
    }
    // F8 from the keyboard goes to the first finding, Shift+F8 twice wraps to the last.
    let order: Vec<String> = h
        .state()
        .shell()
        .shown_rows(NavKind::Messages)
        .iter()
        .filter(|r| r.role == "finding")
        .map(|r| r.key.clone())
        .collect();
    assert!(order.len() >= 2);
    h.key_press(egui::Key::F8);
    settle(&mut h);
    assert_eq!(
        h.state().shell().nav_cursor(NavKind::Messages).as_deref(),
        Some(order[0].as_str())
    );
    h.key_press(egui::Key::F8);
    settle(&mut h);
    assert_eq!(
        h.state().shell().nav_cursor(NavKind::Messages).as_deref(),
        Some(order[1].as_str())
    );
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::F8);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::F8);
    settle(&mut h);
    assert_eq!(
        h.state().shell().nav_cursor(NavKind::Messages).as_deref(),
        order.last().map(String::as_str)
    );
    // EF-08: an error row named with its severity; the detail block names its rule; the Rule page link opens the
    // rule's page of the installed user guide.
    let ef08 = finding(&h, "EW-CORE-033", "-3W1");
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "key": ef08.key}),
    );
    settle(&mut h);
    settle(&mut h);
    let name = item_name(&h, &ef08.label);
    assert!(
        name.starts_with(&format!("{}, finding, error", ef08.label)),
        "{name}"
    );
    assert!(!name.ends_with(", pending"), "{name}");
    let title = ew_ui::ew_viewmodel::ew_api::checks::rule("EW-CORE-033")
        .unwrap()
        .meta()
        .title;
    assert!(
        h.query_by_label(&format!("EW-CORE-033 {title}")).is_some(),
        "the detail block names the rule"
    );
    // The detail block keeps to its pane: in this short window it scrolls, it never spills under the status bar.
    h.get_by_label("Rule page").scroll_to_me();
    settle(&mut h);
    let status_top = h
        .query_all_by_label("saved")
        .map(|n| n.rect().top())
        .fold(f32::INFINITY, f32::min);
    let link = h.get_by_label("Rule page").rect();
    assert!(
        link.bottom() <= status_top,
        "{link:?} above the status bar at {status_top}"
    );
    h.get_by_label("Rule page").click();
    settle(&mut h);
    let urls = opened.borrow().clone();
    assert!(
        urls.last()
            .is_some_and(|u| u.ends_with("reference/rules/ew-core-033.html")),
        "{urls:?}"
    );
    // A new unpaired interruption point on 101 is a pending row: named `…, error, pending`.
    place_ip(&mut h, "MSGTEST");
    let row = finding(&h, "EW-CORE-051", "MSGTEST");
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "key": row.key}),
    );
    settle(&mut h);
    settle(&mut h);
    let name = item_name(&h, &row.label);
    assert!(name.ends_with(", finding, error, pending"), "{name}");
    let shown = h
        .state()
        .shell()
        .shown_rows(NavKind::Messages)
        .into_iter()
        .find(|r| r.key == row.key)
        .unwrap();
    assert!(shown.finding.is_some_and(|f| f.pending && f.severity == "error"));
    h.state_mut().shell_mut().shutdown();
}

/// W31-1: at 1280×800 the bottom dock holds the Messages pane's header, at least three whole rows, and with a
/// finding selected both its row (of at least two whole rows) and its detail block, with the footer below.
#[test]
fn w31_1_the_selected_row_and_its_detail_are_both_on_show_at_1280_by_800() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, Scripted::default());
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    support::open_menu(&mut h, &["Check"]);
    settle(&mut h);
    support::menu_node(&h, "Messages").click();
    settle(&mut h);
    settle(&mut h);
    let whole = |h: &Harness<'_, App>| -> (egui::Rect, Vec<(String, egui::Rect)>) {
        let list = h.get_by_label("Messages navigator").rect();
        let rows = h
            .get_all_by_role(egui::Role::TreeItem)
            .filter_map(|n| n.accesskit_node().label().map(|l| (l, n.rect())))
            .filter(|(_, r)| list.expand(0.5).contains_rect(*r))
            .collect();
        (list, rows)
    };
    let (list, rows) = whole(&h);
    assert!(rows.len() >= 3, "three whole rows in {list:?}: {rows:?}");
    // A finding further down the list: its row is scrolled into view next to its detail block.
    let findings: Vec<(String, String)> = h
        .state()
        .shell()
        .shown_rows(NavKind::Messages)
        .iter()
        .filter(|r| r.role == "finding")
        .map(|r| (r.key.clone(), r.name.clone()))
        .collect();
    let (key, label) = findings[findings.len().min(6) - 1].clone();
    dispatch(&mut h, "navigator.row", json!({"kind": "messages", "key": key}));
    settle(&mut h);
    settle(&mut h);
    let (list, rows) = whole(&h);
    // Beside a detail block the list keeps at least two rows (was three).
    assert!(rows.len() >= 2, "two whole rows beside the detail: {rows:?}");
    assert!(
        rows.iter().any(|(l, _)| l.starts_with(&label)),
        "the selected row {label} in {list:?}: {rows:?}"
    );
    let detail = h.state().shell().messages_detail().unwrap();
    let rule = h
        .get_by_label(&format!("{} {}", detail.rule, detail.title))
        .rect();
    assert!(
        rule.top() >= list.bottom() - 0.5 && rule.bottom() <= 800.0,
        "the detail {rule:?} below the rows {list:?}"
    );
    let footer = h
        .get_by_label("Checks support, but do not replace, engineering verification.")
        .rect();
    assert!(
        footer.top() >= rule.bottom() && footer.bottom() <= 800.0,
        "the footer {footer:?} below the detail {rule:?}"
    );
    // X32b: the Rule page link stands on the detail's title line, whole above the footer and inside the pane.
    let link = h.get_by_label("Rule page").rect();
    assert!(
        link.top() >= list.bottom() - 0.5 && link.bottom() <= footer.top() + 0.5,
        "the link {link:?} between the rows {list:?} and the footer {footer:?}"
    );
    assert!(
        (link.center().y - rule.center().y).abs() <= 2.0 && link.right() <= list.right() + 0.5,
        "the link {link:?} at the right end of the title line {rule:?}"
    );
    // C39 (W39c2): the link keeps a gutter from the detail's scroll bar at the pane's right edge.
    let tokens = ew_ui::theme::current(&h.ctx);
    assert!(
        link.right() <= list.right() - tokens.space_2,
        "the link {link:?} touches the pane's edge {list:?}"
    );
}

/// `navigator.row` to a finding under a collapsed group expands the path and scrolls the cursor row into view;
/// a finding out of view is scrolled to as well (Golden A's findings, the window 1280 by 800; FX38d: the dock leaves the
/// page view half the window, so a 640-high window shows too few rows).
#[test]
fn navigator_row_into_a_collapsed_group_expands_it_and_scrolls_the_row_into_view() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, Scripted::default());
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    support::open_menu(&mut h, &["Check"]);
    settle(&mut h);
    support::menu_node(&h, "Messages").click();
    settle(&mut h);
    settle(&mut h);
    let on_show = |h: &Harness<'_, App>, label: &str| -> bool {
        let list = h.get_by_label("Messages navigator").rect();
        h.get_all_by_role(egui::Role::TreeItem)
            .filter_map(|n| n.accesskit_node().label().map(|l| (l, n.rect())))
            .any(|(l, r)| l.starts_with(label) && list.expand(0.5).contains_rect(r))
    };
    // The last finding of the tree (every group expanded) and its group.
    let rows = h.state().shell().shown_rows(NavKind::Messages);
    let target = rows
        .iter()
        .rev()
        .find(|r| r.role == "finding" && r.parent.is_some())
        .expect("a finding under a group")
        .clone();
    let group = target.parent.clone().unwrap();
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "to": "first"}),
    );
    dispatch(
        &mut h,
        "navigator.toggle",
        json!({"kind": "messages", "key": group}),
    );
    settle(&mut h);
    settle(&mut h);
    let shown = h.state().shell().shown_rows(NavKind::Messages);
    assert!(
        !shown.iter().any(|r| r.key == target.key),
        "the finding is hidden under its collapsed group"
    );
    assert!(
        !on_show(&h, &target.name),
        "the finding is not on show before the move"
    );
    // The move: the group opens, the cursor is on the finding and its row is scrolled into the list.
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "key": target.key}),
    );
    settle(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(
        shell.nav_cursor(NavKind::Messages).as_deref(),
        Some(target.key.as_str())
    );
    let group_row = shell
        .shown_rows(NavKind::Messages)
        .into_iter()
        .find(|r| r.key == group)
        .unwrap();
    assert_eq!(group_row.expanded, Some(true), "the group is expanded again");
    assert!(
        on_show(&h, &target.name),
        "the cursor row {} is on show",
        target.name
    );
    // A finding out of view: back to the first row, then to the last finding by key.
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "to": "first"}),
    );
    settle(&mut h);
    settle(&mut h);
    assert!(
        !on_show(&h, &target.name),
        "the first row's list does not show the last finding"
    );
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "key": target.key}),
    );
    settle(&mut h);
    settle(&mut h);
    assert!(
        on_show(&h, &target.name),
        "the last finding is scrolled into view"
    );
}

/// Doc 16a §5.7, §6.4 (the rest): the Messages dock tab carries a count chip of the open findings (its
/// node's value), and the bottom dock's tab row has one close button that closes every pane of the dock.
#[test]
fn the_dock_tab_counts_the_findings_and_the_close_button_closes_the_dock() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, Scripted::default());
    h.set_size(egui::Vec2::new(1000.0, 800.0));
    dispatch(&mut h, "messages.show", json!({"on": true}));
    dispatch(&mut h, "history.show", json!({"on": true}));
    settle(&mut h);
    let c = h.state().shell().messages_counts().unwrap();
    let open = c.errors + c.warnings + c.infos;
    assert!(open > 0, "Golden A has open findings");
    let tab = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| {
            n.accesskit_node().label().as_deref() == Some("Messages") && n.accesskit_node().value().is_some()
        })
        .expect("the Messages dock tab with its count");
    assert_eq!(tab.accesskit_node().value(), Some(open.to_string()));
    h.get_by_label("Close the bottom dock").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert!(!shell.navigator_open(NavKind::Messages), "Messages closed");
    assert!(!shell.history_shown(), "History closed");
    assert!(h.query_by_label("Close the bottom dock").is_none());
    // Ctrl+J (`view.bottom_dock`) opens the dock again with the same panes, and closes it again.
    dispatch(&mut h, "ui.focus", json!({"area": "canvas"}));
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.navigator_open(NavKind::Messages), "Messages reopened");
    assert!(shell.history_shown(), "History reopened");
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    settle(&mut h);
    assert!(!h.state().shell().bottom_dock_shown(), "Ctrl+J closes it");
    // With nothing remembered the dock opens on Messages.
    dispatch(&mut h, "view.bottom_dock", json!({"on": true}));
    dispatch(&mut h, "history.show", json!({"on": false}));
    dispatch(&mut h, "messages.show", json!({"on": false}));
    dispatch(&mut h, "view.bottom_dock", json!({}));
    let shell = h.state().shell();
    assert!(shell.navigator_open(NavKind::Messages) && !shell.history_shown());
}

/// Cycle 37 (window check): Golden A's folio 103 without its bond B1 (0 V and PE merge, EW-POT-001 stands). The
/// finding's message names three member CPs and counts the rest, and in a short dock (300 points; FX38d: in a window
/// 900 high, as the dock leaves the page view half the window) its detail block scrolls to
/// the message's last line above the footer instead of cutting it at the pane's edge.
#[test]
fn the_detail_of_a_long_conflict_scrolls_and_names_three_member_cps() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, Scripted::default());
    h.set_size(egui::Vec2::new(1000.0, 900.0));
    dispatch(&mut h, "page.goto", json!({"folio": "103"}));
    dispatch(&mut h, "navigator.show", json!({"kind": "potentials"}));
    let bond = {
        let shell = h.state().shell();
        let t = shell
            .nav(NavKind::Potentials)
            .and_then(|n| n.table())
            .expect("the Potentials table");
        let rows: Vec<_> = t.nodes.iter().filter(|n| n.kinds == ["bond"]).collect();
        assert_eq!(rows.len(), 1, "Golden A has the one bond B1");
        rows[0].records[0]
    };
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "bond", "id": bond.to_string()}]}}),
    );
    dispatch(&mut h, "messages.show", json!({"on": true}));
    dispatch(&mut h, "messages.check", json!({}));
    settle(&mut h);
    let row = table_nodes(&h)
        .into_iter()
        .find(|n| n.role == NavRole::Finding && n.cells[1] == "EW-POT-001")
        .expect("the EW-POT-001 finding");
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "key": row.key}),
    );
    settle(&mut h);
    settle(&mut h);
    let message = h.state().shell().messages_detail().expect("the detail").message;
    assert_eq!(message.matches("member CP ").count(), 3, "{message}");
    assert!(message.contains(" more member CPs"), "{message}");
    let node = h.get_by_label(&message);
    node.scroll_to_me();
    settle(&mut h);
    let footer = h
        .get_by_label("Checks support, but do not replace, engineering verification.")
        .rect();
    let text = h.get_by_label(&message).rect();
    assert!(
        text.bottom() <= footer.top() + 0.5,
        "{text:?} ends above the footer at {footer:?}"
    );
}

/// FT-915 (W42a): with Golden A's first EW-LAY-001 finding selected, the detail block holds its message whole, wrapped
/// to the pane's width, between the list and the footer, and the list keeps at least two whole rows; at the 1280 × 800
/// floor (doc 16a §2: truncation is judged at the floor) and at 1920 × 1080. The ruling (project owner (delegated),
/// cycle 42): the detail is as tall as its content, the list above it keeps at least two rows (was three), and only a
/// still taller detail scrolls; never an ellipsis, a clipped line or a fixed pixel cap.
#[test]
fn the_lay_001_detail_shows_its_message_whole_at_the_floor_and_at_1920() {
    for (w, hgt) in [(1280.0, 800.0), (1920.0, 1080.0)] {
        let tmp = Tmp::new();
        let mut h = on_golden(&tmp, Scripted::default());
        h.set_size(egui::Vec2::new(w, hgt));
        settle(&mut h);
        dispatch(&mut h, "messages.show", json!({"on": true}));
        settle(&mut h);
        let row = table_nodes(&h)
            .into_iter()
            .find(|n| n.role == NavRole::Finding && n.cells[1] == "EW-LAY-001")
            .expect("an EW-LAY-001 finding in Golden A");
        dispatch(
            &mut h,
            "navigator.row",
            json!({"kind": "messages", "key": row.key}),
        );
        settle(&mut h);
        settle(&mut h);
        settle(&mut h);
        let list = h.get_by_label("Messages navigator").rect();
        let rows = h
            .get_all_by_role(egui::Role::TreeItem)
            .filter(|n| list.expand(0.5).contains_rect(n.rect()))
            .count();
        assert!(
            rows >= 2,
            "{w}×{hgt}: two whole rows above the detail in {list:?}, {rows} shown"
        );
        let message = h.state().shell().messages_detail().expect("the detail").message;
        let text = h.get_by_label(&message).rect();
        let footer = h
            .get_by_label("Checks support, but do not replace, engineering verification.")
            .rect();
        assert!(
            text.top() >= list.bottom() - 0.5 && text.bottom() <= footer.top() + 0.5,
            "{w}×{hgt}: the message {text:?} whole between the rows {list:?} and the footer {footer:?}"
        );
        assert!(
            text.right() <= list.right() + 0.5,
            "{w}×{hgt}: the message {text:?} wrapped inside the pane {list:?}"
        );
        h.state_mut().shell_mut().shutdown();
    }
}

/// The texts the last frame painted, each with its galley's rectangle and whether it was cut with an ellipsis.
fn painted_texts(h: &Harness<'_, App>) -> Vec<(String, egui::Rect, bool)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect, bool)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                t.galley.rect.translate(t.pos.to_vec2()),
                t.galley.elided,
            )),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// Golden A's folio 101 with its two junctions joined by a wire and both wiring orders listing that wire first: each
/// junction takes it for its feed, so EW-POT-011 stands on the merged potential, named `24VDC`.
fn ambiguous_junction_feed(h: &mut Harness<'_, App>) {
    dispatch(h, "page.goto", json!({"folio": "101"}));
    settle(h);
    let page = h
        .state()
        .shell()
        .page_rows()
        .iter()
        .find(|r| r.folio == "101")
        .expect("folio 101")
        .id;
    let junctions: Vec<_> = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .junctions
        .iter()
        .map(|j| j.id)
        .collect();
    let [j1, j2] = junctions[..] else {
        panic!("folio 101 has two junctions: {junctions:?}")
    };
    fn junction(id: impl ToString) -> Value {
        json!({"junction": {"junction": id.to_string()}})
    }
    dispatch(
        h,
        "command.run",
        json!({"cmd": "connection.draw", "params": {"page": page.to_string(), "a": junction(j1), "b": junction(j2)}}),
    );
    settle(h);
    let tables = h.state().shell().canvas().unwrap().page().tables.clone();
    let link = tables
        .connections
        .iter()
        .find(|c| {
            matches!(
                (c.a, c.b),
                (TableEnd::Junction { id: a }, TableEnd::Junction { id: b }) if (a, b) == (j1, j2) || (a, b) == (j2, j1)
            )
        })
        .expect("the wire between the junctions")
        .id;
    for j in [j1, j2] {
        let mut sequence: Vec<String> = tables
            .junctions
            .iter()
            .find(|x| x.id == j)
            .expect("the junction")
            .sequence
            .iter()
            .filter(|c| **c != link)
            .map(ToString::to_string)
            .collect();
        sequence.insert(0, link.to_string());
        dispatch(
            h,
            "command.run",
            json!({"cmd": "junction.edit", "params": {"junction": j.to_string(), "sequence": sequence}}),
        );
    }
    dispatch(
        h,
        "command.run",
        json!({"cmd": "potential.define", "params": {"at": junction(j1), "name": "24VDC"}}),
    );
    settle(h);
}

/// FT-1097 (play-test rv-vis-7, t0084): EW-POT-011's message is cut at the panel's edge. The finding's list
/// row keeps one line (a cut row has an ellipsis and its whole text in its tooltip). The detail block shows the rule's title and the message whole, wrapped to the pane's width
/// and between the list and the footer, at the floor and at 1920 × 1080.
#[test]
fn the_pot_011_detail_shows_its_title_and_message_whole_at_the_floor_and_at_1920() {
    const TITLE: &str = "EW-POT-011 Junction feed ambiguous";
    // FT-1146 (item 10): the potential once, first; the row shows the message alone (it leads with its
    // subject).
    const MESSAGE: &str = "24VDC has a junction or a group of interruption points whose feed is unclear; draw one \
                           wire into it from the supply side";
    for (w, hgt) in [(1280.0, 800.0), (1920.0, 1080.0)] {
        let tmp = Tmp::new();
        let mut h = on_golden(&tmp, Scripted::default());
        h.set_size(egui::Vec2::new(w, hgt));
        settle(&mut h);
        ambiguous_junction_feed(&mut h);
        dispatch(&mut h, "messages.show", json!({"on": true}));
        dispatch(&mut h, "messages.check", json!({}));
        settle(&mut h);
        let row = table_nodes(&h)
            .into_iter()
            .find(|n| n.role == NavRole::Finding && n.cells[1] == "EW-POT-011")
            .expect("an EW-POT-011 finding on the joined junctions");
        assert_eq!(row.label, MESSAGE, "{w}×{hgt}: the row names 24VDC once");
        dispatch(
            &mut h,
            "navigator.row",
            json!({"kind": "messages", "key": row.key}),
        );
        settle(&mut h);
        settle(&mut h);
        settle(&mut h);
        let detail = h.state().shell().messages_detail().expect("the detail");
        assert_eq!(
            (
                format!("{} {}", detail.rule, detail.title),
                detail.message.as_str()
            ),
            (TITLE.to_owned(), MESSAGE),
            "{w}×{hgt}"
        );
        let list = h.get_by_label("Messages navigator").rect();
        let footer = h
            .get_by_label("Checks support, but do not replace, engineering verification.")
            .rect();
        let painted = painted_texts(&h);
        // The detail block: the title and the message, each painted whole (no ellipsis), inside the pane, between the
        // list and the footer.
        for text in [TITLE, MESSAGE] {
            let shown: Vec<_> = painted
                .iter()
                .filter(|(t, r, _)| t == text && r.top() >= list.bottom() - 0.5)
                .collect();
            let [(_, rect, elided)] = shown[..] else {
                panic!("{w}×{hgt}: {text:?} painted once below the list: {shown:?}")
            };
            assert!(!elided, "{w}×{hgt}: {text:?} is not cut");
            assert!(
                rect.right() <= list.right() + 0.5 && rect.bottom() <= footer.top() + 0.5,
                "{w}×{hgt}: {text:?} {rect:?} inside the pane {list:?} above the footer {footer:?}"
            );
        }
        // The list row keeps one line. FT-1146 (item 10): with the potential named once it fits at the floor
        // too (it was cut there, its whole text in its tooltip).
        let cut = painted
            .iter()
            .filter(|(t, r, _)| t == &row.label && r.bottom() <= list.bottom() + 0.5)
            .map(|(_, _, elided)| *elided)
            .collect::<Vec<_>>();
        assert_eq!(
            cut,
            [false],
            "{w}×{hgt}: the row {:?} is painted once, whole",
            row.label
        );
        h.state_mut().shell_mut().shutdown();
    }
}

/// The quick-fix card's strip in the Messages pane counts its records, tags and cross-reference
/// texts in words, in en-GB and in nl-NL (it read "1, 0, 0").
#[test]
fn the_quick_fix_card_counts_in_words_in_english_and_dutch() {
    use ew_ui::ew_viewmodel::{key, strings};
    for (lang, line) in [
        (
            "en",
            "Preview open: Quick fix EW-CORE-023 · 1 change, 0 tags, 0 cross-reference texts",
        ),
        (
            "nl",
            "Voorbeeld open: Snelle oplossing EW-CORE-023 · 1 wijziging, 0 kenmerken, 0 kruisverwijzingsteksten",
        ),
    ] {
        let tmp = Tmp::new();
        let mut h = on_golden(&tmp, Scripted::default());
        dispatch(&mut h, "view.language", json!({ "value": lang }));
        dispatch(&mut h, "page.goto", json!({"folio": "102"}));
        let page = h
            .state()
            .shell()
            .page_rows()
            .iter()
            .find(|r| r.folio == "102")
            .map(|r| r.id.to_string())
            .expect("folio 102");
        dispatch(
            &mut h,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [page],
                "path": "description_override", "value": "Overridden"}}),
        );
        dispatch(&mut h, "messages.show", json!({"on": true}));
        settle(&mut h);
        let fix = table_nodes(&h)
            .into_iter()
            .find(|n| {
                n.role == NavRole::Finding
                    && n.cells[1] == "EW-CORE-023"
                    && n.badges.iter().any(|b| b == "fix")
            })
            .expect("EW-CORE-023's finding with a fix");
        dispatch(&mut h, "messages.fix", json!({"key": fix.key}));
        settle(&mut h);
        h.get_by_role_and_label(egui::Role::Button, strings::text(key!("nav-title-messages")))
            .click();
        settle(&mut h);
        settle(&mut h);
        let shown = h
            .root()
            .children_recursive()
            .any(|n| n.accesskit_node().value().as_deref() == Some(line));
        assert!(shown, "{lang}: the strip reads {line:?}");
        h.state_mut().shell_mut().shutdown();
    }
}

/// In nl-NL a finding row with a quick fix still paints its Fix status badge (`badge-fix`, "Oplossen"):
/// the window paints the badge from its code, the row's text carries the Dutch word.
#[test]
fn a_finding_keeps_its_fix_badge_in_dutch() {
    use ew_ui::ew_viewmodel::{key, strings};
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, Scripted::default());
    dispatch(&mut h, "view.language", json!({ "value": "nl" }));
    dispatch(&mut h, "messages.show", json!({"on": true}));
    settle(&mut h);
    let fixable = table_nodes(&h)
        .into_iter()
        .find(|n| n.role == NavRole::Finding && n.badges.iter().any(|b| b == "fix"))
        .expect("a finding with a quick fix");
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "key": fixable.key}),
    );
    settle(&mut h);
    let row = h
        .state()
        .shell()
        .shown_rows(NavKind::Messages)
        .into_iter()
        .find(|r| r.key == fixable.key)
        .expect("the row on show");
    assert!(
        row.badge_codes.iter().any(|c| c == "fix"),
        "{:?}",
        row.badge_codes
    );
    assert!(row.text.ends_with(" [oplossing]"), "{}", row.text);
    assert_eq!(strings::text(key!("badge-fix")), "Oplossen");
    assert!(
        h.query_all_by_label("Oplossen").count() >= 1,
        "the Fix status badge is painted"
    );
}

/// FT-1186 (doc 16a §6.24): a group or rule row's count stands `space.2` after its name, at 1920 as at the floor, not
/// at the row's far end.
#[test]
fn a_group_rows_count_follows_its_name() {
    for size in [egui::Vec2::new(1920.0, 1080.0), egui::Vec2::new(1280.0, 800.0)] {
        let tmp = Tmp::new();
        let mut h = on_golden(&tmp, Scripted::default());
        h.set_size(size);
        dispatch(&mut h, "navigator.show", json!({"kind": "messages"}));
        settle(&mut h);
        settle(&mut h);
        let t = ew_ui::theme::current(&h.ctx);
        let row = h
            .state()
            .shell()
            .shown_rows(NavKind::Messages)
            .into_iter()
            .find(|r| r.finding.is_none() && r.badges.iter().any(|b| b.parse::<usize>().is_ok()))
            .expect("a group row with a count");
        let count = row
            .badges
            .iter()
            .find(|b| b.parse::<usize>().is_ok())
            .unwrap()
            .clone();
        let texts = painted_texts(&h);
        let (_, name, _) = texts
            .iter()
            .find(|(s, _, _)| *s == row.label)
            .unwrap_or_else(|| panic!("{} painted", row.label));
        let (_, badge, _) = texts
            .iter()
            .filter(|(s, r, _)| *s == count && (r.center().y - name.center().y).abs() < 4.0)
            .min_by(|a, b| a.1.left().total_cmp(&b.1.left()))
            .expect("its count painted on its row");
        // The badge is `size.badge` wide at least, its number centred in it; the name's button keeps its padding.
        let inner = ((badge.width() + 2.0 * t.space_icon).max(t.size_badge) - badge.width()) / 2.0;
        let gap = badge.left() - name.right();
        assert!(
            (gap - (t.space_2 + inner)).abs() < 1.0,
            "{size:?}: {} then {count}, {gap} points apart",
            row.label
        );
        h.state_mut().shell_mut().shutdown();
    }
}
