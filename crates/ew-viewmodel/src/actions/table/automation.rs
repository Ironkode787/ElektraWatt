// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `automation`: the window's entries of the PLC, bus and fluid commands (WP-S2.13), each
//! building one declared command (`plc.*`, `bus.*`, `fluid.*`, `assembly.attach`) from the row or the selection it is
//! aimed at.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    // A CPU row's Attach cards without a CPU (FT-1672): the cards that wait for a CPU become children of the station.
    with_params(
        "station:ref",
        needs_parameters(in_navigator(with_aliases(
            &[("station", ParamForm::Whole)],
            act(
                "plc.attach_cards",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_attach_cards(p),
            ),
        ))),
    ),
    // A station row's Rack and slots… (FR-PLC-028) and the editor's own controls.
    with_params(
        "station:ref",
        needs_parameters(in_navigator(with_aliases(
            &[("station", ParamForm::Whole)],
            act(
                "plc.rack_editor",
                View,
                Menu::Project,
                &[],
                project_open,
                None,
                |s, p| s.act_plc_rack_editor(p),
            ),
        ))),
    ),
    with_params(
        "part:ref",
        hidden(with_aliases(
            &[("part", ParamForm::Whole)],
            act(
                "plc.rack_part",
                View,
                Menu::Project,
                &[],
                project_open,
                None,
                |s, p| s.act_plc_rack_part(p),
            ),
        )),
    ),
    with_params(
        "slot:uint, rack?:uint",
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
        needs_parameters(in_navigator(act(
            "plc.preview",
            Command,
            Menu::Project,
            &[],
            writable,
            None,
            |s, p| s.act_plc_preview(p),
        ))),
    ),
    // PLC exchange (doc 10 §4, §6): Export… and Import… in the Output menu, the import preview's own controls.
    with_params(
        "format?:text, stations?:text, date?:text, path?:path, replace?:bool",
        act(
            "plc.export",
            Command,
            Menu::Output,
            &[],
            writable,
            slot(Menu::Output),
            |s, p| s.act_plc_export(p),
        ),
    ),
    with_params(
        "path?:path, complete?:bool",
        act(
            "plc.import",
            View,
            Menu::Output,
            &[],
            writable,
            slot(Menu::Output),
            |s, p| s.act_plc_import(p),
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
            act(
                "plc.instrument_list",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_instrument_list(p),
            ),
        ))),
    ),
    // The PLC navigator rows' settings forms (FT-1752): Symbol policy…, Safety data…, Mode and safety….
    with_params(
        "config:ref, template?:text, tag_table?:text, strict_iec?:text",
        needs_parameters(in_navigator(with_aliases(
            &[("config", ParamForm::Whole)],
            act(
                "plc.symbol_policy",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_plc_symbol_policy(p),
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
    // Project ▸ Process-tag scheme…, Fluid settings… and Channel addresses at terminals (FT-1752).
    with_params(
        "scheme?:text",
        act(
            "plc.process_tag_scheme",
            Command,
            Menu::Project,
            &[],
            writable,
            slot(Menu::Project),
            |s, p| s.act_process_tag_scheme(p),
        ),
    ),
    with_params(
        "od?:text, id?:text, material?:text, line_tags?:text",
        act(
            "fluid.settings",
            Command,
            Menu::Project,
            &[],
            writable,
            slot(Menu::Project),
            |s, p| s.act_fluid_settings(p),
        ),
    ),
    with_params(
        "on?:bool",
        act(
            "project.plc_addresses_at_terminals",
            Command,
            Menu::Project,
            &[],
            writable,
            slot(Menu::Project),
            |s, p| s.act_plc_addresses_at_terminals(p),
        ),
    ),
    // A valve terminal row's Generate overview… (FR-FLU-033): the previewed overview pages.
    with_params(
        "station:ref",
        needs_parameters(in_navigator(with_aliases(
            &[("station", ParamForm::Whole)],
            act(
                "fluid.generate_overview",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_fluid_generate_overview(p),
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
        "name?:text, p_nom?:text, code?:text",
        act(
            "fluid.medium_new",
            Command,
            Menu::Project,
            &[],
            writable,
            slot(Menu::Project),
            |s, p| s.act_fluid_medium_new(p),
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
];
