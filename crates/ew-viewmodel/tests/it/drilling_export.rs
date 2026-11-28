// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Output ▸ Workshop ▸ Drilling… (doc 13 §9): greyed in words in a project without holes; on the sample its form lists
//! the surfaces with holes, each ticked with the names of its two files, and Export writes the door's template and DXF
//! into the folder chosen, its toast saying which files are which surface's.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, Outcome};
use serde_json::json;

#[test]
fn drilling_is_greyed_without_holes_and_writes_the_door_s_drawings() {
    let tmp = Tmp::new("drilling");
    let mut empty = common::new_project(&tmp);
    assert_eq!(
        common::refused(&mut empty, "drilling.export", json!({})),
        "reason-no-holes"
    );
    empty.shutdown();

    let mut s = tmp.sample_in(Mode::Gui, true, "904");
    done(&mut s, "drilling.export", json!({}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("the form: {:?}", s.dialog())
    };
    let table = form.table.expect("the surfaces");
    assert!(
        table
            .rows
            .iter()
            .any(|r| r.key.starts_with("Door") && r.key.ends_with("+CAB Door template.pdf, +CAB Door.dxf")),
        "{:?}",
        table.rows
    );
    let dir = tmp.path("drill");
    assert!(form.fields.iter().any(|f| f.name == "dir"), "the folder");
    s.set_form_field("dir", &dir.to_string_lossy());
    assert_eq!(s.submit_form(), Outcome::Done);
    // The toast names the surfaces by the words of their rows, each of them where there are several.
    let words: Vec<&str> = table
        .rows
        .iter()
        .filter_map(|r| r.key.split(" · ").next())
        .collect();
    let toast = s.notifications().newest_visible().cloned().expect("a toast");
    assert!(
        toast.text.ends_with(&format!(
            ": a template and a DXF for each of {}.",
            words.join(", ")
        )),
        "{}",
        toast.text
    );
    // The door alone: the toast names it alone.
    let tick = table.columns[0].0.clone();
    let rows: Vec<serde_json::Value> = table
        .rows
        .iter()
        .map(|r| {
            let on = r.key.starts_with("Door · ");
            json!({ "values": { tick.clone(): if on { ew_viewmodel::shell::TICKED } else { "" } } })
        })
        .collect();
    done(
        &mut s,
        "drilling.export",
        json!({ "surfaces": rows, "template": true, "dxf": true, "dir": tmp.path("door").to_string_lossy() }),
    );
    let toast = s.notifications().newest_visible().cloned().expect("a toast");
    assert!(
        toast
            .text
            .ends_with(": a template and a DXF for Door."),
        "{}",
        toast.text
    );
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("the folder is made")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .collect();
    for ext in [".dxf", ".pdf"] {
        assert!(
            names.iter().any(|n| n.contains("Door") && n.ends_with(ext)),
            "{names:?}"
        );
    }
    s.shutdown();
}
