// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The layout's scale and fitting the plate where a person looks: Fit the layout to the sheet is a
//! button of the Draw tab's layout group on a layout page, and a scale at which the layout runs over the frame says
//! so in a toast whose action fits it. The sample's panel layout 902 (1:5).

use crate::common;

use common::{Tmp, done, toast};
use ew_viewmodel::command_bar::command_bar;
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn fit_is_on_the_draw_tab_and_a_scale_that_runs_over_offers_it() {
    let tmp = Tmp::new("layout-scale-fit");
    let mut s = tmp.sample_in(Mode::Headless, true, "902");
    done(&mut s, "view.command_tab", json!({"tab": "draw"}));
    assert!(
        bar_offers(&s, "layout.fit_sheet"),
        "the Draw tab offers Fit the layout to the sheet"
    );
    done(&mut s, "props.show", json!({"on": true}));
    done(&mut s, "props.set", json!({"row": "page/scale", "value": "1:2"}));
    let said = toast(&s);
    assert!(
        said.starts_with(
            "At 1:2 the layout runs over the frame's drawing area. Fit the layout to the sheet draws it at 1:"
        ),
        "{said}"
    );
    // The toast's action fits the layout: the page draws at the fitting scale again and nothing runs over.
    let entry = s.notifications().entries().next().expect("the toast").serial;
    done(
        &mut s,
        "notifications.run",
        json!({"entry": entry, "action": "fit_sheet"}),
    );
    let fitted = said
        .rsplit("1:")
        .next()
        .and_then(|t| t.split(',').next())
        .unwrap_or_default()
        .to_owned();
    let sheet = s.properties().sheet().expect("the page's sheet");
    let scale = match &ew_viewmodel::properties::find_row(sheet, "page/scale")
        .expect("the Scale row")
        .1
        .value
    {
        ew_viewmodel::properties::RowValue::One { text, .. } => text.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        scale,
        format!("1:{fitted}"),
        "the page draws at the scale the toast named"
    );
}

/// Whether the bar on show offers `id`, as a button or in a split button's variants (Enclosure's).
fn bar_offers(s: &ew_viewmodel::shell::ShellModel, id: &str) -> bool {
    command_bar(s).groups.iter().flat_map(|g| &g.buttons).any(|b| {
        b.id == id
            || matches!(&b.kind, ew_viewmodel::command_bar::ButtonKind::Split(v) if v.iter().any(|i| i.id == id))
    })
}
