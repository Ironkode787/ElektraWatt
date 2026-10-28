// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-410: the golden parts table read through `read_parts_tables` and dispatched as `parts.import` into a new
//! project: the golden project's 29 part copies, project-local, the starter FnDefs resolved; a second import skips
//! every key; the picker lists the parts with a function template.

use std::path::Path;

use ew_api::import::{ImportError, PARTS_IMPORT, PartsReadError, read_parts_tables};
use ew_api::{AnyCommand, Session, SessionOptions};

fn session() -> Session {
    let project = serde_json::from_str(
        r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#,
    )
    .unwrap();
    let mut s = Session::from_records(project, [], [], SessionOptions::default()).unwrap();
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params: serde_json::json!({"number": 1, "title": "Cover"}),
    })
    .unwrap();
    s
}

fn golden_parts() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/J-E-2416/tables/parts.csv")
}

fn import(s: &mut Session) -> ew_api::Receipt {
    let project = s.query().project().id.as_uuid();
    let parts = read_parts_tables(&golden_parts(), project).expect("the golden parts table reads");
    assert_eq!(parts.source, "parts.csv");
    assert_eq!(parts.siblings.len(), 4, "every sibling table lies beside it");
    let cmd = AnyCommand {
        name: PARTS_IMPORT.into(),
        params: parts.params(),
    };
    let preview = s
        .preview_with(&cmd, ew_api::Submit::from(ew_api::Origin::Gui))
        .unwrap();
    s.commit(preview).unwrap()
}

#[test]
fn the_golden_parts_table_imports_into_a_new_project_once() {
    let mut s = session();
    let r = import(&mut s);
    assert_eq!(r.outcome.note.as_deref(), Some("created 29, skipped 0"));
    assert_eq!(s.query().parts().iter().count(), 29);
    let r = import(&mut s);
    assert_eq!(r.outcome.note.as_deref(), Some("created 0, skipped 29"));
    assert_eq!(s.query().parts().iter().count(), 29);
    let folio = s.page_labels().first().cloned().unwrap();
    let cat = s.place_catalogue(&folio).expect("a drawn page");
    assert!(
        !cat.parts.is_empty(),
        "the picker lists the parts with a template"
    );
    let numbers: Vec<&str> = cat.parts.iter().map(|p| p.part_number.as_str()).collect();
    let mut sorted = numbers.clone();
    sorted.sort_unstable();
    assert_eq!(numbers, sorted, "part-number order (FT-439)");
}

#[test]
fn a_parts_table_above_the_size_limit_is_refused_before_it_is_read() {
    let dir = std::env::temp_dir().join(format!("ew-parts-limit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("parts.csv");
    let limit = ew_import_tables::TableLimits::default().max_file_bytes;
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(limit + 1).unwrap();
    drop(file);
    let project = session().query().project().id.as_uuid();
    let result = read_parts_tables(&path, project);
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        matches!(
            result,
            Err(PartsReadError::Import(ImportError::LimitExceeded { .. }))
        ),
        "{result:?}"
    );
}

/// JB8, FT-464: a `parts.csv` and sibling tables that a spreadsheet saved with a byte-order mark import the same copies
/// as the tables without it, through the user's path (`read_parts_tables`, which `parts.import` and `ew import parts`
/// both call); the bootstrap's own reader stays strict.
#[test]
fn jb8_a_bom_prefixed_parts_table_imports_the_same_copies() {
    let dir = std::env::temp_dir().join(format!("ew-parts-bom-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let golden = golden_parts();
    let tables = golden.parent().unwrap();
    for file in [
        "parts.csv",
        "part_cps.csv",
        "part_templates.csv",
        "cable_cores.csv",
        "part_plc.csv",
    ] {
        let text = std::fs::read_to_string(tables.join(file)).unwrap();
        assert!(!text.starts_with('\u{feff}'), "{file} has no BOM of its own");
        std::fs::write(dir.join(file), format!("\u{feff}{text}")).unwrap();
    }
    let project = session().query().project().id.as_uuid();
    let plain = read_parts_tables(&golden, project).expect("the golden tables read");
    let marked = read_parts_tables(&dir.join("parts.csv"), project);
    std::fs::remove_dir_all(&dir).unwrap();
    let marked = marked.expect("the BOM-prefixed tables read");
    assert_eq!(marked.siblings, plain.siblings);
    assert_eq!(marked.copies.len(), 29);
    assert_eq!(marked.copies, plain.copies, "the same part copies");
    // The bootstrap's reader keeps refusing the mark.
    let strict = ew_import_tables::csv::parse(
        "\u{feff}a\n".as_bytes(),
        &ew_import_tables::TableLimits::default(),
    );
    assert!(strict.is_err(), "ew_import_tables stays strict");
    // And `parts.import` commits them like the plain ones.
    let mut s = session();
    let cmd = AnyCommand {
        name: PARTS_IMPORT.into(),
        params: marked.params(),
    };
    let preview = s
        .preview_with(&cmd, ew_api::Submit::from(ew_api::Origin::Gui))
        .unwrap();
    let r = s.commit(preview).unwrap();
    assert_eq!(r.outcome.note.as_deref(), Some("created 29, skipped 0"));
}
