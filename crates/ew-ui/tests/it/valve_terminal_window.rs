// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A valve terminal in the window, by its own path at 1920 × 1080: placed with one click, drawn as its positions,
//! its multi-pin plug, a terminal made as a station or first on a bus page, the station view's cells, its menu, its
//! solenoids and blanking plates.

#[allow(clippy::wildcard_imports)]
use crate::pneumatics_from_nothing::*;
use crate::relays_paste_and_ip_wires::{click, key, new_project, starting_with, typed};
use crate::support::Tmp;
use egui::Modifiers;
use ew_ui::visual::{Driver, Input};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::Outcome;
use serde_json::json;

/// The terminal tool took a first and a second click, each made a terminal, the two
/// tags overprinted and nothing was said. Armed, the hint says a click places it; one click places one terminal, the
/// tool ends and the toast says what was placed; a terminal clicked onto another is refused in words.
#[test]
fn one_click_places_one_terminal_and_says_so() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("valve terminal"));
    d.settle();
    let terminal = starting_with(&d, "Valve terminal with PROFIBUS DP bus node, 4");
    d.input(&Input::DoubleClick {
        at: terminal,
        mods: Modifiers::NONE,
    });
    d.settle();
    d.input(&Input::Hover { at: (760, 300) });
    d.settle();
    let hint = crate::relays_paste_and_ip_wires::state(&mut d)["status"][0].clone();
    assert_eq!(hint, json!("Click to place; Esc ends"));
    click_at(&mut d, (760, 300));
    assert_eq!(d.shell_mut().tool().name(), "select", "one click, one terminal");
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        toast.starts_with("Placed -") && toast.ends_with(": 4 positions, all empty."),
        "{toast}"
    );
    d.input(&key("escape"));
    d.settle();
    // A second terminal clicked onto the first is refused in words.
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("valve terminal"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Valve terminal with PROFIBUS DP bus node, 4"),
        mods: Modifiers::NONE,
    });
    d.settle();
    click_at(&mut d, (780, 320));
    let status = crate::relays_paste_and_ip_wires::state(&mut d)["status"][0].clone();
    assert_eq!(status, json!("Another box stands here: click on a free place"));
    assert_eq!(cells(&mut d).len(), 4, "one terminal only");
    d.shell_mut().shutdown();
}

/// F15 of a test: the valve terminal took up a box of two clicks, its empty and blank positions were not drawn,
/// and a coil placed in the box was taken as a valve. Placed with one click it is drawn as its four positions in a row;
/// a click on an empty cell opens the device browser narrowed to valves for a terminal, and the valve chosen fills that
/// cell; the cell's menu has Replace, Empty and Blanking plate; a stand-alone valve placed in a cell stays a valve of its
/// own and the toast says so.
#[test]
fn a_terminal_is_drawn_as_its_positions_and_filled_where_they_are() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_terminal(&mut d, (760, 300));
    let c = cells(&mut d);
    assert_eq!(c.len(), 4, "the starter terminal's positions");
    assert!(c.windows(2).all(|w| w[0].rect[1] == w[1].rect[1]), "in a row");
    // A click on the empty cell 2: the device browser, narrowed to valves for a terminal.
    let _ = d.shell_mut().dispatch("tool.select", &json!({}));
    let at = cell_px(&mut d, 1);
    click_at(&mut d, at);
    let picker = d.shell_mut().picker().cloned().expect("the device browser");
    assert!(picker.cell.is_some(), "it fills the cell");
    assert!(
        picker.rows.iter().all(|r| match r {
            ew_viewmodel::tools::PickerRow::Part(p) => picker.catalogue.parts[*p].terminal_valve,
            _ => true,
        }),
        "valves for a terminal only"
    );
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Solenoid valve 5/2 for a valve terminal"),
        mods: Modifiers::NONE,
    });
    d.settle();
    let c = cells(&mut d);
    assert!(c[1].holder.is_some() && !c[1].blank, "cell 2 holds the valve");
    assert!(c[0].holder.is_none(), "cell 1 stays empty");
    // Its menu: Replace, Empty, Blanking plate; chosen, the plate is drawn as one.
    let at = cell_px(&mut d, 1);
    let on_cell = menu_at(&mut d, at);
    for label in [
        "Replace at position 2…",
        "Empty position 2",
        "Blanking plate at position 2",
    ] {
        assert!(on_cell.iter().any(|l| l == label), "{label}: {on_cell:?}");
    }
    d.input(&right_click(at));
    d.settle();
    d.input(&click(starting_with(&d, "Blanking plate at position 2")));
    d.settle();
    assert!(cells(&mut d)[1].blank, "a blanking plate at position 2");
    // A stand-alone valve (its coil wired) placed in cell 3 stays its own.
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("solenoid valve 5/2, spring"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Solenoid valve 5/2, spring return"),
        mods: Modifiers::NONE,
    });
    d.settle();
    let at = cell_px(&mut d, 2);
    click_at(&mut d, at);
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.contains("stays a device of its own"), "{toast}");
    d.input(&key("escape"));
    d.settle();
    assert!(cells(&mut d)[2].holder.is_none(), "not taken as a position");
    d.shell_mut().shutdown();
}

