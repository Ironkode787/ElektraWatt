// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-705 (the cycle-38 rulings): a wire the Wire tool draws over two free connection points of one other
//! representation, where the run enters and leaves it, is drawn as two connections in one transaction (one undo
//! step), so a terminal drawn into a vertical circuit is in the circuit; a run over one free CP (a T) stays one
//! connection. The wires are drawn by the window's own path (pointer presses on the canvas, the bends fixed with Alt
//! on grid points, as a drafter does), on a writable copy of Golden A's folio 103 in an empty area. FT-708: a double
//! click on the pin a wire just ended on starts the next wire there.

mod common;

use common::{Tmp, at, done, state};
use ew_viewmodel::ew_api::output::{PageTables, TableConnection, TableCp, TableEnd};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The page point of grid index `k` on the shown page.
fn page_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// The CP with `label`.
fn cp(s: &ShellModel, label: &str) -> TableCp {
    tables(s)
        .cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no CP {label}"))
        .clone()
}

/// Places `pick` with its insertion point at grid index `k`, without autoconnect.
fn place(s: &mut ShellModel, pick: serde_json::Value, k: (i64, i64)) {
    done(s, "place.pick", pick);
    let p = page_point(s, k);
    let sheet = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", at(sheet, json!({})));
    done(s, "canvas.activate_alt", json!({}));
    done(s, "tool.select", json!({}));
}

/// A pointer click of the canvas at the page point `p` (`alt`: a bend on the grid point, the object snaps off) as the
/// window sends it (`canvas.rs`): the pointer there, the press, the pointer again with the hover cleared, the release.
fn click(s: &mut ShellModel, p: (i64, i64), alt: bool) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "canvas.pointer", at(q, json!({ "alt": alt })));
    done(s, "canvas.press", at(q, json!({ "alt": alt })));
    done(
        s,
        "canvas.pointer",
        at(q, json!({ "alt": alt, "clear_hover": true })),
    );
    done(s, "canvas.release", at(q, json!({ "alt": alt })));
}

/// A shell at actual size on folio 103 with a switch at grid index (60, 20), a terminal of -X1031 below it at
/// (60, 30) and a coil at `coil`, none of them wired.
fn circuit(tmp: &Tmp, coil: (i64, i64)) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    place(
        &mut s,
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
        (60, 20),
    );
    place(
        &mut s,
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "terminal_designation": "70"}),
        (60, 30),
    );
    place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        coil,
    );
    s
}

/// The labels of a connection's ends, in its order.
fn ends(t: &PageTables, c: &TableConnection) -> [String; 2] {
    let label = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone())
            .unwrap_or_default(),
        TableEnd::Junction { .. } => "@".to_owned(),
    };
    [label(&c.a), label(&c.b)]
}

fn new_connections(before: &PageTables, after: &PageTables) -> Vec<TableConnection> {
    after
        .connections
        .iter()
        .filter(|c| !before.connections.iter().any(|b| b.id == c.id))
        .cloned()
        .collect()
}

