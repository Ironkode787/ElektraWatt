// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A PLC card's box is laid out in one row whichever way it is placed (doc 10 §3.6): Insert device and Place by two
//! corners,
//! the Black box tool with the part, and Place here from the station view each give the box Draw on a page
//! gives, its channel pins on one edge.

use crate::common::{Tmp, click_at, done, grid_point, new_project, tables};
use ew_viewmodel::ew_api::geometry::Dir;
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// A box's shape apart from where it stands: its outline and its pins, each from its outline's top-left corner, to the
/// ten nanometres (a point of the QElectroTech grid lies a nanometre off its exact place by its index).
type Shape = (Vec<(i64, i64)>, Vec<(u16, (i64, i64), Dir)>);

/// The boxes of the page shown, by representation: their shapes; none while a station's view is in front.
fn boxes(s: &ShellModel) -> Vec<(Uuid, Shape)> {
    if s.canvas().is_none() {
        return Vec::new();
    }
    tables(s)
        .reps
        .iter()
        .filter_map(|r| {
            let b = r.black_box.as_ref()?;
            let x = b.outline.iter().map(|p| p.0).min()?;
            let y = b.outline.iter().map(|p| p.1).min()?;
            let near = |v: i64| (v + 5).div_euclid(10);
            let outline = b.outline.iter().map(|p| (near(p.0 - x), near(p.1 - y))).collect();
            let mut cps: Vec<_> = b
                .cps
                .iter()
                .map(|(i, p, d)| (*i, (near(p.0 - x), near(p.1 - y)), *d))
                .collect();
            cps.sort_by_key(|c| c.0);
            Some((r.id, (outline, cps)))
        })
        .collect()
}

/// The shape of the one box the page shows now that it did not show `before`.
fn new_box(s: &ShellModel, before: &[(Uuid, Shape)]) -> Shape {
    let added: Vec<Shape> = boxes(s)
        .into_iter()
        .filter(|(id, _)| !before.iter().any(|(b, _)| b == id))
        .map(|(_, shape)| shape)
        .collect();
    assert_eq!(added.len(), 1, "one box is placed: {added:?}");
    added.into_iter().next().unwrap_or_default()
}

/// Two clicks at the grid points `a` and `b`: the corners of a box dragged by the tool armed.
fn two_corners(s: &mut ShellModel, a: (i64, i64), b: (i64, i64)) {
    let (pa, pb) = (grid_point(s, a), grid_point(s, b));
    click_at(s, pa);
    click_at(s, pb);
    done(s, "tool.select", json!({}));
    if s.station_tab().is_some() {
        done(s, "ui.dismiss", json!({}));
    }
}

#[test]
fn a_card_placed_any_way_is_laid_out_in_one_row_as_draw_on_a_page_lays_it() {
    let tmp = Tmp::new("card-box-one-row");
    let mut s = new_project(&tmp);
    // Draw on a page: the box every other way must give.
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-CPU-1"}));
    done(&mut s, "plc.rack_rename", json!({"text": "-1A1"}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-DI-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    done(&mut s, "plc.rack_select", json!({"slot": 1}));
    let before = boxes(&s);
    done(&mut s, "plc.module_draw", json!({"folio": "101"}));
    let drawn = new_box(&s, &before);
    let channels: Vec<_> = drawn.1.iter().filter(|c| c.2 != Dir::Left).collect();
    assert!(channels.len() >= 2, "{drawn:?}");
    assert!(
        channels.iter().all(|c| c.2 == Dir::Up),
        "Draw on a page puts an input card's channel pins on its upper edge: {drawn:?}"
    );

    // Insert device and Place: the part has no symbol of its own, so its box is dragged by two corners.
    let before = boxes(&s);
    done(&mut s, "place.pick", json!({"part": "EW-GEN-DI-1"}));
    two_corners(&mut s, (10, 10), (30, 14));
    assert_eq!(new_box(&s, &before), drawn, "Insert device and Place");

    // The Black box tool with the part.
    let before = boxes(&s);
    done(
        &mut s,
        "tool.black_box",
        json!({"part": "EW-GEN-DI-1", "template": 0}),
    );
    two_corners(&mut s, (10, 24), (16, 40));
    assert_eq!(new_box(&s, &before), drawn, "the Black box tool");

    // Place here on the station's second card, not drawn yet.
    done(&mut s, "plc.rack_editor", json!({"station": "tag:-1A1"}));
    done(&mut s, "plc.rack_select", json!({"slot": 2}));
    done(&mut s, "pages.open", json!({"folio": "101"}));
    let before = boxes(&s);
    done(&mut s, "plc.rack_editor", json!({"station": "tag:-1A1"}));
    done(&mut s, "plc.rack_select", json!({"slot": 2}));
    done(&mut s, "plc.module_place", json!({}));
    two_corners(&mut s, (10, 44), (20, 50));
    assert_eq!(new_box(&s, &before), drawn, "Place here");
    s.shutdown();
}

/// A card of one's own starts its device box (Pins and box…, and the box placed from it until the box is edited) as a
/// card's: its channels in one row on one edge, its supply down the left side.
#[test]
fn a_new_card_s_device_box_starts_in_one_row() {
    let tmp = Tmp::new("card-box-one-row-editor");
    let mut s = new_project(&tmp);
    done(&mut s, "parts.new", json!({"start": "module"}));
    for (field, value) in [
        ("manufacturer", "Example"),
        ("part_number", "DIO-8"),
        ("plc.kind", "di"),
        ("plc.count", "4"),
        ("plc.kind2", "do"),
        ("plc.count2", "4"),
        ("plc.l_plus", "L+"),
        ("plc.m", "M"),
    ] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "parts.draw", json!({"what": "box"}));
    let e = s
        .symbol_editor()
        .and_then(|e| e.device_box())
        .expect("the card's device box");
    let b = e.steps();
    let edges: Vec<_> = b.pins.iter().map(|p| p.1.clone()).collect();
    let up = edges
        .iter()
        .filter(|d| **d == ew_viewmodel::ew_api::part_box::Dir4::Up)
        .count();
    let left = edges
        .iter()
        .filter(|d| **d == ew_viewmodel::ew_api::part_box::Dir4::Left)
        .count();
    assert_eq!((up, left, edges.len()), (8, 2, 10), "{edges:?}");
}
