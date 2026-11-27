// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The symbol editor's screen through the window: what a person learned on a page works on its drawing (a
//! selection rectangle, the arrow keys whatever was clicked last, Select all, Delete, the clipboard), its panels show
//! the same selection, and a part's device box is laid out with the pointer.

use crate::support;

use egui::{Modifiers, PointerButton};
use ew_ui::visual::{Driver, Input, parse_chord};
use ew_viewmodel::shell::Outcome;
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

/// Parts on a new drawing named `name`.
fn drawing(tmp: &Tmp, name: &str) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(&mut d, "parts.open", json!({}));
    done(&mut d, "parts.new_symbol", json!({}));
    done(
        &mut d,
        "parts.field",
        json!({"field": "symbol.name", "value": name}),
    );
    d.settle();
    d
}

/// The drawing with two small squares, a filled one around the origin (taken by its face) and one right of it that
/// is not filled (taken by its line), nothing selected, and nothing else: the texts its kind brings are deleted. The
/// drawing's centre is the symbol's origin (a new drawing is fitted around it).
fn two_squares(tmp: &Tmp) -> (Driver, (i64, i64)) {
    let mut d = drawing(tmp, "Window");
    done(&mut d, "symbol.select", json!({"items": "all"}));
    done(&mut d, "symbol.delete", json!({}));
    done(
        &mut d,
        "symbol.draw",
        json!({"tool": "rectangle", "x": -2, "y": -2, "x2": 2, "y2": 2}),
    );
    done(&mut d, "symbol.select", json!({"items": ["graphic:0"]}));
    done(&mut d, "symbol.prop", json!({"key": "fill", "value": true}));
    done(
        &mut d,
        "symbol.draw",
        json!({"tool": "rectangle", "x": 6, "y": -2, "x2": 10, "y2": 2}),
    );
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
    assert!(
        d.find("Width (mm) of 2 things selected").is_some(),
        "{:?}",
        d.labels()
    );
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
    assert_eq!(
        graphics(&mut d)[0]["rect"]["rect"][0],
        json!([-1_500_000, -2_000_000])
    );
    // A row of the outline selects the other square; the arrows move it, never the list's cursor.
    let row = d.find("Rectangle 2").expect("the outline's row");
    d.input(&click(row));
    assert_eq!(selection(&mut d), ["graphic:1"]);
    d.input(&key("Down"));
    assert_eq!(
        graphics(&mut d)[1]["rect"]["rect"][0],
        json!([6_000_000, -1_500_000])
    );
    // After a list of Properties was used (Fill shape), the keys still act on the drawing.
    let fill = d.find("Fill shape of Rectangle 2").expect("the row's check box");
    d.input(&click(fill));
    assert_eq!(graphics(&mut d)[1]["rect"]["fill"], json!("by_layer"));
    d.input(&key("Left"));
    assert_eq!(
        graphics(&mut d)[1]["rect"]["rect"][0],
        json!([5_500_000, -1_500_000])
    );
    d.input(&key("ctrl+a"));
    assert_eq!(selection(&mut d).len(), 2);
    d.input(&key("ctrl+d"));
    assert_eq!(graphics(&mut d).as_array().unwrap().len(), 4);
    d.input(&key("Delete"));
    assert_eq!(graphics(&mut d).as_array().unwrap().len(), 2);
    d.input(&key("ctrl+z"));
    assert_eq!(
        graphics(&mut d).as_array().unwrap().len(),
        4,
        "Ctrl+Z is the editor's undo"
    );
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
    assert!(
        moved > 0 && moved % 500_000 == 0,
        "moved right on the drawing grid: {moved}"
    );
    assert_eq!(after[0][1], before[0][1]);
    assert_eq!(
        after[1][0].as_i64().unwrap() - before[1][0].as_i64().unwrap(),
        moved,
        "the whole shape moved"
    );
    d.shell_mut().shutdown();
}

