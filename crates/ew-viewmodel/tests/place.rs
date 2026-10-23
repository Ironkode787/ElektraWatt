// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 1 in the view model (acceptance checks PL3, PL4, PL5 and the view-model part of PL7 and PL8): the
//! picker and the starter embedding, the Place tool, moving by drag and by keys, transform and delete on a
//! selection, change symbol, the transcript members and the describe lines with their 28 words, and the exact
//! `Session` counts of pointer and cursor moves. Every test works on a writable temporary copy of Golden A.

mod common;

use common::{Tmp, at, click, done, refused, state};
use ew_viewmodel::ew_api::output::{PageTables, TableRep};
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::tools::OverlayItem;
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
}

fn tables(shell: &ShellModel) -> std::sync::Arc<PageTables> {
    std::sync::Arc::clone(&shell.canvas().unwrap().page().tables)
}

/// The representation of the page drawing the symbol `key` (the first in ID order).
fn rep_with(shell: &ShellModel, key: &str) -> TableRep {
    tables(shell)
        .reps
        .iter()
        .find(|r| r.symbol_key == key)
        .unwrap_or_else(|| panic!("no {key}"))
        .clone()
}

/// The sheet point of the page point `p` of the focused page.
fn sheet(shell: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    shell.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The sheet point `k` grid steps from the sheet point of the page point `p`.
fn steps_from(shell: &ShellModel, p: (i64, i64), k: (i64, i64)) -> (i64, i64) {
    let s = &shell.canvas().unwrap().page().snapper;
    let (ix, iy) = (s.grid_index_of(p.0), s.grid_index_of(p.1));
    (
        s.sheet_of_index(ix + k.0, 0) - s.sheet_of_index(ix, 0) + s.to_sheet(p.0, 0),
        s.sheet_of_index(iy + k.1, 1) - s.sheet_of_index(iy, 1) + s.to_sheet(p.1, 1),
    )
}

/// The page point of grid index `k` of the focused page.
fn grid_point(shell: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let s = &shell.canvas().unwrap().page().snapper;
    (s.page_of_index(k.0), s.page_of_index(k.1))
}

/// A free grid point of folio 101's empty lower right area.
fn free_point(shell: &ShellModel) -> (i64, i64) {
    grid_point(shell, (60, 30))
}

fn ghost(shell: &ShellModel) -> Value {
    state(shell)["canvas"]["ghost"].clone()
}

fn describe_line(shell: &ShellModel, prefix: &str) -> String {
    shell
        .describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no line {prefix}"))
        .to_owned()
}

#[test]
fn the_picker_lists_the_grid_family_embeds_a_starter_symbol_once_and_disables_rows_with_reasons() {
    let tmp = Tmp::new("pick");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.place", json!({}));
    let picker = state(&s)["canvas"]["picker"].clone();
    assert_eq!(picker["mode"], "place");
    assert_eq!(picker["filter"], "");
    assert_eq!(picker["row"], 0);
    let p = s.picker().unwrap();
    assert!(
        p.catalogue.symbols.iter().all(|r| r.key.ends_with("@qet10")),
        "the page's grid family"
    );
    // The filter: case-folded substrings of the key, the part number and the FnDef key.
    done(&mut s, "picker.filter", json!({"text": "MCB"}));
    let rows = s.picker().unwrap().rows.len();
    assert!(rows >= 2, "{rows}");
    let text = s.describe();
    assert!(
        text.contains(&format!("canvas picker filter \"MCB\" rows {rows}")),
        "{text}"
    );
    assert!(
        text.contains("  row symbol \"ew.iec.mcb-1p@qet10\" embedded"),
        "{text}"
    );
    // A black-box symbol row is disabled with its reason, and choosing it is refused.
    done(&mut s, "picker.filter", json!({"text": "blackbox"}));
    let line = describe_line(&s, "  row symbol \"ew.gen.blackbox@qet10\"");
    assert!(line.contains("disabled: Use the black box tool (B)"), "{line}");
    assert_eq!(
        refused(&mut s, "picker.choose", json!({})),
        "reason-black-box-tool"
    );
    // Rows move and clamp.
    done(&mut s, "picker.filter", json!({"text": ""}));
    let n = s.picker().unwrap().rows.len();
    done(&mut s, "picker.row", json!({"to": -1}));
    assert_eq!(s.picker().unwrap().row, n - 1);
    done(&mut s, "picker.row", json!({"step": 5}));
    assert_eq!(s.picker().unwrap().row, n - 1);
    done(&mut s, "picker.row", json!({"to": 0}));
    // A starter symbol (Golden A has no NO contact): one `library.embed`, then the tool floats it.
    done(&mut s, "picker.filter", json!({"text": "contact-no@"}));
    assert!(describe_line(&s, "  row symbol \"ew.iec.contact-no@qet10\"").contains("starter"));
    let undo_before = s.can_undo();
    let calls = s.session_calls();
    done(&mut s, "picker.choose", json!({"letter": "K"}));
    assert!(s.picker().is_none());
    assert_eq!(s.tool().name(), "place");
    assert_eq!(s.tool().state(), "floating");
    assert!(!undo_before && s.can_undo(), "the embedding is an undo step");
    assert_eq!(
        s.session_calls().run_script,
        calls.run_script + 1,
        "one host commit"
    );
    // Escape ends the tool without a change; a second choice of the symbol embeds nothing.
    let rev = s.revision();
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.tool().name(), "select");
    assert_eq!(s.revision(), rev, "Escape leaves the revision unchanged");
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "picker.filter", json!({"text": "contact-no@"}));
    assert!(describe_line(&s, "  row symbol \"ew.iec.contact-no@qet10\"").contains("embedded"));
    let calls = s.session_calls();
    done(&mut s, "picker.choose", json!({"letter": "K"}));
    assert_eq!(s.session_calls().run_script, calls.run_script, "nothing embedded");
    assert_eq!(s.revision(), rev);
    // Undo takes back the embedding.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "edit.undo", json!({}));
    assert!(!s.can_undo());
}

