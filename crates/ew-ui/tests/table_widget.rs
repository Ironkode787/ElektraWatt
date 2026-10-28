// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The navigators' table widget (WP-S2.06 part 3 §8.1; acceptance check TB7 and the window half of TB2): the key
//! column stays at the left while the table scrolls right, a header click sorts, the column chooser hides and orders
//! columns, every cell has the AccessKit name of §8.1, and the flows of TB2 (a cell edit and fill down, one undo
//! step), TB3 (pattern fill) and TB4 (paste with an ambiguous number resolved on the card) run from the keyboard and
//! AccessKit actions alone, without a pointer event. The accessible-name walker runs after every step (`settle`).

mod support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole, SortBy};
use ew_ui::ew_viewmodel::shell::{Outcome, ShellModel};
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// The window on a writable copy of Golden A, folio 102 shown, navigator `kind` in table mode filtered `filter`,
/// with the focus.
fn table(tmp: &Tmp, kind: &str, filter: &str) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "navigator.show", json!({ "kind": kind }));
    done(shell, "navigator.table", json!({ "kind": kind }));
    done(shell, "navigator.filter", json!({"kind": kind, "text": filter}));
    done(shell, "ui.focus", json!({"area": "navigator", "kind": kind}));
    settle(&mut h);
    h
}

/// Presses `key` with `modifiers` and lets the window settle.
fn key(h: &mut Harness<'static, App>, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    settle(h);
}

/// Presses `key` `n` times.
fn keys(h: &mut Harness<'static, App>, k: Key, n: usize) {
    for _ in 0..n {
        key(h, Modifiers::NONE, k);
    }
}

/// Types `text` (one text event) and lets the window settle.
fn type_text(h: &mut Harness<'static, App>, text: &str) {
    h.event(egui::Event::Text(text.to_owned()));
    settle(h);
}

/// Whether a node named exactly `name` is on show.
fn shown(h: &Harness<'static, App>, name: &str) -> bool {
    h.query_by_label(name).is_some()
}

/// The texts of the column labelled `label` of navigator `kind`, rows in display order.
fn texts(shell: &ShellModel, kind: NavKind, label: &str) -> Vec<String> {
    let n = shell.nav(kind).unwrap();
    let t = n.table().unwrap();
    let c = t.columns.iter().position(|c| c.label == label).unwrap();
    n.visible()
        .iter()
        .map(|r| t.nodes[r.node as usize].cells[c].clone())
        .collect()
}

/// The committed steps so far.
fn steps(shell: &ShellModel) -> usize {
    shell.recorded_commands().len()
}

/// TB7: in the Terminal strips table (-X1031, 20 rows) every cell is named `<column> <text>, row <i> of <n>` (`,
/// read-only` for the derived or key cells), the headers are `Sort by <column>` buttons; End moves the cell cursor to
/// the last column and the table scrolls right: the key column (Strip) stays at the left edge while Terminal scrolls
/// out and State comes into view; Home scrolls back.
#[test]
fn tb7_the_key_column_stays_while_the_table_scrolls_right() {
    let tmp = Tmp::new();
    let mut h = table(&tmp, "strips", "x1031");
    for name in [
        "Strip -X1031, row 1 of 20, read-only",
        "Terminal 1, row 1 of 20",
        "Function text 1+, row 1 of 20",
        "Sort by Strip",
        "Sort by Terminal",
        "Terminal strips table",
    ] {
        assert!(shown(&h, name), "{name}");
    }
    let key_left = h
        .get_by_label("Strip -X1031, row 1 of 20, read-only")
        .rect()
        .left();
    let state = |h: &Harness<'static, App>| {
        h.query_all_by_label_contains("State ").any(|n| {
            n.accesskit_node()
                .label()
                .is_some_and(|l| l.ends_with(", row 1 of 20"))
        })
    };
    assert!(!state(&h), "State is right of the pane");
    key(&mut h, Modifiers::NONE, Key::End);
    assert_eq!(
        h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .cursor_cell()
            .map(|c| c.1),
        Some(9),
        "the cursor is on the last column"
    );
    assert!(state(&h), "State scrolled into view");
    assert!(!shown(&h, "Terminal 1, row 1 of 20"), "Terminal scrolled out");
    let key_cell = h.get_by_label("Strip -X1031, row 1 of 20, read-only");
    assert_eq!(key_cell.rect().left(), key_left, "the key column is frozen");
    key(&mut h, Modifiers::NONE, Key::Home);
    assert!(shown(&h, "Terminal 1, row 1 of 20"), "Home scrolls back");
    h.state_mut().shell_mut().shutdown();
}

