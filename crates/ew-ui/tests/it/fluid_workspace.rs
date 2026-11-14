// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Fluid workspace in the window (FR-UI-003, FR-UI-006, doc 11 §7), at 1920 × 1080: View ▸ Workspaces ▸ Fluid power
//! shows the fluid navigator with Golden A's valve terminal, its modules and its valves with their lines, and the tube
//! and medium tables in the bottom dock; a tube's length typed into its cell becomes manual; New medium… adds a medium,
//! a fluid line's context menu sets it and picks the line's kind, and the medium's row highlights its zones.

use crate::support;

use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::ew_viewmodel::layout::PaneKind;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use serde_json::json;
use support::{Tmp, dispatch, on_golden, settle};

/// The four pointer events of a double click at `at`, in one frame.
fn double_click(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, at: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false, true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    settle(h);
}

#[test]
fn the_fluid_workspace_lists_valves_lines_and_tubes_and_edits_a_length() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    h.get_by_role_and_label(egui::Role::Tab, "View").click();
    settle(&mut h);
    support::bar_button(&h, "Workspaces").click();
    settle(&mut h);
    support::menu_node(&h, "Fluid power").click();
    settle(&mut h);
    {
        let shell = h.state().shell();
        assert_eq!(shell.workspace(), "Fluid");
        let layout = shell.layout();
        assert_eq!(
            layout.shown_in_place_of(PaneKind::Pages),
            Some(PaneKind::Navigator(NavKind::Fluid))
        );
        assert!(layout.panes().contains(&PaneKind::Navigator(NavKind::Tubes)));
        assert!(layout.panes().contains(&PaneKind::Navigator(NavKind::Media)));
    }
    let rows = h.state().shell().shown_rows(NavKind::Fluid);
    assert!(rows.iter().any(|r| r.text == "Valve terminals"));
    let terminal = rows
        .iter()
        .find(|r| r.label == "-201U1")
        .expect("the valve terminal");
    let modules: Vec<&str> = rows
        .iter()
        .filter(|r| r.parent.as_deref() == Some(terminal.key.as_str()) && r.role == NavRole::Device.word())
        .map(|r| r.label.as_str())
        .collect();
    assert!(
        modules.contains(&"-201U3") && modules.contains(&"-202U2"),
        "{modules:?}"
    );
    let valves = rows
        .iter()
        .filter(|r| r.level == 4 && r.role == NavRole::Device.word())
        .count();
    assert_eq!(valves, 28, "FR-UI-006: the 28 golden valves");
    assert!(
        rows.iter()
            .any(|r| r.role == NavRole::Connection.word() && r.label.starts_with("→ -203Q1:")),
        "the first valve's lines to its cylinder"
    );
    // The tube table: a length typed into the first tube's cell becomes manual.
    let tubes = h.state().shell().shown_rows(NavKind::Tubes);
    assert!(!tubes.is_empty(), "the tube list's rows");
    let at = h
        .get_all_by_role(egui::accesskit::Role::Cell)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Length"))
        })
        .expect("a length cell")
        .rect()
        .center();
    double_click(&mut h, at);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("2500".to_owned()));
    settle(&mut h);
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert!(
        h.query_all_by_label_contains("Source manual").count() >= 1,
        "the length's source is manual"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A net whose medium is defined at its other end: the line's context menu checks that medium, and Medium: of another
/// one replaces the net's definition rather than adding a second one beside it, so the net carries the new medium
/// alone (not "mixed") and the menu checks it.
#[test]
fn a_medium_set_on_a_line_replaces_the_definition_of_its_net() {
    use ew_ui::ew_viewmodel::ew_api::output::TableEnd;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    for (name, p_nom) in [("Air 6 bar", "6"), ("Air 4 bar", "4")] {
        dispatch(&mut h, "fluid.medium_new", json!({"name": name, "p_nom": p_nom}));
    }
    let (line, b) = {
        let c = h.state().shell().canvas().unwrap();
        let line = c
            .page()
            .tables
            .connections
            .iter()
            .find(|c| c.kind == "fluid" && c.polyline.len() >= 2 && matches!(c.b, TableEnd::Cp { .. }))
            .expect("a fluid line on 203");
        (line.id.to_string(), line.b)
    };
    let TableEnd::Cp { function, idx } = b else {
        unreachable!("a CP");
    };
    dispatch(&mut h, "rail.show", json!({"pane": "fluid"}));
    settle(&mut h);
    let six = h
        .state()
        .shell()
        .nav(NavKind::Fluid)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::Medium && n.label == "Air 6 bar")
                .map(|n| n.records[0])
        })
        .expect("Air 6 bar");
    dispatch(
        &mut h,
        "command.run",
        json!({"cmd": "fluid.medium.define", "params": {"function": function.to_string(), "idx": idx,
            "medium": six.to_string()}}),
    );
    settle(&mut h);
    let at = crate::context_menu::point_on(&h, &line);
    let checked = |h: &egui_kittest::Harness<'_, ew_ui::app::App>, label: &str| {
        h.get_by_label(label).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
    };
    crate::tools::right_click(&mut h, at);
    assert!(
        checked(&h, "Medium: Air 6 bar"),
        "the medium defined at the other end"
    );
    support::menu_node(&h, "Medium: Air 4 bar").click();
    settle(&mut h);
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert_eq!(toast, "The tube carries Air 4 bar now.");
    crate::tools::right_click(&mut h, at);
    assert!(checked(&h, "Medium: Air 4 bar"), "the net carries the new medium");
    assert!(!checked(&h, "Medium: Air 6 bar"), "and not the old one");
    h.state_mut().shell_mut().shutdown();
}

