// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-2134: a drawn card is put on another page from its station's view (Move to page, `plc.module_move`): its box
//! moves, and the card stays the same module in its slot with its channels and their symbol names; a card whose wire
//! would be left behind is refused in words and stays (doc 10 §6). A persona run found only Go to page there.

use crate::common::{Tmp, done, new_project};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

/// The card in slot 1 of the station on show: its module, the folio it is drawn on and its channels' symbol names.
fn slot_one(s: &ShellModel) -> (Option<String>, Option<String>, Vec<String>) {
    let view = s.shown_rack().expect("the rack");
    let row = view.rows.iter().find(|r| r.slot == 1).expect("slot 1");
    let symbols = s
        .station_shown()
        .expect("the view")
        .channels
        .iter()
        .map(|c| c.symbol.clone())
        .collect();
    (row.module.map(|m| m.to_string()), row.page.clone(), symbols)
}

#[test]
fn move_to_page_puts_a_drawn_card_on_another_page_as_the_same_card() {
    let tmp = Tmp::new("card-move-page");
    let mut s = new_project(&tmp);
    for folio in ["120", "121"] {
        let page = json!({"folio": folio, "type": "multi_line", "title": "Control"});
        done(&mut s, "page.new", page);
    }
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    done(&mut s, "plc.module_draw", json!({"folio": "120"}));
    let station = s.last_station().expect("the station").to_string();
    done(&mut s, "plc.rack_editor", json!({"station": station}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    let channel = s.station_shown().expect("the view").channels[0]
        .channel
        .to_string();
    done(
        &mut s,
        "plc.rack_channel",
        json!({"channel": channel, "column": "symbol"}),
    );
    done(&mut s, "plc.rack_channel", json!({"text": "Door_closed"}));
    let before = slot_one(&s);
    assert_eq!(before.1.as_deref(), Some("120"), "drawn on 120 first");
    done(&mut s, "plc.module_move", json!({"folio": "121"}));
    done(&mut s, "plc.rack_editor", json!({"station": station}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    let after = slot_one(&s);
    assert_eq!(after.1.as_deref(), Some("121"), "drawn on 121 now");
    assert_eq!(
        (&after.0, &after.2),
        (&before.0, &before.2),
        "the same card, its symbol names kept"
    );
    s.shutdown();
}

#[test]
fn a_card_whose_wire_would_be_left_behind_is_refused_in_words_and_stays() {
    let tmp = Tmp::new("card-move-page-wired");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    done(&mut s, "plc.rack_editor", json!({"station": "tag:-104A1"}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    let before = slot_one(&s);
    let out = s.dispatch("plc.module_move", &json!({"folio": "103"}));
    assert!(
        matches!(&out, Outcome::Failed { message } if !message.is_empty()),
        "refused in words: {out:?}"
    );
    assert_eq!(slot_one(&s), before, "the card stays where it was");
    s.shutdown();
}
