// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Undo of a drawing setting names the setting.
//! On the sample's 101.

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn undo_of_print_wire_numbers_names_it() {
    let tmp = Tmp::new("undo-names-setting");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "project.wire_numbers", json!({"on": true}));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(toast(&s), "Undone: Print wire numbers");
}
