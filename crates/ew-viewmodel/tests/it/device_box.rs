// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The device box (rc.3, doc 07 §2.5): a part's pins and where they stand on its box, edited on the symbol editor's
//! screen, saved with the part, and drawn that way when the part is placed.

use crate::common::{Tmp, actual_size, cursor, done, failed, grid_point, new_project, tables};
use ew_viewmodel::ew_api::geometry::Dir;
use ew_viewmodel::shell::symbol_editor::{DeviceBox, side_word};
use ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::json;

fn device_box(s: &ShellModel) -> &DeviceBox {
    s.symbol_editor()
        .expect("the editor")
        .device_box()
        .expect("the device box")
}

/// The pin table as (name, edge, millimetres along it).
fn pins(s: &ShellModel) -> Vec<(String, &'static str, i64)> {
    device_box(s)
        .pin_table()
        .into_iter()
        .map(|l| (l.name, side_word(&l.side), l.at * 4))
        .collect()
}

/// A page of the native 4 mm grid, in front.
fn page_150(s: &mut ShellModel) {
    done(
        s,
        "command.run",
        json!({"cmd": "page.create", "params": {
            "number": 150, "title": "Boxes", "kind": "multi_line", "grid": {"num": 4_000_000, "den": 1},
        }}),
    );
    done(s, "page.goto", json!({"folio": "150"}));
    done(s, "view.zoom_actual", json!({}));
}

/// What the owner asked for: a library part that is drawn as a box (the sample circuit protector) opened from the
/// project, given one more pin, its outputs put on the bottom edge and its contacts on the right, saved; the project
/// then holds its own copy of the part with the pin and the layout, the boxes of it that are drawn are offered for
/// redrawing with their count, and a click with the Device tool places the box as it was laid out.
#[test]
fn a_library_part_s_box_gets_a_pin_and_a_layout_and_is_placed_that_way() {
    let tmp = Tmp::new("device-box-library-part");
    let mut s = actual_size(&tmp, "102");
    page_150(&mut s);
    done(&mut s, "part.box", json!({"part": "MICO 4.10"}));
    // The box as it was always drawn: the first half of the pins on top, the rest below.
    let before = pins(&s);
    assert_eq!(before.len(), 10);
    assert_eq!(before.iter().filter(|p| p.1 == "top").count(), 5);
    assert!(
        device_box(&s).pins_from().is_none(),
        "the pins are the person's to change"
    );
    done(&mut s, "part.pin", json!({"add": true, "name": "Out5"}));
    done(&mut s, "part.box_size", json!({"width": 28, "height": 16}));
    let table = device_box(&s).pin_table();
    let idx = |name: &str, nth: usize| {
        table
            .iter()
            .filter(|l| l.name == name)
            .nth(nth)
            .unwrap_or_else(|| panic!("{name}: {table:?}"))
            .idx
    };
    for (k, (name, nth)) in [("24VDC", 0), ("24VDC", 1), ("GND", 0), ("GND", 1)]
        .into_iter()
        .enumerate()
    {
        done(
            &mut s,
            "part.pin",
            json!({"pin": idx(name, nth), "side": "top", "at": 4 * (k + 1)}),
        );
    }
    for (k, name) in ["Out1", "Out2", "Out3", "Out4", "Out5"].into_iter().enumerate() {
        done(
            &mut s,
            "part.pin",
            json!({"pin": idx(name, 0), "side": "bottom", "at": 4 * (k + 1)}),
        );
    }
    done(
        &mut s,
        "part.pin",
        json!({"pin": idx("13", 0), "side": "right", "at": 4}),
    );
    done(
        &mut s,
        "part.pin",
        json!({"pin": idx("14", 0), "side": "right", "at": 8}),
    );
    let laid_out = pins(&s);
    assert_eq!(
        laid_out
            .iter()
            .filter(|p| p.1 == "bottom")
            .map(|p| (p.0.as_str(), p.2))
            .collect::<Vec<_>>(),
        [("Out1", 4), ("Out2", 8), ("Out3", 12), ("Out4", 16), ("Out5", 20)]
    );
    assert_eq!(
        laid_out
            .iter()
            .filter(|p| p.1 == "right")
            .map(|p| p.0.as_str())
            .collect::<Vec<_>>(),
        ["13", "14"]
    );
    // One step back and forth in the box's own history.
    done(&mut s, "edit.undo", json!({}));
    assert_ne!(pins(&s), laid_out);
    done(&mut s, "edit.redo", json!({}));
    assert_eq!(pins(&s), laid_out);
    assert!(device_box(&s).dirty());
    done(&mut s, "symbol.save", json!({}));
    let b = device_box(&s);
    assert!(!b.dirty(), "saved");
    assert_eq!(pins(&s), laid_out, "the saved part reads back as it was laid out");
    let copy = b.draft.copy;
    assert!(
        !b.draft.fork,
        "the project has its own copy of the library's part now"
    );
    // Closing the box returns to the project, where it was opened from.
    done(&mut s, "symbol.close", json!({}));
    assert!(s.symbol_editor().is_none());
    // One click places the box as the part lays it out.
    done(
        &mut s,
        "tool.black_box",
        json!({"part": copy.to_string(), "template": 0}),
    );
    let at = grid_point(&s, (10, 10));
    cursor(&mut s, at);
    done(&mut s, "canvas.activate", json!({}));
    let t = tables(&s);
    let rep = t
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the box")
        .clone();
    let b = rep.black_box.as_ref().unwrap();
    let step = 4_000_000;
    assert_eq!(
        b.outline,
        [
            at,
            (at.0 + 7 * step, at.1),
            (at.0 + 7 * step, at.1 + 4 * step),
            (at.0, at.1 + 4 * step)
        ]
    );
    assert_eq!(b.cps.len(), 11);
    let bottom: Vec<i64> = b
        .cps
        .iter()
        .filter(|c| c.2 == Dir::Down)
        .map(|c| (c.1.0 - at.0) / step)
        .collect();
    assert_eq!(bottom, [1, 2, 3, 4, 5], "the five outputs along the bottom edge");
    assert!(
        b.cps
            .iter()
            .filter(|c| c.2 == Dir::Down)
            .all(|c| c.1.1 == at.1 + 4 * step)
    );
    assert_eq!(b.cps.iter().filter(|c| c.2 == Dir::Right).count(), 2);
    let labels: Vec<&str> = t
        .cps
        .iter()
        .filter(|c| c.rep == rep.id)
        .filter_map(|c| c.label.split_once(':').map(|(_, d)| d))
        .collect();
    assert!(
        labels.contains(&"Out5"),
        "the new pin is a connection point of the device: {labels:?}"
    );
}

