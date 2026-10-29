// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 3, acceptance check DK9: the data grid of doc 16a §6.21 on Golden A's Terminal strips navigator in
//! table mode. Every key and pointer gesture of §6.21 dispatches the actions it dispatched before the restyle (the log
//! `tests/expected/table_keys.txt`, recorded from the as-built grid before the restyle; `EW_BLESS=1` rewrites it).
//! The accessible-name walker runs after every step (`settle`).

mod support;

use std::fmt::Write as _;

use egui::{Key, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigators::NavKind;
use ew_ui::ew_viewmodel::shell::{Mode, Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness_with, settle, wait_for_page};

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The window on a writable copy of Golden A, folio 102 shown, the Terminal strips navigator in table mode filtered
/// `x1031` and sorted by Terminal, with the focus; Properties hidden so that the grid has the whole column.
fn strips(tmp: &Tmp) -> Harness<'static, App> {
    strips_in(tmp, None)
}

/// [`strips`] in `density` (the setting's default when `None`).
fn strips_in(tmp: &Tmp, density: Option<&str>) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    if let Some(d) = density {
        done(&mut shell, "settings.set", json!({"key": "density", "value": d}));
    }
    let mut h: Harness<'static, App> = harness_with(shell, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1000.0, 900.0));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "props.show", json!({"on": false}));
    done(shell, "navigator.show", json!({"kind": "strips"}));
    done(shell, "navigator.table", json!({"kind": "strips"}));
    // WP-S2.07's Pos column is hidden: these checks are WP-S2.06's grid in its as-built layout.
    done(shell, "table.columns", json!({"kind": "strips", "hide": ["pos"]}));
    done(
        shell,
        "navigator.filter",
        json!({"kind": "strips", "text": "x1031"}),
    );
    done(
        shell,
        "navigator.sort",
        json!({"kind": "strips", "by": "terminal"}),
    );
    done(shell, "ui.focus", json!({"area": "navigator", "kind": "strips"}));
    settle(&mut h);
    h
}

