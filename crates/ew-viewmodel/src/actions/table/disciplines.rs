// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `disciplines`: the PLC and Fluid tabs' own entries (doc 16a §5.3), each an action the
//! window already has with its discipline's parameters (`ShellModel::act_discipline_*`). The PLC and fluid commands
//! themselves are placed where they are declared (section `automation`, `tool.fluid_line` in `draw`).

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
        "device:text, port:text",
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
    // PLC ▸ Workspace: the PLC workspace and the PLC guide.
    placed(
        place(Group::PlcWorkspace, 1, CommandSize::Large, None),
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
    // Fluid ▸ Place: the symbol browser (on a fluid page it offers the fluid symbols and greys the rest), the device
    // browser at the valve terminals, the symbol browser at the air supply and its preparation.
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
            fluid_symbol_page,
            None,
            |s, _| s.act_open_picker(crate::tools::PickerMode::Place),
        ),
    )),
    // A valve terminal of the project drawn on no fluid page yet comes first (rc.2): Draw -104A1 here, or a new one.
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
                fluid_symbol_page,
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
            fluid_symbol_page,
            None,
            |s, _| s.act_discipline_picker(crate::tools::PickerMode::Place, "AirPreparation"),
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
    // Fluid ▸ Workspace: the Fluid workspace and the fluid guide.
    placed(
        place(Group::FluidWorkspace, 1, CommandSize::Large, None),
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
