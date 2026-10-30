// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.19 part 3 acceptance EM-1 to EM-8 (the reference form of the commands, D7 to D9): a fixture company library
//! with the golden parts 2904602 and A9P52610 (and an MCB A9P52616 made up for the replacement), their manufacturers;
//! `Session::embed_command`, `assign_part`, `update_from_library` and `library_status` over it and over Golden A.

mod golden_common;

use std::path::{Path, PathBuf};

use ew_api::library::{LibraryTiers, Registry};
use ew_api::output::page_view;
use ew_api::{
    AnyCommand, ApiError, AssignMode, ChangeClass, CmdError, CmdLog, CopyState, Session, SessionOptions,
};
use ew_derive::PageOrder;
use ew_ids::{Id, Uuid, v5};
use ew_schema::{Device, LibDocument, LibRef, Part, Record};
use ew_units::Dec3;
use serde_json::{Value, json};

const PROJECT: &str = r#"{"id":"7f0c2a91-5d3e-7b48-9a61-0e2f4c8d1b37","derivation_profile":"2027.1"}"#;

/// The item of the made-up MCB A9P52616 (C16).
const A9P52616: Uuid = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_a916);

struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn tiers(root: &Path) -> LibraryTiers {
    LibraryTiers {
        system: Vec::new(),
        community: root.join("data").join("packs"),
        company: vec![root.join("company")],
        user: root.join("data").join("libraries"),
        cache: root.join("cache"),
    }
}

/// The golden document of a part number, as JSON.
fn golden_part(number: &str) -> Value {
    let batch = golden_common::batch();
    let doc = batch
        .lib
        .iter()
        .find(|d| matches!(d, LibDocument::Part(p) if p.item.part_number.as_str() == number))
        .expect("a golden part");
    serde_json::to_value(doc).unwrap()
}

/// The golden manufacturer copy of the part number's manufacturer, as JSON.
fn golden_manufacturer_of(number: &str) -> Value {
    let part = golden_part(number);
    let item = part["manufacturer"]["item"].clone();
    let batch = golden_common::batch();
    let doc = batch
        .lib
        .iter()
        .find(|d| matches!(d, LibDocument::Manufacturer(m) if json!(m.item_id.as_uuid().to_string()) == item))
        .expect("a golden manufacturer");
    serde_json::to_value(doc).unwrap()
}

/// A golden document moved into the library `lib` (the registry recomputes the copy ID and hash on write).
fn into_library(mut v: Value, lib: Uuid) -> LibDocument {
    v["library_id"] = json!(lib.to_string());
    if v.get("manufacturer").is_some() {
        v["manufacturer"]["library"] = json!(lib.to_string());
        v["manufacturer"]["version_req"] = json!("^1");
    }
    LibDocument::from_value(v).expect("a library document")
}

/// The fixture documents: 2904602 and A9P52610 with their manufacturers, and A9P52616 (Schneider Electric, C16).
fn fixture_docs(lib: Uuid) -> Vec<LibDocument> {
    let mut mcb16 = golden_part("A9P52610");
    mcb16["item_id"] = json!(A9P52616.to_string());
    mcb16["part_number"] = json!("A9P52616");
    mcb16["meta"]["key"] = json!("A9P52616");
    mcb16["tech_data"][1]["value"]["value"] = json!("16");
    vec![
        into_library(golden_part("2904602"), lib),
        into_library(golden_part("A9P52610"), lib),
        into_library(mcb16, lib),
        into_library(golden_manufacturer_of("2904602"), lib),
        into_library(golden_manufacturer_of("A9P52610"), lib),
    ]
}

/// A fresh company library holding the fixture documents.
fn company(name: &str) -> (Dir, LibraryTiers, Uuid) {
    let root = Dir(ew_testdir::fresh_dir(name));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(
            &root.0.join("company").join("acme.ewlib"),
            "acme",
            "Acme",
            "CC0-1.0",
        )
        .unwrap();
    reg.write(lib, &fixture_docs(lib)).unwrap();
    (root, t, lib)
}

fn options(libraries: Option<LibraryTiers>) -> SessionOptions {
    SessionOptions {
        libraries,
        ..SessionOptions::default()
    }
}

/// Golden A with the libraries of `tiers`.
fn golden(tiers: Option<LibraryTiers>) -> Session {
    let batch = golden_common::batch();
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    Session::from_records(project.unwrap(), records, batch.lib.clone(), options(tiers)).unwrap()
}

fn lib_ref(lib: Uuid, item: Uuid) -> LibRef {
    serde_json::from_value(json!({"library": lib.to_string(), "item": item.to_string(), "version_req": "^1"}))
        .unwrap()
}