/// A shape that is not filled is taken by its line, as on a page: a drag that starts inside it is a selection rectangle
/// and leaves it where it is, and a rectangle dragged to the left takes it where it touches its line.
#[test]
fn a_drag_inside_a_shape_that_is_not_filled_is_a_selection_rectangle() {
    let tmp = Tmp::new();
    let (mut d, (cx, cy)) = two_squares(&tmp);
    done(&mut d, "symbol.select", json!({"items": ["graphic:0"]}));
    done(&mut d, "symbol.prop", json!({"key": "fill", "value": false}));
    done(&mut d, "symbol.select", json!({"items": []}));
    let before = graphics(&mut d);
    d.input(&click((cx, cy)));
    assert!(selection(&mut d).is_empty(), "a click inside takes nothing");
    // From inside the square down and to the left, over its bottom line.
    d.input(&drag((cx, cy), (cx - 10, cy + 150)));
    assert_eq!(graphics(&mut d), before, "the square stays where it is");
    assert_eq!(selection(&mut d), ["graphic:0"], "the band touched its line");
    d.shell_mut().shutdown();
}

/// Ctrl+C and Ctrl+V through the window copy and paste what is selected, after a row of the outline was clicked
/// too: a shape, a text and a connection point; in a device box, a pin.
#[test]
fn ctrl_c_and_ctrl_v_copy_and_paste_shapes_texts_points_and_pins() {
    let tmp = Tmp::new();
    let (mut d, _) = two_squares(&tmp);
    let count = |d: &mut Driver| {
        let e = d.shell_mut().symbol_editor().unwrap();
        (e.draft.symbol.graphics.len(), e.draft.symbol.cps.len())
    };
    d.input(&Input::Wait { ms: 100 });
    d.input(&click(d.find("Rectangle 2").expect("the outline's row")));
    d.input(&key("ctrl+c"));
    d.input(&key("ctrl+v"));
    assert_eq!(count(&mut d), (3, 0));
    assert_eq!(selection(&mut d), ["graphic:2"], "the pasted square is selected");
    done(
        &mut d,
        "symbol.cp",
        json!({"x": 0, "y": -4, "designation": "1", "direction": "up"}),
    );
    done(
        &mut d,
        "symbol.draw",
        json!({"tool": "text", "x": 0, "y": 4, "text": "P"}),
    );
    done(&mut d, "symbol.select", json!({"items": ["cp:0", "graphic:3"]}));
    d.input(&key("ctrl+c"));
    d.input(&key("ctrl+v"));
    assert_eq!(count(&mut d), (5, 2), "the text and the point pasted");
    d.shell_mut().shutdown();

    let tmp = Tmp::new();
    let mut d = box_with_three_pins(&tmp);
    done(&mut d, "part.box_select", json!({"pins": [0]}));
    let first = box_now(&mut d).1[0].clone();
    d.input(&key("ctrl+c"));
    d.input(&key("ctrl+v"));
    let pins = box_now(&mut d).1;
    assert_eq!(pins.len(), 4);
    assert!(
        pins.iter().filter(|p| p.0 == first.0 && p.1 == first.1).count() == 2,
        "a pin of the same name on the same edge: {pins:?}"
    );
    d.shell_mut().shutdown();
}

