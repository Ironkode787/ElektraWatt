// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `view`. The view: zoom, panes, the Messages pane, text size, theme, language, workspaces, the command bar.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 2, CommandSize::Small, None),
            "zoom-in",
            act(
                "view.zoom_in",
                View,
                Menu::View,
                &["Ctrl+Plus", "Ctrl+Equals"],
                can_zoom_in,
                None,
                |s, p| s.act_zoom_in(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 3, CommandSize::Small, None),
            "zoom-out",
            act(
                "view.zoom_out",
                View,
                Menu::View,
                &["Ctrl+Minus"],
                can_zoom_out,
                None,
                |s, p| s.act_zoom_out(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 4, CommandSize::Small, None),
            "zoom-reset",
            act(
                "view.zoom_actual",
                View,
                Menu::View,
                &["Ctrl+phys:Digit0"],
                has_pages,
                None,
                |s, p| s.act_zoom_actual(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        placed(
            place(Group::Zoom, 1, CommandSize::Large, None),
            "maximize",
            act(
                "view.fit_page",
                View,
                Menu::View,
                &["Home"],
                has_pages,
                None,
                |s, p| s.act_fit_page(p),
            ),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_left",
            View,
            Menu::View,
            &["Ctrl+Arrow Left"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, -1.0, 0.0),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_right",
            View,
            Menu::View,
            &["Ctrl+Arrow Right"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, 1.0, 0.0),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_up",
            View,
            Menu::View,
            &["Ctrl+Arrow Up"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, 0.0, -1.0),
        ),
    )),
    keeps_banner(with_params(
        "group?:uint",
        act(
            "view.pan_down",
            View,
            Menu::View,
            &["Ctrl+Arrow Down"],
            has_pages,
            grp_sub(Group::Zoom, Submenu::Pan),
            |s, p| s.act_pan_step(p, 0.0, 1.0),
        ),
    )),
    keeps_banner(placed(
        place(Group::Layout, 1, CommandSize::Small, None),
        "layout-columns",
        act(
            "view.side_by_side",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, _| s.act_side_by_side(),
        ),
    )),
    keeps_banner(with_params(
        "mode:enum(flat|section|structure)",
        needs_parameters(act(
            "pages.mode",
            View,
            Menu::View,
            &[],
            has_pages,
            grp_sub(Group::Panels, Submenu::PageNavigator),
            |s, p| s.act_pages_mode(p),
        )),
    )),
    keeps_banner(with_params(
        "by:enum(folio|type)",
        // FT-1174: the palette row's icon (doc 16a §6.14).
        needs_parameters(with_icon(
            "arrows-sort",
            act(
                "pages.sort",
                View,
                Menu::View,
                &[],
                has_pages,
                grp_sub(Group::Panels, Submenu::PageNavigator),
                |s, p| s.act_pages_sort(p),
            ),
        )),
    )),
    keeps_banner(placed(
        place(Group::Show, 1, CommandSize::Small, None),
        "grid-dots",
        act("view.grid", View, Menu::View, &[], has_pages, None, |s, _| {
            s.act_grid()
        }),
    )),
    keeps_banner(placed(
        place(Group::Show, 3, CommandSize::Small, None),
        "chart-line",
        act("view.frame_stats", View, Menu::View, &[], always, None, |s, _| {
            s.act_frame_stats()
        }),
    )),
    act(
        "view.cpu_canvas",
        View,
        Menu::View,
        &[],
        always,
        grp(Group::Show),
        |s, _| s.act_cpu_canvas(),
    ),
    // WP-S2.08 part 6 (D18): View ▸ Wire colours, a view toggle of the session.
    keeps_banner(with_params(
        "on?:bool",
        act(
            "view.wire_colours",
            View,
            Menu::View,
            &[],
            has_pages,
            slot(Menu::View),
            |s, p| s.act_wire_colours(p),
        ),
    )),
    // The property grid (WP-S2.05 part 1, spec §6.10).
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 2, CommandSize::Small, None),
            "layout-sidebar-right",
            act("props.show", View, Menu::View, &[], project_open, None, |s, p| {
                s.act_props_show(p)
            }),
        ),
    )),
    // WP-S2.17 part 3 (§4.3, D11, D15): the Advanced rows of the property grid, and the rails' own control.
    with_params(
        "on?:bool",
        act(
            "props.show_advanced",
            View,
            Menu::View,
            &[],
            always,
            grp(Group::Panels),
            |s, p| s.act_props_show_advanced(p),
        ),
    ),
    with_params(
        "side?:enum(left|right), pane:text",
        hidden(act(
            "rail.show",
            View,
            Menu::View,
            &[],
            project_open,
            None,
            |s, p| s.act_rail_show(p),
        )),
    ),
    // WP-S2.09 part 1 (§6.8): View ▸ Jobs and View ▸ Notifications.
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Jobs, 1, CommandSize::Small, None),
            "activity",
            act(
                "jobs.show",
                View,
                Menu::Output,
                &[],
                project_open,
                None,
                |s, p| s.act_jobs_show(p),
            ),
        ),
    )),
    // FT-1372 (ID-1362): Output ▸ Reports ▸ Report block…, `report_block.create` through its form.
    with_params(
        "kind:text, folio:text, title?:text",
        act(
            "report_block.new",
            Command,
            Menu::Output,
            &[],
            writable,
            grp(Group::Reports),
            |s, p| s.act_report_block_new(p),
        ),
    ),
    // FT-717: Output ▸ Parts list opens the first page of the project's parts list (the first `parts_summary` block,
    // else the first `parts_list` block, in page order).
    placed(
        place(Group::Lists, 1, CommandSize::Large, Some(key!("bar-lists-parts"))),
        "list-details",
        act(
            "lists.parts",
            View,
            Menu::Output,
            &[],
            has_parts_list,
            None,
            |s, _| s.act_lists_parts(),
        ),
    ),
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 5, CommandSize::Small, None),
            "bell",
            act(
                "notifications.show",
                View,
                Menu::View,
                &[],
                always,
                None,
                |s, p| s.act_notifications_show(p),
            ),
        ),
    )),
    // WP-S3.09 D9 d (T11): the Messages pane's choice of the migration report beside the project's scheme.
    with_params(
        "on?:bool",
        hidden(act(
            "messages.migration",
            View,
            Menu::Check,
            &[],
            project_open,
            None,
            |s, p| s.act_messages_migration(p),
        )),
    ),
    // WP-S2.09 part 2 (§7.6): View ▸ Messages, F8 and Shift+F8, the quick fix of a finding row.
    with_params(
        "on?:bool",
        placed(
            place(Group::Findings, 1, CommandSize::Large, None),
            "messages",
            act(
                "messages.show",
                View,
                Menu::Check,
                &[],
                project_open,
                None,
                |s, p| s.act_messages_show(p),
            ),
        ),
    ),
    placed(
        place(Group::Findings, 2, CommandSize::Small, None),
        "arrow-narrow-down",
        act(
            "messages.next",
            View,
            Menu::Check,
            &["F8"],
            project_open,
            None,
            |s, _| s.act_messages_walk(true),
        ),
    ),
    placed(
        place(Group::Findings, 3, CommandSize::Small, None),
        "arrow-narrow-up",
        act(
            "messages.previous",
            View,
            Menu::Check,
            &["Shift+F8"],
            project_open,
            None,
            |s, _| s.act_messages_walk(false),
        ),
    ),
    with_params(
        "key?:text",
        hidden(in_navigator(act(
            "messages.fix",
            Command,
            Menu::Check,
            &[],
            writable,
            None,
            |s, p| s.act_messages_fix(p),
        ))),
    ),
    // FT-674: Waive… and Unwaive in the Messages detail (`check.waive`, `check.unwaive`; window only).
    with_params(
        "key?:text, reason?:text, author?:text, date?:text",
        hidden(in_navigator(act(
            "messages.waive",
            Command,
            Menu::Check,
            &[],
            writable,
            None,
            |s, p| s.act_messages_waive(p),
        ))),
    ),
    with_params(
        "key?:text",
        hidden(in_navigator(act(
            "messages.unwaive",
            Command,
            Menu::Check,
            &[],
            writable,
            None,
            |s, p| s.act_messages_unwaive(p),
        ))),
    ),
    with_params(
        "scale:json",
        needs_parameters(placed(
            place(Group::Appearance, 3, CommandSize::Small, None),
            "text-size",
            act(
                "view.ui_scale",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::UiScale, false),
                |s, p| s.act_ui_scale(p),
            ),
        )),
    ),
    // WP-S2.08 D14 (ID-1272 ruling 5): View ▸ Minimum text size (`pt` 0 is off, else 8 to 16) and View ▸ Reduce motion,
    // the settings `min_font_pt` and `reduce_motion`.
    with_params(
        "pt:uint",
        needs_parameters(act(
            "view.min_font",
            View,
            Menu::View,
            &[],
            always,
            sub(Menu::View, Submenu::MinFont, true),
            |s, p| s.act_min_font(p),
        )),
    ),
    with_params(
        "on?:bool",
        act(
            "view.reduce_motion",
            View,
            Menu::View,
            &[],
            always,
            slot(Menu::View),
            |s, p| s.act_reduce_motion(p),
        ),
    ),
    // FT-1439 (ID-1397): View ▸ Mouse wheel scrolls, the setting `wheel_scrolls`; off, the wheel zooms.
    with_params(
        "on?:bool",
        act(
            "view.wheel_scrolls",
            View,
            Menu::View,
            &[],
            always,
            slot(Menu::View),
            |s, p| s.act_wheel_scrolls(p),
        ),
    ),
    // FT-1280 (ID-1315 ruling 3): View ▸ Show tips, the setting `tips` (FR-UI-048), beside Reduce motion. FT-1367
    // (ID-1392): a switch of the View tab's Show group too, so its menu item is View ▸ Show ▸ Show tips.
    with_params(
        "on?:bool",
        placed(
            place(Group::Show, 4, CommandSize::Small, None),
            "info-circle",
            act(
                "view.tips",
                View,
                Menu::View,
                &[],
                always,
                slot(Menu::View),
                |s, p| s.act_tips(p),
            ),
        ),
    ),
    // WP-S2.17 part 1: View ▸ Theme and View ▸ Density (doc 16a §4.13, D5, D6). The theme words of the spec
    // (`hc-dark`, `hc-light`) are not words of the signature grammar, so `value` is text, checked against `THEMES`.
    with_params(
        "value:text",
        needs_parameters(placed(
            place(Group::Appearance, 1, CommandSize::Small, None),
            "sun-moon",
            act(
                "view.theme",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Theme, false),
                |s, p| s.act_view_setting("theme", p),
            ),
        )),
    ),
    with_params(
        "value:enum(comfortable|compact)",
        needs_parameters(placed(
            place(Group::Appearance, 2, CommandSize::Small, None),
            "baseline-density-medium",
            act(
                "view.density",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Density, false),
                |s, p| s.act_view_setting("density", p),
            ),
        )),
    ),
    // WP-S2.08 part 3 (D10, ID-1219 ruling 5): View ▸ Language switches the UI language at run time; `value` is a
    // value of the `language` setting.
    with_params(
        "value:text",
        // FT-1174: the palette row's icon (doc 16a §6.14).
        needs_parameters(with_icon(
            "language",
            act(
                "view.language",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Language, true),
                |s, p| s.act_view_setting("language", p),
            ),
        )),
    ),
    // WP-S2.17 part 2 (§4.3): the command bar's tab and its collapse (Ctrl+F1, a tab's double click), view actions.
    keeps_banner(with_params(
        "tab:enum(project|page|draw|check|output|view)",
        needs_parameters(act(
            "view.command_tab",
            View,
            Menu::View,
            &[],
            always,
            grp_sub(Group::Layout, Submenu::CommandTab),
            |s, p| s.act_command_tab(p),
        )),
    )),
    keeps_banner(with_params(
        "collapsed?:bool",
        act(
            "view.command_bar",
            View,
            Menu::View,
            &["Ctrl+F1"],
            always,
            grp(Group::Layout),
            |s, p| s.act_command_bar(p),
        ),
    )),
    with_params(
        "name:text",
        needs_parameters(placed(
            place(Group::Layout, 2, CommandSize::Small, Some(key!("bar-workspaces"))),
            "layout-board",
            act(
                "view.workspace_load",
                View,
                Menu::View,
                &[],
                always,
                sub(Menu::View, Submenu::Workspace, false),
                |s, p| s.act_workspace_load(p),
            ),
        )),
    ),
    with_params(
        "name:text",
        act(
            "view.workspace_save",
            View,
            Menu::View,
            &[],
            always,
            grp_sub(Group::Layout, Submenu::Workspace),
            |s, p| s.act_workspace_save(p),
        ),
    ),
    placed(
        place(Group::Layout, 3, CommandSize::Small, None),
        "restore",
        act(
            "view.workspace_reset",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, _| s.act_workspace_reset(),
        ),
    ),
    keeps_banner(with_params(
        "query?:text",
        with_icon(
            "search",
            act(
                "tools.palette",
                View,
                Menu::View,
                &["Ctrl+K", "Ctrl+Shift+P"],
                always,
                grp(Group::Panels),
                |s, p| s.act_palette(p),
            ),
        ),
    )),
];
