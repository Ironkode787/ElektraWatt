// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Table mode in the window (WP-S2.06 part 3) and FT-509: the platform's copy and paste events in a navigator's table
//! run `table.copy` and `table.paste` (the TSV goes to the system clipboard), the table card in the navigator's pane
//! with its Drop buttons and Apply, and the left dock's "All tabs" menu that reaches every navigator tab hidden behind
//! the tab strip's overflow; the accessible-name walker after every step (`settle`).

mod support;

use egui::OutputCommand;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_ui::app::App;
use ew_ui::ew_viewmodel::layout::PaneKind;
use ew_ui::ew_viewmodel::navigators::NavKind;
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

/// The window on a writable copy of Golden A, folio 102 shown.
fn window(tmp: &Tmp) -> Harness<'static, App> {
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    h
}

/// The Terminal strips navigator in table mode, filtered `x1032`, sorted by Terminal, with the focus.
fn strips_table(h: &mut Harness<'static, App>) {
    let shell = h.state_mut().shell_mut();
    done(shell, "navigator.show", json!({"kind": "strips"}));
    done(shell, "navigator.table", json!({"kind": "strips"}));
    done(
        shell,
        "navigator.filter",
        json!({"kind": "strips", "text": "x1032"}),
    );
    let by = shell
        .nav(NavKind::Strips)
        .and_then(|n| n.table())
        .and_then(|t| {
            t.columns
                .iter()
                .find(|c| c.label == "Terminal")
                .map(|c| c.key.clone())
        })
        .unwrap();
    done(shell, "navigator.sort", json!({"kind": "strips", "by": by}));
    done(shell, "ui.focus", json!({"area": "navigator", "kind": "strips"}));
    settle(h);
}

/// §8.6 in the window: with the Terminal strips table focused, the platform's copy event (Ctrl+C) runs `table.copy`
/// and hands the TSV to the system clipboard; its paste event (Ctrl+V) runs `table.paste` with the system clipboard's
/// text: pasted back onto the same row it changes nothing; a read-only cell opens the table card in the pane, whose
/// Drop button ends the gesture.
#[test]
fn copy_and_paste_events_run_the_table_actions() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    strips_table(&mut h);
    h.event(egui::Event::Copy);
    h.step();
    let copied = h
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .expect("the frame hands the TSV to the system clipboard");
    // WP-S2.07 part 1: the Pos column after the key column.
    assert!(
        copied.starts_with("Strip\tPos\tTerminal\tFunction text\t"),
        "{copied}"
    );
    assert_eq!(copied.lines().count(), 2, "the header and the cursor row");
    settle(&mut h);
    let rev = h.state().shell().revision();
    h.event(egui::Event::Paste(copied));
    settle(&mut h);
    assert_eq!(h.state().shell().revision(), rev, "the same text changes nothing");
    assert!(h.state().shell().properties().card().is_none());
    // A read-only cell: the card in the pane, Drop ends the gesture.
    h.event(egui::Event::Paste("X".into()));
    settle(&mut h);
    let gesture = h
        .state()
        .shell()
        .table_gesture()
        .cloned()
        .expect("the table card");
    assert_eq!(gesture.blocking(), 1);
    // No cell can be applied: Apply is disabled (its reason is the tooltip), Cancel and Drop stay.
    let applies: Vec<_> = h.query_all_by_label("Apply").collect();
    assert!(!applies.is_empty(), "the card's Apply");
    for a in applies {
        assert!(a.accesskit_node().is_disabled(), "Apply with nothing to apply");
    }
    h.get_by_label("Drop").click();
    settle(&mut h);
    assert!(h.state().shell().table_gesture().is_none());
    assert_eq!(h.state().shell().revision(), rev);
    h.state_mut().shell_mut().shutdown();
}

/// FT-509, WP-S2.17 part 3 DK1: the navigator group draws no tab bar; every navigator is one named rail button away
/// (a click shows it in the group), Ctrl+`<n>` picks the n-th entry, and the Project entry's segmented control switches
/// between Pages and Devices.
#[test]
fn ft509_every_tab_of_the_left_dock_is_reachable() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    settle(&mut h);
    assert!(
        h.query_by_label("All tabs").is_none(),
        "the navigator group has no tab bar"
    );
    for (title, pane) in [
        ("Terminal strips", PaneKind::Navigator(NavKind::Strips)),
        ("Cables", PaneKind::Navigator(NavKind::Cables)),
        ("Potentials", PaneKind::Navigator(NavKind::Potentials)),
        ("Structure identifiers", PaneKind::Navigator(NavKind::Structure)),
        ("Connections", PaneKind::Navigator(NavKind::Connections)),
        ("PLC", PaneKind::Navigator(NavKind::Plc)),
        ("Interruption points", PaneKind::Navigator(NavKind::Ips)),
        ("Parts in use", PaneKind::Navigator(NavKind::Parts)),
        ("Report blocks", PaneKind::Navigator(NavKind::Blocks)),
        ("Project", PaneKind::Pages),
    ] {
        h.get_by_role_and_label(egui::Role::Button, title).click();
        settle(&mut h);
        assert_eq!(
            h.state().shell().layout().shown_in_place_of(PaneKind::Pages),
            Some(pane),
            "{title}"
        );
    }
    // Ctrl+2 is the second entry (Terminal strips).
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    settle(&mut h);
    assert_eq!(
        h.state().shell().layout().shown_in_place_of(PaneKind::Pages),
        Some(PaneKind::Navigator(NavKind::Strips))
    );
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    settle(&mut h);
    // The segmented control under the Project header: Pages chosen, Devices one click away.
    h.get_by_role_and_label(egui::Role::Tab, "Devices").click();
    settle(&mut h);
    assert_eq!(
        h.state().dock_active_beside(PaneKind::Pages),
        Some(PaneKind::Navigator(NavKind::Devices))
    );
    h.state_mut().shell_mut().shutdown();
}

