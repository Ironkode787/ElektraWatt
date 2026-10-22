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
use egui_kittest::kittest::NodeT as _;
use ew_ui::app::{App, Pick, Platform, setup};
use ew_ui::ew_viewmodel::shell::{Mode, Prompt, Resources, ShellConfig, ShellModel};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../content");

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A fresh temporary directory, removed on drop.
pub struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new() -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ew-ui-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Tmp(dir)
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
    for _ in 0..600 {
        harness.run_ok();
        if harness.state().page_view_ready() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the page image did not arrive");
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
        // The one exception: the splitter handles of the dock are egui_tiles' own `interact` areas, which publish
        // neither a role nor a name (upstream; the dock's panes, titles and tabs are named, a follow-up FT names the
        // splitters). Exactly as many handles as the dock has splitters are let through, no other unnamed node.
        if node.role() == egui::accesskit::Role::Unknown {
            splitters += 1;
            continue;
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
        "the unnamed handles are the dock's splitters and nothing else"
    );
}