// Program boundary of the test binary: the bless switch of the repository's tests (`EW_BLESS=1`).
#[expect(clippy::disallowed_methods, reason = "the test binary's bless switch")]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// One input of the dispatch log.
enum Input {
    Key(Modifiers, Key),
    Text(&'static str),
    /// A click on the node named so (see [`node`]), with modifiers.
    Click(&'static str, Modifiers),
    /// A double click on the node named so (four button events in one frame).
    DoubleClick(&'static str),
    /// The shell takes the focus back to the grid (not a gesture of the grid; nothing is logged for it).
    Refocus,
    /// The shell moves the cell cursor to the column labelled so, which scrolls it into view (not a gesture of the
    /// grid: the restyled columns start at `size.table_column`, so the target of the next gesture is scrolled in
    /// first; neither the step nor its label is logged).
    ShowColumn(&'static str),
}

/// The centre of the node named `target`; `<column>@<row>` is the cell of that column in that row, whatever its text.
fn node<'t>(h: &'t Harness<'static, App>, target: &'t str) -> egui_kittest::Node<'t> {
    match target.split_once('@') {
        Some((column, row)) => {
            let row = format!(", row {row} of ");
            let column = format!("{column} ");
            h.get_all_by_role(egui::Role::Cell)
                .find(|n| {
                    n.accesskit_node()
                        .label()
                        .is_some_and(|l| l.starts_with(&column) && l.contains(&row))
                })
                .unwrap_or_else(|| panic!("the cell {target}"))
        }
        None => h.get_by_label(target),
    }
}

/// The key of the Terminal strips column labelled `label`.
fn column_key(h: &Harness<'static, App>, label: &str) -> String {
    let n = h.state().shell().nav(NavKind::Strips).unwrap();
    let t = n.table().unwrap();
    t.columns
        .iter()
        .find(|c| c.label == label)
        .map(|c| c.key.clone())
        .unwrap_or_else(|| panic!("the column {label}"))
}

/// Moves the cell cursor to the column labelled `label` through the shell (nothing is logged), which scrolls it
/// into view.
fn show_column(h: &mut Harness<'static, App>, label: &str) {
    let key = column_key(h, label);
    done(
        h.state_mut().shell_mut(),
        "table.cell",
        json!({"kind": "strips", "column": key}),
    );
    settle(h);
}

/// The four pointer events of a double click at `at`, all in one frame (no clock decides it).
fn double_click(h: &mut Harness<'static, App>, at: egui::Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_ok();
    for pressed in [true, false, true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    settle(h);
}

/// The gestures of doc 16a §6.21 in order, each with what the as-built grid dispatched for it.
fn gestures() -> Vec<(&'static str, Input)> {
    use Input::*;
    vec![
        ("Ctrl+Home", Key(Modifiers::COMMAND, egui::Key::Home)),
        ("Down", Key(Modifiers::NONE, egui::Key::ArrowDown)),
        ("Right", Key(Modifiers::NONE, egui::Key::ArrowRight)),
        ("Tab", Key(Modifiers::NONE, egui::Key::Tab)),
        ("Shift+Tab", Key(Modifiers::SHIFT, egui::Key::Tab)),
        ("Left", Key(Modifiers::NONE, egui::Key::ArrowLeft)),
        ("Up", Key(Modifiers::NONE, egui::Key::ArrowUp)),
        ("End", Key(Modifiers::NONE, egui::Key::End)),
        ("Home", Key(Modifiers::NONE, egui::Key::Home)),
        ("Ctrl+End", Key(Modifiers::COMMAND, egui::Key::End)),
        ("Ctrl+Home", Key(Modifiers::COMMAND, egui::Key::Home)),
        ("Shift+Down", Key(Modifiers::SHIFT, egui::Key::ArrowDown)),
        ("Shift+Down", Key(Modifiers::SHIFT, egui::Key::ArrowDown)),
        ("Shift+Up", Key(Modifiers::SHIFT, egui::Key::ArrowUp)),
        ("Ctrl+Enter (no card)", Key(Modifiers::COMMAND, egui::Key::Enter)),
        ("Right", Key(Modifiers::NONE, egui::Key::ArrowRight)),
        ("Right", Key(Modifiers::NONE, egui::Key::ArrowRight)),
        ("type Q", Text("Q")),
        ("editor: type x", Text("x")),
        ("editor: Escape", Key(Modifiers::NONE, egui::Key::Escape)),
        ("type Z", Text("Z")),
        ("editor: Enter", Key(Modifiers::NONE, egui::Key::Enter)),
        ("Shift+Down", Key(Modifiers::SHIFT, egui::Key::ArrowDown)),
        ("type W", Text("W")),
        ("editor: Ctrl+Enter", Key(Modifiers::COMMAND, egui::Key::Enter)),
        ("", ShowColumn("Terminal")),
        ("click", Click("Terminal@3", Modifiers::NONE)),
        ("Ctrl+click", Click("Terminal@5", Modifiers::COMMAND)),
        ("Shift+click", Click("Terminal@7", Modifiers::SHIFT)),
        ("", ShowColumn("Function text")),
        ("double click", DoubleClick("Function text@2")),
        ("editor: Escape", Key(Modifiers::NONE, egui::Key::Escape)),
        ("", ShowColumn("Terminal")),
        (
            "header click",
            Click("Sort by Terminal, sorted ascending", Modifiers::NONE),
        ),
        (
            "header click again",
            Click("Sort by Terminal, sorted descending", Modifiers::NONE),
        ),
        ("refocus", Refocus),
        ("Enter", Key(Modifiers::NONE, egui::Key::Enter)),
    ]
}

/// DK9: every key of doc 16a §6.21 (arrows, Tab, Shift+Tab, Home, End, Ctrl+Home, Ctrl+End, Shift+Up, Shift+Down,
/// Enter, Ctrl+Enter, a printable character; in the editor Enter, Ctrl+Enter and Escape) and the pointer gestures
/// (click, Ctrl+click, Shift+click, double click, header clicks) dispatch the same actions with the same parameters as
/// before the restyle: the log equals the expected log recorded from the as-built grid.
#[test]
fn dk9_every_key_dispatches_the_as_built_actions() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let mut log = String::new();
    for (label, input) in gestures() {
        let before = h.state().dispatched_calls().len();
        assert!(before < 200, "the dispatch log keeps its last 256 entries");
        match input {
            Input::Key(m, k) => {
                h.key_press_modifiers(m, k);
                settle(&mut h);
            }
            Input::Text(t) => {
                h.event(egui::Event::Text(t.to_owned()));
                settle(&mut h);
            }
            Input::Click(target, m) => {
                node(&h, target).click_modifiers(m);
                settle(&mut h);
            }
            Input::DoubleClick(target) => {
                let at = node(&h, target).rect().center();
                double_click(&mut h, at);
            }
            Input::ShowColumn(label) => {
                show_column(&mut h, label);
                continue;
            }
            Input::Refocus => {
                let shell = h.state_mut().shell_mut();
                done(shell, "ui.focus", json!({"area": "navigator", "kind": "strips"}));
                settle(&mut h);
            }
        }
        let calls = h.state().dispatched_calls();
        writeln!(log, "{label}").unwrap();
        for (action, params) in &calls[before..] {
            writeln!(log, "  {action} {params}").unwrap();
        }
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/table_keys.txt");
    if bless() {
        std::fs::write(&path, &log).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("{} is blessed (EW_BLESS=1)", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(log, expected, "the grid's dispatch log");
    h.state_mut().shell_mut().shutdown();
}

// ---------------------------------------------------------------------------------------------------------------
// The painted shapes of the restyled grid

/// Every shape the last frame painted, nested ones flattened.
fn painted(h: &Harness<'_, App>) -> Vec<egui::Shape> {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            s => out.push(s.clone()),
        }
    }
    let mut out = Vec::new();
    for c in &h.output().shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// The filled rects of `colour` painted.
fn fills(shapes: &[egui::Shape], colour: egui::Color32) -> Vec<egui::Rect> {
    shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r) if r.fill == colour => Some(r.rect),
            _ => None,
        })
        .collect()
}

/// The outlined rects painted: (rect, stroke, kind).
fn outlines(shapes: &[egui::Shape]) -> Vec<(egui::Rect, egui::Stroke, egui::StrokeKind)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r) if r.stroke.width > 0.0 => Some((r.rect, r.stroke, r.stroke_kind)),
            _ => None,
        })
        .collect()
}

