// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! FT-1860: a parts table whose function template names a function definition of the project's own library (a held
//! FnDef with a key the starter has not) is read and imported, so a part can carry a function drawn by a symbol of the
//! project's own; before, the parts import refused it as an unknown FnDef. A key no FnDef has stays refused.

use ew_api::import::{PARTS_IMPORT, read_parts_tables};
use ew_api::{AnyCommand, Session, SessionOptions};
use ew_ids::{Id, Uuid};
use ew_schema::{FnDef, LibDocument};
use serde_json::json;

/// The key of the person's own function definition.
const OWN: &str = "own.fn.signal";

/// A FnDef of a library of the person's own, keyed [`OWN`], with its copy identity and hash.
fn own_fndef() -> LibDocument {
    let (library, item) = (Uuid::from_u128(0x0e11), Uuid::from_u128(0x0e12));
    let id: Id<FnDef> = ew_ids::v5::lib_copy_id(library, item);
    let mut doc = LibDocument::from_value(json!({
        "$k": "fndef",
        "id": id.to_string(),
        "library_id": library.to_string(),
        "item_id": item.to_string(),
        "version": "1.0.0",
        "sha256": format!("sha256:{}", "0".repeat(64)),
        "meta": {"key": OWN},
        "category": ["Lamp", "Signal", "Own"],
        "contact_kind": "black_box",
        "cps": [{"idx": 0, "designation": "X1"}, {"idx": 1, "designation": "X2"}]
    }))
    .expect("a FnDef");
    let hash = ew_commands::commands::library_copy_hash(&doc).expect("a hash");
    if let LibDocument::Fndef(c) = &mut doc {
        c.sha256 = ew_schema::Sha256(hash);
    }
    doc
}

/// A project that holds [`own_fndef`].
fn session() -> Session {
    let project = serde_json::from_str(
        r#"{"id":"3c1e7a52-9b04-4d6f-8e21-5a7d0c9b2f14","derivation_profile":"2027.1"}"#,
    )
    .expect("a project");
    Session::from_records(project, [], [own_fndef()], SessionOptions::default()).expect("a session")
}

/// A parts table with one part whose template's one function is the FnDef `fndef`, in a folder of its own.
fn table(name: &str, fndef: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ew-parts-own-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the folder");
    std::fs::write(dir.join("parts.csv"), "key,manufacturer,part_number\nLAMP-1,Own,LAMP-1\n").expect("parts.csv");
    std::fs::write(
        dir.join("part_templates.csv"),
        format!("key,part,n,fndef,main,cps\nLAMP-1:0,LAMP-1,0,{fndef},true,\n"),
    )
    .expect("part_templates.csv");
    dir
}

#[test]
fn a_template_names_a_function_definition_of_the_projects_own_library() {
    let mut s = session();
    let dir = table("held", OWN);
    let read = read_parts_tables(&dir.join("parts.csv"), &s);
    std::fs::remove_dir_all(&dir).expect("the folder goes");
    let read = read.expect("the table reads: its FnDef is the project's own");
    let template = &read.copies[0]["function_template"][0];
    assert_eq!(template["fndef"]["key"], OWN, "{template}");
    assert_eq!(template["fndef"]["library"], Uuid::from_u128(0x0e11).to_string());
    let preview = s
        .preview_with(
            &AnyCommand {
                name: PARTS_IMPORT.into(),
                params: read.params(),
            },
            ew_api::Submit::from(ew_api::Origin::Gui),
        )
        .expect("parts.import previews");
    s.commit(preview).expect("parts.import commits");
    assert_eq!(s.query().parts().iter().count(), 1, "the one part of the table");
}

#[test]
fn a_key_no_function_definition_has_stays_refused() {
    let s = session();
    let dir = table("unknown", "own.fn.nowhere");
    let read = read_parts_tables(&dir.join("parts.csv"), &s);
    std::fs::remove_dir_all(&dir).expect("the folder goes");
    let e = read.expect_err("an unknown FnDef");
    assert!(e.to_string().contains("unknown FnDef `own.fn.nowhere`"), "{e}");
}
