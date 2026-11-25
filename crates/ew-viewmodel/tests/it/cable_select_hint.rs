// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cable… with nothing selected says what to select:
//! the wires that become the cable's cores. On the sample's 101.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::{key, strings};
use serde_json::json;

#[test]
fn cable_with_nothing_selected_says_to_select_the_wires() {
    let tmp = Tmp::new("cable-select-hint");
    let mut s = tmp.sample_in(ew_viewmodel::shell::Mode::Headless, true, "101");
    done(&mut s, "select.set", json!({"ids": []}));
    assert_eq!(
        refused(&mut s, "cable.from_wires", json!({})),
        "reason-cable-select-wires"
    );
    assert_eq!(
        strings::text(key!("reason-cable-select-wires")),
        "Select the wires that become the cable's cores"
    );
}