#[test]
fn placing_a_contact_cycles_the_variants_and_commits_the_chosen_orientation() {
    let tmp = Tmp::new("variant");
    let mut s = shell(&tmp, "101");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    let p = free_point(&s);
    let pt0 = sheet(&s, p);
    done(&mut s, "canvas.pointer", at(pt0, json!({})));
    let g = ghost(&s);
    assert_eq!(g["kind"], "place");
    assert_eq!(g["symbol"], "ew.iec.contact-no@qet10");
    assert_eq!(g["variant"], "A");
    assert_eq!(g["target"], "new");
    assert_eq!(g["at_nm"], json!([p.0, p.1]));
    assert_eq!(g["hints"], 0);
    // The overlay: the outline, a marker per CP, the label with the variant and the target.
    let overlay = s.tool_overlay().clone();
    assert!(matches!(overlay.items[0], OverlayItem::Paths { transform: (0, q), .. } if q == p));
    assert!(overlay.labels().any(|l| l == "A new K"), "{overlay:?}");
    // V cycles A to H and back to A.
    let mut letters = String::new();
    for _ in 0..8 {
        done(&mut s, "tool.next_variant", json!({}));
        letters.push_str(ghost(&s)["variant"].as_str().unwrap());
    }
    assert_eq!(letters, "BCDEFGHA");
    // R, Shift+R, X and Y compose as rep.transform does.
    for (action, letter) in [
        ("tool.rotate_cw", "B"),
        ("tool.rotate_ccw", "A"),
        ("tool.rotate_ccw", "D"),
        ("tool.rotate_cw", "A"),
        ("tool.mirror_x", "E"),
        ("tool.mirror_x", "A"),
        ("tool.mirror_y", "G"),
        ("tool.mirror_y", "A"),
    ] {
        done(&mut s, action, json!({}));
        assert_eq!(ghost(&s)["variant"], letter, "{action}");
    }
    // The live region says what floats.
    let said = s.tool_announcement().unwrap_or_default().to_owned();
    assert!(
        said.starts_with("Place ew.iec.contact-no@qet10: new K, variant A"),
        "{said}"
    );
    // Variant C, commit: the representation has orientation C (R180).
    done(&mut s, "tool.next_variant", json!({}));
    done(&mut s, "tool.next_variant", json!({}));
    let before = tables(&s).reps.len();
    let pt1 = sheet(&s, p);
    done(&mut s, "canvas.press", at(pt1, json!({})));
    let pt2 = sheet(&s, p);
    done(&mut s, "canvas.release", at(pt2, json!({})));
    let t = tables(&s);
    assert_eq!(t.reps.len(), before + 1);
    let new = t
        .reps
        .iter()
        .find(|r| r.symbol_key == "ew.iec.contact-no@qet10")
        .unwrap();
    assert_eq!(new.orient, 2, "variant C");
    assert_eq!(new.pos, p);
    // Sticky: the tool keeps floating; a dwell previews the next placement and the ghost shows its tag.
    assert_eq!(s.tool().name(), "place");
    let q = (p.0 + 7_937_500 * 2, p.1);
    let q = grid_point(&s, (60 + 6, 30)).max(q);
    let pt3 = sheet(&s, q);
    done(&mut s, "canvas.pointer", at(pt3, json!({})));
    let calls = s.session_calls();
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(s.session_calls().preview_with, calls.preview_with + 1);
    let tag = ghost(&s)["tag"].as_str().map(str::to_owned);
    let tag = tag.expect("the predicted tag");
    assert!(tag.contains('K'), "{tag}");
    assert!(
        s.tool_overlay().labels().any(|l| l.ends_with(&tag)),
        "the label shows the tag"
    );
    let line = describe_line(&s, "canvas ghost place");
    assert!(
        line.contains("symbol \"ew.iec.contact-no@qet10\" variant \"C\" rep \"multi_line\" at ")
            && line.contains(" mm target new letter \"K\" tag \""),
        "{line}"
    );
    // Escape leaves the revision unchanged.
    let rev = s.revision();
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.revision(), rev);
    assert_eq!(state(&s)["canvas"]["ghost"], Value::Null);
}

