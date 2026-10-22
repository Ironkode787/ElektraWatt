// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool framework in the window without rendering (WP-S2.03 part 2, acceptance checks T6, T8, T10 and T11): the
//! keyboard-only Select-and-Measure flow with the accessible-name walker after every step, the pointer's press and
//! release as the tool's input, the dwell timer and the link preview popup, and the exact counts of the window host.

mod support;

use std::time::Duration;

use egui::{Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::ew_viewmodel::tools::measure_label;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": folio }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// The text of the focused canvas's live region.
fn live(h: &Harness<'_, App>) -> String {
    h.query_all_by_label("Canvas cursor")
        .filter_map(|n| n.value())
        .find(|v| !v.is_empty())
        .unwrap_or_default()
}

fn press(h: &mut Harness<'_, App>, key: Key) {
    h.key_press(key);
    settle(h);
}

#[test]
fn t6_t10_select_and_measure_from_the_keyboard_alone() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    // The Tools menu's Measure through the palette, keyboard only.
    h.key_press_modifiers(Modifiers::COMMAND, Key::K);
    settle(&mut h);
    h.get_all_by_role(egui::Role::TextInput)
        .next()
        .expect("the palette's field")
        .type_text("Measure");
    settle(&mut h);
    press(&mut h, Key::Enter);
    assert_eq!(h.state().shell().tool().name(), "measure");
    assert!(live(&h).starts_with("Measure: "), "{}", live(&h));
    // `]` puts the cursor on a connection point; the snap marker sits there.
    press(&mut h, Key::CloseBracket);
    let a = h.state().shell().canvas().unwrap().cursor().expect("a cursor");
    let snap = h.state().shell().snap_indicator().expect("a snap marker");
    assert_eq!(snap.at_nm, a);
    assert!(snap.label.is_some());
    assert!(live(&h).contains(", connection point "), "{}", live(&h));
    // Enter fixes A; `]` twice and Enter fix B.
    press(&mut h, Key::Enter);
    assert_eq!(h.state().shell().tool().state(), "measuring");
    press(&mut h, Key::CloseBracket);
    press(&mut h, Key::CloseBracket);
    let b = h.state().shell().canvas().unwrap().cursor().unwrap();
    press(&mut h, Key::Enter);
    let labels: Vec<String> = h
        .state()
        .shell()
        .tool_overlay()
        .labels()
        .map(str::to_owned)
        .collect();
    assert_eq!(labels, vec![measure_label(a, b)]);
    assert_eq!(live(&h), "Measure: Click or Enter at the first point");
    // `[` walks back; Backspace steps the tool back; Escape returns to Select.
    press(&mut h, Key::OpenBracket);
    press(&mut h, Key::Enter);
    assert_eq!(h.state().shell().tool().state(), "measuring");
    press(&mut h, Key::Backspace);
    assert_eq!(h.state().shell().tool().state(), "armed");
    press(&mut h, Key::Escape);
    assert_eq!(h.state().shell().tool().name(), "select");
    // N goes to a free connection point; Enter there is a click at the cursor (a selection or a follow).
    press(&mut h, Key::N);
    let cursor = h.state().shell().canvas().unwrap().cursor().unwrap();
    let cp = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .cps
        .iter()
        .find(|c| c.at_nm == cursor)
        .cloned()
        .expect("the cursor on a CP");
    assert!(!cp.connected);
    let folio = h.state().shell().current_folio().unwrap().to_owned();
    press(&mut h, Key::Enter);
    let shell = h.state().shell();
    assert!(
        !shell.selection().is_empty() || shell.current_folio() != Some(folio.as_str()),
        "Enter clicked at the cursor"
    );
    if !shell.selection().is_empty() {
        assert!(live(&h).ends_with(" selected"), "{}", live(&h));
        press(&mut h, Key::Escape);
        assert!(h.state().shell().selection().is_empty());
    }
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn t10_the_primary_button_is_the_tools_input() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    let canvas = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node");
    let bounds = canvas.accesskit_node().raw_bounds().expect("bounds");
    let (x0, y0, x1, y1) = (
        bounds.x0 as f32,
        bounds.y0 as f32,
        bounds.x1 as f32,
        bounds.y1 as f32,
    );
    // A drag from the canvas's upper left quarter to its centre: a rubber band, then a selection.
    let from = Pos2::new(x0 + (x1 - x0) * 0.2, y0 + (y1 - y0) * 0.2);
    let to = Pos2::new(x0 + (x1 - x0) * 0.6, y0 + (y1 - y0) * 0.6);
    h.event(egui::Event::PointerMoved(from));
    h.run_ok();
    h.event(egui::Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    h.event(egui::Event::PointerMoved(to));
    h.run_ok();
    h.run_ok();
    let state = h.state().shell().tool().state();
    assert!(state == "rubber-band" || state == "pressed", "{state}");
    if state == "rubber-band" {
        assert_eq!(h.state().shell().tool_overlay().len(), 1, "the band");
    }
    h.event(egui::Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    h.run_ok();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().state(), "idle");
    assert!(h.state().shell().tool_overlay().is_empty());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn t8_the_window_reports_the_rest_and_shows_the_popup() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let link = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .find(|l| {
            l.target
                .as_ref()
                .is_some_and(|t| t.folio != "101" && t.at_nm.is_some())
        })
        .unwrap()
        .clone();
    let p = ew_ui::ew_viewmodel::canvas::rect_centre(link.rect);
    let calls = h.state().shell().session_calls();
    // The window's pointer over the link: its screen point from the canvas's view.
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    let pos = Pos2::new(
        bounds.x0 as f32 + ((p.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((p.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    );
    h.event(egui::Event::PointerMoved(pos));
    let started = ew_render2d::stats::now();
    while h.state().shell().link_preview().is_none() && started.elapsed() < Duration::from_secs(5) {
        h.run_ok();
        std::thread::sleep(Duration::from_millis(25));
    }
    let target = link.target.clone().unwrap();
    let lp = h
        .state()
        .shell()
        .link_preview()
        .expect("the rest showed the link preview");
    assert_eq!(lp.target, target);
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "not before 300 ms of rest"
    );
    let after = h.state().shell().session_calls();
    assert!(
        after.page_content <= calls.page_content + 1,
        "the target page once"
    );
    // The popup draws from the tile pool: its scene joins the host's while it shows.
    for _ in 0..5 {
        h.run_ok();
    }
    assert!(h.state().canvas_scene_count() >= 2);
    // A pointer move closes it.
    h.event(egui::Event::PointerMoved(pos + egui::Vec2::new(40.0, 0.0)));
    h.run_ok();
    h.run_ok();
    assert!(h.state().shell().link_preview().is_none());
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn t11_exact_counts_of_tool_events_in_the_window() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    dispatch(&mut h, "tab.new", json!({}));
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "tab.next", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let calls = h.state().shell().session_calls();
    assert!(
        calls.page_content > 0 && calls.page_cps > 0 && calls.page_cells > 0,
        "{calls:?}"
    );
    let memo = h.state().shell().memo_totals().unwrap();
    let counters = h.state().canvas_counters();
    assert!(counters.tiles_requested > 0 && counters.scenes > 0);
    let at = |x: i64, y: i64| json!({"x_mm": x, "y_mm": y});
    for i in 0..150i64 {
        let (action, params) = match i % 15 {
            0 => ("canvas.press", at(60 + i % 7, 80)),
            1 => ("canvas.pointer", at(90 + i % 11, 95)),
            2 => ("canvas.release", at(90 + i % 11, 95)),
            3 => ("cp.next", json!({})),
            4 => ("canvas.activate", json!({})),
            5 => ("tool.measure", json!({})),
            6 => ("cp.next_unconnected", json!({})),
            7 => ("canvas.activate", json!({})),
            8 => ("canvas.pointer", at(120, 100 + i % 5)),
            9 => ("canvas.dwell", json!({"ms": 80})),
            10 => ("tool.back", json!({})),
            11 => ("ui.dismiss", json!({})),
            12 => ("tab.next", json!({})),
            13 => ("cp.previous", json!({})),
            _ => ("tool.select", json!({})),
        };
        let out = h.state_mut().shell_mut().dispatch(action, &params);
        assert!(
            matches!(out, Outcome::Done | Outcome::Refused { .. }),
            "{action}: {out:?}"
        );
        if i % 10 == 0 {
            h.run_ok();
        }
    }
    settle(&mut h);
    let after = h.state().shell().session_calls();
    assert_eq!(after.page_content, calls.page_content, "{after:?}");
    assert_eq!(after.page_cells, calls.page_cells);
    assert_eq!(after.page_cps, calls.page_cps);
    assert_eq!(after.link_targets, calls.link_targets);
    assert_eq!(after.preview_with, 0);
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
