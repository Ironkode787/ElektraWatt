// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 1 (L1-4) at the command layer: `library.embed` of manufacturer and macro copies with the checks
//! of the other kinds, and the links of a `macro_instance` under `page.delete` and `edit.delete` (the page
//! cascades, a deleted member is unlinked, undo restores both), and the normaliser's direct removals (a dissolved
//! junction, a merged connection, an emptied wire marker) under `connection.delete`. No command creates an instance
//! yet: the tests load one with the model.
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{
    DeleteConnections, DeletePages, DeleteSelection, EmbedLibraryCopies, KindName, TargetRef,
    library_copy_hash,
};
use ew_commands::{CmdError, DynCommand, Engine, Origin, Receipt};
use ew_ids::Id;
use ew_model::{Entity, Model};
use ew_schema::{
    AnyId, Kind, LibCopy, LibDocument, MacroDef, MacroInstance, Manufacturer, MarkerField, PageItem,
    PageItemKind, Record, WireMarker,
};
use serde_json::{Value, json};

/// The library the copies of these tests come from (not the project's, so not project-local).
const LIBRARY_M: u128 = 0xb000;
const ITEM_MFR: u128 = 0xb001;
const ITEM_MAC: u128 = 0xb002;
/// The instance of the link tests and its fragment-local IDs.
const INSTANCE: Id<MacroInstance> = id(0xb100);
const LOCAL_G1: u128 = 0xb101;
const LOCAL_F2: u128 = 0xb102;

fn manufacturer_body() -> Value {
    json!({
        "meta": {"key": "ew.mfr.eaton", "licence": "CC0-1.0"},
        "key": "EATON",
        "name": "Eaton",
        "aliases": ["Eaton / Holec"],
        "brands": ["Holec"]
    })
}

fn macro_body() -> Value {
    json!({
        "meta": {"key": "ew.macro.psu-block", "licence": "CC0-1.0"},
        "name": {"en-GB": "PSU block"},
        "kind": "window",
        "variants": [{"rep_type": "multi_line", "key": "A", "fragment": {"format": "ew-fragment v1", "records": []}}],
        "variables": [{
            "name": "counter",
            "ty": "tag_counter",
            "bindings": [{"record": uuid(LOCAL_G1).to_string(), "field": "device.tag.counter"}],
            "default": 1
        }],
        "value_sets": [{"name": "1", "values": {"counter": 1}}]
    })
}

/// `sha256:<hex>` of the compact JSON of `item` in Rust member order (D2), computed here without
/// `ew-commands` or `ew-format`.
fn sha_of<T: serde::Serialize>(item: &T) -> String {
    use sha2::Digest;
    let text = serde_json::to_string(item).expect("item json");
    let digest = sha2::Sha256::digest(text.as_bytes());
    let mut s = String::from("sha256:");
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// A library document of `kind` (`manufacturer` or `macro`) with true lineage (`library`, `item`, the copy
/// identity) and the true hash of its item, in the `lib/` JSON form `library.embed` takes.
fn copy_json(kind: &str, library: u128, item: u128, body: Value) -> (AnyId, Value) {
    let copy_id: AnyId = ew_ids::v5::lib_copy_id(uuid(library), uuid(item));
    let mut v = json!({
        "$k": kind,
        "id": copy_id.to_string(),
        "library_id": uuid(library).to_string(),
        "item_id": uuid(item).to_string(),
        "version": "1.0.0",
        "sha256": format!("sha256:{}", "00".repeat(32)),
    });
    for (k, m) in body.as_object().expect("an object body") {
        v[k] = m.clone();
    }
    let doc = LibDocument::from_value(v.clone()).expect("a library document");
    let sha = match &doc {
        LibDocument::Manufacturer(c) => sha_of(&c.item),
        LibDocument::Macro(c) => sha_of(&c.item),
        other => panic!("not a part-one kind: {:?}", other.kind()),
    };
    // The command's hash function agrees with the rule (D2; the format's is checked in ew-api).
    let computed = library_copy_hash(&doc).expect("a known kind");
    let hex: String = computed.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        sha,
        format!("sha256:{hex}"),
        "{kind}: library_copy_hash is the compact-JSON hash"
    );
    v["sha256"] = json!(sha);
    (copy_id, v)
}