/// TB7: a header click sorts by that column, a second one reverses it; the column chooser shows a check box per column
/// (the key column's is disabled), hides a column and moves another left. FT-523: the sorted header's name ends
/// ", sorted ascending" or ", sorted descending".
#[test]
fn tb7_header_click_sorts_and_the_chooser_hides_and_orders() {
    let tmp = Tmp::new();
    let mut h = table(&tmp, "strips", "x1031");
    h.get_by_label("Sort by Terminal").click();
    settle(&mut h);
    let nav = |h: &Harness<'static, App>| {
        let n = h.state().shell().nav(NavKind::Strips).unwrap();
        (n.sort.clone(), n.descending)
    };
    assert_eq!(nav(&h), (SortBy::Column("terminal".into()), false));
    assert!(shown(&h, "Sort by Terminal, sorted ascending"));
    assert!(shown(&h, "Sort by Strip"), "an unsorted header keeps its name");
    h.get_by_label("Sort by Terminal, sorted ascending").click();
    settle(&mut h);
    assert_eq!(nav(&h), (SortBy::Column("terminal".into()), true));
    assert!(shown(&h, "Sort by Terminal, sorted descending"));
    assert!(shown(&h, "Terminal 20, row 1 of 20"), "descending");
    // The chooser.
    h.get_by_label("Columns").click();
    settle(&mut h);
    assert!(
        h.get_by_label("Strip").accesskit_node().is_disabled(),
        "the key column stays"
    );
    h.get_by_label("Move State left").click();
    settle(&mut h);
    let order = |h: &Harness<'static, App>| {
        let n = h.state().shell().nav(NavKind::Strips).unwrap();
        let t = n.table().unwrap();
        n.table_mode()
            .unwrap()
            .shown_columns(t)
            .into_iter()
            .map(|c| t.columns[c].label.clone())
            .collect::<Vec<_>>()
    };
    let after = order(&h);
    assert_eq!(after[after.len() - 2], "State", "{after:?}");
    if h.query_by_label("Function text").is_none() {
        // The menu closed with the click: open it again.
        h.get_by_label("Columns").click();
        settle(&mut h);
    }
    h.get_by_label("Function text").click();
    settle(&mut h);
    assert!(!order(&h).contains(&"Function text".to_owned()));
    key(&mut h, Modifiers::NONE, Key::Escape);
    assert!(!shown(&h, "Sort by Function text"), "the column is hidden");
    h.state_mut().shell_mut().shutdown();
}

