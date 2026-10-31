// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The tool framework through the shell on temporary copies of Golden A (WP-S2.03 part 2, acceptance checks T4, T5,
//! T6, T8, T9, T10 and the model half of T11): Select with its clicks and bands, snapping on the QET grid, the
//! keyboard tool input, the link hover preview, Escape safety, the transcript and describe members and the exact
//! counts of tool events.

use crate::common;

use std::collections::BTreeSet;

use common::{Tmp, at, click, done, empty_point, entity_points, refused, state};
use ew_viewmodel::canvas::reading_key;
use ew_viewmodel::ew_api::output::ew_dl::EntityRef;
use ew_viewmodel::ew_api::output::ew_dl::ew_units::Rect;
use ew_viewmodel::shell::ShellModel;
use ew_viewmodel::tools::{SnapKind, measure_label};
use serde_json::{Value, json};

/// The selection's UUIDs.
fn selected(shell: &ShellModel) -> Vec<EntityRef> {
    shell.selection().items.clone()
}

/// The entities of the focused page in paint order of their first rectangle, with all their rectangles.
fn entities(shell: &ShellModel) -> Vec<(EntityRef, Vec<Rect>)> {
    let mut out: Vec<(EntityRef, Vec<Rect>)> = Vec::new();
    for (e, r) in &shell.canvas().unwrap().page().entities {
        match out.iter_mut().find(|(x, _)| x == e) {
            Some((_, rs)) => rs.push(*r),
            None => out.push((*e, vec![*r])),
        }
    }
    out
}

/// The band oracle (spec §7.6): left to right every rectangle inside, right to left any rectangle touching.
fn band_oracle(shell: &ShellModel, a: (i64, i64), b: (i64, i64)) -> Vec<EntityRef> {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let inside = |r: &Rect| r.min().x.0 >= x0 && r.max().x.0 <= x1 && r.min().y.0 >= y0 && r.max().y.0 <= y1;
    let touch = |r: &Rect| r.min().x.0 <= x1 && r.max().x.0 >= x0 && r.min().y.0 <= y1 && r.max().y.0 >= y0;
    // In reading order of the centre of each entity's rectangles together (y, then x), ties by ID (FT-439).
    let mut picked: Vec<((i64, i64), EntityRef)> = entities(shell)
        .into_iter()
        .filter(|(_, rs)| {
            if b.0 >= a.0 {
                rs.iter().all(inside)
            } else {
                rs.iter().any(touch)
            }
        })
        .map(|(e, rs)| {
            let x0 = rs.iter().map(|r| r.min().x.0).min().unwrap();
            let x1 = rs.iter().map(|r| r.max().x.0).max().unwrap();
            let y0 = rs.iter().map(|r| r.min().y.0).min().unwrap();
            let y1 = rs.iter().map(|r| r.max().y.0).max().unwrap();
            ((y0 + (y1 - y0) / 2, x0 + (x1 - x0) / 2), e)
        })
        .collect();
    picked.sort();
    picked.into_iter().map(|(_, e)| e).collect()
}

#[test]
fn t4_select_clicks_shift_and_ctrl() {
    let tmp = Tmp::new("tools-t4");
    let mut shell = tmp.shell("103");
    assert_eq!(shell.tool().name(), "select");
    let points = entity_points(shell.canvas().unwrap());
    assert!(points.len() >= 3, "{}", points.len());
    let (a, pa) = points[0];
    let (b, pb) = points[1];
    click(&mut shell, pa, json!({}));
    assert_eq!(selected(&shell), vec![a]);
    assert_eq!(shell.selection().primary, Some(a));
    let generation = shell.selection().generation;
    click(&mut shell, pb, json!({"shift": true}));
    assert_eq!(selected(&shell), vec![a, b], "Shift adds");
    click(&mut shell, pa, json!({"ctrl": true}));
    assert_eq!(selected(&shell), vec![b], "Ctrl toggles");
    click(&mut shell, pa, json!({"ctrl": true}));
    assert_eq!(selected(&shell), vec![b, a]);
    assert_eq!(shell.selection().generation, generation + 3);
    // A click replaces; a click on nothing clears (Shift keeps).
    click(&mut shell, pb, json!({}));
    assert_eq!(selected(&shell), vec![b]);
    let empty = empty_point(shell.canvas().unwrap());
    click(&mut shell, empty, json!({"shift": true}));
    assert_eq!(selected(&shell), vec![b]);
    // The selected outline is drawn around each selected entity's rectangles on the page.
    let rects: Vec<Rect> = shell
        .canvas()
        .unwrap()
        .page()
        .entities
        .iter()
        .filter(|(e, _)| *e == b)
        .map(|(_, r)| *r)
        .collect();
    assert_eq!(shell.selected_rects(0), rects);
    // The live region announces the change.
    assert_eq!(shell.tool_announcement(), Some("1 selected"));
    click(&mut shell, empty, json!({}));
    assert!(shell.selection().is_empty());
    assert!(shell.selected_rects(0).is_empty());
    shell.shutdown();
}

