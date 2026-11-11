// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the view model: the Wire marker tool's crossing order, fields and
//! angle, the Structure box tool's rectangle, polygon and fields, the marker mode action, Escape and Back, the two
//! tools' ghost lines and words, their exact `Session` counts, and the page change that ends every part-4b tool.
//! Every test works on a writable temporary copy of Golden A.

use crate::common;

use std::path::Path;

use common::{
    GOLDEN_A, Tmp, actual_size, assert_failed, at, canvas, click_at, cursor, describe_line, done, grid_point,
    refused, sheet, tables,
};
use ew_viewmodel::actions::{ActionItem, Menu, Submenu, menu_model};
use ew_viewmodel::ew_api::output::{PageTables, TableConnection, TableEnd};
use ew_viewmodel::shell::{Outcome, ShellModel};
use ew_viewmodel::tools::{FineKind, crossed_connections};
use serde_json::{Value, json};

/// A primary press at page point `p`.
fn press(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "canvas.press", at(q, json!({})));
}

/// The pointer to page point `p`.
fn pointer(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "canvas.pointer", at(q, json!({})));
}

/// A primary release at page point `p`.
fn release(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "canvas.release", at(q, json!({})));
}

/// The label of the connection end `e`.
fn end_label(t: &PageTables, e: &TableEnd) -> String {
    match e {
        TableEnd::Cp { function, idx } => t
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map_or_else(String::new, |c| c.label.clone()),
        TableEnd::Junction { .. } => String::new(),
    }
}

/// The connection one of whose ends prints `label`.
fn connection_at_label<'a>(t: &'a PageTables, label: &str) -> &'a TableConnection {
    t.connections
        .iter()
        .find(|c| end_label(t, &c.a) == label || end_label(t, &c.b) == label)
        .unwrap_or_else(|| panic!("no connection ending at {label}"))
}

/// The vertical segments of `c`.
fn vertical_segments(c: &TableConnection) -> Vec<((i64, i64), (i64, i64))> {
    c.polyline
        .windows(2)
        .filter(|w| w[0].0 == w[1].0 && w[0].1 != w[1].1)
        .map(|w| (w[0], w[1]))
        .collect()
}

/// A horizontal crossing line across W4 and W5 (the wires ending at `S51:2` and `S51:4`), from one grid step left
/// of the left wire to one grid step right of the right one, at a y inside both their vertical segments; and the two
/// connections in the line's order.
fn w4_w5_line(s: &ShellModel) -> ((i64, i64), (i64, i64), Vec<String>) {
    let t = tables(s);
    let w4 = connection_at_label(&t, "S51:2");
    let w5 = connection_at_label(&t, "S51:4");
    let v4 = vertical_segments(w4);
    let v5 = vertical_segments(w5);
    // The pair of vertical segments (one of each wire) with the widest shared y range.
    let mut best: Option<(i64, usize, usize)> = None;
    for (i, a) in v4.iter().enumerate() {
        for (j, b) in v5.iter().enumerate() {
            let (alo, ahi) = (a.0.1.min(a.1.1), a.0.1.max(a.1.1));
            let (blo, bhi) = (b.0.1.min(b.1.1), b.0.1.max(b.1.1));
            let lo = alo.max(blo);
            let hi = ahi.min(bhi);
            if hi <= lo {
                continue;
            }
            let overlap = hi - lo;
            if best.is_none_or(|(o, _, _)| overlap > o) {
                best = Some((overlap, i, j));
            }
        }
    }
    let (_, i, j) = best.expect("W4 and W5 share a y range of vertical segments");
    let (a, b) = (v4[i], v5[j]);
    let lo = a.0.1.min(a.1.1).max(b.0.1.min(b.1.1));
    let hi = a.0.1.max(a.1.1).min(b.0.1.max(b.1.1));
    let y = lo + (hi - lo) / 2;
    let (x4, x5) = (a.0.0, b.0.0);
    let sn = &s.canvas().unwrap().page().snapper;
    let (left, right) = (
        sn.page_of_index(sn.grid_index_of(x4.min(x5)) - 1),
        sn.page_of_index(sn.grid_index_of(x4.max(x5)) + 1),
    );
    let order = if x4 < x5 {
        vec![w4.id.to_string(), w5.id.to_string()]
    } else {
        vec![w5.id.to_string(), w4.id.to_string()]
    };
    ((left, y), (right, y), order)
}