#[test]
fn a_vertical_run_through_a_terminal_draws_two_connections_in_one_undo_step() {
    let tmp = Tmp::new("ft705-pass");
    let mut s = circuit(&tmp, (60, 40));
    let (switch, top, bottom, coil) = (
        cp(&s, "-103S1:14"),
        cp(&s, "-X1031:70/b"),
        cp(&s, "-X1031:70/a"),
        cp(&s, "-103K1:A1"),
    );
    // The terminal stands in the straight line from the switch to the coil, its CPs two grid steps apart.
    assert!(
        [top.at.0, bottom.at.0, coil.at.0]
            .iter()
            .all(|x| *x == switch.at.0)
    );
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    assert_eq!(s.tool().state(), "routing");
    // The drafter's bends straight down over the terminal (the router alone would go round its body).
    for k in [29, 30, 31] {
        let p = page_point(&s, (60, k));
        click(&mut s, p, true);
    }
    // The preview is the very command: two connections in one.
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(coil.at);
    done(&mut s, "canvas.pointer", at(q, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    let preview = s.preview_state().expect("a preview");
    assert_eq!(preview.name, "connection.draw_many");
    assert!(preview.result.is_ok(), "{:?}", preview.result.as_ref().err());
    click(&mut s, coil.at, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(s.tool().state(), "armed");
    let after = tables(&s);
    let mut new: Vec<[String; 2]> = new_connections(&before, &after)
        .iter()
        .map(|c| ends(&after, c))
        .collect();
    new.sort();
    assert_eq!(
        new,
        [
            ["-103S1:14".to_owned(), "-X1031:70/b".to_owned()],
            ["-X1031:70/a".to_owned(), "-103K1:A1".to_owned()],
        ],
        "switch to the terminal's top, the terminal's bottom to the coil"
    );
    // Each route is straight from CP to CP.
    for c in new_connections(&before, &after) {
        let line = &c.polyline;
        assert!(line.iter().all(|p| p.0 == switch.at.0), "{line:?}");
    }
    // One undo step removes both.
    done(&mut s, "edit.undo", json!({}));
    assert!(
        new_connections(&before, &tables(&s)).is_empty(),
        "both connections are gone"
    );
    // The step before it, the coil's placement, is still there.
    assert!(tables(&s).cps.iter().any(|c| c.label == "-103K1:A1"));
    // Redo brings both back, again in one step.
    done(&mut s, "edit.redo", json!({}));
    assert_eq!(new_connections(&before, &tables(&s)).len(), 2);
}

#[test]
fn a_run_over_one_free_cp_of_another_symbol_stays_one_connection() {
    let tmp = Tmp::new("ft705-tee");
    // The coil stands four grid steps to the left, so the run leaves the line at the terminal's middle.
    let mut s = circuit(&tmp, (56, 40));
    let (switch, top, coil) = (cp(&s, "-103S1:14"), cp(&s, "-X1031:70/b"), cp(&s, "-103K1:A1"));
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    // Down over the terminal's top CP to its middle, then left and down to the coil.
    for k in [(60, 29), (60, 30), (56, 30)] {
        let p = page_point(&s, k);
        click(&mut s, p, true);
    }
    click(&mut s, coil.at, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let after = tables(&s);
    let new = new_connections(&before, &after);
    assert_eq!(new.len(), 1, "one connection");
    assert_eq!(
        ends(&after, &new[0]),
        ["-103S1:14".to_owned(), "-103K1:A1".to_owned()]
    );
    // The run passes over the top CP, which stays free, as does the bottom one.
    let on_run = new[0].polyline.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        a.0 == b.0 && a.0 == top.at.0 && (a.1.min(b.1)..=a.1.max(b.1)).contains(&top.at.1)
    });
    assert!(on_run, "{:?}", new[0].polyline);
    assert!(cp(&s, "-X1031:70/b").connections.is_empty());
    assert!(cp(&s, "-X1031:70/a").connections.is_empty());
}

/// A wire from `from` to `to` by pointer clicks as the window sends them.
fn wire(s: &mut ShellModel, from: (i64, i64), to: (i64, i64)) {
    click(s, from, false);
    click(s, to, false);
}

// FT-708: the window turns the second of two presses within 0.5 s and 4 points into `tool.finish` (a double click,
// WP-S2.04 §6.1). A click that ends a wire on a pin and a quick second one there to start the next wire are that
// double click, and the Wire tool ignored it: the second wire was silently not drawn. The model takes a second
// connection at a wired CP (`connection.draw` has no one-connection rule), so the wire is drawn.
#[test]
fn a_double_click_on_the_pin_a_wire_just_ended_on_starts_the_next_wire_there() {
    let tmp = Tmp::new("ft708-draw");
    let mut s = circuit(&tmp, (60, 40));
    let (switch, coil, coil_out) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"), cp(&s, "-103K1:A2"));
    done(&mut s, "tool.wire", json!({}));
    // The first wire, from the switch to the coil (round the terminal, as the router goes).
    wire(&mut s, switch.at, coil.at);
    assert_eq!(cp(&s, "-103K1:A1").connections.len(), 1);
    assert_eq!(s.tool().state(), "armed");
    // The second press on the coil's pin comes as `tool.finish`: it starts the next wire there.
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(
        s.tool().state(),
        "routing",
        "the next wire starts on the wired pin"
    );
    assert_eq!(state(&s)["canvas"]["route"]["from"], "-103K1:A1");
    let announced = s.tool_announcement().unwrap_or_default().to_owned();
    assert!(
        announced.starts_with("Wire: routing from -103K1:A1."),
        "{announced}"
    );
    let rev = s.revision();
    click(&mut s, coil_out.at, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(cp(&s, "-103K1:A1").connections.len(), 2, "two wires at the pin");
    assert_eq!(cp(&s, "-103K1:A2").connections.len(), 1);
    // While routing, a double click's second press is nothing: the first fixed the bend or the end.
    click(&mut s, switch.at, false);
    let rev = s.revision();
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "routing");
    assert_eq!(s.revision(), rev);
    // Armed on the empty grid, it is nothing either: a wire needs a start.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.wire", json!({}));
    let empty = page_point(&s, (80, 25));
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(empty);
    done(&mut s, "canvas.pointer", at(q, json!({})));
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "armed");
}
