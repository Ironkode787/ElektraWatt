// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The window's ways to the two things a draughtsman's drawing showed and the model had no place for: Join
//! device… (J) on a terminal's symbol offers the other terminals of its strip as "next level" and joins it into the
//! chosen one (doc 08 §6.3); the part dialog's Type not known yet… gives the selected devices a placeholder part, and a
//! real part chosen later for one of them replaces it on all (doc 07 §3.3). On a writable copy of the sample, folio 102.

use crate::common;

use common::{Tmp, done, failed, tables};
use ew_viewmodel::shell::{Dialog, Mode, ShellModel};
use ew_viewmodel::tools::ChooserValue;
use serde_json::json;

fn shell(tmp: &Tmp) -> ShellModel {
    tmp.sample_in(Mode::Headless, true, "102")
}

/// The device ID text of the terminal `-X1021:<n>` (selected by its tag, the selection is the device).
fn terminal(s: &mut ShellModel, n: &str) -> String {
    done(s, "select.set", json!({"refs": [format!("tag:-X1021:{n}")]}));
    s.selection()
        .items
        .first()
        .expect("the terminal is selected")
        .0
        .to_string()
}

/// Selects the symbol of `device` on 102.
fn select_symbol(s: &mut ShellModel, device: &str) {
    let rep = tables(s)
        .reps
        .iter()
        .find(|r| r.device.is_some_and(|d| d.to_string() == device))
        .map(|r| r.id.to_string())
        .unwrap_or_else(|| panic!("{device} drawn on 102"));
    done(s, "select.set", json!({"ids": [rep]}));
}

#[test]
fn j_on_a_terminal_joins_it_into_another_terminal_of_its_strip_as_its_next_level() {
    let tmp = Tmp::new("join-level");
    let mut s = shell(&tmp);
    let five = terminal(&mut s, "5");
    let six = terminal(&mut s, "6");
    select_symbol(&mut s, &six);
    done(&mut s, "place.join", json!({}));
    let chooser = s.chooser().expect("the Join chooser").clone();
    let row = chooser
        .rows
        .iter()
        .position(|r| r.name == "-X1021:5")
        .unwrap_or_else(|| panic!("a row for -X1021:5: {:?}", chooser.rows));
    assert_eq!(chooser.rows[row].kind, "next level");
    assert!(
        chooser
            .rows
            .iter()
            .all(|r| matches!(&r.value, ChooserValue::JoinRep { device, .. } if *device != six)),
        "its own terminal is no row"
    );
    done(&mut s, "chooser.choose", json!({"row": row}));
    assert_eq!(
        s.recorded_commands().last().map(|c| c.cmd.as_str()),
        Some("rep.join_device")
    );
    let devices: Vec<String> = tables(&s)
        .reps
        .iter()
        .filter_map(|r| r.device.map(|d| d.to_string()))
        .collect();
    assert!(!devices.contains(&six), "terminal 6 joined the block");
    assert_eq!(
        devices.iter().filter(|d| **d == five).count(),
        2,
        "the block is drawn twice on 102, once per level"
    );
}

#[test]
fn type_not_known_gives_a_placeholder_that_a_real_part_replaces_on_every_device() {
    let tmp = Tmp::new("placeholder");
    let mut s = shell(&tmp);
    let four: Vec<String> = ["1", "2", "3", "4"].iter().map(|n| terminal(&mut s, n)).collect();
    let refs: Vec<String> = ["1", "2", "3", "4"]
        .iter()
        .map(|n| format!("tag:-X1021:{n}"))
        .collect();
    done(&mut s, "select.set", json!({ "refs": refs }));
    done(&mut s, "part.choose", json!({}));
    let why = failed(&mut s, "part.placeholder", json!({"description": "  "}));
    assert!(why.contains("what the part is"), "{why}");
    done(
        &mut s,
        "part.placeholder",
        json!({"description": "Grey terminal"}),
    );
    assert!(s.dialog().is_none(), "the dialog closes");
    let step = s.recorded_commands().last().expect("a command").clone();
    assert_eq!(step.cmd, "part.placeholder");
    assert_eq!(step.params["description"], "Grey terminal");
    let mut given: Vec<String> = step.params["devices"]
        .as_array()
        .expect("devices")
        .iter()
        .map(|d| d.as_str().unwrap_or_default().to_owned())
        .collect();
    given.sort();
    let mut expected = four.clone();
    expected.sort();
    assert_eq!(given, expected);

    // One of them chosen again: the dialog's target is every device with the placeholder, and its pick replaces it.
    done(&mut s, "select.set", json!({"refs": ["tag:-X1021:2"]}));
    done(&mut s, "part.choose", json!({}));
    let Some(Dialog::PartChoose(c)) = s.dialog() else {
        panic!("the part dialog")
    };
    let ew_viewmodel::ew_api::PartTarget::Devices { devices, mode } = &c.target else {
        panic!("devices")
    };
    let mut target: Vec<String> = devices.iter().map(ToString::to_string).collect();
    target.sort();
    assert_eq!(target, expected, "every device with the placeholder");
    assert!(matches!(mode, ew_viewmodel::ew_api::AssignMode::Replace { .. }));
    done(&mut s, "part.search", json!({"text": "EW-GEN-TRM-1"}));
    done(&mut s, "part.pick", json!({"index": 1}));
    let step = s.recorded_commands().last().expect("a command").clone();
    assert_eq!(step.cmd, "part.assign");
    assert_eq!(step.params["mode"], "replace");
    assert_eq!(step.params["devices"].as_array().map(Vec::len), Some(4));
}
