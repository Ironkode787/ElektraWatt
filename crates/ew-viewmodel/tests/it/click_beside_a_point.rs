// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1800: a click a few pixels beside a pin or a port became a free point without a word: a start
//! that took nothing, a bend beside the port, and a tube that then ended on the next port clicked. With the Wire tool
//! and the Tube tool a click within ten screen points of a connection point takes it at every zoom, and a click that
//! leaves a free point right beside one says so at once in the status line.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_api::output::{TableCp, TableEnd, TableRep};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// A new project showing a new page `folio` of `kind`.
fn project(tmp: &Tmp, folio: &str, kind: &str) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let dir = tmp.0.join("new.ew");
    done(
        &mut s,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": folio, "type": kind, "title": "Beside"}),
    );
    done(&mut s, "page.goto", json!({"folio": folio}));
    s
}

/// A click at the sheet point `q` as the window sends it.
fn click(s: &mut ShellModel, q: (i64, i64)) {
    for a in ["canvas.pointer", "canvas.press", "canvas.release"] {
        done(s, a, at(q, json!({})));
    }
}

/// A part placed with its anchor at grid index `k`, joined to nothing.
fn place(s: &mut ShellModel, part: &str, k: (i64, i64)) -> TableRep {
    let before = tables(s).reps.clone();
    done(s, "place.pick", json!({"part": part, "template": 0}));
    let sn = &s.canvas().unwrap().page().snapper;
    let q = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
    for a in ["canvas.pointer", "canvas.press", "canvas.release"] {
        done(s, a, at(q, json!({"alt": true})));
    }
    done(s, "tool.select", json!({}));
    tables(s)
        .reps
        .iter()
        .find(|r| before.iter().all(|b| b.id != r.id))
        .cloned()
        .expect("the new symbol")
}

/// The connection point of `rep` that leaves in direction `dir`.
fn pin(s: &ShellModel, rep: &TableRep, dir: ew_api::geometry::Dir) -> TableCp {
    tables(s)
        .cps
        .iter()
        .find(|c| c.rep == rep.id && c.dir == Some(dir))
        .cloned()
        .expect("the pin")
}

/// The sheet point `points` screen points from `cp` along the way it leaves, at the canvas's zoom.
fn off(s: &ShellModel, cp: &TableCp, points: f64) -> (i64, i64) {
    let c = s.canvas().unwrap();
    let p = c.page().snapper.page_to_sheet(cp.at);
    let v = cp.dir.expect("a direction").vector();
    let d = (points / c.points_per_nm()).round() as i64;
    (p.0 + v.0 * d, p.1 + v.1 * d)
}

/// Whether the page's last connection ends at `cp`.
fn last_ends_at(s: &ShellModel, cp: &TableCp) -> bool {
    let end = TableEnd::Cp {
        function: cp.function,
        idx: cp.idx,
    };
    tables(s)
        .connections
        .last()
        .is_some_and(|c| c.a == end || c.b == end)
}

/// The tool (Wire or Tube) from `a` to `b`, then past `b` to `c`, at 100 % and at 200 %: nine points off `b` take it;
/// thirteen points off, the start takes nothing, a bend lies beside it, and the wire ended at `c` bends beside it, each
/// said in the status line.
fn reach_and_beside(s: &mut ShellModel, tool: &str, (a, b, c): (&TableCp, &TableCp, &TableCp)) {
    let sheet = |s: &ShellModel, p: (i64, i64)| s.canvas().unwrap().page().snapper.page_to_sheet(p);
    for percent in [100, 200] {
        let centre = sheet(s, b.at);
        done(
            s,
            "view.zoom_to",
            json!({"percent": percent, "x_mm": centre.0 as f64 / 1e6, "y_mm": centre.1 as f64 / 1e6}),
        );
        let n = tables(s).connections.len();
        done(s, tool, json!({}));
        click(s, sheet(s, a.at));
        click(s, off(s, b, 9.0));
        assert_eq!(tables(s).connections.len(), n + 1, "{tool} at {percent} %");
        assert!(
            last_ends_at(s, b),
            "nine points off the pin take it ({tool}, {percent} %)"
        );
        done(s, "edit.undo", json!({}));
        // A start beside the pin takes nothing and says so.
        done(s, tool, json!({}));
        click(s, off(s, b, 13.0));
        let hint = s.tool_hint();
        // A connection point on a wiring page, a port on a fluid page: the tool's own words.
        assert!(hint.contains("was beside a "), "{hint}");
        // A bend beside the pin says so at once, and the wire ended elsewhere again.
        click(s, sheet(s, a.at));
        click(s, off(s, b, 13.0));
        let hint = s.tool_hint();
        assert!(hint.contains("placed a bend beside a "), "{hint}");
        click(s, sheet(s, c.at));
        assert_eq!(tables(s).connections.len(), n + 1);
        assert!(last_ends_at(s, c));
        let hint = s.tool_hint();
        assert!(
            hint.contains(" just drawn bends beside a "),
            "{hint}"
        );
        // The next wire that starts on a point forgets it.
        click(s, sheet(s, c.at));
        assert!(!s.tool_hint().contains("beside"), "{}", s.tool_hint());
        s.dispatch("ui.dismiss", &json!({}));
        done(s, "tool.select", json!({}));
        done(s, "edit.undo", json!({}));
    }
}

#[test]
fn the_wire_tool_takes_a_pin_ten_points_off_and_says_a_click_beside_one() {
    use ew_api::geometry::Dir;
    let tmp = Tmp::new("beside-wire");
    let mut s = project(&tmp, "102", "multi_line");
    let (one, two, three) = (
        place(&mut s, "EW-GEN-LMP-1", (20, 20)),
        place(&mut s, "EW-GEN-LMP-1", (20, 40)),
        place(&mut s, "EW-GEN-LMP-1", (40, 40)),
    );
    let (a, b, c) = (
        pin(&s, &one, Dir::Down),
        pin(&s, &two, Dir::Up),
        pin(&s, &three, Dir::Up),
    );
    reach_and_beside(&mut s, "tool.wire", (&a, &b, &c));
}

#[test]
fn the_tube_tool_takes_a_port_ten_points_off_and_says_a_click_beside_one() {
    use ew_api::geometry::Dir;
    let tmp = Tmp::new("beside-tube");
    let mut s = project(&tmp, "201", "fluid");
    let (one, two, three) = (
        place(&mut s, "EW-GEN-FCV-1", (20, 20)),
        place(&mut s, "EW-GEN-FCV-1", (40, 30)),
        place(&mut s, "EW-GEN-FCV-1", (40, 45)),
    );
    let (a, b, c) = (
        pin(&s, &one, Dir::Right),
        pin(&s, &two, Dir::Left),
        pin(&s, &three, Dir::Left),
    );
    reach_and_beside(&mut s, "tool.fluid_line", (&a, &b, &c));
}