/// TB2 in the window, from the keyboard alone: Terminal strips in table mode filtered `x1031` and sorted by Terminal
/// (an AccessKit click on its header) list 1 to 20; Ctrl+Home, Down to terminal 13, Right to Function text, typing
/// "Spare" and Enter set its function text (one step); Shift+Down four times marks 13 to 17; Ctrl+D fills "Spare" down
/// as one `fields.set` with 5 targets, a direct commit with the toast; Ctrl+Z undoes it in one step and restores 14 to
/// 17.
#[test]
fn tb2_fill_down_from_the_keyboard_is_one_step() {
    let tmp = Tmp::new();
    let mut h = table(&tmp, "strips", "x1031");
    // WP-S2.17 part 1: twenty rows of `size.row` (28 points) need a taller window than 700 points to be all on show.
    h.set_size(egui::Vec2::new(1000.0, 900.0));
    // WP-S2.17 part 3 (D10): Properties under the navigator would halve it; this test needs the whole column.
    done(h.state_mut().shell_mut(), "props.show", json!({"on": false}));
    settle(&mut h);
    h.get_by_label("Sort by Terminal").click_accesskit();
    settle(&mut h);
    let order: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
    assert_eq!(texts(h.state().shell(), NavKind::Strips, "Terminal"), order);
    for n in 1..=20 {
        assert!(shown(&h, &format!("Terminal {n}, row {n} of 20")), "{n}");
    }
    let before = texts(h.state().shell(), NavKind::Strips, "Function text");
    key(&mut h, Modifiers::COMMAND, Key::Home);
    keys(&mut h, Key::ArrowDown, 12);
    keys(&mut h, Key::ArrowRight, 2);
    assert!(
        h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .cursor_cell()
            .is_some()
    );
    let n = steps(h.state().shell());
    type_text(&mut h, "Spare");
    let name = format!("Function text {}, row 13 of 20", before[12]);
    assert!(
        h.state()
            .shell()
            .nav(NavKind::Strips)
            .unwrap()
            .table_mode()
            .unwrap()
            .edit()
            .is_some_and(|e| e.buffer == "Spare"),
        "typing starts the edit"
    );
    assert!(h.get_by_label(&name).is_focused(), "the editor has the keyboard");
    key(&mut h, Modifiers::NONE, Key::Enter);
    assert_eq!(steps(h.state().shell()), n + 1, "one step");
    assert!(shown(&h, "Function text Spare, row 13 of 20"));
    for _ in 0..4 {
        key(&mut h, Modifiers::SHIFT, Key::ArrowDown);
    }
    assert_eq!(h.state().shell().nav(NavKind::Strips).unwrap().marked().len(), 5);
    key(&mut h, Modifiers::COMMAND, Key::D);
    let shell = h.state().shell();
    assert_eq!(steps(shell), n + 2, "one more step");
    let last = shell.recorded_commands().last().unwrap();
    assert_eq!(last.cmd, "fields.set");
    assert_eq!(last.params["targets"].as_array().unwrap().len(), 5);
    assert!(shell.properties().card().is_none(), "a direct commit");
    let filled = texts(shell, NavKind::Strips, "Function text");
    assert_eq!(&filled[12..17], ["Spare"; 5]);
    for row in 13..=17 {
        assert!(
            shown(&h, &format!("Function text Spare, row {row} of 20")),
            "{row}"
        );
    }
    key(&mut h, Modifiers::COMMAND, Key::Z);
    let undone = texts(h.state().shell(), NavKind::Strips, "Function text");
    assert_eq!(undone[12], "Spare");
    assert_eq!(
        &undone[13..17],
        &before[13..17],
        "one undo step restores 14 to 17"
    );
    h.state_mut().shell_mut().shutdown();
}

/// TB3 in the window, from the keyboard alone: Space marks -X1031's 12 spare terminals, Ctrl+Shift+D opens the
/// pattern dialog with the keyboard in its field, typing `Spare_{n:02}` shows the first five values, Enter applies one
/// `fields.set_many`.
#[test]
fn tb3_pattern_fill_from_the_keyboard() {
    let tmp = Tmp::new();
    let mut h = table(&tmp, "strips", "x1031");
    h.get_by_label("Sort by Terminal").click_accesskit();
    settle(&mut h);
    let states = texts(h.state().shell(), NavKind::Strips, "State");
    key(&mut h, Modifiers::COMMAND, Key::Home);
    for s in &states {
        if s == "spare" {
            key(&mut h, Modifiers::NONE, Key::Space);
        }
        key(&mut h, Modifiers::NONE, Key::ArrowDown);
    }
    assert_eq!(h.state().shell().nav(NavKind::Strips).unwrap().marked().len(), 12);
    key(&mut h, Modifiers::NONE, Key::Home);
    keys(&mut h, Key::ArrowRight, 2);
    key(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::D);
    assert!(
        h.get_by_label("Pattern").is_focused(),
        "the dialog's field has the keyboard"
    );
    type_text(&mut h, "Spare_{n:02}");
    assert!(shown(
        &h,
        "Values: Spare_01, Spare_02, Spare_03, Spare_04, Spare_05"
    ));
    let n = steps(h.state().shell());
    key(&mut h, Modifiers::NONE, Key::Enter);
    let shell = h.state().shell();
    assert_eq!(steps(shell), n + 1, "one step");
    assert_eq!(shell.recorded_commands().last().unwrap().cmd, "fields.set_many");
    let fn_texts = texts(shell, NavKind::Strips, "Function text");
    let spare: Vec<&String> = states
        .iter()
        .zip(&fn_texts)
        .filter(|(s, _)| *s == "spare")
        .map(|(_, t)| t)
        .collect();
    let expected: Vec<String> = (1..=12).map(|n| format!("Spare_{n:02}")).collect();
    assert_eq!(spare, expected.iter().collect::<Vec<_>>());
    assert!(!shown(&h, "Pattern"), "the dialog closed");
    h.state_mut().shell_mut().shutdown();
}

