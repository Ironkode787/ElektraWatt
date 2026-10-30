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
    // The first wire, from the switch to the coil (through the terminal in line, two connections in one step).
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

// The cycle-38 window fix (W38, rv-vis-2): the router of FT-706 reserves every other CP and its stub, so a wire drawn
// with two clicks from a contact to a coil went round a free terminal standing in line between them and FT-705's
// pass-through never saw it. The direct route through a free terminal in line is taken when nothing else is in its
// way; elsewhere the router is unchanged.

/// A shell like [`circuit`] with a terminal of -X1031 at each of `terminals` (grid index, designation).
fn circuit_with(tmp: &Tmp, terminals: &[((i64, i64), &str)], coil: (i64, i64)) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    place(
        &mut s,
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
        (60, 20),
    );
    for (k, designation) in terminals {
        place(
            &mut s,
            json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "terminal_designation": designation}),
            *k,
        );
    }
    place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        coil,
    );
    s
}

/// The new connections' end labels, sorted.
fn new_ends(before: &PageTables, s: &ShellModel) -> Vec<[String; 2]> {
    let after = tables(s);
    let mut new: Vec<[String; 2]> = new_connections(before, &after)
        .iter()
        .map(|c| ends(&after, c))
        .collect();
    new.sort();
    new
}

fn pair(a: &str, b: &str) -> [String; 2] {
    [a.to_owned(), b.to_owned()]
}

