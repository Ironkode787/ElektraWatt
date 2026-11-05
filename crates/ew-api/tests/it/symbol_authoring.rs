// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use ew_api::symbols::{self, SymbolDraft};
use serde_json::json;

#[test]
fn qet_element_keeps_grid_pins_and_foreign_provenance() {
    let bytes = include_bytes!("../../../ew-import-qet/tests/fixtures/contact_no.elmt");
    let d = symbols::import_qet(bytes, "contact.elmt", "example.contact").unwrap();
    assert_eq!(d.symbol.grid, symbols::units::Grid::QET_10PX);
    assert_eq!(d.symbol.cps.len(), 2);
    assert!(!d.symbol.meta.sources.is_empty());
    assert!(!d.symbol.graphics.is_empty());
    assert_eq!(
        d.function
            .cps
            .iter()
            .map(|p| p.designation.as_str())
            .collect::<Vec<_>>(),
        ["13", "14"]
    );
}

#[test]
fn qet_conversion_details_survive_a_portable_round_trip() {
    let xml = include_str!("../../../ew-import-qet/tests/fixtures/contact_no.elmt").replacen(
        "end1=\"none\"",
        "end1=\"square\"",
        1,
    );
    let draft = symbols::import_qet(xml.as_bytes(), "contact.elmt", "example.contact").unwrap();
    let finding = draft
        .import_details
        .iter()
        .find(|d| d.code == "EW-IMP-004")
        .unwrap();
    assert_eq!(
        finding.message,
        "line end square not supported, drawn without marker"
    );
    assert!(finding.line.is_some());
    let reopened = SymbolDraft::parse(&serde_json::to_vec(&draft).unwrap()).unwrap();
    assert_eq!(reopened.import_details, draft.import_details);
}

pub fn block() -> SymbolDraft {
    let mut d = serde_json::to_value(SymbolDraft::new("example.power", "Power block").unwrap()).unwrap();
    d["symbol"]["graphics"] =
        json!([{"rect":{"rect":[[500_000,0],[24_000_000,28_000_000]],"stroke":{"pen":"p026"}}}]);
    d["symbol"]["cps"] = json!(
        (1..=6)
            .map(|i| json!({"idx":i,"fn_cp":i,"pos":[0,i*4_000_000],"dir":"left"}))
            .collect::<Vec<_>>()
    );
    d["function"]["cps"] = json!(
        (1..=6)
            .map(|i| json!({"idx":i,"designation":if i==6{"FE".into()}else{i.to_string()}}))
            .collect::<Vec<_>>()
    );
    d["symbol"]["text_slots"] =
        json!([{"kind":"tag","pos":[0,-4_000_000],"align":"top_left","visible":true}]);
    SymbolDraft::parse(&serde_json::to_vec(&d).unwrap()).unwrap()
}

#[test]
fn six_pin_block_variants_roundtrip_and_canonical_publication() {
    let d = block();
    assert_eq!(d.validate(), vec![]);
    for v in 0..8 {
        let g = d.preview(v).unwrap();
        assert_eq!(
            g.cps.iter().map(|p| p.fn_cp).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 6]
        );
        assert!(g.texts[0].visible);
    }
    let docs = d.documents(symbols::DRAFT_LIBRARY).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(SymbolDraft::parse(&serde_json::to_vec(&d).unwrap()).unwrap(), d);
    let dir = ew_testdir::fresh_dir("symbol-publish");
    let lib = dir.join("symbols.ewlib");
    let tiers = ew_api::library::LibraryTiers::under(&dir, &dir.join("cache"), vec![], vec![lib.clone()]);
    let mut registry = ew_api::library::Registry::open(&tiers).unwrap();
    let id = registry
        .create(&lib, "symbols", "Symbols", "CC-BY-SA-4.0")
        .unwrap();
    d.publish(&mut registry, id).unwrap();
    drop(registry);
    let registry = ew_api::library::Registry::open(&tiers).unwrap();
    let symbols = registry.documents(id, symbols::schema::Kind::Symbol);
    assert_eq!(symbols.len(), 1);
    let symbols::schema::LibDocument::Symbol(s) = &symbols[0] else {
        panic!("symbol")
    };
    assert_eq!(s.item.cps.len(), 6);
    assert!(registry.resolve(&s.item.fndef).is_ok());
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn off_grid_is_retained_and_blocks_save_while_fine_graphics_are_valid() {
    let mut d = block();
    let mut value = serde_json::to_value(&d).unwrap();
    value["symbol"]["cps"][0]["pos"] = json!([4_500_000, 4_000_000]);
    d = SymbolDraft::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        d.validate(),
        vec![symbols::Issue {
            code: "symbol-cp-off-grid",
            path: "/symbol/cps/1/pos".into(),
            detail: "1".into()
        }]
    );
    assert!(d.documents(symbols::DRAFT_LIBRARY).is_err());
    assert!(d.preview(0).is_ok());
}

#[test]
fn eplan_draft_needs_terms_review_and_reliable_connections() {
    let data=br#"<EplanPxfRoot PxfVersion="1.23"><MacroVariant><O31 A531="0/0" A532="4/0"/><O17 A185="1"><S40x1201 A762="0/0"/></O17></MacroVariant></EplanPxfRoot>"#;
    let drafts = symbols::import_eplan(data, "example.ems", "example.import").unwrap();
    let d = &drafts[0];
    assert_eq!(
        d.validate().iter().map(|i| i.code).collect::<Vec<_>>(),
        [
            "symbol-provenance-required",
            "symbol-import-review",
            "symbol-cp-direction"
        ]
    );
    assert!(d.symbol.meta.foreign_terms);
    assert!(d.documents(symbols::DRAFT_LIBRARY).is_err());
}

#[test]
fn duplicate_json_mapping_and_unsafe_geometry_refuse() {
    assert!(SymbolDraft::parse(br#"{"format":"ew.symbol/1","format":"ew.symbol/1"}"#).is_err());
    let mut value = serde_json::to_value(block()).unwrap();
    value["symbol"]["cps"][0]["pos"] = json!([1_000_000_000_001_i64, 0]);
    let d = SymbolDraft::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(d.preview(0).is_err());
}
