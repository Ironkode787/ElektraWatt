// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Macros, on the sample project: `Session::save_macro` of its supply page 101, of the power supply with its input
//! breaker, and of the pneumatics page 201 into a fixture user library, `insert_macro` with new identities and derived
//! tags and
//! `macro_conflicts` on the preview, the replay of the command with no library, twice-run determinism, the
//! variables, value sets and placeholder groups, the refusals of `macro.insert`, and the old and new
//! fragment forms (ADR-0011 amendment 5).

use crate::common::{self, Dir, tiers};

use ew_api::library::{LibraryTiers, Registry};
use ew_api::window::parse_fragment;
use ew_api::{
    AnyCommand, CmdError, CmdLog, InsertAt, MacroConflict, MacroError, MacroScope, MacroSpec, MacroValues,
    SavedMacro, Session, SessionOptions,
};
use ew_ids::Uuid;
use ew_mltext::MlText;
use ew_schema::LibDocument;
use serde_json::{Value, json};

/// A fresh, empty user library.
fn user_library(name: &str) -> (Dir, LibraryTiers, Uuid) {
    let root = Dir(ew_testdir::fresh_dir(name));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("mine.ewlib"), "mine", "Mine", "CC0-1.0")
        .unwrap();
    (root, t, lib)
}

/// The sample with the libraries of `tiers`, over its records in memory.
fn sample(tiers: Option<LibraryTiers>) -> Session {
    let (project, records, lib) = common::sample_records();
    let options = SessionOptions {
        libraries: tiers,
        ids: Some(ew_testids::seeded_ids(36)),
        ..SessionOptions::default()
    };
    Session::from_records(project, records, lib, options).unwrap()
}

/// The printed tag of every device.
fn tags(s: &Session) -> Vec<String> {
    let q = s.query();
    let mut out: Vec<String> = q
        .devices()
        .ids()
        .filter_map(|id| {
            q.get::<ew_derive::DeviceTag>(id)
                .as_ref()
                .as_ref()
                .map(|t| t.short())
        })
        .collect();
    out.sort();
    out
}

/// The folio labels in page order.
fn folios(s: &Session) -> Vec<String> {
    let q = s.query();
    q.get::<ew_derive::PageOrder>(())
        .entries
        .iter()
        .map(|e| e.folio.clone())
        .collect()
}

fn page_of(s: &Session, folio: &str) -> Uuid {
    s.page_tables(folio).expect("a drawn page").page
}

fn device(s: &Session, tag: &str) -> Uuid {
    s.resolve_reference(&format!("tag:{tag}")).expect(tag)
}

/// Every representation, connection and junction of `folio`.
fn whole_page(s: &Session, folio: &str) -> Vec<Uuid> {
    let t = s.page_tables(folio).expect("a drawn page");
    t.reps
        .iter()
        .map(|r| r.id)
        .chain(t.connections.iter().map(|c| c.id))
        .chain(t.junctions.iter().map(|j| j.id))
        .collect()
}

/// The power supply -101G1 and its input breaker -101F1 with the two wires between them, without the interruption
/// points (whose source points EW-CORE-052 allows once per name).
fn psu_core(s: &Session) -> Vec<Uuid> {
    let t = s.page_tables("101").expect("a drawn page");
    let devices = [device(s, "-101F1"), device(s, "-101G1")];
    let functions: Vec<Uuid> = t
        .reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .flat_map(|r| r.functions.clone())
        .collect();
    let inside = |e: &ew_api::output::TableEnd| matches!(e, ew_api::output::TableEnd::Cp { function, .. } if functions.contains(function));
    t.reps
        .iter()
        .filter(|r| r.device.is_some_and(|d| devices.contains(&d)))
        .map(|r| r.id)
        .chain(
            t.connections
                .iter()
                .filter(|c| inside(&c.a) && inside(&c.b))
                .map(|c| c.id),
        )
        .collect()
}

/// A whole page as one window macro, interruption points included.
fn page_block(s: &Session, folio: &str) -> MacroSpec {
    MacroSpec {
        key: format!("page-{folio}"),
        scope: MacroScope::Window(whole_page(s, folio)),
        ..psu_spec(s)
    }
}