#[test]
fn t4_bands_left_to_right_enclose_and_right_to_left_touch() {
    let tmp = Tmp::new("tools-t4b");
    let mut shell = tmp.shell("103");
    let empty = empty_point(shell.canvas().unwrap());
    let (w, h) = shell.canvas().unwrap().sheet();
    let far = (w * 3 / 5, h * 3 / 5);
    // Left to right, from an empty point: the band is solid while it encloses.
    done(&mut shell, "canvas.press", at(empty, json!({})));
    assert_eq!(shell.tool().state(), "rubber-band");
    done(&mut shell, "canvas.pointer", at(far, json!({})));
    assert_eq!(state(&shell)["canvas"]["overlay"], 1);
    done(&mut shell, "canvas.release", at(far, json!({})));
    let enclosed = band_oracle(&shell, empty, far);
    assert!(!enclosed.is_empty(), "the band encloses something");
    assert_eq!(selected(&shell), enclosed);
    assert_eq!(shell.tool().state(), "idle");
    assert!(shell.tool_overlay().is_empty(), "a selection clears the overlay");
    // Right to left from the same corners: everything touched, a superset.
    let right = (far.0, empty.1);
    let left = (empty.0, far.1);
    let touched_oracle = band_oracle(&shell, right, left);
    done(&mut shell, "canvas.press", at(right, json!({})));
    let started_on_entity = shell.tool().state() == "pressed";
    done(&mut shell, "canvas.pointer", at(left, json!({})));
    assert_eq!(shell.tool().state(), "rubber-band");
    done(&mut shell, "canvas.release", at(left, json!({})));
    if !started_on_entity {
        assert_eq!(selected(&shell), touched_oracle);
    }
    let touched: BTreeSet<EntityRef> = touched_oracle.iter().copied().collect();
    assert!(enclosed.iter().all(|e| touched.contains(e)));
    assert!(touched.len() > enclosed.len());
    // Shift adds a band to the selection.
    click(&mut shell, empty, json!({}));
    let points = entity_points(shell.canvas().unwrap());
    click(&mut shell, points[0].1, json!({}));
    done(&mut shell, "canvas.press", at(empty, json!({"shift": true})));
    done(&mut shell, "canvas.pointer", at(far, json!({})));
    done(&mut shell, "canvas.release", at(far, json!({"shift": true})));
    let mut expected = vec![points[0].0];
    expected.extend(enclosed.iter().filter(|e| **e != points[0].0));
    assert_eq!(selected(&shell), expected);
    shell.shutdown();
}

/// Window check, item 5: the rubber band is gone when its drag ends and when the page changes under it, not only on
/// the next pointer move.
#[test]
fn a_band_leaves_no_overlay_on_release_or_page_change() {
    let tmp = Tmp::new("tools-band-clear");
    let mut shell = tmp.shell("103");
    let empty = empty_point(shell.canvas().unwrap());
    let (w, h) = shell.canvas().unwrap().sheet();
    let far = (w * 3 / 5, h * 3 / 5);
    done(&mut shell, "canvas.press", at(empty, json!({})));
    done(&mut shell, "canvas.pointer", at(far, json!({})));
    assert_eq!(shell.tool().state(), "rubber-band");
    assert!(!shell.tool_overlay().is_empty(), "the band is drawn");
    done(&mut shell, "canvas.release", at(far, json!({})));
    assert_eq!(shell.tool().state(), "idle");
    assert!(shell.tool_overlay().is_empty(), "no band after the release");
    // A band still open when the page changes is dropped with the page.
    done(&mut shell, "canvas.press", at(empty, json!({})));
    done(&mut shell, "canvas.pointer", at(far, json!({})));
    assert_eq!(shell.tool().state(), "rubber-band");
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    assert_eq!(shell.tool().state(), "idle");
    assert!(shell.tool_overlay().is_empty(), "no band after the page change");
    shell.shutdown();
}