/// Media from nothing on Golden A, which defines none: Project ▸ New medium… adds one; a fluid line's context menu on
/// folio 203 sets it on the line's net and sets the line's kind; the medium table's row counts the consumers and
/// highlights the zones.
#[test]
fn a_medium_is_added_set_on_a_line_and_highlighted() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    support::open_menu(&mut h, &["Project"]);
    settle(&mut h);
    support::menu_node(&h, "New medium…").click();
    settle(&mut h);
    h.get_by_label("Name").click();
    h.run_ok();
    h.event(egui::Event::Text("Compressed air 6 bar".to_owned()));
    h.get_by_label("Pressure (bar)").click();
    h.run_ok();
    h.event(egui::Event::Text("6".to_owned()));
    settle(&mut h);
    h.get_by_label("Create").click();
    settle(&mut h);
    assert!(
        h.state().shell().dialog().is_none(),
        "{:?}",
        h.state().shell().dialog()
    );
    let line = {
        let c = h.state().shell().canvas().unwrap();
        c.page()
            .tables
            .connections
            .iter()
            .find(|c| c.kind == "fluid" && c.polyline.len() >= 2)
            .expect("a fluid line on 203")
            .id
            .to_string()
    };
    let at = crate::context_menu::point_on(&h, &line);
    crate::tools::right_click(&mut h, at);
    support::menu_node(&h, "Medium: Compressed air 6 bar").click();
    settle(&mut h);
    let toast = h
        .state()
        .shell()
        .toast()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    assert_eq!(toast, "The tube carries Compressed air 6 bar now.");
    crate::tools::right_click(&mut h, at);
    support::menu_node(&h, "Line kind: Pilot").click();
    settle(&mut h);
    let kind = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .find(|c| c.id.to_string() == line)
        .and_then(|c| c.line_kind.clone());
    assert_eq!(kind.as_deref(), Some("pilot"));
    // FT-1733: a flexible line (a hose) is drawn as an arc.
    let curved = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        use ew_ui::ew_viewmodel::ew_api::output::ew_dl::{ItemKind, PathEl};
        let c = h.state().shell().canvas().unwrap();
        c.page().list.items.iter().any(|i| {
            i.entity.is_some_and(|e| e.0.to_string() == line)
                && matches!(&i.kind, ItemKind::Path(p) if p.path.els.iter().any(|e| matches!(e, PathEl::QuadTo(..))))
        })
    };
    assert!(!curved(&h), "a pilot line is straight");
    crate::tools::right_click(&mut h, at);
    support::menu_node(&h, "Line kind: Flexible").click();
    settle(&mut h);
    assert!(curved(&h), "a flexible line is an arc");
    // The medium's zones, counted in the fluid navigator and highlighted from the medium table's row.
    dispatch(&mut h, "rail.show", json!({"pane": "fluid"}));
    settle(&mut h);
    let row = h
        .state()
        .shell()
        .shown_rows(NavKind::Fluid)
        .into_iter()
        .find(|r| r.role == NavRole::Medium.word())
        .expect("the medium's row");
    assert!(
        row.badges
            .iter()
            .any(|b| b.ends_with("tube") || b.ends_with("tubes")),
        "{:?}",
        row.badges
    );
    dispatch(&mut h, "navigator.show", json!({"kind": "media"}));
    settle(&mut h);
    h.get_all_by_role(egui::accesskit::Role::Cell)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Medium Compressed air"))
        })
        .expect("the medium table's row")
        .click();
    settle(&mut h);
    // A medium set on a working line counts the cylinder it feeds.
    let counts: Vec<String> = h
        .get_all_by_role(egui::accesskit::Role::Cell)
        .filter_map(|n| n.accesskit_node().label().filter(|l| l.starts_with("Count ")))
        .collect();
    assert!(
        counts.len() == 1 && counts[0].starts_with("Count 1,"),
        "the medium's consumers: {counts:?}"
    );
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::H);
    settle(&mut h);
    let highlights = h.state().shell().highlights().items().len();
    assert_eq!(highlights, 1, "the zones are highlighted");
    assert!(!h.state().shell().highlight_overlay(0).is_empty(), "drawn on 203");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1733: on fluid page 203 a silencer placed facing a valve's free fluid port autoconnects to it, and the committed
