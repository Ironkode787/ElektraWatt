// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The persona round on rc.3 (FT-1658 to FT-1660), by the window's own path: clicks, drags and keys.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::script::state_json;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::visual::{Driver, Input, parse_chord};
use serde_json::{Value, json};

fn click(at: (i64, i64)) -> Input {
    Input::Click {
        at,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    }
}

fn key(chord: &str) -> Input {
    let (key, mods) = parse_chord(chord).unwrap();
    Input::Key { key, mods, repeat: 1 }
}

fn typed(text: &str) -> Input {
    Input::Type {
        text: text.to_owned(),
    }
}

fn state(d: &mut Driver) -> Value {
    serde_json::from_str(&state_json(d.shell_mut())).unwrap()
}

/// The centre of the first widget whose accessible name starts with `prefix`.
fn starting_with(d: &Driver, prefix: &str) -> (i64, i64) {
    d.labels()
        .into_iter()
        .find(|(l, _)| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no widget named {prefix:?}"))
        .1
}

/// A driver over a new project from the built-in template, on its first page.
fn new_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// The tags of the Devices navigator's device rows (its labels start with the tag).
fn devices(d: &mut Driver) -> Vec<String> {
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({"kind": "devices"}));
    d.settle();
    let t = d
        .shell_mut()
        .nav(NavKind::Devices)
        .unwrap()
        .table()
        .unwrap()
        .clone();
    let mut tags: Vec<String> = t
        .nodes
        .iter()
        .filter(|n| n.role == NavRole::Device)
        .map(|n| n.label.clone())
        .collect();
    tags.sort();
    tags
}

/// Places the relay of the device browser's Relays (its coil, the part's main function) at `at`, then Escape.
fn place_relay(d: &mut Driver, at: (i64, i64)) {
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("relay"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(d, "Relay with four"),
        mods: Modifiers::NONE,
    });
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
    d.input(&key("escape"));
    d.settle();
}

/// The window pixel of a sheet point, from two pointer positions the canvas reports.
fn pixel_map(d: &mut Driver) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    let probe = |d: &mut Driver, at: (i64, i64)| {
        d.input(&Input::Hover { at });
        d.settle();
        let p = state(d)["canvas"]["pointer_nm"].clone();
        (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())
    };
    let (f0, f1) = ((700, 400), (1000, 700));
    let (s0, s1) = (probe(d, f0), probe(d, f1));
    move |sheet: (i64, i64)| {
        (
            f0.0 + (sheet.0 - s0.0) * (f1.0 - f0.0) / (s1.0 - s0.0),
            f0.1 + (sheet.1 - s0.1) * (f1.1 - f0.1) / (s1.1 - s0.1),
        )
    }
}

/// The window pixel of a page point.
fn page_pixel(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64), p: (i64, i64)) -> (i64, i64) {
    pixel(d.shell_mut().canvas().unwrap().page().snapper.page_to_sheet(p))
}

/// The page point of the connection point labelled `label` on the page shown.
fn cp_at(d: &mut Driver, label: &str) -> (i64, i64) {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    tables
        .cps
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no connection point {label}"))
        .at
}

/// The persona picked a contact of -102K1 from the device browser's "In this project" and got a second relay. The
/// part's contact row names the relay it fills, and placing it fills that relay's free contact: one device.
#[test]
fn ft1658_a_contact_picked_in_the_browser_fills_the_relay_of_the_project() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_relay(&mut d, (900, 450));
    assert_eq!(devices(&mut d), ["-1K1"]);
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&click(starting_with(&d, "In this project")));
    d.settle();
    // The part's row, opened to its functions by the keys: Tab to the tree, Tab to the table, Right opens it.
    d.input(&click(starting_with(&d, "Relay with four")));
    d.input(&key("tab"));
    d.input(&key("tab"));
    d.input(&key("arrowright"));
    d.settle();
    let row = d
        .labels()
        .into_iter()
        .find(|(l, _)| l.starts_with("2 of 5"))
        .expect("the part's second function");
    assert!(
        row.0.contains("join -1K1"),
        "the row names the relay it fills: {}",
        row.0
    );
    d.input(&Input::DoubleClick {
        at: row.1,
        mods: Modifiers::NONE,
    });
    d.input(&Input::Hover { at: (1100, 450) });
    d.settle();
    d.input(&click((1100, 450)));
    d.settle();
    d.input(&key("escape"));
    d.settle();
    assert_eq!(devices(&mut d), ["-1K1"], "no second relay");
    d.shell_mut().shutdown();
}