#[test]
fn two_clicks_through_a_terminal_in_line_connect_it_in_one_undo_step() {
    let tmp = Tmp::new("fx38c-inline");
    let mut s = circuit(&tmp, (60, 40));
    let (switch, coil) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"));
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    // Before the second click the preview is the very command: what the user sees is what commits.
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(coil.at);
    done(&mut s, "canvas.pointer", at(q, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    let preview = s.preview_state().expect("a preview");
    assert_eq!(preview.name, "connection.draw_many");
    assert!(preview.result.is_ok(), "{:?}", preview.result.as_ref().err());
    click(&mut s, coil.at, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(
        new_ends(&before, &s),
        [pair("-103S1:14", "-X1031:70/b"), pair("-X1031:70/a", "-103K1:A1")],
        "the contact to the terminal's top, the terminal's bottom to the coil"
    );
    for c in new_connections(&before, &tables(&s)) {
        assert!(
            c.polyline.iter().all(|p| p.0 == switch.at.0),
            "straight: {:?}",
            c.polyline
        );
    }
    // No open square: both of the terminal's CPs are connected.
    assert_eq!(cp(&s, "-X1031:70/b").connections.len(), 1);
    assert_eq!(cp(&s, "-X1031:70/a").connections.len(), 1);
    done(&mut s, "edit.undo", json!({}));
    assert!(new_ends(&before, &s).is_empty(), "one undo step removes both");
}

#[test]
fn a_terminal_one_step_off_the_line_is_routed_around() {
    let tmp = Tmp::new("fx38c-off");
    let mut s = circuit_with(&tmp, &[((61, 30), "70")], (60, 40));
    let (switch, coil) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"));
    let before = tables(&s);
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    click(&mut s, coil.at, false);
    assert_eq!(new_ends(&before, &s), [pair("-103S1:14", "-103K1:A1")]);
    assert!(cp(&s, "-X1031:70/b").connections.is_empty());
    assert!(cp(&s, "-X1031:70/a").connections.is_empty());
}

#[test]
fn two_terminals_in_line_are_both_passed() {
    let tmp = Tmp::new("fx38c-two");
    let mut s = circuit_with(&tmp, &[((60, 28), "70"), ((60, 34), "71")], (60, 44));
    let (switch, coil) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"));
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    click(&mut s, coil.at, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(
        new_ends(&before, &s),
        [
            pair("-103S1:14", "-X1031:70/b"),
            pair("-X1031:70/a", "-X1031:71/b"),
            pair("-X1031:71/a", "-103K1:A1"),
        ]
    );
}

#[test]
fn a_direct_route_on_another_wire_s_track_is_left_to_the_router() {
    let tmp = Tmp::new("fx38c-track");
    let mut s = circuit(&tmp, (60, 50));
    // Another wire with a run on the line between the terminal and the coil: from a second contact's 14 over
    // (66, 40), (60, 40), (60, 44), (54, 44) to a second coil's A1 (the bends fixed with Alt).
    place(
        &mut s,
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
        (66, 34),
    );
    place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        (54, 48),
    );
    let (s2, k2) = (cp(&s, "-103S2:14"), cp(&s, "-103K2:A1"));
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, s2.at, false);
    for k in [(66, 40), (60, 40), (60, 44), (54, 44)] {
        let p = page_point(&s, k);
        click(&mut s, p, true);
    }
    click(&mut s, k2.at, false);
    assert_eq!(cp(&s, "-103K2:A1").connections.len(), 1, "the other wire");
    let other = &tables(&s).connections;
    let on_line = other.iter().any(|c| {
        c.polyline
            .windows(2)
            .any(|w| w[0].0 == w[1].0 && w[0].0 == page_point(&s, (60, 0)).0)
    });
    assert!(on_line, "the other wire runs on the line");
    let (switch, coil) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"));
    let before = tables(&s);
    click(&mut s, switch.at, false);
    click(&mut s, coil.at, false);
    assert_eq!(
        new_ends(&before, &s),
        [pair("-103S1:14", "-103K1:A1")],
        "the router's route, round the terminal"
    );
    assert!(cp(&s, "-X1031:70/b").connections.is_empty());
    assert!(cp(&s, "-X1031:70/a").connections.is_empty());
}

// C39 (FT-734, FT-758): a black box's caption is an obstacle to the router, the through route of the cycle-38 window
// fix (FX38c) included. Beside the line the through route is still taken; standing on the line it is not.

/// Draws a MICO 4.10 black box with the grid corners `a` and `b` (it grows until its CPs fit) and returns the page
/// rectangles `[x0, y0, x1, y1]` of its texts left of its outline (up to its edge): its caption.
fn black_box_caption(s: &mut ShellModel, a: (i64, i64), b: (i64, i64)) -> Vec<[i64; 4]> {
    done(s, "tool.black_box", json!({"part": "MICO 4.10", "template": 0}));
    for k in [a, b] {
        let p = page_point(s, k);
        let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
        done(s, "cursor.set", at(q, json!({})));
        done(s, "canvas.dwell", json!({"ms": 80}));
        done(s, "canvas.activate", json!({}));
    }
    done(s, "tool.select", json!({}));
    let t = tables(s);
    let rep = t.reps.iter().find(|r| r.black_box.is_some()).expect("the box");
    let left = rep
        .black_box
        .as_ref()
        .unwrap()
        .outline
        .iter()
        .map(|p| p.0)
        .min()
        .unwrap();
    let page = s.canvas().unwrap().page();
    page.entities
        .iter()
        .filter(|(e, _)| e.0 == rep.id)
        .map(|(_, r)| {
            let p = page.snapper.sheet_to_page((r.min().x.0, r.min().y.0));
            let q = page.snapper.sheet_to_page((r.max().x.0, r.max().y.0));
            [p.0.min(q.0), p.1.min(q.1), p.0.max(q.0), p.1.max(q.1)]
        })
        .filter(|r| r[2] <= left)
        .collect()
}

#[test]
fn the_through_route_passes_a_black_box_whose_caption_stands_beside_the_line() {
    let tmp = Tmp::new("c39-box-beside");
    let mut s = circuit(&tmp, (60, 40));
    let caption = black_box_caption(&mut s, (68, 34), (72, 40));
    let (switch, coil) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"));
    let x = switch.at.0;
    let g = page_point(&s, (1, 0)).0 - page_point(&s, (0, 0)).0;
    // The case: the caption stands level with the run, within two grid steps of it, not on it.
    assert_eq!(caption.len(), 3, "tag, part number, manufacturer");
    assert!(
        caption
            .iter()
            .any(|r| r[1] < coil.at.1 && r[3] > switch.at.1 && r[0] - x < 2 * g),
        "beside the line: {caption:?}"
    );
    assert!(caption.iter().all(|r| r[0] > x), "{caption:?}");
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    click(&mut s, coil.at, false);
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(
        new_ends(&before, &s),
        [pair("-103S1:14", "-X1031:70/b"), pair("-X1031:70/a", "-103K1:A1")],
        "through the terminal in line"
    );
    for c in new_connections(&before, &tables(&s)) {
        assert!(c.polyline.iter().all(|p| p.0 == x), "straight: {:?}", c.polyline);
    }
    done(&mut s, "edit.undo", json!({}));
    assert!(new_ends(&before, &s).is_empty(), "one undo step removes both");
}

#[test]
fn a_black_box_caption_on_the_line_turns_the_through_route_away() {
    let tmp = Tmp::new("c39-box-on");
    let mut s = circuit(&tmp, (60, 40));
    let caption = black_box_caption(&mut s, (66, 36), (70, 42));
    let (switch, coil) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"));
    let x = switch.at.0;
    // The case: a caption line crosses the straight run between the terminal and the coil.
    assert!(
        caption
            .iter()
            .any(|r| r[0] < x && x < r[2] && r[1] < coil.at.1 && r[3] > cp(&s, "-X1031:70/a").at.1),
        "on the line: {caption:?}"
    );
    let before = tables(&s);
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    click(&mut s, coil.at, false);
    let new = new_connections(&before, &tables(&s));
    assert_eq!(
        new_ends(&before, &s),
        [pair("-103S1:14", "-103K1:A1")],
        "the router's route"
    );
    // It runs through no caption text.
    let line = &new[0].polyline;
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
        for r in &caption {
            assert!(
                !(x0 < r[2] && r[0] < x1.max(x0 + 1) && y0 < r[3] && r[1] < y1.max(y0 + 1)),
                "{line:?} through {r:?}"
            );
        }
    }
    assert!(cp(&s, "-X1031:70/b").connections.is_empty());
    assert!(cp(&s, "-X1031:70/a").connections.is_empty());
}

// FT-758: an interruption point one step inside the drawing area, leaving towards the sheet's edge (Golden A's
// folio 101 feeds), still turns on the area's edge: the frame's inward step gives way on that side.
#[test]
fn an_interruption_point_at_the_area_edge_still_routes() {
    let tmp = Tmp::new("c39-edge");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut s, "view.zoom_actual", json!({}));
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("the drawing area");
    let (from, to) = (cp(&s, "J-E-206-L"), cp(&s, "1X1:1/b"));
    let g = page_point(&s, (1, 0)).0 - page_point(&s, (0, 0)).0;
    // The case: one step inside the area's left edge, leaving left.
    assert_eq!(from.at.0, area[0] + g);
    let before = tables(&s);
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, from.at, false);
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(to.at);
    done(&mut s, "canvas.pointer", at(q, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert!(!s.tool_hint().contains("drawn direct"), "{}", s.tool_hint());
    click(&mut s, to.at, false);
    let new = new_connections(&before, &tables(&s));
    assert_eq!(new.len(), 1);
    let line = &new[0].polyline;
    assert!(line.iter().all(|p| p.0 >= area[0]), "{line:?}");
}

// FT-758: the frame outside the drawing area (the header bands, the title block and its fields) is an obstacle to
// the router, its edge grown inward by a grid step as a box's outline is.
#[test]
fn a_wire_keeps_out_of_the_title_block() {
    let tmp = Tmp::new("c39-frame");
    let mut s = circuit(&tmp, (60, 40));
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("the drawing area");
    // Folio 103's drawing area ends at grid row 66; a black box reaching down to a step above it leaves no way
    // round below it inside the area.
    assert_eq!(page_point(&s, (0, 66)).1, area[3]);
    black_box_caption(&mut s, (70, 54), (80, 65));
    place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        (64, 56),
    );
    place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        (88, 56),
    );
    let (a, b) = (cp(&s, "-103K2:A2"), cp(&s, "-103K3:A2"));
    let before = tables(&s);
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, a.at, false);
    click(&mut s, b.at, false);
    assert_eq!(new_ends(&before, &s), [pair("-103K2:A2", "-103K3:A2")]);
    let new = new_connections(&before, &tables(&s));
    let line = &new[0].polyline;
    assert!(
        line.iter()
            .all(|p| area[0] < p.0 && p.0 < area[2] && area[1] < p.1 && p.1 < area[3]),
        "inside the drawing area {area:?}: {line:?}"
    );
    assert!(
        line.iter().any(|p| p.1 < page_point(&s, (0, 54)).1),
        "over the box: {line:?}"
    );
}

