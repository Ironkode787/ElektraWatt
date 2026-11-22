// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A project whose page holds the conflict record the merge driver
//! writes for a wire's colour opens editable with the conflict in the model's own table, and a full save writes the
//! page back byte for byte, the conflict record last.

// The writer's tests read the model untracked, ADR-0001 amendment 2.
#![allow(clippy::disallowed_methods)]

use crate::common;

use common::{fixture, mem_project_of, options, project_files, root};
use ew_store::ew_format::ew_schema::{Conflict, ConflictBody};
use ew_store::mem::DirSync;
use ew_store::{Access, Changes, Store};

/// The shield wire's page file, as the merge driver leaves it when both sides set the wire's colour (BU and WH)
/// and one its size: the wire with csa 4 and colour BU, and the conflict record. Also the wire's ID.
fn merged_page() -> (String, Vec<u8>, String) {
    let files = fixture();
    // The shield wire is the wire whose cable core is a shield (the sample draws one such core, -103W1's).
    let shields: Vec<String> = files
        .values()
        .flat_map(|bytes| {
            String::from_utf8_lossy(bytes)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|l| l.starts_with(r#"{"$k":"conductor""#) && l.contains(r#""shield":true"#))
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(&l).expect("JSON")["id"]
                .as_str()
                .expect("an ID")
                .to_owned()
        })
        .collect();
    let is_shield = |l: &str| {
        l.starts_with(r#"{"$k":"connection""#)
            && shields
                .iter()
                .any(|s| l.contains(&format!(r#""kind":{{"core":"{s}"}}"#)))
    };
    let (path, page) = files
        .iter()
        .filter(|(path, _)| path.starts_with("pages/"))
        .map(|(path, bytes)| (path.clone(), String::from_utf8(bytes.clone()).expect("UTF-8")))
        .find(|(_, page)| page.lines().any(is_shield))
        .expect("the shield wire's page");
    let wire = page
        .lines()
        .find(|l| is_shield(l))
        .expect("the shield wire")
        .to_owned();
    let id = serde_json::from_str::<serde_json::Value>(&wire).expect("JSON")["id"]
        .as_str()
        .expect("an ID")
        .to_owned();
    // The overrides are the line's last member, where the canonical writer puts them.
    let open = wire.strip_suffix('}').expect("a JSON line");
    let kept = format!(r#"{open},"overrides":{{"csa":"4","colour":["BU"]}}}}"#);
    let conflict = format!(
        r#"{{"$k":"conflict","id":"{id}","kind":"connection","base":{{"overrides":{{}}}},"ours":{{"overrides":{{"colour":["BU"]}}}},"theirs":{{"overrides":{{"colour":["WH"]}}}}}}"#
    );
    let merged = format!("{}{conflict}\n", page.replace(&wire, &kept)).into_bytes();
    (path, merged, id)
}

#[test]
fn a_conflict_record_opens_editable_in_its_table_and_saves_back_byte_identical() {
    let (page, merged, wire) = merged_page();
    let mut files = fixture();
    files.insert(page.clone(), merged.clone());
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
    assert_eq!(conflicts, [(wire, "connection".to_owned())]);
    let model = opened.model.clone();
    opened.store.save(&model, Changes::All).expect("the save");
    assert_eq!(
        String::from_utf8(project_files(&vfs, &root_dir)[&page].clone()).expect("UTF-8"),
        String::from_utf8(merged).expect("UTF-8")
    );
}
