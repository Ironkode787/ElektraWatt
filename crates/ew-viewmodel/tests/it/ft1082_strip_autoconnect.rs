// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1082 (decision log ID-1177): autoconnect never joins two terminals of one strip. In a play-test, terminal 2 of
//! a strip placed in line under terminal 1 was connected to it, shorting the two feed terminals; two terminals of a
//! strip are joined by a bridge, never by a wire autoconnect proposes. A terminal of another strip, in the same
//! geometry, is proposed and connected as before; a move of the terminal into that line proposes nothing either. The
//! geometry is the play-test's (FT-705's fixture): a switch at grid index (60, 20) on a writable copy of Golden A's
//! folio 103, terminal 70 of -X1031 at (60, 28), the second terminal at (60, 34).

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::ew_api::output::{PageTables, TableEnd};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The sheet point of grid index `k` on the shown page.
fn sheet_of(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// Picks `pick`, puts the grid cursor at grid index `k` and commits with `activate` (`canvas.activate` with
/// autoconnect, `canvas.activate_alt` without); returns the status hint before the commit.
fn place(s: &mut ShellModel, pick: serde_json::Value, k: (i64, i64), activate: &str) -> String {
    done(s, "place.pick", pick);
    let q = sheet_of(s, k);
    done(s, "cursor.set", at(q, json!({})));
    let hint = s.tool_hint();
    done(s, activate, json!({}));
    done(s, "tool.select", json!({}));
    hint
}

fn terminal(strip: &str, designation: &str) -> serde_json::Value {
    json!({"symbol": "ew.iec.terminal@qet10", "strip": strip, "terminal_designation": designation})
}

/// The page's connections whose ends are both connection points of the strips -X1031 or -X1032 or the switch, as
/// their end labels, each pair sorted, the list sorted.
fn connections(s: &ShellModel) -> Vec<[String; 2]> {
    let t = tables(s);
    let label = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone())
            .unwrap_or_default(),
        TableEnd::Junction { .. } => String::new(),
    };
    let ours = |l: &str| l.starts_with("-X1031:") || l.starts_with("-X1032:") || l.starts_with("-103S1:");
    let mut out: Vec<[String; 2]> = t
        .connections
        .iter()
        .map(|c| {
            let mut e = [label(&c.a), label(&c.b)];
            e.sort();
            e
        })
        .filter(|e| ours(&e[0]) && ours(&e[1]))
        .collect();
    out.sort();
    out
}

fn pair(a: &str, b: &str) -> [String; 2] {
    [a.to_owned(), b.to_owned()]
}

/// A shell at actual size on folio 103 with a switch at (60, 20) and terminal 70 of -X1031 at (60, 28) placed under
/// it with autoconnect, so that its upper CP is connected to the switch and its lower CP is free.
fn feed(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    place(
        &mut s,
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
        (60, 20),
        "canvas.activate_alt",
    );
    place(&mut s, terminal("-X1031", "70"), (60, 28), "canvas.activate");
    assert_eq!(
        connections(&s),
        [pair("-103S1:14", "-X1031:70/b")],
        "terminal 1 is connected to the switch"
    );
    s
}

fn toast(s: &ShellModel) -> String {
    s.toast().expect("the placement's toast").text.clone()
}

#[test]
fn ft1082_a_terminal_placed_under_one_of_its_strip_is_not_connected_to_it() {
    let tmp = Tmp::new("ft1082-same");
    let mut s = feed(&tmp);
    let rev = s.revision();
    let hint = place(&mut s, terminal("-X1031", "71"), (60, 34), "canvas.activate");
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(
        hint, "Click or Enter to place; R, X, Y and V turn it; Escape ends",
        "no proposal: the place hint without them"
    );
    assert_eq!(
        connections(&s),
        [pair("-103S1:14", "-X1031:70/b")],
        "the placement made no connection"
    );
    assert_eq!(toast(&s), "Placed -X1031 terminal 71 at 103-E9");
}

#[test]
fn ft1082_a_terminal_of_another_strip_in_that_line_is_proposed_and_connected() {
    let tmp = Tmp::new("ft1082-other");
    let mut s = feed(&tmp);
    let hint = place(&mut s, terminal("-X1032", "90"), (60, 34), "canvas.activate");
    assert_eq!(
        hint,
        "Click or Enter to place and connect the proposed wires; Alt+Click or Ctrl+Enter places without them; Tab tries \
         the next target; Escape ends"
    );
    assert_eq!(
        connections(&s),
        [
            pair("-103S1:14", "-X1031:70/b"),
            pair("-X1031:70/a", "-X1032:90/b"),
        ]
    );
    assert_eq!(
        toast(&s),
        "Placed -X1032 terminal 90 at 103-E9 and connected 1 wire: \"2\" -> \"-X1031:70/a\". Alt+Click or Ctrl+Enter \
         places without connecting"
    );
}

/// The `canvas proposals` line of the describe text while a drag moves the terminal at `from` by `by` grid steps.
fn drag_proposals(s: &mut ShellModel, from: (i64, i64), by: (i64, i64)) -> Option<String> {
    let page = s.canvas().unwrap().page();
    let p = (
        page.snapper.page_of_index(from.0),
        page.snapper.page_of_index(from.1),
    );
    let r = page
        .tables
        .reps
        .iter()
        .find(|r| r.pos == p)
        .expect("the terminal")
        .clone();
    let g = page.snapper.page_of_index(1);
    let centre = ((r.body[0] + r.body[2]) / 2, (r.body[1] + r.body[3]) / 2);
    let sheet = |q: (i64, i64)| s.canvas().unwrap().page().snapper.page_to_sheet(q);
    let (c, step, to) = (
        sheet(centre),
        sheet((centre.0 + g, centre.1)),
        sheet((centre.0 + by.0 * g, centre.1 + by.1 * g)),
    );
    done(s, "canvas.press", at(c, json!({})));
    done(s, "canvas.pointer", at(step, json!({})));
    done(s, "canvas.pointer", at(to, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    let line = s
        .describe()
        .lines()
        .find(|l| l.starts_with("canvas proposals "))
        .map(str::to_owned);
    done(s, "ui.dismiss", json!({}));
    line
}

#[test]
fn ft1082_a_terminal_moved_under_one_of_its_strip_proposes_nothing() {
    let tmp = Tmp::new("ft1082-move");
    let mut s = feed(&tmp);
    // Terminal 71 of the strip and terminal 90 of -X1032 placed out of line, without autoconnect.
    place(&mut s, terminal("-X1031", "71"), (50, 40), "canvas.activate_alt");
    place(&mut s, terminal("-X1032", "90"), (70, 40), "canvas.activate_alt");
    assert_eq!(drag_proposals(&mut s, (50, 40), (10, -6)), None);
    // The other strip's terminal dragged to the same point is proposed (shown, never committed).
    assert_eq!(
        drag_proposals(&mut s, (70, 40), (-10, -6)).as_deref(),
        Some("canvas proposals 1 \"90/b\" -> \"-X1031:70/a\" connect none")
    );
}
