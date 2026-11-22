// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Copy pages… renames the copies' titles and descriptions too (a persona run's copies kept "Inst 1 - …"): the form's
//! rename table opens with one empty row, a row added is empty too, the card lists the titles that change, and the
//! copy carries them.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

fn title_of(s: &ShellModel, folio: &str) -> Option<String> {
    s.page_rows()
        .iter()
        .find(|r| r.folio == folio)
        .map(|r| r.title.clone())
}

#[test]
fn the_rename_reaches_the_titles_and_the_card_lists_them() {
    let tmp = Tmp::new("copy-titles");
    let mut s = ShellModel::new(
        Mode::Gui,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("job.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "11", "type": "multi_line", "title": "Inst 1 - Control"}),
    );
    done(&mut s, "pages.copy", json!({}));
    let Some(Dialog::Form(f)) = s.dialog() else {
        panic!("the Copy pages form: {:?}", s.dialog());
    };
    let rows: Vec<&str> = f.table.as_ref().expect("the rename table").rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(rows, [""], "one empty row to fill");
    s.add_form_table_row();
    let Some(Dialog::Form(f)) = s.dialog() else { panic!() };
    assert_eq!(f.table.as_ref().unwrap().rows[1].key, "", "an added row is empty too");
    s.remove_form_table_row(1);
    s.set_form_table_cell(0, None, "Inst 1");
    s.set_form_table_cell(0, Some(0), "Inst 2");
    s.set_form_field("offset", "10");
    assert_eq!(s.submit_form(), Outcome::Done);
    let notices = s.preview_pane().notices().join("\n");
    assert!(
        notices.contains("Page titles that change: Inst 1 - Control → Inst 2 - Control"),
        "{notices}"
    );
    done(&mut s, "preview.apply", json!({}));
    assert_eq!(title_of(&s, "21").as_deref(), Some("Inst 2 - Control"));
    assert_eq!(title_of(&s, "11").as_deref(), Some("Inst 1 - Control"), "the original keeps its own");
    s.shutdown();
}