/// F15 follow-up: the multipin terminal's plug is drawn from the terminal's row: on an electrical page its menu offers
/// Draw the plug…, which arms the box of its pins; placed, the plug says beside its pins what each carries (the valve and
/// its solenoid, an empty position's number, the common).
#[test]
fn the_multipin_plug_is_drawn_from_its_terminal_and_names_its_pins() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("multipin"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Multipin valve terminal"),
        mods: Modifiers::NONE,
    });
    d.settle();
    click_at(&mut d, (760, 300));
    d.input(&key("escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("tool.select", &json!({}));
    let at = cell_px(&mut d, 0);
    click_at(&mut d, at);
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Solenoid valve 5/2 for a valve terminal"),
        mods: Modifiers::NONE,
    });
    d.settle();
    let terminal = fluid_nodes(&mut d)
        .into_iter()
        .find(|n| n.role == NavRole::Device)
        .map(|n| n.label)
        .expect("the terminal's row");
    // On the electrical page 1, the terminal's row offers Draw the plug….
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "1"}));
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    d.settle();
    let row = row_of(&mut d, &terminal);
    d.input(&right_click(row));
    d.settle();
    d.input(&click(starting_with(&d, &format!("Draw the plug of {terminal}"))));
    d.settle();
    assert_eq!(
        d.shell_mut().tool().name(),
        "black_box",
        "the plug's box on the pointer"
    );
    // One click places the plug, sized by its pins (friction 12).
    click_at(&mut d, (800, 400));
    assert_eq!(d.shell_mut().tool().name(), "select", "one click, one plug");
    let mut boxes = Vec::new();
    let p = d.shell_mut().canvas().unwrap().page().clone();
    glyph_boxes(&p.list.items, &|q| q, &mut boxes);
    // Each pin's text reads horizontally beside its pin: the pins stand down the box's two sides.
    let tables = std::sync::Arc::clone(&p.tables);
    let plug = tables
        .reps
        .iter()
        .find_map(|r| r.black_box.as_ref())
        .expect("the plug's box");
    use ew_viewmodel::ew_api::geometry::Dir;
    assert!(
        plug.cps.iter().all(|c| matches!(c.2, Dir::Left | Dir::Right)),
        "pins down the sides: {:?}",
        plug.cps
    );
    for (t, b) in boxes.iter().filter(|(t, _)| t == "COM" || t == "2 · 14") {
        assert!(b[2] - b[0] > b[3] - b[1], "{t} reads horizontally: {b:?}");
    }
    let texts: Vec<String> = boxes.into_iter().map(|(t, _)| t).collect();
    assert!(texts.iter().any(|t| t == "COM"), "the common: {texts:?}");
    assert!(
        texts.iter().any(|t| t == "2 · 14"),
        "an empty position's pin: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.ends_with(" 14") && t.starts_with('-')),
        "the valve's pin names the valve and its solenoid: {texts:?}"
    );
    d.shell_mut().shutdown();
}

/// Places the starter multipin valve terminal on the fluid page shown, its top-left corner at `a` (one click), then
/// Escape and Select; returns its tag.
fn place_multipin(d: &mut Driver, a: (i64, i64)) -> String {
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("multipin"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(d, "Multipin valve terminal"),
        mods: Modifiers::NONE,
    });
    d.settle();
    click_at(d, a);
    d.input(&key("escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("tool.select", &json!({}));
    fluid_nodes(d)
        .into_iter()
        .find(|n| n.role == NavRole::Device)
        .map(|n| n.label)
        .expect("the terminal's row")
}

/// The multipin terminal's plug could be drawn only from the Devices tree's row
/// while an electrical page was open; neither the terminal's menu on the fluid page nor the Fluid tab offered it. From
/// the fluid page the terminal's menu, its Properties row and the Fluid tab draw it as a valve's solenoid is drawn: the
/// electrical page shows with the plug's box on the pointer; placed, the menu and the row say where it is.
#[test]
fn the_multipin_plug_is_drawn_from_the_terminal_on_the_fluid_page() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let reason = |d: &mut Driver| {
        ew_viewmodel::actions::find("fluid.draw_plug")
            .and_then(|a| (a.enabled)(d.shell_mut()).err())
            .map(ew_viewmodel::strings::text)
    };
    assert_eq!(
        reason(&mut d),
        Some("Select a multipin valve terminal to draw its plug"),
        "the Fluid tab's button, greyed in words"
    );
    let terminal = place_multipin(&mut d, (760, 300));
    // Its body (the supply end, no cell): Draw the plug on an electrical page….
    let body = supply_px(&mut d);
    let draw = format!("Draw the plug of {terminal} on an electrical page…");
    let menu = menu_at(&mut d, body);
    assert!(menu.contains(&draw), "{menu:?}");
    click_at(&mut d, body);
    assert_eq!(
        reason(&mut d),
        None,
        "the Fluid tab's button, for the selected terminal"
    );
    assert_eq!(prop_row(&mut d, "device/derived.plug").0, "not drawn: draw it…");
    d.input(&right_click(body));
    d.settle();
    d.input(&click(starting_with(&d, &draw)));
    d.settle();
    // The page asked: the only electrical page chosen.
    assert_eq!(d.shell_mut().submit_form(), Outcome::Done);
    d.settle();
    assert_eq!(d.shell_mut().current_folio(), Some("1"), "the electrical page");
    assert_eq!(
        d.shell_mut().tool().name(),
        "black_box",
        "the plug's box on the pointer"
    );
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(
        toast,
        format!("Click where the plug of {terminal} goes on page 1.")
    );
    click_at(&mut d, (800, 400));
    // Back on the fluid page, the terminal says where its plug is.
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "201"}));
    let _ = d.shell_mut().dispatch("tool.select", &json!({}));
    d.settle();
    let body = supply_px(&mut d);
    let menu = menu_at(&mut d, body);
    let go = format!("Go to the plug of {terminal} at 1-");
    assert!(menu.iter().any(|l| l.starts_with(&go)), "{menu:?}");
    click_at(&mut d, body);
    assert!(
        prop_row(&mut d, "device/derived.plug").0.starts_with("1-"),
        "where it is drawn"
    );
    d.shell_mut().shutdown();
}

/// The cells of the box of valve terminal `terminal` drawn as its positions on the page shown.
fn cells_of(d: &mut Driver, terminal: Uuid) -> Vec<ew_viewmodel::ew_api::output::SlotCell> {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    tables
        .reps
        .iter()
        .filter(|r| r.device == Some(terminal))
        .find_map(|r| r.black_box.as_ref().filter(|b| !b.cells.is_empty()))
        .map(|b| b.cells.clone())
        .unwrap_or_default()
}

