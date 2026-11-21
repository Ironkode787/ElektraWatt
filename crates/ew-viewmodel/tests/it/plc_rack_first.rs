// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Two persona runs of the rack-first PLC work, through the shell on a new project with page 101: a click on what is
//! selected already brings Properties back to it, and the PLC tab's Bus panel brings an open panel to the front. Not
//! here: the station view, its catalogue and its buttons in the window (`ew-ui`'s `plc_rack_first.rs`).

use crate::common::{Tmp, cursor, done, grid_point};

use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::json;

/// A new project with page 101, the PLC panel shown.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = ShellModel::new(Mode::Gui, ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"));
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("rack-first.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Control"}),
    );
    done(&mut s, "rail.show", json!({"pane": "plc"}));
    s
}

/// The grid cursor to grid cell `k` of the page shown.
fn cursor_k(s: &mut ShellModel, k: (i64, i64)) {
    let p = grid_point(s, k);
    cursor(s, p);
}

/// A persona run: Properties stayed on the project while the switch was selected, until he selected something else
/// first. A click on what is selected already brings Properties back to it.
#[test]
fn a_click_on_the_selection_brings_its_properties_back() {
    let tmp = Tmp::new("rack-first-click");
    let mut s = new_project(&tmp);
    // The starter switch drawn as a black box from grid cell (10, 10), six by eight cells.
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "EW-GEN-ESW-8", "template": 0}),
    );
    cursor_k(&mut s, (10, 10));
    done(&mut s, "canvas.activate", json!({}));
    cursor_k(&mut s, (16, 18));
    done(&mut s, "canvas.activate", json!({}));
    done(&mut s, "tool.select", json!({}));
    // A station placed on a page opens its view in a tab; Escape goes back to the page.
    if s.station_tab().is_some() {
        done(&mut s, "ui.dismiss", json!({}));
    }
    done(&mut s, "props.show", json!({"on": true}));
    let click = |s: &mut ShellModel| {
        cursor_k(s, (13, 14));
        done(s, "canvas.activate", json!({}));
    };
    let kinds = |s: &ShellModel| -> Vec<String> {
        s.properties()
            .sheet()
            .map(|x| x.sections.iter().map(|x| x.kind.clone()).collect())
            .unwrap_or_default()
    };
    click(&mut s);
    assert!(kinds(&s).iter().any(|k| k == "device"), "{:?}", kinds(&s));
    done(&mut s, "props.project", json!({}));
    assert!(!kinds(&s).iter().any(|k| k == "device"), "the project's sheet");
    click(&mut s);
    assert!(kinds(&s).iter().any(|k| k == "device"), "{:?}", kinds(&s));
    s.shutdown();
}

/// A small one of a persona run: the PLC tab's Bus panel closed the panel that stood open behind another tab.
/// Bus panel brings the panel to the front and the status bar says so.
#[test]
fn the_bus_panel_comes_to_the_front() {
    use ew_viewmodel::layout::PaneKind;
    use ew_viewmodel::navigators::NavKind;
    let tmp = Tmp::new("rack-first-bus");
    let mut s = new_project(&tmp);
    let bus = PaneKind::Navigator(NavKind::Bus);
    done(&mut s, "bus.panel", json!({}));
    done(&mut s, "rail.show", json!({"pane": "plc"}));
    assert_ne!(
        s.layout().shown_in_place_of(bus),
        Some(bus),
        "behind the PLC panel"
    );
    done(&mut s, "bus.panel", json!({}));
    assert_eq!(
        s.layout().shown_in_place_of(bus),
        Some(bus),
        "in front, not closed"
    );
    assert_eq!(
        s.status_note(),
        Some("The Bus panel was open already: it is in front now.")
    );
    s.shutdown();
}
