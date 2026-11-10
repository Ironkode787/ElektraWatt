// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1337 in the window, on a writable copy of Golden A with the UI in Dutch: a
//! command's own refusal shows in the window's banner in Dutch, whole, a record named in words in Dutch too; FT-1241's
//! refusal (a point that defines a potential already) is one of them.

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
    let dir = tmp.golden("g.ew");
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
    assert_eq!(
        refused_in_window(
            &mut h,
            "page.surface_view.set",
            json!({"page": {"ref": "folio:101"}, "surface": "00000000-0000-0000-0000-0000000000ab",
                "datum_at": [0, 0]})
        ),
        "folio 101 is geen paneellayoutpagina; een paneelaanzicht hoort op een paneellayoutpagina"
    );
    h.state_mut().shell_mut().shutdown();
}
