// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawn card is put on another page from its station's view (Move to page, `plc.module_move`): its box
//! moves, and the card stays the same module in its slot with its channels and their symbol names; a wire to what
//! stays behind is kept as a pair of interruption points, which a preview lists before anything moves (doc 10 §6,
//! doc 06 §3.4), and the selected symbols of a page move the same way (Page ▸ Move to page…).

use crate::common::{Tmp, done, new_project, refused};
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

/// Move to page's entries read as moves, not as Draw on page's, leave out the card's own page, and
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
    assert!(
        toast.contains(&format!("page {page}")),
        "the toast names {page}: {toast}"
    );
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

/// The pairs the preview lists, one line each.
fn pairs(s: &ShellModel) -> Vec<String> {
    s.preview_pane()
        .notice_words()
        .into_iter()
        .filter(|n| n.contains("is kept as the pair of interruption points"))
        .collect()
}

/// The interruption points the page `folio` draws.
fn points_on(s: &mut ShellModel, folio: &str) -> usize {
    done(s, "page.goto", json!({"folio": folio}));
    let page = s.canvas().expect("the page").page();
    page.tables.reps.iter().filter(|r| r.ip.is_some()).count()
}

/// A wired card's Move to page takes the move of the selected symbols: on the sample's PLC page its supply wire runs
/// through a chain of junctions, which the move refuses in words naming the wire, and the card stays.
#[test]
fn a_card_whose_wire_runs_through_two_junctions_is_refused_in_words_and_stays() {
    let tmp = Tmp::new("card-move-page-wired");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    done(&mut s, "plc.rack_editor", json!({"station": "tag:-104A1"}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    let before = slot_one(&s);
    let out = s.dispatch("plc.module_move", &json!({"folio": "103"}));
    assert!(
        matches!(&out, Outcome::Failed { message } if message.contains("runs through more than one junction")),
        "refused in words: {out:?}"
    );
    assert_eq!(slot_one(&s), before, "the card stays where it was");
    s.shutdown();
}

/// Move to page… started without its page (its button, the right-click menu, the command search) asks for it: the
/// palette's form offers the other schematic pages, not the page shown, and a new page. It moves the selected symbols:
/// the first symbol of the sample's page 102 with a wire to what stays behind shows the preview of its pairs first,
/// nothing moved; Apply moves it and makes the pairs, one point of each on each page; one Undo takes them back.
#[test]
fn the_selected_symbols_move_to_another_page_after_the_preview_of_their_pairs() {
    let tmp = Tmp::new("selection-move-page");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    let (on_102, on_103) = (points_on(&mut s, "102"), points_on(&mut s, "103"));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let devices: Vec<_> = s
        .canvas()
        .expect("page 102")
        .page()
        .tables
        .reps
        .iter()
        .filter_map(|r| r.device)
        .collect();
    done(&mut s, "nav.show", json!({"record": devices[0].to_string()}));
    done(&mut s, "edit.move_to_page", json!({}));
    let form = s.palette().and_then(|p| p.form.clone()).expect("the question");
    assert_eq!(form.action, "edit.move_to_page");
    let page = &form.fields[0];
    let words: Vec<String> = page.choices().iter().map(|c| page.choice_words(c)).collect();
    assert_eq!(page.label(), "Folio");
    assert!(words.iter().any(|e| e.starts_with("To page 103")), "{words:?}");
    assert!(!words.iter().any(|e| e.starts_with("To page 102")), "{words:?}");
    assert_eq!(words.last().map(String::as_str), Some("To a new page"));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.palette().is_none());
    let mut listed = 0;
    for d in devices {
        done(&mut s, "nav.show", json!({"record": d.to_string()}));
        // The question answered: page 103, then Enter.
        done(&mut s, "edit.move_to_page", json!({}));
        s.set_palette_field(0, "103");
        assert_eq!(s.run_palette_selection(), Outcome::Done);
        assert!(s.palette().is_none(), "the question closes");
        listed = pairs(&s).len();
        if listed > 0 {
            break;
        }
        // A move without a pair is made at once: undone, the next symbol is tried.
        done(&mut s, "edit.undo", json!({}));
        done(&mut s, "page.goto", json!({"folio": "102"}));
    }
    assert!(
        listed > 0,
        "a symbol of page 102 with a wire to what stays behind"
    );
    assert_eq!(points_on(&mut s, "103"), on_103, "nothing moves before Apply");
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(
        points_on(&mut s, "102"),
        on_102 + listed,
        "a source point of each pair"
    );
    assert_eq!(
        points_on(&mut s, "103"),
        on_103 + listed,
        "a target point of each pair"
    );
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(
        (points_on(&mut s, "102"), points_on(&mut s, "103")),
        (on_102, on_103),
        "one undo step"
    );
    s.shutdown();
}

/// Move to page… is where a person looks for it: on the Page tab beside Copy pages…, and in the right-click menu of
/// selected symbols (a persona run found it only in the command search).
#[test]
fn move_to_page_is_on_the_page_tab_and_in_the_right_click_menu_of_symbols() {
    use ew_viewmodel::command_bar::{Group, MenuSpot, Tab, canvas_menu, command_bar_of};
    let tmp = Tmp::new("move-page-entry");
    let mut s = tmp.sample_in(Mode::Gui, true, "102");
    let rep = s
        .canvas()
        .expect("page 102")
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device.is_some())
        .expect("a symbol")
        .id;
    done(&mut s, "select.set", json!({"ids": [rep.to_string()]}));
    let menu = canvas_menu(&s, MenuSpot::Object, None);
    let entry = menu
        .iter()
        .find(|i| i.id == "edit.move_to_page")
        .expect("in the right-click menu");
    assert_eq!((entry.label.as_str(), entry.enabled), ("Move to page…", true));
    let order = command_bar_of(&s, Tab::Page)
        .groups
        .into_iter()
        .find(|g| g.group == Group::Order)
        .expect("the Order group");
    assert!(
        order
            .buttons
            .iter()
            .any(|b| b.id == "edit.move_to_page" && b.enabled),
        "on the Page tab"
    );
    s.shutdown();
}

