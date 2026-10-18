// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The kind registry against doc 05 §2.2: names, ranks, file rules, schema versions, anchors and the
//! personal-data table.

mod common;

use std::collections::BTreeSet;

use common::fill;
use ew_schema::{
    AnchorRef, FileRule, Kind, KindRegistry, KindStatus, PERSONAL_DATA_FIELDS, Record, SCHEMA_VERSIONS,
    kind_schema,
};
use serde_json::Value;

#[test]
fn names_and_ranks_are_unique_and_never_reused() {
    let names: BTreeSet<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(names.len(), Kind::ALL.len());
    let ranks: Vec<u8> = Kind::ALL.iter().filter_map(|k| k.rank()).collect();
    let unique: BTreeSet<u8> = ranks.iter().copied().collect();
    assert_eq!(unique.len(), ranks.len(), "a rank is used twice");
    // Record kinds are listed in rank order.
    assert!(ranks.windows(2).all(|w| w[0] < w[1]));
    for kind in Kind::ALL {
        assert_eq!(Kind::from_name(kind.name()), Some(kind));
    }
}

#[test]
fn the_rank_table_of_doc05_is_complete() {
    let expected: &[(u8, &str)] = &[
        (10, "project"),
        (11, "plc_config_project"),
        (12, "logical_net"),
        (13, "physical_net"),
        (14, "medium"),
        (15, "import_lineage"),
        (20, "structure_node"),
        (21, "section"),
        (22, "external_ref"),
        (30, "page"),
        (31, "device"),
        (32, "function"),
        (33, "representation"),
        (34, "connection"),
        (35, "junction"),
        (36, "conductor"),
        (37, "channel"),
        (38, "potential_def"),
        (39, "net_def"),
        (40, "medium_def"),
        (41, "bond"),
        (42, "device_link"),
        (43, "page_item"),
        (44, "structure_box"),
        (45, "report_block"),
        (46, "macro_instance"),
        (47, "placeholder_group"),
        (50, "layout_space"),
        (51, "enclosure"),
        (52, "mounting_surface"),
        (53, "carrier"),
        (54, "placement"),
        (55, "route_override"),
        (56, "committed_route"),
        (57, "routing_path"),
        (58, "surface_feature"),
        (60, "revision"),
        (61, "waiver"),
        (62, "comment"),
        (63, "transmittal"),
        (70, "exchange_record"),
        (71, "foreign_id"),
        (72, "import_run"),
        (255, "conflict"),
    ];
    let actual: Vec<(u8, &str)> = Kind::ALL
        .iter()
        .filter_map(|k| k.rank().map(|r| (r, k.name())))
        .collect();
    assert_eq!(actual, expected);
    let library: Vec<&str> = Kind::ALL
        .iter()
        .filter(|k| k.is_library())
        .map(|k| k.name())
        .collect();
    assert_eq!(
        library,
        [
            "part",
            "symbol",
            "fndef",
            "form",
            "macro",
            "manufacturer",
            "plc_profile",
            "machine_profile",
            "label_profile"
        ]
    );
}

#[test]
fn every_implemented_kind_has_one_schema_version_and_a_schema() {
    for kind in Kind::ALL {
        let listed = SCHEMA_VERSIONS.iter().filter(|(k, _)| *k == kind).count();
        match kind.status() {
            KindStatus::Implemented => {
                assert_eq!(listed, 1, "{kind} must be listed once in SCHEMA_VERSIONS");
                assert_eq!(kind.schema_version(), 1, "v0 kinds are all at $v 1");
                assert!(kind_schema(kind).is_some(), "{kind} has no schema");
            }
            KindStatus::Reserved => {
                assert_eq!(listed, 0, "{kind} is reserved and has no version yet");
                assert!(kind_schema(kind).is_none());
            }
        }
    }
}

#[test]
fn file_rules_follow_doc05() {
    assert_eq!(KindRegistry::file_rule(Kind::Project), FileRule::ProjectFile);
    assert_eq!(KindRegistry::file_rule(Kind::Section), FileRule::StructureFile);
    assert_eq!(KindRegistry::file_rule(Kind::Device), FileRule::Shard);
    assert_eq!(KindRegistry::file_rule(Kind::Connection), FileRule::PageOrShard);
    assert_eq!(
        KindRegistry::file_rule(Kind::Part),
        FileRule::LibraryDocument("parts")
    );
    assert_eq!(
        FileRule::LibraryDocument("parts").to_string(),
        "lib/parts/<id>.json"
    );
    assert_eq!(
        KindRegistry::file_rule(Kind::Waiver),
        FileRule::RootFile("waivers.jsonl")
    );
}

