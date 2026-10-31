// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! WP-S1.22 part 4 (D11 to D13): the public queries of `ew query` on Golden A, `--explain`, and the query registry's
//! snapshot `crates/ew-api/api/queries.json` (blessed with `EW_BLESS=1`).
//!
//! Golden A is opened read-only, as a reader: no lock, no state directory, nothing written.

use std::path::{Path, PathBuf};

use ew_api::cli_results::{self as results, QueryExplain};
use ew_api::queries::{QueryError, REGISTRY};
use ew_api::{MemoMode, Session};
use serde_json::{Value, json};

const GOLDEN_A: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416.ew");
const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../golden/J-E-2416/expected");

/// Golden A as a reader with the memo mode `memo` (a fresh open: an empty memo store).
fn golden(memo: MemoMode) -> Session {
    Session::open_reader_with(Path::new(GOLDEN_A), "ElektraWatt test", "query", memo)
        .expect("Golden A opens")
        .0
}

fn output(s: &Session, name: &str, key: &str) -> Value {
    s.run_query(name, key, false)
        .unwrap_or_else(|e| panic!("{name} {key}: {e}"))
        .output
}

/// The snapshot `crates/ew-api/api/queries.json`.
fn snapshot() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("api/queries.json")
}

/// Whether `EW_BLESS=1` asks for the snapshot to be rewritten.
// The flag only chooses whether the committed file is rewritten; the compared result never depends on it.
#[allow(clippy::disallowed_methods)]
fn bless() -> bool {
    std::env::var_os("EW_BLESS").is_some_and(|v| v == "1")
}

/// The fields of a line of a golden CSV file: commas outside double quotes split, `""` is a quote.
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

/// The data rows of a committed golden CSV file.
fn expected_rows(rel: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(Path::new(EXPECTED).join(rel)).unwrap();
    text.lines()
        .skip(1)
        .map(csv_fields)
        .filter(|f| f[0] == "data")
        .collect()
}

// ---------------------------------------------------------------------------------------------------------
// QU-5: the registry and its snapshot

/// QU-5 (D13): `crates/ew-api/api/queries.json` lists the twelve entries with their output schemas: an equality
/// snapshot of `ew queries --json`, rewritten with `EW_BLESS=1`.
#[test]
fn the_query_snapshot_equals_the_registry() {
    let generated = results::to_text(&results::queries());
    if bless() {
        std::fs::create_dir_all(snapshot().parent().unwrap()).unwrap();
        std::fs::write(snapshot(), &generated).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(snapshot()).unwrap_or_default();
    assert!(
        committed == generated,
        "crates/ew-api/api/queries.json is out of date; run `EW_BLESS=1 cargo nextest run -p ew-api --all-features \
         -E 'test(the_query_snapshot_equals_the_registry)'` and commit the result"
    );
    let value: Value = serde_json::from_str(&committed).unwrap();
    assert_eq!(value["$schema"], results::QUERIES);
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
// QU-1 and QU-3: the golden examples

/// QU-1: `device_tag tag:-101F1` prints the full designation and the displayed tag.
#[test]
fn device_tag_prints_the_full_and_the_displayed_tag() {
    let s = golden(MemoMode::On);
    let a = s.run_query("device_tag", "tag:-101F1", false).unwrap();
    assert_eq!(
        a.output,
        json!({"full": "=P01+J-E-2416-101F1", "displayed": "-101F1"})
    );
    assert_eq!(a.text(), "full       =P01+J-E-2416-101F1\ndisplayed  -101F1\n");
    assert_eq!(a.explain, None);
}

/// QU-3: every registry entry answers its golden example on Golden A. The facts come from the specification
/// (36 folios; -101F1 on folio 101; eight addresses of -201U5) and from Golden A's committed files (the numbers of
/// folio 102 and the 101.2 wire in `wires.csv`, the rows of `bom_parts.csv`).
#[test]
fn every_query_answers_its_golden_example() {
    let s = golden(MemoMode::On);
    let mut answered = Vec::new();
    for q in REGISTRY {
        let out = output(&s, q.name, q.example);
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
                        "1", "2", "2.1", "3", "100", "101", "102", "103", "106", "200", "201", "202", "203",
                        "204", "205", "206", "900", "901", "901.1", "902", "903", "904", "905", "906", "907",
                        "907.1", "907.2", "908", "908.1", "908.2", "909", "910", "910.1", "910.2", "910.3",
                        "911"
                    ]
                );
                assert_eq!(folios.len(), 36);
                assert_eq!(folios, s.page_labels());
            }
            "cell_of" => assert_eq!(out, json!({"folio": "101", "cell": "B8"})),
            "channel_addresses" => {
                let want: Vec<Value> = (0..8)
                    .map(|n| json!({"number": n, "address": format!("Q2.{n}")}))
                    .collect();
                assert_eq!(out, json!({ "channels": want }));
            }
            "device_tag" => assert_eq!(out, json!({"full": "=P01+J-E-2416-101F1", "displayed": "-101F1"})),
            "displayed_tag" => assert_eq!(out, json!({"displayed": "-101F1"})),
            "plc_address_text" => assert_eq!(out, json!({"text": "Q0.1"})),
            "potential_name" => assert_eq!(
                out,
                json!({
                    "name": "PE",
                    "aliases": ["J-E-206-PE", "PE-1.1", "PE-1.2", "PE-1.3", "PE-1.4", "PE-AIR1", "PE-AIR2"]
                })
            ),
            "title_block" => {
                assert_eq!(out["values"]["page.folio"], "101");
                assert_eq!(out["values"]["page.title"], "Power Supply 1 - Main Current");
                assert_eq!(out["values"]["doc.page_count"], "36");
            }
            "wire_numbers" => {
                let want: Vec<String> = expected_rows("csv/wires.csv")
                    .into_iter()
                    .map(|r| r[1].clone())
                    .filter(|n| n.starts_with("102."))
                    .collect();
                assert_eq!(want.len(), 13);
                assert_eq!(out, json!({ "numbers": want }));
            }
            "wire_props_of" => {
                // `wires.csv`: 101.2, N, BU, 1.5 mm², from the rules.
                assert_eq!(
                    out,
                    json!({
                        "potential": "N",
                        "wires": [{
                            "number": "101.2",
                            "cross_section": {
                                "value": "1.5",
                                "via": "potential N · size profile '0.75/1.5' · class mains"
                            },
                            "colour": {"value": "BU", "via": "potential N · colour profile 'DBU/WH 24 V'"}
                        }]
                    })
                );
            }
            "xref_text_of" => assert_eq!(out, json!({"text": "∞ 3-B2, 106-G3"})),
            "bom_rows" => {
                let rows = out["rows"].as_array().unwrap();
                let want = expected_rows("csv/bom_parts.csv");
                assert_eq!(rows.len(), want.len());
                for (row, csv) in rows.iter().zip(&want) {
                    // Pos, Qty, Manufacturer, Part number, Folios (space-separated in the CSV).
                    assert_eq!(row["position"], csv[1].as_str(), "{csv:?}");
                    assert_eq!(row["quantity"], csv[2].as_str(), "{csv:?}");
                    assert_eq!(row["manufacturer"], csv[4].as_str(), "{csv:?}");
                    assert_eq!(row["part_number"], csv[5].as_str(), "{csv:?}");
                }
            }
            other => panic!("{other} has no golden example in this test"),
        }
    }
    assert_eq!(answered, REGISTRY.iter().map(|q| q.name).collect::<Vec<_>>());
}

