// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The public queries of `ew query` on the sample, `--explain`, and the query registry as `ew queries --json` prints
//! it.
//!
//! The sample is opened read-only, as a reader: no lock, no state directory, nothing written. The registry's own
//! examples name keys of the reference project; each query is asked here with a key of the sample instead
//! ([`sample_key`]).

use std::path::{Path, PathBuf};

use ew_api::cli_results::{self as results, QueryExplain};
use ew_api::queries::{QueryError, REGISTRY};
use ew_api::{MemoMode, Session};
use serde_json::{Value, json};

/// The sample as a reader with the memo mode `memo` (a fresh open: an empty memo store).
fn sample(memo: MemoMode) -> Session {
    Session::open_reader_with(Path::new(ew_testdir::SAMPLE), "ElektraWatt test", "query", memo)
        .expect("the sample opens")
        .0
}

/// The ID of the sample's wire numbered `number`.
fn wire(s: &Session, number: &str) -> String {
    s.navigator_table(ew_api::output::NavKind::Connections)
        .nodes
        .iter()
        .find(|n| n.table_row && n.label == number)
        .map(|n| n.records[0].to_string())
        .unwrap_or_else(|| panic!("no wire {number}"))
}

/// The key each query is asked with on the sample; a query this test does not know fails here.
fn sample_key(s: &Session, name: &str) -> String {
    match name {
        "bom_rows" | "medium_zones" | "page_order" => "-".to_owned(),
        "cell_of" | "device_tag" => "tag:-101F1".to_owned(),
        "channel_addresses" => "tag:-104A1-A2".to_owned(),
        "displayed_tag" => "tag:-101F1,folio:101".to_owned(),
        "plc_address_text" => "tag:-201K1-Q2".to_owned(),
        "potential_name" => "-X1011:L1".to_owned(),
        "title_block" => "folio:101".to_owned(),
        "tube_props" => {
            let q = s.query();
            let line = q
                .connections()
                .iter()
                .find(|(_, c)| matches!(c.kind, ew_schema::ConnectionKind::Fluid(_)))
                .map(|(id, _)| id.to_string())
                .expect("a tube");
            format!("id:{line}")
        }
        "wire_numbers" => "folio:102".to_owned(),
        "wire_props_of" => format!("id:{}", wire(s, "101.2")),
        "xref_text_of" => "tag:-102Q1".to_owned(),
        other => panic!("{other} has no key on the sample in this test"),
    }
}

/// The values of column `column` of the export's data rows.
fn column(out: &ew_api::output::RowsCsv, column: usize) -> Vec<String> {
    out.csv
        .lines()
        .skip(1)
        .map(csv_fields)
        .filter(|f| f[0] == "data")
        .map(|f| f[column].clone())
        .collect()
}

/// The numbers of the wire list's wires on folio `folio`.
fn wire_numbers(s: &Session, folio: &str) -> Vec<String> {
    let out = ew_api::output::row_export(&s.query(), &ew_api::output::RowExport::Wires, None)
        .expect("the wire list");
    column(&out, 1)
        .into_iter()
        .filter(|n| n.starts_with(&format!("{folio}.")))
        .collect()
}

fn output(s: &Session, name: &str, key: &str) -> Value {
    s.run_query(name, key, false)
        .unwrap_or_else(|e| panic!("{name} {key}: {e}"))
        .output
}

/// The fields of a line of a CSV file: commas outside double quotes split, `""` is a quote.
fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                fields.last_mut().unwrap().push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(String::new()),
            other => fields.last_mut().unwrap().push(other),
        }
    }
    fields
}

// ---------------------------------------------------------------------------------------------------------
// The registry and its snapshot

