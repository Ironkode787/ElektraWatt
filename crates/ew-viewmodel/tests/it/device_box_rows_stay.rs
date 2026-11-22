// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The pin list of a device box keeps its rows while the same pins are edited (FT-2005): the sample's power supply's
//! box, a pin of its top edge given the bottom edge, keeps its row; selecting another pin sorts the list by the box
//! again, the edited pin among the bottom edge's.

use crate::common::{Tmp, actual_size, done};
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::shell::symbol_editor::side_word;
use serde_json::json;

/// The pin table as (pin index, edge).
fn rows(s: &ShellModel) -> Vec<(u16, &'static str)> {
    s.symbol_editor()
        .and_then(|e| e.device_box())
        .expect("the device box")
        .pin_table()
        .into_iter()
        .map(|l| (l.idx, side_word(&l.side)))
        .collect()
}

#[test]
fn a_pin_given_another_edge_keeps_its_row_while_it_is_edited() {
    let tmp = Tmp::new("device-box-rows-stay");
    let mut s = actual_size(&tmp, "102");
    done(&mut s, "part.box", json!({"part": "EW-GEN-PSU-1"}));
    let before = rows(&s);
    let (pin, _) = *before
        .iter()
        .find(|(_, side)| *side == "top")
        .expect("a pin on the top edge");
    done(&mut s, "part.box_select", json!({"pins": [pin]}));
    done(&mut s, "part.pin", json!({"pin": pin, "side": "bottom"}));
    let edited = rows(&s);
    let order = |r: &[(u16, &str)]| r.iter().map(|(i, _)| *i).collect::<Vec<_>>();
    assert_eq!(order(&edited), order(&before), "the rows keep their places");
    assert!(
        edited.contains(&(pin, "bottom")),
        "the pin is on the bottom edge: {edited:?}"
    );
    // Another pin selected: the list is sorted by the box again, the pin among the bottom edge's.
    let other = before.iter().map(|(i, _)| *i).find(|i| *i != pin).unwrap();
    done(&mut s, "part.box_select", json!({"pins": [other]}));
    let sorted = rows(&s);
    let at = sorted.iter().position(|(i, _)| *i == pin).unwrap();
    assert!(
        sorted[..at]
            .iter()
            .all(|(_, side)| *side == "top" || *side == "bottom"),
        "{sorted:?}"
    );
    assert_ne!(order(&sorted), order(&before), "sorted by the box again");
}
