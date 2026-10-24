// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-410: the golden parts table read through `read_parts_tables` and dispatched as `parts.import` into a new
//! project: the golden project's 29 part copies, project-local, the starter FnDefs resolved; a second import skips
//! every key; the picker lists the parts with a function template.

use std::path::Path;

use ew_api::import::{PARTS_IMPORT, read_parts_tables};
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
