// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wires, by the window's own
//! path: clicks, drags and keys.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::visual::{Driver, Input, parse_chord};
use ew_viewmodel::script::state_json;
use ew_viewmodel::shell::Outcome;
use ew_viewmodel::tools::Ghost;
use serde_json::{Value, json};

pub(crate) fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

pub(crate) fn key(chord: &str) -> Input {
    let (key, mods) = parse_chord(chord).unwrap();
    Input::Key { key, mods, repeat: 1 }
}

pub(crate) fn typed(text: &str) -> Input {
    Input::Type {
        text: text.to_owned(),
    }
}

pub(crate) fn state(d: &mut Driver) -> Value {
    serde_json::from_str(&state_json(d.shell_mut())).unwrap()
}

/// A driver over a new project from the built-in template, on its first page.
pub(crate) fn new_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// The window pixel of a sheet point, from two pointer positions the canvas reports.
pub(crate) fn pixel_map(d: &mut Driver) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    let probe = |d: &mut Driver, at: (i64, i64)| {
        d.input(&Input::Hover { at });
        d.settle();
        let p = state(d)["canvas"]["pointer_nm"].clone();
        (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())
    };
    let (f0, f1) = ((700, 400), (1000, 700));
    let (s0, s1) = (probe(d, f0), probe(d, f1));
    move |sheet: (i64, i64)| {
        (
            f0.0 + (sheet.0 - s0.0) * (f1.0 - f0.0) / (s1.0 - s0.0),
            f0.1 + (sheet.1 - s0.1) * (f1.1 - f0.1) / (s1.1 - s0.1),
        )
    }
}

/// The window pixel of a page point.
pub(crate) fn page_pixel(
    d: &mut Driver,
    pixel: &impl Fn((i64, i64)) -> (i64, i64),
    p: (i64, i64),
) -> (i64, i64) {
    pixel(d.shell_mut().canvas().unwrap().page().snapper.page_to_sheet(p))
}

/// The page tables of the page shown.
pub(crate) fn tables(d: &mut Driver) -> std::sync::Arc<ew_viewmodel::ew_api::output::PageTables> {
    std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables)
}

/// Clicks the widget named `label`, selects its text, types `text` and presses Enter.
pub(crate) fn fill(d: &mut Driver, label: &str, text: &str) {
    d.input(&click(
        d.find(label).unwrap_or_else(|| panic!("no widget {label:?}")),
    ));
    d.input(&key("ctrl+a"));
    d.input(&typed(text));
    d.input(&key("enter"));
    d.settle();
}

/// The centre of the first widget whose accessible name starts with `prefix`.
pub(crate) fn starting_with(d: &Driver, prefix: &str) -> (i64, i64) {
    d.labels()
        .into_iter()
        .find(|(l, _)| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no widget named {prefix:?}"))
        .1
}

/// Places the relay of the device browser's Relays at `at`, then Escape.
pub(crate) fn place_relay(d: &mut Driver, at: (i64, i64)) {
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("relay"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(d, "Relay with four"),
        mods: Modifiers::NONE,
    });
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
    d.input(&key("escape"));
    d.settle();
}

/// Two source interruption points, L1 and N, side by side, each with a wire down to the coil of a relay under it:
/// two wires, as the owner drew them. Returns the pixel map.
pub(crate) fn two_wires(d: &mut Driver) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    two_named_wires(d, ["L1", "N"])
}

/// [`two_wires`] with the points named `names`, left to right.
pub(crate) fn two_named_wires(d: &mut Driver, names: [&str; 2]) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    named_wires(d, &names)
}

