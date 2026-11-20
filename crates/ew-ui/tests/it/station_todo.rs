// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The station view's To do run to its end on every station kind, by the window's own path at 1920 × 1080:
//! each line's button clicked, a page chosen where it asks, the box put down where it hangs on the pointer. Every
//! side of a station (its boxes on an electrical page, its bus ports on a bus page, a valve terminal's valves on a
//! fluid page) lands on a page of its kind, and no step ends in a refusal.

use crate::relays_paste_and_ip_wires::{click, key, new_project};
use crate::support::Tmp;

use ew_ui::visual::{Driver, Input};
use ew_viewmodel::ew_api::output::Uuid;
use ew_viewmodel::shell::Outcome;
use ew_viewmodel::shell::station::TodoFix;
use serde_json::json;

/// A click at `at` after the pointer rested there, so that a tool's ghost follows it first.
fn click_at(d: &mut Driver, at: (i64, i64)) {
    d.input(&Input::Hover { at });
    d.settle();
    d.input(&click(at));
    d.settle();
}

/// The steps of the station view on show that draw a side of the station: their lines' words.
fn sides(d: &mut Driver) -> Vec<String> {
    d.shell_mut()
        .station_todo()
        .into_iter()
        .filter(|t| matches!(t.fix, Some(TodoFix::OnPage { .. })))
        .map(|t| t.text)
        .collect()
}

/// Runs the To do of the view of `station` from its first line on, as a person does: the line's button, the first
/// page of its list (else New page…), the box put down where a click puts it (`placed` counts the places taken on the
/// pages, so that no box lands on another); until no line is left that draws a side. Each step does its thing: its
/// line goes, without a refusal under the rack and without a failure in the window.
fn run_todo(d: &mut Driver, station: Uuid, placed: &mut i64) {
    let mut last = None;
    for _ in 0..8 {
        let rack = json!({"station": station.to_string()});
        assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
        d.input(&Input::Hover { at: (1000, 600) });
        d.settle();
        let todo = d.shell_mut().station_todo();
        let Some(item) = todo
            .iter()
            .find(|t| matches!(t.fix, Some(TodoFix::OnPage { .. })))
        else {
            return;
        };
        assert_ne!(last.as_ref(), Some(&item.text), "the step before left its line");
        last = Some(item.text.clone());
        let Some(TodoFix::OnPage {
            label, folio, pages, ..
        }) = &item.fix
        else {
            unreachable!()
        };
        let button = d
            .find(label)
            .unwrap_or_else(|| panic!("the button {label} of {:?}", item.text));
        d.input(&click(button));
        d.settle();
        if folio.is_none() {
            let choice = pages.first().map_or_else(
                || "New page…".to_owned(),
                |(f, t)| format!("{f} {t}").trim().to_owned(),
            );
            // The list lies over the window: its entry is the last widget of that name (a page's row has it too).
            let at = d
                .labels()
                .into_iter()
                .rev()
                .find(|(l, _)| *l == choice)
                .map(|(_, at)| at)
                .unwrap_or_else(|| panic!("{choice} in the list of {label}"));
            d.input(&click(at));
            d.settle();
        }
        let shell = d.shell_mut();
        if let Some(tab) = shell.station_tab() {
            assert_eq!(tab.error, None, "{:?} refused in the view", item.text);
        }
        if shell.tool().name() != "select" {
            // One click, or the two corners of a bus port's box; each box a place of its own.
            let at = (650 + 260 * (*placed % 4), 300 + 200 * (*placed / 4));
            *placed += 1;
            let revision = d.shell_mut().revision();
            click_at(d, at);
            if d.shell_mut().revision() == revision {
                click_at(d, (at.0 + 80, at.1 + 60));
            }
            d.input(&key("escape"));
            d.settle();
        }
        let toast = d.shell_mut().toast().map(|t| t.text.clone()).unwrap_or_default();
        assert!(!toast.contains("No longer in the project"), "{toast}");
        assert!(d.shell_mut().banner().is_none(), "{:?}", d.shell_mut().banner());
    }
    panic!("To do does not end: {:?}", sides(d));
}

