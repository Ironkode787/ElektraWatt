// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawing in Parts (doc 07 §5.6), the journeys of the place's design on a new project from nothing: a symbol
//! nobody has, drawn from the page and placed there (journey 4); the tenth one, drawn as he left the last, like the
//! last one, and like a symbol on the page (journey 5); a contactor of his own whose coil he draws (a piece's
//! symbol); a network switch of his own drawn as a box he arranges (journey h).

use crate::common::{Tmp, cursor, done, grid_point, tables};
use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A shell on a new project from the built-in template with a schematic page 101 of the 4 mm grid on show, the
/// person's user folder (My parts) under `tmp`.
fn project(tmp: &Tmp) -> ShellModel {
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = ShellModel::new(Mode::Headless, tmp.config().with_libraries(Some(mine)));
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}),
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {
            "number": 101, "title": "Control", "kind": "multi_line", "grid": {"num": 4_000_000, "den": 1},
        }}),
    );
    done(&mut s, "page.goto", json!({"folio": "101"}));
    s
}

/// A click on page 101 at grid point `k` with the tool armed.
fn click(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid_point(s, k);
    cursor(s, p);
    done(s, "canvas.activate", json!({}));
    done(s, "ui.dismiss", json!({}));
}

/// The symbols placed on the page shown, by their keys.
fn placed(s: &ShellModel) -> Vec<String> {
    tables(s).reps.iter().map(|r| r.symbol_key.clone()).collect()
}

/// Draws a contact with two pins on the drawing: a line and a circle, Pin clicked twice.
fn draw_contact(s: &mut ShellModel) {
    done(
        s,
        "symbol.draw",
        json!({"tool": "line", "x": 0, "y": 0, "x2": 0, "y2": 8}),
    );
    done(
        s,
        "symbol.draw",
        json!({"tool": "circle", "x": 0, "y": 4, "x2": 2, "y2": 4}),
    );
    for (y, d) in [(0, "1"), (8, "2")] {
        done(s, "symbol.cp", json!({"x": 0, "y": y, "designation": d}));
    }
}

/// Journey 4: on page 101 the symbol picker has nothing that fits; its foot's New symbol… opens Parts on the drawing
/// at once, for page 101: the page's grid, a normally open contact as the first guess, the device tag's text already
/// where the standard puts it. Named, drawn and pinned, Save and place on 101 saves it into My parts and arms the
/// Place tool with it on the page; one click places it, and Undo takes the placing back, not the symbol.
#[test]
fn journey_4_a_symbol_nobody_has_is_drawn_from_the_page_and_placed_there() {
    let tmp = Tmp::new("journey-4");
    let mut s = project(&tmp);
    let page_grid = tables(&s).grid;
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "picker.filter", json!({"text": "flow sw"}));
    // The foot's New symbol…
    done(&mut s, "parts.new_symbol", json!({}));
    assert!(s.picker().is_none(), "the picker gave way");
    let editor = s.symbol_editor().expect("Parts on the drawing");
    let strip = editor.strip().expect("the strip");
    assert!(strip.contains("101"), "{strip}");
    assert_eq!(editor.back_label(), "Back without it");
    assert_eq!(editor.save_label(), "Save and place on 101");
    let grid = serde_json::to_value(editor.draft.symbol.grid).unwrap();
    assert_eq!(
        (grid["num"].as_i64(), grid["den"].as_i64()),
        (Some(page_grid.0), Some(page_grid.1)),
        "page 101's grid"
    );
    assert_eq!(editor.what(), Some("contact-no"));
    let slots = serde_json::to_value(&editor.draft.symbol.text_slots).unwrap();
    assert!(
        slots.as_array().unwrap().iter().any(|s| s["kind"] == "tag"),
        "the device tag's text stands on it: {slots}"
    );
    assert!(!editor.dirty(), "nothing drawn yet");
    done(
        &mut s,
        "parts.field",
        json!({"field": "symbol.name", "value": "Flow switch"}),
    );
    draw_contact(&mut s);
    assert!(
        s.symbol_editor().unwrap().outline().todo.is_empty(),
        "ready to save: {:?}",
        s.symbol_editor()
            .unwrap()
            .outline()
            .todo
            .iter()
            .map(|t| t.text.clone())
            .collect::<Vec<_>>()
    );
    let key = s.symbol_editor().unwrap().draft.key.clone();
    // Save and place on 101.
    done(&mut s, "parts.save", json!({}));
    assert!(s.parts_place().is_none(), "back on the page");
    assert_eq!(s.tool().name(), "place");
    let before = placed(&s);
    click(&mut s, (10, 10));
    let after = placed(&s);
    assert_eq!(after.len(), before.len() + 1);
    assert!(after.contains(&key), "{after:?}");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(placed(&s), before, "Undo takes the placing back");
    let address = s
        .settings()
        .symbols_recent
        .first()
        .cloned()
        .expect("the saved symbol");
    assert!(address.starts_with("library:"), "{address}");
    assert_eq!(s.settings().symbol_last_kind.as_deref(), Some("contact-no"));
}