/// `ew queries --json` lists every entry of the registry with its output schema. The list is written for
/// the user guide where `EW_REFERENCE_DIR` asks for it.
// The variable only chooses whether the generated file is also written out.
#[allow(clippy::disallowed_methods)]
#[test]
fn the_query_list_names_the_registry_with_its_output_schemas() {
    let generated = results::to_text(&results::queries());
    if let Some(dir) = std::env::var_os("EW_REFERENCE_DIR").map(PathBuf::from) {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("queries.json"), &generated).unwrap();
    }
    let value: Value = serde_json::from_str(&generated).unwrap();
    assert_eq!(value["$schema"], results::QueriesResult::SCHEMA);
    let names: Vec<&str> = value["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["name"].as_str().unwrap())
        .collect();
    let registry: Vec<&str> = REGISTRY.iter().map(|q| q.name).collect();
    assert_eq!(names, registry);
    for q in value["queries"].as_array().unwrap() {
        assert_eq!(q["output_version"], 1, "{}", q["name"]);
        assert_eq!(q["output"]["type"], "object", "{}", q["name"]);
        assert_eq!(
            q["output"]["$schema"], "https://json-schema.org/draft/2020-12/schema",
            "{}",
            q["name"]
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// The examples on the sample

/// `device_tag tag:-101F1` prints the full designation and the displayed tag.
#[test]
fn device_tag_prints_the_full_and_the_displayed_tag() {
    let s = sample(MemoMode::On);
    let a = s.run_query("device_tag", "tag:-101F1", false).unwrap();
    assert_eq!(a.output, json!({"full": "+CAB-101F1", "displayed": "-101F1"}));
    assert_eq!(a.text(), "full       +CAB-101F1\ndisplayed  -101F1\n");
    assert_eq!(a.explain, None);
}

/// Every registry entry answers on the sample. The facts come from the sample's script (27 folios; -101F1 on
/// folio 101; the output card's eight addresses from Q1) and from its other exports (the wire list's numbers on folio
/// 102, the parts list's rows).
#[test]
fn every_query_answers_on_the_sample() {
    let s = sample(MemoMode::On);
    let mut answered = Vec::new();
    for q in REGISTRY {
        let out = output(&s, q.name, &sample_key(&s, q.name));
        answered.push(q.name);
        match q.name {
            "page_order" => {
                let folios: Vec<&str> = out["folios"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| f.as_str().unwrap())
                    .collect();
                assert_eq!(
                    folios,
                    [
                        "1", "2", "2.1", "100", "101", "102", "103", "104", "105", "200", "201", "900",
                        "901", "901.1", "902", "902.1", "902.2", "903", "906", "907", "907.1", "907.2",
                        "908", "908.1", "909", "909.1", "910", "910.1", "911", "912", "912.1", "913", "913.1",
                        "914", "914.1"
                    ]
                );
                assert_eq!(folios, s.page_labels());
            }
            "cell_of" => assert_eq!(out, json!({"folio": "101", "cell": "E8"})),
            "channel_addresses" => {
                let want: Vec<Value> = (0..8)
                    .map(|n| json!({"number": n, "address": format!("Q1.{n}")}))
                    .collect();
                assert_eq!(out, json!({ "channels": want }));
            }
            "device_tag" => assert_eq!(out, json!({"full": "+CAB-101F1", "displayed": "-101F1"})),
            "displayed_tag" => assert_eq!(out, json!({"displayed": "-101F1"})),
            // The valve -201K1-Q2's coil is bound to the valve terminal's output 2.
            "plc_address_text" => assert_eq!(out, json!({"text": "Q0.2"})),
            "potential_name" => assert_eq!(out["name"], "L1", "{out}"),
            "title_block" => {
                assert_eq!(out["values"]["page.folio"], "101");
                assert_eq!(out["values"]["page.title"], "Supply 400 V AC");
                assert_eq!(out["values"]["doc.page_count"], "35");
            }
            "wire_numbers" => {
                let want = wire_numbers(&s, "102");
                assert!(!want.is_empty());
                assert_eq!(out, json!({ "numbers": want }));
            }
            "wire_props_of" => {
                // The wire list: 101.2, L2, BK, 1.5 mm², from the rules.
                assert_eq!(out["potential"], "L2", "{out}");
                let w = &out["wires"][0];
                assert_eq!(
                    (&w["number"], &w["cross_section"]["value"], &w["colour"]["value"]),
                    (&json!("101.2"), &json!("1.5"), &json!("BK")),
                    "{out}"
                );
            }
            // The contactor's coil lists its four contacts: the self-hold contact on 102, the three main contacts on
            // 103.
            "xref_text_of" => assert_eq!(out, json!({"text": "∞ 102-D5, 103-D2, 103-D3\n103-D3"})),
            // The sample defines no medium.
            "medium_zones" => assert_eq!(out, json!({"zones": []})),
            "tube_props" => assert!(out.get("od").is_some(), "{out}"),
            "bom_rows" => {
                let rows = out["rows"].as_array().unwrap();
                let bom = ew_api::output::row_export(
                    &s.query(),
                    &ew_api::output::RowExport::Bom(ew_api::BomMode::Component),
                    None,
                )
                .expect("the parts list");
                let want: Vec<Vec<String>> = bom
                    .csv
                    .lines()
                    .skip(1)
                    .map(csv_fields)
                    .filter(|f| f[0] == "data")
                    .collect();
                assert_eq!(rows.len(), want.len());
                for (row, csv) in rows.iter().zip(&want) {
                    // Pos, Qty, Manufacturer, Part number.
                    assert_eq!(row["position"], csv[1].as_str(), "{csv:?}");
                    assert_eq!(row["quantity"], csv[2].as_str(), "{csv:?}");
                    assert_eq!(row["manufacturer"], csv[4].as_str(), "{csv:?}");
                    assert_eq!(row["part_number"], csv[5].as_str(), "{csv:?}");
                }
            }
            other => panic!("{other} has no key on the sample in this test"),
        }
    }
    assert_eq!(answered, REGISTRY.iter().map(|q| q.name).collect::<Vec<_>>());
}

/// The other key forms: a bare tag for a terminal's potential, `id:` for a device, a terminal by its bare tag.
#[test]
fn keys_take_every_reference_form_of_their_kind() {
    let s = sample(MemoMode::On);
    assert_eq!(
        output(&s, "potential_name", "-X1011:N")["name"],
        "N",
        "the incoming strip's N terminal"
    );
    let id = s.resolve_reference("tag:-101F1").unwrap();
    assert_eq!(
        output(&s, "device_tag", &format!("id:{id}")),
        output(&s, "device_tag", "-101F1")
    );
    let folio_101 = wire_numbers(&s, "101");
    assert_eq!(
        output(&s, "wire_numbers", "folio:101"),
        json!({ "numbers": folio_101 })
    );
}

// ---------------------------------------------------------------------------------------------------------
// The failure paths

/// Every refusal names what went wrong in words: the replay's message for a key that names no record, and for a
/// record of another kind what it is and what the query takes ("Every adapter checks the resolved record's
/// kind").
#[test]
fn refusals_name_the_key_and_what_the_query_takes() {
    let s = sample(MemoMode::On);
    let err = |name: &str, key: &str| s.run_query(name, key, true).unwrap_err().to_string();
    assert_eq!(
        err("no_such", "-"),
        "unknown query `no_such`; `ew queries` lists the queries"
    );
    assert_eq!(
        s.run_query("no_such", "-", false).unwrap_err(),
        QueryError::Unknown("no_such".into())
    );
    assert_eq!(
        err("device_tag", "tag:-999Z9"),
        "`tag:-999Z9`: no device prints this tag"
    );
    assert_eq!(err("device_tag", "id:not-a-uuid"), "`id:not-a-uuid`: not a UUID");
    assert_eq!(
        err("device_tag", "folio:101"),
        "`folio:101` names page 101 Supply 400 V AC; `device_tag` takes a device (`tag:-101F1` or \
         `id:<uuid>`)"
    );
    assert_eq!(
        err("channel_addresses", "tag:-101F1"),
        "`tag:-101F1` names device -101F1, which is not a PLC module; `channel_addresses` takes a PLC module \
         (`tag:-104A1-A2`)"
    );
    // The channel form is a command-log reference, not this key: a terminal is a device of another kind.
    assert_eq!(
        err("channel_addresses", "-X1031:U"),
        "`-X1031:U` names device -X1031:U, which is not a PLC module; `channel_addresses` takes a PLC module \
         (`tag:-104A1-A2`)"
    );
    assert_eq!(err("page_order", "x"), "`page_order` takes no key: give `-`");
    assert_eq!(err("bom_rows", "tag:-101F1"), "`bom_rows` takes no key: give `-`");
    assert_eq!(
        err("potential_name", "tag:-101F1"),
        "`tag:-101F1` names device -101F1, which is not a terminal; `potential_name` takes a wire or a junction \
         (`id:<uuid>`) or a terminal (`tag:-X1011:L1`)"
    );
    assert_eq!(
        err("wire_props_of", "tag:-101F1"),
        "`tag:-101F1` names device -101F1; `wire_props_of` takes a wire (`id:<uuid>`)"
    );
    assert_eq!(
        err("displayed_tag", "tag:-101F1"),
        "`tag:-101F1`: `displayed_tag` takes a device and a folio (`tag:-101F1,folio:101`)"
    );
    assert_eq!(
        err("wire_numbers", "folio:901"),
        "`folio:901`: no drawn page has this folio label"
    );
    assert_eq!(
        err("title_block", "tag:-101F1"),
        "`tag:-101F1` names device -101F1; `title_block` takes a drawn page (`folio:101`)"
    );
    assert_eq!(
        err("plc_address_text", "folio:101"),
        "`folio:101` names page 101 Supply 400 V AC; `plc_address_text` takes a function (`id:<uuid>`) \
         or a device (`tag:-201K1-Q2`, its main function)"
    );
    assert_eq!(
        err("cell_of", "-X99"),
        "`-X99`: no device prints this tag; a reference is a tag (`-101Q1`, a terminal as `-X1031:5`), \
         `tag:<tag>`, `section:<number>`, `folio:<label>` or `id:<uuid>`"
    );
}

// ---------------------------------------------------------------------------------------------------------
// `--explain`

/// `--explain` adds the
/// policy line, the dependencies (the tag specification, the identifier set and the numbering rule, the main
/// representation on folio 101, the location +CAB) and the provenance of the tag row's `via`. A fresh
/// open prints a miss: the hit flag is read before the key is resolved ("explain reads memo_info and the trail before
/// the value is computed and prints hit from that reading").
#[test]
fn explain_names_the_dependencies_of_a_device_tag() {
    let s = sample(MemoMode::On);
    let a = s.run_query("device_tag", "tag:-101F1", true).unwrap();
    assert_eq!(
        a.text(),
        "device_tag tag:-101F1 @ rev 0
  full       +CAB-101F1
  displayed  -101F1
  policy     Memo · device_tag · verified at rev 0 (miss) · changed at rev 0
  depends on
    device -101F1 · tag specification: letter F, counter 1 (rev 0)
    index functions by device at device -101F1 (rev 0)
    function of -101F1 (rev 0)
    index reps by function at function of -101F1 (rev 0)
    main representation of -101F1 on folio 101 (rev 0)
    rep_structure_box(main representation of -101F1 on folio 101) (rev 0)
    page 101 Supply 400 V AC (rev 0)
    structure node +CAB (rev 0)
    project settings · identifier set, numbering rules, master data and derivation profile (rev 0)
  provenance numbering scheme 'page-based'
"
    );
    let e = a.explain.unwrap();
    assert_eq!(e.family, "device_tag");
    assert_eq!(
        (e.hit, e.verified_at, e.changed_at),
        (Some(false), Some(0), Some(0))
    );
    assert_eq!(e.provenance.as_deref(), Some("numbering scheme 'page-based'"));
}

/// The hit flag reads the memo store as the query found it: a second query of one session finds the memo.
#[test]
fn a_second_explain_in_one_session_is_a_hit() {
    let s = sample(MemoMode::On);
    let first = s.run_query("page_order", "-", true).unwrap().explain.unwrap();
    let second = s.run_query("page_order", "-", true).unwrap().explain.unwrap();
    assert_eq!(first.hit, Some(false));
    assert_eq!(second.hit, Some(true));
    assert_eq!(first.depends_on, second.depends_on);
    assert!(
        first.policy.ends_with("(miss) · changed at rev 0"),
        "{}",
        first.policy
    );
    assert!(
        second.policy.ends_with("(hit) · changed at rev 0"),
        "{}",
        second.policy
    );
}

/// A composed query explains its outermost memoised family; a family whose policy is `Cold` prints its policy line
/// only ("A family with Policy::Cold has no trail: print its policy line only"). `cell_of` backs
/// the main-location row and prints its provenance; `wire_props_of` prints none ("no provenance line for it at
/// MVP").
#[test]
fn explain_of_a_cold_family_is_its_policy_line() {
    let s = sample(MemoMode::On);
    let a = s.run_query("cell_of", "tag:-101F1", true).unwrap();
    assert_eq!(
        a.explain,
        Some(QueryExplain {
            family: "rep_anchor_cell".into(),
            policy: "Cold · rep_anchor_cell is computed on demand and has no memo".into(),
            hit: None,
            verified_at: None,
            changed_at: None,
            depends_on: Vec::new(),
            provenance: Some("main representation".into()),
        })
    );
    assert_eq!(
        a.text(),
        "cell_of tag:-101F1 @ rev 0
  folio      101
  cell       E8
  policy     Cold · rep_anchor_cell is computed on demand and has no memo
  provenance main representation
"
    );
    let shown = s
        .run_query("displayed_tag", "tag:-101F1,folio:101", true)
        .unwrap();
    assert_eq!(shown.explain.unwrap().family, "displayed_tag");
    let wire = s
        .run_query("wire_props_of", &format!("id:{}", wire(&s, "101.2")), true)
        .unwrap()
        .explain
        .unwrap();
    assert_eq!(wire.family, "wire_props_of");
    assert_eq!(wire.provenance, None);
    assert!(!wire.depends_on.is_empty());
}

/// A session without a memo store says so on the policy line.
#[test]
fn explain_without_a_memo_store_says_so() {
    let s = sample(MemoMode::Off);
    let e = s.run_query("page_order", "-", true).unwrap().explain.unwrap();
    assert_eq!(e.policy, "Memo · the session keeps no memo store");
    assert_eq!((e.hit, e.verified_at), (None, None));
}

/// Two fresh opens print the same text for every query with `--explain`; `--explain` changes no memo state (the
/// memory per family is equal before and after an explained query whose value was already read), and the
/// derived-data oracle of `ew verify-derived` (`verify_derived`, unchanged) finds no difference after it.
#[test]
fn explain_is_deterministic_and_changes_no_memo_state() {
    let texts = |s: &Session| -> Vec<String> {
        REGISTRY
            .iter()
            .map(|q| s.run_query(q.name, &sample_key(s, q.name), true).unwrap().text())
            .collect()
    };
    let a = sample(MemoMode::On);
    let b = sample(MemoMode::On);
    assert_eq!(texts(&a), texts(&b));

    // `verify_derived` compares every memoised key with its cold value itself; the session's own oracle is not needed.
    let s = sample(MemoMode::On);
    let keys: Vec<String> = REGISTRY.iter().map(|q| sample_key(&s, q.name)).collect();
    for (q, key) in REGISTRY.iter().zip(&keys) {
        let _ = s.run_query(q.name, key, false).unwrap();
    }
    let memo = s.memo().expect("a memoising session");
    let before = memo.memory();
    for (q, key) in REGISTRY.iter().zip(&keys) {
        let e = s.run_query(q.name, key, true).unwrap().explain.unwrap();
        assert_ne!(e.hit, Some(false), "{}: the value was read before", q.name);
    }
    assert_eq!(
        memo.memory(),
        before,
        "explaining adds and drops no memo or value"
    );
    let report = ew_api::verify::verify_derived(&s).expect("a memoising session");
    assert!(report.is_clean(), "{:?} {:?}", report.differences, report.oracle);
}
