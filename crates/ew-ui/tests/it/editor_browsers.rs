// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The owner's hands-on test of 0.2.0-rc.1, the symbol editor and the browsers (FT-1690, FT-1691, FT-1694 to FT-1697,
//! FT-1702 to FT-1704), each on the window's own path: the pointer and the keys of the visual driver.

use crate::support;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::ew_viewmodel::tools::PickerRow;
use ew_ui::visual::{Driver, Input, parse_chord};
use serde_json::json;
use support::Tmp;

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn key(chord: &str) -> Input {
    let (key, mods) = parse_chord(chord).unwrap();
    Input::Key { key, mods, repeat: 1 }
}

fn drag(from: (i64, i64), to: (i64, i64)) -> Input {
    Input::Drag {
        from,
        to,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

/// Clicks the widget named `label`, then settles.
fn press(d: &mut Driver, label: &str) {
    let at = d
        .find(label)
        .unwrap_or_else(|| panic!("{label}: {:?}", d.labels()));
    d.input(&click(at));
    d.settle();
}

/// Clicks the first widget whose name starts with `start`, then settles.
fn press_starting(d: &mut Driver, start: &str) {
    let at = d
        .labels()
        .into_iter()
        .find(|(l, _)| l.starts_with(start))
        .unwrap_or_else(|| panic!("{start}: {:?}", d.labels()))
        .1;
    d.input(&click(at));
    d.settle();
}

/// A driver on a new project from the built-in template, on its first page, with its accessibility tree.
fn new_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// A driver with the symbol editor open on a new draft named "Drawn".
fn editor(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let new = json!({"key": "example.drawn", "name": "Drawn"});
    assert_eq!(d.shell_mut().dispatch("symbol.new", &new), Outcome::Done);
    d.settle();
    d
}

/// FT-1703: every tool of the editor's bar says in its tooltip, under its name and key, what it does and how.
#[test]
fn the_symbol_editor_s_tools_say_what_they_do_and_how() {
    let tmp = Tmp::new();
    let mut d = editor(&tmp);
    let line = d.find("Line").unwrap();
    d.input(&Input::Hover { at: line });
    d.input(&Input::Wait { ms: 1500 });
    d.settle();
    let texts = d.texts();
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Line (") && t.ends_with("\nDrag, or click two points")),
        "{texts:?}"
    );
    d.shell_mut().shutdown();
}

/// FT-1690: the drawing grid chosen in the editor's header is the step a drag snaps to; Snap to grid off takes the
/// pointer as it is; a connection point keeps to the connection grid whatever is chosen; the choice outlives the draft.
#[test]
fn the_symbol_editor_snaps_to_the_drawing_grid_chosen_and_pins_to_the_connection_grid() {
    let tmp = Tmp::new();
    let mut d = editor(&tmp);
    press(&mut d, "Drawing grid");
    press(&mut d, "4 mm");
    press(&mut d, "Line");
    let canvas = d.find("Drawing of Drawn").unwrap();
    d.input(&drag(
        (canvas.0 - 113, canvas.1 - 71),
        (canvas.0 + 59, canvas.1 + 37),
    ));
    d.settle();
    let ends = |d: &mut Driver, i: usize| -> Vec<i64> {
        let g =
            serde_json::to_value(&d.shell_mut().symbol_editor().unwrap().draft.symbol.graphics[i]).unwrap();
        let line = &g["line"];
        [&line["from"], &line["to"]]
            .iter()
            .flat_map(|p| [p[0].as_i64().unwrap(), p[1].as_i64().unwrap()])
            .collect()
    };
    let first = ends(&mut d, 0);
    assert!(
        first.iter().all(|v| v % 4_000_000 == 0),
        "on the 4 mm grid: {first:?}"
    );
    assert!(first.iter().any(|v| *v != 0), "{first:?}");
    // Off: the pointer as it is, to 0.01 mm, no longer on the 0.5 mm grid.
    press(&mut d, "Snap to grid");
    d.input(&drag(
        (canvas.0 - 113, canvas.1 - 71),
        (canvas.0 + 59, canvas.1 + 37),
    ));
    d.settle();
    let free = ends(&mut d, 1);
    assert!(free.iter().any(|v| v % 500_000 != 0), "off the grid: {free:?}");
    // A connection point lands on the connection grid with snapping off.
    press(&mut d, "Connection point");
    d.input(&click((canvas.0 - 113, canvas.1 - 71)));
    d.settle();
    let cp = d
        .shell_mut()
        .symbol_editor()
        .unwrap()
        .draft
        .symbol
        .cps
        .iter()
        .next()
        .unwrap()
        .pos;
    assert!(cp.x.0 % 4_000_000 == 0 && cp.y.0 % 4_000_000 == 0, "{cp:?}");
    // The grid is the session's, not the draft's: a new draft keeps 4 mm with snapping back on.
    press(&mut d, "Snap to grid");
    let new = json!({"key": "example.next", "name": "Next", "discard": true});
    assert_eq!(d.shell_mut().dispatch("symbol.new", &new), Outcome::Done);
    d.settle();
    press(&mut d, "Line");
    let canvas = d.find("Drawing of Next").unwrap();
    d.input(&drag(
        (canvas.0 - 113, canvas.1 - 71),
        (canvas.0 + 59, canvas.1 + 37),
    ));
    d.settle();
    let next = ends(&mut d, 0);
    assert!(next.iter().all(|v| v % 4_000_000 == 0), "still 4 mm: {next:?}");
    d.shell_mut().shutdown();
}

/// FT-1691: a symbol drawn in the editor takes the category chosen there, in the browser's words, and the symbol
/// browser lists it under that category once saved.
#[test]
fn a_drawn_symbol_is_listed_under_the_category_chosen_in_the_editor() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    press(&mut d, "Symbol editor");
    for (action, params) in [
        (
            "symbol.draw",
            json!({"tool": "rectangle", "x": -4, "y": 4, "x2": 4, "y2": 8}),
        ),
        (
            "symbol.cp",
            json!({"x": 0, "y": 0, "designation": "A1", "direction": "up"}),
        ),
    ] {
        assert_eq!(d.shell_mut().dispatch(action, &params), Outcome::Done, "{action}");
    }
    d.settle();
    press(&mut d, "Category");
    // The list scrolls to its R.
    let list = d.find("Connectors").unwrap();
    for _ in 0..8 {
        if d.find("Relays").is_some_and(|r| r.1 < list.1 + 150) {
            break;
        }
        d.input(&Input::Scroll {
            at: list,
            dx: 0,
            dy: 4,
            mods: Modifiers::NONE,
        });
        d.settle();
    }
    press(&mut d, "Relays");
    assert_eq!(
        d.shell_mut()
            .symbol_editor()
            .unwrap()
            .draft
            .function
            .category
            .0
            .as_str(),
        "Relay"
    );
    press(&mut d, "Save to library");
    assert_eq!(d.shell_mut().symbol_editor().unwrap().error, None);
    // The save's message stands over the header's corner until it goes.
    d.input(&Input::Wait { ms: 15_000 });
    press(&mut d, "Close editor");
    press(&mut d, "Symbol");
    press_starting(&mut d, "Relays, ");
    let picker = d.shell_mut().picker().unwrap();
    let names: Vec<String> = picker
        .rows
        .iter()
        .filter_map(|r| picker.row_symbol(*r).map(|s| s.name.clone()))
        .collect();
    assert!(names.iter().any(|n| n == "New symbol"), "{names:?}");
    d.shell_mut().shutdown();
}

/// FT-1696: the symbol browser lists symbols alone (no part, no terminal) and says so in its head; the device browser
/// lists the parts and says what a device is; the right rail names them as the browsers' titles do.
#[test]
fn the_symbol_and_the_device_browser_say_what_they_list_and_share_no_row() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    assert!(
        d.find("Symbol browser").is_some(),
        "the right rail: {:?}",
        d.labels()
    );
    assert!(
        d.find("Device browser").is_some(),
        "the right rail: {:?}",
        d.labels()
    );
    press(&mut d, "Symbol");
    let head = "A symbol is a drawing without a part. To place a part with its functions, use Device.";
    assert!(d.texts().iter().any(|t| t == head), "{:?}", d.texts());
    {
        let picker = d.shell_mut().picker().unwrap();
        assert!(!picker.rows.is_empty());
        for row in &picker.rows {
            let PickerRow::Symbol(i) = row else {
                panic!("a part in the symbol browser: {row:?}");
            };
            assert!(
                !picker.catalogue.symbols[*i].terminal,
                "a terminal in the symbol browser"
            );
        }
    }
    d.input(&key("escape"));
    d.settle();
    press(&mut d, "Device");
    let head = "A device is a part of the catalogue with its functions. To draw without a part, use Symbol.";
    assert!(d.texts().iter().any(|t| t == head), "{:?}", d.texts());
    let picker = d.shell_mut().picker().unwrap();
    assert!(picker.rows.iter().all(|r| matches!(r, PickerRow::Part(_))));
    d.shell_mut().shutdown();
}

