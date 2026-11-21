// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawn box follows its corrected part (FT-1766, doc 10 §2.6): a switch of one's own drawn on a bus page with its
//! six sockets, corrected to eight, shows the two new sockets on its box at once, after the last one of their edge;
//! corrected to four, its box keeps no mark of the sockets it lost.

use crate::common::{Tmp, cursor, done, grid_point, new_project, tables};
use ew_viewmodel::ew_api::output::TableBox;
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The one interface box on the page shown, its sockets (index, point, direction) by index.
fn sockets(s: &ShellModel) -> TableBox {
    let t = tables(s);
    let mut b = t
        .reps
        .iter()
        .filter(|r| r.rep_type == "topology")
        .find_map(|r| r.black_box.clone())
        .expect("the switch's box");
    b.cps.sort_by_key(|c| c.0);
    b
}

/// Edit part… on SW-6, its port count set to `ports`, saved, and the update applied.
fn correct(s: &mut ShellModel, ports: &str) {
    done(s, "part.edit", json!({"part": "SW-6"}));
    done(s, "part.field", json!({"field": "bus.0.ports", "value": ports}));
    done(s, "part.save", json!({}));
    done(s, "preview.apply", json!({}));
    done(s, "page.goto", json!({"folio": "102"}));
}

#[test]
fn a_corrected_part_s_drawn_box_takes_the_sockets_it_has_now() {
    let tmp = Tmp::new("corrected-part-sockets");
    let mut s = new_project(&tmp);
    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "bus_topology", "title": "Network"}),
    );
    done(&mut s, "part.new", json!({}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", "SW-6")] {
        done(&mut s, "part.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "part.bus_row", json!({}));
    for (field, value) in [
        ("bus.0.interface", "X1"),
        ("bus.0.bus", "profinet"),
        ("bus.0.role", "passive"),
        ("bus.0.ports", "6"),
    ] {
        done(&mut s, "part.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "part.save", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "SW-6"}));
    for k in [(10, 10), (22, 16)] {
        let p = grid_point(&s, k);
        cursor(&mut s, p);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let six = sockets(&s).cps;
    assert_eq!(six.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
    // Eight: the two new sockets stand at once, after socket 6 on its edge, leaving as it does, on the box.
    correct(&mut s, "8");
    let TableBox {
        cps: eight, outline, ..
    } = sockets(&s);
    assert_eq!(
        eight.iter().map(|c| c.0).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
    let kept = |i: usize| six.iter().find(|c| c.0 == eight[i].0).map(|c| c.1);
    assert!(
        (0..6).all(|i| kept(i) == Some(eight[i].1)),
        "the six keep their places"
    );
    let last = six[5];
    for new in &eight[6..] {
        assert_eq!(new.2, last.2, "{new:?} leaves as socket 6 does");
        assert_eq!(new.1.1, last.1.1, "{new:?} on socket 6's edge");
        assert!(new.1.0 > last.1.0, "{new:?} after socket 6");
    }
    let right = outline.iter().map(|p| p.0).max().unwrap_or(0);
    assert!(eight[7].1.0 < right, "the box grew to take them");
    let points: std::collections::BTreeSet<(i64, i64)> = eight.iter().map(|c| c.1).collect();
    assert_eq!(points.len(), 8, "no two sockets on one point");
    // Four: the sockets it lost leave the box.
    correct(&mut s, "4");
    let four = sockets(&s).cps;
    assert_eq!(four.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 1, 2, 3]);
}