/// The vertical line segments painted: (x, y range, stroke).
fn vlines(shapes: &[egui::Shape]) -> Vec<(f32, egui::Rangef, egui::Stroke)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::LineSegment {
                points: [a, b],
                stroke,
            } if (a.x - b.x).abs() < 0.01 => {
                Some((a.x, egui::Rangef::new(a.y.min(b.y), a.y.max(b.y)), *stroke))
            }
            _ => None,
        })
        .collect()
}

/// Whether icon `id` was painted in `colour` at `size` with `stroke` somewhere inside `region`: its paths, as
/// [`ew_ui::icons::shapes`] makes them, appear among the painted paths up to a translation.
fn icon_in(
    shapes: &[egui::Shape],
    region: egui::Rect,
    id: ew_ui::icons::IconId,
    colour: egui::Color32,
    size: f32,
    stroke: f32,
) -> bool {
    let paths = |shapes: &[egui::Shape]| -> Vec<Vec<egui::Pos2>> {
        shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Path(p) if p.stroke.color == egui::epaint::ColorMode::Solid(colour) => {
                    Some(p.points.clone())
                }
                _ => None,
            })
            .collect()
    };
    let canon = ew_ui::icons::shapes(
        id,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(size)),
        colour,
        stroke,
    );
    let canon = paths(&canon);
    let found: Vec<Vec<egui::Pos2>> = paths(shapes)
        .into_iter()
        .filter(|p| p.iter().all(|q| region.contains(*q)))
        .collect();
    let same = |a: &[egui::Pos2], b: &[egui::Pos2], d: egui::Vec2| {
        a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (*p + d - *q).length() < 0.01)
    };
    found.iter().any(|anchor| {
        anchor.len() == canon[0].len() && {
            let d = anchor[0] - canon[0][0];
            canon.iter().all(|c| found.iter().any(|f| same(c, f, d)))
        }
    })
}

/// The rect of the cell of `column` in row `row` (1-based).
fn cell(h: &Harness<'static, App>, column: &str, row: usize) -> egui::Rect {
    let row = format!(", row {row} of ");
    let column = format!("{column} ");
    h.get_all_by_role(egui::Role::Cell)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with(&column) && l.contains(&row))
        })
        .unwrap_or_else(|| {
            let cells: Vec<String> = h
                .get_all_by_role(egui::Role::Cell)
                .filter_map(|n| n.accesskit_node().label())
                .collect();
            panic!("the cell {column}{row} among {cells:?}")
        })
        .rect()
}

/// Whether `a` and `b` are the same rect within half a point.
fn same_rect(a: egui::Rect, b: egui::Rect) -> bool {
    (a.min - b.min).length() < 0.5 && (a.max - b.max).length() < 0.5
}