/// connection is a fluid line, not a wire.
#[test]
fn a_silencer_autoconnects_to_a_free_fluid_port_with_a_fluid_line() {
    use ew_ui::ew_viewmodel::tools::Ghost;
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    let (free, step) = {
        let c = h.state().shell().canvas().unwrap();
        let t = &c.page().tables;
        let free: Vec<(i64, i64)> = t
            .cps
            .iter()
            .filter(|p| p.fluid && p.connections.is_empty() && p.dir.is_some())
            .map(|p| p.at)
            .collect();
        (free, t.grid.0 / t.grid.1.max(1))
    };
    assert!(!free.is_empty(), "Golden A's spare valves have free fluid ports");
    let before: usize = {
        let c = h.state().shell().canvas().unwrap();
        c.page()
            .tables
            .connections
            .iter()
            .filter(|c| c.kind == "fluid")
            .count()
    };
    dispatch(&mut h, "tool.place", json!({}));
    dispatch(
        &mut h,
        "place.pick",
        json!({"symbol": "ew.iso.silencer@qet10", "letter": "R"}),
    );
    settle(&mut h);
    let mut found = false;
    'search: for at in &free {
        for dy in (-10..=10).filter(|d| *d != 0) {
            for dx in -3..=3 {
                let p = (at.0 + dx * step, at.1 + dy * step);
                let (x, y) = {
                    let c = h.state().shell().canvas().unwrap();
                    let s = c.page().snapper.page_to_sheet(p);
                    (s.0 as f64 / 1e6, s.1 as f64 / 1e6)
                };
                dispatch(&mut h, "cursor.set", json!({"x_mm": x, "y_mm": y}));
                if matches!(h.state().shell().tool_ghost(), Some(Ghost::Place { hints, .. }) if hints > 0) {
                    found = true;
                    break 'search;
                }
            }
        }
    }
    assert!(found, "a place where the silencer faces a free fluid port");
    dispatch(&mut h, "canvas.activate", json!({}));
    settle(&mut h);
    let after: usize = {
        let c = h.state().shell().canvas().unwrap();
        c.page()
            .tables
            .connections
            .iter()
            .filter(|c| c.kind == "fluid")
            .count()
    };
    assert_eq!(after, before + 1, "the hint is a fluid line");
    h.state_mut().shell_mut().shutdown();
}