/// Move to page… is greyed until symbols are selected: with nothing selected and with only a wire selected, it
/// says to select the symbols to move, in the menu and on a call (it said "Nothing is selected" with a wire selected).
#[test]
fn move_to_page_asks_for_the_symbols_to_move() {
    let tmp = Tmp::new("move-page-symbols");
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    let gate = |s: &ShellModel| {
        ew_viewmodel::actions::find("edit.move_to_page")
            .expect("the action")
            .enabled_now(s)
            .err()
            .map(|t| t.key())
    };
    done(&mut s, "select.set", json!({"ids": []}));
    assert_eq!(gate(&s), Some("reason-move-select-symbols"), "nothing selected");
    let wire = s.canvas().expect("page 102").page().tables.connections[0]
        .id
        .to_string();
    done(&mut s, "select.set", json!({"ids": [wire]}));
    assert_eq!(gate(&s), Some("reason-move-select-symbols"), "a wire alone");
    assert_eq!(
        refused(&mut s, "edit.move_to_page", json!({"folio": "103"})),
        "reason-move-select-symbols"
    );
    s.shutdown();
}

/// A selected text moved with a symbol stays on its page: the preview says so first, one line counted, even where the
/// move keeps no wire as a pair; Apply moves the symbol and leaves the text, and one Undo takes the move back.
#[test]
fn a_text_selected_with_a_symbol_stays_and_the_preview_says_so() {
    let tmp = Tmp::new("move-page-text-stays");
    let mut s = tmp.sample_in(Mode::Headless, true, "102");
    let page = s.canvas().expect("page 102").page().tables.page.to_string();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "item.place", "params": {
            "page": page, "kind": {"free_text": {"text": "stays here", "at": [100_000_000, 100_000_000]}}
        }}),
    );
    let text_on_102 = |s: &mut ShellModel| {
        done(s, "page.goto", json!({"folio": "102"}));
        let tables = &s.canvas().expect("page 102").page().tables;
        tables
            .items
            .iter()
            .find(|i| i.text.as_deref() == Some("stays here"))
            .map(|i| i.id)
    };
    let text = text_on_102(&mut s).expect("the text").to_string();
    let reps: Vec<String> = s
        .canvas()
        .expect("page 102")
        .page()
        .tables
        .reps
        .iter()
        .filter(|r| r.device.is_some())
        .map(|r| r.id.to_string())
        .collect();
    let line = "1 selected text or box stays on page 102";
    let mut moved = None;
    for rep in reps {
        done(&mut s, "select.set", json!({"ids": [rep, text]}));
        let out = s.dispatch("edit.move_to_page", &json!({"folio": "103"}));
        if out == Outcome::Done && s.properties().card().is_some() {
            moved = Some(rep);
            break;
        }
        // A symbol whose wires the move cannot keep: the next.
        let _ = s.dispatch("preview.cancel", &json!({}));
    }
    let rep = moved.expect("a symbol of page 102 that moves");
    assert!(
        s.preview_pane().notice_words().iter().any(|n| n == line),
        "{:?}",
        s.preview_pane().notice_words()
    );
    let steps = s.revision();
    done(&mut s, "preview.apply", json!({}));
    assert_ne!(s.revision(), steps, "Apply moves");
    assert!(text_on_102(&mut s).is_some(), "the text stays");
    let on_102 = |s: &ShellModel| {
        s.canvas()
            .expect("page 102")
            .page()
            .tables
            .reps
            .iter()
            .any(|r| r.id.to_string() == rep)
    };
    assert!(!on_102(&s), "the symbol moved");
    done(&mut s, "edit.undo", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert!(on_102(&s), "one undo step");
    s.shutdown();
}