/// DK9: the grid is one `Table` node named "Terminal strips table"; its header cells are buttons "Sort by <column>"
/// of `size.table_header` on the `color.surface.sunken` band; the sorted one (Terminal, ascending) is named ", sorted
/// ascending" (FT-523) and paints `arrow-narrow-up` in `color.text.primary` after its label, the others no sort icon;
/// after a second click it paints `arrow-narrow-down`. A derived column's header paints `math-function` in
/// `color.text.tertiary`.
#[test]
fn dk9_one_table_node_with_sort_buttons_and_the_sort_icon() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    let tables: Vec<_> = h.get_all_by_role(egui::Role::Table).collect();
    assert_eq!(tables.len(), 1, "one Table node");
    assert_eq!(
        tables[0].accesskit_node().label().as_deref(),
        Some("Terminal strips table")
    );
    let shapes = painted(&h);
    let sorted = h.get_by_label("Sort by Terminal, sorted ascending").rect();
    assert!(
        (sorted.height() - t.size_table_header).abs() < 0.5,
        "the header is size.table_header high: {sorted:?}"
    );
    assert!(
        fills(&shapes, t.surface_sunken.color32()).iter().any(|r| {
            (r.top() - sorted.top()).abs() < 0.5
                && (r.bottom() - sorted.bottom()).abs() < 0.5
                && r.left() <= sorted.left()
        }),
        "the header band in color.surface.sunken behind the header"
    );
    let primary = t.text_primary.color32();
    let up = |shapes: &[egui::Shape], r| {
        icon_in(
            shapes,
            r,
            ew_ui::icons::IconId::arrow_narrow_up,
            primary,
            t.icon_sm,
            t.icon_stroke,
        )
    };
    let down = |shapes: &[egui::Shape], r| {
        icon_in(
            shapes,
            r,
            ew_ui::icons::IconId::arrow_narrow_down,
            primary,
            t.icon_sm,
            t.icon_stroke,
        )
    };
    assert!(up(&shapes, sorted), "the sorted header paints arrow-narrow-up");
    assert!(!down(&shapes, sorted));
    let strip = h.get_by_label("Sort by Strip").rect();
    assert!(
        !up(&shapes, strip) && !down(&shapes, strip),
        "no sort icon on an unsorted header"
    );
    h.get_by_label("Sort by Terminal, sorted ascending").click();
    settle(&mut h);
    let shapes = painted(&h);
    let sorted = h.get_by_label("Sort by Terminal, sorted descending").rect();
    assert!(down(&shapes, sorted), "descending paints arrow-narrow-down");
    assert!(!up(&shapes, sorted));
    // A derived column on show paints `math-function` in tertiary.
    let derived: Vec<String> = {
        let n = h.state().shell().nav(NavKind::Strips).unwrap();
        let table = n.table().unwrap();
        n.table_mode()
            .unwrap()
            .shown_columns(table)
            .into_iter()
            .filter(|c| table.columns[*c].derived)
            .map(|c| table.columns[c].label.clone())
            .collect()
    };
    let mut checked = 0;
    for label in &derived {
        show_column(&mut h, label);
        let shapes = painted(&h);
        let name = format!("Sort by {label}");
        let Some(node) = h.query_by_label(&name) else {
            continue; // right of the pane
        };
        assert!(
            icon_in(
                &shapes,
                node.rect(),
                ew_ui::icons::IconId::math_function,
                t.text_tertiary.color32(),
                t.icon_sm,
                t.icon_stroke
            ),
            "{label}: the math-function icon"
        );
        checked += 1;
    }
    assert!(checked > 0, "a derived column is on show: {derived:?}");
    h.state_mut().shell_mut().shutdown();
}

/// DK9: the key column (Strip) keeps its place after End scrolls the grid right, and both before and after it paints
/// its 1-px `color.table.frozen_edge` line at its right edge from the header to the last row on show.
#[test]
fn dk9_the_key_column_stays_and_paints_the_frozen_edge() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    let edge = |h: &Harness<'static, App>| {
        let key = cell(h, "Strip", 1);
        let header = h.get_by_label("Sort by Strip").rect();
        let shapes = painted(h);
        let lines = vlines(&shapes);
        let x = key.right() - t.border_width / 2.0;
        let on = |r: egui::Rect| {
            lines.iter().any(|(lx, y, s)| {
                (lx - x).abs() < 0.01
                    && s.width == t.border_width
                    && s.color == t.table_frozen_edge.color32()
                    && y.min <= r.top() + 0.5
                    && y.max >= r.bottom() - 0.5
            })
        };
        assert!(on(header), "the header's key cell paints the edge");
        assert!(on(key), "the key cell paints the edge");
        key.left()
    };
    let left = edge(&h);
    h.key_press(Key::End);
    settle(&mut h);
    assert!(
        h.query_by_label("Terminal 1, row 1 of 20").is_none(),
        "the grid scrolled right"
    );
    assert_eq!(edge(&h), left, "the key column stays at the left");
    h.state_mut().shell_mut().shutdown();
}