fn psu_spec(s: &Session) -> MacroSpec {
    MacroSpec {
        key: "psu-24vdc".into(),
        name: MlText::neutral("PSU 24 V DC"),
        description: MlText::neutral("Power supply with its input breaker"),
        scope: MacroScope::Window(psu_core(s)),
        origin: None,
        keep_tags: Vec::new(),
        variables: Vec::new(),
        groups: Vec::new(),
        value_sets: Vec::new(),
    }
}

/// Folio 106: a new multi-line page in 101's section and structure.
fn add_folio_106(s: &mut Session) -> Uuid {
    let p101 = page_of(s, "101");
    let q = s.query();
    let page = serde_json::to_value(q.pages().get(ew_ids::Id::from_uuid(p101)).unwrap().as_ref()).unwrap();
    drop(q);
    let mut params = json!({"number": 106, "title": "Power Supply 6"});
    if let Some(section) = page["folio"].get("section") {
        params["section"] = section.clone();
    }
    if let Some(structure) = page.get("structure") {
        params["structure"] = structure.clone();
    }
    s.execute(&AnyCommand {
        name: "page.create".into(),
        params,
    })
    .unwrap();
    page_of(s, "106")
}

fn commit(s: &mut Session, cmd: &AnyCommand) {
    let preview = s.preview(cmd).unwrap();
    s.commit(preview).unwrap();
}

/// The QET grid point two hundred grid steps right and down of the origin: free on a new page.
fn free_point() -> ew_units::Pt2 {
    let g = ew_units::Grid::QET_10PX;
    ew_units::Pt2::new(g.at(20).unwrap(), g.at(20).unwrap())
}

const INSTANCE: Uuid = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0105);

