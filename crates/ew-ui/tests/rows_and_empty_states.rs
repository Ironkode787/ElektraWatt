// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 parts 3 and 4, lane K32: the Messages and History rows in the design language (DK10, DK11) and the empty
//! states of Messages, Find, History and the navigators (CP5), in the window without rendering.

mod support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, harness_with, settle, wait_for_page};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The names of the nodes of `role` (a `Label` node carries its text as its value).
fn labels(h: &Harness<'_, App>, role: egui::Role) -> Vec<String> {
    h.get_all_by_role(role)
        .filter_map(|n| {
            let n = n.accesskit_node();
            n.label().or_else(|| n.value())
        })
        .collect()
}

/// Whether a `Group` node is named `name` (an empty state).
fn group_named(h: &Harness<'_, App>, name: &str) -> bool {
    labels(h, egui::Role::Group).iter().any(|l| l == name)
}

/// A copy of Golden A in the window at 1280 × 800, on folio 101.
fn on_golden(tmp: &Tmp, read_only: bool) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(
        shell.dispatch(
            "file.open",
            &json!({"dir": dir.to_string_lossy(), "read_only": read_only}),
        ),
        Outcome::Done
    );
    let mut h = harness_with(shell, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    settle(&mut h);
    h
}

/// Places an unpaired interruption point named `name` on the shown page: an open completeness finding there, pending
/// while that page is the editing page.
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
    dispatch(h, "canvas.activate", json!({}));
    dispatch(h, "ui.dismiss", json!({}));
    settle(h);
}

/// The key of the first shown Messages row of `role` whose badges hold `badge` (any badge when `None`).
fn row_key(h: &Harness<'_, App>, role: NavRole, badge: Option<&str>) -> Option<String> {
    let shell = h.state().shell();
    let table = shell.nav(NavKind::Messages)?.table()?;
    table
        .nodes
        .iter()
        .find(|n| n.role == role && badge.is_none_or(|b| n.badges.iter().any(|x| x == b)))
        .map(|n| n.key.clone())
}

/// DK10 (doc 16a §6.24): on Golden A with an error, a warning and a pending completeness finding, every finding row on
/// show is named with its severity word; the pending row carries a "Pending" status badge node; the header is one
/// node named by the as-built sentence; the walker passes.
#[test]
fn dk10_messages_rows_badges_and_header() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, false);
    dispatch(&mut h, "messages.show", json!({"on": true}));
    settle(&mut h);
    place_ip(&mut h, "K32PEND");
    settle(&mut h);
    let shell = h.state().shell();
    let counts = shell.messages_counts().unwrap();
    // Golden A's online scheme reports one error and infos, no warning: the warning's icon and colour are proved by the
    // unit tests of `messages.rs`.
    assert!(counts.errors > 0 && counts.infos > 0, "{counts:?}");
    assert!(counts.pending > 0, "{counts:?}");
    let header = format!(
        "drafting · E {} · W {} · I {} · waived {} · pending {}",
        counts.errors, counts.warnings, counts.infos, counts.waived, counts.pending
    );
    let node = h.get_by_label(&header);
    assert_eq!(node.accesskit_node().role(), egui::Role::Label, "the header");
    assert_eq!(node.accesskit_node().value().as_deref(), Some(header.as_str()));
    // The pending row: moved into view, its name ends with ", pending" and it shows the "Pending" badge node.
    let pending = h
        .state()
        .shell()
        .shown_rows(NavKind::Messages)
        .into_iter()
        .find(|r| r.finding.is_some_and(|f| f.pending))
        .expect("a pending row");
    dispatch(
        &mut h,
        "navigator.row",
        json!({"kind": "messages", "key": pending.key}),
    );
    settle(&mut h);
    settle(&mut h);
    let items = labels(&h, egui::Role::TreeItem);
    assert!(
        items
            .iter()
            .any(|l| l == &pending.name && l.ends_with(", pending")),
        "{items:?}"
    );
    // The cursor row is scrolled into view above the detail block that opened with it (W31-1, fixed by K32 for a list
    // that was taller the frame before).
    let list = h.get_by_label("Messages navigator").rect();
    let row = h
        .get_all_by_role(egui::Role::TreeItem)
        .find(|n| n.accesskit_node().label().as_deref() == Some(pending.name.as_str()))
        .unwrap()
        .rect();
    assert!(list.expand(0.5).contains_rect(row), "{row:?} inside {list:?}");
    assert!(
        labels(&h, egui::Role::Label).iter().any(|l| l == "Pending"),
        "the Pending badge node: {:?}",
        h.query_all_by_label_contains("Pend")
            .map(|n| (n.accesskit_node().role(), n.accesskit_node().label()))
            .collect::<Vec<_>>()
    );
    // Every finding row on show names its severity word; no row paints the word "error" as a bracketed badge.
    let findings: Vec<&String> = items.iter().filter(|l| l.contains(", finding, ")).collect();
    assert!(!findings.is_empty());
    for name in &findings {
        let severity = name
            .rsplit(", finding, ")
            .next()
            .unwrap()
            .split(", ")
            .next()
            .unwrap();
        assert!(["error", "warning", "info"].contains(&severity), "{name}");
    }
    assert!(
        !labels(&h, egui::Role::Label)
            .iter()
            .any(|l| l == "error" || l == "warning"),
        "the severity word is the severity cell, not a badge chip"
    );
    support::assert_named(&h);
    h.state_mut().shell_mut().shutdown();
}

