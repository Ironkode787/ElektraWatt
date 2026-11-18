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
    // A second box of the part beside it.
    done(
        &mut s,
        "place.pick",
        json!({"part": copy.to_string(), "template": 0}),
    );
    let beside = grid_point(&s, (40, 10));
    cursor(&mut s, beside);
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
    // The pin that stands first on the top edge of each box drawn of the part.
    let firsts = |s: &ShellModel| -> Vec<Option<u16>> {
        tables(s)
            .reps
            .iter()
            .filter_map(|r| r.black_box.clone())
            .map(|b| {
                b.cps
                    .iter()
                    .filter(|c| c.2 == Dir::Up)
                    .min_by_key(|c| c.1.0)
                    .map(|c| c.0)
            })
            .collect()
    };
    let before = top(&s);
    assert_eq!(before.len(), 2);
    assert_eq!(firsts(&s), [Some(0), Some(0)], "two boxes, L first");
    // Back to the box: the two top pins change places, saved; the drawn box is offered, and redrawn on request.
    done(&mut s, "symbol.new", json!({}));
    done(&mut s, "part.pin", json!({"pin": 0, "side": "top"}));
    done(&mut s, "symbol.save", json!({}));
    assert_eq!(
        crate::common::toast(&s),
        "Saved the device box of PSU-24. Boxes of it that are drawn the old way: 2."
    );
    done(&mut s, "symbol.close", json!({}));
    // Redraw them: both boxes in one undo step (rc.3 review: one step per box before).
    let rev = s.revision();
    done(&mut s, "part.redraw_boxes", json!({"part": format!("id:{copy}")}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one step");
    assert_eq!(
        firsts(&s),
        [Some(1), Some(1)],
        "N stands first on the top edge of both now"
    );
    assert_eq!(top(&s).len(), 2);
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(firsts(&s), [Some(0), Some(0)], "one Undo takes back both");
}

/// A box laid out on the 4 mm grid is drawn on a page of the 4 mm grid and on one of the QET grid with its pins on
/// that page's grid points, in the layout's order and on its sides, every distance at least as long as the layout's:
/// the QET grid's 2.65 mm steps take a 4 mm distance up to two of them, so the box comes out no tighter and wires meet
/// its pins. Each is clicked at the page's corner, outside the frame's drawing area, so the box is moved inside it
/// first.
#[test]
fn a_box_lands_on_the_grid_of_the_page_it_is_drawn_on() {
    let tmp = Tmp::new("device-box-grids");
    let mut s = new_project(&tmp);
    done(&mut s, "part.box", json!({"new": true}));
    for (key, value) in [("part_number", "PSU-5"), ("manufacturer", "Example")] {
        done(&mut s, "symbol.prop", json!({"key": key, "value": value}));
    }
    for name in ["L", "N", "PE", "+"] {
        done(&mut s, "part.pin", json!({"add": true, "name": name}));
    }
    done(&mut s, "part.box_size", json!({"width": 16, "height": 12}));
    for (pin, side, at) in [(0, "top", 4), (1, "top", 8), (2, "top", 12), (3, "bottom", 8)] {
        done(&mut s, "part.pin", json!({"pin": pin, "side": side, "at": at}));
    }
    done(&mut s, "symbol.save", json!({}));
    let copy = device_box(&s).draft.copy;
    done(&mut s, "symbol.close", json!({}));
    for (number, grid) in [(150, (4_000_000, 1)), (160, (7_937_500, 3))] {
        done(
            &mut s,
            "command.run",
            json!({"cmd": "page.create", "params": {
                "number": number, "title": "Boxes", "kind": "multi_line", "grid": {"num": grid.0, "den": grid.1},
            }}),
        );
        done(&mut s, "page.goto", json!({"folio": number.to_string()}));
        done(&mut s, "view.zoom_actual", json!({}));
        done(
            &mut s,
            "tool.black_box",
            json!({"part": copy.to_string(), "template": 0}),
        );
        let at = grid_point(&s, (1, 1));
        cursor(&mut s, at);
        done(&mut s, "canvas.activate", json!({}));
        let t = tables(&s);
        assert_eq!(t.grid, grid);
        let b = t.reps.iter().find_map(|r| r.black_box.clone()).expect("the box");
        let g = ew_viewmodel::ew_api::symbols::units::Grid::new(grid.0 as u64, grid.1 as u64).unwrap();
        let on_grid = |v: i64| {
            g.snap_point(ew_viewmodel::ew_api::symbols::units::Nm(v))
                .map(|n| n.0)
                == Some(v)
        };
        for (idx, p, _) in &b.cps {
            assert!(
                on_grid(p.0) && on_grid(p.1),
                "page {number}: pin {idx} at {p:?} is off the grid"
            );
        }
        // The top edge: the pins in the layout's order, each at least 4 mm from the corner and from the one before.
        let left = b.outline.iter().map(|p| p.0).min().unwrap();
        let mut top: Vec<(i64, u16)> = b
            .cps
            .iter()
            .filter(|c| c.2 == Dir::Up)
            .map(|c| (c.1.0, c.0))
            .collect();
        top.sort_unstable();
        assert_eq!(
            top.iter().map(|p| p.1).collect::<Vec<_>>(),
            [0, 1, 2],
            "page {number}"
        );
        let mut last = left;
        for (x, idx) in &top {
            assert!(
                x - last >= 4_000_000,
                "page {number}: pin {idx} only {} nm on",
                x - last
            );
            last = *x;
        }
        assert!(
            b.cps.iter().any(|c| c.0 == 3 && c.2 == Dir::Down),
            "page {number}"
        );
    }
}

/// The owner's case: a starter part drawn as a box that the project does not hold (the power supply) is listed on the
/// start card's Device box for a part…; choosing it opens a copy of the person's own with all its pins where a device
/// of it is drawn, a line saying so; a pin moved and the copy saved, the project holds it as his own part.
#[test]
fn a_starter_part_that_is_not_placed_opens_as_a_copy_of_one_s_own() {
    let tmp = Tmp::new("device-box-starter-copy");
    let mut s = new_project(&tmp);
    done(&mut s, "symbol.start", json!({}));
    let (part, _, detail) = s
        .box_parts()
        .iter()
        .find(|(_, number, _)| number == "EW-GEN-PSU-1")
        .cloned()
        .expect("the starter power supply is listed");
    assert!(detail.contains("ElektraWatt starter parts"), "{detail}");
    done(&mut s, "part.box", json!({ "part": part }));
    let b = device_box(&s);
    assert!(b.draft.new, "a new part of the person's own");
    assert!(
        b.copy_line()
            .is_some_and(|l| l.starts_with("A copy of your own of EW-GEN-PSU-1")),
        "{:?}",
        b.copy_line()
    );
    let names: Vec<String> = pins(&s).into_iter().map(|p| p.0).collect();
    assert_eq!(names, ["L", "N", "PE", "+", "+", "-", "-"]);
    let plus = device_box(&s).pin_table()[3].idx;
    done(&mut s, "part.pin", json!({"pin": plus, "side": "right", "at": 4}));
    done(&mut s, "symbol.save", json!({}));
    let b = device_box(&s);
    assert!(!b.draft.new && b.draft.own, "saved as the person's own part");
    assert_eq!(b.copy_line(), None);
    assert!(pins(&s).contains(&("+".to_owned(), "right", 4)));
}

/// A starter part placed on a page: its box, opened from the start card, says that the starter library cannot be
/// changed and that Save makes a copy of the person's own in the project, which its devices take.
#[test]
fn a_placed_starter_part_s_box_says_save_makes_a_copy() {
    let tmp = Tmp::new("device-box-starter-placed");
    let mut s = new_project(&tmp);
    page_150(&mut s);
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-PSU-1", "template": 0}),
    );
    // A part without a layout: its box is drawn by two clicks.
    for corner in [(10, 10), (30, 30)] {
        let at = grid_point(&s, corner);
        cursor(&mut s, at);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "ui.dismiss", json!({}));
    assert!(
        tables(&s).reps.iter().any(|r| r.black_box.is_some()),
        "the box is drawn"
    );
    done(&mut s, "symbol.start", json!({}));
    let (part, _, _) = s
        .box_parts()
        .iter()
        .find(|(_, number, _)| number == "EW-GEN-PSU-1")
        .cloned()
        .expect("the placed power supply is listed");
    assert!(part.starts_with("id:"), "the project holds it: {part}");
    done(&mut s, "part.box", json!({ "part": part }));
    let line = device_box(&s).copy_line().expect("the copy line");
    assert!(
        line.starts_with("ElektraWatt starter parts cannot be changed: Save makes a copy of your own"),
        "{line}"
    );
    assert_eq!(pins(&s).len(), 7);
}

