// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1049 on a temporary copy of Golden A: a routed wire whose coil end lies off the sheet is
//! drawn along the sheet's edge by the scene's clamp, where normaliser 7 does not split it. The page tables name
//! that stretch; the Place tool proposes no split on it and refuses a drop with a connection point strictly inside
//! it, in words; a symbol placed there by a command is treated as before (the wire is not split).

use crate::common;

use common::{Tmp, at, done, refused};
use ew_viewmodel::ew_api::output::{PageTables, TableConnection, TableEnd};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

const REFUSAL: &str = "Not placed: this wire runs off the sheet here; move its ends onto the sheet first";

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// The grid cursor at the page point `p`.
fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", at(q, json!({})));
}

/// The page point of grid index `k` on the shown page.
fn page_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The labels of a connection's two ends.
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

/// The coil's wire (its A1 to the switch's 14).
fn coil_wire(t: &PageTables) -> TableConnection {
    t.connections
        .iter()
        .find(|c| {
            let mut e = ends(t, c);
            e.sort();
            e == ["-103K1:A1".to_owned(), "-103S1:14".to_owned()]
        })
        .expect("the coil's wire")
        .clone()
}

/// Page 103 with a switch at grid (60, 20), a coil autoconnected under it, the coil then moved off the sheet's left
/// edge by a command: the wire's stored route follows the coil and the scene draws its off-sheet part along the edge
/// (also FT-1074's bare end). (A wire without a route is unrouted and has no wire geometry, .)
pub(crate) fn fixture(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    for (pick, k, activate) in [
        (
            json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
            (60, 20),
            "canvas.activate_alt",
        ),
        (
            json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
            (60, 40),
            "canvas.activate",
        ),
    ] {
        done(&mut s, "place.pick", pick);
        let p = page_point(&s, k);
        cursor(&mut s, p);
        done(&mut s, activate, json!({}));
        done(&mut s, "tool.select", json!({}));
    }
    // The autoconnected wire stores its straight run.
    let t = tables(&s);
    assert!(coil_wire(&t).stored_route, "the autoconnected wire has a route");
    let coil = t
        .cps
        .iter()
        .find(|c| c.label == "-103K1:A1")
        .expect("the coil's A1")
        .rep;
    done(
        &mut s,
        "command.run",
        json!({"cmd": "rep.move", "params": {"reps": [coil.to_string()], "steps": [-80, 0]}}),
    );
    s
}

/// The stretch the clamp drew: its two ends, the one nearer the top first.
fn stretch(t: &PageTables) -> ((i64, i64), (i64, i64)) {
    let w = coil_wire(t);
    assert!(w.stored_route);
    assert_eq!(w.off_sheet.len(), 1, "one stretch along the edge: {w:?}");
    let i = w.off_sheet[0];
    let (a, b) = (w.polyline[i], w.polyline[i + 1]);
    assert_eq!(a.0, b.0, "a vertical stretch along the left edge: {w:?}");
    if a.1 <= b.1 { (a, b) } else { (b, a) }
}

#[test]
fn the_place_tool_refuses_a_drop_on_the_edge_stretch_and_proposes_no_split() {
    let tmp = Tmp::new("ft1049-place");
    let mut s = fixture(&tmp);
    let t = tables(&s);
    let (top, bottom) = stretch(&t);
    let wires = t.connections.len();
    // A contact half-way down the stretch: both its CPs lie strictly inside it, facing its ends.
    let mid = (top.0, top.1 + (bottom.1 - top.1) / 2);
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
    );
    cursor(&mut s, mid);
    assert!(s.tool().name() == "place", "the Place tool floats the contact");
    // A dwell previews nothing there, as the click places nothing.
    let calls = s.session_calls();
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(s.session_calls().preview_with, calls.preview_with, "no preview");
    let rev = s.revision();
    assert_eq!(
        refused(&mut s, "canvas.activate", json!({})),
        "reason-place-wire-off-sheet"
    );
    assert_eq!(s.tool_notice(), Some(REFUSAL));
    assert_eq!(s.revision(), rev, "nothing placed");
    assert_eq!(tables(&s).connections.len(), wires);
}

#[test]
fn a_symbol_placed_there_by_a_command_is_treated_as_before() {
    let tmp = Tmp::new("ft1049-command");
    let mut s = fixture(&tmp);
    let t = tables(&s);
    let (top, bottom) = stretch(&t);
    let before = coil_wire(&t);
    let wires = t.connections.len();
    // The command a placement elsewhere sends, sent again at the stretch's middle.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
    );
    let p = page_point(&s, (100, 60));
    cursor(&mut s, p);
    done(&mut s, "canvas.activate_alt", json!({}));
    done(&mut s, "tool.select", json!({}));
    let mut params = s
        .recorded_commands()
        .last()
        .expect("the placement")
        .params
        .clone();
    // The grid point nearest the stretch's middle (the page's connection grid, num / den nm a step).
    let (num, den) = (i128::from(t.grid.0), i128::from(t.grid.1));
    let k = (i128::from(top.1 + (bottom.1 - top.1) / 2) * den + num / 2) / num;
    let mid = (
        top.0,
        s.canvas()
            .unwrap()
            .page()
            .snapper
            .page_of_index(i64::try_from(k).unwrap()),
    );
    assert!(top.1 < mid.1 && mid.1 < bottom.1, "inside the stretch");
    params["pos"] = json!([mid.0, mid.1]);
    params.as_object_mut().unwrap().remove("connect");
    let rev = s.revision();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "device.place", "params": params}),
    );
    assert_ne!(s.revision(), rev, "the command places it");
    let after = tables(&s);
    assert_eq!(after.connections.len(), wires, "no split");
    let wire = coil_wire(&after);
    assert_eq!(
        (wire.id, wire.polyline.clone()),
        (before.id, before.polyline.clone())
    );
}
