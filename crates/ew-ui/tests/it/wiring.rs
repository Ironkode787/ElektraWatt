// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 2 in the window without rendering (acceptance checks WR3 and WR10's window part): FR-UI-015's
//! keyboard wiring on folio 102 (W, `]`, `[`, Enter) with exactly one new connection and one revision, the junction
//! sequence editor popup driven by its keys, and the exact `Session` reads of the Wire tool in the window. The
//! accessible-name walker runs after every step.

use crate::support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn on_golden(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": folio }));
    settle(&mut h);
    wait_for_page(&mut h);
    settle(&mut h);
    h
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action}"
    );
}

fn press(h: &mut Harness<'_, App>, key: Key) {
    h.key_press(key);
    settle(h);
}

/// The sheet point of the CP labelled `label` on the focused page.
fn cp_sheet(h: &Harness<'_, App>, label: &str) -> (i64, i64) {
    let c = h.state().shell().canvas().unwrap();
    let cp = c
        .page()
        .tables
        .cps
        .iter()
        .find(|x| x.label == label)
        .unwrap_or_else(|| panic!("{label}"))
        .clone();
    c.page().snapper.page_to_sheet(cp.at)
}

/// `]` (or `[`) until the grid cursor stands on the CP labelled `label`.
fn step_to(h: &mut Harness<'_, App>, label: &str, key: Key) {
    let target = cp_sheet(h, label);
    let n = h.state().shell().canvas().unwrap().page().cps.len();
    for _ in 0..=n {
        if h.state().shell().canvas().unwrap().cursor() == Some(target) {
            return;
        }
        press(h, key);
    }
    panic!("the cursor never reached {label}");
}

fn connections(h: &Harness<'_, App>) -> Vec<ew_ui::ew_viewmodel::ew_api::output::TableConnection> {
    h.state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .clone()
}

#[test]
fn wr3_fr_ui_015_a_connection_drawn_from_the_keyboard_alone() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    let before = connections(&h);
    let rev = h.state().shell().revision();
    // W arms the Wire tool.
    press(&mut h, Key::W);
    assert_eq!(h.state().shell().tool().name(), "wire");
    // `]` to -102F1:2, Enter starts the wire there.
    step_to(&mut h, "-102F1:2", Key::CloseBracket);
    press(&mut h, Key::Enter);
    assert_eq!(h.state().shell().tool().state(), "routing");
    let live = h
        .state()
        .shell()
        .tool_announcement()
        .unwrap_or_default()
        .to_owned();
    assert!(live.starts_with("Wire: routing from -102F1:2."), "{live}");
    // `[` to -102F2:24VDC#1, Enter ends it.
    step_to(&mut h, "-102F2:24VDC#1", Key::OpenBracket);
    press(&mut h, Key::Enter);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1), "one revision");
    let after = connections(&h);
    let new: Vec<_> = after
        .iter()
        .filter(|c| !before.iter().any(|b| b.id == c.id))
        .collect();
    assert_eq!(new.len(), 1, "exactly one new connection");
    let tables = h.state().shell().canvas().unwrap().page().tables.clone();
    let label = |e: &TableEnd| match e {
        TableEnd::Cp { function, idx } => tables
            .cps
            .iter()
            .find(|c| c.function == *function && c.idx == *idx)
            .map(|c| c.label.clone())
            .unwrap_or_default(),
        TableEnd::Junction { .. } => String::new(),
    };
    let mut ends = [label(&new[0].a), label(&new[0].b)];
    ends.sort();
    assert_eq!(ends, ["-102F1:2".to_owned(), "-102F2:24VDC#1".to_owned()]);
    // N steps to the next unconnected CP; Escape twice ends the tool.
    press(&mut h, Key::N);
    press(&mut h, Key::Escape);
    assert_eq!(h.state().shell().tool().name(), "select");
    h.state_mut().shell_mut().shutdown();
}