/// The device browser's picture of a part drawn as a box is its box with its pins as the part lays them out (it was
/// the bare frame every box shares).
#[test]
fn the_device_browser_shows_a_box_part_with_its_pins() {
    let tmp = Tmp::new("device-box-preview");
    let mut s = new_project(&tmp);
    done(&mut s, "part.box", json!({"new": true}));
    for (key, value) in [("part_number", "ECP-8"), ("manufacturer", "Example")] {
        done(&mut s, "symbol.prop", json!({"key": key, "value": value}));
    }
    for name in ["24V", "0V", "1", "2"] {
        done(&mut s, "part.pin", json!({"add": true, "name": name}));
    }
    done(&mut s, "symbol.save", json!({}));
    done(&mut s, "symbol.close", json!({}));
    done(&mut s, "tool.place_part", json!({}));
    done(&mut s, "picker.filter", json!({"text": "ECP-8"}));
    let preview = s.picker().and_then(|p| p.preview.clone()).expect("a picture");
    let names: Vec<&str> = preview.cps.iter().map(|c| c.designation.as_str()).collect();
    assert_eq!(names, ["24V", "0V", "1", "2"]);
    assert_eq!(
        preview.outline.len(),
        1 + names.len(),
        "the box and a lead per pin"
    );
}

/// Close box returns where the box was opened from: the editor's ways in, which offer the symbol draft in hand to
/// continue, not that draft as if it had been opened again.
#[test]
fn close_box_returns_to_the_ways_in_it_was_opened_from() {
    let tmp = Tmp::new("device-box-close");
    let mut s = new_project(&tmp);
    done(
        &mut s,
        "symbol.new",
        json!({"key": "example.lamp", "name": "Lamp"}),
    );
    done(
        &mut s,
        "symbol.draw",
        json!({"tool": "rectangle", "x": -2, "y": -2, "x2": 2, "y2": 2}),
    );
    done(&mut s, "symbol.start", json!({}));
    done(&mut s, "part.box", json!({"new": true}));
    done(&mut s, "symbol.close", json!({}));
    let editor = s.symbol_editor().expect("the editor stays");
    assert!(editor.device_box().is_none() && editor.start, "the ways in");
    assert!(editor.in_hand(), "the lamp waits to be continued");
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