/// FT-1694: a starter symbol offers Copy and edit alone and says why; the copy opens in the editor and saves to the
/// user's library; that copy then offers Edit symbol, which opens it under its own key, saving to its library.
#[test]
fn the_browser_edits_a_library_symbol_and_copies_a_read_only_one() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    press(&mut d, "Symbol");
    d.input(&Input::Type {
        text: "coil, rect".to_owned(),
    });
    d.settle();
    let why = d
        .texts()
        .into_iter()
        .find(|t| t.contains("is read-only. Copy and edit makes a copy"));
    assert!(why.is_some(), "{:?}", d.texts());
    press(&mut d, "Edit symbol");
    assert!(
        d.shell_mut().symbol_editor().is_none(),
        "a greyed Edit symbol opens nothing"
    );
    press(&mut d, "Copy and edit");
    {
        let editor = d.shell_mut().symbol_editor().expect("the editor on the copy");
        assert!(editor.draft.identity.is_none(), "a copy is a new item");
        assert!(editor.draft.key.starts_with("user."), "{}", editor.draft.key);
        assert!(!editor.dirty());
    }
    assert!(d.shell_mut().picker().is_none(), "the browser closed");
    press(&mut d, "Save to library");
    assert_eq!(d.shell_mut().symbol_editor().unwrap().error, None);
    let key = d.shell_mut().symbol_editor().unwrap().draft.key.clone();
    let folder = d.shell_mut().symbol_editor().unwrap().directory.clone();
    // The save's message stands over the header's corner until it goes.
    d.input(&Input::Wait { ms: 15_000 });
    press(&mut d, "Close editor");
    // The copy, in the user's library, is edited in place.
    press(&mut d, "Symbol");
    d.input(&Input::Type {
        text: "coil, rect".to_owned(),
    });
    d.settle();
    let row = {
        let picker = d.shell_mut().picker().unwrap();
        picker
            .rows
            .iter()
            .position(|r| {
                picker
                    .row_symbol(*r)
                    .is_some_and(|s| s.key.starts_with("library:"))
            })
            .expect("the copy's row")
    };
    assert_eq!(
        d.shell_mut().dispatch("picker.row", &json!({"to": row})),
        Outcome::Done
    );
    d.settle();
    d.input(&Input::Wait { ms: 300 });
    press(&mut d, "Edit symbol");
    let editor = d.shell_mut().symbol_editor().expect("the editor on the symbol");
    assert_eq!(editor.draft.key, key);
    assert!(editor.draft.identity.is_some(), "the same item");
    assert_eq!(editor.directory, folder);
    d.shell_mut().shutdown();
}

