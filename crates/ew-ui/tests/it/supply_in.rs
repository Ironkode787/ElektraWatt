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
    // Undo brings the findings back.
    d.input(&key("escape"));
    d.input(&key("ctrl+z"));
    d.settle();
    assert!(has(&findings(&mut d), "EW-CORE-051"));
    d.input(&key("ctrl+y"));
    d.settle();
    assert!(!has(&findings(&mut d), "EW-CORE-051"));
    // Remove the supply mark on one marked point, clicked alone, takes it off that point.
    let at = points(&mut d, &pixel);
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