/// MC-1 and MC-3 (the card): the whole supply page 101 saves as one window macro into a user library; the parts and
/// symbols are named by reference, and the sample's (the starter's) all resolve; a second save of the key is the next
/// minor version of the same item. Inserted on a new folio 106, the conflicts list the second sources of 101's six
/// outgoing marks (1L1, 1L2, 1L3, PE, 0V, 24V), which EW-CORE-052 refuses until they are resolved; the control page
/// 102 inserted so lists the pairings of its incoming marks 24V, 0V and PE.
#[test]
fn the_psu_block_saves_and_its_insertion_lists_the_conflicts() {
    let (_root, t, lib) = user_library("ew-api-mc1");
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &page_block(&s, "101")).unwrap();
    assert_eq!(saved.version, "1.0.0");
    assert_eq!(saved.reference.version_req.to_string(), "^1.0.0");
    let doc = s.libraries().resolve(&saved.reference).unwrap();
    let LibDocument::Macro(m) = &doc else {
        panic!("a macro document")
    };
    assert_eq!(m.id.as_uuid(), saved.copy);
    assert_eq!(m.item.variants.len(), 1);
    let fragment = &m.item.variants[0].fragment;
    assert!(
        fragment
            .get("copies")
            .is_none_or(|c| c.as_array().is_some_and(Vec::is_empty)),
        "library copies become references"
    );
    let refs = fragment["lib"].as_array().unwrap();
    assert!(!refs.is_empty(), "the parts and symbols are named by reference");
    assert_eq!(saved.unresolved, [], "the starter's libraries are installed");
    let again = s.save_macro(lib, &page_block(&s, "101")).unwrap();
    assert_eq!(
        (again.version.as_str(), again.reference.item),
        ("1.1.0", saved.reference.item)
    );

    let p106 = add_folio_106(&mut s);
    let insert = |s: &Session, reference, instance| {
        s.insert_macro(
            reference,
            &InsertAt {
                page: p106,
                at: Some(free_point()),
                orient: ew_units::Orient2::R0,
            },
            &MacroValues::default(),
            instance,
        )
        .unwrap()
    };
    let cmd = insert(&s, &saved.reference, INSTANCE);
    assert_eq!(cmd.name, "macro.insert");
    let conflicts = s.macro_conflicts(&cmd).unwrap();
    let refused: Vec<&str> = conflicts
        .iter()
        .filter_map(|c| match c {
            MacroConflict::Refused { rule, message } if rule == "EW-CORE-052" => Some(message.as_str()),
            _ => None,
        })
        .collect();
    for name in ["1L1", "1L2", "1L3", "PE", "0V", "24V"] {
        assert!(
            refused.iter().any(|m| m.contains(&format!("point {name}:"))),
            "{name} in {conflicts:?}"
        );
    }
    assert!(
        !conflicts.iter().any(|c| matches!(c, MacroConflict::Tag(_))),
        "the tags derive on 106: {conflicts:?}"
    );
    match s.preview(&cmd) {
        Err(CmdError::Prevented { rule, .. }) => assert_eq!(rule, "EW-CORE-052"),
        other => panic!("expected the second sources refused, got {other:?}"),
    }
    // The control page's incoming marks pair with 101's sources.
    let control = s.save_macro(lib, &page_block(&s, "102")).unwrap();
    let cmd = insert(
        &s,
        &control.reference,
        Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0102),
    );
    let conflicts = s.macro_conflicts(&cmd).unwrap();
    let pairs: Vec<&str> = conflicts
        .iter()
        .filter_map(|c| match c {
            MacroConflict::Pairs { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    for name in ["24V", "0V", "PE"] {
        assert!(pairs.contains(&name), "{name} in {conflicts:?}");
    }
}

/// MC-3: the supply and its breaker inserted on folio 106 yield -106G1 and -106F1 with the IDs
/// v5(instance, local) (D13), one undo step, no conflict.
#[test]
fn the_psu_inserts_with_new_identities_and_derived_tags() {
    let (_root, t, lib) = user_library("ew-api-mc3");
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &psu_spec(&s)).unwrap();
    let p106 = add_folio_106(&mut s);
    let undo_depth = s.history().entries().len();
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(free_point()),
                orient: ew_units::Orient2::R0,
            },
            &MacroValues::default(),
            INSTANCE,
        )
        .unwrap();
    assert_eq!(s.macro_conflicts(&cmd).unwrap(), []);
    let before = tags(&s).len();
    commit(&mut s, &cmd);
    assert_eq!(s.history().entries().len(), undo_depth + 1, "one undo step");
    for tag in ["-106G1", "-106F1", "-101G1", "-101F1"] {
        device(&s, tag);
    }
    assert_eq!(tags(&s).len(), before + 2);
    let q = s.query();
    let inst = q
        .table::<ew_schema::MacroInstance>()
        .get(ew_ids::Id::from_uuid(INSTANCE))
        .expect("the instance record");
    let g1 = device(&s, "-101G1");
    let g1_new = Uuid::new_v5(&INSTANCE, g1.hyphenated().to_string().as_bytes());
    assert!(
        inst.members
            .iter()
            .any(|(local, new)| local.as_uuid() == g1 && new.as_uuid() == g1_new),
        "-106G1 is v5(instance, -101G1)"
    );
    assert_eq!(device(&s, "-106G1"), g1_new);
    assert_eq!(inst.page.as_uuid(), p106);
    drop(q);
    s.undo(&ew_api::Origin::Gui).unwrap();
    assert!(s.resolve_reference("tag:-106G1").is_err());
}

/// MC-2: the pneumatics page 201 saved as a page macro and inserted after 201 creates folio 202 with the valve
/// terminal, its channels and the valves.
#[test]
fn a_page_macro_inserted_after_201_proposes_202() {
    let (_root, t, lib) = user_library("ew-api-mc2");
    let mut s = sample(Some(t));
    let p206 = page_of(&s, "201");
    let spec = MacroSpec {
        key: "valve-page".into(),
        name: MlText::neutral("Valve manifold page"),
        description: MlText::neutral(""),
        scope: MacroScope::Pages(vec![p206]),
        ..psu_spec(&s)
    };
    let saved = s.save_macro(lib, &spec).unwrap();
    let doc = s.libraries().resolve(&saved.reference).unwrap();
    let LibDocument::Macro(m) = &doc else {
        panic!("a macro document")
    };
    let fragment = &m.item.variants[0].fragment;
    assert_eq!(fragment["pages"].as_array().unwrap().len(), 1);
    let kinds: Vec<&str> = fragment["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["$k"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"channel"), "the module's channels: {kinds:?}");
    let channels_before = s.query().table::<ew_schema::Channel>().iter().count();
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p206,
                at: None,
                orient: ew_units::Orient2::R0,
            },
            &MacroValues::default(),
            INSTANCE,
        )
        .unwrap();
    commit(&mut s, &cmd);
    let folios = folios(&s);
    let at = folios.iter().position(|f| f == "201").unwrap();
    assert_eq!(folios[at + 1], "202", "{folios:?}");
    assert!(s.query().table::<ew_schema::Channel>().iter().count() > channels_before);
    s.page_tables("202").expect("a drawn page 202");
}

