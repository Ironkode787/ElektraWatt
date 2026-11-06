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
fn nameless_embedded_qet_symbol_can_be_forked_by_copy_identity() {
    use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
    let mut original = symbols::import_qet(
        include_bytes!("../../../ew-import-qet/tests/fixtures/contact_no.elmt"),
        "contact.elmt",
        "example.qet",
    )
    .unwrap();
    original.symbol.meta.licence = Some("CC-BY-SA-4.0".into());
    let mut copies = original.documents(symbols::DRAFT_LIBRARY).unwrap();
    let symbol = copies
        .iter_mut()
        .find(|d| matches!(d, symbols::schema::LibDocument::Symbol(_)))
        .unwrap();
    if let symbols::schema::LibDocument::Symbol(s) = symbol {
        s.item.meta.key = None;
    }
    let digest = ew_store::ew_format::library_copy_hash(symbol).unwrap();
    let source = if let symbols::schema::LibDocument::Symbol(s) = symbol {
        s.sha256 = ew_schema::Sha256(digest);
        s.id.to_string()
    } else {
        unreachable!()
    };
    let mut session = Session::new_project(
        ProjectTemplate::Builtin,
        NewProjectKind::Schematic,
        SessionOptions::default(),
    )
    .unwrap();
    session
        .execute(&AnyCommand {
            name: "library.embed".into(),
            params: json!({"copies":copies}),
        })
        .unwrap();
    let fork = session.symbol_draft(&source, "example.copy").unwrap();
    assert_eq!(fork.symbol.grid, symbols::units::Grid::QET_10PX);
    assert_eq!(fork.symbol.graphics, original.symbol.graphics);
    assert_eq!(fork.function.cps, original.function.cps);
    assert_eq!(fork.symbol.meta.key.unwrap().as_str(), "example.copy");
    assert!(fork.function_ref.is_none());
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
    let mut d = block();
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
    let issues = d.validate();
    assert_eq!(
        issues,
        vec![symbols::Issue {
            code: "symbol-cp-off-grid",
            path: "/symbol/cps/0/pos".into(),
            detail: "1".into()
        }]
    );
    // The pointer names the moved pin's position in the draft, not its neighbour's.
    assert_eq!(
        serde_json::to_value(&d).unwrap().pointer(&issues[0].path),
        Some(&json!([4_500_000, 4_000_000]))
    );
    assert!(d.documents(symbols::DRAFT_LIBRARY).is_err());
    assert!(d.preview(0).is_ok());
}

