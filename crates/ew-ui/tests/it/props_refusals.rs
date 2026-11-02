// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cycle 46's window check W46.4 (kittest, no rendering), on a writable copy of Golden A: a Properties refusal shows
//! under its row only, never again as a banner, and the next refusal names its own point (F1); a banner the window
//! raised for a failure gives way to the next failure, a banner of the shell's own does not; a new row error scrolls
//! into view at the 1280 × 800 floor (F2); Escape in an editor holding an error cancels the edit (i3).

use crate::support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::key;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::ew_viewmodel::strings;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

/// The star row of an interruption point's sheet.
const STAR: &str = "function/kind.ip.star_source";

/// The refusal of a star on the external point "IP".
const EXTERNAL: &str =
    "interruption point IP: an external point never pairs, so it takes no star; clear the star";

/// The refusal of a star on the target point "W464".
const TARGET: &str = "interruption point W464: a star source is a source point; set the direction to source, or clear the star";

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

/// Golden A on 102 at the window's floor (1280 × 800) with Properties shown, an external interruption point and a
/// target point W464 placed (W46.4's script i3).
fn with_two_points(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(&mut h, "view.zoom_actual", json!({}));
    dispatch(&mut h, "props.show", json!({"on": true}));
    dispatch(&mut h, "tool.ip", json!({}));
    dispatch(&mut h, "ip.external", json!({"external": "JE-N1000-03:N"}));
    dispatch(&mut h, "cursor.set", json!({"x_mm": 155.924, "y_mm": 101.124}));
    dispatch(&mut h, "canvas.activate", json!({}));
    dispatch(&mut h, "ip.external", json!({"external": ""}));
    dispatch(&mut h, "ip.name", json!({"text": "W464"}));
    dispatch(&mut h, "tool.ip_direction", json!({"direction": "target"}));
    dispatch(&mut h, "cursor.set", json!({"x_mm": 60.0, "y_mm": 160.0}));
    dispatch(&mut h, "canvas.activate", json!({}));
    dispatch(&mut h, "ui.dismiss", json!({}));
    dispatch(&mut h, "tool.select", json!({}));
    settle(&mut h);
    wait_for_page(&mut h);
    h
}

/// Selects the point at (`x`, `y`) mm, opens its star row's editor and types `yes` and Enter into it.
fn star_with_keys(h: &mut Harness<'_, App>, x: f64, y: f64) {
    dispatch(h, "canvas.press", json!({"x_mm": x, "y_mm": y}));
    dispatch(h, "canvas.release", json!({"x_mm": x, "y_mm": y}));
    dispatch(h, "ui.focus", json!({"area": "properties"}));
    settle(h);
    dispatch(h, "props.edit", json!({"row": STAR}));
    settle(h);
    settle(h);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.event(egui::Event::Text("yes".to_owned()));
    h.run_ok();
    h.key_press(Key::Enter);
    settle(h);
    settle(h);
}

/// The row error of the Properties pane, whole.
fn row_error(h: &Harness<'_, App>) -> Option<(String, String)> {
    h.state()
        .shell()
        .properties()
        .error()
        .map(|(r, m)| (r.to_owned(), m.to_owned()))
}

/// W46.4 F1 (ruling 1): two successive star refusals on two points, typed into the editor, raise no banner; the pane's
/// error under the row is the second point's.
#[test]
fn two_refusals_on_two_points_show_under_the_row_and_raise_no_banner() {
    let tmp = Tmp::new();
    let mut h = with_two_points(&tmp);
    star_with_keys(&mut h, 155.924, 99.124);
    assert_eq!(row_error(&h), Some((STAR.to_owned(), EXTERNAL.to_owned())));
    assert_eq!(
        h.state().shell().banner(),
        None,
        "no banner beside the row's error"
    );
    dispatch(&mut h, "props.cancel", json!({}));
    star_with_keys(&mut h, 59.371, 161.143);
    assert_eq!(row_error(&h), Some((STAR.to_owned(), TARGET.to_owned())));
    assert_eq!(
        h.state().shell().banner(),
        None,
        "no banner beside the row's error"
    );
    h.state_mut().shell_mut().shutdown();
}

