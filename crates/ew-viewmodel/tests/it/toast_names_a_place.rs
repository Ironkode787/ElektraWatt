// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A toast that names a pane is a way to it: "drag a device from Unplaced devices …" offers Unplaced devices, which
//! opens the navigator, in English and in Dutch.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_toast_that_names_unplaced_devices_opens_it() {
    let tmp = Tmp::new("toast-place");
    let mut s = tmp.sample_in(Mode::Gui, true, "902");
    for (language, title) in [("en", "Unplaced devices"), ("nl", "Ongeplaatste apparaten")] {
        done(&mut s, "view.language", json!({ "value": language }));
        // Symbol on a panel page: greyed, its reason names the place the devices are dragged from.
        s.explain_disabled("tool.place", &json!({}));
        let toast = s.toast().cloned().expect("the reason's toast");
        assert!(toast.text.contains(title), "{}", toast.text);
        assert_eq!(toast.label("unplaced"), title);
        assert!(!s.navigator_open(NavKind::Unplaced));
        done(
            &mut s,
            "notifications.run",
            json!({"entry": toast.serial, "action": "unplaced"}),
        );
        assert!(s.navigator_open(NavKind::Unplaced), "{language}");
        done(&mut s, "navigator.show", json!({"kind": "unplaced", "on": false}));
    }
    s.shutdown();
}
