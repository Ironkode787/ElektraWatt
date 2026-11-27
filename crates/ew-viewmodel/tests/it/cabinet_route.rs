// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Route on the sample's panel page 902 through the shell (doc 12 §12.5): on the routed sample nothing is due; a turned
//! strip says its ways no longer hold with the offer Route; Route opens the cabinet's card in the Preview tab with the
//! canvas's ways and the duct's chip, the tick plans every wire again, Apply stores the ways in one undo step with its
//! toast; on a schematic page Route routes the drawing as before.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::command_bar::hud;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

fn cabinet_card(shell: &ShellModel) -> Option<ew_viewmodel::shell::cabinet::CabinetCard> {
    shell.properties().card().and_then(|c| match &c.origin {
        CardOrigin::Cabinet(c) => Some(c.as_ref().clone()),
        _ => None,
    })
}

fn toasts(shell: &ShellModel) -> Vec<String> {
    shell
        .notifications()
        .visible()
        .iter()
        .map(|n| n.text.clone())
        .collect()
}

/// -103F1 moved to +FLD: its wires are made on site, so Route lays nothing, yet their stored ways go with its next run.
/// Route opens the card, whose lead says how many ways it removes, and Apply removes them in one step with its toast;
/// after it nothing is due again.
#[test]
fn route_that_lays_nothing_removes_the_ways_of_wires_the_cabinet_no_longer_lays() {
    let tmp = Tmp::new("vm-cabinet-gone");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    let (session, _) = ew_api::Session::open_read_only(&tmp.sample_copy(), "test").expect("the sample");
    let f1 = session.resolve_reference("tag:-103F1").expect("-103F1");
    let fld = session
        .query()
        .structure_nodes()
        .iter()
        .find(|(_, n)| n.code.as_str() == "FLD")
        .map(|(id, _)| id.as_uuid())
        .expect("+FLD");
    drop(session);
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [f1.to_string()],
            "path": "structure.location", "value": fld.to_string()}}),
    );
    done(&mut shell, "layout.route", json!({}));
    let card = cabinet_card(&shell).expect("the card");
    assert!(
        !card.route.all && card.route.lengths.is_empty() && card.route.gone > 0,
        "{:?}",
        card.route
    );
    assert!(
        card.lead.starts_with("Route: cabinet +CAB. ")
            && card
                .lead
                .contains(" of wires this cabinet no longer lays are removed. "),
        "{}",
        card.lead
    );
    let before = shell.revision();
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(shell.revision(), before.map(|r| r + 1), "one step");
    assert!(
        toasts(&shell)
            .iter()
            .any(|t| t.starts_with("Removed ") && t.ends_with(" ways of wires +CAB no longer lays.")),
        "{:?}",
        toasts(&shell)
    );
    done(&mut shell, "layout.route", json!({}));
    assert!(
        toasts(&shell)
            .iter()
            .any(|t| t == "Every wire of +CAB has its way."),
        "{:?}",
        toasts(&shell)
    );
}

