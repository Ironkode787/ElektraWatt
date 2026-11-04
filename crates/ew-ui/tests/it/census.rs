// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The accessibility census (WP-S2.11 D3, D5, D6; NFR-A11Y-02, G-M2-10): every surface of the window is opened on a
//! copy of Golden A that the suite's `Tmp` made (the committed project is only read), and the walker's six rules
//! (`support::accessibility_problems`, `support::availability_problems`) run on each. A failing node is a line of its
//! surface, and every line of every surface is reported at once.
//!
//! The surfaces: folio 101 with a device and a wire selected (the cards show); the six command-bar tabs; every menu of
//! the action registry; every `PaneKind` and `NavKind`, the Preview, History, Find and Messages panes with content;
//! every `Dialog` variant (by a `match` without a wildcard arm, so a new variant does not compile until it is listed);
//! the start page, the five first-run steps and the ten wizard steps. The creation forms are W49.2-F1's census
//! (`window_check_w49_2.rs`): it opens every form whose action the source builds, settles on each (the six rules) and
//! checks its list with [`form_problems`].
//!
//! No step commits to disk: the dialogs are dismissed, the copies are dropped with their `Tmp`.

use crate::support;

use std::collections::BTreeSet;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::actions::menu_model;
use ew_ui::ew_viewmodel::command_bar::Tab;
use ew_ui::ew_viewmodel::ew_api::{FormatStep, MigrationError, Migrations};
use ew_ui::ew_viewmodel::first_run::FirstRunStep;
use ew_ui::ew_viewmodel::layout::PaneKind;
use ew_ui::ew_viewmodel::navigators::NavKind;
use ew_ui::ew_viewmodel::shell::{Dialog, Mode, Outcome, ShellModel};
use ew_ui::ew_viewmodel::wizard::Step;
use ew_ui::ew_viewmodel::{key, strings};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, harness_with, wait_for_page};

/// The lines of every surface visited, and the surfaces.
#[derive(Default)]
struct Census {
    problems: Vec<String>,
    surfaces: Vec<String>,
}

impl Census {
    /// Settles the frame and runs the six rules on what it shows, as `surface`.
    fn visit(&mut self, h: &mut Harness<'_, App>, surface: &str) {
        h.run_ok();
        h.run_ok();
        self.problems.extend(support::accessibility_problems(h, surface));
        self.problems.extend(support::availability_problems(h, surface));
        self.surfaces.push(surface.to_owned());
    }

    #[track_caller]
    fn assert_clean(&self) {
        assert_eq!(
            self.problems,
            Vec::<String>::new(),
            "over {} surfaces",
            self.surfaces.len()
        );
    }
}

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The window on a writable copy of Golden A at folio 101 (an open of the committed project would be read-only).
fn on_golden(tmp: &Tmp, name: &str) -> Harness<'static, App> {
    let dir = tmp.golden(name);
    let mut h = harness(tmp, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    h.run_ok();
    wait_for_page(&mut h);
    dispatch(&mut h, "page.goto", json!({ "folio": "101" }));
    wait_for_page(&mut h);
    h.run_ok();
    h
}

/// U2 of WP-S2.11 §5: each action of `built` (the source's form actions) that `opened` (the census's) lacks.
pub(crate) fn form_problems(opened: &BTreeSet<String>, built: &BTreeSet<String>) -> Vec<String> {
    built
        .difference(opened)
        .map(|a| format!("the action {a} builds a form that the census does not open"))
        .collect()
}

/// How the census opens a pane (a navigator is one of [`NavKind`]'s; the panes with content get it here).
fn open_pane(h: &mut Harness<'_, App>, pane: PaneKind) {
    match pane {
        // On show in the project view.
        PaneKind::Pages | PaneKind::PageView => {}
        PaneKind::Properties => dispatch(h, "props.show", json!({"on": true})),
        PaneKind::Find => {
            dispatch(h, "find.open", json!({}));
            dispatch(h, "find.text", json!({"text": "-101"}));
            dispatch(h, "find.run", json!({}));
        }
        PaneKind::Navigator(kind) => open_navigator(h, kind),
        PaneKind::Jobs => dispatch(h, "jobs.show", json!({})),
        // The Preview tab with Renumber devices' card (WP-S2.09 PV2's steps): its history then holds the step.
        PaneKind::Preview => {
            dispatch(
                h,
                "command.run",
                json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": {"default":
                    "page-based", "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}", "rules": [],
                    "child_separator": "-"}]}}}),
            );
            dispatch(h, "devices.renumber", json!({}));
            h.run_ok();
            support::off_chrome(h, "Renumber")
                .expect("the dialog's Renumber")
                .click();
            h.run_ok();
            assert!(h.state().shell().preview_shown(), "the Preview tab");
        }
        PaneKind::History => dispatch(h, "history.show", json!({})),
        PaneKind::Glossary => dispatch(h, "help.glossary", json!({})),
    }
}

