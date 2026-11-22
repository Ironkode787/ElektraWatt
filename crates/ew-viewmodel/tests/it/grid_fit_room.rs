// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A page's grid changed in the window leaves every wire a way (docs/design/06-schematic-editor.md §8 item 3,
//! FT-2010): the window hands `page.set_grid` the page's drawing area, so a card whose pins face its edge keeps a wire's
//! way out for each of them, and two pins wired straight that stood in line keep a column for the wire between them.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::ew_api::Session;
use ew_viewmodel::shell::Mode;
use serde_json::json;

/// The sample's 104 (-104A1's cards, their supply pins facing the drawing area's left edge, L+ and M of the output card
/// both turning down along it) and 201 (terminals wired straight down from -201K1's pins, 5.29 mm apart) changed to the
/// 4 mm grid: Change and fit routes every wire anew, and on 104 none runs through a text (with one grid line for the
/// output card's two supply wires, L+ went over the card and through its tag).
#[test]
fn the_sample_pages_changed_to_4_mm_route_every_wire() {
    let tmp = Tmp::new("grid-fit-room");
    let mut shell = tmp.sample_in(Mode::Headless, true, "104");
    for folio in ["104", "201"] {
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        done(&mut shell, "select.set", json!({ "ids": [] }));
        done(&mut shell, "page.fit_grid", json!({"grid": "mm4"}));
        let page = shell.canvas().expect("a page on show").page();
        assert_eq!(page.snapper.grid(), (4_000_000, 1));
        assert!(!page.tables.connections.is_empty());
        let unrouted: Vec<String> = page
            .tables
            .connections
            .iter()
            .filter(|c| !c.stored_route)
            .map(|c| c.id.to_string())
            .collect();
        assert_eq!(unrouted, Vec::<String>::new(), "folio {folio}: every wire routed");
    }
    done(&mut shell, "file.save", json!({}));
    shell.shutdown();
    let (session, _) = Session::open_read_only(&tmp.sample_copy(), "test").expect("the saved copy");
    let layout = session.layout_report(Some("104")).expect("104's layout");
    let through: Vec<&str> = layout[0]
        .through_texts
        .iter()
        .map(|t| t.connection.as_str())
        .collect();
    assert_eq!(through, Vec::<&str>::new(), "no wire through a text on 104");
}
