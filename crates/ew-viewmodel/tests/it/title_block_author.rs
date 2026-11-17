// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Title block fields form never applies half (rc.2 persona run, entry 3): in a session that may not name a
//! revision's author (a script; `revision.set_author` is a person's step at the window), the form leaves Author out
//! and says why under its fields before Apply; Apply then applies everything the form holds. A project made as the
//! persona made it: from the company standard, page 101, its open revision naming no author.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, ShellConfig, ShellModel};
use serde_json::json;

#[test]
fn a_field_the_session_may_not_write_is_said_before_apply() {
    let tmp = Tmp::new("titleblock-author");
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut s, "tools.company_standard", json!({}));
    let company = tmp.0.join("company.ew");
    done(
        &mut s,
        "wizard.finish",
        json!({ "dir": company.to_string_lossy() }),
    );
    let _ = s.dispatch("wizard.cancel", &json!({}));
    let job = tmp.0.join("job.ew");
    done(&mut s, "file.new", json!({ "dir": job.to_string_lossy() }));
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Supply"}),
    );
    done(&mut s, "page.title_block", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the form: {:?}", s.dialog())
    };
    let names: Vec<&str> = form.fields.iter().map(|f| f.name).collect();
    assert!(!names.contains(&"revision.current.author.name"), "{names:?}");
    let foot = s.form_foot(&form).unwrap_or_default();
    assert!(
        foot.starts_with("Author is not in this form: a person names the revision's author at the window"),
        "{foot}"
    );
    // Apply applies all it holds and closes, with no refusal after it.
    let mut params = serde_json::Map::new();
    for f in &form.fields {
        params.insert(f.name.to_owned(), json!(f.value));
    }
    params.insert("page.title".to_owned(), json!("Control panel"));
    done(&mut s, "page.title_block", serde_json::Value::Object(params));
    assert!(s.dialog().is_none(), "{:?}", s.dialog());
}

/// The designer as a persona run looked for him: Project properties have a plain row per person the title block can
/// print (Designer, Checker, Customer…) instead of a raw list; the designer typed there prints in the new project's
/// Author cell while no revision names its author; and Title block fields… says which cell prints each field.
#[test]
fn the_designer_is_a_plain_row_that_the_author_cell_prints() {
    let tmp = Tmp::new("titleblock-designer");
    let mut s = ShellModel::new(
        Mode::Gui,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(&mut s, "tools.company_standard", json!({}));
    let company = tmp.0.join("company.ew");
    done(
        &mut s,
        "wizard.finish",
        json!({ "dir": company.to_string_lossy() }),
    );
    done(&mut s, "wizard.cancel", json!({}));
    let job = tmp.0.join("job.ew");
    done(&mut s, "file.new", json!({ "dir": job.to_string_lossy(), "template": company.to_string_lossy() }));
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.project", json!({}));
    let labels = |s: &ShellModel| -> Vec<String> {
        let sheet = s.properties().sheet().expect("the project's sheet");
        let section = &sheet.sections[0];
        ew_viewmodel::properties::groups(section, false)
            .into_iter()
            .find(|(g, _)| *g == ew_viewmodel::properties::RowGroup::Main)
            .map(|(_, rows)| {
                rows.iter()
                    .map(|r| ew_viewmodel::properties::row_label(section, r))
                    .collect()
            })
            .unwrap_or_default()
    };
    let main = labels(&s);
    for person in ["Designer", "Checker", "Customer", "End user"] {
        assert!(main.iter().any(|l| l == person), "{person}: {main:?}");
    }
    assert!(!main.iter().any(|l| l == "Parties"), "the raw list waits under More: {main:?}");
    done(
        &mut s,
        "props.set",
        json!({"row": "project/people.author", "value": "A. Designer"}),
    );
    done(&mut s, "page.title_block", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the form: {:?}", s.dialog())
    };
    let author = form
        .fields
        .iter()
        .find(|f| f.name == "revision.current.author.name")
        .expect("Author, for a person at the window");
    assert_eq!(author.value, "A. Designer", "the Author cell prints the designer");
    let note = s.form_note(&form, "revision.current.author.name").unwrap_or_default();
    assert!(note.contains("Prints in the cell \"Author\""), "{note}");
    let note = s.form_note(&form, "project.reference").unwrap_or_default();
    assert!(note.contains("Prints in the cell \"Project\""), "{note}");
}
