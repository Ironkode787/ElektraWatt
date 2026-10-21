// SPDX-FileCopyrightText: 2026 The ElektraWatt contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! The import of the committed golden tables: report, counts, order, identity, library, endpoints, positions
//! and determinism.

mod common;

use ew_ids::Id;
use ew_import_tables::ids::{self, record_id};
use ew_interop_api::{Code, ImportBatch, Severity};
use ew_schema::{
    Bond, Connection, ConnectionKind, Device, Endpoint, Function, ImportLineage, LibDocument, LineageAnchor,
    Project, Record, Representation,
};
use ew_units::{Grid, Nm, Pt2};

fn golden() -> ImportBatch {
    common::import(&common::golden_inputs()).expect("the golden tables import")
}

fn connection(batch: &ImportBatch, key: &str) -> Connection {
    let id: Id<Connection> = record_id("connections", key);
    batch
        .records
        .iter()
        .find_map(|r| match r {
            Record::Connection(c) if c.id == id => Some(c.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("connection {key} is emitted"))
}

fn cp(function_key: &str, idx: u16) -> Endpoint {
    Endpoint::Cp(record_id::<Function>("functions", function_key), idx)
}

fn lib_key(d: &LibDocument) -> Option<String> {
    match d {
        LibDocument::Symbol(c) => c.item.meta.key.as_ref().map(|k| k.to_string()),
        LibDocument::Fndef(c) => c.item.meta.key.as_ref().map(|k| k.to_string()),
        LibDocument::Form(c) => c.item.meta.key.as_ref().map(|k| k.to_string()),
        _ => None,
    }
}

/// Only the two schema-v0 gaps (the profiles) are reported, each once as an info.
#[test]
fn the_report_has_only_the_two_known_infos() {
    let b = golden();
    let bad: Vec<_> = b
        .report
        .entries
        .iter()
        .filter(|e| matches!(e.severity, Severity::Warning | Severity::Error))
        .collect();
    assert!(bad.is_empty(), "{bad:#?}");
    let infos: Vec<_> = b.report.with_code(Code::imp(4)).collect();
    assert_eq!(b.report.entries.len(), 2, "{:#?}", b.report.entries);
    assert_eq!(infos.len(), 2);
    assert!(infos.iter().all(|e| e.severity == Severity::Info));
    let mut files: Vec<String> = infos
        .iter()
        .map(|e| e.location.as_ref().unwrap().file.clone())
        .collect();
    files.sort();
    assert_eq!(files, ["colour_profile.csv", "size_profile.csv"]);
}

/// The per-kind counts of the report are those of doc 20.
#[test]
fn the_per_kind_counts_are_those_of_doc_20() {
    let b = golden();
    let n = |k: &str| b.report.stats.per_kind.get(k).copied().unwrap_or(0);
    // doc 20 §5: 13 drawn pages.
    assert_eq!(n("page"), 13);
    // doc 20 §4.3: sections 1, 2, 9; =P01, +J-E-2416, +J-E-206, +FIELD; four external points (§15).
    assert_eq!(n("section"), 3);
    assert_eq!(n("structure_node"), 4);
    assert_eq!(n("external_ref"), 4);
    // doc 20 §6.1 and §9: 68 devices plus 28 terminal devices.
    assert_eq!(n("device"), 96);
    // doc 20 §8.2, §8.3: 72 drawn wire parts plus 19 bridge connections.
    assert_eq!(n("connection"), 91);
    // doc 20 §8.3: J1 to J6; §10: 13 conductors; §7.3: 18 potentials without DP; §7.4: B1.
    assert_eq!(n("junction"), 6);
    assert_eq!(n("conductor"), 13);
    assert_eq!(n("potential_def"), 18);
    assert_eq!(n("bond"), 1);
    // doc 20 §16, §17.3: 11 page-oriented and 4 embedded report blocks.
    assert_eq!(n("report_block"), 15);
    // doc 20 §14.1: 23 BOM rows with a part number.
    assert_eq!(n("part"), 23);
    assert_eq!(n("project"), 1);
    assert_eq!(n("import_lineage"), 1);
    // doc 20 §4.1: the revisions V1.0 and V2.0.
    assert_eq!(n("revision"), 2);
}

/// The revision records carry label, date, state, the author's party role and the page notes (doc 03 §18).
#[test]
fn the_revisions_are_the_two_of_doc_20() {
    let b = golden();
    let revisions: Vec<&serde_json::Value> = b
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Unknown(v) if v.get("$k").and_then(|k| k.as_str()) == Some("revision") => Some(v),
            _ => None,
        })
        .collect();
    let by_label = |l: &str| {
        revisions
            .iter()
            .find(|v| v["label"] == l)
            .unwrap_or_else(|| panic!("revision {l}"))
    };
    let (v1, v2) = (by_label("V1.0"), by_label("V2.0"));
    assert_eq!(
        (v1["state"].as_str(), v1["date"].as_str()),
        (Some("released"), Some("2025-03-03"))
    );
    assert_eq!(
        (v2["state"].as_str(), v2["date"].as_str()),
        (Some("open"), Some("2025-04-14"))
    );
    assert_eq!(v2["author"], "engineer");
    assert!(v1.get("author").is_none() && v2.get("checker").is_none());
    // Changed pages 101 and 106 are the page notes of V2.0; V1.0 has none.
    let page = |key: &str| serde_json::json!(record_id::<ew_schema::Page>("pages", key));
    assert_eq!(
        v2["page_notes"],
        serde_json::json!([{"page": page("101")}, {"page": page("106")}])
    );
    assert!(v1.get("page_notes").is_none());
    let id: Id<()> = record_id("revisions", "V2.0");
    assert_eq!(v2["id"], serde_json::json!(id));
}

