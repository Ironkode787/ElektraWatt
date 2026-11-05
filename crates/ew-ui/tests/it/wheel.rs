// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-895: the mouse wheel on the canvas, measured. For each of (dy +3, dy −3, dx +3, dx −3) × (no modifier, Ctrl,
//! Shift), the zoom and the view's centre before and after one wheel event of three lines, sent (a) as egui's events
//! straight into the window and (b) by the visual driver's `scroll` step.

use crate::support;

use egui::{Modifiers, MouseWheelSource, MouseWheelUnit, TouchPhase, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input};
use serde_json::json;
use support::{Scripted, Tmp, harness, settle, wait_for_page};

/// The script's wheel steps: `(dx, dy)` in lines, as the driver's `scroll` takes them (positive: right, down).
const STEPS: [(i64, i64); 4] = [(0, 3), (0, -3), (3, 0), (-3, 0)];

/// The modifiers of a row.
fn mods(name: &str) -> Modifiers {
    match name {
        "ctrl" => Modifiers::CTRL,
        "shift" => Modifiers::SHIFT,
        _ => Modifiers::NONE,
    }
}

/// What a step did: the zoom in per cent, and the centre's move in mm (positive: right, down).
#[derive(Debug, PartialEq)]
struct Change {
    zoom: (i64, i64),
    moved_mm: (i64, i64),
}

/// The table, from 100 % with the pointer at the canvas's centre: (modifiers, (dx, dy), zoom after in per cent, the
/// centre's move in mm). The plain vertical wheel steps the zoom ladder three steps about the pointer (by design, not
/// changed); Ctrl with the wheel is egui's continuous zoom, out for a line down, in for a line up (the play-test's "in
/// both directions" did not reproduce); FT-895's fix: a sideways wheel and Shift with the wheel pan horizontally, three
/// of egui's lines (32 mm at 100 %), a line down or right showing more of the right (doc 16a §5.5 names no wheel).
/// A row of the table: modifiers, the step `(dx, dy)`, the zoom after in per cent, the centre's move in mm.
type Row = (&'static str, (i64, i64), i64, (i64, i64));

const TABLE: [Row; 12] = [
    ("none", (0, 3), 35, (0, 0)),
    ("none", (0, -3), 283, (0, 0)),
    ("none", (3, 0), 100, (32, 0)),
    ("none", (-3, 0), 100, (-32, 0)),
    ("ctrl", (0, 3), 55, (0, 0)),
    ("ctrl", (0, -3), 182, (0, 0)),
    ("ctrl", (3, 0), 55, (0, 0)),
    ("ctrl", (-3, 0), 182, (0, 0)),
    ("shift", (0, 3), 100, (32, 0)),
    ("shift", (0, -3), 100, (-32, 0)),
    ("shift", (3, 0), 100, (32, 0)),
    ("shift", (-3, 0), 100, (-32, 0)),
];

/// The table as `(modifiers, step, change)` rows.
fn expected() -> Vec<(&'static str, (i64, i64), Change)> {
    TABLE
        .iter()
        .map(|&(m, step, zoom, moved_mm)| {
            (
                m,
                step,
                Change {
                    zoom: (100, zoom),
                    moved_mm,
                },
            )
        })
        .collect()
}

fn change(before: (f64, (i64, i64)), after: (f64, (i64, i64))) -> Change {
    let pct = |z: f64| (z * 100.0).round() as i64;
    Change {
        zoom: (pct(before.0), pct(after.0)),
        moved_mm: (
            ((after.1.0 - before.1.0) as f64 / 1e6).round() as i64,
            ((after.1.1 - before.1.1) as f64 / 1e6).round() as i64,
        ),
    }
}

fn on_golden(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let out = h
        .state_mut()
        .shell_mut()
        .dispatch("page.goto", &json!({"folio": "101"}));
    assert_eq!(out, Outcome::Done);
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn view(h: &Harness<'_, App>) -> (f64, (i64, i64)) {
    let c = h.state().shell().canvas().unwrap();
    (c.zoom(), c.centre())
}

/// The canvas's centre in window points.
fn canvas_centre(h: &Harness<'_, App>) -> egui::Pos2 {
    let b = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    egui::Pos2::new(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
}

/// (a): egui's events of a mouse wheel of `(dx, dy)` lines with `m` held, as a backend sends them (the modifiers
/// changed, then the wheel event with them; a line down is a negative `delta.y`), then the frames that settle it.
#[test]
fn ft895_the_wheel_table_by_egui_events() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    let mut table = Vec::new();
    for m in ["none", "ctrl", "shift"] {
        for step in STEPS {
            let out = h
                .state_mut()
                .shell_mut()
                .dispatch("view.zoom_to", &json!({"percent": 100.0}));
            assert_eq!(out, Outcome::Done);
            settle(&mut h);
            let at = canvas_centre(&h);
            h.input_mut().events.push(egui::Event::PointerMoved(at));
            h.step();
            let before = view(&h);
            let input = h.input_mut();
            input.events.push(egui::Event::ModifiersChanged(mods(m)));
            input.events.push(egui::Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: Vec2::new(-(step.0 as f32), -(step.1 as f32)),
                phase: TouchPhase::Move,
                source: MouseWheelSource::Unknown,
                modifiers: mods(m),
            });
            h.step();
            h.input_mut()
                .events
                .push(egui::Event::ModifiersChanged(Modifiers::NONE));
            settle(&mut h);
            table.push((m, step, change(before, view(&h))));
        }
    }
    assert_eq!(table, expected());
    h.state_mut().shell_mut().shutdown();
}

/// (b): the same through the visual driver's `scroll` step at the canvas's centre.
#[test]
fn ft895_the_wheel_table_by_the_visual_driver() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let mut table = Vec::new();
    let view = |d: &mut Driver| {
        let c = d.shell_mut().canvas().unwrap();
        (c.zoom(), c.centre())
    };
    for m in ["none", "ctrl", "shift"] {
        for step in STEPS {
            let _ = d.shell_mut().dispatch("view.zoom_to", &json!({"percent": 100.0}));
            d.settle();
            let name = ew_ui::ew_viewmodel::status::canvas_node(d.app().unwrap().shell(), 0)
                .expect("the canvas node")
                .name;
            let at = d.find(&name).expect("the canvas");
            d.input(&Input::Hover { at });
            let before = view(&mut d);
            d.input(&Input::Scroll {
                at,
                dx: step.0,
                dy: step.1,
                mods: mods(m),
            });
            d.settle();
            table.push((m, step, change(before, view(&mut d))));
        }
    }
    assert_eq!(table, expected());
}