/// The key of the Terminal strips row on show whose cell in column `label` is `text`.
fn strips_row(shell: &ShellModel, label: &str, text: &str) -> String {
    let n = shell.nav(NavKind::Strips).expect("the navigator");
    let t = n.table().expect("its table");
    let c = t
        .columns
        .iter()
        .position(|c| c.label == label)
        .expect("the column");
    n.visible()
        .iter()
        .map(|r| &t.nodes[r.node as usize])
        .find(|node| node.cells[c] == text)
        .unwrap_or_else(|| panic!("no row with {label} {text}"))
        .key
        .clone()
}

/// The text of the cell in column `label` of row `key`.
fn strips_cell(shell: &ShellModel, key: &str, label: &str) -> String {
    let n = shell.nav(NavKind::Strips).expect("the navigator");
    let t = n.table().expect("its table");
    let c = t
        .columns
        .iter()
        .position(|c| c.label == label)
        .expect("the column");
    t.nodes.iter().find(|x| x.key == key).expect("the row").cells[c].clone()
}

/// WP-S2.07 part 1 in the window (SE15's walker, §6.5): the strip editor on -X1031 is the Terminal strips grid; an
/// accessory row is a row of the grid (its cells named like a terminal row's); with two end-bracket parts in the
/// project, Add accessory opens the chooser list (a `ListBox` named "Choose the part", its rows `ListBoxOption`s with
/// position and set size), Down and Enter pick the second part (one `fields.set` of `strip.accessories`), and the list
/// closes; the accessible-name walker after every step.
#[test]
fn se15_the_strip_editor_and_its_chooser_list() {
    let tmp = Tmp::new();
    let mut h = window(&tmp);
    // A second end bracket, so that Add accessory has to ask.
    let csv = tmp.0.join("brackets.csv");
    std::fs::write(
        &csv,
        "key,manufacturer,part_number,kind,type_designation,description,unit,test_data,tech_data\n\
         ACME end bracket,ACME,EB-1,end_bracket,,End bracket,piece,,\n",
    )
    .expect("the parts table");
    {
        let shell = h.state_mut().shell_mut();
        done(shell, "parts.import", json!({"path": csv.to_string_lossy()}));
        done(shell, "preview.apply", json!({}));
        done(shell, "strip.edit", json!({"strip": "-X1031"}));
        let k20 = strips_row(shell, "Terminal", "20");
        done(shell, "table.cell", json!({"kind": "strips", "row": k20}));
    }
    settle(&mut h);
    support::assert_named(&h);
    // Add accessory: two parts fit, the chooser list opens at the row.
    let rev = h.state().shell().revision();
    done(h.state_mut().shell_mut(), "strip.add_accessory", json!({}));
    settle(&mut h);
    support::assert_named(&h);
    assert_eq!(h.state().shell().revision(), rev, "nothing committed yet");
    let items = h
        .state()
        .shell()
        .nav(NavKind::Strips)
        .and_then(|n| n.table_mode())
        .and_then(|m| m.chooser())
        .expect("the chooser list")
        .items
        .clone();
    assert_eq!(items.len(), 2, "{items:?}");
    let list = h.get_by_label("Choose the part");
    assert_eq!(list.accesskit_node().role(), egui::accesskit::Role::ListBox);
    for (i, (text, _)) in items.iter().enumerate() {
        let option = h.get_by_label(text);
        let node = option.accesskit_node();
        assert_eq!(node.role(), egui::accesskit::Role::ListBoxOption);
        assert_eq!(node.position_in_set(), Some(i + 1));
        assert_eq!(node.size_of_set(), Some(2));
        assert_eq!(
            node.is_selected() == Some(true),
            i == 0,
            "the first row is active"
        );
    }
    // Down, Enter: the second part.
    h.key_press(egui::Key::ArrowDown);
    settle(&mut h);
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    support::assert_named(&h);
    assert_eq!(h.state().shell().revision(), rev.map(|r| r + 1), "one step");
    let last = h
        .state()
        .shell()
        .recorded_commands()
        .last()
        .expect("the commit")
        .clone();
    assert_eq!(last.cmd, "fields.set", "{}", last.params);
    assert_eq!(last.params["path"], "strip.accessories");
    assert!(h.query_by_label("Choose the part").is_none(), "the list closed");
    let shell = h.state().shell();
    let acc = strips_row(shell, "Pos", "20.1");
    let number = items[1].0.split(" (").next().expect("the part number");
    assert_eq!(strips_cell(shell, &acc, "Part"), number);
    // The accessory row is a row of the grid: its cells are named like a terminal row's.
    assert!(
        h.query_by_label_contains("Pos 20.1, row 21 of 21").is_some(),
        "the accessory row's Pos cell"
    );
    h.state_mut().shell_mut().shutdown();
}
