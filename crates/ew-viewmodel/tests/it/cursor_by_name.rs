// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `cursor.set` names its place by a pin or by a device's symbol as well as by millimetres, so that a script that
//! names them reaches them wherever the page's layout has put them; a name the page shown does not draw is refused in
//! words.

use crate::common;

use common::{Tmp, at, done, failed, tables};
use serde_json::json;

#[test]
fn the_cursor_goes_to_a_pin_or_a_symbol_by_its_name() {
    let tmp = Tmp::new("cursor-by-name");
    let mut s = tmp.sample_shell("102");
    let t = tables(&s);
    let cp = t
        .cps
        .iter()
        .find(|c| c.label == "-102F1:2")
        .expect("-102F1:2 on 102");
    let rep = t.reps.iter().find(|r| r.id == cp.rep).expect("its symbol");
    let cursor = |s: &ew_viewmodel::shell::ShellModel| s.canvas().unwrap().cursor();
    // The tables are in page nm, the cursor in sheet nm.
    let snapper = &s.canvas().unwrap().page().snapper;
    let (pin_at, symbol_at) = (snapper.page_to_sheet(cp.at), snapper.page_to_sheet(rep.pos));

    // A pin by its name: where its millimetres put the cursor.
    done(&mut s, "cursor.set", at(pin_at, json!({})));
    let by_mm = cursor(&s);
    assert!(by_mm.is_some());
    done(&mut s, "cursor.set", json!({}));
    assert_eq!(cursor(&s), None);
    done(&mut s, "cursor.set", json!({"pin": "-102F1:2"}));
    assert_eq!(cursor(&s), by_mm);

    // A device by its tag: its symbol's insertion point.
    done(&mut s, "cursor.set", at(symbol_at, json!({})));
    let by_mm = cursor(&s);
    done(&mut s, "cursor.set", json!({}));
    done(&mut s, "cursor.set", json!({"tag": "-102F1"}));
    assert_eq!(cursor(&s), by_mm);

    // What the page shown does not draw, and two ways at once, in words.
    assert_eq!(
        failed(&mut s, "cursor.set", json!({"pin": "-102F1:9"})),
        "No pin \"-102F1:9\" is drawn on the page shown; name one as the page text does: -102F1:2"
    );
    assert_eq!(
        failed(&mut s, "cursor.set", json!({"tag": "-101F1"})),
        "No symbol of -101F1 is drawn on the page shown"
    );
    assert_eq!(
        failed(&mut s, "cursor.set", at(cp.at, json!({"pin": "-102F1:2"}))),
        "Name the place one way: x_mm and y_mm, a pin or a tag"
    );
    assert_eq!(cursor(&s), by_mm, "a refused step leaves the cursor");
}
