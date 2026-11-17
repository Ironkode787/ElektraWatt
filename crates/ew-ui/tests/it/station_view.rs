// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A station's view in its tab (doc 10 §6), at 1920 × 1080, as a person uses it: the rack is changed with the pointer,
//! the PLC tab and the keys while the window keeps working around it (replace, remove, move, undo); a valve terminal's
//! positions are places like slots; a module leads to the page it is drawn on; what is wrong with a station is said at
//! the place, in a sentence, with its fix. Building a rack and dragging its modules are in `plc_workspace.rs`, a
//! station not made yet in `plc_rack_first.rs`.

use crate::plc_workspace::{at, new_project_with_cpu, station_of, take};
use crate::support::{self, Tmp, dispatch, settle};

use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::ew_viewmodel::ew_api::output::{RackRow, Uuid};
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::json;

type Window = egui_kittest::Harness<'static, ew_ui::app::App>;

/// Two clicks at `pos` without time between them: a double click.
fn double_click(h: &mut Window, pos: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(pos));
    for _ in 0..2 {
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    h.run();
    settle(h);
}

/// A click at the widget named `label`.
fn click(h: &mut Window, label: &str) {
    let pos = at(h, label);
    h.input_mut().events.push(egui::Event::PointerMoved(pos));
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(h);
}

/// A new project whose CPU's view is opened by a double click on the CPU in the PLC panel, with the parts `modules`
/// (part numbers) taken from the catalogue into its slots: the window and the station.
fn rack_with(tmp: &Tmp, modules: &[&str]) -> (Window, Uuid) {
    let mut h = new_project_with_cpu(tmp);
    let station = h
        .state()
        .shell()
        .shown_rows(NavKind::Plc)
        .into_iter()
        .find(|r| r.role == NavRole::Station.word())
        .expect("the CPU's row");
    let row = at(&h, &station.name);
    double_click(&mut h, row);
    let id = station_of(&h);
    for number in modules {
        take(&mut h, number);
    }
    (h, id)
}

/// The slots of station `id` that hold a module.
fn modules(h: &Window, id: Uuid) -> Vec<RackRow> {
    let view = h.state().shell().rack_view(id).expect("the rack");
    view.rows.into_iter().filter(|r| r.module.is_some()).collect()
}

/// R2, change it: a click on a card selects the module for the whole window (Properties shows it, the PLC tab's
/// Module commands act on it); Replace gives it another part and the addresses of the modules behind it follow, which
/// the view says; the arrow keys go from slot to slot; Delete empties the slot; Move right changes places with the
/// module beside it; a right click gives the same commands. Each step is one Undo.
#[test]
fn a_module_is_replaced_removed_and_moved_from_the_bar_and_the_keys() {
    let tmp = Tmp::new();
    let (mut h, id) = rack_with(&tmp, &["EW-GEN-DI-1", "EW-GEN-DI-1"]);
    let before = modules(&h, id);
    assert_eq!(before[1].addresses, "I1.0–I1.7");
    click(&mut h, &format!("Slot 1, {}", before[0].tag));
    assert_eq!(h.state().shell().station_device(), before[0].module);
    let sheet = h
        .state()
        .shell()
        .properties()
        .sheet()
        .cloned()
        .expect("Properties");
    assert!(
        sheet
            .sections
            .iter()
            .any(|s| s.kind == "device" && s.targets == vec![before[0].module.unwrap_or_default()]),
        "Properties shows the module"
    );
    // Replace: the parts that fit the slot; the output card takes the input card's place.
    support::menu_node(&h, "Replace").click();
    settle(&mut h);
    support::menu_node(&h, "EW-GEN-DO-1 · PLC digital output card, 8 channels · 8 DO").click();
    settle(&mut h);
    let replaced = modules(&h, id);
    assert_eq!(replaced[0].module, before[0].module, "the same module");
    assert_eq!(
        replaced[0].part,
        "EW-GEN-DO-1 · PLC digital output card, 8 channels"
    );
    assert_eq!(
        (replaced[0].addresses.as_str(), replaced[1].addresses.as_str()),
        ("Q0.0–Q0.7", "I0.0–I0.7"),
        "the addresses follow"
    );
    h.get_by_label("The modules' addresses moved with their slots.");
    // The arrow goes to the next slot, Delete empties it.
    h.key_press(egui::Key::ArrowRight);
    settle(&mut h);
    assert_eq!(h.state().shell().station_device(), before[1].module);
    h.key_press(egui::Key::Delete);
    settle(&mut h);
    assert_eq!(modules(&h, id).len(), 1, "slot 2 is empty");
    dispatch(&mut h, "edit.undo", json!({}));
    settle(&mut h);
    assert_eq!(modules(&h, id).len(), 2, "one Undo brings it back");
    // Move right: the module in slot 1 changes places with the one in slot 2.
    click(&mut h, &format!("Slot 1, {}", replaced[0].tag));
    support::menu_node(&h, "Move right").click();
    settle(&mut h);
    let moved = modules(&h, id);
    assert_eq!(
        (moved[0].module, moved[1].module),
        (before[1].module, before[0].module),
        "the two changed places"
    );
    assert_eq!(
        h.state().shell().station_device(),
        before[0].module,
        "the cursor went with it"
    );
    // A right click on a card gives the Module commands.
    let card = at(&h, &format!("Slot 1, {}", moved[0].tag));
    h.input_mut().events.push(egui::Event::PointerMoved(card));
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: card,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(&mut h);
    support::menu_node(&h, "Remove module").click();
    settle(&mut h);
    assert_eq!(modules(&h, id).len(), 1, "removed from its menu");
    h.state_mut().shell_mut().shutdown();
}

