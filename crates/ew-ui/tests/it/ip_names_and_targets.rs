// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Interruption tool as a draughtsman carries a supply to several places: the
//! name in the field wins, one source feeds every target of its name, and the HUD says before the click which of the
//! two the next click places. A mark dropped on a pin or beside it joins it, a mark left beside a pin is said, and
//! the check offers to join the two.

use crate::relays_paste_and_ip_wires::{click, cp_at, key, new_project, page_pixel, pixel_map, state, typed};
use crate::support::Tmp;

use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input};
use serde_json::json;

/// (name, direction, cross-reference) of every interruption point of the page on show, sorted.
fn ips(d: &mut Driver) -> Vec<(String, String, String)> {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let mut out: Vec<(String, String, String)> = tables
        .reps
        .iter()
        .filter_map(|r| {
            r.ip.as_ref()
                .map(|ip| (ip.name.clone(), ip.direction.clone(), ip.continuation.clone()))
        })
        .collect();
    out.sort();
    out
}

/// (name, direction) of every interruption point of the page on show, sorted.
fn points(d: &mut Driver) -> Vec<(String, String)> {
    ips(d).into_iter().map(|(n, dir, _)| (n, dir)).collect()
}

fn pair(name: &str, direction: &str) -> (String, String) {
    (name.to_owned(), direction.to_owned())
}

/// The ghost's (name, direction) with the pointer at `at`.
fn ghost_at(d: &mut Driver, at: (i64, i64)) -> (String, String) {
    d.input(&Input::Hover { at });
    d.settle();
    match d.shell_mut().tool_ghost() {
        Some(ew_viewmodel::tools::Ghost::Ip { name, direction, .. }) => pair(&name, direction),
        other => panic!("no interruption point ghost: {other:?}"),
    }
}

fn place(d: &mut Driver, at: (i64, i64)) {
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
}

/// Types `name` into the tool's name field as a person does: a click on it, Ctrl+A, the name, Enter.
fn name_field(d: &mut Driver, name: &str) {
    d.input(&click(d.find("Interruption point name").expect("the name field")));
    d.input(&key("ctrl+a"));
    if name.is_empty() {
        d.input(&key("backspace"));
    } else {
        d.input(&typed(name));
    }
    d.input(&key("enter"));
    d.settle();
}

/// t0045: 24V and 0V placed as sources on the supply page, the tool chosen again later with "0V" standing in the field;
/// he typed 24V, chose Target and clicked: the program placed 0V. Chosen from the bar again, the field takes the keys
/// with the name standing selected, so what is typed replaces it, and the point is the 24V of the field.
#[test]
fn the_name_typed_in_the_field_is_the_points_name_when_the_tool_is_chosen_again() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    d.input(&typed("24V"));
    d.input(&key("enter"));
    place(&mut d, (930, 420));
    name_field(&mut d, "0V");
    d.input(&click(d.find("Source").expect("the Source choice")));
    d.settle();
    place(&mut d, (1060, 420));
    assert_eq!(points(&mut d), [pair("0V", "source"), pair("24V", "source")]);
    d.input(&key("escape"));
    d.settle();
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    d.input(&typed("24V"));
    d.input(&key("enter"));
    d.settle();
    assert_eq!(
        d.shell_mut().ip_typed_name(),
        Some("24V"),
        "the field took the name"
    );
    d.input(&click(d.find("Target").expect("the Target choice")));
    d.settle();
    assert_eq!(ghost_at(&mut d, (930, 620)), pair("24V", "target"));
    place(&mut d, (930, 620));
    assert_eq!(
        points(&mut d),
        [pair("0V", "source"), pair("24V", "source"), pair("24V", "target")]
    );
    d.shell_mut().shutdown();
}