#[test]
fn a_contact_dropped_inside_a_black_box_joins_its_device() {
    let tmp = Tmp::new("join");
    let mut s = shell(&tmp, "102");
    let t = tables(&s);
    let bx = t
        .reps
        .iter()
        .filter(|r| r.black_box.is_some())
        .find(|r| {
            t.cps
                .iter()
                .any(|c| c.rep == r.id && c.label.starts_with("-102F2:"))
        })
        .expect("-102F2's box")
        .clone();
    let outline = &bx.black_box.as_ref().unwrap().outline;
    let (x0, x1) = (
        outline.iter().map(|p| p.0).min().unwrap(),
        outline.iter().map(|p| p.0).max().unwrap(),
    );
    let (y0, y1) = (
        outline.iter().map(|p| p.1).min().unwrap(),
        outline.iter().map(|p| p.1).max().unwrap(),
    );
    let inside = ((x0 + x1) / 2, (y0 + y1) / 2);
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    let pt4 = sheet(&s, inside);
    done(&mut s, "canvas.pointer", at(pt4, json!({})));
    assert_eq!(ghost(&s)["target"], "join");
    assert!(
        s.tool_overlay().labels().any(|l| l.contains("join -102F2")),
        "{:?}",
        s.tool_overlay()
    );
    let here = sheet(&s, inside);
    done(&mut s, "canvas.press", at(here, json!({})));
    let t = tables(&s);
    let new = t
        .reps
        .iter()
        .find(|r| r.symbol_key == "ew.iec.contact-no@qet10")
        .expect("placed");
    assert_eq!(new.device, bx.device, "an auxiliary function of -102F2");
}

#[test]
fn a_selection_of_two_representations_moves_by_drag_and_by_keys_in_grid_steps() {
    let tmp = Tmp::new("move");
    let mut s = shell(&tmp, "101");
    let (a, b) = (
        rep_with(&s, "ew.iec.heater@qet10"),
        rep_with(&s, "ew.iec.lamp@qet10"),
    );
    // A click selects E55, Shift+click adds E56.
    let centre = |r: &TableRep| ((r.body[0] + r.body[2]) / 2, (r.body[1] + r.body[3]) / 2);
    let pt5 = sheet(&s, centre(&a));
    click(&mut s, pt5, json!({}));
    let pt6 = sheet(&s, centre(&b));
    click(&mut s, pt6, json!({"shift": true}));
    assert_eq!(s.selection().items.len(), 2);
    let mut pos = (a.pos, b.pos);
    for k in 1..=3i64 {
        let ca = centre(&rep_with(&s, "ew.iec.heater@qet10"));
        let from = sheet(&s, ca);
        let to = steps_from(&s, ca, (k, 0));
        done(&mut s, "canvas.press", at(from, json!({})));
        done(&mut s, "canvas.pointer", at(to, json!({})));
        assert_eq!(s.tool().state(), "dragging");
        let g = ghost(&s);
        assert_eq!(g, json!({"kind": "move", "count": 2, "steps": [k, 0]}));
        let o = s.tool_overlay();
        assert!(o.items.iter().any(|i| matches!(i, OverlayItem::Ghost { .. })));
        assert!(
            o.items.iter().any(|i| matches!(i, OverlayItem::Polyline { .. })),
            "rubber bands"
        );
        let readout = o.labels().next().unwrap().to_owned();
        assert!(
            readout.starts_with("dx ") && readout.ends_with(" dy 0.000 mm"),
            "{readout}"
        );
        let line = describe_line(&s, "canvas ghost move");
        assert_eq!(
            line,
            format!("canvas ghost move 2 steps {k} 0 readout \"{readout}\"")
        );
        done(&mut s, "canvas.release", at(to, json!({})));
        // Each moved to the grid point at its own index plus k, exactly.
        let snap = s.canvas().unwrap().page().snapper.clone();
        let exact = |p: (i64, i64)| (snap.page_of_index(snap.grid_index_of(p.0) + k), p.1);
        let (na, nb) = (
            rep_with(&s, "ew.iec.heater@qet10"),
            rep_with(&s, "ew.iec.lamp@qet10"),
        );
        assert_eq!(na.pos, exact(pos.0), "E55 by {k}");
        assert_eq!(nb.pos, exact(pos.1), "E56 by {k}");
        pos = (na.pos, nb.pos);
        // The selection after the commit is the moved representations.
        assert_eq!(s.selection().items.len(), 2);
        assert_eq!(s.tool().state(), "idle");
    }
    // Escape during a drag cancels it.
    let ca = centre(&rep_with(&s, "ew.iec.heater@qet10"));
    let pt7 = sheet(&s, ca);
    done(&mut s, "canvas.press", at(pt7, json!({})));
    let pt8 = steps_from(&s, ca, (2, 1));
    done(&mut s, "canvas.pointer", at(pt8, json!({})));
    let rev = s.revision();
    done(&mut s, "ui.dismiss", json!({}));
    assert_eq!(s.revision(), rev);
    assert_eq!(s.tool().state(), "idle");
    // The keyboard move: M, two steps down by the cursor keys, Enter.
    done(&mut s, "cursor.set", json!({}));
    done(&mut s, "tool.move", json!({}));
    assert_eq!(s.tool().state(), "moving");
    done(&mut s, "cursor.down", json!({}));
    done(&mut s, "cursor.down", json!({}));
    assert_eq!(ghost(&s)["steps"], json!([0, 2]));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "rep.move");
    done(&mut s, "canvas.activate", json!({}));
    let snap = s.canvas().unwrap().page().snapper.clone();
    let na = rep_with(&s, "ew.iec.heater@qet10");
    assert_eq!(
        na.pos,
        (pos.0.0, snap.page_of_index(snap.grid_index_of(pos.0.1) + 2))
    );
    // Undo takes back one step at a time.
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(rep_with(&s, "ew.iec.heater@qet10").pos, pos.0);
}

