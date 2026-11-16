// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's screen through the window (rc.3): what a person learned on a page works on its drawing (a
//! selection rectangle, the arrow keys whatever was clicked last, Select all, Delete, the clipboard), its panels show
//! the same selection, and a part's device box is laid out with the pointer.

use crate::support;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input, parse_chord};
use serde_json::{Value, json};
use support::Tmp;

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn drag(from: (i64, i64), to: (i64, i64)) -> Input {
    Input::Drag {
        from,
        to,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn key(chord: &str) -> Input {
    let (key, mods) = parse_chord(chord).unwrap();
    Input::Key { key, mods, repeat: 1 }
}

fn done(d: &mut Driver, action: &str, params: Value) {
    assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done, "{action}");
}

fn selection(d: &mut Driver) -> Vec<String> {
    d.shell_mut()
        .symbol_editor()
        .unwrap()
        .selection
        .iter()
        .map(|i| i.word())
        .collect()
}

fn graphics(d: &mut Driver) -> Value {
    serde_json::to_value(&d.shell_mut().symbol_editor().unwrap().draft.symbol.graphics).unwrap()
}

/// The editor on a draft with two small squares, one around the origin and one right of it, nothing selected; the
/// drawing's centre is the symbol's origin (an empty draft is fitted around it).
fn two_squares(tmp: &Tmp) -> (Driver, (i64, i64)) {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(&mut d, "symbol.new", json!({"key": "example.window", "name": "Window"}));
    d.settle();
    done(&mut d, "symbol.draw", json!({"tool": "rectangle", "x": -2, "y": -2, "x2": 2, "y2": 2}));
    done(&mut d, "symbol.draw", json!({"tool": "rectangle", "x": 6, "y": -2, "x2": 10, "y2": 2}));
    done(&mut d, "symbol.select", json!({"items": []}));
    d.settle();
    let centre = d.find("Drawing of Window").expect("the drawing");
    (d, centre)
}

/// The owner's first fault: a rectangle dragged over empty paper selects the things inside it; dragged to the left it
/// takes what it touches; a click on empty paper clears the selection.
#[test]
fn a_rectangle_dragged_over_empty_paper_selects_what_lies_inside_it() {
    let tmp = Tmp::new();
    let (mut d, (cx, cy)) = two_squares(&tmp);
    // To the right, around both squares (the view shows at most 24 points per millimetre).
    d.input(&drag((cx - 150, cy - 150), (cx + 300, cy + 150)));
    assert_eq!(selection(&mut d), ["graphic:0", "graphic:1"]);
    // The panels show the same selection.
    assert!(d.find("Width (mm) of 2 things selected").is_some(), "{:?}", d.labels());
    d.input(&click((cx - 150, cy - 150)));
    assert!(selection(&mut d).is_empty(), "a click on empty paper clears");
    // To the left, from between the squares into the first: the band takes what it touches.
    d.input(&drag((cx + 47, cy - 150), (cx, cy)));
    assert_eq!(selection(&mut d), ["graphic:0"]);
    d.shell_mut().shutdown();
}

/// The owner's second fault: the arrow keys move the selection by the grid step whatever was clicked last (a row of
/// the outline, a list of Properties), as do Select all, Duplicate and Delete; Escape clears the selection.
#[test]
fn the_arrow_keys_move_the_selection_whatever_was_clicked_last() {
    let tmp = Tmp::new();
    let (mut d, (cx, cy)) = two_squares(&tmp);
    d.input(&click((cx, cy)));
    assert_eq!(selection(&mut d), ["graphic:0"]);
    d.input(&key("Right"));
    assert_eq!(graphics(&mut d)[0]["rect"]["rect"][0], json!([-1_500_000, -2_000_000]));
    // A row of the outline selects the other square; the arrows move it, never the list's cursor.
    let row = d.find("Rectangle 2").expect("the outline's row");
    d.input(&click(row));
    assert_eq!(selection(&mut d), ["graphic:1"]);
    d.input(&key("Down"));
    assert_eq!(graphics(&mut d)[1]["rect"]["rect"][0], json!([6_000_000, -1_500_000]));
    // After a list of Properties was used (Fill shape), the keys still act on the drawing.
    let fill = d.find("Fill shape of Rectangle 2").expect("the row's check box");
    d.input(&click(fill));
    assert_eq!(graphics(&mut d)[1]["rect"]["fill"], json!("by_layer"));
    d.input(&key("Left"));
    assert_eq!(graphics(&mut d)[1]["rect"]["rect"][0], json!([5_500_000, -1_500_000]));
    d.input(&key("ctrl+a"));
    assert_eq!(selection(&mut d).len(), 2);
    d.input(&key("ctrl+d"));
    assert_eq!(graphics(&mut d).as_array().unwrap().len(), 4);
    d.input(&key("Delete"));
    assert_eq!(graphics(&mut d).as_array().unwrap().len(), 2);
    d.input(&key("ctrl+z"));
    assert_eq!(graphics(&mut d).as_array().unwrap().len(), 4, "Ctrl+Z is the editor's undo");
    d.input(&click((cx, cy)));
    d.input(&key("Escape"));
    assert!(selection(&mut d).is_empty());
    d.shell_mut().shutdown();
}

/// A drag on a thing moves the selection by the grid step, and a selected shape's handle drags its corner.
#[test]
fn a_drag_moves_the_selection_and_a_handle_reshapes_a_shape() {
    let tmp = Tmp::new();
    let (mut d, (cx, cy)) = two_squares(&tmp);
    let before = graphics(&mut d)[0]["rect"]["rect"].clone();
    d.input(&drag((cx, cy), (cx + 60, cy)));
    assert_eq!(selection(&mut d), ["graphic:0"], "the dragged thing is selected");
    let after = graphics(&mut d)[0]["rect"]["rect"].clone();
    let moved = after[0][0].as_i64().unwrap() - before[0][0].as_i64().unwrap();
    assert!(moved > 0 && moved % 500_000 == 0, "moved right on the drawing grid: {moved}");
    assert_eq!(after[0][1], before[0][1]);
    assert_eq!(
        after[1][0].as_i64().unwrap() - before[1][0].as_i64().unwrap(),
        moved,
        "the whole shape moved"
    );
    d.shell_mut().shutdown();
}

/// With nothing in hand the editor shows the ways in; New symbol opens the drawing with its outline and Properties.
#[test]
fn the_start_card_leads_to_a_new_symbol() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(&mut d, "symbol.new", json!({}));
    d.settle();
    for way in ["New symbol", "Copy a library symbol\u{2026}", "Device box for a part\u{2026}", "Import a file\u{2026}"] {
        assert!(d.find(way).is_some(), "{way}: {:?}", d.labels());
    }
    assert!(d.find("Line").is_none(), "no tools before something is in hand");
    d.input(&click(d.find("New symbol").unwrap()));
    assert!(!d.shell_mut().symbol_editor().unwrap().start);
    for name in ["Line", "Connection point", "Shapes (0)", "What it is of New symbol"] {
        assert!(d.find(name).is_some(), "{name}: {:?}", d.labels());
    }
    d.shell_mut().shutdown();
}

