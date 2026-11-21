// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Refusals in Dutch in the window, on a writable copy of the sample with the UI in Dutch: a command's own refusal
//! (a point that defines a potential already) shows in the window's banner in Dutch, whole. Not here: the Dutch
//! words of the other refusals (`ew-viewmodel`'s `refusals_in_dutch.rs`).

use crate::support;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::App;
use serde_json::{Value, json};
use support::{Scripted, Tmp, dispatch, harness, settle, wait_for_page};

/// `command.run` of `cmd` as the window runs it: its refusal is the banner, which the window shows.
fn refused_in_window(h: &mut Harness<'_, App>, cmd: &str, params: Value) -> String {
    h.state_mut()
        .dispatch_from_window("command.run", &json!({"cmd": cmd, "params": params}));
    settle(h);
    let banner = h.state().shell().banner().expect("a banner").to_owned();
    assert!(h.query_by_label(&banner).is_some(), "the window shows {banner:?}");
    banner
}

#[test]
fn a_refused_command_shows_its_reason_in_dutch_in_the_window() {
    let tmp = Tmp::new();
    let dir = tmp.sample("s.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "102" }));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "view.language", json!({ "value": "nl" }));
    settle(&mut h);
    let junction = h.state().shell().canvas().unwrap().page().tables.junctions[0].id;
    let at = json!({"junction": {"junction": junction.to_string()}});
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "potential.define", "params": {"at": at, "name": "FT1241"}}),
    );
    assert_eq!(
        refused_in_window(&mut h, "potential.define", json!({"at": at, "name": "FT1241-B"})),
        "dit punt definieert de potentiaal FT1241 al; wijzig in plaats daarvan die definitie"
    );
    h.state_mut().shell_mut().shutdown();
}
