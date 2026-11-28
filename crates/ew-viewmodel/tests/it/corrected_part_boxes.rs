// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A drawn box follows its corrected part (doc 10 §2.6): a switch of one's own drawn as a box with its six sockets,
//! corrected to eight, shows the two new sockets on its box at once, after the last one of their edge; corrected to
//! four, its box keeps no mark of the sockets it lost. A box that would grow past the drawing area goes back inside it,
//! and the Save's card says so (FT-2208). The same switch on a bus page is its bus symbol, which the part draws anew:
//! the card names the sockets it gains (doc 10 §2.7).

use crate::common::{Tmp, click_at, cursor, done, grid_point, new_project, tables};
use ew_viewmodel::ew_api::output::TableBox;
use ew_viewmodel::properties::CardOrigin;
use ew_viewmodel::shell::ShellModel;
use serde_json::json;

/// The switch's box on the page shown, its sockets (index, point, direction) by index.
fn sockets(s: &ShellModel) -> TableBox {
    let t = tables(s);
    let mut b = t
        .reps
        .iter()
        .find_map(|r| r.black_box.clone())
        .expect("the switch's box");
    b.cps.sort_by_key(|c| c.0);
    b
}

/// Edit part… on SW-6, its port count set to `ports`, saved: the card's lines of the boxes drawn anew.
fn save_corrected(s: &mut ShellModel, ports: &str) -> Vec<String> {
    done(s, "parts.open", json!({"part": "SW-6"}));
    done(s, "parts.field", json!({"field": "bus.0.ports", "value": ports}));
    done(s, "parts.save", json!({}));
    match s.properties().card().map(|c| &c.origin) {
        Some(CardOrigin::Part(card)) => card.boxes.clone(),
        other => panic!("the part's card: {other:?}"),
    }
}

/// [`save_corrected`], and the update applied.
fn correct(s: &mut ShellModel, ports: &str) {
    save_corrected(s, ports);
    done(s, "preview.apply", json!({}));
    done(s, "page.goto", json!({"folio": "101"}));
}

/// A new project with a switch of one's own, SW-6 (one PROFINET interface of six sockets), and a bus page 102; the
/// switch drawn as a box by two corners on page 101 (a part without a symbol of its own on a schematic page).
fn switch_drawn(tmp: &Tmp) -> ShellModel {
    let mut s = new_project(tmp);
    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "bus_topology", "title": "Network"}),
    );
    done(&mut s, "parts.new", json!({"start": "box"}));
    for (field, value) in [("manufacturer", "Example"), ("part_number", "SW-6")] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.row", json!({"table": "bus"}));
    for (field, value) in [
        ("bus.0.interface", "X1"),
        ("bus.0.bus", "profinet"),
        ("bus.0.role", "passive"),
        ("bus.0.ports", "6"),
    ] {
        done(&mut s, "parts.field", json!({"field": field, "value": value}));
    }
    done(&mut s, "parts.save", json!({}));
    done(&mut s, "page.goto", json!({"folio": "101"}));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "SW-6"}));
    for k in [(10, 10), (22, 16)] {
        let p = grid_point(&s, k);
        cursor(&mut s, p);
        done(&mut s, "canvas.activate", json!({}));
    }
    done(&mut s, "page.goto", json!({"folio": "101"}));
    s
}

#[test]
fn a_corrected_part_s_drawn_box_takes_the_sockets_it_has_now() {
    let tmp = Tmp::new("corrected-part-sockets");
    let mut s = switch_drawn(&tmp);
    let six = sockets(&s).cps;
    assert_eq!(six.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5]);
    // Eight: the two new sockets stand at once, after socket 6 on its edge, leaving as it does, on the box.
    correct(&mut s, "8");
    let TableBox {
        cps: eight, outline, ..
    } = sockets(&s);
    assert_eq!(
        eight.iter().map(|c| c.0).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6, 7]
    );
    let kept = |i: usize| six.iter().find(|c| c.0 == eight[i].0).map(|c| c.1);
    assert!(
        (0..6).all(|i| kept(i) == Some(eight[i].1)),
        "the six keep their places"
    );
    let last = six[5];
    for new in &eight[6..] {
        assert_eq!(new.2, last.2, "{new:?} leaves as socket 6 does");
        assert_eq!(new.1.1, last.1.1, "{new:?} on socket 6's edge");
        assert!(new.1.0 > last.1.0, "{new:?} after socket 6");
    }
    let right = outline.iter().map(|p| p.0).max().unwrap_or(0);
    assert!(eight[7].1.0 < right, "the box grew to take them");
    let points: std::collections::BTreeSet<(i64, i64)> = eight.iter().map(|c| c.1).collect();
    assert_eq!(points.len(), 8, "no two sockets on one point");
    // Four: the sockets it lost leave the box.
    correct(&mut s, "4");
    let four = sockets(&s).cps;
    assert_eq!(four.iter().map(|c| c.0).collect::<Vec<_>>(), [0, 1, 2, 3]);
}

