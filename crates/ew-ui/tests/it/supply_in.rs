// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The supply that enters a drawing set by interruption points (L1, N from another drawing) is marked on the
//! points themselves, by the pointer on a new project: two source points, L1 and N, each with a wire to a relay's coil,
//! as the owner drew them. Check project reports each point as unpaired; Supply comes in here… on both points (their
//! right-click menu, its form's proposals, Apply) makes them external points of one reference, and the findings are
//! gone; Undo brings them back; Remove the supply mark takes the mark off again.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::visual::{Driver, Input};
use ew_viewmodel::navigators::{NavKind, NavRole};
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
        ew_viewmodel::shell::Outcome::Done
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
    // Where it comes from can be changed: Where the supply comes from… on a marked point shows the place
    // and device it comes from now, and Apply with another place moves it there.
    let at = points(&mut d, &pixel);
    d.input(&key("escape"));
    d.settle();
    menu(&mut d, at[0], "Where the supply comes from");
    let place = match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => f.fields[0].value.clone(),
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

/// Three supply points one under the other (L1, N, PE), marked at once, print where they come from once,
/// under the last of them; the right-click menu takes one out of the group (it prints its own again); Properties of a
/// marked point have its place in a row, and a place typed there moves the point to it.
#[test]
fn supply_points_that_stand_together_print_their_place_once() {
    use ew_viewmodel::properties::{RowValue, find_row};
    use ew_viewmodel::shell::Outcome;
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
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": ips })),
        Outcome::Done
    );
    assert_eq!(
        shell.dispatch("ip.supply_in", &json!({"location": "Supply", "device": "-X0"})),
        Outcome::Done
    );
    // The marks' Potential panel (H9): Not now.
    assert_eq!(shell.dispatch("ui.dismiss", &json!({})), Outcome::Done);
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
    let sheet = d
        .shell_mut()
        .properties()
        .sheet()
        .expect("the point's properties")
        .clone();
    let row = "function/derived.supply_place";
    match find_row(&sheet, row).map(|(_, r)| r.value.clone()) {
        Some(RowValue::One { text, .. }) => assert_eq!(text, "Supply"),
        other => panic!("{row}: {other:?}"),
    }
    assert_eq!(
        d.shell_mut()
            .dispatch("props.set", &json!({"row": row, "value": "MCC1"})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    let texts = crate::own_parts::page_texts(&mut d);
    assert!(texts.iter().any(|t| t.starts_with("+MCC1")), "{texts:?}");
    // A place emptied is not known: the point prints the device there alone.
    assert_eq!(
        d.shell_mut()
            .dispatch("props.set", &json!({"row": row, "value": " "})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    let texts = crate::own_parts::page_texts(&mut d);
    assert!(texts.iter().any(|t| t == "-X0"), "{texts:?}");
    // Marked alone from the place of the points under it, it joins their group: the place is printed once again.
    assert_eq!(
        d.shell_mut()
            .dispatch("ip.supply_in", &json!({"location": "Supply", "device": "-X0"})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    assert_eq!(printed(&mut d), 1, "it joined the group under it");
    d.shell_mut().shutdown();
}

/// H9: supply marks placed in one go ask what comes in. Three points L1, N and PE drawn with the Interruption tool, N
/// defined as the neutral first; Supply comes in here… on all three opens the Potential panel at them with a row per
/// mark whose potential has no type yet (L1 and PE: N continues its potential and is not asked), pre-filled from the
/// names. A voltage typed in the L1 row and Enter define both potentials in one undo step; Potential… on the marks'
/// right-click menu shows what they carry now. Escape on a fresh mark places it without a potential.
#[test]
fn supply_marks_placed_in_one_go_ask_their_potential() {
    use ew_viewmodel::shell::Outcome;
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
    let reps: Vec<(i64, String)> = {
        let mut r: Vec<_> = tables(&mut d)
            .reps
            .iter()
            .filter(|r| r.ip.is_some())
            .map(|r| (r.pos.1, r.id.to_string()))
            .collect();
        r.sort();
        r
    };
    assert_eq!(reps.len(), 3, "three points");
    let shell = d.shell_mut();
    // N is the neutral already.
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": [reps[1].1] })),
        Outcome::Done
    );
    assert_eq!(
        shell.dispatch("potential.draw", &json!({"name": "N", "type": "N"})),
        Outcome::Done
    );
    let ids: Vec<&String> = reps.iter().map(|(_, id)| id).collect();
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": ids })),
        Outcome::Done
    );
    assert_eq!(
        shell.dispatch("ip.supply_in", &json!({"location": "Supply", "device": "-X0"})),
        Outcome::Done
    );
    d.settle();
    let rows = |d: &mut Driver| -> Vec<(String, String, String)> {
        d.shell_mut()
            .supply_panel()
            .map(|p| {
                p.rows
                    .iter()
                    .map(|r| (r.name.clone(), r.kind.clone(), r.voltage.clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        rows(&mut d),
        vec![
            ("L1".to_owned(), "l1".to_owned(), String::new()),
            ("PE".to_owned(), "pe".to_owned(), String::new()),
        ],
        "the panel asks for the marks whose potential has no type, pre-filled from their names"
    );
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    assert!(d.find("Not now").is_some(), "the panel is shown");
    // A voltage typed in the L1 row, then Enter.
    assert_eq!(
        d.shell_mut()
            .dispatch("ip.supply_potential", &json!({"row": 0, "voltage": "230 V AC"})),
        Outcome::Done
    );
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&key("enter"));
    d.settle();
    assert!(d.shell_mut().supply_panel().is_none(), "Enter closes the panel");
    let defined: Vec<_> = d.shell_mut().recorded_commands()[steps..]
        .iter()
        .map(|c| (c.cmd.clone(), c.params["name"].clone(), c.params["ptype"].clone()))
        .collect();
    assert_eq!(
        defined,
        vec![
            ("potential.define".to_owned(), json!("L1"), json!("l1")),
            ("potential.define".to_owned(), json!("PE"), json!("pe")),
        ]
    );
    assert_eq!(
        d.shell_mut().recorded_commands()[steps].params["env"],
        json!({"ac_dc": "ac"})
    );
    // Potential… on the marks' right-click menu shows what they carry now.
    let shell = d.shell_mut();
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": [reps[0].1] })),
        Outcome::Done
    );
    assert_eq!(
        shell.supply_potential_item().map(|i| i.label).as_deref(),
        Some("Potential…")
    );
    assert_eq!(shell.dispatch("ip.supply_potential", &json!({})), Outcome::Done);
    assert_eq!(
        rows(&mut d),
        vec![("L1".to_owned(), "l1".to_owned(), "230 V AC".to_owned())]
    );
    // Escape closes it and changes nothing.
    let steps = d.shell_mut().recorded_commands().len();
    d.input(&key("escape"));
    d.settle();
    assert!(d.shell_mut().supply_panel().is_none());
    assert_eq!(d.shell_mut().recorded_commands().len(), steps);
    // One undo takes both definitions back: L1's row is its name's again.
    assert_eq!(d.shell_mut().dispatch("edit.undo", &json!({})), Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("ip.supply_potential", &json!({})),
        Outcome::Done
    );
    assert_eq!(rows(&mut d)[0], ("L1".to_owned(), "l1".to_owned(), String::new()));
    d.shell_mut().shutdown();
}

/// The Potential panel stands at marks of the page it opened on, as the project stood: an Undo closes it, so
/// does a change of page; and an Enter after the marks were deleted defines nothing and says so in words, never "the
/// device".
#[test]
fn the_potential_panel_closes_on_undo_and_on_a_page_change() {
    use ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    for (name, y) in [("L1", 400), ("PE", 460)] {
        crate::wires_and_cables::fill(&mut d, "Interruption point name", name);
        d.input(&Input::Hover { at: (900, y) });
        d.settle();
        d.input(&click((900, y)));
        d.settle();
    }
    d.input(&key("escape"));
    d.settle();
    let ids: Vec<String> = tables(&mut d)
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .map(|r| r.id.to_string())
        .collect();
    let shell = d.shell_mut();
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": ids })),
        Outcome::Done
    );
    let supply = json!({"location": "Supply", "device": "-X0"});
    assert_eq!(shell.dispatch("ip.supply_in", &supply), Outcome::Done);
    assert!(shell.supply_panel().is_some(), "the panel asks");
    // An Undo takes the marks back: the panel goes with them.
    assert_eq!(shell.dispatch("edit.undo", &json!({})), Outcome::Done);
    assert!(shell.supply_panel().is_none(), "Undo closes the panel");
    assert_eq!(shell.dispatch("edit.redo", &json!({})), Outcome::Done);
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": ids })),
        Outcome::Done
    );
    assert_eq!(shell.dispatch("ip.supply_potential", &json!({})), Outcome::Done);
    assert!(shell.supply_panel().is_some(), "Potential… opens it again");
    // Another page shown: the panel goes.
    let page = json!({"folio": "2", "type": "multi_line", "title": ""});
    assert_eq!(shell.dispatch("page.new", &page), Outcome::Done);
    d.settle();
    assert!(
        d.shell_mut().supply_panel().is_none(),
        "a page change closes the panel"
    );
    // The marks deleted under the open panel: Enter defines nothing and names no device.
    let shell = d.shell_mut();
    let _ = shell.dispatch("page.goto", &json!({"folio": "1"}));
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": ids })),
        Outcome::Done
    );
    assert_eq!(shell.dispatch("ip.supply_potential", &json!({})), Outcome::Done);
    let rev = shell.revision();
    d.input(&key("delete"));
    d.settle();
    let shell = d.shell_mut();
    if shell.revision() == rev {
        let _ = shell.dispatch("preview.apply", &json!({}));
    }
    assert_ne!(shell.revision(), rev, "the marks are deleted");
    assert!(
        shell.supply_panel().is_some(),
        "the panel stands over the deleted marks"
    );
    let out = shell.dispatch("ip.supply_potential", &json!({"apply": true}));
    assert_eq!(
        out,
        Outcome::refused(ew_viewmodel::key!("reason-supply-potential-gone"))
    );
    assert!(shell.supply_panel().is_none(), "and it closes");
    d.shell_mut().shutdown();
}

/// Supply on the Draw tab asks what comes in, the marks
/// float as one group, and the click places them as supply marks from the place typed, one under the other, in one
/// undo step; their Potential panel opens with a row per mark.
#[test]
fn supply_on_the_draw_tab_places_the_marks_of_what_comes_in() {
    use ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Supply").expect("Supply on the Draw tab")));
    d.settle();
    assert!(
        matches!(d.shell_mut().dialog(), Some(ew_viewmodel::shell::Dialog::Form(_))),
        "its form"
    );
    d.shell_mut().set_form_field("system", "supply_3p");
    d.shell_mut().set_form_field("location", "Mains");
    // The form's Place (the bar has a Place of its own, above it).
    let place = d
        .labels()
        .into_iter()
        .filter(|(l, _)| l == "Place")
        .max_by_key(|(_, at)| at.1)
        .expect("the form's Place")
        .1;
    d.input(&click(place));
    d.settle();
    assert_eq!(d.shell_mut().tool().name(), "supply");
    d.input(&Input::Hover { at: (900, 400) });
    d.settle();
    d.input(&click((900, 400)));
    d.settle();
    let t = tables(&mut d);
    let mut marks: Vec<(i64, String, bool)> = t
        .reps
        .iter()
        .filter_map(|r| r.ip.as_ref().map(|ip| (r.pos.1, ip.name.clone(), ip.external)))
        .collect();
    marks.sort();
    let names: Vec<&str> = marks.iter().map(|(_, n, _)| n.as_str()).collect();
    assert_eq!(names, ["L1", "L2", "L3", "N", "PE"], "one under the other");
    assert!(
        marks.iter().all(|(_, _, e)| *e),
        "all marked as where the supply comes in"
    );
    let rows: Vec<String> = d
        .shell_mut()
        .supply_panel()
        .map(|p| p.rows.iter().map(|r| r.kind.clone()).collect())
        .unwrap_or_default();
    assert_eq!(
        rows,
        ["l1", "l2", "l3", "n", "pe"],
        "the Potential panel, filled in from the names"
    );
    // One undo takes the whole group back.
    assert_eq!(d.shell_mut().dispatch("ui.dismiss", &json!({})), Outcome::Done);
    assert_eq!(d.shell_mut().dispatch("edit.undo", &json!({})), Outcome::Done);
    d.settle();
    assert!(
        tables(&mut d).reps.iter().all(|r| r.ip.is_none()),
        "one undo step"
    );
    d.shell_mut().shutdown();
}

/// The names field comes with Own,
/// in plain words, and goes with another choice.
#[test]
fn the_supply_forms_names_field_comes_with_own() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Supply").expect("Supply on the Draw tab")));
    d.settle();
    let names = "Names, one per conductor";
    assert!(!d.texts().iter().any(|t| t == names), "not for single phase");
    // The list's choice as the window sends it, then a frame drawn.
    let choose = |d: &mut Driver, system: &str| {
        d.shell_mut().set_form_field("system", system);
        d.input(&Input::Hover { at: (960, 540) });
        d.settle();
    };
    let shown = |d: &Driver| d.texts().iter().any(|t| t == names);
    choose(&mut d, "supply_own");
    assert!(shown(&d), "with Own names");
    choose(&mut d, "supply_dc");
    assert!(!shown(&d), "gone with 24 V DC");
    d.shell_mut().shutdown();
}

/// Marks of one group that stand in a row each print where they come
/// from; before, the middle one printed nothing and seemed to come from nowhere. The Potential panel lists 0 V near
/// the top, where a person finds it without scrolling.
#[test]
fn supply_marks_in_a_row_each_print_where_they_come_from() {
    use ew_viewmodel::shell::Outcome;
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    for (name, x) in [("L1", 800), ("N", 1000), ("PE", 1200)] {
        crate::wires_and_cables::fill(&mut d, "Interruption point name", name);
        d.input(&Input::Hover { at: (x, 400) });
        d.settle();
        d.input(&click((x, 400)));
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
    assert_eq!(ips.len(), 3);
    let shell = d.shell_mut();
    assert_eq!(
        shell.dispatch("select.set", &json!({ "ids": ips })),
        Outcome::Done
    );
    assert_eq!(
        shell.dispatch("ip.supply_in", &json!({"location": "Mains", "device": "-X0"})),
        Outcome::Done
    );
    let kinds: Vec<String> = shell.supply_kinds().into_iter().map(|(w, _)| w).collect();
    let m = kinds.iter().position(|w| w == "m").expect("0 V in the list");
    assert!(m <= 9, "0 V near the top: {kinds:?}");
    assert_eq!(shell.dispatch("ui.dismiss", &json!({})), Outcome::Done);
    d.input(&Input::Hover { at: (4, 4) });
    d.settle();
    let printed = crate::own_parts::page_texts(&mut d)
        .iter()
        .filter(|t| t.contains("-X0"))
        .count();
    assert_eq!(printed, 3, "each mark of the row prints where it comes from");
    d.shell_mut().shutdown();
}