/// A part of one's own made on the screen: named, given pins, saved to the project; a drawn box of it is redrawn when
/// its layout changes later and the person says so.
#[test]
fn a_new_part_s_box_is_saved_and_a_drawn_box_is_redrawn_on_request() {
    let tmp = Tmp::new("device-box-new-part");
    let mut s = new_project(&tmp);
    page_150(&mut s);
    done(&mut s, "part.box", json!({"new": true}));
    // Nothing is saved before the part has a number and a manufacturer; the refusal says which.
    assert_eq!(
        failed(&mut s, "symbol.save", json!({})),
        "Symbol editor: Give the part a part number."
    );
    done(
        &mut s,
        "symbol.prop",
        json!({"key": "part_number", "value": "PSU-24"}),
    );
    done(
        &mut s,
        "symbol.prop",
        json!({"key": "manufacturer", "value": "Example"}),
    );
    for name in ["L", "N", "+", "-"] {
        done(&mut s, "part.pin", json!({"add": true, "name": name}));
    }
    assert_eq!(
        pins(&s).iter().map(|p| (p.0.as_str(), p.1)).collect::<Vec<_>>(),
        [("L", "top"), ("N", "top"), ("+", "bottom"), ("-", "bottom")]
    );
    // A pin's rows in Properties: its name and the potential it carries.
    done(&mut s, "part.box_select", json!({"pins": [0]}));
    done(&mut s, "symbol.prop", json!({"key": "role", "value": "l"}));
    done(&mut s, "symbol.prop", json!({"key": "name", "value": "L1"}));
    assert_eq!(device_box(&s).pin_table()[0].name, "L1");
    done(&mut s, "symbol.save", json!({}));
    let copy = device_box(&s).draft.copy;
    assert!(!device_box(&s).draft.new);
    // Test on a page arms the Device tool with the part; a click places its box.
    done(&mut s, "symbol.scratch", json!({}));
    assert!(s.symbol_editor().is_none(), "the editor steps aside");
    assert_eq!(s.tool().name(), "black_box");
    let at = grid_point(&s, (10, 10));
    cursor(&mut s, at);
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    let top = |s: &ShellModel| -> Vec<i64> {
        let t = tables(s);
        let b = t.reps.iter().find_map(|r| r.black_box.clone()).expect("the box");
        b.cps
            .iter()
            .filter(|c| c.2 == Dir::Up)
            .map(|c| (c.1.0 - at.0) / 4_000_000)
            .collect()
    };
    let before = top(&s);
    assert_eq!(before.len(), 2);
    // Back to the box: the two top pins change places, saved; the drawn box is offered, and redrawn on request.
    done(&mut s, "symbol.new", json!({}));
    done(&mut s, "part.pin", json!({"pin": 0, "side": "top"}));
    done(&mut s, "symbol.save", json!({}));
    assert_eq!(
        crate::common::toast(&s),
        "Saved the device box of PSU-24. Boxes of it that are drawn the old way: 1."
    );
    done(&mut s, "symbol.close", json!({}));
    done(&mut s, "part.redraw_boxes", json!({"part": format!("id:{copy}")}));
    let t = tables(&s);
    let b = t.reps.iter().find_map(|r| r.black_box.clone()).expect("the box");
    let first_top = b
        .cps
        .iter()
        .filter(|c| c.2 == Dir::Up)
        .min_by_key(|c| c.1.0)
        .map(|c| c.0);
    assert_eq!(first_top, Some(1), "N stands first on the top edge now");
    assert_eq!(top(&s).len(), 2);
}

/// A PLC card's pins come from its channels: the box shows, and a change of its pins is refused with the sentence
/// that says where they come from.
#[test]
fn a_plc_card_s_pins_are_not_edited_on_the_box() {
    let tmp = Tmp::new("device-box-plc");
    let mut s = new_project(&tmp);
    done(&mut s, "part.new", json!({"plc": "module"}));
    done(
        &mut s,
        "part.field",
        json!({"field": "manufacturer", "value": "Example"}),
    );
    done(
        &mut s,
        "part.field",
        json!({"field": "part_number", "value": "DI-8"}),
    );
    done(&mut s, "part.box", json!({}));
    assert!(s.dialog().is_none(), "the part editor gave way to the box");
    let why = device_box(&s).pins_from().expect("the sentence");
    assert!(why.contains("PLC card"), "{why}");
    assert_eq!(
        s.dispatch("part.pin", &json!({"add": true})),
        Outcome::Failed { message: why }
    );
}
