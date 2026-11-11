// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A terminal placed where a terminal of its strip already stands lands two grid steps
//! to the right. The ghost shows where it lands, not the click, and the "Placed" toast
//! says why it moved, in en-GB and nl-NL. On a writable copy of Golden A's folio 103: terminal 70 of -X1031 at grid
//! index (60, 28), then terminals 71 and 72 of -X1031 clicked on the same point.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::tools::{Ghost, OverlayItem};
use serde_json::json;

/// The page point of grid index `k` on the shown page.
fn page_of(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// Picks terminal `designation` of -X1031 and puts the grid cursor at grid index `k`.
fn hover(s: &mut ShellModel, designation: &str, k: (i64, i64)) {
    done(
        s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "terminal_designation": designation}),
    );
    let p = page_of(s, k);
    let q = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", at(q, json!({})));
}

/// A page point, if any.
type Point = Option<(i64, i64)>;

/// Where the ghost stands: its insertion point and the outline's.
fn ghost_at(s: &ShellModel) -> (Point, Point) {
    let at = match s.tool_ghost() {
        Some(Ghost::Place { at_nm, .. }) => at_nm,
        _ => None,
    };
    let outline = s.tool_overlay().items.iter().find_map(|i| match i {
        OverlayItem::Paths { transform, .. } => Some(transform.1),
        _ => None,
    });
    (at, outline)
}

fn toast(s: &ShellModel) -> String {
    s.toast().expect("the placement's toast").text.clone()
}

fn placed(s: &mut ShellModel, designation: &str, k: (i64, i64)) {
    hover(s, designation, k);
    done(s, "canvas.activate_alt", json!({}));
    done(s, "tool.select", json!({}));
}

fn language(s: &mut ShellModel, value: &str) {
    done(s, "view.language", json!({ "value": value }));
}

#[test]
fn a_terminal_moved_off_its_strip_s_terminal_shows_where_it_lands_and_says_why() {
    let tmp = Tmp::new("ft1208");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    placed(&mut s, "70", (60, 28));
    assert_eq!(toast(&s), "Placed -X1031 terminal 70 at 103-D9.");

    // The click point is taken by terminal 70: the ghost stands two grid steps right, where the terminal lands.
    hover(&mut s, "71", (60, 28));
    let landing = page_of(&s, (62, 28));
    assert_eq!(ghost_at(&s), (Some(landing), Some(landing)));
    done(&mut s, "canvas.activate_alt", json!({}));
    done(&mut s, "tool.select", json!({}));
    let tables = &s.canvas().unwrap().page().tables;
    assert!(
        tables.reps.iter().any(|r| r.pos == landing && r.terminal),
        "terminal 71 lands where its ghost stood"
    );
    assert_eq!(
        toast(&s),
        "Placed -X1031 terminal 71 at 103-D10, 2 grid steps right: -X1031 has a terminal on the click point."
    );

    // Two terminals stand on the way: four steps, in Dutch.
    language(&mut s, "nl");
    placed(&mut s, "72", (60, 28));
    assert_eq!(
        toast(&s),
        "-X1031 klem 72 geplaatst op 103-D10, 4 rasterstappen naar rechts: -X1031 heeft een klem op het klikpunt."
    );
    language(&mut s, "en");

    // A free point: the ghost stands on it and the toast says nothing more.
    hover(&mut s, "73", (60, 40));
    let free = page_of(&s, (60, 40));
    assert_eq!(ghost_at(&s), (Some(free), Some(free)));
    done(&mut s, "canvas.activate_alt", json!({}));
    assert_eq!(toast(&s), "Placed -X1031 terminal 73 at 103-E9.");
}
