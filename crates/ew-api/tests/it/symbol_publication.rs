// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! A symbol draft's publication: its own item identity, keys held by other items, the owned function
//! definition's version, unknown members and the portable export.

use std::path::Path;

use ew_api::library::Registry;
use ew_api::symbols::{self, SymbolDraft, schema::LibDocument};
use ew_derive::ew_ids::Uuid;
use serde_json::json;

use crate::symbol_authoring::{block, library};

/// The generation the library's manifest on disk holds, which the registry's agrees with.
fn generation(registry: &Registry, id: Uuid) -> u64 {
    let info = registry.libraries().into_iter().find(|l| l.id == id).unwrap();
    let manifest = std::fs::read(info.path.unwrap().join("library.json")).unwrap();
    let on_disk = serde_json::from_slice::<serde_json::Value>(&manifest).unwrap()["generation"]
        .as_u64()
        .unwrap();
    assert_eq!(on_disk, info.generation);
    on_disk
}

/// The library's symbol with the key `key`.
fn symbol(registry: &Registry, id: Uuid, key: &str) -> ew_schema::LibCopy<ew_schema::SymbolDef> {
    registry
        .documents(id, symbols::schema::Kind::Symbol)
        .into_iter()
        .find_map(|d| match d {
            LibDocument::Symbol(s) if s.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => Some(s),
            _ => None,
        })
        .unwrap()
}

/// The library's function definition with the key `key`.
fn function(registry: &Registry, id: Uuid, key: &str) -> ew_schema::LibCopy<ew_schema::FnDef> {
    registry
        .documents(id, symbols::schema::Kind::Fndef)
        .into_iter()
        .find_map(|d| match d {
            LibDocument::Fndef(f) if f.item.meta.key.as_ref().is_some_and(|k| k.as_str() == key) => Some(f),
            _ => None,
        })
        .unwrap()
}

fn line() -> ew_schema::Primitive {
    serde_json::from_value(json!({"line":{"from":[0,0],"to":[4_000_000,0],"stroke":{"pen":"p026"}}})).unwrap()
}

