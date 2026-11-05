// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The accessibility census (WP-S2.11 D3, D5, D6; NFR-A11Y-02, G-M2-10): every surface of the window is opened on a
//! copy of Golden A that the suite's `Tmp` made (the committed project is only read), and the walker's six rules
//! (`support::accessibility_problems`, `support::availability_problems`) run on each. A failing node is a line of its
//! surface, and every line of every surface is reported at once.
//!
//! The surfaces: folio 101 with a device and a wire selected (the cards show); the six command-bar tabs; every menu of
//! the action registry and every submenu path in it; every `PaneKind` and `NavKind`, the Preview, History, Find and Messages panes with content;
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
use ew_ui::ew_viewmodel::actions::{ActionItem, MenuEntry, menu_model};
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
    // Every menu of the app menu and every submenu path in it (the command-bar groups, Open Recent, UI scale,
    // Theme); R6 reaches the entries of each, and an unavailable action's entry that is not shown is a line.
    let menu_button = strings::text(key!("commandbar-menu"));
    let mut disabled_entries = 0;
    let mut paths = Vec::new();
    for menu in menu_model(h.state().shell()) {
        menu_paths(vec![menu.label.clone()], &menu.entries, &mut paths);
    }
    for (path, items) in paths {
        let labels: Vec<&str> = path.iter().map(String::as_str).collect();
        support::open_menu(&mut h, &labels);
        let surface = format!("the {} menu", labels.join(" > "));
        census.visit(&mut h, &surface);
        census
            .problems
            .extend(support::menu_entry_problems(&h, &surface, &items));
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
        close_menus(&mut h);
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

/// Each menu path under `path` with the items it shows directly: `path` itself, then each submenu and each
/// command-bar group in it, depth first.
fn menu_paths(path: Vec<String>, entries: &[MenuEntry], out: &mut Vec<(Vec<String>, Vec<ActionItem>)>) {
    let items = entries
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Item(i) => Some(i.clone()),
            _ => None,
        })
        .collect();
    out.push((path.clone(), items));
    for e in entries {
        let mut inner = path.clone();
        match e {
            MenuEntry::Submenu { label, items, .. } => {
                inner.push(label.clone());
                out.push((inner, items.clone()));
            }
            MenuEntry::Group { label, entries, .. } => {
                inner.push(label.clone());
                menu_paths(inner, entries, out);
            }
            MenuEntry::Separator | MenuEntry::Item(_) => {}
        }
    }
}

/// Escape until no menu is open.
fn close_menus(h: &mut Harness<'_, App>) {
    for _ in 0..8 {
        if h.query_all_by_role(egui::Role::Menu).next().is_none() {
            return;
        }
        h.key_press(egui::Key::Escape);
        h.run_ok();
    }
    panic!("the app menu stays open");
}

/// A local mirror of [`Dialog`]'s variants: [`kind`] maps every variant without a wildcard arm, [`DialogKind::all`]
/// lists every kind and [`open_dialog`] opens it, so a new variant fails to compile until the census opens it.
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
    ImportQet,
    ReimportMatch,
    Reimport,
}

impl DialogKind {
    /// The kind after this one in the census's order, by a `match` without a wildcard arm: a new kind does not
    /// compile until it takes its place in the chain.
    fn next(self) -> Option<Self> {
        Some(match self {
            Self::About => Self::Shortcuts,
            Self::Shortcuts => Self::ConfirmDiscard,
            Self::ConfirmDiscard => Self::GotoPage,
            Self::GotoPage => Self::Notices,
            Self::Notices => Self::ConfirmJournalDiscard,
            Self::ConfirmJournalDiscard => Self::ConfirmCheckScheme,
            Self::ConfirmCheckScheme => Self::SaveWorkspace,
            Self::SaveWorkspace => Self::Form,
            Self::Form => Self::Recovery,
            Self::Recovery => Self::ConfirmPurge,
            Self::ConfirmPurge => Self::PartChoose,
            Self::PartChoose => Self::PartEdit,
            Self::PartEdit => Self::Print,
            Self::Print => Self::ExternalChange,
            Self::ExternalChange => Self::ConfirmUpgrade,
            Self::ConfirmUpgrade => Self::OpenEwz,
            Self::OpenEwz => Self::ConfirmGitSetup,
            Self::ConfirmGitSetup => Self::ImportQet,
            Self::ImportQet => Self::ReimportMatch,
            Self::ReimportMatch => Self::Reimport,
            Self::Reimport => return None,
        })
    }

