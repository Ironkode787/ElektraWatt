// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1699: the supply that enters a drawing set by interruption points (L1, N from another drawing) is marked on the
//! points themselves, by the pointer on a new project: two source points, L1 and N, each with a wire to a relay's coil,
//! as the owner drew them. Check project reports each point as unpaired; Supply comes in here… on both points (their
//! right-click menu, its form's proposals, Apply) makes them external points of one reference, and the findings are
//! gone; Undo brings them back; Remove the supply mark takes the mark off again.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::visual::{Driver, Input};
use serde_json::json;

use crate::wires_and_cables::{click, key, new_project, pixel_map, starting_with, tables, two_wires};

/// The findings of Check project, as (rule, message) pairs; the Messages pane closed again, so the page stays where
/// it was.
fn findings(d: &mut Driver) -> Vec<(String, String)> {
    let shell = d.shell_mut();
    let _ = shell.dispatch("messages.show", &json!({"on": true}));
    let _ = shell.dispatch("messages.check", &json!({}));
    d.settle();
    let found = d
        .shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .map(|t| {
            t.nodes
                .iter()
                .filter(|n| n.role == NavRole::Finding)
                .map(|n| (n.cells[1].clone(), n.label.clone()))
                .collect()
        })
        .unwrap_or_default();
    let _ = d.shell_mut().dispatch("messages.show", &json!({"on": false}));
    d.settle();
    found
}

/// Whether a finding of `rule` is among `of`.
fn has(of: &[(String, String)], rule: &str) -> bool {
    of.iter().any(|(r, _)| r == rule)
}

/// The window pixels of the two points, left to right, each where the pointer hovers the point itself.
fn points(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) -> Vec<(i64, i64)> {
    let page = std::sync::Arc::clone(d.shell_mut().canvas().unwrap().page());
    let mut ips: Vec<_> = page
        .tables
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .map(|r| (r.pos, r.id))
        .collect();
    ips.sort();
    let mut out = Vec::new();
    for (_, id) in ips {
        let rects: Vec<_> = page
            .entities
            .iter()
            .filter(|(e, _)| e.0 == id)
            .map(|(_, b)| *b)
            .collect();
        let found = rects.iter().find_map(|b| {
            let (a, z) = (b.min(), b.max());
            (1..4)
                .flat_map(|i| (1..4).map(move |j| (i, j)))
                .find_map(|(i, j)| {
                    let at = pixel((a.x.0 + (z.x.0 - a.x.0) * i / 4, a.y.0 + (z.y.0 - a.y.0) * j / 4));
                    d.input(&Input::Hover { at });
                    d.settle();
                    let over = d.shell_mut().canvas().unwrap().hover().map(|(e, _)| e.0);
                    (over == Some(id)).then_some(at)
                })
        });
        out.push(found.expect("a pixel that hovers the point"));
    }
    out
}

/// A right click at `at`, then a click on the menu entry whose name starts with `entry`.
fn menu(d: &mut Driver, at: (i64, i64), entry: &str) {
    d.input(&Input::Hover { at });
    d.input(&Input::Click {
        at,
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    });
    d.settle();
    let item = starting_with(d, entry);
    d.input(&click(item));
    d.settle();
}

