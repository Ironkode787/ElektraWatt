// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The folio in front is a page's or none: with a station's view in front it is absent (it stood for the page
//! behind the view, and for the first page where none was found), the view keeps the page it stands on, and a page
//! action off a page refuses in words.

use crate::common::{Tmp, done, refused};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn off_a_page_there_is_no_folio_in_front_and_a_page_action_says_so() {
    let tmp = Tmp::new("folio-in-front");
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    assert_eq!(s.current_folio(), Some("102"));
    assert_eq!(s.station_page_folio(), None, "no station's view in front");
    done(&mut s, "plc.rack_editor", json!({"station": "tag:-104A1"}));
    assert_eq!(s.current_folio(), None, "a station's view shows no page");
    assert_eq!(
        s.station_page_folio(),
        Some("102"),
        "it stands on the page it was opened from"
    );
    assert_eq!(
        refused(&mut s, "page.describe", json!({})),
        "reason-no-page",
        "no page to describe in words"
    );
    done(&mut s, "page.describe", json!({"folio": "102"}));
    s.shutdown();
}
