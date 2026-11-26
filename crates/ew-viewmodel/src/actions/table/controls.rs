// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `controls`. The controls of screens and dialogs: first run, the wizard, the property grid, previews, jobs, the find pane, the part dialog, the keymap editor, the symbol editor.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::enabled::*;
use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    with_params(
        "field:text, value:json",
        hidden(act(
            "first_run.set",
            View,
            Menu::Help,
            &[],
            first_run_open,
            None,
            |s, p| s.act_first_run_set(p),
        )),
    ),
    hidden(act(
        "first_run.next",
        View,
        Menu::Help,
        &[],
        first_run_open,
        None,
        |s, _| s.act_first_run_step(1),
    )),
    hidden(act(
        "first_run.back",
        View,
        Menu::Help,
        &[],
        first_run_open,
        None,
        |s, _| s.act_first_run_step(-1),
    )),
    with_params(
        "start?:enum(wizard|sample|qet|new|none)",
        hidden(act(
            "first_run.finish",
            Command,
            Menu::Help,
            &[],
            first_run_open,
            None,
            |s, p| s.act_first_run_finish(p),
        )),
    ),
    with_params(
        "field:text, value:text",
        hidden(act(
            "wizard.set",
            View,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_set(p),
        )),
    ),
    hidden(act(
        "wizard.next",
        View,
        Menu::Project,
        &[],
        wizard_open,
        None,
        |s, _| s.act_wizard_step(1),
    )),
    hidden(act(
        "wizard.back",
        View,
        Menu::Project,
        &[],
        wizard_open,
        None,
        |s, _| s.act_wizard_step(-1),
    )),
    with_params(
        "step:uint",
        hidden(act(
            "wizard.goto",
            View,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_goto(p),
        )),
    ),
    hidden(act(
        "wizard.reset",
        View,
        Menu::Project,
        &[],
        wizard_open,
        None,
        |s, _| s.act_wizard_reset(),
    )),
    with_params(
        "answer?:enum(keep|discard|stay)",
        hidden(act(
            "wizard.cancel",
            View,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_cancel(p),
        )),
    ),
    with_params(
        "folder?:path",
        hidden(act(
            "wizard.folder",
            View,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_folder(p),
        )),
    ),
    with_params(
        "dir?:path",
        hidden(act(
            "wizard.finish",
            Command,
            Menu::Project,
            &[],
            wizard_open,
            None,
            |s, p| s.act_wizard_finish(p),
        )),
    ),
    // The New project dialog's fields, its Browse… and its Another project….
    with_params(
        "name?:text, folder?:text, start?:uint, pages?:bool",
        hidden(act(
            "new_project.set",
            View,
            Menu::File,
            &[],
            always,
            None,
            |s, p| s.act_new_project_set(p),
        )),
    ),
    with_params(
        "folder?:path",
        hidden(act(
            "new_project.browse",
            View,
            Menu::File,
            &[],
            always,
            None,
            |s, p| s.act_new_project_browse(p),
        )),
    ),
    with_params(
        "dir?:path",
        hidden(act(
            "new_project.other",
            View,
            Menu::File,
            &[],
            always,
            None,
            |s, p| s.act_new_project_other(p),
        )),
    ),
    // The Templates dialog's list and the actions of its selected template.
    with_params(
        "index:uint",
        hidden(act(
            "templates.select",
            View,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_templates_select(p),
        )),
    ),
    with_params(
        "index:uint, on?:bool",
        hidden(act(
            "templates.use",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_templates_use(p),
        )),
    ),
    with_params(
        "index:uint, discard?:bool",
        hidden(act(
            "templates.edit",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_templates_edit(p),
        )),
    ),
    with_params(
        "index:uint, name?:text",
        hidden(act(
            "templates.rename",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_templates_rename(p),
        )),
    ),
    with_params(
        "index:uint",
        hidden(act(
            "templates.remove",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_templates_remove(p),
        )),
    ),
    with_params(
        "dir?:path",
        hidden(act(
            "templates.add",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_templates_add(p),
        )),
    ),
    // The Text styles page's controls: a style's height, typeface and bold, Reset, Standard sizes.
    with_params(
        "style:text, height?:text, face?:enum(standard|narrow|mono|technical_drawing), bold?:bool",
        hidden(act(
            "text_styles.set",
            Command,
            Menu::Project,
            &[],
            writable,
            None,
            |s, p| s.act_text_styles_set(p),
        )),
    ),
    with_params(
        "style?:text",
        hidden(act(
            "text_styles.reset",
            Command,
            Menu::Project,
            &[],
            writable,
            None,
            |s, p| s.act_text_styles_reset(p),
        )),
    ),
    hidden(act(
        "text_styles.standard",
        Command,
        Menu::Project,
        &[],
        writable,
        None,
        |s, _| s.act_text_styles_standard(),
    )),
    // The Standard typeface's font file (doc 09 §4.8).
    with_params(
        "path?:text",
        hidden(act(
            "text_styles.typeface",
            Command,
            Menu::Project,
            &[],
            writable,
            None,
            |s, p| s.act_text_styles_typeface(p),
        )),
    ),
    hidden(act(
        "text_styles.typeface_clear",
        Command,
        Menu::Project,
        &[],
        writable,
        None,
        |s, _| s.act_text_styles_typeface_clear(),
    )),
    // The property grid's own controls: the window and the headless mode dispatch them.
    keeps_banner(with_params(
        "ids?:list(ref), refs?:list(ref)",
        hidden(with_aliases(
            &[("ids", ParamForm::List), ("refs", ParamForm::RefList)],
            act("select.set", View, Menu::Edit, &[], project_open, None, |s, p| {
                s.act_select_set(p)
            }),
        )),
    )),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(in_properties(act(
            "props.row",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_row(p),
        ))),
    )),
    // Doc 16a §6.8: the filter field of a grid whose open categories hold more than twelve rows.
    keeps_banner(with_params(
        "text:text",
        hidden(in_properties(act(
            "props.filter",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_filter(p),
        ))),
    )),
    keeps_banner(with_params(
        "category:text, kind?:text, expanded?:bool",
        hidden(in_properties(act(
            "props.category",
            View,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_category(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text, record?:text",
        hidden(in_properties(act(
            "props.edit",
            View,
            Menu::Edit,
            // Enter is the canvas's `canvas.activate`: the grid's own Enter is the pane's key.
            &["F2"],
            grid_shown,
            None,
            |s, p| s.act_props_edit(p),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(in_properties(with_aliases(
            &[("text", ParamForm::IdRef)],
            act("props.input", View, Menu::Edit, &[], grid_shown, None, |s, p| {
                s.act_props_input(p)
            }),
        ))),
    )),
    keeps_banner(with_params(
        "step?:int",
        hidden(in_properties(act(
            "props.commit",
            Command,
            Menu::Edit,
            &[],
            grid_writable,
            None,
            |s, p| s.act_props_commit(p),
        ))),
    )),
    keeps_banner(hidden(in_properties(act(
        "props.cancel",
        View,
        Menu::Edit,
        &[],
        grid_shown,
        None,
        |s, _| s.act_props_cancel(),
    )))),
    keeps_banner(with_params(
        "row?:text, value?:text, choice?:uint",
        hidden(with_aliases(
            &[("value", ParamForm::IdRef)],
            act(
                "props.set",
                Command,
                Menu::Edit,
                &[],
                grid_writable,
                None,
                |s, p| s.act_props_set(p),
            ),
        )),
    )),
    keeps_banner(hidden(act(
        "props.accept_tag_proposal",
        Command,
        Menu::Edit,
        &[],
        grid_writable,
        None,
        |s, _| s.act_props_accept_tag_proposal(),
    ))),
    keeps_banner(with_params(
        "index?:uint, step?:int",
        hidden(in_properties(act(
            "props.choose",
            Command,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_choose(p),
        ))),
    )),
    keeps_banner(with_params(
        "step?:int",
        hidden(in_properties(act(
            "props.row_menu",
            View,
            Menu::Edit,
            &["Shift+F10"],
            grid_shown,
            None,
            |s, p| s.act_props_row_menu(p),
        ))),
    )),
    // F (or a click) on a link row of the grid follows it, as F follows a link on the canvas.
    keeps_banner(with_params(
        "row?:text, page?:text",
        hidden(in_properties(act(
            "props.follow",
            View,
            Menu::Edit,
            &["F"],
            grid_shown,
            None,
            |s, p| s.act_props_follow(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text, action:enum(edit|clear|override|reset|freeze|unfreeze)",
        hidden(in_properties(act(
            "props.action",
            Command,
            Menu::Edit,
            &[],
            grid_shown,
            None,
            |s, p| s.act_props_action(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text, op:enum(add|remove|up|down)",
        hidden(in_properties(act(
            "props.list",
            Command,
            Menu::Edit,
            &[],
            grid_writable,
            None,
            |s, p| s.act_props_list(p),
        ))),
    )),
    keeps_banner(with_params(
        "row?:text, lang?:text, text?:text",
        hidden(in_properties(act(
            "props.mltext",
            Command,
            Menu::Edit,
            &["Shift+Enter"],
            grid_shown,
            None,
            |s, p| s.act_props_mltext(p),
        ))),
    )),
    // A connection point marked not used, or used again, from its Properties row and from the right-click menu
    // over it on the page.
    with_params(
        "function:text, idx:uint, on?:bool",
        hidden(act(
            "cp.unused",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_cp_unused(p),
        )),
    ),
    keeps_banner(hidden(in_properties(act(
        "preview.apply",
        Command,
        Menu::Edit,
        &["Ctrl+Enter"],
        card_writable,
        None,
        |s, _| s.act_preview_apply(),
    )))),
    keeps_banner(hidden(in_properties(act(
        "preview.cancel",
        View,
        Menu::Edit,
        &[],
        card_shown,
        None,
        |s, _| s.act_preview_cancel(),
    )))),
    // The table import card's switch "Take the file's values".
    keeps_banner(with_params(
        "on?:bool",
        hidden(act(
            "preview.take_file",
            View,
            Menu::Edit,
            &[],
            card_shown,
            None,
            |s, p| s.act_preview_take_file(p),
        )),
    )),
    // The Preview tab and the history panel's own moves.
    keeps_banner(with_params(
        "on?:bool",
        hidden(act(
            "preview.show",
            View,
            Menu::View,
            &[],
            project_open,
            None,
            |s, p| s.act_preview_show(p),
        )),
    )),
    keeps_banner(with_params(
        "step?:int, to?:enum(first|last)",
        hidden(act(
            "history.row",
            View,
            Menu::Edit,
            &[],
            history_open,
            None,
            |s, p| s.act_history_row(p),
        )),
    )),
    with_params(
        "entry?:uint",
        hidden(act(
            "history.undo_to",
            Command,
            Menu::Edit,
            &[],
            history_writable,
            None,
            |s, p| s.act_history_undo_to(p),
        )),
    ),
    keeps_banner(with_params(
        "entry?:uint",
        hidden(act(
            "history.changes",
            View,
            Menu::Edit,
            &[],
            history_open,
            None,
            |s, p| s.act_history_changes(p),
        )),
    )),
    keeps_banner(hidden(act(
        "toast.undo",
        Command,
        Menu::Edit,
        &[],
        writable,
        None,
        |s, _| s.act_toast_undo(),
    ))),
    keeps_banner(hidden(act(
        "toast.dismiss",
        View,
        Menu::Edit,
        &[],
        project_open,
        None,
        |s, _| s.act_toast_dismiss(),
    ))),
    // The jobs' and the notifications' own controls, and the recovery dialog's answers.
    keeps_banner(with_params(
        "job?:uint",
        hidden(act(
            "jobs.cancel",
            Command,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_jobs_cancel(p),
        )),
    )),
    keeps_banner(with_params(
        "job?:uint",
        headless(hidden(act(
            "jobs.wait",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_jobs_wait(p),
        ))),
    )),
    keeps_banner(with_params(
        "job?:uint",
        hidden(act(
            "jobs.open_result",
            View,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_jobs_open_result(p),
        )),
    )),
    // A toast's action, which goes while the symbol editor shows too.
    keeps_banner(with_params(
        "entry:uint, action:text",
        in_symbol_editor(hidden(act(
            "notifications.run",
            Command,
            Menu::View,
            &[],
            always,
            None,
            |s, p| s.act_notifications_run(p),
        ))),
    )),
    answers_recovery(hidden(act(
        "journal.recover",
        Command,
        Menu::File,
        &[],
        recovery_open,
        None,
        |s, _| s.act_journal_recover(),
    ))),
    answers_recovery(keeps_banner(hidden(act(
        "journal.keep",
        View,
        Menu::File,
        &[],
        recovery_open,
        None,
        |s, _| s.act_journal_keep(),
    )))),
    // The start-up offer's answers.
    with_params(
        "project:uint",
        hidden(act(
            "startup.recover",
            Command,
            Menu::File,
            &[],
            always,
            None,
            |s, p| s.act_startup_recover(p),
        )),
    ),
    keeps_banner(hidden(act(
        "startup.open_log_folder",
        View,
        Menu::File,
        &[],
        always,
        None,
        |s, _| s.act_startup_open_log_folder(),
    ))),
    keeps_banner(hidden(act(
        "startup.delete_log",
        Command,
        Menu::File,
        &[],
        always,
        None,
        |s, _| s.act_startup_delete_log(),
    ))),
    keeps_banner(hidden(act(
        "startup.dismiss",
        View,
        Menu::File,
        &[],
        always,
        None,
        |s, _| s.act_startup_dismiss(),
    ))),
    // The find pane's own controls; the pane handles Enter, Up and Down itself.
    keeps_banner(hidden(in_find(act(
        "find.close",
        View,
        Menu::Edit,
        &[],
        find_shown,
        None,
        |s, _| s.act_find_close(),
    )))),
    keeps_banner(with_params(
        "text:text",
        hidden(in_find(act(
            "find.text",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_text(p, false),
        ))),
    )),
    keeps_banner(with_params(
        "text:text",
        hidden(in_find(act(
            "find.replacement",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_text(p, true),
        ))),
    )),
    keeps_banner(with_params(
        "option:enum(regex|match_case|whole_word|in_selection), on?:bool",
        hidden(in_find(act(
            "find.option",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_option(p),
        ))),
    )),
    keeps_banner(with_params(
        "fields:list(text)",
        hidden(in_find(act(
            "find.fields",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_fields(p),
        ))),
    )),
    keeps_banner(with_params(
        "folios:list(text)",
        hidden(in_find(act(
            "find.folios",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_folios(p),
        ))),
    )),
    keeps_banner(hidden(in_find(act(
        "find.run",
        View,
        Menu::Edit,
        &[],
        find_shown,
        None,
        |s, _| s.act_find_run(),
    )))),
    keeps_banner(with_params(
        "step?:int, to?:uint",
        hidden(in_find(act(
            "find.row",
            View,
            Menu::Edit,
            &[],
            find_shown,
            None,
            |s, p| s.act_find_row(p),
        ))),
    )),
    keeps_banner(hidden(in_find(act(
        "find.jump",
        View,
        Menu::Edit,
        &[],
        find_shown,
        None,
        |s, _| s.act_find_jump(),
    )))),
    // The page's content as text (FR-UI-046's headless v0).
    with_params(
        "folio?:text",
        headless(act(
            "page.describe",
            View,
            Menu::Page,
            &[],
            page_describable,
            None,
            |s, p| s.act_page_describe(p),
        )),
    ),
    // The part selection dialog and the part editor.
    with_params(
        "target?:enum(parts|bridge_part|accessory_part), use?:uint, add?:bool, strip?:text, cable?:text",
        enabled_when_named(
            &["strip"],
            strip_parts_named,
            act(
                "part.choose",
                View,
                Menu::Edit,
                &[],
                writable,
                slot(Menu::Edit),
                |s, p| s.act_part_choose(p),
            ),
        ),
    ),
    with_params(
        "text?:text, kind?:text, category?:text, fits?:bool, view?:enum(all|project|favourites|recent), inactive?:bool",
        hidden(act(
            "part.search",
            View,
            Menu::Edit,
            &[],
            part_dialog_open,
            None,
            |s, p| s.act_part_search(p),
        )),
    ),
    with_params(
        "index?:uint, part?:text",
        hidden(act(
            "part.pick",
            Command,
            Menu::Edit,
            &[],
            part_dialog_open,
            None,
            |s, p| s.act_part_pick(p),
        )),
    ),
    // Type not known yet…: in the part dialog, and in the command search (placeholder, stand-in, TBD) and the Parts
    // row's menu beside Choose part…, which open the dialog with its field.
    with_params(
        "description?:text",
        act(
            "part.placeholder",
            Command,
            Menu::Edit,
            &[],
            writable,
            slot(Menu::Edit),
            |s, p| s.act_part_placeholder(p),
        ),
    ),
    with_params(
        "index?:uint",
        hidden(act(
            "part.favourite",
            View,
            Menu::Edit,
            &[],
            part_dialog_open,
            None,
            |s, p| s.act_part_favourite(p),
        )),
    ),
    with_params(
        "part?:text",
        act(
            "part.edit",
            View,
            Menu::Edit,
            &[],
            writable,
            slot(Menu::Edit),
            |s, p| s.act_part_edit(p),
        ),
    ),
    // New part… in Edit, and in New station…'s split on the PLC tab, where a person makes a card or a head station of
    // his own.
    with_params(
        "symbol?:text, from?:text, plc?:text, cable?:bool",
        act(
            "part.new",
            View,
            Menu::Edit,
            &[],
            writable,
            slot(Menu::Edit),
            |s, p| s.act_part_new(p),
        ),
    ),
    hidden(act(
        "part.delete",
        Command,
        Menu::Edit,
        &[],
        part_editor_open,
        None,
        |s, _| s.act_part_delete(),
    )),
    with_params(
        "section:text",
        hidden(act(
            "part.section",
            View,
            Menu::Edit,
            &[],
            part_editor_open,
            None,
            |s, p| s.act_part_section(p),
        )),
    ),
    with_params(
        "table:text, remove?:uint",
        hidden(act(
            "part.table_row",
            View,
            Menu::Edit,
            &[],
            part_editor_open,
            None,
            |s, p| s.act_part_table_row(p),
        )),
    ),
    with_params(
        "remove?:uint",
        hidden(act(
            "part.bus_row",
            View,
            Menu::Edit,
            &[],
            part_editor_open,
            None,
            |s, p| s.act_part_bus_row(p),
        )),
    ),
    with_params(
        "field:text, value:text",
        hidden(act(
            "part.field",
            View,
            Menu::Edit,
            &[],
            part_editor_open,
            None,
            |s, p| s.act_part_field(p),
        )),
    ),
    // The device box (doc 07 §2.5): a part's pins and where they stand on its box, edited on the symbol
    // editor's screen.
    with_params(
        "part?:text, new?:bool, discard?:bool",
        hidden(act("part.box", View, Menu::Edit, &[], writable, None, |s, p| {
            s.act_part_box(p)
        })),
    ),
    with_params(
        "add?:bool, name?:text, remove?:uint, pin?:uint, role?:text, side?:enum(top|bottom|left|right), at?:mm, before?:uint",
        hidden(act("part.pin", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_part_pin(p)
        })),
    ),
    with_params(
        "width?:mm, height?:mm, from_left?:bool, from_top?:bool",
        hidden(act(
            "part.box_size",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_part_box_size(p),
        )),
    ),
    with_params(
        "pins:json, mode?:enum(replace|add|toggle)",
        hidden(act(
            "part.box_select",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_part_box_select(p),
        )),
    ),
    hidden(act(
        "part.box_details",
        View,
        Menu::Edit,
        &[],
        writable,
        None,
        |s, _| s.act_part_box_details(),
    )),
    // Redraw the boxes, which the device box's Save in the symbol editor offers in one.
    with_params(
        "part:text",
        in_symbol_editor(hidden(act(
            "part.redraw_boxes",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_part_redraw_boxes(p),
        ))),
    ),
    with_params(
        "another?:bool",
        hidden(act(
            "part.save",
            Command,
            Menu::Edit,
            &[],
            part_editor_open,
            None,
            |s, p| s.act_part_save(p),
        )),
    ),
    // A part of My parts taken out of it: the part editor's Remove from my parts library…, confirmed; also in
    // the command search (delete part, remove part) for the selected device's part or the Libraries navigator's row.
    with_params(
        "part?:text, confirm?:bool",
        act(
            "part.remove_own",
            Command,
            Menu::Edit,
            &[],
            always,
            slot(Menu::Edit),
            |s, p| s.act_part_remove_own(p),
        ),
    ),
    with_params(
        "choice:enum(adapt|update)",
        hidden(act(
            "paste.resolve",
            Command,
            Menu::Edit,
            &[],
            paste_choice_open,
            None,
            |s, p| s.act_paste_resolve(p),
        )),
    ),
    // The keymap editor's own controls (Help ▸ Keyboard shortcuts); the window dispatches them.
    with_params(
        "text:text",
        hidden(act(
            "keymap.search",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_search(p),
        )),
    ),
    with_params(
        "action?:text",
        hidden(act(
            "keymap.record_start",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_record_start(p),
        )),
    ),
    with_params(
        "action:text, keys:text",
        hidden(act(
            "keymap.record",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_record(p),
        )),
    ),
    with_params(
        "action:text",
        hidden(act(
            "keymap.reset",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_reset(p),
        )),
    ),
    with_params(
        "path?:path",
        hidden(act(
            "keymap.import",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_import(p),
        )),
    ),
    with_params(
        "path?:path",
        hidden(act(
            "keymap.export",
            View,
            Menu::Help,
            &[],
            keymap_editor_open,
            None,
            |s, p| s.act_keymap_export(p),
        )),
    ),
    // The editor has an entry of its own on the Draw tab, named for what it is and alone in its column (a
    // button named "Symbols" under Black box was not found); the group's two Large buttons are Symbol and Device.
    // The Place group holds its menu entry and palette row.
    with_params(
        "key?:text, name?:text, discard?:bool",
        placed(
            place(
                Group::Place,
                5,
                CommandSize::Small,
                Some(key!("bar-symbol-editor")),
            ),
            "pencil",
            act(
                "symbol.new",
                View,
                Menu::Edit,
                &[],
                always,
                slot(Menu::Edit),
                |s, p| s.act_symbol("new", p),
            ),
        ),
    ),
    with_params(
        "file?:path, key?:text, discard?:bool",
        hidden(act("symbol.open", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("open", p)
        })),
    ),
    // The Library folder, typed or from the folder picker.
    with_params(
        "directory?:path",
        hidden(act(
            "symbol.directory",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("directory", p),
        )),
    ),
    with_params(
        "index:uint, discard?:bool",
        hidden(act(
            "symbol.candidate",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("candidate", p),
        )),
    ),
    with_params(
        "key:text",
        hidden(act(
            "symbol.function",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("function", p),
        )),
    ),
    with_params(
        "cp:uint, fn_cp:uint",
        hidden(act(
            "symbol.map_cp",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("map_cp", p),
        )),
    ),
    // The discipline the symbol is drawn for, or with `cp` one point's.
    with_params(
        "discipline:enum(electrical|pneumatic|vacuum|hydraulic|bus), cp?:uint",
        hidden(act(
            "symbol.discipline",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("discipline", p),
        )),
    ),
    // Part 3's nit: a connection point's potential role, for scripts and AI tools as for the role list.
    with_params(
        "cp:uint, role:enum(none|l1|l2|l3|l|n|pe|pen|fe|l_plus|l_minus|m|signal|undefined)",
        hidden(act("symbol.role", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("role", p)
        })),
    ),
    with_params(
        "name:text, x:mm, y:mm",
        hidden(act("symbol.link", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("link", p)
        })),
    ),
    with_params(
        "points:json, closed?:bool",
        hidden(act("symbol.path", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("path", p)
        })),
    ),
    with_params(
        "source:text, key:text, discard?:bool",
        hidden(act("symbol.fork", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("fork", p)
        })),
    ),
    // The browsers' Edit symbol and Copy and edit, on the symbol named (its key, copy ID or library address)
    // or the browser's current row; a row's context menu calls them too.
    with_params(
        "symbol?:text, discard?:bool",
        hidden(act("symbol.edit", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol_edit(p, false)
        })),
    ),
    with_params(
        "symbol?:text, discard?:bool",
        hidden(act(
            "symbol.copy_edit",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol_edit(p, true),
        )),
    ),
    with_params(
        "discard?:bool",
        hidden(act(
            "symbol.close",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("close", p),
        )),
    ),
    with_params(
        "document:json",
        hidden(act(
            "symbol.replace",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("replace", p),
        )),
    ),
    hidden(act("symbol.undo", View, Menu::Edit, &[], always, None, |s, p| {
        s.act_symbol("undo", p)
    })),
    hidden(act("symbol.redo", View, Menu::Edit, &[], always, None, |s, p| {
        s.act_symbol("redo", p)
    })),
    // Doc 16a P8: the editor's tool, also chosen by a key of the keymap.
    with_params(
        "tool:enum(select|line|rectangle|circle|ellipse|arc|text|cp|slot|anchor|link|polyline|polygon)",
        hidden(act("symbol.tool", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("tool", p)
        })),
    ),
    // What a person does with the drawing's selection, as on a page: the page's own keys run these while the
    // editor shows ([`crate::shell::symbol_editor::EDIT_KEYS`]).
    with_params(
        "items:json, mode?:enum(replace|add|toggle)",
        hidden(act(
            "symbol.select",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("select", p),
        )),
    ),
    with_params(
        "dx?:mm, dy?:mm",
        hidden(act("symbol.move", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("move", p)
        })),
    ),
    with_params(
        "dx?:int, dy?:int",
        hidden(act(
            "symbol.nudge",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("nudge", p),
        )),
    ),
    hidden(act(
        "symbol.delete",
        View,
        Menu::Edit,
        &[],
        always,
        None,
        |s, p| s.act_symbol("delete", p),
    )),
    hidden(act("symbol.copy", View, Menu::Edit, &[], always, None, |s, p| {
        s.act_symbol("copy", p)
    })),
    hidden(act("symbol.cut", View, Menu::Edit, &[], always, None, |s, p| {
        s.act_symbol("cut", p)
    })),
    hidden(act(
        "symbol.paste",
        View,
        Menu::Edit,
        &[],
        always,
        None,
        |s, p| s.act_symbol("paste", p),
    )),
    hidden(act(
        "symbol.duplicate",
        View,
        Menu::Edit,
        &[],
        always,
        None,
        |s, p| s.act_symbol("duplicate", p),
    )),
    with_params(
        "to:enum(left|centre|right|top|middle|bottom)",
        hidden(act(
            "symbol.align",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("align", p),
        )),
    ),
    with_params(
        "grip:uint, x:mm, y:mm",
        hidden(act("symbol.grip", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("grip", p)
        })),
    ),
    with_params(
        "step?:mm, snap?:bool",
        hidden(act("symbol.grid", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("grid", p)
        })),
    ),
    // The start card: the ways in, in the drawing's place; `leave` returns to the draft in hand.
    with_params(
        "leave?:bool",
        hidden(act(
            "symbol.start",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("start", p),
        )),
    ),
    // A row of the editor's Properties: of the selection, or of the symbol itself with nothing selected.
    with_params(
        "key:text, value:json",
        hidden(act("symbol.prop", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("prop", p)
        })),
    ),
    with_params(
        "variant:uint",
        hidden(act(
            "symbol.variant",
            View,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("variant", p),
        )),
    ),
    hidden(act(
        "symbol.validate",
        View,
        Menu::Edit,
        &[],
        always,
        None,
        |s, p| s.act_symbol("validate", p),
    )),
    with_params(
        "tool:enum(line|rectangle|circle|ellipse|arc|text|anchor), x:mm, y:mm, x2?:mm, y2?:mm, text?:text, start?:int, sweep?:int",
        hidden(act("symbol.draw", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("draw", p)
        })),
    ),
    with_params(
        "x:mm, y:mm, idx?:uint, fn_cp?:uint, designation?:text, direction?:enum(up|right|down|left)",
        hidden(act("symbol.cp", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("cp", p)
        })),
    ),
    with_params(
        "kind?:json, x:mm, y:mm",
        hidden(act("symbol.slot", View, Menu::Edit, &[], always, None, |s, p| {
            s.act_symbol("slot", p)
        })),
    ),
    with_params(
        "file?:path",
        hidden(act(
            "symbol.export",
            Command,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("export", p),
        )),
    ),
    with_params(
        "directory?:path",
        hidden(act(
            "symbol.save",
            Command,
            Menu::Edit,
            &[],
            always,
            None,
            |s, p| s.act_symbol("save", p),
        )),
    ),
    with_params(
        "x:mm, y:mm",
        hidden(act(
            "symbol.place",
            Command,
            Menu::Edit,
            &[],
            writable,
            None,
            |s, p| s.act_symbol("place", p),
        )),
    ),
    hidden(act(
        "symbol.scratch",
        Command,
        Menu::Edit,
        &[],
        writable,
        None,
        |s, p| s.act_symbol("scratch", p),
    )),
    // The editor's offer for a draft saved to a library and unchanged since.
    hidden(act(
        "symbol.new_page",
        Command,
        Menu::Edit,
        &[],
        writable,
        None,
        |s, p| s.act_symbol("new_page", p),
    )),
];
