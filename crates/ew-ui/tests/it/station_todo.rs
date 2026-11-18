// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The station view's To do run to its end on every station kind (rc.5, H2), by the window's own path at 1920 × 1080:
//! each line's button clicked, a page chosen where it asks, the box put down where it hangs on the pointer. Every
//! side of a station (its boxes on an electrical page, its bus ports on a bus page, a valve terminal's valves on a
//! fluid page) lands on a page of its kind, and no step ends in a refusal.

use crate::persona_round::{click, key, new_project};
use crate::support::Tmp;

use ew_ui::ew_viewmodel::ew_api::output::Uuid;
use ew_ui::ew_viewmodel::shell::Outcome;
use ew_ui::ew_viewmodel::shell::automation::TodoFix;
use ew_ui::visual::{Driver, Input};
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
        let Some(item) = todo.iter().find(|t| matches!(t.fix, Some(TodoFix::OnPage { .. }))) else {
            return;
        };
        assert_ne!(last.as_ref(), Some(&item.text), "the step before left its line");
        last = Some(item.text.clone());
        let Some(TodoFix::OnPage { label, folio, pages, .. }) = &item.fix else {
            unreachable!()
        };
        let button = d
            .find(label)
            .unwrap_or_else(|| panic!("the button {label} of {:?}", item.text));
        d.input(&click(button));
        d.settle();
        if folio.is_none() {
            let choice = pages
                .first()
                .map_or_else(|| "New page…".to_owned(), |(f, t)| format!("{f} {t}").trim().to_owned());
            // The list lies over the window: its entry is the last widget of that name (a page's row has it too).
            let at = d
                .labels()
                .into_iter()
                .filter(|(l, _)| *l == choice)
                .map(|(_, at)| at)
                .last()
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
    assert_eq!(d.shell_mut().dispatch("plc.rack_create", &json!({})), Outcome::Done);
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
    let dp = station(&mut d, "valve_terminal", "EW-GEN-VTM-1", &["EW-GEN-VLV-3", "EW-GEN-VLV-3"]);
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
    assert!(d.find("Draw on an electrical page…").is_some(), "{:?}", sides(&mut d));
    d.input(&click(d.find("Draw on page 201").expect("the pneumatic side's button")));
    d.settle();
    click_at(&mut d, (650, 900));
    d.input(&key("escape"));
    d.settle();
    let cells = d.shell_mut().rack_view(pn).expect("the terminal").cells;
    assert_eq!(cells[0].at.page.as_deref(), Some("201"), "its valve in its cell");
    run_todo(&mut d, pn, &mut placed);
    // A bus device of one's own, drawn on the electrical page and given to a PLC from a new station's view.
    let _ = d.shell_mut().dispatch("page.goto", &json!({"folio": "101"}));
    let _ = d.shell_mut().dispatch("tool.black_box", &json!({"part": "EW-GEN-PND-1", "template": 0}));
    click_at(&mut d, (900, 800));
    click_at(&mut d, (1000, 900));
    d.input(&key("escape"));
    d.settle();
    let _ = d.shell_mut().dispatch("plc.new_station", &json!({"kind": "remote"}));
    let (device, _) = d.shell_mut().station_candidates()[0].clone();
    let add = json!({"device": device.to_string()});
    assert_eq!(d.shell_mut().dispatch("plc.add_to_plc", &add), Outcome::Done);
    run_todo(&mut d, device, &mut placed);
    d.shell_mut().shutdown();
}
