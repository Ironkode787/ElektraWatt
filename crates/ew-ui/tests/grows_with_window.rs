// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 39 (window check W39c2; doc 16a §2, ID-908: the UI is not fitted to the floor): the Choose part dialog, the
//! Device-from-part picker and the command palette grow with the window. Each is measured at the floor (1280 by 800)
//! and at the typical size (1920 by 1080): wider at the typical size, a share of the window there, and never narrower
//! than at the floor.

mod support;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, size: egui::Vec2) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    h.set_size(size);
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

/// The width of the text field with the placeholder `hint`.
fn field_width(h: &Harness<'_, App>, hint: &str) -> f32 {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().data().placeholder() == Some(hint))
        .unwrap_or_else(|| panic!("the field {hint}"))
        .rect()
        .width()
}

/// Measures `measure` after `open` at the floor and at the typical size.
fn at_both_sizes(
    open: impl Fn(&mut Harness<'_, App>),
    measure: impl Fn(&Harness<'_, App>) -> f32,
) -> (f32, f32) {
    let mut out = Vec::new();
    for size in [egui::Vec2::new(1280.0, 800.0), egui::Vec2::new(1920.0, 1080.0)] {
        let tmp = Tmp::new();
        let mut h = on_golden(&tmp, size);
        open(&mut h);
        settle(&mut h);
        settle(&mut h);
        settle(&mut h);
        out.push(measure(&h));
        h.state_mut().shell_mut().shutdown();
    }
    (out[0], out[1])
}

fn assert_grows(what: &str, floor: f32, typical: f32) {
    assert!(
        typical > floor,
        "{what}: {typical} at 1920 against {floor} at 1280"
    );
    assert!(
        typical >= 0.3 * 1920.0,
        "{what}: {typical} is no share of the 1920-wide window"
    );
}

#[test]
fn the_choose_part_dialog_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| {
            dispatch(h, "select.set", json!({"refs": ["tag:F55"]}));
            dispatch(h, "part.choose", json!({"use": 0}));
        },
        |h| {
            h.get_by_role_and_label(egui::Role::ListBox, "Parts")
                .rect()
                .width()
        },
    );
    assert_grows("the part list", floor, typical);
}

#[test]
fn the_part_picker_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| {
            dispatch(h, "page.goto", json!({"folio": "101"}));
            dispatch(h, "tool.place_part", json!({}));
        },
        |h| field_width(h, "Filter by key, part number or function"),
    );
    assert_grows("the picker's filter", floor, typical);
}

#[test]
fn the_command_palette_grows_with_the_window() {
    let (floor, typical) = at_both_sizes(
        |h| dispatch(h, "tools.palette", json!({})),
        |h| field_width(h, "Type a command…"),
    );
    assert_grows("the palette's field", floor, typical);
}
