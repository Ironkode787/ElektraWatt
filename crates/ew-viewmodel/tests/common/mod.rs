// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers of the tool-framework tests: the sample project (read-only in place or a copy), a new project,
//! dispatch with checks, points of a page.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};

use ew_viewmodel::canvas::{CanvasModel, pick, rect_centre};
use ew_viewmodel::pointer::HOVER_SLOP_PT;
use ew_viewmodel::script;
use ew_viewmodel::shell::{Dialog, FormDialog, Mode, Outcome, ShellConfig, ShellModel};
use serde_json::{Value, json};

pub use ew_testdir::{SAMPLE, copy_dir};

/// A fresh temporary directory ([`ew_testdir::Tmp`], removed on drop), with the shells of the tests.
pub struct Tmp(pub ew_testdir::Tmp);

/// The shared directory's own methods (`dir()`, `path()`, `sample()`) through `Deref`.
impl std::ops::Deref for Tmp {
    type Target = ew_testdir::Tmp;

    fn deref(&self) -> &ew_testdir::Tmp {
        &self.0
    }
}

impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        Tmp(ew_testdir::Tmp::new(&format!("ew-vm-{tag}")))
    }

    /// The shell's configuration of the tests: its settings in this directory's `config`.
    pub fn config(&self) -> ShellConfig {
        ShellConfig::in_dir(&self.0.join("config"), "0.0.0-test")
    }

    /// A shell in `mode` with no project open, its settings in this directory.
    pub fn empty_shell(&self, mode: Mode) -> ShellModel {
        ShellModel::new(mode, self.config())
    }

    /// A shell in `mode` on a new project in this directory ([`make_project`] at `new.ew`) with `pages`.
    pub fn new_project_in(&self, mode: Mode, pages: &[(&str, &str, &str)]) -> ShellModel {
        let mut shell = self.empty_shell(mode);
        make_project(&mut shell, &self.0.join("new.ew"), pages);
        shell
    }

    /// A headless shell with the sample open on the page it opens on: read-only in place in the source tree, or
    /// (`writable`) a copy in this directory ([`Tmp::sample_copy`]).
    pub fn open_sample(&self, writable: bool) -> ShellModel {
        self.open_sample_in(Mode::Headless, writable)
    }

    /// A shell in `mode` with the sample open, showing `folio`: read-only in place in the source tree, as the window
    /// opens an installed sample, or (`writable`) a copy in this directory ([`Tmp::sample_copy`]).
    pub fn sample_in(&self, mode: Mode, writable: bool, folio: &str) -> ShellModel {
        let mut shell = self.open_sample_in(mode, writable);
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        shell
    }

    fn open_sample_in(&self, mode: Mode, writable: bool) -> ShellModel {
        let dir = if writable {
            let dir = self.sample_copy();
            if !dir.exists() {
                copy_dir(Path::new(SAMPLE), &dir);
            }
            dir
        } else {
            PathBuf::from(SAMPLE)
        };
        let mut shell = self.empty_shell(mode);
        done(
            &mut shell,
            "file.open",
            json!({"dir": dir.to_string_lossy(), "read_only": !writable}),
        );
        shell
    }

    /// The directory of the copy of the sample that a writable [`Tmp::sample_in`] opens (for a test that saves and
    /// reopens it).
    pub fn sample_copy(&self) -> PathBuf {
        self.0.join("sample.ew")
    }

    /// A headless shell with the sample open read-only in place on `folio`.
    pub fn sample_shell(&self, folio: &str) -> ShellModel {
        self.sample_in(Mode::Headless, false, folio)
    }
}

/// Gives the shell a job waker that signals the returned channel; set before a job starts, it lets a test block where
/// the window would be woken.
pub fn job_wakes(shell: &mut ShellModel) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    shell.set_job_waker(Some(Arc::new(move || {
        let _ = tx.lock().map(|tx| tx.send(()));
    })));
    rx
}

