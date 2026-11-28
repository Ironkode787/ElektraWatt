// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A device on a bus-topology page (doc 10 §2.7): placed, it stands as one bus symbol, inside the drawing area and
//! off another device; a page drawn by an earlier version, with a box per interface, keeps its boxes moving together
//! until Draw as one symbol puts the device's symbol in their place, its bus line keeping its socket, in one undo
//! step.

use crate::common::{Tmp, actual_size, at, cursor, done, grid_point, refused, sheet, tables, toast};
use ew_viewmodel::ew_api::output::{TableRep, Uuid};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::{Value, json};

/// The bus symbols of the page shown, by device: representations of the built-in `ew.gen.bus`.
fn symbols(s: &ShellModel) -> Vec<TableRep> {
    tables(s)
        .reps
        .iter()
        .filter(|r| r.symbol_key.starts_with("ew.gen.bus") && r.device.is_some())
        .cloned()
        .collect()
}

/// The boxes of the page shown that draw a device's interface (an earlier version's form).
fn boxes(s: &ShellModel) -> Vec<TableRep> {
    tables(s)
        .reps
        .iter()
        .filter(|r| r.black_box.is_some() && r.rep_type == "topology" && r.device.is_some())
        .cloned()
        .collect()
}

/// The page point `k` grid steps from `p` along x and `j` along y.
fn steps(s: &ShellModel, p: (i64, i64), k: i64, j: i64) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (
        sn.page_of_index(sn.grid_index_of(p.0) + k),
        sn.page_of_index(sn.grid_index_of(p.1) + j),
    )
}

/// The point of the connection point labelled `label` on the page shown.
fn cp_at(s: &ShellModel, label: &str) -> (i64, i64) {
    tables(s)
        .cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("{label}"))
        .at
}

/// A copy of the sample whose page 105 draws -104A1 as an earlier version did: a box for DP, its socket where the
/// symbol's DP socket stands (the bus line ends there), and a box for X1 right of it; open on page 105.
fn page_of_boxes(tmp: &Tmp) -> ShellModel {
    let dir = tmp.sample_copy();

    // What the boxes take from the sample as it is: the symbol's record, its DP socket, the black-box frame's copy.
    let mut s = tmp.sample_in(Mode::Headless, true, "105");
    let symbol = symbols(&s)
        .into_iter()
        .find(|r| r.functions.len() == 2)
        .expect("-104A1's symbol");
    let dp = cp_at(&s, "-104A1:A");
    let (box_pos, x1_pos) = (steps(&s, dp, -2, 0), steps(&s, dp, 4, 0));
    let corner = steps(&s, (0, 0), 4, 3);
    let socket = steps(&s, (0, 0), 2, 0);
    done(&mut s, "page.goto", json!({"folio": "104"}));
    let frame = tables(&s)
        .reps
        .iter()
        .find(|r| r.symbol_key.starts_with("ew.gen.blackbox"))
        .map(|r| r.symbol)
        .expect("the black-box frame");
    s.shutdown();
    let page = std::fs::read_dir(dir.join("pages"))
        .expect("pages")
        .map(|e| e.expect("a page file").path())
        .find(|p| std::fs::read_to_string(p).is_ok_and(|t| t.contains(&symbol.id.to_string())))
        .expect("page 105's file");
    let text = std::fs::read_to_string(&page).expect("page 105");
    let boxed = |n: u128, function: Uuid, pos: (i64, i64)| -> String {
        json!({
            "$k": "representation", "id": Uuid::from_u128(0x019b_76da_ffff_7000_8000_0000_0000_0000 | n).to_string(),
            "functions": [function.to_string()], "page": tables_page(&text), "symbol": frame.to_string(),
            "pos": [pos.0, pos.1], "rep_type": "topology",
            "box": {"outline": [[0, 0], [corner.0, 0], [corner.0, corner.1], [0, corner.1]],
                    "cps": [{"idx": 0, "pos": [socket.0, 0], "dir": "up"}]},
        })
        .to_string()
    };
    let lines: Vec<String> = text
        .lines()
        .flat_map(|l| {
            if l.contains(&format!("\"id\":\"{}\"", symbol.id)) {
                vec![
                    boxed(1, symbol.functions[0], box_pos),
                    boxed(2, symbol.functions[1], x1_pos),
                ]
            } else {
                vec![l.to_owned()]
            }
        })
        .collect();
    std::fs::write(&page, lines.join("\n") + "\n").expect("written");
    let mut s = tmp.sample_in(Mode::Headless, true, "105");
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// The page ID the page file's records name.
fn tables_page(text: &str) -> String {
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find_map(|v| {
            (v["$k"] == "representation")
                .then(|| v["page"].as_str().map(str::to_owned))
                .flatten()
        })
        .expect("a representation's page")
}

/// A device placed on a bus page with one click stands as one bus symbol drawing its port functions, moved into the
/// drawing area where it would run past the edge; over another device it is refused in words, and nothing is placed.
#[test]
fn a_device_placed_on_a_bus_page_is_one_symbol_inside_the_page_and_off_another_device() {
    let tmp = Tmp::new("ports-place-inside");
    let mut s = actual_size(&tmp, "105");
    let area = ew_viewmodel::tools::ip::drawing_area(s.canvas().unwrap().page()).expect("the frame");
    let before = symbols(&s);
    let k = |s: &ShellModel, x: i64| s.canvas().unwrap().page().snapper.grid_index_of(x);
    // Over the sample's controller: refused.
    let held = before
        .iter()
        .find(|r| r.functions.len() == 2)
        .expect("-104A1")
        .body;
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "EW-GEN-CPU-2"}));
    assert_eq!(s.tool().name(), "black_box");
    let over = grid_point(&s, (k(&s, held[0]) + 1, k(&s, held[1]) + 1));
    cursor(&mut s, over);
    // The command's words: the window refuses what a script is refused.
    let words = crate::common::failed(&mut s, "canvas.activate", json!({}));
    assert!(words.contains("another device stands where"), "{words}");
    assert_eq!(symbols(&s).len(), before.len(), "nothing placed");
    done(&mut s, "ui.dismiss", json!({}));
    // Near the right edge, in the empty foot of the page: one click.
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "place.pick", json!({"part": "EW-GEN-CPU-2"}));
    let (right, foot) = (k(&s, area[2]), k(&s, area[3]));
    let near = grid_point(&s, (right - 2, foot - 8));
    cursor(&mut s, near);
    done(&mut s, "canvas.activate", json!({}));
    // A new station opens its station view; the page is shown again.
    done(&mut s, "page.goto", json!({"folio": "105"}));
    let placed: Vec<TableRep> = symbols(&s)
        .into_iter()
        .filter(|r| !before.iter().any(|b| b.id == r.id))
        .collect();
    let [placed] = placed.as_slice() else {
        panic!("one symbol: {placed:?}")
    };
    assert_eq!(placed.functions.len(), 2, "the new controller's DP and X1");
    let edges = area.map(|v| k(&s, v));
    let b = placed.body.map(|v| k(&s, v));
    assert!(
        edges[0] <= b[0] && b[2] <= edges[2] && edges[1] <= b[1] && b[3] <= edges[3],
        "{b:?} inside the drawing area {edges:?}"
    );
}