#[test]
fn supply_comes_in_here_marks_the_points_and_their_findings_go() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let _ = two_wires(&mut d);
    let before = findings(&mut d);
    // The page as it stands after Messages opened and closed.
    let pixel = pixel_map(&mut d);
    assert!(has(&before, "EW-CORE-051"), "{before:?}");
    // Both points selected (as a band or Shift+clicks select them), then a right click on one of them keeps both.
    let ips: Vec<String> = tables(&mut d)
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({ "ids": ips })),
        ew_ui::ew_viewmodel::shell::Outcome::Done
    );
    let at = points(&mut d, &pixel);
    menu(&mut d, at[0], "Supply comes in here");
    assert_eq!(d.shell_mut().selection().items.len(), 2, "both points");
    assert!(d.shell_mut().dialog().is_some(), "its form");
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&click(d.find("Apply").expect("Apply")));
    d.settle();
    let shell = d.shell_mut();
    assert_eq!(shell.recorded_commands().len(), steps + 1, "one step");
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "ip.supply_in");
    let t = tables(&mut d);
    assert!(t.reps.iter().filter_map(|r| r.ip.as_ref()).all(|ip| ip.external));
    let after = findings(&mut d);
    assert!(!has(&after, "EW-CORE-051"), "{after:?}");
    // The points' reference stands clear of their wires, which leave the tail and turn down under the arrow.
    assert!(!has(&after, "EW-CORE-070"), "{after:?}");
    // Undo brings the findings back.
    d.input(&key("escape"));
    d.input(&key("ctrl+z"));
    d.settle();
    assert!(has(&findings(&mut d), "EW-CORE-051"));
    d.input(&key("ctrl+y"));
    d.settle();
    assert!(!has(&findings(&mut d), "EW-CORE-051"));
    // rc.3 (G7): where it comes from can be changed: Where the supply comes from… on a marked point shows the place
    // and device it comes from now, and Apply with another place moves it there.
    let at = points(&mut d, &pixel);
    d.input(&key("escape"));
    d.settle();
    menu(&mut d, at[0], "Where the supply comes from");
    let place = match d.shell_mut().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Form(f)) => f.fields[0].value.clone(),
        other => panic!("the form: {other:?}"),
    };
    let shown_before = d.shell_mut().supply_in_item().map(|i| i.label);
    assert_eq!(shown_before.as_deref(), Some("Where the supply comes from…"));
    d.shell_mut().set_form_field("location", "MCC2");
    d.input(&click(d.find("Apply").expect("Apply")));
    d.settle();
    assert!(
        !place.is_empty() && place != "MCC2",
        "the place it came from: {place}"
    );
    assert_eq!(
        d.shell_mut().recorded_commands().last().unwrap().params["location"],
        json!("MCC2")
    );
    // Remove the supply mark on one marked point, clicked alone, takes it off that point.
    d.input(&key("escape"));
    d.settle();
    menu(&mut d, at[0], "Remove the supply mark");
    assert!(has(&findings(&mut d), "EW-CORE-051"), "L1 is unpaired again");
    let t = tables(&mut d);
    assert_eq!(
        t.reps
            .iter()
            .filter_map(|r| r.ip.as_ref())
            .filter(|ip| ip.external)
            .count(),
        1
    );
    d.shell_mut().shutdown();
}

/// rc.3 (G7): three supply points one under the other (L1, N, PE), marked at once, print where they come from once,
/// under the last of them; the right-click menu takes one out of the group (it prints its own again); Properties of a
/// marked point have its place in a row, and a place typed there moves the point to it.
#[test]
fn supply_points_that_stand_together_print_their_place_once() {
    use ew_ui::ew_viewmodel::properties::{RowValue, find_row};
    use ew_ui::ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    for (name, y) in [("L1", 400), ("N", 460), ("PE", 520)] {
        crate::wires_and_cables::fill(&mut d, "Interruption point name", name);
        d.input(&Input::Hover { at: (900, y) });
        d.settle();
        d.input(&click((900, y)));
        d.settle();
    }
    d.input(&key("escape"));
    d.settle();
    let ips: Vec<String> = tables(&mut d)
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .map(|r| r.id.to_string())
        .collect();
    assert_eq!(ips.len(), 3, "three points");
    let shell = d.shell_mut();
    assert_eq!(shell.dispatch("select.set", &json!({ "ids": ips })), Outcome::Done);
    assert_eq!(
        shell.dispatch("ip.supply_in", &json!({"location": "Supply", "device": "-X0"})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    let printed = |d: &mut Driver| {
        crate::own_parts::page_texts(d)
            .iter()
            .filter(|t| t.contains("-X0"))
            .count()
    };
    assert_eq!(printed(&mut d), 1, "the group prints its place once");
    // The first point alone, by its right-click menu: out of its group, it prints its place itself.
    let pixel = pixel_map(&mut d);
    let at = points(&mut d, &pixel);
    let first = tables(&mut d)
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .min_by_key(|r| (r.pos.1, r.pos.0))
        .map(|r| r.id.to_string())
        .unwrap();
    assert_eq!(
        d.shell_mut().dispatch("select.set", &json!({ "ids": [first] })),
        Outcome::Done
    );
    let top = *at.iter().min_by_key(|p| p.1).unwrap();
    menu(&mut d, top, "Take out of its group");
    assert_eq!(printed(&mut d), 2, "the point taken out prints its own");
    // Its Properties: where it comes from, changed in the row.
    let sheet = d.shell_mut().properties().sheet().expect("the point's properties").clone();
    let row = "function/derived.supply_place";
    match find_row(&sheet, row).map(|(_, r)| r.value.clone()) {
        Some(RowValue::One { text, .. }) => assert_eq!(text, "Supply"),
        other => panic!("{row}: {other:?}"),
    }
    assert_eq!(
        d.shell_mut().dispatch("props.set", &json!({"row": row, "value": "MCC1"})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    let texts = crate::own_parts::page_texts(&mut d);
    assert!(texts.iter().any(|t| t.starts_with("+MCC1")), "{texts:?}");
    // Marked alone from the place of the points under it, it joins their group: the place is printed once again.
    assert_eq!(
        d.shell_mut().dispatch("ip.supply_in", &json!({"location": "Supply", "device": "-X0"})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    assert_eq!(printed(&mut d), 1, "it joined the group under it");
    d.shell_mut().shutdown();
}
