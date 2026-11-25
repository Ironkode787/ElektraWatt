// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! An interruption point dropped on a drawn box's pin never lies along or in the box (doc 06 §3.5): EW's two
//! forms point right whatever the side, so on the lower side the point stands a grid step below the pin and on the
//! upper side two steps above it, joined by a straight wire; a target on the left side and a source on the right side
//! stand on the pin; a source on the left side and a target on the right side, which would point into the box, are
//! refused in words.

use crate::common;

use common::{Tmp, actual_size, at, cursor, done, grid_point, refused, select_ids, sheet, tables, toast};
use ew_api::output::{TableCp, TableEnd};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// A new page 150 on the QET grid of the sample's copy with a box drawn without a part from grid (30, 30) to (42, 38)
/// and its pins: `t` at (34, 30) on the upper side, `b` at (34, 38) on the lower, `l` and `l2` at (30, 32) and
/// (30, 36) on the left, `r` and `r2` at (42, 32) and (42, 36) on the right.
fn box_with_a_pin_on_each_side(tmp: &Tmp) -> ShellModel {
    let mut s = actual_size(tmp, "102");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {
            "number": 150, "title": "Box", "kind": "multi_line", "grid": {"num": 7_937_500, "den": 3},
        }}),
    );
    done(&mut s, "page.goto", json!({"folio": "150"}));
    done(&mut s, "view.zoom_actual", json!({}));
    done(
        &mut s,
        "tool.black_box",
        json!({"symbol": "ew.gen.blackbox@qet10", "letter": "A"}),
    );
    for corner in [(30, 30), (42, 38)] {
        let p = grid_point(&s, corner);
        cursor(&mut s, p);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "tool.select", json!({}));
    let rep = tables(&s)
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the box")
        .id;
    for (name, k) in [
        ("t", (34, 30)),
        ("b", (34, 38)),
        ("l", (30, 32)),
        ("l2", (30, 36)),
        ("r", (42, 32)),
        ("r2", (42, 36)),
    ] {
        select_ids(&mut s, &[rep.to_string()]);
        let p = sheet(&s, grid_point(&s, k));
        done(&mut s, "box.add_cp", at(p, json!({})));
        done(&mut s, "text.input", json!({ "text": name }));
        done(&mut s, "tool.finish", json!({}));
    }
    s
}

/// The box's pin `name`.
fn pin(s: &ShellModel, name: &str) -> TableCp {
    tables(s)
        .cps
        .iter()
        .find(|c| c.label.ends_with(&format!(":{name}")))
        .unwrap_or_else(|| panic!("pin {name}"))
        .clone()
}

/// Arms the IP tool with the point `name` of `direction` and puts the grid cursor where its CP lies on `pin`: a
/// source's insertion point (its tip) one grid step right of its CP at the tail, a target's at its CP.
fn arm_on(s: &mut ShellModel, name: &str, direction: &str, pin: (i64, i64)) {
    done(s, "tool.ip", json!({}));
    done(s, "tool.ip_direction", json!({ "direction": direction }));
    done(s, "ip.name", json!({ "text": name }));
    let sn = s.canvas().unwrap().page().snapper.clone();
    let k = (sn.grid_index_of(pin.0), sn.grid_index_of(pin.1));
    let tip = if direction == "source" { (k.0 + 1, k.1) } else { k };
    let p = grid_point(s, tip);
    cursor(s, p);
}

/// The connection point of the interruption point `name` and the route of the wire from the pin `from` to it.
fn placed(s: &ShellModel, name: &str, from: &TableCp) -> ((i64, i64), Vec<(i64, i64)>) {
    let t = tables(s);
    let rep = t
        .reps
        .iter()
        .find(|r| r.ip.as_ref().is_some_and(|i| i.name == name))
        .unwrap_or_else(|| panic!("the point {name}"));
    let cp = t
        .cps
        .iter()
        .find(|c| c.rep == rep.id)
        .expect("its connection point");
    let wire = t
        .connections
        .iter()
        .find(|c| {
            [&c.a, &c.b].iter().any(|e| {
                matches!(e, TableEnd::Cp { function, idx } if *function == from.function && *idx == from.idx)
            })
        })
        .expect("the wire from the pin");
    (cp.at, wire.polyline.clone())
}

#[test]
fn a_point_on_a_box_s_upper_or_lower_side_stands_off_it_with_a_wire() {
    let tmp = Tmp::new("ip-box-sides-stand-off");
    let mut s = box_with_a_pin_on_each_side(&tmp);
    let (b, t) = (pin(&s, "b"), pin(&s, "t"));
    // Below the lower side: one step down, for a source as for a target.
    arm_on(&mut s, "OUT", "source", b.at);
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    let below = grid_point(&s, (34, 39));
    assert_eq!(placed(&s, "OUT", &b), (below, vec![b.at, below]));
    // Above the upper side: two steps up, the cross-reference under the arrow between it and the box.
    arm_on(&mut s, "IN", "target", t.at);
    // The ghost stands where the point will, before the click.
    let above = grid_point(&s, (34, 28));
    assert_eq!(
        common::canvas(&s)["ghost"]["at_nm"],
        json!([above.0, above.1]),
        "the ghost stands above the box"
    );
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(placed(&s, "IN", &t), (above, vec![t.at, above]));
}

#[test]
fn a_point_on_a_box_s_left_or_right_side_stands_on_the_pin_or_is_refused() {
    let tmp = Tmp::new("ip-box-sides-left-right");
    let mut s = box_with_a_pin_on_each_side(&tmp);
    let (l, l2, r, r2) = (pin(&s, "l"), pin(&s, "l2"), pin(&s, "r"), pin(&s, "r2"));
    // A target's arrow left of the left side and a source's right of the right side: on the pin.
    arm_on(&mut s, "LIN", "target", l.at);
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(placed(&s, "LIN", &l).0, l.at);
    arm_on(&mut s, "ROUT", "source", r.at);
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(placed(&s, "ROUT", &r).0, r.at);
    // The other two would point into the box: refused in words, nothing committed.
    for (name, direction, pin, words) in [
        (
            "LOUT",
            "source",
            &l2,
            "a source's wire leaves its arrow at the tail, on the left",
        ),
        (
            "RIN",
            "target",
            &r2,
            "a target's wire leaves its arrow at the tip, on the right",
        ),
    ] {
        arm_on(&mut s, name, direction, pin.at);
        let rev = s.revision();
        assert_eq!(
            refused(&mut s, "canvas.activate", json!({})),
            "reason-ip-not-placed",
            "{name}"
        );
        assert_eq!(s.revision(), rev, "{name}: nothing committed");
        let notice = toast(&s);
        assert!(
            notice.starts_with(&format!("Not placed: {words}")) && notice.contains("would lie in the box"),
            "{name}: {notice}"
        );
        done(&mut s, "ui.dismiss", json!({}));
        done(&mut s, "ui.dismiss", json!({}));
    }
}
