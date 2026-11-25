// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The IP tool offers the partner of a point that waits for one: a target placed
//! on folio 101 of a new project, the tool armed again on folio 102 offers that name as a source, not a new name, and
//! the toast says what was made and where its partner is, never what D would have made.

use crate::common;

use common::{Tmp, at, done, new_project, tables, toast};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::tools::Ghost;
use serde_json::json;

/// Moves the grid cursor to the grid index `k` of the page on show.
fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let page = std::sync::Arc::clone(s.canvas().unwrap().page());
    let p = (page.snapper.page_of_index(k.0), page.snapper.page_of_index(k.1));
    done(s, "cursor.set", at(page.snapper.page_to_sheet(p), json!({})));
}

/// The ghost's name and direction.
fn ghost(s: &ShellModel) -> (String, &'static str) {
    match s.tool_ghost() {
        Some(Ghost::Ip { name, direction, .. }) => (name, direction),
        other => panic!("the IP tool's ghost: {other:?}"),
    }
}

#[test]
fn the_tool_offers_the_partner_of_a_waiting_point() {
    let tmp = Tmp::new("ip-partner-first");
    let mut s = new_project(&tmp);
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "tool.ip_direction", json!({"direction": "target"}));
    cursor_k(&mut s, (40, 30));
    let (name, direction) = ghost(&s);
    assert_eq!(direction, "target");
    done(&mut s, "canvas.activate", json!({}));
    assert!(
        toast(&s).starts_with(&format!("{name} at ")) && toast(&s).contains("has no source yet: place where"),
        "{}",
        toast(&s)
    );
    done(&mut s, "tool.select", json!({}));

    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "multi_line", "title": "Control"}),
    );
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "tool.ip", json!({}));
    cursor_k(&mut s, (40, 30));
    assert_eq!(ghost(&s), (name.clone(), "source"), "the waiting target's source");
    done(&mut s, "canvas.activate", json!({}));
    let placed = tables(&s)
        .reps
        .iter()
        .find_map(|r| r.ip.clone())
        .expect("the source on 102");
    assert_eq!(
        (
            placed.name.as_str(),
            ew_api::output::ip_direction_word(&placed.direction).as_str()
        ),
        (name.as_str(), "source")
    );
    let said = toast(&s);
    assert!(
        said.starts_with(&format!("{name} at "))
            && said.ends_with(&format!("goes on at {}.", placed.continuation)),
        "{said}"
    );
    assert!(
        !placed.continuation.is_empty(),
        "the pair prints its cross-reference"
    );
}