/// [`two_wires`] with one point and relay per name in `names`, left to right, 100 pixels apart, ending at x 1000.
pub(crate) fn named_wires(d: &mut Driver, names: &[&str]) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    let n = i64::try_from(names.len()).unwrap();
    let xs: Vec<i64> = (0..n).map(|i| 1000 - 100 * (n - 1 - i)).collect();
    for x in &xs {
        place_relay(d, (*x, 620));
    }
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    for (name, x) in names.iter().zip(xs.iter().copied()) {
        fill(d, "Interruption point name", name);
        d.input(&Input::Hover { at: (x, 420) });
        d.settle();
        d.input(&click((x, 420)));
        d.settle();
    }
    d.input(&key("escape"));
    d.settle();
    let pixel = pixel_map(d);
    let t = tables(d);
    let mut ips: Vec<(i64, i64)> = t
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .filter_map(|r| t.cps.iter().find(|c| c.rep == r.id).map(|c| c.at))
        .collect();
    ips.sort();
    // The coils' A1, left to right (their tags follow the project's numbering).
    let mut a1: Vec<(i64, i64)> = t
        .cps
        .iter()
        .filter(|c| c.label.ends_with(":A1"))
        .map(|c| c.at)
        .collect();
    a1.sort();
    let ends: Vec<_> = ips.iter().copied().zip(a1.iter().copied()).collect();
    d.input(&click(d.find("Wire").unwrap()));
    d.settle();
    for (a, b) in ends {
        for p in [a, b] {
            let at = page_pixel(d, &pixel, p);
            d.input(&Input::Hover { at });
            d.input(&click(at));
            d.settle();
        }
    }
    d.input(&key("escape"));
    d.settle();
    assert_eq!(tables(d).connections.len(), names.len(), "a wire per point");
    pixel
}

/// The banner's text (where a refused step says why).
pub(crate) fn banner(d: &mut Driver) -> String {
    state(d)["banner"].as_str().unwrap_or_default().to_owned()
}

/// Arms the Marker tool by its button, and drags it across every wire of the page, halfway down; returns the end of
/// the drag.
fn mark_across(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) -> (i64, i64) {
    arm_marker(d);
    drag_across(d, pixel)
}

/// Arms the Wire annotation tool: Annotate on the Draw tab; where the window folds the Annotate group, the same name
/// opens the group's menu, whose entry Wire annotation arms it.
fn arm_marker(d: &mut Driver) {
    d.input(&click(d.find("Annotate").expect("Annotate on the Draw tab")));
    d.settle();
    if d.shell_mut().tool().name() != "marker" {
        let entry = starting_with(d, "Wire annotation");
        d.input(&click(entry));
        d.settle();
    }
    assert_eq!(d.shell_mut().tool().name(), "marker");
}

