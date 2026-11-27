// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test support of the shell tests: a temporary copy of the sample project, a harness around the
//! application frame without rendering, a scripted file dialog, and the accessibility walker (ADR-0014 amendment 2,
//! NFR-A11Y-02).

#![allow(missing_docs)]

pub mod colour;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;

use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use ew_ui::app::{App, Pick, Platform, setup};
use ew_ui::clock::Clock;
use ew_ui::visual::Driver;
use ew_viewmodel::shell::{Mode, Outcome, Prompt, Resources, ShellConfig, ShellModel};
use ew_viewmodel::{key, strings};

/// The temporary directory of a test ([`ew_testdir::Tmp`], reached through `Deref`: `dir()`, `path()`, `sample()` and
/// the public path `.0`), with the shell configurations of the tests.
pub struct Tmp(ew_testdir::Tmp);

impl std::ops::Deref for Tmp {
    type Target = ew_testdir::Tmp;

    fn deref(&self) -> &ew_testdir::Tmp {
        &self.0
    }
}

impl Tmp {
    pub fn new() -> Tmp {
        Tmp(ew_testdir::Tmp::new("ew-ui-test"))
    }

    pub fn config(&self) -> ShellConfig {
        ShellConfig::in_dir(&self.0.0.join("config"), "0.0.0-test")
    }

    /// The configuration with the sample project and the starter content of the source tree as resources.
    pub fn config_with_resources(&self) -> ShellConfig {
        self.config().with_resources(Resources {
            sample_dir: Some(PathBuf::from(ew_testdir::SAMPLE)),
            content_dir: Some(PathBuf::from(ew_testdir::CONTENT)),
            docs_dir: None,
            notices_file: None,
            privacy_file: None,
        })
    }
}

/// The file dialog of a test: answers are queued by the test, and every prompt is recorded.
#[derive(Clone, Default)]
pub struct Scripted {
    pub answers: Rc<RefCell<VecDeque<Pick>>>,
    pub asked: Rc<RefCell<Vec<Prompt>>>,
    /// The URLs the frame asked the system browser to open.
    pub opened: Rc<RefCell<Vec<String>>>,
    /// The written files the frame asked the system's file opener to open.
    pub opened_files: Rc<RefCell<Vec<PathBuf>>>,
    /// The projects the frame asked to open in a second window of the program.
    pub opened_windows: Rc<RefCell<Vec<PathBuf>>>,
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

    fn open_file(&mut self, file: &std::path::Path) -> bool {
        self.opened_files.borrow_mut().push(file.to_path_buf());
        true
    }

    fn open_window(&mut self, project: &std::path::Path) -> bool {
        self.opened_windows.borrow_mut().push(project.to_path_buf());
        true
    }
}

/// A harness around the application frame: no wgpu, no snapshot, fonts installed before the first pass.
pub fn harness<'a>(tmp: &Tmp, platform: Scripted, initial: Option<PathBuf>) -> Harness<'a, App> {
    harness_with(ShellModel::new(Mode::Gui, tmp.config()), platform, initial)
}

/// The clock every window test runs on: the frame clock, so a frame takes the page's tiles that the frame before
/// asked for, and no test of the window reads the wall clock.
pub const CLOCK: Clock = Clock::Frame;

/// The main resolution, in points (doc 16a §2): the size of every harness's window.
pub const MAIN: Vec2 = Vec2::new(1920.0, 1080.0);

/// A harness around the frame of a shell the test made itself (resources, a first-run flow already started), in a
/// window of 1920 × 1080 points, the main resolution (doc 16a §2), on [`CLOCK`]. Two widgets of one ID in a view the
/// test opens fail it in the frame that paints them: egui writes its warning ("First use of widget ID …") behind a
/// glyph the window's fonts lack (U+1F525), and the harness's missing-glyph policy refuses it ("No glyph for …").
pub fn harness_with<'a>(shell: ShellModel, platform: Scripted, initial: Option<PathBuf>) -> Harness<'a, App> {
    let app = App::new(shell, Box::new(platform), initial, CLOCK);
    Harness::builder()
        .with_size(MAIN)
        .with_context_setup(setup)
        .build_ui_state(|ui, app: &mut App| app.ui(ui), app)
}

/// Runs the frame until it settles, then checks every interactive widget's name, role and state
/// ([`assert_accessible`]). The walk costs about 1 ms against about 10 ms per frame of the window (debug build,
/// measured 2026-10-29), and the tree a settle ends on is not the one of the settle before, so it runs on every
/// settle.
#[track_caller]
pub fn settle(harness: &mut Harness<'_, App>) {
    harness.run_ok();
    assert_accessible(harness);
}

