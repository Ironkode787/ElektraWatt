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
use support::{Scripted, Tmp, assert_named, harness, settle};

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
    // On a schematic page it arms the tool.
    dispatch(&mut h, "page.goto", json!({"folio": "102"}));
    settle(&mut h);
    h.get_by_label("Insert: Interruption point").click();
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
    assert!(group(&h, "Geen term gevonden"), "the empty state");
    dispatch(&mut h, "view.language", json!({"value": "en"}));
    settle(&mut h);
    assert_named(&h);
    h.state_mut().shell_mut().shutdown();
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
            .map(|s| usize::from(s.cased) + if s.colours.len() == 1 { 1 } else { s.pieces().len() })
            .sum();
        assert_eq!(shapes.len(), count);
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