/// Drags the armed Marker tool across every wire of the page, halfway down; returns the end of the drag.
fn drag_across(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) -> (i64, i64) {
    let t = tables(d);
    let ys: Vec<i64> = t
        .connections
        .iter()
        .flat_map(|c| c.polyline.iter().map(|p| p.1))
        .collect();
    let mid_y = (ys.iter().min().unwrap() + ys.iter().max().unwrap()) / 2;
    let xs: Vec<i64> = t
        .connections
        .iter()
        .flat_map(|c| c.polyline.iter().map(|p| p.0))
        .collect();
    let (x0, x1) = (
        xs.iter().min().unwrap() - 4_000_000,
        xs.iter().max().unwrap() + 4_000_000,
    );
    let from = page_pixel(d, pixel, (x0, mid_y));
    let to = page_pixel(d, pixel, (x1, mid_y));
    d.input(&Input::Drag {
        from,
        to,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    d.settle();
    assert_eq!(d.shell_mut().tool().state(), "typing");
    to
}

/// Two wires, the Marker tool dragged across both, cross-section 1.5 and colour BK typed in its fields,
/// the pointer back on the page, Enter: a marker stands over both wires and says 1.5 and BK for each. (Enter placed
/// nothing after a drag, and one colour for two wires was refused.) A colour the project does not know is refused
/// in words that name the codes it knows, and the lower-case code is taken.
#[test]
fn the_marker_tool_marks_two_wires_with_a_typed_colour() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    let to = mark_across(&mut d, &pixel);
    fill(&mut d, "Cross-section", "1.5");
    fill(&mut d, "Colours", "black-ish");
    let said = banner(&mut d);
    assert!(
        said.contains("\"black-ish\" is not a colour") && said.contains("BK, BN") && !said.contains('/'),
        "{said:?}"
    );
    fill(&mut d, "Colours", "bk");
    d.input(&Input::Hover { at: to });
    d.input(&key("enter"));
    d.settle();
    let t = tables(&mut d);
    let marker = t.items.iter().find(|i| i.kind == "wire_marker");
    assert_eq!(
        marker.and_then(|m| m.text.as_deref()),
        Some("1.5 BK BK"),
        "the window said {:?}",
        banner(&mut d)
    );
    d.shell_mut().shutdown();
}

/// Two wires drawn, a marker placed over them with 0.75 typed, then the
/// wires made the cores of a 3G1.5 cable: the wire stayed 0.75 until it was drawn again. A wire that is a cable's core
/// has its core's cross-section and colour at once, however it got there: Properties shows 1.5 and BN, from the
/// cable, and offers no override there.
#[test]
fn a_wire_made_a_core_takes_the_cores_cross_section_over_its_marker() {
    use ew_viewmodel::properties::{RowAction, RowValue, chip_text, find_row};
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    let to = mark_across(&mut d, &pixel);
    fill(&mut d, "Cross-section", "0.75");
    d.input(&Input::Hover { at: to });
    d.input(&key("enter"));
    d.input(&key("escape"));
    d.settle();
    select_both(&mut d, &pixel);
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    d.settle();
    d.input(&key("tab"));
    d.input(&typed("3G1.5"));
    d.input(&click(d.find("Apply").unwrap()));
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "{}", banner(&mut d));
    d.input(&key("escape"));
    let at = on_wire(&mut d, &pixel, 0);
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    let sheet = d
        .shell_mut()
        .properties()
        .sheet()
        .expect("the wire's properties")
        .clone();
    for (key, want) in [
        ("connection/derived.csa", "1.5"),
        ("connection/derived.colour", "BN"),
    ] {
        let (_, row) = find_row(&sheet, key).unwrap_or_else(|| panic!("{key}"));
        match &row.value {
            RowValue::One { text, .. } => assert!(text.contains(want), "{key}: {text}"),
            other => panic!("{key}: {other:?}"),
        }
        let chip = chip_text(&row.provenance);
        assert!(
            chip.contains("cable -1W1") && chip.contains("core BN"),
            "{key}: {chip}"
        );
        assert!(
            !row.actions.contains(&RowAction::Override),
            "{key}: changed at the cable"
        );
    }
    d.shell_mut().shutdown();
}

/// The text of the one Wire marker of the page that is not among `before`, and every marker's ID now.
fn new_marker(d: &mut Driver, before: &[String]) -> (Option<String>, Vec<String>) {
    let t = tables(d);
    let markers: Vec<_> = t.items.iter().filter(|i| i.kind == "wire_marker").collect();
    let text = markers
        .iter()
        .find(|m| !before.contains(&m.id.to_string()))
        .and_then(|m| m.text.clone());
    (text, markers.iter().map(|m| m.id.to_string()).collect())
}

/// Clicks the Marker field named `label` and types `text`, leaving the field without Enter.
fn type_in(d: &mut Driver, label: &str, text: &str) {
    d.input(&click(
        d.find(label).unwrap_or_else(|| panic!("no widget {label:?}")),
    ));
    d.input(&key("ctrl+a"));
    d.input(&typed(text));
    d.settle();
}

