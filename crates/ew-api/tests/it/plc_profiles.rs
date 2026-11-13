// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FR-PLC-008: a profile file put into a library is chosen by `plc.set_profile` without a code change.
//! A fixture library holds a third absolute profile (`%Q0.1`-style, its own `profile.id`); the command-log line that
//! pins it embeds it first (D8), Golden A's configuration project's channels then render in it, and the project,
//! saved and reopened without the library, still renders them from its copy. T1 (a name at hand neither built in nor
//! held) and T2 (a profile file that does not render: refused by `library.embed`, left out of the profiles at hand,
//! noted in the library's listing) are asserted whole.

use std::path::Path;

use crate::common::{Dir as Tmp, copy_dir, tiers};

use ew_api::library::{LibraryTiers, Registry};
use ew_api::output::{NavKind, NavRole};
use ew_api::{AnyCommand, CmdLog, CmdLogError, Recovery, Session, SessionOptions, StoreOptions};
use ew_commands::CmdError;
use ew_derive::{ChannelAddr, ChannelAddresses, Q};
use ew_ids::{Id, Uuid};
use ew_schema::{Device, LibDocument, PlcConfigProject};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");

/// The built-in TIA profile file with another pin and notation: `fixture-absolute@1`, `%Q0.1` on the drawing.
fn fixture_profile(library: Uuid, id: &str, edit: impl FnOnce(&mut Value)) -> LibDocument {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/plc-profiles/siemens-tia-intl.json"),
    )
    .unwrap();
    let mut v: Value = serde_json::from_str(&text).unwrap();
    let item = ew_ids::v5::imported_id::<()>(library, id).as_uuid();
    v["library_id"] = json!(library.to_string());
    v["item_id"] = json!(item.to_string());
    v["id"] = json!(item.to_string());
    v["meta"] = json!({"key": id, "licence": "CC0-1.0", "sources": ["ew.original"]});
    v["profile"] = json!({"id": id, "version": "1"});
    v["label"] = json!("Fixture absolute");
    v["render"]["drawing"] = json!("%");
    edit(&mut v);
    LibDocument::from_value(v).unwrap()
}

/// A user library holding the fixture profile and a profile that does not render (base 16).
fn library(root: &Path) -> (LibraryTiers, Uuid) {
    let t = tiers(root);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("plc.ewlib"), "plc", "PLC profiles", "CC0-1.0")
        .unwrap();
    let docs = vec![
        fixture_profile(lib, "fixture-absolute", |_| {}),
        fixture_profile(lib, "fixture-hex", |v| v["base"] = json!(16)),
    ];
    reg.write(lib, &docs).unwrap();
    (t, lib)
}

/// The badges and the State cell of the Libraries navigator's row of the library `name`.
fn library_row(s: &Session, name: &str) -> (Vec<String>, String) {
    let t = s.navigator_table(NavKind::Libraries);
    let n = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::Library && n.label == name)
        .expect("the library's row");
    let state = t.columns.iter().position(|c| c.key == "state").unwrap();
    (n.badges.clone(), n.cells[state].clone())
}

fn open(dir: &Path, root: &Path, libraries: Option<LibraryTiers>) -> Session {
    let mut store = StoreOptions::new("0.0.0-test");
    store.state_dir = Some(root.join("state"));
    let options = SessionOptions {
        libraries,
        ..SessionOptions::default()
    };
    Session::open(dir, store, options, Recovery::Keep).unwrap().0
}

fn cfg(s: &Session) -> Uuid {
    let q = s.query();
    let ids: Vec<Uuid> = q
        .table::<PlcConfigProject>()
        .iter()
        .map(|(id, _)| id.as_uuid())
        .collect();
    let [one] = ids.as_slice() else {
        panic!("one configuration project: {ids:?}")
    };
    *one
}

