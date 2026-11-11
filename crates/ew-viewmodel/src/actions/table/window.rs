// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `window`. Revisions and the release, the canvas tabs, help.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    // Check ▸ Revision, the revision dialogs; each answered by its own action with parameters
    // (`confirm` submits the `HumanOnly` command, only from the window a person drives).
    // In the order of the workflow: the author, the sign-off, the release, a withdrawal.
    with_params(
        "role?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.set_author",
            Command,
            Menu::Check,
            &[],
            open_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_set_author(p),
        ),
    ),
    with_params(
        "role?:text, typed?:text, date?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.approve",
            Command,
            Menu::Check,
            &[],
            open_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_approve(p),
        ),
    ),
    with_params(
        "typed?:text, git?:bool, date?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.release",
            Command,
            Menu::Check,
            &[],
            open_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_release(p),
        ),
    ),
    with_params(
        "revision?:text, reason?:text, confirm?:bool, cancel?:bool",
        act(
            "revision.withdraw",
            Command,
            Menu::Check,
            &[],
            released_revision,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_withdraw(p),
        ),
    ),
    with_params(
        "how?:enum(complete|discard), git?:bool, confirm?:bool, cancel?:bool",
        act(
            "revision.incomplete",
            Command,
            Menu::Check,
            &[],
            incomplete_release,
            sub(Menu::Check, Submenu::Revision, true),
            |s, p| s.act_revision_incomplete(p),
        ),
    ),
    keeps_banner(act(
        "tab.new",
        View,
        Menu::Window,
        &["Ctrl+T"],
        can_add_tab,
        slot(Menu::Window),
        |s, _| s.act_tab_new(),
    )),
    keeps_banner(act(
        "tab.close",
        View,
        Menu::Window,
        &["Ctrl+W"],
        can_close_tab,
        slot(Menu::Window),
        |s, _| s.act_tab_close(),
    )),
    keeps_banner(act(
        "tab.next",
        View,
        Menu::Window,
        &["Ctrl+Tab"],
        can_cycle_tabs,
        slot(Menu::Window),
        |s, _| s.act_tab_cycle(1),
    )),
    keeps_banner(act(
        "tab.previous",
        View,
        Menu::Window,
        &["Ctrl+Shift+Tab"],
        can_cycle_tabs,
        slot(Menu::Window),
        |s, _| s.act_tab_cycle(-1),
    )),
    keeps_banner(act(
        "tab.focus_other",
        View,
        Menu::Window,
        &["Ctrl+F6"],
        two_groups,
        slot(Menu::Window),
        |s, _| s.act_focus_other(),
    )),
    answers_recovery(keeps_banner(with_params(
        "topic?:text",
        act(
            "help.documentation",
            View,
            Menu::Help,
            &["F1"],
            always,
            slot(Menu::Help),
            |s, p| s.act_documentation(p),
        ),
    ))),
    with_params(
        "dir?:path, discard?:bool, tutorial?:text",
        act(
            "help.tutorials",
            Command,
            Menu::Help,
            &[],
            always,
            sub(Menu::Help, Submenu::Tutorials, false),
            |s, p| s.act_tutorials(p),
        ),
    ),
    with_params(
        "step?:enum(language|origin|privacy|appearance|start)",
        act(
            "help.first_run",
            View,
            Menu::Help,
            &[],
            always,
            slot_sep(Menu::Help),
            |s, p| s.act_first_run_open(p),
        ),
    ),
    keeps_banner(act(
        "help.shortcuts",
        View,
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
        |s, p| s.act_help_shortcuts(p),
    )),
    // Help ▸ Glossary, the glossary pane in the bottom dock, and its search text.
    keeps_banner(with_params(
        "on?:bool",
        act(
            "help.glossary",
            View,
            Menu::Help,
            &[],
            project_open,
            slot(Menu::Help),
            |s, p| s.act_help_glossary(p),
        ),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(act(
            "glossary.filter",
            View,
            Menu::Help,
            &[],
            project_open,
            None,
            |s, p| s.act_glossary_filter(p),
        )),
    )),
    // The glossary pane's row cursor (Up and Down in its field).
    keeps_banner(with_params(
        "step?:int, to?:uint",
        hidden(act(
            "glossary.row",
            View,
            Menu::Help,
            &[],
            project_open,
            None,
            |s, p| s.act_glossary_row(p),
        )),
    )),
    keeps_banner(act(
        "help.notices",
        View,
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
        |s, _| s.open_dialog(crate::shell::Dialog::Notices),
    )),
    keeps_banner(act(
        "help.about",
        View,
        Menu::Help,
        &[],
        always,
        slot(Menu::Help),
        |s, _| s.open_dialog(crate::shell::Dialog::About),
    )),
];