/// A cabinet run (friction 24, 25): a valve terminal made first as a PLC station, its valve in its rack, could not
/// be put on the pneumatic page: Fluid ▸ Valve terminal made a second terminal without a word, and the station's Draw on
/// this page refused a fluid page. Fluid ▸ Valve terminal offers the terminal drawn on no fluid page first (Draw … here,
/// then A new terminal…); a new one placed meanwhile says that the other is still undrawn; the rack dialog's Draw on a
/// fluid page… puts its box of positions on the pointer, its valve in its cell.
#[test]
fn a_terminal_made_as_a_station_is_drawn_on_the_fluid_page() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let _ = d.shell_mut().dispatch("plc.new_station", &json!({}));
    let set = json!({"head": "EW-GEN-VTM-1"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_set", &set), Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("plc.rack_create", &json!({})),
        Outcome::Done
    );
    let station = d
        .shell_mut()
        .station_tab()
        .and_then(|t| t.station)
        .expect("the station's view");
    let tag = fluid_nodes(&mut d)
        .into_iter()
        .find(|n| n.records.first() == Some(&station))
        .map(|n| n.label)
        .expect("the station's row");
    let _ = d.shell_mut().dispatch("ui.dismiss", &json!({}));
    let add = json!({"module": station.to_string(), "position": 1});
    assert_eq!(d.shell_mut().dispatch("fluid.valve_add", &add), Outcome::Done);
    d.input(&key("escape"));
    d.settle();
    // Fluid ▸ Valve terminal: the station first, then a new one.
    assert_eq!(
        d.shell_mut().dispatch("fluid.valve_terminal", &json!({})),
        Outcome::Done
    );
    let options: Vec<String> = match d.shell_mut().dialog() {
        Some(ew_viewmodel::shell::Dialog::Form(f)) => f.fields[0]
            .options
            .iter()
            .map(|(_, words)| words.clone())
            .collect(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        options,
        [format!("Draw {tag} here"), "A new valve terminal…".to_owned()]
    );
    // A new one, placed while the station is undrawn, says so.
    let new = json!({"terminal": "new"});
    assert_eq!(
        d.shell_mut().dispatch("fluid.valve_terminal", &new),
        Outcome::Done
    );
    assert!(d.shell_mut().picker().is_some(), "the device browser");
    d.settle();
    d.input(&typed("valve terminal"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Valve terminal with PROFIBUS DP bus node, 4"),
        mods: Modifiers::NONE,
    });
    d.settle();
    click_at(&mut d, (760, 300));
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        toast.contains(&format!("valve terminal {tag} is drawn on no fluid page yet")),
        "{toast}"
    );
    d.input(&key("escape"));
    d.settle();
    d.input(&key("escape"));
    d.settle();
    // The station's view, opened from the fluid page: Draw on page 201, its valve in cell 1.
    let rack = json!({"station": station.to_string()});
    assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
    // A frame to show the dialog (a dispatch asks for none).
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    // The toast of the terminal placed before lies over the To do's button: it is dismissed.
    d.input(&click(d.find("Dismiss").expect("the toast's Dismiss")));
    d.settle();
    d.input(&click(d.find("Draw on page 201").expect("the rack's step")));
    d.settle();
    assert_eq!(d.shell_mut().tool().name(), "black_box");
    click_at(&mut d, (760, 650));
    let c = cells_of(&mut d, station);
    assert_eq!(c.len(), 4, "drawn as its positions");
    assert!(c[0].holder.is_some(), "its valve in cell 1: {c:?}");
    // No terminal is left undrawn: the button opens the device browser at once.
    d.input(&key("escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("fluid.valve_terminal", &json!({}));
    assert!(d.shell_mut().dialog().is_none(), "no choice left");
    assert!(d.shell_mut().picker().is_some(), "the device browser");
    d.shell_mut().shutdown();
}

/// A valve terminal with a PROFINET node made in the PLC station view with `valves` valves; its device.
fn profinet_terminal(d: &mut Driver, valves: usize) -> Uuid {
    let _ = d.shell_mut().dispatch("plc.new_station", &json!({}));
    let set = json!({"head": "EW-GEN-VTM-2"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_set", &set), Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("plc.rack_create", &json!({})),
        Outcome::Done
    );
    for _ in 0..valves {
        let add = json!({"part": "EW-GEN-VLV-3"});
        assert_eq!(d.shell_mut().dispatch("plc.rack_add", &add), Outcome::Done);
    }
    d.shell_mut()
        .station_tab()
        .and_then(|t| t.station)
        .expect("the station's view")
}

/// Whether the To do list of the station view on show still asks for the terminal on a fluid page.
fn asks_for_fluid_page(d: &mut Driver) -> bool {
    use ew_viewmodel::shell::station::TodoFix;
    d.shell_mut()
        .station_todo()
        .iter()
        .any(|t| matches!(t.fix, Some(TodoFix::OnPage { step: "fluid", .. })))
}

/// A cabinet run (blocker): a valve terminal with a bus node, put on the bus page first, never reached the
/// pneumatic page: the Valve terminal dialog's Create and the station's Draw on a fluid page… did nothing and said
/// nothing (its bus port had become its main function). Create puts its box on the pointer; a click draws it with its
/// valves in their cells, and its To do no longer asks for it.
#[test]
fn a_terminal_on_a_bus_page_first_is_drawn_on_the_fluid_page() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let station = profinet_terminal(&mut d, 2);
    let bus = json!({"step": "bus"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_next", &bus), Outcome::Done);
    click_at(&mut d, (760, 300));
    d.input(&key("escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "201"}));
    // Fluid ▸ Valve terminal: Draw … here, then Create.
    assert_eq!(
        d.shell_mut().dispatch("fluid.valve_terminal", &json!({})),
        Outcome::Done
    );
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    d.input(&click(d.find("Create").expect("the dialog's Create")));
    d.settle();
    assert_eq!(d.shell_mut().tool().name(), "black_box", "its box on the pointer");
    click_at(&mut d, (760, 400));
    d.input(&key("escape"));
    d.settle();
    let c = cells_of(&mut d, station);
    assert_eq!(c.len(), 4, "drawn as its positions");
    assert!(
        c[0].holder.is_some() && c[1].holder.is_some(),
        "its valves in their cells: {c:?}"
    );
    let rack = json!({"station": station.to_string()});
    assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
    assert!(!asks_for_fluid_page(&mut d), "drawn on a fluid page");
    d.shell_mut().shutdown();
}

/// A rack run: in a project without a fluid page the station's Draw on a fluid page… did nothing and said
/// nothing. Draw on a new fluid page makes one there and then, says so, and puts the terminal's box on the pointer
/// there.
#[test]
fn draw_on_a_fluid_page_makes_one_when_there_is_none() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let station = profinet_terminal(&mut d, 1);
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    d.input(&click(
        d.find("Draw on a new fluid page").expect("the To do's button"),
    ));
    d.settle();
    let kind = {
        let s = d.shell_mut();
        s.page_rows()
            .get(s.page_index())
            .map(|r| ew_viewmodel::ew_api::output::page_kind_name(&r.kind))
    };
    assert_eq!(kind.as_deref(), Some("fluid"), "a fluid page made and shown");
    assert_eq!(d.shell_mut().tool().name(), "black_box");
    click_at(&mut d, (760, 400));
    assert_eq!(cells_of(&mut d, station).len(), 4, "drawn there");
    d.shell_mut().shutdown();
}

/// With several fluid pages the station's Draw on a fluid page… asks which, as Draw on a page… does, and draws on
/// the one chosen (from a view opened on another page than a fluid one).
#[test]
fn draw_on_a_fluid_page_asks_which_of_several() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let page = json!({"folio": "202", "type": "fluid", "title": "Eject"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "1"}));
    let station = profinet_terminal(&mut d, 1);
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    d.input(&click(
        d.find("Draw on a fluid page…").expect("the To do's button"),
    ));
    d.settle();
    assert!(d.find("201 Clamp pneumatics").is_some(), "the first page offered");
    assert!(d.find("New page…").is_some(), "a new page offered");
    d.input(&click(d.find("202 Eject").expect("the second page offered")));
    d.settle();
    assert_eq!(d.shell_mut().current_folio(), Some("202"));
    click_at(&mut d, (760, 400));
    assert_eq!(cells_of(&mut d, station).len(), 4, "drawn there");
    d.shell_mut().shutdown();
}

/// Valves put on a valve terminal's positions in its station view stood in its cells as tags only, with no
/// port a tube could reach. All four positions are filled, as on the owner's terminal. Drawn on the fluid page from the view's Draw on a fluid page…, each valve stands in its cell
/// as its symbol, its working ports 2 and 4 on the cell's top edge, and the terminal's own supply ports 1, 3 and 5 in
/// its left end; a tube joins port 4 of a valve to a cylinder, and the view's positions name the page.
#[test]
fn valves_filled_in_the_station_view_stand_in_their_cells_with_ports() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let _ = d.shell_mut().dispatch("plc.new_station", &json!({}));
    let set = json!({"head": "EW-GEN-VTM-1"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_set", &set), Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("plc.rack_create", &json!({})),
        Outcome::Done
    );
    let station = d
        .shell_mut()
        .station_tab()
        .and_then(|t| t.station)
        .expect("the station's view");
    for _ in 0..4 {
        let add = json!({"part": "EW-GEN-VLV-3"});
        assert_eq!(d.shell_mut().dispatch("plc.rack_add", &add), Outcome::Done);
    }
    let fluid = json!({"step": "fluid"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_next", &fluid), Outcome::Done);
    assert_eq!(d.shell_mut().tool().name(), "black_box");
    click_at(&mut d, (760, 400));
    d.input(&key("escape"));
    d.settle();
    let c = cells_of(&mut d, station);
    assert_eq!(c.len(), 4, "drawn as its positions");
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let ports_of = |device: Uuid| -> Vec<(String, (i64, i64))> {
        tables
            .cps
            .iter()
            .filter(|p| {
                tables
                    .reps
                    .iter()
                    .any(|r| r.id == p.rep && r.device == Some(device) && r.black_box.is_none())
            })
            .map(|p| (p.label.rsplit(':').next().unwrap_or_default().to_owned(), p.at))
            .collect()
    };
    let mut valves = Vec::new();
    for cell in &c {
        let valve = cell.holder.expect("a valve on the position");
        let symbol = tables
            .reps
            .iter()
            .find(|r| r.device == Some(valve))
            .expect("the valve's symbol on the page");
        assert!(
            cell.rect[0] < symbol.pos.0 && symbol.pos.0 < cell.rect[2],
            "in its cell"
        );
        let ports = ports_of(valve);
        for working in ["2", "4"] {
            let at = ports.iter().find(|(d, _)| d == working).map(|(_, at)| *at);
            assert_eq!(
                at.map(|a| a.1),
                Some(cell.rect[1]),
                "port {working} on the top edge"
            );
        }
        valves.push(valve);
    }
    let mut supply: Vec<String> = ports_of(station).into_iter().map(|(d, _)| d).collect();
    supply.sort();
    assert_eq!(supply, ["1", "3", "5"], "the terminal's supply ports");
    // A tube from port 4 of the first valve to a cylinder above the terminal.
    place_symbol(&mut d, "ew.iso.cylinder-double-acting@qet10", (900, 250));
    let cylinder = device_drawn(&mut d, "ew.iso.cylinder");
    let (a, b) = (port(&mut d, valves[0], "4"), port(&mut d, cylinder, "A"));
    draw_tube(&mut d, a, b);
    assert_eq!(tubes(&mut d), 1, "the tube");
    let view = d.shell_mut().rack_view(station).expect("the station's view");
    let pages: Vec<Option<String>> = view
        .cells
        .iter()
        .filter(|c| c.at.holder.is_some())
        .map(|c| c.at.page.clone())
        .collect();
    assert_eq!(
        pages,
        vec![Some("201".to_owned()); 4],
        "four valves, each drawn in its cell"
    );
    d.shell_mut().shutdown();
}

/// The owner's order: the valve terminal made as a station and drawn on the schematic page first, then on the
/// fluid page from its rack dialog: there too it is drawn as its positions, another drawing of the same terminal.
#[test]
fn a_station_drawn_on_the_schematic_is_drawn_on_the_fluid_page_too() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "1"}));
    let _ = d.shell_mut().dispatch("plc.new_station", &json!({}));
    let _ = d
        .shell_mut()
        .dispatch("plc.rack_set", &json!({"head": "EW-GEN-VTM-1"}));
    assert_eq!(
        d.shell_mut().dispatch("plc.rack_create", &json!({})),
        Outcome::Done
    );
    let station = d
        .shell_mut()
        .station_tab()
        .and_then(|t| t.station)
        .expect("the station's view");
    let draw = json!({"step": "draw"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_next", &draw), Outcome::Done);
    let _ = d
        .shell_mut()
        .dispatch("plc.rack_editor", &json!({"station": station.to_string()}));
    let fluid = json!({"step": "fluid"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_next", &fluid), Outcome::Done);
    assert_eq!(d.shell_mut().current_folio(), Some("201"));
    assert_eq!(d.shell_mut().tool().name(), "black_box");
    click_at(&mut d, (760, 400));
    assert_eq!(cells_of(&mut d, station).len(), 4, "drawn as its positions");
    d.shell_mut().shutdown();
}

/// A right click on the multipin terminal's body offered no
/// position (only one exactly in a cell did), its kind's line looked like a command with a key and did nothing, the
/// browser of an empty cell listed hand-operated valves that fit no position, and a valve's "No medium" line had a key
/// too. On the body the menu lists every position under a heading, the same for both kinds; the kind's line is a
/// heading; the cell's browser lists the valves made for a terminal, the first of them chosen.
#[test]
fn a_terminals_menu_lists_its_positions_from_the_body() {
    use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let terminal = place_multipin(&mut d, (760, 300));
    let body = supply_px(&mut d);
    let menu = menu_at(&mut d, body);
    for label in [
        "Position 1: empty",
        "Fill position 1…",
        "Position 8: empty",
        "Blanking plate at position 8",
    ] {
        assert!(menu.iter().any(|l| l == label), "{label}: {menu:?}");
    }
    click_at(&mut d, body);
    let items = canvas_menu(d.shell_mut(), MenuSpot::Object, None);
    let kind = items
        .iter()
        .find(|i| i.label == "Valve terminal · multipin, wired")
        .expect("the kind's line");
    assert!(kind.shortcut.is_empty() && !kind.enabled, "a heading: {kind:?}");
    let heading = items
        .iter()
        .find(|i| i.label == "Position 1: empty")
        .expect("the position's heading");
    assert!(heading.shortcut.is_empty() && !heading.enabled);
    // The empty cell's browser: the valves made for a terminal, the first chosen.
    let at = cell_px(&mut d, 0);
    click_at(&mut d, at);
    let picker = d.shell_mut().picker().cloned().expect("the device browser");
    let parts: Vec<&str> = picker
        .rows
        .iter()
        .filter_map(|r| match r {
            ew_viewmodel::tools::PickerRow::Part(p) => Some(picker.catalogue.parts[*p].part_number.as_str()),
            _ => None,
        })
        .collect();
    assert!(!parts.is_empty(), "valves for a terminal");
    assert!(
        !parts
            .iter()
            .any(|p| *p == "EW-GEN-SOV-1" || *p == "EW-GEN-VLV-10"),
        "no hand-operated valve: {parts:?}"
    );
    assert!(
        matches!(
            picker.rows.get(picker.row),
            Some(ew_viewmodel::tools::PickerRow::Part(p))
                if picker.catalogue.parts[*p].terminal_valve
        ),
        "the first valve chosen"
    );
    // Its card names its valve function in words, the part's description, not its category ("Lever onoffvalve").
    let card = picker.card.as_ref().expect("the first valve's card");
    let words = &picker.catalogue.parts[match picker.rows[picker.row] {
        ew_viewmodel::tools::PickerRow::Part(p) => p,
        _ => unreachable!(),
    }]
    .description;
    assert!(
        card.card.functions.iter().any(|(k, _)| k == words),
        "{:?}",
        card.card.functions
    );
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Solenoid valve 5/2 for a valve terminal"),
        mods: Modifiers::NONE,
    });
    d.settle();
    // The valve's medium line has no key.
    let valve = fluid_nodes(&mut d)
        .into_iter()
        .find(|n| n.role == NavRole::Device && n.label != terminal)
        .and_then(|n| n.records.first().copied())
        .expect("the valve");
    let rep = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device == Some(valve))
        .map(|r| r.id)
        .expect("the valve's symbol");
    let _ = d
        .shell_mut()
        .dispatch("select.set", &json!({ "ids": [rep.to_string()] }));
    d.settle();
    let items = canvas_menu(d.shell_mut(), MenuSpot::Object, None);
    let none = items
        .iter()
        .find(|i| i.label.starts_with("No medium"))
        .unwrap_or_else(|| panic!("the medium line: {items:?}"));
    assert!(none.shortcut.is_empty(), "no key: {none:?}");
    // The bus terminal's body lists its positions the same way.
    place_terminal(&mut d, (760, 650));
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let supply = tables
        .reps
        .iter()
        .filter_map(|r| r.black_box.as_ref().and_then(|b| b.supply_end))
        .max_by_key(|r| r[1])
        .expect("the second terminal's supply end");
    let at = page_pixel(&mut d, ((supply[0] + supply[2]) / 2, (supply[1] + supply[3]) / 2));
    let menu = menu_at(&mut d, at);
    assert!(menu.iter().any(|l| l == "Position 4: empty"), "{menu:?}");
    d.shell_mut().shutdown();
}

/// The window pixels of the middles of the supply ends of the valve terminals drawn on the page shown, top to bottom.
fn supply_ends(d: &mut Driver) -> Vec<(i64, i64)> {
    let tables = std::sync::Arc::clone(&d.shell_mut().canvas().unwrap().page().tables);
    let mut ends: Vec<[i64; 4]> = tables
        .reps
        .iter()
        .filter_map(|r| r.black_box.as_ref().and_then(|b| b.supply_end))
        .collect();
    ends.sort_by_key(|r| r[1]);
    ends.into_iter()
        .map(|r| page_pixel(d, ((r[0] + r[2]) / 2, (r[1] + r[3]) / 2)))
        .collect()
}

/// A re-test (friction 13): the bus-node terminal's Properties said "Valve terminal · PLC station" and the
/// multipin one's "Black-box frame · electrical", so the two kinds could be told apart only by their side effects. Each
/// terminal's Properties header, its menu's heading and its row's hover in the Devices list say its kind in the same
/// words.
#[test]
fn each_valve_terminal_says_its_kind() {
    use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let multipin = place_multipin(&mut d, (760, 300));
    place_terminal(&mut d, (760, 650));
    let ends = supply_ends(&mut d);
    for (at, words) in [
        (ends[0], "Valve terminal · multipin, wired"),
        (ends[1], "Valve terminal · PLC station on PROFIBUS DP"),
    ] {
        click_at(&mut d, at);
        assert!(
            d.texts().iter().any(|t| t == words),
            "Properties' header: {words}"
        );
        let menu = canvas_menu(d.shell_mut(), MenuSpot::Object, None);
        assert!(
            menu.iter().any(|i| i.label == words && !i.enabled),
            "the menu's heading: {words}"
        );
    }
    // The Devices list's row of the multipin terminal says it under the pointer.
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({ "kind": "devices" }));
    // A frame to show the list (a dispatch asks for none).
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    let name = d
        .shell_mut()
        .shown_rows(NavKind::Devices)
        .into_iter()
        .find(|r| r.label == multipin)
        .map(|r| r.name)
        .expect("the terminal's row in the Devices list");
    let at = d.find(&name).expect("the row on show");
    d.input(&Input::Hover { at });
    d.input(&Input::Wait { ms: 1000 });
    d.settle();
    assert!(
        d.texts().iter().any(|t| t == "Valve terminal · multipin, wired"),
        "the row's hover"
    );
    d.shell_mut().shutdown();
}

/// A re-test (friction 11): a valve for a terminal placed from the device browser over position 1 of the multipin
/// terminal was taken (its plug's pin 1 named it), but the toast said the terminal had no free position and the valve
/// stayed one of its own. The toast says where it sits, as on the bus-node terminal.
#[test]
fn a_valve_dropped_on_a_multipin_position_is_said_to_sit_there() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let terminal = place_multipin(&mut d, (760, 300));
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("for a valve terminal"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Solenoid valve 5/2 for a valve terminal"),
        mods: Modifiers::NONE,
    });
    d.settle();
    let at = cell_px(&mut d, 0);
    click_at(&mut d, at);
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(
        toast.ends_with(&format!(" is the valve at position 1 of {terminal} now.")),
        "{toast}"
    );
    d.input(&key("escape"));
    d.settle();
    assert!(cells(&mut d)[0].holder.is_some(), "position 1 holds it");
    d.shell_mut().shutdown();
}