/// FT-1695: Terminal… asks for the strip (a new one here), the kind, how many and from which number; Place floats
/// the first terminal and one click lays the row.
#[test]
fn terminal_asks_its_strip_kind_and_count_and_one_click_lays_the_row() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    press(&mut d, "Terminal");
    assert!(
        d.texts().iter().any(|t| t == "Place terminals"),
        "the form: {:?}",
        d.texts()
    );
    // The first text field, How many, has the focus; Tab goes to From number; Enter presses Place.
    d.input(&key("ctrl+a"));
    d.input(&Input::Type { text: "3".to_owned() });
    d.input(&key("tab"));
    d.input(&Input::Type { text: "5".to_owned() });
    d.input(&key("enter"));
    d.settle();
    assert_eq!(d.shell_mut().tool().name(), "place", "the first terminal floats");
    let canvas = d
        .shell_mut()
        .canvas()
        .map(|c| c.page().tables.reps.iter().filter(|r| r.terminal).count())
        .unwrap();
    assert_eq!(canvas, 0);
    d.input(&click((640, 420)));
    d.settle();
    let reps: Vec<(i64, i64)> = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .filter(|r| r.terminal)
        .map(|r| r.pos)
        .collect();
    assert_eq!(reps.len(), 3, "{reps:?}");
    assert!(
        reps.windows(2).all(|w| w[0].1 == w[1].1 && w[0].0 < w[1].0),
        "a row: {reps:?}"
    );
    let labels: Vec<String> = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .cps
        .iter()
        .map(|c| c.label.clone())
        .collect();
    for n in ["5", "6", "7"] {
        assert!(
            labels.iter().any(|l| l.contains(&format!(":{n}"))),
            "terminal {n}: {labels:?}"
        );
    }
    d.shell_mut().shutdown();
}