/// Journey 5: Ctrl+I on the page opens the drawing with What it is, the drawing grid and Snap as he left them; Like
/// the last one opens a copy of the symbol he saved last; Draw one like it… on a placed symbol reads Save and use for
/// its tag, and Save exchanges the symbol on the page for the new one.
#[test]
fn journey_5_the_tenth_symbol_is_drawn_as_the_last_one_was_left() {
    let tmp = Tmp::new("journey-5");
    let mut s = project(&tmp);
    // The first: a coil, on a 1 mm drawing grid without Snap.
    done(&mut s, "parts.new_symbol", json!({}));
    done(
        &mut s,
        "parts.field",
        json!({"field": "symbol.name", "value": "Valve coil"}),
    );
    done(
        &mut s,
        "parts.field",
        json!({"field": "symbol.what", "value": "coil"}),
    );
    done(&mut s, "symbol.grid", json!({"step": 1, "snap": false}));
    draw_contact(&mut s);
    done(&mut s, "parts.save", json!({}));
    click(&mut s, (10, 10));
    let first = s.settings().symbols_recent.first().cloned().unwrap();
    // Ctrl+I: What it is, the grid and Snap as he left them.
    done(&mut s, "parts.new_symbol", json!({}));
    let editor = s.symbol_editor().unwrap();
    assert_eq!(editor.what(), Some("coil"));
    assert_eq!((editor.grid, editor.snap), (1, false));
    done(&mut s, "parts.back", json!({}));
    assert!(s.parts_place().is_none(), "Back without it returns to the page");
    // Like the last one: a copy of the coil, a new symbol of his own.
    done(
        &mut s,
        "parts.new_symbol",
        json!({"like": "last", "discard": true}),
    );
    let editor = s.symbol_editor().unwrap();
    assert_eq!(
        ew_viewmodel::shell::symbol_editor::display_name(&editor.draft),
        "Valve coil"
    );
    assert!(editor.draft.identity.is_none(), "a new symbol");
    assert_eq!(editor.draft.symbol.cps.len(), 2);
    done(&mut s, "parts.back", json!({}));
    // Draw one like it… on the placed coil.
    let rep = tables(&s).reps[0].clone();
    done(&mut s, "select.set", json!({"ids": [rep.id.to_string()]}));
    done(
        &mut s,
        "parts.new_symbol",
        json!({"like": "selected", "discard": true}),
    );
    let editor = s.symbol_editor().unwrap();
    let label = editor.save_label();
    assert!(label.starts_with("Save and use for -"), "{label}");
    done(
        &mut s,
        "parts.field",
        json!({"field": "symbol.name", "value": "Valve coil, LED"}),
    );
    done(
        &mut s,
        "symbol.draw",
        json!({"tool": "line", "x": 2, "y": 2, "x2": 4, "y2": 2}),
    );
    let key = s.symbol_editor().unwrap().draft.key.clone();
    done(&mut s, "parts.save", json!({}));
    assert!(s.parts_place().is_none(), "back on the page");
    assert_eq!(placed(&s), [key], "the symbol on the page is exchanged");
    assert_ne!(s.settings().symbols_recent.first(), Some(&first));
}

