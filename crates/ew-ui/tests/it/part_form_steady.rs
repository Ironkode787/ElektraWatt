// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The New part form stands still under the pointer (FT-2130): the pins' preview and the bits the channels occupy have
//! their room before the channels are typed, and a station's form opened after a form whose bus row was taken away
//! starts at its top with its first field focused, not scrolled down to its own bus row. A person clicked where a
//! field had been and typed into another.

use crate::own_parts::{fill, new_project, press};
use crate::relays_paste_and_ip_wires::key;
use crate::support::{Scripted, Tmp, dispatch, harness, settle};

use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::visual::Driver;
use serde_json::json;

/// The centre's height of the last widget named `name` (the dialog's, painted over the window); `None` when no widget
/// of that name is in sight.
fn y_of(d: &Driver, name: &str) -> Option<i64> {
    d.labels()
        .into_iter()
        .rfind(|(l, _)| l == name)
        .map(|(_, at)| at.1)
}

/// New part… in the device browser.
fn open_new_part(d: &mut Driver) {
    d.input(&key("Shift+I"));
    d.settle();
    press(d, "New part…");
}

#[test]
fn the_fields_under_the_channels_stay_while_the_channels_are_typed() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    open_new_part(&mut d);
    press(&mut d, "PLC role");
    press(&mut d, "Module");
    // The form from the top, as a person types a card of eight inputs and four outputs.
    press(&mut d, "Channel kind");
    press(&mut d, "DI");
    let before = y_of(&d, "Supply L+ pin");
    assert!(before.is_some(), "the supply pins are in sight");
    fill(&mut d, "Channels", "8");
    for hint in ["Channel 0 → pin 1 … channel 7 → pin 8", "Occupies 8 input bits"] {
        assert!(d.texts().iter().any(|t| t == hint), "{hint}: {:?}", d.texts());
    }
    assert_eq!(
        y_of(&d, "Supply L+ pin"),
        before,
        "the pins' preview and the bits had their room"
    );
    press(&mut d, "Second channel kind");
    press(&mut d, "DO");
    let before = y_of(&d, "Supply L+ pin");
    fill(&mut d, "Its channels", "4");
    assert!(
        d.texts().iter().any(|t| t == "Occupies 4 output bits"),
        "the second way's bits: {:?}",
        d.texts()
    );
    assert_eq!(
        y_of(&d, "Supply L+ pin"),
        before,
        "the second way's bits had their room"
    );
}

#[test]
fn the_form_opened_again_starts_at_its_top_on_its_first_field() {
    type H = egui_kittest::Harness<'static, ew_ui::app::App>;
    let tmp = Tmp::new();
    let mut h = harness(&tmp, Scripted::default(), None);
    settle(&mut h);
    let project = json!({"dir": tmp.0.join("p.ew").to_string_lossy(), "template": "builtin"});
    dispatch(&mut h, "file.new", project);
    settle(&mut h);
    // New part… of a new station's catalogue: a CPU's form, longer than the dialog.
    dispatch(&mut h, "plc.new_station", json!({}));
    settle(&mut h);
    let open = |h: &mut H| {
        h.get_by_label("New part…").click();
        settle(h);
    };
    let first = |h: &H| {
        let node = h
            .query_all_by_label("Manufacturer")
            .last()
            .expect("the form's first field");
        (node.rect().min.y, node.accesskit_node().is_focused())
    };
    open(&mut h);
    let (top, focused) = first(&h);
    assert!(focused, "the first field has the keys");
    // Down to the bus interfaces, the preset's row taken away (a card has none), then Cancel while the form stands there.
    h.hover_at(egui::pos2(960.0, 600.0));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -20.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut h);
    assert!(first(&h).0 < top, "the form scrolled");
    h.get_by_label("Remove interface X1").click();
    settle(&mut h);
    h.get_by_label("Cancel").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "the form is closed");
    open(&mut h);
    assert_eq!(
        first(&h),
        (top, true),
        "the form opens at its top, on its first field"
    );
    h.state_mut().shell_mut().shutdown();
}