/// How the census opens a navigator.
fn open_navigator(h: &mut Harness<'_, App>, kind: NavKind) {
    match kind {
        NavKind::Messages => dispatch(h, "messages.show", json!({"on": true})),
        NavKind::Pages
        | NavKind::Devices
        | NavKind::Connections
        | NavKind::Strips
        | NavKind::Cables
        | NavKind::Plc
        | NavKind::Potentials
        | NavKind::Ips
        | NavKind::Structure
        | NavKind::Parts
        | NavKind::Blocks
        | NavKind::Unplaced
        | NavKind::Macros
        | NavKind::Libraries => dispatch(h, "navigator.show", json!({"kind": kind.word()})),
    }
}

/// D3: folio 101 with a device and a wire selected, the six command-bar tabs, every menu of the registry, every pane
/// and navigator, with the Preview, History, Find and Messages panes holding content.
#[test]
fn census_the_window_its_tabs_menus_panes_and_navigators() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "g.ew");
    let mut census = Census::default();
    let ids: Vec<String> = {
        let tables = &h.state().shell().canvas().expect("folio 101").page().tables;
        let rep = tables.reps.first().expect("a device on 101").id.to_string();
        let wire = tables.connections.first().expect("a wire on 101").id.to_string();
        vec![rep, wire]
    };
    dispatch(&mut h, "select.set", json!({ "ids": ids }));
    census.visit(&mut h, "folio 101 with a device and a wire");
    for tab in Tab::ALL {
        dispatch(&mut h, "view.command_tab", json!({"tab": tab.word()}));
        census.visit(&mut h, &format!("the {} tab", strings::text(tab.label_key())));
    }
    // Every menu of the app menu; R6 reaches its entries (a disabled one is under the Menu button).
    let menu_button = strings::text(key!("commandbar-menu"));
    let mut disabled_entries = 0;
    for menu in menu_model(h.state().shell()) {
        support::open_menu(&mut h, &[&menu.label]);
        census.visit(&mut h, &format!("the {} menu", menu.label));
        disabled_entries += h
            .root()
            .children_recursive()
            .filter(|n| {
                let n = n.accesskit_node();
                let mut at = n.parent();
                let mut under_menu = false;
                while let Some(a) = at {
                    under_menu |= a.role() == egui::Role::Button && a.label().as_deref() == Some(menu_button);
                    at = a.parent();
                }
                under_menu && n.role() == egui::Role::Button && n.is_disabled()
            })
            .count();
        h.key_press(egui::Key::Escape);
        h.key_press(egui::Key::Escape);
        h.run_ok();
    }
    assert!(
        disabled_entries > 0,
        "an unavailable action's entry is shown disabled in the app menu"
    );
    for pane in PaneKind::ALL {
        open_pane(&mut h, pane);
        census.visit(&mut h, &format!("the {} pane", pane.key()));
    }
    for kind in NavKind::ALL {
        open_pane(&mut h, PaneKind::Navigator(kind));
        census.visit(&mut h, &format!("the {} navigator", kind.word()));
    }
    h.state_mut().shell_mut().shutdown();
    census.assert_clean();
}

/// A local mirror of [`Dialog`]'s variants: [`kind`] maps every variant without a wildcard arm, and [`open_dialog`]
/// opens every kind, so a new variant fails to compile until the census opens it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DialogKind {
    About,
    Shortcuts,
    ConfirmDiscard,
    GotoPage,
    Notices,
    ConfirmJournalDiscard,
    ConfirmCheckScheme,
    SaveWorkspace,
    Form,
    Recovery,
    ConfirmPurge,
    PartChoose,
    PartEdit,
    Print,
    ExternalChange,
    ConfirmUpgrade,
    OpenEwz,
    ConfirmGitSetup,
}

