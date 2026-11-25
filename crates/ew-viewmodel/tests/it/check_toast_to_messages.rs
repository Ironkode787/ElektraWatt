// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The check's toast said what it found and had no way to the Messages list. A check that finds something
//! offers Show messages, which opens the list.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_checks_toast_opens_the_messages_list() {
    let tmp = Tmp::new("check-toast");
    // The sample's check finds infos (tubes crossing locations, frozen tags off their scheme).
    let mut s = tmp.sample_in(Mode::Headless, false, "101");
    done(&mut s, "messages.check", json!({}));
    let toast = s.toast().cloned().expect("the check's toast");
    assert_eq!(toast.actions(), ["show_messages"]);
    assert_eq!(
        ew_viewmodel::strings::text(ew_viewmodel::key!("note-action-show-messages")),
        "Show messages"
    );
    assert!(!s.navigator_open(NavKind::Messages));
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "show_messages"}),
    );
    assert!(s.navigator_open(NavKind::Messages));
}
