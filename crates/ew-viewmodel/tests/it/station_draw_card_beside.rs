// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Draw on a page from the station view sets a card of inputs beside the controller's box, not under it (FT-1894,
//! doc 06a §7.5): the supply wires that come down from the head then cross none of its channels.

use crate::common::{Tmp, done, new_project};
use serde_json::json;

#[test]
fn a_card_of_inputs_stands_beside_the_controller_not_under_it() {
    let tmp = Tmp::new("station-draw-card-beside");
    let mut s = new_project(&tmp);
    let folio = s.canvas().expect("a page on show").page().folio.clone();
    let before: Vec<_> = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id)
        .collect();
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    done(&mut s, "plc.rack_next", json!({"step": "draw", "folio": folio}));
    let page = s.canvas().unwrap().page();
    let spans: Vec<(i64, i64, i64)> = page
        .tables
        .reps
        .iter()
        .filter(|r| !before.contains(&r.id))
        .filter_map(|r| {
            let o = &r.black_box.as_ref()?.outline;
            let x = (o.iter().map(|p| p.0).min()?, o.iter().map(|p| p.0).max()?);
            Some((x.0, x.1, o.iter().map(|p| p.1).min()?))
        })
        .collect();
    // The controller's box at the head, the card's at the foot: the one with the higher top is the controller's.
    let [a, b] = spans.as_slice() else {
        panic!("the controller and its card are drawn: {spans:?}")
    };
    let (cpu, card) = if a.2 < b.2 { (a, b) } else { (b, a) };
    assert!(
        card.0 > cpu.1 || card.1 < cpu.0,
        "the card {card:?} stands beside the controller {cpu:?}, not under it"
    );
}