/// W46.4 F1 (ruling 2): a failure outside Properties replaces the banner the window raised for the last one; a banner
/// the shell raised itself is not replaced.
#[test]
fn the_next_failure_replaces_the_windows_banner_not_the_shells() {
    let tmp = Tmp::new();
    let mut h = with_two_points(&tmp);
    // `tab.activate` keeps the banner standing: the window's own is replaced by the next failure, a page.goto's
    // failure (which clears the banner first) shows as before.
    let bad = |name: &str| strings::format(key!("error-bad-param"), &[name]);
    h.state_mut()
        .dispatch_from_window("page.goto", &json!({"folio": "Q1"}));
    assert_eq!(
        h.state().shell().banner(),
        Some(strings::format(key!("error-no-folio"), &["Q1"]).as_str())
    );
    h.state_mut()
        .dispatch_from_window("tab.activate", &json!({"group": 9, "index": 0}));
    assert_eq!(h.state().shell().banner(), Some(bad("group").as_str()));
    h.state_mut()
        .dispatch_from_window("tab.activate", &json!({"group": 0, "index": 99}));
    assert_eq!(h.state().shell().banner(), Some(bad("index").as_str()));
    let own = strings::text(key!("banner-no-file-dialog")).to_owned();
    h.state_mut().shell_mut().show_banner(&own);
    h.state_mut()
        .dispatch_from_window("tab.activate", &json!({"group": 9, "index": 0}));
    assert_eq!(
        h.state().shell().banner(),
        Some(own.as_str()),
        "the shell's banner stands"
    );
    h.state_mut().shell_mut().shutdown();
}

/// The clipped texts the last frame painted: (text, its rectangle, its clip rectangle).
fn painted_texts(h: &Harness<'_, App>) -> Vec<(String, egui::Rect, egui::Rect)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                t.galley.rect.translate(t.pos.to_vec2()),
                clip,
            )),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, c.clip_rect, &mut out);
    }
    out
}

/// W46.4 F2 (ruling 3): at the 1280 × 800 floor the refused Star source row's error line is drawn inside the pane's
/// visible rectangle (its clip).
#[test]
fn at_the_floor_the_refused_rows_error_scrolls_into_view() {
    let tmp = Tmp::new();
    let mut h = with_two_points(&tmp);
    star_with_keys(&mut h, 59.371, 161.143);
    settle(&mut h);
    let texts = painted_texts(&h);
    // The pane's visible rectangle: the clip of its rows' labels.
    let (_, _, pane) = texts
        .iter()
        .find(|(t, _, _)| t == "Star source")
        .expect("the Star source row is painted");
    let (_, text, _) = texts
        .iter()
        .find(|(t, _, clip)| t == TARGET && clip == pane)
        .unwrap_or_else(|| panic!("the error line is painted in the pane {pane:?}"));
    assert!(
        pane.contains_rect(*text),
        "the error line {text:?} lies inside the pane's visible rectangle {pane:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// W46.4 i3 (ruling 4): Escape in an editor that holds an error closes the editor, leaves the value as it was and
/// sends no commit (no refusal).
#[test]
fn escape_in_an_editor_holding_an_error_cancels_the_edit() {
    let tmp = Tmp::new();
    let mut h = with_two_points(&tmp);
    star_with_keys(&mut h, 59.371, 161.143);
    assert_eq!(row_error(&h), Some((STAR.to_owned(), TARGET.to_owned())));
    let rev = h.state().shell().revision();
    let sent = h.state().dispatched().len();
    assert!(sent < 200, "the dispatch log keeps what Escape sends: {sent}");
    h.key_press(Key::Escape);
    settle(&mut h);
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.properties().edit().is_none(), "the editor closed");
    assert_eq!(row_error(&h), None);
    assert_eq!(shell.revision(), rev, "the value is unchanged");
    let after: Vec<&str> = h.state().dispatched()[sent..].to_vec();
    assert!(after.contains(&"props.cancel"), "{after:?}");
    assert!(
        !after.contains(&"props.commit"),
        "no commit after Escape: {after:?}"
    );
    assert!(
        !after.contains(&"ui.dismiss"),
        "Escape is the editor's: {after:?}"
    );
    h.state_mut().shell_mut().shutdown();
}
