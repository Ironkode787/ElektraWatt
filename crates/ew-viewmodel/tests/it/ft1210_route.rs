// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1210 (play-test rv-vis-8, t0053): a short wire from an interruption point down into a terminal takes no loop.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::ew_api::geometry::Dir;
use ew_viewmodel::ew_api::output::{PageTables, TableConnection, TableCp, TableRep};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

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
        json!({"folio": "101", "type": "multi_line", "title": "Supply"}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "view.zoom_actual", json!({}));
    done(&mut s, "strip.new", json!({"letter": "X"}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

fn page_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

fn click(s: &mut ShellModel, p: (i64, i64), alt: bool) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "canvas.pointer", at(q, json!({ "alt": alt })));
    done(s, "canvas.press", at(q, json!({ "alt": alt })));
    done(s, "canvas.release", at(q, json!({ "alt": alt })));
}

fn new_rep(s: &ShellModel, before: &[TableRep]) -> TableRep {
    let t = tables(s);
    let new: Vec<&TableRep> = t
        .reps
        .iter()
        .filter(|r| before.iter().all(|b| b.id != r.id))
        .collect();
    assert_eq!(new.len(), 1, "one new representation");
    new[0].clone()
}

fn cps_of(s: &ShellModel, rep: &TableRep) -> Vec<TableCp> {
    tables(s)
        .cps
        .iter()
        .filter(|c| c.rep == rep.id)
        .cloned()
        .collect()
}

fn wire(s: &mut ShellModel, a: (i64, i64), b: (i64, i64)) -> TableConnection {
    let before = tables(s);
    done(s, "tool.wire", json!({}));
    click(s, a, false);
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(b);
    done(s, "canvas.pointer", at(q, json!({})));
    click(s, b, false);
    let after = tables(s);
    let new: Vec<TableConnection> = after
        .connections
        .iter()
        .filter(|c| before.connections.iter().all(|o| o.id != c.id))
        .cloned()
        .collect();
    assert_eq!(new.len(), 1, "one new connection");
    new[0].clone()
}

/// An interruption point named L1 at grid index (40, 20), its arrow turned `quarter_turns` times clockwise, and a
/// terminal of a new strip placed free `dy` grid steps below the point's CP, in line with it; the point's CP and the
/// terminal's upper CP.
fn ip_over_terminal(tmp: &Tmp, quarter_turns: usize, dy: i64) -> (ShellModel, TableCp, TableCp) {
    let mut s = new_project(tmp);
    let before = tables(&s).reps.clone();
    done(&mut s, "tool.ip", json!({}));
    done(&mut s, "ip.name", json!({"text": "L1"}));
    for _ in 0..quarter_turns {
        done(&mut s, "tool.rotate_cw", json!({}));
    }
    let p = page_point(&s, (40, 20));
    click(&mut s, p, true);
    done(&mut s, "tool.select", json!({}));
    let ip = new_rep(&s, &before);
    let ip_cp = cps_of(&s, &ip)[0].clone();
    let before = tables(&s).reps.clone();
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-?X?", "terminal_designation": "1"}),
    );
    let sn = s.canvas().unwrap().page().snapper.clone();
    let k = (sn.grid_index_of(ip_cp.at.0), sn.grid_index_of(ip_cp.at.1) + dy);
    let q = page_point(&s, k);
    click(&mut s, q, true);
    done(&mut s, "tool.select", json!({}));
    let terminal = new_rep(&s, &before);
    let top = cps_of(&s, &terminal).into_iter().min_by_key(|c| c.at.1).unwrap();
    (s, ip_cp, top)
}

/// The vertices of a polyline with consecutive duplicates removed.
fn vertices(line: &[(i64, i64)]) -> Vec<(i64, i64)> {
    let mut v = line.to_vec();
    v.dedup();
    v
}

/// The play-test's case: the point's arrow as placed, its CP leaving up, and the terminal below it. The wire from the
/// point down into the terminal is the straight run between the two CPs, drawn from either end; before, it left up
/// and came back, a loop or a refused run back over itself.
#[test]
fn ft1210_a_short_wire_from_an_interruption_point_down_into_a_terminal_takes_no_loop() {
    for dy in [3, 4, 6] {
        let tmp = Tmp::new("ft1210-down");
        let (mut s, ip, top) = ip_over_terminal(&tmp, 0, dy);
        assert_eq!(ip.dir, Some(Dir::Up), "the arrow as placed");
        let c = wire(&mut s, ip.at, top.at);
        assert_eq!(
            vertices(&c.polyline),
            [ip.at, top.at],
            "{dy} steps, from the point"
        );
        let tmp = Tmp::new("ft1210-up");
        let (mut s, ip, top) = ip_over_terminal(&tmp, 0, dy);
        let c = wire(&mut s, top.at, ip.at);
        assert_eq!(
            vertices(&c.polyline),
            [top.at, ip.at],
            "{dy} steps, from the terminal"
        );
    }
}

/// A point turned to leave down, towards the terminal, keeps its direction: the straight run, as before.
#[test]
fn ft1210_a_point_that_leaves_towards_the_terminal_routes_straight_as_before() {
    let tmp = Tmp::new("ft1210-turned");
    let (mut s, ip, top) = ip_over_terminal(&tmp, 2, 4);
    assert_eq!(ip.dir, Some(Dir::Down));
    let c = wire(&mut s, ip.at, top.at);
    assert_eq!(vertices(&c.polyline), [ip.at, top.at]);
}