/// A station of kind `kind` with head part `head` and the modules or valves `parts` made in its view: the station.
fn station(d: &mut Driver, kind: &str, head: &str, parts: &[&str]) -> Uuid {
    let _ = d.shell_mut().dispatch("plc.new_station", &json!({"kind": kind}));
    let set = json!({"head": head});
    assert_eq!(d.shell_mut().dispatch("plc.rack_set", &set), Outcome::Done);
    assert_eq!(
        d.shell_mut().dispatch("plc.rack_create", &json!({})),
        Outcome::Done
    );
    for part in parts {
        let add = json!({"part": part});
        assert_eq!(d.shell_mut().dispatch("plc.rack_add", &add), Outcome::Done);
    }
    d.shell_mut()
        .station_tab()
        .and_then(|t| t.station)
        .expect("the station's view")
}

/// H2: on a project with a CPU and a card, a remote I/O station with a card, both valve terminals (with a PROFIBUS and
/// with a PROFINET node, valves on their positions) and a bus device of one's own given to a PLC, every line of To do
/// does its thing: each side lands on a page of its kind (made when the project has none, chosen from the list when
/// it has), the valve terminal stands on the fluid page as its positions with its valves in their cells, and To do
/// ends with no side left to draw.
#[test]
fn every_station_kinds_todo_runs_to_its_end() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "101", "type": "multi_line", "title": "Control"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let mut placed = 0;
    let cpu = station(&mut d, "cpu", "EW-GEN-CPU-2", &["EW-GEN-DI-1"]);
    run_todo(&mut d, cpu, &mut placed);
    let remote = station(&mut d, "remote", "EW-GEN-RIO-1", &["EW-GEN-DO-1"]);
    run_todo(&mut d, remote, &mut placed);
    // The owner's order: the terminal's box on the electrical page and its bus port on the bus page first, its
    // pneumatic side last; a fluid page of one's own waits for it.
    let fluid = json!({"folio": "201", "type": "fluid", "title": "Pneumatics"});
    assert_eq!(d.shell_mut().dispatch("page.new", &fluid), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    let dp = station(
        &mut d,
        "valve_terminal",
        "EW-GEN-VTM-1",
        &["EW-GEN-VLV-3", "EW-GEN-VLV-3"],
    );
    run_todo(&mut d, dp, &mut placed);
    let view = d.shell_mut().rack_view(dp).expect("the terminal");
    let pages: Vec<Option<String>> = view.cells.iter().map(|c| c.at.page.clone()).collect();
    assert_eq!(
        &pages[..2],
        &[Some("201".to_owned()), Some("201".to_owned())],
        "its valves in their cells on the fluid page"
    );
    // From the fluid page its pneumatic side goes there at once, before the rest: then its electrical side is
    // another drawing of the box it stands in there, on the page chosen.
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "201"}));
    let pn = station(&mut d, "valve_terminal", "EW-GEN-VTM-2", &["EW-GEN-VLV-3"]);
    d.input(&Input::Hover { at: (1000, 600) });
    d.settle();
    assert!(
        d.find("Draw on an electrical page…").is_some(),
        "{:?}",
        sides(&mut d)
    );
    d.input(&click(
        d.find("Draw on page 201").expect("the pneumatic side's button"),
    ));
    d.settle();
    click_at(&mut d, (650, 900));
    d.input(&key("escape"));
    d.settle();
    let cells = d.shell_mut().rack_view(pn).expect("the terminal").cells;
    assert_eq!(cells[0].at.page.as_deref(), Some("201"), "its valve in its cell");
    run_todo(&mut d, pn, &mut placed);
    // A bus device of one's own, drawn on the electrical page and given to a PLC from a new station's view.
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    let _ = d
        .shell_mut()
        .dispatch("tool.black_box", &json!({"part": "EW-GEN-PND-1", "template": 0}));
    click_at(&mut d, (900, 800));
    click_at(&mut d, (1000, 900));
    d.input(&key("escape"));
    d.settle();
    let _ = d
        .shell_mut()
        .dispatch("plc.new_station", &json!({"kind": "remote"}));
    let (device, _) = d.shell_mut().station_candidates()[0].clone();
    let add = json!({"device": device.to_string()});
    assert_eq!(d.shell_mut().dispatch("plc.add_to_plc", &add), Outcome::Done);
    run_todo(&mut d, device, &mut placed);
    d.shell_mut().shutdown();
}

