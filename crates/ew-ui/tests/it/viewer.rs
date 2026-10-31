// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The read-only viewer in the window without rendering (WP-S2.03 part 1, acceptance checks V10 to V12): the
//! keyboard-only flow (Go to, links, Back, tabs, side by side, the navigator, the grid cursor and its AccessKit
//! announcement) with the accessible-name walker after every step, the exact counts of the multi-canvas host, and
//! FT-356's retention for hidden canvas tabs.

use crate::support;

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

/// The text of the grid cursor's live region (the one that is not empty: only the focused group shows a cursor here).
fn live(h: &Harness<'_, App>) -> String {
    h.query_all_by_label("Canvas cursor")
        .filter_map(|n| n.value())
        .find(|v| !v.is_empty())
        .unwrap_or_default()
}

/// Whether an ancestor of the first node with `label` has `role` (AccessKit's own tree, WP-S2.03 §6.9).
fn has_ancestor(h: &Harness<'_, App>, label: &str, role: egui::Role) -> bool {
    let mut node = h.query_all_by_label(label).next().expect("the node").parent();
    while let Some(n) = node {
        if n.accesskit_node().role() == role {
            return true;
        }
        node = n.parent();
    }
    false
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
    let text = live(&h);
    assert!(text.starts_with("103-A1, x "), "{text}");
    // The live region is a child of the canvas node; the navigator's rows are items of its tree.
    assert!(has_ancestor(&h, "Canvas cursor", egui::Role::Canvas));
    let item = h
        .get_all_by_role(egui::Role::TreeItem)
        .next()
        .expect("a tree item");
    let mut up = item.parent();
    let mut in_tree = false;
    while let Some(n) = up {
        in_tree |= n.accesskit_node().role() == egui::Role::Tree;
        up = n.parent();
    }
    assert!(in_tree, "a navigator row is an item of the Tree node");
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
    let text = live(&h);
    assert!(text.contains(", link to "), "{text}");
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
    assert_eq!(support::view_tabs(&h).len(), 2);
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
    support::open_menu(&mut h, &["View", "Layout"]);
    settle(&mut h);
    support::menu_node(&h, "Side by side").click();
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let a = h.state().shell().area().unwrap();
    assert_eq!((a.groups.len(), a.focused), (2, 1));
    assert_eq!(h.get_all_by_role(egui::Role::Canvas).count(), 2);
    // The dock's two splitters (pages | page view | properties, WP-S2.05) and the canvas area's.
    assert_eq!(
        h.state().splitter_count(),
        3,
        "the dock's splitter and the canvas area's"
    );
    h.key_press_modifiers(Modifiers::COMMAND, Key::F6);
    settle(&mut h);
    assert_eq!(h.state().shell().area().unwrap().focused, 0);
    // F6 to the navigator, Down, Enter: the next row opens.
    while h.state().shell().focus() != Some(Focus::Navigator(ew_ui::ew_viewmodel::navigators::NavKind::Pages))
    {
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
    let before_live = live(&h);
    h.key_press(Key::ArrowRight);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    settle(&mut h);
    let after = canvas_description(&h);
    assert_ne!(before, after);
    assert_ne!(before_live, live(&h));
    assert!(after.contains(", cursor "), "{after}");
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn v10_exact_counts_of_the_multi_canvas_host() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    // Every page a cross-reference of section 1 leads to is shown once, so a follow stays in the cache.
    for folio in ["101", "106", "102"] {
        dispatch(&mut h, "page.goto", json!({ "folio": folio }));
    }
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
    // Checklist 9: the counters are live, so their equality below cannot pass vacuously.
    assert!(
        calls.page_content > 0 && calls.page_cells > 0 && calls.link_targets > 0 && calls.page_rows > 0,
        "{calls:?}"
    );
    let memo = h.state().shell().memo_totals().unwrap();
    let counters = h.state().canvas_counters();
    assert!(
        counters.tiles_requested > 0 && counters.scenes > 0,
        "the counters are live: {counters:?}"
    );
    // 200 mixed steps between pages in the page-content cache at one revision.
    let mut done = std::collections::BTreeMap::<&str, u32>::new();
    for i in 0..200 {
        if i % 20 == 0 {
            // A jump and its history: the cursor on a link, follow it, back, and every other time forward.
            let mut steps = vec!["nav.next_link", "nav.follow", "nav.back"];
            if i % 40 == 0 {
                steps.push("nav.forward");
            }
            for action in steps {
                let out = h.state_mut().shell_mut().dispatch(action, &json!({}));
                assert!(
                    matches!(out, Outcome::Done | Outcome::Refused { .. }),
                    "{action}: {out:?}"
                );
                if out == Outcome::Done {
                    *done.entry(action).or_default() += 1;
                }
            }
        }
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
    for action in ["nav.next_link", "nav.follow", "nav.back", "nav.forward"] {
        assert!(
            done.get(action).copied().unwrap_or(0) >= 1,
            "{action} ran: {done:?}"
        );
    }
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
    // Back to the hidden tab of 102: its tiles are still cached, so the very first frame draws the page complete,
    // with no tile to wait for. (Only what the test controls is counted: the cache's content, not the pool's
    // deliveries, whose timing varies.) Both pages are fitted A4 sheets in one pane, so the view does not change and
    // the plan is settled at once.
    let fitted_103 = h.state().shell().canvas().unwrap().view_nm();
    dispatch(&mut h, "tab.previous", json!({}));
    assert_eq!(folio(&h), "102");
    assert_eq!(h.state().shell().canvas().unwrap().view_nm(), fitted_103);
    h.step();
    assert!(
        h.state().page_view_ready(),
        "every tile of the hidden tab's page was kept"
    );
    // Closing the last tab of 103 drops its scene.
    dispatch(&mut h, "tab.next", json!({}));
    dispatch(&mut h, "tab.close", json!({}));
    settle(&mut h);
    assert_eq!(h.state().canvas_scene_count(), 1);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_splitter_share_survives_a_tab_click_and_ctrl_f6() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(&mut h, "tab.new", json!({}));
    dispatch(&mut h, "view.side_by_side", json!({}));
    settle(&mut h);
    let equal = h.state().canvas_area_shares().expect("two groups");
    assert!((equal[0] - equal[1]).abs() < 1e-3, "{equal:?}");
    // Drag the splitter between the two canvases 120 points to the right.
    let rects: Vec<egui::Rect> = h.get_all_by_role(egui::Role::Canvas).map(|n| n.rect()).collect();
    let at = egui::pos2((rects[0].right() + rects[1].left()) / 2.0, rects[0].center().y);
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    for step in 1..=6 {
        h.input_mut().events.push(egui::Event::PointerMoved(
            at + egui::vec2(20.0 * step as f32, 0.0),
        ));
        h.run_ok();
    }
    h.input_mut().events.push(egui::Event::PointerButton {
        pos: at + egui::vec2(120.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    settle(&mut h);
    let dragged = h.state().canvas_area_shares().unwrap();
    assert!(
        dragged[0] > dragged[1] + 0.05,
        "the drag moved the splitter: {dragged:?}"
    );
    // A tab click and Ctrl+F6 change the model's version; the tree is rebuilt and the share stays.
    let tab = support::view_tabs(&h)
        .into_iter()
        .find(|n| !n.accesskit_node().is_selected().unwrap_or(false))
        .expect("an inactive tab");
    tab.click();
    settle(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::F6);
    settle(&mut h);
    let kept = h.state().canvas_area_shares().unwrap();
    assert!(
        (kept[0] - dragged[0]).abs() < 1e-4 && (kept[1] - dragged[1]).abs() < 1e-4,
        "{kept:?} {dragged:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The navigator's rows are built once per (revision, navigator state), not every frame: frames without a change
/// build none, a collapsed node builds them again once, and the next frames again none.
#[test]
fn the_navigator_rows_are_built_once_per_revision_and_navigator_state() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    let built = h.state().navigator_rebuilds();
    assert!(built >= 1, "the first frame builds the rows");
    for _ in 0..5 {
        h.run_ok();
    }
    assert_eq!(
        h.state().navigator_rebuilds(),
        built,
        "frames without a change rebuild nothing"
    );
    let node = h
        .state()
        .shell()
        .navigator_rows()
        .into_iter()
        .find(|r| matches!(r.kind, RowKind::Node { .. }))
        .expect("the golden navigator has a node");
    dispatch(&mut h, "pages.toggle", json!({"key": node.key}));
    settle(&mut h);
    assert_eq!(
        h.state().navigator_rebuilds(),
        built + 1,
        "a collapsed node builds the rows once"
    );
    assert!(
        !h.state()
            .shell()
            .navigator_rows()
            .iter()
            .any(|r| r.key == node.key && r.expanded)
    );
    for _ in 0..5 {
        h.run_ok();
    }
    assert_eq!(h.state().navigator_rebuilds(), built + 1);
    h.state_mut().shell_mut().shutdown();
}

/// J32 (doc 16a §6.6, WP-S2.17 part 3's DK2 for the canvas tabs): no canvas tab's label carries a bracketed word; the
/// badges are the page rows' caption chips painted inside the tab, and the accessible name reads them after a comma.
#[test]
fn j32_canvas_tab_badges_are_chips_not_bracketed_words() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    wait_for_page(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::T);
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "2"}));
    wait_for_page(&mut h);
    settle(&mut h);
    let tabs: Vec<(String, egui::Rect)> = support::view_tabs(&h)
        .iter()
        .map(|n| (n.accesskit_node().label().unwrap_or_default(), n.rect()))
        .filter(|(l, _)| l.starts_with("101 ") || l.starts_with("2 "))
        .collect();
    assert_eq!(tabs.len(), 2, "{tabs:?}");
    for (label, _) in &tabs {
        assert!(!label.contains('['), "{label}");
    }
    let qet = tabs.iter().find(|(l, _)| l.starts_with("101 ")).unwrap();
    assert!(qet.0.ends_with(", QET grid"), "{}", qet.0);
    let generated = tabs.iter().find(|(l, _)| l.starts_with("2 ")).unwrap();
    assert!(generated.0.ends_with(", generated"), "{}", generated.0);
    // The chips' words are painted inside their tabs.
    let mut painted = Vec::new();
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push((t.galley.text().to_owned(), t.pos)),
            _ => {}
        }
    }
    for c in &h.output().shapes {
        walk(&c.shape, &mut painted);
    }
    for ((_, rect), word) in [(qet, "QET grid"), (generated, "generated")] {
        assert!(
            painted.iter().any(|(t, p)| t == word && rect.contains(*p)),
            "{word} in {rect:?}"
        );
    }
    assert!(
        !painted.iter().any(|(t, _)| t.contains("[QET grid]")),
        "no bracketed badge is painted"
    );
    h.state_mut().shell_mut().shutdown();
}
