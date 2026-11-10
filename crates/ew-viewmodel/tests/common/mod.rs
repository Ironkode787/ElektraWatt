// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the tool-framework tests: temporary copies of Golden A, dispatch with checks,
//! points of a page.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use ew_viewmodel::canvas::{CanvasModel, HOVER_SLOP_PT, pick, rect_centre};
use ew_viewmodel::script;
use ew_viewmodel::shell::{Dialog, FormDialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

pub const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

pub use ew_testdir as fresh_dir;

/// A fresh temporary directory, removed on drop.
pub struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        Tmp(fresh_dir::fresh_dir(&format!("ew-vm-{tag}")))
    }

    /// A shell in `mode` with no project open, its settings in this directory.
    pub fn empty_shell(&self, mode: Mode) -> ShellModel {
        ShellModel::new(mode, ShellConfig::in_dir(&self.0.join("config"), "0.0.0-test"))
    }

    /// A headless shell with a copy of Golden A open (read-only unless `writable`), on the page it opens on.
    pub fn open(&self, writable: bool) -> ShellModel {
        self.open_in(Mode::Headless, writable)
    }

    fn open_in(&self, mode: Mode, writable: bool) -> ShellModel {
        let dir = self.golden_copy();
        if !dir.exists() {
            copy_dir(Path::new(GOLDEN_A), &dir);
        }
        let mut shell = self.empty_shell(mode);
        done(
            &mut shell,
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": !writable}),
        );
        shell
    }

    /// A shell in `mode` with a copy of Golden A open (read-only unless `writable`), showing `folio`.
    pub fn shell_in(&self, mode: Mode, writable: bool, folio: &str) -> ShellModel {
        let mut shell = self.open_in(mode, writable);
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        shell
    }

    /// The directory of the copy of Golden A that [`Tmp::shell_in`] opens (for a test that saves and reopens it).
    pub fn golden_copy(&self) -> PathBuf {
        self.0.join("J-E-2416.ew")
    }

    /// A headless shell with a read-only copy of Golden A on `folio`.
    pub fn shell(&self, folio: &str) -> ShellModel {
        self.shell_in(Mode::Headless, false, folio)
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn copy_dir(from: &Path, to: &Path) {
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

/// Whether `EW_BLESS=1` asks the golden-text tests to rewrite their expected files.
// Allowed: the program boundary of a test, read once; the value never reaches a result.
#[allow(clippy::disallowed_methods)]
pub fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

#[track_caller]
pub fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

#[track_caller]
pub fn refused(shell: &mut ShellModel, action: &str, params: Value) -> &'static str {
    match shell.dispatch(action, &params) {
        Outcome::Refused { reason_key } => reason_key.as_str(),
        other => panic!("{action}: {other:?}"),
    }
}

/// The open form dialog.
#[track_caller]
pub fn form(s: &ShellModel) -> FormDialog {
    match s.dialog() {
        Some(Dialog::Form(f)) => f.clone(),
        other => panic!("no form: {other:?}"),
    }
}

/// `{"x_mm", "y_mm"}` of a sheet point, with more members.
pub fn at(p: (i64, i64), more: Value) -> Value {
    let mut v = json!({"x_mm": p.0 as f64 / 1e6, "y_mm": p.1 as f64 / 1e6});
    if let (Some(o), Value::Object(m)) = (v.as_object_mut(), more) {
        o.extend(m);
    }
    v
}

/// The message of a dispatch that fails.
#[track_caller]
pub fn failed(shell: &mut ShellModel, action: &str, params: Value) -> String {
    match shell.dispatch(action, &params) {
        Outcome::Failed { message } => message,
        other => panic!("{action} {params}: {other:?}"),
    }
}

/// The tables of the page on show.
pub fn tables(s: &ShellModel) -> std::sync::Arc<ew_viewmodel::ew_api::output::PageTables> {
    std::sync::Arc::clone(&s.canvas().unwrap().page().tables)
}

/// A headless shell with a writable copy of Golden A on `folio`, at actual size.
pub fn actual_size(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.shell_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// The sheet point of the page point `p` of the focused page.
pub fn sheet(s: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    s.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The page point of grid index `k` of the focused page.
pub fn grid_point(s: &ShellModel, k: (i64, i64)) -> (i64, i64) {
    let sn = &s.canvas().unwrap().page().snapper;
    (sn.page_of_index(k.0), sn.page_of_index(k.1))
}

/// The grid cursor to page point `p`.
pub fn cursor(s: &mut ShellModel, p: (i64, i64)) {
    let q = sheet(s, p);
    done(s, "cursor.set", at(q, json!({})));
}

/// A click at page point `p` through the grid cursor.
pub fn click_at(s: &mut ShellModel, p: (i64, i64)) {
    cursor(s, p);
    done(s, "canvas.activate", json!({}));
}

/// The `canvas` member of the shell's state.
pub fn canvas(s: &ShellModel) -> Value {
    state(s)["canvas"].clone()
}

/// The first describe line that starts with `prefix`.
pub fn describe_line(s: &ShellModel, prefix: &str) -> Option<String> {
    s.describe()
        .lines()
        .find(|l| l.starts_with(prefix))
        .map(str::to_owned)
}

pub fn select_ids(s: &mut ShellModel, ids: &[String]) {
    done(s, "select.set", json!({ "ids": ids }));
}

/// A dispatch that must fail.
#[track_caller]
pub fn assert_failed(s: &mut ShellModel, action: &str, params: Value) {
    failed(s, action, params);
}

/// Twenty pointer moves from page point `p` change no `Session` read count.
#[track_caller]
pub fn assert_moves_read_nothing(s: &mut ShellModel, p: (i64, i64), what: &str) {
    let calls = s.session_calls();
    for k in 0..20 {
        let q = sheet(s, (p.0 + k * 300_000, p.1 + k * 200_000));
        done(s, "canvas.pointer", at(q, json!({})));
    }
    assert_eq!(s.session_calls(), calls, "{what} reads nothing");
}

/// The toast's text, empty without one.
pub fn toast(s: &ShellModel) -> String {
    state(s)["toast"]["text"].as_str().unwrap_or_default().to_owned()
}

/// A headless shell with a new project (the builtin template) and a new folio 101 shown.
pub fn new_project(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.empty_shell(Mode::Headless);
    done(
        &mut s,
        "file.new",
        json!({"dir": tmp.0.join("new.ew").to_string_lossy(), "template": "builtin"}),
    );
    done(
        &mut s,
        "page.new",
        json!({"folio": "101", "type": "multi_line", "title": "Power"}),
    );
    s
}

pub fn state(shell: &ShellModel) -> Value {
    serde_json::from_str(&script::state_json(shell)).unwrap()
}

/// A click (press and release) at `p` with `mods` (`{"shift": true}` …).
pub fn click(shell: &mut ShellModel, p: (i64, i64), mods: Value) {
    done(shell, "canvas.press", at(p, mods.clone()));
    done(shell, "canvas.release", at(p, mods));
}

/// The centres of the entity rectangles that are no link and whose pick is that entity, in paint order.
pub fn entity_points(c: &CanvasModel) -> Vec<(ew_viewmodel::ew_api::output::ew_dl::EntityRef, (i64, i64))> {
    let slop = HOVER_SLOP_PT / c.points_per_nm();
    let mut out = Vec::new();
    for (e, r) in &c.page().entities {
        let p = rect_centre(*r);
        if c.link_at(p).is_none()
            && pick(&c.page().entities, Some(p), slop).map(|(x, _)| x) == Some(*e)
            && !out.iter().any(|(x, _)| x == e)
        {
            out.push((*e, p));
        }
    }
    out
}

/// A point of the sheet with no entity and no link under it.
pub fn empty_point(c: &CanvasModel) -> (i64, i64) {
    let slop = HOVER_SLOP_PT / c.points_per_nm();
    let (w, h) = c.sheet();
    for y in (5_000_000..h).step_by(3_000_000) {
        for x in (5_000_000..w).step_by(3_000_000) {
            if pick(&c.page().entities, Some((x, y)), slop).is_none() && c.link_at((x, y)).is_none() {
                return (x, y);
            }
        }
    }
    panic!("no empty point on {}", c.page().folio)
}

/// FT-500's overlap and its wire through a symbol on a writable Golden A shown at folio 101, which the routed folio no
/// longer draws (Golden A's route tool): three wires given the routes the provisional path drew them along
/// before, by `connection.route` (a command stores the route it is given). `-101F1:1 -> F55:1` and
/// `S51:3 -> -101F1:N#1` then overlap on one line inside -101F1's rectangle, and `T55:3 -> E55:N` runs straight down
/// through E55's body. Returns the sheet y of the overlap's line.
#[allow(dead_code)]
pub fn provisional_overlap_on_101(shell: &mut ShellModel) -> i64 {
    let (routes, y) = {
        let page = shell.canvas().expect("a page").page();
        let t = &page.tables;
        let cp = |label: &str| {
            t.cps
                .iter()
                .find(|c| c.label == label)
                .unwrap_or_else(|| panic!("{label}"))
                .clone()
        };
        let wire = |a: &str, b: &str| {
            let (a, b) = (cp(a), cp(b));
            let id = t
                .connections
                .iter()
                .find(|c| a.connections.contains(&c.id) && b.connections.contains(&c.id))
                .unwrap_or_else(|| panic!("a wire from {} to {}", a.label, b.label))
                .id;
            (id, a.at, b.at)
        };
        let f1 = wire("-101F1:1", "F55:1");
        let s51 = wire("S51:3", "-101F1:N#1");
        let t55 = wire("T55:3", "E55:N");
        let routes = vec![
            (f1.0, vec![f1.1, f1.2]),
            (s51.0, vec![s51.1, (s51.1.0, s51.2.1), s51.2]),
            (t55.0, vec![t55.1, t55.2]),
        ];
        (routes, page.snapper.page_to_sheet(f1.1).1)
    };
    for (id, route) in routes {
        let route: Vec<Value> = route.iter().map(|p| json!([p.0, p.1])).collect();
        done(
            shell,
            "command.run",
            json!({"cmd": "connection.route", "params": {"connection": id.to_string(), "route": route}}),
        );
    }
    y
}

/// The key of the row labelled `label` with role `role` in an open navigator.
pub fn row_key(
    shell: &ShellModel,
    kind: ew_viewmodel::navigators::NavKind,
    role: ew_viewmodel::navigators::NavRole,
    label: &str,
) -> String {
    let t = shell
        .nav(kind)
        .and_then(|n| n.table())
        .expect("an open navigator with its table");
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

/// The lines of the describe text that start with `prefix`.
pub fn lines_with(shell: &ShellModel, prefix: &str) -> Vec<String> {
    shell
        .describe()
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(str::to_owned)
        .collect()
}

/// The row lines under the header of the navigator titled `title`.
pub fn nav_rows(shell: &ShellModel, title: &str) -> Vec<String> {
    let d = shell.describe();
    let mut out = Vec::new();
    let mut inside = false;
    for l in d.lines() {
        if l.starts_with(&format!("navigator \"{title}\"")) {
            inside = true;
            continue;
        }
        if inside && !l.starts_with("  row ") {
            break;
        }
        if inside {
            out.push(l.to_owned());
        }
    }
    out
}

/// The ID (as the commands take it) of the page with folio `folio`.
pub fn page_id(shell: &ShellModel, folio: &str) -> String {
    shell
        .page_rows()
        .iter()
        .find(|r| r.folio == folio)
        .map(|r| r.id.to_string())
        .unwrap_or_else(|| panic!("no page {folio}"))
}

/// The library tiers of a fixture under `root`: no system path (the compiled-in libraries), a user directory.
pub fn library_tiers(root: &Path) -> ew_viewmodel::ew_api::library::LibraryTiers {
    ew_viewmodel::ew_api::library::LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: Vec::new(),
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// A fresh user library "Mine" under `root`: the tiers and its ID.
pub fn user_library(
    root: &Path,
) -> (
    ew_viewmodel::ew_api::library::LibraryTiers,
    ew_viewmodel::ew_api::output::Uuid,
) {
    let t = library_tiers(root);
    let mut reg = ew_viewmodel::ew_api::library::Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    (t, lib)
}

/// The column key of the Terminal strips column labelled `label`.
pub fn strip_col(shell: &ShellModel, label: &str) -> String {
    let t = shell
        .nav(ew_viewmodel::navigators::NavKind::Strips)
        .and_then(|n| n.table())
        .expect("a table");
    t.columns
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("no column {label}"))
        .key
        .clone()
}

/// The Terminal strips rows on show in display order: (key, the cell of the column labelled `label`).
pub fn strip_column(shell: &ShellModel, label: &str) -> Vec<(String, String)> {
    let n = shell.nav(ew_viewmodel::navigators::NavKind::Strips).unwrap();
    let t = n.table().unwrap();
    let c = t.columns.iter().position(|c| c.label == label).unwrap();
    n.visible()
        .iter()
        .map(|r| {
            let node = &t.nodes[r.node as usize];
            (node.key.clone(), node.cells[c].clone())
        })
        .collect()
}

/// The texts of the Terminal strips column labelled `label` on show.
pub fn strip_texts(shell: &ShellModel, label: &str) -> Vec<String> {
    strip_column(shell, label).into_iter().map(|(_, t)| t).collect()
}

/// The key of the Terminal strips row on show whose cell in column `label` is `text`.
pub fn strip_row(shell: &ShellModel, label: &str, text: &str) -> String {
    strip_column(shell, label)
        .into_iter()
        .find(|(_, t)| t == text)
        .unwrap_or_else(|| panic!("no row with {label} {text}"))
        .0
}

/// The Terminal strips cursor row's key.
pub fn strip_cursor(shell: &ShellModel) -> String {
    shell
        .nav(ew_viewmodel::navigators::NavKind::Strips)
        .and_then(|n| n.cursor_key())
        .expect("a cursor row")
}

/// Puts the Terminal strips cursor on the row `key`.
pub fn strip_at(shell: &mut ShellModel, key: &str) {
    done(shell, "table.cell", json!({"kind": "strips", "row": key}));
}
