// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 4b in the view model (acceptance check PI7): the Text tool with its binding, text edit, the Graphic
//! tool's six shapes with their Back and finish rules and their arc direction, the two tools' ghost lines and words,
//! and their exact `Session` counts. Every test works on a writable temporary copy of Golden A.

mod common;

use std::sync::Arc;

use common::{Tmp, at, done, refused, state};
use ew_viewmodel::ew_api::output::ew_dl::EntityRef;
use ew_viewmodel::ew_api::output::{PageTables, TableItem};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::tools::BINDABLE;
use ew_viewmodel::tools::graphic::round_distance;
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

fn tables(s: &ShellModel) -> Arc<PageTables> {
    Arc::clone(&s.canvas().unwrap().page().tables)
}

fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The page point of grid index `k`.
fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The grid cursor to page point `p`.
fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "cursor.set", at(q, json!({})));
}

/// A click at page point `p` through the grid cursor.
fn click_at(s: &mut ShellModel, p: (i64, i64)) {
    cursor(s, p);
    done(s, "canvas.activate", json!({}));
}

fn canvas(s: &ShellModel) -> Value {
    state(s)["canvas"].clone()
}

fn describe_line(s: &ShellModel, prefix: &str) -> Option<String> {
    s.describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(str::to_owned)
}

fn select_ids(s: &mut ShellModel, ids: &[String]) {
    done(s, "select.set", json!({ "ids": ids }));
}

/// The free text whose printed text is `text`.
fn free_text(s: &ShellModel, text: &str) -> TableItem {
    tables(s)
        .items
        .iter()
        .find(|i| i.kind == "free_text" && i.text.as_deref() == Some(text))
        .cloned()
        .unwrap_or_else(|| panic!("no free text {text}"))
}

/// The IDs of the page's items.
fn item_ids(s: &ShellModel) -> Vec<String> {
    tables(s).items.iter().map(|i| i.id.to_string()).collect()
}

/// Checks the last commit is an `item.place` of the graphic `shape`, and remembers its item.
fn assert_graphic_commit(s: &ShellModel, page: &str, shape: Value, before: &[String]) {
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    assert_eq!(
        last.params,
        json!({"page": page, "kind": {"graphic": {"shape": shape}}}),
        "{last:?}"
    );
    let t = tables(s);
    let fresh: Vec<&TableItem> = t
        .items
        .iter()
        .filter(|i| !before.contains(&i.id.to_string()))
        .collect();
    assert_eq!(fresh.len(), 1, "one new item");
    assert_eq!(fresh[0].kind, "graphic");
}

/// A failed dispatch.
fn assert_failed(s: &mut ShellModel, action: &str, params: Value) {
    match s.dispatch(action, &params) {
        Outcome::Failed { .. } => {}
        other => panic!("{action}: {other:?}"),
    }
}

/// The structural tokens of a describe line: quoted text, numbers, aliases and punctuation left out.
fn structural_tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut quoted = false;
    let mut cur = String::new();
    for ch in line.chars() {
        if ch == '"' {
            quoted = !quoted;
            cur.clear();
            continue;
        }
        if quoted {
            continue;
        }
        if ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter()
        .filter(|t| {
            !ew_viewmodel::alias::is_alias(t)
                && t.parse::<f64>().is_err()
                && !t.contains('.')
                && !matches!(t.as_str(), "(" | ")" | "[" | "]" | "," | "|" | "%")
        })
        .collect()
}

/// `num / den` rounded to the nearest integer, halves away from zero (`den > 0`).
fn div_round(num: i128, den: i128) -> i128 {
    let q = num / den;
    let r = num % den;
    if 2 * r.abs() >= den { q + num.signum() } else { q }
}

/// The fine grid's point nearest to `p` on a QET page: `793_750/3` nm per axis, halves away from zero.
fn fine_snap(p: (i64, i64)) -> (i64, i64) {
    let coord = |v: i64| {
        let k = div_round(i128::from(v) * 3, 793_750);
        i64::try_from(div_round(k * 793_750, 3)).expect("the page coordinate is in range")
    };
    (coord(p.0), coord(p.1))
}

// ---------------------------------------------------------------------------------------------------------------
// PI7: the Text tool, its binding and text edit