/// Runs frames until no job runs and no print build is due, waiting for each running job's own end and the print
/// pre-dialog's build between them: an import reads its file in a job and goes on in a later frame. No clock: a
/// loaded machine ends the job later, never not.
#[track_caller]
pub fn finish_jobs(harness: &mut Harness<'_, App>) {
    for _ in 0..50 {
        harness.run_ok();
        let shell = harness.state().shell();
        if shell.jobs().running() == 0 && !shell.print_waits() {
            harness.run_ok();
            return;
        }
        wait_for_workers(harness);
    }
    panic!("a job is still running after 50 frames that each waited for it");
}

/// Waits for the running jobs without running frames, then settles: for a test of what the ends show, not of how the
/// window takes them (that is `jobs.rs`'s). Frames run the window's clock on, so polling through a long export ran an
/// earlier toast's 8 seconds out before the assertions.
#[track_caller]
pub fn wait_jobs_and_settle(harness: &mut Harness<'_, App>) {
    harness.state_mut().shell_mut().wait_jobs();
    settle(harness);
    assert_eq!(harness.state().shell().jobs().running(), 0);
}

/// Runs frames until `done` holds of the shell, waiting between them for the running jobs' ends and the print
/// build as [`finish_jobs`] does, then settles. After Create or Enter a test waits on the state, not on one frame.
#[track_caller]
pub fn wait_until(harness: &mut Harness<'_, App>, what: &str, done: impl Fn(&ShellModel) -> bool) {
    for _ in 0..50 {
        harness.run_ok();
        if done(harness.state().shell()) {
            settle(harness);
            return;
        }
        wait_for_workers(harness);
    }
    panic!(
        "{what} did not happen (dialog {:?})",
        harness.state().shell().dialog()
    );
}

/// Blocks on the end of every running job and on a print build no test gate holds, taking their ends.
fn wait_for_workers(harness: &mut Harness<'_, App>) {
    let shell = harness.state_mut().shell_mut();
    if shell.jobs().running() > 0 {
        shell.wait_jobs();
    }
    if shell.print_waits() {
        shell.wait_print();
    }
}

/// Runs frames until the page on show is drawn. On [`CLOCK`] a frame waits for the tiles the frame before asked
/// for, so the frames are a bound and no wait on time.
#[track_caller]
pub fn wait_for_page(harness: &mut Harness<'_, App>) {
    for _ in 0..600 {
        harness.run_ok();
        if harness.state().page_view_ready() {
            return;
        }
    }
    // What the next sighting needs to tell a slow worker from a project that opened otherwise.
    let shell = harness.state().shell();
    panic!(
        "the page image did not arrive (access {:?}, banner {:?}, folio {:?})",
        shell.access(),
        shell.banner(),
        shell.current_folio()
    );
}

