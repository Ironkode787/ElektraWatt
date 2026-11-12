// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the view model: the
//! picker and the starter embedding, the Place tool, moving by drag and by keys, transform and delete on a
//! selection, change symbol, the transcript members and the describe lines with their 28 words, and the exact
//! `Session` counts of pointer and cursor moves. Every test works on a writable temporary copy of Golden A.

use crate::common;
use ew_viewmodel::key;

use common::{Tmp, at, click, done, grid_point, refused, sheet, state, tables};
use ew_viewmodel::ew_api::output::TableRep;
use ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use ew_viewmodel::tools::OverlayItem;
use serde_json::{Value, json};

/// A headless shell with a writable copy of Golden A on `folio`.
fn shell(tmp: &Tmp, folio: &str) -> ShellModel {
    tmp.shell_in(Mode::Headless, true, folio)
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

/// The sheet point `k` grid steps from the sheet point of the page point `p`.
fn steps_from(shell: &ShellModel, p: (i64, i64), k: (i64, i64)) -> (i64, i64) {
    let s = &shell.canvas().unwrap().page().snapper;
    let (ix, iy) = (s.grid_index_of(p.0), s.grid_index_of(p.1));
    (
        s.sheet_of_index(ix + k.0, 0) - s.sheet_of_index(ix, 0) + s.to_sheet(p.0, 0),
        s.sheet_of_index(iy + k.1, 1) - s.sheet_of_index(iy, 1) + s.to_sheet(p.1, 1),
    )
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
fn the_picker_lists_the_grid_family_places_a_starter_symbol_in_one_step_and_disables_rows_with_reasons() {
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
        text.contains("  row symbol \"mcb-1p\" embedded \"ew.iec.mcb-1p@qet10\""),
        "{text}"
    );
    // A black-box symbol row is enabled: choosing it arms the black box tool
    // (`tests/it/ip_and_black_box.rs`).
    done(&mut s, "picker.filter", json!({"text": "blackbox"}));
    let line = describe_line(&s, "  row symbol \"blackbox\"");
    assert!(!line.contains("disabled"), "{line}");
    // Rows move and clamp.
    done(&mut s, "picker.filter", json!({"text": ""}));
    let n = s.picker().unwrap().rows.len();
    done(&mut s, "picker.row", json!({"to": -1}));
    assert_eq!(s.picker().unwrap().row, n - 1);
    done(&mut s, "picker.row", json!({"step": 5}));
    assert_eq!(s.picker().unwrap().row, n - 1);
    done(&mut s, "picker.row", json!({"to": 0}));
    // The signature's words work without `step`.
    done(&mut s, "picker.row", json!({"to": "last"}));
    assert_eq!(s.picker().unwrap().row, n - 1);
    done(&mut s, "picker.row", json!({"to": "first"}));
    assert_eq!(s.picker().unwrap().row, 0);
    // A word outside the signature is refused by name.
    match s.dispatch("picker.row", &json!({"to": "middle"})) {
        Outcome::Failed { message } => assert!(message.contains("`to`"), "{message}"),
        other => panic!("{other:?}"),
    }
    // A starter symbol (Golden A has no NO contact): choosing it commits nothing, the tool floats it (FT-656).
    done(&mut s, "picker.filter", json!({"text": "contact-no@"}));
    assert!(describe_line(&s, "  row symbol \"contact-no\"").contains("starter"));
    let undo_before = s.can_undo();
    let calls = s.session_calls();
    let rev = s.revision();
    done(&mut s, "picker.choose", json!({"letter": "K"}));
    assert!(s.picker().is_none());
    assert_eq!(s.tool().name(), "place");
    assert_eq!(s.tool().state(), "floating");
    assert!(!undo_before && !s.can_undo(), "choosing is no undo step");
    assert_eq!(s.session_calls().run_script, calls.run_script, "no host commit");
    assert_eq!(s.revision(), rev);
    // The placement embeds the symbol and its FnDef with the device: one undo step.
    let p = free_point(&s);
    let pt = sheet(&s, p);
    done(&mut s, "canvas.press", at(pt, json!({})));
    done(&mut s, "canvas.release", at(pt, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one commit");
    let place = s.recorded_commands().last().expect("the placement");
    assert_eq!(place.cmd, "device.place");
    let kinds: Vec<&str> = place.params["copies"]
        .as_array()
        .expect("copies")
        .iter()
        .map(|c| c["$k"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(kinds, ["symbol", "fndef"]);
    // A second placement with the same tool carries no copies: the project holds them now.
    let qt = sheet(&s, steps_from(&s, p, (8, 0)));
    done(&mut s, "canvas.press", at(qt, json!({})));
    done(&mut s, "canvas.release", at(qt, json!({})));
    assert_eq!(s.revision(), rev.map(|r| r + 2), "a second commit");
    let again = s.recorded_commands().last().expect("the second placement");
    assert_eq!(again.cmd, "device.place");
    assert!(again.params.get("copies").is_none(), "{:?}", again.params);
    // Two undo steps take back both placements and the embedding: the row is a starter row again.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "edit.undo", json!({}));
    done(&mut s, "edit.undo", json!({}));
    assert!(!s.can_undo(), "two placements, two undo steps");
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "picker.filter", json!({"text": "contact-no@"}));
    assert!(describe_line(&s, "  row symbol \"contact-no\"").contains("starter"));
    done(&mut s, "ui.dismiss", json!({}));
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
    assert!(overlay.labels().any(|l| l == "Variant A · new K"), "{overlay:?}");
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
    // symbol CP would draw a function CP by no decision of the map's, which leaves no CP unmapped: the
    // ghost names none, before the dwell too: the map comes from the two shapes.
    done(&mut s, "rep.exchange", json!({"symbol": "ew.iec.heater@qet10"}));
    assert_eq!(s.tool().name(), "exchange");
    assert_eq!(s.tool().state(), "armed");
    assert_eq!(ghost(&s)["unmapped"], json!([]));
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
    let browse = |s: &mut ShellModel| {
        for text in ["m", "mc", "mcb", ""] {
            done(s, "picker.filter", json!({ "text": text }));
            done(s, "picker.row", json!({"step": 1}));
        }
    };
    browse(&mut s);
    // The browser's picture reads the shape of each symbol it shows, once per revision; nothing else.
    let shown = s.session_calls();
    assert_eq!(
        ew_viewmodel::shell::SessionCalls {
            symbol_shape: calls.symbol_shape,
            ..shown
        },
        calls,
        "the picker filters without a Session call"
    );
    browse(&mut s);
    assert_eq!(s.session_calls(), shown, "the shapes shown are cached");
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

/// The Place tool's label names a strip without a tag yet in words, never by its
/// placeholder tag.
#[test]
fn the_place_label_names_a_strip_without_a_tag_in_words() {
    let tmp = Tmp::new("terminal-untagged");
    let mut s = shell(&tmp, "103");
    done(&mut s, "strip.new", json!({}));
    done(&mut s, "navigator.show", json!({"kind": "strips"}));
    let tag = s
        .nav(ew_viewmodel::navigators::NavKind::Strips)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == ew_viewmodel::navigators::NavRole::Strip && n.label.contains('?'))
                .map(|n| n.label.clone())
        })
        .expect("the new strip");
    // The preview chip's "before" of the strip reads in words; a device's "?" stays as it is.
    let strip_id = s
        .nav(ew_viewmodel::navigators::NavKind::Strips)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.label == tag)
                .and_then(|n| n.records.first().copied())
        })
        .expect("the strip's record");
    assert_eq!(
        s.device_tag_label(strip_id, &tag),
        "New terminal strip · no tag yet"
    );
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": tag}),
    );
    let p = grid_point(&s, (70, 40));
    let pt = sheet(&s, p);
    done(&mut s, "canvas.pointer", at(pt, json!({})));
    let labels =
        |s: &ShellModel| -> Vec<String> { s.tool_overlay().labels().map(|l| l.to_string()).collect() };
    let before = labels(&s);
    assert!(
        before
            .iter()
            .any(|l| l == "Variant A \u{b7} terminal 1 on the new strip (no tag yet)"),
        "{before:?}"
    );
    assert!(!before.iter().any(|l| l.contains('?')), "{before:?}");
    // Once the preview predicts the strip's tag, the label names the strip by it, as one phrase.
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    let tag = ghost(&s)["tag"].as_str().expect("the predicted tag").to_owned();
    let strip = tag.rsplit_once(':').expect("<strip>:<terminal>").0.to_owned();
    assert!(!strip.contains('?'), "{tag}");
    let after = labels(&s);
    assert!(
        after
            .iter()
            .any(|l| *l == format!("Variant A \u{b7} terminal 1 on {strip} (new)")),
        "{after:?}"
    );
    done(&mut s, "view.language", json!({"value": "nl"}));
    done(&mut s, "canvas.pointer", at(pt, json!({})));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    let nl = labels(&s);
    assert!(
        nl.iter()
            .any(|l| *l == format!("Variant A \u{b7} klem 1 op {strip} (nieuw)")),
        "{nl:?}"
    );
    done(&mut s, "view.language", json!({"value": "en"}));
    // A device that is no strip keeps its "?" as it is (folio 101's relay).
    done(&mut s, "tool.select", json!({}));
    done(&mut s, "page.goto", json!({"folio": "101"}));
    let device = tables(&s)
        .reps
        .iter()
        .filter(|r| !r.terminal)
        .find_map(|r| r.device)
        .expect("a device on 101");
    assert_eq!(s.device_tag_label(device, "-?K?"), "-?K?");
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
        s.tool_overlay()
            .labels()
            .any(|l| l == "Variant A · terminal 30 on -X1031"),
        "{:?}",
        s.tool_overlay()
    );
    assert!(describe_line(&s, "canvas ghost place").contains("target terminal strip \"-X1031:30\""));
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(state(&s)["canvas"]["preview"]["state"], "ready");
    assert!(ghost(&s)["tag"].is_string(), "{g}");
    // The whole label after the preview, the terminal's tag not added to it.
    let labels: Vec<String> = s.tool_overlay().labels().map(|l| l.to_string()).collect();
    assert_eq!(labels, ["Variant A · terminal 30 on -X1031"]);
    // View ▸ Language relabels it at once, before the pointer moves.
    done(&mut s, "view.language", json!({"value": "nl"}));
    let labels: Vec<String> = s.tool_overlay().labels().map(|l| l.to_string()).collect();
    assert_eq!(labels, ["Variant A · klem 30 op -X1031"]);
    done(&mut s, "view.language", json!({"value": "en"}));
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

