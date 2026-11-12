// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The owner's test of 0.2.0-rc.1, his rows about wires (FT-1684, FT-1685, FT-1688, FT-1692), by the window's own
//! path: clicks, drags and keys.

use crate::support::Tmp;

use egui::{Modifiers, PointerButton};
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

/// A driver over a new project from the built-in template, on its first page.
fn new_project(tmp: &Tmp) -> Driver {
    let mut d = Driver::in_folder(tmp.config(), None, tmp.0.clone()).unwrap();
    d.enable_accesskit();
    let project = json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
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

/// The page tables of the page shown.
fn tables(d: &mut Driver) -> std::sync::Arc<ew_ui::ew_viewmodel::ew_api::output::PageTables> {
    std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables)
}

/// Clicks the widget named `label`, selects its text, types `text` and presses Enter.
fn fill(d: &mut Driver, label: &str, text: &str) {
    d.input(&click(d.find(label).unwrap_or_else(|| panic!("no widget {label:?}"))));
    d.input(&key("ctrl+a"));
    d.input(&typed(text));
    d.input(&key("enter"));
    d.settle();
}

/// The centre of the first widget whose accessible name starts with `prefix`.
fn starting_with(d: &Driver, prefix: &str) -> (i64, i64) {
    d.labels()
        .into_iter()
        .find(|(l, _)| l.starts_with(prefix))
        .unwrap_or_else(|| panic!("no widget named {prefix:?}"))
        .1
}

/// Places the relay of the device browser's Relays at `at`, then Escape.
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

/// Two source interruption points, L1 and N, side by side, each with a wire down to the coil of a relay under it:
/// two wires, as the owner drew them. Returns the pixel map.
fn two_wires(d: &mut Driver) -> impl Fn((i64, i64)) -> (i64, i64) + use<> {
    for x in [900, 1000] {
        place_relay(d, (x, 620));
    }
    d.input(&click(d.find("Interruption").unwrap()));
    d.settle();
    for (name, x) in [("L1", 900), ("N", 1000)] {
        fill(d, "Interruption point name", name);
        d.input(&Input::Hover { at: (x, 420) });
        d.settle();
        d.input(&click((x, 420)));
        d.settle();
    }
    d.input(&key("escape"));
    d.settle();
    let pixel = pixel_map(d);
    let t = tables(d);
    let mut ips: Vec<(i64, i64)> = t
        .reps
        .iter()
        .filter(|r| r.ip.is_some())
        .filter_map(|r| t.cps.iter().find(|c| c.rep == r.id).map(|c| c.at))
        .collect();
    ips.sort();
    let coil = |label: &str| t.cps.iter().find(|c| c.label == label).expect("the coil").at;
    let ends = [(ips[0], coil("-1K1:A1")), (ips[1], coil("-1K2:A1"))];
    d.input(&click(d.find("Wire").unwrap()));
    d.settle();
    for (a, b) in ends {
        for p in [a, b] {
            let at = page_pixel(d, &pixel, p);
            d.input(&Input::Hover { at });
            d.input(&click(at));
            d.settle();
        }
    }
    d.input(&key("escape"));
    d.settle();
    assert_eq!(tables(d).connections.len(), 2, "two wires");
    pixel
}

/// The banner's text (where a refused step says why).
fn banner(d: &mut Driver) -> String {
    state(d)["banner"].as_str().unwrap_or_default().to_owned()
}