/// Runs `action` through the shell and asserts it was done.
#[track_caller]
pub fn dispatch(harness: &mut Harness<'_, App>, action: &str, params: serde_json::Value) {
    assert_eq!(
        harness.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The project's numbering set to the page-based scheme (`{page}{letter}{counter}`), so that Renumber devices… has tags
/// to change.
#[track_caller]
pub fn page_based_numbering(harness: &mut Harness<'_, App>) {
    dispatch(
        harness,
        "command.run",
        serde_json::json!({"cmd": "project.settings.set", "params": {"member": "numbering", "value": {"default": "page-based",
            "schemes": [{"key": "page-based", "format": "{page}{letter}{counter}", "rules": [], "child_separator": "-"}]}}}),
    );
}

/// The function text of -101F1 set to `text` as one command.
#[track_caller]
pub fn set_function_text(harness: &mut Harness<'_, App>, text: &str) {
    dispatch(
        harness,
        "command.run",
        serde_json::json!({"cmd": "fields.set", "params": {"kind": "device", "targets": [{"ref": "tag:-101F1"}],
            "path": "function_text", "value": text}}),
    );
}

/// Runs `action` on a shell and asserts it was done.
#[track_caller]
pub fn done(shell: &mut ShellModel, action: &str, params: serde_json::Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The parts table of the test tables, whose columns [`import_parts`] takes.
const TABLE_PARTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/tables/parts.csv");

/// The row of the test tables' parts table whose key is `key`.
pub fn table_part(key: &str) -> String {
    std::fs::read_to_string(TABLE_PARTS)
        .unwrap()
        .lines()
        .find(|l| l.starts_with(&format!("{key},")))
        .unwrap_or_else(|| panic!("the row of {key}"))
        .to_owned()
}

/// `rows` (in the columns of the test tables' parts table) imported into the open copy of the sample: project part copies
/// the sample's starter parts do not give (a supplier's part number, a long order code, a duct without a size).
pub fn import_parts(harness: &mut Harness<'_, App>, tmp: &Tmp, rows: &[String]) {
    let table = std::fs::read_to_string(TABLE_PARTS).unwrap();
    let header = table.lines().next().expect("the header");
    let file = tmp.path("parts.csv");
    std::fs::write(&file, format!("{header}\n{}\n", rows.join("\n"))).unwrap();
    // The copy is the sample's project: its parts are read for the sample, as `ew import parts` reads them (the window's
    // Import… puts a table's parts into My parts).
    let (sample, _) =
        ew_viewmodel::ew_api::Session::open_read_only(std::path::Path::new(ew_testdir::SAMPLE), "test")
            .expect("the sample");
    let read = ew_viewmodel::ew_api::import::read_parts_tables(&file, &sample).expect("the rows read");
    dispatch(
        harness,
        "command.run",
        serde_json::json!({"cmd": "parts.import", "params": read.params()}),
    );
    settle(harness);
}

/// The sample's panel page 902 places every device of the cabinet: the placement of `tag` deleted, the page has a
/// device to place. Leaves 902 on show.
#[track_caller]
pub fn unplace(harness: &mut Harness<'_, App>, tag: &str) {
    dispatch(harness, "page.goto", serde_json::json!({"folio": "902"}));
    wait_for_page(harness);
    let placement = harness
        .state()
        .shell()
        .panel_tables()
        .expect("the panel page")
        .placements
        .iter()
        .find(|p| p.tag == tag)
        .unwrap_or_else(|| panic!("the placement of {tag}"))
        .id
        .to_string();
    dispatch(harness, "select.set", serde_json::json!({ "ids": [placement] }));
    dispatch(harness, "edit.delete_selection", serde_json::json!({}));
    settle(harness);
}

/// The ID of the wire of the page on show between the connection points labelled `a` and `b`.
pub fn wire_between(h: &Harness<'_, App>, a: &str, b: &str) -> String {
    let tables = &h.state().shell().canvas().expect("a page").page().tables;
    let at = |label: &str| {
        tables
            .cps
            .iter()
            .find(|p| p.label == label)
            .unwrap_or_else(|| panic!("no connection point {label}"))
    };
    let (a, b) = (at(a), at(b));
    a.connections
        .iter()
        .find(|c| b.connections.contains(c))
        .expect("a wire between the two")
        .to_string()
}

/// The window position (points) of the sheet point `p` on the focused canvas.
pub fn screen(harness: &Harness<'_, App>, p: (i64, i64)) -> egui::Pos2 {
    let bounds = harness
        .get_all_by_role(egui::Role::Canvas)
        .next()
        .expect("the canvas node")
        .accesskit_node()
        .raw_bounds()
        .expect("bounds");
    let c = harness.state().shell().canvas().unwrap();
    let k = c.points_per_nm();
    egui::Pos2::new(
        bounds.x0 as f32 + ((p.0 - c.centre().0) as f64 * k + c.pane().0 / 2.0) as f32,
        bounds.y0 as f32 + ((p.1 - c.centre().1) as f64 * k + c.pane().1 / 2.0) as f32,
    )
}

/// An edit of two devices' function text on folio 101: one edit toast on show.
pub fn edit_toast(harness: &mut Harness<'_, App>) {
    dispatch(harness, "page.goto", serde_json::json!({"folio": "101"}));
    dispatch(
        harness,
        "select.set",
        serde_json::json!({"refs": ["tag:-101F1", "tag:-102F1"]}),
    );
    dispatch(
        harness,
        "props.set",
        serde_json::json!({"row": "device/function_text", "value": "Spare"}),
    );
}

/// The window on a writable copy of the sample project on the page it opens on, settled, its image arrived, settled.
pub fn on_sample_first_page(tmp: &Tmp) -> Harness<'static, App> {
    on_sample_sized(tmp, "s.ew", None, MAIN)
}

/// The window of `size` points on a writable copy `name` of the sample project, settled with its image arrived; with
/// `folio`, that page is on show and its image arrived too.
pub fn on_sample_sized(tmp: &Tmp, name: &str, folio: Option<&str>, size: Vec2) -> Harness<'static, App> {
    let mut h = harness(tmp, Scripted::default(), Some(tmp.sample(name)));
    h.set_size(size);
    settle(&mut h);
    wait_for_page(&mut h);
    if let Some(folio) = folio {
        dispatch(&mut h, "page.goto", serde_json::json!({ "folio": folio }));
        settle(&mut h);
        wait_for_page(&mut h);
    }
    settle(&mut h);
    h
}

/// A visual driver of the window with its settings in `tmp`'s `config` (`config`, the same or with more) and its
/// accessibility tree on.
pub fn driver(tmp: &Tmp, config: ShellConfig) -> Driver {
    let mut d = Driver::in_folder(config, None, tmp.0.to_path_buf()).expect("the visual driver");
    d.enable_accesskit();
    d
}

/// A visual driver on a new project `p.ew` in `tmp` from the built-in template, settled.
pub fn driver_on_new_project(tmp: &Tmp, config: ShellConfig) -> Driver {
    let mut d = driver(tmp, config);
    let project = serde_json::json!({"dir": tmp.0.join("p.ew"), "template": "builtin"});
    assert_eq!(d.shell_mut().dispatch("file.new", &project), Outcome::Done);
    d.settle();
    d
}

/// A visual driver on a writable copy of the sample project with folio `folio` on show, settled.
pub fn driver_on_sample(tmp: &Tmp, folio: &str) -> Driver {
    let sample = tmp.sample("s.ew");
    let mut d = driver(tmp, tmp.config());
    let opened = d
        .shell_mut()
        .dispatch("file.open", &serde_json::json!({"dir": sample.to_string_lossy()}));
    assert_eq!(opened, Outcome::Done);
    let _ = d
        .shell_mut()
        .dispatch("page.goto", &serde_json::json!({ "folio": folio }));
    d.settle();
    d
}

/// The window on a writable copy of the sample project with folio `folio` on show and its image arrived.
pub fn on_sample(tmp: &Tmp, folio: &str) -> Harness<'static, App> {
    on_sample_sized(tmp, "s.ew", Some(folio), MAIN)
}