/// Again: the Marker's cross-section and colours typed and left by a
/// click on the other field, then the line dragged, then Enter: the marker printed the wires' own 0.75, not the 1.5
/// typed (a field was taken on Enter only). Every order a person takes keeps the fields: fields then line; line then
/// fields and a click on the sheet; Tab between the fields; line then fields and Enter in the last one. Each time
/// the marker prints what the fields show.
#[test]
fn the_marker_fields_hold_in_every_order_of_use() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    let mut seen = Vec::new();
    let mut check = |d: &mut Driver, csa: &str, colour: &str, order: &str| {
        let (text, now) = new_marker(d, &seen);
        assert_eq!(
            text,
            Some(format!("{csa} {colour} {colour}")),
            "{order}: the window said {:?}",
            banner(d)
        );
        let ghost = d.shell_mut().tool_ghost();
        assert!(
            matches!(&ghost, Some(Ghost::Marker { text: Some(t), .. }) if *t == format!("{csa} {colour}")),
            "{order}: the fields still say what the marker printed: {ghost:?}"
        );
        seen = now;
    };
    // The tester's order: the fields, then the line, then Enter.
    arm_marker(&mut d);
    type_in(&mut d, "Cross-section", "1.5");
    type_in(&mut d, "Colours", "BK");
    let _ = drag_across(&mut d, &pixel);
    d.input(&key("enter"));
    d.settle();
    check(&mut d, "1.5", "BK", "fields, line, Enter");
    // The line, then the fields, left by a click on the sheet, then Enter.
    arm_marker(&mut d);
    let to = drag_across(&mut d, &pixel);
    type_in(&mut d, "Cross-section", "2.5");
    type_in(&mut d, "Colours", "BU");
    d.input(&Input::Hover { at: to });
    d.input(&click(to));
    d.settle();
    d.input(&key("enter"));
    d.settle();
    check(&mut d, "2.5", "BU", "line, fields, click on the sheet, Enter");
    // Tab from one field to the next, then the line and Enter.
    arm_marker(&mut d);
    type_in(&mut d, "Cross-section", "4");
    d.input(&key("tab"));
    d.input(&typed("RD"));
    d.input(&key("tab"));
    d.settle();
    let _ = drag_across(&mut d, &pixel);
    d.input(&key("enter"));
    d.settle();
    check(&mut d, "4", "RD", "Tab between the fields");
    // The line, then the fields, and Enter in the last field.
    arm_marker(&mut d);
    let to = drag_across(&mut d, &pixel);
    type_in(&mut d, "Cross-section", "6");
    type_in(&mut d, "Colours", "GY");
    d.input(&key("enter"));
    d.settle();
    d.input(&Input::Hover { at: to });
    d.input(&key("enter"));
    d.settle();
    check(&mut d, "6", "GY", "line, fields, Enter in the field");
    d.shell_mut().shutdown();
}

/// The window pixel a quarter along the longest segment of the `i`-th wire of the page (clear of a cable's mark, which
/// stands in the middle).
pub(crate) fn on_wire(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64), i: usize) -> (i64, i64) {
    let t = tables(d);
    let (p, q) = t.connections[i]
        .polyline
        .windows(2)
        .map(|s| (s[0], s[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .expect("a segment");
    page_pixel(d, pixel, ((3 * p.0 + q.0) / 4, (3 * p.1 + q.1) / 4))
}

/// A wire whose potential is given the type L1, and nothing else, takes a phase's colour and cross-section
/// (1.5 mm², the mains class), not the control circuit's 0.75 mm²: its marker says so.
#[test]
fn a_wire_of_an_l1_potential_takes_the_phases_cross_section() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    // The first wire, clicked, then Draw ▸ Define potential…: its form, the name and the type L1.
    let at = on_wire(&mut d, &pixel, 0);
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    let shell = d.shell_mut();
    assert_eq!(shell.dispatch("potential.draw", &json!({})), Outcome::Done);
    shell.set_palette_field(0, "L1");
    shell.set_palette_field(1, "l1");
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "potential.define");
    d.settle();
    d.input(&key("escape"));
    d.settle();
    let to = mark_across(&mut d, &pixel);
    d.input(&Input::Hover { at: to });
    d.input(&key("enter"));
    d.settle();
    let t = tables(&mut d);
    let marker = t.items.iter().find(|i| i.kind == "wire_marker");
    let text = marker.and_then(|m| m.text.clone()).unwrap_or_default();
    // The L1 wire 1.5 mm² BK, the other wire (no type) the control circuit's 0.75 mm².
    assert!(text.starts_with("1.5/0.75 BK"), "{text:?}");
    d.shell_mut().shutdown();
}

/// Selects the page's two wires by the pointer: a click on the first, a Shift+click on the second.
pub(crate) fn select_both(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) {
    for (i, mods) in [(0, Modifiers::NONE), (1, Modifiers::SHIFT)] {
        let at = on_wire(d, pixel, i);
        d.input(&Input::Hover { at });
        d.input(&Input::Click {
            at,
            button: PointerButton::Primary,
            mods,
        });
        d.settle();
    }
}

/// The core cell of the Cable form's row `i`.
fn core_cell(d: &mut Driver, i: usize) -> String {
    match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => f.table.as_ref().unwrap().rows[i].cells[0].clone(),
        other => panic!("not the Cable form: {other:?}"),
    }
}

