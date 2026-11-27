// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New part… in the catalogue of a station not made yet: the form starts as the station's head, so a card
//! made there starts without the head's bus interface once its role is Module; and the part saved there stands in the
//! catalogue, offered, not put into the station's empty head unasked, unless the head was the place chosen.

use crate::common::{Tmp, done, new_project};
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The open part editor's bus rows.
fn bus_rows(s: &ShellModel) -> Vec<ew_viewmodel::ew_api::BusRow> {
    match s.parts_place().and_then(|p| p.sheet.as_ref()) {
        Some(e) => e.sheet.bus.clone(),
        None => panic!("a part is open in Parts"),
    }
}

/// A part made in the open editor: its maker and number, then Save.
fn save_part(s: &mut ShellModel, number: &str) {
    for (field, value) in [("manufacturer", "Example"), ("part_number", number)] {
        done(s, "parts.field", json!({"field": field, "value": value}));
    }
    done(s, "parts.save", json!({}));
    assert!(s.dialog().is_none(), "{number} is saved");
}

#[test]
fn a_card_made_from_a_new_station_s_catalogue_starts_without_a_bus_interface() {
    let tmp = Tmp::new("station-new-part-card");
    let mut s = new_project(&tmp);
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "parts.new", json!({"plc": "cpu"}));
    assert_eq!(bus_rows(&s).len(), 1, "the head's form starts with its bus row");
    done(
        &mut s,
        "parts.field",
        json!({"field": "plc.role", "value": "module"}),
    );
    assert_eq!(bus_rows(&s), Vec::new(), "a module starts without one");
    // A row the person filled in is the person's: it stays whatever the role.
    done(
        &mut s,
        "parts.field",
        json!({"field": "plc.role", "value": "station"}),
    );
    done(&mut s, "parts.row", json!({"table": "bus"}));
    done(
        &mut s,
        "parts.field",
        json!({"field": "bus.0.ports", "value": "2"}),
    );
    done(
        &mut s,
        "parts.field",
        json!({"field": "plc.role", "value": "module"}),
    );
    assert_eq!(bus_rows(&s).len(), 1, "the filled-in row stays");
}

#[test]
fn a_part_saved_from_a_new_station_s_catalogue_is_offered_not_made_its_head() {
    let tmp = Tmp::new("station-new-part-head");
    let mut s = new_project(&tmp);
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "parts.new", json!({"plc": "cpu"}));
    save_part(&mut s, "CPU-OWN-1");
    assert_eq!(
        s.station_draft().and_then(|d| d.head),
        None,
        "the head stays empty"
    );
    assert!(
        s.station_catalogue().iter().any(|p| p.number == "CPU-OWN-1"),
        "the catalogue offers the new part"
    );
    // With the empty head chosen first, the part made is its head.
    done(&mut s, "plc.rack_select", json!({"head": true}));
    done(&mut s, "parts.new", json!({"plc": "cpu"}));
    save_part(&mut s, "CPU-OWN-2");
    let head = s.station_draft().and_then(|d| d.head).expect("a head");
    let number = s
        .station_catalogue()
        .into_iter()
        .find(|p| p.copy == head)
        .map(|p| p.number);
    assert_eq!(number.as_deref(), Some("CPU-OWN-2"));
}