/// A re-test (friction 5, 7): on the control page the valve's coil row in the Devices list offered Location…
/// only, and Follow reference with the coil selected did nothing visible. The coil's row offers Draw solenoid 14 on this
/// page… while an electrical page is in front (on an electrical page… from the fluid page) and Go to the valve; drawn
/// and selected, Follow reference goes to the valve.
#[test]
fn the_solenoid_is_drawn_from_the_coils_row_and_followed_back() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    d.input(&click(d.find("Device").unwrap()));
    d.settle();
    d.input(&typed("solenoid valve 5/2, spring"));
    d.settle();
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Solenoid valve 5/2, spring return"),
        mods: Modifiers::NONE,
    });
    d.settle();
    click_at(&mut d, (900, 500));
    d.input(&key("escape"));
    d.settle();
    let device = d.shell_mut().canvas().unwrap().page().tables.reps[0]
        .device
        .expect("the valve");
    let _ = d
        .shell_mut()
        .dispatch("navigator.show", &json!({ "kind": "devices" }));
    // A frame to show the list (a dispatch asks for none).
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    let valve = d
        .shell_mut()
        .nav(NavKind::Devices)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.records.first() == Some(&device))
                .cloned()
        })
        .map(|n| n.label)
        .expect("the valve's row");
    let coil_row = |d: &mut Driver| {
        let name = d
            .shell_mut()
            .shown_rows(NavKind::Devices)
            .into_iter()
            .find(|r| r.label.starts_with("Coil"))
            .map(|r| r.name)
            .expect("the coil's row");
        d.find(&name).expect("the coil's row on show")
    };
    let row = coil_row(&mut d);
    let on_fluid = menu_at(&mut d, row);
    assert!(
        on_fluid.contains(&"Draw solenoid 14 on an electrical page…".to_owned()),
        "{on_fluid:?}"
    );
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "1"}));
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    d.settle();
    let row = coil_row(&mut d);
    let menu = menu_at(&mut d, row);
    let go = format!("Go to the valve {valve}");
    for label in ["Draw solenoid 14 on this page…", go.as_str()] {
        assert!(menu.iter().any(|l| l == label), "{label}: {menu:?}");
    }
    d.input(&right_click(row));
    d.settle();
    d.input(&click(starting_with(&d, "Draw solenoid 14 on this page…")));
    d.settle();
    assert_eq!(d.shell_mut().current_folio(), Some("1"), "this page");
    assert_eq!(d.shell_mut().tool().name(), "place");
    click_at(&mut d, (900, 500));
    d.input(&key("escape"));
    d.settle();
    // The coil selected, Follow reference (its key, F) goes to the valve.
    let coil = d
        .shell_mut()
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.symbol_key.starts_with("ew.iec.valve-coil"))
        .map(|r| r.id)
        .expect("the coil drawn");
    let _ = d
        .shell_mut()
        .dispatch("select.set", &json!({ "ids": [coil.to_string()] }));
    d.settle();
    d.input(&key("F"));
    d.settle();
    assert_eq!(d.shell_mut().current_folio(), Some("201"), "the valve's page");
    d.shell_mut().shutdown();
}

