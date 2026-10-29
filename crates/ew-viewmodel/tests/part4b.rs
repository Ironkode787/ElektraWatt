// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 4b in the view model: Select drags and the keyboard move of page items and structure boxes, one
//! `rep.move` for a representation, an item and a box, Delete of the three, the box's outline pick against a press
//! inside it, the part-first arm with a part whose template needs two FnDefs, the failure that names a definition no
//! library holds, and `page.describe`'s item and box lines with their vocabulary. Every test works on a writable
//! temporary copy of Golden A.

mod common;

use common::{Tmp, at, done, state, structural_tokens};
use ew_viewmodel::canvas::{HOVER_SLOP_PT, pick};
use ew_viewmodel::ew_api::output::{PageTables, TableEnd, TableItem, TableRep, TableStructureBox};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};

/// The fixture part `MICO 4.10 TWO` (WP-S2.04 part 4b's `ew-testkit` test writes it).
const TWO_FNDEF_PART: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-api/tests/expected/two_fndef_part.json"
);
/// The fixture part `MICO 4.10 MISSING`, whose second template FnDef no library holds.
const MISSING_FNDEF_PART: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-api/tests/expected/missing_fndef_part.json"
);
/// The fixture part `MICO 4.10 STARTER`, which needs two starter FnDefs and no black box.
const TWO_STARTER_FNDEF_PART: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../ew-api/tests/expected/two_starter_fndef_part.json"
);

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The sheet point of grid index `k`.
fn grid_sheet(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.sheet_of_index(k.0, 0), sn.sheet_of_index(k.1, 1))
}

/// The page point of grid index `k`.
fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The sheet point `k` grid steps from the sheet point `from` (the displacement a drag of `k` steps has).
fn step_sheet(s: &ShellModel, from: (i64, i64), k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.sheet_to_page(from);
    let (ix, iy) = (sn.grid_index_of(p.0), sn.grid_index_of(p.1));
    (
        from.0 + sn.sheet_of_index(ix + k.0, 0) - sn.sheet_of_index(ix, 0),
        from.1 + sn.sheet_of_index(iy + k.1, 1) - sn.sheet_of_index(iy, 1),
    )
}

fn canvas(s: &ShellModel) -> Value {
    state(s)["canvas"].clone()
}

fn select_ids(s: &mut ShellModel, ids: &[String]) {
    done(s, "select.set", json!({ "ids": ids }));
}

/// The representation of the page drawing the symbol `key` (the first in ID order).
fn rep_with(s: &ShellModel, key: &str) -> TableRep {
    tables(s)
        .reps
        .iter()
        .find(|r| r.symbol_key == key)
        .unwrap_or_else(|| panic!("no {key}"))
        .clone()
}

/// The centre of `bounds`.
fn centre(bounds: &[i64; 4]) -> (i64, i64) {
    ((bounds[0] + bounds[2]) / 2, (bounds[1] + bounds[3]) / 2)
}

/// Places the free text `text` at page point `at` and returns its page-table row.
fn place_text(s: &mut ShellModel, at: (i64, i64), text: &str) -> TableItem {
    let page = tables(s).page.to_string();
    done(
        s,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": text, "at": [at.0, at.1]}}}}),
    );
    tables(s)
        .items
        .iter()
        .find(|i| i.text.as_deref() == Some(text))
        .cloned()
        .unwrap_or_else(|| panic!("no free text {text}"))
}

/// Places a structure box with `outline` and `caption` and returns its page-table row.
fn place_box(s: &mut ShellModel, outline: &[(i64, i64)], caption: &str) -> TableStructureBox {
    let page = tables(s).page.to_string();
    done(
        s,
        "command.run",
        json!({"cmd": "box.structure", "params": {"page": page,
            "outline": outline.iter().map(|(x, y)| json!([x, y])).collect::<Vec<_>>(),
            "style": "dotted", "caption": caption, "assigns": {}}}),
    );
    tables(s)
        .structure_boxes
        .iter()
        .find(|b| b.caption == caption)
        .cloned()
        .unwrap_or_else(|| panic!("no box {caption}"))
}

