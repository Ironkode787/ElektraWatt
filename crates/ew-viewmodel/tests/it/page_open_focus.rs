// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opening a page from the Pages list (Enter, a double click) puts the keyboard focus on the page, so that a tool's key
//! works at once: a persona pressed W after opening a page and the list, which kept the focus, refused it.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::navigator::RowKind;
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::{Focus, Mode};
use serde_json::json;

#[test]
fn a_page_opened_from_the_list_takes_the_focus() {
    let tmp = Tmp::new("page-open-focus");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "ui.focus", json!({"area": "navigator", "kind": "pages"}));
    assert_eq!(s.focus(), Some(Focus::Navigator(NavKind::Pages)));
    let key = s
        .navigator_rows()
        .into_iter()
        .find(|r| matches!(r.kind, RowKind::Page(i) if s.page_rows()[i].folio == "102"))
        .expect("the row of 102")
        .key;
    done(&mut s, "navigator.open", json!({"kind": "pages", "key": key}));
    assert_eq!(s.current_folio(), Some("102"));
    assert!(
        matches!(s.focus(), Some(Focus::Canvas(_))),
        "the page has the focus: {:?}",
        s.focus()
    );
    // W arms the Wire tool there.
    done(&mut s, "tool.wire", json!({}));
    assert_eq!(s.tool().name(), "wire");
}
