// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Remove from my parts library… asks first (doc 16a §6.12): undo does not bring a part back into My parts,
//! so the part editor's button opens a confirmation whose Primary is Cancel; Cancel returns to the part editor and
//! leaves the part in My parts, Remove takes it out. The window's own path: clicks on a new project at 1920 × 1080.

use crate::own_parts::{edit_part, new_project, press};
use crate::support::Tmp;

use ew_viewmodel::shell::{Dialog, Outcome};
use serde_json::json;

/// Whether the person's parts library under the test's folder holds a part numbered `number`.
fn in_my_parts(tmp: &Tmp, number: &str) -> bool {
    let mut stack = vec![tmp.0.join("data")];
    let needle = format!("\"part_number\":\"{number}\"");
    while let Some(at) = stack.pop() {
        for e in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            if e.path().is_dir() {
                stack.push(e.path());
            } else if std::fs::read_to_string(e.path()).is_ok_and(|t| t.contains(&needle)) {
                return true;
            }
        }
    }
    false
}

#[test]
fn removing_a_part_from_my_parts_asks_and_cancel_keeps_it() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    for (action, params) in [
        ("parts.new", json!({})),
        (
            "parts.field",
            json!({"field": "manufacturer", "value": "Example"}),
        ),
        ("parts.field", json!({"field": "part_number", "value": "SW-6"})),
        ("parts.save", json!({})),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done, "{action}");
    }
    assert!(in_my_parts(&tmp, "SW-6"), "saved into My parts");
    edit_part(&mut d, "SW-6");
    press(&mut d, "Remove");
    assert!(
        matches!(d.shell_mut().dialog(), Some(Dialog::ConfirmRemovePart { number, .. }) if number == "SW-6"),
        "the button asks: {:?}",
        d.shell_mut().dialog()
    );
    press(&mut d, "Cancel");
    assert!(
        d.shell_mut().dialog().is_none()
            && d.shell_mut()
                .parts_place()
                .and_then(|p| p.sheet.as_ref())
                .is_some(),
        "Cancel returns to the part in Parts"
    );
    assert!(in_my_parts(&tmp, "SW-6"), "Cancel leaves the part in My parts");
    press(&mut d, "Remove");
    press(&mut d, "Remove");
    assert!(d.shell_mut().dialog().is_none(), "both give way");
    assert!(!in_my_parts(&tmp, "SW-6"), "Remove takes it out");
    d.shell_mut().shutdown();
}