/// The marker point the tool computes for a crossing from `from` to `to`: the midpoint, fine-snapped as a text.
fn expected_marker_at(s: &ShellModel, from: (i64, i64), to: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    let a = sn.sheet_to_page(sn.page_to_sheet(from));
    let b = sn.sheet_to_page(sn.page_to_sheet(to));
    let mid = (
        ((i128::from(a.0) + i128::from(b.0)).div_euclid(2)) as i64,
        ((i128::from(a.1) + i128::from(b.1)).div_euclid(2)) as i64,
    );
    sn.snap_fine(sn.page_to_sheet(mid), FineKind::Text, false).1
}

/// Draws the crossing line across W4 and W5 with the pointer and releases it.
fn cross_w4_w5(s: &mut ShellModel) -> ((i64, i64), (i64, i64), Vec<String>) {
    let (from, to, order) = w4_w5_line(s);
    press(s, from);
    assert_eq!(s.tool().state(), "crossing");
    pointer(s, to);
    assert_eq!(canvas(s)["ghost"]["connections"], 2, "the crossed wires");
    release(s, to);
    (from, to, order)
}

/// G-PP blind run, draft row 13: a press and a release at another point with no pointer move between take the
/// release point as the end of the line, so the crossed wires go to typing in line order.
#[test]
fn a_press_and_a_release_without_moves_mark_the_crossed_wires() {
    let tmp = Tmp::new("marker-press-release");
    let mut s = actual_size(&tmp, "101");
    done(&mut s, "tool.marker", json!({}));
    let (from, to, order) = w4_w5_line(&s);
    press(&mut s, from);
    release(&mut s, to);
    assert_eq!(s.tool().state(), "typing");
    assert_eq!(canvas(&s)["ghost"]["connections"], 2, "the crossed wires");
    done(&mut s, "tool.finish", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "marker.place");
    assert_eq!(last.params["connections"], json!(order));
}

/// A line that meets no wire: below every connection of the page.
fn empty_marker_line(s: &ShellModel) -> ((i64, i64), (i64, i64)) {
    let t = tables(s);
    let max_y = t
        .connections
        .iter()
        .flat_map(|c| c.polyline.iter().map(|p| p.1))
        .max()
        .expect("Golden A has connections");
    let max_x = t
        .connections
        .iter()
        .flat_map(|c| c.polyline.iter().map(|p| p.0))
        .max()
        .expect("Golden A has connections");
    let sn = &s.canvas().unwrap().page().snapper;
    let step = sn.page_of_index(1) - sn.page_of_index(0);
    ((0, max_y + step), (max_x + step, max_y + step))
}

/// The UUID text of the structure node `J-E-206` of Golden A.
fn location_node_uuid() -> String {
    let (session, _) = ew_viewmodel::ew_api::Session::open_read_only(Path::new(GOLDEN_A), "test")
        .expect("Golden A opens read-only");
    let q = session.query();
    q.structure_nodes()
        .iter()
        .find(|(_, node)| {
            serde_json::to_value(&node.aspect).ok() == Some(Value::from("location"))
                && node.code.as_str() == "J-E-206"
        })
        .map(|(id, _)| id.as_uuid().to_string())
        .expect("Golden A has the location node J-E-206")
}

/// The items of the Wire markers submenu.
fn marker_mode_items(s: &ShellModel) -> Vec<ActionItem> {
    // Draw, Connect, Wire markers.
    let draw = menu_model(s)
        .into_iter()
        .find(|m| m.menu == Menu::Draw)
        .expect("the Draw menu");
    draw.submenu(Submenu::WireMarkers)
        .expect("the Wire markers submenu")
        .to_vec()
}