#[test]
fn the_text_tool_places_a_free_text_on_the_fine_grid() {
    let tmp = Tmp::new("text-tool");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.text", json!({}));
    assert_eq!(s.tool().name(), "text");
    assert_eq!(s.tool().state(), "armed");
    // A page point off the 1-px fine grid (793_750/3 nm on this QET page).
    let base = grid_point(&s, (40, 30));
    let raw = (base.0 + 222_222, base.1 + 111_111);
    let point = sheet(&s, raw);
    // The tool snaps the exact page point of the pointer, computed here without `Snapper::snap_fine`.
    let page_raw = s.canvas().unwrap().page().snapper.sheet_to_page(point);
    let snapped = fine_snap(page_raw);
    assert_ne!(snapped, raw, "the raw point is not on the fine grid");
    done(&mut s, "canvas.press", at(point, json!({})));
    assert_eq!(s.tool().state(), "typing");
    let ghost = canvas(&s)["ghost"].clone();
    assert_eq!(ghost["kind"], "text");
    assert_eq!(ghost["at_nm"], json!([snapped.0, snapped.1]));
    assert_eq!(ghost["text"], "");
    assert_eq!(ghost["bind"], Value::Null);
    done(&mut s, "canvas.release", at(point, json!({})));
    done(&mut s, "text.input", json!({"text": "40x60"}));
    assert_eq!(canvas(&s)["ghost"]["text"], "40x60");
    let rev = s.revision();
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1));
    assert_eq!(s.tool().state(), "armed");
    let page = tables(&s).page.to_string();
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    assert_eq!(
        last.params,
        json!({"page": page, "kind": {"free_text": {"text": "40x60", "at": [snapped.0, snapped.1]}}})
    );
    let item = free_text(&s, "40x60");
    assert_eq!(item.bind, None);
    // A second text, committed with Enter (`canvas.activate`) instead of `tool.finish`.
    let raw2 = (base.0 + 500_000, base.1 + 400_000);
    let point2 = sheet(&s, raw2);
    let page_raw2 = s.canvas().unwrap().page().snapper.sheet_to_page(point2);
    let snapped2 = fine_snap(page_raw2);
    assert_ne!(snapped2, raw2, "the second raw point is off the fine grid");
    done(&mut s, "canvas.press", at(point2, json!({})));
    assert_eq!(s.tool().state(), "typing");
    done(&mut s, "text.input", json!({"text": "60x80"}));
    done(&mut s, "cursor.set", at(point2, json!({})));
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "Enter commits the text");
    assert_eq!(s.tool().state(), "armed");
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    assert_eq!(
        last.params,
        json!({"page": page, "kind": {"free_text": {"text": "60x80", "at": [snapped2.0, snapped2.1]}}})
    );
    assert_eq!(free_text(&s, "60x80").bind, None);
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
}

#[test]
fn escape_leaves_typing_then_the_text_tool() {
    let tmp = Tmp::new("text-escape");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.text", json!({}));
    let p = grid_point(&s, (40, 30));
    click_at(&mut s, p);
    assert_eq!(s.tool().state(), "typing");
    let rev = s.revision();
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "text");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(s.revision(), rev);
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
    assert_eq!(s.revision(), rev);
}