/// Runs frames until the image of the wizard's preview has arrived: on [`CLOCK`] it is rendered in the frame that
/// asks for it.
#[track_caller]
pub fn wait_for_preview(harness: &mut Harness<'_, App>) {
    for _ in 0..600 {
        harness.run_ok();
        if harness.state().preview_ready() {
            return;
        }
    }
    panic!("the preview image did not arrive");
}

/// Every shape the last frame painted, nested ones flattened, each with its clip rectangle.
pub fn shapes(harness: &Harness<'_, App>) -> Vec<(egui::Shape, egui::Rect)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<(egui::Shape, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            s => out.push((s.clone(), clip)),
        }
    }
    let mut out = Vec::new();
    for c in &harness.output().shapes {
        walk(&c.shape, c.clip_rect, &mut out);
    }
    out
}

/// Every text the last frame painted, in painting order.
pub fn texts(harness: &Harness<'_, App>) -> Vec<egui::epaint::TextShape> {
    shapes(harness)
        .into_iter()
        .filter_map(|(s, _)| match s {
            egui::Shape::Text(t) => Some(t),
            _ => None,
        })
        .collect()
}

/// The surface a settle checks, for [`assert_accessible`]'s lines: the open dialog's title, the first run, or the
/// window.
pub fn surface_name(harness: &Harness<'_, App>) -> String {
    let shell = harness.state().shell();
    if let Some(dialog) = shell.dialog() {
        ew_viewmodel::dialogs::spec(dialog, shell).title
    } else if shell.first_run().is_some() {
        strings::text(key!("firstrun-title")).to_owned()
    } else {
        "window".to_owned()
    }
}

/// The accessibility walker (ADR-0014 amendment 2, NFR-A11Y-02): every visible, enabled AccessKit node a
/// user can reach (an input role, or one that takes the keyboard focus) has (R1) a name, a label or a placeholder;
/// (R2) a check box, switch or radio button its toggled state; (R3) a combo box, a node with a pop-up or a tree item
/// with children its expanded state; (R4) an option, tab, row or tree item of a selectable container its selected
/// state; (R5) a text input or spin button a value (empty allowed). Read-only text is no input. Every failing node
/// is a line, so one run shows them all. R6 needs the action registry: [`availability_problems`].
pub fn accessibility_problems<S>(harness: &Harness<'_, S>, surface: &str) -> Vec<String> {
    use egui::accesskit::Role;
    let mut problems = Vec::new();
    for node in harness.root().children_recursive() {
        let node = node.accesskit_node();
        if node.is_hidden() || node.is_disabled() || node.data().is_read_only() {
            continue;
        }
        let role = node.role();
        let reachable = egui::accessibility::is_input(role)
            || node.data().supports_action(egui::accesskit::Action::Focus);
        if !reachable {
            continue;
        }
        let name = node.label().filter(|l| !l.trim().is_empty()).or_else(|| {
            node.data()
                .placeholder()
                .filter(|p| !p.trim().is_empty())
                .map(str::to_owned)
        });
        let Some(name) = name else {
            problems.push(format!(
                "{surface}: {role:?} at {:?} has no accessible name",
                node.raw_bounds()
            ));
            continue;
        };
        let mut states = Vec::new();
        if matches!(role, Role::CheckBox | Role::Switch | Role::RadioButton) && node.toggled().is_none() {
            states.push("toggled");
        }
        let has_children = node
            .children()
            .any(|c| c.role() == Role::TreeItem || c.data().is_expanded().is_some());
        if (role == Role::ComboBox || node.has_popup().is_some() || (role == Role::TreeItem && has_children))
            && node.data().is_expanded().is_none()
        {
            states.push("expanded");
        }
        let mut in_selectable = false;
        let mut at = node.parent();
        while let Some(a) = at {
            in_selectable |= matches!(
                a.role(),
                Role::ListBox | Role::TabList | Role::Tree | Role::TreeGrid | Role::Grid | Role::ListGrid
            );
            at = a.parent();
        }
        if matches!(role, Role::ListBoxOption | Role::Tab | Role::Row | Role::TreeItem)
            && in_selectable
            && node.is_selected().is_none()
        {
            states.push("selected");
        }
        if (role == Role::SpinButton || node.is_text_input())
            && node.value().is_none()
            && node.numeric_value().is_none()
        {
            states.push("value");
        }
        for state in states {
            problems.push(format!("{surface}: {role:?} \"{name}\" has no {state} state"));
        }
    }
    problems
}

