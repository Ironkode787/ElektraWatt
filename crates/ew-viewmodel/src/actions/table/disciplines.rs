// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `disciplines`: the PLC and Fluid tabs' own entries (doc 16a §5.3), each an action the
//! window already has with its discipline's parameters (`ShellModel::act_discipline_*`). The PLC and fluid commands
//! themselves are placed where they are declared (section `automation`, `tool.fluid_line` in `draw`).

use super::super::enabled::*;
use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    // PLC ▸ Bus: a bus-topology page, a port not drawn yet, the Bus panel.
    placed(
        place(Group::Bus, 1, CommandSize::Large, Some(key!("bar-bus-page"))),
        "topology-star-3",
        act("bus.page_new", Command, Menu::Plc, &[], writable, None, |s, _| {
            s.act_discipline_page("bus_topology")
        }),
    ),
    with_params(
        "device:ref, port:text",
        placed(
            place(Group::Bus, 2, CommandSize::Small, None),
            "plug",
            act(
                "bus.place_port",
                Command,
                Menu::Plc,
                &[],
                editable_page,
                sub(Menu::Plc, Submenu::BusPorts, false),
                |s, p| s.act_bus_place_port(p),
            ),
        ),
    ),
    // Put a station on this page…: its stations head Place port's list; the bus page's own menu has it. A
    // station named (a menu's row, a row dropped on the page) is refused by the action in words of its own.
    with_params(
        "station?:text, x_mm?:mm, y_mm?:mm, group?:uint",
        enabled_when_named(
            &["station"],
            |m, _| editable_page(m),
            act(
                "bus.put_station",
                Command,
                Menu::Plc,
                &[],
                put_station_page,
                sub(Menu::Plc, Submenu::BusPorts, false),
                |s, p| s.act_bus_put_station(p),
            ),
        ),
    ),
    placed(
        place(Group::Bus, 3, CommandSize::Small, None),
        "topology-bus",
        act("bus.panel", View, Menu::Plc, &[], project_open, None, |s, _| {
            s.act_discipline_panel(crate::navigators::NavKind::Bus)
        }),
    ),
    // PLC ▸ Lists: the PLC grid (the PLC panel is on the rail).
    placed(
        place(Group::PlcLists, 1, CommandSize::Large, None),
        "layout-grid",
        act("plc.grid", View, Menu::Plc, &[], project_open, None, |s, _| {
            s.act_discipline_panel(crate::navigators::NavKind::PlcGrid)
        }),
    ),
    // PLC ▸ Workspace: the PLC workspace and the PLC guide, small: the workspace switch in the tab row does the
    // same, and Select and Wire lead the tab.
    placed(
        place(Group::PlcWorkspace, 1, CommandSize::Small, None),
        "app-window",
        act("plc.workspace", View, Menu::Plc, &[], always, None, |s, _| {
            s.act_discipline_workspace("PLC")
        }),
    ),
    keeps_banner(placed(
        place(
            Group::PlcWorkspace,
            2,
            CommandSize::Small,
            Some(key!("bar-guide")),
        ),
        "book",
        act("plc.guide", View, Menu::Plc, &[], always, None, |s, _| {
            s.act_discipline_guide("guide-plc")
        }),
    )),
    // Fluid ▸ Tubes: a fluid page (the Tube tool is `tool.fluid_line`).
    placed(
        place(Group::Tubes, 1, CommandSize::Large, Some(key!("bar-fluid-page"))),
        "droplet-plus",
        act(
            "fluid.page_new",
            Command,
            Menu::Fluid,
            &[],
            writable,
            None,
            |s, _| s.act_discipline_page("fluid"),
        ),
    ),
    // Fluid ▸ Place: the device browser at the pneumatic parts, at the valve terminals, and at the air supply and its
    // preparation, so that what is placed has its part (the air supply placed from the symbols had none); a bare
    // symbol stays Draw ▸ Symbol's. From any page they work on a fluid page: the nearest, else one made then (greyed
    // elsewhere, they say why).
    keeps_banner(placed(
        place(
            Group::FluidPlace,
            1,
            CommandSize::Large,
            Some(key!("bar-fluid-symbol")),
        ),
        "droplet-half-2",
        act(
            "fluid.symbol",
            Command,
            Menu::Fluid,
            &[],
            writable,
            None,
            |s, _| s.act_fluid_picker("pneumatics"),
        ),
    )),
    // A valve terminal of the project drawn on no fluid page yet comes first: Draw -104A1 here, or a new one.
    with_params(
        "terminal?:text",
        keeps_banner(placed(
            place(Group::FluidPlace, 2, CommandSize::Small, None),
            "columns-3",
            act(
                "fluid.valve_terminal",
                Command,
                Menu::Fluid,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_valve_terminal(p),
            ),
        )),
    ),
    keeps_banner(placed(
        place(Group::FluidPlace, 3, CommandSize::Small, None),
        "wind",
        act(
            "fluid.air_supply",
            Command,
            Menu::Fluid,
            &[],
            writable,
            None,
            |s, _| s.act_fluid_picker("AirPreparation"),
        ),
    )),
    // Fluid ▸ Media: the medium table (New medium… and Fluid settings… are declared in `automation`).
    placed(
        place(Group::Media, 3, CommandSize::Small, None),
        "flask",
        act(
            "fluid.media",
            View,
            Menu::Fluid,
            &[],
            project_open,
            None,
            |s, _| s.act_discipline_panel(crate::navigators::NavKind::Media),
        ),
    ),
    // Fluid ▸ Lists: the tube table (Generate overview… is declared in `automation`; the Fluid panel is on the rail).
    placed(
        place(Group::FluidLists, 1, CommandSize::Large, None),
        "table",
        act(
            "fluid.tubes",
            View,
            Menu::Fluid,
            &[],
            project_open,
            None,
            |s, _| s.act_discipline_panel(crate::navigators::NavKind::Tubes),
        ),
    ),
    // Fluid ▸ Workspace: the Fluid workspace and the fluid guide, small as PLC's.
    placed(
        place(Group::FluidWorkspace, 1, CommandSize::Small, None),
        "app-window",
        act("fluid.workspace", View, Menu::Fluid, &[], always, None, |s, _| {
            s.act_discipline_workspace("Fluid")
        }),
    ),
    keeps_banner(placed(
        place(
            Group::FluidWorkspace,
            2,
            CommandSize::Small,
            Some(key!("bar-guide")),
        ),
        "book",
        act("fluid.guide", View, Menu::Fluid, &[], always, None, |s, _| {
            s.act_discipline_guide("guide-fluid")
        }),
    )),
];