#[test]
fn a_bound_text_is_placed_at_the_press() {
    let tmp = Tmp::new("text-bound");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.text", json!({}));
    done(&mut s, "text.bind", json!({"bind": "project.description"}));
    assert_eq!(canvas(&s)["ghost"]["bind"], "project.description");
    let p = grid_point(&s, (42, 31));
    let sp = sheet(&s, p);
    let rev = s.revision();
    done(&mut s, "canvas.press", at(sp, json!({})));
    done(&mut s, "canvas.release", at(sp, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "the click places the text");
    assert_eq!(s.tool().state(), "armed");
    let page = tables(&s).page.to_string();
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    let free = &last.params["kind"]["free_text"];
    assert_eq!(free["bind"], "project.description");
    assert_eq!(free["at"], json!([p.0, p.1]));
    assert!(free.get("text").is_none(), "a bound text carries no text: {free}");
    assert_eq!(last.params["page"], page.as_str());
    let t = tables(&s);
    let item = t
        .items
        .iter()
        .find(|i| i.kind == "free_text")
        .expect("the bound text");
    assert_eq!(item.bind.as_deref(), Some("project.description"));
}

#[test]
fn text_bind_is_refused_outside_the_tool_and_fails_on_an_unknown_name() {
    let tmp = Tmp::new("text-bind");
    let mut s = shell(&tmp, "101");
    assert_eq!(
        refused(&mut s, "text.bind", json!({"bind": "page.title"})),
        "reason-text-tool-only"
    );
    done(&mut s, "tool.text", json!({}));
    assert_failed(&mut s, "text.bind", json!({"bind": "page.colour"}));
    done(&mut s, "text.bind", json!({"bind": "page.title"}));
    assert_eq!(canvas(&s)["ghost"]["bind"], "page.title");
    done(&mut s, "text.bind", json!({"bind": ""}));
    assert_eq!(canvas(&s)["ghost"]["bind"], Value::Null);
    done(&mut s, "text.bind", json!({"bind": "page.folio"}));
    done(&mut s, "text.bind", json!({"bind": null}));
    assert_eq!(canvas(&s)["ghost"]["bind"], Value::Null);
    assert_eq!(BINDABLE.len(), 6);
}

#[test]
fn text_edit_changes_the_text_in_one_undo_step() {
    let tmp = Tmp::new("text-edit");
    let mut s = shell(&tmp, "101");
    let p = grid_point(&s, (44, 32));
    let page = tables(&s).page.to_string();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "40x60", "at": [p.0, p.1]}}}}),
    );
    let item = free_text(&s, "40x60");
    select_ids(&mut s, &[item.id.to_string()]);
    done(&mut s, "text.edit", json!({}));
    assert_eq!(s.tool().name(), "text_edit");
    assert_eq!(s.tool().state(), "typing");
    let ghost = canvas(&s)["ghost"].clone();
    assert_eq!(ghost["kind"], "text");
    assert_eq!(ghost["text"], "40x60");
    assert_eq!(ghost["at_nm"], json!([item.bounds[0], item.bounds[1]]));
    done(&mut s, "text.input", json!({"text": "60x80"}));
    assert_eq!(canvas(&s)["ghost"]["text"], "60x80");
    let rev = s.revision();
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one fields.set");
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "fields.set");
    assert_eq!(
        last.params,
        json!({"kind": "page_item", "targets": [item.id.to_string()],
            "path": "kind.free_text.text", "value": "60x80"})
    );
    let changed = tables(&s)
        .items
        .iter()
        .find(|i| i.id == item.id)
        .cloned()
        .expect("the text");
    assert_eq!(changed.text.as_deref(), Some("60x80"));
    assert_eq!(s.tool().name(), "select", "the edit returns to Select");
    assert_eq!(s.selection().items, vec![EntityRef(item.id)], "kept");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(free_text(&s, "40x60").id, item.id);
}

#[test]
fn tool_finish_on_a_selected_free_text_opens_the_edit() {
    let tmp = Tmp::new("text-finish");
    let mut s = shell(&tmp, "101");
    let p = grid_point(&s, (46, 33));
    let page = tables(&s).page.to_string();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"text": "40x60", "at": [p.0, p.1]}}}}),
    );
    let item = free_text(&s, "40x60");
    select_ids(&mut s, &[item.id.to_string()]);
    cursor(&mut s, p);
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().name(), "text_edit");
    assert_eq!(canvas(&s)["ghost"]["text"], "40x60");
    let rev = s.revision();
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(s.tool().name(), "select", "unchanged text ends the edit");
    assert_eq!(s.revision(), rev, "nothing committed");
    assert_eq!(s.selection().items, vec![EntityRef(item.id)]);
}

#[test]
fn text_edit_is_refused_for_a_bound_text_and_without_one_free_text() {
    let tmp = Tmp::new("text-edit-refused");
    let mut s = shell(&tmp, "101");
    let p = grid_point(&s, (48, 34));
    let page = tables(&s).page.to_string();
    done(
        &mut s,
        "command.run",
        json!({"cmd": "item.place", "params": {"page": page,
            "kind": {"free_text": {"bind": "project.description", "at": [p.0, p.1]}}}}),
    );
    assert_eq!(
        refused(&mut s, "text.edit", json!({})),
        "reason-select-one-text",
        "nothing is selected"
    );
    let bound = tables(&s)
        .items
        .iter()
        .find(|i| i.kind == "free_text")
        .cloned()
        .expect("the bound text");
    select_ids(&mut s, &[bound.id.to_string()]);
    assert_eq!(refused(&mut s, "text.edit", json!({})), "reason-text-bound");
    let rep = tables(&s).reps[0].id.to_string();
    select_ids(&mut s, &[rep]);
    assert_eq!(
        refused(&mut s, "text.edit", json!({})),
        "reason-select-one-text",
        "a representation is no free text"
    );
}