/// A re-test (friction 8, 9, 14): the check said "fit a valve or a blanking plate", and the person searched the
/// parts for a blanking plate and found none. The browser of an empty cell lists Blanking plate first (the first valve
/// stays chosen) and blanks the position; EW-FL-013 says where it is (a right click on the position) and offers it as
/// its fix; Fix all blanks every empty position in one step, and one undo brings them back.
#[test]
fn a_blanking_plate_is_found_from_the_cell_and_the_check() {
    use ew_viewmodel::tools::PickerRow;
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_terminal(&mut d, (760, 300));
    let _ = d.shell_mut().dispatch("tool.select", &json!({}));
    let at = cell_px(&mut d, 0);
    click_at(&mut d, at);
    let picker = d.shell_mut().picker().cloned().expect("the device browser");
    assert_eq!(picker.rows.first(), Some(&PickerRow::Blank));
    assert!(
        matches!(picker.rows.get(picker.row), Some(PickerRow::Part(_))),
        "the first valve chosen"
    );
    d.input(&Input::DoubleClick {
        at: starting_with(&d, "Blanking plate"),
        mods: Modifiers::NONE,
    });
    d.settle();
    assert!(cells(&mut d)[0].blank, "position 1 blanked");
    // The check says where, and its fix fits the plate.
    let messages = findings(&mut d, "EW-FL-013");
    assert_eq!(messages.len(), 3, "positions 2 to 4 empty: {messages:?}");
    assert!(
        messages
            .iter()
            .all(|m| m.ends_with("(right-click the position: Blanking plate)")),
        "{messages:?}"
    );
    let nodes = d
        .shell_mut()
        .nav(NavKind::Messages)
        .and_then(|n| n.table())
        .map(|t| t.nodes.clone())
        .unwrap_or_default();
    assert!(
        nodes
            .iter()
            .any(|n| n.cells.iter().any(|c| c == "Blanking plate on position 2")),
        "the fix"
    );
    let group = "severity:warning/rule:EW-FL-013";
    assert_eq!(
        d.shell_mut().fix_all_offer(group).map(|o| o.1),
        Some(3),
        "Fix all (3)…"
    );
    let fix_all = json!({ "key": group });
    assert_eq!(
        d.shell_mut().dispatch("messages.fix_all", &fix_all),
        Outcome::Done
    );
    assert_eq!(d.shell_mut().dispatch("preview.apply", &json!({})), Outcome::Done);
    d.settle();
    assert!(cells(&mut d).iter().all(|c| c.blank), "every position blanked");
    let _ = d.shell_mut().dispatch("edit.undo", &json!({}));
    d.settle();
    let c = cells(&mut d);
    assert!(
        c[0].blank && c[1..].iter().all(|c| c.holder.is_none()),
        "one undo"
    );
    d.shell_mut().shutdown();
}