/// DK9: two marked rows paint `color.accent.subtle` across the row; the cursor row paints the 2-px accent bar at the
/// key cell's left, just inside the grid's focus ring, inset `space.2` top and bottom; the cursor cell paints a 2-px `color.table.cursor` ring inside
/// it while the grid has the keyboard and a 1-px `color.table.cursor_idle` ring once the focus is on the canvas.
#[test]
fn dk9_marked_rows_the_cursor_row_bar_and_the_cursor_ring() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Home);
    settle(&mut h);
    h.key_press(Key::ArrowRight);
    settle(&mut h);
    h.key_press_modifiers(Modifiers::SHIFT, Key::ArrowDown);
    settle(&mut h);
    assert_eq!(h.state().shell().nav(NavKind::Strips).unwrap().marked().len(), 2);
    let shapes = painted(&h);
    let marked = fills(&shapes, t.accent_subtle.color32());
    for row in [1, 2] {
        for c in [cell(&h, "Strip", row), cell(&h, "Terminal", row)] {
            assert!(
                marked.iter().any(|r| r.contains_rect(c.shrink(0.5))),
                "row {row}: the marked fill under {c:?}"
            );
        }
    }
    let unmarked = cell(&h, "Terminal", 3);
    assert!(
        !marked.iter().any(|r| r.intersects(unmarked.shrink(1.0))),
        "row 3 is not marked"
    );
    // The cursor is in row 2, column Terminal. The bar sits inside the grid's focus ring (inset 1 px, `focus_width`
    // wide): it starts at the ring's inner edge, in one place whether or not the grid has the keyboard (the cycle-33
    // ruling).
    let key = cell(&h, "Strip", 2);
    let grid = h.get_by_label("Terminal strips table").rect();
    let inner = grid.left() + 1.0 + t.focus_width;
    let bar = egui::Rect::from_min_max(
        egui::pos2(inner, key.top() + t.space_2),
        egui::pos2(inner + t.focus_width, key.bottom() - t.space_2),
    );
    let accent = fills(&shapes, t.accent.color32());
    assert!(
        accent.iter().any(|r| same_rect(*r, bar)),
        "the cursor row's 2-px bar {bar:?}"
    );
    let first = cell(&h, "Strip", 1);
    assert!(
        !accent.iter().any(|r| r.intersects(first.shrink(1.0))),
        "no bar on another row"
    );
    let cursor = cell(&h, "Terminal", 2);
    let ring = |shapes: &[egui::Shape], width: f32, colour: egui::Color32| {
        outlines(shapes).iter().any(|(r, s, k)| {
            same_rect(*r, cursor) && s.width == width && s.color == colour && *k == egui::StrokeKind::Inside
        })
    };
    assert!(
        ring(&shapes, t.focus_width, t.table_cursor.color32()),
        "the 2-px ring while the grid has the keyboard"
    );
    done(
        h.state_mut().shell_mut(),
        "ui.focus",
        json!({"area": "canvas", "group": 0}),
    );
    settle(&mut h);
    let shapes = painted(&h);
    assert!(
        ring(&shapes, t.border_width, t.table_cursor_idle.color32()),
        "the 1-px ring when the focus is elsewhere"
    );
    assert!(!ring(&shapes, t.focus_width, t.table_cursor.color32()));
    assert!(
        fills(&shapes, t.accent.color32())
            .iter()
            .any(|r| same_rect(*r, bar)),
        "the bar stays where it was when the focus leaves the grid"
    );
    h.state_mut().shell_mut().shutdown();
}

