// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.08 part 6 in the window: the glossary pane in the bottom dock with its Insert button (GT-1) and the
//! accessible-name walker over it (GT-5), and the wire-colour view, which the PDF export never sees (GT-4).

use crate::support;

use std::path::Path;
use std::sync::Arc;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable};
use ew_render2d::view::DrawTransform;
use ew_ui::app::App;
use ew_ui::ew_viewmodel::shell::Outcome;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::{Scripted, Tmp, assert_accessible, harness, settle};

fn dispatch(h: &mut Harness<'_, App>, action: &str, params: Value) {
    assert_eq!(
        h.state_mut().shell_mut().dispatch(action, &params),
        Outcome::Done,
        "{action} {params}"
    );
}

/// Whether a `Group` node is named `name`.
fn group(h: &Harness<'_, App>, name: &str) -> bool {
    h.get_all_by_label(name)
        .any(|n| n.accesskit_node().role() == egui::accesskit::Role::Group)
}

#[test]
fn gt1_gt5_the_glossary_pane_is_named_and_folio_reports_insert_arms_the_ip_tool_in_english_and_dutch() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "help.glossary", json!({}));
    dispatch(&mut h, "glossary.filter", json!({"text": "folio report"}));
    settle(&mut h);
    assert!(group(&h, "Glossary"), "the pane");
    assert!(group(&h, "Interruption point"), "the row");
    let row = h
        .get_all_by_label("Interruption point")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Group)
        .unwrap();
    assert_eq!(
        row.accesskit_node().description().as_deref(),
        Some(
            "EPLAN: Interruption point; QET: Folio report; Where a potential leaves one page and continues on \
             another; pairs are found by name, star or chain."
        ),
        "its other cells, for a screen reader"
    );
    // On the title page the IP tool cannot be armed: the button is disabled.
    let insert = h.get_by_label("Insert: Interruption point");
    assert!(insert.accesskit_node().is_disabled());
    // C49-p8#6: its reason, as the menus give it, beside the button.
    insert.hover();
    settle(&mut h);
    settle(&mut h);
    assert!(
        h.query_by_label("A title page takes no symbols or wires: draw on a schematic page")
            .is_some(),
        "the disabled button's reason"
    );
    // On a schematic page it arms the tool.
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    settle(&mut h);
    // The pointer still rests on it: its tooltip names it too.
    h.get_all_by_label("Insert: Interruption point")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
        .unwrap()
        .click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "ip");
    dispatch(&mut h, "tool.select", json!({}));
    // In Dutch.
    dispatch(&mut h, "view.language", json!({"value": "nl"}));
    dispatch(&mut h, "glossary.filter", json!({"text": "onderbrekingspunt"}));
    settle(&mut h);
    assert!(group(&h, "Woordenlijst"), "the pane in Dutch");
    assert!(group(&h, "Onderbrekingspunt"), "the row in Dutch");
    h.get_by_label("Invoegen: Onderbrekingspunt").click();
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "ip");
    // No term: the empty state.
    dispatch(&mut h, "glossary.filter", json!({"text": "zzqx"}));
    settle(&mut h);
    // C49-p8#7: the search quoted, and Clear, as in the navigators.
    assert!(group(&h, "Niets komt overeen met \"zzqx\""), "the empty state");
    h.get_by_label("Filter wissen").click();
    settle(&mut h);
    assert_eq!(h.state().shell().glossary_filter(), "");
    assert!(group(&h, "Onderbrekingspunt"), "every term again");
    dispatch(&mut h, "view.language", json!({"value": "en"}));
    settle(&mut h);
    assert_accessible(&h);
    h.state_mut().shell_mut().shutdown();
}

/// The glossary pane's search field.
fn field<'h>(h: &'h Harness<'_, App>) -> egui_kittest::Node<'h> {
    h.get_all_by_label("Glossary")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Group)
        .unwrap()
        .get_by_role(egui::accesskit::Role::TextInput)
}