/// R6 of the walker: a command-bar button or an entry of the open app menu whose action the registry reports
/// unavailable is shown disabled, not enabled.
pub fn availability_problems(harness: &Harness<'_, App>, surface: &str) -> Vec<String> {
    use egui::accesskit::Role;
    use ew_viewmodel::{actions, command_bar};
    let shell = harness.state().shell();
    let bar = command_bar::command_bar(shell);
    let mut problems = Vec::new();
    let menu_button = strings::text(key!("commandbar-menu"));
    // The names on show, read in one walk (a label's text is its value, as the query reads it): a name no node bears
    // has no node to check, and the query walks the whole tree once per name, which cost about 50 ms per settle. The
    // same walk sees whether a menu is open: without one no menu entry is shown, and the menu model, about 8 ms of a
    // 20 ms settle, is not built.
    let mut menu_open = false;
    let shown: std::collections::BTreeSet<String> = harness
        .root()
        .children_recursive()
        .filter_map(|n| {
            let n = n.accesskit_node();
            menu_open |= n.role() == Role::Menu;
            if n.role() == Role::Label {
                n.value()
            } else {
                n.label()
            }
        })
        .collect();
    let mut check = |label: &str, in_menu: bool| {
        if !shown.contains(label) {
            return;
        }
        for node in harness.query_all_by_label(label) {
            // The app menu's entries are children of its button ("Menu"); a context menu's are not.
            let at_place = if in_menu {
                let up = ancestors(&node);
                up.iter().any(|(r, _)| *r == Role::Menu)
                    && up.iter().any(|(r, l)| *r == Role::Button && l == menu_button)
            } else {
                in_chrome(&node)
            };
            let n = node.accesskit_node();
            if at_place && !n.is_hidden() && !n.is_disabled() && n.role() == Role::Button {
                problems.push(format!(
                    "{surface}: {:?} \"{label}\" is enabled, but its action is unavailable",
                    n.role()
                ));
            }
        }
    };
    let buttons = bar
        .quick
        .iter()
        .chain(bar.groups.iter().flat_map(|g| &g.buttons))
        .chain(&bar.next);
    // A folded group's button bears the group's name, which a button of the group may share (Draw ▸ Annotate ▸
    // Annotate): the button that opens a group is not that button.
    let group_names: Vec<&str> = bar.groups.iter().map(|g| g.label.as_str()).collect();
    // While the symbol editor shows, its own bar replaces the command bar's buttons.
    for b in buttons.filter(|b| {
        !b.enabled && shell.symbol_editor().is_none() && !group_names.contains(&b.bar_label.as_str())
    }) {
        check(&b.bar_label, false);
    }
    // A menu's own button bears the menu's name, which an entry elsewhere may share (the PLC menu and View ▸
    // Navigators ▸ PLC): the button that opens a menu is no entry of it.
    if !menu_open {
        return problems;
    }
    let menus = actions::menu_model(shell);
    let menu_names: Vec<&str> = menus.iter().map(|m| m.label.as_str()).collect();
    // An entry of the same name that is available elsewhere (a station under Rack… and, without cards waiting, under
    // Attach cards) is a node the walk cannot tell apart from this one.
    let available: std::collections::BTreeSet<String> = menus
        .iter()
        .flat_map(|m| m.items())
        .filter(|i| i.enabled)
        .map(menu_label)
        .collect();
    for menu in &menus {
        for item in menu.items().into_iter().filter(|i| !i.enabled) {
            let label = menu_label(item);
            if !menu_names.contains(&label.as_str()) && !available.contains(&label) {
                check(&label, true);
            }
        }
    }
    problems
}

/// The accessible name of a menu entry: its label, then its shortcut as the entry shows it (egui joins a button's
/// texts with a space).
pub fn menu_label(item: &ew_viewmodel::actions::ActionItem) -> String {
    if item.shortcut.is_empty() {
        item.label.clone()
    } else {
        format!(
            "{} {}",
            item.label,
            ew_viewmodel::keymap::short_display(&item.shortcut)
        )
    }
}

/// R6 on an open menu of the app menu: each of `items` whose action is unavailable has a
/// shown entry in it. One that has none is a line, so an entry never passes unseen; whether it is disabled is
/// [`availability_problems`]'s line.
pub fn menu_entry_problems(
    harness: &Harness<'_, App>,
    surface: &str,
    items: &[ew_viewmodel::actions::ActionItem],
) -> Vec<String> {
    use egui::accesskit::Role;
    let menu_button = strings::text(key!("commandbar-menu"));
    items
        .iter()
        .filter(|i| !i.enabled)
        .filter(|i| {
            !harness.query_all_by_label(&menu_label(i)).any(|node| {
                let up = ancestors(&node);
                let n = node.accesskit_node();
                up.iter().any(|(r, _)| *r == Role::Menu)
                    && up.iter().any(|(r, l)| *r == Role::Button && l == menu_button)
                    && n.role() == Role::Button
                    && !n.is_hidden()
            })
        })
        .map(|i| {
            format!(
                "{surface}: the unavailable action's entry \"{}\" is not shown",
                i.label
            )
        })
        .collect()
}