/// Ctrl+Enter in the canvas is a click with Alt held at the grid cursor: a placement sends no autoconnect hint.
#[test]
fn ctrl_enter_places_without_autoconnect() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(
        &mut h,
        "place.pick",
        json!({"symbol": "ew.iec.contact-no@qet10", "letter": "K"}),
    );
    settle(&mut h);
    press(&mut h, Key::ArrowRight);
    let reps = h.state().shell().canvas().unwrap().page().tables.reps.len();
    let before = connections(&h).len();
    h.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    settle(&mut h);
    assert_eq!(
        h.state().shell().canvas().unwrap().page().tables.reps.len(),
        reps + 1,
        "placed"
    );
    assert_eq!(connections(&h).len(), before, "no autoconnect wire");
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn the_junction_editor_popup_reorders_with_alt_down_and_closes_with_escape() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    let j = h.state().shell().canvas().unwrap().page().tables.junctions[0].clone();
    dispatch(&mut h, "select.set", json!({"ids": [j.id.to_string()]}));
    dispatch(&mut h, "junction.open", json!({}));
    settle(&mut h);
    let first = h.state().shell().junction_editor().unwrap().sequence_labels();
    let rev = h.state().shell().revision();
    // Alt+Down moves the focused (first) member down: one `junction.edit`.
    h.key_press_modifiers(Modifiers::ALT, Key::ArrowDown);
    settle(&mut h);
    settle(&mut h);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1));
    let now = h.state().shell().junction_editor().unwrap().sequence_labels();
    assert_eq!(now, vec![first[1].clone(), first[0].clone(), first[2].clone()]);
    // Escape closes the popup, not the selection.
    press(&mut h, Key::Escape);
    assert!(h.state().shell().junction_editor().is_none());
    assert_eq!(h.state().shell().selection().items.len(), 1);
    h.state_mut().shell_mut().shutdown();
}

#[test]
fn wr10_the_wire_tool_reads_nothing_while_the_cursor_moves_in_the_window() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    press(&mut h, Key::W);
    step_to(&mut h, "-102F1:2", Key::CloseBracket);
    press(&mut h, Key::Enter);
    let calls = h.state().shell().session_calls();
    for _ in 0..8 {
        press(&mut h, Key::ArrowRight);
    }
    press(&mut h, Key::ArrowDown);
    let after = h.state().shell().session_calls();
    assert_eq!(after.page_content, calls.page_content, "{after:?}");
    assert_eq!(after.page_tables, calls.page_tables);
    assert_eq!(after.page_cps, calls.page_cps);
    assert_eq!(after.symbol_shape, calls.symbol_shape);
    // (Previews are not counted here: the window's dwell timer may fire between frames on a slow machine, a wall-clock
    // matter ID-559 keeps out of the assertions; the view-model test counts them with the dwell as an action.)
    assert_eq!(
        after.commit + after.execute_with,
        calls.commit + calls.execute_with
    );
    h.state_mut().shell_mut().shutdown();
}

/// The window position (points) of the page point `p` on the focused canvas.
fn screen_of_page(h: &Harness<'_, App>, p: (i64, i64)) -> egui::Pos2 {
    use egui_kittest::kittest::{NodeT as _, Queryable};
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = h.state().shell().canvas().unwrap();
    let s = c.page().snapper.page_to_sheet(p);
    let k = c.points_per_nm();
    egui::Pos2::new(
        bounds.x0 as f32 + ((s.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((s.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    )
}

/// A primary-button drag from `a` to `b` in `n` pointer steps, each followed by a frame.
fn drag(h: &mut Harness<'_, App>, a: egui::Pos2, b: egui::Pos2, n: usize) {
    let button = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    h.event(egui::Event::PointerMoved(a));
    h.step();
    h.event(button(a, true));
    h.step();
    for i in 1..=n {
        h.event(egui::Event::PointerMoved(a + (b - a) * (i as f32 / n as f32)));
        h.step();
    }
    h.event(button(b, false));
    settle(h);
}

/// Window check, item 1: a page change ends Edit route (an editing tool works on one page); Select takes over and the
/// strip says so. The selection survives the page change (WP-S2.03 §7.6).
#[test]
fn a_page_change_ends_edit_route_and_select_takes_over() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    let w = connections(&h)[0].id;
    dispatch(&mut h, "select.set", json!({"ids": [w.to_string()]}));
    dispatch(&mut h, "tool.route_edit", json!({}));
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "route_edit");
    dispatch(&mut h, "page.goto", json!({"folio": "101"}));
    settle(&mut h);
    wait_for_page(&mut h);
    assert_eq!(h.state().shell().tool().name(), "select");
    assert!(
        support::hud_value(&h).starts_with("Select \u{b7} "),
        "the HUD shows Select (WP-S2.17 part 2)"
    );
    assert_eq!(
        h.state().shell().selection().items.len(),
        1,
        "the selection survives"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Window check, item 2: the tool strip always holds its row, so a gesture never moves the page view: a horizontal
/// drag of a representation by about two grid steps changes only its x.
#[test]
fn a_horizontal_drag_moves_a_representation_along_x_only() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    assert!(
        !support::hud_value(&h).is_empty(),
        "the HUD at rest (WP-S2.17 part 2: it replaced the tool strip)"
    );
    let rep = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.symbol_key == "ew.iec.mcb-1p@qet10")
        .unwrap()
        .clone();
    let centre = ((rep.body[0] + rep.body[2]) / 2, (rep.body[1] + rep.body[3]) / 2);
    let a = screen_of_page(&h, centre);
    let step = screen_of_page(&h, (centre.0 + 2 * 2_645_833, centre.1)).x - a.x;
    drag(&mut h, a, a + egui::Vec2::new(step, 0.0), 6);
    let moved = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.id == rep.id)
        .unwrap()
        .clone();
    assert_ne!(moved.pos.0, rep.pos.0, "moved along x");
    assert_eq!(moved.pos.1, rep.pos.1, "not along y");
    h.state_mut().shell_mut().shutdown();
}

/// Window check, item 5: at 58 % a press on the middle segment of a selected connection drags it (4 points around the
/// 1-pixel wire), it does not start a rubber band.
#[test]
fn a_segment_drag_at_58_percent_moves_the_middle_segment() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    let w = connections(&h)
        .into_iter()
        .find(|c| c.polyline.last().is_some_and(|p| p.1 == 13_229_167) && c.polyline[0].0 == 121_708_333)
        .expect("-102G1:PE to PE-1.2");
    let (a, b) = (w.polyline[0], *w.polyline.last().unwrap());
    let row = 21_166_667; // grid row 8 (8 × 7 937 500 / 3, rounded)
    let route = json!([[a.0, a.1], [a.0, row], [b.0, row], [b.0, b.1]]);
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "connection.route", "params": {"connection": w.id.to_string(), "route": route}}),
    );
    dispatch(&mut h, "select.set", json!({"ids": [w.id.to_string()]}));
    dispatch(&mut h, "view.zoom_to", json!({"percent": 58}));
    settle(&mut h);
    let rev = h.state().shell().revision();
    let on = screen_of_page(&h, ((a.0 + b.0) / 2, row));
    drag(&mut h, on, on + egui::Vec2::new(0.0, 20.0), 5);
    assert_eq!(
        h.state().shell().revision(),
        rev.map(|r| r + 1),
        "one connection.route"
    );
    let now = connections(&h).into_iter().find(|c| c.id == w.id).unwrap();
    assert!(
        now.polyline.iter().all(|p| p.1 != row),
        "the middle segment moved: {:?}",
        now.polyline
    );
    assert_eq!(
        h.state().shell().selection().items.len(),
        1,
        "the selection stays"
    );
    h.state_mut().shell_mut().shutdown();
}

