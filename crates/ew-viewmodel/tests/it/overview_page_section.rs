// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1823: the overview page a valve terminal's To do makes stands with its fluid diagram (doc 10 §6): a terminal
//! made in the station view of the sample and drawn first on an electrical page of the wiring section, then on the
//! fluid page of the Pneumatics section, gets its overview page in the Pneumatics section.

use crate::common::{Tmp, done};
use ew_viewmodel::shell::Mode;
use serde_json::json;

/// The To do's three steps on a new valve terminal of the sample: its box on page 104, its box on fluid page 201,
/// then Make the overview page, applied. The new overview page is in the section of 201, not of 104.
#[test]
fn the_overview_page_of_the_todo_stands_in_the_fluid_section() {
    let tmp = Tmp::new("overview-section");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    let _ = s.dispatch("plc.new_station", &json!({"kind": "valve_terminal"}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-VTM-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    let station = s.station_tab().and_then(|t| t.station).expect("the station");
    // Each step shows the page it draws on; the view opens again for the next.
    let view = json!({"station": station.to_string()});
    done(&mut s, "plc.rack_next", json!({"step": "draw", "folio": "104"}));
    done(&mut s, "plc.rack_editor", view.clone());
    // On the fluid page its box is on the pointer: a click below the drawing places it.
    done(&mut s, "plc.rack_next", json!({"step": "fluid", "folio": "201"}));
    done(&mut s, "nav.goto", json!({"text": "201-G8"}));
    done(&mut s, "canvas.activate", json!({"alt": true}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "plc.rack_editor", view);
    let before: Vec<String> = s.page_rows().iter().map(|r| r.folio.clone()).collect();
    done(&mut s, "plc.rack_next", json!({"step": "overview"}));
    done(&mut s, "preview.apply", json!({}));
    let section_of = |folio: &str| {
        s.page_rows()
            .iter()
            .find(|r| r.folio == folio)
            .and_then(|r| r.section.clone())
    };
    let new: Vec<String> = s
        .page_rows()
        .iter()
        .filter(|r| ew_viewmodel::ew_api::output::page_kind_name(&r.kind) == "assembly_overview" && !before.contains(&r.folio))
        .map(|r| r.folio.clone())
        .collect();
    assert!(!new.is_empty(), "the overview page is made");
    for folio in &new {
        assert_eq!(section_of(folio), section_of("201"), "overview page {folio}");
        assert_ne!(section_of(folio), section_of("104"), "overview page {folio}");
    }
    s.shutdown();
}
