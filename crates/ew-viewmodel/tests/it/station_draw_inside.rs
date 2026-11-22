// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw on a page from the station view keeps a station's CPU on the sheet (FT-1907): a new station drawn on the
//! sample's PLC page 104, whose cards stand down to the drawing area's foot, gets its CPU's box inside the drawing
//! area, not below the frame.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_cpu_drawn_on_a_full_page_stands_inside_its_drawing_area() {
    let tmp = Tmp::new("station-draw-inside");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    let before: Vec<_> = s.canvas().unwrap().page().tables.reps.iter().map(|r| r.id).collect();
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "plc.rack_next", json!({"step": "draw", "folio": "104"}));
    let page = s.canvas().unwrap().page();
    let area = ew_viewmodel::tools::ip::drawing_area(page).expect("the frame's drawing area");
    let new: Vec<_> = page.tables.reps.iter().filter(|r| !before.contains(&r.id)).collect();
    assert!(!new.is_empty(), "the CPU is drawn on 104");
    for rep in new {
        let outline = &rep.black_box.as_ref().expect("the CPU's box").outline;
        let bottom = outline.iter().map(|p| p.1).max().unwrap_or(rep.body[3]);
        assert!(
            bottom <= area[3] && rep.body[1] >= area[1],
            "the box {:?} stands inside the drawing area {area:?}",
            outline
        );
    }
}
