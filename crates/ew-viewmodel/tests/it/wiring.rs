// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 2 in the view model (acceptance checks WR2, WR4, WR5, WR7, the view-model part of WR9 and WR10): the
//! Wire tool by keyboard (CP, junction and segment ends, bends, Backspace, the committed route equal to the ghost),
//! FR-CON-006's drawing order, the autoconnect proposals of a placement and of a move, route edit (pointer and
//! keyboard) and the split of a routed wire on insert, the junction sequence editor, the crossings setting, the
//! transcript members and the describe lines with their 21 words, and the exact `Session` counts of the Wire tool.
//! Every test works on a writable temporary copy of Golden A.

use crate::common;
use ew_viewmodel::key;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::ew_api::output::{PageTables, TableConnection, TableCp, TableEnd};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::{MarkerKind, OverlayItem};
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`, at actual size (8 points are less than one grid step,
/// so a grid point next to a connection point is a point of its own).
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// The CP with `label` (the first in ID order).
fn cp(s: &ShellModel, label: &str) -> TableCp {
    tables(s)
        .cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no CP {label}"))
        .clone()
}

/// The sheet point of a page point.
fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The page point of grid index `k`.
fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The grid cursor to the page point `p`.
fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "cursor.set", at(q, json!({})));
}

/// The cursor to `p`, then Enter.
fn enter_at(s: &mut ShellModel, p: (i64, i64)) {
    cursor(s, p);
    done(s, "canvas.activate", json!({}));
}

fn canvas(s: &ShellModel) -> Value {
    state(s)["canvas"].clone()
}

fn describe_line(s: &ShellModel, prefix: &str) -> Option<String> {
    s.describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(str::to_owned)
}

/// The label of a connection end on the page (`@` for a junction).
fn end_label(t: &PageTables, e: &TableEnd) -> String {
    match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone())
            .unwrap_or_default(),
        TableEnd::Junction { .. } => "@".to_owned(),
    }
}

/// The connections that are new in `after` against `before`.
fn new_connections(before: &PageTables, after: &PageTables) -> Vec<TableConnection> {
    after
        .connections
        .iter()
        .filter(|c| !before.connections.iter().any(|b| b.id == c.id))
        .cloned()
        .collect()
}

/// The deduplicated polyline of a connection row.
fn line(c: &TableConnection) -> Vec<(i64, i64)> {
    let mut out: Vec<(i64, i64)> = Vec::new();
    for p in &c.polyline {
        if out.last() != Some(p) {
            out.push(*p);
        }
    }
    out
}

fn orthogonal(r: &[(i64, i64)]) -> bool {
    r.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1)
}

// ---------------------------------------------------------------------------------------------------------------
// The Wire tool (WR2, WR3's view-model part)

#[test]
fn wr2_the_wire_tool_draws_a_routed_connection_from_cp_to_cp_by_keyboard() {
    let tmp = Tmp::new("wire");
    let mut s = shell(&tmp, "102");
    let (a, b) = (cp(&s, "-102F1:2"), cp(&s, "-102F2:24VDC#1"));
    let before = tables(&s);
    let rev = s.revision();
    done(&mut s, "tool.wire", json!({}));
    assert_eq!(s.tool().name(), "wire");
    assert_eq!(s.tool().state(), "armed");
    cursor(&mut s, a.at);
    assert_eq!(canvas(&s)["snap"]["kind"], "cp");
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().state(), "routing");
    let announced = s.tool_announcement().unwrap_or_default().to_owned();
    assert!(
        announced.starts_with("Wire: routing from -102F1:2."),
        "{announced}"
    );
    assert_eq!(canvas(&s)["route"]["from"], "-102F1:2");
    cursor(&mut s, b.at);
    let route = canvas(&s)["route"].clone();
    assert_eq!(route["end"], "-102F2:24VDC#1");
    assert_eq!(route["bends"], 0);
    let line_text = describe_line(&s, "canvas route ").expect("the route line");
    assert!(
        line_text.starts_with("canvas route from \"-102F1:2\" bends 0 points ")
            && line_text.ends_with(" end \"-102F2:24VDC#1\""),
        "{line_text}"
    );
    // The ghost is drawn and its length is read out.
    assert!(
        s.tool_overlay()
            .items
            .iter()
            .any(|i| matches!(i, OverlayItem::Polyline { .. }))
    );
    assert!(s.tool_overlay().labels().any(|l| l.ends_with(" mm")));
    // The preview is the very command, with the ghost's route.
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    let preview = s.preview_state().expect("a preview");
    assert_eq!(preview.name, "connection.draw");
    assert!(preview.result.is_ok(), "{:?}", preview.result.as_ref().err());
    let ghost: Vec<(i64, i64)> = preview.command["route"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p[0].as_i64().unwrap(), p[1].as_i64().unwrap()))
        .collect();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    assert_eq!(s.tool().state(), "armed", "the tool stays armed");
    let after = tables(&s);
    let new = new_connections(&before, &after);
    assert_eq!(new.len(), 1, "exactly one new connection");
    let c = &new[0];
    assert!(c.stored_route);
    let mut ends = [end_label(&after, &c.a), end_label(&after, &c.b)];
    ends.sort();
    assert_eq!(ends, ["-102F1:2".to_owned(), "-102F2:24VDC#1".to_owned()]);
    // The committed route is the ghost, ends exact, orthogonal.
    assert_eq!(line(c), ghost);
    assert_eq!(ghost.first(), Some(&a.at));
    assert_eq!(ghost.last(), Some(&b.at));
    assert!(orthogonal(&ghost), "{ghost:?}");
    // Escape returns to Select.
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
}

#[test]
fn wr2_bends_back_junction_and_segment_ends_and_an_empty_grid_start() {
    let tmp = Tmp::new("ends");
    let mut s = shell(&tmp, "102");
    done(&mut s, "tool.wire", json!({}));
    // A wire needs two endpoints: Enter on the empty grid does not start one.
    let empty = grid_point(&s, (20, 60));
    let rev = s.revision();
    enter_at(&mut s, empty);
    assert_eq!(canvas(&s)["snap"]["kind"], "grid");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev);
    // From a free CP, a bend on the grid, Backspace, the bend again, then onto a junction.
    let start = cp(&s, "-102F2:13");
    enter_at(&mut s, start.at);
    let bend = grid_point(&s, (58, 33));
    enter_at(&mut s, bend);
    assert_eq!(canvas(&s)["route"]["bends"], 1);
    done(&mut s, "tool.back", json!({}));
    assert_eq!(canvas(&s)["route"]["bends"], 0);
    done(&mut s, "tool.back", json!({}));
    assert_eq!(
        s.tool().state(),
        "routing",
        "Backspace without a bend does nothing"
    );
    let bend = grid_point(&s, (58, 33));
    enter_at(&mut s, bend);
    let junction = tables(&s)
        .junctions
        .iter()
        .find(|j| j.style == "bevel" && j.wiring == "chain")
        .unwrap()
        .clone();
    // WP-S2.19 D13 (RW4): a junction that already joins three wires is no end, and the hint says so.
    assert!(junction.sequence.len() >= 3);
    cursor(&mut s, junction.at);
    // The snap falls past it: to a point on one of its wires or to the grid, never to the junction itself.
    let label = canvas(&s)["snap"]["label"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(!label.starts_with('@') || label.contains(" -> "), "{label}");
    assert_eq!(
        s.tool_hint(),
        "This junction already joins three wires: end on a wire to make a new junction."
    );
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().state(), "armed");
    // From another free CP onto a point of a wire: a junction there and the wire split.
    let before = tables(&s);
    let w = before
        .connections
        .iter()
        .find(|c| line(c).len() == 3 && line(c)[1].1 == line(c)[2].1 && line(c)[2].1 == 15_875_000)
        .expect("-102G1:PE to PE-1.2")
        .clone();
    let x = s.canvas().unwrap().page().snapper.page_of_index(76);
    let point = (x, 15_875_000);
    let free = cp(&s, "-102F2:14");
    enter_at(&mut s, free.at);
    cursor(&mut s, point);
    assert_eq!(
        canvas(&s)["snap"]["kind"],
        "junction",
        "a split point shows the junction marker"
    );
    done(&mut s, "canvas.activate", json!({}));
    let after = tables(&s);
    assert_eq!(after.junctions.len(), before.junctions.len() + 1);
    assert_eq!(after.connections.len(), before.connections.len() + 2);
    let nj = after
        .junctions
        .iter()
        .find(|x| !before.junctions.iter().any(|b| b.id == x.id))
        .unwrap();
    assert_eq!(nj.at, point);
    assert_eq!(nj.sequence.len(), 3);
    assert_eq!(
        nj.sequence[0], w.id,
        "the split wire keeps its ID on the first side"
    );
}

/// Fix round item 2: Backspace after two collinear bends removes only the last one (the earlier bend, cleaned out of
/// the straight route, comes back).
#[test]
fn backspace_after_two_collinear_bends_keeps_the_first() {
    let tmp = Tmp::new("bends");
    let mut s = shell(&tmp, "102");
    done(&mut s, "tool.wire", json!({}));
    let start = cp(&s, "-102F2:13");
    enter_at(&mut s, start.at);
    // Bends straight above the CP, on rows no wire runs along.
    let (b1, b2) = (grid_point(&s, (58, 33)), grid_point(&s, (58, 32)));
    enter_at(&mut s, b1);
    enter_at(&mut s, b2);
    assert_eq!(canvas(&s)["route"]["bends"], 2);
    done(&mut s, "tool.back", json!({}));
    assert_eq!(canvas(&s)["route"]["bends"], 1);
    let aside = grid_point(&s, (64, 32));
    cursor(&mut s, aside);
    let ghost = s.wire_ghost_route().expect("routing");
    assert!(
        ghost.contains(&(start.at.0, b1.1)),
        "the first bend stays: {ghost:?}"
    );
    assert!(
        !ghost.contains(&(start.at.0, b2.1)),
        "the second bend is gone: {ghost:?}"
    );
}

/// Fix round item 1: the Wire tool draws the page type's connection kind (doc 06 §2.2): refused on a bus-topology
/// page (folio 3), `overview` on an assembly overview (folio 201, whose overview symbols draw no connection points in
/// Golden A, so the kind is read from the host), a fluid line on a fluid page (folio 203), a wire (no
/// `kind` sent) on a multi-line page.
#[test]
fn the_wire_tool_draws_the_page_types_kind_or_is_refused() {
    let tmp = Tmp::new("kinds");
    let mut s = shell(&tmp, "3");
    assert_eq!(refused(&mut s, "tool.wire", json!({})), "reason-no-wire-kind");
    assert!(s.describe().contains("item tool.wire \"Wire\" [W] disabled: "));
    done(&mut s, "page.goto", json!({"folio": "201"}));
    assert_eq!(s.wire_kind(), Ok(Some("overview")));
    done(&mut s, "tool.wire", json!({}));
    assert_eq!(s.tool().name(), "wire");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "page.goto", json!({"folio": "203"}));
    assert_eq!(s.wire_kind(), Ok(Some("fluid")), "a fluid page");
    done(&mut s, "tool.wire", json!({}));
    assert_eq!(s.tool().name(), "wire");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert_eq!(s.wire_kind(), Ok(None));
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "tool.wire", json!({}));
    let t = tables(&s);
    let (a, b) = (cp(&s, "-102F2:13"), cp(&s, "-102F2:14"));
    enter_at(&mut s, a.at);
    enter_at(&mut s, b.at);
    let new = new_connections(&t, &tables(&s));
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].kind, "wire");
}

#[test]
fn fr_con_006_two_wires_to_one_cp_are_listed_in_drawing_order() {
    let tmp = Tmp::new("order");
    let mut s = shell(&tmp, "102");
    let target = cp(&s, "-102F1:1");
    done(&mut s, "tool.wire", json!({}));
    let mut drawn = Vec::new();
    for from in ["-102F2:13", "-102F2:14"] {
        let before = tables(&s);
        let start = cp(&s, from);
        enter_at(&mut s, start.at);
        enter_at(&mut s, target.at);
        let new = new_connections(&before, &tables(&s));
        assert_eq!(new.len(), 1, "{from}");
        drawn.push(new[0].id);
    }
    let listed = cp(&s, "-102F1:1").connections;
    let pos = |id| listed.iter().position(|x| *x == id).unwrap();
    assert!(pos(drawn[0]) < pos(drawn[1]), "drawing order: {listed:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// Autoconnect (WR4, FR-CON-003)

/// The connection points the place ghost shows, relative to the ghost's insertion point `p` (page nm).
fn ghost_cps(s: &ShellModel, p: (i64, i64)) -> Vec<(i64, i64)> {
    let page = s.canvas().unwrap().page();
    s.tool_overlay()
        .items
        .iter()
        .filter_map(|i| match i {
            OverlayItem::Marker {
                at,
                kind: MarkerKind::Cp,
            } => {
                let q = page.snapper.sheet_to_page(*at);
                Some((q.0 - p.0, q.1 - p.1))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn wr4_a_contact_between_two_facing_free_cps_proposes_two_connections_and_alt_commits_none() {
    let tmp = Tmp::new("auto");
    let mut s = shell(&tmp, "101");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    // The contact's CP 1 lies `span` grid rows below its CP 0.
    let p0 = grid_point(&s, (60, 38));
    cursor(&mut s, p0);
    let rel = ghost_cps(&s, p0);
    assert_eq!(rel.len(), 2, "{rel:?}");
    let g = s.canvas().unwrap().page().snapper.page_of_index(1);
    let span = (rel[1].1 - rel[0].1).abs().div_euclid(g).max(1) + 1;
    // A above, B below, the ghost between them.
    let before = tables(&s).connections.len();
    done(&mut s, "canvas.activate", json!({}));
    // B with Alt (Ctrl+Enter, the keyboard's Alt click): its CP 0 would autoconnect to A's CP 1.
    let pb = grid_point(&s, (60, 38 + 3 * span + 2));
    cursor(&mut s, pb);
    done(&mut s, "canvas.activate_alt", json!({}));
    assert_eq!(tables(&s).connections.len(), before, "nothing to connect yet");
    let mid = grid_point(&s, (60, 38 + span + 1));
    cursor(&mut s, mid);
    assert_eq!(canvas(&s)["ghost"]["hints"], 2, "{}", s.describe());
    let line_text = describe_line(&s, "canvas proposals ").expect("the proposals line");
    assert!(line_text.starts_with("canvas proposals 2 "), "{line_text}");
    // Tab cycles each ray to its next hit (one hit each: the same proposals).
    done(&mut s, "tool.next_proposal", json!({}));
    assert_eq!(canvas(&s)["ghost"]["hints"], 2);
    // The preview runs the command with the two hints.
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    let preview = s.preview_state().unwrap();
    assert!(preview.result.is_ok(), "{:?}", preview.result.as_ref().err());
    assert_eq!(preview.command["connect"].as_array().map(Vec::len), Some(2));
    // Alt at the commit sends none.
    let rev = s.revision();
    let q = sheet(&s, mid);
    done(&mut s, "canvas.press", at(q, json!({"alt": true})));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(tables(&s).connections.len(), before, "Alt commits no connection");
    // Undo; without Alt the placement brings its two connections.
    done(&mut s, "edit.undo", json!({}));
    enter_at(&mut s, mid);
    assert_eq!(tables(&s).connections.len(), before + 2);
    done(&mut s, "ui.dismiss", json!({}));
    // Undo again and place the middle contact with Alt: a drag of it shows the proposals and commits none.
    done(&mut s, "edit.undo", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    cursor(&mut s, mid);
    let q = sheet(&s, mid);
    done(&mut s, "canvas.press", at(q, json!({"alt": true})));
    done(&mut s, "ui.dismiss", json!({}));
    let t = tables(&s);
    let c = t
        .reps
        .iter()
        .find(|r| r.pos == mid)
        .expect("the middle contact")
        .clone();
    let centre = sheet(&s, ((c.body[0] + c.body[2]) / 2, (c.body[1] + c.body[3]) / 2));
    let right = sheet(&s, ((c.body[0] + c.body[2]) / 2 + g, (c.body[1] + c.body[3]) / 2));
    // WP-S2.19 RM2 (D15): the contact moved a step right, off the facing CPs, connects nothing.
    let n = tables(&s).connections.len();
    done(&mut s, "canvas.press", at(centre, json!({})));
    done(&mut s, "canvas.pointer", at(right, json!({})));
    done(&mut s, "canvas.release", at(right, json!({})));
    assert_eq!(tables(&s).connections.len(), n);
    // Dragged back onto them, the move shows its two proposals and connects them on release, in one undo step.
    let drag_back = |s: &mut ShellModel| {
        done(s, "canvas.press", at(right, json!({})));
        done(s, "canvas.pointer", at(centre, json!({})));
        assert_eq!(s.tool().state(), "dragging");
    };
    drag_back(&mut s);
    let line_text = describe_line(&s, "canvas proposals ").expect("a move's proposals");
    assert!(line_text.ends_with(" connect all"), "{line_text}");
    let rev = s.revision();
    done(&mut s, "canvas.release", at(centre, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one step");
    assert_eq!(
        tables(&s).connections.len(),
        n + 2,
        "the move connects its proposals"
    );
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(
        tables(&s).connections.len(),
        n,
        "the undo takes the move and its wires"
    );
    // With Alt held at the release: none.
    drag_back(&mut s);
    done(&mut s, "canvas.release", at(centre, json!({"alt": true})));
    assert_eq!(
        tables(&s).connections.len(),
        n,
        "Alt at the release connects none"
    );
    done(&mut s, "edit.undo", json!({}));
    // `tool.connect_on_move` turns it off for the session, and the status bar says so with the key.
    done(&mut s, "tool.connect_on_move", json!({}));
    assert_eq!(s.tool_notice(), Some("C Connect on move: off"));
    drag_back(&mut s);
    let line_text = describe_line(&s, "canvas proposals ").expect("a move's proposals");
    assert!(line_text.ends_with(" connect none"), "{line_text}");
    done(&mut s, "canvas.release", at(centre, json!({})));
    assert_eq!(tables(&s).connections.len(), n, "connect on move off");
}

/// WP-S2.19 RM1 (D14): on an empty stretch of Golden A's folio 103, coils -103K1 and -103K2 side by side joined twice,
/// under the A2s and over the A1s, and -103K3 below between them. A drag of the first two four steps down would carry
/// the lower wire through -103K3: the release re-solves it in the same undo step, and the upper wire, clean where it
/// follows, keeps its vertices translated.
#[test]
fn rm1_a_move_re_solves_a_followed_route_that_would_cross_a_symbol_and_keeps_a_clean_one() {
    use ew_viewmodel::ew_api::routing::measure;
    let tmp = Tmp::new("rm1");
    let mut s = shell(&tmp, "103");
    for k in [(55, 20), (65, 20), (60, 29)] {
        done(
            &mut s,
            "place.pick",
            json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        );
        let p = grid_point(&s, k);
        enter_at(&mut s, p);
        done(&mut s, "ui.dismiss", json!({}));
    }
    let page = tables(&s).page.to_string();
    let pin = |s: &ShellModel, label: &str| {
        let c = cp(s, label);
        json!({"cp": {"function": c.function.to_string(), "idx": c.idx}})
    };
    let route = |s: &ShellModel, points: &[(i64, i64)]| -> Value {
        points
            .iter()
            .map(|k| {
                let p = grid_point(s, *k);
                json!([p.0, p.1])
            })
            .collect()
    };
    for (a, b, points) in [
        ("-103K1:A2", "-103K2:A2", [(55, 24), (55, 26), (65, 26), (65, 24)]),
        ("-103K1:A1", "-103K2:A1", [(55, 20), (55, 17), (65, 17), (65, 20)]),
    ] {
        let params = json!({"a": pin(&s, a), "b": pin(&s, b), "page": page, "route": route(&s, &points)});
        done(
            &mut s,
            "command.run",
            json!({"cmd": "connection.draw", "params": params}),
        );
    }
    let wire = |s: &ShellModel, a: &str| -> TableConnection {
        let t = tables(s);
        let f = t.cps.iter().find(|c| c.label == a).expect("the CP").clone();
        t.connections
            .iter()
            .find(|c| {
                c.a == TableEnd::Cp {
                    function: f.function,
                    idx: f.idx,
                }
            })
            .expect("the wire")
            .clone()
    };
    let upper = wire(&s, "-103K1:A1").polyline;
    let centre = |s: &ShellModel, tag: &str| {
        let t = tables(s);
        let r = t
            .reps
            .iter()
            .find(|r| {
                t.cps
                    .iter()
                    .any(|c| c.rep == r.id && c.label == format!("{tag}:A1"))
            })
            .expect("the coil")
            .clone();
        sheet(s, ((r.body[0] + r.body[2]) / 2, (r.body[1] + r.body[3]) / 2))
    };
    let (k1, k2) = (centre(&s, "-103K1"), centre(&s, "-103K2"));
    done(&mut s, "canvas.press", at(k1, json!({})));
    done(&mut s, "canvas.release", at(k1, json!({})));
    done(&mut s, "canvas.press", at(k2, json!({"shift": true})));
    done(&mut s, "canvas.release", at(k2, json!({"shift": true})));
    let d = grid_point(&s, (0, 24)).1 - grid_point(&s, (0, 20)).1;
    let down = sheet(&s, (0, d)).1 - sheet(&s, (0, 0)).1;
    let rev = s.revision();
    done(&mut s, "canvas.press", at(k2, json!({})));
    done(&mut s, "canvas.pointer", at((k2.0, k2.1 + down), json!({})));
    done(&mut s, "canvas.release", at((k2.0, k2.1 + down), json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
    // The upper wire: its route translated, vertex for vertex.
    let moved: Vec<(i64, i64)> = upper.iter().map(|p| (p.0, p.1 + d)).collect();
    assert_eq!(wire(&s, "-103K1:A1").polyline, moved);
    // The lower wire: routed again, through no symbol, and the page breaks no hard rule.
    let lower = wire(&s, "-103K1:A2");
    let geometry = s.canvas().unwrap().page().geometry.clone();
    let m = measure(&geometry);
    assert_eq!(
        (
            m.touches.len(),
            m.overlaps.len(),
            m.through_bodies.len(),
            m.through_texts.len()
        ),
        (0, 0, 0, 0),
        "{:?}",
        lower.polyline
    );
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(wire(&s, "-103K1:A1").polyline, upper, "the undo restores both");
}

/// C53-rt#3 (D3, RM1): an unrouted connection's dashed line is no wire, so a moved wire whose followed route lies
/// along it stays clean and keeps its route vertex for vertex. On folio 103, -103K1:A2 → -103K2:A2 is unrouted along
/// row 24, and the routed -103K3:A1 → -103K4:A1 runs along row 24 between them; -103K4 moves one step right.
#[test]
fn a_followed_route_along_an_unrouted_line_is_kept() {
    let tmp = Tmp::new("rt3");
    let mut s = shell(&tmp, "103");
    coils_and_wires(
        &mut s,
        &[(55, 20), (65, 20), (58, 30), (62, 30)],
        &[
            ("-103K1:A2", "-103K2:A2", &[]),
            (
                "-103K3:A1",
                "-103K4:A1",
                &[(58, 30), (58, 24), (62, 24), (62, 30)],
            ),
        ],
    );
    let routed = |s: &ShellModel| -> Vec<(i64, i64)> {
        let t = tables(s);
        t.connections
            .iter()
            .find(|c| end_label(&t, &c.a) == "-103K3:A1")
            .expect("the routed wire")
            .polyline
            .clone()
    };
    let k4 = {
        let t = tables(&s);
        let r = t
            .reps
            .iter()
            .find(|r| t.cps.iter().any(|c| c.rep == r.id && c.label == "-103K4:A1"))
            .expect("-103K4")
            .clone();
        sheet(&s, ((r.body[0] + r.body[2]) / 2, (r.body[1] + r.body[3]) / 2))
    };
    let step = sheet(&s, grid_point(&s, (1, 0))).0 - sheet(&s, grid_point(&s, (0, 0))).0;
    done(&mut s, "canvas.press", at(k4, json!({})));
    done(&mut s, "canvas.pointer", at((k4.0 + step, k4.1), json!({})));
    done(
        &mut s,
        "canvas.release",
        at((k4.0 + step, k4.1), json!({"alt": true})),
    );
    let expected: Vec<(i64, i64)> = [(58, 30), (58, 24), (63, 24), (63, 30)]
        .iter()
        .map(|k| grid_point(&s, *k))
        .collect();
    assert_eq!(routed(&s), expected, "followed, not routed again");
}

/// C53-rt#12 (ADR-0007 decision 4): a contact dragged onto a routed wire, its two CPs on the wire facing its ends,
/// lists no proposal, since a move never splits a wire; the release changes no connection.
#[test]
fn a_move_onto_a_wire_lists_no_split() {
    let tmp = Tmp::new("rt12");
    let mut s = shell(&tmp, "103");
    coils_and_wires(
        &mut s,
        &[(60, 20), (60, 40)],
        &[("-103K1:A2", "-103K2:A1", &[(60, 24), (60, 40)])],
    );
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    let p = grid_point(&s, (70, 30));
    cursor(&mut s, p);
    done(&mut s, "canvas.activate_alt", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    let t = tables(&s);
    let c = t.reps.iter().find(|r| r.pos == p).expect("the contact").clone();
    let n = t.connections.len();
    let centre = sheet(&s, ((c.body[0] + c.body[2]) / 2, (c.body[1] + c.body[3]) / 2));
    let left = sheet(&s, grid_point(&s, (0, 0))).0 - sheet(&s, grid_point(&s, (10, 0))).0;
    done(&mut s, "canvas.press", at(centre, json!({})));
    done(
        &mut s,
        "canvas.pointer",
        at((centre.0 + left, centre.1), json!({})),
    );
    assert_eq!(s.tool().state(), "dragging");
    assert_eq!(describe_line(&s, "canvas proposals "), None);
    done(
        &mut s,
        "canvas.release",
        at((centre.0 + left, centre.1), json!({})),
    );
    assert_eq!(tables(&s).connections.len(), n, "no split, no wire");
}

/// ID-1509 (9), the window check's C2: the connect-on-move key pressed during a drag turns the state the move's prompt
/// shows at once and keeps its readout; no notice takes the prompt's place.
#[test]
fn the_connect_on_move_key_keeps_the_move_s_readout() {
    use ew_viewmodel::command_bar::hud;
    let tmp = Tmp::new("c2");
    let mut s = shell(&tmp, "103");
    coils_and_wires(&mut s, &[(60, 20)], &[]);
    let t = tables(&s);
    let r = t
        .reps
        .iter()
        .find(|r| t.cps.iter().any(|c| c.rep == r.id && c.label == "-103K1:A1"))
        .expect("-103K1")
        .clone();
    let centre = sheet(&s, ((r.body[0] + r.body[2]) / 2, (r.body[1] + r.body[3]) / 2));
    let step = sheet(&s, grid_point(&s, (1, 0))).0 - sheet(&s, grid_point(&s, (0, 0))).0;
    done(&mut s, "canvas.press", at(centre, json!({})));
    done(
        &mut s,
        "canvas.pointer",
        at((centre.0 + step, centre.1), json!({})),
    );
    let readout = match s.tool_ghost() {
        Some(ew_viewmodel::tools::Ghost::Move { readout, .. }) => readout,
        other => panic!("a move's ghost: {other:?}"),
    };
    assert_eq!(hud(&s).prompt, format!("{readout} \u{b7} C Connect on move: on"));
    done(&mut s, "tool.connect_on_move", json!({}));
    assert_eq!(s.tool_notice(), None);
    assert_eq!(hud(&s).prompt, format!("{readout} \u{b7} C Connect on move: off"));
    done(&mut s, "tool.connect_on_move", json!({}));
    assert_eq!(hud(&s).prompt, format!("{readout} \u{b7} C Connect on move: on"));
}

/// A wire to draw: its two CP labels and its route's grid indices.
type WireSpec<'a> = (&'a str, &'a str, &'a [(i64, i64)]);

/// Coils at the grid indices `ks` on folio 103 (`-103K1`, `-103K2`, … in this order) and the wires `(a, b, route)`
/// between their CPs, each route a list of grid indices (empty: unrouted), drawn by `connection.draw`.
fn coils_and_wires(s: &mut ShellModel, ks: &[(i64, i64)], wires: &[WireSpec<'_>]) {
    for k in ks {
        done(
            s,
            "place.pick",
            json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        );
        let p = grid_point(s, *k);
        // With Alt: the coils join only by the wires given, never by autoconnect.
        cursor(s, p);
        done(s, "canvas.activate_alt", json!({}));
        done(s, "ui.dismiss", json!({}));
    }
    let page = tables(s).page.to_string();
    for (a, b, points) in wires {
        let pin = |label: &str| {
            let c = cp(s, label);
            json!({"cp": {"function": c.function.to_string(), "idx": c.idx}})
        };
        let route: Vec<Value> = points
            .iter()
            .map(|k| {
                let p = grid_point(s, *k);
                json!([p.0, p.1])
            })
            .collect();
        let params = json!({"a": pin(a), "b": pin(b), "page": page, "route": route});
        done(
            s,
            "command.run",
            json!({"cmd": "connection.draw", "params": params}),
        );
    }
}

/// The card line of the describe text, while a card is open.
fn card_line(s: &ShellModel) -> String {
    describe_line(s, "card ").expect("a card")
}

/// WP-S2.19 RW1 (D13, §12a): on folio 103 with two unrouted wires, one between two free coils and one from a CP whose
/// stub ends in another coil, Route page previews the plan, its lead and its unrouted row in en-GB and nl-NL; Apply
/// commits one `connection.route_many` (one undo step) and the toast says what it routed; a second Route page has
/// nothing to do.
#[test]
fn rw1_route_page_previews_the_plan_and_applies_it_in_one_step() {
    let tmp = Tmp::new("rw1");
    let mut s = shell(&tmp, "103");
    coils_and_wires(
        &mut s,
        &[(55, 20), (55, 29), (70, 20), (70, 25)],
        &[("-103K1:A2", "-103K2:A1", &[]), ("-103K3:A2", "-103K2:A2", &[])],
    );
    done(&mut s, "route.page", json!({}));
    assert_eq!(
        card_line(&s),
        "card ready \"connection.route_many\" targets 1 changes 1 tags 0 xrefs 0 route \"Route 2 wires on folio 103: 1 \
         routed, 1 not routed.\" \"No route keeps clear of symbols: -103K3:A2 → -103K2:A2.\""
    );
    // ID-1509 (9), the window check's C3: the card's title names the button.
    let title = |s: &ShellModel| s.properties().card().expect("the card").command().to_owned();
    assert_eq!(title(&s), "Route page");
    done(&mut s, "view.language", json!({"value": "nl"}));
    done(&mut s, "preview.cancel", json!({}));
    done(&mut s, "route.page", json!({}));
    assert_eq!(title(&s), "Pagina routeren");
    assert_eq!(
        card_line(&s),
        "card ready \"connection.route_many\" targets 1 changes 1 tags 0 xrefs 0 route \"2 draden op folio 103 \
         routeren: 1 gerouteerd, 1 niet gerouteerd.\" \"Geen route blijft vrij van symbolen: -103K3:A2 → \
         -103K2:A2.\""
    );
    done(&mut s, "view.language", json!({"value": "en"}));
    let rev = s.revision();
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one undo step");
    assert_eq!(
        s.toast().map(|t| t.text.as_str()),
        Some("Routed 1 wire on folio 103.")
    );
    let t = tables(&s);
    let routed: Vec<bool> = t
        .connections
        .iter()
        .filter(|c| c.polyline.len() > 1 && cp_label_at(&t, c).starts_with("-103K"))
        .map(|c| c.stored_route)
        .collect();
    assert_eq!(routed.iter().filter(|r| **r).count(), 1, "{routed:?}");
    // The unroutable wire stays; only it is left, so Route page previews it again and routes none.
    assert_eq!(
        done_outcome(&mut s, "route.page"),
        "Route 1 wire on folio 103: 0 routed, 1 not routed."
    );
}

/// ID-1509 (8), the window check's F2: a Route selection plan whose routes are the ones drawn opens no card and says
/// so, in en-GB and nl-NL.
#[test]
fn route_selection_with_a_plan_that_changes_nothing_opens_no_card() {
    let tmp = Tmp::new("f2");
    let mut s = shell(&tmp, "103");
    coils_and_wires(&mut s, &[(55, 20), (55, 29)], &[("-103K1:A2", "-103K2:A1", &[])]);
    done(&mut s, "route.page", json!({}));
    done(&mut s, "preview.apply", json!({}));
    let t = tables(&s);
    let wire = t
        .connections
        .iter()
        .find(|c| end_label(&t, &c.a) == "-103K1:A2")
        .expect("the wire")
        .clone();
    assert!(wire.stored_route, "Route page routed it");
    let mid = sheet(
        &s,
        (
            (wire.polyline[0].0 + wire.polyline[1].0) / 2,
            (wire.polyline[0].1 + wire.polyline[1].1) / 2,
        ),
    );
    done(&mut s, "canvas.press", at(mid, json!({})));
    done(&mut s, "canvas.release", at(mid, json!({})));
    let rev = s.revision();
    done(&mut s, "route.selection", json!({}));
    assert_eq!(describe_line(&s, "card "), None, "no card");
    assert_eq!(s.revision(), rev, "nothing committed");
    assert_eq!(
        s.toast().map(|t| t.text.as_str()),
        Some("Nothing to change: the selected wires keep their routes.")
    );
    done(&mut s, "view.language", json!({"value": "nl"}));
    done(&mut s, "route.selection", json!({}));
    assert_eq!(
        s.toast().map(|t| t.text.as_str()),
        Some("Niets te wijzigen: de geselecteerde draden houden hun routes.")
    );
}

/// The label of a connection's end `a`.
fn cp_label_at(t: &PageTables, c: &TableConnection) -> String {
    end_label(t, &c.a)
}

/// The failure message of `action` (a solve that routes nothing fails with its lead).
fn done_outcome(s: &mut ShellModel, action: &str) -> String {
    match s.dispatch(action, &json!({})) {
        ew_viewmodel::shell::Outcome::Failed { message } => message,
        other => panic!("{action}: {other:?}"),
    }
}

/// WP-S2.19 RW2 (D13): Route selection with one routed wire selected routes it again, in the card's one step; the
/// page's other wires keep their routes.
#[test]
fn rw2_route_selection_re_routes_a_selected_wire_and_nothing_else() {
    let tmp = Tmp::new("rw2");
    let mut s = shell(&tmp, "103");
    coils_and_wires(
        &mut s,
        &[(55, 20), (65, 20)],
        &[(
            "-103K1:A2",
            "-103K2:A2",
            &[(55, 24), (55, 31), (60, 31), (60, 26), (65, 26), (65, 24)],
        )],
    );
    let before = tables(&s);
    let wire = before
        .connections
        .iter()
        .find(|c| end_label(&before, &c.a) == "-103K1:A2")
        .expect("the wire")
        .clone();
    let mid = sheet(
        &s,
        (wire.polyline[1].0, (wire.polyline[1].1 + wire.polyline[2].1) / 2),
    );
    done(&mut s, "canvas.press", at(mid, json!({})));
    done(&mut s, "canvas.release", at(mid, json!({})));
    done(&mut s, "route.selection", json!({}));
    assert_eq!(
        card_line(&s),
        "card ready \"connection.route_many\" targets 1 changes 1 tags 0 xrefs 0 route \"Route 1 wire on folio 103: 1 \
         routed, 0 not routed.\""
    );
    // ID-1509 (9), the window check's C3: the card's title names the button.
    let card = s.properties().card().expect("the card").command().to_owned();
    assert_eq!(card, "Route selection");
    done(&mut s, "view.language", json!({"value": "nl"}));
    assert_eq!(
        s.properties().card().expect("the card").command(),
        "Selectie routeren"
    );
    done(&mut s, "view.language", json!({"value": "en"}));
    done(&mut s, "preview.apply", json!({}));
    let after = tables(&s);
    let now = after
        .connections
        .iter()
        .find(|c| c.id == wire.id)
        .expect("the wire");
    assert_ne!(now.polyline, wire.polyline, "routed again");
    let others = |t: &PageTables| -> Vec<(ew_viewmodel::ew_api::output::Uuid, Vec<(i64, i64)>)> {
        t.connections
            .iter()
            .filter(|c| c.id != wire.id)
            .map(|c| (c.id, c.polyline.clone()))
            .collect()
    };
    assert_eq!(others(&after), others(&before), "nothing else changes");
}

/// ID-1509 (8), C53-rt#6: Route selection needs a selected wire or symbol; with nothing selected it is refused,
/// whatever the page holds unrouted.
#[test]
fn route_selection_needs_a_selection() {
    let tmp = Tmp::new("rt6");
    let mut s = shell(&tmp, "103");
    coils_and_wires(&mut s, &[(55, 20), (55, 29)], &[("-103K1:A2", "-103K2:A1", &[])]);
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(
        s.dispatch("route.selection", &json!({})),
        ew_viewmodel::shell::Outcome::Refused {
            reason_key: ew_viewmodel::key!("reason-route-selection")
        }
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Route edit and the split on insert (WR5)

#[test]
fn wr5_a_route_given_by_the_route_editor_and_a_contact_inserted_split_the_wire() {
    let tmp = Tmp::new("split");
    let mut s = shell(&tmp, "101");
    // W13: from J1 to E55:L, routed by Golden A's route tool.
    let e55 = cp(&s, "E55:L");
    let w13 = tables(&s)
        .connections
        .iter()
        .find(|c| {
            matches!(c.a, TableEnd::Junction { .. })
                && matches!(c.b, TableEnd::Cp { function, idx } if function == e55.function && idx == e55.idx)
        })
        .expect("W13")
        .clone();
    let drawn = line(&w13);
    // W13's printed number before the insert.
    let number_before = {
        let (golden, _) =
            ew_viewmodel::ew_api::Session::open_read_only(&tmp.0.join("J-E-2416.ew"), "test").unwrap();
        golden
            .property_sheet(&[w13.id.to_string().parse().unwrap()])
            .header
            .title
    };
    // An override first, so the split shows it copied (FR-CON-008).
    done(
        &mut s,
        "command.run",
        json!({"cmd": "fields.set", "params": {
            "kind": "connection", "targets": [w13.id.to_string()], "path": "overrides.csa", "value": "1.5"
        }}),
    );
    done(&mut s, "select.set", json!({"ids": [w13.id.to_string()]}));
    done(&mut s, "tool.route_edit", json!({}));
    assert_eq!(s.tool().name(), "route_edit");
    // Enter without a move stores the drawn polyline as the wire's route.
    done(&mut s, "canvas.activate", json!({}));
    let routed = tables(&s)
        .connections
        .iter()
        .find(|c| c.id == w13.id)
        .unwrap()
        .clone();
    assert!(routed.stored_route);
    assert_eq!(line(&routed), drawn);
    // A contact dropped onto its longest vertical segment, its CPs facing the ends: one split proposal, no hint.
    let seg = drawn
        .windows(2)
        .filter(|w| w[0].0 == w[1].0)
        .max_by_key(|w| (w[0].1 - w[1].1).abs())
        .expect("a vertical segment");
    let x = seg[0].0;
    let (y0, y1) = (seg[0].1.min(seg[1].1), seg[0].1.max(seg[1].1));
    let sn = s.canvas().unwrap().page().snapper.clone();
    let k = (sn.grid_index_of(y0) + sn.grid_index_of(y1)) / 2 - 2;
    let p = (x, sn.page_of_index(k));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    cursor(&mut s, p);
    let line_text = describe_line(&s, "canvas proposals ").expect("the split proposal");
    assert!(line_text.ends_with(" -> segment"), "{line_text}");
    assert_eq!(canvas(&s)["ghost"]["hints"], 1);
    let before = tables(&s);
    done(&mut s, "canvas.activate", json!({}));
    let after = tables(&s);
    assert_eq!(
        after.connections.len(),
        before.connections.len() + 1,
        "W13 split in two"
    );
    let kept = after
        .connections
        .iter()
        .find(|c| c.id == w13.id)
        .expect("W13 keeps its ID");
    assert!(matches!(kept.a, TableEnd::Junction { .. }), "on the J1 side");
    let new = new_connections(&before, &after);
    assert_eq!(new.len(), 1);
    assert_eq!(end_label(&after, &new[0].b), "E55:L");
    // The records: W13's number spec stays on the kept side, both halves carry the override, the new half's number is
    // unassigned. (Golden A prints connection-oriented numbers derived for every wire, so the new half prints the
    // next derived number, not `?`: `?` is the text of an unassigned number with online wire numbering off.)
    done(&mut s, "file.save", json!({}));
    let (session, _) =
        ew_viewmodel::ew_api::Session::open_read_only(&tmp.0.join("J-E-2416.ew"), "test").unwrap();
    let q = session.query();
    let rec = |id: &str| {
        let (_, c) = q
            .connections()
            .iter()
            .find(|(cid, _)| cid.to_string() == id)
            .expect("the connection");
        serde_json::to_value(&**c).unwrap()
    };
    let (kept, half) = (rec(&w13.id.to_string()), rec(&new[0].id.to_string()));
    assert_eq!(kept["overrides"], json!({"csa": "1.5"}), "{kept}");
    assert_eq!(
        half["overrides"], kept["overrides"],
        "the same overrides on both halves"
    );
    assert_eq!(
        kept.get("number"),
        None,
        "W13's number spec is kept (unassigned, as Golden A's)"
    );
    assert_eq!(half.get("number"), None, "the new half is unnumbered");
    drop(q);
    let printed = |id: &str| session.property_sheet(&[id.parse().unwrap()]).header.title;
    let (kept_number, half_number) = (printed(&w13.id.to_string()), printed(&new[0].id.to_string()));
    assert_eq!(kept_number, number_before, "W13's number stays on the kept side");
    assert!(
        kept_number.starts_with("101."),
        "W13 prints a number: {kept_number}"
    );
    assert!(
        half_number.starts_with("101.") && half_number != kept_number,
        "{half_number}"
    );
}

#[test]
fn route_edit_drags_a_middle_segment_of_a_selected_connection() {
    let tmp = Tmp::new("drag");
    let mut s = shell(&tmp, "102");
    // -102G1:PE to PE-1.2 given a Z route with a middle segment on grid row 8.
    let w = tables(&s)
        .connections
        .iter()
        .find(|c| line(c).len() == 3 && line(c)[2].1 == 15_875_000 && line(c)[0].0 == 127_000_000)
        .expect("-102G1:PE to PE-1.2")
        .clone();
    let pts = line(&w);
    let sn = s.canvas().unwrap().page().snapper.clone();
    let row = sn.page_of_index(8);
    let z = [pts[0], (pts[0].0, row), (pts[2].0, row), pts[2]];
    let route: Vec<Value> = z.iter().map(|p| json!([p.0, p.1])).collect();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "connection.route", "params": {"connection": w.id.to_string(), "route": route}}),
    );
    done(&mut s, "select.set", json!({"ids": [w.id.to_string()]}));
    let on = sheet(&s, ((pts[0].0 + pts[2].0) / 2, row));
    let down = sheet(&s, ((pts[0].0 + pts[2].0) / 2, sn.page_of_index(10)));
    done(&mut s, "canvas.press", at(on, json!({})));
    done(&mut s, "canvas.pointer", at(down, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    assert!(s.tool_overlay().labels().any(|l| l.starts_with("dx 0.000 dy ")));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(s.preview_state().unwrap().name, "connection.route");
    done(&mut s, "canvas.release", at(down, json!({})));
    let moved = tables(&s)
        .connections
        .iter()
        .find(|c| c.id == w.id)
        .unwrap()
        .clone();
    let new_row = sn.page_of_index(10);
    assert_eq!(
        line(&moved),
        vec![pts[0], (pts[0].0, new_row), (pts[2].0, new_row), pts[2]]
    );
    // WR6 (FR-CON-007): saved and reopened, the route is the same to the nm.
    let dragged = line(&moved);
    done(&mut s, "file.save", json!({}));
    done(&mut s, "file.close", json!({}));
    let dir = tmp.0.join("J-E-2416.ew");
    done(&mut s, "file.open", json!({"dir": dir.to_string_lossy()}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let reopened = tables(&s)
        .connections
        .iter()
        .find(|c| c.id == w.id)
        .unwrap()
        .clone();
    assert_eq!(
        line(&reopened),
        dragged,
        "the same route after a save and a reopen"
    );
    // A press on the first segment of the selected connection is no segment drag: a click selects.
    done(&mut s, "select.set", json!({"ids": [w.id.to_string()]}));
    let first = sheet(&s, (pts[0].0, (pts[0].1 + new_row) / 2));
    done(&mut s, "canvas.press", at(first, json!({})));
    assert_eq!(s.tool().state(), "pressed");
    done(&mut s, "canvas.release", at(first, json!({})));
}

// ---------------------------------------------------------------------------------------------------------------
// The junction sequence editor (WR7)

#[test]
fn wr7_the_junction_editor_lists_reorders_and_restyles() {
    let tmp = Tmp::new("junction");
    let mut s = shell(&tmp, "101");
    let f55 = cp(&s, "F55:2");
    let j1 = tables(&s)
        .junctions
        .iter()
        .find(|j| {
            j.sequence.first().is_some_and(|c| {
                tables(&s).connections.iter().any(|x| {
                    x.id == *c
                        && [x.a, x.b].iter().any(
                            |e| matches!(e, TableEnd::Cp { function, idx } if *function == f55.function && *idx == f55.idx),
                        )
                })
            })
        })
        .expect("J1")
        .clone();
    done(&mut s, "select.set", json!({"ids": [j1.id.to_string()]}));
    let calls = s.session_calls();
    // Enter on the selected junction opens the editor.
    done(&mut s, "canvas.activate", json!({}));
    let e = s.junction_editor().expect("the editor");
    assert_eq!(e.sequence_labels(), ["F55:2", "E55:L", "E56:X2"]);
    assert_eq!(e.style, "dot");
    assert_eq!(e.wiring, "star");
    assert_eq!(e.wires.len(), 2);
    assert_eq!(s.session_calls().junction_wires, calls.junction_wires + 1);
    let editor = canvas(&s)["junction_editor"].clone();
    assert_eq!(editor["sequence"], json!(["F55:2", "E55:L", "E56:X2"]));
    let cell = editor["junction"].as_str().unwrap();
    assert!(cell.len() < 12 && !cell.is_empty(), "a cell, no UUID: {cell}");
    let text = s.describe();
    let head = text.lines().find(|l| l.starts_with("canvas junction ")).unwrap();
    assert!(
        head.ends_with("style dot wiring star sequence \"F55:2\" \"E55:L\" \"E56:X2\""),
        "{head}"
    );
    assert_eq!(text.lines().filter(|l| l.starts_with("  wire \"")).count(), 2);
    // A reorder: one `junction.edit`, the editor follows, the wires follow (star: the first member is the hub).
    let wires_before = e.wires.clone();
    let rev = s.revision();
    let runs = s.session_calls().run_script;
    done(&mut s, "junction.member", json!({"from": 0, "to": 2}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(s.session_calls().run_script, runs + 1, "one commit");
    let e = s.junction_editor().unwrap();
    assert_eq!(e.sequence_labels(), ["E55:L", "E56:X2", "F55:2"]);
    assert_eq!(e.row, 2, "the moved member stays focused");
    assert_ne!(e.wires, wires_before, "the physical wires follow");
    // Style and wiring.
    done(&mut s, "junction.style", json!({"style": "bevel", "orient": 1}));
    let e = s.junction_editor().unwrap();
    assert_eq!(
        (e.style.as_str(), e.bevel_orient, e.wiring.as_str()),
        ("bevel", 1, "chain")
    );
    done(&mut s, "junction.wiring", json!({"wiring": "star"}));
    let e = s.junction_editor().unwrap();
    assert!(e.explicit_wiring && e.wiring == "star");
    assert_eq!(
        refused(&mut s, "junction.member", json!({"from": 0, "to": 9})),
        "reason-junction-member"
    );
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.junction_editor().is_none());
    assert_eq!(
        refused(&mut s, "junction.style", json!({"style": "dot"})),
        "reason-no-junction-editor"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Crossings, the 21 words and the exact counts (WR8's toggle, WR9, WR10)

#[test]
fn crossings_toggle_the_drawing_setting() {
    let tmp = Tmp::new("hops");
    let mut s = shell(&tmp, "103");
    assert!(!s.crossings_hops());
    assert!(describe_line(&s, "canvas crossings").is_none());
    done(&mut s, "project.crossings", json!({}));
    assert!(s.crossings_hops());
    assert_eq!(
        describe_line(&s, "canvas crossings").as_deref(),
        Some("canvas crossings hops")
    );
    assert!(
        s.describe()
            .contains("item project.crossings \"Show crossings as hops\" (checked) enabled")
    );
    done(&mut s, "project.crossings", json!({"on": false}));
    assert!(!s.crossings_hops());
    assert!(
        describe_line(&s, "canvas crossings").is_none(),
        "off removes the member"
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd": "project.drawing.set", "params": {"crossings": "plain"}}),
    );
    assert_eq!(
        describe_line(&s, "canvas crossings").as_deref(),
        Some("canvas crossings plain")
    );
}

#[test]
fn wr10_the_wire_tool_routes_once_per_changed_target_and_reads_nothing() {
    let tmp = Tmp::new("counts");
    let mut s = shell(&tmp, "102");
    done(&mut s, "tool.wire", json!({}));
    let a = cp(&s, "-102F1:2");
    enter_at(&mut s, a.at);
    let calls = s.session_calls();
    let routes = s.wire_route_calls().unwrap();
    // Ten cursor steps: ten changed targets, ten routes; the same point again routes nothing.
    for k in 0..10 {
        let q = grid_point(&s, (60 + k, 40));
        cursor(&mut s, q);
    }
    assert_eq!(s.wire_route_calls().unwrap(), routes + 10);
    let q = grid_point(&s, (69, 40));
    cursor(&mut s, q);
    assert_eq!(
        s.wire_route_calls().unwrap(),
        routes + 10,
        "an unchanged target is not routed again"
    );
    let b = cp(&s, "-102F2:24VDC#1");
    cursor(&mut s, b.at);
    let after = s.session_calls();
    assert_eq!(after.page_content, calls.page_content);
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.page_cps, calls.page_cps);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
    assert_eq!(after.preview_with, calls.preview_with);
    // A dwell previews once; a second dwell of the same command previews nothing.
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    done(&mut s, "canvas.dwell", json!({"ms": 200}));
    assert_eq!(s.session_calls().preview_with, calls.preview_with + 1);
}

// ---------------------------------------------------------------------------------------------------------------
// FT-423 in the view model: the rubber band of a move is the route the command stores

#[test]
fn the_rubber_band_of_a_move_is_the_route_rep_move_stores() {
    let tmp = Tmp::new("band");
    let mut s = shell(&tmp, "102");
    // The routed wire from -102F2:Out1 to 24VDC-1, then -102F2's box moved one step right: the far end stays and the
    // wire's route follows, in the rubber band and in the stored route alike.
    let a = cp(&s, "-102F2:Out1");
    let target = cp(&s, "24VDC-1");
    let t = tables(&s);
    let wire = t
        .connections
        .iter()
        .filter(|c| c.stored_route)
        .find(|c| end_label(&t, &c.a) == "-102F2:Out1")
        .unwrap()
        .clone();
    let rep = t.reps.iter().find(|r| r.id == a.rep).unwrap().clone();
    let centre = sheet(
        &s,
        ((rep.body[0] + rep.body[2]) / 2, (rep.body[1] + rep.body[3]) / 2),
    );
    let sn = s.canvas().unwrap().page().snapper.clone();
    let right = (
        centre.0 + sn.sheet_of_index(1, 0) - sn.sheet_of_index(0, 0),
        centre.1,
    );
    done(&mut s, "canvas.press", at(centre, json!({})));
    done(&mut s, "canvas.pointer", at(right, json!({})));
    let band: Vec<(i64, i64)> = s
        .tool_overlay()
        .items
        .iter()
        .filter_map(|i| match i {
            OverlayItem::Polyline { points, .. } => Some(points.clone()),
            _ => None,
        })
        .find(|pts| pts.last() == Some(&sheet(&s, target.at)) || pts.first() == Some(&sheet(&s, target.at)))
        .expect("the wire's rubber band");
    done(&mut s, "canvas.release", at(right, json!({})));
    let stored: Vec<(i64, i64)> = line(tables(&s).connections.iter().find(|c| c.id == wire.id).unwrap())
        .iter()
        .map(|p| sheet(&s, *p))
        .collect();
    assert_eq!(band, stored);
}

/// Window check, items 7 and 3: Edit ▸ Junction sequence… and Edit ▸ Edit route are disabled with the reason naming what
/// to select; an undo drops the tool's marks (overlay and preview) of the revision it leaves.
#[test]
fn junction_and_route_entries_name_what_to_select_and_undo_drops_the_marks() {
    let tmp = Tmp::new("window");
    let mut s = shell(&tmp, "102");
    // Headless: without a selection `junction.open` is refused with the reason naming what to select.
    assert_eq!(refused(&mut s, "junction.open", json!({})), "reason-one-junction");
    assert_eq!(
        ew_viewmodel::strings::text(key!("reason-one-junction")),
        "Select exactly one junction"
    );
    let text = s.describe();
    assert!(
        text.contains("item junction.open \"Junction sequence…\" disabled: Select exactly one junction"),
        "{text}"
    );
    assert!(text.contains("item tool.route_edit \"Edit route\" disabled: Select exactly one connection"));
    let w = tables(&s).connections[0].id;
    done(&mut s, "select.set", json!({"ids": [w.to_string()]}));
    assert_eq!(refused(&mut s, "junction.open", json!({})), "reason-one-junction");
    let j = tables(&s).junctions[0].id;
    done(&mut s, "select.set", json!({"ids": [j.to_string()]}));
    assert!(
        s.describe()
            .contains("item junction.open \"Junction sequence…\" enabled")
    );
    done(&mut s, "junction.open", json!({}));
    assert!(
        s.junction_editor().is_some(),
        "one junction selected: the editor opens"
    );
    done(&mut s, "ui.dismiss", json!({}));
    // A placement with the floating ghost, then an undo: no overlay and no preview stand.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.mcb-1p@qet10", "letter": "F"}),
    );
    let p = grid_point(&s, (20, 60));
    cursor(&mut s, p);
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    done(&mut s, "canvas.activate_alt", json!({}));
    let q = grid_point(&s, (24, 60));
    cursor(&mut s, q);
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert!(!s.tool_overlay().is_empty() && s.preview_state().is_some());
    done(&mut s, "edit.undo", json!({}));
    assert!(
        s.tool_overlay().is_empty(),
        "the overlay of the undone revision goes"
    );
    assert!(s.preview_state().is_none(), "and its preview");
}

/// Window check (r23): after the keyboard wiring of folio 103 (W, `]`, Enter, `]`, `]`, Enter, Escape) and after Ctrl+Z no
/// tool mark stands; the keyboard grid cursor stays where it was (WP-S2.03 §7.7: a keyboard user's position does not
/// vanish when a tool ends or a step is undone).
#[test]
fn escape_and_undo_leave_no_tool_mark_and_keep_the_grid_cursor() {
    let tmp = Tmp::new("marks");
    let mut s = shell(&tmp, "103");
    done(&mut s, "tool.wire", json!({}));
    done(&mut s, "cp.next", json!({}));
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "cp.next", json!({}));
    done(&mut s, "cp.next", json!({}));
    done(&mut s, "canvas.activate", json!({}));
    let cursor_at = canvas(&s)["cursor_nm"].clone();
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
    assert!(s.tool_overlay().is_empty(), "no snap mark or ghost");
    assert!(s.snap_indicator().is_none());
    assert_eq!(canvas(&s)["cursor_nm"], cursor_at, "the grid cursor stays");
    done(&mut s, "edit.undo", json!({}));
    assert!(s.tool_overlay().is_empty(), "no mark after the undo");
    assert_eq!(canvas(&s)["cursor_nm"], cursor_at, "the grid cursor stays");
}
