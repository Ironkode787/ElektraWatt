// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! rc.2, the owner's F9 and F12: a device shows what belongs to what it is. Properties says what the selected thing
//! is in trade words, shows only the rows that belong to it, and its menus offer only what fits it.

use crate::common;

use common::{Tmp, at, describe_line, done, failed, refused};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::ew_api::output::TableRep;
use ew_viewmodel::properties::{
    PropertySheet, RowGroup, RowValue, chip_text, find_row, groups, visible_rows, what_words,
};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// A new project with the electrical page 101 and the fluid page 102.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let dir = tmp.0.join("new.ew");
    done(
        &mut s,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    for (folio, ty) in [("101", "multi_line"), ("102", "fluid")] {
        done(
            &mut s,
            "page.new",
            json!({"folio": folio, "type": ty, "title": ty}),
        );
    }
    s
}

fn place_at(s: &mut ShellModel, k: (i64, i64)) {
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(k.0), sn.page_of_index(k.1)));
    done(s, "canvas.press", at(p, json!({"alt": true})));
    done(s, "canvas.release", at(p, json!({"alt": true})));
    let _ = s.dispatch("ui.dismiss", &json!({}));
    let _ = s.dispatch("ui.dismiss", &json!({}));
}

fn reps(s: &ShellModel) -> Vec<TableRep> {
    s.canvas().unwrap().page().tables.reps.clone()
}

fn place(s: &mut ShellModel, params: serde_json::Value, k: (i64, i64)) -> TableRep {
    let before = reps(s);
    done(s, "place.pick", params);
    place_at(s, k);
    reps(s)
        .into_iter()
        .find(|r| before.iter().all(|b| b.id != r.id))
        .expect("a new symbol")
}

/// The header's line under the title of the one symbol `rep`'s sheet.
fn what_of(s: &mut ShellModel, rep: &TableRep) -> String {
    done(s, "select.set", json!({"ids": [rep.id.to_string()]}));
    let sheet = s.properties().sheet().expect("a sheet");
    what_words(sheet.header.what.as_ref().expect("what the device is"))
}

/// Item 1: the header says what the device is, from its symbol and the disciplines of its points: an air supply is
/// pneumatic and nothing electrical, a 5/2 valve pneumatic with its one solenoid, a relay's coil electrical; the same
/// words reach the text interface.
#[test]
fn the_header_says_what_a_device_is() {
    let tmp = Tmp::new("props-kind-what");
    let mut s = new_project(&tmp);
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let air = place(
        &mut s,
        json!({"symbol": "ew.iso.air-supply@qet10", "letter": "X"}),
        (10, 10),
    );
    let what = what_of(&mut s, &air);
    assert!(what.ends_with(" · pneumatic"), "{what}");
    let line = describe_line(&s, "properties what").expect("the describe line");
    assert!(line.contains(&what), "{line}");
    let valve = place(&mut s, json!({"part": "EW-GEN-VLV-1", "template": 0}), (30, 10));
    let what = what_of(&mut s, &valve);
    assert!(what.ends_with(" · pneumatic, 1 solenoid"), "{what}");
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let coil = place(&mut s, json!({"part": "EW-GEN-RLY-1", "template": 0}), (10, 10));
    let what = what_of(&mut s, &coil);
    assert!(what.ends_with(" · electrical"), "{what}");
    s.shutdown();
}

/// The sheet of the one symbol `rep`.
fn sheet_of(s: &mut ShellModel, rep: &TableRep) -> PropertySheet {
    done(s, "select.set", json!({"ids": [rep.id.to_string()]}));
    s.properties().sheet().expect("a sheet").clone()
}

/// The keys of every row of `sheet`.
fn keys(sheet: &PropertySheet) -> Vec<String> {
    sheet
        .sections
        .iter()
        .flat_map(|x| &x.rows)
        .map(|r| r.key.clone())
        .collect()
}