/// DK10: Fix… is disabled on a read-only project (doc 16a §6.24, §6.1). The finding with a quick fix is EW-CORE-023
/// of a page description override (the MS3 script's), saved, then reopened read-only.
#[test]
fn dk10_fix_is_disabled_on_a_read_only_project() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    for (action, params) in [
        ("file.open", json!({"dir": dir.to_string_lossy()})),
        (
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "page", "targets": [{"ref": "folio:102"}],
                "path": "description_override", "value": "Overridden"}}),
        ),
        ("file.save", json!({})),
    ] {
        assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
    }
    shell.shutdown();
    drop(shell);
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(
        shell.dispatch(
            "file.open",
            &json!({"dir": dir.to_string_lossy(), "read_only": true})
        ),
        Outcome::Done
    );
    let mut h = harness_with(shell, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "messages.show", json!({"on": true}));
    settle(&mut h);
    assert!(
        row_key(&h, NavRole::Finding, Some("fix")).is_some(),
        "a finding with a quick fix"
    );
    // As the MS3 script: the filter to the rule, then the next finding.
    dispatch(
        &mut h,
        "navigator.filter",
        json!({"kind": "messages", "text": "EW-CORE-023"}),
    );
    dispatch(&mut h, "messages.next", json!({}));
    settle(&mut h);
    settle(&mut h);
    let fix = h.get_by_role_and_label(egui::Role::Button, "Fix…");
    assert!(fix.accesskit_node().is_disabled(), "Fix… disabled");
    // The row's "Fix" badge is a named node.
    assert!(
        labels(&h, egui::Role::Label).iter().any(|l| l == "Fix"),
        "{:?}",
        labels(&h, egui::Role::TreeItem)
    );
    h.state_mut().shell_mut().shutdown();
}

/// DK11 (doc 16a §6.24): after three commits, a save and one undo, the undone entry's name ends ", undone"; the row
/// actions are named buttons (Redo to here on the undone entry, Undo to here on the others, Show changes); the marker
/// line is a separator row named by its text; Show changes opens "Changes of step n" and Escape closes it.
#[test]
fn dk11_history_rows_actions_marker_and_show_changes() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    for text in ["one", "two", "three"] {
        dispatch(
            &mut h,
            "command.run",
            json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
                "path": "function_text", "value": text}}),
        );
    }
    dispatch(&mut h, "file.save", json!({}));
    dispatch(&mut h, "edit.undo", json!({}));
    dispatch(&mut h, "history.show", json!({}));
    settle(&mut h);
    settle(&mut h);
    let groups = labels(&h, egui::Role::Group);
    let undone: Vec<&String> = groups.iter().filter(|l| l.ends_with(", undone")).collect();
    assert_eq!(undone.len(), 1, "{groups:?}");
    let buttons = labels(&h, egui::Role::Button);
    let count = |name: &str| buttons.iter().filter(|l| *l == name).count();
    assert_eq!(count("Redo to here"), 1, "{buttons:?}");
    assert_eq!(count("Undo to here"), 2, "{buttons:?}");
    assert_eq!(count("Show changes"), 3, "{buttons:?}");
    assert!(
        labels(&h, egui::Role::Splitter).iter().any(|l| l == "Saved"),
        "the marker line is a separator row"
    );
    // Show changes of the newest entry (the undone one, first row).
    h.get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Show changes"))
        .unwrap()
        .click();
    settle(&mut h);
    assert!(h.query_by_label("Changes of step 3").is_some());
    assert!(
        h.get_by_role_and_label(egui::Role::Button, "Close")
            .accesskit_node()
            .role()
            == egui::Role::Button
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    assert!(
        h.state().shell().history_pane().changes().is_none(),
        "Escape closes"
    );
    assert!(h.query_by_label("Changes of step 3").is_none());
    support::assert_named(&h);
    h.state_mut().shell_mut().shutdown();
}