/// The anchor column of doc 05 §2.2, on golden records.
#[test]
fn anchors_and_concrete_file_rules() {
    let rec = |t: &str| serde_json::from_str::<Record>(&fill(t)).unwrap();
    let uuid = |token: &str| fill(token).parse::<ew_ids::Uuid>().unwrap();

    let page = rec(
        r#"{"$k":"page","id":"@P101","folio":{"number":101},"kind":"multi_line","grid":{"num":7937500,"den":3}}"#,
    );
    assert_eq!(KindRegistry::anchor(&page), Some(uuid("@P101")));
    assert_eq!(page.file_rule(), Some(FileRule::PageFile));

    let device = rec(r#"{"$k":"device","id":"@DEV_G1","tag":{"letter":"G","counter":1}}"#);
    assert_eq!(device.anchor(), Some(uuid("@DEV_G1")));
    assert_eq!(device.file_rule(), Some(FileRule::Shard));

    let function = rec(r#"{"$k":"function","id":"@FN_G1","device":"@DEV_G1","def":"@FNDEF_BB"}"#);
    assert_eq!(function.anchor(), Some(uuid("@DEV_G1")));
    assert_eq!(function.file_rule(), Some(FileRule::Shard));

    let ip = rec(
        r#"{"$k":"function","id":"@FN_IP24","def":"@FNDEF_IP","kind":{"ip":{"name":"24VDC-1","direction":"source"}}}"#,
    );
    assert_eq!(ip.anchor(), None);
    assert!(matches!(ip.anchor_ref(), AnchorRef::PageOfRepresentationOf(_)));
    assert_eq!(ip.file_rule(), Some(FileRule::PageFile));

    let bridge = rec(
        r#"{"$k":"connection","id":"@CON_BR1","a":{"cp":["@FN_X1031_1",3]},"b":{"cp":["@FN_X1031_2",3]},"kind":{"bridge":{"kind":"plug_in","group":1,"shaft":1}}}"#,
    );
    assert!(matches!(bridge.anchor_ref(), AnchorRef::DeviceOfFunction(_)));
    assert_eq!(bridge.file_rule(), Some(FileRule::Shard));

    let wire = rec(
        r#"{"$k":"connection","id":"@CON_W7","a":{"cp":["@FN_S51",0]},"b":{"cp":["@FN_F1",0]},"page":"@P101"}"#,
    );
    assert_eq!(wire.anchor(), Some(uuid("@P101")));
    assert_eq!(wire.file_rule(), Some(FileRule::PageFile));

    let core = rec(r#"{"$k":"conductor","id":"@COND_BN","cable":"@DEV_W106_1","core":"BN"}"#);
    assert_eq!(core.anchor(), Some(uuid("@DEV_W106_1")));

    let pot = rec(r#"{"$k":"potential_def","id":"@POT_LS","at":{"junction":"@JN_J1"},"name":"L-S"}"#);
    assert!(matches!(pot.anchor_ref(), AnchorRef::PageOfJunction(_)));
    assert_eq!(pot.file_rule(), Some(FileRule::PageFile));

    let toc = rec(
        r#"{"$k":"report_block","id":"@RB_TOC","source":{"kind":"toc"},"form":"@FORM_TOC","first_folio":2}"#,
    );
    assert_eq!(toc.anchor_ref(), AnchorRef::Root);
    assert_eq!(toc.file_rule(), Some(FileRule::ReportBlocksFile));

    let embedded = rec(
        r#"{"$k":"report_block","id":"@RB_PD201","source":{"kind":"part_data"},"form":"@FORM_TOC","first_folio":201,"embedded":{"page":"@P201","rect":[[0,0],[1,1]]}}"#,
    );
    assert_eq!(embedded.anchor(), Some(uuid("@P201")));
    assert_eq!(embedded.file_rule(), Some(FileRule::PageFile));

    let section = rec(r#"{"$k":"section","id":"@SEC1","number":1,"folio_range":[100,199]}"#);
    assert_eq!(section.anchor_ref(), AnchorRef::Root);
    assert_eq!(section.file_rule(), Some(FileRule::StructureFile));
}

/// Follows `$ref`s and picks the non-null branch of an optional member.
fn resolve(defs: &Value, node: &Value) -> Value {
    let mut node = node.clone();
    while let Some(r) = node.get("$ref").and_then(Value::as_str) {
        let name = r.trim_start_matches("#/$defs/").to_owned();
        node = defs[name.as_str()].clone();
    }
    if let Some(any) = node.get("anyOf").and_then(Value::as_array) {
        if let Some(first) = any.iter().find(|v| v.get("type") != Some(&Value::from("null"))) {
            return resolve(defs, first);
        }
    }
    node
}

/// Resolves a personal-data path (`a.b`, `a[]`) in a kind's JSON Schema.
fn schema_has_path(schema: &Value, path: &str) -> bool {
    let defs = &schema["$defs"];
    let mut node = schema.clone();
    for segment in path.split('.') {
        let (name, array) = match segment.strip_suffix("[]") {
            Some(n) => (n, true),
            None => (segment, false),
        };
        let current = resolve(defs, &node);
        let Some(mut next) = current["properties"].get(name).cloned() else {
            return false;
        };
        if array {
            next = resolve(defs, &next)["items"].clone();
            if next.is_null() {
                return false;
            }
        }
        node = resolve(defs, &next);
    }
    true
}

#[test]
fn every_personal_data_path_exists() {
    assert!(!PERSONAL_DATA_FIELDS.is_empty());
    for (kind, path) in PERSONAL_DATA_FIELDS {
        let schema = kind_schema(*kind).unwrap();
        assert!(schema_has_path(&schema, path), "{kind}: {path} does not exist");
    }
    // The checker itself rejects a wrong path.
    assert!(!schema_has_path(
        &kind_schema(Kind::Project).unwrap(),
        "parties[].nickname"
    ));
}