// ---------------------------------------------------------------------------------------------------------------
// PI7: the Graphic tool

#[test]
fn the_graphic_tool_draws_every_shape() {
    let tmp = Tmp::new("graphic-shapes");
    let mut s = shell(&tmp, "101");
    let page = tables(&s).page.to_string();

    // Line: two clicks, L's default shape.
    done(&mut s, "tool.graphic", json!({}));
    assert_eq!(s.tool().name(), "graphic");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(
        s.tool_ghost(),
        Some(ew_viewmodel::tools::Ghost::Graphic {
            shape: "line",
            points: 0,
            anticlockwise: false,
        })
    );
    let a = grid_point(&s, (40, 20));
    let b = grid_point(&s, (46, 23));
    let before = item_ids(&s);
    click_at(&mut s, a);
    assert_eq!(s.tool().state(), "drawing");
    assert_eq!(canvas(&s)["ghost"]["points"], 1);
    click_at(&mut s, b);
    assert_eq!(s.tool().state(), "armed");
    assert_graphic_commit(
        &s,
        &page,
        json!({"line": {"a": [a.0, a.1], "b": [b.0, b.1]}}),
        &before,
    );

    // Rectangle: two clicked corners, as clicked.
    done(&mut s, "tool.graphic", json!({"shape": "rect"}));
    let a = grid_point(&s, (50, 20));
    let b = grid_point(&s, (55, 24));
    let before = item_ids(&s);
    click_at(&mut s, a);
    click_at(&mut s, b);
    assert_eq!(s.tool().state(), "armed");
    assert_graphic_commit(
        &s,
        &page,
        json!({"rect": {"min": [a.0, a.1], "max": [b.0, b.1]}}),
        &before,
    );

    // Circle: centre and a point on it, the radius from the two page points.
    done(&mut s, "tool.graphic", json!({"shape": "circle"}));
    let c = grid_point(&s, (60, 30));
    let on = grid_point(&s, (63, 34));
    let r = round_distance(c, on);
    let before = item_ids(&s);
    click_at(&mut s, c);
    assert_eq!(canvas(&s)["ghost"]["points"], 1);
    click_at(&mut s, on);
    assert_eq!(s.tool().state(), "armed");
    assert_graphic_commit(
        &s,
        &page,
        json!({"circle": {"centre": [c.0, c.1], "r": r}}),
        &before,
    );

    // Arc: centre, a start to the right and an end above it; clockwise, so -270°.
    done(&mut s, "tool.graphic", json!({"shape": "arc"}));
    let c = grid_point(&s, (70, 30));
    let st = grid_point(&s, (74, 30));
    let end = grid_point(&s, (70, 26));
    let before = item_ids(&s);
    click_at(&mut s, c);
    click_at(&mut s, st);
    assert_eq!(canvas(&s)["ghost"]["points"], 2);
    click_at(&mut s, end);
    assert_eq!(s.tool().state(), "armed");
    assert_graphic_commit(
        &s,
        &page,
        json!({"arc": {"centre": [c.0, c.1], "r": round_distance(c, st),
            "start": 0, "sweep": -270_000}}),
        &before,
    );

    // The same arc with Shift held on the end press: anticlockwise, so +90°.
    done(&mut s, "tool.graphic", json!({"shape": "arc"}));
    let c = grid_point(&s, (80, 30));
    let st = grid_point(&s, (84, 30));
    let end = grid_point(&s, (80, 26));
    let before = item_ids(&s);
    click_at(&mut s, c);
    click_at(&mut s, st);
    cursor(&mut s, end);
    let end_sheet = sheet(&s, end);
    done(&mut s, "canvas.press", at(end_sheet, json!({"shift": true})));
    assert_eq!(s.tool().state(), "armed");
    assert_graphic_commit(
        &s,
        &page,
        json!({"arc": {"centre": [c.0, c.1], "r": round_distance(c, st),
            "start": 0, "sweep": 90_000}}),
        &before,
    );

    // Polyline: three clicks and `tool.finish`.
    done(&mut s, "tool.graphic", json!({"shape": "polyline"}));
    let p0 = grid_point(&s, (40, 40));
    let p1 = grid_point(&s, (44, 42));
    let p2 = grid_point(&s, (48, 40));
    let before = item_ids(&s);
    for p in [p0, p1, p2] {
        click_at(&mut s, p);
    }
    assert_eq!(s.tool().state(), "drawing");
    assert_eq!(canvas(&s)["ghost"]["points"], 3);
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "armed");
    assert_graphic_commit(
        &s,
        &page,
        json!({"polyline": {"points": [[p0.0, p0.1], [p1.0, p1.1], [p2.0, p2.1]]}}),
        &before,
    );

    // Polygon: three non-collinear clicks and `tool.finish`.
    done(&mut s, "tool.graphic", json!({"shape": "polygon"}));
    let p0 = grid_point(&s, (50, 40));
    let p1 = grid_point(&s, (55, 44));
    let p2 = grid_point(&s, (60, 40));
    let before = item_ids(&s);
    for p in [p0, p1, p2] {
        click_at(&mut s, p);
    }
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "armed");
    assert_graphic_commit(
        &s,
        &page,
        json!({"polygon": {"points": [[p0.0, p0.1], [p1.0, p1.1], [p2.0, p2.1]]}}),
        &before,
    );
}

