// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Auto-fill rail lets the person choose which devices go on the rail. Its form lists the cabinet's unplaced
//! devices and the devices without a location, each with a tick set by a first guess from what the model already says
//! (the cabinet's location, a part that goes on a rail, a field category); field devices start unticked and are
//! neither given the cabinet's location nor placed. A script names the devices or takes the guess.

use crate::common;

use common::{Tmp, done};
use ew_api::output::{PanelTables, Uuid};
use ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use serde_json::json;

fn panel(s: &ShellModel) -> std::sync::Arc<PanelTables> {
    s.panel_tables().expect("a panel page with a surface view")
}

/// The sample's devices printed as `tags` (the IDs are the fixture's, alike in every copy).
fn devices(tmp: &Tmp, tags: &[&str]) -> Vec<Uuid> {
    let (session, _) = ew_api::Session::open_read_only(&tmp.sample_copy(), "test").expect("the sample");
    tags.iter()
        .map(|tag| {
            session
                .resolve_reference(&format!("tag:{tag}"))
                .unwrap_or_else(|_| panic!("{tag}"))
        })
        .collect()
}

/// Whether `tag` stands on the page.
fn placed(s: &ShellModel, tag: &str) -> bool {
    panel(s).placements.iter().any(|p| p.tag == tag)
}

/// Whether `tag` waits in the cabinet's Unplaced devices: it has the cabinet's location and no placement.
fn in_cabinet(s: &ShellModel, tag: &str) -> bool {
    panel(s).unplaced.iter().any(|u| u.label == tag)
}

/// 902 with the breakers -102F1 and -103F1 taken off rail 1, and -103F1, the position switch -104B1 and the cylinder
/// -201M1 without a location; the rail -103F1 stood on (rail 1, which has room for both breakers).
fn set_up(tmp: &Tmp, s: &mut ShellModel) -> String {
    let t = panel(s);
    let rail = t
        .placements
        .iter()
        .find(|p| p.tag == "-103F1")
        .and_then(|p| p.carrier)
        .and_then(|c| t.carrier(c))
        .map(|c| c.label.to_string())
        .expect("-103F1 on a rail");
    for tag in ["-102F1", "-103F1"] {
        let p = t.placements.iter().find(|p| p.tag == tag).expect(tag).id;
        done(
            s,
            "command.run",
            json!({"cmd": "layout.remove", "params": {"placement": p.to_string()}}),
        );
    }
    let targets: Vec<String> = devices(tmp, &["-103F1", "-104B1", "-201M1"])
        .iter()
        .map(ToString::to_string)
        .collect();
    done(
        s,
        "command.run",
        json!({"cmd": "fields.set", "params": {"kind": "device", "targets": targets,
            "path": "structure.location", "value": "none"}}),
    );
    assert!(in_cabinet(s, "-102F1") && !in_cabinet(s, "-103F1"));
    rail
}

#[test]
fn the_form_ticks_the_cabinets_devices_and_leaves_the_field_unticked() {
    let tmp = Tmp::new("fill-choice");
    let mut s = tmp.sample_in(Mode::Gui, true, "902");
    let rail = set_up(&tmp, &mut s);
    done(&mut s, "layout.auto_fill_rail", json!({"rail": rail}));
    let Some(Dialog::Form(form)) = s.dialog().cloned() else {
        panic!("{:?}", s.dialog())
    };
    let table = form.table.expect("the devices with their ticks");
    let tick = |tag: &str| {
        table
            .rows
            .iter()
            .find(|r| r.key == tag)
            .map(|r| r.cells[0] == "yes")
            .unwrap_or_else(|| panic!("{tag} is offered: {:?}", table.rows))
    };
    assert!(tick("-102F1"), "the cabinet's own breaker");
    assert!(
        tick("-103F1"),
        "a breaker without a location whose part goes on the rail"
    );
    assert!(!tick("-104B1"), "a position switch is field equipment");
    assert!(!tick("-201M1"), "so is a cylinder");

    assert_eq!(s.submit_form(), Outcome::Done);
    // -103F1 is placed in the cabinet's space, so it took the cabinet's location (the fill places no other).
    assert!(placed(&s, "-102F1") && placed(&s, "-103F1"));
    assert!(!placed(&s, "-104B1") && !placed(&s, "-201M1"));
    assert!(!in_cabinet(&s, "-104B1"), "the field device took no location");
    // The location and the fill are one undo step.
    done(&mut s, "edit.undo", json!({}));
    // Its page is the layout page it placed on: the Undo stays there.
    assert_eq!(s.current_folio(), Some("902"));
    assert!(!placed(&s, "-103F1") && !in_cabinet(&s, "-103F1"));
    assert!(in_cabinet(&s, "-102F1"));
    s.shutdown();
}

#[test]
fn a_script_names_the_devices_or_takes_the_guess() {
    let tmp = Tmp::new("fill-script");
    let mut s = tmp.sample_in(Mode::Headless, true, "902");
    let rail = set_up(&tmp, &mut s);
    // Named: only -103F1, which takes the cabinet's location with it.
    done(
        &mut s,
        "layout.auto_fill_rail",
        json!({"rail": rail, "order": "dt", "devices": ["-103F1"]}),
    );
    assert!(placed(&s, "-103F1") && !placed(&s, "-102F1"));
    // A device it does not offer (now placed) is refused in words.
    let Outcome::Failed { message } = s.dispatch(
        "layout.auto_fill_rail",
        &json!({"rail": rail, "order": "dt", "devices": ["-103F1"]}),
    ) else {
        panic!("a placed device is refused")
    };
    assert!(
        message.starts_with("-103F1 is not a device Auto-fill rail can place here"),
        "{message}"
    );
    // The guess: -102F1, and not the field devices.
    done(
        &mut s,
        "layout.auto_fill_rail",
        json!({"rail": rail, "order": "dt"}),
    );
    assert!(placed(&s, "-102F1"));
    assert!(!placed(&s, "-104B1") && !placed(&s, "-201M1"));
    assert!(!in_cabinet(&s, "-201M1"));
    s.shutdown();
}
