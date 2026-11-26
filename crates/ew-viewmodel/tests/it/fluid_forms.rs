// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! New medium… suggests what to type, and the fluid and automation forms say what each field
//! changes.

use crate::common;

use common::{Tmp, done};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::json;

fn new_project(tmp: &Tmp, mode: Mode) -> ShellModel {
    tmp.new_project_in(mode, &[])
}

/// The toast's first line.
fn toast(s: &ShellModel) -> String {
    s.toast().map(|n| n.lines().0.to_owned()).unwrap_or_default()
}

/// New medium… starts from a usual medium: choosing one fills the name and its usual pressure, both still editable;
/// the form has no field that nothing reads (the ISO 1219-2 letter), and every field says what it is for.
#[test]
fn new_medium_starts_from_a_usual_medium_and_explains_its_fields() {
    let tmp = Tmp::new("medium-preset");
    let mut s = new_project(&tmp, Mode::Gui);
    done(&mut s, "fluid.medium_new", json!({}));
    let form = common::form(&s);
    let names: Vec<&str> = form.fields.iter().map(|f| f.name.as_ref()).collect();
    assert_eq!(names, ["preset", "name", "p_nom"]);
    let preset = &form.fields[0];
    assert!(
        preset
            .options
            .iter()
            .any(|(v, words)| v == "vacuum" && words == "Vacuum"),
        "{:?}",
        preset.options
    );
    for f in &form.fields {
        assert!(form.note(&f.name).is_some(), "{} says what it is for", f.name);
    }
    s.set_form_field("preset", "compressed_air");
    let form = common::form(&s);
    let value = |n: &str| form.fields.iter().find(|f| f.name == n).unwrap().value.clone();
    assert_eq!(
        (value("name"), value("p_nom")),
        ("Compressed air".to_owned(), "6".to_owned())
    );
    // Still editable: the person's own pressure is the one made.
    s.set_form_field("p_nom", "7");
    assert_eq!(s.submit_form(), Outcome::Done);
    assert!(toast(&s).contains("Compressed air"), "{}", toast(&s));
    // A script names the usual medium alone.
    let mut h = new_project(&Tmp::new("medium-preset-script"), Mode::Headless);
    done(&mut h, "fluid.medium_new", json!({"preset": "hydraulic_oil"}));
    assert!(toast(&h).contains("Hydraulic oil"), "{}", toast(&h));
}

/// Fluid settings…: every field says what it changes, with an example.
#[test]
fn the_fluid_settings_fields_say_what_they_change() {
    let tmp = Tmp::new("fluid-settings-hints");
    let mut s = new_project(&tmp, Mode::Gui);
    done(&mut s, "fluid.settings", json!({}));
    let form = common::form(&s);
    for f in &form.fields {
        let note = form
            .note(&f.name)
            .unwrap_or_else(|| panic!("{} has no hint", f.name));
        assert!(note.contains("such as"), "{}: {note}", f.name);
    }
}