/// The rendered addresses of module -201U3's channels.
fn module_texts(s: &Session) -> Vec<String> {
    let module = s.resolve_reference("tag:-201U3").unwrap();
    let q: Q = s.query();
    q.get::<ChannelAddresses>(Id::<Device>::from_uuid(module))
        .iter()
        .map(|e| match &e.address {
            ChannelAddr::At { text, .. } => text.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

fn line(cfg: Uuid, profile: &str) -> CmdLog {
    let text = format!(
        "{}\n{}\n",
        json!({"$schema": ew_api::CMDLOG_SCHEMA, "label": "profiles"}),
        json!({"cmd": "plc.set_profile", "params": {"config_project": cfg.to_string(), "profile": profile}})
    );
    CmdLog::parse(&text).unwrap()
}

/// The message of a refused command (an invalid parameter).
fn refusal(e: &CmdError) -> String {
    match e {
        CmdError::InvalidParams { message, .. } => message.clone(),
        other => panic!("not an invalid parameter: {other:?}"),
    }
}

#[test]
fn a_library_profile_is_chosen_without_a_code_change_and_stays_with_the_project() {
    let root = Tmp(ew_testdir::fresh_dir("ew-api-plc-profiles"));
    let (t, lib) = library(&root.0);
    let dir = root.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut s = open(&dir, &root.0, Some(t.clone()));
    let cfg = cfg(&s);
    assert_eq!(
        module_texts(&s)[1],
        "Q0.1",
        "the built-in TIA profile Golden A pins"
    );

    // T1: a name neither built in nor held, nor in a library.
    let e = s
        .run_script(&line(cfg, "no-such-profile"), "profiles")
        .unwrap_err();
    let CmdLogError::Failed { error, .. } = &e else {
        panic!("{e:?}")
    };
    assert_eq!(
        refusal(error),
        "`no-such-profile` is not an address profile of this build or of the project; the profiles at hand are \
         siemens-tia-intl@2027.1, iec-61131-3@2027.1, rockwell-logix@2027.1"
    );
    // T2: the library's profile that does not render is noted in its listing and left out of the profiles at hand.
    assert_eq!(
        ew_api::unreadable_profiles(s.libraries(), lib),
        ["the address profile fixture-hex@1 cannot be read: only base 10 is rendered"]
    );
    // The Libraries navigator notes it too, on the library's row.
    let (badges, state) = library_row(&s, "PLC profiles");
    assert_eq!(badges, ["profile_unreadable"]);
    assert_eq!(
        state,
        "Index up to date; the address profile fixture-hex@1 cannot be read: only base 10 is rendered"
    );
    let e = s.run_script(&line(cfg, "fixture-hex"), "profiles").unwrap_err();
    let CmdLogError::Failed { error, .. } = &e else {
        panic!("{e:?}")
    };
    assert_eq!(
        refusal(error),
        "`fixture-hex` is not an address profile of this build or of the project; the profiles at hand are \
         siemens-tia-intl@2027.1, iec-61131-3@2027.1, rockwell-logix@2027.1"
    );
    // ... and refused by `library.embed`, so it never reaches a project.
    let hex = s
        .libraries()
        .documents(lib, ew_schema::Kind::PlcProfile)
        .into_iter()
        .find(|d| matches!(d, LibDocument::PlcProfile(c) if c.item.profile.id == "fixture-hex"))
        .unwrap();
    let e = s
        .execute(&AnyCommand {
            name: "library.embed".into(),
            params: json!({ "copies": [hex] }),
        })
        .unwrap_err();
    assert_eq!(
        refusal(&e),
        "the address profile fixture-hex@1 cannot be read: only base 10 is rendered"
    );

    // The library's third profile: the line embeds it, then pins it; the channels render in it.
    s.run_script(&line(cfg, "fixture-absolute"), "profiles").unwrap();
    let pinned = s
        .query()
        .table::<PlcConfigProject>()
        .get(Id::from_uuid(cfg))
        .unwrap()
        .profile
        .to_string();
    assert_eq!(pinned, "fixture-absolute@1");
    assert_eq!(
        s.query().plc_profiles().iter().count(),
        1,
        "the project holds its copy"
    );
    // The copy is listed by its label as a PLC profile, never "Item".
    let t = s.navigator_table(NavKind::Libraries);
    let copy = t
        .nodes
        .iter()
        .find(|n| n.role == NavRole::LibCopy && n.label == "Fixture absolute")
        .expect("the profile's copy row");
    let kind = t.columns.iter().position(|c| c.key == "kind").unwrap();
    assert_eq!(copy.cells[kind], "PLC profile");
    let texts = module_texts(&s);
    assert_eq!(&texts[..2], ["%Q0.0", "%Q0.1"]);
    s.save().unwrap();
    s.close();

    // Reopened without the library, the project renders from its own copy (NFR-REL-11).
    let (reopened, _) = Session::open_read_only(&dir, "0.0.0-test").unwrap();
    assert_eq!(module_texts(&reopened), texts);
}

/// An embedded, edited copy of a built-in profile, with the built-in's own pin, is the
/// project's profile both ways: the channels print in the copy's notation and `fields.set` parses in it.
#[test]
fn an_edited_copy_of_a_builtin_profile_prints_and_parses_in_its_own_notation() {
    let root = Tmp(ew_testdir::fresh_dir("ew-api-plc-profiles-edited"));
    let t = tiers(&root.0);
    let mut reg = Registry::open(&t).unwrap();
    let lib = reg
        .create(&t.user.join("edited.ewlib"), "edited", "Edited TIA", "CC0-1.0")
        .unwrap();
    let edited = fixture_profile(lib, "siemens-tia-intl", |v| {
        v["profile"] = json!({"id": "siemens-tia-intl", "version": "2027.1"});
        v["render"]["drawing"] = json!("");
        v["mnemonics"]["area"]["I"] = json!("E");
        v["mnemonics"]["area"]["Q"] = json!("A");
    });
    reg.write(lib, &[edited]).unwrap();
    let dir = root.0.join("J-E-2416.ew");
    copy_dir(Path::new(GOLDEN_A), &dir);
    let mut s = open(&dir, &root.0, Some(t));
    assert_eq!(module_texts(&s)[1], "Q0.1", "the built-in before the embed");
    let copy = s.libraries().documents(lib, ew_schema::Kind::PlcProfile);
    s.execute(&AnyCommand {
        name: "library.embed".into(),
        params: json!({ "copies": copy }),
    })
    .unwrap();

    assert_eq!(&module_texts(&s)[..2], ["A0.0", "A0.1"]);
    let module = s.resolve_reference("tag:-201U3").unwrap();
    let channel = s.query().get::<ChannelAddresses>(Id::<Device>::from_uuid(module))[2]
        .channel
        .as_uuid();
    let set = |s: &mut Session, text: &str| {
        s.execute(&AnyCommand {
            name: "fields.set".into(),
            params: json!({"kind": "channel", "targets": [channel.to_string()], "path": "address", "value": text}),
        })
    };
    set(&mut s, "A4.7").unwrap();
    assert_eq!(module_texts(&s)[2], "A4.7");
    let e = set(&mut s, "Q4.6").unwrap_err();
    assert_eq!(
        refusal(&e),
        "`Q4.6` is not an address of profile siemens-tia-intl@2027.1: expected an address like A0.1"
    );
}