fn checked_mode(s: &ShellModel, mode: &str) -> bool {
    marker_mode_items(s)
        .into_iter()
        .find(|i| i.params["mode"] == mode)
        .unwrap_or_else(|| panic!("the {mode} item"))
        .checked
}

// ---------------------------------------------------------------------------------------------------------------
// PI7: the Wire marker tool

#[test]
fn the_marker_tool_takes_w4_and_w5_in_line_order() {
    let tmp = Tmp::new("marker-w4w5");
    let mut s = actual_size(&tmp, "101");
    done(&mut s, "tool.marker", json!({}));
    assert_eq!(s.tool().name(), "marker");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(
        canvas(&s)["ghost"],
        json!({"kind": "marker", "connections": 0, "text": null})
    );
    let (from, to, order) = cross_w4_w5(&mut s);
    assert_eq!(s.tool().state(), "typing");
    let ghost = canvas(&s)["ghost"].clone();
    assert_eq!(ghost["kind"], "marker");
    assert_eq!(ghost["connections"], 2, "{ghost}");
    assert_eq!(ghost["text"], Value::Null);
    // The `n crossed` label of the crossing overlay names the two wires.
    done(&mut s, "marker.fields", json!({"csa": "2.5"}));
    assert_eq!(canvas(&s)["ghost"]["text"], "2.5");
    let rev = s.revision();
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one marker.place");
    assert_eq!(s.tool().state(), "armed");
    let page = tables(&s).page.to_string();
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "marker.place");
    assert_eq!(last.params["page"], page.as_str());
    assert_eq!(last.params["connections"], json!(order));
    assert!(last.params.get("angle").is_none(), "{}", last.params);
    assert_eq!(last.params["csa"], "2.5");
    assert!(last.params.get("colours").is_none(), "{}", last.params);
    let expected = expected_marker_at(&s, from, to);
    assert_eq!(last.params["at"], json!([expected.0, expected.1]));
    let t = tables(&s);
    assert_eq!(
        t.items.iter().filter(|i| i.kind == "wire_marker").count(),
        1,
        "one wire marker item"
    );
    let marker = t
        .items
        .iter()
        .find(|i| i.kind == "wire_marker")
        .expect("the wire marker");
    assert_eq!(marker.text.as_deref(), Some("2.5 BK BU"));

    // The same line drawn right to left (a fresh copy) lists the two the other way round; the keyboard commit
    // needs a grid cursor even though the pointer drew the line.
    let tmp = Tmp::new("marker-w4w5-rev");
    let mut s = actual_size(&tmp, "101");
    done(&mut s, "tool.marker", json!({}));
    let (from, to, order) = w4_w5_line(&s);
    press(&mut s, to);
    pointer(&mut s, from);
    release(&mut s, from);
    assert_eq!(s.tool().state(), "typing");
    cursor(&mut s, from);
    done(&mut s, "canvas.activate", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    let mut reversed = order.clone();
    reversed.reverse();
    assert_eq!(last.params["connections"], json!(reversed));
}

