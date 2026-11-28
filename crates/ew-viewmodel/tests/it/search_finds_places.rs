// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The command search finds a place beside the commands: a page by its kind ("panel": the panel layouts, each said to
//! be a page of that kind) and a row of Project properties by its words ("rail fixing"), which Enter shows with its
//! rows under it; the Properties filter never shows a group's head without its rows.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::actions::PaletteRun;
use ew_viewmodel::shell::Mode;
use serde_json::json;

#[test]
fn the_search_finds_a_page_by_its_kind_and_a_setting_by_its_words() {
    let tmp = Tmp::new("search-places");
    let mut s = tmp.sample_in(Mode::Gui, true, "101");
    done(&mut s, "tools.palette", json!({}));
    s.set_palette_query("panel");
    let entries = s.palette_entries();
    let layout = entries
        .iter()
        .find(|e| e.run == PaletteRun::Page("902".to_owned()))
        .expect("the panel layout's page");
    assert_eq!(layout.breadcrumb, "Page · Panel layout");

    s.set_palette_query("rail fixing");
    let entries = s.palette_entries();
    let first = entries.first().expect("a hit");
    assert_eq!(first.label, "Rail fixing");
    assert_eq!(first.breadcrumb, "Project properties · Workshop");
    s.set_palette_query("hole pitch");
    assert!(
        s.palette_entries()
            .iter()
            .any(|e| e.label == "Holes at most … apart" && e.breadcrumb.ends_with("Rail fixing")),
        "the spacing of a rail's holes"
    );

    // Enter shows the project's sheet with the group and its rows.
    s.set_palette_query("rail fixing");
    assert_eq!(s.run_palette_selection(), ew_viewmodel::shell::Outcome::Done);
    assert_eq!(
        s.properties_row().as_deref(),
        Some("project/settings.workshop.rail_fixing")
    );
    done(
        &mut s,
        "props.row",
        json!({"to": "project/settings.workshop.rail_fixing.pitch"}),
    );
    s.shutdown();
}