/// The switch moved to stand one grid step from the drawing area's right edge, then corrected to sixteen sockets: its
/// box would grow past the area, so it goes back left inside it with its sockets in their order, and the Save's card
/// says so before anything changes.
#[test]
fn a_box_that_would_grow_past_the_drawing_area_goes_back_inside() {
    let tmp = Tmp::new("corrected-part-inside");
    let mut s = switch_drawn(&tmp);
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("a frame grid");
    let step = grid_point(&s, (1, 0)).0 - grid_point(&s, (0, 0)).0;
    let rep = {
        let t = tables(&s);
        t.reps
            .iter()
            .find(|r| r.black_box.is_some())
            .expect("the switch's box")
            .clone()
    };
    let right = |b: &TableBox| b.outline.iter().map(|p| p.0).max().unwrap_or(0);
    let steps = (area[2] - right(&sockets(&s))) / step - 1;
    let moved = json!({"reps": [rep.id.to_string()], "steps": [steps, 0]});
    done(&mut s, "command.run", json!({"cmd": "rep.move", "params": moved}));
    let before = sockets(&s);
    assert!(right(&before) < area[2], "the box inside the area");
    let card = save_corrected(&mut s, "16");
    assert_eq!(card.len(), 1, "{card:?}");
    assert!(
        card[0].starts_with("Drawn anew: ") && card[0].ends_with(", moved back inside the drawing area"),
        "{card:?}"
    );
    done(&mut s, "preview.apply", json!({}));
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let after = sockets(&s);
    assert_eq!(after.cps.len(), 16);
    assert!(right(&after) <= area[2], "{} past {}", right(&after), area[2]);
    let left = |b: &TableBox| b.outline.iter().map(|p| p.0).min().unwrap_or(0);
    assert!(left(&after) < left(&before), "the box went back left");
    let shift = after.cps[0].1.0 - before.cps[0].1.0;
    assert!(
        (0..6).all(|i| after.cps[i].1 == (before.cps[i].1.0 + shift, before.cps[i].1.1)),
        "the six keep their places on the box"
    );
}

/// The switch placed on bus page 102 stands as its bus symbol; corrected to eight sockets, the Save's card names the two
/// it gains, and after Apply the symbol draws all eight.
#[test]
fn a_corrected_part_s_bus_symbol_gains_its_sockets_and_the_card_says_so() {
    let tmp = Tmp::new("corrected-part-symbol");
    let mut s = switch_drawn(&tmp);
    done(&mut s, "page.goto", json!({"folio": "102"}));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "SW-6"}));
    let p = grid_point(&s, (10, 10));
    cursor(&mut s, p);
    done(&mut s, "canvas.activate", json!({}));
    let drawn = |s: &ShellModel| tables(s).cps.iter().filter(|c| c.label.contains(":P")).count();
    assert_eq!(drawn(&s), 6, "its six sockets");
    let card = save_corrected(&mut s, "8");
    assert!(
        card.iter()
            .any(|l| l.ends_with("on page 102, its bus symbol with X1 P7, X1 P8 added")),
        "{card:?}"
    );
    done(&mut s, "preview.apply", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert_eq!(drawn(&s), 8, "its eight sockets");
}

/// Two switches SW-6 on bus page 102 joined by a bus line at their P4s, which move from the bottom edge to the top one
/// when the part has eight sockets: the Save's card names each device whose symbol is drawn anew, and after Apply the
/// line ends at both new sockets.
#[test]
fn the_save_card_names_each_device_whose_bus_line_follows_its_socket() {
    let tmp = Tmp::new("corrected-part-lines");
    let mut s = switch_drawn(&tmp);
    for k in [(10, 10), (50, 10)] {
        done(&mut s, "page.goto", json!({"folio": "102"}));
        done(&mut s, "tool.place", json!({}));
        done(&mut s, "place.pick", json!({"part": "SW-6"}));
        let p = grid_point(&s, k);
        click_at(&mut s, p);
    }
    done(&mut s, "page.goto", json!({"folio": "102"}));
    // Each switch's P4 socket: its device's tag and its point.
    let p4 = |s: &ShellModel| -> Vec<(String, (i64, i64))> {
        let mut out: Vec<(String, (i64, i64))> = tables(s)
            .cps
            .iter()
            .filter_map(|c| Some((c.label.strip_suffix(":P4")?.to_owned(), c.at)))
            .collect();
        out.sort();
        out
    };
    let before = p4(&s);
    assert_eq!(before.len(), 2, "{before:?}");
    done(&mut s, "tool.wire", json!({}));
    for (_, at) in &before {
        click_at(&mut s, *at);
    }
    done(&mut s, "ui.dismiss", json!({}));
    let card = save_corrected(&mut s, "8");
    for (tag, _) in &before {
        assert!(
            card.iter()
                .any(|l| l.starts_with(&format!("Drawn anew: {tag} on page 102"))),
            "{tag} on the card: {card:?}"
        );
    }
    done(&mut s, "preview.apply", json!({}));
    done(&mut s, "page.goto", json!({"folio": "102"}));
    let after = p4(&s);
    assert!(
        after.iter().zip(&before).all(|(a, b)| a.0 == b.0 && a.1 != b.1),
        "both P4s moved: {before:?} {after:?}"
    );
    let t = tables(&s);
    let line = t
        .connections
        .iter()
        .find(|c| c.kind == "bus")
        .expect("the bus line");
    let ends = [line.polyline.first(), line.polyline.last()];
    for (_, at) in &after {
        assert!(ends.contains(&Some(at)), "{:?} ends at {at:?}", line.polyline);
    }
}