/// t0049: after the first target of 24V his next clicks made "24V" and "24V-1", points of nothing. Every click with
/// 24V in the field places one more place the 24V source feeds, and its cross-reference lists every one; with the
/// field erased, a target proposed from 24V keeps the name as well.
#[test]
fn one_source_feeds_every_target_placed_with_its_name() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    d.input(&typed("24V"));
    d.input(&key("enter"));
    place(&mut d, (930, 420));
    for at in [(800, 520), (1060, 520), (930, 620)] {
        place(&mut d, at);
    }
    assert_eq!(
        d.shell_mut().ip_typed_name(),
        Some("24V"),
        "the field keeps the name"
    );
    name_field(&mut d, "");
    assert_eq!(ghost_at(&mut d, (800, 720)), pair("24V", "target"), "no 24V-1");
    place(&mut d, (800, 720));
    let all = ips(&mut d);
    let targets = all.iter().filter(|(_, dir, _)| dir == "target").count();
    assert_eq!(all.len(), targets + 1, "one source: {all:?}");
    assert!(all.iter().all(|(n, ..)| n == "24V"), "{all:?}");
    // The source's cross-reference lists the place of every target.
    let source = &all.iter().find(|(_, dir, _)| dir == "source").unwrap().2;
    assert_eq!(source.split([',', '\n']).count(), targets, "{source:?}");
    d.shell_mut().shutdown();
}

/// t0032: he placed 24V as a source, and the Source | Target choice turned to Target without a word. Before the next
/// click the HUD says what it places and why, and the choice shows it.
#[test]
fn the_hud_says_before_the_click_that_the_next_point_is_where_the_name_arrives() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    d.input(&typed("24V"));
    d.input(&key("enter"));
    place(&mut d, (930, 420));
    assert_eq!(ghost_at(&mut d, (930, 520)), pair("24V", "target"));
    assert_eq!(
        ew_viewmodel::command_bar::hud(d.shell_mut()).prompt,
        "24V \u{b7} next: where 24V arrives (its source is drawn) \u{b7} D: source"
    );
    d.shell_mut().shutdown();
}

/// A driver over a copy of Golden A on folio 102, the page fitted: -102F2:13 and :14 have no wire.
fn golden_102(tmp: &Tmp) -> Driver {
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("page.goto", &json!({"folio": "102"})),
        Outcome::Done
    );
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    d.settle();
    d
}

/// The wires that end at the connection point `label` on the page shown.
fn wires_at(d: &mut Driver, label: &str) -> usize {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    tables
        .cps
        .iter()
        .find(|c| c.label == label)
        .map_or(0, |c| c.connections.len())
}

fn toast(d: &mut Driver) -> String {
    state(d)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

/// t0203 to t0210: he dropped source marks on the supply pins; a source's wire end is at its arrow's tail, so the mark
/// stood a grid step beside the pin, unjoined, and the check then said "has no wire" and offered "Mark not used". A
/// mark dropped on a pin joins it whatever its form; one placed on it with Alt is told that it does not join it, and
/// the check says the mark stands beside the pin and its fix joins the two.
#[test]
fn a_mark_dropped_on_a_pin_joins_it_and_one_left_beside_it_is_told_and_joined_by_the_fix() {
    let tmp = Tmp::new();
    let mut d = golden_102(&tmp);
    let pixel = pixel_map(&mut d);
    let pin = cp_at(&mut d, "-102F2:14");
    let at = page_pixel(&mut d, &pixel, pin);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    d.input(&typed("+24V"));
    d.input(&key("enter"));
    d.input(&click(d.find("Source").expect("the Source choice")));
    d.settle();
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
    assert_eq!(wires_at(&mut d, "-102F2:14"), 1, "{}", toast(&mut d));
    assert!(toast(&mut d).contains("and connected it to"), "{}", toast(&mut d));
    // The same with Alt, which places a point without joining: it stands on the pin unjoined, and says so.
    d.input(&key("ctrl+z"));
    d.settle();
    assert_eq!(wires_at(&mut d, "-102F2:14"), 0);
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&Input::Click {
        at,
        button: egui::PointerButton::Primary,
        mods: egui::Modifiers::ALT,
    });
    d.settle();
    assert_eq!(wires_at(&mut d, "-102F2:14"), 0);
    let said = toast(&mut d);
    assert!(
        said.contains("+24V stands beside -102F2") && said.contains("without joining it"),
        "{said}"
    );
    d.input(&key("escape"));
    d.settle();
    // The check: the pin's message names the mark, and its fix joins the two.
    assert_eq!(
        d.shell_mut().dispatch("messages.check", &json!({})),
        Outcome::Done
    );
    assert_eq!(
        d.shell_mut().dispatch("messages.show", &json!({"on": true})),
        Outcome::Done
    );
    d.settle();
    let row = d
        .shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| {
                    n.role == NavRole::Finding && n.cells[1] == "EW-POT-009" && n.cells[3].ends_with(":14")
                })
                .cloned()
        })
        .expect("the pin's message");
    assert!(
        row.cells[2].contains("and the mark +24V stands right beside it without joining it"),
        "{:?}",
        row.cells
    );
    assert_eq!(row.cells[5], "Join the pin and the mark");
    assert_eq!(
        d.shell_mut().dispatch("messages.fix", &json!({"key": row.key})),
        Outcome::Done
    );
    assert_eq!(
        d.shell_mut().dispatch("navigator.apply", &json!({})),
        Outcome::Done
    );
    d.settle();
    assert_eq!(wires_at(&mut d, "-102F2:14"), 1, "the fix joins them");
    d.shell_mut().shutdown();
}