#[test]
fn authored_symbol_closure_is_carried_when_placing_a_part_without_a_template() {
    use ew_api::{AnyCommand, NewProjectKind, ProjectTemplate, Session, SessionOptions};
    let dir = ew_testdir::fresh_dir("symbol-part-closure");
    let lib = dir.join("symbols.ewlib");
    let tiers = ew_api::library::LibraryTiers::under(&dir, &dir.join("cache"), vec![], vec![lib.clone()]);
    let mut registry = ew_api::library::Registry::open(&tiers).unwrap();
    let library = registry
        .create(&lib, "symbols", "Symbols", "CC-BY-SA-4.0")
        .unwrap();
    let mut draft = block();
    draft.publish(&mut registry, library).unwrap();
    let docs = draft.documents(library).unwrap();
    let symbol = docs
        .iter()
        .find_map(|doc| match doc {
            symbols::schema::LibDocument::Symbol(s) => Some(s),
            _ => None,
        })
        .unwrap();
    let address = format!("library:{}:{}", symbol.library_id, symbol.item_id);
    let mut part: serde_json::Value =
        serde_json::from_str(include_str!("../expected/two_fndef_part.json")).unwrap();
    part["function_template"] = json!([]);
    let mut part = symbols::schema::LibDocument::from_value(part).unwrap();
    let hash = ew_store::ew_format::library_copy_hash(&part).unwrap();
    let id = if let symbols::schema::LibDocument::Part(p) = &mut part {
        p.sha256 = ew_schema::Sha256(hash);
        p.id.as_uuid()
    } else {
        panic!("part fixture")
    };
    let mut session = Session::new_project(
        ProjectTemplate::Builtin,
        NewProjectKind::Schematic,
        SessionOptions::default(),
    )
    .unwrap();
    *session.libraries_mut() = registry;
    session
        .execute(&AnyCommand {
            name: "library.embed".into(),
            params: json!({"copies":[part]}),
        })
        .unwrap();
    let params = session
        .starter_embed_part(id, &address)
        .unwrap()
        .expect("authored symbol closure");
    session
        .execute(&AnyCommand {
            name: "library.embed".into(),
            params,
        })
        .unwrap();
    assert!(session.query().symbols().contains(symbol.id));
    assert!(session.starter_embed_part(id, &address).unwrap().is_none());
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

#[test]
fn unknown_electrical_semantics_are_inspectable_but_cannot_publish_or_place() {
    for pointer in [
        "/function/contact_kind",
        "/function/cps/0/discipline",
        "/function/cps/0/potential_role",
        "/function/cps/0/port_role",
        "/function/cps/0/side",
        "/symbol/cps/0/discipline",
    ] {
        let mut doc = serde_json::to_value(block()).unwrap();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        doc.pointer_mut(parent).unwrap()[key] = json!("unsupported-kind");
        let draft = SymbolDraft::parse(&serde_json::to_vec(&doc).unwrap()).unwrap();
        assert!(draft.preview(0).is_ok(), "{pointer}");
        assert!(
            draft
                .validate()
                .iter()
                .any(|i| i.code == "symbol-function-semantics"),
            "{pointer}"
        );
        assert!(draft.documents(symbols::DRAFT_LIBRARY).is_err(), "{pointer}");
    }
}

#[test]
fn preview_uses_the_page_text_style_in_every_variant() {
    let mut draft = block();
    draft.symbol.text_slots[0].style = Some("tag.main".parse().unwrap());
    let defaults = ew_api::output::new_project_text_style_table();
    assert_eq!(
        defaults.get("cp.small").unwrap().em,
        ew_api::new_project_text_styles()["cp.small"].size
    );
    let styles = symbols::TextStyleTable::from_rows(defaults.rows().iter().map(|row| {
        (
            row.key.as_str(),
            if row.key == "tag.main" {
                symbols::units::Nm(6_000_000)
            } else {
                row.em
            },
            row.weight,
        )
    }));
    for variant in 0..8 {
        let g = draft.preview(variant).unwrap();
        let list = symbols::preview_display_list(&draft, &g, &styles, &|_| "Text".into());
        let runs: Vec<_> = list
            .items
            .iter()
            .filter_map(|i| match &i.kind {
                symbols::drawing::ItemKind::Glyphs(run) => Some(run),
                _ => None,
            })
            .collect();
        assert!(!runs.is_empty());
        let expected = symbols::units::Nm(6_000_000);
        assert!(runs.iter().all(|run| run.size == expected));
    }
}

#[test]
// This opt-in test reads a private corpus path at the test boundary, never in production code.
#[allow(clippy::disallowed_methods)]
fn optional_native_slk_sample_opens_without_eplan() {
    let Some(path) = std::env::var_os("EW_EPLAN_SLK_SAMPLE") else {
        return;
    };
    let drafts = symbols::open(std::path::Path::new(&path), "sample.slk").unwrap();
    assert!(!drafts.is_empty());
    let sample = drafts
        .iter()
        .find(|d| serde_json::to_string(&d.symbol.name).unwrap().contains("QF-FU"))
        .unwrap();
    assert!(!sample.symbol.graphics.is_empty());
    assert!(sample.function.cps.len() >= 6);
    assert!(sample.import_notes.iter().any(|n| n == "eplan-slk-subset"));
}