fn item_of(number: &str) -> Uuid {
    golden_part(number)["item_id"].as_str().unwrap().parse().unwrap()
}

fn copy_in(lib: Uuid, item: Uuid) -> Uuid {
    let id: Id<Part> = v5::lib_copy_id(lib, item);
    id.as_uuid()
}

fn golden_copy(number: &str) -> Uuid {
    golden_part(number)["id"].as_str().unwrap().parse().unwrap()
}

fn device(s: &Session, tag: &str) -> Id<Device> {
    golden_common::tags(s)
        .into_iter()
        .find(|(_, t)| t == tag)
        .unwrap_or_else(|| panic!("no device {tag}"))
        .0
}

/// The slot texts printed by the representations of a device, on every page.
fn slot_texts(s: &Session, d: Id<Device>) -> Vec<String> {
    let q = s.query();
    let mut out = Vec::new();
    for entry in &q.get::<PageOrder>(()).entries {
        for rep in page_view(&q, entry).reps {
            let of_device = rep
                .rep
                .functions
                .iter()
                .any(|f| q.functions().get(*f).and_then(|f| f.device) == Some(d));
            if of_device {
                out.extend(rep.slots.iter().map(|s| s.text.clone()));
            }
        }
    }
    out
}

fn part_uses(s: &Session, d: Id<Device>) -> Vec<Uuid> {
    let q = s.query();
    q.devices()
        .get(d)
        .unwrap()
        .parts
        .iter()
        .map(|u| u.part.copy.as_uuid())
        .collect()
}

/// Commits a command, through a preview when it is bulk.
fn commit(s: &mut Session, cmd: &AnyCommand) {
    let preview = s.preview(cmd).unwrap();
    s.commit(preview).unwrap();
}

