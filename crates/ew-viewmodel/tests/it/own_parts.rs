// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part of one's own made through the actions of Parts (`parts.new`, `parts.field`, `parts.save`) on new projects:
//! a module part that nothing draws is listed greyed in the next project's station catalogue. Not here: making parts
//! in the window by clicks and keys, placing them and their buses (`ew-ui`'s `own_parts.rs`).

use crate::common::{Tmp, done};

use ew_viewmodel::ew_api::library::LibraryTiers;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// A shell in the window's mode with the libraries `tiers`, on a new project `name` from the built-in template.
fn new_project(tmp: &Tmp, tiers: LibraryTiers, name: &str) -> ShellModel {
    let mut s = ShellModel::new(Mode::Gui, tmp.config().with_libraries(Some(tiers)));
    crate::common::make_project(&mut s, &tmp.0.join(name), &[]);
    s
}

/// A module part that nothing draws (its channels have no pins, so Parts gave it no function), in a library
/// installed for the next project, which holds no copy of it: the station's catalogue lists it greyed with what to do
/// in Parts, not left out silently, and taking it is refused with the same words. The library is the first
/// project's My parts, installed as the installation's own.
#[test]
fn a_module_part_nothing_draws_is_listed_greyed_with_what_to_do() {
    let tmp = Tmp::new("own-parts-unfit");
    let mine = LibraryTiers::under(&tmp.0.join("data"), &tmp.0.join("cache"), Vec::new(), Vec::new());
    let mut s = new_project(&tmp, mine, "p.ew");
    done(&mut s, "parts.new", json!({"start": "module"}));
    for (field, value) in [
        ("manufacturer", "Acme"),
        ("part_number", "DI-NP"),
        ("description", "Input card without pins"),
        ("plc.kind", "di"),
        ("plc.count", "8"),
        ("plc.pin_of", ""),
    ] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.save", json!({}));
    assert!(s.dialog().is_none(), "saved: {:?}", s.dialog());
    s.shutdown();
    drop(s);
    let installed = vec![tmp.0.join("data").join("libraries")];
    let tiers = LibraryTiers::under(&tmp.0.join("next"), &tmp.0.join("cache"), installed, Vec::new());
    let mut s = new_project(&tmp, tiers, "q.ew");
    let world = s.world_for_tests().expect("a project");
    let held = ew_commands::rows::model_lines(&world.model)
        .expect("every record has a JSON form")
        .into_iter()
        .filter_map(|(_, line)| serde_json::from_str::<Value>(&line).ok())
        .any(|v| v["part_number"] == "DI-NP");
    assert!(!held, "the next project holds no copy of it");
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-CPU-1"}));
    let why = "Not offered: nothing draws this part, as it has no pins. In Parts, give it a Pin pattern or \
               its supply pins under PLC, then save it";
    let listed = s
        .station_catalogue()
        .into_iter()
        .find(|p| p.number == "DI-NP")
        .expect("the station's catalogue lists DI-NP");
    assert_eq!(listed.unfit.as_deref(), Some(why));
    assert_eq!(
        s.dispatch("plc.rack_add", &json!({"part": "DI-NP"})),
        Outcome::Failed {
            message: why.to_owned()
        }
    );
    s.shutdown();
}
