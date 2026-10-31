// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The `$defs` of the published parameter schemas: one definition per name across all commands, and the
//! ew-commands proxies keep their own `Cmd*` names and the patterns of the ew-schema counterparts.

use std::collections::BTreeMap;

use ew_commands::CommandRegistry;
use serde_json::Value;

type Defs = BTreeMap<String, Vec<(String, Value)>>;

/// Every `$defs` member of every schema file: name -> list of (file, definition).
fn all_defs() -> Defs {
    let mut out: Defs = BTreeMap::new();
    for (file, text) in CommandRegistry::builtin().schema_files() {
        if !file.ends_with(".schema.json") {
            continue;
        }
        let doc: Value = serde_json::from_str(&text).expect("schema file is JSON");
        if let Some(defs) = doc.get("$defs").and_then(Value::as_object) {
            for (name, def) in defs {
                out.entry(name.clone())
                    .or_default()
                    .push((file.clone(), def.clone()));
            }
        }
    }
    out
}

#[test]
fn every_defs_name_has_one_definition_across_all_schemas() {
    for (name, defs) in all_defs() {
        let (first_file, first) = &defs[0];
        for (file, def) in &defs[1..] {
            assert_eq!(first, def, "$defs/{name} differs between {first_file} and {file}");
        }
    }
}

/// The `pattern` of the first definition named `name`; panics when a definition has none.
fn pattern_of(defs: &Defs, name: &str) -> Option<String> {
    let (file, def) = defs.get(name)?.first()?;
    let p = def
        .get("pattern")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("$defs/{name} in {file} has no string pattern"));
    Some(p.to_owned())
}

#[test]
fn ident_dec3_and_uuid_defs_carry_their_pattern() {
    let defs = all_defs();
    for (plain, cmd) in [("Ident", "CmdIdent"), ("Dec3", "CmdDec3"), ("Uuid", "CmdUuid")] {
        let p_cmd = pattern_of(&defs, cmd);
        assert!(p_cmd.is_some(), "$defs/{cmd} is never emitted");
        if let Some(p_plain) = pattern_of(&defs, plain) {
            assert_eq!(p_plain, p_cmd.unwrap(), "{cmd} pattern differs from {plain}");
        }
    }
    // The MlText proxy constrains its keys like ew-schema's.
    let cmd_ml = &defs.get("CmdMlText").expect("CmdMlText is emitted")[0].1;
    assert!(
        cmd_ml.to_string().contains("propertyNames"),
        "CmdMlText has no propertyNames pattern"
    );
    if let Some(ml) = defs.get("MlText") {
        let key = |v: &Value| v["anyOf"][1]["propertyNames"].clone();
        assert_eq!(
            key(&ml[0].1),
            key(cmd_ml),
            "CmdMlText propertyNames differ from MlText"
        );
    }
}
