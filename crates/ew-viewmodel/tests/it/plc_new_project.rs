// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! PLC work through the shell on a new project with page 101: a click on what is selected already
//! brings Properties back to it, the PLC tab's Bus panel brings an open panel to the front, a terminal on a traced
//! channel's path prints the channel's address with Project ▸ Channel addresses at terminals, Fix all marks a CPU's
//! unconnected pins not used in one step, and the PLC panel's Place here (on a channel's row, and ended by a Delete of
//! its part). Not here: the station view, its To do, the PLC
//! workspace and their buttons in the window (`ew-ui`'s `plc_rack_first.rs`, `station_todo.rs`, `plc_workspace.rs`).

use crate::common::{Tmp, cursor, done, grid_point};

use ew_viewmodel::command_bar::navigator_row_menu;
use ew_viewmodel::ew_api::output::{NavNode, Uuid};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// A new project with page 101, the PLC panel shown.
fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.empty_shell(Mode::Gui);
    crate::common::make_project(
        &mut s,
        &tmp.0.join("rack-first.ew"),
        &[("101", "multi_line", "Control")],
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

/// Properties must not stay on the project while the switch is selected. A click on what is selected already brings
/// Properties back to it.
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

/// The PLC tab's Bus panel must not close the panel that stands open behind another tab. Bus panel brings the panel
/// to the front and the status bar says so.
#[test]
fn the_bus_panel_comes_to_the_front() {
    use ew_viewmodel::layout::PaneKind;
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
    done(&mut s, "project.plc_addresses_at_terminals", json!({"on": true}));
    assert_eq!(at_terminal(&s), 1, "the terminal prints the channel's address");
    s.shutdown();
}

/// Twenty-nine "pin is not connected" warnings, each with its own Fix and Apply. A card with
/// eight unconnected pins: its group's Fix all (n)… makes one card, one Apply marks them all not used (the toast says
/// how many, with Undo), the warnings are gone, and one Undo brings them all back.
#[test]
fn fix_all_marks_every_unconnected_pin_in_one_step() {
    let tmp = Tmp::new("plc-fix-all");
    let mut s = new_project(&tmp);
    black_box(&mut s, "EW-GEN-CPU-2", (10, 10), (16, 18));
    let pins = |s: &mut ShellModel| -> (usize, Option<String>) {
        done(s, "messages.show", json!({"on": true}));
        done(s, "messages.check", json!({}));
        s.wait_jobs();
        let nodes = s
            .nav(NavKind::Messages)
            .and_then(|n| n.table())
            .map(|t| t.nodes.clone())
            .unwrap_or_default();
        let count = nodes
            .iter()
            .filter(|n| {
                n.role == NavRole::Finding
                    && n.cells.first().is_some_and(|c| c == "warning")
                    && n.cells.get(1).is_some_and(|c| c == "EW-POT-009")
            })
            .count();
        let group = nodes
            .iter()
            .find(|n| n.role == NavRole::Rule && n.key == "severity:warning/rule:EW-POT-009")
            .map(|n| n.key.clone());
        (count, group)
    };
    let (before, group) = pins(&mut s);
    assert!(before >= 2, "the unconnected pins: {before}");
    let group = group.expect("the group of unconnected pins");
    let offer = s.fix_all_offer(&group);
    assert_eq!(offer.as_ref().map(|o| o.1), Some(before), "Fix all ({before})…");
    done(&mut s, "messages.fix_all", json!({"key": group}));
    done(&mut s, "preview.apply", json!({}));
    let toast = s.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(toast, format!("{before} pins marked not used."));
    assert_eq!(pins(&mut s).0, 0, "the warnings are gone");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(pins(&mut s).0, before, "one undo brings them all back");
    s.shutdown();
}

/// A station of kind `kind` with head part `head` and the modules `parts` made in its view: the station.
fn station(s: &mut ShellModel, kind: &str, head: &str, parts: &[&str]) -> Uuid {
    let _ = s.dispatch("plc.new_station", &json!({"kind": kind}));
    done(s, "plc.rack_set", json!({"head": head}));
    done(s, "plc.rack_create", json!({}));
    for part in parts {
        done(s, "plc.rack_add", json!({"part": part}));
    }
    s.station_tab()
        .and_then(|t| t.station)
        .expect("the station's view")
}

/// The first PLC panel row of role `role` whose records satisfy `of`.
fn plc_node(s: &mut ShellModel, role: NavRole, of: impl Fn(&[Uuid]) -> bool) -> NavNode {
    done(s, "rail.show", json!({"pane": "plc"}));
    let _ = s.shown_rows(NavKind::Plc);
    s.nav(NavKind::Plc)
        .and_then(|n| n.table())
        .and_then(|t| t.nodes.iter().find(|n| n.role == role && of(&n.records)).cloned())
        .unwrap_or_else(|| panic!("a {role:?} row"))
}

/// A Delete in a navigator of the part a box hangs on the pointer for (Place here) ends the placement, as an Undo that
/// takes it away does: the click that follows draws nothing and names nothing gone.
#[test]
fn a_delete_in_a_navigator_ends_the_box_of_what_it_deleted() {
    let tmp = Tmp::new("plc-place-delete");
    let mut s = new_project(&tmp);
    let cpu = station(&mut s, "cpu", "EW-GEN-CPU-2", &[]);
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let node = plc_node(&mut s, NavRole::Station, |r| r.first() == Some(&cpu));
    let place = navigator_row_menu(&s, NavKind::Plc, &node)
        .into_iter()
        .find(|i| i.label == "Place here")
        .unwrap_or_else(|| panic!("Place here on {}", node.label));
    done(&mut s, place.id, place.params.clone());
    assert_eq!(s.tool().name(), "black_box", "its box on the pointer");
    let kind = NavKind::Plc.word();
    done(&mut s, "navigator.row", json!({"kind": kind, "key": node.key}));
    let revision = s.revision();
    let _ = s.dispatch("navigator.delete", &json!({"kind": kind}));
    if s.revision() == revision {
        done(&mut s, "preview.apply", json!({}));
    }
    assert_ne!(s.revision(), revision, "the station is deleted");
    assert_eq!(s.tool().name(), "select", "the placement ends with it");
    s.shutdown();
}

/// A channel of a card no page draws offers its card's Place here, and
/// its row says "not drawn"; the entry puts the card's box on the pointer.
#[test]
fn a_channel_of_an_undrawn_card_offers_place_here() {
    let tmp = Tmp::new("plc-place-channel");
    let mut s = new_project(&tmp);
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let cpu = station(&mut s, "cpu", "EW-GEN-CPU-2", &["EW-GEN-DI-1"]);
    let card = s.rack_view(cpu).expect("the rack").rows[0]
        .module
        .expect("the card");
    done(&mut s, "ui.dismiss", json!({}));
    let node = plc_node(&mut s, NavRole::Channel, |r| r.get(1) == Some(&card));
    let place = navigator_row_menu(&s, NavKind::Plc, &node)
        .into_iter()
        .find(|i| i.id == "plc.module_place")
        .expect("Place here on the channel's row");
    assert!(place.enabled, "{:?}", place.reason);
    assert_eq!(place.params["device"], json!(card.to_string()), "its card's box");
    assert!(node.badges.iter().any(|b| b == "unplaced"), "{:?}", node.badges);
    done(&mut s, place.id, place.params.clone());
    assert_eq!(s.tool().name(), "black_box", "its card's box on the pointer");
    s.shutdown();
}