/// A free grid rectangle on the page: no entity and no link under its four corners, its four edge midpoints or its
/// centre. Its outline and the page point of its centre.
fn free_rectangle(s: &ShellModel) -> (Vec<(i64, i64)>, (i64, i64)) {
    let c = s.canvas().unwrap();
    let slop = HOVER_SLOP_PT / c.points_per_nm();
    let sn = &c.page().snapper;
    let free = |p: (i64, i64)| {
        let sheet = sn.page_to_sheet(p);
        c.link_at(sheet).is_none() && pick(&c.page().entities, Some(sheet), slop).is_none()
    };
    for ky in (10..90).step_by(4) {
        for kx in (10..120).step_by(4) {
            let pt = |dx: i64, dy: i64| (sn.page_of_index(kx + dx), sn.page_of_index(ky + dy));
            let checks = [
                pt(-2, -2),
                pt(2, -2),
                pt(2, 2),
                pt(-2, 2),
                pt(0, -2),
                pt(2, 0),
                pt(0, 2),
                pt(-2, 0),
                pt(0, 0),
            ];
            if checks.iter().all(|p| free(*p)) {
                return (vec![pt(-2, -2), pt(2, -2), pt(2, 2), pt(-2, 2)], pt(0, 0));
            }
        }
    }
    panic!("no free rectangle on folio {}", c.page().folio);
}

/// Embeds the fixture document at `path` through `library.embed`.
fn embed_fixture(s: &mut ShellModel, path: &str) {
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("JSON");
    done(
        s,
        "command.run",
        json!({"cmd": "library.embed", "params": {"copies": [fixture]}}),
    );
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

/// The connection whose two end labels are `a` and `b` (either order).
fn connection_between<'a>(
    t: &'a PageTables,
    a: &str,
    b: &str,
) -> &'a ew_viewmodel::ew_api::output::TableConnection {
    t.connections
        .iter()
        .find(|c| {
            let (x, y) = (end_label(t, &c.a), end_label(t, &c.b));
            (x == a && y == b) || (x == b && y == a)
        })
        .unwrap_or_else(|| panic!("no connection between {a} and {b}"))
}

// ---------------------------------------------------------------------------------------------------------------
// Select moves and deletes items and boxes

#[test]
fn a_free_text_dragged_alone_moves_by_grid_steps() {
    let tmp = Tmp::new("part4b-text");
    let mut s = shell(&tmp, "101");
    let p = grid_point(&s, (60, 30));
    let text = place_text(&mut s, p, "40x60");
    let before = text.bounds;
    let from = sheet(&s, centre(&before));
    let rev = s.revision();
    done(&mut s, "canvas.press", at(from, json!({})));
    assert_eq!(s.tool().state(), "pressed");
    let to = step_sheet(&s, from, (3, 0));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let after = tables(&s)
        .items
        .iter()
        .find(|i| i.id == text.id)
        .cloned()
        .expect("the text");
    assert_eq!(after.bounds[0] - before[0], 7_937_500, "x: three grid steps");
    assert_eq!(after.bounds[2] - before[2], 7_937_500);
    assert_eq!((after.bounds[1], after.bounds[3]), (before[1], before[3]));
    done(&mut s, "edit.undo", json!({}));
    let restored = tables(&s)
        .items
        .iter()
        .find(|i| i.id == text.id)
        .cloned()
        .expect("the text");
    assert_eq!(restored.bounds, before);
}