/// Join on a placed contact was refused as "not a contact": J (and the page's right-click menu) on one placed
/// symbol opens the Join chooser, and its row joins the contact to the relay.
#[test]
fn ft1658_j_on_a_placed_contact_joins_it_to_the_relay() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_relay(&mut d, (900, 450));
    // A plain changeover contact: a device of its own.
    d.input(&click(d.find("Symbol").unwrap()));
    d.settle();
    d.input(&typed("changeover"));
    d.settle();
    d.input(&key("enter"));
    let at = (1100, 450);
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
    d.input(&key("escape"));
    d.settle();
    assert_eq!(devices(&mut d), ["-1K1", "-1K2"]);
    // The right-click menu on it, selected, offers Join device….
    d.input(&click(at));
    d.settle();
    d.input(&Input::Click {
        at,
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    });
    d.settle();
    d.input(&click(starting_with(&d, "Join device…")));
    d.settle();
    assert!(
        d.shell_mut().chooser().is_some(),
        "the menu's entry opens the Join chooser"
    );
    d.input(&key("escape"));
    d.settle();
    // J opens it too; Enter joins the relay, its first row.
    d.input(&click(at));
    d.settle();
    d.input(&key("j"));
    d.settle();
    let rows = d.shell_mut().chooser().expect("the Join chooser").rows.clone();
    assert_eq!(rows[0].name, "-1K1");
    d.input(&key("enter"));
    d.settle();
    assert_eq!(devices(&mut d), ["-1K1"], "the contact is the relay's");
    d.shell_mut().shutdown();
}

/// Deleting the one placed symbol of a relay left "-?K1" with its four unplaced contacts: the relay goes with it.
#[test]
fn ft1658_deleting_a_relays_one_symbol_leaves_no_empty_relay() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_relay(&mut d, (900, 450));
    d.input(&click((900, 450)));
    d.settle();
    d.input(&key("delete"));
    d.settle();
    assert_eq!(devices(&mut d), Vec::<String>::new());
    d.shell_mut().shutdown();
}

/// The Fix badge of the duplicate-designation error did nothing, and its fix renumbered nothing when the keeper's
/// tag was frozen: the badge opens the fix, and Apply gives the other relay a free tag.
#[test]
fn ft1658_the_fix_badge_of_a_duplicate_tag_renumbers_the_other_device() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_relay(&mut d, (900, 450));
    place_relay(&mut d, (1100, 450));
    // The second relay renamed to the first one's tag in Properties.
    d.input(&click((1100, 450)));
    d.settle();
    d.input(&click(d.find("-1K2").unwrap()));
    d.settle();
    d.input(&key("ctrl+a"));
    d.input(&typed("-1K1"));
    d.input(&key("enter"));
    d.settle();
    assert_eq!(devices(&mut d), ["-1K1", "-1K1"]);
    d.input(&Input::Wait { ms: 300 });
    d.input(&click(starting_with(&d, "1 Error")));
    d.settle();
    d.input(&click(d.find("Fix").expect("the finding's Fix badge")));
    d.settle();
    assert!(
        d.shell_mut().properties().card().is_some_and(|c| c.changes > 0),
        "the fix's card shows what it changes"
    );
    d.input(&click(d.find("Apply").unwrap()));
    d.settle();
    assert_eq!(devices(&mut d), ["-1K1", "-1K2"]);
    d.shell_mut().shutdown();
}