/// TB4 in the window, from the keyboard and AccessKit actions: in the Cables table (-106W1, sorted by Cable) with the
/// cursor on the first core's mm² cell, the platform's paste event with `0,75` and `1,500` opens the table card with
/// the second cell ambiguous; Ctrl+Enter is refused while it blocks; an AccessKit click on the reading "1.5
/// (decimal)" resolves it and Ctrl+Enter applies one command.
#[test]
fn tb4_paste_resolve_and_apply_without_a_pointer() {
    let tmp = Tmp::new();
    let mut h = table(&tmp, "cables", "-106W1");
    h.get_by_label("Sort by Cable").click_accesskit();
    settle(&mut h);
    let (first, csa) = {
        let n = h.state().shell().nav(NavKind::Cables).unwrap();
        let t = n.table().unwrap();
        let first = n
            .visible()
            .iter()
            .position(|r| t.nodes[r.node as usize].role == NavRole::Conductor)
            .expect("a core row");
        let shown = n.table_mode().unwrap().shown_columns(t);
        let csa = shown.iter().position(|c| t.columns[*c].label == "mm²").unwrap();
        (first, csa)
    };
    key(&mut h, Modifiers::COMMAND, Key::Home);
    keys(&mut h, Key::ArrowDown, first);
    keys(&mut h, Key::ArrowRight, csa);
    let n = steps(h.state().shell());
    h.event(egui::Event::Paste("0,75\r\n1,500\r\n".into()));
    settle(&mut h);
    let gesture = h.state().shell().table_gesture().cloned().expect("the card");
    assert_eq!(gesture.blocking(), 1);
    key(&mut h, Modifiers::COMMAND, Key::Enter);
    assert_eq!(steps(h.state().shell()), n, "refused while a cell blocks");
    assert!(h.state().shell().table_gesture().is_some());
    h.get_by_label("1.5 (decimal)").click_accesskit();
    settle(&mut h);
    assert_eq!(h.state().shell().table_gesture().map(|g| g.blocking()), Some(0));
    key(&mut h, Modifiers::COMMAND, Key::Enter);
    let shell = h.state().shell();
    assert_eq!(steps(shell), n + 1, "one command");
    assert!(shell.table_gesture().is_none());
    let vals = texts(shell, NavKind::Cables, "mm²");
    assert_eq!(&vals[first..first + 2], ["0.75 mm²", "1.5 mm²"]);
    h.state_mut().shell_mut().shutdown();
}

/// W31-3: a header whose text is wider than its column ("Function text") is cut to the column less its padding, not
/// run into the next one.
#[test]
fn w31_3_a_header_text_is_truncated_within_its_column() {
    let tmp = Tmp::new();
    let mut h = table(&tmp, "strips", "x1031");
    settle(&mut h);
    let header = h.get_by_label("Sort by Function text").rect();
    let cell = h.get_by_label("Function text 1+, row 1 of 20").rect();
    // The full header text is wider than the column: what is on show is cut.
    let font = egui::TextStyle::Button.resolve(&h.ctx.global_style());
    let full = h.ctx.fonts_mut(|f| {
        f.layout_no_wrap("Function text".to_owned(), font, egui::Color32::WHITE)
            .size()
            .x
    });
    assert!(full > header.width(), "{full} wider than {header:?}");
    assert!(
        header.left() >= cell.left() - 0.5 && header.right() <= cell.right() + 0.5,
        "the header {header:?} inside its column {cell:?}"
    );
    let next = h.get_by_label("Sort by Group").rect();
    assert!(header.right() < next.left(), "{header:?} ends before {next:?}");
    h.state_mut().shell_mut().shutdown();
}