/// FT-1439 (ID-1397): with View ▸ Mouse wheel scrolls on, the plain vertical wheel scrolls three of egui's lines (32 mm
/// at 100 %) and leaves the zoom, a line down showing more of the bottom; Ctrl with the wheel still zooms.
#[test]
fn ft1439_the_wheel_scrolls_by_the_preference() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp);
    let out = h
        .state_mut()
        .shell_mut()
        .dispatch("view.wheel_scrolls", &json!({"on": true}));
    assert_eq!(out, Outcome::Done);
    let mut table = Vec::new();
    for m in ["none", "ctrl"] {
        for step in [(0, 3), (0, -3)] {
            let out = h
                .state_mut()
                .shell_mut()
                .dispatch("view.zoom_to", &json!({"percent": 100.0}));
            assert_eq!(out, Outcome::Done);
            settle(&mut h);
            let at = canvas_centre(&h);
            h.input_mut().events.push(egui::Event::PointerMoved(at));
            h.step();
            let before = view(&h);
            let input = h.input_mut();
            input.events.push(egui::Event::ModifiersChanged(mods(m)));
            input.events.push(egui::Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: Vec2::new(-(step.0 as f32), -(step.1 as f32)),
                phase: TouchPhase::Move,
                source: MouseWheelSource::Unknown,
                modifiers: mods(m),
            });
            h.step();
            h.input_mut()
                .events
                .push(egui::Event::ModifiersChanged(Modifiers::NONE));
            settle(&mut h);
            table.push((m, step, change(before, view(&h))));
        }
    }
    let row = |m, step, zoom: i64, moved_mm| {
        (
            m,
            step,
            Change {
                zoom: (100, zoom),
                moved_mm,
            },
        )
    };
    assert_eq!(
        table,
        [
            row("none", (0, 3), 100, (0, 32)),
            row("none", (0, -3), 100, (0, -32)),
            row("ctrl", (0, 3), 55, (0, 0)),
            row("ctrl", (0, -3), 182, (0, 0)),
        ]
    );
    h.state_mut().shell_mut().shutdown();
}
