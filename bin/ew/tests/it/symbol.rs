// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ew symbol` through the built binary: its five subcommands, their exit codes, a library created
//! and then reused, and a key that another symbol holds.

use std::path::Path;

use serde_json::{Value, json};

use crate::support::{EwTmp, Tmp, args, stderr, stdout};

/// A valid one-pin draft keyed `key`, from the template.
fn valid(tmp: &Tmp, key: &str) -> Value {
    let o = tmp.ew(&args([&"symbol", &"template", &key, &"Contact"]));
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let mut draft: Value = serde_json::from_str(&stdout(&o)).unwrap();
    draft["symbol"]["graphics"] = json!([{"line":{"from":[0,0],"to":[0,4_000_000],"stroke":{"pen":"p026"}}}]);
    draft["symbol"]["cps"] = json!([{"idx":1,"fn_cp":1,"pos":[0,0],"dir":"up"}]);
    draft["function"]["cps"] = json!([{"idx":1,"designation":"1"}]);
    draft
}

fn write(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn template_schema_and_validate_exit_as_documented() {
    let tmp = Tmp::new("ew-tables");
    let o = tmp.ew(&args([&"symbol", &"template", &"example.contact", &"Contact"]));
    assert_eq!(o.status.code(), Some(0));
    let template: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(template["format"], "ew.symbol/1");
    assert_eq!(template["key"], "example.contact");
    let o = tmp.ew(&args([&"symbol", &"schema"]));
    assert_eq!(o.status.code(), Some(0));
    let schema: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(schema["properties"]["format"]["const"], "ew.symbol/1");
    assert!(schema["properties"]["identity"].is_object());
    // The template has no drawing yet: exit 1 with the issue.
    let file = tmp.0.join("template.symbol.json");
    write(&file, &template);
    let o = tmp.ew(&args([&"symbol", &"validate", &file]));
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&o)).unwrap(),
        json!({"format":"ew.symbol-validation/1","valid":false,"issues":[
            {"code":"symbol-graphics-required","path":"/symbol/graphics","detail":"example.contact"}]})
    );
    write(&file, &valid(&tmp, "example.contact"));
    let o = tmp.ew(&args([&"symbol", &"validate", &file]));
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&o)).unwrap(),
        json!({"format":"ew.symbol-validation/1","valid":true,"issues":[]})
    );
    // Unreadable input: exit 2.
    std::fs::write(&file, b"{").unwrap();
    let o = tmp.ew(&args([&"symbol", &"validate", &file]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(stdout(&o), "");
}

#[test]
fn import_takes_the_file_name_as_its_key_unless_one_is_given() {
    let tmp = Tmp::new("ew-tables");
    let file = tmp.0.join("contact no.elmt");
    std::fs::write(
        &file,
        include_bytes!("../../../../crates/ew-import-qet/tests/fixtures/contact_no.elmt"),
    )
    .unwrap();
    for (extra, key) in [(None, "contact_no"), (Some("example.qet"), "example.qet")] {
        let mut list = args([&"symbol", &"import", &file]);
        if let Some(k) = extra {
            list.extend(args([&"--key", &k]));
        }
        let o = tmp.ew(&list);
        assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
        let drafts: Value = serde_json::from_str(&stdout(&o)).unwrap();
        assert_eq!(drafts.as_array().unwrap().len(), 1);
        assert_eq!(drafts[0]["key"], key);
    }
    // Input that is no element: exit 2, nothing printed.
    let broken = tmp.0.join("broken.elmt");
    std::fs::write(&broken, b"not an element").unwrap();
    let o = tmp.ew(&args([&"symbol", &"import", &broken]));
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(stdout(&o), "");
}

#[test]
fn publish_creates_then_reuses_the_library_and_refuses_a_key_another_symbol_holds() {
    let tmp = Tmp::new("ew-tables");
    let library = tmp.0.join("symbols.ewlib");
    let file = tmp.0.join("contact.symbol.json");
    let publish = |file: &Path| tmp.ew(&args([&"symbol", &"publish", &file, &"--library", &library]));
    // An invalid draft creates no library.
    let mut off_grid = valid(&tmp, "example.contact");
    off_grid["symbol"]["cps"][0]["pos"] = json!([4_500_000, 0]);
    write(&file, &off_grid);
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(1));
    assert!(!library.exists());
    // The first publication creates the library and adds the symbol.
    let mut draft = valid(&tmp, "example.contact");
    write(&file, &draft);
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let first: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(first["result"], "added");
    assert_eq!(first["key"], "example.contact");
    assert!(library.join("library.json").is_file());
    let manifest = || std::fs::read_to_string(library.join("library.json")).unwrap();
    let held = manifest();
    // Without the identity, the same key is another symbol's: refused in words, the library unchanged.
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(2));
    assert_eq!(
        stderr(&o),
        "error: The library already holds a different symbol with the key example.contact. Choose another key.\n"
    );
    assert_eq!(manifest(), held);
    // With the printed identity the draft is the same symbol: unchanged, then updated at a new version.
    draft["identity"] = first["identity"].clone();
    write(&file, &draft);
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&o)).unwrap()["result"],
        "unchanged"
    );
    draft["version"] = json!("1.0.1");
    draft["symbol"]["graphics"][0]["line"]["to"] = json!([0, 8_000_000]);
    write(&file, &draft);
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let updated: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(updated["result"], "updated");
    assert_eq!(updated["identity"], first["identity"]);
}

/// ID-1476 (M25-p6 nit, ID-1474's path): a draft that carries the identity of a published symbol under another key
/// publishes a new symbol: `added`, a new identity printed, and the first symbol stays.
#[test]
fn publish_under_another_key_adds_a_symbol_with_a_new_identity() {
    let tmp = Tmp::new("ew-tables");
    let library = tmp.0.join("symbols.ewlib");
    let file = tmp.0.join("contact.symbol.json");
    let publish = |file: &Path| tmp.ew(&args([&"symbol", &"publish", &file, &"--library", &library]));
    let mut draft = valid(&tmp, "example.first");
    write(&file, &draft);
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let first: Value = serde_json::from_str(&stdout(&o)).unwrap();
    draft["identity"] = first["identity"].clone();
    draft["key"] = json!("example.second");
    write(&file, &draft);
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let second: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(second["result"], "added");
    assert_eq!(second["key"], "example.second");
    assert!(second["identity"]["symbol"].is_string());
    assert_ne!(second["identity"]["symbol"], first["identity"]["symbol"]);
    // The first symbol stays: its draft with its identity is unchanged.
    draft["identity"] = first["identity"].clone();
    draft["key"] = json!("example.first");
    write(&file, &draft);
    let o = publish(&file);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&o)).unwrap()["result"],
        "unchanged"
    );
}