/// Records are sorted by (kind rank, id), library documents by (kind name, copy id).
#[test]
fn records_and_library_are_sorted() {
    let b = golden();
    let keys: Vec<_> = b
        .records
        .iter()
        .map(|r| (r.kind().and_then(|k| k.rank()), r.id()))
        .collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "records are not sorted");
    let lib: Vec<_> = b
        .lib
        .iter()
        .map(|d| {
            let id = match d {
                LibDocument::Part(c) => c.id.as_uuid(),
                LibDocument::Symbol(c) => c.id.as_uuid(),
                LibDocument::Fndef(c) => c.id.as_uuid(),
                LibDocument::Form(c) => c.id.as_uuid(),
                LibDocument::Unknown(_) => panic!("unknown library document"),
            };
            (d.kind().map(|k| k.name()), id)
        })
        .collect();
    assert!(lib.windows(2).all(|w| w[0] <= w[1]), "library is not sorted");
}

/// Record IDs, source refs, the lineage record and the project ID follow the identity rule.
#[test]
fn identity_follows_the_rule() {
    let b = golden();
    let id: Id<Device> = record_id("devices", "-101F1");
    let dev = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Device(d) if d.id == id => Some(d),
            _ => None,
        })
        .expect("device -101F1");
    let sr = dev.source_ref.as_ref().expect("source_ref");
    assert_eq!(sr.key, "devices|-101F1");
    assert_eq!(sr.lineage, ids::lineage_id());

    let lineage = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::ImportLineage(l) => Some::<&ImportLineage>(l),
            _ => None,
        })
        .expect("lineage record");
    assert_eq!(lineage.id, ids::lineage_id());
    assert_eq!(lineage.importer.as_str(), "csv");
    assert!(matches!(&lineage.anchor, LineageAnchor::Fixed(l) if l.as_str() == "golden"));
    assert_eq!(lineage.namespace.as_uuid(), ids::golden_namespace());

    let project = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Project(p) => Some::<&Project>(p),
            _ => None,
        })
        .expect("project");
    assert_eq!(project.id, ids::project_id("J-E-2416"));

    let parts: Vec<_> = b
        .lib
        .iter()
        .filter_map(|d| {
            if let LibDocument::Part(p) = d {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(parts.len(), 23);
    for p in parts {
        assert_eq!(p.library_id.as_uuid(), project.id.as_uuid());
    }
}

/// The embedded library holds exactly the referenced starter content.
#[test]
fn the_embedded_library_is_the_referenced_closure() {
    let b = golden();
    let keys: Vec<String> = b.lib.iter().filter_map(lib_key).collect();
    for k in [
        "ew.iec.mcb-1pn@qet10",
        "ew.fn.mcb-1pn",
        "ew.frame.qet-a4-landscape-grid",
        "ew.frame.qet-a4-landscape-nogrid",
        "ew.titleblock.qet-a4",
    ] {
        assert!(keys.iter().any(|x| x == k), "{k} is not embedded; have {keys:?}");
    }
    assert!(
        !keys.iter().any(|x| x == "ew.iec.motor-3ph@qet10"),
        "an unreferenced symbol is embedded"
    );
}

/// Endpoint notation resolves to the documented CP indexes, connection kinds and bond vias.
#[test]
fn endpoints_resolve_to_cp_indexes() {
    let b = golden();
    let w11 = connection(&b, "W11");
    assert_eq!(w11.a, cp("L-1@102", 0));
    assert_eq!(w11.b, cp("-102G1", 0));
    // The clamp b of a terminal is pattern position 1.
    assert_eq!(connection(&b, "W35.103").a, cp("-X1031:12", 1));
    assert_eq!(connection(&b, "W5").b, cp("S51", 3));
    assert_eq!(connection(&b, "W27").kind, ConnectionKind::Wire);
    let bond = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Bond(x) => Some::<&Bond>(x),
            _ => None,
        })
        .expect("bond B1");
    assert_eq!(bond.via, Some(record_id::<Connection>("connections", "W27")));
}

