// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Add hole here…'s Size says its unit and an example for the kind chosen, under the field.

use crate::common::{Tmp, done, form};

use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_size_of_a_hole_says_its_unit_and_an_example() {
    let tmp = Tmp::new("hole-hint");
    let mut s = tmp.sample_in(Mode::Gui, true, "904");
    done(&mut s, "layout.feature_add", json!({}));
    assert_eq!(
        form(&s).note("size"),
        Some("A screw from M3 to M8, such as M6"),
        "a fixing first"
    );
    s.set_form_field("kind", "round");
    assert_eq!(
        form(&s).note("size"),
        Some("A diameter in millimetres, such as 8.5")
    );
    s.shutdown();
}