#[test]
fn a_rep_an_item_and_a_box_move_in_one_command() {
    let tmp = Tmp::new("part4b-three");
    let mut s = shell(&tmp, "101");
    let s51 = rep_with(&s, "ew.iec.switch-2p-manual@qet10");
    let p = grid_point(&s, (62, 30));
    let text = place_text(&mut s, p, "together");
    let (outline, _) = free_rectangle(&s);
    let b = place_box(&mut s, &outline, "Moved");
    let before = (s51.pos, text.bounds, b.outline.clone());
    select_ids(
        &mut s,
        &[s51.id.to_string(), text.id.to_string(), b.id.to_string()],
    );
    let from = sheet(&s, centre(&s51.body));
    let rev = s.revision();
    done(&mut s, "canvas.press", at(from, json!({})));
    assert_eq!(s.selection().items.len(), 3, "the press keeps the selection");
    let to = step_sheet(&s, from, (2, 0));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one rep.move");
    let t = tables(&s);
    let after = t.reps.iter().find(|r| r.id == s51.id).expect("S51");
    let sn = &s.canvas().unwrap().page().snapper;
    assert_eq!(
        after.pos,
        (sn.page_of_index(sn.grid_index_of(before.0.0) + 2), before.0.1),
        "S51 two grid steps right"
    );
    let moved_text = t.items.iter().find(|i| i.id == text.id).expect("the text");
    assert_eq!(moved_text.bounds[0] - before.1[0], 5_291_667);
    assert_eq!(moved_text.bounds[2] - before.1[2], 5_291_667);
    assert_eq!(
        (moved_text.bounds[1], moved_text.bounds[3]),
        (before.1[1], before.1[3])
    );
    let moved_box = t.structure_boxes.iter().find(|x| x.id == b.id).expect("the box");
    for (vertex, old) in moved_box.outline.iter().zip(&before.2) {
        assert_eq!(vertex.0 - old.0, 5_291_667);
        assert_eq!(vertex.1, old.1);
    }
    // One undo step restores all three.
    done(&mut s, "edit.undo", json!({}));
    let t = tables(&s);
    assert_eq!(t.reps.iter().find(|r| r.id == s51.id).unwrap().pos, before.0);
    assert_eq!(t.items.iter().find(|i| i.id == text.id).unwrap().bounds, before.1);
    assert_eq!(
        t.structure_boxes.iter().find(|x| x.id == b.id).unwrap().outline,
        before.2
    );
}

#[test]
fn the_keyboard_move_moves_items_and_boxes() {
    let tmp = Tmp::new("part4b-keys");
    let mut s = shell(&tmp, "101");
    let p = grid_point(&s, (60, 30));
    let text = place_text(&mut s, p, "keys");
    let (outline, _) = free_rectangle(&s);
    let b = place_box(&mut s, &outline, "Keys");
    select_ids(&mut s, &[text.id.to_string(), b.id.to_string()]);
    done(&mut s, "tool.move", json!({}));
    assert_eq!(s.tool().state(), "moving");
    let ghost = &canvas(&s)["ghost"];
    assert_eq!(ghost["kind"], "move");
    assert_eq!(ghost["count"], 2, "the text and the box");
    // One grid step down from the anchor the host put the cursor on.
    let from = s.canvas().unwrap().cursor().expect("the cursor is shown");
    let to = step_sheet(&s, from, (0, 1));
    done(&mut s, "cursor.set", at(to, json!({})));
    assert_eq!(canvas(&s)["ghost"]["steps"], json!([0, 1]));
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let t = tables(&s);
    let moved_text = t.items.iter().find(|i| i.id == text.id).expect("the text");
    assert_eq!(moved_text.bounds[1] - text.bounds[1], 2_645_833);
    assert_eq!(moved_text.bounds[0], text.bounds[0]);
    let moved_box = t.structure_boxes.iter().find(|x| x.id == b.id).expect("the box");
    for (vertex, old) in moved_box.outline.iter().zip(&b.outline) {
        assert_eq!(vertex.1 - old.1, 2_645_833);
        assert_eq!(vertex.0, old.0);
    }
}

#[test]
fn delete_removes_a_rep_an_item_and_a_box_in_one_step() {
    let tmp = Tmp::new("part4b-delete");
    let mut s = shell(&tmp, "101");
    let s51 = rep_with(&s, "ew.iec.switch-2p-manual@qet10");
    let p = grid_point(&s, (62, 32));
    let text = place_text(&mut s, p, "deleted");
    let (outline, _) = free_rectangle(&s);
    let b = place_box(&mut s, &outline, "Delete");
    select_ids(
        &mut s,
        &[s51.id.to_string(), text.id.to_string(), b.id.to_string()],
    );
    let rev = s.revision();
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one edit.delete");
    let t = tables(&s);
    assert!(!t.reps.iter().any(|r| r.id == s51.id));
    assert!(!t.items.iter().any(|i| i.id == text.id));
    assert!(!t.structure_boxes.iter().any(|x| x.id == b.id));
    assert!(s.selection().is_empty(), "deleted records leave the selection");
    done(&mut s, "edit.undo", json!({}));
    let t = tables(&s);
    assert!(t.reps.iter().any(|r| r.id == s51.id));
    assert!(t.items.iter().any(|i| i.id == text.id));
    assert!(t.structure_boxes.iter().any(|x| x.id == b.id));
}