/// MC-6: the command carries the macro and replays from a command log on a session with no library.
#[test]
fn the_insertion_replays_with_no_library() {
    let (_root, t, lib) = user_library("ew-api-mc6");
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &psu_spec(&s)).unwrap();
    let p106 = add_folio_106(&mut s);
    let create = s.history().entries().len();
    assert_eq!(create, 1);
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(free_point()),
                orient: ew_units::Orient2::R0,
            },
            &MacroValues::default(),
            INSTANCE,
        )
        .unwrap();
    let kinds: Vec<&str> = cmd.params["copies"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["$k"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["macro"],
        "the sample holds the closure; the macro is carried"
    );
    commit(&mut s, &cmd);

    let mut replay = sample(None);
    add_folio_106(&mut replay);
    let line = json!({"cmd": cmd.name, "params": cmd.params});
    let text = format!("{{\"$schema\":\"ew.cmdlog/1\",\"label\":\"MC-6\"}}\n{line}\n");
    replay
        .run_script(&CmdLog::parse(&text).unwrap(), "mc6.ewcmd")
        .unwrap();
    assert_eq!(tags(&replay), tags(&s));
    assert_eq!(
        replay
            .query()
            .table::<ew_schema::MacroInstance>()
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        s.query()
            .table::<ew_schema::MacroInstance>()
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>()
    );
}

fn saved_bytes(name: &str) -> (SavedMacro, Value, AnyCommand) {
    let (_root, t, lib) = user_library(name);
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &psu_spec(&s)).unwrap();
    let doc = serde_json::to_value(s.libraries().resolve(&saved.reference).unwrap()).unwrap();
    let p106 = add_folio_106(&mut s);
    let cmd = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(free_point()),
                orient: ew_units::Orient2::R0,
            },
            &MacroValues::default(),
            INSTANCE,
        )
        .unwrap();
    (saved, doc, cmd)
}

/// MC-7: two runs save byte-equal macro documents and compose equal commands.
#[test]
fn save_and_insert_are_deterministic() {
    let (a, doc_a, cmd_a) = saved_bytes("ew-api-mc7a");
    let (b, doc_b, cmd_b) = saved_bytes("ew-api-mc7b");
    assert_eq!((a.version, a.open_ends), (b.version, b.open_ends));
    // The library IDs differ per run (a fresh library each); the item documents do not.
    let strip = |mut v: Value| {
        for k in ["id", "library_id", "item_id", "sha256"] {
            v.as_object_mut().unwrap().remove(k);
        }
        v
    };
    assert_eq!(strip(doc_a), strip(doc_b));
    let mut pa = cmd_a.params.clone();
    let mut pb = cmd_b.params.clone();
    for p in [&mut pa, &mut pb] {
        p.as_object_mut().unwrap().remove("copies");
        p.as_object_mut().unwrap().remove("macro");
    }
    assert_eq!(pa, pb);
}