/// The walker on the surface on show, with the splitter count: every splitter handle of the dock and
/// the canvas area is a named `Splitter` node.
#[track_caller]
pub fn assert_accessible(harness: &Harness<'_, App>) {
    let surface = surface_name(harness);
    let mut problems = accessibility_problems(harness, &surface);
    problems.extend(availability_problems(harness, &surface));
    assert!(
        problems.is_empty(),
        "nodes that fail the accessibility rules: {problems:#?}"
    );
    let mut splitters = 0;
    // "Resize {0} and {1}": the text before the first argument and the one between the two.
    let splitter_name = strings::text(key!("splitter-name"));
    let (splitter_lead, rest) = splitter_name.split_once("{0}").unwrap_or_default();
    let splitter_join = rest.split_once("{1}").map_or(rest, |(join, _)| join);
    for node in harness.root().children_recursive() {
        let node = node.accesskit_node();
        // The dock's and the canvas area's handles are named `Splitter` nodes ("Resize Pages and Page view")
        // like every other input. (A table's column handles and egui's panel handle are splitters with other names.)
        // The name's form in the UI language.
        let reachable = egui::accessibility::is_input(node.role())
            || node.data().supports_action(egui::accesskit::Action::Focus);
        if !node.is_hidden()
            && !node.data().is_read_only()
            && reachable
            && node.role() == egui::accesskit::Role::Splitter
            && node
                .label()
                .is_some_and(|l| l.starts_with(splitter_lead) && l.contains(splitter_join))
        {
            splitters += 1;
        }
    }
    assert_eq!(
        splitters,
        harness.state().splitter_count(),
        "every splitter handle of the dock and the canvas area is a `Splitter` node"
    );
}

/// The fit check (NFR-I18N-02), beside [`assert_accessible`]: every visible labelled AccessKit node lies
/// inside the window and inside its nearest bounded ancestor across (1 px slack), and no two interactive siblings
/// overlap.
///
/// What egui's AccessKit tree cannot tell is left out: it carries no clip rectangle, so a node wholly outside the
/// window or its ancestor is scrolled out of view and not checked, and a node cut at the top or the bottom is a row
/// of a scrolled list (the pseudo-locale lengthens texts across, never down); a control inside another (a tab's
/// close button, a row's actions) is placed there, and splitter handles cross where the docks meet.
#[track_caller]
pub fn assert_fits(harness: &Harness<'_, App>) {
    let problems = fit_problems(harness);
    assert!(problems.is_empty(), "nodes that do not fit: {problems:#?}");
}

/// The problems [`assert_fits`] reports, each as `<role> "<label>" <what>`.
pub fn fit_problems<S>(harness: &Harness<'_, S>) -> Vec<String> {
    use egui::accesskit::{Action, Rect, Role};
    const SLACK: f64 = 1.0;
    let inside = |a: &Rect, b: &Rect| a.x0 >= b.x0 - SLACK && a.x1 <= b.x1 + SLACK;
    let contains = |a: &Rect, b: &Rect| {
        a.x0 <= b.x0 + SLACK && a.y0 <= b.y0 + SLACK && a.x1 >= b.x1 - SLACK && a.y1 >= b.y1 - SLACK
    };
    let apart = |a: &Rect, b: &Rect| a.x1 <= b.x0 || b.x1 <= a.x0 || a.y1 <= b.y0 || b.y1 <= a.y0;
    let overlap = |a: &Rect, b: &Rect| {
        a.x1.min(b.x1) - a.x0.max(b.x0) > SLACK && a.y1.min(b.y1) - a.y0.max(b.y0) > SLACK
    };
    // AccessKit's bounds are in physical pixels, the content rectangle in points.
    let screen = harness.ctx.content_rect() * harness.ctx.pixels_per_point();
    let window = Rect {
        x0: f64::from(screen.min.x),
        y0: f64::from(screen.min.y),
        x1: f64::from(screen.max.x),
        y1: f64::from(screen.max.y),
    };
    let mut problems = Vec::new();
    // (the parent node's ID, bounds, role and label) of the shown interactive nodes.
    let mut interactive: Vec<(String, Rect, String)> = Vec::new();
    for node in harness.root().children_recursive() {
        let node = node.accesskit_node();
        let Some(label) = node.label().filter(|l| !l.trim().is_empty()) else {
            continue;
        };
        let Some(bounds) = node.bounding_box().filter(|b| b.x1 > b.x0 && b.y1 > b.y0) else {
            continue;
        };
        if node.is_hidden() || apart(&bounds, &window) {
            continue;
        }
        // A table's columns and rows scroll in both directions: its cells and header are not cut, only scrolled.
        let mut ancestor = node.parent();
        let mut in_table = false;
        while let Some(a) = ancestor {
            in_table |= matches!(a.role(), Role::Table | Role::Grid | Role::TreeGrid);
            ancestor = a.parent();
        }
        let name = format!("{:?} {label:?} at {bounds:?}", node.role());
        let mut parent = node.parent();
        while let Some(p) = parent.filter(|p| p.bounding_box().is_none()) {
            parent = p.parent();
        }
        let parent_bounds = parent.and_then(|p| p.bounding_box());
        if parent_bounds.is_some_and(|p| apart(&bounds, &p)) {
            continue;
        }
        if !in_table && !inside(&bounds, &window) {
            problems.push(format!("{name} leaves the window"));
        }
        if let Some(p) = parent_bounds.filter(|p| !in_table && !inside(&bounds, p)) {
            problems.push(format!("{name} leaves its parent at {p:?}"));
        }
        let data = node.data();
        if node.role() != Role::Splitter
            && (data.supports_action(Action::Click) || data.supports_action(Action::Focus))
        {
            let parent_id = node.parent().map(|p| format!("{:?}", p.id())).unwrap_or_default();
            for (other_parent, other, other_name) in &interactive {
                if *other_parent == parent_id
                    && overlap(&bounds, other)
                    && !contains(&bounds, other)
                    && !contains(other, &bounds)
                {
                    problems.push(format!("{name} overlaps {other_name}"));
                }
            }
            interactive.push((parent_id, bounds, name));
        }
    }
    problems
}