#[test]
fn a_press_inside_a_box_starts_a_band_and_its_outline_picks_it() {
    let tmp = Tmp::new("part4b-box-pick");
    let mut s = shell(&tmp, "101");
    let (outline, centre) = free_rectangle(&s);
    let b = place_box(&mut s, &outline, "Pick");
    // Inside, away from the outline: nothing is picked, a band starts.
    let from = sheet(&s, centre);
    done(&mut s, "canvas.press", at(from, json!({})));
    assert_eq!(s.tool().state(), "rubber-band");
    let d = (10.0 / s.canvas().unwrap().points_per_nm()) as i64;
    done(&mut s, "canvas.pointer", at((from.0 + d, from.1), json!({})));
    assert_eq!(s.tool().state(), "rubber-band");
    done(&mut s, "ui.dismiss", json!({}));
    // The middle of the top edge picks the box; two grid steps drag it alone.
    let top = sheet(&s, (centre.0, outline[0].1));
    done(&mut s, "canvas.press", at(top, json!({})));
    assert_eq!(s.tool().state(), "pressed");
    let to = step_sheet(&s, top, (2, 0));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    assert_eq!(s.tool().state(), "dragging");
    let rev = s.revision();
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "rep.move");
    assert_eq!(last.params["items"], json!([b.id.to_string()]));
}

// ---------------------------------------------------------------------------------------------------------------
// The part-first arm

#[test]
fn a_two_fndef_part_embeds_its_missing_fndef_once() {
    let tmp = Tmp::new("part4b-part-fndef");
    let mut s = shell(&tmp, "101");
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(TWO_FNDEF_PART).expect("the fixture")).expect("JSON");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "library.embed", "params": {"copies": [fixture]}}),
    );
    // The black box's template function: the arm embeds the part's missing contact FnDef once.
    let rev = s.revision();
    done(
        &mut s,
        "place.pick",
        json!({"part": "MICO 4.10 TWO", "template": 0, "letter": "A"}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one library.embed");
    assert_eq!(s.tool().name(), "black_box");
    assert_eq!(s.tool().state(), "armed");
    // Two corners with the grid cursor and Enter.
    let (kx, ky) = (60, 40);
    let first = grid_sheet(&s, (kx, ky));
    done(&mut s, "cursor.set", at(first, json!({})));
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().state(), "outline");
    let second = grid_sheet(&s, (kx + 6, ky + 4));
    done(&mut s, "cursor.set", at(second, json!({})));
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "box.black");
    let t = tables(&s);
    let rep = t
        .reps
        .iter()
        .find(|r| r.black_box.is_some())
        .expect("the new box")
        .clone();
    let device = rep.device.expect("the box's device");
    // The device has the part's two functions: read from the saved copy.
    done(&mut s, "file.save", json!({}));
    let dir = tmp.golden_copy();
    let (session, _) =
        ew_viewmodel::ew_api::Session::open_read_only(&dir, "test").expect("the saved copy opens");
    let n = {
        let q = session.query();
        q.functions()
            .iter()
            .filter(|(_, f)| f.device.is_some_and(|d| d.as_uuid() == device))
            .count()
    };
    assert_eq!(n, 2, "the black box and the contact");
    // A second pick of the same template commits nothing.
    let rev = s.revision();
    done(
        &mut s,
        "place.pick",
        json!({"part": "MICO 4.10 TWO", "template": 0, "letter": "A"}),
    );
    assert_eq!(s.revision(), rev);
}