/// Variables, value sets and placeholder groups: the value set sets the process tag of the supply, a literal wins
/// over it, and a group whose boolean is false is dropped with its connections.
#[test]
fn values_bind_fields_and_a_false_group_is_dropped() {
    let (_root, t, lib) = user_library("ew-api-macro-values");
    let mut s = sample(Some(t));
    let g1 = device(&s, "-101G1");
    let f1 = device(&s, "-101F1");
    let mut spec = psu_spec(&s);
    spec.key = "psu-values".into();
    spec.variables = serde_json::from_value(json!([
        {"name": "pt", "ty": "process_tag", "bindings": [{"record": g1.to_string(), "field": "device.process_tag"}]},
        {"name": "with_f1", "ty": "bool", "default": true}
    ]))
    .unwrap();
    spec.groups = serde_json::from_value(json!([
        {"name": "input_breaker", "members": [f1.to_string()], "include": "with_f1"}
    ]))
    .unwrap();
    spec.value_sets = serde_json::from_value(json!([
        {"name": "a", "values": {"pt": "PSU-A"}}
    ]))
    .unwrap();
    let saved = s.save_macro(lib, &spec).unwrap();
    let p106 = add_folio_106(&mut s);
    let insert = |s: &Session, values: MacroValues, instance: u128| {
        s.insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(free_point()),
                orient: ew_units::Orient2::R0,
            },
            &values,
            Uuid::from_u128(instance),
        )
        .unwrap()
    };
    let process_tag = |s: &Session, tag: &str| {
        let q = s.query();
        q.devices()
            .get(ew_ids::Id::from_uuid(device(s, tag)))
            .unwrap()
            .process_tag
            .as_ref()
            .map(|p| p.as_str().to_owned())
    };
    // The value set, and the group dropped.
    let cmd = insert(
        &s,
        MacroValues {
            value_set: Some("a".into()),
            values: [("with_f1".to_owned(), json!(false))].into(),
        },
        1,
    );
    commit(&mut s, &cmd);
    assert_eq!(process_tag(&s, "-106G1").as_deref(), Some("PSU-A"));
    assert!(
        s.resolve_reference("tag:-106F1").is_err(),
        "the input breaker is dropped"
    );
    s.undo(&ew_api::Origin::Gui).unwrap();
    // A literal wins over the value set; the default includes the group.
    let cmd = insert(
        &s,
        MacroValues {
            value_set: Some("a".into()),
            values: [("pt".to_owned(), json!("PSU-B"))].into(),
        },
        2,
    );
    commit(&mut s, &cmd);
    assert_eq!(process_tag(&s, "-106G1").as_deref(), Some("PSU-B"));
    assert!(s.resolve_reference("tag:-106F1").is_ok());
    // A value set the macro does not have (or a name that is no identifier) is refused at `/value_set`, naming it
    // and the macro's sets.
    for name in ["zzz", "not a name"] {
        let e = s
            .insert_macro(
                &saved.reference,
                &InsertAt {
                    page: p106,
                    at: Some(free_point()),
                    orient: ew_units::Orient2::R0,
                },
                &MacroValues {
                    value_set: Some(name.into()),
                    values: Default::default(),
                },
                Uuid::from_u128(3),
            )
            .unwrap_err();
        match e {
            ew_api::ApiError::Cmd(ew_api::CmdError::InvalidParams { path, message, .. }) => {
                assert_eq!(path, "/value_set");
                assert_eq!(message, format!("no value set `{name}`: the macro has a"));
            }
            other => panic!("expected a refusal at /value_set, got {other:?}"),
        }
    }
}

/// A variable bound to a field outside the catalogue, and a value of the wrong type, are refused at save time with
/// their pointers.
#[test]
fn an_inconsistent_spec_is_refused_with_its_pointer() {
    let (_root, t, lib) = user_library("ew-api-macro-spec");
    let mut s = sample(Some(t));
    let g1 = device(&s, "-101G1");
    let mut spec = psu_spec(&s);
    spec.variables = serde_json::from_value(json!([
        {"name": "x", "ty": "text", "bindings": [{"record": g1.to_string(), "field": "device.id"}]}
    ]))
    .unwrap();
    match s.save_macro(lib, &spec).unwrap_err() {
        MacroError::Spec { at, .. } => assert_eq!(at, "variables/0/bindings/0"),
        other => panic!("{other:?}"),
    }
    spec.variables = serde_json::from_value(json!([
        {"name": "flag", "ty": "bool", "default": "yes"}
    ]))
    .unwrap();
    assert!(matches!(
        s.save_macro(lib, &spec).unwrap_err(),
        MacroError::Spec { .. }
    ));
}

