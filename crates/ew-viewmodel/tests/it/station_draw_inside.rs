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
        // Nor across a wire the page drew before it (the 0 V and 24 V lines run under the cards).
        let (x0, x1) = (outline.iter().map(|p| p.0).min().unwrap(), outline.iter().map(|p| p.0).max().unwrap());
        let (y0, y1) = (outline.iter().map(|p| p.1).min().unwrap(), bottom);
        for wire in &page.tables.connections {
            for s in wire.polyline.windows(2) {
                let across = s[0].0.min(s[1].0) <= x1
                    && x0 <= s[0].0.max(s[1].0)
                    && s[0].1.min(s[1].1) <= y1
                    && y0 <= s[0].1.max(s[1].1);
                assert!(!across, "the box {outline:?} stands across the wire {:?}", wire.polyline);
            }
        }
    }
}