#[test]
fn a_crossing_of_horizontal_segments_places_the_marker_at_90_degrees() {
    let tmp = Tmp::new("marker-angle");
    let mut s = actual_size(&tmp, "101");
    let t = tables(&s);
    // The horizontal segments of the page, by connection.
    let mut segments: Vec<(usize, i64, i64, i64)> = Vec::new();
    for (i, c) in t.connections.iter().enumerate() {
        for w in c.polyline.windows(2) {
            if w[0].1 == w[1].1 && w[0].0 != w[1].0 {
                segments.push((i, w[0].1, w[0].0.min(w[1].0), w[0].0.max(w[1].0)));
            }
        }
    }
    let sn = &s.canvas().unwrap().page().snapper;
    let mut found: Option<((i64, i64), (i64, i64))> = None;
    'pairs: for a in 0..segments.len() {
        for b in a + 1..segments.len() {
            let (ca, ya, a0, a1) = segments[a];
            let (cb, yb, b0, b1) = segments[b];
            if ca == cb || ya == yb {
                continue;
            }
            let (lo, hi) = (a0.max(b0), a1.min(b1));
            if hi <= lo {
                continue;
            }
            let y0 = sn.page_of_index(sn.grid_index_of(ya.min(yb)) - 1);
            let y1 = sn.page_of_index(sn.grid_index_of(ya.max(yb)) + 1);
            // The keyboard cursor sits on the connection grid.
            for k in sn.grid_index_of(lo) + 1..=sn.grid_index_of(hi) {
                let x = sn.page_of_index(k);
                if x <= lo || x >= hi {
                    continue;
                }
                let hits = crossed_connections(&t, (x, y0), (x, y1));
                if hits.len() == 2 && hits.iter().all(|(_, horizontal)| *horizontal) {
                    found = Some(((x, y0), (x, y1)));
                    break 'pairs;
                }
            }
        }
    }
    let (from, to) = found.expect("two horizontal segments over a common x range");
    // The keyboard: cursor, Enter, cursor, Enter.
    done(&mut s, "tool.marker", json!({}));
    cursor(&mut s, from);
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().state(), "crossing");
    cursor(&mut s, to);
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().state(), "typing");
    let rev = s.revision();
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "marker.place");
    assert_eq!(last.params["angle"], 90_000, "{}", last.params);
    assert_eq!(last.params["connections"].as_array().map(Vec::len), Some(2));
}

#[test]
fn a_crossing_of_no_wire_returns_to_armed() {
    let tmp = Tmp::new("marker-empty");
    let mut s = actual_size(&tmp, "101");
    let (from, to) = empty_marker_line(&s);
    let rev = s.revision();
    done(&mut s, "tool.marker", json!({}));
    press(&mut s, from);
    assert_eq!(s.tool().state(), "crossing");
    assert_eq!(canvas(&s)["ghost"]["connections"], 0);
    pointer(&mut s, to);
    assert_eq!(s.tool().state(), "crossing");
    assert_eq!(canvas(&s)["ghost"]["connections"], 0);
    release(&mut s, to);
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev, "nothing committed");
    assert_eq!(canvas(&s)["ghost"]["connections"], 0);
}

#[test]
fn marker_fields_parse_colours_and_are_refused_outside_the_tool() {
    let tmp = Tmp::new("marker-fields");
    let mut s = actual_size(&tmp, "101");
    assert_eq!(
        refused(&mut s, "marker.fields", json!({"csa": "2.5"})),
        "reason-marker-tool-only"
    );
    done(&mut s, "tool.marker", json!({}));
    assert_failed(&mut s, "marker.fields", json!({}));
    done(&mut s, "marker.fields", json!({"colours": "BK GN+YE"}));
    assert_eq!(canvas(&s)["ghost"]["text"], "BK GN+YE");
    assert_failed(&mut s, "marker.fields", json!({"colours": "A+B+C"}));
    assert_eq!(
        canvas(&s)["ghost"]["text"],
        "BK GN+YE",
        "a refused field changes nothing"
    );
    assert_failed(&mut s, "marker.fields", json!({"colours": "BK++GN"}));
    // The csa is trimmed first: an empty or all-whitespace text clears it.
    done(&mut s, "marker.fields", json!({"csa": " 2.5 "}));
    assert_eq!(canvas(&s)["ghost"]["text"], "2.5 BK GN+YE");
    done(&mut s, "marker.fields", json!({"csa": "   "}));
    assert_eq!(canvas(&s)["ghost"]["text"], "BK GN+YE");
    done(&mut s, "marker.fields", json!({"csa": ""}));
    assert_eq!(canvas(&s)["ghost"]["text"], "BK GN+YE");
    // Two entries for two crossed wires: the committed colours are lists of one and two codes.
    let (from, to, order) = cross_w4_w5(&mut s);
    assert_eq!(s.tool().state(), "typing");
    assert_eq!(canvas(&s)["ghost"]["connections"], 2);
    done(&mut s, "tool.finish", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "marker.place");
    assert_eq!(last.params["connections"], json!(order));
    assert_eq!(last.params["colours"], json!([["BK"], ["GN", "YE"]]));
    let expected = expected_marker_at(&s, from, to);
    assert_eq!(last.params["at"], json!([expected.0, expected.1]));
}