/// On a page drawn by an earlier version the boxes of a device's interfaces move together: dragging one by three grid
/// steps moves both by them, in one command.
#[test]
fn the_boxes_of_a_device_s_interfaces_move_together() {
    let tmp = Tmp::new("ports-move-together");
    let mut s = page_of_boxes(&tmp);
    let pair = boxes(&s);
    assert_eq!(pair.len(), 2, "-104A1's DP and X1 boxes");
    let outline = &pair[0].black_box.as_ref().expect("a box").outline;
    let (xs, ys) = (outline.iter().map(|p| p.0), outline.iter().map(|p| p.1));
    let centre = (
        (xs.clone().min().unwrap_or(0) + xs.max().unwrap_or(0)) / 2,
        (ys.clone().min().unwrap_or(0) + ys.max().unwrap_or(0)) / 2,
    );
    let to = (steps(&s, centre, 3, 0).0, centre.1);
    let dx = to.0 - centre.0;
    let rev = s.revision();
    let (from, to_sheet) = (sheet(&s, centre), sheet(&s, to));
    done(&mut s, "canvas.press", at(from, json!({})));
    done(&mut s, "canvas.pointer", at(to_sheet, json!({})));
    done(&mut s, "canvas.release", at(to_sheet, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one rep.move");
    let after = boxes(&s);
    for before in &pair {
        let now = after.iter().find(|r| r.id == before.id).expect("each box");
        assert_eq!(
            (now.pos.0 - before.pos.0, now.pos.1 - before.pos.1),
            (dx, 0),
            "each box three steps right"
        );
    }
}

/// Draw as one symbol on a box of -104A1: the device's symbol stands where its first box stood, both boxes go, and the
/// bus line -105W1 keeps its end at the DP socket, which its route follows; the toast says so with Undo, and one undo
/// gives the boxes back. On a device drawn as a symbol already the action is greyed with its reason.
#[test]
fn draw_as_one_symbol_replaces_a_device_s_boxes_and_its_line_follows_the_socket() {
    let tmp = Tmp::new("ports-one-symbol");
    let mut s = page_of_boxes(&tmp);
    let pair = boxes(&s);
    let first = pair.iter().map(|r| r.pos).min().expect("a box");
    done(&mut s, "select.set", json!({"ids": [pair[1].id.to_string()]}));
    done(&mut s, "bus.one_symbol", json!({}));
    assert_eq!(toast(&s), "-104A1 is drawn as one symbol.");
    assert!(boxes(&s).is_empty(), "the boxes are gone");
    let symbol = symbols(&s)
        .into_iter()
        .find(|r| r.functions.len() == 2)
        .expect("-104A1's symbol");
    assert_eq!(symbol.pos, first, "where its first box stood");
    assert_eq!(
        s.selection().items,
        [ew_dl::EntityRef(symbol.id)],
        "the symbol is selected, not the gone boxes"
    );
    let dp = cp_at(&s, "-104A1:A");
    let line = tables(&s)
        .connections
        .iter()
        .find(|c| c.kind == "bus")
        .expect("-105W1")
        .polyline
        .clone();
    assert!(
        line.first() == Some(&dp) || line.last() == Some(&dp),
        "{line:?} ends at {dp:?}"
    );
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(boxes(&s).len(), 2, "one undo step");
    // A symbol selected: greyed with its reason.
    let other = symbols(&s).into_iter().next().expect("-201K1's symbol");
    done(&mut s, "select.set", json!({"ids": [other.id.to_string()]}));
    assert_eq!(refused(&mut s, "bus.one_symbol", json!({})), "reason-not-bus-box");
}