#[test]
fn route_on_902_opens_the_cabinets_card_and_apply_stores_the_ways() {
    let tmp = Tmp::new("vm-cabinet-route");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    assert!(
        !hud(&shell).buttons.iter().any(|b| b.action == "layout.route"),
        "the sample is routed: nothing is due"
    );
    // Route then says so, and its card opens with every wire planned again.
    let before = shell.canvas().unwrap().viewport();
    done(&mut shell, "layout.route", json!({}));
    assert!(
        toasts(&shell)
            .iter()
            .any(|t| t == "Every wire of +CAB has its way."),
        "{:?}",
        toasts(&shell)
    );
    let card = cabinet_card(&shell).expect("the card");
    assert!(card.route.all && !card.route.lengths.is_empty());
    // Its groups in three columns: Ducts; Lengths; Typed by hand, Made on site, Without a way and the tick.
    let columns: Vec<Vec<String>> = card
        .columns()
        .0
        .iter()
        .map(|c| c.iter().map(|g| g.heading.clone()).collect())
        .collect();
    assert_eq!(
        columns,
        [
            vec!["Ducts".to_owned()],
            vec!["Lengths".to_owned()],
            vec![
                "Typed by hand".to_owned(),
                "Made on site".to_owned(),
                "Without a way".to_owned(),
                String::new()
            ]
        ]
    );
    let groups: Vec<_> = card.columns().groups().cloned().collect();
    let typed = groups[2]
        .rows
        .iter()
        .find(|r| r.cells[0] == "101.1")
        .expect("101.1");
    assert!(
        typed.cells[1].starts_with("keeps 400 mm; Route gives "),
        "{:?}",
        typed.cells
    );
    let site = groups[3]
        .rows
        .iter()
        .find(|r| r.cells[0] == "104.4")
        .expect("104.4");
    assert!(
        site.cells[1].starts_with("its end ") && site.cells[1].ends_with(" is in +FLD"),
        "{:?}",
        site.cells
    );
    let duct = &groups[0].rows[0];
    assert!(
        duct.cells[2].contains(" wires · fullest piece ")
            && duct.note.as_deref().is_some_and(|n| n.contains("limit 60 %")),
        "{duct:?}"
    );
    assert_eq!(
        groups[5].tick.as_ref().map(|t| t.words.as_str()),
        Some("Route every wire again (0 more: every wire is planned)")
    );
    // The canvas shows the cabinet whole while the card is open; the view comes back when it closes.
    assert!(shell.canvas().unwrap().region().is_some(), "the cabinet whole");
    let marks = shell.cabinet_marks(0);
    assert!(
        marks.notes.iter().any(|(_, w)| w.ends_with(" wires to the door")),
        "{marks:?}"
    );
    done(&mut shell, "preview.cancel", json!({}));
    assert!(cabinet_card(&shell).is_none());
    assert!(shell.canvas().unwrap().region().is_none());
    assert_eq!(shell.canvas().unwrap().viewport(), before);

    // -X1031 turned half round on its rail: its wires' ways no longer hold, and the toast offers Route.
    let t = shell.panel_tables().expect("902's tables");
    let fp = t
        .placements
        .iter()
        .find(|p| p.tag == "-X1031")
        .expect("-X1031")
        .footprint;
    let centre = (
        (i128::from(fp.min().x.0) + i128::from(fp.max().x.0)) / 2,
        (i128::from(fp.min().y.0) + i128::from(fp.max().y.0)) / 2,
    );
    let at = shell
        .canvas()
        .unwrap()
        .page()
        .snapper
        .page_to_sheet(t.to_page(centre));
    common::click(&mut shell, at, json!({}));
    assert_eq!(shell.selected_placements(), 1, "a click selects -X1031");
    assert!(
        !shell.selected_ways(0).is_empty(),
        "the selected strip shows the ways of its wires"
    );
    done(&mut shell, "layout.flip", json!({}));
    assert!(
        toasts(&shell)
            .iter()
            .any(|t| t.contains("The ways of ") && t.ends_with(" wires no longer hold.")),
        "{:?}",
        toasts(&shell)
    );
    assert!(
        shell.selected_ways(0).iter().all(|(_, stale)| *stale),
        "drawn dashed"
    );
    let offered = shell.notifications().visible().iter().any(|n| {
        n.text.starts_with("Turned -X1031. The ways of ")
            && n.label("route") == "Route"
            && !n.label("undo").is_empty()
    });
    assert!(
        offered,
        "the toast says the turn and offers Undo and Route: {:?}",
        toasts(&shell)
    );

    // Route plans the wires whose ways no longer hold: the card, the canvas's ways and the duct's chip.
    done(&mut shell, "layout.route", json!({}));
    let card = cabinet_card(&shell).expect("the cabinet's card");
    assert!(card.lead.starts_with("Route: cabinet +CAB. "), "{}", card.lead);
    assert!(
        card.columns()
            .groups()
            .any(|g| g.heading == "Lengths" && g.count == Some(g.rows.len())),
        "{card:?}"
    );
    let marks = shell.cabinet_marks(0);
    assert!(
        !marks.ways.is_empty() && marks.chips.iter().any(|(_, w)| w.ends_with(" %")),
        "{marks:?}"
    );
    // A row picked shows its way alone.
    let wire = card.route.lengths[0].wire;
    done(&mut shell, "layout.route_card", json!({"wire": wire.to_string()}));
    let picked = shell.cabinet_marks(0);
    assert_eq!(picked.ways.iter().filter(|(_, p)| *p).count(), 1);
    // The tick plans every wire again.
    done(&mut shell, "layout.route_card", json!({"all": true}));
    assert!(cabinet_card(&shell).is_some_and(|c| c.route.all));
    done(&mut shell, "layout.route_card", json!({"all": false}));

    let before = shell.revision();
    done(&mut shell, "preview.apply", json!({}));
    assert_eq!(shell.revision(), before.map(|r| r + 1), "one step");
    assert!(
        toasts(&shell)
            .iter()
            .any(|t| t.starts_with("Routed ") && t.ends_with(" of +CAB.")),
        "{:?}",
        toasts(&shell)
    );
    assert!(
        shell.selected_ways(0).iter().all(|(_, stale)| !*stale),
        "the ways hold again"
    );
}