/// FT-1733: Draw ▸ Connect ▸ Fluid line… picks the line kind first; the Wire tool then draws a pilot line between
/// two free fluid ports of fluid page 203.
#[test]
fn the_fluid_line_picker_draws_a_pilot_line() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    support::open_menu(&mut h, &["Draw", "Connect"]);
    settle(&mut h);
    support::menu_node(&h, "Tube…").click();
    settle(&mut h);
    h.get_all_by_role(egui::Role::ComboBox)
        .last()
        .expect("the Line kind list")
        .click();
    settle(&mut h);
    h.get_by_label("Pilot").click();
    settle(&mut h);
    support::off_chrome(&h, "Draw").expect("Draw").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "wire");
    let free: Vec<(i64, i64)> = {
        let c = h.state().shell().canvas().unwrap();
        c.page()
            .tables
            .cps
            .iter()
            .filter(|p| p.fluid && p.connections.is_empty() && p.dir.is_some())
            .map(|p| p.at)
            .collect()
    };
    for at in &free[..2] {
        let (x, y) = {
            let c = h.state().shell().canvas().unwrap();
            let s = c.page().snapper.page_to_sheet(*at);
            (s.0 as f64 / 1e6, s.1 as f64 / 1e6)
        };
        dispatch(&mut h, "cursor.set", json!({"x_mm": x, "y_mm": y}));
        dispatch(&mut h, "canvas.activate", json!({}));
    }
    settle(&mut h);
    let pilot = h
        .state()
        .shell()
        .canvas()
        .unwrap()
        .page()
        .tables
        .connections
        .iter()
        .any(|c| c.kind == "fluid" && c.line_kind.as_deref() == Some("pilot"));
    assert!(pilot, "a pilot line is drawn");
    h.state_mut().shell_mut().shutdown();
}

/// Opens the row menu of the fluid navigator's row `label`, after narrowing the tree to it with the navigator's filter.
fn row_menu(h: &mut egui_kittest::Harness<'_, ew_ui::app::App>, label: &str) {
    h.get_all_by_role(egui::accesskit::Role::TextInput)
        .filter(|n| n.rect().left() < 600.0 && n.rect().top() > 100.0)
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the navigator's filter")
        .click();
    h.run_ok();
    h.key_press(egui::Key::End);
    for _ in 0..24 {
        h.key_press(egui::Key::Backspace);
    }
    h.event(egui::Event::Text(label.trim_start_matches('-').to_owned()));
    settle(h);
    let row = h
        .state()
        .shell()
        .shown_rows(NavKind::Fluid)
        .into_iter()
        .find(|r| r.label == label)
        .unwrap_or_else(|| panic!("the row {label}"));
    // The lines that also match stand above the valve terminals: scroll the tree down to the row.
    for _ in 0..12 {
        if h.query_by_label(&row.name).is_some() {
            break;
        }
        h.input_mut()
            .events
            .push(egui::Event::PointerMoved(egui::pos2(250.0, 400.0)));
        h.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        });
        settle(h);
    }
    h.get_by_label(&row.name).click_secondary();
    settle(h);
}

/// The labels of the open menu's items that start with `prefix`.
fn items(h: &egui_kittest::Harness<'_, ew_ui::app::App>, prefix: &str) -> Vec<String> {
    h.query_all_by_label_contains(prefix)
        .filter_map(|n| n.accesskit_node().label())
        .filter(|l| l.starts_with(prefix))
        .collect()
}

