// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text that writes a connection point's number goes with the point (FT-1990): moved alone by the arrow keys
//! it moves as far; given another edge in Properties it stands again inside the body at the point, turned by its edge.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::{Value, json};

/// The connection point's place and its number text's place, rotation in the symbol frame.
fn places(shell: &ShellModel) -> ((i64, i64), (i64, i64), Value) {
    let draft = &shell.symbol_editor().expect("the editor").draft;
    let doc = serde_json::to_value(draft).unwrap();
    let cp = &doc["symbol"]["cps"][0];
    let slot = doc["symbol"]["text_slots"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|s| s["kind"].get("cp_designation") == Some(&cp["idx"]))
        })
        .cloned()
        .expect("the point's number text");
    let at = |v: &Value| (v["pos"][0].as_i64().unwrap(), v["pos"][1].as_i64().unwrap());
    (at(cp), at(&slot), slot["rot"].clone())
}

#[test]
fn a_points_number_text_moves_and_turns_with_it() {
    let tmp = Tmp::new("symbol-cp-text-follows");
    let mut shell = tmp.empty_shell(Mode::Headless);
    done(
        &mut shell,
        "symbol.new",
        json!({"key": "example.follow", "name": "Follow"}),
    );
    done(
        &mut shell,
        "symbol.cp",
        json!({"x": 0, "y": 4, "designation": "1", "direction": "left"}),
    );
    let (cp, text, _) = places(&shell);
    done(&mut shell, "symbol.select", json!({"items": ["cp:0"]}));
    done(&mut shell, "symbol.nudge", json!({"dx": 2, "dy": 1}));
    let (moved_cp, moved_text, rot) = places(&shell);
    assert_ne!(moved_cp, cp, "the point moves");
    assert_eq!(
        (moved_text.0 - text.0, moved_text.1 - text.1),
        (moved_cp.0 - cp.0, moved_cp.1 - cp.1),
        "its text moves as far"
    );
    done(&mut shell, "symbol.prop", json!({"key": "dir", "value": "up"}));
    let (up_cp, up_text, up_rot) = places(&shell);
    assert_eq!(up_cp, moved_cp, "another edge does not move the point");
    assert!(
        up_text != moved_text || up_rot != rot,
        "the text stands again for the new edge"
    );
    assert_eq!(
        up_text.0, up_cp.0,
        "a pin on the top edge has its text in line below it"
    );
}