/// DK9: a double click on a writable cell opens the editor in the cell with the accent border: the panel fill inset
/// `space.1`, a 1-px `color.accent` border inside and a 1-px ring outside, `radius.sm`; the editor has the keyboard.
#[test]
fn dk9_a_double_click_opens_the_editing_cell_with_the_accent_border() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    show_column(&mut h, "Function text");
    let target = cell(&h, "Function text", 2);
    double_click(&mut h, target.center());
    assert!(
        h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .table_mode()
            .unwrap()
            .edit()
            .is_some(),
        "the double click opened the editor"
    );
    let editor = h
        .get_all_by_role(egui::Role::TextInput)
        .find(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.starts_with("Function text") && l.contains(", row 2 of "))
        })
        .expect("the editor node");
    assert!(editor.is_focused(), "the editor has the keyboard");
    let shapes = painted(&h);
    let frame = target.shrink(t.space_1);
    let accent = t.accent.color32();
    let strokes: Vec<_> = outlines(&shapes)
        .into_iter()
        .filter(|(r, s, _)| same_rect(*r, frame) && s.color == accent && s.width == t.border_width)
        .map(|(_, _, k)| k)
        .collect();
    assert!(
        strokes.contains(&egui::StrokeKind::Inside),
        "the 1-px accent border"
    );
    assert!(
        strokes.contains(&egui::StrokeKind::Outside),
        "the 1-px accent ring"
    );
    assert!(
        shapes.iter().any(|s| matches!(s, egui::Shape::Rect(r)
            if same_rect(r.rect, frame)
                && r.fill == t.surface_panel.color32()
                && r.corner_radius == ew_ui::theme::radius(t.radius_sm))),
        "the panel fill with radius.sm"
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// DK9: rows are `size.row` high and the header `size.table_header`, in Comfortable and Compact.
#[test]
fn dk9_row_and_header_heights_in_both_densities() {
    for density in ["comfortable", "compact"] {
        let tmp = Tmp::new();
        let mut h = strips_in(&tmp, Some(density));
        let t = ew_ui::theme::current(&h.ctx);
        let expected_row = if density == "compact" { 24.0 } else { 28.0 };
        assert_eq!(t.size_row, expected_row, "{density}: the theme on show");
        for row in [1, 2, 3] {
            let c = cell(&h, "Terminal", row);
            assert!(
                (c.height() - t.size_row).abs() < 0.5,
                "{density}: row {row} is size.row high: {c:?}"
            );
        }
        let header = h.get_by_label("Sort by Terminal, sorted ascending").rect();
        assert!(
            (header.height() - t.size_table_header).abs() < 0.5,
            "{density}: the header {header:?}"
        );
        h.state_mut().shell_mut().shutdown();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// FT-532 and FT-536: the empty state, the grid's focus ring and the Columns menu's move icons

/// Whether a `Group` node is named `name` (an empty state, doc 16a §6.17).
fn group_named(h: &Harness<'static, App>, name: &str) -> bool {
    h.get_all_by_role(egui::Role::Group)
        .any(|n| n.accesskit_node().label().as_deref() == Some(name))
}

/// FT-532: a navigator in table mode without rows shows the empty state of the tree mode (CP5) in place of the grid:
/// with a filter without hits "Nothing matches "…"" and Clear filter, which clears it and brings the grid back; on a
/// new project the Terminal strips table says "No terminal strips yet".
#[test]
fn ft532_table_mode_without_rows_shows_the_empty_state() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    assert!(
        h.query_by_label("Sort by Terminal, sorted ascending").is_some(),
        "the grid on show"
    );
    done(
        h.state_mut().shell_mut(),
        "navigator.filter",
        json!({"kind": "strips", "text": "ft532nomatch"}),
    );
    settle(&mut h);
    settle(&mut h);
    assert!(
        h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .visible()
            .is_empty(),
        "no row matches"
    );
    assert!(
        group_named(&h, "Nothing matches \"ft532nomatch\""),
        "the filter's empty state"
    );
    assert!(
        h.query_by_label("Sort by Terminal, sorted ascending").is_none(),
        "no bare header"
    );
    h.get_all_by_role(egui::Role::Button)
        .find(|n| n.accesskit_node().label().as_deref() == Some("Clear filter"))
        .expect("Clear filter")
        .click();
    settle(&mut h);
    settle(&mut h);
    assert!(
        !h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .visible()
            .is_empty(),
        "the filter is cleared"
    );
    assert!(
        h.query_by_label("Sort by Terminal, sorted ascending").is_some(),
        "the grid is back"
    );
    support::assert_named(&h);
    h.state_mut().shell_mut().shutdown();

    // A new project: no terminal strip at all.
    let tmp = Tmp::new();
    let dir = tmp.0.join("blank.ew");
    let mut shell = ShellModel::new(Mode::Gui, tmp.config());
    done(
        &mut shell,
        "file.new",
        json!({"dir": dir.to_string_lossy(), "template": "builtin"}),
    );
    let mut h: Harness<'static, App> = harness_with(shell, Scripted::default(), None);
    h.set_size(egui::Vec2::new(1000.0, 900.0));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "navigator.show", json!({"kind": "strips"}));
    done(shell, "navigator.table", json!({"kind": "strips"}));
    settle(&mut h);
    settle(&mut h);
    assert!(
        h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .table_mode()
            .is_some(),
        "in table mode"
    );
    assert!(
        group_named(&h, "No terminal strips yet"),
        "the navigator's empty state"
    );
    assert!(
        h.query_by_label("Terminal strips table").is_some(),
        "the grid node stays"
    );
    h.state_mut().shell_mut().shutdown();
}

/// FT-532 (doc 16a §6.21): while the grid has the keyboard the focus ring of §6.1 (2-px `color.focus`, `radius.sm`)
/// is drawn around the whole grid, inset 1 px into the pane; with the focus on the canvas it is gone.
#[test]
fn ft532_the_focus_ring_around_the_whole_grid() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    let grid = h.get_by_label("Terminal strips table").rect();
    let expected = grid.shrink(1.0);
    let ring = |shapes: &[egui::Shape]| {
        shapes.iter().any(|s| {
            matches!(s, egui::Shape::Rect(r)
                if same_rect(r.rect, expected)
                    && r.stroke.width == t.focus_width
                    && r.stroke.color == t.focus.color32()
                    && r.stroke_kind == egui::StrokeKind::Inside
                    && r.corner_radius == ew_ui::theme::radius(t.radius_sm))
        })
    };
    assert!(ring(&painted(&h)), "the focus ring around {grid:?}");
    done(
        h.state_mut().shell_mut(),
        "ui.focus",
        json!({"area": "canvas", "group": 0}),
    );
    settle(&mut h);
    assert!(!ring(&painted(&h)), "no ring with the focus elsewhere");
    h.state_mut().shell_mut().shutdown();
}