/// The device navigator's fluid actions on a fluid navigator's rows (doc 11 §7, FT-1733): a valve is set spare and its
/// position blanked; the module's row then adds a valve of the terminal's part at that position, whose working ports
/// are open and plugged from its own row; each one undo step.
#[test]
fn valves_ports_and_positions_are_set_from_the_fluid_navigator() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "view.workspace_load", json!({"name": "Fluid"}));
    settle(&mut h);
    row_menu(&mut h, "-201U3-Q1");
    support::menu_node(&h, "Set spare").click();
    settle(&mut h);
    row_menu(&mut h, "-201U3-Q1");
    assert!(!items(&h, "Set used").is_empty(), "the valve is spare now");
    support::menu_node(&h, "Blank position 1").click();
    settle(&mut h);
    row_menu(&mut h, "-201U3");
    support::menu_node(&h, "Add valve at position 1").click();
    settle(&mut h);
    row_menu(&mut h, "-201U3-Q1");
    let plugs = items(&h, "Plug port ");
    assert_eq!(
        plugs.len(),
        2,
        "the new valve's working ports are open: {plugs:?}"
    );
    support::menu_node(&h, &plugs[0]).click();
    settle(&mut h);
    row_menu(&mut h, "-201U3-Q1");
    assert_eq!(items(&h, "Plug port ").len(), 1, "one port plugged");
    h.state_mut().shell_mut().shutdown();
}

/// The overview page generator (FR-FLU-033, doc 11 §5.5): a module added to Golden A's valve terminal in the rack and
/// slot editor has no overview yet; the terminal's row in the fluid navigator offers Generate overview…, whose preview
/// names each page's members; Apply draws the new module with the starter overview module beside the others.
#[test]
fn the_valve_terminals_overview_is_generated_from_its_row() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "201");
    dispatch(&mut h, "plc.rack_editor", json!({"station": "tag:-201U1"}));
    settle(&mut h);
    let station = match h.state().shell().dialog() {
        Some(ew_ui::ew_viewmodel::shell::Dialog::Rack { station, .. }) => *station,
        other => panic!("{other:?}"),
    };
    let before = h.state().shell().rack_view(station).expect("the rack");
    let free = before
        .rows
        .iter()
        .find(|r| r.module.is_none())
        .expect("a free slot")
        .slot;
    dispatch(&mut h, "plc.rack_add", json!({"slot": free}));
    dispatch(&mut h, "ui.dismiss", json!({}));
    settle(&mut h);
    let added = h
        .state()
        .shell()
        .rack_view(station)
        .and_then(|v| {
            v.rows
                .into_iter()
                .find(|r| r.module.is_some() && !before.rows.iter().any(|b| b.module == r.module))
        })
        .expect("the new module");
    dispatch(&mut h, "view.workspace_load", json!({"name": "Fluid"}));
    settle(&mut h);
    let row = h
        .state()
        .shell()
        .shown_rows(NavKind::Fluid)
        .into_iter()
        .find(|r| r.label == "-201U1")
        .expect("the terminal's row");
    // The navigator's filter (the text field at the left column's top) narrows the tree to the terminal.
    h.get_all_by_role(egui::accesskit::Role::TextInput)
        .filter(|n| n.rect().left() < 600.0 && n.rect().top() > 100.0)
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the navigator's filter")
        .click();
    h.run_ok();
    h.event(egui::Event::Text("201U1".to_owned()));
    settle(&mut h);
    h.get_by_label(&row.name).click_secondary();
    settle(&mut h);
    support::menu_node(&h, "Generate overview…").click();
    settle(&mut h);
    let notices = h.state().shell().preview_pane().notices().to_vec();
    assert!(
        notices.iter().any(|n| n.starts_with("page 201: -201U1, ")),
        "{notices:?}"
    );
    assert!(notices.iter().any(|n| n.contains(&added.tag)), "{notices:?}");
    support::off_chrome(&h, "Apply")
        .expect("the card's Apply")
        .click();
    settle(&mut h);
    let page = notices
        .iter()
        .find(|n| n.contains(&added.tag))
        .and_then(|n| n.strip_prefix("page "))
        .and_then(|n| n.split([':', ' ']).next())
        .unwrap()
        .to_owned();
    dispatch(&mut h, "page.goto", json!({"folio": page}));
    settle(&mut h);
    let drawn = h.state().shell().canvas().is_some_and(|c| {
        c.page().tables.reps.iter().any(|r| {
            r.rep_type == "overview" && r.device == added.module && r.symbol_key.starts_with("ew.ovw.module")
        })
    });
    assert!(drawn, "the new module drawn on {page}");
    h.state_mut().shell_mut().shutdown();
}

