// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1845: the part dialog finds a part by what it is for as well as by its words, and chooses no row for the person.
//! On a copy of the sample with the compiled-in starter catalogue: its position switch is filed under the category
//! `SensingPosition` ("Position sensors") and its pressure switch under `FluidSensor`, neither description says
//! "sensor"; its contactor's description names an auxiliary normally open contact while its main function is the
//! coil, and its pushbutton's main function is a normally open contact.

use crate::common;

use common::{Tmp, done, refused};
use ew_viewmodel::dialogs::spec;
use ew_viewmodel::shell::{Dialog, Mode, ShellModel};
use serde_json::json;

/// The part dialog of the sample's power supply -101G1 with `text` searched.
fn searched(tmp: &Tmp, text: &str) -> ShellModel {
    let mut s = tmp.sample_in(Mode::Headless, true, "101");
    done(&mut s, "select.set", json!({ "refs": ["tag:-101G1"] }));
    done(&mut s, "part.choose", json!({}));
    done(&mut s, "part.search", json!({ "text": text }));
    s
}

/// The part numbers the dialog offers, in order.
fn offered(s: &ShellModel) -> Vec<String> {
    let Some(Dialog::PartChoose(c)) = s.dialog() else {
        panic!("no part dialog")
    };
    c.offers.iter().map(|o| o.part_number.clone()).collect()
}

fn position(list: &[String], pn: &str) -> usize {
    list.iter()
        .position(|p| p == pn)
        .unwrap_or_else(|| panic!("{pn} not offered: {list:?}"))
}

#[test]
fn sensor_finds_the_parts_filed_as_sensors() {
    let tmp = Tmp::new("purpose-sensor");
    let s = searched(&tmp, "sensor");
    let list = offered(&s);
    let switches = [position(&list, "EW-GEN-POS-1"), position(&list, "EW-GEN-PSW-1")];
    // A part found by its words only (the IO-Link sensor cable, where it is offered) comes after them.
    if let Some(cable) = list.iter().position(|p| p == "EW-GEN-CBL-29") {
        assert!(switches.iter().all(|i| *i < cable), "{list:?}");
    }
}

#[test]
fn normally_open_contact_ranks_the_parts_whose_main_function_is_one_first() {
    let tmp = Tmp::new("purpose-no");
    let s = searched(&tmp, "normally open contact");
    let list = offered(&s);
    let contactor = position(&list, "EW-GEN-CON-1");
    for pn in ["EW-GEN-PB-1", "EW-GEN-POS-1", "EW-GEN-SEL-1"] {
        assert!(position(&list, pn) < contactor, "{pn} before the contactor: {list:?}");
    }
}

#[test]
fn no_row_is_chosen_until_the_person_chooses_one() {
    let tmp = Tmp::new("purpose-cursor");
    let mut s = searched(&tmp, "pushbutton");
    let Some(dialog) = s.dialog() else {
        panic!("no part dialog")
    };
    let Dialog::PartChoose(c) = dialog else {
        panic!("not the part dialog")
    };
    assert!(!c.offers.is_empty());
    assert_eq!(c.cursor, None);
    assert!(
        spec(dialog, &s).primary.disabled.is_some(),
        "Choose waits for a row"
    );
    assert_eq!(
        refused(&mut s, "part.pick", json!({})),
        "reason-part-choose-none"
    );
    done(&mut s, "part.pick", json!({ "index": 1 }));
}
