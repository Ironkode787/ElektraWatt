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
    // `#` finds among the pages what the plain search finds: one matcher.
    let pages = |entries: Vec<ew_viewmodel::actions::PaletteEntry>| -> Vec<PaletteRun> {
        entries
            .into_iter()
            .filter(|e| matches!(e.run, PaletteRun::Page(_)))
            .map(|e| e.run)
            .collect()
    };
    let plain = pages(entries);
    s.set_palette_query("#panel");
    let mut hashed = pages(s.palette_entries());
    let mut plain_sorted = plain.clone();
    plain_sorted.sort_by_key(|r| format!("{r:?}"));
    hashed.sort_by_key(|r| format!("{r:?}"));
    assert_eq!(hashed, plain_sorted);

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

/// A command's name still finds the command first, though pages bear that name as their title (the search keeps every
/// hit, so how many pages there are does not move a command down); a page's kind is found by its Dutch words in a
/// Dutch window, and says so.
#[test]
fn a_commands_name_finds_the_command_before_pages_of_that_title() {
    let tmp = Tmp::new("search-many-pages");
    let mut s = tmp.sample_in(Mode::Gui, true, "101");
    let names = ["Route", "Supply", "Messages"];
    for i in 0..9 {
        done(
            &mut s,
            "page.new",
            json!({"folio": (3000 + i).to_string(), "type": "multi_line", "title": names[i % names.len()]}),
        );
    }
    done(&mut s, "tools.palette", json!({}));
    for name in names {
        s.set_palette_query(name);
        let entries = s.palette_entries();
        let first = entries.first().expect("a hit");
        assert!(
            matches!(first.run, PaletteRun::Action(_)),
            "{name}: {:?} {}",
            first.run,
            first.label
        );
        assert!(
            entries.iter().any(|e| matches!(e.run, PaletteRun::Page(_))),
            "{name}: its pages are found too"
        );
    }

    done(&mut s, "view.language", json!({ "value": "nl" }));
    s.set_palette_query("paneellayout");
    let entries = s.palette_entries();
    let layout = entries
        .iter()
        .find(|e| e.run == PaletteRun::Page("902".to_owned()))
        .expect("the panel layout's page by its Dutch kind");
    assert_eq!(layout.breadcrumb, "Pagina · Paneellayout");
    s.shutdown();
}