fn invalid_path(e: CmdError) -> (String, String) {
    match e {
        CmdError::InvalidParams { path, message, .. } => (path, message),
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

/// EM-1 (FT-274, D7): `embed_command` by reference embeds 2904602 with its manufacturer and its template FnDef in one
/// command, and the command replays on a session with no library.
#[test]
fn em1_embedding_by_reference_carries_the_closure_and_replays_without_a_library() {
    let (_root, t, lib) = company("ew-api-em1");
    let project: ew_schema::Project = serde_json::from_str(PROJECT).unwrap();
    let mut s = Session::from_records(project.clone(), [], [], options(Some(t))).unwrap();
    let cmd = s.embed_command(&[lib_ref(lib, item_of("2904602"))]).unwrap();
    assert_eq!(cmd.name, "library.embed");
    let kinds: Vec<&str> = cmd.params["copies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["$k"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["part", "fndef", "manufacturer"],
        "the item first, then FnDefs, then manufacturers"
    );
    s.execute(&cmd).unwrap();
    assert_eq!(s.history().entries().len(), 1, "one command");
    let held = |s: &Session| {
        let q = s.query();
        (
            q.parts().ids().map(|i| i.as_uuid()).collect::<Vec<_>>(),
            q.fndefs().ids().map(|i| i.as_uuid()).collect::<Vec<_>>(),
            q.manufacturers().ids().map(|i| i.as_uuid()).collect::<Vec<_>>(),
        )
    };
    let (parts, fndefs, makers) = held(&s);
    assert_eq!(parts, [copy_in(lib, item_of("2904602"))]);
    assert_eq!((fndefs.len(), makers.len()), (1, 1));
    // The manufacturer names itself from the embedded copy.
    let q = s.query();
    let part = q.parts().get(Id::from_uuid(parts[0])).unwrap();
    assert_eq!(
        ew_derive::manufacturer_name(&q, &part.item.manufacturer),
        "Phoenix Contact"
    );
    drop(q);
    // The replay: a command log line on a session that has no library at all.
    let line = json!({"cmd": cmd.name, "params": cmd.params});
    let text = format!("{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"EM-1\"}}\n{line}\n");
    let mut replay = Session::from_records(project, [], [], options(None)).unwrap();
    replay
        .run_script(&CmdLog::parse(&text).unwrap(), "em1.ewcmd")
        .unwrap();
    assert_eq!(held(&replay), held(&s));
    // Asking again: the project holds everything.
    assert_eq!(
        s.embed_command(&[lib_ref(lib, item_of("2904602"))]).unwrap_err(),
        ApiError::NothingToDo
    );
}

/// EM-2: every rejection of `part.assign`, with its pointer.
#[test]
fn em2_every_rejection_of_part_assign_names_its_pointer() {
    let (_root, t, lib) = company("ew-api-em2");
    let mut s = golden(Some(t));
    let f55 = device(&s, "F55").as_uuid().to_string();
    let f1 = device(&s, "-101F1").as_uuid().to_string();
    let held_part = golden_copy("A9P52610").to_string();
    let lib_part = copy_in(lib, item_of("A9P52610")).to_string();
    let closure = s
        .assign_part(
            &[device(&s, "F55").as_uuid()],
            &lib_ref(lib, item_of("A9P52610")),
            AssignMode::Append,
            Dec3::ONE,
        )
        .unwrap();
    let copies = closure.params["copies"].clone();
    let run = |s: &mut Session, params: Value| {
        invalid_path(
            s.execute(&AnyCommand {
                name: "part.assign".into(),
                params,
            })
            .unwrap_err(),
        )
    };
    let cases: Vec<(Value, &str, &str)> = vec![
        (
            json!({"devices": [], "part": held_part}),
            "/devices",
            "at least one device",
        ),
        (
            json!({"devices": [Uuid::from_u128(7).to_string()], "part": held_part}),
            "/devices/0",
            "no device",
        ),
        (
            json!({"devices": [f55, f55], "part": held_part}),
            "/devices/1",
            "listed twice",
        ),
        (
            json!({"devices": [f55], "part": lib_part}),
            "/part",
            "neither held",
        ),
        (
            json!({"devices": [f55], "part": held_part, "mode": "replace"}),
            "/from",
            "needs `from`",
        ),
        (
            json!({"devices": [f55], "part": held_part, "from": held_part}),
            "/from",
            "mode `replace` only",
        ),
        (
            json!({"devices": [f55], "part": held_part, "mode": "replace", "from": held_part}),
            "/devices/0",
            "does not use part",
        ),
        (
            json!({"devices": [f1], "part": held_part, "qty": "0"}),
            "/qty",
            "greater than 0",
        ),
        (
            json!({"devices": [f1], "part": lib_part, "copies": [copies[0], copies[0]]}),
            "/copies/1",
            "listed twice",
        ),
    ];
    for (params, path, text) in cases {
        let (p, m) = run(&mut s, params.clone());
        assert_eq!(p, path, "{params}: {m}");
        assert!(m.contains(text), "{params}: {m}");
    }
    // A copy of an item the project holds at another version: update from library first.
    s.execute(&closure).unwrap();
    let mut other = copies[0].clone();
    other["version"] = json!("1.0.1");
    other["description"] = json!("Miniature circuit breaker 1P+N, C10");
    let doc = LibDocument::from_value(other).unwrap();
    let hash = ew_commands::commands::library_copy_hash(&doc).unwrap();
    let mut other = serde_json::to_value(&doc).unwrap();
    other["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    let (p, m) = run(
        &mut s,
        json!({"devices": [f1], "part": lib_part, "copies": [other]}),
    );
    assert_eq!(p, "/copies/0");
    assert!(m.contains("update from library first"), "{m}");
    assert_eq!(
        s.history().entries().len(),
        1,
        "only the valid assignment committed"
    );
}

/// EM-3 (D8, D10): assigning A9P52610 to F55 from the company library embeds the part and Schneider Electric's
/// manufacturer copy, sets the part use and prints the manufacturer, the part number and the rating in the slots.
#[test]
fn em3_assigning_from_a_company_library_embeds_and_prints() {
    let (_root, t, lib) = company("ew-api-em3");
    let mut s = golden(Some(t));
    let f55 = device(&s, "F55");
    let before = slot_texts(&s, f55);
    assert!(
        before.iter().any(|t| t == "Siemens") && before.iter().any(|t| t == "5SY4204-7"),
        "{before:?}"
    );
    let makers = s.query().manufacturers().ids().count();
    let cmd = s
        .assign_part(
            &[f55.as_uuid()],
            &lib_ref(lib, item_of("A9P52610")),
            AssignMode::ReplaceAll,
            Dec3::ONE,
        )
        .unwrap();
    let kinds: Vec<&str> = cmd.params["copies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["$k"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["part", "manufacturer"]);
    s.execute(&cmd).unwrap();
    assert_eq!(part_uses(&s, f55), [copy_in(lib, item_of("A9P52610"))]);
    assert_eq!(s.query().manufacturers().ids().count(), makers + 1);
    let after = slot_texts(&s, f55);
    for text in ["Schneider Electric", "A9P52610", "C10"] {
        assert!(after.iter().any(|t| t == text), "{text}: {after:?}");
    }
    assert!(!after.iter().any(|t| t == "Siemens"), "{after:?}");
}

/// EM-4 (FR-PRT-023): replacing A9P52610 by another MCB across the devices that use it lists -101F1 in the preview and
/// commits once.
#[test]
fn em4_replacing_a_part_across_devices_lists_them_and_commits_once() {
    let (_root, t, lib) = company("ew-api-em4");
    let mut s = golden(Some(t));
    let from = golden_copy("A9P52610");
    let users: Vec<Uuid> = s
        .query()
        .devices()
        .iter()
        .filter(|(_, d)| d.parts.iter().any(|u| u.part.copy.as_uuid() == from))
        .map(|(id, _)| id.as_uuid())
        .collect();
    let f1 = device(&s, "-101F1");
    assert!(users.contains(&f1.as_uuid()));
    let cmd = s
        .assign_part(
            &users,
            &lib_ref(lib, A9P52616),
            AssignMode::Replace { from },
            Dec3::ONE,
        )
        .unwrap();
    let preview = s.preview(&cmd).unwrap();
    let changed: Vec<Uuid> = preview
        .diff()
        .changes
        .iter()
        .filter(|c| c.key.0 == ew_schema::Kind::Device)
        .map(|c| c.key.1.as_uuid())
        .collect();
    assert!(changed.contains(&f1.as_uuid()), "{changed:?}");
    let steps = s.history().entries().len();
    s.commit(preview).unwrap();
    assert_eq!(s.history().entries().len(), steps + 1);
    assert_eq!(part_uses(&s, f1), [copy_in(lib, A9P52616)]);
    let texts = slot_texts(&s, f1);
    assert!(
        texts.iter().any(|t| t == "A9P52616") && texts.iter().any(|t| t == "C16"),
        "{texts:?}"
    );
}

/// EM-5 (FR-PRT-014, D9): updating 2904602 with a description change is a patch; the plan shows the field diff and
/// -102G1 before the commit; the library status says so before and after.
#[test]
fn em5_a_patch_update_shows_the_field_diff_and_the_device() {
    let (_root, t, lib) = company("ew-api-em5");
    let mut s = golden(Some(t));
    let g1 = device(&s, "-102G1");
    let item = item_of("2904602");
    let cmd = s
        .assign_part(
            &[g1.as_uuid()],
            &lib_ref(lib, item),
            AssignMode::Replace {
                from: golden_copy("2904602"),
            },
            Dec3::ONE,
        )
        .unwrap();
    s.execute(&cmd).unwrap();
    let copy = copy_in(lib, item);
    let state = |s: &Session| {
        s.library_status()
            .into_iter()
            .find(|c| c.copy == copy)
            .map(|c| (c.state, c.devices))
            .unwrap()
    };
    assert_eq!(state(&s), (CopyState::Current, vec![g1]));
    assert_eq!(
        s.update_from_library(&[copy], &[]).unwrap_err(),
        ApiError::NothingToDo
    );
    // The library's next version: a description change.
    let mut next = serde_json::to_value(s.libraries().resolve(&lib_ref(lib, item)).unwrap()).unwrap();
    next["version"] = json!("1.0.1");
    next["description"] = json!("Power supply 1 AC / DC, 24 V, 20 A");
    s.libraries_mut()
        .write(lib, &[LibDocument::from_value(next).unwrap()])
        .unwrap();
    assert_eq!(
        state(&s).0,
        CopyState::UpdateAvailable {
            version: "1.0.1".into(),
            class: ChangeClass::Patch
        }
    );
    let plan = s.update_from_library(&[copy], &[]).unwrap();
    assert_eq!(plan.copies.len(), 1);
    let c = &plan.copies[0];
    assert_eq!(
        (c.from.as_str(), c.to.as_str(), c.class),
        ("1.0.0", "1.0.1", ChangeClass::Patch)
    );
    let members: Vec<&str> = c.fields.iter().map(|f| f.member.as_str()).collect();
    assert_eq!(members, ["description"]);
    assert_eq!(c.fields[0].new, Some(json!("Power supply 1 AC / DC, 24 V, 20 A")));
    assert_eq!(plan.devices, [g1]);
    assert!(!plan.representations.is_empty());
    assert!(
        plan.derived.is_empty(),
        "a description changes no tag and no cross-reference"
    );
    assert_eq!(plan.command.name, "library.update");
    // Bulk: refused without a preview, committed through one.
    assert!(matches!(
        s.execute(&plan.command),
        Err(CmdError::NeedsPreview { .. })
    ));
    commit(&mut s, &plan.command);
    let q = s.query();
    let held = q.parts().get(Id::from_uuid(copy)).unwrap();
    assert_eq!(
        (held.version.as_str(), held.item.description.as_neutral()),
        ("1.0.1", Some("Power supply 1 AC / DC, 24 V, 20 A"))
    );
    drop(q);
    assert_eq!(state(&s).0, CopyState::Current);
}

/// FT-640: the library's next version of 2904602 names a manufacturer the project does not hold. The plan's
/// `library.update` embeds it in the same command; the command without it is refused with the missing item listed.
#[test]
fn em9_an_update_embeds_the_new_versions_missing_closure_or_is_refused() {
    let (_root, t, lib) = company("ew-api-em9");
    let mut s = golden(Some(t));
    let g1 = device(&s, "-102G1");
    let item = item_of("2904602");
    let cmd = s
        .assign_part(
            &[g1.as_uuid()],
            &lib_ref(lib, item),
            AssignMode::Replace {
                from: golden_copy("2904602"),
            },
            Dec3::ONE,
        )
        .unwrap();
    s.execute(&cmd).unwrap();
    let copy = copy_in(lib, item);
    // The next version names a new manufacturer of the library.
    let maker_item = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0640);
    let mut maker = golden_manufacturer_of("2904602");
    maker["item_id"] = json!(maker_item.to_string());
    maker["key"] = json!("PowerCo");
    maker["name"] = json!("PowerCo");
    let mut next = serde_json::to_value(s.libraries().resolve(&lib_ref(lib, item)).unwrap()).unwrap();
    next["version"] = json!("1.1.0");
    next["manufacturer"]["item"] = json!(maker_item.to_string());
    s.libraries_mut()
        .write(
            lib,
            &[into_library(maker, lib), LibDocument::from_value(next).unwrap()],
        )
        .unwrap();
    let plan = s.update_from_library(&[copy], &[]).unwrap();
    let embed: Vec<(&str, &str)> = plan.command.params["embed"]
        .as_array()
        .expect("the closure the new version adds")
        .iter()
        .map(|d| (d["$k"].as_str().unwrap(), d["item_id"].as_str().unwrap()))
        .collect();
    let maker_text = maker_item.to_string();
    assert_eq!(embed, [("manufacturer", maker_text.as_str())]);
    // Without `embed`: refused, the missing item listed.
    let mut bare = plan.command.clone();
    bare.params.as_object_mut().unwrap().remove("embed");
    let maker_copy = copy_in(lib, maker_item).to_string();
    match s.preview(&bare).unwrap_err() {
        CmdError::Rejected { reason, .. } => assert!(
            reason.contains("FT-640") && reason.contains(&format!("manufacturer copy {maker_copy}")),
            "{reason}"
        ),
        other => panic!("expected the missing closure refused, got {other:?}"),
    }
    // With it: one command, and the part names the embedded manufacturer.
    commit(&mut s, &plan.command);
    let q = s.query();
    let held = q.parts().get(Id::from_uuid(copy)).unwrap();
    assert_eq!(held.version.as_str(), "1.1.0");
    assert_eq!(
        ew_derive::manufacturer_name(&q, &held.item.manufacturer),
        "PowerCo"
    );
}

/// The held symbol copy with this key, as JSON.
fn held_symbol(s: &Session, key: &str) -> Value {
    let q = s.query();
    let doc = q
        .symbols()
        .iter()
        .find(|(_, c)| c.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key))
        .map(|(_, c)| LibDocument::Symbol((*c).as_ref().clone()))
        .expect("a held symbol");
    serde_json::to_value(doc).unwrap()
}

/// A changed library document: version 9.0.0 and the hash of its item.
fn rehashed(mut v: Value) -> Value {
    v["version"] = json!("9.0.0");
    let doc = LibDocument::from_value(v).unwrap();
    let hash = ew_commands::commands::library_copy_hash(&doc).unwrap();
    let mut v = serde_json::to_value(&doc).unwrap();
    v["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    v
}

/// EM-6 (FR-SYM-016, D9): a symbol update that moves the anchor is minor and changes cross-reference texts, which the
/// preview lists; one that removes a connected CP is refused with the list.
#[test]
fn em6_a_symbol_update_is_minor_with_a_moved_anchor_and_refused_without_a_connected_cp() {
    let mut s = golden(None);
    let old = held_symbol(&s, "ew.gen.ip@qet10");
    let mut moved = old.clone();
    moved["anchor"] = json!({"named": [80_000_000, 0]});
    let moved = rehashed(moved);
    let diff = ew_api::classify_update(
        &LibDocument::from_value(old.clone()).unwrap(),
        &LibDocument::from_value(moved.clone()).unwrap(),
    );
    assert_eq!(diff.class, ChangeClass::Minor);
    let cmd = AnyCommand {
        name: "library.update".into(),
        params: json!({"copies": [moved]}),
    };
    let preview = s.preview(&cmd).unwrap();
    let derived = s.derived_diff(&preview);
    assert!(
        !derived.xrefs.is_empty(),
        "the moved anchor moves the cross-referenced cells"
    );
    // A removed CP that is connected: refused, with the list.
    let mut cut = old.clone();
    cut["cps"] = json!([]);
    let cut = rehashed(cut);
    let diff = ew_api::classify_update(
        &LibDocument::from_value(old).unwrap(),
        &LibDocument::from_value(cut.clone()).unwrap(),
    );
    assert_eq!(diff.class, ChangeClass::Major);
    let e = s
        .preview(&AnyCommand {
            name: "library.update".into(),
            params: json!({"copies": [cut]}),
        })
        .unwrap_err();
    let CmdError::Rejected { reason, .. } = &e else {
        panic!("{e:?}")
    };
    assert!(
        reason.contains("connected CPs without a CP") && reason.contains("connection"),
        "{reason}"
    );
}

/// EM-7 (doc 07 §4.5): a declared patch that removes a CP is classified major, whatever its version says.
#[test]
fn em7_a_declared_patch_that_removes_a_cp_is_major() {
    let old = golden_part("2904602");
    let mut new = old.clone();
    new["version"] = json!("1.0.1");
    new["cp_pattern"].as_array_mut().unwrap().pop();
    let diff = ew_api::classify_update(
        &LibDocument::from_value(old).unwrap(),
        &LibDocument::from_value(new).unwrap(),
    );
    assert_eq!(diff.class, ChangeClass::Major);
    assert_eq!(diff.major_reasons, ["CP 6 (−) disappears"]);
}

/// The CP-map path of `library.update`: a part whose CP index changes designation is major, the connected CP needs a
/// mapping, and the mapping moves the connection endpoints.
#[test]
fn a_major_part_update_remaps_connections_through_the_cp_map() {
    let mut s = golden(None);
    let old = golden_part("2904602");
    let copy = golden_copy("2904602");
    // CPs 3 and 4 ("+") swap places with 5 and 6 ("−"): by designation, 3 → 5 and 5 → 3 (the lowest index first).
    let mut new = old.clone();
    let pattern = new["cp_pattern"].as_array_mut().unwrap();
    for cp in pattern.iter_mut() {
        let d = match cp["designation"].as_str().unwrap() {
            "+" => "−",
            "−" => "+",
            other => other,
        };
        cp["designation"] = json!(d);
    }
    let new = rehashed(new);
    let ends_at = |s: &Session, idx: u16| -> usize {
        let q = s.query();
        let g1 = device(s, "-102G1");
        let f = q
            .functions()
            .iter()
            .find(|(_, f)| f.device == Some(g1))
            .map(|(id, _)| id)
            .unwrap();
        q.connections()
            .iter()
            .filter(|(_, c)| {
                [&c.a, &c.b]
                    .iter()
                    .any(|e| matches!(e, ew_schema::Endpoint::Cp(g, i) if *g == f && *i == idx))
            })
            .count()
    };
    let before: Vec<usize> = (0..7).map(|i| ends_at(&s, i)).collect();
    commit(
        &mut s,
        &AnyCommand {
            name: "library.update".into(),
            params: json!({"copies": [new]}),
        },
    );
    let after: Vec<usize> = (0..7).map(|i| ends_at(&s, i)).collect();
    assert_eq!(
        after,
        [
            before[0], before[1], before[2], before[5], before[6], before[3], before[4]
        ]
    );
    let q = s.query();
    assert_eq!(q.parts().get(Id::from_uuid(copy)).unwrap().version, "9.0.0");
}

/// `library.update`'s other refusals: a copy the project does not hold, a downgrade, a mapping for no copy.
#[test]
fn library_update_refuses_what_it_cannot_update() {
    let (_root, t, lib) = company("ew-api-update-refusals");
    let mut s = golden(Some(t));
    let not_held = serde_json::to_value(s.libraries().resolve(&lib_ref(lib, A9P52616)).unwrap()).unwrap();
    let run = |s: &mut Session, params: Value| {
        invalid_path(
            s.preview(&AnyCommand {
                name: "library.update".into(),
                params,
            })
            .unwrap_err(),
        )
    };
    let (p, m) = run(&mut s, json!({"copies": [not_held]}));
    assert_eq!(p, "/copies/0");
    assert!(m.contains("holds no part copy"), "{m}");
    let mut lower = golden_part("2904602");
    lower["description"] = json!("older");
    let mut lower = rehashed(lower);
    lower["version"] = json!("0.9.0");
    let doc = LibDocument::from_value(lower).unwrap();
    let mut lower = serde_json::to_value(&doc).unwrap();
    lower["sha256"] =
        json!(ew_schema::Sha256(ew_commands::commands::library_copy_hash(&doc).unwrap()).to_string());
    let (p, m) = run(&mut s, json!({"copies": [lower.clone()]}));
    assert_eq!(p, "/copies/0/version");
    assert!(m.contains("downgrade"), "{m}");
    let (p, _) = run(
        &mut s,
        json!({"copies": [rehashed(golden_part("2904602"))], "cp_maps": [{"copy": Uuid::from_u128(1).to_string(), "map": []}]}),
    );
    assert_eq!(p, "/cp_maps/0/copy");
    let (p, _) = run(
        &mut s,
        json!({"copies": [rehashed(golden_part("2904602"))], "cp_maps": [{"copy": golden_copy("2904602").to_string(), "map": [[9, 0]]}]}),
    );
    assert_eq!(p, "/cp_maps/0/map/0/0");
}

/// EM-8 (FR-PRT-034): deprecating A9P52610 with a successor; Golden A still opens unchanged with the library, and
/// `library_status` offers the successor for -101F1 once it holds the library's copy.
#[test]
fn em8_a_deprecated_part_offers_its_successor() {
    let (_root, t, lib) = company("ew-api-em8");
    let item = item_of("A9P52610");
    {
        let mut reg = Registry::open(&t).unwrap();
        let mut v = serde_json::to_value(reg.resolve(&lib_ref(lib, item)).unwrap()).unwrap();
        v["version"] = json!("1.1.0");
        v["meta"]["status"] = json!({"deprecated": {"successor": lib_ref(lib, A9P52616)}});
        reg.write(lib, &[LibDocument::from_value(v).unwrap()]).unwrap();
    }
    // Golden A opens as it does without libraries: every page the same.
    let plain = golden(None);
    let mut s = golden(Some(t));
    let pages = |s: &Session| {
        let q = s.query();
        q.get::<PageOrder>(())
            .entries
            .iter()
            .map(|e| page_view(&q, e))
            .collect::<Vec<_>>()
    };
    assert!(pages(&s) == pages(&plain));
    // -101F1 takes the library's copy (at version 1.0.0 here, the version the project had embedded before).
    let f1 = device(&s, "-101F1");
    let mut v = golden_part("A9P52610");
    v["library_id"] = json!(lib.to_string());
    v["id"] = json!(copy_in(lib, item).to_string());
    v["manufacturer"]["library"] = json!(lib.to_string());
    v["manufacturer"]["version_req"] = json!("^1");
    let doc = LibDocument::from_value(v).unwrap();
    let mut v = serde_json::to_value(&doc).unwrap();
    v["sha256"] =
        json!(ew_schema::Sha256(ew_commands::commands::library_copy_hash(&doc).unwrap()).to_string());
    let maker = serde_json::to_value(s.libraries().closure(&lib_ref(lib, item)).unwrap()[1].clone()).unwrap();
    s.execute(&AnyCommand {
        name: "part.assign".into(),
        params: json!({"devices": [f1.as_uuid().to_string()], "part": copy_in(lib, item).to_string(),
            "copies": [v, maker], "mode": "replace", "from": golden_copy("A9P52610").to_string()}),
    })
    .unwrap();
    let status = s.library_status();
    let mcb = status.iter().find(|c| c.copy == copy_in(lib, item)).unwrap();
    assert_eq!(mcb.devices, [f1]);
    assert_eq!(
        mcb.state,
        CopyState::Deprecated {
            successor: Some(lib_ref(lib, A9P52616))
        }
    );
    // The successor resolves and can replace it on -101F1.
    let cmd = s
        .assign_part(
            &[f1.as_uuid()],
            &lib_ref(lib, A9P52616),
            AssignMode::Replace {
                from: copy_in(lib, item),
            },
            Dec3::ONE,
        )
        .unwrap();
    s.execute(&cmd).unwrap();
    assert_eq!(part_uses(&s, f1), [copy_in(lib, A9P52616)]);
}

/// The 0.1.0 copies Golden A held before FT-698 (cycle 38), by key, with their published hashes.
const GOLDEN_PRE_FT698: [(&str, &str); 3] = [
    (
        "ew.iec.lamp@qet10",
        "sha256:6361cb1f5af892dcf3c76c6dd5a44f3bfb74172810e1b7275fad93de3e3e8955",
    ),
    (
        "ew.iec.position-switch-no@qet10",
        "sha256:f9bc6581347a7d9f8a1516604c5447cef1935fa3c57865597dfac2fde38c5d6f",
    ),
    (
        "ew.iso.valve-5-2-sol-spring@qet10",
        "sha256:3195e0603f56bbfdfb265ecb3055317d187fa0ef819c4737f81238d88a846cd3",
    ),
];

/// A 0.2.0 starter symbol of FT-698 as its 0.1.0 predecessor: the appended `plc_address` slot and its arrangement
/// entries removed, the version and the hash of the item put back.
fn before_ft698(doc: &LibDocument) -> LibDocument {
    let mut v = serde_json::to_value(doc).unwrap();
    let slots = v["text_slots"].as_array_mut().unwrap();
    let last = slots.pop().unwrap();
    assert_eq!(last["kind"], json!("plc_address"), "the appended slot");
    let index = json!(slots.len());
    for a in v["arrangements"].as_array_mut().unwrap() {
        a["slots"].as_array_mut().unwrap().retain(|i| *i != index);
    }
    v["version"] = json!("0.1.0");
    let doc = LibDocument::from_value(v).unwrap();
    let hash = ew_commands::commands::library_copy_hash(&doc).unwrap();
    let mut v = serde_json::to_value(&doc).unwrap();
    v["sha256"] = json!(ew_schema::Sha256(hash).to_string());
    LibDocument::from_value(v).unwrap()
}

/// The key of a symbol document.
fn symbol_key(d: &LibDocument) -> Option<String> {
    match d {
        LibDocument::Symbol(c) => c.item.meta.key.as_ref().map(|k| k.as_str().to_owned()),
        _ => None,
    }
}

/// FT-698 (cycle 38), the product path of Golden A's symbol change: over Golden A as it was (its lamp, position
/// switch and 5/2 valve at 0.1.0, rebuilt from the new copies and checked against the published hashes), the library
/// status offers exactly those three copies a minor update to the starter's 0.2.0; "update from library" updates them
/// in one command, after which the held copies equal the bootstrap's (the committed fixture's), no copy has an update
/// left and the valve of -201U3-Q1 prints its channel address Q0.1.
#[test]
fn ft698_update_from_library_brings_golden_a_symbols_to_the_plc_address_slot() {
    let batch = golden_common::batch();
    let mut current = Vec::new();
    let mut lib = batch.lib.clone();
    for doc in &mut lib {
        let Some((key, published)) =
            symbol_key(doc).and_then(|k| GOLDEN_PRE_FT698.into_iter().find(|(p, _)| *p == k))
        else {
            continue;
        };
        current.push(doc.clone());
        let before = before_ft698(doc);
        let LibDocument::Symbol(c) = &before else {
            unreachable!("a symbol stays a symbol")
        };
        assert_eq!(c.sha256.to_string(), published, "{key}");
        *doc = before;
    }
    assert_eq!(current.len(), GOLDEN_PRE_FT698.len());
    let mut project = None;
    let mut records = Vec::new();
    for r in &batch.records {
        match r {
            Record::Project(p) => project = Some(p.clone()),
            other => records.push(other.clone()),
        }
    }
    let mut s = Session::from_records(project.unwrap(), records, lib, options(None)).unwrap();
    let copies: Vec<Uuid> = current
        .iter()
        .map(|d| match d {
            LibDocument::Symbol(c) => c.id.as_uuid(),
            _ => unreachable!("symbols only"),
        })
        .collect();
    let valve = device(&s, "-201U3-Q1");
    assert!(!slot_texts(&s, valve).iter().any(|t| t == "Q0.1"));
    for c in s.library_status() {
        if copies.contains(&c.copy) {
            assert_eq!(
                c.state,
                CopyState::UpdateAvailable {
                    class: ChangeClass::Minor,
                    version: "0.2.0".into()
                },
                "{}",
                c.copy
            );
        } else {
            assert!(!matches!(c.state, CopyState::UpdateAvailable { .. }), "{c:?}");
        }
    }
    let plan = s.update_from_library(&copies, &[]).unwrap();
    let mut planned: Vec<(Uuid, &str, &str, ChangeClass)> = plan
        .copies
        .iter()
        .map(|c| (c.copy, c.from.as_str(), c.to.as_str(), c.class))
        .collect();
    planned.sort_by_key(|p| p.0);
    let mut expected: Vec<(Uuid, &str, &str, ChangeClass)> = copies
        .iter()
        .map(|c| (*c, "0.1.0", "0.2.0", ChangeClass::Minor))
        .collect();
    expected.sort_by_key(|p| p.0);
    assert_eq!(planned, expected);
    commit(&mut s, &plan.command);
    for doc in &current {
        let key = symbol_key(doc).unwrap();
        assert_eq!(held_symbol(&s, &key), serde_json::to_value(doc).unwrap(), "{key}");
    }
    assert!(
        s.library_status()
            .iter()
            .all(|c| !matches!(c.state, CopyState::UpdateAvailable { .. }))
    );
    assert!(slot_texts(&s, valve).iter().any(|t| t == "Q0.1"));
}
