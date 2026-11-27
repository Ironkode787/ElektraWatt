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

/// A contactor of his own made in the test, `number`, with a coil A1, A2 and a normally open contact 13, 14, saved.
fn own_contactor(s: &mut ShellModel, number: &str) {
    done(s, "parts.new", json!({"start": "device"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", number)] {
        done(s, "parts.field", json!({"field": field, "value": value}));
    }
    for (i, what, pins) in [(0, "ew.fn.coil", "A1, A2"), (1, "ew.fn.contact-no", "13, 14")] {
        done(s, "parts.row", json!({"table": "pieces"}));
        done(
            s,
            "parts.field",
            json!({"field": format!("pieces[{i}].what"), "value": what}),
        );
        done(
            s,
            "parts.field",
            json!({"field": format!("pieces[{i}].pins"), "value": pins}),
        );
    }
    done(s, "parts.save", json!({}));
}

/// A new symbol from a page opens without a name, To do asking for one first (no two symbols called "New symbol"),
/// and Save says so; What it is taken from the symbol drawn last says so; the sample tag is what a page would show
/// for a device of it, unnumbered; the tool is the one used last; a drawing whose connection grid is no longer the
/// page's says so on its strip, naming both grids.
#[test]
fn a_new_symbol_from_a_page_asks_a_name_keeps_the_last_and_warns_of_another_grid() {
    let tmp = Tmp::new("journey-4-sheet");
    let mut s = project(&tmp);
    done(&mut s, "parts.new_symbol", json!({}));
    let editor = s.symbol_editor().expect("the drawing");
    assert!(editor.unnamed, "no name of his yet");
    let name = editor.symbol_rows().into_iter().find(|r| r.key == "name").expect("Name");
    assert_eq!(
        name.value,
        ew_viewmodel::shell::symbol_editor::RowValue::Text(Some(String::new())),
        "Name shows empty"
    );
    let todo = editor.outline().todo;
    assert_eq!(todo[0].text, "Give it a name");
    assert!(!editor.kept_what, "nothing drawn before");
    assert_eq!(s.drawing_sample_tag(), "-K?", "a contact's device, unnumbered");
    assert_eq!(
        crate::common::failed(&mut s, "parts.save", json!({})),
        "Give it a name"
    );
    done(
        &mut s,
        "parts.field",
        json!({"field": "symbol.name", "value": "Level switch"}),
    );
    done(&mut s, "symbol.tool", json!({"tool": "line"}));
    draw_contact(&mut s);
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    // The next: What it is and the tool as left.
    done(&mut s, "parts.new_symbol", json!({}));
    let editor = s.symbol_editor().unwrap();
    assert!(editor.kept_what, "What it is as for the symbol drawn last");
    assert_eq!(editor.tool, "line");
    // Its connection grid made another (an import on the QET grid): the strip names both.
    let mut doc = serde_json::to_value(&editor.draft).unwrap();
    doc["symbol"]["grid"] = json!({"num": 7_937_500, "den": 3});
    let file = tmp.0.join("qet.symbol.json");
    std::fs::write(&file, serde_json::to_vec(&doc).unwrap()).unwrap();
    done(&mut s, "symbol.open", json!({"file": file, "discard": true}));
    let strip = s.symbol_editor().unwrap().strip().unwrap_or_default();
    assert!(
        strip.contains("This symbol is on QET 10 px; page 101 is on 4 mm"),
        "{strip}"
    );
}

/// A piece's symbol is named after the piece and the part, its key after the name, and its strip names both; its
/// sample tag is the part's first device's. Use it for every part with this piece… names his other contactor with a
/// coil of two pins before it applies, and Apply gives that part's coil the symbol.
#[test]
fn a_piece_s_symbol_is_named_by_its_part_and_is_used_for_every_part_with_the_piece() {
    let tmp = Tmp::new("journey-piece-for-all");
    let mut s = project(&tmp);
    own_contactor(&mut s, "MY-CON-8");
    own_contactor(&mut s, "MY-CON-9");
    done(&mut s, "parts.draw", json!({"piece": 0, "from": "new"}));
    let editor = s.symbol_editor().expect("the drawing");
    let name = ew_viewmodel::shell::symbol_editor::display_name(&editor.draft);
    assert!(name.ends_with(" A1, A2 of MY-CON-9"), "{name}");
    assert!(editor.draft.key.ends_with("-a1-a2-of-my-con-9"), "{}", editor.draft.key);
    let strip = editor.strip().unwrap_or_default();
    assert!(
        strip.starts_with("You are changing the symbol of the piece ") && strip.contains(" A1, A2 of MY-CON-9."),
        "{strip}"
    );
    assert_eq!(s.drawing_sample_tag(), "-K1");
    done(
        &mut s,
        "symbol.draw",
        json!({"tool": "rectangle", "x": -4, "y": 4, "x2": 12, "y2": 12}),
    );
    done(&mut s, "parts.use_for_all", json!({}));
    let card: Vec<String> = s
        .symbol_editor()
        .and_then(|e| e.use_for_all.clone())
        .expect("the card")
        .into_iter()
        .map(|p| p.number)
        .collect();
    assert_eq!(card, ["MY-CON-8"], "his other part with the piece, not this one");
    done(&mut s, "parts.use_for_all", json!({"apply": true}));
    assert!(s.symbol_editor().unwrap().use_for_all.is_none(), "the card is spent");
    let address = s.settings().symbols_recent.first().cloned().expect("the saved symbol");
    done(&mut s, "parts.draw", json!({"done": true}));
    done(&mut s, "parts.open", json!({"part": "MY-CON-8", "discard": true}));
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the other part");
    assert_eq!(open.sheet.pieces[0].symbol, address, "its coil names the symbol");
    assert!(open.sheet.pieces[1].symbol.is_empty(), "its contact keeps its own");
}

/// A symbol a library holds lists the parts a page draws with it (its sheet's Parts drawn with it): the starter coil
/// lists his contactor, whose coil names no symbol of its own.
#[test]
fn a_symbol_lists_the_parts_drawn_with_it() {
    let tmp = Tmp::new("journey-drawn-with");
    let mut s = project(&tmp);
    own_contactor(&mut s, "MY-CON-7");
    let coil = ew_viewmodel::ew_api::output::starter_piece_symbol("ew.fn.coil").expect("the starter coil");
    done(&mut s, "parts.open", json!({"symbol": coil}));
    let parts: Vec<String> = s.drawing_parts().into_iter().map(|p| p.number).collect();
    assert!(parts.iter().any(|n| n == "MY-CON-7"), "{parts:?}");
}

/// A box arranged says on the status line whose box it is and how many pins it has, and a bus device's line says
/// where its network ports are drawn.
#[test]
fn a_box_speaks_of_its_part_and_its_ports() {
    let tmp = Tmp::new("journey-h-status");
    let mut s = project(&tmp);
    done(&mut s, "parts.new", json!({"start": "box"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", "PN-SW2")] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    for name in ["L+", "M"] {
        done(&mut s, "part.pin", json!({"add": true, "name": name}));
    }
    done(&mut s, "parts.row", json!({"table": "bus"}));
    for (field, value) in [("bus.0.interface", "X1"), ("bus.0.ports", "2")] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.draw", json!({"what": "box"}));
    let status: Vec<String> = ew_viewmodel::status::bar(&s, false).into_iter().map(|f| f.text).collect();
    assert!(
        status.iter().any(|f| f == "PN-SW2 · changed, not saved · the box: 2 pins"),
        "{status:?}"
    );
    let line = s
        .symbol_editor()
        .and_then(|e| e.device_box())
        .and_then(|b| b.ports_line());
    assert_eq!(
        line.as_deref(),
        Some("Network ports X1 P1 and X1 P2 are drawn on a bus page.")
    );
}

/// A box part of his own made in the test, `number`, with the supply pins L+ and M, saved and placed on page 101 at
/// grid points (10, 10) to (30, 20), drawn by its corners as a box nobody arranged is: the placed box's id.
fn own_box_placed(s: &mut ShellModel, number: &str) -> String {
    done(s, "parts.new", json!({"start": "box"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", number)] {
        done(s, "parts.field", json!({"field": field, "value": value}));
    }
    for name in ["L+", "M"] {
        done(s, "part.pin", json!({"add": true, "name": name}));
    }
    done(s, "parts.save", json!({}));
    done(s, "parts.back", json!({}));
    done(s, "tool.place", json!({}));
    done(s, "place.pick", json!({"part": number}));
    for k in [(10, 10), (30, 20)] {
        let p = grid_point(s, k);
        cursor(s, p);
        done(s, "canvas.activate", json!({}));
    }
    done(s, "ui.dismiss", json!({}));
    done(s, "tool.select", json!({}));
    let rep = tables(s).reps.iter().find(|r| r.black_box.is_some()).map(|r| r.id.to_string());
    rep.expect("the box on the page")
}

/// Every way into Parts' drawing that a page offers works from the page, Parts not open: a placed box's Arrange its
/// box… and the device browser's Arrange…, which open Parts on that part's box; a placed symbol's Draw one like it…;
/// Open in Parts of a symbol; and the Symbol cell's Draw a new symbol… of a part opened from the page.
#[test]
fn the_ways_into_parts_drawing_work_from_a_page() {
    use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
    let tmp = Tmp::new("drawing-from-a-page");
    let mut s = project(&tmp);
    let rep = own_box_placed(&mut s, "PN-SW3");
    crate::common::select_ids(&mut s, &[rep]);
    let menu = canvas_menu(&s, MenuSpot::Object, None);
    let arrange = menu.iter().find(|i| i.label == "Arrange its box…").expect("Arrange its box…");
    assert!(arrange.enabled, "{:?}", arrange.reason);
    done(&mut s, arrange.id, arrange.params.clone());
    assert!(s.symbol_editor().and_then(|e| e.device_box()).is_some(), "the box in Parts");
    done(&mut s, "parts.draw", json!({"done": true}));
    done(&mut s, "parts.back", json!({"discard": true}));
    // The device browser's Arrange…, Parts not open.
    assert!(s.parts_place().is_none());
    done(&mut s, "parts.draw", json!({"what": "box", "part": "PN-SW3"}));
    assert!(s.symbol_editor().and_then(|e| e.device_box()).is_some(), "the box in Parts");
    done(&mut s, "parts.draw", json!({"done": true}));
    done(&mut s, "parts.back", json!({"discard": true}));
    // Draw one like it… of a placed symbol.
    done(&mut s, "tool.place", json!({}));
    let coil = ew_viewmodel::ew_api::output::starter_piece_symbol("ew.fn.coil").expect("the starter coil");
    done(&mut s, "place.pick", json!({"symbol": coil}));
    click(&mut s, (30, 10));
    done(&mut s, "tool.select", json!({}));
    let placed = tables(&s).reps.iter().find(|r| r.symbol_key == coil).map(|r| r.id.to_string());
    crate::common::select_ids(&mut s, &[placed.expect("the coil")]);
    let menu = canvas_menu(&s, MenuSpot::Object, None);
    let like = menu.iter().find(|i| i.label == "Draw one like it…").expect("Draw one like it…");
    assert!(like.enabled, "{:?}", like.reason);
    done(&mut s, like.id, like.params.clone());
    assert!(s.symbol_editor().is_some(), "the drawing");
    done(&mut s, "parts.back", json!({"discard": true}));
    // Open in Parts of a symbol, then a part's Symbol cell.
    done(&mut s, "parts.open", json!({"symbol": coil}));
    done(&mut s, "parts.back", json!({"discard": true}));
    own_contactor(&mut s, "MY-CON-6");
    done(&mut s, "parts.back", json!({}));
    done(&mut s, "parts.open", json!({"part": "MY-CON-6"}));
    done(&mut s, "parts.draw", json!({"piece": 0, "from": "new"}));
    assert!(s.symbol_editor().is_some(), "the piece's drawing");
}

/// Save by any way on the drawing is the strip's: Ctrl+S (File ▸ Save) on a starter symbol he only views publishes
/// nothing into My parts and says why, and on a box he arranges finishes the box (Done), never the symbol in hand
/// under it.
#[test]
fn file_save_on_the_drawing_is_the_strip_s_save() {
    let tmp = Tmp::new("drawing-file-save");
    let mut s = project(&tmp);
    let coil = ew_viewmodel::ew_api::output::starter_piece_symbol("ew.fn.coil").expect("the starter coil");
    done(&mut s, "parts.open", json!({"symbol": coil}));
    let locked = s.symbol_editor().and_then(|e| e.locked.clone()).expect("a starter symbol is not his");
    assert_eq!(crate::common::failed(&mut s, "file.save", json!({})), locked);
    assert!(s.settings().symbols_recent.is_empty(), "nothing published into My parts");
    done(&mut s, "parts.back", json!({"discard": true}));
    // A symbol in hand, unnamed, under a box he arranges: Ctrl+S is the box's Done.
    done(&mut s, "parts.new_symbol", json!({}));
    done(&mut s, "parts.draw", json!({"done": true}));
    done(&mut s, "parts.new", json!({"start": "box"}));
    done(&mut s, "part.pin", json!({"add": true, "name": "L+"}));
    done(&mut s, "parts.draw", json!({"what": "box"}));
    done(&mut s, "part.pin", json!({"pin": 0, "side": "top"}));
    done(&mut s, "file.save", json!({}));
    assert!(s.symbol_editor().and_then(|e| e.device_box()).is_none(), "the box is done");
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the part");
    assert!(open.sheet.box_.is_some(), "its sheet took the box");
}

/// Leaving a page's drawing never drops a part sheet open in Parts: Parts stepped aside for Test on a page with a part
/// half typed, New symbol on that page, then Save and place: the page shows with the Place tool armed, and Parts
/// comes back with the part and its typing.
#[test]
fn save_and_place_keeps_a_part_sheet_open_in_parts() {
    let tmp = Tmp::new("drawing-place-keeps-sheet");
    let mut s = project(&tmp);
    done(&mut s, "parts.new", json!({"start": "device"}));
    done(&mut s, "parts.field", json!({"field": "part_number", "value": "HALF-TYPED"}));
    done(&mut s, "parts.new_symbol", json!({}));
    done(&mut s, "parts.field", json!({"field": "symbol.name", "value": "Tested"}));
    draw_contact(&mut s);
    done(&mut s, "symbol.scratch", json!({}));
    assert!(s.symbol_editor().is_none(), "the page shows");
    done(&mut s, "parts.new_symbol", json!({"discard": true}));
    done(&mut s, "parts.field", json!({"field": "symbol.name", "value": "Placed"}));
    draw_contact(&mut s);
    done(&mut s, "parts.save", json!({}));
    assert!(s.symbol_editor().is_none(), "the page shows to place it");
    done(&mut s, "parts.open", json!({}));
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the part is still open");
    assert!(open.changed, "with its typing");
    assert_eq!(open.title(), "HALF-TYPED");
}

/// With no project open (Parts from the start page), Done on a piece's new symbol gives the piece the symbol it saved,
/// and the Symbols shelf lists it: the libraries the place reads are those the symbol was saved into.
#[test]
fn a_piece_s_new_symbol_is_given_to_it_with_no_project_open() {
    let tmp = Tmp::new("journey-piece-no-project");
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = ShellModel::new(Mode::Headless, tmp.config().with_libraries(Some(mine)));
    done(&mut s, "parts.open", json!({}));
    own_contactor(&mut s, "MY-CON-5");
    done(&mut s, "parts.draw", json!({"piece": 0, "from": "new"}));
    done(
        &mut s,
        "symbol.draw",
        json!({"tool": "rectangle", "x": -4, "y": 4, "x2": 12, "y2": 12}),
    );
    done(&mut s, "parts.save", json!({}));
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the part");
    let symbol = open.sheet.pieces[0].symbol.clone();
    assert!(symbol.starts_with("library:"), "the coil names its new symbol: {:?}", open.sheet.pieces);
    done(&mut s, "parts.find", json!({"shelf": "symbols", "text": "MY-CON-5"}));
    let shelf: Vec<String> = s.parts_place().map(|p| p.symbols.iter().map(|f| f.address.clone()).collect()).unwrap_or_default();
    assert!(shelf.contains(&symbol), "the shelf lists it: {shelf:?}");
}
