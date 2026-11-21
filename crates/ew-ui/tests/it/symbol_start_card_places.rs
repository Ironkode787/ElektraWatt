// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1810, through the window: the symbol editor's start card keeps its ways in where they stand when a draft is in
//! hand (Carry on with… came on top and moved every entry down).

use crate::support;

use egui::{Modifiers, PointerButton};
use ew_ui::visual::{Driver, Input};
use ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::Tmp;

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn done(d: &mut Driver, action: &str, params: Value) {
    assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done, "{action}");
}

#[test]
fn the_ways_in_keep_their_places_with_a_draft_in_hand() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(&mut d, "symbol.new", json!({}));
    d.settle();
    let before = d.find("New symbol").expect("New symbol on the start card");
    d.input(&click(before));
    d.settle();
    done(
        &mut d,
        "symbol.draw",
        json!({"tool": "line", "x": 0, "y": 0, "x2": 0, "y2": 4}),
    );
    d.settle();
    d.input(&click(d.find("Start").expect("the editor's Start")));
    d.settle();
    let carry = d
        .labels()
        .into_iter()
        .find(|(l, _)| l.starts_with("Carry on with "))
        .map(|(_, at)| at)
        .unwrap_or_else(|| panic!("Carry on with the draft: {:?}", d.labels()));
    assert_eq!(
        d.find("New symbol"),
        Some(before),
        "New symbol stays where it stood"
    );
    let import = d.find("Import a file\u{2026}").expect("Import a file…");
    assert!(carry.1 > import.1, "Carry on with… after the ways in");
    d.shell_mut().shutdown();
}
