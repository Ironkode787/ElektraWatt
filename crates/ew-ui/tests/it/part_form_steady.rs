// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The New part form stands still under the pointer: the pins' preview and the bits the channels occupy have
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
    press(d, "PLC card or station");
}

#[test]
fn the_fields_under_the_channels_stay_while_the_channels_are_typed() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    open_new_part(&mut d);
    press(&mut d, "PLC section");
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

/// The PLC section's fields, top to bottom, that stand whatever is typed (Its channels comes with a second kind,
/// which these steps do not choose).
const FIELDS: [&str; 9] = [
    "Channels",
    "Second channel kind",
    "Data type",
    "Channel name",
    "Pin pattern",
    "Supply L+ pin",
    "Supply M pin",
    "Allowed slots",
    "Most modules",
];

/// A step of typing into the form: a field and its text, or a kind chosen.
enum Step {
    Kind,
    Type(&'static str, &'static str),
}

/// A module's form, then `steps` one by one: after each, every field of the section stands where it stood when the
/// section was shown, though a hint or a message comes or goes.
fn fields_stand(steps: &[Step]) {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    open_new_part(&mut d);
    press(&mut d, "PLC section");
    press(&mut d, "PLC role");
    press(&mut d, "Module");
    // From the PLC role's field: a click on a field low in the form may scroll it into sight, which moves nothing in it.
    let places = |d: &Driver| {
        let top = y_of(d, "PLC role").unwrap_or_default();
        FIELDS.map(|f| y_of(d, f).map(|y| y - top))
    };
    let shown = places(&d);
    assert!(shown.iter().all(Option::is_some), "{shown:?}");
    for step in steps {
        let what = match step {
            Step::Kind => {
                press(&mut d, "Channel kind");
                press(&mut d, "DI");
                "the kind"
            }
            Step::Type(field, text) => {
                fill(&mut d, field, text);
                *field
            }
        };
        assert_eq!(places(&d), shown, "after {what}");
    }
}

#[test]
fn the_fields_stand_while_kind_count_and_supply_pins_are_typed_from_the_top() {
    fields_stand(&[
        Step::Kind,
        Step::Type("Channels", "8"),
        Step::Type("Supply L+ pin", "1L+, 2L+"),
        Step::Type("Supply M pin", "M, M2, M3"),
    ]);
}

#[test]
fn the_fields_stand_while_count_kind_and_supply_pins_are_typed_from_below() {
    fields_stand(&[
        Step::Type("Channels", "8"),
        Step::Kind,
        Step::Type("Supply M pin", "M"),
        Step::Type("Supply L+ pin", "L+"),
        Step::Type("Allowed slots", "x"),
    ]);
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
            .query_all_by_label("Maker")
            .last()
            .expect("the form's first field");
        (node.rect().min.y, node.accesskit_node().is_focused())
    };
    open(&mut h);
    let (top, focused) = first(&h);
    assert!(focused, "the first field has the keys");
    // The bus interfaces: the preset's row taken away (a card has none); then the PLC section scrolled down, and
    // Cancel while the form stands there.
    h.get_by_label("Network ports section").click();
    settle(&mut h);
    h.get_by_label("Remove interface X1").click();
    settle(&mut h);
    h.get_by_label("PLC section").click();
    settle(&mut h);
    let role = |h: &H| h.query_all_by_label("PLC role").last().map(|n| n.rect().min.y);
    let before = role(&h).expect("the PLC role");
    h.hover_at(egui::pos2(960.0, 600.0));
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, -20.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut h);
    assert!(role(&h).is_none_or(|y| y < before), "the section scrolled");
    dispatch(&mut h, "parts.back", json!({ "discard": true }));
    settle(&mut h);
    assert!(h.state().shell().dialog().is_none(), "the form is closed");
    open(&mut h);
    assert_eq!(
        first(&h),
        (top, true),
        "the form opens at its top, on its first field, in its first section"
    );
    h.state_mut().shell_mut().shutdown();
}