/// Two wires selected, Cable on the Draw tab, the type typed as a person writes it (3G1.5), the second
/// wire's core changed in the form, Apply: one cable with the type's three cores, the two wires its cores as the form
/// said, in one undo step, and the cable's mark over the wires at once: its tag, its type and each core's name.
/// Cable on a selection that holds a cable's wire opens the same form on that cable.
#[test]
fn cable_makes_the_selected_wires_a_cable_in_one_step() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    select_both(&mut d, &pixel);
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    d.settle();
    // The cable type comes first: the starter cables that fit two wires, a two-core one first and chosen;
    // typing a size instead makes a cable without a part.
    match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => {
            let part = &f.fields[0];
            assert_eq!(part.name, "part");
            let chosen = part
                .options
                .iter()
                .find(|o| o.0 == part.value)
                .map(|o| o.1.clone());
            assert!(
                chosen
                    .as_deref()
                    .is_some_and(|w| w.contains(" · 2X") && w.ends_with(" · BN BU")),
                "{chosen:?} of {:?}",
                part.options
            );
        }
        other => panic!("not the Cable form: {other:?}"),
    }
    // The form's first text field, the tag, has the keys; Tab goes to the type, whose text is selected.
    d.input(&key("tab"));
    d.input(&typed("3G1.5"));
    d.settle();
    // The type's cores: the first wire BN, the second BU; the second takes GNYE instead.
    assert_eq!(
        (core_cell(&mut d, 0), core_cell(&mut d, 1)),
        ("BN".to_owned(), "BU".to_owned())
    );
    let cell = starting_with(&d, "Core, ");
    let second = d
        .labels()
        .into_iter()
        .filter(|(l, _)| l.starts_with("Core, "))
        .nth(1)
        .map_or(cell, |(_, at)| at);
    d.input(&click(second));
    d.input(&key("ctrl+a"));
    d.input(&typed("GNYE"));
    d.settle();
    d.input(&click(d.find("Apply").unwrap()));
    d.settle();
    assert!(
        d.shell_mut().dialog().is_none(),
        "the form is closed: {}",
        banner(&mut d)
    );
    let added = &d.shell_mut().recorded_commands()[steps..];
    assert_eq!(added.len(), 1, "one step");
    assert_eq!(added[0].cmd, "cable.create");
    let conductors = added[0].params["conductors"].as_array().unwrap().clone();
    let t = tables(&mut d);
    let wire = |i: usize| json!([t.connections[i].id.to_string()]);
    assert_eq!(
        conductors
            .iter()
            .map(|c| (c["core"].clone(), c["csa"].clone(), c["wires"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("BN"), json!("1.5"), wire(0)),
            (json!("BU"), json!("1.5"), json!([])),
            (json!("GNYE"), json!("1.5"), wire(1)),
        ]
    );
    // Properties of the first wire, clicked: its cable and core, and their cross-section and colour.
    d.input(&key("escape"));
    d.settle();
    let at = on_wire(&mut d, &pixel, 0);
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    {
        use ew_viewmodel::properties::{RowValue, find_row};
        let sheet = d
            .shell_mut()
            .properties()
            .sheet()
            .expect("the wire's properties")
            .clone();
        let text = |key: &str| match find_row(&sheet, key).map(|(_, r)| r.value.clone()) {
            Some(RowValue::One { text, .. }) => text,
            other => panic!("{key}: {other:?}"),
        };
        assert_eq!(text("connection/kind"), "-1W1 BN");
    }
    // The cable's mark is on the page at once, nobody placed it: one line over both wires, the cable's tag,
    // its type line under it, and each core's name beside its own wire.
    let t = tables(&mut d);
    let marks: Vec<_> = t.items.iter().filter(|i| i.kind == "wire_marker").collect();
    assert_eq!(marks.len(), 1, "one mark");
    let texts = page_texts(&mut d);
    for said in ["-1W1", "3G1.5", "BN", "GNYE"] {
        assert!(texts.iter().any(|t| t == said), "{said} on the page: {texts:?}");
    }
    assert!(
        !texts.iter().any(|t| t.contains("0.75")),
        "no 0.75 on the page: {texts:?}"
    );
    // The tag moves as a symbol's text does: the mark clicked by its tag, then the tag dragged up.
    let mark = marks[0].id;
    let tag = |d: &mut Driver| {
        d.shell_mut()
            .canvas()
            .unwrap()
            .page()
            .texts
            .iter()
            .find(|t| t.rep == mark)
            .copied()
            .expect("the tag's box")
    };
    let before = tag(&mut d);
    let r = before.rect;
    let middle = ((r.min().x.0 + r.max().x.0) / 2, (r.min().y.0 + r.max().y.0) / 2);
    d.input(&key("escape"));
    let at = page_pixel(&mut d, &pixel, middle);
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    d.input(&Input::Drag {
        from: at,
        to: (at.0, at.1 - 40),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    d.settle();
    let moved = tag(&mut d);
    assert!(moved.moved, "the tag is moved");
    assert!(
        moved.at.y < before.at.y,
        "up: {:?} from {:?}",
        moved.at,
        before.at
    );
    assert_eq!(
        d.shell_mut().recorded_commands().last().map(|c| c.cmd.clone()),
        Some("fields.set".to_owned())
    );
    // The same form on a selection holding a cable's wire: the cable's tag, type and cores.
    d.input(&key("escape"));
    d.settle();
    select_both(&mut d, &pixel);
    d.input(&click(d.find("Cable").unwrap()));
    d.settle();
    assert_eq!(
        (core_cell(&mut d, 0), core_cell(&mut d, 1)),
        ("BN".to_owned(), "GNYE".to_owned())
    );
    d.shell_mut().shutdown();
}

/// A driver over a new project made from the wizard's company standard (its strips numbered `{letter}{page}{counter}`,
/// its other devices `{page}{letter}{counter}`), on its first page.
fn company_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let shell = d.shell_mut();
    assert_eq!(
        shell.dispatch("tools.company_standard", &json!({})),
        Outcome::Done
    );
    d.settle();
    let basic = json!({"dir": tmp.0.join("company.ew")});
    assert_eq!(d.shell_mut().dispatch("wizard.finish", &basic), Outcome::Done);
    d.settle();
    let _ = d.shell_mut().dispatch("wizard.cancel", &json!({}));
    let project = json!({"dir": tmp.0.join("p.ew")});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    // A frame of the project: the wizard's success toast waits for its own time, so settling alone paints none.
    d.input(&Input::Wait { ms: 100 });
    d
}

/// The tag field of the Cable form.
fn form_tag(d: &mut Driver) -> String {
    match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => {
            f.fields.iter().find(|x| x.name == "tag").unwrap().value.clone()
        }
        other => panic!("not the Cable form: {other:?}"),
    }
}

