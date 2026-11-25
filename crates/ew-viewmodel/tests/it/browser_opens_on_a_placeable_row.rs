// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Insert symbol on a schematic page opened on the bus connector, a row greyed there (it goes on a bus
//! page), so Enter placed nothing. The browser's current row is the first that can be placed.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_symbol_browser_opens_on_a_row_that_can_be_placed() {
    let tmp = Tmp::new("browser-first-row");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "tool.place", json!({}));
    let p = s.picker().expect("the browser");
    let first = *p.rows.first().expect("rows");
    assert!(
        p.disabled(first).is_some(),
        "the sample's schematic page lists a greyed symbol first (the fixture this guards)"
    );
    assert!(p.disabled(p.rows[p.row]).is_none(), "row {} is greyed", p.row);
}