#[test]
fn back_and_finish_rules_of_the_graphic_tool() {
    let tmp = Tmp::new("graphic-back");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.graphic", json!({"shape": "polyline"}));
    let a = grid_point(&s, (40, 20));
    let b = grid_point(&s, (44, 24));
    click_at(&mut s, a);
    click_at(&mut s, b);
    assert_eq!(canvas(&s)["ghost"]["points"], 2);
    done(&mut s, "tool.back", json!({}));
    assert_eq!(s.tool().state(), "drawing");
    assert_eq!(canvas(&s)["ghost"]["points"], 1);
    let rev = s.revision();
    done(&mut s, "tool.back", json!({}));
    assert_eq!(s.tool().state(), "armed", "the last point goes");
    assert_eq!(canvas(&s)["ghost"]["points"], 0);
    assert_eq!(s.revision(), rev);
    // A polygon with two points does not finish; a click on the last point adds nothing.
    done(&mut s, "tool.graphic", json!({"shape": "polygon"}));
    let a = grid_point(&s, (50, 20));
    let b = grid_point(&s, (54, 24));
    click_at(&mut s, a);
    click_at(&mut s, b);
    done(&mut s, "tool.finish", json!({}));
    assert_eq!(s.tool().state(), "drawing");
    assert_eq!(canvas(&s)["ghost"]["points"], 2);
    assert_eq!(s.revision(), rev, "nothing committed");
    click_at(&mut s, b);
    assert_eq!(s.tool().state(), "drawing");
    assert_eq!(canvas(&s)["ghost"]["points"], 2);
}

#[test]
fn d_turns_the_arc_and_the_ghost_says_so() {
    let tmp = Tmp::new("graphic-d");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.graphic", json!({"shape": "arc"}));
    assert_failed(&mut s, "tool.ip_direction", json!({"direction": "source"}));
    done(&mut s, "tool.ip_direction", json!({}));
    let ghost = canvas(&s)["ghost"].clone();
    assert_eq!(ghost["kind"], "graphic");
    assert_eq!(ghost["shape"], "arc");
    assert_eq!(ghost["anticlockwise"], true);
    let line = describe_line(&s, "canvas ghost item graphic ").expect("the ghost line");
    assert!(line.ends_with(" anticlockwise"), "{line}");
    // The arc committed now has a positive sweep.
    let c = grid_point(&s, (70, 30));
    let st = grid_point(&s, (74, 30));
    let end = grid_point(&s, (70, 26));
    click_at(&mut s, c);
    click_at(&mut s, st);
    click_at(&mut s, end);
    let last = s.recorded_commands().last().expect("the commit");
    assert_eq!(last.cmd, "item.place");
    let arc = &last.params["kind"]["graphic"]["shape"]["arc"];
    assert_eq!(arc["sweep"], 90_000, "{arc}");
    assert!(arc["sweep"].as_i64().unwrap() > 0);
    // The next ghost is clockwise again.
    assert_eq!(canvas(&s)["ghost"]["anticlockwise"], false);
    let line = describe_line(&s, "canvas ghost item graphic ").expect("the ghost line");
    assert!(!line.ends_with(" anticlockwise"), "{line}");
}

