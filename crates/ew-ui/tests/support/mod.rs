// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support of the shell tests: a temporary copy of Golden A, a harness around the application frame without
//! rendering, a scripted file dialog, and the accessible-name walker (ADR-0014 amendment 2, NFR-A11Y-02).

#![allow(dead_code, missing_docs)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_ui::app::{App, Pick, Platform, setup};
use ew_ui::ew_viewmodel::shell::{Mode, Prompt, Resources, ShellConfig, ShellModel};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
pub struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new() -> Tmp {
        Tmp(fresh_dir(&std::env::temp_dir(), "ew-ui-test", &COUNTER))
    }

    pub fn config(&self) -> ShellConfig {
        ShellConfig::in_dir(&self.0.join("config"), "0.0.0-test")
    }

    /// The configuration with the sample project and the starter content of the source tree as resources.
    pub fn config_with_resources(&self) -> ShellConfig {
        self.config().with_resources(Resources {
            sample_dir: Some(PathBuf::from(GOLDEN_A)),
            content_dir: Some(PathBuf::from(CONTENT)),
            docs_dir: None,
            notices_file: None,
        })
    }

    /// A copy of Golden A in this directory.
    pub fn golden(&self, name: &str) -> PathBuf {
        let target = self.0.join(name);
        copy_dir(Path::new(GOLDEN_A), &target);
        target
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A new, empty directory `<parent>/<stem>-<pid>-<n>` that this call created (FT-524, FT-538). A test process that is
/// killed (a cancelled or timed-out run) leaves its directory behind with the project's lock, the journal, the
/// settings and the dock layout in it; a later test process that gets the same PID (Windows and Linux reuse them)
/// used to adopt it through `create_dir_all`, and opened the project read-only, with a recovery offer or another
/// layout. `create_dir` fails on such a leftover, and the next number is taken instead; the leftover is not touched.
pub fn fresh_dir(parent: &Path, stem: &str, counter: &AtomicU32) -> PathBuf {
    loop {
        let n = counter.fetch_add(1, Ordering::Relaxed);
        let dir = parent.join(format!("{stem}-{}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("cannot create the test directory {}: {e}", dir.display()),
        }
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The file dialog of a test: answers are queued by the test, and every prompt is recorded.
#[derive(Clone, Default)]
pub struct Scripted {
    pub answers: Rc<RefCell<VecDeque<Pick>>>,
    pub asked: Rc<RefCell<Vec<Prompt>>>,
    /// The URLs the frame asked the system browser to open.
    pub opened: Rc<RefCell<Vec<String>>>,
}

impl Platform for Scripted {
    fn pick(&mut self, prompt: Prompt) -> Pick {
        self.asked.borrow_mut().push(prompt);
        self.answers.borrow_mut().pop_front().unwrap_or(Pick::Cancelled)
    }

    fn open_url(&mut self, url: &str) -> bool {
        self.opened.borrow_mut().push(url.to_owned());
        true
    }
}

/// A harness around the application frame: no wgpu, no snapshot, fonts installed before the first pass.
pub fn harness<'a>(tmp: &Tmp, platform: Scripted, initial: Option<PathBuf>) -> Harness<'a, App> {
    harness_with(ShellModel::new(Mode::Gui, tmp.config()), platform, initial)
}

/// A harness around the frame of a shell the test made itself (resources, a first-run flow already started).
pub fn harness_with<'a>(shell: ShellModel, platform: Scripted, initial: Option<PathBuf>) -> Harness<'a, App> {
    let app = App::new(shell, Box::new(platform), initial);
    Harness::builder()
        .with_size(Vec2::new(1000.0, 700.0))
        .with_context_setup(setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app)
}

/// Runs the frame until it settles, then checks that every input widget has a name.
#[track_caller]
pub fn settle(harness: &mut Harness<'_, App>) {
    harness.run_ok();
    assert_named(harness);
}

/// Waits (bounded) for the render worker's image of the page on show.
#[track_caller]
pub fn wait_for_page(harness: &mut Harness<'_, App>) {
    let started = ew_render2d::stats::now();
    for _ in 0..600 {
        harness.run_ok();
        if harness.state().page_view_ready() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // FT-524: what the next sighting needs to tell a slow worker from a project that opened otherwise.
    let shell = harness.state().shell();
    panic!(
        "the page image did not arrive after {:?} (access {:?}, banner {:?}, folio {:?})",
        started.elapsed(),
        shell.access(),
        shell.banner(),
        shell.current_folio()
    );
}

/// Waits (bounded) for the render worker's image of the wizard's preview.
#[track_caller]
pub fn wait_for_preview(harness: &mut Harness<'_, App>) {
    for _ in 0..600 {
        harness.run_ok();
        if harness.state().preview_ready() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the preview image did not arrive");
}

/// The accessible-name walker: every visible AccessKit node a user can reach (an input role, or one that takes the
/// keyboard focus) has a name, a label or a placeholder. Read-only text is no input.
#[track_caller]
pub fn assert_named(harness: &Harness<'_, App>) {
    let mut unnamed = Vec::new();
    let mut splitters = 0;
    for node in harness.root().children_recursive() {
        let node = node.accesskit_node();
        if node.is_hidden() || node.data().is_read_only() {
            continue;
        }
        let reachable = egui::accessibility::is_input(node.role())
            || node.data().supports_action(egui::accesskit::Action::Focus);
        if !reachable {
            continue;
        }
        // FT-338: the dock's and the canvas area's handles are named `Splitter` nodes ("Resize Pages and Page view")
        // like every other input; no unnamed node is let through any more. (A table's column handles and egui's
        // panel handle are splitters with other names.)
        if node.role() == egui::accesskit::Role::Splitter
            && node
                .label()
                .is_some_and(|l| l.starts_with("Resize ") && l.contains(" and "))
        {
            splitters += 1;
        }
        let named = node.label().is_some_and(|l| !l.trim().is_empty())
            || node.data().placeholder().is_some_and(|p| !p.trim().is_empty());
        if !named {
            unnamed.push(format!("{:?} at {:?}", node.role(), node.raw_bounds()));
        }
    }
    assert!(
        unnamed.is_empty(),
        "widgets without an accessible name: {unnamed:#?}"
    );
    assert_eq!(
        splitters,
        harness.state().splitter_count(),
        "every splitter handle of the dock and the canvas area is a `Splitter` node"
    );
}

// WP-S2.17 part 2: the app menu replaced the menu bar, the HUD the tool strip.

/// The value of the tool HUD's `Status` node ("Select · Click to select …"); empty without a HUD.
pub fn hud_value(harness: &Harness<'_, App>) -> String {
    harness
        .query_all_by_role_and_label(egui::accesskit::Role::Status, "Tool")
        .next()
        .and_then(|n| n.accesskit_node().value())
        .unwrap_or_default()
}

/// The role and label of every ancestor of `node`.
fn ancestors(node: &egui_kittest::Node<'_>) -> Vec<(egui::accesskit::Role, String)> {
    let mut out = Vec::new();
    let mut at = node.accesskit_node().parent();
    while let Some(n) = at {
        out.push((n.role(), n.label().unwrap_or_default()));
        at = n.parent();
    }
    out
}

/// Whether `node` is in the command bar or the HUD and not in a menu (a popup menu's node is a child of the button
/// that opened it).
fn in_chrome(node: &egui_kittest::Node<'_>) -> bool {
    for (role, label) in ancestors(node) {
        if role == egui::accesskit::Role::Menu {
            return false;
        }
        if (role == egui::accesskit::Role::Toolbar && label == "Command bar")
            || (role == egui::accesskit::Role::Status && label == "Tool")
        {
            return true;
        }
    }
    false
}

/// The node labelled `label` outside the command bar and the HUD: an entry of the open menus (the app menu's
/// submenus share their names with the command tabs, some items with the bar's buttons).
pub fn menu_node<'t>(harness: &'t Harness<'_, App>, label: &'t str) -> egui_kittest::Node<'t> {
    // WP-S2.17 part 3: the rails and the panel headers share names with the submenus ("Project"): an entry inside an
    // open menu first.
    harness
        .query_all_by_label(label)
        .rfind(|n| {
            ancestors(n)
                .iter()
                .any(|(role, _)| *role == egui::accesskit::Role::Menu)
        })
        .or_else(|| off_chrome(harness, label))
        .unwrap_or_else(|| panic!("no menu entry {label:?}"))
}

/// The last node labelled `label` outside the command bar and the HUD (a dialog's "Renumber" next to the bar's
/// Renumber button, the toast's "Undo" next to the quick-access Undo), if any.
pub fn off_chrome<'t>(harness: &'t Harness<'_, App>, label: &'t str) -> Option<egui_kittest::Node<'t>> {
    harness.query_all_by_label(label).rfind(|n| !in_chrome(n))
}

/// The `Tab` nodes of the page view and the docks, without the command bar's tabs.
pub fn view_tabs<'t>(harness: &'t Harness<'_, App>) -> Vec<egui_kittest::Node<'t>> {
    harness
        .get_all_by_role(egui::accesskit::Role::Tab)
        .filter(|n| !in_chrome(n))
        // WP-S2.17 part 3: the Project entry's segmented control is no view tab.
        .filter(|n| {
            !ancestors(n)
                .iter()
                .any(|(role, label)| *role == egui::accesskit::Role::TabList && label == "Pages or devices")
        })
        .collect()
}

/// Opens the app menu (its button "Menu") and then each submenu of `path` in turn ("File", "Import").
pub fn open_menu(harness: &mut Harness<'_, App>, path: &[&str]) {
    harness.get_by_label("Menu").click();
    harness.run_ok();
    for label in path {
        menu_node(harness, label).click();
        harness.run_ok();
    }
}

/// The command-bar button labelled `label` (the HUD and the menus may hold an entry of that name too).
pub fn bar_button<'t>(harness: &'t Harness<'_, App>, label: &'t str) -> egui_kittest::Node<'t> {
    harness
        .query_all_by_label(label)
        .find(|n| {
            in_chrome(n)
                && ancestors(n)
                    .iter()
                    .any(|(role, l)| *role == egui::accesskit::Role::Toolbar && l == "Command bar")
        })
        .unwrap_or_else(|| panic!("no command-bar button {label:?}"))
}