/// Drags the Marker tool across every wire of the page, halfway down; returns the end of the drag.
fn mark_across(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64)) -> (i64, i64) {
    let t = tables(d);
    let ys: Vec<i64> = t.connections.iter().flat_map(|c| c.polyline.iter().map(|p| p.1)).collect();
    let mid_y = (ys.iter().min().unwrap() + ys.iter().max().unwrap()) / 2;
    let xs: Vec<i64> = t.connections.iter().flat_map(|c| c.polyline.iter().map(|p| p.0)).collect();
    let (x0, x1) = (xs.iter().min().unwrap() - 4_000_000, xs.iter().max().unwrap() + 4_000_000);
    d.input(&click(d.find("Marker").unwrap()));
    d.settle();
    let from = page_pixel(d, pixel, (x0, mid_y));
    let to = page_pixel(d, pixel, (x1, mid_y));
    d.input(&Input::Drag {
        from,
        to,
        button: PointerButton::Primary,
        mods: Modifiers::NONE,
    });
    d.settle();
    assert_eq!(d.shell_mut().tool().state(), "typing");
    to
}

/// FT-1684: two wires, the Marker tool dragged across both, cross-section 1.5 and colour BK typed in its fields,
/// the pointer back on the page, Enter: a marker stands over both wires and says 1.5 and BK for each. (Enter placed
/// nothing after a drag, and one colour for two wires was refused.) A colour the project does not know is refused
/// in words that name the codes it knows, and the lower-case code is taken.
#[test]
fn ft1684_the_marker_tool_marks_two_wires_with_a_typed_colour() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    let to = mark_across(&mut d, &pixel);
    fill(&mut d, "Cross-section", "1.5");
    fill(&mut d, "Colours", "black-ish");
    let said = banner(&mut d);
    assert!(
        said.contains("\"black-ish\" is not a colour") && said.contains("BK, BN") && !said.contains('/'),
        "{said:?}"
    );
    fill(&mut d, "Colours", "bk");
    d.input(&Input::Hover { at: to });
    d.input(&key("enter"));
    d.settle();
    let t = tables(&mut d);
    let marker = t.items.iter().find(|i| i.kind == "wire_marker");
    assert_eq!(
        marker.and_then(|m| m.text.as_deref()),
        Some("1.5 BK BK"),
        "the window said {:?}",
        banner(&mut d)
    );
    d.shell_mut().shutdown();
}

/// The window pixel of the middle of the longest segment of the `i`-th wire of the page.
fn on_wire(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64), i: usize) -> (i64, i64) {
    let t = tables(d);
    let (p, q) = t.connections[i]
        .polyline
        .windows(2)
        .map(|s| (s[0], s[1]))
        .max_by_key(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
        .expect("a segment");
    page_pixel(d, pixel, ((p.0 + q.0) / 2, (p.1 + q.1) / 2))
}

/// FT-1688: a wire whose potential is given the type L1, and nothing else, takes a phase's colour and cross-section
/// (1.5 mm², the mains class), not the control circuit's 0.75 mm²: its marker says so.
#[test]
fn ft1688_a_wire_of_an_l1_potential_takes_the_phases_cross_section() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = two_wires(&mut d);
    // The first wire, clicked, then Draw ▸ Define potential…: its form, the name and the type L1.
    let at = on_wire(&mut d, &pixel, 0);
    d.input(&Input::Hover { at });
    d.input(&click(at));
    d.settle();
    let shell = d.shell_mut();
    assert_eq!(shell.dispatch("potential.draw", &json!({})), Outcome::Done);
    shell.set_palette_field(0, "L1");
    shell.set_palette_field(1, "L1");
    assert_eq!(shell.run_palette_selection(), Outcome::Done);
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "potential.define");
    d.settle();
    d.input(&key("escape"));
    d.settle();
    let to = mark_across(&mut d, &pixel);
    d.input(&Input::Hover { at: to });
    d.input(&key("enter"));
    d.settle();
    let t = tables(&mut d);
    let marker = t.items.iter().find(|i| i.kind == "wire_marker");
    let text = marker.and_then(|m| m.text.clone()).unwrap_or_default();
    // The L1 wire 1.5 mm² BK, the other wire (no type) the control circuit's 0.75 mm².
    assert!(text.starts_with("1.5/0.75 BK"), "{text:?}");
    d.shell_mut().shutdown();
}


