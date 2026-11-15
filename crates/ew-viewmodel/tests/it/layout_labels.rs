// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A layout page that can be read (the owner's finding of the first 0.3.0 candidate: at 1:10 the tags ran into each
//! other; he asked for numbers, a table beside the layout and labels that move), through the shell on temporary copies
//! of Golden A: a balloon off its device drags alone and the device's menu puts it back; a new layout page at 1:10 shows
//! item numbers, gets its legend (with the description) and prints its tags without the cabinet's location; Add legend is refused where the
//! page holds one.

use crate::common;

use common::{Tmp, at, done, refused};
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
use ew_viewmodel::ew_api::output::{PanelTables, Uuid};
use ew_viewmodel::shell::{Mode, ShellModel};
use serde_json::json;

const MM: i64 = 1_000_000;

fn panel(shell: &ShellModel) -> std::sync::Arc<PanelTables> {
    shell.panel_tables().expect("the panel tables")
}

/// The sheet point of a page point of the page shown.
fn sheet(shell: &ShellModel, p: (i64, i64)) -> (i64, i64) {
    shell.canvas().unwrap().page().snapper.page_to_sheet(p)
}

/// The placement of the device tagged `tag`.
fn placed(shell: &ShellModel, tag: &str) -> Uuid {
    panel(shell)
        .placements
        .iter()
        .find(|p| p.tag == tag)
        .unwrap_or_else(|| panic!("{tag} is not placed"))
        .id
}

/// The texts the page on show prints.
fn texts(shell: &ShellModel) -> Vec<String> {
    use ew_viewmodel::ew_api::output::ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => out.push(run.text.clone()),
                ItemKind::Group(group) => walk(&group.items, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&shell.canvas().unwrap().page().list.items, &mut out);
    out
}

#[test]
fn a_balloon_off_its_device_drags_alone_and_the_devices_menu_puts_it_back() {
    let tmp = Tmp::new("layout-label-drag");
    let mut shell = tmp.shell_in(Mode::Headless, true, "902");
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/derived.labels", "value": "Item numbers"}),
    );
    // -101F1 is 1.8 mm wide on paper: its balloon stands above it with a leader.
    let id = placed(&shell, "-101F1");
    let row = panel(&shell).placement(id).unwrap().clone();
    let bx = row.label_box.expect("902 prints the balloon");
    let grab = sheet(
        &shell,
        (
            (bx.min().x.0 + bx.max().x.0) / 2,
            (bx.min().y.0 + bx.max().y.0) / 2,
        ),
    );
    let to = (grab.0 + 15 * MM, grab.1 - 10 * MM);
    done(&mut shell, "canvas.press", at(grab, json!({})));
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    done(&mut shell, "canvas.release", at(to, json!({})));
    let moved = panel(&shell).placement(id).unwrap().clone();
    assert!(moved.label_moved, "the label was moved");
    assert_eq!(moved.pose, row.pose, "the device stays where it is");
    let new_box = moved.label_box.unwrap();
    assert!(
        new_box.min().x.0 > bx.max().x.0,
        "the label stands right of where it was"
    );
    // A click on the moved balloon selects its device, whose menu puts the balloon back.
    let on = sheet(
        &shell,
        (
            (new_box.min().x.0 + new_box.max().x.0) / 2,
            (new_box.min().y.0 + new_box.max().y.0) / 2,
        ),
    );
    done(&mut shell, "canvas.press", at(on, json!({})));
    done(&mut shell, "canvas.release", at(on, json!({})));
    let menu: Vec<&str> = canvas_menu(&shell, MenuSpot::Object, None)
        .iter()
        .map(|i| i.id)
        .collect();
    assert!(menu.contains(&"layout.reset_label"), "{menu:?}");
    done(&mut shell, "layout.reset_label", json!({}));
    let back = panel(&shell).placement(id).unwrap().clone();
    assert!(!back.label_moved);
    assert_eq!(back.label_box, Some(bx), "back where the label rules put it");
    shell.shutdown();
}

#[test]
fn a_new_layout_page_at_one_to_ten_numbers_its_devices_and_gets_its_legend() {
    let tmp = Tmp::new("layout-legend");
    let mut shell = tmp.shell_in(Mode::Headless, true, "902");
    // Golden A's 902 holds its legend, printed with the kind's columns.
    assert!(!texts(&shell).iter().any(|t| t == "Description"));
    assert_eq!(
        refused(&mut shell, "page.add_legend", json!({})),
        "reason-legend-exists"
    );
    let surface = panel(&shell).surface.to_string();
    done(
        &mut shell,
        "page.new",
        json!({"folio": "980", "type": "panel_layout", "title": "Cabinet"}),
    );
    done(&mut shell, "page.goto", json!({"folio": "980"}));
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/derived.surface", "value": surface}),
    );
    let t = panel(&shell);
    assert!(t.numbered, "at 1:10 the devices carry item numbers");
    assert_eq!(
        refused(&mut shell, "page.add_legend", json!({})),
        "reason-legend-exists",
        "the new page got its legend with the view"
    );
    // The legend a person gets prints the description too; Golden A's legend on 902 keeps its four columns.
    assert!(
        texts(&shell).iter().any(|t| t == "Description"),
        "the new legend's Description"
    );
    // The page has no location of its own: the cabinet's is not printed before every tag.
    assert!(
        t.placements.iter().all(|p| !p.label.contains('+')),
        "{:?}",
        t.placements.iter().map(|p| p.label.clone()).collect::<Vec<_>>()
    );
    shell.shutdown();
}