/// A representation without a named anchor sits at the cell centre on the page grid.
#[test]
fn a_representation_sits_at_its_cell_position() {
    let b = golden();
    let id: Id<Representation> = record_id("representations", "S51");
    let rep = b
        .records
        .iter()
        .find_map(|r| match r {
            Record::Representation(x) if x.id == id => Some(x),
            _ => None,
        })
        .expect("representation S51");
    // Cell B2 of the grid: column 2, row B (index 1); the symbol has no named anchor, so no anchor offset.
    assert_eq!(rep.pos.x, Grid::QET_10PX.at(5 + 6 * 2).unwrap());
    assert_eq!(rep.pos.y, Grid::QET_10PX.at(6 + 8).unwrap());
}

/// The 28 valve-terminal valves, 8 process valves and 8 actuators on the folios 203-206 select the arrangement
/// `ports` (doc 20 §6.2: valve and field tags are not printed there); every other representation has none.
#[test]
fn valve_and_field_representations_select_the_ports_arrangement() {
    let b = golden();
    let mut keys = Vec::new();
    for m in [
        "-201U3", "-201U4", "-201U5", "-201U6", "-201U7", "-201U8", "-202U2",
    ] {
        for q in 1..=4 {
            keys.push(format!("{m}-Q{q}"));
        }
    }
    for n in 1..=8 {
        keys.push(format!("-203Q{n}"));
        keys.push(format!("-203Q{n}#actuator"));
    }
    let want: std::collections::BTreeSet<Id<Representation>> =
        keys.iter().map(|k| record_id("representations", k)).collect();
    assert_eq!(want.len(), 28 + 8 + 8, "{keys:?}");
    let mut with = 0;
    for r in &b.records {
        if let Record::Representation(rep) = r {
            let expected = want.contains(&rep.id).then_some("ports");
            assert_eq!(
                rep.arrangement.as_ref().map(|a| a.to_string()).as_deref(),
                expected,
                "{:?}",
                rep.source_ref
            );
            with += usize::from(rep.arrangement.is_some());
        }
    }
    assert_eq!(with, 44);
}

/// Exactly the four black boxes with a CP pattern carry box geometry: top row first, in pattern order, on the
/// page grid (doc 07 §4.2); the `#aux` boxes without a pattern and every other representation have none.
#[test]
fn black_boxes_carry_their_cp_pattern_geometry() {
    let b = golden();
    let grid = Grid::QET_10PX;
    let mut boxed = std::collections::BTreeMap::new();
    let mut total = 0;
    for r in &b.records {
        if let Record::Representation(rep) = r {
            total += 1;
            if let Some(spec) = &rep.box_ {
                boxed.insert(rep.id, (rep.pos, spec.clone()));
            }
        }
    }
    // (key, top count, bottom count)
    let expect = [
        ("-102G1", 3, 4),
        ("-102F2", 5, 5),
        ("-201U1#power", 3, 3),
        ("-202U1#power", 3, 3),
    ];
    assert_eq!(
        boxed.len(),
        expect.len(),
        "{:?}",
        boxed.keys().collect::<Vec<_>>()
    );
    assert!(total > expect.len());
    for (key, top, bottom) in expect {
        let id: Id<Representation> = record_id("representations", key);
        let (pos, spec) = boxed.get(&id).unwrap_or_else(|| panic!("{key} has a box"));
        let step = |p: Pt2| {
            (
                grid.snap(Nm(pos.x.0 + p.x.0)) - grid.snap(pos.x),
                grid.snap(Nm(pos.y.0 + p.y.0)) - grid.snap(pos.y),
            )
        };
        let cps: Vec<_> = spec.cps.iter().map(|c| (c.idx, step(c.pos))).collect();
        assert_eq!(cps.len(), top + bottom, "{key}");
        for (i, (idx, (x, y))) in cps.iter().enumerate() {
            assert_eq!(usize::from(*idx), i, "{key}: CP index order");
            if i < top {
                assert_eq!((*x, *y), (1 + 2 * i as i64, 0), "{key} top {i}");
            } else {
                assert_eq!((*x, *y), (1 + 2 * (i - top) as i64, 6), "{key} bottom {i}");
            }
        }
        let outline: Vec<_> = spec.outline.iter().map(|p| step(*p)).collect();
        assert_eq!(outline, [(0, 0), (10, 0), (10, 6), (0, 6)], "{key}");
        assert!(spec.inner.is_empty());
    }
}

/// Two imports give byte-equal records, library documents and equal report entries.
#[test]
fn the_import_is_deterministic() {
    let (a, b) = (golden(), golden());
    let json = |x: &ImportBatch| -> Vec<String> {
        x.records
            .iter()
            .map(|r| serde_json::to_string(r).unwrap())
            .chain(x.lib.iter().map(|d| serde_json::to_string(d).unwrap()))
            .collect()
    };
    assert_eq!(json(&a), json(&b));
    assert_eq!(a.report.entries, b.report.entries);
}
