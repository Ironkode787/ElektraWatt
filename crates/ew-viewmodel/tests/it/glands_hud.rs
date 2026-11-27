// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The HUD of a panel page counts the cables' glands that wait (doc 12 §3.9): on the bottom's page with its button
//! Place cable glands, on the plate's with Bottom; a page of a door or a wall never says the plate's first steps (the
//! location sentence the bottom's page said with its glands placed).

use crate::common;

use common::{Tmp, at, done};
use ew_viewmodel::command_bar::hud;
use ew_viewmodel::shell::Mode;
use serde_json::json;

const MM: i128 = 1_000_000;

#[test]
fn the_hud_counts_the_glands_that_wait_and_offers_their_page_or_their_placing() {
    let tmp = Tmp::new("glands-hud");
    let mut s = tmp.sample_in(Mode::Gui, true, "905");
    let h = hud(&s);
    assert!(
        !h.prompt.contains("location"),
        "the bottom's page with its glands placed: {}",
        h.prompt
    );
    assert!(h.buttons.is_empty(), "{:?}", h.buttons);

    // The two glands taken off the bottom wait again: a band around the row at 80 and 115 mm, 125 mm up (the bottom's
    // datum is its frame's corner), then Delete.
    let t = s.panel_tables().expect("905's tables");
    let sheet = |u: i128, v: i128| s.canvas().unwrap().page().snapper.page_to_sheet(t.to_page((u * MM, v * MM)));
    let (from, to) = (sheet(50, 155), sheet(145, 95));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_eq!(s.selection().items.len(), 2, "the two glands");
    done(&mut s, "edit.delete_selection", json!({}));
    let h = hud(&s);
    assert_eq!(h.prompt, "2 glands to place");
    let place: Vec<_> = h.buttons.iter().map(|b| (b.label.as_str(), b.action)).collect();
    assert_eq!(place, [("Place cable glands", "layout.glands_place")]);
    done(&mut s, "page.goto", json!({"folio": "902"}));
    let h = hud(&s);
    assert_eq!(h.prompt, "2 glands to place");
    assert_eq!(h.buttons[0].label, "Bottom");
    assert_eq!(h.buttons[0].action, "layout.surface_page");

    // EW-LAY-018's Place on the bottom, a window action, goes to the bottom's page without a card.
    done(&mut s, "messages.show", json!({"on": true}));
    let key = s
        .nav(ew_viewmodel::navigators::NavKind::Messages)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| {
                    n.role == ew_viewmodel::navigators::NavRole::Finding
                        && n.cells[1] == "EW-LAY-018"
                        && n.cells[3].ends_with("-101W1")
                })
                .map(|n| n.key.clone())
        })
        .expect("EW-LAY-018 on -101W1");
    done(&mut s, "messages.fix", json!({"key": key}));
    assert_eq!(s.current_folio(), Some("905"));
    assert!(s.properties().card().is_none());

    // The button places them in a row on the bottom.
    done(&mut s, "layout.glands_place", json!({}));
    let h = hud(&s);
    assert!(!h.prompt.contains("glands to place"), "{}", h.prompt);
    s.shutdown();
}

/// The glands that wait are rows of Unplaced devices under Glands; dropped at a sheet point (as the canvas drops a
/// row) the second snaps to the first; the cable's Properties name its gland, proposed from its diameter, and its
/// hole; a gland of the project typed there is chosen.
#[test]
fn unplaced_lists_the_glands_a_drop_places_them_and_the_cable_names_its_gland() {
    let tmp = Tmp::new("glands-drop");
    let mut s = tmp.sample_in(Mode::Gui, true, "905");
    let t = s.panel_tables().expect("905's tables");
    let sheet = |u: i128, v: i128| s.canvas().unwrap().page().snapper.page_to_sheet(t.to_page((u * MM, v * MM)));
    let (from, to, near) = (sheet(50, 155), sheet(145, 95), sheet(112, 131));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    done(&mut s, "canvas.release", at(to, json!({})));
    done(&mut s, "edit.delete_selection", json!({}));

    done(&mut s, "navigator.show", json!({"kind": "unplaced"}));
    let rows = common::nav_rows(&s, "Unplaced devices");
    for tag in ["Glands", "-101W1", "-103W1"] {
        assert!(rows.iter().any(|r| r.contains(tag)), "{tag}: {rows:?}");
    }

    done(
        &mut s,
        "layout.glands_place",
        json!({"cables": "tag:-101W1", "x": "80", "y": "125"}),
    );
    done(
        &mut s,
        "layout.glands_place",
        at(near, json!({"cables": "tag:-103W1"})),
    );
    assert!(!hud(&s).prompt.contains("glands to place"));

    done(&mut s, "select.set", json!({"refs": ["tag:-103W1"]}));
    let (part, _) = row_value(&s, "device/derived.gland");
    assert_eq!(part, "EW-GEN-GLD-M25");
    let (hole, _) = row_value(&s, "device/derived.gland_hole");
    assert_eq!(hole, "H2", "the second of the bottom's row, 35 mm to the right of the first");
    done(
        &mut s,
        "props.set",
        json!({"row": "device/derived.gland", "value": "EW-GEN-GLD-M32"}),
    );
    assert_eq!(row_value(&s, "device/derived.gland").0, "EW-GEN-GLD-M32");
    s.shutdown();
}

/// The value text and the JSON of row `key` of the window's property sheet.
fn row_value(shell: &ew_viewmodel::shell::ShellModel, key: &str) -> (String, serde_json::Value) {
    let sheet = shell.properties().sheet().expect("a sheet");
    let row = sheet
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .find(|r| r.key == key)
        .unwrap_or_else(|| panic!("no row {key}"));
    let json = match &row.value {
        ew_viewmodel::properties::RowValue::One { json, .. } => json.clone(),
        other => panic!("{key}: {other:?}"),
    };
    (ew_viewmodel::properties::value_text(row), json)
}
