// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text under the eight symbol variants (doc 07 §4.1, ID-49): the anchor transforms, glyphs are never
//! mirrored, and the alignment flips as the readability turn and the mirror require.

use ew_dl::Transform;
use ew_scene::symbol::{TextSource, lower};
use ew_schema::{Align, LibDocument, SymbolDef};
use ew_units::{Angle2, Nm, Orient2, Pt2};
use serde_json::{Value, json};

/// A starter symbol with its text slots replaced by one top-left tag slot at (4 mm, 2 mm), turned by `rot`.
fn symbol_with_slot(rot: i32, align: &str) -> SymbolDef {
    let mut doc: Value =
        serde_json::from_str(include_str!("../../../../content/symbols/ew.iec.mcb-1pn.json"))
            .expect("starter symbol json");
    doc["text_slots"] = json!([{
        "kind": "tag",
        "pos": [4_000_000, 2_000_000],
        "rot": rot,
        "align": align,
        "visible": true
    }]);
    doc["arrangements"] = json!([{"name": "full", "slots": [0]}]);
    doc["anchor"] = json!("origin");
    // Static texts of the graphics would add entries; the starter symbol has none.
    match LibDocument::from_json(&doc.to_string()).expect("symbol document") {
        LibDocument::Symbol(copy) => copy.item,
        other => panic!("not a symbol: {other:?}"),
    }
}

#[test]
fn alignment_and_angle_per_variant_for_a_top_left_slot() {
    // Variant, rendered angle, alignment of a TopLeft text (doc 07 §4.1 table: horizontal flips with the
    // turn, vertical flips when exactly one of turn and mirror applies).
    let table = [
        (Orient2::R0, 0, Align::TopLeft),
        (Orient2::R90, 90_000, Align::BottomRight),
        (Orient2::R180, 0, Align::BottomRight),
        (Orient2::R270, 90_000, Align::TopLeft),
        (Orient2::M0, 0, Align::TopRight),
        (Orient2::M90, 90_000, Align::BottomLeft),
        (Orient2::M180, 0, Align::BottomLeft),
        (Orient2::M270, 90_000, Align::TopRight),
    ];
    let def = symbol_with_slot(0, "top_left");
    for (variant, angle, align) in table {
        let g = lower(&def, variant);
        assert_eq!(g.texts.len(), 1, "{variant:?}");
        let t = &g.texts[0];
        assert_eq!(t.source, TextSource::Slot(ew_schema::SlotKind::Tag));
        assert_eq!(t.angle, Angle2(angle), "{variant:?}");
        assert_eq!(t.align, align, "{variant:?}");
    }
}

#[test]
fn centre_and_middle_alignments_flip_only_on_their_axis() {
    // E (plain mirror): left <-> right, vertical stays; centre stays everywhere except under a vertical flip.
    let cases = [
        ("middle_left", Orient2::M0, Align::MiddleRight),
        ("middle_right", Orient2::M0, Align::MiddleLeft),
        ("centre", Orient2::M0, Align::Centre),
        ("top_centre", Orient2::M0, Align::TopCentre),
        ("bottom_left", Orient2::M0, Align::BottomRight),
        ("middle_left", Orient2::R180, Align::MiddleRight),
        ("top_centre", Orient2::R180, Align::BottomCentre),
        ("middle_left", Orient2::M90, Align::MiddleLeft),
        ("top_left", Orient2::M90, Align::BottomLeft),
    ];
    for (align, variant, want) in cases {
        let g = lower(&symbol_with_slot(0, align), variant);
        assert_eq!(g.texts[0].align, want, "{align} {variant:?}");
    }
}

#[test]
fn mirrored_variants_place_the_text_on_the_mirrored_side_without_mirroring_glyphs() {
    let def = symbol_with_slot(0, "top_left");
    let anchor = Pt2::new(Nm(4_000_000), Nm(2_000_000));
    let r0 = lower(&def, Orient2::R0);
    assert_eq!(r0.texts[0].pos, anchor);
    let m0 = lower(&def, Orient2::M0);
    // The anchor is mirrored in x: the text sits on the mirrored side of the symbol.
    assert_eq!(m0.texts[0].pos, Pt2::new(Nm(-anchor.x.0), anchor.y));
    // Left-aligned text right of the symbol becomes right-aligned left of it: it still extends away from
    // the symbol.
    assert_eq!(m0.texts[0].align, Align::TopRight);
    // The glyphs are not mirrored: the placed text carries a rotation only, and the run transform built
    // from it is a pure rotation (no mirror) that maps +x to a direction reading left to right or upward.
    for variant in Orient2::ALL {
        let t = &lower(&def, variant).texts[0];
        let run = Transform::from_angle(t.angle);
        assert!(!run.orient.is_mirrored(), "{variant:?}");
        let along = run.apply(Pt2::new(Nm(1_000_000), Nm::ZERO));
        // Baseline direction: right (0 degrees) or up (90 degrees), never left or down.
        assert!(along.x.0 >= 0 && along.y.0 <= 0, "{variant:?}: {along:?}");
        // A proper rotation keeps the orientation of (right, up): with y down the cross product of their
        // images stays negative, as for the identity (a mirror would make it positive).
        let up = run.apply(Pt2::new(Nm::ZERO, Nm(-1_000_000)));
        let det = i128::from(along.x.0) * i128::from(up.y.0) - i128::from(along.y.0) * i128::from(up.x.0);
        assert!(det < 0, "{variant:?}: det {det}");
    }
}
