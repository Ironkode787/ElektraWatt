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

/// A station of a CPU and an input card in a new project of pages 120 and 121, the card drawn on 120 and the station's
/// view open on it; the station.
fn card_on_120(s: &mut ShellModel) -> String {
    for folio in ["120", "121"] {
        let page = json!({"folio": folio, "type": "multi_line", "title": "Control"});
        done(s, "page.new", page);
    }
    done(s, "plc.new_station", json!({}));
    done(s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(s, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    done(s, "plc.rack_create", json!({}));
    done(s, "plc.rack_select", json!({"slot": 1}));
    done(s, "plc.module_draw", json!({"folio": "120"}));
    let station = s.last_station().expect("the station").to_string();
    done(s, "plc.rack_editor", json!({"station": station}));
    done(s, "plc.rack_select", json!({"slot": 1}));
    station
}

/// The entries of the card's Move to page: its pages and the new page, by their words.
fn move_entries(s: &ShellModel) -> Vec<String> {
    ew_viewmodel::actions::station_tools(s)
        .into_iter()
        .find_map(|(_, e)| match e {
            ew_viewmodel::actions::MenuEntry::Submenu { items, .. }
                if items.iter().any(|i| i.id == "plc.module_move") =>
            {
                Some(items.into_iter().map(|i| i.label).collect())
            }
            _ => None,
        })
        .expect("Move to page's entries")
}

/// Persona re-test: Move to page's entries read as moves, not as Draw on page's, leave out the card's own page, and
/// "To a new page" makes the page after the card's, in its section, and moves the card there in one undo step.
#[test]
fn move_to_a_new_page_makes_the_page_and_moves_the_card_in_one_step() {
    let tmp = Tmp::new("card-move-new-page");
    let mut s = new_project(&tmp);
    let station = card_on_120(&mut s);
    let entries = move_entries(&s);
    assert!(
        entries.iter().all(|e| e.starts_with("To ")),
        "the entries read as moves: {entries:?}"
    );
    assert!(
        entries.iter().any(|e| e.starts_with("To page 121")),
        "{entries:?}"
    );
    assert!(
        !entries.iter().any(|e| e.starts_with("To page 120")),
        "not the card's own page: {entries:?}"
    );
    let pages = s.page_rows().len();
    let module = slot_one(&s).0;
    done(&mut s, "plc.module_move", json!({"folio": "new"}));
    let toast = s.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.contains("stands on the new page"), "{toast}");
    assert_eq!(s.page_rows().len(), pages + 1, "a page is made");
    done(&mut s, "plc.rack_editor", json!({"station": station}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    let (after_module, page, _) = slot_one(&s);
    assert_eq!(after_module, module, "the same card");
    let page = page.expect("drawn on a page");
    assert!(toast.contains(&format!("page {page}")), "the toast names {page}: {toast}");
    assert!(page != "120" && page != "121", "a new page: {page}");
    // One undo step takes both back.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.page_rows().len(), pages, "the page goes with one undo");
    s.shutdown();
}

#[test]
fn move_to_page_puts_a_drawn_card_on_another_page_as_the_same_card() {
    let tmp = Tmp::new("card-move-page");
    let mut s = new_project(&tmp);
    let station = card_on_120(&mut s);
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