#[test]
fn t4_a_click_on_a_link_follows_it_and_the_selection_survives_tabs() {
    let tmp = Tmp::new("tools-t4c");
    let mut shell = tmp.shell("101");
    let points = entity_points(shell.canvas().unwrap());
    click(&mut shell, points[0].1, json!({}));
    let sel = selected(&shell);
    let c = shell.canvas().unwrap();
    let link = c
        .page()
        .links
        .iter()
        .find(|l| {
            l.target
                .as_ref()
                .is_some_and(|t| t.folio != "101" && t.at_nm.is_some())
        })
        .unwrap()
        .clone();
    let p = ew_viewmodel::canvas::rect_centre(link.rect);
    click(&mut shell, p, json!({}));
    let target = link.target.unwrap();
    assert_eq!(shell.current_folio(), Some(target.folio.as_str()));
    assert_eq!(
        shell.canvas().unwrap().cursor(),
        target.at_nm,
        "the cursor at the target"
    );
    assert_eq!(selected(&shell), sel, "a follow keeps the selection");
    // Tab and page changes keep it too.
    done(&mut shell, "tab.new", json!({}));
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    done(&mut shell, "tab.next", json!({}));
    done(&mut shell, "view.side_by_side", json!({}));
    assert_eq!(selected(&shell), sel);
    // A press on a link that moves away is dropped: no follow, nothing selected.
    done(&mut shell, "view.side_by_side", json!({}));
    done(&mut shell, "page.goto", json!({"folio": "101"}));
    let folio = shell.current_folio().unwrap().to_owned();
    done(&mut shell, "canvas.press", at(p, json!({})));
    done(
        &mut shell,
        "canvas.pointer",
        at((p.0 + 20_000_000, p.1), json!({})),
    );
    done(
        &mut shell,
        "canvas.release",
        at((p.0 + 20_000_000, p.1), json!({})),
    );
    assert_eq!(shell.current_folio(), Some(folio.as_str()));
    assert_eq!(selected(&shell), sel);
    shell.shutdown();
}

#[test]
fn t5_snapping_on_the_qet_grid_of_folio_102() {
    let tmp = Tmp::new("tools-t5");
    let shell = tmp.shell("102");
    let c = shell.canvas().unwrap();
    let page = c.page();
    let f1 = page
        .cps
        .iter()
        .find(|cp| cp.label == "-102F1:2")
        .expect("-102F1:2 on 102");
    let s = &page.snapper;
    assert_eq!(s.grid(), (7_937_500, 3));
    let ppn = c.points_per_nm();
    let reach = (8.0 / ppn) as i64;
    // A CP within 8 points wins, beyond it the grid; Alt suppresses the CP.
    let near = s.snap((f1.at_nm.0 + reach / 2, f1.at_nm.1), ppn, false);
    assert_eq!(near.kind, SnapKind::Cp);
    assert_eq!(near.label.as_deref(), Some("-102F1:2"));
    assert_eq!(near.at_nm, f1.at_nm);
    let alt = s.snap((f1.at_nm.0 + reach / 2, f1.at_nm.1), ppn, true);
    assert_eq!(alt.kind, SnapKind::Grid);
    let beyond = s.snap(
        (f1.at_nm.0 + reach + reach / 2, f1.at_nm.1 + reach + reach / 2),
        ppn,
        false,
    );
    assert!(
        beyond.kind == SnapKind::Grid || beyond.label.as_deref() != Some("-102F1:2"),
        "beyond 8 points -102F1:2 does not attract"
    );
    // Grid snaps are exact grid indices: the page point within 1 nm of k × g, and the snapped sheet point a point of
    // the grid overlay (the grid cursor's).
    let map = c.grid_map();
    for i in 0..60_i64 {
        let p = (31_000_000 + i * 4_123_457, 27_000_000 + i * 2_345_679);
        let g = s.snap(p, ppn, true);
        for (k, page_nm) in [(g.grid_index.0, g.page_nm.0), (g.grid_index.1, g.page_nm.1)] {
            assert!(
                (i128::from(page_nm) * 3 - i128::from(k) * 7_937_500).abs() <= 3,
                "{k} {page_nm}"
            );
        }
        assert_eq!(
            g.at_nm,
            (map.point(g.grid_index.0, 0), map.point(g.grid_index.1, 1))
        );
    }
    // The CP's own grid index is its page point's.
    assert_eq!(Some(near.grid_index), f1.grid_index);
}