/// Polls the jobs until `done` holds of the shell, each poll after a job woke it, as the window takes an end. No
/// clock: a job that never ends is caught by the run's time limit.
pub fn poll_until(shell: &mut ShellModel, wakes: &mpsc::Receiver<()>, done: impl Fn(&ShellModel) -> bool) {
    loop {
        shell.poll_jobs();
        if done(shell) {
            return;
        }
        wakes.recv().expect("a running job wakes the shell");
    }
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
        Outcome::Refused { reason: reason_key } => reason_key.key(),
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

/// A headless shell with a writable copy of the sample on `folio`, at actual size.
pub fn actual_size(tmp: &Tmp, folio: &str) -> ShellModel {
    let mut s = tmp.sample_in(Mode::Headless, true, folio);
    done(&mut s, "view.zoom_actual", json!({}));
    s
}

/// Places a push button in the empty right half of folio 102 (shown), at (200, 80) mm, without autoconnect: -102S3,
/// whose pins 13 and 14 no wire reaches (every connection point of the sample is wired).
pub fn place_free_button(s: &mut ShellModel) {
    done(s, "cursor.set", json!({"x_mm": 200.0, "y_mm": 80.0}));
    done(s, "tool.place", json!({}));
    done(s, "place.pick", json!({"part": "EW-GEN-PB-1", "letter": "S"}));
    done(s, "canvas.activate", json!({"alt": true}));
    done(s, "ui.dismiss", json!({}));
}

/// Imports `rows` (parts-table CSV rows, without the header) into the copy of the sample open in `s`, as `ew import
/// parts` reads a table of the project tables into a project (the window's Import… puts a table's parts into My parts):
/// the project's own parts a test needs that the sample does not hold (a screw-mounted device, an end bracket, a
/// bridge).
pub fn import_parts(s: &mut ShellModel, tmp: &Tmp, rows: &str) {
    let csv = tmp.0.join("parts.csv");
    let header = "key,manufacturer,part_number,kind,type_designation,description,unit,test_data,tech_data,width,\
                  height,depth,mount_kind,enclosure,enclosure_plate,plate,rail_profile,solenoids";
    std::fs::write(&csv, format!("{header}\n{rows}")).unwrap();
    // The copy is the sample's project: its parts are read for the sample.
    let (sample, _) = ew_api::Session::open_read_only(Path::new(SAMPLE), "test").expect("the sample");
    let read = ew_api::import::read_parts_tables(&csv, &sample).expect("the rows read");
    done(s, "command.run", json!({"cmd": "parts.import", "params": read.params()}));
}

/// The bridge BRIDGE-1 as an [`import_parts`] row (the sample holds no bridge part; its end bracket is the starter's
/// EW-GEN-EB-1, which its terminals bring with them).
pub const ACCESSORY_PARTS: &str = "BRIDGE-1,Generic,BRIDGE-1,accessory,,Plug-in bridge,piece,,,,,,,,,,,\n";

/// A headless shell with a writable copy of the sample on folio 103 and a second strip made there, -X1032, of one
/// terminal (1) in the page's empty lower right: the sample draws one strip per page.
#[allow(dead_code)]
pub fn with_a_second_strip_on_103(tmp: &Tmp) -> ShellModel {
    let mut s = tmp.sample_in(Mode::Headless, true, "103");
    done(&mut s, "strip.new", json!({"letter": "X"}));
    done(&mut s, "cursor.set", json!({"x_mm": 250.0, "y_mm": 170.0}));
    done(&mut s, "tool.place", json!({}));
    done(
        &mut s,
        "place.pick",
        json!({"designation": "1", "strip": "-X??", "part": "EW-GEN-TRM-1"}),
    );
    done(&mut s, "canvas.activate", json!({"alt": true}));
    done(&mut s, "ui.dismiss", json!({}));
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
    tmp.new_project_in(Mode::Headless, &[("101", "multi_line", "Power")])
}

/// Makes a new project at `dir` in `shell` from the built-in template, then the pages `pages` in order, each
/// (folio, page type, title): the one way the tests make a project of their own.
pub fn make_project(shell: &mut ShellModel, dir: &Path, pages: &[(&str, &str, &str)]) {
    done(
        shell,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    for (folio, ty, title) in pages {
        done(
            shell,
            "page.new",
            json!({"folio": folio, "type": ty, "title": title}),
        );
    }
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
pub fn entity_points(c: &CanvasModel) -> Vec<(ew_dl::EntityRef, (i64, i64))> {
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

/// A route from a wire's pin end to its other end, given both points.
type PathOf<'a> = dyn Fn((i64, i64), (i64, i64)) -> Vec<(i64, i64)> + 'a;

/// An overlap and its wire through a symbol on a writable copy of the sample shown at folio 101, which the
/// router never draws: three wires given routes by `connection.route` (a command stores the route it is given). The
/// wires of -101F1:1 and -101F1:3 then run on one line, the breaker's top pin row, inside -101F1's rectangle (from
/// -101F1:3 one grid step right, then up to their other ends), and the wire of the power supply's bottom pin
/// -101G1:+#2 runs straight up into -101G1's body before it turns to its other end. Returns the sheet point in
/// the middle of the overlap.
#[allow(dead_code)]
pub fn provisional_overlap_on_101(shell: &mut ShellModel) -> (i64, i64) {
    let (routes, over) = {
        let page = shell.canvas().expect("a page").page();
        let t = &page.tables;
        let cp = |label: &str| {
            t.cps
                .iter()
                .find(|c| c.label == label)
                .unwrap_or_else(|| panic!("{label}"))
                .clone()
        };
        // The wire at `pin` given the route `path` from the pin to its other end, in the wire's own direction.
        let route = |pin: &str, path: &PathOf<'_>| {
            let pin = cp(pin);
            let c = t
                .connections
                .iter()
                .find(|c| pin.connections.contains(&c.id))
                .unwrap_or_else(|| panic!("a wire at {}", pin.label));
            let (first, last) = (c.polyline[0], *c.polyline.last().unwrap());
            if first == pin.at {
                (c.id, path(first, last))
            } else {
                let mut r = path(last, first);
                r.reverse();
                (c.id, r)
            }
        };
        let (pin1, pin3) = (cp("-101F1:1").at, cp("-101F1:3").at);
        let step = pin3.0 - pin1.0;
        let x = page.snapper.page_of_index(page.snapper.grid_index_of(pin3.0) + 1);
        let beside = |pin: (i64, i64), other: (i64, i64)| vec![pin, (x, pin.1), (x, other.1), other];
        // Up from the bottom pin into the body, out to the right one grid step past the body, down and back.
        let psu = cp("-101G1:+#2");
        let body = t.reps.iter().find(|r| r.id == psu.rep).expect("-101G1").body;
        let right = page
            .snapper
            .page_of_index(page.snapper.grid_index_of(body[2]) + 1);
        let up = |pin: (i64, i64), other: (i64, i64)| {
            let y = pin.1 - step;
            vec![pin, (pin.0, y), (right, y), (right, other.1), other]
        };
        let routes = vec![
            route("-101F1:1", &beside),
            route("-101F1:3", &beside),
            route("-101G1:+#2", &up),
        ];
        (routes, page.snapper.page_to_sheet(((pin3.0 + x) / 2, pin1.1)))
    };
    for (id, route) in routes {
        let route: Vec<Value> = route.iter().map(|p| json!([p.0, p.1])).collect();
        done(
            shell,
            "command.run",
            json!({"cmd": "connection.route", "params": {"connection": id.to_string(), "route": route}}),
        );
    }
    over
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

/// Deletes the legend list whose first page is `folio` as a person does, on its row of the Blocks navigator.
pub fn delete_legend_list(shell: &mut ShellModel, folio: &str) {
    use ew_viewmodel::navigators::{NavKind, NavRole};
    done(shell, "navigator.show", json!({"kind": "blocks"}));
    let key = shell
        .nav(NavKind::Blocks)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.nodes
                .iter()
                .find(|n| n.role == NavRole::ReportBlock && n.cells[2] == folio)
                .map(|n| n.key.clone())
        })
        .unwrap_or_else(|| panic!("a legend list at {folio}"));
    done(
        shell,
        "navigator.select",
        json!({"kind": "blocks", "keys": [key]}),
    );
    done(shell, "navigator.delete", json!({"kind": "blocks"}));
    // The delete's card, applied.
    done(shell, "preview.apply", json!({}));
}

/// A legend box as a project made before 0.3.5 holds one (doc 12 §5.4), on a copy of the sample: a new panel page 980
/// at 1:10 shows the field location +FLD's new enclosure (Enclosure…), its legend list is deleted, and Add legend puts
/// the legend, the enclosure and its plate, beside the layout, where it fits whole. The box's ID; page 980 shown.
pub fn legend_box(tmp: &Tmp) -> (ShellModel, String) {
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    done(
        &mut shell,
        "page.new",
        json!({"folio": "980", "type": "panel_layout", "title": "Field"}),
    );
    done(&mut shell, "page.goto", json!({"folio": "980"}));
    done(&mut shell, "select.set", json!({"ids": []}));
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/scale", "value": "1:10"}),
    );
    done(
        &mut shell,
        "layout.enclosure_new",
        json!({"location": "+FLD", "part": "EW-GEN-ENC-1"}),
    );
    delete_legend_list(&mut shell, "980.1");
    done(&mut shell, "page.goto", json!({"folio": "980"}));
    done(&mut shell, "page.add_legend", json!({}));
    let id = shell
        .panel_tables()
        .and_then(|t| t.blocks.first().map(|b| b.id.to_string()))
        .expect("the legend box beside the layout");
    (shell, id)
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

/// The Terminal strips rows on show in display order: (key, the cell of the column labelled `label`). The rows of
/// what the terminals' parts bring (`derived:`) are left out: the tests that read them read them by key.
pub fn strip_column(shell: &ShellModel, label: &str) -> Vec<(String, String)> {
    let n = shell.nav(ew_viewmodel::navigators::NavKind::Strips).unwrap();
    let t = n.table().unwrap();
    let c = t.columns.iter().position(|c| c.label == label).unwrap();
    n.visible()
        .iter()
        .filter(|r| !t.nodes[r.node as usize].key.contains("/derived:"))
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

/// Makes the person's own part `own` from the starter part `starter` in Parts (a starter part is never changed): Make
/// my own, its number, Save, Back.
pub fn own_of(shell: &mut ShellModel, starter: &str, own: &str) {
    own_open(shell, starter, own);
    done(shell, "parts.back", json!({}));
}

/// A new symbol of his own named `name`, drawn in Parts (New symbol in the place, then its Name typed); a drawing in
/// hand before it goes.
pub fn new_drawing(shell: &mut ShellModel, name: &str) {
    if shell.parts_place().is_none() {
        done(shell, "parts.open", json!({}));
    }
    done(shell, "parts.new_symbol", json!({ "discard": true }));
    done(
        shell,
        "parts.field",
        json!({ "field": "symbol.name", "value": name }),
    );
}

/// [`own_of`] without Back: the part saved stays open in Parts.
pub fn own_open(shell: &mut ShellModel, starter: &str, own: &str) {
    done(shell, "parts.open", json!({ "part": starter }));
    done(shell, "parts.own", json!({}));
    done(
        shell,
        "parts.field",
        json!({ "field": "manufacturer", "value": "Example" }),
    );
    done(
        shell,
        "parts.field",
        json!({ "field": "part_number", "value": own }),
    );
    done(shell, "parts.save", json!({}));
}
