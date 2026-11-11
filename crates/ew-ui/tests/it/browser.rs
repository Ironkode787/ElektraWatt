// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The owner's hands-on test of 0.1.0 by the window's own path (clicks and keys): the device browser opens by its
//! button, a category of its tree lists that category alone, a double click on a part places it (FT-1636); the browser
//! and every other floating overlay close by their close button and by a click beside them (FT-1635).

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::shell::{Outcome, ShellModel};
use ew_ui::ew_viewmodel::tools::{Node, PickerRow};
use ew_ui::visual::{Driver, Input};
use serde_json::json;

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

/// A point of the window beside every overlay: the left rail, half way down.
const BESIDE: (i64, i64) = (4, 500);

/// The centre of the first widget whose accessible name starts with `prefix`.
fn starting_with(d: &Driver, prefix: &str) -> (i64, i64) {
    d.labels()
        .into_iter()
        .find(|(l, _)| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no {prefix:?} among {:?}", d.labels()))
        .1
}

/// A driver over a new project from the built-in template, whose starter parts the browser lists.
fn new_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

#[test]
fn the_device_browser_opens_by_its_button_browses_a_category_places_by_a_double_click_and_closes() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    assert!(
        d.shell_mut().picker().is_some(),
        "the Device button opens the browser"
    );
    // A category of the tree, clicked, lists its parts alone.
    d.input(&click(starting_with(&d, "Overcurrent protection,")));
    d.settle();
    let picker = d.shell_mut().picker().expect("the browser").clone();
    assert_eq!(picker.node, Node::Category("ProtectionOvercurrent".to_owned()));
    assert!(!picker.rows.is_empty());
    for row in &picker.rows {
        let PickerRow::Part(p) = row else {
            panic!("{row:?} in Device")
        };
        assert_eq!(picker.catalogue.parts[*p].category, "ProtectionOvercurrent");
    }
    // A double click on a part places it: the Place tool floats its main function.
    let PickerRow::Part(p) = picker.rows[0] else {
        unreachable!()
    };
    let name = picker.catalogue.parts[p].description.clone();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, &name),
        mods: Modifiers::NONE,
    });
    d.settle();
    assert!(d.shell_mut().picker().is_none(), "the browser closes");
    assert_eq!(d.shell_mut().tool().name(), "place");
    d.input(&Input::Key {
        key: Some(egui::Key::Escape),
        mods: Modifiers::NONE,
        repeat: 1,
    });
    d.settle();
    // Closed by its X and by a click on the backdrop beside it, for Device and for Symbol.
    for button in ["Device", "Symbol"] {
        for close in [None, Some(BESIDE)] {
            d.input(&click(d.find(button).unwrap()));
            d.settle();
            assert!(d.shell_mut().picker().is_some(), "{button} opens it");
            let at = close.unwrap_or_else(|| d.find("Close").expect("the close button"));
            d.input(&click(at));
            d.settle();
            assert!(d.shell_mut().picker().is_none(), "{button}: closed at {at:?}");
            assert_eq!(
                d.shell_mut().tool().name(),
                "select",
                "the click reached nothing under it"
            );
        }
    }
    d.shell_mut().shutdown();
}

/// The command palette, the junction editor, the text popup, the Join chooser and the notifications log close by a
/// click beside them, as Escape closes them; those with a title row by their close button too.
#[test]
fn every_floating_overlay_closes_by_a_click_beside_it_and_by_its_close_button() {
    let tmp = Tmp::new();
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    type Open = fn(&mut ShellModel);
    type IsOpen = fn(&ShellModel) -> bool;
    let overlays: [(&str, Open, IsOpen, bool); 5] = [
        (
            "palette",
            |s| {
                let _ = s.dispatch("tools.palette", &json!({}));
            },
            |s| s.palette().is_some(),
            true,
        ),
        (
            "junction editor",
            |s| {
                let _ = s.dispatch("page.goto", &json!({"folio": "102"}));
                let j = s.canvas().unwrap().page().tables.junctions[0].id.to_string();
                let _ = s.dispatch("select.set", &json!({ "ids": [j] }));
                let _ = s.dispatch("junction.open", &json!({}));
            },
            |s| s.junction_editor().is_some(),
            true,
        ),
        (
            "text popup",
            |s| {
                let _ = s.dispatch("page.goto", &json!({"folio": "101"}));
                for (action, params) in [
                    ("tool.text", json!({})),
                    ("cursor.set", json!({ "x_mm": 100.0, "y_mm": 100.0 })),
                    ("canvas.activate", json!({})),
                ] {
                    let _ = s.dispatch(action, &params);
                }
            },
            |s| s.text_popup().is_some(),
            true,
        ),
        (
            "join chooser",
            |s| {
                let _ = s.dispatch("page.goto", &json!({"folio": "101"}));
                let symbol = json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"});
                let _ = s.dispatch("place.pick", &symbol);
                let _ = s.dispatch("place.join", &json!({}));
            },
            |s| s.chooser().is_some(),
            false,
        ),
        (
            "notifications log",
            |s| {
                let _ = s.dispatch("notifications.show", &json!({"on": true}));
            },
            |s| s.notifications().popup_open(),
            false,
        ),
    ];
    for (name, open, is_open, titled) in overlays {
        let closes: &[Option<(i64, i64)>] = if titled {
            &[Some(BESIDE), None]
        } else {
            &[Some(BESIDE)]
        };
        for close in closes {
            open(d.shell_mut());
            d.settle();
            // The overlay fades in over `motion.base` (an area shown again takes its frames).
            d.input(&Input::Wait { ms: 300 });
            assert!(is_open(d.shell_mut()), "{name} opens");
            let at = close.unwrap_or_else(|| {
                d.find("Close")
                    .unwrap_or_else(|| panic!("{name}: no close button"))
            });
            d.input(&click(at));
            d.settle();
            assert!(!is_open(d.shell_mut()), "{name}: closed at {at:?}");
            // Back to the Select tool at rest for the next one.
            let _ = d.shell_mut().dispatch("tool.select", &json!({}));
            d.settle();
        }
    }
    d.shell_mut().shutdown();
}
