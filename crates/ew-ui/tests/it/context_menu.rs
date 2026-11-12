// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1693 and FT-1707: the canvas's right-click menu is the quick way to what fits the thing under the pointer, each
//! test by the pointer on a copy of Golden A: a terminal adds a terminal and opens its strip's editor; a wire offers
//! its route, marker, potential, cable and wire numbers, two wires a cable; a coil its free contacts and a contact its
//! coil; an interruption point its partner; the title block its fields; Shift+F10 opens the menu at the selection.

use crate::support;

use egui::{Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::canvas::{pick, rect_centre};
use ew_ui::ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_ui::ew_viewmodel::ew_api::output::TableRep;
use ew_ui::ew_viewmodel::navigators::NavKind;
use ew_ui::visual::{Driver, Input};
use serde_json::json;
use support::{Tmp, dispatch, on_golden, screen, settle, wait_for_page};

use crate::wires_and_cables::{
    click, key, new_project, page_pixel, pixel_map, place_relay, starting_with, tables,
};

/// A window point on `id` (a symbol's or a wire's ID) where the canvas picks it and no link is: a third of the way
/// along a wire's segment (a selected wire's handles stand at the middles), else its rectangle's middle.
fn point_on(h: &Harness<'_, App>, id: &str) -> Pos2 {
    let c = h.state().shell().canvas().unwrap();
    let page = std::sync::Arc::clone(c.page());
    let segments = page
        .tables
        .connections
        .iter()
        .filter(|w| w.id.to_string() == id)
        .flat_map(|w| {
            w.polyline
                .windows(2)
                .map(|s| {
                    page.snapper
                        .page_to_sheet(((2 * s[0].0 + s[1].0) / 3, (2 * s[0].1 + s[1].1) / 3))
                })
                .collect::<Vec<_>>()
        });
    let centres = page
        .entities
        .iter()
        .filter(|(e, _)| e.0.to_string() == id)
        .map(|(_, r)| rect_centre(*r));
    let at = segments
        .chain(centres)
        .find(|p| {
            c.link_at(*p).is_none()
                && pick(&page.entities, Some(*p), 0.0).is_some_and(|(e, _)| e.0.to_string() == id)
        })
        .expect("a point that picks it");
    screen(h, at)
}

/// A right click at `at`.
fn right_click(h: &mut Harness<'static, App>, at: Pos2) {
    crate::tools::right_click(h, at);
}

/// The first symbol of the page on show whose menu (as the window opens it over it) holds an entry for which `want`
/// holds; its ID.
fn rep_whose_menu(h: &mut Harness<'static, App>, want: &dyn Fn(&str, &str) -> bool) -> Option<String> {
    let ids: Vec<String> = h
        .state()
        .shell()
        .canvas()?
        .page()
        .tables
        .reps
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    let found = ids.into_iter().find(|id| {
        dispatch(h, "select.set", json!({ "ids": [id] }));
        canvas_menu(h.state().shell(), MenuSpot::Object)
            .iter()
            .any(|i| i.enabled && want(i.id, &i.label))
    });
    dispatch(h, "select.set", json!({ "ids": [] }));
    found
}

/// The first page of Golden A in page order with a symbol whose menu holds an entry `want` takes, left on show; the
/// symbol's ID.
fn find_rep(h: &mut Harness<'static, App>, want: &dyn Fn(&str, &str) -> bool) -> String {
    let folios: Vec<String> = h.state().shell().page_labels().to_vec();
    for folio in folios {
        dispatch(h, "page.goto", json!({ "folio": folio }));
        settle(h);
        wait_for_page(h);
        if let Some(id) = rep_whose_menu(h, want) {
            settle(h);
            return id;
        }
    }
    panic!("no symbol of Golden A has such an entry");
}

/// The entry of the menu opened at `at` whose label holds `label`: of the widgets so named, the one whose left edge
/// is nearest the point (the menu opens there; Properties may name the same command).
fn entry<'h>(h: &'h Harness<'_, App>, at: Pos2, label: &'h str) -> egui_kittest::Node<'h> {
    h.query_all_by_label_contains(label)
        .min_by(|a, b| {
            let d =
                |n: &egui_kittest::Node<'_>| (n.rect().left() - at.x).abs() + (n.rect().top() - at.y).abs();
            d(a).total_cmp(&d(b))
        })
        .unwrap_or_else(|| panic!("{label} in the menu"))
}

/// The tops of the menu entries opened at `at` named `labels`, in the order given.
fn listed(h: &Harness<'_, App>, at: Pos2, labels: &[&str]) -> Vec<f32> {
    labels.iter().map(|l| entry(h, at, l).rect().top()).collect()
}