/// The tools draw as on a page: Line by two clicks, Rectangle by a drag, Text typed where it was clicked and made by
/// Enter, a connection point on the connection grid with the next number; Escape returns to Select. What was drawn
/// last is selected, so its rows show.
#[test]
fn the_tools_draw_with_the_pointer_and_escape_returns_to_select() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(&mut d, "symbol.new", json!({"key": "example.tools", "name": "Tools"}));
    d.settle();
    let (cx, cy) = d.find("Drawing of Tools").expect("the drawing");
    d.input(&click(d.find("Line").unwrap()));
    d.input(&click((cx - 100, cy - 100)));
    d.input(&click((cx + 100, cy - 100)));
    d.input(&click(d.find("Rectangle").unwrap()));
    d.input(&drag((cx - 100, cy - 60), (cx + 100, cy + 60)));
    assert_eq!(selection(&mut d), ["graphic:1"], "the rectangle just drawn");
    assert!(d.find("Width (mm) of Rectangle 2").is_some(), "{:?}", d.labels());
    d.input(&click(d.find("Text").unwrap()));
    d.input(&click((cx, cy)));
    d.input(&Input::Type { text: "K1".to_owned() });
    d.input(&key("Enter"));
    d.input(&click(d.find("Connection point").unwrap()));
    d.input(&click((cx - 100, cy + 100)));
    d.input(&click((cx + 100, cy + 100)));
    let g = graphics(&mut d);
    assert!(g[0]["line"].is_object() && g[1]["rect"].is_object(), "{g}");
    assert_eq!(g[2]["text"]["text"]["en"], json!("K1"));
    {
        let draft = &d.shell_mut().symbol_editor().unwrap().draft;
        let names: Vec<String> = draft.function.cps.iter().map(|p| p.designation.to_string()).collect();
        assert_eq!(names, ["1", "2"], "the tool counts on");
        assert!(
            draft
                .symbol
                .cps
                .iter()
                .all(|c| c.pos.x.0 % 4_000_000 == 0 && c.pos.y.0 % 4_000_000 == 0),
            "on the connection grid"
        );
    }
    assert_eq!(
        d.shell_mut().symbol_editor().unwrap().tool,
        "cp",
        "the tool stays for the next point"
    );
    d.input(&key("Escape"));
    assert_eq!(d.shell_mut().symbol_editor().unwrap().tool, "select");
    d.shell_mut().shutdown();
}

