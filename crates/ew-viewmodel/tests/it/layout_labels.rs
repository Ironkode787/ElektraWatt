// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A layout page that can be read (at 1:10 the tags must not run into each other: numbers, a table beside the layout
//! and labels that move), through the shell on temporary copies of the sample: a balloon off its device drags alone and
//! the device's menu puts it back; a new layout view prints tags where they fit and balloons where they do not, gets
//! its legend page in the same step and prints its tags without the cabinet's location; a legend box beside a layout
//! moves and resizes; Add legend is refused where the layout has its legend.

use crate::common;

use common::{Tmp, at, done, refused};
use ew_api::output::PanelTables;
use ew_units::Rect;
use ew_viewmodel::command_bar::{MenuSpot, canvas_menu};
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

/// Whether the page point `p` lies in the footprint `fp` of the page's panel tables `t`.
fn in_footprint(t: &PanelTables, fp: Rect, p: (i64, i64)) -> bool {
    let (a, b) = (
        t.to_page((i128::from(fp.min().x.0), i128::from(fp.min().y.0))),
        t.to_page((i128::from(fp.max().x.0), i128::from(fp.max().y.0))),
    );
    a.0.min(b.0) <= p.0 && p.0 <= a.0.max(b.0) && a.1.min(b.1) <= p.1 && p.1 <= a.1.max(b.1)
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
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    // At 1:20 the sample's devices are narrow on paper, so their balloons stand off them.
    done(&mut shell, "select.set", json!({"ids": []}));
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/scale", "value": "1:20"}),
    );
    done(
        &mut shell,
        "props.set",
        json!({"row": "page/derived.labels", "value": "Item numbers"}),
    );
    // A narrow placement whose balloon stands off it with a leader.
    let t = panel(&shell);
    let row = t
        .placements
        .iter()
        .find(|p| {
            p.label_box.is_some_and(|bx| {
                !in_footprint(
                    &t,
                    p.footprint,
                    (
                        (bx.min().x.0 + bx.max().x.0) / 2,
                        (bx.min().y.0 + bx.max().y.0) / 2,
                    ),
                )
            })
        })
        .expect("a balloon off its device")
        .clone();
    let id = row.id;
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

