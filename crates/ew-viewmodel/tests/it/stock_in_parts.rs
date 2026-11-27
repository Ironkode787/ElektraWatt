// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The workshop stock in Parts (doc 07 §5.10): Edit stock… opens Parts on the sample's stock in its order; on a stock
//! part's sheet it moves, goes out and comes back, each one undo step; a part the stock does not hold is refused in
//! words; Cut to is a row of the project's Workshop settings.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::ew_api::parts::PartScope;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

/// The stock's part numbers as the list shows them, in its order.
fn stock(s: &ShellModel) -> Vec<String> {
    let place = s.parts_place().expect("Parts");
    assert_eq!(place.scope, PartScope::Stock);
    place.found.iter().map(|f| f.offer.part_number.clone()).collect()
}

/// The line of `describe` that says where the open part stands in the stock.
fn stock_line(s: &ShellModel) -> String {
    s.describe()
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("stock "))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_stock_is_changed_on_a_parts_sheet_one_step_each() {
    let tmp = Tmp::new("stock-in-parts");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "parts.stock", json!({}));
    let before = stock(&s);
    assert_eq!(before.first().map(String::as_str), Some("EW-GEN-FER-0.5"));
    let first = s.parts_place().unwrap().found[0].param();
    done(&mut s, "parts.open", json!({"part": first}));
    assert!(stock_line(&s).starts_with("stock 1 of "), "{}", stock_line(&s));
    // Down one place among the ferrules: one undo step takes it back.
    done(&mut s, "parts.stock", json!({"op": "move", "to": 2}));
    assert_eq!(stock(&s)[1], before[0]);
    assert!(stock_line(&s).starts_with("stock 2 of "));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(stock(&s), before);
    // Out of the stock, and back after the last of its kind.
    done(&mut s, "parts.stock", json!({"op": "remove"}));
    assert!(!stock(&s).contains(&before[0]));
    assert_eq!(stock_line(&s), "stock out");
    done(&mut s, "parts.stock", json!({"op": "add"}));
    assert!(stock(&s).contains(&before[0]));
    let words = match s.dispatch("parts.stock", &json!({"op": "add", "part": "EW-GEN-MCB-1"})) {
        Outcome::Refused { reason } => ew_viewmodel::strings::render(&reason),
        other => panic!("{other:?}"),
    };
    assert!(words.starts_with("EW-GEN-MCB-1 is no wire, ferrule, marker or cable gland"), "{words}");
    // Cut to: a row of the project's Workshop settings.
    let cut = |value: serde_json::Value| {
        json!({"cmd": "project.settings.set", "params": {"member": "workshop.cut_increment", "value": value}})
    };
    done(&mut s, "command.run", cut(json!(10_000_000)));
    assert_ne!(s.dispatch("command.run", &cut(json!(0))), Outcome::Done);
    s.shutdown();
}
