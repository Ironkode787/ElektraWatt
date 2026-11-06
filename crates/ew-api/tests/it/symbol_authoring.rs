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
        let list = symbols::preview_display_list(&draft, &g, &styles, "Text");
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

const PARTS: &str =
    r#"<partsmanagement><part><functiontemplate connectionDesignation="1"/></part></partsmanagement>"#;
const LIMIT: &str = "The EPLAN input exceeds the supported size or complexity.";
const COMPANION: &str = "Keep the SLK file beside its complete .sdb folder, then open it again. A required drawing table is missing.";
const PATH: &str = "The EPLAN archive has unsafe or duplicate member names.";

/// A stored ZIP of `count` copies of `data`, named `parts<n>.xml`.
fn stored_zip(data: &[u8], count: usize) -> Vec<u8> {
    let names: Vec<String> = (0..count).map(|i| format!("parts{i}.xml")).collect();
    let entries: Vec<_> = names
        .iter()
        .map(|n| (n.as_str(), ew_api::zip::Method::Stored, data))
        .collect();
    ew_api::zip::write(&entries).unwrap()
}

/// M25-p2#3, SM25#1, SM25#3 and SM25#4: every symbol import passes WP-S3.09's front door under the element preset
/// and is refused in words: a 10,000-level XML, and an archive past the preset's 16 MiB that the reader alone would
/// read; a `.pxf` file opens through the EPLAN reader.
#[test]
fn symbol_imports_pass_the_front_door_and_refuse_in_words() {
    let deep = format!("{}{}", "<a>".repeat(10_000), "</a>".repeat(10_000));
    let e = symbols::import_eplan(deep.as_bytes(), "deep.xml", "k").unwrap_err();
    assert_eq!(e.to_string(), LIMIT);
    let member = PARTS.replace("</part>", &format!("</part>{}", " ".repeat(3_500_000)));
    let big = stored_zip(member.as_bytes(), 5);
    assert!(big.len() > 16 * 1024 * 1024);
    assert_eq!(
        symbols::import_eplan(&big, "big.edz", "k")
            .unwrap_err()
            .to_string(),
        LIMIT
    );
    assert_eq!(
        symbols::import_eplan(&stored_zip(member.as_bytes(), 4), "a.edz", "k")
            .unwrap()
            .len(),
        4
    );
    let dir = ew_testdir::fresh_dir("symbol-import");
    let pxf = dir.join("macro.pxf");
    std::fs::write(
        &pxf,
        br#"<EplanPxfRoot PxfVersion="1.23" Name="M"><MacroVariant><O31 A531="0/0" A532="4/0"/></MacroVariant></EplanPxfRoot>"#,
    )
    .unwrap();
    assert_eq!(symbols::open(&pxf, "k").unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A link at `at` to `target`, where the system lets a test make one: Windows asks a privilege for a symbolic link,
/// so a folder falls back to a junction, which it does not.
fn link(target: &std::path::Path, at: &std::path::Path, dir: bool) -> bool {
    #[cfg(windows)]
    let made = if dir {
        std::os::windows::fs::symlink_dir(target, at).or_else(|e| {
            let junction = std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(at)
                .arg(target)
                .output()?;
            if junction.status.success() { Ok(()) } else { Err(e) }
        })
    } else {
        std::os::windows::fs::symlink_file(target, at)
    };
    #[cfg(unix)]
    let made = {
        let _ = dir;
        std::os::unix::fs::symlink(target, at)
    };
    made.is_ok()
}

/// ID-1461 and M25-p2#4: an SLK library reads only the tables of its `.sdb` folder beside it, and is refused in
/// words otherwise: a missing folder or table, a parent path, and a linked table or folder.
#[test]
fn slk_reads_only_its_own_tables_beside_it() {
    let dir = ew_testdir::fresh_dir("symbol-slk");
    let slk = dir.join("lib.slk");
    let words = |descriptor: &str| {
        std::fs::write(&slk, descriptor).unwrap();
        symbols::open(&slk, "k").unwrap_err().to_string()
    };
    assert_eq!(words("lib.sdb\n1.2\n"), COMPANION);
    let tables = dir.join("lib.sdb");
    std::fs::create_dir(&tables).unwrap();
    assert_eq!(words("lib.sdb\n1.2\n"), COMPANION);
    assert_eq!(words("../lib.sdb\n1.2\n"), PATH);
    for name in ew_import_eplan::slk::FILES {
        std::fs::write(tables.join(name), b"").unwrap();
    }
    let real = tables.join("real.eox");
    std::fs::write(&real, b"").unwrap();
    std::fs::remove_file(tables.join("Symbol.eox")).unwrap();
    if link(&real, &tables.join("Symbol.eox"), false) {
        assert_eq!(words("lib.sdb\n1.2\n"), PATH);
    }
    if link(&tables, &dir.join("other.sdb"), true) {
        assert_eq!(words("other.sdb\n1.2\n"), PATH);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
