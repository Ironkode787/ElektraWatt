// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The box of a part drawn as a box (doc 07 §2.5, §5.6): Arrange… opens it as the drawing of Parts, where its pins
//! are placed on its edges; Done gives the sheet the layout, the part's Save keeps it and draws the placed boxes of
//! the part anew, and a device of it is drawn that way.

use crate::common::{Tmp, actual_size, cursor, done, grid_point, new_drawing, new_project, own_open, tables};
use ew_api::geometry::Dir;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::symbol_editor::{DeviceBox, side_word};
use ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::json;

fn device_box(s: &ShellModel) -> &DeviceBox {
    s.symbol_editor()
        .expect("the drawing")
        .device_box()
        .expect("the box")
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

/// A new part of his own drawn as a box, `number` of maker Example with pins `names`, open in Parts and not saved.
fn new_box_part(s: &mut ShellModel, number: &str, names: &[&str]) {
    done(s, "parts.new", json!({"start": "box"}));
    for (field, value) in [("part_number", number), ("manufacturer", "Example")] {
        done(s, "parts.field", json!({"field": field, "value": value}));
    }
    for name in names {
        done(s, "part.pin", json!({"add": true, "name": name}));
    }
}

/// Leaves Parts for the page: Back from the box, the sheet and the place.
fn leave_parts(s: &mut ShellModel) {
    for _ in 0..3 {
        if s.parts_place().is_none() {
            return;
        }
        done(s, "parts.back", json!({}));
    }
    assert!(s.parts_place().is_none(), "back on the page");
}

/// Places part `number` on the page shown at grid point `k` with the Place tool.
fn place_part(s: &mut ShellModel, number: &str, k: (i64, i64)) {
    done(s, "tool.place", json!({}));
    done(s, "place.pick", json!({"part": number}));
    let at = grid_point(s, k);
    cursor(s, at);
    done(s, "canvas.activate", json!({}));
    done(s, "ui.dismiss", json!({}));
}

/// What the owner asked for: his own copy of a part drawn as a box (the starter power supply, its pins L, N and PE
/// and two + and two -), arranged: one more pin (a DC-OK contact), its inputs on the top edge, its outputs on the
/// bottom edge and the contact on the right; Done gives the sheet the layout, Save keeps it, and a click with the
/// Place tool draws the box as it was laid out.
#[test]
fn an_own_part_s_box_gets_a_pin_and_a_layout_and_is_placed_that_way() {
    let tmp = Tmp::new("device-box-library-part");
    let mut s = actual_size(&tmp, "102");
    page_150(&mut s);
    own_open(&mut s, "EW-GEN-PSU-1", "PSU-OWN");
    done(&mut s, "parts.draw", json!({"what": "box"}));
    // The box as it was always drawn: the first half of the pins on top, the rest below.
    let before = pins(&s);
    assert_eq!(before.len(), 7);
    assert_eq!(before.iter().filter(|p| p.1 == "top").count(), 3);
    assert!(
        device_box(&s).pins_from().is_none(),
        "the pins are the person's to change"
    );
    done(&mut s, "part.pin", json!({"add": true, "name": "OK"}));
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
    for (k, name) in ["L", "N", "PE"].into_iter().enumerate() {
        done(
            &mut s,
            "part.pin",
            json!({"pin": idx(name, 0), "side": "top", "at": 4 * (k + 1)}),
        );
    }
    for (k, (name, nth)) in [("+", 0), ("+", 1), ("-", 0), ("-", 1)].into_iter().enumerate() {
        done(
            &mut s,
            "part.pin",
            json!({"pin": idx(name, nth), "side": "bottom", "at": 4 * (k + 1)}),
        );
    }
    done(
        &mut s,
        "part.pin",
        json!({"pin": idx("OK", 0), "side": "right", "at": 4}),
    );
    // A new selection sorts the list by the box again (it keeps its rows while the pins are edited).
    done(&mut s, "part.box_select", json!({"pins": []}));
    let laid_out = pins(&s);
    assert_eq!(
        laid_out
            .iter()
            .filter(|p| p.1 == "bottom")
            .map(|p| (p.0.as_str(), p.2))
            .collect::<Vec<_>>(),
        [("+", 4), ("+", 8), ("-", 12), ("-", 16)]
    );
    assert_eq!(
        laid_out
            .iter()
            .filter(|p| p.1 == "right")
            .map(|p| p.0.as_str())
            .collect::<Vec<_>>(),
        ["OK"]
    );
    // One step back and forth in the box's own history.
    done(&mut s, "edit.undo", json!({}));
    assert_ne!(pins(&s), laid_out);
    done(&mut s, "edit.redo", json!({}));
    assert_eq!(pins(&s), laid_out);
    // Done: the sheet has the pin and the layout, changed and not saved yet; Save keeps them.
    done(&mut s, "parts.save", json!({}));
    assert!(s.symbol_editor().is_none(), "back on the part");
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the part");
    assert!(open.changed);
    assert!(open.sheet.pins.iter().any(|p| p.name == "OK"));
    // The project holds the part: Save's card first, then the change.
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "preview.apply", json!({}));
    leave_parts(&mut s);
    done(&mut s, "parts.open", json!({"part": "PSU-OWN"}));
    done(&mut s, "parts.draw", json!({"what": "box"}));
    assert_eq!(pins(&s), laid_out, "the saved part reads back as it was laid out");
    leave_parts(&mut s);
    // One click places the box as the part lays it out.
    done(&mut s, "page.goto", json!({"folio": "150"}));
    place_part(&mut s, "PSU-OWN", (10, 10));
    let at = grid_point(&s, (10, 10));
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
    assert_eq!(b.cps.len(), 8);
    let bottom: Vec<i64> = b
        .cps
        .iter()
        .filter(|c| c.2 == Dir::Down)
        .map(|c| (c.1.0 - at.0) / step)
        .collect();
    assert_eq!(bottom, [1, 2, 3, 4], "the four outputs along the bottom edge");
    assert_eq!(b.cps.iter().filter(|c| c.2 == Dir::Right).count(), 1);
    let labels: Vec<&str> = t
        .cps
        .iter()
        .filter(|c| c.rep == rep.id)
        .filter_map(|c| c.label.split_once(':').map(|(_, d)| d))
        .collect();
    assert!(
        labels.contains(&"OK"),
        "the new pin is a connection point of the device: {labels:?}"
    );
}