/// H2 of a test, the owner's refusal: a valve terminal's box armed by To do's Draw on a fluid page, then Undo
/// (Ctrl+Z) of the step that drew its electrical side, then the click: the box named a symbol copy the Undo had taken
/// out, and the window said "An item the command names is no longer in the project". An Undo or a Redo now ends the
/// placement it may have left without its records; To do, read from the project as the step left it, asks again and
/// arms it anew. With the view open the whole time, every line of To do is pressed after undo and redo to its end.
#[test]
fn undo_and_redo_under_the_open_view_leave_no_step_without_its_records() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    for (folio, kind) in [("101", "multi_line"), ("201", "fluid")] {
        let page = json!({"folio": folio, "type": kind, "title": ""});
        assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    }
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let terminal = station(&mut d, "valve_terminal", "EW-GEN-VTM-1", &["EW-GEN-VLV-3"]);
    let rack = json!({"station": terminal.to_string()});
    let draw = json!({"step": "draw", "folio": "101"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_next", &draw), Outcome::Done);
    assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
    let fluid = json!({"step": "fluid", "folio": "201"});
    assert_eq!(d.shell_mut().dispatch("plc.rack_next", &fluid), Outcome::Done);
    assert_eq!(d.shell_mut().tool().name(), "black_box", "the box on the pointer");
    // The first Ctrl+Z puts the box away and undoes nothing, and the status line says so.
    let revision = d.shell_mut().revision();
    d.input(&key("ctrl+z"));
    d.settle();
    assert_eq!(d.shell_mut().tool().name(), "select", "Undo puts the box away");
    assert_eq!(d.shell_mut().revision(), revision, "and undoes nothing");
    assert!(
        d.shell_mut()
            .tool_notice()
            .is_some_and(|n| n.starts_with("Undo put away what was on the pointer")),
        "{:?}",
        d.shell_mut().tool_notice()
    );
    // The next takes back the electrical side, drawn on another page: the toast says what and where, and goes there.
    d.input(&key("ctrl+z"));
    d.settle();
    let toast = d.shell_mut().toast().cloned().expect("what Undo took back");
    assert!(
        toast.text.starts_with("Undone: ") && toast.text.contains("101"),
        "{}",
        toast.text
    );
    assert_eq!(
        toast.action(),
        Some(ew_viewmodel::notifications::NoteAction::GoTo)
    );
    click_at(&mut d, (800, 400));
    assert!(d.shell_mut().banner().is_none(), "{:?}", d.shell_mut().banner());
    // To do reads the project as the Undo left it: the electrical side is to draw again.
    assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
    assert!(
        sides(&mut d)[0].starts_with("The electrical side of"),
        "{:?}",
        sides(&mut d)
    );
    d.input(&key("ctrl+shift+z"));
    d.settle();
    assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
    assert!(
        sides(&mut d)[0].starts_with("The bus side of"),
        "redone: {:?}",
        sides(&mut d)
    );
    let mut placed = 0;
    run_todo(&mut d, terminal, &mut placed);
    let cells = d.shell_mut().rack_view(terminal).expect("the terminal").cells;
    assert_eq!(cells[0].at.page.as_deref(), Some("201"), "its valve in its cell");
    // Undone twice and redone under the open view, To do runs to its end again.
    assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
    for chord in ["ctrl+z", "ctrl+z", "ctrl+shift+z"] {
        d.input(&key(chord));
        d.settle();
    }
    run_todo(&mut d, terminal, &mut placed);
    d.shell_mut().shutdown();
}

/// The PLC panel's row of PLC part `device` (its station's or its card's), as its right-click reads it.
fn plc_row(d: &mut Driver, device: Uuid) -> ew_viewmodel::ew_api::output::NavNode {
    use ew_viewmodel::navigators::{NavKind, NavRole};
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "plc"}));
    d.settle();
    let _ = d.shell_mut().shown_rows(NavKind::Plc);
    d.shell_mut()
        .nav(NavKind::Plc)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| {
                    matches!(n.role, NavRole::Station | NavRole::Module) && n.records.first() == Some(&device)
                })
                .cloned()
        })
        .expect("the part's row")
}

/// The Place here entry of `device`'s row in the PLC panel.
fn place_here(d: &mut Driver, device: Uuid) -> ew_viewmodel::actions::ActionItem {
    let node = plc_row(d, device);
    let menu = ew_viewmodel::command_bar::navigator_row_menu(
        d.shell_mut(),
        ew_viewmodel::navigators::NavKind::Plc,
        &node,
    );
    menu.into_iter()
        .find(|i| i.label == "Place here")
        .unwrap_or_else(|| panic!("Place here on {}", node.label))
}