#[test]
fn a_part_function_with_a_starter_symbol_embeds_symbol_and_fndefs_in_one_step() {
    let tmp = Tmp::new("part4b-part-first");
    let mut s = shell(&tmp, "101");
    // Golden A embeds neither FnDef the fixture's template names.
    let fixture: Value =
        serde_json::from_str(&std::fs::read_to_string(TWO_STARTER_FNDEF_PART).expect("the fixture"))
            .expect("JSON");
    {
        let (golden, _) =
            ew_viewmodel::ew_api::Session::open_read_only(std::path::Path::new(common::GOLDEN_A), "test")
                .expect("Golden A opens read-only");
        let q = golden.query();
        for entry in fixture["function_template"].as_array().expect("a template") {
            let library = entry["fndef"]["library"].as_str().expect("a library");
            let item = entry["fndef"]["item"].as_str().expect("an item");
            assert!(
                !q.fndefs()
                    .iter()
                    .any(|(_, d)| d.library_id.to_string() == library && d.item_id.to_string() == item),
                "Golden A embeds the fixture FnDef {}",
                entry["fndef"]["key"]
            );
        }
    }
    embed_fixture(&mut s, TWO_STARTER_FNDEF_PART);
    let rev = s.revision();
    done(
        &mut s,
        "place.pick",
        json!({"part": "MICO 4.10 STARTER", "template": 0, "letter": "K"}),
    );
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one library.embed");
    assert_eq!(s.tool().name(), "place");
    assert_eq!(s.tool().state(), "floating");
    let ghost = &canvas(&s)["ghost"];
    assert_eq!(ghost["symbol"], "ew.iec.contact-no@qet10");
    assert_eq!(ghost["target"], "part");
    // The one host commit embedded the symbol and both starter FnDefs, in that order.
    let embed = s.recorded_commands().last().expect("the library.embed");
    assert_eq!(embed.cmd, "library.embed");
    let keys: Vec<&str> = embed.params["copies"]
        .as_array()
        .expect("copies")
        .iter()
        .map(|c| c["meta"]["key"].as_str().expect("a key"))
        .collect();
    assert_eq!(
        keys,
        ["ew.iec.contact-no@qet10", "ew.fn.contact-no", "ew.fn.contact-nc"]
    );
    // A checked-free grid point, then Alt+Enter: `from_part` refuses unless the template's second FnDef is
    // embedded too.
    let (_, p) = free_rectangle(&s);
    let cursor = sheet(&s, p);
    done(&mut s, "cursor.set", at(cursor, json!({})));
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({"alt": true}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "device.place");
    assert_eq!(
        s.recorded_commands().last().map(|c| c.cmd.as_str()),
        Some("device.place")
    );
}

#[test]
fn a_part_whose_fndef_no_library_holds_fails_naming_it() {
    let tmp = Tmp::new("part4b-missing");
    let mut s = shell(&tmp, "101");
    embed_fixture(&mut s, MISSING_FNDEF_PART);
    let rev = s.revision();
    match s.dispatch(
        "place.pick",
        &json!({"part": "MICO 4.10 MISSING", "template": 0, "letter": "A"}),
    ) {
        Outcome::Failed { message } => assert!(message.contains("test.fn.unknown"), "{message}"),
        other => panic!("the pick must fail, got {other:?}"),
    }
    assert_eq!(s.revision(), rev, "nothing committed");
    assert_eq!(s.tool().name(), "select", "the tool is not armed");
}

// ---------------------------------------------------------------------------------------------------------------
// `page.describe`'s item and box lines