/// FT-1546: the counter fills a gap and then skips the designations in use. -X1031's lowest free designation is 21;
/// with 22 placed first, the tool proposes 21, and after placing it proposes 23, not 22 (which the command refuses).
#[test]
fn the_terminal_counter_skips_a_designation_in_use_after_filling_a_gap() {
    let tmp = Tmp::new("gap");
    let mut s = shell(&tmp, "103");
    let first = sheet(&s, grid_point(&s, (70, 40)));
    let second = sheet(&s, grid_point(&s, (76, 40)));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031", "designation": "22"}),
    );
    done(&mut s, "canvas.press", at(first, json!({})));
    done(&mut s, "ui.dismiss", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1031"}),
    );
    assert!(describe_line(&s, "canvas ghost place").contains("strip \"-X1031:21\""));
    let count = tables(&s).reps.len();
    done(&mut s, "canvas.press", at(second, json!({})));
    assert_eq!(tables(&s).reps.len(), count + 1);
    let line = describe_line(&s, "canvas ghost place");
    assert!(line.contains("strip \"-X1031:23\""), "{line}");
}

/// A terminal symbol without a strip opens the picker's strip chooser (every strip of the project by tag, the
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
    // The toast's example is the tag the first terminal gives the strip on this page (checked below).
    let toast = state(&s)["toast"]["text"].as_str().unwrap_or_default().to_owned();
    assert_eq!(
        toast,
        "New terminal strip; its first terminal tags it, such as -X1033: Draw ▸ Terminal… places its terminals."
    );
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
    assert_eq!(numbered, "-X1033", "the toast's example");
    let line = describe_line(&s, "canvas ghost place");
    assert!(line.contains(&format!("strip \"{numbered}:2\"")), "{line}");
}

