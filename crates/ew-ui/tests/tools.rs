// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool framework in the window without rendering (WP-S2.03 part 2, acceptance checks T6, T8, T10 and T11): the
//! keyboard-only Select-and-Measure flow with the accessible-name walker after every step, the pointer's press and
//! release as the tool's input, the dwell timer and the link preview popup, and the exact counts of the window host.

mod support;

use std::time::Duration;

use egui::{Key, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2, TouchPhase, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::ew_viewmodel::tools::{Dash, Ghost, OverlayItem, Token, band_selection, measure_label, rect_of};
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

/// A harness on a folio without waiting for its tiles: the dashed overlays are model state and are painted before the
/// page image arrives.
fn quick_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": folio }));
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

/// The window position (points) of the sheet point `p` on the focused canvas, from the canvas's view.
fn screen(h: &Harness<'_, App>, p: (i64, i64)) -> Pos2 {
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    Pos2::new(
        bounds.x0 as f32 + ((p.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((p.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    )
}

/// The sheet point under the window point `pos` on the focused canvas, the inverse of [`screen`].
fn sheet_at_screen(h: &Harness<'_, App>, pos: Pos2) -> (i64, i64) {
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    let x = (f64::from(pos.x - bounds.x0 as f32) - c.pane().0 / 2.0) / k + c.centre().0 as f64;
    let y = (f64::from(pos.y - bounds.y0 as f32) - c.pane().1 / 2.0) / k + c.centre().1 as f64;
    (x.round() as i64, y.round() as i64)
}

/// A primary press and release at `pos`, both delivered before one frame (a touchpad tap, a slow frame).
fn tap(h: &mut Harness<'_, App>, pos: Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.run_ok();
    h.run_ok();
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
    // The readout reaches a screen reader: the live region says it (NFR-A11Y-01).
    assert_eq!(live(&h), measure_label(a, b));
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
    let from_nm = h.state().shell().canvas().unwrap().pointer().unwrap();
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
    assert_eq!(h.state().shell().tool().state(), "rubber-band");
    assert!(
        matches!(
            h.state().shell().tool_overlay().items.as_slice(),
            [OverlayItem::Rect { style, .. }] if style.dash == Dash::Solid
        ),
        "the enclosing band"
    );
    let to_nm = h.state().shell().canvas().unwrap().pointer().unwrap();
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
    // The selection is what the band from the press to the release encloses.
    let page = std::sync::Arc::clone(h.state().shell().canvas().unwrap().page());
    let expected = band_selection(&page.entities, rect_of(from_nm, to_nm), true);
    assert!(!expected.is_empty(), "the band encloses something");
    assert_eq!(h.state().shell().selection().items, expected);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a_press_and_its_release_in_one_frame_still_click() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    // An entity whose centre is no link and picks it, with no wire within reach (the Select tool takes a wire's line
    // drawn through a symbol's middle first, FT-500, and the tap's point is rounded to the screen).
    let (entity, p) = {
        let c = h.state().shell().canvas().unwrap();
        let slop = ew_ui::ew_viewmodel::canvas::HOVER_SLOP_PT / c.points_per_nm();
        let page = c.page();
        page.entities
            .iter()
            .map(|(e, r)| (*e, ew_ui::ew_viewmodel::canvas::rect_centre(*r)))
            .find(|(e, p)| {
                c.link_at(*p).is_none()
                    && ew_ui::ew_viewmodel::canvas::pick(&page.entities, Some(*p), slop).map(|x| x.0)
                        == Some(*e)
                    && {
                        let stack = ew_ui::ew_viewmodel::tools::pick_stack(page, &page.snapper, *p, slop);
                        stack.first() == Some(e)
                            && stack
                                .iter()
                                .all(|x| page.tables.connections.iter().all(|w| w.id != x.0))
                    }
            })
            .expect("an entity off the links")
    };
    let pos = screen(&h, p);
    tap(&mut h, pos);
    settle(&mut h);
    assert_eq!(h.state().shell().selection().items, vec![entity], "a tap selects");
    // A tap on a link follows it.
    let link = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .find(|l| l.target.as_ref().is_some_and(|t| t.folio != "103"))
        .unwrap()
        .clone();
    let pos = screen(&h, ew_ui::ew_viewmodel::canvas::rect_centre(link.rect));
    tap(&mut h, pos);
    settle(&mut h);
    assert_eq!(
        h.state().shell().current_folio(),
        Some(link.target.unwrap().folio.as_str()),
        "a tap follows"
    );
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn a_press_on_the_splitter_between_the_groups_is_no_tool_input() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    dispatch(&mut h, "view.side_by_side", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let canvases: Vec<_> = h
        .get_all_by_role(egui::Role::Canvas)
        .map(|n| n.accesskit_node().raw_bounds().expect("bounds"))
        .collect();
    assert_eq!(canvases.len(), 2);
    let (left, right) = if canvases[0].x0 < canvases[1].x0 {
        (canvases[0], canvases[1])
    } else {
        (canvases[1], canvases[0])
    };
    // Inside the left canvas, 2 points from its edge: within the splitter's grab area between the groups.
    let y = ((left.y0 + left.y1) / 2.0) as f32;
    let from = Pos2::new(left.x1 as f32 - 2.0, y);
    assert!(from.x < right.x0 as f32);
    let before = h.state().shell().selection().clone();
    h.event(egui::Event::PointerMoved(from));
    h.run_ok();
    h.event(egui::Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    let to = from - egui::Vec2::new(60.0, 40.0);
    h.event(egui::Event::PointerMoved(to));
    h.run_ok();
    assert_ne!(h.state().shell().tool().state(), "rubber-band", "no band");
    assert!(h.state().shell().tool_overlay().is_empty());
    h.event(egui::Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    settle(&mut h);
    assert_eq!(
        h.state().shell().selection(),
        &before,
        "the selection is unchanged"
    );
    assert_eq!(h.state().shell().tool().state(), "idle");
    h.state_mut().shell_mut().shutdown();
}

/// The canvases' bounds, left to right.
fn canvas_bounds(h: &Harness<'_, App>) -> Vec<(f64, f64, f64, f64)> {
    let mut v: Vec<_> = h
        .get_all_by_role(egui::Role::Canvas)
        .map(|n| {
            let r = n.accesskit_node().raw_bounds().expect("bounds");
            (r.x0, r.y0, r.x1, r.y1)
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0));
    v
}

fn button(h: &mut Harness<'_, App>, pos: Pos2, pressed: bool) {
    h.event(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
}

/// FT-392: a canvas group hidden while a primary press is held on it (View ▸ Side by side off) escapes the tool as a
/// focus change does, and the release that follows reaches no tool, now or from a later click: no selection change,
/// no focus jump to the group when it shows again.
#[test]
fn a_group_hidden_while_a_press_is_held_drops_the_release() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    dispatch(&mut h, "view.side_by_side", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let bounds = canvas_bounds(&h);
    assert_eq!(bounds.len(), 2);
    let right = bounds[1];
    let centre = |r: (f64, f64, f64, f64)| Pos2::new(((r.0 + r.2) / 2.0) as f32, ((r.1 + r.3) / 2.0) as f32);
    // A band started in the right group (group 1), the button still down.
    let from = centre(right);
    let before = h.state().shell().selection().clone();
    h.event(egui::Event::PointerMoved(from));
    h.run_ok();
    button(&mut h, from, true);
    let to = from + Vec2::new(40.0, 30.0);
    h.event(egui::Event::PointerMoved(to));
    h.run_ok();
    assert_eq!(h.state().shell().tool().state(), "rubber-band");
    assert_eq!(h.state().shell().area().expect("area").focused, 1);
    // Side by side off while it is held: the band is escaped, as for a focus change.
    dispatch(&mut h, "view.side_by_side", json!({}));
    settle(&mut h);
    assert_eq!(canvas_bounds(&h).len(), 1);
    assert_eq!(h.state().shell().tool().state(), "idle");
    assert!(h.state().shell().tool_overlay().is_empty());
    // The release reaches no tool: the selection is unchanged.
    button(&mut h, to, false);
    settle(&mut h);
    assert_eq!(
        h.state().shell().selection(),
        &before,
        "the selection is unchanged"
    );
    assert_eq!(h.state().shell().tool().state(), "idle");
    // Side by side again (group 1 focused), then a click in the left group: the left group keeps the focus; no
    // release left over from the hidden group's press goes to group 1.
    dispatch(&mut h, "view.side_by_side", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    let left = canvas_bounds(&h)[0];
    let at = centre(left);
    h.event(egui::Event::PointerMoved(at));
    h.run_ok();
    button(&mut h, at, true);
    button(&mut h, at, false);
    settle(&mut h);
    assert_eq!(
        h.state().shell().area().expect("area").focused,
        0,
        "the left group keeps the focus"
    );
    assert_eq!(h.state().shell().tool().state(), "idle");
    h.state_mut().shell_mut().shutdown();
}

/// FT-392 headless: `canvas.release` after the pressed group was hidden is dropped (it names a group that no longer
/// exists and changes nothing), and the next press works as usual.
#[test]
fn a_release_after_the_pressed_group_was_hidden_is_dropped() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    dispatch(&mut h, "view.side_by_side", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    let (w, hh) = shell.canvas().expect("canvas").sheet();
    let p = |x: i64, y: i64| json!({"x_mm": x as f64 / 1e6, "y_mm": y as f64 / 1e6, "group": 1});
    let before = shell.selection().clone();
    assert_eq!(shell.dispatch("canvas.press", &p(w / 4, hh / 4)), Outcome::Done);
    assert_eq!(
        shell.dispatch("canvas.pointer", &p(w * 3 / 5, hh * 3 / 5)),
        Outcome::Done
    );
    assert_eq!(shell.tool().state(), "rubber-band");
    assert_eq!(shell.dispatch("view.side_by_side", &json!({})), Outcome::Done);
    assert_eq!(shell.tool().state(), "idle");
    assert_eq!(
        shell.dispatch("canvas.release", &p(w * 3 / 5, hh * 3 / 5)),
        Outcome::Done
    );
    assert_eq!(shell.selection(), &before);
    assert_eq!(shell.area().expect("area").groups.len(), 1);
    shell.shutdown();
}

#[test]
fn the_physical_bracket_key_steps_the_connection_points() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    // QWERTZ: the key right of P is `+` logically and `]` physically.
    let bracket = |h: &mut Harness<'_, App>| {
        h.event(egui::Event::Key {
            key: Key::Plus,
            physical_key: Some(Key::CloseBracket),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        });
        h.run_ok();
        h.run_ok();
    };
    let cps = h.state().shell().canvas().unwrap().page().cps.clone();
    let zoom = h.state().shell().canvas().unwrap().zoom();
    bracket(&mut h);
    let first = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .cursor()
        .expect("cp.next placed the cursor");
    let i = cps.iter().position(|c| c.at_nm == first).expect("on a CP");
    bracket(&mut h);
    assert_eq!(
        h.state().shell().canvas().unwrap().cursor(),
        Some(cps[(i + 1) % cps.len()].at_nm),
        "the next CP"
    );
    assert_eq!(
        h.state().shell().canvas().unwrap().zoom(),
        zoom,
        "not a zoom-in by the logical `+`"
    );
    settle(&mut h);
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
    assert_eq!(after.page_content, calls.page_content + 1, "the target page once");
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
    // The property grid fetches a sheet per selection change (WP-S2.05, its counts in `tests/properties.rs`): closed
    // here, so that the tool events alone are counted.
    dispatch(&mut h, "props.show", json!({"on": false}));
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
    // Checklist 9: every kind of step must also succeed, so the equalities below cannot pass vacuously.
    let generation = h.state().shell().selection().generation;
    let mut ok = std::collections::BTreeMap::<&str, u32>::new();
    let mut measuring = false;
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
        if out == Outcome::Done {
            *ok.entry(action).or_default() += 1;
        }
        measuring |= h.state().shell().tool().state() == "measuring";
        if i % 10 == 0 {
            h.run_ok();
        }
    }
    settle(&mut h);
    for action in [
        "canvas.press",
        "canvas.release",
        "canvas.pointer",
        "cp.next",
        "cp.previous",
        "cp.next_unconnected",
        "canvas.activate",
        "tab.next",
        "tool.measure",
        "tool.select",
        "canvas.dwell",
        "ui.dismiss",
    ] {
        assert!(
            ok.get(action).copied().unwrap_or(0) >= 1,
            "{action} succeeded: {ok:?}"
        );
    }
    assert!(
        h.state().shell().selection().generation > generation,
        "the selection changed"
    );
    assert!(measuring, "Measure reached measuring");
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

/// The dashed rectangles the canvas can paint: a rubber-band tool mark or the dashed hover outline. Both must be
/// absent on a page that was just loaded or changed and after a release, until the pointer moves again.
#[track_caller]
fn assert_no_dashed_rectangle(h: &Harness<'_, App>) {
    let shell = h.state().shell();
    let bands = shell
        .tool_overlay()
        .items
        .iter()
        .filter(|i| matches!(i, OverlayItem::Rect { style, .. } if style.token == Token::RubberBand))
        .count();
    assert_eq!(bands, 0, "a band mark in the tool overlay");
    let canvas = shell.canvas().expect("a canvas");
    assert!(canvas.hover().is_none(), "a dashed hover outline");
    assert!(canvas.hovered_link().is_none(), "a dashed link hover outline");
}

/// Ctrl+G, a folio, Enter: the go-to dialog's path.
fn goto_by_key(h: &mut Harness<'_, App>, folio: &str) {
    h.key_press_modifiers(Modifiers::COMMAND, Key::G);
    h.run_ok();
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some("Folio or cell"))
        .expect("the go-to field")
        .type_text(folio);
    h.run_ok();
    h.key_press(Key::Enter);
    h.run_ok();
}

/// Window check, item 5: a page loaded without pointer input paints no dashed rectangle before the pointer moves.
#[test]
fn a_page_change_without_pointer_input_paints_no_dashed_rectangle() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    // The pointer is over the canvas, but the go-to page change itself sends no pointer event.
    h.event(egui::Event::PointerMoved(Pos2::new(500.0, 350.0)));
    h.run_ok();
    goto_by_key(&mut h, "102");
    h.run_ok();
    assert_eq!(h.state().shell().current_folio(), Some("102"));
    assert_no_dashed_rectangle(&h);
    h.state_mut().shell_mut().shutdown();
}

/// Window check, item 5: a page change after a hover, with no pointer event after it, paints no dashed rectangle.
#[test]
fn a_page_change_after_a_hover_paints_no_dashed_rectangle() {
    let tmp = Tmp::new();
    let mut h = quick_golden(&tmp, "102");
    // The centre of an entity, wherever the fit put the sheet.
    let at = {
        let c = h.state().shell().canvas().unwrap();
        let p = ew_ui::ew_viewmodel::canvas::rect_centre(c.page().entities[0].1);
        screen(&h, p)
    };
    h.event(egui::Event::PointerMoved(at));
    settle(&mut h);
    assert!(
        h.state().shell().canvas().unwrap().hover().is_some(),
        "the hover before the page change"
    );
    goto_by_key(&mut h, "101");
    h.run_ok();
    assert_eq!(h.state().shell().current_folio(), Some("101"));
    assert_no_dashed_rectangle(&h);
    h.state_mut().shell_mut().shutdown();
}

/// Window check, item 5: after a band drag's release no dashed rectangle remains without a further mouse move.
#[test]
fn a_band_release_without_pointer_movement_paints_no_dashed_rectangle() {
    let tmp = Tmp::new();
    let mut h = quick_golden(&tmp, "102");
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    let from = screen(&h, (w / 5, hh / 5));
    let to = screen(&h, (w * 3 / 5, hh * 3 / 5));
    h.event(egui::Event::PointerMoved(from));
    settle(&mut h);
    h.event(egui::Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    h.event(egui::Event::PointerMoved(to));
    h.run_ok();
    h.event(egui::Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run_ok();
    h.run_ok();
    assert_eq!(h.state().shell().tool().state(), "idle");
    assert_no_dashed_rectangle(&h);
    h.state_mut().shell_mut().shutdown();
}

/// R2: a click without moving keeps the pointer known, so the Place tool's ghost is drawn again at it.
#[test]
fn a_stationary_click_keeps_the_place_ghost() {
    let tmp = Tmp::new();
    let mut h = quick_golden(&tmp, "103");
    dispatch(&mut h, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "place");
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    let pos = screen(&h, (w / 2, hh / 2));
    h.event(egui::Event::PointerMoved(pos));
    settle(&mut h);
    let Some(Ghost::Place {
        at_nm: Some(before), ..
    }) = h.state().shell().tool_ghost()
    else {
        panic!("the place ghost before the click");
    };
    // Press and release at the same point, with no PointerMoved between or after.
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    h.step();
    let Some(Ghost::Place {
        at_nm: Some(after), ..
    }) = h.state().shell().tool_ghost()
    else {
        panic!("the place ghost after the click");
    };
    assert_eq!(after, before, "the ghost stays at the pointer");
    assert!(
        !h.state().shell().tool_overlay().is_empty(),
        "the ghost is still drawn after the click"
    );
    h.state_mut().shell_mut().shutdown();
}

/// R2: a view change with a stationary pointer replays the pointer from its screen position, so ghosts and the hover
/// outline follow the new view.
#[test]
fn a_view_change_with_a_stationary_pointer_replays_the_pointer() {
    let tmp = Tmp::new();
    let mut h = quick_golden(&tmp, "102");
    let (w, hh) = h.state().shell().canvas().unwrap().sheet();
    let pos = screen(&h, (w / 2, hh / 2));
    h.event(egui::Event::PointerMoved(pos));
    settle(&mut h);
    let before = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .pointer()
        .expect("the pointer on the canvas");
    // A trackpad pan (a Point-unit wheel) moves the view under the stationary pointer.
    h.event(egui::Event::MouseWheel {
        unit: MouseWheelUnit::Point,
        delta: Vec2::new(80.0, 40.0),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    });
    h.step();
    h.step();
    let after = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .pointer()
        .expect("the pointer after the view change");
    assert_ne!(after, before, "the view moved the sheet under the pointer");
    assert_eq!(
        after,
        sheet_at_screen(&h, pos),
        "the pointer is replayed from its screen position"
    );
    assert!(
        h.state().shell().canvas().unwrap().hover().is_some(),
        "the hover outline is chosen again on the new view"
    );
    // A fit (Home) is the same view change: the pointer is replayed from the same screen point again.
    dispatch(&mut h, "view.fit_page", json!({}));
    h.step();
    h.step();
    assert_eq!(
        h.state().shell().canvas().unwrap().pointer().unwrap(),
        sheet_at_screen(&h, pos),
        "the pointer follows the fit too"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The Place tool armed with a terminal of strip -X1031 on folio 103, its ghost moved by the grid cursor and rested
/// on: the preview names the new terminal's tag. Returns the ghost's insertion point (page nm).
fn place_and_rest(h: &mut Harness<'_, App>) -> (i64, i64) {
    dispatch(
        h,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
    );
    settle(h);
    h.key_press(Key::ArrowRight);
    settle(h);
    dispatch(h, "canvas.dwell", json!({"ms": 80}));
    settle(h);
    match h.state().shell().tool_ghost() {
        Some(Ghost::Place {
            at_nm: Some(at),
            tag: Some(_),
            ..
        }) => at,
        other => panic!("a resting place ghost with its tag: {other:?}"),
    }
}

/// Whether `frame` stands where the chip rule (WP-S2.17 D13) may put a chip by the point `anchor`: one of its four
/// candidates, 8 points off.
fn by_point(frame: egui::Rect, anchor: Pos2) -> bool {
    use ew_ui::ew_viewmodel::canvas::chip::{Pos2 as P, Rect as R, Vec2 as V, candidates};
    candidates(
        R::point(P::new(anchor.x, anchor.y)),
        V::new(frame.width(), frame.height()),
    )
    .iter()
    .any(|c| (c.min.x - frame.min.x).abs() < 1.0 && (c.min.y - frame.min.y).abs() < 1.0)
}

/// The screen point of the sheet point `p` on the focused canvas as its last frame drew it: with the transform the
/// chips were placed by, which once the view rests is the settled one (`round(D)` dpi, a whole-pixel origin) and not
/// the model's exact zoom; away from the pane's centre the two differ by a few points.
fn drawn_of(h: &mut Harness<'_, App>, p: (i64, i64)) -> Pos2 {
    h.state_mut()
        .canvas_host()
        .drawn_screen_of(0, p)
        .expect("a drawn canvas")
}

/// Every chip of the focused canvas: no two intersect, none is faded, none covers `panel`.
fn assert_chips_apart(
    h: &mut Harness<'_, App>,
    panel: Option<egui::Rect>,
) -> Vec<ew_ui::canvas_chips::CanvasChip> {
    let chips = h.state_mut().canvas_host().canvas_chips(0).to_vec();
    for (i, a) in chips.iter().enumerate() {
        assert!(!a.faded, "{a:?} found a free place");
        if let Some(p) = panel {
            assert!(!a.rect.intersects(p), "{a:?} covers the panel {p:?}");
        }
        for b in &chips[i + 1..] {
            assert!(!a.rect.intersects(b.rect), "{a:?} and {b:?} apart");
        }
    }
    chips
}

/// FT-389, FT-507 and FT-508 (WP-S2.17 part 5, OV2): the Place ghost of a terminal on Golden A draws one label with
/// its tag once, even after a second rest; the preview's change chip stands by the floating insertion point, off the
/// label (the rule's next candidate), the header by it; with the frame statistics shown and the anchor
/// moved beside their panel, no chip covers the panel; no two chips intersect, with the statistics on and off.
#[test]
fn ov2_the_place_ghost_draws_its_tag_once_and_its_chips_keep_apart() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    let at = place_and_rest(&mut h);
    // A second rest at the same point (the 300-ms dwell previews again): the label still names the tag once.
    dispatch(&mut h, "canvas.dwell", json!({"ms": 300}));
    settle(&mut h);
    let tl = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .snapper
        .page_to_sheet(at);
    let (chips, stats) = h.state().preview_chips();
    assert!(stats.is_none());
    assert!(chips.len() >= 2, "{chips:?}");
    let all = assert_chips_apart(&mut h, None);
    let labels: Vec<_> = all
        .iter()
        .filter(|c| c.kind == ew_ui::canvas_chips::ChipKind::Label)
        .collect();
    assert_eq!(labels.len(), 1, "one ghost label: {all:?}");
    assert_eq!(
        labels[0].text.matches("-X1031:30").count(),
        1,
        "the tag once: {:?}",
        labels[0].text
    );
    let anchor = drawn_of(&mut h, tl);
    let (text, frame) = &chips[1];
    assert!(text.contains("-X1031:30"), "{chips:?}");
    assert!(by_point(*frame, anchor), "{frame:?} by {anchor:?}");
    assert!(
        !frame.intersects(labels[0].rect),
        "the chip keeps off the ghost's label"
    );
    let head = chips[0].1;
    assert!(chips[0].0.contains("device.place"), "{chips:?}");
    // The header's target is the point 8 points left of the change's top-left corner: straight above the change
    // where that is free; here the ghost's label takes that place, and the header takes the rule's next free one.
    assert!(
        by_point(head, frame.left_top() - Vec2::new(8.0, 0.0)),
        "{head:?} by {frame:?}"
    );

    // The statistics shown, and the view panned so that the anchor lies just left of their panel, level with its
    // middle: the candidates right of the anchor fall on the panel, and the chips take the free ones. The panel is
    // anchored at the canvas's top right and sized by its live text (frame rate and p99 timings, measured against
    // the wall clock), so its left edge moves by a few points from frame to frame: the pan aims at the panel of one
    // frame, and the checks read the panel, the chips and the anchor of one later frame, all drawn together.
    dispatch(&mut h, "view.frame_stats", json!({}));
    settle(&mut h);
    let panel = h.state().preview_chips().1.expect("the statistics panel");
    let d = Pos2::new(panel.left() - 4.0, panel.center().y) - anchor;
    dispatch(&mut h, "view.pan", json!({"dx": d.x, "dy": d.y}));
    settle(&mut h);
    let anchor = drawn_of(&mut h, tl);
    let (chips, stats) = h.state().preview_chips();
    let panel = stats.expect("the statistics panel");
    assert!(chips.len() >= 2, "the preview is kept: {chips:?}");
    // The setting holds in this frame: both candidates right of the anchor (8 points off, the change chip's size)
    // fall on the panel, so the rule has to take a left one.
    {
        use ew_ui::ew_viewmodel::canvas::chip::{Pos2 as P, Rect as R, Vec2 as V, candidates};
        let size = chips[1].1.size();
        let c = candidates(R::point(P::new(anchor.x, anchor.y)), V::new(size.x, size.y));
        for right in [&c[0], &c[1]] {
            let r = egui::Rect::from_min_max(
                Pos2::new(right.min.x, right.min.y),
                Pos2::new(right.max.x, right.max.y),
            );
            assert!(
                r.intersects(panel),
                "the right-hand candidate {r:?} of the anchor {anchor:?} falls on the panel {panel:?}"
            );
        }
    }
    assert_chips_apart(&mut h, Some(panel));
    assert!(by_point(chips[1].1, anchor), "still by the tag: {:?}", chips[1]);
    assert!(chips[1].1.right() <= anchor.x, "a left candidate: {:?}", chips[1]);
    h.state_mut().shell_mut().shutdown();
}

/// R30 F3 in the window: after Insert ▸ Image…'s file dialog answers, the status hint is the armed tool's at once,
/// without a pointer event over the canvas.
#[test]
fn r30_the_image_hint_follows_the_file_answer_without_a_pointer_move() {
    let tmp = Tmp::new();
    let mut png = [0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a].to_vec();
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&8u32.to_be_bytes());
    png.extend_from_slice(&2u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    let file = tmp.0.join("pick.png");
    std::fs::write(&file, &png).unwrap();
    let scripted = Scripted::default();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, scripted.clone(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    scripted
        .answers
        .borrow_mut()
        .push_back(ew_ui::app::Pick::Path(file));
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "tool.image", json!({}));
    h.run_ok();
    let shell = h.state().shell();
    assert_eq!(shell.tool().state(), "armed");
    let fields = shell.status_fields(false);
    assert!(
        !fields.iter().any(|f| f.contains("Choose an image asset")),
        "{fields:?}"
    );
    assert!(fields.iter().any(|f| f.contains("first corner")), "{fields:?}");
    h.state_mut().shell_mut().shutdown();
}

/// A platform whose Open image dialog answers `path` from its own thread, as the window's native dialog does (FT-467).
struct ThreadedImage(std::path::PathBuf);

impl ew_ui::app::Platform for ThreadedImage {
    fn pick(&mut self, _: ew_ui::ew_viewmodel::shell::Prompt) -> ew_ui::app::Pick {
        panic!("a threaded platform is asked through start_pick");
    }

    fn start_pick(
        &mut self,
        _: ew_ui::ew_viewmodel::shell::Prompt,
        ctx: &egui::Context,
    ) -> ew_ui::app::PickReply {
        let path = self.0.clone();
        ew_ui::app::PickReply::Later(
            ew_ui::app::spawn_pick(ctx, move || ew_ui::app::Pick::Path(path)).unwrap(),
        )
    }
}

/// R30 F3 through the dialog's thread: the frames that follow the answer show the armed hint in the status bar
/// without any pointer event.
#[test]
fn r30_the_image_hint_follows_a_threaded_file_answer_without_a_pointer_move() {
    let tmp = Tmp::new();
    let mut png = [0x89u8, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a].to_vec();
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&8u32.to_be_bytes());
    png.extend_from_slice(&2u32.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    let file = tmp.0.join("pick.png");
    std::fs::write(&file, &png).unwrap();
    let dir = tmp.golden("g.ew");
    let shell =
        ew_ui::ew_viewmodel::shell::ShellModel::new(ew_ui::ew_viewmodel::shell::Mode::Gui, tmp.config());
    let app = App::new(shell, Box::new(ThreadedImage(file)), Some(dir));
    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(1000.0, 700.0))
        .with_context_setup(ew_ui::app::setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app);
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "tool.image", json!({}));
    // The dialog's thread answers at once; a bounded number of frames waits for it (no wall clock in tests).
    for _ in 0..1000 {
        if h.state().shell().tool().state() == "armed" {
            break;
        }
        std::thread::yield_now();
        h.step();
    }
    h.step();
    assert_eq!(h.state().shell().tool().state(), "armed");
    assert!(
        h.query_all_by_label_contains("Choose an image asset")
            .next()
            .is_none()
    );
    // WP-S2.17 part 2: the hint is the HUD's.
    assert!(
        support::hud_value(&h).contains("first corner"),
        "{}",
        support::hud_value(&h)
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-502: the chip of a tag change on an existing device stands above its printed tag text (`TableRep::tag_at`),
/// not the body's corner. The flow: on a copy of Golden A with a numbering scheme that prints the column (Golden A's
/// own tags change with no move on a page), -101F1 is selected, moved to the right with the keyboard and rested on.
#[test]
fn the_chip_of_an_existing_devices_tag_change_stands_above_its_printed_tag() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    // WP-S2.17 part 3 (D10): the rails and the wider left column narrow the page view; at 1000 px the chip meets the
    // view's edge and is pushed off its candidates, so this window has the reference width.
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    let numbering = json!({"default": "page-based", "schemes": [
        {"key": "page-based", "format": "{page}{letter}{counter}.{column}", "rules": []}]});
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": numbering}}),
    );
    settle(&mut h);
    let rep = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.symbol_key == "ew.iec.mcb-1pn@qet10")
        .cloned()
        .expect("-101F1");
    let tag_at = rep.tag_at.expect("-101F1 prints its tag");
    let snap = h.state().shell().canvas().unwrap().page().snapper.clone();
    let centre = snap.page_to_sheet(((rep.body[0] + rep.body[2]) / 2, (rep.body[1] + rep.body[3]) / 2));
    dispatch(
        &mut h,
        "cursor.set",
        json!({"x_mm": centre.0 as f64 / 1e6, "y_mm": centre.1 as f64 / 1e6}),
    );
    dispatch(&mut h, "canvas.activate", json!({}));
    dispatch(&mut h, "cursor.set", json!({}));
    dispatch(&mut h, "tool.move", json!({}));
    for _ in 0..12 {
        dispatch(&mut h, "cursor.right", json!({}));
    }
    dispatch(&mut h, "canvas.dwell", json!({"ms": 80}));
    settle(&mut h);
    let (chips, _) = h.state().preview_chips();
    assert!(chips.len() >= 2, "{chips:?}");
    assert!(chips[0].0.contains("rep.move"), "{chips:?}");
    let (text, frame) = &chips[1];
    assert!(text.contains("-101F1."), "{chips:?}");
    let anchor = drawn_of(&mut h, snap.page_to_sheet(tag_at));
    // By the printed tag text, by the chip rule of WP-S2.17 D13 (one of its four candidates).
    assert!(by_point(*frame, anchor), "{frame:?} by {anchor:?}");
    // Not by the body's corner, where FT-389 put it.
    let corner = drawn_of(&mut h, snap.page_to_sheet((rep.body[0], rep.body[1])));
    assert!(
        !by_point(*frame, corner),
        "{frame:?} is by the body's corner {corner:?}"
    );
    h.state_mut().shell_mut().shutdown();
}