const DIALOGS: [DialogKind; 18] = [
    DialogKind::About,
    DialogKind::Shortcuts,
    DialogKind::ConfirmDiscard,
    DialogKind::GotoPage,
    DialogKind::Notices,
    DialogKind::ConfirmJournalDiscard,
    DialogKind::ConfirmCheckScheme,
    DialogKind::SaveWorkspace,
    DialogKind::Form,
    DialogKind::Recovery,
    DialogKind::ConfirmPurge,
    DialogKind::PartChoose,
    DialogKind::PartEdit,
    DialogKind::Print,
    DialogKind::ExternalChange,
    DialogKind::ConfirmUpgrade,
    DialogKind::OpenEwz,
    DialogKind::ConfirmGitSetup,
];

fn kind(d: &Dialog) -> DialogKind {
    match d {
        Dialog::About => DialogKind::About,
        Dialog::Shortcuts => DialogKind::Shortcuts,
        Dialog::ConfirmDiscard { .. } => DialogKind::ConfirmDiscard,
        Dialog::GotoPage { .. } => DialogKind::GotoPage,
        Dialog::Notices => DialogKind::Notices,
        Dialog::ConfirmJournalDiscard => DialogKind::ConfirmJournalDiscard,
        Dialog::ConfirmCheckScheme { .. } => DialogKind::ConfirmCheckScheme,
        Dialog::SaveWorkspace { .. } => DialogKind::SaveWorkspace,
        Dialog::Form(_) => DialogKind::Form,
        Dialog::Recovery { .. } => DialogKind::Recovery,
        Dialog::ConfirmPurge { .. } => DialogKind::ConfirmPurge,
        Dialog::PartChoose(_) => DialogKind::PartChoose,
        Dialog::PartEdit(_) => DialogKind::PartEdit,
        Dialog::Print(_) => DialogKind::Print,
        Dialog::ExternalChange { .. } => DialogKind::ExternalChange,
        Dialog::ConfirmUpgrade { .. } => DialogKind::ConfirmUpgrade,
        Dialog::OpenEwz { .. } => DialogKind::OpenEwz,
        Dialog::ConfirmGitSetup { .. } => DialogKind::ConfirmGitSetup,
    }
}

fn same(value: Value) -> Result<Value, MigrationError> {
    Ok(value)
}

/// A chain from the test format `0.0` to this one that changes nothing but the stamp (as `collab.rs`'s XW-6).
static CHAIN: Migrations = Migrations {
    kinds: &[],
    formats: &[FormatStep {
        from: "0.0",
        to: "0.1",
        manifest: same,
        record: same,
    }],
};

/// A copy of Golden A whose last session ended with unsaved changes: its journal stays (`scaling.rs`'s recovery).
fn crashed(tmp: &Tmp) -> std::path::PathBuf {
    let dir = tmp.golden("crashed.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    for (action, params) in [
        ("file.open", json!({ "dir": dir.to_string_lossy() })),
        (
            "command.run",
            json!({"cmd": "page.create", "params": {"number": 996}}),
        ),
    ] {
        assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
    }
    shell.shutdown();
    dir
}