/// FR-UI-027: the buttons of the open dialog (its Primary and Cancel among
/// them) or of the first run's step on show lie inside the window down as well as across ([`fit_problems`] checks
/// across only: a dialog grows down). A button is looked for in its own window only: the dialog's modal
/// that holds its title (the first run's window), so a button of the same name elsewhere never stands in for it. A
/// button not shown there is reported "not shown". Empty without a dialog or a first run.
pub fn footer_problems(harness: &Harness<'_, App>) -> Vec<String> {
    use egui::accesskit::Role;
    use ew_viewmodel::first_run::FirstRunStep;
    const SLACK: f64 = 1.0;
    let shell = harness.state().shell();
    let (title, labels): (String, Vec<String>) = if let Some(dialog) = shell.dialog() {
        let spec = ew_viewmodel::dialogs::spec(dialog, shell);
        let labels = spec.buttons().iter().map(|b| b.label.clone()).collect();
        (spec.title, labels)
    } else if let Some(run) = shell.first_run() {
        // The screen's own lists of its buttons.
        let text = |k| strings::text(k).to_owned();
        let mut labels: Vec<String> = Vec::new();
        if run.step == FirstRunStep::Start {
            labels.extend(ew_viewmodel::first_run::STARTS.iter().map(|(k, _)| text(*k)));
        }
        labels.extend(
            ew_viewmodel::first_run::footer(run.step)
                .into_iter()
                .map(|(k, _)| text(k)),
        );
        (text(key!("firstrun-title")), labels)
    } else {
        return Vec::new();
    };
    let screen = harness.ctx.content_rect() * harness.ctx.pixels_per_point();
    let (x0, y0) = (f64::from(screen.min.x) - SLACK, f64::from(screen.min.y) - SLACK);
    let (x1, y1) = (f64::from(screen.max.x) + SLACK, f64::from(screen.max.y) + SLACK);
    let root = harness.root();
    // The dialog's own window (a modal's `Dialog` node; for the first run, which fills the window, the root):
    // the title is looked for inside a dialog when a dialog is open, and the buttons under that window alone.
    fn window_of<'a>(
        n: &egui_kittest::kittest::AccessKitNode<'a>,
    ) -> Option<egui_kittest::kittest::AccessKitNode<'a>> {
        let mut at = n.parent();
        while let Some(a) = at {
            if matches!(a.role(), Role::Dialog | Role::Window) {
                return Some(a);
            }
            at = a.parent();
        }
        None
    }
    let want = if shell.dialog().is_some() {
        Role::Dialog
    } else {
        Role::Window
    };
    let Some(window) = root
        .children_recursive()
        .filter(|n| {
            let n = n.accesskit_node();
            n.label().or_else(|| n.value()).as_deref() == Some(title.as_str())
        })
        .find_map(|n| window_of(&n.accesskit_node()).filter(|w| w.role() == want))
    else {
        return vec![format!("the title {title:?} is not shown")];
    };
    /// The bounds of the buttons named `label` under `node`.
    fn buttons(
        node: &egui_kittest::kittest::AccessKitNode<'_>,
        label: &str,
        out: &mut Vec<egui::accesskit::Rect>,
    ) {
        for child in node.children() {
            if child.role() == Role::Button
                && child.label().as_deref() == Some(label)
                && let Some(b) = child.bounding_box()
            {
                out.push(b);
            }
            buttons(&child, label, out);
        }
    }
    let mut problems = Vec::new();
    for label in &labels {
        let mut bounds = Vec::new();
        buttons(&window, label, &mut bounds);
        if bounds.is_empty() {
            problems.push(format!("{label:?} is not shown"));
        } else if !bounds
            .iter()
            .any(|b| b.x0 >= x0 && b.x1 <= x1 && b.y0 >= y0 && b.y1 <= y1)
        {
            problems.push(format!("{label:?} at {bounds:?} leaves the window"));
        }
    }
    problems
}

