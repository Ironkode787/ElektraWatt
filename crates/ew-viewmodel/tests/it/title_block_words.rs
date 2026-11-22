// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Project properties and the title block speak one vocabulary (FT-1803): a row of the project that a cell of the
//! sample's title block prints is named as that cell, in en-GB and in nl-NL.

use crate::common::{Tmp, done};
use ew_viewmodel::properties::row_label;
use ew_viewmodel::shell::{Dialog, ShellModel};
use ew_viewmodel::{key, strings};
use serde_json::json;

/// The label of the project's row `key` in the sheet on show.
fn label_of(s: &ShellModel, key: &str) -> String {
    let sheet = s.properties().sheet().expect("the project's sheet");
    sheet
        .sections
        .iter()
        .find_map(|section| {
            let row = section.rows.iter().find(|r| r.key == key)?;
            Some(row_label(section, row))
        })
        .unwrap_or_else(|| panic!("{key} is a row of the project"))
}

#[test]
fn the_project_rows_a_cell_prints_are_named_as_the_cell() {
    let tmp = Tmp::new("titleblock-words");
    let mut s = tmp.open_sample(true);
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.show_advanced", json!({"on": true}));
    done(&mut s, "page.goto", json!({"folio": "101"}));
    // The title-block path each cell prints, the project's row that feeds it, and the cell's Dutch label in the
    // shipped title block (the sample prints in English, so Title block fields… names its cells in English).
    let fed = [
        ("project.reference", "project/master.project_ref", "Project"),
        ("project.drawing_id", "project/master.drawing_id", "Tekening"),
        ("project.object", "project/master.object_id", "Object"),
        ("project.edited", "project/master.edited", "Opgeslagen"),
        (
            "revision.current.author.name",
            "project/people.author",
            "Getekend",
        ),
    ];
    done(&mut s, "props.project", json!({}));
    let rows: Vec<String> = fed.iter().map(|(_, key, _)| label_of(&s, key)).collect();
    done(&mut s, "page.title_block", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the form: {:?}", s.dialog())
    };
    for ((path, key, _), label) in fed.iter().zip(rows) {
        let cell = strings::format(key!("titleblock-prints-in"), &[&label]);
        let note = s.form_note(&form, path).unwrap_or_default();
        assert!(
            note.contains(&cell),
            "{key}: {label:?} is not the cell of {note:?}"
        );
    }
    let _ = s.dispatch("ui.dismiss", &json!({}));
    assert!(strings::set_locale("nl"));
    done(&mut s, "props.project", json!({}));
    for (_, key, dutch) in fed {
        assert_eq!(label_of(&s, key), dutch, "{key}");
    }
    assert!(strings::set_locale("en"));
}