/// Two wires from the points N and L1 (N's wire the left one), both selected,
/// Cable…: the form put N on the brown core and L1 on the blue one, and proposed a tag in the strips' format
/// (`-W11`) while the cable took `-1W1`. The neutral takes the blue core and the phase the brown (HD 308 S2) whatever
/// their order, retyping the type keeps that, the tag proposed is the tag the cable gets, and the toast says it in
/// words.
#[test]
fn cable_gives_n_the_blue_core_and_proposes_the_tag_it_takes() {
    let tmp = Tmp::new();
    let mut d = company_project(&tmp);
    let pixel = two_named_wires(&mut d, ["N", "L1"]);
    select_both(&mut d, &pixel);
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    d.settle();
    let n_first = |d: &mut Driver| (core_cell(d, 0), core_cell(d, 1));
    assert_eq!(
        n_first(&mut d),
        ("BU".to_owned(), "BN".to_owned()),
        "N blue, L1 brown"
    );
    let proposed = form_tag(&mut d);
    // Retyped as a person writes it: the neutral stays on blue, no wire takes the green-yellow core.
    d.input(&key("tab"));
    d.input(&typed("3G1.5"));
    d.settle();
    assert_eq!(n_first(&mut d), ("BU".to_owned(), "BN".to_owned()), "after 3G1.5");
    d.input(&click(d.find("Apply").unwrap()));
    d.settle();
    assert!(
        d.shell_mut().dialog().is_none(),
        "the form is closed: {}",
        banner(&mut d)
    );
    let said = state(&mut d)["toast"].to_string();
    assert!(
        said.contains(&format!("Cable {proposed} made from 2 wires.")),
        "{said}"
    );
    // The first wire, clicked: its cable is the one the form proposed.
    d.input(&key("escape"));
    d.settle();
    let at = on_wire(&mut d, &pixel, 0);
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    use ew_viewmodel::properties::{RowValue, find_row};
    let sheet = d
        .shell_mut()
        .properties()
        .sheet()
        .expect("the wire's properties")
        .clone();
    match find_row(&sheet, "connection/kind").map(|(_, r)| r.value.clone()) {
        Some(RowValue::One { text, .. }) => assert!(text.starts_with(&format!("{proposed} ")), "{text}"),
        other => panic!("{other:?}"),
    }
    d.shell_mut().shutdown();
}

