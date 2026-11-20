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
use ew_units::Rect;
use ew_api::output::{PanelTables, Uuid};
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
    use ew_dl::{Item, ItemKind};
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

/// The persona round of the second 0.3.0 candidate: two clicks and three drags on an item number only selected its
/// device, and the menu had no Reset. With its placement selected, a balloon inside the footprint drags alone (its
/// handle shows, the device stays); the placement's menu offers Reset label position greyed with its reason until the
/// label moved, and the page's menu puts every moved label back in one undo step.
#[test]
fn a_selected_placements_balloon_drags_alone_and_the_pages_menu_puts_labels_back() {
    let tmp = Tmp::new("layout-label-inside");
    let mut shell = tmp.shell_in(Mode::Headless, true, "902");
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/derived.labels", "value": "Item numbers"}),
    );
    // A placement whose balloon stands inside its footprint.
    let t = panel(&shell);
    let row = t
        .placements
        .iter()
        .find(|p| {
            let (Some(bx), fp) = (p.label_box, p.footprint) else {
                return false;
            };
            let (a, b) = (
                t.to_page((i128::from(fp.min().x.0), i128::from(fp.min().y.0))),
                t.to_page((i128::from(fp.max().x.0), i128::from(fp.max().y.0))),
            );
            a.0.min(b.0) <= bx.min().x.0
                && bx.max().x.0 <= a.0.max(b.0)
                && a.1.min(b.1) <= bx.min().y.0
                && bx.max().y.0 <= a.1.max(b.1)
        })
        .expect("a balloon inside its device")
        .clone();
    let bx = row.label_box.unwrap();
    let grab = sheet(
        &shell,
        (
            (bx.min().x.0 + bx.max().x.0) / 2,
            (bx.min().y.0 + bx.max().y.0) / 2,
        ),
    );
    // A click on the balloon selects the device; its menu greys Reset label position with the reason.
    common::click(&mut shell, grab, json!({}));
    assert_eq!(shell.selected_placements(), 1);
    let reset = |shell: &ShellModel| {
        canvas_menu(shell, MenuSpot::Object, None)
            .into_iter()
            .find(|i| i.id == "layout.reset_label")
            .map(|i| (i.enabled, i.reason))
    };
    assert_eq!(
        reset(&shell),
        Some((
            false,
            Some("The label stands where the label rules put it".to_owned())
        ))
    );
    assert_eq!(shell.tool_overlay().len(), 2, "the label's box and its handle");
    // A drag that starts on the balloon moves the balloon alone.
    let to = (grab.0 + 20 * MM, grab.1 - 15 * MM);
    done(&mut shell, "canvas.pointer", at(grab, json!({})));
    assert!(shell.label_under_pointer(), "the grab pointer");
    done(&mut shell, "canvas.press", at(grab, json!({})));
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    done(&mut shell, "canvas.release", at(to, json!({})));
    let moved = panel(&shell).placement(row.id).unwrap().clone();
    assert!(moved.label_moved, "the label was moved");
    assert_eq!(moved.pose, row.pose, "the device stays where it is");
    assert_eq!(reset(&shell).map(|r| r.0), Some(true));
    // The page's menu puts it back, one undo step.
    done(&mut shell, "select.set", json!({"ids": []}));
    let page_menu: Vec<&str> = canvas_menu(&shell, MenuSpot::Sheet, None)
        .iter()
        .map(|i| i.id)
        .collect();
    assert!(page_menu.contains(&"layout.reset_labels"), "{page_menu:?}");
    let rev = shell.revision();
    done(&mut shell, "layout.reset_labels", json!({}));
    assert_ne!(shell.revision(), rev);
    assert!(panel(&shell).placements.iter().all(|p| !p.label_moved));
    assert_eq!(
        refused(&mut shell, "layout.reset_labels", json!({})),
        "reason-labels-not-moved"
    );
    done(&mut shell, "edit.undo", json!({}));
    assert!(panel(&shell).placement(row.id).unwrap().label_moved);
    shell.shutdown();
}

