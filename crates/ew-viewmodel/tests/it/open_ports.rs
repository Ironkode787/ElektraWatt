// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A rack run: the open working ports of the valves on a terminal were plugged one at a time from a right-click
//! menu found by chance, the Fluid tab's Draw plug stayed greyed, and two valves set spare still warned. Plug ports on
//! the bar plugs the selected valve's open ports, EW-FL-001's fix plugs a valve's, and Set spare plugs them with it.

use crate::common;

use common::{Tmp, at, done};
use ew_api::output::Uuid;
use ew_viewmodel::command_bar::{MenuSpot, Tab, canvas_menu, command_bar_of};
use ew_viewmodel::navigators::{NavKind, NavRole};
use ew_viewmodel::shell::{Mode, Outcome, ShellConfig, ShellModel};
use serde_json::json;

/// A new project with the fluid page 102 and a valve terminal made in its station view with three valves, drawn there
/// as its positions; the valves, left to right.
fn terminal_with_valves(tmp: &Tmp) -> (ShellModel, Vec<Uuid>) {
    let mut s = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join("config"), "0.0.0-test"),
    );
    let dir = tmp.0.join("new.ew");
    done(
        &mut s,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "102", "type": "fluid", "title": "Fluid"}),
    );
    done(&mut s, "plc.new_station", json!({}));
    done(&mut s, "plc.rack_set", json!({"head": "EW-GEN-VTM-1"}));
    done(&mut s, "plc.rack_create", json!({}));
    for _ in 0..3 {
        done(&mut s, "plc.rack_add", json!({"part": "EW-GEN-VLV-3"}));
    }
    done(&mut s, "plc.rack_next", json!({"step": "fluid"}));
    let sn = &s.canvas().unwrap().page().snapper;
    let p = sn.page_to_sheet((sn.page_of_index(10), sn.page_of_index(30)));
    done(&mut s, "canvas.press", at(p, json!({"alt": true})));
    done(&mut s, "canvas.release", at(p, json!({"alt": true})));
    let _ = s.dispatch("ui.dismiss", &json!({}));
    let valves = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find_map(|r| r.black_box.as_ref().filter(|b| !b.cells.is_empty()))
        .map(|b| b.cells.iter().filter_map(|c| c.holder).collect())
        .expect("the terminal drawn as its positions");
    (s, valves)
}

/// The keys of the EW-FL-001 findings of a fresh check.
fn open_port_findings(s: &mut ShellModel) -> Vec<String> {
    done(s, "messages.show", json!({"on": true}));
    done(s, "messages.check", json!({}));
    s.nav(NavKind::Messages)
        .and_then(|n| n.table())
        .map(|t| {
            t.nodes
                .iter()
                .filter(|n| n.role == NavRole::Finding && n.cells[1] == "EW-FL-001")
                .map(|n| n.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Selects the symbol of valve `valve` on the page shown.
fn select_valve(s: &mut ShellModel, valve: Uuid) {
    let rep = s
        .canvas()
        .unwrap()
        .page()
        .tables
        .reps
        .iter()
        .find(|r| r.device == Some(valve) && r.black_box.is_none())
        .map(|r| r.id.to_string())
        .expect("the valve's symbol");
    done(s, "select.set", json!({"ids": [rep]}));
}

#[test]
fn plug_ports_the_findings_fix_and_set_spare_leave_no_port_open() {
    let tmp = Tmp::new("open-ports");
    let (mut s, valves) = terminal_with_valves(&tmp);
    assert_eq!(valves.len(), 3, "three valves in their cells");
    assert_eq!(open_port_findings(&mut s).len(), 3, "each valve's ports open");
    // Nothing selected: Plug ports is greyed with its reason.
    done(&mut s, "select.set", json!({"ids": []}));
    let bar = command_bar_of(&s, Tab::Fluid)
        .groups
        .into_iter()
        .flat_map(|g| g.buttons)
        .find(|b| b.id == "fluid.plug_ports")
        .expect("the Fluid tab's Plug ports");
    assert!(!bar.enabled);
    assert_eq!(
        bar.reason.as_deref(),
        Some("Select a valve whose working ports are open")
    );
    // The first valve selected: Plug ports plugs its two ports, one undo step.
    select_valve(&mut s, valves[0]);
    let rev = s.revision();
    done(&mut s, "fluid.plug_ports", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one step");
    assert_eq!(open_port_findings(&mut s).len(), 2);
    // The second set spare from its menu: its ports are plugged with it.
    select_valve(&mut s, valves[1]);
    let spare = canvas_menu(&s, MenuSpot::Object, None)
        .into_iter()
        .find(|i| i.label == "Set spare")
        .expect("Set spare on the valve's menu");
    assert_eq!(s.dispatch(spare.id, &spare.params), Outcome::Done);
    assert_eq!(
        open_port_findings(&mut s).len(),
        1,
        "the spare valve asks for nothing"
    );
    // The last one's finding offers the fix that plugs them.
    let key = open_port_findings(&mut s).remove(0);
    done(&mut s, "messages.fix", json!({"key": key}));
    done(&mut s, "navigator.apply", json!({}));
    assert!(open_port_findings(&mut s).is_empty());
    s.shutdown();
}

/// A valve with both working ports open is one finding whose fix is two plugs. Fix all counted the plugs against the
/// findings and went below nought: the program stopped at the click.
#[test]
fn fix_all_plugs_every_valves_open_ports_in_one_step() {
    let tmp = Tmp::new("open-ports-all");
    let (mut s, valves) = terminal_with_valves(&tmp);
    assert_eq!(valves.len(), 3, "three valves in their cells");
    let findings = open_port_findings(&mut s);
    assert_eq!(
        findings.len(),
        3,
        "each valve's two ports open: three findings, six plugs"
    );
    done(&mut s, "messages.fix_all", json!({"key": findings[0]}));
    let rev = s.revision();
    done(&mut s, "navigator.apply", json!({}));
    assert_eq!(s.revision(), rev.map(|r| r + 1), "one step for all of them");
    assert!(
        open_port_findings(&mut s).is_empty(),
        "no working port is left open"
    );
    s.shutdown();
}
