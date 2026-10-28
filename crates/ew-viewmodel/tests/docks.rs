// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.17 part 3 through the shell on copies of Golden A: the property grid's groups (DK3), the status bar's fields
//! (DK4), the default workspace and an older workspace file (DK5), the rail model and `rail.show` (DK1's model half),
//! `props.show_advanced`, and the History focus of FT-519.

mod common;

use std::collections::BTreeSet;

use common::{Tmp, done, refused};
use ew_viewmodel::layout::{Layout, LayoutNode, PaneKind, SplitDir};
use ew_viewmodel::navigators::NavKind;
use ew_viewmodel::properties::{RowGroup, groups, is_advanced, row_label};
use ew_viewmodel::rail::{RailTarget, rail};
use ew_viewmodel::shell::{Focus, Mode, ShellModel};
use ew_viewmodel::status::{StatusKind, Tone, bar};
use serde_json::json;

/// W29 on folio 103 (`connections|W29`).
const W29: &str = "9eb2d865-fca4-59b1-87bf-c073d7190ceb";

/// DK3: for a page, a device, a terminal and a connection, no two rows of a section share a label; Main is the first
/// group; the Advanced rows stay out until `props.show_advanced {on: true}` (the window's half, no chip on an
/// authored row and no edit button, is `ew-ui`'s `docks` test).
#[test]
fn dk3_the_groups_labels_and_advanced_rows() {
    let tmp = Tmp::new("docks-dk3");
    let mut shell = tmp.shell_in(Mode::Headless, true, "102");
    let mut seen_advanced = 0;
    for (folio, select) in [
        ("101", json!({"refs": []})),
        ("101", json!({"refs": ["tag:-101F1"]})),
        ("102", json!({"refs": ["tag:-X1031:1"]})),
        ("103", json!({"ids": [W29]})),
    ] {
        done(&mut shell, "page.goto", json!({ "folio": folio }));
        done(&mut shell, "select.set", select.clone());
        let Some(sheet) = shell.properties().sheet() else {
            continue;
        };
        for section in &sheet.sections {
            let mut labels = BTreeSet::new();
            for row in &section.rows {
                let label = row_label(section, row);
                assert!(
                    labels.insert(label.clone()),
                    "{select}: two rows of {} are labelled {label}",
                    section.kind
                );
            }
            let hidden = groups(section, false);
            assert!(
                hidden
                    .iter()
                    .all(|(_, rows)| rows.iter().all(|r| !is_advanced(r))),
                "{select}: an Advanced row without show_advanced"
            );
            let shown = groups(section, true);
            seen_advanced += shown
                .iter()
                .flat_map(|(_, rows)| rows.iter())
                .filter(|r| is_advanced(r))
                .count();
            if let Some((first, rows)) = hidden.first()
                && !rows.is_empty()
            {
                assert_eq!(*first, RowGroup::Main, "{select}: Main first");
            }
        }
    }
    assert!(seen_advanced > 0, "Golden A has Advanced rows");
    // The toggle is a setting and a menu check.
    assert!(!shell.properties_show_advanced());
    done(&mut shell, "props.show_advanced", json!({"on": true}));
    assert!(shell.properties_show_advanced());
    done(&mut shell, "props.show_advanced", json!({}));
    assert!(!shell.properties_show_advanced(), "no `on` toggles");
}

/// DK4: the status bar's fields in D12's order with "Ready" at the left, no page or language field; the findings only
/// while Messages is open and as one field; the renderer in the warning tone with its tooltip on the CPU.
#[test]
fn dk4_the_status_bar_fields() {
    let tmp = Tmp::new("docks-dk4");
    let mut shell = tmp.shell_in(Mode::Headless, true, "101");
    done(&mut shell, "select.set", json!({"refs": ["tag:-101F1"]}));
    let fields = bar(&shell, false);
    assert_eq!(fields[0].kind, StatusKind::State);
    assert_eq!(fields[0].text, "Ready");
    let kinds: Vec<StatusKind> = fields.iter().map(|f| f.kind).collect();
    let mut sorted = kinds.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(kinds, sorted, "D12's order, each field once");
    assert!(kinds.contains(&StatusKind::Selection));
    assert!(!kinds.contains(&StatusKind::Findings), "Messages is closed");
    assert!(
        fields.iter().all(|f| !f.text.contains(" of ")),
        "no page field: {fields:?}"
    );
    let renderer = fields
        .iter()
        .find(|f| f.kind == StatusKind::Renderer)
        .expect("the renderer");
    if !renderer.text.starts_with("GPU") {
        assert_eq!(renderer.tone, Tone::Warning);
        assert!(!renderer.tip.is_empty());
    }
    let save = fields.iter().find(|f| f.kind == StatusKind::Save).unwrap();
    assert_eq!((save.text.as_str(), save.tone), ("Saved", Tone::Success));
    done(&mut shell, "messages.show", json!({"on": true}));
    let findings: Vec<_> = bar(&shell, false)
        .into_iter()
        .filter(|f| f.kind == StatusKind::Findings)
        .collect();
    assert!(findings.len() <= 1, "one field: {findings:?}");
    let describe = ew_viewmodel::describe::describe(&shell);
    let line = describe
        .lines()
        .find(|l| l.starts_with("status bar: "))
        .expect("the status bar line");
    assert!(line.starts_with("status bar: Ready | "), "{line}");
}

