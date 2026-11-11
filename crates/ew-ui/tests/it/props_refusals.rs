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
use serde_json::json;
use support::{Scripted, Tmp, dispatch, harness, settle, wait_for_page};

/// The star row of an interruption point's sheet.
const STAR: &str = "function/kind.ip.star_source";

/// The refusal of a star on the external point "IP", as the pane shows it: a capital first.
const EXTERNAL: &str =
    "Interruption point IP: an external point never pairs, so it takes no star; clear the star";

/// The refusal of a star on the target point "W464", as the pane shows it.
const TARGET: &str = "Interruption point W464: a star source is a source point; set the direction to source, or clear the star";

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
    dispatch(&mut h, "cursor.set", json!({"x_mm": 185.208, "y_mm": 101.124}));
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
    star_with_keys(&mut h, 185.208, 99.124);
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

/// A toast whose line is cut (a point with a long name refuses a second point on its
/// spot) shows its whole text under the pointer in doc 16a §6.18's tooltip, `color.surface.inverse`.
#[test]
fn a_toast_with_a_cut_line_shows_its_whole_text_in_the_inverse_tooltip() {
    use egui_kittest::kittest::{NodeT as _, Queryable as _};
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(&mut h, "tool.ip", json!({}));
    let name = format!("W464{}", "0".repeat(120));
    dispatch(&mut h, "ip.name", json!({"text": name}));
    dispatch(&mut h, "cursor.set", json!({"x_mm": 60.0, "y_mm": 160.0}));
    dispatch(&mut h, "canvas.activate", json!({}));
    dispatch(&mut h, "ip.name", json!({"text": "W465"}));
    assert_eq!(
        h.state_mut().shell_mut().dispatch("canvas.activate", &json!({})),
        Outcome::Refused {
            reason_key: key!("reason-ip-not-placed")
        }
    );
    settle(&mut h);
    let whole = format!(
        "Not placed: the interruption point {name} is already here; place it on a pin or a free grid point"
    );
    let toast = h
        .get_all_by_role(egui::Role::Group)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Notification"))
        .expect("the toast's node")
        .rect();
    let inverse = ew_ui::theme::current(&h.ctx).surface_inverse.color32();
    let mut shown = false;
    h.hover_at(toast.center());
    // Two seconds on the frame clock under a still pointer (a tooltip waits half a second), as the sibling FT-1144 test.
    let start = h.ctx.input(|i| i.time);
    for i in 0..40 {
        h.input_mut().time = Some(start + f64::from(i) * 0.05);
        h.step();
        // The tooltip's label outside the toast (whose title carries the whole text as its live value), on the
        // inverse surface.
        let tip = h
            .get_all_by_role(egui::Role::Label)
            .filter(|n| n.accesskit_node().value().as_deref() == Some(whole.as_str()))
            .map(|n| n.rect())
            .find(|r| !toast.contains_rect(*r));
        // Its first frame is a sizing pass, which paints nothing.
        fn on(shape: &egui::Shape, fill: egui::Color32, tip: egui::Rect) -> bool {
            match shape {
                egui::Shape::Vec(v) => v.iter().any(|s| on(s, fill, tip)),
                egui::Shape::Rect(r) => r.fill == fill && r.rect.contains_rect(tip),
                _ => false,
            }
        }
        if tip.is_some_and(|tip| h.output().shapes.iter().any(|c| on(&c.shape, inverse, tip))) {
            shown = true;
            break;
        }
    }
    assert!(shown, "the whole text in the inverse tooltip");
    h.state_mut().shell_mut().shutdown();
}

/// A toast's line stands under its title with a capital, as the window draws it: "Not placed" over
/// "The interruption point W464 is already here; ...". The toast's own text (its tooltip, the log) keeps the
/// command's lowercase.
#[test]
fn a_toasts_line_is_drawn_with_a_capital() {
    let tmp = Tmp::new();
    let mut h = with_two_points(&tmp);
    dispatch(&mut h, "tool.ip", json!({}));
    dispatch(&mut h, "ip.name", json!({"text": "W465"}));
    dispatch(&mut h, "cursor.set", json!({"x_mm": 60.0, "y_mm": 160.0}));
    assert_eq!(
        h.state_mut().shell_mut().dispatch("canvas.activate", &json!({})),
        Outcome::Refused {
            reason_key: key!("reason-ip-not-placed")
        }
    );
    settle(&mut h);
    let texts: Vec<String> = painted_texts(&h).into_iter().map(|(t, _, _)| t).collect();
    assert!(texts.contains(&"Not placed".to_owned()), "{texts:?}");
    let line = "The interruption point W464 is already here; place it on a pin or a free grid point";
    assert!(texts.contains(&line.to_owned()), "{texts:?}");
    let whole =
        "Not placed: the interruption point W464 is already here; place it on a pin or a free grid point";
    assert!(
        h.state()
            .shell()
            .notifications()
            .visible()
            .iter()
            .any(|n| n.text == whole),
        "the toast's own text is the command's"
    );
    h.state_mut().shell_mut().shutdown();
}

/// With the place tool armed, its press in the canvas's top-right quarter places a lamp
/// and sends the Placed toast low. The pointer then travels to the toast's Undo with the tool still armed: the stack
/// stays where it stood on every step, and the click on Undo takes the placing back.
#[test]
fn c52_p3_2_a_placed_toasts_undo_is_reached_with_a_tool_armed() {
    use egui_kittest::kittest::{NodeT as _, Queryable as _};
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    wait_for_page(&mut h);
    dispatch(&mut h, "tool.place", json!({}));
    dispatch(&mut h, "picker.filter", json!({"text": "lamp"}));
    dispatch(&mut h, "picker.choose", json!({}));
    settle(&mut h);
    let rev = h.state().shell().revision();
    let reps = |h: &Harness<'_, App>| {
        h.state()
            .shell()
            .canvas()
            .map_or(0, |c| c.page().tables.reps.len())
    };
    let before = reps(&h);
    let canvas = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas")
        .rect();
    let press = egui::pos2(
        canvas.left() + 0.8 * canvas.width(),
        canvas.top() + 0.25 * canvas.height(),
    );
    let click = |h: &mut Harness<'_, App>, at: egui::Pos2| {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            h.step();
        }
    };
    h.event(egui::Event::PointerMoved(press));
    h.step();
    click(&mut h, press);
    settle(&mut h);
    assert_ne!(h.state().shell().revision(), rev, "the lamp is placed");
    assert_eq!(h.state().shell().tool().name(), "place", "the tool stays armed");
    let toast = |h: &Harness<'_, App>| {
        h.get_all_by_role(egui::Role::Group)
            .find(|n| n.accesskit_node().label().as_deref() == Some("Notification"))
            .expect("the Placed toast")
            .rect()
    };
    let low = toast(&h);
    assert!(
        low.center().y > canvas.center().y,
        "the toast stands low: {low:?}"
    );
    let undo = h
        .get_all_by_role(egui::Role::Button)
        .map(|n| n.rect())
        .find(|r| low.contains_rect(*r) && r.width() > r.height())
        .expect("the toast's Undo");
    for k in 1..=10 {
        let t = k as f32 / 10.0;
        h.event(egui::Event::PointerMoved(press + (undo.center() - press) * t));
        h.step();
        assert_eq!(toast(&h), low, "step {k}: the stack stays");
    }
    click(&mut h, undo.center());
    settle(&mut h);
    assert!(h.state().shell().can_redo(), "Undo ran");
    assert_eq!(reps(&h), before, "Undo took the placing back");
    h.state_mut().shell_mut().shutdown();
}