#[test]
fn t6_connection_points_by_keyboard_in_reading_order() {
    let tmp = Tmp::new("tools-t6");
    let mut shell = tmp.shell("102");
    let cps = shell.canvas().unwrap().page().cps.clone();
    let n = cps.len();
    assert!(n >= 10);
    // `]` from no cursor: the first CP after the view centre in reading order; then one by one, wrapping.
    let page = std::sync::Arc::clone(shell.canvas().unwrap().page());
    let centre = shell.canvas().unwrap().centre();
    let here = reading_key(page.cells.as_ref(), centre);
    let first = (0..n)
        .find(|&j| reading_key(page.cells.as_ref(), cps[j].at_nm) > here)
        .unwrap_or(0);
    let at_cursor = |shell: &ShellModel| shell.canvas().unwrap().cursor();
    done(&mut shell, "cp.next", json!({}));
    assert_eq!(at_cursor(&shell), Some(cps[first].at_nm));
    for k in 1..=n {
        done(&mut shell, "cp.next", json!({}));
        assert_eq!(at_cursor(&shell), Some(cps[(first + k) % n].at_nm), "step {k}");
    }
    // `[` walks back.
    done(&mut shell, "cp.previous", json!({}));
    assert_eq!(at_cursor(&shell), Some(cps[(first + n - 1) % n].at_nm));
    done(&mut shell, "cp.next", json!({}));
    assert_eq!(at_cursor(&shell), Some(cps[first].at_nm));
    // N: only CPs that no connection ends at, wrapping.
    let free: Vec<usize> = (0..n).filter(|&j| !cps[j].connected).collect();
    assert!(!free.is_empty());
    let mut seen = Vec::new();
    for _ in 0..=free.len() {
        done(&mut shell, "cp.next_unconnected", json!({}));
        let c = at_cursor(&shell).unwrap();
        let j = cps.iter().position(|cp| cp.at_nm == c).unwrap();
        assert!(!cps[j].connected);
        seen.push(j);
    }
    assert_eq!(seen[0], seen[free.len()], "it wraps");
    // The describe text names the CP at the cursor.
    let label = &cps[seen[0]].label;
    assert!(
        shell
            .describe()
            .lines()
            .any(|l| l.starts_with("canvas cursor ") && l.ends_with(&format!(" cp \"{label}\""))),
        "{}",
        shell.describe()
    );
    shell.shutdown();
}

#[test]
fn t6_enter_selects_and_measure_from_the_keyboard_alone() {
    let tmp = Tmp::new("tools-t6b");
    let mut shell = tmp.shell("102");
    // Enter without a cursor is refused; on an entity it selects it.
    assert_eq!(
        refused(&mut shell, "canvas.activate", json!({})),
        "reason-no-cursor"
    );
    // (The cursor snaps to the grid: an entity whose nearest grid point is on it and on no link.)
    let mut under = None;
    for (_, p) in entity_points(shell.canvas().unwrap()) {
        done(&mut shell, "cursor.set", at(p, json!({})));
        let c = shell.canvas().unwrap();
        // The hover's entity, where the Select tool's pick (WP-S2.04 §6.6 with FT-500) agrees with it.
        let page = c.page();
        let slop = ew_viewmodel::canvas::HOVER_SLOP_PT / c.points_per_nm();
        if c.cursor_link().is_none()
            && let Some((e, _)) = c.cursor_entity()
            && let Some(p) = c.cursor()
            && ew_viewmodel::tools::pick_stack(page, &page.snapper, p, slop).first() == Some(&e)
        {
            under = Some(e);
            break;
        }
    }
    let under = under.expect("an entity at a grid point");
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(shell.current_folio(), Some("102"));
    assert_eq!(selected(&shell), vec![under]);
    // Measure between two CPs: the exact distance.
    done(&mut shell, "tool.measure", json!({}));
    assert_eq!(shell.tool().state(), "armed");
    done(&mut shell, "cp.next", json!({}));
    let a = shell.canvas().unwrap().cursor().unwrap();
    assert_eq!(
        state(&shell)["canvas"]["snap"]["kind"],
        "cp",
        "the snap indicator on the CP"
    );
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(shell.tool().state(), "measuring");
    done(&mut shell, "cp.next", json!({}));
    done(&mut shell, "cp.next", json!({}));
    let b = shell.canvas().unwrap().cursor().unwrap();
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(shell.tool().state(), "armed", "B fixed");
    let labels: Vec<String> = shell.tool_overlay().labels().map(str::to_owned).collect();
    assert_eq!(labels, vec![measure_label(a, b)]);
    let (dx, dy) = (i128::from(b.0 - a.0), i128::from(b.1 - a.1));
    let exact = ((dx * dx + dy * dy) as f64).sqrt();
    let label = &labels[0];
    let d: f64 = label
        .rsplit(", ")
        .next()
        .unwrap()
        .trim_end_matches(" mm")
        .parse()
        .unwrap();
    assert!((d * 1e6 - exact).abs() <= 500.0 + 1e-6, "{label} vs {exact}");
    assert!(
        shell
            .describe()
            .contains(&format!("canvas overlay 4 label \"{label}\"")),
        "{}",
        shell.describe()
    );
    // The overlay stays until the next move; Backspace and Escape step back.
    done(&mut shell, "canvas.activate", json!({}));
    assert_eq!(shell.tool().state(), "measuring");
    done(&mut shell, "tool.back", json!({}));
    assert_eq!(shell.tool().state(), "armed");
    assert!(shell.tool_overlay().is_empty());
    // Escape from armed returns to Select.
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().name(), "select");
    shell.shutdown();
}

