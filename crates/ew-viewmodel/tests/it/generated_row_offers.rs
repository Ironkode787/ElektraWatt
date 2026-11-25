// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A list page's rows say where they are changed:
//! an edit is refused under the row with a button that runs the way there: the
//! printed columns List columns… on the Output tab, the rows of one's own Rows of your own…, any other row the list's
//! report block. The sample's parts list 901.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Dialog, Mode, Outcome};
use serde_json::json;

#[test]
fn a_list_pages_rows_offer_where_they_are_changed() {
    let tmp = Tmp::new("generated-row-offers");
    let mut s = tmp.sample_in(Mode::Gui, true, "901");
    done(&mut s, "props.show", json!({"on": true}));
    for (row, label, action) in [
        (
            "report_block/columns",
            "List columns… on the Output tab",
            "report_block.columns",
        ),
        (
            "report_block/added_rows",
            "Rows of your own… on the Output tab",
            "report_block.rows",
        ),
        ("report_block/title", "Show report block", "select.set"),
    ] {
        assert!(
            matches!(
                s.dispatch("props.edit", &json!({"row": row})),
                Outcome::Failed { .. }
            ),
            "{row}"
        );
        assert_eq!(
            s.properties().error().map(|(r, _)| r),
            Some(row),
            "the reason stands under {row}"
        );
        let offer = s.row_offer(row).unwrap_or_else(|| panic!("{row}: an offer"));
        assert_eq!(offer.label, label, "{row}");
        assert_eq!(offer.steps[0].0, action, "{row}");
    }
    // List columns… opens its form on the list on show.
    let offer = s.row_offer("report_block/columns").unwrap();
    for (action, params) in offer.steps {
        done(&mut s, action, params);
    }
    assert!(
        matches!(s.dialog(), Some(Dialog::Form(f)) if f.action == "report_block.columns"),
        "{:?}",
        s.dialog()
    );
}
