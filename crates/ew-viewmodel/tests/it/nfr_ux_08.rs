// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! NFR-UX-08 (SC-8): the company-standard wizard with the pilot's choices (its defaults, and "As in the
//! sample project" for the text sizes, D11) writes a basic project; a project made from it with `file.new` carries the
//! effective conventions of the sample project (Golden A), member by member, except the members of [`EXCLUDED`]. With
//! "Readable (new projects)", the default, the project carries the built-in basic project's text styles.

use crate::common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use common::{GOLDEN_A, Tmp, done};
use ew_viewmodel::shell::{Mode, ShellConfig, ShellModel};
use serde_json::{Value, json};

/// The members the comparison leaves out, each with its reason (D10).
const EXCLUDED: &[(&str, &str)] = &[
    (
        "parties and master data",
        "the project's own: its customer, numbers and dates",
    ),
    (
        "report blocks 903 to 905, and the title of 908",
        "the per-strip and per-device blocks: their selection names project records (908's names its station)",
    ),
    (
        "the location nodes",
        "the project's own cabinets and process field (doc 20 §4.3); the wizard's location field is empty by default",
    ),
    (
        "the title-block cell mapping",
        "FT-365: no command maps a form's cells yet",
    ),
];

/// Every JSON Lines record of `files` under `dir` (a file, or a directory of files).
fn records(dir: &Path, files: &[&str]) -> Vec<Value> {
    let mut out = Vec::new();
    for f in files {
        let p = dir.join(f);
        let mut paths: Vec<PathBuf> = if p.is_dir() {
            std::fs::read_dir(&p)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect()
        } else {
            vec![p]
        };
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            out.extend(
                text.lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| serde_json::from_str::<Value>(l).unwrap()),
            );
        }
    }
    out
}

/// The key of a form copy of a project directory (`lib/forms/<id>.json`'s `meta.key`).
fn form_key(dir: &Path, id: &Value) -> String {
    let Some(id) = id.as_str() else {
        return "(none)".to_owned();
    };
    let text = std::fs::read_to_string(dir.join("lib/forms").join(format!("{id}.json"))).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    v["meta"]["key"].as_str().unwrap().to_owned()
}

/// The effective conventions of a project directory, by member: the settings the derivation reads (an absent member
/// is the derivation profile's default on either side), the derivation profile, the frame of each page type (the form
/// its pages name, else the frame defaults), the sections, the structure aspect signs and the report blocks.
fn conventions(dir: &Path, page_types: &BTreeSet<String>) -> Vec<(String, Value)> {
    let project = &records(dir, &["project.jsonl"])[0];
    let settings = &project["settings"];
    let mut out: Vec<(String, Value)> = [
        "identifier_set",
        "numbering",
        "xref_format",
        "colour_profile",
        "size_profile",
        "text_styles",
        "formatting",
        "revision_format",
        "check_scheme",
    ]
    .iter()
    .map(|m| (format!("settings.{m}"), settings[*m].clone()))
    .collect();
    out.push(("derivation_profile".into(), project["derivation_profile"].clone()));
    let pages: Vec<Value> = records(dir, &["pages"])
        .into_iter()
        .filter(|r| r["$k"] == "page")
        .collect();
    let frames = &project["frames"];
    for ty in page_types {
        let named: BTreeSet<String> = pages
            .iter()
            .filter(|p| p["type"] == ty.as_str() && p.get("form").is_some())
            .map(|p| form_key(dir, &p["form"]))
            .collect();
        let frame = if named.is_empty() {
            let per_type = frames["per_page_type"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|e| e["page_type"] == ty.as_str())
                .map(|e| e["form"].clone());
            form_key(dir, &per_type.unwrap_or_else(|| frames["default"].clone()))
        } else {
            named.into_iter().collect::<Vec<_>>().join(", ")
        };
        out.push((format!("frame of {ty} pages"), Value::from(frame)));
    }
    let structure = records(dir, &["structure.jsonl"]);
    let mut sections: Vec<Value> = structure
        .iter()
        .filter(|r| r["$k"] == "section")
        .map(|s| json!([s["number"], s["folio_range"], s["divider"]]))
        .collect();
    sections.sort_by_key(|s| s[0].as_u64());
    out.push(("sections".into(), Value::from(sections)));
    // The function aspect's nodes (the location nodes are the project's own, EXCLUDED); the identifier set gives
    // every aspect its sign.
    let function: BTreeSet<String> = structure
        .iter()
        .filter(|r| r["$k"] == "structure_node" && r["aspect"] == "function")
        .filter_map(|n| n["code"].as_str().map(str::to_owned))
        .collect();
    out.push(("structure: the function aspect's nodes".into(), json!(function)));
    let mut blocks: Vec<Value> = records(dir, &["report-blocks.jsonl"])
        .into_iter()
        .filter(|b| b["$k"] == "report_block")
        .filter(|b| !(903..=905).contains(&b["first_folio"].as_u64().unwrap_or(0)))
        .map(|b| {
            let title = if b["first_folio"] == 908 {
                Value::Null
            } else {
                b["title"].clone()
            };
            json!([
                b["source"]["kind"],
                b["first_folio"],
                form_key(dir, &b["form"]),
                title
            ])
        })
        .collect();
    blocks.sort_by_key(|b| b[1].as_u64());
    out.push(("report blocks".into(), Value::from(blocks)));
    out
}

/// A project made with `file.new` from the basic project the wizard writes with `text_styles` = `choice`.
fn new_project(tmp: &Tmp, choice: &str) -> PathBuf {
    let mut shell = ShellModel::new(
        Mode::Headless,
        ShellConfig::in_dir(&tmp.0.join(format!("config-{choice}")), "0.0.0-test"),
    );
    done(&mut shell, "tools.company_standard", json!({}));
    done(
        &mut shell,
        "wizard.set",
        json!({"field": "text_styles", "value": choice}),
    );
    let basic = tmp.0.join(format!("company-{choice}.ew"));
    done(
        &mut shell,
        "wizard.finish",
        json!({ "dir": basic.to_string_lossy() }),
    );
    let new = tmp.0.join(format!("job-{choice}.ew"));
    done(&mut shell, "file.new", json!({ "dir": new.to_string_lossy() }));
    shell.shutdown();
    new
}

#[test]
fn golden_from_the_basic_project() {
    let tmp = Tmp::new("nfr-ux-08");
    let golden = Path::new(GOLDEN_A);
    let types: BTreeSet<String> = records(golden, &["pages"])
        .into_iter()
        .filter(|r| r["$k"] == "page")
        .filter_map(|p| p["type"].as_str().map(str::to_owned))
        .collect();
    let new = new_project(&tmp, "sample");
    let (want, got) = (conventions(golden, &types), conventions(&new, &types));
    let differ: Vec<String> = want
        .iter()
        .zip(&got)
        .filter(|(w, g)| w != g)
        .map(|(w, g)| format!("{}: sample {} | basic {}", w.0, w.1, g.1))
        .collect();
    assert_eq!(differ, Vec::<String>::new(), "excluded: {EXCLUDED:?}");
}

/// SC-8: with "Readable (new projects)", the default, the project carries the built-in basic project's text styles.
#[test]
fn the_readable_choice_carries_the_built_in_text_styles() {
    let tmp = Tmp::new("nfr-ux-08-readable");
    let new = new_project(&tmp, "readable");
    let project = &records(&new, &["project.jsonl"])[0];
    assert_eq!(
        project["settings"]["text_styles"],
        serde_json::to_value(ew_viewmodel::ew_api::new_project_text_styles()).unwrap()
    );
}