/// The other key forms: a bare tag for a terminal's potential, `id:` for a device, a terminal by its bare tag.
#[test]
fn keys_take_every_reference_form_of_their_kind() {
    let s = golden(MemoMode::On);
    assert_eq!(output(&s, "potential_name", "-X1031:1")["name"], "24VDC-1");
    let id = s.resolve_reference("tag:-101F1").unwrap();
    assert_eq!(
        output(&s, "device_tag", &format!("id:{id}")),
        output(&s, "device_tag", "-101F1")
    );
    let folio_101: Vec<String> = expected_rows("csv/wires.csv")
        .into_iter()
        .map(|r| r[1].clone())
        .filter(|n| n.starts_with("101."))
        .collect();
    assert_eq!(
        output(&s, "wire_numbers", "folio:101"),
        json!({ "numbers": folio_101 })
    );
}

// ---------------------------------------------------------------------------------------------------------
// The failure paths (QU-3's unresolvable key; D11's kinds, project owner (delegated) ruling 3)

/// Every refusal names what went wrong in words: the replay's message for a key that names no record, and for a
/// record of another kind what it is and what the query takes ("Every adapter checks the resolved record's kind",
/// ruling 3 of cycle 41).
#[test]
fn refusals_name_the_key_and_what_the_query_takes() {
    let s = golden(MemoMode::On);
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
        "`folio:101` names page 101 Power Supply 1 - Main Current; `device_tag` takes a device (`tag:-101F1` or \
         `id:<uuid>`)"
    );
    assert_eq!(
        err("channel_addresses", "tag:-101F1"),
        "`tag:-101F1` names device -101F1, which is not a PLC module; `channel_addresses` takes a PLC module \
         (`tag:-201U5`)"
    );
    // FT-721's channel form is a command-log reference, not this key: a terminal is a device of another kind.
    assert_eq!(
        err("channel_addresses", "-X1031:5"),
        "`-X1031:5` names device -X1031:5, which is not a PLC module; `channel_addresses` takes a PLC module \
         (`tag:-201U5`)"
    );
    assert_eq!(err("page_order", "x"), "`page_order` takes no key: give `-`");
    assert_eq!(err("bom_rows", "tag:-101F1"), "`bom_rows` takes no key: give `-`");
    assert_eq!(
        err("potential_name", "tag:-101F1"),
        "`tag:-101F1` names device -101F1, which is not a terminal; `potential_name` takes a wire or a junction \
         (`id:<uuid>`) or a terminal (`tag:-X1031:1`)"
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
        "`folio:101` names page 101 Power Supply 1 - Main Current; `plc_address_text` takes a function (`id:<uuid>`) \
         or a device (`tag:-201U3-Q1`, its main function)"
    );
    assert_eq!(
        err("cell_of", "-X99"),
        "`-X99`: no device prints this tag; a reference is a tag (`-101Q1`, a terminal as `-X1031:5`), \
         `tag:<tag>`, `section:<number>`, `folio:<label>` or `id:<uuid>`"
    );
}