#[test]
fn t8_the_link_hover_preview() {
    let tmp = Tmp::new("tools-t8");
    let mut shell = tmp.shell("101");
    let c = shell.canvas().unwrap();
    let link = c
        .page()
        .links
        .iter()
        .find(|l| {
            l.target
                .as_ref()
                .is_some_and(|t| t.folio != "101" && t.at_nm.is_some())
        })
        .unwrap()
        .clone();
    let target = link.target.clone().unwrap();
    assert!(!shell.cached_pages().contains(&target.folio), "not cached yet");
    let p = ew_viewmodel::canvas::rect_centre(link.rect);
    done(&mut shell, "canvas.pointer", at(p, json!({})));
    done(&mut shell, "canvas.dwell", json!({"ms": 80}));
    done(&mut shell, "canvas.dwell", json!({"ms": 299}));
    assert!(shell.link_preview().is_none(), "299 ms is not enough");
    let calls = shell.session_calls();
    done(&mut shell, "canvas.dwell", json!({"ms": 300}));
    let lp = shell.link_preview().expect("a preview");
    assert_eq!(lp.target, target);
    assert_eq!(lp.canvas.page().folio, target.folio);
    assert_eq!(lp.canvas.pane(), (320.0, 200.0));
    assert_eq!(lp.canvas.centre(), target.at_nm.unwrap(), "centred on the target");
    assert!(lp.canvas.zoom() >= 2.0 * lp.canvas.fit_zoom() - 1e-12);
    assert_eq!(shell.session_calls().page_content, calls.page_content + 1);
    assert_eq!(state(&shell)["canvas"]["link_preview"], json!(target.folio));
    assert!(
        shell
            .describe()
            .contains(&format!("canvas link preview \"{}\"", target.folio))
    );
    // A pointer move clears it; a second preview of the same page makes no Session call.
    done(&mut shell, "canvas.pointer", at((p.0 + 1, p.1), json!({})));
    assert!(shell.link_preview().is_none());
    assert_eq!(state(&shell)["canvas"]["link_preview"], Value::Null);
    let calls = shell.session_calls();
    done(&mut shell, "canvas.dwell", json!({"ms": 300}));
    assert!(shell.link_preview().is_some());
    assert_eq!(
        shell.session_calls(),
        calls,
        "the target page comes from the cache"
    );
    // Escape closes it as the topmost popup; a cursor move or a view action clears it too.
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.link_preview().is_none());
    done(&mut shell, "canvas.dwell", json!({"ms": 300}));
    done(&mut shell, "cursor.right", json!({}));
    assert!(shell.link_preview().is_none());
    done(&mut shell, "canvas.pointer", at(p, json!({})));
    done(&mut shell, "canvas.dwell", json!({"ms": 300}));
    done(&mut shell, "view.zoom_in", json!({}));
    assert!(shell.link_preview().is_none());
    // Measure does not show the hover: no preview.
    done(&mut shell, "tool.measure", json!({}));
    done(&mut shell, "canvas.pointer", at(p, json!({})));
    done(&mut shell, "canvas.dwell", json!({"ms": 300}));
    assert!(shell.link_preview().is_none());
    assert_eq!(
        refused(&mut shell, "canvas.preview_snapshot", json!({"path": "x.png"})),
        "reason-no-preview"
    );
    shell.shutdown();
}

