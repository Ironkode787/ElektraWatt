// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A part's data at a glance (F22): the device browser's detail shows the part under the cursor as a card, and a
//! placed device's Properties link Part details. The window's own path on a new project.

use crate::own_parts::new_project;
use crate::persona_round::key;
use crate::support::Tmp;

use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::Input;
use serde_json::json;

/// The owner's wish: quickly see a part's specifications in the device browser. The starter contactor under the cursor
/// shows its manufacturer and number, its technical data by their labels in words, its size to scale with how it is
/// mounted, what it gives (its contacts, its connection points) and where it comes from; placed, its device's
/// Properties link Part details.
#[test]
fn the_device_browser_shows_a_parts_data_and_a_placed_device_links_them() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    assert_eq!(
        d.shell_mut().dispatch("tool.place_part", &json!({})),
        Outcome::Done
    );
    let filter = json!({"text": "EW-GEN-CON-1"});
    assert_eq!(d.shell_mut().dispatch("picker.filter", &filter), Outcome::Done);
    d.settle();
    let view = d
        .shell_mut()
        .picker()
        .and_then(|p| p.card.clone())
        .expect("the contactor's card");
    let w = &view.words;
    assert!(w.title.ends_with("EW-GEN-CON-1"), "{}", w.title);
    assert!(!w.rows.is_empty(), "its technical data");
    assert!(
        w.rows.iter().all(|(label, _)| !label.contains('_')),
        "labels in words: {:?}",
        w.rows
    );
    assert!(w.size.contains(" mm") && w.size.contains("TS 35"), "{}", w.size);
    assert!(
        w.gives.iter().any(|g| g.contains("connection points")),
        "{:?}",
        w.gives
    );
    assert!(w.gives.len() > 1, "its functions too: {:?}", w.gives);
    assert!(
        w.source.contains("ElektraWatt starter parts, version"),
        "{}",
        w.source
    );
    d.input(&Input::Wait { ms: 100 });
    let texts = d.texts();
    for heading in ["Size", "Gives", "From"] {
        assert!(texts.iter().any(|t| t == heading), "the detail shows {heading}");
    }
    // Placed, the device's sheet links Part details.
    assert_eq!(d.shell_mut().dispatch("picker.choose", &json!({})), Outcome::Done);
    let _ = d
        .shell_mut()
        .dispatch("cursor.set", &json!({"x_mm": 100.0, "y_mm": 100.0}));
    let _ = d.shell_mut().dispatch("canvas.activate", &json!({}));
    d.input(&key("Escape"));
    d.settle();
    let rep = d.shell_mut().canvas().expect("the page").page().tables.reps[0]
        .id
        .to_string();
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({"ids": [rep]})),
        Outcome::Done
    );
    d.input(&Input::Wait { ms: 100 });
    assert!(
        d.labels().iter().any(|(l, _)| l == "Part details"),
        "the device's Properties link Part details"
    );
}