    /// Every kind, from the first along [`DialogKind::next`].
    fn all() -> impl Iterator<Item = Self> {
        std::iter::successors(Some(Self::About), |k| k.next())
    }
}

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
        Dialog::ImportQet { .. } => DialogKind::ImportQet,
        Dialog::ReimportMatch { .. } => DialogKind::ReimportMatch,
        Dialog::Reimport { .. } => DialogKind::Reimport,
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
        // WP-S3.09: a QElectroTech file's import card; then, once it is a project, a changed copy of the file matched
        // to that import by its items (the proposal) and accepted (the re-import card).
        DialogKind::ImportQet | DialogKind::ReimportMatch | DialogKind::Reimport => {
            let qet = tmp.0.join("plant.qet");
            let project = |title: &str| {
                format!(
                    r#"<project version="0.100" title="Plant"><diagram title="{title}" folio="1" order="1" cols="17" colsize="60" rows="8" rowsize="80" height="640"/></project>"#
                )
            };
            std::fs::write(&qet, project("Supply")).unwrap();
            let file = qet.to_string_lossy().into_owned();
            let dir = tmp.0.join("plant.ew").to_string_lossy().into_owned();
            let mut h = simple("file.import_qet", json!({ "file": file, "dir": dir }));
            assert!(h.state_mut().shell_mut().wait_jobs().is_empty());
            if kind != DialogKind::ImportQet {
                dispatch(&mut h, "file.import_qet_apply", json!({ "confirm": true }));
                std::fs::write(&qet, project("Main supply")).unwrap();
                let accept = kind == DialogKind::Reimport;
                dispatch(
                    &mut h,
                    "file.reimport_qet",
                    json!({ "file": file, "accept": accept }),
                );
                // FT-1472: the re-import runs as a job whose end opens the card; waiting for that end (not for a
                // number of frames) makes the census independent of the machine's load.
                assert!(h.state_mut().shell_mut().wait_jobs().is_empty());
            }
            h
        }
    }
}

/// D3: every dialog of the shell, each in its state.
#[test]
fn census_every_dialog() {
    let mut census = Census::default();
    for want in DialogKind::all() {
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

/// FT-1424: every context menu that has a registry entry (`command_bar::canvas_menu` over an object, over nothing and
/// over a placement of a panel page; `command_bar::navigator_row_menu` on a Potentials row), opened by a right click as
/// a user opens it, is a surface of the census.
#[test]
fn census_every_context_menu() {
    use ew_ui::ew_viewmodel::canvas::{pick, rect_centre};
    use ew_ui::ew_viewmodel::command_bar::canvas_menu;
    let mut census = Census::default();
    // The centre of an object's rectangle whose pick is that object, on the page on show; `only` narrows the objects.
    let object = |h: &Harness<'_, App>, only: &dyn Fn(String) -> bool| {
        let c = h.state().shell().canvas().unwrap();
        let page = std::sync::Arc::clone(c.page());
        page.entities
            .iter()
            .map(|(e, r)| (*e, rect_centre(*r)))
            .find(|(e, p)| {
                only(e.0.to_string())
                    && c.link_at(*p).is_none()
                    && pick(&page.entities, Some(*p), 0.0).map(|(x, _)| x) == Some(*e)
            })
            .map(|(_, p)| p)
            .expect("an object")
    };
    // A right click at `at`; the menu's first entry shows.
    let open = |h: &mut Harness<'_, App>, at: egui::Pos2, over: bool, census: &mut Census, surface: &str| {
        crate::tools::right_click(h, at);
        let first = canvas_menu(h.state().shell(), over)
            .first()
            .map(|i| i.label.clone())
            .expect("a menu");
        assert!(h.query_by_label_contains(&first).is_some(), "{surface}: {first}");
        census.visit(h, surface);
    };
    // Each menu in a window of its own: a right click elsewhere first closes an open menu.
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "nothing.ew");
    let rep = h.state().shell().canvas().unwrap().page().tables.reps[0]
        .id
        .to_string();
    dispatch(&mut h, "select.set", json!({ "ids": [rep] }));
    dispatch(&mut h, "edit.copy", json!({}));
    dispatch(&mut h, "select.set", json!({ "ids": [] }));
    let bounds = crate::tools::canvas_bounds(&h)[0];
    let nothing = egui::Pos2::new(bounds.0 as f32 + 12.0, bounds.1 as f32 + 12.0);
    open(
        &mut h,
        nothing,
        false,
        &mut census,
        "the canvas context menu over nothing",
    );
    h.state_mut().shell_mut().shutdown();
    let mut h = on_golden(&tmp, "object.ew");
    let at = crate::tools::screen(&h, object(&h, &|_| true));
    open(
        &mut h,
        at,
        true,
        &mut census,
        "the canvas context menu over an object",
    );
    h.state_mut().shell_mut().shutdown();
    let mut h = on_golden(&tmp, "placement.ew");
    dispatch(&mut h, "page.goto", json!({ "folio": "902" }));
    wait_for_page(&mut h);
    h.run_ok();
    let placements: Vec<String> = h
        .state()
        .shell()
        .panel_tables()
        .expect("a panel page")
        .placements
        .iter()
        .map(|p| p.id.to_string())
        .collect();
    let at = crate::tools::screen(&h, object(&h, &|e| placements.contains(&e)));
    open(
        &mut h,
        at,
        true,
        &mut census,
        "the canvas context menu over a placement",
    );
    assert!(h.state().shell().selected_placements() > 0, "a placement");
    h.state_mut().shell_mut().shutdown();
    let mut h = on_golden(&tmp, "row.ew");
    let (name, _) = crate::potential_define::a_row_to_define(&mut h);
    h.get_by_label(&name).click_secondary();
    h.run_ok();
    assert!(
        h.query_by_label(strings::text(key!("action-potential-draw-row")))
            .is_some(),
        "the row's menu"
    );
    census.visit(&mut h, "the context menu of a Potentials row");
    h.state_mut().shell_mut().shutdown();
    census.assert_clean();
}

