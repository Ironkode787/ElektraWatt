// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! CR-1 of WP-S1.17 part 5: a project whose page holds the conflict record the merge driver
//! writes for W20 (MD-6) opens editable with the conflict in the model's own table, and a full save writes the page
//! back byte for byte, the conflict record last.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::{fixture, mem_project_of, options, project_files, root};
use ew_store::ew_format::ew_schema::{Conflict, ConflictBody};
use ew_store::mem::DirSync;
use ew_store::{Access, Changes, Store};

/// Page 102 of Golden A.
const PAGE: &str = "pages/4aa43308-d0d3-57ee-be0c-d9383f429f50.jsonl";
const W20_OVERRIDES: &str = r#""overrides":{"csa":"1.5","show_marker":true}"#;
/// MD-6's conflict record on W20's colour, as `ew merge-driver` writes it.
const CONFLICT: &str = r#"{"$k":"conflict","id":"1c5963b7-6c34-53d6-8fa6-9d02d4629711","kind":"connection","base":{"overrides":{}},"ours":{"overrides":{"colour":["BU"]}},"theirs":{"overrides":{"colour":["WH"]}}}"#;

/// Page 102 as the merge driver leaves it in MD-6's conflict variant: W20 with csa 2.5 and colour BU, and the
/// conflict record.
fn merged_page() -> Vec<u8> {
    let page = String::from_utf8(fixture()[PAGE].clone()).expect("UTF-8");
    let w20 = page
        .lines()
        .find(|l| l.contains("connections|W20"))
        .expect("W20")
        .to_owned();
    let kept = w20.replace(
        W20_OVERRIDES,
        r#""overrides":{"csa":"2.5","colour":["BU"],"show_marker":true}"#,
    );
    format!("{}{CONFLICT}\n", page.replace(&w20, &kept)).into_bytes()
}

#[test]
fn a_conflict_record_opens_editable_in_its_table_and_saves_back_byte_identical() {
    let mut files = fixture();
    files.insert(PAGE.to_owned(), merged_page());
    let (vfs, root_dir) = mem_project_of(DirSync::Explicit, files);
    assert_eq!(root_dir, root());
    let mut opened = Store::open(&root_dir, options(&vfs, 1)).expect("the project opens");
    assert_eq!(opened.report.access, Access::Edit);
    let conflicts: Vec<(String, String)> = opened
        .model
        .table::<Conflict>()
        .iter()
        .map(|(id, c)| {
            let kind = match &c.body {
                ConflictBody::Record { kind, .. } => kind.clone(),
                ConflictBody::File { path, .. } => path.clone(),
            };
            (id.to_string(), kind)
        })
        .collect();
    assert_eq!(
        conflicts,
        [(
            "1c5963b7-6c34-53d6-8fa6-9d02d4629711".to_owned(),
            "connection".to_owned()
        )]
    );
    let model = opened.model.clone();
    opened.store.save(&model, Changes::All).expect("the save");
    assert_eq!(
        String::from_utf8(project_files(&vfs, &root_dir)[PAGE].clone()).expect("UTF-8"),
        String::from_utf8(merged_page()).expect("UTF-8")
    );
}
