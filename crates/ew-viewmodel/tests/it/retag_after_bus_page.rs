// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A station placed first on a bus page takes its tag from that page; its box placed later on a schematic page takes
//! that page's, as a device's tag names the page that draws its main function (FT-1766). The tag changes, and the toast
//! says so with both tags and why.

use crate::common::{Tmp, cursor, done, grid_point, new_project, tables, toast};
use serde_json::json;

/// Two corners of a box at grid indices `a` and `b` of the page shown.
fn two_corners(s: &mut ew_viewmodel::shell::ShellModel, a: (i64, i64), b: (i64, i64)) {
    for k in [a, b] {
        let p = grid_point(s, k);
        cursor(s, p);
        done(s, "canvas.activate", json!({}));
    }
}

#[test]
fn a_station_s_box_on_a_schematic_page_says_its_new_tag() {
    let tmp = Tmp::new("retag-after-bus-page");
    let mut s = new_project(&tmp);
    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "bus_topology", "title": "Bus"}),
    );
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "EW-GEN-CPU-2"}));
    two_corners(&mut s, (10, 10), (18, 16));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let station = tables(&s)
        .reps
        .iter()
        .find_map(|r| r.device.filter(|_| r.rep_type == "topology"))
        .expect("the station's interface boxes");
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "plc.module_place", json!({"device": station.to_string()}));
    two_corners(&mut s, (10, 10), (24, 22));
    let said = toast(&s);
    let (pair, why) = said.split_once(": ").unwrap_or_else(|| panic!("{said:?}"));
    let (was, now) = pair.split_once(" is now ").unwrap_or_else(|| panic!("{said:?}"));
    assert_ne!(was, now, "{said}");
    assert_eq!(
        why,
        "a device takes its tag from the page that draws its own box, not from a bus page, which shows its interfaces."
    );
}