/// After an Undo of a placed terminal the Place tool proposes the number the Undo freed, not the
/// one after it, and after a Redo the number after the one redone; a strip keeps no gap.
#[test]
fn after_an_undo_the_place_tool_proposes_the_number_it_freed() {
    let tmp = Tmp::new("terminal-undo");
    let mut s = shell(&tmp, "103");
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.terminal@qet10", "strip": "-X1032"}),
    );
    let (a, b) = (
        sheet(&s, grid_point(&s, (82, 40))),
        sheet(&s, grid_point(&s, (86, 40))),
    );
    click(&mut s, a, json!({}));
    click(&mut s, b, json!({}));
    done(&mut s, "canvas.pointer", at(a, json!({})));
    assert!(describe_line(&s, "canvas ghost place").contains("strip \"-X1032:8\""));
    done(&mut s, "edit.undo", json!({}));
    assert_eq!(s.tool().name(), "place");
    done(&mut s, "canvas.pointer", at(b, json!({})));
    assert_eq!(
        describe_line(&s, "canvas ghost place"),
        "canvas ghost place symbol \"ew.iec.terminal@qet10\" variant \"A\" rep \"multi_line\" at 227.542 105.833 mm \
         target terminal strip \"-X1032:7\""
    );
    done(&mut s, "edit.redo", json!({}));
    done(&mut s, "canvas.pointer", at(a, json!({})));
    assert!(describe_line(&s, "canvas ghost place").contains("strip \"-X1032:8\""));
}