#[test]
fn t9_escape_cancels_every_shipped_tool_in_every_state() {
    let tmp = Tmp::new("tools-t9");
    let mut shell = tmp.shell("103");
    let rev = shell.revision();
    let points = entity_points(shell.canvas().unwrap());
    let empty = empty_point(shell.canvas().unwrap());
    let link = ew_viewmodel::canvas::rect_centre(shell.canvas().unwrap().page().links[0].rect);
    // Select: pressed on an entity, rubber-band from empty canvas, a press on a link.
    for (press, state_word) in [(points[0].1, "pressed"), (empty, "rubber-band"), (link, "idle")] {
        done(&mut shell, "canvas.press", at(press, json!({})));
        assert_eq!(shell.tool().state(), state_word);
        done(&mut shell, "ui.dismiss", json!({}));
        assert_eq!(shell.tool().state(), "idle");
        assert!(shell.tool_overlay().is_empty());
        done(&mut shell, "canvas.release", at(press, json!({})));
        assert!(
            shell.selection().is_empty(),
            "the cancelled gesture selects nothing"
        );
        assert_eq!(shell.current_folio(), Some("103"), "nor follows");
    }
    // Measure: armed, measuring.
    done(&mut shell, "tool.measure", json!({}));
    done(&mut shell, "canvas.press", at(empty, json!({})));
    assert_eq!(shell.tool().state(), "measuring");
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().state(), "armed");
    assert!(shell.tool_overlay().is_empty());
    done(&mut shell, "ui.dismiss", json!({}));
    assert_eq!(shell.tool().name(), "select");
    // Select with a selection: Escape clears it.
    click(&mut shell, points[0].1, json!({}));
    assert!(!shell.selection().is_empty());
    done(&mut shell, "ui.dismiss", json!({}));
    assert!(shell.selection().is_empty());
    assert_eq!(shell.revision(), rev, "the revision never changed");
    assert!(!shell.is_dirty());
    shell.shutdown();
}

