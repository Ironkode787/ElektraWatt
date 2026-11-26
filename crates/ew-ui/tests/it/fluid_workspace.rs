// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The Fluid workspace in the window (FR-UI-003, FR-UI-006, doc 11 §7), at 1920 × 1080: View ▸ Workspaces ▸ Fluid power
//! shows the fluid navigator with the sample's valve terminal and its valves with their lines, and the tube
//! and medium tables in the bottom dock; a tube's length typed into its cell becomes manual; New medium… adds a medium,
//! a fluid line's context menu sets it and picks the line's kind, and the medium's row highlights its zones. Not here:
//! the silencer's autoconnect, the interruption point's fluid form, an overview generated again on page 201 and the
//! panels' Dutch words (`ew-viewmodel`'s `fluid_page.rs`).

use crate::support;

use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_viewmodel::layout::PaneKind;
use ew_viewmodel::navigators::{NavKind, NavRole};
use serde_json::json;
use support::{Tmp, dispatch, on_sample, settle};

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
    let mut h = on_sample(&tmp, "201");
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
        .find(|r| r.label == "-201K1")
        .expect("the valve terminal");
    // FR-UI-006: the four valves the sample's script adds to the terminal's four positions.
    let valves: Vec<&str> = rows
        .iter()
        .filter(|r| r.parent.as_deref() == Some(terminal.key.as_str()) && r.role == NavRole::Device.word())
        .map(|r| r.label.as_str())
        .collect();
    assert_eq!(valves, ["-201K1-Q1", "-201K1-Q2", "-201K1-Q3", "-201K1-Q4"]);
    assert!(
        rows.iter()
            .any(|r| r.role == NavRole::Connection.word() && r.label.starts_with("→ -201M1:")),
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
        h.query_all_by_label_contains("Source typed").count() >= 1,
        "the length's source is typed"
    );
    h.state_mut().shell_mut().shutdown();
}

/// A net whose medium is defined at its other end: the line's context menu checks that medium, and Medium: of another
/// one replaces the net's definition rather than adding a second one beside it, so the net carries the new medium
/// alone (not "mixed") and the menu checks it.
#[test]
fn a_medium_set_on_a_line_replaces_the_definition_of_its_net() {
    use ew_viewmodel::ew_api::output::TableEnd;
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "201");
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
            .expect("a fluid line on 201");
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

/// Media from nothing on the sample, which defines none: Fluid ▸ New medium… adds one; a fluid line's context menu on
/// folio 201 sets it on the line's net and sets the line's kind; the medium table's row counts the consumers and
/// highlights the zones.
#[test]
fn a_medium_is_added_set_on_a_line_and_highlighted() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "201");
    support::open_menu(&mut h, &["Fluid"]);
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
            .expect("a fluid line on 201")
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
    // A flexible line (a hose) is drawn as an arc.
    let curved = |h: &egui_kittest::Harness<'_, ew_ui::app::App>| {
        use ew_dl::{ItemKind, PathEl};
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
            // Its consumers, as the medium table counts them.
            .any(|b| b.ends_with("consumer") || b.ends_with("consumers")),
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
    assert!(!h.state().shell().highlight_overlay(0).is_empty(), "drawn on 201");
    h.state_mut().shell_mut().shutdown();
}

/// Fluid ▸ Tube draws at once; the HUD's Line kind steps to Pilot, and the Wire tool then draws a
/// pilot line between two free fluid ports of fluid page 201.
#[test]
fn the_tube_tools_line_kind_draws_a_pilot_line() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "201");
    support::open_menu(&mut h, &["Fluid"]);
    settle(&mut h);
    support::menu_node(&h, "Tube").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "wire", "no dialog first");
    h.get_by_label("Line kind: next").click();
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

/// The device navigator's fluid actions on a fluid navigator's rows (doc 11 §7): a valve is set spare and its
/// position blanked; the terminal's row then adds a valve of the terminal's part at that position, whose working ports
/// are open and plugged from its own row; each one undo step.
#[test]
fn valves_ports_and_positions_are_set_from_the_fluid_navigator() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "201");
    dispatch(&mut h, "view.workspace_load", json!({"name": "Fluid"}));
    settle(&mut h);
    row_menu(&mut h, "-201K1-Q1");
    support::menu_node(&h, "Set spare").click();
    settle(&mut h);
    row_menu(&mut h, "-201K1-Q1");
    assert!(!items(&h, "Set used").is_empty(), "the valve is spare now");
    support::menu_node(&h, "Blank position 1").click();
    settle(&mut h);
    row_menu(&mut h, "-201K1");
    support::menu_node(&h, "Add valve at position 1").click();
    settle(&mut h);
    row_menu(&mut h, "-201K1-Q1");
    let plugs = items(&h, "Plug port ");
    assert_eq!(
        plugs.len(),
        2,
        "the new valve's working ports are open: {plugs:?}"
    );
    support::menu_node(&h, &plugs[0]).click();
    settle(&mut h);
    row_menu(&mut h, "-201K1-Q1");
    assert_eq!(items(&h, "Plug port ").len(), 1, "one port plugged");
    h.state_mut().shell_mut().shutdown();
}

/// The members Generate overview… previews on each page, by folio, from the Preview pane's notices
/// (`page <folio>: <tag>, <tag>, …`; the mark of a page it adds, `<folio> (new page)`, left out).
fn overview_pages(h: &egui_kittest::Harness<'_, ew_ui::app::App>) -> Vec<(String, Vec<String>)> {
    h.state()
        .shell()
        .preview_pane()
        .notices()
        .iter()
        .filter_map(|n| n.words.strip_prefix("page "))
        .filter_map(|n| n.split_once(": "))
        .map(|(folio, members)| {
            (
                folio.split(' ').next().unwrap_or_default().to_owned(),
                members.split(", ").map(str::to_owned).collect(),
            )
        })
        .collect()
}

/// The overview page generator (FR-FLU-033, doc 11 §5.5): the sample's valve terminal has no overview yet; its row in
/// the fluid navigator offers Generate overview…, whose preview names each page's members, the terminal first; Apply
/// draws them with the starter overview symbols.
#[test]
fn the_valve_terminals_overview_is_generated_from_its_row() {
    let tmp = Tmp::new();
    let mut h = on_sample(&tmp, "201");
    dispatch(&mut h, "view.workspace_load", json!({"name": "Fluid"}));
    settle(&mut h);
    // The row selected first is scrolled into view: it stands below every line the filter of `row_menu` matches.
    let key = h
        .state()
        .shell()
        .shown_rows(NavKind::Fluid)
        .into_iter()
        .find(|r| r.label == "-201K1")
        .expect("the terminal's row")
        .key;
    dispatch(
        &mut h,
        "navigator.select",
        json!({"kind": "fluid", "keys": [key]}),
    );
    settle(&mut h);
    row_menu(&mut h, "-201K1");
    support::menu_node(&h, "Generate overview…").click();
    settle(&mut h);
    let pages = overview_pages(&h);
    let (page, members) = pages.first().cloned().expect("a page of the overview");
    assert_eq!(members.first().map(String::as_str), Some("-201K1"), "{pages:?}");
    support::off_chrome(&h, "Apply")
        .expect("the card's Apply")
        .click();
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": page}));
    settle(&mut h);
    let drawn = h.state().shell().canvas().is_some_and(|c| {
        c.page()
            .tables
            .reps
            .iter()
            .any(|r| r.rep_type == "overview" && r.symbol_key.starts_with("ew.ovw."))
    });
    assert!(drawn, "the overview drawn on {page}");
    h.state_mut().shell_mut().shutdown();
}