// ---------------------------------------------------------------------------------------------------------------
// PI7: the Structure box tool

#[test]
fn the_structure_box_tool_draws_a_rectangle_with_its_fields() {
    let tmp = Tmp::new("structure-box");
    let mut s = actual_size(&tmp, "101");
    done(&mut s, "tool.structure_box", json!({}));
    assert_eq!(s.tool().name(), "structure_box");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(canvas(&s)["ghost"]["outline"], 0);
    let a = grid_point(&s, (40, 40));
    let b = grid_point(&s, (46, 44));
    click_at(&mut s, a);
    click_at(&mut s, b);
    assert_eq!(canvas(&s)["ghost"]["outline"], 2);
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "typing");
    assert_eq!(canvas(&s)["ghost"]["outline"], 4);
    let node = location_node_uuid();
    let (external, label) = s
        .external_choices()
        .into_iter()
        .next()
        .expect("Golden A has external references");
    done(
        &mut s,
        "structure_box.fields",
        json!({"style": "dash_dot", "caption": "Spare", "location": "+J-E-206",
            "external": label}),
    );
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one box.structure");
    assert_eq!(s.tool().state(), "armed");
    let page = tables(&s).page.to_string();
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "box.structure");
    assert_eq!(last.params["page"], page.as_str());
    assert_eq!(
        last.params["outline"],
        json!([[a.0, a.1], [b.0, a.1], [b.0, b.1], [a.0, b.1]]),
        "top-left first, clockwise"
    );
    assert_eq!(last.params["style"], "dash_dot");
    assert_eq!(last.params["caption"], "Spare");
    assert_eq!(last.params["assigns"], json!({"location": node}));
    assert_eq!(last.params["external"], external);
    let t = tables(&s);
    let b = t
        .structure_boxes
        .iter()
        .find(|b| b.caption.starts_with("Spare"))
        .expect("the structure box");
    assert_eq!(b.style, "dash_dot");
}

#[test]
fn the_structure_box_tool_draws_a_polygon() {
    let tmp = Tmp::new("structure-polygon");
    let mut s = actual_size(&tmp, "101");
    done(&mut s, "tool.structure_box", json!({}));
    let points = [
        grid_point(&s, (40, 20)),
        grid_point(&s, (46, 26)),
        grid_point(&s, (54, 22)),
    ];
    for p in points {
        click_at(&mut s, p);
    }
    assert_eq!(canvas(&s)["ghost"]["outline"], 3);
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "typing");
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "box.structure");
    assert_eq!(
        last.params["outline"],
        json!(points.iter().map(|p| json!([p.0, p.1])).collect::<Vec<_>>())
    );
    assert_eq!(last.params["style"], "dotted");
    assert_eq!(last.params["assigns"], json!({}));
}