/// A terminal placed from a part whose template names no clamps (EW-GEN-TRM-1) is named
/// by its own tag, the strip's and its designation (`-X1032:7`), in the page text and the placement's toast, not by the
/// strip's tag alone; a clamped terminal of Golden A keeps `-X1031:<n>`.
#[test]
fn a_terminal_placed_from_a_part_is_named_by_its_own_tag() {
    let tmp = Tmp::new("terminal-part");
    let mut s = shell(&tmp, "103");
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-TRM-1", "strip": "-X1032", "designation": "7"}),
    );
    let here = sheet(&s, grid_point(&s, (82, 40)));
    click(&mut s, here, json!({}));
    let toast = state(&s)["toast"]["text"].as_str().unwrap_or_default().to_owned();
    assert!(toast.starts_with("Placed -X1032 terminal 7 at "), "{toast}");
    done(&mut s, "page.describe", json!({"folio": "103"}));
    let text = s.take_page_text().expect("page.describe leaves its text");
    let reps: Vec<&str> = text.lines().filter(|l| l.starts_with("  rep ")).collect();
    assert!(
        reps.iter().any(|l| l.starts_with("  rep \"-X1032:7\" ")),
        "{reps:#?}"
    );
    assert!(
        !reps.iter().any(|l| l.starts_with("  rep \"-X1032\" ")),
        "{reps:#?}"
    );
    // Golden A's clamped terminals are named as before (its devices: `page_describe`'s expected texts).
    assert!(
        reps.iter().any(|l| l.starts_with("  rep \"-X1031:11\" ")),
        "{reps:#?}"
    );
}