/// A re-test (friction 12): from the fluid page Draw plug went to page 1, an empty front page, without a word,
/// and could not be pointed at the control page. From a fluid page it asks on which electrical page (the one after this
/// page chosen, New page… offered) and draws there; with an electrical page in front it draws there, asking nothing.
#[test]
fn draw_plug_draws_where_the_person_is() {
    use ew_viewmodel::shell::Dialog;
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let page = json!({"folio": "202", "type": "multi_line", "title": "Clamp controls"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "201"}));
    let _ = d.shell_mut().dispatch("view.fit_page", &json!({}));
    d.settle();
    let terminal = place_multipin(&mut d, (760, 300));
    let body = supply_px(&mut d);
    click_at(&mut d, body);
    // The Fluid tab's Draw plug, the terminal selected.
    assert_eq!(
        d.shell_mut().dispatch("fluid.draw_plug", &json!({})),
        Outcome::Done
    );
    d.settle();
    {
        let Some(Dialog::Form(form)) = d.shell_mut().dialog() else {
            panic!("the page asked");
        };
        let page = form.fields.iter().find(|f| f.name == "page").expect("the page");
        assert_eq!(page.value, "202", "the page after this one");
        assert!(
            page.options.iter().any(|(v, w)| v == "new" && w == "New page…"),
            "{:?}",
            page.options
        );
    }
    assert_eq!(d.shell_mut().current_folio(), Some("201"), "nothing moved yet");
    assert_eq!(d.shell_mut().submit_form(), Outcome::Done);
    d.settle();
    assert_eq!(d.shell_mut().current_folio(), Some("202"));
    assert_eq!(d.shell_mut().tool().name(), "black_box");
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(
        toast,
        format!("Click where the plug of {terminal} goes on page 202.")
    );
    d.input(&key("escape"));
    d.settle();
    // On page 1 in front, the terminal's row draws it there, asking nothing.
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "1"}));
    d.settle();
    let row = row_of(&mut d, &terminal);
    d.input(&right_click(row));
    d.settle();
    let here = format!("Draw the plug of {terminal} on this page…");
    d.input(&click(d.find(&here).expect("on this page")));
    d.settle();
    assert!(d.shell_mut().dialog().is_none(), "nothing asked");
    assert_eq!(d.shell_mut().current_folio(), Some("1"));
    assert_eq!(d.shell_mut().tool().name(), "black_box");
    d.shell_mut().shutdown();
}

