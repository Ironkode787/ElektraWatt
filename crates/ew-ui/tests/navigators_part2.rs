// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The further navigators and highlighting in the window (WP-S2.06 part 2, acceptance checks NP3 and NP4): the
//! highlight of 24VDC-1 composited into `canvas.snapshot` with the slot colour at each drawn segment's midpoint and the
//! hatch lines at 45°, Ctrl+Shift+H in the Potentials pane, and a selection made on the canvas marking the W29 rows of
//! the Connections and Potentials panes with the focus left on the canvas; the accessible-name walker after every step
//! (`settle`).

mod support;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_render2d::view::View;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::highlight::SLOT_COLOURS;
use ew_ui::ew_viewmodel::navigators::{NavKind, NavRole};
use ew_ui::ew_viewmodel::shell::{Focus, Mode, Outcome, ShellModel};
use ew_ui::ew_viewmodel::tools::OverlayItem;
use serde_json::{Value, json};
use support::{Scripted, Tmp, harness, settle, wait_for_page};

fn done(shell: &mut ShellModel, action: &str, params: Value) {
    assert_eq!(
        shell.dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

fn row_key(shell: &ShellModel, kind: NavKind, role: NavRole, label: &str) -> String {
    let t = shell
        .nav(kind)
        .and_then(|n| n.table())
        .expect("an open navigator");
    t.nodes
        .iter()
        .find(|n| n.role == role && n.label == label)
        .map(|n| n.key.clone())
        .unwrap_or_else(|| panic!("no {role:?} row {label}"))
}

/// The RGBA of pixel `(x, y)`.
fn px(img: &ew_render_cpu::RgbaImage, x: i64, y: i64) -> [u8; 4] {
    if x < 0 || y < 0 || x >= i64::from(img.width) || y >= i64::from(img.height) {
        return [0; 4];
    }
    let i = (y as usize * img.width as usize + x as usize) * 4;
    img.pixels[i..i + 4].try_into().unwrap()
}

/// NP3 (FR-UI-018, FT-362; D14), the snapshot half: with 24VDC-1 highlighted from the Potentials navigator (slot 0),
/// `canvas.snapshot` of folios 102 and 103 shows the slot colour at every drawn member segment's midpoint (the band, or
/// a hatch line in the full colour within 3 pixels), the hatch lines run at 45° (a full-colour pixel continues
/// up-right, not down-right), and without the highlight the same pixels are as before.
#[test]
fn np3_the_highlight_is_composited_into_the_snapshot() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut shell = ShellModel::new(Mode::Headless, tmp.config());
    done(
        &mut shell,
        "file.open",
        json!({"dir": dir.to_string_lossy(), "read_only": true}),
    );
    done(&mut shell, "navigator.show", json!({"kind": "potentials"}));
    let key = row_key(&shell, NavKind::Potentials, NavRole::Potential, "24VDC-1");
    let (r, g, b) = SLOT_COLOURS[0];
    let full = [r, g, b, 255];
    for folio in ["102", "103"] {
        done(&mut shell, "page.goto", json!({"folio": folio}));
        let (before, _) = ew_ui::canvas::snapshot_image(&shell, 0).unwrap();
        done(
            &mut shell,
            "navigator.highlight",
            json!({"kind": "potentials", "key": key}),
        );
        let (after, _) = ew_ui::canvas::snapshot_image(&shell, 0).unwrap();
        let c = shell.canvas().unwrap();
        let view = View {
            centre: c.centre(),
            zoom: c.zoom(),
            pane: c.native_pane(),
            ppp: c.native_ppp(),
        };
        let t = view.settled_transform();
        let overlay = shell.highlight_overlay(0);
        let (OverlayItem::Highlight { polylines, slot }, _) = &overlay[0] else {
            panic!("a highlight on {folio}");
        };
        assert_eq!(*slot, 0);
        let mut segments = 0;
        let mut diagonal = false;
        for line in polylines {
            for w in line.windows(2) {
                let (a, b) = (
                    t.to_screen((w[0].0 as f64, w[0].1 as f64)),
                    t.to_screen((w[1].0 as f64, w[1].1 as f64)),
                );
                if (a.0 - b.0).abs() + (a.1 - b.1).abs() < 8.0 {
                    continue;
                }
                let (mx, my) = (
                    ((a.0 + b.0) / 2.0).floor() as i64,
                    ((a.1 + b.1) / 2.0).floor() as i64,
                );
                segments += 1;
                assert_ne!(
                    px(&after, mx, my),
                    px(&before, mx, my),
                    "{folio}: tinted at ({mx}, {my})"
                );
                let window: Vec<(i64, i64)> = (-3..=3)
                    .flat_map(|dy| (-3..=3).map(move |dx| (mx + dx, my + dy)))
                    .collect();
                assert!(
                    window.iter().any(|(x, y)| px(&after, *x, *y) == full),
                    "{folio}: a hatch pixel in the slot colour near ({mx}, {my})"
                );
                // 45°: a full-colour pixel whose up-right and down-left neighbours are full colour too, and not both
                // of its down-right and up-left ones.
                diagonal |= window.iter().any(|(x, y)| {
                    px(&after, *x, *y) == full
                        && px(&after, x + 1, y - 1) == full
                        && px(&after, x - 1, y + 1) == full
                        && !(px(&after, x + 1, y + 1) == full && px(&after, x - 1, y - 1) == full)
                });
            }
        }
        assert!(segments > 0, "{folio}: drawn segments");
        assert!(diagonal, "{folio}: the hatch lines run at 45°");
        // Off again: the snapshot is the one before.
        done(
            &mut shell,
            "navigator.highlight",
            json!({"kind": "potentials", "key": key}),
        );
        let (again, _) = ew_ui::canvas::snapshot_image(&shell, 0).unwrap();
        assert!(
            again.pixels == before.pixels,
            "{folio}: removing the highlight restores the snapshot"
        );
    }
    shell.shutdown();
}

/// NP3 and NP4 in the window: Ctrl+Shift+H in the Potentials pane highlights the cursor row (24VDC-1) and again removes
/// it; with Connections and Potentials open, a selection made on the canvas (W29 on 103) marks its row in both panes
/// as selected tree items, with the focus left on the canvas.
#[test]
fn np3_np4_the_panes_highlight_and_follow_the_canvas() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h: Harness<'static, App> = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    wait_for_page(&mut h);
    let shell = h.state_mut().shell_mut();
    done(shell, "page.goto", json!({"folio": "103"}));
    done(shell, "navigator.show", json!({"kind": "connections"}));
    done(shell, "navigator.show", json!({"kind": "potentials"}));
    let key = row_key(shell, NavKind::Potentials, NavRole::Potential, "24VDC-1");
    done(shell, "navigator.row", json!({"kind": "potentials", "key": key}));
    settle(&mut h);
    assert_eq!(h.state().shell().focused_navigator(), Some(NavKind::Potentials));
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::H);
    settle(&mut h);
    let items = h.state().shell().highlights().items().to_vec();
    assert_eq!(items.len(), 1, "Ctrl+Shift+H highlights the cursor row");
    assert_eq!((items[0].slot, items[0].name.as_str()), (0, "24VDC-1"));
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::H);
    settle(&mut h);
    assert!(h.state().shell().highlights().is_empty(), "and again removes it");

    // The canvas selects W29 (the selection a click produces): both panes mark its row.
    let shell = h.state_mut().shell_mut();
    let row = row_key(shell, NavKind::Connections, NavRole::Connection, "102.10");
    let (_, node) = shell.nav(NavKind::Connections).unwrap().node_of(&row).unwrap();
    let w29 = node
        .drawables
        .iter()
        .find(|d| d.folio == "103")
        .map(|d| d.record)
        .unwrap();
    done(shell, "ui.focus", json!({"area": "canvas"}));
    done(shell, "select.set", json!({"ids": [w29.to_string()]}));
    settle(&mut h);
    let shell = h.state().shell();
    assert!(shell.nav(NavKind::Connections).unwrap().marked().contains(&row));
    let marked: Vec<String> = shell
        .nav(NavKind::Potentials)
        .unwrap()
        .marked()
        .iter()
        .cloned()
        .collect();
    assert_eq!(marked.len(), 1);
    assert!(marked[0].starts_with(&key), "under 24VDC-1");
    assert!(matches!(shell.focus(), Some(Focus::Canvas(_))));
    let selected: Vec<String> = h
        .get_all_by_role(egui::Role::TreeItem)
        .filter(|n| n.accesskit_node().is_selected() == Some(true))
        .filter_map(|n| n.accesskit_node().label())
        .collect();
    assert!(
        selected
            .iter()
            .filter(|l| l.starts_with("102.10, connection"))
            .count()
            >= 1,
        "{selected:?}"
    );
    h.state_mut().shell_mut().shutdown();
}
