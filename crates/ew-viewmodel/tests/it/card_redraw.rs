// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A PLC card of the older two-row form drawn again in one row in its place (FT-1897, doc 10 §3.6): one
//! `plc.card_redraw` replaces its box with the box of Draw on a page, keeps its device, channels and connections, and
//! is one undo step. The card here is the sample's input card on folio 104 with half its channel pins moved to the
//! lower edge, as the older form drew them.

use crate::common::{Tmp, done, tables};

use ew_api::geometry::Dir;
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

/// The representation of the input card -104A1-A1 on the shown page, by its printed CP labels.
fn card(s: &ShellModel) -> Option<ew_api::output::TableRep> {
    let t = tables(s);
    t.reps
        .iter()
        .find(|r| {
            r.black_box.is_some()
                && t.cps
                    .iter()
                    .any(|c| c.rep == r.id && c.label.starts_with("-104A1-A1:"))
        })
        .cloned()
}

/// The input card made a card of the older form: every second pin of its upper edge moved to its lower edge, leaving
/// downward. The card as it stood before.
fn make_older(s: &mut ShellModel) -> ew_api::output::TableRep {
    let rep = card(s).expect("the input card");
    let b = rep.black_box.clone().unwrap();
    assert!(
        b.cps.iter().filter(|c| c.2 == Dir::Up).count() >= 2,
        "{:?}",
        b.cps
    );
    let (x0, y0) = rep.pos;
    let bottom = b.outline.iter().map(|p| p.1).max().unwrap() - y0;
    let mut up = 0;
    let cps: Vec<serde_json::Value> = b
        .cps
        .iter()
        .map(|(idx, at, dir)| {
            let (x, y) = (at.0 - x0, at.1 - y0);
            if *dir == Dir::Up {
                up += 1;
                if up % 2 == 0 {
                    return json!({"idx": idx, "pos": [x, bottom], "dir": "down"});
                }
            }
            json!({"idx": idx, "pos": [x, y], "dir": dir.word()})
        })
        .collect();
    let outline: Vec<[i64; 2]> = b.outline.iter().map(|p| [p.0 - x0, p.1 - y0]).collect();
    done(
        s,
        "command.run",
        json!({"cmd": "box.reshape", "params": {"rep": rep.id.to_string(), "outline": outline, "cps": cps}}),
    );
    rep
}

#[test]
fn a_card_in_two_rows_is_drawn_again_in_one_row_and_keeps_its_wires() {
    let tmp = Tmp::new("card-redraw");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    let rep = make_older(&mut s);
    let connections = tables(&s).connections.len();
    // Offered only for the older form.
    let older = card(&s).unwrap();
    assert!(
        older
            .black_box
            .as_ref()
            .unwrap()
            .cps
            .iter()
            .any(|c| c.2 == Dir::Down)
    );
    done(
        &mut s,
        "select.set",
        json!({"refs": [format!("id:{}", older.id)]}),
    );
    done(&mut s, "plc.card_redraw", json!({}));
    let new = card(&s).expect("the card drawn again");
    assert_ne!(new.id, older.id, "the old box is replaced");
    assert_eq!(new.pos, rep.pos, "in its place");
    let nb = new.black_box.unwrap();
    assert!(!nb.cps.iter().any(|c| c.2 == Dir::Down), "one row: {:?}", nb.cps);
    assert_eq!(
        tables(&s).connections.len(),
        connections,
        "every connection stays"
    );
    // One undo step gives the older box back.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(card(&s).map(|r| r.id), Some(older.id));
}

/// FT-2083: the card of the older form moved 40 steps right, where its eight inputs in one row would pass the drawing
/// area's right: Draw the card in one row splits the row into boxes of as many pins as fit, one under the other, every
/// box inside the area, and every pin and connection stays on the page.
#[test]
fn a_row_wider_than_the_drawing_area_is_split_into_boxes() {
    let tmp = Tmp::new("card-redraw-wide");
    let mut s = tmp.sample_in(Mode::Headless, true, "104");
    let rep = make_older(&mut s);
    let moved = json!({"reps": [rep.id.to_string()], "steps": [40, 0]});
    done(&mut s, "command.run", json!({"cmd": "rep.move", "params": moved}));
    let (connections, pins) = {
        let t = tables(&s);
        let pins = t.cps.iter().filter(|c| c.label.starts_with("-104A1-A1:")).count();
        (t.connections.len(), pins)
    };
    done(&mut s, "select.set", json!({"refs": [format!("id:{}", rep.id)]}));
    done(&mut s, "plc.card_redraw", json!({}));
    let t = tables(&s);
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("a frame grid");
    let boxes: Vec<_> = t
        .reps
        .iter()
        .filter(|r| {
            t.cps
                .iter()
                .any(|c| c.rep == r.id && c.label.starts_with("-104A1-A1:"))
        })
        .collect();
    assert!(boxes.len() > 1, "split into boxes");
    for r in &boxes {
        let right = r
            .black_box
            .as_ref()
            .unwrap()
            .outline
            .iter()
            .map(|p| p.0)
            .max()
            .unwrap();
        assert!(
            right <= area[2],
            "a box to {right} nm, the area to {} nm",
            area[2]
        );
    }
    let drawn = t.cps.iter().filter(|c| c.label.starts_with("-104A1-A1:")).count();
    assert_eq!(
        (t.connections.len(), drawn),
        (connections, pins),
        "every connection and pin stays"
    );
}
