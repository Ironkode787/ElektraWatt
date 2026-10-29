// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP blind-drafter run's findings of cycle 33, lane M33 (draft FT rows 4, 6 and 13), through the text
//! interface: a first black box from a part arms with its connection points, a tag-letter change takes the next free
//! counter, and the marker tool takes a press and a release with no move between.

mod common;

use common::{GOLDEN_A, Tmp, done, state};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// A headless shell with a new project (the builtin template) and a new folio 101 shown.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

/// Row 4: the very first pick of a black-box part on a new project (its FnDefs not yet embedded) arms the black box
/// with the part's connection points, the same as a repeat pick.
#[test]
fn row4_the_first_black_box_from_a_part_has_its_connection_points() {
    let tmp = Tmp::new("m33-row4");
    let mut s = new_project(&tmp);
    // As the run's prelude: the part alone is embedded, its FnDef (ew.fn.blackbox) is not.
    let part: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{GOLDEN_A}/lib/parts/68075c2c-1eed-5753-8749-88e37b4496d5.json"
        ))
        .unwrap(),
    )
    .unwrap();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "library.embed", "params": {"copies": [part]}}),
    );
    done(&mut s, "place.pick", json!({"part": "2904602"}));
    let first = state(&s)["canvas"]["ghost"].clone();
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "place.pick", json!({"part": "2904602"}));
    let again = state(&s)["canvas"]["ghost"].clone();
    assert_eq!(first["kind"], "box", "{first}");
    assert_eq!(first["cps"], again["cps"], "first {first} again {again}");
    assert_ne!(first["cps"], json!(0), "{first}");
}
