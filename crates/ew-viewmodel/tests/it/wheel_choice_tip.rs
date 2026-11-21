// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1830: a person who expects the mouse wheel to pan met a wheel that zooms, and only the zoom's tooltip said that
//! View ▸ Mouse wheel scrolls makes it pan. The plain wheel's first zoom offers that choice where it is met, once.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::notifications::NoteAction;
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_wheels_first_zoom_offers_the_wheel_that_scrolls() {
    let tmp = Tmp::new("wheel-tip");
    let mut s = tmp.sample_in(Mode::Headless, false, "101");
    assert!(!s.settings().wheel_scrolls);
    // A zoom by a button or a key offers nothing.
    done(&mut s, "view.zoom_to", json!({"percent": 150}));
    assert!(
        s.toast()
            .is_none_or(|t| t.actions() != [NoteAction::WheelScrolls])
    );
    done(&mut s, "view.zoom_to", json!({"percent": 175, "wheel": true}));
    let toast = s.toast().cloned().expect("the tip");
    assert_eq!(toast.actions(), [NoteAction::WheelScrolls]);
    done(
        &mut s,
        "notifications.run",
        json!({"entry": toast.serial, "action": "wheel_scrolls"}),
    );
    assert!(s.settings().wheel_scrolls, "the wheel scrolls now");
    // Once: back to zooming, the wheel's next zoom offers it no more.
    done(&mut s, "view.wheel_scrolls", json!({"on": false}));
    done(&mut s, "view.zoom_to", json!({"percent": 200, "wheel": true}));
    assert!(
        s.toast()
            .is_none_or(|t| t.actions() != [NoteAction::WheelScrolls])
    );
}