/// The window with a dialog of `kind` open, in a fixture of its own where it needs one (the shell's own actions and
/// test hooks only, D5).
fn open_dialog(tmp: &Tmp, kind: DialogKind) -> Harness<'static, App> {
    let simple = |action: &str, params: Value| {
        let mut h = on_golden(tmp, &format!("{kind:?}.ew"));
        dispatch(&mut h, action, params);
        h
    };
    match kind {
        DialogKind::About => simple("help.about", json!({})),
        DialogKind::Shortcuts => simple("help.shortcuts", json!({})),
        DialogKind::Notices => simple("help.notices", json!({})),
        DialogKind::GotoPage => simple("nav.goto", json!({})),
        DialogKind::SaveWorkspace => simple("view.workspace_save", json!({})),
        DialogKind::Form => simple("page.new", json!({})),
        DialogKind::Print => simple("file.print", json!({"range": "current"})),
        DialogKind::ConfirmDiscard => {
            let mut h = simple(
                "command.run",
                json!({"cmd": "page.create", "params": {"number": 999}}),
            );
            dispatch(&mut h, "file.close", json!({}));
            h
        }
        DialogKind::PartChoose | DialogKind::PartEdit => {
            let mut h = simple("select.set", json!({"refs": ["tag:F55"]}));
            let action = if kind == DialogKind::PartChoose {
                ("part.choose", json!({"use": 0}))
            } else {
                ("part.edit", json!({}))
            };
            dispatch(&mut h, action.0, action.1);
            h
        }
        DialogKind::Recovery | DialogKind::ConfirmJournalDiscard => {
            let dir = crashed(tmp);
            let mut h = harness(tmp, Scripted::default(), Some(dir));
            h.set_size(egui::Vec2::new(1280.0, 800.0));
            h.run_ok();
            if kind == DialogKind::ConfirmJournalDiscard {
                dispatch(&mut h, "file.discard_journal", json!({}));
            }
            h
        }
        DialogKind::ConfirmCheckScheme => {
            let dir = tmp.golden("wizard.ew");
            let mut shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
            assert_eq!(
                shell.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() })),
                Outcome::Done
            );
            let mut h = harness_with(shell, Scripted::default(), None);
            h.set_size(egui::Vec2::new(1280.0, 800.0));
            h.run_ok();
            dispatch(&mut h, "tools.company_standard", json!({}));
            support::wait_for_preview(&mut h);
            let basic = tmp.0.join("company.ew");
            for (action, params) in [
                ("wizard.set", json!({"field": "check_scheme", "value": "full"})),
                ("wizard.finish", json!({ "dir": basic.to_string_lossy() })),
                ("wizard.apply_check_scheme", json!({})),
            ] {
                dispatch(&mut h, action, params);
            }
            h
        }
        DialogKind::ConfirmPurge => {
            // The first logo is unused once the project is re-opened (`snapshots.rs`'s purge question).
            let dir = tmp.golden("purge.ew");
            let mut shell = ShellModel::new(Mode::Gui, tmp.config());
            let logo = |svg: &str| {
                json!({"cmd": "project.company.set",
                    "params": {"logo": ew_ui::ew_viewmodel::ew_api::base64_encode(svg.as_bytes())}})
            };
            for (action, params) in [
                ("file.open", json!({ "dir": dir.to_string_lossy() })),
                (
                    "command.run",
                    logo(r#"<svg viewBox="0 0 8 2"><rect width="8" height="2"/></svg>"#),
                ),
                ("file.save", json!({})),
                (
                    "command.run",
                    logo(r#"<svg viewBox="0 0 3 3"><circle r="1"/></svg>"#),
                ),
                ("file.save", json!({})),
                ("file.close", json!({})),
            ] {
                assert_eq!(shell.dispatch(action, &params), Outcome::Done, "{action}");
            }
            shell.shutdown();
            let mut h = harness(tmp, Scripted::default(), Some(dir));
            h.set_size(egui::Vec2::new(1280.0, 800.0));
            h.run_ok();
            dispatch(&mut h, "project.purge_assets", json!({}));
            h
        }
        DialogKind::ExternalChange => {
            // W49.3's collision: W20's colour edited here, and saved otherwise by a colleague on disk.
            let dir = tmp.golden("live.ew");
            let mut h = harness(tmp, Scripted::default(), Some(dir.clone()));
            h.set_size(egui::Vec2::new(1280.0, 800.0));
            h.run_ok();
            wait_for_page(&mut h);
            let w20 = crate::window_check_w49_3::w20(&dir);
            dispatch(
                &mut h,
                "command.run",
                json!({"cmd": "fields.set", "params": {"kind": "connection", "targets": [w20],
                    "path": "overrides.colour", "value": ["BU"]}}),
            );
            crate::window_check_w49_3::colleague(tmp, &dir, "WH");
            dispatch(&mut h, "project.check_disk", json!({}));
            h
        }
        DialogKind::ConfirmUpgrade => {
            let dir = tmp.golden("older.ew");
            let manifest = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
            let old = manifest
                .replace("\"format\": \"0.1\"", "\"format\": \"0.0\"")
                .replace("\"min_reader\": \"0.1\"", "\"min_reader\": \"0.0\"");
            assert_ne!(old, manifest);
            std::fs::write(dir.join("manifest.json"), old).unwrap();
            let mut shell = ShellModel::new(Mode::Gui, tmp.config());
            shell.set_migrations_for_tests(Some(&CHAIN));
            assert_eq!(
                shell.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() })),
                Outcome::Done
            );
            let mut h = harness_with(shell, Scripted::default(), None);
            h.set_size(egui::Vec2::new(1280.0, 800.0));
            h.run_ok();
            dispatch(&mut h, "file.save", json!({}));
            h
        }
        DialogKind::OpenEwz => {
            let ewz = tmp.0.join("transport.ewz");
            let mut h = simple("file.export_ewz", json!({"path": ewz.to_string_lossy()}));
            dispatch(&mut h, "file.open_ewz", json!({"path": ewz.to_string_lossy()}));
            h
        }
        DialogKind::ConfirmGitSetup => {
            // A worktree is a folder with `.git` in it or above it: the question needs no git executable.
            std::fs::create_dir_all(tmp.0.join("repo").join(".git")).unwrap();
            let dir = tmp.golden("repo/project.ew");
            let mut h = harness(tmp, Scripted::default(), Some(dir));
            h.set_size(egui::Vec2::new(1280.0, 800.0));
            h.run_ok();
            dispatch(&mut h, "git.setup", json!({}));
            h
        }
    }
}