/// A driver over a copy of Golden A on folio 101, the page fitted.
fn golden_101(tmp: &Tmp) -> Driver {
    let golden = tmp.golden("g.ew");
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let opened = d
        .shell_mut()
        .dispatch("file.open", &json!({"dir": golden.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    d.settle();
    d
}

/// The representation drawing the connection point `label`: its ID text and the centre of its body (page nm).
fn rep_of(d: &mut Driver, label: &str) -> (String, (i64, i64)) {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let cp = tables.cps.iter().find(|c| c.label == label).unwrap();
    let r = tables.reps.iter().find(|r| r.id == cp.rep).unwrap();
    let b = r.body;
    (r.id.to_string(), ((b[0] + b[2]) / 2, (b[1] + b[3]) / 2))
}

/// A drag from the lamp's centre just after the click that selected it did nothing: the drag's press was taken as
/// the double click's second press. It moves the lamp now.
#[test]
fn ft1659_a_drag_from_a_symbols_centre_right_after_selecting_it_moves_it() {
    let tmp = Tmp::new();
    let mut d = golden_101(&tmp);
    let pixel = pixel_map(&mut d);
    let (id, centre) = rep_of(&mut d, "E56:X1");
    let at = page_pixel(&mut d, &pixel, centre);
    let step = page_pixel(&mut d, &pixel, (centre.0 + 8_000_000, centre.1)).0 - at.0;
    d.input(&click(at));
    d.settle();
    assert_eq!(state(&mut d)["canvas"]["selection"], json!([id]));
    d.input(&Input::Drag {
        from: at,
        to: (at.0 + step, at.1),
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    d.settle();
    assert_ne!(rep_of(&mut d, "E56:X1").1, centre, "the lamp moved");
    d.shell_mut().shutdown();
}

/// A paste landed below and right of the click (the fragment hung from its top-left corner): its middle is at the
/// click now, within a grid step.
#[test]
fn ft1659_a_paste_lands_with_its_middle_at_the_click() {
    let tmp = Tmp::new();
    let mut d = golden_101(&tmp);
    let pixel = pixel_map(&mut d);
    let (_, centre) = rep_of(&mut d, "E56:X1");
    let at = page_pixel(&mut d, &pixel, centre);
    d.input(&click(at));
    d.input(&key("ctrl+c"));
    d.input(&key("ctrl+v"));
    let target = (at.0 - 120, at.1 - 120);
    d.input(&Input::Hover { at: target });
    d.settle();
    let p = state(&mut d)["canvas"]["pointer_nm"].clone();
    let snapper = d.shell_mut().canvas().unwrap().page().snapper.clone();
    let clicked = snapper.sheet_to_page((p[0].as_i64().unwrap(), p[1].as_i64().unwrap()));
    let before: Vec<_> = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id)
        .collect();
    d.input(&click(target));
    d.settle();
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let pasted = tables
        .reps
        .iter()
        .find(|r| !before.contains(&r.id))
        .expect("the pasted lamp");
    let b = pasted.body;
    let middle = ((b[0] + b[2]) / 2, (b[1] + b[3]) / 2);
    let step = snapper.page_of_index(1) - snapper.page_of_index(0);
    assert!(
        (middle.0 - clicked.0).abs() <= step && (middle.1 - clicked.1).abs() <= step,
        "{middle:?} at the click {clicked:?}"
    );
    d.shell_mut().shutdown();
}

/// The findings chip of two selected lamps opened Messages filtered on "2 devices", which matches nothing: with
/// several records selected it opens Messages without a filter.
#[test]
fn ft1659_the_findings_chip_of_two_symbols_opens_messages_unfiltered() {
    let tmp = Tmp::new();
    let mut d = golden_101(&tmp);
    let pixel = pixel_map(&mut d);
    let (_, e55) = rep_of(&mut d, "E55:L");
    let (_, e56) = rep_of(&mut d, "E56:X1");
    let (a, b) = (page_pixel(&mut d, &pixel, e55), page_pixel(&mut d, &pixel, e56));
    d.input(&click(a));
    d.settle();
    d.input(&Input::Click {
        at: b,
        button: PointerButton::Primary,
        mods: Modifiers::SHIFT,
    });
    d.settle();
    assert_eq!(
        state(&mut d)["canvas"]["selection"].as_array().map(Vec::len),
        Some(2)
    );
    let chip = d
        .labels()
        .into_iter()
        .find(|(l, _)| l.contains("Warning"))
        .expect("the findings chip")
        .1;
    d.input(&click(chip));
    d.settle();
    let filter = d.shell_mut().nav(NavKind::Messages).map(|n| n.filter.clone());
    assert_eq!(filter.as_deref(), Some(""));
    d.shell_mut().shutdown();
}

/// After a wire ended on a wire, the pointer rests on the junction it made, and the bar read "This junction already
/// joins three wires" as if the wire were refused. The hint is the Wire tool's own until the next wire starts.
#[test]
fn ft1660_a_wire_ended_on_a_wire_leaves_no_refusal_in_the_bar() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    for at in [(800, 400), (1000, 400), (900, 550)] {
        place_relay(&mut d, at);
    }
    let pixel = pixel_map(&mut d);
    let (a, b, c) = (
        cp_at(&mut d, "-1K1:A2"),
        cp_at(&mut d, "-1K2:A2"),
        cp_at(&mut d, "-1K3:A1"),
    );
    d.input(&click(d.find("Wire").unwrap()));
    d.settle();
    for p in [a, b] {
        let at = page_pixel(&mut d, &pixel, p);
        d.input(&Input::Hover { at });
        d.input(&click(at));
        d.settle();
    }
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let wire = tables
        .connections
        .last()
        .expect("the first wire")
        .polyline
        .clone();
    // The second wire from -1K3:A1 up onto the middle of the first wire's longest segment.
    let (p, q) = wire
        .windows(2)
        .map(|s| (s[0], s[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .expect("a segment");
    let on = ((p.0 + q.0) / 2, (p.1 + q.1) / 2);
    for at in [page_pixel(&mut d, &pixel, c), page_pixel(&mut d, &pixel, on)] {
        d.input(&Input::Hover { at });
        d.input(&click(at));
        d.settle();
    }
    let joined = d.shell_mut().canvas().unwrap().page().tables.junctions.len();
    assert!(joined > 0, "the wire ended on a wire at a junction");
    assert!(
        !d.shell_mut().tool_hint().contains("three wires"),
        "{}",
        d.shell_mut().tool_hint()
    );
    d.shell_mut().shutdown();
}

/// A click on an outgoing interruption point's arrow snapped to the grid beside it (its connection point is a grid
/// step behind the tip), so no wire reached it: the arrow ends the wire on its connection point.
#[test]
fn ft1660_a_click_on_an_interruption_points_arrow_ends_the_wire_on_it() {
    wire_to_the_arrow(0);
}

/// A click two pixels beyond the arrow's tip, in line with a wire coming to it, placed a bend and the wire went on
/// (the persona round before 0.2.0): the arrow counts with the snap's reach around it, as a connection point does.
#[test]
fn a_click_just_beyond_the_arrows_tip_ends_the_wire_on_it_too() {
    wire_to_the_arrow(2);
}

/// A wire from a coil to an outgoing interruption point, ended by a click `beyond` pixels right of the arrow's tip:
/// it ends on the point's connection point.
fn wire_to_the_arrow(beyond: i64) {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    place_relay(&mut d, (930, 620));
    // An outgoing point named L1, placed by the pointer.
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    d.input(&click(d.find("Interruption point name").unwrap()));
    d.input(&key("ctrl+a"));
    d.input(&typed("L1"));
    d.input(&key("enter"));
    d.input(&Input::Hover { at: (930, 520) });
    d.settle();
    d.input(&click((930, 520)));
    d.settle();
    d.input(&key("escape"));
    d.settle();
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let ip = tables
        .reps
        .iter()
        .find(|r| r.ip.is_some())
        .expect("the point")
        .clone();
    // Zoomed in, as the persona worked: the connection point is then beyond the snap's reach from the tip.
    for _ in 0..3 {
        d.input(&click(d.find("Zoom in").unwrap()));
        d.settle();
    }
    let pixel = pixel_map(&mut d);
    let coil = cp_at(&mut d, "-1K1:A1");
    d.input(&click(d.find("Wire").unwrap()));
    d.settle();
    // From the coil to the arrow's tip, its insertion point.
    for (p, dx) in [(coil, 0), (ip.pos, beyond)] {
        let at = page_pixel(&mut d, &pixel, p);
        let at = (at.0 + dx, at.1);
        d.input(&Input::Hover { at });
        d.input(&click(at));
        d.settle();
    }
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let cp = tables
        .cps
        .iter()
        .find(|c| c.rep == ip.id)
        .expect("the point's connection point");
    assert_eq!(cp.connections.len(), 1, "the wire ends on the point");
    d.shell_mut().shutdown();
}