/// Window check, item 6: C (connect on move) is refused in this package; the status bar says why.
#[test]
fn c_shows_why_it_does_not_connect_while_moving() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    press(&mut h, Key::C);
    let fields = h.state().shell().status_fields(false);
    assert!(
        fields[0].starts_with("Proposals connect when a symbol is placed"),
        "{fields:?}"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-512, with WP-S2.17 part 5's tooltip (OV4, doc 16a §7.2): the pointer over a connection point of folio 101 shows
/// a tooltip with the CP's label as `cp_label` prints it, its kind and its potential (`1X1:1/a · terminal · L`), and
/// none once the pointer leaves it.
#[test]
fn hovering_a_connection_point_shows_its_label_in_a_tooltip() {
    use egui_kittest::kittest::Queryable as _;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let page_point = |h: &Harness<'_, App>, label: &str| {
        let c = h.state().shell().canvas().unwrap();
        c.page()
            .tables
            .cps
            .iter()
            .find(|x| x.label == label)
            .unwrap_or_else(|| panic!("{label}"))
            .at
    };
    assert!(
        h.query_by_label("1X1:1/a").is_none(),
        "no tooltip before the hover"
    );
    for label in ["1X1:1/a", "1X1:1/b"] {
        let at = screen_of_page(&h, page_point(&h, label));
        h.hover_at(at);
        settle(&mut h);
        assert_eq!(
            h.state()
                .shell()
                .canvas()
                .unwrap()
                .hovered_cp()
                .map(|cp| cp.label.clone())
                .as_deref(),
            Some(label)
        );
        // The tooltip shows after egui's delay: frames until it does (bounded).
        let tip = format!("{label} · terminal · L");
        assert_eq!(
            h.state().shell().canvas().unwrap().hovered_cp_tip().as_deref(),
            Some(tip.as_str())
        );
        let mut shown = false;
        for _ in 0..120 {
            h.step();
            if h.query_by_label(&tip).is_some() {
                shown = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(shown, "a tooltip reads {tip}");
    }
    // Away from every CP: no CP, no tooltip.
    h.hover_at(egui::Pos2::new(2.0, 2.0));
    settle(&mut h);
    for _ in 0..10 {
        h.step();
    }
    assert!(h.state().shell().canvas().unwrap().hovered_cp().is_none());
    assert!(h.query_all_by_label_contains("1X1:1/b").next().is_none());
    h.state_mut().shell_mut().shutdown();
}