#[test]
fn the_part_4b_ghost_lines_use_only_vocabulary_words() {
    let tmp = Tmp::new("graphic-words");
    let mut s = shell(&tmp, "101");
    let mut seen = std::collections::BTreeSet::new();
    let scan = |s: &ShellModel, seen: &mut std::collections::BTreeSet<String>| {
        for l in s.describe().lines().filter(|l| l.starts_with("canvas ")) {
            for t in structural_tokens(l) {
                assert!(
                    ew_viewmodel::describe::VOCABULARY.contains(&t.as_str()),
                    "`{t}` is no vocabulary word; line: {l}"
                );
                seen.insert(t);
            }
        }
    };
    // A typing Text tool.
    done(&mut s, "tool.text", json!({}));
    let p = grid_point(&s, (40, 30));
    click_at(&mut s, p);
    done(&mut s, "text.input", json!({"text": "40x60"}));
    let line = describe_line(&s, "canvas ghost item text ").expect("the typing text's ghost line");
    assert!(line.contains("text \"40x60\""), "{line}");
    scan(&s, &mut seen);
    // A bound Text tool.
    done(&mut s, "tool.text", json!({}));
    done(&mut s, "text.bind", json!({"bind": "page.title"}));
    let line = describe_line(&s, "canvas ghost item text ").expect("the bound text's ghost line");
    assert!(line.contains("text \"{page.title}\""), "{line}");
    scan(&s, &mut seen);
    // A drawing arc.
    done(&mut s, "tool.graphic", json!({"shape": "arc"}));
    let p = grid_point(&s, (60, 30));
    click_at(&mut s, p);
    let line = describe_line(&s, "canvas ghost item graphic arc points 1").expect("the arc's ghost line");
    assert!(!line.ends_with(" anticlockwise"), "{line}");
    scan(&s, &mut seen);
    done(&mut s, "tool.ip_direction", json!({}));
    let line = describe_line(&s, "canvas ghost item graphic arc points 1").expect("the arc's ghost line");
    assert!(line.ends_with(" anticlockwise"), "{line}");
    scan(&s, &mut seen);
    assert!(seen.contains("graphic"), "{seen:?}");
    assert!(seen.contains("anticlockwise"), "{seen:?}");
    let words = ew_viewmodel::describe::ITEM_WORDS;
    for w in words {
        assert!(ew_viewmodel::describe::VOCABULARY.contains(w));
    }
}

#[test]
fn pointer_moves_with_the_text_and_graphic_tools_call_no_session() {
    let tmp = Tmp::new("graphic-counts");
    let mut s = shell(&tmp, "101");
    let p = grid_point(&s, (40, 30));
    let step = |k: i64| (p.0 + k * 300_000, p.1 + k * 200_000);
    // The Text tool, armed and typing.
    done(&mut s, "tool.text", json!({}));
    let calls = s.session_calls();
    for k in 0..20 {
        let q = sheet(&s, step(k));
        done(&mut s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "the armed Text tool reads nothing");
    click_at(&mut s, p);
    let calls = s.session_calls();
    for k in 0..20 {
        let q = sheet(&s, step(k));
        done(&mut s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "the typing Text tool reads nothing");
    // The Graphic tool, armed and drawing.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.graphic", json!({"shape": "polyline"}));
    let calls = s.session_calls();
    for k in 0..20 {
        let q = sheet(&s, step(k));
        done(&mut s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "the armed Graphic tool reads nothing");
    click_at(&mut s, p);
    let calls = s.session_calls();
    for k in 0..20 {
        let q = sheet(&s, step(k));
        done(&mut s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "the drawing Graphic tool reads nothing");
}
