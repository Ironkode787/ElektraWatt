// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-622 (the orchestrator's ruling): a terminal placed on a page point where a terminal of the same strip already
//! sits moves to the next free point to the right, two grid steps along, as Golden A's 1X1 draws its three terminals of
//! one cell 20 QET px apart; a terminal of another strip is not moved.

use crate::common;

use common::{Tmp, done, state};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The page point of grid index `k` on the shown page.
fn page_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The insertion point of the representation whose connection points print `label` (a terminal's with its clamp, `/a`).
fn pos_of(s: &ShellModel, label: &str) -> (i64, i64) {
    let t = &s.canvas().unwrap().page().tables;
    let cp = t
        .cps
        .iter()
        .find(|c| c.label == label || c.label.starts_with(&format!("{label}/")))
        .unwrap_or_else(|| {
            let labels: Vec<&str> = t.cps.iter().map(|c| c.label.as_str()).collect();
            panic!("no CP {label}: {labels:?}")
        });
    t.reps.iter().find(|r| r.id == cp.rep).unwrap().pos
}

/// Places a terminal of `strip` with `designation` at grid index `k` without autoconnect; the toast it gave.
fn place(s: &mut ShellModel, strip: &str, designation: &str, k: (i64, i64)) -> String {
    done(
        s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": strip, "terminal_designation": designation}),
    );
    let p = page_point(s, k);
    let sheet = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(s, "cursor.set", common::at(sheet, json!({})));
    done(s, "canvas.activate_alt", json!({}));
    let toast = state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned();
    done(s, "tool.select", json!({}));
    toast
}

#[test]
fn terminals_of_one_strip_on_one_point_stack_two_grid_steps_apart() {
    let tmp = Tmp::new("ft622");
    let mut s = tmp.shell_in(Mode::Headless, true, "103");
    done(&mut s, "view.zoom_actual", json!({}));
    let k = (110, 30);
    let p = page_point(&s, k);
    // Two grid steps along per terminal already there, by grid index (QET's 10 px is a fractional step in nm).
    for (i, d) in ["50", "51", "52"].into_iter().enumerate() {
        let toast = place(&mut s, "-X1031", d, k);
        assert!(
            toast.starts_with(&format!("Placed -X1031 terminal {d}")),
            "{toast}"
        );
        let at = pos_of(&s, &format!("-X1031:{d}"));
        assert_eq!(at, page_point(&s, (k.0 + 2 * i as i64, k.1)), "-X1031:{d}");
    }
    // A terminal of another strip of folio 103 on the same point is not moved.
    let toast = place(&mut s, "-X1032", "60", k);
    assert!(toast.starts_with("Placed -X1032 terminal 60"), "{toast}");
    assert_eq!(pos_of(&s, "-X1032:60"), p, "-X1032 is not moved");
    // A device placed on the stacked point is not moved either: only a terminal stacks.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    let sheet = s.canvas().unwrap().page().snapper.page_to_sheet(p);
    done(&mut s, "cursor.set", common::at(sheet, json!({})));
    done(&mut s, "canvas.activate_alt", json!({}));
    let t = &s.canvas().unwrap().page().tables;
    assert_eq!(
        t.reps.iter().filter(|r| r.pos == p).count(),
        3,
        "-X1031:50, -X1032:60 and the contact"
    );
}
