// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! ID-1462 (FIXIP53): an automatically named interruption point takes the first free name among the project's own
//! interruption points, read from the model when it is named. A reopen (the session's memory gone), an undo that brings
//! a point back and a paste of a point never make the next automatic name repeat one, so no automatic name joins two
//! points: the point of the other page prints no cross-reference to them. Golden A's folios 102 and 103, on copies.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::ew_api::output::PageTables;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// Moves the grid cursor to the grid index `k` of the page on show.
fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let page = std::sync::Arc::clone(s.canvas().unwrap().page());
    let p = (page.snapper.page_of_index(k.0), page.snapper.page_of_index(k.1));
    done(s, "cursor.set", at(page.snapper.page_to_sheet(p), json!({})));
}

/// The links of the page on show whose target is on `folio` (a paired interruption point prints one to its partner).
fn links_to(s: &ShellModel, folio: &str) -> usize {
    s.canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .filter(|l| l.target.as_ref().is_some_and(|t| t.folio == folio))
        .count()
}

/// The representation ID and name of the interruption point of the page on show that `before` does not hold.
fn new_ip(before: &PageTables, s: &ShellModel) -> (String, String) {
    let new: Vec<(String, String)> = tables(s)
        .reps
        .iter()
        .filter(|r| !before.reps.iter().any(|b| b.id == r.id))
        .filter_map(|r| r.ip.as_ref().map(|ip| (r.id.to_string(), ip.name.clone())))
        .collect();
    assert_eq!(new.len(), 1, "{new:?}");
    new[0].clone()
}

/// The IP tool's click at the grid index `k` with the direction `direction` chosen: the new point's representation and
/// name. The tool stays armed.
fn place_ip(s: &mut ShellModel, k: (i64, i64), direction: &str) -> (String, String) {
    if s.tool().name() != "ip" {
        done(s, "tool.ip", json!({}));
    }
    done(s, "tool.ip_direction", json!({ "direction": direction }));
    let before = tables(s);
    cursor_k(s, k);
    done(s, "canvas.activate", json!({}));
    new_ip(&before, s)
}

/// Place, save, close, reopen and place: the session's memory is gone, the model is not, so the point on 103 is not
/// named "IP" again and does not become the partner of 102's source "IP".
#[test]
fn a_point_placed_after_a_reopen_takes_a_free_name_and_joins_nothing() {
    let tmp = Tmp::new("ip-free-reopen");
    let mut s = shell(&tmp, "102");
    assert_eq!(place_ip(&mut s, (46, 40), "source").1, "IP");
    assert_eq!(place_ip(&mut s, (50, 40), "source").1, "IP-1");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "file.save", json!({}));
    done(&mut s, "file.close", json!({}));
    let dir = tmp.golden_copy();
    done(&mut s, "file.open", json!({"dir": dir.to_string_lossy()}));
    done(&mut s, "page.goto", json!({"folio": "103"}));
    let links = links_to(&s, "102");
    assert_eq!(place_ip(&mut s, (46, 40), "target").1, "IP-2");
    assert_eq!(
        links_to(&s, "102"),
        links,
        "the new target pairs with no point of 102"
    );
    s.shutdown();
}

/// An undo brings back a point the session's memory no longer names: the next automatic name skips it.
#[test]
fn a_point_placed_after_an_undo_brings_one_back_takes_a_free_name() {
    let tmp = Tmp::new("ip-free-undo");
    let mut s = shell(&tmp, "102");
    let (rep, name) = place_ip(&mut s, (46, 40), "source");
    assert_eq!(name, "IP");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "select.set", json!({ "ids": [rep] }));
    done(&mut s, "edit.delete_selection", json!({}));
    // On 103 "IP" is free now; its undo frees nothing else.
    done(&mut s, "page.goto", json!({"folio": "103"}));
    assert_eq!(place_ip(&mut s, (46, 40), "target").1, "IP");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "edit.undo", json!({}));
    // The delete undone (the undo shows 102): its source "IP" is back.
    done(&mut s, "edit.undo", json!({}));
    assert!(
        tables(&s)
            .reps
            .iter()
            .any(|r| r.ip.as_ref().is_some_and(|ip| ip.name == "IP"))
    );
    done(&mut s, "page.goto", json!({"folio": "103"}));
    let links = links_to(&s, "102");
    assert_eq!(place_ip(&mut s, (46, 40), "target").1, "IP-1");
    assert_eq!(
        links_to(&s, "102"),
        links,
        "the new target pairs with no point of 102"
    );
    s.shutdown();
}

/// A pasted point holds its name: the next automatic name on its page skips it, whatever its direction.
#[test]
fn a_point_placed_after_a_paste_takes_a_free_name() {
    let tmp = Tmp::new("ip-free-paste");
    let mut s = shell(&tmp, "102");
    let (rep, name) = place_ip(&mut s, (46, 40), "source");
    assert_eq!(name, "IP");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "select.set", json!({ "ids": [rep] }));
    done(&mut s, "edit.cut", json!({}));
    done(&mut s, "page.goto", json!({"folio": "103"}));
    done(&mut s, "edit.paste_clip", json!({}));
    let before = tables(&s);
    cursor_k(&mut s, (46, 40));
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(new_ip(&before, &s).1, "IP", "the pasted point keeps its name");
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(place_ip(&mut s, (50, 40), "target").1, "IP-1");
    s.shutdown();
}

/// A copied and pasted point keeps its name, an explicit act like a typed name, so it continues the
/// points of that name; the paste's toast says which, whole, so nothing joins silently.
#[test]
fn a_pasted_point_keeps_its_name_and_says_which_point_it_continues() {
    let tmp = Tmp::new("ip-paste-continues");
    let mut s = shell(&tmp, "102");
    assert_eq!(place_ip(&mut s, (46, 40), "source").1, "IP");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "page.goto", json!({"folio": "103"}));
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.name", json!({"text": "IP"}));
    let (target, name) = place_ip(&mut s, (46, 40), "target");
    assert_eq!(name, "IP", "typed");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "select.set", json!({ "ids": [target] }));
    done(&mut s, "edit.copy", json!({}));
    done(&mut s, "edit.paste_clip", json!({}));
    let before = tables(&s);
    cursor_k(&mut s, (60, 40));
    done(&mut s, "canvas.activate", json!({}));
    let (pasted, name) = new_ip(&before, &s);
    assert_eq!(name, "IP", "the pasted point keeps its name");
    let continuation = tables(&s)
        .reps
        .iter()
        .find(|r| r.id.to_string() == pasted)
        .and_then(|r| r.ip.as_ref())
        .map(|ip| ip.continuation.clone())
        .expect("the pasted point");
    assert!(continuation.starts_with("102"), "{continuation}");
    assert_eq!(
        s.toast().map(|n| n.lines().0.to_owned()),
        Some(format!(
            "The pasted interruption point IP keeps its name, so it continues IP at {continuation}."
        ))
    );
    s.shutdown();
}
