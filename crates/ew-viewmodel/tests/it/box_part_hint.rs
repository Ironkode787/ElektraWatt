// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part without a symbol of its own, placed with the Place tool, is drawn as a box and the hint says so (FT-1808);
//! the Black box tool, where a person chose the box, keeps its two-corner hint.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn placing_a_part_without_a_symbol_says_it_is_drawn_as_a_box() {
    let tmp = Tmp::new("box-part-hint");
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-PSU-1", "template": 0}),
    );
    assert_eq!(s.tool().name(), "black_box");
    assert!(
        s.tool_hint()
            .starts_with("This part has no symbol of its own and is drawn as a box: click its first corner"),
        "{}",
        s.tool_hint()
    );
    done(&mut s, "ui.dismiss", json!({}));
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "EW-GEN-PSU-1", "template": 0}),
    );
    assert!(
        s.tool_hint().starts_with("Click the box's first corner"),
        "{}",
        s.tool_hint()
    );
}