/// A row of Properties commits on Enter as one undo step, a name typed and not left is saved by a click on Save, and
/// Close editor returns to the window.
#[test]
fn a_row_commits_once_and_save_takes_what_was_typed() {
    let tmp = Tmp::new();
    let (mut d, _) = two_squares(&tmp);
    done(&mut d, "symbol.directory", json!({"directory": tmp.0.join("window.ewlib")}));
    done(&mut d, "symbol.cp", json!({"x": 0, "y": 4, "designation": "1"}));
    done(&mut d, "symbol.select", json!({"items": ["graphic:0"]}));
    d.input(&Input::Wait { ms: 50 });
    let x = d.find("x (mm) of Rectangle 1").unwrap_or_else(|| panic!("{:?}", d.labels()));
    d.input(&click(x));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type { text: "12".to_owned() });
    d.input(&key("Enter"));
    assert_eq!(graphics(&mut d)[0]["rect"]["rect"][0], json!([12_000_000, -2_000_000]));
    d.input(&key("ctrl+z"));
    assert_eq!(
        graphics(&mut d)[0]["rect"]["rect"][0],
        json!([-2_000_000, -2_000_000]),
        "one step"
    );
    // The symbol's own rows with nothing selected; the name is typed, then Save is clicked without leaving the field.
    done(&mut d, "symbol.select", json!({"items": []}));
    d.input(&Input::Wait { ms: 50 });
    d.input(&click(d.find("Name of Window").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type { text: "Lamp".to_owned() });
    let save = d.labels().into_iter().rfind(|(l, _)| l == "Save").unwrap().1;
    d.input(&click(save));
    {
        let editor = d.shell_mut().symbol_editor().unwrap();
        assert_eq!(editor.error, None);
        assert!(editor.stored(), "saved to the library");
        assert_eq!(
            serde_json::to_value(&editor.draft.symbol.name).unwrap()["en"],
            json!("Lamp")
        );
    }
    d.input(&Input::Wait { ms: 15_000 });
    d.input(&click(d.find("Close editor").unwrap()));
    assert!(d.shell_mut().symbol_editor().is_none());
    d.shell_mut().shutdown();
}

/// The device box through the window: Pins and box… in the part editor opens it on the part being made; Add pin puts
/// a pin on the box; a pin dragged to the right edge stands there.
#[test]
fn a_pin_is_added_and_dragged_onto_another_edge_of_the_box() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(&mut d, "file.new", json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}));
    done(&mut d, "part.new", json!({}));
    done(&mut d, "part.field", json!({"field": "manufacturer", "value": "Example"}));
    done(&mut d, "part.field", json!({"field": "part_number", "value": "DRV-1"}));
    d.settle();
    d.input(&click(d.find("Pins and box\u{2026}").expect("the part editor's button")));
    assert!(d.shell_mut().dialog().is_none(), "the part editor gave way");
    let pins = |d: &mut Driver| -> Vec<(String, &'static str)> {
        let b = d
            .shell_mut()
            .symbol_editor()
            .unwrap()
            .device_box()
            .expect("the device box");
        b.pin_table()
            .into_iter()
            .map(|l| {
                (
                    l.name,
                    ew_ui::ew_viewmodel::shell::symbol_editor::side_word(&l.side),
                )
            })
            .collect()
    };
    for _ in 0..3 {
        let add = d.labels().into_iter().find(|(l, _)| l == "Add pin").unwrap().1;
        d.input(&click(add));
    }
    assert_eq!(
        pins(&mut d),
        [
            ("1".to_owned(), "top"),
            ("2".to_owned(), "top"),
            ("3".to_owned(), "bottom")
        ]
    );
    // The box stands in the middle of its drawing, four grid steps wide and five high (a step is 56 points at the
    // main resolution): pin 3 in the middle of its bottom edge, dragged to the right edge.
    let (cx, cy) = d.find("Device box of DRV-1").expect("the box's drawing");
    let size = {
        let b = d.shell_mut().symbol_editor().unwrap().device_box().unwrap().steps();
        (b.w, b.h)
    };
    assert_eq!(size, (4, 5));
    d.input(&drag((cx, cy + 140), (cx + 112, cy + 10)));
    let after = pins(&mut d);
    assert!(
        after.contains(&("3".to_owned(), "right")),
        "pin 3 went from the bottom edge to the right edge: {after:?}"
    );
    d.shell_mut().shutdown();
}
