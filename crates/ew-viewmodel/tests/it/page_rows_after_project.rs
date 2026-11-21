// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! After `props.project` the page's own rows can be reached again (FT-1868): `select.set` with no IDs, as a click on
//! the empty page, and another page shown give Properties the page's sheet; before, it stayed on the project's until
//! something was selected.

use crate::common::{Tmp, done, state};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The kinds of the sheets Properties shows.
fn kinds(s: &ShellModel) -> Vec<String> {
    state(s)["properties"]["sections"]
        .as_array()
        .map(|a| a.iter().map(|x| x["kind"].as_str().unwrap_or_default().to_owned()).collect())
        .unwrap_or_default()
}

#[test]
fn the_page_rows_come_back_after_the_project_sheet() {
    let tmp = Tmp::new("page-rows-after-project");
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "props.project", json!({}));
    assert_eq!(kinds(&s), ["project"]);
    done(&mut s, "select.set", json!({"ids": []}));
    assert_eq!(kinds(&s), ["page"], "an empty selection set shows the page's rows");

    done(&mut s, "props.project", json!({}));
    assert_eq!(kinds(&s), ["project"]);
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert_eq!(kinds(&s), ["page"], "another page shows its own rows");
}
