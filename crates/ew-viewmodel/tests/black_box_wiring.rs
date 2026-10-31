// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The play-test of cycle 40 through the tools (findings 1 to 4): a black box drawn with two corners from a part,
//! then moved by one grid step with the keyboard move (M), mirrored top to bottom (Y) or turned (R), takes a wire on
//! every pin with the Wire tool (each wire ended in an interruption point with P), and a mirror or a half turn keeps
//! the box where it was.

mod common;

use common::{Tmp, at, done};
use ew_viewmodel::ew_api::geometry::Dir;
use ew_viewmodel::ew_api::output::{PageTables, TableCp};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::{Value, json};

const TWO_FNDEF_PART: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-api/tests/expected/two_fndef_part.json"
);

fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// The sheet point of grid index `k`.
fn grid_sheet(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.sheet_of_index(k.0, 0), sn.sheet_of_index(k.1, 1))
}

fn index_of(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.grid_index_of(p.0), sn.grid_index_of(p.1))
}

/// The grid cursor at the page point `p`.
fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", at(q, json!({})));
}

fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let q = grid_sheet(s, k);
    done(s, "cursor.set", at(q, json!({})));
}

/// Draws the black box of the two-FnDef part with corners at grid indices (60, 40) and (66, 44); returns its ID text.
fn draw_box(s: &mut ShellModel) -> String {
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(TWO_FNDEF_PART).expect("the fixture")).expect("JSON");
    done(
        s,
        "command.run",
        json!({"cmd": "library.embed", "params": {"copies": [fixture]}}),
    );
    done(
        s,
        "place.pick",
        json!({"part": "MICO 4.10 TWO", "template": 0, "letter": "A"}),
    );
    assert_eq!(s.tool().name(), "black_box");
    cursor_k(s, (60, 40));
    done(s, "canvas.activate", json!({}));
    cursor_k(s, (66, 44));
    done(s, "canvas.activate", json!({}));
    done(s, "ui.dismiss", json!({}));
    tables(s)
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the new box")
        .id
        .to_string()
}

fn pins(s: &ShellModel, rep: &str) -> Vec<TableCp> {
    tables(s)
        .cps
        .iter()
        .filter(|c| c.rep.to_string() == rep)
        .cloned()
        .collect()
}

/// The absolute outline's bounding box of the black box.
fn bbox(s: &ShellModel, rep: &str) -> ((i64, i64), (i64, i64)) {
    let t = tables(s);
    let r = t.reps.iter().find(|r| r.id.to_string() == rep).expect("the box");
    let o = &r.black_box.as_ref().expect("a box").outline;
    let xs = o.iter().map(|p| p.0);
    let ys = o.iter().map(|p| p.1);
    (
        (xs.clone().min().unwrap(), ys.clone().min().unwrap()),
        (xs.max().unwrap(), ys.max().unwrap()),
    )
}

/// For every pin of the box: the Wire tool from the pin to the grid point three steps out along the pin's direction,
/// ended in an interruption point with P; each is one commit whose wire starts exactly at the pin.
fn wire_every_pin(s: &mut ShellModel, rep: &str, what: &str) {
    // From right to left: an interruption point's name stands to its right, so the next wire's end stays clear of it.
    let mut order = pins(s, rep);
    order.sort_by_key(|c| (std::cmp::Reverse(c.at.0), c.at.1));
    assert!(!order.is_empty(), "{what}: the box has pins");
    for pin in order {
        done(s, "tool.wire", json!({}));
        cursor(s, pin.at);
        done(s, "canvas.activate", json!({}));
        assert_eq!(s.tool().state(), "routing", "{what}: pin {}", pin.label);
        let k = index_of(s, pin.at);
        let (dx, dy) = match pin.dir {
            Some(Dir::Up) => (0, -3),
            Some(Dir::Down) => (0, 3),
            Some(Dir::Left) => (-3, 0),
            _ => (3, 0),
        };
        cursor_k(s, (k.0 + dx, k.1 + dy));
        let before = tables(s);
        let rev = s.revision();
        done(s, "tool.ip", json!({}));
        assert_eq!(
            s.revision(),
            rev.map(|r| r + 1),
            "{what}: the wire from pin {} at {:?} is one commit",
            pin.label,
            pin.at
        );
        let after = tables(s);
        let wire = after
            .connections
            .iter()
            .find(|c| !before.connections.iter().any(|b| b.id == c.id))
            .expect("the new wire");
        assert_eq!(wire.polyline.first(), Some(&pin.at), "{what}: pin {}", pin.label);
        done(s, "ui.dismiss", json!({}));
    }
}

fn select(s: &mut ShellModel, rep: &str) {
    done(s, "select.set", json!({ "ids": [rep] }));
}

#[test]
fn a_black_box_moved_one_step_with_m_takes_a_wire_on_every_pin() {
    for (dx, dy) in [(1, 0), (0, 1), (2, 0)] {
        let tmp = Tmp::new("bb-move");
        let mut s = shell(&tmp, "101");
        let rep = draw_box(&mut s);
        let before = bbox(&s, &rep);
        select(&mut s, &rep);
        cursor_k(&mut s, (60, 40));
        done(&mut s, "tool.move", json!({}));
        cursor_k(&mut s, (60 + dx, 40 + dy));
        done(&mut s, "canvas.activate", json!({}));
        let after = bbox(&s, &rep);
        assert_ne!(before, after, "the box moved by ({dx}, {dy})");
        wire_every_pin(&mut s, &rep, &format!("moved by ({dx}, {dy})"));
    }
}

#[test]
fn a_black_box_mirrored_or_turned_stays_in_place_and_takes_a_wire_on_every_pin() {
    for key in ["tool.mirror_y", "tool.mirror_x", "tool.rotate_cw"] {
        let tmp = Tmp::new("bb-turn");
        let mut s = shell(&tmp, "101");
        let rep = draw_box(&mut s);
        let (min0, max0) = bbox(&s, &rep);
        select(&mut s, &rep);
        done(&mut s, key, json!({}));
        let (min1, max1) = bbox(&s, &rep);
        if key == "tool.rotate_cw" {
            // About the centre: the sides swap and the centre stays within a grid step.
            assert_eq!(
                (max1.0 - min1.0, max1.1 - min1.1),
                (max0.1 - min0.1, max0.0 - min0.0)
            );
            let step = 2_645_834;
            assert!(((min1.0 + max1.0) - (min0.0 + max0.0)).abs() <= step, "{key}");
            assert!(((min1.1 + max1.1) - (min0.1 + max0.1)).abs() <= step, "{key}");
        } else {
            for (a, b) in [
                (min0.0, min1.0),
                (min0.1, min1.1),
                (max0.0, max1.0),
                (max0.1, max1.1),
            ] {
                assert!(
                    (a - b).abs() <= 1,
                    "{key}: {min0:?} {max0:?} became {min1:?} {max1:?}"
                );
            }
        }
        done(&mut s, "ui.dismiss", json!({}));
        wire_every_pin(&mut s, &rep, key);
    }
}
