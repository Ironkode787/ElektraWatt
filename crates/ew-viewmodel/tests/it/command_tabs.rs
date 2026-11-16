// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The PLC and Fluid tabs of the command bar and the workspace switch (rc.2, the owner's F3): the PLC and fluid
//! commands have buttons of their own instead of hiding in menus and row menus, each tab switches to its workspace,
//! and the tab row shows which workspace is active.

use crate::common::{Tmp, done, new_project};
use ew_viewmodel::actions::ACTIONS;
use ew_viewmodel::command_bar::{ButtonKind, CommandButton, Tab, command_bar_of, workspace_choices};
use ew_viewmodel::shell::{Mode, ShellModel};
use ew_viewmodel::strings;
use serde_json::json;

/// The buttons of `tab` in the state of `s`.
fn buttons(s: &ShellModel, tab: Tab) -> Vec<CommandButton> {
    command_bar_of(s, tab)
        .groups
        .into_iter()
        .flat_map(|g| g.buttons)
        .collect()
}

fn button(s: &ShellModel, tab: Tab, id: &str) -> CommandButton {
    buttons(s, tab)
        .into_iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("{id} has no button on {tab:?}"))
}

/// Every command of the PLC, bus and fluid power that a person starts by hand has a button on the PLC or the Fluid
/// tab; the settings of one module, one channel or one fluid line, and a bus device given to a PLC, stay on that
/// thing's own menu, and Put a station on this page heads the list of Place port's button. The exchange with PLC
/// software moved from Output to PLC, and each tab ends with its workspace and its guide.
#[test]
fn every_plc_and_fluid_command_has_a_button_on_its_tab() {
    const ROW_ONLY: [&str; 5] = [
        "plc.module_safety",
        "plc.channel_settings",
        "fluid.set_medium",
        "plc.add_to_plc",
        "bus.put_station",
    ];
    for a in ACTIONS.iter().filter(|a| !a.hidden && !a.headless_only) {
        let discipline = ["plc.", "bus.", "fluid."].iter().any(|p| a.id.starts_with(p))
            || matches!(a.id, "tool.fluid_line" | "project.plc_addresses_at_terminals");
        if !discipline || ROW_ONLY.contains(&a.id) {
            continue;
        }
        assert!(
            a.placement
                .is_some_and(|p| matches!(p.tab, Tab::Plc | Tab::Fluid)),
            "{} has no button on the PLC or the Fluid tab",
            a.id
        );
    }
    let tmp = Tmp::new("tabs-placed");
    let mut s = new_project(&tmp);
    assert!(buttons(&s, Tab::Output).iter().all(|b| b.id != "plc.export"));
    for id in ["plc.export", "plc.import", "plc.workspace", "plc.guide"] {
        button(&s, Tab::Plc, id);
    }
    for id in ["tool.fluid_line", "fluid.workspace", "fluid.guide"] {
        button(&s, Tab::Fluid, id);
    }
    // The tabs stand between Draw and Check.
    let tabs: Vec<Tab> = command_bar_of(&s, Tab::Draw)
        .tabs
        .iter()
        .map(|(t, _)| *t)
        .collect();
    let at = |t: Tab| tabs.iter().position(|x| *x == t).unwrap();
    assert!(at(Tab::Draw) < at(Tab::Plc) && at(Tab::Plc) < at(Tab::Fluid) && at(Tab::Fluid) < at(Tab::Check));
    s.shutdown();
}

/// The workspace switch: PLC workspace on the PLC tab loads the PLC workspace and shows as on, a second press goes
/// back to Schematic; the tab row's switch marks the active one, and a saved workspace by its name while it is active.
#[test]
fn the_workspace_switch_shows_the_active_workspace() {
    let tmp = Tmp::new("tabs-workspace");
    let mut s = new_project(&tmp);
    let active = |s: &ShellModel| -> Vec<(String, bool)> {
        workspace_choices(s)
            .into_iter()
            .map(|c| (c.label, c.active))
            .collect()
    };
    assert_eq!(
        active(&s),
        [
            ("Schematic".to_owned(), true),
            ("PLC".to_owned(), false),
            ("Fluid power".to_owned(), false)
        ]
    );
    assert!(!button(&s, Tab::Plc, "plc.workspace").checked);
    done(&mut s, "plc.workspace", json!({}));
    assert_eq!(s.workspace(), "PLC");
    assert!(button(&s, Tab::Plc, "plc.workspace").checked);
    assert!(!button(&s, Tab::Fluid, "fluid.workspace").checked);
    assert_eq!(active(&s)[1], ("PLC".to_owned(), true));
    done(&mut s, "plc.workspace", json!({}));
    assert_eq!(s.workspace(), "Schematic");
    done(&mut s, "fluid.workspace", json!({}));
    assert!(button(&s, Tab::Fluid, "fluid.workspace").checked);
    done(&mut s, "view.workspace_save", json!({"name": "Bench"}));
    let shown = active(&s);
    assert_eq!(shown.last(), Some(&("Bench".to_owned(), true)));
    assert_eq!(shown.iter().filter(|(_, on)| *on).count(), 1, "{shown:?}");
    s.shutdown();
}

/// A Fluid tab button that needs a fluid page is greyed elsewhere with its reason in words, and works on one.
#[test]
fn the_fluid_buttons_need_a_fluid_page_and_say_so() {
    let tmp = Tmp::new("tabs-fluid");
    let mut s = new_project(&tmp);
    let symbol = button(&s, Tab::Fluid, "fluid.symbol");
    assert!(!symbol.enabled);
    assert_eq!(
        symbol.reason.as_deref(),
        Some(strings::text(ew_viewmodel::key!("reason-fluid-page-symbols")))
    );
    assert!(!button(&s, Tab::Fluid, "tool.fluid_line").enabled);
    done(&mut s, "page.new", json!({"folio": "201", "type": "fluid"}));
    for id in [
        "fluid.symbol",
        "fluid.air_supply",
        "fluid.valve_terminal",
        "tool.fluid_line",
    ] {
        assert!(button(&s, Tab::Fluid, id).enabled, "{id}");
    }
    done(&mut s, "fluid.air_supply", json!({}));
    assert!(s.picker().is_some(), "the device browser is open");
    s.shutdown();
}

/// The PLC tab's menu buttons list their targets: Rack lists the stations, and a choice opens that station's rack
/// and slots; without a station the button is greyed with the reason.
#[test]
fn the_rack_button_lists_the_stations() {
    let tmp = Tmp::new("tabs-rack");
    let mut empty = new_project(&tmp);
    let rack = button(&empty, Tab::Plc, "plc.rack_editor");
    assert!(!rack.enabled);
    assert_eq!(
        rack.reason.as_deref(),
        Some(strings::text(ew_viewmodel::key!("reason-no-station")))
    );
    empty.shutdown();
    let tmp = Tmp::new("tabs-rack-golden");
    let mut s = tmp.shell_in(Mode::Headless, true, "101");
    let rack = button(&s, Tab::Plc, "plc.rack_editor");
    let ButtonKind::Menu(items) = rack.kind else {
        panic!("Rack is a menu button")
    };
    let first = items.first().expect("a station");
    assert!(first.enabled && !first.label.is_empty(), "{first:?}");
    done(&mut s, first.id, first.params.clone());
    assert!(matches!(
        s.dialog(),
        Some(ew_viewmodel::shell::Dialog::Rack { .. })
    ));
    s.shutdown();
}