/// H13 of a test: a PLC part that no page draws yet offers Place here on its row in the PLC panel. On a bus
/// page a card's entry is greyed with the page to open; on an electrical page the station's box hangs on the pointer
/// as after choosing it in the device browser, and two corners draw it there. The same entry stands in the station
/// view's menu of a card.
#[test]
fn place_here_puts_an_unplaced_plc_part_on_the_pointer() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "101", "type": "multi_line", "title": "Control"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let bus = json!({"folio": "300", "type": "bus_topology", "title": "Bus"});
    assert_eq!(d.shell_mut().dispatch("page.new", &bus), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "300"}));
    let cpu = station(&mut d, "cpu", "EW-GEN-CPU-2", &["EW-GEN-DI-1"]);
    let card = d.shell_mut().rack_view(cpu).expect("the rack").rows[0]
        .module
        .expect("the card");
    let _ = d.shell_mut().dispatch("ui.dismiss", &json!({}));
    d.settle();
    let greyed = place_here(&mut d, card);
    assert!(!greyed.enabled, "a bus page takes no card's box");
    assert_eq!(
        greyed.reason.as_deref(),
        Some("Open an electrical page: its box is placed there")
    );
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let place = place_here(&mut d, cpu);
    assert!(place.enabled, "{:?}", place.reason);
    assert_eq!(d.shell_mut().dispatch(place.id, &place.params), Outcome::Done);
    assert_eq!(d.shell_mut().tool().name(), "black_box", "its box on the pointer");
    click_at(&mut d, (700, 400));
    click_at(&mut d, (820, 500));
    // The part is drawn: the tool gives way to Select, so a second click draws nothing and is refused nowhere.
    assert_eq!(d.shell_mut().tool().name(), "select", "one part, one box");
    let head = d.shell_mut().rack_view(cpu).expect("the rack").head;
    assert_eq!(head.page.as_deref(), Some("101"), "drawn on the page shown");
    // The card's menu in the station view offers it too.
    let rack = json!({"station": cpu.to_string()});
    assert_eq!(d.shell_mut().dispatch("plc.rack_editor", &rack), Outcome::Done);
    let select = json!({"card": card.to_string()});
    assert_eq!(d.shell_mut().dispatch("plc.rack_select", &select), Outcome::Done);
    let tools = ew_viewmodel::actions::station_tools(d.shell_mut());
    let entry = tools.iter().find_map(|(_, e)| match e {
        ew_viewmodel::actions::MenuEntry::Item(i) if i.id == "plc.module_place" => Some(i.label.clone()),
        _ => None,
    });
    assert_eq!(entry.as_deref(), Some("Place here"));
    d.shell_mut().shutdown();
}

/// A Delete in a navigator of the part a box hangs on the pointer for (Place here) ends the placement, as an Undo that
/// takes it away does: the click that follows draws nothing and names nothing gone.
#[test]
fn a_delete_in_a_navigator_ends_the_box_of_what_it_deleted() {
    use ew_viewmodel::navigators::NavKind;
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "101", "type": "multi_line", "title": "Control"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let cpu = station(&mut d, "cpu", "EW-GEN-CPU-2", &[]);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    d.settle();
    let place = place_here(&mut d, cpu);
    assert_eq!(d.shell_mut().dispatch(place.id, &place.params), Outcome::Done);
    assert_eq!(d.shell_mut().tool().name(), "black_box", "its box on the pointer");
    let node = plc_row(&mut d, cpu);
    let kind = NavKind::Plc.word();
    let row = json!({"kind": kind, "key": node.key});
    assert_eq!(d.shell_mut().dispatch("navigator.row", &row), Outcome::Done);
    let revision = d.shell_mut().revision();
    let _ = d.shell_mut().dispatch("navigator.delete", &json!({"kind": kind}));
    if d.shell_mut().revision() == revision {
        assert_eq!(d.shell_mut().dispatch("preview.apply", &json!({})), Outcome::Done);
    }
    assert_ne!(d.shell_mut().revision(), revision, "the station is deleted");
    assert_eq!(
        d.shell_mut().tool().name(),
        "select",
        "the placement ends with it"
    );
    d.shell_mut().shutdown();
}