/// Item 2 (F12): an air supply's sheet lists no row of another trade (no bus port, terminal, strip, cable or assembly,
/// no cross-reference, no place in a parent it does not have); Main holds what a drafter sets first, in that order; the
/// rare rows wait in More, closed, and the filter still finds them. A valve's Main lists its ports with what each
/// carries, a port without a medium saying where to set one; a relay's coil keeps its cross-reference.
#[test]
fn only_the_rows_that_belong_are_shown() {
    let tmp = Tmp::new("props-kind-rows");
    let mut s = new_project(&tmp);
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let air = place(
        &mut s,
        json!({"symbol": "ew.iso.air-supply@qet10", "letter": "X"}),
        (10, 10),
    );
    let sheet = sheet_of(&mut s, &air);
    let all = keys(&sheet);
    for other in [
        "device/terminal",
        "device/strip",
        "device/cable",
        "device/assembly",
        "device/position",
        "function/bus_port",
        "function/kind.ip",
        "representation/derived.xref",
    ] {
        assert!(
            !all.iter().any(|k| k == other),
            "{other} on an air supply: {all:?}"
        );
    }
    let device = &sheet.sections[0];
    let main: Vec<&str> = groups(device, false)
        .into_iter()
        .find(|(g, _)| *g == RowGroup::Main)
        .map(|(_, rows)| rows.iter().map(|r| r.key.as_str()).collect())
        .unwrap_or_default();
    assert_eq!(
        main,
        [
            "device/derived.tag",
            "device/function_text",
            "device/parts",
            "device/derived.structure.location",
            "device/derived.medium"
        ]
    );
    let collapsed = s.properties_collapsed();
    assert!(collapsed.contains("device/more"), "More is closed: {collapsed:?}");
    let shown = |filter: &str| -> Vec<String> {
        visible_rows(&sheet, &collapsed, false, filter)
            .iter()
            .map(|v| v.row.key.clone())
            .collect()
    };
    assert!(!shown("").contains(&"device/kind".to_owned()));
    assert!(
        shown("Kind").contains(&"device/kind".to_owned()),
        "the filter finds a row in More"
    );
    // A valve's ports in Main, each saying it carries no medium yet and, in its chip, where to choose one.
    let valve = place(&mut s, json!({"part": "EW-GEN-VLV-1", "template": 0}), (30, 10));
    let sheet = sheet_of(&mut s, &valve);
    let (_, port) = find_row(&sheet, "device/derived.port[0]").expect("a port row");
    let none = strings::text(ew_viewmodel::key!("props-port-no-medium"));
    assert!(
        matches!(&port.value, RowValue::One { text, .. } if text == none),
        "{port:?}"
    );
    let what_to_do = strings::text(ew_viewmodel::key!("chip-via-port-no-medium"));
    assert!(chip_text(&port.provenance).contains(what_to_do), "{port:?}");
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let coil = place(&mut s, json!({"part": "EW-GEN-RLY-1", "template": 0}), (10, 10));
    let sheet = sheet_of(&mut s, &coil);
    assert!(keys(&sheet).iter().any(|k| k == "representation/derived.xref"));
    assert!(!keys(&sheet).iter().any(|k| k.starts_with("device/derived.port[")));
    s.shutdown();
}

/// The canvas menu of the one symbol `rep`: each entry's label, action and whether it is checked.
fn menu(s: &mut ShellModel, rep: &TableRep) -> Vec<(String, &'static str, bool)> {
    done(s, "select.set", json!({"ids": [rep.id.to_string()]}));
    canvas_menu(s, MenuSpot::Object, None)
        .into_iter()
        .map(|i| (i.label, i.id, i.checked))
        .collect()
}