/// Places the starter power supply from the device browser as a black box drawn by two clicks.
fn place_power_supply(d: &mut Driver) {
    press(d, "Device");
    d.input(&Input::Type {
        text: "EW-GEN-PSU".to_owned(),
    });
    d.settle();
    d.input(&key("enter"));
    d.settle();
    d.input(&click((560, 380)));
    d.input(&click((700, 470)));
    d.settle();
}

/// FT-1702: the power supply's box has its inputs (L, N, PE) on top and its outputs (+, +, −, −) at the bottom.
#[test]
fn the_power_supply_s_outputs_stand_together_at_the_bottom_of_its_box() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_power_supply(&mut d);
    let tables = d.shell_mut().canvas().unwrap().page().tables.clone();
    let rep = tables
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the box");
    let words: Vec<(u16, &str)> = rep
        .black_box
        .as_ref()
        .unwrap()
        .cps
        .iter()
        .map(|(idx, _, dir)| (*idx, dir.word()))
        .collect();
    for (idx, dir) in &words {
        let want = if *idx < 3 { "up" } else { "down" };
        assert_eq!(*dir, want, "pin {idx}: {words:?}");
    }
    d.shell_mut().shutdown();
}

/// FT-1704: a connection point's row chosen in a device's Properties is marked on the drawing: the canvas has its
/// point to ring, the point of that row's connection point.
#[test]
fn a_connection_point_chosen_in_properties_is_marked_on_the_drawing() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_power_supply(&mut d);
    d.input(&key("escape"));
    d.input(&key("escape"));
    d.settle();
    let id = {
        let tables = &d.shell_mut().canvas().unwrap().page().tables;
        tables
            .reps
            .iter()
            .find(|r| r.black_box.is_some())
            .unwrap()
            .id
            .to_string()
    };
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({"ids": [id]})),
        Outcome::Done
    );
    d.settle();
    assert_eq!(d.shell_mut().properties_cp_point(), None, "no row chosen yet");
    press(&mut d, "Filter properties");
    d.input(&Input::Type {
        text: "Connection point".to_owned(),
    });
    d.settle();
    // The first "+" of the supply, its row's value cell (the row's label is its first widget, its value its second):
    // the points are listed by their labels, -#1, -#2, +#1.
    let value = d
        .labels()
        .into_iter()
        .filter(|(l, _)| l == "Connection point 3")
        .nth(1)
        .expect("the value cell")
        .1;
    d.input(&click(value));
    d.settle();
    let at = d.shell_mut().properties_cp_point().expect("the point to mark");
    let tables = &d.shell_mut().canvas().unwrap().page().tables;
    let plus = tables.cps.iter().find(|c| c.label == "-1G1:+#1").unwrap();
    assert_eq!(at, plus.at, "the row's own point");
    d.shell_mut().shutdown();
}

/// FT-1697: the relay coil the symbol browser shows is a box wider than high across its vertical leads, as IEC 60617
/// draws a coil, no longer a resistor's upright box.
#[test]
fn the_relay_coil_is_drawn_wider_than_high() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    press(&mut d, "Symbol");
    d.input(&Input::Type {
        text: "coil, rect".to_owned(),
    });
    d.settle();
    let picker = d.shell_mut().picker().unwrap();
    let shape = picker.preview.clone().expect("the coil's picture");
    let boxed = shape
        .outline
        .iter()
        .find(|line| line.len() == 5 && line.first() == line.last())
        .expect("the coil's box");
    let (xs, ys): (Vec<i64>, Vec<i64>) = boxed.iter().copied().unzip();
    let width = xs.iter().max().unwrap() - xs.iter().min().unwrap();
    let height = ys.iter().max().unwrap() - ys.iter().min().unwrap();
    assert!(width > height, "{width} × {height}");
    d.shell_mut().shutdown();
}
