// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An Arrange and route card's rows scroll in the Preview pane under the mouse wheel (a persona run saw 8 of 18 symbol
//! rows, the last cut at the pane's edge, and the wheel moved nothing). On a copy of the sample, its page 103.

use crate::support;

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, TouchPhase, Vec2};
use egui_kittest::kittest::Queryable;
use serde_json::json;
use support::{Tmp, dispatch, settle};

#[test]
fn the_arrange_cards_rows_scroll_under_the_wheel() {
    let tmp = Tmp::new();
    let mut h = support::on_sample(&tmp, "103");
    dispatch(&mut h, "select.set", json!({"ids": []}));
    dispatch(&mut h, "layout.arrange", json!({}));
    settle(&mut h);
    settle(&mut h);
    // The pane made low, so that the card's rows run past its end.
    let at = h
        .get_by_role_and_label(egui::Role::Splitter, "Resize Page view and Preview")
        .rect()
        .center();
    let below = at + Vec2::new(0.0, 250.0);
    h.event(Event::PointerMoved(at));
    h.run_ok();
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.event(press(at, true));
    h.run_ok();
    h.event(Event::PointerMoved(below));
    h.run_ok();
    h.event(press(below, false));
    settle(&mut h);
    let pane = h
        .get_by_role_and_label(egui::Role::Group, "Preview")
        .rect();
    // The top of the card's first row, where the pane shows one.
    let first = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        h.query_all_by_role(egui::Role::CheckBox)
            .map(|n| n.rect())
            .filter(|r| pane.contains(r.center()))
            .map(|r| r.top())
            .reduce(f32::min)
    };
    let before = first(&h).expect("a row of the card");
    h.hover_at(pane.center());
    h.event(Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: Vec2::new(0.0, -3.0),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    });
    settle(&mut h);
    settle(&mut h);
    let after = first(&h).unwrap_or(f32::MIN);
    assert!(after < before, "the rows moved up: {before} -> {after}");
}