// The app menu replaced the menu bar, the HUD the tool strip.

/// The value of the tool HUD's `Status` node ("Select · Click to select …"); empty without a HUD.
pub fn hud_value(harness: &Harness<'_, App>) -> String {
    harness
        .query_all_by_role_and_label(egui::accesskit::Role::Status, strings::text(key!("hud-name")))
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
        if (role == egui::accesskit::Role::Toolbar && label == strings::text(key!("commandbar-name")))
            || (role == egui::accesskit::Role::Status && label == strings::text(key!("hud-name")))
        {
            return true;
        }
    }
    false
}

/// The node labelled `label` outside the command bar and the HUD: an entry of the open menus (the app menu's
/// submenus share their names with the command tabs, some items with the bar's buttons).
pub fn menu_node<'t>(harness: &'t Harness<'_, App>, label: &'t str) -> egui_kittest::Node<'t> {
    // The rails and the panel headers share names with the submenus ("Project"): an entry inside an
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

/// The combo boxes of the window outside the property grid, whose rows chosen from a list are combo boxes too (a
/// dialog's lists, a pane's), in tree order.
pub fn combos<'t>(harness: &'t Harness<'_, App>) -> Vec<egui_kittest::Node<'t>> {
    let grid = harness
        .query_all_by_label("Properties")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .map(|n| n.rect());
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .filter(|n| {
            let r = n.rect();
            grid.is_none_or(|g| {
                !(r.left() >= g.left() && r.right() <= g.right() + 1.0 && r.top() > g.bottom())
            })
        })
        .collect()
}

/// The `Tab` nodes of the page view and the docks, without the command bar's tabs.
pub fn view_tabs<'t>(harness: &'t Harness<'_, App>) -> Vec<egui_kittest::Node<'t>> {
    harness
        .get_all_by_role(egui::accesskit::Role::Tab)
        .filter(|n| !in_chrome(n))
        // The Project entry's segmented control is no view tab.
        .filter(|n| {
            !ancestors(n)
                .iter()
                .any(|(role, label)| *role == egui::accesskit::Role::TabList && label == "Pages or devices")
        })
        .collect()
}

/// Opens the app menu (its button "Menu") and then each submenu of `path` in turn ("File", "Import").
pub fn open_menu(harness: &mut Harness<'_, App>, path: &[&str]) {
    harness
        .get_by_label(strings::text(key!("commandbar-menu")))
        .click();
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
                && ancestors(n).iter().any(|(role, l)| {
                    *role == egui::accesskit::Role::Toolbar && l == strings::text(key!("commandbar-name"))
                })
        })
        .unwrap_or_else(|| panic!("no command-bar button {label:?}"))
}

/// The cases of `items` that fall to shard `k` of `of`: the ones at an index that is `k` modulo `of`. A test that
/// loops over independent cases runs the loop through this in `of` test functions ([`shard_tests`]), so that nextest
/// runs the shards side by side, and a case added to the list lands in a shard by itself. The shards together hold
/// every case once (`shards_hold_every_case_once`); a list too short to give every shard a case fails, so no shard
/// passes empty.
#[track_caller]
pub fn shard<T>(items: impl IntoIterator<Item = T>, k: usize, of: usize) -> Vec<T> {
    let all: Vec<T> = items.into_iter().collect();
    assert!(k < of, "shard {k} of {of}");
    assert!(all.len() >= of, "{} cases cannot fill {of} shards", all.len());
    all.into_iter()
        .enumerate()
        .filter_map(|(i, x)| (i % of == k).then_some(x))
        .collect()
}

/// Declares one `#[test]` per shard, each running `$run($k, $of)`.
macro_rules! shard_tests {
    ($run:ident, $of:literal: $($name:ident = $k:literal),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                $run($k, $of);
            }
        )+
    };
}
pub(crate) use shard_tests;

#[test]
fn shards_hold_every_case_once() {
    for (cases, of) in [(26, 4), (4, 4), (10, 3), (7, 2)] {
        let mut seen: Vec<usize> = (0..of).flat_map(|k| shard(0..cases, k, of)).collect();
        seen.sort_unstable();
        assert_eq!(
            seen,
            (0..cases).collect::<Vec<_>>(),
            "{cases} cases in {of} shards"
        );
    }
}
