// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire at a pin of a corrected card stays at that pin (doc 10 §2.6): a card of one's own with four inputs
//! and its supply pins L+ and M, its L+ wired, corrected to six inputs, so the pattern numbers its pins again (L+ was
//! the fifth pin and is the seventh); the wire still ends at L+, not at the input that took L+'s old number.

use crate::common::{Tmp, cursor, done, grid_point, new_project, tables};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The page point of the connection point labelled with the designation `pin` of a device of the page shown.
fn pin_at(s: &ShellModel, device: &str, pin: &str) -> (i64, i64) {
    let t = tables(s);
    let label = format!("{device}:{pin}");
    t.cps
        .iter()
        .find(|c| c.label == label)
        .map(|c| c.at)
        .unwrap_or_else(|| {
            panic!(
                "no {label}: {:?}",
                t.cps.iter().map(|c| &c.label).collect::<Vec<_>>()
            )
        })
}

/// The labels of both ends of every wire of the page shown.
fn wire_ends(s: &ShellModel) -> Vec<Vec<String>> {
    let t = tables(s);
    let label = |end: &ew_viewmodel::ew_api::output::TableEnd| match end {
        ew_viewmodel::ew_api::output::TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone())
            .unwrap_or_default(),
        _ => String::new(),
    };
    t.connections
        .iter()
        .map(|c| {
            let mut ends = vec![label(&c.a), label(&c.b)];
            ends.sort();
            ends
        })
        .collect()
}

#[test]
fn a_wire_keeps_its_pin_when_the_card_s_pins_are_numbered_again() {
    let tmp = Tmp::new("corrected-card-wires");
    let mut s = new_project(&tmp);
    done(&mut s, "part.new", json!({"plc": "module"}));
    for (field, value) in [
        ("manufacturer", "Example"),
        ("part_number", "DI-4S"),
        ("plc.kind", "di"),
        ("plc.count", "4"),
        ("plc.pin_of", "{n+1}"),
        ("plc.l_plus", "L+"),
        ("plc.m", "M"),
    ] {
        done(&mut s, "part.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "part.save", json!({}));
    // The card drawn by two corners, a push button beside it.
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "DI-4S"}));
    for k in [(20, 20), (40, 30)] {
        let p = grid_point(&s, k);
        cursor(&mut s, p);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "tool.select", json!({}));
    let p = grid_point(&s, (60, 50));
    cursor(&mut s, p);
    done(&mut s, "tool.place", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-PB-1", "letter": "S"}),
    );
    done(&mut s, "canvas.activate", json!({"alt": true}));
    done(&mut s, "tool.select", json!({}));
    let card = tables(&s)
        .cps
        .iter()
        .find(|c| c.label.ends_with(":L+"))
        .and_then(|c| c.label.split_once(':').map(|(d, _)| d.to_owned()))
        .expect("the card's L+");
    let button = tables(&s)
        .cps
        .iter()
        .find(|c| c.label.ends_with(":13"))
        .and_then(|c| c.label.split_once(':').map(|(d, _)| d.to_owned()))
        .expect("the button's 13");
    // L+ wired to the button.
    let (from, to) = (pin_at(&s, &card, "L+"), pin_at(&s, &button, "13"));
    done(&mut s, "tool.wire", json!({}));
    for at in [from, to] {
        cursor(&mut s, at);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "tool.select", json!({}));
    let mut wired = vec![format!("{button}:13"), format!("{card}:L+")];
    wired.sort();
    assert_eq!(wire_ends(&s), [wired.clone()], "the wire as drawn");
    // Six inputs: the pins are numbered again.
    done(&mut s, "part.edit", json!({"part": "DI-4S"}));
    done(&mut s, "part.field", json!({"field": "plc.count", "value": "6"}));
    done(&mut s, "part.save", json!({}));
    done(&mut s, "preview.apply", json!({}));
    done(&mut s, "page.goto", json!({"folio": "101"}));
    assert_eq!(wire_ends(&s), [wired], "the wire still ends at L+");
}
