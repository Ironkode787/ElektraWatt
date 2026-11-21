// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drag that starts on a pin's label of a symbol that is not selected moves the label alone, as a drag at its tag
//! does (doc 06 §2.5); the symbol stays where it was.

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

const MM: i64 = 1_000_000;

#[test]
fn a_pin_s_label_drags_alone_without_its_symbol_selected() {
    let tmp = Tmp::new("pin-label-drag");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    let page = shell.canvas().unwrap().page().clone();
    // A pin's label of a symbol (the text of a connection point's designation) that lies on no other text.
    let label = *page
        .texts
        .iter()
        .find(|t| {
            t.pin
                && page
                    .texts
                    .iter()
                    .filter(|o| o.rect.max().x.0 >= t.rect.min().x.0 && o.rect.min().x.0 <= t.rect.max().x.0)
                    .filter(|o| o.rect.max().y.0 >= t.rect.min().y.0 && o.rect.min().y.0 <= t.rect.max().y.0)
                    .count()
                    == 1
        })
        .expect("a pin's label on the page");
    let rep = page
        .tables
        .reps
        .iter()
        .find(|r| r.id == label.rep)
        .expect("its symbol")
        .clone();
    let r = label.rect;
    let grab = page
        .snapper
        .page_to_sheet(((r.min().x.0 + r.max().x.0) / 2, (r.min().y.0 + r.max().y.0) / 2));
    done(&mut shell, "select.set", json!({"ids": []}));
    let to = (grab.0 + 10 * MM, grab.1 + 5 * MM);
    done(&mut shell, "canvas.press", at(grab, json!({})));
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    assert_eq!(shell.tool().state(), "dragging");
    done(&mut shell, "canvas.release", at(to, json!({})));
    let now = shell.canvas().unwrap().page().clone();
    let moved = now
        .texts
        .iter()
        .find(|t| t.rep == rep.id && t.slot == label.slot)
        .expect("the label still prints");
    assert!(
        moved.moved && moved.at.x.0 > label.at.x.0 + 8 * MM,
        "the label moved alone: {:?} → {:?}",
        label.at,
        moved.at
    );
    let after = now.tables.reps.iter().find(|r| r.id == rep.id).expect("the symbol");
    assert_eq!(after.pos, rep.pos, "its symbol stayed");
    shell.shutdown();
}
