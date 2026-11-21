// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The boxes of a device with several bus interfaces on a bus-topology page (FT-1766, doc 10 §2.7): while one box for
//! such a device is not built (FT-1904), its boxes move together, and a row of them placed near the page's edge stands
//! inside the drawing area; a row that would cover another device is refused in words.

use std::collections::BTreeMap;

use crate::common::{Tmp, actual_size, at, cursor, done, grid_point, refused, sheet, tables};
use ew_viewmodel::ew_api::output::{TableRep, Uuid};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The interface boxes of the page shown, by device.
fn boxes(s: &ShellModel) -> BTreeMap<Uuid, Vec<TableRep>> {
    let mut out: BTreeMap<Uuid, Vec<TableRep>> = BTreeMap::new();
    for r in tables(s).reps.iter() {
        if let (Some(d), Some(_), "topology") = (r.device, &r.black_box, r.rep_type.as_str()) {
            out.entry(d).or_default().push(r.clone());
        }
    }
    out
}

/// The bounds `[x0, y0, x1, y1]` of a box's outline.
fn bounds(r: &TableRep) -> [i64; 4] {
    let o = &r.black_box.as_ref().expect("a box").outline;
    o.iter().fold([i64::MAX, i64::MAX, i64::MIN, i64::MIN], |a, p| {
        [a[0].min(p.0), a[1].min(p.1), a[2].max(p.0), a[3].max(p.1)]
    })
}

/// The page point `k` grid steps from `p` along x.
fn steps_right(s: &ShellModel, p: (i64, i64), k: i64) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(sn.grid_index_of(p.0) + k), p.1)
}

/// The sample's controller -104A1 stands on its PROFIBUS page as two boxes, DP and X1. Dragging one by three grid
/// steps moves both by them, in one command.
#[test]
fn the_boxes_of_a_device_s_interfaces_move_together() {
    let tmp = Tmp::new("ports-move-together");
    let mut s = actual_size(&tmp, "105");
    let (device, pair) = boxes(&s)
        .into_iter()
        .find(|(_, b)| b.len() == 2)
        .expect("the controller's two interface boxes");
    let b = bounds(&pair[0]);
    let centre = ((b[0] + b[2]) / 2, (b[1] + b[3]) / 2);
    let to = steps_right(&s, centre, 3);
    let dx = to.0 - centre.0;
    let rev = s.revision();
    let (from, to_sheet) = (sheet(&s, centre), sheet(&s, to));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to_sheet, json!({})));
    done(&mut s, "canvas.release", at(to_sheet, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one rep.move");
    let after = boxes(&s).remove(&device).expect("the boxes");
    for before in &pair {
        let now = after.iter().find(|r| r.id == before.id).expect("each box");
        assert_eq!(
            (now.pos.0 - before.pos.0, now.pos.1 - before.pos.1),
            (dx, 0),
            "each box three steps right"
        );
    }
}

/// A new controller with two interfaces, placed from the device browser on the PROFIBUS page by two corners close to
/// the drawing area's right edge: its second box would run past the edge, so the row of boxes stands inside the area.
/// Drawn over the sample's controller it is refused in words, and nothing is placed.
#[test]
fn a_row_of_interface_boxes_stays_on_the_page_and_off_another_device() {
    let tmp = Tmp::new("ports-place-inside");
    let mut s = actual_size(&tmp, "105");
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("the frame");
    let before = boxes(&s);
    let k = |s: &ShellModel, x: i64| s.canvas().unwrap().page().snapper.grid_index_of(x);
    // Over the sample's controller: refused, the tool stays on its second corner.
    let held = before
        .values()
        .find(|b| b.len() == 2)
        .and_then(|b| b.first())
        .map(bounds)
        .expect("the controller's box");
    let (hx, hy) = (k(&s, held[0]), k(&s, held[1]));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "EW-GEN-CPU-2"}));
    assert_eq!(s.tool().name(), "black_box");
    let corner = grid_point(&s, (hx + 1, hy + 1));
    cursor(&mut s, corner);
    done(&mut s, "canvas.activate", json!({}));
    let corner = grid_point(&s, (hx + 5, hy + 4));
    cursor(&mut s, corner);
    assert_eq!(
        refused(&mut s, "canvas.activate", json!({})),
        "reason-ports-occupied"
    );
    assert_eq!(boxes(&s).len(), before.len(), "nothing placed");
    done(&mut s, "ui.dismiss", json!({}));
    // Near the right edge, in the empty foot of the page.
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "EW-GEN-CPU-2"}));
    let (right, foot) = (k(&s, area[2]), k(&s, area[3]));
    let corner = grid_point(&s, (right - 6, foot - 8));
    cursor(&mut s, corner);
    done(&mut s, "canvas.activate", json!({}));
    let corner = grid_point(&s, (right - 2, foot - 4));
    cursor(&mut s, corner);
    done(&mut s, "canvas.activate", json!({}));
    // A new station opens its station view; the page is shown again.
    done(&mut s, "page.goto", json!({"folio": "105"}));
    let placed: Vec<TableRep> = boxes(&s)
        .into_iter()
        .filter(|(d, _)| !before.contains_key(d))
        .flat_map(|(_, b)| b)
        .collect();
    assert_eq!(placed.len(), 2, "the new controller's DP and X1");
    // In grid steps: the area's edges lie on grid lines that the QET grid's thirds of a nanometre round apart.
    let edges = area.map(|v| k(&s, v));
    for r in &placed {
        let b = bounds(r).map(|v| k(&s, v));
        assert!(
            edges[0] <= b[0] && b[2] <= edges[2] && edges[1] <= b[1] && b[3] <= edges[3],
            "{b:?} inside the drawing area {edges:?}"
        );
    }
    assert!(
        placed.iter().any(|r| k(&s, bounds(r)[2]) == edges[2]),
        "the row stands at the right edge, moved in as far as it ran past"
    );
}
