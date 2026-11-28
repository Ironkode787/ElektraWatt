// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `view`. The view: zoom, panes, the Messages pane, text size, theme, language, workspaces, the command bar.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::enabled::*;
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
        act(
            "pages.mode",
            View,
            Menu::View,
            &[],
            has_pages,
            grp_sub(Group::Panels, Submenu::PageNavigator),
            |s, p| s.act_pages_mode(p),
        ),
    )),
    keeps_banner(with_params(
        "by:enum(folio|type)",
        // The palette row's icon (doc 16a §6.14).
        with_icon(
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
        ),
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
    // View ▸ Wire colours, a view toggle of the session.
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
    // The property grid.
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
    // The Advanced rows of the property grid, and the rails' own control.
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
    // Ctrl+1 to Ctrl+9: the left rail's entry at that position, each key the next entry (doc 16a §5.4).
    chord_param(
        "n",
        with_params(
            "n:uint",
            hidden(act(
                "rail.entry",
                View,
                Menu::View,
                &[
                    "Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4", "Ctrl+5", "Ctrl+6", "Ctrl+7", "Ctrl+8", "Ctrl+9",
                ],
                rail_entries,
                None,
                |s, p| s.act_rail_entry(p),
            )),
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
    // View ▸ Jobs and View ▸ Notifications.
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
    // Output ▸ Reports ▸ Report block…, `report_block.create` through its form; on the Output tab as Report, where
    // every list page is made, and on the Page tab beside New page,
    // since a report block is put on a page (the owner looked for it there).
    with_params(
        "kind?:text, folio?:text, title?:text",
        placed_also(
            place(Group::Pages, 5, CommandSize::Large, Some(key!("bar-report"))),
            placed(
                place(Group::Reports, 1, CommandSize::Large, Some(key!("bar-report"))),
                "report",
                act(
                    "report_block.new",
                    Command,
                    Menu::Output,
                    &[],
                    writable,
                    None,
                    |s, p| s.act_report_block_new(p),
                ),
            ),
        ),
    ),
    // Output ▸ List columns… and Rows of your own… of the list on show (doc 09 §3.8), beside Report.
    with_params(
        "caption?:text, legend?:text, columns?:json",
        placed(
            place(Group::Reports, 2, CommandSize::Small, None),
            "table-options",
            act(
                "report_block.columns",
                Command,
                Menu::Output,
                &[],
                writable_columns,
                None,
                |s, p| s.act_report_block_columns(p),
            ),
        ),
    ),
    with_params(
        "rows?:json",
        placed(
            place(Group::Reports, 3, CommandSize::Small, None),
            "list",
            act(
                "report_block.rows",
                Command,
                Menu::Output,
                &[],
                writable_list,
                None,
                |s, p| s.act_report_block_rows(p),
            ),
        ),
    ),
    // Output ▸ Parts list opens the first page of the project's parts list (the first `parts_summary` block,
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
    // Output ▸ Workshop ▸ Wire list, Assembly lists, Cut list and Consumables open the first page of the project's list
    // of that kind, else Report block… on that kind (doc 13 §12, 13-workshop-lists). The wire list is the fitter's
    // first sheet and stands large; a group holds at most two large commands.
    placed(
        place(
            Group::Workshop,
            1,
            CommandSize::Large,
            Some(key!("bar-lists-wires")),
        ),
        "wave-sine",
        act(
            "lists.wires",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, _| s.act_open_list("wire_list"),
        ),
    ),
    placed(
        place(
            Group::Workshop,
            2,
            CommandSize::Small,
            Some(key!("bar-lists-assembly")),
        ),
        "assembly",
        act(
            "lists.assembly",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, _| s.act_open_list("assembly_list"),
        ),
    ),
    placed(
        place(
            Group::Workshop,
            3,
            CommandSize::Small,
            Some(key!("bar-lists-cut")),
        ),
        "scissors",
        act(
            "lists.cut",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, _| s.act_open_list("cut_list"),
        ),
    ),
    placed(
        place(
            Group::Workshop,
            4,
            CommandSize::Small,
            Some(key!("bar-lists-consumables")),
        ),
        "basket",
        act(
            "lists.consumables",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, _| s.act_open_list("consumables"),
        ),
    ),
    // Output ▸ Workshop ▸ Export list…: the list on show as an Excel or CSV file (doc 13-workshop-lists), after the
    // lists; greyed with its reason where no list page is shown. A list page's menu offers it too.
    with_params(
        "path?:path, replace?:bool",
        placed(
            place(Group::Workshop, 5, CommandSize::Small, None),
            "table-export",
            act(
                "lists.export",
                Command,
                Menu::Output,
                &[],
                list_shown,
                None,
                |s, p| s.act_list_export(p),
            ),
        ),
    ),
    // Output ▸ Workshop ▸ Rework list…: what the fitter changes since a release (doc 13 §3.5); the dialog's Archive
    // file… and Export…; its second place is the Check tab's Revision group, beside the release it compares with.
    with_params(
        "revision?:text",
        placed_also(
            place(Group::Revision, 5, CommandSize::Small, None),
            placed(
                place(Group::Workshop, 6, CommandSize::Small, None),
                "refresh",
                act(
                    "rework.show",
                    View,
                    Menu::Output,
                    &[],
                    project_open,
                    None,
                    |s, p| s.act_rework_show(p),
                ),
            ),
        ),
    ),
    // Output ▸ Workshop ▸ Labels…: the label files, one per label set and marker part (doc 13 §5.4); the dialog's
    // ticks, Export and Change… (Label file).
    with_params(
        "since?:text, archive?:path",
        placed(
            place(Group::Workshop, 7, CommandSize::Small, None),
            "sticker",
            act(
                "labels.show",
                View,
                Menu::Output,
                &[],
                project_open,
                None,
                |s, p| s.act_labels_show(p),
            ),
        ),
    ),
    with_params(
        "file:text, on:bool",
        hidden(act(
            "labels.tick",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_labels_tick(p),
        )),
    ),
    with_params(
        "on?:bool",
        hidden(act(
            "labels.misfits",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_labels_misfits(p),
        )),
    ),
    with_params(
        "dir?:path",
        hidden(act(
            "labels.write",
            Command,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_labels_write(p),
        )),
    ),
    with_params(
        "columns?:json, file?:enum(@FILE_FORM), delimiter?:enum(@DELIMITER), generic?:bool",
        hidden(act(
            "labels.columns",
            Command,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_labels_columns(p),
        )),
    ),
    // Output ▸ Workshop ▸ Hole tables opens the project's hole table, else Report block… on that kind (doc 13 §9).
    placed(
        place(Group::Workshop, 8, CommandSize::Small, None),
        "circles",
        act(
            "lists.holes",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, _| s.act_lists_holes(),
        ),
    ),
    // Output ▸ Workshop ▸ Drilling…: the drawings for drilling of the surfaces with holes (doc 13 §9).
    with_params(
        "surfaces?:json, template?:json, dxf?:json, paper?:text, dir?:path",
        placed(
            place(Group::Workshop, 10, CommandSize::Small, None),
            "crosshair",
            act(
                "drilling.export",
                View,
                Menu::Output,
                &[],
                has_holes,
                None,
                |s, p| s.act_drilling_export(p),
            ),
        ),
    ),
    // Output ▸ Workshop ▸ Package…: one file for the workshop, the group's second large command (doc 13-package §8.5);
    // the dialog's ticks, Show, fixes, Open Messages and Choose….
    with_params(
        "leave_out?:json, personal?:bool, path?:path, replace?:bool, wait?:bool",
        placed(
            place(Group::Workshop, 11, CommandSize::Large, Some(key!("bar-package"))),
            "package-export",
            act(
                "package.export",
                Command,
                Menu::Output,
                &[],
                has_pages,
                None,
                |s, p| s.act_package_export(p),
            ),
        ),
    ),
    with_params(
        "member:text, on:bool",
        hidden(act(
            "package.tick",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_package_tick(p),
        )),
    ),
    with_params(
        "on:bool",
        hidden(act(
            "package.personal",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_package_personal(p),
        )),
    ),
    with_params(
        "key:text, on?:bool",
        hidden(act(
            "package.show_finding",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_package_show_finding(p),
        )),
    ),
    with_params(
        "key:text",
        hidden(act(
            "package.fix",
            Command,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_package_fix(p),
        )),
    ),
    hidden(act(
        "package.messages",
        View,
        Menu::Output,
        &[],
        project_open,
        None,
        |s, _| s.act_package_messages(),
    )),
    with_params(
        "path?:path",
        hidden(act(
            "package.file",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_package_file(p),
        )),
    ),
    // Output ▸ Workshop ▸ Workshop settings: the project's Properties at its Workshop rows (doc 13 §12).
    placed(
        place(Group::Workshop, 9, CommandSize::Small, None),
        "file-settings",
        act(
            "workshop.settings",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, _| s.act_workshop_settings(),
        ),
    ),
    with_params(
        "path?:path",
        hidden(act(
            "rework.archive",
            View,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_rework_archive(p),
        )),
    ),
    with_params(
        "path?:path",
        hidden(act(
            "rework.export",
            Command,
            Menu::Output,
            &[],
            project_open,
            None,
            |s, p| s.act_rework_export(p),
        )),
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
    // The Messages pane's choice of the migration report beside the project's scheme.
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
    // View ▸ Messages, F8 and Shift+F8, the quick fix of a finding row.
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
    // Fix all of a group of findings whose fixes are of one kind: one card, one Apply.
    with_params(
        "key:text",
        hidden(in_navigator(act(
            "messages.fix_all",
            Command,
            Menu::Check,
            &[],
            writable,
            None,
            |s, p| s.act_messages_fix_all(p),
        ))),
    ),
    // Waive… and Unwaive in the Messages detail (`check.waive`, `check.unwaive`; window only).
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
        placed(
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
        ),
    ),
    // View ▸ Minimum text size (`pt` 0 is off, else 8 to 16) and View ▸ Reduce motion,
    // the settings `min_font_pt` and `reduce_motion`.
    with_params(
        "pt:uint",
        act(
            "view.min_font",
            View,
            Menu::View,
            &[],
            always,
            sub(Menu::View, Submenu::MinFont, true),
            |s, p| s.act_min_font(p),
        ),
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
    // View ▸ Mouse wheel scrolls, the setting `wheel_scrolls`; off, the wheel zooms.
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
    // View ▸ Show tips, the setting `tips` (FR-UI-048), beside Reduce motion, and
    // a switch of the View tab's Show group too, so its menu item is View ▸ Show ▸ Show tips.
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
    // View ▸ Theme and View ▸ Density (doc 16a §4.13). The theme words of the spec
    // (`hc-dark`, `hc-light`) are not words of the signature grammar, so `value` is text, checked against `THEMES`.
    with_params(
        "value:text",
        placed(
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
        ),
    ),
    with_params(
        "value:enum(comfortable|compact)",
        placed(
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
        ),
    ),
    // View ▸ Language switches the UI language at run time; `value` is a
    // value of the `language` setting.
    with_params(
        "value:text",
        // The palette row's icon (doc 16a §6.14).
        with_icon(
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
        ),
    ),
    // The command bar's tab and its collapse (Ctrl+F1, a tab's double click), view actions.
    keeps_banner(with_params(
        "tab:enum(@TABS)",
        enabled_when_named(
            &["tab"],
            tab_named,
            act(
                "view.command_tab",
                View,
                Menu::View,
                &[],
                always,
                grp_sub(Group::Layout, Submenu::CommandTab),
                |s, p| s.act_command_tab(p),
            ),
        ),
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
        placed(
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
        ),
    ),
    with_params(
        "name?:text",
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
