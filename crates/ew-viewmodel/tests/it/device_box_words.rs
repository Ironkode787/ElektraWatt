// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device box says what it does with what a person typed (FT-1811): a pin's place between two steps of the pin
//! grid lands on a step and a note says where; a copy of a starter part says that it keeps the starter's part number.

use crate::common::{Tmp, actual_size, done, new_project};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::symbol_editor::DeviceBox;
use serde_json::json;

fn device_box(s: &ShellModel) -> &DeviceBox {
    s.symbol_editor()
        .expect("the editor")
        .device_box()
        .expect("the device box")
}

/// Whether a note of the session reads `text`.
fn noted(s: &ShellModel, text: &str) -> bool {
    s.notifications().entries().any(|n| n.text.contains(text))
}

#[test]
fn a_pin_typed_between_two_steps_says_where_it_stands() {
    let tmp = Tmp::new("device-box-pin-step");
    let mut s = actual_size(&tmp, "102");
    done(&mut s, "part.box", json!({"part": "EW-GEN-PSU-1"}));
    let pin = device_box(&s).pin_table()[3].idx;
    // On a step: no word.
    done(&mut s, "part.pin", json!({"pin": pin, "side": "right", "at": 8}));
    assert!(!noted(&s, "The pin stands at"));
    // 5 mm lies between the steps 4 and 8 mm of the 4 mm grid: the pin goes to 4 mm, and the note says so.
    done(&mut s, "part.pin", json!({"pin": pin, "side": "right", "at": 5}));
    assert!(
        noted(&s, "The pin stands at 4 mm: a pin stands on a step of 4 mm, clear of the corners, and 5 mm is not one."),
        "{:?}",
        s.notifications().entries().map(|n| n.text.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn a_copy_of_a_starter_part_says_it_keeps_the_part_number() {
    let tmp = Tmp::new("device-box-copy-number");
    let mut s = new_project(&tmp);
    done(&mut s, "symbol.start", json!({}));
    let (part, _, _) = s
        .box_parts()
        .iter()
        .find(|(_, number, _)| number == "EW-GEN-PSU-1")
        .cloned()
        .expect("the starter power supply is listed");
    done(&mut s, "part.box", json!({ "part": part }));
    let line = device_box(&s).copy_line().unwrap_or_default();
    assert!(
        line.contains("The copy keeps the part number EW-GEN-PSU-1 until you give it one of its own"),
        "{line}"
    );
}
