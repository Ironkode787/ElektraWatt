// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The G-PP blind-drafter run's part findings (cycle 33, lane J33), through the text interface: the minimal part
//! chooser of the Parts row (draft FT row 1), part-first terminal placement into a strip (row 2) and no orphan device
//! after deleting its last representation (row 5). Each test repeats the run's step on a writable copy of Golden A and
//! reads the bill of materials of the saved project.

mod common;

use common::{Tmp, done};
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
