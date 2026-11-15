// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! rc.2, the owner's F9 and F12: a device shows what belongs to what it is. Properties says what the selected thing
//! is in trade words, shows only the rows that belong to it, and its menus offer only what fits it.

use crate::common;

use common::{Tmp, at, describe_line, done};
use ew_viewmodel::ew_api::output::TableRep;
use ew_viewmodel::properties::what_words;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let dir = tmp.0.join("new.ew");
    done(
        &mut s,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    for (folio, ty) in [("101", "multi_line"), ("102", "fluid")] {
        done(
            &mut s,
            "page.new",
            json!({"folio": folio, "type": ty, "title": ty}),
        );
    }
    s
}

fn place_at(s: &mut ShellModel, k: (i64, i64)) {
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
    let _ = s.dispatch("ui.dismiss", &json!({}));
    let _ = s.dispatch("ui.dismiss", &json!({}));
}

fn reps(s: &ShellModel) -> Vec<TableRep> {
    s.canvas().unwrap().page().tables.reps.clone()
}

fn place(s: &mut ShellModel, params: serde_json::Value, k: (i64, i64)) -> TableRep {
    let before = reps(s);
    done(s, "place.pick", params);
    place_at(s, k);
    reps(s)
        .into_iter()
        .find(|r| before.iter().all(|b| b.id != r.id))
        .expect("a new symbol")
}

/// The header's line under the title of the one symbol `rep`'s sheet.
fn what_of(s: &mut ShellModel, rep: &TableRep) -> String {
    done(s, "select.set", json!({"ids": [rep.id.to_string()]}));
    let sheet = s.properties().sheet().expect("a sheet");
    what_words(sheet.header.what.as_ref().expect("what the device is"))
}

/// Item 1: the header says what the device is, from its symbol and the disciplines of its points: an air supply is
/// pneumatic and nothing electrical, a 5/2 valve pneumatic with its one solenoid, a relay's coil electrical; the same
/// words reach the text interface.
#[test]
fn the_header_says_what_a_device_is() {
    let tmp = Tmp::new("props-kind-what");
    let mut s = new_project(&tmp);
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let air = place(&mut s, json!({"symbol": "ew.iso.air-supply@qet10", "letter": "X"}), (10, 10));
    let what = what_of(&mut s, &air);
    assert!(what.ends_with(" · pneumatic"), "{what}");
    let line = describe_line(&s, "properties what").expect("the describe line");
    assert!(line.contains(&what), "{line}");
    let valve = place(&mut s, json!({"part": "EW-GEN-VLV-1", "template": 0}), (30, 10));
    let what = what_of(&mut s, &valve);
    assert!(what.ends_with(" · pneumatic, 1 solenoid"), "{what}");
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let coil = place(&mut s, json!({"part": "EW-GEN-RLY-1", "template": 0}), (10, 10));
    let what = what_of(&mut s, &coil);
    assert!(what.ends_with(" · electrical"), "{what}");
    s.shutdown();
}
