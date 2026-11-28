// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Whether the door closes is said (doc 12 §7), from the closed-door check's own judgement: a device in the door
//! without a depth (his own pilot lamp, made here without one) is not judged, and the door's page says so with how
//! many and Edit part… on the first; a check that judged a door whole and found it clear says that it closes.

use crate::common::{Tmp, at, done, own_of};

use ew_viewmodel::command_bar::hud;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// How many devices the door's page says the check did not judge, from its words ("… for 7 devices …").
fn unjudged(s: &ShellModel) -> usize {
    let prompt = hud(s).prompt;
    let rest = prompt
        .strip_prefix("The closed door is not judged for ")
        .unwrap_or_else(|| panic!("{prompt}"));
    rest.split(' ')
        .next()
        .and_then(|n| n.parse().ok())
        .expect("a count")
}

#[test]
fn the_door_says_whether_it_closes() {
    let tmp = Tmp::new("door-closes");
    let mut s = tmp.sample_in(Mode::Gui, true, "904");
    done(&mut s, "select.set", json!({"ids": []}));
    let before = unjudged(&s);
    let edit = hud(&s)
        .buttons
        .into_iter()
        .find(|b| b.action == "parts.open")
        .expect("Edit part… on the first");
    assert_eq!(edit.label, "Edit part\u{2026}");
    assert_eq!(edit.params["field"], "mech.depth");

    // His own pilot lamp, made without a depth, on a new device of the cabinet placed in its door.
    own_of(&mut s, "EW-GEN-LMP-1", "PL-NO-DEPTH");
    done(&mut s, "parts.open", json!({"part": "PL-NO-DEPTH"}));
    done(&mut s, "parts.field", json!({"field": "mech.depth", "value": ""}));
    done(&mut s, "parts.save", json!({}));
    if s.properties().card().is_some() {
        done(&mut s, "preview.apply", json!({}));
    }
    done(&mut s, "parts.back", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "cursor.set", json!({"x_mm": 200.0, "y_mm": 80.0}));
    done(&mut s, "tool.place", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"part": "PL-NO-DEPTH", "letter": "H"}),
    );
    done(&mut s, "canvas.activate", json!({"alt": true}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "page.goto", json!({"folio": "904"}));
    let tables = s.panel_tables().expect("904");
    let lamp = tables
        .unplaced
        .iter()
        .find(|u| u.part_number.as_deref() == Some("PL-NO-DEPTH"))
        .expect("the new lamp waits for the door")
        .device;
    let middle = {
        let c = s.canvas().expect("the door's page");
        let (w, h) = c.sheet();
        (w / 2, h / 2)
    };
    done(&mut s, "tool.place_layout", json!({"device": lamp.to_string()}));
    done(&mut s, "canvas.pointer", at(middle, json!({})));
    done(&mut s, "canvas.release", at(middle, json!({})));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "select.set", json!({"ids": []}));
    assert_eq!(
        unjudged(&s),
        before + 1,
        "his lamp is not judged: it has no depth"
    );

    // The field cabinet's door, with nothing behind it: judged whole and clear, the check says it closes.
    done(
        &mut s,
        "page.new",
        json!({"folio": "980", "type": "panel_layout", "title": "Field"}),
    );
    done(&mut s, "page.goto", json!({"folio": "980"}));
    done(&mut s, "select.set", json!({"ids": []}));
    done(&mut s, "props.set", json!({"row": "page/scale", "value": "1:10"}));
    done(
        &mut s,
        "layout.enclosure_new",
        json!({"location": "+FLD", "part": "EW-GEN-ENC-1"}),
    );
    done(&mut s, "layout.surface_page", json!({"surface": "door"}));
    done(&mut s, "messages.check", json!({}));
    let toast = s.toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.ends_with("closes over what stands behind it."), "{toast}");
    s.shutdown();
}
