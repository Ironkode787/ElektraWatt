// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The store's per-file row index and the commands' row dump over Golden A's page items and structure boxes
//! (WP-S2.04 part 4a): a kind missing from the model's key list is dropped by the next save that rewrites its
//! file, and `render_all` (`Session::create`) drops it too. Both readers are generated from the model's entity
//! list now; these tests keep the save path and the row dump covered end to end.

use crate::golden_common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ew_api::{AnyCommand, Recovery, Session, SessionOptions, StoreOptions};
use ew_derive::Q;
use ew_derive::ew_ids::Id;
use ew_schema::{Kind, Page, StructureNode};
use ew_store::mem::{DirSync, MemVfs};
use ew_store::{ManualClock, Vfs};
use golden_common::{batch, ids_by_key, session};
use serde_json::{Value, json};

fn cmd(name: &str, params: Value) -> AnyCommand {
    AnyCommand {
        name: name.into(),
        params,
    }
}

fn run(s: &mut Session, name: &str, params: Value) -> ew_api::Receipt {
    s.execute(&cmd(name, params))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn id<T>(ids: &std::collections::BTreeMap<String, String>, key: &str) -> Id<T> {
    ids.get(key)
        .unwrap_or_else(|| panic!("no record {key}"))
        .parse()
        .unwrap_or_else(|_| panic!("{key} is an ID"))
}

/// The location node of a structure node code.
fn node(q: &Q, code: &str) -> Id<StructureNode> {
    q.structure_nodes()
        .iter()
        .find(|(_, n)| n.code.as_str() == code)
        .map(|(id, _)| id)
        .unwrap_or_else(|| panic!("no node {code}"))
}

/// Store options on the in-memory file system (the project and its journals stay inside the test).
fn mem_store(vfs: &Arc<MemVfs>) -> StoreOptions {
    let mut o = StoreOptions::new("ElektraWatt test");
    o.vfs = Arc::clone(vfs) as Arc<dyn Vfs>;
    o.clock = Arc::new(ManualClock::fixed());
    o.state_dir = Some(PathBuf::from("/state"));
    o
}

/// The page tables' item kinds of page 101, and its structure boxes' captions, sorted.
fn items_of(s: &Session) -> (Vec<String>, Vec<String>) {
    let t = s.page_tables("101").expect("page 101");
    let mut items: Vec<String> = t.items.iter().map(|i| i.kind.clone()).collect();
    items.sort();
    let mut boxes: Vec<String> = t.structure_boxes.iter().map(|b| b.caption.clone()).collect();
    boxes.sort();
    (items, boxes)
}

/// A page item and a structure box survive the later saves of their page and a Save As, through the store's
/// per-file index (built from the model's generated key list) and `render_all` (`Session::create`).
#[test]
fn page_items_and_structure_boxes_survive_later_saves_and_save_as() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page: Id<Page> = id(&ids, "pages|101");
    let vfs = Arc::new(MemVfs::new(DirSync::Explicit));
    vfs.put_dir(Path::new("/state"));
    let dir = PathBuf::from("/p");
    // A writable copy of Golden A.
    let mut s = session(&batch);
    s.create(&dir, mem_store(&vfs)).expect("the copy is written");
    assert_eq!(s.save().expect("an unchanged copy saves nothing").generation, 1);
    let je206 = {
        let q = s.query();
        node(&q, "J-E-206")
    };
    run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {"text": "survives", "at": [50_000_000, 60_000_000]}}}),
    );
    run(
        &mut s,
        "box.structure",
        json!({
            "page": page.to_string(),
            "outline": [[40_000_000, 40_000_000], [70_000_000, 40_000_000], [70_000_000, 70_000_000], [40_000_000, 70_000_000]],
            "style": "dash_dot",
            "assigns": {"location": je206.to_string()},
        }),
    );
    s.save().expect("the first save writes the item and the box");
    // A second edit on the same page, in the same session.
    run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"graphic": {"shape": {"line": {"a": [50_000_000, 80_000_000], "b": [60_000_000, 80_000_000]}}}}}),
    );
    s.save().expect("the second save keeps the item and the box");
    s.close();
    // Reopened: the free text, the box and the line are there.
    let (mut r, report) = Session::open(&dir, mem_store(&vfs), SessionOptions::default(), Recovery::Keep)
        .expect("the copy reopens");
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    let (items, boxes) = items_of(&r);
    assert_eq!(
        items.iter().filter(|kind| kind.as_str() == "free_text").count(),
        1,
        "{items:?}"
    );
    assert!(items.iter().any(|kind| kind.as_str() == "graphic"), "{items:?}");
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    // A later save of a freshly opened session rewrites the page's shard from the store's index: the three rows
    // must still be in it.
    run(
        &mut r,
        "item.place",
        json!({"page": page.to_string(), "kind": {"graphic": {"shape": {"circle": {"centre": [90_000_000, 90_000_000], "r": 2_000_000}}}}}),
    );
    r.save().expect("the later save keeps the item and the box");
    r.close();
    let (mut again, _) = Session::open(&dir, mem_store(&vfs), SessionOptions::default(), Recovery::Keep)
        .expect("the copy reopens after the later save");
    let (items, boxes) = items_of(&again);
    assert_eq!(
        items.iter().filter(|kind| kind.as_str() == "free_text").count(),
        1,
        "{items:?}"
    );
    assert_eq!(
        items.iter().filter(|kind| kind.as_str() == "graphic").count(),
        2,
        "{items:?}"
    );
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    // Save As through `Session::create` (the store's `render_all`): the three rows are in the new directory.
    let as_dir = PathBuf::from("/p-as");
    again
        .create(&as_dir, mem_store(&vfs))
        .expect("the save-as is written");
    again.close();
    let (saved_as, _) = Session::open(
        &as_dir,
        mem_store(&vfs),
        SessionOptions::default(),
        Recovery::Keep,
    )
    .expect("the save-as reopens");
    let (items, boxes) = items_of(&saved_as);
    assert_eq!(
        items.iter().filter(|kind| kind.as_str() == "free_text").count(),
        1,
        "{items:?}"
    );
    assert_eq!(
        items.iter().filter(|kind| kind.as_str() == "graphic").count(),
        2,
        "{items:?}"
    );
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    saved_as.close();
}