/// A contactor of his own: its coil drawn with a symbol he draws (Draw a new symbol…: the coil's pins stand on the
/// drawing already), its contact with his copy of the starter's (Change it for this part…). Done gives each piece its
/// symbol; saved and placed, the page draws the coil with his symbol.
#[test]
fn a_contactor_of_his_own_is_drawn_with_the_symbols_he_draws_for_it() {
    let tmp = Tmp::new("journey-piece-symbol");
    let mut s = project(&tmp);
    done(&mut s, "parts.new", json!({"start": "device"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", "CON-OWN")] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    for (i, what, pins) in [(0, "ew.fn.coil", "A1, A2"), (1, "ew.fn.contact-no", "13, 14")] {
        done(&mut s, "parts.row", json!({"table": "pieces"}));
        done(
            &mut s,
            "parts.field",
            json!({"field": format!("pieces[{i}].what"), "value": what}),
        );
        done(
            &mut s,
            "parts.field",
            json!({"field": format!("pieces[{i}].pins"), "value": pins}),
        );
    }
    // Draw a new symbol… for the coil.
    done(&mut s, "parts.draw", json!({"piece": 0, "from": "new"}));
    let editor = s.symbol_editor().expect("the drawing");
    assert_eq!(editor.back_label(), "To the part");
    assert_eq!(editor.save_label(), "Done");
    assert_eq!(editor.draft.symbol.cps.len(), 2, "the coil's pins stand on it");
    done(
        &mut s,
        "parts.field",
        json!({"field": "symbol.name", "value": "Coil, wide"}),
    );
    done(
        &mut s,
        "symbol.draw",
        json!({"tool": "rectangle", "x": -4, "y": 4, "x2": 12, "y2": 12}),
    );
    let coil = s.symbol_editor().unwrap().draft.key.clone();
    done(&mut s, "parts.save", json!({}));
    assert!(s.symbol_editor().is_none(), "back on the part");
    // Change it for this part… for the contact: his copy of the starter's.
    done(&mut s, "parts.draw", json!({"piece": 1, "from": "change"}));
    let editor = s.symbol_editor().expect("the copy");
    assert!(editor.draft.identity.is_none() && editor.locked.is_none());
    assert!(!editor.draft.symbol.cps.is_empty());
    done(&mut s, "parts.save", json!({}));
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the part");
    assert!(open.changed);
    assert!(
        open.sheet.pieces.iter().all(|p| p.symbol.starts_with("library:")),
        "each piece names its symbol: {:?}",
        open.sheet.pieces
    );
    done(&mut s, "parts.save", json!({}));
    // Opened again, the pieces keep their symbols.
    done(&mut s, "parts.open", json!({"part": "CON-OWN"}));
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the part");
    assert!(open.sheet.pieces.iter().all(|p| !p.symbol.is_empty()));
    done(&mut s, "parts.back", json!({}));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "CON-OWN"}));
    click(&mut s, (10, 10));
    assert!(
        placed(&s).contains(&coil),
        "the coil is drawn with his symbol: {:?}",
        placed(&s)
    );
}

/// Journey h: a network switch of his own drawn as a box, its supply pins L+ and M and one PROFINET interface of two
/// sockets (drawn on the network page); Arrange… puts both supply pins on the top edge; Done and Save, and one click
/// of the Place tool draws the box so on the schematic page.
#[test]
fn journey_h_a_box_of_his_own_is_arranged_and_placed_that_way() {
    use ew_api::geometry::Dir;
    let tmp = Tmp::new("journey-h");
    let mut s = project(&tmp);
    done(&mut s, "parts.new", json!({"start": "box"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", "PN-SW2")] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    for name in ["L+", "M"] {
        done(&mut s, "part.pin", json!({"add": true, "name": name}));
    }
    done(&mut s, "parts.row", json!({"table": "bus"}));
    for (field, value) in [
        ("bus.0.interface", "X1"),
        ("bus.0.bus", "profinet"),
        ("bus.0.role", "passive"),
        ("bus.0.ports", "2"),
    ] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    // Arrange…
    done(&mut s, "parts.draw", json!({"what": "box"}));
    let table = s
        .symbol_editor()
        .and_then(|e| e.device_box())
        .expect("the box")
        .pin_table();
    let supply: Vec<u16> = table
        .iter()
        .filter(|l| l.name == "L+" || l.name == "M")
        .map(|l| l.idx)
        .collect();
    assert_eq!(supply.len(), 2, "{table:?}");
    for (k, pin) in supply.iter().enumerate() {
        done(
            &mut s,
            "part.pin",
            json!({"pin": pin, "side": "top", "at": 4 * (k + 1)}),
        );
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "PN-SW2"}));
    click(&mut s, (10, 10));
    let b = tables(&s)
        .reps
        .iter()
        .find_map(|r| r.black_box.clone())
        .expect("the box");
    let on = |d: Dir| {
        let mut v: Vec<u16> = b.cps.iter().filter(|c| c.2 == d).map(|c| c.0).collect();
        v.sort_unstable();
        v
    };
    let sorted = |v: &[u16]| {
        let mut v = v.to_vec();
        v.sort_unstable();
        v
    };
    assert_eq!(on(Dir::Up), sorted(&supply), "the supply on top");
    assert!(on(Dir::Down).is_empty(), "nothing below");
}
