// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire behind a breaker that nothing feeds takes no size from the breaker's rating, since no circuit leads to it
//! (doc 08 §4.2); its Cross-section row says so, naming the breaker, where it said only "profile"; and so does a wire
//! no supply reaches or no protective device stands before.

use crate::common;

use common::{Tmp, done, tables};
use ew_viewmodel::properties::{Provenance, find_row};
use ew_viewmodel::shell::Mode;
use serde_json::json;

/// A 24 V control wire of folio 102, from the C16 breaker -102F1 to the strip -X1021, by its ends.
const W29: &str = "wire:-102F1:2 -> -X1021:1/a";

fn csa_via(shell: &ew_viewmodel::shell::ShellModel) -> ew_viewmodel::ew_api::Text {
    let sheet = shell.properties().sheet().expect("a sheet");
    match &find_row(sheet, "connection/derived.csa")
        .expect("the Cross-section row")
        .1
        .provenance
    {
        Provenance::Derived { via } => via.clone(),
        other => panic!("{other:?}"),
    }
}

/// The Cross-section rows' texts of every wire of the page shown, where they are derived.
fn page_vias(shell: &mut ew_viewmodel::shell::ShellModel) -> Vec<ew_viewmodel::ew_api::Text> {
    let ids: Vec<String> = tables(shell)
        .connections
        .iter()
        .map(|c| c.id.to_string())
        .collect();
    let mut out = Vec::new();
    for id in ids {
        done(shell, "select.set", json!({"ids": [id]}));
        let Some(sheet) = shell.properties().sheet() else {
            continue;
        };
        if let Some((_, row)) = find_row(sheet, "connection/derived.csa")
            && let Provenance::Derived { via } = &row.provenance
        {
            out.push(via.clone());
        }
    }
    out
}

/// Where no rating sizes a wire, its Cross-section row says why at the size: on the supply page a mains wire before
/// any protective device ("no protection"), and behind a breaker whose feed is taken away a wire that does not end on
/// it ("no supply"); a size from the profile said only "profile" there.
#[test]
fn a_wire_no_protective_device_sizes_says_why_at_its_size() {
    let tmp = Tmp::new("csa-unrated");
    let mut shell = tmp.sample_in(Mode::Headless, true, "101");
    let none = page_vias(&mut shell)
        .into_iter()
        .find(|v| v.to_string().ends_with("no protective device upstream sizes it"))
        .expect("a mains wire before the first protective device");
    assert_eq!(
        ew_viewmodel::properties::chip_word(&Provenance::Derived { via: none }),
        "no protection"
    );
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    let feed: Vec<String> = tables(&shell)
        .cps
        .iter()
        .filter(|c| c.label == "-102F1:1")
        .flat_map(|c| c.connections.iter().map(ToString::to_string))
        .collect();
    done(&mut shell, "select.set", json!({"ids": feed}));
    done(&mut shell, "edit.delete_selection", json!({}));
    let unfed = page_vias(&mut shell)
        .into_iter()
        .find(|v| {
            v.to_string()
                .ends_with("no supply found upstream, so no protective device sizes it")
        })
        .expect("a wire behind the breaker that does not end on it");
    assert_eq!(
        ew_viewmodel::properties::chip_word(&Provenance::Derived { via: unfed }),
        "no supply"
    );
    shell.shutdown();
}

#[test]
fn a_wire_behind_an_unfed_breaker_says_why_its_rating_is_not_used() {
    let tmp = Tmp::new("csa-unfed");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    done(&mut shell, "select.set", json!({"refs": [W29]}));
    assert!(
        !csa_via(&shell).to_string().contains("no supply"),
        "fed: {}",
        csa_via(&shell)
    );
    // The breaker's feed taken away: the wires at its input.
    let feed: Vec<String> = tables(&shell)
        .cps
        .iter()
        .filter(|c| c.label == "-102F1:1")
        .flat_map(|c| c.connections.iter().map(ToString::to_string))
        .collect();
    assert!(!feed.is_empty(), "the breaker's feed");
    done(&mut shell, "select.set", json!({"ids": feed}));
    done(&mut shell, "edit.delete_selection", json!({}));
    done(&mut shell, "select.set", json!({"refs": [W29]}));
    let via = csa_via(&shell);
    assert!(
        via.to_string()
            .ends_with("no supply found upstream of -102F1, so its rating is not used"),
        "{via}"
    );
    // The row's chip says it where it shows, the whole text on hover.
    assert_eq!(
        ew_viewmodel::properties::chip_word(&Provenance::Derived { via }),
        "no supply"
    );
    // A size set by hand there (0.5 mm² behind the C16 breaker) is checked against no rating, and its row says why.
    let wire = shell.selection().items[0].0.to_string();
    done(
        &mut shell,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "connection", "targets": [wire],
            "path": "overrides.csa", "value": "0.5"}}),
    );
    let sheet = shell.properties().sheet().expect("a sheet");
    let row = find_row(sheet, "connection/derived.csa")
        .expect("the Cross-section row")
        .1;
    match &row.provenance {
        Provenance::Overridden { note } => assert!(
            note.to_string()
                .ends_with("no supply found upstream of -102F1, so its rating is not used"),
            "{note}"
        ),
        other => panic!("{other:?}"),
    }
    shell.shutdown();
}