#[test]
fn transform_keys_and_delete_act_on_the_selection_with_their_refusals() {
    let tmp = Tmp::new("transform");
    let mut s = shell(&tmp, "101");
    // Nothing selected: M and Delete are refused.
    assert_eq!(refused(&mut s, "tool.move", json!({})), "reason-nothing-to-move");
    assert_eq!(
        refused(&mut s, "edit.delete_selection", json!({})),
        "reason-nothing-selected"
    );
    assert_eq!(
        refused(&mut s, "tool.next_rep_type", json!({})),
        "reason-place-only"
    );
    let (a, b) = (
        rep_with(&s, "ew.iec.heater@qet10"),
        rep_with(&s, "ew.iec.lamp@qet10"),
    );
    let centre = |r: &TableRep| ((r.body[0] + r.body[2]) / 2, (r.body[1] + r.body[3]) / 2);
    let pt9 = sheet(&s, centre(&a));
    click(&mut s, pt9, json!({}));
    // R on one representation: a quarter turn.
    done(&mut s, "tool.rotate_cw", json!({}));
    assert_eq!(rep_with(&s, "ew.iec.heater@qet10").orient, 1);
    // V: the next letter (B → C).
    done(&mut s, "tool.next_variant", json!({}));
    assert_eq!(rep_with(&s, "ew.iec.heater@qet10").orient, 2);
    let pt10 = sheet(&s, centre(&rep_with(&s, "ew.iec.lamp@qet10")));
    click(&mut s, pt10, json!({"shift": true}));
    assert_eq!(refused(&mut s, "tool.next_variant", json!({})), "reason-one-rep");
    // X on two: both mirrored.
    done(&mut s, "tool.mirror_x", json!({}));
    assert_eq!(rep_with(&s, "ew.iec.lamp@qet10").orient, b.orient + 4);
    // Delete: both gone, the selection follows.
    let before = tables(&s).reps.len();
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(tables(&s).reps.len(), before - 2);
    assert!(s.selection().is_empty(), "deleted records leave the selection");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(tables(&s).reps.len(), before);
}