/// While the strip chooser is open the hint says it waits for a strip, and a press on
/// the canvas starts no rubber band (refused with the reason, its release dropped); the chooser stays open and a
/// chosen strip arms Place as before.
#[test]
fn the_strip_chooser_says_what_it_waits_for_and_a_canvas_press_starts_nothing() {
    let tmp = Tmp::new("strip-chooser");
    let mut s = shell(&tmp, "103");
    let select_hint = state(&s)["status"][0].clone();
    done(&mut s, "place.pick", json!({"symbol": "ew.iec.terminal@qet10"}));
    assert_eq!(state(&s)["canvas"]["picker"]["mode"], "strip");
    let hint = ew_viewmodel::strings::text(key!("hint-choose-strip"));
    assert_eq!(state(&s)["status"][0], hint);
    assert_eq!(s.tool_hint(), hint);
    let here = sheet(&s, free_point(&s));
    let away = sheet(&s, grid_point(&s, (70, 40)));
    assert_eq!(
        s.dispatch("canvas.press", &at(here, json!({}))),
        Outcome::Refused {
            reason_key: key!("reason-choose-strip")
        }
    );
    done(&mut s, "canvas.pointer", at(away, json!({})));
    done(&mut s, "canvas.release", at(away, json!({})));
    let st = state(&s);
    assert_eq!(st["canvas"]["tool"]["state"], "idle", "{st}");
    assert_eq!(st["canvas"]["selection"], json!([]), "{st}");
    assert_eq!(st["canvas"]["picker"]["mode"], "strip", "the chooser stays open");
    // Escape closes it and the tool's own hint returns; a chosen strip arms Place.
    done(&mut s, "ui.dismiss", json!({}));
    assert!(s.picker().is_none());
    assert_eq!(state(&s)["status"][0], select_hint);
    done(&mut s, "place.pick", json!({"symbol": "ew.iec.terminal@qet10"}));
    done(&mut s, "picker.choose", json!({}));
    assert_eq!(s.tool().name(), "place");
    assert_ne!(state(&s)["status"][0], hint);
}

/// The chooser lists the project's own
/// parts first, then the libraries', each by (maker, part number). Placing a terminal from a library part embeds the
/// part, which then stands among the project's own: the chooser opened again has the same rows, in that order.
#[test]
fn the_chooser_lists_the_projects_parts_first_also_after_a_placement_embeds_one() {
    use ew_viewmodel::tools::PickerRow;
    let tmp = Tmp::new("ft1087-order");
    let mut s = shell(&tmp, "103");
    // The part rows: maker, part number, the template function the row places; and whether the project held the
    // part.
    let rows = |s: &ShellModel| -> Vec<(String, String, u16, bool)> {
        let p = s.picker().expect("the picker");
        p.rows
            .iter()
            .filter_map(|r| match r {
                PickerRow::Part(i) => {
                    let part = &p.catalogue.parts[*i];
                    Some((
                        part.manufacturer.clone(),
                        part.part_number.clone(),
                        part.functions[p.part_main(*i)?].index,
                        part.library.is_none(),
                    ))
                }
                _ => None,
            })
            .collect()
    };
    done(&mut s, "tool.place_part", json!({}));
    let before = rows(&s);
    let held_before = |n: &str| before.iter().any(|r| r.1 == n && r.3);
    assert!(!held_before("EW-GEN-TRM-1"), "the library part is not held yet");
    assert!(before.iter().any(|r| r.1 == "EW-GEN-TRM-1"));
    let in_order = |v: &[(String, String, u16, bool)]| {
        let keys: Vec<(bool, &str, &str)> = v.iter().map(|r| (!r.3, r.0.as_str(), r.1.as_str())).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        keys == sorted
    };
    assert!(
        in_order(&before),
        "the project's first, then maker, then part number"
    );
    done(&mut s, "ui.dismiss", json!({}));
    // The placement embeds the part with its closure.
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-TRM-1", "strip": "-X1032", "designation": "7"}),
    );
    let here = sheet(&s, grid_point(&s, (82, 40)));
    click(&mut s, here, json!({}));
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.place_part", json!({}));
    let after = rows(&s);
    assert!(
        after.iter().any(|r| r.1 == "EW-GEN-TRM-1" && r.3),
        "the part is held after its placement"
    );
    assert!(in_order(&after), "the embedded part among the project's own");
    let rows_of = |v: &[(String, String, u16, bool)]| -> std::collections::BTreeSet<(String, String, u16)> {
        v.iter().map(|r| (r.0.clone(), r.1.clone(), r.2)).collect()
    };
    assert_eq!(rows_of(&after), rows_of(&before), "the same rows");
    s.shutdown();
}

