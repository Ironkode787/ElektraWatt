// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The action table, section `file`. File and its neighbours: new, open, import, save, export, print, close.
//!
//! The sections follow one another in the order of `super::SECTIONS`, and that order is the registry's: it sets the
//! order of menu items, palette rows and the shortcut list. Move an entry only where its menu neighbours are.

use super::super::*;

pub(in crate::actions) const ACTIONS: &[ActionMeta] = &[
    with_params(
        "dir?:path, template?:path, discard?:bool",
        act(
            "file.new",
            Command,
            Menu::File,
            &["Ctrl+N"],
            always,
            slot(Menu::File),
            |s, p| s.act_new(p),
        ),
    ),
    with_params(
        "dir?:path, read_only?:bool, discard?:bool",
        act(
            "file.open",
            Command,
            Menu::File,
            &["Ctrl+O"],
            always,
            slot(Menu::File),
            |s, p| s.act_open(p),
        ),
    ),
    // File ▸ Import…, beside Open: a QElectroTech project starts the import, an `.ewz` its Open .ewz dialog.
    with_params(
        "file?:path, discard?:bool",
        act(
            "file.import",
            Command,
            Menu::File,
            &[],
            always,
            slot(Menu::File),
            |s, p| s.act_import(p),
        ),
    ),
    // A DXF, DWG or PDF drawing as a new page or an underlay of the shown page; File ▸ Import… reaches it.
    with_params(
        "file?:path, as?:enum(page|underlay), embed?:bool, units?:enum(mm|cm|m|in|ft|um|dm|yd), converter?:path",
        act(
            "file.import_drawing",
            Command,
            Menu::File,
            &[],
            project_open,
            None,
            |s, p| s.act_import_drawing(p),
        ),
    ),
    // An `.ewz`, asking Extract and open or Open read-only; File ▸ Import… reaches it.
    with_params(
        "path?:path, how?:enum(extract|read_only), discard?:bool",
        act("file.open_ewz", Command, Menu::File, &[], always, None, |s, p| {
            s.act_open_ewz(p)
        }),
    ),
    with_params(
        "index:uint, discard?:bool",
        needs_parameters(act(
            "file.open_recent",
            Command,
            Menu::File,
            &[],
            has_recent,
            sub(Menu::File, Submenu::OpenRecent, false),
            |s, p| s.act_open_recent(p),
        )),
    ),
    with_params(
        "discard?:bool",
        act(
            "file.open_sample",
            Command,
            Menu::File,
            &[],
            always,
            slot(Menu::File),
            |s, p| s.act_open_sample(p),
        ),
    ),
    with_params(
        "file?:path, dir?:path, discard?:bool",
        placed(
            place(Group::Import, 2, CommandSize::Small, None),
            "file-import",
            act(
                "file.import_qet",
                Command,
                Menu::Project,
                &[],
                always,
                // It is in Project, Import (its ID unchanged).
                None,
                |s, p| s.act_import_qet(p),
            ),
        ),
    ),
    // The import card's Apply and the checkpoint confirmation's Create project.
    with_params(
        "confirm?:bool",
        hidden(act(
            "file.import_qet_apply",
            Command,
            Menu::Project,
            &[],
            always,
            None,
            |s, p| s.act_import_qet_apply(p),
        )),
    ),
    // Project, Import, Re-import QElectroTech project (the open project's earlier import), its
    // proposal's Re-import (`accept`) and the re-import card's Apply. A command-bar button beside
    // the import's.
    with_params(
        "file?:path, accept?:bool",
        placed(
            place(Group::Import, 3, CommandSize::Small, None),
            "file-import",
            act(
                "file.reimport_qet",
                Command,
                Menu::Project,
                &[],
                project_open,
                slot(Menu::Project),
                |s, p| s.act_reimport_qet(p),
            ),
        ),
    ),
    hidden(act(
        "file.reimport_qet_apply",
        Command,
        Menu::Project,
        &[],
        project_open,
        None,
        |s, _| s.act_reimport_qet_apply(),
    )),
    // File, Import, Parts table…
    with_params(
        "path?:path",
        placed(
            // Both imports in one group, Import, the parts table first.
            place(
                Group::Import,
                1,
                CommandSize::Large,
                Some(key!("bar-import-parts")),
            ),
            "database-import",
            act(
                "parts.import",
                Command,
                Menu::Project,
                &[],
                writable,
                None,
                |s, p| s.act_parts_import(p),
            ),
        ),
    ),
    // The parts import's column mapping step (the card's fields and its Import button).
    with_params(
        "source?:text, column?:text, member?:text",
        hidden(act(
            "parts.map",
            Command,
            Menu::Project,
            &[],
            parts_map_open,
            None,
            |s, p| s.act_parts_map(p),
        )),
    ),
    hidden(act(
        "parts.map_import",
        Command,
        Menu::Project,
        &[],
        parts_map_open,
        None,
        |s, _| s.act_parts_map_import(),
    )),
    with_icon(
        "device-floppy",
        act(
            "file.save",
            Command,
            Menu::File,
            &["Ctrl+S"],
            writable,
            slot_sep(Menu::File),
            |s, _| s.act_save(),
        ),
    ),
    // While the symbol editor shows, Save is the symbol's and the File menu keeps Save project.
    hidden(act(
        "file.save_project",
        Command,
        Menu::File,
        &[],
        writable,
        None,
        |s, _| s.act_save_project(),
    )),
    with_params(
        "dir?:path",
        act(
            "file.save_as",
            Command,
            Menu::File,
            &["Ctrl+Shift+S"],
            project_open,
            slot(Menu::File),
            |s, p| s.act_save_as(p),
        ),
    ),
    answers_recovery(keeps_banner(with_params(
        "confirm?:bool",
        act(
            "file.discard_journal",
            Command,
            Menu::File,
            &[],
            can_discard_journal,
            slot(Menu::File),
            |s, p| s.act_discard_journal(p),
        ),
    ))),
    // The export runs as a job (§6.3); `replace` confirms an existing file, `wait` (headless) returns at
    // once.
    with_params(
        "path?:path, replace?:bool, wait?:bool",
        placed(
            place(Group::Pdf, 1, CommandSize::Large, None),
            "file-type-pdf",
            act(
                "file.export_pdf",
                Command,
                Menu::Output,
                &["Ctrl+Shift+E"],
                has_pages,
                None,
                |s, p| s.act_export_pdf(p),
            ),
        ),
    ),
    // File ▸ Export .ewz…, the saved generation as one file.
    with_params(
        "path?:path, replace?:bool",
        act(
            "file.export_ewz",
            Command,
            Menu::File,
            &[],
            project_open,
            slot(Menu::File),
            |s, p| s.act_export_ewz(p),
        ),
    ),
    // An external change now, as the window's poll finds it; the upgrade dialog's Upgrade
    // and save; the collision dialog's buttons and choices; Set up merge driver.
    keeps_banner(act(
        "project.check_disk",
        Command,
        Menu::File,
        &[],
        project_open,
        slot(Menu::File),
        |s, _| s.act_check_disk(),
    )),
    hidden(act(
        "file.upgrade",
        Command,
        Menu::File,
        &[],
        project_open,
        None,
        |s, _| s.act_upgrade(),
    )),
    keeps_banner(with_params(
        "take?:enum(mine|theirs)",
        hidden(act(
            "external.apply",
            Command,
            Menu::File,
            &[],
            project_open,
            None,
            |s, p| s.act_external_apply(p),
        )),
    )),
    keeps_banner(hidden(act(
        "external.review",
        View,
        Menu::File,
        &[],
        project_open,
        None,
        |s, _| s.act_external_review(),
    ))),
    // FT-1499: the damaged-copy banner's Update from library.
    hidden(act(
        "library.update_damaged",
        Command,
        Menu::File,
        &[],
        damaged_update_offered,
        None,
        |s, _| s.act_update_damaged(),
    )),
    keeps_banner(with_params(
        "row:uint, take:enum(mine|theirs)",
        hidden(act(
            "external.choose",
            View,
            Menu::File,
            &[],
            project_open,
            None,
            |s, p| s.act_external_choose(p),
        )),
    )),
    with_params(
        "confirm?:bool",
        act(
            "git.setup",
            Command,
            Menu::Project,
            &[],
            project_open,
            slot(Menu::Project),
            |s, p| s.act_git_setup(p),
        ),
    ),
    // The print pre-dialog, its options settable at once; it builds its
    // print set as a job (`wait`, headless: return at once).
    with_params(
        "range?:enum(all|current|sections|pages), sections?:text, pages?:text, paper?:text, fit?:bool, mono?:bool, wait?:bool",
        placed(
            place(Group::Pdf, 2, CommandSize::Small, None),
            "printer",
            act(
                "file.print",
                View,
                Menu::Output,
                &["Ctrl+P"],
                has_pages,
                None,
                |s, p| s.act_file_print(p),
            ),
        ),
    ),
    with_params(
        "range?:enum(all|current|sections|pages), sections?:text, pages?:text, paper?:text, fit?:bool, mono?:bool, wait?:bool",
        hidden(act(
            "print.set",
            View,
            Menu::Output,
            &[],
            print_dialog_open,
            None,
            |s, p| s.act_print_set(p),
        )),
    ),
    with_params(
        "to?:enum(next|previous), page?:uint",
        hidden(act(
            "print.page",
            View,
            Menu::Output,
            &[],
            print_dialog_open,
            None,
            |s, p| s.act_print_page(p),
        )),
    ),
    with_params(
        "via?:enum(printer|viewer)",
        hidden(act(
            "print.send",
            View,
            Menu::Output,
            &[],
            print_dialog_open,
            None,
            |s, p| s.act_print_send(p),
        )),
    ),
    with_params(
        "discard?:bool",
        act(
            "file.close",
            Command,
            Menu::File,
            &["Ctrl+Shift+W"],
            project_open,
            slot_sep(Menu::File),
            |s, p| s.act_close(p),
        ),
    ),
    answers_recovery(with_params(
        "discard?:bool",
        act(
            "app.quit",
            Command,
            Menu::File,
            &["Ctrl+Q"],
            always,
            slot(Menu::File),
            |s, p| s.act_quit(p),
        ),
    )),
];
