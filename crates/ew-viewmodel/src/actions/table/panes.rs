// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `panes`. The panes' own controls: the palette, the navigators, table mode, focus.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    with_params(
        "cmd:text, params?:json",
        needs_parameters(with_aliases(
            &[("params", ParamForm::CommandRefs)],
            act("command.run", Command, Menu::View, &[], writable, None, |s, p| {
                s.act_command_run(p)
            }),
        )),
    ),
    keeps_link_preview(keeps_banner(with_params(
        "path:path, dpi?:uint",
        headless(act(
            "view.snapshot",
            View,
            Menu::View,
            &[],
            page_shown,
            None,
            |s, p| s.act_snapshot(p),
        )),
    ))),
    keeps_link_preview(keeps_banner(with_params(
        "path:path, group?:uint",
        headless(act(
            "canvas.snapshot",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_snapshot(p),
        )),
    ))),
    keeps_banner(with_params(
        "percent:json, x_mm?:mm, y_mm?:mm, group?:uint",
        hidden(act(
            "view.zoom_to",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_zoom_to(p),
        )),
    )),
    keeps_banner(with_params(
        "dx:json, dy:json, group?:uint",
        hidden(act("view.pan", View, Menu::View, &[], has_pages, None, |s, p| {
            s.act_pan(p)
        })),
    )),
    answers_recovery(keeps_banner(with_params(
        "width:json, height:json, ppp?:json, group?:uint",
        hidden(act(
            "view.canvas_resize",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_canvas_resize(p),
        )),
    ))),
    keeps_banner(with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint, shift?:bool, ctrl?:bool, alt?:bool",
        hidden(act(
            "canvas.pointer",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_pointer(p),
        )),
    )),
    keeps_link_preview(keeps_banner(with_params(
        "path:path",
        headless(act(
            "canvas.preview_snapshot",
            View,
            Menu::View,
            &[],
            has_link_preview,
            None,
            |s, p| s.act_snapshot(p),
        )),
    ))),
    keeps_banner(with_params(
        "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint",
        hidden(in_canvas(act(
            "canvas.press",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_press(p, false),
        ))),
    )),
    keeps_banner(with_params(
        "x_mm:mm, y_mm:mm, button?:enum(primary|middle), shift?:bool, ctrl?:bool, alt?:bool, group?:uint",
        hidden(in_canvas(act(
            "canvas.release",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_press(p, true),
        ))),
    )),
    keeps_link_preview(keeps_banner(with_params(
        "ms:uint",
        hidden(in_canvas(act(
            "canvas.dwell",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_dwell(p),
        ))),
    ))),
    // A script step that expects its inner step to be refused, so a refusal's window state can be framed.
    with_params(
        "action:text, params?:json",
        headless(act(
            "script.expect_refused",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_script_expect_refused(p),
        )),
    ),
    // A script step that expects its inner step to fail, so a failure's window state (a form's error) can be
    // framed.
    with_params(
        "action:text, params?:json",
        headless(act(
            "script.expect_failed",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_script_expect_failed(p),
        )),
    ),
    keeps_link_preview(answers_recovery(keeps_banner(headless(act(
        "ui.describe",
        View,
        Menu::View,
        &[],
        always,
        None,
        |s, p| s.act_ui_describe(p),
    ))))),
    with_params(
        "folio:text",
        hidden(act(
            "page.goto",
            View,
            Menu::Page,
            &[],
            has_pages,
            None,
            |s, p| s.act_page_goto(p),
        )),
    ),
    keeps_banner(with_params(
        "group:uint, index:uint",
        hidden(act(
            "tab.activate",
            View,
            Menu::Window,
            &[],
            has_pages,
            None,
            |s, p| s.act_tab_activate(p),
        )),
    )),
    keeps_banner(with_params(
        "group:uint, index:uint, to_group:uint, to_index:uint",
        hidden(act(
            "tab.move",
            View,
            Menu::Window,
            &[],
            has_pages,
            None,
            |s, p| s.act_tab_move(p),
        )),
    )),
    with_params(
        "folio:text, group?:uint",
        hidden(act(
            "tab.open",
            View,
            Menu::Window,
            &[],
            has_pages,
            None,
            |s, p| s.act_tab_open(p),
        )),
    ),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.left",
            View,
            Menu::Page,
            &["Arrow Left"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, -1, 0, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.right",
            View,
            Menu::Page,
            &["Arrow Right"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 1, 0, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.up",
            View,
            Menu::Page,
            &["Arrow Up"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, -1, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.down",
            View,
            Menu::Page,
            &["Arrow Down"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, 1, false),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_left",
            View,
            Menu::Page,
            &["Shift+Arrow Left"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, -1, 0, true),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_right",
            View,
            Menu::Page,
            &["Shift+Arrow Right"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 1, 0, true),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_up",
            View,
            Menu::Page,
            &["Shift+Arrow Up"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, -1, true),
        ))),
    )),
    keeps_banner(with_params(
        "group?:uint",
        hidden(in_canvas(act(
            "cursor.cell_down",
            View,
            Menu::Page,
            &["Shift+Arrow Down"],
            has_pages,
            None,
            |s, p| s.act_cursor(p, 0, 1, true),
        ))),
    )),
    keeps_banner(with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint",
        hidden(in_canvas(act(
            "cursor.set",
            View,
            Menu::Page,
            &[],
            has_pages,
            None,
            |s, p| s.act_cursor_set(p),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "pages.filter",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_pages_filter(p),
        )),
    )),
    keeps_banner(with_params(
        "key:text",
        hidden(in_navigator(with_aliases(
            &[("key", ParamForm::Embedded)],
            act("pages.toggle", View, Menu::View, &[], has_pages, None, |s, p| {
                s.act_pages_toggle(p)
            }),
        ))),
    )),
    keeps_banner(with_params(
        "key?:text, step?:int, to?:enum(first|last)",
        hidden(in_navigator(with_aliases(
            &[("key", ParamForm::Embedded)],
            act("pages.row", View, Menu::View, &[], has_pages, None, |s, p| {
                s.act_pages_row(p)
            }),
        ))),
    )),
    keeps_banner(with_params(
        "key?:text, folio?:text, new_tab?:bool",
        hidden(in_navigator(with_aliases(
            &[("key", ParamForm::Embedded)],
            act("pages.open", View, Menu::View, &[], has_pages, None, |s, p| {
                s.act_pages_open(p)
            }),
        ))),
    )),
    // The navigator framework (§6.9).
    keeps_banner(with_params(
        "kind:enum(pages|devices|connections|strips|cables|plc|potentials|ips|structure|parts|blocks|unplaced|macros|libraries|messages), on?:bool",
        needs_parameters(placed(
            place(Group::Panels, 1, CommandSize::Small, Some(key!("bar-navigators"))),
            "layout-sidebar",
            act(
                "navigator.show",
                View,
                Menu::View,
                &[],
                has_pages,
                sub(Menu::View, Submenu::Navigators, false),
                |s, p| s.act_navigator_show(p),
            ),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, view:text",
        hidden(act(
            "navigator.view",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_view(p),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, by:text, descending?:bool",
        hidden(act(
            "navigator.sort",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_sort(p),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, text:text",
        hidden(act(
            "navigator.filter",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_filter(p),
        )),
    )),
    keeps_banner(with_params(
        "kind:text, key:text",
        hidden(in_navigator(with_aliases(
            &[("key", ParamForm::Embedded)],
            act(
                "navigator.toggle",
                View,
                Menu::View,
                &[],
                has_pages,
                None,
                |s, p| s.act_navigator_toggle(p),
            ),
        ))),
    )),
    keeps_banner(with_params(
        "kind:text, key?:text, step?:int, to?:enum(first|last), extend?:bool",
        hidden(in_navigator(with_aliases(
            &[("key", ParamForm::Embedded)],
            act("navigator.row", View, Menu::View, &[], has_pages, None, |s, p| {
                s.act_navigator_row(p)
            }),
        ))),
    )),
    keeps_banner(with_params(
        "kind:text, key?:text, new_tab?:bool",
        hidden(in_navigator(with_aliases(
            &[("key", ParamForm::Embedded)],
            act(
                "navigator.open",
                View,
                Menu::View,
                &[],
                has_pages,
                None,
                |s, p| s.act_navigator_open(p),
            ),
        ))),
    )),
    // Draw ▸ Insert macro… with the place tools; a macro row dropped on the canvas; the macro
    // card's value set; Edit ▸ Save as macro…; the Libraries navigator's Update…; a property row's Make variable.
    with_params(
        "key?:text",
        in_canvas(act(
            "macros.insert",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            grp(Group::Place),
            |s, p| s.act_macros_insert(p),
        )),
    ),
    with_params(
        "key:text, x_mm:mm, y_mm:mm, value_set?:text, group?:uint",
        hidden(in_canvas(act(
            "macros.place",
            Command,
            Menu::Draw,
            &[],
            editable_page,
            None,
            |s, p| s.act_macros_place(p),
        ))),
    ),
    with_params(
        "name?:text",
        hidden(act(
            "macros.value_set",
            Command,
            Menu::Draw,
            &[],
            writable,
            None,
            |s, p| s.act_macros_value_set(p),
        )),
    ),
    // An ad-hoc value of the macro card's variable `name` (empty or absent `value`: none).
    with_params(
        "name:text, value?:text",
        hidden(act(
            "macros.value",
            Command,
            Menu::Draw,
            &[],
            writable,
            None,
            |s, p| s.act_macros_value(p),
        )),
    ),
    with_params(
        "name?:text, key?:text, description?:text, library?:text, kind?:text, pages?:list(text), x_mm?:mm, \
         y_mm?:mm, value_sets?:json, confirm?:bool",
        in_canvas(act(
            "macros.save",
            Command,
            Menu::Edit,
            &[],
            macro_scope_shown,
            slot(Menu::Edit),
            |s, p| s.act_macros_save(p),
        )),
    ),
    with_params(
        "keys?:list(text)",
        in_navigator(act(
            "libraries.update",
            Command,
            Menu::Project,
            &[],
            writable,
            slot(Menu::Project),
            |s, p| s.act_libraries_update(p),
        )),
    ),
    with_params(
        "row?:text, name?:text",
        hidden(in_properties(act(
            "props.make_variable",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_make_variable(p),
        ))),
    ),
    // A channel row dropped on the canvas (FR-PLC-020) binds the channel to the connection point under
    // the drop point with `plc.bind_channel`.
    with_params(
        "channel:ref, x_mm:mm, y_mm:mm, group?:uint",
        hidden(in_canvas(with_aliases(
            &[("channel", ParamForm::Whole)],
            act(
                "navigator.bind",
                Command,
                Menu::Edit,
                &[],
                editable_page,
                None,
                |s, p| s.act_navigator_bind(p),
            ),
        ))),
    ),
    keeps_banner(with_params(
        "kind:text, keys:list(text), add?:bool",
        hidden(with_aliases(
            &[("keys", ParamForm::EmbeddedList)],
            act(
                "navigator.select",
                View,
                Menu::View,
                &[],
                has_pages,
                None,
                |s, p| s.act_navigator_select(p),
            ),
        )),
    )),
    keeps_banner(with_params(
        "kind?:text, key?:text",
        hidden(in_navigator(with_aliases(
            &[("key", ParamForm::Embedded)],
            act(
                "navigator.mark",
                View,
                Menu::View,
                &["Space"],
                has_pages,
                None,
                |s, p| s.act_navigator_mark(p),
            ),
        ))),
    )),
    keeps_banner(with_params(
        "kind:text, prefix:text",
        hidden(in_navigator(act(
            "navigator.jump",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_jump(p),
        ))),
    )),
    hidden(in_navigator(act(
        "navigator.apply",
        Command,
        Menu::Edit,
        &[],
        reorder_card,
        None,
        |s, _| s.act_navigator_apply(),
    ))),
    with_params(
        "pages?:list(text), before?:text, end?:bool",
        hidden(in_navigator(act(
            "pages.reorder",
            Command,
            Menu::Page,
            &[],
            pages_writable,
            None,
            |s, p| s.act_pages_reorder(p),
        ))),
    ),
    in_navigator(placed(
        place(Group::Order, 2, CommandSize::Small, Some(key!("bar-move-up"))),
        "arrow-up",
        act(
            "pages.move_up",
            Command,
            Menu::Page,
            &["Alt+Arrow Up"],
            pages_writable,
            None,
            |s, _| s.act_pages_move(true),
        ),
    )),
    in_navigator(placed(
        place(Group::Order, 3, CommandSize::Small, Some(key!("bar-move-down"))),
        "arrow-down",
        act(
            "pages.move_down",
            Command,
            Menu::Page,
            &["Alt+Arrow Down"],
            pages_writable,
            None,
            |s, _| s.act_pages_move(false),
        ),
    )),
    // Potential and net highlighting (§7.4).
    keeps_banner(with_params(
        "kind?:text, key?:text, off?:bool",
        in_navigator(placed(
            place(Group::Trace, 1, CommandSize::Small, None),
            "highlight",
            act(
                "navigator.highlight",
                View,
                Menu::Check,
                &["Ctrl+Shift+H"],
                has_pages,
                None,
                |s, p| s.act_navigator_highlight(p),
            ),
        )),
    )),
    keeps_banner(placed(
        place(Group::Trace, 2, CommandSize::Small, None),
        "highlight-off",
        act(
            "view.clear_highlights",
            View,
            Menu::Check,
            &[],
            has_highlights,
            None,
            |s, _| s.act_clear_highlights(),
        ),
    )),
    // Table mode (§8.8).
    keeps_banner(with_params(
        "kind:text, on?:bool",
        hidden(act(
            "navigator.table",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_navigator_table(p),
        )),
    )),
    // The table on show to a file as a job, and a round trip's file back
    // through the Preview tab; in the table's Columns menu and the Output menu.
    with_params(
        "path?:path, format?:enum(xlsx|csv), preset?:text, replace?:bool, wait?:bool",
        act(
            "navigator.export_table",
            Command,
            Menu::Output,
            &[],
            table_shown,
            grp(Group::Lists),
            |s, p| s.act_export_table(p),
        ),
    ),
    with_params(
        "path?:path, take_file?:bool",
        act(
            "navigator.import_table",
            Command,
            Menu::Output,
            &[],
            table_shown,
            grp(Group::Lists),
            |s, p| s.act_import_table(p),
        ),
    ),
    keeps_banner(with_params(
        "kind:text, show?:list(text), hide?:list(text), order?:list(text)",
        hidden(act(
            "table.columns",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_table_columns(p),
        )),
    )),
    keeps_banner(with_params(
        "kind?:text, row?:text, column?:text, step?:int, across?:int",
        hidden(in_navigator(act(
            "table.cell",
            View,
            Menu::View,
            &[],
            has_pages,
            None,
            |s, p| s.act_table_cell(p),
        ))),
    )),
    keeps_banner(with_params(
        "kind?:text, row?:text, column?:text",
        hidden(in_navigator(act(
            "table.edit",
            View,
            Menu::Edit,
            &["F2"],
            pages_writable,
            None,
            |s, p| s.act_table_edit(p),
        ))),
    )),
    keeps_banner(with_params(
        "kind?:text, text:text",
        hidden(in_navigator(act(
            "table.input",
            View,
            Menu::Edit,
            &[],
            pages_writable,
            None,
            |s, p| s.act_table_input(p),
        ))),
    )),
    with_params(
        "kind?:text, all?:bool",
        hidden(in_navigator(act(
            "table.commit",
            Command,
            Menu::Edit,
            &[],
            pages_writable,
            None,
            |s, p| s.act_table_commit(p),
        ))),
    ),
    with_params(
        "kind:text, column:text, value:text, rows?:list(text), all?:bool",
        hidden(act(
            "table.set",
            Command,
            Menu::Edit,
            &[],
            pages_writable,
            None,
            |s, p| s.act_table_set(p),
        )),
    ),
    with_params(
        "kind?:text",
        in_navigator(act(
            "table.fill_down",
            Command,
            Menu::Edit,
            &["Ctrl+D"],
            pages_writable,
            None,
            |s, p| s.act_table_fill_down(p),
        )),
    ),
    with_params(
        "kind?:text, pattern?:text",
        in_navigator(act(
            "table.pattern_fill",
            Command,
            Menu::Edit,
            &["Ctrl+Shift+D"],
            pages_writable,
            None,
            |s, p| s.act_table_pattern_fill(p),
        )),
    ),
    with_params(
        "kind?:text, text?:text",
        in_navigator(act(
            "table.paste",
            Command,
            Menu::Edit,
            &["Ctrl+V"],
            pages_writable,
            None,
            |s, p| s.act_table_paste(p),
        )),
    ),
    keeps_banner(with_params(
        "kind?:text",
        in_navigator(act(
            "table.copy",
            View,
            Menu::Edit,
            &["Ctrl+C"],
            has_pages,
            None,
            |s, p| s.act_table_copy(p),
        )),
    )),
    with_params(
        "kind?:text",
        in_navigator(act(
            "table.clear",
            Command,
            Menu::Edit,
            &["Delete"],
            pages_writable,
            None,
            |s, p| s.act_table_clear(p),
        )),
    ),
    with_params(
        "cell:uint, choice:uint",
        hidden(in_navigator(act(
            "table.resolve",
            Command,
            Menu::Edit,
            &[],
            table_card,
            None,
            |s, p| s.act_table_resolve(p),
        ))),
    ),
    with_params(
        "cell:uint",
        hidden(in_navigator(act(
            "table.drop",
            Command,
            Menu::Edit,
            &[],
            table_card,
            None,
            |s, p| s.act_table_drop(p),
        ))),
    ),
    keeps_banner(with_params(
        "area:enum(navigator|canvas|properties|find|commandbar|history), group?:uint, kind?:text",
        hidden(act(
            "ui.focus",
            View,
            Menu::Window,
            &[],
            project_open,
            None,
            |s, p| s.act_focus(p),
        )),
    )),
    keeps_banner(hidden(act(
        "ui.focus_next",
        View,
        Menu::Window,
        &["F6"],
        project_open,
        None,
        |s, _| s.act_focus_cycle(1),
    ))),
    keeps_banner(hidden(act(
        "ui.focus_previous",
        View,
        Menu::Window,
        &["Shift+F6"],
        project_open,
        None,
        |s, _| s.act_focus_cycle(-1),
    ))),
    keeps_link_preview(answers_recovery(keeps_banner(hidden(act(
        "ui.dismiss",
        View,
        Menu::View,
        &["Escape"],
        always,
        None,
        |s, _| s.act_dismiss(),
    ))))),
    with_params(
        "key:text, value:json",
        hidden(act(
            "settings.set",
            Command,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_settings_set(p),
        )),
    ),
];
