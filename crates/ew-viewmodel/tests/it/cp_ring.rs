// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The persona runs of the third 0.3.0 candidate: with the page fitted to the window a connection point is one or two
//! pixels, and wiring meant zooming in. With Select, a connection point within a few screen points of the pointer
//! shows its ring and the bar says how a wire starts there; with Wire, the click there starts the wire at that point.

use crate::common;

use common::{Tmp, at, done, tables};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::tools::{MarkerKind, OverlayItem};
use serde_json::json;

fn rings(s: &ShellModel) -> usize {
    s.tool_overlay()
        .items
        .iter()
        .filter(|i| {
            matches!(
                i,
                OverlayItem::Marker {
                    kind: MarkerKind::Cp,
                    ..
                }
            )
        })
        .count()
}

#[test]
fn a_connection_point_at_the_fitted_zoom_shows_its_ring_and_starts_a_wire() {
    let tmp = Tmp::new("cp-ring");
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
        "place.pick",
        json!({"part": "EW-GEN-MCB-1", "template": 0}),
    );
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(30), sn.page_of_index(20)))
    };
    common::click(&mut s, p, json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "select.set", json!({"ids": []}));
    done(&mut s, "view.fit_page", json!({}));
    let cp = tables(&s)
        .cps
        .first()
        .cloned()
        .expect("the breaker's first point");
    let (sheet, nm_per_point) = {
        let c = s.canvas().unwrap();
        let page = c.page();
        (page.snapper.page_to_sheet(cp.at), 1.0 / c.points_per_nm())
    };
    // Five screen points beside it.
    let off = (5.0 * nm_per_point) as i64;
    let near = (sheet.0 + off, sheet.1);
    done(&mut s, "canvas.pointer", at(near, json!({})));
    assert_eq!(rings(&s), 1, "the ring");
    assert_eq!(
        ew_viewmodel::command_bar::hud(&s).prompt,
        "A wire starts at this connection point: W, then a click on it"
    );
    // Thirty points away: no ring.
    done(
        &mut s,
        "canvas.pointer",
        at((sheet.0 + 6 * off, sheet.1), json!({})),
    );
    assert_eq!(rings(&s), 0);
    // W, then a click five points beside the point, starts the wire at it.
    done(&mut s, "tool.wire", json!({}));
    common::click(&mut s, near, json!({}));
    assert_eq!(s.tool().state(), "routing", "the wire starts");
    s.shutdown();
}
