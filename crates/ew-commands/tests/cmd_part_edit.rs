// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S2.07 part 4a at the command layer: `part.edit_copy` (D20, D21; FT-394), in place and as a fork that re-points
//! every part-copy link of the link table, with each refusal of §9.3 at its pointer; and `copies` on `fields.set` and
//! `fields.set_many` (D16, PS4's command half).
// A reader of ew-model's raw API (ADR-0001 amendment 2): the workspace clippy.toml guard does not apply here.
#![allow(clippy::disallowed_methods)]

mod common;

use common::fixture::*;
use common::*;
use ew_commands::commands::{EditPartCopy, KindName, SetFields, SetManyFields, library_copy_hash};
use ew_commands::{CmdError, Engine, Origin};
use ew_ids::{Id, v5};
use ew_model::Model;
use ew_schema::{
    AnyId, Connection, Device, LibCopy, LibDocument, LibItem, Manufacturer, Part, PotentialDef, Sha256,
};
use serde_json::{Value, json};

/// The project ID of the mini golden project.
fn project_uuid() -> ew_ids::Uuid {
    project().id.as_uuid()
}

/// A library of the copies these tests embed (not the project's).
const LIB_Q: u128 = 0xc000;
const ITEM_Q: u128 = 0xc001;
const ITEM_MFR_Q: u128 = 0xc002;
/// The item of the project-local copy.
const ITEM_LOCAL: u128 = 0xc010;
/// The fork's item.
const FORK: u128 = 0xc020;

fn with_hash<T: Clone>(mut copy: LibCopy<T>, wrap: fn(LibCopy<T>) -> LibDocument) -> LibCopy<T> {
    let hash = library_copy_hash(&wrap(copy.clone())).expect("a hash");
    copy.sha256 = Sha256(hash);
    copy
}

/// A part copy of library `library`, item `item`, with its copy identity and true hash.
fn part_copy(library: ew_ids::Uuid, item: ew_ids::Uuid, number: &str) -> LibCopy<Part> {
    let mut c = part(v5::lib_copy_id(library, item), number);
    c.library_id = Id::from_uuid(library);
    c.item_id = Id::from_uuid(item);
    with_hash(c, LibDocument::Part)
}

/// The project-local part copy of the in-place tests.
fn local_copy() -> LibCopy<Part> {
    part_copy(project_uuid(), uuid(ITEM_LOCAL), "LOCAL-1")
}

/// A manufacturer document of [`LIB_Q`] in the `lib/` JSON form.
fn manufacturer_doc() -> (Id<Manufacturer>, Value) {
    let id: Id<Manufacturer> = v5::lib_copy_id(uuid(LIB_Q), uuid(ITEM_MFR_Q));
    let doc = LibDocument::from_value(json!({
        "$k": "manufacturer", "id": id.to_string(), "library_id": uuid(LIB_Q).to_string(),
        "item_id": uuid(ITEM_MFR_Q).to_string(), "version": "1.0.0",
        "sha256": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "key": "QX", "name": "Q Electric",
    }))
    .expect("a manufacturer");
    let LibDocument::Manufacturer(c) = doc else {
        unreachable!()
    };
    let c = with_hash(c, LibDocument::Manufacturer);
    (id, serde_json::to_value(LibDocument::Manufacturer(c)).unwrap())
}

/// The mini golden model with the project-local copy added.
fn model() -> Model {
    let (p, r, _) = mini_golden_parts();
    let mut lib = library();
    lib.push(LibDocument::Part(local_copy()));
    Model::from_records(p, r, lib).expect("valid")
}

fn set(kind: KindName, target: AnyId, path: &str, value: Value) -> SetFields {
    SetFields {
        kind,
        targets: vec![target],
        path: path.into(),
        value,
        copies: Vec::new(),
    }
}

fn copy_of(e: &Engine, id: Id<Part>) -> Option<LibCopy<Part>> {
    e.model().table::<LibCopy<Part>>().get(&id).map(|c| (**c).clone())
}

/// The edited item of `copy` with `description`.
fn described(copy: &LibCopy<Part>, description: &str) -> Value {
    let mut v = serde_json::to_value(&copy.item).unwrap();
    v["description"] = json!(description);
    v
}

fn edit(copy: Id<Part>, part: Value, fork: Option<u128>) -> EditPartCopy {
    EditPartCopy {
        copy,
        part,
        fork: fork.map(|f| Id::<LibItem>::from_uuid(uuid(f))),
        copies: Vec::new(),
    }
}