#[test]
fn change_symbol_previews_and_commits_on_one_representation() {
    let tmp = Tmp::new("exchange");
    let mut s = shell(&tmp, "101");
    let e56 = rep_with(&s, "ew.iec.lamp@qet10");
    let centre = ((e56.body[0] + e56.body[2]) / 2, (e56.body[1] + e56.body[3]) / 2);
    assert_eq!(
        refused(&mut s, "rep.exchange", json!({})),
        "reason-nothing-selected"
    );
    let pt11 = sheet(&s, centre);
    click(&mut s, pt11, json!({}));
    // The picker in exchange mode lists allowed symbols only.
    done(&mut s, "rep.exchange", json!({}));
    assert_eq!(state(&s)["canvas"]["picker"]["mode"], "exchange");
    done(&mut s, "ui.dismiss", json!({}));
    // A symbol of another function definition with no CP matching by designation: the preview is refused because a
    // symbol CP would draw a function CP by no decision of the map's (ruling 5), which leaves no CP unmapped: the
    // ghost names none (fix round, item 4).
    done(&mut s, "rep.exchange", json!({"symbol": "ew.iec.heater@qet10"}));
    assert_eq!(s.tool().name(), "exchange");
    assert_eq!(s.tool().state(), "armed");
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(state(&s)["canvas"]["preview"]["state"], "failed");
    let g = ghost(&s);
    assert_eq!(g["kind"], "exchange");
    assert_eq!(g["symbol"], "ew.iec.heater@qet10");
    assert_eq!(g["unmapped"], json!([]));
    assert_eq!(
        describe_line(&s, "canvas ghost exchange"),
        "canvas ghost exchange symbol \"ew.iec.heater@qet10\" unmapped none"
    );
    done(&mut s, "ui.dismiss", json!({}));
    // Its own symbol again: mapped by index, nothing unmapped; the commit keeps both connections.
    let pt12 = sheet(&s, centre);
    click(&mut s, pt12, json!({}));
    done(&mut s, "rep.exchange", json!({"symbol": "ew.iec.lamp@qet10"}));
    assert_eq!(ghost(&s)["unmapped"], json!([]));
    let connections = tables(&s).connections.len();
    done(&mut s, "canvas.press", at(pt12, json!({})));
    assert_eq!(tables(&s).connections.len(), connections);
}

#[test]
fn pointer_and_cursor_moves_with_a_tool_armed_and_picker_filtering_call_no_session() {
    let tmp = Tmp::new("counts");
    let mut s = shell(&tmp, "101");
    // The first frame of the page has fetched its content.
    done(&mut s, "tool.place", json!({}));
    let calls = s.session_calls();
    for text in ["m", "mc", "mcb", ""] {
        done(&mut s, "picker.filter", json!({ "text": text }));
        done(&mut s, "picker.row", json!({"step": 1}));
    }
    assert_eq!(
        s.session_calls(),
        calls,
        "the picker filters without a Session call"
    );
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"}));
    let calls = s.session_calls();
    for i in 0..20 {
        let p = grid_point(&s, (40 + i, 20 + i / 3));
        let pt13 = sheet(&s, p);
        done(&mut s, "canvas.pointer", at(pt13, json!({})));
    }
    done(&mut s, "cursor.set", json!({}));
    for _ in 0..5 {
        done(&mut s, "cursor.right", json!({}));
        done(&mut s, "tool.rotate_cw", json!({}));
    }
    assert_eq!(
        s.session_calls(),
        calls,
        "pointer, cursor and key moves call no Session"
    );
    // A dwell previews exactly once; a commit commits exactly once; the next frame fetches the page once.
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    done(&mut s, "canvas.dwell", json!({"ms": 120}));
    let after = s.session_calls();
    assert_eq!(after.preview_with, calls.preview_with + 1);
    done(&mut s, "canvas.activate", json!({}));
    let c = s.session_calls();
    assert_eq!(
        c.run_script,
        after.run_script + 1,
        "headless commits run one command log"
    );
    assert_eq!(c.page_content, after.page_content + 1);
    assert_eq!(c.page_tables, after.page_tables + 1);
    assert_eq!(
        c.symbol_shape,
        after.symbol_shape + 1,
        "the floating shape once for the new revision"
    );
    // A drag of a representation with Select: pointer moves call no Session; the release commits once.
    done(&mut s, "ui.dismiss", json!({}));
    let e55 = rep_with(&s, "ew.iec.heater@qet10");
    let centre = ((e55.body[0] + e55.body[2]) / 2, (e55.body[1] + e55.body[3]) / 2);
    let from = sheet(&s, centre);
    done(&mut s, "canvas.press", at(from, json!({})));
    let calls = s.session_calls();
    for k in 1..=6 {
        let to = steps_from(&s, centre, (k, k % 2));
        done(&mut s, "canvas.pointer", at(to, json!({})));
    }
    assert_eq!(s.tool().state(), "dragging");
    assert_eq!(s.session_calls(), calls, "a drag's moves call no Session");
    let to = steps_from(&s, centre, (6, 0));
    done(&mut s, "canvas.release", at(to, json!({})));
    assert_eq!(s.session_calls().run_script, calls.run_script + 1);
}

#[test]
fn a_terminal_joins_strip_x1031_with_its_tag_predicted_and_the_designation_advanced() {
    let tmp = Tmp::new("terminal");
    let mut s = shell(&tmp, "103");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
    );
    let p = grid_point(&s, (70, 40));
    let pt14 = sheet(&s, p);
    done(&mut s, "canvas.pointer", at(pt14, json!({})));
    let g = ghost(&s);
    assert_eq!(g["target"], "terminal");
    assert!(
        s.tool_overlay().labels().any(|l| l == "A terminal -X1031:30"),
        "{:?}",
        s.tool_overlay()
    );
    assert!(describe_line(&s, "canvas ghost place").contains("target terminal strip \"-X1031:30\""));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(state(&s)["canvas"]["preview"]["state"], "ready");
    assert!(ghost(&s)["tag"].is_string(), "{g}");
    let before = tables(&s).reps.len();
    // Enter at the grid cursor on the same point.
    let here = sheet(&s, p);
    done(&mut s, "cursor.set", at(here, json!({})));
    done(&mut s, "canvas.activate", json!({}));
    assert_eq!(tables(&s).reps.len(), before + 1);
    // Sticky with the next designation.
    assert!(describe_line(&s, "canvas ghost place").contains("strip \"-X1031:31\""));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(tables(&s).reps.len(), before);
}