#[test]
fn t10_the_transcript_and_describe_members_of_the_tools() {
    let tmp = Tmp::new("tools-t10");
    let mut shell = tmp.shell("102");
    let s = state(&shell);
    let raw = ew_viewmodel::script::state_json(&shell);
    let canvas = &raw[raw.find("\"canvas\"").unwrap()..raw.find("\"tabs\"").unwrap()];
    let order = [
        "\"link\"",
        "\"tool\"",
        "\"snap\"",
        "\"selection\"",
        "\"preview\"",
        "\"overlay\"",
        "\"link_preview\"",
    ];
    let positions: Vec<usize> = order.iter().map(|k| canvas.find(k).unwrap()).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{canvas}");
    assert_eq!(s["canvas"]["tool"], json!({"name": "select", "state": "idle"}));
    assert_eq!(s["canvas"]["snap"], Value::Null);
    assert_eq!(s["canvas"]["selection"], json!([]));
    assert_eq!(s["canvas"]["preview"], Value::Null);
    assert_eq!(s["canvas"]["overlay"], 0);
    assert_eq!(s["canvas"]["link_preview"], Value::Null);
    // The describe lines follow the canvas lines.
    let text = shell.describe();
    let lines: Vec<&str> = text.lines().collect();
    let r = lines
        .iter()
        .position(|l| l.starts_with("canvas renderer"))
        .unwrap();
    assert_eq!(
        &lines[r + 1..r + 6],
        &[
            "canvas tool \"Select\" state idle hint \"Click to select; drag for a rubber band\"",
            "canvas snap none",
            "canvas selection none",
            "canvas preview none",
            "canvas overlay 0",
        ]
    );
    // Measure armed on -102F1:2: the snap line with the label.
    done(&mut shell, "tool.measure", json!({}));
    let f1 = shell
        .canvas()
        .unwrap()
        .page()
        .cps
        .iter()
        .find(|c| c.label == "-102F1:2")
        .unwrap()
        .at_nm;
    done(
        &mut shell,
        "canvas.pointer",
        at((f1.0 + 100_000, f1.1), json!({})),
    );
    let s = state(&shell);
    assert_eq!(
        s["canvas"]["snap"],
        json!({"kind": "cp", "at_nm": [f1.0, f1.1], "label": "-102F1:2"})
    );
    assert_eq!(s["canvas"]["tool"], json!({"name": "measure", "state": "armed"}));
    let text = shell.describe();
    assert!(
        text.contains(&format!(
            "canvas snap cp \"-102F1:2\" at {} {} mm",
            ew_viewmodel::canvas::mm3(f1.0),
            ew_viewmodel::canvas::mm3(f1.1)
        )),
        "{text}"
    );
    assert!(text.contains("canvas tool \"Measure\" state armed hint \"Click or Enter at the first point\""));
    // With Alt the grid.
    done(
        &mut shell,
        "canvas.pointer",
        at((f1.0 + 100_000, f1.1), json!({"alt": true})),
    );
    assert_eq!(state(&shell)["canvas"]["snap"]["kind"], "grid");
    assert!(shell.describe().contains("canvas snap grid at "));
    // The status hint and the announcement follow the tool.
    assert_eq!(shell.status_fields(false)[0], "Click or Enter at the first point");
    done(&mut shell, "tool.select", json!({}));
    assert_eq!(
        shell.tool_announcement(),
        Some("Select: Click to select; drag for a rubber band")
    );
    let node = ew_viewmodel::status::canvas_node(&shell, 0).unwrap();
    assert_eq!(node.live, "Select: Click to select; drag for a rubber band");
    done(&mut shell, "cursor.right", json!({}));
    let node = ew_viewmodel::status::canvas_node(&shell, 0).unwrap();
    assert!(node.live.contains(" x "), "the cursor again: {}", node.live);
    // The Draw menu checks the active tool (WP-S2.17 part 2: Tools dissolved into Draw).
    let menus = ew_viewmodel::actions::menu_model(&shell);
    let draw = menus
        .iter()
        .find(|m| m.menu == ew_viewmodel::actions::Menu::Draw)
        .unwrap();
    let checked: Vec<&str> = draw
        .items()
        .into_iter()
        .filter(|i| i.checked && i.id.starts_with("tool."))
        .map(|i| i.id)
        .collect();
    assert_eq!(checked, vec!["tool.select"]);
    shell.shutdown();
}