/// FT-1281: in the glossary's field Down and Up move a row cursor, its row alone selected for AccessKit, a new search
/// starts it again, and Enter runs the cursor row's first enabled action (Insert arms the IP tool).
#[test]
fn ft1281_the_glossary_has_a_keyboard_row_cursor() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    dispatch(&mut h, "help.glossary", json!({}));
    settle(&mut h);
    let selected = |h: &Harness<'_, App>| -> Vec<String> {
        h.root()
            .children_recursive()
            .filter(|n| {
                let n = n.accesskit_node();
                n.role() == egui::accesskit::Role::Group && n.is_selected() == Some(true)
            })
            .map(|n| n.accesskit_node().label().unwrap_or_default())
            .collect()
    };
    field(&h).click();
    settle(&mut h);
    assert_eq!(h.state().shell().glossary_row(), None);
    assert_eq!(selected(&h), Vec::<String>::new());
    for _ in 0..2 {
        h.key_press(egui::Key::ArrowDown);
        settle(&mut h);
    }
    assert_eq!(h.state().shell().glossary_row(), Some(1));
    let second = h.state().shell().glossary_rows()[1].term.clone();
    assert_eq!(selected(&h), [second]);
    h.key_press(egui::Key::ArrowUp);
    h.key_press(egui::Key::ArrowUp);
    settle(&mut h);
    assert_eq!(h.state().shell().glossary_row(), Some(0), "kept at the first row");
    // A search starts the cursor again; Down and Enter run the row's Insert.
    dispatch(&mut h, "glossary.filter", json!({"text": "folio report"}));
    settle(&mut h);
    assert_eq!(h.state().shell().glossary_row(), None);
    field(&h).click();
    settle(&mut h);
    h.key_press(egui::Key::ArrowDown);
    settle(&mut h);
    assert_eq!(selected(&h), ["Interruption point"]);
    h.key_press(egui::Key::Enter);
    settle(&mut h);
    assert_eq!(h.state().shell().tool().name(), "ip");
    h.state_mut().shell_mut().shutdown();
}

