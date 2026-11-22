// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A wire behind a breaker that nothing feeds takes no size from the breaker's rating, since no circuit leads to it
//! (doc 08 §4.2); its Cross-section row says so, naming the breaker, where it said only "profile".

use crate::common;

use common::{Tmp, done, tables};
use ew_viewmodel::properties::{Provenance, find_row};
use ew_viewmodel::shell::Mode;
use serde_json::json;

/// A 24 V control wire of folio 102, from the C16 breaker -102F1 to the strip -X1021, by its ends.
const W29: &str = "wire:-102F1:2 -> -X1021:1/a";

fn csa_via(shell: &ew_viewmodel::shell::ShellModel) -> String {
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

#[test]
fn a_wire_behind_an_unfed_breaker_says_why_its_rating_is_not_used() {
    let tmp = Tmp::new("csa-unfed");
    let mut shell = tmp.sample_in(Mode::Headless, true, "102");
    done(&mut shell, "select.set", json!({"refs": [W29]}));
    assert!(!csa_via(&shell).contains("no supply"), "fed: {}", csa_via(&shell));
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
        via.ends_with("no supply found upstream of -102F1, so its rating is not used"),
        "{via}"
    );
    // The row's chip says it where it shows, the whole text on hover.
    assert_eq!(
        ew_viewmodel::properties::chip_word(&Provenance::Derived { via }),
        "no supply"
    );
    shell.shutdown();
}