/// Item 3 (F9): an air supply's menu offers no Define potential… (its command refuses a fluid port in words) but its
/// medium: one entry per medium of the project in words, the current one checked, and New medium…; a valve fed by
/// nothing says in its menu that its ports carry no medium.
#[test]
fn an_air_supplys_menu_offers_its_medium_and_no_potential() {
    let tmp = Tmp::new("props-kind-menu");
    let mut s = new_project(&tmp);
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(
        &mut s,
        "fluid.medium_new",
        json!({"name": "Compressed air", "p_nom": "6"}),
    );
    let air = place(
        &mut s,
        json!({"symbol": "ew.iso.air-supply@qet10", "letter": "X"}),
        (10, 10),
    );
    let items = menu(&mut s, &air);
    assert!(
        items.iter().all(|(_, id, _)| *id != "potential.draw"),
        "{items:?}"
    );
    assert_eq!(
        refused(&mut s, "potential.draw", json!({})),
        "reason-potential-fluid"
    );
    let label = "Medium: Compressed air, 6 bar";
    let checked = |items: &[(String, &'static str, bool)]| {
        items
            .iter()
            .find(|(l, _, _)| l == label)
            .map(|(_, _, c)| *c)
            .unwrap_or_else(|| panic!("the medium entry: {items:?}"))
    };
    assert!(!checked(&items), "no medium yet");
    assert!(
        items.iter().any(|(_, id, _)| *id == "fluid.medium_new"),
        "{items:?}"
    );
    let entry = canvas_menu(&s, MenuSpot::Object, None)
        .into_iter()
        .find(|i| i.label == label)
        .expect("the entry");
    done(&mut s, entry.id, entry.params);
    assert!(checked(&menu(&mut s, &air)), "the current medium is checked");
    // The command refuses a fluid port in words.
    let message = failed(
        &mut s,
        "command.run",
        json!({"cmd": "potential.define", "params": {
            "at": {"cp": {"function": air.functions[0].to_string(), "idx": 0}}, "name": "L1"}}),
    );
    assert!(message.contains("is a fluid port"), "{message}");
    // A valve on no tube: its ports carry nothing, and its menu goes to the air supply, where a medium is set (rc.2).
    let valve = place(&mut s, json!({"part": "EW-GEN-VLV-1", "template": 0}), (30, 10));
    assert!(
        menu(&mut s, &valve)
            .iter()
            .any(|(l, id, _)| l.starts_with("No medium: go to the air supply") && *id == "nav.goto")
    );
    s.shutdown();
}

/// Item 4 (F9, last sentence): the symbol editor sets what a symbol is drawn for. Drawn as pneumatic, its function
/// types follow (a valve offered, no contact), a new point takes the discipline and one point can differ; the symbol
/// browser shows it, and a device of the saved symbol is one Properties calls pneumatic: a source with its Medium row,
/// whatever its category.
#[test]
fn a_symbol_drawn_as_pneumatic_gives_a_pneumatic_device() {
    use ew_viewmodel::ew_api::symbols::schema::Discipline;
    use ew_viewmodel::shell::symbol_editor::function_kinds;
    let tmp = Tmp::new("props-kind-symbol");
    let mut s = tmp.empty_shell(Mode::Headless);
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("project"), "template": "builtin"}),
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd": "page.create", "params": {"number": 2, "grid": {"num": 4000000, "den": 1}}}),
    );
    done(&mut s, "page.goto", json!({"folio": "2"}));
    done(
        &mut s,
        "symbol.new",
        json!({"key": "example.air", "name": "Air point"}),
    );
    done(&mut s, "symbol.discipline", json!({"discipline": "pneumatic"}));
    let editor = s.symbol_editor().expect("the editor");
    assert_eq!(editor.discipline(), "pneumatic");
    let kinds = function_kinds(editor.discipline());
    assert!(kinds.contains(&"valve") && !kinds.contains(&"no"), "{kinds:?}");
    done(
        &mut s,
        "symbol.draw",
        json!({"tool": "rectangle", "x": 0, "y": 0, "x2": 8, "y2": 8}),
    );
    done(&mut s, "symbol.cp", json!({"x": 0, "y": 0, "designation": "1"}));
    let pin = |s: &ShellModel| {
        let editor = s.symbol_editor().expect("the editor");
        let pins: Vec<Discipline> = editor
            .draft
            .function
            .cps
            .iter()
            .map(|c| c.discipline.clone())
            .collect();
        let points: Vec<Discipline> = editor
            .draft
            .symbol
            .cps
            .iter()
            .map(|c| c.discipline.clone())
            .collect();
        (pins, points)
    };
    assert_eq!(
        pin(&s),
        (vec![Discipline::Pneumatic], vec![Discipline::Pneumatic])
    );
    // One point can differ, and back.
    done(
        &mut s,
        "symbol.discipline",
        json!({"discipline": "electrical", "cp": 1}),
    );
    assert_eq!(
        pin(&s),
        (vec![Discipline::Electrical], vec![Discipline::Electrical])
    );
    done(
        &mut s,
        "symbol.discipline",
        json!({"discipline": "pneumatic", "cp": 1}),
    );
    done(
        &mut s,
        "symbol.save",
        json!({"directory": tmp.0.join("symbols.ewlib")}),
    );
    done(&mut s, "symbol.close", json!({}));
    done(&mut s, "tool.place", json!({}));
    let row = s
        .picker()
        .expect("the browser")
        .catalogue
        .symbols
        .iter()
        .find(|r| r.name.starts_with("Air point"))
        .cloned()
        .expect("the saved symbol");
    assert_eq!(row.discipline, Some(Discipline::Pneumatic));
    done(&mut s, "ui.dismiss", json!({}));
    let air = place(&mut s, json!({"symbol": row.key, "letter": "X"}), (10, 10));
    let what = what_of(&mut s, &air);
    assert!(what.ends_with(" · pneumatic"), "{what}");
    let sheet = sheet_of(&mut s, &air);
    assert!(
        find_row(&sheet, "device/derived.medium").is_some(),
        "{:?}",
        keys(&sheet)
    );
    s.shutdown();
}

/// rc.2 persona run, entries 1 and 2: the project's Properties put what a drafter fills first under Main, in the
/// order of a title block: the title, the project and drawing numbers, the description, the parties, the dates.
#[test]
fn the_project_shows_what_a_drafter_fills_first_under_main() {
    let tmp = Tmp::new("props-project-main");
    let mut s = new_project(&tmp);
    done(&mut s, "props.project", json!({}));
    let sheet = s.properties().sheet().expect("a sheet").clone();
    let project = sheet
        .sections
        .iter()
        .find(|x| x.kind == "project")
        .expect("the project's section");
    let main: Vec<&str> = groups(project, false)
        .into_iter()
        .find(|(g, _)| *g == RowGroup::Main)
        .map(|(_, rows)| rows.iter().map(|r| r.key.as_str()).collect())
        .unwrap_or_default();
    let want = [
        "project/master.document_title",
        "project/master.project_ref",
        "project/master.drawing_id",
        "project/master.description",
        "project/people.customer",
        "project/people.author",
        "project/master.created",
    ];
    let first: Vec<&str> = main.iter().copied().filter(|k| want.contains(k)).collect();
    assert_eq!(first, want, "{main:?}");
    s.shutdown();
}