/// C49-p8#8: a cased stroke's casing is painted first, `CASING_PX` physical pixel wider on each side, in the casing
/// colour; then the stroke in its colour.
#[test]
fn a_cased_strokes_casing_is_painted_first_one_pixel_wider_on_each_side() {
    use ew_ui::ew_viewmodel::wire_colours::{CASING_RGB, STROKE_PT, WireStroke};
    let white = WireStroke::new(
        ew_ui::ew_viewmodel::ew_api::output::Uuid::nil(),
        vec![(10_000_000, 10_000_000), (20_000_000, 10_000_000)],
        (420_000_000, 297_000_000),
        &["WH".to_owned()],
        &[],
    );
    assert!(white.cased);
    let t = DrawTransform {
        dpi: 96.0,
        origin: (0.0, 0.0),
        raster_dpi: None,
    };
    let ppp = 2.0;
    let shapes = ew_ui::canvas::wire_colour_shapes(&[white], egui::Pos2::ZERO, ppp, &t);
    let strokes: Vec<(f32, egui::Color32)> = shapes
        .iter()
        .map(|s| match s {
            egui::Shape::Path(p) => match &p.stroke.color {
                egui::epaint::ColorMode::Solid(c) => (p.stroke.width, *c),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect();
    let (r, g, b) = CASING_RGB;
    assert_eq!(
        strokes,
        [
            (STROKE_PT as f32 + 2.0 / ppp, egui::Color32::from_rgb(r, g, b)),
            (STROKE_PT as f32, egui::Color32::WHITE),
        ]
    );
}

/// W49.2 F8: at the 1280 × 800 floor in Dutch the term column is as wide as "Onderbrekingspunt" needs, so the term
/// wraps between words, never inside one; the pane's rows fit.
#[test]
fn f8_the_dutch_term_keeps_its_longest_word_whole_at_the_floor() {
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    h.set_size(egui::Vec2::new(1280.0, 800.0));
    settle(&mut h);
    dispatch(&mut h, "view.language", json!({"value": "nl"}));
    dispatch(&mut h, "help.glossary", json!({}));
    dispatch(&mut h, "glossary.filter", json!({"text": "onderbrekingspunt"}));
    settle(&mut h);
    assert!(group(&h, "Onderbrekingspunt"));
    support::assert_fits(&h);
    // The column rule: the word's width with the cell padding, at the pane's width.
    let pane = h
        .get_all_by_label("Woordenlijst")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Group)
        .unwrap()
        .accesskit_node()
        .raw_bounds()
        .unwrap();
    let t = ew_ui::theme::current(&h.ctx);
    let strong = ew_ui::theme::TypeToken {
        semibold: true,
        ..t.type_small
    };
    let painter = egui::Painter::new(h.ctx.clone(), egui::LayerId::background(), egui::Rect::EVERYTHING);
    let word = painter
        .layout_no_wrap(
            "Onderbrekingspunt".to_owned(),
            ew_ui::theme::font(&strong),
            egui::Color32::BLACK,
        )
        .size()
        .x;
    let width = (pane.x1 - pane.x0) as f32;
    let needed = word + 2.0 * t.space_4;
    assert!(
        needed > width * ew_ui::glossary::TEXT_SHARE,
        "the word needs more than the share"
    );
    let widths = ew_ui::glossary::column_widths(width, [needed, 0.0, 0.0], 100.0);
    assert!(widths[0] >= needed, "{widths:?} for {needed}");
    assert!(widths[3] > 0.0, "the definition keeps its column");
    dispatch(&mut h, "view.language", json!({"value": "en"}));
    h.state_mut().shell_mut().shutdown();
}

/// W49.2 F9: a WH and a GNYE wire on one path are drawn side by side, both colours on show; each code on its own.
#[test]
fn f9_two_wires_on_one_path_are_drawn_side_by_side() {
    use ew_ui::ew_viewmodel::wire_colours::{LANE_PT, WireStroke, assign_lanes};
    let nil = ew_ui::ew_viewmodel::ew_api::output::Uuid::nil();
    let path = vec![(10_000_000, 10_000_000), (50_000_000, 10_000_000)];
    let sheet = (420_000_000, 297_000_000);
    let mut strokes = vec![
        WireStroke::new(nil, path.clone(), sheet, &["WH".to_owned()], &[]),
        WireStroke::new(nil, path, sheet, &["GN".to_owned(), "YE".to_owned()], &[]),
    ];
    assign_lanes(&mut strokes);
    let t = DrawTransform {
        dpi: 96.0,
        origin: (0.0, 0.0),
        raster_dpi: None,
    };
    let ys = |s: &WireStroke| -> Vec<f32> {
        ew_ui::canvas::wire_colour_shapes(std::slice::from_ref(s), egui::Pos2::ZERO, 1.0, &t)
            .iter()
            .flat_map(|sh| match sh {
                egui::Shape::Path(p) => p.points.iter().map(|q| q.y).collect::<Vec<_>>(),
                egui::Shape::LineSegment { points, .. } => points.iter().map(|q| q.y).collect(),
                other => panic!("{other:?}"),
            })
            .collect()
    };
    let (white, stripes) = (ys(&strokes[0]), ys(&strokes[1]));
    let y = (10_000_000.0 * t.px_per_nm()) as f32;
    assert!(white.iter().all(|v| (v - y).abs() < 1e-3), "{white:?}");
    // The stripes stand one lane off, clear of the white stroke and its casing.
    assert!(
        stripes.iter().all(|v| (v - y).abs() >= LANE_PT as f32 - 1e-3),
        "{stripes:?}"
    );
    // Each code on its own stroke's lane.
    assert_eq!(strokes[0].midpoint().unwrap().lane, 0);
    assert_eq!(strokes[1].midpoint().unwrap().lane, 1);
}

/// FT-1277 (doc 16a §7.4): the code chips of wires whose middles fall together are placed apart, by the rule's
/// candidates and then stacked inward, inside the canvas, none faded while a free place exists; a code alone takes
/// the rule's preferred anchor, above right of its stroke's middle.
#[test]
fn ft1277_the_wire_codes_stand_apart_by_the_chip_rule() {
    use egui::{Pos2, Rect, Vec2};
    let canvas = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
    let size = Vec2::new(36.0, 16.0);
    let codes = vec![(Pos2::new(400.0, 300.0), size); 6];
    let placed = ew_ui::canvas_chips::wire_code_chips(canvas, &codes, 4.0);
    assert_eq!(placed.len(), 6);
    for (i, (a, faded)) in placed.iter().enumerate() {
        assert!(!faded, "{i} has a free place");
        assert!(canvas.contains_rect(*a), "{i} inside the canvas");
        for (b, _) in &placed[..i] {
            assert!(!a.intersects(b.shrink(0.5)), "{i}: {a:?} overlaps {b:?}");
        }
    }
    let alone = ew_ui::canvas_chips::wire_code_chips(canvas, &codes[..1], 4.0);
    let a = alone[0].0;
    assert!(
        a.left() > 400.0 && a.bottom() < 300.0,
        "above right of its anchor: {a:?}"
    );
}

/// The SHA-256 of `bytes` in hex.
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn gt4_the_exported_pdf_is_the_golden_one_with_the_wire_colour_view_on_and_off() {
    let expected = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/expected/pdf/sha256.txt"),
    )
    .unwrap();
    let tmp = Tmp::new();
    let dir = tmp.golden("g.ew");
    let mut h = harness(&tmp, Scripted::default(), Some(dir));
    settle(&mut h);
    dispatch(&mut h, "page.goto", json!({"folio": "103"}));
    settle(&mut h);
    let list = Arc::clone(&h.state().shell().canvas().unwrap().page().list);
    for (n, on) in [true, false].into_iter().enumerate() {
        dispatch(&mut h, "view.wire_colours", json!({ "on": on }));
        settle(&mut h);
        let shell = h.state().shell();
        assert_eq!(shell.wire_colours_on(), on);
        // The page's display list is the same one: the view is drawn over it, never into it.
        assert!(Arc::ptr_eq(&list, &shell.canvas().unwrap().page().list));
        let strokes = shell.wire_colour_overlay(0);
        assert_eq!(strokes.is_empty(), !on);
        // One shape per one-colour stroke, one per dash of a two-colour one, and a casing under a WH or YE one.
        let t = DrawTransform {
            dpi: 96.0,
            origin: (0.0, 0.0),
            raster_dpi: None,
        };
        let shapes = ew_ui::canvas::wire_colour_shapes(&strokes, egui::Pos2::ZERO, 1.0, &t);
        let count: usize = strokes
            .iter()
            .map(|s| {
                // FT-1283: a laned stroke is drawn on its path moved into its lanes.
                let s = s.in_lanes(1.0 / t.px_per_nm());
                let casing = usize::from(s.cased);
                let solid = s.colours.len() == 1 && !s.unresolved();
                casing + if solid { 1 } else { s.pieces().len() }
            })
            .sum();
        assert_eq!(shapes.len(), count);
        // C49-p8#4: the headless snapshot shows the view as the window does: W29's DBU only while it is on.
        let (image, _) = ew_ui::canvas::snapshot_image(shell, 0).unwrap();
        let dbu = ew_ui::ew_viewmodel::wire_colours::company_rgb(
            "DBU",
            &[ew_ui::ew_viewmodel::wire_colours::CompanyColour {
                code: "DBU".to_owned(),
                base: "BU".to_owned(),
                shade: Some("dark".to_owned()),
            }],
        )
        .unwrap();
        let painted = image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| (p[0], p[1], p[2]) == dbu)
            .count();
        assert_eq!(painted > 0, on, "DBU pixels in the snapshot, view on: {on}");
        let pdf = tmp.0.join(format!("out-{n}.pdf"));
        dispatch(
            &mut h,
            "file.export_pdf",
            json!({ "path": pdf.to_string_lossy(), "wait": true }),
        );
        h.state_mut().shell_mut().wait_jobs();
        settle(&mut h);
        let bytes = std::fs::read(&pdf).expect("the PDF is written");
        assert_eq!(sha256(&bytes), expected.trim(), "view on: {on}");
    }
    h.state_mut().shell_mut().shutdown();
}
