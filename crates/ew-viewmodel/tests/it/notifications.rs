// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! JB6 (WP-S2.09 part 1, D6, §6.5; FR-UI-042): after a direct grid commit on 20 devices the toast's Undo still works
//! from the notifications log once the toast is gone, and is expired after another commit; toasts of one key replace
//! each other. The log's caps (three visible, 100 kept) are the unit tests of `ew_viewmodel::notifications`.

use crate::common;

use common::{Tmp, done, refused, state};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

fn terminals() -> Vec<String> {
    (1..=20).map(|i| format!("tag:-X1031:{i}")).collect()
}

fn qty(shell: &ShellModel) -> String {
    let sheet = shell.properties().sheet().expect("a sheet");
    let (_, row) = find_row(sheet, "device/parts[0].qty").expect("the quantity row");
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    }
}

#[test]
fn jb6_a_toasts_undo_works_from_the_log_and_expires_after_another_commit() {
    let tmp = Tmp::new("jb6-grid");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut s, "select.set", json!({"refs": terminals()}));
    let before = qty(&s);
    done(
        &mut s,
        "props.set",
        json!({"row": "device/parts[0].qty", "value": "2"}),
    );
    assert_eq!(qty(&s), "2");
    let toast = s.toast().expect("the toast of a commit on 20 devices");
    assert_eq!(toast.text, "Set quantity on 20 devices");
    assert!(s.describe().contains("toast \"Set quantity on 20 devices\" undo"));
    assert_eq!(state(&s)["toast"]["undo"], true);
    // The toast goes (the window's 8 seconds, or Close); the log keeps it with its Undo.
    done(&mut s, "toast.dismiss", json!({}));
    assert!(s.toast().is_none());
    done(&mut s, "notifications.show", json!({}));
    let d = s.describe();
    assert!(
        d.contains("notifications 1\n  notification 1 \"Set quantity on 20 devices\" undo"),
        "{d}"
    );
    done(&mut s, "notifications.run", json!({"entry": 1, "action": "undo"}));
    assert_eq!(qty(&s), before, "the log's Undo undid the step");
    assert!(
        s.describe()
            .contains("  notification 1 \"Set quantity on 20 devices\" undo expired"),
        "spent once it ran"
    );
    // Two more commits: the newer toast replaces the older (one key), and the older one's Undo expires.
    done(
        &mut s,
        "props.set",
        json!({"row": "device/parts[0].qty", "value": "3"}),
    );
    done(
        &mut s,
        "props.set",
        json!({"row": "device/parts[0].qty", "value": "4"}),
    );
    assert_eq!(s.notifications().visible().len(), 1, "one key, one toast");
    let d = s.describe();
    assert!(
        d.contains("  notification 3 \"Set quantity on 20 devices\" undo\n  notification 2 \"Set quantity on 20 devices\" undo expired"),
        "{d}"
    );
    assert_eq!(
        refused(&mut s, "notifications.run", json!({"entry": 2, "action": "undo"})),
        "reason-action-expired"
    );
    assert_eq!(qty(&s), "4", "an expired Undo changes nothing");
    // The newest one is valid, through the toast too.
    done(&mut s, "toast.undo", json!({}));
    assert_eq!(qty(&s), "3");
    assert!(s.describe().contains("status bar: "));
    assert!(
        state(&s)["status"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "Notifications 3"),
        "the status bar counts the log"
    );
    s.shutdown();
}

#[test]
fn jb6_the_toasts_of_a_closed_project_leave_the_screen_and_their_undo_expires() {
    let tmp = Tmp::new("jb6-close");
    let mut s = tmp.shell_in(Mode::Headless, true, "102");
    done(&mut s, "select.set", json!({"refs": terminals()}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/parts[0].qty", "value": "2"}),
    );
    done(&mut s, "file.close", json!({"discard": true}));
    assert!(s.toast().is_none());
    let entry = s.notifications().get(1).cloned().expect("the log keeps it");
    assert!(!s.notification_valid(&entry));
    assert_eq!(
        refused(&mut s, "notifications.run", json!({"entry": 1, "action": "undo"})),
        "reason-action-expired"
    );
    s.shutdown();
}
