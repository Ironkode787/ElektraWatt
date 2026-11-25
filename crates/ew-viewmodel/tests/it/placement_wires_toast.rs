// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A placement says which wires it made, and how to place without them, and that toast stays up while the next
//! placements come.
//! On a page of a new project of the test's own.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The texts of the visible toasts, oldest first.
fn toasts(s: &ShellModel) -> Vec<(String, &'static str)> {
    s.notifications()
        .visible()
        .iter()
        .map(|n| (n.text.clone(), n.key))
        .collect()
}

/// Places a normally open contact at `cell`, without wires when `alt`.
fn contact(s: &mut ShellModel, cell: &str, alt: bool) {
    done(s, "tool.place", json!({}));
    done(
        s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    done(s, "nav.goto", json!({ "text": cell }));
    done(s, "canvas.activate", json!({ "alt": alt }));
    done(s, "ui.dismiss", json!({}));
}

#[test]
fn a_placement_names_its_wires_and_its_toast_stays_up() {
    let tmp = Tmp::new("placement-wires");
    let mut s = common::new_project(&tmp);
    contact(&mut s, "101-C4", true);
    contact(&mut s, "101-E4", false);
    let wired: Vec<String> = toasts(&s)
        .into_iter()
        .filter(|(_, k)| *k == "wired")
        .map(|(t, _)| t)
        .collect();
    assert_eq!(
        wired,
        [
            "Placed -101K2 at 101-E4 and connected 13 to -101K1:14. Alt+Click or Ctrl+Enter places without connecting."
        ]
    );
    // A placement without wires leaves it up.
    contact(&mut s, "101-C8", true);
    assert!(
        toasts(&s)
            .iter()
            .any(|(t, _)| t.contains("connected 13 to -101K1:14")),
        "{:?}",
        toasts(&s)
    );
    // In Dutch.
    done(&mut s, "view.language", json!({"value": "nl"}));
    contact(&mut s, "101-E8", false);
    assert!(
        toasts(&s).iter().any(|(t, _)| t.ends_with(
            "-101K4 geplaatst op 101-E8 en 13 verbonden met -101K3:14. Alt+klik of Ctrl+Enter plaatst zonder te \
             verbinden."
        )),
        "{:?}",
        toasts(&s)
    );
    s.shutdown();
}

/// A Terminal… row says the wires its terminals made, though it names its last terminal.
#[test]
fn a_terminal_row_names_the_wires_of_its_terminals() {
    let tmp = Tmp::new("placement-wires-row");
    let mut s = common::new_project(&tmp);
    contact(&mut s, "101-C4", true);
    done(&mut s, "tool.place_terminal", json!({"strip": "new", "count": 3}));
    done(&mut s, "nav.goto", json!({ "text": "101-E4" }));
    done(&mut s, "canvas.activate", json!({}));
    let said = toasts(&s);
    assert!(
        said.iter()
            .any(|(t, k)| *k == "wired" && t.contains(" terminal 1 to -101K1:14.") && !t.contains('?')),
        "{said:?}"
    );
    s.shutdown();
}