#[test]
fn route_on_a_schematic_page_routes_the_drawing() {
    let tmp = Tmp::new("vm-cabinet-route-101");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut shell, "layout.route", json!({}));
    assert!(
        cabinet_card(&shell).is_none(),
        "no cabinet card on a schematic page"
    );
}

/// A row of the Connections table selected beside 902 (doc 12-3d §12.5): the panel page shows the surface whole, the
/// wire's way with its chip and the device in the door it leaves for; the view comes back when the selection goes.
#[test]
fn a_wire_selected_in_the_table_shows_its_way_on_902() {
    let tmp = Tmp::new("vm-cabinet-route-v2");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    let before = shell.canvas().unwrap().viewport();
    done(&mut shell, "navigator.show", json!({"kind": "connections"}));
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "connections", "keys": ["104.3"]}),
    );
    let labels: Vec<String> = shell.selected_way_labels(0).into_iter().map(|(_, w)| w).collect();
    assert!(
        labels
            .iter()
            .any(|w| w.starts_with("104.3 · ") && w.ends_with(" mm"))
            && labels.iter().any(|w| w == "to -104E1 in the door"),
        "{labels:?}"
    );
    assert!(shell.canvas().unwrap().region().is_some(), "the surface whole");
    done(&mut shell, "select.set", json!({"ids": []}));
    assert!(shell.canvas().unwrap().region().is_none());
    assert_eq!(shell.canvas().unwrap().viewport(), before);
}

/// The same row's Properties in the window are headed by the wire, not by its count of stretches.
#[test]
fn a_wire_selected_in_the_table_is_headed_by_its_number_and_ends() {
    let tmp = Tmp::new("vm-cabinet-route-v2-title");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    done(&mut shell, "navigator.show", json!({"kind": "connections"}));
    done(
        &mut shell,
        "navigator.select",
        json!({"kind": "connections", "keys": ["104.3"]}),
    );
    let sheet = shell.properties().sheet().expect("a sheet");
    assert_eq!(sheet.header.title, "104.3 · -104E1:X2 → -104A1-A1:M");
    // Its section counts the wire, not its stretches.
    let section = &sheet.sections[0];
    assert!(section.targets.len() > 1, "the wire's stretches");
    assert_eq!(
        ew_viewmodel::properties::section_title(sheet, section),
        "Connection (1)"
    );
}

/// A Route that lays wires and removes stored ways names the removed ways on its card before Apply (doc 12-3d §8.5):
/// -103F1 left without a location makes 101.8 unfinished, and its way goes with Route every wire again; the card says
/// "… ways of wires +CAB no longer lays are removed: … 101.8 …", the toast after Apply says what it routed and how
/// many ways it removed, and Undo brings the way back (the next Route names it again).
#[test]
fn route_names_the_ways_it_removes_also_when_it_lays_other_wires() {
    let tmp = Tmp::new("vm-cabinet-named-gone");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-103F1"}],
            "path": "structure.location", "value": "none"}}),
    );
    let removed = |shell: &ShellModel| {
        let card = cabinet_card(shell).expect("the card");
        card.columns()
            .0
            .iter()
            .flatten()
            .map(|g| g.heading.clone())
            .find(|h| h.contains(" no longer lays are removed: "))
    };
    done(&mut shell, "layout.route", json!({}));
    done(&mut shell, "layout.route_card", json!({"all": true}));
    let card = cabinet_card(&shell).expect("the card");
    assert!(!card.route.lengths.is_empty(), "it lays the cabinet's wires");
    let words = removed(&shell).expect("the card names the ways it removes");
    assert!(words.contains("101.8"), "{words}");
    done(&mut shell, "preview.apply", json!({}));
    assert!(
        toasts(&shell)
            .iter()
            .any(|t| t.starts_with("Routed ") && t.contains(" Removed ") && t.contains(" no longer lays.")),
        "{:?}",
        toasts(&shell)
    );
    done(&mut shell, "edit.undo", json!({}));
    done(&mut shell, "layout.route", json!({}));
    let words = removed(&shell).expect("the way is back, so Route removes it again");
    assert!(words.contains("101.8"), "{words}");
    shell.shutdown();
}