/// The picker's filter matches a part's description and category and a symbol's name
/// and keywords besides the keys, every typed token somewhere in the row: "relay" finds EW-GEN-RLY-1 ("Relay with
/// four changeover contacts"), "relay changeover" too, "relay valve" nothing of it; "feed-through" (a keyword) finds
/// the terminal symbol.
#[test]
fn the_picker_filter_matches_descriptions_names_and_keywords_by_token() {
    use ew_viewmodel::tools::PickerRow;
    let tmp = Tmp::new("picker-words");
    let mut s = shell(&tmp, "101");
    // The parts are the device browser's (FT-1696).
    done(&mut s, "tool.place_part", json!({}));
    let rows = |s: &ShellModel| -> (Vec<String>, Vec<String>) {
        let p = s.picker().expect("the picker");
        let (mut parts, mut symbols) = (Vec::new(), Vec::new());
        for r in &p.rows {
            match r {
                PickerRow::Part(i) => parts.push(p.catalogue.parts[*i].part_number.clone()),
                PickerRow::Symbol(i) => symbols.push(p.catalogue.symbols[*i].key.clone()),
                PickerRow::Strip(_) | PickerRow::PartFn(..) => {}
            }
        }
        (parts, symbols)
    };
    for filter in ["relay", "Relay  CHANGEOVER", "changeover relay"] {
        done(&mut s, "picker.filter", json!({ "text": filter }));
        let (parts, _) = rows(&s);
        assert!(parts.iter().any(|n| n == "EW-GEN-RLY-1"), "{filter}: {parts:?}");
    }
    done(&mut s, "picker.filter", json!({"text": "relay valve"}));
    assert!(!rows(&s).0.iter().any(|n| n == "EW-GEN-RLY-1"), "{:?}", rows(&s));
    // The keys still match as before.
    done(&mut s, "picker.filter", json!({"text": "rly"}));
    assert!(rows(&s).0.iter().any(|n| n == "EW-GEN-RLY-1"));
    // A symbol's keyword ("actuator", not in the coil's name) finds it in the symbol browser.
    done(&mut s, "ui.dismiss", json!({}));
    done(&mut s, "tool.place", json!({}));
    done(&mut s, "picker.filter", json!({"text": "actuator"}));
    let (_, symbols) = rows(&s);
    assert!(symbols.iter().any(|k| k == "ew.iec.coil@qet10"), "{symbols:?}");
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
            "reason-page-type-no-symbols-title",
            "{action}"
        );
    }
    assert_eq!(
        refused(&mut s, "place.pick", json!({"symbol": "ew.iec.mcb-1p@qet10"})),
        "reason-page-type-no-symbols-title"
    );
}

/// On each page kind without drawing tools, the Wire button's disabled reason names the kind in words; a
/// generated section divider reaches the divider's text, not the generic "drawn pages" one.
#[test]
fn the_wire_buttons_disabled_reason_names_the_page_kind() {
    let tmp = Tmp::new("ft892");
    let mut s = shell(&tmp, "1");
    let wire = ew_viewmodel::actions::find("tool.wire").unwrap();
    let expected = [
        (
            "section_divider",
            "A section divider takes no symbols or wires: draw on a schematic page",
        ),
        (
            "title_page",
            "A title page takes no symbols or wires: draw on a schematic page",
        ),
        (
            "report",
            "A report page takes no symbols or wires: draw on a schematic page",
        ),
        (
            "panel_layout",
            "A panel layout page takes no symbols or wires: draw on a schematic page",
        ),
    ];
    let rows: Vec<(String, String)> = s
        .page_rows()
        .iter()
        .map(|r| (r.kind.clone(), r.folio.clone()))
        .collect();
    for (kind, text) in expected {
        let (_, folio) = rows
            .iter()
            .find(|(k, _)| k == kind)
            .unwrap_or_else(|| panic!("Golden A has a {kind} page: {rows:?}"));
        done(&mut s, "page.goto", json!({ "folio": folio }));
        let reason = (wire.enabled)(&s).expect_err(kind);
        assert_eq!(ew_viewmodel::strings::text(reason), text, "{kind} {folio}");
    }
}