/// The texts the page shown prints, groups followed.
pub(crate) fn page_texts(d: &mut Driver) -> Vec<String> {
    use ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(g) => walk(&g.items, out),
                _ => {}
            }
        }
    }
    let list = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().list);
    let mut out = Vec::new();
    walk(&list.items, &mut out);
    out
}

/// Show wire numbers on the pages (Draw ▸ Connect ▸ Show wire numbers on the pages), run from the command search, prints the number of each
/// of the page's two wires on the page of a new project (`1.1`, `1.2`: the page's folio and the counter); ticked off,
/// the numbers are gone.
#[test]
fn print_wire_numbers_puts_each_wires_number_on_the_page() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let _ = two_wires(&mut d);
    let numbers = |d: &mut Driver| {
        let mut n: Vec<String> = page_texts(d)
            .into_iter()
            .filter(|t| t.starts_with("1.") && t[2..].chars().all(|c| c.is_ascii_digit()))
            .collect();
        n.sort();
        n
    };
    assert_eq!(
        numbers(&mut d),
        Vec::<String>::new(),
        "no numbers before the setting"
    );
    // The command search, clicked: "Show wire numbers on the pages" typed, Enter.
    let tick = |d: &mut Driver| {
        d.input(&click(
            d.find("Search commands and devices").expect("the command search"),
        ));
        d.settle();
        d.input(&typed("Show wire numbers on the pages"));
        d.settle();
        d.input(&key("enter"));
        d.settle();
    };
    tick(&mut d);
    assert_eq!(numbers(&mut d), ["1.1", "1.2"]);
    tick(&mut d);
    assert_eq!(numbers(&mut d), Vec::<String>::new(), "ticked off, no numbers");
    d.shell_mut().shutdown();
}