#[test]
fn page_describe_lists_items_and_boxes() {
    let tmp = Tmp::new("part4b-describe");
    let mut s = shell(&tmp, "101");
    let page = tables(&s).page.to_string();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"graphic": {"shape": {"line": {
                "a": [50_000_000, 90_000_000], "b": [60_000_000, 90_000_000]}}}}}}),
    );
    done(
        &mut s,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "40x60", "at": [50_000_000, 100_000_000]}}}}),
    );
    let t = tables(&s);
    let w4 = connection_between(&t, "1X1:1/a", "S51:2");
    let w5 = connection_between(&t, "1X1:2/a", "S51:4");
    done(
        &mut s,
        "command.run",
        json!({"cmd": "marker.place", "params": {"page": page,
            "connections": [w4.id.to_string(), w5.id.to_string()], "at": [60_000_000, 40_000_000]}}),
    );
    let (outline, _) = free_rectangle(&s);
    done(
        &mut s,
        "command.run",
        json!({"cmd": "box.structure", "params": {"page": page,
            "outline": outline.iter().map(|(x, y)| json!([x, y])).collect::<Vec<_>>(),
            "style": "dotted", "caption": "Spare", "assigns": {}}}),
    );
    done(&mut s, "page.describe", json!({"folio": "101"}));
    let text = s.take_page_text().expect("the page text");
    let first = text.lines().next().expect("the header");
    assert!(first.ends_with(" items 3 boxes 1"), "{first}");
    let count = |prefix: &str| text.lines().filter(|l| l.starts_with(prefix)).count();
    assert_eq!(count("  item graphic cell "), 1);
    assert_eq!(count("  item text cell "), 1);
    assert_eq!(count("  item marker cell "), 1);
    assert_eq!(count("  box structure cell "), 1);
    assert!(
        text.lines()
            .any(|l| l.starts_with("  item text cell ") && l.ends_with(" \"40x60\"")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("  item marker cell ") && l.ends_with(" \"2.5 BK BU\"")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("  box structure cell ") && l.ends_with(" \"Spare\"")),
        "{text}"
    );
    for line in text
        .lines()
        .filter(|l| l.starts_with("  item ") || l.starts_with("  box "))
    {
        for token in structural_tokens(line) {
            assert!(
                ew_viewmodel::describe::VOCABULARY.contains(&token.as_str()),
                "`{token}` is no vocabulary word; line: {line}"
            );
        }
    }
    // G-PP triage row 12: every item line has a position in mm, and the marker's position picks the marker, whose
    // selection title names its kind in words.
    for line in text.lines().filter(|l| l.starts_with("  item ")) {
        assert!(line.contains(" mm"), "{line}");
    }
    let marker = text
        .lines()
        .find(|l| l.starts_with("  item marker cell "))
        .expect("the marker line");
    let at: Vec<&str> = marker
        .split(" at ")
        .nth(1)
        .expect("an at")
        .split(' ')
        .take(2)
        .collect();
    let p = json!({"x_mm": at[0].parse::<f64>().unwrap(), "y_mm": at[1].parse::<f64>().unwrap()});
    done(&mut s, "canvas.press", p.clone());
    done(&mut s, "canvas.release", p);
    let sheet = s.properties().sheet().expect("a sheet");
    assert_eq!(sheet.sections[0].kind, "page_item", "{marker}");
    assert_eq!(sheet.header.title, "wire marker");
    // G-PP triage row 21: a terminal CP names its side beside its clamp (the strip's field side is side 2, /b).
    assert!(
        text.lines()
            .any(|l| l.starts_with("    cp \"1X1:1/a\" internal at ")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("    cp \"1X1:1/b\" field at ")),
        "{text}"
    );
}

/// Only replacing the active tool advances the epoch; a host reset cannot recycle its previous value.
#[test]
fn tool_epoch_tracks_replacements_without_session_reads() {
    let tmp = Tmp::new("tool-epoch");
    let blank = ShellModel::new(
        Mode::Headless,
        ew_viewmodel::shell::ShellConfig::in_dir(&tmp.0.join("blank"), "0.0.0-test"),
    );
    assert_eq!(blank.tool_epoch(), 0);
    let mut s = shell(&tmp, "101");
    let initial = s.tool_epoch();
    let calls = s.session_calls();
    assert_eq!(s.tool_epoch(), initial);
    assert_eq!(s.session_calls(), calls, "the accessor reads no Session data");
    done(&mut s, "tool.marker", json!({}));
    assert_eq!(s.tool_epoch(), initial.wrapping_add(1));
    let armed = s.tool_epoch();
    done(&mut s, "marker.fields", json!({"csa":"2.5"}));
    done(&mut s, "cursor.set", json!({"x_mm":100.0,"y_mm":100.0}));
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(
        s.tool_epoch(),
        armed,
        "ordinary events and field edits preserve drafts"
    );
    done(&mut s, "tool.marker", json!({}));
    assert_eq!(
        s.tool_epoch(),
        armed.wrapping_add(1),
        "same-tool activation replaces the instance"
    );
    let before_page = s.tool_epoch();
    done(&mut s, "page.goto", json!({"folio":"102"}));
    assert_eq!(s.tool().name(), "select");
    assert_eq!(
        s.tool_epoch(),
        before_page.wrapping_add(1),
        "the direct context replacement advances the epoch"
    );
    let before_close = s.tool_epoch();
    done(&mut s, "file.close", json!({}));
    assert_eq!(
        s.tool_epoch(),
        before_close.wrapping_add(1),
        "resetting the host preserves the generation"
    );
    done(
        &mut s,
        "file.open",
        json!({"dir":tmp.golden_copy().to_string_lossy()}),
    );
    // Opening first closes the empty host, then install_view creates the project host.
    assert_eq!(s.tool_epoch(), before_close.wrapping_add(3));
    s.shutdown();
}