/// Two clicks and three drags on an item number only selected its
/// device, and the menu had no Reset. With its placement selected, a balloon inside the footprint drags alone (its
/// handle shows, the device stays); the placement's menu offers Reset label position greyed with its reason until the
/// label moved, and the page's menu puts every moved label back in one undo step.
#[test]
fn a_selected_placements_balloon_drags_alone_and_the_pages_menu_puts_labels_back() {
    let tmp = Tmp::new("layout-label-inside");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
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
            in_footprint(&t, fp, (bx.min().x.0, bx.min().y.0))
                && in_footprint(&t, fp, (bx.max().x.0, bx.max().y.0))
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

/// The sample's plate on a new layout page 980 at 1:10 through the page's Surface row: a new view, which gets its legend
/// page with it.
fn new_layout_page(shell: &mut ShellModel) {
    let surface = panel(shell).surface.to_string();
    done(
        shell,
        "page.new",
        json!({"folio": "980", "type": "panel_layout", "title": "Cabinet"}),
    );
    done(shell, "page.goto", json!({"folio": "980"}));
    done(shell, "select.set", json!({"ids": []}));
    done(shell, "props.set", json!({"row": "page/scale", "value": "1:10"}));
    done(
        shell,
        "props.set",
        json!({"row": "page/derived.surface", "value": surface}),
    );
}

/// The legend Add legend made stood over the frame's column ruler as a narrow strip with every cell cut, and could not be
/// clicked. A legend box beside a layout (as a project made before 0.3.5 holds one, or as Add legend puts one where it
/// fits whole) lies inside the frame's drawing area, is one table across its block, prints every position and every
/// tag whole, and can be selected, dragged and resized by a corner, each one undo step.
#[test]
fn the_legend_beside_a_layout_is_a_lookup_table_that_moves_and_resizes() {
    let tmp = Tmp::new("layout-legend-table");
    let (mut shell, _) = common::legend_box(&tmp);
    let t = panel(&shell);
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
    // One table: the header once; the enclosure's and the plate's rows, their positions whole, no text cut.
    assert_eq!(printed.iter().filter(|t| *t == "Pos").count(), 1, "{printed:?}");
    assert!(
        ["1", "2"].iter().all(|p| printed.iter().any(|t| t == p)),
        "{printed:?}"
    );
    assert!(printed.iter().all(|t| !t.ends_with('\u{2026}')), "{printed:?}");
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

/// At 1:5 nine small devices printed their tags over each other. A new layout view prints each device's tag where it
/// fits inside the device and its item number in a balloon where it does not (doc 12 §5.3), whatever the scale: the
/// sample's terminals, about a millimetre wide at 1:5, take balloons.
#[test]
fn a_new_layout_view_prints_tags_where_they_fit_and_balloons_where_they_do_not() {
    let tmp = Tmp::new("layout-numbers-fit");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
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
    let t = panel(&shell);
    assert_eq!(t.show, ew_api::output::LabelShow::TagsOrNumbers);
    assert!(t.numbered, "numbers where tags do not fit");
    shell.shutdown();
}

/// The legend of a layout is a list page of its own right after it (doc 12 §5.4): the sample's 902 has its legend on
/// 902.1 and is offered no second one, nor is a second page of its plate, which prints its tags without the cabinet's
/// location; Enclosure… on a new page gives the new layout its legend page in the undo step of its view, titled after
/// its layout and printing the description.
#[test]
fn a_new_layout_view_gets_its_legend_page_in_the_same_undo_step() {
    let tmp = Tmp::new("layout-legend");
    let mut shell = tmp.sample_in(Mode::Headless, true, "902");
    assert_eq!(
        refused(&mut shell, "page.add_legend", json!({})),
        "reason-legend-paged"
    );
    new_layout_page(&mut shell);
    let t = panel(&shell);
    assert_eq!(
        refused(&mut shell, "page.add_legend", json!({})),
        "reason-legend-paged",
        "the plate has its legend on 902.1"
    );
    // The page has no location of its own: the cabinet's is not printed before every tag.
    assert!(
        t.placements.iter().all(|p| !p.label.contains('+')),
        "{:?}",
        t.placements.iter().map(|p| p.label.clone()).collect::<Vec<_>>()
    );
    // A new enclosure on a new page: its view and its legend page.
    done(
        &mut shell,
        "page.new",
        json!({"folio": "981", "type": "panel_layout", "title": "Field"}),
    );
    done(&mut shell, "page.goto", json!({"folio": "981"}));
    done(
        &mut shell,
        "layout.enclosure_new",
        json!({"location": "+FLD", "part": "EW-GEN-ENC-1"}),
    );
    let legend = shell
        .page_rows()
        .iter()
        .find(|r| r.folio == "981.1")
        .cloned()
        .expect("the legend page 981.1");
    assert_eq!(legend.title, "Legend of layout 981");
    done(&mut shell, "page.goto", json!({"folio": "981.1"}));
    assert!(
        texts(&shell).iter().any(|t| t == "Description"),
        "the legend's Description"
    );
    // One undo takes back the view and its legend page; the enclosure stays.
    done(&mut shell, "page.goto", json!({"folio": "981"}));
    done(&mut shell, "edit.undo", json!({}));
    assert!(shell.page_rows().iter().all(|r| r.folio != "981.1"));
    assert!(shell.panel_tables().is_none(), "the page shows no plate");
    shell.shutdown();
}

/// The Labels choice is not found only far down the page's Properties. The layout page's menu offers Labels: Tags, Item
/// numbers, Both and Tags or numbers, the current one checked, each one undo step; a legend added beside tags offers to
/// show the item numbers it lists.
#[test]
fn the_pages_menu_chooses_the_labels_and_a_legend_beside_tags_offers_numbers() {
    let tmp = Tmp::new("layout-labels-menu");
    let (mut shell, legend) = common::legend_box(&tmp);
    let checked = |shell: &ShellModel| -> Vec<String> {
        canvas_menu(shell, MenuSpot::Sheet, None)
            .into_iter()
            .filter(|i| i.id == "layout.labels" && i.checked)
            .map(|i| i.label)
            .collect()
    };
    assert_eq!(checked(&shell), ["Labels: Tags or numbers"]);
    done(&mut shell, "layout.labels", json!({"show": "tags"}));
    assert!(!panel(&shell).numbered);
    assert_eq!(checked(&shell), ["Labels: Tags"]);
    // The legend goes, and comes back beside the tags: its toast offers the numbers.
    done(&mut shell, "select.set", json!({"ids": [legend]}));
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