/// CP5 (doc 16a §6.17): on a new project, History, Messages, Find and a navigator without rows show their empty
/// states as named groups; a filter without hits says so with Clear filter, which clears it.
#[test]
fn cp5_the_empty_states_of_messages_find_history_and_the_navigators() {
    let tmp = Tmp::new();
    let dir = tmp.0.join("blank.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(
        shell.dispatch(
            "file.new",
            &json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
        ),
        Outcome::Done
    );
    let mut h = harness_with(shell, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    // History before any edit.
    dispatch(&mut h, "history.show", json!({}));
    settle(&mut h);
    assert!(group_named(&h, "Nothing to undo yet"), "History's empty state");
    dispatch(&mut h, "history.show", json!({"on": false}));
    // A filter without hits in the Pages navigator, cleared by its button.
    dispatch(&mut h, "pages.filter", json!({"text": "k32nomatch"}));
    settle(&mut h);
    settle(&mut h);
    assert!(
        group_named(&h, "Nothing matches \"k32nomatch\""),
        "{:?} {:?}",
        labels(&h, egui::Role::Group),
        h.state().shell().shown_rows(NavKind::Pages)
    );
    let clear = h
        .get_all_by_role(egui::Role::Button)
        .filter(|n| n.accesskit_node().label().as_deref() == Some("Clear filter"))
        .count();
    assert!(clear >= 1, "Clear filter");
    h.get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Clear filter"))
        .unwrap()
        .click();
    settle(&mut h);
    settle(&mut h);
    assert!(h.state().shell().navigator().filter.is_empty(), "cleared");
    // A navigator without rows: the Devices navigator of a new project.
    dispatch(&mut h, "navigator.show", json!({"kind": "devices"}));
    settle(&mut h);
    settle(&mut h);
    assert!(
        group_named(&h, "No devices yet"),
        "{:?}",
        labels(&h, egui::Role::Group)
    );
    // Messages without findings.
    dispatch(&mut h, "messages.show", json!({"on": true}));
    settle(&mut h);
    settle(&mut h);
    assert_eq!(
        h.state().shell().messages_counts(),
        Some(Default::default()),
        "a new project has no findings"
    );
    assert!(group_named(&h, "No findings"), "Messages' empty state");
    // Find before a search and after one without hits.
    dispatch(&mut h, "find.open", json!({}));
    settle(&mut h);
    assert!(group_named(&h, "Find in the project"), "Find's start state");
    dispatch(&mut h, "find.text", json!({"text": "zzqqk32"}));
    dispatch(&mut h, "find.run", json!({}));
    settle(&mut h);
    assert!(
        group_named(&h, "Nothing matches \"zzqqk32\""),
        "Find's empty result"
    );
    support::assert_named(&h);
    h.state_mut().shell_mut().shutdown();
}

/// DK6 (FT-348): a window's geometry recorded in the shell is written at the end of the window and read by the next
/// start; a run that recorded none leaves no `[window]` table.
#[test]
fn dk6_the_window_geometry_survives_a_restart() {
    use ew_ui::ew_viewmodel::window::WindowGeometry;
    let tmp = Tmp::new();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(shell.settings().window, None);
    shell.shutdown();
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(shell.settings().window, None, "nothing recorded, nothing written");
    let g = WindowGeometry {
        x: 120.0,
        y: 64.0,
        width: 1440.0,
        height: 900.0,
        maximized: true,
    };
    shell.set_window(g);
    shell.shutdown();
    let shell = ShellModel::new(Mode::Gui, tmp.config());
    assert_eq!(shell.settings().window, Some(g));
    let builder = ew_ui::window_memory::restore(egui::ViewportBuilder::default(), shell.settings().window);
    assert_eq!(builder.inner_size, Some(egui::vec2(1440.0, 900.0)));
    assert_eq!(builder.position, Some(egui::pos2(120.0, 64.0)));
    // R3-1: the builder never maximises; the first frame places the window, then maximises it (`first_frame`).
    assert_eq!(builder.maximized, None);
}
