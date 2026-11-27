// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device box of a PLC card (doc 07 §2.5): its pins come from its channels, so their names and what they
//! carry are the part's; where each pin stands on the box, and the box's size, are the box's, edited there as on any
//! box and drawn that way when the card is placed.

use crate::common::{Tmp, actual_size, cursor, done, failed, grid_point, new_project, tables};
use ew_api::geometry::Dir;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::symbol_editor::side_word;
use ew_viewmodel::strings;
use serde_json::json;

/// The pin table of the open device box as (index, name, edge).
fn pins(s: &ShellModel) -> Vec<(u16, String, &'static str)> {
    s.symbol_editor()
        .and_then(|e| e.device_box())
        .expect("the device box")
        .pin_table()
        .into_iter()
        .map(|l| (l.idx, l.name, side_word(&l.side)))
        .collect()
}

/// A card of four digital inputs made in the test, its pins 1 to 4 by the pattern `{n+1}`, opened in its device box:
/// a pin keeps its name, which its channel gives, and goes to the left edge; the box is made larger; saved, the part
/// lays its box out so, and one click of the Device tool places the card with that pin on its left edge.
#[test]
fn a_card_s_pins_take_their_places_on_its_box_and_keep_their_names() {
    let tmp = Tmp::new("device-box-card");
    let mut s = new_project(&tmp);
    done(&mut s, "parts.new", json!({"start": "module"}));
    for (field, value) in [
        ("manufacturer", "Example"),
        ("part_number", "DI-4"),
        ("plc.kind", "di"),
        ("plc.count", "4"),
        ("plc.pin_of", "{n+1}"),
    ] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "part.box", json!({"part": "DI-4"}));
    let first = pins(&s)
        .into_iter()
        .find(|p| p.1 == "1")
        .expect("pin 1, the first channel's");
    assert_ne!(first.2, "left", "no pin stands on the left edge yet");
    // Its name is its channel's.
    let why = s
        .symbol_editor()
        .and_then(|e| e.device_box())
        .and_then(|b| b.pins_from())
        .expect("the sentence");
    assert_eq!(
        failed(&mut s, "part.pin", json!({"pin": first.0, "name": "X1"})),
        why
    );
    assert_eq!(failed(&mut s, "part.pin", json!({"add": true})), why);
    // Its place is the box's.
    done(
        &mut s,
        "part.pin",
        json!({"pin": first.0, "side": "left", "at": 8}),
    );
    done(&mut s, "part.box_size", json!({"width": 24, "height": 16}));
    assert!(pins(&s).contains(&(first.0, "1".to_owned(), "left")));
    done(&mut s, "symbol.save", json!({}));
    let saved = s
        .symbol_editor()
        .and_then(|e| e.device_box())
        .expect("the saved box");
    assert!(!saved.dirty(), "saved");
    let layout = saved.draft.box_.clone().expect("the part lays its box out");
    assert!(
        layout
            .pins
            .iter()
            .any(|p| p.idx == first.0 && side_word(&p.side) == "left"),
        "{layout:?}"
    );
    let (copy, template) = (saved.draft.copy, saved.draft.main_entry());
    done(&mut s, "symbol.close", json!({}));
    done(
        &mut s,
        "tool.black_box",
        json!({"part": copy.to_string(), "template": template}),
    );
    let at = grid_point(&s, (20, 20));
    cursor(&mut s, at);
    done(&mut s, "canvas.activate", json!({}));
    let t = tables(&s);
    let b = t
        .reps
        .iter()
        .find_map(|r| r.black_box.clone())
        .expect("the card's box");
    let left: Vec<u16> = b.cps.iter().filter(|c| c.2 == Dir::Left).map(|c| c.0).collect();
    assert_eq!(left, [first.0], "pin 1 on the left edge: {b:?}");
    assert_eq!(b.cps.len(), 4, "every channel's pin");
}

/// A starter card's box nobody changed is saved as nothing: Save says the part is unchanged, the project keeps the
/// library's card without a layout (Draw on a page still draws it as a card, doc 06a §7.5) and makes no step. One pin
/// put on the left edge, and Save writes the layout.
#[test]
fn an_untouched_card_s_box_saves_nothing_and_a_placed_pin_lays_it_out() {
    let tmp = Tmp::new("device-box-card-untouched");
    let mut s = actual_size(&tmp, "102");
    done(&mut s, "part.box", json!({"part": "EW-GEN-DI-1"}));
    let laid_out = |s: &ShellModel| {
        s.symbol_editor()
            .and_then(|e| e.device_box())
            .expect("the device box")
            .draft
            .box_
            .is_some()
    };
    assert!(!laid_out(&s), "the starter card lays out no box");
    let rev = s.revision();
    let said = failed(&mut s, "symbol.save", json!({}));
    assert!(
        said.ends_with(strings::text(ew_viewmodel::key!("reason-part-unchanged"))),
        "{said}"
    );
    assert!(!laid_out(&s), "Save wrote no layout");
    assert_eq!(s.revision(), rev, "Save made no step");
    let first = pins(&s).into_iter().next().expect("a channel's pin");
    done(
        &mut s,
        "part.pin",
        json!({"pin": first.0, "side": "left", "at": 8}),
    );
    done(&mut s, "symbol.save", json!({}));
    assert!(laid_out(&s), "the placed pin is the card's layout");
    assert_ne!(s.revision(), rev);
}