/// A pending keyboard move does not survive Delete or undo: Delete cancels the move first and
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

/// A numbering scheme whose tags print the anchor cell's column (`-101F1.8`), set on a copy of Golden A, so that a
/// move of an existing device changes its tag (Golden A's own page-based tags change neither with a move on
/// the page nor with a structure box, whose location the displayed tag omits).
fn column_numbering(s: &mut ShellModel) {
    let numbering = json!({"default": "page-based", "schemes": [
        {"key": "page-based", "format": "{page}{letter}{counter}.{column}", "rules": []}]});
    done(
        s,
        "command.run",
        json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": numbering}}),
    );
}

/// The end-to-end flow of an existing device: with a numbering scheme that prints the column, the keyboard
/// move of -101F1 to the right keeps a preview that names its tag change, and the page tables give the device's
/// printed tag position (`TableRep::tag_at`), by its body.
#[test]
fn moving_an_existing_device_changes_its_tag_in_the_preview() {
    let tmp = Tmp::new("ft502");
    let mut s = shell(&tmp, "101");
    column_numbering(&mut s);
    let before = rep_with(&s, "ew.iec.mcb-1pn@qet10");
    let centre = (
        (before.body[0] + before.body[2]) / 2,
        (before.body[1] + before.body[3]) / 2,
    );
    let c = sheet(&s, centre);
    click(&mut s, c, json!({}));
    done(&mut s, "cursor.set", json!({}));
    done(&mut s, "tool.move", json!({}));
    for _ in 0..12 {
        done(&mut s, "cursor.right", json!({}));
    }
    done(&mut s, "canvas.dwell", json!({"ms": 80}));
    assert_eq!(state(&s)["canvas"]["preview"]["command"], "rep.move");
    let p = s.preview_state().expect("a kept preview");
    let changes: Vec<(String, String)> = p
        .derived
        .tags
        .iter()
        .map(|c| (c.before.clone(), c.after.clone()))
        .collect();
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert!(
        changes[0].0.starts_with("-101F1.") && changes[0].1.starts_with("-101F1."),
        "{changes:?}"
    );
    assert_ne!(changes[0].0, changes[0].1);
    assert_eq!(p.derived.tags[0].key.as_uuid(), before.device.unwrap());
    // The device prints its tag: the tables give where, by the body.
    let at = before.tag_at.expect("-101F1 prints its tag");
    let (w, h) = (before.body[2] - before.body[0], before.body[3] - before.body[1]);
    assert!(
        (before.body[0] - w..=before.body[2] + w).contains(&at.0)
            && (before.body[1] - h..=before.body[3] + h).contains(&at.1),
        "{at:?} by {:?}",
        before.body
    );
    // An interruption point prints no tag.
    assert!(
        tables(&s)
            .reps
            .iter()
            .filter(|r| r.ip.is_some())
            .all(|r| r.tag_at.is_none())
    );
    done(&mut s, "ui.dismiss", json!({}));
    s.shutdown();
}

