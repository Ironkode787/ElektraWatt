// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Print wire numbers is found where a person looks for a project-wide switch: a checkbox in the
//! project's Properties that the filter "wire" finds, its state in the command search, a toast that says what it now
//! is for the whole project; Number wires by potential says what it did, also while the numbers are not printed.

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::properties::{RowValue, row_label, visible_rows};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

const ROW: &str = "project/settings.drawing.wire_numbers";

/// The label and the checkbox's state of Print wire numbers among the project's rows the filter `filter` leaves.
fn row_under_filter(s: &ShellModel, filter: &str) -> Option<(String, bool)> {
    let sheet = s.properties().sheet()?;
    visible_rows(sheet, &Default::default(), false, filter)
        .into_iter()
        .find(|v| v.row.key == ROW)
        .map(|v| {
            let on = matches!(&v.row.value, RowValue::One { json, .. } if json.as_bool() == Some(true));
            (row_label(v.section, v.row), on)
        })
}

fn palette_crumb(s: &mut ShellModel) -> String {
    if s.palette().is_none() {
        done(s, "tools.palette", json!({}));
    }
    s.set_palette_query("print wire numbers");
    let entries = s.palette_entries();
    let entry = entries
        .iter()
        .find(|e| e.id == "project.wire_numbers")
        .expect("the switch in the search");
    entry.breadcrumb.clone()
}

#[test]
fn print_wire_numbers_is_a_row_of_the_project_with_its_state_and_a_toast() {
    let tmp = Tmp::new("wire-numbers-row");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    assert_eq!(
        row_under_filter(&s, "wire"),
        Some(("Print wire numbers".to_owned(), false)),
        "the sample prints no wire numbers"
    );
    assert!(
        palette_crumb(&mut s).ends_with("· Off"),
        "{}",
        palette_crumb(&mut s)
    );

    // While the numbers are off, numbering by potential says that they show once printing is on.
    done(&mut s, "project.wire_numbering", json!({"on": true}));
    let said = toast(&s);
    assert!(said.contains("its potential's"), "{said}");
    assert!(said.contains("once Print wire numbers is on"), "{said}");

    done(&mut s, "props.project", json!({}));
    done(&mut s, "props.set", json!({"row": ROW, "value": "yes"}));
    assert!(s.wire_numbers_on(), "the tick turns the project's setting on");
    let said = toast(&s);
    assert!(said.contains("every page of the project"), "{said}");
    assert_eq!(row_under_filter(&s, "wire").map(|(_, on)| on), Some(true));
    assert!(
        palette_crumb(&mut s).ends_with("· On"),
        "{}",
        palette_crumb(&mut s)
    );

    // With printing on, numbering by potential says nothing about switching it on.
    done(&mut s, "project.wire_numbering", json!({"on": false}));
    let said = toast(&s);
    assert!(said.contains("its own number"), "{said}");
    assert!(!said.contains("once Print wire numbers"), "{said}");

    done(&mut s, "props.project", json!({}));
    done(&mut s, "props.set", json!({"row": ROW, "value": "no"}));
    assert!(!s.wire_numbers_on());
    assert!(toast(&s).contains("no longer print"), "{}", toast(&s));
}
