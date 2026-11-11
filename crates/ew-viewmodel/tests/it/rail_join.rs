// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1084 and FT-1096 (ID-1181, the cycle-46 play-test): a press of the Wire tool within reach of a rail but not of
//! its end's connection point, beside the end's grid point, joins the rail at the next grid point along it with a
//! junction, rather than fixing a bend there that leaves the wire to end elsewhere; and the T-join the tool makes on
//! the default template has its junction and the junction's dot. The situation is built through the window's own
//! path in a new project at actual size: a switch above a coil, one vertical rail between them, a second coil beside
//! the rail whose pin starts the joining wire.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::ew_api::output::ew_dl::{EntityRef, Item, ItemKind};
use ew_viewmodel::ew_api::output::{TableCp, TableRep};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// A new project (the built-in template) with the multi-line page 101 shown at actual size.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// The page point of grid index `k`.
fn page_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// Places `pick` with its insertion point at grid index `k`, without autoconnect; returns its representation.
fn place(s: &mut ShellModel, pick: serde_json::Value, k: (i64, i64)) -> TableRep {
    let before: Vec<TableRep> = tables(s).reps.clone();
    done(s, "place.pick", pick);
    let sheet = s.canvas().unwrap().page().snapper.page_to_sheet(page_point(s, k));
    done(s, "cursor.set", at(sheet, json!({})));
    done(s, "canvas.activate_alt", json!({}));
    done(s, "tool.select", json!({}));
    let t = tables(s);
    let new: Vec<&TableRep> = t
        .reps
        .iter()
        .filter(|r| before.iter().all(|b| b.id != r.id))
        .collect();
    assert_eq!(new.len(), 1, "one new representation");
    new[0].clone()
}

/// The CPs of `rep`, top first.
fn cps_of(s: &ShellModel, rep: &TableRep) -> Vec<TableCp> {
    let mut cps: Vec<TableCp> = tables(s)
        .cps
        .iter()
        .filter(|c| c.rep == rep.id)
        .cloned()
        .collect();
    cps.sort_by_key(|c| (c.at.1, c.at.0));
    cps
}

/// A pointer click at the sheet point `q` as the window sends it: the pointer there, the press, the release.
fn click_sheet(s: &mut ShellModel, q: (i64, i64)) {
    done(s, "canvas.pointer", at(q, json!({})));
    done(s, "canvas.press", at(q, json!({})));
    done(s, "canvas.release", at(q, json!({})));
}

fn click(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    click_sheet(s, q);
}

/// Whether the display list draws an item of `entity` (groups opened).
fn draws(items: &[Item], entity: EntityRef) -> bool {
    items.iter().any(|i| {
        i.entity == Some(entity)
            || match &i.kind {
                ItemKind::Group(g) => draws(&g.items, entity),
                _ => false,
            }
    })
}

/// The rail: a switch at grid index (60, 20) above a coil at (60, 40), its make contact's lower pin wired to the
/// coil's upper pin in one straight vertical connection; a second coil at (70, 30). Returns the shell, the rail's
/// lower end (the coil's pin) and the second coil's upper pin.
fn rail(tmp: &Tmp) -> (ShellModel, (i64, i64), TableCp) {
    let mut s = new_project(tmp);
    let switch = place(
        &mut s,
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "S"}),
        (60, 20),
    );
    let coil = place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        (60, 40),
    );
    let other = place(
        &mut s,
        json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}),
        (70, 30),
    );
    let (top, bottom) = (
        cps_of(&s, &switch)
            .last()
            .cloned()
            .expect("the switch's lower pin"),
        cps_of(&s, &coil).first().cloned().expect("the coil's upper pin"),
    );
    assert_eq!(top.at.0, bottom.at.0, "in one vertical line");
    done(&mut s, "tool.wire", json!({}));
    click(&mut s, top.at);
    click(&mut s, bottom.at);
    let t = tables(&s);
    assert_eq!(t.connections.len(), 1, "the rail");
    let line = &t.connections[0].polyline;
    assert!(line.iter().all(|p| p.0 == top.at.0), "straight: {line:?}");
    let start = cps_of(&s, &other)
        .first()
        .cloned()
        .expect("the second coil's upper pin");
    (s, bottom.at, start)
}

/// The sheet point of a press beside the rail's lower end: 0.45 of a grid step up along the rail, so the end's grid
/// point is the rail's nearest, and to the right far enough to miss the end's CP but within reach of the rail.
/// Returns it with the rail's grid point one step up from the end.
fn beside_the_end(s: &ShellModel, end: (i64, i64)) -> ((i64, i64), (i64, i64)) {
    let c = s.canvas().unwrap();
    let sn = &c.page().snapper;
    let (ex, ey) = (sn.grid_index_of(end.0), sn.grid_index_of(end.1));
    let up = (end.0, sn.page_of_index(ey - 1));
    assert_eq!(sn.page_of_index(ex), end.0, "the end is on the grid");
    let (e, u) = (sn.page_to_sheet(end), sn.page_to_sheet(up));
    let step = (e.1 - u.1).abs() as f64;
    let reach = 8.0 / c.points_per_nm();
    let along = 0.45 * step;
    // Half way between the CP's reach (seen from the press, `along` up) and the rail's.
    let off = 0.5 * ((reach * reach - along * along).max(0.0).sqrt() + reach);
    assert!(
        along * along + off * off > reach * reach && off < reach,
        "the geometry: step {step}, reach {reach}"
    );
    let dir = (u.1 - e.1).signum() as f64;
    let press = (e.0 + off.round() as i64, e.1 + (dir * along).round() as i64);
    (press, up)
}

#[test]
fn a_press_beside_a_rail_ends_grid_point_joins_the_rail_with_a_junction() {
    let tmp = Tmp::new("ft1084-join");
    let (mut s, end, start) = rail(&tmp);
    let (press, joined) = beside_the_end(&s, end);
    let rev = s.revision();
    click(&mut s, start.at);
    assert_eq!(s.tool().state(), "routing");
    click_sheet(&mut s, press);
    assert_eq!(
        (s.tool().state(), s.revision()),
        ("armed", rev.map(|r| r + 1)),
        "the press ended the wire"
    );
    let t = tables(&s);
    let at: Vec<(i64, i64)> = t.junctions.iter().map(|j| j.at).collect();
    assert_eq!(at, [joined], "a junction one grid step up the rail");
    assert_eq!(t.connections.len(), 3, "the rail's two halves and the new wire");
}

#[test]
fn a_t_join_the_wire_tool_makes_has_its_junction_dot() {
    let tmp = Tmp::new("ft1096-dot");
    let (mut s, end, start) = rail(&tmp);
    let (_, joined) = beside_the_end(&s, end);
    let mid = (joined.0, s.canvas().unwrap().page().snapper.page_of_index(30));
    click(&mut s, start.at);
    click(&mut s, mid);
    let t = tables(&s);
    assert_eq!(t.junctions.len(), 1, "{:?}", t.junctions);
    let j = &t.junctions[0];
    assert_eq!((j.at, j.style.as_str()), (mid, "dot"));
    let list = std::sync::Arc::clone(&s.canvas().unwrap().page().list);
    assert!(draws(&list.items, EntityRef(j.id)), "the junction's dot is drawn");
}