/// A contact placed from a starter part's auxiliary template entry (EW-GEN-CON-1,
/// entry 4, letter K) before its coil gets a numbered tag like any placed device, never "-?K?": the device is placed
/// through the contact (`numbering::main_rep`'s auxiliary fallback). Both paths that arm it: `place.pick` with the part,
/// and the picker's part opened to its functions and the entry's row chosen with `picker.choose`.
#[test]
fn a_contact_placed_from_a_part_before_its_coil_gets_a_numbered_tag() {
    use ew_viewmodel::tools::PickerRow;
    let placed = |s: &ShellModel| -> String {
        let toast = state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned();
        assert!(toast.starts_with("Placed -101K"), "{toast}");
        assert!(!toast.contains('?'), "{toast}");
        toast
    };
    // `place.pick` with the part.
    let tmp = Tmp::new("part-aux-pick");
    let mut s = shell(&tmp, "101");
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-CON-1", "template": 4, "letter": "K"}),
    );
    let here = sheet(&s, free_point(&s));
    click(&mut s, here, json!({}));
    let first = placed(&s);
    done(&mut s, "page.describe", json!({"folio": "101"}));
    let text = s.take_page_text().expect("page.describe leaves its text");
    assert!(
        !text.lines().any(|l| l.starts_with("  rep ") && l.contains('?')),
        "{text}"
    );
    // A second one on the same page gets another counter.
    done(
        &mut s,
        "place.pick",
        json!({"part": "EW-GEN-CON-1", "template": 4, "letter": "K"}),
    );
    let next = sheet(&s, grid_point(&s, (66, 30)));
    click(&mut s, next, json!({}));
    assert_ne!(placed(&s), first);
    s.shutdown();

    // The picker's part row.
    let tmp = Tmp::new("part-aux-row");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.place_part", json!({}));
    done(&mut s, "picker.filter", json!({"text": "EW-GEN-CON-1"}));
    // The part's row opens to its functions.
    done(&mut s, "picker.open", json!({}));
    let row = {
        let p = s.picker().expect("the picker");
        p.rows
            .iter()
            .position(|r| match r {
                PickerRow::PartFn(i, k) => {
                    let part = &p.catalogue.parts[*i];
                    part.part_number == "EW-GEN-CON-1" && part.functions[*k].index == 4
                }
                _ => false,
            })
            .expect("the part's entry 4 row")
    };
    done(&mut s, "picker.row", json!({ "to": row }));
    done(&mut s, "picker.choose", json!({"letter": "K"}));
    assert_eq!(s.tool().name(), "place");
    let here = sheet(&s, free_point(&s));
    click(&mut s, here, json!({}));
    placed(&s);
    s.shutdown();
}

/// FT-900: after a placement the representation it made is the selection, replacing what was selected; the
/// Place tool stays armed and floats the next one.
#[test]
fn ft900_a_placement_selects_what_it_placed_and_the_tool_stays_armed() {
    let tmp = Tmp::new("ft900-place");
    let mut s = shell(&tmp, "101");
    let first = tables(&s).reps[0].clone();
    done(&mut s, "select.set", json!({"ids": [first.id.to_string()]}));
    assert_eq!(state(&s)["canvas"]["selection"], json!([first.id.to_string()]));
    done(
        &mut s,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    let before: Vec<_> = tables(&s).reps.iter().map(|r| r.id).collect();
    let pt = sheet(&s, free_point(&s));
    click(&mut s, pt, json!({}));
    let placed: Vec<String> = tables(&s)
        .reps
        .iter()
        .filter(|r| !before.contains(&r.id))
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(placed.len(), 1);
    assert_eq!(state(&s)["canvas"]["selection"], json!(placed));
    assert_eq!(s.tool().name(), "place");
    assert_eq!(s.tool().state(), "floating");
}

/// A part row carries its rating beside its part number, which tells two parts of one kind apart:
/// the starter fuse EW-GEN-FUS-1 is rated 4 A.
#[test]
fn a_part_row_carries_its_rating() {
    use ew_viewmodel::tools::PickerRow;
    let tmp = Tmp::new("picker-rating");
    let mut s = shell(&tmp, "101");
    done(&mut s, "tool.place_part", json!({}));
    done(&mut s, "picker.filter", json!({"text": "EW-GEN-FUS-1"}));
    let p = s.picker().expect("the picker");
    let ratings: Vec<(String, String)> = p
        .rows
        .iter()
        .filter_map(|r| match r {
            PickerRow::Part(i) => Some(&p.catalogue.parts[*i]),
            _ => None,
        })
        .map(|part| (part.part_number.clone(), part.rating.clone()))
        .collect();
    assert_eq!(ratings, [("EW-GEN-FUS-1".to_owned(), "4 A".to_owned())]);
}
