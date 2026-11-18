// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A persona run of the third candidate with cables, by the window's own path: the cable's mark clicked shows the
//! cable, its Part row takes a catalogue cable whose cores replace the old ones, a cable is made of every selected
//! wire or the form says which wire cannot, and a wire of a cable says where its cross-section is changed.

use crate::support::Tmp;
use crate::wires_and_cables::{
    banner, click, key, named_wires, new_project, page_pixel, page_texts, select_both, state, tables, two_wires,
    typed,
};

use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::properties::{find_row, what_words};
use ew_ui::ew_viewmodel::shell::{Dialog, Outcome};
use ew_ui::visual::{Driver, Input};
use serde_json::json;

/// Cable… on the selected wires, the form's proposal applied.
fn cable_of_selection(d: &mut Driver) {
    d.input(&click(d.find("Cable").expect("Cable on the Draw tab")));
    d.settle();
    d.input(&click(d.find("Apply").unwrap()));
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "{}", banner(d));
    d.input(&key("escape"));
    d.settle();
}

/// Clicks the cable's mark on the page by its tag.
fn click_mark(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) {
    let mark = tables(d)
        .items
        .iter()
        .find(|i| i.kind == "wire_marker")
        .expect("the cable's mark")
        .id;
    let r = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .texts
        .iter()
        .find(|t| t.rep == mark)
        .copied()
        .expect("the tag's box")
        .rect;
    let middle = ((r.min().x.0 + r.max().x.0) / 2, (r.min().y.0 + r.max().y.0) / 2);
    let at = page_pixel(d, pixel, middle);
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
}

/// The type line the cable's mark prints under its tag (the text after the tag `-1W1`).
fn type_line(d: &mut Driver) -> String {
    let texts = page_texts(d);
    let at = texts.iter().position(|t| t == "-1W1").expect("the cable's tag");
    texts[at + 1].clone()
}

/// The persona run: the cable's mark clicked selected a raw "wire marker" with its internals in Properties, and the
/// Devices list called the cable on the page "not drawn". Now the mark is the cable's: Properties shows the cable's
/// rows under its tag and says it is its mark, and the Devices list counts the cable drawn where its cores run.
#[test]
fn the_cables_mark_shows_the_cable_and_the_cable_is_drawn() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    select_both(&mut d, &pixel);
    cable_of_selection(&mut d);
    click_mark(&mut d, &pixel);
    let sheet = d.shell_mut().properties().sheet().expect("a sheet").clone();
    assert_eq!(sheet.header.title, "-1W1");
    assert_eq!(
        what_words(sheet.header.what.as_ref().expect("what it is")),
        "Mark of the cable"
    );
    assert!(find_row(&sheet, "device/parts").is_some(), "the cable's rows");
    assert_eq!(
        d.shell_mut()
            .dispatch("navigator.show", &json!({ "kind": "devices" })),
        Outcome::Done
    );
    d.settle();
    let shell = d.shell_mut();
    let t = shell.nav(NavKind::Devices).and_then(|n| n.table()).unwrap();
    let row = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Device && n.label == "-1W1")
        .expect("the cable's row");
    assert!(row.badges.is_empty(), "{:?}", row.badges);
    d.shell_mut().shutdown();
}

/// The persona run: the cable's Part row refused a catalogue cable's number, and a part swapped there left the old
/// cores and a mark that read both types at once. Now the Part row takes the catalogue cable: its cores replace the
/// cable's (the wires stay cores, the neutral's blue core becomes the new type's black one), the mark reads the new type
/// only, and one Undo takes it all back.
#[test]
fn a_cables_part_row_takes_a_catalogue_cable_and_its_cores_follow() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    select_both(&mut d, &pixel);
    cable_of_selection(&mut d);
    let before = type_line(&mut d);
    assert!(page_texts(&mut d).iter().any(|t| t == "BU"), "N on blue");
    click_mark(&mut d, &pixel);
    let steps = d.shell_mut().recorded_commands().len();
    assert_eq!(
        d.shell_mut().dispatch(
            "props.set",
            &json!({"row": "device/parts[0].part", "value": "EW-GEN-CBL-11"})
        ),
        Outcome::Done,
        "{}",
        banner(&mut d)
    );
    d.settle();
    assert_eq!(
        d.shell_mut().recorded_commands().len(),
        steps + 1,
        "one step"
    );
    assert_eq!(type_line(&mut d), "4G1.5", "the new type only");
    let texts = page_texts(&mut d);
    assert!(
        texts.iter().any(|t| t == "BK") && !texts.iter().any(|t| t == "BU"),
        "the new cores: {texts:?}"
    );
    assert_eq!(
        d.shell_mut().dispatch("edit.undo", &json!({})),
        Outcome::Done
    );
    d.settle();
    assert_eq!(type_line(&mut d), before, "one Undo");
    d.shell_mut().shutdown();
}

