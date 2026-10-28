// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The message panel in the window without rendering (WP-S2.09 part 2, acceptance check MS5 and the window half of
//! MS1 and MS4): View ▸ Messages docks the pane at the bottom with its header line, rows and footer; the finding rows
//! are tree items named `<label>, finding, <severity>[, fix][, pending]`; F8 and Shift+F8 walk the findings from the
//! keyboard; the detail block names the cursor's rule and its Rule page link opens the rule's page in the browser; a
//! new unpaired interruption point on the editing page is a pending row; the accessible-name walker after every step.

mod support;

use egui::Modifiers;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavNode, NavRole};
use ew_ui::ew_viewmodel::shell::{Focus, Mode, Outcome, Resources, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// A copy of Golden A in the window, with an installed user guide that holds the rule pages.
fn on_golden(tmp: &Tmp, platform: Scripted) -> Harness<'static, App> {
    let docs = tmp.0.join("docs");
    std::fs::create_dir_all(docs.join("reference/rules")).unwrap();
    std::fs::write(docs.join("reference/rules/ew-core.html"), "<html></html>").unwrap();
    let config = tmp.config().with_resources(Resources {
        sample_dir: None,
        content_dir: None,
        docs_dir: Some(docs),
        notices_file: None,
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
fn ms5_the_pane_in_the_window_named_rows_keyboard_detail_and_pending() {
    let tmp = Tmp::new();
    let platform = Scripted::default();
    let opened = platform.opened.clone();
    let mut h = on_golden(&tmp, platform);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    dispatch(&mut h, "view.zoom_actual", json!({}));
    settle(&mut h);
    // View ▸ Messages docks the pane at the bottom and focuses it.
    h.get_by_label("View").click();
    settle(&mut h);
    h.get_by_label("Messages").click();
    settle(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.navigator_open(NavKind::Messages));
    assert_eq!(shell.focus(), Some(Focus::Navigator(NavKind::Messages)));
    assert!(
        h.query_by_label("Messages navigator").is_some(),
        "the Tree is named"
    );
    let counts = shell.messages_counts().unwrap();
    let header = format!(
        "drafting · E {} · W {} · I {} · waived {} · pending {}",
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
            .is_some_and(|u| u.ends_with("reference/rules/ew-core.html#ew-core-033")),
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
