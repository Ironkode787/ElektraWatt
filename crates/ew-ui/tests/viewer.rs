// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The read-only viewer in the window without rendering (WP-S2.03 part 1, acceptance checks V10 to V12): the
//! keyboard-only flow (Go to, links, Back, tabs, side by side, the navigator, the grid cursor and its AccessKit
//! announcement) with the accessible-name walker after every step, the exact counts of the multi-canvas host, and
//! FT-356's retention for hidden canvas tabs.

mod support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigator::RowKind;
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

fn folio(h: &Harness<'_, App>) -> String {
    h.state().shell().current_folio().unwrap().to_owned()
}

fn status(h: &Harness<'_, App>) -> Vec<String> {
    h.state().shell().status_fields(false)
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// The description of the focused group's canvas node.
fn canvas_description(h: &Harness<'_, App>) -> String {
    let focused = h.state().shell().area().unwrap().focused;
    let nodes: Vec<_> = h.get_all_by_role(egui::Role::Canvas).collect();
    nodes[focused.min(nodes.len() - 1)]
        .accesskit_node()
        .description()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn v12_the_viewer_without_a_pointer() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    // Ctrl+G, "103-A1", Enter: folio 103, the status field reads the cell.
    h.key_press_modifiers(Modifiers::COMMAND, Key::G);
    settle(&mut h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some("Folio or cell"))
        .expect("the go-to field")
        .type_text("103-A1");
    settle(&mut h);
    h.key_press(Key::Enter);
    settle(&mut h);
    assert_eq!(folio(&h), "103");
    assert!(h.state().shell().dialog().is_none());
    assert!(
        status(&h).iter().any(|f| f.starts_with("103-A1 · x ")),
        "{:?}",
        status(&h)
    );
    assert!(
        canvas_description(&h).contains(", cursor 103-A1 x "),
        "{}",
        canvas_description(&h)
    );
    // The cursor's live region carries the cell.
    let live = h
        .get_by_label("Canvas cursor")
        .value()
        .expect("the live region's text");
    assert!(live.starts_with("103-A1, x "), "{live}");
    // Shift+F to a link, F follows it, Alt+Left comes back.
    h.key_press_modifiers(Modifiers::SHIFT, Key::F);
    settle(&mut h);
    assert!(h.state().shell().canvas().unwrap().cursor_link().is_some());
    let target = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .cursor_link()
        .and_then(|l| l.target.clone())
        .unwrap();
    let live = h.get_by_label("Canvas cursor").value().unwrap();
    assert!(live.contains(", link to "), "{live}");
    h.key_press(Key::F);
    settle(&mut h);
    assert_eq!(folio(&h), target.folio);
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowLeft);
    settle(&mut h);
    assert_eq!(folio(&h), "103");
    // Ctrl+T, Ctrl+Tab, Ctrl+W.
    h.key_press_modifiers(Modifiers::COMMAND, Key::T);
    settle(&mut h);
    assert_eq!(h.state().shell().area().unwrap().tab_count(), 2);
    assert_eq!(h.get_all_by_role(egui::Role::Tab).count(), 2);
    assert_eq!(
        h.query_all_by_label("Close 103").count(),
        2,
        "each tab's close button names its folio"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
    settle(&mut h);
    assert_eq!(h.state().shell().area().unwrap().focused_group().active, 0);
    h.key_press_modifiers(Modifiers::COMMAND, Key::W);
    settle(&mut h);
    assert_eq!(h.state().shell().area().unwrap().tab_count(), 1);
    assert!(
        h.state().shell().has_project(),
        "Ctrl+W closes the tab, not the project"
    );
    // View, Side by side; Ctrl+F6 focuses the other group.
    h.get_by_label("View").click();
    settle(&mut h);
    h.get_by_label("Side by side").click();
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let a = h.state().shell().area().unwrap();
    assert_eq!((a.groups.len(), a.focused), (2, 1));
    assert_eq!(h.get_all_by_role(egui::Role::Canvas).count(), 2);
    assert_eq!(
        h.state().splitter_count(),
        2,
        "the dock's splitter and the canvas area's"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::F6);
    settle(&mut h);
    assert_eq!(h.state().shell().area().unwrap().focused, 0);
    // F6 to the navigator, Down, Enter: the next row opens.
    while h.state().shell().focus() != Some(Focus::Navigator) {
        h.key_press(Key::F6);
        settle(&mut h);
    }
    let rows = h.state().shell().navigator_rows();
    let at = rows
        .iter()
        .position(|r| Some(r.key.clone()) == h.state().shell().navigator_row_key())
        .unwrap();
    h.key_press(Key::ArrowDown);
    settle(&mut h);
    let next = &rows[at + 1];
    assert_eq!(
        h.state().shell().navigator_row_key().as_deref(),
        Some(next.key.as_str())
    );
    h.key_press(Key::Enter);
    settle(&mut h);
    match next.kind {
        RowKind::Page(i) => {
            assert_eq!(folio(&h), h.state().shell().page_rows()[i].folio);
        }
        RowKind::Node { .. } => {
            assert!(
                !h.state()
                    .shell()
                    .navigator_rows()
                    .iter()
                    .any(|r| r.key == next.key && r.expanded)
            );
        }
    }
    let tree_items = h.get_all_by_role(egui::Role::TreeItem).count();
    assert!(tree_items > 0);
    // F6 back to a canvas: the arrows move the grid cursor, and its description and live region follow.
    while !matches!(h.state().shell().focus(), Some(Focus::Canvas(_))) {
        h.key_press(Key::F6);
        settle(&mut h);
    }
    dispatch(&mut h, "cursor.set", json!({"x_mm": 150, "y_mm": 100}));
    settle(&mut h);
    let before = canvas_description(&h);
    let before_live = h.get_by_label("Canvas cursor").value().unwrap();
    h.key_press(Key::ArrowRight);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    settle(&mut h);
    let after = canvas_description(&h);
    assert_ne!(before, after);
    assert_ne!(before_live, h.get_by_label("Canvas cursor").value().unwrap());
    assert!(after.contains(", cursor "), "{after}");
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn v10_exact_counts_of_the_multi_canvas_host() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(&mut h, "tab.new", json!({}));
    dispatch(&mut h, "page.goto", json!({"folio": "103"}));
    dispatch(&mut h, "view.side_by_side", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    // Show every tab's page once, settled.
    dispatch(&mut h, "tab.focus_other", json!({}));
    dispatch(&mut h, "tab.next", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "tab.next", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let calls = h.state().shell().session_calls();
    let memo = h.state().shell().memo_totals().unwrap();
    let counters = h.state().canvas_counters();
    assert!(
        counters.tiles_requested > 0 && counters.scenes > 0,
        "the counters are live: {counters:?}"
    );
    // 200 mixed steps between pages in the page-content cache at one revision.
    for i in 0..200 {
        let (action, params) = match i % 10 {
            0 => ("tab.next", json!({})),
            1 => ("cursor.right", json!({})),
            2 => ("canvas.pointer", json!({"x_mm": 100 + i % 9, "y_mm": 70})),
            3 => ("nav.next_link", json!({})),
            4 => ("tab.focus_other", json!({})),
            5 => ("cursor.cell_down", json!({})),
            6 => ("canvas.pointer", json!({"group": 1, "x_mm": 80, "y_mm": 90})),
            7 => ("view.side_by_side", json!({})),
            8 => ("view.side_by_side", json!({})),
            _ => ("canvas.pointer", json!({})),
        };
        let out = h.state_mut().shell_mut().dispatch(action, &params);
        assert!(
            matches!(out, Outcome::Done | Outcome::Refused { .. }),
            "{action}: {out:?}"
        );
        if i % 25 == 0 {
            h.run_ok();
        }
    }
    settle(&mut h);
    let after = h.state().shell().session_calls();
    assert_eq!(after.link_targets, calls.link_targets, "{after:?} {calls:?}");
    assert_eq!(after.page_cells, calls.page_cells);
    assert_eq!(after.page_content, calls.page_content);
    assert_eq!(
        h.state().shell().memo_totals().unwrap().1,
        memo.1,
        "no memo re-execution"
    );
    let c = h.state().canvas_counters();
    assert_eq!(c.requested_while_cached, 0);
    assert_eq!(
        c.rastered_while_cached, 0,
        "no tile rasterised twice while cached"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn v11_hidden_canvas_tabs_keep_their_tiles_and_a_closed_page_drops_its_scene() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "tab.new", json!({}));
    dispatch(&mut h, "page.goto", json!({"folio": "103"}));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let scenes = h.state().canvas_scene_count();
    assert_eq!(scenes, 2, "two tabs, two pages, two scenes");
    assert!(h.state().canvas_hidden_bytes() <= ew_ui::canvas::HIDDEN_CACHE_BYTES);
    // Back to the hidden tab of 102: its tiles are still cached, no tile is rasterised.
    let rastered = h.state().canvas_counters().tiles_rastered;
    dispatch(&mut h, "tab.previous", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    assert_eq!(h.state().canvas_counters().tiles_rastered, rastered);
    // Closing the last tab of 103 drops its scene.
    dispatch(&mut h, "tab.next", json!({}));
    dispatch(&mut h, "tab.close", json!({}));
    settle(&mut h);
    assert_eq!(h.state().canvas_scene_count(), 1);
    h.state_mut().shell_mut().shutdown();
}