#[test]
fn t11_tool_events_on_a_cached_page_make_no_session_call() {
    let tmp = Tmp::new("tools-t11");
    let mut shell = tmp.shell("103");
    // The property grid fetches a sheet per selection change (WP-S2.05 §9 item 4, its own exact counts in
    // `tests/properties.rs`): closed here, so that the tool events alone are counted.
    done(&mut shell, "props.show", json!({"on": false}));
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "tab.new", json!({}));
    done(&mut shell, "page.goto", json!({"folio": "103"}));
    let calls = shell.session_calls();
    assert!(
        calls.page_content > 0 && calls.page_cps > 0 && calls.page_cells > 0,
        "{calls:?}"
    );
    let memo = shell.memo_totals().unwrap();
    let points = entity_points(shell.canvas().unwrap());
    let empty = empty_point(shell.canvas().unwrap());
    let mut ran = 0;
    // Checklist 9: every kind of step must also succeed, so the equalities below cannot pass vacuously.
    let rev = shell.revision();
    let generation = shell.selection().generation;
    let mut ok = std::collections::BTreeMap::<&str, u32>::new();
    let mut measuring = false;
    for i in 0..120usize {
        let p = points[i % points.len()].1;
        let steps: Vec<(&str, Value)> = match i % 12 {
            0 => vec![
                ("canvas.press", at(p, json!({}))),
                ("canvas.release", at(p, json!({}))),
            ],
            1 => vec![("canvas.pointer", at((p.0 + 7_000, p.1), json!({})))],
            2 => vec![
                ("canvas.press", at(empty, json!({}))),
                ("canvas.pointer", at(p, json!({}))),
                ("canvas.release", at(p, json!({"shift": true}))),
            ],
            3 => vec![("cp.next", json!({})), ("canvas.activate", json!({}))],
            4 => vec![
                ("tool.measure", json!({})),
                ("cp.next", json!({})),
                ("canvas.activate", json!({})),
            ],
            5 => vec![
                ("canvas.pointer", at(p, json!({}))),
                ("canvas.press", at(p, json!({}))),
            ],
            6 => vec![("canvas.dwell", json!({"ms": 80})), ("tool.back", json!({}))],
            7 => vec![("ui.dismiss", json!({})), ("ui.dismiss", json!({}))],
            8 => vec![("tab.next", json!({})), ("cp.next_unconnected", json!({}))],
            9 => vec![("cp.previous", json!({})), ("canvas.dwell", json!({"ms": 300}))],
            10 => vec![("tool.select", json!({})), ("cursor.down", json!({}))],
            _ => vec![("canvas.pointer", json!({}))],
        };
        for (action, params) in steps {
            let out = shell.dispatch(action, &params);
            assert!(
                matches!(
                    out,
                    ew_viewmodel::shell::Outcome::Done | ew_viewmodel::shell::Outcome::Refused { .. }
                ),
                "{action}: {out:?}"
            );
            if out == ew_viewmodel::shell::Outcome::Done {
                *ok.entry(action).or_default() += 1;
            }
            measuring |= shell.tool().state() == "measuring";
            ran += 1;
        }
    }
    assert!(ran > 200);
    for action in [
        "canvas.press",
        "canvas.release",
        "canvas.pointer",
        "cp.next",
        "cp.previous",
        "cp.next_unconnected",
        "canvas.activate",
        "tab.next",
        "tool.measure",
        "tool.select",
        "canvas.dwell",
        "ui.dismiss",
    ] {
        assert!(
            ok.get(action).copied().unwrap_or(0) >= 1,
            "{action} succeeded: {ok:?}"
        );
    }
    assert!(shell.selection().generation > generation, "the selection changed");
    assert!(measuring, "Measure reached measuring");
    assert_eq!(shell.revision(), rev, "the revision never changed");
    let after = shell.session_calls();
    assert_eq!(after.page_content, calls.page_content, "{after:?}");
    assert_eq!(after.page_cells, calls.page_cells);
    assert_eq!(after.page_cps, calls.page_cps);
    assert_eq!(after.link_targets, calls.link_targets);
    assert_eq!(after.preview_with, 0, "no shipped tool previews");
    assert_eq!(shell.memo_totals().unwrap().1, memo.1, "no memo re-execution");
    shell.shutdown();
}

/// FT-715: the canvas context menu over an object lists the command bar's selection commands (the Arrange group, a
/// split button followed by its other variant) and Delete, in the bar's order and the registry's words; over nothing
/// it offers Paste only when Paste is enabled. On a read-only project every entry is disabled and says why.
#[test]
fn the_canvas_context_menu_offers_the_bar_selection_commands_and_delete() {
    use ew_viewmodel::command_bar::canvas_menu;
    use ew_viewmodel::shell::Mode;
    let tmp = Tmp::new("ctx-menu");
    let mut shell = tmp.shell_in(Mode::Headless, true, "103");
    // The window selects the object under the pointer before it opens the menu (M39: Delete needs a selection).
    done(&mut shell, "page.goto", json!({"folio": "102"}));
    done(&mut shell, "select.set", json!({"refs": ["tag:-102F1"]}));
    let labels = |s: &ShellModel, over: bool| -> Vec<(String, bool)> {
        canvas_menu(s, over)
            .into_iter()
            .map(|i| (i.label, i.enabled))
            .collect()
    };
    let words: Vec<String> = labels(&shell, true).into_iter().map(|(l, _)| l).collect();
    assert_eq!(
        words,
        [
            "Rotate clockwise",
            "Mirror left to right",
            "Mirror top to bottom",
            "Renumber devices…",
            "Delete"
        ]
    );
    assert!(labels(&shell, true).iter().all(|(_, enabled)| *enabled));
    // Nothing under the pointer: Paste, as it is enabled on a writable page.
    let paste: Vec<String> = labels(&shell, false).into_iter().map(|(l, _)| l).collect();
    assert_eq!(paste, ["Paste"]);
    shell.shutdown();
    let tmp = Tmp::new("ctx-menu-ro");
    let mut shell = tmp.shell_in(Mode::Headless, false, "103");
    for item in canvas_menu(&shell, true) {
        assert!(!item.enabled, "{}", item.label);
        assert!(
            item.reason.as_deref().is_some_and(|r| !r.is_empty()),
            "{}",
            item.label
        );
    }
    // Paste is disabled there, so nothing is offered over nothing: no menu opens.
    assert!(canvas_menu(&shell, false).is_empty());
    shell.shutdown();
}
