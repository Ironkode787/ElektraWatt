// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 40 lane H40 at shell level: Output ▸ Parts list (FT-717).

use crate::common;
use ew_viewmodel::key;
use ew_viewmodel::strings::Key;

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
    assert_eq!(strings::text(Key::run_time(&meta.label_key())), "Open parts list");
    assert_eq!(strings::text(key!("bar-lists-parts")), "Parts list");
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
    assert_eq!((meta.enabled)(&shell), Err(key!("reason-no-project")));
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
    assert_eq!((meta.enabled)(&shell), Err(key!("reason-no-parts-list")));
    assert_eq!(
        strings::text(key!("reason-no-parts-list")),
        "The project has no parts list: it needs a parts list report block"
    );
    assert_eq!(
        refused(&mut shell, "lists.parts", json!({})),
        "reason-no-parts-list"
    );
}

/// "the command ends with the BOM's first page fitted in the canvas and its row selected in the page
/// tree", and "dismiss the earlier export toast". After a PDF export on 102, zoomed in, Parts list shows 901 at the
/// fit zoom, marks its row (the row cursor on it) and leaves no export toast on screen.
#[test]
fn parts_list_fits_the_page_selects_its_row_and_dismisses_the_export_toast() {
    let tmp = Tmp::new("h40-ft1028");
    let mut shell = tmp.shell("102");
    done(&mut shell, "view.zoom_in", json!({}));
    let path = tmp.0.join("set.pdf");
    done(
        &mut shell,
        "file.export_pdf",
        json!({"path": path.to_string_lossy()}),
    );
    let toasts =
        |s: &ShellModel| -> Vec<&'static str> { s.notifications().visible().iter().map(|n| n.key).collect() };
    assert_eq!(toasts(&shell), ["export"]);
    done(&mut shell, "lists.parts", json!({}));
    assert_eq!(shell.current_folio(), Some("901"));
    let c = shell.canvas().expect("a canvas");
    assert_eq!(c.zoom(), c.fit_zoom(), "fitted");
    let rows = shell.page_rows();
    let marked: Vec<String> = shell
        .navigator_rows()
        .iter()
        .filter(|r| shell.navigator().is_marked(&r.key))
        .filter_map(|r| match r.kind {
            ew_viewmodel::navigator::RowKind::Page(i) => Some(rows[i].folio.clone()),
            ew_viewmodel::navigator::RowKind::Node { .. } => None,
        })
        .collect();
    assert_eq!(marked, ["901"]);
    assert_eq!(toasts(&shell), Vec::<&str>::new());
}