/// A re-test (friction 10): the 8-position multipin terminal placed near the frame's right edge ran past it,
/// positions 4 and up cut off, without a word. Placed there, it stands inside the drawing area, moved left.
#[test]
fn a_terminal_placed_at_the_frames_edge_stands_inside_it() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    let area = {
        let page = d.shell_mut().canvas().unwrap().page().clone();
        ew_viewmodel::tools::ip::drawing_area(&page).expect("a frame grid")
    };
    // A frame under the pointer first, so the canvas reports where it is.
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    let near_edge = page_pixel(&mut d, (area[2] - 20_000_000, area[1] + 40_000_000));
    place_multipin(&mut d, near_edge);
    let c = cells(&mut d);
    assert_eq!(c.len(), 8, "the multipin terminal's positions");
    let right = c.iter().map(|x| x.rect[2]).max().unwrap();
    // To the nanometre the frame's page-to-sheet map rounds the drawing area's edge to.
    assert!(
        right <= area[2] + 1,
        "inside the drawing area: {right} > {}",
        area[2]
    );
    d.shell_mut().shutdown();
}

/// A re-test (friction 2, 3): after choosing "Compressed air, 6 bar" the supply's Medium row read "Compressed
/// air", marked derived, and the Tube button's mode read "Wire". The row shows the medium as the list offered it, as the
/// supply's own; the mode reads Tube.
#[test]
fn the_medium_row_and_the_tube_mode_say_what_was_chosen() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_symbol(&mut d, "ew.iso.air-supply@qet10", (1300, 800));
    let supply = device_drawn(&mut d, "ew.iso.air-supply");
    let _ = d
        .shell_mut()
        .dispatch("select.set", &json!({ "ids": [supply.to_string()] }));
    let edit = json!({"row": "device/derived.medium"});
    assert_eq!(d.shell_mut().dispatch("props.edit", &edit), Outcome::Done);
    assert_eq!(
        choice_texts(&mut d).first().map(String::as_str),
        Some("Compressed air, 6 bar")
    );
    assert_eq!(
        d.shell_mut().dispatch("props.choose", &json!({"index": 0})),
        Outcome::Done
    );
    let _ = d
        .shell_mut()
        .dispatch("select.set", &json!({ "ids": [supply.to_string()] }));
    let (text, chip) = prop_row(&mut d, "device/derived.medium");
    assert_eq!(text, "Compressed air, 6 bar");
    assert!(!chip.contains("derived"), "its own: {chip}");
    assert_eq!(
        d.shell_mut().dispatch("tool.fluid_line", &json!({})),
        Outcome::Done
    );
    let hud = ew_viewmodel::command_bar::hud(d.shell_mut());
    assert_eq!(hud.tool_label, "Tube");
    d.shell_mut().shutdown();
}