/// FT-759 (rv-vis-2): a terminal dropped on a wire is split into it (normaliser 7, beside FX38c's through route, which
/// connects a terminal already in line), and the placement toast says that it is in the circuit.
#[test]
fn ft759_a_terminal_dropped_on_a_wire_says_it_is_in_the_circuit() {
    let tmp = Tmp::new("ft759-drop");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    place(
        &mut s,
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
        (60, 20),
    );
    place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        (60, 40),
    );
    let (switch, coil) = (cp(&s, "-103S1:14"), cp(&s, "-103K1:A1"));
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, switch.at, false);
    click(&mut s, coil.at, false);
    done(&mut s, "tool.select", json!({}));
    let wired = tables(&s);
    assert_eq!(
        wired
            .connections
            .iter()
            .filter(|c| ends(&wired, c) == ["-103S1:14".to_owned(), "-103K1:A1".to_owned()])
            .count(),
        1,
        "one wire from the switch to the coil"
    );
    place(
        &mut s,
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "terminal_designation": "71"}),
        (60, 30),
    );
    let after = tables(&s);
    let mut split: Vec<[String; 2]> = after
        .connections
        .iter()
        .map(|c| ends(&after, c))
        .filter(|e| e.iter().any(|l| l.starts_with("-X1031:71/")))
        .collect();
    split.sort();
    assert_eq!(
        split,
        [
            ["-103S1:14".to_owned(), "-X1031:71/b".to_owned()],
            ["-X1031:71/a".to_owned(), "-103K1:A1".to_owned()],
        ],
        "the wire is split at the terminal"
    );
    let toast = state(&s)["toast"]["text"].as_str().unwrap_or_default().to_owned();
    assert!(
        toast.starts_with("Placed -X1031 terminal 71")
            && toast.ends_with(", connected into the wire it was dropped on"),
        "{toast}"
    );
}