fn manufacturer() -> (Id<Manufacturer>, Value) {
    let (id, v) = copy_json("manufacturer", LIBRARY_M, ITEM_MFR, manufacturer_body());
    (id.cast(), v)
}

fn macro_copy() -> (Id<MacroDef>, Value) {
    let (id, v) = copy_json("macro", LIBRARY_M, ITEM_MAC, macro_body());
    (id.cast(), v)
}

fn embed(copies: Vec<Value>) -> EmbedLibraryCopies {
    EmbedLibraryCopies { copies }
}

fn has_copy<T>(e: &Engine, id: Id<T>) -> bool
where
    LibCopy<T>: Entity<Marker = T>,
{
    e.model().table::<LibCopy<T>>().get(&id).is_some()
}

fn invalid_path(r: Result<Receipt, CmdError>) -> String {
    match r {
        Err(CmdError::InvalidParams { command, path, .. }) => {
            assert_eq!(command, "library.embed");
            path
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

#[test]
fn library_embed_inserts_a_manufacturer_and_a_macro_copy_once_and_undo_removes_them() {
    let mut e = engine(golden_model());
    let before = lines(e.model());
    let (mfr, mfr_json) = manufacturer();
    let (mac, mac_json) = macro_copy();
    let r = e
        .execute(&embed(vec![mfr_json.clone(), mac_json.clone()]))
        .expect("library.embed");
    assert_eq!(r.outcome.primary, Some((Kind::Manufacturer, mfr.cast())));
    assert!(has_copy(&e, mfr));
    assert!(has_copy(&e, mac));
    let row = e
        .model()
        .table::<LibCopy<Manufacturer>>()
        .get(&mfr)
        .expect("embedded")
        .clone();
    assert_eq!(row.item.key.as_str(), "EATON");
    assert_eq!(
        serde_json::to_value(&row.sha256).expect("sha json"),
        mfr_json["sha256"]
    );
    let row = e
        .model()
        .table::<LibCopy<MacroDef>>()
        .get(&mac)
        .expect("embedded")
        .clone();
    assert_eq!(row.item.variables[0].bindings[0].record, id::<()>(LOCAL_G1));
    // Embedded already, at any version: refused, each kind on its own.
    for again in [mfr_json, mac_json] {
        match e.execute(&embed(vec![again])) {
            Err(CmdError::Rejected { reason, .. }) => {
                assert!(reason.contains("embedded already"), "{reason}")
            }
            other => panic!("expected a rejection, got {other:?}"),
        }
    }
    e.undo(&Origin::Gui).expect("undo");
    assert!(!has_copy(&e, mfr));
    assert!(!has_copy(&e, mac));
    assert_eq!(lines(e.model()), before, "undo restores the model byte for byte");
}

#[test]
fn library_embed_checks_manufacturer_and_macro_copies_like_the_others() {
    let mut e = engine(golden_model());
    for (kind, good) in [("manufacturer", manufacturer().1), ("macro", macro_copy().1)] {
        // the same item twice
        assert_eq!(
            invalid_path(e.execute(&embed(vec![good.clone(), good.clone()]))),
            "/copies/1",
            "{kind}"
        );
        // not the copy identity
        let mut bad_id = good.clone();
        bad_id["id"] = json!(uuid(0xbeef).to_string());
        assert_eq!(
            invalid_path(e.execute(&embed(vec![bad_id]))),
            "/copies/0/id",
            "{kind}"
        );
        // a hash that is not the item's
        let mut bad_sha = good.clone();
        bad_sha["sha256"] = json!(format!("sha256:{}", "00".repeat(32)));
        assert_eq!(
            invalid_path(e.execute(&embed(vec![bad_sha]))),
            "/copies/0/sha256",
            "{kind}"
        );
        // a changed member without a new hash
        let mut edited = good.clone();
        edited["meta"]["key"] = json!("ew.other");
        assert_eq!(
            invalid_path(e.execute(&embed(vec![edited]))),
            "/copies/0/sha256",
            "{kind}"
        );
        // a project-local item (library = the project)
        let body = good.as_object().expect("object").clone();
        let item = if kind == "macro" { ITEM_MAC } else { ITEM_MFR };
        let mut members = Value::Object(body);
        for k in ["$k", "id", "library_id", "item_id", "version", "sha256"] {
            members.as_object_mut().expect("object").remove(k);
        }
        let (_, local) = copy_json(kind, PROJECT.as_uuid().as_u128(), item, members);
        assert_eq!(
            invalid_path(e.execute(&embed(vec![local]))),
            "/copies/0/library_id",
            "{kind}"
        );
    }
    assert!(!has_copy(&e, manufacturer().0));
    assert!(!has_copy(&e, macro_copy().0));
}

// ---------------------------------------------------------------------------------------------------------
// macro_instance links under the delete commands

/// The mini golden model plus the macro copy and an instance on page 102 whose members are the device
/// `-G1` and the representation of `-F2`.
fn model_with_instance() -> Model {
    model_with_members(
        vec![(id(LOCAL_G1), DEV_102G1.cast()), (id(LOCAL_F2), REP_102F2.cast())],
        Vec::new(),
    )
}

/// The mini golden model plus the macro copy, the `extra` records and an instance on page 102 listing `members`.
fn model_with_members(members: Vec<(AnyId, AnyId)>, extra: Vec<Record>) -> Model {
    let (p, mut records, _) = mini_golden_parts();
    let (mac, mac_json) = macro_copy();
    let mut lib = library();
    lib.push(LibDocument::from_value(mac_json).expect("the macro copy"));
    records.extend(extra);
    records.push(Record::MacroInstance(MacroInstance {
        id: INSTANCE,
        v: 1,
        macro_copy: mac,
        variant: ident("A"),
        value_set: Some(ident("1")),
        values: [("counter".to_owned(), json!(1))].into_iter().collect(),
        members,
        page: PAGE_102,
        extra: Default::default(),
    }));
    Model::from_records(p, records, lib).expect("an instance whose members and copy resolve is valid")
}

fn instance(e: &Engine) -> Option<MacroInstance> {
    e.model()
        .table::<MacroInstance>()
        .get(&INSTANCE)
        .map(|r| (**r).clone())
}

fn bulk(e: &mut Engine, cmd: &dyn DynCommand) -> Result<Receipt, CmdError> {
    let p = e.preview(cmd)?;
    e.commit(p)
}

#[test]
fn page_delete_takes_the_instance_along_and_undo_restores_it() {
    let mut e = engine(model_with_instance());
    let before = lines(e.model());
    assert!(instance(&e).is_some());
    bulk(
        &mut e,
        &DeletePages {
            pages: vec![PAGE_102],
            delete_devices: false,
        },
    )
    .expect("page.delete");
    assert!(
        instance(&e).is_none(),
        "the instance cascades with its anchor page"
    );
    assert!(
        has_copy(&e, macro_copy().0),
        "the macro copy stays (unused copies stay until purged)"
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(
        lines(e.model()),
        before,
        "undo restores the page and the instance byte for byte"
    );
}

#[test]
fn edit_delete_of_a_member_unlinks_it_and_keeps_the_instance() {
    let mut e = engine(model_with_instance());
    let before = lines(e.model());
    e.execute(&DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Representation,
            id: REP_102F2.cast(),
        }],
    })
    .expect("edit.delete");
    let mi = instance(&e).expect("the instance stays");
    assert_eq!(
        mi.members,
        vec![(id(LOCAL_G1), DEV_102G1.cast())],
        "only the deleted member is unlinked"
    );
    // Deleting the last member leaves an empty instance, not none.
    e.execute(&DeleteSelection {
        targets: vec![TargetRef {
            kind: KindName::Device,
            id: DEV_102G1.cast(),
        }],
    })
    .expect("edit.delete");
    assert_eq!(instance(&e).expect("the instance stays").members, vec![]);
    e.undo(&Origin::Gui).expect("undo");
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(
        lines(e.model()),
        before,
        "undo restores the members byte for byte"
    );
}

/// A wire marker on page 102 listing `connections`.
fn wire_marker(item: Id<PageItem>, connections: Vec<Id<ew_schema::Connection>>) -> Record {
    Record::PageItem(PageItem {
        id: item,
        v: 1,
        page: PAGE_102,
        layer: None,
        kind: PageItemKind::WireMarker(WireMarker {
            connections,
            at: pt(10_000_000, 10_000_000),
            angle: ew_units::Angle2::ZERO,
            fields: vec![MarkerField::Csa],
            extra: Default::default(),
        }),
        extra: Default::default(),
    })
}

/// Deleting a wire so a junction dissolves (normaliser #8 removes the junction and merges the second member into
/// the kept one): the removed junction and connection leave the instance that lists them, the edit commits (no
/// EW-INV-001), the instance and its other members stay, and undo restores everything (WP-S1.19 §6.1).
#[test]
fn a_dissolving_junction_leaves_the_instance_that_lists_it() {
    let members = vec![
        (id(0xb110), J5.cast()),
        (id(0xb111), CON_W22.cast()),
        (id(LOCAL_G1), DEV_102G1.cast()),
    ];
    let mut e = engine(model_with_members(members, Vec::new()));
    let before = lines(e.model());
    e.execute(&DeleteConnections {
        connections: vec![CON_W27],
    })
    .expect("connection.delete commits although the instance lists the dissolved junction");
    assert!(e.model().table::<ew_schema::Junction>().get(&J5).is_none());
    assert!(e.model().table::<ew_schema::Connection>().get(&CON_W22).is_none());
    assert_eq!(
        instance(&e).expect("the instance stays").members,
        vec![(id(LOCAL_G1), DEV_102G1.cast())],
        "the junction and the merged connection leave the instance"
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(
        lines(e.model()),
        before,
        "undo restores the members byte for byte"
    );
}

/// Deleting two of a junction's three wires removes the junction with its last connection, and the wire marker
/// that listed only that connection is removed as empty: all three leave the instance, which stays.
#[test]
fn a_removed_junction_its_last_wire_and_an_emptied_marker_leave_the_instance() {
    let marker: Id<PageItem> = id(0xb120);
    let members = vec![
        (id(0xb110), J5.cast()),
        (id(0xb111), CON_J5_IN.cast()),
        (id(0xb112), marker.cast()),
        (id(LOCAL_G1), DEV_102G1.cast()),
    ];
    let mut e = engine(model_with_members(
        members,
        vec![wire_marker(marker, vec![CON_J5_IN])],
    ));
    let before = lines(e.model());
    e.execute(&DeleteConnections {
        connections: vec![CON_W27, CON_W22],
    })
    .expect("connection.delete commits although the instance lists the removed records");
    assert!(e.model().table::<ew_schema::Junction>().get(&J5).is_none());
    assert!(
        e.model()
            .table::<ew_schema::Connection>()
            .get(&CON_J5_IN)
            .is_none()
    );
    assert!(
        e.model().table::<PageItem>().get(&marker).is_none(),
        "the marker left empty goes"
    );
    assert_eq!(
        instance(&e).expect("the instance stays").members,
        vec![(id(LOCAL_G1), DEV_102G1.cast())]
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(
        lines(e.model()),
        before,
        "undo restores the members byte for byte"
    );
}

#[test]
fn the_macro_copy_an_instance_names_cannot_be_removed() {
    let e = engine(model_with_instance());
    let copy = macro_copy().0;
    match e.model().delete_plan(&(Kind::Macro, copy.cast())) {
        Err(ew_model::ModelError::Referenced { .. }) => {}
        other => panic!("expected Referenced, got {other:?}"),
    }
}
