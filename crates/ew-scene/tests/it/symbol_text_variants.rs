// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text under the eight symbol variants (doc 07 §4.1): the anchor transforms, glyphs are never
//! mirrored or turned, and the alignment keeps the text's side of its anchor.

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
    // Variant, rendered angle, alignment of a TopLeft text (doc 07 §4.1: the text stays upright and lies on the side
    // of its anchor that the variant maps its side to; a TopLeft text lies right of and below its anchor).
    let table = [
        (Orient2::R0, 0, Align::TopLeft),
        (Orient2::R90, 0, Align::TopRight),
        (Orient2::R180, 0, Align::BottomRight),
        (Orient2::R270, 0, Align::BottomLeft),
        (Orient2::M0, 0, Align::TopRight),
        (Orient2::M90, 0, Align::BottomRight),
        (Orient2::M180, 0, Align::BottomLeft),
        (Orient2::M270, 0, Align::TopLeft),
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
fn alignments_keep_their_side_of_the_anchor() {
    // E (plain mirror): left <-> right, vertical stays; under a quarter turn a text beside its anchor goes above or
    // below it, centred.
    let cases = [
        ("middle_left", Orient2::M0, Align::MiddleRight),
        ("middle_right", Orient2::M0, Align::MiddleLeft),
        ("centre", Orient2::M0, Align::Centre),
        ("top_centre", Orient2::M0, Align::TopCentre),
        ("bottom_left", Orient2::M0, Align::BottomRight),
        ("middle_left", Orient2::R180, Align::MiddleRight),
        ("top_centre", Orient2::R180, Align::BottomCentre),
        ("middle_left", Orient2::M90, Align::BottomCentre),
        ("top_left", Orient2::M90, Align::BottomRight),
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

/// A starter symbol as it ships.
fn starter(json: &str) -> SymbolDef {
    match LibDocument::from_json(json).expect("symbol document") {
        LibDocument::Symbol(copy) => copy.item,
        other => panic!("not a symbol: {other:?}"),
    }
}

/// The alignment of the slot text of `kind` in `g`.
fn align_of(g: &ew_scene::symbol::SymbolGeometry, kind: &ew_schema::SlotKind) -> Align {
    g.texts
        .iter()
        .find(|t| t.source == TextSource::Slot(kind.clone()))
        .map(|t| t.align.clone())
        .unwrap_or_else(|| panic!("no {kind:?}"))
}

/// Turning or mirroring a symbol never turns a text (a turned part's tag and connection texts stay upright). A
/// transformer has its tag at the upper left of the body and a designation beside each of its four points.
#[test]
fn a_turned_transformer_keeps_every_text_upright_and_beside_its_point() {
    use ew_schema::SlotKind::{CpDesignation, Tag};
    let def = starter(include_str!(
        "../../../../content/symbols/ew.iec.transformer-1ph.json"
    ));
    for variant in Orient2::ALL {
        let g = lower(&def, variant);
        assert!(g.texts.iter().all(|t| t.angle == Angle2::ZERO), "{variant:?}");
    }
    let r90 = lower(&def, Orient2::R90);
    // The tag, at the upper left of the body, stands above it.
    assert_eq!(align_of(&r90, &Tag), Align::BottomLeft);
    // The points' lines leave right: the designations, right of their lines unturned, run right along them below.
    assert_eq!(align_of(&r90, &CpDesignation(0)), Align::TopLeft);
    assert_eq!(align_of(&r90, &CpDesignation(2)), Align::TopLeft);
    // Unturned and mirrored they keep their sides.
    assert_eq!(
        align_of(&lower(&def, Orient2::R0), &CpDesignation(0)),
        Align::MiddleLeft
    );
    assert_eq!(
        align_of(&lower(&def, Orient2::M0), &CpDesignation(0)),
        Align::MiddleRight
    );
}

/// An air supply turned by 270° shows its tag and its port's designation horizontally, the designation above the
/// port's line, which now leaves right.
#[test]
fn an_air_supply_turned_by_270_degrees_reads_horizontally() {
    use ew_schema::SlotKind::{CpDesignation, Tag};
    let def = starter(include_str!("../../../../content/symbols/ew.iso.air-supply.json"));
    let g = lower(&def, Orient2::R270);
    assert!(g.texts.iter().all(|t| t.angle == Angle2::ZERO));
    assert_eq!(align_of(&g, &CpDesignation(0)), Align::BottomLeft);
    assert_eq!(align_of(&g, &Tag), Align::TopCentre);
}
