// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Parts list pressed again says that it opened the set's list and made no second one.
//! The sample's 901.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_parts_list_button_says_it_opened_the_sets_list() {
    let tmp = Tmp::new("parts-list-again");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    let pages = s.page_rows().len();
    for _ in 0..2 {
        done(&mut s, "lists.parts", json!({}));
        let said = s
            .notifications()
            .entries()
            .map(|n| n.text.clone())
            .find(|t| t.starts_with("Opened the set's parts list"))
            .unwrap_or_default();
        assert_eq!(
            said,
            "Opened the set's parts list on page 901; it makes no second one. Report… on the Output tab adds another list."
        );
    }
    assert_eq!(s.page_rows().len(), pages, "no page was added");
}