/// FT-536 (doc 16a §6.21): the Columns menu's move items are 24-px icon buttons painting `arrow-left` and
/// `arrow-right`, named "Move <column> left" and "… right"; the first movable column's left and the last one's right
/// are disabled (the icon in `color.text.disabled`); no text button "Move left" or "Move right" is left.
#[test]
fn ft536_the_move_items_are_arrow_icon_buttons() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    let t = ew_ui::theme::current(&h.ctx);
    h.get_by_label("Columns").click();
    settle(&mut h);
    let labels: Vec<String> = h
        .get_all_by_role(egui::Role::Button)
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert!(
        !labels.iter().any(|l| l == "Move left" || l == "Move right"),
        "{labels:?}"
    );
    let shapes = painted(&h);
    for (name, icon, enabled) in [
        ("Move Function text left", ew_ui::icons::IconId::arrow_left, true),
        (
            "Move Function text right",
            ew_ui::icons::IconId::arrow_right,
            true,
        ),
        ("Move Terminal left", ew_ui::icons::IconId::arrow_left, false),
        ("Move State right", ew_ui::icons::IconId::arrow_right, false),
    ] {
        let node = h.get_by_label(name);
        assert_eq!(node.accesskit_node().is_disabled(), !enabled, "{name}");
        let rect = node.rect();
        assert!(
            (rect.width() - t.size_control_inline).abs() < 0.5
                && (rect.height() - t.size_control_inline).abs() < 0.5,
            "{name}: 24 px {rect:?}"
        );
        let colour = if enabled {
            t.text_secondary
        } else {
            t.text_disabled
        };
        assert!(
            icon_in(&shapes, rect, icon, colour.color32(), t.icon_sm, t.icon_stroke),
            "{name}: the {icon:?} icon"
        );
    }
    h.get_by_label("Move Function text right").click();
    settle(&mut h);
    let n = h.state().shell().nav(NavKind::Strips).unwrap();
    let table = n.table().unwrap();
    let order: Vec<String> = n
        .table_mode()
        .unwrap()
        .shown_columns(table)
        .into_iter()
        .map(|c| table.columns[c].label.clone())
        .collect();
    let f = order.iter().position(|l| l == "Function text").unwrap();
    assert!(f > 2, "Function text moved right: {order:?}");
    h.key_press(Key::Escape);
    settle(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// The baseline (y) and the left edge (x) of the first glyph of the text `text` painted inside `region`.
fn text_origin(shapes: &[egui::Shape], region: egui::Rect, text: &str) -> Option<egui::Pos2> {
    shapes.iter().find_map(|s| match s {
        egui::Shape::Text(ts) if ts.galley.text() == text && region.contains(ts.pos) => {
            let row = ts.galley.rows.first()?;
            let glyph = row.row.glyphs.first()?;
            Some(ts.pos + row.pos.to_vec2() + glyph.pos.to_vec2())
        }
        _ => None,
    })
}

/// Doc 16a §6.21 (B33): the cell editor shows its text where the cell showed it, so nothing jumps when it opens: the
/// first glyph's baseline and left edge equal the cell label's within half a point.
#[test]
fn the_editor_text_sits_on_the_cell_text_baseline() {
    let tmp = Tmp::new();
    let mut h = strips(&tmp);
    show_column(&mut h, "Function text");
    let target = cell(&h, "Function text", 2);
    let text = {
        let n = h.state().shell().nav(NavKind::Strips).unwrap();
        let table = n.table().unwrap();
        let c = table
            .columns
            .iter()
            .position(|c| c.label == "Function text")
            .unwrap();
        let row = n.visible()[1].node as usize;
        table.nodes[row].cells[c].clone()
    };
    assert!(!text.is_empty(), "the cell has a text");
    let before = text_origin(&painted(&h), target, &text).expect("the cell's label");
    double_click(&mut h, target.center());
    settle(&mut h);
    assert!(
        h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .table_mode()
            .unwrap()
            .edit()
            .is_some(),
        "the editor is open"
    );
    let after = text_origin(&painted(&h), target, &text).expect("the editor's text");
    assert!(
        (after.y - before.y).abs() <= 0.5,
        "the baseline: label {before:?}, editor {after:?}"
    );
    assert!(
        (after.x - before.x).abs() <= 0.5,
        "the left edge: label {before:?}, editor {after:?}"
    );
    h.key_press(Key::Escape);
    settle(&mut h);
    h.state_mut().shell_mut().shutdown();
}

/// B33: the horizontal scroll handle of a grid wider than its pane stays on show at rest (X32b) in
/// `color.border.control`, never in the text colour (a near-white bar on the dark panel), in light and dark.
#[test]
fn the_scroll_handle_is_a_quiet_boundary_not_the_text_colour() {
    for theme in ["light", "dark"] {
        let tmp = Tmp::new();
        let dir = tmp.golden("g.ew");
        let mut shell = ShellModel::new(Mode::Gui, tmp.config());
        done(
            &mut shell,
            "settings.set",
            json!({"key": "theme", "value": theme}),
        );
        let mut h: Harness<'static, App> = harness_with(shell, Scripted::default(), Some(dir));
        h.set_size(egui::Vec2::new(1000.0, 900.0));
        settle(&mut h);
        wait_for_page(&mut h);
        let shell = h.state_mut().shell_mut();
        for (action, params) in [
            ("props.show", json!({"on": false})),
            ("navigator.show", json!({"kind": "strips"})),
            ("navigator.table", json!({"kind": "strips"})),
            ("navigator.filter", json!({"kind": "strips", "text": "x1031"})),
        ] {
            done(shell, action, params);
        }
        settle(&mut h);
        let t = ew_ui::theme::current(&h.ctx);
        let grid = h.get_by_label("Terminal strips table").rect();
        let key = cell(&h, "Strip", 1);
        // The bottom band of the grid, right of the frozen key column: where the horizontal handle runs.
        let band =
            egui::Rect::from_min_max(egui::pos2(key.right(), grid.bottom() - 12.0), grid.right_bottom());
        let shapes = painted(&h);
        let in_band = |colour: egui::Color32| {
            fills(&shapes, colour)
                .into_iter()
                .filter(|r| band.contains_rect(*r) && r.width() > r.height())
                .count()
        };
        assert!(
            in_band(t.border_control.color32()) >= 1,
            "{theme}: the handle in color.border.control"
        );
        assert_eq!(
            in_band(t.text_primary.color32()),
            0,
            "{theme}: no handle in the text colour"
        );
        h.state_mut().shell_mut().shutdown();
    }
}