// ---------------------------------------------------------------------------------------------------------
// QU-2 and QU-4: `--explain`

/// QU-2 (FR-EXT-005's acceptance; doc 17 §3.4's example without the time and the file paths): `--explain` adds the
/// policy line, the dependencies (the tag specification, the identifier set and the numbering rule, the main
/// representation on folio 101, the structure =P01 and +J-E-2416) and the provenance of the tag row's `via`. A fresh
/// open prints a miss: the hit flag is read before the key is resolved ("explain reads memo_info and the trail before
/// the value is computed and prints hit from that reading", ruling 1 of cycle 41).
#[test]
fn explain_names_the_dependencies_of_a_device_tag() {
    let s = golden(MemoMode::On);
    let a = s.run_query("device_tag", "tag:-101F1", true).unwrap();
    assert_eq!(
        a.text(),
        "device_tag tag:-101F1 @ rev 0
  full       =P01+J-E-2416-101F1
  displayed  -101F1
  policy     Memo · device_tag · verified at rev 0 (miss) · changed at rev 0
  depends on
    device -101F1 · tag specification: letter F, counter 1 (rev 0)
    index functions by device at device -101F1 (rev 0)
    function of -101F1 (rev 0)
    index reps by function at function of -101F1 (rev 0)
    main representation of -101F1 on folio 101 (rev 0)
    rep_structure_box(main representation of -101F1 on folio 101) (rev 0)
    page 101 Power Supply 1 - Main Current (rev 0)
    structure node =P01 (rev 0)
    structure node +J-E-2416 (rev 0)
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
    let s = golden(MemoMode::On);
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
/// only ("A family with Policy::Cold has no trail: print its policy line only", ruling 1 of cycle 41). `cell_of` backs
/// the main-location row and prints its provenance; `wire_props_of` prints none ("no provenance line for it at
/// MVP", ruling 2 of cycle 41).
#[test]
fn explain_of_a_cold_family_is_its_policy_line() {
    let s = golden(MemoMode::On);
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
  cell       B8
  policy     Cold · rep_anchor_cell is computed on demand and has no memo
  provenance main representation
"
    );
    let shown = s
        .run_query("displayed_tag", "tag:-101F1,folio:101", true)
        .unwrap();
    assert_eq!(shown.explain.unwrap().family, "displayed_tag");
    let wire = s
        .run_query("wire_props_of", "id:13c75a2c-b8f2-5b51-8e22-4cb6522139b0", true)
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
    let s = golden(MemoMode::Off);
    let e = s.run_query("page_order", "-", true).unwrap().explain.unwrap();
    assert_eq!(e.policy, "Memo · the session keeps no memo store");
    assert_eq!((e.hit, e.verified_at), (None, None));
}

/// QU-4: two fresh opens print the same text for every query with `--explain`; `--explain` changes no memo state (the
/// memory per family is equal before and after an explained query whose value was already read), and the
/// derived-data oracle of `ew verify-derived` (`verify_derived`, unchanged) finds no difference after it.
#[test]
fn explain_is_deterministic_and_changes_no_memo_state() {
    let texts = |s: &Session| -> Vec<String> {
        REGISTRY
            .iter()
            .map(|q| s.run_query(q.name, q.example, true).unwrap().text())
            .collect()
    };
    let a = golden(MemoMode::On);
    let b = golden(MemoMode::On);
    assert_eq!(texts(&a), texts(&b));

    // `verify_derived` compares every memoised key with its cold value itself; the session's own oracle is not needed.
    let s = golden(MemoMode::On);
    for q in REGISTRY {
        let _ = s.run_query(q.name, q.example, false).unwrap();
    }
    let memo = s.memo().expect("a memoising session");
    let before = memo.memory();
    for q in REGISTRY {
        let e = s.run_query(q.name, q.example, true).unwrap().explain.unwrap();
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
