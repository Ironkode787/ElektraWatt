// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part has a size that can be typed (F23): the part editor's Size and mounting, for a new part and a saved one.
//! The window's own path on a new project.

use crate::own_parts::{fill, new_project, press, records};
use crate::relays_paste_and_ip_wires::key;
use crate::support::Tmp;

use ew_viewmodel::shell::{Dialog, Outcome};
use serde_json::json;

/// The owner met no way to give a part its footprint. A new part is typed with its width, height, depth, its mounting
/// on a rail and its clearance above; a length that is no length is refused in words at its field. Saved, the project's
/// part has the size the layout draws from; edited again, its width is changed and the change shows as the part's
/// update card, as every other part change does.
#[test]
fn a_new_part_takes_its_size_and_a_saved_one_changes_it_through_the_card() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&key("Shift+I"));
    d.settle();
    press(&mut d, "New part…");
    fill(&mut d, "Maker", "Acme");
    fill(&mut d, "Part number", "RL-22");
    fill(&mut d, "Description", "Interface relay, 1 changeover");
    press(&mut d, "In the cabinet section");
    fill(&mut d, "Width (mm)", "six");
    assert!(
        d.texts()
            .iter()
            .any(|t| t.starts_with("Width: \u{201c}six\u{201d} is not a length")
                || t.starts_with("Width: \"six\" is not a length")),
        "a wrong width is refused in words in the form's foot, naming its field"
    );
    for _ in 0..3 {
        d.input(&key("Backspace"));
    }
    fill(&mut d, "Width (mm)", "6,2");
    fill(&mut d, "Height (mm)", "90");
    fill(&mut d, "Depth (mm)", "70");
    // Below the fold of the editor's scrolled body: the action the fields send.
    for (field, value) in [("mech.clearance.top", "10"), ("mech.mount_kind", "ts35x7_5")] {
        let set = json!({"field": field, "value": value});
        assert_eq!(d.shell_mut().dispatch("part.field", &set), Outcome::Done);
    }
    d.settle();
    press(&mut d, "Save");
    assert!(d.shell_mut().dialog().is_none(), "Save closes the editor");
    let part = records(&mut d)
        .into_iter()
        .find(|v| v["part_number"] == "RL-22")
        .expect("the project holds the new part");
    assert_eq!(
        part["mech"],
        json!({"width": 6_200_000, "height": 90_000_000, "depth": 70_000_000, "mount_kind": "ts35x7_5",
            "clearance": {"top": 10_000_000}}),
    );
    // Edited again: the width changes through the part's update card.
    let copy = part["id"].as_str().expect("the copy's ID").to_owned();
    let edit = json!({"part": format!("id:{copy}")});
    assert_eq!(d.shell_mut().dispatch("part.edit", &edit), Outcome::Done);
    d.settle();
    let text = match d.shell_mut().dialog() {
        Some(Dialog::PartEdit(e)) => e.draft.text("mech.width").to_owned(),
        other => panic!("the part editor: {other:?}"),
    };
    assert_eq!(text, "6.2", "the editor shows the saved width");
    let width = json!({"field": "mech.width", "value": "12"});
    assert_eq!(d.shell_mut().dispatch("part.field", &width), Outcome::Done);
    assert_eq!(d.shell_mut().dispatch("part.save", &json!({})), Outcome::Done);
    d.settle();
    assert!(
        matches!(
            d.shell_mut().properties().card().map(|c| &c.origin),
            Some(ew_viewmodel::properties::CardOrigin::Part(_))
        ),
        "the size change is the part's update card"
    );
}

/// At 1920 × 1080 the whole New part form shows with its sections closed: the box "Also add to my parts library"
/// stands inside the dialog above its foot row, and a field's message, said in the foot row, moves it not at all.
#[test]
fn the_new_part_forms_library_box_shows_and_a_message_moves_nothing() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&key("Shift+I"));
    d.settle();
    press(&mut d, "New part…");
    let at = |d: &ew_ui::visual::Driver, name: &str| {
        d.labels()
            .into_iter()
            .rfind(|(l, _)| l == name)
            .unwrap_or_else(|| panic!("no widget named {name:?}"))
            .1
    };
    let library = at(&d, "Also add to my parts library");
    let save = at(&d, "Save");
    let title = at(&d, "Maker");
    // A row below the box's centre and half a button above Save's: the box ends clear of the foot row.
    assert!(
        library.1 > title.1 && library.1 + 24 < save.1 - 18,
        "{library:?} {save:?}"
    );
    fill(&mut d, "Maker", "Acme");
    press(&mut d, "In the cabinet section");
    let before = at(&d, "Width (mm)");
    fill(&mut d, "Width (mm)", "six");
    assert!(
        d.texts().iter().any(|t| t.starts_with("Width: ")),
        "the foot names the field"
    );
    assert_eq!(at(&d, "Width (mm)"), before, "the message moves no field");
}
