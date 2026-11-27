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

#[test]
fn route_on_902_opens_the_cabinets_card_and_apply_stores_the_ways() {
    let tmp = Tmp::new("vm-cabinet-route");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    assert!(
        !hud(&shell).buttons.iter().any(|b| b.action == "layout.route"),
        "the sample is routed: nothing is due"
    );

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
            .any(|t| t.starts_with("The ways of ") && t.ends_with(" wires no longer hold.")),
        "{:?}",
        toasts(&shell)
    );
    assert!(
        shell.selected_ways(0).iter().all(|(_, stale)| *stale),
        "drawn dashed"
    );
    let offered = shell
        .notifications()
        .visible()
        .iter()
        .any(|n| n.text.starts_with("The ways of ") && n.label("route") == "Route");
    assert!(offered, "the toast offers Route");

    // Route plans the wires whose ways no longer hold: the card, the canvas's ways and the duct's chip.
    done(&mut shell, "layout.route", json!({}));
    let card = cabinet_card(&shell).expect("the cabinet's card");
    assert!(card.lead.starts_with("Route: cabinet +CAB. "), "{}", card.lead);
    let sections: Vec<String> = card.sections().into_iter().map(|(h, _)| h).collect();
    assert!(
        sections.contains(&"Ducts".to_owned()) && sections.contains(&"Lengths".to_owned()),
        "{sections:?}"
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