/// R3, a valve terminal: its view is the same screen, its positions places like slots. A valve of the catalogue goes
/// onto the selected position and the position shows its part, as a card and as a row; a module of the catalogue is
/// greyed there with the reason; Blanking plate closes a position; Delete empties one.
#[test]
fn a_valve_terminals_positions_are_filled_like_slots() {
    let tmp = Tmp::new();
    let mut h = new_project_with_cpu(&tmp);
    dispatch(&mut h, "plc.new_station", json!({"kind": "valve_terminal"}));
    settle(&mut h);
    take(&mut h, "EW-GEN-VTM-1");
    h.get_by_label("Create").click();
    settle(&mut h);
    let id = station_of(&h);
    let cells = |h: &Window| h.state().shell().rack_view(id).expect("the terminal").cells;
    assert_eq!(cells(&h).len(), 4, "four positions");
    assert!(cells(&h).iter().all(|c| c.at.holder.is_none()), "all empty");
    click(&mut h, "Empty position 1");
    let shown = h.state().shell().station_shown().expect("the view");
    let card = shown
        .catalogue
        .iter()
        .find(|p| p.number == "EW-GEN-DI-1")
        .expect("the input card in the catalogue");
    assert_eq!(
        card.unfit.as_deref(),
        Some("A module goes into a slot, not onto a valve position")
    );
    take(&mut h, "EW-GEN-VLV-3");
    let first = cells(&h)[0].at.clone();
    let part = "EW-GEN-VLV-3 · Solenoid valve 5/2 for a valve terminal, spring return";
    assert_eq!(first.part, part, "the valve's part");
    let tag = first.valve.clone().expect("a valve on position 1");
    assert_eq!(
        h.query_all_by_label(&format!("Position 1, {tag}")).count(),
        2,
        "the valve stands on its position as a card and as a row"
    );
    let in_table = h
        .query_all_by_label(part)
        .filter(|n| n.accesskit_node().role() == egui::accesskit::Role::Label)
        .count();
    assert_eq!(in_table, 1, "its row says the part in words");
    assert_eq!(
        h.state().shell().station_device(),
        first.holder,
        "the valve is selected"
    );
    click(&mut h, "Empty position 2");
    h.get_by_label("Blanking plate").click();
    settle(&mut h);
    assert!(cells(&h)[1].at.blank, "a blanking plate on position 2");
    click(&mut h, &format!("Position 1, {tag}"));
    h.key_press(egui::Key::Delete);
    settle(&mut h);
    assert!(cells(&h)[0].at.holder.is_none(), "position 1 is empty again");
    h.state_mut().shell_mut().shutdown();
}

