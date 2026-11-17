// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `automation`: the window's entries of the PLC, bus and fluid commands (WP-S2.13), each
//! building one declared command (`plc.*`, `bus.*`, `fluid.*`, `assembly.attach`) from the row or the selection it is
//! aimed at.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    // A CPU row's Attach cards without a station (FT-1672): the cards that wait for a CPU become children of the station.
    with_params(
        "station:ref, card?:ref",
        needs_parameters(in_navigator(with_aliases(
            &[("station", ParamForm::Whole)],
            placed(
                place(
                    Group::Station,
                    20,
                    CommandSize::Small,
                    Some(key!("bar-attach-cards")),
                ),
                "cards",
                act(
                    "plc.attach_cards",
                    Command,
                    Menu::Plc,
                    &[],
                    writable,
                    sub(Menu::Plc, Submenu::AttachCards, false),
                    |s, p| s.act_plc_attach_cards(p),
                ),
            ),
        ))),
    ),
    // A station's Rack… (FR-PLC-028) opens its view as a tab, and the view's own controls.
    with_params(
        "station:ref",
        needs_parameters(in_navigator(with_aliases(
            &[("station", ParamForm::Whole)],
            placed(
                place(Group::Station, 10, CommandSize::Small, Some(key!("bar-rack"))),
                "server",
                act(
                    "plc.rack_editor",
                    View,
                    Menu::Plc,
                    &[],
                    project_open,
                    sub(Menu::Plc, Submenu::RackEditor, false),
                    |s, p| s.act_plc_rack_editor(p),
                ),
            ),
        ))),
    ),
    with_params(
        "head?:bool, slot?:uint, rack?:uint, module?:ref, position?:uint, card?:ref, step?:int",
        hidden(with_aliases(
            &[("module", ParamForm::Whole), ("card", ParamForm::Whole)],
            act(
                "plc.rack_select",
                View,
                Menu::Project,
                &[],
                project_open,
                None,
                |s, p| s.act_plc_rack_select(p),
            ),
        )),
    ),
    // The commands on the module under a station view's cursor (`plc.module_*`): the view's own controls, in its bar
    // above the rack and on a right click on a card, as a module's settings stay on the module's own menu. They are
    // no buttons of the PLC tab, which is full at 1920 × 1080 in Dutch and in the long-text locale (doc 16a §5.3,
    // law 3).
    with_params(
        "part:text",
        hidden(act(
            "plc.module_replace",
            Command,
            Menu::Plc,
            &[],
            station_module,
            None,
            |s, p| s.act_plc_module(p, "replace"),
        )),
    ),
    hidden(act(
        "plc.module_remove",
        Command,
        Menu::Plc,
        &[],
        station_module,
        None,
        |s, p| s.act_plc_module(p, "empty"),
    )),
    with_params(
        "folio:text",
        hidden(act(
            "plc.module_draw",
            Command,
            Menu::Plc,
            &[],
            station_module_undrawn,
            None,
            |s, p| s.act_plc_module(p, "draw"),
        )),
    ),
    hidden(act(
        "plc.module_goto",
        View,
        Menu::Plc,
        &[],
        station_module_drawn,
        None,
        |s, _| s.act_plc_module_goto(),
    )),
    hidden(act(
        "plc.module_left",
        Command,
        Menu::Plc,
        &[],
        station_module,
        None,
        |s, p| s.act_plc_module(p, "up"),
    )),
    hidden(act(
        "plc.module_right",
        Command,
        Menu::Plc,
        &[],
        station_module,
        None,
        |s, p| s.act_plc_module(p, "down"),
    )),
    with_params(
        "part:text, slot?:uint, rack?:uint, module?:ref, position?:uint",
        hidden(act(
            "plc.rack_add",
            Command,
            Menu::Project,
            &[],
            writable,
            None,
            |s, p| s.act_plc_rack_add(p),
        )),
    ),
    // A configuration project's or a module's previewed addressing command (its row menu, FR-PLC-014/-015).
    with_params(
        "cmd:text, params?:json",
        needs_parameters(in_navigator(placed(
            place(
                Group::Addresses,
                1,
                CommandSize::Large,
                Some(key!("bar-addresses")),
            ),
            "hash",
            act(
                "plc.preview",
                Command,
                Menu::Plc,
                &[],
                writable,
                sub(Menu::Plc, Submenu::PlcAddressing, false),
                |s, p| s.act_plc_preview(p),
            ),
        ))),
    ),
    // PLC exchange (doc 10 §4, §6): Export… and Import… on the PLC tab and in its menu, the import preview's own
    // controls.
    with_params(
        "format?:text, stations?:text, date?:text, path?:path, replace?:bool",
        placed(
            place(
                Group::Exchange,
                1,
                CommandSize::Small,
                Some(key!("bar-plc-export")),
            ),
            "transfer-out",
            act("plc.export", Command, Menu::Plc, &[], writable, None, |s, p| {
                s.act_plc_export(p)
            }),
        ),
    ),
    with_params(
        "path?:path, complete?:bool",
        placed(
            place(
                Group::Exchange,
                2,
                CommandSize::Small,
                Some(key!("bar-plc-import")),
            ),
            "transfer-in",
            act("plc.import", View, Menu::Plc, &[], writable, None, |s, p| {
                s.act_plc_import(p)
            }),
        ),
    ),
    with_params(
        "tab:enum(changed|conflicts|removed|parking)",
        hidden(act(
            "plc.import_tab",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_plc_import_tab(p),
        )),
    ),
    with_params(
        "id:text, choice?:enum(ours|theirs|create)",
        hidden(act(
            "plc.import_choose",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_plc_import_choose(p),
        )),
    ),
    hidden(act(
        "plc.import_apply",
        Command,
        Menu::Output,
        &[],
        writable,
        None,
        |s, _| s.act_plc_import_apply(),
    )),
    // A configuration project row's Import instrument list… (FR-PLC-050), previewed.
    with_params(
        "config:ref, path?:path",
        needs_parameters(in_navigator(with_aliases(
            &[("config", ParamForm::Whole)],
            placed(
                place(
                    Group::PlcLists,
                    3,
                    CommandSize::Small,
                    Some(key!("bar-instrument-list")),
                ),
                "file-spreadsheet",
                act(
                    "plc.instrument_list",
                    Command,
                    Menu::Plc,
                    &[],
                    writable,
                    sub(Menu::Plc, Submenu::InstrumentList, false),
                    |s, p| s.act_plc_instrument_list(p),
                ),
            ),
        ))),
    ),
    // The PLC navigator rows' settings forms (FT-1752): Symbol policy…, Safety data…, Mode and safety….
    with_params(
        "config:ref, template?:text, tag_table?:text, strict_iec?:text",
        needs_parameters(in_navigator(with_aliases(
            &[("config", ParamForm::Whole)],
            placed(
                place(Group::Addresses, 4, CommandSize::Small, None),
                "code",
                act(
                    "plc.symbol_policy",
                    Command,
                    Menu::Plc,
                    &[],
                    writable,
                    sub(Menu::Plc, Submenu::SymbolPolicy, false),
                    |s, p| s.act_plc_symbol_policy(p),
                ),
            ),
        ))),
    ),
    with_params(
        "module:ref, f_source?:text, f_dest?:text, f_wd_ms?:text",
        needs_parameters(in_navigator(with_aliases(
            &[("module", ParamForm::Whole)],
            act(
                "plc.module_safety",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_module_safety(p),
            ),
        ))),
    ),
    with_params(
        "channel:ref, mode?:text, evaluation?:text, requirement?:text",
        needs_parameters(in_navigator(with_aliases(
            &[("channel", ParamForm::Whole)],
            act(
                "plc.channel_settings",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_channel_settings(p),
            ),
        ))),
    ),
    // PLC ▸ Addresses: Process-tag scheme… and Channel addresses at terminals; Fluid ▸ Media: Fluid settings… (FT-1752).
    with_params(
        "scheme?:text",
        placed(
            place(Group::Addresses, 3, CommandSize::Small, None),
            "abc",
            act(
                "plc.process_tag_scheme",
                Command,
                Menu::Plc,
                &[],
                writable,
                None,
                |s, p| s.act_process_tag_scheme(p),
            ),
        ),
    ),
    with_params(
        "od?:text, id?:text, material?:text, line_tags?:text",
        placed(
            place(Group::Media, 2, CommandSize::Small, None),
            "settings-2",
            act(
                "fluid.settings",
                Command,
                Menu::Fluid,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_settings(p),
            ),
        ),
    ),
    with_params(
        "on?:bool",
        placed(
            place(
                Group::Addresses,
                2,
                CommandSize::Small,
                Some(key!("bar-addresses-at-terminals")),
            ),
            "tags",
            act(
                "project.plc_addresses_at_terminals",
                Command,
                Menu::Plc,
                &[],
                writable,
                None,
                |s, p| s.act_plc_addresses_at_terminals(p),
            ),
        ),
    ),
    // A valve terminal row's Generate overview… (FR-FLU-033): the previewed overview pages; without a row, of the
    // selected valve terminal, greyed in words while none is selected (the persona round).
    with_params(
        "station?:ref",
        in_navigator(with_aliases(
            &[("station", ParamForm::Whole)],
            placed(
                place(Group::FluidLists, 3, CommandSize::Small, None),
                "report-analytics",
                act(
                    "fluid.generate_overview",
                    Command,
                    Menu::Fluid,
                    &[],
                    valve_terminal_selected,
                    None,
                    |s, p| s.act_fluid_generate_overview(p),
                ),
            ),
        )),
    ),
    // A valve terminal's Add valve at position n (doc 11 §7): the valve child, then its symbol on the pointer.
    with_params(
        "module:ref, position:uint",
        needs_parameters(hidden(with_aliases(
            &[("module", ParamForm::Whole)],
            act(
                "fluid.valve_add",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_valve_add(p),
            ),
        ))),
    ),
    // A position of a valve terminal drawn as its positions (rc.2, F15): Fill position n… and Replace… open the device
    // browser narrowed to valves for a terminal; the valve chosen (or `part`) is placed in that cell.
    with_params(
        "terminal:ref, module:ref, position:uint, replace?:bool, part?:ref",
        needs_parameters(hidden(with_aliases(
            &[("terminal", ParamForm::Whole)],
            act(
                "fluid.cell_fill",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_cell_fill(p),
            ),
        ))),
    ),
    // A terminal valve's Go to output (rc.2, F15): the channel that drives its solenoid, in the PLC list.
    with_params(
        "channel?:ref",
        hidden(act(
            "fluid.go_output",
            Command,
            Menu::Project,
            &[],
            project_open,
            None,
            |s, p| s.act_fluid_go_output(p),
        )),
    ),
    // A valve terminal drawn before as a free box: Draw as slots (rc.2, F15), previewed.
    with_params(
        "rep:ref",
        needs_parameters(hidden(with_aliases(
            &[("rep", ParamForm::Whole)],
            act(
                "fluid.draw_slots",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_draw_slots(p),
            ),
        ))),
    ),
    // A stand-alone valve's Draw solenoid 14 on this page… or on an electrical page… (rc.2, F11): the coil's symbol on the
    // pointer on the electrical page in front, `page` (a folio or `new`), else the page the window asks for.
    with_params(
        "function:ref, page?:text",
        needs_parameters(hidden(with_aliases(
            &[("function", ParamForm::Whole)],
            act(
                "fluid.draw_solenoid",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_draw_solenoid(p),
            ),
        ))),
    ),
    // A multipin valve terminal's Draw the plug on an electrical page… (rc.2): from any page, its menu, its Properties
    // row and the Fluid tab (of the selected terminal, greyed in words while none is selected).
    with_params(
        "terminal?:ref, page?:text",
        with_aliases(
            &[("terminal", ParamForm::Whole)],
            placed(
                place(
                    Group::FluidLists,
                    4,
                    CommandSize::Small,
                    Some(key!("bar-draw-plug")),
                ),
                "bolt",
                act(
                    "fluid.draw_plug",
                    Command,
                    Menu::Fluid,
                    &[],
                    multipin_terminal_selected,
                    None,
                    |s, p| s.act_fluid_draw_plug(p),
                ),
            ),
        ),
    ),
    // Go to a thing by its drawing (rc.2): a valve's solenoid and the coil's valve, the symbol selected and named.
    with_params(
        "record:text, words?:text",
        needs_parameters(hidden(act(
            "nav.show",
            View,
            Menu::Page,
            &[],
            has_pages,
            None,
            |s, p| s.act_nav_show(p),
        ))),
    ),
    // A valve terminal made as a station, drawn on a fluid page as its positions (rc.2): its PLC row, its rack dialog.
    with_params(
        "terminal:ref",
        needs_parameters(hidden(with_aliases(
            &[("terminal", ParamForm::Whole)],
            act(
                "fluid.draw_terminal",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_draw_terminal(p),
            ),
        ))),
    ),
    // A fluid line's medium (its context menu) and New medium… (doc 11 §7, FR-FLU-008/-009).
    with_params(
        "line:ref, medium:ref",
        needs_parameters(in_canvas(with_aliases(
            &[("line", ParamForm::Whole), ("medium", ParamForm::Whole)],
            act(
                "fluid.set_medium",
                Command,
                Menu::Draw,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_set_medium(p),
            ),
        ))),
    ),
    with_params(
        "preset?:text, name?:text, p_nom?:text, code?:text",
        placed(
            place(Group::Media, 1, CommandSize::Large, None),
            "bubble-plus",
            act(
                "fluid.medium_new",
                Command,
                Menu::Fluid,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_medium_new(p),
            ),
        ),
    ),
    with_params(
        "module:ref, slot:text",
        hidden(with_aliases(
            &[("module", ParamForm::Whole)],
            act(
                "plc.rack_move",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_rack_move(p),
            ),
        )),
    ),
    // A station is made as a rack and filled; a bus device is given to a PLC.
    with_params(
        "kind?:text",
        placed(
            place(
                Group::Station,
                1,
                CommandSize::Large,
                Some(key!("bar-new-station")),
            ),
            "cpu-2",
            act(
                "plc.new_station",
                Command,
                Menu::Plc,
                &[],
                writable,
                None,
                |s, p| s.act_plc_new_station(p),
            ),
        ),
    ),
    with_params(
        "kind?:text, head?:text, config?:text, filter?:text",
        hidden(act(
            "plc.rack_set",
            View,
            Menu::Project,
            &[],
            project_open,
            None,
            |s, p| s.act_plc_rack_set(p),
        )),
    ),
    with_params(
        "op:text, slot?:uint, rack?:uint, part?:text, folio?:text",
        hidden(act(
            "plc.rack_slot",
            Command,
            Menu::Project,
            &[],
            writable,
            None,
            |s, p| s.act_plc_rack_slot(p),
        )),
    ),
    hidden(act(
        "plc.rack_create",
        Command,
        Menu::Project,
        &[],
        writable,
        None,
        |s, _| s.act_plc_rack_create(),
    )),
    with_params(
        "step:text, folio?:text",
        hidden(act(
            "plc.rack_next",
            Command,
            Menu::Project,
            &[],
            writable,
            None,
            |s, p| s.act_plc_rack_next(p),
        )),
    ),
    with_params(
        "device:ref, config?:ref",
        needs_parameters(in_canvas(with_aliases(
            &[("device", ParamForm::Whole), ("config", ParamForm::Whole)],
            act(
                "plc.add_to_plc",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_add_to_plc(p),
            ),
        ))),
    ),
];