/// A tool of [`every_tool_cancels_with_escape_and_changes_nothing`]: the folio, the tool's name and the steps that start
/// it.
type ToolRow<'a> = (&'a str, &'a str, Vec<(&'a str, Value)>);

/// FT-1444 (NFR-UX-07, doc 16 §10.5): every tool started on its page gives way to Select at Escape and leaves the
/// revision as it was. Each row starts one tool as a user does (its action, a picker's choice, a copied selection).
#[test]
fn every_tool_cancels_with_escape_and_changes_nothing() {
    let tmp = Tmp::new();
    let mut h = on_golden(&tmp, "tools.ew");
    let mut started = BTreeSet::new();
    let mut problems = Vec::new();
    // `<rep>`, `<wire>` and `<device>` stand for the first representation and connection of the page on show and the
    // first device the panel page has not placed.
    let rows: [ToolRow<'_>; 16] = [
        ("101", "wire", vec![("tool.wire", json!({}))]),
        ("101", "ip", vec![("tool.ip", json!({}))]),
        ("101", "text", vec![("tool.text", json!({}))]),
        ("101", "graphic", vec![("tool.graphic", json!({}))]),
        ("101", "marker", vec![("tool.marker", json!({}))]),
        ("101", "structure_box", vec![("tool.structure_box", json!({}))]),
        ("101", "measure", vec![("tool.measure", json!({}))]),
        ("101", "image", vec![("tool.image", json!({}))]),
        (
            "101",
            "route_edit",
            vec![
                ("select.set", json!({"ids": ["<wire>"]})),
                ("tool.route_edit", json!({})),
            ],
        ),
        (
            "101",
            "place",
            vec![
                ("tool.place", json!({})),
                ("picker.filter", json!({"text": "lamp"})),
                ("picker.choose", json!({})),
            ],
        ),
        (
            "101",
            "black_box",
            vec![("tool.black_box", json!({"part": "EW-GEN-CPU-1", "template": 0}))],
        ),
        (
            "101",
            "paste",
            vec![
                ("select.set", json!({"ids": ["<rep>"]})),
                ("edit.copy", json!({})),
                ("edit.paste_clip", json!({})),
            ],
        ),
        (
            "101",
            "exchange",
            vec![
                ("select.set", json!({"ids": ["<rep>"]})),
                ("rep.exchange", json!({})),
                ("picker.choose", json!({})),
            ],
        ),
        ("902", "rail_draw", vec![("tool.rail_draw", json!({}))]),
        ("902", "duct_draw", vec![("tool.duct_draw", json!({}))]),
        (
            "902",
            "layout_place",
            vec![("tool.place_layout", json!({"device": "<device>"}))],
        ),
    ];
    for (folio, want, steps) in rows {
        dispatch(&mut h, "page.goto", json!({ "folio": folio }));
        wait_for_page(&mut h);
        dispatch(&mut h, "select.set", json!({"ids": []}));
        let shell = h.state().shell();
        let tables = &shell.canvas().expect("a page").page().tables;
        let fill = [
            ("<rep>", tables.reps.first().map(|r| r.id.to_string())),
            ("<wire>", tables.connections.first().map(|c| c.id.to_string())),
            (
                "<device>",
                shell
                    .panel_tables()
                    .and_then(|p| p.unplaced.first().map(|u| u.device.to_string())),
            ),
        ];
        for (action, params) in &steps {
            let mut text = params.to_string();
            for (mark, id) in &fill {
                if let Some(id) = id {
                    text = text.replace(mark, id);
                }
            }
            let params: Value = serde_json::from_str(&text).unwrap();
            let out = h.state_mut().shell_mut().dispatch(action, &params);
            if out != Outcome::Done {
                problems.push(format!("{want}: {action} gave {out:?}"));
            }
        }
        h.run_ok();
        let name = h.state().shell().tool().name();
        if name != want {
            problems.push(format!("{want}: {name} started"));
        }
        if name == "select" {
            continue;
        }
        started.insert(name.to_owned());
        let rev_started = h.state().shell().revision();
        for _ in 0..3 {
            if h.state().shell().tool().name() == "select" {
                break;
            }
            h.key_press(egui::Key::Escape);
            h.run_ok();
        }
        let after = h.state().shell().tool().name();
        if after != "select" {
            problems.push(format!("{want}: {name} is still active after Escape"));
        }
        if h.state().shell().revision() != rev_started {
            problems.push(format!("{want}: Escape changed the revision of {name}"));
        }
    }
    h.state_mut().shell_mut().shutdown();
    assert_eq!(problems, Vec::<String>::new(), "started: {started:?}");
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
