// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 40 lane H40 at shell level: Output ▸ Parts list (FT-717).

mod common;

use common::{Tmp, done, refused};
use ew_viewmodel::actions;
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// FT-717: on Golden A, Parts list opens the first page of the parts list (the `parts_summary` block at folio 901)
/// in the focused group.
#[test]
fn parts_list_opens_the_first_page_of_the_bom_block() {
    let tmp = Tmp::new("h40-parts");
    let mut shell = tmp.shell("102");
    let meta = actions::find("lists.parts").expect("registered");
    assert_eq!(strings::text(meta.label_key), "Parts list");
    assert_eq!((meta.enabled)(&shell), Ok(()));
    let folio = shell.parts_list_folio().expect("Golden A has a parts list");
    assert_eq!(folio, "901");
    done(&mut shell, "lists.parts", json!({}));
    let row = shell
        .current_page()
        .and_then(|p| shell.row_index(p))
        .map(|i| shell.page_rows()[i].clone())
        .expect("a page is shown");
    assert_eq!(row.folio, "901");
    assert!(row.generated, "the parts list is a generated report page");
}

/// FT-717: a project without a parts list report block refuses Parts list with its reason in words.
#[test]
fn parts_list_without_a_bom_block_is_refused_in_words() {
    let tmp = Tmp::new("h40-noparts");
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let meta = actions::find("lists.parts").expect("registered");
    assert_eq!((meta.enabled)(&shell), Err("reason-no-project"));
    done(
        &mut shell,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut shell,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    assert_eq!(shell.parts_list_folio(), None);
    assert_eq!((meta.enabled)(&shell), Err("reason-no-parts-list"));
    assert_eq!(
        strings::text("reason-no-parts-list"),
        "The project has no parts list: it needs a parts list report block"
    );
    assert_eq!(refused(&mut shell, "lists.parts", json!({})), "reason-no-parts-list");
}
