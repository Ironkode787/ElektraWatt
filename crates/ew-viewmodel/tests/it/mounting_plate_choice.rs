// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mounting plate… opens the page's Surface choice whatever the Properties pane held.
//! The sample's layout 902.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn mounting_plate_opens_the_surface_choice_under_a_filter() {
    let tmp = Tmp::new("mounting-plate-choice");
    let mut s = tmp.sample_in(Mode::Headless, true, "902");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.filter", json!({"text": "scale"}));
    done(&mut s, "layout.choose_plate", json!({}));
    assert_eq!(
        s.properties().filter(),
        "",
        "the filter that hid the row is cleared"
    );
    assert_eq!(
        s.properties().edit().map(|e| e.row.as_str()),
        Some("page/derived.surface"),
        "the Surface row is being chosen"
    );
}
