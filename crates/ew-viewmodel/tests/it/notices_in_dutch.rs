// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A command's notices (`Outcome.notices`, what it dropped instead of refusing) carry a text key, so a Dutch window
//! words them in Dutch (FT-1819); they read in English before.

use crate::common::{Tmp, at, done, failed, state, tables};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

const COIL: &str = "ew.iec.coil@qet10";
const NO: &str = "ew.iec.contact-no@qet10";

/// Places `symbol` as a new device with `letter` at grid index `k` of folio 101 and returns its printed tag.
fn new_device(s: &mut ShellModel, symbol: &str, letter: &str, k: (i64, i64)) -> String {
    done(s, "place.pick", json!({"symbol": symbol, "letter": letter}));
    let p = {
        let sn = &s.canvas().unwrap().page().snapper;
        sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)))
    };
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
    done(s, "ui.dismiss", json!({}));
    let sn = &s.canvas().unwrap().page().snapper;
    let pos = (sn.page_of_index(k.0), sn.page_of_index(k.1));
    let t = tables(s);
    let rep = t.reps.iter().find(|r| r.pos == pos).expect("the new symbol");
    let label = &t.cps.iter().find(|c| c.rep == rep.id).unwrap().label;
    label.rsplit_once(':').unwrap().0.to_owned()
}

/// A contact with a function text joined to a relay in a Dutch window: the toast names what went with the joined
/// device in Dutch.
#[test]
fn a_join_toast_names_what_went_with_the_device_in_dutch() {
    let tmp = Tmp::new("notices-in-dutch");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "view.language", json!({"value": "nl"}));
    let relay = new_device(&mut s, COIL, "K", (60, 30));
    let own = new_device(&mut s, NO, "K", (66, 30));
    done(&mut s, "select.set", json!({"refs": [format!("tag:{own}")]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/function_text", "value": "Pomp aan"}),
    );
    let _ = failed(
        &mut s,
        "props.set",
        json!({"row": "device/derived.tag", "value": relay}),
    );
    done(&mut s, "props.accept_tag_proposal", json!({}));
    assert_eq!(s.recorded_commands().last().unwrap().cmd, "rep.join_device");
    let toast = state(&s)["toast"]["text"].as_str().unwrap_or_default().to_owned();
    assert!(
        toast.contains("functietekst van het samengevoegde apparaat is ermee vervallen"),
        "{toast}"
    );
    assert!(!toast.contains("dropped"), "{toast}");
}