/// The Place here entry of the first PLC panel row of role `role` whose records satisfy `of`, with the row's badges.
fn place_on_row(
    d: &mut Driver,
    role: ew_viewmodel::navigators::NavRole,
    of: impl Fn(&[Uuid]) -> bool,
) -> (Option<ew_viewmodel::actions::ActionItem>, Vec<String>) {
    use ew_viewmodel::navigators::NavKind;
    let _ = d.shell_mut().dispatch("rail.show", &json!({"pane": "plc"}));
    d.settle();
    let node = d
        .shell_mut()
        .nav(NavKind::Plc)
        .and_then(|n| n.table())
        .and_then(|t| t.nodes.iter().find(|n| n.role == role && of(&n.records)).cloned())
        .unwrap_or_else(|| panic!("a {role:?} row"));
    let menu = ew_viewmodel::command_bar::navigator_row_menu(d.shell_mut(), NavKind::Plc, &node);
    let place = menu.into_iter().find(|i| i.id == "plc.module_place");
    (place, node.badges.clone())
}

/// A persona run: Place here was hard to find. A channel of a card no page draws offers its card's Place here, and
/// its row says "not drawn"; the entry puts the card's box on the pointer.
#[test]
fn a_channel_of_an_undrawn_card_offers_place_here() {
    use ew_viewmodel::navigators::NavRole;
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "101", "type": "multi_line", "title": "Control"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    let cpu = station(&mut d, "cpu", "EW-GEN-CPU-2", &["EW-GEN-DI-1"]);
    let card = d.shell_mut().rack_view(cpu).expect("the rack").rows[0]
        .module
        .expect("the card");
    let _ = d.shell_mut().dispatch("ui.dismiss", &json!({}));
    d.settle();
    let (place, badges) = place_on_row(&mut d, NavRole::Channel, |r| r.get(1) == Some(&card));
    let place = place.expect("Place here on the channel's row");
    assert!(place.enabled, "{:?}", place.reason);
    assert_eq!(place.params["device"], json!(card.to_string()), "its card's box");
    assert!(badges.iter().any(|b| b == "unplaced"), "{badges:?}");
    assert_eq!(d.shell_mut().dispatch(place.id, &place.params), Outcome::Done);
    assert_eq!(
        d.shell_mut().tool().name(),
        "black_box",
        "its card's box on the pointer"
    );
    d.shell_mut().shutdown();
}

/// A valve terminal whose bus side stands on a bus page and whose electrical side is drawn after it (or drawn
/// again after an Undo) takes its tag from the electrical page: its box is its main drawing, the bus port a second one.
#[test]
fn a_box_drawn_after_the_bus_port_numbers_the_tag_by_its_page() {
    let tmp = Tmp::new();
    let mut d = new_project(&tmp);
    let page = json!({"folio": "101", "type": "multi_line", "title": "Control"});
    assert_eq!(d.shell_mut().dispatch("page.new", &page), Outcome::Done);
    let bus = json!({"folio": "300", "type": "bus_topology", "title": "Bus"});
    assert_eq!(d.shell_mut().dispatch("page.new", &bus), Outcome::Done);
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "300"}));
    let terminal = station(&mut d, "valve_terminal", "EW-GEN-VTM-1", &["EW-GEN-VLV-3"]);
    let _ = d.shell_mut().dispatch("ui.dismiss", &json!({}));
    d.settle();
    for (folio, at) in [("300", (700, 400)), ("101", (900, 500))] {
        let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": folio}));
        d.settle();
        let place = place_here(&mut d, terminal);
        assert!(place.enabled, "{folio}: {:?}", place.reason);
        assert_eq!(d.shell_mut().dispatch(place.id, &place.params), Outcome::Done);
        let revision = d.shell_mut().revision();
        click_at(&mut d, at);
        if d.shell_mut().revision() == revision {
            click_at(&mut d, (at.0 + 120, at.1 + 100));
        }
        assert_ne!(d.shell_mut().revision(), revision, "drawn on {folio}");
        d.input(&key("escape"));
        d.settle();
    }
    let tag = d.shell_mut().rack_view(terminal).expect("the terminal").tag;
    assert!(tag.starts_with("-101"), "{tag}");
    d.shell_mut().shutdown();
}