/// FT-1707: a right click on a terminal offers Add terminal and Edit strip above Cut; Edit strip opens the Terminal
/// strips navigator on its strip, and Add terminal floats the strip's next terminal at the pointer.
#[test]
fn a_terminals_menu_adds_a_terminal_and_edits_its_strip() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let terminal = find_rep(&mut h, &|id, _| id == "tool.place_terminal");
    let at = point_on(&h, &terminal);
    right_click(&mut h, at);
    let tops = listed(&h, at, &["Add terminal", "Edit strip", "Cut"]);
    assert!(tops[0] < tops[2] && tops[1] < tops[2], "{tops:?}");
    entry(&h, at, "Edit strip").click();
    settle(&mut h);
    assert_eq!(h.state().shell().focused_navigator(), Some(NavKind::Strips));
    let at = point_on(&h, &terminal);
    right_click(&mut h, at);
    entry(&h, at, "Add terminal").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "place", "a terminal floats");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1693: a right click on a wire offers Edit route, Wire marker, Define potential…, Cable… and Print wire numbers
/// (which prints them); two wires selected offer Make cable from the selection…, which opens the cable's form.
#[test]
fn a_wires_menu_and_two_wires_make_a_cable() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "102");
    let wires: Vec<String> = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .filter(|w| w.kind == "wire")
        .map(|w| w.id.to_string())
        .take(2)
        .collect();
    let first = point_on(&h, &wires[0]);
    right_click(&mut h, first);
    let tops = listed(
        &h,
        first,
        &[
            "Edit route",
            "Wire marker",
            "Define potential",
            "Cable…",
            "Print wire numbers",
            "Cut",
        ],
    );
    assert!(tops[..5].iter().all(|t| *t < tops[5]), "{tops:?}");
    assert!(!h.state().shell().wire_numbers_on());
    entry(&h, first, "Print wire numbers").click();
    settle(&mut h);
    assert!(h.state().shell().wire_numbers_on(), "the numbers print");
    // A click on the first wire, a Shift+click on the second: both selected, and a right click on the first keeps them.
    for (i, modifiers) in [(0, Modifiers::NONE), (1, Modifiers::SHIFT)] {
        let at = point_on(&h, &wires[i]);
        h.event(egui::Event::PointerMoved(at));
        h.run_ok();
        h.event(egui::Event::ModifiersChanged(modifiers));
        h.run_ok();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers,
            });
            h.run_ok();
        }
        h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
        h.run_ok();
        settle(&mut h);
    }
    assert_eq!(h.state().shell().selection().items.len(), 2);
    let first = point_on(&h, &wires[0]);
    right_click(&mut h, first);
    assert_eq!(h.state().shell().selection().items.len(), 2);
    entry(&h, first, "Make cable from the selection").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_some(), "the cable's form");
    h.state_mut().shell_mut().shutdown();
}

/// The window pixel of the middle of `rep`'s body.
fn body_pixel(d: &mut Driver, pixel: &impl Fn((i64, i64)) -> (i64, i64), rep: &TableRep) -> (i64, i64) {
    let [x0, y0, x1, y1] = rep.body;
    page_pixel(d, pixel, ((x0 + x1) / 2, (y0 + y1) / 2))
}

/// FT-1693: on a new project with a relay placed from the device browser, a right click on its coil offers its free
/// contacts by name; Add contact floats the contact, a click places it; a right click on the contact goes to the coil.
#[test]
fn a_coils_menu_adds_a_contact_and_a_contacts_menu_goes_to_the_coil() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let pixel = pixel_map(&mut d);
    place_relay(&mut d, (800, 500));
    let coil = tables(&mut d).reps[0].clone();
    let at = body_pixel(&mut d, &pixel, &coil);
    d.input(&Input::Click {
        at,
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    });
    d.settle();
    let add = starting_with(&d, "Add contact ");
    d.input(&click(add));
    d.settle();
    assert_eq!(d.shell_mut().tool().name(), "place", "the contact floats");
    let to = (1100, 500);
    d.input(&Input::Hover { at: to });
    d.settle();
    d.input(&click(to));
    d.settle();
    d.input(&key("escape"));
    d.settle();
    let contact = tables(&mut d)
        .reps
        .iter()
        .find(|r| r.id != coil.id && r.device == coil.device)
        .cloned()
        .expect("the contact is placed on the coil's relay");
    let at = body_pixel(&mut d, &pixel, &contact);
    d.input(&Input::Click {
        at,
        button: PointerButton::Secondary,
        mods: Modifiers::NONE,
    });
    d.settle();
    let go = starting_with(&d, "Go to coil");
    d.input(&click(go));
    d.settle();
    let cursor = d
        .shell_mut()
        .canvas()
        .unwrap()
        .cursor()
        .expect("the cursor at the coil");
    let page = std::sync::Arc::clone(d.shell_mut().canvas().unwrap().page());
    let [x0, y0, x1, y1] = coil.body;
    let (a, b) = (
        page.snapper.page_to_sheet((x0, y0)),
        page.snapper.page_to_sheet((x1, y1)),
    );
    let reach = 10_000_000;
    assert!(
        (a.0.min(b.0) - reach..=a.0.max(b.0) + reach).contains(&cursor.0)
            && (a.1.min(b.1) - reach..=a.1.max(b.1) + reach).contains(&cursor.1),
        "{cursor:?} at the coil {a:?} {b:?}"
    );
    d.shell_mut().shutdown();
}

