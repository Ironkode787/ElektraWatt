// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP blind-drafter run's part findings (cycle 33, lane J33), through the text interface: the minimal part
//! chooser of the Parts row (draft FT row 1), part-first terminal placement into a strip (row 2) and no orphan device
//! after deleting its last representation (row 5). Each test repeats the run's step on a writable copy of Golden A and
//! reads the bill of materials of the saved project.

mod common;

use common::{Tmp, click, done, empty_point, state};
use ew_viewmodel::ew_api::checks;
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::ew_api::{BomMode, Export, Session};
use ew_viewmodel::properties::{RowValue, find_row};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`.
fn golden(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

fn failed(s: &mut ShellModel, action: &str, params: Value) -> String {
    match s.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

/// The component BOM of the saved project as CSV lines.
fn bom(tmp: &Tmp, s: &mut ShellModel) -> Vec<String> {
    done(s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "test").unwrap();
    let csv = session.export_csv(Export::Bom(BomMode::Component)).unwrap();
    csv.lines().map(str::to_owned).collect()
}

/// The BOM line of part number `pn` (a CSV field of its own).
fn line_of<'b>(bom: &'b [String], pn: &str) -> Option<&'b String> {
    bom.iter()
        .find(|l| l.split(',').any(|f| f.trim_matches('"') == pn))
}

fn parts_text(s: &ShellModel) -> String {
    let sheet = s.properties().sheet().expect("a sheet");
    let (_, row) = find_row(sheet, "device/parts").expect("the Parts row");
    match &row.value {
        RowValue::One { text, .. } => text.clone(),
        other => format!("{other:?}"),
    }
}

/// The texts of the open choice editor's options.
fn choices(s: &ShellModel) -> Vec<String> {
    s.properties()
        .choices()
        .map(|c| c.items.iter().map(|(t, _)| t.clone()).collect())
        .unwrap_or_default()
}

/// Finding 1: S51, -101F1 and a strip terminal take their part by part number through `props.set` and through the
/// chooser's options (`props.edit`, `props.input`, `props.choose`); the BOM lists them; a number that matches no
/// copy answers with a sentence, never serde text.
#[test]
fn row1_parts_are_chosen_by_part_number_and_reach_the_bom() {
    let tmp = Tmp::new("gpp-parts-1");
    let mut s = golden(&tmp, "101");
    // Clear the three devices' parts first (an empty text is the empty list).
    for tag in ["S51", "-101F1", "-X1031:1"] {
        done(&mut s, "select.set", json!({ "refs": [format!("tag:{tag}")] }));
        done(&mut s, "props.set", json!({"row": "device/parts", "value": ""}));
    }
    let cleared = bom(&tmp, &mut s);
    assert!(
        line_of(&cleared, "533356").is_none(),
        "S51's part is gone: {cleared:?}"
    );
    // The run's step: a bare number. Before the chooser this answered "invalid type: integer `533356`, expected a
    // sequence".
    done(&mut s, "select.set", json!({"refs": ["tag:S51"]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/parts", "value": "533356"}),
    );
    // -101F1 through the options: the editor lists the project's part copies, the typed text filters them.
    done(&mut s, "select.set", json!({"refs": ["tag:-101F1"]}));
    done(&mut s, "props.edit", json!({"row": "device/parts"}));
    let all = choices(&s);
    assert!(all.len() >= 10, "the project's part copies: {all:?}");
    assert!(all.iter().any(|c| c == "533356 (Eaton)"), "{all:?}");
    done(&mut s, "props.input", json!({"text": "A9P5"}));
    assert_eq!(choices(&s), ["A9P52610 (Schneider Electric)"]);
    done(&mut s, "props.choose", json!({"index": 0}));
    // A strip terminal through the text, in the row's display form.
    done(&mut s, "select.set", json!({"refs": ["tag:-X1031:1"]}));
    done(
        &mut s,
        "props.set",
        json!({"row": "device/parts", "value": "PT 2.5-TWIN (Phoenix Contact)"}),
    );
    assert_eq!(parts_text(&s), "PT 2.5-TWIN × 1");
    // A number no copy has: a sentence of the strings table.
    let m = failed(
        &mut s,
        "props.set",
        json!({"row": "device/parts", "value": "4711"}),
    );
    assert_eq!(
        m,
        "The project has no part with the part number 4711: choose one of the project's parts from the list"
    );
    let bom = bom(&tmp, &mut s);
    let s51 = line_of(&bom, "533356").expect("533356 in the BOM");
    assert!(s51.contains("S51"), "{s51}");
    let f1 = line_of(&bom, "A9P52610").expect("A9P52610 in the BOM");
    assert!(f1.contains("-101F1"), "{f1}");
    let x = line_of(&bom, "PT 2.5-TWIN").expect("PT 2.5-TWIN in the BOM");
    assert!(x.contains("-X1031:1"), "{x}");
}

/// Finding 2: `place.pick part="PT 2.5-TWIN" strip=… designation=…` places a terminal of that strip with that
/// designation, and the terminal carries the part (the run made a free device -103X1 and said nothing). Golden A's
/// -X1031:1 is deleted and placed again from its part; a strip that does not exist fails and lists the strips.
#[test]
fn row2_a_terminal_part_joins_its_strip_with_its_designation_and_carries_the_part() {
    let tmp = Tmp::new("gpp-parts-2");
    let mut s = golden(&tmp, "103");
    done(&mut s, "select.set", json!({"refs": ["tag:-X1031:1"]}));
    done(&mut s, "edit.delete_selection", json!({}));
    assert!(
        s.dispatch("select.set", &json!({"refs": ["tag:-X1031:1"]})) != Outcome::Done,
        "-X1031:1 is gone"
    );
    done(
        &mut s,
        "place.pick",
        json!({"part": "PT 2.5-TWIN", "strip": "-X1031", "designation": "1"}),
    );
    let ghost = state(&s)["canvas"]["ghost"].clone();
    assert_eq!(ghost["target"], "terminal", "{ghost}");
    let p = empty_point(s.canvas().unwrap());
    click(&mut s, p, json!({}));
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "select.set", json!({"refs": ["tag:-X1031:1"]}));
    assert_eq!(parts_text(&s), "PT 2.5-TWIN × 1");
    // A strip that does not exist fails and lists the project's strips (lane K33), not turned into a free device.
    let message = failed(
        &mut s,
        "place.pick",
        json!({"part": "PT 2.5-TWIN", "strip": "-X9999", "designation": "1"}),
    );
    assert!(
        message.starts_with("No terminal strip -X9999 in the project; its strips are "),
        "{message}"
    );
    assert!(s.picker().is_none(), "nothing is armed");
    let bom = bom(&tmp, &mut s);
    let line = line_of(&bom, "PT 2.5-TWIN").expect("PT 2.5-TWIN in the BOM");
    assert!(line.contains("-X1031:1"), "{line}");
    assert!(!line.contains("-103X"), "no free device: {line}");
}

/// The device of the only selected item (a `tag:` reference selects the device).
fn selected_device(s: &ShellModel) -> Uuid {
    let items = &s.selection().items;
    assert_eq!(items.len(), 1, "{items:?}");
    items[0].0
}

/// The page's representations of device `d`.
fn reps_of(s: &ShellModel, d: Uuid) -> Vec<Uuid> {
    s.canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .filter(|r| r.device == Some(d))
        .map(|r| r.id)
        .collect()
}

/// The EW-CORE-047 findings of the saved project.
fn core047(tmp: &Tmp, s: &mut ShellModel) -> Vec<String> {
    done(s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "test").unwrap();
    let q = session.query();
    checks::run(&q, &checks::FULL, checks::CheckMode::Online)
        .findings
        .into_iter()
        .filter(|f| f.rule == "EW-CORE-047")
        .map(|f| f.message)
        .collect()
}

/// The device E55 of folio 101 (the cabinet light, part SK 3102.000): one representation, a part, and no potential
/// definition, bond, page-less or off-page connection at its CPs.
const E55: &str = "85760c34-951e-5b8a-bbe2-3253c69e3db2";

/// Finding 5: deleting the only representation of a device deletes the device and its part use in the same command,
/// so the parts list has no phantom `-?G1`; one undo restores both. A device left without a representation by a
/// delete of its representation alone is EW-CORE-047. (Cycle 33 review finding 1: the run's -102G1 is kept now, as it
/// carries the +24V and 0V definitions and a bond; this row runs on E55, which carries no such intent.)
#[test]
fn row5_deleting_the_last_representation_deletes_the_device_and_one_undo_restores_both() {
    let tmp = Tmp::new("gpp-parts-5");
    let mut s = golden(&tmp, "101");
    let g = core047(&tmp, &mut s);
    assert!(g.is_empty(), "Golden A draws every device: {g:#?}");
    let device: Uuid = E55.parse().unwrap();
    let reps = reps_of(&s, device);
    assert_eq!(reps.len(), 1, "E55 is one symbol");
    done(&mut s, "select.set", json!({ "ids": [reps[0].to_string()] }));
    let before = bom(&tmp, &mut s);
    let light = line_of(&before, "SK 3102.000")
        .expect("SK 3102.000 in the BOM")
        .clone();
    assert!(light.contains("E55"), "{light}");
    let commands = s.recorded_commands().len();
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(s.recorded_commands().len(), commands + 1, "one command");
    // FT-709: the device went too, so the toast names no device left without a symbol.
    assert_eq!(unplaced_toast(&s), None);
    assert!(
        format!("{:?}", s.recorded_commands()[commands]).contains(E55),
        "the device went with its last representation"
    );
    let after = bom(&tmp, &mut s);
    assert!(
        line_of(&after, "SK 3102.000").is_none_or(|l| !l.contains("E55")),
        "no phantom E55: {after:?}"
    );
    assert!(core047(&tmp, &mut s).is_empty());
    // One undo step restores the device, its part and its symbol.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(reps_of(&s, device).len(), 1);
    assert_eq!(bom(&tmp, &mut s), before);
    // The representation deleted alone (a script's own command) leaves the device and its part: EW-CORE-047 reports
    // it, saying that the parts list still counts it.
    done(
        &mut s,
        "command.run",
        json!({"cmd": "edit.delete", "params": {"targets": [{"kind": "representation", "id": reps[0].to_string()}]}}),
    );
    let found = core047(&tmp, &mut s);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].ends_with("has a part but no representation on any page; the parts list still counts it"),
        "{found:?}"
    );
}

/// The visible toast when it says that a device has no symbol on a page any more (FT-709).
fn unplaced_toast(s: &ShellModel) -> Option<String> {
    s.toast()
        .map(|t| t.text.clone())
        .filter(|t| t.contains("no symbol on a page now"))
}

/// The potential definition names (sorted), the bond count and the device IDs of the saved project.
fn connect_intent(tmp: &Tmp, s: &mut ShellModel) -> (Vec<String>, usize, Vec<Uuid>) {
    done(s, "file.save", json!({}));
    let (session, _) = Session::open_read_only(&tmp.golden_copy(), "test").unwrap();
    let q = session.query();
    let mut names: Vec<String> = q
        .potential_defs()
        .iter()
        .map(|(_, p)| p.name.to_string())
        .collect();
    names.sort();
    let devices = q.devices().iter().map(|(id, _)| id.as_uuid()).collect();
    (names, q.bonds().iter().count(), devices)
}

/// Cycle 33 review finding 1: deleting the only box of -102G1 on folio 102 keeps the device, because its CPs carry
/// the +24V and 0V definitions and the 0V-PE bond, which a delete of the drawing must not drop unseen. The device is
/// left to EW-CORE-047.
#[test]
fn row5_a_device_whose_cps_carry_potentials_and_a_bond_is_kept() {
    let tmp = Tmp::new("gpp-parts-5c");
    let mut s = golden(&tmp, "102");
    let (names_before, bonds_before, _) = connect_intent(&tmp, &mut s);
    assert!(names_before.iter().any(|n| n == "+24V"), "{names_before:?}");
    assert!(names_before.iter().any(|n| n == "0V"), "{names_before:?}");
    done(&mut s, "select.set", json!({"refs": ["tag:-102G1"]}));
    let device = selected_device(&s);
    assert_eq!(reps_of(&s, device).len(), 1, "-102G1 is one black box");
    done(&mut s, "edit.delete_selection", json!({}));
    assert!(reps_of(&s, device).is_empty(), "the box is deleted");
    // FT-709: the kept device is named in the delete's toast, with where it stays.
    assert_eq!(
        unplaced_toast(&s).as_deref(),
        Some(
            "-102G1 (now -?G1) has no symbol on a page now; it stays in the Devices navigator and the parts list"
        )
    );
    let (names_after, bonds_after, devices) = connect_intent(&tmp, &mut s);
    assert!(devices.contains(&device), "the device remains");
    assert_eq!(names_after, names_before, "+24V and 0V are still defined");
    assert_eq!(bonds_after, bonds_before, "the bond still exists");
    let found = core047(&tmp, &mut s);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("G1"), "{found:?}");
}

/// Finding 5's limit: a device with another representation keeps its device when one representation is deleted.
#[test]
fn row5_a_device_with_another_representation_is_kept() {
    let tmp = Tmp::new("gpp-parts-5b");
    let mut s = golden(&tmp, "101");
    // A device that a page of Golden A draws more than once.
    let mut found = None;
    for folio in ["101", "102", "103", "104", "105", "106"] {
        if s.dispatch("page.goto", &json!({ "folio": folio })) != Outcome::Done {
            continue;
        }
        let tables = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
        found = tables
            .reps
            .iter()
            .filter_map(|r| r.device)
            .find(|d| tables.reps.iter().filter(|r| r.device == Some(*d)).count() > 1);
        if found.is_some() {
            break;
        }
    }
    let device = found.expect("a device drawn twice on one page");
    let reps = reps_of(&s, device);
    done(&mut s, "select.set", json!({ "ids": [reps[0].to_string()] }));
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(
        reps_of(&s, device),
        reps[1..],
        "the device keeps its other representations"
    );
    // FT-709: another representation remains, so no notice.
    assert_eq!(unplaced_toast(&s), None);
}

/// Finding 2, the run's own step: a new strip `-X??` and its first terminal placed from the part.
#[test]
fn row2_the_first_terminal_of_a_new_strip_from_its_part() {
    let tmp = Tmp::new("gpp-parts-2b");
    let mut s = golden(&tmp, "103");
    let qty = |bom: &[String]| -> String {
        let line = line_of(bom, "PT 2.5-TWIN").expect("PT 2.5-TWIN in the BOM");
        line.split(',').nth(2).unwrap_or_default().to_owned()
    };
    let before = qty(&bom(&tmp, &mut s));
    done(&mut s, "strip.new", json!({"letter": "X"}));
    done(
        &mut s,
        "place.pick",
        json!({"part": "PT 2.5-TWIN", "strip": "-X??", "designation": "1"}),
    );
    assert_eq!(state(&s)["canvas"]["ghost"]["target"], "terminal");
    let p = empty_point(s.canvas().unwrap());
    click(&mut s, p, json!({}));
    done(&mut s, "tool.select", json!({}));
    assert!(
        s.dispatch("select.set", &json!({"refs": ["tag:-103X1"]})) != Outcome::Done,
        "no free device -103X1"
    );
    let after = qty(&bom(&tmp, &mut s));
    assert_eq!(
        after.parse::<u32>().unwrap(),
        before.parse::<u32>().unwrap() + 1,
        "the new terminal carries PT 2.5-TWIN ({before} -> {after})"
    );
}