/// A system library is read-only: the save is refused before anything is written.
#[test]
fn a_save_into_a_read_only_library_is_refused() {
    let (_root, t, _lib) = user_library("ew-api-macro-ro");
    let mut s = sample(Some(t));
    assert_eq!(
        s.save_macro(ew_api::library::starter::LIBRARY, &psu_spec(&s))
            .unwrap_err(),
        MacroError::Library(ew_api::library::LibraryError::ReadOnly)
    );
}

fn invalid_path(e: CmdError) -> String {
    match e {
        CmdError::InvalidParams { path, .. } => path,
        other => panic!("expected InvalidParams, got {other:?}"),
    }
}

/// The refusals of `macro.insert`, each at its pointer.
#[test]
fn macro_insert_refusals_name_their_pointer() {
    let (_root, t, lib) = user_library("ew-api-macro-refusals");
    let mut s = sample(Some(t));
    let saved = s.save_macro(lib, &psu_spec(&s)).unwrap();
    let p106 = add_folio_106(&mut s);
    let good = s
        .insert_macro(
            &saved.reference,
            &InsertAt {
                page: p106,
                at: Some(free_point()),
                orient: ew_units::Orient2::R0,
            },
            &MacroValues::default(),
            INSTANCE,
        )
        .unwrap();
    let with = |f: &dyn Fn(&mut Value)| {
        let mut c = good.clone();
        f(&mut c.params);
        c
    };
    // No anchor for a window macro.
    let cmd = with(&|p| {
        p.as_object_mut().unwrap().remove("at");
    });
    assert_eq!(invalid_path(s.preview(&cmd).unwrap_err()), "/at");
    // The macro is not carried.
    let cmd = with(&|p| p["copies"] = json!([]));
    assert_eq!(invalid_path(s.preview(&cmd).unwrap_err()), "/macro");
    // An unknown value set.
    let cmd = with(&|p| p["value_set"] = json!("nope"));
    assert_eq!(invalid_path(s.preview(&cmd).unwrap_err()), "/value_set");
    // An unknown variable.
    let cmd = with(&|p| p["values"] = json!({"nope": 1}));
    assert!(invalid_path(s.preview(&cmd).unwrap_err()).starts_with("/values"));
    // The instance ID taken: the second insertion with the same instance.
    commit(&mut s, &good);
    assert!(s.preview(&good).is_err(), "an instance ID is used once");
}

/// The fragment forms (ADR-0011 amendment 5): an old fragment (no `pages`, no `lib`) serialises unchanged through the
/// widened object form; `edit.paste` refuses pages; the text form still refuses a channel line.
#[test]
fn old_fragments_round_trip_and_new_content_is_refused_where_it_does_not_belong() {
    let mut s = sample(None);
    let copied = s.copy_fragment(&whole_page(&s, "102")).unwrap();

    let old = parse_fragment(&copied.text).unwrap().params;
    let typed: ew_commands::commands::Fragment = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&typed).unwrap(),
        old,
        "an old fragment is unchanged"
    );
    assert!(old.get("pages").is_none() && old.get("lib").is_none());
    // A new fragment with pages and lib refs round-trips too.
    let mut new = old.clone();
    new["pages"] = json!([{"$k": "page", "id": Uuid::from_u128(7).to_string()}]);
    new["lib"] = json!([{"library": Uuid::from_u128(8).to_string(), "item": Uuid::from_u128(9).to_string(), "version_req": "^1"}]);
    let typed: ew_commands::commands::Fragment = serde_json::from_value(new.clone()).unwrap();
    assert_eq!(serde_json::to_value(&typed).unwrap(), new);
    // edit.paste refuses pages.
    let mut pasted = new.clone();
    pasted.as_object_mut().unwrap().remove("lib");
    let cmd = AnyCommand {
        name: "edit.paste".into(),
        params: json!({"page": page_of(&s, "103").to_string(), "at": old["anchor"], "fragment": pasted}),
    };
    assert_eq!(invalid_path(s.preview(&cmd).unwrap_err()), "/fragment/pages");
    // The text form does not carry channels: a channel line is refused.
    let channel = format!(
        "{}\n{{\"$k\":\"channel\",\"id\":\"{}\"}}\n",
        copied.text.trim_end(),
        Uuid::from_u128(10)
    );
    assert!(parse_fragment(&channel).is_err());
}
