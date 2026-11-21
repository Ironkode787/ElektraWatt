// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! PLC work through the shell on a new project with page 101, from persona runs: a click on what is selected already
//! brings Properties back to it, the PLC tab's Bus panel brings an open panel to the front, and a terminal on a traced
//! channel's path prints the channel's address with Project ▸ Channel addresses at terminals. Not here: the station
//! view, the PLC workspace and their buttons in the window (`ew-ui`'s `plc_rack_first.rs`, `plc_workspace.rs`).

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

/// The starter part `number` drawn as a black box from grid cell `a` to grid cell `b` of the page shown, then the
/// Select tool.
fn black_box(s: &mut ShellModel, number: &str, a: (i64, i64), b: (i64, i64)) {
    done(s, "tool.black_box", json!({"part": number, "template": 0}));
    for k in [a, b] {
        cursor_k(s, k);
        done(s, "canvas.activate", json!({}));
    }
    done(s, "tool.select", json!({}));
    // A station placed on a page opens its view in a tab; Escape goes back to the page.
    if s.station_tab().is_some() {
        done(s, "ui.dismiss", json!({}));
    }
}

/// A persona run: Properties stayed on the project while the switch was selected, until he selected something else
/// first. A click on what is selected already brings Properties back to it.
#[test]
fn a_click_on_the_selection_brings_its_properties_back() {
    let tmp = Tmp::new("rack-first-click");
    let mut s = new_project(&tmp);
    black_box(&mut s, "EW-GEN-ESW-8", (10, 10), (16, 18));
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

/// FT-1734: the shipped terminal symbol has a `PlcAddress` slot. In a new project the digital output card's pin Q0.0
/// is wired through terminal -X1:1 to a coil; with Project ▸ Channel addresses at terminals on, the terminal prints Q0.0,
/// and without it nothing.
#[test]
fn a_terminal_on_a_traced_path_prints_the_channel_address() {
    use ew_viewmodel::ew_api::output::TableCp;
    let tmp = Tmp::new("plc-traced");
    let mut s = new_project(&tmp);
    black_box(&mut s, "EW-GEN-CPU-1", (10, 10), (16, 18));
    black_box(&mut s, "EW-GEN-DO-1", (24, 10), (30, 18));
    done(&mut s, "strip.new", json!({"tag": "-X1"}));
    for (pick, at) in [
        (
            json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1", "designation": "1"}),
            (40, 26),
        ),
        (json!({"symbol": "ew.iec.coil@qet10", "letter": "K"}), (40, 36)),
    ] {
        done(&mut s, "tool.place", json!({}));
        done(&mut s, "place.pick", pick);
        cursor_k(&mut s, at);
        done(&mut s, "canvas.activate", json!({"alt": true}));
        done(&mut s, "ui.dismiss", json!({}));
        done(&mut s, "ui.dismiss", json!({}));
    }
    let (page, card_pin, term, coil) = {
        let t = std::sync::Arc::clone(&s.canvas().unwrap().page().tables);
        let q00 = |p: &TableCp| p.plc_address.as_deref() == Some("Q0.0");
        let card = t
            .reps
            .iter()
            .find(|r| r.black_box.is_some() && t.cps.iter().any(|p| p.rep == r.id && q00(p)))
            .expect("the card");
        let pin = t
            .cps
            .iter()
            .find(|p| p.rep == card.id && q00(p))
            .expect("pin Q0.0")
            .clone();
        let of = |key: &str| {
            let r = t.reps.iter().find(|r| r.symbol_key.starts_with(key)).expect(key);
            let mut cps: Vec<_> = t.cps.iter().filter(|p| p.rep == r.id).cloned().collect();
            cps.sort_by_key(|p| p.idx);
            cps
        };
        (t.page, pin, of("ew.iec.terminal"), of("ew.iec.coil"))
    };
    let cp = |p: &TableCp| json!({"cp": {"function": p.function.to_string(), "idx": p.idx}});
    for (a, b) in [(&card_pin, &term[1]), (&term[0], &coil[0])] {
        done(
            &mut s,
            "command.run",
            json!({"cmd": "connection.draw", "params": {"page": page.to_string(), "a": cp(a), "b": cp(b)}}),
        );
    }
    // The texts "Q0.0" drawn within 10 mm of the terminal's insertion point.
    let at_terminal = |s: &ShellModel| {
        use ew_dl::{Item, ItemKind};
        fn runs(items: &[Item], out: &mut Vec<(String, (i64, i64))>) {
            for item in items {
                match &item.kind {
                    ItemKind::Glyphs(run) => out.push((run.text.clone(), (run.origin.x.0, run.origin.y.0))),
                    ItemKind::Group(g) => runs(&g.items, out),
                    _ => {}
                }
            }
        }
        let page = s.canvas().unwrap().page();
        let rep = page
            .tables
            .reps
            .iter()
            .find(|r| r.symbol_key.starts_with("ew.iec.terminal"))
            .expect("the terminal");
        let at = page.snapper.page_to_sheet(rep.pos);
        let mut all = Vec::new();
        runs(&page.list.items, &mut all);
        all.into_iter()
            .filter(|(t, o)| {
                t == "Q0.0" && (o.0 - at.0).abs() < 10_000_000 && (o.1 - at.1).abs() < 10_000_000
            })
            .count()
    };
    assert_eq!(at_terminal(&s), 0, "off by default");
    done(
        &mut s,
        "project.plc_addresses_at_terminals",
        json!({"on": true}),
    );
    assert_eq!(at_terminal(&s), 1, "the terminal prints the channel's address");
    s.shutdown();
}