/// D3: every dialog of the shell, each in its state.
#[test]
fn census_every_dialog() {
    let mut census = Census::default();
    for want in DIALOGS {
        let tmp = Tmp::new();
        let mut h = open_dialog(&tmp, want);
        h.run_ok();
        let got = h.state().shell().dialog().map(kind);
        assert_eq!(got, Some(want), "{want:?} opens");
        let surface = support::surface_name(&h);
        census.visit(&mut h, &surface);
        h.state_mut().shell_mut().shutdown();
    }
    census.assert_clean();
}

/// D3: the start page, the five first-run steps and the ten wizard steps.
#[test]
fn census_the_start_page_the_first_run_and_the_wizard() {
    let mut census = Census::default();
    let tmp = Tmp::new();
    let mut h = harness_with(
        ShellModel::new(Mode::Gui, tmp.config_with_resources()),
        Scripted::default(),
        None,
    );
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    census.visit(&mut h, "the start page");
    h.state_mut().shell_mut().shutdown();
    drop(h);

    let first = Tmp::new();
    let mut shell = ShellModel::new(Mode::Gui, first.config_with_resources());
    shell.start_first_run_if_needed();
    let mut h = harness_with(shell, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    for step in FirstRunStep::ALL {
        h.run_ok();
        let on = h.state().shell().first_run().map(|r| r.step);
        assert_eq!(on, Some(step));
        census.visit(
            &mut h,
            &format!("the first run's {} step", strings::text(step.title_key())),
        );
        if step != FirstRunStep::Start {
            dispatch(&mut h, "first_run.next", json!({}));
        }
    }
    h.state_mut().shell_mut().shutdown();
    drop(h);

    let dir = tmp.golden("wizard.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config_with_resources());
    assert_eq!(
        shell.dispatch("file.open", &json!({ "dir": dir.to_string_lossy() })),
        Outcome::Done
    );
    let mut h = harness_with(shell, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    h.run_ok();
    dispatch(&mut h, "tools.company_standard", json!({}));
    support::wait_for_preview(&mut h);
    for step in Step::ALL {
        dispatch(&mut h, "wizard.goto", json!({"step": step.index()}));
        census.visit(
            &mut h,
            &format!("the wizard's {} step", strings::text(step.title_key())),
        );
    }
    dispatch(&mut h, "wizard.cancel", json!({}));
    h.state_mut().shell_mut().shutdown();
    census.assert_clean();
}

/// GT-3: the walker's lines on a fixture UI: a check box without its toggled state, a combo box without its expanded
/// state, a button without a name.
#[test]
fn the_walker_names_each_failing_node_in_words() {
    let mut grid = true;
    let mut h = Harness::builder()
        .with_context_setup(ew_ui::app::setup)
        // kittest's own check would stop at the unnamed button: the walker's lines are the subject here.
        .with_accessibility_check(false)
        .build_ui(|ui| {
            let check = ui.checkbox(&mut grid, "Grid");
            ui.ctx().accesskit_node_builder(check.id, |n| n.clear_toggled());
            egui::ComboBox::from_label("Kind")
                .selected_text("contact")
                .show_ui(ui, |_| {});
            ui.add(egui::Button::new(""));
        });
    h.run();
    let bounds = h
        .get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().is_none())
        .expect("the unnamed button")
        .accesskit_node()
        .raw_bounds();
    assert_eq!(
        support::accessibility_problems(&h, "fixture"),
        [
            "fixture: CheckBox \"Grid\" has no toggled state".to_owned(),
            "fixture: ComboBox \"Kind\" has no expanded state".to_owned(),
            format!("fixture: Button at {bounds:?} has no accessible name"),
        ]
    );
}

/// GT-3: the form census's source check names a form action it does not open (§5 U2).
#[test]
fn the_form_check_names_a_form_the_census_does_not_open() {
    let built = crate::window_check_w49_2::form_actions();
    assert!(built.contains("page.new"), "{built:?}");
    let mut opened = built.clone();
    opened.remove("page.new");
    assert_eq!(
        form_problems(&opened, &built),
        ["the action page.new builds a form that the census does not open"]
    );
    assert_eq!(form_problems(&built, &built), Vec::<String>::new());
}