/// The structural tokens of a describe or transcript line: every token outside quotes up to a free-text marker
/// (which counts, the rest of the line being user text), numbers and punctuation left out.
fn structural_tokens(line: &str) -> Vec<String> {
    let mut outside = String::new();
    let mut quoted = false;
    for c in line.chars() {
        if c == '"' {
            quoted = !quoted;
            outside.push(' ');
        } else if !quoted {
            outside.push(c);
        }
    }
    let outside = outside.replace(['(', ')', '[', ']', ',', '|', '%'], " ");
    let mut words = Vec::new();
    for token in outside.split_whitespace() {
        if ew_viewmodel::describe::FREE_TEXT_MARKERS.contains(&token) {
            words.push(token.to_owned());
            break;
        }
        let number = token.parse::<f64>().is_ok() || token.chars().all(|c| !c.is_alphanumeric());
        if !number {
            words.push(token.to_owned());
        }
    }
    words
}

/// §7.7's token check over real output: every structural token of the canvas lines is a vocabulary word or a
/// free-text marker, and the states below reach every one of part 1's 28 words. Each step must succeed.
#[test]
fn every_part_1_word_of_the_describe_text_is_reached_and_in_the_vocabulary() {
    let tmp = Tmp::new("words");
    let mut texts: Vec<String> = Vec::new();
    // Folio 101: the picker (symbols and parts with a main template entry), a starter symbol floating with its
    // predicted tag, another representation of an existing function, a drag and the keyboard move, change symbol.
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "picker.filter", json!({"text": "mcb"}));
    texts.push(s.describe());
    done(&mut s, "picker.filter", json!({"text": "contact-no@"}));
    texts.push(s.describe());
    done(&mut s, "picker.choose", json!({"letter": "K"}));
    let p = free_point(&s);
    let here = sheet(&s, p);
    done(&mut s, "canvas.pointer", at(here, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    texts.push(s.describe());
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.place_part", json!({}));
    texts.push(s.describe());
    done(&mut s, "ui.dismiss", json!({}));
    let e56 = rep_with(&s, "ew.iec.lamp@qet10");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.lamp@qet10", "function": e56.functions[0].to_string()}),
    );
    done(&mut s, "canvas.pointer", at(here, json!({})));
    texts.push(s.describe());
    done(&mut s, "ui.dismiss", json!({}));
    let centre = ((e56.body[0] + e56.body[2]) / 2, (e56.body[1] + e56.body[3]) / 2);
    let c = sheet(&s, centre);
    click(&mut s, c, json!({}));
    done(&mut s, "canvas.press", at(c, json!({})));
    let to = steps_from(&s, centre, (2, 0));
    done(&mut s, "canvas.pointer", at(to, json!({})));
    texts.push(s.describe());
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "cursor.set", json!({}));
    done(&mut s, "tool.move", json!({}));
    texts.push(s.describe());
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "rep.exchange", json!({"symbol": "ew.iec.heater@qet10"}));
    texts.push(s.describe());
    done(&mut s, "ui.dismiss", json!({}));
    s.shutdown();
    // Folio 103: a terminal of strip -X1031 (`terminal`, `strip`).
    let tmp3 = Tmp::new("words3");
    let mut s = shell(&tmp3, "103");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
    );
    let p = grid_point(&s, (70, 40));
    let here = sheet(&s, p);
    done(&mut s, "canvas.pointer", at(here, json!({})));
    texts.push(s.describe());
    s.shutdown();
    // Folio 102: a contact inside -102F2's box (`join`).
    let tmp2 = Tmp::new("words2");
    let mut s = shell(&tmp2, "102");
    let t = tables(&s);
    let bx = t
        .reps
        .iter()
        .filter(|r| r.black_box.is_some())
        .find(|r| {
            t.cps
                .iter()
                .any(|c| c.rep == r.id && c.label.starts_with("-102F2:"))
        })
        .expect("-102F2's box")
        .clone();
    let o = &bx.black_box.as_ref().unwrap().outline;
    let mid = (
        (o.iter().map(|p| p.0).min().unwrap() + o.iter().map(|p| p.0).max().unwrap()) / 2,
        (o.iter().map(|p| p.1).min().unwrap() + o.iter().map(|p| p.1).max().unwrap()) / 2,
    );
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    let here = sheet(&s, mid);
    done(&mut s, "canvas.pointer", at(here, json!({})));
    texts.push(s.describe());
    s.shutdown();
    // Every structural token of the canvas lines (the lines of WP-S2.03 and WP-S2.04's tools) is in the
    // vocabulary or a free-text marker.
    let vocabulary = ew_viewmodel::describe::VOCABULARY;
    let markers = ew_viewmodel::describe::FREE_TEXT_MARKERS;
    let mut seen = std::collections::BTreeSet::new();
    for text in &texts {
        for line in text
            .lines()
            .filter(|l| l.starts_with("canvas ") || l.starts_with("  row "))
        {
            for token in structural_tokens(line) {
                assert!(
                    vocabulary.contains(&token.as_str()) || markers.contains(&token.as_str()),
                    "`{token}` is no vocabulary word; line: {line}"
                );
                seen.insert(token);
            }
        }
    }
    let missing: Vec<&&str> = ew_viewmodel::describe::PLACE_WORDS
        .iter()
        .filter(|w| !seen.contains(**w))
        .collect();
    assert!(missing.is_empty(), "words no state reached: {missing:?}");
}