fn run(e: &mut Engine, cmd: &EditPartCopy) -> Result<ew_commands::Receipt, CmdError> {
    let preview = e.preview(cmd)?;
    e.commit(preview)
}

fn pointer(err: CmdError) -> String {
    match err {
        CmdError::InvalidParams { command, path, .. } => {
            assert_eq!(command, "part.edit_copy");
            path
        }
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

/// D20: `part.edit_copy` is Bulk: it runs only through a preview.
#[test]
fn edit_copy_is_bulk() {
    let mut e = engine(model());
    let c = local_copy();
    let err = e
        .execute(&edit(c.id, described(&c, "Changed"), None))
        .unwrap_err();
    assert!(matches!(err, CmdError::NeedsPreview { .. }), "{err:?}");
}

/// PS6's command half: a project-local copy is rewritten in place (same IDs and version, the hash of the new item);
/// undo restores the records byte for byte.
#[test]
fn a_project_local_copy_is_edited_in_place() {
    let mut e = engine(model());
    let before = lines(e.model());
    let c = local_copy();
    let receipt = run(&mut e, &edit(c.id, described(&c, "MCB 1P+N C4"), None)).expect("in place");
    assert_eq!(
        receipt.outcome.primary,
        Some((ew_schema::Kind::Part, c.id.cast()))
    );
    let after = copy_of(&e, c.id).expect("same copy");
    assert_eq!(
        (after.library_id, after.item_id, after.version.as_str()),
        (c.library_id, c.item_id, "1.0.0")
    );
    assert_eq!(after.item.description.as_neutral(), Some("MCB 1P+N C4"));
    assert_eq!(
        Some(after.sha256.0),
        library_copy_hash(&LibDocument::Part(after.clone())),
        "EW-CORE-014: the hash of the new item"
    );
    e.undo(&Origin::Gui).expect("undo");
    assert_eq!(lines(e.model()), before);
}

/// PS7's command half (D20, R7): a copy from a library forks: one new project-local copy with the fork's item ID,
/// the original's version and `meta.source.fork_of`; every part-copy link of the link table that named the original
/// names the fork (part use, strip accessory, assembly configuration, a connection's wire part, a potential
/// definition's wire part); the original copy stays unchanged; undo restores every record.
#[test]
fn a_library_copy_forks_and_every_part_copy_link_is_repointed() {
    let mut e = engine(model());
    let mcb = PART_MCB;
    let copy_ref = json!({"copy": mcb.to_string()});
    e.execute(&set(
        KindName::Device,
        DEV_X1031.cast(),
        "strip.accessories",
        json!([{"after_seq": 0, "kind": "end_clamp", "part": copy_ref}]),
    ))
    .expect("accessory");
    e.execute(&set(
        KindName::Device,
        DEV_102G1.cast(),
        "assembly",
        json!({"configs": [{"order_code": "X1", "part": copy_ref}]}),
    ))
    .expect("assembly");
    e.execute(&set(
        KindName::Connection,
        CON_W11A.cast(),
        "overrides.wire_part",
        copy_ref.clone(),
    ))
    .expect("wire part");
    e.execute(&set(
        KindName::PotentialDef,
        PD_24V.cast(),
        "props",
        json!({"wire_part": copy_ref}),
    ))
    .expect("potential wire part");
    let before = lines(e.model());
    let original = copy_of(&e, mcb).expect("held");
    let receipt = run(
        &mut e,
        &edit(mcb, described(&original, "MCB 1P+N C4"), Some(FORK)),
    )
    .expect("the fork");
    let fork_id: Id<Part> = v5::lib_copy_id(project_uuid(), uuid(FORK));
    assert_eq!(
        receipt.outcome.primary,
        Some((ew_schema::Kind::Part, fork_id.cast()))
    );
    let fork = copy_of(&e, fork_id).expect("the fork copy");
    assert_eq!(fork.library_id.as_uuid(), project_uuid());
    assert_eq!(fork.item_id.as_uuid(), uuid(FORK));
    assert_eq!(fork.version, original.version);
    assert_eq!(
        fork.item.meta.extra.get("source"),
        Some(&json!({"fork_of": {
            "library": original.library_id.as_uuid().to_string(),
            "item": original.item_id.as_uuid().to_string(),
            "version": original.version,
        }}))
    );
    assert_eq!(
        Some(fork.sha256.0),
        library_copy_hash(&LibDocument::Part(fork.clone()))
    );
    assert_eq!(copy_of(&e, mcb), Some(original), "the original copy is unchanged");
    let dev = |d: Id<Device>| e.model().table::<Device>().get(&d).cloned().unwrap();
    assert_eq!(dev(DEV_101F1).parts[0].part.copy, fork_id, "part use");
    assert_eq!(
        dev(DEV_X1031).aspects.strip.as_ref().unwrap().accessories[0]
            .part
            .copy,
        fork_id,
        "strip accessory"
    );
    assert_eq!(
        dev(DEV_102G1).aspects.assembly.as_ref().unwrap().configs[0]
            .part
            .as_ref()
            .unwrap()
            .copy,
        fork_id,
        "assembly configuration"
    );
    let con = e.model().table::<Connection>().get(&CON_W11A).cloned().unwrap();
    assert_eq!(
        con.overrides.wire_part.as_ref().unwrap().copy,
        fork_id,
        "connection wire part"
    );
    let pd = e.model().table::<PotentialDef>().get(&PD_24V).cloned().unwrap();
    assert_eq!(
        pd.props.wire_part.as_ref().unwrap().copy,
        fork_id,
        "potential wire part"
    );
    // A second edit of the fork is in place: no second fork.
    let again = edit(fork_id, described(&fork, "MCB 1P+N C6"), None);
    run(&mut e, &again).expect("in place");
    assert_eq!(
        copy_of(&e, fork_id).unwrap().item.description.as_neutral(),
        Some("MCB 1P+N C6")
    );
    e.undo(&Origin::Gui).expect("undo the second edit");
    e.undo(&Origin::Gui).expect("undo the fork");
    assert_eq!(lines(e.model()), before);
}

/// §9.3's refusals, each at its pointer, and the unchanged item refused as a whole.
#[test]
fn every_refusal_has_its_pointer() {
    let mut e = engine(model());
    let local = local_copy();
    let item = || serde_json::to_value(&local.item).unwrap();
    let with = |k: &str, v: Value| {
        let mut p = item();
        p[k] = v;
        p
    };
    let cases: Vec<(EditPartCopy, &str)> = vec![
        (edit(id(0xdead), item(), None), "/copy"),
        (edit(local.id, json!("a part"), None), "/part"),
        (
            edit(
                local.id,
                with("cp_pattern", json!([{"idx": 1, "designation": "1"}])),
                None,
            ),
            "/part",
        ),
        (
            edit(local.id, with("manufacturer", Value::Null), None),
            "/part/manufacturer",
        ),
        (
            edit(local.id, with("part_number", json!("  ")), None),
            "/part/part_number",
        ),
        (edit(local.id, with("unit", json!("pcs")), None), "/part/unit"),
        (
            edit(
                local.id,
                with(
                    "tech_data",
                    json!([{"key": "in", "value": "C"}, {"key": "in", "value": "B"}]),
                ),
                None,
            ),
            "/part/tech_data/1",
        ),
        (
            edit(
                local.id,
                with(
                    "manufacturer",
                    json!({"library": uuid(LIB_Q).to_string(),
                    "item": uuid(ITEM_MFR_Q).to_string(), "version_req": "*"}),
                ),
                None,
            ),
            "/part/manufacturer",
        ),
        (edit(local.id, described(&local, "x"), Some(FORK)), "/fork"),
        (
            edit(PART_MCB, described(&copy_of(&e, PART_MCB).unwrap(), "x"), None),
            "/fork",
        ),
        (
            edit(
                PART_MCB,
                described(&copy_of(&e, PART_MCB).unwrap(), "x"),
                Some(ITEM_LOCAL),
            ),
            "/fork",
        ),
    ];
    for (cmd, at) in cases {
        let err = e.preview(&cmd).map(|_| ()).unwrap_err();
        assert_eq!(pointer(err), at, "{cmd:?}");
    }
    // A document of `copies` that does not read as a library document.
    let mut bad_copies = edit(local.id, described(&local, "x"), None);
    bad_copies.copies = vec![json!({"$k": "part"})];
    assert_eq!(
        pointer(e.preview(&bad_copies).map(|_| ()).unwrap_err()),
        "/copies/0"
    );
    // The unchanged item is refused as a whole.
    match e.preview(&edit(local.id, item(), None)).map(|_| ()).unwrap_err() {
        CmdError::Rejected { reason, .. } => assert!(reason.contains("unchanged"), "{reason}"),
        other => panic!("{other:?}"),
    }
    // A manufacturer the project lacks is accepted when `copies` carries it.
    let (mfr, doc) = manufacturer_doc();
    let mut ok = edit(
        local.id,
        with(
            "manufacturer",
            json!({"library": uuid(LIB_Q).to_string(),
            "item": uuid(ITEM_MFR_Q).to_string(), "version_req": "*"}),
        ),
        None,
    );
    ok.copies = vec![doc];
    run(&mut e, &ok).expect("with the manufacturer embedded");
    assert!(e.model().table::<LibCopy<Manufacturer>>().get(&mfr).is_some());
}

/// The technical data are put into key order, so a caller may list them in any order.
#[test]
fn technical_data_are_sorted() {
    let mut e = engine(model());
    let local = local_copy();
    let mut p = serde_json::to_value(&local.item).unwrap();
    p["tech_data"] = json!([{"key": "rated_current", "value": {"value": "4", "unit": "A"}},
                            {"key": "characteristic", "value": "C"}]);
    run(&mut e, &edit(local.id, p, None)).expect("unsorted input");
    let keys: Vec<String> = copy_of(&e, local.id)
        .unwrap()
        .item
        .tech_data
        .iter()
        .map(|t| t.key.as_str().to_owned())
        .collect();
    assert_eq!(keys, ["characteristic", "rated_current"]);
}

/// PS4's command half (D16): `fields.set` with `copies` embeds the closure in the same transaction (one undo step),
/// reuses a held copy of the same hash, and refuses another version at `/copies/<i>`; `fields.set_many` likewise,
/// and refuses `copies` on an entry.
#[test]
fn fields_set_embeds_its_copies() {
    let mut e = engine(model());
    let before = lines(e.model());
    let q = part_copy(uuid(LIB_Q), uuid(ITEM_Q), "FX-BRG-1");
    let doc = serde_json::to_value(LibDocument::Part(q.clone())).unwrap();
    let mut cmd = set(
        KindName::Connection,
        CON_W11A.cast(),
        "overrides.wire_part",
        json!({"copy": q.id.to_string()}),
    );
    // Without the copy the link dangles.
    assert!(e.execute(&cmd).is_err());
    cmd.copies = vec![doc.clone()];
    e.execute(&cmd).expect("embedded and set");
    assert_eq!(copy_of(&e, q.id), Some(q.clone()));
    // The same document again is reused.
    let mut again = set(
        KindName::Connection,
        CON_W11B.cast(),
        "overrides.wire_part",
        json!({"copy": q.id.to_string()}),
    );
    again.copies = vec![doc.clone()];
    e.execute(&again).expect("reused");
    // Another version (another hash) is refused at /copies/0.
    let mut v2 = q.clone();
    v2.version = "1.1.0".into();
    v2.item.description = ew_mltext::MlText::neutral("changed");
    let v2 = with_hash(v2, LibDocument::Part);
    let mut other = set(
        KindName::Connection,
        CON_W22.cast(),
        "overrides.wire_part",
        json!({"copy": q.id.to_string()}),
    );
    other.copies = vec![serde_json::to_value(LibDocument::Part(v2)).unwrap()];
    match e.execute(&other).unwrap_err() {
        CmdError::InvalidParams { path, message, .. } => {
            assert_eq!(path, "/copies/0");
            assert!(message.contains("update from library"), "{message}");
        }
        err => panic!("{err:?}"),
    }
    e.undo(&Origin::Gui).expect("undo");
    e.undo(&Origin::Gui).expect("undo the embedding set: one step");
    assert_eq!(lines(e.model()), before);
    // fields.set_many: the command's copies embed; an entry's are refused.
    let mut many = SetManyFields {
        edits: vec![set(
            KindName::Connection,
            CON_W11A.cast(),
            "overrides.wire_part",
            json!({"copy": q.id.to_string()}),
        )],
        copies: vec![doc.clone()],
    };
    e.execute(&many).expect("set_many embeds");
    assert!(copy_of(&e, q.id).is_some());
    e.undo(&Origin::Gui).expect("undo");
    many.edits[0].copies = vec![doc];
    many.copies = Vec::new();
    match e.execute(&many).unwrap_err() {
        CmdError::InvalidParams { path, .. } => assert_eq!(path, "/edits/0/copies"),
        err => panic!("{err:?}"),
    }
}
