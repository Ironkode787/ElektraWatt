// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `strip.new` with the tag of a strip the project has already is refused in words, instead of making a second
//! strip of that tag.

use crate::common;

use common::{Tmp, failed};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_new_strip_with_the_tag_of_another_is_refused_naming_it() {
    let tmp = Tmp::new("strip-taken");
    // The sample has the strip -X1031 (its terminals on page 102).
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    let steps = s.recorded_commands().len();
    for typed in ["-X1031", "X1031"] {
        let message = failed(&mut s, "strip.new", json!({"tag": typed}));
        assert!(
            message.contains("-X1031") && message.contains("already"),
            "{typed}: {message}"
        );
    }
    assert_eq!(s.recorded_commands().len(), steps, "no strip is made");
}