/// A part of one's own made in Parts, its pins named there and arranged on its box (a pin's rows in Properties: its
/// name and the potential it carries); two of its boxes placed. Its box arranged anew and the part saved, Save's card
/// names both boxes drawn anew, and applying the change redraws them in one undo step.
#[test]
fn a_new_part_s_box_is_arranged_and_its_save_draws_the_placed_boxes_anew() {
    let tmp = Tmp::new("device-box-new-part");
    let mut s = new_project(&tmp);
    page_150(&mut s);
    new_box_part(&mut s, "PSU-24", &["L", "N", "+", "-"]);
    done(&mut s, "parts.draw", json!({"what": "box"}));
    assert_eq!(
        pins(&s).iter().map(|p| (p.0.as_str(), p.1)).collect::<Vec<_>>(),
        [("L", "top"), ("N", "top"), ("+", "bottom"), ("-", "bottom")]
    );
    done(&mut s, "part.box_select", json!({"pins": [0]}));
    done(&mut s, "symbol.prop", json!({"key": "role", "value": "l"}));
    done(&mut s, "symbol.prop", json!({"key": "name", "value": "L1"}));
    assert_eq!(device_box(&s).pin_table()[0].name, "L1");
    // Test on a page waits for the part's Save.
    assert!(matches!(
        s.dispatch("symbol.scratch", &json!({})),
        Outcome::Failed { .. }
    ));
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
    place_part(&mut s, "PSU-24", (10, 10));
    place_part(&mut s, "PSU-24", (40, 10));
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
    assert_eq!(firsts(&s), [Some(0), Some(0)], "two boxes, L1 first");
    // Arranged anew: the two top pins change places; Save's card names the boxes it draws anew.
    done(&mut s, "parts.open", json!({"part": "PSU-24"}));
    done(&mut s, "parts.draw", json!({"what": "box"}));
    done(&mut s, "part.pin", json!({"pin": 0, "side": "top"}));
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.save", json!({}));
    let boxes = match s.properties().card().map(|c| &c.origin) {
        Some(CardOrigin::Part(card)) => card.boxes.clone(),
        other => panic!("the part's card: {other:?}"),
    };
    assert_eq!(boxes.len(), 2, "{boxes:?}");
    assert!(boxes.iter().all(|b| b.starts_with("Drawn anew: ")), "{boxes:?}");
    let rev = s.revision();
    done(&mut s, "preview.apply", json!({}));
    done(&mut s, "page.goto", json!({"folio": "150"}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one step");
    assert_eq!(
        firsts(&s),
        [Some(1), Some(1)],
        "N stands first on the top edge of both now"
    );
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
    new_box_part(&mut s, "PSU-5", &["L", "N", "PE", "+"]);
    done(&mut s, "parts.draw", json!({"what": "box"}));
    done(&mut s, "part.box_size", json!({"width": 16, "height": 12}));
    for (pin, side, at) in [(0, "top", 4), (1, "top", 8), (2, "top", 12), (3, "bottom", 8)] {
        done(&mut s, "part.pin", json!({"pin": pin, "side": side, "at": at}));
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
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
        place_part(&mut s, "PSU-5", (1, 1));
        let t = tables(&s);
        assert_eq!(t.grid, grid);
        let b = t.reps.iter().find_map(|r| r.black_box.clone()).expect("the box");
        let g = ew_api::symbols::units::Grid::new(grid.0 as u64, grid.1 as u64).unwrap();
        let on_grid = |v: i64| g.snap_point(ew_api::symbols::units::Nm(v)).map(|n| n.0) == Some(v);
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

/// A part that is not his is never arranged: Arrange… on a starter part is refused in words, and Make my own from it
/// is the way.
#[test]
fn arrange_is_refused_on_a_part_that_is_not_his() {
    let tmp = Tmp::new("device-box-not-his");
    let mut s = actual_size(&tmp, "102");
    done(&mut s, "parts.open", json!({"part": "EW-GEN-PSU-1"}));
    assert_eq!(
        crate::common::refused(&mut s, "parts.draw", json!({"what": "box"})),
        "reason-part-not-yours"
    );
    assert!(s.symbol_editor().is_none());
}

/// The device browser's picture of a part drawn as a box is its box with its pins as the part lays them out (it was
/// the bare frame every box shares).
#[test]
fn the_device_browser_shows_a_box_part_with_its_pins() {
    let tmp = Tmp::new("device-box-preview");
    let mut s = new_project(&tmp);
    new_box_part(&mut s, "ECP-8", &["24V", "0V", "1", "2"]);
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.back", json!({}));
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

/// To the part returns from the box to the part's sheet; a symbol drawn before waits in hand under it.
#[test]
fn to_the_part_returns_to_the_sheet_and_keeps_the_drawing_in_hand() {
    let tmp = Tmp::new("device-box-close");
    let mut s = new_project(&tmp);
    new_drawing(&mut s, "Lamp");
    done(
        &mut s,
        "symbol.draw",
        json!({"tool": "rectangle", "x": -2, "y": -2, "x2": 2, "y2": 2}),
    );
    done(&mut s, "parts.back", json!({}));
    new_box_part(&mut s, "LMP-1", &["X1", "X2"]);
    done(&mut s, "parts.draw", json!({"what": "box"}));
    assert!(s.symbol_editor().and_then(|e| e.device_box()).is_some());
    done(&mut s, "parts.back", json!({}));
    assert!(s.symbol_editor().is_none(), "the part's sheet shows");
    assert!(s.parts_place().and_then(|p| p.sheet.as_ref()).is_some());
    let kept = s.kept_symbol_editor().expect("the lamp waits in hand");
    assert!(kept.device_box().is_none() && kept.in_hand());
}

/// With no project open (Parts from the start page) a box of his own is arranged as the sheet is, and saved into My
/// parts with its layout.
#[test]
fn a_box_is_arranged_with_no_project_open() {
    let tmp = Tmp::new("device-box-alone");
    let mine =
        ew_api::library::LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), vec![], vec![]);
    let mut s = ShellModel::new(
        ew_viewmodel::shell::Mode::Headless,
        tmp.config().with_libraries(Some(mine)),
    );
    done(&mut s, "parts.open", json!({}));
    assert!(s.parts_without_project());
    new_box_part(&mut s, "PSU-ALONE", &["L", "N", "+", "-"]);
    done(&mut s, "parts.draw", json!({"what": "box"}));
    done(&mut s, "part.pin", json!({"pin": 0, "side": "left", "at": 4}));
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.save", json!({}));
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("the part");
    assert!(!open.changed, "saved");
    let layout = open.sheet.box_.as_ref().expect("its layout");
    assert!(
        layout
            .pins
            .iter()
            .any(|p| p.idx == 0 && side_word(&p.side) == "left"),
        "{layout:?}"
    );
}

/// A PLC card's pins come from its channels: the box shows, and a change of its pins is refused with the sentence
/// that says where they come from.
#[test]
fn a_plc_card_s_pins_are_not_edited_on_the_box() {
    let tmp = Tmp::new("device-box-plc");
    let mut s = new_project(&tmp);
    done(&mut s, "parts.new", json!({"start": "module"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", "DI-8")] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.draw", json!({"what": "box"}));
    let why = device_box(&s).pins_from().expect("the sentence");
    assert!(why.contains("PLC card"), "{why}");
    assert_eq!(
        s.dispatch("part.pin", &json!({"add": true})),
        Outcome::Failed { message: why }
    );
}

/// A box part of his own whose box nobody arranged (as 0.3.5 saved every one): saved, and saved again for its
/// description, it holds no layout and none of its placed boxes is drawn anew, so a box he drew on his page in a shape
/// of his own keeps it.
#[test]
fn a_box_nobody_arranged_is_saved_without_a_layout_and_redraws_no_placed_box() {
    let tmp = Tmp::new("device-box-unarranged");
    let mut s = actual_size(&tmp, "102");
    page_150(&mut s);
    own_open(&mut s, "EW-GEN-PSU-1", "PSU-OLD");
    let open = s.parts_place().and_then(|p| p.sheet.as_ref()).expect("his part");
    assert!(open.sheet.box_.is_none(), "Save wrote no layout into a box nobody arranged");
    leave_parts(&mut s);
    // His box, drawn by its two corners in a shape of his own, as 0.3.5 drew a box nobody arranged.
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "PSU-OLD"}));
    for k in [(10, 10), (50, 40)] {
        let at = grid_point(&s, k);
        cursor(&mut s, at);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "ui.dismiss", json!({}));
    let drawn = |s: &ShellModel| -> Vec<ew_api::output::TableBox> {
        tables(s).reps.iter().filter_map(|r| r.black_box.clone()).collect()
    };
    let before = drawn(&s);
    assert_eq!(before.len(), 1, "one box: {before:?}");
    done(&mut s, "parts.open", json!({"part": "PSU-OLD"}));
    done(
        &mut s,
        "parts.field",
        json!({"field": "description", "value": "Power supply 24 V, mine"}),
    );
    done(&mut s, "parts.save", json!({}));
    if let Some(CardOrigin::Part(card)) = s.properties().card().map(|c| &c.origin) {
        assert!(card.boxes.is_empty(), "no box drawn anew: {:?}", card.boxes);
        done(&mut s, "preview.apply", json!({}));
    }
    let open = s.parts_place().and_then(|p| p.sheet.as_ref());
    assert!(open.is_none_or(|o| o.sheet.box_.is_none()), "still no layout");
    leave_parts(&mut s);
    done(&mut s, "page.goto", json!({"folio": "150"}));
    assert_eq!(drawn(&s), before, "the box keeps its shape");
}
