// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire drawn that brings an error finding says so at once, in the finding's words, with Undo.
//! On a copy of the sample, its power supply on 101.

use crate::common;

use common::{Tmp, click_at, done, tables};
use ew_viewmodel::notifications::NoteTone;
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_wire_that_makes_an_error_says_so() {
    let tmp = Tmp::new("wire-error-toast");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    let t = tables(&s);
    let pin = |label: &str| {
        t.cps
            .iter()
            .find(|c| c.label == label)
            .map(|c| c.at)
            .unwrap_or_else(|| panic!("no pin {label}"))
    };
    let (l, n) = (pin("-101G1:L"), pin("-101G1:N"));
    done(&mut s, "tool.wire", json!({}));
    click_at(&mut s, l);
    click_at(&mut s, n);
    let toast = s.toast().cloned().expect("a toast");
    assert!(
        toast.text.starts_with("The wire makes an error: ") && !toast.text.ends_with(".."),
        "{}",
        toast.text
    );
    assert_eq!(toast.tone(), NoteTone::Error);
    assert_eq!(toast.action(), Some("undo"));
}
