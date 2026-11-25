// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Move the legend to its own page (`report_block.legend_to_page`, doc 12 §5.4): a legend box beside a layout, as a
//! project made before 0.3.5 holds one, becomes the layout's legend page in one undo step, and the box goes; the
//! legend's menu offers it, and a page without a box refuses it in words.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::key;
use ew_viewmodel::shell::Outcome;
use serde_json::json;

#[test]
fn a_legend_box_moves_to_its_own_page_in_one_undo_step() {
    let tmp = Tmp::new("legend-to-page");
    let (mut shell, legend) = common::legend_box(&tmp);
    // The box's menu offers the move.
    done(&mut shell, "select.set", json!({"ids": [legend]}));
    let menu = canvas_menu(&shell, MenuSpot::Object, None);
    assert!(
        menu.iter()
            .any(|i| i.id == "report_block.legend_to_page" && i.enabled),
        "{menu:?}"
    );
    done(&mut shell, "report_block.legend_to_page", json!({}));
    let t = shell.panel_tables().expect("980's layout");
    assert!(t.blocks.is_empty(), "the box is gone: {:?}", t.blocks);
    let page = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "980.1")
        .cloned()
        .expect("the legend page 980.1");
    assert_eq!(page.title, "Legend of layout 980");
    assert!(
        common::toast(&shell).contains("980.1"),
        "{}",
        common::toast(&shell)
    );
    // Its legend is a page now: no second legend, and nothing left to move.
    assert_eq!(
        refused(&mut shell, "page.add_legend", json!({})),
        "reason-legend-paged"
    );
    assert_eq!(
        refused(&mut shell, "report_block.legend_to_page", json!({})),
        "reason-legend-no-box"
    );
    // One undo brings the box back and takes the page away.
    done(&mut shell, "edit.undo", json!({}));
    assert!(shell.page_rows().iter().all(|r| r.folio != "980.1"));
    let back = shell.panel_tables().expect("980's layout");
    assert_eq!(
        back.blocks.iter().map(|b| b.id.to_string()).collect::<Vec<_>>(),
        [legend]
    );
    shell.shutdown();
}

/// Fit the layout to the sheet waits for the legend box (doc 12 §5.4: "once its legend has gone"): while the box
/// stands beside the layout, the plate would take its room, so the fit is refused in words; with the legend on its
/// own page it is no longer refused for the box.
#[test]
fn fit_the_layout_waits_for_the_legend_box() {
    let tmp = Tmp::new("legend-box-fit");
    let (mut shell, _) = common::legend_box(&tmp);
    assert_eq!(
        refused(&mut shell, "layout.fit_sheet", json!({})),
        "reason-layout-fit-legend"
    );
    done(&mut shell, "report_block.legend_to_page", json!({}));
    done(&mut shell, "page.goto", json!({"folio": "980"}));
    assert_ne!(
        shell.dispatch("layout.fit_sheet", &json!({})),
        Outcome::refused(key!("reason-layout-fit-legend"))
    );
    shell.shutdown();
}
