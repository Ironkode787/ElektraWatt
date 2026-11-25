// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A tap on a mark's own cross-reference follows it while the mark is selected: the sample's page 103, its
//! incoming PE mark selected, a click on its cross-reference text to page 101 shows page 101, as it does with nothing
//! selected.

use crate::common::{Tmp, click, done, sheet};
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn a_click_on_the_selected_marks_cross_reference_follows_it() {
    let tmp = Tmp::new("selected-mark-link");
    let mut s = tmp.sample_in(Mode::Headless, true, "103");
    let mark = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.ip.as_ref().is_some_and(|ip| ip.name == "PE"))
        .cloned()
        .expect("103's PE mark");
    let centre = sheet(
        &s,
        (
            (mark.body[0] + mark.body[2]) / 2,
            (mark.body[1] + mark.body[3]) / 2,
        ),
    );
    // The mark's own cross-reference: the link to 101 nearest to it.
    let link = s
        .canvas()
        .unwrap()
        .page()
        .links
        .iter()
        .filter(|l| l.target.as_ref().is_some_and(|t| t.folio == "101"))
        .map(|l| {
            let c = (
                (l.rect.min().x.0 + l.rect.max().x.0) / 2,
                (l.rect.min().y.0 + l.rect.max().y.0) / 2,
            );
            ((c.0 - centre.0).abs() + (c.1 - centre.1).abs(), c)
        })
        .min()
        .map(|(_, c)| c)
        .expect("a link to 101");
    done(&mut s, "select.set", json!({"ids": [mark.id.to_string()]}));
    click(&mut s, link, json!({}));
    assert_eq!(s.current_folio(), Some("101"));
}
