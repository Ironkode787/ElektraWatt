// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `page`. Pages: new, renumber, next and previous, go to, the keyboard cursor on the sheet.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::enabled::*;
use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    // Page, New page.
    with_params(
        "folio:text, type?:enum(title_page|table_of_contents|section_divider|graphic|single_line|multi_line|overview|bus_topology|fluid|assembly_overview|panel_layout|model_view|external_document), title?:text, section?:text, place?:enum(after|end), grid?:enum(mm4|qet_10px), at?:text",
        placed(
            place(Group::Pages, 1, CommandSize::Large, None),
            "file-plus",
            in_symbol_editor(act(
                "page.new",
                Command,
                Menu::Page,
                &[],
                writable,
                None,
                |s, p| s.act_page_new(p),
            )),
        ),
    ),
    // The question before a page's Connection grid changes under its items: Change and fit.
    with_params(
        "grid:enum(mm4|qet_10px)",
        hidden(act(
            "page.fit_grid",
            Command,
            Menu::Page,
            &[],
            writable_page,
            None,
            |s, p| s.act_page_fit_grid(p),
        )),
    ),
    // Page, Title block fields…: the focused page's title-block fields in one form, for the title block's
    // right-click menu too. Its parameters are the title-block paths the form shows (`project.drawing_id`), which no
    // signature word can spell.
    placed(
        place(Group::Pages, 2, CommandSize::Small, None),
        "layout-bottombar",
        act(
            "page.title_block",
            Command,
            Menu::Page,
            &[],
            writable_page,
            None,
            |s, p| s.act_page_title_block(p),
        ),
    ),
    // Page, Connection grid…: the page's grid, asked and fitted where it holds items.
    // In the Order group: a fourth entry of Pages would turn that group into a submenu.
    placed(
        place(Group::Order, 8, CommandSize::Small, None),
        "ew-qet-grid",
        act(
            "page.grid",
            Command,
            Menu::Page,
            &[],
            writable_page,
            None,
            |s, _| s.act_page_grid(),
        ),
    ),
    // Add legend: the panel page's item list beside the layout, with its button on the Page tab.
    placed(
        place(Group::Order, 9, CommandSize::Small, None),
        "table-plus",
        act(
            "page.add_legend",
            Command,
            Menu::Page,
            &[],
            legend_addable,
            None,
            |s, _| s.act_page_add_legend(),
        ),
    ),
    // Move the legend to its own page: an older legend box becomes its layout's legend page (doc 12 §5.4); in Page ▸
    // Order and the legend's context menu, no button.
    act(
        "report_block.legend_to_page",
        Command,
        Menu::Page,
        &[],
        legend_movable,
        grp(Group::Order),
        |s, _| s.act_report_block_legend_to_page(),
    ),
    // Fit the layout to the sheet: the layout page takes the whole sheet at the fitting scale, centred, as a new view
    // does (doc 12 §5.1), once its legend box has gone to its own page; in Page ▸ Order, the page's context menu and
    // Enclosure's split on the Draw tab's layout group.
    act(
        "layout.fit_sheet",
        Command,
        Menu::Page,
        &[],
        sheet_fittable,
        grp(Group::Order),
        |s, _| s.act_layout_fit_sheet(),
    ),
    // Page, New section…
    with_params(
        "title?:text, first:uint, last:uint, number?:uint",
        placed(
            place(Group::Pages, 4, CommandSize::Small, None),
            "folder-plus",
            act("section.new", Command, Menu::Page, &[], writable, None, |s, p| {
                s.act_section_new(p)
            }),
        ),
    ),
    // Edit section… (the Pages list's section row, Project properties): a section's title and folio range.
    with_params(
        "number:uint, title?:text, first?:uint, last?:uint",
        hidden(act(
            "section.edit",
            Command,
            Menu::Page,
            &[],
            writable,
            None,
            |s, p| s.act_section_edit(p),
        )),
    ),
    // Page, Renumber pages…
    with_params(
        "section?:text, start?:uint, step?:uint, keep_gaps?:bool",
        placed(
            place(Group::Order, 1, CommandSize::Small, None),
            "list-numbers",
            act(
                "pages.renumber",
                Command,
                Menu::Page,
                &[],
                writable,
                None,
                |s, p| s.act_pages_renumber(p),
            ),
        ),
    ),
    // Page, Copy pages…: the marked pages copied for a second installation (doc 06 §3.6); also on a page
    // row's right-click menu.
    with_params(
        "pages?:list(text), offset?:uint, after?:text, node?:text, code?:text, rename?:json",
        placed(
            place(Group::Order, 4, CommandSize::Small, None),
            "copy",
            act("pages.copy", Command, Menu::Page, &[], writable, None, |s, p| {
                s.act_pages_copy(p)
            }),
        ),
    ),
    act(
        "page.next",
        View,
        Menu::Page,
        &["Page Down"],
        has_next,
        grp(Group::Go),
        |s, _| s.walk(s.page_index() + 1),
    ),
    act(
        "page.previous",
        View,
        Menu::Page,
        &["Page Up"],
        has_previous,
        grp(Group::Go),
        |s, _| s.walk(s.page_index().saturating_sub(1)),
    ),
    act(
        "page.first",
        View,
        Menu::Page,
        &[],
        has_pages,
        grp(Group::Go),
        |s, _| s.walk(0),
    ),
    act(
        "page.last",
        View,
        Menu::Page,
        &[],
        has_pages,
        grp(Group::Go),
        |s, _| s.walk(s.page_count().saturating_sub(1)),
    ),
    with_params(
        "text?:text",
        placed(
            place(Group::Go, 1, CommandSize::Large, Some(key!("bar-go-to"))),
            "current-location",
            act(
                "nav.goto",
                View,
                Menu::Page,
                &["Ctrl+G"],
                has_pages,
                None,
                |s, p| s.act_nav_goto(p),
            ),
        ),
    ),
    keeps_banner(placed(
        place(Group::Go, 2, CommandSize::Small, None),
        "arrow-left",
        act(
            "nav.back",
            View,
            Menu::Page,
            &["Alt+Arrow Left"],
            can_go_back,
            None,
            |s, _| s.act_history(true),
        ),
    )),
    keeps_banner(placed(
        place(Group::Go, 3, CommandSize::Small, None),
        "arrow-right",
        act(
            "nav.forward",
            View,
            Menu::Page,
            &["Alt+Arrow Right"],
            can_go_forward,
            None,
            |s, _| s.act_history(false),
        ),
    )),
    keeps_banner(with_params(
        "x_mm?:mm, y_mm?:mm, group?:uint",
        in_canvas(placed(
            place(
                Group::Go,
                4,
                CommandSize::Small,
                Some(key!("bar-follow-reference")),
            ),
            "link",
            act("nav.follow", View, Menu::Page, &["F"], has_pages, None, |s, p| {
                s.act_follow(p)
            }),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "nav.next_link",
            View,
            Menu::Page,
            &["Shift+F"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_link_step(p, true),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "nav.previous_link",
            View,
            Menu::Page,
            &["Ctrl+Shift+F"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_link_step(p, false),
        )),
    )),
    // Keyboard tool input: the connection points in reading order; `[` and `]` are matched as the
    // physical keys by the window, so they need no AltGr on QWERTZ and AZERTY (doc 16 §4.4).
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "cp.next",
            View,
            Menu::Page,
            &["]", "Alt+N"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_cp_step(p, crate::shell::tools::CpStep::Next),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "cp.previous",
            View,
            Menu::Page,
            &["[", "Alt+Shift+N"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_cp_step(p, crate::shell::tools::CpStep::Previous),
        )),
    )),
    keeps_banner(with_params(
        "group?:uint",
        in_canvas(act(
            "cp.next_unconnected",
            View,
            Menu::Page,
            &["N"],
            has_pages,
            grp(Group::Go),
            |s, p| s.act_cp_step(p, crate::shell::tools::CpStep::NextUnconnected),
        )),
    )),
    keeps_banner(with_params(
        "alt?:bool",
        in_canvas(act(
            "canvas.activate",
            View,
            Menu::Page,
            &["Enter"],
            has_pages,
            None,
            |s, p| s.act_activate_with(p),
        )),
    )),
    keeps_banner(in_canvas(act(
        "tool.back",
        View,
        Menu::Page,
        &["Backspace"],
        has_pages,
        None,
        |s, _| s.act_tool_back(),
    ))),
];