#[test]
fn a_refused_commit_keeps_the_designation_and_a_non_decimal_one_asks_again() {
    let tmp = Tmp::new("designation");
    let mut s = shell(&tmp, "103");
    let p = grid_point(&s, (70, 40));
    let here = sheet(&s, p);
    // Designation 1 is used in -X1031: the commit is refused and the designation stays 1.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "1"}),
    );
    done(&mut s, "canvas.pointer", at(here, json!({})));
    let before = tables(&s).reps.len();
    assert!(matches!(
        s.dispatch("canvas.press", &at(here, json!({}))),
        Outcome::Failed { .. }
    ));
    assert_eq!(tables(&s).reps.len(), before);
    assert!(
        describe_line(&s, "canvas ghost place").contains("strip \"-X1031:1\""),
        "kept"
    );
    done(&mut s, "ui.dismiss", json!({}));
    // A decimal designation advances after the commit.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "30"}),
    );
    done(&mut s, "canvas.press", at(here, json!({})));
    assert!(describe_line(&s, "canvas ghost place").contains("strip \"-X1031:31\""));
    done(&mut s, "ui.dismiss", json!({}));
    // A designation that is not decimal digits: after the commit the tool asks again; a click is refused until
    // `text.input` gives the next one.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "PE7"}),
    );
    let q = grid_point(&s, (76, 40));
    let there = sheet(&s, q);
    done(&mut s, "canvas.press", at(there, json!({})));
    assert!(describe_line(&s, "canvas ghost place").contains("strip \"-X1031:?\""));
    let count = tables(&s).reps.len();
    assert_eq!(
        refused(&mut s, "canvas.press", at(there, json!({}))),
        "reason-designation-needed"
    );
    assert_eq!(tables(&s).reps.len(), count);
    done(&mut s, "text.input", json!({"text": "PE8"}));
    let r = grid_point(&s, (82, 40));
    let yonder = sheet(&s, r);
    done(&mut s, "canvas.press", at(yonder, json!({})));
    assert_eq!(tables(&s).reps.len(), count + 1);
}

