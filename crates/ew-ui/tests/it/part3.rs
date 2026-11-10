// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.04 part 3 in the window without rendering: the IP tool's name field in the tool strip keeps the canvas keys
//! (P, D, B) while it has the focus (the `text_edit_focused` gate of the key routing in `app.rs`), and Enter sends the
//! name to the tool.

use crate::support;

use egui::Key;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
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

fn ghost(h: &Harness<'_, App>) -> Option<ew_ui::ew_viewmodel::tools::Ghost> {
    h.state().shell().tool_ghost()
}

fn direction(h: &Harness<'_, App>) -> &'static str {
    match ghost(h) {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { direction, .. }) => direction,
        other => panic!("not the IP tool's ghost: {other:?}"),
    }
}

#[test]
fn p_d_and_b_stay_with_the_ip_name_field_while_it_has_the_focus() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    // P on the canvas arms the IP tool; D turns its direction.
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    let first = direction(&h);
    press(&mut h, Key::D);
    let second = direction(&h);
    assert_ne!(first, second, "D reaches the tool from the canvas");
    // The name field in the tool strip takes the focus and the text.
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field");
    field.click();
    settle(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field");
    field.type_text("X1");
    settle(&mut h);
    // While it has the focus, P, D and B are the field's: the tool, its direction and the active tool stay.
    let rev = h.state().shell().revision();
    for k in [Key::P, Key::D, Key::B] {
        press(&mut h, k);
    }
    assert_eq!(h.state().shell().tool().name(), "ip", "B arms no black box tool");
    assert_eq!(direction(&h), second, "D does not reach the tool");
    assert_eq!(h.state().shell().revision(), rev);
    // "Enter or Tab ends the name and gives the keys back to the tool." Enter sends the name, the
    // field gives up the focus, and D turns the tool again.
    press(&mut h, Key::Enter);
    assert_eq!(ghost_name(&h), "X1");
    assert!(!h.ctx.text_edit_focused(), "Enter ends the field's focus");
    press(&mut h, Key::D);
    assert_ne!(direction(&h), second, "D reaches the tool again");
    assert_eq!(ghost_name(&h), "X1", "the tool keeps the name");
}

/// The name the IP tool's ghost carries.
fn ghost_name(h: &Harness<'_, App>) -> String {
    match ghost(h) {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { name, .. }) => name,
        other => panic!("not the IP tool's ghost: {other:?}"),
    }
}

/// "Enter or Tab ends the name and gives the keys back to the tool." Tab does what Enter does: the
/// focus goes to no other widget, Tab does not reach the canvas, and D turns the tool again.
#[test]
fn tab_ends_the_ip_name_and_gives_the_keys_back_to_the_tool() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    let first = direction(&h);
    type_in_name_field(&mut h, "X1");
    press(&mut h, Key::D);
    assert_eq!(direction(&h), first, "D is the field's");
    press(&mut h, Key::Tab);
    assert!(!h.ctx.text_edit_focused(), "Tab ends the field's focus");
    let focused: Vec<egui::Role> = h
        .query_all(egui_kittest::kittest::by().predicate(|n| n.is_focused()))
        .map(|n| n.accesskit_node().role())
        .collect();
    assert_eq!(focused, [egui::Role::Canvas], "the keys go back to the canvas");
    assert_eq!(ghost_name(&h), "X1");
    press(&mut h, Key::D);
    assert_ne!(direction(&h), first, "D reaches the tool again");
    assert_eq!(ghost_name(&h), "X1", "the tool keeps the name");
}

/// The field's tooltip names the keys that end it (doc 16a P8).
#[test]
fn the_ip_name_tooltip_names_the_keys_that_end_it() {
    const TIP: &str = "Enter or Tab ends the name and gives the keys back to the tool";
    assert_eq!(
        ew_ui::ew_viewmodel::strings::text(ew_ui::ew_viewmodel::key!("action-ip-name-hint")),
        TIP
    );
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    press(&mut h, Key::P);
    assert!(h.query_by_label(TIP).is_none(), "no tooltip before the hover");
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .hover();
    settle(&mut h);
    settle(&mut h);
    assert!(h.query_by_label(TIP).is_some(), "the field's tooltip");
}

/// The IP points of the page on show: (representation, name).
fn ips(h: &Harness<'_, App>) -> Vec<(String, String)> {
    h.state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .filter_map(|r| r.ip.as_ref().map(|ip| (r.id.to_string(), ip.name.clone())))
        .collect()
}

/// The text of the IP name field.
fn name_field_text(h: &Harness<'_, App>) -> Option<String> {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .accesskit_node()
        .value()
        .map(|v| v.to_string())
}

/// Clicks the IP name field (it takes the focus) and types `text` into it.
fn type_in_name_field(h: &mut Harness<'_, App>, text: &str) {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .click();
    settle(h);
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .type_text(text);
    settle(h);
}

