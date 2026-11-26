// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! At shell level: Output ▸ Workshop ▸ Wire list (doc 13 §12).

use crate::common;
use ew_viewmodel::key;

use common::{Tmp, done, refused};
use ew_viewmodel::actions;
use ew_viewmodel::command_bar::Group;
use ew_viewmodel::shell::Mode;
use serde_json::json;

/// On the sample, Wire list on the Output tab's Workshop group opens the first page of the wire list (the
/// `wire_list` block at folio 907) and says so.
#[test]
fn wire_list_opens_the_first_page_of_the_wire_list() {
    let tmp = Tmp::new("wire-list-open");
    let mut shell = tmp.sample_shell("102");
    let meta = actions::find("lists.wires").expect("registered");
    assert_eq!(meta.placement.map(|p| p.group), Some(Group::Workshop));
    done(&mut shell, "lists.wires", json!({}));
    let row = shell
        .current_page()
        .and_then(|p| shell.row_index(p))
        .map(|i| shell.page_rows()[i].clone())
        .expect("a page is shown");
    assert_eq!(row.folio, "907");
    let toasts: Vec<String> = shell
        .notifications()
        .visible()
        .iter()
        .map(|n| n.text.clone())
        .collect();
    assert!(
        toasts.iter().any(|t| t == "Opened the wire list on page 907."),
        "{toasts:?}"
    );
}

/// A project without a wire list: the window opens Report block… on that kind; headless there is no form, and the
/// command is refused in words.
#[test]
fn wire_list_without_a_wire_list_is_refused_in_words_headless() {
    let tmp = Tmp::new("wire-list-none");
    let mut shell = tmp.empty_shell(Mode::Headless);
    crate::common::make_project(&mut shell, &tmp.0.join("new.ew"), &[]);
    done(
        &mut shell,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    assert_eq!(
        refused(&mut shell, "lists.wires", json!({})),
        "reason-no-wire-list"
    );
    assert_eq!(
        ew_viewmodel::strings::text(key!("reason-no-wire-list")),
        "The project has no wire list yet: Report… on the Output tab makes one"
    );
}
