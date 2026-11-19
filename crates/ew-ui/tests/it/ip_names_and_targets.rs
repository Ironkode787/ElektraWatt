// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Interruption tool as a draughtsman carries a supply to several places (the persona round of 0.3.0-rc.5): the
//! name in the field wins, one source feeds every target of its name, and the HUD says before the click which of the
//! two the next click places.

use crate::persona_round::{click, key, new_project, typed};
use crate::support::Tmp;

use ew_ui::visual::{Driver, Input};

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
        Some(ew_ui::ew_viewmodel::tools::Ghost::Ip { name, direction, .. }) => pair(&name, direction),
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
        ew_ui::ew_viewmodel::command_bar::hud(d.shell_mut()).prompt,
        "24V \u{b7} next: where 24V arrives (its source is drawn) \u{b7} D: source"
    );
    d.shell_mut().shutdown();
}
