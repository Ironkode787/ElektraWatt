// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP blind-drafter run's part findings (cycle 33, lane J33), through the text interface: the minimal part
//! chooser of the Parts row (draft FT row 1), part-first terminal placement into a strip (row 2) and no orphan device
//! after deleting its last representation (row 5). Each test repeats the run's step on a writable copy of Golden A and
//! reads the bill of materials of the saved project.

mod common;

use common::{Tmp, click, done, empty_point, refused, state};
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
/// -X1031:1 is deleted and placed again from its part; a strip that does not exist is refused with its reason.
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
    // A strip that does not exist is refused with its reason, not turned into a free device.
    assert_eq!(
        refused(
            &mut s,
            "place.pick",
            json!({"part": "PT 2.5-TWIN", "strip": "-X9999", "designation": "1"})
        ),
        "reason-no-strip"
    );
    let bom = bom(&tmp, &mut s);
    let line = line_of(&bom, "PT 2.5-TWIN").expect("PT 2.5-TWIN in the BOM");
    assert!(line.contains("-X1031:1"), "{line}");
    assert!(!line.contains("-103X"), "no free device: {line}");
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