/// `ew-commands`' row dump (`model_lines`, generated from the model's key list) lists the page item and the
/// structure box like any other record.
#[test]
fn model_lines_sees_page_items_and_structure_boxes() {
    let batch = batch();
    let ids = ids_by_key(&batch);
    let page: Id<Page> = id(&ids, "pages|101");
    let mut s = session(&batch);
    let je206 = {
        let q = s.query();
        node(&q, "J-E-206")
    };
    run(
        &mut s,
        "item.place",
        json!({"page": page.to_string(), "kind": {"free_text": {"text": "listed", "at": [50_000_000, 60_000_000]}}}),
    );
    run(
        &mut s,
        "box.structure",
        json!({
            "page": page.to_string(),
            "outline": [[40_000_000, 40_000_000], [70_000_000, 40_000_000], [70_000_000, 70_000_000], [40_000_000, 70_000_000]],
            "style": "dash_dot",
            "assigns": {"location": je206.to_string()},
        }),
    );
    let tables = s.page_tables("101").expect("page 101");
    let item = tables
        .items
        .iter()
        .find(|i| i.kind == "free_text")
        .expect("the free text");
    let box_ = tables.structure_boxes.first().expect("the structure box");
    let lines = ew_commands::rows::model_lines(&s.world().model).expect("the rows");
    for (kind, id) in [(Kind::PageItem, item.id), (Kind::StructureBox, box_.id)] {
        assert!(
            lines
                .iter()
                .any(|(key, _)| key.0 == kind && key.1.to_string() == id.to_string()),
            "no model line for {kind}:{id}"
        );
    }
}