/// FT-421: a terminal symbol without a strip opens the picker's strip chooser (every strip of the project by tag, the
/// next free designation proposed); a strip made while it is open is listed and current; the tag comes from the model.
#[test]
fn a_terminal_symbol_asks_for_its_strip_and_proposes_the_next_designation() {
    let tmp = Tmp::new("strips");
    let mut s = shell(&tmp, "103");
    done(&mut s, "place.pick", json!({"symbol": "ew.iec.terminal@qet10"}));
    let picker = state(&s)["canvas"]["picker"].clone();
    assert_eq!(picker["mode"], "strip");
    let text = s.describe();
    assert!(text.contains("  row strip \"-X1031\" terminal \"21\""), "{text}");
    assert!(text.contains("  row strip \"-X1032\" terminal \"6\""), "{text}");
    done(&mut s, "picker.filter", json!({"text": "1032"}));
    assert_eq!(s.picker().unwrap().rows.len(), 1);
    done(&mut s, "picker.choose", json!({}));
    assert!(s.picker().is_none());
    assert_eq!(s.tool().name(), "place");
    let here = sheet(&s, grid_point(&s, (82, 40)));
    done(&mut s, "canvas.pointer", at(here, json!({})));
    let line = describe_line(&s, "canvas ghost place");
    assert!(line.contains("strip \"-X1032:6\""), "{line}");
    done(&mut s, "ui.dismiss", json!({}));
    // A new strip while the chooser is open: listed, current, and its tag is the model's.
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "designation": "1"}),
    );
    let before = s.picker().unwrap().rows.len();
    done(&mut s, "strip.new", json!({"letter": "X"}));
    let p = s.picker().expect("the chooser stays open");
    assert_eq!(p.rows.len(), before + 1);
    let ew_viewmodel::tools::PickerRow::Strip(k) = p.rows[p.row] else {
        panic!("a strip row")
    };
    let tag = p.catalogue.strips[k].tag.clone();
    assert_eq!(p.catalogue.strips[k].next_designation, "1");
    done(&mut s, "picker.choose", json!({}));
    done(&mut s, "canvas.pointer", at(here, json!({})));
    let line = describe_line(&s, "canvas ghost place");
    assert!(line.contains(&format!("strip \"{tag}:1\"")), "{line}");
    done(&mut s, "canvas.press", at(here, json!({})));
    // The first terminal numbers the strip: its CP label prints the strip's tag, which the tool reads from the model
    // after the commit; the next placement proposes designation 2.
    let t = tables(&s);
    let placed = t
        .reps
        .iter()
        .find(|r| r.symbol_key == "ew.iec.terminal@qet10" && r.pos == grid_point(&s, (82, 40)))
        .expect("placed");
    let label = t
        .cps
        .iter()
        .find(|c| c.rep == placed.id)
        .map(|c| c.label.clone())
        .expect("a CP label");
    let (numbered, rest) = label.split_once(':').expect("tag:designation");
    assert!(rest.starts_with('1'), "{label}");
    assert_ne!(numbered, tag, "numbered by its first terminal");
    let line = describe_line(&s, "canvas ghost place");
    assert!(line.contains(&format!("strip \"{numbered}:2\"")), "{line}");
}

#[test]
fn a_page_type_without_symbol_tools_refuses_the_editing_actions() {
    let tmp = Tmp::new("pagetype");
    let mut s = shell(&tmp, "1");
    let kind = s.page_rows()[s.page_index()].kind.clone();
    assert_eq!(kind, "title_page", "folio 1 is the title page");
    for action in [
        "tool.place",
        "tool.place_part",
        "strip.new",
        "tool.move",
        "edit.delete_selection",
    ] {
        assert_eq!(
            refused(&mut s, action, json!({})),
            "reason-page-type-no-symbols",
            "{action}"
        );
    }
    assert_eq!(
        refused(&mut s, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"})),
        "reason-page-type-no-symbols"
    );
}

/// A pending keyboard move does not survive Delete or undo (fix round, item 11): Delete cancels the move first and
/// deletes; two undos bring the representation back with no ghost, no pending move, no preview and no banner.
#[test]
fn a_pending_keyboard_move_is_cancelled_by_delete_and_undo() {
    let tmp = Tmp::new("pending");
    let mut s = shell(&tmp, "101");
    let e55 = rep_with(&s, "ew.iec.heater@qet10");
    let centre = ((e55.body[0] + e55.body[2]) / 2, (e55.body[1] + e55.body[3]) / 2);
    let c = sheet(&s, centre);
    click(&mut s, c, json!({}));
    done(&mut s, "cursor.set", json!({}));
    done(&mut s, "tool.move", json!({}));
    done(&mut s, "cursor.right", json!({}));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(s.tool().state(), "moving");
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "rep.move");
    let before = tables(&s).reps.len();
    done(&mut s, "edit.delete_selection", json!({}));
    assert_eq!(tables(&s).reps.len(), before - 1, "deleted");
    assert_eq!(s.tool().state(), "idle", "the move is cancelled");
    assert_eq!(ghost(&s), Value::Null);
    assert_eq!(state(&s)["canvas"]["preview"], Value::Null);
    assert!(s.tool_overlay().is_empty());
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(tables(&s).reps.len(), before);
    assert!(matches!(
        s.dispatch("edit.undo", &json!({})),
        Outcome::Done | Outcome::Refused { .. }
    ));
    assert_eq!(ghost(&s), Value::Null);
    assert_eq!(s.tool().state(), "idle");
    assert_eq!(state(&s)["canvas"]["preview"], Value::Null);
    assert_eq!(s.banner(), None, "no banner");
    // A move pending while an undo changes the page is cancelled too.
    let e56 = rep_with(&s, "ew.iec.lamp@qet10");
    let c = sheet(
        &s,
        ((e56.body[0] + e56.body[2]) / 2, (e56.body[1] + e56.body[3]) / 2),
    );
    click(&mut s, c, json!({}));
    done(&mut s, "tool.rotate_cw", json!({}));
    done(&mut s, "tool.move", json!({}));
    done(&mut s, "cursor.down", json!({}));
    assert_eq!(s.tool().state(), "moving");
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.tool().state(), "idle");
    assert_eq!(ghost(&s), Value::Null);
    assert!(s.tool_overlay().is_empty());
}