/// The glyph runs the page on show prints inside the page rectangle `r`, in paint order.
fn texts_in(shell: &ShellModel, r: Rect) -> Vec<String> {
    use ew_dl::{Item, ItemKind};
    fn walk(items: &[Item], r: Rect, out: &mut Vec<String>) {
        for item in items {
            match &item.kind {
                ItemKind::Glyphs(run) => {
                    let (x, y) = (run.origin.x, run.origin.y);
                    if r.min().x <= x && x <= r.max().x && r.min().y <= y && y <= r.max().y {
                        out.push(run.text.clone());
                    }
                }
                ItemKind::Group(group) => walk(&group.items, r, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&shell.canvas().unwrap().page().list.items, r, &mut out);
    out
}

/// Golden A's plate on a new layout page 980, which gets its legend with the view.
fn new_layout_page(shell: &mut ShellModel) {
    let surface = panel(shell).surface.to_string();
    done(
        shell,
        "page.new",
        json!({"folio": "980", "type": "panel_layout", "title": "Cabinet"}),
    );
    done(shell, "page.goto", json!({"folio": "980"}));
    done(
        shell,
        "props.set",
        json!({"row": "page/derived.surface", "value": surface}),
    );
}

/// The persona round of the second 0.3.0 candidate: the legend Add legend made stood over the frame's column ruler
/// as a narrow strip with every cell cut, and could not be clicked. The legend of a new layout page lies inside the
/// frame's drawing area, is one table across its block, prints every position and every tag whole, and can be
/// selected, dragged and resized by a corner, each one undo step.
#[test]
fn the_legend_beside_a_layout_is_a_lookup_table_that_moves_and_resizes() {
    let tmp = Tmp::new("layout-legend-table");
    let mut shell = tmp.shell_in(Mode::Headless, true, "902");
    new_layout_page(&mut shell);
    let t = panel(&shell);
    assert!(t.numbered);
    assert!(
        !common::toast(&shell).contains("show their tags"),
        "a legend beside numbered devices offers no switch: {}",
        common::toast(&shell)
    );
    let [block] = t.blocks.as_slice() else {
        panic!("one legend: {:?}", t.blocks)
    };
    let [a0, b0, a1, b1] =
        ew_viewmodel::tools::ip::drawing_area(shell.canvas().unwrap().page()).expect("a drawing area");
    let r = block.rect;
    assert!(
        a0 <= r.min().x.0 && r.max().x.0 <= a1 && b0 <= r.min().y.0 && r.max().y.0 <= b1,
        "the legend {r:?} inside the drawing area ({a0}, {b0})–({a1}, {b1})"
    );
    assert!(r.width().0 >= 60 * MM, "{r:?}");
    let printed = texts_in(&shell, block.drawn);
    // One table: the header once, every position whole, every tag whole.
    assert_eq!(printed.iter().filter(|t| *t == "Pos").count(), 1, "{printed:?}");
    let rows = printed.iter().filter(|t| t.parse::<u32>().is_ok()).count();
    assert!(rows >= 20, "{printed:?}");
    assert!(
        printed
            .iter()
            .filter(|t| t.starts_with('='))
            .all(|t| !t.ends_with('\u{2026}')),
        "{printed:?}"
    );
    // A click on the legend selects it and shows its corners.
    let centre = sheet(
        &shell,
        (
            (block.drawn.min().x.0 + block.drawn.max().x.0) / 2,
            (block.drawn.min().y.0 + block.drawn.max().y.0) / 2,
        ),
    );
    common::click(&mut shell, centre, json!({}));
    assert_eq!(shell.selection().items.len(), 1);
    assert_eq!(shell.selection().items[0].0, block.id);
    assert_eq!(shell.tool_overlay().len(), 4, "a grip at each corner");
    // A drag moves it 10 mm left.
    let to = (centre.0 - 10 * MM, centre.1);
    done(&mut shell, "canvas.press", at(centre, json!({})));
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    done(&mut shell, "canvas.release", at(to, json!({})));
    let moved = panel(&shell).block(block.id).copied().expect("the legend");
    assert_eq!(moved.rect.min().x.0, r.min().x.0 - 10 * MM);
    assert_eq!(moved.rect.width(), r.width());
    // Its bottom right corner, dragged 15 mm left, makes it narrower.
    let corner = sheet(&shell, (moved.drawn.max().x.0, moved.drawn.max().y.0));
    let to = (corner.0 - 15 * MM, corner.1);
    done(&mut shell, "canvas.press", at(corner, json!({})));
    done(&mut shell, "canvas.pointer", at(to, json!({})));
    done(&mut shell, "canvas.release", at(to, json!({})));
    let resized = panel(&shell).block(block.id).copied().expect("the legend");
    assert_eq!(resized.rect.width().0, r.width().0 - 15 * MM);
    assert_eq!(resized.rect.min(), moved.rect.min());
    done(&mut shell, "edit.undo", json!({}));
    done(&mut shell, "edit.undo", json!({}));
    assert_eq!(panel(&shell).block(block.id).map(|b| b.rect), Some(r));
    shell.shutdown();
}

/// The persona round of the second 0.3.0 candidate: at 1:5 nine small devices printed their tags over each other. A
/// new layout view looks at whether the tags of the placements fit their footprints, not only at the scale: Golden
/// A's terminals are about a millimetre wide at 1:5, so the view starts with item numbers.
#[test]
fn a_new_layout_view_at_one_to_five_numbers_tags_that_do_not_fit() {
    let tmp = Tmp::new("layout-numbers-fit");
    let mut shell = tmp.shell_in(Mode::Headless, true, "902");
    let surface = panel(&shell).surface.to_string();
    done(
        &mut shell,
        "page.new",
        json!({"folio": "981", "type": "panel_layout", "title": "Cabinet"}),
    );
    done(&mut shell, "page.goto", json!({"folio": "981"}));
    done(&mut shell, "select.set", json!({"ids": []}));
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/scale", "value": "1:5"}),
    );
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/derived.surface", "value": surface}),
    );
    assert!(panel(&shell).numbered, "item numbers at 1:5");
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

/// The persona round of the second 0.3.0 candidate found the Labels choice only far down the page's Properties. The
/// layout page's menu offers Labels: Tags, Item numbers and Both, the current one checked, each one undo step; a
/// legend added beside tags offers to show the item numbers it lists.
#[test]
fn the_pages_menu_chooses_the_labels_and_a_legend_beside_tags_offers_numbers() {
    let tmp = Tmp::new("layout-labels-menu");
    let mut shell = tmp.shell_in(Mode::Headless, true, "902");
    new_layout_page(&mut shell);
    let checked = |shell: &ShellModel| -> Vec<String> {
        canvas_menu(shell, MenuSpot::Sheet, None)
            .into_iter()
            .filter(|i| i.id == "layout.labels" && i.checked)
            .map(|i| i.label)
            .collect()
    };
    assert_eq!(checked(&shell), ["Labels: Item numbers"]);
    done(&mut shell, "layout.labels", json!({"show": "tags"}));
    assert!(!panel(&shell).numbered);
    assert_eq!(checked(&shell), ["Labels: Tags"]);
    // The legend goes, and comes back beside the tags: its toast offers the numbers.
    let block = panel(&shell).blocks[0].id;
    done(&mut shell, "select.set", json!({"ids": [block.to_string()]}));
    done(&mut shell, "edit.delete_selection", json!({}));
    assert!(panel(&shell).blocks.is_empty());
    done(&mut shell, "page.add_legend", json!({}));
    let toast = shell.notifications().newest_visible().expect("the toast").clone();
    assert_eq!(
        toast.text,
        "Legend added. The devices show their tags, not the numbers it lists."
    );
    done(
        &mut shell,
        "notifications.run",
        json!({"entry": toast.serial, "action": "item_numbers"}),
    );
    assert!(panel(&shell).numbered);
    assert_eq!(checked(&shell), ["Labels: Item numbers"]);
    done(&mut shell, "edit.undo", json!({}));
    assert!(!panel(&shell).numbered, "one undo step");
    shell.shutdown();
}