#[test]
fn structure_box_fields_refuse_an_unknown_node_and_change_nothing() {
    let tmp = Tmp::new("structure-fields");
    let mut s = actual_size(&tmp, "101");
    assert_eq!(
        refused(&mut s, "structure_box.fields", json!({"caption": "X"})),
        "reason-structure-box-tool-only"
    );
    done(&mut s, "tool.structure_box", json!({}));
    let a = grid_point(&s, (40, 30));
    let b = grid_point(&s, (46, 34));
    click_at(&mut s, a);
    click_at(&mut s, b);
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "typing");
    let before = canvas(&s)["ghost"].clone();
    match s.dispatch(
        "structure_box.fields",
        &json!({"location": "+NOPE", "caption": "X"}),
    ) {
        Outcome::Failed { message } => assert!(message.contains("+NOPE"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(canvas(&s)["ghost"], before, "the outline is unchanged");
    assert_eq!(s.tool().state(), "typing");
    // The refused caption and location were not applied: the commit carries neither.
    done(&mut s, "canvas.activate", json!({}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "box.structure");
    assert!(last.params.get("caption").is_none(), "{}", last.params);
    assert_eq!(last.params["assigns"], json!({}));
}

// ---------------------------------------------------------------------------------------------------------------
// PI7: the marker mode action

#[test]
fn the_marker_mode_action_sets_clears_and_skips_an_equal_mode() {
    let tmp = Tmp::new("marker-mode");
    let mut s = actual_size(&tmp, "101");
    assert_eq!(s.marker_mode_word(), "per_wire");
    assert!(checked_mode(&s, "per_wire"));
    let rev = s.revision();
    done(&mut s, "project.marker_mode", json!({"mode": "none"}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "project.drawing.set");
    assert_eq!(last.params, json!({"marker_mode": "none"}));
    assert_eq!(s.marker_mode_word(), "none");
    assert!(checked_mode(&s, "none"));
    // The same mode again commits nothing.
    let rev = s.revision();
    let steps = s.recorded_commands().len();
    done(&mut s, "project.marker_mode", json!({"mode": "none"}));
    assert_eq!(s.revision(), rev);
    assert_eq!(s.recorded_commands().len(), steps);
    // `per_wire` removes the member.
    done(&mut s, "project.marker_mode", json!({"mode": "per_wire"}));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.params, json!({"marker_mode": null}));
    assert_eq!(s.marker_mode_word(), "per_wire");
    assert!(checked_mode(&s, "per_wire"));
    // An unknown mode fails.
    assert_failed(&mut s, "project.marker_mode", json!({"mode": "some"}));
    assert_failed(&mut s, "project.marker_mode", json!({}));
}

// ---------------------------------------------------------------------------------------------------------------
// PI7: Escape and Back

#[test]
fn escape_and_back_in_the_marker_and_structure_box_tools() {
    let tmp = Tmp::new("marker-escape");
    let mut s = actual_size(&tmp, "101");
    // Escape from crossing.
    done(&mut s, "tool.marker", json!({}));
    let (from, to, _) = w4_w5_line(&s);
    press(&mut s, from);
    assert_eq!(s.tool().state(), "crossing");
    let rev = s.revision();
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "marker");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev);
    // Escape from typing.
    press(&mut s, from);
    pointer(&mut s, to);
    release(&mut s, to);
    assert_eq!(s.tool().state(), "typing");
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev);
    // Escape from armed returns to Select.
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
    // Escape from the structure box's outline, then its typing state.
    done(&mut s, "tool.structure_box", json!({}));
    let a = grid_point(&s, (40, 30));
    let b = grid_point(&s, (46, 34));
    click_at(&mut s, a);
    assert_eq!(s.tool().state(), "outline");
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "structure_box");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev);
    click_at(&mut s, a);
    click_at(&mut s, b);
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "typing");
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev);
    // Back removes the last point of an outline; the last one goes back to armed.
    click_at(&mut s, a);
    click_at(&mut s, b);
    assert_eq!(canvas(&s)["ghost"]["outline"], 2);
    done(&mut s, "tool.back", json!({}));
    assert_eq!(s.tool().state(), "outline");
    assert_eq!(canvas(&s)["ghost"]["outline"], 1);
    done(&mut s, "tool.back", json!({}));
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev, "Back commits nothing");
    // Escape from armed returns to Select.
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
}

// ---------------------------------------------------------------------------------------------------------------
// Transcript lines and exact counts

#[test]
fn the_marker_and_structure_box_ghost_lines_describe_the_tools() {
    let tmp = Tmp::new("marker-words");
    let mut s = actual_size(&tmp, "101");
    done(&mut s, "tool.marker", json!({}));
    assert_eq!(
        describe_line(&s, "canvas ghost marker ").as_deref(),
        Some("canvas ghost marker connections 0")
    );
    let (from, to, _) = w4_w5_line(&s);
    press(&mut s, from);
    assert_eq!(s.tool().state(), "crossing");
    pointer(&mut s, to);
    release(&mut s, to);
    assert_eq!(s.tool().state(), "typing");
    // The marker's line reads as the specification says, also after the fields changed.
    done(&mut s, "marker.fields", json!({"csa": "2.5", "colours": "BK BU"}));
    assert_eq!(
        describe_line(&s, "canvas ghost marker ").as_deref(),
        Some("canvas ghost marker connections 2 text \"2.5 BK BU\"")
    );
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.structure_box", json!({}));
    let a = grid_point(&s, (40, 30));
    let b = grid_point(&s, (46, 34));
    click_at(&mut s, a);
    assert_eq!(s.tool().state(), "outline");
    click_at(&mut s, b);
    assert_eq!(
        describe_line(&s, "canvas ghost box ").as_deref(),
        Some("canvas ghost box outline 2")
    );
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "typing");
    assert!(describe_line(&s, "canvas ghost box outline 4").is_some());
}