#[test]
fn a_second_draft_on_the_same_key_is_refused_and_the_first_symbol_stays() {
    let (dir, mut registry, id) = library("symbol-two-drafts");
    let mut first = block();
    let added = first.publish(&mut registry, id).unwrap();
    let identity = first.identity.clone().expect("minted at the first publication");
    assert_eq!(identity.symbol.as_uuid().get_version_num(), 7);
    assert_eq!(identity.fndef.unwrap().as_uuid().get_version_num(), 7);
    assert!(added.added.contains(&identity.symbol.as_uuid()));
    let held = generation(&registry, id);
    // Another drawing on the same key, before and after the version bump the immutability refusal asks for.
    let mut second = block();
    second.symbol.graphics = vec![line()];
    for version in ["1.0.0", "1.0.1"] {
        second.version = version.into();
        let refused = second.publish(&mut registry, id).unwrap_err();
        assert_eq!(
            refused.to_string(),
            "The library already holds a different symbol with the key example.power. Choose another key."
        );
        assert_eq!(second.identity, None, "a refused publication keeps no identity");
        assert_eq!(generation(&registry, id), held);
    }
    assert_eq!(
        symbol(&registry, id, "example.power").item.graphics,
        first.symbol.graphics
    );
    // The draft that published the symbol updates it.
    first.version = "1.0.1".into();
    first.symbol.graphics.push(line());
    let updated = first.publish(&mut registry, id).unwrap();
    assert_eq!(updated.updated, vec![identity.symbol.as_uuid()]);
    assert_eq!(first.identity.as_ref(), Some(&identity));
    // So does a draft forked from it under its own key; under another key it is a new item.
    let published = symbol(&registry, id, "example.power");
    let same = SymbolDraft::fork(&registry, &published, "example.power").unwrap();
    assert_eq!(same.identity, Some(identity));
    let copy = SymbolDraft::fork(&registry, &published, "example.copy").unwrap();
    assert_eq!(copy.identity, None);
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_symbol_keyed_like_a_function_definition_is_its_own_item() {
    let (dir, mut registry, id) = library("symbol-kind-identity");
    block().publish(&mut registry, id).unwrap();
    let mut named = block();
    named.key = "example.power.function".into();
    named.publish(&mut registry, id).unwrap();
    let power = symbol(&registry, id, "example.power");
    assert!(matches!(
        registry.resolve(&power.item.fndef).unwrap(),
        LibDocument::Fndef(f) if f.item.meta.key.as_ref().unwrap().as_str() == "example.power.function"
    ));
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_function_key_held_by_another_item_is_refused_and_nothing_is_written() {
    let (dir, mut registry, id) = library("symbol-function-key");
    let mut other = block();
    other.key = "example.other".into();
    let mut docs = other.documents(id).unwrap();
    docs.retain(|d| matches!(d, LibDocument::Fndef(_)));
    // The function definition of a symbol keyed `example`, without that symbol.
    let LibDocument::Fndef(f) = &mut docs[0] else {
        panic!("function")
    };
    f.item.meta.key = Some("example.function".parse().unwrap());
    let hash = ew_store::ew_format::library_copy_hash(&docs[0]).unwrap();
    if let LibDocument::Fndef(f) = &mut docs[0] {
        f.sha256 = ew_schema::Sha256(hash);
    }
    registry.write(id, &docs).unwrap();
    let held = generation(&registry, id);
    let mut draft = block();
    draft.key = "example".into();
    let refused = draft.publish(&mut registry, id).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "The library already holds a different function definition with the key example.function. Choose another \
         symbol key."
    );
    assert_eq!(generation(&registry, id), held);
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_refused_publication_leaves_the_library_generation_unchanged() {
    let (dir, mut registry, id) = library("symbol-refused-generation");
    let held = generation(&registry, id);
    let mut value = serde_json::to_value(block()).unwrap();
    value["symbol"]["cps"][0]["pos"] = json!([4_500_000, 4_000_000]);
    let mut off_grid = SymbolDraft::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(matches!(
        off_grid.publish(&mut registry, id),
        Err(symbols::Error::Invalid(_))
    ));
    let mut published = block();
    published.publish(&mut registry, id).unwrap();
    let after = generation(&registry, id);
    assert_eq!(after, held + 1);
    // Changed content under the version the library holds: the writer's immutability refusal.
    published.symbol.graphics.push(line());
    assert!(matches!(
        published.publish(&mut registry, id),
        Err(symbols::Error::Library(_))
    ));
    assert_eq!(generation(&registry, id), after);
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_unchanged_function_definition_keeps_its_version() {
    let (dir, mut registry, id) = library("symbol-function-version");
    let mut draft = block();
    draft.publish(&mut registry, id).unwrap();
    // A new drawing at a new version: the function definition stays at 1.0.0, so references to it still resolve.
    draft.version = "1.0.1".into();
    draft.symbol.graphics.push(line());
    draft.publish(&mut registry, id).unwrap();
    let held = function(&registry, id, "example.power.function");
    assert_eq!(held.version, "1.0.0");
    assert_eq!(
        symbol(&registry, id, "example.power").item.fndef.version_req,
        "=1.0.0"
    );
    // A changed function definition takes the draft's version.
    draft.version = "1.1.0".into();
    let mut cp = draft.function.cps.iter().next().unwrap().clone();
    cp.designation = "L+".parse().unwrap();
    draft.function.cps.insert(cp);
    draft.publish(&mut registry, id).unwrap();
    assert_eq!(function(&registry, id, "example.power.function").version, "1.1.0");
    assert_eq!(
        symbol(&registry, id, "example.power").item.fndef.version_req,
        "=1.1.0"
    );
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn unknown_members_block_publication_and_import_extension_keys_do_not() {
    let mut value = serde_json::to_value(block()).unwrap();
    value["symbol"]["text_slot"] = json!([]);
    value["symbol"]["meta"]["licence_url"] = json!("");
    value["function"]["cps"][1]["a/b"] = json!(1);
    value["symbol"]["cps"][2]["qet.uuid"] = json!("kept");
    value["function"]["source_ref"] = json!({});
    let draft = SymbolDraft::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    let issue = |path: &str, detail: &str| symbols::Issue {
        code: "symbol-unknown-member",
        path: path.into(),
        detail: detail.into(),
    };
    assert_eq!(
        draft.validate(),
        vec![
            issue("/symbol/text_slot", "text_slot"),
            issue("/symbol/meta/licence_url", "licence_url"),
            issue("/function/cps/1/a~1b", "a/b"),
        ]
    );
    assert!(matches!(
        draft.documents(symbols::DRAFT_LIBRARY),
        Err(symbols::Error::Invalid(_))
    ));
    let qet = symbols::import_qet(
        include_bytes!("../../../ew-import-qet/tests/fixtures/contact_no.elmt"),
        "contact.elmt",
        "example.contact",
    )
    .unwrap();
    assert!(qet.validate().iter().all(|i| i.code != "symbol-unknown-member"));
}

#[test]
fn unknown_members_of_nested_objects_block_publication() {
    let mut value = serde_json::to_value(block()).unwrap();
    value["symbol"]["graphics"][0]["rect"]["glow"] = json!(1);
    value["symbol"]["graphics"][0]["rect"]["stroke"]["wdth"] = json!(1);
    value["symbol"]["text_slots"][0]["visibl"] = json!(true);
    value["symbol"]["arrangements"] = json!([{"name":"full","slots":[0],"slotz":[]}]);
    value["symbol"]["link_points"] = json!([{"name":"a","pos":[0,0],"posn":[0,0]}]);
    value["function"]["internal"] =
        json!([{"a":1,"b":2,"kind":{"pass_through":{"opens":true,"open":true}},"note":""}]);
    value["function"]["flags"] = json!({"n_isolation":true,"n_isolaton":true});
    let mut reference = value["symbol"]["fndef"].clone();
    reference["pinned"] = json!(true);
    // A deprecation's object and its successor's reference.
    let mut successor = reference.clone();
    successor.as_object_mut().unwrap().remove("pinned");
    successor["pined"] = json!(true);
    value["symbol"]["meta"]["status"] = json!({"deprecated": {"successor": successor, "reason": "old"}});
    value["function_ref"] = reference;
    let draft = SymbolDraft::parse(&serde_json::to_vec(&value).unwrap()).unwrap();
    let paths: Vec<_> = draft
        .validate()
        .into_iter()
        .filter(|i| i.code == "symbol-unknown-member")
        .map(|i| i.path)
        .collect();
    assert_eq!(
        paths,
        [
            "/symbol/graphics/0/rect/glow",
            "/symbol/graphics/0/rect/stroke/wdth",
            "/symbol/text_slots/0/visibl",
            "/symbol/arrangements/0/slotz",
            "/symbol/link_points/0/posn",
            "/function/internal/0/note",
            "/function/internal/0/kind/pass_through/open",
            "/function/flags/n_isolaton",
            "/function_ref/pinned",
            "/symbol/meta/status/deprecated/reason",
            "/symbol/meta/status/deprecated/successor/pined",
        ]
    );
    // Each pointer names the member in the draft.
    let doc = serde_json::to_value(&draft).unwrap();
    assert!(paths.iter().all(|p| doc.pointer(p).is_some()), "{paths:?}");
}

#[test]
fn a_draft_under_another_key_than_its_items_publishes_new_ones_and_both_stay() {
    let (dir, mut registry, id) = library("symbol-renamed-draft");
    let mut a = block();
    a.publish(&mut registry, id).unwrap();
    let first = a.identity.clone().unwrap();
    // The same draft under key B, at the version the immutability refusal would ask for.
    let mut b = a.clone();
    b.key = "example.other".into();
    b.version = "1.0.1".into();
    b.symbol.graphics.push(line());
    let report = b.publish(&mut registry, id).unwrap();
    let second = b.identity.clone().unwrap();
    assert_ne!(second.symbol, first.symbol);
    assert_ne!(second.fndef, first.fndef);
    let mut added = vec![second.symbol.as_uuid(), second.fndef.unwrap().as_uuid()];
    added.sort();
    assert_eq!(report.added, added);
    assert_eq!(report.updated, Vec::<Uuid>::new());
    let held_a = symbol(&registry, id, "example.power");
    assert_eq!(held_a.item_id.as_uuid(), first.symbol.as_uuid());
    assert_eq!(held_a.item.graphics, a.symbol.graphics);
    assert_eq!(held_a.version, "1.0.0");
    assert_eq!(
        function(&registry, id, "example.power.function")
            .item_id
            .as_uuid(),
        first.fndef.unwrap().as_uuid()
    );
    assert_eq!(
        symbol(&registry, id, "example.other").item.graphics,
        b.symbol.graphics
    );
    // A draft file carrying A's identity under a third key does the same; B's draft now updates B.
    let mut foreign = block();
    foreign.key = "example.third".into();
    foreign.identity = Some(first.clone());
    foreign.publish(&mut registry, id).unwrap();
    assert_ne!(foreign.identity, Some(first));
    b.version = "1.0.2".into();
    b.symbol.graphics.push(line());
    assert_eq!(
        b.publish(&mut registry, id).unwrap().updated,
        vec![second.symbol.as_uuid()]
    );
    for key in ["example.power", "example.other", "example.third"] {
        symbol(&registry, id, key);
    }
    drop(registry);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn opening_a_zip_reads_it_as_an_archive_and_bytes_that_are_no_text_are_refused_in_words() {
    let dir = ew_testdir::fresh_dir("symbol-zip");
    let zip = dir.join("symbols.zip");
    std::fs::write(&zip, b"PK\x03\x04 not a real archive").unwrap();
    let refused = symbols::open(&zip, "example").unwrap_err();
    assert_eq!(
        refused.to_string(),
        "The EPLAN archive is damaged, encrypted or unsupported."
    );
    let refused = SymbolDraft::parse(b"\xff\xfe binary").unwrap_err();
    assert_eq!(refused.to_string(), "The symbol document is not valid.");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_empty_key_is_named_before_publication() {
    let draft = SymbolDraft::new("", "Untitled").unwrap();
    assert_eq!(draft.symbol.meta.key, None);
    assert_eq!(
        draft.validate()[0],
        symbols::Issue {
            code: "symbol-key-required",
            path: "/key".into(),
            detail: String::new(),
        }
    );
    assert_eq!(symbols::file_key(Path::new("dir/contact no.elmt")), "contact_no");
    assert_eq!(symbols::file_key(Path::new("power.block.symbol.json")), "power");
    assert_eq!(symbols::file_key(Path::new(".elmt")), "_elmt");
}

#[test]
fn export_places_a_new_file_and_refuses_an_existing_one() {
    let dir = ew_testdir::fresh_dir("symbol-export");
    let path = dir.join("power.symbol.json");
    let draft = block();
    symbols::export(&draft, &path).unwrap();
    assert_eq!(SymbolDraft::parse(&std::fs::read(&path).unwrap()).unwrap(), draft);
    let mut other = block();
    other.version = "2.0.0".into();
    let refused = symbols::export(&other, &path).unwrap_err();
    assert!(matches!(&refused, symbols::Error::Input(code) if code == "symbol-export-exists"));
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["power.symbol.json"], "no temporary file is left");
    assert_eq!(SymbolDraft::parse(&std::fs::read(&path).unwrap()).unwrap(), draft);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn export_without_hard_links_renames_after_checking_and_syncs_the_folder() {
    use ew_store::mem::{DirSync, MemVfs};
    let vfs = MemVfs::new(DirSync::Explicit);
    vfs.put_dir(Path::new("/out"));
    vfs.set_noreplace_unsupported(true);
    vfs.set_trace(true);
    let draft = block();
    let path = Path::new("/out/power.symbol.json");
    symbols::export_with(&vfs, &draft, path).unwrap();
    let ops: Vec<String> = vfs
        .trace()
        .iter()
        .map(|l| l.split(' ').nth(1).unwrap().to_owned())
        .collect();
    assert_eq!(
        ops,
        [
            "create_new",
            "sync",
            "rename_noreplace",
            "stat",
            "rename",
            "sync_dir",
            "remove_file"
        ]
    );
    let files = vfs.files(Path::new("/out"));
    assert_eq!(files.keys().collect::<Vec<_>>(), ["power.symbol.json"]);
    assert_eq!(SymbolDraft::parse(&files["power.symbol.json"]).unwrap(), draft);
    // The name is now taken: the fallback refuses and removes its temporary file.
    let refused = symbols::export_with(&vfs, &block(), path).unwrap_err();
    assert!(matches!(&refused, symbols::Error::Input(code) if code == "symbol-export-exists"));
    assert_eq!(
        vfs.files(Path::new("/out")).keys().collect::<Vec<_>>(),
        ["power.symbol.json"]
    );
}

#[test]
fn a_failed_export_never_removes_an_earlier_identical_file() {
    use ew_store::mem::{DirSync, Fault, MemVfs};
    let vfs = MemVfs::new(DirSync::Explicit);
    let path = Path::new("/out/power.symbol.json");
    let draft = block();
    let earlier = serde_json::to_vec_pretty(&draft).unwrap();
    vfs.put(path, &earlier);
    vfs.reset_calls();
    // The temporary file's sync (call 2) fails: this call placed nothing.
    vfs.set_fault(Fault::Error {
        at: 2,
        kind: std::io::ErrorKind::Other,
    });
    assert!(symbols::export_with(&vfs, &draft, path).is_err());
    let files = vfs.files(Path::new("/out"));
    assert_eq!(files.keys().collect::<Vec<_>>(), ["power.symbol.json"]);
    assert_eq!(files["power.symbol.json"], earlier);
}