/// A cabinet run (friction 26, 27): Tube opened a small dialog first, so the clicks meant for the ports fell
/// into it, and nothing showed the tube's size. Tube starts drawing at once with the line kind used last, the HUD says
/// the kind and the project's tube and changes the kind, and a project without a default tube is told once where its
/// size is set.
#[test]
fn the_tube_tool_draws_at_once_and_says_its_kind_and_size() {
    let tmp = Tmp::new();
    let mut d = fluid_page(&tmp);
    place_symbol(&mut d, "ew.iso.cylinder-double-acting@qet10", (900, 600));
    place_symbol(&mut d, "ew.iso.air-supply@qet10", (1300, 800));
    let (cylinder, supply) = (
        device_drawn(&mut d, "ew.iso.cylinder"),
        device_drawn(&mut d, "ew.iso.air-supply"),
    );
    let (a, b) = (port(&mut d, supply, "1"), port(&mut d, cylinder, "A"));
    assert_eq!(
        d.shell_mut().dispatch("tool.fluid_line", &json!({})),
        Outcome::Done
    );
    assert!(d.shell_mut().dialog().is_none(), "no dialog first");
    assert_eq!(d.shell_mut().tool().name(), "wire");
    let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
    assert!(toast.starts_with("This project has no default tube"), "{toast}");
    let hud = |d: &mut Driver| ew_viewmodel::command_bar::hud(d.shell_mut());
    let h = hud(&mut d);
    assert!(
        h.prompt.starts_with("Working line · no default tube"),
        "{}",
        h.prompt
    );
    assert!(h.quick.contains(&"fluid.tube_kind"), "{:?}", h.quick);
    click_at(&mut d, a);
    click_at(&mut d, b);
    d.input(&key("escape"));
    d.settle();
    assert_eq!(tubes(&mut d), 1, "the clicks drew the tube");
    // The kind is changed in the HUD and remembered; the project's tube shows once it is set.
    assert_eq!(
        d.shell_mut().dispatch("fluid.tube_kind", &json!({})),
        Outcome::Done
    );
    let set = json!({"od": "6", "id": "4"});
    assert_eq!(d.shell_mut().dispatch("fluid.settings", &set), Outcome::Done);
    let _ = d.shell_mut().dispatch("tool.select", &json!({}));
    let _ = d.shell_mut().dispatch("tool.fluid_line", &json!({}));
    let h = hud(&mut d);
    assert!(
        !h.prompt.starts_with("Working") && h.prompt.contains(" line · tube 6/4 mm"),
        "{}",
        h.prompt
    );
    d.shell_mut().shutdown();
}
