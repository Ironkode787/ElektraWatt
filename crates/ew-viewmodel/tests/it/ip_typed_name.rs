// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The play-test of cycle 40 (finding 5): the name typed in Tool options (`ip.name`) names the next interruption
//! point, on the IP tool's click and on the Wire tool's P, which takes it after a switch of tools; a placed point
//! takes it, so the one after is proposed again.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::ew_api::output::PageTables;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// The page point of grid index `k`.
fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

fn index_of(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.grid_index_of(p.0), sn.grid_index_of(p.1))
}

fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", at(q, json!({})));
}

/// The names of the interruption points of the page on show that `before` does not hold.
fn new_ip_names(before: &PageTables, s: &ShellModel) -> Vec<String> {
    tables(s)
        .reps
        .iter()
        .filter(|r| !before.reps.iter().any(|b| b.id == r.id))
        .filter_map(|r| r.ip.as_ref().map(|ip| ip.name.clone()))
        .collect()
}

/// Starts a wire at `-102F2:<cp>` and moves the grid cursor four steps down from it (an empty grid point).
fn route_down_from(s: &mut ShellModel, label: &str) {
    let start = tables(s)
        .cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("{label}"))
        .clone();
    done(s, "tool.wire", json!({}));
    cursor(s, start.at);
    done(s, "canvas.activate", json!({}));
    assert_eq!(s.tool().state(), "routing");
    let k = index_of(s, start.at);
    let end = grid_point(s, (k.0, k.1 + 4));
    cursor(s, end);
}

#[test]
fn a_name_typed_with_the_ip_tool_names_the_end_of_the_next_wire_ended_with_p() {
    let tmp = Tmp::new("typed-p");
    let mut s = shell(&tmp, "102");
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.name", json!({"text": "0V"}));
    let before = tables(&s);
    route_down_from(&mut s, "-102F2:14");
    done(&mut s, "tool.ip", json!({}));
    assert_eq!(new_ip_names(&before, &s), vec!["0V".to_owned()]);
    // The name is taken: the next wire's end is proposed again (from its start's potential or the last name's
    // series), never the typed name unchanged a second time on the same page.
    let before = tables(&s);
    route_down_from(&mut s, "-102F2:13");
    done(&mut s, "tool.ip", json!({}));
    let names = new_ip_names(&before, &s);
    assert_eq!(names.len(), 1);
    assert_ne!(names[0], "0V");
}

#[test]
fn a_name_typed_with_the_ip_tool_names_the_point_its_click_places() {
    let tmp = Tmp::new("typed-click");
    let mut s = shell(&tmp, "102");
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.name", json!({"text": "0V"}));
    // A switch of tools keeps the typed name for the IP tool armed again.
    done(&mut s, "tool.wire", json!({}));
    done(&mut s, "tool.ip", json!({}));
    let before = tables(&s);
    let k = (40, 40);
    let p = grid_point(&s, k);
    cursor(&mut s, p);
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(new_ip_names(&before, &s), vec!["0V".to_owned()]);
}