/// The play-test of cycle 40 (finding 5): a name typed in Tool options and left by a click on the canvas, without
/// Enter, names the point that click places; the field empties (its own clear on the lost focus cancels nothing).
#[test]
fn a_typed_name_left_by_a_click_on_the_canvas_names_the_placed_point() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    type_in_name_field(&mut h, "0V");
    assert_eq!(click_canvas_centre(&mut h), vec!["0V".to_owned()]);
    // The point took the typed name, so the field is empty again.
    assert_eq!(name_field_text(&h).unwrap_or_default(), "");
}

/// The field shows the typed name after the focus leaves it, until a point takes it; empty, it shows
/// the HUD's proposal as its hint.
#[test]
fn ft851_the_ip_name_field_keeps_the_typed_name_until_a_point_takes_it() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    let proposal = ghost_name(&h);
    assert!(!proposal.is_empty());
    assert_eq!(name_field_text(&h).unwrap_or_default(), "");
    assert_eq!(name_field_placeholder(&h).as_deref(), Some(proposal.as_str()));
    type_in_name_field(&mut h, "X1");
    press(&mut h, Key::Tab);
    assert!(!h.ctx.text_edit_focused(), "Tab ends the field's focus");
    assert_eq!(
        name_field_text(&h).as_deref(),
        Some("X1"),
        "the field keeps the typed name"
    );
    assert_eq!(h.state().shell().ip_typed_name(), Some("X1"));
    assert_eq!(click_canvas_centre(&mut h), vec!["X1".to_owned()]);
    assert_eq!(h.state().shell().ip_typed_name(), None);
    assert_eq!(
        name_field_text(&h).unwrap_or_default(),
        "",
        "a point took the name"
    );
}

/// The hint text of the IP name field.
fn name_field_placeholder(h: &Harness<'_, App>) -> Option<String> {
    h.get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .accesskit_node()
        .placeholder()
        .map(|v| v.to_string())
}

/// The cycle-41 review (finding 3): a name typed and then erased is cancelled, so the point the next click places
/// takes its automatic name, not the last text the field sent ("0").
#[test]
fn a_typed_name_erased_from_the_field_is_cancelled() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    type_in_name_field(&mut h, "0V");
    press(&mut h, Key::Backspace);
    press(&mut h, Key::Backspace);
    assert_eq!(name_field_text(&h).unwrap_or_default(), "");
    match ghost(&h) {
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { name, .. }) => assert_eq!(name, "IP"),
        other => panic!("not the IP tool's ghost: {other:?}"),
    }
    assert_eq!(click_canvas_centre(&mut h), vec!["IP".to_owned()]);
}

/// A click in the middle of the canvas (no Enter before it); returns the names of the interruption points it placed.
fn click_canvas_centre(h: &mut Harness<'_, App>) -> Vec<String> {
    let bounds = h
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let at = egui::pos2(
        ((bounds.x0 + bounds.x1) / 2.0) as f32,
        ((bounds.y0 + bounds.y1) / 2.0) as f32,
    );
    let before = ips(h);
    h.event(egui::Event::PointerMoved(at));
    settle(h);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(h);
    ips(h)
        .into_iter()
        .filter(|ip| !before.contains(ip))
        .map(|(_, name)| name)
        .collect()
}

/// FT-1086 at the floor: the IP tool's Tool options card takes the pointer only with its controls. A click on
/// its title (its free area) places a point on the sheet beneath; while the tool's ghost is under the card the card is
/// drawn at 40 % opacity and is no chip obstacle, and away from it the card is whole and an obstacle again.
#[test]
fn ft1086_a_click_on_the_free_area_of_the_tool_options_card_reaches_the_sheet() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    settle(&mut h);
    let t = ew_ui::theme::current(&h.ctx);
    let title = h.get_by_label("Tool options").rect();
    let card_fill = t.surface_panel.color32();
    let faded_fill = card_fill.gamma_multiply(0.4);
    // The painted card: the largest filled rectangle in the card's fill colour, whole or at 40 %.
    let painted = |h: &Harness<'_, App>, fill: egui::Color32| {
        fn walk(shape: &egui::Shape, fill: egui::Color32, found: &mut bool) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, fill, found)),
                egui::Shape::Rect(r) => {
                    *found |= r.fill == fill && r.rect.width() > 100.0 && r.rect.width() < 600.0
                }
                _ => {}
            }
        }
        let mut found = false;
        for c in &h.output().shapes {
            walk(&c.shape, fill, &mut found);
        }
        found
    };
    // The pointer away from the card: the card is whole and a chip obstacle (the HUD, the zoom control, the card).
    let away = egui::pos2(title.right() + 300.0, title.center().y + 200.0);
    h.event(egui::Event::PointerMoved(away));
    settle(&mut h);
    settle(&mut h);
    // The three by membership, no total (a set that grows). The zoom control by its bottom-right corner.
    let (_, hud, pane) = h.state().command_surface_rects();
    let (hud, pane) = (hud.unwrap(), pane.unwrap());
    let zoom_corner = egui::pos2(pane.right() - t.space_4 - 1.0, pane.bottom() - t.space_5 - 1.0);
    let obstacles = h.state_mut().canvas_host().chrome_obstacles(0).to_vec();
    assert!(obstacles.contains(&hud), "the HUD: {obstacles:?}");
    assert!(
        obstacles.iter().any(|r| r.contains(zoom_corner)),
        "the zoom control: {obstacles:?}"
    );
    assert!(
        obstacles.iter().any(|r| r.contains(title.center())),
        "the card: {obstacles:?}"
    );
    assert!(painted(&h, card_fill), "the card whole");
    assert!(!painted(&h, faded_fill));
    // The pointer on the card's title: the ghost is under it.
    h.event(egui::Event::PointerMoved(title.center()));
    settle(&mut h);
    settle(&mut h);
    let (_, hud, _) = h.state().command_surface_rects();
    let obstacles = h.state_mut().canvas_host().chrome_obstacles(0).to_vec();
    assert!(obstacles.contains(&hud.unwrap()), "the HUD: {obstacles:?}");
    assert!(
        obstacles.iter().any(|r| r.contains(zoom_corner)),
        "the zoom control: {obstacles:?}"
    );
    assert!(
        !obstacles.iter().any(|r| r.contains(title.center())),
        "no obstacle for a ghost under the card: {obstacles:?}"
    );
    assert!(painted(&h, faded_fill), "the card at 40 %");
    // A click there places a point on the sheet.
    let before = ips(&h);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: title.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(&mut h);
    assert_eq!(ips(&h).len(), before.len() + 1, "the click reached the sheet");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1115 at the floor: the Tool options card's band takes the pointer only on its controls. A click in the
