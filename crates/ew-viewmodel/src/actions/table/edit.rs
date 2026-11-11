// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `edit`. Undo and redo, the history, the properties of the selection, find.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    with_icon(
        "arrow-back-up",
        act(
            "edit.undo",
            Command,
            Menu::Edit,
            &["Ctrl+Z"],
            can_undo,
            slot(Menu::Edit),
            |s, _| s.act_undo(),
        ),
    ),
    with_icon(
        "arrow-forward-up",
        act(
            "edit.redo",
            Command,
            Menu::Edit,
            &["Ctrl+Shift+Z", "Ctrl+Y"],
            can_redo,
            slot(Menu::Edit),
            |s, _| s.act_redo(),
        ),
    ),
    // Edit, History.
    keeps_banner(with_params(
        "on?:bool",
        placed(
            place(Group::Panels, 4, CommandSize::Small, None),
            "history",
            act(
                "history.show",
                View,
                Menu::View,
                &[],
                project_open,
                None,
                |s, p| s.act_history_show(p),
            ),
        ),
    )),
    // Doc 16a §5.7: View ▸ Bottom dock (Ctrl+J) closes the bottom dock and opens it again with the panes it had.
    keeps_banner(with_params(
        "on?:bool",
        act(
            "view.bottom_dock",
            View,
            Menu::View,
            &["Ctrl+J"],
            project_open,
            grp(Group::Panels),
            |s, p| s.act_bottom_dock(p),
        ),
    )),
    // Edit, Properties of selection (E and Alt+Enter on the canvas) and Project properties: the shell has no Project
    // menu yet, so the project's properties sit beside the selection's.
    keeps_banner(in_canvas(act(
        "props.focus",
        View,
        Menu::Edit,
        &["E", "Alt+Enter"],
        project_open,
        slot_sep(Menu::Edit),
        |s, _| s.act_props_focus(),
    ))),
    keeps_banner(placed(
        place(
            Group::Project,
            1,
            CommandSize::Large,
            Some(key!("bar-properties")),
        ),
        "adjustments",
        act(
            "props.project",
            View,
            Menu::Project,
            &[],
            project_open,
            None,
            |s, _| s.act_props_project(),
        ),
    )),
    // Edit, Find…, Find and replace… and Replace all (§7.6). macOS's ⌥⌘F (doc 16 §4.4) needs a
    // per-platform binding: `Ctrl+Alt+F` would be a Ctrl+Alt+letter chord, which NFR-I18N-06 forbids.
    keeps_banner(act(
        "find.open",
        View,
        Menu::Edit,
        &["Ctrl+F"],
        project_open,
        slot_sep(Menu::Edit),
        |s, _| s.act_find_open(false),
    )),
    keeps_banner(act(
        "find.open_replace",
        View,
        Menu::Edit,
        &["Ctrl+H"],
        project_open,
        slot(Menu::Edit),
        |s, _| s.act_find_open(true),
    )),
    keeps_banner(in_find(act(
        "find.replace_all",
        Command,
        Menu::Edit,
        &["Ctrl+Enter"],
        find_writable,
        slot(Menu::Edit),
        |s, _| s.act_find_replace_all(),
    ))),
];
