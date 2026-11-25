// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The starter content under the category table's flags (`ew_rds::category_flags`): what a symbol's tag prints in
//! and what general code asks of a function category follow the table, and for the starter definitions they read as
//! the older rules did (the prefix `ew.fn.terminal` of a symbol's definition key, the category names written in the
//! code). Each starter item that reads differently is printed, before and after.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ew_schema::{FnDef, LibCopy, LibDocument, SlotKind, SymbolDef, draws_terminal, slot_default_style};

/// The library documents of `content/<folder>`, its sub-folders included.
fn documents(folder: &str) -> Vec<LibDocument> {
    let mut dirs = vec![
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../content")
            .join(folder),
    ];
    let mut out = Vec::new();
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "json") {
                out.push(LibDocument::from_json(&std::fs::read_to_string(&path).unwrap()).unwrap());
            }
        }
    }
    out
}

fn fndefs() -> BTreeMap<ew_ids::Uuid, LibCopy<FnDef>> {
    documents("fndefs")
        .into_iter()
        .filter_map(|d| match d {
            LibDocument::Fndef(c) => Some((c.item_id.as_uuid(), c)),
            _ => None,
        })
        .collect()
}

/// Every starter symbol with a tag prints it in the terminal style exactly when its definition's key began with
/// `ew.fn.terminal`, as before: the strip segment, a terminal category too, keeps the main tag style.
#[test]
fn a_starter_symbols_tag_prints_in_the_style_it_printed_in_before() {
    let fndefs = fndefs();
    let mut changed = Vec::new();
    let mut terminals = 0;
    for doc in documents("symbols") {
        let LibDocument::Symbol(LibCopy { item: symbol, .. }) = doc else {
            continue;
        };
        let symbol: SymbolDef = symbol;
        if !symbol.text_slots.iter().any(|s| s.kind == SlotKind::Tag) {
            continue;
        }
        let key = symbol
            .fndef
            .extra
            .get("key")
            .and_then(|k| k.as_str())
            .unwrap_or_default();
        let before = slot_default_style(key.starts_with("ew.fn.terminal"), &SlotKind::Tag);
        let def = &fndefs[&symbol.fndef.item.as_uuid()].item;
        let after = slot_default_style(draws_terminal(&symbol, def), &SlotKind::Tag);
        terminals += usize::from(after == "terminal");
        if before != after {
            changed.push(format!("{key}: {before} -> {after}"));
        }
    }
    println!("starter symbols whose tag style changed: {changed:?}");
    assert!(changed.is_empty(), "{changed:#?}");
    assert!(terminals > 0, "the starter terminals print in the terminal style");
}

/// The category names the code wrote for terminals, protective devices and field equipment are the categories the
/// table flags, for every starter definition.
#[test]
fn a_starter_definitions_category_flags_read_as_the_names_the_code_wrote() {
    let field = [
        "SensingPosition",
        "SensingPressure",
        "SensingTemperature",
        "FluidSensor",
        "CylinderFluid",
        "MotorRotary",
        "ValveTerminal",
        "ValveTerminalNode",
        "ValveTerminalModule",
    ];
    let mut changed = Vec::new();
    for (_, copy) in fndefs() {
        let category = copy.item.category.0.as_str();
        let before = (
            matches!(category, "Terminal" | "TerminalEarth"),
            category.starts_with("Protection"),
            field.contains(&category),
        );
        let flags = ew_rds::category_flags(category);
        let after = (flags.terminal, flags.protection, flags.field);
        if before != after {
            changed.push(format!("{category}: {before:?} -> {after:?}"));
        }
    }
    println!("starter definitions whose category flags changed: {changed:?}");
    assert!(changed.is_empty(), "{changed:#?}");
}