/// gap between the IP name's label and its field places a point on the sheet beneath; a click on the field does not.
/// Folio 3, whose sheet is free beneath the band (103's routed wires run there, and a point is not placed on a wire).
#[test]
fn ft1115_a_click_in_a_gap_of_the_tool_options_band_reaches_the_sheet() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "3");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    press(&mut h, Key::P);
    assert_eq!(h.state().shell().tool().name(), "ip");
    settle(&mut h);
    let field = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Interruption point name"))
        .expect("the IP name field")
        .rect();
    let label = h
        .get_all_by_role(egui::Role::Label)
        .find(|n| n.accesskit_node().value().as_deref() == Some("Interruption point name"))
        .expect("the IP name's label")
        .rect();
    assert!(
        label.right() < field.left(),
        "a gap between {label:?} and {field:?}"
    );
    let click = |h: &mut Harness<'_, App>, at: egui::Pos2| {
        h.event(egui::Event::PointerMoved(at));
        settle(h);
        settle(h);
        let before = ips(h).len();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        settle(h);
        ips(h).len() - before
    };
    let gap = egui::pos2((label.right() + field.left()) / 2.0, field.center().y);
    assert_eq!(click(&mut h, gap), 1, "the click in the gap reached the sheet");
    assert_eq!(click(&mut h, field.center()), 0, "the field takes its click");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1077 at the floor: the Interruption point row of a placed point's function has no value, so its label
/// runs across the value column and is whole; a row with a value keeps the label column (M39).
#[test]
fn ft1077_a_row_with_no_value_lets_its_label_run_across_the_value_column() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    press(&mut h, Key::P);
    click_canvas_centre(&mut h);
    let id = ips(&h)[0].0.clone();
    press(&mut h, Key::Escape);
    dispatch(&mut h, "select.set", json!({"ids": [id]}));
    settle(&mut h);
    settle(&mut h);
    let t = ew_ui::theme::current(&h.ctx);
    let label_rect = |h: &Harness<'_, App>, label: &str| {
        let row = h
            .get_all_by_role(egui::Role::Group)
            .find(|n| n.accesskit_node().label().as_deref() == Some(label))
            .unwrap_or_else(|| panic!("the row {label}"))
            .rect();
        h.get_all_by_role(egui::Role::Button)
            .find(|n| n.accesskit_node().label().as_deref() == Some(label) && row.contains_rect(n.rect()))
            .unwrap_or_else(|| panic!("the label {label}"))
            .rect()
    };
    let whole = h.ctx.fonts_mut(|f| {
        f.layout_no_wrap(
            "Interruption point".to_owned(),
            ew_ui::theme::font(&t.type_small),
            egui::Color32::WHITE,
        )
        .size()
        .x
    });
    let empty = label_rect(&h, "Interruption point");
    assert!(
        empty.width() >= whole,
        "the label needs {whole} and has {}",
        empty.width()
    );
    // A row with a value keeps the column the labels share.
    let with_value = label_rect(&h, "Direction");
    let name = label_rect(&h, "Name");
    assert_eq!(with_value.width(), name.width());
    assert!(
        with_value.width() <= empty.width(),
        "the column {} is wider than the run-on label {} (FT-1413: the column may take its labels' width)",
        with_value.width(),
        empty.width()
    );
}