/// DK5: the default workspace is D10's (Pages over Properties at the left, 233 ‰; the page view 767 ‰), its file is
/// deterministic and reads back equal; a workspace file written before the package loads with its panes in place.
#[test]
fn dk5_the_default_workspace_and_an_older_file() {
    let layout = Layout::schematic();
    assert_eq!(
        layout.root,
        LayoutNode::Split {
            dir: SplitDir::Horizontal,
            children: vec![
                (
                    233,
                    LayoutNode::Split {
                        dir: SplitDir::Vertical,
                        children: vec![
                            (500, LayoutNode::Pane(PaneKind::Pages)),
                            (500, LayoutNode::Pane(PaneKind::Properties)),
                        ],
                    }
                ),
                (767, LayoutNode::Pane(PaneKind::PageView)),
            ],
        }
    );
    let a = layout.to_json("Schematic");
    assert_eq!(a, Layout::schematic().to_json("Schematic"), "deterministic bytes");
    assert_eq!(Layout::parse(&a).unwrap().1, layout);
    // The layout before part 3: Pages | Page view | Properties side by side.
    let older = Layout {
        root: LayoutNode::Split {
            dir: SplitDir::Horizontal,
            children: vec![
                (200, LayoutNode::Pane(PaneKind::Pages)),
                (600, LayoutNode::Pane(PaneKind::PageView)),
                (200, LayoutNode::Pane(PaneKind::Properties)),
            ],
        },
    };
    let tmp = Tmp::new("docks-dk5");
    let dir = tmp.0.join("config/workspaces");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Schematic.json"), older.to_json("Schematic")).unwrap();
    let shell = tmp.shell("101");
    assert_eq!(shell.layout(), &older, "the older file keeps its panes in place");
}

/// DK1's model half: the rail's entries in the mockup's order with Ctrl+1 to Ctrl+9, History at the foot, the right
/// rail's two palettes; `rail.show` shows a navigator in the group and collapses the column on its own entry.
#[test]
fn dk1_the_rail_model_and_rail_show() {
    let tmp = Tmp::new("docks-dk1");
    let mut shell = tmp.shell("101");
    let model = rail(&shell);
    let words: Vec<&str> = model.left.iter().map(|b| b.word).collect();
    assert_eq!(
        words,
        [
            "project",
            "strips",
            "cables",
            "potentials",
            "structure",
            "connections",
            "plc",
            "ips",
            "parts",
            "blocks"
        ]
    );
    assert_eq!(model.left[0].shortcut, "Ctrl+1");
    assert_eq!(model.left[8].shortcut, "Ctrl+9");
    assert!(model.left[9].shortcut.is_empty());
    assert!(model.left[0].active, "Project shows Pages");
    assert_eq!(model.foot.word, "history");
    assert_eq!(model.right.len(), 2);
    assert!(matches!(model.right[0].target, RailTarget::Action(_)));
    done(&mut shell, "rail.show", json!({"pane": "cables"}));
    assert_eq!(
        shell.layout().shown_in_place_of(PaneKind::Pages),
        Some(PaneKind::Navigator(NavKind::Cables))
    );
    assert!(rail(&shell).left[2].active);
    assert_eq!(shell.focus(), Some(Focus::Navigator(NavKind::Cables)));
    done(&mut shell, "rail.show", json!({"pane": "cables"}));
    assert!(shell.rail_collapsed(), "a click on the shown entry collapses");
    assert!(rail(&shell).collapsed);
    done(&mut shell, "rail.show", json!({"pane": "devices"}));
    assert!(!shell.rail_collapsed());
    assert_eq!(
        shell.layout().shown_in_place_of(PaneKind::Pages),
        Some(PaneKind::Navigator(NavKind::Devices))
    );
    assert_eq!(
        refused(
            &mut shell,
            "rail.show",
            json!({"side": "right", "pane": "symbols"})
        ),
        "reason-rail-no-palettes"
    );
    let describe = ew_viewmodel::describe::describe(&shell);
    assert!(describe.contains("rail left \"Project\" (shown)"), "{describe}");
}

/// FT-519's model half: `ui.focus {area: history}` while the panel is shown, and F6 reaches it.
#[test]
fn ft519_the_history_focus() {
    let tmp = Tmp::new("docks-ft519");
    let mut shell: ShellModel = tmp.shell("101");
    assert!(matches!(
        shell.dispatch("ui.focus", &json!({"area": "history"})),
        ew_viewmodel::shell::Outcome::Failed { .. }
    ));
    done(&mut shell, "history.show", json!({}));
    done(&mut shell, "ui.focus", json!({"area": "history"}));
    assert_eq!(shell.focus(), Some(Focus::History));
    done(&mut shell, "ui.focus", json!({"area": "navigator"}));
    for _ in 0..12 {
        if shell.focus() == Some(Focus::History) {
            break;
        }
        done(&mut shell, "ui.focus_next", json!({}));
    }
    assert_eq!(shell.focus(), Some(Focus::History));
}