/// R4, where is this module on my drawings: a module drawn nowhere says so and offers Draw on a page…, which draws it
/// on the page chosen and shows that page; the station's tab stays beside the page. Back in the view the module names
/// its page as a link, which goes there, and Go to page on the PLC tab does the same.
#[test]
fn a_module_leads_to_the_page_it_is_drawn_on() {
    let tmp = Tmp::new();
    let (mut h, id) = rack_with(&tmp, &["EW-GEN-DI-1"]);
    let module = modules(&h, id)[0].clone();
    assert!(!module.drawn);
    let to_go = support::menu_node(&h, "Go to page");
    assert!(to_go.accesskit_node().is_disabled(), "nowhere to go yet");
    click(&mut h, &format!("Draw on a page… {}", module.tag));
    support::menu_node(&h, "101 Control").click();
    settle(&mut h);
    let on_page = |h: &Window| {
        let shell = h.state().shell();
        shell.station_tab().is_none() && shell.current_folio() == Some("101")
    };
    assert!(on_page(&h), "page 101 is on show");
    let boxes = h.state().shell().canvas().map_or(0, |c| {
        c.page()
            .tables
            .reps
            .iter()
            .filter(|r| r.device == module.module)
            .count()
    });
    assert_eq!(boxes, 1, "the module's box is drawn there");
    // The station's tab stayed beside the page's: back in it, the module names its page.
    let tab = h
        .query_all_by_label_contains("· rack")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Tab)
        .expect("the station's tab")
        .rect()
        .center();
    h.input_mut().events.push(egui::Event::PointerMoved(tab));
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: tab,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(&mut h);
    assert_eq!(station_of(&h), id);
    let drawn = modules(&h, id)[0].clone();
    assert_eq!(drawn.page.as_deref(), Some("101"));
    // The link in its row goes to the page.
    let link = h
        .query_all_by_label("Page 101")
        .rfind(|n| n.accesskit_node().role() == egui::accesskit::Role::Link)
        .expect("the module's page link")
        .rect()
        .center();
    h.input_mut().events.push(egui::Event::PointerMoved(link));
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: link,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(&mut h);
    assert!(on_page(&h), "the link goes to page 101");
    // Go to page on the PLC tab, for the module under the cursor.
    dispatch(&mut h, "plc.rack_editor", json!({"station": id.to_string()}));
    settle(&mut h);
    click(&mut h, &format!("Slot 1, {}", drawn.tag));
    support::menu_node(&h, "Go to page").click();
    settle(&mut h);
    assert!(on_page(&h), "Go to page goes to page 101");
    h.state_mut().shell_mut().shutdown();
}