/// FT-1693: an interruption point's menu goes to its partner and offers its direction and Continue on page….
#[test]
fn an_interruption_points_menu_goes_to_its_partner() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    let ip = find_rep(&mut h, &|_, label| label.starts_with("Go to partner "));
    let folio = h.state().shell().current_folio().map(str::to_owned);
    let view = h.state().shell().canvas().unwrap().viewport();
    let at = point_on(&h, &ip);
    right_click(&mut h, at);
    for label in ["Direction: Source", "Direction: Target", "Continue on page"] {
        entry(&h, at, label);
    }
    entry(&h, at, "Go to partner").click();
    settle(&mut h);
    let moved = h.state().shell().current_folio().map(str::to_owned) != folio
        || h.state().shell().canvas().unwrap().viewport() != view;
    assert!(moved, "the partner is shown");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1693: a right click on the title block offers Title block fields… first, which opens its form, then what an
/// empty spot offers (Paste, Page properties, New page…); Shift+F10 opens the menu of the selection at it.
#[test]
fn the_title_blocks_menu_edits_its_fields_and_shift_f10_opens_the_menu() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "103");
    let band = {
        let c = h.state().shell().canvas().unwrap();
        let page = std::sync::Arc::clone(c.page());
        let [x0, y0, x1, y1] = *page
            .geometry
            .frame_bands
            .iter()
            .max_by_key(|[x0, y0, x1, y1]| (x1 - x0) * (y1 - y0))
            .expect("a frame band");
        // A spot of the band where the canvas picks nothing.
        (1..8)
            .flat_map(|i| (1..8).map(move |j| (x0 + (x1 - x0) * i / 8, y0 + (y1 - y0) * j / 8)))
            .map(|p| page.snapper.page_to_sheet(p))
            .find(|p| pick(&page.entities, Some(*p), 0.0).is_none() && c.link_at(*p).is_none())
            .expect("an empty spot of the title block")
    };
    let at = screen(&h, band);
    right_click(&mut h, at);
    let tops = listed(
        &h,
        at,
        &["Title block fields", "Paste", "Page properties", "New page"],
    );
    assert!(tops.windows(2).all(|w| w[0] < w[1]), "{tops:?}");
    entry(&h, at, "Title block fields").click();
    settle(&mut h);
    assert!(h.state().shell().dialog().is_some(), "the title block's form");
    h.key_press(egui::Key::Escape);
    settle(&mut h);
    // Shift+F10 with a symbol selected and the canvas holding the keys.
    let rep = h.state().shell().canvas().unwrap().page().tables.reps[0]
        .id
        .to_string();
    dispatch(&mut h, "select.set", json!({ "ids": [rep] }));
    dispatch(&mut h, "ui.focus", json!({"area": "canvas", "group": 0}));
    settle(&mut h);
    assert!(h.query_by_label_contains("Properties of selection").is_none());
    // Shift held through the frame, as a real keyboard holds it.
    h.event(egui::Event::ModifiersChanged(Modifiers::SHIFT));
    h.run_ok();
    for pressed in [true, false] {
        h.event(egui::Event::Key {
            key: egui::Key::F10,
            pressed,
            modifiers: Modifiers::SHIFT,
            repeat: false,
            physical_key: None,
        });
    }
    h.run_ok();
    h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    h.run_ok();
    settle(&mut h);
    assert!(
        h.query_by_label_contains("Properties of selection").is_some(),
        "the menu at the selection"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-1707: a right click on a strip's row in the Terminal strips navigator offers Add terminal and Edit strip, as the
/// strip's terminals do on the page; Edit strip opens the strip editor on it.
#[test]
fn a_strip_rows_menu_edits_the_strip() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "101");
    dispatch(&mut h, "navigator.show", json!({ "kind": "strips" }));
    settle(&mut h);
    let name = h
        .state()
        .shell()
        .shown_rows(NavKind::Strips)
        .into_iter()
        .find(|r| r.role == "strip")
        .map(|r| r.name)
        .expect("a strip row");
    h.get_by_label(&name).click_secondary();
    settle(&mut h);
    assert!(h.query_by_label_contains("Add terminal").is_some());
    h.get_by_label_contains("Edit strip").click();
    settle(&mut h);
    let shell = h.state().shell();
    assert_eq!(shell.focused_navigator(), Some(NavKind::Strips));
    assert!(
        shell.nav(NavKind::Strips).and_then(|n| n.table_mode()).is_some(),
        "the strip editor"
    );
    h.state_mut().shell_mut().shutdown();
}