/// The core cells of the Cable form's rows, in the rows' order.
fn core_cells(d: &mut Driver) -> Vec<String> {
    match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => f
            .table
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .map(|r| r.cells[0].clone())
            .collect(),
        other => panic!("not the Cable form: {other:?}"),
    }
}

/// The sample's power supply on folio 101 (its L and N from the breaker -101F1, its PE from the strip's PE terminal),
/// the three wires selected, Cable…: the form proposes a type with a green-yellow core (`3G1.5`), since one wire is
/// the protective conductor, and gives N the blue core, PE the green-yellow and L the brown. Retyped to a type without a blue core and
/// back, the cores are proposed afresh: before, the neutral kept the brown core the first type gave it.
#[test]
fn cable_on_a_supply_with_pe_proposes_a_g_type_and_retyping_proposes_the_cores_again() {
    let tmp = Tmp::new();
    let dir = tmp.sample("s.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let shell = d.shell_mut();
    assert_eq!(shell.dispatch("file.open", &json!({"dir": dir})), Outcome::Done);
    assert_eq!(
        shell.dispatch("page.goto", &json!({"folio": "101"})),
        Outcome::Done
    );
    d.settle();
    // The power supply's N, PE and L wires, by its connection points (each takes one wire).
    let ids: Vec<String> = {
        let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
        ["-101G1:N", "-101G1:PE", "-101G1:L"]
            .iter()
            .map(|label| {
                let cp = tables.cps.iter().find(|c| c.label == *label).expect(label);
                assert_eq!(cp.connections.len(), 1, "{label}");
                cp.connections[0].to_string()
            })
            .collect()
    };
    let out = d.shell_mut().dispatch("select.set", &json!({ "ids": ids }));
    assert_eq!(out, Outcome::Done, "{out:?}");
    d.settle();
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    d.settle();
    // The type: the cable part chosen for the wires, else the size proposed; with a PE wire a G type.
    let type_text = |d: &mut Driver| match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => {
            let part = f.fields.iter().find(|x| x.name == "part");
            let chosen = part.and_then(|p| p.options.iter().find(|o| !p.value.is_empty() && o.0 == p.value));
            chosen.map_or_else(
                || f.fields.iter().find(|x| x.name == "type").unwrap().value.clone(),
                |o| o.1.clone(),
            )
        }
        other => panic!("not the Cable form: {other:?}"),
    };
    // The form lists the wires in their records' order: N blue, PE green-yellow, L brown, in that order.
    let mut cores: Vec<(&String, &str)> = ids.iter().zip(["BU", "GNYE", "BN"]).collect();
    cores.sort();
    let cores: Vec<&str> = cores.into_iter().map(|(_, c)| c).collect();
    let proposed = type_text(&mut d);
    assert!(
        proposed == "3G1.5" || proposed.contains(" · 3G"),
        "a PE among the wires: a G type: {proposed}"
    );
    assert_eq!(core_cells(&mut d), cores, "N blue, PE green-yellow, L brown");
    // Retyped as a person writes it: first a type without blue and green-yellow cores, then one with them again.
    d.input(&key("tab"));
    d.input(&typed("3x1.5"));
    d.settle();
    d.input(&key("ctrl+a"));
    d.input(&typed("3G2.5"));
    d.settle();
    assert_eq!(type_text(&mut d), "3G2.5");
    assert_eq!(core_cells(&mut d), cores, "after 3x1.5 and back to 3G2.5");
    d.shell_mut().shutdown();
}
