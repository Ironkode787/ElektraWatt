// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! In the window without rendering: FR-UI-015's
//! keyboard wiring on folio 102 (W, `]`, `[`, Enter) with exactly one new connection and one revision, the junction
//! sequence editor popup driven by its keys, and the exact `Session` reads of the Wire tool in the window. The
//! accessible-name walker runs after every step.

use crate::support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
use serde_json::json;
use support::{Tmp, dispatch, on_golden, screen, settle, wait_for_page};

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
fn fr_ui_015_a_connection_drawn_from_the_keyboard_alone() {
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
fn the_wire_tool_reads_nothing_while_the_cursor_moves_in_the_window() {
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
    // matter kept out of the assertions; the view-model test counts them with the dwell as an action.)
    assert_eq!(
        after.commit + after.execute_with,
        calls.commit + calls.execute_with
    );
    h.state_mut().shell_mut().shutdown();
}

/// The window position (points) of the page point `p` on the focused canvas.
fn screen_of_page(h: &Harness<'_, App>, p: (i64, i64)) -> egui::Pos2 {
    screen(
        h,
        h.state()
            .shell()
            .canvas()
            .unwrap()
            .page()
            .snapper
            .page_to_sheet(p),
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
/// strip says so. The selection survives the page change.
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
        .find(|c| c.polyline.last().is_some_and(|p| p.1 == 15_875_000) && c.polyline[0].0 == 127_000_000)
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

/// C turns connect on move off for the session, and on again; the status bar says
/// the state with the key.
#[test]
fn c_turns_connect_on_move_off_and_on_and_the_status_bar_says_so() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    press(&mut h, Key::C);
    assert_eq!(
        h.state().shell().status_fields(false)[0],
        "C Connect on move: off"
    );
    press(&mut h, Key::C);
    assert_eq!(h.state().shell().status_fields(false)[0], "C Connect on move: on");
    h.state_mut().shell_mut().shutdown();
}

/// The pointer over a cross-reference label of folio 101 that leads to another page shows the pointing hand and
/// the tooltip "Go to <its target's cell or folio>"; over empty sheet the cursor is the default one. The
/// tooltip waits while the link preview shows (the rest opens the preview first) and shows once it is dismissed.
/// The preview says, under its title, how to go there.
#[test]
fn a_page_link_shows_the_pointing_hand_and_its_target_in_a_tooltip() {
    use egui_kittest::kittest::Queryable as _;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let (centre, label) = {
        let c = h.state().shell().canvas().unwrap();
        let link = c
            .page()
            .links
            .iter()
            .find(|l| l.target.is_some())
            .expect("a cross-reference with a target on 101");
        let r = link.rect;
        let (a, b) = (r.min(), r.max());
        (((a.x.0 + b.x.0) / 2, (a.y.0 + b.y.0) / 2), link.label().unwrap())
    };
    let at = screen(&h, centre);
    h.hover_at(at);
    settle(&mut h);
    assert!(h.state().shell().canvas().unwrap().hovered_link().is_some());
    assert!(h.state().shell().canvas().unwrap().hovered_cp().is_none());
    // The hovered link is underlined: a 1-px `canvas.hover` line along its bottom.
    let hover = ew_ui::theme::current(&h.ctx).canvas_hover.color32();
    fn lines(shape: &egui::Shape, colour: egui::Color32, n: &mut usize) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| lines(s, colour, n)),
            egui::Shape::LineSegment { stroke, .. } if stroke.color == colour => *n += 1,
            _ => {}
        }
    }
    let mut underlines = 0;
    for c in &h.output().shapes {
        lines(&c.shape, hover, &mut underlines);
    }
    assert_eq!(underlines, 1, "the link's underline");
    // The tooltip says the gesture.
    let tip = format!("Click to go to {label}");
    // The rest opens the link preview first, and the tooltip waits while it shows ...
    let mut preview = false;
    for _ in 0..120 {
        h.hover_at(at);
        h.step();
        if h.state().shell().link_preview().is_some() {
            preview = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(preview, "the rest over the link shows its preview");
    // The popup says how to go there, on a line under its title, with `nav.follow`'s key.
    let go = "Click the link or press F to go there";
    assert_eq!(h.state().shell().link_preview().unwrap().go, go);
    h.step();
    fn walk(shape: &egui::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, texts)),
            egui::Shape::Text(t) => texts.push(t.galley.text().to_owned()),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut texts);
    }
    assert!(texts.iter().any(|t| t == go), "the popup's line in {texts:?}");
    for _ in 0..30 {
        h.hover_at(at);
        h.step();
        assert!(h.query_by_label(&tip).is_none(), "no tooltip beside the preview");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // ... and shows once the preview is dismissed, the pointer still on the link.
    dispatch(&mut h, "ui.dismiss", json!({}));
    let mut shown = false;
    for _ in 0..120 {
        h.step();
        if h.query_by_label(&tip).is_some() {
            shown = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        shown,
        "a tooltip reads {tip}; preview {} link {}",
        h.state().shell().link_preview().is_some(),
        h.state().shell().canvas().unwrap().hovered_link().is_some()
    );
    assert_eq!(
        h.output().platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    // Over empty sheet (1 mm inside the sheet's top-left corner, outside the frame): the default cursor, no tooltip.
    let empty = screen(&h, (1_000_000, 1_000_000));
    h.hover_at(empty);
    settle(&mut h);
    h.step();
    assert!(h.state().shell().canvas().unwrap().hovered_link().is_none());
    assert_eq!(h.output().platform_output.cursor_icon, egui::CursorIcon::Default);
    assert!(h.query_by_label(&tip).is_none());
    h.state_mut().shell_mut().shutdown();
}

/// The pointer over a fluid line of folio 203 shows a tooltip naming its line kind ("Working
/// line"); over an electrical wire of folio 102 there is none.
#[test]
fn hovering_a_fluid_line_names_its_kind() {
    use egui_kittest::kittest::Queryable as _;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    // The middle of the longest segment of the page's longest fluid line, away from its ends.
    let middle = |h: &Harness<'_, App>, fluid: bool| {
        let c = h.state().shell().canvas().unwrap();
        let length = |w: &[(i64, i64)]| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs();
        let (line, w) = c
            .page()
            .tables
            .connections
            .iter()
            .filter(|c| c.line_kind.is_some() == fluid)
            .flat_map(|c| c.polyline.windows(2).map(move |w| (c, [w[0], w[1]])))
            .max_by_key(|(_, w)| length(w))
            .expect("a line");
        (
            line.line_kind.clone(),
            ((w[0].0 + w[1].0) / 2, (w[0].1 + w[1].1) / 2),
        )
    };
    let (kind, at) = middle(&h, true);
    assert_eq!(kind.as_deref(), Some("working"));
    let at = screen_of_page(&h, at);
    h.hover_at(at);
    settle(&mut h);
    let tip = "Working line";
    assert_eq!(
        h.state().shell().canvas().unwrap().hovered_line_tip().as_deref(),
        Some(tip)
    );
    // The tooltip shows after egui's delay: frames until it does (bounded).
    let mut shown = false;
    for _ in 0..120 {
        h.step();
        if h.query_by_label(tip).is_some() {
            shown = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(shown, "a tooltip reads {tip}");
    // An electrical wire has none.
    dispatch(&mut h, "page.goto", json!({ "folio": "102" }));
    settle(&mut h);
    wait_for_page(&mut h);
    let (kind, at) = middle(&h, false);
    assert_eq!(kind, None);
    let at = screen_of_page(&h, at);
    h.hover_at(at);
    settle(&mut h);
    let c = h.state().shell().canvas().unwrap();
    assert!(c.hover().is_some(), "the wire is hovered");
    assert_eq!(c.hovered_line_tip(), None);
    h.state_mut().shell_mut().shutdown();
}

/// With the tooltip (doc 16a §7.2): the pointer over a connection point of folio 101 shows
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