/// Twenty cursor and pointer moves around grid point (40, 30) change no `Session` read count.
fn assert_grid_moves_read_nothing(s: &mut ShellModel, what: &str) {
    let p = grid_point(s, (40, 30));
    let calls = s.session_calls();
    for k in 0..20 {
        let c = grid_point(s, (40 + k % 3, 30 + k % 2));
        cursor(s, c);
        let q = sheet(s, (p.0 + k * 300_000, p.1 + k * 200_000));
        done(s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "{what} reads nothing");
}

#[test]
fn pointer_moves_with_the_marker_and_structure_box_tools_call_no_session() {
    let tmp = Tmp::new("marker-counts");
    let mut s = actual_size(&tmp, "101");
    let p = grid_point(&s, (40, 30));
    done(&mut s, "tool.marker", json!({}));
    assert_grid_moves_read_nothing(&mut s, "the armed Wire marker tool");
    let (from, to, _) = w4_w5_line(&s);
    press(&mut s, from);
    assert_grid_moves_read_nothing(&mut s, "the crossing Wire marker tool");
    release(&mut s, to);
    assert_grid_moves_read_nothing(&mut s, "the typing Wire marker tool");
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.structure_box", json!({}));
    assert_grid_moves_read_nothing(&mut s, "the armed Structure box tool");
    click_at(&mut s, p);
    assert_grid_moves_read_nothing(&mut s, "the drawing Structure box tool");
    let b = grid_point(&s, (46, 34));
    click_at(&mut s, b);
    done(&mut s, "tool.finish", json!({}));
    assert_grid_moves_read_nothing(&mut s, "the typing Structure box tool");
}

// ---------------------------------------------------------------------------------------------------------------
// The page change rule (§6.4 item 5)

#[test]
fn a_page_change_ends_every_part_4b_tool() {
    // Text, Graphic, Wire marker and Structure box while armed.
    for tool in ["tool.text", "tool.graphic", "tool.marker", "tool.structure_box"] {
        let tmp = Tmp::new(&format!("page-change-{tool}"));
        let mut s = actual_size(&tmp, "101");
        done(&mut s, tool, json!({}));
        assert_ne!(s.tool().name(), "select", "{tool}");
        let rep = tables(&s).reps[0].id.to_string();
        done(&mut s, "select.set", json!({"ids": [rep]}));
        let selected = s.selection().items.clone();
        done(&mut s, "page.goto", json!({"folio": "102"}));
        assert_eq!(s.tool().name(), "select", "{tool} ends on a page change");
        assert_eq!(s.selection().items, selected, "{tool} keeps the selection");
    }
    // Text edit, reached through `text.edit` on a placed free text.
    let tmp = Tmp::new("page-change-text-edit");
    let mut s = actual_size(&tmp, "101");
    let p = grid_point(&s, (44, 32));
    let page = tables(&s).page.to_string();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "40x60", "at": [p.0, p.1]}}}}),
    );
    let item = tables(&s)
        .items
        .iter()
        .find(|i| i.kind == "free_text" && i.text.as_deref() == Some("40x60"))
        .expect("the free text")
        .id
        .to_string();
    done(&mut s, "select.set", json!({"ids": [item]}));
    done(&mut s, "text.edit", json!({}));
    assert_eq!(s.tool().name(), "text_edit");
    done(&mut s, "page.goto", json!({"folio": "102"}));
    assert_eq!(s.tool().name(), "select", "text edit ends on a page change");
}