/// R5, what is wrong with this station: a module dropped on a slot its part does not allow is refused at that slot,
/// with the reason in a sentence under the rack; a module whose part is gone says No part in its row, with Choose
/// part… beside it and in To do; a module drawn on no page stands in To do with Draw on a page….
#[test]
fn what_is_wrong_is_said_at_the_place_with_its_fix() {
    let tmp = Tmp::new();
    let (mut h, id) = rack_with(&tmp, &[]);
    // A module part of one's own that goes into slots 1 and 2 only.
    dispatch(&mut h, "part.new", json!({"plc": "module"}));
    for (field, value) in [
        ("manufacturer", "Acme"),
        ("part_number", "DI-12"),
        ("description", "Input card for slots 1 and 2"),
        ("plc.kind", "di"),
        ("plc.count", "8"),
        ("plc.slots", "1-2"),
    ] {
        dispatch(&mut h, "part.field", json!({"field": field, "value": value}));
    }
    dispatch(&mut h, "part.save", json!({}));
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "the part is saved");
    assert_eq!(station_of(&h), id, "the view is still on show");
    take(&mut h, "DI-12");
    let first = modules(&h, id)[0].clone();
    assert_eq!(first.slot, 1);
    // Typed to slot 3, as a drop there does: refused, at that slot, in a sentence.
    let refused = h.state_mut().shell_mut().dispatch(
        "plc.rack_move",
        &json!({"module": first.module.unwrap_or_default().to_string(), "slot": "3"}),
    );
    assert!(matches!(refused, Outcome::Failed { .. }), "{refused:?}");
    settle(&mut h);
    let tab = h.state().shell().station_tab().cloned().expect("the view");
    let (place, why) = tab.error.expect("the refusal stays in the view");
    assert_eq!(
        place,
        Some(ew_ui::ew_viewmodel::area::StationCursor::Slot { rack: 0, slot: 3 }),
        "at the slot it was refused at"
    );
    h.get_by_label(&why);
    assert!(
        why.starts_with(&first.tag) && !why.contains("EW-PLC-011"),
        "a sentence about the module, the rule's code left to the findings: {why}"
    );
    assert_eq!(modules(&h, id)[0].slot, 1, "the module stays where it was");
    // In the catalogue the part is greyed for slot 3 (the free slot after a second module), with the reason.
    take(&mut h, "EW-GEN-DI-1");
    dispatch(&mut h, "plc.rack_select", json!({"slot": 3}));
    settle(&mut h);
    let shown = h.state().shell().station_shown().expect("the view");
    let own = shown
        .catalogue
        .iter()
        .find(|p| p.number == "DI-12")
        .expect("DI-12");
    assert_eq!(
        own.unfit.as_deref(),
        Some("Not allowed in slot 3: it goes into slot 1, 2")
    );
    // Not drawn: said in To do, with the fix.
    assert!(
        shown
            .todo
            .iter()
            .any(|t| t.text.starts_with("Drawn on no page yet")),
        "{:?}",
        shown.todo
    );
    h.get_by_label("Draw on a page…");
    // Its part taken away: the row says so, never an empty cell, and offers the way out.
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device",
            "targets": [first.module.unwrap_or_default().to_string()], "path": "parts", "value": []}}),
    );
    settle(&mut h);
    assert_eq!(modules(&h, id)[0].part, "");
    h.get_by_label("No part");
    assert!(
        h.query_all_by_label("Choose part…").count() >= 1,
        "Choose part… in To do"
    );
    click(&mut h, &format!("Choose part… {}", first.tag));
    support::menu_node(&h, "EW-GEN-DI-1 · PLC digital input card, 8 channels · 8 DI").click();
    settle(&mut h);
    assert_eq!(
        modules(&h, id)[0].part,
        "EW-GEN-DI-1 · PLC digital input card, 8 channels",
        "the part chosen is the module's"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A new station's view is closed only after a question once something is chosen, because nothing of it is written
/// before Create: Cancel keeps it, Close without making it closes the tab and leaves the project as it was.
#[test]
fn closing_a_station_not_made_yet_asks() {
    use ew_ui::ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut h = new_project_with_cpu(&tmp);
    let revision = h.state().shell().revision();
    dispatch(&mut h, "plc.new_station", json!({}));
    settle(&mut h);
    take(&mut h, "EW-GEN-CPU-2");
    dispatch(&mut h, "tab.close", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().dialog(), Some(&Dialog::DiscardStation));
    support::menu_node(&h, "Cancel").click();
    settle(&mut h);
    assert!(
        h.state().shell().station_draft().is_some(),
        "Cancel keeps the view"
    );
    dispatch(&mut h, "tab.close", json!({}));
    settle(&mut h);
    support::menu_node(&h, "Close without making it").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.dialog().is_none(), "{:?}", shell.dialog());
    let drafts = shell
        .area()
        .expect("the canvas area")
        .tabs()
        .filter(|(_, _, t)| t.station.as_ref().is_some_and(|s| s.draft.is_some()))
        .count();
    assert_eq!(drafts, 0, "its tab is closed");
    assert_eq!(shell.revision(), revision, "nothing was written");
    h.state_mut().shell_mut().shutdown();
}