/// t0049, t0050: his second supply mark, on the pin beside the first, was refused as too close, the tool had turned
/// to Target, and his click on Source seemed to do nothing. A mark on a pin is never refused for room; the direction
/// the bar shows before the click is the one placed; a click on Source or Target sets the next point's direction and
/// the HUD says the same; a free point too close says how far apart the two must stand.
#[test]
fn the_direction_the_bar_shows_is_placed_and_a_point_too_close_says_how_far_apart() {
    let tmp = Tmp::new();
    let mut d = golden_102(&tmp);
    let pixel = pixel_map(&mut d);
    let (p13, p14) = (cp_at(&mut d, "-102F2:13"), cp_at(&mut d, "-102F2:14"));
    let first = page_pixel(&mut d, &pixel, p13);
    let second = page_pixel(&mut d, &pixel, p14);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    d.input(&typed("+24V"));
    d.input(&key("enter"));
    d.settle();
    let shown = ghost_at(&mut d, first).1;
    place(&mut d, first);
    assert!(
        points(&mut d).contains(&pair("+24V", &shown)),
        "{:?} {}",
        points(&mut d),
        toast(&mut d)
    );
    // The second mark, on the pin beside the first, with its own name.
    name_field(&mut d, "0V");
    let shown = ghost_at(&mut d, second).1;
    place(&mut d, second);
    assert!(points(&mut d).contains(&pair("0V", &shown)), "{}", toast(&mut d));
    assert_eq!(wires_at(&mut d, "-102F2:13") + wires_at(&mut d, "-102F2:14"), 2);
    // The two buttons set the next point's direction, and the HUD says the same.
    let free = (first.0, first.1 - 200);
    for (button, word, next) in [("Target", "target", "source"), ("Source", "source", "target")] {
        d.input(&click(d.find(button).unwrap()));
        d.settle();
        assert_eq!(ghost_at(&mut d, free).1, word);
        let hud = ew_viewmodel::command_bar::hud(d.shell_mut()).prompt;
        assert!(hud.ends_with(&format!("D: {next}")), "{hud}");
    }
    // A free point right above another: refused, with the distance the two must stand apart.
    name_field(&mut d, "X9");
    let free = (first.0 + 300, first.1 - 200);
    place(&mut d, free);
    place(&mut d, (free.0, free.1 + 16));
    let said = toast(&mut d);
    assert!(
        said.contains("grid steps (") && said.contains("mm) apart"),
        "{said}"
    );
    d.shell_mut().shutdown();
}
