// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The owner's test of 0.2.0-rc.1, his rows about wires (FT-1684, FT-1685, FT-1688, FT-1692), by the window's own
//! path: clicks, drags and keys.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::script::state_json;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::ew_viewmodel::tools::Ghost;
use ew_ui::visual::{Driver, Input, parse_chord};
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

fn typed(text: &str) -> Input {
    Input::Type {
        text: text.to_owned(),
    }
}

fn state(d: &mut Driver) -> Value {
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
pub(crate) fn tables(d: &mut Driver) -> std::sync::Arc<ew_ui::ew_viewmodel::ew_api::output::PageTables> {
    std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables)
}

/// Clicks the widget named `label`, selects its text, types `text` and presses Enter.
fn fill(d: &mut Driver, label: &str, text: &str) {
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
fn two_named_wires(d: &mut Driver, names: [&str; 2]) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    for x in [900, 1000] {
        place_relay(d, (x, 620));
    }
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    for (name, x) in names.into_iter().zip([900, 1000]) {
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
    let ends = [(ips[0], a1[0]), (ips[1], a1[1])];
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
    assert_eq!(tables(d).connections.len(), 2, "two wires");
    pixel
}

/// The banner's text (where a refused step says why).
fn banner(d: &mut Driver) -> String {
    state(d)["banner"].as_str().unwrap_or_default().to_owned()
}

/// Arms the Marker tool by its button, and drags it across every wire of the page, halfway down; returns the end of
/// the drag.
fn mark_across(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) -> (i64, i64) {
    arm_marker(d);
    drag_across(d, pixel)
}

/// Arms the Marker tool: Marker on the Draw tab, or Wire marker in its Annotate group's menu where the window folds
/// the group.
fn arm_marker(d: &mut Driver) {
    let marker = match d.find("Marker") {
        Some(at) => at,
        None => {
            d.input(&click(d.find("Annotate").expect("the Annotate group")));
            d.settle();
            starting_with(d, "Wire marker")
        }
    };
    d.input(&click(marker));
    d.settle();
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

/// FT-1684: two wires, the Marker tool dragged across both, cross-section 1.5 and colour BK typed in its fields,
/// the pointer back on the page, Enter: a marker stands over both wires and says 1.5 and BK for each. (Enter placed
/// nothing after a drag, and one colour for two wires was refused.) A colour the project does not know is refused
/// in words that name the codes it knows, and the lower-case code is taken.
#[test]
fn ft1684_the_marker_tool_marks_two_wires_with_a_typed_colour() {
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

/// A play-tester of the second candidate (FT-1684 again): the Marker's cross-section and colours typed and left by a
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

/// The window pixel of the middle of the longest segment of the `i`-th wire of the page.
fn on_wire(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64), i: usize) -> (i64, i64) {
    let t = tables(d);
    let (p, q) = t.connections[i]
        .polyline
        .windows(2)
        .map(|s| (s[0], s[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .expect("a segment");
    page_pixel(d, pixel, ((p.0 + q.0) / 2, (p.1 + q.1) / 2))
}

/// FT-1688: a wire whose potential is given the type L1, and nothing else, takes a phase's colour and cross-section
/// (1.5 mm², the mains class), not the control circuit's 0.75 mm²: its marker says so.
#[test]
fn ft1688_a_wire_of_an_l1_potential_takes_the_phases_cross_section() {
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
fn select_both(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) {
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
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => {
            f.table.as_ref().unwrap().rows[i].cells[0].clone()
        }
        other => panic!("not the Cable form: {other:?}"),
    }
}

/// FT-1685: two wires selected, Cable on the Draw tab, the type typed as a person writes it (3G1.5), the second
/// wire's core changed in the form, Apply: one cable with the type's three cores, the two wires its cores as the form
/// said, in one undo step, and the marker over the wires prints the cores' cross-section and colours, not 0.75.
/// Cable on a selection that holds a cable's wire opens the same form on that cable.
#[test]
fn ft1685_cable_makes_the_selected_wires_a_cable_in_one_step() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    select_both(&mut d, &pixel);
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    d.settle();
    // The form's first field, the tag, has the keys; Tab goes to the type, whose text is selected.
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
        use ew_ui::ew_viewmodel::properties::{RowValue, find_row};
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
    // The marker across both wires: the cores' 1.5 mm² and colours.
    d.input(&key("escape"));
    d.settle();
    let to = mark_across(&mut d, &pixel);
    d.input(&Input::Hover { at: to });
    d.input(&key("enter"));
    d.settle();
    let t = tables(&mut d);
    let marker = t.items.iter().find(|i| i.kind == "wire_marker");
    assert_eq!(marker.and_then(|m| m.text.as_deref()), Some("1.5 BN GNYE"));
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
    d.settle();
    d
}

/// The tag field of the Cable form.
fn form_tag(d: &mut Driver) -> String {
    match d.shell_mut().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f.fields[0].value.clone(),
        other => panic!("not the Cable form: {other:?}"),
    }
}

/// A play-tester of the second candidate: two wires from the points N and L1 (N's wire the left one), both selected,
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
    use ew_ui::ew_viewmodel::properties::{RowValue, find_row};
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
fn page_texts(d: &mut Driver) -> Vec<String> {
    use ew_ui::ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
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

/// FT-1692: Print wire numbers (Draw ▸ Connect ▸ Print wire numbers), run from the command search, prints the number of each
/// of the page's two wires on the page of a new project (`1.1`, `1.2`: the page's folio and the counter); ticked off,
/// the numbers are gone.
#[test]
fn ft1692_print_wire_numbers_puts_each_wires_number_on_the_page() {
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
    // The command search, clicked: "Print wire numbers" typed, Enter.
    let tick = |d: &mut Driver| {
        d.input(&click(
            d.find("Search commands and devices").expect("the command search"),
        ));
        d.settle();
        d.input(&typed("Print wire numbers"));
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