/// Doc 11 §11, AC-32: Generate overview… on Golden A's valve terminal, whose overviews 201 and 202 are drawn in member
/// order, keeps every member where it stands: the preview names -201U1 … -201U9 on 201 and -202U1 … -202U3 on 202, and
/// changes no record.
#[test]
fn golden_as_overview_is_refreshed_without_a_change() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "201");
    dispatch(
        &mut h,
        "fluid.generate_overview",
        json!({"station": "tag:-201U1"}),
    );
    settle(&mut h);
    let pane = h.state().shell().preview_pane();
    let notices = pane.notices().to_vec();
    let on = |folio: &str| {
        notices
            .iter()
            .find_map(|n| n.strip_prefix(&format!("page {folio}: ")))
            .map(|m| m.split(", ").map(str::to_owned).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let first = on("201");
    assert_eq!(first.first().map(String::as_str), Some("-201U1"), "{notices:?}");
    assert_eq!(first.last().map(String::as_str), Some("-201U9"), "{notices:?}");
    assert_eq!(on("202"), ["-202U1", "-202U2", "-202U3"], "{notices:?}");
    assert!(pane.entities().is_empty(), "{:?}", pane.entities());
    h.state_mut().shell_mut().shutdown();
}

/// FT-1757: on a fluid page (Golden A's 203) the interruption-point tool places the fluid line's form `ew.iso.ip`, not
/// the wire's.
#[test]
fn the_interruption_tool_places_the_fluid_form_on_a_fluid_page() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    let forms = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        h.state()
            .shell()
            .canvas()
            .unwrap()
            .page()
            .tables
            .reps
            .iter()
            .filter(|r| r.ip.is_some())
            .map(|r| r.symbol_key.clone())
            .collect::<Vec<_>>()
    };
    let before = forms(&h).len();
    dispatch(&mut h, "tool.ip", json!({}));
    crate::navigators::cursor_k(&mut h, (60, 6));
    dispatch(&mut h, "canvas.activate_alt", json!({}));
    dispatch(&mut h, "tool.select", json!({}));
    settle(&mut h);
    let after = forms(&h);
    assert_eq!(after.len(), before + 1, "{after:?}");
    assert!(after.iter().all(|k| k.starts_with("ew.iso.ip")), "{after:?}");
    h.state_mut().shell_mut().shutdown();
}

/// No English in a Dutch run of the fluid workspace: the valve terminal's role badge, the tube table's port roles,
/// line kinds and length sources are Dutch, and its colour column is a tube's colour, not a core's.
#[test]
fn the_fluid_workspace_speaks_dutch() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "203");
    dispatch(&mut h, "view.language", json!({ "value": "nl" }));
    dispatch(&mut h, "rail.show", json!({"pane": "plc"}));
    dispatch(&mut h, "navigator.show", json!({"kind": "tubes"}));
    settle(&mut h);
    let rows = h.state().shell().shown_rows(NavKind::Plc);
    let terminal = rows
        .iter()
        .find(|r| r.role == NavRole::Station.word())
        .expect("the valve terminal");
    assert!(
        terminal.badges.iter().any(|b| b == "ventieleiland"),
        "{:?}",
        terminal.badges
    );
    let table = h
        .state()
        .shell()
        .nav(NavKind::Tubes)
        .and_then(|n| n.table())
        .cloned()
        .expect("the tube table");
    let column = |key: &str| table.columns.iter().position(|c| c.key == key).expect(key);
    assert_eq!(table.columns[column("colour")].label, "Kleur");
    let first = &table.nodes[0].cells;
    assert_eq!(first[column("line_kind")], "werkleiding", "{first:?}");
    assert_eq!(first[column("from_role")], "werkpoort", "{first:?}");
    assert_eq!(first[column("length_source")], "ontbreekt", "{first:?}");
    h.state_mut().shell_mut().shutdown();
}