/// New symbol in Parts opens the drawing at once, no question first, with its tools, its short sheet and its outline.
#[test]
fn new_symbol_in_parts_opens_the_drawing_at_once() {
    let tmp = Tmp::new();
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(&mut d, "parts.open", json!({}));
    d.settle();
    assert!(d.find("Line").is_none(), "no tools before a drawing is open");
    d.input(&click(d.find("New symbol").expect("New symbol in Parts")));
    d.settle();
    assert!(d.shell_mut().symbol_editor().is_some());
    for name in ["Line", "Pin", "What it is of New symbol"] {
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
    let mut d = drawing(&tmp, "Tools");
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
    d.input(&Input::Type {
        text: "K1".to_owned(),
    });
    d.input(&key("Enter"));
    d.input(&click(d.find("Pin").unwrap()));
    d.input(&click((cx - 100, cy + 100)));
    d.input(&click((cx + 100, cy + 100)));
    let g = graphics(&mut d);
    assert!(g[0]["line"].is_object() && g[1]["rect"].is_object(), "{g}");
    assert_eq!(g[2]["text"]["text"]["en"], json!("K1"));
    {
        let draft = &d.shell_mut().symbol_editor().unwrap().draft;
        let names: Vec<String> = draft
            .function
            .cps
            .iter()
            .map(|p| p.designation.to_string())
            .collect();
        assert_eq!(names, ["13", "14"], "a first symbol draws the contact's pins");
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

/// A row of Properties commits on Enter as one undo step, a name typed on the short sheet and not left is saved by a
/// click on Save, and Back leaves the drawing.
#[test]
fn a_row_commits_once_and_save_takes_what_was_typed() {
    let tmp = Tmp::new();
    let (mut d, _) = two_squares(&tmp);
    done(&mut d, "symbol.cp", json!({"x": 0, "y": 4, "designation": "1"}));
    done(&mut d, "symbol.select", json!({"items": ["graphic:0"]}));
    d.input(&Input::Wait { ms: 50 });
    let x = d
        .find("x (mm) of Rectangle 1")
        .unwrap_or_else(|| panic!("{:?}", d.labels()));
    d.input(&click(x));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: "12".to_owned(),
    });
    d.input(&key("Enter"));
    assert_eq!(
        graphics(&mut d)[0]["rect"]["rect"][0],
        json!([12_000_000, -2_000_000])
    );
    d.input(&key("ctrl+z"));
    assert_eq!(
        graphics(&mut d)[0]["rect"]["rect"][0],
        json!([-2_000_000, -2_000_000]),
        "one step"
    );
    // The symbol's own rows on its short sheet; the name is typed, then Save is clicked without leaving the field.
    done(&mut d, "symbol.select", json!({"items": []}));
    d.input(&Input::Wait { ms: 50 });
    d.input(&click(d.find("Name of Window").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&Input::Type {
        text: "Lamp".to_owned(),
    });
    let save = d.labels().into_iter().rfind(|(l, _)| l == "Save").unwrap().1;
    d.input(&click(save));
    {
        let editor = d.shell_mut().symbol_editor().unwrap();
        assert_eq!(editor.error, None);
        assert!(editor.stored(), "saved into My parts");
        assert_eq!(
            serde_json::to_value(&editor.draft.symbol.name).unwrap()["en"],
            json!("Lamp")
        );
    }
    d.input(&Input::Wait { ms: 15_000 });
    d.input(&click(d.find("Back").unwrap()));
    assert!(d.shell_mut().symbol_editor().is_none());
    d.shell_mut().shutdown();
}

/// The box of a new part DRV-1, opened by Arrange… in Parts, with three pins added by Add pin.
fn box_with_three_pins(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    done(
        &mut d,
        "file.new",
        json!({"dir": tmp.0.join("p.ew"), "template": "builtin"}),
    );
    done(&mut d, "parts.new", json!({"start": "box"}));
    done(
        &mut d,
        "parts.field",
        json!({"field": "manufacturer", "value": "Example"}),
    );
    done(
        &mut d,
        "parts.field",
        json!({"field": "part_number", "value": "DRV-1"}),
    );
    d.settle();
    // The pins are a section of the editor (picture A): a click shows it.
    crate::own_parts::press(&mut d, "On the schematic section");
    d.input(&click(d.find("Arrange\u{2026}").expect("the sheet's button")));
    assert!(d.shell_mut().dialog().is_none(), "Parts gave way");
    for _ in 0..3 {
        let add = d.labels().into_iter().find(|(l, _)| l == "Add a pin").unwrap().1;
        d.input(&click(add));
    }
    d
}

/// A pin of the device box: its name, its edge and its steps along it.
type PinPlace = (String, &'static str, i64);

/// The device box's size in grid steps and its pins.
fn box_now(d: &mut Driver) -> ((i64, i64), Vec<PinPlace>) {
    let b = d
        .shell_mut()
        .symbol_editor()
        .unwrap()
        .device_box()
        .expect("the device box");
    let steps = b.steps();
    let pins = b
        .pin_table()
        .into_iter()
        .map(|l| {
            (
                l.name,
                ew_viewmodel::shell::symbol_editor::side_word(&l.side),
                l.at,
            )
        })
        .collect();
    ((steps.w, steps.h), pins)
}

/// The box through the window: Arrange… in Parts opens it on the part being made; Add pin puts
/// a pin on the box; a pin dragged to the right edge stands there.
#[test]
fn a_pin_is_added_and_dragged_onto_another_edge_of_the_box() {
    let tmp = Tmp::new();
    let mut d = box_with_three_pins(&tmp);
    let pins = |d: &mut Driver| -> Vec<(String, &'static str)> {
        let b = d
            .shell_mut()
            .symbol_editor()
            .unwrap()
            .device_box()
            .expect("the device box");
        b.pin_table()
            .into_iter()
            .map(|l| (l.name, ew_viewmodel::shell::symbol_editor::side_word(&l.side)))
            .collect()
    };
    assert_eq!(
        pins(&mut d),
        [
            ("1".to_owned(), "top"),
            ("2".to_owned(), "top"),
            ("3".to_owned(), "bottom")
        ]
    );
    // The box stands in the middle of its drawing, four grid steps wide and five high: pin 3 in the middle of its
    // bottom edge, two and a half steps under the centre, is dragged to the right edge, two steps right of it.
    let (cx, cy) = d.find("Device box of DRV-1").expect("the box's drawing");
    let pin = d.find("Pin 3").expect("pin 3 on the box");
    let step = (pin.1 - cy) * 2 / 5;
    assert!(
        step > 0 && (pin.0 - cx).abs() <= 2,
        "{pin:?} under {:?}",
        (cx, cy)
    );
    let size = {
        let b = d
            .shell_mut()
            .symbol_editor()
            .unwrap()
            .device_box()
            .unwrap()
            .steps();
        (b.w, b.h)
    };
    assert_eq!(size, (4, 5));
    d.input(&drag(pin, (cx + 2 * step, cy + step / 2)));
    let after = pins(&mut d);
    assert!(
        after.contains(&("3".to_owned(), "right")),
        "pin 3 went from the bottom edge to the right edge: {after:?}"
    );
    d.shell_mut().shutdown();
}

/// Every edge of the device box sizes it: the left edge dragged out two steps widens the box to the left, the pins on
/// the top and bottom edges staying where they stand (two steps further from the left corner); dragged in past them
/// it stops one step before the first; the top edge dragged up a step makes the box a step higher.
#[test]
fn every_edge_of_the_box_sizes_it_and_the_pins_stay_where_they_stand() {
    let tmp = Tmp::new();
    let mut d = box_with_three_pins(&tmp);
    let places =
        |pins: &[PinPlace]| -> Vec<(&'static str, i64)> { pins.iter().map(|p| (p.1, p.2)).collect() };
    let (size, before) = box_now(&mut d);
    assert_eq!(size, (4, 5));
    assert_eq!(places(&before), [("top", 1), ("top", 2), ("bottom", 2)]);
    // The box stands in the middle of its drawing; pin 3 in the middle of its bottom edge gives the grid step.
    let geometry = |d: &mut Driver| {
        let centre = d.find("Device box of DRV-1").expect("the box's drawing");
        let pin = d.find("Pin 3").expect("pin 3 on the box");
        (centre, (pin.1 - centre.1) * 2 / 5)
    };
    let ((cx, cy), step) = geometry(&mut d);
    assert!(step > 0);
    d.input(&drag((cx - 2 * step, cy), (cx - 4 * step, cy)));
    let (size, wider) = box_now(&mut d);
    assert_eq!(size, (6, 5), "two steps wider");
    assert_eq!(places(&wider), [("top", 3), ("top", 4), ("bottom", 4)]);
    // Dragged in to the far right, the left edge stops a step before the first top pin.
    let ((cx, cy), step) = geometry(&mut d);
    d.input(&drag((cx - 3 * step, cy), (cx + 4 * step, cy)));
    let (size, narrow) = box_now(&mut d);
    assert_eq!(size, (4, 5), "no narrower than the pins need");
    assert_eq!(places(&narrow), [("top", 1), ("top", 2), ("bottom", 2)]);
    // The top edge, half the box's height above the centre, dragged a step up a step right of the centre (pin 2
    // stands on the centre).
    let ((cx, cy), step) = geometry(&mut d);
    d.input(&drag(
        (cx + step, cy - 5 * step / 2),
        (cx + step, cy - 7 * step / 2),
    ));
    let (size, higher) = box_now(&mut d);
    assert_eq!(size, (4, 6), "a step higher");
    assert_eq!(places(&higher), places(&narrow), "the pins keep their places");
    d.shell_mut().shutdown();
}
